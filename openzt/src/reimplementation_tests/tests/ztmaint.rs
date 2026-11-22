//! Live checks for `ztmaint.rs`. Each comparison builds a maintenance worker with the real vanilla constructor twice
//! (one for the real body, one for the Rust port), runs the real body on one and the port on the other, and compares
//! return values and the workers' whole memory (heap pointers may differ, shared pointers may not).

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

use openzt_detour::generated::{
    bfentity::UPDATE as BFENTITY_UPDATE,
    standalone::OPERATOR_NEW,
    ztguest::F_ROUTINE,
    ztmaint::{FINISH_TASK, FIND_FENCE_TO_FIX_ON_TILE, IS_TARGETED, IS_TASK_AVAILABLE},
};

use super::ztkeeper::{goals_find_predator, goals_goal_set, install_goals_hook, GOALS_INTERCEPT, GOALS_LOG, GOALS_PREDATOR};
use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::{
        harness::{finish_test, write_success_line},
        io_redirect,
    },
    util::{get_from_memory, save_to_memory},
    ztkeeper::{self, unit_offsets},
    ztmaint::{
        live_support::{self, real},
        task_offsets, ZTMaint, CURRENT_TASK_OFFSET, FIELD_0X294_OFFSET, FIELD_0X298_OFFSET, FIELD_0X2A0_OFFSET, MAINT_SIZE, MAINT_VTABLE_RVA,
        TIMER_OFFSET,
    },
};

const VTABLE_SLOTS: u32 = 204;
/// The game's `rand` state (`DAT_00638060`), advanced by `ZTStaff::initStatusVars` and `BFUnit::idle`.
const RNG_STATE_RVA: u32 = 0x0023_8060;
/// `DAT_00638fa4`: the `ZTMaintTaskPool` list sentinel, swapped for a fake while a test runs.
const TASK_LIST_RVA: u32 = 0x0023_8fa4;
/// `DAT_00639474`: the world's entity list sentinel.
const ENTITY_LIST_RVA: u32 = 0x0023_9474;
/// `DAT_00638004`: head of vanilla's 12-byte small-block freelist.
const FREELIST_12_RVA: u32 = 0x0023_8004;

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn words(address: u32) -> Vec<u32> {
    (0..MAINT_SIZE / 4).map(|i| get_from_memory::<u32>(address + i * 4)).collect()
}

/// Whether two words agree: equal, or both heap pointers neither of which is a shared pointer.
fn equivalent(real: u32, port: u32, shared: &[u32]) -> bool {
    real == port || (real >= 0x10000 && port >= 0x10000 && !shared.contains(&real) && !shared.contains(&port))
}

fn compare_memory(label: &str, real: ZTMaint, port: ZTMaint, shared: &[u32], failures: &mut Vec<String>) {
    for (i, (r, p)) in words(real.0).into_iter().zip(words(port.0)).enumerate() {
        if !equivalent(r, p, shared) {
            failures.push(format!("{label}: +{:#x} real {r:#x}, port {p:#x}", i * 4));
        }
    }
}

fn alloc(size: u32) -> u32 {
    let memory = unsafe { OPERATOR_NEW.original()(size) } as u32;
    unsafe { std::ptr::write_bytes(memory as *mut u8, 0, size as usize) };
    memory
}

/// A circular doubly linked list of `payloads`: nodes are `{next, previous, payload}` (12 bytes, vanilla-allocated so
/// the freelist pushes of the code under test land on real pool memory). Returns the sentinel.
fn make_list(payloads: &[u32]) -> u32 {
    let sentinel = alloc(12);
    save_to_memory(sentinel, sentinel);
    save_to_memory(sentinel + 4, sentinel);
    for &payload in payloads {
        let node = alloc(12);
        let last: u32 = get_from_memory(sentinel + 4);
        save_to_memory(node, sentinel);
        save_to_memory(node + 4, last);
        save_to_memory(node + 8, payload);
        save_to_memory(last, node);
        save_to_memory(sentinel + 4, node);
    }
    sentinel
}

fn list_payloads(sentinel: u32) -> Vec<u32> {
    let mut payloads = Vec::new();
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel && payloads.len() < 64 {
        payloads.push(get_from_memory(node + 8));
        node = get_from_memory(node);
    }
    payloads
}

/// How many nodes were pushed on the 12-byte freelist since its head was `before`.
fn freelist_pushed(before: u32) -> usize {
    let mut node: u32 = get_from_memory(base() + FREELIST_12_RVA);
    let mut count = 0;
    while node != before && node != 0 && count < 64 {
        count += 1;
        node = get_from_memory(node);
    }
    count
}

fn pin_rng(seed: u32) {
    save_to_memory(base() + RNG_STATE_RVA, seed);
}

fn rng() -> u32 {
    get_from_memory(base() + RNG_STATE_RVA)
}

/// `ZTMAINT_DETOURS_ENABLED`: every `ZTMaint` detour reports enabled, so the comparisons below exercise the hooked
/// path rather than silently running vanilla.
pub(crate) fn run_ztmaint_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

