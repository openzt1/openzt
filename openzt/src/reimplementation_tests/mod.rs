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

        // Installs `research_save_reimplementation`'s `SAVE`/`LOAD` detour (the `not(vanilla-research-save)`
        // default arm - a no-op under the `vanilla-research-save` feature) before `detour_zoo_main`'s own
        // battery runs, so the ZTRESEARCHMGR_SAVE/ZTRESEARCHMGR_LOAD tests' `mgr.save()`/`mgr.load()` calls
        // exercise the actual promoted live path, not just the pure `serialize`/`predict_load` helpers
        // against untouched vanilla. Deliberately scoped to just this one detour, not the full
        // `ztresearch::init()` production init chain, which this harness never calls.
        crate::ztresearch::research_save_reimplementation::init();

        // Installs `marketing_save_reimplementation`'s `SAVE`/`LOAD` detour, for the same reason as
        // the research one above - so the ZTMARKETINGMGR_SAVE/ZTMARKETINGMGR_LOAD tests' `mgr.save()`/
        // `mgr.load()` calls exercise the actual promoted live path.
        crate::ztmarketing::marketing_save_reimplementation::init();

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
        fs::OpenOptions,
        io::Write,
        sync::{
            atomic::{AtomicBool, Ordering},
            Once,
        },
    };

    thread_local! {
        static BACKTRACE: Cell<Option<Backtrace>> = const { Cell::new(None) };
    }

    #[cfg(target_os = "windows")]
    use openzt_detour::generated::bfapp::LOAD_LANG_DLLS;
    use openzt_detour::generated::bfentity::GET_FOOTPRINT as BFENTITY_GET_FOOTPRINT;
    use openzt_detour::generated::bftile::GET_LOCAL_ELEVATION;
    use openzt_detour::generated::ztanimal::GET_FOOTPRINT as ZTANIMAL_GET_FOOTPRINT;
    use openzt_detour::generated::ztapp::UPDATE_SIM;
    use openzt_detour::generated::ztmarketing;
    use openzt_detour::generated::ztmarketingmgr::UPDATE as ZTMARKETINGMGR_UPDATE;
    use openzt_detour::generated::ztresearchbranch;
    use openzt_detour::generated::ztresearchbranch::GET_FUNDING_TEXT as ZTRESEARCHBRANCH_GET_FUNDING_TEXT;
    use openzt_detour::generated::ztresearchbranch::UPDATE as ZTRESEARCHBRANCH_UPDATE;
    use openzt_detour::generated::ztresearchmgr;
    use openzt_detour::generated::ztresearchmgr::FORCE_RESEARCH as ZTRESEARCHMGR_FORCE_RESEARCH;
    use openzt_detour::generated::ztresearchmgr::UPDATE as ZTRESEARCHMGR_UPDATE;
    use openzt_detour::generated::ztresearchprogram;
    use openzt_detour::generated::ztunit::GET_FOOTPRINT as ZTUNIT_GET_FOOTPRINT;
    use openzt_detour::FunctionDef;
    use proptest::prelude::*;
    use tracing::{error, info};

    use crate::{
        bfentitytype::{BFEntityType, ZTAnimalType, ZTUnitType},
        globals::globals,
        util::{get_from_memory, ZTBufferString, ZTString},
        ztmapview::BFTile,
        ztmarketing::{live_support as marketing_live_support, marketing_save_reimplementation, ZTMarketingMgr},
        ztresearch::research_save_reimplementation::{self, live_support, SaveRecord},
        ztresearch::{ZTResearchBranch, ZTResearchEffectKind, ZTResearchMgr},
        ztworldmgr::{BFEntity, IVec3, ZTAnimal, ZTUnit},
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

        fail_flag |= run_bfentity_get_footprint_tests(&mut failure_log);
        fail_flag |= run_ztunit_get_footprint_tests(&mut failure_log);
        fail_flag |= run_ztanimal_get_footprint_tests(&mut failure_log);
        fail_flag |= run_research_branch_funding_test(&mut failure_log);
        fail_flag |= run_research_branch_pct_days_remaining_test(&mut failure_log);
        fail_flag |= run_marketing_increase_funding_test(&mut failure_log);
        fail_flag |= run_marketing_decrease_funding_test(&mut failure_log);
        fail_flag |= run_marketing_set_funding_level_test(&mut failure_log);
        fail_flag |= run_marketingmgr_update_test(&mut failure_log);
        fail_flag |= run_marketingmgr_save_test(&mut failure_log);
        fail_flag |= run_marketingmgr_load_test(&mut failure_log);

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

        fail_flag |= run_funding_text_test(&mut failure_log);
        fail_flag |= run_branch_update_test(&mut failure_log);
        fail_flag |= run_research_mgr_update_branches_test(&mut failure_log);
        fail_flag |= run_marketing_update_test(&mut failure_log);
        fail_flag |= run_marketing_funding_text_test(&mut failure_log);

        if fail_flag {
            error!("Proptest failed for some cases, check the failure log at: {}", failure_log_path);
            std::process::exit(1);
        }
        std::process::exit(0);
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
    /// Further restricted to the *insufficient-cash* case - `AVAILABLE_CASH` is fixed to `0.0` and
    /// `funding_cost`/`days` are generated strictly positive, so `cash_delta` is always `> 0.0 ==
    /// available_cash` and neither side's `subtractCash`/`subtract_cash` ever actually runs. This is
    /// deliberate, not an oversight: `ZTGameMgr::subtractCash` also calls `ZTUI::main::setMoneyText`,
    /// a real UI refresh - empirically (crashes with varying exception codes/offsets across repeated
    /// live runs, before this restriction was added) that call depends on UI/window state that isn't
    /// safely touchable from this harness's injection point, the same "downstream, not reimplemented"
    /// surface this file already declines to reimplement elsewhere. The *affordable* branch's math
    /// (`cash_delta`/`progress_delta` computation, and applying them) is still covered - by
    /// `predict_branch_progress`'s own pure proptests, which need no live game/UI state at all - just
    /// not compared byte-for-byte against a live `subtractCash` call here. What this test *does* cover
    /// live: the full eligibility gate (`always_check_expansion`/`getAnyExpansionsDisabled`/
    /// `isExpansionDisabled`/category-enabled/program-selected checks) and confirms both sides leave
    /// `current_progress` identically unchanged when unaffordable.
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

        const AVAILABLE_CASH: f32 = 0.0;
        const TARGET_COST: f32 = 1_000_000.0;

        match runner.run(
            &(1u32..1000u32, -1000f32..1000f32, 1f32..1000f32, 0f32..1000f32),
            |(days, funding_rate, funding_cost, initial_progress)| {
                let real_progress = live_support::with_update_test_branch(TARGET_COST, initial_progress, funding_rate, funding_cost, |mgr| {
                    let branch = mgr.branch_mut(0);
                    live_support::with_ztgamemgr_cash(AVAILABLE_CASH, || unsafe {
                        ZTRESEARCHBRANCH_UPDATE.original()((branch as *mut ZTResearchBranch) as *const u32, days);
                    });
                    branch.current_program().map(|p| p.current_progress())
                });

                let reimpl_progress = live_support::with_update_test_branch(TARGET_COST, initial_progress, funding_rate, funding_cost, |mgr| {
                    let branch = mgr.branch_mut(0);
                    live_support::with_ztgamemgr_cash(AVAILABLE_CASH, || branch.update(days));
                    branch.current_program().map(|p| p.current_progress())
                });

                prop_assert_eq!(
                    real_progress,
                    reimpl_progress,
                    "current_progress mismatch for days={}, funding_rate={}, funding_cost={}, initial_progress={}",
                    days,
                    funding_rate,
                    funding_cost,
                    initial_progress
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

    /// ZTRESEARCHMGR_UPDATE (branch-count extension): compares the real `ZTResearchMgr::update`'s
    /// effect on `elapsed_ticks` and every branch's currently-selected program's `current_progress`
    /// against the reimplemented `update`, for 1-3 synthetic branches built via
    /// `live_support::with_update_test_branches`. The zero-branch `ZTRESEARCHMGR_UPDATE` test above
    /// only exercises `elapsed_ticks`' accumulator/day-count bookkeeping in isolation - this is the
    /// first test that actually exercises `ZTResearchMgr::update` iterating multiple branches and
    /// threading the correct `days` count to each (via `ZTResearchBranch::update`, native since Phase F).
    ///
    /// Restricted the same way `ZTRESEARCHBRANCH_UPDATE` is - `target_cost` fixed far above any possible
    /// progress delta and the *insufficient-cash* case only (`AVAILABLE_CASH = 0.0`, `funding_cost`
    /// generated strictly positive) - so `on_completion`/`pick_random_program`/the `subtractCash`-driven
    /// UI refresh never run on either side; see `run_branch_update_test`'s own doc comment for why that
    /// restriction exists and stays in place here too. `funding_rate` is fixed to `0.0`, documented as
    /// inert under this restriction like every other fixed-but-unused field in `live_support`.
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

        const AVAILABLE_CASH: f32 = 0.0;
        const TARGET_COST: f32 = 1_000_000.0;
        const FUNDING_RATE: f32 = 0.0;

        let branch_spec_strategy = (1f32..1000f32, 0f32..1000f32).prop_map(|(funding_cost, initial_progress)| {
            live_support::UpdateTestBranchSpec { target_cost: TARGET_COST, initial_progress, funding_rate: FUNDING_RATE, funding_cost }
        });

        match runner.run(
            &(any::<u32>(), any::<u32>(), prop::collection::vec(branch_spec_strategy, 1..4)),
            |(elapsed_ticks_before, delta_ticks, branch_specs)| {
                let (real_elapsed_ticks, real_progress_bits) = live_support::with_update_test_branches(&branch_specs, |mgr| {
                    mgr.set_elapsed_ticks(elapsed_ticks_before);
                    live_support::with_ztgamemgr_cash(AVAILABLE_CASH, || unsafe {
                        ZTRESEARCHMGR_UPDATE.original()((mgr as *mut ZTResearchMgr) as *const u32, delta_ticks);
                    });
                    let progress_bits =
                        mgr.branches().flat_map(|b| b.current_program()).map(|p| p.current_progress().to_bits()).collect::<Vec<_>>();
                    (mgr.elapsed_ticks(), progress_bits)
                });

                let (reimpl_elapsed_ticks, reimpl_progress_bits) = live_support::with_update_test_branches(&branch_specs, |mgr| {
                    mgr.set_elapsed_ticks(elapsed_ticks_before);
                    live_support::with_ztgamemgr_cash(AVAILABLE_CASH, || mgr.update(delta_ticks));
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
    /// reimplemented `increase_funding`, for two independently-constructed but structurally identical
    /// standalone `ZTMarketing`s (built via `marketing_live_support::build_standalone_marketing` - not
    /// spliced into any `ZTMarketingMgr`, since `increaseFunding` only ever reads/writes `this`).
    /// `current_funding_level` spans `0..6` to cover in-range/top-of-range/one-past-the-end starting
    /// values against funding tables spanning empty (`0`) through a few entries. Compares both the
    /// resulting index and vanilla's masked low-byte return value (`increase_funding`'s own doc comment
    /// on `ztmarketing.rs` explains why that byte is exactly what a standalone `isFundingMaxed()` call
    /// would report after the operation).
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
    /// reimplemented `set_funding_level`. `level` spans `0..6` (in-range through one/several past the
    /// end) against funding tables spanning empty (`0`) through a few entries, to cover
    /// `setFundingLevel`'s "reset to `0`" out-of-range behavior - deliberately different from
    /// `increaseFunding`'s saturating behavior, see `set_funding_level`'s own doc comment.
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

    /// ZTMARKETINGMGR_UPDATE: compares the real `ZTMarketingMgr::update`'s effect on
    /// `tick_accumulator` against the reimplemented `update`, for a synthetic manager with no owned
    /// `ZTMarketing` (`marketing_ptr = null`) - so `ZTMarketing::update` (which needs a live
    /// `GLOBAL_ZTGameMgr` - see `run_marketing_update_test` below) never actually runs on either side,
    /// matching vanilla's own null-pointer guard exactly. Mirrors `ZTRESEARCHMGR_UPDATE`'s own
    /// zero-branches version.
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
    /// `ZTMarketing` (a one-entry funding table at index `0`, so `ZTMarketing::update`'s unchecked
    /// `funding_level(current_funding_level)` read is always safe) against the reimplemented `update`,
    /// with `tick_accumulator` fixed so a threshold crossing always happens (`delta_ticks` generated
    /// `3000..10000`, always `> 359` days' worth per `predict_mgr_update`) - so `ZTMarketing::update`
    /// genuinely runs on both sides, not just the accumulator bookkeeping already covered by
    /// `ZTMARKETINGMGR_UPDATE` above. Run from `run_on_completion_reset_test_and_exit`'s `updateSim`
    /// injection point rather than the earlier `LOAD_LANG_DLLS` battery, same as
    /// `ZTRESEARCHBRANCH_UPDATE`/`ZTRESEARCHMGR_UPDATE_BRANCHES` - `GLOBAL_ZTGameMgr` isn't constructed
    /// yet at `LOAD_LANG_DLLS` (confirmed live: this test reported "skipped" there before moving here).
    ///
    /// Restricted to the *insufficient-cash* case exactly like `ZTRESEARCHBRANCH_UPDATE`
    /// (`AVAILABLE_CASH = 0.0`, `funding_cost` generated strictly positive) - see that test's own doc
    /// comment in `ztresearch.rs`'s harness for why: `ZTGameMgr::subtractCash` also calls
    /// `ZTUI::main::setMoneyText`, a real UI refresh not safely touchable from this injection point.
    /// What this test *does* cover live: that `ZTMarketing::update` reads the right funding level, the
    /// right day count, and leaves the budget/index untouched when unaffordable - on both sides
    /// identically.
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

        const AVAILABLE_CASH: f32 = 0.0;

        match runner.run(&(3000u32..10000, 1f32..1000f32), |(delta_ticks, funding_cost)| {
            let real_marketing_ptr = marketing_live_support::build_standalone_marketing_with_cost(funding_cost);
            let real_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, real_marketing_ptr);
            marketing_live_support::with_ztgamemgr_cash(AVAILABLE_CASH, || unsafe {
                ZTMARKETINGMGR_UPDATE.original()((real_mgr_ptr as *mut ZTMarketingMgr) as *const u32, delta_ticks);
            });
            let real_tick_accumulator = unsafe { &*real_mgr_ptr }.tick_accumulator();
            let real_index = unsafe { &*real_marketing_ptr }.current_funding_level();
            marketing_live_support::destroy_standalone_marketing_mgr(real_mgr_ptr);
            marketing_live_support::destroy_standalone_marketing(real_marketing_ptr);

            let reimpl_marketing_ptr = marketing_live_support::build_standalone_marketing_with_cost(funding_cost);
            let reimpl_mgr_ptr = marketing_live_support::build_standalone_marketing_mgr(0, reimpl_marketing_ptr);
            marketing_live_support::with_ztgamemgr_cash(AVAILABLE_CASH, || unsafe { &mut *reimpl_mgr_ptr }.update(delta_ticks));
            let reimpl_tick_accumulator = unsafe { &*reimpl_mgr_ptr }.tick_accumulator();
            let reimpl_index = unsafe { &*reimpl_marketing_ptr }.current_funding_level();
            marketing_live_support::destroy_standalone_marketing_mgr(reimpl_mgr_ptr);
            marketing_live_support::destroy_standalone_marketing(reimpl_marketing_ptr);

            prop_assert_eq!(
                real_tick_accumulator,
                reimpl_tick_accumulator,
                "tick_accumulator mismatch for delta_ticks={}, funding_cost={}",
                delta_ticks,
                funding_cost
            );
            prop_assert_eq!(
                real_index,
                reimpl_index,
                "current_funding_level mismatch for delta_ticks={}, funding_cost={}",
                delta_ticks,
                funding_cost
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

    /// ZTMARKETING_GET_FUNDING_TEXT: compares the real `ZTMarketing::getFundingText`'s output against
    /// the reimplemented `ZTMarketing::funding_text`, for a standalone marketing (not spliced into any
    /// `ZTMarketingMgr` - see `marketing_live_support::build_standalone_marketing_with_levels`'s doc
    /// comment) with a generated funding table and `current_funding_level` spanning negative/in-range/
    /// out-of-range relative to the table's length. Same shape as `run_funding_text_test` above,
    /// reusing its `funding_level_case_strategy` for the (name_id, cost) generation - per the
    /// implementation plan's item 3, `ZTMarketing::getFundingText` goes through the exact same
    /// `bfinternat::getMoneyText`/string-table machinery research's own `funding_text` already
    /// confirmed, just without the `1.0/30.0` day-scale pre-multiply (see `ZTMarketing::funding_text`'s
    /// own doc comment in `ztmarketing.rs`), so a resolvable-or-not name id is comparably meaningful to
    /// either side regardless of which class "owns" that string id in vanilla's own data.
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
    /// `io_redirect`, the same `WRITE_BYTES_TO_FILE` redirect `ZTRESEARCHMGR_SAVE` uses) against the
    /// single little-endian `u32` funding-level index vanilla is expected to write - `0` when no
    /// `ZTMarketing` is owned, per `ZTMarketingMgr_save.c`.
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
}
