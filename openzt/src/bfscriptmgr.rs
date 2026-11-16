//! `BFScriptMgr::parseFunction` (`0x437d9b`): turns a behavior-script call string such as
//! `fPlaySet("a b", 3)` into a `BFFunctionCall`. Its only caller is `BFBehaviorSet::executeCall`, and the
//! resulting call object is read by raw offset by every `ZTAIMgr`/`BFAIMgr` handler, so the struct mirrors
//! vanilla's layout and is written in place (style 1).
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
//! Strings and the argument vector are built and torn down only through vanilla's own helpers
//! (`0x406c55` assign, `0x410ac3` erase, `0x406de7` insert, `BASIC_STRING_2` construct), so every buffer
//! stays vanilla-allocated and vanilla's destructors for the call object remain valid.

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bfscriptmgr::PARSE_FUNCTION, msvc_std_basic_string::ASSIGN_RANGE_0X406C55, msvc_std_vectorbasic_string::{ERASE_RANGE_0X410AC3, INSERT_0X406DE7},
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    util::{ref_from_memory, LiveMemory},
    vanilla_string::VanillaString,
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

/// Empties a call object's name and argument vector and zeroes its flag bytes, through vanilla's own erase
/// helper so the argument strings are freed by their allocator. Shared by `parseFunction` and
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
    unsafe { ERASE_RANGE_0X410AC3.original()((dest as u32 + ARGS_OFFSET) as *const u32, call.args_begin, call.args_end as i32) };
    write_live!(call, result_flag, 0u8);
    write_live!(call, flag_19, 0u8);
    write_live!(call, flag_1a, 0u8);
}

/// `BFScriptMgr::parseFunction(callString, call)`, writing `dest` in place.
///
/// # Safety
/// `input` must point to a valid `{begin, end, cap}` string and `dest` to a constructed `BFFunctionCall`.
/// They must not overlap.
pub(crate) unsafe fn parse_function(input: *const u32, dest: *const u32) -> bool {
    let parsed = parse(unsafe { &*(input as *const RawString) }.bytes());
    let call = unsafe { ref_from_memory::<BfFunctionCall>(dest as u32) };
    unsafe { reset_call(dest) };
    let name = parsed.name.as_ptr_range();
    unsafe { ASSIGN_RANGE_0X406C55.original()(dest as *const c_void, name.start as *const u32, name.end as *const u32, 0) };

    for arg in &parsed.args {
        let value = VanillaString::from_bytes(arg);
        let end = unsafe { std::ptr::read_volatile(&call.args_end) };
        unsafe { INSERT_0X406DE7.original()((dest as u32 + ARGS_OFFSET) as *const u32, end, value.as_ptr(), 1) };
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

    /// Real vanilla body for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn parse_function(input: *const i32, dest: *const u32) -> bool {
            unsafe { super::PARSE_FUNCTION_DETOUR.call(input, dest) }
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
    use openzt_detour::generated::bfconfigfile::UNRESOLVED_CTOR_0X410A7A;

    use super::*;

    /// A call object built by vanilla's own constructor in a leaked zeroed buffer. Its strings and vector
    /// are vanilla-allocated, so it is never freed (the leak is bounded by the test count).
    pub(crate) fn standalone_call() -> u32 {
        let buf: &'static mut [u32] = Box::leak(vec![0u32; BFFUNCTIONCALL_SIZE / 4].into_boxed_slice());
        let ptr = buf.as_mut_ptr();
        unsafe { UNRESOLVED_CTOR_0X410A7A.original()(ptr) };
        ptr as u32
    }

    /// Runs vanilla's call-object constructor over `at`, which must have `BFFUNCTIONCALL_SIZE` writable bytes.
    pub(crate) unsafe fn construct_call(at: u32) {
        unsafe { UNRESOLVED_CTOR_0X410A7A.original()(at as *const u32) };
    }

    pub(crate) fn real_parse_function(input: *const u32, dest: u32) -> bool {
        detours::test_real::parse_function(input as *const i32, dest as *const u32)
    }

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
}
