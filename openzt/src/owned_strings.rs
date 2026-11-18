//! Rust-owned buffers behind vanilla-layout strings and `vector<string>`s.
//!
//! A vanilla `std::string` here is `{begin, end, cap}` with a NUL after the text, and a `vector<string>` is
//! `{begin, end, cap}` over 12-byte string entries. Un-ported vanilla code still reads some of these
//! (`findAnim`, `convertConstantParam`, `BFUnit::checkDone`/`drawAIInfo`/`runBehaviorSet`, three goal routines),
//! and only reads them, so the layout stays exactly vanilla's while the buffers behind it come from Rust's
//! allocator.
//!
//! Every buffer allocated here is recorded in `RUST_BUFFERS`. Releasing a buffer that is not recorded means vanilla
//! built it (an object made by vanilla's own constructor, as the live tests' "real" side does, or one that existed
//! before the detours were installed), so it is freed the way `PoolAlloc::deallocate` does it: into the size-class
//! freelist, or `operator delete` above `0x80` bytes. An object is therefore always freed by whoever allocated each
//! piece of it, and mixed objects are safe.
//!
//! The vanilla string and vector helpers (`generated::msvc_std_basic_string`, `msvc_std_vectorbasic_string`, and
//! `bfscriptmgr::{construct_empty_string, push_back_string, read_file_string, ...}`) stay available for ported
//! code that still has to build vanilla-owned strings.

use std::{
    alloc::{alloc, dealloc, handle_alloc_error, Layout},
    collections::HashSet,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};
#[cfg(feature = "reimplementation-tests")]
use std::sync::atomic::{AtomicUsize, Ordering};

use openzt_detour::generated::{poolalloc::DEALLOCATE as POOL_DEALLOCATE, standalone::OPERATOR_DELETE};

/// Addresses of the buffers this module allocated.
static RUST_BUFFERS: LazyLock<Mutex<HashSet<usize>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Releases that took the vanilla branch of [`free_buffer`] (a buffer this module did not allocate).
#[cfg(feature = "reimplementation-tests")]
static VANILLA_FREES: AtomicUsize = AtomicUsize::new(0);

/// Size of one string entry (`{begin, end, cap}`).
pub(crate) const STRING_BYTES: usize = 12;
/// The smallest argument array a push allocates, in strings.
const VECTOR_MIN_CAPACITY: usize = 4;
/// Largest block `PoolAlloc` serves from its size-class freelists; bigger ones go to `operator new`.
const POOL_LIMIT: usize = 0x80;

/// A `{begin, end, cap}` string slot.
pub(crate) type StringSlot = *mut [*mut u8; 3];

/// Locks the tracker. A panic while the lock is held must not poison it into a second panic across an
/// `extern "thiscall"` boundary, and the set stays consistent after any single insert/remove, so poison is ignored.
fn tracked_buffers() -> MutexGuard<'static, HashSet<usize>> {
    RUST_BUFFERS.lock().unwrap_or_else(PoisonError::into_inner)
}

fn buffer_layout(size: usize, align: usize) -> Layout {
    Layout::from_size_align(size.max(1), align).expect("string buffer layout")
}

fn rust_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = buffer_layout(size, align);
    let pointer = unsafe { alloc(layout) };
    if pointer.is_null() {
        handle_alloc_error(layout);
    }
    tracked_buffers().insert(pointer as usize);
    pointer
}

/// Frees `pointer` and returns true when this module allocated it; false (and nothing freed) otherwise.
unsafe fn rust_free(pointer: *mut u8, size: usize, align: usize) -> bool {
    if !tracked_buffers().remove(&(pointer as usize)) {
        return false;
    }
    unsafe { dealloc(pointer, buffer_layout(size, align)) };
    true
}

/// Releases a buffer of `size` bytes: through Rust's allocator when this module allocated it, otherwise the
/// way vanilla's `PoolAlloc::deallocate` does.
pub(crate) unsafe fn free_buffer(pointer: *mut u8, size: usize, align: usize) {
    if pointer.is_null() || unsafe { rust_free(pointer, size, align) } {
        return;
    }
    #[cfg(feature = "reimplementation-tests")]
    VANILLA_FREES.fetch_add(1, Ordering::Relaxed);
    unsafe {
        if size > POOL_LIMIT {
            OPERATOR_DELETE.original()(pointer as u32);
        } else {
            POOL_DEALLOCATE.original()(pointer as *const u32, size as u32);
        }
    }
}

