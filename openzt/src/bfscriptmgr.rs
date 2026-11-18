//! `BFScriptMgr::parseFunction` (`0x437d9b`): turns a behavior-script call string such as
//! `fPlaySet("a b", 3)` into a `BFFunctionCall`. Its only caller is `BFBehaviorSet::executeCall`, and the
//! resulting call object is read by raw offset by every `ZTAIMgr`/`BFAIMgr` handler (and passed by pointer to
//! vanilla's `findAnim`/`convertConstantParam`), so the struct keeps vanilla's layout and is written in place;
//! only the buffers behind it are owned by Rust (see "Allocation").
//!
//! ## Layout
//! `BFFunctionCall`, `0x1c` bytes: `+0x00` the call name (a `{begin, end, cap}` string, the same type as
//! `VanillaString`), `+0x0c` the argument `vector<string>` (`{begin, end, cap}`, 12-byte elements), `+0x18`
//! the result flag, `+0x19`/`+0x1a` two further flag bytes. `parseFunction` zeroes all three flag bytes.
//!
//! ## Parsing
//! Name is the text before the first `(`. The text after it must end in `)`. The `)` is dropped, every space and
//! `"` is removed, and the remainder is split on `,`. A piece is always produced for the first segment, so
//! `f()` yields one empty argument, and a trailing comma does not yield a trailing empty argument
//! (`f(a,)` is `["a"]`, `f(a,,b)` is `["a", "", "b"]`).
//!
//! ## Deliberate differences from vanilla
//! Vanilla parses through fixed stack buffers (128 bytes for the name and for the text after `(`, 32 bytes
//! per argument) with no length check, so longer input overflows the stack; the port has no such limit.
//! When there is no `(`, or it is the last character, vanilla builds the name from an uninitialised stack
//! buffer; the port gives an empty name. Both only matter for malformed input. In every case the return
//! value, the argument list and the flag bytes agree with vanilla. Failure is not rolled back in either
//! implementation: the name has been replaced and the arguments cleared by the time it returns `false`.
//!
//! ## Allocation
//! The call name's buffer, the argument array and each argument's buffer are allocated and freed here, from
//! Rust's allocator, and every vanilla reader only reads them (strings stay `{begin, end, cap}` with a NUL
//! after the text, the array stays 12-byte entries) through `owned_strings.rs`, which also frees a buffer that
//! vanilla built (a call object constructed by vanilla's own constructor, as the live tests' "real" side does)
//! through vanilla's allocator, so an object is always freed by whoever allocated each piece of it.

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bffunctioncall::{LOAD as CALL_LOAD, SAVE as CALL_SAVE},
        bfscriptmgr::PARSE_FUNCTION,
        msvc_std_basic_string::{ASSIGN_RANGE, BASIC_STRING_0, BASIC_STRING_2},
        msvc_std_vectorbasic_string::INSERT_0X406DE7,
        standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE},
        uitext::SET_TEXT_0,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    owned_strings::{string_assign, string_construct, string_release, strvec_clear, strvec_free, strvec_push, StringSlot},
    util::{ref_from_memory, LiveMemory},
    write_live,
};

pub const BFFUNCTIONCALL_SIZE: usize = 0x1c;
const ARGS_OFFSET: u32 = 0x0c;

/// A `{begin, end, cap}` string as read from the call string argument.
#[repr(C)]
pub(crate) struct RawString {
    pub(crate) begin: *const u8,
    pub(crate) end: *const u8,
    pub(crate) capacity: *const u8,
}

impl RawString {
    pub(crate) fn bytes(&self) -> &[u8] {
        if self.begin.is_null() || self.end < self.begin {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.begin, self.end as usize - self.begin as usize) }
    }
}

#[repr(C)]
pub(crate) struct BfFunctionCall {
    pub(crate) name_begin: *mut u8,
    pub(crate) name_end: *mut u8,
    pub(crate) name_capacity: *mut u8,
    pub(crate) args_begin: *const u32,
    pub(crate) args_end: *const u32,
    pub(crate) args_capacity: *const u32,
    pub(crate) result_flag: u8,
    pub(crate) flag_19: u8,
    pub(crate) flag_1a: u8,
    pub _live: LiveMemory,
}

