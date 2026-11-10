//! Centralized registry for accessing global C++ manager instances.
//!
//! This module provides a type-safe, lazy-initialized way to access global
//! manager instances from the injected DLL. It encapsulates pointer chain
//! resolution and eliminates scattered magic numbers.

use std::ffi::CString;
use std::marker::PhantomData;
use std::sync::OnceLock;

// Forward declarations of manager types - these are defined in their respective modules
// We use opaque pointers here to avoid circular dependencies

/// Opaque type for ZTWorldMgr
#[repr(C)]
pub struct ZTWorldMgr {
    _private: [u8; 0], // Size will be determined by the actual definition
}

/// Opaque type for ZTHabitatMgr
#[repr(C)]
pub struct ZTHabitatMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTAdvTerrainMgr_raw
#[repr(C)]
pub struct ZTAdvTerrainMgr_raw {
    _private: [u8; 0],
}

/// Opaque type for ZTGameMgr
#[repr(C)]
pub struct ZTGameMgr {
    _private: [u8; 0],
}

/// Opaque type for BFResourceMgr
#[repr(C)]
pub struct BFResourceMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTResearchMgr
#[repr(C)]
pub struct ZTResearchMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTMarketingMgr
#[repr(C)]
pub struct ZTMarketingMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTThoughtMgr
#[repr(C)]
pub struct ZTThoughtMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTMegatileMgr
#[repr(C)]
pub struct ZTMegatileMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTShowMgr
#[repr(C)]
pub struct ZTShowMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTAIMgr. Out-of-scope for reimplementation (see
/// `openzt/plans/ztgamemgr-implementation-plan.md`) - only exposed so `ZTGameMgr::setNewGameDefaults` can
/// call through its real vtable slot `+0x4` (`0x0058f269`, inherited unchanged from `BFAIMgr`) exactly as
/// vanilla does.
#[repr(C)]
pub struct ZTAIMgr {
    _private: [u8; 0],
}

/// Walks a pointer chain and returns the final address.
///
/// # Arguments
/// * `base` - The starting address
/// * `offsets` - Array of offsets to apply:
///   - `&[]`: Direct access, no dereference
///   - `&[0]`: Dereference once, add 0
///   - `&[0, 0x58]`: Dereference, add 0, dereference, add 0x58
///
/// # Safety
/// This function performs raw pointer dereferencing. The caller must ensure
/// that all addresses in the chain are valid and aligned.
unsafe fn resolve_chain(base: usize, offsets: &[usize]) -> Option<usize> {
    let mut addr = base;

    // Empty offsets = direct access, no dereference
    if offsets.is_empty() {
        return Some(addr);
    }

    // For each offset: dereference, then add offset
    for &offset in offsets {
        unsafe {
            addr = *(addr as *const usize);
        }
        if addr == 0 {
            return None;
        }
        addr = addr.wrapping_add(offset);
    }

    Some(addr)
}

/// Cached global instance wrapper for stable singleton objects.
///
/// This type is used for global C++ objects that are created once and live
/// for the lifetime of the game (e.g., manager singletons). Only a *successful*
/// (non-null) resolution is cached, since these singletons don't exist yet at
/// early injection points (e.g. before a scenario/save has finished loading) -
/// latching a failed lookup would permanently strand every later caller with a
/// stale null even once the real global becomes valid.
pub struct CachedGlobalInstance<T> {
    base: usize,
    offsets: &'static [usize],
    cache: std::sync::atomic::AtomicUsize,
    _marker: PhantomData<*mut T>,
}

impl<T> CachedGlobalInstance<T> {
    /// Creates a new cached global instance.
    ///
    /// # Arguments
    /// * `base` - The base address (usually module_base + offset)
    /// * `offsets` - The pointer chain offsets
    pub const fn new(base: usize, offsets: &'static [usize]) -> Self {
        Self {
            base,
            offsets,
            cache: std::sync::atomic::AtomicUsize::new(0),
            _marker: PhantomData,
        }
    }

    /// Returns a raw pointer to the instance, or null if resolution fails.
    ///
    /// A non-null resolution is cached (the chain is re-walked on every call
    /// until the first success); a failed resolution is never cached, so a
    /// singleton that doesn't exist yet is retried on the next call instead of
    /// being stuck at null forever.
    pub unsafe fn get(&self) -> *mut T {
        use std::sync::atomic::Ordering;
        let cached = self.cache.load(Ordering::Relaxed);
        if cached != 0 {
            return cached as *mut T;
        }
        let addr = unsafe { resolve_chain(self.base, self.offsets).unwrap_or(0) };
        if addr != 0 {
            self.cache.store(addr, Ordering::Relaxed);
        }
        addr as *mut T
    }
}

// SAFETY: CachedGlobalInstance only contains plain integers (base address and offsets)
// and an AtomicUsize, which is thread-safe. It's safe to share across threads.
unsafe impl<T> Send for CachedGlobalInstance<T> {}
unsafe impl<T> Sync for CachedGlobalInstance<T> {}

/// Centralized registry for all global manager instances.
pub struct Globals {
    ztworldmgr: CachedGlobalInstance<ZTWorldMgr>,
    zthabitatmgr: CachedGlobalInstance<ZTHabitatMgr>,
    ztadvterrainmgr: CachedGlobalInstance<ZTAdvTerrainMgr_raw>,
    ztgamemgr: CachedGlobalInstance<ZTGameMgr>,
    bfresourcemgr: CachedGlobalInstance<BFResourceMgr>,
    ztresearchmgr: CachedGlobalInstance<ZTResearchMgr>,
    ztmarketingmgr: CachedGlobalInstance<ZTMarketingMgr>,
    ztthoughtmgr: CachedGlobalInstance<ZTThoughtMgr>,
    ztmegatilemgr: CachedGlobalInstance<ZTMegatileMgr>,
    ztshowmgr: CachedGlobalInstance<ZTShowMgr>,
    ztaimgr: CachedGlobalInstance<ZTAIMgr>,
}

impl Globals {
    /// Returns a shared reference to the ZTWorldMgr (read-only).
    pub fn ztworldmgr(&self) -> &crate::ztworldmgr::ZTWorldMgr {
        unsafe {
            &*(self.ztworldmgr.get() as *const crate::ztworldmgr::ZTWorldMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTWorldMgr for mutation.
    pub fn ztworldmgr_ptr(&self) -> *mut crate::ztworldmgr::ZTWorldMgr {
        unsafe {
            self.ztworldmgr.get() as *mut crate::ztworldmgr::ZTWorldMgr
        }
    }

    /// Returns a shared reference to the ZTHabitatMgr (read-only).
    pub fn zthabitatmgr(&self) -> &crate::zthabitatmgr::ZTHabitatMgr {
        unsafe {
            &*(self.zthabitatmgr.get() as *const crate::zthabitatmgr::ZTHabitatMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTHabitatMgr for mutation.
    pub fn zthabitatmgr_ptr(&self) -> *mut crate::zthabitatmgr::ZTHabitatMgr {
        unsafe {
            self.zthabitatmgr.get() as *mut crate::zthabitatmgr::ZTHabitatMgr
        }
    }

    /// Returns a shared reference to the ZTAdvTerrainMgr_raw (read-only).
    pub fn ztadvterrainmgr(&self) -> &crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw {
        unsafe {
            &*(self.ztadvterrainmgr.get() as *const crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw)
        }
    }

    /// Returns a raw mutable pointer to the ZTAdvTerrainMgr_raw for mutation.
    pub fn ztadvterrainmgr_ptr(&self) -> *mut crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw {
        unsafe {
            self.ztadvterrainmgr.get() as *mut crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw
        }
    }

    /// Returns a shared reference to the ZTGameMgr (read-only).
    pub fn ztgamemgr(&self) -> &crate::ztgamemgr::ZTGameMgr {
        unsafe {
            &*(self.ztgamemgr.get() as *const crate::ztgamemgr::ZTGameMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTGameMgr for mutation.
    pub fn ztgamemgr_ptr(&self) -> *mut crate::ztgamemgr::ZTGameMgr {
        unsafe {
            self.ztgamemgr.get() as *mut crate::ztgamemgr::ZTGameMgr
        }
    }

    /// Returns a shared reference to the BFResourceMgr (read-only).
    pub fn bfresourcemgr(&self) -> &crate::resource_manager::bfresourcemgr::BFResourceMgr {
        unsafe {
            &*(self.bfresourcemgr.get() as *const crate::resource_manager::bfresourcemgr::BFResourceMgr)
        }
    }

    /// Returns a raw mutable pointer to the BFResourceMgr for mutation.
    pub fn bfresourcemgr_ptr(&self) -> *mut crate::resource_manager::bfresourcemgr::BFResourceMgr {
        unsafe {
            self.bfresourcemgr.get() as *mut crate::resource_manager::bfresourcemgr::BFResourceMgr
        }
    }

    /// Returns a shared reference to the ZTResearchMgr (read-only).
    pub fn ztresearchmgr(&self) -> &crate::ztresearch::ZTResearchMgr {
        unsafe {
            &*(self.ztresearchmgr.get() as *const crate::ztresearch::ZTResearchMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTResearchMgr for mutation.
    pub fn ztresearchmgr_ptr(&self) -> *mut crate::ztresearch::ZTResearchMgr {
        unsafe {
            self.ztresearchmgr.get() as *mut crate::ztresearch::ZTResearchMgr
        }
    }

    /// Returns a shared reference to the ZTMarketingMgr (read-only).
    pub fn ztmarketingmgr(&self) -> &crate::ztmarketing::ZTMarketingMgr {
        unsafe {
            &*(self.ztmarketingmgr.get() as *const crate::ztmarketing::ZTMarketingMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTMarketingMgr for mutation.
    pub fn ztmarketingmgr_ptr(&self) -> *mut crate::ztmarketing::ZTMarketingMgr {
        unsafe {
            self.ztmarketingmgr.get() as *mut crate::ztmarketing::ZTMarketingMgr
        }
    }

    /// Returns a shared reference to the ZTThoughtMgr (read-only).
    pub fn ztthoughtmgr(&self) -> &crate::ztthoughtmgr::ZTThoughtMgr {
        unsafe {
            &*(self.ztthoughtmgr.get() as *const crate::ztthoughtmgr::ZTThoughtMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTThoughtMgr for mutation.
    pub fn ztthoughtmgr_ptr(&self) -> *mut crate::ztthoughtmgr::ZTThoughtMgr {
        unsafe {
            self.ztthoughtmgr.get() as *mut crate::ztthoughtmgr::ZTThoughtMgr
        }
    }

    /// Returns a shared reference to the ZTMegatileMgr (read-only).
    pub fn ztmegatilemgr(&self) -> &crate::ztmegatilemgr::ZTMegatileMgr {
        unsafe {
            &*(self.ztmegatilemgr.get() as *const crate::ztmegatilemgr::ZTMegatileMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTMegatileMgr for mutation.
    pub fn ztmegatilemgr_ptr(&self) -> *mut crate::ztmegatilemgr::ZTMegatileMgr {
        unsafe {
            self.ztmegatilemgr.get() as *mut crate::ztmegatilemgr::ZTMegatileMgr
        }
    }

    /// Returns a shared reference to the ZTShowMgr (read-only). Note this is `ZTShowMgr` itself, not the
    /// `ZTShowScriptMgr` embedded within it at `+0x34` (that data lives in `ztshowscriptmgr.rs`'s own
    /// independent Rust store, not read through this accessor).
    pub fn ztshowmgr(&self) -> &crate::ztshowmgr::ZTShowMgr {
        unsafe {
            &*(self.ztshowmgr.get() as *const crate::ztshowmgr::ZTShowMgr)
        }
    }

    /// Returns a raw pointer to `ZTShowMgr`, possibly null (mirrors vanilla's own `GLOBAL_ZTShowMgr != 0`
    /// guard before dereferencing it - see `ztshow::do_trick_event`'s threshold lookup).
    pub fn ztshowmgr_ptr(&self) -> *const crate::ztshowmgr::ZTShowMgr {
        unsafe {
            self.ztshowmgr.get() as *const crate::ztshowmgr::ZTShowMgr
        }
    }

    /// Returns a raw pointer to the real vanilla `ZTAIMgr` singleton (`GLOBAL_ZTAIMgr`), possibly null.
    /// `ZTAIMgr` itself is out of scope for reimplementation - this is only exposed as an opaque `*const
    /// u32`-shaped handle to pass as `this` into its real vtable slot `+0x4` call (see
    /// `ztgamemgr::set_new_game_defaults`), never dereferenced as a typed struct.
    pub fn ztaimgr_ptr(&self) -> *const u32 {
        unsafe { self.ztaimgr.get() as *const u32 }
    }
}

// SAFETY: Globals only contains CachedGlobalInstance values which are Send + Sync
unsafe impl Send for Globals {}
unsafe impl Sync for Globals {}

/// Static storage for the globals registry.
static GLOBALS: OnceLock<Globals> = OnceLock::new();

/// Gets the module base address for the given module name.
///
/// # Arguments
/// * `name` - The name of the module (e.g., "zoo.exe")
///
/// # Panics
/// Panics if the module cannot be found.
pub fn get_module_base(name: &str) -> usize {
    // zoo.exe is never unloaded and this is called per-entity in hot loops.
    static ZOO_EXE_BASE: OnceLock<usize> = OnceLock::new();
    if name == "zoo.exe" {
        return *ZOO_EXE_BASE.get_or_init(|| lookup_module_base(name));
    }
    lookup_module_base(name)
}

fn lookup_module_base(name: &str) -> usize {
    let cname = CString::new(name).unwrap();
    unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleA(
            windows::core::PCSTR(cname.as_ptr() as _)
        )
        .unwrap()
        .0 as usize
    }
}

/// Ensures the globals registry is initialized and returns it.
///
/// The registry is lazily initialized on first access. This function is
/// thread-safe and can be called from any thread.
fn ensure_globals() -> &'static Globals {
    GLOBALS.get_or_init(|| {
        let base = get_module_base("zoo.exe");
        Globals {
            // All offsets are &[0] because each address points to a pointer to the struct
            // (single indirection)
            ztworldmgr: CachedGlobalInstance::new(base + 0x00238040, &[0]),
            zthabitatmgr: CachedGlobalInstance::new(base + 0x0023805c, &[0]),
            ztadvterrainmgr: CachedGlobalInstance::new(base + 0x00238058, &[0]),
            ztgamemgr: CachedGlobalInstance::new(base + 0x00238048, &[0]),
            // BFResourceMgr uses empty offsets because the global address points directly
            // to the struct (no indirection)
            bfresourcemgr: CachedGlobalInstance::new(base + 0x002380C0, &[]),
            // 0x639010 (Ghidra address, default base 0x400000) -> RVA 0x239010
            ztresearchmgr: CachedGlobalInstance::new(base + 0x00239010, &[0]),
            // 0x639000 (Ghidra address, default base 0x400000) -> RVA 0x239000
            ztmarketingmgr: CachedGlobalInstance::new(base + 0x00239000, &[0]),
            // 0x639090 (Ghidra address, default base 0x400000) -> RVA 0x239090
            ztthoughtmgr: CachedGlobalInstance::new(base + 0x00239090, &[0]),
            // 0x639004 (Ghidra address, default base 0x400000) -> RVA 0x239004. Confirmed directly via
            // Ghidra (OOAnalyzer-assigned symbol `GLOBAL_ZTMegatileMgr`), in the same neighborhood as
            // ZTMarketingMgr/ZTResearchMgr/ZTThoughtMgr (0x639000/0x639010/0x639090).
            ztmegatilemgr: CachedGlobalInstance::new(base + 0x00239004, &[0]),
            // GLOBAL_ZTShowMgr, Ghidra VA 0x00639008 -> RVA 0x00239008 (same neighborhood cluster as the
            // managers above) - see ztshowscriptmgr-implementation-plan.md's "Composition" section.
            ztshowmgr: CachedGlobalInstance::new(base + 0x00239008, &[0]),
            // GLOBAL_ZTAIMgr, Ghidra VA 0x00638098 -> RVA 0x00238098 (same neighborhood cluster as
            // ZTWorldMgr/ZTGameMgr/ZTAdvTerrainMgr/ZTHabitatMgr at 0x238040-0x23805c above).
            ztaimgr: CachedGlobalInstance::new(base + 0x00238098, &[0]),
        }
    })
}

/// Returns the global manager registry.
///
/// This is the main entry point for accessing global managers.
///
/// # Example
/// ```ignore
/// let world_mgr = globals().ztworldmgr();
/// println!("Map size: {}x{}", world_mgr.map_x_size, world_mgr.map_y_size);
/// ```
pub fn globals() -> &'static Globals {
    ensure_globals()
}
