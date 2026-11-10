//! Compares real vanilla `ZTShowScriptState` against its Rust reimplementation (production file
//! `openzt/src/ztshowscriptstate.rs`): the detour-wiring check, standalone `init`, a save/load
//! round-trip, `setNextItem` behavior over the representative in-range/out-of-range/empty-script
//! cases, and the create/find-or-insert entry point (`generated.rs`'s mislabeled
//! `ztshowscriptstate::CONSTRUCTOR`, really `ZTShow::createShowScriptState`) against a synthetic tree.
//! Script states are the `0x14`-byte values of the tree at `ZTShowState`+`0x1c` - see
//! `crate::ztshowscriptstate::live_support` for the standalone fixtures.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::standalone;
use openzt_detour::generated::ztshowscriptstate::{
    CONSTRUCTOR as ZTSHOWSCRIPTSTATE_CONSTRUCTOR, INIT as ZTSHOWSCRIPTSTATE_INIT, LOAD as ZTSHOWSCRIPTSTATE_LOAD,
    SAVE as ZTSHOWSCRIPTSTATE_SAVE, SET_NEXT_ITEM_0 as ZTSHOWSCRIPTSTATE_SET_NEXT_ITEM_0,
    SET_NEXT_ITEM_1 as ZTSHOWSCRIPTSTATE_SET_NEXT_ITEM_1,
};

use crate::globals::get_module_base;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::tests::ztshowscriptmgr::{add_matching_item, make_registered_show_script};
use crate::util::{get_from_memory, save_to_memory};
use crate::ztshowscriptmgr;
use crate::ztshowscriptstate::live_support as ztshowscriptstate_live_support;
use crate::ztshowscriptstate::RVA_SCRIPT_STATE_VTABLE;

