//! A real, vanilla-allocator-owned `std::string` (MSVC `{char* ptr; u32 len; u32 capacity}`, 12 bytes,
//! no small-string-optimization buffer for this build - confirmed independently by
//! `MenuMusicHandler_MenuMusicHandler.asm`'s stack-slot accounting). Shared by every
//! class-reimplementation file that needs to build a temporary vanilla `std::string` argument or
//! receive one via the hidden-return-pointer (RVO) convention - construction/destruction always go
//! through vanilla's own allocator (`BASIC_STRING_2`/`BASIC_STRING_0`), never Rust's, per `CLAUDE.md`'s
//! cross-allocator hazard.

use std::ffi::c_void;

use openzt_detour::generated::msvc_std_basic_string::{BASIC_STRING_0, BASIC_STRING_2};

#[repr(C)]
pub(crate) struct VanillaString {
    ptr: *mut u8,
    len: u32,
    capacity: u32,
}

impl VanillaString {
    /// Constructs from an iterator range `[str.as_ptr(), str.as_ptr() + str.len())`, matching
    /// `BASIC_STRING_2`'s real vanilla calling convention exactly (`this, first, last, allocator` - the
    /// trailing allocator argument is an uninitialized/unused stack slot in vanilla's own call site too,
    /// passed as `0` here).
    pub(crate) fn new(s: &str) -> Self {
        let mut this = VanillaString { ptr: std::ptr::null_mut(), len: 0, capacity: 0 };
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
        VanillaString { ptr: std::ptr::null_mut(), len: 0, capacity: 0 }
    }

    pub(crate) fn as_ptr(&self) -> *const u32 {
        self as *const VanillaString as *const u32
    }

    pub(crate) fn as_str(&self) -> std::borrow::Cow<'_, str> {
        if self.ptr.is_null() || self.len == 0 {
            return std::borrow::Cow::Borrowed("");
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, self.len as usize) };
        String::from_utf8_lossy(bytes)
    }
}

impl Drop for VanillaString {
    fn drop(&mut self) {
        unsafe { BASIC_STRING_0.original()(self as *mut VanillaString as *const c_void) };
    }
}
