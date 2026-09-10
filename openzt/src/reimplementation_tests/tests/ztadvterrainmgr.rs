//! Exercises the reimplemented `ZTAdvTerrainMgr` (production file `openzt/src/ztadvterrainmgr.rs`)
//! against the live singleton. See each test's doc comment for why these deliberately do *not*
//! also diff against the real `START`/`UPDATE.original()`.

use proptest::prelude::*;
use std::io::Write;
use tracing::{error, info};

use crate::globals::globals;
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::reimplementation_tests::harness::write_success_line;

/// ZTADVTERRAINMGR_START: sanity-checks the reimplemented `ZTAdvTerrainMgr::start()` against a real,
/// live singleton. Deliberately does **not** also invoke the real `START.original()` for comparison,
/// unlike the usual real-vs-reimplemented pattern: both bodies call through to the real
/// `start2D`/`startD3D`/`loadTextures`/`setupRender` D3D bring-up functions, which aren't re-entrant -
/// running them twice in one test would risk live device/texture corruption for no comparison value
/// (the short-circuit call sequence is simple enough to verify by review). Instead this runs the
/// reimplementation once and checks the result is plausible (succeeds, and leaves `state == 2` per
/// `ZTAdvTerrainMgr_start.c`).
pub(crate) fn run_ztadvterrainmgr_start_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTADVTERRAINMGR_START";
    let mgr_ptr = globals().ztadvterrainmgr_ptr();
    if mgr_ptr.is_null() {
        write_success_line(failure_log, &format!("{} (skipped: ZTAdvTerrainMgr not initialized)", test_name));
        return false;
    }
    let mgr = unsafe { &mut *mgr_ptr };
    let before_state = mgr.state();

    let result = mgr.start();
    let after_state = mgr.state();

    // Restore the pre-test state regardless of outcome - `start()` is meant to run once at bring-up,
    // not repeatedly under test.
    mgr.set_state(before_state);

    if result && after_state == 2 {
        write_success_line(failure_log, test_name);
        false
    } else {
        error!("{}: expected success with state==2, got success={} state={}", test_name, result, after_state);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: success={} state={}\n", test_name, result, after_state).as_bytes());
        }
        true
    }
}

/// ZTADVTERRAINMGR_UPDATE: exercises the reimplemented `ZTAdvTerrainMgr::update()` against the live
/// singleton's real world/queue state, for `delta_ticks` in `0..0x1000` crossed with every branch of
/// `compute_update_state`'s `state` switch. Deliberately does **not** also call the real
/// `UPDATE.original()`: the only shared, meaningfully mutable state is the live pending-tile queue
/// (`+0x1d8`), and running both back-to-back could pop/free the same vanilla-owned node twice - see
/// `ztadvterrainmgr.rs`'s module doc comment on the cross-allocator hazard. The live queue is
/// populated only by other, un-reimplemented vanilla code and may well be empty during this test -
/// that's an expected, non-failing case; when non-empty, this still safely exercises the real
/// pop-front/recycle path against genuine vanilla-allocated nodes. The assertion is narrow but real:
/// `update()` never mutates `state` (it's read-only in `ZTAdvTerrainMgr_update.c`), so forcing each
/// branch and checking it comes back unchanged catches any accidental write.
pub(crate) fn run_ztadvterrainmgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTADVTERRAINMGR_UPDATE";
    let mgr_ptr = globals().ztadvterrainmgr_ptr();
    if mgr_ptr.is_null() {
        write_success_line(failure_log, &format!("{} (skipped: ZTAdvTerrainMgr not initialized)", test_name));
        return false;
    }
    let original_state = unsafe { &*mgr_ptr }.state();

    let runner_config = ProptestConfig { failure_persistence: Some(Box::new(NoopFailurePersistence)), ..ProptestConfig::default() };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let mut fail_flag = false;
    let state_strategy = prop_oneof![Just(0i32), Just(1i32), Just(2i32), Just(3i32), Just(4i32), Just(-1i32), Just(5i32)];
    match runner.run(&(state_strategy, 0u32..0x1000u32), |(state, delta_ticks)| {
        let mgr = unsafe { &mut *mgr_ptr };
        mgr.set_state(state);
        mgr.update(delta_ticks);
        let after_state = mgr.state();
        mgr.set_state(original_state);
        prop_assert_eq!(after_state, state, "update() must not mutate state (forced state={}, delta_ticks={})", state, delta_ticks);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    // Restore regardless of outcome.
    unsafe { &mut *mgr_ptr }.set_state(original_state);
    fail_flag
}
