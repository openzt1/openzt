//! Compares real vanilla `ZTShowInfo` methods against the Rust reimplementations in production file
//! `openzt/src/ztshowinfo.rs`: the detour-wiring check, then comparison tests split by real-dependency
//! availability - the pending-scripts-tree-only groups (status predicates, accumulators) run in
//! `always_late_tests`, while the groups needing a live, `run_load_live_zoo`-populated global
//! (`GLOBAL_ZTWorldMgr` for the keeper predicates, `GLOBAL_ZTAIMgr` for the schedule/frequency pair)
//! run in `live_zoo_tests` - see `ztshowinfo.rs`'s own module doc comment for the per-method reasons.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::ztshowinfo::{
    ADD_SHOW, CLEANUP_EVENTS, CREATE_DEFAULT_SCRIPT, GET_SCHEDULED_SHOW_KEEPER_TYPE, GET_SCHEDULED_SHOW_SCRIPT, HAS_KEEPER,
    INCREMENT_ATTENDANCE, INCREMENT_RECEIPTS, IS_READY, IS_STARTED, IS_STOPPED, LISTEN, NEEDS_KEEPER, RECALCULATE_SCHEDULE,
    REMOVE_SHOW, SEND_EVENT, SET_SHOW_FREQUENCY,
};

use crate::globals::globals;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::util::{get_from_memory, save_to_memory};
use crate::ztshow::{self, live_support as ztshow_live_support};
use crate::ztshowinfo::live_support as ztshowinfo_live_support;

use super::ztshow::find_real_show_tank_habitat;

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
/// on one, `.original()`/`GET_EVENTS_FIXED.original()` (debug trampoline to the pre-detour body) on the
/// other.
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
    // `GET_EVENTS_FIXED.original()`'s corrected 2-arg signature.
    unsafe { crate::ztshowinfo::GET_EVENTS_FIXED.original()(real_info as *const u32, (real_info + 0x5c) as *const u32) };

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
