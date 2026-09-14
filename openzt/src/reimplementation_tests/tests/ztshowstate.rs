//! Compares real vanilla `ZTShowState` against its Rust reimplementation (production file
//! `openzt/src/ztshowstate.rs`): the detour-wiring check, standalone `init`, and a save/load
//! round-trip. Script states themselves are separate freelist-allocated nodes owned by each
//! show's `ZTShowInfo` - see `crate::ztshowstate::live_support` for the standalone fixture.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::standalone;
use openzt_detour::generated::ztshowstate::{INIT as ZTSHOWSTATE_INIT, LOAD as ZTSHOWSTATE_LOAD, SAVE as ZTSHOWSTATE_SAVE};

use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, save_to_memory};
use crate::ztshow;
use crate::ztshowstate::{self, live_support as ztshowstate_live_support};
/// `ZTSHOWSTATE_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `ztshowstate::init()`, and this asserts all four of its detours actually report enabled (same
/// rationale as the MenuMusicHandler/ZTSoundscape/ZooStatus blocks in that `init()` function - see
/// each of their own `_DETOURS_ENABLED` tests). Added after this exact gap - `ztshowstate::init()`
/// missing from `reimplementation_tests::init()`'s own list - let `ZTSHOWSTATE_INIT` pass as a false
/// positive (real vanilla `init` and this port happen to produce identical results) while
/// `ZTSHOWSTATE_SAVE_LOAD_ROUNDTRIP` silently ran real vanilla `save`/`load` instead of this module's
/// own code with no error logged anywhere.
pub(crate) fn run_ztshowstate_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSTATE_DETOURS_ENABLED";
    let mut disabled: Vec<&'static str> = Vec::new();
    for (name, enabled) in ztshowstate_live_support::detour_status() {
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

/// `ZTSHOWSTATE_INIT` - Stage 1 of `ztshowinfo-implementation-plan.md`: `ZTShowState::init` is this
/// class's one vtable slot. Builds two real, standalone `ZTShowState` instances (via the real,
/// un-detoured constructor - `ztshowstate_live_support::build_standalone_show_state`), poisons every
/// field `init` is supposed to reset on both, then runs the Rust port (`.hooked()`) on one and the
/// real vanilla body (`.original()`, routed through the debug trampoline to the pre-detour body) on
/// the other - comparing every field afterward, including the tree header pointer (`+0x1c`) and the
/// two trailing fields (`+0x20`/`+0x24`), which `init` must leave untouched.
pub(crate) fn run_ztshowstate_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSTATE_INIT";
    let mut failures: Vec<String> = Vec::new();

    let rust_state = ztshowstate_live_support::build_standalone_show_state();
    let real_state = ztshowstate_live_support::build_standalone_show_state();

    fn poison(state: u32) {
        save_to_memory(state + 0x4, 0x1111u16);
        save_to_memory(state + 0x6, 0x22u8);
        save_to_memory(state + 0x7, 0x33u8);
        save_to_memory(state + 0x8, 0x44u8);
        save_to_memory(state + 0xc, 0x5555_5555u32);
        save_to_memory(state + 0x10, 0x6666_6666u32);
        save_to_memory(state + 0x14, 0x7777_7777u32);
        save_to_memory(state + 0x18, 0x8888_8888u32);
        save_to_memory(state + 0x20, 0x9999_9999u32);
        save_to_memory(state + 0x24, 0xaau8);
    }
    poison(rust_state);
    poison(real_state);

    let rust_header_before = get_from_memory::<u32>(rust_state + 0x1c);
    let real_header_before = get_from_memory::<u32>(real_state + 0x1c);

    unsafe {
        ZTSHOWSTATE_INIT.hooked()(rust_state as *const u32);
        ZTSHOWSTATE_INIT.original()(real_state as *const u32);
    }

    if get_from_memory::<u16>(rust_state + 0x4) != get_from_memory::<u16>(real_state + 0x4) {
        failures.push("+0x4 mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u8>(rust_state + 0x6) != get_from_memory::<u8>(real_state + 0x6) {
        failures.push("+0x6 mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u8>(rust_state + 0x7) != get_from_memory::<u8>(real_state + 0x7) {
        failures.push("+0x7 mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u8>(rust_state + 0x8) != get_from_memory::<u8>(real_state + 0x8) {
        failures.push("+0x8 mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u32>(rust_state + 0xc) != get_from_memory::<u32>(real_state + 0xc) {
        failures.push("+0xc mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u32>(rust_state + 0x10) != get_from_memory::<u32>(real_state + 0x10) {
        failures.push("+0x10 mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u32>(rust_state + 0x14) != get_from_memory::<u32>(real_state + 0x14) {
        failures.push("+0x14 mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u32>(rust_state + 0x18) != get_from_memory::<u32>(real_state + 0x18) {
        failures.push("+0x18 mismatch between rust and real poles".to_string());
    }
    if get_from_memory::<u8>(rust_state + 0x8) != 1 {
        failures.push(format!("+0x8 should be reset to 1, got {}", get_from_memory::<u8>(rust_state + 0x8)));
    }

    if get_from_memory::<u32>(rust_state + 0x1c) != rust_header_before {
        failures.push("rust pole: init must not touch the tree header pointer (+0x1c)".to_string());
    }
    if get_from_memory::<u32>(real_state + 0x1c) != real_header_before {
        failures.push("real pole: init touched the tree header pointer (+0x1c) - contradicts this port's own understanding of vanilla init".to_string());
    }
    if get_from_memory::<u32>(rust_state + 0x20) != 0x9999_9999 {
        failures.push("rust pole: init must not touch +0x20".to_string());
    }
    if get_from_memory::<u8>(rust_state + 0x24) != 0xaa {
        failures.push("rust pole: init must not touch +0x24".to_string());
    }

    ztshowstate_live_support::destroy_standalone_show_state(rust_state);
    ztshowstate_live_support::destroy_standalone_show_state(real_state);

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWSTATE_SAVE_LOAD_ROUNDTRIP` - Stage 1: seeds a real, standalone `ZTShowState` (built via
/// the real constructor) with known header-scalar values and two synthetic `ZTShowScriptState`
/// entries (built the same way `show_state_load` itself builds them - `operator_new(0x14)`, then
/// linked into the tree via the module's own `find_or_insert_state_node`), captures the Rust port's
/// own `save` output via `io_redirect`, then replays that capture into a second standalone
/// instance's `load` and checks every field/entry round-trips. Cross-checks the reloaded tree
/// against `ztshow.rs`'s own already-shipped, read-only `get_show_script_state` (via a minimal
/// `ZTShow`-shaped shim whose only live field is `+0x34`, the exact indirection `get_show_script_state`
/// reads) - reusing the existing reader as this test's own oracle, not a second hand-rolled walk.
pub(crate) fn run_ztshowstate_save_load_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSTATE_SAVE_LOAD_ROUNDTRIP";
    let mut failures: Vec<String> = Vec::new();
    const CURRENT_VERSION: u32 = 0x100;

    fn leaked_bytes(size: usize) -> u32 {
        let buf: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
        buf.as_mut_ptr() as u32
    }

    fn make_script_state_value(script_id: u16, unit_id: u32, trick_index: u16, flags: [u8; 6]) -> u32 {
        let value = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
        unsafe { std::ptr::write_bytes(value as *mut u8, 0, 0x14) };
        save_to_memory(value + 0x4, script_id);
        save_to_memory(value + 0x8, unit_id);
        save_to_memory(value + 0xc, trick_index);
        save_to_memory(value + 0xe, flags[0]);
        save_to_memory(value + 0xf, flags[1]);
        save_to_memory(value + 0x10, flags[2]);
        save_to_memory(value + 0x11, flags[3]);
        save_to_memory(value + 0x12, flags[4]);
        save_to_memory(value + 0x13, flags[5]);
        value
    }

    let source = ztshowstate_live_support::build_standalone_show_state();
    save_to_memory(source + 0x4, 0x4242u16);
    save_to_memory(source + 0x6, 0x11u8);
    save_to_memory(source + 0x7, 0x22u8);
    save_to_memory(source + 0x8, 0x1u8);
    save_to_memory(source + 0xc, 0x1000u32);
    save_to_memory(source + 0x10, 0x2000u32);
    save_to_memory(source + 0x14, 0x3000u32);
    save_to_memory(source + 0x18, 0x4000u32);
    save_to_memory(source + 0x20, 2u32);

    let header = get_from_memory::<u32>(source + 0x1c);
    let value_a = make_script_state_value(0x4242, 101, 5, [0, 1, 0, 0, 1, 0]);
    let value_b = make_script_state_value(0x4242, 202, 0xffff, [1, 0, 0, 0, 0, 1]);
    let node_a = ztshowstate::find_or_insert_state_node(header, 101);
    save_to_memory(node_a + 0x14, value_a);
    let node_b = ztshowstate::find_or_insert_state_node(header, 202);
    save_to_memory(node_b + 0x14, value_b);

    let dummy_file: u32 = 0;
    let file_ptr = &dummy_file as *const u32;

    io_redirect::begin_capture();
    let save_ret = unsafe { ZTSHOWSTATE_SAVE.hooked()(source as *const u32, file_ptr) };
    let bytes = io_redirect::end_capture();
    if !save_ret {
        failures.push(format!("hooked save should return true, got {save_ret}"));
    }

    let target = ztshowstate_live_support::build_standalone_show_state();
    io_redirect::begin_replay(bytes);
    let load_ret = unsafe { ZTSHOWSTATE_LOAD.hooked()(target as *const u32, file_ptr, CURRENT_VERSION) };
    io_redirect::end_replay();
    if !load_ret {
        failures.push("hooked load should return true".to_string());
    }

    if get_from_memory::<u16>(target + 0x4) != 0x4242 {
        failures.push("field +0x4 did not round-trip".to_string());
    }
    if get_from_memory::<u8>(target + 0x6) != 0x11 {
        failures.push("field +0x6 did not round-trip".to_string());
    }
    if get_from_memory::<u8>(target + 0x7) != 0x22 {
        failures.push("field +0x7 did not round-trip".to_string());
    }
    if get_from_memory::<u8>(target + 0x8) != 0x1 {
        failures.push("field +0x8 did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target + 0xc) != 0x1000 {
        failures.push("field +0xc did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target + 0x10) != 0x2000 {
        failures.push("field +0x10 did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target + 0x14) != 0x3000 {
        failures.push("field +0x14 did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target + 0x18) != 0x4000 {
        failures.push("field +0x18 did not round-trip".to_string());
    }
    if get_from_memory::<u32>(target + 0x20) != 2 {
        failures.push("field +0x20 (entry count) did not round-trip".to_string());
    }

    let show_shim = leaked_bytes(0x38);
    save_to_memory(show_shim + 0x34, get_from_memory::<u32>(target + 0x1c));

    let found_a = ztshow::get_show_script_state(show_shim, 101);
    if found_a == 0 {
        failures.push("get_show_script_state should find the reloaded key 101 entry".to_string());
    } else {
        if get_from_memory::<u16>(found_a + 0x4) != 0x4242 {
            failures.push("reloaded key-101 entry's own script id did not round-trip".to_string());
        }
        if get_from_memory::<u16>(found_a + 0xc) != 5 {
            failures.push("reloaded key-101 entry's own trick index did not round-trip".to_string());
        }
    }

    let found_b = ztshow::get_show_script_state(show_shim, 202);
    if found_b == 0 {
        failures.push("get_show_script_state should find the reloaded key 202 entry".to_string());
    } else if get_from_memory::<u16>(found_b + 0xc) != 0xffff {
        failures.push("reloaded key-202 entry's own trick index did not round-trip".to_string());
    }

    if ztshow::get_show_script_state(show_shim, 999) != 0 {
        failures.push("get_show_script_state should not find a key that was never inserted".to_string());
    }

    ztshowstate_live_support::destroy_standalone_show_state(source);
    ztshowstate_live_support::destroy_standalone_show_state(target);

    finish_test(test_name, failures, failure_log)
}