const _: () = assert!(std::mem::size_of::<BfFunctionCall>() == BFFUNCTIONCALL_SIZE);

/// The outcome of parsing one call string, independent of where it is stored.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedCall {
    pub name: Vec<u8>,
    pub args: Vec<Vec<u8>>,
    pub ok: bool,
}

/// The pure part of `parseFunction`. `args` is only filled when `ok` is true.
pub(crate) fn parse(input: &[u8]) -> ParsedCall {
    let open = input.iter().position(|&b| b == b'(');
    let name = match open {
        Some(p) if p + 1 < input.len() => input[..p].to_vec(),
        _ => Vec::new(),
    };
    let mut parsed = ParsedCall { name, args: Vec::new(), ok: false };

    let after = match open {
        Some(p) => &input[p + 1..],
        None => input,
    };
    if after.last() != Some(&b')') {
        return parsed;
    }

    let mut rest: Vec<u8> = after[..after.len() - 1].iter().copied().filter(|&b| b != b' ' && b != b'"').collect();
    loop {
        match rest.iter().position(|&b| b == b',') {
            Some(comma) => {
                parsed.args.push(rest[..comma].to_vec());
                rest.drain(..=comma);
                if rest.is_empty() {
                    break;
                }
            }
            None => {
                parsed.args.push(rest);
                break;
            }
        }
    }
    parsed.ok = true;
    parsed
}

/// A freshly constructed call name has room for this many bytes (vanilla builds its strings with capacity 8).
const NAME_INITIAL_CAPACITY: usize = 8;

/// Builds an empty call object at `at` (`BFFunctionCall::BFFunctionCall`): a name with room for
/// [`NAME_INITIAL_CAPACITY`] bytes, no arguments, the flag bytes zero. The padding byte at `+0x1b` is left as it
/// was, as in vanilla.
///
/// # Safety
/// `at` must have `BFFUNCTIONCALL_SIZE` writable bytes.
pub(crate) unsafe fn construct_call(at: u32) {
    unsafe {
        string_construct(at as StringSlot, NAME_INITIAL_CAPACITY);
        for offset in [0x0cu32, 0x10, 0x14] {
            ((at + offset) as *mut u32).write_volatile(0);
        }
        for offset in [0x18u32, 0x19, 0x1a] {
            ((at + offset) as *mut u8).write_volatile(0);
        }
    }
}

/// Frees a call object's name and argument buffers (`BFFunctionCall::~BFFunctionCall`). The object's words are
/// left as they are, as vanilla's destructor leaves them.
///
/// # Safety
/// `at` must point to a constructed `BFFunctionCall`.
pub(crate) unsafe fn destroy_call(at: u32) {
    unsafe {
        strvec_free(at + ARGS_OFFSET);
        string_release(at as StringSlot);
    }
}

/// Replaces the call's name.
///
/// # Safety
/// `at` must point to a constructed `BFFunctionCall`.
pub(crate) unsafe fn set_call_name(at: u32, name: &[u8]) {
    unsafe { string_assign(at as StringSlot, name) };
}

/// Empties the call's argument list, keeping its array.
///
/// # Safety
/// `at` must point to a constructed `BFFunctionCall`.
pub(crate) unsafe fn clear_call_args(at: u32) {
    unsafe { strvec_clear(at + ARGS_OFFSET) };
}

/// Replaces the call's arguments with copies of `arguments`.
///
/// # Safety
/// `at` must point to a constructed `BFFunctionCall`.
pub(crate) unsafe fn set_call_args(at: u32, arguments: &[&[u8]]) {
    unsafe {
        strvec_clear(at + ARGS_OFFSET);
        for argument in arguments {
            strvec_push(at + ARGS_OFFSET, argument);
        }
    }
}

/// Empties a call object's name and argument vector and zeroes its flag bytes. Shared by `parseFunction` and
/// `BFBehaviorSet::clear`.
///
/// # Safety
/// `dest` must point to a constructed `BFFunctionCall`.
pub(crate) unsafe fn reset_call(dest: *const u32) {
    let call = unsafe { ref_from_memory::<BfFunctionCall>(dest as u32) };
    let begin = call.name_begin;
    if !begin.is_null() {
        unsafe { begin.write_volatile(0) };
    }
    write_live!(call, name_end, begin);
    unsafe { strvec_clear(dest as u32 + ARGS_OFFSET) };
    write_live!(call, result_flag, 0u8);
    write_live!(call, flag_19, 0u8);
    write_live!(call, flag_1a, 0u8);
}

