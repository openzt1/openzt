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
use std::sync::{
    atomic::{AtomicU32, AtomicU8, Ordering},
    Mutex,
};

use tracing::error;

use openzt_detour::generated::{
    bfbehaviorset::{CLEAR, CONSTRUCTOR, DESTRUCTOR, EXECUTE, EXECUTE_CALL, EXECUTE_CHILD, EXECUTE_HEAD, INIT_CHILD, KILL_CHILD, LOAD_0, LOAD_1, REMOVE_HEAD, SAVE, SET_NAME},
    bffunctioncall::{LOAD as CALL_LOAD, SAVE as CALL_SAVE},
    poolalloc::ALLOCATE as POOL_ALLOCATE,
};

use crate::bfbehaviorset::live_support::{self as bfbehaviorset_live_support, SetSnapshot};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::bfscriptmgr::live_support as call_support;
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, save_to_memory};
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

/// `BFBEHAVIORSET_DESTRUCTOR` - `~BFBehaviorSet` against vanilla on sets with no, one and two nested children,
/// comparing the set's state and how many blocks each pool size class gains. The root set is not freed by the
/// destructor, so it stays allocated and only its members are checked.
pub(crate) fn run_bfbehaviorset_destructor_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_DESTRUCTOR";
    let mut failures: Vec<String> = Vec::new();
    let chains: [&[&[u8]]; 3] = [&[], &[b"childA"], &[b"childA", b"childB"]];
    for chain in chains {
        let real_set = bfbehaviorset_live_support::build_set(b"set", b"fA()", &[b"fA()", b"fB(1, 2)"], chain, 0);
        let port_set = bfbehaviorset_live_support::build_set(b"set", b"fA()", &[b"fA()", b"fB(1, 2)"], chain, 0);
        bfbehaviorset_live_support::prime_call(real_set, "fA(1, 2)");
        bfbehaviorset_live_support::prime_call(port_set, "fA(1, 2)");

        super::bfentity::warm_pool();
        let before = super::bfentity::pool_lengths();
        real::destructor(real_set);
        let middle = super::bfentity::pool_lengths();
        unsafe { DESTRUCTOR.hooked()(port_set as *const u32) };
        let after = super::bfentity::pool_lengths();

        let real_delta: Vec<u32> = middle.iter().zip(&before).map(|(a, b)| a.wrapping_sub(*b)).collect();
        let port_delta: Vec<u32> = after.iter().zip(&middle).map(|(a, b)| a.wrapping_sub(*b)).collect();
        let label = format!("children={}", chain.len());
        if real_delta != port_delta {
            failures.push(format!("{label}: freelist growth real={real_delta:?} port={port_delta:?}"));
        }
        if real_delta.iter().all(|delta| *delta == 0) {
            failures.push(format!("{label}: vanilla freed nothing, the fixture is not exercising the teardown"));
        }
        for offset in (0..0x60u32).step_by(4) {
            let (r, p) = (get_from_memory::<u32>(real_set + offset), get_from_memory::<u32>(port_set + offset));
            if r != p && !(r >= 0x0100_0000 && p >= 0x0100_0000) {
                failures.push(format!("{label}: +{offset:#x} real={r:#x} port={p:#x}"));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static AFTER_PARSE_CALLS: AtomicU32 = AtomicU32::new(0);
/// NUL-terminated lines the fake behavior manager returns, and its return value.
pub(crate) static SCRIPT_LINES: Mutex<Vec<&'static [u8]>> = Mutex::new(Vec::new());
pub(crate) static SCRIPT_RESULT: AtomicU8 = AtomicU8::new(1);

/// Stands in for the entity behavior manager's script lookup (vtable `+0x38`): fills `out` with
/// `SCRIPT_LINES` in a pool-allocated buffer, which `load` frees through vanilla's pool.
extern "thiscall" fn fake_get_script(_this: u32, out: *mut [u32; 3], _name: *const u32) -> u8 {
    let lines = SCRIPT_LINES.lock().unwrap();
    if !lines.is_empty() {
        let size = lines.len() * 4;
        let buffer = unsafe { POOL_ALLOCATE.original()(size as u32) } as *mut u32;
        for (i, line) in lines.iter().enumerate() {
            unsafe { buffer.add(i).write(line.as_ptr() as u32) };
        }
        let begin = buffer as u32;
        unsafe { out.write([begin, begin + size as u32, begin + size as u32]) };
    }
    SCRIPT_RESULT.load(Ordering::SeqCst)
}

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
    let type_object = build_fake_type();
    let entity_vtable = leak(0x80);
    entity_vtable[0x4c / 4] = fake_after_parse as *const () as usize as u32;
    let entity = leak(0x80);
    entity[0] = entity_vtable.as_ptr() as u32;
    entity[0x128 / 4] = type_object;
    entity.as_ptr() as u32
}

/// A type object whose behavior manager (`+0x80`, present because `+0x84` is non-zero) returns `SCRIPT_LINES`
/// and whose `isCastClass` answers false.
pub(crate) fn build_fake_type() -> u32 {
    let leak = |words: usize| Box::leak(vec![0u32; words].into_boxed_slice());
    let type_vtable = leak(0x80);
    type_vtable[0x1c / 4] = fake_is_cast as *const () as usize as u32;
    type_vtable[0x38 / 4] = fake_get_script as *const () as usize as u32;
    let type_object = leak(0x40);
    type_object[0] = type_vtable.as_ptr() as u32;
    type_object[0x84 / 4] = 1;
    type_object.as_ptr() as u32
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

static DISPATCH_PATTERN: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static DISPATCH_COUNT: AtomicU32 = AtomicU32::new(0);

/// Fake `ZTAIMgr::executeCall` (vtable `+0xc`): writes the parsed call's result flag to `done` the way the real
/// one does, and returns the next scripted `AL`.
extern "thiscall" fn fake_execute_call(_this: u32, call: u32, _entity: u32, done: *mut u8) -> u8 {
    unsafe { done.write_volatile(get_from_memory::<u8>(call + 0x18)) };
    let index = DISPATCH_COUNT.fetch_add(1, Ordering::SeqCst) as usize;
    let pattern = DISPATCH_PATTERN.lock().unwrap();
    pattern[index % pattern.len()]
}

/// Points `GLOBAL_ZTAIMgr` at a fake manager until dropped.
struct FakeManagerGuard {
    previous: u32,
}

impl FakeManagerGuard {
    fn install() -> Self {
        let vtable = Box::leak(vec![0u32; 0x20].into_boxed_slice());
        vtable[0xc / 4] = fake_execute_call as *const () as usize as u32;
        let manager = Box::leak(vec![0u32; 4].into_boxed_slice());
        manager[0] = vtable.as_ptr() as u32;
        let previous = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
        save_to_memory(GLOBAL_ZTAIMGR, manager.as_ptr() as u32);
        Self { previous }
    }
}

impl Drop for FakeManagerGuard {
    fn drop(&mut self) {
        save_to_memory(GLOBAL_ZTAIMGR, self.previous);
    }
}

fn compare_chains(label: &str, real_set: u32, port_set: u32, failures: &mut Vec<String>) {
    let (real_chain, port_chain) = (bfbehaviorset_live_support::snapshot_chain(real_set), bfbehaviorset_live_support::snapshot_chain(port_set));
    if real_chain != port_chain {
        failures.push(format!("{label}: chain real={real_chain:?} port={port_chain:?}"));
    }
}

fn entity_flag(entity: u32) -> u8 {
    unsafe { std::ptr::read_volatile((entity + 0xd6) as *const u8) }
}

/// `BFBEHAVIORSET_LOAD` - `load()` against scripts of zero, one, several and long lines, and the three
/// preconditions that make it return false.
pub(crate) fn run_bfbehaviorset_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_LOAD";
    let mut failures: Vec<String> = Vec::new();
    let entity = build_fake_entity();
    let long: &'static [u8] = b"fACallWithAVeryLongArgumentListThatExceedsTheInitialStringBuffer(aaaaaaaa, bbbbbbbb, cccccccc)\0";
    let scripts: [&[&'static [u8]]; 4] = [&[], &[b"fA(1)\0"], &[b"fA(1)\0", b"fB()\0", b"fC(\"x y\", 2)\0"], &[long, b"fB()\0"]];
    let old_entries: [&[&[u8]]; 2] = [&[], &[b"old1", b"old2"]];

    for script in scripts {
        for result in [0u8, 1] {
            for existing in old_entries {
                for (name, break_precondition) in [(&b"set"[..], 0), (b"", 1), (b"set", 2), (b"set", 3)] {
                    *SCRIPT_LINES.lock().unwrap() = script.to_vec();
                    SCRIPT_RESULT.store(result, Ordering::SeqCst);
                    let make = || {
                        let set = bfbehaviorset_live_support::build_set(name, b"", existing, &[], entity);
                        bfbehaviorset_live_support::bind(set, entity);
                        match break_precondition {
                            2 => unsafe { ((set + 0x50) as *mut u32).write(0) },
                            3 => unsafe { ((set + 0x54) as *mut u32).write(0) },
                            _ => {}
                        }
                        bfbehaviorset_live_support::set_flags(set, [1, 1, 0, 0, 0]);
                        set
                    };
                    let (real_set, port_set) = (make(), make());
                    let label = format!("lines={} result={result} existing={} name={:?} break={break_precondition}", script.len(), existing.len(), name);
                    let real_ret = real::load(real_set);
                    let port_ret = unsafe { LOAD_0.hooked()(port_set as *const u32) };
                    if real_ret != port_ret {
                        failures.push(format!("{label}: returned real={real_ret} port={port_ret}"));
                    }
                    compare(&label, &bfbehaviorset_live_support::snapshot(real_set), &bfbehaviorset_live_support::snapshot(port_set), &mut failures);
                }
            }
        }
    }
    SCRIPT_LINES.lock().unwrap().clear();
    finish_test(test_name, failures, failure_log)
}

/// `BFBEHAVIORSET_INIT_CHILD` - under a set with no child, one child and a nested child, with and without
/// an entity, and with scripts that load or come back empty.
pub(crate) fn run_bfbehaviorset_init_child_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_INIT_CHILD";
    let mut failures: Vec<String> = Vec::new();
    let entity = build_fake_entity();
    let chains: [&[&[u8]]; 3] = [&[], &[b"first"], &[b"first", b"second"]];
    let scripts: [&[&'static [u8]]; 2] = [&[], &[b"fA(1)\0", b"fB()\0"]];

    for chain in chains {
        for script in scripts {
            for child_entity in [entity, 0] {
                *SCRIPT_LINES.lock().unwrap() = script.to_vec();
                SCRIPT_RESULT.store(1, Ordering::SeqCst);
                let make = || {
                    let set = bfbehaviorset_live_support::build_set(b"root", b"", &[], chain, entity);
                    bfbehaviorset_live_support::bind(set, entity);
                    set
                };
                let (real_set, port_set) = (make(), make());
                let name = VanillaString::new("childSet");
                let label = format!("children={} lines={} entity={child_entity:#x}", chain.len(), script.len());
                let real_ret = real::init_child(real_set, child_entity, name.as_ptr() as *const i32);
                let port_ret = unsafe { INIT_CHILD.hooked()(port_set as *const u32, child_entity as i32, name.as_ptr() as *const i32) };
                if real_ret != port_ret {
                    failures.push(format!("{label}: returned real={real_ret} port={port_ret}"));
                }
                // With no entity vanilla never writes `+0x54`, so a child keeps whatever the heap held there.
                for set in [real_set, port_set] {
                    for node in bfbehaviorset_live_support::chain(set) {
                        if get_from_memory::<u32>(node + 0x50) == 0 {
                            unsafe { ((node + 0x54) as *mut u32).write(0) };
                        }
                    }
                }
                compare_chains(&label, real_set, port_set, &mut failures);
            }
        }
    }
    SCRIPT_LINES.lock().unwrap().clear();
    finish_test(test_name, failures, failure_log)
}

/// Builds a set for the `execute` family: a root with `list`, and an optional child that is idle
/// (`child == 1`, `dirty == 0`) or running one more call (`child == 2`).
fn build_execute_set(entity: u32, list: &[&[u8]], child: u8, root_flags: [u8; 5], primed: bool) -> u32 {
    let chain: &[&[u8]] = if child == 0 { &[] } else { &[b"child"] };
    let set = bfbehaviorset_live_support::build_set(b"set", b"", list, chain, entity);
    bfbehaviorset_live_support::bind(set, entity);
    if primed {
        bfbehaviorset_live_support::prime_call(set, "fNoSuchCall(9)");
    }
    bfbehaviorset_live_support::set_flags(set, root_flags);
    if child != 0 {
        let child_set = get_from_memory::<u32>(set + 0x4c);
        if child == 2 {
            bfbehaviorset_live_support::push_list(child_set, b"fNoSuchCall()");
        }
        bfbehaviorset_live_support::set_flags(child_set, [1, (child == 2) as u8, 0, 0, 1]);
    }
    set
}

/// `BFBEHAVIORSET_EXECUTE` and `_EXECUTE_CHILD` share this driver across the dirty/removed/pending flag
/// combinations, call lists of zero to two entries, and no, an idle or a running child.
///
/// With `patterns == None` the real `ZTAIMgr` runs; its unknown call names always report "call finished". With
/// `Some(patterns)` a fake manager answers each dispatch from the pattern (nonzero `AL` means the call is
/// still running), which covers the multi-tick paths the real manager cannot reach.
fn run_execute_family_test(test_name: &str, child_only: bool, patterns: Option<&[&[u8]]>, failure_log: &mut Option<std::fs::File>) -> bool {
    if patterns.is_none() && manager_missing() {
        write_success_line(failure_log, &format!("{test_name} (skipped: no GLOBAL_ZTAIMgr)"));
        return false;
    }
    let mut failures: Vec<String> = Vec::new();
    let entity = build_fake_entity();
    let lists: [&[&[u8]]; 3] = [&[], &[b"fNoSuchCall(1)"], &[b"fNoSuchCall(1)", b"fOther()"]];
    let _manager = patterns.map(|_| FakeManagerGuard::install());

    for pattern in patterns.map_or_else(|| vec![None], |all| all.iter().map(|p| Some(*p)).collect()) {
        if let Some(pattern) = pattern {
            *DISPATCH_PATTERN.lock().unwrap() = pattern.to_vec();
        }
    for list in lists {
        for child in [0u8, 1, 2] {
            for flags in 0u8..8 {
                for primed in [false, true] {
                    for loaded in [1u8, 0] {
                        let root_flags = [1, flags & 1, (flags >> 1) & 1, (flags >> 2) & 1, loaded];
                        // Vanilla's own `execute` never returns for this state: a running child leaves `removed`
                        // set, and `removeHead` only clears it while `dirty` is set. It cannot arise in play.
                        let spins_forever = patterns.is_some() && !child_only && child == 2 && loaded == 1 && flags & 1 == 0 && (flags >> 1) & 1 == 1;
                        if spins_forever && !list.is_empty() {
                            continue;
                        }
                        let make = || build_execute_set(entity, list, child, root_flags, primed);
                        let (real_set, port_set) = (make(), make());
                        let label = format!("list={} child={child} flags={root_flags:?} primed={primed} pattern={pattern:?}", list.len());

                        let run = |set: u32, real: bool| -> (u8, u32, u8) {
                            AFTER_PARSE_CALLS.store(0, Ordering::SeqCst);
                            DISPATCH_COUNT.store(0, Ordering::SeqCst);
                            unsafe { ((entity + 0xd6) as *mut u8).write_volatile(0) };
                            let mut out: u8 = 0xaa;
                            let out_ptr = &mut out as *mut u8;
                            if child_only {
                                let ret = if real { real::execute_child(set) } else { unsafe { EXECUTE_CHILD.hooked()(set as *const u32) } };
                                unsafe { out_ptr.write_volatile(ret as u8) };
                            } else if real {
                                real::execute(set, out_ptr)
                            } else {
                                unsafe { EXECUTE.hooked()(set as *const u32, out_ptr as *const bool) }
                            }
                            (unsafe { out_ptr.read_volatile() }, AFTER_PARSE_CALLS.load(Ordering::SeqCst), entity_flag(entity))
                        };
                        let real_result = run(real_set, true);
                        let real_dispatches = DISPATCH_COUNT.load(Ordering::SeqCst);
                        let port_result = run(port_set, false);
                        let port_dispatches = DISPATCH_COUNT.load(Ordering::SeqCst);
                        if real_result != port_result || real_dispatches != port_dispatches {
                            failures.push(format!(
                                "{label}: (out, slot calls, entity flag) real={real_result:?} port={port_result:?}, dispatches real={real_dispatches} port={port_dispatches}"
                            ));
                        }
                        compare_chains(&label, real_set, port_set, &mut failures);
                    }
                }
            }
        }
    }
    }
    finish_test(test_name, failures, failure_log)
}

/// Scripted answers for the fake `ZTAIMgr`: dispatch `n` returns `pattern[n % len]`.
const RUNNING_PATTERNS: [&[u8]; 3] = [&[1], &[0, 1], &[1, 0]];

/// `BFBEHAVIORSET_EXECUTE_RUNNING` - `execute` while calls report "still running".
pub(crate) fn run_bfbehaviorset_execute_running_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_execute_family_test("BFBEHAVIORSET_EXECUTE_RUNNING", false, Some(&RUNNING_PATTERNS), failure_log)
}

/// `BFBEHAVIORSET_EXECUTE_CHILD_RUNNING` - `executeChild` while the child's calls report "still running".
pub(crate) fn run_bfbehaviorset_execute_child_running_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_execute_family_test("BFBEHAVIORSET_EXECUTE_CHILD_RUNNING", true, Some(&RUNNING_PATTERNS), failure_log)
}

/// `BFBEHAVIORSET_EXECUTE`.
pub(crate) fn run_bfbehaviorset_execute_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_execute_family_test("BFBEHAVIORSET_EXECUTE", false, None, failure_log)
}

/// `BFBEHAVIORSET_EXECUTE_CHILD`.
pub(crate) fn run_bfbehaviorset_execute_child_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_execute_family_test("BFBEHAVIORSET_EXECUTE_CHILD", true, None, failure_log)
}

/// A saved behavior set, in the order the save file stores it.
#[derive(Clone)]
struct SavedSet {
    name: &'static [u8],
    flag_58: u8,
    stack: Vec<&'static [u8]>,
    current_call: &'static [u8],
    call_name: &'static [u8],
    call_args: Vec<&'static [u8]>,
    call_flags: [u8; 3],
    child: Option<Box<SavedSet>>,
    list: Vec<&'static [u8]>,
    /// `+0x5c`, `+0x5a`, `+0x59`, `+0x5b`.
    flags: [u8; 4],
}

impl SavedSet {
    fn simple(name: &'static [u8]) -> Self {
        Self {
            name,
            flag_58: 1,
            stack: Vec::new(),
            current_call: b"",
            call_name: b"",
            call_args: Vec::new(),
            call_flags: [0; 3],
            child: None,
            list: Vec::new(),
            flags: [0; 4],
        }
    }
}

fn put_string(out: &mut Vec<u8>, text: &[u8]) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text);
}

