use std::{
    ffi::{c_char, CStr, CString},
    fmt, marker,
    mem::transmute,
    path::PathBuf,
    ptr,
};

#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS};

/// Allows memory utility functions to accept either raw `u32` addresses or typed raw pointers.
/// On 32-bit Windows all pointers are 4 bytes, so a pointer's numeric value is a valid `u32` address.
pub trait MemAddr: Copy {
    fn as_u32(self) -> u32;
}
impl MemAddr for u32 {
    fn as_u32(self) -> u32 { self }
}
impl<T> MemAddr for *const T {
    fn as_u32(self) -> u32 { self as u32 }
}
impl<T> MemAddr for *mut T {
    fn as_u32(self) -> u32 { self as u32 }
}

/// Display wrapper for memory addresses. Accepts any `MemAddr` (u32 or raw pointer).
/// Use in format strings instead of `ptr as u32`:
///   info!("{:#x}", Addr::of(this_ptr));
pub struct Addr(pub u32);

impl Addr {
    pub fn of(addr: impl MemAddr) -> Self { Addr(addr.as_u32()) }
}

impl fmt::LowerHex for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::LowerHex::fmt(&self.0, f) }
}
impl fmt::UpperHex for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::UpperHex::fmt(&self.0, f) }
}
impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:#010x}", self.0) }
}

pub unsafe fn ref_from_memory<T>(address: impl MemAddr) -> &'static T {
    unsafe { &*(address.as_u32() as *const T) }
}

pub unsafe fn mut_from_memory<T>(address: impl MemAddr) -> &'static mut T {
    unsafe { &mut *(address.as_u32() as *mut T) }
}

// TODO: Test replacing most uses of get_from_memory with map_from_memory : Unclear if we need mem::forget each reference afterwards?
pub fn map_from_memory<T>(address: impl MemAddr) -> &'static mut T {
    unsafe { transmute::<u32, &mut T>(address.as_u32()) }
}

pub fn get_from_memory<T>(address: impl MemAddr) -> T {
    unsafe { ptr::read_volatile(address.as_u32() as *const T) }
}

pub fn checked_get_from_memory<T: Checkable>(address: impl MemAddr) -> anyhow::Result<T> {
    let addr = address.as_u32();
    T::check(addr)?;
    Ok(unsafe { ptr::read_volatile(addr as *const T) })
}

pub fn save_to_memory<T>(address: impl MemAddr, value: T) {
    unsafe { ptr::write_volatile(address.as_u32() as *mut T, value) };
}

/// Interprets a raw vanilla return value as a bool using only its low byte, ignoring the upper 3 bytes.
/// Ghidra frequently decompiles a function that really only sets `AL` as
/// `return CONCAT31((int3)(garbage >> 8), local_flag)` (or the sibling shape `some_reg & 0xffffff00`,
/// forcing the low byte to a fixed value while leaving the upper bytes as whatever was already in the
/// register) - an explicitly undefined/garbage upper 3 bytes around the one real byte. Real vanilla
/// callers always match this with `TEST AL, AL` (low byte only); comparing the full return value against
/// `0` is a genuine bug that happens to work whenever the garbage bytes are zero and silently breaks once
/// they aren't. Found twice so far - `ZooStatus::fChance` and `ZTHabitatMgr::createHabitat`'s
/// `doTankCheck`/`ZTTankExhibit::removeIllegalEntities` reads - see `CLAUDE.md`'s reimplementation-pattern
/// section. Always check the raw decompile's own `return` statements for `CONCAT31`/`extraout_*` before
/// deciding whether a given call needs this instead of a plain `!= 0`.
pub fn low_byte_bool(value: u32) -> bool {
    (value & 0xff) != 0
}

