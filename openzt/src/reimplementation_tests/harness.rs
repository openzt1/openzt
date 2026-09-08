//! Generic, battery-agnostic plumbing for running a named list of comparison tests and logging
//! their pass/fail/skip outcome. `battery.rs` owns *what* runs and in what order (the
//! `RegisteredTest` lists and the detours that drive them); this module only knows how to run a
//! list and how to write the log lines every test shares the same format for.

use std::io::Write;

use tracing::info;

/// A single named entry in a reimplementation-comparison battery's ordered test list. See
/// `battery.rs`'s `early_tests`/`always_late_tests`/`live_zoo_tests` for how these lists are
/// built and why their order matters.
pub(crate) struct RegisteredTest {
    /// Matches the `test_name` the function itself logs under - reused to write an explicit skip
    /// line for a `live_zoo_tests` entry when `run_load_live_zoo` fails, so a gap between the
    /// battery's start/finish markers is always explained by a line in the log, not silence.
    pub(crate) name: &'static str,
    pub(crate) run: fn(&mut Option<std::fs::File>) -> bool,
}

/// Writes a `===`-delimited marker line to the battery's log file - used both for the
/// "N tests expected" line `detour_target` writes when it (re)creates the log, and the
/// "battery finished" line `run_on_completion_reset_test_and_exit` writes at the very end. If the
/// process crashes or hangs mid-battery, the log simply ends without a finish marker, and comparing
/// the number of `Test Passed`/`Test Failed` lines actually present against the expected count named
/// in the start marker shows exactly how far it got.
pub(crate) fn write_battery_marker(failure_log: &mut Option<std::fs::File>, message: &str) {
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("=== {} ===\n", message).as_bytes());
    }
}

/// Runs every entry in `tests` in order, logging an `info!` trace before each call so a crash mid-test
/// names the last test attempted in `openzt.log`, even though the per-test pass/fail/skip line only
/// lands in `failure_log` on that test's own return.
pub(crate) fn run_registered_tests(tests: &[RegisteredTest], failure_log: &mut Option<std::fs::File>) -> bool {
    let mut fail_flag = false;
    for test in tests {
        info!("Running {}", test.name);
        fail_flag |= (test.run)(failure_log);
    }
    fail_flag
}

pub(crate) fn write_success_line(failure_log: &mut Option<std::fs::File>, test_name: &str) {
    let success_line = format!("Test Passed {}\n", test_name);
    if let Some(log_file) = failure_log
        && let Err(write_err) = log_file.write_all(success_line.as_bytes())
    {
        tracing::error!("Failed to write to failure log: {}", write_err);
    }
}
