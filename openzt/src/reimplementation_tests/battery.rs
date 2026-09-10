//! The reimplementation-tests battery: the harness detours that drive the whole comparison
//! run (hooked onto real vanilla boot/tick functions - see each detour's own doc comment for
//! why that particular hook point was chosen), plus the ordered `RegisteredTest` lists
//! (`early_tests`/`always_late_tests`/`live_zoo_tests`) that determine what runs and in what
//! order. See `openzt/plans/reimplementation-tests-refactor-plan.md` for the ongoing split of
//! this module's test bodies out into `tests/<class>.rs`.

#[cfg(target_os = "windows")]
use crate::detour_mod;

use super::harness::{RegisteredTest, run_registered_tests, write_battery_marker, write_success_line};
use super::tests;

#[cfg(target_os = "windows")]
#[detour_mod]
pub(super) mod detour_zoo_main {
    use std::{
        backtrace::Backtrace,
        cell::Cell,
        ffi::{CStr, CString},
        fs::OpenOptions,
        io::Write,
        sync::{
            atomic::{AtomicBool, Ordering},
            Once, OnceLock,
        },
    };

    thread_local! {
        static BACKTRACE: Cell<Option<Backtrace>> = const { Cell::new(None) };
    }

    #[cfg(target_os = "windows")]
    use openzt_detour::generated::bfapp::LOAD_LANG_DLLS;
    use openzt_detour::generated::standalone;
    use openzt_detour::generated::ztapp::UPDATE_SIM;
    use openzt_detour::generated::ztmarketingmgr::LOAD_CONFIGURATIONS;
    use openzt_detour::generated::ztui_gameopts::LOAD_FILE as ZTUI_GAMEOPTS_LOAD_FILE;
    use tracing::{error, info};

    use super::tests;
    use super::{RegisteredTest, run_registered_tests, write_battery_marker, write_success_line};

