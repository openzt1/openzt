#![allow(dead_code)]

use std::{any::Any, fmt};

use proptest::test_runner::{FailurePersistence, PersistedSeed};
use tracing::{error, info};

#[cfg(target_os = "windows")]
use crate::detour_mod;

/// Redirects the `fwrite`/`fread`-shaped primitives `ZTResearchMgr::save`/`load` (and every other
/// vanilla `*::save`/`*::load`) go through, to in-memory buffers - lets `detour_zoo_main`'s live
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

        unsafe { detour_zoo_main::init_detours() }.is_err().then(|| {
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

#[cfg(target_os = "windows")]
#[detour_mod]
mod detour_zoo_main {
    use std::{
        backtrace::Backtrace,
        cell::Cell,
        ffi::{c_void, CStr, CString},
        fs::OpenOptions,
        io::Write,
        mem::size_of,
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
    use openzt_detour::generated::bfentity::GET_FOOTPRINT as BFENTITY_GET_FOOTPRINT;
    use openzt_detour::generated::bftile::GET_LOCAL_ELEVATION;
    use openzt_detour::generated::standalone;
    use openzt_detour::generated::ztanimal::GET_FOOTPRINT as ZTANIMAL_GET_FOOTPRINT;
    use openzt_detour::generated::ztapp::UPDATE_SIM;
    use openzt_detour::generated::ztmarketing;
    use openzt_detour::generated::ztmarketingmgr::{
        CLEAR_CONFIGURATIONS as ZTMARKETINGMGR_CLEAR_CONFIGURATIONS, LOAD_CONFIGURATIONS, UPDATE as ZTMARKETINGMGR_UPDATE, ZTMARKETING_MGR_1 as ZTMARKETINGMGR_DTOR,
    };
    use openzt_detour::generated::ztresearchbranch;
    use openzt_detour::generated::ztshowinfo;
    use openzt_detour::generated::ztresearchbranch::GET_FUNDING_TEXT as ZTRESEARCHBRANCH_GET_FUNDING_TEXT;
    use openzt_detour::generated::ztresearchbranch::UPDATE as ZTRESEARCHBRANCH_UPDATE;
    use openzt_detour::generated::ztresearchmgr;
    use openzt_detour::generated::ztresearchmgr::FORCE_RESEARCH as ZTRESEARCHMGR_FORCE_RESEARCH;
    use openzt_detour::generated::ztresearchmgr::UPDATE as ZTRESEARCHMGR_UPDATE;
    use openzt_detour::generated::ztresearchprogram;
    use openzt_detour::generated::ztthought as gen_ztthought;
    use openzt_detour::generated::ztthoughtmgr as gen_ztthoughtmgr;
    use openzt_detour::generated::ztmegatilemgr as gen_ztmegatilemgr;
    use openzt_detour::generated::ztguest as gen_ztguest;
    use openzt_detour::generated::ztawardmgr as gen_ztawardmgr;
    use openzt_detour::generated::uilistbox as gen_uilistbox;
    use openzt_detour::generated::bfuimgr::GET_ELEMENT_0 as BFUIMGR_GET_ELEMENT_0;
    use openzt_detour::generated::zthabitatmgr;
    use openzt_detour::generated::ztui_gameopts::LOAD_FILE as ZTUI_GAMEOPTS_LOAD_FILE;
    use openzt_detour::generated::ztunit::GET_FOOTPRINT as ZTUNIT_GET_FOOTPRINT;
    use openzt_detour::generated::ztshow::GET_SHOW_SCRIPT_STATE;
    use openzt_detour::generated::ztshowscriptstate::CONSTRUCTOR as CREATE_SHOW_SCRIPT_STATE;
    use openzt_detour::generated::ztshowscript::CONSTRUCTOR as ZTSHOWSCRIPT_CONSTRUCTOR;
    use openzt_detour::generated::ztshowinfo::GET_NUM_UNITS as ZTSHOWINFO_GET_NUM_UNITS;
    use openzt_detour::generated::bfapp::GET_INSTALLED_EXPANSION as BFAPP_GET_INSTALLED_EXPANSION;
    use openzt_detour::generated::ztshowmgr::{
        CONSTRUCTOR as ZTSHOWMGR_CONSTRUCTOR, ENTER_NEW_MONTH as ZTSHOWMGR_ENTER_NEW_MONTH, GET_SCRIPT as ZTSHOWMGR_GET_SCRIPT,
        GET_SCRIPT_ID as ZTSHOWMGR_GET_SCRIPT_ID, GET_SHOW_INFO as ZTSHOWMGR_GET_SHOW_INFO, IS_DOING_SHOW as ZTSHOWMGR_IS_DOING_SHOW,
        IS_SHOW_SCRIPT_DONE as ZTSHOWMGR_IS_SHOW_SCRIPT_DONE, LOAD as ZTSHOWMGR_LOAD, REGISTER_SCRIPT as ZTSHOWMGR_REGISTER_SCRIPT,
        REGISTER_SHOW as ZTSHOWMGR_REGISTER_SHOW, SAVE as ZTSHOWMGR_SAVE, UNREGISTER_SCRIPT as ZTSHOWMGR_UNREGISTER_SCRIPT,
        UNREGISTER_SHOW as ZTSHOWMGR_UNREGISTER_SHOW, UPDATE as ZTSHOWMGR_UPDATE,
    };
    use openzt_detour::generated::bfconfigfile::{CONSTRUCTOR_0 as BFCONFIGFILE_CONSTRUCTOR_0, RELEASE as BFCONFIGFILE_RELEASE};
    use openzt_detour::generated::bfworldmgr::GET_TYPE as BFWORLDMGR_GET_TYPE;
    use openzt_detour::generated::bfscenariomgr::{
        GET_CROWD_AMBIENTS_NAME, GET_CROWD_CONFIG_NAME, GET_WORLD_AMBIENTS_NAME, GET_WORLD_CONFIG_NAME,
    };
    use openzt_detour::generated::ztgamemgr::{
        ADD_CASH as ZTGAMEMGR_ADD_CASH, ANIMAL_TIME_AGO as ZTGAMEMGR_ANIMAL_TIME_AGO, GET_DATE as ZTGAMEMGR_GET_DATE,
        HOURS_AGO as ZTGAMEMGR_HOURS_AGO, IS_GAME_DATE as ZTGAMEMGR_IS_GAME_DATE, IS_REAL_WORLD_DATE as ZTGAMEMGR_IS_REAL_WORLD_DATE,
        LOAD as ZTGAMEMGR_LOAD, OVERRIDE_NEW_GAME_DEFAULTS as ZTGAMEMGR_OVERRIDE_NEW_GAME_DEFAULTS, PEOPLE_TIME_AGO as ZTGAMEMGR_PEOPLE_TIME_AGO,
        SAVE as ZTGAMEMGR_SAVE, SET_NEW_GAME_DEFAULTS as ZTGAMEMGR_SET_NEW_GAME_DEFAULTS, SUBTRACT_CASH as ZTGAMEMGR_SUBTRACT_CASH,
        TIME_AGO as ZTGAMEMGR_TIME_AGO, UPDATE as ZTGAMEMGR_UPDATE, UPDATE_SIM as ZTGAMEMGR_UPDATE_SIM,
    };
    use openzt_detour::generated::zoostatus::{
        BUY_ANIMAL as ZOOSTATUS_BUY_ANIMAL, BUY_PEOPLE_FOOD as ZOOSTATUS_BUY_PEOPLE_FOOD, CALCULATE_SUMS as ZOOSTATUS_CALCULATE_SUMS,
        CHANGE_ENDOWMENT_MEMBERS as ZOOSTATUS_CHANGE_ENDOWMENT_MEMBERS, F_GRANT_DONATION as ZOOSTATUS_F_GRANT_DONATION,
        HEAL_ANIMAL as ZOOSTATUS_HEAL_ANIMAL,
        INCREASE_ADMISSIONS as ZOOSTATUS_INCREASE_ADMISSIONS, INCREASE_ADMISSIONS_INCOME as ZOOSTATUS_INCREASE_ADMISSIONS_INCOME,
        INCREASE_DONATIONS as ZOOSTATUS_INCREASE_DONATIONS, INCREASE_ENDOWMENT as ZOOSTATUS_INCREASE_ENDOWMENT,
        INCREASE_SHOW_ADMISSION as ZOOSTATUS_INCREASE_SHOW_ADMISSION, INIT as ZOOSTATUS_INIT, LOAD as ZOOSTATUS_LOAD,
        MESSAGE_CHECKS as ZOOSTATUS_MESSAGE_CHECKS, NEWGUEST_CHECKS as ZOOSTATUS_NEWGUEST_CHECKS, OVERRIDE as ZOOSTATUS_OVERRIDE,
        PURCHASE_FOOD as ZOOSTATUS_PURCHASE_FOOD,
        RATING_CHECKS as ZOOSTATUS_RATING_CHECKS, SAVE as ZOOSTATUS_SAVE,
        REFUND_ANIMAL_COST as ZOOSTATUS_REFUND_ANIMAL_COST, REFUND_CONSTRUCTION as ZOOSTATUS_REFUND_CONSTRUCTION,
        SET_ADULT_ADMISSION_PRICE as ZOOSTATUS_SET_ADULT_ADMISSION_PRICE,
        SPEND_BUILDING_UPKEEP as ZOOSTATUS_SPEND_BUILDING_UPKEEP, SPEND_CONSTRUCTION as ZOOSTATUS_SPEND_CONSTRUCTION,
        SPEND_GUIDE_WAGES as ZOOSTATUS_SPEND_GUIDE_WAGES,
        SPEND_KEEPER_WAGES as ZOOSTATUS_SPEND_KEEPER_WAGES_1, SPEND_MAINT_WAGES as ZOOSTATUS_SPEND_MAINT_WAGES,
        SPEND_MARKETING as ZOOSTATUS_SPEND_MARKETING, SPEND_RESEARCH as ZOOSTATUS_SPEND_RESEARCH,
    };
    use openzt_detour::FunctionDef;
    use proptest::prelude::*;
    use tracing::{error, info};
    use windows::Win32::Foundation::FILETIME;

    use crate::{
        bfentitytype::{BFEntityType, ZTAnimalType, ZTUnitType},
        globals::{get_module_base, globals},
        util::{get_from_memory, save_to_memory, ZTBufferString, ZTString},
        zthabitatmgr::ZTHabitat,
        ztmapview::BFTile,
        ztmarketing::{live_support as marketing_live_support, marketing_save_reimplementation, predict_mgr_update, ZTMarketing, ZTMarketingMgr},
        ztresearch::research_save_reimplementation::{self, live_support, SaveRecord},
        ztresearch::{predict_branch_progress, predict_update, ZTResearchBranch, ZTResearchEffectKind, ZTResearchMgr},
        ztthoughtmgr::{live_support as thought_live_support, ZTThought, ZTThoughtMgr},
        ztmegatilemgr::live_support as megatile_live_support,
        ztgamemgr::{self, live_support as gamemgr_live_support},
        ztgamemgr_menumusichandler::{self, live_support as menumusichandler_live_support},
        ztsoundscape::{live_support as soundscape_live_support, ZTSoundscape},
        ztguest::{self, live_support as guest_live_support},
        ztawardmgr::{self, live_support as award_live_support},
        ztworldmgr::{BFEntity, IVec3, ZTAnimal, ZTUnit},
        ztshow::{self, live_support as ztshow_live_support},
        ztshowscriptmgr,
        ztshowmgr::{self, live_support as showmgr_live_support, ZTShowMgr},
        ztshowui,
        zoostatus::{live_support as zoostatus_live_support, ZooStatus, GET_STATUS_FIXED},
    };

    use super::io_redirect;

    /// One generated program: `saved_progress_bits`, when `Some`, becomes a `Program` record in the
    /// stream fed to `load`; the initial `current_progress`/`target_cost` only matter for the `save`
    /// test (`load` always resets `current_progress` to `0` first regardless of these). `effect_kind_raw`
    /// spans `-1..=8` (unset through one past the last valid kind) but is only consumed by
    /// `ZTRESEARCHMGR_LOAD`'s own tree-building - `generated_branches` below (shared with the `save`/
    /// `force_research` tests) deliberately ignores it and always pins `-1`, since only `load`'s tail
    /// (`on_completion`, for any program whose loaded `current_progress` ends up `>= target_cost`)
    /// dispatches on this field.
    #[derive(Debug, Clone)]
    struct ProgramCase {
        id: i32,
        target_cost: f32,
        initial_progress: f32,
        saved_progress_bits: Option<u32>,
        effect_kind_raw: i32,
    }

    #[derive(Debug, Clone)]
    struct CategoryCase {
        id: i32,
        initial_enabled: u8,
        saved_enabled: Option<u8>,
        programs: Vec<ProgramCase>,
    }

    /// `saved_funding_level`, when generated, deliberately spans negative/in-range/out-of-range
    /// relative to `funding_level_count`, to exercise `ZTResearchMgr::load`'s clamp rule.
    #[derive(Debug, Clone)]
    struct BranchCase {
        id: i32,
        initial_funding_level: i32,
        funding_level_count: usize,
        saved_funding_level: Option<i32>,
        categories: Vec<CategoryCase>,
    }

    fn program_case_strategy() -> impl Strategy<Value = ProgramCase> {
        (any::<i32>(), any::<f32>(), any::<f32>(), prop::option::of(any::<u32>()), -1i32..=8i32).prop_map(
            |(id, target_cost, initial_progress, saved_progress_bits, effect_kind_raw)| ProgramCase {
                id,
                target_cost,
                initial_progress,
                saved_progress_bits,
                effect_kind_raw,
            },
        )
    }

    fn category_case_strategy() -> impl Strategy<Value = CategoryCase> {
        (any::<i32>(), any::<u8>(), prop::option::of(any::<u8>()), prop::collection::vec(program_case_strategy(), 0..3)).prop_map(
            |(id, initial_enabled, saved_enabled, programs)| CategoryCase { id, initial_enabled, saved_enabled, programs },
        )
    }

    fn branch_case_strategy() -> impl Strategy<Value = BranchCase> {
        (
            any::<i32>(),
            any::<i32>(),
            0usize..4,
            prop::option::of(prop_oneof![Just(-1i32), 0i32..8i32]),
            prop::collection::vec(category_case_strategy(), 0..3),
        )
            .prop_map(|(id, initial_funding_level, funding_level_count, saved_funding_level, categories)| BranchCase {
                id,
                initial_funding_level,
                funding_level_count,
                saved_funding_level,
                categories,
            })
    }

    /// Converts generated cases into the synthetic tree `live_support::with_synthetic_branches` splices
    /// into `ZTResearchMgr::branch_array`, using each case's *initial* field values. Used by the `save`/
    /// `force_research` tests, neither of which dispatch on `effect_kind_raw` - always pinned to `-1`
    /// here regardless of what `ProgramCase::effect_kind_raw` was generated as, keeping this helper's
    /// output unchanged from before that field existed. See `generated_branches_for_load` for the one
    /// call site that does vary it.
    fn generated_branches(cases: &[BranchCase]) -> Vec<live_support::GeneratedBranch> {
        cases
            .iter()
            .map(|branch| live_support::GeneratedBranch {
                id: branch.id,
                current_funding_level: branch.initial_funding_level,
                funding_level_count: branch.funding_level_count,
                categories: branch
                    .categories
                    .iter()
                    .map(|category| live_support::GeneratedCategory {
                        id: category.id,
                        enabled: category.initial_enabled,
                        programs: category
                            .programs
                            .iter()
                            .map(|program| live_support::GeneratedProgram {
                                id: program.id,
                                target_cost: program.target_cost,
                                current_progress: program.initial_progress,
                                effect_kind_raw: -1,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Same as `generated_branches`, but threads each program's generated `effect_kind_raw` through
    /// instead of pinning `-1` - used only by `ZTRESEARCHMGR_LOAD`, whose tail (`on_completion`, for any
    /// program whose loaded `current_progress` ends up `>= target_cost`) is the one place in this file's
    /// live battery that actually dispatches on this field.
    fn generated_branches_for_load(cases: &[BranchCase]) -> Vec<live_support::GeneratedBranch> {
        cases
            .iter()
            .map(|branch| live_support::GeneratedBranch {
                id: branch.id,
                current_funding_level: branch.initial_funding_level,
                funding_level_count: branch.funding_level_count,
                categories: branch
                    .categories
                    .iter()
                    .map(|category| live_support::GeneratedCategory {
                        id: category.id,
                        enabled: category.initial_enabled,
                        programs: category
                            .programs
                            .iter()
                            .map(|program| live_support::GeneratedProgram {
                                id: program.id,
                                target_cost: program.target_cost,
                                current_progress: program.initial_progress,
                                effect_kind_raw: program.effect_kind_raw,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// One program-per-category case for the `ZTRESEARCHMGR_FORCE_RESEARCH` comparison. Unlike
    /// `BranchCase`/`CategoryCase` above (which generate `0..3` programs per category to exercise
    /// `save`/`load`), `ZTResearchMgr::forceResearch` walks *every* program in *every* category (see
    /// `ZTResearchMgr::force_research`'s doc comment) and then calls `pick_random_program` once per
    /// branch - fixing exactly one program per category keeps that RNG-driven call a safe, deterministic
    /// no-crash operation without needing to also compare which program it ends up selecting.
    #[derive(Debug, Clone)]
    struct ForceResearchProgramCase {
        id: i32,
        target_cost: f32,
        initial_progress: f32,
    }

    #[derive(Debug, Clone)]
    struct ForceResearchCategoryCase {
        id: i32,
        program: ForceResearchProgramCase,
    }

    #[derive(Debug, Clone)]
    struct ForceResearchBranchCase {
        id: i32,
        categories: Vec<ForceResearchCategoryCase>,
    }

    fn force_research_program_case_strategy() -> impl Strategy<Value = ForceResearchProgramCase> {
        (any::<i32>(), any::<f32>(), any::<f32>())
            .prop_map(|(id, target_cost, initial_progress)| ForceResearchProgramCase { id, target_cost, initial_progress })
    }

    fn force_research_category_case_strategy() -> impl Strategy<Value = ForceResearchCategoryCase> {
        (any::<i32>(), force_research_program_case_strategy()).prop_map(|(id, program)| ForceResearchCategoryCase { id, program })
    }

    fn force_research_branch_case_strategy() -> impl Strategy<Value = ForceResearchBranchCase> {
        (any::<i32>(), prop::collection::vec(force_research_category_case_strategy(), 0..3))
            .prop_map(|(id, categories)| ForceResearchBranchCase { id, categories })
    }

    /// Every generated category is `enabled` with exactly one program, and every branch gets an empty
    /// funding table - `force_research`/`forceResearch` never read funding levels, only
    /// `pick_random_program` does (for `current_funding_rate`, unrelated to program selection).
    fn force_research_generated_branches(cases: &[ForceResearchBranchCase]) -> Vec<live_support::GeneratedBranch> {
        cases
            .iter()
            .map(|branch| live_support::GeneratedBranch {
                id: branch.id,
                current_funding_level: 0,
                funding_level_count: 0,
                categories: branch
                    .categories
                    .iter()
                    .map(|category| live_support::GeneratedCategory {
                        id: category.id,
                        enabled: 1,
                        programs: vec![live_support::GeneratedProgram {
                            id: category.program.id,
                            target_cost: category.program.target_cost,
                            current_progress: category.program.initial_progress,
                            effect_kind_raw: -1,
                        }],
                    })
                    .collect(),
            })
            .collect()
    }

    /// One generated program for `ZTRESEARCHMGR_LOOKUPS`, id-only - `get_branch`/`get_category`/
    /// `get_program` only ever compare against `id`.
    #[derive(Debug, Clone)]
    struct LookupProgramCase {
        id: i32,
    }

    #[derive(Debug, Clone)]
    struct LookupCategoryCase {
        id: i32,
        programs: Vec<LookupProgramCase>,
    }

    #[derive(Debug, Clone)]
    struct LookupBranchCase {
        id: i32,
        categories: Vec<LookupCategoryCase>,
    }

    /// Small, overlapping id range (`0..8`), unlike the wide `any::<i32>()` ids `BranchCase`/etc. use
    /// elsewhere in this file (which essentially never collide by chance) - so generated
    /// `ZTRESEARCHMGR_LOOKUPS` trees actually produce duplicate/colliding ids across branches/
    /// categories/programs at a reasonable rate, exercising `get_branch`/`get_category`/`get_program`'s
    /// "first match in traversal order" semantics.
    fn lookup_program_case_strategy() -> impl Strategy<Value = LookupProgramCase> {
        (0i32..8i32).prop_map(|id| LookupProgramCase { id })
    }

    fn lookup_category_case_strategy() -> impl Strategy<Value = LookupCategoryCase> {
        (0i32..8i32, prop::collection::vec(lookup_program_case_strategy(), 0..3)).prop_map(|(id, programs)| LookupCategoryCase { id, programs })
    }

    fn lookup_branch_case_strategy() -> impl Strategy<Value = LookupBranchCase> {
        (0i32..8i32, prop::collection::vec(lookup_category_case_strategy(), 0..3)).prop_map(|(id, categories)| LookupBranchCase { id, categories })
    }

    /// Converts generated lookup cases into the synthetic tree for `ZTRESEARCHMGR_LOOKUPS` - read-only,
    /// so unlike `generated_branches`/`force_research_generated_branches` nothing here needs to be
    /// realistic beyond the id fields under test; funding/enabled/cost/progress/effect fields are all
    /// fixed to inert values.
    fn lookup_generated_branches(cases: &[LookupBranchCase]) -> Vec<live_support::GeneratedBranch> {
        cases
            .iter()
            .map(|branch| live_support::GeneratedBranch {
                id: branch.id,
                current_funding_level: 0,
                funding_level_count: 0,
                categories: branch
                    .categories
                    .iter()
                    .map(|category| live_support::GeneratedCategory {
                        id: category.id,
                        enabled: 1,
                        programs: category
                            .programs
                            .iter()
                            .map(|program| live_support::GeneratedProgram {
                                id: program.id,
                                target_cost: 0.0,
                                current_progress: 0.0,
                                effect_kind_raw: -1,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// One generated program for `ZTRESEARCHMGR_SET_EFFECT_DISCOUNT`: `effect_kind_raw` spans `-1..=8`
    /// (unset through one past the last valid kind) to cover both matching and non-matching programs
    /// within the same tree; `target_cost` is bounded well away from `f32`'s extremes since the
    /// discount math (`(100 - discount_pct) as f32 * target_cost * 0.01`) is compared bit-for-bit and
    /// non-finite/extreme inputs aren't a meaningful case to compare (same reasoning as
    /// `funding_level_case_strategy`'s `cost` bound).
    #[derive(Debug, Clone)]
    struct EffectDiscountProgramCase {
        id: i32,
        target_cost: f32,
        effect_kind_raw: i32,
    }

    #[derive(Debug, Clone)]
    struct EffectDiscountCategoryCase {
        id: i32,
        programs: Vec<EffectDiscountProgramCase>,
    }

    #[derive(Debug, Clone)]
    struct EffectDiscountBranchCase {
        id: i32,
        categories: Vec<EffectDiscountCategoryCase>,
    }

    fn effect_discount_program_case_strategy() -> impl Strategy<Value = EffectDiscountProgramCase> {
        (any::<i32>(), -1_000_000f32..1_000_000f32, -1i32..=8i32)
            .prop_map(|(id, target_cost, effect_kind_raw)| EffectDiscountProgramCase { id, target_cost, effect_kind_raw })
    }

    fn effect_discount_category_case_strategy() -> impl Strategy<Value = EffectDiscountCategoryCase> {
        (any::<i32>(), prop::collection::vec(effect_discount_program_case_strategy(), 0..3))
            .prop_map(|(id, programs)| EffectDiscountCategoryCase { id, programs })
    }

    fn effect_discount_branch_case_strategy() -> impl Strategy<Value = EffectDiscountBranchCase> {
        (any::<i32>(), prop::collection::vec(effect_discount_category_case_strategy(), 0..3))
            .prop_map(|(id, categories)| EffectDiscountBranchCase { id, categories })
    }

    /// Converts generated cases into a synthetic tree for `ZTRESEARCHMGR_SET_EFFECT_DISCOUNT` - called
    /// twice per test case (see the test itself) to build two independently-constructed but
    /// structurally identical trees, since `set_effect_discount` mutates `target_cost` in place.
    fn effect_discount_generated_branches(cases: &[EffectDiscountBranchCase]) -> Vec<live_support::GeneratedBranch> {
        cases
            .iter()
            .map(|branch| live_support::GeneratedBranch {
                id: branch.id,
                current_funding_level: 0,
                funding_level_count: 0,
                categories: branch
                    .categories
                    .iter()
                    .map(|category| live_support::GeneratedCategory {
                        id: category.id,
                        enabled: 1,
                        programs: category
                            .programs
                            .iter()
                            .map(|program| live_support::GeneratedProgram {
                                id: program.id,
                                target_cost: program.target_cost,
                                current_progress: 0.0,
                                effect_kind_raw: program.effect_kind_raw,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// `(records, branch_ids, category_ids, program_ids, funding_level_counts)` - see
    /// `loaded_records_and_ids`.
    type LoadedRecordsAndIds = (Vec<SaveRecord>, Vec<i32>, Vec<i32>, Vec<i32>, std::collections::HashMap<i32, usize>);

    /// The `(kind, id, value)` records a save file would contain for `cases` (using each case's
    /// *saved_* fields, skipping ids with no override), plus every id/funding-level-count in the tree -
    /// exactly what `research_save_reimplementation::predict_load` needs to predict `load`'s outcome.
    fn loaded_records_and_ids(cases: &[BranchCase]) -> LoadedRecordsAndIds {
        let mut records = Vec::new();
        let mut branch_ids = Vec::new();
        let mut category_ids = Vec::new();
        let mut program_ids = Vec::new();
        let mut funding_level_counts = std::collections::HashMap::new();

        for branch in cases {
            branch_ids.push(branch.id);
            funding_level_counts.insert(branch.id, branch.funding_level_count);
            if let Some(current_funding_level) = branch.saved_funding_level {
                records.push(SaveRecord::Branch { id: branch.id, current_funding_level });
            }
            for category in &branch.categories {
                category_ids.push(category.id);
                if let Some(enabled) = category.saved_enabled {
                    records.push(SaveRecord::Category { id: category.id, enabled });
                }
                for program in &category.programs {
                    program_ids.push(program.id);
                    if let Some(current_progress_bits) = program.saved_progress_bits {
                        records.push(SaveRecord::Program { id: program.id, current_progress_bits });
                    }
                }
            }
        }

        (records, branch_ids, category_ids, program_ids, funding_level_counts)
    }

    /// A single named entry in the reimplementation-comparison battery. `early_tests`/
    /// `always_late_tests`/`live_zoo_tests` build the three ordered lists below, which
    /// `detour_target`/`run_on_completion_reset_test_and_exit` walk instead of the old flat sequence
    /// of `fail_flag |= run_..._test(&mut failure_log);` lines - this is the single source of truth
    /// both functions use to compute the battery's total expected test count for the start/finish
    /// markers (see `write_battery_marker`), so the two stay in sync automatically as tests are added,
    /// removed, or reordered.
    struct RegisteredTest {
        /// Matches the `test_name` the function itself logs under - reused to write an explicit skip
        /// line for a `live_zoo_tests` entry when `run_load_live_zoo` fails, so a gap between the
        /// battery's start/finish markers is always explained by a line in the log, not silence.
        name: &'static str,
        run: fn(&mut Option<std::fs::File>) -> bool,
    }

    /// Runs unconditionally in `detour_target`, before the inline `ZTRESEARCHMGR_*`/
    /// `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET` proptest blocks (kept inline rather than folded into
    /// this registry, since - unlike every entry here - they share a single `proptest::TestRunner`).
    fn early_tests() -> Vec<RegisteredTest> {
        let mut tests = Vec::new();
        tests.push(RegisteredTest { name: "BFENTITY_GET_FOOTPRINT", run: run_bfentity_get_footprint_tests });
        tests.push(RegisteredTest { name: "ZTUNIT_GET_FOOTPRINT", run: run_ztunit_get_footprint_tests });
        tests.push(RegisteredTest { name: "ZTANIMAL_GET_FOOTPRINT", run: run_ztanimal_get_footprint_tests });
        tests.push(RegisteredTest { name: "ZTRESEARCHBRANCH_FUNDING", run: run_research_branch_funding_test });
        tests.push(RegisteredTest { name: "ZTRESEARCHBRANCH_PCT_DAYS_REMAINING", run: run_research_branch_pct_days_remaining_test });
        tests.push(RegisteredTest { name: "ZTMARKETING_INCREASE_FUNDING", run: run_marketing_increase_funding_test });
        tests.push(RegisteredTest { name: "ZTMARKETING_DECREASE_FUNDING", run: run_marketing_decrease_funding_test });
        tests.push(RegisteredTest { name: "ZTMARKETING_SET_FUNDING_LEVEL", run: run_marketing_set_funding_level_test });
        tests.push(RegisteredTest { name: "ZTMARKETINGMGR_UPDATE", run: run_marketingmgr_update_test });
        tests.push(RegisteredTest { name: "ZTMARKETINGMGR_SAVE", run: run_marketingmgr_save_test });
        tests.push(RegisteredTest { name: "ZTMARKETINGMGR_LOAD", run: run_marketingmgr_load_test });
        tests.push(RegisteredTest { name: "ZTMARKETINGMGR_CLEAR_CONFIGURATIONS", run: run_marketingmgr_clear_configurations_test });
        tests.push(RegisteredTest { name: "ZTMARKETINGMGR_DTOR", run: run_marketingmgr_dtor_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_ADD_THOUGHT", run: run_thoughtmgr_add_thought_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_THINKER", run: run_thoughtmgr_remove_thoughts_by_thinker_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_OBJECT", run: run_thoughtmgr_remove_thoughts_by_object_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_HABITAT", run: run_thoughtmgr_remove_thoughts_by_habitat_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_GET_THOUGHTS_BY_THINKER", run: run_thoughtmgr_get_thoughts_by_thinker_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_GET_THOUGHTS_BY_OBJECT", run: run_thoughtmgr_get_thoughts_by_object_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_GET_THOUGHTS_BY_HABITAT", run: run_thoughtmgr_get_thoughts_by_habitat_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_SAVE", run: run_thoughtmgr_save_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_LOAD", run: run_thoughtmgr_load_test });
        tests.push(RegisteredTest { name: "ZTAWARDMGR_ADD_AWARD_SAVE_LOAD", run: run_awardmgr_add_award_save_load_test });
        tests.push(RegisteredTest { name: "ZTAWARDMGR_START", run: run_awardmgr_start_test });
        tests.push(RegisteredTest { name: "ZTAWARDMGR_GET_AWARD", run: run_awardmgr_get_award_test });
        tests.push(RegisteredTest { name: "ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT", run: run_ztscenariosimplegoal_eval_award_count_test });
        // ZTShowScriptMgr save/load wire format - independent of any live zoo/manager state, so these
        // run from the early battery alongside the award tests above.
        tests.push(RegisteredTest { name: "ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE", run: run_ztshowscriptmgr_save_load_roundtrip_live_test });
        tests.push(RegisteredTest { name: "ZTSHOWSCRIPTMGR_LOAD_VERSION_GATES_LIVE", run: run_ztshowscriptmgr_load_version_gates_live_test });
        tests
    }

    /// Runs unconditionally in `run_on_completion_reset_test_and_exit`, before the live-zoo gate. The
    /// two `*_ORIGINAL_ROUTES_TO_TRAMPOLINE` entries only exist in debug builds - see `generated.rs`'s
    /// module doc comment on `.original()`'s per-profile routing.
    fn always_late_tests() -> Vec<RegisteredTest> {
        let mut tests = Vec::new();
        tests.push(RegisteredTest { name: "ZTRESEARCHBRANCH_FUNDING_TEXT", run: run_funding_text_test });
        tests.push(RegisteredTest { name: "ZTRESEARCHBRANCH_UPDATE", run: run_branch_update_test });
        tests.push(RegisteredTest { name: "ZTRESEARCHBRANCH_UPDATE_REIMPL_BOUNDARY_REPRO", run: run_branch_update_reimpl_boundary_test });
        tests.push(RegisteredTest { name: "ZTRESEARCHMGR_UPDATE_BRANCHES", run: run_research_mgr_update_branches_test });
        tests.push(RegisteredTest { name: "ZTMARKETING_UPDATE", run: run_marketing_update_test });
        tests.push(RegisteredTest { name: "ZTMARKETING_UPDATE_BOUNDARY_REPRO", run: run_marketing_update_boundary_test });
        tests.push(RegisteredTest { name: "ZTMARKETING_UPDATE_REIMPL_BOUNDARY_REPRO", run: run_marketing_update_reimpl_boundary_test });
        tests.push(RegisteredTest { name: "ZTMARKETING_GET_FUNDING_TEXT", run: run_marketing_funding_text_test });
        tests.push(RegisteredTest { name: "ZTMARKETINGMGR_LOAD_CONFIGURATIONS", run: run_marketingmgr_load_configurations_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_LOAD_MODERN", run: run_thoughtmgr_load_modern_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_ADD_THOUGHT_ANIMAL_OVERRIDE", run: run_thoughtmgr_add_thought_animal_override_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_POPULATE_THOUGHTS", run: run_thoughtmgr_populate_thoughts_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHT_GET_STRING", run: run_thought_get_string_test });
        tests.push(RegisteredTest { name: "ZTGAMEMGR_STANDALONE_ROUNDTRIP", run: run_gamemgr_standalone_roundtrip_test });
        tests.push(RegisteredTest { name: "MENUMUSICHANDLER_DETOURS_ENABLED", run: run_menumusichandler_detours_enabled_test });
        #[cfg(debug_assertions)]
        tests.push(RegisteredTest { name: "MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE", run: run_menumusichandler_original_routes_to_trampoline_test });
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_DETOURS_ENABLED", run: run_ztsoundscape_detours_enabled_test });
        #[cfg(debug_assertions)]
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_ORIGINAL_ROUTES_TO_TRAMPOLINE", run: run_ztsoundscape_original_routes_to_trampoline_test });
        tests.push(RegisteredTest { name: "MENUMUSICHANDLER_STANDALONE_ROUNDTRIP", run: run_menumusichandler_standalone_roundtrip_test });
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP", run: run_ztsoundscape_standalone_roundtrip_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_STANDALONE_ROUNDTRIP", run: run_ztshowmgr_standalone_roundtrip_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_INIT_SHOW_PARAMS", run: run_ztshowmgr_init_show_params_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_REGISTER_UNREGISTER_SHOW", run: run_ztshowmgr_register_unregister_show_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID", run: run_ztshowmgr_get_show_info_get_script_id_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_ENTER_NEW_MONTH", run: run_ztshowmgr_enter_new_month_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_UPDATE", run: run_ztshowmgr_update_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_SAVE_LOAD", run: run_ztshowmgr_save_load_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_IS_DOING_SHOW", run: run_ztshowmgr_is_doing_show_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_IS_SHOW_SCRIPT_DONE", run: run_ztshowmgr_is_show_script_done_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_REGISTER_UNREGISTER_GET_SCRIPT", run: run_ztshowmgr_register_unregister_get_script_test });
        tests.push(RegisteredTest { name: "ZTSHOW_GET_SHOW_SCRIPT_STATE", run: run_ztshow_get_show_script_state_test });
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_FADE_CONSTANTS", run: run_ztsoundscape_fade_constants_test });
        tests.push(RegisteredTest { name: "MENUMUSICHANDLER_INIT", run: run_menumusichandler_init_test });
        tests.push(RegisteredTest { name: "MENUMUSICHANDLER_START_PLAY", run: run_menumusichandler_start_play_test });
        tests.push(RegisteredTest { name: "MENUMUSICHANDLER_START_FADE", run: run_menumusichandler_start_fade_test });
        tests.push(RegisteredTest { name: "MENUMUSICHANDLER_UPDATE", run: run_menumusichandler_update_test });
        tests.push(RegisteredTest { name: "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS", run: run_gamemgr_set_new_game_defaults_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_DETOURS_ENABLED", run: run_zoostatus_detours_enabled_test });
        #[cfg(debug_assertions)]
        tests.push(RegisteredTest { name: "ZOOSTATUS_ORIGINAL_ROUTES_TO_TRAMPOLINE", run: run_zoostatus_original_routes_to_trampoline_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_INIT", run: run_zoostatus_init_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_ACCUMULATORS", run: run_zoostatus_accumulators_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_GET_STATUS", run: run_zoostatus_get_status_test });
        tests.push(RegisteredTest { name: "ZTGAMEMGR_SAVE_LOAD", run: run_gamemgr_save_load_test });
        tests.push(RegisteredTest { name: "ZTGAMEMGR_UPDATE_SIM", run: run_gamemgr_update_sim_test });
        tests.push(RegisteredTest { name: "ZTGAMEMGR_FINANCE_DATE_HELPERS", run: run_gamemgr_finance_date_helpers_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_CHECKS", run: run_zoostatus_checks_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_F_GRANT_DONATION_NO_OP", run: run_zoostatus_f_grant_donation_no_op_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_NEWGUEST_CHECKS_SMOKE", run: run_zoostatus_newguest_checks_smoke_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_PRICING", run: run_zoostatus_pricing_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_CALCULATE_SUMS", run: run_zoostatus_calculate_sums_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_SHOW_PRICES_SMOKE", run: run_zoostatus_show_prices_smoke_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_OVERRIDE", run: run_zoostatus_override_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_SAVE_LOAD", run: run_zoostatus_save_load_test });
        tests.push(RegisteredTest { name: "ZOOSTATUS_LOAD_LEGACY", run: run_zoostatus_load_legacy_test });
        tests
    }

    /// Needs `run_load_live_zoo` to have succeeded first - run from within
    /// `run_on_completion_reset_test_and_exit`. When it hasn't, every entry here is skipped with an
    /// explicit `Test Passed NAME (skipped: live zoo not loaded)` line instead of silently vanishing -
    /// see the call site in `run_on_completion_reset_test_and_exit`.
    fn live_zoo_tests() -> Vec<RegisteredTest> {
        let mut tests = Vec::new();
        tests.push(RegisteredTest { name: "ZTHABITATMGR_GET_HABITAT_PTR_LIVE", run: run_habitat_get_habitat_ptr_live_test });
        // Diagnosing a real save-corruption report: round-trips whatever real show-script data
        // run_load_live_zoo just populated (not synthetic data) through encode_mgr/load_mgr directly -
        // run first, before any other live_zoo_tests entry (several add/mutate scripts) can change the
        // as-loaded state being diffed.
        tests.push(RegisteredTest { name: "ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE", run: run_ztshowscriptmgr_real_zoo_roundtrip_live_test });
        tests.push(RegisteredTest { name: "ZTSHOWMGR_REAL_ZOO_STORE_CONSISTENCY_LIVE", run: run_ztshowmgr_real_zoo_store_consistency_live_test });
        tests.push(RegisteredTest { name: "ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE", run: run_ztshow_pending_script_tree_real_zoo_integrity_live_test });
        tests.push(RegisteredTest { name: "ZTSHOWINFO_REAL_SAVE_LOAD_BYTE_COUNT_LIVE", run: run_ztshowinfo_real_save_load_byte_count_live_test });
        tests.push(RegisteredTest { name: "ZTRESEARCHMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE", run: run_ztresearchmgr_real_zoo_save_roundtrip_live_test });
        // openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md's three order-independent items: none
        // of these three mutate real vanilla memory (marketing's real singleton round-trips its own
        // reimplemented state; award/thought read real vanilla memory read-only and only ever mutate
        // their own independent Rust-side stores, reset back to empty afterward where relevant).
        tests.push(RegisteredTest { name: "ZTMARKETINGMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE", run: run_ztmarketingmgr_real_zoo_save_load_roundtrip_live_test });
        tests.push(RegisteredTest { name: "ZTAWARDMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE", run: run_ztawardmgr_real_zoo_save_load_roundtrip_live_test });
        tests.push(RegisteredTest { name: "ZTTHOUGHTMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE", run: run_ztthoughtmgr_real_zoo_save_roundtrip_live_test });
        // Risk-sequenced per ztmegatilemgr.rs's module doc comment: update() first (trivial scalar
        // logic), then recalculate_characteristics() (in-place map mutation, no vector resize), then
        // the category-map node-layout live check, then init() last (the only vector-resize path).
        tests.push(RegisteredTest { name: "ZTMEGATILEMGR_UPDATE", run: run_megatilemgr_update_test });
        tests.push(RegisteredTest { name: "ZTMEGATILEMGR_RECALCULATE_CHARACTERISTICS", run: run_megatilemgr_recalculate_characteristics_test });
        tests.push(RegisteredTest { name: "ZTMEGATILE_CATEGORY_MAP_LAYOUT", run: run_megatile_category_map_layout_test });
        tests.push(RegisteredTest { name: "ZTMEGATILEMGR_INIT", run: run_megatilemgr_init_test });
        tests.push(RegisteredTest { name: "ZTADVTERRAINMGR_START", run: run_ztadvterrainmgr_start_test });
        tests.push(RegisteredTest { name: "ZTADVTERRAINMGR_UPDATE", run: run_ztadvterrainmgr_update_test });
        tests.push(RegisteredTest { name: "ZTGUEST_MEGATILE_METHODS_LIVE", run: run_ztguest_megatile_methods_live_test });
        // Re-run: the early-phase call in `early_tests` skips gracefully (GLOBAL_ZTGameMgr isn't
        // initialized yet at that injection point) - retry now that run_load_live_zoo has guaranteed a
        // live one.
        tests.push(RegisteredTest { name: "ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT", run: run_ztscenariosimplegoal_eval_award_count_test });
        tests.push(RegisteredTest { name: "ZTAWARDMGR_SHOW_AWARDS", run: run_awardmgr_show_awards_test });
        // ZTShowScriptMgr reimplementation plan, open item 11 (Stage 2 live coverage): all three need
        // a real, loaded zoo - Group 1 (ADD_SCRIPT/CHECK_PENDING_SCRIPTS) needs a live GLOBAL_ZTGameMgr
        // for GET_DATE, Groups 2/3 need real GLOBAL_ZTHabitatMgr/GLOBAL_ZTWorldMgr data.
        //
        // The ZTShowScriptMgr/ZTShow detours are now installed unconditionally near the top of this
        // file's `init()` (alongside `research_save_reimplementation`/`marketing_save_reimplementation`)
        // rather than only here, after `run_load_live_zoo` - see open item 1's diagnostics there for
        // why that used to be necessary and what fixed it. **Real, session-defining finding** (from the
        // session that first installed these two detours here, after the zoo already loaded): this
        // crate's production entry point (`openztlib::init()`, reached via `zoo_init::init_detours()`)
        // is never called by `openzt-test-dll`'s own `DllMain` (`openzt-test-dll/src/lib.rs` calls
        // `openztlib::reimplementation_tests::init()` directly instead) - meaning *no* per-module
        // detour (`ztshow`, `ztshowscriptmgr`, `ztawardmgr`, `ztthoughtmgr`, `ztmegatilemgr`, ...) was
        // ever installed in this test harness except the handful explicitly installed in this file's
        // own `init()`. Every "call the real, now-hooked address directly" live test in this file
        // predating that finding (`ZTAWARDMGR_SHOW_AWARDS`, `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT`)
        // was therefore silently calling real, un-hooked vanilla code all along and only ever verifying
        // "vanilla doesn't crash" - not exercising the Rust reimplementation at all, despite each one's
        // own doc comment describing it as testing the hooked path. Confirmed live via reliable,
        // non-tracing (`std::sync::atomic`/direct-file-write) diagnostics, after `tracing`-based
        // `error!`/`info!` diagnostics turned out to be lossy under this battery's `std::process::exit()`
        // end-of-run (queued-but-unflushed log lines vanish silently - `error!` calls placed early in a
        // test function routinely never made it to `openzt.log`, while calls placed right at the end
        // reliably did).
        //
        // Since fixed for these two specifically: `ztawardmgr::eval_award_count_override::init`/
        // `ztawardmgr::show_awards_detour::init` are now installed in this file's own `init()` too
        // (deliberately *not* the whole `ztawardmgr::init`, which would also hook `ADD_AWARD`/
        // `GET_AWARD`/`SAVE`/`LOAD`/`START` and break the three other award tests' use of
        // `.original()` for real-vanilla comparison), and both tests now compare against real vanilla
        // via a `retour` trampoline (`call_real`) instead of `.original()`, which can't reach real
        // vanilla once a function is hooked in-process (see either `call_real`'s own doc comment).
        tests.push(RegisteredTest { name: "ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE", run: run_ztshowinfo_add_script_check_pending_scripts_live_test });
        tests.push(RegisteredTest { name: "ZTSHOWINFO_PENDING_SCRIPT_TREE_STRESS_LIVE", run: run_ztshowinfo_pending_script_tree_stress_live_test });
        tests.push(RegisteredTest { name: "ZTSHOW_CHECK_OWNING_HABITAT_LIVE", run: run_ztshow_check_owning_habitat_live_test });
        tests.push(RegisteredTest { name: "ZTSHOW_GROUP3_TRICK_LIVE", run: run_ztshow_group3_trick_live_test });
        tests.push(RegisteredTest { name: "ZTSHOWUI_FILL_TRICK_LISTS_LIVE", run: run_ztshowui_fill_trick_lists_live_test });
        tests.push(RegisteredTest { name: "ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE", run: run_ztshowscript_ctor_registration_live_test });
        // Run last (see this test's own doc comment): a one-shot wiring smoke test for
        // set_new_game_defaults's is_new_game=true branch, which calls through GLOBAL_ZTAIMgr's real
        // vtable slot and so may have real side effects on live AI state.
        tests.push(RegisteredTest { name: "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS_IS_NEW_GAME_SMOKE", run: run_gamemgr_set_new_game_defaults_is_new_game_smoke_test });
        // These three (only ZTGAMEMGR_START_STOP_SMOKE runs after them): need the live, zoo-loaded
        // GLOBAL_ZTScenarioMgr registry for the four config/name getters - pre-zoo the registry is
        // non-null-but-uninitialized and both BFConfigFile::attempt calls would fail, silently
        // leaving the tests covering only init's defaults/tail while looking green.
        // ZTSOUNDSCAPE_UPDATE and ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE additionally need the live
        // GLOBAL_ZTGameMgr guest count.
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_INIT", run: run_ztsoundscape_init_test });
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_UPDATE", run: run_ztsoundscape_update_test });
        tests.push(RegisteredTest { name: "ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE", run: run_ztsoundscape_update_attempt_failure_test });
        // Run last (see this test's own doc comment): a one-shot wiring smoke test for start()/stop(),
        // which read the live GLOBAL_ZTScenarioMgr/GLOBAL_ZTApp singletons, run the Rust
        // soundscape ctor/init + vanilla destructor end to end, and call through to real vanilla
        // unpauseGame.
        tests.push(RegisteredTest { name: "ZTGAMEMGR_START_STOP_SMOKE", run: run_gamemgr_start_stop_smoke_test });
        // openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md's ZTGameMgr item: mutates the live
        // singleton's cash/date/elapsed_sim_ticks in place (there's no cheap standalone copy of a
        // fully-populated real ZTGameMgr to load into instead) - run genuinely last so nothing above
        // depends on those fields being untouched afterward.
        tests.push(RegisteredTest { name: "ZTGAMEMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE", run: run_ztgamemgr_real_zoo_save_load_roundtrip_live_test });
        tests
    }

    /// Total number of `Test Passed`/`Test Failed`/skip lines the battery is expected to produce if it
    /// runs to completion: the 9 always-inline proptest blocks (`BFTILE_GET_LOCAL_ELEVATION`, the six
    /// `ZTRESEARCHMGR_*`/`ZTRESEARCHMGR_SAVE`/`LOAD`/`LOAD_CORRUPT_STREAM`/`UPDATE`/`FORCE_RESEARCH`/
    /// `LOOKUPS`/`SET_EFFECT_DISCOUNT` blocks in `detour_target`, and `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`
    /// in `run_on_completion_reset_test_and_exit`) plus every `RegisteredTest` entry. Bump this constant
    /// if an inline block is ever added or removed - everything else here stays in sync automatically.
    const INLINE_TEST_COUNT: usize = 9;

    /// `run_load_live_zoo` always logs exactly one `LOAD_LIVE_ZOO` line (success or failure) but isn't
    /// itself a `RegisteredTest` - its bool return value is what gates `live_zoo_tests`, so it's called
    /// directly rather than through `run_registered_tests`. Counted here so it isn't silently dropped
    /// from the expected total.
    const LOAD_LIVE_ZOO_COUNT: usize = 1;

    fn expected_test_count() -> usize {
        INLINE_TEST_COUNT + LOAD_LIVE_ZOO_COUNT + early_tests().len() + always_late_tests().len() + live_zoo_tests().len()
    }

    /// Writes a `===`-delimited marker line to the battery's log file - used both for the
    /// "N tests expected" line `detour_target` writes when it (re)creates the log, and the
    /// "battery finished" line `run_on_completion_reset_test_and_exit` writes at the very end. If the
    /// process crashes or hangs mid-battery, the log simply ends without a finish marker, and comparing
    /// the number of `Test Passed`/`Test Failed` lines actually present against the expected count named
    /// in the start marker shows exactly how far it got.
    fn write_battery_marker(failure_log: &mut Option<std::fs::File>, message: &str) {
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("=== {} ===\n", message).as_bytes());
        }
    }

    /// Runs every entry in `tests` in order, logging an `info!` trace before each call so a crash mid-test
    /// names the last test attempted in `openzt.log`, even though the per-test pass/fail/skip line only
    /// lands in `failure_log` on that test's own return.
    fn run_registered_tests(tests: &[RegisteredTest], failure_log: &mut Option<std::fs::File>) -> bool {
        let mut fail_flag = false;
        for test in tests {
            info!("Running {}", test.name);
            fail_flag |= (test.run)(failure_log);
        }
        fail_flag
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

        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "BFTILE_GET_LOCAL_ELEVATION";
        // Create a enum strategy so this is all tested in one test and test that each enum variant is tested
        let unknown_byte_values = vec![
            0x1, 0x4, 0x5, 0x10, 0x11, 0x14, 0x15, 0x19, 0x40, 0x41, 0x44, 0x45, 0x46, 0x50, 0x51, 0x54, 0x64, 0x91,
        ];
        let mut fail_flag = false;
        for unknown_byte_2 in unknown_byte_values {
            match runner.run(&(0..1000i32, 0..1000i32), |(x, y)| {
                let pos = IVec3::new(x, y, 0);
                let tile = BFTile::new(pos, unknown_byte_2);
                let reimplemented_result = tile.get_local_elevation(pos);

                let result = GET_LOCAL_ELEVATION.original()(&raw const tile as *const u32, &raw const pos as *const u32);
                assert_eq!(
                    result,
                    reimplemented_result,
                    "Failed for pos: {:?}, tile: {:?}, unknown_byte_2: {}, real: {}, reimplemented: {}",
                    pos,
                    tile,
                    unknown_byte_2,
                    result,
                    reimplemented_result
                );
                Ok(())
            }) {
                Ok(_) => {
                    info!("Proptest passed for unknown_byte_2: {}", unknown_byte_2);
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let proptest::test_runner::TestError::Fail(r, (x, y)) = e {
                        let pos = IVec3::new(x, y, 0);
                        let tile = BFTile::new(pos, unknown_byte_2);
                        let reimplemented_result = tile.get_local_elevation(pos);
                        let result = GET_LOCAL_ELEVATION.original()(&raw const tile as *const u32, &raw const pos as *const u32);
                        let failure_line = format!("unknown_byte_2: {}, x: {}, y: {}, real: {}, reimplemented: {}\n", unknown_byte_2, x, y, result, reimplemented_result);

                        if let Some(ref mut log_file) = failure_log {
                            if let Err(write_err) = log_file.write_all(failure_line.as_bytes()) {
                                error!("Failed to write to failure log: {}", write_err);
                            }
                        }

                        info!("Failed case ({}): x: {}, y: {}", r, x, y);
                        fail_flag = true;
                    }
                }
            }
        }

        if fail_flag {
            error!("Proptest failed for some cases, check the failure log at: {}", failure_log_path);
        } else {
            let success_line = format!("Test Passed {}\n", test_name);

            if let Some(ref mut log_file) = failure_log {
                if let Err(write_err) = log_file.write_all(success_line.as_bytes()) {
                    error!("Failed to write to failure log: {}", write_err);
                }
            }
        }

        fail_flag |= run_registered_tests(&early_tests(), &mut failure_log);

        // ZTRESEARCHMGR_SAVE: compares the real ZTResearchMgr::save's captured output against
        // research_save_reimplementation::serialize(&snapshot_mgr(mgr)) for generated synthetic trees.
        {
            let test_name = "ZTRESEARCHMGR_SAVE";
            match runner.run(&prop::collection::vec(branch_case_strategy(), 0..4), |cases| {
                let branches = generated_branches(&cases);

                let dummy_file: u32 = 0;
                let (expected_records, captured_bytes) = live_support::with_standalone_mgr(&branches, |mgr| {
                    let expected_records = research_save_reimplementation::snapshot_mgr(mgr);
                    io_redirect::begin_capture();
                    let _ = mgr.save(&dummy_file as *const u32);
                    let captured_bytes = io_redirect::end_capture();
                    (expected_records, captured_bytes)
                });

                let expected_bytes = research_save_reimplementation::serialize(&expected_records);
                prop_assert_eq!(captured_bytes, expected_bytes, "ZTResearchMgr::save byte mismatch for cases: {:?}", cases);
                Ok(())
            }) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Passed {}\n", test_name).as_bytes());
                    }
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

        // ZTRESEARCHMGR_LOAD: compares the real ZTResearchMgr::load's effect on funding
        // levels/enabled flags/current progress against research_save_reimplementation::predict_load,
        // for generated synthetic trees, generated save-stream records, and versions spanning both
        // sides of the 0x28 threshold that gates whether the stream is read at all.
        {
            let test_name = "ZTRESEARCHMGR_LOAD";
            match runner.run(&(prop::collection::vec(branch_case_strategy(), 0..4), 0u32..0x40), |(cases, version)| {
                let branches = generated_branches_for_load(&cases);
                let (records, branch_ids, category_ids, program_ids, funding_level_counts) = loaded_records_and_ids(&cases);
                let predicted = research_save_reimplementation::predict_load(&branch_ids, &category_ids, &program_ids, &funding_level_counts, &records, version);
                let bytes = research_save_reimplementation::serialize(&records);

                // `load` dereferences the file pointer directly to check a CRT-`FILE`-shaped EOF flag
                // at offset 0xc, unlike `save` - a zeroed 16-byte buffer keeps that flag clear so the
                // redirected `deallocate` calls actually run.
                let file_buffer = [0u32; 4];
                let actual_records = live_support::with_standalone_mgr(&branches, |mgr| {
                    live_support::with_global_ztresearchmgr_ptr(mgr, |mgr| {
                        io_redirect::begin_replay(bytes);
                        let _ = mgr.load(file_buffer.as_ptr(), version);
                        io_redirect::end_replay();
                        research_save_reimplementation::snapshot_mgr(mgr)
                    })
                });

                let mut actual_funding = std::collections::HashMap::new();
                let mut actual_enabled = std::collections::HashMap::new();
                let mut actual_progress = std::collections::HashMap::new();
                for record in actual_records {
                    match record {
                        SaveRecord::Branch { id, current_funding_level } => {
                            actual_funding.insert(id, current_funding_level);
                        }
                        SaveRecord::Category { id, enabled } => {
                            actual_enabled.insert(id, enabled);
                        }
                        SaveRecord::Program { id, current_progress_bits } => {
                            actual_progress.insert(id, current_progress_bits);
                        }
                    }
                }

                prop_assert_eq!(actual_funding, predicted.funding_levels, "funding level mismatch for version {} cases: {:?}", version, cases);
                prop_assert_eq!(actual_enabled, predicted.enabled, "enabled mismatch for version {} cases: {:?}", version, cases);
                prop_assert_eq!(actual_progress, predicted.progress_bits, "progress mismatch for version {} cases: {:?}", version, cases);
                Ok(())
            }) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Passed {}\n", test_name).as_bytes());
                    }
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

        // ZTRESEARCHMGR_LOAD_CORRUPT_STREAM: exercises `load`'s corrupt-stream abort path - a
        // malformed `kind` tag (anything other than `0`/`1`/`2`/the `-1` terminator) partway through
        // the stream, which `detours::load`'s reading loop treats the same as a genuine I/O read
        // failure: `load` returns `false` without running the `on_completion`/`pick_random_program`
        // tail, but every record read *before* the corruption point has already been applied (per
        // `ZTResearchMgr_load.c`/`.asm`'s "any read failure aborts the whole load" semantics - not a
        // rollback of already-applied records). `SaveRecord`/`serialize` can only ever produce
        // well-formed `kind` tags, so this path has no coverage anywhere else in this file. `version`
        // is fixed `>= 0x28` (the threshold gating whether the stream is read at all - below it,
        // there's nothing to corrupt); `raw_truncate_at` is reduced modulo `records.len() + 1` to land
        // in `0..=records.len()`, choosing how many well-formed records precede the injected
        // corruption. The expected state is `predict_load` fed only the *prefix* of records (before the
        // corruption point) - the unconditional reset always applies, and every record before the
        // corruption point was already parsed and applied before the abort, so this needs no new
        // prediction logic.
        {
            let test_name = "ZTRESEARCHMGR_LOAD_CORRUPT_STREAM";
            match runner.run(
                &(prop::collection::vec(branch_case_strategy(), 0..4), 0x28u32..0x40u32, any::<usize>()),
                |(cases, version, raw_truncate_at)| {
                    let branches = generated_branches_for_load(&cases);
                    let (records, branch_ids, category_ids, program_ids, funding_level_counts) = loaded_records_and_ids(&cases);
                    let truncate_at = raw_truncate_at % (records.len() + 1);

                    // header + well-formed records[..truncate_at] + terminator, then drop that
                    // terminator and replace it with a malformed `kind` tag (`3`, not `0`/`1`/`2`/`-1`)
                    // plus a dummy id, so the reading loop actually reaches and executes its
                    // `kind > 2` check rather than just running out of bytes mid-read.
                    let mut bytes = research_save_reimplementation::serialize(&records[..truncate_at]);
                    bytes.truncate(bytes.len() - 4);
                    bytes.extend_from_slice(&3i32.to_le_bytes());
                    bytes.extend_from_slice(&0i32.to_le_bytes());

                    let predicted = research_save_reimplementation::predict_load(
                        &branch_ids,
                        &category_ids,
                        &program_ids,
                        &funding_level_counts,
                        &records[..truncate_at],
                        version,
                    );

                    let file_buffer = [0u32; 4];
                    let (load_result, actual_records) = live_support::with_standalone_mgr(&branches, |mgr| {
                        live_support::with_global_ztresearchmgr_ptr(mgr, |mgr| {
                            io_redirect::begin_replay(bytes);
                            let load_result = mgr.load(file_buffer.as_ptr(), version);
                            io_redirect::end_replay();
                            (load_result, research_save_reimplementation::snapshot_mgr(mgr))
                        })
                    });

                    prop_assert!(!load_result, "load() should return false on a corrupt stream, cases: {:?}, truncate_at: {}", cases, truncate_at);

                    let mut actual_funding = std::collections::HashMap::new();
                    let mut actual_enabled = std::collections::HashMap::new();
                    let mut actual_progress = std::collections::HashMap::new();
                    for record in actual_records {
                        match record {
                            SaveRecord::Branch { id, current_funding_level } => {
                                actual_funding.insert(id, current_funding_level);
                            }
                            SaveRecord::Category { id, enabled } => {
                                actual_enabled.insert(id, enabled);
                            }
                            SaveRecord::Program { id, current_progress_bits } => {
                                actual_progress.insert(id, current_progress_bits);
                            }
                        }
                    }

                    prop_assert_eq!(
                        actual_funding,
                        predicted.funding_levels,
                        "funding level mismatch for version {} truncate_at {} cases: {:?}",
                        version,
                        truncate_at,
                        cases
                    );
                    prop_assert_eq!(
                        actual_enabled,
                        predicted.enabled,
                        "enabled mismatch for version {} truncate_at {} cases: {:?}",
                        version,
                        truncate_at,
                        cases
                    );
                    prop_assert_eq!(
                        actual_progress,
                        predicted.progress_bits,
                        "progress mismatch for version {} truncate_at {} cases: {:?}",
                        version,
                        truncate_at,
                        cases
                    );
                    Ok(())
                },
            ) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Passed {}\n", test_name).as_bytes());
                    }
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

        // ZTRESEARCHMGR_UPDATE: compares the real ZTResearchMgr::update's effect on `elapsed_ticks`
        // against the reimplemented `ZTResearchMgr::update`, for a synthetic manager with zero
        // branches - so `ZTResearchBranch::update` (still a call into the original implementation)
        // never actually runs, keeping this comparison independent of branch-level state.
        {
            let test_name = "ZTRESEARCHMGR_UPDATE";
            match runner.run(&(any::<u32>(), any::<u32>()), |(elapsed_ticks_before, delta_ticks)| {
                let real_elapsed_ticks = live_support::with_standalone_mgr(&[], |mgr| {
                    mgr.set_elapsed_ticks(elapsed_ticks_before);
                    unsafe { ZTRESEARCHMGR_UPDATE.original()((mgr as *mut ZTResearchMgr) as *const u32, delta_ticks) };
                    mgr.elapsed_ticks()
                });
                let reimplemented_elapsed_ticks = live_support::with_standalone_mgr(&[], |mgr| {
                    mgr.set_elapsed_ticks(elapsed_ticks_before);
                    mgr.update(delta_ticks);
                    mgr.elapsed_ticks()
                });

                prop_assert_eq!(
                    real_elapsed_ticks,
                    reimplemented_elapsed_ticks,
                    "elapsed_ticks mismatch for elapsed_ticks_before={}, delta_ticks={}",
                    elapsed_ticks_before,
                    delta_ticks
                );
                Ok(())
            }) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Passed {}\n", test_name).as_bytes());
                    }
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

        // ZTRESEARCHMGR_FORCE_RESEARCH: compares the real ZTResearchMgr::forceResearch's effect on
        // every program's `current_progress` against the reimplemented `ZTResearchMgr::force_research`,
        // for generated synthetic trees with exactly one program per category (see
        // `force_research_generated_branches`'s doc comment) and both `continue_program` values. Every
        // generated program's `effect_kind_raw` is fixed to `-1` (unset) by `live_support::GeneratedProgram`,
        // making `on_completion` a guaranteed no-op that needs no `GLOBAL_ZTWorldMgr`, so - like SAVE/LOAD/
        // UPDATE above - this can run at this early injection point rather than waiting for `updateSim`.
        {
            let test_name = "ZTRESEARCHMGR_FORCE_RESEARCH";
            match runner.run(&(prop::collection::vec(force_research_branch_case_strategy(), 0..3), proptest::bool::ANY), |(cases, continue_program)| {
                let real_progress_bits = {
                    let branches = force_research_generated_branches(&cases);
                    live_support::with_standalone_mgr(&branches, |mgr| {
                        live_support::with_global_ztresearchmgr_ptr(mgr, |mgr| {
                            unsafe { ZTRESEARCHMGR_FORCE_RESEARCH.original()((mgr as *mut ZTResearchMgr) as *const u32, continue_program) };
                            mgr.branches().flat_map(|b| b.categories()).flat_map(|c| c.programs()).map(|p| p.current_progress().to_bits()).collect::<Vec<_>>()
                        })
                    })
                };

                let reimplemented_progress_bits = {
                    let branches = force_research_generated_branches(&cases);
                    live_support::with_standalone_mgr(&branches, |mgr| {
                        live_support::with_global_ztresearchmgr_ptr(mgr, |mgr| {
                            mgr.force_research(continue_program);
                            mgr.branches().flat_map(|b| b.categories()).flat_map(|c| c.programs()).map(|p| p.current_progress().to_bits()).collect::<Vec<_>>()
                        })
                    })
                };

                prop_assert_eq!(
                    real_progress_bits,
                    reimplemented_progress_bits,
                    "current_progress mismatch for continue_program={} cases: {:?}",
                    continue_program,
                    cases
                );
                Ok(())
            }) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Passed {}\n", test_name).as_bytes());
                    }
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

        // ZTRESEARCHMGR_LOOKUPS: compares the real ZTResearchMgr::getBranch/getCategory/getProgram
        // against the reimplemented get_branch/get_category/get_program, on one shared generated tree
        // (read-only, so - unlike SET_EFFECT_DISCOUNT below - both sides query the very same tree
        // instance, no need for two independently-built ones), with small overlapping ids (see
        // `lookup_branch_case_strategy`'s doc comment) to exercise duplicate/colliding-id traversal
        // order, plus a lookup id that may or may not be present in the tree.
        {
            let test_name = "ZTRESEARCHMGR_LOOKUPS";
            match runner.run(&(prop::collection::vec(lookup_branch_case_strategy(), 0..4), any::<i32>()), |(cases, lookup_id)| {
                let branches = lookup_generated_branches(&cases);
                let (real_branch, real_category, real_program, reimpl_branch, reimpl_category, reimpl_program) =
                    live_support::with_standalone_mgr(&branches, |mgr| {
                        let mgr_ptr = (mgr as *mut ZTResearchMgr) as *const u32;
                        let real_branch = unsafe { ztresearchmgr::GET_BRANCH.original()(mgr_ptr, lookup_id) } as u32;
                        let real_category = unsafe { ztresearchmgr::GET_CATEGORY.original()(mgr_ptr, lookup_id) } as u32;
                        let real_program = unsafe { ztresearchmgr::GET_PROGRAM.original()(mgr_ptr, lookup_id) } as u32;

                        let reimpl_branch = mgr.get_branch(lookup_id).map_or(0u32, |b| b as *const _ as u32);
                        let reimpl_category = mgr.get_category(lookup_id).map_or(0u32, |c| c as *const _ as u32);
                        let reimpl_program = mgr.get_program(lookup_id).map_or(0u32, |p| p as *const _ as u32);

                        (real_branch, real_category, real_program, reimpl_branch, reimpl_category, reimpl_program)
                    });

                prop_assert_eq!(real_branch, reimpl_branch, "get_branch mismatch for lookup_id={} cases: {:?}", lookup_id, cases);
                prop_assert_eq!(real_category, reimpl_category, "get_category mismatch for lookup_id={} cases: {:?}", lookup_id, cases);
                prop_assert_eq!(real_program, reimpl_program, "get_program mismatch for lookup_id={} cases: {:?}", lookup_id, cases);
                Ok(())
            }) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Passed {}\n", test_name).as_bytes());
                    }
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

        // ZTRESEARCHMGR_SET_EFFECT_DISCOUNT: compares the real ZTResearchMgr::setEffectDiscount's
        // effect on every program's `target_cost` against the reimplemented `set_effect_discount`, for
        // two independently-constructed but structurally identical trees
        // (`effect_discount_generated_branches` is called once per side) - mutating, unlike LOOKUPS
        // above, so each side needs its own tree rather than sharing one. `kind` spans every valid
        // `ZTResearchEffectKind`; each generated program's own `effect_kind_raw` spans `-1..=8` (see
        // `effect_discount_program_case_strategy`'s doc comment), covering both matching and
        // non-matching programs within the same tree; `discount_pct` spans below `0`, within `0..=100`,
        // and above `100` - the reimplementation applies the raw arithmetic unconditionally, with no
        // range clamp on either side.
        {
            let test_name = "ZTRESEARCHMGR_SET_EFFECT_DISCOUNT";
            match runner.run(
                &(prop::collection::vec(effect_discount_branch_case_strategy(), 0..3), 0i32..=7i32, -50i32..150i32),
                |(cases, kind_raw, discount_pct)| {
                    let kind = ZTResearchEffectKind::try_from(kind_raw).expect("kind_raw generated in 0..=7, always a valid ZTResearchEffectKind");

                    let real_target_cost_bits = {
                        let branches = effect_discount_generated_branches(&cases);
                        live_support::with_standalone_mgr(&branches, |mgr| {
                            unsafe { ztresearchmgr::SET_EFFECT_DISCOUNT.original()((mgr as *mut ZTResearchMgr) as *const u32, kind_raw, discount_pct) };
                            mgr.branches()
                                .flat_map(|b| b.categories())
                                .flat_map(|c| c.programs())
                                .map(|p| p.target_cost().to_bits())
                                .collect::<Vec<_>>()
                        })
                    };

                    let reimpl_target_cost_bits = {
                        let branches = effect_discount_generated_branches(&cases);
                        live_support::with_standalone_mgr(&branches, |mgr| {
                            mgr.set_effect_discount(kind, discount_pct);
                            mgr.branches()
                                .flat_map(|b| b.categories())
                                .flat_map(|c| c.programs())
                                .map(|p| p.target_cost().to_bits())
                                .collect::<Vec<_>>()
                        })
                    };

                    prop_assert_eq!(
                        real_target_cost_bits,
                        reimpl_target_cost_bits,
                        "target_cost mismatch for kind_raw={}, discount_pct={} cases: {:?}",
                        kind_raw,
                        discount_pct,
                        cases
                    );
                    Ok(())
                },
            ) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Passed {}\n", test_name).as_bytes());
                    }
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

        // `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET` needs `GLOBAL_ZTWorldMgr` initialized (every
        // underlying effect call it exercises walks its entity-type list), which - confirmed live via
        // the Lua console's `get_zt_world_mgr()` - isn't true yet at this injection point (this early,
        // `LOAD_LANG_DLLS` hasn't even loaded language DLLs yet) but *is* true by the time
        // `ZTApp::updateSim` starts ticking (entity types load during app init, before the main loop
        // starts). So: stash this battery's `fail_flag` and hand off to the real `LOAD_LANG_DLLS`
        // instead of exiting here, so the game actually continues init through to the main loop;
        // `detour_update_sim` below runs the research test on the first tick and does the final exit
        // for the whole combined battery.
        EARLY_TESTS_FAILED.store(fail_flag, Ordering::SeqCst);
        unsafe { LOAD_LANG_DLLS_DETOUR.call(this) }
    }

    static EARLY_TESTS_FAILED: AtomicBool = AtomicBool::new(false);
    static RAN_UPDATE_SIM_TESTS: Once = Once::new();

    /// The resource-relative path vanilla's own boot-time `ZTMarketingMgr::loadConfigurations` call
    /// passes (e.g. `"mktg.cfg"`) - captured below so `run_marketingmgr_load_configurations_test` can
    /// reuse the real path. Only the first call's path is kept.
    static CAPTURED_MARKETING_PATH: OnceLock<String> = OnceLock::new();

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
    /// actually initialized: compares the real `ZTResearchProgram::onCompletion`/`reset` against
    /// `ztresearch::dispatch_on_completion`/`dispatch_reset` (via the public `on_completion()`/
    /// `reset()` wrappers) for every `effect_kind_raw` from `-1` (unset) through `8` (one past the
    /// last valid kind), on freestanding programs built with `live_support::build_standalone_program`
    /// (safe no-op target/entity ids - see its own doc comment). Only the low byte of the real return
    /// value is compared - the rest is undefined garbage left over in EAX from whatever vanilla called
    /// last (see `ZTResearchProgram::on_completion`'s doc comment). Appends to the same log file
    /// `detour_target` started, then performs the final exit for the whole combined battery.
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

        let test_name = "ZTRESEARCHPROGRAM_ON_COMPLETION_RESET";
        if globals().ztworldmgr_ptr().is_null() {
            info!("Skipping {}: GLOBAL_ZTWorldMgr not initialized at this injection point", test_name);
            write_success_line(&mut failure_log, &format!("{} (skipped: ZTWorldMgr not initialized)", test_name));
        } else {
            let runner_config = ProptestConfig {
                failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
                ..ProptestConfig::default()
            };
            let mut runner = proptest::test_runner::TestRunner::new(runner_config);
            match runner.run(&(-1i32..=8i32), |effect_kind_raw| {
                let (real_on_completion, reimpl_on_completion, real_reset, reimpl_reset) = live_support::with_standalone_mgr(&[], |mgr| {
                    live_support::with_global_ztresearchmgr_ptr(mgr, |_mgr| {
                        let real_ptr = live_support::build_standalone_program(effect_kind_raw);
                        let reimpl_ptr = live_support::build_standalone_program(effect_kind_raw);

                        let real_on_completion = unsafe { ztresearchprogram::ON_COMPLETION.original()(real_ptr as *const u32) } as u32;
                        let reimpl_on_completion = unsafe { (*reimpl_ptr).on_completion() };

                        let real_reset = unsafe { ztresearchprogram::RESET.original()(real_ptr as *const u32) } as u32;
                        let reimpl_reset = unsafe { (*reimpl_ptr).reset() };

                        live_support::destroy_standalone_program(real_ptr);
                        live_support::destroy_standalone_program(reimpl_ptr);

                        (real_on_completion, reimpl_on_completion, real_reset, reimpl_reset)
                    })
                });

                prop_assert_eq!(real_on_completion, reimpl_on_completion, "on_completion mismatch for effect_kind_raw={}", effect_kind_raw);
                prop_assert_eq!(real_reset, reimpl_reset, "reset mismatch for effect_kind_raw={}", effect_kind_raw);
                Ok(())
            }) {
                Ok(_) => {
                    info!("Proptest passed for {}", test_name);
                    write_success_line(&mut failure_log, test_name);
                }
                Err(e) => {
                    error!("Proptest failed: {:?}", e);
                    if let Some(ref mut log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                    }
                    fail_flag = true;
                }
            }
        }

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

    /// `ZTGAMEMGR_STANDALONE_ROUNDTRIP` - `ztgamemgr-implementation-plan.md` Stage 0: builds one
    /// standalone `ZTGameMgr` via the real vanilla free-function constructor
    /// (`ztgamemgr::live_support::build_standalone_mgr`, wrapping `standalone::CREATE_ZTGAME_MGR`),
    /// confirms it's non-null, dumps its raw bytes and logs which offsets are non-zero (resolves the
    /// "does `operator_new` zero the block" caveat empirically - `_CreateZTGameMgr.c` explicitly zeroes
    /// `started`/`soundscape_ptr`/`menu_music_handler_ptr` but says nothing about the rest), then
    /// immediately destroys it. No comparison logic yet - this only proves the construct/destroy harness
    /// itself is safe before Stage 1's `SET_NEW_GAME_DEFAULTS` test builds on it. Doesn't need a live
    /// zoo (`GLOBAL_ZTWorldMgr`/`GLOBAL_ZTGameMgr`), so it runs alongside the other standalone-only tests
    /// above, before `run_load_live_zoo`.
    fn run_gamemgr_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_STANDALONE_ROUNDTRIP";
        let ptr = gamemgr_live_support::build_standalone_mgr();
        if ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
            }
            return true;
        }

        let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, struct_size) };
        let non_zero_offsets: Vec<usize> = bytes.iter().enumerate().filter(|(_, b)| **b != 0).map(|(offset, _)| offset).collect();
        info!(
            "{}: freshly-constructed standalone ZTGameMgr has {} non-zero bytes out of {}; offsets: {:?}",
            test_name,
            non_zero_offsets.len(),
            struct_size,
            non_zero_offsets
        );

        gamemgr_live_support::destroy_standalone_mgr(ptr);
        write_success_line(failure_log, test_name);
        false
    }

    /// `MENUMUSICHANDLER_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
    /// `ztgamemgr_menumusichandler::init()`, and this asserts all five of its detours actually report
    /// enabled. Without it, a silently-failed `init_detours()` (error logged, game continues on
    /// vanilla) would leave the whole battery green while every hooked production path runs vanilla -
    /// the trampoline-based comparisons below can't distinguish that from a working hook. Runs before
    /// the other `MENUMUSICHANDLER_*` tests so a wiring failure is visible first.
    fn run_menumusichandler_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "MENUMUSICHANDLER_DETOURS_ENABLED";
        let mut disabled: Vec<&'static str> = Vec::new();
        for (name, enabled) in menumusichandler_live_support::detour_status() {
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

    /// `MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE` - debug-only anti-regression for
    /// `openzt-detour`'s hook registry: `FunctionDef::original()` must return the *real vanilla*
    /// function (routed through the detour's trampoline) even for the five addresses this battery
    /// has itself hooked, not silently re-enter our own Rust detours. For each of them, asserts the
    /// registry holds a trampoline, that `.original()` returns exactly that pointer value, and that
    /// it differs from the raw address (zoo.exe has no ASLR, so an un-routed raw cast would compare
    /// equal - pointer equality can't pass vacuously here the way the old `.original() == .original()`
    /// comparisons could). Also asserts zero registry overflows: a full slot array fails open into
    /// exactly the raw-cast behavior this test guards against. Release builds cfg this out (the raw
    /// cast is release's documented `.original()`); the release battery is still run once-off since
    /// its vanilla poles go through the `real_*` trampolines instead.
    #[cfg(debug_assertions)]
    fn run_menumusichandler_original_routes_to_trampoline_test(failure_log: &mut Option<std::fs::File>) -> bool {
        use openzt_detour::generated::ztgamemgr_menumusichandler as mmh;

        /// `.original()`'s return value as a raw pointer value. The pointer is only inspected,
        /// never called.
        fn original_ptr<T>(def: &FunctionDef<T>) -> usize
        where
            T: retour::Function,
        {
            let original = unsafe { def.original() };
            original.to_ptr() as usize
        }

        let test_name = "MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE";
        let hooked: [(&'static str, u32, usize); 5] = [
            ("CONSTRUCTOR", mmh::MENU_MUSIC_HANDLER_1.address, original_ptr(&mmh::MENU_MUSIC_HANDLER_1)),
            ("INIT", mmh::INIT.address, original_ptr(&mmh::INIT)),
            ("START_PLAY", mmh::START_PLAY.address, original_ptr(&mmh::START_PLAY)),
            ("START_FADE", mmh::START_FADE.address, original_ptr(&mmh::START_FADE)),
            ("UPDATE", mmh::UPDATE.address, original_ptr(&mmh::UPDATE)),
        ];

        let mut failures: Vec<String> = Vec::new();
        for (name, address, original) in hooked {
            match openzt_detour::trampoline_for(address) {
                Some(trampoline) => {
                    if original != trampoline {
                        failures.push(format!(
                            "{name} ({address:#010x}): .original() = {original:#010x} != registered trampoline {trampoline:#010x}"
                        ));
                    }
                    if original == address as usize {
                        failures.push(format!(
                            "{name} ({address:#010x}): .original() equals the raw address - routing fell back to the raw cast"
                        ));
                    }
                }
                None => failures.push(format!(
                    "{name} ({address:#010x}): no trampoline registered - detour() did not publish, or the registry overflowed"
                )),
            }
        }
        let overflow = openzt_detour::registry_overflow_count();
        if overflow != 0 {
            failures.push(format!("{overflow} address(es) failed to register in the hook registry (capacity overflow - fail-open raw casts)"));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes(),
                );
            }
            true
        }
    }

    /// `MENUMUSICHANDLER_STANDALONE_ROUNDTRIP` - `menumusichandler-implementation-plan.md` Stage 1: builds
    /// two fresh `0x14`-byte standalone `MenuMusicHandler` blocks, runs the real vanilla constructor
    /// (via the `real_constructor` trampoline) on one and the Rust reimplementation
    /// (`MenuMusicHandler::construct`) directly on the other, then byte-diffs the full struct. Both sides
    /// call through to the same real `BFIniFile::read("UI", "noMenuMusic", 0)`, so `ini_menu_music_disabled`
    /// should come out identical too - with one acknowledged blind spot: on a default install the key is
    /// absent, so both sides read 0 and a section/key argument-order bug would pass silently (the `.c`/
    /// `.asm` confirm the order; setting `noMenuMusic=1` in `UI.ini` and re-running would exercise the
    /// real disabled path instead of the marker-forced one). No exclusions needed for the fields the
    /// constructor actually touches. Both blocks are pre-zeroed before either constructor runs: neither
    /// the real constructor
    /// nor `MenuMusicHandler::construct` writes the `_pad1`/`_pad2` bytes (confirmed by the first live run
    /// of this test, which saw real-side offsets `5..8` come back as raw `operator_new` heap leftovers,
    /// `[175, 235, 3]`, against the Rust side's zeroed padding) - same "operator_new doesn't zero memory"
    /// caveat `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` documents.
    fn run_menumusichandler_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "MENUMUSICHANDLER_STANDALONE_ROUNDTRIP";

        let real_ptr = menumusichandler_live_support::allocate_uninitialized();
        let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(reimpl_ptr);
            }
            return true;
        }

        let struct_size = size_of::<ztgamemgr_menumusichandler::MenuMusicHandler>();
        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);

            menumusichandler_live_support::real_constructor(real_ptr as *const u32);
            (*reimpl_ptr).construct();
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) };
        let mismatches: Vec<(usize, u8, u8)> =
            (0..struct_size).filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None }).collect();

        let failed = !mismatches.is_empty();
        if failed {
            error!("{}: byte mismatch(es) (offset, real, reimpl): {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: byte mismatch(es) (offset, real, reimpl): {:?}\n", test_name, mismatches).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        menumusichandler_live_support::destroy_standalone(real_ptr);
        menumusichandler_live_support::destroy_standalone(reimpl_ptr);
        failed
    }

    /// `ZTSOUNDSCAPE_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
    /// `ztsoundscape::init()`, and this asserts all three of its detours actually report enabled.
    /// Without it, a silently-failed `init_detours()` (error logged, game continues on vanilla) would
    /// leave the whole battery green while every hooked production path runs vanilla - the
    /// trampoline-based comparisons below can't distinguish that from a working hook. Runs before the
    /// other `ZTSOUNDSCAPE_*` tests so a wiring failure is visible first.
    fn run_ztsoundscape_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSOUNDSCAPE_DETOURS_ENABLED";
        let mut disabled: Vec<&'static str> = Vec::new();
        for (name, enabled) in soundscape_live_support::detour_status() {
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

    /// `ZTSOUNDSCAPE_ORIGINAL_ROUTES_TO_TRAMPOLINE` - debug-only anti-regression for
    /// `openzt-detour`'s hook registry, same shape as
    /// [`run_menumusichandler_original_routes_to_trampoline_test`] over this class's three hooked
    /// addresses: `FunctionDef::original()` must return the *real vanilla* function (routed through the
    /// detour's trampoline) even for the addresses this battery has itself hooked, not silently re-enter
    /// our own Rust detours. See that test's doc comment for the full rationale (pointer equality vs.
    /// the raw address, registry-overflow fail-open check, release cfg-out).
    #[cfg(debug_assertions)]
    fn run_ztsoundscape_original_routes_to_trampoline_test(failure_log: &mut Option<std::fs::File>) -> bool {
        use openzt_detour::generated::ztsoundscape as gen_ztsoundscape;

        /// `.original()`'s return value as a raw pointer value. The pointer is only inspected,
        /// never called.
        fn original_ptr<T>(def: &FunctionDef<T>) -> usize
        where
            T: retour::Function,
        {
            let original = unsafe { def.original() };
            original.to_ptr() as usize
        }

        let test_name = "ZTSOUNDSCAPE_ORIGINAL_ROUTES_TO_TRAMPOLINE";
        let hooked: [(&'static str, u32, usize); 3] = [
            ("UPDATE", gen_ztsoundscape::UPDATE.address, original_ptr(&gen_ztsoundscape::UPDATE)),
            ("INIT", gen_ztsoundscape::INIT.address, original_ptr(&gen_ztsoundscape::INIT)),
            ("CONSTRUCTOR", gen_ztsoundscape::CONSTRUCTOR.address, original_ptr(&gen_ztsoundscape::CONSTRUCTOR)),
        ];

        let mut failures: Vec<String> = Vec::new();
        for (name, address, original) in hooked {
            match openzt_detour::trampoline_for(address) {
                Some(trampoline) => {
                    if original != trampoline {
                        failures.push(format!(
                            "{name} ({address:#010x}): .original() = {original:#010x} != registered trampoline {trampoline:#010x}"
                        ));
                    }
                    if original == address as usize {
                        failures.push(format!(
                            "{name} ({address:#010x}): .original() equals the raw address - routing fell back to the raw cast"
                        ));
                    }
                }
                None => failures.push(format!(
                    "{name} ({address:#010x}): no trampoline registered - detour() did not publish, or the registry overflowed"
                )),
            }
        }
        let overflow = openzt_detour::registry_overflow_count();
        if overflow != 0 {
            failures.push(format!("{overflow} address(es) failed to register in the hook registry (capacity overflow - fail-open raw casts)"));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes(),
                );
            }
            true
        }
    }

    /// `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP` - `ztsoundscape-implementation-plan.md` stage 1 (pulled
    /// forward from stage 5): builds two fresh `0x54`-byte standalone `ZTSoundscape` blocks, runs the
    /// real vanilla constructor on one and the Rust reimplementation (`ZTSoundscape::construct`) on the
    /// other, then byte-diffs the full struct. The constructor is pure constant writes, so the compare
    /// is meaningful with no exclusions.
    ///
    /// Both blocks are pre-zeroed before either constructor runs: the ctor writes only 32 of the `0x54`
    /// bytes (the three embedded slots' `{vtable, inner}` dwords and the two `Ambients` pointers) - the
    /// scalars, the filename/atten tables, and the `+0xb` pad byte stay heap garbage on both sides until
    /// `init` writes them. Comparing uninitialized memory would diff `operator_new` leftovers, not
    /// constructor behavior (same "operator_new doesn't zero" precedent as
    /// `MENUMUSICHANDLER_STANDALONE_ROUNDTRIP` and `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS`).
    ///
    /// Pole note: the vanilla side goes through `soundscape_live_support::real_constructor` (a
    /// `CONSTRUCTOR_DETOUR.call` trampoline) - the stage-4 obligation this test used to document (a
    /// release build's raw-cast `.original()` would silently re-enter the Rust detour and degenerate
    /// the test into Rust-vs-Rust) is discharged now that the detours are installed.
    fn run_ztsoundscape_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP";

        let real_ptr = soundscape_live_support::allocate_uninitialized();
        let reimpl_ptr = soundscape_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                soundscape_live_support::destroy_standalone(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                soundscape_live_support::destroy_standalone(reimpl_ptr);
            }
            return true;
        }

        let struct_size = size_of::<ZTSoundscape>();
        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);

            soundscape_live_support::real_constructor(real_ptr as *const c_void);
            (*reimpl_ptr).construct();
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) };
        let mismatches: Vec<(usize, u8, u8)> =
            (0..struct_size).filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None }).collect();

        let failed = !mismatches.is_empty();
        if failed {
            error!("{}: byte mismatch(es) (offset, real, reimpl): {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: byte mismatch(es) (offset, real, reimpl): {:?}\n", test_name, mismatches).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        soundscape_live_support::destroy_standalone(real_ptr);
        soundscape_live_support::destroy_standalone(reimpl_ptr);
        failed
    }

    /// `ZTSHOWMGR_STANDALONE_ROUNDTRIP` - `ztshowmgr-implementation-plan.md` stage 1: builds two fresh
    /// `0x44`-byte standalone `ZTShowMgr` blocks, runs the real vanilla constructor on one and the Rust
    /// reimplementation ([`ZTShowMgr::construct`]) on the other, then compares them field-by-field.
    ///
    /// Not a whole-struct byte-diff, deliberately - three field groups *can't* legally compare equal
    /// across two separately-constructed instances, each excluded for its own documented reason:
    /// - `+0x8..+0x28` (the eight `initShowParams` thresholds): the real ctor's post-default values are
    ///   config-dependent (`BFConfigFile`/`shows.cfg`, gated on expansion pack 2) - stage 2 ports that
    ///   half. The Rust side's pre-config defaults are asserted exactly instead.
    /// - `+0x28`/`+0x38` (both maps' `DAT_00638008`-freelist header pointers): per-instance
    ///   allocations, and `construct` deliberately leaves them null (see its doc comment). The real
    ///   side's nodes are instead *shape*-checked (color `0`, null parent, left/right self-referential
    ///   - the standard empty MSVC `_Tree` header), and the reimpl side's nullness is asserted.
    /// - `+0x30`/`+0x40` (the two tag bytes): each instance writes the high byte of *its own* address
    ///   there, so each side is checked against its own address rather than against the other.
    ///
    /// Everything else compares byte-identical: both vtables (`+0x0`/`+0x34`), `+0x4` and every padding
    /// byte, and both map sizes. Also asserts the Rust registered-shows store stays empty (standalone
    /// construction must never touch it).
    ///
    /// Pole note: `ztshowmgr::CONSTRUCTOR` is deliberately never detoured (see `ztshowmgr.rs`'s module
    /// doc comment), so `.original()` reaches real vanilla in every build profile. If that ever changes,
    /// this test needs the `*_DETOUR.call` trampoline treatment instead - see
    /// `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP`'s own doc comment for that exact failure mode. Since stage 2,
    /// though, the real ctor's *internal* tail-call into `initShowParams` now lands in the stage-2 Rust
    /// detour (the address itself is patched, in every profile) - harmless here because everything that
    /// detour writes (the eight threshold fields) is excluded from the byte-compare groups below and
    /// identical to what vanilla's own `initShowParams` would have written; noted so the exclusion isn't
    /// mistaken for a vanilla-vs-vanilla guarantee. Teardown is
    /// leak-only: the real-ctor side owns freelist nodes with no safe Rust-side return path (see
    /// `showmgr_live_support::allocate_uninitialized`'s doc comment), so both buffers stay allocated for
    /// the one-shot test process's lifetime.
    fn run_ztshowmgr_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_STANDALONE_ROUNDTRIP";

        let real_ptr = showmgr_live_support::allocate_uninitialized();
        let reimpl_ptr = showmgr_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes(),
                );
            }
            return true;
        }

        let mut failures: Vec<String> = Vec::new();
        let struct_size = size_of::<ZTShowMgr>();
        let real_addr = real_ptr as u32;
        let reimpl_addr = reimpl_ptr as u32;

        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);

            ZTSHOWMGR_CONSTRUCTOR.original()(real_ptr as *const u32);
            (*reimpl_ptr).construct();

            // Byte-identical regions: everything except the three exclusion groups above.
            let real_bytes = std::slice::from_raw_parts(real_ptr as *const u8, struct_size);
            let reimpl_bytes = std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size);
            const COMPARED: [(usize, usize); 5] = [(0x00, 0x08), (0x2c, 0x30), (0x34, 0x38), (0x3c, 0x40), (0x41, 0x44)];
            for (start, end) in COMPARED {
                for i in start..end {
                    if real_bytes[i] != reimpl_bytes[i] {
                        failures.push(format!("byte {:#04x}: real={:#04x} reimpl={:#04x}", i, real_bytes[i], reimpl_bytes[i]));
                    }
                }
            }

            // Both tag bytes carry each instance's own high address byte.
            for (addr, name) in [(real_addr, "real"), (reimpl_addr, "reimpl")] {
                for offset in [0x30, 0x40] {
                    let tag = get_from_memory::<u8>(addr + offset);
                    if tag != (addr >> 24) as u8 {
                        failures.push(format!("{} +{:#x}: tag byte {:#04x}, expected {:#04x}", name, offset, tag, (addr >> 24) as u8));
                    }
                }
            }

            // Real side: both maps get a real freelist node, shaped like a standard empty MSVC
            // `_Tree` header. Reimpl side: both stay null by design.
            for (offset, name) in [(0x28, "ZTShowMgr map"), (0x38, "embedded ZTShowScriptMgr map")] {
                let header = get_from_memory::<u32>(real_addr + offset);
                if header == 0 {
                    failures.push(format!("real {name} header is null"));
                } else {
                    let color = get_from_memory::<u8>(header);
                    let parent = get_from_memory::<u32>(header + 4);
                    let left = get_from_memory::<u32>(header + 8);
                    let right = get_from_memory::<u32>(header + 0xc);
                    if color != 0 || parent != 0 || left != header || right != header {
                        failures.push(format!(
                            "real {name} header {header:#x} is not a self-referential empty _Tree header (color={color}, parent={parent:#x}, left={left:#x}, right={right:#x})"
                        ));
                    }
                }
                let reimpl_header = get_from_memory::<u32>(reimpl_addr + offset);
                if reimpl_header != 0 {
                    failures.push(format!("reimpl {name} header should be null, got {reimpl_header:#x}"));
                }
            }
        }

        // The Rust side's pre-config threshold defaults (`initShowParams`'s own writes before its
        // expansion-gated `shows.cfg` override).
        const DEFAULT_THRESHOLDS: [(u32, u32); 8] =
            [(0x8, 0), (0xc, 3), (0x10, 6), (0x14, 0x19), (0x18, 0x32), (0x1c, 0x4b), (0x20, 6), (0x24, 6)];
        for (offset, expected) in DEFAULT_THRESHOLDS {
            let actual = unsafe { get_from_memory::<u32>(reimpl_addr + offset) };
            if actual != expected {
                failures.push(format!("reimpl +{offset:#x}: {actual}, expected default {expected}"));
            }
        }

        if ztshowmgr::registered_show_count() != 0 {
            failures.push(format!(
                "Rust registered-shows store should stay empty across standalone construction, has {} entries",
                ztshowmgr::registered_show_count()
            ));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_INIT_SHOW_PARAMS` - `ztshowmgr-implementation-plan.md` stage 2: builds two fresh
    /// standalone `ZTShowMgr` blocks, runs the real vanilla `initShowParams` on one and the Rust
    /// reimplementation ([`ZTShowMgr::init_show_params`]) on the other, then byte-compares the eight
    /// config-loaded threshold fields at `+0x8..+0x28` (the only memory this function touches).
    ///
    /// Environment-sensitive by design: both sides share the same expansion-2 gate (real
    /// `BFApp::getInstalledExpansion` on the live `GLOBAL_ZTApp`) and the same real `shows.cfg`, so
    /// the comparison is meaningful in either state - on a machine with expansion pack 2 installed
    /// both sides must carry identical config-override values, otherwise both must carry identical
    /// defaults. The gate state and both sides' final values are logged so a run can tell which path
    /// it exercised. Vanilla-allocator side effects are balanced on both sides (each constructs and
    /// releases its own stack-local `BFConfigFile`; the Rust port also returns the config's tree-root
    /// node to the freelist it came from - see the method's doc comment).
    ///
    /// Pole note: the vanilla side goes through `showmgr_live_support::call_real_init_show_params`
    /// (the `INIT_SHOW_PARAMS_DETOUR.call` trampoline) - a release build's raw-cast `.original()`
    /// would silently re-enter the Rust detour and degenerate the test into Rust-vs-Rust, the exact
    /// mode `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP`'s doc comment describes. Teardown is leak-only (see
    /// `showmgr_live_support::allocate_uninitialized`).
    fn run_ztshowmgr_init_show_params_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_INIT_SHOW_PARAMS";

        let real_ptr = showmgr_live_support::allocate_uninitialized();
        let reimpl_ptr = showmgr_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr)
                        .as_bytes(),
                );
            }
            return true;
        }

        let mut failures: Vec<String> = Vec::new();

        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, size_of::<ZTShowMgr>());
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, size_of::<ZTShowMgr>());

            let real_return = showmgr_live_support::call_real_init_show_params(real_ptr as *const u32);
            let reimpl_return = (*reimpl_ptr).init_show_params();
            // The real body's only return write is `MOV AL, 0x1` - the upper 24 bits of EAX are
            // leftover register garbage (the `.c` decompile's own `CONCAT31(...,1)` return), so only
            // the low byte is compared.
            if real_return & 0xff != 1 || reimpl_return != 1 {
                failures.push(format!(
                    "returns differ: real={:#x} (low byte {:#04x}) reimpl={reimpl_return}",
                    real_return,
                    real_return & 0xff
                ));
            }

            let real_bytes = std::slice::from_raw_parts(real_ptr as *const u8, size_of::<ZTShowMgr>());
            let reimpl_bytes = std::slice::from_raw_parts(reimpl_ptr as *const u8, size_of::<ZTShowMgr>());
            for i in 0x8..0x28 {
                if real_bytes[i] != reimpl_bytes[i] {
                    failures.push(format!("byte {:#04x}: real={:#04x} reimpl={:#04x}", i, real_bytes[i], reimpl_bytes[i]));
                }
            }
        }

        // Log which path the environment exercised, and surface both sides' final values so a run
        // can tell config-override from defaults at a glance. `GLOBAL_ZTApp`'s RVA is
        // `ztshowmgr.rs`'s own private `GLOBAL_ZTAPP_RVA` - re-declared here per the repo's
        // no-shared-consts convention.
        let global_ztapp_rva: u32 = 0x00638154 - 0x400000;
        let ztapp_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + global_ztapp_rva);
        let expansion_2_installed = ztapp_ptr != 0
            && unsafe { BFAPP_GET_INSTALLED_EXPANSION.original()(ztapp_ptr as *const u32, 2) } != 0;
        let threshold_names = [
            ("badTrick", 0x8),
            ("goodTrick", 0xc),
            ("greatTrick", 0x10),
            ("badShow", 0x14),
            ("goodShow", 0x18),
            ("greatShow", 0x1c),
            ("minIdealLength", 0x20),
            ("maxIdealLength", 0x24),
        ];
        let values: Vec<String> = threshold_names
            .iter()
            .map(|(name, offset)| {
                format!("{}={}", name, unsafe { get_from_memory::<u32>(real_ptr as u32 + offset) })
            })
            .collect();
        let gate_state = if expansion_2_installed {
            "OPEN (config override expected)"
        } else {
            "closed (defaults expected)"
        };

        if failures.is_empty() {
            write_success_line(failure_log, &format!("{} (expansion-2 gate {}, {})", test_name, gate_state, values.join(", ")));
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_REGISTER_UNREGISTER_SHOW` - `ztshowmgr-implementation-plan.md` stages 3+9: drives
    /// the full `registerShow`/`unregisterShow` ports (stage 9 dropped the stage-3 shadow/mirror
    /// call-throughs, so the hooked addresses are pure Rust now) through every branch of both
    /// vanilla bodies (`.asm`/`.c`-verified), pinning the store content after each op. The old
    /// cross-store diff oracle - the real vanilla tree agreeing with the store after every hooked
    /// mutation - is gone with the dual-write it verified: hooked writers stopped maintaining the
    /// tree in stage 9, which this test now pins *positively* (the tree must stay empty under
    /// hooked writes, through the `GET_SHOW_INFO` trampoline).
    ///
    /// The op matrix covers: null-show register, preset-id register (the no-force reuse path -
    /// counter untouched), the already-registered early return (with and without force, and never
    /// consuming the counter), the id-0 fresh-id assignment (deterministic via a seeded counter:
    /// the id is the post-increment counter masked to 16 bits, here exactly `0x0101`), the fresh-id
    /// setter's embedded-`ZTShow` sync (`+0x6` id copy, `+0x10` back-pointer), force over an
    /// unregistered preset id (fresh counter id, the preset value never entering the store), the
    /// insert-or-assign **collision** (a force-assigned fresh id landing exactly on a registered
    /// key overwrites that entry's value in place - the stolen show keeps its stale `field_0x70`
    /// but is unreachable by id), path-A/B/C unregisters (both clear-flag states - `clear=true`
    /// really executes the real `clearShowScriptStates` through path C, which targets the show
    /// directly with no lookup), double unregister (absent-key silent success), the null+null
    /// `AL=0` return, and the counter **wrap** semantics (`0xffff` is never assigned - counter
    /// `0xfffe` assigns id `0`; a counter of `0xffff` wraps to `0` and also assigns id `0`; and a
    /// `field_0x70 == 0` show whose store already holds key `0` early-returns even with force,
    /// because vanilla's find runs before the fresh-id branch).
    ///
    /// `clear=true` on a standalone `ZTShowInfo` needs one piece of setup: the embedded `ZTShow`'s
    /// script-state map header at `show_info+0x38` (read unconditionally by the real
    /// `ZTShowState::clear`). Real vanilla keeps it as a separate `0x18`-byte freelist node
    /// (self-referential when empty), and a zeroed buffer would crash the first clear-flag op, so
    /// each show gets a real, leak-only, empty-tree header node allocated there - the same shape
    /// `ZTSHOWMGR_STANDALONE_ROUNDTRIP` verifies on the real constructor's own map headers. All
    /// teardown is leak-only (`showmgr_live_support::allocate_uninitialized`'s doc comment); the
    /// store is drained through the hooked unregister path and asserted empty, and the counter is
    /// restored, since both are process-global state shared with the rest of the battery.
    fn run_ztshowmgr_register_unregister_show_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_REGISTER_UNREGISTER_SHOW";

        if ztshowmgr::registered_show_count() != 0 {
            let msg = "Rust registered-shows store should be empty when the test starts";
            error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
            }
            return true;
        }

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }

        /// Pre-initializes `show_info`'s embedded `ZTShow` script-state map header (`show_info+0x38`)
        /// with a real, empty, self-referential `_Tree` header node - see the test's doc comment for
        /// why a zeroed buffer can't survive the clear-flag ops. Leak-only, like everything else
        /// this test allocates.
        fn init_script_state_header(show_info: u32) {
            let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
            unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
            save_to_memory(node + 0x8, node);
            save_to_memory(node + 0xc, node);
            save_to_memory(show_info + 0x38, node);
        }

        let mut failures: Vec<String> = Vec::new();
        let mgr_addr = mgr as u32;

        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        let show_a = ztshow_live_support::build_standalone_show_info();
        let show_b = ztshow_live_support::build_standalone_show_info();
        let show_c = ztshow_live_support::build_standalone_show_info();
        let show_d = ztshow_live_support::build_standalone_show_info();
        for show in [show_a, show_b, show_c, show_d] {
            init_script_state_header(show);
        }

        // Preset ids; show_b keeps its zero-init `field_0x70` (the counter-assignment case).
        const PRESET_ID_A: u16 = 0x1234;
        const PRESET_ID_C: u16 = 0x4321;
        const PRESET_ID_D: u16 = 0x9999;
        save_to_memory(show_a + 0x70, PRESET_ID_A);
        save_to_memory(show_c + 0x70, PRESET_ID_C);
        save_to_memory(show_d + 0x70, PRESET_ID_D);

        // All ops go through the hooked addresses (the raw function address - the detour itself,
        // installed by `reimplementation_tests::init`'s `crate::ztshowmgr::init()`), so what is
        // exercised is the promoted live path, not a test-side shortcut.
        let register =
            |show: u32, force: bool| -> u32 { unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show as *const u32, force) } };
        let unregister =
            |id: u16, show: u32, clear: bool| -> u32 { unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, id, show as *const u32, clear) } };

        // Stage 9 pin: the standalone vanilla tree stays inert under hooked writes. Nothing in this
        // test plants through the raw trampoline, so every id must read back empty through the real
        // `getShowInfo` walk even while the store holds registrations - if a writer call-through
        // ever came back, this is where it shows.
        let assert_tree_inert = |step: &str, ids: &[u16], failures: &mut Vec<String>| {
            for id in ids {
                if showmgr_live_support::call_real_get_show_info(mgr_addr as *const u32, *id) != 0 {
                    failures.push(format!("{step}: id {id:#06x} - the standalone vanilla tree should be inert (stage 9 dropped the writer call-throughs), but the real getShowInfo walk found an entry"));
                }
            }
        };

        // Restore point for the process-global counter; every block below seeds exact values, so
        // the pins hold regardless of what earlier battery stages left here.
        let counter_start = showmgr_live_support::show_id_counter();

        // Null-show register: vanilla's AL=0 early return, nothing written.
        if register(0, false) != 0 {
            failures.push("register(null, false) should return 0".to_string());
        }

        // Preset-id register - the no-force reuse path: the id is kept and the counter is left
        // untouched (the complementary pin to the force-fresh split below).
        let counter_before = showmgr_live_support::show_id_counter();
        if register(show_a, false) != 1 {
            failures.push("register(A, false) with preset id should return 1".to_string());
        }
        if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a) {
            failures.push("store[A's preset id] should be show A after register(A, false)".to_string());
        }
        if showmgr_live_support::show_id_counter() != counter_before {
            failures.push("the no-force reuse path must leave the counter untouched".to_string());
        }

        // Already-registered early return - vanilla's find on the *current* field_0x70 hits,
        // nothing written, with or without force (the force flag is only read after the miss), and
        // the counter is never consumed.
        if register(show_a, false) != 0 {
            failures.push("re-register(A, false) should return 0 (already registered)".to_string());
        }
        if register(show_a, true) != 0 {
            failures.push("re-register(A, true) should return 0 (already registered; force must not reach the counter)".to_string());
        }
        if get_from_memory::<u16>(show_a + 0x70) != PRESET_ID_A {
            failures.push("A's field_0x70 should be untouched by the already-registered early returns".to_string());
        }
        if showmgr_live_support::show_id_counter() != counter_before {
            failures.push("the already-registered early returns must leave the counter untouched".to_string());
        }
        assert_tree_inert("after A's ops", &[PRESET_ID_A], &mut failures);

        // Id-0 fresh-id assignment, made deterministic by seeding the counter: the assigned id is
        // the post-increment counter, exactly 0x0101 here - no wrap ambiguity.
        showmgr_live_support::set_show_id_counter(0x0100);
        if register(show_b, false) != 1 {
            failures.push("register(B, false) with id 0 should return 1".to_string());
        }
        let id_b = get_from_memory::<u16>(show_b + 0x70);
        if id_b != 0x0101 {
            failures.push(format!("register(B, false) from a seeded counter of 0x0100 should assign exactly 0x0101 (INC then 16-bit read), got {id_b:#06x}"));
        }
        if showmgr_live_support::show_id_counter() != 0x0101 {
            failures.push(format!(
                "the fresh-id path should have advanced the counter 0x0100 -> 0x0101, got {:#06x}",
                showmgr_live_support::show_id_counter()
            ));
        }
        if ztshowmgr::registered_show_for_id(id_b) != Some(show_b) {
            failures.push(format!("store[assigned id {id_b:#06x}] should be show B after register(B, false)"));
        }
        // The fresh id went through the ported `ZTShowInfo::setShowInfoID`: the embedded `ZTShow`'s
        // `+0x6` id copy and `+0x10` back-pointer (was zero) must be in sync, not just `field_0x70`.
        if get_from_memory::<u16>(show_b + 0x4 + 0x6) != id_b {
            failures.push("the embedded ZTShow's +0x6 id copy should carry the fresh id".to_string());
        }
        if get_from_memory::<u32>(show_b + 0x4 + 0x10) != show_b {
            failures.push("the embedded ZTShow's +0x10 back-pointer should point at the show".to_string());
        }

        // Force over an unregistered preset id: a fresh counter id is assigned even though C's
        // field_0x70 was non-zero, and the preset value itself never enters the store.
        if register(show_c, true) != 1 {
            failures.push("register(C, true) should return 1".to_string());
        }
        let id_c = get_from_memory::<u16>(show_c + 0x70);
        if id_c != 0x0102 {
            failures.push(format!("register(C, true) should have force-assigned exactly the next counter id 0x0102, got {id_c:#06x}"));
        }
        if ztshowmgr::registered_show_for_id(id_c) != Some(show_c) {
            failures.push(format!("store[force-assigned id {id_c:#06x}] should be show C after register(C, true)"));
        }
        if ztshowmgr::registered_show_for_id(PRESET_ID_C).is_some() {
            failures.push("store should not hold C's preset id - force reassignment never inserts it".to_string());
        }
        assert_tree_inert("after B+C registers", &[PRESET_ID_A, id_b, id_c, 0x0000, 0xffff, 0x7fff, 0x8000], &mut failures);

        // Insert-or-assign collision: seed the counter so D's force-assigned fresh id lands exactly
        // on A's registered key. Vanilla's tree write overwrites the existing entry's value in
        // place - D steals A's slot; A keeps its stale field_0x70 but is unreachable by id.
        showmgr_live_support::set_show_id_counter(PRESET_ID_A - 1);
        if register(show_d, true) != 1 {
            failures.push("register(D, true) should return 1".to_string());
        }
        if get_from_memory::<u16>(show_d + 0x70) != PRESET_ID_A {
            failures.push(format!(
                "D's force-assigned id should be exactly {PRESET_ID_A:#06x} (the seeded collision), got {:#06x}",
                get_from_memory::<u16>(show_d + 0x70)
            ));
        }
        if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_d) {
            failures.push("store[A's id] should now be show D - the collision overwrites the entry's value in place".to_string());
        }
        if get_from_memory::<u16>(show_a + 0x70) != PRESET_ID_A {
            failures.push("stolen show A must keep its stale field_0x70 (vanilla never repairs it)".to_string());
        }
        if ztshowmgr::registered_show_for_id(PRESET_ID_D).is_some() {
            failures.push("store should not hold D's preset id - the force-fresh path replaced it before the insert".to_string());
        }
        if showmgr_live_support::show_id_counter() != PRESET_ID_A {
            failures.push("the collision register should have consumed exactly one counter step".to_string());
        }

        // Path A (show == null, id != 0): erase by id alone, using B's id read back above.
        if unregister(id_b, 0, false) != 1 {
            failures.push(format!("unregister({id_b:#06x}, null, false) should return 1"));
        }

        // Path B (show != null, id != 0), both clear-flag states. (The clear target here is picked
        // off the store, where each op's id has already been removed by its preceding unregister -
        // so neither flag state reaches a real clearShowScriptStates; that stays exercised through
        // path C below.)
        if unregister(PRESET_ID_A, show_a, false) != 1 {
            failures.push("unregister(A's id, A, false) should return 1".to_string());
        }
        if unregister(PRESET_ID_A, show_a, true) != 1 {
            failures.push("unregister(A's id, A, true) should return 1 (absent-key erase is still success)".to_string());
        }

        // Absent-key id unregister: silent no-op success.
        const ABSENT_PROBE_ID: u16 = 0x0bb7;
        if unregister(ABSENT_PROBE_ID, 0, false) != 1 {
            failures.push(format!("unregister(absent id {ABSENT_PROBE_ID:#06x}, null, false) should return 1 (silent no-op)"));
        }

        // Path C (show != null, id == 0): the id is derived from the show's own field_0x70 -
        // deliberately stale after a prior unregister, since vanilla never zeroes that field.
        if unregister(0, show_a, false) != 1 {
            failures.push("unregister(0, A, false) should return 1".to_string());
        }

        // Re-register A: with field_0x70 still carrying the stale preset id and that id no longer
        // in the store, the preset path re-registers under the very same id.
        if register(show_a, false) != 1 {
            failures.push("re-register(A, false) after unregister should return 1 (stale field_0x70 is reusable)".to_string());
        }
        if get_from_memory::<u16>(show_a + 0x70) != PRESET_ID_A {
            failures.push("A's field_0x70 should still carry the stale preset id after unregister + re-register".to_string());
        }
        if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a) {
            failures.push("store[A's preset id] should be show A again after the re-register".to_string());
        }

        // clear=true through path C: the one op that really executes the real
        // `clearShowScriptStates` (targeted directly at the show, no lookup), running it over the
        // pre-initialized empty header node at show+0x38.
        if unregister(0, show_b, true) != 1 {
            failures.push("unregister(0, B, true) should return 1".to_string());
        }

        // Double unregister: absent-key silent success again, store unchanged.
        if unregister(0, show_b, false) != 1 {
            failures.push("double unregister(0, B, false) should still return 1 (silent no-op)".to_string());
        }

        // Null show + null id: vanilla's AL=0 early return.
        if unregister(0, 0, false) != 0 {
            failures.push("unregister(0, null, false) should return 0".to_string());
        }
        assert_tree_inert("after the unregister matrix", &[PRESET_ID_A, id_b, id_c, ABSENT_PROBE_ID], &mut failures);

        // Cleanup of the main matrix: drain through the hooked unregister (path C derives each
        // show's own stale field_0x70, which is exactly the key each insert used).
        for show in [show_a, show_b, show_c, show_d] {
            unregister(0, show, false);
        }

        // Counter wrap semantics, both directions of the boundary:
        // - from 0xfffe the increment lands on 0xffff, and 0xffff % 0xffff == 0 - so id 0xffff is
        //   never assigned; the show registers under key 0 with field_0x70 left 0;
        // - the next id-0 register then finds key 0 already held by the *current* field_0x70 value
        //   (0) and early-returns - even with force, which vanilla only reads after that find;
        // - after key 0 drains, a counter of 0xffff itself increments (word wrap) to 0 and again
        //   assigns id 0.
        let show_x = ztshow_live_support::build_standalone_show_info();
        let show_y = ztshow_live_support::build_standalone_show_info();
        showmgr_live_support::set_show_id_counter(0xfffe);
        if register(show_x, false) != 1 {
            failures.push("wrap: register(X, false) at counter 0xfffe should return 1".to_string());
        }
        if get_from_memory::<u16>(show_x + 0x70) != 0 || ztshowmgr::registered_show_for_id(0) != Some(show_x) {
            failures.push("wrap: counter 0xfffe must assign id 0 (0xffff is never assigned) - field_0x70 and store[0] should both say so".to_string());
        }
        if showmgr_live_support::show_id_counter() != 0xffff {
            failures.push(format!(
                "wrap: the counter should now sit at 0xffff, got {:#06x}",
                showmgr_live_support::show_id_counter()
            ));
        }
        if register(show_y, false) != 0 || register(show_y, true) != 0 {
            failures.push("wrap: an id-0 register while key 0 is held must early-return 0, with or without force".to_string());
        }
        if showmgr_live_support::show_id_counter() != 0xffff {
            failures.push("wrap: the early returns must not consume the counter".to_string());
        }
        if unregister(0, show_x, false) != 1 {
            failures.push("wrap: unregister(0, X) should return 1".to_string());
        }
        showmgr_live_support::set_show_id_counter(0xffff);
        if register(show_y, false) != 1 {
            failures.push("wrap: register(Y, false) at counter 0xffff should return 1".to_string());
        }
        if showmgr_live_support::show_id_counter() != 0 {
            failures.push(format!(
                "wrap: the counter should have word-wrapped 0xffff -> 0, got {:#06x}",
                showmgr_live_support::show_id_counter()
            ));
        }
        if get_from_memory::<u16>(show_y + 0x70) != 0 || ztshowmgr::registered_show_for_id(0) != Some(show_y) {
            failures.push("wrap: the wrapped counter must assign id 0 - field_0x70 and store[0] should both say so".to_string());
        }
        if unregister(0, show_y, false) != 1 {
            failures.push("wrap: unregister(0, Y) should return 1".to_string());
        }
        showmgr_live_support::set_show_id_counter(counter_start);

        // Hygiene: the process-global store must be empty - nothing may leak into the rest of the
        // battery.
        let remaining = ztshowmgr::registered_show_count();
        if remaining != 0 {
            failures.push(format!("store should be empty after cleanup, has {} entries", remaining));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID` - `ztshowmgr-implementation-plan.md` stage 4 (the
    /// read cutover): drives the store-backed `getShowInfo`/`getScriptID` detours against a
    /// standalone, real-constructor `ZTShowMgr` (`this` stand-in - the readers ignore it) and two
    /// standalone `ZTShowInfo`s registered through the hooked `REGISTER_SHOW`. Since stage 9 the
    /// hooked writers maintain only the store, so that is the only copy that populates.
    ///
    /// Poles per probe:
    /// - the hooked addresses (the promoted live path): `GET_SHOW_INFO.hooked()` must round-trip
    ///   each registered show's pointer and return `0` for every absent/boundary id;
    ///   `GET_SCRIPT_ID.hooked()` must return the found show's `+0x8` assigned-script-id u16
    ///   zero-extended (a high-bit-set value pins the zero-extension - real vanilla leaves EAX's
    ///   upper half as register garbage there, which no caller observes), return `0` for a *found*
    ///   show whose `+0x8` is `0` (cross-checked against `GET_SHOW_INFO` still finding it - the
    ///   found-but-zero vs. miss ambiguity vanilla itself has), and `0` for a miss;
    /// - the real vanilla `getScriptID` through its own trampoline - half-real by construction: its
    ///   body reaches `getShowInfo` by raw address (`ZTShowMgr_getScriptID.asm`'s
    ///   `CALL ZTShowMgr::getShowInfo`), which is the detoured, store-backed reader, so it exercises
    ///   the real ABI glue and the real `+0x8` read on top of the store's answer. Compared through a
    ///   16-bit mask (see `cross_check_poles` - the real found path leaves the show-info pointer's
    ///   high bits in EAX's upper half, which the port's clean zero-extension contract doesn't
    ///   reproduce). (The sibling real-`getShowInfo` tree-walk pole this test carried during the
    ///   dual-write phase is gone with it: since stage 9 stopped the writers maintaining the tree,
    ///   that walk answers only raw-planted entries and can no longer agree with hooked
    ///   registrations - `ZTSHOWMGR_REGISTER_UNREGISTER_SHOW` now pins the tree's inertness
    ///   instead.)
    ///
    /// Also pins the cutover's one deliberate benign divergence: vanilla's `getShowInfo` faults on a
    /// null `this` (unguarded `[ECX+0x28]` read); the detour never touches `this`, so a
    /// null-manager lookup returns the store's answer instead.
    ///
    /// Teardown is leak-only (see `showmgr_live_support::allocate_uninitialized`'s doc comment); the
    /// store is drained through the hooked unregister path and asserted empty.
    fn run_ztshowmgr_get_show_info_get_script_id_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID";

        if ztshowmgr::registered_show_count() != 0 {
            let msg = "Rust registered-shows store should be empty when the test starts";
            error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
            }
            return true;
        }

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }

        /// Cross-pole agreement for every id the test has touched: the hooked reader must answer
        /// exactly what the store holds, and the real `getScriptID` must agree with the hooked one
        /// - through a 16-bit mask, because the real body's found path (`MOV %AX, word ptr
        /// [EAX+0x8]`, no `movzx`) leaves the upper EAX holding the upper half of the `getShowInfo`
        /// return (the show-info pointer's high bits), which the port's clean zero-extension
        /// contract deliberately does not reproduce.
        fn cross_check_poles(mgr_addr: u32, step: &str, touched_ids: &[u16], failures: &mut Vec<String>) {
            for id in touched_ids {
                let store = ztshowmgr::registered_show_for_id(*id).unwrap_or(0);
                let hooked = unsafe { ZTSHOWMGR_GET_SHOW_INFO.hooked()(mgr_addr as *const u32, *id) };
                if hooked != store {
                    failures.push(format!("{step}: id {id:#06x} - hooked={hooked:#010x}, store={store:#010x}"));
                }
                let hooked_script = unsafe { ZTSHOWMGR_GET_SCRIPT_ID.hooked()(mgr_addr as *const u32, *id) };
                let real_script = showmgr_live_support::call_real_get_script_id(mgr_addr as *const u32, *id) & 0xffff;
                if hooked_script != real_script {
                    failures.push(format!(
                        "{step}: id {id:#06x} - getScriptId hooked={hooked_script:#010x}, real(trampoline)={real_script:#010x}"
                    ));
                }
            }
        }

        let mut failures: Vec<String> = Vec::new();
        let mgr_addr = mgr as u32;

        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        let show_a = ztshow_live_support::build_standalone_show_info();
        let show_b = ztshow_live_support::build_standalone_show_info();

        // Preset id; show_b keeps its zero-init `field_0x70` (the counter-assignment case).
        const PRESET_ID_A: u16 = 0x1234;
        // High bit set on purpose: proves the `+0x8` u16 comes back zero-extended, not
        // sign-extended (vanilla's `MOV %AX` would leave the upper EAX as garbage).
        const SCRIPT_ID_A: u16 = 0x8abc;
        save_to_memory(show_a + 0x70, PRESET_ID_A);
        save_to_memory(show_a + 0x8, SCRIPT_ID_A);
        // show_b keeps its zero-init `+0x8` (the found-but-zero case).

        // All reads go through the hooked addresses (the promoted live path); the real-side poles
        // go through the stage-4 trampolines.
        let get_show_info = |id: u16| -> u32 { unsafe { ZTSHOWMGR_GET_SHOW_INFO.hooked()(mgr as *const u32, id) } };
        let get_script_id = |id: u16| -> u32 { unsafe { ZTSHOWMGR_GET_SCRIPT_ID.hooked()(mgr as *const u32, id) } };

        // Everything both poles and the store must agree on, seeded with the boundary probes and
        // A's preset id; B's fresh id gets pushed as the test discovers it.
        let mut touched_ids: Vec<u16> = vec![0x0000, 0xffff, 0x7fff, 0x8000, PRESET_ID_A];

        // Before any registration every pole reads empty.
        cross_check_poles(mgr_addr, "before any registration", &touched_ids, &mut failures);

        // Null-manager read: pins the cutover's one deliberate benign divergence - vanilla's own
        // body faults here (unguarded `[ECX+0x28]`); the detour ignores `this` and answers from the
        // store, which is empty for this id.
        if unsafe { ZTSHOWMGR_GET_SHOW_INFO.hooked()(std::ptr::null(), PRESET_ID_A) } != 0 {
            failures.push("hooked getShowInfo(null mgr, absent id) should return 0".to_string());
        }

        // Register A (preset id) and B (id-0 counter assignment) through the hooked REGISTER_SHOW.
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } != 1 {
            failures.push("register(A, false) with preset id should return 1".to_string());
        }
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } != 1 {
            failures.push("register(B, false) with id 0 should return 1".to_string());
        }
        let id_b = get_from_memory::<u16>(show_b + 0x70);
        if id_b == 0 || id_b == PRESET_ID_A {
            failures.push(format!("register(B, false) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
        }
        touched_ids.push(id_b);
        if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a) {
            failures.push("store[A's preset id] should be show A after register(A)".to_string());
        }
        if ztshowmgr::registered_show_for_id(id_b) != Some(show_b) {
            failures.push(format!("store[assigned id {id_b:#06x}] should be show B after register(B)"));
        }
        cross_check_poles(mgr_addr, "after register(A)+register(B)", &touched_ids, &mut failures);

        // Hooked reads: the cutover path round-trips each registration; the boundary probes stay
        // absent.
        if get_show_info(PRESET_ID_A) != show_a {
            failures.push(format!("hooked getShowInfo(A's id) should be {show_a:#010x}, got {:#010x}", get_show_info(PRESET_ID_A)));
        }
        if get_show_info(id_b) != show_b {
            failures.push(format!("hooked getShowInfo(B's id) should be {show_b:#010x}, got {:#010x}", get_show_info(id_b)));
        }
        for id in [0x0000u16, 0xffff, 0x7fff, 0x8000] {
            if get_show_info(id) != 0 {
                failures.push(format!("hooked getShowInfo(absent boundary id {id:#06x}) should return 0"));
            }
        }

        // Hooked getScriptID: A's +0x8 script id, zero-extended; B's is the found-but-zero case.
        if get_script_id(PRESET_ID_A) != SCRIPT_ID_A as u32 {
            failures.push(format!(
                "hooked getScriptId(A's id) should be {SCRIPT_ID_A:#06x} (zero-extended), got {:#010x}",
                get_script_id(PRESET_ID_A)
            ));
        }
        if get_script_id(id_b) != 0 {
            failures.push(format!("hooked getScriptId(B's id) should be 0 (B's +0x8 is zero), got {:#010x}", get_script_id(id_b)));
        }
        if get_show_info(id_b) != show_b {
            failures.push("hooked getShowInfo(B's id) should still find B - the zero script id must not read as a miss".to_string());
        }
        if get_script_id(0xffff) != 0 {
            failures.push("hooked getScriptId(absent id) should return 0".to_string());
        }

        // Cleanup: drain through the hooked unregister path (path C derives each show's own stale
        // `field_0x70`, which is exactly the key each insert used), then assert both the store and
        // the standalone vanilla tree drained.
        for show in [show_a, show_b] {
            unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
        }
        cross_check_poles(mgr_addr, "after cleanup", &touched_ids, &mut failures);
        let remaining = ztshowmgr::registered_show_count();
        if remaining != 0 {
            failures.push(format!("store should be empty after cleanup, has {} entries", remaining));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_ENTER_NEW_MONTH` - `ztshowmgr-implementation-plan.md` stage 5: the Rust walk must
    /// visit exactly the store's registered, non-null shows and run the real, untouched
    /// `ZTShowInfo::enterNewMonth` on each, while the real vanilla body (through the stage-5
    /// trampoline) walks the standalone vanilla tree planted alongside the store (stage 9 stopped
    /// the hooked writers maintaining the tree, so the tree-side registrations are made explicitly
    /// through the raw-body trampoline).
    ///
    /// Both poles run the same vanilla visitor, so per-show verification rests on its observable
    /// transform (`ZTShowInfo_enterNewMonth.c`): copy `+0x7c` into `+0x80` and zero `+0x7c`, copy
    /// `+0x94` into `+0x98` and zero `+0x94`, copy `+0x88` into `+0x8c`, then recompute `+0x88`
    /// through the opaque `FUN_0059e8f0` - fed the show's `field_0x70` id and, per the decompile's
    /// `unaff_SI`, whatever the walk left in `%ESI`, so genuinely call-dependent and unpredictible
    /// here - and add it into `+0x90`. Seeding `+0x90` to `0.0` makes that final sum exact in any
    /// precision (`F + 0.0` stores back bit-for-bit as the same float the `+0x88` store rounded,
    /// even through the real body's x87 float10 arithmetic), so one visit is fully characterized by
    /// the two copy-and-zero pairs, the `+0x8c` copy, and `+0x90 == +0x88`. A second visit (the real
    /// pole re-walking A/B) can't reuse the sum identity once `+0x90` is non-zero - its float10
    /// double-rounding isn't reproducible from Rust - so it asserts the six precision-free fields
    /// only.
    ///
    /// The differential set: A (preset id) and B (counter-assigned id) register through the hooked
    /// `REGISTER_SHOW` into the store and are then planted into the standalone vanilla tree through
    /// the raw `call_real_register_show` trampoline as well (hooked first, so B's store-assigned id
    /// is already in `field_0x70` when the raw body reads it and both stores key B identically);
    /// C (its own preset id) plants into the vanilla tree only, never the store - the Rust pole
    /// must leave it untouched (post-cutover its reads come from the store, not the tree: the
    /// property this pins), while the real pole must visit it. A never-registered control show must
    /// be untouched by both poles. Each show needs a real, empty, self-referential `0x18` header
    /// node at `+0x44`: the real callee starts its embedded pending-scripts walk at the header's
    /// *leftmost* pointer (`header+0x8`), which `build_standalone_show_info`'s zeroed embedded
    /// self-header leaves null - fine for `ztshow.rs`'s own root-based Rust walks, a null deref for
    /// the real body (same node shape the stage-3 test builds for `+0x38`'s clear path). All
    /// teardown is leak-only (`showmgr_live_support::allocate_uninitialized`'s doc comment); the
    /// store drains through the hooked unregister path, the tree-side plants (A, B, C) drain
    /// through the raw unregister trampoline, and a final both-poles pass over the emptied map must
    /// produce zero deltas - the walk's empty-map no-op on both sides.
    fn run_ztshowmgr_enter_new_month_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_ENTER_NEW_MONTH";

        if ztshowmgr::registered_show_count() != 0 {
            let msg = "Rust registered-shows store should be empty when the test starts";
            error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
            }
            return true;
        }

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }

        /// Builds a real, empty, self-referential `_Tree` header node and stores it at
        /// `show_info+0x44` - see the test's doc comment for why the stock standalone shape can't
        /// survive the real callee's leftmost-based walk. Leak-only, like everything else here.
        fn init_pending_script_header(show_info: u32) {
            let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
            unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
            save_to_memory(node + 0x8, node);
            save_to_memory(node + 0xc, node);
            save_to_memory(show_info + 0x44, node);
        }

        /// The seven fields the real `ZTShowInfo::enterNewMonth` transform touches (f32s compared
        /// as raw bits throughout).
        #[derive(Clone, Copy, PartialEq)]
        struct ShowAccumulators {
            f7c: u32,
            f80: u32,
            f94: u32,
            f98: u32,
            f88: u32,
            f8c: u32,
            f90: u32,
        }

        fn snap(show: u32) -> ShowAccumulators {
            ShowAccumulators {
                f7c: get_from_memory(show + 0x7c),
                f80: get_from_memory(show + 0x80),
                f94: get_from_memory(show + 0x94),
                f98: get_from_memory(show + 0x98),
                f88: get_from_memory(show + 0x88),
                f8c: get_from_memory(show + 0x8c),
                f90: get_from_memory(show + 0x90),
            }
        }

        fn push_mismatch(label: &str, detail: String, failures: &mut Vec<String>) {
            failures.push(format!("{label}: {detail}"));
        }

        /// One more application of the transform on top of `prev`. `check_sum` additionally asserts
        /// the `+0x90 += F` recompute via the seed-`0.0` identity - only valid while `prev.f90`
        /// was zero.
        fn assert_visited(label: &str, show: u32, prev: &ShowAccumulators, cur: &ShowAccumulators, check_sum: bool, failures: &mut Vec<String>) {
            if cur.f80 != prev.f7c {
                push_mismatch(label, format!("show {show:#010x} +0x80 should be old +0x7c ({:#010x}), got {:#010x}", prev.f7c, cur.f80), failures);
            }
            if cur.f7c != 0 {
                push_mismatch(label, format!("show {show:#010x} +0x7c should be zeroed, got {:#010x}", cur.f7c), failures);
            }
            if cur.f98 != prev.f94 {
                push_mismatch(label, format!("show {show:#010x} +0x98 should be old +0x94 ({:#010x}), got {:#010x}", prev.f94, cur.f98), failures);
            }
            if cur.f94 != 0 {
                push_mismatch(label, format!("show {show:#010x} +0x94 should be zeroed, got {:#010x}", cur.f94), failures);
            }
            if cur.f8c != prev.f88 {
                push_mismatch(label, format!("show {show:#010x} +0x8c should be old +0x88 ({:#010x}), got {:#010x}", prev.f88, cur.f8c), failures);
            }
            if check_sum && cur.f90 != cur.f88 {
                push_mismatch(label, format!("show {show:#010x} +0x90 should equal the recomputed +0x88 ({:#010x}) with the 0.0 seed, got {:#010x}", cur.f88, cur.f90), failures);
            }
        }

        fn assert_untouched(label: &str, show: u32, prev: &ShowAccumulators, cur: &ShowAccumulators, failures: &mut Vec<String>) {
            if *cur != *prev {
                push_mismatch(
                    label,
                    format!(
                        "show {show:#010x} must be untouched by this pole (+0x7c {:#010x}->{:#010x}, +0x80 {:#010x}->{:#010x}, +0x94 {:#010x}->{:#010x}, +0x98 {:#010x}->{:#010x}, +0x88 {:#010x}->{:#010x}, +0x8c {:#010x}->{:#010x}, +0x90 {:#010x}->{:#010x})",
                        prev.f7c, cur.f7c, prev.f80, cur.f80, prev.f94, cur.f94, prev.f98, cur.f98, prev.f88, cur.f88, prev.f8c, cur.f8c, prev.f90, cur.f90
                    ),
                    failures,
                );
            }
        }

        let mut failures: Vec<String> = Vec::new();

        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        // Distinct seeds per show so a copy from the wrong show can never pass; `+0x90`'s explicit
        // 0.0 documents the sum-identity premise the first-application assertions rest on.
        let show_a = ztshow_live_support::build_standalone_show_info();
        let show_b = ztshow_live_support::build_standalone_show_info();
        let show_c = ztshow_live_support::build_standalone_show_info();
        let control = ztshow_live_support::build_standalone_show_info();
        const SEEDS: [(u32, u32, f32); 4] = [(0x2a, 0x2b, 1.5), (0x3c, 0x3d, 2.5), (0x4e, 0x4f, 3.5), (0x5f, 0x60, 4.5)];
        for (show, (seed_7c, seed_94, seed_88)) in [show_a, show_b, show_c, control].into_iter().zip(SEEDS) {
            init_pending_script_header(show);
            save_to_memory(show + 0x7c, seed_7c);
            save_to_memory(show + 0x94, seed_94);
            save_to_memory(show + 0x88, seed_88.to_bits());
            save_to_memory(show + 0x90, 0.0f32.to_bits());
        }

        const PRESET_ID_A: u16 = 0x1234;
        const PRESET_ID_C: u16 = 0x4321;
        save_to_memory(show_a + 0x70, PRESET_ID_A);
        save_to_memory(show_c + 0x70, PRESET_ID_C);
        // show_b keeps its zero-init `field_0x70` (the counter-assignment case); its fresh id is
        // read back after registering.

        // A and B go through the hooked register (store) and are then planted into the vanilla tree
        // through the raw vanilla-body trampoline (hooked first: B's store-assigned id must be in
        // field_0x70 before the raw body reads it, so both stores key B identically); C plants into
        // the vanilla tree only - the store must never see it.
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } != 1 {
            failures.push("register(A, false) should return 1".to_string());
        }
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } != 1 {
            failures.push("register(B, false) should return 1".to_string());
        }
        if (showmgr_live_support::call_real_register_show(mgr as *const u32, show_a as *const u32, false) & 0xff) != 1 {
            failures.push("raw register(A, false) should return 1 (tree-side plant)".to_string());
        }
        if (showmgr_live_support::call_real_register_show(mgr as *const u32, show_b as *const u32, false) & 0xff) != 1 {
            failures.push("raw register(B, false) should return 1 (tree-side plant under B's already-assigned id)".to_string());
        }
        if (showmgr_live_support::call_real_register_show(mgr as *const u32, show_c as *const u32, false) & 0xff) != 1 {
            failures.push("raw register(C, false) should return 1".to_string());
        }
        let id_b = get_from_memory::<u16>(show_b + 0x70);
        if id_b == 0 || id_b == PRESET_ID_A || id_b == PRESET_ID_C {
            failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
        }
        if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a)
            || ztshowmgr::registered_show_for_id(id_b) != Some(show_b)
            || ztshowmgr::registered_show_for_id(PRESET_ID_C).is_some()
        {
            failures.push("store should hold exactly A and B after the three registrations".to_string());
        }

        // Rust pole: visits exactly the store's pair; C (tree-only) and the control stay untouched.
        let pre_rust = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
        unsafe { ZTSHOWMGR_ENTER_NEW_MONTH.hooked()(mgr as *const u32) };
        let post_rust = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
        assert_visited("rust pole", show_a, &pre_rust[0], &post_rust[0], true, &mut failures);
        assert_visited("rust pole", show_b, &pre_rust[1], &post_rust[1], true, &mut failures);
        assert_untouched("rust pole", show_c, &pre_rust[2], &post_rust[2], &mut failures);
        assert_untouched("rust pole", control, &pre_rust[3], &post_rust[3], &mut failures);

        // Real pole: the planted vanilla tree holds A+B+C, so A/B get a second application
        // (precision-free fields only - see the doc comment) and C its first (sum identity valid).
        let pre_real = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
        showmgr_live_support::call_real_enter_new_month(mgr as *const u32);
        let post_real = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
        assert_visited("real pole", show_a, &pre_real[0], &post_real[0], false, &mut failures);
        assert_visited("real pole", show_b, &pre_real[1], &post_real[1], false, &mut failures);
        assert_visited("real pole", show_c, &pre_real[2], &post_real[2], true, &mut failures);
        assert_untouched("real pole", control, &pre_real[3], &post_real[3], &mut failures);

        // Cleanup: drain A/B from the store through the hooked unregister and A/B/C from the tree
        // through the raw one (stage 9: the hooked path no longer touches the tree), then both
        // poles over the emptied map must be no-ops.
        for show in [show_a, show_b] {
            unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
        }
        for show in [show_a, show_b, show_c] {
            showmgr_live_support::call_real_unregister_show(mgr as *const u32, 0, show as *const u32, false);
        }
        let remaining = ztshowmgr::registered_show_count();
        if remaining != 0 {
            failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
        }
        let pre_empty = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
        unsafe { ZTSHOWMGR_ENTER_NEW_MONTH.hooked()(mgr as *const u32) };
        showmgr_live_support::call_real_enter_new_month(mgr as *const u32);
        let post_empty = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
        for (i, show) in [show_a, show_b, show_c, control].into_iter().enumerate() {
            assert_untouched("empty-map poles", show, &pre_empty[i], &post_empty[i], &mut failures);
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// The `ZTSHOWMGR_UPDATE` sentinel's visit log - recorded `this` values in call order.
    /// Module-level so the sentinel fn can reach it; only that test touches it, and the battery is
    /// single-threaded.
    static ZTSHOWMGR_UPDATE_VISITS: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

    /// Sentinel the `ZTSHOWMGR_UPDATE` test plants in each standalone show's fake vtable at slot
    /// `+0x20` - the exact slot both `ZTShowMgr::update` bodies virtually dispatch through - and
    /// which records the `this` it was called with.
    unsafe extern "thiscall" fn ztshowmgr_update_sentinel(this: *const u32) {
        ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().push(this as u32);
    }

    /// `ZTSHOWMGR_UPDATE` - stage 5's second walk (`ztshowmgr-implementation-plan.md`), verified
    /// through sentinel vtables rather than the real `ZTShowInfo::update` callee: that callee
    /// virtual-calls the show's own `listen`/`cleanupEvents` vtable slots plus a slot on the
    /// embedded `ZTShow` (`ZTShowInfo_update.c`), the whole event-list subsystem a zeroed
    /// standalone show can't back. The sentinel measures exactly the part stage 5 owns - the walk
    /// set, the ascending-key order, the size guard, and the dispatch convention - while the callee
    /// behind slot `+0x20` stays real vanilla in the live game, untouched.
    ///
    /// The differential set is the stage-5 standard: A (preset id) and B (counter-assigned id)
    /// register through the hooked `REGISTER_SHOW` (store) and are planted into the vanilla tree
    /// through the raw `call_real_register_show` trampoline as well (hooked first, so B's
    /// store-assigned id keys both stores identically - stage 9 stopped the hooked writers
    /// maintaining the tree), C (its own preset id) through the raw trampoline only (vanilla tree),
    /// plus a never-registered control. The Rust pole must record exactly A+B in ascending-id order
    /// (the `BTreeMap` iteration order substituting for vanilla's in-order walk) and nothing else;
    /// the real pole - the vanilla body through the stage-5 trampoline, walking the planted
    /// standalone tree - must record A+B+C in ascending-id order through the *same* sentinel, which
    /// also pins that vanilla really dispatches slot `+0x20` with `this` = the show pointer. With
    /// the store drained and the tree emptied, both poles must record nothing - the port's
    /// `!is_empty()` guard and vanilla's `mbr_0x2c > 0` guard both on an empty map. All teardown is
    /// leak-only.
    fn run_ztshowmgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_UPDATE";

        if ztshowmgr::registered_show_count() != 0 {
            let msg = "Rust registered-shows store should be empty when the test starts";
            error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
            }
            return true;
        }

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }

        /// Gives `show` a fake vtable whose only populated slot is `+0x20` (the dispatched one) -
        /// a walk that read any other slot would call null and crash the test process rather than
        /// pass. Leak-only, like everything else here.
        fn install_sentinel_vtable(show: u32) {
            let vtable = unsafe { standalone::OPERATOR_NEW.original()(0x24) } as u32;
            unsafe { std::ptr::write_bytes(vtable as *mut u8, 0, 0x24) };
            save_to_memory(vtable + 0x20, ztshowmgr_update_sentinel as *const () as usize as u32);
            save_to_memory(show, vtable);
        }

        let mut failures: Vec<String> = Vec::new();

        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        let show_a = ztshow_live_support::build_standalone_show_info();
        let show_b = ztshow_live_support::build_standalone_show_info();
        let show_c = ztshow_live_support::build_standalone_show_info();
        let control = ztshow_live_support::build_standalone_show_info();
        for show in [show_a, show_b, show_c, control] {
            install_sentinel_vtable(show);
        }

        const PRESET_ID_A: u16 = 0x1234;
        const PRESET_ID_C: u16 = 0x4321;
        save_to_memory(show_a + 0x70, PRESET_ID_A);
        save_to_memory(show_c + 0x70, PRESET_ID_C);
        // show_b keeps its zero-init `field_0x70` (the counter-assignment case).

        // A and B through the hooked register (store) plus the raw trampoline plant (vanilla tree,
        // hooked first so B's store-assigned id keys both stores identically); C through the raw
        // vanilla-body trampoline only (vanilla tree).
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } != 1 {
            failures.push("register(A, false) should return 1".to_string());
        }
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } != 1 {
            failures.push("register(B, false) should return 1".to_string());
        }
        if (showmgr_live_support::call_real_register_show(mgr as *const u32, show_a as *const u32, false) & 0xff) != 1 {
            failures.push("raw register(A, false) should return 1 (tree-side plant)".to_string());
        }
        if (showmgr_live_support::call_real_register_show(mgr as *const u32, show_b as *const u32, false) & 0xff) != 1 {
            failures.push("raw register(B, false) should return 1 (tree-side plant under B's already-assigned id)".to_string());
        }
        if (showmgr_live_support::call_real_register_show(mgr as *const u32, show_c as *const u32, false) & 0xff) != 1 {
            failures.push("raw register(C, false) should return 1 (the raw body only guarantees AL; the hooked path returns the port's cleaned 0/1)".to_string());
        }
        let id_b = get_from_memory::<u16>(show_b + 0x70);
        if id_b == 0 || id_b == PRESET_ID_A || id_b == PRESET_ID_C {
            failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
        }
        if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a)
            || ztshowmgr::registered_show_for_id(id_b) != Some(show_b)
            || ztshowmgr::registered_show_for_id(PRESET_ID_C).is_some()
        {
            failures.push("store should hold exactly A and B after the three registrations".to_string());
        }

        // Expected visit sequences, ascending by the id each show registered under (vanilla's
        // in-order walk order, which the BTreeMap's ascending-u16 iteration reproduces).
        let mut store_ids: Vec<(u16, u32)> = vec![(PRESET_ID_A, show_a), (id_b, show_b)];
        store_ids.sort_unstable();
        let mut tree_ids = store_ids.clone();
        tree_ids.push((PRESET_ID_C, show_c));
        tree_ids.sort_unstable();
        let pointers = |pairs: &[(u16, u32)]| -> Vec<u32> { pairs.iter().map(|(_, ptr)| *ptr).collect() };

        // Rust pole: exactly the store's pair, in ascending-id order - C (tree-only) and the
        // control must not appear.
        ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clear();
        unsafe { ZTSHOWMGR_UPDATE.hooked()(mgr as *const u32) };
        let rust_visits = ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clone();
        if rust_visits != pointers(&store_ids) {
            failures.push(format!(
                "rust pole visits should be exactly the store's registrations in ascending-id order ({:#010x?}), got {rust_visits:#010x?}",
                pointers(&store_ids)
            ));
        }

        // Real pole: the vanilla walk over the planted tree must dispatch slot +0x20 with
        // this = the show pointer for A+B+C, through the same sentinel.
        ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clear();
        showmgr_live_support::call_real_update(mgr as *const u32);
        let real_visits = ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clone();
        if real_visits != pointers(&tree_ids) {
            failures.push(format!(
                "real pole visits (vanilla walk dispatching slot +0x20) should be exactly the tree's registrations in ascending-id order ({:#010x?}), got {real_visits:#010x?}",
                pointers(&tree_ids)
            ));
        }

        // Cleanup: drain A/B from the store through the hooked unregister and A/B/C from the tree
        // through the raw one (stage 9: the hooked path no longer touches the tree), then both
        // poles over the emptied map must record nothing.
        for show in [show_a, show_b] {
            unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
        }
        for show in [show_a, show_b, show_c] {
            showmgr_live_support::call_real_unregister_show(mgr as *const u32, 0, show as *const u32, false);
        }
        let remaining = ztshowmgr::registered_show_count();
        if remaining != 0 {
            failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
        }
        ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clear();
        unsafe { ZTSHOWMGR_UPDATE.hooked()(mgr as *const u32) };
        showmgr_live_support::call_real_update(mgr as *const u32);
        let leftover_visits = ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clone();
        if !leftover_visits.is_empty() {
            failures.push(format!("both poles on the emptied map must visit nothing, got {leftover_visits:#010x?}"));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_SAVE_LOAD` - stage 6 (`ztshowmgr-implementation-plan.md`): `ZTShowMgr::save`/
    /// `load` wrap exactly two pieces - the embedded `ZTShowScriptMgr`'s own save/load (already
    /// `ztshowscriptmgr`'s Rust store, reached through the same direct `CALL` vanilla makes) and
    /// the 2-byte show-id-counter persistence - so the test drives those with `io_redirect`
    /// standing in for the real file. One script is seeded into the script store and a known
    /// counter value into **both** copies of the counter (stage 9 moved the live counter into the
    /// Rust store, but the real save/load bodies reached through the stage-6 trampolines still
    /// read/write the vanilla global in place - so the Rust poles exercise the store copy and the
    /// real poles the global, seeded identically so their streams still compare). Then:
    /// - the Rust save and the real body through the stage-6 trampoline must capture
    ///   byte-identical streams (both delegations reach the same Rust script-store save, so the
    ///   pole isolates each body's own tail: the Rust port's store read vs. vanilla's counter
    ///   global read in place, and the 2-bytes/count-1 write shape). The real pole's *return
    ///   byte* is deliberately not asserted: vanilla's body computes it via a full-EAX
    ///   `CMP %EAX, 1` on `WriteBytesToFile`'s return (`ZTShowMgr_save.asm`), and inside a
    ///   capture window that callee is `io_redirect`'s detour, whose Rust `bool` return defines
    ///   only `AL` - upper EAX is register garbage, so the compare reads as failure. A pure
    ///   test-harness artifact: the redirect path exists only inside capture windows, while the
    ///   passthrough path (and the un-hooked real function the live game calls) returns a
    ///   full-width 0/1. The load pole's real body returning 1 live-verifies the identical
    ///   `SETZ`/`AND` return tail - there the other redirected callee (`DEALLOCATE`) returns a
    ///   full-width `u32`, so vanilla's compare succeeds;
    /// - the stream must be exactly the script store's own payload plus the counter's 2 LE bytes
    ///   on the end - proven by replaying the prefix through `ztshowscriptmgr::load_mgr` (which
    ///   also must recover the seeded script) and comparing the tail against the seeded value;
    /// - replaying the full stream at version 0x100 through both poles must restore the script
    ///   store and the counter (the store copy through the hooked pole, the global through the
    ///   real one) - and the hooked pole's restored store counter must **continue**: a subsequent
    ///   id-0 register through the hooked `REGISTER_SHOW` must assign exactly
    ///   `SEEDED_COUNTER + 1`, pinning the register-after-load counter continuity stage 9 created
    ///   (one owner, both consumers Rust-side now);
    /// - replaying at version 0x60 (at/under the gate) must restore the scripts but leave the
    ///   counter untouched, through both poles (each in its own copy);
    /// - replaying only the store payload (counter bytes stripped) at version 0x100 must fail on
    ///   `ZTShowMgr`'s own counter read and return failure with the counter untouched - the
    ///   scriptmgr's own trailing counter inside that payload satisfies its loader, so the
    ///   failure is specifically the outer read.
    /// Both counter copies are saved and restored around the whole test; the script store
    /// is reset before and after (successful replays also restore its own persisted counter -
    /// reset away again), and the registered-shows store must still be empty at the end.
    fn run_ztshowmgr_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_SAVE_LOAD";
        const CURRENT_VERSION: u32 = 0x100;
        const GATED_VERSION: u32 = 0x60;
        const SEEDED_COUNTER: u16 = 0x0ABC;

        if ztshowmgr::registered_show_count() != 0 {
            let msg = "Rust registered-shows store should be empty when the test starts";
            error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
            }
            return true;
        }

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }
        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        let counter_addr = showmgr_live_support::show_id_counter_addr();
        let original_counter = get_from_memory::<u16>(counter_addr);
        let original_store_counter = showmgr_live_support::show_id_counter();

        let mut failures: Vec<String> = Vec::new();
        let dummy_file: u32 = 0;
        let file_ptr = &dummy_file as *const u32;

        // Seed: one script in the script store, a known show-id counter in both copies (the Rust
        // poles read the store's, the real bodies the vanilla global).
        ztshowscriptmgr::live_support::reset_state();
        let script_a = make_registered_show_script(11, 101);
        save_to_memory(counter_addr, SEEDED_COUNTER);
        showmgr_live_support::set_show_id_counter(SEEDED_COUNTER);

        // Rust save, then the real body's save - captures must be byte-identical. The real
        // pole's return byte is deliberately unread (harness artifact, see this test's doc
        // comment): vanilla's full-EAX compare against io_redirect's bool-returning write
        // detour cannot succeed inside a capture window.
        io_redirect::begin_capture();
        let rust_save_ret = unsafe { ZTSHOWMGR_SAVE.hooked()(mgr as *const u32, file_ptr as *const i8) };
        let rust_bytes = io_redirect::end_capture();
        io_redirect::begin_capture();
        let _real_save_ret = showmgr_live_support::call_real_save(mgr as *const u32, file_ptr as *const i8);
        let real_bytes = io_redirect::end_capture();
        if rust_save_ret != 1 {
            failures.push(format!("hooked save should return 1, got {rust_save_ret:#010x}"));
        }
        if real_bytes != rust_bytes {
            failures.push(format!(
                "real and rust save captures must be byte-identical ({} vs {} bytes)",
                real_bytes.len(),
                rust_bytes.len()
            ));
        }

        // Shape: the stream is the script store's payload (prefix decodes via load_mgr and
        // recovers the seeded script) plus the counter's 2 LE bytes on the end.
        if rust_bytes.len() < 6 {
            failures.push(format!("save capture implausibly small ({} bytes), expected script payload + 2 counter bytes", rust_bytes.len()));
        } else {
            let (payload, counter_tail) = rust_bytes.split_at(rust_bytes.len() - 2);
            if counter_tail != SEEDED_COUNTER.to_le_bytes() {
                failures.push(format!(
                    "save capture must end with the counter's 2 LE bytes ({SEEDED_COUNTER:#06x}), got {counter_tail:02x?}"
                ));
            }
            ztshowscriptmgr::live_support::reset_state();
            io_redirect::begin_replay(payload.to_vec());
            let payload_ok = ztshowscriptmgr::load_mgr(file_ptr, CURRENT_VERSION);
            io_redirect::end_replay();
            if !payload_ok || !ztshowscriptmgr::script_exists_by_id(script_a) {
                failures.push("save capture's prefix should decode as the script store's payload containing the seeded script".to_string());
            }
        }

        // Full-stream load, version over the gate: scripts + counter restored. Rust pole first
        // (its counter lands in the store), then the real body through the trampoline (its counter
        // lands in the vanilla global). Both copies are clobbered to distinct sentinels before
        // each pole so a no-op restore is detectable on the side that pole owns.
        ztshowscriptmgr::live_support::reset_state();
        save_to_memory(counter_addr, 1_u16);
        showmgr_live_support::set_show_id_counter(1);
        io_redirect::begin_replay(rust_bytes.clone());
        let rust_load_ret = unsafe { ZTSHOWMGR_LOAD.hooked()(mgr as *const u32, file_ptr, CURRENT_VERSION) };
        io_redirect::end_replay();
        if rust_load_ret != 1 {
            failures.push(format!("hooked load should return 1, got {rust_load_ret:#010x}"));
        }
        if !ztshowscriptmgr::script_exists_by_id(script_a) {
            failures.push("hooked load should have restored the seeded script".to_string());
        }
        if showmgr_live_support::show_id_counter() != SEEDED_COUNTER {
            failures.push(format!(
                "hooked load should have restored the store counter to {SEEDED_COUNTER:#06x}, got {:#06x}",
                showmgr_live_support::show_id_counter()
            ));
        }

        // Register-after-load counter continuity: the restored store counter is the live one, so
        // the next id-0 register must continue from it - exactly SEEDED_COUNTER + 1 (no wrap at
        // this seed).
        let continuity_show = ztshow_live_support::build_standalone_show_info();
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, continuity_show as *const u32, false) } != 1 {
            failures.push("register after load should return 1".to_string());
        }
        if get_from_memory::<u16>(continuity_show + 0x70) != SEEDED_COUNTER + 1 {
            failures.push(format!(
                "register after load should assign exactly {:#06x} (the restored counter + 1), got {:#06x}",
                SEEDED_COUNTER + 1,
                get_from_memory::<u16>(continuity_show + 0x70)
            ));
        }
        if unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, continuity_show as *const u32, false) } != 1 {
            failures.push("unregistering the continuity show should return 1".to_string());
        }

        ztshowscriptmgr::live_support::reset_state();
        save_to_memory(counter_addr, 2_u16);
        showmgr_live_support::set_show_id_counter(2);
        io_redirect::begin_replay(real_bytes.clone());
        let real_load_ret = showmgr_live_support::call_real_load(mgr as *const u32, file_ptr, CURRENT_VERSION);
        io_redirect::end_replay();
        if (real_load_ret & 0xff) != 1 {
            failures.push(format!("real load should return 1 in its low byte, got {real_load_ret:#010x}"));
        }
        if !ztshowscriptmgr::script_exists_by_id(script_a) {
            failures.push("real load should have restored the seeded script".to_string());
        }
        if get_from_memory::<u16>(counter_addr) != SEEDED_COUNTER {
            failures.push(format!(
                "real load should have restored the counter global to {SEEDED_COUNTER:#06x}, got {:#06x}",
                get_from_memory::<u16>(counter_addr)
            ));
        }

        // Version gate: at/under 0x60 the scripts still load but neither pole touches the counter.
        ztshowscriptmgr::live_support::reset_state();
        save_to_memory(counter_addr, 3_u16);
        showmgr_live_support::set_show_id_counter(3);
        io_redirect::begin_replay(rust_bytes.clone());
        let gated_ret = unsafe { ZTSHOWMGR_LOAD.hooked()(mgr as *const u32, file_ptr, GATED_VERSION) };
        io_redirect::end_replay();
        if gated_ret != 1 {
            failures.push(format!("hooked load at version 0x60 should still return 1, got {gated_ret:#010x}"));
        }
        if !ztshowscriptmgr::script_exists_by_id(script_a) {
            failures.push("hooked load at version 0x60 should still have restored the seeded script".to_string());
        }
        if showmgr_live_support::show_id_counter() != 3 {
            failures.push("hooked load at version 0x60 must not touch the store counter (gate not passed)".to_string());
        }

        ztshowscriptmgr::live_support::reset_state();
        save_to_memory(counter_addr, 4_u16);
        showmgr_live_support::set_show_id_counter(4);
        io_redirect::begin_replay(real_bytes.clone());
        let real_gated_ret = showmgr_live_support::call_real_load(mgr as *const u32, file_ptr, GATED_VERSION);
        io_redirect::end_replay();
        if (real_gated_ret & 0xff) != 1 {
            failures.push(format!("real load at version 0x60 should still return 1 in its low byte, got {real_gated_ret:#010x}"));
        }
        if get_from_memory::<u16>(counter_addr) != 4 {
            failures.push("real load at version 0x60 must not touch the counter global (gate not passed)".to_string());
        }

        // Short read: the store payload alone (its own trailing counter satisfies the scriptmgr's
        // loader) leaves nothing for ZTShowMgr's own 2-byte read - both poles must report
        // failure and leave their counter copy untouched.
        let stripped = rust_bytes[..rust_bytes.len() - 2].to_vec();
        ztshowscriptmgr::live_support::reset_state();
        save_to_memory(counter_addr, 5_u16);
        showmgr_live_support::set_show_id_counter(5);
        io_redirect::begin_replay(stripped.clone());
        let short_ret = unsafe { ZTSHOWMGR_LOAD.hooked()(mgr as *const u32, file_ptr, CURRENT_VERSION) };
        io_redirect::end_replay();
        if short_ret != 0 {
            failures.push(format!("hooked load on a stream missing its counter bytes should return 0, got {short_ret:#010x}"));
        }
        if showmgr_live_support::show_id_counter() != 5 {
            failures.push("a failed counter read must leave the store counter untouched".to_string());
        }

        ztshowscriptmgr::live_support::reset_state();
        save_to_memory(counter_addr, 6_u16);
        showmgr_live_support::set_show_id_counter(6);
        io_redirect::begin_replay(stripped);
        let real_short_ret = showmgr_live_support::call_real_load(mgr as *const u32, file_ptr, CURRENT_VERSION);
        io_redirect::end_replay();
        if (real_short_ret & 0xff) != 0 {
            failures.push(format!("real load on a stream missing its counter bytes should return 0 in its low byte, got {real_short_ret:#010x}"));
        }
        if get_from_memory::<u16>(counter_addr) != 6 {
            failures.push("a failed real counter read must leave the counter global untouched".to_string());
        }

        // Hygiene: reset the script store, put both counter copies back, and confirm the
        // registered-shows store was never touched.
        ztshowscriptmgr::live_support::reset_state();
        save_to_memory(counter_addr, original_counter);
        showmgr_live_support::set_show_id_counter(original_store_counter);
        let remaining = ztshowmgr::registered_show_count();
        if remaining != 0 {
            failures.push(format!("registered-shows store should still be empty, has {remaining} entries"));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_IS_DOING_SHOW` - stage 7 (`ztshowmgr-implementation-plan.md`): `ZTShowMgr::
    /// isDoingShow` composes the store-backed `getShowInfo` lookup (stage 4) with one real,
    /// untouched `ZTShow::getShowScriptState` walk over the found show's embedded `ZTShow`, so
    /// both poles read the same registrations out of the same store - the real body's internal
    /// `getShowInfo` `CALL` lands in the stage-4 detour - and the differential is the glue each
    /// pole owns: the port's clean 0/1 (macOS's body normalizes to the same full-width predicate)
    /// against vanilla's `SETNZ %AL`-only return plus the `LEA %ECX, [EAX + 0x4]` embedded-show
    /// hand-off.
    ///
    /// The state side is built by hand on a standalone show: the embedded `ZTShow`'s script-state
    /// map is the self-referential header object at `show_info+0x38` (the same header shape the
    /// stage-3 test builds for `ZTShowState::clear`), with one zero-padded `0x18` node hung off
    /// its root slot at `+0x3c` - key (the unit id) at node `+0x10`, a non-null value at `+0x14`
    /// (the real callee returns `[node+0x14]` without dereferencing it, so an opaque non-null
    /// buffer stands in for the `ZTShowScriptState`). `AI_cls_0x404fd6::find`'s key compare is a
    /// full 32-bit `CMP dword ptr [EAX+0x10]` (`AI_cls_0x404fd6_find.asm`), which the zeroed node
    /// padding satisfies and which a probe crossing bit 16 must NOT match - both poles answer
    /// through the same real walk, so that probe pins the effective key width rather than
    /// differentiating the poles. All teardown is leak-only; the store drains through the hooked
    /// unregister path and a final both-poles pass confirms the registration is really gone.
    fn run_ztshowmgr_is_doing_show_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_IS_DOING_SHOW";

        if ztshowmgr::registered_show_count() != 0 {
            let msg = "Rust registered-shows store should be empty when the test starts";
            error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
            }
            return true;
        }

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }

        /// Both poles over one (unit, show) probe. The Rust pole must return exactly `expected`
        /// (the port's clean 0/1); the real body defines only its `AL` byte (upper EAX holds the
        /// state pointer's high bits on a hit), so its return is compared through the low-byte
        /// mask.
        fn check(label: &str, mgr: *mut ZTShowMgr, unit_id: u32, show_id: u16, expected: u32, failures: &mut Vec<String>) {
            let rust_ret = unsafe { ZTSHOWMGR_IS_DOING_SHOW.hooked()(mgr as *const u32, unit_id, show_id) };
            if rust_ret != expected {
                failures.push(format!("{label}: rust pole should return {expected}, got {rust_ret:#010x}"));
            }
            let real_ret = showmgr_live_support::call_real_is_doing_show(mgr as *const u32, unit_id, show_id) & 0xff;
            if real_ret != expected {
                failures.push(format!("{label}: real pole should return {expected} (AL-masked), got {real_ret:#010x}"));
            }
        }

        let mut failures: Vec<String> = Vec::new();

        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        // show_a carries the seeded state (unit UNIT_A -> a non-null script-state pointer) inside
        // its embedded ZTShow's script-state map; show_b stays stateless (empty self-referential
        // header only). Leak-only allocations throughout.
        const UNIT_A: u32 = 0x2a;
        let show_a = ztshow_live_support::build_standalone_show_info();
        let show_b = ztshow_live_support::build_standalone_show_info();
        for show in [show_a, show_b] {
            // The map object at show_info+0x38 doubles as its own empty header (self-sentinel) -
            // a zeroed +0x38 would leave `find`'s header load null and crash its root read.
            save_to_memory(show + 0x38, show + 0x38);
        }
        let fake_state = unsafe { standalone::OPERATOR_NEW.original()(0x20) } as u32;
        unsafe { std::ptr::write_bytes(fake_state as *mut u8, 0, 0x20) };
        let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
        unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
        save_to_memory(node + 0x4, show_a + 0x38); // parent = header (hygiene; find never reads it)
        save_to_memory(node + 0x10, UNIT_A); // key: the unit id; node stays zero-padded at +0x12
        save_to_memory(node + 0x14, fake_state); // value: only null-tested by the real callee
        save_to_memory(show_a + 0x3c, node); // the header's root slot

        const PRESET_ID_A: u16 = 0x1234;
        save_to_memory(show_a + 0x70, PRESET_ID_A);
        // show_b keeps its zero-init `field_0x70` (the counter-assignment case).
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } != 1 {
            failures.push("register(A, false) should return 1".to_string());
        }
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } != 1 {
            failures.push("register(B, false) should return 1".to_string());
        }
        let id_b = get_from_memory::<u16>(show_b + 0x70);
        if id_b == 0 || id_b == PRESET_ID_A {
            failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
        }

        // Unregistered probes: both poles miss on the store-backed lookup.
        for probe in [0x0000u16, 0x8000, 0xffff] {
            check(&format!("unregistered id {probe:#06x}"), mgr, UNIT_A, probe, 0, &mut failures);
        }
        // The seeded hit: UNIT_A is doing show A through both poles.
        check("seeded state hit", mgr, UNIT_A, PRESET_ID_A, 1, &mut failures);
        // Misses against the seeded show: unknown unit, the unit-id-0 boundary, and a key that
        // matches only below bit 16 (the 32-bit key-compare pin).
        check("unknown unit on seeded show", mgr, UNIT_A + 1, PRESET_ID_A, 0, &mut failures);
        check("unit id 0 on seeded show", mgr, 0, PRESET_ID_A, 0, &mut failures);
        check("key differing above bit 16", mgr, UNIT_A | 0x1_0000, PRESET_ID_A, 0, &mut failures);
        // Registered but stateless: the walk runs over an empty map and misses for every unit.
        check("stateless show, seeded unit", mgr, UNIT_A, id_b, 0, &mut failures);
        check("stateless show, unit id 0", mgr, 0, id_b, 0, &mut failures);

        // Cleanup: drain both through the hooked unregister (clear=false - the clear path is the
        // stage-3 tests' concern), then the hit probe must miss through both poles.
        for show in [show_a, show_b] {
            unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
        }
        let remaining = ztshowmgr::registered_show_count();
        if remaining != 0 {
            failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
        }
        check("unregistered after cleanup", mgr, UNIT_A, PRESET_ID_A, 0, &mut failures);

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_IS_SHOW_SCRIPT_DONE` - stage 8 (`ztshowmgr-implementation-plan.md`): `ZTShowMgr::
    /// isShowScriptDone` is structurally stage 7's sibling - the same store-backed `getShowInfo`
    /// lookup (whose stage-4 detour the real body's raw `CALL` lands in) and the same real
    /// `ZTShow::getShowScriptState` walk - so both poles again read the same registrations out of
    /// the same store, and the differential is the glue each pole owns: the port's clean
    /// zero-extended done byte against vanilla's `MOV %AL, byte ptr [EAX + 0x13]` read plus its
    /// AL-only return.
    ///
    /// The state side is built exactly like the stage-7 test's (self-referential header at
    /// `show_info+0x38`, one zero-padded `0x18` node hung off its root slot, key at `+0x10`,
    /// non-null value at `+0x14`) with one extra dimension: unlike `isDoingShow`, stage 8
    /// *dereferences* the state pointer, so the stand-in's `+0x13` byte is meaningful. It is swept
    /// through `0x00`/`0x37`/`0xff`: the `0x37` probe pins that the port returns the raw byte (a
    /// wrongly normalized 0/1 would return `1` there, and the real pole's AL-masked compare would
    /// catch the mirror-image mistake), and `0x00` pins that a found state with a zero done byte is
    /// indistinguishable from a miss through the observable return, exactly like vanilla. All
    /// teardown is leak-only; the store drains through the hooked unregister path and a final
    /// both-poles pass confirms the registration is really gone.
    fn run_ztshowmgr_is_show_script_done_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_IS_SHOW_SCRIPT_DONE";

        if ztshowmgr::registered_show_count() != 0 {
            let msg = "Rust registered-shows store should be empty when the test starts";
            error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
            }
            return true;
        }

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }

        /// Both poles over one (script, show) probe. The Rust pole must return exactly `expected`
        /// (the port's clean zero-extended byte); the real body defines only its `AL` byte (upper
        /// EAX holds the state pointer's high bits on a hit), so its return is compared through the
        /// low-byte mask.
        fn check(label: &str, mgr: *mut ZTShowMgr, script_id: u32, show_id: u16, expected: u32, failures: &mut Vec<String>) {
            let rust_ret = unsafe { ZTSHOWMGR_IS_SHOW_SCRIPT_DONE.hooked()(mgr as *const u32, script_id, show_id) };
            if rust_ret != expected {
                failures.push(format!("{label}: rust pole should return {expected}, got {rust_ret:#010x}"));
            }
            let real_ret = showmgr_live_support::call_real_is_show_script_done(mgr as *const u32, script_id, show_id) & 0xff;
            if real_ret != expected {
                failures.push(format!("{label}: real pole should return {expected} (AL-masked), got {real_ret:#010x}"));
            }
        }

        let mut failures: Vec<String> = Vec::new();

        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        // show_a carries the seeded state (unit UNIT_A -> a script-state stand-in whose +0x13 done
        // byte is swept below) inside its embedded ZTShow's script-state map; show_b stays
        // stateless (empty self-referential header only). Leak-only allocations throughout.
        const UNIT_A: u32 = 0x2a;
        let show_a = ztshow_live_support::build_standalone_show_info();
        let show_b = ztshow_live_support::build_standalone_show_info();
        for show in [show_a, show_b] {
            // The map object at show_info+0x38 doubles as its own empty header (self-sentinel) -
            // a zeroed +0x38 would leave `find`'s header load null and crash its root read.
            save_to_memory(show + 0x38, show + 0x38);
        }
        let fake_state = unsafe { standalone::OPERATOR_NEW.original()(0x20) } as u32;
        unsafe { std::ptr::write_bytes(fake_state as *mut u8, 0, 0x20) };
        let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
        unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
        save_to_memory(node + 0x4, show_a + 0x38); // parent = header (hygiene; find never reads it)
        save_to_memory(node + 0x10, UNIT_A); // key: the unit id; node stays zero-padded at +0x12
        save_to_memory(node + 0x14, fake_state); // value: dereferenced at +0x13 by both poles
        save_to_memory(show_a + 0x3c, node); // the header's root slot

        const PRESET_ID_A: u16 = 0x1234;
        save_to_memory(show_a + 0x70, PRESET_ID_A);
        // show_b keeps its zero-init `field_0x70` (the counter-assignment case).
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } != 1 {
            failures.push("register(A, false) should return 1".to_string());
        }
        if unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } != 1 {
            failures.push("register(B, false) should return 1".to_string());
        }
        let id_b = get_from_memory::<u16>(show_b + 0x70);
        if id_b == 0 || id_b == PRESET_ID_A {
            failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
        }

        // Unregistered probes: both poles miss on the store-backed lookup.
        for probe in [0x0000u16, 0x8000, 0xffff] {
            check(&format!("unregistered id {probe:#06x}"), mgr, UNIT_A, probe, 0, &mut failures);
        }
        // The seeded hit, with the done byte swept: the raw byte must come back through both poles
        // unchanged (0x37 would expose a wrongly normalized 0/1 on either side).
        for byte in [0x00u8, 0x37, 0xff] {
            save_to_memory(fake_state + 0x13, byte);
            check(&format!("done byte {byte:#04x}"), mgr, UNIT_A, PRESET_ID_A, byte as u32, &mut failures);
        }
        // Misses against the seeded show: unknown unit, the unit-id-0 boundary, and a key that
        // matches only below bit 16 (the 32-bit key-compare pin).
        check("unknown unit on seeded show", mgr, UNIT_A + 1, PRESET_ID_A, 0, &mut failures);
        check("unit id 0 on seeded show", mgr, 0, PRESET_ID_A, 0, &mut failures);
        check("key differing above bit 16", mgr, UNIT_A | 0x1_0000, PRESET_ID_A, 0, &mut failures);
        // Registered but stateless: the walk runs over an empty map and misses for every unit.
        check("stateless show, seeded unit", mgr, UNIT_A, id_b, 0, &mut failures);
        check("stateless show, unit id 0", mgr, 0, id_b, 0, &mut failures);

        // Cleanup: drain both through the hooked unregister (clear=false - the clear path is the
        // stage-3 tests' concern), then the hit probe must miss through both poles.
        for show in [show_a, show_b] {
            unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
        }
        let remaining = ztshowmgr::registered_show_count();
        if remaining != 0 {
            failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
        }
        check("unregistered after cleanup", mgr, UNIT_A, PRESET_ID_A, 0, &mut failures);

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOWMGR_REGISTER_UNREGISTER_GET_SCRIPT` - review follow-up, not part of the original
    /// `ztshowmgr-implementation-plan.md`: `ZTShowMgr::registerScript`/`unregisterScript`/`getScript`
    /// (the outer `ztshowmgr::REGISTER_SCRIPT`/`UNREGISTER_SCRIPT`/`GET_SCRIPT`,
    /// `0x0046e89c`/`0x00473120`/`0x005a25b7`) are confirmed via `.asm` (`ADD ECX,0x34` + tail `CALL`)
    /// to be genuine, un-detoured delegations into the embedded `ZTShowScriptMgr` sub-object's own
    /// already-detoured addresses (`ztshowscriptmgr::{REGISTER_SCRIPT, UNREGISTER_SCRIPT, GET_SCRIPT}`,
    /// exercised directly elsewhere in this battery). No prior live test drove these *outer*
    /// addresses, so this closes that gap. The fourth delegation-shaped method,
    /// `getShowScriptItems`, is deliberately excluded: its callee ignores the passed sub-object
    /// pointer entirely and instead reads `GLOBAL_ZTWorldMgr`/`ZTUnitType::getTrickList`, so there is
    /// no Rust-owned behavior for it to reach (see the implementation plan doc's "Composition with
    /// ZTShowScriptMgr" section).
    ///
    /// These three outer addresses are never detoured anywhere in the repo, so `.original()` on them
    /// is always safe here and correctly routes through vanilla into the (hooked) inner address.
    fn run_ztshowmgr_register_unregister_get_script_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_REGISTER_UNREGISTER_GET_SCRIPT";

        ztshowscriptmgr::live_support::reset_state();

        let mgr = showmgr_live_support::allocate_uninitialized();
        if mgr.is_null() {
            error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
            }
            return true;
        }

        let mut failures: Vec<String> = Vec::new();

        unsafe {
            std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
            ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
        }

        const SCRIPT_TYPE: u32 = 0x1357;
        let alloc = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
        let script_ptr = unsafe { ZTSHOWSCRIPT_CONSTRUCTOR.original()(alloc as *const u32, SCRIPT_TYPE, false) } as u32;
        if script_ptr == 0 {
            failures.push("ZTShowScript CONSTRUCTOR returned null".to_string());
        }

        // Null-show register: rejected before it ever reaches the embedded sub-object.
        if unsafe { ZTSHOWMGR_REGISTER_SCRIPT.original()(mgr as *const u32, std::ptr::null()) } != 0 {
            failures.push("REGISTER_SCRIPT(mgr, null) should return 0".to_string());
        }

        if unsafe { ZTSHOWMGR_REGISTER_SCRIPT.original()(mgr as *const u32, script_ptr as *const u32) } != 1 {
            failures.push("REGISTER_SCRIPT(mgr, script) should return 1".to_string());
        }

        let assigned_id = get_from_memory::<u16>(script_ptr + 0x4);
        if !ztshowscriptmgr::script_exists_by_id(assigned_id) {
            failures.push(format!("assigned id {assigned_id:#06x} should exist in the ztshowscriptmgr store after REGISTER_SCRIPT"));
        }

        let outer_handle = unsafe { ZTSHOWMGR_GET_SCRIPT.original()(mgr as *const u32, assigned_id) };
        let inner_handle = ztshowscriptmgr::get_script(assigned_id);
        if outer_handle == 0 || outer_handle != inner_handle {
            failures.push(format!(
                "GET_SCRIPT(mgr, {assigned_id:#06x}) should match ztshowscriptmgr::get_script and be non-null, got outer={outer_handle:#010x} inner={inner_handle:#010x}"
            ));
        }

        if unsafe { ZTSHOWMGR_UNREGISTER_SCRIPT.original()(mgr as *const u32, script_ptr as *const u32) } != 1 {
            failures.push("UNREGISTER_SCRIPT(mgr, script) should return 1".to_string());
        }
        if ztshowscriptmgr::script_exists_by_id(assigned_id) {
            failures.push(format!("id {assigned_id:#06x} should no longer exist in the store after UNREGISTER_SCRIPT"));
        }
        if unsafe { ZTSHOWMGR_GET_SCRIPT.original()(mgr as *const u32, assigned_id) } != 0 {
            failures.push(format!("GET_SCRIPT(mgr, {assigned_id:#06x}) should return 0 after unregister"));
        }

        if unsafe { ZTSHOWMGR_UNREGISTER_SCRIPT.original()(mgr as *const u32, script_ptr as *const u32) } != 0 {
            failures.push("double UNREGISTER_SCRIPT(mgr, script) should return 0".to_string());
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }

    /// `ZTSHOW_GET_SHOW_SCRIPT_STATE` - review follow-up: diffs the new pure Rust
    /// [`ztshow::get_show_script_state`] reader against the real, never-hooked
    /// `ztshow::GET_SHOW_SCRIPT_STATE.original()` (`0x0059eb99`) over synthetic `Box::leak`'d
    /// fixtures (read-only on both sides, so no `standalone::OPERATOR_NEW` is needed). Covers an
    /// empty tree (self-referential header, per the same trick `ZTSHOWMGR_IS_SHOW_SCRIPT_DONE`'s
    /// fixture uses at `show_info+0x38`), a single node (exact hit, near misses either side, and a
    /// probe crossing bit 16 that pins the 32-bit-width key compare - a wrongly 16-bit-masked
    /// implementation would false-hit there), and a 3-node tree (root + left + right children, exact
    /// hits on all three plus an in-between miss - pinning this is exact-match `find`, not a
    /// nearest/lower-bound return).
    fn run_ztshow_get_show_script_state_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOW_GET_SHOW_SCRIPT_STATE";
        let mut failures: Vec<String> = Vec::new();

        fn leaked_bytes(size: usize) -> u32 {
            let buf: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
            buf.as_mut_ptr() as u32
        }

        fn make_ztshow(header: u32) -> u32 {
            let addr = leaked_bytes(0x38);
            save_to_memory(addr + 0x34, header);
            addr
        }

        fn make_header(root: u32) -> u32 {
            let addr = leaked_bytes(0x18);
            save_to_memory(addr + 0x4, root);
            addr
        }

        fn make_node(key: u32, value: u32, left: u32, right: u32) -> u32 {
            let addr = leaked_bytes(0x18);
            save_to_memory(addr + 0x8, left);
            save_to_memory(addr + 0xc, right);
            save_to_memory(addr + 0x10, key);
            save_to_memory(addr + 0x14, value);
            addr
        }

        fn check(label: &str, ztshow_ptr: u32, key: u32, expected: u32, failures: &mut Vec<String>) {
            let rust_ret = ztshow::get_show_script_state(ztshow_ptr, key);
            if rust_ret != expected {
                failures.push(format!("{label}: rust pole should return {expected:#010x}, got {rust_ret:#010x}"));
            }
            let real_ret = unsafe { GET_SHOW_SCRIPT_STATE.original()(ztshow_ptr as *const u32, key) };
            if real_ret != expected {
                failures.push(format!("{label}: real pole should return {expected:#010x}, got {real_ret:#010x}"));
            }
        }

        // Empty tree: the header field doubles as the header node itself (self-referential), so its
        // own root slot (at header+4, i.e. ztshow+0x38) is naturally 0 out of the zeroed allocation.
        let empty_show = leaked_bytes(0x40);
        save_to_memory(empty_show + 0x34, empty_show + 0x34);
        check("empty tree, key 0", empty_show, 0, 0, &mut failures);
        check("empty tree, key 0xffffffff", empty_show, 0xffff_ffff, 0, &mut failures);

        // Single node, key chosen above bit 16 to pin the 32-bit-width compare.
        const OPAQUE_VALUE: u32 = 0xdead_beef;
        let node = make_node(0x1_0007, OPAQUE_VALUE, 0, 0);
        let header = make_header(node);
        let show = make_ztshow(header);
        check("single node exact hit", show, 0x1_0007, OPAQUE_VALUE, &mut failures);
        check("single node near miss below", show, 0x1_0006, 0, &mut failures);
        check("single node near miss above", show, 0x1_0008, 0, &mut failures);
        check("single node low-16-bits-only match", show, 0x0007, 0, &mut failures);

        // 3-node tree: root + left + right children, exact hits plus an in-between miss.
        let left = make_node(5, 0x1111, 0, 0);
        let right = make_node(15, 0x3333, 0, 0);
        let root = make_node(10, 0x2222, left, right);
        let header3 = make_header(root);
        let show3 = make_ztshow(header3);
        check("3-node tree root hit", show3, 10, 0x2222, &mut failures);
        check("3-node tree left hit", show3, 5, 0x1111, &mut failures);
        check("3-node tree right hit", show3, 15, 0x3333, &mut failures);
        check("3-node tree in-between miss", show3, 7, 0, &mut failures);

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
            }
            true
        }
    }
    /// re-declared here per the repo's no-shared-consts precedent (`ztsoundscape.rs` carries the
    /// originals it actually uses). Values confirmed once via a manual PE-section parse of zoo.exe's
    /// `.rdata` (see `ztsoundscape.rs`'s doc comment for the derivation and the `fade_atten_a`/
    /// `fade_atten_b` unit tests that bake them in as literals).
    const FADE_DAT_0063542C_RVA: u32 = 0x0063542c - 0x400000;
    const FADE_DAT_00635428_RVA: u32 = 0x00635428 - 0x400000;
    const FADE_DAT_00635490_RVA: u32 = 0x00635490 - 0x400000;

    /// `ZTSOUNDSCAPE_FADE_CONSTANTS` - a review finding, not part of the original implementation plan:
    /// `ZTSoundscape::update`'s fade-attenuation math (`fade_atten_a`/`fade_atten_b`) reads three
    /// `.rdata` floats live via `get_module_base + RVA`, and the whole f64-truncation-parity argument
    /// for those functions rests on those constants holding the exact values a one-time manual PE
    /// parse confirmed (`DAT_0063542c` = f32 `0x38D1B717`, `DAT_00635428` = `4500.0`, `DAT_00635490` =
    /// `1.0`). Nothing else in the battery would catch drift here: `SET_FADE_ATTENUATION` is called on
    /// an opaque real `SNDSound` object, so `ZTSOUNDSCAPE_UPDATE`'s struct-only compare never observes
    /// the actual attenuation argument the port computes from these constants. This test closes that
    /// gap directly - no live zoo/game state needed, just the loaded module's `.rdata`, so it runs
    /// alongside the other standalone-only tests rather than after `run_load_live_zoo`.
    fn run_ztsoundscape_fade_constants_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSOUNDSCAPE_FADE_CONSTANTS";

        let base = get_module_base("zoo.exe") as u32;
        let c1: f32 = get_from_memory(base + FADE_DAT_0063542C_RVA);
        let c2: f32 = get_from_memory(base + FADE_DAT_00635428_RVA);
        let c3: f32 = get_from_memory(base + FADE_DAT_00635490_RVA);

        let expected_c1 = f32::from_bits(0x38D1_B717);
        let expected_c2 = 4500.0_f32;
        let expected_c3 = 1.0_f32;

        let mut mismatches: Vec<String> = Vec::new();
        if c1.to_bits() != expected_c1.to_bits() {
            mismatches.push(format!(
                "DAT_0063542c: expected {:#010x} ({expected_c1}), got {:#010x} ({c1})",
                expected_c1.to_bits(),
                c1.to_bits()
            ));
        }
        if c2 != expected_c2 {
            mismatches.push(format!("DAT_00635428: expected {expected_c2}, got {c2}"));
        }
        if c3 != expected_c3 {
            mismatches.push(format!("DAT_00635490: expected {expected_c3}, got {c3}"));
        }

        if mismatches.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            error!("{}: mismatch(es): {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
            }
            true
        }
    }

    /// `GLOBAL_ZTScenarioMgr`'s global-slot RVA (`ZTGameMgr_start.c`/`.asm` ground truth). Re-declared
    /// here per the repo's no-shared-consts precedent (each file declares its own copy -
    /// `ztgamemgr.rs` carries the original); that file's copy is left untouched so this stage's diff
    /// stays out of its staged call-site rewiring.
    const GLOBAL_ZTSCENARIOMGR_RVA: u32 = 0x00638ff8 - 0x400000;

    /// The shared game RNG state's RVA (`DAT_00638060`) that `ZTSoundscape::update`'s position jitter
    /// advances through the classic MSVC LCG. Re-declared here per the same no-shared-consts precedent
    /// (`ztsoundscape.rs` carries the original).
    const GAME_RNG_RVA: u32 = 0x00638060 - 0x400000;

    /// `ZTSOUNDSCAPE_INIT` - `ztsoundscape-implementation-plan.md` stage 2 (pulled forward from stage
    /// 5; needs no detours of its own): builds two fresh standalone `ZTSoundscape` blocks (real
    /// vanilla ctor / [`ZTSoundscape::construct`]), **0xAA-fills both** before constructing, then
    /// calls `init` on each - real vanilla (via the `real_init` trampoline) vs. the Rust port - and
    /// compares.
    ///
    /// Inputs come from the live `GLOBAL_ZTScenarioMgr` singleton via its four real getter
    /// call-throughs, captured **once** up front and handed to both poles, so the poles can't drift
    /// apart on getter results. Must run after `run_load_live_zoo` (registered last in the battery,
    /// right before `ZTGAMEMGR_START_STOP_SMOKE`): pre-zoo the scenario registry is non-null-but-
    /// uninitialized (the hazard class `ZTGAMEMGR_START_STOP_SMOKE`'s doc describes), both
    /// `BFConfigFile::attempt`s would fail, and the test would silently cover only the defaults/tail
    /// while looking green.
    ///
    /// **Vanilla pole first, then a full snapshot of its block plus owned copies of every string its
    /// pointer fields reference, and only then the reimpl pole.** The snapshot is load-bearing: both
    /// poles' `init` calls reuse the same two *global* `BFConfigFile` instances, and the second
    /// pole's `release`+`attempt`+parse frees/reallocates the parsed storage the first pole's
    /// `crowd_filename`/`world_name` pointers point into (live-confirmed false-failure mode of this
    /// test's first draft, which compared both sides live: the vanilla side's slot 2 was left reading
    /// the new parse's `"sounds/quiet.wav"` buffer and slots 0/1 landed mid-string - vanilla's own
    /// values were correct at its init time). The reimpl side is compared while its own parse is
    /// still live.
    ///
    /// Comparison set (snapshot vs. the live reimpl block; masked regions re-covered by replacements):
    /// - `+0x09` (`fade_step_in`, one of the two deliberately-uninitialized bytes): asserted still
    ///   `0xAA` on **both** sides - the byte-diff alone can't catch a port that wrongly *writes* a
    ///   byte both sides leave alone, but a raw filler assert does.
    /// - `+0x1c..=0x2b` (`crowd_filename`): per-slot CStr **content** compare - the pointers
    ///   legitimately differ across the two parses.
    /// - `+0x40..=0x43` (`world_snd.inner`): null-ness parity only. Per-attempt vanilla-owned
    ///   resource object (live-confirmed: the same attempted name handed the two poles different
    ///   handles), so a value compare is wrong by construction.
    /// - `+0x44..=0x47` (`world_name`): null-ness parity, then content compare when the vanilla
    ///   snapshot's is non-zero - its pointer also legitimately differs across parses (live-confirmed).
    /// - `+0x48..=0x4b` (`world_atten`, the second deliberately-uninitialized byte - untouched when
    ///   no world sound is configured): compared only when the vanilla snapshot's `world_name`
    ///   (`+0x44`) is non-zero.
    /// - `+0x4c..=0x53` (both `Ambients*`, real heap addresses): null-ness parity only.
    /// Everything else - the scalars, both idle crowd `SNDSound` slots, the world slot's vtable, and
    /// the four `crowd_atten` values - is byte-compared as-is.
    ///
    /// Distinctness probe: the four vanilla-side `crowd_filename` pointers are checked for pairwise
    /// distinctness (all equal/overlapping would mean `getString` reuses a scratch buffer, parity
    /// still holds, and the content compare degenerates to trivial). Recorded in this test's own
    /// success line (direct file write - `info!` lines placed mid-test are lost to the battery's
    /// known tracing lossiness under `std::process::exit`): live run 2026-09-02 recorded **pairwise
    /// distinct**. Relevant to `update`'s later filename reads: parsed names live in per-key storage,
    /// not one scratch buffer.
    ///
    /// Audible caveat: both poles run `init` for real, so the battery briefly plays the world sound
    /// **twice, overlapping** (each side loops one until teardown stops it) - documented, not a
    /// failure.
    ///
    /// Teardown goes through [`soundscape_live_support::destroy_standalone_after_init`] on both
    /// blocks, which releases the vanilla-allocated `Ambients` blocks via the real destructor and
    /// stops each side's sound (see its doc comment for the cross-allocator reasoning).
    ///
    /// Pole note: the vanilla side goes through `soundscape_live_support::real_init` (an
    /// `INIT_DETOUR.call` trampoline) - the stage-4 obligation this test used to document (a release
    /// build's raw-cast `.original()` would re-enter the Rust detour and degenerate this test into
    /// Rust-vs-Rust) is discharged now that the detours are installed. The four `bfscenariomgr` getter
    /// captures above stay `.original()` - none of those is detoured.
    ///
    /// Accepted gaps: `OPERATOR_NEW`'s failure paths (the store-`0` + skip-ctor propagation) can't be
    /// exercised live; `world_snd.inner` is only null-ness-compared (see above).
    fn run_ztsoundscape_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSOUNDSCAPE_INIT";

        let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
        if scenariomgr_ptr == 0 {
            info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
            return false;
        }

        // Capture the four getter results once; both poles get exactly these.
        let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let crowd_config = unsafe { GET_CROWD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };
        let world_config = unsafe { GET_WORLD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };

        let real_ptr = soundscape_live_support::allocate_uninitialized();
        let reimpl_ptr = soundscape_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            // Nothing was constructed or init'ed on this path, so a plain free is complete - the
            // dtor-aware path would walk 0xAA garbage.
            if !real_ptr.is_null() {
                soundscape_live_support::destroy_standalone(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                soundscape_live_support::destroy_standalone(reimpl_ptr);
            }
            return true;
        }

        // Owned copy of one pointed-to name; `None` = no usable pointer (null or 0xAA filler).
        fn copy_cstr(p: u32) -> Option<Vec<u8>> {
            if p == 0 || p == 0xAAAA_AAAA {
                return None;
            }
            Some(unsafe { CStr::from_ptr(p as *const i8) }.to_bytes().to_vec())
        }

        let struct_size = size_of::<ZTSoundscape>();
        let mut mismatches: Vec<String> = Vec::new();

        unsafe {
            // 0xAA fill (not zero) so every deliberately-uninitialized byte is detectably garbage.
            std::ptr::write_bytes(real_ptr as *mut u8, 0xAA, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0xAA, struct_size);

            soundscape_live_support::real_constructor(real_ptr as *const c_void);
            (*reimpl_ptr).construct();

            // Vanilla pole first: the generated INIT's params 2/3 carry a `*const u32` wart - cast
            // here, in the test, not inside the port (see `ZTSoundscape::init`'s doc comment).
            soundscape_live_support::real_init(
                real_ptr as *const c_void,
                crowd_ambients as *const u32,
                world_ambients as *const u32,
                crowd_config,
                world_config,
            );

        }

        // Snapshot before the reimpl pole re-parses the shared global config instances (see this
        // test's doc comment): the whole block; the pointer-valued fields get owned string copies
        // extracted below, before anything can invalidate the storage they reference.
        let vanilla_snap = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) }.to_vec();

        // Vanilla-side snapshot extraction (all safe - vanilla_snap is an owned copy), plus the
        // distinctness probe (see this test's doc comment): pointers equal -> getString shares a
        // scratch buffer and the content compare below degenerates to trivial.
        let dword = |bytes: &[u8], off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let vanilla_name_ptrs: [u32; 4] = std::array::from_fn(|s| dword(&vanilla_snap, 0x1c + s * 4));
        let vanilla_crowd_names: [Option<Vec<u8>>; 4] = std::array::from_fn(|s| copy_cstr(vanilla_name_ptrs[s]));
        let vanilla_world_name_ptr = dword(&vanilla_snap, 0x44);
        let vanilla_world_name = copy_cstr(vanilla_world_name_ptr);
        let vanilla_distinct = (0..4).all(|a| (a + 1..4).all(|b| vanilla_name_ptrs[a] != vanilla_name_ptrs[b]));

        unsafe {
            // Reimpl pole second; its own parse is still live at compare time.
            (*reimpl_ptr).init(crowd_ambients, world_ambients, crowd_config, world_config);
            let reimpl_bytes = std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size);

            // fade_step_in (+0x09): must still be raw filler on both sides.
            if vanilla_snap[0x09] != 0xAA {
                mismatches.push("fade_step_in (+0x09) written on the vanilla side (expected untouched 0xAA filler)".to_string());
            }
            if reimpl_bytes[0x09] != 0xAA {
                mismatches.push("fade_step_in (+0x09) written on the reimpl side (expected untouched 0xAA filler)".to_string());
            }

            // Whole-block byte diff, minus the masked regions - crowd_filename, world_snd.inner,
            // world_name, world_atten, both Ambients pointers - each re-covered by a replacement
            // check below.
            for i in 0..struct_size {
                if matches!(i, 0x1c..=0x2b | 0x40..=0x53) || vanilla_snap[i] == reimpl_bytes[i] {
                    continue;
                }
                mismatches.push(format!("byte +{i:#04x}: vanilla={:#04x}, reimpl={:#04x}", vanilla_snap[i], reimpl_bytes[i]));
            }

            // crowd_filename: per-slot content compare of snapshot vs. the live reimpl pointers.
            for slot in 0..4 {
                let reimpl_ptr_val = dword(reimpl_bytes, 0x1c + slot * 4);
                match (vanilla_crowd_names[slot].as_deref(), copy_cstr(reimpl_ptr_val).as_deref()) {
                    (Some(v), Some(r)) if v == r => {}
                    (v, r) => mismatches.push(format!(
                        "crowd_filename[{slot}] content: vanilla={:?}, reimpl={:?}",
                        v.map(|b| String::from_utf8_lossy(b).into_owned()),
                        r.map(|b| String::from_utf8_lossy(b).into_owned()),
                    )),
                }
            }

            // world_snd.inner: null-ness parity only (per-attempt resource object - see doc).
            let (vanilla_inner, reimpl_inner) = (dword(&vanilla_snap, 0x40), dword(reimpl_bytes, 0x40));
            if (vanilla_inner != 0) != (reimpl_inner != 0) {
                mismatches.push(format!("world_snd.inner null-ness: vanilla={vanilla_inner:#010x}, reimpl={reimpl_inner:#010x}"));
            }

            // world_name: null-ness parity, then content compare when configured.
            let reimpl_world_name_ptr = dword(reimpl_bytes, 0x44);
            if (vanilla_world_name_ptr != 0) != (reimpl_world_name_ptr != 0) {
                mismatches.push(format!("world_name null-ness: vanilla={vanilla_world_name_ptr:#010x}, reimpl={reimpl_world_name_ptr:#010x}"));
            } else if let (Some(v), Some(r)) = (vanilla_world_name.as_deref(), copy_cstr(reimpl_world_name_ptr).as_deref()) {
                if v != r {
                    mismatches.push(format!(
                        "world_name content: vanilla={:?}, reimpl={:?}",
                        String::from_utf8_lossy(v),
                        String::from_utf8_lossy(r),
                    ));
                }
            }

            // world_atten: only comparable when a world sound was actually configured.
            if vanilla_world_name_ptr != 0 {
                let (v, r) = (dword(&vanilla_snap, 0x48), dword(reimpl_bytes, 0x48));
                if v != r {
                    mismatches.push(format!("world_atten: vanilla={v:#010x}, reimpl={r:#010x}"));
                }
            }

            // Both Ambients pointers: null-ness parity only.
            for (name, off) in [("crowd_ambients", 0x4c), ("world_ambients", 0x50)] {
                let (v, r) = (dword(&vanilla_snap, off), dword(reimpl_bytes, off));
                if (v != 0) != (r != 0) {
                    mismatches.push(format!("{name} null-ness: vanilla={v:#010x}, reimpl={r:#010x}"));
                }
            }
        }

        if !mismatches.is_empty() {
            error!("{}: mismatch(es): {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
            }
        } else {
            write_success_line(failure_log, &format!("{} (vanilla crowd_filename pointers pairwise distinct: {})", test_name, vanilla_distinct));
        }

        soundscape_live_support::destroy_standalone_after_init(real_ptr);
        soundscape_live_support::destroy_standalone_after_init(reimpl_ptr);
        !mismatches.is_empty()
    }

    /// `ZTSOUNDSCAPE_UPDATE` - `ztsoundscape-implementation-plan.md` stage 3 (pulled forward from
    /// stage 5 per the per-stage pattern stages 1-2 established): runs `ZTSoundscape::update` on
    /// **three** standalone twins - A = real vanilla (via the `real_update` trampoline), B = the Rust
    /// port, C = vanilla again (determinism control, so "port diverged" is distinguishable from
    /// "environment nondeterministic") - and compares. Needs the live zoo (scenario-registry config
    /// names + real crowd `.wav`s), so it registers right after [`run_ztsoundscape_init_test`].
    ///
    /// Twins are built exactly as `ZTSOUNDSCAPE_INIT` builds its two: `allocate_uninitialized` ->
    /// `0xAA` fill -> ctor (vanilla A/C / Rust B) -> `init` with the same four captured
    /// `GLOBAL_ZTScenarioMgr` getter strings.
    ///
    /// **Pre-equalization** (needed because each pole's `init` re-parses the two shared *global*
    /// `BFConfigFile` instances, so pointer-valued fields legitimately drift - the same discovery
    /// `ZTSOUNDSCAPE_INIT` documented; `update` never re-reads configs): `crowd_filename[4]`,
    /// `crowd_atten[4]`, `world_name`, and `world_atten` are copied from B (the freshest live parse)
    /// into A and C, and `fade_step_in` (+0x09, deliberately-uninitialized filler) is written `0` on
    /// all three - only ever read while `fading`, but this makes +0x09 byte-comparable once the start
    /// block (which rewrites it identically from the equal `next_slot_is_b = 0`) has run. Each side's
    /// own `world_snd.inner` is left alone (teardown still releases its own handle).
    ///
    /// **Guest-count override**: the hysteresis holds a track forever at a constant guest count, so
    /// the plan's phase script (mid-fade tick, then a clamp-to-endpoint tick with a same-tick
    /// restart, then an endpoint tick that really stops the playing slot and restarts again) is only
    /// reachable in a guest band where every phase's selection lands on a *new* target: `>= 161`
    /// (`-1 -> 1` on the start tick, `1 -> 2` on phase 2's fall-through restart, `2 -> 3` on phase
    /// 3's). The dword at `ZTGameMgr+0x54` is therefore forced to 200 for the test (vanilla reads a
    /// full dword there) and restored afterwards; the live value is recorded in the success line.
    ///
    /// **RNG discipline** (update jitters both `Ambients` blocks through the shared global game RNG):
    /// the state at VA `0x00638060` is snapshotted before phase 1; A runs the phase's ticks, the RNG
    /// is rewound, B runs the same ticks, rewound again, C runs them; the snapshot is restored at test
    /// end (no net stream shift for the vanilla consumers). The rewinds survive for that stream
    /// discipline, but the *position-equality* compare across poles they were meant to enable is
    /// retired - see the fallback note below. Phases 2-3 run A then B without rewinds.
    ///
    /// **Fallback applied (first live run)**: the plan's escape hatch ("if the rewind/compare proves
    /// flaky in practice, scope down to struct-only compare + an 'ambients positions changed' sanity
    /// assert", `ZTAdvTerrainMgr` precedent) fired on the very first run. The A-vs-C vanilla
    /// determinism control failed identically to A-vs-B, with **B == C bit-for-bit** (crowd
    /// (3111, 596, 0), world (3262, 702, 0) on both) and only A - the pole that ran first - holding
    /// different positions. That is vanilla nondeterminism under the rewind scheme, not a port
    /// divergence (a port jitter bug reads A == C != B): the real sound subsystem's asynchronous
    /// response to `Ambients::play` (itself a vanilla shared-RNG consumer) draws the global state from
    /// its own thread, so the first pole runs from the clean snapshot while every later pole runs from
    /// a state polluted after its rewind. Genuinely non-comparable, so: struct-only compare (the
    /// state machine never holds jitter values and passed byte-exact through all three phases) plus a
    /// per-pole "ambients positions changed" sanity assert; the jitter math itself stays pinned by the
    /// hand-computed seed vectors in `ztsoundscape.rs`'s unit tests, and this run's B == C is live
    /// evidence the port's jitter reproduces vanilla's bit-for-bit from an equal starting state.
    ///
    /// Phase script (both fade ticks land mid-script by construction - the start tick sets
    /// `fade = 10000` with `fading = 1` and no fade block):
    /// - **Phase 1** - two `delta = 1000` ticks per pole: tick 1 starts the crowd loop on slot A
    ///   (`fading = 1`, `fade = 10000`, `fade_step_in = 0`), tick 2 steps the ramp to 9000 (real
    ///   `SET_FADE_ATTENUATION`/`SET_VOLUME` on the live slot A at a mid-ramp value). Compare: masked
    ///   struct A vs B and A vs C, plus the per-pole "ambients positions changed" sanity.
    /// - **Phase 2** - one `delta = 10000` tick on A then B: fade 9000 -> wraps past 0 -> clamps to 0
    ///   -> endpoint stop gated off (slot B never started, `VALID` false) -> `fading` cleared ->
    ///   fall-through restart on slot B (`fade_step_in = 1`, `fade = 0`). Covers advance+clamp, the
    ///   endpoint logic's gated-off arm, and the same-tick restart.
    /// - **Phase 3** - one `delta = 10000` tick on A then B: fade 0 -> 10000 -> endpoint **really
    ///   stops the playing slot A** (`VALID` true -> `STOP` + `RELEASE`) and restarts on slot A
    ///   (target 3, `next_slot_is_b` back at 0).
    ///
    /// Masked compare regions (carry-over from `ZTSOUNDSCAPE_INIT`'s live-confirmed discoveries):
    /// per-attempt vanilla-owned inner handles - `+0x10..=0x13`, `+0x18..=0x1b` (crowd slots, both
    /// firing attempts from phase 1 on) and `+0x40..=0x43` (world) - null-ness parity only, plus
    /// `+0x4c..=0x53` (each twin's own `Ambients*`, real per-twin heap addresses - null-ness only;
    /// the plan's masked list omits these, but they can never be byte-equal across twins).
    /// Everything else is byte-compared, including `+0x09`.
    ///
    /// Pole note: both vanilla poles go through `soundscape_live_support::real_update` (an
    /// `UPDATE_DETOUR.call` trampoline) - the stage-4 obligation this test used to document (a release
    /// build's raw-cast `.original()` would re-enter the Rust detour and degenerate them into
    /// Rust-vs-Rust, taking the A/C determinism control down with them) is discharged now that the
    /// detours are installed.
    ///
    /// Audible caveat: real crowd loops are started/stopped across the phases and several overlap
    /// (A/B/C each loop one world + crowd sound until teardown) - documented, not a failure.
    ///
    /// Teardown: `destroy_standalone_after_init` x3 (real vanilla destructor + `operator delete` -
    /// see [`soundscape_live_support::destroy_standalone_after_init`]'s cross-allocator reasoning).
    fn run_ztsoundscape_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSOUNDSCAPE_UPDATE";

        let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
        if scenariomgr_ptr == 0 {
            info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
            return false;
        }
        let gamemgr_ptr = globals().ztgamemgr_ptr();
        if gamemgr_ptr.is_null() {
            info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
            return false;
        }

        // Capture the four getter results once; all three poles get exactly these.
        let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let crowd_config = unsafe { GET_CROWD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };
        let world_config = unsafe { GET_WORLD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };

        let vanilla_a = soundscape_live_support::allocate_uninitialized();
        let reimpl_b = soundscape_live_support::allocate_uninitialized();
        let vanilla_c = soundscape_live_support::allocate_uninitialized();
        if vanilla_a.is_null() || reimpl_b.is_null() || vanilla_c.is_null() {
            error!(
                "{}: OPERATOR_NEW returned null (a={:?}, b={:?}, c={:?})",
                test_name, vanilla_a, reimpl_b, vanilla_c
            );
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!(
                        "Test Failed {}: OPERATOR_NEW returned null (a={:?}, b={:?}, c={:?})\n",
                        test_name, vanilla_a, reimpl_b, vanilla_c
                    )
                    .as_bytes(),
                );
            }
            // Nothing was constructed or init'ed on this path, so a plain free is complete.
            for ptr in [vanilla_a, reimpl_b, vanilla_c] {
                if !ptr.is_null() {
                    soundscape_live_support::destroy_standalone(ptr);
                }
            }
            return true;
        }

        let base = get_module_base("zoo.exe") as u32;
        let rng_addr = base + GAME_RNG_RVA;
        let guests_addr = gamemgr_ptr as u32 + 0x54;
        let live_guests: i32 = get_from_memory(guests_addr);
        const FORCED_GUESTS: i32 = 200; // >= 161: every phase's hysteresis selection lands on a new target

        let struct_size = size_of::<ZTSoundscape>();
        let dword = |bytes: &[u8], off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let mut mismatches: Vec<String> = Vec::new();

        unsafe {
            // Build all three twins: 0xAA fill, ctor, init (vanilla A/C, Rust B). The generated
            // INIT's params 2/3 carry a `*const u32` wart - cast here, in the test, not inside the
            // port (see `ZTSoundscape::init`'s doc comment).
            for ptr in [vanilla_a, reimpl_b, vanilla_c] {
                std::ptr::write_bytes(ptr as *mut u8, 0xAA, struct_size);
            }
            soundscape_live_support::real_constructor(vanilla_a as *const c_void);
            soundscape_live_support::real_constructor(vanilla_c as *const c_void);
            (*reimpl_b).construct();
            for ptr in [vanilla_a, vanilla_c] {
                soundscape_live_support::real_init(
                    ptr as *const c_void,
                    crowd_ambients as *const u32,
                    world_ambients as *const u32,
                    crowd_config,
                    world_config,
                );
            }
            (*reimpl_b).init(crowd_ambients, world_ambients, crowd_config, world_config);

            // Pre-equalization: B's freshest live parse into A and C (see this test's doc comment).
            for twin in [vanilla_a, vanilla_c] {
                for off in (0x1c..0x3c).step_by(4) {
                    // crowd_filename[4] + crowd_atten[4]
                    let v: u32 = get_from_memory(reimpl_b as u32 + off);
                    save_to_memory(twin as u32 + off, v);
                }
                for off in [0x44, 0x48] {
                    // world_name, world_atten
                    let v: u32 = get_from_memory(reimpl_b as u32 + off);
                    save_to_memory(twin as u32 + off, v);
                }
            }
            for twin in [vanilla_a, reimpl_b, vanilla_c] {
                save_to_memory(twin as u32 + 0x09, 0u8); // fade_step_in
            }
        }

        // Masked struct compare (see this test's doc comment for the masked regions). The Ambients
        // position triples are deliberately NOT compared across poles - retired per the applied
        // fallback (see this test's doc comment); their only live check is the "positions changed"
        // sanity below.
        let compare = |label: &'static str,
                       a_ptr: *const ZTSoundscape,
                       b_ptr: *const ZTSoundscape,
                       mismatches: &mut Vec<String>| {
            let a = unsafe { std::slice::from_raw_parts(a_ptr as *const u8, struct_size) };
            let b = unsafe { std::slice::from_raw_parts(b_ptr as *const u8, struct_size) };
            for i in 0..struct_size {
                if matches!(i, 0x10..=0x13 | 0x18..=0x1b | 0x40..=0x43 | 0x4c..=0x53) || a[i] == b[i] {
                    continue;
                }
                mismatches.push(format!("{label}: byte +{i:#04x}: {:#04x} vs {:#04x}", a[i], b[i]));
            }
            for (name, off) in [
                ("crowd_snd_a.inner", 0x10),
                ("crowd_snd_b.inner", 0x18),
                ("world_snd.inner", 0x40),
                ("crowd_ambients", 0x4c),
                ("world_ambients", 0x50),
            ] {
                let (x, y) = (dword(a, off), dword(b, off));
                if (x != 0) != (y != 0) {
                    mismatches.push(format!("{label}: {name} null-ness: {x:#010x} vs {y:#010x}"));
                }
            }
        };

        // One pole's `(crowd, world)` Ambients position triples, read from its own blocks.
        let ambients_triple = |ptr: *const ZTSoundscape| -> [(i32, i32, i32); 2] {
            [0x4c, 0x50].map(|off| {
                let p = dword(unsafe { std::slice::from_raw_parts(ptr as *const u8, struct_size) }, off);
                if p == 0 {
                    (0, 0, 0)
                } else {
                    unsafe {
                        (
                            get_from_memory::<i32>(p + 0xc),
                            get_from_memory::<i32>(p + 0x10),
                            get_from_memory::<i32>(p + 0x14),
                        )
                    }
                }
            })
        };

        // The guest-count override covers every update call below; both orders are safe because the
        // vanilla pole reads the same dword the Rust port does.
        save_to_memory(guests_addr, FORCED_GUESTS);

        // "Positions changed" sanity (the applied fallback - see doc comment): capture each pole's
        // pre-update triples to diff against after phase 1.
        let poles = [("A", vanilla_a), ("B", reimpl_b), ("C", vanilla_c)];
        let pre_positions: Vec<_> = poles.iter().map(|(_, p)| ambients_triple(*p)).collect();

        // Phase 1: two delta=1000 ticks per pole, RNG-rewound between poles (see doc comment).
        let s0: u32 = get_from_memory(rng_addr);
        unsafe {
            // Pole A: real vanilla twice.
            soundscape_live_support::real_update(vanilla_a as *const c_void, 1000);
            soundscape_live_support::real_update(vanilla_a as *const c_void, 1000);
        }
        save_to_memory(rng_addr, s0);
        unsafe {
            // Pole B: the Rust port twice.
            (*reimpl_b).update(1000);
            (*reimpl_b).update(1000);
        }
        save_to_memory(rng_addr, s0);
        unsafe {
            // Pole C: real vanilla again (determinism control).
            soundscape_live_support::real_update(vanilla_c as *const c_void, 1000);
            soundscape_live_support::real_update(vanilla_c as *const c_void, 1000);
        }
        compare("phase 1 A/B", vanilla_a, reimpl_b, &mut mismatches);
        compare("phase 1 A/C", vanilla_a, vanilla_c, &mut mismatches);

        // "Positions changed" sanity: each pole's jitter + write path must have moved at least one
        // Ambients block off its pre-update triple (a zero-jitter coincidence on one block is
        // possible but both blocks standing still means step 4 never ran on that pole).
        for ((name, ptr), pre) in poles.iter().zip(&pre_positions) {
            let post = ambients_triple(*ptr);
            if post == *pre {
                mismatches.push(format!(
                    "phase 1: ambients positions did not change on pole {name}: {pre:?} -> {post:?}"
                ));
            }
        }

        // Phase 2: fade 9000 -> clamp 0 -> gated-off endpoint -> fall-through restart on slot B.
        unsafe {
            soundscape_live_support::real_update(vanilla_a as *const c_void, 10000);
        }
        unsafe {
            (*reimpl_b).update(10000);
        }
        compare("phase 2 A/B", vanilla_a, reimpl_b, &mut mismatches);

        // Phase 3: fade 0 -> 10000 -> endpoint really stops slot A -> fall-through restart on slot A.
        unsafe {
            soundscape_live_support::real_update(vanilla_a as *const c_void, 10000);
        }
        unsafe {
            (*reimpl_b).update(10000);
        }
        compare("phase 3 A/B", vanilla_a, reimpl_b, &mut mismatches);

        // Teardown: restore the shared global state first, then release all three twins through the
        // real vanilla destructor (stops every sound they started).
        save_to_memory(rng_addr, s0);
        save_to_memory(guests_addr, live_guests);
        soundscape_live_support::destroy_standalone_after_init(vanilla_a);
        soundscape_live_support::destroy_standalone_after_init(reimpl_b);
        soundscape_live_support::destroy_standalone_after_init(vanilla_c);

        if !mismatches.is_empty() {
            error!("{}: mismatch(es): {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
            }
        } else {
            write_success_line(
                failure_log,
                &format!("{} (live guest count {}, forced to {} for the phase script)", test_name, live_guests, FORCED_GUESTS),
            );
        }
        !mismatches.is_empty()
    }

    /// `ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE` - a review finding, not part of the original
    /// implementation plan: `ZTSOUNDSCAPE_UPDATE`'s phase script always plays real, present crowd
    /// `.wav`s, so it never reaches `update`'s start block's `ATTEMPT`-fails branch. Per
    /// `ZTSoundscape::update`'s doc comment, `current_track` updates to the selected target even when
    /// the attempt to start that track's sound fails - only the `fading`/`fade_step_in`/`fade`/
    /// `next_slot_is_b` crossfade-state-machine advance is gated on success. This test forces that
    /// branch directly and pins both halves of that contract.
    ///
    /// Builds two standalone twins (real vanilla / reimpl) exactly as `ZTSOUNDSCAPE_INIT`/`_UPDATE` do
    /// (`allocate_uninitialized` -> `0xAA` fill -> ctor -> `init` with the same four captured
    /// `GLOBAL_ZTScenarioMgr` getter strings), then - because each pole's `init` re-parses the shared
    /// global `BFConfigFile` instances and so legitimately ends up with different pointer values in
    /// `crowd_filename`/`crowd_atten`/`world_name`/`world_atten` (`ZTSOUNDSCAPE_INIT`'s documented
    /// discovery) - pre-equalizes those fields from the reimpl pole into the vanilla pole exactly as
    /// `ZTSOUNDSCAPE_UPDATE` does, so the later struct compare isn't comparing two independent parses.
    ///
    /// Only then does it overwrite `crowd_filename[0]` on **both** twins with a filename engineered to
    /// fail, and forces the live `GLOBAL_ZTGameMgr` guest count to `0`: with `current_track` fresh at
    /// `-1` after `init`, `select_target_track(-1, 0)` deterministically picks target `0` (the
    /// `g <= 14 && t != 0` arm), so the start block's `ATTEMPT` is guaranteed to run against the bogus
    /// filename on both poles.
    ///
    /// **The filename can't just be "nonexistent" - it has to fail `SNDSound::attempt`'s own gate.**
    /// `SNDSound_attempt.asm` shows `attempt` never touches the filesystem or `DX8SndMgr` for a
    /// same-vtable check first: it reads the filename's **last character** and short-circuits to
    /// `false` with no allocation at all unless that character is `'v'`/`'V'` (a crude `.wav`-extension
    /// sniff, `CMP %CL, 0x76` / `0x56` at `.1ece9e`/`.1ecebc`) - only past that gate does it allocate a
    /// `DX8Sound` and call through to real `BFSndMgr`/DirectSound. A first draft of this test used a
    /// `__openzt_test_nonexistent_*.wav` name (mirroring `MENUMUSICHANDLER_INIT`'s own guaranteed-
    /// missing-file idiom) and it live-failed: both poles agreed the attempt **succeeded** (`fading = 1,
    /// fade = 10000, next_slot_is_b = 1` on both) - not a port divergence, since real and reimpl matched
    /// bit-for-bit, but proof the deeper `DX8Sound`/`BFSndMgr` path doesn't fail synchronously on a
    /// missing file either (the real load is presumably async/deferred). A second draft then tried
    /// `"...notawav"`, missing that "notawav" itself still ends in `'v'` - same live failure, same
    /// signature, root-caused by re-reading `.asm:14` (`MOV %CL, [ECX + EBX - 1]`, the string's *last*
    /// byte, not merely "looks like a `.wav` name" as a whole). The filename below ends in `.txt`
    /// instead, which fails deterministically at the string-shape gate alone - no dependency on any
    /// real sound-loading behavior, and no near-miss on the extension check either.
    ///
    /// One `update` tick runs on each pole, then: a masked struct compare (same per-attempt inner-handle
    /// and `Ambients*` null-ness-only regions as `ZTSOUNDSCAPE_UPDATE` - no RNG rewind needed since
    /// those are the only RNG-sensitive bytes in the compared struct, and they're masked) catches any
    /// unexpected divergence, and four explicit assertions per pole pin the contract itself:
    /// `current_track == 0` (updated despite the failed attempt), `fading == 0`, `fade == 0`, and
    /// `next_slot_is_b == 0` (the crossfade state machine never armed).
    ///
    /// Teardown via `destroy_standalone_after_init` on both, same cross-allocator reasoning as the
    /// other `ZTSOUNDSCAPE_*` tests.
    fn run_ztsoundscape_update_attempt_failure_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE";

        let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
        if scenariomgr_ptr == 0 {
            info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
            return false;
        }
        let gamemgr_ptr = globals().ztgamemgr_ptr();
        if gamemgr_ptr.is_null() {
            info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
            return false;
        }

        let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let crowd_config = unsafe { GET_CROWD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };
        let world_config = unsafe { GET_WORLD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };

        let vanilla_a = soundscape_live_support::allocate_uninitialized();
        let reimpl_b = soundscape_live_support::allocate_uninitialized();
        if vanilla_a.is_null() || reimpl_b.is_null() {
            error!("{}: OPERATOR_NEW returned null (a={:?}, b={:?})", test_name, vanilla_a, reimpl_b);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: OPERATOR_NEW returned null (a={:?}, b={:?})\n", test_name, vanilla_a, reimpl_b).as_bytes(),
                );
            }
            for ptr in [vanilla_a, reimpl_b] {
                if !ptr.is_null() {
                    soundscape_live_support::destroy_standalone(ptr);
                }
            }
            return true;
        }

        let struct_size = size_of::<ZTSoundscape>();
        let guests_addr = gamemgr_ptr as u32 + 0x54;
        let live_guests: i32 = get_from_memory(guests_addr);
        const FORCED_GUESTS: i32 = 0; // <= 14: select_target_track(-1, 0) deterministically picks target 0
        // Last byte before the nul is deliberately NOT 'v'/'V' - SNDSound::attempt (SNDSound_attempt.asm)
        // reads only that byte and short-circuits to false with no allocation and no BFSndMgr/DX8Sound
        // call at all otherwise (see this test's doc comment for why a "*.wav" name doesn't work here).
        // NB: "notawav" itself still ends in 'v' - the live-confirmed failure mode of this constant's
        // first fix attempt. ".txt" ends in 't', clear of the gate.
        const BOGUS_FILENAME: &[u8] = b"__openzt_test_forced_attempt_failure.txt\0";

        let mut mismatches: Vec<String> = Vec::new();

        unsafe {
            std::ptr::write_bytes(vanilla_a as *mut u8, 0xAA, struct_size);
            std::ptr::write_bytes(reimpl_b as *mut u8, 0xAA, struct_size);

            soundscape_live_support::real_constructor(vanilla_a as *const c_void);
            (*reimpl_b).construct();

            // Generated INIT's params 2/3 carry a `*const u32` wart - cast here, in the test, not
            // inside the port (see `ZTSoundscape::init`'s doc comment).
            soundscape_live_support::real_init(
                vanilla_a as *const c_void,
                crowd_ambients as *const u32,
                world_ambients as *const u32,
                crowd_config,
                world_config,
            );
            (*reimpl_b).init(crowd_ambients, world_ambients, crowd_config, world_config);

            // Pre-equalization: B's freshest live parse into A, same shape as ZTSOUNDSCAPE_UPDATE's
            // (both poles' `init` re-parse the shared global config instances, so these pointer-valued
            // fields legitimately drift between independent parses - see that test's doc comment).
            for off in (0x1c..0x3c).step_by(4) {
                // crowd_filename[4] + crowd_atten[4]
                let v: u32 = get_from_memory(reimpl_b as u32 + off);
                save_to_memory(vanilla_a as u32 + off, v);
            }
            for off in [0x44, 0x48] {
                // world_name, world_atten
                let v: u32 = get_from_memory(reimpl_b as u32 + off);
                save_to_memory(vanilla_a as u32 + off, v);
            }

            // Overwrite crowd_filename[0] on both twins with a filename that cannot resolve, forcing
            // the update start block's ATTEMPT to fail on the branch this test targets. Applied after
            // pre-equalization so it isn't clobbered by the copy above.
            let bogus_ptr = BOGUS_FILENAME.as_ptr() as u32;
            save_to_memory(vanilla_a as u32 + 0x1c, bogus_ptr);
            save_to_memory(reimpl_b as u32 + 0x1c, bogus_ptr);
        }

        save_to_memory(guests_addr, FORCED_GUESTS);
        unsafe {
            soundscape_live_support::real_update(vanilla_a as *const c_void, 1000);
            (*reimpl_b).update(1000);
        }
        save_to_memory(guests_addr, live_guests);

        let dword = |bytes: &[u8], off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        unsafe {
            let a = std::slice::from_raw_parts(vanilla_a as *const u8, struct_size);
            let b = std::slice::from_raw_parts(reimpl_b as *const u8, struct_size);

            // Masked struct compare - same per-attempt inner-handle / Ambients-pointer null-ness-only
            // regions as ZTSOUNDSCAPE_UPDATE.
            for i in 0..struct_size {
                if matches!(i, 0x10..=0x13 | 0x18..=0x1b | 0x40..=0x43 | 0x4c..=0x53) || a[i] == b[i] {
                    continue;
                }
                mismatches.push(format!("byte +{i:#04x}: vanilla={:#04x}, reimpl={:#04x}", a[i], b[i]));
            }
            for (name, off) in [
                ("crowd_snd_a.inner", 0x10),
                ("crowd_snd_b.inner", 0x18),
                ("world_snd.inner", 0x40),
                ("crowd_ambients", 0x4c),
                ("world_ambients", 0x50),
            ] {
                let (x, y) = (dword(a, off), dword(b, off));
                if (x != 0) != (y != 0) {
                    mismatches.push(format!("{name} null-ness: vanilla={x:#010x}, reimpl={y:#010x}"));
                }
            }

            // The contract this test exists to pin: current_track updates to the selected target on
            // BOTH poles even though the attempt failed, while the crossfade state machine (fading/
            // fade/next_slot_is_b) must NOT have advanced from init's tail values.
            let current_track = |p: &[u8]| i32::from_le_bytes(p[0x0..0x4].try_into().unwrap());
            for (label, bytes) in [("vanilla", a), ("reimpl", b)] {
                if current_track(bytes) != 0 {
                    mismatches.push(format!("{label}: current_track = {} (expected 0, the selected target)", current_track(bytes)));
                }
                if bytes[0xa] != 0 {
                    mismatches.push(format!("{label}: fading = {} (expected 0 - the attempt-gated block must not have run)", bytes[0xa]));
                }
                if dword(bytes, 0x4) != 0 {
                    mismatches.push(format!("{label}: fade = {} (expected 0, untouched from init's tail)", dword(bytes, 0x4) as i32));
                }
                if bytes[0x8] != 0 {
                    mismatches.push(format!("{label}: next_slot_is_b = {} (expected 0, untouched from init's tail)", bytes[0x8]));
                }
            }
        }

        if !mismatches.is_empty() {
            error!("{}: mismatch(es): {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
            }
        } else {
            write_success_line(failure_log, &format!("{} (live guest count {}, forced to {} for one tick)", test_name, live_guests, FORCED_GUESTS));
        }

        soundscape_live_support::destroy_standalone_after_init(vanilla_a);
        soundscape_live_support::destroy_standalone_after_init(reimpl_b);
        !mismatches.is_empty()
    }

    /// `MENUMUSICHANDLER_INIT` - `menumusichandler-implementation-plan.md` Stage 2: constructs two fresh
    /// standalone `MenuMusicHandler`s (real vanilla ctor / `MenuMusicHandler::construct`), then calls
    /// `init` on each (real vanilla via the `real_init` trampoline / `MenuMusicHandler::init` - see
    /// `run_menumusichandler_detours_enabled_test` for why `.original()` can't be used here) with a
    /// guaranteed-missing filename - per the plan's own caveat, this avoids the "plays audio during the
    /// test battery" side effect a real `sounds/*.wav` path would have, at the cost of only exercising
    /// `init`'s allocation/attenuation-call shape, not `DX8SndMgr::attempt`'s success path. Compares the
    /// `bool` return and the fields in [`menumusichandler_field_mismatches`]; `sound_ptr` is compared
    /// for null-ness only (a real heap address, expected to differ between the two independent
    /// allocations; both sides go through the same real `operator_new`, so a real vs. reimplemented
    /// `SNDSound` should end up either both allocated or both null together, matching `init`'s own
    /// success/failure branching).
    ///
    /// A second phase calls `init` again on the already-init'ed pair - entering the "existing sound,
    /// not playing" branch (`sound_ptr != 0`, `IS_PLAYING` false -> skip stop/release), then
    /// re-allocating and re-running the failed attempt. The first init's `SNDSound` is deliberately
    /// leaked on both sides (vanilla's own re-init path leaks it identically - 8 bytes once per battery
    /// run); the teardown below releases only the current `sound_ptr`.
    fn run_menumusichandler_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "MENUMUSICHANDLER_INIT";

        let real_ptr = menumusichandler_live_support::allocate_uninitialized();
        let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(reimpl_ptr);
            }
            return true;
        }

        const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
        const ATTENUATION: i32 = 0;
        let mut failed = false;

        let (real, reimpl) = unsafe { (&*real_ptr, &*reimpl_ptr) };
        let mut mismatches: Vec<String> = Vec::new();

        unsafe {
            menumusichandler_live_support::real_constructor(real_ptr as *const u32);
            (*reimpl_ptr).construct();

            // Phase 1: first init on a fresh constructor.
            let real_result = menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
            let reimpl_result = (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION) as u32;

            if (real_result != 0) != (reimpl_result != 0) {
                mismatches.push(format!("return value: real={real_result}, reimpl={reimpl_result}"));
            }
            mismatches.extend(menumusichandler_field_mismatches(real, reimpl));
            if !mismatches.is_empty() {
                error!("{}: mismatch(es): {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            }

            // Phase 2: second init on the already-init'ed pair (see this test's doc comment).
            if !failed {
                let real_result = menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
                let reimpl_result = (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION) as u32;

                mismatches.clear();
                if (real_result != 0) != (reimpl_result != 0) {
                    mismatches.push(format!("second-init return value: real={real_result}, reimpl={reimpl_result}"));
                }
                mismatches.extend(menumusichandler_field_mismatches(real, reimpl));
                if !mismatches.is_empty() {
                    error!("{}: second-init mismatch(es): {:?}", test_name, mismatches);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: second-init mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
                    }
                    failed = true;
                }
            }
        }

        if !failed {
            write_success_line(failure_log, test_name);
        }

        menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
        menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
        failed
    }

    /// Field-by-field comparison shared by the two Stage-3 `MenuMusicHandler` tests - same comparison set
    /// as `MENUMUSICHANDLER_INIT` (`sound_ptr` by null-ness only, everything else exactly).
    fn menumusichandler_field_mismatches(
        real: &ztgamemgr_menumusichandler::MenuMusicHandler,
        reimpl: &ztgamemgr_menumusichandler::MenuMusicHandler,
    ) -> Vec<String> {
        let mut mismatches: Vec<String> = Vec::new();
        if (real.sound_ptr() != 0) != (reimpl.sound_ptr() != 0) {
            mismatches.push(format!("sound_ptr null-ness: real={:#x}, reimpl={:#x}", real.sound_ptr(), reimpl.sound_ptr()));
        }
        if real.fading() != reimpl.fading() {
            mismatches.push(format!("fading: real={}, reimpl={}", real.fading(), reimpl.fading()));
        }
        if real.fade_counter() != reimpl.fade_counter() {
            mismatches.push(format!("fade_counter: real={}, reimpl={}", real.fade_counter(), reimpl.fade_counter()));
        }
        if real.ini_menu_music_disabled() != reimpl.ini_menu_music_disabled() {
            mismatches.push(format!("ini_menu_music_disabled: real={}, reimpl={}", real.ini_menu_music_disabled(), reimpl.ini_menu_music_disabled()));
        }
        if real.warmup_ticks() != reimpl.warmup_ticks() {
            mismatches.push(format!("warmup_ticks: real={}, reimpl={}", real.warmup_ticks(), reimpl.warmup_ticks()));
        }
        mismatches
    }

    /// Writes marker values through raw field offsets (the struct's fields are private outside
    /// `ztgamemgr_menumusichandler`, and its layout is `const`-asserted): `fading` (+0x4) = `fading`,
    /// `fade_counter` (+0x8) = `fade_counter`, `ini_menu_music_disabled` (+0xc) = `ini_disabled`,
    /// `warmup_ticks` (+0x10) = `warmup`.
    ///
    /// # Safety
    /// `ptr` must point at a live, `0x14`-byte standalone `MenuMusicHandler` allocation.
    unsafe fn write_menumusichandler_markers(ptr: *mut ztgamemgr_menumusichandler::MenuMusicHandler, fading: u8, fade_counter: i32, ini_disabled: u8, warmup: i32) {
        let base = ptr as *mut u8;
        base.add(0x4).write(fading);
        (base.add(0x8) as *mut i32).write(fade_counter);
        base.add(0xc).write(ini_disabled);
        (base.add(0x10) as *mut i32).write(warmup);
    }

    /// `MENUMUSICHANDLER_START_PLAY` - `menumusichandler-implementation-plan.md` Stage 3: constructs and
    /// `init`s two fresh standalone `MenuMusicHandler`s (real vanilla / Rust reimplementation, same
    /// guaranteed-missing filename as `MENUMUSICHANDLER_INIT` - see that test's doc comment for why no
    /// real `.wav` path), then calls `startPlay` on each (real vanilla via the `real_start_play`
    /// trampoline / `MenuMusicHandler::start_play`) and field-diffs. With `attempt` known to have failed, the
    /// `VALID`-gated play branch is skipped on both sides while the unconditional tail (clear
    /// `fading`/`fade_counter`, `SET_FADE_ATTENUATION(0)`/`SET_VOLUME(0)`) still runs - vanilla makes
    /// those last two calls even on a failed-attempt sound, so both sides exercise the same calls.
    /// A second phase then covers `startPlay`'s first gate: with `ini_menu_music_disabled` forced to 1
    /// and non-zero marker values written into `fading`/`fade_counter` on both sides, another `startPlay`
    /// on each must leave both untouched (without the gate, `startPlay` would have cleared them to 0).
    fn run_menumusichandler_start_play_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "MENUMUSICHANDLER_START_PLAY";

        let real_ptr = menumusichandler_live_support::allocate_uninitialized();
        let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(reimpl_ptr);
            }
            return true;
        }

        const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
        const ATTENUATION: i32 = 0;
        let mut failed = false;

        unsafe {
            menumusichandler_live_support::real_constructor(real_ptr as *const u32);
            (*reimpl_ptr).construct();
            menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
            (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION);

            // Phase 1: the normal path (gates open, VALID-gated play branch skipped by the failed
            // attempt, unconditional tail runs).
            menumusichandler_live_support::real_start_play(real_ptr as *const u32);
            (*reimpl_ptr).start_play();
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: startPlay phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: startPlay phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            }

            // Phase 2: the ini gate - forced-disabled plus non-zero markers must survive untouched.
            if !failed {
                write_menumusichandler_markers(real_ptr, 1, 77, 1, 0);
                write_menumusichandler_markers(reimpl_ptr, 1, 77, 1, 0);
                menumusichandler_live_support::real_start_play(real_ptr as *const u32);
                (*reimpl_ptr).start_play();
                let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
                if !mismatches.is_empty() {
                    error!("{}: ini-gate phase mismatches: {:?}", test_name, mismatches);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: ini-gate phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                    }
                    failed = true;
                } else if (*real_ptr).fading() != 1 || (*real_ptr).fade_counter() != 77 || (*real_ptr).ini_menu_music_disabled() != 1 {
                    // Both sides agreeing isn't enough here - the gate's whole point is that nothing
                    // changes, so also check the markers really did survive on the real side (a broken
                    // port and a broken gate would agree with each other just as well as two working
                    // sides would).
                    let msg = format!(
                        "ini-gate markers did not survive: fading={}, fade_counter={}, ini={}",
                        (*real_ptr).fading(), (*real_ptr).fade_counter(), (*real_ptr).ini_menu_music_disabled()
                    );
                    error!("{}: {}", test_name, msg);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                    }
                    failed = true;
                }
            }

            if !failed {
                write_success_line(failure_log, test_name);
            }
        }

        menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
        menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
        failed
    }

    /// `MENUMUSICHANDLER_START_FADE` - `menumusichandler-implementation-plan.md` Stage 3: calls
    /// `startFade` (real vanilla via the `real_start_fade` trampoline / `MenuMusicHandler::start_fade`)
    /// on warm (already-`init`ed)
    /// standalone instances and compares `fading`/`fade_counter`. The positive branch (`IS_PLAYING` true
    /// -> arm the fade) needs genuinely playing audio, so - same missing-filename caveat as
    /// `MENUMUSICHANDLER_INIT`/`_START_PLAY` - only the gate paths are exercised live: with a
    /// failed-attempt sound, `IS_PLAYING` reads false and `startFade` must leave everything untouched;
    /// with `fading` marker-forced to 1, the already-fading gate must also leave everything untouched
    /// (markers checked for survival, not just real-vs-reimpl equality); and a construct-only pair
    /// (`sound_ptr` still 0) covers the null-sound gate.
    fn run_menumusichandler_start_fade_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "MENUMUSICHANDLER_START_FADE";

        let real_ptr = menumusichandler_live_support::allocate_uninitialized();
        let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(reimpl_ptr);
            }
            return true;
        }

        // A second, construct-only pair for the null-`sound_ptr` gate - no `init`, so these tear down
        // through plain `destroy_standalone`.
        let nullgate_real_ptr = menumusichandler_live_support::allocate_uninitialized();
        let nullgate_reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
        if nullgate_real_ptr.is_null() || nullgate_reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null for null-gate pair (real={:?}, reimpl={:?})", test_name, nullgate_real_ptr, nullgate_reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for null-gate pair (real={:?}, reimpl={:?})\n", test_name, nullgate_real_ptr, nullgate_reimpl_ptr).as_bytes());
            }
            if !nullgate_real_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(nullgate_real_ptr);
            }
            if !nullgate_reimpl_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(nullgate_reimpl_ptr);
            }
            menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
            menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
            return true;
        }

        const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
        const ATTENUATION: i32 = 0;
        let mut failed = false;

        unsafe {
            menumusichandler_live_support::real_constructor(real_ptr as *const u32);
            (*reimpl_ptr).construct();
            menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
            (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION);

            // Phase 1: warm instance, sound not playing - the IS_PLAYING gate must keep everything at 0.
            menumusichandler_live_support::real_start_fade(real_ptr as *const u32);
            (*reimpl_ptr).start_fade();
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: not-playing phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: not-playing phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            }

            // Phase 2: the already-fading gate - a marker-forced `fading` = 1 must survive untouched.
            if !failed {
                write_menumusichandler_markers(real_ptr, 1, 0x55, 0, 0);
                write_menumusichandler_markers(reimpl_ptr, 1, 0x55, 0, 0);
                menumusichandler_live_support::real_start_fade(real_ptr as *const u32);
                (*reimpl_ptr).start_fade();
                let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
                if !mismatches.is_empty() {
                    error!("{}: already-fading phase mismatches: {:?}", test_name, mismatches);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: already-fading phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                    }
                    failed = true;
                } else if (*real_ptr).fading() != 1 || (*real_ptr).fade_counter() != 0x55 {
                    let msg = format!("already-fading markers did not survive: fading={}, fade_counter={}", (*real_ptr).fading(), (*real_ptr).fade_counter());
                    error!("{}: {}", test_name, msg);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                    }
                    failed = true;
                }
            }

            // Phase 3: the null-sound gate - construct-only instances (`sound_ptr` still 0).
            menumusichandler_live_support::real_constructor(nullgate_real_ptr as *const u32);
            (*nullgate_reimpl_ptr).construct();
            menumusichandler_live_support::real_start_fade(nullgate_real_ptr as *const u32);
            (*nullgate_reimpl_ptr).start_fade();
            let mismatches = menumusichandler_field_mismatches(&*nullgate_real_ptr, &*nullgate_reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: null-sound phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: null-sound phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            }

            if !failed {
                write_success_line(failure_log, test_name);
            }
        }

        menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
        menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
        menumusichandler_live_support::destroy_standalone(nullgate_real_ptr);
        menumusichandler_live_support::destroy_standalone(nullgate_reimpl_ptr);
        failed
    }

    /// `MENUMUSICHANDLER_UPDATE` - `menumusichandler-implementation-plan.md` Stage 4: constructs and
    /// `init`s two fresh standalone `MenuMusicHandler`s (real vanilla / Rust reimplementation, same
    /// guaranteed-missing filename as the other `MENUMUSICHANDLER_*` tests - see `MENUMUSICHANDLER_INIT`'s
    /// doc comment for why no real `.wav` path), marker-forces `fading` = 1 (the one thing `startFade`
    /// can't arm live without playing audio, per `MENUMUSICHANDLER_START_FADE`'s doc comment), then drives
    /// `update` (real vanilla via the `real_update` trampoline / `MenuMusicHandler::update`) through its
    /// state machine in phases:
    ///
    /// 1. **Warm-up gating**: five `update(1000)` calls must each just increment `warmup_ticks` (0 -> 5)
    ///    and never touch `fade_counter` - checked for survival, not just real-vs-reimpl equality.
    /// 2. **Accumulation**: the sixth `update(1000)` runs the accumulation path -
    ///    `fade_counter = trunc(1000 * 0.5)` = 500 - which also makes both sides push the new counter
    ///    through `SET_FADE_ATTENUATION`/`SET_VOLUME` on their failed-attempt `SNDSound`s, the same call
    ///    shape `MENUMUSICHANDLER_START_PLAY` already exercises live.
    /// 3. **Delta gate**: `update(2000)` (the unsigned `>=` boundary itself) must leave `fade_counter` at
    ///    500.
    /// 4. **Completion branch, not-playing sound**: with `fade_counter` marker-forced to 2995, one
    ///    `update(100)` crosses the 3000 threshold (3045) - and because the failed-attempt sound reports
    ///    not playing, the decompile leaves *everything* untouched (`IS_PLAYING` gates the clears, not the
    ///    other way round; see `MenuMusicHandler::update`'s doc comment on how the plan's "always clear"
    ///    summary misread this). Survival-checked on `fading`/`fade_counter`/`sound_ptr`.
    ///
    /// The `IS_PLAYING`-**true** completion path (`STOP` + slot-0 release + `sound_ptr` = 0) needs genuinely
    /// playing audio to enter live, so it isn't exercised here - its teardown calls are the exact
    /// [`SNDSOUND_1`] release idiom `MENUMUSICHANDLER_INIT`'s teardown path already runs for real.
    fn run_menumusichandler_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "MENUMUSICHANDLER_UPDATE";

        let real_ptr = menumusichandler_live_support::allocate_uninitialized();
        let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                menumusichandler_live_support::destroy_standalone(reimpl_ptr);
            }
            return true;
        }

        const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
        const ATTENUATION: i32 = 0;
        let mut failed = false;

        unsafe {
            menumusichandler_live_support::real_constructor(real_ptr as *const u32);
            (*reimpl_ptr).construct();
            menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
            (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION);

            // Arm the fade marker-wise: `init` leaves `fading` = 0 and only a real playing sound would
            // let `startFade` arm it, so write it directly on both sides.
            write_menumusichandler_markers(real_ptr, 1, 0, 0, 0);
            write_menumusichandler_markers(reimpl_ptr, 1, 0, 0, 0);

            // Phase 1: five warm-up calls - `warmup_ticks` counts 0 -> 5, `fade_counter` untouched.
            for _ in 0..5 {
                menumusichandler_live_support::real_update(real_ptr as *const u32, 1000);
                (*reimpl_ptr).update(1000);
            }
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: warm-up phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: warm-up phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            } else if (*real_ptr).warmup_ticks() != 5 || (*real_ptr).fade_counter() != 0 {
                let msg = format!(
                    "warm-up markers did not advance as expected: warmup_ticks={}, fade_counter={}",
                    (*real_ptr).warmup_ticks(), (*real_ptr).fade_counter()
                );
                error!("{}: {}", test_name, msg);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                }
                failed = true;
            }

            // Phase 2: the sixth call (now warm) accumulates trunc(1000 * 0.5) = 500.
            if !failed {
                menumusichandler_live_support::real_update(real_ptr as *const u32, 1000);
                (*reimpl_ptr).update(1000);
                let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
                if !mismatches.is_empty() {
                    error!("{}: accumulation phase mismatches: {:?}", test_name, mismatches);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: accumulation phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                    }
                    failed = true;
                } else if (*real_ptr).fade_counter() != 500 {
                    let msg = format!("accumulation did not reach 500: fade_counter={}", (*real_ptr).fade_counter());
                    error!("{}: {}", test_name, msg);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                    }
                    failed = true;
                }
            }

            // Phase 3: the delta gate - 2000 (the unsigned boundary itself) must leave everything alone.
            if !failed {
                menumusichandler_live_support::real_update(real_ptr as *const u32, 2000);
                (*reimpl_ptr).update(2000);
                let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
                if !mismatches.is_empty() {
                    error!("{}: delta-gate phase mismatches: {:?}", test_name, mismatches);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: delta-gate phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                    }
                    failed = true;
                } else if (*real_ptr).fade_counter() != 500 {
                    let msg = format!("delta gate did not hold: fade_counter={}", (*real_ptr).fade_counter());
                    error!("{}: {}", test_name, msg);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                    }
                    failed = true;
                }
            }

            // Phase 4: completion branch with a not-playing sound - everything must survive untouched.
            if !failed {
                write_menumusichandler_markers(real_ptr, 1, 2995, 0, 5);
                write_menumusichandler_markers(reimpl_ptr, 1, 2995, 0, 5);
                menumusichandler_live_support::real_update(real_ptr as *const u32, 100);
                (*reimpl_ptr).update(100);
                let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
                if !mismatches.is_empty() {
                    error!("{}: not-playing completion phase mismatches: {:?}", test_name, mismatches);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: not-playing completion phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                    }
                    failed = true;
                } else if (*real_ptr).fading() != 1 || (*real_ptr).fade_counter() != 3045 || (*real_ptr).sound_ptr() == 0 {
                    let msg = format!(
                        "not-playing completion markers did not survive: fading={}, fade_counter={}, sound_ptr={:#x}",
                        (*real_ptr).fading(), (*real_ptr).fade_counter(), (*real_ptr).sound_ptr()
                    );
                    error!("{}: {}", test_name, msg);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                    }
                    failed = true;
                }
            }

            if !failed {
                write_success_line(failure_log, test_name);
            }
        }

        menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
        menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
        failed
    }

    /// `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` - `ztgamemgr-implementation-plan.md` Stage 1: builds two
    /// standalone `ZTGameMgr` instances (Stage 0's harness), runs the real
    /// `SET_NEW_GAME_DEFAULTS.original()` against one and the Rust
    /// `ztgamemgr::ZTGameMgr::set_new_game_defaults` against the other (one shared, real vanilla-
    /// constructed `BFConfigFile` passed to both - see below for why a zeroed/`Default` one crashes),
    /// then diffs the full `0x11b0`-byte block.
    ///
    /// **`config` must be built via the real vanilla constructor, not a zeroed `BFConfigFile::default()`.**
    /// First attempt used a zeroed instance and reliably crashed inside vanilla `ZooStatus::init`'s
    /// tail call into `BFConfigFile::getString` (`bfconfigfile::GET_STRING_1`) ->
    /// `standalone::SEARCH_CONFIG_METHOD`, a null-pointer dereference (`mov edi,[edx+4]` with `edx=0`,
    /// confirmed via `./openzt.bat crash-capture`). `BFConfigFile_BFConfigFile_0.c` shows why: a real
    /// constructor allocates a red-black-tree sentinel node and links it to itself
    /// (`node->left = node; node->right = node`) as `tree_root` - a *raw* `0` there (what `#[derive(Default)]`
    /// produces) isn't a valid "empty tree", it's a dangling sentinel the search code doesn't guard against.
    /// So this builds a real one via `BFCONFIGFILE_CONSTRUCTOR_0.original()` and tears it down via
    /// `BFCONFIGFILE_RELEASE.original()` - matching the real `BFConfigFile::BFConfigFile`/`::release`
    /// pair, entirely vanilla-allocator-owned (its tree node comes from vanilla's own small-object
    /// freelist - see `BFConfigFile_BFConfigFile_0.c`'s `FUN_00402f85`/freelist-pop shape), so there's no
    /// cross-allocator hazard freeing it via the matching real `release` call.
    ///
    /// `is_new_game` is pinned to `false` on both sides rather than proptested: the `true` branch calls
    /// through `GLOBAL_ZTAIMgr`'s real vtable slot `+0x4` (`openzt_detour::generated::ztaimgr::VIRT_METH_0X58F269`),
    /// the *global*, shared AI manager singleton - not part of either standalone instance's own memory -
    /// so triggering it here would be a real side effect on live game state, the same class of risk the
    /// plan's own Stage 3 flags for `ZTUI::main::set*`/`ZTSoundscape::update`.
    fn run_gamemgr_set_new_game_defaults_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS";

        let real_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
            }
            return true;
        }

        // Resolves Stage 0's own "operator_new doesn't zero memory" caveat: ZooStatus::init reads at
        // least one field (`this[0xd].field_0xc`, per `ZooStatus_init.c`) before ever writing it in this
        // function - genuine uninitialized-read behavior in the real decompile, not a porting bug - so
        // two independently-allocated standalone instances can carry different heap leftovers there and
        // diverge downstream. Zeroing both blocks first (matching a fresh page from a clean process heap,
        // the same assumption vanilla's own single real construction relies on) makes both sides start
        // identical, so the diff below only ever reflects a genuine `set_new_game_defaults` difference.
        let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
        }

        let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
        let config_ptr = config.as_mut_ptr() as *const u32;
        let kind_tag_byte: u8 = 0;
        unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };

        unsafe {
            ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
            (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
        }

        unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

        let real_bytes = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) };

        // soundscape_ptr (0x1190)/menu_music_handler_ptr (0x11A4): both null pre-start() on a freshly
        // constructed instance, so these should already match - excluded only defensively, per the plan.
        let excluded_ranges: [std::ops::Range<usize>; 2] = [0x1190..0x1194, 0x11A4..0x11A8];

        let mismatches: Vec<(usize, u8, u8)> = (0..struct_size)
            .filter(|i| !excluded_ranges.iter().any(|r| r.contains(i)))
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
        failed
    }

    /// `ZOOSTATUS_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
    /// `zoostatus::init()`, and this asserts all 36 of its detours actually report enabled (see
    /// `zoostatus.rs`'s own `zoostatus_detours` module doc comment for the full list and for the three
    /// addresses deliberately left un-hooked). Without it, a silently-failed `init_detours()` (error
    /// logged, game continues on vanilla) would leave the whole battery green while every hooked
    /// production path runs vanilla - the trampoline-based comparisons below can't distinguish that from
    /// a working hook, mirroring `MENUMUSICHANDLER_DETOURS_ENABLED`'s own doc comment. Runs before the
    /// other `ZOOSTATUS_*` tests so a wiring failure is visible first.
    fn run_zoostatus_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_DETOURS_ENABLED";
        let mut disabled: Vec<&'static str> = Vec::new();
        for (name, enabled) in zoostatus_live_support::detour_status() {
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

    /// `ZOOSTATUS_ORIGINAL_ROUTES_TO_TRAMPOLINE` - debug-only anti-regression for `openzt-detour`'s
    /// hook registry, mirroring `MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE`'s own doc comment:
    /// `FunctionDef::original()` must return the *real vanilla* function (routed through the detour's
    /// trampoline) even for the 36 addresses this battery has itself hooked, not silently re-enter our
    /// own Rust detours. For each of them, asserts the registry holds a trampoline, that `.original()`
    /// returns exactly that pointer value, and that it differs from the raw address (`zoo.exe` has no
    /// ASLR, so an un-routed raw cast would compare equal - pointer equality can't pass vacuously here).
    /// Also asserts zero registry overflows: a full slot array fails open into exactly the raw-cast
    /// behavior this test guards against. Release builds cfg this out (the raw cast is release's
    /// documented `.original()`); the release battery is still run once-off since every existing
    /// `ZOOSTATUS_*` comparison test's vanilla pole goes through `.original()` directly rather than a
    /// `real_*` trampoline (none was needed - see the `crate::zoostatus::init()` call site's own comment).
    #[cfg(debug_assertions)]
    fn run_zoostatus_original_routes_to_trampoline_test(failure_log: &mut Option<std::fs::File>) -> bool {
        use openzt_detour::generated::zoostatus as zs;

        /// `.original()`'s return value as a raw pointer value. The pointer is only inspected,
        /// never called.
        fn original_ptr<T>(def: &FunctionDef<T>) -> usize
        where
            T: retour::Function,
        {
            let original = unsafe { def.original() };
            original.to_ptr() as usize
        }

        let test_name = "ZOOSTATUS_ORIGINAL_ROUTES_TO_TRAMPOLINE";
        let hooked: [(&'static str, u32, usize); 36] = [
            ("INIT", zs::INIT.address, original_ptr(&zs::INIT)),
            ("OVERRIDE", zs::OVERRIDE.address, original_ptr(&zs::OVERRIDE)),
            ("RESET_FINANCE_INFO", zs::RESET_FINANCE_INFO.address, original_ptr(&zs::RESET_FINANCE_INFO)),
            ("SPEND_CONSTRUCTION", zs::SPEND_CONSTRUCTION.address, original_ptr(&zs::SPEND_CONSTRUCTION)),
            ("SPEND_BUILDING_UPKEEP", zs::SPEND_BUILDING_UPKEEP.address, original_ptr(&zs::SPEND_BUILDING_UPKEEP)),
            ("SPEND_GUIDE_WAGES", zs::SPEND_GUIDE_WAGES.address, original_ptr(&zs::SPEND_GUIDE_WAGES)),
            ("BUY_ANIMAL", zs::BUY_ANIMAL.address, original_ptr(&zs::BUY_ANIMAL)),
            ("SPEND_KEEPER_WAGES", zs::SPEND_KEEPER_WAGES.address, original_ptr(&zs::SPEND_KEEPER_WAGES)),
            ("SPEND_MAINT_WAGES", zs::SPEND_MAINT_WAGES.address, original_ptr(&zs::SPEND_MAINT_WAGES)),
            ("SPEND_MARKETING", zs::SPEND_MARKETING.address, original_ptr(&zs::SPEND_MARKETING)),
            ("SPEND_RESEARCH", zs::SPEND_RESEARCH.address, original_ptr(&zs::SPEND_RESEARCH)),
            ("REFUND_ANIMAL_COST", zs::REFUND_ANIMAL_COST.address, original_ptr(&zs::REFUND_ANIMAL_COST)),
            ("REFUND_CONSTRUCTION", zs::REFUND_CONSTRUCTION.address, original_ptr(&zs::REFUND_CONSTRUCTION)),
            ("INCREASE_DONATIONS", zs::INCREASE_DONATIONS.address, original_ptr(&zs::INCREASE_DONATIONS)),
            ("INCREASE_ENDOWMENT", zs::INCREASE_ENDOWMENT.address, original_ptr(&zs::INCREASE_ENDOWMENT)),
            ("INCREASE_SHOW_ADMISSION", zs::INCREASE_SHOW_ADMISSION.address, original_ptr(&zs::INCREASE_SHOW_ADMISSION)),
            ("BUY_PEOPLE_FOOD", zs::BUY_PEOPLE_FOOD.address, original_ptr(&zs::BUY_PEOPLE_FOOD)),
            ("CHANGE_ENDOWMENT_MEMBERS", zs::CHANGE_ENDOWMENT_MEMBERS.address, original_ptr(&zs::CHANGE_ENDOWMENT_MEMBERS)),
            ("ANIMAL_ESCAPED", zs::ANIMAL_ESCAPED.address, original_ptr(&zs::ANIMAL_ESCAPED)),
            ("ADMISSION_MESSAGE", zs::ADMISSION_MESSAGE.address, original_ptr(&zs::ADMISSION_MESSAGE)),
            ("NEWGUEST_CHECKS", zs::NEWGUEST_CHECKS.address, original_ptr(&zs::NEWGUEST_CHECKS)),
            ("MESSAGE_CHECKS", zs::MESSAGE_CHECKS.address, original_ptr(&zs::MESSAGE_CHECKS)),
            ("RATING_CHECKS", zs::RATING_CHECKS.address, original_ptr(&zs::RATING_CHECKS)),
            ("F_GRANT_DONATION", zs::F_GRANT_DONATION.address, original_ptr(&zs::F_GRANT_DONATION)),
            ("F_ZOO_MESSAGE", zs::F_ZOO_MESSAGE.address, original_ptr(&zs::F_ZOO_MESSAGE)),
            ("SET_ADULT_ADMISSION_PRICE", zs::SET_ADULT_ADMISSION_PRICE.address, original_ptr(&zs::SET_ADULT_ADMISSION_PRICE)),
            ("SHOW_PRICES", zs::SHOW_PRICES.address, original_ptr(&zs::SHOW_PRICES)),
            ("CALCULATE_SUMS", zs::CALCULATE_SUMS.address, original_ptr(&zs::CALCULATE_SUMS)),
            ("UPDATE", zs::UPDATE.address, original_ptr(&zs::UPDATE)),
            ("SAVE", zs::SAVE.address, original_ptr(&zs::SAVE)),
            ("LOAD", zs::LOAD.address, original_ptr(&zs::LOAD)),
            ("HEAL_ANIMAL", zs::HEAL_ANIMAL.address, original_ptr(&zs::HEAL_ANIMAL)),
            ("PURCHASE_FOOD", zs::PURCHASE_FOOD.address, original_ptr(&zs::PURCHASE_FOOD)),
            ("INCREASE_ADMISSIONS_INCOME", zs::INCREASE_ADMISSIONS_INCOME.address, original_ptr(&zs::INCREASE_ADMISSIONS_INCOME)),
            ("INCREASE_ADMISSIONS", zs::INCREASE_ADMISSIONS.address, original_ptr(&zs::INCREASE_ADMISSIONS)),
            ("GET_STATUS_FIXED", GET_STATUS_FIXED.address, original_ptr(&GET_STATUS_FIXED)),
        ];

        let mut failures: Vec<String> = Vec::new();
        for (name, address, original) in hooked {
            match openzt_detour::trampoline_for(address) {
                Some(trampoline) => {
                    if original != trampoline {
                        failures.push(format!(
                            "{name} ({address:#010x}): .original() = {original:#010x} != registered trampoline {trampoline:#010x}"
                        ));
                    }
                    if original == address as usize {
                        failures.push(format!(
                            "{name} ({address:#010x}): .original() equals the raw address - routing fell back to the raw cast"
                        ));
                    }
                }
                None => failures.push(format!(
                    "{name} ({address:#010x}): no trampoline registered - detour() did not publish, or the registry overflowed"
                )),
            }
        }
        let overflow = openzt_detour::registry_overflow_count();
        if overflow != 0 {
            failures.push(format!("{overflow} address(es) failed to register in the hook registry (capacity overflow - fail-open raw casts)"));
        }

        if failures.is_empty() {
            write_success_line(failure_log, test_name);
            false
        } else {
            for msg in &failures {
                error!("{}: {}", test_name, msg);
            }
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes(),
                );
            }
            true
        }
    }

    /// `ZOOSTATUS_INIT` - `zoostatus-implementation-plan.md` Stage 2's live comparison: builds two
    /// standalone `ZTGameMgr` blocks (same harness `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` uses), zeroes each
    /// one's *embedded `ZooStatus` sub-region only* (`+0x10..+0x1190`, matching that test's own
    /// "`ZooStatus::init` reads at least one field before ever writing it" finding -
    /// [`crate::zoostatus::ZooStatus::admission_price`] here specifically, never written before
    /// `setAdultAdmissionPrice` reads it back), then runs real vanilla `ZOOSTATUS_INIT.original()`
    /// against one and [`ZooStatus::init`] against the other, both with a null config pointer.
    ///
    /// **Null config is deliberately safe, not a shortcut**: `ZooStatus::override`'s decompile
    /// (`ZooStatus_override.c`, read in full) null-checks its `param_1` as the very first statement and
    /// returns immediately otherwise - so passing null exercises `init`'s own unconditional writes plus
    /// a real, genuinely-taken call into `override`/`setAdultAdmissionPrice` (not skipped), without
    /// needing a real `BFConfigFile` instance built for this test the way
    /// `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` needs one.
    ///
    /// Masked byte ranges (documented, not silently swallowed):
    /// - `+0x68..+0x6c` (`max_guests`): `ZooStatus::init`'s own `BFIniFile::read` of `AI`/`maxGuests` is
    ///   a real, untouched dependency (constructing its `std::string` arguments - see `ztgamemgr.rs`'s
    ///   `initMenuMusic` doc comment for the same class of gap) - [`ZooStatus::init`] hardcodes the
    ///   vanilla default (`1000`) instead. Masked defensively; in practice the real pole should read the
    ///   same default unless the live environment's ini actually overrides this key.
    /// - `+0x1178..+0x1180` (`last_animal_escape_timestamp_*`): both poles call the real
    ///   [`GET_OLD_DATE`](openzt_detour::generated::standalone::GET_OLD_DATE) independently, a couple of
    ///   CPU cycles apart - genuinely time-dependent, not a porting bug.
    fn run_zoostatus_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_INIT";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            ZOOSTATUS_INIT.original()(real_zoostatus_ptr as *const u32, std::ptr::null());
            (*reimpl_zoostatus_ptr).init(std::ptr::null());
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let excluded_ranges: [std::ops::Range<usize>; 2] = [0x68..0x6c, 0x1178..0x1180];

        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter(|i| !excluded_ranges.iter().any(|r| r.contains(i)))
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_ACCUMULATORS` - `zoostatus-implementation-plan.md` Stage 3's live comparison: builds
    /// two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT` uses), zeroes each one's embedded
    /// `ZooStatus` sub-region, then seeds [`ZooStatus::current_month_index`]/
    /// [`ZooStatus::current_year_index`] to non-default values (`5`/`7`, not `init`'s own `1`/`0`)
    /// directly via raw offset writes on both instances identically - deliberately exercising the
    /// dynamic `LEA [ECX+EAX*4+<offset>]` index-scaling every one of these 15 methods does, not just the
    /// zero-index case `init`'s own default would otherwise leave untested.
    ///
    /// Runs every one of the 14 "simple accumulator" methods' real `.original()` against one instance and
    /// the matching [`ZooStatus`] method against the other, with the same varied (including negative and
    /// fractional) `f32` amount each time, then [`ZooStatus::change_endowment_members`] three times
    /// (positive/negative/zero delta) to cover its three-way branch - both real
    /// [`ZOOSTATUS_CHANGE_ENDOWMENT_MEMBERS`] and the Rust port take the same `i32` sequence. A single
    /// full-struct byte comparison at the end catches any divergence across the whole run (no masking
    /// needed - none of these methods touch [`ZooStatus::admission_price`]/the escape timestamp, the only
    /// fields `ZOOSTATUS_INIT`'s own test had to mask).
    ///
    /// **Extended (Stage 10)** with the five real Windows methods a fresh Ghidra pass recovered from the
    /// macOS-only corpus: [`ZooStatus::buy_animal`] (the renamed, previously-mislabeled
    /// `SPEND_KEEPER_WAGES_0`), [`ZooStatus::heal_animal`], [`ZooStatus::purchase_food`],
    /// [`ZooStatus::increase_admissions_income`] (all four fit this same shared-instance byte-diff
    /// harness, same as every other accumulator method) and [`ZooStatus::increase_admissions`] (the
    /// `i32`-count outlier, run once here rather than getting its own test - same reasoning as
    /// `change_endowment_members` sharing this harness instead of a dedicated one).
    /// [`ZooStatus::get_status`] (the sixth recovered method, a pure reader) has its own dedicated test,
    /// `ZOOSTATUS_GET_STATUS`, since it doesn't fit this write-and-diff shape.
    fn run_zoostatus_accumulators_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_ACCUMULATORS";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
                save_to_memory(ptr as u32 + 0x14c, 5i32);
                save_to_memory(ptr as u32 + 0x150, 7i32);
            }

            ZOOSTATUS_SPEND_CONSTRUCTION.original()(real_zoostatus_ptr as *const u32, 1234.5);
            (*reimpl_zoostatus_ptr).spend_construction(1234.5);
            ZOOSTATUS_SPEND_BUILDING_UPKEEP.original()(real_zoostatus_ptr as *const u32, 42.25);
            (*reimpl_zoostatus_ptr).spend_building_upkeep(42.25);
            ZOOSTATUS_SPEND_GUIDE_WAGES.original()(real_zoostatus_ptr as *const u32, 99.0);
            (*reimpl_zoostatus_ptr).spend_guide_wages(99.0);
            ZOOSTATUS_BUY_ANIMAL.original()(real_zoostatus_ptr as *const u32, 17.5);
            (*reimpl_zoostatus_ptr).buy_animal(17.5);
            ZOOSTATUS_HEAL_ANIMAL.original()(real_zoostatus_ptr as *const u32, 8.25);
            (*reimpl_zoostatus_ptr).heal_animal(8.25);
            ZOOSTATUS_PURCHASE_FOOD.original()(real_zoostatus_ptr as *const u32, 4.5);
            (*reimpl_zoostatus_ptr).purchase_food(4.5);
            ZOOSTATUS_INCREASE_ADMISSIONS_INCOME.original()(real_zoostatus_ptr as *const u32, 320.0);
            (*reimpl_zoostatus_ptr).increase_admissions_income(320.0);
            ZOOSTATUS_INCREASE_ADMISSIONS.original()(real_zoostatus_ptr as *const u32, 7);
            (*reimpl_zoostatus_ptr).increase_admissions(7);
            ZOOSTATUS_SPEND_KEEPER_WAGES_1.original()(real_zoostatus_ptr as *const u32, 250.0);
            (*reimpl_zoostatus_ptr).spend_keeper_wages_1(250.0);
            ZOOSTATUS_SPEND_MAINT_WAGES.original()(real_zoostatus_ptr as *const u32, 3.75);
            (*reimpl_zoostatus_ptr).spend_maint_wages(3.75);
            ZOOSTATUS_SPEND_MARKETING.original()(real_zoostatus_ptr as *const u32, 500.0);
            (*reimpl_zoostatus_ptr).spend_marketing(500.0);
            ZOOSTATUS_SPEND_RESEARCH.original()(real_zoostatus_ptr as *const u32, 1000.0);
            (*reimpl_zoostatus_ptr).spend_research(1000.0);
            ZOOSTATUS_REFUND_ANIMAL_COST.original()(real_zoostatus_ptr as *const u32, 60.0);
            (*reimpl_zoostatus_ptr).refund_animal_cost(60.0);
            ZOOSTATUS_REFUND_CONSTRUCTION.original()(real_zoostatus_ptr as *const u32, 80.0);
            (*reimpl_zoostatus_ptr).refund_construction(80.0);
            ZOOSTATUS_INCREASE_DONATIONS.original()(real_zoostatus_ptr as *const u32, 25.0);
            (*reimpl_zoostatus_ptr).increase_donations(25.0);
            ZOOSTATUS_INCREASE_ENDOWMENT.original()(real_zoostatus_ptr as *const u32, 5000.0);
            (*reimpl_zoostatus_ptr).increase_endowment(5000.0);
            ZOOSTATUS_INCREASE_SHOW_ADMISSION.original()(real_zoostatus_ptr as *const u32, 12.0);
            (*reimpl_zoostatus_ptr).increase_show_admission(12.0);
            ZOOSTATUS_BUY_PEOPLE_FOOD.original()(real_zoostatus_ptr as *const u32, 6.5);
            (*reimpl_zoostatus_ptr).buy_people_food(6.5);

            for delta in [3i32, -4i32, 0i32] {
                ZOOSTATUS_CHANGE_ENDOWMENT_MEMBERS.original()(real_zoostatus_ptr as *const u32, delta);
                (*reimpl_zoostatus_ptr).change_endowment_members(delta);
            }
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_GET_STATUS` - `zoostatus-implementation-plan.md` Stage 10's live comparison for
    /// [`ZooStatus::get_status`], the one recovered macOS-only method that's a pure reader rather than a
    /// mutator, so it doesn't fit `ZOOSTATUS_ACCUMULATORS`' write-and-diff harness. Builds a single
    /// standalone `ZTGameMgr` block (same harness the other `ZOOSTATUS_*` tests use), zeroes its embedded
    /// `ZooStatus` sub-region, then fills every 4-byte word of the whole struct with a distinct non-zero
    /// float (`index + 0.5`) - not representative game data, just enough that a wrong offset/stride reads
    /// back a detectably different value instead of coincidentally matching a zeroed neighbor - and seeds
    /// [`ZooStatus::current_month_index`]/[`ZooStatus::current_year_index`] to non-default values (`5`/`7`,
    /// matching `ZOOSTATUS_ACCUMULATORS`' own convention) to exercise the `index == -1` default-cursor
    /// path for real.
    ///
    /// Calls real vanilla [`GET_STATUS_FIXED`] (the locally-corrected `FunctionDef` - see its own doc
    /// comment for why `generated.rs`'s own `GET_STATUS` entry can't be used) and [`ZooStatus::get_status`]
    /// against the *same* memory for a spread of `(category, when, index)` triples: monthly (`when = 0`)
    /// and yearly (`when = 1`) each with an explicit `index` and with `index = -1` (default-cursor), at
    /// category `0` (the row-0 discrepancy case - see [`ZooStatus::get_status`]'s own doc comment), a
    /// mid-range category, and category `30` (the last row, right up against the region boundary); flat
    /// (`when = 2`) at a few categories; and `when = 3` to hit the real fallback branch (returns
    /// `zoostatus.rs`'s `raw_globals::ATTENDANCE_VS_RESEARCH_THRESHOLD_RVA` global unconditionally).
    /// Every category/index value here stays within the struct's real bounds (verified by hand against
    /// `get_status`'s own addressing - see that method's doc comment), so this never reads outside the
    /// allocated block.
    fn run_zoostatus_get_status_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_GET_STATUS";

        let mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let zoostatus_ptr = (mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(zoostatus_ptr as *mut u8, 0, zoostatus_size);
            for i in 0..(zoostatus_size / 4) {
                save_to_memory(zoostatus_ptr as u32 + (i * 4) as u32, (i as f32) + 0.5);
            }
            save_to_memory(zoostatus_ptr as u32 + 0x14c, 5i32);
            save_to_memory(zoostatus_ptr as u32 + 0x150, 7i32);
        }

        let cases: &[(i32, i32, i32)] = &[
            (0, 0, 0),
            (0, 0, -1),
            (5, 0, 3),
            (30, 0, 11),
            (0, 1, 0),
            (0, 1, -1),
            (5, 1, 12),
            (30, 1, 19),
            (0, 2, -1),
            (5, 2, -1),
            (30, 2, -1),
            (0, 3, -1),
        ];

        let mut mismatches: Vec<(i32, i32, i32, f32, f32)> = Vec::new();
        for &(category, when, index) in cases {
            let real = unsafe { GET_STATUS_FIXED.original()(zoostatus_ptr as *const u32, category, when, index) };
            let reimpl = unsafe { (*zoostatus_ptr).get_status(category, when, index) };
            if real != reimpl {
                mismatches.push((category, when, index, real, reimpl));
            }
        }

        let failed = !mismatches.is_empty();
        if failed {
            error!("{}: {} mismatch(es) (category, when, index, real, reimpl): {:?}", test_name, mismatches.len(), mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} mismatch(es): {:?}\n", test_name, mismatches.len(), mismatches).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_NEWGUEST_CHECKS_SMOKE` - `zoostatus-implementation-plan.md` Stage 4's live coverage for
    /// [`ZooStatus::newguest_checks`]. Upgraded from a crash/hang-only smoke test to a real full-struct
    /// byte comparison against real vanilla `ZOOSTATUS_NEWGUEST_CHECKS.original()`, by forcing
    /// [`Self::guest_type_arrival_multiplier`] to `[0; 5]` on both poles - whichever of the five entries
    /// [`Self::newguest_dispatch`] would otherwise select, this pins `F_CHANCE`'s roll parameter to `0`,
    /// which (per [`Self::newguest_checks`]'s own doc comment on the `& 0xff == 0` masking) deterministically
    /// fails on both poles regardless of the real, shared `fChance` RNG stream's actual draw - so
    /// `F_CREATE_GUEST` (the cross-allocator freelist hazard) and [`Self::admission_message`]'s real UI
    /// path (the hang this test's earlier smoke-only form was added to guard against - see
    /// `ZOOSTATUS_CHECKS`'s own doc comment for the incident) are both **provably never reached**, on
    /// either pole, independent of live `GLOBAL_ZTHabitatMgr`/`GLOBAL_ZTMarketingMgr`/escaped-animal-list
    /// state. Both poles still read that same live state identically, so whichever branch it steers them
    /// into (early return on no occupied habitats/an escaped animal, or the full price-tier write +
    /// band/tier dispatch), they take the *same* branch - giving a genuine byte-for-byte comparison of
    /// [`Self::newguest_check_elapsed`]'s reset and [`Self::field_0x48`]'s tier write whenever live state
    /// happens to reach that path, and of the early-return no-op otherwise. The band/tier dispatch
    /// *table* itself stays additionally covered independent of live state by `#[cfg(test)]`'s
    /// `price_tier_matches_the_confirmed_boundary_chain`/`newguest_dispatch_matches_the_derived_band_tier_table`.
    fn run_zoostatus_newguest_checks_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_NEWGUEST_CHECKS_SMOKE";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
                (*ptr).init(std::ptr::null());
                // Pins F_CHANCE's roll parameter to 0 - see this function's own doc comment.
                (*ptr).guest_type_arrival_multiplier = [0; 5];
            }

            ZOOSTATUS_NEWGUEST_CHECKS.original()(real_zoostatus_ptr as *const u32);
            (*reimpl_zoostatus_ptr).newguest_checks();
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_CHECKS` - `zoostatus-implementation-plan.md` Stage 4's live comparison for the two
    /// fully-native methods this stage ports, [`ZooStatus::message_checks`]/[`ZooStatus::rating_checks`].
    /// Builds two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS`
    /// use), zeroes each one's embedded `ZooStatus` sub-region, seeds both identically with representative
    /// values for every field these two methods read (guest/animal counts, message thresholds,
    /// `species_rating_cap`, `field_0x4c`, a past escape timestamp, non-default
    /// `current_month_index`/`current_year_index`), then runs real vanilla
    /// `MESSAGE_CHECKS.original()`/`RATING_CHECKS.original()` against one and
    /// [`ZooStatus::message_checks`]/[`ZooStatus::rating_checks`] against the other, in the same order
    /// vanilla's own `update` would call them.
    ///
    /// `rating_checks` calls [`ZooStatus::calculate_sums`] (native as of Stage 5) - the real pole's
    /// `RATING_CHECKS.original()` calls real vanilla `calculateSums` internally, and the reimpl pole's
    /// `rating_checks()` calls the Rust port; since that call only reads live global manager state
    /// (`GLOBAL_ZTWorldMgr`/`GLOBAL_ZTHabitatMgr`/`GLOBAL_ZTResearchMgr`) and never touches anything but
    /// `this`, and both poles read the exact same live globals, the many counters it overwrites
    /// (`num_animals`/`num_species`/the guest-condition counters/`non_blank_tile_fraction`/
    /// `research_completion_percent`/etc.) end up byte-identical without needing to be pre-seeded here -
    /// this is, incidentally, a second live comparison point for [`ZooStatus::calculate_sums`] beyond
    /// `ZOOSTATUS_CALCULATE_SUMS`'s own dedicated test below.
    ///
    /// Masked byte range (documented, not silently swallowed): `+0x1178..+0x1180`
    /// (`last_animal_escape_timestamp_*`) - only actually touched if the live escaped-animal list happens
    /// to be non-empty at test time, in which case both poles independently call the real
    /// `ZTGameMgr::getDate`/[`ZooStatus::animal_escaped`] a couple of CPU cycles apart (same genuinely
    /// time-dependent masking `ZOOSTATUS_INIT` already applies to the same field).
    ///
    /// **`message_checks` deliberately never triggers a real `fZooMessage` here** - every threshold-check
    /// input above is chosen so none of its eight frequency checks or two guest-rating-band checks
    /// evaluate true, and the live `ZTGameMgr` singleton's cash is temporarily pinned comfortably positive
    /// (`with_ztgamemgr_cash`, restored afterward) to keep its own three cash-band checks quiet too. A
    /// first draft of this test picked a `guest_rating_metric`/`field_0xfc` pair that satisfied one of
    /// those checks, which called real vanilla `BFUIMgr::displayMessage` against the live game and hung
    /// the whole reimplementation-test run (a real vanilla UI call working as intended against synthetic
    /// test data, not a bug in this port) - this test exists to compare arithmetic, not to exercise real
    /// UI side effects.
    fn run_zoostatus_checks_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_CHECKS";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
                let status = &mut *ptr;
                status.num_animals = 40;
                status.animal_condition_counter_1 = 4;
                status.num_species = 12;
                status.species_rating_cap = 44;
                status.num_tired_guests = 3;
                status.num_hungry_guests = 5;
                status.num_thirst_guests = 2;
                status.num_guests_restroom_need = 6;
                status.guest_condition_counter_1 = 1;
                status.guest_condition_counter_2 = 2;
                status.guest_tile_count = 15;
                status.field_0x4c = 12000;
                status.animal_rating_metric = 20;
                // Kept clear of field_0xf4/field_0xfc below so no `fZooMessage` fires - see this
                // function's own doc comment.
                status.guest_rating_metric = 50;
                status.non_blank_tile_fraction = 0.6;
                status.message_threshold_0x70 = 0.5;
                status.message_threshold_0x74 = 0.5;
                status.message_threshold_0x7c = 0.5;
                status.message_threshold_0x84 = 0.5;
                status.message_threshold_0x8c = 0.5;
                status.message_threshold_0x94 = 0.5;
                status.message_threshold_0xa0 = 0.5;
                status.message_threshold_0xa8 = 0.5;
                status.current_month_index = 5;
                status.current_year_index = 7;
                // A plausible past escape timestamp - a real FILETIME comfortably before "now", so
                // rating_checks' hours-since-escape decay term exercises a real, non-zero value.
                status.last_animal_escape_timestamp_low = 0;
                status.last_animal_escape_timestamp_high = 0x01c00000;

                let base = ptr as u32;
                save_to_memory(base + 0xf4, 200i32);
                save_to_memory(base + 0xfc, -50i32);
            }

            // Pin the live ZTGameMgr singleton's cash comfortably positive (avoiding message_checks'
            // three cash-band `fZooMessage` branches - see the seed-value comment above) and restore it
            // afterward, matching the established `with_ztgamemgr_cash` convention.
            marketing_live_support::with_ztgamemgr_cash(50000.0, || {
                ZOOSTATUS_MESSAGE_CHECKS.original()(real_zoostatus_ptr as *const u32);
                (*reimpl_zoostatus_ptr).message_checks();
            });

            ZOOSTATUS_RATING_CHECKS.original()(real_zoostatus_ptr as *const u32);
            (*reimpl_zoostatus_ptr).rating_checks();
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let excluded_ranges: [std::ops::Range<usize>; 1] = [0x1178..0x1180];

        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter(|i| !excluded_ranges.iter().any(|r| r.contains(i)))
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_F_GRANT_DONATION_NO_OP` - live comparison for [`ZooStatus::f_grant_donation`]'s
    /// `AlreadyPastBound` branch (see `zoostatus.rs`'s own `donation_outcome` doc comment for the
    /// three-way split it dispatches through). This is deliberately the *only* branch of that function
    /// this battery compares live: the other two (`BoundJustCrossed`/`Grant`) both call through to real
    /// vanilla `BFUIMgr::displayMessage`, which `ZOOSTATUS_CHECKS`'s own doc comment already documents
    /// hanging the whole reimplementation-test run when accidentally reached live - not a risk worth
    /// re-taking here. The two message-firing branches instead get direct, live-call-free coverage from
    /// `donation_outcome`'s own `#[cfg(test)]` unit test in `zoostatus.rs`.
    ///
    /// Builds two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS`
    /// use), zeroes each one's embedded `ZooStatus` sub-region, seeds `donation_count_this_period`/
    /// `donation_count_bound` on both so the post-increment count (`11.0`) is neither `<= bound` (`3`) nor
    /// exactly `bound + 1` (`4`) - guaranteeing the silent no-op branch and nothing else - then runs real
    /// vanilla `F_GRANT_DONATION.original()` against one and [`ZooStatus::f_grant_donation`] against the
    /// other, with a full-struct byte comparison after. No masking needed: this branch touches nothing but
    /// `donation_count_this_period` itself, on `self` - never the live `GLOBAL_ZTGameMgr` singleton.
    fn run_zoostatus_f_grant_donation_no_op_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_F_GRANT_DONATION_NO_OP";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
                let status = &mut *ptr;
                status.donation_count_this_period = 10.0;
                status.donation_count_bound = 3;
            }

            ZOOSTATUS_F_GRANT_DONATION.original()(real_zoostatus_ptr as *const u32);
            (*reimpl_zoostatus_ptr).f_grant_donation();
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_PRICING` - `zoostatus-implementation-plan.md` Stage 5's live comparison for
    /// [`ZooStatus::set_adult_admission_price`]. Builds two standalone `ZTGameMgr` blocks (same harness
    /// `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS` use), zeroes each one's embedded `ZooStatus` sub-region,
    /// seeds `admission_price_min`/`_max` identically on both, then runs real vanilla
    /// `SET_ADULT_ADMISSION_PRICE.original()` against one and [`ZooStatus::set_adult_admission_price`]
    /// against the other across five representative prices in one sequence (below min, at min, mid-range,
    /// at max, above max - exercising every branch of the clamp), with a full-struct byte comparison
    /// after each call. No masking needed - this method touches nothing but `admission_price` itself.
    fn run_zoostatus_pricing_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_PRICING";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
                let status = &mut *ptr;
                status.admission_price_min = 10.0;
                status.admission_price_max = 100.0;
            }

            for price in [1.0f32, 10.0, 55.0, 100.0, 250.0] {
                ZOOSTATUS_SET_ADULT_ADMISSION_PRICE.original()(real_zoostatus_ptr as *const u32, price);
                (*reimpl_zoostatus_ptr).set_adult_admission_price(price);
            }
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_CALCULATE_SUMS` - `zoostatus-implementation-plan.md` Stage 5's dedicated live
    /// comparison for [`ZooStatus::calculate_sums`] (see also `ZOOSTATUS_CHECKS`'s own doc comment for a
    /// second, incidental comparison point via `rating_checks`). Builds two standalone `ZTGameMgr`
    /// blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS` use), zeroes each one's embedded
    /// `ZooStatus` sub-region, seeds non-default `current_month_index`/`current_year_index` (`4`/`6`) on
    /// both identically (exercising the dynamic history-slot addressing the same way
    /// `ZOOSTATUS_ACCUMULATORS` does), then runs real vanilla `CALCULATE_SUMS.original()` against one and
    /// [`ZooStatus::calculate_sums`] against the other, with a full-struct byte comparison. No masking
    /// needed: `calculate_sums` only ever reads live global manager state
    /// (`GLOBAL_ZTWorldMgr`/`GLOBAL_ZTHabitatMgr`/`GLOBAL_ZTResearchMgr`) - both poles read the exact
    /// same live globals in the same process, so every counter it writes ends up byte-identical, and it
    /// never touches [`ZooStatus::admission_price`]/the escape timestamp (the only genuinely
    /// non-deterministic fields elsewhere in this struct).
    fn run_zoostatus_calculate_sums_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_CALCULATE_SUMS";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
                save_to_memory(ptr as u32 + 0x14c, 4i32);
                save_to_memory(ptr as u32 + 0x150, 6i32);
            }

            ZOOSTATUS_CALCULATE_SUMS.original()(real_zoostatus_ptr as *const u32);
            (*reimpl_zoostatus_ptr).calculate_sums();
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// `ZOOSTATUS_SHOW_PRICES_SMOKE` - `zoostatus-implementation-plan.md` Stage 5's live coverage for
    /// [`ZooStatus::show_prices`]. **Not** a byte-comparison - `show_prices` reads `self` but writes
    /// nothing back into it (see that method's own doc comment), so there is no struct state to diff.
    /// Builds one standalone `ZTGameMgr` block, runs real [`ZooStatus::init`] to get real, in-bounds
    /// `admission_price`/`_min`/`_max` values, then calls [`ZooStatus::show_prices`] against the live
    /// `GLOBAL_BFUIMgr` singleton and its real `0x105e`/`0x1061`/`0x1062`/`0x1063`/`0x105f` UI elements -
    /// confirms the real `bfinternat::setMoneyText`/`BFUIMgr::getElement`/`UIElement::enable`/`disable`
    /// call chain doesn't crash or hang, matching the precedent `ZOOSTATUS_NEWGUEST_CHECKS_SMOKE` set for
    /// methods with real UI/live-singleton side effects a byte-diff can't usefully cover.
    fn run_zoostatus_show_prices_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_SHOW_PRICES_SMOKE";

        let mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
            }
            return true;
        }

        let zoostatus_ptr = (mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        unsafe {
            std::ptr::write_bytes(zoostatus_ptr as *mut u8, 0, size_of::<ZooStatus>());
            (*zoostatus_ptr).init(std::ptr::null());
            (*zoostatus_ptr).show_prices();
        }

        write_success_line(failure_log, test_name);
        gamemgr_live_support::destroy_standalone_mgr(mgr_ptr);
        false
    }

    /// `ZOOSTATUS_OVERRIDE` - `zoostatus-implementation-plan.md` Stage 6's live comparison for
    /// [`ZooStatus::override_config`]. Builds two standalone `ZTGameMgr` blocks (same harness
    /// `ZOOSTATUS_INIT`/`ZOOSTATUS_PRICING` use), zeroes each one's embedded `ZooStatus` sub-region, then
    /// constructs one real, shared `BFConfigFile` over the actual shipped `economy.cfg`
    /// (`BFCONFIGFILE_CONSTRUCTOR_0`, the same construct-and-parse pattern
    /// `ztshowmgr.rs`'s `initShowParams` test already established for `shows.cfg` - see that test's own
    /// doc comment; `economy.cfg`'s filename address comes from `ZTScenarioMgr_commonSetup.c`'s own
    /// `BFConfigFile::attempt` call site, the only other place in the decompile corpus that opens this
    /// file). Runs real vanilla `OVERRIDE.original()` against one `ZooStatus` and
    /// [`ZooStatus::override_config`] against the other, both reading the *same* live, already-parsed
    /// config object (pure reads - safe to reuse across both calls), then full-struct byte-compares.
    ///
    /// **Also verifies the one write `override` makes outside `this`**: the `cAdultAdmission` config
    /// list gets copied into a real process-global (`0x6392ac..0x6392c0`, see
    /// [`zoostatus::raw_globals::PRICE_TIER_BOUNDARY_0_RVA`] and
    /// [`ZooStatus::override_config`]'s own doc comment for the full evidence trail), which a pure
    /// `ZooStatus`-struct byte diff can't see at all - both poles write to the *same* shared process
    /// memory, so the second call's write would silently clobber the first's if compared naively. This
    /// captures the global's value right after each pole's own call instead (`real_boundaries`/
    /// `reimpl_boundaries`), and additionally checks the real pole's own result against `economy.cfg`'s
    /// known shipped values (`49`/`29`/`19`/`9`/`0`) - a positive check that the write actually happened,
    /// not just that both poles agree by both being silent no-ops.
    fn run_zoostatus_override_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_OVERRIDE";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        // `ZTScenarioMgr_commonSetup.c`'s own call site uses `PTR_s_economy_cfg_00641bf4` - the `PTR_`
        // prefix means `0x641bf4` is a *pointer variable* holding the real string's address, not the
        // string bytes themselves (unlike `ZooStatus_override.c`'s own bare `s_<text>_<addr>` section/key
        // literals, which are direct addresses - see `override_config_keys`). One extra dereference is
        // needed here. Also the shared small-object freelist head `BFConfigFile`'s own ctor/dtor pop/push
        // uses (`DAT_0063800c`) - the same freelist `ztshowmgr.rs`'s `CONFIG_FREELIST_HEAD_RVA` reads,
        // kept as its own local copy per this codebase's "not shared even with each other" convention.
        const ECONOMY_CFG_FILENAME_PTR_RVA: u32 = 0x00641bf4 - 0x400000;
        const CONFIG_FREELIST_HEAD_RVA: u32 = 0x0063800c - 0x400000;

        let base = get_module_base("zoo.exe") as u32;
        let economy_cfg_filename_ptr: u32 = get_from_memory(base + ECONOMY_CFG_FILENAME_PTR_RVA);
        let config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
        let config_ptr = config.as_ptr() as *const u32;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

            BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, economy_cfg_filename_ptr as *const u8);
        }

        if get_from_memory::<i32>(config_ptr as u32 + 0x4) == 0 {
            error!("{}: economy.cfg failed to load - real vanilla BFConfigFile has no data", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: economy.cfg failed to load\n", test_name).as_bytes());
            }
            unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            return true;
        }

        // `override`'s `cAdultAdmission` copy loop writes into a *global* (`0x6392ac..0x6392c0`, see
        // `zoostatus.rs`'s `raw_globals::PRICE_TIER_BOUNDARY_0_RVA..=_4_RVA`), not into `this` - the
        // whole-struct byte comparison below can't see that write at all, since both poles share the same
        // process memory and the second call's write would silently clobber the first's. Capture it after
        // each pole's own call instead, so a divergence there is caught directly rather than masked.
        const PRICE_TIER_BOUNDARY_BASE_RVA: u32 = 0x006392ac - 0x400000;
        let read_price_tier_boundaries = || -> [f32; 5] {
            std::array::from_fn(|i| get_from_memory(base + PRICE_TIER_BOUNDARY_BASE_RVA + (i as u32) * 4))
        };

        unsafe {
            ZOOSTATUS_OVERRIDE.original()(real_zoostatus_ptr as *const u32, config_ptr);
        }
        let real_boundaries = read_price_tier_boundaries();

        unsafe {
            (*reimpl_zoostatus_ptr).override_config(config_ptr as *const std::ffi::c_void);
        }
        let reimpl_boundaries = read_price_tier_boundaries();

        unsafe {
            BFCONFIGFILE_RELEASE.original()(config_ptr);
        }

        // ~BFConfigFile's inlined dtor tail - see `ztshowmgr.rs`'s `init_show_params` for the same pattern.
        let tree_root: u32 = get_from_memory(config_ptr as u32);
        if tree_root != 0 {
            let freelist_head = (base + CONFIG_FREELIST_HEAD_RVA) as *mut u32;
            unsafe {
                let head = *freelist_head;
                *(tree_root as *mut u32) = head;
                *freelist_head = tree_root;
            }
        }

        let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

        let mut mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
            .collect();

        // `economy.cfg`'s real, shipped `cAdultAdmission` values - both a positive check that this write
        // actually happened (not just "both poles agree by both being no-ops") and the byte-level
        // real-vs-reimpl comparison the struct diff above can't reach.
        let expected_boundaries: [f32; 5] = [49.0, 29.0, 19.0, 9.0, 0.0];
        if real_boundaries != expected_boundaries {
            error!("{}: real vanilla's own price-tier-boundary globals don't match economy.cfg's known values: {:?}", test_name, real_boundaries);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: real vanilla price-tier-boundary globals unexpected: {:?}\n", test_name, real_boundaries).as_bytes());
            }
            mismatches.push((usize::MAX, 0, 0));
        }
        if reimpl_boundaries != real_boundaries {
            error!("{}: price-tier-boundary globals mismatch after override_config (real, reimpl): {:?} vs {:?}", test_name, real_boundaries, reimpl_boundaries);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: price-tier-boundary globals mismatch (real, reimpl): {:?} vs {:?}\n", test_name, real_boundaries, reimpl_boundaries).as_bytes());
            }
            mismatches.push((usize::MAX, 0, 0));
        }

        let failed = !mismatches.is_empty();
        if failed {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
        } else {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        failed
    }

    /// Writes distinct, non-default values into every field [`ZooStatus::save`]/[`ZooStatus::load`]
    /// actually touch - a different value per history-array slot (`row*100+col` style), so an
    /// offset/indexing bug anywhere in the three array regions surfaces as a mismatch instead of being
    /// masked by every slot sharing one value. Used identically on both the real and reimplemented
    /// instance by `ZOOSTATUS_SAVE_LOAD`.
    fn seed_zoostatus_for_save_load(ptr: *mut ZooStatus) {
        unsafe {
            (*ptr).rating_check_elapsed = 111;
            (*ptr).message_check_elapsed = 222;
            (*ptr).newguest_check_elapsed = 333;
            (*ptr).finance_check_pending = true;
            (*ptr).zoo_rating_current = 42;
            (*ptr).field_0x48 = 3;
            (*ptr).field_0x50 = 555;
            (*ptr).field_0x54 = 666;
            (*ptr).donation_count_this_period = 12.5;
            (*ptr).current_month_index = 5;
            (*ptr).current_year_index = 7;
            for (row, months) in (*ptr).monthly_history.iter_mut().enumerate() {
                for (col, v) in months.iter_mut().enumerate() {
                    *v = row as f32 * 100.0 + col as f32;
                }
            }
            for (row, years) in (*ptr).yearly_history.iter_mut().enumerate() {
                for (col, v) in years.iter_mut().enumerate() {
                    *v = row as f32 * 1000.0 + col as f32 * 10.0;
                }
            }
            for (i, v) in (*ptr).flat_totals.iter_mut().enumerate() {
                *v = i as f32 * 7.5;
            }
            (*ptr).admission_price = 49.5;
            (*ptr).last_animal_escape_timestamp_low = 0xdeadbeef;
            (*ptr).last_animal_escape_timestamp_high = 0x12345678;
        }
    }

    /// `ZOOSTATUS_SAVE_LOAD` - `zoostatus-implementation-plan.md` Stage 7's live comparison for
    /// [`ZooStatus::save`]/[`ZooStatus::load`] (current-version path only, `version >= 0x47` - see
    /// [`ZooStatus::load`]'s own doc comment for why older versions are out of this stage's scope).
    ///
    /// Builds two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_OVERRIDE`
    /// use), zeroes each one's embedded `ZooStatus` sub-region, then seeds both identically via
    /// [`seed_zoostatus_for_save_load`]. Runs real `SAVE.original()` against one and [`ZooStatus::save`]
    /// against the other, both captured via `io_redirect` - the two byte streams must be identical,
    /// since a real-vs-reimpl format difference would otherwise only surface later, indirectly, as a
    /// `load` mismatch. Then builds two more fresh, zeroed standalone blocks and replays each side's
    /// own captured bytes into real `LOAD.original()`/[`ZooStatus::load`] respectively
    /// (`version = 0x47`, the exact boundary this stage's scope claims to support), and full-struct
    /// byte-compares the results - `load` only ever writes into fields `save` persisted, so a
    /// fresh-zeroed destination needs no masking: every byte either round-trips to its seeded value or
    /// stays zero on both sides identically.
    fn run_zoostatus_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_SAVE_LOAD";

        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            return true;
        }

        let zoostatus_size = size_of::<ZooStatus>();
        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        }
        seed_zoostatus_for_save_load(real_zoostatus_ptr);
        seed_zoostatus_for_save_load(reimpl_zoostatus_ptr);

        let dummy_file: u32 = 0;

        io_redirect::begin_capture();
        unsafe { ZOOSTATUS_SAVE.original()(real_zoostatus_ptr as *const u32, &dummy_file as *const u32 as *const i8) };
        let real_bytes = io_redirect::end_capture();

        io_redirect::begin_capture();
        let _ = unsafe { (*reimpl_zoostatus_ptr).save(&dummy_file as *const u32 as *const i8) };
        let reimpl_bytes = io_redirect::end_capture();

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);

        if real_bytes != reimpl_bytes {
            error!("{}: save byte mismatch, real len={} reimpl len={}", test_name, real_bytes.len(), reimpl_bytes.len());
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: save byte mismatch, real len={} reimpl len={}\n", test_name, real_bytes.len(), reimpl_bytes.len()).as_bytes());
            }
            return true;
        }

        let real_load_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_load_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_load_mgr_ptr.is_null() || reimpl_load_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null for the load pair (real={:?}, reimpl={:?})", test_name, real_load_mgr_ptr, reimpl_load_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null for the load pair\n", test_name).as_bytes());
            }
            if !real_load_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_load_mgr_ptr);
            }
            if !reimpl_load_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_load_mgr_ptr);
            }
            return true;
        }

        let real_load_zoostatus_ptr = (real_load_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_load_zoostatus_ptr = (reimpl_load_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        unsafe {
            std::ptr::write_bytes(real_load_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_load_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        }

        const CURRENT_VERSION: u32 = 0x47;

        io_redirect::begin_replay(real_bytes.clone());
        let real_load_ok = unsafe { ZOOSTATUS_LOAD.original()(real_load_zoostatus_ptr as *const u32, &dummy_file as *const u32 as *const u8, CURRENT_VERSION) };
        io_redirect::end_replay();

        io_redirect::begin_replay(reimpl_bytes.clone());
        let reimpl_load_ok = unsafe { (*reimpl_load_zoostatus_ptr).load(&dummy_file as *const u32, CURRENT_VERSION) };
        io_redirect::end_replay();

        let mut failed = false;
        if (real_load_ok & 0xff != 0) != (reimpl_load_ok & 0xff != 0) {
            error!("{}: load ok mismatch (real={:#x}, reimpl={:#x})", test_name, real_load_ok, reimpl_load_ok);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: load ok mismatch (real={:#x}, reimpl={:#x})\n", test_name, real_load_ok, reimpl_load_ok).as_bytes());
            }
            failed = true;
        }

        let real_bytes_after = unsafe { std::slice::from_raw_parts(real_load_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes_after = unsafe { std::slice::from_raw_parts(reimpl_load_zoostatus_ptr as *const u8, zoostatus_size) };
        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter_map(|i| if real_bytes_after[i] != reimpl_bytes_after[i] { Some((i, real_bytes_after[i], reimpl_bytes_after[i])) } else { None })
            .collect();
        if !mismatches.is_empty() {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: {} byte mismatch(es) after load (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es) after load, first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
            }
            failed = true;
        }

        if !failed {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_load_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_load_mgr_ptr);
        failed
    }

    /// Builds a synthetic pre-`0x47` `ZooStatus::load` byte stream by hand, following real vanilla's own
    /// exact per-version read order (`ZooStatus_load.c`/`.asm`, see [`ZooStatus::load`]'s own doc comment
    /// for the full per-threshold breakdown this mirrors): every field/slot a given `version` actually
    /// reads gets a distinct, recognizable value (`category*100+month`-style for history slots) so an
    /// offset/loop-bound bug anywhere surfaces as a mismatch rather than being masked by a shared value.
    /// `version < 0x26` slots are encoded as raw `i32` (matching that range's genuine `FILD` int->float
    /// conversion); `version >= 0x26` slots are encoded as literal `f32` bytes (matching that range's plain
    /// reinterpreting read) - using the wrong encoding for a given version would make real vanilla's own
    /// `LOAD.original()` interpret the stream differently than intended, so this distinction is load-bearing
    /// for the test's own correctness, not just style.
    fn build_legacy_load_bytes(version: u32, category_count: i32, year_count: i32) -> Vec<u8> {
        let mut buf = Vec::new();
        if version <= 0xc {
            return buf;
        }

        buf.extend_from_slice(&111i32.to_le_bytes()); // rating_check_elapsed
        buf.extend_from_slice(&222i32.to_le_bytes()); // message_check_elapsed
        buf.extend_from_slice(&333i32.to_le_bytes()); // newguest_check_elapsed

        if version < 0x17 {
            buf.extend_from_slice(&400_000i32.to_le_bytes()); // > 360000 -> finance_check_pending = true
        } else {
            buf.push(1u8); // finance_check_pending
        }

        buf.extend_from_slice(&42i32.to_le_bytes()); // zoo_rating_current
        buf.extend_from_slice(&3i32.to_le_bytes()); // field_0x48
        buf.extend_from_slice(&555i32.to_le_bytes()); // field_0x50
        buf.extend_from_slice(&666i32.to_le_bytes()); // field_0x54

        if version < 0x18 {
            buf.extend_from_slice(&0xdeadi32.to_le_bytes()); // discarded
        }

        buf.extend_from_slice(&12.5f32.to_le_bytes()); // donation_count_this_period

        if version < 0x17 {
            return buf;
        }

        buf.extend_from_slice(&5i32.to_le_bytes()); // current_month_index
        buf.extend_from_slice(&7i32.to_le_bytes()); // current_year_index
        buf.extend_from_slice(&category_count.to_le_bytes());
        buf.extend_from_slice(&year_count.to_le_bytes());

        if version < 0x26 {
            for category in 0..category_count.max(0) {
                for month in 0..12 {
                    buf.extend_from_slice(&(category * 100 + month).to_le_bytes());
                }
            }
            for category in 0..category_count.max(0) {
                for year in 0..year_count.max(0) {
                    buf.extend_from_slice(&(category * 1000 + year * 10).to_le_bytes());
                }
            }
            for category in 0..category_count.max(0) {
                buf.extend_from_slice(&(category * 7).to_le_bytes());
            }
        } else {
            for category in 0..category_count.max(0) {
                for month in 0..12 {
                    buf.extend_from_slice(&(category as f32 * 100.0 + month as f32).to_le_bytes());
                }
            }
            for category in 0..category_count.max(0) {
                for year in 0..year_count.max(0) {
                    buf.extend_from_slice(&(category as f32 * 1000.0 + year as f32 * 10.0).to_le_bytes());
                }
            }
            for category in 0..category_count.max(0) {
                buf.extend_from_slice(&(category as f32 * 7.5).to_le_bytes());
            }
        }

        if version >= 0x27 {
            buf.extend_from_slice(&49.5f32.to_le_bytes()); // admission_price
        }

        if version < 0x47 {
            return buf;
        }

        buf.extend_from_slice(&0xdeadbeefu32.to_le_bytes());
        buf.extend_from_slice(&0x12345678u32.to_le_bytes());

        buf
    }

    /// `ZOOSTATUS_LOAD_LEGACY` - Stage 9 of `zoostatus-implementation-plan.md`: live comparison of
    /// [`ZooStatus::load`]'s full pre-`0x47` version range (`ZOOSTATUS_SAVE_LOAD` already covers the
    /// `>= 0x47` current-format path). For each `(version, category_count, year_count)` below - chosen to
    /// exercise every threshold this stage added, plus the `category_count`/`year_count` edge shapes real
    /// old saves could plausibly hit - builds a synthetic byte stream via [`build_legacy_load_bytes`],
    /// replays the identical bytes into a fresh, zeroed-then-identically-seeded standalone `ZooStatus` via
    /// both real `LOAD.original()` and this port's [`ZooStatus::load`], and full-struct byte-compares the
    /// result (masking the escape timestamp for `version < 0x47`, since both sides re-seed it from a live,
    /// non-deterministic [`GET_OLD_DATE`] call rather than reading it from the stream).
    ///
    /// Cases, by what each is chosen to exercise:
    /// - `version = 8`: below the `0xc` floor - nothing read at all, seeded sentinel values must survive
    ///   untouched.
    /// - `version = 0x10`: header-scalars-only path (`< 0x17`), including the `raw > 360_000` derivation
    ///   for `finance_check_pending` and the pre-`0x18` discard read.
    /// - `version = 0x17`, `category_count = 10`, `year_count = 15`: enters the real migration path
    ///   (`< 0x26`) with the pre-`0x19` row-14 differencing active, and `category_count < 15` so row 14's
    ///   differenced value is never clobbered by that category's own straight write (see
    ///   [`ZooStatus::load`]'s own doc comment for why `>= 15` would clobber it).
    /// - `version = 0x19`, `category_count = 35`, `year_count = 25`: still the migration path but past the
    ///   differencing cutoff (pure `i32`->`f32` conversion only), with `category_count`/`year_count` both
    ///   deliberately over this struct's own `31`/`20` geometry to exercise per-slot discard.
    /// - `version = 0x26`, `category_count = 5`, `year_count = 3`: the new generalized-but-permissive
    ///   straight-copy path, short enough to exercise the trailing zero-fill.
    /// - `version = 0x2a`, `category_count = 31`, `year_count = 20`: same path at this struct's own exact
    ///   geometry, past the `0x27` admission-price-read threshold.
    /// - `version = 0x46`: same path, right at the boundary below the already-covered `0x47` fast path.
    fn run_zoostatus_load_legacy_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZOOSTATUS_LOAD_LEGACY";
        let dummy_file: u32 = 0;
        let zoostatus_size = size_of::<ZooStatus>();

        let cases: [(u32, i32, i32); 7] = [
            (8, 0, 0),
            (0x10, 0, 0),
            (0x17, 10, 15),
            (0x19, 35, 25),
            (0x26, 5, 3),
            (0x2a, 31, 20),
            (0x46, 31, 20),
        ];

        let mut any_failed = false;

        for (version, category_count, year_count) in cases {
            let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
            let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
            if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
                error!("{}: CREATE_ZTGAME_MGR returned null for version {:#x} (real={:?}, reimpl={:?})", test_name, version, real_mgr_ptr, reimpl_mgr_ptr);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null for version {:#x}\n", test_name, version).as_bytes());
                }
                if !real_mgr_ptr.is_null() {
                    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
                }
                if !reimpl_mgr_ptr.is_null() {
                    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
                }
                any_failed = true;
                continue;
            }

            let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
            let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
            unsafe {
                std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
                std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            }
            // Seed a recognizable non-zero baseline on both sides identically, so a version whose real
            // control flow leaves some region untouched (e.g. `version <= 0xc`, or any category/year row
            // past `category_count`/`year_count`) proves it by both sides keeping this sentinel, not by
            // both merely staying zero (which a bug that skips a *write* could also produce).
            seed_zoostatus_for_save_load(real_zoostatus_ptr);
            seed_zoostatus_for_save_load(reimpl_zoostatus_ptr);

            let bytes = build_legacy_load_bytes(version, category_count, year_count);

            io_redirect::begin_replay(bytes.clone());
            let real_ok = unsafe { ZOOSTATUS_LOAD.original()(real_zoostatus_ptr as *const u32, &dummy_file as *const u32 as *const u8, version) };
            io_redirect::end_replay();

            io_redirect::begin_replay(bytes);
            let reimpl_ok = unsafe { (*reimpl_zoostatus_ptr).load(&dummy_file as *const u32, version) };
            io_redirect::end_replay();

            let mut failed = false;
            if (real_ok & 0xff != 0) != (reimpl_ok & 0xff != 0) {
                error!("{}: version {:#x} load ok mismatch (real={:#x}, reimpl={:#x})", test_name, version, real_ok, reimpl_ok);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: version {:#x} load ok mismatch (real={:#x}, reimpl={:#x})\n", test_name, version, real_ok, reimpl_ok).as_bytes());
                }
                failed = true;
            }

            let real_bytes_after = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
            let reimpl_bytes_after = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };
            let timestamp_offset = std::mem::offset_of!(ZooStatus, last_animal_escape_timestamp_low);
            let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
                .filter(|&i| version >= 0x47 || !(timestamp_offset..timestamp_offset + 8).contains(&i))
                .filter_map(|i| if real_bytes_after[i] != reimpl_bytes_after[i] { Some((i, real_bytes_after[i], reimpl_bytes_after[i])) } else { None })
                .collect();
            if !mismatches.is_empty() {
                let shown = &mismatches[..mismatches.len().min(32)];
                error!("{}: version {:#x} (category_count={}, year_count={}) {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, version, category_count, year_count, mismatches.len(), shown.len(), shown);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: version {:#x} (category_count={}, year_count={}) {} byte mismatch(es), first {}: {:?}\n", test_name, version, category_count, year_count, mismatches.len(), shown.len(), shown).as_bytes());
                }
                failed = true;
            }

            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            any_failed |= failed;
        }

        if !any_failed {
            write_success_line(failure_log, test_name);
        }
        any_failed
    }

    /// Canonicalizes a `cash` bit pattern for `ZTGAMEMGR_SAVE_LOAD`'s comparison: any NaN collapses to a
    /// single representative bit pattern, sidestepping both IEEE-754 `NaN != NaN` on direct equality *and*
    /// a real, root-caused x87-vs-SSE2 NaN-canonicalization artifact this test's own failures surfaced.
    ///
    /// A failing case had `cash` written as a *signaling* NaN (mantissa MSB `0`): `save`'s captured output
    /// was bit-identical real vs. reimpl (so `ZooStatus::save`, which never touches `cash`, wasn't
    /// involved), but after `load`, the real side came back as a *quiet* NaN (mantissa MSB `1`, i.e.
    /// `real_bits == reimpl_bits | 0x0040_0000`) while the reimplemented side kept the original signaling
    /// bits. `ZTGameMgr_load.asm` (read in full) pins this to `ZTGameMgr::load`'s own `this->cash =
    /// local_8;` line: it compiles to `FLD float ptr [ESP+0x10]` / `FSTP float ptr [ESP]` (the field is
    /// genuinely `float`-typed, even though the decompiler shows a raw `undefined4` dword copy) - x87
    /// silences a signaling NaN by setting its quiet bit on any load/store through the FPU stack. This
    /// reimplementation's `self.cash = cash;` is a plain SSE2 move with no FPU round-trip, so it preserves
    /// the raw bits unchanged - not a port bug, just not bit-for-bit identical to a real `load` that
    /// happens to touch a signaling NaN, which real gameplay never produces from a legitimate cash value.
    fn normalize_cash_bits(cash: f32) -> u32 {
        if cash.is_nan() {
            0x7fc0_0000
        } else {
            cash.to_bits()
        }
    }

    /// `ZTGAMEMGR_SAVE_LOAD` - `ztgamemgr-implementation-plan.md` Stage 2: builds two Stage-1-seeded
    /// standalone `ZTGameMgr` instances (real `SET_NEW_GAME_DEFAULTS.original()` run on both, via the
    /// same real `BFConfigFile` construction `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS`'s own test already uses),
    /// then for a generated `(cash, date_bytes, elapsed_sim_ticks, version)` seeds both instances
    /// identically via the test-only `set_cash`/`set_date_bytes`/`set_elapsed_sim_ticks` accessors
    /// (`ztgamemgr.rs`'s `Systemtime` is private, so the raw 16-byte `date` blob is generated/compared
    /// byte-for-byte rather than field-by-field) and runs the real `SAVE.original()` against one and the
    /// reimplemented `ztgamemgr::ZTGameMgr::save` against the other, capturing each side's
    /// `WRITE_BYTES_TO_FILE` output via `io_redirect` - both should be byte-identical, since
    /// `ZooStatus::save`'s own contribution is the *same* real function running against identically-seeded
    /// memory on both sides. Then replays each side's captured bytes back into a fresh, zeroed third
    /// standalone instance (real `LOAD.original()`/reimplemented `load()` respectively) and compares the
    /// resulting `cash`/`date`/`elapsed_sim_ticks` fields - `version` is generated from both sides of the
    /// `BFGameMgr::load` `0x48` threshold (`BFGameMgr_load.c`) so both the "read elapsed_sim_ticks" and
    /// "zero it instead" branches get exercised. `cash` is compared via [`normalize_cash_bits`] rather
    /// than raw `to_bits()` - see that function's doc comment for why.
    fn run_gamemgr_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_SAVE_LOAD";

        let real_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
            }
            return true;
        }

        let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
        }

        let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
        let config_ptr = config.as_mut_ptr() as *const u32;
        let kind_tag_byte: u8 = 0;
        unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };
        unsafe {
            ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
            (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
        }
        unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let mut fail_flag = false;

        let dummy_file: u32 = 0;
        let date_bytes_strategy = prop::collection::vec(any::<u8>(), 16).prop_map(|v| {
            let mut out = [0u8; 0x10];
            out.copy_from_slice(&v);
            out
        });
        let version_strategy = prop_oneof![0u32..0x49, 0x49u32..0x1000];

        let result = runner.run(&(any::<f32>(), date_bytes_strategy, any::<u32>(), version_strategy), |(cash, date_bytes, elapsed_sim_ticks, version)| {
            unsafe {
                (*real_ptr).set_cash(cash);
                (*real_ptr).set_date_bytes(date_bytes);
                (*real_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
                (*reimpl_ptr).set_cash(cash);
                (*reimpl_ptr).set_date_bytes(date_bytes);
                (*reimpl_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
            }

            io_redirect::begin_capture();
            unsafe { ZTGAMEMGR_SAVE.original()(real_ptr as *const u32, &dummy_file as *const u32) };
            let real_bytes = io_redirect::end_capture();

            io_redirect::begin_capture();
            let _ = unsafe { (*reimpl_ptr).save(&dummy_file as *const u32) };
            let reimpl_bytes = io_redirect::end_capture();

            prop_assert_eq!(
                &real_bytes,
                &reimpl_bytes,
                "save byte mismatch for cash={}, date_bytes={:?}, elapsed_sim_ticks={}",
                cash,
                date_bytes,
                elapsed_sim_ticks
            );

            let real_load_ptr = gamemgr_live_support::build_standalone_mgr();
            let reimpl_load_ptr = gamemgr_live_support::build_standalone_mgr();
            prop_assume!(!real_load_ptr.is_null() && !reimpl_load_ptr.is_null());
            unsafe {
                std::ptr::write_bytes(real_load_ptr as *mut u8, 0, struct_size);
                std::ptr::write_bytes(reimpl_load_ptr as *mut u8, 0, struct_size);
            }

            io_redirect::begin_replay(real_bytes.clone());
            let real_load_ok = unsafe { ZTGAMEMGR_LOAD.original()(real_load_ptr as *const u32, &dummy_file as *const u32, version) };
            io_redirect::end_replay();

            io_redirect::begin_replay(reimpl_bytes.clone());
            let reimpl_load_ok = unsafe { (*reimpl_load_ptr).load(&dummy_file as *const u32, version) };
            io_redirect::end_replay();

            let real_result = unsafe { (normalize_cash_bits((*real_load_ptr).cash()), (*real_load_ptr).date_bytes(), (*real_load_ptr).elapsed_sim_ticks()) };
            let reimpl_result = unsafe { (normalize_cash_bits((*reimpl_load_ptr).cash()), (*reimpl_load_ptr).date_bytes(), (*reimpl_load_ptr).elapsed_sim_ticks()) };

            gamemgr_live_support::destroy_standalone_mgr(real_load_ptr);
            gamemgr_live_support::destroy_standalone_mgr(reimpl_load_ptr);

            prop_assert_eq!(real_load_ok != 0, reimpl_load_ok, "load ok mismatch for version={}", version);
            prop_assert_eq!(real_result, reimpl_result, "load result mismatch for version={}", version);

            Ok(())
        });

        match result {
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

        gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

        fail_flag
    }

    /// `ZTGAMEMGR_UPDATE_SIM` - `ztgamemgr-implementation-plan.md` Stage 3: builds two Stage-1-seeded
    /// standalone `ZTGameMgr` instances, then for a generated `(delta, valid date fields,
    /// elapsed_sim_ticks)` seeds both instances identically (`set_date_bytes`/`set_elapsed_sim_ticks`/
    /// `set_day_changed_flag(false)`) and runs the real `UPDATE_SIM.original()` against one and the
    /// reimplemented `ztgamemgr::ZTGameMgr::update_sim` against the other, comparing the resulting
    /// `date`/`elapsed_sim_ticks`/`day_changed_flag` (via the same accessor methods - real and reimpl
    /// memory share the same layout, so `(*real_ptr).date_bytes()` etc. work identically on either
    /// pointer, no separate raw-offset reads needed).
    ///
    /// Generated dates are constrained to valid `SYSTEMTIME` field ranges (year `1601..=9999`, month
    /// `1..=12`, day `1..=28`, etc.) rather than arbitrary byte garbage: an invalid `SYSTEMTIME` makes
    /// `SystemTimeToFileTime` fail, and vanilla's own decompiled body (`ZTGameMgr_updateSim.c`/`.asm`)
    /// then proceeds with whatever garbage bytes happened to be on its stack in that case - not
    /// reproducible from this side, and not the interesting path this test means to exercise (the real
    /// date-arithmetic round-trip). `delta` is bounded to `0..=0x3e9` (1001) and the shared global tick
    /// accumulator (`DAT_006394b8`) is reset to `0` immediately before *each* side's call - both
    /// standalone instances' `updateSim` reads/writes the *same* process-wide global, so without this
    /// reset the two sides would race each other into (and out of) the `ZTUI::main::set*`-refresh branch
    /// depending purely on call order. Per the implementation plan's own caution, this branch is never
    /// exercised here: calling those UI-refresh functions against a standalone, non-globally-registered
    /// `ZTGameMgr` risks corrupting real, unrelated live UI state (the rating-formula arithmetic that
    /// branch also gates is covered separately, live-independent, by `ztgamemgr.rs`'s own
    /// `rating_from_metric` unit tests).
    ///
    /// Two more branches also get zero live exercise here, worth calling out explicitly rather than
    /// leaving implicit: `soundscape_ptr`/`menu_music_handler_ptr` (the latter via [`Self::update`] below,
    /// not `update_sim` itself) stay null on every standalone instance this battery ever builds - nothing
    /// in the Stage-1 `set_new_game_defaults` seeding path or anywhere else in scope ever sets either
    /// field (only the out-of-scope `start()` does, per `ztgamemgr.rs`'s Stage-5 doc comment) - so their
    /// `ZTSoundscape::update`/`MenuMusicHandler::update` call-through branches never run, live or
    /// otherwise. Acceptable since both delegate entirely to still-out-of-scope classes with no logic of
    /// `ZTGameMgr`'s own to verify, but genuinely untested rather than intentionally skipped.
    ///
    /// One more branch is deliberately disabled rather than left to chance, added after this test crashed
    /// the live battery: `update_sim` calls `ZooStatus::update` unconditionally every tick, which can roll
    /// a real vanilla `fChance` and fire `ZooStatus::f_grant_donation` - a genuine UI-dialog path in the
    /// same family the paragraph above already avoids, just reached through a different call chain
    /// (`ZooStatus::update`, not `update_sim` itself). Both standalone instances get
    /// `donation_chance_percent` forced to `0` right after `set_new_game_defaults`, before the proptest
    /// loop starts, so that roll is always a deterministic no - `finance_check_pending` is forced `false`
    /// alongside it, defensively, since `ZooStatus::update`'s `financeChecks` call-through carries the same
    /// cross-allocator risk class. See the field-zeroing code's own comment for the crash this reproduces
    /// and root-causes.
    fn run_gamemgr_update_sim_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_UPDATE_SIM";

        let real_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
            }
            return true;
        }

        let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
        }

        let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
        let config_ptr = config.as_mut_ptr() as *const u32;
        let kind_tag_byte: u8 = 0;
        unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };
        unsafe {
            ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
            (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
        }
        unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

        // `set_new_game_defaults`'s real `economy.cfg`-driven `override` gives both instances a
        // genuinely non-zero `donation_chance_percent`, and `ZooStatus::update` (called unconditionally
        // every `update_sim`) rolls it via real vanilla `fChance` whenever `cash` is below
        // `DAT_00635128` - true for a fresh instance's starting `cash`. Across up to 256 proptest cases
        // that roll eventually lands, firing `f_grant_donation`, which - per its own doc comment - grants
        // through the *live* `GLOBAL_ZTGameMgr` (not `self`) and loads real localized strings via
        // `ZTApp::getApp`/`BFApp::loadString`. Both are unsafe to exercise here: `always_late_tests` (this
        // test's own group) runs on `ZTApp::updateSim`'s first tick, before `run_load_live_zoo` loads a
        // real zoo - `GLOBAL_ZTGameMgr` isn't the live game's own manager yet, and the string
        // registry/`BFApp` singleton isn't necessarily fully populated, which crashed this test live
        // (confirmed via `crash-capture`: `ZooStatus::f_grant_donation` -> `load_localized_string` ->
        // `CStr::from_ptr` walking off the end of a not-null-terminated 512-byte stack buffer). Zeroing
        // `donation_chance_percent` on both instances makes every `fChance(0)` roll deterministically
        // false, so `f_grant_donation` never fires - the same "keep a real UI-dialog condition false"
        // discipline `ZOOSTATUS_CHECKS`'s own doc comment already established for this exact hazard.
        //
        // `finance_check_pending` is forced `false` alongside it for the same reason, defensively:
        // `set_new_game_defaults`/`init` already leave it `false` and nothing in this test's call path
        // sets it `true`, but `ZooStatus::update`'s `financeChecks` call-through walks `ZTWorldMgr`'s
        // freelist-backed building list - the same cross-allocator risk class as the donation-roll crash
        // this guard's sibling above was added for. An explicit write here, rather than relying on it
        // staying false by construction, means a future caller change can't silently reintroduce that
        // hazard into this test unnoticed.
        unsafe {
            (*((real_ptr as u32 + 0x10) as *mut ZooStatus)).donation_chance_percent = 0;
            (*((reimpl_ptr as u32 + 0x10) as *mut ZooStatus)).donation_chance_percent = 0;
            (*((real_ptr as u32 + 0x10) as *mut ZooStatus)).finance_check_pending = false;
            (*((reimpl_ptr as u32 + 0x10) as *mut ZooStatus)).finance_check_pending = false;
        }

        let dat_006394b8_addr = get_module_base("zoo.exe") as u32 + (0x006394b8u32 - 0x400000u32);

        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);

        let date_fields_strategy = (1601u16..=9999, 1u16..=12, 1u16..=28, 0u16..=23, 0u16..=59, 0u16..=59, 0u16..=999);

        let result = runner.run(&(0u32..=0x3e9, date_fields_strategy, any::<u32>()), |(delta, (year, month, day, hour, minute, second, milliseconds), elapsed_sim_ticks)| {
            let mut date_bytes = [0u8; 0x10];
            date_bytes[0..2].copy_from_slice(&year.to_le_bytes());
            date_bytes[2..4].copy_from_slice(&month.to_le_bytes());
            // date_bytes[4..6] (w_day_of_week) intentionally left 0 - ignored on input by SystemTimeToFileTime.
            date_bytes[6..8].copy_from_slice(&day.to_le_bytes());
            date_bytes[8..10].copy_from_slice(&hour.to_le_bytes());
            date_bytes[10..12].copy_from_slice(&minute.to_le_bytes());
            date_bytes[12..14].copy_from_slice(&second.to_le_bytes());
            date_bytes[14..16].copy_from_slice(&milliseconds.to_le_bytes());

            unsafe {
                (*real_ptr).set_date_bytes(date_bytes);
                (*real_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
                (*real_ptr).set_day_changed_flag(false);
                (*reimpl_ptr).set_date_bytes(date_bytes);
                (*reimpl_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
                (*reimpl_ptr).set_day_changed_flag(false);
            }

            save_to_memory(dat_006394b8_addr, 0i32);
            unsafe { ZTGAMEMGR_UPDATE_SIM.original()(real_ptr as *const u32, delta) };

            save_to_memory(dat_006394b8_addr, 0i32);
            unsafe { (*reimpl_ptr).update_sim(delta) };

            let real_result = unsafe { ((*real_ptr).date_bytes(), (*real_ptr).elapsed_sim_ticks(), (*real_ptr).day_changed_flag()) };
            let reimpl_result = unsafe { ((*reimpl_ptr).date_bytes(), (*reimpl_ptr).elapsed_sim_ticks(), (*reimpl_ptr).day_changed_flag()) };

            prop_assert_eq!(
                real_result,
                reimpl_result,
                "updateSim mismatch for delta={}, date=({},{},{},{},{},{},{}), elapsed_sim_ticks={}",
                delta,
                year,
                month,
                day,
                hour,
                minute,
                second,
                milliseconds,
                elapsed_sim_ticks
            );

            Ok(())
        });

        let mut fail_flag = false;
        match result {
            Ok(_) => {
                info!("Proptest passed for {}", test_name);
            }
            Err(e) => {
                error!("Proptest failed: {:?}", e);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                }
                fail_flag = true;
            }
        }

        // A small, dedicated `update(delta)` check - both instances have a null menu_music_handler_ptr
        // (never set by set_new_game_defaults or anything above), so this should be a pure no-op on both
        // sides with nothing to diff, but still gets *some* live coverage per the implementation plan.
        unsafe {
            ZTGAMEMGR_UPDATE.original()(real_ptr as *const u32, 16);
            (*reimpl_ptr).update(16);
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

        fail_flag
    }

    /// `ZTGAMEMGR_FINANCE_DATE_HELPERS` - `ztgamemgr-implementation-plan.md` Stage 4 (+ Stage 7's
    /// follow-up methods): builds two Stage-1-seeded standalone `ZTGameMgr` instances, then proptests
    /// `addCash`/`subtractCash`/`getDate`/`isGameDate`/`isRealWorldDate`/`timeAgo`/`hoursAgo`/
    /// `animalTimeAgo`/`peopleTimeAgo`/`overrideNewGameDefaults` real `.original()` vs the reimplemented
    /// methods. `removedZooDoo` itself is not ported/detoured - see `ztgamemgr.rs`'s Stage-5 doc comment
    /// for why - so there's no test for it here.
    ///
    /// `addCash`/`subtractCash` mutate `cash`, so both sides are reseeded to the same generated `cash`
    /// value (`set_cash`) before each call and compared via `normalize_cash_bits` (NaN-safe, see that
    /// helper's own doc comment). The date-family helpers (`getDate`/`isGameDate`/`timeAgo`/`hoursAgo`/
    /// `animalTimeAgo`/`peopleTimeAgo`) don't mutate `this`, so both sides are seeded with the same
    /// generated `date` bytes (`set_date_bytes`) and compared purely on return value - `isGameDate`'s real
    /// return has garbage upper bits (see `ZTGameMgr::is_game_date`'s own doc comment), so only the low
    /// byte is compared; `animalTimeAgo`/`peopleTimeAgo` similarly only compare the low dword/byte (see
    /// their own doc comments for why the rest is undefined leftover). `isRealWorldDate` takes no
    /// `this`/seeded state at all (calls `GetSystemTime` directly on both sides, independently,
    /// microseconds apart) - comparing real vs reimpl booleans here only risks a spurious mismatch in the
    /// astronomically unlikely case a call lands exactly on a day/month rollover between the two calls.
    /// `overrideNewGameDefaults` mutates the embedded `ZooStatus` via the same real vanilla function on
    /// both sides, so the whole `0x10..0x1160` region is byte-diffed afterward rather than compared
    /// field-by-field.
    ///
    /// Calls the real `TIME_AGO.original()`/`HOURS_AGO.original()` using the hand-corrected signatures
    /// now in `generated.rs` (see those `FunctionDef`s' own doc comments) - calling either with the
    /// original, auto-generated signatures would have corrupted the stack/dropped the high dword.
    fn run_gamemgr_finance_date_helpers_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_FINANCE_DATE_HELPERS";

        let real_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
            }
            return true;
        }

        let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
        }

        let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
        let config_ptr = config.as_mut_ptr() as *const u32;
        let kind_tag_byte: u8 = 0;
        unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };
        unsafe {
            ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
            (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
        }
        unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

        // Dedicated, kept-alive config for the overrideNewGameDefaults comparison below - the one above
        // is released immediately after seeding set_new_game_defaults, matching the rest of this test's
        // existing pattern.
        let mut override_config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
        let override_config_ptr = override_config.as_mut_ptr() as *const u32;
        unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(override_config_ptr, &kind_tag_byte as *const u8) };

        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);

        let date_fields_strategy = (1601u16..=9999, 1u16..=12, 1u16..=28, 0u16..=23, 0u16..=59, 0u16..=59, 0u16..=999);
        let day_strategy = prop_oneof![Just(0xffffffffu32), 1u32..=28];
        let month_strategy = prop_oneof![Just(0xffffffffu32), 1u32..=12];

        let result = runner.run(
            &(
                any::<f32>(),
                any::<f32>(),
                date_fields_strategy,
                day_strategy,
                month_strategy,
                any::<u64>(),
            ),
            |(add_amount, sub_amount, (year, month, day, hour, minute, second, milliseconds), game_day, game_month, reference)| {
                // addCash/subtractCash: reseed cash identically on both sides, then compare.
                unsafe {
                    (*real_ptr).set_cash(0.0);
                    (*reimpl_ptr).set_cash(0.0);
                    ZTGAMEMGR_ADD_CASH.original()(real_ptr as *const u32, add_amount);
                    (*reimpl_ptr).add_cash(add_amount);
                }
                prop_assert_eq!(
                    normalize_cash_bits(unsafe { (*real_ptr).cash() }),
                    normalize_cash_bits(unsafe { (*reimpl_ptr).cash() }),
                    "addCash mismatch for amount={}",
                    add_amount
                );

                unsafe {
                    (*real_ptr).set_cash(0.0);
                    (*reimpl_ptr).set_cash(0.0);
                    ZTGAMEMGR_SUBTRACT_CASH.original()(real_ptr as *const u32, sub_amount, false);
                    (*reimpl_ptr).subtract_cash(sub_amount);
                }
                prop_assert_eq!(
                    normalize_cash_bits(unsafe { (*real_ptr).cash() }),
                    normalize_cash_bits(unsafe { (*reimpl_ptr).cash() }),
                    "subtractCash mismatch for amount={}",
                    sub_amount
                );

                // getDate/isGameDate/timeAgo/hoursAgo: reseed date identically, then compare.
                let mut date_bytes = [0u8; 0x10];
                date_bytes[0..2].copy_from_slice(&year.to_le_bytes());
                date_bytes[2..4].copy_from_slice(&month.to_le_bytes());
                date_bytes[6..8].copy_from_slice(&day.to_le_bytes());
                date_bytes[8..10].copy_from_slice(&hour.to_le_bytes());
                date_bytes[10..12].copy_from_slice(&minute.to_le_bytes());
                date_bytes[12..14].copy_from_slice(&second.to_le_bytes());
                date_bytes[14..16].copy_from_slice(&milliseconds.to_le_bytes());
                unsafe {
                    (*real_ptr).set_date_bytes(date_bytes);
                    (*reimpl_ptr).set_date_bytes(date_bytes);
                }

                let mut real_date_out = FILETIME::default();
                let real_date_ptr = unsafe { ZTGAMEMGR_GET_DATE.original()(real_ptr as *const u32, &mut real_date_out as *const FILETIME) };
                prop_assert_eq!(real_date_ptr as *const FILETIME, &real_date_out as *const FILETIME, "getDate should return the out-pointer it was given");
                let real_date_ticks = ((real_date_out.dwHighDateTime as u64) << 32) | real_date_out.dwLowDateTime as u64;
                let reimpl_date = unsafe { (*reimpl_ptr).get_date() };
                prop_assert_eq!(real_date_ticks, reimpl_date, "getDate mismatch for date=({},{},{},{},{},{},{})", year, month, day, hour, minute, second, milliseconds);

                let real_is_game_date = unsafe { ZTGAMEMGR_IS_GAME_DATE.original()(real_ptr as *const u32, game_day, game_month) };
                let reimpl_is_game_date = unsafe { (*reimpl_ptr).is_game_date(game_day, game_month) };
                prop_assert_eq!(
                    real_is_game_date & 0xff,
                    reimpl_is_game_date as u32,
                    "isGameDate mismatch for day={}, month={}, date=({},{},{},{},{},{},{})",
                    game_day,
                    game_month,
                    year,
                    month,
                    day,
                    hour,
                    minute,
                    second,
                    milliseconds
                );

                let reference_low = reference as u32;
                let reference_high = (reference >> 32) as u32;
                let reference_filetime = FILETIME {
                    dwLowDateTime: reference_low,
                    dwHighDateTime: reference_high,
                };
                let mut real_time_ago_out = FILETIME::default();
                unsafe {
                    ZTGAMEMGR_TIME_AGO.original()(real_ptr as *const u32, &mut real_time_ago_out as *const FILETIME, reference_filetime);
                }
                let real_time_ago_ticks = ((real_time_ago_out.dwHighDateTime as u64) << 32) | real_time_ago_out.dwLowDateTime as u64;
                let reimpl_time_ago = unsafe { (*reimpl_ptr).time_ago(reference) };
                prop_assert_eq!(real_time_ago_ticks, reimpl_time_ago, "timeAgo mismatch for reference={}", reference);

                let real_hours_ago = unsafe { ZTGAMEMGR_HOURS_AGO.original()(real_ptr as *const u32, reference_low, reference_high as i32) };
                let reimpl_hours_ago = unsafe { (*reimpl_ptr).hours_ago(reference) };
                prop_assert_eq!(real_hours_ago, reimpl_hours_ago, "hoursAgo mismatch for reference={}", reference);

                // animalTimeAgo/peopleTimeAgo: same seeded date/reference as timeAgo/hoursAgo above - only
                // the low dword of the real register-pair return is meaningful (see each method's own doc
                // comment), so only that half is compared.
                let real_animal_time_ago = unsafe { ZTGAMEMGR_ANIMAL_TIME_AGO.original()(real_ptr as *const u32, reference_low, reference_high as i32) };
                let reimpl_animal_time_ago = unsafe { (*reimpl_ptr).animal_time_ago(reference) };
                prop_assert_eq!(
                    real_animal_time_ago as u32,
                    reimpl_animal_time_ago,
                    "animalTimeAgo mismatch for reference={}",
                    reference
                );

                let real_people_time_ago = unsafe { ZTGAMEMGR_PEOPLE_TIME_AGO.original()(real_ptr as *const u32, reference_low, reference_high as i32) };
                let reimpl_people_time_ago = unsafe { (*reimpl_ptr).people_time_ago(reference) };
                prop_assert_eq!(
                    real_people_time_ago as u8 as u32,
                    reimpl_people_time_ago,
                    "peopleTimeAgo mismatch for reference={}",
                    reference
                );
                // overrideNewGameDefaults: the same real ZooStatus::override runs against both sides'
                // embedded ZooStatus with the same config, so the whole embedded region (0x10..0x1160)
                // should stay byte-identical afterward - nothing else in this test's proptest body touches
                // that region.
                unsafe {
                    ZTGAMEMGR_OVERRIDE_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, override_config_ptr);
                    (*reimpl_ptr).override_new_game_defaults(override_config_ptr);
                }
                let real_zoostatus_bytes = unsafe { std::slice::from_raw_parts((real_ptr as *const u8).add(0x10), 0x1150) };
                let reimpl_zoostatus_bytes = unsafe { std::slice::from_raw_parts((reimpl_ptr as *const u8).add(0x10), 0x1150) };
                prop_assert_eq!(real_zoostatus_bytes, reimpl_zoostatus_bytes, "overrideNewGameDefaults: embedded ZooStatus diverged");

                // isRealWorldDate: no seeded state - both sides call GetSystemTime independently.
                let real_is_real_world = unsafe { ZTGAMEMGR_IS_REAL_WORLD_DATE.original()(game_day as i32, game_month) };
                let reimpl_is_real_world = ztgamemgr::ZTGameMgr::is_real_world_date(game_day, game_month);
                prop_assert_eq!(
                    (real_is_real_world & 0xff) != 0,
                    reimpl_is_real_world,
                    "isRealWorldDate mismatch for day={}, month={}",
                    game_day,
                    game_month
                );

                Ok(())
            },
        );

        let mut fail_flag = false;
        match result {
            Ok(_) => {
                info!("Proptest passed for {}", test_name);
            }
            Err(e) => {
                error!("Proptest failed: {:?}", e);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
                }
                fail_flag = true;
            }
        }

        unsafe { BFCONFIGFILE_RELEASE.original()(override_config_ptr) };

        if !fail_flag {
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

        fail_flag
    }

    /// `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS_IS_NEW_GAME_SMOKE` - one-shot wiring check for
    /// `set_new_game_defaults`'s `is_new_game=true` branch, which `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` above
    /// deliberately never exercises (that test pins `is_new_game=false` on both sides - see its own doc
    /// comment for why: the `true` branch calls through `GLOBAL_ZTAIMgr`'s real vtable slot, the live,
    /// shared AI manager singleton, so exercising it is a real side effect on live game state, not
    /// something a synthetic standalone instance can safely absorb before a zoo has even loaded). Deferred
    /// to here, after `run_load_live_zoo`, for the same reason `removedZooDoo`'s own smoke test had to move
    /// (see `ztgamemgr.rs`'s Stage-5 doc comment / this file's git history): a global pointer being
    /// non-null (`GLOBAL_ZTAIMgr` is set well before this point) is not the same guarantee as the global's
    /// *internal* state being genuinely constructed, and calling into a still-uninitialized manager's real
    /// vtable slot pre-zoo-load is exactly the class of bug that crashed `getBuildingList` there. Once a
    /// real zoo is loaded, calling this is no different from what real "start new game" gameplay already
    /// does.
    ///
    /// Not a byte-diff: both sides call through to the *same* live `GLOBAL_ZTAIMgr` singleton, so a memory
    /// comparison between the two standalone instances wouldn't reflect anything meaningful about either
    /// side's own logic. This only confirms the call wiring (`this`/args passed into
    /// `BFAIMGR_LOAD_DATA.original()`) doesn't crash on either side - a wrong-`this`/wrong-arg bug there is
    /// exactly what the pinned-`false` Stage 1 test structurally cannot see. Run last in this file's
    /// battery (see `run_all_tests`), since `BFAIMgr::loadData` may have real side effects on live AI state
    /// that earlier tests shouldn't have to account for.
    fn run_gamemgr_set_new_game_defaults_is_new_game_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS_IS_NEW_GAME_SMOKE";

        let real_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
            }
            if !real_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
            }
            return true;
        }

        let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
        unsafe {
            std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
        }

        let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
        let config_ptr = config.as_mut_ptr() as *const u32;
        let kind_tag_byte: u8 = 0;
        unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };

        unsafe {
            ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, true);
            (*reimpl_ptr).set_new_game_defaults(config_ptr, true);
        }

        unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

        info!("{}: is_new_game=true call-through completed without crashing on both sides", test_name);
        write_success_line(failure_log, test_name);

        gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

        false
    }

    /// `ZTGAMEMGR_START_STOP_SMOKE` - one-shot wiring check for the reimplemented `start`/`stop`
    /// (`ztgamemgr.rs`'s Stage-5 doc comment covers the full port). Not a byte-diff: `start`/`stop` read
    /// the live `GLOBAL_ZTScenarioMgr`/`GLOBAL_ZTApp` singletons and call through to real vanilla
    /// `ZTSoundscape`/`ZTUI::main::unpauseGame` - side effects on shared global/audio state, not something
    /// a standalone instance's own memory can meaningfully diff against a second standalone instance. This
    /// only confirms the call sequence (allocate/construct/init the soundscape, read the two new raw
    /// globals, tail-call `unpauseGame`) doesn't crash. Deferred to run last, after `run_load_live_zoo` and
    /// after the `is_new_game=true` smoke test above, for the same reason that one is deferred:
    /// `GLOBAL_ZTScenarioMgr`/`GLOBAL_ZTApp` being non-null pointers is not the same guarantee as their
    /// internal state being genuinely constructed pre-zoo-load (the same "non-null but uninitialized
    /// registry" hazard class that crashed `getBuildingList` during the `removedZooDoo` investigation).
    fn run_gamemgr_start_stop_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_START_STOP_SMOKE";

        let ptr = gamemgr_live_support::build_standalone_mgr();
        if ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
            }
            return true;
        }

        unsafe { (*ptr).start() };
        let started_after_start = unsafe { (*ptr).started() };
        let soundscape_after_start = unsafe { (*ptr).soundscape_ptr() };

        unsafe { (*ptr).stop() };
        let started_after_stop = unsafe { (*ptr).started() };
        let soundscape_after_stop = unsafe { (*ptr).soundscape_ptr() };

        let mut fail_flag = false;
        if !started_after_start || soundscape_after_start == 0 {
            fail_flag = true;
            error!(
                "{}: expected started=true and a non-null soundscape_ptr after start(), got started={} soundscape_ptr={:#x}",
                test_name, started_after_start, soundscape_after_start
            );
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!(
                        "Test Failed {}: after start(), started={} soundscape_ptr={:#x}\n",
                        test_name, started_after_start, soundscape_after_start
                    )
                    .as_bytes(),
                );
            }
        }
        if started_after_stop || soundscape_after_stop != 0 {
            fail_flag = true;
            error!(
                "{}: expected started=false and a null soundscape_ptr after stop(), got started={} soundscape_ptr={:#x}",
                test_name, started_after_stop, soundscape_after_stop
            );
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!(
                        "Test Failed {}: after stop(), started={} soundscape_ptr={:#x}\n",
                        test_name, started_after_stop, soundscape_after_stop
                    )
                    .as_bytes(),
                );
            }
        }

        if !fail_flag {
            info!("{}: start()/stop() call sequence completed without crashing, flags/pointer toggled as expected", test_name);
            write_success_line(failure_log, test_name);
        }

        gamemgr_live_support::destroy_standalone_mgr(ptr);

        fail_flag
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

    /// Compares the real `ZTHabitatMgr::getHabitat` against the reimplemented `get_habitat_ptr`, for
    /// small in-range tile coordinates, now that `run_load_live_zoo` has populated
    /// `other_array_start`/`other_array_end` with a real zoo's bounds (`get_habitat_ptr` does no
    /// bounds-checking of its own, so this needs a real, loaded zoo to be safe).
    fn run_habitat_get_habitat_ptr_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTHABITATMGR_GET_HABITAT_PTR_LIVE";
        let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
        let habitat_mgr = globals().zthabitatmgr();

        let mut fail_flag = false;
        for x in 0..5i32 {
            for y in 0..5i32 {
                let real = unsafe { zthabitatmgr::GET_HABITAT.original()(mgr_ptr, x, y) };
                let reimpl = habitat_mgr.get_habitat_ptr(x, y);
                if real != reimpl {
                    error!("{}: mismatch at ({}, {}): real={:#010x}, reimpl={:#010x}", test_name, x, y, real, reimpl);
                    if let Some(log_file) = failure_log {
                        let _ = log_file.write_all(
                            format!("Test Failed {}: mismatch at ({}, {}): real={:#010x}, reimpl={:#010x}\n", test_name, x, y, real, reimpl).as_bytes(),
                        );
                    }
                    fail_flag = true;
                }
            }
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        }
        fail_flag
    }

    /// ZTMEGATILEMGR_UPDATE: compares the real `ZTMegatileMgr::update`'s effect on
    /// `dirty`/`tick_accumulator` against the reimplemented `update`, for `delta_ticks` below the
    /// `0x1d4b` threshold only. Snapshots the live singleton's scalars, calls the real
    /// `UPDATE.original()`, records the result as expected, restores the snapshot (via
    /// `megatile_live_support::restore_scalars`), calls the reimplemented `update()`, and compares.
    /// Runs after `run_load_live_zoo` so the live singleton is a real, populated grid - see
    /// `ztmegatilemgr.rs`'s own module doc comment for why this is implemented/tested first.
    ///
    /// Deliberately never crosses the threshold here: doing so calls through to
    /// `recalculate_characteristics` as a side effect (real or reimplemented, either way), which is
    /// exercised directly and more thoroughly by `run_megatilemgr_recalculate_characteristics_test`
    /// below - no need for this test to also trigger it as a side effect. The threshold/dirty-flag
    /// transition logic itself is still fully covered by `ztmegatilemgr::tests::update_state_*` (pure,
    /// no live memory touched).
    fn run_megatilemgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTMEGATILEMGR_UPDATE";
        let mgr_ptr = globals().ztmegatilemgr_ptr();
        if mgr_ptr.is_null() {
            info!("Skipping {}: GLOBAL_ZTMegatileMgr not initialized", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTMegatileMgr not initialized)", test_name));
            return false;
        }
        let real_this = mgr_ptr as *const u32;

        let runner_config = ProptestConfig { failure_persistence: Some(Box::new(super::NoopFailurePersistence)), ..ProptestConfig::default() };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let mut fail_flag = false;
        match runner.run(&(0u32..0x1000u32), |delta_ticks| {
            let mgr = unsafe { &mut *mgr_ptr };
            // Force a known-safe starting accumulator (not just restore the pre-call value) - the live
            // singleton's own accumulator may already be close to the threshold from real game ticks
            // that ran before this test, and `delta_ticks + before_accumulator` crossing `0x1d4b` would
            // hit the same live recalc crash this test exists to avoid (see the doc comment above).
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
            // in a self-consistent state per its own logic - `ZTMEGATILEMGR_RECALCULATE_CHARACTERISTICS`
            // is the dedicated test for whether that recalculation itself matches, so this test only
            // compares the scalars `update` itself owns.
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

    /// Compares two megatile-grid snapshots allowing a small tolerance on the float fields (`stink`,
    /// formerly `esthetic_bonus` - see `ztmegatilemgr.rs`'s `ZTMegatile::stink`/finding 2) - real vanilla
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
    fn run_megatilemgr_recalculate_characteristics_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
    fn run_megatile_category_map_layout_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
                    if let Some(v) = mt.category_value(key) {
                        if !v.is_finite() {
                            error!("{}: non-finite value {} for key {:#x} at column={}, row={}", test_name, v, key, column, row);
                            fail_flag = true;
                        }
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

    /// ZTMEGATILEMGR_INIT: run last, and only after the other three are passing - `init()` resizes the
    /// outer/inner vectors (the actual allocation-adjacent call), the single highest-risk piece in
    /// `ztmegatilemgr.rs`. For a few small tile-count targets (both shrinking and growing relative to the
    /// live map's own size), calls the real `INIT.original()` to capture the resulting grid dimensions,
    /// resets to a different size, then calls the reimplemented `init()` for the same target and compares
    /// dimensions. Restores the live singleton to the real map's own dimensions afterward regardless of
    /// outcome, via the real vanilla `init`/`recalculateCharacteristics`, so nothing later depends on
    /// whatever the last reimplemented call left behind.
    fn run_megatilemgr_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

    /// ZTADVTERRAINMGR_START: sanity-checks the reimplemented `ZTAdvTerrainMgr::start()` against a real,
    /// live singleton. Deliberately does **not** also invoke the real `START.original()` for comparison,
    /// unlike this repo's usual real-vs-reimplemented pattern: `start()`'s own body (both the real one and
    /// this reimplementation) calls through to the real `start2D`/`startD3D`/`loadTextures`/`setupRender`
    /// D3D bring-up functions - those aren't designed to be re-entrant, so invoking them via both the real
    /// vtable call *and* the reimplementation in the same test run would re-run real device/texture
    /// bring-up twice in a row, risking a live D3D device corruption or crash for no comparison value (the
    /// orchestration logic itself - the short-circuit call sequence - is what this reimplementation adds,
    /// and it's simple enough to verify by code review; see the module's own `ztadvterrainmgr.rs` doc
    /// comment). Instead this just runs the reimplementation once against the live singleton and checks
    /// the result is plausible (succeeds, and leaves `state == 2` per `ZTAdvTerrainMgr_start.c`).
    fn run_ztadvterrainmgr_start_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
    /// `UPDATE.original()` for comparison in the same run: `update()`'s only shared, meaningfully mutable
    /// state is the live pending-tile queue (`+0x1d8`), and calling both the real and reimplemented
    /// versions back-to-back could each try to pop/free the same vanilla-owned node, double-freeing it -
    /// see `ztadvterrainmgr.rs`'s own module doc comment on the cross-allocator hazard. The live queue is
    /// populated only by other, un-reimplemented vanilla code and may well be empty during this test -
    /// that's an expected, non-failing case; when non-empty, this still safely exercises the real
    /// pop-front/recycle path against genuine vanilla-allocated nodes. The assertion itself is narrow but
    /// real: `update()` never mutates `state` (confirmed - it's read-only in `ZTAdvTerrainMgr_update.c`),
    /// so forcing `state` to each branch and checking it comes back unchanged catches any accidental write.
    fn run_ztadvterrainmgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTADVTERRAINMGR_UPDATE";
        let mgr_ptr = globals().ztadvterrainmgr_ptr();
        if mgr_ptr.is_null() {
            write_success_line(failure_log, &format!("{} (skipped: ZTAdvTerrainMgr not initialized)", test_name));
            return false;
        }
        let original_state = unsafe { &*mgr_ptr }.state();

        let runner_config = ProptestConfig { failure_persistence: Some(Box::new(super::NoopFailurePersistence)), ..ProptestConfig::default() };
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

    /// ZTGUEST_MEGATILE_METHODS_LIVE: compares the real `ZTGuest::fCrowdDensityMegatile`/
    /// `fStinkyMegatile`/`fEstheticBonusMegatile` (see `ztguest.rs`'s module doc comment for how these
    /// three addresses were confirmed) against their Rust reimplementations, for *every* live guest
    /// `guest_live_support::find_live_guests` finds on the loaded save - not just one, so the comparison
    /// samples whatever spread of tiles/megatiles/entity-type category ids the live population actually
    /// has, rather than a single arbitrary data point. Runs after the megatile-grid tests above so the
    /// live singleton's grid is already in a real, recalculated state.
    fn run_ztguest_megatile_methods_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGUEST_MEGATILE_METHODS_LIVE";
        let guests = guest_live_support::find_live_guests();
        if guests.is_empty() {
            write_success_line(failure_log, &format!("{} (skipped: no live guest found)", test_name));
            return false;
        }

        let mut fail_flag = false;
        let mut compared = 0usize;
        for (guest_ptr, tile) in guests {
            let this = guest_ptr as *const u32;
            let entity = unsafe { crate::util::ref_from_memory::<BFEntity>(guest_ptr) };
            compared += 1;

            let real_crowd = unsafe { gen_ztguest::F_CROWD_DENSITY_MEGATILE.original()(this) };
            let reimpl_crowd = ztguest::crowd_density_megatile(&tile);
            if real_crowd != reimpl_crowd {
                error!("{}: crowd density mismatch for guest {:#010x} at {}: real={}, reimpl={}", test_name, guest_ptr, tile, real_crowd, reimpl_crowd);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(
                        format!("Test Failed {}: crowd density mismatch for guest {:#010x} at {}: real={}, reimpl={}\n", test_name, guest_ptr, tile, real_crowd, reimpl_crowd).as_bytes(),
                    );
                }
                fail_flag = true;
            }

            let real_stink = unsafe { gen_ztguest::F_STINKY_MEGATILE.original()(this) };
            let reimpl_stink = ztguest::stinky_megatile(&tile);
            if (real_stink - reimpl_stink).abs() >= 0.01 {
                error!("{}: stink mismatch for guest {:#010x} at {}: real={}, reimpl={}", test_name, guest_ptr, tile, real_stink, reimpl_stink);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(
                        format!("Test Failed {}: stink mismatch for guest {:#010x} at {}: real={}, reimpl={}\n", test_name, guest_ptr, tile, real_stink, reimpl_stink).as_bytes(),
                    );
                }
                fail_flag = true;
            }

            let real_esthetic = unsafe { gen_ztguest::F_ESTHETIC_BONUS_MEGATILE.original()(this) };
            let reimpl_esthetic = ztguest::esthetic_bonus_megatile(entity, &tile);
            if (real_esthetic - reimpl_esthetic).abs() >= 0.01 {
                error!("{}: esthetic bonus mismatch for guest {:#010x} at {}: real={}, reimpl={}", test_name, guest_ptr, tile, real_esthetic, reimpl_esthetic);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(
                        format!(
                            "Test Failed {}: esthetic bonus mismatch for guest {:#010x} at {}: real={}, reimpl={}\n",
                            test_name, guest_ptr, tile, real_esthetic, reimpl_esthetic
                        )
                        .as_bytes(),
                    );
                }
                fail_flag = true;
            }
        }

        if !fail_flag {
            write_success_line(failure_log, &format!("{} ({} guests compared)", test_name, compared));
        }
        fail_flag
    }

    /// One funding-level entry: `name_id` is always one of the real, known-valid `research.ini`
    /// funding-level name string ids (`23100`/`23101`/`23102`/`23103` - `"%s none"`/`"%s min"`/
    /// `"%s normal"`/`"%s max"`, per `ZTResearchFundingLevel::name`'s own doc comment) so the `%s`
    /// substitution both sides perform has a real, resolvable template to work with - this test runs
    /// at the `updateSim` first-tick injection point specifically so those ids are actually resolvable
    /// (language DLLs load during app init, same reasoning as `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`
    /// above). `cost` is bounded well away from `f32`'s extremes - `funding_text` casts
    /// `cost * (1.0/30.0)` to `i32` after rounding, and vanilla's own float-to-int conversion is
    /// undefined for non-finite/out-of-i32-range inputs, which isn't a meaningful case to compare.
    fn funding_level_case_strategy() -> impl Strategy<Value = (i32, f32)> {
        (prop_oneof![Just(23100i32), Just(23101i32), Just(23102i32), Just(23103i32)], -1_000_000f32..1_000_000f32)
    }

    /// ZTRESEARCHBRANCH_FUNDING_TEXT: compares the real `ZTResearchBranch::getFundingText`'s output
    /// against the reimplemented `ZTResearchBranch::funding_text`, for a standalone branch (not
    /// spliced into any `ZTResearchMgr` - see `live_support::build_standalone_funding_branch`'s doc
    /// comment) with a generated funding table and `current_funding_level` spanning negative/in-range/
    /// out-of-range relative to the table's length, to exercise the "no active level" empty-string path
    /// alongside the normal formatted-text path.
    fn run_funding_text_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTRESEARCHBRANCH_FUNDING_TEXT";
        let mut fail_flag = false;

        match runner.run(&(-2i32..4i32, prop::collection::vec(funding_level_case_strategy(), 0..3)), |(current_funding_level, levels)| {
            let branch_ptr = live_support::build_standalone_funding_branch(current_funding_level, &levels);

            let mut buffer = [0u32; 3];
            unsafe {
                ZTRESEARCHBRANCH_GET_FUNDING_TEXT.original()(branch_ptr as *const u32, buffer.as_mut_ptr() as *const u32);
            }
            let real_text = get_from_memory::<ZTBufferString>(buffer.as_ptr() as u32).copy_to_string();
            let branch: &ZTResearchBranch = unsafe { &*branch_ptr };
            let reimpl_text = branch.funding_text();

            live_support::destroy_standalone_funding_branch(branch_ptr);

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

    /// ZTRESEARCHBRANCH_UPDATE: compares the real `ZTResearchBranch::update`'s progress effect on the
    /// currently-selected program against the reimplemented `update`, for a single synthetic
    /// branch/category/program/funding-level built via `live_support::build_update_test_branch`.
    /// Restricted to the non-completing case - `TARGET_COST` is fixed far above any possible
    /// `progress_delta` for the generated `days`/`funding_rate` ranges (max ~23, per
    /// `predict_branch_progress`'s scale constant), so `on_completion`/`pick_random_program`/UI
    /// (covered separately by the `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`/`ZTRESEARCHMGR_FORCE_RESEARCH`
    /// tests) never actually run on either side.
    ///
    /// `available_cash` is generated as `cash_delta * cash_multiplier` for `cash_multiplier` in
    /// `0.0..2.0` (`cash_delta` computed from the same generated `days`/`funding_cost` via
    /// `predict_branch_progress(.., f32::MAX).0`), so roughly half of generated cases land unaffordable
    /// and half affordable - exercising both `ZTGameMgr::subtractCash`/`subtract_cash` on the real and
    /// reimplemented sides. This used to be restricted to the *insufficient-cash* case only, because of
    /// a real bug: `openzt-detour/src/generated.rs`'s `SUBTRACT_CASH` `FunctionDef` declared one `f32`
    /// stack arg, but the real `ZTGameMgr::subtractCash` takes `(f32, bool)` per its `.asm`'s `RET 8` -
    /// a 4-byte stack imbalance on every `.original()` call. That's now fixed (see
    /// `ztmarketing-update-setmoneytext-crash-investigation.md`'s "Resolution" section), so the
    /// affordable branch is safe to exercise here too. The exact `available_cash == cash_delta` boundary
    /// is separately covered deterministically by `run_branch_update_reimpl_boundary_test` below.
    fn run_branch_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTRESEARCHBRANCH_UPDATE";
        let mut fail_flag = false;

        if live_support::ztgamemgr_ptr_is_null() {
            info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
            return fail_flag;
        }

        const TARGET_COST: f32 = 1_000_000.0;

        match runner.run(
            &(1u32..1000u32, -1000f32..1000f32, 1f32..1000f32, 0f32..1000f32, 0.0f32..2.0f32),
            |(days, funding_rate, funding_cost, initial_progress, cash_multiplier)| {
                let (cash_delta, _) = predict_branch_progress(days, funding_cost, funding_rate, f32::MAX);
                let available_cash = (cash_delta * cash_multiplier).max(0.0);

                let real_progress = live_support::with_update_test_branch(TARGET_COST, initial_progress, funding_rate, funding_cost, |mgr| {
                    let branch = mgr.branch_mut(0);
                    live_support::with_ztgamemgr_cash(available_cash, || unsafe {
                        ZTRESEARCHBRANCH_UPDATE.original()((branch as *mut ZTResearchBranch) as *const u32, days);
                    });
                    branch.current_program().map(|p| p.current_progress())
                });

                let reimpl_progress = live_support::with_update_test_branch(TARGET_COST, initial_progress, funding_rate, funding_cost, |mgr| {
                    let branch = mgr.branch_mut(0);
                    live_support::with_ztgamemgr_cash(available_cash, || branch.update(days));
                    branch.current_program().map(|p| p.current_progress())
                });

                prop_assert_eq!(
                    real_progress,
                    reimpl_progress,
                    "current_progress mismatch for days={}, funding_rate={}, funding_cost={}, initial_progress={}, available_cash={}",
                    days,
                    funding_rate,
                    funding_cost,
                    initial_progress,
                    available_cash
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

    /// Deterministic single-case regression for the *reimplemented* side of `ZTRESEARCHBRANCH_UPDATE`'s
    /// affordable branch - the actual previously-buggy path (`ZTResearchBranch::update` ->
    /// `ZTGameMgr::spend_research`/`subtract_cash`, routed through our own `SPEND_RESEARCH`/`SUBTRACT_CASH`
    /// `FunctionDef`s). Mirrors `run_marketing_update_reimpl_boundary_test`'s shape/rationale - see
    /// `ztmarketing-update-setmoneytext-crash-investigation.md`'s "Resolution" section and "Suggested next
    /// steps" item 7.
    ///
    /// Calls only the reimplemented `ZTResearchBranch::update`, skipping `.original()` entirely. `TARGET_COST`
    /// stays fixed far above any possible progress delta (same rationale as `run_branch_update_test` above), so
    /// completion/`on_completion`/UI never runs here - this is narrowly about the `subtract_cash` boundary call
    /// surviving. `available_cash` is pinned to exactly `cash_delta`, forcing the affordable branch.
    fn run_branch_update_reimpl_boundary_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTRESEARCHBRANCH_UPDATE_REIMPL_BOUNDARY_REPRO";

        if live_support::ztgamemgr_ptr_is_null() {
            info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
            return false;
        }

        const TARGET_COST: f32 = 1_000_000.0;
        let days: u32 = 5;
        let funding_rate: f32 = 30.0;
        let funding_cost: f32 = 100.0;
        let initial_progress: f32 = 0.0;
        let (cash_delta, _) = predict_branch_progress(days, funding_cost, funding_rate, f32::MAX);

        live_support::with_update_test_branch(TARGET_COST, initial_progress, funding_rate, funding_cost, |mgr| {
            let branch = mgr.branch_mut(0);
            info!("{}: about to call reimplemented ZTResearchBranch::update with available_cash == cash_delta ({})", test_name, cash_delta);
            live_support::with_ztgamemgr_cash(cash_delta, || branch.update(days));
            info!("{}: reimplemented call returned without crashing", test_name);
        });

        write_success_line(failure_log, test_name);
        false
    }

    /// ZTRESEARCHMGR_UPDATE (branch-count extension): compares the real `ZTResearchMgr::update`'s
    /// effect on `elapsed_ticks` and every branch's currently-selected program's `current_progress`
    /// against the reimplemented `update`, for 1-3 synthetic branches built via
    /// `live_support::with_update_test_branches`. The zero-branch `ZTRESEARCHMGR_UPDATE` test above
    /// only exercises `elapsed_ticks`' accumulator/day-count bookkeeping in isolation - this is the
    /// first test that actually exercises `ZTResearchMgr::update` iterating multiple branches and
    /// threading the correct `days` count to each (via `ZTResearchBranch::update`, native since Phase F).
    ///
    /// `target_cost` stays fixed far above any possible progress delta, same rationale as
    /// `run_branch_update_test` above, so `on_completion`/`pick_random_program`/UI never run on either
    /// side. Unlike that test, cash affordability here is drawn from one *shared* pool across every
    /// branch in a call: `ZTResearchMgr::update` (ztresearch.rs) iterates branches sequentially, and
    /// each `ZTResearchBranch::update` reads/spends the real, shared `GLOBAL_ZTGameMgr` cash fresh - so
    /// cash genuinely depletes across branches within one call, not per-branch. `available_cash` is
    /// `total_cash_delta * cash_multiplier` (`cash_multiplier` in `0.0..2.0`), where `total_cash_delta`
    /// sums every branch's own `cash_delta` (via `predict_branch_progress(.., f32::MAX).0`) for the
    /// shared `days` count the whole call receives. A multiplier below `1.0` naturally produces
    /// "prefix affordable, suffix not" cases as an earlier branch exhausts the shared pool - exercising
    /// real sequential depletion, not just per-branch affordability in isolation.
    ///
    /// This used to be restricted to the *insufficient-cash* case only (`AVAILABLE_CASH = 0.0`,
    /// `funding_rate` fixed inert at `0.0`), because of a real bug: `openzt-detour/src/generated.rs`'s
    /// `SUBTRACT_CASH` `FunctionDef` declared one `f32` stack arg, but the real
    /// `ZTGameMgr::subtractCash` takes `(f32, bool)` per its `.asm`'s `RET 8` - a 4-byte stack imbalance
    /// on every `.original()` call. That's now fixed (see
    /// `ztmarketing-update-setmoneytext-crash-investigation.md`'s "Resolution" section), so
    /// `funding_rate` is now generated too - a fixed `0.0` would leave `current_progress` trivially
    /// unchanged regardless of whether the affordable-branch math is right, silently defeating the
    /// comparison now that cash is sometimes affordable.
    fn run_research_mgr_update_branches_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTRESEARCHMGR_UPDATE_BRANCHES";
        let mut fail_flag = false;

        if live_support::ztgamemgr_ptr_is_null() {
            info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
            return fail_flag;
        }

        const TARGET_COST: f32 = 1_000_000.0;

        let branch_spec_strategy = (-1000f32..1000f32, 1f32..1000f32, 0f32..1000f32).prop_map(|(funding_rate, funding_cost, initial_progress)| {
            live_support::UpdateTestBranchSpec { target_cost: TARGET_COST, initial_progress, funding_rate, funding_cost }
        });

        match runner.run(
            &(any::<u32>(), any::<u32>(), prop::collection::vec(branch_spec_strategy, 1..4), 0.0f32..2.0f32),
            |(elapsed_ticks_before, delta_ticks, branch_specs, cash_multiplier)| {
                let (_, days) = predict_update(elapsed_ticks_before, delta_ticks);
                let total_cash_delta: f32 =
                    branch_specs.iter().map(|s| predict_branch_progress(days, s.funding_cost, s.funding_rate, f32::MAX).0).sum();
                let available_cash = (total_cash_delta * cash_multiplier).max(0.0);

                let (real_elapsed_ticks, real_progress_bits) = live_support::with_update_test_branches(&branch_specs, |mgr| {
                    mgr.set_elapsed_ticks(elapsed_ticks_before);
                    live_support::with_ztgamemgr_cash(available_cash, || unsafe {
                        ZTRESEARCHMGR_UPDATE.original()((mgr as *mut ZTResearchMgr) as *const u32, delta_ticks);
                    });
                    let progress_bits =
                        mgr.branches().flat_map(|b| b.current_program()).map(|p| p.current_progress().to_bits()).collect::<Vec<_>>();
                    (mgr.elapsed_ticks(), progress_bits)
                });

                let (reimpl_elapsed_ticks, reimpl_progress_bits) = live_support::with_update_test_branches(&branch_specs, |mgr| {
                    mgr.set_elapsed_ticks(elapsed_ticks_before);
                    live_support::with_ztgamemgr_cash(available_cash, || mgr.update(delta_ticks));
                    let progress_bits =
                        mgr.branches().flat_map(|b| b.current_program()).map(|p| p.current_progress().to_bits()).collect::<Vec<_>>();
                    (mgr.elapsed_ticks(), progress_bits)
                });

                prop_assert_eq!(
                    real_elapsed_ticks,
                    reimpl_elapsed_ticks,
                    "elapsed_ticks mismatch for elapsed_ticks_before={}, delta_ticks={}, branch_count={}",
                    elapsed_ticks_before,
                    delta_ticks,
                    branch_specs.len()
                );
                prop_assert_eq!(
                    real_progress_bits,
                    reimpl_progress_bits,
                    "current_progress mismatch for elapsed_ticks_before={}, delta_ticks={}, branch_count={}",
                    elapsed_ticks_before,
                    delta_ticks,
                    branch_specs.len()
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

    fn write_success_line(failure_log: &mut Option<std::fs::File>, test_name: &str) {
        let success_line = format!("Test Passed {}\n", test_name);
        if let Some(log_file) = failure_log {
            if let Err(write_err) = log_file.write_all(success_line.as_bytes()) {
                error!("Failed to write to failure log: {}", write_err);
            }
        }
    }

    /// Calls the real GET_FOOTPRINT function at `entity_ptr` and reads the `IVec3` it writes back.
    fn call_original_get_footprint(
        original: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32, bool) -> *const u32>,
        entity_ptr: *const u32,
        use_map_footprint: bool,
    ) -> IVec3 {
        let mut result = IVec3::default();
        unsafe {
            (original.original())(entity_ptr, &raw mut result as *const u32, use_map_footprint);
        }
        result
    }

    fn run_bfentity_get_footprint_tests(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "BFENTITY_GET_FOOTPRINT";
        let mut fail_flag = false;

        match runner.run(
            &(-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32, -8i32..8i32, proptest::bool::ANY),
            |(fx, fy, fz, rotation, use_map_footprint)| {
                let mut entity_type: BFEntityType = unsafe { std::mem::zeroed() };
                entity_type.footprintx = fx;
                entity_type.footprinty = fy;
                entity_type.footprintz = fz;

                let entity = BFEntity::new_for_test(&raw const entity_type as u32, rotation, 0);

                let reimplemented_result = entity.get_footprint(use_map_footprint);
                let real_result = call_original_get_footprint(BFENTITY_GET_FOOTPRINT, &raw const entity as *const u32, use_map_footprint);

                assert_eq!(
                    (real_result.x, real_result.y, real_result.z),
                    (reimplemented_result.x, reimplemented_result.y, reimplemented_result.z),
                    "BFEntity::get_footprint mismatch: fx={}, fy={}, fz={}, rotation={}, use_map_footprint={}, real={:?}, reimplemented={:?}",
                    fx, fy, fz, rotation, use_map_footprint, real_result, reimplemented_result
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
                if let proptest::test_runner::TestError::Fail(r, (fx, fy, fz, rotation, use_map_footprint)) = e {
                    let failure_line = format!(
                        "{}: fx={}, fy={}, fz={}, rotation={}, use_map_footprint={}, reason={}\n",
                        test_name, fx, fy, fz, rotation, use_map_footprint, r
                    );
                    if let Some(log_file) = failure_log {
                        if let Err(write_err) = log_file.write_all(failure_line.as_bytes()) {
                            error!("Failed to write to failure log: {}", write_err);
                        }
                    }
                    fail_flag = true;
                }
            }
        }

        fail_flag
    }

    /// `ZTUnit::getFootprint`'s `use_map_footprint=true` branch virtual-dispatches through
    /// `entity_type`'s vtable (not `this`'s own), so the fixture's `ZTUnitType` needs a real
    /// vtable pointer - see `ZTUnitType::new_for_test` / `ztunit-ztanimal-footprint-crash-investigation.md`.
    fn run_ztunit_get_footprint_tests(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTUNIT_GET_FOOTPRINT";
        let mut fail_flag = false;

        match runner.run(
            &(
                -1000i32..1000i32,
                -1000i32..1000i32,
                -1000i32..1000i32,
                -1000i32..1000i32,
                -8i32..8i32,
                proptest::bool::ANY,
            ),
            |(fx, fy, fz, map_footprint, rotation, use_map_footprint)| {
                let entity_type = ZTUnitType::new_for_test(IVec3::new(fx, fy, fz), map_footprint);
                let entity = ZTUnit::new_for_test(&raw const entity_type as u32, rotation, 0);

                let reimplemented_result = entity.get_footprint(use_map_footprint);
                let real_result = call_original_get_footprint(ZTUNIT_GET_FOOTPRINT, &raw const entity as *const u32, use_map_footprint);

                assert_eq!(
                    (real_result.x, real_result.y, real_result.z),
                    (reimplemented_result.x, reimplemented_result.y, reimplemented_result.z),
                    "ZTUnit::get_footprint mismatch: fx={}, fy={}, fz={}, map_footprint={}, rotation={}, use_map_footprint={}, real={:?}, reimplemented={:?}",
                    fx, fy, fz, map_footprint, rotation, use_map_footprint, real_result, reimplemented_result
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
                if let proptest::test_runner::TestError::Fail(r, (fx, fy, fz, map_footprint, rotation, use_map_footprint)) = e {
                    let failure_line = format!(
                        "{}: fx={}, fy={}, fz={}, map_footprint={}, rotation={}, use_map_footprint={}, reason={}\n",
                        test_name, fx, fy, fz, map_footprint, rotation, use_map_footprint, r
                    );
                    if let Some(log_file) = failure_log {
                        if let Err(write_err) = log_file.write_all(failure_line.as_bytes()) {
                            error!("Failed to write to failure log: {}", write_err);
                        }
                    }
                    fail_flag = true;
                }
            }
        }

        fail_flag
    }

    /// `ZTAnimal::getFootprint`'s `is_egg`/`is_boxed` branches virtual-dispatch through
    /// `entity_type`'s vtable unconditionally (regardless of `use_map_footprint`) - same fixture
    /// requirement as `ZTUnit` above.
    fn run_ztanimal_get_footprint_tests(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTANIMAL_GET_FOOTPRINT";
        let mut fail_flag = false;

        match runner.run(
            &(
                (-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32),
                -1000i32..1000i32,
                (-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32),
                (-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32),
                -8i32..8i32,
                proptest::bool::ANY,
                proptest::bool::ANY,
                proptest::bool::ANY,
            ),
            |((fx, fy, fz), map_footprint, box_footprint, egg_footprint, rotation, use_map_footprint, is_egg, is_boxed)| {
                let entity_type = ZTAnimalType::new_for_test(
                    IVec3::new(fx, fy, fz),
                    map_footprint,
                    IVec3::new(box_footprint.0, box_footprint.1, box_footprint.2),
                    IVec3::new(egg_footprint.0, egg_footprint.1, egg_footprint.2),
                );
                let entity = ZTAnimal::new_for_test(&raw const entity_type as u32, rotation, 0, is_egg, is_boxed);

                let reimplemented_result = entity.get_footprint(use_map_footprint);
                let real_result = call_original_get_footprint(ZTANIMAL_GET_FOOTPRINT, &raw const entity as *const u32, use_map_footprint);

                assert_eq!(
                    (real_result.x, real_result.y, real_result.z),
                    (reimplemented_result.x, reimplemented_result.y, reimplemented_result.z),
                    "ZTAnimal::get_footprint mismatch: is_egg={}, is_boxed={}, rotation={}, use_map_footprint={}, real={:?}, reimplemented={:?}",
                    is_egg, is_boxed, rotation, use_map_footprint, real_result, reimplemented_result
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
                if let proptest::test_runner::TestError::Fail(r, ((fx, fy, fz), map_footprint, box_footprint, egg_footprint, rotation, use_map_footprint, is_egg, is_boxed)) = e {
                    let failure_line = format!(
                        "{}: fx={}, fy={}, fz={}, map_footprint={}, box_footprint={:?}, egg_footprint={:?}, rotation={}, use_map_footprint={}, is_egg={}, is_boxed={}, reason={}\n",
                        test_name, fx, fy, fz, map_footprint, box_footprint, egg_footprint, rotation, use_map_footprint, is_egg, is_boxed, r
                    );
                    if let Some(log_file) = failure_log {
                        if let Err(write_err) = log_file.write_all(failure_line.as_bytes()) {
                            error!("Failed to write to failure log: {}", write_err);
                        }
                    }
                    fail_flag = true;
                }
            }
        }

        fail_flag
    }

    /// ZTRESEARCHBRANCH_FUNDING: compares the real `ZTResearchBranch::increaseFunding`/
    /// `decreaseFunding` against the reimplemented `increase_funding`/`decrease_funding`, for two
    /// independently-constructed but structurally identical standalone branches (built via
    /// `live_support::build_standalone_funding_branch`, same as `ZTRESEARCHBRANCH_FUNDING_TEXT` -
    /// mutating `current_funding_level`, so each side needs its own branch rather than sharing one).
    /// Only the funding table's *length* matters here (`increase_funding`/`decrease_funding` never read
    /// an entry's own content, only `funding_level_count()`), so every generated entry is a dummy
    /// `(0, 0.0)`. `current_funding_level` spans `-2..4` to cover negative/in-range/top-of-range/
    /// one-past-the-end starting values against funding tables spanning empty (`0`) through a few
    /// entries, for both `increase_funding` and `decrease_funding`.
    fn run_research_branch_funding_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTRESEARCHBRANCH_FUNDING";
        let mut fail_flag = false;

        match runner.run(&(-2i32..4i32, 0usize..5, proptest::bool::ANY), |(current_funding_level, level_count, increase)| {
            let levels = vec![(0i32, 0.0f32); level_count];

            let real_branch_ptr = live_support::build_standalone_funding_branch(current_funding_level, &levels);
            if increase {
                unsafe { ztresearchbranch::INCREASE_FUNDING.original()(real_branch_ptr as *const u32) };
            } else {
                unsafe { ztresearchbranch::DECREASE_FUNDING.original()(real_branch_ptr as *const u32) };
            }
            let real_level = unsafe { &*real_branch_ptr }.current_funding_level();
            live_support::destroy_standalone_funding_branch(real_branch_ptr);

            let reimpl_branch_ptr = live_support::build_standalone_funding_branch(current_funding_level, &levels);
            let reimpl_branch = unsafe { &mut *reimpl_branch_ptr };
            if increase {
                reimpl_branch.increase_funding();
            } else {
                reimpl_branch.decrease_funding();
            }
            let reimpl_level = reimpl_branch.current_funding_level();
            live_support::destroy_standalone_funding_branch(reimpl_branch_ptr);

            prop_assert_eq!(
                real_level,
                reimpl_level,
                "current_funding_level mismatch for current_funding_level={}, level_count={}, increase={}",
                current_funding_level,
                level_count,
                increase
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

    /// ZTMARKETING_INCREASE_FUNDING: compares the real `ZTMarketing::increaseFunding` against the
    /// reimplemented `increase_funding`, on two independent standalone `ZTMarketing`s (not spliced into
    /// any `ZTMarketingMgr`, since `increaseFunding` only reads/writes `this`). `current_funding_level`
    /// spans `0..6` against funding tables of `0..5` entries, to cover in-range/top-of-range/
    /// one-past-the-end starting values. Compares both the resulting index and vanilla's masked
    /// low-byte return value.
    fn run_marketing_increase_funding_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
    fn run_marketing_decrease_funding_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
    fn run_marketing_set_funding_level_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
    fn run_marketingmgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTMARKETINGMGR_UPDATE";
        let mut fail_flag = false;

        match runner.run(&(any::<u32>(), any::<u32>()), |(tick_accumulator_before, delta_ticks)| {
            let real_ptr = marketing_live_support::build_standalone_marketing_mgr(tick_accumulator_before, std::ptr::null_mut());
            unsafe { ZTMARKETINGMGR_UPDATE.original()((real_ptr as *mut ZTMarketingMgr) as *const u32, delta_ticks) };
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
    /// accumulator bookkeeping already covered by `ZTMARKETINGMGR_UPDATE` above. Run from
    /// `run_on_completion_reset_test_and_exit`'s `updateSim` injection point, since `GLOBAL_ZTGameMgr`
    /// isn't constructed yet at the earlier `LOAD_LANG_DLLS` battery.
    ///
    /// The funding table has `1..5` entries (`ZTMarketing::update`'s unchecked
    /// `funding_level(current_funding_level)` read needs a real, non-empty table to be safe), with
    /// `current_funding_level` spanning the whole table rather than fixed at index `0`.
    ///
    /// `available_cash` is generated as `cash_delta * cash_multiplier` for `cash_multiplier` in
    /// `0.0..2.0` (`cash_delta` computed the same way as `ZTMarketing::update`'s own
    /// `DAYS_TO_FUNDING_SCALE` formula below), so roughly half of generated cases land unaffordable and
    /// half affordable - taking the real affordable `<=` branch calls
    /// `ZooStatus::spendMarketing`/`ZTGameMgr::subtractCash` on the real `GLOBAL_ZTGameMgr` singleton on
    /// both sides. This used to be restricted to the *insufficient-cash* case only, because of a real
    /// bug: `openzt-detour/src/generated.rs`'s `SUBTRACT_CASH` `FunctionDef` declared one `f32` stack
    /// arg, but the real `ZTGameMgr::subtractCash` takes `(f32, bool)` per its `.asm`'s `RET 8` - a
    /// 4-byte stack imbalance on every `.original()` call. That's now fixed (see
    /// `ztmarketing-update-setmoneytext-crash-investigation.md`'s "Resolution" section), so the
    /// affordable branch is safe to exercise here too. The exact `available_cash == cash_delta`
    /// boundary is separately covered deterministically by `run_marketing_update_boundary_test` (real
    /// side only) and `run_marketing_update_reimpl_boundary_test` (reimplemented side only) below.
    fn run_marketing_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
                    ZTMARKETINGMGR_UPDATE.original()((real_mgr_ptr as *mut ZTMarketingMgr) as *const u32, delta_ticks);
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

    /// Deterministic single-case reproduction of the `ZTMARKETING_UPDATE` affordable-branch crash.
    /// Calls only the real `ZTMarketingMgr::update`, skipping the reimplemented side entirely, so a
    /// crash unambiguously means the real vanilla call path. `available_cash` is pinned to exactly
    /// `cash_delta` (the `<=` boundary itself, never exercised by `run_marketing_update_test` above),
    /// forcing the real side onto the affordable `spendMarketing`/`subtractCash` branch every time.
    ///
    /// Runs unconditionally as part of the normal battery. This whole module only compiles under the
    /// `reimplementation-tests` feature (never the shipped `openzt.dll`), so there's no production
    /// exposure to gate against. The crash this reproduces only occurs when `zoo.exe` is running under
    /// Windows' "Windows 7" compatibility-mode shim - with that shim off, this runs clean. If this test
    /// ever crashes the battery, check `zoo.exe`'s Compatibility tab before assuming a regression.
    ///
    /// This test's crash risk was always independent of the (now-fixed) `SUBTRACT_CASH` stack-imbalance
    /// bug documented on `run_marketing_update_test` above - it only ever calls genuine vanilla code via
    /// `.original()`, never routing through our own buggy `FunctionDef`. See
    /// `run_marketing_update_reimpl_boundary_test` below for the counterpart that exercises the
    /// reimplemented side, the path that *was* affected by that bug.
    fn run_marketing_update_boundary_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
            ZTMARKETINGMGR_UPDATE.original()((real_mgr_ptr as *mut ZTMarketingMgr) as *const u32, delta_ticks);
        });
        info!("{}: real call returned without crashing", test_name);

        marketing_live_support::destroy_standalone_marketing_mgr(real_mgr_ptr);
        marketing_live_support::destroy_standalone_marketing(real_marketing_ptr);

        write_success_line(failure_log, test_name);
        false
    }

    /// Deterministic single-case regression for the *reimplemented* side of `ZTMARKETING_UPDATE`'s affordable
    /// branch - the actual previously-buggy path (`ZTMarketing::update` -> `ZTGameMgr::spend_marketing`/
    /// `subtract_cash`, routed through our own `SPEND_MARKETING`/`SUBTRACT_CASH` `FunctionDef`s - unlike
    /// `run_marketing_update_boundary_test` above, which only ever calls genuine vanilla). See
    /// `ztmarketing-update-setmoneytext-crash-investigation.md`'s "Resolution" section for the fixed
    /// `SUBTRACT_CASH` signature bug, and its "Suggested next steps" item 7 for why this path specifically
    /// needed an independent check - it had never been exercised past the always-unaffordable branch before.
    ///
    /// Calls only the reimplemented `ZTMarketingMgr::update`, skipping the real vanilla side entirely.
    /// `available_cash` is pinned to exactly `cash_delta`, forcing the affordable branch every time. Runs
    /// unconditionally as part of the normal battery, same rationale as `run_marketing_update_boundary_test`.
    fn run_marketing_update_reimpl_boundary_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
    /// the table's length. Same shape as `run_funding_text_test` above, reusing its
    /// `funding_level_case_strategy` for the (name_id, cost) generation.
    fn run_marketing_funding_text_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
    fn run_marketingmgr_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
    fn run_marketingmgr_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
    /// Unlike every other real-vanilla call this file makes elsewhere (which only ever read memory),
    /// `clearConfigurations` actually **frees** the owned `ZTMarketing` - and freeing a Rust
    /// `Box`-allocated `ZTMarketing` through vanilla's own destructor/delete path is a cross-heap risk.
    /// So the "real" side's `ZTMarketing` is instead allocated via the native `standalone::OPERATOR_NEW`
    /// and initialized via `ztmarketing::CONSTRUCTOR`, keeping the real free heap-consistent with how
    /// the memory was allocated.
    fn run_marketingmgr_clear_configurations_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
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
            unsafe { ZTMARKETINGMGR_CLEAR_CONFIGURATIONS.original()((real_mgr_ptr as *mut ZTMarketingMgr) as *const u32) };
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

    /// ZTMARKETINGMGR_DTOR: verifies the fix for the real correctness bug `ztmarketing.rs`'s
    /// `marketing_dtor_detour` module doc comment describes - vanilla's own `ZTMARKETING_MGR_1`
    /// scalar-deleting destructor, if ever allowed to run over a Rust-`Vec`-allocated funding table,
    /// would call `operator delete` on memory Rust's global allocator owns (the same cross-allocator
    /// hazard CLAUDE.md's "Live Reimplementation-Comparison Tests" section documents for
    /// `ZTThoughtMgr`). This deliberately never calls `.original()()` against Rust-allocated memory -
    /// that would just reproduce the crash the fix exists to prevent.
    ///
    /// Two independent halves, like `run_marketingmgr_clear_configurations_test` above:
    /// - **Real**: a fresh, genuinely vanilla-allocated `ZTMarketingMgr`+`ZTMarketing` (empty funding
    ///   table, same as that test's real side), torn down via `ZTMARKETING_MGR_1.original()` with
    ///   `flags=0` (never deletes `this`) - real-allocated, real-freed, so this is safe regardless of the
    ///   fix and just confirms the real destructor is still callable/well-behaved and returns `this`.
    /// - **Reimplemented**: a standalone, Rust-`Vec`-allocated non-empty funding table (via
    ///   `live_support::build_standalone_marketing_with_levels`), torn down via `ZTMarketingMgr::destroy`
    ///   directly - the actual free path the fix protects.
    fn run_marketingmgr_dtor_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTMARKETINGMGR_DTOR";

        let real_marketing_raw = unsafe { standalone::OPERATOR_NEW.original()(size_of::<ZTMarketing>() as u32) };
        if real_marketing_raw.is_null() {
            error!("{}: OPERATOR_NEW returned null for ZTMarketing, skipping real-side check", test_name);
        } else {
            let real_marketing_ptr = unsafe { ztmarketing::CONSTRUCTOR.original()(real_marketing_raw as *const u32) } as *mut ZTMarketing;
            let real_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, real_marketing_ptr);
            let real_this = unsafe { ZTMARKETINGMGR_DTOR.original()((real_mgr_ptr as *mut ZTMarketingMgr) as *const u32, 0u8) };
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
    fn run_marketingmgr_load_configurations_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

    /// Real vanilla's `daysRemainingOnProgram` runs its whole `FSUB`/`FMUL`/`FDIV` chain on the x87
    /// stack, which computes at 80-bit extended precision internally and only rounds down to a
    /// 32-bit `f32` once, at the very end (when the caller stores the `ST(0)` return value) - unlike
    /// `days_remaining_on_program`'s Rust arithmetic, which (per strict IEEE-754 `f32` semantics,
    /// with no `ST(0)`-equivalent extended-precision accumulator) rounds to `f32` after *each*
    /// intermediate operation. Confirmed live: for `target_cost=8832.339, current_progress=0.0,
    /// funding_rate=2.0866816`, real vanilla returns `126981.6` and this crate's arithmetic returns
    /// `126981.59` - a one-part-in-1.6e7 difference, consistent with a single-ULP x87-vs-strict-`f32`
    /// rounding difference, not a logic bug (a real formula/order-of-operations bug would produce a
    /// far larger, `target_cost`/`rate`-dependent difference, not a fixed few-ULP one). So this
    /// comparison uses a relative-tolerance check for `days`, not `prop_assert_eq!`'s exact equality.
    fn days_approximately_eq(real: Option<f32>, reimpl: Option<f32>) -> bool {
        match (real, reimpl) {
            (None, None) => true,
            (Some(real), Some(reimpl)) => {
                // 64 ULPs' worth of relative slack for a 3-operation `f32` chain - generous next to
                // the single-ULP-scale difference actually observed live, but still ~1500x tighter
                // than the smallest realistic logic-bug difference (e.g. a missing `* 30.0`, or
                // dividing by the wrong field, both of which change the result by orders of magnitude).
                let tolerance = 64.0 * f32::EPSILON * real.abs().max(reimpl.abs()).max(1.0);
                (real - reimpl).abs() <= tolerance
            }
            _ => false,
        }
    }

    /// ZTRESEARCHBRANCH_PCT_DAYS_REMAINING: compares the real `ZTResearchBranch::pctRemainingOnProgram`/
    /// `daysRemainingOnProgram` (`ztresearchbranch::PCT_REMAINING_ON_PROGRAM`/`DAYS_REMAINING_ON_PROGRAM`
    /// - a Ghidra regen has since fixed these `FunctionDef`s' auto-detected signatures, which were
    /// originally wrong: `-> i64` and no return type at all, respectively. See `pct_remaining_on_program`'s
    /// own doc comment in `ztresearch.rs` for the disassembly evidence that drove that fix) against the
    /// reimplemented `pct_remaining_on_program`/`days_remaining_on_program`, on a single branch built via
    /// `live_support::build_update_test_branch`. Both real and reimplemented sides read the exact same
    /// branch instance - these methods are `&self`-only with no side effects, so unlike the funding-level
    /// tests above there's no need to build two independent trees. `target_cost` includes an explicit
    /// `0.0` case alongside a general range: dividing by zero, `pct`'s only real edge case (`days`
    /// divides by `rate`, never `target_cost` - see its own doc comment in `ztresearch.rs`), produces a
    /// NaN/±Infinity that has to survive `pct`'s float-to-int conversion - this is exactly the case that
    /// originally caught the reimplementation's `f32 as i32` saturating cast disagreeing with vanilla's
    /// `FISTP`-based one (see `pct_remaining_on_program`'s own doc comment). Whether there's a real
    /// "None" for a given case is derived from `current_funding_rate() > 0.0` (the same guard both real
    /// and reimplemented code apply) rather than trusting `pct`'s raw `-1` return as a sentinel - `-1` is
    /// also a legitimate in-range percentage (e.g. progress just past target_cost), so it can't be told
    /// apart from the guard-failure sentinel by value alone.
    fn run_research_branch_pct_days_remaining_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTRESEARCHBRANCH_PCT_DAYS_REMAINING";
        let mut fail_flag = false;

        let target_cost_strategy = prop_oneof![Just(0.0f32), -10000.0f32..10000.0f32];

        match runner.run(&(target_cost_strategy, -10000.0f32..10000.0f32, -5.0f32..5.0f32), |(target_cost, current_progress, funding_rate)| {
            let branch_ptr = live_support::build_update_test_branch(target_cost, current_progress, funding_rate, 0.0);
            let branch_ref = unsafe { &*branch_ptr };

            let raw_real_pct = unsafe { ztresearchbranch::PCT_REMAINING_ON_PROGRAM.original()(branch_ptr as *const u32) };
            let raw_real_days = unsafe { ztresearchbranch::DAYS_REMAINING_ON_PROGRAM.original()(branch_ptr as *const u32) };
            let rate_contributing = branch_ref.current_funding_rate().is_some_and(|rate| rate > 0.0);
            let real_pct = rate_contributing.then_some(raw_real_pct);
            let real_days = rate_contributing.then_some(raw_real_days);

            let reimpl_pct = branch_ref.pct_remaining_on_program();
            let reimpl_days = branch_ref.days_remaining_on_program();

            live_support::destroy_update_test_branch(branch_ptr);

            prop_assert_eq!(
                real_pct,
                reimpl_pct,
                "pct_remaining_on_program mismatch for target_cost={}, current_progress={}, funding_rate={}",
                target_cost,
                current_progress,
                funding_rate
            );
            prop_assert!(
                days_approximately_eq(real_days, reimpl_days),
                "days_remaining_on_program mismatch for target_cost={}, current_progress={}, funding_rate={}: real={:?}, reimpl={:?}",
                target_cost,
                current_progress,
                funding_rate,
                real_days,
                reimpl_days
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

    // ============================================================================================
    // Live comparison battery for ZTThoughtMgr/ZTThought. `ZTTHOUGHTMGR_ADD_THOUGHT`/
    // `_REMOVE_THOUGHTS_BY_*`/`_GET_THOUGHTS_BY_*`/`_SAVE`/`_LOAD` are self-contained (no
    // `GLOBAL_ZTWorldMgr`/string-table dependency) and run from the early battery above;
    // `_POPULATE_THOUGHTS` and `ZTTHOUGHT_GET_STRING` need `GLOBAL_ZTWorldMgr`/language DLLs
    // respectively, so they run from `run_on_completion_reset_test_and_exit`'s later chain instead.
    // ============================================================================================

    /// Field tuple used to compare two `ZTThought`s structurally via their existing public getters -
    /// `ZTThought` derives neither `PartialEq` nor a public constructor, so this is simpler than adding
    /// either just for these tests.
    fn thought_fields(t: &ZTThought) -> (u32, u32, u32, i32, i32, u32, u32, u32) {
        (t.string_id(), t.thinker_id(), t.object_id(), t.tile_x(), t.tile_y(), t.thinker_ptr(), t.object_ptr(), t.habitat_ptr())
    }

    /// ZTTHOUGHTMGR_ADD_THOUGHT: compares the real `ZTThoughtMgr::addThought`'s effect on list
    /// order/length against the reimplemented `add_thought`, across a generated sequence of calls and a
    /// small `max_thoughts` cap (so cap-trimming is actually exercised). Restricted to
    /// `thinker_ptr = object_ptr = habitat_ptr = 0` for every call - `ZTThought::new` dereferences all
    /// three when non-null, and there's no live entity/habitat this standalone test could safely point
    /// them at. This still fully exercises `addThought`'s own cap-trim/insertion-order logic, the part
    /// this test actually targets.
    fn run_thoughtmgr_add_thought_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_ADD_THOUGHT";
        let mut fail_flag = false;

        match runner.run(&(1u32..5, prop::collection::vec(any::<u32>(), 0..8)), |(max_thoughts, string_ids)| {
            let real_ptr = thought_live_support::build_standalone_mgr(max_thoughts);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(max_thoughts);

            for &string_id in &string_ids {
                unsafe {
                    gen_ztthoughtmgr::ADD_THOUGHT.original()(real_ptr as *const u32, string_id, std::ptr::null(), std::ptr::null(), std::ptr::null());
                }
                unsafe { &mut *reimpl_ptr }.add_thought(string_id, 0, 0, 0);
            }

            let real_thoughts = thought_live_support::read_raw_chain(unsafe { &*real_ptr });
            let real_fields: Vec<_> = real_thoughts.iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();
            let real_len = real_thoughts.len();
            let reimpl_len = unsafe { &*reimpl_ptr }.len();

            thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(real_len, reimpl_len, "length mismatch for max_thoughts={}, string_ids={:?}", max_thoughts, string_ids);
            prop_assert_eq!(real_fields, reimpl_fields, "content mismatch for max_thoughts={}, string_ids={:?}", max_thoughts, string_ids);
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

    /// Seeds `mgr`'s reimplemented-side store with one `ZTThought` per `specs` entry (`(string_id,
    /// thinker_ptr, object_ptr, habitat_ptr)`), front-to-back, via `insert_front` - for the side of a
    /// comparison driven through a direct reimplemented-method call. `thinker_id`/`object_id`/`tile_x`/
    /// `tile_y` are left at ctor defaults since no consumer of this helper reads them.
    fn seed_thoughts(mgr: &mut ZTThoughtMgr, specs: &[(u32, u32, u32, u32)]) {
        for &(string_id, thinker_ptr, object_ptr, habitat_ptr) in specs {
            mgr.insert_front(thought_live_support::new_thought(string_id, 0, 0, -1, -1, thinker_ptr, object_ptr, habitat_ptr));
        }
    }

    /// Seeds `mgr`'s raw `sentinel_ptr` chain (not its reimplemented-side store) with one `ZTThought` per
    /// `specs` entry, front-to-back, via `seed_raw_chain` - for the "real" side of a comparison driven
    /// through a genuine, undetoured `.original()` call, which reads `sentinel_ptr` directly and knows
    /// nothing about the reimplemented-side store.
    fn seed_thoughts_raw(mgr: &ZTThoughtMgr, specs: &[(u32, u32, u32, u32)]) {
        for &(string_id, thinker_ptr, object_ptr, habitat_ptr) in specs {
            thought_live_support::seed_raw_chain(mgr, thought_live_support::new_thought(string_id, 0, 0, -1, -1, thinker_ptr, object_ptr, habitat_ptr));
        }
    }

    /// Seeds `mgr` on *both* representations at once (raw chain and reimplemented-side store) with
    /// identical content - for tests that drive a *single* instance through both a real `.original()`
    /// call (reads the raw chain) and a direct reimplemented-method call (reads the store), e.g. the
    /// `getThoughtsBy*` comparisons below.
    fn seed_thoughts_both(mgr: &mut ZTThoughtMgr, specs: &[(u32, u32, u32, u32)]) {
        seed_thoughts_raw(mgr, specs);
        seed_thoughts(mgr, specs);
    }

    fn thought_spec_strategy() -> impl Strategy<Value = (u32, u32, u32, u32)> {
        (any::<u32>(), 0u32..5, 0u32..5, 0u32..5)
    }

    /// ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_THINKER: compares the real `ZTThoughtMgr::removeThoughtsByThinker`
    /// against the reimplemented `remove_thoughts_by_thinker`, on two identically-seeded standalone
    /// managers. `thinker_ptr`/`object_ptr`/`habitat_ptr` are generated over a small `0..5` range so
    /// `target` collides with a seeded value often enough to exercise real removals, not just the no-op
    /// case.
    fn run_thoughtmgr_remove_thoughts_by_thinker_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_THINKER";
        let mut fail_flag = false;

        match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5), |(specs, target)| {
            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
            seed_thoughts_raw(unsafe { &*real_ptr }, &specs);
            seed_thoughts(unsafe { &mut *reimpl_ptr }, &specs);

            unsafe {
                gen_ztthoughtmgr::REMOVE_THOUGHTS_BY_THINKER.original()(real_ptr as *const u32, target as *const u32);
            }
            unsafe { &mut *reimpl_ptr }.remove_thoughts_by_thinker(target);

            let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

            thought_live_support::free_raw_chain_mgr(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}", specs, target);
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

    /// ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_OBJECT: same shape as
    /// `run_thoughtmgr_remove_thoughts_by_thinker_test`, comparing `removeThoughtsByObject`/
    /// `remove_thoughts_by_object` instead.
    fn run_thoughtmgr_remove_thoughts_by_object_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_OBJECT";
        let mut fail_flag = false;

        match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5), |(specs, target)| {
            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
            seed_thoughts_raw(unsafe { &*real_ptr }, &specs);
            seed_thoughts(unsafe { &mut *reimpl_ptr }, &specs);

            unsafe {
                gen_ztthoughtmgr::REMOVE_THOUGHTS_BY_OBJECT.original()(real_ptr as *const u32, target as *const u32);
            }
            unsafe { &mut *reimpl_ptr }.remove_thoughts_by_object(target);

            let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

            thought_live_support::free_raw_chain_mgr(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}", specs, target);
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

    /// ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_HABITAT: same shape again, additionally generating `force` -
    /// `removeThoughtsByHabitat` has a third outcome `removeThoughtsBy{Thinker,Object}` don't: a
    /// matching thought with a live `object_ptr` survives with its `habitat_ptr` link cleared instead of
    /// being removed outright, unless `force` is set.
    fn run_thoughtmgr_remove_thoughts_by_habitat_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_HABITAT";
        let mut fail_flag = false;

        match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, any::<bool>()), |(specs, target, force)| {
            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
            seed_thoughts_raw(unsafe { &*real_ptr }, &specs);
            seed_thoughts(unsafe { &mut *reimpl_ptr }, &specs);

            unsafe {
                gen_ztthoughtmgr::REMOVE_THOUGHTS_BY_HABITAT.original()(real_ptr as *const u32, target as *const i32, force as i8);
            }
            unsafe { &mut *reimpl_ptr }.remove_thoughts_by_habitat(target, force);

            let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

            thought_live_support::free_raw_chain_mgr(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, force={}", specs, target, force);
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

    /// ZTTHOUGHTMGR_GET_THOUGHTS_BY_THINKER: compares the real, undetoured `getThoughtsByThinker`'s
    /// output - a real vanilla temporary list, walked read-only via
    /// `thought_live_support::read_raw_chain_from_sentinel` - against the reimplemented
    /// `get_thoughts_by_thinker`, on a single standalone manager seeded on both representations via
    /// `seed_thoughts_both` (the real call reads its raw `sentinel_ptr` chain; the reimplemented call
    /// reads its `THOUGHT_STORES` entry - a single instance can drive both since they're independent
    /// storage, no need for two separate instances).
    fn run_thoughtmgr_get_thoughts_by_thinker_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_GET_THOUGHTS_BY_THINKER";
        let mut fail_flag = false;

        match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, 1i32..5), |(specs, target, max_count)| {
            let mgr_ptr = thought_live_support::build_standalone_mgr(1000);
            seed_thoughts_both(unsafe { &mut *mgr_ptr }, &specs);
            let mgr = unsafe { &*mgr_ptr };

            let mut real_sentinel: u32 = 0;
            unsafe {
                gen_ztthoughtmgr::GET_THOUGHTS_BY_THINKER.original()(
                    mgr_ptr as *const u32,
                    &raw mut real_sentinel as *const i32,
                    target as *const i32,
                    max_count,
                );
            }
            let real_fields: Vec<_> = thought_live_support::read_raw_chain_from_sentinel(real_sentinel).iter().map(thought_fields).collect();

            let reimpl_thoughts = mgr.get_thoughts_by_thinker(target, max_count as usize);
            let reimpl_fields: Vec<_> = reimpl_thoughts.iter().map(thought_fields).collect();

            thought_live_support::destroy_standalone_mgr_both(mgr_ptr);

            prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, max_count={}", specs, target, max_count);
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

    /// ZTTHOUGHTMGR_GET_THOUGHTS_BY_OBJECT: same shape as
    /// `run_thoughtmgr_get_thoughts_by_thinker_test`, for `getThoughtsByObject`/`get_thoughts_by_object`.
    /// `max_count` is passed as `max_count as *const i32` - `GET_THOUGHTS_BY_OBJECT`'s `*const i32`
    /// signature is a Ghidra type-inference artifact; the real calling convention passes the count by
    /// value for all three `getThoughtsBy*` functions.
    fn run_thoughtmgr_get_thoughts_by_object_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_GET_THOUGHTS_BY_OBJECT";
        let mut fail_flag = false;

        match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, 1i32..5), |(specs, target, max_count)| {
            let mgr_ptr = thought_live_support::build_standalone_mgr(1000);
            seed_thoughts_both(unsafe { &mut *mgr_ptr }, &specs);
            let mgr = unsafe { &*mgr_ptr };

            let mut real_sentinel: u32 = 0;
            unsafe {
                gen_ztthoughtmgr::GET_THOUGHTS_BY_OBJECT.original()(
                    mgr_ptr as *const u32,
                    &raw mut real_sentinel as *const i32,
                    target as *const i32,
                    max_count as *const i32,
                );
            }
            let real_fields: Vec<_> = thought_live_support::read_raw_chain_from_sentinel(real_sentinel).iter().map(thought_fields).collect();

            let reimpl_thoughts = mgr.get_thoughts_by_object(target, max_count as usize);
            let reimpl_fields: Vec<_> = reimpl_thoughts.iter().map(thought_fields).collect();

            thought_live_support::destroy_standalone_mgr_both(mgr_ptr);

            prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, max_count={}", specs, target, max_count);
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

    /// ZTTHOUGHTMGR_GET_THOUGHTS_BY_HABITAT: same shape as
    /// `run_thoughtmgr_get_thoughts_by_object_test`, including `max_count`'s own by-value passing - for
    /// `getThoughtsByHabitat`/`get_thoughts_by_habitat`.
    fn run_thoughtmgr_get_thoughts_by_habitat_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_GET_THOUGHTS_BY_HABITAT";
        let mut fail_flag = false;

        match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, 1i32..20), |(specs, target, max_count)| {
            let mgr_ptr = thought_live_support::build_standalone_mgr(1000);
            seed_thoughts_both(unsafe { &mut *mgr_ptr }, &specs);
            let mgr = unsafe { &*mgr_ptr };

            let mut real_sentinel: u32 = 0;
            unsafe {
                gen_ztthoughtmgr::GET_THOUGHTS_BY_HABITAT.original()(
                    mgr_ptr as *const u32,
                    &raw mut real_sentinel as *const i32,
                    target as *const i32,
                    max_count as *const i32,
                );
            }
            let real_fields: Vec<_> = thought_live_support::read_raw_chain_from_sentinel(real_sentinel).iter().map(thought_fields).collect();

            let reimpl_thoughts = mgr.get_thoughts_by_habitat(target, max_count as usize);
            let reimpl_fields: Vec<_> = reimpl_thoughts.iter().map(thought_fields).collect();

            thought_live_support::destroy_standalone_mgr_both(mgr_ptr);

            prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, max_count={}", specs, target, max_count);
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

    /// ZTTHOUGHTMGR_SAVE: compares the real `ZTThoughtMgr::save`'s captured output (via `io_redirect`)
    /// against the reimplemented `save`, on two identically-seeded standalone managers.
    fn run_thoughtmgr_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_SAVE";
        let mut fail_flag = false;

        let record_strategy = (any::<u32>(), any::<u32>(), any::<u32>(), -2i32..4, -2i32..4);
        match runner.run(&prop::collection::vec(record_strategy, 0..6), |records| {
            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
            for &(string_id, thinker_id, object_id, tile_x, tile_y) in &records {
                thought_live_support::seed_raw_chain(
                    unsafe { &*real_ptr },
                    thought_live_support::new_thought(string_id, thinker_id, object_id, tile_x, tile_y, 0, 0, 0),
                );
                unsafe { &mut *reimpl_ptr }.insert_front(thought_live_support::new_thought(string_id, thinker_id, object_id, tile_x, tile_y, 0, 0, 0));
            }

            let dummy_file: u32 = 0;
            io_redirect::begin_capture();
            unsafe { gen_ztthoughtmgr::SAVE.original()(real_ptr as *const u32, &dummy_file as *const u32) };
            let real_bytes = io_redirect::end_capture();

            io_redirect::begin_capture();
            let _ = unsafe { &*reimpl_ptr }.save(&dummy_file as *const u32);
            let reimpl_bytes = io_redirect::end_capture();

            thought_live_support::free_raw_chain_mgr(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(real_bytes, reimpl_bytes, "save byte mismatch for records={:?}", records);
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

    /// ZTTHOUGHTMGR_LOAD: compares the real `ZTThoughtMgr::load`'s effect on list content/order (and its
    /// own return value) against the reimplemented `load`, for a generated stream of legacy-format
    /// `(string_id, object_id, thinker_id)` records and `version < 0x1e` - the pre-`0x1e` legacy branch.
    /// Restricted to this range: `version >= 0x1e` triggers `ZTThought::load`'s own
    /// `thinker_id`/`object_id` -> pointer resolution via `ZTWorldMgr::resolve_entity_by_id`, which
    /// needs `GLOBAL_ZTWorldMgr` initialized - not true yet at this early injection point.
    /// `object_id`/`thinker_id` are generated over a small `0..3` range to land on both sides of
    /// `ZTThoughtMgr::load`'s survival gate (a legacy-format record only splices into the list if both
    /// ids end up `0`), not just the trivially-true `id == 0` case `any::<u32>()` would mostly hit.
    fn run_thoughtmgr_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_LOAD";
        let mut fail_flag = false;

        let record_strategy = (any::<u32>(), 0u32..3, 0u32..3);
        match runner.run(&(prop::collection::vec(record_strategy, 0..6), 0u32..0x1e), |(records, version)| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
            for &(string_id, object_id, thinker_id) in &records {
                bytes.extend_from_slice(&string_id.to_le_bytes());
                bytes.extend_from_slice(&object_id.to_le_bytes());
                bytes.extend_from_slice(&thinker_id.to_le_bytes());
            }

            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
            let file_buffer = [0u32; 4];

            io_redirect::begin_replay(bytes.clone());
            let real_ret = unsafe { gen_ztthoughtmgr::LOAD.original()(real_ptr as *const u32, file_buffer.as_ptr(), version) };
            io_redirect::end_replay();

            io_redirect::begin_replay(bytes);
            let reimpl_ret = unsafe { &mut *reimpl_ptr }.load(file_buffer.as_ptr(), version);
            io_redirect::end_replay();

            let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

            thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(real_ret, reimpl_ret, "load() return mismatch for records={:?}, version={}", records, version);
            prop_assert_eq!(real_fields, reimpl_fields, "loaded content mismatch for records={:?}, version={}", records, version);
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

    // ============================================================================================
    // ZTAwardMgr - see openzt/src/ztawardmgr.rs and openzt/plans/ztawardmgr-implementation-plan.md.
    // `_ADD_AWARD_SAVE_LOAD`/`_START`/`_GET_AWARD` and `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT` are
    // self-contained (resources are already loaded by this early injection point, and none of them
    // need `GLOBAL_ZTWorldMgr`), so they run from the early battery above. `_SHOW_AWARDS` needs a live
    // `BFUIMgr` element, so it runs from `run_on_completion_reset_test_and_exit`'s later chain instead,
    // after `run_load_live_zoo`.
    // ============================================================================================

    /// Resets both the real vanilla singleton's earned-id vector and the Rust-side store to empty.
    /// Exploits `ZTAwardMgr::load`'s own "reset-then-fill" semantics as a safe, allocator-agnostic clear
    /// for the real side (feeding a single `0i32` count via `io_redirect::begin_replay` means the real
    /// `load` resets the vector, reads a `0` count, and returns immediately without calling `addAward`)
    /// - there's no dedicated clear method and no way to build a second standalone instance for this
    /// class (see `ztawardmgr.rs`'s module doc comment).
    fn reset_awardmgr_both_sides() {
        let real_ptr = award_live_support::real_ptr();
        let file_buffer = [0u32; 4];
        io_redirect::begin_replay(0u32.to_le_bytes().to_vec());
        unsafe { gen_ztawardmgr::LOAD.original()(real_ptr, file_buffer.as_ptr(), 0) };
        io_redirect::end_replay();
        award_live_support::reset_reimplemented_store();
    }

    /// ZTAWARDMGR_ADD_AWARD_SAVE_LOAD: for a generated sequence of ids, feeds the same sequence through
    /// the real `ADD_AWARD.original()` and the reimplemented `ztawardmgr::add_award` independently (both
    /// sides reset to empty first via `reset_awardmgr_both_sides`), then compares the real
    /// `SAVE.original()`'s captured output (via `io_redirect`) against the reimplemented `save`'s own
    /// output - should be byte-identical, since both reproduce the exact `i32` count + `i32[count]` wire
    /// format.
    fn run_awardmgr_add_award_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTAWARDMGR_ADD_AWARD_SAVE_LOAD";
        let mut fail_flag = false;

        match runner.run(&prop::collection::vec(any::<i32>(), 0..8), |ids| {
            let real_ptr = award_live_support::real_ptr();
            reset_awardmgr_both_sides();

            for &id in &ids {
                unsafe { gen_ztawardmgr::ADD_AWARD.original()(real_ptr, id) };
                ztawardmgr::add_award(id);
            }

            let dummy_file: u32 = 0;
            io_redirect::begin_capture();
            unsafe { gen_ztawardmgr::SAVE.original()(real_ptr, &dummy_file as *const u32 as *const i8) };
            let real_bytes = io_redirect::end_capture();

            io_redirect::begin_capture();
            let _ = ztawardmgr::save(&dummy_file as *const u32);
            let reimpl_bytes = io_redirect::end_capture();

            prop_assert_eq!(real_bytes, reimpl_bytes, "save byte mismatch for ids={:?}", ids);
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

    /// ZTAWARDMGR_START: calls the real `START.original()` against the live singleton (whose tree is
    /// still empty at this early injection point) to populate it from the real live `award.cfg`
    /// resource, then calls the reimplemented `ztawardmgr::start()` against the same resource data.
    /// Compares every `(id, name_id, tooltip_id)` triple - `award_live_support::read_vanilla_award_tree`
    /// reads the real tree via a read-only in-order walk (never mutates/frees anything, so safe
    /// regardless of which allocator built the nodes), `award_live_support::reimplemented_award_triples`
    /// reads the Rust-side `BTreeMap` (already sorted by id, matching the in-order walk's order). Not a
    /// proptest - there's exactly one real `award.cfg`/one real answer to compare, matching
    /// `ZTMARKETINGMGR_LOAD_CONFIGURATIONS`'s precedent.
    fn run_awardmgr_start_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTAWARDMGR_START";
        let real_ptr = award_live_support::real_ptr();

        let real_ok = (unsafe { gen_ztawardmgr::START.original()(real_ptr) } & 0xff) != 0;
        let real_tree = award_live_support::read_vanilla_award_tree();

        let reimpl_ok = ztawardmgr::start();
        let reimpl_tree = award_live_support::reimplemented_award_triples();

        if real_ok == reimpl_ok && real_tree == reimpl_tree {
            info!("{} passed ({} awards)", test_name, real_tree.len());
            write_success_line(failure_log, test_name);
            false
        } else {
            error!(
                "{} failed: real_ok={}, reimpl_ok={}, real_tree={:?}, reimpl_tree={:?}",
                test_name, real_ok, reimpl_ok, real_tree, reimpl_tree
            );
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!(
                        "Test Failed {}: real_ok={}, reimpl_ok={}, real_tree={:?}, reimpl_tree={:?}\n",
                        test_name, real_ok, reimpl_ok, real_tree, reimpl_tree
                    )
                    .as_bytes(),
                );
            }
            true
        }
    }

    /// ZTAWARDMGR_GET_AWARD: for every id `ZTAWARDMGR_START` found in the real tree (plus one
    /// guaranteed-absent id), compares the real `GET_AWARD.original()`'s dereferenced `+0x14`/`+0x18`
    /// fields (or "not found", when the raw returned pointer is `0`) against the reimplemented
    /// `ztawardmgr::get_award`. Must run after `run_awardmgr_start_test` - relies on both trees already
    /// being populated identically.
    fn run_awardmgr_get_award_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTAWARDMGR_GET_AWARD";
        let real_ptr = award_live_support::real_ptr();
        let real_tree = award_live_support::read_vanilla_award_tree();

        let mut ids: Vec<i32> = real_tree.iter().map(|&(id, _, _)| id).collect();
        let absent_id = ids.iter().copied().max().unwrap_or(0).wrapping_add(1_000_000);
        ids.push(absent_id);

        let mut fail_flag = false;
        for id in ids {
            let real_result_ptr = unsafe { gen_ztawardmgr::GET_AWARD.original()(real_ptr, id) };
            let real = if real_result_ptr == 0 {
                None
            } else {
                Some((get_from_memory::<i32>(real_result_ptr as u32), get_from_memory::<i32>(real_result_ptr as u32 + 4)))
            };

            let reimpl = ztawardmgr::get_award(id).map(|a| (a.name_id(), a.tooltip_id()));

            if real != reimpl {
                error!("{} mismatch for id={}: real={:?}, reimpl={:?}", test_name, id, real, reimpl);
                if let Some(log_file) = failure_log {
                    let _ =
                        log_file.write_all(format!("Test Failed {}: id={}, real={:?}, reimpl={:?}\n", test_name, id, real, reimpl).as_bytes());
                }
                fail_flag = true;
            }
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        }
        fail_flag
    }

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
    fn run_ztscenariosimplegoal_eval_award_count_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

    /// Counts `UIListBox` items by walking `GET_ITEM(index)` until it returns `0` past the end - the same
    /// bounds-check-confirmed technique `ztshowui.rs`'s `copy_list_to_script` already relies on (see that
    /// call site's own doc comment for the `.asm` cross-check), so no separate item-count bookkeeping
    /// needs to be replicated here.
    fn listbox_item_count(listbox: *const u32) -> i32 {
        let mut index = 0i32;
        loop {
            if unsafe { gen_uilistbox::GET_ITEM.original()(listbox, index) } == 0 {
                return index;
            }
            index += 1;
            if index > 10_000 {
                return index;
            }
        }
    }

    /// ZTAWARDMGR_SHOW_AWARDS: real diff-oracle comparison of the reimplemented `_showAwards` detour
    /// against real vanilla. Seeds both the real singleton and the Rust store with the same two catalogue
    /// award ids (from `ZTAWARDMGR_START`, which has already run earlier in this battery), clears the
    /// listbox and populates it via real vanilla (`ztawardmgr::show_awards_detour::call_real`'s `retour`
    /// trampoline - **not** `SHOW_AWARDS.original()`, which in release is a raw address cast that would
    /// just loop back into this same detour once it's hooked; debug `.original()` routes through the hook
    /// registry, but `call_real` keeps the vanilla pole release-safe), counts items via [`listbox_item_count`], then repeats
    /// against the hooked address (our detour, driven by the Rust store) and compares counts. Runs after
    /// `run_load_live_zoo` since it needs a live `BFUIMgr` element `0x101c` to exist.
    ///
    /// Only item *counts* are compared, not per-item content - per `ztawardmgr.rs`'s `show_awards_detour`
    /// doc comment, the icon-buffer/color-argument shape and the `load_string_by_id`-vs-`buildString` text
    /// equivalence remain open items needing separate manual live verification, since `UIListBoxItem`'s
    /// internal field layout for those isn't decompile-confirmed. A count mismatch still catches real bugs
    /// (wrong catalogue filtering, an id silently dropped, an off-by-one in the population loop).
    ///
    /// **Resets both sides, then re-runs `ztawardmgr::start()`, in that order** - not the reverse.
    /// `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT` runs immediately before this test (in this same
    /// post-`run_load_live_zoo` battery) and its own `reset_awardmgr_both_sides()` call clears the
    /// Rust-side catalogue too (`reset_reimplemented_store` clears both `earned_ids` and `awards`), not
    /// just earned-ids - real vanilla's own catalogue tree is untouched by that reset (`ZTAwardMgr::load`
    /// only resets the earned-ids vector), so only the Rust side needs repopulating. **Confirmed live,
    /// twice**: populating the catalogue *before* this function's own `reset_awardmgr_both_sides()` call
    /// left `reimplemented_award_triples()` empty again immediately afterward (the reset doesn't
    /// distinguish "just populated" from stale) - `real_count=2, reimpl_count=0` on the first live run of
    /// this rewritten test, a genuine catch by the new diff oracle, though of a test-harness ordering bug
    /// rather than the detour itself. Resetting first, then calling `start()`, avoids that.
    fn run_awardmgr_show_awards_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTAWARDMGR_SHOW_AWARDS";

        let global_bfuimgr = (get_module_base("zoo.exe") as u32 + 0x0023_8de0) as *const u32;
        let element = unsafe { BFUIMGR_GET_ELEMENT_0.original()(global_bfuimgr, 0x101c) };
        if element.is_null() {
            info!("Skipping {}: BFUIMgr element 0x101c not resolved", test_name);
            write_success_line(failure_log, &format!("{} (skipped: element not resolved)", test_name));
            return false;
        }

        let real_ptr = award_live_support::real_ptr();
        reset_awardmgr_both_sides();
        ztawardmgr::start();
        let catalogue = award_live_support::reimplemented_award_triples();
        if catalogue.is_empty() {
            info!("Skipping {}: no award catalogue entries available (ZTAWARDMGR_START found none)", test_name);
            write_success_line(failure_log, &format!("{} (skipped: empty catalogue)", test_name));
            return false;
        }

        for &(id, _, _) in catalogue.iter().take(2) {
            unsafe { gen_ztawardmgr::ADD_AWARD.original()(real_ptr, id) };
            ztawardmgr::add_award(id);
        }

        unsafe { gen_uilistbox::CLEAR.original()(element) };
        ztawardmgr::show_awards_detour::call_real();
        let real_count = listbox_item_count(element);

        unsafe { gen_uilistbox::CLEAR.original()(element) };
        let hooked = unsafe { std::mem::transmute::<u32, extern "stdcall" fn()>(0x0053167fu32) };
        hooked();
        let reimpl_count = listbox_item_count(element);

        if real_count == reimpl_count {
            info!("{} passed (real_count={}, reimpl_count={})", test_name, real_count, reimpl_count);
            write_success_line(failure_log, test_name);
            false
        } else {
            error!("{} mismatch: real_count={}, reimpl_count={}", test_name, real_count, reimpl_count);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: real_count={}, reimpl_count={}\n", test_name, real_count, reimpl_count).as_bytes(),
                );
            }
            true
        }
    }

    /// Builds a raw `ZTShowScriptItemRaw` (via `ztshowscriptmgr::live_support::raw_item_matching_type`)
    /// and hands it to `ztshowscriptmgr::add_item`, for a script constructed via `make_registered_show_script`.
    fn add_matching_item(script_ptr: u32, script_type: u32, trick_id: u16) {
        let item = ztshowscriptmgr::live_support::raw_item_matching_type(script_type, trick_id);
        ztshowscriptmgr::add_item(script_ptr, &item);
    }

    /// Registers one script directly via `ztshowscriptmgr::register_script` (the exact Rust function
    /// Stage 1's `REGISTER_SCRIPT` detour itself calls into) and adds one matching-type item via
    /// [`add_matching_item`]. Returns the assigned script id.
    ///
    /// **Deliberately does not go through the real `ZTShowScript::ZTShowScript` ctor's own
    /// `auto_register=true` path** (`ztshowscript::CONSTRUCTOR`) - not because that path is broken, but
    /// because `register_script` directly is simpler for a helper called dozens of times across this
    /// file's test battery. An earlier session found that calling the ctor live at *this* injection point
    /// (before `run_load_live_zoo`) left the id it wrote back at `+0x4` unregistered in Stage 1's store,
    /// and flagged it as an open, possibly-significant reimplementation gap. `ZTSHOWSCRIPT_CTOR_
    /// REGISTRATION_LIVE` (this file, runs after `run_load_live_zoo`) resolved that: with a live
    /// `GLOBAL_ZTShowMgr` (confirmed via `globals().ztshowmgr_ptr()`), the real ctor's `auto_register=true`
    /// path registers correctly - the earlier finding was this harness's own early-injection-point timing,
    /// per `ZTShowScript_ZTShowScript.c:25`'s `GLOBAL_ZTShowMgr != 0` guard, the same class of gap already
    /// documented here for `GLOBAL_ZTGameMgr`. `ztshowui::copy_list_to_script`'s own identical ctor call
    /// (the one real, confirmed production consumer of this exact path) is therefore genuinely safe, not
    /// just assumed so - see that function's own doc comment for the pointer to this confirmation.
    fn make_registered_show_script(script_type: u32, trick_id: u16) -> u16 {
        let alloc = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
        let id = ztshowscriptmgr::register_script(alloc, script_type).expect("register_script should never reject a non-null ctor_ptr");
        add_matching_item(alloc, script_type, trick_id);
        id
    }

    /// ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE: `ZTShowScriptMgr::save`/`load` (Stage 1's `SAVE`/`LOAD`
    /// detours, `ztshowscriptmgr::save_mgr`/`load_mgr`) are, like `ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS`
    /// above, full-replacement detours over an independent Rust store with no vanilla-layout struct to
    /// diff against - but unlike those, `load_mgr`'s own *read* side (`read_item`/`read_script`) had zero
    /// coverage before this: the existing `#[cfg(test)]` tests in `ztshowscriptmgr.rs` only pin
    /// `encode_item`/`encode_mgr`'s byte offsets on the write side. Registers two scripts (one with two
    /// items, one with one) via `make_registered_show_script`/`add_matching_item`, calls `SAVE`'s own
    /// real, now-hooked address (`0x00479f44`) with `io_redirect` capturing the write, resets the store,
    /// replays the captured bytes through `LOAD`'s own real, now-hooked address (`0x004c6ebd`), and
    /// asserts every script/item field round-tripped. Uses a fixed literal version (`0x100`) comfortably
    /// above all three save-format gates (`read_item`/`read_script` gate at `0x58`/`0x66`, the counter
    /// restore gates at `0x60`) - no "current save version" constant exists elsewhere in this codebase to
    /// reuse.
    fn run_ztshowscriptmgr_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE";
        const CURRENT_VERSION: u32 = 0x100;

        ztshowscriptmgr::live_support::reset_state();
        const SCRIPT_TYPE_A: u32 = 11;
        const SCRIPT_TYPE_B: u32 = 22;
        let script_a = make_registered_show_script(SCRIPT_TYPE_A, 101);
        add_matching_item(ztshowscriptmgr::get_script(script_a), SCRIPT_TYPE_A, 102);
        let script_b = make_registered_show_script(SCRIPT_TYPE_B, 201);

        let mut fail_flag = false;
        let dummy_file: u32 = 0;

        let save_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, *const i8) -> u32>(0x00479f44u32) };
        io_redirect::begin_capture();
        save_hooked(&dummy_file as *const u32, &dummy_file as *const u32 as *const i8);
        let captured_bytes = io_redirect::end_capture();

        ztshowscriptmgr::live_support::reset_state();

        let load_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, *const u32, u32) -> u32>(0x004c6ebdu32) };
        io_redirect::begin_replay(captured_bytes);
        let load_ok = (load_hooked(&dummy_file as *const u32, &dummy_file as *const u32, CURRENT_VERSION) & 0xff) != 0;
        io_redirect::end_replay();

        if !load_ok {
            error!("{}: LOAD returned failure", test_name);
            fail_flag = true;
        }
        if !ztshowscriptmgr::script_exists_by_id(script_a) || !ztshowscriptmgr::script_exists_by_id(script_b) {
            error!("{}: one or both scripts missing after round-trip (a={}, b={})", test_name, script_a, script_b);
            fail_flag = true;
        }
        if ztshowscriptmgr::script_type_by_id(script_a) != Some(SCRIPT_TYPE_A) {
            error!("{}: script_a type mismatch after round-trip", test_name);
            fail_flag = true;
        }
        if ztshowscriptmgr::script_type_by_id(script_b) != Some(SCRIPT_TYPE_B) {
            error!("{}: script_b type mismatch after round-trip", test_name);
            fail_flag = true;
        }
        if ztshowscriptmgr::script_item_count_by_id(script_a) != 2 {
            error!("{}: script_a item count mismatch: expected 2, got {}", test_name, ztshowscriptmgr::script_item_count_by_id(script_a));
            fail_flag = true;
        }
        if ztshowscriptmgr::script_item_count_by_id(script_b) != 1 {
            error!("{}: script_b item count mismatch: expected 1, got {}", test_name, ztshowscriptmgr::script_item_count_by_id(script_b));
            fail_flag = true;
        }
        match ztshowscriptmgr::item_full_by_id(script_a, 0) {
            Some(item) if item.id == 101 && item.item_type == SCRIPT_TYPE_A => {}
            other => {
                error!("{}: script_a item 0 mismatch after round-trip: {:?}", test_name, other);
                fail_flag = true;
            }
        }
        match ztshowscriptmgr::item_full_by_id(script_a, 1) {
            Some(item) if item.id == 102 && item.item_type == SCRIPT_TYPE_A => {}
            other => {
                error!("{}: script_a item 1 mismatch after round-trip: {:?}", test_name, other);
                fail_flag = true;
            }
        }
        match ztshowscriptmgr::item_full_by_id(script_b, 0) {
            Some(item) if item.id == 201 && item.item_type == SCRIPT_TYPE_B => {}
            other => {
                error!("{}: script_b item 0 mismatch after round-trip: {:?}", test_name, other);
                fail_flag = true;
            }
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE: diagnosing a real save-corruption report (load a real
    /// zoo, save, reload -> "corrupted saved game"/a capacity-overflow panic). Unlike
    /// `ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE` above, which only ever round-trips two small synthetic
    /// scripts, this snapshots whatever *real* scripts/items `run_load_live_zoo` just populated from
    /// `reimplementation-test-zoo.zoo` (real string content, real field values - not the hand-built
    /// matching-type-only items every other live test in this group uses), encodes them via
    /// `ztshowscriptmgr::encode_mgr` (through `snapshot_encoded`, bypassing `WriteBytesToFile`/
    /// `io_redirect` entirely - only the *read* side needs the hooked-address replay mechanism), decodes
    /// them back via the real `load_mgr` (through `io_redirect::begin_replay`, since `read_bytes`
    /// internally calls `DEALLOCATE.hooked()`), and asserts every script's type and every item's full
    /// field set is byte-identical before/after. Registered first in `live_zoo_tests` (before any other
    /// entry that adds/mutates scripts) so the snapshot reflects the zoo file's own as-loaded data, not
    /// this battery's own synthetic additions.
    fn run_ztshowscriptmgr_real_zoo_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE";
        const CURRENT_VERSION: u32 = 0x100;
        let mut fail_flag = false;

        let before_ids = ztshowscriptmgr::live_support::all_script_ids();
        if before_ids.is_empty() {
            info!("{}: no real scripts registered from the loaded zoo - nothing to round-trip, skipping", test_name);
            write_success_line(failure_log, &format!("{} (skipped: no real scripts)", test_name));
            return false;
        }

        fn snapshot(ids: &[u16]) -> Vec<(u16, Option<u32>, Vec<ztshowscriptmgr::ShowScriptItem>)> {
            ids.iter()
                .map(|&id| {
                    let script_type = ztshowscriptmgr::script_type_by_id(id);
                    let count = ztshowscriptmgr::script_item_count_by_id(id) as u16;
                    let items = (0..count).filter_map(|i| ztshowscriptmgr::item_full_by_id(id, i)).collect();
                    (id, script_type, items)
                })
                .collect()
        }

        let before = snapshot(&before_ids);
        let encoded = ztshowscriptmgr::live_support::snapshot_encoded();
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("CHECKPOINT {} scripts={} encoded_len={}\n", test_name, before_ids.len(), encoded.len()).as_bytes(),
            );
        }

        let dummy_file: u32 = 0;
        let file_ptr = &dummy_file as *const u32;
        io_redirect::begin_replay(encoded);
        let load_ok = ztshowscriptmgr::load_mgr(file_ptr, CURRENT_VERSION);
        io_redirect::end_replay();

        if !load_ok {
            error!("{}: load_mgr returned failure re-decoding the real zoo's own encoded script data", test_name);
            fail_flag = true;
        }

        let after_ids = ztshowscriptmgr::live_support::all_script_ids();
        let after = snapshot(&after_ids);

        if before != after {
            error!("{}: real zoo script data did not round-trip byte-identically through encode_mgr/load_mgr", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: before={:?}\nafter={:?}\n", test_name, before, after).as_bytes());
            }
            fail_flag = true;
        }

        if !fail_flag {
            info!("{}: {} real script(s) round-tripped byte-identically", test_name, before_ids.len());
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWSCRIPTMGR_LOAD_VERSION_GATES_LIVE: `ztshowscriptmgr::load_mgr` is already `pub fn`, so unlike
    /// the round-trip test above, this hand-builds byte buffers directly and calls it directly - no need
    /// to go through the hooked address (real `SAVE` always writes the current format, so it can never
    /// naturally produce an old-version stream). Exercises the format's version gates directly:
    /// - `version <= 0x58`: the store is cleared but the stream is never read at all (empty buffer,
    ///   `load_mgr` still returns `true`).
    /// - `0x58 < version <= 0x66`: an item's base fields are read but `normalHelpID`/`grayedHelpID`/icon
    ///   strings are left at [`crate::ztshowscriptmgr::ShowScriptItem::default`]'s values (that half of
    ///   the buffer is never written/read).
    /// - `version > 0x60` (independent of the `0x66` gate above): the trailing `makeID` counter is read
    ///   and restored - checked via `ztshowscriptmgr::live_support::next_id_counter`; a version at/under
    ///   the gate with no trailing bytes still succeeds (never attempts the read), while a version over
    ///   the gate with the trailing bytes missing is a genuine short read and `load_mgr` returns `false`.
    /// - A string length prefix `>= STRING_LENGTH_CAP` makes `load_mgr` return `false` immediately (the
    ///   same guard `read_string` applies to every string field).
    fn run_ztshowscriptmgr_load_version_gates_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWSCRIPTMGR_LOAD_VERSION_GATES_LIVE";
        let mut fail_flag = false;
        let dummy_file: u32 = 0;
        let file_ptr = &dummy_file as *const u32;

        macro_rules! check {
            ($cond:expr, $msg:expr) => {
                if !($cond) {
                    error!("{}: {}", test_name, $msg);
                    fail_flag = true;
                }
            };
        }

        // version <= 0x58: store cleared, stream never read, still reports success.
        ztshowscriptmgr::live_support::reset_state();
        let _ = make_registered_show_script(1, 1);
        io_redirect::begin_replay(Vec::new());
        let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x58);
        io_redirect::end_replay();
        check!(ok, "version<=0x58 should return true");
        check!(ztshowscriptmgr::live_support::registered_script_count() == 0, "version<=0x58 should clear the store");

        // 0x58 < version <= 0x66: base fields read, extended fields stay default. Also stays <= 0x60, so
        // no trailing counter bytes are needed/read.
        ztshowscriptmgr::live_support::reset_state();
        {
            let mut buf = Vec::new();
            buf.extend_from_slice(&1u32.to_le_bytes()); // script count
            buf.extend_from_slice(&55u16.to_le_bytes()); // script id
            buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
            buf.extend_from_slice(&9u32.to_le_bytes()); // script_type
            buf.extend_from_slice(&1u32.to_le_bytes()); // item count
            buf.push(1); // default_available
            buf.push(1); // visible
            buf.extend_from_slice(&77u16.to_le_bytes()); // id
            buf.extend_from_slice(&9u32.to_le_bytes()); // item_type
            buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
            for _ in 0..4 {
                buf.extend_from_slice(&0u32.to_le_bytes()); // name/anim/keeperPreTrick/keeperPostTrick, all empty
            }
            buf.extend_from_slice(&5u32.to_le_bytes()); // building
            buf.extend_from_slice(&3u32.to_le_bytes()); // complexity
            buf.push(0); // return_to_keeper
            buf.extend_from_slice(&10u32.to_le_bytes()); // satisfaction
            buf.extend_from_slice(&11u32.to_le_bytes()); // satisfaction_delta
            buf.extend_from_slice(&12u32.to_le_bytes()); // satisfaction_mirror
            buf.extend_from_slice(&13u32.to_le_bytes()); // minimum_depth
            io_redirect::begin_replay(buf);
            let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x60);
            io_redirect::end_replay();
            check!(ok, "0x58<version<=0x66 should return true");
            match ztshowscriptmgr::item_full_by_id(55, 0) {
                Some(item) => {
                    check!(item.building == 5 && item.satisfaction == 10, "base fields should have been read");
                    check!(
                        item.normal_help_id == 0 && item.grayed_help_id == 0 && item.normal_icon.is_empty() && item.grayed_icon.is_empty(),
                        "extended fields should stay at their default for version<=0x66"
                    );
                }
                None => check!(false, "expected script 55 item 0 to exist after load"),
            }
        }

        // version > 0x60 with the trailing counter present: counter restored.
        ztshowscriptmgr::live_support::reset_state();
        {
            let mut buf = Vec::new();
            buf.extend_from_slice(&0u32.to_le_bytes()); // 0 scripts
            buf.extend_from_slice(&0x1234u16.to_le_bytes()); // makeID counter
            io_redirect::begin_replay(buf);
            let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x70);
            io_redirect::end_replay();
            check!(ok, "version>0x60 with a trailing counter should return true");
            check!(ztshowscriptmgr::live_support::next_id_counter() == 0x1234, "counter should have been restored for version>0x60");
        }

        // version > 0x60 with the trailing counter bytes missing: a genuine short read, load_mgr fails.
        ztshowscriptmgr::live_support::reset_state();
        {
            let buf = 0u32.to_le_bytes().to_vec(); // 0 scripts, no counter bytes follow
            io_redirect::begin_replay(buf);
            let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x70);
            io_redirect::end_replay();
            check!(!ok, "version>0x60 missing its trailing counter bytes should return false");
        }

        // A string length prefix >= STRING_LENGTH_CAP fails immediately - no script/item header even
        // needed after it, `read_string` returns `None` before attempting to read the (absent) bytes.
        ztshowscriptmgr::live_support::reset_state();
        {
            let mut buf = Vec::new();
            buf.extend_from_slice(&1u32.to_le_bytes()); // script count
            buf.extend_from_slice(&1u16.to_le_bytes()); // script id
            buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
            buf.extend_from_slice(&1u32.to_le_bytes()); // script_type
            buf.extend_from_slice(&1u32.to_le_bytes()); // item count
            buf.push(0); // default_available
            buf.push(1); // visible
            buf.extend_from_slice(&1u16.to_le_bytes()); // id
            buf.extend_from_slice(&1u32.to_le_bytes()); // item_type
            buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
            buf.extend_from_slice(&0x1000u32.to_le_bytes()); // name length prefix == STRING_LENGTH_CAP
            io_redirect::begin_replay(buf);
            let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x70);
            io_redirect::end_replay();
            check!(!ok, "a string length prefix >= STRING_LENGTH_CAP should return false");
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE: `ZTShowInfo::addScript`/`checkPendingScripts`
    /// (`ztshowinfo::ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS`) are full-replacement detours over Stage 1's
    /// independent `ZTShowScriptMgr` store, so - unlike `ZTAWARDMGR_SHOW_AWARDS`/
    /// `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT` above, which each still have a real vanilla trampoline to
    /// diff against - there's no real-vs-reimplementation diff oracle to compare against here at all (no
    /// vanilla-layout struct backs Stage 1's store). Instead: builds a standalone `ZTShowInfo` (`ztshow_live_support::
    /// build_standalone_show_info` - a zeroed `OPERATOR_NEW(0xb0)` buffer, **not** the real
    /// `ZTShowInfo::ZTShowInfo` ctor, which unconditionally dereferences an unconfirmed `GLOBAL_ZTAIMgr`
    /// field - see that helper's own doc comment), registers two real `ZTShowScript`s (via
    /// [`make_registered_show_script`], each with one matching-type item so `has_items` is true for both),
    /// then calls through `ADD_SCRIPT`'s and `CHECK_PENDING_SCRIPTS`'s own real, now-*hooked* addresses
    /// directly (same "call the patched address via `transmute`" technique as
    /// `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT` above) and asserts the resulting state directly in the
    /// standalone buffer's own pending-scripts-tree memory and in Stage 1's store.
    ///
    /// Runs after `run_load_live_zoo` for parity with the other real-zoo-dependent tests, though this one
    /// deliberately doesn't actually depend on `GLOBAL_ZTGameMgr` being live - see the next paragraph.
    ///
    /// **Real, live-crash-reproducing finding from this test's first run**: `GLOBAL_ZTGameMgr` is *still*
    /// null at every injection point in this test battery, confirmed directly by `ZTSCENARIOSIMPLEGOAL_
    /// EVAL_AWARD_COUNT`'s own "(skipped: ZTGameMgr not initialized)" log line appearing even *after*
    /// `run_load_live_zoo` (`run_load_live_zoo`'s `FOPEN`/`LOAD_FILE`/`FCLOSE` sequence loads the world/
    /// habitat data directly, bypassing the normal scenario-start flow that would otherwise construct
    /// `ZTGameMgr`). `add_script`'s `was_inserted` branch calls `GET_DATE.original()(globals().
    /// ztgamemgr_ptr(), ...)` unconditionally on a first-ever insert - matching real vanilla `addScript`'s
    /// own decompile exactly (`ZTShowInfo_addScript.c`'s `ZTGameMgr::getDate(GLOBAL_ZTGameMgr, ...)`, also
    /// unconditional) - which crashed this whole test process outright the first time this test actually
    /// ran (no earlier stage-2 live test had ever exercised a first-ever pending-scripts insert before, so
    /// this went undetected until now). Real vanilla's own lack of a null check isn't a vanilla bug - a
    /// real game session always has a live `ZTGameMgr` by the time a habitat can have a show, `addScript`
    /// can be called at all - it's specifically this test harness's own early injection point that can
    /// reach `addScript` before `ZTGameMgr` exists. Worked around here (not "fixed" in `ztshow.rs`, since
    /// there's nothing wrong with the reimplementation) by pre-inserting the pending-scripts node directly
    /// via `find_or_insert_pending_script_node` *before* calling the hooked `ADD_SCRIPT`, so its own
    /// internal `find_or_insert` call finds an existing node (`was_inserted == false`) and never reaches
    /// the `GET_DATE` call at all.
    ///
    /// **Second, independent bug found and fixed while building this test** (see `ztshow.rs`'s
    /// `find_or_insert_pending_script_node` for the full writeup): that function used to also maintain a
    /// "rightmost" cache at the pending-scripts tree header's `+0xc`, which actually aliases `ZTShowInfo`'s
    /// own real `addShow`/`removeShow` dynamic array's `begin` pointer (`+0x50`) - corrupting it on every
    /// first-ever insert for a given `ZTShowInfo`, which would crash the very next real `ADD_SHOW`/
    /// `REMOVE_SHOW` call (an unbounded scan against a corrupted `begin`/still-null `end`). This is a
    /// genuine, previously-unexercised live gameplay-corruption bug - Stage 2's `addScript`/
    /// `checkPendingScripts` had no live test until now (the plan's own open item 11) - fixed by dropping
    /// the "rightmost" cache concept entirely (no real vanilla consumer of it was ever found).
    fn run_ztshowinfo_add_script_check_pending_scripts_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE";

        let show_info = ztshow_live_support::build_standalone_show_info();
        const UNIT_TYPE_ID: u32 = 0x7fff_1234;
        const SCRIPT_TYPE: u32 = 7;

        // Pre-insert the pending-scripts node ourselves - see this function's own doc comment on why
        // `ADD_SCRIPT`'s own internal insert can't be allowed to run with GLOBAL_ZTGameMgr still null.
        let _ = ztshow::find_or_insert_pending_script_node(show_info, UNIT_TYPE_ID);

        let script_a = make_registered_show_script(SCRIPT_TYPE, 1);
        let script_b = make_registered_show_script(SCRIPT_TYPE, 2);

        let mut fail_flag = false;

        // Call through ADD_SCRIPT's own real, now-hooked address directly (0x0046e8b5, ztshowinfo::ADD_SCRIPT).
        let add_script_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, u32, u16) -> bool>(0x0046e8b5u32) };
        let add_ok = add_script_hooked(show_info as *const u32, UNIT_TYPE_ID, script_a);
        if !add_ok {
            error!("{}: ADD_SCRIPT returned false", test_name);
            fail_flag = true;
        }

        // The very first insert for UNIT_TYPE_ID becomes the pending-scripts tree's root directly.
        let header = get_from_memory::<u32>(show_info + 0x44);
        let root = get_from_memory::<u32>(header + 4);
        if root == 0 {
            error!("{}: pending-scripts tree has no root after ADD_SCRIPT", test_name);
            fail_flag = true;
        } else {
            let current = get_from_memory::<u16>(root + 0x1c);
            let pending = get_from_memory::<u16>(root + 0x1e);
            if current != script_a {
                error!("{}: expected current={}, got {} after ADD_SCRIPT", test_name, script_a, current);
                fail_flag = true;
            }
            if pending != 0xffff {
                error!("{}: expected pending reset to 0xffff after ADD_SCRIPT, got {:#x}", test_name, pending);
                fail_flag = true;
            }
            if !ztshowscriptmgr::script_exists_by_id(script_a) {
                error!("{}: script_a {} should exist in the store after ADD_SCRIPT", test_name, script_a);
                fail_flag = true;
            }

            // Simulate a queued pending change (the same state `add_script` itself would leave behind if
            // the show were already started - simpler/more direct to poke it here than to also fake
            // `isStarted()`'s own real precondition chain) and exercise CHECK_PENDING_SCRIPTS through its
            // own real, now-hooked address (0x005a876a, ztshowinfo::CHECK_PENDING_SCRIPTS).
            save_to_memory(root + 0x1e, script_b);
            let check_pending_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32)>(0x005a876au32) };
            check_pending_hooked(show_info as *const u32);

            let current_after = get_from_memory::<u16>(root + 0x1c);
            let pending_after = get_from_memory::<u16>(root + 0x1e);
            if current_after != script_b {
                error!("{}: expected current={} after CHECK_PENDING_SCRIPTS, got {}", test_name, script_b, current_after);
                fail_flag = true;
            }
            if pending_after != 0xffff {
                error!("{}: expected pending reset to 0xffff after CHECK_PENDING_SCRIPTS, got {:#x}", test_name, pending_after);
                fail_flag = true;
            }
            if ztshowscriptmgr::script_exists_by_id(script_a) {
                error!("{}: old current script_a {} should have been dropped from the store after CHECK_PENDING_SCRIPTS", test_name, script_a);
                fail_flag = true;
            }
            if !ztshowscriptmgr::script_exists_by_id(script_b) {
                error!("{}: script_b {} should still exist in the store after CHECK_PENDING_SCRIPTS", test_name, script_b);
                fail_flag = true;
            }
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWINFO_PENDING_SCRIPT_TREE_STRESS_LIVE: stress-tests `find_or_insert_pending_script_node`'s BST
    /// insert logic (`ztshow.rs` - the exact function `ztshowscriptmgr-open-items.md`'s bug 1, the phantom
    /// "rightmost" cache corruption, was found and fixed in) against a real, standalone `ZTShowInfo`
    /// (`ztshow_live_support::build_standalone_show_info`). Generates a fixed-seed-shuffled sequence of
    /// distinct `unit_type_id`s (a trivial inline xorshift32, no new crate dependency), inserts each in
    /// turn, and after every insert asserts the header's `leftmost` cache (`+0x8`) matches the running
    /// minimum key inserted so far - the exact invariant bug 1 violated. Finishes by re-inserting every id
    /// a second time (asserting `was_inserted == false` and the same node address each time) and a full
    /// in-order walk (`ztshow_live_support::collect_pending_script_nodes`) asserting strictly ascending key
    /// order.
    fn run_ztshowinfo_pending_script_tree_stress_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWINFO_PENDING_SCRIPT_TREE_STRESS_LIVE";
        let show_info = ztshow_live_support::build_standalone_show_info();

        // Trivial fixed-seed xorshift32 - deterministic across runs, no new crate dependency.
        let mut state: u32 = 0x9e3779b9;
        let mut next_rand = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };

        const COUNT: usize = 256;
        // Distinct unit_type_ids in a small range, Fisher-Yates shuffled via the xorshift generator above.
        let mut ids: Vec<u32> = (1..=COUNT as u32).collect();
        for i in (1..ids.len()).rev() {
            let j = (next_rand() as usize) % (i + 1);
            ids.swap(i, j);
        }

        let mut fail_flag = false;
        let mut min_seen: Option<u32> = None;
        let mut node_by_id: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();

        for &id in &ids {
            let (node, was_inserted) = ztshow::find_or_insert_pending_script_node(show_info, id);
            if !was_inserted {
                error!("{}: first insert of id {} unexpectedly reported was_inserted=false", test_name, id);
                fail_flag = true;
            }
            node_by_id.insert(id, node);
            min_seen = Some(min_seen.map_or(id, |m| m.min(id)));

            let header = get_from_memory::<u32>(show_info + 0x44);
            let leftmost = get_from_memory::<u32>(header + 8);
            let leftmost_key = get_from_memory::<u32>(leftmost + 0x10);
            if leftmost_key != min_seen.unwrap() {
                error!(
                    "{}: after inserting id {}, leftmost cache key is {} but running minimum is {}",
                    test_name, id, leftmost_key, min_seen.unwrap()
                );
                fail_flag = true;
            }
        }

        // Re-inserting every id should now be a no-op find, returning the same node and was_inserted=false.
        for &id in &ids {
            let (node, was_inserted) = ztshow::find_or_insert_pending_script_node(show_info, id);
            if was_inserted {
                error!("{}: re-inserting already-seen id {} reported was_inserted=true", test_name, id);
                fail_flag = true;
            }
            let expected = node_by_id.get(&id).copied().unwrap_or(0);
            if node != expected {
                error!("{}: re-inserting id {} returned a different node address ({:#010x} vs {:#010x})", test_name, id, node, expected);
                fail_flag = true;
            }
        }

        let in_order = ztshow_live_support::collect_pending_script_nodes(show_info);
        if in_order.len() != COUNT {
            error!("{}: in-order walk found {} nodes, expected {}", test_name, in_order.len(), COUNT);
            fail_flag = true;
        }
        let mut prev_key: Option<u32> = None;
        for &node in &in_order {
            let key = get_from_memory::<u32>(node + 0x10);
            if let Some(prev) = prev_key {
                if key <= prev {
                    error!("{}: in-order walk not strictly ascending: {} then {}", test_name, prev, key);
                    fail_flag = true;
                    break;
                }
            }
            prev_key = Some(key);
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// Scans the live, loaded test zoo's real habitats (`globals().zthabitatmgr().exhibit_array()`) for
    /// one that is both a real tank exhibit (`ZTHabitat::is_tank`) **with water** (`water_level() > 0`) -
    /// i.e. NOT `ztshow::check_owning_habitat`'s blocking predicate - **and** already has a real
    /// `ZTShowInfo*` attached (`ZTHabitat::is_show_tank`), i.e. a genuinely-configured, already-working
    /// show tank that real vanilla would let a show start on. Returns `(habitat_ptr, show_info_ptr)` for
    /// the first match, `None` if the test zoo has none.
    fn find_real_show_tank_habitat() -> Option<(u32, u32)> {
        let habitat_mgr = globals().zthabitatmgr();
        let exhibits = habitat_mgr.exhibit_array();
        for i in 0..exhibits.len() {
            let habitat_ptr = exhibits.get_ptr(i);
            if habitat_ptr == 0 {
                continue;
            }
            let habitat = get_from_memory::<ZTHabitat>(habitat_ptr);
            if habitat.is_tank() && habitat.is_show_tank() {
                // Only safe to read as a `ZTTankExhibit` because `is_tank()` above already confirmed
                // this pointer is really a 0x1e8-byte `ZTTankExhibit`, not a plain 0x178-byte `ZTHabitat`.
                let tank = get_from_memory::<crate::zthabitatmgr::ZTTankExhibit>(habitat_ptr);
                if *tank.water_level() > 0 {
                    return Some((habitat_ptr, *habitat.zt_show_info_ptr()));
                }
            }
        }
        None
    }

    /// ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE: diagnosing a real save-corruption report.
    /// The pending-scripts BST at the known show-tank's `ZTShowInfo+0x44`
    /// (`ztshow::find_or_insert_pending_script_node`) is real, live vanilla memory this crate's code
    /// writes to directly (unlike `ZTShowScriptMgr`'s independent store, already proven clean by
    /// `ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE` above) - a corrupted node/cache here would silently
    /// keep the game running (nothing reads it except real, un-reimplemented `checkPendingScripts`/
    /// `enterNewMonth`/etc.) until the next save serializes it. Bounded-iteration walk (matching this
    /// codebase's own "diagnose a BST before trusting it" convention - see
    /// `find_trick_by_id`'s doc comment for the prior real bug this exact style of check caught) over
    /// whatever real tree `run_load_live_zoo` already populated: collects every node via `left`(`+8`)/
    /// `right`(`+0xc`), asserting (1) the walk terminates within a generous bound (no cycle), (2) an
    /// in-order traversal's keys (`+0x10`) come out strictly ascending (the BST invariant, not just "no
    /// cycle"), and (3) the header's own leftmost cache (`+0x8`) - what real, un-reimplemented
    /// `enterNewMonth`/`checkPendingScripts` start their own walk from - equals the address of whichever
    /// node the walk found with the smallest key (the exact invariant a previous version of this
    /// function's cache-maintenance code broke, per `find_or_insert_pending_script_node`'s own doc
    /// comment).
    fn run_ztshow_pending_script_tree_real_zoo_integrity_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE";
        let mut fail_flag = false;

        let Some((_, show_info_ptr)) = find_real_show_tank_habitat() else {
            error!("{}: BLOCKED - no real show-tank habitat found in test zoo", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no qualifying show-tank habitat found\n", test_name).as_bytes());
            }
            return false;
        };

        let header = get_from_memory::<u32>(show_info_ptr + 0x44);
        let root = get_from_memory::<u32>(header + 4);

        const MAX_NODES: usize = 10_000;
        let mut in_order: Vec<(u32, u32)> = Vec::new(); // (addr, key)
        let mut stack: Vec<u32> = Vec::new();
        let mut node = root;
        let mut iterations = 0usize;
        // Standard iterative in-order walk: push left spine, visit, descend right.
        while (node != 0 && node != header) || !stack.is_empty() {
            iterations += 1;
            if iterations > MAX_NODES {
                error!("{}: walk exceeded {} iterations without terminating - likely a cycle in the tree", test_name, MAX_NODES);
                fail_flag = true;
                break;
            }
            if node != 0 && node != header {
                stack.push(node);
                node = get_from_memory::<u32>(node + 8); // left
            } else if let Some(top) = stack.pop() {
                let key = get_from_memory::<u32>(top + 0x10);
                in_order.push((top, key));
                node = get_from_memory::<u32>(top + 0xc); // right
            }
        }

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} header={:#010x} root={:#010x} node_count={}\n", test_name, header, root, in_order.len()).as_bytes());
        }

        if !fail_flag {
            for pair in in_order.windows(2) {
                if pair[0].1 >= pair[1].1 {
                    error!(
                        "{}: in-order keys not strictly ascending ({:#x} @ {:#010x} then {:#x} @ {:#010x}) - BST invariant violated",
                        test_name, pair[0].1, pair[0].0, pair[1].1, pair[1].0
                    );
                    fail_flag = true;
                }
            }
        }

        if !fail_flag && !in_order.is_empty() {
            let real_leftmost = get_from_memory::<u32>(header + 8);
            let expected_leftmost = in_order[0].0; // smallest key, since in-order is ascending
            if real_leftmost != expected_leftmost {
                error!(
                    "{}: header leftmost cache is {:#010x} but the smallest real key ({:#x}) lives at {:#010x} - stale cache (the class of bug find_or_insert_pending_script_node's own doc comment already found once)",
                    test_name, real_leftmost, in_order[0].1, expected_leftmost
                );
                fail_flag = true;
            }
        }

        if !fail_flag {
            info!("{}: pending-script tree ({} real node(s)) is well-formed and leftmost cache is correct", test_name, in_order.len());
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWINFO_REAL_SAVE_LOAD_BYTE_COUNT_LIVE: diagnosing a real save-corruption report. Real, un-
    /// reimplemented `ZTShowInfo::save`/`load` (`ztshowinfo::SAVE`/`LOAD`) walk the pending-scripts tree
    /// at `ZTShowInfo+0x44` - the same tree `ztshow.rs`'s `find_or_insert_pending_script_node`/
    /// `allocate_pending_script_node` builds. This tests the real, un-reimplemented pair directly against
    /// each other on the real show-tank habitat's real `ZTShowInfo` (already carrying 3 real pending-
    /// script nodes from actual gameplay, per `ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE` above -
    /// deliberately not a synthetic/standalone object, which would confound the result with zeroed-out
    /// unrelated fields `ZTShowInfo::save` also reads): captures real `SAVE`'s output
    /// (`io_redirect::begin_capture`), then real-`LOAD`s those exact bytes back into the same live object
    /// (`io_redirect::begin_replay`), and compares `io_redirect::replay_position()` (bytes `LOAD` actually
    /// consumed) against the captured buffer's own length (bytes `SAVE` actually wrote). A mismatch here
    /// pinpoints a genuine save/load byte-count asymmetry for this exact real data - and since both `SAVE`
    /// and `LOAD` are real, untouched vanilla code, a mismatch would mean our own node construction
    /// (`allocate_pending_script_node`'s simplified `+0x18` sub-structure, standing in for whatever real
    /// vanilla's own node constructor builds there) makes vanilla's real save/load disagree about how much
    /// data it wrote - not a defect in vanilla's own save/load pairing itself. Uses `version=106` (`0x6a`)
    /// - not an arbitrary/future value - to match the exact version boundary a real save actually uses
    /// (confirmed live via `DIAG LOAD_ENTER ZTShowMgr version=106` this session), since some of
    /// `ZTShowInfo::load`'s per-field reads are version-gated and a different version would exercise a
    /// different, non-representative code path. Mutates the live show-tank's `ZTShowInfo` in place (real
    /// `LOAD` writes directly into it) - acceptable since this is a one-shot test process that exits after
    /// the battery, matching this file's own established precedent elsewhere.
    fn run_ztshowinfo_real_save_load_byte_count_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWINFO_REAL_SAVE_LOAD_BYTE_COUNT_LIVE";
        let mut fail_flag = false;

        let Some((_, show_info_ptr)) = find_real_show_tank_habitat() else {
            error!("{}: BLOCKED - no real show-tank habitat found in test zoo", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no qualifying show-tank habitat found\n", test_name).as_bytes());
            }
            return false;
        };

        const REAL_VERSION: u32 = 106;
        let dummy_file: u32 = 0;

        io_redirect::begin_capture();
        let save_ok = unsafe { ztshowinfo::SAVE.original()(show_info_ptr as *const u32, &dummy_file as *const u32 as *const i8) };
        let captured_bytes = io_redirect::end_capture();

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("CHECKPOINT {} show_info={:#010x} save_ok={} bytes_written={}\n", test_name, show_info_ptr, save_ok, captured_bytes.len())
                    .as_bytes(),
            );
        }
        if (save_ok & 0xff) == 0 {
            error!("{}: real ZTShowInfo::save returned failure", test_name);
            fail_flag = true;
        }

        let written_len = captured_bytes.len();
        io_redirect::begin_replay(captured_bytes);
        let load_ok = unsafe { ztshowinfo::LOAD.original()(show_info_ptr as *const u32, &dummy_file as *const u32, REAL_VERSION) };
        let consumed_len = io_redirect::replay_position();
        io_redirect::end_replay();

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("CHECKPOINT {} load_ok={} bytes_consumed={} bytes_written={}\n", test_name, load_ok, consumed_len, written_len).as_bytes(),
            );
        }
        if load_ok == 0 {
            error!("{}: real ZTShowInfo::load returned failure replaying its own save's bytes", test_name);
            fail_flag = true;
        }
        if consumed_len != written_len {
            error!(
                "{}: byte-count mismatch - real save() wrote {} bytes but real load() consumed {} bytes ({}) for the same real ZTShowInfo",
                test_name,
                written_len,
                consumed_len,
                if consumed_len > written_len { "load read PAST what save wrote" } else { "load read LESS than save wrote" }
            );
            fail_flag = true;
        }

        if !fail_flag {
            info!("{}: real save()/load() agree exactly on {} bytes for the real show-tank's ZTShowInfo", test_name, written_len);
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTRESEARCHMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE: same "real, live-loaded data" round-trip philosophy as
    /// `ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE` above, applied to `research_save_reimplementation`
    /// (`ZTRESEARCHMGR_SAVE`'s own coverage is proptest-generated synthetic trees only). Captures the
    /// real, hooked `ZTResearchMgr::save`'s output for the real, live `globals().ztresearchmgr()`
    /// singleton (real branches/categories/programs with real funding levels/progress values - richer
    /// than any hand-built test tree), then asserts `research_save_reimplementation::parse` recovers
    /// exactly what `snapshot_mgr` independently read straight from that same live memory. `save()` has
    /// no side effects (a pure `WriteBytesToFile` call), so this is safe to run against the real
    /// singleton directly - no standalone-instance plumbing needed.
    fn run_ztresearchmgr_real_zoo_save_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTRESEARCHMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE";
        let mut fail_flag = false;

        let mgr = globals().ztresearchmgr();
        let expected_records = research_save_reimplementation::snapshot_mgr(mgr);
        if expected_records.is_empty() {
            info!("{}: no real research data loaded - nothing to round-trip, skipping", test_name);
            write_success_line(failure_log, &format!("{} (skipped: no real research data)", test_name));
            return false;
        }

        let dummy_file: u32 = 0;
        io_redirect::begin_capture();
        let save_ok = mgr.save(&dummy_file as *const u32);
        let captured_bytes = io_redirect::end_capture();

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("CHECKPOINT {} records={} bytes={}\n", test_name, expected_records.len(), captured_bytes.len()).as_bytes(),
            );
        }

        if !save_ok {
            error!("{}: real save() returned failure", test_name);
            fail_flag = true;
        }

        match research_save_reimplementation::parse(&captured_bytes) {
            Some(parsed) if parsed == expected_records => {}
            other => {
                error!("{}: parsed real save bytes don't match the independently-read real research state", test_name);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: expected={:?}\nparsed={:?}\n", test_name, expected_records, other).as_bytes());
                }
                fail_flag = true;
            }
        }

        if !fail_flag {
            info!("{}: {} real research record(s) round-tripped byte-identically", test_name, expected_records.len());
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTMARKETINGMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE: real-zoo round-trip coverage for
    /// `openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md`'s `ZTMarketingMgr` item - the only one
    /// of that plan's four managers where a genuine full round-trip against the real singleton is both
    /// safe and easy (`load` is a pure decode with a ready-made pure oracle,
    /// `marketing_save_reimplementation::predict_load`). Snapshots the real, live
    /// `globals().ztmarketingmgr()` singleton's current funding-level index, captures real `save()`'s
    /// bytes (`.hooked()` - the detoured reimplementation, installed unconditionally by this battery's
    /// own `init()`), replays them into real `load()` at the live save-format version, and asserts the
    /// resulting index matches `predict_load`'s prediction computed from the pre-save index/table
    /// length.
    fn run_ztmarketingmgr_real_zoo_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

    /// ZTAWARDMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE: real-zoo round-trip coverage for
    /// `openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md`'s `ZTAwardMgr` item. Unlike that plan's
    /// literal wording (which assumed `earned_ids()` - the Rust-side store - already reflected the real
    /// zoo's earned awards), this test build never installs `ztawardmgr::award_mgr_detours` (see this
    /// file's own `init()` doc comment on why - `.original()` needs to stay reachable for the other
    /// `ZTAWARDMGR_*` tests' real-vanilla comparisons), so the real zoo's own `ZTAwardMgr::load` ran
    /// genuine, undetoured vanilla code against the real singleton's own `+0xc` vector, never touching
    /// the Rust store. This reads that real vector directly
    /// (`award_live_support::read_vanilla_earned_ids`), captures real vanilla `save()`'s own output for
    /// it (`.original()`, since `SAVE` is undetoured here too), replays those bytes into the Rust
    /// reimplementation's `load()` (`crate::ztawardmgr::load`, a plain function - there's no hooked
    /// address to go through), and asserts the reimplementation's resulting `earned_ids()` matches the
    /// real vector, compared as sorted sets per the plan's own caution about `add_award`'s sorted-unique
    /// re-insertion possibly reordering.
    fn run_ztawardmgr_real_zoo_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTAWARDMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE";
        let mut fail_flag = false;

        let real_ids = award_live_support::read_vanilla_earned_ids();
        if real_ids.is_empty() {
            info!("{}: no real earned awards in the loaded zoo - nothing to round-trip, skipping", test_name);
            write_success_line(failure_log, &format!("{} (skipped: no real earned awards)", test_name));
            return false;
        }

        award_live_support::reset_reimplemented_store();

        let real_ptr = award_live_support::real_ptr();
        let dummy_file: u32 = 0;
        io_redirect::begin_capture();
        let save_ok = unsafe { gen_ztawardmgr::SAVE.original()(real_ptr, &dummy_file as *const u32 as *const i8) };
        let captured_bytes = io_redirect::end_capture();

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} real_ids={:?} bytes={}\n", test_name, real_ids, captured_bytes.len()).as_bytes());
        }

        if save_ok == 0 {
            error!("{}: real vanilla save() returned failure", test_name);
            fail_flag = true;
        }

        io_redirect::begin_replay(captured_bytes);
        let load_ok = ztawardmgr::load(&dummy_file as *const u32);
        io_redirect::end_replay();

        if !load_ok {
            error!("{}: reimplementation load() returned failure replaying real vanilla save bytes", test_name);
            fail_flag = true;
        }

        let mut after: Vec<i32> = ztawardmgr::earned_ids();
        let mut expected: Vec<i32> = real_ids.clone();
        after.sort_unstable();
        expected.sort_unstable();

        if after != expected {
            error!(
                "{}: reimplementation earned_ids() didn't match the real vanilla vector after round-tripping (expected={:?}, got={:?})",
                test_name, expected, after
            );
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: expected={:?}\ngot={:?}\n", test_name, expected, after).as_bytes());
            }
            fail_flag = true;
        }

        award_live_support::reset_reimplemented_store();

        if !fail_flag {
            info!("{}: {} real earned award(s) round-tripped through the reimplementation", test_name, real_ids.len());
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTTHOUGHTMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE: real-zoo SAVE-only coverage for
    /// `openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md`'s `ZTThoughtMgr` item. The real zoo's
    /// own thought list lives in real vanilla memory reachable through the live singleton's own
    /// `sentinel_ptr` chain (this test build never installs `ztthoughtmgr`'s own detours, so real
    /// vanilla `ZTThoughtMgr::load` populated that chain directly, never the Rust-side
    /// `THOUGHT_STORES`) - read read-only via `thought_live_support::read_raw_chain`. Captures real
    /// vanilla `save()`'s own output for the real singleton (`.original()`, undetoured here), then parses
    /// those bytes independently and asserts every record matches the chain snapshot. Deliberately
    /// SAVE-only, not a full round-trip - `ZTThoughtMgr::load`'s `version >= 0x1e` pointer-resolution
    /// step can legitimately drop a record whose referenced object/thinker/habitat no longer resolves,
    /// which isn't a bug (see the plan's own caution).
    fn run_ztthoughtmgr_real_zoo_save_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTTHOUGHTMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE";
        let mut fail_flag = false;

        let mgr = globals().ztthoughtmgr();
        let expected: Vec<(u32, u32, u32, i32, i32)> = thought_live_support::read_raw_chain(mgr)
            .iter()
            .map(|t| (t.string_id(), t.thinker_id(), t.object_id(), t.tile_x(), t.tile_y()))
            .collect();
        if expected.is_empty() {
            info!("{}: no real thoughts active in the loaded zoo - nothing to round-trip, skipping", test_name);
            write_success_line(failure_log, &format!("{} (skipped: no real thoughts)", test_name));
            return false;
        }

        let real_ptr = globals().ztthoughtmgr_ptr() as *const u32;
        let dummy_file: u32 = 0;
        io_redirect::begin_capture();
        let save_ok = unsafe { gen_ztthoughtmgr::SAVE.original()(real_ptr, &dummy_file as *const u32) };
        let captured_bytes = io_redirect::end_capture();

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} thoughts={} bytes={}\n", test_name, expected.len(), captured_bytes.len()).as_bytes());
        }

        if !save_ok {
            error!("{}: real vanilla save() returned failure", test_name);
            fail_flag = true;
        }

        fn parse(bytes: &[u8]) -> Option<Vec<(u32, u32, u32, i32, i32)>> {
            if bytes.len() < 4 {
                return None;
            }
            let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
            let mut offset = 4;
            let mut records = Vec::with_capacity(count);
            for _ in 0..count {
                if offset + 20 > bytes.len() {
                    return None;
                }
                let read_u32 = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                let string_id = read_u32(offset);
                let thinker_id = read_u32(offset + 4);
                let object_id = read_u32(offset + 8);
                let tile_x = read_u32(offset + 12) as i32;
                let tile_y = read_u32(offset + 16) as i32;
                records.push((string_id, thinker_id, object_id, tile_x, tile_y));
                offset += 20;
            }
            Some(records)
        }

        match parse(&captured_bytes) {
            Some(parsed) if parsed == expected => {}
            other => {
                error!("{}: parsed real save bytes don't match the real chain snapshot", test_name);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: expected={:?}\nparsed={:?}\n", test_name, expected, other).as_bytes());
                }
                fail_flag = true;
            }
        }

        if !fail_flag {
            info!("{}: {} real thought(s) round-tripped byte-identically through save()", test_name, expected.len());
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTGAMEMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE: real-zoo round-trip coverage for
    /// `openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md`'s `ZTGameMgr` item. Snapshots
    /// `cash`/`date`/`elapsed_sim_ticks` directly off the real, live `globals().ztgamemgr()` singleton,
    /// captures its own `save()`'s bytes, replays them into `load()` **in place on that same singleton**
    /// (there's no cheap standalone copy of a fully-populated real `ZTGameMgr` to load into instead - see
    /// the plan), and asserts the three fields match afterward. Real `ZooStatus::save`/`load`
    /// (`.original()`, an opaque un-reimplemented vanilla sub-object at `self+0x10`) run as a side
    /// effect of both calls - presumed safe (persisted zoo-status counters only) but not independently
    /// verified, per the plan's own flag. Mutates the live singleton in place, so this is registered
    /// last in `live_zoo_tests` - nothing later in the battery depends on these three fields being
    /// untouched.
    fn run_ztgamemgr_real_zoo_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTGAMEMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE";
        const CURRENT_VERSION: u32 = 0x100;
        let mut fail_flag = false;

        let mgr_ptr = globals().ztgamemgr_ptr();
        if mgr_ptr.is_null() {
            info!("{}: GLOBAL_ZTGameMgr is null - nothing to round-trip, skipping", test_name);
            write_success_line(failure_log, &format!("{} (skipped: no live ZTGameMgr)", test_name));
            return false;
        }
        let mgr = unsafe { &mut *mgr_ptr };

        let cash_before = mgr.cash();
        let date_before = mgr.date_bytes();
        let ticks_before = mgr.elapsed_sim_ticks();

        let dummy_file: u32 = 0;
        io_redirect::begin_capture();
        let save_ok = mgr.save(&dummy_file as *const u32);
        let captured_bytes = io_redirect::end_capture();

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("CHECKPOINT {} cash_before={} ticks_before={} bytes={}\n", test_name, cash_before, ticks_before, captured_bytes.len()).as_bytes(),
            );
        }

        if !save_ok {
            error!("{}: real save() returned failure", test_name);
            fail_flag = true;
        }

        io_redirect::begin_replay(captured_bytes);
        let load_ok = mgr.load(&dummy_file as *const u32, CURRENT_VERSION);
        io_redirect::end_replay();

        if !load_ok {
            error!("{}: real load() returned failure replaying its own save bytes", test_name);
            fail_flag = true;
        }

        let cash_after = mgr.cash();
        let date_after = mgr.date_bytes();
        let ticks_after = mgr.elapsed_sim_ticks();

        if cash_after != cash_before || date_after != date_before || ticks_after != ticks_before {
            error!(
                "{}: real zoo state didn't round-trip byte-identically (cash {}->{}, ticks {}->{}, date {:?}->{:?})",
                test_name, cash_before, cash_after, ticks_before, ticks_after, date_before, date_after
            );
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!(
                        "Test Failed {}: cash_before={} cash_after={} ticks_before={} ticks_after={} date_before={:?} date_after={:?}\n",
                        test_name, cash_before, cash_after, ticks_before, ticks_after, date_before, date_after
                    )
                    .as_bytes(),
                );
            }
            fail_flag = true;
        }

        if !fail_flag {
            info!("{}: real ZTGameMgr cash/date/elapsed_sim_ticks round-tripped byte-identically", test_name);
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWMGR_REAL_ZOO_STORE_CONSISTENCY_LIVE: diagnosing a real save-corruption report. Real vanilla
    /// `ZTShowInfo::updateFromLoad` (`private/resources/decompiles/ZTShowInfo_updateFromLoad.c`) calls
    /// `ZTShowMgr::registerShow(mgr, this, false)` for every show as the zoo loads, then - if applying
    /// the loaded data changed `this`'s own id - `unregisterShow`s the *old* id. If
    /// `ZTShowMgr::register_show`/`unregister_show` (the Rust ports) ever mishandle that dance, the
    /// store would end up either missing a real show, or holding a stale entry (the same real
    /// `show_addr` reachable under two different ids) - both invisible to the player (the game keeps
    /// running normally) until the next save serializes whatever's now wrong into the file. Checks,
    /// against every show `run_load_live_zoo` actually loaded:
    /// 1. No two store entries share the same `show_addr` (a stale leftover from an old id).
    /// 2. Every store entry's key equals the real, live object's own `field_0x70` id - i.e. the store
    ///    and the real `ZTShowInfo` it points at still agree on that show's id.
    /// 3. The known show-tank habitat's real `ZTShowInfo*` ([`find_real_show_tank_habitat`]) is
    ///    registered in the store under its own real, live id.
    fn run_ztshowmgr_real_zoo_store_consistency_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWMGR_REAL_ZOO_STORE_CONSISTENCY_LIVE";
        let mut fail_flag = false;

        let entries = ztshowmgr::all_registered_shows();
        if entries.is_empty() {
            info!("{}: no real shows registered from the loaded zoo - nothing to check, skipping", test_name);
            write_success_line(failure_log, &format!("{} (skipped: no real shows)", test_name));
            return false;
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} entries={:?}\n", test_name, entries).as_bytes());
        }

        let mut seen_addrs = std::collections::HashSet::new();
        for &(id, addr) in &entries {
            if !seen_addrs.insert(addr) {
                error!("{}: show_addr {:#010x} is registered under more than one id (store={:?})", test_name, addr, entries);
                fail_flag = true;
            }
            let real_id = get_from_memory::<u16>(addr + 0x70);
            if real_id != id {
                error!(
                    "{}: store key {:#x} points at show {:#010x} whose own live field_0x70 says id {:#x} - store/object disagree",
                    test_name, id, addr, real_id
                );
                fail_flag = true;
            }
        }

        if let Some((_, show_info_ptr)) = find_real_show_tank_habitat() {
            let real_id = get_from_memory::<u16>(show_info_ptr + 0x70);
            match ztshowmgr::registered_show_for_id(real_id) {
                Some(addr) if addr == show_info_ptr => {}
                other => {
                    error!(
                        "{}: known show-tank's real ZTShowInfo {:#010x} (id {:#x}) not found under that id in the store (got {:?})",
                        test_name, show_info_ptr, real_id, other
                    );
                    fail_flag = true;
                }
            }
        }

        if !fail_flag {
            info!("{}: {} real show(s) all consistent between the store and their live objects", test_name, entries.len());
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// Scans for a real habitat that *does* satisfy `check_owning_habitat`'s blocking predicate (a real
    /// tank exhibit with zero water level) - lets `ZTSHOW_CHECK_OWNING_HABITAT_LIVE` exercise the blocking
    /// path against real `GLOBAL_ZTHabitatMgr`-owned memory too, not just the "should proceed" one.
    fn find_real_empty_tank_habitat() -> Option<u32> {
        let habitat_mgr = globals().zthabitatmgr();
        let exhibits = habitat_mgr.exhibit_array();
        for i in 0..exhibits.len() {
            let habitat_ptr = exhibits.get_ptr(i);
            if habitat_ptr == 0 {
                continue;
            }
            let habitat = get_from_memory::<ZTHabitat>(habitat_ptr);
            if habitat.is_tank() {
                // Only safe to read as a `ZTTankExhibit` because `is_tank()` above already confirmed
                // this pointer is really a 0x1e8-byte `ZTTankExhibit`, not a plain 0x178-byte `ZTHabitat`.
                let tank = get_from_memory::<crate::zthabitatmgr::ZTTankExhibit>(habitat_ptr);
                if *tank.water_level() == 0 {
                    return Some(habitat_ptr);
                }
            }
        }
        None
    }

    /// ZTSHOW_CHECK_OWNING_HABITAT_LIVE: `ztshow::check_owning_habitat` (factored out of `ZTShow::start`'s
    /// own inlined `checkOwningHabitat` logic specifically so it could be live-tested directly - see its
    /// own doc comment in `ztshow.rs`) is exercised here against real `GLOBAL_ZTHabitatMgr`-owned habitat
    /// memory (Route A from the implementation plan, preferred over hand-building a fake `ZTHabitat` with
    /// a copied vtable pointer) wrapped in a small, local, stack-allocated stand-in for a `ZTShowInfo*`
    /// (only `+0xa0`, the habitat back-pointer `check_owning_habitat` reads, needs to be populated -
    /// unlike `ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS` above, `check_owning_habitat` touches no other field,
    /// so there's no need for `ztshow_live_support::build_standalone_show_info`'s full `0xb0` buffer or
    /// its allocator-lifetime concerns here).
    ///
    /// `start`'s own full pipeline (which is what actually calls `check_owning_habitat` in real gameplay)
    /// isn't exercised end-to-end here: reaching the habitat check via the real `START` entry point needs
    /// `RESOLVE_NEXT_SCHEDULED_SCRIPT_ID` to already return a genuinely-scheduled real script id first,
    /// which depends on `ZTShow`'s own scheduling-vector data - a structure this plan never reverse
    /// -engineered (out of scope for this session, flagged as a residual gap in the plan doc). Testing
    /// `check_owning_habitat` directly sidesteps that gap entirely while still exercising the exact logic
    /// `start` relies on, against real habitat memory.
    fn run_ztshow_check_owning_habitat_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOW_CHECK_OWNING_HABITAT_LIVE";

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} entry\n", test_name).as_bytes());
        }

        let Some((qualifying_habitat_ptr, real_show_info)) = find_real_show_tank_habitat() else {
            info!("Skipping {}: no real tank habitat with water_level()>0 and a real ZTShowInfo* attached found in test zoo", test_name);
            write_success_line(failure_log, &format!("{} (skipped: no qualifying real show-tank habitat found)", test_name));
            return false;
        };

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} found qualifying_habitat={:#010x} real_show_info={:#010x}\n", test_name, qualifying_habitat_ptr, real_show_info).as_bytes());
        }

        let mut fail_flag = false;

        // Positive case: a real, working show tank (has water) must NOT be blocked - check_owning_habitat
        // mirrors vanilla's checkOwningHabitat returning its blocking code only for an *empty* tank, so a
        // filled one must return false here (see ztshow.rs's check_owning_habitat doc comment).
        let mut positive_buf = [0u8; 0xa4];
        let positive_show_info = positive_buf.as_mut_ptr() as u32;
        save_to_memory(positive_show_info + 0xa0, qualifying_habitat_ptr);
        if ztshow::check_owning_habitat(positive_show_info) {
            error!("{}: check_owning_habitat returned true (blocked) for a real working tank habitat ({:#010x}, water_level>0)", test_name, qualifying_habitat_ptr);
            fail_flag = true;
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} positive case done\n", test_name).as_bytes());
        }

        // Negative case: a null habitat pointer means there's no tank gating this show at all, so it must
        // not be blocked either.
        let null_buf = [0u8; 0xa4];
        let null_show_info = null_buf.as_ptr() as u32;
        if ztshow::check_owning_habitat(null_show_info) {
            error!("{}: check_owning_habitat returned true (blocked) for a null habitat pointer", test_name);
            fail_flag = true;
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} negative case done\n", test_name).as_bytes());
        }

        // Blocking case: a real tank exhibit with zero water level must be blocked, if the test zoo has
        // one.
        if let Some(empty_tank_habitat_ptr) = find_real_empty_tank_habitat() {
            let mut blocking_buf = [0u8; 0xa4];
            let blocking_show_info = blocking_buf.as_mut_ptr() as u32;
            save_to_memory(blocking_show_info + 0xa0, empty_tank_habitat_ptr);
            if !ztshow::check_owning_habitat(blocking_show_info) {
                error!("{}: check_owning_habitat returned false (not blocked) for a real empty tank habitat ({:#010x})", test_name, empty_tank_habitat_ptr);
                fail_flag = true;
            }
        } else {
            info!("{}: no real empty tank habitat found in test zoo, skipping that half of the check", test_name);
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} blocking case done\n", test_name).as_bytes());
        }

        // Best-effort smoke test of the full START entry point (which is what actually calls
        // check_owning_habitat in real gameplay) against the real show already attached to the qualifying
        // habitat - calling through START's own real, now-hooked address (0x005a3db4, ztshow::START).
        // Not asserted beyond "doesn't crash": whether it proceeds past the habitat check depends on real,
        // un-inspected scheduling data this session didn't reverse-engineer (see this function's own doc
        // comment), so an early return here is just as valid an outcome as a full run.
        if real_show_info != 0 {
            let real_show = real_show_info + 4;
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("CHECKPOINT {} about to call START on real_show={:#010x}\n", test_name, real_show).as_bytes());
            }
            let start_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32)>(0x005a3db4u32) };
            start_hooked(real_show as *const u32);
            info!("{}: START smoke-test against real show {:#010x} completed without crashing", test_name, real_show);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("CHECKPOINT {} START returned\n", test_name).as_bytes());
            }
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// Scans the live, loaded test zoo's real world entities (`globals().ztworldmgr()`'s entity array) for
    /// one whose type passes `ztshow::RVA_SHOW_TRICK_TYPE_CHECK` (the same `entity_type_matches` gate
    /// `do_current_item`/`validate_item` both apply) - i.e. a genuinely trick-eligible animal. Returns
    /// `(entity_ptr, entity_id)` for the first match, `None` if the test zoo has none.
    fn find_real_trick_eligible_unit() -> Option<(u32, u32)> {
        let world = globals().ztworldmgr();
        let start = world.entity_array_start();
        let end = world.entity_array_end();
        let mut i = start;
        while i < end {
            let entity_ptr = get_from_memory::<u32>(i);
            i += 0x4;
            if entity_ptr == 0 {
                continue;
            }
            if unsafe { crate::ztmegatilemgr::entity_type_matches(entity_ptr, ztshow::RVA_SHOW_TRICK_TYPE_CHECK) } {
                let id = get_from_memory::<u32>(entity_ptr + 0x124);
                return Some((entity_ptr, id));
            }
        }
        None
    }

    /// ZTSHOW_GROUP3_TRICK_LIVE: `ZTShow::doCurrentItem`/`validateItem`/`doTrickEvent` are, like
    /// `ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS` above, full-replacement detours with no real-vs-reimplementation
    /// diff oracle - this calls through their own real, now-hooked addresses directly against real,
    /// `run_load_live_zoo`-populated `GLOBAL_ZTWorldMgr`/habitat data (they all internally call
    /// `GET_UNIT.original()`, which needs a real, resolvable unit), asserting no crash plus a few structural
    /// invariants.
    ///
    /// Needs: (1) a real, already-configured show-tank habitat (`find_real_show_tank_habitat`, same
    /// discovery `ZTSHOW_CHECK_OWNING_HABITAT_LIVE` uses) to get a real `ZTShow*`/`ZTShowInfo*` pair, and
    /// (2) a real, trick-eligible animal somewhere in the test zoo (`find_real_trick_eligible_unit`) to
    /// resolve via `GET_UNIT`. If either is missing, this is a genuine coverage gap, reported clearly
    /// rather than skipped silently - see the `else` branch below.
    fn run_ztshow_group3_trick_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOW_GROUP3_TRICK_LIVE";

        let Some((_habitat_ptr, real_show_info)) = find_real_show_tank_habitat() else {
            error!("{}: BLOCKED - no real, already-configured show-tank habitat (ZTHabitat::is_show_tank) found in test zoo; do_current_item/validate_item/do_trick_event have no live coverage", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no real show-tank habitat found in test zoo\n", test_name).as_bytes());
            }
            return false;
        };

        let Some((_unit_ptr, unit_id)) = find_real_trick_eligible_unit() else {
            error!(
                "{}: BLOCKED - test zoo has no animal whose type passes RVA_SHOW_TRICK_TYPE_CHECK; would need a new zoo asset (a trick-eligible animal) to cover do_current_item/validate_item/do_trick_event's real unit-resolution path",
                test_name
            );
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - test zoo has no trick-eligible animal\n", test_name).as_bytes());
            }
            return false;
        };

        let real_show = real_show_info + 4;
        let mut fail_flag = false;

        // Create a real ZTShowScriptState for our chosen unit (real, un-hooked CREATE_SHOW_SCRIPT_STATE -
        // safe against this real, properly-constructed ZTShow's own `+0x34` state map), then fetch it back
        // the same way `do_current_item`'s own body does. Two-arg call only (this, unit_id) - see
        // `ztshow.rs`'s `start()` doc comment / `generated.rs`'s `CONSTRUCTOR` entry for why the old
        // three-arg signature (a bogus `show_id: u16`) was a real stack-imbalance bug.
        let create_result = unsafe { CREATE_SHOW_SCRIPT_STATE.original()(real_show as *const u32, unit_id) };
        if create_result != 0 {
            info!("{}: CREATE_SHOW_SCRIPT_STATE returned {} (nonzero/failure) for unit {:#x}; do_current_item/do_trick_event will still be exercised via their early-return paths", test_name, create_result, unit_id);
        }
        let state_ptr = unsafe { GET_SHOW_SCRIPT_STATE.original()(real_show as *const u32, unit_id) };

        // DO_CURRENT_ITEM (0x005a2508, ztshow::DO_CURRENT_ITEM): safe for any unit_id regardless of
        // whether a state/eligible unit resolved - it handles state==0/unit_ptr==0/ineligible-type
        // internally, returning 5/-1 respectively rather than crashing.
        let do_current_item_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, u32) -> i32>(0x005a2508u32) };
        let current_item_result = do_current_item_hooked(real_show as *const u32, unit_id);
        info!("{}: do_current_item({:#x}) = {} (no crash)", test_name, unit_id, current_item_result);

        // VALIDATE_ITEM (0x005a6d70, ztshow::VALIDATE_ITEM): only safe to call once the real show's own
        // configured unit_type_id (`real_show+0x8`) genuinely has at least one real unit assigned
        // (`GET_SHOW_UNIT_LIST`'s own documented lack of an empty-list check - see `validate_item`'s doc
        // comment in `ztshow.rs`) - checked via GET_NUM_UNITS first rather than risking that dereference
        // speculatively.
        let show_unit_type_id = get_from_memory::<u32>(real_show + 0x8);
        let assigned_unit_count = unsafe { ZTSHOWINFO_GET_NUM_UNITS.original()(real_show_info as *const u32, show_unit_type_id) };
        if assigned_unit_count >= 1 {
            let validate_item_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, u16) -> u32>(0x005a6d70u32) };
            let validate_result = validate_item_hooked(real_show as *const u32, 0);
            info!("{}: validate_item(0) = {} (real show unit_type_id={:#x}, {} unit(s) assigned)", test_name, validate_result, show_unit_type_id, assigned_unit_count);
        } else {
            info!("{}: skipping validate_item - real show's own unit_type_id {:#x} has {} assigned units (validate_item's own empty-list dereference isn't guarded, see its doc comment)", test_name, show_unit_type_id, assigned_unit_count);
        }

        // DO_TRICK_EVENT (0x005a6894, ztshow::DO_TRICK_EVENT): needs a real, non-null ZTShowScriptState* -
        // only call it if GET_SHOW_SCRIPT_STATE actually resolved one above. Registers a fresh synthetic
        // script/item per case, points the real ZTShow at it (snapshotting/restoring `real_show+0x4` and
        // `state_ptr+0xc`/`+0xf` around each call so this doesn't permanently disturb the real, live
        // objects other tests later in this chain still use), and asserts the `+0x28`/`+0x2c`/`+0x30`
        // accumulator deltas match `do_trick_event`'s own accounting - see this function's own inline
        // comments for why the three threshold branches (low/mid/high relative to `ZTShowMgr`'s real
        // `threshold_a`/`threshold_b`/`threshold_c`) aren't distinguishable via those three fields alone
        // (the threshold dispatch only changes which `SEND_EVENT`/`DO_KEEPER_EVENT` calls fire, not the
        // accumulator writes, which all happen unconditionally before the dispatch) - exercising each is
        // still valuable as real-call-path coverage/crash safety, just not as a three-way accumulator diff.
        if state_ptr != 0 {
            let do_trick_event_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, *const u32)>(0x005a6894u32) };

            let original_script_id = get_from_memory::<u16>(real_show + 0x4);
            let original_trick_index = get_from_memory::<u16>(state_ptr + 0xc);
            let original_skip_scoring = get_from_memory::<u8>(state_ptr + 0xf);

            let mgr_ptr = globals().ztshowmgr_ptr();
            let mirror_cases: Vec<(&str, u32)> = if mgr_ptr.is_null() {
                info!(
                    "{}: GLOBAL_ZTShowMgr not initialized at this injection point - threshold-branch coverage skipped, only the skip_scoring/item_type==3 cases below run (same class of gap as ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE's own null check)",
                    test_name
                );
                Vec::new()
            } else {
                let mgr = unsafe { &*mgr_ptr };
                vec![
                    ("low (<=threshold_a)", mgr.threshold_a),
                    ("mid (between threshold_b and threshold_c)", mgr.threshold_b.wrapping_add(mgr.threshold_c) / 2),
                    ("high (>=threshold_c)", mgr.threshold_c),
                ]
            };

            // One case per real threshold branch, plus skip_scoring and the item_type==3 short-circuit.
            struct Case {
                label: String,
                item_type: u32,
                satisfaction: u32,
                satisfaction_mirror: u32,
                skip_scoring: bool,
            }
            let mut cases: Vec<Case> = mirror_cases
                .into_iter()
                .map(|(label, mirror)| Case { label: label.to_string(), item_type: 1, satisfaction: 7, satisfaction_mirror: mirror, skip_scoring: false })
                .collect();
            cases.push(Case { label: "skip_scoring".to_string(), item_type: 1, satisfaction: 7, satisfaction_mirror: 7, skip_scoring: true });
            cases.push(Case {
                label: "item_type==3 short-circuit".to_string(),
                item_type: 3,
                satisfaction: 7,
                satisfaction_mirror: 7,
                skip_scoring: false,
            });

            for (case_index, case) in cases.iter().enumerate() {
                // `add_item` only inserts when the item's own `item_type` matches the script's - register
                // each script with `case.item_type` itself as its type (rather than a fixed `SCRIPT_TYPE`)
                // so every case's item actually gets inserted, including the `item_type==3` short-circuit
                // case, which needs a genuine hit against `item_snapshot_by_id` to reach `do_trick_event`'s
                // own `item.item_type == 3` check at all.
                let script_id = ztshowscriptmgr::register_script(0x8000_0000 | case_index as u32, case.item_type)
                    .expect("register_script should never reject a non-null ctor_ptr");
                let item = ztshowscriptmgr::live_support::raw_item_with_mirror(case.item_type, 1, case.satisfaction, case.satisfaction_mirror);
                ztshowscriptmgr::add_item(0x8000_0000 | case_index as u32, &item);

                save_to_memory(real_show + 0x4, script_id);
                save_to_memory(state_ptr + 0xc, 0u16); // trick_index 0, our only item
                save_to_memory(state_ptr + 0xf, case.skip_scoring as u8);

                let count_before = get_from_memory::<i32>(real_show + 0x28);
                let sum_before = get_from_memory::<i32>(real_show + 0x2c);
                let mirror_sum_before = get_from_memory::<i32>(real_show + 0x30);

                do_trick_event_hooked(real_show as *const u32, state_ptr as *const u32);

                let count_after = get_from_memory::<i32>(real_show + 0x28);
                let sum_after = get_from_memory::<i32>(real_show + 0x2c);
                let mirror_sum_after = get_from_memory::<i32>(real_show + 0x30);

                // Restore before asserting, so a failure doesn't also leave the real objects corrupted for
                // later tests.
                save_to_memory(real_show + 0x4, original_script_id);
                save_to_memory(state_ptr + 0xc, original_trick_index);
                save_to_memory(state_ptr + 0xf, original_skip_scoring);

                if case.item_type == 3 {
                    if sum_after != sum_before || count_after != count_before || mirror_sum_after != mirror_sum_before {
                        error!("{}: case '{}' (item_type==3) should leave all three accumulators unchanged, got count {}->{}, sum {}->{}, mirror_sum {}->{}",
                            test_name, case.label, count_before, count_after, sum_before, sum_after, mirror_sum_before, mirror_sum_after);
                        fail_flag = true;
                    }
                    continue;
                }
                if sum_after != sum_before.wrapping_add(case.satisfaction as i32) {
                    error!("{}: case '{}' expected sum {} -> {}, got {}", test_name, case.label, sum_before, sum_before.wrapping_add(case.satisfaction as i32), sum_after);
                    fail_flag = true;
                }
                if case.skip_scoring {
                    if count_after != count_before || mirror_sum_after != mirror_sum_before {
                        error!("{}: case '{}' (skip_scoring) should leave count/mirror_sum unchanged, got count {}->{}, mirror_sum {}->{}",
                            test_name, case.label, count_before, count_after, mirror_sum_before, mirror_sum_after);
                        fail_flag = true;
                    }
                } else {
                    // count/mirror_sum are written unconditionally before the threshold dispatch (which
                    // itself only changes which SEND_EVENT/DO_KEEPER_EVENT calls fire, not these two
                    // fields), so the same expectation holds whether or not GLOBAL_ZTShowMgr is live.
                    let expected_mirror_sum = mirror_sum_before.wrapping_add(case.satisfaction_mirror as i32);
                    if count_after != count_before + 1 || mirror_sum_after != expected_mirror_sum {
                        error!("{}: case '{}' expected count {} -> {}, mirror_sum {} -> {}, got count={}, mirror_sum={}",
                            test_name, case.label, count_before, count_before + 1, mirror_sum_before, expected_mirror_sum, count_after, mirror_sum_after);
                        fail_flag = true;
                    }
                }
                info!("{}: case '{}' completed without crashing (mirror={})", test_name, case.label, case.satisfaction_mirror);
            }
        } else {
            info!("{}: skipping do_trick_event - no real ZTShowScriptState resolved for unit {:#x}", test_name, unit_id);
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWUI_FILL_TRICK_LISTS_LIVE: `ztshowui::fill_trick_lists`/`copy_list_to_script` (Stage 4 UI
    /// consumers, `ztshowscriptmgr-open-items.md`'s open item 1) had no live trigger test before this -
    /// only verified via a clean DLL load (detours install without error) plus the existing suite's
    /// continued pass. Drives both against a real, already-configured show-tank habitat
    /// ([`find_real_show_tank_habitat`], the same discovery `ZTSHOW_CHECK_OWNING_HABITAT_LIVE`/
    /// `GROUP3_TRICK_LIVE` use) and a real `ZTUnitType*` for the show's own **committed** species,
    /// resolved from its real pending-scripts tree (`ZTShowInfo+0x44`, via
    /// `ztshow::live_support::collect_pending_script_nodes` - confirmed against
    /// `ZTShowInfo::getShowSpeciesList`'s own decompile, which builds its species list the same way).
    /// See this function's own inline comment for two real bugs this resolution went through before
    /// landing here: an earlier version read the unrelated `real_show+0x8` field and called
    /// `BFWORLDMGR_GET_TYPE` on it unconditionally, crashing on that field's unpopulated `0` value; the
    /// version after that abandoned it entirely for [`find_real_trick_eligible_unit`]'s broad
    /// `RVA_SHOW_TRICK_TYPE_CHECK` scan (kept now only as the fallback when the show genuinely has no
    /// pending-script node yet), which reliably found a different, untricked animal instead. Writes the
    /// show-editor's own selection globals directly (`ztshowui::live_support::set_selection`) rather
    /// than driving the real UI click path, then
    /// calling `FILL_TRICK_LISTS`/`COPY_LIST_TO_SCRIPT` through their own real, now-hooked addresses (needs
    /// `crate::ztshowui::init()` wired into this harness's own `init()` - see that call site's comment on
    /// why this matters: a detour never installed here would make an "hooked address" call silently
    /// exercise real vanilla code instead, per the finding documented at this file's `run_load_live_zoo`
    /// call site).
    ///
    /// **Coverage note**: whether `BFUIMgr::getElement` resolves the "available"/"assigned" trick listbox
    /// element ids (`ztshowui::live_support::ui_elements_present()`, logged below) depends on whether the
    /// real show-editor panel (`ZTUI::showpanel::init`/`show`) happens to have been constructed already in
    /// this test run - when it has, both functions' listbox-population halves run genuinely end-to-end
    /// against real UI widgets; when it hasn't, they take their early-return branch instead. Either way this
    /// test gives real coverage of the field-offset-sensitive trick-list walk
    /// (`ztshowui::live_support::trick_list_len`, asserted non-empty against the real unit type -
    /// [`crate::ztshowui::find_trick_by_id`]'s own doc comment has the dummy-head double-indirection bug
    /// this caught and fixed), `find_or_insert_pending_script_node` reuse, and a "no crash calling through
    /// the real hooked entry points against real habitat/unit-type memory" baseline, matching every other
    /// test in this group.
    fn run_ztshowui_fill_trick_lists_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWUI_FILL_TRICK_LISTS_LIVE";

        let Some((habitat_ptr, show_info_ptr)) = find_real_show_tank_habitat() else {
            error!("{}: BLOCKED - no real, already-configured show-tank habitat found in test zoo; fill_trick_lists/copy_list_to_script have no live coverage", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no qualifying show-tank habitat found\n", test_name).as_bytes());
            }
            return false;
        };

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} habitat found: habitat={:#010x} show_info={:#010x}\n", test_name, habitat_ptr, show_info_ptr).as_bytes());
        }

        // Prefer the show's own committed species, read from its real pending-scripts tree
        // (`ZTShowInfo+0x44`, already modeled by `ztshow::live_support::collect_pending_script_nodes` -
        // the exact same walk `ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE` already validates
        // live against this exact show). Each node's `+0x10` is a real unit_type_id with an actual
        // assigned/pending script on this show (`ztshow::allocate_pending_script_node`'s own write) -
        // confirmed against `ZTShowInfo::getShowSpeciesList`'s own decompile, which builds its species
        // list by walking this exact tree, never touching `real_show+0x8`. Two real bugs this function
        // had before landing here: an earlier version read `real_show+0x8` directly and called
        // `BFWORLDMGR_GET_TYPE` on it unconditionally, which returned a non-null but bogus pointer for
        // that field's `0`/unpopulated value and crashed the whole battery walking its garbage `+0x1ac`
        // list; the version after that abandoned `real_show+0x8` for
        // [`find_real_trick_eligible_unit`]'s broad `RVA_SHOW_TRICK_TYPE_CHECK` scan (kept now only as
        // the fallback below), which reliably found a different, untricked animal instead (`real_show+0x8`
        // was never the right field to begin with - it's simply not the species list). Skip `0`/`0x2550`
        // keys, matching `getShowSpeciesList`'s own exact filter (`local_34 != 0 && local_34 != 0x2550`) -
        // `0x2550` is a real, non-species sentinel key this exact tree can hold (confirmed live: this
        // show's tree has 3 real nodes per `ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE`, but a
        // naive `.first()` of the ascending-key in-order walk landed on a sentinel-keyed node instead of
        // a real species, reproducing `committed_unit_type_id=0x0` every run despite real species nodes
        // being present).
        let pending_nodes = ztshow_live_support::collect_pending_script_nodes(show_info_ptr);
        let committed_unit_type_id = pending_nodes
            .iter()
            .map(|&node| get_from_memory::<u32>(node + 0x10))
            .find(|&id| id != 0 && id != 0x2550)
            .unwrap_or(0);
        let (unit_type_ptr, source_unit_id) = if committed_unit_type_id != 0 {
            let world = globals().ztworldmgr_ptr() as *const u32;
            let ptr = unsafe { BFWORLDMGR_GET_TYPE.original()(world, committed_unit_type_id as i32) } as u32;
            (ptr, committed_unit_type_id)
        } else if let Some((trick_unit_ptr, trick_unit_id)) = find_real_trick_eligible_unit() {
            (get_from_memory::<u32>(trick_unit_ptr + 0x128), trick_unit_id)
        } else {
            error!("{}: BLOCKED - show has no committed species and test zoo has no animal whose type passes RVA_SHOW_TRICK_TYPE_CHECK; fill_trick_lists/copy_list_to_script have no live coverage", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no committed species and no trick-eligible animal\n", test_name).as_bytes());
            }
            return false;
        };
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "CHECKPOINT {} got unit_type_ptr={:#010x} (committed_unit_type_id={:#x}, source_id={:#x})\n",
                    test_name, unit_type_ptr, committed_unit_type_id, source_unit_id
                )
                .as_bytes(),
            );
        }
        if unit_type_ptr == 0 {
            error!("{}: BLOCKED - resolved unit type pointer is null (committed_unit_type_id={:#x}, source_id={:#x})", test_name, committed_unit_type_id, source_unit_id);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - resolved unit type pointer is null\n", test_name).as_bytes());
            }
            return false;
        }

        let mut fail_flag = false;
        let trick_count = ztshowui::live_support::trick_list_len(unit_type_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} trick_list_len={}\n", test_name, trick_count).as_bytes());
        }
        if trick_count == 0 {
            error!("{}: real unit type {:#010x}'s own +0x1ac trick list is empty; walk_trick_list/find_trick_by_id get no real coverage from this run", test_name, unit_type_ptr);
            fail_flag = true;
        } else {
            info!("{}: real unit type {:#010x} has {} real trick(s) in its +0x1ac list", test_name, unit_type_ptr, trick_count);
        }

        ztshowui::live_support::set_selection(habitat_ptr, unit_type_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} set_selection done\n", test_name).as_bytes());
        }

        let ui_present = ztshowui::live_support::ui_elements_present();
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} ui_elements_present={}\n", test_name, ui_present).as_bytes());
        }
        let ztshowmgr_ptr_is_null = globals().ztshowmgr_ptr().is_null();
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "CHECKPOINT {} pre-fill: habitat={:#010x} unit_type={:#010x} show_info={:#010x} ui_present={} ztshowmgr_ptr_null={}\n",
                    test_name, habitat_ptr, unit_type_ptr, show_info_ptr, ui_present, ztshowmgr_ptr_is_null
                )
                .as_bytes(),
            );
        }

        // FILL_TRICK_LISTS (0x004751dc, ztui_showpanel::FILL_TRICK_LISTS).
        let fill_trick_lists_hooked = unsafe { std::mem::transmute::<u32, extern "stdcall" fn()>(0x004751dcu32) };
        fill_trick_lists_hooked();
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} post-fill\n", test_name).as_bytes());
        }
        info!("{}: FILL_TRICK_LISTS completed without crashing", test_name);

        // Confirm fill_trick_lists kept the real DAT_0063ba58 vector real vanilla addTrick reads from
        // (ztshowui::AVAILABLE_TRICK_IDS_BEGIN_RVA's own doc comment) in sync with the "available tricks"
        // listbox - only meaningful when the listbox actually exists (see ui_present above; both functions
        // early-return before touching it otherwise, per this file's own doc comment).
        if ui_present {
            let expected = ztshowui::live_support::non_sentinel_trick_count(unit_type_ptr);
            let actual = ztshowui::live_support::available_trick_id_vector();
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("CHECKPOINT {} available_trick_ids expected_len={} actual={:?}\n", test_name, expected, actual).as_bytes(),
                );
            }
            if actual.len() != expected {
                error!(
                    "{}: real DAT_0063ba58 vector has {} entries, expected {} (non-sentinel tricks) - addTrick would read stale/out-of-bounds data on an 'Add' click",
                    test_name, actual.len(), expected
                );
                fail_flag = true;
            } else {
                info!("{}: real DAT_0063ba58 vector has the expected {} entries after FILL_TRICK_LISTS", test_name, expected);
            }
        }

        // COPY_LIST_TO_SCRIPT (0x00475d92, standalone::COPY_LIST_TO_SCRIPT) - already fully ported
        // (`ztshowui::copy_list_to_script`, Stage 4). Previously crashed here whenever the committed
        // species already had a registered script: the port's own call to `recalc_show_stats(script_handle)`
        // passed a `ztshowscriptmgr::synthetic_script_handle` (never a real pointer) into real vanilla
        // `recalcShowStats`, which raw-dereferences it directly - live-confirmed via `crash-capture`
        // symbolizing the fault as `MOV EDX,[ECX+0x10]` with `ECX=0x73000003`
        // (`SYNTHETIC_SCRIPT_HANDLE_BASE (0x73000000) | script_id 3`). Fixed at the source
        // (`copy_list_to_script` now always passes `0` to `recalc_show_stats` - see that call site's own
        // comment for the known, temporary, cosmetic-only inaccuracy this trades in) - this call is
        // expected to complete safely now regardless of whether the committed species already has a
        // registered script.
        let copy_list_to_script_hooked = unsafe { std::mem::transmute::<u32, extern "stdcall" fn() -> u32>(0x00475d92u32) };
        let copy_result = copy_list_to_script_hooked();
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} post-copy result={}\n", test_name, copy_result).as_bytes());
        }
        info!("{}: COPY_LIST_TO_SCRIPT completed without crashing, returned {}", test_name, copy_result);

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE: resolves the open question `make_registered_show_script`'s
    /// own doc comment flags - whether the real `ZTShowScript::ZTShowScript` ctor's `auto_register=true`
    /// path (`ztshowscript::CONSTRUCTOR`, `0x0059f837`, intentionally left un-detoured per
    /// `ztshowscriptmgr.rs`'s module doc comment) actually reaches Stage 1's `REGISTER_SCRIPT` detour
    /// (`0x0046e774`) and registers into the store, or whether the earlier "doesn't register" finding was
    /// a harness-timing artifact of `GLOBAL_ZTShowMgr` not yet being resolved this early. Root cause per
    /// `private/resources/decompiles/ZTShowScript_ZTShowScript.c:25`: the real ctor only calls
    /// `ZTShowMgr::registerScript` when `GLOBAL_ZTShowMgr != 0` (`globals().ztshowmgr_ptr()`) - the same
    /// class of "global not yet resolved at this early test-injection point" issue this file already
    /// documents for `GLOBAL_ZTGameMgr` (see `run_ztscenariosimplegoal_eval_award_count_test`).
    ///
    /// Skips gracefully (not a failure) if `GLOBAL_ZTShowMgr` is still null here, matching that same
    /// convention. Otherwise allocates a real `0x14`-byte object (matching `ztshowui::
    /// copy_list_to_script`'s own identical allocation) and calls the real, un-detoured ctor directly via
    /// `.original()` with `auto_register=true`, then asserts the id it writes back at `ctor_ptr+0x4` is
    /// genuinely present in Stage 1's store.
    fn run_ztshowscript_ctor_registration_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE";

        if globals().ztshowmgr_ptr().is_null() {
            info!("Skipping {}: GLOBAL_ZTShowMgr not initialized at this injection point", test_name);
            write_success_line(failure_log, &format!("{} (skipped: ZTShowMgr not initialized)", test_name));
            return false;
        }

        const SCRIPT_TYPE: u32 = 0x7ace;
        let alloc = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
        let ctor_ptr = unsafe { ZTSHOWSCRIPT_CONSTRUCTOR.original()(alloc as *const u32, SCRIPT_TYPE, true) } as u32;

        let mut fail_flag = false;
        if ctor_ptr == 0 {
            error!("{}: CONSTRUCTOR returned null", test_name);
            fail_flag = true;
        } else {
            let assigned_id = get_from_memory::<u16>(ctor_ptr + 0x4);
            if !ztshowscriptmgr::script_exists_by_id(assigned_id) {
                error!(
                    "{}: ctor's auto_register=true path did NOT register id {} (ctor_ptr={:#010x}) into Stage 1's store - GLOBAL_ZTShowMgr was live, so this is a genuine reimplementation gap",
                    test_name, assigned_id, ctor_ptr
                );
                fail_flag = true;
            } else {
                info!("{}: ctor's auto_register=true path correctly registered id {} into Stage 1's store", test_name, assigned_id);
            }
        }

        if !fail_flag {
            write_success_line(failure_log, test_name);
        } else if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
        }
        fail_flag
    }

    /// ZTTHOUGHTMGR_LOAD_MODERN: compares the real `ZTThoughtMgr::load`'s effect against the
    /// reimplemented `load` for the `version >= 0x1e` branch - the branch `ZTTHOUGHTMGR_LOAD` above
    /// never exercises, since it drives inline `ZTWorldMgr::resolve_entity_by_id`/
    /// `ZTHabitatMgr::get_habitat_ptr` resolution that needs both globals initialized. Runs from
    /// `run_on_completion_reset_test_and_exit`'s later chain, like
    /// `ZTTHOUGHTMGR_POPULATE_THOUGHTS`/`ZTTHOUGHT_GET_STRING` below.
    ///
    /// `thinker_id`/`object_id` stay in the same small `0..5` range `ZTTHOUGHTMGR_POPULATE_THOUGHTS`
    /// already uses (real entity ids are never this low in a fresh test process, so
    /// `resolve_entity_by_id` deterministically returns null on both sides). Every record's tile is
    /// fixed at the `(-1, -1)` "no tile" sentinel, not generated: `ZTHabitatMgr::get_habitat_ptr`
    /// performs no bounds-checking against its own `other_array_start`/`other_array_end` fields, and at
    /// this early injection point (before any zoo is loaded) that array is too small/empty for even
    /// single-digit tile coordinates to stay in-bounds, so tile-based exercise of that path is deferred
    /// rather than attempted here.
    ///
    /// `truncate_at`, when `Some`, cuts the serialized byte stream short - folds in short-read coverage
    /// `ZTTHOUGHTMGR_LOAD` doesn't have. `io_redirect::deallocate` just returns failure once the replay
    /// buffer runs out, never touching out-of-bounds memory.
    fn run_thoughtmgr_load_modern_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let test_name = "ZTTHOUGHTMGR_LOAD_MODERN";

        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let mut fail_flag = false;

        let record_strategy = (any::<u32>(), 0u32..5, 0u32..5);
        let strategy = prop::collection::vec(record_strategy, 0..6).prop_flat_map(|records| {
            let total_len = 4 + records.len() * 20;
            (Just(records), 0x1eu32..0x40, prop::option::of(0usize..total_len))
        });

        match runner.run(&strategy, |(records, version, truncate_at)| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
            for &(string_id, thinker_id, object_id) in &records {
                bytes.extend_from_slice(&string_id.to_le_bytes());
                bytes.extend_from_slice(&thinker_id.to_le_bytes());
                bytes.extend_from_slice(&object_id.to_le_bytes());
                bytes.extend_from_slice(&(-1i32 as u32).to_le_bytes());
                bytes.extend_from_slice(&(-1i32 as u32).to_le_bytes());
            }
            if let Some(cut) = truncate_at {
                bytes.truncate(cut);
            }

            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
            let file_buffer = [0u32; 4];

            io_redirect::begin_replay(bytes.clone());
            let real_ret = unsafe { gen_ztthoughtmgr::LOAD.original()(real_ptr as *const u32, file_buffer.as_ptr(), version) };
            io_redirect::end_replay();

            io_redirect::begin_replay(bytes);
            let reimpl_ret = unsafe { &mut *reimpl_ptr }.load(file_buffer.as_ptr(), version);
            io_redirect::end_replay();

            let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

            thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(
                real_ret,
                reimpl_ret,
                "load() return mismatch for records={:?}, version={}, truncate_at={:?}",
                records,
                version,
                truncate_at
            );
            prop_assert_eq!(
                real_fields,
                reimpl_fields,
                "loaded content mismatch for records={:?}, version={}, truncate_at={:?}",
                records,
                version,
                truncate_at
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

    /// ZTTHOUGHTMGR_ADD_THOUGHT_ANIMAL_OVERRIDE: exercises the animal-subtype override branch inside
    /// `add_thought` (`resolve_object_own_habitat_ptr`'s two vtable calls) - `ZTAnimalType::isCastClass`
    /// at `0x004020cd` and `ZTAnimal::getHabitat` at `0x00410685` (via `calcHabitat` ->
    /// `ZTUnit::getHabitat`).
    ///
    /// Both the "real" (`ADD_THOUGHT.original()`) and "reimplemented" (`add_thought`) sides dispatch
    /// through the exact same real vanilla function pointers here - `resolve_object_own_habitat_ptr` is a
    /// call-through wrapper around vanilla's own vtable slots, not reimplemented logic. So this test
    /// isn't validating a separate vanilla habitat-resolution algorithm; it's validating that our
    /// sequencing (vtable offsets, `this`/argument marshalling, override-vs-fallback logic) matches
    /// vanilla's own `addThought.asm` exactly.
    ///
    /// The fixture: a `ZTAnimal` (zeroed via `ZTAnimal::new_for_test`; entity type built via
    /// `ZTAnimalType::new_for_test` so its vtable is the real `0x00630268` - slot `0x1c` resolves to
    /// `isCastClass`) with its own vtable field overwritten to the real `ZTAnimal` vtable `0x0062ff54`
    /// via a raw memory write, so slot `0x24c` resolves to `getHabitat`. `isCastClass` always returns
    /// true when called on a genuine `ZTAnimalType`-vtabled object, so this fixture reliably exercises
    /// the override-taken path, not the fallback.
    ///
    /// `getHabitat`'s own chain reads `BFEntity::getTile()` (a real vanilla, non-virtual call) and a
    /// show-info flag at a `BFUnit` offset; the fixture's zeroed base leaves that flag `0`, skipping the
    /// `ZTShowMgr` branch, and `pos = (0, 0, 0)` is expected to miss real vanilla's own tile lookup at
    /// this injection point (no zoo loaded yet).
    fn run_thoughtmgr_add_thought_animal_override_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_ADD_THOUGHT_ANIMAL_OVERRIDE";
        let mut fail_flag = false;

        match runner.run(&(any::<u32>(), 0u32..5), |(string_id, fallback_habitat_ptr)| {
            let entity_type = ZTAnimalType::new_for_test(IVec3::default(), 0, IVec3::default(), IVec3::default());
            let animal = ZTAnimal::new_for_test(&entity_type as *const ZTAnimalType as u32, 0, 0, false, false);
            let animal_addr = &animal as *const ZTAnimal as u32;
            save_to_memory::<u32>(animal_addr, 0x0062_ff54u32);

            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);

            unsafe {
                gen_ztthoughtmgr::ADD_THOUGHT.original()(
                    real_ptr as *const u32,
                    string_id,
                    std::ptr::null(),
                    animal_addr as *const u32,
                    fallback_habitat_ptr as *const u32,
                );
            }
            unsafe { &mut *reimpl_ptr }.add_thought(string_id, 0, animal_addr, fallback_habitat_ptr);

            let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

            thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(
                real_fields,
                reimpl_fields,
                "content mismatch for string_id={}, fallback_habitat_ptr={:#x}",
                string_id,
                fallback_habitat_ptr
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

    /// ZTTHOUGHTMGR_POPULATE_THOUGHTS: compares the real `ZTThoughtMgr::populateThoughts`'s effect on
    /// every thought's resolved `thinker_ptr`/`object_ptr`/`habitat_ptr`/`tile_x`/`tile_y` against the
    /// reimplemented `populate_thoughts`, on two identically-seeded standalone managers. Needs
    /// `GLOBAL_ZTWorldMgr` initialized (`ZTThought::populate` calls `ZTWorldMgr::resolve_entity_by_id`
    /// unconditionally), so this runs from `run_on_completion_reset_test_and_exit`'s later chain, not
    /// the early battery. `thinker_id`/`object_id` are generated over a small `0..5` range: real
    /// entities essentially never have ids this low, so `resolve_entity_by_id` returns null on both
    /// sides for the overwhelming majority of cases - a safe, deterministic "no match" - while still
    /// leaving room for a genuine match.
    fn run_thoughtmgr_populate_thoughts_test(failure_log: &mut Option<std::fs::File>) -> bool {
        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHTMGR_POPULATE_THOUGHTS";
        let mut fail_flag = false;

        let record_strategy = (any::<u32>(), 0u32..5, 0u32..5);
        match runner.run(&prop::collection::vec(record_strategy, 0..6), |records| {
            let real_ptr = thought_live_support::build_standalone_mgr(1000);
            let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
            for &(string_id, thinker_id, object_id) in &records {
                thought_live_support::seed_raw_chain(
                    unsafe { &*real_ptr },
                    thought_live_support::new_thought(string_id, thinker_id, object_id, -1, -1, 0, 0, 0),
                );
                unsafe { &mut *reimpl_ptr }.insert_front(thought_live_support::new_thought(string_id, thinker_id, object_id, -1, -1, 0, 0, 0));
            }

            unsafe { gen_ztthoughtmgr::POPULATE_THOUGHTS.original()(real_ptr as *const u32) };
            unsafe { &mut *reimpl_ptr }.populate_thoughts();

            let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
            let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

            thought_live_support::free_raw_chain_mgr(real_ptr);
            thought_live_support::destroy_standalone_mgr(reimpl_ptr);

            prop_assert_eq!(real_fields, reimpl_fields, "mismatch for records={:?}", records);
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

    /// Writes `name` into `entity`'s `name` field (`+0x108`, a `ZTBufferString`) by raw offset write.
    /// `entity` must not move after this call - its address is baked into the write. Returns the
    /// backing byte buffer, which the caller must keep alive at least as long as `entity` is read
    /// through.
    fn set_bfentity_name(entity: &BFEntity, name: &str) -> Vec<u8> {
        let mut encoded = name.as_bytes().to_vec();
        let len = encoded.len() as u32;
        encoded.push(0);
        let start = encoded.as_ptr() as u32;
        let entity_addr = entity as *const BFEntity as u32;
        save_to_memory::<u32>(entity_addr + 0x108, start);
        save_to_memory::<u32>(entity_addr + 0x10c, start + len);
        save_to_memory::<u32>(entity_addr + 0x110, start + encoded.len() as u32);
        encoded
    }

    /// Writes `name` into `habitat`'s `exhibit_name` field (`+0x154`, a `ZTBufferString`) by raw offset
    /// write - same technique and same 3-pointer shape as `set_bfentity_name`.
    ///
    /// **Previously wrote only `start`/`end` (`+0x154`/`+0x158`), modeled on `exhibit_name` as a 2-pointer
    /// `ZTBoundedString`.** `zthabitatmgr.rs`'s own field comment on `exhibit_name` documents that this was
    /// corrected to the 3-pointer `ZTBufferString` (`start`/`end`/`buffer_end`) - this helper was never
    /// updated to match, leaving `buffer_end_ptr` (`+0x15c`) at whatever `build_standalone_show_info`-style
    /// zero-init left it. `ZTBufferString::copy_to_string`'s read loop requires `char_address < buffer_end`
    /// on every iteration, so a zeroed `buffer_end_ptr` (always less than any real `start` address) made it
    /// read zero bytes regardless of `name`'s actual content. Live-reproduced (`ZTTHOUGHT_GET_STRING` with a
    /// widened `cases` count): `case=Habitat("0")` against a real `%s`-shaped template gave `left: "...for
    /// 0."` (real vanilla) vs `right: "...for ."` (reimplementation - `get_string`'s habitat branch read
    /// back an empty exhibit name). Masked at the default case count because it only surfaces when a fuzzed
    /// `string_id` happens to resolve to a real, loadable, `%s`-shaped template *and* the `Habitat` branch
    /// is drawn with a non-empty name - rare enough to look "sporadic" rather than reliably reproducing.
    fn set_habitat_exhibit_name(habitat: &ZTHabitat, name: &str) -> Vec<u8> {
        let mut encoded = name.as_bytes().to_vec();
        let len = encoded.len() as u32;
        encoded.push(0);
        let start = encoded.as_ptr() as u32;
        let habitat_addr = habitat as *const ZTHabitat as u32;
        save_to_memory::<u32>(habitat_addr + 0x154, start);
        save_to_memory::<u32>(habitat_addr + 0x158, start + len);
        save_to_memory::<u32>(habitat_addr + 0x15c, start + encoded.len() as u32);
        encoded
    }

    /// Which of `get_string`'s three substitution branches a `ZTTHOUGHT_GET_STRING` case exercises
    /// (priority: object, then habitat, then no substitution).
    #[derive(Debug, Clone)]
    enum GetStringSubstitution {
        None,
        Object(String),
        Habitat(String),
    }

    fn get_string_substitution_strategy() -> impl Strategy<Value = GetStringSubstitution> {
        prop_oneof![
            Just(GetStringSubstitution::None),
            "[a-zA-Z0-9 ]{0,16}".prop_map(GetStringSubstitution::Object),
            "[a-zA-Z0-9 ]{0,16}".prop_map(GetStringSubstitution::Habitat),
        ]
    }

    /// True if `template` is shaped the way every real, decompile-confirmed `ZTThought` message is: either
    /// no `%` conversion at all, or exactly one `%s`. Real vanilla always calls `wsprintfA(dest, template,
    /// name_ptr)` with exactly one argument when a substitution is attempted (see
    /// `ztthought-getstring-pluralization-bug-handover.md`); this reimplementation's naive
    /// `replacen("%s", name, 1)` only agrees with that for templates shaped this way - confirmed against
    /// every real thought-message string id `ZTGuest::fGuestThought`'s call sites use (see
    /// `run_thought_get_string_test`'s own doc comment).
    fn is_single_percent_s_or_none(template: &str) -> bool {
        match template.matches('%').count() {
            0 => true,
            1 => template.contains("%s"),
            _ => false,
        }
    }

    /// ZTTHOUGHT_GET_STRING: compares the real `ZTThought::getString`'s output against the reimplemented
    /// `get_string`, across all three substitution branches: no substitution (`object_ptr = habitat_ptr
    /// = 0`), object-name substitution (a fixture `BFEntity` with its `name` field set via
    /// `set_bfentity_name`), and habitat exhibit-name substitution (a fixture `ZTHabitat` with
    /// `exhibit_name` set via `set_habitat_exhibit_name`). `get_string` only ever reads these two fields
    /// directly (no vtable dispatch), so a zeroed fixture with just the name field populated is a safe,
    /// complete stand-in for a real live `BFEntity`/`ZTHabitat`. Runs from
    /// `run_on_completion_reset_test_and_exit`'s later chain: language DLLs, which
    /// `load_string_by_id`/`BFApp::loadString` both depend on, aren't loaded yet at the early injection
    /// point.
    ///
    /// `string_id` is fuzzed unconstrained across `any::<u32>()`, which can land on a real, loadable
    /// string that has nothing to do with `ZTThought` (e.g. a research-progress `"Months to complete:
    /// %d"` or a marketing `"Adopt %d %r(s)."` string). Real vanilla's `getString` always calls
    /// `wsprintfA` with exactly one variadic argument no matter what the template asks for, so it only
    /// agrees with this reimplementation's `replacen("%s", name, 1)` when the template has zero `%`
    /// conversions or exactly one `%s`. Every real `ZTThought` message is proven to be shaped that way:
    /// the closed set of literal string-id constants baked into `zoo.exe` at `ZTGuest::fGuestThought`'s
    /// call sites (`0x2758, 0x2759, 0x27fe, 0x2803, 0x2806, 0x2807, 0x280a, 0x282d, 0x2946, 0x2948,
    /// 0x2972, 0x2974, 0x2975`) resolve, per the official string-table dumps, to templates that are
    /// either plain text or exactly one `%s` - never `%d`, never a repeated `%s`. Below, resolved
    /// templates outside that shape are discarded via `prop_assume!` rather than filtering `string_id`
    /// itself, so the fuzzer/shrinker still exercises the full `u32` space and every id that does resolve
    /// to a real `%s`-or-none template. This filtering is skipped for `GetStringSubstitution::None`:
    /// when both `object_ptr` and `habitat_ptr` are null, real vanilla skips `wsprintfA` entirely and
    /// returns the template untouched, and the reimplementation does the same, so both sides already
    /// agree unconditionally there for any template shape.
    ///
    /// Known accepted limitation: a few `fGuestThought` call sites (`ZTBuilding_addUser.c`'s `iVar10`,
    /// `ZTGuest_consumeItem.c`'s `param_1[8]`, `ZTGuest_listen{,_maybe}.c`'s `uVar7`/`uVar4`) pass a
    /// runtime-computed string id rather than a literal, so they aren't covered by the enumeration above.
    /// `param_1[8]` in particular looks `.cfg`-driven, so a mod with a custom item config could in
    /// principle point it at a non-`%s`-shaped string and hit a real (if narrow, mod-specific) divergence
    /// this fix doesn't address.
    fn run_thought_get_string_test(failure_log: &mut Option<std::fs::File>) -> bool {
        debug_assert!(!is_single_percent_s_or_none("Adopt %d %r(s)."));
        debug_assert!(!is_single_percent_s_or_none("Months to complete: %d"));

        let runner_config = ProptestConfig {
            failure_persistence: Some(Box::new(super::NoopFailurePersistence)),
            ..ProptestConfig::default()
        };
        let mut runner = proptest::test_runner::TestRunner::new(runner_config);
        let test_name = "ZTTHOUGHT_GET_STRING";
        let mut fail_flag = false;

        match runner.run(&(any::<u32>(), get_string_substitution_strategy()), |(string_id, case)| {
            if !matches!(case, GetStringSubstitution::None)
                && let Some(template) = &crate::string_registry::load_string_by_id(string_id)
            {
                prop_assume!(is_single_percent_s_or_none(template));
            }

            let entity_storage = BFEntity::new_for_test(0, 0, 0);
            let habitat_storage: ZTHabitat = unsafe { std::mem::zeroed() };
            let _name_buf: Option<Vec<u8>>;

            let (object_ptr, habitat_ptr) = match &case {
                GetStringSubstitution::None => {
                    _name_buf = None;
                    (0u32, 0u32)
                }
                GetStringSubstitution::Object(name) => {
                    _name_buf = Some(set_bfentity_name(&entity_storage, name));
                    (&entity_storage as *const BFEntity as u32, 0u32)
                }
                GetStringSubstitution::Habitat(name) => {
                    _name_buf = Some(set_habitat_exhibit_name(&habitat_storage, name));
                    (0u32, &habitat_storage as *const ZTHabitat as u32)
                }
            };

            let thought = thought_live_support::new_thought(string_id, 0, 0, -1, -1, 0, object_ptr, habitat_ptr);

            let mut buffer = [0u32; 3];
            unsafe {
                gen_ztthought::GET_STRING.original()(&thought as *const ZTThought as *const u32, buffer.as_mut_ptr() as *const u32);
            }
            let real_text = get_from_memory::<ZTBufferString>(buffer.as_ptr() as u32).copy_to_string();
            let reimpl_text = thought.get_string();

            prop_assert_eq!(real_text, reimpl_text, "get_string mismatch for string_id={}, case={:?}", string_id, case);
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
}
