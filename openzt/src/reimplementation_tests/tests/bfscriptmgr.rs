//! Compares real vanilla `BFScriptMgr::parseFunction` against its Rust reimplementation (production file
//! `openzt/src/bfscriptmgr.rs`) on standalone call objects. Both poles write vanilla-allocated strings
//! and vectors through vanilla's own helpers, so nothing here crosses allocators; the objects are leaked.
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
            let port = bfscriptmgr_live_support::standalone_call();
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