#[cfg(target_os = "windows")]
pub fn save_to_protected_memory<T>(address: u32, value: T) -> anyhow::Result<()> {
    unsafe {
        {
            let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(address as *mut _, std::mem::size_of::<T>(), PAGE_EXECUTE_READWRITE, &mut old_protect)?;
            ptr::write(address as *mut _, value);
            VirtualProtect(address as *mut _, std::mem::size_of::<T>(), old_protect, &mut old_protect)?;
        }
    }
    Ok(())
}

pub fn get_base_path() -> PathBuf {
    let mut exe_location = std::env::current_exe().unwrap();
    exe_location.pop();
    exe_location
}

pub fn get_ini_path() -> PathBuf {
    let mut exe_location = get_base_path();
    exe_location.push("zoo.ini");
    exe_location
}

#[cfg(target_os = "windows")]
pub fn patch_nop(address: u32) -> anyhow::Result<()> {
    unsafe {
        {
            let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(address as *mut _, 1, PAGE_EXECUTE_READWRITE, &mut old_protect)?;
            ptr::write(address as *mut _, 0x90u8);
            VirtualProtect(address as *mut _, 1, old_protect, &mut old_protect)?;
        }
    }
    Ok(())
}

pub fn get_string_from_memory_with_size(address: u32, size: u32) -> String {
    get_string_from_memory_bounded(address, address + size, address + size)
}

pub fn get_string_from_memory_bounded(start: u32, end: u32, buffer_end: u32) -> String {
    let mut bytes = Vec::new();
    let mut char_address = start;
    while char_address < end && char_address < buffer_end {
        let byte = get_from_memory::<u8>(char_address);
        if byte == 0 {
            break;
        }
        bytes.push(byte);
        char_address += 1;
    }
    crate::encoding_utils::decode_game_text(&bytes)
}

pub fn get_string_from_memory(address: impl MemAddr) -> String {
    let mut bytes = Vec::new();
    let mut char_address = address.as_u32();
    while {
        let byte = get_from_memory::<u8>(char_address);
        byte != 0
    } {
        bytes.push(get_from_memory::<u8>(char_address));
        char_address += 1;
    }
    crate::encoding_utils::decode_game_text(&bytes)
}

pub fn save_string_to_memory(address: impl MemAddr, string: &str) {
    let encoded_bytes = crate::encoding_utils::encode_to_ansi(string);
    let mut char_address = address.as_u32();
    for byte in encoded_bytes {
        save_to_memory::<u8>(char_address, byte);
        char_address += 1;
    }
    save_to_memory::<u8>(char_address, 0);
}

// TODO: What else should we have here?
// TODO: Impl for ZTString (rename this), ZTShortString and a simple cstr/*const c_char wrapper?
// TODO: Should implement helper methods for any struct that implements ZTString, this should just be the minimum interface requried
pub trait ZTString {
    fn len(&self) -> usize;
    fn capacity(&self) -> usize;
    fn replace(&mut self, new_string: String) -> anyhow::Result<()>;
    fn get_cstr(&self) -> &CStr;
    fn copy_to_string(&self) -> String;
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ZTBufferString {
    start_ptr: u32,
    end_ptr: u32,
    buffer_end_ptr: u32,
}

impl ZTBufferString {
    /// Builds a `ZTBufferString` from raw pointers - the write-side counterpart to `raw_parts`, for
    /// callers constructing/repointing the buffer themselves (e.g. allocating via Rust's own
    /// allocator instead of vanilla's game heap).
    pub fn from_raw_parts(start_ptr: u32, end_ptr: u32, buffer_end_ptr: u32) -> Self {
        Self { start_ptr, end_ptr, buffer_end_ptr }
    }

    /// The raw `(start_ptr, end_ptr, buffer_end_ptr)` triple, for callers that need to
    /// reconstruct/free the underlying buffer themselves.
    pub fn raw_parts(&self) -> (u32, u32, u32) {
        (self.start_ptr, self.end_ptr, self.buffer_end_ptr)
    }
}

impl fmt::Display for ZTBufferString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.copy_to_string())
    }
}