/// How many buffers are currently tracked.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) fn tracked_count() -> usize {
    tracked_buffers().len()
}

/// How many releases have taken the vanilla branch of [`free_buffer`] so far. Compare deltas: objects built by
/// vanilla's own constructors (the live tests' "real" side, anything that predates the detours) legitimately
/// release through it.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn vanilla_free_count() -> usize {
    VANILLA_FREES.load(Ordering::Relaxed)
}

/// Whether `pointer` is a buffer this module allocated and has not yet freed.
#[cfg(test)]
pub(crate) fn is_tracked(pointer: usize) -> bool {
    tracked_buffers().contains(&pointer)
}

/// The string's text.
pub(crate) unsafe fn string_bytes<'a>(slot: *const [*mut u8; 3]) -> &'a [u8] {
    let [begin, end, _] = unsafe { std::ptr::read_volatile(slot) };
    if begin.is_null() || end < begin {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(begin, end as usize - begin as usize) }
}

/// Points `slot` at a new buffer holding `bytes` and a NUL, sized exactly (at least `min_capacity`), without
/// touching whatever the slot held.
pub(crate) unsafe fn string_fill_new(slot: StringSlot, bytes: &[u8], min_capacity: usize) {
    let capacity = (bytes.len() + 1).max(min_capacity);
    let buffer = rust_alloc(capacity, 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
        buffer.add(bytes.len()).write(0);
        std::ptr::write_volatile(slot, [buffer, buffer.add(bytes.len()), buffer.add(capacity)]);
    }
}

/// Builds an empty string at `slot` with room for `capacity` bytes (vanilla builds its strings with capacity 8).
pub(crate) unsafe fn string_construct(slot: StringSlot, capacity: usize) {
    unsafe { string_fill_new(slot, b"", capacity) };
}

/// Frees the buffer behind `slot` (the slot's words are left as they are).
pub(crate) unsafe fn string_release(slot: StringSlot) {
    let [begin, _, capacity] = unsafe { std::ptr::read_volatile(slot) };
    unsafe { free_buffer(begin, capacity as usize - begin as usize, 1) };
}

/// Assigns `bytes` to the string in `slot`: in place when its buffer has room for the text and a NUL, else into a
/// new buffer with the old one released. `bytes` must not point into the slot's own buffer unless it is the whole
/// text.
pub(crate) unsafe fn string_assign(slot: StringSlot, bytes: &[u8]) {
    let [begin, _, capacity] = unsafe { std::ptr::read_volatile(slot) };
    if !begin.is_null() && capacity as usize - begin as usize > bytes.len() {
        unsafe {
            std::ptr::copy(bytes.as_ptr(), begin, bytes.len());
            begin.add(bytes.len()).write(0);
            std::ptr::write_volatile(slot, [begin, begin.add(bytes.len()), capacity]);
        }
        return;
    }
    unsafe {
        string_release(slot);
        string_fill_new(slot, bytes, 1);
    }
}

/// A vanilla-layout string (`{begin, end, cap}`, NUL-terminated) whose buffer comes from Rust's allocator, for
/// temporaries handed to code that only reads them. The slot's address is what callers pass, so it must not move
/// after [`OwnedString::as_ptr`] is taken. Never hand it to code that assigns to, grows or frees the string
/// through vanilla's allocator.
#[repr(C)]
pub(crate) struct OwnedString {
    slot: [*mut u8; 3],
}

impl OwnedString {
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        let mut slot: [*mut u8; 3] = [std::ptr::null_mut(); 3];
        unsafe { string_fill_new(&mut slot, bytes, 1) };
        OwnedString { slot }
    }

    pub(crate) fn new(text: &str) -> Self {
        Self::from_bytes(text.as_bytes())
    }

    /// The address of the `{begin, end, cap}` words.
    pub(crate) fn as_ptr(&self) -> *const u32 {
        self.slot.as_ptr() as *const u32
    }
}

impl Drop for OwnedString {
    fn drop(&mut self) {
        unsafe { string_release(&mut self.slot) };
    }
}

