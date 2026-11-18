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

/// `parseFunction`'s stack frame (offsets from `ESP` after its four register pushes): a 32-byte argument buffer
/// at `+0x2c`, a 128-byte section buffer at `+0x4c`, a 128-byte name buffer at `+0xcc`, then the return address
/// at `+0x14c`. The name is assigned to the call before the section is copied, so the name buffer is dead by
/// then and a section longer than 127 bytes only spills into it.
///
/// Largest name that stays in its buffer (one more byte reaches the return address).
const REAL_NAME_LIMIT: usize = 127;
/// Largest section (the characters after `(`, `)` included) vanilla copies without reaching the return address:
/// the section buffer plus the dead name buffer, with margin.
const REAL_SECTION_LIMIT: usize = 250;
/// The section buffer's own size; a section past this relies on the dead name buffer.
const REAL_SECTION_BUFFER: usize = 127;
/// Largest argument that stays in its 32-byte buffer. A longer one is written over the section buffer, whose
/// terminating NUL it moves, so vanilla's later comma search fails and every following argument is dropped.
const REAL_ARG_LIMIT: usize = 31;

/// Every distinct call string (the value of a key) in a `\BehaviorSet\` section of an `.ai` file in the
/// install directory's `.ztd` archives, read at test time and never stored. Returns `(archives, ai files,
/// strings)`.
fn collect_behavior_call_strings() -> (usize, usize, std::collections::BTreeSet<Vec<u8>>) {
    use std::io::Read;

    fn archives(dir: &std::path::Path, depth: u32, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < 3 {
                    archives(&path, depth + 1, out);
                }
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ztd")) {
                out.push(path);
            }
        }
    }

    let mut calls = std::collections::BTreeSet::new();
    let (mut archive_count, mut file_count) = (0, 0);
    let Some(root) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.to_path_buf())) else {
        return (0, 0, calls);
    };
    let mut paths = Vec::new();
    archives(&root, 0, &mut paths);
    for path in paths {
        let Ok(file) = std::fs::File::open(&path) else { continue };
        let Ok(mut archive) = zip::ZipArchive::new(file) else { continue };
        archive_count += 1;
        for index in 0..archive.len() {
            let Ok(mut entry) = archive.by_index(index) else { continue };
            if !entry.name().is_ok_and(|name| name.to_ascii_lowercase().ends_with(".ai")) {
                continue;
            }
            let mut bytes = Vec::new();
            if entry.read_to_end(&mut bytes).is_err() {
                continue;
            }
            file_count += 1;
            let mut in_behavior_set = false;
            for line in bytes.split(|&b| b == b'\n') {
                let line = line.trim_ascii();
                if line.first() == Some(&b'[') {
                    in_behavior_set = String::from_utf8_lossy(line).to_ascii_lowercase().contains(r"\behaviorset\");
                    continue;
                }
                if !in_behavior_set || line.is_empty() || line[0] == b';' {
                    continue;
                }
                let Some(equals) = line.iter().position(|&b| b == b'=') else { continue };
                let mut value = &line[equals + 1..];
                if let Some(comment) = value.iter().position(|&b| b == b';') {
                    value = &value[..comment];
                }
                let value = value.trim_ascii();
                if !value.is_empty() {
                    calls.insert(value.to_vec());
                }
            }
        }
    }
    (archive_count, file_count, calls)
}