fn put_strings(out: &mut Vec<u8>, items: &[&[u8]]) {
    out.extend_from_slice(&(items.len() as u32).to_le_bytes());
    for item in items {
        put_string(out, item);
    }
}

fn encode_set(set: &SavedSet, out: &mut Vec<u8>) {
    put_string(out, set.name);
    out.push(set.flag_58);
    put_strings(out, &set.stack);
    put_string(out, set.current_call);
    put_string(out, set.call_name);
    put_strings(out, &set.call_args);
    out.extend_from_slice(&set.call_flags);
    out.push(set.child.is_some() as u8);
    if let Some(child) = &set.child {
        encode_set(child, out);
    }
    put_strings(out, &set.list);
    out.extend_from_slice(&set.flags);
}

/// `(real return, port return, bytes each pole consumed)` and the state both leave behind.
fn run_load_poles(label: &str, bytes: &[u8], version: u32, entity: u32, failures: &mut Vec<String>) {
    let make = || {
        let set = bfbehaviorset_live_support::build_set(b"old", b"oldcall", &[b"x", b"y"], &[b"oldchild"], entity);
        bfbehaviorset_live_support::bind(set, entity);
        bfbehaviorset_live_support::set_flags(set, [1, 1, 1, 1, 1]);
        set
    };
    let (real_set, port_set) = (make(), make());

    io_redirect::begin_replay(bytes.to_vec());
    let real_ret = real::load_file(real_set, std::ptr::null(), version, entity);
    let real_consumed = io_redirect::replay_position();
    io_redirect::end_replay();

    io_redirect::begin_replay(bytes.to_vec());
    let port_ret = unsafe { LOAD_1.hooked()(port_set as *const u32, std::ptr::null(), version, entity as i32) };
    let port_consumed = io_redirect::replay_position();
    io_redirect::end_replay();

    if real_ret != port_ret {
        failures.push(format!("{label}: returned real={real_ret} port={port_ret}"));
    }
    if real_consumed != port_consumed {
        failures.push(format!("{label}: consumed real={real_consumed} port={port_consumed} of {}", bytes.len()));
    }
    compare_chains(label, real_set, port_set, failures);
}