fn read_words(vector: u32) -> [usize; 3] {
    unsafe { [std::ptr::read_volatile(vector as *const usize), std::ptr::read_volatile((vector + 4) as *const usize), std::ptr::read_volatile((vector + 8) as *const usize)] }
}

fn write_words(vector: u32, words: [usize; 3]) {
    unsafe {
        std::ptr::write_volatile(vector as *mut usize, words[0]);
        std::ptr::write_volatile((vector + 4) as *mut usize, words[1]);
        std::ptr::write_volatile((vector + 8) as *mut usize, words[2]);
    }
}

/// How many strings the vector at `vector` holds.
pub(crate) fn strvec_len(vector: u32) -> usize {
    let [begin, end, _] = read_words(vector);
    end.wrapping_sub(begin) / STRING_BYTES
}

/// The text of every string in the vector at `vector`.
pub(crate) unsafe fn strvec_texts(vector: u32) -> Vec<Vec<u8>> {
    let [begin, _, _] = read_words(vector);
    (0..strvec_len(vector)).map(|i| unsafe { string_bytes((begin + i * STRING_BYTES) as *const [*mut u8; 3]) }.to_vec()).collect()
}

/// Releases every string and empties the vector, keeping its array.
pub(crate) unsafe fn strvec_clear(vector: u32) {
    let [begin, end, capacity] = read_words(vector);
    let mut element = begin;
    while element < end {
        unsafe { string_release(element as StringSlot) };
        element += STRING_BYTES;
    }
    write_words(vector, [begin, begin, capacity]);
}

/// Appends a string holding `bytes`, growing the array (into Rust's allocator) when it is full.
pub(crate) unsafe fn strvec_push(vector: u32, bytes: &[u8]) {
    let [begin, end, capacity] = read_words(vector);
    let (count, room) = (end.wrapping_sub(begin) / STRING_BYTES, capacity.wrapping_sub(begin) / STRING_BYTES);
    let (begin, capacity) = if count == room {
        let new_room = (room * 2).max(VECTOR_MIN_CAPACITY);
        let fresh = rust_alloc(new_room * STRING_BYTES, 4) as usize;
        unsafe {
            if count != 0 {
                std::ptr::copy_nonoverlapping(begin as *const u8, fresh as *mut u8, count * STRING_BYTES);
            }
            free_buffer(begin as *mut u8, room * STRING_BYTES, 4);
        }
        (fresh, fresh + new_room * STRING_BYTES)
    } else {
        (begin, capacity)
    };
    unsafe { string_fill_new((begin + count * STRING_BYTES) as StringSlot, bytes, 1) };
    write_words(vector, [begin, begin + (count + 1) * STRING_BYTES, capacity]);
}

/// Releases and removes the last string, if any.
pub(crate) unsafe fn strvec_pop(vector: u32) {
    let [begin, end, capacity] = read_words(vector);
    if end > begin {
        let last = end - STRING_BYTES;
        unsafe { string_release(last as StringSlot) };
        write_words(vector, [begin, last, capacity]);
    }
}

/// Releases and removes the first string, moving the rest down one place, if there is one.
pub(crate) unsafe fn strvec_remove_first(vector: u32) {
    let [begin, end, capacity] = read_words(vector);
    if end > begin {
        unsafe {
            string_release(begin as StringSlot);
            std::ptr::copy((begin + STRING_BYTES) as *const u8, begin as *mut u8, end - begin - STRING_BYTES);
        }
        write_words(vector, [begin, end - STRING_BYTES, capacity]);
    }
}