/// `BFSCRIPTMGR_PARSE_FUNCTION_REAL_DATA` - every behavior-script call string in the install's `.ai` data, parsed
/// by real and port into a fresh and a previously-parsed call object, must agree (return, arguments, flags, and
/// the name when the string is well formed). A string the real parser cannot take safely (a name over 127 bytes
/// reaches its return address, a section over about 250 bytes does too, and an argument over 31 makes it drop
/// the arguments after it) is not run through it; the port's parse of it is checked to round-trip and it is
/// reported, as is any string that is not `name(...)`. A section over 127 bytes only spills into the dead name
/// buffer, so those are compared and counted.
pub(crate) fn run_bfscriptmgr_parse_function_real_data_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFSCRIPTMGR_PARSE_FUNCTION_REAL_DATA";
    let mut failures: Vec<String> = Vec::new();
    let (archives, files, calls) = collect_behavior_call_strings();
    if calls.is_empty() {
        write_success_line(failure_log, &format!("{test_name} (skipped: no behavior scripts found, {archives} archives, {files} .ai files)"));
        return false;
    }

    let (mut over_limit, mut not_a_call, mut spills): (Vec<String>, Vec<String>, Vec<String>) = (Vec::new(), Vec::new(), Vec::new());
    let mut compared = 0usize;
    for call in &calls {
        let text = String::from_utf8_lossy(call).into_owned();
        let parsed = crate::bfscriptmgr::parse(call);
        let open = call.iter().position(|&b| b == b'(');
        let (name_len, section_len) = (open.unwrap_or(0), open.map_or(0, |p| call.len() - p - 1));
        let long_arg = parsed.args.iter().any(|a| a.len() > REAL_ARG_LIMIT);
        if name_len > REAL_NAME_LIMIT || section_len > REAL_SECTION_LIMIT || long_arg {
            // The real parser would reach its return address (or drop arguments after the long one); the port has
            // no limits and must still split the text into the same name and arguments a longer buffer would give.
            let expected: Vec<u8> = call.iter().copied().filter(|&b| b != b' ' && b != b'"').collect();
            let mut rebuilt = parsed.name.clone();
            rebuilt.push(b'(');
            rebuilt.extend(parsed.args.join(&b','));
            rebuilt.push(b')');
            if !parsed.ok || rebuilt != expected {
                failures.push(format!("{text:?}: the port's parse of an over-limit string does not round-trip"));
            }
            over_limit.push(text);
            continue;
        }
        if section_len > REAL_SECTION_BUFFER {
            spills.push(text.clone());
        }
        let well_formed = open.is_some_and(|p| p + 1 < call.len()) && parsed.ok;
        if !well_formed {
            not_a_call.push(text.clone());
        }
        for dirty in [false, true] {
            let real = bfscriptmgr_live_support::standalone_call();
            let port = bfscriptmgr_live_support::standalone_rust_call();
            if dirty {
                let previous = parse_both("fOld(x, y, z)", real, port);
                if previous != (true, true) {
                    failures.push(format!("{text:?}: priming parse returned {previous:?}"));
                }
            }
            let (real_ret, port_ret) = parse_both(&text, real, port);
            let label = format!("{text:?} (dirty={dirty})");
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
            if well_formed && real_snap.name != port_snap.name {
                failures.push(format!("{label}: name real={:?} port={:?}", String::from_utf8_lossy(&real_snap.name), String::from_utf8_lossy(&port_snap.name)));
            }
        }
        compared += 1;
    }

    let shown = |list: &[String]| list.iter().take(10).map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", ");
    let summary = format!(
        "{} distinct call strings from {} .ai files in {} archives: {} compared ({} of them spill past the 128-byte section buffer into the dead name buffer), {} unsafe for the real parser [{}], {} not of the form name(...) [{}]",
        calls.len(),
        files,
        archives,
        compared,
        spills.len(),
        over_limit.len(),
        shown(&over_limit),
        not_a_call.len(),
        shown(&not_a_call)
    );
    finish_test(&format!("{test_name} ({summary})"), failures, failure_log)
}

/// `BFSCRIPTMGR_PARSE_FUNCTION_LONG_ARG` - documents the one place the port deliberately differs from vanilla on
/// input that stays clear of the return address. An argument over 31 bytes is written past `parseFunction`'s
/// 32-byte argument buffer into its section buffer, which moves that buffer's terminating NUL, so vanilla's next
/// comma search fails and every later argument is dropped. The port keeps them all. The first argument is intact
/// in both, and a 31-byte argument (the largest that fits) behaves identically.
pub(crate) fn run_bfscriptmgr_parse_function_long_arg_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFSCRIPTMGR_PARSE_FUNCTION_LONG_ARG";
    let mut failures: Vec<String> = Vec::new();
    for (length, vanilla_drops_rest) in [(31usize, false), (32, true), (40, true), (90, true)] {
        let long = "x".repeat(length);
        let input = format!("fCall({long},b,c)");
        let (real, port) = (bfscriptmgr_live_support::standalone_call(), bfscriptmgr_live_support::standalone_rust_call());
        let (real_ret, port_ret) = parse_both(&input, real, port);
        let (real_snap, port_snap) = (bfscriptmgr_live_support::snapshot(real), bfscriptmgr_live_support::snapshot(port));
        let full = vec![long.clone().into_bytes(), b"b".to_vec(), b"c".to_vec()];
        let first_only = vec![long.clone().into_bytes()];
        if !(real_ret && port_ret) {
            failures.push(format!("{length}-byte argument: return real={real_ret} port={port_ret}"));
        }
        if port_snap.args != full {
            failures.push(format!("{length}-byte argument: port args {:?} are not all three", port_snap.args.iter().map(|a| a.len()).collect::<Vec<_>>()));
        }
        let expected_real = if vanilla_drops_rest { &first_only } else { &full };
        if &real_snap.args != expected_real {
            failures.push(format!(
                "{length}-byte argument: real args lengths {:?}, expected {:?}",
                real_snap.args.iter().map(|a| a.len()).collect::<Vec<_>>(),
                expected_real.iter().map(|a| a.len()).collect::<Vec<_>>()
            ));
        }
    }
    finish_test(test_name, failures, failure_log)
}
