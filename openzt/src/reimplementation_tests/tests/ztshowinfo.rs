//! Compares real vanilla `ZTShowInfo` methods against the Rust reimplementations in production file
//! `openzt/src/ztshowinfo.rs`: the detour-wiring check, then comparison tests split by real-dependency
//! availability - the pending-scripts-tree-only groups (status predicates, accumulators) run in
//! `always_late_tests`, while the groups needing a live, `run_load_live_zoo`-populated global
//! (`GLOBAL_ZTWorldMgr` for the keeper predicates, `GLOBAL_ZTAIMgr` for the schedule/frequency pair)
//! run in `live_zoo_tests` - see `ztshowinfo.rs`'s own module doc comment for the per-method reasons.
//! `ZTSHOWINFO_STANDALONE_ROUNDTRIP` (Stage 12) is the one exception to the "real vs. rust" framing above -
//! there is no rust pole for the constructors/destructor, so it instead pins the real ctor's own defaults
//! and confirms the copy ctor's independent allocation; see that test's own doc comment.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::standalone::OPERATOR_NEW;
use openzt_detour::generated::ztshowinfo::{
    ADD_SHOW, ADD_UNIT, ADD_UNIT_TO_LIST, CHECK_UNIT, CLEANUP_EVENTS, CREATE_DEFAULT_SCRIPT, ENTER_NEW_MONTH, GATHER_UNITS, GET_EVENTS,
    GET_NUM_UNITS, GET_SCHEDULED_SHOW_KEEPER_TYPE, GET_SCHEDULED_SHOW_SCRIPT, GET_SHOW_UNIT_LIST, HAS_KEEPER, INCREMENT_ATTENDANCE,
    INCREMENT_RECEIPTS, IS_READY, IS_STARTED, IS_STOPPED, LISTEN, LOAD, NEEDS_KEEPER, RECALCULATE_SCHEDULE, REMOVE_SHOW, REMOVE_UNIT,
    SAVE, SEND_EVENT, SET_SHOW_FREQUENCY, SET_SHOW_INFO_ID, UPDATE, UPDATE_FROM_LOAD,
};

use crate::globals::globals;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, mut_from_memory, save_to_memory};
use crate::ztshow::{self, call_entity_vtable_u32_noargs, find_or_insert_pending_script_node, live_support as ztshow_live_support};
use crate::ztshowinfo::{
    add_show, add_unit_to_list, clear_pending_script_tree, increment_attendance, increment_receipts,
    live_support as ztshowinfo_live_support, remove_unit,
};
use crate::ztshowmgr::ZTShowMgr;
use crate::ztshowstate;

use super::ztshow::{find_real_non_trick_eligible_unit, find_real_show_tank_habitat, find_real_trick_eligible_unit};

/// `ZTSHOWINFO_DETOURS_ENABLED` - wiring check, same rationale as `ztshowstate`'s own
/// `ZTSHOWSTATE_DETOURS_ENABLED` (see that test's own doc comment): catches `ztshowinfo::init()` going
/// missing from `reimplementation_tests::init()`'s own list before it can produce a silent false
/// positive in the comparison tests below.
pub(crate) fn run_ztshowinfo_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_DETOURS_ENABLED";
    let mut disabled: Vec<&'static str> = Vec::new();
    for (name, enabled) in ztshowinfo_live_support::detour_status() {
        if !enabled {
            disabled.push(name);
        }
    }
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