/// Save files older than this version carry no behavior-set data; `load` reports success without reading.
pub(crate) const FILE_VERSION_LAST_WITHOUT_BEHAVIOR_SETS: u32 = 0x3e;
/// A saved string longer than this is rejected as corrupt.
const FILE_STRING_LIMIT: u32 = 0x1000;

/// Reads `size` bytes from a save file into `dest` through vanilla's file-read primitive (the hooked
/// address, so the test harness can replay bytes into it). True when the read succeeded.
pub(crate) unsafe fn read_file_bytes(dest: *mut u8, size: u32, file: *const u8) -> bool {
    unsafe { DEALLOCATE.hooked()(dest as *const u32, size, 1, file) == 1 }
}

/// Reads a 4-byte count from a save file.
pub(crate) unsafe fn read_file_count(file: *const u8) -> Option<i32> {
    let mut count = 0i32;
    unsafe { read_file_bytes(std::ptr::addr_of_mut!(count) as *mut u8, 4, file) }.then_some(count)
}

/// Reads one length-prefixed string from a save file and assigns it to the string at `target`. Nothing is
/// assigned when the length cannot be read or is at least `0x1000`. A short read of the bytes still assigns
/// (zeroes) and reports failure, as vanilla's inlined readers do.
pub(crate) unsafe fn read_file_string(file: *const u8, target: *const RawString) -> bool {
    let Some((text, ok)) = (unsafe { read_file_text(file) }) else {
        return false;
    };
    let range = text.as_ptr_range();
    unsafe { ASSIGN_RANGE.original()(target as *const c_void, range.start as *const u32, range.end, std::ptr::null()) };
    ok
}

/// Reads one length-prefixed string from a save file: `None` when the length cannot be read or is at least
/// `0x1000`, else the bytes (zeroes after a short read) and whether the read succeeded.
unsafe fn read_file_text(file: *const u8) -> Option<(Vec<u8>, bool)> {
    let mut length = 0u32;
    if !unsafe { read_file_bytes(std::ptr::addr_of_mut!(length) as *mut u8, 4, file) } || length >= FILE_STRING_LIMIT {
        return None;
    }
    let mut text = vec![0u8; length as usize];
    let ok = length == 0 || unsafe { read_file_bytes(text.as_mut_ptr(), length, file) };
    Some((text, ok))
}

/// Builds an empty vanilla string (a small `PoolAlloc` buffer) at `at`.
pub(crate) unsafe fn construct_empty_string(at: *mut RawString) {
    let empty = [0u8; 1];
    let start = empty.as_ptr();
    unsafe { BASIC_STRING_2.original()(at as *const c_void, start as *const u32, start as i32, 0) };
}

/// Appends a copy of the string at `value` to the `vector<string>` at `vector`: in place when there is room,
/// through vanilla's `insert` when full, as its inlined `push_back` does.
pub(crate) unsafe fn push_back_string(vector: *const u32, value: *const u32) {
    unsafe {
        let end = std::ptr::read_volatile((vector as u32 + 4) as *const u32);
        let capacity = std::ptr::read_volatile((vector as u32 + 8) as *const u32);
        if end == capacity {
            INSERT_0X406DE7.original()(vector, end as *const u32, value, 1);
        } else {
            if end != 0 {
                SET_TEXT_0.original()(end as *const u32, value as i32);
            }
            std::ptr::write_volatile((vector as u32 + 4) as *mut u32, end + 12);
        }
    }
}

/// Reads one length-prefixed string from a save file into the Rust-owned string at `target`
/// ([`read_file_string`] for a vanilla-owned one).
pub(crate) unsafe fn read_file_owned_string(file: *const u8, target: *const RawString) -> bool {
    let Some((text, ok)) = (unsafe { read_file_text(file) }) else {
        return false;
    };
    unsafe { string_assign(target as StringSlot, &text) };
    ok
}

