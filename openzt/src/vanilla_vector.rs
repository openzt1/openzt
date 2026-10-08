//! Vanilla MSVC `std::vector` layout compatibility.
//!
//! Models the 3-word (`begin`, `end`, `cap_end`) layout used by MSVC's `std::vector`
//! across FFI and RVO boundaries.

use crate::util::{get_from_memory, save_to_memory};
use openzt_detour::generated::poolalloc::ALLOCATE as POOLALLOC_ALLOCATE;

/// Appends one 4-byte `value` to the vanilla `std::vector<T*>`-shaped `{begin, end, cap_end}` header at
/// `vector_ptr`, reproducing vanilla's growth: when full, `PoolAlloc::allocate` a buffer of double the
/// element count (one element for an empty vector), copy the old elements across, store `value`, hand the old
/// buffer to `free_old_buffer(begin, byte_capacity)` and rewrite all three header words. The old buffer's
/// teardown differs between vanilla call sites (manual freelist split, `PoolAlloc::deallocate`,
/// `PoolAlloc::deallocate_n_4`), so the caller supplies it; it runs even for an empty vector (`begin == 0`).
///
/// Works on a raw header address, never a `&mut`, because the header lives inside live game structs that
/// vanilla can re-enter. Only valid for headers whose buffer vanilla's `PoolAlloc` produced (cross-allocator rule).
pub fn push_word(vector_ptr: u32, value: u32, free_old_buffer: impl FnOnce(u32, u32)) {
    let begin = get_from_memory::<u32>(vector_ptr);
    let end = get_from_memory::<u32>(vector_ptr + 4);
    let cap_end = get_from_memory::<u32>(vector_ptr + 8);

    if end == cap_end {
        let old_len = (end - begin) / 4;
        let new_cap = if old_len == 0 { 1 } else { old_len * 2 };
        let new_buf = unsafe { POOLALLOC_ALLOCATE.original()(new_cap * 4) } as u32;

        for i in 0..old_len {
            let v: u32 = get_from_memory(begin + i * 4);
            if new_buf != 0 {
                save_to_memory(new_buf + i * 4, v);
            }
        }
        if new_buf != 0 {
            save_to_memory(new_buf + old_len * 4, value);
        }
        free_old_buffer(begin, cap_end - begin);

        save_to_memory(vector_ptr, new_buf);
        save_to_memory(vector_ptr + 4, new_buf + (old_len + 1) * 4);
        save_to_memory(vector_ptr + 8, new_buf + new_cap * 4);
    } else {
        save_to_memory(end, value);
        save_to_memory(vector_ptr + 4, end + 4);
    }
}

/// Removes the `element_size`-byte element at `element_ptr` from the vector header at `vector_ptr`: every later
/// element shifts down one slot and `end` shrinks by `element_size` (vanilla's plain `memmove`-shaped erase; no
/// reallocation, no element destruction - callers tear the element down first). `element_ptr` must lie in
/// `[begin, end)`.
pub fn erase_element(vector_ptr: u32, element_ptr: u32, element_size: u32) {
    let end = get_from_memory::<u32>(vector_ptr + 4);
    let mut dst = element_ptr;
    let mut src = element_ptr + element_size;
    while src != end {
        for word in (0..element_size).step_by(4) {
            save_to_memory(dst + word, get_from_memory::<u32>(src + word));
        }
        dst += element_size;
        src += element_size;
    }
    save_to_memory(vector_ptr + 4, end - element_size);
}

/// Generic 3-pointer vanilla MSVC `std::vector` layout.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VanillaVector<T> {
    pub begin: *mut T,
    pub end: *mut T,
    pub cap_end: *mut T,
}

impl<T> VanillaVector<T> {
    pub fn rvo_target() -> Self {
        VanillaVector {
            begin: std::ptr::null_mut(),
            end: std::ptr::null_mut(),
            cap_end: std::ptr::null_mut(),
        }
    }

    pub fn as_ptr(&mut self) -> *mut u32 {
        self as *mut Self as *mut u32
    }

    pub fn as_slice(&self) -> &[T] {
        if self.begin.is_null() {
            return &[];
        }
        let len = unsafe { self.end.offset_from(self.begin) } as usize;
        unsafe { std::slice::from_raw_parts(self.begin, len) }
    }
}

impl<T> Default for VanillaVector<T> {
    fn default() -> Self {
        Self::rvo_target()
    }
}

/// Specialized type alias for vanilla `std::vector<float>`, matching `zoostatus.rs`.
pub type VanillaFloatVector = VanillaVector<f32>;

/// The 3-word (`begin`, `end`, `cap_end`) vanilla `std::vector`-shaped out-param holding raw memory addresses,
/// used by `ZTHabitat::getEvents` and `listen`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VanillaEventVector {
    pub begin: u32,
    pub end: u32,
    pub cap_end: u32,
}

impl VanillaEventVector {
    pub fn rvo_target() -> Self {
        VanillaEventVector {
            begin: 0,
            end: 0,
            cap_end: 0,
        }
    }

    pub fn as_ptr(&mut self) -> *mut u32 {
        self as *mut VanillaEventVector as *mut u32
    }
}