/// `ZTMAINT_FIXTURE_LAYOUT_LIVE`: a worker built by the vanilla constructor over a real maintenance type has the
/// `ZTMaint` vtable, `0x294`/`0x298` equal to 2, a 4-bit task vector whose set bits match `isTaskAvailable` and the
/// 16-bit count; the accessors read and write the offsets they name; and the real `initStatusVars` clears `+0x2a0`
/// and reloads the timer from the type without touching `0x294`/`0x298`. That last check ties the named offsets to
/// vanilla's own writes.
pub(crate) fn run_ztmaint_fixture_layout_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_FIXTURE_LAYOUT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let worker = live_support::build_standalone_maint(maint_type);

    let vtable: u32 = get_from_memory(worker.0);
    let expected_vtable = base() + MAINT_VTABLE_RVA;
    if vtable != expected_vtable {
        failures.push(format!("vtable {vtable:#x}, expected {expected_vtable:#x}"));
    }
    for offset in [FIELD_0X294_OFFSET, FIELD_0X298_OFFSET] {
        let value: u32 = get_from_memory(worker.0 + offset);
        if value != 2 {
            failures.push(format!("the constructor left +{offset:#x} at {value}, expected 2"));
        }
    }
    if worker.task_bit_count() != 4 {
        failures.push(format!("the task vector holds {} bits, expected 4", worker.task_bit_count()));
    }
    let mut set_bits = 0u16;
    for index in 0..4 {
        let available = unsafe { IS_TASK_AVAILABLE.original()(worker.0 as *const i32, index) } & 0xff != 0;
        if worker.task_bit(index) != available {
            failures.push(format!("task bit {index} is {}, isTaskAvailable says {available}", worker.task_bit(index)));
        }
        set_bits += available as u16;
    }
    if worker.available_task_count() != set_bits {
        failures.push(format!("available task count {} but {set_bits} bits set", worker.available_task_count()));
    }
    if worker.current_task() != 0 {
        failures.push("the constructor left a current task".to_string());
    }

    worker.set_timer(77);
    worker.set_current_task(0x1234);
    let timer: u32 = get_from_memory(worker.0 + TIMER_OFFSET);
    let task: u32 = get_from_memory(worker.0 + CURRENT_TASK_OFFSET);
    if timer != 77 || task != 0x1234 {
        failures.push(format!("accessors wrote timer {timer}, task {task:#x}"));
    }
    worker.set_current_task(0);
    save_to_memory(worker.0 + FIELD_0X2A0_OFFSET, 0x55u32);
    save_to_memory(worker.0 + FIELD_0X294_OFFSET, 9u32);
    save_to_memory(worker.0 + FIELD_0X298_OFFSET, 9u32);
    let seed = rng();
    real::init_status_vars(worker.0);
    pin_rng(seed);
    let cleared: u32 = get_from_memory(worker.0 + FIELD_0X2A0_OFFSET);
    if cleared != 0 {
        failures.push(format!("real initStatusVars left +0x2a0 at {cleared}"));
    }
    let reload: i32 = get_from_memory(maint_type + 0x1fc);
    if worker.timer() != reload {
        failures.push(format!("real initStatusVars set the timer to {}, the type holds {reload}", worker.timer()));
    }
    for offset in [FIELD_0X294_OFFSET, FIELD_0X298_OFFSET] {
        let value: u32 = get_from_memory(worker.0 + offset);
        if value != 9 {
            failures.push(format!("real initStatusVars changed +{offset:#x} to {value}"));
        }
    }
    live_support::destroy_standalone_maint(worker);
    finish_test(test_name, failures, failure_log)
}