/// `ZTSHOWINFO_STATUS_PREDICATES_LIVE` - Stage 2: `getScheduledShowScript`/`isReady`/`isStarted`/
/// `isStopped`, the four predicates whose only real-memory dependency is the pending-scripts tree
/// (`ZTShowInfo+0x44`, already owned by `ztshow.rs`). Two real, standalone `ZTShowInfo` instances
/// (`ztshow_live_support::build_standalone_show_info` - real freelist/allocator-backed, includes a
/// valid pending-scripts tree header) with identical schedule-array setups: `.hooked()` on one,
/// `.original()` (routed through the debug trampoline to the pre-detour body - see `generated.rs`'s
/// own module doc comment) on the other, comparing results after each step.
///
/// **Caveat on `IS_STOPPED`'s own real pole**: since this module's `IS_READY`/`IS_STARTED` detours are
/// installed process-wide once `ztshowinfo::init()` runs, real vanilla `isStopped`'s own internal calls
/// to `isReady`/`isStarted` land on *this port's* Rust code too (not real vanilla's), even when reached
/// via `IS_STOPPED.original()` - `.original()`'s trampoline only bypasses `isStopped`'s own detour, not
/// the ones its body calls into. `isReady`/`isStarted` still get their own direct, unambiguous
/// real-vs-rust comparisons above/below, so this doesn't leave either of them uncovered - it just means
/// `IS_STOPPED`'s own comparison here is weaker evidence for its short-circuit logic specifically.
pub(crate) fn run_ztshowinfo_status_predicates_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_STATUS_PREDICATES_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    // Case 1: empty schedule array (both instances start this way) - getScheduledShowScript must
    // return 0 without touching the pending-scripts tree at all, and isReady/isStarted/isStopped must
    // agree on their all-zeroed-field defaults.
    let rust_script = unsafe { GET_SCHEDULED_SHOW_SCRIPT.hooked()(rust_info as *const u32) };
    let real_script = unsafe { GET_SCHEDULED_SHOW_SCRIPT.original()(real_info as *const u32) };
    if rust_script != real_script {
        failures.push(format!("empty-schedule getScheduledShowScript mismatch: rust={rust_script:#010x} real={real_script:#010x}"));
    }
    let rust_ready = unsafe { IS_READY.hooked()(rust_info as *const u32) };
    let real_ready = unsafe { IS_READY.original()(real_info as *const u32) };
    if (rust_ready != 0) != (real_ready != 0) {
        failures.push(format!("empty-schedule isReady mismatch: rust={rust_ready:#x} real={real_ready:#x}"));
    }
    let rust_started = unsafe { IS_STARTED.hooked()(rust_info as *const u32) };
    let real_started = unsafe { IS_STARTED.original()(real_info as *const u32) };
    if (rust_started != 0) != (real_started != 0) {
        failures.push(format!("empty-schedule isStarted mismatch: rust={rust_started:#x} real={real_started:#x}"));
    }
    let rust_stopped = unsafe { IS_STOPPED.hooked()(rust_info as *const u32) };
    let real_stopped = unsafe { IS_STOPPED.original()(real_info as *const u32) };
    if (rust_stopped != 0) != (real_stopped != 0) {
        failures.push(format!("empty-schedule isStopped mismatch: rust={rust_stopped:#x} real={real_stopped:#x}"));
    }

    // Case 2: a populated schedule array pointing at an already-known unit_type_id.
    //
    // **Deliberately pre-inserts the pending-scripts node via `ztshow::find_or_insert_pending_script_node`
    // (this module's own safe, unbalanced-BST insert) before calling `GET_SCHEDULED_SHOW_SCRIPT` at
    // all, on both poles** - never lets either pole's own `getScheduledShowScript` discover a genuine
    // *miss* here. Real vanilla's own miss branch calls through to the tree's real insert helper
    // (`AI_cls_0x404fd6::meth_0x5abe74`, per `ztshowinfo.rs`'s own module doc comment) - the exact real,
    // un-ported STL-glue-heavy routine `ztshow.rs`'s own `find_or_insert_pending_script_node` doc
    // comment already flags as too risky to invoke against anything but a fully-real, fully-constructed
    // `AI_cls_0x404fd6` control block. `ztshow_live_support::build_standalone_show_info`'s header is a
    // deliberately minimal `0xc`-byte stand-in (`self`/`root`/`leftmost` only - see that function's own
    // doc comment) that was never verified compatible with `meth_0x5abe74`'s own marshalling - calling
    // `GET_SCHEDULED_SHOW_SCRIPT.original()` against it while the schedule pointed at a fresh id crashed
    // the live battery outright the first time this test was written (real vanilla's own insert path
    // corrupting memory against the synthetic fixture). Pre-inserting first means every
    // `GET_SCHEDULED_SHOW_SCRIPT` call below - on both poles - only ever takes the safe, tree-search-only
    // "found" branch (confirmed via `.asm`: the `meth_0x5abe74` call is unreachable once the exact key
    // already exists).
    fn leaked_u32(value: u32) -> u32 {
        let buf: &'static mut [u32] = Box::leak(vec![value].into_boxed_slice());
        buf.as_mut_ptr() as u32
    }
    const UNIT_TYPE_ID: u32 = 0x1234;
    let rust_schedule = leaked_u32(UNIT_TYPE_ID);
    let real_schedule = leaked_u32(UNIT_TYPE_ID);
    save_to_memory(rust_info + 0x50, rust_schedule);
    save_to_memory(rust_info + 0x54, rust_schedule + 4);
    save_to_memory(rust_info + 0xa4, 0u32);
    save_to_memory(real_info + 0x50, real_schedule);
    save_to_memory(real_info + 0x54, real_schedule + 4);
    save_to_memory(real_info + 0xa4, 0u32);
    ztshow::find_or_insert_pending_script_node(rust_info, UNIT_TYPE_ID);
    ztshow::find_or_insert_pending_script_node(real_info, UNIT_TYPE_ID);

    // Masked to the low 16 bits: real vanilla's own decompile (`ZTShowInfo_getScheduledShowScript.c`)
    // shows the "found" branch's return as `CONCAT22((short)(node_ptr >> 0x10), *(u16*)(node+0x1c))` -
    // i.e. real vanilla's own EAX upper 16 bits are leftover node-pointer bits, an undefined-upper-bits
    // decompiler artifact (same class as `ZooStatus::fChance`'s own, per that fix's own precedent), not
    // real data - confirmed live: this comparison failed unmasked on the first run of this test
    // (`rust=0x00000000 real=0x15df0000`, the `0x15df` matching the real node's own pointer high bits).
    // `ztshowinfo.rs`'s own `get_scheduled_show_script` already documents that no known caller reads
    // beyond the low 16 bits.
    let rust_script = unsafe { GET_SCHEDULED_SHOW_SCRIPT.hooked()(rust_info as *const u32) } & 0xffff;
    let real_script = unsafe { GET_SCHEDULED_SHOW_SCRIPT.original()(real_info as *const u32) } & 0xffff;
    if rust_script != real_script {
        failures.push(format!("pre-inserted-node getScheduledShowScript mismatch: rust={rust_script:#06x} real={real_script:#06x}"));
    }
    let rust_count = ztshow::pending_script_node_count(rust_info);
    let real_count = ztshow::pending_script_node_count(real_info);
    if rust_count != real_count {
        failures.push(format!("pending-scripts node count mismatch: rust={rust_count} real={real_count}"));
    }

    // Case 3: stamp both instances' pre-inserted node with a matching "current" script id, set
    // this+0x8/this+0x22/this+0x23 to make isReady/isStarted/isStopped all take their true branch, and
    // confirm both poles agree. Re-finds the (already-existing) node the same safe way - never a new
    // insert, same reasoning as case 2 above.
    let (rust_node, rust_was_new) = ztshow::find_or_insert_pending_script_node(rust_info, UNIT_TYPE_ID);
    let (real_node, real_was_new) = ztshow::find_or_insert_pending_script_node(real_info, UNIT_TYPE_ID);
    if rust_was_new || real_was_new {
        failures.push("find_or_insert_pending_script_node should find the already-inserted node from case 2, not insert a new one".to_string());
    }
    const CURRENT_SCRIPT_ID: u16 = 0x4242;
    save_to_memory(rust_node + 0x1c, CURRENT_SCRIPT_ID);
    save_to_memory(real_node + 0x1c, CURRENT_SCRIPT_ID);
    save_to_memory(rust_info + 0x8, CURRENT_SCRIPT_ID);
    save_to_memory(real_info + 0x8, CURRENT_SCRIPT_ID);
    save_to_memory(rust_info + 0x22, 1u8);
    save_to_memory(real_info + 0x22, 1u8);
    save_to_memory(rust_info + 0x23, 1u8);
    save_to_memory(real_info + 0x23, 1u8);

    let rust_ready = unsafe { IS_READY.hooked()(rust_info as *const u32) };
    let real_ready = unsafe { IS_READY.original()(real_info as *const u32) };
    if (rust_ready != 0) != (real_ready != 0) {
        failures.push(format!("populated-and-ready isReady mismatch: rust={rust_ready:#x} real={real_ready:#x}"));
    }
    if rust_ready == 0 {
        failures.push("isReady should be true once this+0x22 is set".to_string());
    }
    let rust_started = unsafe { IS_STARTED.hooked()(rust_info as *const u32) };
    let real_started = unsafe { IS_STARTED.original()(real_info as *const u32) };
    if (rust_started != 0) != (real_started != 0) {
        failures.push(format!("populated-and-started isStarted mismatch: rust={rust_started:#x} real={real_started:#x}"));
    }
    if rust_started == 0 {
        failures.push("isStarted should be true once this+0x23 is set and this+0x8 matches the scheduled script id".to_string());
    }
    let rust_stopped = unsafe { IS_STOPPED.hooked()(rust_info as *const u32) };
    let real_stopped = unsafe { IS_STOPPED.original()(real_info as *const u32) };
    if (rust_stopped != 0) != (real_stopped != 0) {
        failures.push(format!("populated-and-ready/started isStopped mismatch: rust={rust_stopped:#x} real={real_stopped:#x}"));
    }
    if rust_stopped != 0 {
        failures.push("isStopped should be false once isReady/isStarted are both true".to_string());
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_ACCUMULATORS_LIVE` - `incrementAttendance`/`incrementReceipts`. Two real, standalone
/// `ZTShowInfo` instances (`ztshow_live_support::build_standalone_show_info`) with `this+0xc` set to the
/// same unit-type id (the key both accumulators look the pending-scripts tree up by): `.hooked()` on
/// one, `.original()` (debug trampoline to the pre-detour body - see `generated.rs`'s own module doc
/// comment) on the other, comparing every accumulator field after each call.
///
/// **Both poles are pre-inserted for the first key before any accumulator call, exactly one of them for
/// the second** - same reasoning as the status-predicates test's own case 2: a genuine *miss* would send
/// real vanilla's own insert helper (`AI_cls_0x404fd6::meth_0x5abe74`) against this fixture's deliberately
/// minimal `0xc`-byte tree header, which crashed the live battery the one time it was allowed to run
/// (see that test's own doc comment). The second key is pre-inserted on the **real** pole only, so the
/// Rust pole's accumulator call takes its own find-or-**insert** branch end-to-end (the already-stressed
/// `ztshow::find_or_insert_pending_script_node` insert, never real vanilla's) while the real pole still
/// only ever searches. Both poles' node counts are compared after each phase, so a node inserted on one
/// side only - or a double insert - fails the test rather than silently drifting the trees apart.
pub(crate) fn run_ztshowinfo_accumulators_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_ACCUMULATORS_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    // Phase 1 - found path, seeded with distinct nonzero starting values so a write to the wrong field
    // or a missing increment shows up as a mismatch, not as two matching zeros. All seeds/exercises are
    // exact binary fractions, so the float comparisons are exact.
    const KEY_A: u32 = 0x2345;
    save_to_memory(rust_info + 0xc, KEY_A);
    save_to_memory(real_info + 0xc, KEY_A);
    let (rust_node_a, rust_a_new) = ztshow::find_or_insert_pending_script_node(rust_info, KEY_A);
    let (real_node_a, real_a_new) = ztshow::find_or_insert_pending_script_node(real_info, KEY_A);
    if !rust_a_new || !real_a_new {
        failures.push("phase 1 pre-insert should have created one fresh node per pole".to_string());
    }
    for (base, node) in [(rust_info, rust_node_a), (real_info, real_node_a)] {
        save_to_memory(node + 0x28, 10.5f32);
        save_to_memory(node + 0x30, 20.25f32);
        save_to_memory(node + 0x34, 100i32);
        save_to_memory(node + 0x3c, 200i32);
        save_to_memory(base + 0x7c, 300i32);
        save_to_memory(base + 0x84, 400i32);
        save_to_memory(base + 0x94, 30.5f32);
        save_to_memory(base + 0x9c, 40.75f32);
    }

    for amount in [1i32, 1, 5] {
        unsafe { INCREMENT_ATTENDANCE.hooked()(rust_info as *const u32, amount) };
        unsafe { INCREMENT_ATTENDANCE.original()(real_info as *const u32, amount) };
    }
    for amount in [2.5f32, 0.75] {
        unsafe { INCREMENT_RECEIPTS.hooked()(rust_info as *const u32, amount) };
        unsafe { INCREMENT_RECEIPTS.original()(real_info as *const u32, amount) };
    }
    // Receipts pairs compared bit-exactly (all values are exact binary fractions); attendance pairs as
    // plain integers. Plus absolute expectations on the rust pole, so "both poles agree" can't mask a
    // shared wrong-field write.
    let float_fields = [
        ("node+0x28", rust_node_a + 0x28, real_node_a + 0x28),
        ("node+0x30", rust_node_a + 0x30, real_node_a + 0x30),
        ("this+0x94", rust_info + 0x94, real_info + 0x94),
        ("this+0x9c", rust_info + 0x9c, real_info + 0x9c),
    ];
    for (label, rust_addr, real_addr) in float_fields {
        let rust_value = get_from_memory::<f32>(rust_addr);
        let real_value = get_from_memory::<f32>(real_addr);
        if rust_value.to_bits() != real_value.to_bits() {
            failures.push(format!("phase 1 {label} mismatch: rust={rust_value} real={real_value}"));
        }
    }
    let int_fields = [
        ("node+0x34", rust_node_a + 0x34, real_node_a + 0x34),
        ("node+0x3c", rust_node_a + 0x3c, real_node_a + 0x3c),
        ("this+0x7c", rust_info + 0x7c, real_info + 0x7c),
        ("this+0x84", rust_info + 0x84, real_info + 0x84),
    ];
    for (label, rust_addr, real_addr) in int_fields {
        let rust_value = get_from_memory::<i32>(rust_addr);
        let real_value = get_from_memory::<i32>(real_addr);
        if rust_value != real_value {
            failures.push(format!("phase 1 {label} mismatch: rust={rust_value} real={real_value}"));
        }
    }
    if get_from_memory::<i32>(rust_node_a + 0x34) != 107 {
        failures.push(format!("phase 1 rust node+0x34 should be 107, holds {}", get_from_memory::<i32>(rust_node_a + 0x34)));
    }
    if get_from_memory::<f32>(rust_info + 0x94) != 33.75 {
        failures.push(format!("phase 1 rust this+0x94 should be 33.75, holds {}", get_from_memory::<f32>(rust_info + 0x94)));
    }

    let rust_count = ztshow::pending_script_node_count(rust_info);
    let real_count = ztshow::pending_script_node_count(real_info);
    if rust_count != 1 || real_count != 1 {
        failures.push(format!("phase 1 found-path inserts must not grow the tree: rust={rust_count} real={real_count}"));
    }

    // Phase 2 - the Rust pole's insert-on-miss branch: key B pre-inserted on the real pole only (see
    // this test's doc comment), never on the Rust pole, whose own accumulator call must create the node.
    const KEY_B: u32 = 0x2346;
    save_to_memory(rust_info + 0xc, KEY_B);
    save_to_memory(real_info + 0xc, KEY_B);
    let (real_node_b, real_b_new) = ztshow::find_or_insert_pending_script_node(real_info, KEY_B);
    if !real_b_new {
        failures.push("phase 2 real-pole pre-insert should have created a fresh node".to_string());
    }
    unsafe { INCREMENT_ATTENDANCE.hooked()(rust_info as *const u32, 3) };
    unsafe { INCREMENT_ATTENDANCE.original()(real_info as *const u32, 3) };
    unsafe { INCREMENT_RECEIPTS.hooked()(rust_info as *const u32, 1.5) };
    unsafe { INCREMENT_RECEIPTS.original()(real_info as *const u32, 1.5) };

    let (rust_node_b, rust_b_new) = ztshow::find_or_insert_pending_script_node(rust_info, KEY_B);
    if rust_b_new {
        failures.push("phase 2 re-find after the accumulator's own insert should find, not insert again".to_string());
    }
    for (label, offset) in [("node+0x28", 0x28u32), ("node+0x30", 0x30)] {
        let rust_value = get_from_memory::<f32>(rust_node_b + offset);
        let real_value = get_from_memory::<f32>(real_node_b + offset);
        if rust_value.to_bits() != real_value.to_bits() {
            failures.push(format!("phase 2 {label} mismatch: rust={rust_value} real={real_value}"));
        }
    }
    for (label, offset) in [("node+0x34", 0x34u32), ("node+0x3c", 0x3c)] {
        let rust_value = get_from_memory::<i32>(rust_node_b + offset);
        let real_value = get_from_memory::<i32>(real_node_b + offset);
        if rust_value != real_value {
            failures.push(format!("phase 2 {label} mismatch: rust={rust_value} real={real_value}"));
        }
    }
    let rust_count = ztshow::pending_script_node_count(rust_info);
    let real_count = ztshow::pending_script_node_count(real_info);
    if rust_count != 2 || real_count != 2 {
        failures.push(format!(
            "phase 2 expected exactly one new node per pole (rust pole via its own insert): rust={rust_count} real={real_count}"
        ));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE` - `setShowFrequency`/`recalculateSchedule`. Two real, standalone
/// `ZTShowInfo` instances (`ztshow_live_support::build_standalone_show_info`): `.hooked()` on one,
/// `.original()` (debug trampoline to the pre-detour body - see `generated.rs`'s own module doc
/// comment) on the other, comparing `this+0x68` (frequency) / `this+0x6c` (next-eligible time) /
/// `this+0xa4` (schedule cursor) after each step. Needs a populated `GLOBAL_ZTAIMgr` - the non-sentinel
/// `+0x6c` recompute reads its `+0xec` global AI-time cursor - hence `live_zoo_tests` rather than
/// `always_late_tests`.
///
/// **The `abortShow` call-through branch is deliberately not exercised.** It fires only on a transition
/// into the `-1` "never" sentinel while the show is *not* started, and real `ZTShow::abortShow`'s own
/// body (`ZTShow_abortShow.c`/`.asm`: event `0x2717` via `sendEventGeneric`, then a dispatch through
/// the embedded `ZTShow`'s *own* vtable `stop` slot) needs a genuinely-constructed `ZTShow` - this
/// fixture's zeroed embedded show has a null vtable pointer, so letting either pole take that branch
/// would crash the battery at the vtable read. Running it against a real show-tank's `ZTShowInfo`
/// instead would abort a real show mid-battery (real event + real stop, mutating live zoo state that
/// later tests source their data from). Everything short of the abort call itself - the sentinel
/// store, the `+0x6c` pin, and the `isStarted` gate that skips it - is covered by phase 4 below, plus
/// the host-side `set_show_frequency_to_never_while_started_stops_at_the_sentinel`; the leaf matches
/// the deliberate-gap precedent of `zoostatus.rs`'s own untested `f_grant_donation` message-firing
/// branches.
///
/// **Same real-pole caveat as `ZTSHOWINFO_STATUS_PREDICATES_LIVE`'s `IS_STOPPED` note**: real vanilla
/// `setShowFrequency`'s internal `isStarted` call lands on *this port's* detoured `IS_STARTED` address
/// (installed process-wide), not real vanilla's own body, even when reached via
/// `SET_SHOW_FREQUENCY.original()` - the trampoline only bypasses `setShowFrequency`'s own detour. The
/// gate decision is therefore evaluated by this port's `is_started` on both poles; `IS_STARTED` still
/// gets its own direct real-vs-rust comparison in the status-predicates test.
pub(crate) fn run_ztshowinfo_schedule_frequency_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    // Phase 1 - initial set on the non-sentinel path (frequency 3, show not started): both poles must
    // store the frequency and derive +0x6c as the AI-time cursor plus it. Cursor read once up front -
    // the battery runs with the simulation quiesced, same determinism assumption every other
    // byte-comparison test here already makes.
    let cursor = get_from_memory::<i32>(globals().ztaimgr_ptr() as u32 + 0xec);
    unsafe { SET_SHOW_FREQUENCY.hooked()(rust_info as *const u32, 3) };
    unsafe { SET_SHOW_FREQUENCY.original()(real_info as *const u32, 3) };
    for (label, base) in [("rust", rust_info), ("real", real_info)] {
        if get_from_memory::<i32>(base + 0x68) != 3 {
            failures.push(format!("phase 1 {label} +0x68 should be 3, holds {}", get_from_memory::<i32>(base + 0x68)));
        }
        if get_from_memory::<i32>(base + 0x6c) != cursor + 3 {
            failures.push(format!(
                "phase 1 {label} +0x6c should be cursor({cursor}) + 3, holds {}",
                get_from_memory::<i32>(base + 0x6c)
            ));
        }
    }

    // Phase 2 - unchanged-frequency short-circuit: re-setting the same frequency must skip even the
    // +0x6c refresh, proven by poisoning +0x6c first and checking the poison survives.
    save_to_memory(rust_info + 0x6c, 0xdead_beefu32);
    save_to_memory(real_info + 0x6c, 0xdead_beefu32);
    unsafe { SET_SHOW_FREQUENCY.hooked()(rust_info as *const u32, 3) };
    unsafe { SET_SHOW_FREQUENCY.original()(real_info as *const u32, 3) };
    for (label, base) in [("rust", rust_info), ("real", real_info)] {
        if get_from_memory::<u32>(base + 0x6c) != 0xdead_beef {
            failures.push(format!(
                "phase 2 {label} unchanged-frequency call must not refresh +0x6c, holds {:#010x}",
                get_from_memory::<u32>(base + 0x6c)
            ));
        }
    }

    // Phase 3 - the schedule-cursor advance: only recalculateSchedule's own boolean argument advances
    // it (setShowFrequency always passes false), wrapping modulo the array's element count. A
    // 3-element array starting at cursor 0 must walk 1, 2, 0, 1 across four advance calls, on both
    // poles; each call also re-derives +0x6c (frequency is still 3), so the poles must keep agreeing
    // on it too.
    fn leaked_schedule(values: [u32; 3]) -> u32 {
        let buf: &'static mut [u32] = Box::leak(values.to_vec().into_boxed_slice());
        buf.as_mut_ptr() as u32
    }
    const SCHEDULE: [u32; 3] = [0x1111, 0x2222, 0x3333];
    let rust_schedule = leaked_schedule(SCHEDULE);
    let real_schedule = leaked_schedule(SCHEDULE);
    save_to_memory(rust_info + 0x50, rust_schedule);
    save_to_memory(rust_info + 0x54, rust_schedule + 12);
    save_to_memory(rust_info + 0xa4, 0u32);
    save_to_memory(real_info + 0x50, real_schedule);
    save_to_memory(real_info + 0x54, real_schedule + 12);
    save_to_memory(real_info + 0xa4, 0u32);
    for expected_slot in [1i32, 2, 0, 1] {
        unsafe { RECALCULATE_SCHEDULE.hooked()(rust_info as *const u32, 1) };
        unsafe { RECALCULATE_SCHEDULE.original()(real_info as *const u32, 1) };
        let rust_slot = get_from_memory::<i32>(rust_info + 0xa4);
        let real_slot = get_from_memory::<i32>(real_info + 0xa4);
        if rust_slot != expected_slot || real_slot != expected_slot {
            failures.push(format!(
                "phase 3 schedule cursor should be {expected_slot} on both poles after advancing, rust={rust_slot} real={real_slot}"
            ));
        }
        if get_from_memory::<i32>(rust_info + 0x6c) != get_from_memory::<i32>(real_info + 0x6c) {
            failures.push("phase 3 poles disagree on +0x6c after the advance".to_string());
        }
    }

    // Phase 4 - transition into the -1 "never" sentinel while the show counts as started: the abort
    // call-through must be skipped (see this test's doc comment for why exercising it would crash the
    // battery), leaving +0x6c pinned to -1. The cursor sits at slot 1 after phase 3, so the scheduled
    // unit-type id is SCHEDULE[1]; both poles' pending-scripts trees are pre-inserted for it (same
    // discipline as the status-predicates test's case 2 - never let the real pole's own tree insert
    // run against this fixture) and stamped with a matching "current" script id so `isStarted` is
    // true on both poles.
    const SCHEDULED_ID: u32 = SCHEDULE[1];
    const CURRENT_SCRIPT_ID: u16 = 0x4242;
    let (rust_node, rust_new) = ztshow::find_or_insert_pending_script_node(rust_info, SCHEDULED_ID);
    let (real_node, real_new) = ztshow::find_or_insert_pending_script_node(real_info, SCHEDULED_ID);
    if !rust_new || !real_new {
        failures.push("phase 4 pre-insert should have created one fresh node per pole".to_string());
    }
    save_to_memory(rust_node + 0x1c, CURRENT_SCRIPT_ID);
    save_to_memory(real_node + 0x1c, CURRENT_SCRIPT_ID);
    save_to_memory(rust_info + 0x8, CURRENT_SCRIPT_ID);
    save_to_memory(real_info + 0x8, CURRENT_SCRIPT_ID);
    save_to_memory(rust_info + 0x23, 1u8);
    save_to_memory(real_info + 0x23, 1u8);

    unsafe { SET_SHOW_FREQUENCY.hooked()(rust_info as *const u32, -1) };
    unsafe { SET_SHOW_FREQUENCY.original()(real_info as *const u32, -1) };
    for (label, base) in [("rust", rust_info), ("real", real_info)] {
        if get_from_memory::<i32>(base + 0x68) != -1 {
            failures.push(format!("phase 4 {label} +0x68 should be -1, holds {}", get_from_memory::<i32>(base + 0x68)));
        }
        if get_from_memory::<i32>(base + 0x6c) != -1 {
            failures.push(format!(
                "phase 4 {label} +0x6c should be pinned to -1 for the never sentinel, holds {}",
                get_from_memory::<i32>(base + 0x6c)
            ));
        }
    }
    let rust_count = ztshow::pending_script_node_count(rust_info);
    let real_count = ztshow::pending_script_node_count(real_info);
    if rust_count != 1 || real_count != 1 {
        failures.push(format!("phase 4 expected exactly one pending-scripts node per pole: rust={rust_count} real={real_count}"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_KEEPER_PREDICATES_LIVE` - Stage 2: `getScheduledShowKeeperType`/`hasKeeper`/
/// `needsKeeper`, the three predicates that call through real, un-detoured `BFWorldMgr::getType`/
/// `ZTShowInfo::getNumUnits` against a live `GLOBAL_ZTWorldMgr` - needs `run_load_live_zoo` to have
/// already populated real entity-type data, hence `live_zoo_tests` rather than `always_late_tests`
/// (see `ztshowinfo.rs`'s own module doc comment). Sources a real, known-valid `unit_type_id` from an
/// already-configured real show (`find_real_show_tank_habitat`'s own `ZTShow+0x8` field - the same
/// discovery `ZTSHOW_CHECK_OWNING_HABITAT_LIVE`/`ZTSHOW_GROUP3_TRICK_LIVE` use) rather than a synthetic
/// one, so `BFWorldMgr::getType` resolves to a real `BFEntityType*` on both poles.
pub(crate) fn run_ztshowinfo_keeper_predicates_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_KEEPER_PREDICATES_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let Some((_, real_show_info_ptr)) = find_real_show_tank_habitat() else {
        write_success_line(failure_log, &format!("{} (skipped: no qualifying real show-tank habitat found)", test_name));
        return false;
    };
    let real_show = real_show_info_ptr + 4; // ZTShow is embedded at ZTShowInfo+0x4.
    let unit_type_id = get_from_memory::<u32>(real_show + 0x8);
    if unit_type_id == 0 {
        write_success_line(failure_log, &format!("{} (skipped: qualifying show-tank has no configured unit_type_id yet)", test_name));
        return false;
    }

    fn leaked_u32(value: u32) -> u32 {
        let buf: &'static mut [u32] = Box::leak(vec![value].into_boxed_slice());
        buf.as_mut_ptr() as u32
    }

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();
    let rust_schedule = leaked_u32(unit_type_id);
    let real_schedule = leaked_u32(unit_type_id);
    save_to_memory(rust_info + 0x50, rust_schedule);
    save_to_memory(rust_info + 0x54, rust_schedule + 4);
    save_to_memory(rust_info + 0xa4, 0u32);
    save_to_memory(real_info + 0x50, real_schedule);
    save_to_memory(real_info + 0x54, real_schedule + 4);
    save_to_memory(real_info + 0xa4, 0u32);

    let rust_keeper_type = unsafe { GET_SCHEDULED_SHOW_KEEPER_TYPE.hooked()(rust_info as *const u32) };
    let real_keeper_type = unsafe { GET_SCHEDULED_SHOW_KEEPER_TYPE.original()(real_info as *const u32) };
    if rust_keeper_type != real_keeper_type {
        failures.push(format!("getScheduledShowKeeperType mismatch: rust={rust_keeper_type:#010x} real={real_keeper_type:#010x}"));
    }

    // `hasKeeper`/`needsKeeper` both call through real, un-hooked `ZTShowInfo::getNumUnits` (Stage 7's
    // own scope), which shares the exact same `ZTShowInfo+0x44` tree `getScheduledShowScript` above
    // does - and, per the plan's own "Known hazards" section, has a real insert path of its own on a
    // genuine miss (new tree node + new list header, both from the shared `DAT_00638004` freelist) that
    // Stage 7/8 haven't verified safe to reproduce, let alone verified safe against a *synthetic*
    // fixture's minimal header. Pre-inserting the `keeper_type` key here (via this module's own safe,
    // already-verified `find_or_insert_pending_script_node`, same reasoning as the status-predicates
    // test's own case 2) guarantees `getNumUnits` only ever takes the safe "found" search branch on both
    // poles below, never real vanilla's own un-ported insert path.
    ztshow::find_or_insert_pending_script_node(rust_info, rust_keeper_type);
    ztshow::find_or_insert_pending_script_node(real_info, real_keeper_type);

    let rust_has_keeper = unsafe { HAS_KEEPER.hooked()(rust_info as *const u32) };
    let real_has_keeper = unsafe { HAS_KEEPER.original()(real_info as *const u32) };
    if (rust_has_keeper != 0) != (real_has_keeper != 0) {
        failures.push(format!("hasKeeper mismatch: rust={rust_has_keeper:#x} real={real_has_keeper:#x}"));
    }

    let rust_needs_keeper_match = unsafe { NEEDS_KEEPER.hooked()(rust_info as *const u32, rust_keeper_type) };
    let real_needs_keeper_match = unsafe { NEEDS_KEEPER.original()(real_info as *const u32, real_keeper_type) };
    if (rust_needs_keeper_match != 0) != (real_needs_keeper_match != 0) {
        failures.push(format!(
            "needsKeeper(keeper_type) mismatch: rust={rust_needs_keeper_match:#x} real={real_needs_keeper_match:#x}"
        ));
    }

    // A unit_type_id that is never the scheduled keeper type must short-circuit to false on both poles.
    let mismatched_type_id = unit_type_id.wrapping_add(0x7fff_ffff);
    let rust_needs_keeper_miss = unsafe { NEEDS_KEEPER.hooked()(rust_info as *const u32, mismatched_type_id) };
    let real_needs_keeper_miss = unsafe { NEEDS_KEEPER.original()(real_info as *const u32, mismatched_type_id) };
    if (rust_needs_keeper_miss != 0) != (real_needs_keeper_miss != 0) {
        failures.push(format!(
            "needsKeeper(mismatched id) mismatch: rust={rust_needs_keeper_miss:#x} real={real_needs_keeper_miss:#x}"
        ));
    }
    if rust_needs_keeper_miss != 0 {
        failures.push("needsKeeper should be false for a unit_type_id that never matches the scheduled keeper type".to_string());
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_ADD_REMOVE_SHOW_LIVE` - `addShow`/`removeShow`, Stage 5. Two real, standalone `ZTShowInfo`
/// instances (`ztshow_live_support::build_standalone_show_info`, all-zeroed `this+0x50/0x54/0x58`
/// registered-unit-types array): `.hooked()` on one, `.original()` (debug trampoline to the pre-detour
/// body) on the other. Adds five distinct ids, exercising every growth step from the empty array (`0->1`,
/// `1->2`, `2->4`), a duplicate (must not grow the array further), then removes one from the middle
/// (exercising the shift-down path and the `this+0xa4` schedule-cursor reset) - comparing array contents
/// after each step, not just the final state, so a divergence at any growth step is caught at its own
/// step.
pub(crate) fn run_ztshowinfo_add_remove_show_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_ADD_REMOVE_SHOW_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    fn array_contents(base: u32) -> Vec<u32> {
        let begin = get_from_memory::<u32>(base + 0x50);
        let end = get_from_memory::<u32>(base + 0x54);
        let mut values = Vec::new();
        let mut cursor = begin;
        while cursor != end {
            values.push(get_from_memory::<u32>(cursor));
            cursor += 4;
        }
        values
    }

    for id in [0x10u32, 0x20, 0x30, 0x40, 0x50] {
        unsafe { ADD_SHOW.hooked()(rust_info as *const u32, id) };
        unsafe { ADD_SHOW.original()(real_info as *const u32, id) };
        let rust_values = array_contents(rust_info);
        let real_values = array_contents(real_info);
        if rust_values != real_values {
            failures.push(format!("after adding {id:#x}: rust={rust_values:?} real={real_values:?}"));
        }
    }

    let rust_before = array_contents(rust_info);
    unsafe { ADD_SHOW.hooked()(rust_info as *const u32, 0x20) };
    unsafe { ADD_SHOW.original()(real_info as *const u32, 0x20) };
    let rust_after = array_contents(rust_info);
    let real_after = array_contents(real_info);
    if rust_after != rust_before {
        failures.push(format!("re-adding an existing id should be a no-op: before={rust_before:?} after={rust_after:?}"));
    }
    if rust_after != real_after {
        failures.push(format!("after re-adding 0x20: rust={rust_after:?} real={real_after:?}"));
    }

    save_to_memory(rust_info + 0xa4, 4i32);
    save_to_memory(real_info + 0xa4, 4i32);
    unsafe { REMOVE_SHOW.hooked()(rust_info as *const u32, 0x30) };
    unsafe { REMOVE_SHOW.original()(real_info as *const u32, 0x30) };
    let rust_values = array_contents(rust_info);
    let real_values = array_contents(real_info);
    if rust_values != real_values {
        failures.push(format!("after removing 0x30: rust={rust_values:?} real={real_values:?}"));
    }
    if rust_values.contains(&0x30) {
        failures.push("removed id 0x30 should no longer be present".to_string());
    }
    let rust_slot = get_from_memory::<i32>(rust_info + 0xa4);
    let real_slot = get_from_memory::<i32>(real_info + 0xa4);
    if rust_slot != real_slot {
        failures.push(format!("schedule-cursor mismatch after removal: rust={rust_slot} real={real_slot}"));
    }
    if rust_slot != 0 {
        failures.push(format!("schedule cursor 4 should reset to 0 once the array shrinks to 4 elements, holds {rust_slot}"));
    }

    unsafe { REMOVE_SHOW.hooked()(rust_info as *const u32, 0x9999) };
    unsafe { REMOVE_SHOW.original()(real_info as *const u32, 0x9999) };
    let rust_values = array_contents(rust_info);
    let real_values = array_contents(real_info);
    if rust_values != real_values {
        failures.push(format!("removing an absent id should be a no-op: rust={rust_values:?} real={real_values:?}"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_EVENT_SYSTEM_LIVE` - Stage 6: `sendEvent`/`getEvents`/`listen`/`cleanupEvents`. Two real,
/// standalone `ZTShowInfo` instances (`ztshow_live_support::build_standalone_show_info`), each with a
/// real `ZTShowInfo` vtable pointer installed (`ztshowinfo_live_support::install_vtable_pointer` - the
/// fixture itself never sets this field, and `getEvents`/`listen` are the first Stage 1-6 port to read it
/// live - see that helper's own doc comment) and `this+0x70` pinned to a sentinel target id
/// (`SENTINEL_TARGET_ID`) that cannot match any real, already-queued `GLOBAL_ZTAIMgr` event: `.hooked()`
/// on one, `.original()`/`GET_EVENTS.original()` (debug trampoline to the pre-detour body) on the other.
///
/// **The `listen` loop's own two vtable-dispatch branches (`0x2714`/`0x2716`) are deliberately never
/// exercised** - same reasoning `ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE`'s own doc comment already gives for
/// skipping the `abortShow` call-through: both branches dispatch through the embedded `ZTShow`'s own
/// vtable slot, and this fixture's embedded `ZTShow` sub-object has a null vtable pointer (`ztshow::
/// live_support::build_standalone_show_info` never constructs one). The `sendEvent` smoke call below
/// deliberately uses event id `0x1`, not `0x2714`/`0x2716` - it targets this same sentinel id, so a
/// same-battery round trip back through `listen`'s own tree (a real, later-delivered queued event, not
/// just already-queued ones) must never land on either dispatch id. `listen`'s own comparison is therefore
/// record-for-record (both poles must see the exact same set of `(target, event_id)` pairs, whatever that
/// turns out to be), with an explicit guard that neither pole's own result ever contains a dispatch id.
///
/// `cleanupEvents` is additionally exercised against a **non-empty, Rust-owned scratch range** (never a
/// real vector allocation) on both poles - this is the live half of `ztshowinfo.rs`'s own "the compaction
/// loop is dead code" determination (see that module's doc comment): if real vanilla's own `.original()`
/// call ever actually touched the range's contents (falsifying that determination), this would show up
/// either as a battery crash or as the scratch bytes changing, not just as a silent pass.
pub(crate) fn run_ztshowinfo_event_system_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_EVENT_SYSTEM_LIVE";
    let mut failures: Vec<String> = Vec::new();

    /// A target id essentially guaranteed never to match any event actually queued for a real entity at
    /// battery-run time.
    const SENTINEL_TARGET_ID: u16 = 0xbeef;

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();
    ztshowinfo_live_support::install_vtable_pointer(rust_info);
    ztshowinfo_live_support::install_vtable_pointer(real_info);
    save_to_memory(rust_info + 0x70, SENTINEL_TARGET_ID);
    save_to_memory(real_info + 0x70, SENTINEL_TARGET_ID);

    // sendEvent: a pure forward into `GLOBAL_ZTAIMgr`'s own embedded sub-object vtable - just confirms
    // both poles survive the call (no return value, no `ZTShowInfo`-local state to compare).
    //
    // **Deliberately not event id `0x2714`/`0x2716`**: this call genuinely queues a real event with
    // `GLOBAL_ZTAIMgr` targeted at `SENTINEL_TARGET_ID` - the exact same id/target `listen` below then
    // queries. Sending one of `listen`'s own two dispatch ids here would make it match its own
    // just-queued event a few lines down and dispatch into the embedded `ZTShow`'s null vtable pointer
    // (see this test's own doc comment on why that branch is otherwise deliberately unexercised). `0x1`
    // is not a dispatch id `listen` recognizes, so even a same-battery round trip back through `listen`
    // stays in its harmless no-op branch.
    unsafe { SEND_EVENT.hooked()(rust_info as *const u32, 0x1, 0, 0x57, 0, 0, 1) };
    unsafe { SEND_EVENT.original()(real_info as *const u32, 0x1, 0, 0x57, 0, 0, 1) };

    // listen: re-populates this+0x5c/this+0x60 via getEvents, then walks it. Since no real event can
    // match SENTINEL_TARGET_ID, both poles must end up with an empty (possibly non-null-begin) range.
    unsafe { LISTEN.hooked()(rust_info as *const u32) };
    unsafe { LISTEN.original()(real_info as *const u32) };
    // Both poles call into the exact same real `GLOBAL_ZTAIMgr` with the exact same target id, so
    // whatever it actually returns (the just-sent `0x1` event above included) must match record-for-record
    // - not necessarily empty, since the earlier `sendEvent` call may have genuinely queued a real,
    // later-delivered event for this same sentinel target.
    fn record_ids(begin: u32, end: u32) -> Vec<(u32, u16)> {
        let mut ids = Vec::new();
        let mut cursor = begin;
        while cursor != end {
            ids.push((get_from_memory::<u32>(cursor + 0xc), get_from_memory::<u16>(cursor + 0x16)));
            cursor += 0x1c;
        }
        ids
    }
    let rust_records = record_ids(get_from_memory::<u32>(rust_info + 0x5c), get_from_memory::<u32>(rust_info + 0x60));
    let real_records = record_ids(get_from_memory::<u32>(real_info + 0x5c), get_from_memory::<u32>(real_info + 0x60));
    if rust_records != real_records {
        failures.push(format!("listen produced different event records: rust={rust_records:?} real={real_records:?}"));
    }
    if rust_records.iter().any(|&(_, id)| id == 0x2714 || id == 0x2716) {
        failures.push(format!(
            "listen returned a dispatch-id record ({rust_records:?}) despite the sentinel target id - the vtable-dispatch branches were assumed unreachable here"
        ));
    }

    // getEvents, called directly (the same call `listen` above already made indirectly) - confirms the
    // real, un-detoured `+0xc` vtable target (`bfunit::GET_EVENTS_3`) still runs correctly through
    // `GET_EVENTS.original()`'s now-corrected (post-regeneration) 2-arg signature.
    unsafe { GET_EVENTS.original()(real_info as *const u32, real_info + 0x5c) };

    // cleanupEvents: a non-empty, Rust-owned scratch range on both poles - see this test's own doc
    // comment for why this is the live half of the "compaction loop is dead code" determination.
    let mut rust_scratch = [0xffu8; 0x1c * 3];
    let mut real_scratch = [0xffu8; 0x1c * 3];
    let rust_scratch_begin = rust_scratch.as_mut_ptr() as u32;
    let real_scratch_begin = real_scratch.as_mut_ptr() as u32;
    save_to_memory(rust_info + 0x5c, rust_scratch_begin);
    save_to_memory(rust_info + 0x60, rust_scratch_begin + 0x1c * 3);
    save_to_memory(real_info + 0x5c, real_scratch_begin);
    save_to_memory(real_info + 0x60, real_scratch_begin + 0x1c * 3);

    unsafe { CLEANUP_EVENTS.hooked()(rust_info as *const u32) };
    unsafe { CLEANUP_EVENTS.original()(real_info as *const u32) };

    if get_from_memory::<u32>(rust_info + 0x60) != rust_scratch_begin {
        failures.push("rust pole: cleanupEvents should reset end to begin".to_string());
    }
    if get_from_memory::<u32>(real_info + 0x60) != real_scratch_begin {
        failures.push("real pole: cleanupEvents should reset end to begin - the 'compaction loop is dead code' determination is wrong".to_string());
    }
    if rust_scratch != [0xffu8; 0x1c * 3] {
        failures.push("rust pole: cleanupEvents must not touch the range's own bytes".to_string());
    }
    if real_scratch != [0xffu8; 0x1c * 3] {
        failures.push("real pole: cleanupEvents touched the range's own bytes - the 'compaction loop is dead code' determination is wrong".to_string());
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_CREATE_DEFAULT_SCRIPT_LIVE` - `createDefaultScript`, Stage 5. Needs a real, already-loaded
/// unit type castable to `ZTAnimalType` (real `BFWorldMgr::getType` + the animal-type vtable check, per
/// `create_default_script`'s own doc comment) and a real `ZTShowInfo` for `validateTrick`'s own building
/// check - both sourced from an already-configured real show-tank habitat, the same discovery
/// `ZTSHOWINFO_KEEPER_PREDICATES_LIVE`/`ZTSHOW_GROUP3_TRICK_LIVE` already use. Each pole constructs its
/// own distinct real `ZTShowScript` (leaked, matching this class family's own short-lived-test-process
/// leak precedent - see `ztshow_live_support::build_standalone_show_info`'s own doc comment), so this
/// compares the resulting scripts' *contents* (item count and each item's real trick id, in order) rather
/// than pointer identity.
pub(crate) fn run_ztshowinfo_create_default_script_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_CREATE_DEFAULT_SCRIPT_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let Some((_, real_show_info_ptr)) = find_real_show_tank_habitat() else {
        write_success_line(failure_log, &format!("{} (skipped: no qualifying real show-tank habitat found)", test_name));
        return false;
    };
    let real_show = real_show_info_ptr + 4; // ZTShow is embedded at ZTShowInfo+0x4.
    let unit_type_id = get_from_memory::<u32>(real_show + 0x8);
    if unit_type_id == 0 {
        write_success_line(failure_log, &format!("{} (skipped: qualifying show-tank has no configured unit_type_id yet)", test_name));
        return false;
    }

    let rust_script = unsafe { CREATE_DEFAULT_SCRIPT.hooked()(real_show_info_ptr as *const u32, unit_type_id as i32) } as u32;
    let real_script = unsafe { CREATE_DEFAULT_SCRIPT.original()(real_show_info_ptr as *const u32, unit_type_id as i32) } as u32;

    if rust_script == 0 || real_script == 0 {
        failures.push(format!(
            "expected both poles to construct a script for a real animal-castable unit type: rust={rust_script:#010x} real={real_script:#010x}"
        ));
        return finish_test(test_name, failures, failure_log);
    }

    let rust_id = get_from_memory::<u16>(rust_script + 0x4);
    let real_id = get_from_memory::<u16>(real_script + 0x4);
    let rust_count = crate::ztshowscriptmgr::script_item_count_by_id(rust_id);
    let real_count = crate::ztshowscriptmgr::script_item_count_by_id(real_id);
    if rust_count != real_count {
        failures.push(format!("item count mismatch: rust={rust_count} real={real_count}"));
    }

    for index in 0..rust_count.min(real_count) as u16 {
        let rust_item = crate::ztshowscriptmgr::item_full_by_id(rust_id, index);
        let real_item = crate::ztshowscriptmgr::item_full_by_id(real_id, index);
        match (rust_item, real_item) {
            (Some(r), Some(e)) if r.id != e.id => {
                failures.push(format!("item {index} trick id mismatch: rust={:#x} real={:#x}", r.id, e.id));
            }
            (None, _) | (_, None) => failures.push(format!("item {index} missing on one pole")),
            _ => {}
        }
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_UNIT_ROSTER_READ_LIVE` - `getNumUnits`/`getShowUnitList`, Stage 7. Two real, standalone
/// `ZTShowInfo` instances (`ztshow_live_support::build_standalone_show_info`): `.hooked()` on one,
/// `.original()` (debug trampoline to the pre-detour body) on the other - both only ever depend on the
/// pending-scripts tree (`ZTShowInfo+0x44`), the same tree `ZTSHOWINFO_ACCUMULATORS_LIVE` already exercises
/// live, so this runs from `always_late_tests` rather than `live_zoo_tests` (no `GLOBAL_ZTWorldMgr` needed -
/// see the module doc comment's own Stage 7 section).
///
/// **Same "never let real vanilla's own insert run against this fixture" discipline as every other test in
/// this file that touches the pending-scripts tree**: phase 1 pre-inserts on both poles via
/// `ztshow::find_or_insert_pending_script_node` before either `GET_NUM_UNITS`/`GET_SHOW_UNIT_LIST` call, and
/// phase 2 pre-inserts on the real pole only (letting the Rust pole's own detoured `GET_NUM_UNITS` take its
/// own, already-stress-tested insert branch end-to-end) - matching `ZTSHOWINFO_ACCUMULATORS_LIVE`'s own
/// two-phase shape exactly.
///
/// **Non-empty list coverage without Stage 8**: `ADD_UNIT`/`ADD_UNIT_TO_LIST` (the real insert path onto a
/// node's own `+0x18` unit list) aren't ported yet, so this manually grafts a few `Box::leak`'d fake list
/// entries directly onto each pole's own node - safe, since `getNumUnits`/`getShowUnitList` only ever read
/// this list (never free or reallocate it; that's Stage 8's own scope), matching this file's own established
/// `Box::leak` fixture-construction precedent (`leaked_u32`/`leaked_schedule` above).
pub(crate) fn run_ztshowinfo_unit_roster_read_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_UNIT_ROSTER_READ_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    // Phase 1 - found path (pre-inserted on both poles).
    const KEY_A: u32 = 0x3456;
    let (rust_node_a, rust_a_new) = ztshow::find_or_insert_pending_script_node(rust_info, KEY_A);
    let (real_node_a, real_a_new) = ztshow::find_or_insert_pending_script_node(real_info, KEY_A);
    if !rust_a_new || !real_a_new {
        failures.push("phase 1 pre-insert should have created one fresh node per pole".to_string());
    }

    // A freshly-inserted node's own unit list must be empty on both poles.
    let rust_count = unsafe { GET_NUM_UNITS.hooked()(rust_info as *const u32, KEY_A) };
    let real_count = unsafe { GET_NUM_UNITS.original()(real_info as *const u32, KEY_A) };
    if rust_count != 0 || real_count != 0 {
        failures.push(format!("phase 1 fresh-node count should be 0 on both poles: rust={rust_count} real={real_count}"));
    }

    let rust_list_ptr = unsafe { GET_SHOW_UNIT_LIST.hooked()(rust_info as *const u32, KEY_A) } as u32;
    let real_list_ptr = unsafe { GET_SHOW_UNIT_LIST.original()(real_info as *const u32, KEY_A) } as u32;
    if rust_list_ptr != rust_node_a + 0x18 {
        failures.push(format!(
            "rust getShowUnitList should return node+0x18 ({:#010x}), returned {:#010x}",
            rust_node_a + 0x18,
            rust_list_ptr
        ));
    }
    if real_list_ptr != real_node_a + 0x18 {
        failures.push(format!(
            "real getShowUnitList should return node+0x18 ({:#010x}), returned {:#010x}",
            real_node_a + 0x18,
            real_list_ptr
        ));
    }

    // Graft three fake unit-list entries onto each pole's own node's list and re-count.
    fn graft_list_entries(sentinel: u32, count: usize) {
        let mut prev = sentinel;
        for _ in 0..count {
            let entry: &'static mut [u32; 3] = Box::leak(Box::new([0u32; 3]));
            let entry_addr = entry.as_mut_ptr() as u32;
            save_to_memory(prev, entry_addr); // prev.next = entry
            save_to_memory(entry_addr + 4, prev); // entry.prev = prev
            prev = entry_addr;
        }
        save_to_memory(prev, sentinel); // close the circle: last.next = sentinel
        save_to_memory(sentinel + 4, prev); // sentinel.prev = last
    }
    let rust_sentinel = get_from_memory::<u32>(rust_node_a + 0x18);
    let real_sentinel = get_from_memory::<u32>(real_node_a + 0x18);
    graft_list_entries(rust_sentinel, 3);
    graft_list_entries(real_sentinel, 3);

    let rust_count = unsafe { GET_NUM_UNITS.hooked()(rust_info as *const u32, KEY_A) };
    let real_count = unsafe { GET_NUM_UNITS.original()(real_info as *const u32, KEY_A) };
    if rust_count != 3 || real_count != 3 {
        failures.push(format!("phase 1 populated-list count should be 3 on both poles: rust={rust_count} real={real_count}"));
    }

    // Phase 2 - the Rust pole's own insert-on-miss branch: key B pre-inserted on the real pole only, same
    // reasoning as `ZTSHOWINFO_ACCUMULATORS_LIVE`'s own phase 2.
    const KEY_B: u32 = 0x3457;
    let (_real_node_b, real_b_new) = ztshow::find_or_insert_pending_script_node(real_info, KEY_B);
    if !real_b_new {
        failures.push("phase 2 real-pole pre-insert should have created a fresh node".to_string());
    }
    let rust_count_b = unsafe { GET_NUM_UNITS.hooked()(rust_info as *const u32, KEY_B) };
    let real_count_b = unsafe { GET_NUM_UNITS.original()(real_info as *const u32, KEY_B) };
    if rust_count_b != 0 || real_count_b != 0 {
        failures.push(format!("phase 2 fresh-via-getNumUnits count should be 0 on both poles: rust={rust_count_b} real={real_count_b}"));
    }
    let (_rust_node_b, rust_b_new) = ztshow::find_or_insert_pending_script_node(rust_info, KEY_B);
    if rust_b_new {
        failures.push("phase 2 re-find after getNumUnits's own insert should find, not insert again".to_string());
    }
    let rust_node_count = ztshow::pending_script_node_count(rust_info);
    let real_node_count = ztshow::pending_script_node_count(real_info);
    if rust_node_count != 2 || real_node_count != 2 {
        failures.push(format!("phase 2 expected exactly two pending-scripts nodes per pole: rust={rust_node_count} real={real_node_count}"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_REMOVE_UNIT_LIVE` - `removeUnit`, Stage 8 (erase-only - see `ztshowinfo.rs`'s own module doc
/// comment for why the insert side stays un-ported). Two real, standalone `ZTShowInfo` instances, same as
/// `ZTSHOWINFO_UNIT_ROSTER_READ_LIVE` above - no live zoo needed, so this runs from `always_late_tests` too.
///
/// **Real `0xc`-byte list entries, not `Box::leak`'d ones**: unlike `ZTSHOWINFO_UNIT_ROSTER_READ_LIVE`'s own
/// fixture grafting (safe there because `getNumUnits`/`getShowUnitList` only ever read the list),
/// `removeUnit` frees the entry it unlinks back to real vanilla's own shared freelist - pushing a
/// Rust-heap/stack-backed pointer onto that freelist would be exactly the cross-allocator corruption
/// `CLAUDE.md` warns about. Entries are allocated via the real `OPERATOR_NEW.original()(0xc)` instead,
/// matching `ztshow.rs`'s own `allocate_pending_script_node` precedent for this class family's sentinels.
///
/// **Real `REMOVE_UNIT.original()`'s third argument is the raw `u32` value bit-reinterpreted into its
/// pointer-typed slot, not an actual pointer** - see `ztshowinfo.rs`'s own module doc comment on why
/// `generated.rs`'s `*const i32` typing here is a wart. Passing a genuine dereferenceable pointer would
/// still "work" only by accident (the real body never dereferences it) - this test passes the value itself,
/// matching both of real vanilla's own confirmed callers and this stage's own detour.
pub(crate) fn run_ztshowinfo_remove_unit_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_REMOVE_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    const TYPE_ID: u32 = 0x3488;
    const UNIT_A: u32 = 111;
    const UNIT_B: u32 = 222;
    const UNIT_MISSING: u32 = 9999;

    /// Grafts one real, `OPERATOR_NEW`-allocated `{next, prev, payload}` entry onto `sentinel`'s own
    /// circular list, at the tail (before the sentinel).
    fn graft_real_entry(sentinel: u32, payload: u32) {
        let entry = unsafe { OPERATOR_NEW.original()(0xc) } as u32;
        let prev = get_from_memory::<u32>(sentinel + 4);
        save_to_memory(entry, sentinel);
        save_to_memory(entry + 4, prev);
        save_to_memory(entry + 8, payload);
        save_to_memory(prev, entry);
        save_to_memory(sentinel + 4, entry);
    }

    let (rust_node, _) = ztshow::find_or_insert_pending_script_node(rust_info, TYPE_ID);
    let (real_node, _) = ztshow::find_or_insert_pending_script_node(real_info, TYPE_ID);
    let rust_sentinel = get_from_memory::<u32>(rust_node + 0x18);
    let real_sentinel = get_from_memory::<u32>(real_node + 0x18);

    for &unit in &[UNIT_A, UNIT_B] {
        graft_real_entry(rust_sentinel, unit);
        graft_real_entry(real_sentinel, unit);
    }

    // Removing an absent id must be a no-op (both poles still count 2).
    remove_unit(rust_info, TYPE_ID, UNIT_MISSING);
    unsafe { REMOVE_UNIT.original()(real_info as *const u32, TYPE_ID, UNIT_MISSING as usize as *const i32) };
    let rust_count = unsafe { GET_NUM_UNITS.hooked()(rust_info as *const u32, TYPE_ID) };
    let real_count = unsafe { GET_NUM_UNITS.original()(real_info as *const u32, TYPE_ID) };
    if rust_count != 2 || real_count != 2 {
        failures.push(format!("removing an absent id should be a no-op: rust={rust_count} real={real_count} (want 2, 2)"));
    }

    // Remove UNIT_A: one entry left (UNIT_B), on both poles.
    remove_unit(rust_info, TYPE_ID, UNIT_A);
    unsafe { REMOVE_UNIT.original()(real_info as *const u32, TYPE_ID, UNIT_A as usize as *const i32) };
    let rust_count = unsafe { GET_NUM_UNITS.hooked()(rust_info as *const u32, TYPE_ID) };
    let real_count = unsafe { GET_NUM_UNITS.original()(real_info as *const u32, TYPE_ID) };
    if rust_count != 1 || real_count != 1 {
        failures.push(format!("after removing UNIT_A: rust={rust_count} real={real_count} (want 1, 1)"));
    }
    let rust_list = unsafe { GET_SHOW_UNIT_LIST.hooked()(rust_info as *const u32, TYPE_ID) } as u32;
    let remaining = get_from_memory::<u32>(get_from_memory::<u32>(rust_list));
    let remaining_payload = get_from_memory::<u32>(remaining + 0x8);
    if remaining_payload != UNIT_B {
        failures.push(format!("remaining rust entry's payload should be UNIT_B ({UNIT_B}), was {remaining_payload}"));
    }

    // Remove UNIT_B: list empty again on both poles.
    remove_unit(rust_info, TYPE_ID, UNIT_B);
    unsafe { REMOVE_UNIT.original()(real_info as *const u32, TYPE_ID, UNIT_B as usize as *const i32) };
    let rust_count = unsafe { GET_NUM_UNITS.hooked()(rust_info as *const u32, TYPE_ID) };
    let real_count = unsafe { GET_NUM_UNITS.original()(real_info as *const u32, TYPE_ID) };
    if rust_count != 0 || real_count != 0 {
        failures.push(format!("after removing both units: rust={rust_count} real={real_count} (want 0, 0)"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_CHECK_UNIT_LIVE` - `checkUnit`, Stage 7. Needs a real unit resolvable through
/// `BFWorldMgr::getUnit` against a live `GLOBAL_ZTWorldMgr`, hence `live_zoo_tests` (unlike
/// `ZTSHOWINFO_UNIT_ROSTER_READ_LIVE` above, `checkUnit` is unrelated to the pending-scripts tree - see the
/// module doc comment's own Stage 7 section). Reuses the same real, already-configured show-tank
/// habitat/trick-eligible-unit discovery `ZTSHOWINFO_KEEPER_PREDICATES_LIVE`/`ZTSHOW_GROUP3_TRICK_LIVE`
/// already use. `checkUnit` is read-only (forwards into [`crate::ztshow::check_unit_type`], itself a pure
/// field read), so both poles can safely run against the exact same real `ZTShowInfo`/unit pair.
///
/// **Also covers the trick-eligibility-check failure branch** (`ztshowinfo.rs:667`'s own `if
/// !entity_type_matches(...) { return 0; }`) via [`find_real_non_trick_eligible_unit`] - a real,
/// `BFWorldMgr::getUnit`-resolvable entity (e.g. a guest or staff member) whose type simply fails
/// `RVA_SHOW_TRICK_TYPE_CHECK`, distinct from the zero-id/bogus-id short-circuits already covered below
/// (those never even resolve a real unit pointer; this one does, then fails the type gate).
pub(crate) fn run_ztshowinfo_check_unit_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_CHECK_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let Some((_, real_show_info_ptr)) = find_real_show_tank_habitat() else {
        write_success_line(failure_log, &format!("{} (skipped: no qualifying real show-tank habitat found)", test_name));
        return false;
    };
    let Some((_, unit_id)) = find_real_trick_eligible_unit() else {
        write_success_line(failure_log, &format!("{} (skipped: no trick-eligible unit found in test zoo)", test_name));
        return false;
    };

    let rust_result = unsafe { CHECK_UNIT.hooked()(real_show_info_ptr as *const u32, unit_id) };
    let real_result = unsafe { CHECK_UNIT.original()(real_show_info_ptr as *const u32, unit_id) };
    if rust_result != real_result {
        failures.push(format!("checkUnit(eligible unit) mismatch: rust={rust_result:#010x} real={real_result:#010x}"));
    }

    // A real, resolvable unit whose entity type fails the trick-eligibility check must return 0 on both
    // poles, taking a genuinely distinct path from the zero-id/bogus-id cases below (which never resolve a
    // real unit pointer at all).
    if let Some(non_eligible_id) = find_real_non_trick_eligible_unit() {
        let rust_ineligible = unsafe { CHECK_UNIT.hooked()(real_show_info_ptr as *const u32, non_eligible_id) };
        let real_ineligible = unsafe { CHECK_UNIT.original()(real_show_info_ptr as *const u32, non_eligible_id) };
        if rust_ineligible != real_ineligible {
            failures.push(format!("checkUnit(non-eligible unit) mismatch: rust={rust_ineligible:#010x} real={real_ineligible:#010x}"));
        }
        if rust_ineligible != 0 {
            failures.push(format!("checkUnit(non-eligible unit) should be 0, rust={rust_ineligible:#010x}"));
        }
    } else {
        error!("{}: no non-trick-eligible unit found in test zoo - the ineligible-type branch is uncovered this run", test_name);
    }

    // unit_id == 0 must short-circuit to 0 on both poles.
    let rust_zero = unsafe { CHECK_UNIT.hooked()(real_show_info_ptr as *const u32, 0) };
    let real_zero = unsafe { CHECK_UNIT.original()(real_show_info_ptr as *const u32, 0) };
    if rust_zero != 0 || real_zero != 0 {
        failures.push(format!("checkUnit(0) should be 0 on both poles: rust={rust_zero:#010x} real={real_zero:#010x}"));
    }

    // A bogus unit id (never resolves via BFWorldMgr::getUnit) must also be 0 on both poles.
    let bogus_id = unit_id.wrapping_add(0x7fff_ffff);
    let rust_bogus = unsafe { CHECK_UNIT.hooked()(real_show_info_ptr as *const u32, bogus_id) };
    let real_bogus = unsafe { CHECK_UNIT.original()(real_show_info_ptr as *const u32, bogus_id) };
    if rust_bogus != real_bogus {
        failures.push(format!("checkUnit(bogus id) mismatch: rust={rust_bogus:#010x} real={real_bogus:#010x}"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_ENTER_NEW_MONTH_LIVE` - Stage 9's `enterNewMonth`. Two real, standalone `ZTShowInfo`
/// instances (`ztshow_live_support::build_standalone_show_info`): `.hooked()` on one, `.original()` (debug
/// trampoline to the pre-detour body) on the other. Unlike `update` below, `enterNewMonth` never touches
/// either object's own vtable pointer (confirmed via `.asm` - no vtable read anywhere in its body), so this
/// is a full, ordinary real-vs-rust field comparison, needing `live_zoo_tests` only for a populated
/// `GLOBAL_ZTWorldMgr` behind the live `generated.rs` `ztworldmgr::GET_GRANDSTANDS_UPKEEP` call (its own
/// real internals were never investigated, so this stays conservative
/// about what "populated" it needs, matching `ZTSHOWINFO_KEEPER_PREDICATES_LIVE`'s own precedent for
/// anything touching `GLOBAL_ZTWorldMgr`).
///
/// **Pre-inserts the pending-scripts node on both poles before either call**, same discipline as every
/// other test in this file that touches this tree (see `ZTSHOWINFO_ACCUMULATORS_LIVE`'s own doc comment) -
/// `enterNewMonth`'s own node loop never inserts (it only mutates existing nodes' value fields), but a
/// *seeded* node is needed so the archive/reset behavior has something to observe.
pub(crate) fn run_ztshowinfo_enter_new_month_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_ENTER_NEW_MONTH_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    const SENTINEL_TARGET_ID: u16 = 0xbee1;
    save_to_memory(rust_info + 0x70, SENTINEL_TARGET_ID);
    save_to_memory(real_info + 0x70, SENTINEL_TARGET_ID);

    // Distinct nonzero seeds so a swapped/missing rollover shows up as a mismatch, not two matching zeros.
    for base in [rust_info, real_info] {
        save_to_memory(base + 0x7c, 100i32); // attendance current month
        save_to_memory(base + 0x84, 900i32); // attendance all-time
        save_to_memory(base + 0x94, 25.5f32); // receipts current month
        save_to_memory(base + 0x9c, 400.25f32); // receipts all-time
        save_to_memory(base + 0x88, 7.5f32); // engagement sample "current"
        save_to_memory(base + 0x90, 2.0f32); // engagement running total
    }

    const KEY: u32 = 0x5678;
    let (rust_node, rust_new) = ztshow::find_or_insert_pending_script_node(rust_info, KEY);
    let (real_node, real_new) = ztshow::find_or_insert_pending_script_node(real_info, KEY);
    if !rust_new || !real_new {
        failures.push("pre-insert should have created one fresh node per pole".to_string());
    }
    for node in [rust_node, real_node] {
        save_to_memory(node + 0x28, 3.5f32);
        save_to_memory(node + 0x30, 40.0f32);
        save_to_memory(node + 0x34, 5i32);
        save_to_memory(node + 0x3c, 60i32);
    }

    unsafe { ENTER_NEW_MONTH.hooked()(rust_info as *const u32) };
    unsafe { ENTER_NEW_MONTH.original()(real_info as *const u32) };

    let float_fields = [
        ("this+0x98", rust_info + 0x98, real_info + 0x98),
        ("this+0x94", rust_info + 0x94, real_info + 0x94),
        ("this+0x9c", rust_info + 0x9c, real_info + 0x9c),
        ("this+0x8c", rust_info + 0x8c, real_info + 0x8c),
        ("this+0x88", rust_info + 0x88, real_info + 0x88),
        ("this+0x90", rust_info + 0x90, real_info + 0x90),
        ("node+0x28", rust_node + 0x28, real_node + 0x28),
        ("node+0x2c", rust_node + 0x2c, real_node + 0x2c),
        ("node+0x30", rust_node + 0x30, real_node + 0x30),
    ];
    for (label, rust_addr, real_addr) in float_fields {
        let rust_value = get_from_memory::<f32>(rust_addr);
        let real_value = get_from_memory::<f32>(real_addr);
        if rust_value.to_bits() != real_value.to_bits() {
            failures.push(format!("{label} mismatch: rust={rust_value} real={real_value}"));
        }
    }
    let int_fields = [
        ("this+0x80", rust_info + 0x80, real_info + 0x80),
        ("this+0x7c", rust_info + 0x7c, real_info + 0x7c),
        ("this+0x84", rust_info + 0x84, real_info + 0x84),
        ("node+0x34", rust_node + 0x34, real_node + 0x34),
        ("node+0x38", rust_node + 0x38, real_node + 0x38),
        ("node+0x3c", rust_node + 0x3c, real_node + 0x3c),
    ];
    for (label, rust_addr, real_addr) in int_fields {
        let rust_value = get_from_memory::<i32>(rust_addr);
        let real_value = get_from_memory::<i32>(real_addr);
        if rust_value != real_value {
            failures.push(format!("{label} mismatch: rust={rust_value} real={real_value}"));
        }
    }

    // Absolute expectations on the rust pole (the pure half of this is already unit-tested, but pinning the
    // archive values here too catches a shared wrong-field write both poles could otherwise agree on).
    if get_from_memory::<f32>(rust_info + 0x98) != 25.5 {
        failures.push(format!("rust this+0x98 should archive 25.5, holds {}", get_from_memory::<f32>(rust_info + 0x98)));
    }
    if get_from_memory::<i32>(rust_info + 0x80) != 100 {
        failures.push(format!("rust this+0x80 should archive 100, holds {}", get_from_memory::<i32>(rust_info + 0x80)));
    }
    if get_from_memory::<i32>(rust_info + 0x7c) != 0 || get_from_memory::<f32>(rust_info + 0x94) != 0.0 {
        failures.push("rust current-month accumulators should reset to 0".to_string());
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_UPDATE_LIVE` - Stage 9's `update()` (vtable slot `+0x20`). Two real, standalone `ZTShowInfo`
/// instances, each with a real `ZTShowInfo` vtable pointer installed
/// (`ztshowinfo_live_support::install_vtable_pointer` - the same precondition `ZTSHOWINFO_EVENT_SYSTEM_LIVE`
/// already establishes safe for `listen`/`cleanupEvents` dispatch) and `this+0x70` pinned to a sentinel
/// target id that can't match any real, already-queued `GLOBAL_ZTAIMgr` event.
///
/// **The embedded `ZTShow`'s own `+0xc` vtable slot is stubbed to a harmless no-op, not left real.** Real
/// vanilla `ZTShow::update` (`0x0059e773`, `private/docs/vtables/ZTShow.md`) is itself un-ported and was
/// never proven safe against anything but a genuinely-constructed `ZTShow` - the same hazard
/// `ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE`'s own doc comment already flags for the sibling `abortShow`
/// call-through, and this fixture's own embedded `ZTShow` is just as synthetic/zeroed. `update()`'s own
/// body only ever reads this one slot off the embedded object (confirmed via `.asm`), so stubbing just it
/// keeps every other real call this function makes (`listen`/`cleanupEvents` through `ZTShowInfo`'s own
/// real vtable, the float sample through real `ztworldmgr::GET_GRANDSTANDS_UPKEEP`/`GLOBAL_ZTWorldMgr`) genuinely live and
/// comparable - matching the plan's own "accepting the same real-callee-call-through caveats
/// `ZTAdvTerrainMgr`'s own tests document when a full byte-diff isn't safe to run twice against
/// shared/live state" allowance, without giving up the comparison outright.
///
/// **Same real-pole caveat as `ZTSHOWINFO_STATUS_PREDICATES_LIVE`'s `IS_STOPPED` note**: real vanilla
/// `update`'s own internal `listen`/`cleanupEvents` calls land on *this port's* detoured addresses
/// (installed process-wide) even when reached via `UPDATE.original()` - the trampoline only bypasses
/// `update`'s own detour, not the ones its body calls into. Both are already covered by their own direct
/// comparisons in `ZTSHOWINFO_EVENT_SYSTEM_LIVE`; this test's own value is the call *order* and the
/// float-decay tail.
pub(crate) fn run_ztshowinfo_update_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_UPDATE_LIVE";
    let mut failures: Vec<String> = Vec::new();

    const SENTINEL_TARGET_ID: u16 = 0xbee0;

    unsafe extern "thiscall" fn noop_ztshow_update(_this: *const u32) {}

    fn install_stub_ztshow_vtable(show_info: u32) {
        let vtable: &'static mut [u32; 4] = Box::leak(Box::new([0u32; 4]));
        vtable[3] = noop_ztshow_update as *const () as u32;
        let vtable_addr = vtable.as_mut_ptr() as u32;
        save_to_memory(show_info + 0x4, vtable_addr); // ZTShow's own vftptr, at ZTShowInfo+0x4+0x0.
    }

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();
    ztshowinfo_live_support::install_vtable_pointer(rust_info);
    ztshowinfo_live_support::install_vtable_pointer(real_info);
    install_stub_ztshow_vtable(rust_info);
    install_stub_ztshow_vtable(real_info);
    save_to_memory(rust_info + 0x70, SENTINEL_TARGET_ID);
    save_to_memory(real_info + 0x70, SENTINEL_TARGET_ID);

    for base in [rust_info, real_info] {
        save_to_memory(base + 0x88, 12.5f32);
        save_to_memory(base + 0x90, 3.0f32);
    }

    unsafe { UPDATE.hooked()(rust_info as *const u32) };
    unsafe { UPDATE.original()(real_info as *const u32) };

    for (label, offset) in [("this+0x88", 0x88u32), ("this+0x90", 0x90)] {
        let rust_value = get_from_memory::<f32>(rust_info + offset);
        let real_value = get_from_memory::<f32>(real_info + offset);
        if rust_value.to_bits() != real_value.to_bits() {
            failures.push(format!("{label} mismatch: rust={rust_value} real={real_value}"));
        }
    }

    // cleanupEvents' own effect: end reset to begin, on both poles.
    let rust_begin = get_from_memory::<u32>(rust_info + 0x5c);
    let rust_end = get_from_memory::<u32>(rust_info + 0x60);
    let real_begin = get_from_memory::<u32>(real_info + 0x5c);
    let real_end = get_from_memory::<u32>(real_info + 0x60);
    if rust_end != rust_begin {
        failures.push("rust pole: update()'s own cleanupEvents call should leave end == begin".to_string());
    }
    if real_end != real_begin {
        failures.push("real pole: update()'s own cleanupEvents call should leave end == begin".to_string());
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_SET_SHOW_INFO_ID_LIVE` - Stage 10's `setShowInfoID` reconciliation
/// (`ztshowinfo-implementation-plan.md`'s Stage 10). Two real, standalone `ZTShowInfo` instances
/// (`ztshow_live_support::build_standalone_show_info`): `.hooked()` on one, `.original()` (debug trampoline
/// to the pre-detour body - see `generated.rs`'s own module doc comment) on the other, comparing `this+0x70`
/// / the embedded `ZTShow`'s `+0x10` back-pointer / `+0x6` id copy after each call. Confirms the real,
/// now-detoured address produces the same result as the shared [`crate::ztshowinfo::set_show_info_id`]
/// `ZTShowMgr::register_show` already calls directly - no `GLOBAL_*` dependency, so this runs from
/// `always_late_tests` like the other pending-scripts-tree-independent groups in this file.
///
/// Exercises all three back-pointer branches [`crate::ztshowinfo::set_show_info_id`]'s own doc comment
/// describes: null (phase 1), stale (phase 2 - a distinct target whose own `field_0x70` disagrees with the
/// new id), and already-agreeing (phase 3 - poisoned first, so a spurious overwrite would show up as a
/// mismatch against the specific "agree" instance's own address, not just any non-null pointer).
pub(crate) fn run_ztshowinfo_set_show_info_id_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_SET_SHOW_INFO_ID_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    // Phase 1 - null back-pointer: must be repointed at `this` itself on both poles.
    unsafe { SET_SHOW_INFO_ID.hooked()(rust_info as *const u32, 0x42) };
    unsafe { SET_SHOW_INFO_ID.original()(real_info as *const u32, 0x42) };
    for (label, base) in [("rust", rust_info), ("real", real_info)] {
        let id = get_from_memory::<u16>(base + 0x70);
        if id != 0x42 {
            failures.push(format!("phase 1 {label} this+0x70 should be 0x42, holds {id:#06x}"));
        }
        if get_from_memory::<u32>(base + 0x4 + 0x10) != base {
            failures.push(format!("phase 1 {label} null back-pointer should be repointed at this"));
        }
        let ztshow_id = get_from_memory::<u16>(base + 0x4 + 0x6);
        if ztshow_id != 0x42 {
            failures.push(format!("phase 1 {label} embedded ZTShow id copy should be 0x42, holds {ztshow_id:#06x}"));
        }
    }

    // Phase 2 - stale back-pointer (target's own field_0x70 disagrees with the new id): must be repointed
    // at `this`.
    let rust_other = ztshow_live_support::build_standalone_show_info();
    let real_other = ztshow_live_support::build_standalone_show_info();
    save_to_memory(rust_other + 0x70, 0x99u16);
    save_to_memory(real_other + 0x70, 0x99u16);
    save_to_memory(rust_info + 0x4 + 0x10, rust_other);
    save_to_memory(real_info + 0x4 + 0x10, real_other);
    unsafe { SET_SHOW_INFO_ID.hooked()(rust_info as *const u32, 0x43) };
    unsafe { SET_SHOW_INFO_ID.original()(real_info as *const u32, 0x43) };
    for (label, base) in [("rust", rust_info), ("real", real_info)] {
        if get_from_memory::<u32>(base + 0x4 + 0x10) != base {
            failures.push(format!("phase 2 {label} stale back-pointer should be repointed at this"));
        }
    }

    // Phase 3 - back-pointer whose own target already agrees on the new id: must be left untouched.
    let rust_agree = ztshow_live_support::build_standalone_show_info();
    let real_agree = ztshow_live_support::build_standalone_show_info();
    save_to_memory(rust_agree + 0x70, 0x44u16);
    save_to_memory(real_agree + 0x70, 0x44u16);
    save_to_memory(rust_info + 0x4 + 0x10, rust_agree);
    save_to_memory(real_info + 0x4 + 0x10, real_agree);
    unsafe { SET_SHOW_INFO_ID.hooked()(rust_info as *const u32, 0x44) };
    unsafe { SET_SHOW_INFO_ID.original()(real_info as *const u32, 0x44) };
    if get_from_memory::<u32>(rust_info + 0x4 + 0x10) != rust_agree {
        failures.push("phase 3 rust: an already-agreeing back-pointer should be left untouched".to_string());
    }
    if get_from_memory::<u32>(real_info + 0x4 + 0x10) != real_agree {
        failures.push("phase 3 real: an already-agreeing back-pointer should be left untouched".to_string());
    }

    finish_test(test_name, failures, failure_log)
}

/// Installs a fresh, empty script-state tree header on a standalone `ZTShowInfo` fixture's embedded
/// `ZTShowState` (`this+0x38`, i.e. `ZTShow+0x34`/`ZTShowState+0x1c` - see `ztshowstate.rs`'s own module
/// doc comment for the field chain). `ztshow_live_support::build_standalone_show_info` only sets up the
/// pending-scripts tree header at `+0x44`; the embedded `ZTShow`/`ZTShowState` sub-object is otherwise a
/// plain zeroed buffer, so calling real, un-ported `ZTShow::save`/`load` against it (as
/// `ZTSHOWINFO_SAVE_LOAD_ROUNDTRIP` below does, via the detoured `ZTShowInfo::save`/`load`'s own tail
/// call-through) would dereference a null tree header inside Stage 1's own `ZTShowState::save`/`load` -
/// exactly the crash class `ZTSHOWINFO_UPDATE_LIVE`'s own doc comment already flags for this same fixture's
/// embedded `ZTShow`. Allocated via plain `OPERATOR_NEW`, matching `build_standalone_show_info`'s own
/// established "doesn't need the real freelist allocator, nothing here frees it through the wrong one"
/// precedent for a one-shot test fixture header.
fn install_empty_show_state_tree_header(show_info: u32) {
    let header = unsafe { OPERATOR_NEW.original()(0x18) } as u32;
    unsafe { std::ptr::write_bytes(header as *mut u8, 0, 0x18) };
    save_to_memory(header + 0x8, header);
    save_to_memory(header + 0xc, header);
    save_to_memory(show_info + 0x38, header);
}

/// `ZTSHOWINFO_SAVE_LOAD_ROUNDTRIP` - Stage 11's `save`/`load` (`ztshowinfo-implementation-plan.md`).
/// Seeds a real, standalone `ZTShowInfo` (`ztshow_live_support::build_standalone_show_info`, plus
/// [`install_empty_show_state_tree_header`] for the embedded `ZTShowState`'s own tree) with known values
/// across every field group [`crate::ztshowinfo::show_info_save`]/`show_info_load` handle: the two scalar
/// header fields, the registered-unit-types array (via [`add_show`], already its own live-tested port), one
/// pending-scripts node's full value-field set (seeded directly via [`find_or_insert_pending_script_node`]
/// for `current`/`pending`, then [`increment_attendance`]/[`increment_receipts`] for the accumulator pairs -
/// both already independently live-tested), and the eleven trailing scalars. The `BFEvent` array is left at
/// zero elements deliberately - `BFEvent` itself is not reimplemented anywhere in this codebase, so this
/// test exercises the array's own count-gating/allocation-skip path (`ev_count == 0`) rather than real
/// element content, which is outside this stage's own scope (see the module doc comment's own note on why
/// `BFEvent`'s per-element content isn't ported). Captures [`SAVE`]'s `.hooked()` output via `io_redirect`,
/// replays it into a second fresh fixture's `.hooked()` [`LOAD`], and compares every field plus the
/// roundtripped array/node contents.
///
/// **Known limitation: self-consistency only, not a real-vanilla comparison.** Both `SAVE` and `LOAD` are
/// `.hooked()` on both sides of this round trip - there is no `.original()` pole anywhere in this test - so
/// it can only catch a bug that makes the Rust `save`/`load` pair *disagree with itself*; a bug present
/// identically in both directions (e.g. both writing and reading the same wrong field, or both silently
/// skipping the same real field) would round-trip cleanly here and never be caught. No practical fix given
/// this battery's own detour-install ordering (there is no way to run real vanilla's own `save` against a
/// `ZTShowInfo` this test controls while `SAVE`'s address is already hooked process-wide) - documented
/// explicitly here rather than left implicit, matching this codebase's convention of calling out known test
/// gaps rather than hiding them.
pub(crate) fn run_ztshowinfo_save_load_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_SAVE_LOAD_ROUNDTRIP";
    let mut failures: Vec<String> = Vec::new();
    const CURRENT_VERSION: u32 = 0x100;

    let source = ztshow_live_support::build_standalone_show_info();
    install_empty_show_state_tree_header(source);

    save_to_memory(source + 0x68, 30i32); // frequency
    save_to_memory(source + 0xa4, 0i32); // schedule slot
    save_to_memory(source + 0x6c, 0x1111_2222u32);
    save_to_memory(source + 0x70, 0x3333u16);
    save_to_memory(source + 0x88, 0x4444_5555u32);
    save_to_memory(source + 0x8c, 0x6666_7777u32);
    save_to_memory(source + 0x90, 1.5f32);
    save_to_memory(source + 0x98, 0x8888_9999u32);
    save_to_memory(source + 0x80, 0xaaaa_bbbbu32);

    add_show(source, 501);
    add_show(source, 502);

    save_to_memory(source + 0xc, 777u32);
    let (node, _) = find_or_insert_pending_script_node(source, 777);
    save_to_memory(node + 0x1c, 42u16);
    save_to_memory(node + 0x1e, 43u16);
    increment_attendance(source, 3);
    increment_receipts(source, 4.5);

    let dummy_file: u32 = 0;

    io_redirect::begin_capture();
    let save_ret = unsafe { SAVE.hooked()(source as *const u32, &dummy_file as *const u32 as *const i8) };
    let bytes = io_redirect::end_capture();
    if save_ret & 0xff != 1 {
        failures.push(format!("hooked save should report success in its low byte, got {save_ret:#010x}"));
    }

    let target = ztshow_live_support::build_standalone_show_info();
    install_empty_show_state_tree_header(target);
    io_redirect::begin_replay(bytes);
    let load_ret = unsafe { LOAD.hooked()(target as *const u32, &dummy_file as *const u32, CURRENT_VERSION) };
    io_redirect::end_replay();
    if load_ret != 1 {
        failures.push(format!("hooked load should return 1, got {load_ret}"));
    }

    for (offset, size, label) in [
        (0x68u32, 4u32, "frequency (+0x68)"),
        (0xa4, 4, "schedule slot (+0xa4)"),
        (0x6c, 4, "+0x6c"),
        (0x70, 2, "+0x70"),
        (0x88, 4, "+0x88"),
        (0x8c, 4, "+0x8c"),
        (0x90, 4, "+0x90 (entrance rotation)"),
        (0x98, 4, "+0x98"),
        (0x80, 4, "+0x80"),
        (0x7c, 4, "+0x7c (attendance current, from increment_attendance)"),
        (0x84, 4, "+0x84 (attendance total, from increment_attendance)"),
        (0x94, 4, "+0x94 (receipts current, from increment_receipts)"),
        (0x9c, 4, "+0x9c (receipts total, from increment_receipts)"),
    ] {
        let src = match size {
            2 => get_from_memory::<u16>(source + offset) as u32,
            _ => get_from_memory::<u32>(source + offset),
        };
        let dst = match size {
            2 => get_from_memory::<u16>(target + offset) as u32,
            _ => get_from_memory::<u32>(target + offset),
        };
        if src != dst {
            failures.push(format!("{label} did not round-trip: source={src:#010x}, target={dst:#010x}"));
        }
    }

    let begin = get_from_memory::<u32>(target + 0x50);
    let end = get_from_memory::<u32>(target + 0x54);
    let ids: Vec<u32> = {
        let mut v = Vec::new();
        let mut cursor = begin;
        while cursor != end {
            v.push(get_from_memory::<u32>(cursor));
            cursor += 4;
        }
        v
    };
    if ids != vec![501, 502] {
        failures.push(format!("registered-unit-types array did not round-trip: got {ids:?}"));
    }

    let (target_node, was_inserted) = find_or_insert_pending_script_node(target, 777);
    if was_inserted {
        failures.push("pending-scripts node for key 777 should already exist after load, not be freshly inserted".to_string());
    }
    if get_from_memory::<u16>(target_node + 0x1c) != 42 {
        failures.push("node +0x1c (current) did not round-trip".to_string());
    }
    if get_from_memory::<u16>(target_node + 0x1e) != 43 {
        failures.push("node +0x1e (pending) did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target_node + 0x28) != get_from_memory::<u32>(node + 0x28) {
        failures.push("node +0x28 (receipts current) did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target_node + 0x30) != get_from_memory::<u32>(node + 0x30) {
        failures.push("node +0x30 (receipts total) did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target_node + 0x34) != get_from_memory::<u32>(node + 0x34) {
        failures.push("node +0x34 (attendance current) did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target_node + 0x3c) != get_from_memory::<u32>(node + 0x3c) {
        failures.push("node +0x3c (attendance total) did not round-trip".to_string());
    }

    let node_count = get_from_memory::<u32>(target + 0x48);
    if node_count != 1 {
        failures.push(format!("target's cached pending-scripts node count should be 1 after load, got {node_count}"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_STANDALONE_ROUNDTRIP` - Stage 12 (closing stage): both constructor overloads and the real
/// destructor, run end-to-end against real vanilla `ZTShowInfo` allocations. See `ztshowinfo.rs`'s own
/// module doc comment's Stage 12 section for why none of the four addresses involved
/// (`ztshowinfo::CONSTRUCTOR_0`/`CONSTRUCTOR_1`/`DESTRUCTOR_0`/`DESTRUCTOR_1`) gets a Rust reimplementation or a
/// detour - there is no owning Rust struct for this class family to redirect construction/destruction
/// onto, and both the copy constructor and the destructor's own tree teardown touch opaque,
/// allocator-shared helpers (`FUN_0040107f`/`FUN_00401118`/`AI_cls_0x404fd6::cls_0x404fd6` on the
/// construction side, `FUN_00401b16`/`FUN_005aade2` on the teardown side) with no independently-resolved
/// address anywhere in this corpus.
///
/// This is therefore not a real-vs-rust comparison (there is no rust pole) - it instead pins this
/// module's own documented understanding of the real default constructor's field defaults, and confirms
/// the copy constructor produces an independent tree/array allocation rather than aliasing the source's
/// own. Every step is a real vanilla address call over real vanilla-allocated memory
/// (`ztshowinfo_live_support::build_standalone_show_info_via_real_ctor`/
/// `build_standalone_show_info_copy_via_real_ctor`/`destroy_standalone_show_info_via_real_dtor`, all
/// `.original()`-backed) - no Rust allocator crossing anywhere, so unlike most of this class family's
/// other standalone fixtures, teardown here is a genuine free, not a leak-only one.
pub(crate) fn run_ztshowinfo_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_STANDALONE_ROUNDTRIP";
    let mut failures: Vec<String> = Vec::new();

    /// `ZTShowInfo`'s own real vtable RVA - see `ztshowinfo.rs`'s own `live_support::
    /// ZTSHOWINFO_VTABLE_RVA` (same value, kept as a private local copy for this test's own locality).
    const ZTSHOWINFO_VTABLE_RVA: u32 = 0x0023_53cc;
    let expected_vtable = crate::globals::get_module_base("zoo.exe") as u32 + ZTSHOWINFO_VTABLE_RVA;

    let cursor_before_default_ctor = get_from_memory::<i32>(globals().ztaimgr_ptr() as u32 + 0xec);
    let default_built = ztshowinfo_live_support::build_standalone_show_info_via_real_ctor();

    if get_from_memory::<u32>(default_built) != expected_vtable {
        failures.push(format!(
            "default ctor: vtable pointer {:#010x}, expected {:#010x}",
            get_from_memory::<u32>(default_built),
            expected_vtable
        ));
    }
    if get_from_memory::<i32>(default_built + 0x68) != 0x5a {
        failures.push(format!("default ctor: +0x68 (frequency) should be 0x5a, holds {}", get_from_memory::<i32>(default_built + 0x68)));
    }
    let expected_next_time = cursor_before_default_ctor + 0x5a;
    if get_from_memory::<i32>(default_built + 0x6c) != expected_next_time {
        failures.push(format!(
            "default ctor: +0x6c should be cursor({cursor_before_default_ctor}) + 0x5a = {expected_next_time}, holds {}",
            get_from_memory::<i32>(default_built + 0x6c)
        ));
    }
    for offset in [0x50u32, 0x54, 0x58, 0x5c, 0x60, 0x64] {
        let value = get_from_memory::<u32>(default_built + offset);
        if value != 0 {
            failures.push(format!("default ctor: +{offset:#x} should be zeroed, holds {value:#010x}"));
        }
    }
    let default_tree_header = get_from_memory::<u32>(default_built + 0x44);
    if default_tree_header == 0 {
        failures.push("default ctor: +0x44 (pending-scripts tree wrapper) should be non-null".to_string());
    }
    if get_from_memory::<u32>(default_built + 0x48) != 0 {
        failures.push(format!(
            "default ctor: +0x48 (cached node count) should be 0, holds {}",
            get_from_memory::<u32>(default_built + 0x48)
        ));
    }

    let copy_built = ztshowinfo_live_support::build_standalone_show_info_copy_via_real_ctor(default_built);

    if get_from_memory::<u32>(copy_built) != expected_vtable {
        failures.push(format!(
            "copy ctor: vtable pointer {:#010x}, expected {:#010x}",
            get_from_memory::<u32>(copy_built),
            expected_vtable
        ));
    }
    for (label, offset) in [("+0x68 (frequency)", 0x68u32), ("+0x6c (next time)", 0x6c), ("+0x70 (show info id)", 0x70)] {
        let source = get_from_memory::<u32>(default_built + offset);
        let copy = get_from_memory::<u32>(copy_built + offset);
        if source != copy {
            failures.push(format!("copy ctor: {label} did not copy from source ({source:#010x}), holds {copy:#010x}"));
        }
    }
    for offset in [0x50u32, 0x54, 0x58, 0x5c, 0x60, 0x64] {
        let value = get_from_memory::<u32>(copy_built + offset);
        if value != 0 {
            failures.push(format!("copy ctor: +{offset:#x} should be zeroed (source's own array was empty), holds {value:#010x}"));
        }
    }
    let copy_tree_header = get_from_memory::<u32>(copy_built + 0x44);
    if copy_tree_header == 0 {
        failures.push("copy ctor: +0x44 (pending-scripts tree wrapper) should be non-null".to_string());
    } else if copy_tree_header == default_tree_header {
        failures.push("copy ctor: +0x44 should be its own independent allocation, not aliasing the source's".to_string());
    }
    if get_from_memory::<u32>(copy_built + 0x48) != 0 {
        failures.push(format!(
            "copy ctor: +0x48 (cached node count) should be 0, holds {}",
            get_from_memory::<u32>(copy_built + 0x48)
        ));
    }

    ztshowinfo_live_support::destroy_standalone_show_info_via_real_dtor(copy_built);
    ztshowinfo_live_support::destroy_standalone_show_info_via_real_dtor(default_built);

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_CLEAR_PENDING_SCRIPT_TREE_LIVE` - Stage 8b Stage 1: [`clear_pending_script_tree`]'s fixed
/// teardown (see `ztshowinfo.rs`'s own doc comment on the cross-allocator bug this replaced - the two old
/// `OPERATOR_DELETE.original()` calls corrupted vanilla's own pool allocator for any pool-carved node).
/// Builds a standalone `ZTShowInfo`, inserts a couple of Rust-side (empty-list) pending-scripts nodes via
/// [`find_or_insert_pending_script_node`], then genuinely exercises the cross-allocator hazard directly: a
/// real, trick-eligible unit's own type-keyed node is pre-inserted the safe way first (same discipline
/// every other test in this file uses before touching this fixture's minimal tree header - see
/// `ZTSHOWINFO_STATUS_PREDICATES_LIVE`'s own doc comment), so the subsequent real, still-un-hooked-for-this-
/// call `ADD_UNIT_TO_LIST.original()` takes only the safe tree "found" branch and performs a genuine,
/// pool-carved list insert. Confirms the fixed teardown doesn't crash against that pool-carved node (the
/// old, buggy version either corrupted vanilla's pool allocator silently or crashed depending on Fault
/// Tolerant Heap state) and resets the tree to a genuinely empty state.
pub(crate) fn run_ztshowinfo_clear_pending_script_tree_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_CLEAR_PENDING_SCRIPT_TREE_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let Some((entity_ptr, _)) = find_real_trick_eligible_unit() else {
        write_success_line(failure_log, &format!("{} (skipped: no trick-eligible unit found in test zoo)", test_name));
        return false;
    };

    let show_info = ztshow_live_support::build_standalone_show_info();

    ztshow::find_or_insert_pending_script_node(show_info, 0x1111);
    ztshow::find_or_insert_pending_script_node(show_info, 0x2222);

    let entity_type_ptr = get_from_memory::<u32>(entity_ptr + 0x128);
    let unit_type_id = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };
    ztshow::find_or_insert_pending_script_node(show_info, unit_type_id);
    unsafe { ADD_UNIT_TO_LIST.original()(show_info as *const u32, entity_ptr as i32) };

    let node_count_before = ztshow::pending_script_node_count(show_info);
    if node_count_before == 0 {
        failures.push("expected at least one pending-scripts node before teardown".to_string());
    }

    clear_pending_script_tree(show_info);

    let header = get_from_memory::<u32>(show_info + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let leftmost = get_from_memory::<u32>(header + 8);
    let node_count_after = get_from_memory::<u32>(show_info + 0x48);
    if root != 0 {
        failures.push(format!("root should be reset to 0 after teardown, holds {root:#010x}"));
    }
    if leftmost != header {
        failures.push(format!("leftmost should reset to the header itself after teardown, holds {leftmost:#010x} (header={header:#010x})"));
    }
    if node_count_after != 0 {
        failures.push(format!("cached node count should reset to 0 after teardown, holds {node_count_after}"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_ADD_UNIT_LIVE` - Stage 8b Stage 3: `addUnit`/`addUnitToList`. Two real, standalone
/// `ZTShowInfo` instances (`ztshow_live_support::build_standalone_show_info`), a real trick-eligible unit's
/// type-keyed node pre-inserted on both (same discipline as every other test in this file touching this
/// fixture's tree - see `ZTSHOWINFO_STATUS_PREDICATES_LIVE`'s own doc comment): `.hooked()` on one,
/// `.original()` (debug trampoline to the pre-detour body) on the other. Confirms a first `addUnit` call
/// links the real, resolvable unit id into the type's own `+0x18` list identically on both poles, and that
/// a duplicate call is the no-op real vanilla's own idempotent walk-then-insert body makes it (see
/// `ztshowinfo.rs`'s own module doc comment).
pub(crate) fn run_ztshowinfo_add_unit_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_ADD_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let Some((entity_ptr, entity_id)) = find_real_trick_eligible_unit() else {
        write_success_line(failure_log, &format!("{} (skipped: no trick-eligible unit found in test zoo)", test_name));
        return false;
    };

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    let entity_type_ptr = get_from_memory::<u32>(entity_ptr + 0x128);
    let unit_type_id = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };
    let (rust_node, _) = ztshow::find_or_insert_pending_script_node(rust_info, unit_type_id);
    let (real_node, _) = ztshow::find_or_insert_pending_script_node(real_info, unit_type_id);

    fn list_contents(node: u32) -> Vec<u32> {
        let sentinel = get_from_memory::<u32>(node + 0x18);
        let mut values = Vec::new();
        let mut cursor = get_from_memory::<u32>(sentinel);
        while cursor != sentinel {
            values.push(get_from_memory::<u32>(cursor + 0x8));
            cursor = get_from_memory::<u32>(cursor);
        }
        values
    }

    let rust_result = unsafe { ADD_UNIT.hooked()(rust_info as *const u32, entity_ptr as i32) };
    let real_result = unsafe { ADD_UNIT.original()(real_info as *const u32, entity_ptr as i32) };
    if (rust_result != 0) != (real_result != 0) {
        failures.push(format!("addUnit mismatch: rust={rust_result:#x} real={real_result:#x}"));
    }
    let rust_list = list_contents(rust_node);
    let real_list = list_contents(real_node);
    if rust_list != real_list {
        failures.push(format!("after first addUnit: rust={rust_list:?} real={real_list:?}"));
    }
    if !rust_list.contains(&entity_id) {
        failures.push(format!("expected unit id {entity_id:#x} to be present after addUnit, got {rust_list:?}"));
    }

    // Idempotency: a duplicate addUnit call for the same unit must not change the list.
    let rust_result2 = unsafe { ADD_UNIT.hooked()(rust_info as *const u32, entity_ptr as i32) };
    let real_result2 = unsafe { ADD_UNIT.original()(real_info as *const u32, entity_ptr as i32) };
    if (rust_result2 != 0) != (real_result2 != 0) {
        failures.push(format!("duplicate addUnit mismatch: rust={rust_result2:#x} real={real_result2:#x}"));
    }
    let rust_list2 = list_contents(rust_node);
    let real_list2 = list_contents(real_node);
    if rust_list2 != rust_list {
        failures.push(format!("duplicate addUnit should be a no-op on the list: before={rust_list:?} after={rust_list2:?}"));
    }
    if rust_list2 != real_list2 {
        failures.push(format!("after duplicate addUnit: rust={rust_list2:?} real={real_list2:?}"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_GATHER_UNITS_LIVE` - Stage 8b Stage 4: `ZTShowInfo::gatherUnits`. Two real, standalone
/// `ZTShowInfo` instances, each populated identically via the now-verified [`add_unit_to_list`] (Stage 3)
/// with the same real, trick-eligible unit, so this genuinely exercises `gatherUnits`' own drain over a
/// real, pool-carved list rather than a synthetic one it never allocates itself: `.hooked()` on one,
/// `.original()` (debug trampoline to the pre-detour body) on the other.
///
/// **A miss (unregistered type) is checked first, and the populated-type call runs only once per pole** -
/// real vanilla's own decompile never re-derives the unit list's sentinel from the tree after draining it
/// (`node+0x18` is left holding the address of the now-freed sentinel, per [`crate::ztshowinfo::gather_units`]'s
/// own doc comment), so a second `gatherUnits` call against the same already-drained type would walk freed
/// memory on both poles - not exercised here for the same reason this file avoids ever exercising a
/// known-unsafe-against-the-fixture branch (see `ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE`'s own doc comment for
/// the precedent).
pub(crate) fn run_ztshowinfo_gather_units_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_GATHER_UNITS_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let Some((entity_ptr, _)) = find_real_trick_eligible_unit() else {
        write_success_line(failure_log, &format!("{} (skipped: no trick-eligible unit found in test zoo)", test_name));
        return false;
    };

    let rust_info = ztshow_live_support::build_standalone_show_info();
    let real_info = ztshow_live_support::build_standalone_show_info();

    let entity_type_ptr = get_from_memory::<u32>(entity_ptr + 0x128);
    let unit_type_id = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };

    add_unit_to_list(rust_info, entity_ptr);
    add_unit_to_list(real_info, entity_ptr);

    // Masked to the low byte: real vanilla's own asm (`ZTShowInfo_gatherUnits.asm`) only ever writes `AL`
    // (`XOR BL,BL` up front, `MOV AL,BL` on exit) - on the miss path in particular, EAX still holds
    // `AI_cls_0x404fd6::find`'s own leftover pointer value in its upper 24 bits, an undefined-upper-bits
    // decompiler artifact (same class as `ZooStatus::fChance`'s/`getScheduledShowScript`'s own, both
    // already documented elsewhere in this file) rather than real data - confirmed live: this comparison
    // failed unmasked the first time this test ran (`rust=0x0 real=0x1afa00`).
    const MISSING_TYPE_ID: u32 = 0x7fff_ffff;
    let rust_miss = unsafe { GATHER_UNITS.hooked()(rust_info as *const u32, MISSING_TYPE_ID) } & 0xff;
    let real_miss = unsafe { GATHER_UNITS.original()(real_info as *const u32, MISSING_TYPE_ID) } & 0xff;
    if rust_miss != real_miss {
        failures.push(format!("gatherUnits(missing type) mismatch: rust={rust_miss:#x} real={real_miss:#x}"));
    }
    if rust_miss != 0 {
        failures.push("gatherUnits should be false for a type with no pending-scripts node".to_string());
    }

    let rust_result = unsafe { GATHER_UNITS.hooked()(rust_info as *const u32, unit_type_id) } & 0xff;
    let real_result = unsafe { GATHER_UNITS.original()(real_info as *const u32, unit_type_id) } & 0xff;
    if rust_result != real_result {
        failures.push(format!("gatherUnits(populated type) mismatch: rust={rust_result:#x} real={real_result:#x}"));
    }
    if rust_result == 0 {
        failures.push("expected gatherUnits to find at least one trick-eligible unit".to_string());
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWINFO_UPDATE_FROM_LOAD_LIVE` - Stage 8b Stage 2: `updateFromLoad`/`ZTShow::operator_assign`. Four
/// real, real-ctor-backed `ZTShowInfo` instances (`ztshowinfo_live_support::build_standalone_show_info_via_
/// real_ctor` - needed here, not the lighter `ztshow_live_support::build_standalone_show_info`, because
/// `update_from_load`'s own script-state-tree half needs a genuinely real, allocated `ZTShowState` header,
/// which only the real constructor sets up): a `(dest, source)` pair per pole, `.hooked()` on one pair,
/// `.original()` (debug trampoline to the pre-detour body) on the other.
///
/// Both `source` fixtures are populated identically (a couple of scalar fields, one registered unit type,
/// one pending-scripts node with distinct value fields, one script-state node with a distinct value byte)
/// and both `dest` fixtures are pre-seeded with a *different* script-state node before the call, so this
/// exercises both halves the module doc comment's own Stage 11 section describes: the pending-scripts
/// tree's find-or-insert **merge** (dest's own pre-existing entries, if any, survive alongside source's)
/// and the script-state tree's clear-then-rebuild **replace** (dest's pre-seeded node must be gone
/// afterward, not merged in).
///
/// **Registers/unregisters through the real `SHOW_STORE`** (`update_from_load`'s own final step) - the
/// post-call ids are captured and explicitly unregistered before teardown, so this test leaves no stale
/// entries behind for `ZTSHOWMGR_REGISTER_UNREGISTER_SHOW`/`ZTSHOWMGR_REAL_ZOO_STORE_CONSISTENCY_LIVE` (both
/// registered under `live_zoo_tests`) to trip over.
pub(crate) fn run_ztshowinfo_update_from_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_UPDATE_FROM_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();

    if globals().ztshowmgr_ptr().is_null() {
        write_success_line(failure_log, &format!("{} (skipped: GLOBAL_ZTShowMgr not initialized)", test_name));
        return false;
    }

    let rust_dest = ztshowinfo_live_support::build_standalone_show_info_via_real_ctor();
    let rust_source = ztshowinfo_live_support::build_standalone_show_info_via_real_ctor();
    let real_dest = ztshowinfo_live_support::build_standalone_show_info_via_real_ctor();
    let real_source = ztshowinfo_live_support::build_standalone_show_info_via_real_ctor();

    fn state_header(show_info: u32) -> u32 {
        get_from_memory::<u32>(show_info + 0x38)
    }

    const SOURCE_PENDING_KEY: u32 = 0x5555;
    const SOURCE_STATE_KEY: u32 = 0x77;
    const DEST_STALE_STATE_KEY: u32 = 0xdead;

    for source in [rust_source, real_source] {
        save_to_memory(source + 0x68, 7i32);
        save_to_memory(source + 0xa4, 2i32);
        add_show(source, 0x1234);

        let (node, _) = find_or_insert_pending_script_node(source, SOURCE_PENDING_KEY);
        save_to_memory(node + 0x1c, 0x11u16);
        save_to_memory(node + 0x28, 12.5f32);
        save_to_memory(node + 0x34, 42i32);

        let value = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
        save_to_memory(value, 0u32);
        save_to_memory(value + 0x8, 0x99u32);
        let state_node = ztshowstate::find_or_insert_state_node(state_header(source), SOURCE_STATE_KEY);
        save_to_memory(state_node + 0x14, value);
    }

    for dest in [rust_dest, real_dest] {
        let old_value = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
        save_to_memory(old_value, 0u32);
        let old_node = ztshowstate::find_or_insert_state_node(state_header(dest), DEST_STALE_STATE_KEY);
        save_to_memory(old_node + 0x14, old_value);
    }

    unsafe { UPDATE_FROM_LOAD.hooked()(rust_dest as *const u32, rust_source as *const u32) };
    unsafe { UPDATE_FROM_LOAD.original()(real_dest as *const u32, real_source as *const u32) };

    if get_from_memory::<i32>(rust_dest + 0x68) != get_from_memory::<i32>(real_dest + 0x68) {
        failures.push("+0x68 (frequency) mismatch after updateFromLoad".to_string());
    }
    if get_from_memory::<i32>(rust_dest + 0x68) != 7 {
        failures.push(format!("+0x68 should be 7 after merging source, holds {}", get_from_memory::<i32>(rust_dest + 0x68)));
    }
    if get_from_memory::<i32>(rust_dest + 0xa4) != get_from_memory::<i32>(real_dest + 0xa4) {
        failures.push("+0xa4 (schedule cursor) mismatch after updateFromLoad".to_string());
    }

    fn array_contents(base: u32) -> Vec<u32> {
        let begin = get_from_memory::<u32>(base + 0x50);
        let end = get_from_memory::<u32>(base + 0x54);
        let mut values = Vec::new();
        let mut cursor = begin;
        while cursor != end {
            values.push(get_from_memory::<u32>(cursor));
            cursor += 4;
        }
        values
    }
    let rust_array = array_contents(rust_dest);
    let real_array = array_contents(real_dest);
    if rust_array != real_array {
        failures.push(format!("registered-unit-types array mismatch: rust={rust_array:?} real={real_array:?}"));
    }
    if rust_array != vec![0x1234] {
        failures.push(format!("registered-unit-types array should be [0x1234] after merging source, got {rust_array:?}"));
    }

    let (rust_pending_node, rust_pending_new) = find_or_insert_pending_script_node(rust_dest, SOURCE_PENDING_KEY);
    let (real_pending_node, real_pending_new) = find_or_insert_pending_script_node(real_dest, SOURCE_PENDING_KEY);
    if rust_pending_new || real_pending_new {
        failures.push("expected source's pending-scripts node to already exist on dest after updateFromLoad".to_string());
    }
    for (label, offset) in [("+0x1c", 0x1cu32), ("+0x28", 0x28), ("+0x34", 0x34)] {
        let rust_value = get_from_memory::<u32>(rust_pending_node + offset);
        let real_value = get_from_memory::<u32>(real_pending_node + offset);
        if rust_value != real_value {
            failures.push(format!("pending-scripts node {label} mismatch: rust={rust_value:#x} real={real_value:#x}"));
        }
    }
    if get_from_memory::<u16>(rust_pending_node + 0x1c) != 0x11 {
        failures.push(format!(
            "pending-scripts node +0x1c should be 0x11 after merging source, holds {:#x}",
            get_from_memory::<u16>(rust_pending_node + 0x1c)
        ));
    }

    fn state_keys(show_info: u32) -> Vec<u32> {
        let header = state_header(show_info);
        let root = get_from_memory::<u32>(header + 0x4);
        let mut nodes = Vec::new();
        ztshowstate::collect_tree_nodes(root, &mut nodes);
        nodes.into_iter().map(|node| get_from_memory::<u32>(node + 0x10)).collect()
    }
    let rust_keys = state_keys(rust_dest);
    let real_keys = state_keys(real_dest);
    if rust_keys != real_keys {
        failures.push(format!("script-state tree key set mismatch: rust={rust_keys:?} real={real_keys:?}"));
    }
    if rust_keys.contains(&DEST_STALE_STATE_KEY) {
        failures.push(format!("script-state tree should have been cleared, not merged - stale key still present: {rust_keys:?}"));
    }
    if !rust_keys.contains(&SOURCE_STATE_KEY) {
        failures.push(format!("script-state tree should contain source's own key {SOURCE_STATE_KEY:#x}, got {rust_keys:?}"));
    }

    let rust_state_header = state_header(rust_dest);
    let real_state_header = state_header(real_dest);
    let rust_state_node = ztshowstate::find_or_insert_state_node(rust_state_header, SOURCE_STATE_KEY);
    let real_state_node = ztshowstate::find_or_insert_state_node(real_state_header, SOURCE_STATE_KEY);
    let rust_value_ptr = get_from_memory::<u32>(rust_state_node + 0x14);
    let real_value_ptr = get_from_memory::<u32>(real_state_node + 0x14);
    if rust_value_ptr == 0 || real_value_ptr == 0 {
        failures.push("expected a cloned script-state value object on both poles".to_string());
    } else {
        let rust_source_node = ztshowstate::find_or_insert_state_node(state_header(rust_source), SOURCE_STATE_KEY);
        let rust_source_value = get_from_memory::<u32>(rust_source_node + 0x14);
        if rust_value_ptr == rust_source_value {
            failures.push("dest's cloned value must not alias source's own value pointer".to_string());
        }
        let rust_byte = get_from_memory::<u32>(rust_value_ptr + 0x8);
        let real_byte = get_from_memory::<u32>(real_value_ptr + 0x8);
        if rust_byte != real_byte || rust_byte != 0x99 {
            failures.push(format!("cloned script-state value +0x8 mismatch: rust={rust_byte:#x} real={real_byte:#x} (want 0x99)"));
        }
    }

    // Cleanup: unregister whatever id each dest ended up with (updateFromLoad's own final step always
    // registers dest, assigning a fresh id since both dest/source start at id 0), so this test leaves the
    // real SHOW_STORE exactly as it found it.
    for dest in [rust_dest, real_dest] {
        let id = get_from_memory::<u16>(dest + 0x70);
        if id != 0 {
            let mgr = unsafe { mut_from_memory::<ZTShowMgr>(globals().ztshowmgr_ptr() as u32) };
            mgr.unregister_show(id, dest as *const u32, false);
        }
    }

    ztshowinfo_live_support::destroy_standalone_show_info_via_real_dtor(rust_dest);
    ztshowinfo_live_support::destroy_standalone_show_info_via_real_dtor(rust_source);
    ztshowinfo_live_support::destroy_standalone_show_info_via_real_dtor(real_dest);
    ztshowinfo_live_support::destroy_standalone_show_info_via_real_dtor(real_source);

    finish_test(test_name, failures, failure_log)
}
