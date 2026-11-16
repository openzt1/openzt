//! Compares real vanilla `BFBehaviorSet` `setName`, `removeHead`, `clear`, `killChild`, `executeHead` and
//! `executeCall` against their Rust reimplementation (production file `openzt/src/bfbehaviorset.rs`) on
//! standalone sets. Strings and vectors are built and destroyed only through vanilla's own helpers, children
//! come from vanilla's `operator_new`, and root sets are leaked, so nothing crosses allocators. The "real"
//! pole runs the pre-detour body via `bfbehaviorset_live_support::real`, the "port" pole runs the address
//! through `.hooked()`.
//!
//! `executeCall`'s `*out` is `AL` from `ZTAIMgr::executeCall` (the handler's result), which the fake entity's
//! unknown call names make 0. The tests compare `*out`, the entity's post-parse slot calls and all other state
//! between poles, and also check each pole's `*out` agrees with whether that slot was called.

use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};

use tracing::error;

use openzt_detour::generated::bfbehaviorset::{CLEAR, EXECUTE_CALL, EXECUTE_HEAD, KILL_CHILD, REMOVE_HEAD, SET_NAME};

use crate::bfbehaviorset::live_support::{self as bfbehaviorset_live_support, SetSnapshot};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::util::get_from_memory;
use crate::vanilla_string::VanillaString;

use bfbehaviorset_live_support::real;

const GLOBAL_ZTAIMGR: u32 = 0x0063_8098;

fn compare(label: &str, real: &SetSnapshot, port: &SetSnapshot, failures: &mut Vec<String>) {
    if real != port {
        failures.push(format!("{label}: state real={real:?} port={port:?}"));
    }
}

/// `BFBEHAVIORSET_DETOURS_ENABLED` - wiring check.
pub(crate) fn run_bfbehaviorset_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_DETOURS_ENABLED";
    let disabled: Vec<&'static str> =
        bfbehaviorset_live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
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

/// `BFBEHAVIORSET_SET_NAME` - empty, new, longer, shorter and self-assigned names.
pub(crate) fn run_bfbehaviorset_set_name_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_SET_NAME";
    let mut failures: Vec<String> = Vec::new();
    let long = "aVeryLongBehaviorSetNameThatExceedsTheInitialBufferCapacity";

    for old in ["", "x", "oldName", long] {
        for new in ["", "walk", "oldName", long, "y"] {
            let real_set = bfbehaviorset_live_support::build_set(old.as_bytes(), b"", &[], &[], 0);
            let port_set = bfbehaviorset_live_support::build_set(old.as_bytes(), b"", &[], &[], 0);
            let source = VanillaString::new(new);
            let real_ret = real::set_name(real_set, source.as_ptr() as *const i32);
            let port_ret = unsafe { SET_NAME.hooked()(port_set as *const u32, source.as_ptr() as *const i32) };
            let label = format!("{old:?} -> {new:?}");
            if real_ret != port_ret {
                failures.push(format!("{label}: return real={real_ret} port={port_ret}"));
            }
            compare(&label, &bfbehaviorset_live_support::snapshot(real_set), &bfbehaviorset_live_support::snapshot(port_set), &mut failures);
        }

        let real_set = bfbehaviorset_live_support::build_set(old.as_bytes(), b"", &[], &[], 0);
        let port_set = bfbehaviorset_live_support::build_set(old.as_bytes(), b"", &[], &[], 0);
        let real_ret = real::set_name(real_set, bfbehaviorset_live_support::name_ptr(real_set));
        let port_ret = unsafe { SET_NAME.hooked()(port_set as *const u32, bfbehaviorset_live_support::name_ptr(port_set)) };
        let label = format!("{old:?} -> itself");
        if real_ret != port_ret {
            failures.push(format!("{label}: return real={real_ret} port={port_ret}"));
        }
        compare(&label, &bfbehaviorset_live_support::snapshot(real_set), &bfbehaviorset_live_support::snapshot(port_set), &mut failures);
    }

    finish_test(test_name, failures, failure_log)
}