/// `ZTMAINT_F_ROUTINE_LIVE`: the shared `fRoutine` copies the stopped byte into `*done` and runs `BFUnit::idle`; real
/// body, port and the hooked address leave the same `done` byte, result and worker memory for both stopped settings.
/// A `ZTGuest` shares the body; the hooked address is exercised on a worker, which is the same code.
pub(crate) fn run_ztmaint_f_routine_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_F_ROUTINE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![maint_type];
    let seed = rng();
    for stopped in [0u8, 1, 0x80] {
        let (real_worker, port_worker, hooked_worker) =
            (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
        for worker in [real_worker, port_worker, hooked_worker] {
            save_to_memory(worker.0 + unit_offsets::STOPPED, stopped);
        }
        let (mut real_done, mut port_done, mut hooked_done) = (0xaau8, 0xaau8, 0xaau8);
        pin_rng(seed);
        let real_result = real::f_routine(real_worker.0, &mut real_done as *mut u8 as *const bool);
        let real_rng = rng();
        pin_rng(seed);
        let port_result = ZTMaint::f_routine(port_worker.0, &mut port_done as *mut u8 as *const bool);
        let port_rng = rng();
        pin_rng(seed);
        let hooked_result = unsafe { F_ROUTINE.hooked()(hooked_worker.0 as *const u32, &mut hooked_done as *mut u8 as *const bool) };
        pin_rng(seed);
        if real_result != port_result || real_result != hooked_result || !real_result {
            failures.push(format!("stopped {stopped:#x}: results real {real_result}, port {port_result}, hooked {hooked_result}"));
        }
        if real_done != port_done || real_done != hooked_done || real_done != stopped {
            failures.push(format!("stopped {stopped:#x}: done bytes real {real_done:#x}, port {port_done:#x}, hooked {hooked_done:#x}"));
        }
        if real_rng != port_rng {
            failures.push(format!("stopped {stopped:#x}: RNG state real {real_rng:#x}, port {port_rng:#x}"));
        }
        compare_memory(&format!("stopped {stopped:#x}"), real_worker, port_worker, &shared, &mut failures);
        compare_memory(&format!("stopped {stopped:#x} hooked"), real_worker, hooked_worker, &shared, &mut failures);
        for worker in [real_worker, port_worker, hooked_worker] {
            live_support::destroy_standalone_maint(worker);
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// While set, `BFEntity::update` (the tail `ZTMaint::update` ends with) is recorded instead of run: the real one
/// animates and moves the unit, which a bare fixture cannot take.
pub(super) static UPDATE_INTERCEPT: AtomicU32 = AtomicU32::new(0);
pub(super) static UPDATE_LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());

#[openzt_detour_macro::detour_mod]
mod entity_update_hook {
    use super::*;

    #[detour(BFENTITY_UPDATE)]
    unsafe extern "thiscall" fn entity_update(this: *const u32, elapsed: u32) {
        if UPDATE_INTERCEPT.load(Ordering::SeqCst) != 0 {
            UPDATE_LOG.lock().unwrap().push(elapsed);
        } else {
            unsafe { BFENTITY_UPDATE_DETOUR.call(this, elapsed) }
        }
    }
}

/// Installs the `BFEntity::update` hook once; other live tests share it.
pub(super) fn install_entity_update_hook() -> Result<(), String> {
    static HOOK_INSTALLED: std::sync::Once = std::sync::Once::new();
    let mut result = Ok(());
    HOOK_INSTALLED.call_once(|| {
        if let Err(e) = unsafe { entity_update_hook::init_detours() } {
            result = Err(format!("could not install the BFEntity::update hook: {e:?}"));
        }
    });
    result
}

/// `ZTMAINT_UPDATE_LIVE`: `update` over every combination of starting timer (above, at and below the elapsed time) and
/// elapsed time, with the two staff pointer pairs set and the type present or absent, comparing the worker's whole
/// memory and the elapsed time handed to `BFEntity::update` (recorded by a test hook).
pub(crate) fn run_ztmaint_update_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_UPDATE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_entity_update_hook() {
        failures.push(message);
    }
    UPDATE_INTERCEPT.store(1, Ordering::SeqCst);
    let shared = vec![maint_type];
    let (real_worker, port_worker) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
    let mut reloaded = false;
    // The fields the loop below overwrites, restored before the fixtures are destroyed: vanilla's destructor frees
    // the staff pointer pairs, which must never be Rust memory.
    let saved: Vec<[u32; 5]> = [real_worker, port_worker]
        .iter()
        .map(|worker| {
            [0x270u32, 0x274, 0x27c, 0x280, unit_offsets::ENTITY_TYPE].map(|offset| get_from_memory::<u32>(worker.0 + offset))
        })
        .collect();
    // Real memory behind the staff pointer pairs, so whatever vanilla does with them cannot fault.
    let buffers: [[Vec<u32>; 2]; 2] = [[vec![0x11; 8], vec![0x22; 8]], [vec![0x11; 8], vec![0x22; 8]]];
    for type_present in [true, false] {
        for timer in [1000i32, 11, 10, 5, 1, 0, -7] {
            for elapsed in [0u32, 1, 5, 10, 400] {
                let mut logs = Vec::new();
                for (pole, worker) in [real_worker, port_worker].into_iter().enumerate() {
                    save_to_memory(worker.0 + unit_offsets::ENTITY_TYPE, if type_present { maint_type } else { 0 });
                    let first = buffers[pole][0].as_ptr() as u32;
                    let second = buffers[pole][1].as_ptr() as u32;
                    save_to_memory(worker.0 + 0x270, first);
                    save_to_memory(worker.0 + 0x274, first + 16);
                    save_to_memory(worker.0 + 0x27c, second);
                    save_to_memory(worker.0 + 0x280, second + 8);
                    worker.set_timer(timer);
                    UPDATE_LOG.lock().unwrap().clear();
                    if pole == 0 {
                        real::update(worker.0, elapsed);
                    } else {
                        worker.update(elapsed);
                    }
                    logs.push(UPDATE_LOG.lock().unwrap().clone());
                }
                let case = format!("timer {timer}, elapsed {elapsed}, type {type_present}");
                if logs[0] != logs[1] || logs[0] != vec![elapsed] {
                    failures.push(format!("{case}: BFEntity::update calls real {:?}, port {:?}", logs[0], logs[1]));
                }
                compare_memory(&case, real_worker, port_worker, &shared, &mut failures);
                for (begin, end) in [(0x270u32, 0x274u32), (0x27c, 0x280)] {
                    let lengths: Vec<u32> = [real_worker, port_worker]
                        .iter()
                        .map(|worker| get_from_memory::<u32>(worker.0 + end).wrapping_sub(get_from_memory::<u32>(worker.0 + begin)))
                        .collect();
                    if lengths[0] != lengths[1] {
                        failures.push(format!("{case}: pair at +{begin:#x} has length real {}, port {}", lengths[0], lengths[1]));
                    }
                }
                if type_present && timer - (elapsed as i32) < 1 {
                    reloaded = true;
                    let reload: i32 = get_from_memory(maint_type + 0x1fc);
                    if port_worker.timer() != reload {
                        failures.push(format!("{case}: the timer reloaded to {}, the type holds {reload}", port_worker.timer()));
                    }
                }
            }
        }
    }
    if !reloaded {
        failures.push("no case reached the timer reload".to_string());
    }
    // The hooked address runs the port.
    UPDATE_LOG.lock().unwrap().clear();
    save_to_memory(port_worker.0 + unit_offsets::ENTITY_TYPE, maint_type);
    port_worker.set_timer(3);
    unsafe { openzt_detour::generated::ztmaint::UPDATE.hooked()(port_worker.0 as *const i32, 9) };
    if UPDATE_LOG.lock().unwrap().as_slice() != [9] {
        failures.push("the hooked address did not run update".to_string());
    }
    UPDATE_INTERCEPT.store(0, Ordering::SeqCst);
    for (worker, values) in [real_worker, port_worker].into_iter().zip(saved) {
        for (offset, value) in [0x270u32, 0x274, 0x27c, 0x280, unit_offsets::ENTITY_TYPE].into_iter().zip(values) {
            save_to_memory(worker.0 + offset, value);
        }
    }
    live_support::destroy_standalone_maint(real_worker);
    live_support::destroy_standalone_maint(port_worker);
    finish_test(test_name, failures, failure_log)
}

static VALID_ANSWER: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_valid(_cog: u32) -> u32 {
    VALID_ANSWER.load(Ordering::SeqCst)
}

/// `ZTMAINT_UPDATE_GOALS_LIVE`: `updateGoals` over the pending-predator flag, the cog goal's current goal id, the
/// predator result, the cog validity (the real `valid` runs on the embedded cog goal; the test swaps in one that
/// answers 0 or 1) and the goal flag, through recording fakes for the virtual calls. `ZTUnit::updateGoals` is
/// recorded by the keeper test's hook. Compares the exact call sequence and the worker's memory.
pub(crate) fn run_ztmaint_update_goals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_UPDATE_GOALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_goals_hook() {
        failures.push(message);
    }
    GOALS_INTERCEPT.store(1, Ordering::SeqCst);
    let shared = vec![maint_type];
    let real_vtable_address = base() + MAINT_VTABLE_RVA;
    let mut fake_vtable: Vec<u32> = (0..VTABLE_SLOTS).map(|i| get_from_memory::<u32>(real_vtable_address + i * 4)).collect();
    fake_vtable[0x2fc / 4] = goals_find_predator as *const () as usize as u32;
    fake_vtable[0x1d8 / 4] = goals_goal_set as *const () as usize as u32;
    let (real_worker, port_worker) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
    for worker in [real_worker, port_worker] {
        save_to_memory(worker.0, fake_vtable.as_ptr() as u32);
    }
    // The embedded cog goal's vtable, copied with slot 0 (`valid`) replaced by an answer the test controls.
    let original_cog_vtable: u32 = get_from_memory(real_worker.0 + unit_offsets::COG_GOAL);
    let mut cog_vtable: Vec<u32> = (0..16).map(|i| get_from_memory::<u32>(original_cog_vtable + i * 4)).collect();
    cog_vtable[0] = fake_valid as *const () as usize as u32;
    for worker in [real_worker, port_worker] {
        save_to_memory(worker.0 + unit_offsets::COG_GOAL, cog_vtable.as_ptr() as u32);
    }
    let seed = rng();
    let mut sequences = std::collections::BTreeSet::new();
    let mut cases = 0;
    for pending in [0u8, 1] {
        for goal in [0x75u32, 0x6e] {
            for predator in [0u32, 0x1234] {
                for valid in [0u32, 1] {
                    for goal_flag in [0u8, 1] {
                        cases += 1;
                        GOALS_PREDATOR.store(predator, Ordering::SeqCst);
                        VALID_ANSWER.store(valid, Ordering::SeqCst);
                        let mut logs = Vec::new();
                        for (pole, worker) in [real_worker, port_worker].into_iter().enumerate() {
                            save_to_memory(worker.0 + unit_offsets::PREDATOR_CHECK_PENDING, pending);
                            save_to_memory(worker.0 + unit_offsets::COG_GOAL + 0x14, goal);
                            save_to_memory(worker.0 + unit_offsets::COG_GOAL_FLAG, goal_flag);
                            GOALS_LOG.lock().unwrap().clear();
                            pin_rng(seed);
                            if pole == 0 {
                                real::update_goals(worker.0);
                            } else {
                                worker.update_goals();
                            }
                            logs.push(GOALS_LOG.lock().unwrap().clone());
                        }
                        let case = format!("pending {pending}, goal {goal:#x}, predator {predator:#x}, valid {valid}, goal flag {goal_flag}");
                        if logs[0] != logs[1] {
                            failures.push(format!("{case}: real calls {:?}, port calls {:?}", logs[0], logs[1]));
                        }
                        sequences.insert(format!("{:?}", logs[0]));
                        // `ZTUnit::updateGoals` is recorded (kind 6) at the end of every path.
                        compare_memory(&case, real_worker, port_worker, &shared, &mut failures);
                    }
                }
            }
        }
    }
    for (label, wanted) in [("predator goal", "[4, 117, 3, 0]"), ("fallback goal", "[4, 120, 4, 0]")] {
        if !sequences.iter().any(|sequence| sequence.contains(wanted)) {
            failures.push(format!("no case reached the {label} ({wanted}) in {cases} cases"));
        }
    }
    if !sequences.iter().any(|sequence| sequence == "[[6, 0, 0, 0]]") {
        failures.push("no case ended with only ZTUnit::updateGoals".to_string());
    }
    GOALS_LOG.lock().unwrap().clear();
    unsafe { openzt_detour::generated::ztmaint::UPDATE_GOALS.hooked()(port_worker.0 as *const i32) };
    if GOALS_LOG.lock().unwrap().is_empty() {
        failures.push("the hooked address made no calls".to_string());
    }
    for worker in [real_worker, port_worker] {
        save_to_memory(worker.0, real_vtable_address);
        save_to_memory(worker.0 + unit_offsets::COG_GOAL, original_cog_vtable);
    }
    live_support::destroy_standalone_maint(real_worker);
    live_support::destroy_standalone_maint(port_worker);
    pin_rng(seed);
    GOALS_INTERCEPT.store(0, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}

/// `ZTMAINT_INIT_STATUS_VARS_LIVE`: `initStatusVars` on twin fixtures whose `+0x2a0`, `+0x64`, `+0x68` and timer are
/// dirtied first, with and without a type, comparing the whole worker; the RNG is pinned because `ZTStaff` draws two
/// numbers.
pub(crate) fn run_ztmaint_init_status_vars_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_INIT_STATUS_VARS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![maint_type, maint_type + 0x80];
    let seed = rng();
    let (real_worker, port_worker) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
    // A worker with no type is not exercised: `ZTStaff::initStatusVars` reads the type before the maintenance
    // override's own null check is reached.
    for type_present in [true] {
        for worker in [real_worker, port_worker] {
            save_to_memory(worker.0 + unit_offsets::ENTITY_TYPE, if type_present { maint_type } else { 0 });
            save_to_memory(worker.0 + FIELD_0X2A0_OFFSET, 0x1234_5678u32);
            save_to_memory(worker.0 + unit_offsets::SELF_POINTER, 0xdead_beefu32);
            save_to_memory(worker.0 + unit_offsets::TYPE_BLOCK_POINTER, 0xdead_beefu32);
            worker.set_timer(-5);
        }
        pin_rng(seed);
        real::init_status_vars(real_worker.0);
        pin_rng(seed);
        port_worker.init_status_vars();
        pin_rng(seed);
        compare_memory(&format!("type {type_present}"), real_worker, port_worker, &shared, &mut failures);
        if type_present {
            let link: u32 = get_from_memory(port_worker.0 + unit_offsets::TYPE_BLOCK_POINTER);
            let expected = if get_from_memory::<u32>(maint_type + 0x84) != 0 { maint_type + 0x80 } else { 0 };
            if link != expected {
                failures.push(format!("+0x68 is {link:#x}, expected {expected:#x}"));
            }
        }
    }
    live_support::destroy_standalone_maint(real_worker);
    live_support::destroy_standalone_maint(port_worker);
    finish_test(test_name, failures, failure_log)
}