/// Reads `count` strings from a save file into the Rust-owned `vector<string>` at `vector`, which must already be
/// empty. Each string that fails to read is still appended (empty). True when every read succeeded.
pub(crate) unsafe fn read_file_strvec(file: *const u8, vector: u32, count: i32) -> bool {
    let mut ok = true;
    for _ in 0..count.max(0) {
        match unsafe { read_file_text(file) } {
            Some((text, read_ok)) => {
                unsafe { strvec_push(vector, &text) };
                ok &= read_ok;
            }
            None => {
                unsafe { strvec_push(vector, b"") };
                ok = false;
            }
        }
    }
    ok
}

/// Reads `count` strings from a save file into the `vector<string>` at `vector`, which must already be empty.
/// Each string that fails to read is still appended (empty). True when every read succeeded.
pub(crate) unsafe fn read_file_string_list(file: *const u8, vector: *const u32, count: i32) -> bool {
    let mut ok = true;
    for _ in 0..count.max(0) {
        let mut value = [0u32; 3];
        unsafe {
            construct_empty_string(value.as_mut_ptr() as *mut RawString);
            ok &= read_file_string(file, value.as_ptr() as *const RawString);
            push_back_string(vector, value.as_ptr());
            BASIC_STRING_0.original()(value.as_ptr() as *const c_void);
        }
    }
    ok
}

/// Writes `size` bytes to a save file through vanilla's write primitive (the hooked address, so the test
/// harness can capture them). True when the write succeeded.
pub(crate) unsafe fn write_file_bytes(source: *const u8, size: u32, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(source as *const u32, size, 1, file) == 1 }
}

/// Writes one length-prefixed string. A string of `0x1000` bytes or more writes nothing and fails. Both
/// writes are attempted even if the length write failed, as vanilla does.
pub(crate) unsafe fn write_file_string(file: *const i8, string: *const RawString) -> bool {
    let (begin, end) = unsafe { ((*string).begin, (*string).end) };
    let length = (end as u32).wrapping_sub(begin as u32);
    if length >= FILE_STRING_LIMIT {
        return false;
    }
    let mut ok = unsafe { write_file_bytes(std::ptr::addr_of!(length) as *const u8, 4, file) };
    if length != 0 {
        let body = unsafe { write_file_bytes(begin, length, file) };
        ok = ok && body;
    }
    ok
}

/// Writes a `vector<string>` as its `i32` element count followed by each string. True when every write
/// succeeded.
pub(crate) unsafe fn write_file_string_list(file: *const i8, vector: *const u32) -> bool {
    let (begin, end) = unsafe { (std::ptr::read_volatile(vector), std::ptr::read_volatile(vector.add(1))) };
    let count = (end.wrapping_sub(begin) as i32) / 12;
    let mut ok = unsafe { write_file_bytes(std::ptr::addr_of!(count) as *const u8, 4, file) };
    for index in 0..count.max(0) as u32 {
        ok &= unsafe { write_file_string(file, (begin + index * 12) as *const RawString) };
    }
    ok
}

/// `BFFunctionCall::save(file)` (`0x477ec3`): the call name, its arguments and the three flag bytes, in the
/// order `load_call` reads them.
///
/// # Safety
/// `call` must point to a constructed `BFFunctionCall`.
pub(crate) unsafe fn save_call(call: *const u32, file: *const i8) -> bool {
    let mut ok = unsafe { write_file_string(file, call as *const RawString) };
    ok &= unsafe { write_file_string_list(file, (call as u32 + ARGS_OFFSET) as *const u32) };
    for offset in [0x18u32, 0x19, 0x1a] {
        ok &= unsafe { write_file_bytes((call as u32 + offset) as *const u8, 1, file) };
    }
    ok
}

/// `BFFunctionCall::load(file, version)` (`0x44fdd1`): the call name, its arguments and the three flag bytes.
/// Succeeds without reading for saves before behavior sets were written.
///
/// # Safety
/// `call` must point to a constructed `BFFunctionCall`.
pub(crate) unsafe fn load_call(call: *const u32, file: *const u8, version: u32) -> bool {
    if version <= FILE_VERSION_LAST_WITHOUT_BEHAVIOR_SETS {
        return true;
    }
    let mut ok = match unsafe { read_file_text(file) } {
        Some((text, read_ok)) => {
            unsafe { set_call_name(call as u32, &text) };
            read_ok
        }
        None => false,
    };
    let count = unsafe { read_file_count(file) };
    ok &= count.is_some();
    if !ok {
        return false;
    }
    let args = call as u32 + ARGS_OFFSET;
    unsafe {
        strvec_clear(args);
        ok &= read_file_strvec(file, args, count.unwrap_or(0));
        for offset in [0x18u32, 0x19, 0x1a] {
            ok &= read_file_bytes((call as u32 + offset) as *mut u8, 1, file);
        }
    }
    ok
}