    /// Runs `BFTILE_GET_LOCAL_ELEVATION` first (the position this battery's formerly hand-inlined
    /// version occupied - the very first thing run after `detour_target` creates the log), then
    /// every other early-phase entry; folded into one ordinary `RegisteredTest` list since Stage 6
    /// of `openzt/plans/reimplementation-tests-refactor-plan.md` (previously the first nine of
    /// these ran as hand-inlined proptest blocks sharing a single `proptest::TestRunner`, tracked
    /// by a separate `INLINE_TEST_COUNT` constant instead of this list's own length).
    fn early_tests() -> Vec<RegisteredTest> {
        vec![
            RegisteredTest { name: "BFTILE_GET_LOCAL_ELEVATION", run: tests::ztresearch::run_bftile_get_local_elevation_test },
            RegisteredTest { name: "BFENTITY_GET_FOOTPRINT", run: tests::footprints::run_bfentity_get_footprint_tests },
            RegisteredTest { name: "ZTUNIT_GET_FOOTPRINT", run: tests::footprints::run_ztunit_get_footprint_tests },
            RegisteredTest { name: "ZTANIMAL_GET_FOOTPRINT", run: tests::footprints::run_ztanimal_get_footprint_tests },
            RegisteredTest { name: "ZTRESEARCHBRANCH_FUNDING", run: tests::ztresearch::run_research_branch_funding_test },
            RegisteredTest { name: "ZTRESEARCHBRANCH_PCT_DAYS_REMAINING", run: tests::ztresearch::run_research_branch_pct_days_remaining_test },
            RegisteredTest { name: "ZTMARKETING_INCREASE_FUNDING", run: tests::ztmarketing::run_marketing_increase_funding_test },
            RegisteredTest { name: "ZTMARKETING_DECREASE_FUNDING", run: tests::ztmarketing::run_marketing_decrease_funding_test },
            RegisteredTest { name: "ZTMARKETING_SET_FUNDING_LEVEL", run: tests::ztmarketing::run_marketing_set_funding_level_test },
            RegisteredTest { name: "ZTMARKETINGMGR_UPDATE", run: tests::ztmarketing::run_marketingmgr_update_test },
            RegisteredTest { name: "ZTMARKETINGMGR_SAVE", run: tests::ztmarketing::run_marketingmgr_save_test },
            RegisteredTest { name: "ZTMARKETINGMGR_LOAD", run: tests::ztmarketing::run_marketingmgr_load_test },
            RegisteredTest { name: "ZTMARKETINGMGR_CLEAR_CONFIGURATIONS", run: tests::ztmarketing::run_marketingmgr_clear_configurations_test },
            RegisteredTest { name: "ZTMARKETINGMGR_DTOR", run: tests::ztmarketing::run_marketingmgr_dtor_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_ADD_THOUGHT", run: tests::ztthoughtmgr::run_thoughtmgr_add_thought_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_THINKER", run: tests::ztthoughtmgr::run_thoughtmgr_remove_thoughts_by_thinker_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_OBJECT", run: tests::ztthoughtmgr::run_thoughtmgr_remove_thoughts_by_object_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_HABITAT", run: tests::ztthoughtmgr::run_thoughtmgr_remove_thoughts_by_habitat_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_GET_THOUGHTS_BY_THINKER", run: tests::ztthoughtmgr::run_thoughtmgr_get_thoughts_by_thinker_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_GET_THOUGHTS_BY_OBJECT", run: tests::ztthoughtmgr::run_thoughtmgr_get_thoughts_by_object_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_GET_THOUGHTS_BY_HABITAT", run: tests::ztthoughtmgr::run_thoughtmgr_get_thoughts_by_habitat_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_SAVE", run: tests::ztthoughtmgr::run_thoughtmgr_save_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_LOAD", run: tests::ztthoughtmgr::run_thoughtmgr_load_test },
            RegisteredTest { name: "ZTAWARDMGR_ADD_AWARD_SAVE_LOAD", run: tests::ztawardmgr::run_awardmgr_add_award_save_load_test },
            RegisteredTest { name: "ZTAWARDMGR_START", run: tests::ztawardmgr::run_awardmgr_start_test },
            RegisteredTest { name: "ZTAWARDMGR_GET_AWARD", run: tests::ztawardmgr::run_awardmgr_get_award_test },
            RegisteredTest { name: "ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT", run: tests::ztscenariosimplegoal::run_ztscenariosimplegoal_eval_award_count_test },
            // ZTShowScriptMgr save/load wire format - independent of any live zoo/manager state, so these
            // run from the early battery alongside the award tests above.
            RegisteredTest { name: "ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE", run: tests::ztshowscriptmgr::run_ztshowscriptmgr_save_load_roundtrip_live_test },
            RegisteredTest { name: "ZTSHOWSCRIPTMGR_LOAD_VERSION_GATES_LIVE", run: tests::ztshowscriptmgr::run_ztshowscriptmgr_load_version_gates_live_test },
            // The following seven ran as hand-inlined proptest blocks in `detour_target`, straight after
            // this list, until Stage 6 of the refactor plan folded them into ordinary `RegisteredTest`
            // entries at the same position - order preserved exactly.
            RegisteredTest { name: "ZTRESEARCHMGR_SAVE", run: tests::ztresearch::run_ztresearchmgr_save_test },
            RegisteredTest { name: "ZTRESEARCHMGR_LOAD", run: tests::ztresearch::run_ztresearchmgr_load_test },
            RegisteredTest { name: "ZTRESEARCHMGR_LOAD_CORRUPT_STREAM", run: tests::ztresearch::run_ztresearchmgr_load_corrupt_stream_test },
            RegisteredTest { name: "ZTRESEARCHMGR_UPDATE", run: tests::ztresearch::run_ztresearchmgr_update_test },
            RegisteredTest { name: "ZTRESEARCHMGR_FORCE_RESEARCH", run: tests::ztresearch::run_ztresearchmgr_force_research_test },
            RegisteredTest { name: "ZTRESEARCHMGR_LOOKUPS", run: tests::ztresearch::run_ztresearchmgr_lookups_test },
            RegisteredTest { name: "ZTRESEARCHMGR_SET_EFFECT_DISCOUNT", run: tests::ztresearch::run_ztresearchmgr_set_effect_discount_test },
        ]
    }

    /// Runs unconditionally in `run_on_completion_reset_test_and_exit`, before the live-zoo gate. The
    /// two `*_ORIGINAL_ROUTES_TO_TRAMPOLINE` entries only exist in debug builds - see `generated.rs`'s
    /// module doc comment on `.original()`'s per-profile routing. `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`
    /// is first - the position this battery's formerly hand-inlined version occupied (it ran before
    /// this whole list, straight after `run_on_completion_reset_test_and_exit` opened the log for
    /// appending - see Stage 6 of `openzt/plans/reimplementation-tests-refactor-plan.md`).
    fn always_late_tests() -> Vec<RegisteredTest> {
        let mut tests = vec![
            RegisteredTest { name: "ZTRESEARCHPROGRAM_ON_COMPLETION_RESET", run: tests::ztresearch::run_ztresearchprogram_on_completion_reset_test },
            RegisteredTest { name: "ZTRESEARCHBRANCH_FUNDING_TEXT", run: tests::ztresearch::run_funding_text_test },
            RegisteredTest { name: "ZTRESEARCHBRANCH_UPDATE", run: tests::ztresearch::run_branch_update_test },
            RegisteredTest { name: "ZTRESEARCHBRANCH_UPDATE_REIMPL_BOUNDARY_REPRO", run: tests::ztresearch::run_branch_update_reimpl_boundary_test },
            RegisteredTest { name: "ZTRESEARCHMGR_UPDATE_BRANCHES", run: tests::ztresearch::run_research_mgr_update_branches_test },
            RegisteredTest { name: "ZTMARKETING_UPDATE", run: tests::ztmarketing::run_marketing_update_test },
            RegisteredTest { name: "ZTMARKETING_UPDATE_BOUNDARY_REPRO", run: tests::ztmarketing::run_marketing_update_boundary_test },
            RegisteredTest { name: "ZTMARKETING_UPDATE_REIMPL_BOUNDARY_REPRO", run: tests::ztmarketing::run_marketing_update_reimpl_boundary_test },
            RegisteredTest { name: "ZTMARKETING_GET_FUNDING_TEXT", run: tests::ztmarketing::run_marketing_funding_text_test },
            RegisteredTest { name: "ZTMARKETINGMGR_LOAD_CONFIGURATIONS", run: tests::ztmarketing::run_marketingmgr_load_configurations_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_LOAD_MODERN", run: tests::ztthoughtmgr::run_thoughtmgr_load_modern_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_ADD_THOUGHT_ANIMAL_OVERRIDE", run: tests::ztthoughtmgr::run_thoughtmgr_add_thought_animal_override_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_POPULATE_THOUGHTS", run: tests::ztthoughtmgr::run_thoughtmgr_populate_thoughts_test },
            RegisteredTest { name: "ZTTHOUGHT_GET_STRING", run: tests::ztthoughtmgr::run_thought_get_string_test },
            RegisteredTest { name: "ZTGAMEMGR_STANDALONE_ROUNDTRIP", run: tests::ztgamemgr::run_gamemgr_standalone_roundtrip_test },
            RegisteredTest { name: "MENUMUSICHANDLER_DETOURS_ENABLED", run: tests::ztgamemgr_menumusichandler::run_menumusichandler_detours_enabled_test },
        ];
        // `*_ORIGINAL_ROUTES_TO_TRAMPOLINE` entries only exist in debug builds - see `generated.rs`'s
        // module doc comment on `.original()`'s per-profile routing.
        #[cfg(debug_assertions)]
        tests.extend([RegisteredTest { name: "MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE", run: tests::ztgamemgr_menumusichandler::run_menumusichandler_original_routes_to_trampoline_test }]);
        tests.push(RegisteredTest { name: "AMBIENTS_DETOURS_ENABLED", run: tests::ambients::run_ambients_detours_enabled_test });
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_DETOURS_ENABLED", run: tests::ztsoundscape::run_ztsoundscape_detours_enabled_test });
        #[cfg(debug_assertions)]
        tests.extend([RegisteredTest { name: "ZTSOUNDSCAPE_ORIGINAL_ROUTES_TO_TRAMPOLINE", run: tests::ztsoundscape::run_ztsoundscape_original_routes_to_trampoline_test }]);
        tests.extend([
            RegisteredTest { name: "MENUMUSICHANDLER_STANDALONE_ROUNDTRIP", run: tests::ztgamemgr_menumusichandler::run_menumusichandler_standalone_roundtrip_test },
            RegisteredTest { name: "AMBIENTS_STANDALONE_ROUNDTRIP", run: tests::ambients::run_ambients_standalone_roundtrip_test },
            RegisteredTest { name: "ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP", run: tests::ztsoundscape::run_ztsoundscape_standalone_roundtrip_test },
            RegisteredTest { name: "ZTSHOWMGR_STANDALONE_ROUNDTRIP", run: tests::ztshowmgr::run_ztshowmgr_standalone_roundtrip_test },
            RegisteredTest { name: "ZTSHOWMGR_INIT_SHOW_PARAMS", run: tests::ztshowmgr::run_ztshowmgr_init_show_params_test },
            RegisteredTest { name: "ZTSHOWMGR_REGISTER_UNREGISTER_SHOW", run: tests::ztshowmgr::run_ztshowmgr_register_unregister_show_test },
            RegisteredTest { name: "ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID", run: tests::ztshowmgr::run_ztshowmgr_get_show_info_get_script_id_test },
            RegisteredTest { name: "ZTSHOWMGR_ENTER_NEW_MONTH", run: tests::ztshowmgr::run_ztshowmgr_enter_new_month_test },
            RegisteredTest { name: "ZTSHOWMGR_UPDATE", run: tests::ztshowmgr::run_ztshowmgr_update_test },
            RegisteredTest { name: "ZTSHOWMGR_SAVE_LOAD", run: tests::ztshowmgr::run_ztshowmgr_save_load_test },
            RegisteredTest { name: "ZTSHOWMGR_IS_DOING_SHOW", run: tests::ztshowmgr::run_ztshowmgr_is_doing_show_test },
            RegisteredTest { name: "ZTSHOWMGR_IS_SHOW_SCRIPT_DONE", run: tests::ztshowmgr::run_ztshowmgr_is_show_script_done_test },
            RegisteredTest { name: "ZTSHOWMGR_REGISTER_UNREGISTER_GET_SCRIPT", run: tests::ztshowmgr::run_ztshowmgr_register_unregister_get_script_test },
            RegisteredTest { name: "ZTSHOW_GET_SHOW_SCRIPT_STATE", run: tests::ztshow::run_ztshow_get_show_script_state_test },
            RegisteredTest { name: "ZTSHOWSTATE_DETOURS_ENABLED", run: tests::ztshowstate::run_ztshowstate_detours_enabled_test },
            RegisteredTest { name: "ZTSHOWSTATE_INIT", run: tests::ztshowstate::run_ztshowstate_init_test },
            RegisteredTest { name: "ZTSHOWSTATE_SAVE_LOAD_ROUNDTRIP", run: tests::ztshowstate::run_ztshowstate_save_load_roundtrip_test },
            RegisteredTest { name: "ZTSHOWINFO_DETOURS_ENABLED", run: tests::ztshowinfo::run_ztshowinfo_detours_enabled_test },
            RegisteredTest { name: "ZTSHOWINFO_STATUS_PREDICATES_LIVE", run: tests::ztshowinfo::run_ztshowinfo_status_predicates_live_test },
            RegisteredTest { name: "ZTSHOWINFO_ACCUMULATORS_LIVE", run: tests::ztshowinfo::run_ztshowinfo_accumulators_live_test },
            RegisteredTest { name: "ZTSHOWINFO_ADD_REMOVE_SHOW_LIVE", run: tests::ztshowinfo::run_ztshowinfo_add_remove_show_live_test },
            RegisteredTest { name: "ZTSHOWINFO_EVENT_SYSTEM_LIVE", run: tests::ztshowinfo::run_ztshowinfo_event_system_live_test },
            RegisteredTest { name: "ZTSHOWINFO_UNIT_ROSTER_READ_LIVE", run: tests::ztshowinfo::run_ztshowinfo_unit_roster_read_live_test },
            RegisteredTest { name: "ZTSHOWINFO_REMOVE_UNIT_LIVE", run: tests::ztshowinfo::run_ztshowinfo_remove_unit_live_test },
            RegisteredTest { name: "ZTSHOWINFO_SET_SHOW_INFO_ID_LIVE", run: tests::ztshowinfo::run_ztshowinfo_set_show_info_id_live_test },
            RegisteredTest { name: "ZTSHOWINFO_SAVE_LOAD_ROUNDTRIP", run: tests::ztshowinfo::run_ztshowinfo_save_load_roundtrip_test },
            RegisteredTest { name: "ZTSHOWINFO_UPDATE_FROM_LOAD_LIVE", run: tests::ztshowinfo::run_ztshowinfo_update_from_load_live_test },
            RegisteredTest { name: "ZTSHOWINFO_STANDALONE_ROUNDTRIP", run: tests::ztshowinfo::run_ztshowinfo_standalone_roundtrip_test },
            RegisteredTest { name: "ZTSOUNDSCAPE_FADE_CONSTANTS", run: tests::ztsoundscape::run_ztsoundscape_fade_constants_test },
            RegisteredTest { name: "MENUMUSICHANDLER_INIT", run: tests::ztgamemgr_menumusichandler::run_menumusichandler_init_test },
            RegisteredTest { name: "MENUMUSICHANDLER_START_PLAY", run: tests::ztgamemgr_menumusichandler::run_menumusichandler_start_play_test },
            RegisteredTest { name: "MENUMUSICHANDLER_START_FADE", run: tests::ztgamemgr_menumusichandler::run_menumusichandler_start_fade_test },
            RegisteredTest { name: "MENUMUSICHANDLER_UPDATE", run: tests::ztgamemgr_menumusichandler::run_menumusichandler_update_test },
            RegisteredTest { name: "ZTGAMEMGR_CONSTRUCT", run: tests::ztgamemgr::run_gamemgr_construct_test },
            RegisteredTest { name: "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS", run: tests::ztgamemgr::run_gamemgr_set_new_game_defaults_test },
            RegisteredTest { name: "ZOOSTATUS_DETOURS_ENABLED", run: tests::zoostatus::run_zoostatus_detours_enabled_test },
        ]);
        #[cfg(debug_assertions)]
        tests.extend([RegisteredTest { name: "ZOOSTATUS_ORIGINAL_ROUTES_TO_TRAMPOLINE", run: tests::zoostatus::run_zoostatus_original_routes_to_trampoline_test }]);
        tests.extend([
            RegisteredTest { name: "ZOOSTATUS_INIT", run: tests::zoostatus::run_zoostatus_init_test },
            RegisteredTest { name: "ZOOSTATUS_ACCUMULATORS", run: tests::zoostatus::run_zoostatus_accumulators_test },
            RegisteredTest { name: "ZOOSTATUS_GET_STATUS", run: tests::zoostatus::run_zoostatus_get_status_test },
            RegisteredTest { name: "ZTGAMEMGR_SAVE_LOAD", run: tests::ztgamemgr::run_gamemgr_save_load_test },
            RegisteredTest { name: "ZTGAMEMGR_UPDATE_SIM", run: tests::ztgamemgr::run_gamemgr_update_sim_test },
            RegisteredTest { name: "ZTGAMEMGR_FINANCE_DATE_HELPERS", run: tests::ztgamemgr::run_gamemgr_finance_date_helpers_test },
            RegisteredTest { name: "ZOOSTATUS_CHECKS", run: tests::zoostatus::run_zoostatus_checks_test },
            RegisteredTest { name: "ZOOSTATUS_F_GRANT_DONATION_NO_OP", run: tests::zoostatus::run_zoostatus_f_grant_donation_no_op_test },
            RegisteredTest { name: "ZOOSTATUS_NEWGUEST_CHECKS_SMOKE", run: tests::zoostatus::run_zoostatus_newguest_checks_smoke_test },
            RegisteredTest { name: "ZOOSTATUS_PRICING", run: tests::zoostatus::run_zoostatus_pricing_test },
            RegisteredTest { name: "ZOOSTATUS_CALCULATE_SUMS", run: tests::zoostatus::run_zoostatus_calculate_sums_test },
            RegisteredTest { name: "ZOOSTATUS_SHOW_PRICES_SMOKE", run: tests::zoostatus::run_zoostatus_show_prices_smoke_test },
            RegisteredTest { name: "ZOOSTATUS_OVERRIDE", run: tests::zoostatus::run_zoostatus_override_test },
            RegisteredTest { name: "ZOOSTATUS_SAVE_LOAD", run: tests::zoostatus::run_zoostatus_save_load_test },
            RegisteredTest { name: "ZOOSTATUS_LOAD_LEGACY", run: tests::zoostatus::run_zoostatus_load_legacy_test },
        ]);
        tests
    }

    /// Needs `run_load_live_zoo` to have succeeded first - run from within
    /// `run_on_completion_reset_test_and_exit`. When it hasn't, every entry here is skipped with an
    /// explicit `Test Passed NAME (skipped: live zoo not loaded)` line instead of silently vanishing -
    /// see the call site in `run_on_completion_reset_test_and_exit`.
    fn live_zoo_tests() -> Vec<RegisteredTest> {
        vec![
            RegisteredTest { name: "ZTHABITATMGR_GET_HABITAT_PTR_LIVE", run: tests::zthabitatmgr::run_habitat_get_habitat_ptr_live_test },
            // Diagnosing a real save-corruption report: round-trips whatever real show-script data
            // run_load_live_zoo just populated (not synthetic data) through encode_mgr/load_mgr directly -
            // run first, before any other live_zoo_tests entry (several add/mutate scripts) can change the
            // as-loaded state being diffed.
            RegisteredTest { name: "ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE", run: tests::ztshowscriptmgr::run_ztshowscriptmgr_real_zoo_roundtrip_live_test },
            RegisteredTest { name: "ZTSHOWMGR_REAL_ZOO_STORE_CONSISTENCY_LIVE", run: tests::ztshowmgr::run_ztshowmgr_real_zoo_store_consistency_live_test },
            RegisteredTest { name: "ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE", run: tests::ztshow::run_ztshow_pending_script_tree_real_zoo_integrity_live_test },
            RegisteredTest { name: "ZTSHOWINFO_REAL_SAVE_LOAD_BYTE_COUNT_LIVE", run: tests::ztshow::run_ztshowinfo_real_save_load_byte_count_live_test },
            RegisteredTest { name: "ZTRESEARCHMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE", run: tests::ztresearch::run_ztresearchmgr_real_zoo_save_roundtrip_live_test },
            // Order-independent: none of these three mutates real vanilla memory (marketing's real
            // singleton round-trips its own reimplemented state; award/thought read real vanilla memory
            // read-only and only ever mutate their own independent Rust-side stores, reset back to empty
            // afterward where relevant).
            RegisteredTest { name: "ZTMARKETINGMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE", run: tests::ztmarketing::run_ztmarketingmgr_real_zoo_save_load_roundtrip_live_test },
            RegisteredTest { name: "ZTAWARDMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE", run: tests::ztawardmgr::run_ztawardmgr_real_zoo_save_load_roundtrip_live_test },
            RegisteredTest { name: "ZTTHOUGHTMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE", run: tests::ztthoughtmgr::run_ztthoughtmgr_real_zoo_save_roundtrip_live_test },
            // Risk-sequenced: update() first (trivial scalar logic), then recalculate_characteristics()
            // (in-place map mutation, no vector resize), then the category-map node-layout live check,
            // then init() last (the only vector-resize path).
            RegisteredTest { name: "ZTMEGATILEMGR_UPDATE", run: tests::ztmegatilemgr::run_megatilemgr_update_test },
            RegisteredTest { name: "ZTMEGATILEMGR_RECALCULATE_CHARACTERISTICS", run: tests::ztmegatilemgr::run_megatilemgr_recalculate_characteristics_test },
            RegisteredTest { name: "ZTMEGATILE_CATEGORY_MAP_LAYOUT", run: tests::ztmegatilemgr::run_megatile_category_map_layout_test },
            RegisteredTest { name: "ZTMEGATILEMGR_INIT", run: tests::ztmegatilemgr::run_megatilemgr_init_test },
            RegisteredTest { name: "ZTADVTERRAINMGR_START", run: tests::ztadvterrainmgr::run_ztadvterrainmgr_start_test },
            RegisteredTest { name: "ZTADVTERRAINMGR_UPDATE", run: tests::ztadvterrainmgr::run_ztadvterrainmgr_update_test },
            RegisteredTest { name: "ZTGUEST_MEGATILE_METHODS_LIVE", run: tests::ztguest::run_ztguest_megatile_methods_live_test },
            // Re-run: the early-phase call in `early_tests` skips gracefully (GLOBAL_ZTGameMgr isn't
            // initialized yet at that injection point) - retry now that run_load_live_zoo has guaranteed a
            // live one.
            RegisteredTest { name: "ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT", run: tests::ztscenariosimplegoal::run_ztscenariosimplegoal_eval_award_count_test },
            RegisteredTest { name: "ZTAWARDMGR_SHOW_AWARDS", run: tests::ztawardmgr::run_awardmgr_show_awards_test },
        // ZTShowScriptMgr/ZTShow live coverage: all three need a real, loaded zoo - Group 1
        // (ADD_SCRIPT/CHECK_PENDING_SCRIPTS) needs a live GLOBAL_ZTGameMgr for GET_DATE, Groups 2/3
        // need real GLOBAL_ZTHabitatMgr/GLOBAL_ZTWorldMgr data.
        //
        // Harness wiring: `openzt-test-dll`'s DllMain calls `openztlib::reimplementation_tests::init()`
        // directly and never runs `lib.rs`'s production boot cascade, so the only detours in this
        // harness are the ones this file's own `init()` installs - no per-module detour (`ztshow`,
        // `ztshowscriptmgr`, `ztawardmgr`, `ztthoughtmgr`, `ztmegatilemgr`, ...) exists unless that
        // includes it. The ZTShowScriptMgr/ZTShow detours are installed unconditionally near the top
        // of this file's `init()` (alongside `research_save_reimplementation`/
        // `marketing_save_reimplementation`), not gated behind `run_load_live_zoo`.
        //
        // The two award tests registered above run through a `retour` trampoline (`call_real`) instead
        // of `.original()`, which can't reach real vanilla once a function is hooked in-process (see
        // either `call_real`'s own doc comment); only `ztawardmgr::eval_award_count_override::init`/
        // `ztawardmgr::show_awards_detour::init` are installed for them (deliberately *not* the whole
        // `ztawardmgr::init`, which would also hook `ADD_AWARD`/`GET_AWARD`/`SAVE`/`LOAD`/`START` and
        // break the three other award tests' use of `.original()` for real-vanilla comparison).
            RegisteredTest { name: "ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE", run: tests::ztshow::run_ztshowinfo_add_script_check_pending_scripts_live_test },
            RegisteredTest { name: "ZTSHOWINFO_PENDING_SCRIPT_TREE_STRESS_LIVE", run: tests::ztshow::run_ztshowinfo_pending_script_tree_stress_live_test },
            RegisteredTest { name: "ZTSHOW_CHECK_OWNING_HABITAT_LIVE", run: tests::ztshow::run_ztshow_check_owning_habitat_live_test },
            RegisteredTest { name: "ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE", run: tests::ztshowinfo::run_ztshowinfo_schedule_frequency_live_test },
            RegisteredTest { name: "ZTSHOWINFO_KEEPER_PREDICATES_LIVE", run: tests::ztshowinfo::run_ztshowinfo_keeper_predicates_live_test },
            RegisteredTest { name: "ZTSHOWINFO_CREATE_DEFAULT_SCRIPT_LIVE", run: tests::ztshowinfo::run_ztshowinfo_create_default_script_live_test },
            RegisteredTest { name: "ZTSHOWINFO_CHECK_UNIT_LIVE", run: tests::ztshowinfo::run_ztshowinfo_check_unit_live_test },
            RegisteredTest { name: "ZTSHOWINFO_CLEAR_PENDING_SCRIPT_TREE_LIVE", run: tests::ztshowinfo::run_ztshowinfo_clear_pending_script_tree_live_test },
            RegisteredTest { name: "ZTSHOWINFO_ADD_UNIT_LIVE", run: tests::ztshowinfo::run_ztshowinfo_add_unit_live_test },
            RegisteredTest { name: "ZTSHOWINFO_GATHER_UNITS_LIVE", run: tests::ztshowinfo::run_ztshowinfo_gather_units_live_test },
            RegisteredTest { name: "ZTSHOWINFO_ENTER_NEW_MONTH_LIVE", run: tests::ztshowinfo::run_ztshowinfo_enter_new_month_live_test },
            RegisteredTest { name: "ZTSHOWINFO_UPDATE_LIVE", run: tests::ztshowinfo::run_ztshowinfo_update_live_test },
            RegisteredTest { name: "ZTSHOW_GROUP3_TRICK_LIVE", run: tests::ztshow::run_ztshow_group3_trick_live_test },
            RegisteredTest { name: "ZTSHOWUI_FILL_TRICK_LISTS_LIVE", run: tests::ztshowui::run_ztshowui_fill_trick_lists_live_test },
            RegisteredTest { name: "ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE", run: tests::ztshowscriptmgr::run_ztshowscript_ctor_registration_live_test },
            // Run last (see this test's own doc comment): a one-shot wiring smoke test for
            // set_new_game_defaults's is_new_game=true branch, which calls through GLOBAL_ZTAIMgr's real
            // vtable slot and so may have real side effects on live AI state.
            RegisteredTest { name: "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS_IS_NEW_GAME_SMOKE", run: tests::ztgamemgr::run_gamemgr_set_new_game_defaults_is_new_game_smoke_test },
            // These three (only ZTGAMEMGR_START_STOP_SMOKE runs after them): need the live, zoo-loaded
            // GLOBAL_ZTScenarioMgr registry for the four config/name getters - pre-zoo the registry is
            // non-null-but-uninitialized and both BFConfigFile::attempt calls would fail, silently
            // leaving the tests covering only init's defaults/tail while looking green.
            // ZTSOUNDSCAPE_UPDATE and ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE additionally need the live
            // GLOBAL_ZTGameMgr guest count. AMBIENTS_LIVE_GROUP_COMPARE and AMBIENTSGROUP_STANDALONE_COMPARE
            // need the same live scenario registry (for a real ambients config to open) and run first
            // since they're independent of ZTSoundscape's own state. AMBIENTSGROUP_STANDALONE_COMPARE
            // doesn't touch `Ambients` at all - it builds `AmbientsGroup` blocks directly against the
            // same config, independently verifying the construction logic AMBIENTS_LIVE_GROUP_COMPARE's
            // own real-vanilla pole can't (see that test's own doc comment for why).
            RegisteredTest { name: "AMBIENTS_LIVE_GROUP_COMPARE", run: tests::ambients::run_ambients_live_group_compare_test },
            RegisteredTest { name: "AMBIENTSGROUP_STANDALONE_COMPARE", run: tests::ambients::run_ambientsgroup_standalone_compare_test },
            RegisteredTest { name: "ZTSOUNDSCAPE_INIT", run: tests::ztsoundscape::run_ztsoundscape_init_test },
            RegisteredTest { name: "ZTSOUNDSCAPE_UPDATE", run: tests::ztsoundscape::run_ztsoundscape_update_test },
            RegisteredTest { name: "ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE", run: tests::ztsoundscape::run_ztsoundscape_update_attempt_failure_test },
            RegisteredTest { name: "ZTSOUNDSCAPE_DESTRUCT", run: tests::ztsoundscape::run_ztsoundscape_destruct_test },
            // Run last (see this test's own doc comment): a one-shot wiring smoke test for start()/stop(),
            // which read the live GLOBAL_ZTScenarioMgr/GLOBAL_ZTApp singletons, run the Rust
            // soundscape ctor/init + vanilla destructor end to end, and call through to real vanilla
            // unpauseGame.
            RegisteredTest { name: "ZTGAMEMGR_START_STOP_SMOKE", run: tests::ztgamemgr::run_gamemgr_start_stop_smoke_test },
            // openzt/plans/ztgamemgr-vanilla-storage-migration-plan.md's Stage 4 live test: also needs
            // start() (see ZTGAMEMGR_START_STOP_SMOKE's own note above), so stays right after it.
            RegisteredTest { name: "ZTGAMEMGR_DESTRUCT", run: tests::ztgamemgr::run_gamemgr_destruct_test },
            // openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md's ZTGameMgr item: mutates the live
            // singleton's cash/date/elapsed_sim_ticks in place (there's no cheap standalone copy of a
            // fully-populated real ZTGameMgr to load into instead) - run genuinely last so nothing above
            // depends on those fields being untouched afterward.
            RegisteredTest { name: "ZTGAMEMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE", run: tests::ztgamemgr::run_ztgamemgr_real_zoo_save_load_roundtrip_live_test },
        ]
    }

    /// `run_load_live_zoo` always logs exactly one `LOAD_LIVE_ZOO` line (success or failure) but isn't
    /// itself a `RegisteredTest` - its bool return value is what gates `live_zoo_tests`, so it's called
    /// directly rather than through `run_registered_tests`. Counted here so it isn't silently dropped
    /// from the expected total.
    const LOAD_LIVE_ZOO_COUNT: usize = 1;

    /// Total number of `Test Passed`/`Test Failed`/skip lines the battery is expected to produce if it
    /// runs to completion: `LOAD_LIVE_ZOO_COUNT` plus every `RegisteredTest` entry across the three
    /// lists - the single source of truth since Stage 6 of
    /// `openzt/plans/reimplementation-tests-refactor-plan.md` folded the last holdout hand-inlined
    /// proptest blocks into ordinary list entries (previously a separate `INLINE_TEST_COUNT` constant
    /// had to be hand-maintained alongside these lists' own lengths).
    fn expected_test_count() -> usize {
        LOAD_LIVE_ZOO_COUNT + early_tests().len() + always_late_tests().len() + live_zoo_tests().len()
    }

    // TODO: Fix this so it works with a crate/mod prefix
    #[detour(LOAD_LANG_DLLS)]
    unsafe extern "thiscall" fn detour_target(this: *const u32) -> u32 {
        info!("Detour success");

        // Read filepath from environment variable with default
        let failure_log_path =
            std::env::var("OPENZT_TEST_LOG").unwrap_or_else(|_| "C:\\Program Files (x86)\\Microsoft Games\\Zoo Tycoon\\openzt_test.log".to_string());

        // Create or truncate the file
        let mut failure_log = match OpenOptions::new().create(true).write(true).truncate(true).open(&failure_log_path) {
            Ok(file) => Some(file),
            Err(e) => {
                error!("Failed to create failure log file '{}': {}", failure_log_path, e);
                None
            }
        };

        // Written before anything else runs, so a battery that crashes or hangs partway through still
        // leaves behind how many result lines to expect - compare that count against how many
        // `Test Passed`/`Test Failed`/skip lines actually made it into the log to see how far it got.
        write_battery_marker(&mut failure_log, &format!("OpenZT reimplementation-test battery started: {} tests expected", expected_test_count()));

        let fail_flag = run_registered_tests(&early_tests(), &mut failure_log);

        // `GLOBAL_ZTWorldMgr` isn't initialized yet at this injection point (this early,
        // `LOAD_LANG_DLLS` hasn't even loaded language DLLs yet) - confirmed live via the Lua
        // console's `get_zt_world_mgr()` - but *is* true by the time `ZTApp::updateSim` starts
        // ticking (entity types load during app init, before the main loop starts). So: stash this
        // battery's `fail_flag` and hand off to the real `LOAD_LANG_DLLS` instead of exiting here,
        // so the game actually continues init through to the main loop; `detour_update_sim` below
        // runs `always_late_tests()`/`live_zoo_tests()` on the first tick and does the final exit
        // for the whole combined battery.
        EARLY_TESTS_FAILED.store(fail_flag, Ordering::SeqCst);
        unsafe { LOAD_LANG_DLLS_DETOUR.call(this) }
    }

    static EARLY_TESTS_FAILED: AtomicBool = AtomicBool::new(false);
    static RAN_UPDATE_SIM_TESTS: Once = Once::new();

    /// The resource-relative path vanilla's own boot-time `ZTMarketingMgr::loadConfigurations` call
    /// passes (e.g. `"mktg.cfg"`) - captured below so `run_marketingmgr_load_configurations_test` can
    /// reuse the real path. Only the first call's path is kept.
    pub(crate) static CAPTURED_MARKETING_PATH: OnceLock<String> = OnceLock::new();

    /// Transparent path-capture detour on `ZTMarketingMgr::loadConfigurations` - always calls through
    /// to the original and never alters its return value or behavior, just records the path into
    /// `CAPTURED_MARKETING_PATH`.
    #[detour(LOAD_CONFIGURATIONS)]
    unsafe extern "thiscall" fn detour_capture_marketing_load_configurations_path(this: *const u32, path: *const i8) -> u32 {
        let path_str = unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned();
        let _ = CAPTURED_MARKETING_PATH.set(path_str);
        unsafe { LOAD_CONFIGURATIONS_DETOUR.call(this, path) }
    }

    #[detour(UPDATE_SIM)]
    unsafe extern "thiscall" fn detour_update_sim(this_ptr: *const u32, param_2: u32) {
        RAN_UPDATE_SIM_TESTS.call_once(run_on_completion_reset_test_and_exit);
        unsafe { UPDATE_SIM_DETOUR.call(this_ptr, param_2) }
    }

    /// Runs on `ZTApp::updateSim`'s first tick (see `detour_update_sim`), once `GLOBAL_ZTWorldMgr` is
    /// actually initialized - `always_late_tests()`'s first entry,
    /// `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`, is what actually needs that (see its own doc comment
    /// in `tests/ztresearch.rs`). Appends to the same log file `detour_target` started, then performs
    /// the final exit for the whole combined battery.
    fn run_on_completion_reset_test_and_exit() {
        let failure_log_path =
            std::env::var("OPENZT_TEST_LOG").unwrap_or_else(|_| "C:\\Program Files (x86)\\Microsoft Games\\Zoo Tycoon\\openzt_test.log".to_string());
        let mut failure_log = match OpenOptions::new().create(true).append(true).open(&failure_log_path) {
            Ok(file) => Some(file),
            Err(e) => {
                error!("Failed to open failure log file '{}': {}", failure_log_path, e);
                None
            }
        };

        let mut fail_flag = EARLY_TESTS_FAILED.load(Ordering::SeqCst);

        fail_flag |= run_registered_tests(&always_late_tests(), &mut failure_log);

        // Loads a real save file directly, so GLOBAL_ZTWorldMgr/GLOBAL_ZTHabitatMgr go from
        // empty/synthetic to real, populated state. Everything in `live_zoo_tests` runs against that
        // real zoo instead of a standalone/synthetic struct.
        let live_zoo = live_zoo_tests();
        if run_load_live_zoo(&mut failure_log) {
            fail_flag |= run_registered_tests(&live_zoo, &mut failure_log);
        } else {
            // Explicit per-test skip line (mirroring every other graceful skip in this file) instead of
            // silently producing no line at all - this is exactly the ambiguity this registry-driven
            // battery was built to remove: previously, a failed live-zoo load meant these 21 tests just
            // never appeared in the log, indistinguishable from a mid-battery crash.
            for test in &live_zoo {
                info!("Skipping {}: live zoo not loaded", test.name);
                write_success_line(&mut failure_log, &format!("{} (skipped: live zoo not loaded)", test.name));
            }
        }

        write_battery_marker(
            &mut failure_log,
            &format!(
                "OpenZT reimplementation-test battery finished: {} tests expected, overall {}",
                expected_test_count(),
                if fail_flag { "FAILED" } else { "PASSED" }
            ),
        );

        if fail_flag {
            error!("Proptest failed for some cases, check the failure log at: {}", failure_log_path);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    /// Default path for `run_load_live_zoo`'s save file - a real save (not embedded/synthetic) placed
    /// in the actual Zoo Tycoon "Saved Games" directory, overridable via `OPENZT_TEST_ZOO` for anyone
    /// whose install lives elsewhere.
    const DEFAULT_TEST_ZOO_PATH: &str = r"C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\Saved Games\reimplementation-test-zoo.zoo";

    /// Loads `OPENZT_TEST_ZOO` (or `DEFAULT_TEST_ZOO_PATH`) into the running game, bringing up a real,
    /// fully-populated `GLOBAL_ZTWorldMgr`/`GLOBAL_ZTHabitatMgr`/etc. - unlike every test above this
    /// point in the battery, which only ever build standalone/synthetic structs.
    ///
    /// Calls `FOPEN`/`ZTUI_GAMEOPTS_LOAD_FILE`/`FCLOSE` directly - the same primitives
    /// `ZTUI::gameopts::loadGame`/`ZTUI::clickContinue` use, minus the file-picker dialog and UI click
    /// handlers, neither of which touches `GLOBAL_ZTWorldMgr`/`GLOBAL_ZTHabitatMgr`.
    ///
    /// Returns `true` only on a real load success (`LOAD_FILE`'s low byte non-zero).
    fn run_load_live_zoo(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "LOAD_LIVE_ZOO";
        let path = std::env::var("OPENZT_TEST_ZOO").unwrap_or_else(|_| DEFAULT_TEST_ZOO_PATH.to_string());

        let path_cstring = match CString::new(path.clone()) {
            Ok(c) => c,
            Err(e) => {
                error!("{}: path {:?} contains a NUL byte: {}", test_name, path, e);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: path {:?} contains a NUL byte: {}\n", test_name, path, e).as_bytes());
                }
                return false;
            }
        };
        let mode_cstring = c"rb";

        let file_ptr = unsafe { standalone::FOPEN.original()(path_cstring.as_ptr() as u32, mode_cstring.as_ptr()) };
        if file_ptr.is_null() {
            error!("{}: fopen failed for {:?} (file missing? see OPENZT_TEST_ZOO)", test_name, path);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: fopen failed for {:?}\n", test_name, path).as_bytes());
            }
            return false;
        }

        let load_result = unsafe { ZTUI_GAMEOPTS_LOAD_FILE.original()(file_ptr as *const u8) };
        unsafe { standalone::FCLOSE.original()(file_ptr) };

        let success = (load_result & 0xff) != 0;
        if success {
            info!("{}: loaded {:?}", test_name, path);
            write_success_line(failure_log, test_name);
        } else {
            error!("{}: LOAD_FILE reported failure (raw result {:#010x}) for {:?}", test_name, load_result, path);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: LOAD_FILE reported failure (raw result {:#010x}) for {:?}\n", test_name, load_result, path).as_bytes());
            }
        }
        success
    }

}
