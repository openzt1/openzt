#![allow(dead_code)]

//! Live reimplementation-comparison test battery: builds standalone instances of a reimplemented
//! class, calls the real vanilla function via `.original()`/`.hooked()` on one and the Rust
//! reimplementation on the other, and compares results/state. See CLAUDE.md's "Live
//! Reimplementation-Comparison Tests" section for the full pattern and gotchas.
//!
//! Module layout:
//! - `harness` - generic, battery-agnostic helpers (`RegisteredTest`, `run_registered_tests`,
//!   log-line writers). No knowledge of which tests exist.
//! - `battery` - the actual detours that drive the battery (hooked onto real vanilla boot/tick
//!   functions) and the three ordered `RegisteredTest` lists (`early_tests`/`always_late_tests`/
//!   `live_zoo_tests`) that determine what runs and in what order. Order in these lists is often
//!   load-bearing (see the comments on individual entries) - never reorder without reading why a
//!   test is where it is.
//! - `tests::<class>` - one file per tested class (mirroring `openzt/src/<class>.rs`), holding
//!   that class's `run_..._test(failure_log: &mut Option<std::fs::File>) -> bool` functions and
//!   any private proptest strategies/case structs they need. `battery.rs` references these
//!   functions by path; the test files themselves don't know about list order.
//!
//! **To add a new test**: write `run_..._test` in the right `tests/<class>.rs` (create the file,
//! following an existing one as a template, if this is the class's first live test), then add a
//! `RegisteredTest { name: "...", run: tests::<class>::run_..._test }` entry to the correct list
//! in `battery.rs` - `early_tests`/`always_late_tests` if no live zoo is needed (see each list's
//! own doc comment for the distinction), `live_zoo_tests` if it needs `run_load_live_zoo` to have
//! already succeeded. Check nearby comments in that list for ordering constraints before picking
//! a position. See `openzt/plans/reimplementation-tests-refactor-plan.md` for the state of this
//! module's ongoing split into the layout above.

use std::{any::Any, fmt};

use proptest::test_runner::{FailurePersistence, PersistedSeed};
use tracing::error;

mod harness;

#[cfg(target_os = "windows")]
mod battery;

#[cfg(target_os = "windows")]
mod tests;

/// Redirects the `fwrite`/`fread`-shaped primitives `ZTResearchMgr::save`/`load` (and every other
/// vanilla `*::save`/`*::load`) go through, to in-memory buffers - lets `battery`'s live
/// research save/load comparison call the real `.original()` functions without a real save file.
#[cfg(target_os = "windows")]
mod io_redirect;

