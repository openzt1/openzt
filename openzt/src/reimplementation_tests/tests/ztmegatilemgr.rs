//! Compares real vanilla `ZTMegatileMgr` against its Rust reimplementation (production file
//! `openzt/src/ztmegatilemgr.rs`): update/recalculate_characteristics/init against the live loaded
//! singleton plus the category-map node-layout check. Registered in risk order: `update` first
//! (trivial scalar logic), `init` last (the only vector-resize path).

use openzt_detour::generated::ztmegatilemgr as gen_ztmegatilemgr;
use proptest::prelude::*;
use std::io::Write;
use tracing::{error, info};

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::ztmegatilemgr::live_support as megatile_live_support;
/// ZTMEGATILEMGR_UPDATE: compares the real `ZTMegatileMgr::update`'s effect on
/// `dirty`/`tick_accumulator` against the reimplemented `update`, for `delta_ticks` below the
/// `0x1d4b` threshold only. Snapshots the live singleton's scalars, calls the real
/// `UPDATE.original()`, records the result as expected, restores the snapshot (via
/// `megatile_live_support::restore_scalars`), calls the reimplemented `update()`, and compares.
/// Runs after `run_load_live_zoo` so the live singleton is a real, populated grid.
///
/// Deliberately never crosses the threshold: crossing it calls through to
/// `recalculate_characteristics` as a side effect, which is
/// `run_megatilemgr_recalculate_characteristics_test`'s job to compare. The threshold/dirty-flag
/// transition logic itself is covered by `ztmegatilemgr::tests::update_state_*` (pure, no live
/// memory touched).
pub(crate) fn run_megatilemgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMEGATILEMGR_UPDATE";
    let mgr_ptr = globals().ztmegatilemgr_ptr();
    if mgr_ptr.is_null() {
        info!("Skipping {}: GLOBAL_ZTMegatileMgr not initialized", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTMegatileMgr not initialized)", test_name));
        return false;
    }
    let real_this = mgr_ptr as *const u32;

    let runner_config = ProptestConfig { failure_persistence: Some(Box::new(NoopFailurePersistence)), ..ProptestConfig::default() };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let mut fail_flag = false;
    match runner.run(&(0u32..0x1000u32), |delta_ticks| {
        let mgr = unsafe { &mut *mgr_ptr };
        // Force a known-safe starting accumulator rather than restoring the pre-call value - the
        // live singleton's own accumulator may already be close to `0x1d4b` from real game ticks,
        // so `delta_ticks` on top of it could cross the threshold and trigger a full recalculation
        // (see the doc comment above).
        megatile_live_support::restore_scalars(mgr, false, 0);
        let before_dirty = false;
        let before_accumulator = 0u32;

        unsafe { gen_ztmegatilemgr::UPDATE.original()(real_this, delta_ticks as i32) };
        let expected_dirty = mgr.is_dirty();
        let expected_accumulator = mgr.tick_accumulator();

        megatile_live_support::restore_scalars(mgr, before_dirty, before_accumulator);
        mgr.update(delta_ticks);
        let reimpl_dirty = mgr.is_dirty();
        let reimpl_accumulator = mgr.tick_accumulator();

        // A recalculation triggered by either side (real or reimplemented) already leaves the grid
        // in a self-consistent state per its own logic - this test only compares the scalars
        // `update` itself owns.
        prop_assert_eq!(reimpl_dirty, expected_dirty, "dirty mismatch for delta_ticks={}", delta_ticks);
        prop_assert_eq!(reimpl_accumulator, expected_accumulator, "tick_accumulator mismatch for delta_ticks={}", delta_ticks);
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
    fail_flag
}

/// Compares two megatile-grid snapshots allowing a small tolerance on the `stink` field - real vanilla
/// x87 arithmetic and Rust's SSE2 `f32` arithmetic can differ in the last bit or two despite following
/// the same formula, which isn't a meaningful mismatch for this test. `guest_count` (an integer
/// accumulation) is still compared exactly.
fn grids_approximately_equal(expected: &megatile_live_support::GridSnapshot, actual: &megatile_live_support::GridSnapshot) -> bool {
    if expected.columns.len() != actual.columns.len() {
        return false;
    }
    expected.columns.iter().zip(actual.columns.iter()).all(|(e_col, a_col)| {
        e_col.len() == a_col.len()
            && e_col.iter().zip(a_col.iter()).all(|(&(e_guests, e_stink), &(a_guests, a_stink))| e_guests == a_guests && (e_stink - a_stink).abs() < 0.01)
    })
}

/// ZTMEGATILEMGR_RECALCULATE_CHARACTERISTICS: compares the real
/// `ZTMegatileMgr::recalculateCharacteristics`'s effect on the full megatile grid against the
/// reimplemented `recalculate_characteristics`. Recalculation is a pure function of live world state
/// (every field is zeroed then recomputed from scratch each call - see that method's own doc
/// comment), so this simply runs the real call, snapshots as expected, runs the reimplemented call
/// on top, and snapshots again - no restore needed.
pub(crate) fn run_megatilemgr_recalculate_characteristics_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMEGATILEMGR_RECALCULATE_CHARACTERISTICS";
    let mgr_ptr = globals().ztmegatilemgr_ptr();
    if mgr_ptr.is_null() {
        write_success_line(failure_log, &format!("{} (skipped: ZTMegatileMgr not initialized)", test_name));
        return false;
    }
    let mgr = unsafe { &mut *mgr_ptr };
    let real_this = mgr_ptr as *const u32;

    unsafe { gen_ztmegatilemgr::RECALCULATE_CHARACTERISTICS.original()(real_this) };
    let expected = megatile_live_support::snapshot_grid(mgr);

    mgr.recalculate_characteristics();
    let actual = megatile_live_support::snapshot_grid(mgr);

    if grids_approximately_equal(&expected, &actual) {
        write_success_line(failure_log, test_name);
        false
    } else {
        error!("{}: grid mismatch after recalculate_characteristics", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: grid mismatch\n", test_name).as_bytes());
        }
        true
    }
}

