//! A scope guard for vanilla `{begin, end, capacity}` vectors whose buffer came from the game's `PoolAlloc`.
//!
//! Vanilla vectors are three words (`begin`, `end`, `capacity`) over a buffer from the small-object pool, and every
//! vanilla function that returns or fills one expects the caller to hand the buffer back to the same pool. Doing that
//! by hand repeats the size arithmetic at every exit. [`PoolVec`] adopts such a triple and returns the buffer when it
//! goes out of scope. It never allocates and never touches a buffer that Rust owns: the memory must have come from
//! vanilla's allocator (a vanilla call that filled an out-vector, or a vanilla-owned vector whose old buffer is being
//! replaced), and a vector still owned by a vanilla object must not be wrapped.

use openzt_detour::generated::poolalloc::{DEALLOCATE, DEALLOCATE_N_4};

/// How the buffer goes back to the pool: vanilla frees some vectors through the typed four-byte entry point and the
/// rest through the plain `(pointer, bytes)` one, and the two are kept as vanilla uses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Release {
    /// `PoolAlloc::deallocate_n_4(begin, capacity in elements)`: a vector of four-byte elements.
    FourByteElements,
    /// `PoolAlloc::deallocate(begin, capacity in whole elements * element size)`.
    Bytes { element_size: u32 },
}

/// An adopted vanilla pool vector. Dropping it frees the buffer when `begin` is not null.
#[derive(Debug)]
pub struct PoolVec {
    words: [u32; 3],
    release: Release,
}

impl PoolVec {
    /// A vector of four-byte elements, freed through `deallocate_n_4`.
    pub fn four_byte_elements() -> Self {
        Self { words: [0; 3], release: Release::FourByteElements }
    }

    /// A vector whose buffer is freed through `deallocate(begin, bytes)` with `bytes` rounded down to whole elements.
    pub fn sized(element_size: u32) -> Self {
        Self { words: [0; 3], release: Release::Bytes { element_size } }
    }

    /// Takes over an already filled `{begin, end, capacity}` triple of four-byte elements.
    pub fn adopt_four_byte_elements(words: [u32; 3]) -> Self {
        Self { words, release: Release::FourByteElements }
    }

    /// Takes over an already filled triple whose buffer is freed with `deallocate`.
    pub fn adopt_sized(words: [u32; 3], element_size: u32) -> Self {
        Self { words, release: Release::Bytes { element_size } }
    }

    /// The pointer a vanilla call writes its `{begin, end, capacity}` result to.
    pub fn out_pointer(&mut self) -> *mut [u32; 3] {
        &mut self.words
    }

    pub fn begin(&self) -> u32 {
        self.words[0]
    }

    pub fn end(&self) -> u32 {
        self.words[1]
    }

    pub fn capacity_end(&self) -> u32 {
        self.words[2]
    }

    /// Whether the vector holds no elements (an unallocated vector is empty).
    pub fn is_empty(&self) -> bool {
        self.words[0] == self.words[1]
    }

    /// The raw triple, for code that must hand it back to vanilla.
    pub fn words(&self) -> [u32; 3] {
        self.words
    }
}

impl Drop for PoolVec {
    fn drop(&mut self) {
        let [begin, _, capacity_end] = self.words;
        if begin == 0 {
            return;
        }
        let capacity_bytes = capacity_end.wrapping_sub(begin) as i32;
        match self.release {
            Release::FourByteElements => unsafe { DEALLOCATE_N_4.original()(begin as *const u32, capacity_bytes >> 2) },
            Release::Bytes { element_size } => {
                let bytes = ((capacity_bytes / element_size as i32) as u32).wrapping_mul(element_size);
                if bytes == 0 {
                    // `PoolAlloc::deallocate(_, 0)` indexes the freelist table with `(0 - 1) >> 3`.
                    return;
                }
                unsafe { DEALLOCATE.original()(begin as *const u32, bytes) }
            }
        }
    }
}