/// `BFBEHAVIORSET_REMOVE_HEAD` - call lists of one to three entries with a parsed call.
pub(crate) fn run_bfbehaviorset_remove_head_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_REMOVE_HEAD";
    let mut failures: Vec<String> = Vec::new();
    let lists: [&[&[u8]]; 4] = [&[b"fA(1)"], &[b"fA(1)", b"fLongerSecondCall(1, 2, 3)"], &[b"fA(1)", b"fB()", b"fC(\"x\")"], &[b"a", b"bb", b"cccccccccccccccccccccccc", b"d"]];
    for list in lists {
        let real_set = bfbehaviorset_live_support::build_set(b"set", b"fA(1)", list, &[], 0);
        let port_set = bfbehaviorset_live_support::build_set(b"set", b"fA(1)", list, &[], 0);
        bfbehaviorset_live_support::prime_call(real_set, "fA(1, 2)");
        bfbehaviorset_live_support::prime_call(port_set, "fA(1, 2)");
        real::remove_head(real_set);
        unsafe { REMOVE_HEAD.hooked()(port_set as *const u32) };
        compare(&format!("{} entries", list.len()), &bfbehaviorset_live_support::snapshot(real_set), &bfbehaviorset_live_support::snapshot(port_set), &mut failures);
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFBEHAVIORSET_CLEAR` - populated sets with no child, one child and a nested child.
pub(crate) fn run_bfbehaviorset_clear_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_CLEAR";
    let mut failures: Vec<String> = Vec::new();
    let chains: [&[&[u8]]; 3] = [&[], &[b"childA"], &[b"childA", b"childB"]];
    for chain in chains {
        for populated in [false, true] {
            let (name, current, list): (&[u8], &[u8], &[&[u8]]) =
                if populated { (b"behaviorSet", b"fA(1, 2)", &[b"fA(1, 2)", b"fB()"]) } else { (b"", b"", &[]) };
            let real_set = bfbehaviorset_live_support::build_set(name, current, list, chain, 0);
            let port_set = bfbehaviorset_live_support::build_set(name, current, list, chain, 0);
            for set in [real_set, port_set] {
                bfbehaviorset_live_support::set_flags(set, [1, 1, 1, 1, 1]);
                if populated {
                    bfbehaviorset_live_support::prime_call(set, "fA(1, 2)");
                }
            }
            real::clear(real_set);
            unsafe { CLEAR.hooked()(port_set as *const u32) };
            compare(
                &format!("children={} populated={populated}", chain.len()),
                &bfbehaviorset_live_support::snapshot(real_set),
                &bfbehaviorset_live_support::snapshot(port_set),
                &mut failures,
            );
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFBEHAVIORSET_KILL_CHILD` - no child, one child and a nested child.
pub(crate) fn run_bfbehaviorset_kill_child_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_KILL_CHILD";
    let mut failures: Vec<String> = Vec::new();
    let chains: [&[&[u8]]; 3] = [&[], &[b"childA"], &[b"childA", b"childB"]];
    for chain in chains {
        let real_set = bfbehaviorset_live_support::build_set(b"set", b"fA()", &[b"fA()"], chain, 0);
        let port_set = bfbehaviorset_live_support::build_set(b"set", b"fA()", &[b"fA()"], chain, 0);
        real::kill_child(real_set);
        unsafe { KILL_CHILD.hooked()(port_set as *const u32) };
        compare(&format!("children={}", chain.len()), &bfbehaviorset_live_support::snapshot(real_set), &bfbehaviorset_live_support::snapshot(port_set), &mut failures);
    }
    finish_test(test_name, failures, failure_log)
}

static AFTER_PARSE_CALLS: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_is_cast(_this: u32, _tag: u32) -> bool {
    false
}

extern "thiscall" fn fake_after_parse(_this: u32) {
    AFTER_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
}

/// An entity just real enough for `ZTAIMgr::executeCall`'s unknown-name path: a type object whose
/// `isCastClass` answers false, and a vtable whose `+0x4c` slot counts its calls.
fn build_fake_entity() -> u32 {
    let leak = |words: usize| Box::leak(vec![0u32; words].into_boxed_slice());
    let type_vtable = leak(0x80);
    type_vtable[0x1c / 4] = fake_is_cast as *const () as usize as u32;
    let type_object = leak(4);
    type_object[0] = type_vtable.as_ptr() as u32;
    let entity_vtable = leak(0x80);
    entity_vtable[0x4c / 4] = fake_after_parse as *const () as usize as u32;
    let entity = leak(0x80);
    entity[0] = entity_vtable.as_ptr() as u32;
    entity[0x128 / 4] = type_object.as_ptr() as u32;
    entity.as_ptr() as u32
}