/// `ZTMAINT_TASK_AVAILABLE_LIVE`: `isTaskAvailable` for indices 0 to 40 (past the end of the type's bit vector) with the
/// type present and absent, real against port and the hooked address (low byte only: vanilla leaves the rest undefined).
pub(crate) fn run_ztmaint_task_available_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_TASK_AVAILABLE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let worker = live_support::build_standalone_maint(maint_type);
    let mut trues = 0;
    for type_present in [true, false] {
        save_to_memory(worker.0 + unit_offsets::ENTITY_TYPE, if type_present { maint_type } else { 0 });
        for index in 0..40u32 {
            let real_result = real::is_task_available(worker.0, index) & 0xff != 0;
            let port_result = worker.is_task_available(index);
            let hooked = unsafe { IS_TASK_AVAILABLE.hooked()(worker.0 as *const i32, index) } & 0xff != 0;
            if real_result != port_result || real_result != hooked {
                failures.push(format!("type {type_present}, index {index}: real {real_result}, port {port_result}, hooked {hooked}"));
            }
            trues += real_result as u32;
        }
    }
    if trues == 0 {
        failures.push("no index was available, so the bit test was never true".to_string());
    }
    save_to_memory(worker.0 + unit_offsets::ENTITY_TYPE, maint_type);
    live_support::destroy_standalone_maint(worker);
    finish_test(test_name, failures, failure_log)
}

