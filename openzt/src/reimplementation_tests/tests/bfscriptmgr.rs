//! Compares real vanilla `BFScriptMgr::parseFunction` against its Rust reimplementation (production file
//! `openzt/src/bfscriptmgr.rs`) on standalone call objects. The "real" pole's object is built by vanilla's
//! constructor and every buffer behind it is vanilla-allocated; the "port" pole's is built by the port's own
//! constructor and its buffers are Rust-allocated, so each pole only ever frees its own. The objects are leaked.
//! The "real" pole runs the pre-detour body via `bfscriptmgr_live_support::real_parse_function`, the "port"
//! pole runs the address through `.hooked()`.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::bfscriptmgr::PARSE_FUNCTION;

use crate::bfscriptmgr::live_support as bfscriptmgr_live_support;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::vanilla_string::VanillaString;

/// Inputs whose name vanilla builds from the text before `(`.
const WELL_FORMED: &[&str] = &[
    "fWalk()",
    "fWalk(1)",
    "fMoveF(1,2,3)",
    "fPlaySet(\"a b\", 3)",
    "name(arg1, arg2 , \"arg 3\")",
    "f(a,)",
    "f(a,,b)",
    "f(,a)",
    "f( )",
    "f(\"\")",
    "()",
    "f(a(b),c)",
    "fSpray(cWalkRandomlyTime1, 12, \"x\")",
];

/// Inputs vanilla rejects. Its name is built from uninitialised stack when there is no `(` before the end,
/// so only the return value, the arguments and the flag bytes are compared.
const MALFORMED: &[&str] = &["", "abc", "f(", "f(a", "(", "f(a)b", "f(a,b"];

fn parse_both(input: &str, real: u32, port: u32) -> (bool, bool) {
    let source = VanillaString::new(input);
    let real_ret = bfscriptmgr_live_support::real_parse_function(source.as_ptr(), real);
    let port_ret = unsafe { PARSE_FUNCTION.hooked()(source.as_ptr() as *const i32, port as *const u32) };
    (real_ret, port_ret)
}

/// `BFSCRIPTMGR_DETOURS_ENABLED` - wiring check.
pub(crate) fn run_bfscriptmgr_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFSCRIPTMGR_DETOURS_ENABLED";
    let disabled: Vec<&'static str> =
        bfscriptmgr_live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("detours not enabled: {disabled:?}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// `BFSCRIPTMGR_PARSE_FUNCTION` - every corpus string is parsed into a fresh call object and into one that
/// already holds a previous parse (so clearing the old name, arguments and flags is covered), by real and
/// port. Return value, argument strings and flag bytes must match; the name must match whenever vanilla
/// defines it.
pub(crate) fn run_bfscriptmgr_parse_function_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFSCRIPTMGR_PARSE_FUNCTION";
    let mut failures: Vec<String> = Vec::new();

    for (input, name_defined) in WELL_FORMED.iter().map(|s| (*s, true)).chain(MALFORMED.iter().map(|s| (*s, false))) {
        for dirty in [false, true] {
            let real = bfscriptmgr_live_support::standalone_call();
            let port = bfscriptmgr_live_support::standalone_rust_call();
            if dirty {
                let previous = parse_both("fOld(x, y, z)", real, port);
                if previous != (true, true) {
                    failures.push(format!("{input:?}: priming parse returned {previous:?}"));
                }
            }

            let (real_ret, port_ret) = parse_both(input, real, port);
            let label = format!("{input:?} (dirty={dirty})");
            if real_ret != port_ret {
                failures.push(format!("{label}: return real={real_ret} port={port_ret}"));
            }
            let (real_snap, port_snap) = (bfscriptmgr_live_support::snapshot(real), bfscriptmgr_live_support::snapshot(port));
            if real_snap.args != port_snap.args {
                failures.push(format!("{label}: args real={:?} port={:?}", real_snap.args, port_snap.args));
            }
            if real_snap.flags != port_snap.flags {
                failures.push(format!("{label}: flags real={:?} port={:?}", real_snap.flags, port_snap.flags));
            }
            if name_defined && real_snap.name != port_snap.name {
                failures.push(format!(
                    "{label}: name real={:?} port={:?}",
                    String::from_utf8_lossy(&real_snap.name),
                    String::from_utf8_lossy(&port_snap.name)
                ));
            }
        }
    }

    finish_test(test_name, failures, failure_log)
}

/// `BFSCRIPTMGR_RUST_OWNED_CALL` - a whole call-object lifecycle on the port's own storage: construct, parse every
/// corpus string repeatedly (growing the name and the argument array), replace the name and arguments, reset
/// and destroy. No size class of vanilla's `PoolAlloc` may gain or lose a block (the call object never touches
/// vanilla's allocator) and the allocator's own bookkeeping must end where it started.
pub(crate) fn run_bfscriptmgr_rust_owned_call_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::bfscriptmgr::{construct_call, destroy_call, reset_call, set_call_args, set_call_name};
    use crate::reimplementation_tests::tests::bfentity::{pool_lengths, warm_pool};

    let test_name = "BFSCRIPTMGR_RUST_OWNED_CALL";
    let mut failures: Vec<String> = Vec::new();

    let mut storage = Box::new([0xcdcd_cdcdu32; crate::bfscriptmgr::BFFUNCTIONCALL_SIZE / 4]);
    let call = storage.as_mut_ptr() as u32;
    let sources: Vec<VanillaString> = WELL_FORMED.iter().chain(MALFORMED).map(|text| VanillaString::new(text)).collect();

    warm_pool();
    let (pool_before, tracked_before) = (pool_lengths(), bfscriptmgr_live_support::rust_buffer_count());
    unsafe { construct_call(call) };
    for _ in 0..3 {
        for source in &sources {
            unsafe { PARSE_FUNCTION.hooked()(source.as_ptr() as *const i32, call as *const u32) };
        }
    }
    let after_parses = bfscriptmgr_live_support::snapshot(call);
    const LONG_NAME: &[u8] = b"a name that is much longer than the initial capacity of a call name";
    unsafe {
        set_call_name(call, LONG_NAME);
        set_call_args(call, &[b"1", b"2", b"3", b"4", b"5", b"6", b"7", b"8", b"9"]);
    }
    let replaced = bfscriptmgr_live_support::snapshot(call);
    unsafe { reset_call(call as *const u32) };
    let reset = bfscriptmgr_live_support::snapshot(call);
    unsafe { destroy_call(call) };
    let (pool_after, tracked_after) = (pool_lengths(), bfscriptmgr_live_support::rust_buffer_count());

    if pool_before != pool_after {
        failures.push(format!("vanilla pool changed: before {pool_before:?}, after {pool_after:?}"));
    }
    if tracked_before != tracked_after {
        failures.push(format!("call allocator tracked {tracked_before} buffers before and {tracked_after} after"));
    }
    // The corpus ends on "f(a,b", which fails after clearing the arguments, so only the name `f` remains.
    if after_parses.name != b"f" || !after_parses.args.is_empty() {
        failures.push(format!("unexpected state after the parses: {after_parses:?}"));
    }
    if replaced.args.len() != 9 || replaced.name != LONG_NAME {
        failures.push(format!("replacement not applied: {replaced:?}"));
    }
    if !reset.name.is_empty() || !reset.args.is_empty() {
        failures.push(format!("reset left data behind: {reset:?}"));
    }
    finish_test(test_name, failures, failure_log)
}