fn manager_missing() -> bool {
    get_from_memory::<u32>(GLOBAL_ZTAIMGR) == 0
}

/// Runs one pole and returns `(out, post-parse slot calls)`.
fn run_pole(run: impl FnOnce(*const u8)) -> (u8, u32) {
    let out: u8 = 0xaa;
    AFTER_PARSE_CALLS.store(0, Ordering::SeqCst);
    run(&out);
    (unsafe { std::ptr::read_volatile(&out) }, AFTER_PARSE_CALLS.load(Ordering::SeqCst))
}

fn check_pole(label: &str, pole: &str, parsed: bool, (out, slot_calls): (u8, u32), failures: &mut Vec<String>) {
    if out > 1 {
        failures.push(format!("{label}: {pole} left out={out:#x}"));
    }
    let expected = (parsed && out == 0) as u32;
    if slot_calls != expected {
        failures.push(format!("{label}: {pole} called the post-parse slot {slot_calls} times with out={out}, parsed={parsed}"));
    }
}

/// `BFBEHAVIORSET_EXECUTE_CALL` and `_HEAD` share this driver. Each case runs with a stale parse (reparse
/// forced by `removed`), no parsed call yet, and an already parsed call that is reused.
fn run_execute_test(test_name: &str, head: bool, failure_log: &mut Option<std::fs::File>) -> bool {
    if manager_missing() {
        write_success_line(failure_log, &format!("{test_name} (skipped: no GLOBAL_ZTAIMgr)"));
        return false;
    }
    let mut failures: Vec<String> = Vec::new();
    let entity = build_fake_entity();
    let calls = ["fNoSuchCall(1, 2)", "fNoSuchCall()", "fOther(\"a b\")"];

    for call in calls {
        for (removed, primed) in [(1u8, false), (0, false), (0, true), (1, true)] {
            let make = || {
                let set = bfbehaviorset_live_support::build_set(b"set", b"", &[call.as_bytes(), b"fSecond()"], &[], entity);
                if primed {
                    bfbehaviorset_live_support::prime_call(set, "fNoSuchCall(9)");
                }
                bfbehaviorset_live_support::set_flags(set, [1, 0, removed, 0, 1]);
                set
            };
            let (real_set, port_set) = (make(), make());
            let text = VanillaString::new(call);
            let parsed = removed != 0 || !primed;
            let label = format!("{call:?} removed={removed} primed={primed}");

            let real_result = run_pole(|out| {
                if head {
                    real::execute_head(real_set, out)
                } else {
                    real::execute_call(real_set, text.as_ptr(), out)
                }
            });
            let port_result = run_pole(|out| unsafe {
                if head {
                    EXECUTE_HEAD.hooked()(port_set as *const u32, out)
                } else {
                    EXECUTE_CALL.hooked()(port_set as *const u32, text.as_ptr(), out)
                }
            });
            if real_result != port_result {
                failures.push(format!("{label}: (out, slot calls) real={real_result:?} port={port_result:?}"));
            }
            check_pole(&label, "real", parsed, real_result, &mut failures);
            check_pole(&label, "port", parsed, port_result, &mut failures);
            compare(&label, &bfbehaviorset_live_support::snapshot(real_set), &bfbehaviorset_live_support::snapshot(port_set), &mut failures);
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFBEHAVIORSET_EXECUTE_CALL`.
pub(crate) fn run_bfbehaviorset_execute_call_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_execute_test("BFBEHAVIORSET_EXECUTE_CALL", false, failure_log)
}

/// `BFBEHAVIORSET_EXECUTE_HEAD`.
pub(crate) fn run_bfbehaviorset_execute_head_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_execute_test("BFBEHAVIORSET_EXECUTE_HEAD", true, failure_log)
}
