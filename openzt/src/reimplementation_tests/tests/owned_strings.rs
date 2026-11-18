//! Integrity checks for the ownership model behind `owned_strings`: in production every behavior-set call
//! object is built by Rust, so a release that falls through to vanilla's allocator (the backstop branch of
//! `free_buffer`) means vanilla built an object Rust then freed, and the tracker must not grow across a
//! load, play and reload cycle.

use std::io::Write;

use openzt_detour::generated::ztworldmgr::UPDATE as ZTWORLDMGR_UPDATE;

use crate::owned_strings::{tracked_count, vanilla_free_count};
use crate::reimplementation_tests::battery::detour_zoo_main::run_load_live_zoo;
use crate::reimplementation_tests::harness::finish_test;
use crate::globals::globals;

/// `OWNED_STRINGS_OWNERSHIP_LIVE`: loads the test zoo (a warm-up that settles whatever earlier tests left),
/// steps `ZTWorldMgr::update` for 300 ticks (guests spawn and leave, animals and keepers run their behavior
/// sets), then reloads the same save, which destroys every entity through the real teardown path. Checks:
/// no release took the vanilla branch during the tick or reload phases, and `tracked_count()` after the reload
/// equals its value after the warm-up (nothing leaked, nothing left tracked for a destroyed object).
pub(crate) fn run_owned_strings_ownership_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    const TICKS: u32 = 300;
    const TICK_MS: u32 = 55;

    let test_name = "OWNED_STRINGS_OWNERSHIP_LIVE";
    let mut failures: Vec<String> = Vec::new();

    if !run_load_live_zoo(&mut None) {
        failures.push("warm-up load failed".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let (vanilla_start, tracked_start) = (vanilla_free_count(), tracked_count());

    let world = globals().ztworldmgr_ptr() as *const u32;
    for tick in 0..TICKS {
        if tick % 50 == 0 {
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("CHECKPOINT {} tick {}
", test_name, tick).as_bytes());
                let _ = log_file.flush();
            }
        }
        unsafe { ZTWORLDMGR_UPDATE.hooked()(world, TICK_MS) };
    }
    let (vanilla_ticked, tracked_ticked) = (vanilla_free_count(), tracked_count());

    if !run_load_live_zoo(&mut None) {
        failures.push("reload failed".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let (vanilla_end, tracked_end) = (vanilla_free_count(), tracked_count());

    if vanilla_ticked != vanilla_start {
        failures.push(format!("{} release(s) took the vanilla branch during {} update ticks", vanilla_ticked - vanilla_start, TICKS));
    }
    if vanilla_end != vanilla_ticked {
        failures.push(format!("{} release(s) took the vanilla branch while reloading (entity teardown)", vanilla_end - vanilla_ticked));
    }
    if tracked_end != tracked_start {
        failures.push(format!(
            "tracked buffers: {} after the warm-up load, {} after {} ticks, {} after the reload",
            tracked_start, tracked_ticked, TICKS, tracked_end
        ));
    }
    finish_test(&format!("{} (tracked {} -> {} -> {}, vanilla frees {})", test_name, tracked_start, tracked_ticked, tracked_end, vanilla_end - vanilla_start), failures, failure_log)
}
