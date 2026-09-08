//! Compares the installed `ZTScenarioSimpleGoal::eval` award-count override
//! (`crate::ztawardmgr::eval_award_count_override` - see that detour's own doc comment; the goal
//! class itself has no Rust reimplementation beyond the override) against real vanilla behavior on
//! synthetic `ZTScenarioSimpleGoal*` fixtures.

use openzt_detour::generated::ztawardmgr as gen_ztawardmgr;
use std::io::Write;
use tracing::{error, info};

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::util::{get_from_memory, save_to_memory};
use crate::ztawardmgr::{self, live_support as award_live_support};

use super::ztawardmgr::reset_awardmgr_both_sides;

/// ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT: drives `ZTScenarioSimpleGoal::eval`'s installed override by
/// calling through its real, now-patched address directly (`ztawardmgr::eval_award_count_override::
/// init` - installed specifically, not the whole `ztawardmgr::init` - has already installed the
/// detour by this point in the battery, and the game's `.exe` has no ASLR, so the raw Ghidra VA is
/// safe to call via a plain `transmute` - same pattern `ztthoughtmgr.rs`'s
/// `resolve_object_own_habitat_ptr` uses for a vtable slot). Builds a fully synthetic, zeroed,
/// leaked buffer standing in for a `ZTScenarioSimpleGoal*` (safe: every case exercised here only
/// touches `+0xc`/`+0x10`/`+0x1c` on `this`), seeds a known, identical, non-zero award count on both
/// representations (the real vector via `ADD_AWARD.original()`, the Rust store via
/// `ztawardmgr::add_award`) so a mismatch would be visible, then compares real vanilla behavior
/// (`ztawardmgr::eval_award_count_override::call_real`, the `retour` trampoline - **not**
/// `EVAL.original()`, which in release is a raw address cast with no trampoline that would loop back
/// into this same detour once it's hooked; debug `.original()` routes through the hook registry, but
/// `call_real` keeps the vanilla pole release-safe, see that helper's own doc comment) against a direct call
/// through the hooked address for: the gate-passing case at the exact threshold boundary (both should
/// equal the seeded count, since both representations are in sync), the gate-failing case just past
/// the boundary, and two unrelated submetric values under the same goal kind - the override must fall
/// through to identical vanilla behavior for all three of those.
pub(crate) fn run_ztscenariosimplegoal_eval_award_count_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT";
    let game_mgr_ptr = globals().ztgamemgr_ptr();
    if game_mgr_ptr.is_null() {
        info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
        return false;
    }

    let field_0x15c = get_from_memory::<i32>(game_mgr_ptr as u32 + 0x15c);
    let field_0x160 = get_from_memory::<i32>(game_mgr_ptr as u32 + 0x160);
    let threshold_boundary = field_0x15c + field_0x160 * 12;

    let real_ptr = award_live_support::real_ptr();
    reset_awardmgr_both_sides();
    for id in [9_100_001i32, 9_100_002, 9_100_003] {
        unsafe { gen_ztawardmgr::ADD_AWARD.original()(real_ptr, id) };
        ztawardmgr::add_award(id);
    }

    let goal_buf = Box::into_raw(Box::new([0u8; 0x20]));
    let goal_ptr = goal_buf as *const u32;

    let cases: [(&str, i32, i32, i32); 4] = [
        ("gate-passing at boundary", 1, 0xb, threshold_boundary),
        ("gate-failing just past boundary", 1, 0xb, threshold_boundary + 1),
        ("unrelated submetric 0", 1, 0, threshold_boundary),
        ("unrelated submetric 7", 1, 7, threshold_boundary),
    ];

    let mut fail_flag = false;
    for (label, kind, submetric, threshold) in cases {
        save_to_memory::<i32>(goal_ptr as u32 + 0xc, kind);
        save_to_memory::<i32>(goal_ptr as u32 + 0x10, submetric);
        save_to_memory::<i32>(goal_ptr as u32 + 0x1c, threshold);

        let expected = ztawardmgr::eval_award_count_override::call_real(goal_ptr);
        let hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32) -> i32>(0x0041d665u32) };
        let actual = hooked(goal_ptr);

        if expected != actual {
            error!("{} mismatch for case '{}': expected={}, actual={}", test_name, label, expected, actual);
            if let Some(log_file) = failure_log {
                let _ = log_file
                    .write_all(format!("Test Failed {}: case '{}', expected={}, actual={}\n", test_name, label, expected, actual).as_bytes());
            }
            fail_flag = true;
        }
    }

    drop(unsafe { Box::from_raw(goal_buf) });

    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}