/// ZTMEGATILE_CATEGORY_MAP_LAYOUT: validates the reconstructed `MapHeader`/`TreeNode` layout itself,
/// independent of the tests above. After a real `RECALCULATE_CHARACTERISTICS.original()` call on the
/// live singleton, walks every populated `ZTMegatile`'s `category_map` via `category_value()` for
/// every key in the observed range `0x251f..0x2523` (9503-9506) and asserts every returned value is
/// finite. `category_value`'s own step cap turns a wrong left/right offset guess into a graceful
/// `None` rather than a live hang - see its doc comment - so this test's real signal is "did any
/// value come back non-finite", which would mean the walk landed somewhere nonsensical.
pub(crate) fn run_megatile_category_map_layout_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMEGATILE_CATEGORY_MAP_LAYOUT";
    let mgr_ptr = globals().ztmegatilemgr_ptr();
    if mgr_ptr.is_null() {
        write_success_line(failure_log, &format!("{} (skipped: ZTMegatileMgr not initialized)", test_name));
        return false;
    }
    let mgr = unsafe { &*mgr_ptr };
    let real_this = mgr_ptr as *const u32;
    unsafe { gen_ztmegatilemgr::RECALCULATE_CHARACTERISTICS.original()(real_this) };

    let keys: Vec<i32> = (0x251fi32..0x2523).collect();
    let mut fail_flag = false;
    for column in 0..mgr.megatile_columns() {
        for row in 0..mgr.megatile_rows_in_column(column) {
            let Some(mt) = mgr.megatile(column, row) else {
                continue;
            };
            for &key in &keys {
                if let Some(v) = mt.category_value(key)
                    && !v.is_finite()
                {
                    error!("{}: non-finite value {} for key {:#x} at column={}, row={}", test_name, v, key, column, row);
                    fail_flag = true;
                }
            }
        }
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}

/// ZTMEGATILEMGR_INIT: registered last of the four ZTMegatile tests - `init()` resizes the
/// outer/inner vectors (the actual allocation-adjacent call), the highest-risk piece in
/// `ztmegatilemgr.rs`. For a few small tile-count targets (both shrinking and growing relative to the
/// live map's own size), calls the real `INIT.original()` to capture the resulting grid dimensions,
/// resets to a different size, then calls the reimplemented `init()` for the same target and compares
/// dimensions. Restores the live singleton to the real map's own dimensions afterward regardless of
/// outcome, via the real vanilla `init`/`recalculateCharacteristics`, so nothing later depends on
/// whatever the last reimplemented call left behind.
pub(crate) fn run_megatilemgr_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMEGATILEMGR_INIT";
    let mgr_ptr = globals().ztmegatilemgr_ptr();
    if mgr_ptr.is_null() {
        write_success_line(failure_log, &format!("{} (skipped: ZTMegatileMgr not initialized)", test_name));
        return false;
    }
    let mgr = unsafe { &mut *mgr_ptr };
    let real_this = mgr_ptr as *const u32;

    let world = globals().ztworldmgr();
    let original_x = world.map_x_size as i32;
    let original_y = world.map_y_size as i32;

    let mut fail_flag = false;
    for &(x, y) in &[(3i32, 3i32), (8i32, 8i32), (original_x, original_y)] {
        unsafe { gen_ztmegatilemgr::INIT.original()(real_this, x as u32, y as u32) };
        let expected_columns = mgr.megatile_columns();
        let expected_rows: Vec<usize> = (0..expected_columns).map(|c| mgr.megatile_rows_in_column(c)).collect();

        unsafe { gen_ztmegatilemgr::INIT.original()(real_this, 1, 1) };
        mgr.init(x, y);
        let actual_columns = mgr.megatile_columns();
        let actual_rows: Vec<usize> = (0..actual_columns).map(|c| mgr.megatile_rows_in_column(c)).collect();

        if actual_columns != expected_columns || actual_rows != expected_rows {
            error!(
                "{}: dimension mismatch for ({}, {}): expected {} columns {:?}, got {} columns {:?}",
                test_name, x, y, expected_columns, expected_rows, actual_columns, actual_rows
            );
            fail_flag = true;
        }
    }

    // Restore real state regardless of outcome.
    unsafe { gen_ztmegatilemgr::INIT.original()(real_this, original_x as u32, original_y as u32) };
    unsafe { gen_ztmegatilemgr::RECALCULATE_CHARACTERISTICS.original()(real_this) };

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}

