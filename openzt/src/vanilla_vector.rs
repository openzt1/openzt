//! Vanilla MSVC `std::vector` layout compatibility.
//!
//! Models the 3-word (`begin`, `end`, `cap_end`) layout used by MSVC's `std::vector`
//! across FFI and RVO boundaries.

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