/// `BFScriptMgr::parseFunction(callString, call)`, writing `dest` in place.
///
/// # Safety
/// `input` must point to a valid `{begin, end, cap}` string and `dest` to a constructed `BFFunctionCall`.
/// They must not overlap.
pub(crate) unsafe fn parse_function(input: *const u32, dest: *const u32) -> bool {
    let parsed = parse(unsafe { &*(input as *const RawString) }.bytes());
    unsafe {
        reset_call(dest);
        set_call_name(dest as u32, &parsed.name);
        for arg in &parsed.args {
            strvec_push(dest as u32 + ARGS_OFFSET, arg);
        }
    }
    parsed.ok
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(PARSE_FUNCTION)]
    unsafe extern "stdcall" fn parse_function_detour(input: *const i32, dest: *const u32) -> bool {
        unsafe { parse_function(input as *const u32, dest) }
    }

    #[detour(CALL_LOAD)]
    unsafe extern "thiscall" fn call_load_detour(this: *const u32, file: *const i8, version: u32) -> bool {
        unsafe { load_call(this, file as *const u8, version) }
    }

    #[detour(CALL_SAVE)]
    unsafe extern "thiscall" fn call_save_detour(this: *const u32, file: *const i8) -> bool {
        unsafe { save_call(this, file) }
    }

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn parse_function(input: *const i32, dest: *const u32) -> bool {
            unsafe { super::PARSE_FUNCTION_DETOUR.call(input, dest) }
        }
        pub(crate) fn load_call(this: u32, file: *const i8, version: u32) -> bool {
            unsafe { super::CALL_LOAD_DETOUR.call(this as *const u32, file, version) }
        }
        pub(crate) fn save_call(this: u32, file: *const i8) -> bool {
            unsafe { super::CALL_SAVE_DETOUR.call(this as *const u32, file) }
        }
    }
}