impl ZTString for ZTBufferString {
    fn len(&self) -> usize {
        (self.end_ptr - self.start_ptr) as usize
    }

    fn capacity(&self) -> usize {
        (self.buffer_end_ptr - self.start_ptr) as usize
    }

    fn replace(&mut self, new_string: String) -> anyhow::Result<()> {
        if new_string.len() + 1 > self.capacity() {
            Err(anyhow::anyhow!("New string is too long"))
        } else {
            let new_end_ptr = self.start_ptr + new_string.len() as u32;
            save_string_to_memory(self.start_ptr, &new_string);
            self.end_ptr = new_end_ptr;
            Ok(())
        }
    }

    fn get_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.start_ptr as *const c_char) }
    }

    fn copy_to_string(&self) -> String {
        get_string_from_memory_bounded(self.start_ptr, self.end_ptr, self.buffer_end_ptr)
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ZTBoundedString {
    start_ptr: u32,
    end_ptr: u32,
}

impl ZTString for ZTBoundedString {
    fn len(&self) -> usize {
        (self.end_ptr - self.start_ptr) as usize
    }

    fn capacity(&self) -> usize {
        self.len()
    }

    fn replace(&mut self, new_string: String) -> anyhow::Result<()> {
        if new_string.len() + 1 != self.capacity() {
            Err(anyhow::anyhow!("New string is too long"))
        } else {
            let new_end_ptr = self.start_ptr + new_string.len() as u32;
            save_string_to_memory(self.start_ptr, &new_string);
            self.end_ptr = new_end_ptr;
            Ok(())
        }
    }

    fn get_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.start_ptr as *const c_char) }
    }

    fn copy_to_string(&self) -> String {
        get_string_from_memory_bounded(self.start_ptr, self.end_ptr, self.end_ptr)
    }
}

impl fmt::Display for ZTBoundedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.copy_to_string())
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ZTStringPtr {
    ptr: *const c_char,
}

impl ZTString for ZTStringPtr {
    fn len(&self) -> usize {
        unsafe { CStr::from_ptr(self.ptr).to_bytes().len() }
    }

    fn capacity(&self) -> usize {
        self.len()
    }

    fn replace(&mut self, _new_string: String) -> anyhow::Result<()> {
        // TODO: We could probably implement this, by getting the current length of the string and making sure the new string is the exact same size? Or padding with spaces if smaller?
        Err(anyhow::anyhow!("Cannot replace string without bounds"))
    }

    fn get_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.ptr) }
    }

    fn copy_to_string(&self) -> String {
        unsafe { CStr::from_ptr(self.ptr).to_string_lossy().into_owned() }
    }
}

impl fmt::Display for ZTStringPtr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.copy_to_string())
    }
}