/// `BFBEHAVIORSET_LOAD_FILE` - the save-file `load` against synthetic saves: empty, populated, with one and two
/// nested children, long strings, strings at the length limit, old file versions, and every truncation of a
/// populated save (each failure path).
pub(crate) fn run_bfbehaviorset_load_file_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_LOAD_FILE";
    let mut failures: Vec<String> = Vec::new();
    let entity = build_fake_entity();
    let long: &'static [u8] = b"aVeryLongBehaviorSetNameThatExceedsTheInitialStringBufferCapacity";

    let mut populated = SavedSet::simple(b"walk");
    populated.stack = vec![b"first", b"second", b"third"];
    populated.current_call = b"fMoveF(1, 2)";
    populated.call_name = b"fMoveF";
    populated.call_args = vec![b"1", b"2"];
    populated.call_flags = [1, 0, 1];
    populated.list = vec![b"fMoveF(1, 2)", b"fWait()", long];
    populated.flags = [1, 0, 1, 1];

    let mut one_child = populated.clone();
    one_child.child = Some(Box::new(SavedSet::simple(b"childSet")));
    let mut nested = populated.clone();
    let mut mid = SavedSet::simple(b"mid");
    mid.list = vec![b"fA()"];
    mid.child = Some(Box::new(SavedSet::simple(b"leaf")));
    nested.child = Some(Box::new(mid));
    let mut long_strings = SavedSet::simple(long);
    long_strings.stack = vec![long, long];
    long_strings.call_args = vec![long];
    long_strings.list = vec![long];

    let cases: Vec<(&str, SavedSet)> =
        vec![("empty", SavedSet::simple(b"")), ("populated", populated.clone()), ("one child", one_child), ("nested children", nested), ("long strings", long_strings)];

    for (label, set) in &cases {
        let mut bytes = Vec::new();
        encode_set(set, &mut bytes);
        for version in [0x3e, 0x3f, 0x40] {
            run_load_poles(&format!("{label} v{version:#x}"), &bytes, version, entity, &mut failures);
        }
    }

    let mut bytes = Vec::new();
    encode_set(&populated, &mut bytes);
    for cut in 0..bytes.len() {
        run_load_poles(&format!("populated truncated to {cut}/{}", bytes.len()), &bytes[..cut], 0x40, entity, &mut failures);
    }

    let oversized = {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x1000u32.to_le_bytes());
        bytes.extend_from_slice(&[b'a'; 0x20]);
        bytes
    };
    run_load_poles("name length 0x1000", &oversized, 0x40, entity, &mut failures);
    let mut just_under = Vec::new();
    let mut under = SavedSet::simple(b"");
    let big: &'static [u8] = Box::leak(vec![b'b'; 0xfff].into_boxed_slice());
    under.name = big;
    encode_set(&under, &mut just_under);
    run_load_poles("name length 0xfff", &just_under, 0x40, entity, &mut failures);

    finish_test(test_name, failures, failure_log)
}