pub fn init() {
    #[cfg(target_os = "windows")]
    {
        #[cfg(feature = "tui")]
        let tui_config: Option<&crate::tui_console::TuiConfig> = None;
        #[cfg(not(feature = "tui"))]
        let tui_config = None;

        if let Err(e) = crate::logging::init_with_console(
            &crate::logging::LoggingConfig::default(),
            tui_config,
        ) {
            eprintln!("Failed to initialize logging: {}", e);
        }

        io_redirect::init();

        // Installs `resource_manager::init()`'s hooks so `LAZY_RESOURCE_MAP` is populated before
        // `detour_zoo_main`'s battery runs, letting `ZTMARKETINGMGR_LOAD_CONFIGURATIONS`'s
        // `load_configurations()` call resolve real game resources via `get_file`.
        crate::resource_manager::init();

        // Installs the research/marketing `SAVE`/`LOAD` detours so the corresponding comparison tests'
        // `mgr.save()`/`mgr.load()` calls exercise the actual promoted live path.
        crate::ztresearch::research_save_reimplementation::init();
        crate::ztmarketing::marketing_save_reimplementation::init();

        // ZTShowScriptMgr reimplementation plan, open item 1: installed here (unconditionally, before
        // `run_load_live_zoo`) rather than only after the real zoo has loaded, specifically to exercise
        // `ZTShowScriptMgr::load`/`ZTShowScript::load` against real save data. A prior session tried this
        // and hit a process crash during `run_load_live_zoo` itself - see this function's own diagnostics
        // for what was found.
        crate::ztshowscriptmgr::init();
        crate::ztshow::init();
        crate::ztshowstate::init();
        crate::ztshowinfo::init();
        // ZTShowMgr's detours (stages 2-6: `initShowParams`, the `registerShow`/`unregisterShow`
        // shadow/mirror pair, the `getShowInfo`/`getScriptID` read cutover, the
        // `enterNewMonth`/`update` walk ports, and the `save`/`load` pair) - same reason as the
        // installs above:
        // `openzt-test-dll` never runs `openztlib::init()`, so nothing else installs them, and the
        // ZTSHOWMGR_* tests drive the hooked addresses directly via `.hooked()`.
        crate::ztshowmgr::init();
        crate::ztshowui::init();

        // ztawardmgr's own-method detours (ADD_AWARD/GET_AWARD/SAVE/LOAD/START) are deliberately NOT
        // installed here - ZTAWARDMGR_ADD_AWARD_SAVE_LOAD/START/GET_AWARD rely on `.original()` reaching
        // real, un-hooked vanilla code for comparison against the Rust reimplementation, and in release
        // builds `.original()` is still a raw address cast with no trampoline (debug builds route it
        // through openzt-detour's hook registry) - installing that submodule's detours would make
        // `.original()` loop back into our own code on both sides of those diffs in release. Only the two
        // override-style detours needed for a live diff of their own routing/dispatch logic are installed,
        // each exposing a `call_real` trampoline wrapper so the corresponding test can still reach
        // genuine vanilla behavior once hooked.
        crate::ztawardmgr::eval_award_count_override::init();
        crate::ztawardmgr::show_awards_detour::init();

        // MenuMusicHandler: installs the class's five detours so the MENUMUSICHANDLER_* tests exercise
        // the actual hooked path and MENUMUSICHANDLER_DETOURS_ENABLED can assert the wiring itself
        // (nothing else in the battery distinguishes "detour installed" from "silently still vanilla").
        // The corresponding tests' "real vanilla" pole therefore goes through live_support's real_*
        // trampolines - `.original()` on these five addresses re-enters the Rust detours once hooked in
        // release (raw address cast; debug builds route through the hook registry - see
        // ztgamemgr_menumusichandler's menu_music_handler_detours::test_real doc comment).
        crate::ztgamemgr_menumusichandler::init();

        // Ambients/AmbientsGroup: installs the two classes' five detours (same rationale as the
        // MenuMusicHandler/ZTSoundscape blocks above) so AMBIENTS_DETOURS_ENABLED can assert the wiring
        // itself. Installed before ZTSoundscape::init() below since that class's own init/update now
        // call these Rust methods directly rather than through the detoured addresses.
        crate::ambients::init();

        // ZTSoundscape: installs the class's three detours so the ZTSOUNDSCAPE_* tests exercise the
        // actual hooked path and ZTSOUNDSCAPE_DETOURS_ENABLED can assert the wiring itself (same
        // rationale as the MenuMusicHandler block above). Those tests' "real vanilla" poles therefore
        // go through soundscape_live_support's real_* trampolines for the same per-profile reason.
        crate::ztsoundscape::init();

        // ZooStatus: installs the class's 31 detours (zoostatus-implementation-plan.md Stage 8) so
        // ZOOSTATUS_DETOURS_ENABLED can assert the wiring itself (same rationale as the MenuMusicHandler/
        // ZTSoundscape blocks above). The existing ZOOSTATUS_* comparison tests are unaffected: they call
        // `ZOOSTATUS_*.original()` directly, which keeps reaching real vanilla in debug builds regardless
        // of hook state (routed through the hook registry's trampoline - see `openzt-detour`'s
        // `FunctionDef::original` doc comment).
        crate::zoostatus::init();

        unsafe { battery::detour_zoo_main::init_detours() }.is_err().then(|| {
            error!("Error initialising zoo_main detours");
        });
    }
}

#[derive(Debug, Default, PartialEq)]
struct NoopFailurePersistence;

impl FailurePersistence for NoopFailurePersistence {
    fn load_persisted_failures2(&self, _source_file: Option<&'static str>) -> Vec<PersistedSeed> {
        Vec::new()
    }

    fn save_persisted_failure2(&mut self, _source_file: Option<&'static str>, _seed: PersistedSeed, _shrunken_value: &dyn fmt::Debug) {}

    fn box_clone(&self) -> Box<dyn FailurePersistence> {
        Box::new(NoopFailurePersistence)
    }

    fn eq(&self, other: &dyn FailurePersistence) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
