//! A real, vanilla-allocator-owned `std::string` (MSVC `{char* begin; char* end; char* cap_end}`, 12 bytes,
//! no small-string-optimization buffer for this build - confirmed independently by
//! `MenuMusicHandler_MenuMusicHandler.asm`'s stack-slot accounting). Shared by every
//! class-reimplementation file that needs to build a temporary vanilla `std::string` argument or
//! receive one via the hidden-return-pointer (RVO) convention - construction/destruction always go
//! through vanilla's own allocator (`BASIC_STRING_2`/`BASIC_STRING_0`), never Rust's, per `CLAUDE.md`'s
//! cross-allocator hazard.

use std::ffi::c_void;

use openzt_detour::generated::msvc_std_basic_string::{BASIC_STRING_0, BASIC_STRING_2};

use crate::util::get_from_memory;

/// The bytes of a vanilla `{begin, end}` string range, empty when it is unset (`begin` null or `end` not past it).
/// Reads volatile bytes straight from vanilla memory, so the string may be one the caller does not own.
pub(crate) fn range_bytes(begin: u32, end: u32) -> Vec<u8> {
    if begin == 0 || end <= begin {
        return Vec::new();
    }
    (begin..end).map(get_from_memory::<u8>).collect()
}

/// [`range_bytes`] of the string object (`{begin, end, ..}`) at `address`.
pub(crate) fn object_bytes(address: u32) -> Vec<u8> {
    range_bytes(get_from_memory(address), get_from_memory(address + 4))
}

/// The bytes of the NUL-terminated string at `address`, without the terminator.
pub(crate) fn c_string_bytes(address: u32) -> Vec<u8> {
    (address..).map(get_from_memory::<u8>).take_while(|&byte| byte != 0).collect()
}

#[repr(C)]
pub(crate) struct VanillaString {
    ptr: *mut u8,
    end: *mut u8,
    cap_end: *mut u8,
}

impl VanillaString {
    /// Constructs from an iterator range `[str.as_ptr(), str.as_ptr() + str.len())`, matching
    /// `BASIC_STRING_2`'s real vanilla calling convention exactly (`this, first, last, allocator` - the
    /// trailing allocator argument is an uninitialized/unused stack slot in vanilla's own call site too,
    /// passed as `0` here).
    pub(crate) fn new(s: &str) -> Self {
        Self::from_bytes(s.as_bytes())
    }

    /// [`Self::new`] for arbitrary bytes (a call parameter need not be UTF-8).
    pub(crate) fn from_bytes(s: &[u8]) -> Self {
        let mut this = VanillaString { ptr: std::ptr::null_mut(), end: std::ptr::null_mut(), cap_end: std::ptr::null_mut() };
        let start = s.as_ptr();
        let end = unsafe { start.add(s.len()) };
        unsafe {
            BASIC_STRING_2.original()(&mut this as *mut VanillaString as *const c_void, start as *const u32, end as i32, 0);
        }
        this
    }

    /// A zeroed instance suitable as an RVO out-parameter for a vanilla function that
    /// placement-constructs its return value at the address it's given - the callee never reads this
    /// memory before writing it, so a zeroed instance is a safe destination.
    pub(crate) fn rvo_target() -> Self {
        VanillaString { ptr: std::ptr::null_mut(), end: std::ptr::null_mut(), cap_end: std::ptr::null_mut() }
    }

    pub(crate) fn words(&self) -> [u32; 3] {
        [self.ptr as u32, self.end as u32, self.cap_end as u32]
    }

    /// Hands the string's three words to a caller that takes ownership (vanilla frees them), without running the destructor.
    pub(crate) fn release(self) -> [u32; 3] {
        let words = self.words();
        std::mem::forget(self);
        words
    }

    pub(crate) fn as_ptr(&self) -> *const u32 {
        self as *const VanillaString as *const u32
    }

    /// The string's bytes (empty when unset).
    pub(crate) fn bytes(&self) -> Vec<u8> {
        if self.ptr.is_null() || self.end <= self.ptr {
            return Vec::new();
        }
        unsafe { std::slice::from_raw_parts(self.ptr, self.end as usize - self.ptr as usize) }.to_vec()
    }

    pub(crate) fn as_str(&self) -> std::borrow::Cow<'_, str> {
        if self.ptr.is_null() || self.end <= self.ptr {
            return std::borrow::Cow::Borrowed("");
        }
        let len = self.end as usize - self.ptr as usize;
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, len) };
        String::from_utf8_lossy(bytes)
    }
}

impl Drop for VanillaString {
    fn drop(&mut self) {
        unsafe { BASIC_STRING_0.original()(self as *mut VanillaString as *const c_void) };
    }
}
