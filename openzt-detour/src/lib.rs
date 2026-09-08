use std::marker::PhantomData;

use retour::GenericDetour;

pub mod generated;
pub mod structs;

pub struct FunctionDef<T> {
    pub address: u32,
    function_type: PhantomData<T>,
}

impl<T> FunctionDef<T> {
    /// Builds a `FunctionDef` for an address with a signature not (yet) reflected in `generated.rs`.
    ///
    /// `generated.rs` is auto-generated from a Ghidra analysis pass run outside this repo - per
    /// `CLAUDE.md`, it must never be hand-edited, even when a specific entry's signature is known to be
    /// wrong or incomplete (missing a hidden-return-pointer parameter, wrong ABI, etc.). This constructor
    /// lets a call site define its own corrected, local `FunctionDef` for the same address instead of
    /// waiting on a regeneration - a stopgap, not a substitute for eventually fixing the real entry.
    pub const fn new(address: u32) -> Self {
        Self { address, function_type: PhantomData }
    }
}

impl<T> FunctionDef<T>
where
    T: retour::Function,
{
    /// Creates an (initially disabled) detour for this function's address with `target` as its
    /// replacement body. In debug builds the detour's trampoline is also published to a
    /// process-global registry (see [`hook_registry`]) so that [`FunctionDef::original`] keeps
    /// returning the real vanilla function once this address is hooked.
    ///
    /// # Safety
    ///
    /// This function will cause issues if the address or signature is not correct.
    pub unsafe fn detour(self, target: T) -> Result<GenericDetour<T>, retour::Error> {
        let detour = unsafe { GenericDetour::<T>::new(::retour::Function::from_ptr(self.address as *const ()), target) };
        #[cfg(debug_assertions)]
        if let Ok(detour) = &detour {
            hook_registry::register(self.address, detour);
        }
        detour
    }

    // TODO: Would be nice to have a `call` that calls the original function without having to detour it first.
    /// The real vanilla function at this address, regardless of whether a detour is installed
    /// there. In debug builds this routes through the registered trampoline, so calling the result
    /// never re-enters this crate's own detour; in release builds it is a raw cast of the address,
    /// so **calling it from inside that same function's own detour re-enters the detour** - always
    /// use the macro-generated `<NAME>_DETOUR.call(...)` trampoline inside a detour body instead.
    /// When the intent is deliberately to re-enter the hook, use [`FunctionDef::hooked`].
    ///
    /// # Safety
    ///
    /// This function will cause issues if the address is not correct
    pub unsafe fn original(&self) -> T {
        #[cfg(debug_assertions)]
        if let Some(trampoline) = hook_registry::trampoline_for(self.address) {
            return unsafe { <T as ::retour::Function>::from_ptr(trampoline as *const ()) };
        }
        unsafe { ::retour::Function::from_ptr(self.address as *const ()) }
    }

    /// The function currently installed at this address - this crate's own detour body if a hook
    /// is enabled there, otherwise the real vanilla function. That is exactly what a vanilla
    /// caller invoking the address would execute, and it is byte-identical to a raw cast of
    /// `address` in every build. Use ONLY where re-entering the hook is the intent (e.g. a
    /// reimplementation that wants the string-registry-aware behavior of `BFApp::loadString`); for
    /// the real vanilla body use [`FunctionDef::original`].
    ///
    /// # Safety
    ///
    /// This function will cause issues if the address is not correct
    pub unsafe fn hooked(&self) -> T {
        unsafe { ::retour::Function::from_ptr(self.address as *const ()) }
    }
}

/// Debug-only registry mapping hooked addresses to the trampoline retour generated for them, so
/// [`FunctionDef::original`] keeps returning the real vanilla function in every hook state instead
/// of silently re-entering the detour. Entries are written once, from `FunctionDef::detour` (the
/// sole construction path for a `GenericDetour` in this workspace), and the trampoline is built by
/// `GenericDetour::new` - valid before `enable()` and held by the `GenericDetour` for process
/// lifetime (each one is a macro-generated `static LazyLock`) - so a registered entry is always
/// dereferenceable.
///
/// Lock-free: registration scans for a free slot, lookups scan all slots. Overflow fails open (the
/// address stays unregistered and `original()` falls back to a raw cast) and bumps
/// [`registry_overflow_count`], which the live reimplementation battery asserts is zero.
#[cfg(debug_assertions)]
mod hook_registry {
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use retour::GenericDetour;

    /// Sized from an audit of the workspace: 172 `#[detour]` sites, ~166 installable
    /// simultaneously with the `experimental` feature set (the remainder are test-only and never
    /// co-install with them).
    const CAPACITY: usize = 256;

    /// Each slot packs `(address: u32) << 32 | (trampoline as u32)`; `0` = empty (trampolines are
    /// never null). 32-bit pointers only - this crate is i686-only (the `thiscall` ABI it detours
    /// with is Windows-x86-specific).
    static SLOTS: [AtomicU64; CAPACITY] = [const { AtomicU64::new(0) }; CAPACITY];

    static OVERFLOW_COUNT: AtomicUsize = AtomicUsize::new(0);

    /// Idempotent: re-registering an address (mutually-exclusive feature-gated modules can
    /// generate detours for one shared address) overwrites the previous entry, last writer wins.
    pub(super) fn register<T: retour::Function>(address: u32, detour: &GenericDetour<T>) {
        let trampoline = detour.trampoline() as *const () as usize;
        let entry = ((address as u64) << 32) | trampoline as u64;
        for slot in SLOTS.iter() {
            match slot.compare_exchange(0, entry, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return,
                Err(current) => {
                    if (current >> 32) as u32 == address {
                        slot.store(entry, Ordering::Release);
                        return;
                    }
                }
            }
        }
        OVERFLOW_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn trampoline_for(address: u32) -> Option<usize> {
        let prefix = (address as u64) << 32;
        for slot in SLOTS.iter() {
            let current = slot.load(Ordering::Acquire);
            if (current & !0xffff_ffff) == prefix {
                return Some((current & 0xffff_ffff) as usize);
            }
        }
        None
    }

    pub(super) fn registry_overflow_count() -> usize {
        OVERFLOW_COUNT.load(Ordering::Relaxed)
    }
}

/// The trampoline registered for `address` by [`FunctionDef::detour`], or `None` if no detour has
/// been constructed for it (debug builds only - the registry does not exist in release). Returned
/// as a raw pointer value; transmute it back to `FunctionDef`'s function type to call.
#[cfg(debug_assertions)]
pub fn trampoline_for(address: u32) -> Option<usize> {
    hook_registry::trampoline_for(address)
}

/// Number of addresses that failed to register because [`hook_registry`]'s fixed slot array was
/// full. Zero is the invariant the live reimplementation battery asserts; nonzero means
/// [`FunctionDef::original`] silently degrades to a raw cast for the unregistered addresses
/// (debug builds only - the registry does not exist in release).
#[cfg(debug_assertions)]
pub fn registry_overflow_count() -> usize {
    hook_registry::registry_overflow_count()
}
