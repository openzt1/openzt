//! Compares real vanilla `ZTMarketing`/`ZTMarketingMgr` against their Rust reimplementation
//! (production file `openzt/src/ztmarketing.rs`): increase/decrease/set funding level, mgr
//! update/save/load/clear/dtor, `ZTMarketing::update` (incl. boundary reproductions), and
//! `loadConfigurations` against the path vanilla's own boot-time call passes (captured by
//! `battery.rs`'s transparent path-capture detour). `funding_level_case_strategy` is also used by
//! `ztresearch.rs`'s `ZTResearchBranch` funding-text tests.

use openzt_detour::generated::standalone;
use openzt_detour::generated::ztmarketing;
use openzt_detour::generated::ztmarketingmgr::{
    CLEAR_CONFIGURATIONS as ZTMARKETINGMGR_CLEAR_CONFIGURATIONS, UPDATE as ZTMARKETINGMGR_UPDATE,
    DESTRUCTOR_1 as ZTMARKETINGMGR_DTOR,
};
use proptest::prelude::*;
use std::io::Write;
use std::mem::size_of;
use tracing::{error, info};

use crate::globals::globals;
use crate::reimplementation_tests::battery::detour_zoo_main::CAPTURED_MARKETING_PATH;
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::util::{get_from_memory, ZTBufferString, ZTString};
use crate::ztmarketing::{
    live_support as marketing_live_support, marketing_save_reimplementation, predict_mgr_update, ZTMarketing,
};

/// funding-level name string ids (`23100`/`23101`/`23102`/`23103` - `"%s none"`/`"%s min"`/
/// `"%s normal"`/`"%s max"`, per `ZTResearchFundingLevel::name`'s own doc comment) so the `%s`
/// substitution both sides perform has a real, resolvable template to work with. `cost` is bounded
/// well away from `f32`'s extremes - `funding_text` casts `cost * (1.0/30.0)` to `i32` after
/// rounding, and vanilla's own float-to-int conversion is undefined for non-finite/out-of-i32-range
/// inputs, which isn't a meaningful case to compare.
pub(crate) fn funding_level_case_strategy() -> impl Strategy<Value = (i32, f32)> {
    (prop_oneof![Just(23100i32), Just(23101i32), Just(23102i32), Just(23103i32)], -1_000_000f32..1_000_000f32)
}