/// Installs this module's detours. Called from both `lib.rs`'s boot cascade and
/// `reimplementation_tests::init()`.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise bfscriptmgr detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::bffunctioncall::CONSTRUCTOR as CALL_CONSTRUCTOR;

    use super::*;

    /// A call object built by vanilla's own constructor in a leaked zeroed buffer. Its strings and vector
    /// are vanilla-allocated, so it is never freed (the leak is bounded by the test count).
    pub(crate) fn standalone_call() -> u32 {
        let buf: &'static mut [u32] = Box::leak(vec![0u32; BFFUNCTIONCALL_SIZE / 4].into_boxed_slice());
        let ptr = buf.as_mut_ptr();
        unsafe { CALL_CONSTRUCTOR.original()(ptr) };
        ptr as u32
    }

    /// A call object built by this module's own constructor in a leaked buffer: every buffer behind it is
    /// Rust-allocated.
    pub(crate) fn standalone_rust_call() -> u32 {
        let buf: &'static mut [u32] = Box::leak(vec![0xcdcd_cdcdu32; BFFUNCTIONCALL_SIZE / 4].into_boxed_slice());
        let at = buf.as_mut_ptr() as u32;
        unsafe { construct_call(at) };
        at
    }

    /// How many buffers the call-object allocator currently tracks.
    pub(crate) fn rust_buffer_count() -> usize {
        crate::owned_strings::tracked_count()
    }

    /// Runs vanilla's call-object constructor over `at`, which must have `BFFUNCTIONCALL_SIZE` writable bytes.
    pub(crate) unsafe fn construct_vanilla_call(at: u32) {
        unsafe { CALL_CONSTRUCTOR.original()(at as *const u32) };
    }

    pub(crate) fn real_parse_function(input: *const u32, dest: u32) -> bool {
        detours::test_real::parse_function(input as *const i32, dest as *const u32)
    }

    pub(crate) use super::detours::test_real as real;

    /// Everything `parseFunction` writes: name, arguments and the three flag bytes.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct CallSnapshot {
        pub name: Vec<u8>,
        pub args: Vec<Vec<u8>>,
        pub flags: [u8; 3],
    }

    pub(crate) fn snapshot(call: u32) -> CallSnapshot {
        let call = unsafe { &*(call as *const BfFunctionCall) };
        let name = unsafe { &*(call as *const BfFunctionCall as *const RawString) }.bytes().to_vec();
        let count = (call.args_end as usize - call.args_begin as usize) / std::mem::size_of::<RawString>();
        let args = (0..count).map(|i| unsafe { &*(call.args_begin as *const RawString).add(i) }.bytes().to_vec()).collect();
        CallSnapshot { name, args, flags: [call.result_flag, call.flag_19, call.flag_1a] }
    }

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        detours::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_strings::{is_tracked, strvec_push, STRING_BYTES};

    fn parsed(input: &str) -> (String, Vec<String>, bool) {
        let p = parse(input.as_bytes());
        (
            String::from_utf8(p.name).unwrap(),
            p.args.into_iter().map(|a| String::from_utf8(a).unwrap()).collect(),
            p.ok,
        )
    }

    #[test]
    fn layout_offsets_match_vanilla() {
        assert_eq!(std::mem::offset_of!(BfFunctionCall, args_begin), 0x0c);
        assert_eq!(std::mem::offset_of!(BfFunctionCall, result_flag), 0x18);
        assert_eq!(std::mem::offset_of!(BfFunctionCall, flag_1a), 0x1a);
    }

    #[test]
    fn splits_and_strips_arguments() {
        assert_eq!(parsed("fMoveF(1,2,3)"), ("fMoveF".into(), vec!["1".into(), "2".into(), "3".into()], true));
        assert_eq!(parsed("fPlaySet(\"a b\", 3)"), ("fPlaySet".into(), vec!["ab".into(), "3".into()], true));
    }

    #[test]
    fn empty_and_trailing_pieces() {
        assert_eq!(parsed("fWalk()"), ("fWalk".into(), vec!["".into()], true));
        assert_eq!(parsed("f(a,)"), ("f".into(), vec!["a".into()], true));
        assert_eq!(parsed("f(a,,b)"), ("f".into(), vec!["a".into(), "".into(), "b".into()], true));
        assert_eq!(parsed("f( )"), ("f".into(), vec!["".into()], true));
    }

    #[test]
    fn malformed_input_fails() {
        for input in ["", "abc", "f(", "f(a", "(", "f(a)b"] {
            assert!(!parse(input.as_bytes()).ok, "{input:?}");
        }
        assert!(parse(b"f(a").args.is_empty());
        assert_eq!(parsed("f(a"), ("f".into(), vec![], false));
    }

    #[test]
    fn no_open_paren_but_closing_paren_still_parses() {
        assert_eq!(parsed("a,b)"), ("".into(), vec!["a".into(), "b".into()], true));
    }

    /// The call object's words, read back the way a vanilla reader would.
    fn read_call(at: u32) -> (Vec<u8>, Vec<Vec<u8>>, [u8; 3]) {
        let call = unsafe { &*(at as *const BfFunctionCall) };
        let name = unsafe { &*(at as *const RawString) }.bytes().to_vec();
        let count = (call.args_end as usize - call.args_begin as usize) / STRING_BYTES;
        let args = (0..count).map(|i| unsafe { &*(call.args_begin as *const RawString).add(i) }.bytes().to_vec()).collect();
        (name, args, [call.result_flag, call.flag_19, call.flag_1a])
    }

    /// Every string a call hands to vanilla readers ends in a NUL after its text.
    fn assert_nul_terminated(at: u32) {
        let call = unsafe { &*(at as *const BfFunctionCall) };
        assert_eq!(unsafe { *(*(at as *const *const u8)).add(read_call(at).0.len()) }, 0);
        for i in 0..(call.args_end as usize - call.args_begin as usize) / STRING_BYTES {
            let string = unsafe { &*(call.args_begin as *const RawString).add(i) };
            assert_eq!(unsafe { *string.end }, 0, "argument {i}");
        }
    }

    fn fresh_call() -> (Box<[u32; BFFUNCTIONCALL_SIZE / 4]>, u32) {
        let storage = Box::new([0xcdcd_cdcdu32; BFFUNCTIONCALL_SIZE / 4]);
        let at = storage.as_ptr() as u32;
        unsafe { construct_call(at) };
        (storage, at)
    }

    #[test]
    fn constructed_call_is_empty_with_room_for_a_short_name() {
        let (_storage, at) = fresh_call();
        assert_eq!(read_call(at), (vec![], vec![], [0, 0, 0]));
        let name = unsafe { &*(at as *const RawString) };
        assert_eq!(name.capacity as usize - name.begin as usize, NAME_INITIAL_CAPACITY);
        assert_nul_terminated(at);
        unsafe { destroy_call(at) };
    }

    #[test]
    fn name_is_assigned_in_place_when_it_fits_and_reallocated_when_it_does_not() {
        let (_storage, at) = fresh_call();
        let begin = unsafe { (*(at as *const RawString)).begin };
        unsafe { set_call_name(at, b"fWalk") };
        assert_eq!(unsafe { (*(at as *const RawString)).begin }, begin);
        assert_eq!(read_call(at).0, b"fWalk");
        unsafe { set_call_name(at, b"fPlaySpecialLongName") };
        assert_eq!(read_call(at).0, b"fPlaySpecialLongName");
        assert_nul_terminated(at);
        unsafe { set_call_name(at, b"") };
        assert_eq!(read_call(at).0, b"");
        assert_nul_terminated(at);
        unsafe { destroy_call(at) };
    }

    #[test]
    fn arguments_grow_past_the_first_allocation_and_keep_their_text() {
        let (_storage, at) = fresh_call();
        let texts: Vec<Vec<u8>> = (0..11).map(|i| format!("arg{i}{}", "x".repeat(i)).into_bytes()).collect();
        for text in &texts {
            unsafe { strvec_push(at + ARGS_OFFSET, text) };
        }
        assert_eq!(read_call(at).1, texts);
        assert_nul_terminated(at);
        unsafe { destroy_call(at) };
    }

    #[test]
    fn set_call_args_replaces_and_tolerates_aliasing_free_inputs() {
        let (_storage, at) = fresh_call();
        unsafe { set_call_args(at, &[b"one", b"two", b"three"]) };
        assert_eq!(read_call(at).1, vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]);
        unsafe { set_call_args(at, &[b"only"]) };
        assert_eq!(read_call(at).1, vec![b"only".to_vec()]);
        unsafe { set_call_args(at, &[]) };
        assert!(read_call(at).1.is_empty());
        unsafe { destroy_call(at) };
    }

    #[test]
    fn reset_clears_name_arguments_and_flags_but_keeps_the_buffers() {
        let (_storage, at) = fresh_call();
        unsafe {
            set_call_name(at, b"fMoveF");
            set_call_args(at, &[b"1", b"2"]);
            let call = &mut *(at as *mut BfFunctionCall);
            call.result_flag = 1;
            call.flag_19 = 2;
            call.flag_1a = 3;
        }
        let (name_begin, args_begin) = unsafe { ((*(at as *const RawString)).begin, (*(at as *const BfFunctionCall)).args_begin) };
        unsafe { reset_call(at as *const u32) };
        assert_eq!(read_call(at), (vec![], vec![], [0, 0, 0]));
        assert_eq!(unsafe { (*(at as *const RawString)).begin }, name_begin);
        assert_eq!(unsafe { (*(at as *const BfFunctionCall)).args_begin }, args_begin);
        unsafe { destroy_call(at) };
    }

    #[test]
    fn destroying_a_call_releases_every_buffer_it_allocated() {
        let (_storage, at) = fresh_call();
        unsafe {
            set_call_name(at, b"a name longer than the initial capacity");
            set_call_args(at, &[b"1", b"2", b"3", b"4", b"5", b"6"]);
        }
        let call = unsafe { &*(at as *const BfFunctionCall) };
        let mut owned = vec![unsafe { (*(at as *const RawString)).begin } as usize, call.args_begin as usize];
        owned.extend((0..6).map(|i| unsafe { (*(call.args_begin as *const RawString).add(i)).begin } as usize));
        assert!(owned.iter().all(|pointer| is_tracked(*pointer)));
        unsafe { destroy_call(at) };
        assert!(owned.iter().all(|pointer| !is_tracked(*pointer)));
    }
}