/// Releases every string and the array itself (`~vector<string>`). The vector's words are left as they are, as
/// vanilla's destructor leaves them.
pub(crate) unsafe fn strvec_free(vector: u32) {
    unsafe { strvec_clear(vector) };
    let [begin, _, capacity] = read_words(vector);
    unsafe { free_buffer(begin as *mut u8, capacity - begin, 4) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector() -> Box<[u32; 3]> {
        Box::new([0; 3])
    }

    fn address(vector: &Box<[u32; 3]>) -> u32 {
        vector.as_ptr() as u32
    }

    fn texts(items: &[&str]) -> Vec<Vec<u8>> {
        items.iter().map(|item| item.as_bytes().to_vec()).collect()
    }

    #[test]
    fn push_grows_past_the_first_allocation_and_keeps_every_text() {
        let v = vector();
        let expected: Vec<String> = (0..13).map(|i| format!("entry{i}{}", "x".repeat(i))).collect();
        for item in &expected {
            unsafe { strvec_push(address(&v), item.as_bytes()) };
        }
        assert_eq!(unsafe { strvec_texts(address(&v)) }, expected.iter().map(|e| e.clone().into_bytes()).collect::<Vec<_>>());
        for i in 0..expected.len() {
            let string = unsafe { &*((v[0] as usize + i * STRING_BYTES) as *const [*mut u8; 3]) };
            assert_eq!(unsafe { *string[1] }, 0, "entry {i} is not NUL-terminated");
        }
        unsafe { strvec_free(address(&v)) };
    }

    #[test]
    fn pop_and_remove_first_release_and_shift() {
        let v = vector();
        for item in ["a", "bb", "ccc", "dddd"] {
            unsafe { strvec_push(address(&v), item.as_bytes()) };
        }
        unsafe { strvec_remove_first(address(&v)) };
        assert_eq!(unsafe { strvec_texts(address(&v)) }, texts(&["bb", "ccc", "dddd"]));
        unsafe { strvec_pop(address(&v)) };
        assert_eq!(unsafe { strvec_texts(address(&v)) }, texts(&["bb", "ccc"]));
        unsafe { strvec_remove_first(address(&v)) };
        unsafe { strvec_pop(address(&v)) };
        assert_eq!(strvec_len(address(&v)), 0);
        unsafe {
            strvec_pop(address(&v));
            strvec_remove_first(address(&v));
        }
        assert_eq!(strvec_len(address(&v)), 0);
        unsafe { strvec_free(address(&v)) };
    }

    #[test]
    fn clear_keeps_the_array_and_free_releases_everything() {
        let v = vector();
        for item in ["one", "two", "three", "four", "five"] {
            unsafe { strvec_push(address(&v), item.as_bytes()) };
        }
        let array = v[0] as usize;
        let mut owned: Vec<usize> = vec![array];
        owned.extend((0..5).map(|i| unsafe { (*((array + i * STRING_BYTES) as *const [usize; 3]))[0] }));
        assert!(owned.iter().all(|pointer| is_tracked(*pointer)));
        unsafe { strvec_clear(address(&v)) };
        assert_eq!(v[0] as usize, array, "clear keeps the array");
        assert!(is_tracked(array));
        assert!(owned[1..].iter().all(|pointer| !is_tracked(*pointer)));
        unsafe { strvec_free(address(&v)) };
        assert!(!is_tracked(array));
    }

    #[test]
    fn owned_string_is_nul_terminated_tracked_and_released_on_drop() {
        let string = OwnedString::from_bytes(b"dustball");
        let [begin, end, capacity] = unsafe { *(string.as_ptr() as *const [usize; 3]) };
        assert!(is_tracked(begin));
        assert_eq!(end - begin, 8);
        assert!(capacity > end);
        assert_eq!(unsafe { *(end as *const u8) }, 0);
        assert_eq!(unsafe { string_bytes(string.as_ptr() as *const [*mut u8; 3]) }, b"dustball");
        drop(string);
        assert!(!is_tracked(begin));
        let empty = OwnedString::new("");
        assert_eq!(unsafe { string_bytes(empty.as_ptr() as *const [*mut u8; 3]) }, b"");
    }

    #[test]
    fn string_assign_is_in_place_when_it_fits() {
        let mut slot: [*mut u8; 3] = [std::ptr::null_mut(); 3];
        unsafe {
            string_construct(&mut slot, 8);
            let begin = slot[0];
            string_assign(&mut slot, b"short");
            assert_eq!(slot[0], begin);
            assert_eq!(string_bytes(&slot), b"short");
            string_assign(&mut slot, b"a considerably longer replacement");
            assert_eq!(string_bytes(&slot), b"a considerably longer replacement");
            assert_eq!(*slot[1], 0);
            string_assign(&mut slot, b"");
            assert_eq!(string_bytes(&slot), b"");
            string_release(&mut slot);
        }
    }
}