/// Builds a fake pool list holding `payloads` and swaps it into `DAT_00638fa4`; returns the sentinel and the original.
fn swap_task_list(payloads: &[u32]) -> (u32, u32) {
    let sentinel = make_list(payloads);
    let original: u32 = get_from_memory(base() + TASK_LIST_RVA);
    save_to_memory(base() + TASK_LIST_RVA, sentinel);
    (sentinel, original)
}

/// `ZTMAINT_TASKS_LIVE`: `setNewTask` (no current task; a current task held by zero, one and two pool nodes among
/// others) and `finishTask` (through the hooked fastcall address), real against port. The pool list is a fake swapped
/// into the global; compared: the surviving payloads, the worker's task and how many nodes went on the freelist. Then
/// `removeFromMap` both ways: `false` detaches the task without freeing it (the task's worker field clears), `true`
/// finishes it.
pub(crate) fn run_ztmaint_tasks_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_TASKS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![maint_type];
    let other_a = 0x4000_0010u32;
    let other_b = 0x4000_0020u32;
    // Layout of each case: payload sequence with `T` standing for the worker's own task.
    let layouts: [(&str, &[Option<u32>]); 4] = [
        ("task not queued", &[Some(other_a), Some(other_b)]),
        ("queued once", &[Some(other_a), None, Some(other_b)]),
        ("queued twice", &[None, Some(other_a), None]),
        ("empty list", &[]),
    ];
    for (label, layout) in layouts {
        for new_task in [0u32, 0x5555] {
            let mut results = Vec::new();
            for pole in 0..2 {
                let worker = live_support::build_standalone_maint(maint_type);
                let task = alloc(0x10);
                save_to_memory(task + task_offsets::WORKER, worker.0);
                worker.set_current_task(task);
                let payloads: Vec<u32> = layout.iter().map(|entry| entry.unwrap_or(task)).collect();
                let (sentinel, original) = swap_task_list(&payloads);
                let before: u32 = get_from_memory(base() + FREELIST_12_RVA);
                if pole == 0 {
                    real::set_new_task(worker.0, new_task);
                } else {
                    worker.set_new_task(new_task);
                }
                let remaining: Vec<u32> = list_payloads(sentinel).into_iter().map(|p| if p == task { 1 } else { p }).collect();
                let pushed = freelist_pushed(before);
                results.push((remaining, worker.current_task(), pushed));
                save_to_memory(base() + TASK_LIST_RVA, original);
                live_support::destroy_standalone_maint(worker);
            }
            let case = format!("{label}, new task {new_task:#x}");
            if results[0] != results[1] {
                failures.push(format!("setNewTask {case}: real {:?}, port {:?}", results[0], results[1]));
            }
            let expected_queued = layout.iter().filter(|entry| entry.is_none()).count();
            if results[1].2 != expected_queued || results[1].1 != new_task {
                failures.push(format!("setNewTask {case}: port freed {} nodes (expected {expected_queued}), task {:#x}", results[1].2, results[1].1));
            }
        }
    }
    // No current task: the new one is simply stored and the list is untouched.
    for pole in 0..2 {
        let worker = live_support::build_standalone_maint(maint_type);
        let (sentinel, original) = swap_task_list(&[other_a]);
        if pole == 0 {
            real::set_new_task(worker.0, 0x777);
        } else {
            worker.set_new_task(0x777);
        }
        if worker.current_task() != 0x777 || list_payloads(sentinel) != vec![other_a] {
            failures.push(format!("setNewTask without a task, pole {pole}: task {:#x}, list {:?}", worker.current_task(), list_payloads(sentinel)));
        }
        worker.set_current_task(0);
        save_to_memory(base() + TASK_LIST_RVA, original);
        live_support::destroy_standalone_maint(worker);
    }
    // finishTask through the hooked fastcall address.
    {
        let worker = live_support::build_standalone_maint(maint_type);
        let task = alloc(0x10);
        worker.set_current_task(task);
        let (sentinel, original) = swap_task_list(&[task]);
        unsafe { FINISH_TASK.hooked()(worker.0 as *const std::ffi::c_void) };
        if worker.current_task() != 0 || !list_payloads(sentinel).is_empty() {
            failures.push(format!("hooked finishTask: task {:#x}, list {:?}", worker.current_task(), list_payloads(sentinel)));
        }
        save_to_memory(base() + TASK_LIST_RVA, original);
        live_support::destroy_standalone_maint(worker);
    }
    // removeFromMap both ways.
    for param in [false, true] {
        let (real_worker, port_worker) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
        let mut observed = Vec::new();
        for (pole, worker) in [real_worker, port_worker].into_iter().enumerate() {
            let task = alloc(0x10);
            save_to_memory(task + task_offsets::WORKER, worker.0);
            worker.set_current_task(task);
            let (sentinel, original) = swap_task_list(&[task]);
            if pole == 0 {
                real::remove_from_map(worker.0, param);
            } else {
                worker.remove_from_map(param);
            }
            observed.push((worker.current_task(), list_payloads(sentinel).len(), if param { 0 } else { get_from_memory::<u32>(task + task_offsets::WORKER) }));
            save_to_memory(base() + TASK_LIST_RVA, original);
        }
        if observed[0] != observed[1] || observed[1].0 != 0 {
            failures.push(format!("removeFromMap({param}): real {:?}, port {:?}", observed[0], observed[1]));
        }
        compare_memory(&format!("removeFromMap({param})"), real_worker, port_worker, &shared, &mut failures);
        live_support::destroy_standalone_maint(real_worker);
        live_support::destroy_standalone_maint(port_worker);
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTMAINT_IS_TARGETED_LIVE`: `isTargeted` against a fake entity list holding maintenance workers heading for
/// different tiles, a keeper (not a worker), a null entry and the asking worker itself, for tiles that match one
/// other worker, only the asker, only the keeper, or nothing; real, port and the hooked address.
pub(crate) fn run_ztmaint_is_targeted_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_IS_TARGETED_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let keeper = ztkeeper::live_support::find_keeper_type().map(ztkeeper::live_support::build_standalone_keeper);
    let asker = live_support::build_standalone_maint(maint_type);
    let (other_one, other_two) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
    save_to_memory(asker.0 + unit_offsets::TARGET_TILE, 0x100u32);
    save_to_memory(other_one.0 + unit_offsets::TARGET_TILE, 0x200u32);
    save_to_memory(other_two.0 + unit_offsets::TARGET_TILE, 0x300u32);
    let mut entries = vec![0, asker.0, other_one.0, other_two.0];
    if let Some(keeper) = keeper {
        save_to_memory(keeper.0 + unit_offsets::TARGET_TILE, 0x400u32);
        entries.push(keeper.0);
    }
    let sentinel = make_list(&entries);
    let original: u32 = get_from_memory(base() + ENTITY_LIST_RVA);
    save_to_memory(base() + ENTITY_LIST_RVA, sentinel);
    let mut trues = 0;
    for tile in [0x100u32, 0x200, 0x300, 0x400, 0x500] {
        for worker in [asker, other_one] {
            let real_result = real::is_targeted(worker.0, tile) & 0xff != 0;
            let port_result = worker.is_targeted(tile);
            let hooked = unsafe { IS_TARGETED.hooked()(worker.0 as *const i32, tile as i32) } & 0xff != 0;
            if real_result != port_result || real_result != hooked {
                failures.push(format!("tile {tile:#x}, asker {:#x}: real {real_result}, port {port_result}, hooked {hooked}", worker.0));
            }
            trues += real_result as u32;
        }
    }
    if trues == 0 {
        failures.push("no query was answered true".to_string());
    }
    save_to_memory(base() + ENTITY_LIST_RVA, original);
    live_support::destroy_standalone_maint(asker);
    live_support::destroy_standalone_maint(other_one);
    live_support::destroy_standalone_maint(other_two);
    if let Some(keeper) = keeper {
        ztkeeper::live_support::destroy_standalone_keeper(keeper);
    }
    finish_test(test_name, failures, failure_log)
}

/// Bytes of worker data `load` consumes before `ZTStaff::load` for save version `version` and a stream storing `stored`
/// task bits.
fn maint_load_bytes(version: u32, stored: usize) -> usize {
    let mut length = 0;
    if version > 0x22 {
        length += 4;
    }
    if version > 0x5b {
        length += 4 + 4 + 2 + 4 + stored;
    }
    length
}

fn run_load(worker: ZTMaint, bytes: Vec<u8>, version: u32, real_body: bool) -> (bool, usize) {
    io_redirect::begin_replay(bytes);
    let result = if real_body { real::load(worker.0, std::ptr::null(), version) } else { worker.load(std::ptr::null(), version) };
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed)
}

/// `ZTMAINT_SAVE_LOAD_LIVE`: `save` writes the same bytes and result as the real body for a dirtied worker (the bytes
/// are `+0x2a0`, `+0x298`, `+0x294`, the 16-bit count, the bit count, one byte per bit, then the `ZTStaff` data); `load` is
/// compared over the version boundaries (`0x22`, `0x23`, `0x5b`, `0x5c`, `0x5d`) with streams that hold exactly the
/// worker's data (so the `ZTStaff` read finds nothing left), one byte short, and with 3 / 4 stored task bits
/// against the worker's 4-bit vector (a longer stream would leave bytes for `ZTStaff::load`, which is not exercised), comparing result, bytes consumed and the whole worker. A last round trip loads
/// a real save.
pub(crate) fn run_ztmaint_save_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_SAVE_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![maint_type];
    let dirty = |worker: ZTMaint| {
        save_to_memory(worker.0 + FIELD_0X2A0_OFFSET, 0x0102_0304u32);
        save_to_memory(worker.0 + FIELD_0X298_OFFSET, 0x1112_1314u32);
        save_to_memory(worker.0 + FIELD_0X294_OFFSET, 0x2122_2324u32);
        save_to_memory(worker.0 + 0x2bc, 0x3132u16);
        worker.set_task_bit(0, true);
        worker.set_task_bit(1, false);
        worker.set_task_bit(2, true);
        worker.set_task_bit(3, true);
    };

    let (real_worker, port_worker) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
    dirty(real_worker);
    dirty(port_worker);
    io_redirect::begin_capture();
    let real_ok = real::save(real_worker.0, std::ptr::null());
    let real_bytes = io_redirect::end_capture();
    io_redirect::begin_capture();
    let port_ok = port_worker.save(std::ptr::null());
    let port_bytes = io_redirect::end_capture();
    if real_ok != port_ok {
        failures.push(format!("save: real returned {real_ok}, port {port_ok}"));
    }
    if real_bytes != port_bytes {
        failures.push(format!("save: real wrote {} bytes, port {}, or the bytes differ", real_bytes.len(), port_bytes.len()));
    }
    let expected_prefix = [0x04u8, 0x03, 0x02, 0x01, 0x14, 0x13, 0x12, 0x11, 0x24, 0x23, 0x22, 0x21, 0x32, 0x31, 4, 0, 0, 0, 1, 0, 1, 1];
    if !port_bytes.starts_with(&expected_prefix) {
        failures.push(format!("save: leading bytes {:02x?}, expected {:02x?}", &port_bytes[..port_bytes.len().min(22)], expected_prefix));
    }
    live_support::destroy_standalone_maint(real_worker);
    live_support::destroy_standalone_maint(port_worker);

    for version in [0x22_u32, 0x23, 0x5b, 0x5c, 0x5d] {
        for stored in [3usize, 4] {
            if version <= 0x5b && stored != 4 {
                continue;
            }
            let length = maint_load_bytes(version, stored);
            let mut stream: Vec<u8> = Vec::new();
            if version > 0x22 {
                stream.extend_from_slice(&0x0a0b_0c0du32.to_le_bytes());
            }
            if version > 0x5b {
                stream.extend_from_slice(&0x1112_1314u32.to_le_bytes());
                stream.extend_from_slice(&0x2122_2324u32.to_le_bytes());
                stream.extend_from_slice(&3u16.to_le_bytes());
                stream.extend_from_slice(&(stored as u32).to_le_bytes());
                stream.extend((0..stored).map(|i| (i % 2) as u8));
            }
            debug_assert_eq!(stream.len(), length);
            let mut streams = vec![("exact", stream.clone())];
            if length > 0 {
                streams.push(("short", stream[..length - 1].to_vec()));
            }
            for (label, bytes) in streams {
                let (real_worker, port_worker) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
                let (real_result, real_consumed) = run_load(real_worker, bytes.clone(), version, true);
                let (port_result, port_consumed) = run_load(port_worker, bytes, version, false);
                let case = format!("load v{version:#x} stored {stored} ({label})");
                if real_result != port_result {
                    failures.push(format!("{case}: real returned {real_result}, port {port_result}"));
                }
                if real_consumed != port_consumed {
                    failures.push(format!("{case}: real consumed {real_consumed} bytes, port {port_consumed}"));
                }
                compare_memory(&case, real_worker, port_worker, &shared, &mut failures);
                live_support::destroy_standalone_maint(real_worker);
                live_support::destroy_standalone_maint(port_worker);
            }
        }
    }

    let source = live_support::build_standalone_maint(maint_type);
    dirty(source);
    io_redirect::begin_capture();
    real::save(source.0, std::ptr::null());
    let saved = io_redirect::end_capture();
    live_support::destroy_standalone_maint(source);
    let (real_worker, port_worker) = (live_support::build_standalone_maint(maint_type), live_support::build_standalone_maint(maint_type));
    let (real_result, real_consumed) = run_load(real_worker, saved.clone(), 0x5d, true);
    let (port_result, port_consumed) = run_load(port_worker, saved, 0x5d, false);
    if real_result != port_result || real_consumed != port_consumed {
        failures.push(format!("round trip: real {real_result}/{real_consumed}, port {port_result}/{port_consumed}"));
    }
    compare_memory("round trip", real_worker, port_worker, &shared, &mut failures);
    if port_worker.task_bit(0) != true || port_worker.task_bit(1) != false || port_worker.task_bit(2) != true || port_worker.task_bit(3) != true {
        failures.push("round trip: the port did not restore the task bits".to_string());
    }
    live_support::destroy_standalone_maint(real_worker);
    live_support::destroy_standalone_maint(port_worker);
    finish_test(test_name, failures, failure_log)
}

/// `ZTMAINT_FIND_FENCE_LIVE`: `findFenceToFixOnTile` over every live tile (up to 200x200): the returned direction and
/// the wear written through the out pointer, real against port, plus the hooked address on a sample. A zoo with no
/// fence worth fixing only exercises the empty result, which is logged as the coverage gap it is.
pub(crate) fn run_ztmaint_find_fence_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_FIND_FENCE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let world = globals().ztworldmgr();
    let mut mismatches = 0;
    let mut found = 0;
    // Interior tiles only: vanilla dereferences a missing neighbour on the map edge and faults.
    for y in 1..world.map_y_size.min(200) - 1 {
        for x in 1..world.map_x_size.min(200) - 1 {
            let tile = world.get_tile_ptr(x, y);
            let (mut real_wear, mut port_wear, mut hooked_wear) = (0xabcdi32, 0xabcdi32, 0xabcdi32);
            let real_direction = real::find_fence_to_fix_on_tile(tile, &mut real_wear as *mut i32 as u32);
            let port_direction = crate::ztmaint::find_fence_to_fix_on_tile(tile, &mut port_wear as *mut i32 as u32);
            if real_direction != port_direction || real_wear != port_wear {
                mismatches += 1;
                if mismatches <= 10 {
                    failures.push(format!("tile ({x},{y}): real {real_direction:#x}/{real_wear}, port {port_direction:#x}/{port_wear}"));
                }
            }
            found += (real_direction != 0xffff_ffff) as u32;
            if x == y && x % 17 == 0 {
                let hooked = unsafe { FIND_FENCE_TO_FIX_ON_TILE.hooked()(tile as *const u32, &mut hooked_wear as *mut i32 as *const i32) };
                if hooked != real_direction || hooked_wear != real_wear {
                    failures.push(format!("tile ({x},{y}): hooked {hooked:#x}/{hooked_wear}, real {real_direction:#x}/{real_wear}"));
                }
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    tracing::info!("{test_name}: {found} tiles had a fence worth fixing");
    finish_test(test_name, failures, failure_log)
}

static POOL_EVENT_CALLS: Mutex<Vec<[u32; 3]>> = Mutex::new(Vec::new());

extern "thiscall" fn fake_pool_get_events(_this: u32, a: u32, b: u32, c: u32) {
    POOL_EVENT_CALLS.lock().unwrap().push([a, b, c]);
}

/// `ZTMAINT_TASK_POOL_LIVE`: `ZTMaintTaskPool::getEvents` forwards `(out, 0, 0x4d)` to its own vtable `+0xc`; `clear` on a
/// fake pool of three tasks (one with no worker, one whose worker holds that task, one whose worker holds another
/// task) releases the workers' tasks as vanilla does, leaves the list empty and self-linked, and pushes every list
/// node on the freelist; real against port.
pub(crate) fn run_ztmaint_task_pool_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMAINT_TASK_POOL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(maint_type) = live_support::find_maint_type() else {
        failures.push("the live world holds no maintenance-worker type".to_string());
        return finish_test(test_name, failures, failure_log);
    };

    let mut fake_pool_vtable = vec![0u32; 8];
    fake_pool_vtable[3] = fake_pool_get_events as *const () as usize as u32;
    let pool_for_events = vec![fake_pool_vtable.as_ptr() as u32, 0];
    for pole in 0..2 {
        POOL_EVENT_CALLS.lock().unwrap().clear();
        if pole == 0 {
            real::pool_get_events(pool_for_events.as_ptr() as u32, 0x77);
        } else {
            unsafe { openzt_detour::generated::ztmainttaskpool::GET_EVENTS.hooked()(pool_for_events.as_ptr() as *const i32, 0x77) };
        }
        if POOL_EVENT_CALLS.lock().unwrap().as_slice() != [[0x77, 0, 0x4d]] {
            failures.push(format!("getEvents pole {pole}: calls {:?}", POOL_EVENT_CALLS.lock().unwrap()));
        }
    }

    let mut observed = Vec::new();
    for pole in 0..2 {
        let worker_holding = live_support::build_standalone_maint(maint_type);
        let worker_other = live_support::build_standalone_maint(maint_type);
        let tasks = [alloc(0x10), alloc(0x10), alloc(0x10)];
        let other_task = alloc(0x10);
        save_to_memory(tasks[1] + task_offsets::WORKER, worker_holding.0);
        worker_holding.set_current_task(tasks[1]);
        save_to_memory(tasks[2] + task_offsets::WORKER, worker_other.0);
        worker_other.set_current_task(other_task);
        save_to_memory(other_task + task_offsets::WORKER, worker_other.0);
        let sentinel = make_list(&tasks);
        let pool = [0u32, sentinel];
        let before: u32 = get_from_memory(base() + FREELIST_12_RVA);
        if pole == 0 {
            real::pool_clear(pool.as_ptr() as u32);
        } else {
            unsafe { openzt_detour::generated::ztmainttaskpool::CLEAR.hooked()(pool.as_ptr() as *const std::ffi::c_void) };
        }
        let empty = get_from_memory::<u32>(sentinel) == sentinel && get_from_memory::<u32>(sentinel + 4) == sentinel;
        observed.push((empty, freelist_pushed(before), worker_holding.current_task(), worker_other.current_task(), get_from_memory::<u32>(other_task + task_offsets::WORKER)));
        live_support::destroy_standalone_maint(worker_holding);
        live_support::destroy_standalone_maint(worker_other);
    }
    if observed[0] != observed[1] || observed[1] != (true, 3, 0, 0, 0) {
        failures.push(format!("clear: real {:?}, port {:?}, expected (true, 3, 0, 0, 0)", observed[0], observed[1]));
    }
    finish_test(test_name, failures, failure_log)
}