/// `BFBEHAVIORSET_CONSTRUCTOR` - the fused constructor, with and without an entity.
pub(crate) fn run_bfbehaviorset_constructor_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_CONSTRUCTOR";
    let mut failures: Vec<String> = Vec::new();
    let entity = build_fake_entity();

    for entity in [entity, 0] {
        // Without an entity vanilla never writes `+0x54`, so start from zeroed memory; otherwise dirty memory.
        let fill = if entity == 0 { 0u32 } else { 0xaaaa_aaaa };
        let alloc = || Box::leak(vec![fill; 0x60 / 4].into_boxed_slice()).as_mut_ptr() as u32;
        let (real_set, port_set) = (alloc(), alloc());

        let real_ret = real::constructor(real_set, entity);
        let port_ret = unsafe { CONSTRUCTOR.hooked()(port_set as *const u32, entity as i32) } as u32;
        let label = format!("entity={entity:#x}");
        if real_ret != real_set || port_ret != port_set {
            failures.push(format!("{label}: did not return this (real {real_ret:#x} for {real_set:#x}, port {port_ret:#x} for {port_set:#x})"));
        }
        compare(&label, &bfbehaviorset_live_support::snapshot(real_set), &bfbehaviorset_live_support::snapshot(port_set), &mut failures);
    }
    finish_test(test_name, failures, failure_log)
}