/// `ZTSHOWSCRIPTSTATE_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `ztshowscriptstate::init()`, and this asserts all seven of its detours actually report enabled
/// (same rationale as the MenuMusicHandler/ZTSoundscape/ZooStatus/ZTShowState blocks in that `init()`
/// function - see each of their own `_DETOURS_ENABLED` tests). A comparison test alone can't catch
/// the missing-wiring gap: real vanilla and this port happen to agree on the cases exercised, so a
/// silently un-detoured address makes every `.hooked()` call fall through to vanilla and still pass.
pub(crate) fn run_ztshowscriptstate_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTSTATE_DETOURS_ENABLED";
    let mut disabled: Vec<&'static str> = Vec::new();
    for (name, enabled) in ztshowscriptstate_live_support::detour_status() {
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

/// Byte-for-byte cross-pole comparison over the fixture's full `0x14` extent - vtable included,
/// since both poles must leave `+0x0` untouched and both build from identically stamped fixtures.
fn push_field_diffs(failures: &mut Vec<String>, rust_state: u32, real_state: u32) {
    for off in 0x0..0x14 {
        let rust = get_from_memory::<u8>(rust_state + off);
        let real = get_from_memory::<u8>(real_state + off);
        if rust != real {
            failures.push(format!(
                "+{off:#04x} mismatch between rust and real poles (rust {rust:#04x}, real {real:#04x})"
            ));
        }
    }
}

/// Raw byte snapshot of a fixture's full `0x14` extent - the poison round-trip oracle.
fn snapshot(state: u32) -> Vec<u8> {
    (0x0..0x14).map(|off| get_from_memory::<u8>(state + off)).collect()
}

/// `ZTSHOWSCRIPTSTATE_INIT` - `init` is this class's one vtable slot. Builds two standalone
/// fixtures, poisons every field `init` is supposed to reset on both (plus the never-read `+0x6`
/// padding and all six flags), runs the Rust port (`.hooked()`) on one and the real vanilla body
/// (through the `test_real` trampoline) on the other, then compares every byte of both fixtures
/// against each other and against vanilla's own documented field list. A pole touching the padding
/// or the vtable - or missing any flag - diverges from the other byte-for-byte.
pub(crate) fn run_ztshowscriptstate_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTSTATE_INIT";
    let mut failures: Vec<String> = Vec::new();

    let rust_state = ztshowscriptstate_live_support::build_standalone_script_state();
    let real_state = ztshowscriptstate_live_support::build_standalone_script_state();

    fn poison(state: u32) {
        save_to_memory(state + 0x4, 0x1234u16);
        save_to_memory(state + 0x6, 0x55u16);
        save_to_memory(state + 0x8, 0xdead_f00du32);
        save_to_memory(state + 0xc, 0xa5a5u16);
        save_to_memory(state + 0xe, 0x11u8);
        save_to_memory(state + 0xf, 0x22u8);
        save_to_memory(state + 0x10, 0x33u8);
        save_to_memory(state + 0x11, 0x44u8);
        save_to_memory(state + 0x12, 0x55u8);
        save_to_memory(state + 0x13, 0x66u8);
    }
    poison(rust_state);
    poison(real_state);

    let rust_vtable_before = get_from_memory::<u32>(rust_state);
    let real_vtable_before = get_from_memory::<u32>(real_state);

    unsafe {
        ZTSHOWSCRIPTSTATE_INIT.hooked()(rust_state as *const u32);
        ztshowscriptstate_live_support::real_init(real_state);
    }

    push_field_diffs(&mut failures, rust_state, real_state);

    for (pole, state) in [("rust", rust_state), ("real", real_state)] {
        if get_from_memory::<u16>(state + 0x4) != 0 {
            failures.push(format!(
                "{pole} pole: +0x4 should reset to 0, got {:#06x}",
                get_from_memory::<u16>(state + 0x4)
            ));
        }
        if get_from_memory::<u32>(state + 0x8) != 0 {
            failures.push(format!(
                "{pole} pole: +0x8 should reset to 0, got {:#010x}",
                get_from_memory::<u32>(state + 0x8)
            ));
        }
        if get_from_memory::<u16>(state + 0xc) != 0xffff {
            failures.push(format!(
                "{pole} pole: +0xc should reset to the 0xffff \"no item assigned\" sentinel, got {:#06x}",
                get_from_memory::<u16>(state + 0xc)
            ));
        }
        for (off, name) in [(0xe, "+0xe"), (0xf, "+0xf"), (0x10, "+0x10"), (0x11, "+0x11"), (0x12, "+0x12"), (0x13, "+0x13")] {
            if get_from_memory::<u8>(state + off) != 0 {
                failures.push(format!("{pole} pole: {name} flag should reset to 0, got {:#04x}", get_from_memory::<u8>(state + off)));
            }
        }
        if get_from_memory::<u16>(state + 0x6) != 0x55 {
            failures.push(format!("{pole} pole: init must not touch the never-read +0x6 padding"));
        }
    }
    if get_from_memory::<u32>(rust_state) != rust_vtable_before {
        failures.push("rust pole: init must not touch the vtable pointer (+0x0)".to_string());
    }
    if get_from_memory::<u32>(real_state) != real_vtable_before {
        failures.push("real pole: init touched the vtable pointer (+0x0) - contradicts this port's own understanding of vanilla init".to_string());
    }

    ztshowscriptstate_live_support::destroy_standalone_script_state(rust_state);
    ztshowscriptstate_live_support::destroy_standalone_script_state(real_state);

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWSCRIPTSTATE_SAVE_LOAD_ROUNDTRIP` - poisons two identical fixtures with the exact nine
/// fields `save` writes/`load` reads (script_id, key, trick_index, then the six flags), captures
/// each pole's `save` stream via `io_redirect` (real vanilla save calls the same raw
/// `WRITE_BYTES_TO_FILE` address the redirect detours, so both poles land in the capture), asserts
/// the two streams are byte-for-byte identical, then replays each pole's own stream into a fresh
/// fixture through that pole's `load` and checks every byte round-trips. Also pins the shared
/// save/load version gate: at `version == 0x60` the guard skips the whole body - load reports
/// success, consumes nothing (empty replay buffer proves no read happened), and leaves every field
/// poisoned. `save` returns are compared in the low byte only - vanilla's `CONCAT31` return
/// construction leaves the upper 3 bytes as register garbage.
pub(crate) fn run_ztshowscriptstate_save_load_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTSTATE_SAVE_LOAD_ROUNDTRIP";
    let mut failures: Vec<String> = Vec::new();
    const CURRENT_VERSION: u32 = 0x100;

    let dummy_file: u32 = 0;
    let file_ptr = &dummy_file as *const u32;

    fn poison(state: u32) {
        save_to_memory(state + 0x4, 0x1234u16);
        save_to_memory(state + 0x8, 0xdead_f00du32);
        save_to_memory(state + 0xc, 0x0005u16);
        save_to_memory(state + 0xe, 0u8);
        save_to_memory(state + 0xf, 1u8);
        save_to_memory(state + 0x10, 0u8);
        save_to_memory(state + 0x11, 1u8);
        save_to_memory(state + 0x12, 0u8);
        save_to_memory(state + 0x13, 1u8);
    }

    let save_source_real = ztshowscriptstate_live_support::build_standalone_script_state();
    let save_source_rust = ztshowscriptstate_live_support::build_standalone_script_state();
    poison(save_source_real);
    poison(save_source_rust);
    let source_bytes = snapshot(save_source_rust);

    io_redirect::begin_capture();
    let real_save_ret = ztshowscriptstate_live_support::real_save(save_source_real, file_ptr);
    let real_bytes = io_redirect::end_capture();
    io_redirect::begin_capture();
    let rust_save_ret = unsafe { ZTSHOWSCRIPTSTATE_SAVE.hooked()(save_source_rust as *const u32, file_ptr) };
    let rust_bytes = io_redirect::end_capture();

    if !real_save_ret {
        failures.push("real save should return true".to_string());
    }
    if !rust_save_ret {
        failures.push("rust save should return true".to_string());
    }
    // The exact nine fields: 2 (script_id) + 4 (key) + 2 (trick_index) + 6x1 (flags).
    if real_bytes.len() != 14 {
        failures.push(format!("real save should write 14 bytes, got {}", real_bytes.len()));
    }
    if rust_bytes != real_bytes {
        failures.push(format!(
            "rust and real save streams differ: rust {rust_bytes:02x?}, real {real_bytes:02x?}"
        ));
    }

    ztshowscriptstate_live_support::destroy_standalone_script_state(save_source_real);
    ztshowscriptstate_live_support::destroy_standalone_script_state(save_source_rust);

    let load_target_real = ztshowscriptstate_live_support::build_standalone_script_state();
    let load_target_rust = ztshowscriptstate_live_support::build_standalone_script_state();

    io_redirect::begin_replay(real_bytes);
    let real_load_ret = ztshowscriptstate_live_support::real_load(load_target_real, file_ptr, CURRENT_VERSION);
    let real_consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    io_redirect::begin_replay(rust_bytes);
    let rust_load_ret = unsafe { ZTSHOWSCRIPTSTATE_LOAD.hooked()(load_target_rust as *const u32, file_ptr, CURRENT_VERSION) };
    let rust_consumed = io_redirect::replay_position();
    io_redirect::end_replay();

    if !real_load_ret {
        failures.push("real load should return true".to_string());
    }
    if !rust_load_ret {
        failures.push("rust load should return true".to_string());
    }
    if real_consumed != 14 {
        failures.push(format!("real load should consume all 14 bytes, got {real_consumed}"));
    }
    if rust_consumed != 14 {
        failures.push(format!("rust load should consume all 14 bytes, got {rust_consumed}"));
    }
    push_field_diffs(&mut failures, load_target_rust, load_target_real);
    for (pole, state) in [("rust", load_target_rust), ("real", load_target_real)] {
        let loaded = snapshot(state);
        if loaded != source_bytes {
            failures.push(format!("{pole} pole: loaded struct does not match the saved source bytes: {loaded:02x?}"));
        }
    }

    ztshowscriptstate_live_support::destroy_standalone_script_state(load_target_real);
    ztshowscriptstate_live_support::destroy_standalone_script_state(load_target_rust);

    // Version gate: `version <= 0x60` is vanilla's own `if (0x60 < version)` guard around the whole
    // body - load reports success without reading a single byte (the empty replay buffer plus the
    // position check prove no read happened) and leaves every field poisoned.
    let gate_real = ztshowscriptstate_live_support::build_standalone_script_state();
    let gate_rust = ztshowscriptstate_live_support::build_standalone_script_state();
    poison(gate_real);
    poison(gate_rust);

    io_redirect::begin_replay(Vec::new());
    let gate_real_ret = ztshowscriptstate_live_support::real_load(gate_real, file_ptr, 0x60);
    let gate_real_consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    io_redirect::begin_replay(Vec::new());
    let gate_rust_ret = unsafe { ZTSHOWSCRIPTSTATE_LOAD.hooked()(gate_rust as *const u32, file_ptr, 0x60) };
    let gate_rust_consumed = io_redirect::replay_position();
    io_redirect::end_replay();

    if !gate_real_ret {
        failures.push("real load at version 0x60 should return true without reading".to_string());
    }
    if !gate_rust_ret {
        failures.push("rust load at version 0x60 should return true without reading".to_string());
    }
    if gate_real_consumed != 0 {
        failures.push(format!("real load at version 0x60 must not read anything, consumed {gate_real_consumed} bytes"));
    }
    if gate_rust_consumed != 0 {
        failures.push(format!("rust load at version 0x60 must not read anything, consumed {gate_rust_consumed} bytes"));
    }
    push_field_diffs(&mut failures, gate_rust, gate_real);
    for (pole, state) in [("rust", gate_rust), ("real", gate_real)] {
        if snapshot(state) != source_bytes {
            failures.push(format!("{pole} pole: version-0x60 load must leave every field poisoned"));
        }
    }

    ztshowscriptstate_live_support::destroy_standalone_script_state(gate_real);
    ztshowscriptstate_live_support::destroy_standalone_script_state(gate_rust);

    finish_test(test_name, failures, failure_log)
}

/// Distinct nonzero poison on every field a `setNextItem` call might touch or clear: `script_id` set
/// to the case's registered script id so both poles' `get_num_items` resolves the same count,
/// `trick_index` to a value only the rejected cases expect to survive, and the five clearable flags
/// each their own value so "cleared exactly `+0xe`-`+0x12`" is checkable per byte.
fn poison_for_set_next_item(state: u32, script_id: u16, trick_index: u16) {
    save_to_memory(state + 0x4, script_id);
    save_to_memory(state + 0x8, 0x0bad_c0deu32);
    save_to_memory(state + 0xc, trick_index);
    save_to_memory(state + 0xe, 0x11u8);
    save_to_memory(state + 0xf, 0x22u8);
    save_to_memory(state + 0x10, 0x33u8);
    save_to_memory(state + 0x11, 0x44u8);
    save_to_memory(state + 0x12, 0x55u8);
    save_to_memory(state + 0x13, 1u8);
}

/// Per-pole flag-block expectations after a `setNextItem` call: a successful assignment clears
/// exactly `+0xe`-`+0x12`, a rejected one leaves them poisoned, and `flag_f` (`+0x13`) stays 1 on
/// every path - the one flag only `init` resets.
fn check_flag_block(failures: &mut Vec<String>, case: &str, pole: &str, state: u32, cleared: bool) {
    for (off, poison) in [(0xe, 0x11u8), (0xf, 0x22), (0x10, 0x33), (0x11, 0x44), (0x12, 0x55)] {
        let actual = get_from_memory::<u8>(state + off);
        let expected = if cleared { 0 } else { poison };
        if actual != expected {
            failures.push(format!(
                "{case} ({pole} pole): flag +{off:#04x} should be {}, got {actual:#04x}",
                if cleared { "cleared" } else { "poisoned" }
            ));
        }
    }
    if get_from_memory::<u8>(state + 0x13) != 1 {
        failures.push(format!("{case} ({pole} pole): +0x13 (flag_f) must stay 1 - setNextItem never touches it"));
    }
}

/// One `setNextItem(u16)` case on a fresh fixture pair: rust pole via `SET_NEXT_ITEM_0.hooked()`,
/// real pole via the `test_real` trampoline; compares return values and full post-state bytes, then
/// the case's own trick_index/flag-block expectations.
fn run_set_next_item_case(
    failures: &mut Vec<String>,
    script_id: u16,
    case: &str,
    requested: u16,
    expected_ret: u32,
    expected_trick_index: u16,
    clears_flags: bool,
) {
    let rust_state = ztshowscriptstate_live_support::build_standalone_script_state();
    let real_state = ztshowscriptstate_live_support::build_standalone_script_state();
    poison_for_set_next_item(rust_state, script_id, 0xa5a5);
    poison_for_set_next_item(real_state, script_id, 0xa5a5);

    let rust_ret = unsafe { ZTSHOWSCRIPTSTATE_SET_NEXT_ITEM_0.hooked()(rust_state as *const u32, requested) };
    let real_ret = ztshowscriptstate_live_support::real_set_next_item(real_state, requested);

    if rust_ret != real_ret || rust_ret != expected_ret {
        failures.push(format!("{case}: return mismatch (expected {expected_ret}, rust {rust_ret}, real {real_ret})"));
    }
    push_field_diffs(failures, rust_state, real_state);
    for (pole, state) in [("rust", rust_state), ("real", real_state)] {
        let trick_index = get_from_memory::<u16>(state + 0xc);
        if trick_index != expected_trick_index {
            failures.push(format!(
                "{case} ({pole} pole): trick_index should be {expected_trick_index:#06x}, got {trick_index:#06x}"
            ));
        }
        check_flag_block(failures, case, pole, state, clears_flags);
    }

    ztshowscriptstate_live_support::destroy_standalone_script_state(rust_state);
    ztshowscriptstate_live_support::destroy_standalone_script_state(real_state);
}

/// `ZTSHOWSCRIPTSTATE_SET_NEXT_ITEM` - behavior comparison over the representative cases: the
/// one-based "assigned item" encoding, the out-of-range and empty-script rejections, the `0xffff`
/// "assign first" request, and the `()`-returning overload (whose "requested" value is the fixture's
/// own pre-set trick_index; it has no return value, so the post-state is the whole contract). Both
/// poles resolve item counts through the same hooked `GET_NUM_ITEMS` address into the shared
/// ztshowscriptmgr store (real setNextItem calls getNumItems as a real call), so counts are
/// controlled purely by what's registered there. Registered scripts coexist with the rest of the
/// battery's own registrations by unique id - no reset_state.
pub(crate) fn run_ztshowscriptstate_set_next_item_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTSTATE_SET_NEXT_ITEM";
    let mut failures: Vec<String> = Vec::new();

    const SCRIPT_TYPE: u32 = 0x7a11;
    let script_id = make_registered_show_script(SCRIPT_TYPE, 0x100);
    add_matching_item(ztshowscriptmgr::get_script(script_id), SCRIPT_TYPE, 0x101);
    add_matching_item(ztshowscriptmgr::get_script(script_id), SCRIPT_TYPE, 0x102);

    run_set_next_item_case(&mut failures, script_id, "requested 0 (first item)", 0, 0, 1, true);
    run_set_next_item_case(&mut failures, script_id, "requested n-1", 2, 0, 3, true);
    run_set_next_item_case(&mut failures, script_id, "requested n (out of range)", 3, 7, 0xa5a5, false);
    run_set_next_item_case(&mut failures, script_id, "requested 0xffff (assign first)", 0xffff, 0, 0, true);

    // A 0-item script: any request is rejected with 6 and nothing is touched.
    let empty_alloc = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
    let empty_id = ztshowscriptmgr::register_script(empty_alloc, SCRIPT_TYPE)
        .expect("register_script should never reject a non-null ctor_ptr");
    run_set_next_item_case(&mut failures, empty_id, "empty script, requested 0", 0, 6, 0xa5a5, false);
    run_set_next_item_case(&mut failures, empty_id, "empty script, requested 0xffff", 0xffff, 6, 0xa5a5, false);

    for (current, expected_trick_index, clears) in [(1u16, 2u16, true), (0xffff, 0, true), (3, 3, false)] {
        let case = format!("overload, current trick_index {current:#06x}");
        let rust_state = ztshowscriptstate_live_support::build_standalone_script_state();
        let real_state = ztshowscriptstate_live_support::build_standalone_script_state();
        poison_for_set_next_item(rust_state, script_id, current);
        poison_for_set_next_item(real_state, script_id, current);

        unsafe {
            ZTSHOWSCRIPTSTATE_SET_NEXT_ITEM_1.hooked()(rust_state as *const u32);
        }
        ztshowscriptstate_live_support::real_set_next_item_current(real_state);

        push_field_diffs(&mut failures, rust_state, real_state);
        for (pole, state) in [("rust", rust_state), ("real", real_state)] {
            let trick_index = get_from_memory::<u16>(state + 0xc);
            if trick_index != expected_trick_index {
                failures.push(format!(
                    "{case} ({pole} pole): trick_index should be {expected_trick_index:#06x}, got {trick_index:#06x}"
                ));
            }
            check_flag_block(&mut failures, &case, pole, state, clears);
        }

        ztshowscriptstate_live_support::destroy_standalone_script_state(rust_state);
        ztshowscriptstate_live_support::destroy_standalone_script_state(real_state);
    }

    finish_test(test_name, failures, failure_log)
}

/// In-order walk of the script-state tree rooted at `header`'s own root cache (`header+0x4`),
/// returning `(node, key)` pairs in ascending-key order. Guards against *both* a null child and a
/// child that circles back to `header` before terminating - same dual condition
/// `ztshow.rs`'s `run_ztshow_pending_script_tree_real_zoo_integrity_live_test` uses for a sibling
/// tree: this port's own writes always use a null leaf terminator, but real vanilla's genuine STL
/// insert-with-hint routine (the real pole's tree here) may use the classic MSVC circular-sentinel
/// convention instead, so checking only one or the other risks an infinite loop on whichever pole
/// guessed wrong.
fn in_order_keys(header: u32) -> Vec<(u32, u32)> {
    const MAX_NODES: usize = 1000;
    let mut out = Vec::new();
    let mut stack: Vec<u32> = Vec::new();
    let mut node = get_from_memory::<u32>(header + 0x4);
    let mut iterations = 0usize;
    while (node != 0 && node != header) || !stack.is_empty() {
        iterations += 1;
        if iterations > MAX_NODES {
            break;
        }
        if node != 0 && node != header {
            stack.push(node);
            node = get_from_memory::<u32>(node + 0x8); // left
        } else if let Some(top) = stack.pop() {
            out.push((top, get_from_memory::<u32>(top + 0x10)));
            node = get_from_memory::<u32>(top + 0xc); // right
        }
    }
    out
}

/// Finds the node holding `key` via [`in_order_keys`] - simple and sufficient for this test's tiny
/// (at most 3-node) trees; no need for a separate descent helper.
fn find_by_key(header: u32, key: u32) -> Option<u32> {
    in_order_keys(header).into_iter().find(|&(_, k)| k == key).map(|(node, _)| node)
}

/// Byte-for-byte comparison of a freshly-*constructed* value pair, like [`push_field_diffs`] but
/// skipping the never-read `+0x6`/`+0x7` padding: real vanilla's own constructor never writes it (it
/// stays whatever `operator_new` happened to return - genuinely uninitialized heap garbage), while
/// this port's [`crate::ztshowscriptstate::create_show_script_state`] deliberately zeroes the whole
/// `0x14` bytes first (see that function's own doc comment) - a harmless, intentional deviation since
/// nothing reads that padding either way, but not one `push_field_diffs`' full-range comparison can
/// tolerate for a fresh construction. (The `SAVE_LOAD_ROUNDTRIP`/`INIT` tests don't need this variant:
/// there the padding is explicitly test-poisoned/zeroed on both poles up front, not left as real
/// heap garbage, so a full-range comparison holds.)
fn push_fresh_value_diffs(failures: &mut Vec<String>, rust_value: u32, real_value: u32) {
    for off in 0x0..0x14u32 {
        if off == 0x6 || off == 0x7 {
            continue;
        }
        let rust = get_from_memory::<u8>(rust_value + off);
        let real = get_from_memory::<u8>(real_value + off);
        if rust != real {
            failures.push(format!(
                "+{off:#04x} mismatch between rust and real poles (rust {rust:#04x}, real {real:#04x})"
            ));
        }
    }
}

/// Per-field assertion of a freshly-inserted value's contents against the confirmed constructor field
/// list (`ZTShowScriptState_ZTShowScriptState.c`/macOS `ZTShowScriptState::ZTShowScriptState`): vtable,
/// `script_id`, `key`, the `0xffff` trick-index sentinel, and all six flags zeroed.
fn assert_fresh_value_fields(failures: &mut Vec<String>, case: &str, pole: &str, value: u32, script_id: u16, key: u32) {
    let expected_vtable = get_module_base("zoo.exe") as u32 + RVA_SCRIPT_STATE_VTABLE;
    let checks: [(u32, u32, &str); 4] = [
        (get_from_memory::<u32>(value), expected_vtable, "+0x0 vtable"),
        (get_from_memory::<u16>(value + 0x4) as u32, script_id as u32, "+0x4 script_id"),
        (get_from_memory::<u32>(value + 0x8), key, "+0x8 key"),
        (get_from_memory::<u16>(value + 0xc) as u32, 0xffff, "+0xc trick_index"),
    ];
    for (actual, expected, field) in checks {
        if actual != expected {
            failures.push(format!("{case} ({pole} pole): {field} should be {expected:#x}, got {actual:#x}"));
        }
    }
    for off in [0xe, 0xf, 0x10, 0x11, 0x12, 0x13] {
        let flag = get_from_memory::<u8>(value + off);
        if flag != 0 {
            failures.push(format!("{case} ({pole} pole): +{off:#04x} flag should be 0, got {flag:#04x}"));
        }
    }
}

/// `ZTSHOWSCRIPTSTATE_CREATE_OR_FIND` - the create/find-or-insert entry point itself
/// (`crate::ztshowscriptstate::create_show_script_state`, `generated.rs`'s mislabeled
/// `ztshowscriptstate::CONSTRUCTOR`), against real vanilla `ZTShow::createShowScriptState` on a
/// synthetic, empty tree (`ztshowscriptstate_live_support::build_standalone_show_and_header`). Review
/// follow-up: the save/load and `setNextItem` tests above never actually exercise this function's own
/// tree-insert-plus-allocation logic (their fixtures are built directly via
/// `build_standalone_script_state`, bypassing the constructor entirely), and `ztshow.rs`'s own live
/// tests only ever call this function's *real* pole (`real_create_show_script_state`), never the Rust
/// port - so this was previously uncovered by any cross-pole comparison, despite being the one method
/// in this class that combines tree mutation with allocation.
///
/// Each pole gets its own independent `(show, header)` environment - tree node *addresses* are never
/// comparable across poles, only field *content*, which is deterministic. Cases, run in order against
/// the same environment so later cases see earlier state:
/// 1. fresh insert of `KEY_A` into an empty tree - return `0`, the new node becomes root/leftmost/
///    rightmost, and the value's fields match the confirmed constructor field list exactly;
/// 2. re-insert of `KEY_A` (found path) - return `0xffffffff`, header and value bytes provably
///    untouched (before/after snapshot);
/// 3. insert `KEY_B < KEY_A` - return `0`, becomes the new leftmost cache target, rightmost unchanged;
/// 4. insert `KEY_C > KEY_A` - return `0`, becomes the new rightmost cache target;
/// 5. an in-order walk confirms all three nodes in ascending-key order and that the leftmost/rightmost
///    caches point at the walk's own first/last entries, on **both** poles independently - the real
///    pole's tree is built entirely by real vanilla's own STL insert-with-hint routine, not this
///    port's simplified BST, so this is the one place that routine's own behavior gets checked;
/// 6. re-insert of `KEY_B` (found path, now in a non-root position) - return `0xffffffff`, tree shape
///    (in-order key sequence) unchanged.
///
/// The allocation-failure path (`operator_new` returning null, return `5`) is not covered here - not
/// forceable live without fault-injection plumbing this class doesn't otherwise need.
pub(crate) fn run_ztshowscriptstate_create_or_find_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTSTATE_CREATE_OR_FIND";
    let mut failures: Vec<String> = Vec::new();

    const SCRIPT_ID: u16 = 0x55aa;
    const KEY_A: u32 = 0x2000;
    const KEY_B: u32 = 0x1000;
    const KEY_C: u32 = 0x3000;

    let (rust_show, rust_header) = ztshowscriptstate_live_support::build_standalone_show_and_header(SCRIPT_ID);
    let (real_show, real_header) = ztshowscriptstate_live_support::build_standalone_show_and_header(SCRIPT_ID);

    fn call_rust(show: u32, key: u32) -> u32 {
        unsafe { ZTSHOWSCRIPTSTATE_CONSTRUCTOR.hooked()(show as *const u32, key) }
    }
    fn call_real(show: u32, key: u32) -> u32 {
        ztshowscriptstate_live_support::real_create_show_script_state(show, key)
    }

    // Case 1: fresh insert of KEY_A into an empty tree.
    let rust_ret = call_rust(rust_show, KEY_A);
    let real_ret = call_real(real_show, KEY_A);
    if rust_ret != 0 || real_ret != 0 {
        failures.push(format!("fresh insert KEY_A: expected 0 on both poles, got rust {rust_ret:#x}, real {real_ret:#x}"));
    }

    let rust_node_a = get_from_memory::<u32>(rust_header + 0x4);
    let real_node_a = get_from_memory::<u32>(real_header + 0x4);
    for (pole, header, node) in [("rust", rust_header, rust_node_a), ("real", real_header, real_node_a)] {
        let leftmost = get_from_memory::<u32>(header + 0x8);
        let rightmost = get_from_memory::<u32>(header + 0xc);
        if leftmost != node || rightmost != node {
            failures.push(format!(
                "fresh insert KEY_A ({pole} pole): header leftmost/rightmost should both be the new root {node:#010x}, got leftmost {leftmost:#010x} rightmost {rightmost:#010x}"
            ));
        }
        let node_key = get_from_memory::<u32>(node + 0x10);
        if node_key != KEY_A {
            failures.push(format!("fresh insert KEY_A ({pole} pole): node key should be {KEY_A:#x}, got {node_key:#x}"));
        }
    }

    let rust_value_a = get_from_memory::<u32>(rust_node_a + 0x14);
    let real_value_a = get_from_memory::<u32>(real_node_a + 0x14);
    push_fresh_value_diffs(&mut failures, rust_value_a, real_value_a);
    assert_fresh_value_fields(&mut failures, "fresh insert KEY_A", "rust", rust_value_a, SCRIPT_ID, KEY_A);
    assert_fresh_value_fields(&mut failures, "fresh insert KEY_A", "real", real_value_a, SCRIPT_ID, KEY_A);

    // Case 2: re-insert of KEY_A - found path, must touch nothing.
    let rust_value_a_before = snapshot(rust_value_a);
    let real_value_a_before = snapshot(real_value_a);
    let rust_header_before = (get_from_memory::<u32>(rust_header + 0x4), get_from_memory::<u32>(rust_header + 0x8), get_from_memory::<u32>(rust_header + 0xc));
    let real_header_before = (get_from_memory::<u32>(real_header + 0x4), get_from_memory::<u32>(real_header + 0x8), get_from_memory::<u32>(real_header + 0xc));

    let rust_ret = call_rust(rust_show, KEY_A);
    let real_ret = call_real(real_show, KEY_A);
    if rust_ret != 0xffff_ffff || real_ret != 0xffff_ffff {
        failures.push(format!("re-insert KEY_A: expected 0xffffffff on both poles, got rust {rust_ret:#x}, real {real_ret:#x}"));
    }
    if snapshot(rust_value_a) != rust_value_a_before {
        failures.push("re-insert KEY_A (rust pole): found path touched the existing value's bytes".to_string());
    }
    if snapshot(real_value_a) != real_value_a_before {
        failures.push("re-insert KEY_A (real pole): found path touched the existing value's bytes".to_string());
    }
    let rust_header_after = (get_from_memory::<u32>(rust_header + 0x4), get_from_memory::<u32>(rust_header + 0x8), get_from_memory::<u32>(rust_header + 0xc));
    let real_header_after = (get_from_memory::<u32>(real_header + 0x4), get_from_memory::<u32>(real_header + 0x8), get_from_memory::<u32>(real_header + 0xc));
    if rust_header_after != rust_header_before {
        failures.push("re-insert KEY_A (rust pole): found path touched the tree header".to_string());
    }
    if real_header_after != real_header_before {
        failures.push("re-insert KEY_A (real pole): found path touched the tree header".to_string());
    }

    // Case 3: insert KEY_B < KEY_A - new leftmost, rightmost (still the root) unchanged.
    let rust_ret = call_rust(rust_show, KEY_B);
    let real_ret = call_real(real_show, KEY_B);
    if rust_ret != 0 || real_ret != 0 {
        failures.push(format!("insert KEY_B: expected 0 on both poles, got rust {rust_ret:#x}, real {real_ret:#x}"));
    }
    for (pole, header, root) in [("rust", rust_header, rust_node_a), ("real", real_header, real_node_a)] {
        let leftmost = get_from_memory::<u32>(header + 0x8);
        let rightmost = get_from_memory::<u32>(header + 0xc);
        let leftmost_key = get_from_memory::<u32>(leftmost + 0x10);
        if leftmost_key != KEY_B {
            failures.push(format!(
                "insert KEY_B ({pole} pole): leftmost cache should now point at KEY_B ({KEY_B:#x}), points at key {leftmost_key:#x}"
            ));
        }
        if rightmost != root {
            failures.push(format!("insert KEY_B ({pole} pole): rightmost cache should be unchanged (still the root), got {rightmost:#010x}"));
        }
    }

    // Case 4: insert KEY_C > KEY_A - new rightmost.
    let rust_ret = call_rust(rust_show, KEY_C);
    let real_ret = call_real(real_show, KEY_C);
    if rust_ret != 0 || real_ret != 0 {
        failures.push(format!("insert KEY_C: expected 0 on both poles, got rust {rust_ret:#x}, real {real_ret:#x}"));
    }
    for (pole, header) in [("rust", rust_header), ("real", real_header)] {
        let rightmost = get_from_memory::<u32>(header + 0xc);
        let rightmost_key = get_from_memory::<u32>(rightmost + 0x10);
        if rightmost_key != KEY_C {
            failures.push(format!(
                "insert KEY_C ({pole} pole): rightmost cache should now point at KEY_C ({KEY_C:#x}), points at key {rightmost_key:#x}"
            ));
        }
    }

    // Cross-pole value-content comparison for KEY_B/KEY_C, same shape as KEY_A's above.
    for (case, key) in [("KEY_B", KEY_B), ("KEY_C", KEY_C)] {
        let rust_node = find_by_key(rust_header, key);
        let real_node = find_by_key(real_header, key);
        match (rust_node, real_node) {
            (Some(rn), Some(realn)) => {
                let rust_value = get_from_memory::<u32>(rn + 0x14);
                let real_value = get_from_memory::<u32>(realn + 0x14);
                push_fresh_value_diffs(&mut failures, rust_value, real_value);
                assert_fresh_value_fields(&mut failures, &format!("insert {case}"), "rust", rust_value, SCRIPT_ID, key);
                assert_fresh_value_fields(&mut failures, &format!("insert {case}"), "real", real_value, SCRIPT_ID, key);
            }
            _ => failures.push(format!("insert {case}: node not found in tree on one or both poles (rust {rust_node:?}, real {real_node:?})")),
        }
    }

    // Case 5: in-order walk - 3 nodes, ascending key order, caches match the walk's own ends.
    for (pole, header) in [("rust", rust_header), ("real", real_header)] {
        let order = in_order_keys(header);
        let keys: Vec<u32> = order.iter().map(|&(_, k)| k).collect();
        if keys != [KEY_B, KEY_A, KEY_C] {
            failures.push(format!("{pole} pole: in-order walk should yield [{KEY_B:#x}, {KEY_A:#x}, {KEY_C:#x}], got {keys:#x?}"));
        }
        if let (Some(&(first, _)), Some(&(last, _))) = (order.first(), order.last()) {
            let leftmost = get_from_memory::<u32>(header + 0x8);
            let rightmost = get_from_memory::<u32>(header + 0xc);
            if leftmost != first {
                failures.push(format!("{pole} pole: leftmost cache {leftmost:#010x} does not match in-order walk's first node {first:#010x}"));
            }
            if rightmost != last {
                failures.push(format!("{pole} pole: rightmost cache {rightmost:#010x} does not match in-order walk's last node {last:#010x}"));
            }
        }
    }

    // Case 6: re-insert KEY_B (found, non-root position) - tree shape must be unchanged.
    let rust_ret = call_rust(rust_show, KEY_B);
    let real_ret = call_real(real_show, KEY_B);
    if rust_ret != 0xffff_ffff || real_ret != 0xffff_ffff {
        failures.push(format!("re-insert KEY_B: expected 0xffffffff on both poles, got rust {rust_ret:#x}, real {real_ret:#x}"));
    }
    for (pole, header) in [("rust", rust_header), ("real", real_header)] {
        let keys: Vec<u32> = in_order_keys(header).iter().map(|&(_, k)| k).collect();
        if keys != [KEY_B, KEY_A, KEY_C] {
            failures.push(format!("{pole} pole: re-inserting KEY_B changed tree shape, in-order walk now {keys:#x?}"));
        }
    }

    finish_test(test_name, failures, failure_log)
}
