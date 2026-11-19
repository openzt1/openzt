//! Compares real vanilla `BFUnit::getMoveAnimSpeed` (`0x4a9e00`) against its Rust reimplementation (production
//! file `openzt/src/bfunit.rs`) on units built by the real `BFEntity` constructor whose vtable is replaced by one
//! whose speed slots return distinct values. It runs against the live `ZTAIMgr`, whose predicates classify the
//! deepest behavior set's parsed call, so it needs a loaded zoo.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::bfunit::{GET_MOVE_ANIM_SPEED, GET_MOVE_ANIM_SYMBOL};

use crate::bfbehaviorset::live_support as bfbehaviorset_live_support;
use crate::bfentity::{live_support as bfentity_live_support, BFENTITY_SIZE};
use crate::bfunit::live_support as bfunit_live_support;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::util::get_from_memory;

extern "thiscall" fn slow_speed(_this: u32) -> u32 {
    0xabcd_0001
}
extern "thiscall" fn medium_speed(_this: u32) -> u32 {
    0xabcd_0002
}
extern "thiscall" fn fast_speed(_this: u32) -> u32 {
    0xabcd_0003
}
extern "thiscall" fn slow_symbol(_this: u32) -> u32 {
    0x1234_5671
}
extern "thiscall" fn medium_symbol(_this: u32) -> u32 {
    0x1234_5672
}
extern "thiscall" fn fast_symbol(_this: u32) -> u32 {
    0x1234_5673
}

/// `BFUNIT_DETOURS_ENABLED` - wiring check.
pub(crate) fn run_bfunit_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFUNIT_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = bfunit_live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
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

fn build_unit(chain: &[&[u8]], deepest_call: &str) -> u32 {
    let unit = Box::leak(vec![0u32; BFENTITY_SIZE / 4].into_boxed_slice()).as_mut_ptr() as u32;
    bfentity_live_support::real::constructor(unit, 0);
    let vtable = Box::leak(vec![0u32; 0x60].into_boxed_slice());
    vtable[0x140 / 4] = slow_speed as *const () as usize as u32;
    vtable[0x144 / 4] = medium_speed as *const () as usize as u32;
    vtable[0x148 / 4] = fast_speed as *const () as usize as u32;
    vtable[0x120 / 4] = slow_symbol as *const () as usize as u32;
    vtable[0x124 / 4] = medium_symbol as *const () as usize as u32;
    vtable[0x128 / 4] = fast_symbol as *const () as usize as u32;
    unsafe { (unit as *mut u32).write(vtable.as_ptr() as u32) };

    let set = unit + 0x14;
    // Every set above the deepest holds a call that would pick a different speed if it were used.
    bfbehaviorset_live_support::prime_call(set, "fMoveFast()");
    let mut parent = set;
    for (depth, name) in chain.iter().enumerate() {
        let child = bfbehaviorset_live_support::build_set(name, b"", &[], &[], 0);
        unsafe { ((parent + 0x4c) as *mut u32).write(child) };
        parent = child;
        if depth + 1 < chain.len() {
            bfbehaviorset_live_support::prime_call(parent, "fMoveMedium()");
        }
    }
    bfbehaviorset_live_support::prime_call(parent, deepest_call);
    unit
}

/// `BFUNIT_GET_MOVE_ANIM_SPEED` - slow, medium and fast move calls (by name and by `fWalk`/`fTrot`/`fRun`), an
/// unrelated call and an empty one, with no, one and two nested child sets.
pub(crate) fn run_bfunit_get_move_anim_speed_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_move_class_test(
        "BFUNIT_GET_MOVE_ANIM_SPEED",
        |unit| bfunit_live_support::real::get_move_anim_speed(unit) as u32,
        |unit| unsafe { GET_MOVE_ANIM_SPEED.hooked()(unit as *const u32) as u32 },
        failure_log,
    )
}

/// `BFUNIT_GET_MOVE_ANIM_SYMBOL` - the same cases as the speed test, through the symbol slots.
pub(crate) fn run_bfunit_get_move_anim_symbol_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_move_class_test(
        "BFUNIT_GET_MOVE_ANIM_SYMBOL",
        bfunit_live_support::real::get_move_anim_symbol,
        |unit| unsafe { GET_MOVE_ANIM_SYMBOL.hooked()(unit as *const u32) },
        failure_log,
    )
}

fn run_move_class_test(test_name: &str, real: impl Fn(u32) -> u32, port: impl Fn(u32) -> u32, failure_log: &mut Option<std::fs::File>) -> bool {
    let mut failures: Vec<String> = Vec::new();
    if get_from_memory::<u32>(0x0063_8098) == 0 {
        failures.push("GLOBAL_ZTAIMgr is null".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let chains: [&[&[u8]]; 3] = [&[], &[b"childA"], &[b"childA", b"childB"]];
    let calls = ["fMoveSlow()", "fMoveMedium()", "fMoveFast()", "fWalk(\"walk\")", "fTrot(\"trot\")", "fRun(\"run\")", "fOther(1)", "fMoveFast"];
    let mut seen = std::collections::BTreeSet::new();
    for chain in chains {
        for call in calls {
            let (real_unit, port_unit) = (build_unit(chain, call), build_unit(chain, call));
            let (real_ret, port_ret) = (real(real_unit), port(port_unit));
            seen.insert(real_ret);
            if real_ret != port_ret {
                failures.push(format!("children={} call={call:?}: real={real_ret:#x} port={port_ret:#x}", chain.len()));
            }
        }
    }
    if seen.len() < 3 {
        failures.push(format!("the calls only reached {} of the three slots: {seen:?}", seen.len()));
    }
    finish_test(test_name, failures, failure_log)
}