/// Runs `run` with `WRITE_BYTES_TO_FILE` redirected into memory and returns its result and the bytes written.
fn capture(run: impl FnOnce() -> bool) -> (bool, Vec<u8>) {
    io_redirect::begin_capture();
    let ok = run();
    (ok, io_redirect::end_capture())
}

struct SaveCase {
    label: &'static str,
    name: Vec<u8>,
    extra_stack: Vec<&'static [u8]>,
    current: &'static [u8],
    call_text: Option<&'static str>,
    list: Vec<&'static [u8]>,
    children: Vec<&'static [u8]>,
    /// `+0x58`, `+0x59`, `+0x5a`, `+0x5b`, `+0x5c`; distinct values so a wrong order shows.
    flags: [u8; 5],
}

fn build_save_set(case: &SaveCase, entity: u32) -> u32 {
    let set = bfbehaviorset_live_support::build_set(&case.name, case.current, &case.list, &case.children, entity);
    bfbehaviorset_live_support::bind(set, entity);
    for text in &case.extra_stack {
        bfbehaviorset_live_support::push_stack(set, text);
    }
    if let Some(text) = case.call_text {
        bfbehaviorset_live_support::prime_call(set, text);
    }
    bfbehaviorset_live_support::set_flags(set, case.flags);
    for (depth, node) in bfbehaviorset_live_support::chain(set).into_iter().enumerate().skip(1) {
        bfbehaviorset_live_support::prime_call(node, "fChild(7, 8)");
        bfbehaviorset_live_support::set_flags(node, [9 + depth as u8, 8, 7, 6, 5]);
    }
    set
}