impl From<CString> for ZTStringPtr {
    fn from(c_string: CString) -> Self {
        ZTStringPtr { ptr: c_string.into_raw() }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct ZTArray<T> {
    // ptr: *const T,
    start_ptr: u32,
    end_ptr: u32,
    buffer_end_ptr: u32,
    // Rust doesn't allow us to have a struct that is generic over T without referencing it,
    //  because we're just storing pointers to an array of type T we use PhantomData to tell the compiler that we're using T
    _marker: marker::PhantomData<T>,
    // _marker: marker::PhantomData<&'a T>,
}

impl<T> fmt::Display for ZTArray<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ZTArray {{ start_ptr: {:#x}, end_ptr: {:#x}, buffer_end_ptr: {:#x} }} -> len({})",
            self.start_ptr,
            self.end_ptr,
            self.buffer_end_ptr,
            self.len()
        )
    }
}

impl<T> ZTArray<T> {
    /// Builds a `ZTArray<T>` from raw pointers - the write-side counterpart to `raw_parts`, for
    /// callers constructing/repointing the array themselves (e.g. allocating via Rust's own
    /// allocator instead of vanilla's game heap).
    pub fn from_raw_parts(start_ptr: u32, end_ptr: u32, buffer_end_ptr: u32) -> Self {
        Self { start_ptr, end_ptr, buffer_end_ptr, _marker: marker::PhantomData }
    }

    /// The raw `(start_ptr, end_ptr, buffer_end_ptr)` triple, for callers that need to
    /// reconstruct/free the underlying buffer themselves.
    ///
    /// Reads through [`get_from_memory`] rather than a plain `self.start_ptr` field access: this
    /// array's 3 words are live vanilla game memory that real, un-detoured vanilla code (or another
    /// call through [`Self::set_raw_parts`]) can change independent of anything Rust's aliasing model
    /// can see. `get_from_memory`'s volatile read is what keeps repeated calls from being treated as
    /// re-derivable from a single load and cached/CSE'd - see [`Self::set_raw_parts`]'s own doc
    /// comment for the write-side manifestation of the same hazard.
    pub fn raw_parts(&self) -> (u32, u32, u32) {
        let addr = self as *const Self as u32;
        (get_from_memory(addr), get_from_memory(addr + 4), get_from_memory(addr + 8))
    }

    /// Overwrites this array's own 3 live memory words in place (its own address, not a Rust-side
    /// copy - `#[repr(C)]` guarantees `start_ptr`/`end_ptr`/`buffer_end_ptr` sit at `+0x0`/`+0x4`/`+0x8`
    /// of `self`) - for callers reimplementing a real vanilla vector-growth method (e.g.
    /// `ZTHabitatMgr::addHabitat`) that must mutate a live `ZTArray` field embedded in a larger live
    /// game struct, which Rust's ownership model otherwise has no way to reach (the field is private,
    /// and the containing struct is usually only reachable through a shared `&self`).
    ///
    /// Must go through [`save_to_memory`], not a plain `ptr::write` through a pointer derived from
    /// `&self`: `&self` being a shared reference lets the optimizer assume the memory it points to is
    /// never mutated for the reference's lifetime, and in a release build this was observed to
    /// silently drop the write to `start_ptr` entirely (confirmed live: real vanilla code reading this
    /// same field via completely independent, un-detoured machine code kept reading `0` after this
    /// call, even though `end_ptr`/`buffer_end_ptr` read back correctly) while leaving the other two
    /// words - each a separate, independently-optimizable store - untouched by whatever eliminated the
    /// first. `save_to_memory`'s volatile write is never eliminated or reordered by the optimizer
    /// regardless of that aliasing assumption.
    pub fn set_raw_parts(&self, start_ptr: u32, end_ptr: u32, buffer_end_ptr: u32) {
        let addr = self as *const Self as u32;
        save_to_memory(addr, start_ptr);
        save_to_memory(addr + 4, end_ptr);
        save_to_memory(addr + 8, buffer_end_ptr);
    }

    pub fn len(&self) -> usize {
        let (start, end, _) = self.raw_parts();
        ((end - start) / 4) as usize
    }

    pub fn capacity(&self) -> usize {
        let (start, _, buffer_end) = self.raw_parts();
        ((buffer_end - start) / 4) as usize
    }

    pub fn get_ptr(&self, index: usize) -> u32 {
        let (start, _, _) = self.raw_parts();
        get_from_memory(start + (index * 4) as u32)
    }

    pub fn get(&self, index: usize) -> T {
        get_from_memory::<T>(self.get_ptr(index))
    }

    pub fn set(&self, index: usize, value: T) {
        save_to_memory(self.get_ptr(index), value);
    }

    pub fn get_vec(&self) -> Vec<T> {
        let mut vec = Vec::new();
        for i in 0..self.len() {
            vec.push(self.get(i));
        }
        vec
    }
}

pub trait Checkable {
    fn check(ptr: u32) -> anyhow::Result<()>;
}