/// ZTMARKETING_INCREASE_FUNDING: compares the real `ZTMarketing::increaseFunding` against the
/// reimplemented `increase_funding`, on two independent standalone `ZTMarketing`s (not spliced into
/// any `ZTMarketingMgr`, since `increaseFunding` only reads/writes `this`). `current_funding_level`
/// spans `0..6` against funding tables of `0..5` entries, to cover in-range/top-of-range/
/// one-past-the-end starting values. Compares both the resulting index and vanilla's masked
/// low-byte return value.
pub(crate) fn run_marketing_increase_funding_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETING_INCREASE_FUNDING";
    let mut fail_flag = false;

    match runner.run(&(0u32..6, 0usize..5), |(current_funding_level, level_count)| {
        let real_ptr = marketing_live_support::build_standalone_marketing(current_funding_level, level_count);
        let real_ret = unsafe { ztmarketing::INCREASE_FUNDING.original()(real_ptr as *const u32) };
        let real_index = unsafe { &*real_ptr }.current_funding_level();
        marketing_live_support::destroy_standalone_marketing(real_ptr);

        let reimpl_ptr = marketing_live_support::build_standalone_marketing(current_funding_level, level_count);
        let reimpl_marketing = unsafe { &mut *reimpl_ptr };
        let reimpl_ret = reimpl_marketing.increase_funding();
        let reimpl_index = reimpl_marketing.current_funding_level();
        marketing_live_support::destroy_standalone_marketing(reimpl_ptr);

        prop_assert_eq!(
            real_index,
            reimpl_index,
            "current_funding_level mismatch for start={}, level_count={}",
            current_funding_level,
            level_count
        );
        prop_assert_eq!(
            (real_ret & 0xff) != 0,
            reimpl_ret,
            "return-flag mismatch for start={}, level_count={}",
            current_funding_level,
            level_count
        );
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

/// ZTMARKETING_DECREASE_FUNDING: same shape as `run_marketing_increase_funding_test`, comparing
/// `ZTMarketing::decreaseFunding` against `decrease_funding`.
pub(crate) fn run_marketing_decrease_funding_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETING_DECREASE_FUNDING";
    let mut fail_flag = false;

    match runner.run(&(0u32..6, 0usize..5), |(current_funding_level, level_count)| {
        let real_ptr = marketing_live_support::build_standalone_marketing(current_funding_level, level_count);
        let real_ret = unsafe { ztmarketing::DECREASE_FUNDING.original()(real_ptr as *const u32) };
        let real_index = unsafe { &*real_ptr }.current_funding_level();
        marketing_live_support::destroy_standalone_marketing(real_ptr);

        let reimpl_ptr = marketing_live_support::build_standalone_marketing(current_funding_level, level_count);
        let reimpl_marketing = unsafe { &mut *reimpl_ptr };
        let reimpl_ret = reimpl_marketing.decrease_funding();
        let reimpl_index = reimpl_marketing.current_funding_level();
        marketing_live_support::destroy_standalone_marketing(reimpl_ptr);

        prop_assert_eq!(
            real_index,
            reimpl_index,
            "current_funding_level mismatch for start={}, level_count={}",
            current_funding_level,
            level_count
        );
        prop_assert_eq!(
            (real_ret & 0xff) != 0,
            reimpl_ret,
            "return-flag mismatch for start={}, level_count={}",
            current_funding_level,
            level_count
        );
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

/// ZTMARKETING_SET_FUNDING_LEVEL: compares the real `ZTMarketing::setFundingLevel` against the
/// reimplemented `set_funding_level`. `level` spans `0..6` against funding tables of `0..5`
/// entries, to cover `setFundingLevel`'s "reset to `0`" out-of-range behavior.
pub(crate) fn run_marketing_set_funding_level_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETING_SET_FUNDING_LEVEL";
    let mut fail_flag = false;

    match runner.run(&(0u32..2, 0usize..5, 0u32..6), |(current_funding_level, level_count, level)| {
        let real_ptr = marketing_live_support::build_standalone_marketing(current_funding_level, level_count);
        unsafe { ztmarketing::SET_FUNDING_LEVEL.original()(real_ptr as *const u32, level) };
        let real_index = unsafe { &*real_ptr }.current_funding_level();
        marketing_live_support::destroy_standalone_marketing(real_ptr);

        let reimpl_ptr = marketing_live_support::build_standalone_marketing(current_funding_level, level_count);
        let reimpl_marketing = unsafe { &mut *reimpl_ptr };
        reimpl_marketing.set_funding_level(level);
        let reimpl_index = reimpl_marketing.current_funding_level();
        marketing_live_support::destroy_standalone_marketing(reimpl_ptr);

        prop_assert_eq!(
            real_index,
            reimpl_index,
            "current_funding_level mismatch for start={}, level_count={}, level={}",
            current_funding_level,
            level_count,
            level
        );
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

/// ZTMARKETINGMGR_UPDATE: compares the real `ZTMarketingMgr::update`'s effect on `tick_accumulator`
/// against the reimplemented `update`, for a synthetic manager with no owned `ZTMarketing`
/// (`marketing_ptr = null`) - so `ZTMarketing::update` (which needs a live `GLOBAL_ZTGameMgr`, see
/// `run_marketing_update_test` below) never runs on either side.
pub(crate) fn run_marketingmgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETINGMGR_UPDATE";
    let mut fail_flag = false;

    match runner.run(&(any::<u32>(), any::<u32>()), |(tick_accumulator_before, delta_ticks)| {
        let real_ptr = marketing_live_support::build_standalone_marketing_mgr(tick_accumulator_before, std::ptr::null_mut());
        unsafe { ZTMARKETINGMGR_UPDATE.original()(real_ptr as *const u32, delta_ticks) };
        let real_tick_accumulator = unsafe { &*real_ptr }.tick_accumulator();
        marketing_live_support::destroy_standalone_marketing_mgr(real_ptr);

        let reimpl_ptr = marketing_live_support::build_standalone_marketing_mgr(tick_accumulator_before, std::ptr::null_mut());
        unsafe { &mut *reimpl_ptr }.update(delta_ticks);
        let reimpl_tick_accumulator = unsafe { &*reimpl_ptr }.tick_accumulator();
        marketing_live_support::destroy_standalone_marketing_mgr(reimpl_ptr);

        prop_assert_eq!(
            real_tick_accumulator,
            reimpl_tick_accumulator,
            "tick_accumulator mismatch for tick_accumulator_before={}, delta_ticks={}",
            tick_accumulator_before,
            delta_ticks
        );
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

/// ZTMARKETING_UPDATE: compares the real `ZTMarketingMgr::update`'s effect on a *wired-in*
/// `ZTMarketing` against the reimplemented `update`, with `tick_accumulator` fixed so a threshold
/// crossing always happens (`delta_ticks` generated `3000..10000`, always `> 359` days' worth per
/// `predict_mgr_update`) - so `ZTMarketing::update` genuinely runs on both sides, not just the
/// accumulator bookkeeping already covered by `ZTMARKETINGMGR_UPDATE` above. Registered in
/// `always_late_tests()`, which runs from `run_on_completion_reset_test_and_exit`'s `updateSim`
/// injection point - `GLOBAL_ZTGameMgr` isn't constructed yet at the earlier `early_tests()` point.
///
/// The funding table has `1..5` entries (`ZTMarketing::update`'s unchecked
/// `funding_level(current_funding_level)` read needs a real, non-empty table to be safe), with
/// `current_funding_level` spanning the whole table rather than fixed at index `0`.
///
/// `available_cash` is generated as `cash_delta * cash_multiplier` for `cash_multiplier` in
/// `0.0..2.0` (`cash_delta` computed the same way as `ZTMarketing::update`'s own
/// `DAYS_TO_FUNDING_SCALE` formula below), so roughly half of generated cases land unaffordable and
/// half affordable - the affordable `<=` branch calls
/// `ZooStatus::spendMarketing`/`ZTGameMgr::subtractCash` on the real `GLOBAL_ZTGameMgr` singleton on
/// both sides. The exact `available_cash == cash_delta`
/// boundary is separately covered deterministically by `run_marketing_update_boundary_test` (real
/// side only) and `run_marketing_update_reimpl_boundary_test` (reimplemented side only) below.
pub(crate) fn run_marketing_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETING_UPDATE";
    let mut fail_flag = false;

    if marketing_live_support::ztgamemgr_ptr_is_null() {
        info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
        return fail_flag;
    }

    // Mirrors `ZTMarketing::update`'s own `DAYS_TO_FUNDING_SCALE` constant so the generated
    // `available_cash` can land just below the real `cash_delta` (`days * cost * scale`, where
    // `days` comes from `predict_mgr_update`'s tick-to-day conversion).
    const DAYS_TO_FUNDING_SCALE: f32 = 1.0 / 43200.0;

    match runner.run(
        &(3000u32..10000, prop::collection::vec(1f32..1000f32, 1..5), any::<usize>(), 0.0f32..2.0f32),
        |(delta_ticks, costs, raw_index, cash_multiplier)| {
            let current_funding_level = (raw_index % costs.len()) as u32;
            let levels: Vec<(i32, f32)> = costs.iter().map(|&cost| (0i32, cost)).collect();
            let selected_cost = costs[current_funding_level as usize];
            let (_, days) = predict_mgr_update(0, delta_ticks);
            let cash_delta = days as f32 * selected_cost * DAYS_TO_FUNDING_SCALE;
            let available_cash = (cash_delta * cash_multiplier).max(0.0);

            let real_marketing_ptr = marketing_live_support::build_standalone_marketing_with_levels(current_funding_level, &levels);
            let real_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, real_marketing_ptr);
            marketing_live_support::with_ztgamemgr_cash(available_cash, || unsafe {
                ZTMARKETINGMGR_UPDATE.original()(real_mgr_ptr as *const u32, delta_ticks);
            });
            let real_tick_accumulator = unsafe { &*real_mgr_ptr }.tick_accumulator();
            let real_index = unsafe { &*real_marketing_ptr }.current_funding_level();
            marketing_live_support::destroy_standalone_marketing_mgr(real_mgr_ptr);
            marketing_live_support::destroy_standalone_marketing(real_marketing_ptr);

            let reimpl_marketing_ptr = marketing_live_support::build_standalone_marketing_with_levels(current_funding_level, &levels);
            let reimpl_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, reimpl_marketing_ptr);
            marketing_live_support::with_ztgamemgr_cash(available_cash, || unsafe { &mut *reimpl_mgr_ptr }.update(delta_ticks));
            let reimpl_tick_accumulator = unsafe { &*reimpl_mgr_ptr }.tick_accumulator();
            let reimpl_index = unsafe { &*reimpl_marketing_ptr }.current_funding_level();
            marketing_live_support::destroy_standalone_marketing_mgr(reimpl_mgr_ptr);
            marketing_live_support::destroy_standalone_marketing(reimpl_marketing_ptr);

            prop_assert_eq!(
                real_tick_accumulator,
                reimpl_tick_accumulator,
                "tick_accumulator mismatch for delta_ticks={}, current_funding_level={}, costs={:?}",
                delta_ticks,
                current_funding_level,
                costs
            );
            prop_assert_eq!(
                real_index,
                reimpl_index,
                "current_funding_level mismatch for delta_ticks={}, current_funding_level={}, costs={:?}",
                delta_ticks,
                current_funding_level,
                costs
            );
            Ok(())
        },
    ) {
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

/// Deterministic single-case probe of the real side of `ZTMARKETING_UPDATE`'s affordable branch.
/// Calls only the real `ZTMarketingMgr::update`, skipping the reimplemented side entirely, so a
/// crash unambiguously means the real vanilla call path. `available_cash` is pinned to exactly
/// `cash_delta` (the `<=` boundary itself, never exercised by `run_marketing_update_test` above),
/// forcing the real side onto the affordable `spendMarketing`/`subtractCash` branch every time.
///
/// Known to crash when `zoo.exe` runs under Windows' "Windows 7" compatibility-mode shim; runs
/// clean otherwise. If this test crashes the battery, check `zoo.exe`'s Compatibility tab before
/// assuming a regression. The reimplemented counterpart is `run_marketing_update_reimpl_boundary_test`
/// below.
pub(crate) fn run_marketing_update_boundary_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMARKETING_UPDATE_BOUNDARY_REPRO";

    if marketing_live_support::ztgamemgr_ptr_is_null() {
        info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
        return false;
    }

    const DAYS_TO_FUNDING_SCALE: f32 = 1.0 / 43200.0;
    let delta_ticks: u32 = 5000;
    let cost: f32 = 100.0;
    let levels: Vec<(i32, f32)> = vec![(0, cost)];
    let (_, days) = predict_mgr_update(0, delta_ticks);
    let cash_delta = days as f32 * cost * DAYS_TO_FUNDING_SCALE;

    let real_marketing_ptr = marketing_live_support::build_standalone_marketing_with_levels(0, &levels);
    let real_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, real_marketing_ptr);

    info!(
        "{}: about to call real ZTMarketingMgr::update with available_cash == cash_delta ({}) - known to crash under Windows 7 compatibility mode, expected clean otherwise",
        test_name, cash_delta
    );
    marketing_live_support::with_ztgamemgr_cash(cash_delta, || unsafe {
        ZTMARKETINGMGR_UPDATE.original()(real_mgr_ptr as *const u32, delta_ticks);
    });
    info!("{}: real call returned without crashing", test_name);

    marketing_live_support::destroy_standalone_marketing_mgr(real_mgr_ptr);
    marketing_live_support::destroy_standalone_marketing(real_marketing_ptr);

    write_success_line(failure_log, test_name);
    false
}

/// Deterministic single-case coverage for the *reimplemented* side of `ZTMARKETING_UPDATE`'s affordable
/// branch (`ZTMarketing::update` -> `spend_marketing`/`subtract_cash`, unlike
/// `run_marketing_update_boundary_test` above, which only ever calls genuine vanilla).
///
/// Calls only the reimplemented `ZTMarketingMgr::update`, skipping the real vanilla side entirely.
/// `available_cash` is pinned to exactly `cash_delta`, forcing the affordable branch every time.
pub(crate) fn run_marketing_update_reimpl_boundary_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMARKETING_UPDATE_REIMPL_BOUNDARY_REPRO";

    if marketing_live_support::ztgamemgr_ptr_is_null() {
        info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
        return false;
    }

    const DAYS_TO_FUNDING_SCALE: f32 = 1.0 / 43200.0;
    let delta_ticks: u32 = 5000;
    let cost: f32 = 100.0;
    let levels: Vec<(i32, f32)> = vec![(0, cost)];
    let (_, days) = predict_mgr_update(0, delta_ticks);
    let cash_delta = days as f32 * cost * DAYS_TO_FUNDING_SCALE;

    let reimpl_marketing_ptr = marketing_live_support::build_standalone_marketing_with_levels(0, &levels);
    let reimpl_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, reimpl_marketing_ptr);

    info!("{}: about to call reimplemented ZTMarketingMgr::update with available_cash == cash_delta ({})", test_name, cash_delta);
    marketing_live_support::with_ztgamemgr_cash(cash_delta, || unsafe { &mut *reimpl_mgr_ptr }.update(delta_ticks));
    info!("{}: reimplemented call returned without crashing", test_name);

    marketing_live_support::destroy_standalone_marketing_mgr(reimpl_mgr_ptr);
    marketing_live_support::destroy_standalone_marketing(reimpl_marketing_ptr);

    write_success_line(failure_log, test_name);
    false
}

/// ZTMARKETING_GET_FUNDING_TEXT: compares the real `ZTMarketing::getFundingText`'s output against
/// the reimplemented `ZTMarketing::funding_text`, for a standalone marketing with a generated
/// funding table and `current_funding_level` spanning negative/in-range/out-of-range relative to
/// the table's length. Same shape as `tests::ztresearch::run_funding_text_test`, reusing its
/// `funding_level_case_strategy` for the (name_id, cost) generation.
pub(crate) fn run_marketing_funding_text_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETING_GET_FUNDING_TEXT";
    let mut fail_flag = false;

    match runner.run(&(-2i32..4i32, prop::collection::vec(funding_level_case_strategy(), 0..3)), |(current_funding_level, levels)| {
        let current_funding_level = current_funding_level as u32;
        let marketing_ptr = marketing_live_support::build_standalone_marketing_with_levels(current_funding_level, &levels);

        let mut buffer = [0u32; 3];
        unsafe {
            ztmarketing::GET_FUNDING_TEXT.original()(marketing_ptr as *const u32, buffer.as_mut_ptr() as *const u32);
        }
        let real_text = get_from_memory::<ZTBufferString>(buffer.as_ptr() as u32).copy_to_string();
        let reimpl_text = unsafe { &*marketing_ptr }.funding_text();

        marketing_live_support::destroy_standalone_marketing(marketing_ptr);

        prop_assert_eq!(
            real_text,
            reimpl_text,
            "funding_text mismatch for current_funding_level={}, levels={:?}",
            current_funding_level,
            levels
        );
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

/// ZTMARKETINGMGR_SAVE: compares the real `ZTMarketingMgr::save`'s captured output (via
/// `io_redirect`) against the single little-endian `u32` funding-level index vanilla is expected to
/// write - `0` when no `ZTMarketing` is owned.
pub(crate) fn run_marketingmgr_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETINGMGR_SAVE";
    let mut fail_flag = false;

    match runner.run(&(any::<bool>(), 0u32..10), |(has_marketing, current_funding_level)| {
        let marketing_ptr = if has_marketing {
            marketing_live_support::build_standalone_marketing(current_funding_level, 0)
        } else {
            std::ptr::null_mut()
        };
        let mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, marketing_ptr);
        let mgr = unsafe { &mut *mgr_ptr };

        let dummy_file: u32 = 0;
        io_redirect::begin_capture();
        let _ = mgr.save(&dummy_file as *const u32);
        let captured_bytes = io_redirect::end_capture();

        marketing_live_support::destroy_standalone_marketing_mgr(mgr_ptr);
        marketing_live_support::destroy_standalone_marketing(marketing_ptr);

        let expected_index: u32 = if has_marketing { current_funding_level } else { 0 };
        let expected_bytes = expected_index.to_le_bytes().to_vec();

        prop_assert_eq!(
            captured_bytes,
            expected_bytes,
            "ZTMarketingMgr::save byte mismatch for has_marketing={}, current_funding_level={}",
            has_marketing,
            current_funding_level
        );
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

/// ZTMARKETINGMGR_LOAD: compares the real `ZTMarketingMgr::load`'s effect on the owned
/// `ZTMarketing`'s funding-level index (and its own return value) against
/// `marketing_save_reimplementation::predict_load`, for a generated funding-level table size,
/// starting index, save-format version (spanning both sides of the `0x3a` threshold), and stream
/// content - `bytes_present = false` supplies an empty replay buffer, exercising `load`'s
/// read-failure abort path (`predict_load`'s `None` branch) when `version` is above the threshold.
pub(crate) fn run_marketingmgr_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETINGMGR_LOAD";
    let mut fail_flag = false;

    match runner.run(
        &(0u32..0x50, any::<u32>(), 0usize..6, 0u32..8, any::<bool>()),
        |(version, saved_value, level_count, current_funding_level, bytes_present)| {
            let marketing_ptr = marketing_live_support::build_standalone_marketing(current_funding_level, level_count);
            let mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, marketing_ptr);
            let mgr = unsafe { &mut *mgr_ptr };

            let bytes = if bytes_present { saved_value.to_le_bytes().to_vec() } else { Vec::new() };
            let file_buffer = [0u32; 4];
            io_redirect::begin_replay(bytes);
            let load_ret = mgr.load(file_buffer.as_ptr(), version);
            io_redirect::end_replay();

            let real_index = unsafe { &*marketing_ptr }.current_funding_level();
            marketing_live_support::destroy_standalone_marketing_mgr(mgr_ptr);
            marketing_live_support::destroy_standalone_marketing(marketing_ptr);

            let read_value = bytes_present.then_some(saved_value);
            let (expected_ok, expected_index) = marketing_save_reimplementation::predict_load(version, read_value, level_count, current_funding_level);

            prop_assert_eq!(
                load_ret,
                expected_ok,
                "return value mismatch for version={}, level_count={}, current_funding_level={}, bytes_present={}",
                version,
                level_count,
                current_funding_level,
                bytes_present
            );
            prop_assert_eq!(
                real_index,
                expected_index,
                "current_funding_level mismatch for version={}, saved_value={}, level_count={}, current_funding_level={}, bytes_present={}",
                version,
                saved_value,
                level_count,
                current_funding_level,
                bytes_present
            );
            Ok(())
        },
    ) {
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

/// ZTMARKETINGMGR_CLEAR_CONFIGURATIONS: compares the real `ZTMarketingMgr::clearConfigurations`
/// against the reimplemented `clear_configurations`, confirming both leave `tick_accumulator == 0`
/// and `marketing_ptr` left dangling (non-null, pointing at now-freed memory) rather than nulled.
/// The stale pointer is never dereferenced further here, only checked for non-nullness via
/// `marketing_ptr_raw()`.
///
/// `clearConfigurations` **frees** the owned `ZTMarketing`, and freeing a Rust `Box`-allocated one
/// through vanilla's own destructor/delete path is a cross-heap risk - so the "real" side's
/// `ZTMarketing` is allocated via the native `standalone::OPERATOR_NEW` and initialized via
/// `ztmarketing::CONSTRUCTOR`, keeping the real free heap-consistent with how the memory was
/// allocated.
pub(crate) fn run_marketingmgr_clear_configurations_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTMARKETINGMGR_CLEAR_CONFIGURATIONS";
    let mut fail_flag = false;

    match runner.run(&any::<u32>(), |tick_accumulator| {
        let real_raw = unsafe { standalone::OPERATOR_NEW.original()(size_of::<ZTMarketing>() as u32) };
        prop_assume!(!real_raw.is_null());
        let real_marketing_ptr = unsafe { ztmarketing::CONSTRUCTOR.original()(real_raw as *const u32) } as *mut ZTMarketing;

        let real_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(tick_accumulator, real_marketing_ptr);
        unsafe { ZTMARKETINGMGR_CLEAR_CONFIGURATIONS.original()(real_mgr_ptr as *const u32) };
        let real_mgr = unsafe { &*real_mgr_ptr };
        let real_tick_accumulator = real_mgr.tick_accumulator();
        let real_marketing_ptr_nonnull = real_mgr.marketing_ptr_raw() != 0;
        marketing_live_support::destroy_standalone_marketing_mgr(real_mgr_ptr);

        let reimpl_marketing_ptr = marketing_live_support::build_standalone_marketing(0, 0);
        let reimpl_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(tick_accumulator, reimpl_marketing_ptr);
        unsafe { &mut *reimpl_mgr_ptr }.clear_configurations();
        let reimpl_mgr = unsafe { &*reimpl_mgr_ptr };
        let reimpl_tick_accumulator = reimpl_mgr.tick_accumulator();
        let reimpl_marketing_ptr_nonnull = reimpl_mgr.marketing_ptr_raw() != 0;
        marketing_live_support::destroy_standalone_marketing_mgr(reimpl_mgr_ptr);

        prop_assert_eq!(real_tick_accumulator, 0, "real tick_accumulator not reset for tick_accumulator={}", tick_accumulator);
        prop_assert_eq!(reimpl_tick_accumulator, 0, "reimplemented tick_accumulator not reset for tick_accumulator={}", tick_accumulator);
        prop_assert!(real_marketing_ptr_nonnull, "real marketing_ptr unexpectedly nulled for tick_accumulator={}", tick_accumulator);
        prop_assert!(reimpl_marketing_ptr_nonnull, "reimplemented marketing_ptr unexpectedly nulled for tick_accumulator={}", tick_accumulator);
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

/// ZTMARKETINGMGR_DTOR: exercises the teardown hazard `ztmarketing.rs`'s `marketing_dtor_detour`
/// module doc comment describes - vanilla's own `ztmarketingmgr::DESTRUCTOR_1` scalar-deleting destructor, if
/// ever allowed to run over a Rust-`Vec`-allocated funding table, would call `operator delete` on
/// memory Rust's global allocator owns (the same cross-allocator hazard CLAUDE.md's "Live
/// Reimplementation-Comparison Tests" section documents for `ZTThoughtMgr`). This deliberately never
/// calls `.original()()` against Rust-allocated memory - that would just reproduce the crash the
/// detour exists to prevent.
///
/// Two independent halves, like `run_marketingmgr_clear_configurations_test` above:
/// - **Real**: a fresh, genuinely vanilla-allocated `ZTMarketingMgr`+`ZTMarketing` (empty funding
///   table, same as that test's real side), torn down via `ZTMARKETINGMGR_DTOR.original()` with
///   `flags=0` (never deletes `this`) - real-allocated, real-freed, so this is safe regardless of the
///   fix and just confirms the real destructor is still callable/well-behaved and returns `this`.
/// - **Reimplemented**: a standalone, Rust-`Vec`-allocated non-empty funding table (via
///   `live_support::build_standalone_marketing_with_levels`), torn down via `ZTMarketingMgr::destroy`
///   directly - the actual free path the fix protects.
pub(crate) fn run_marketingmgr_dtor_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMARKETINGMGR_DTOR";

    let real_marketing_raw = unsafe { standalone::OPERATOR_NEW.original()(size_of::<ZTMarketing>() as u32) };
    if real_marketing_raw.is_null() {
        error!("{}: OPERATOR_NEW returned null for ZTMarketing, skipping real-side check", test_name);
    } else {
        let real_marketing_ptr = unsafe { ztmarketing::CONSTRUCTOR.original()(real_marketing_raw as *const u32) } as *mut ZTMarketing;
        let real_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, real_marketing_ptr);
        let real_this = unsafe { ZTMARKETINGMGR_DTOR.original()(real_mgr_ptr as *const u32, 0u8) };
        if real_this != (real_mgr_ptr as *const u32) {
            error!("{}: real destructor returned {:?}, expected {:?} (this)", test_name, real_this, real_mgr_ptr);
            marketing_live_support::destroy_standalone_marketing_mgr(real_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: real destructor did not return `this`\n", test_name).as_bytes());
            }
            return true;
        }
        // The real destructor already freed `marketing_ptr`'s ZTMarketing (real-allocated, real-freed)
        // - only the outer Box-allocated ZTMarketingMgr wrapper remains to free here.
        marketing_live_support::destroy_standalone_marketing_mgr(real_mgr_ptr);
    }

    let reimpl_marketing_ptr = marketing_live_support::build_standalone_marketing_with_levels(1, &[(100, 5.0), (101, 10.0), (102, 15.0)]);
    let reimpl_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(42, reimpl_marketing_ptr);
    unsafe { &mut *reimpl_mgr_ptr }.destroy();
    let reimpl_mgr = unsafe { &*reimpl_mgr_ptr };
    let tick_accumulator_reset = reimpl_mgr.tick_accumulator() == 0;
    let marketing_ptr_left_dangling = reimpl_mgr.marketing_ptr_raw() != 0;
    marketing_live_support::destroy_standalone_marketing_mgr(reimpl_mgr_ptr);

    if tick_accumulator_reset && marketing_ptr_left_dangling {
        info!("{} passed", test_name);
        write_success_line(failure_log, test_name);
        false
    } else {
        error!(
            "{} failed: tick_accumulator_reset={}, marketing_ptr_left_dangling={}",
            test_name, tick_accumulator_reset, marketing_ptr_left_dangling
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "Test Failed {}: tick_accumulator_reset={}, marketing_ptr_left_dangling={}\n",
                    test_name, tick_accumulator_reset, marketing_ptr_left_dangling
                )
                .as_bytes(),
            );
        }
        true
    }
}

/// ZTMARKETINGMGR_LOAD_CONFIGURATIONS: compares the real, live `globals().ztmarketingmgr()`'s
/// funding table - populated by vanilla's own untouched boot-time `loadConfigurations` call, whose
/// path was captured into `CAPTURED_MARKETING_PATH` - against this crate's own
/// `ZTMarketingMgr::load_configurations` reimplementation, run directly on a standalone
/// `ZTMarketingMgr` with the same captured path. Not a proptest - there's exactly one real path/one
/// real answer to compare. Skipped (not failed) if the path was never captured, or if
/// `GLOBAL_ZTMarketingMgr` itself isn't initialized yet.
pub(crate) fn run_marketingmgr_load_configurations_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMARKETINGMGR_LOAD_CONFIGURATIONS";

    let Some(path) = CAPTURED_MARKETING_PATH.get() else {
        info!("Skipping {}: no path captured from ZTMarketingMgr::loadConfigurations", test_name);
        write_success_line(failure_log, &format!("{} (skipped: no path captured)", test_name));
        return false;
    };

    if globals().ztmarketingmgr_ptr().is_null() {
        info!("Skipping {}: GLOBAL_ZTMarketingMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTMarketingMgr not initialized)", test_name));
        return false;
    }

    let expected_marketing = globals().ztmarketingmgr().marketing();
    let expected_levels: Vec<(i32, i32, u32)> =
        expected_marketing.map(|m| m.funding_levels().iter().map(|l| (l.name_id(), l.benefit(), l.cost().to_bits())).collect()).unwrap_or_default();
    let expected_index = expected_marketing.map(|m| m.current_funding_level()).unwrap_or(0);

    let mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, std::ptr::null_mut());
    let mgr = unsafe { &mut *mgr_ptr };
    mgr.load_configurations(path);

    let actual_marketing = mgr.marketing();
    let actual_levels: Vec<(i32, i32, u32)> =
        actual_marketing.map(|m| m.funding_levels().iter().map(|l| (l.name_id(), l.benefit(), l.cost().to_bits())).collect()).unwrap_or_default();
    let actual_index = actual_marketing.map(|m| m.current_funding_level()).unwrap_or(0);

    mgr.clear_configurations();
    marketing_live_support::destroy_standalone_marketing_mgr(mgr_ptr);

    if expected_levels == actual_levels && expected_index == actual_index {
        info!("{} passed for path '{}'", test_name, path);
        write_success_line(failure_log, test_name);
        false
    } else {
        error!(
            "{} failed for path '{}': expected_levels={:?}, actual_levels={:?}, expected_index={}, actual_index={}",
            test_name, path, expected_levels, actual_levels, expected_index, actual_index
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "Test Failed {}: path '{}', expected_levels={:?}, actual_levels={:?}, expected_index={}, actual_index={}\n",
                    test_name, path, expected_levels, actual_levels, expected_index, actual_index
                )
                .as_bytes(),
            );
        }
        true
    }
}

/// ZTMARKETINGMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE: genuine full round-trip against the real, live
/// `globals().ztmarketingmgr()` singleton - safe because `load` is a pure decode with a ready-made
/// pure oracle, `marketing_save_reimplementation::predict_load`. Snapshots the singleton's current
/// funding-level index, captures real `save()`'s bytes (`.hooked()` - the detoured reimplementation,
/// installed by this battery's own `init()`), replays them into real `load()` at the live save-format
/// version, and asserts the resulting index matches `predict_load`'s prediction computed from the
/// pre-save index/table length.
pub(crate) fn run_ztmarketingmgr_real_zoo_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTMARKETINGMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE";
    const CURRENT_VERSION: u32 = 0x100;
    let mut fail_flag = false;

    let mgr = unsafe { &mut *globals().ztmarketingmgr_ptr() };
    let Some(marketing) = mgr.marketing() else {
        info!("{}: no real ZTMarketing config loaded - nothing to round-trip, skipping", test_name);
        write_success_line(failure_log, &format!("{} (skipped: no real marketing config loaded)", test_name));
        return false;
    };
    let index_before = marketing.current_funding_level();
    let level_count = marketing.funding_levels().len();

    let dummy_file: u32 = 0;
    io_redirect::begin_capture();
    let save_ok = mgr.save(&dummy_file as *const u32);
    let captured_bytes = io_redirect::end_capture();

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(
            format!("CHECKPOINT {} index_before={} level_count={} bytes={}\n", test_name, index_before, level_count, captured_bytes.len()).as_bytes(),
        );
    }

    if !save_ok || captured_bytes.len() != 4 {
        error!("{}: real save() failed or produced an unexpected byte count ({})", test_name, captured_bytes.len());
        fail_flag = true;
    }

    let read_value = (captured_bytes.len() == 4).then(|| u32::from_le_bytes(captured_bytes[..4].try_into().unwrap()));
    let (expected_ok, expected_index) = marketing_save_reimplementation::predict_load(CURRENT_VERSION, read_value, level_count, index_before);

    io_redirect::begin_replay(captured_bytes);
    let load_ok = mgr.load(&dummy_file as *const u32, CURRENT_VERSION);
    io_redirect::end_replay();

    let index_after = mgr.marketing().map(|m| m.current_funding_level());

    if load_ok != expected_ok || index_after != Some(expected_index) {
        error!(
            "{}: real load() result didn't match predict_load's oracle (load_ok={}, expected_ok={}, index_after={:?}, expected_index={})",
            test_name, load_ok, expected_ok, index_after, expected_index
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "Test Failed {}: load_ok={} expected_ok={} index_after={:?} expected_index={}\n",
                    test_name, load_ok, expected_ok, index_after, expected_index
                )
                .as_bytes(),
            );
        }
        fail_flag = true;
    }

    if !fail_flag {
        info!("{}: real funding-level index {} round-tripped to {} matching predict_load's oracle", test_name, index_before, expected_index);
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