/// `BFBEHAVIORSET_SAVE` - `save` against vanilla byte for byte (and the result), that it leaves the set
/// untouched, and that the bytes load back into an equal set.
pub(crate) fn run_bfbehaviorset_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFBEHAVIORSET_SAVE";
    let mut failures: Vec<String> = Vec::new();
    let entity = build_fake_entity();
    let long: &'static [u8] = b"aVeryLongBehaviorSetNameThatExceedsTheInitialStringBufferCapacity";
    let oversize: &'static [u8] = Box::leak(vec![b'o'; 0x1000].into_boxed_slice());
    let just_under: Vec<u8> = vec![b'u'; 0xfff];

    let base = |label| SaveCase { label, name: b"".to_vec(), extra_stack: vec![], current: b"", call_text: None, list: vec![], children: vec![], flags: [0; 5] };
    let populated = |label| SaveCase {
        label,
        name: b"walk".to_vec(),
        extra_stack: vec![b"s1", b"s2"],
        current: b"fMoveF(1, 2)",
        call_text: Some("fMoveF(1, 2)"),
        list: vec![b"fMoveF(1, 2)", b"fWait()", long],
        children: vec![],
        flags: [1, 2, 3, 4, 5],
    };
    let mut one_child = populated("one child");
    one_child.children = vec![b"child"];
    let mut nested = populated("nested children");
    nested.children = vec![b"mid", b"leaf"];
    let mut oversize_name = populated("name of 0x1000 bytes");
    oversize_name.name = oversize.to_vec();
    let mut oversize_list = populated("list entry of 0x1000 bytes");
    oversize_list.list = vec![b"fA()", oversize];
    let mut under_name = populated("name of 0xfff bytes");
    under_name.name = just_under;
    let cases = vec![base("empty"), populated("populated"), one_child, nested, oversize_name, oversize_list, under_name];

    for case in &cases {
        let (real_set, port_set) = (build_save_set(case, entity), build_save_set(case, entity));
        let before = bfbehaviorset_live_support::snapshot_chain(port_set);
        let (real_ok, real_bytes) = capture(|| real::save(real_set, std::ptr::null()));
        let (port_ok, port_bytes) = capture(|| unsafe { SAVE.hooked()(port_set as *const u32, std::ptr::null()) });
        let label = case.label;
        if real_ok != port_ok {
            failures.push(format!("{label}: returned real={real_ok} port={port_ok}"));
        }
        if real_bytes != port_bytes {
            failures.push(format!("{label}: bytes differ (real {} bytes, port {} bytes)", real_bytes.len(), port_bytes.len()));
        }
        if bfbehaviorset_live_support::snapshot_chain(port_set) != before {
            failures.push(format!("{label}: save changed the set"));
        }
        compare_chains(label, real_set, port_set, &mut failures);

        if real_ok {
            let target = bfbehaviorset_live_support::build_set(b"old", b"oldcall", &[b"x"], &[], entity);
            bfbehaviorset_live_support::bind(target, entity);
            io_redirect::begin_replay(port_bytes.clone());
            let loaded = unsafe { LOAD_1.hooked()(target as *const u32, std::ptr::null(), 0x40, entity as i32) };
            let consumed = io_redirect::replay_position();
            io_redirect::end_replay();
            if !loaded || consumed != port_bytes.len() {
                failures.push(format!("{label}: round trip load returned {loaded}, consumed {consumed} of {}", port_bytes.len()));
            }
            let (want, got) = (bfbehaviorset_live_support::snapshot_chain(port_set), bfbehaviorset_live_support::snapshot_chain(target));
            if want != got {
                failures.push(format!("{label}: round trip differs: saved={want:?} loaded={got:?}"));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFFUNCTIONCALL_SAVE_LOAD` - the call object's `save` and `load` against vanilla: identical bytes from the
/// same call, and identical state and bytes consumed when loading each truncation and old file versions.
pub(crate) fn run_bffunctioncall_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFFUNCTIONCALL_SAVE_LOAD";
    let mut failures: Vec<String> = Vec::new();
    let texts = ["f()", "fMoveF(1, 2)", "fPlaySet(\"a b\", 3, x)", "fLong(aVeryLongArgumentThatExceedsTheInitialStringBufferCapacity, b)"];

    let primed = |text: &str, flags: [u8; 3]| {
        let call = call_support::standalone_call();
        let source = VanillaString::new(text);
        call_support::real_parse_function(source.as_ptr(), call);
        for (offset, flag) in flags.iter().enumerate() {
            unsafe { ((call + 0x18 + offset as u32) as *mut u8).write(*flag) };
        }
        call
    };

    for text in texts {
        let (real_call, port_call) = (primed(text, [1, 2, 3]), primed(text, [1, 2, 3]));
        let (real_ok, real_bytes) = capture(|| call_support::real::save_call(real_call, std::ptr::null()));
        let (port_ok, port_bytes) = capture(|| unsafe { CALL_SAVE.hooked()(port_call as *const u32, std::ptr::null()) });
        if real_ok != port_ok || real_bytes != port_bytes {
            failures.push(format!("{text:?}: save real ok={real_ok} {} bytes, port ok={port_ok} {} bytes, equal={}", real_bytes.len(), port_bytes.len(), real_bytes == port_bytes));
        }

        for cut in 0..=real_bytes.len() {
            for version in [0x3e, 0x40] {
                let (real_target, port_target) = (primed("fOld(1)", [4, 5, 6]), primed("fOld(1)", [4, 5, 6]));
                io_redirect::begin_replay(real_bytes[..cut].to_vec());
                let real_ret = call_support::real::load_call(real_target, std::ptr::null(), version);
                let real_consumed = io_redirect::replay_position();
                io_redirect::end_replay();
                io_redirect::begin_replay(real_bytes[..cut].to_vec());
                let port_ret = unsafe { CALL_LOAD.hooked()(port_target as *const u32, std::ptr::null(), version) };
                let port_consumed = io_redirect::replay_position();
                io_redirect::end_replay();
                let label = format!("{text:?} cut {cut}/{} v{version:#x}", real_bytes.len());
                if real_ret != port_ret || real_consumed != port_consumed {
                    failures.push(format!("{label}: load real=({real_ret}, {real_consumed}) port=({port_ret}, {port_consumed})"));
                }
                let (real_state, port_state) = (call_support::snapshot(real_target), call_support::snapshot(port_target));
                if real_state != port_state {
                    failures.push(format!("{label}: state real={real_state:?} port={port_state:?}"));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFBEHAVIORSET_RUST_OWNED` - a whole set lifecycle on the port's own storage: construct, name, call-list entries,
/// a parsed call, nested children from `initChild`, `removeHead`, `clear` and destruction. Every string and vector
/// in it is Rust-allocated, so no size class of vanilla's `PoolAlloc` may gain or lose a block and the allocator's
/// own bookkeeping must end where it started.
pub(crate) fn run_bfbehaviorset_rust_owned_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::bfbehaviorset::{construct_set_members, destroy_set_members, BfBehaviorSet, BFBEHAVIORSET_SIZE};
    use crate::bfscriptmgr::{parse_function, RawString};
    use crate::owned_strings::{strvec_push, tracked_count};
    use crate::reimplementation_tests::tests::bfentity::{pool_lengths, warm_pool};

    let test_name = "BFBEHAVIORSET_RUST_OWNED";
    let mut failures: Vec<String> = Vec::new();

    let text = |value: &str| -> (Vec<u8>, RawString) {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        let range = bytes.as_ptr_range();
        let raw = RawString { begin: range.start, end: unsafe { range.end.sub(1) }, capacity: range.end };
        (bytes, raw)
    };

    let mut storage = Box::new([0xcdcd_cdcdu32; BFBEHAVIORSET_SIZE / 4]);
    let base = storage.as_mut_ptr() as u32;
    let set = unsafe { &*(base as *const BfBehaviorSet) };

    warm_pool();
    let (pool_before, tracked_before) = (pool_lengths(), tracked_count());
    unsafe { construct_set_members(base) };
    let fresh = bfbehaviorset_live_support::snapshot(base);
    if !fresh.name.is_empty() || !fresh.current_call.is_empty() || !fresh.list.is_empty() || fresh.has_child || fresh.flags != [1, 0, 0, 0, 0] {
        failures.push(format!("constructed set is not empty: {fresh:?}"));
    }

    let (_keep_root, root) = text("root");
    if !unsafe { set.set_name(&root) } {
        failures.push("setName refused a non-empty name".to_string());
    }
    for line in ["fA(1)", "fB(2)", "fC(3)"] {
        unsafe { strvec_push(base + 0x28, line.as_bytes()) };
    }
    let (_keep_call, call_text) = text("fA(1, 2)");
    unsafe { parse_function(&call_text as *const RawString as *const u32, (base + 0xc) as *const u32) };
    unsafe { set.remove_head() };
    let after_remove = bfbehaviorset_live_support::snapshot(base);
    if after_remove.name != b"root" || after_remove.list != vec![b"fB(2)".to_vec(), b"fC(3)".to_vec()] || !after_remove.call.name.is_empty() {
        failures.push(format!("after removeHead: {after_remove:?}"));
    }
    if after_remove.call.args != vec![b"1".to_vec(), b"2".to_vec()] {
        failures.push(format!("removeHead should leave the call arguments: {:?}", after_remove.call.args));
    }

    for child_name in ["child1", "child2"] {
        let (_keep, name) = text(child_name);
        if !unsafe { set.init_child(0, &name) } {
            failures.push(format!("initChild({child_name}) failed"));
        }
    }
    let chain = bfbehaviorset_live_support::snapshot_chain(base);
    if chain.len() != 3 || chain[0].stack != vec![b"child1".to_vec(), b"child2".to_vec()] || chain[1].name != b"child1" || chain[2].name != b"child2" {
        failures.push(format!("child chain: {chain:?}"));
    }

    unsafe { set.clear() };
    let cleared = bfbehaviorset_live_support::snapshot_chain(base);
    // `killChild` pops one name-stack entry per call whatever the chain's depth (`BFBEHAVIORSET_KILL_CHILD` compares
    // that with vanilla), so `child1` is left on the stack.
    if cleared.len() != 1 || !cleared[0].name.is_empty() || !cleared[0].list.is_empty() || cleared[0].stack != vec![b"child1".to_vec()] || !cleared[0].call.args.is_empty() {
        failures.push(format!("after clear: {cleared:?}"));
    }

    let (_keep_again, again) = text("again");
    unsafe { set.set_name(&again) };
    for line in ["x", "yy"] {
        unsafe { strvec_push(base + 0x28, line.as_bytes()) };
    }
    unsafe { destroy_set_members(base) };

    let (pool_after, tracked_after) = (pool_lengths(), tracked_count());
    if pool_before != pool_after {
        failures.push(format!("vanilla pool changed: before {pool_before:?}, after {pool_after:?}"));
    }
    if tracked_before != tracked_after {
        failures.push(format!("string allocator tracked {tracked_before} buffers before and {tracked_after} after"));
    }
    finish_test(test_name, failures, failure_log)
}
