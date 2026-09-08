//! `BFTile`/`ZTResearchMgr`/`ZTResearchBranch`/`ZTResearchProgram` comparison tests (production
//! files `openzt/src/ztmapview.rs` and `openzt/src/ztresearch.rs`). Mixed phases: the
//! `BFTILE_GET_LOCAL_ELEVATION`/`ZTRESEARCHBRANCH_FUNDING`/`ZTRESEARCHBRANCH_PCT_DAYS_REMAINING`/
//! `ZTRESEARCHMGR_*` entries run in the early battery (while the game is still mid-boot), the
//! `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET` and `ZTRESEARCHBRANCH_FUNDING_TEXT`/`_UPDATE`-family
//! entries need the first `updateSim` tick (always-late), and
//! `ZTRESEARCHMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE` needs the loaded real zoo. File order mirrors
//! battery order.

use std::io::Write;

use openzt_detour::generated::bftile::GET_LOCAL_ELEVATION;
use openzt_detour::generated::ztresearchbranch;
use openzt_detour::generated::ztresearchbranch::GET_FUNDING_TEXT as ZTRESEARCHBRANCH_GET_FUNDING_TEXT;
use openzt_detour::generated::ztresearchbranch::UPDATE as ZTRESEARCHBRANCH_UPDATE;
use openzt_detour::generated::ztresearchmgr;
use openzt_detour::generated::ztresearchmgr::FORCE_RESEARCH as ZTRESEARCHMGR_FORCE_RESEARCH;
use openzt_detour::generated::ztresearchmgr::UPDATE as ZTRESEARCHMGR_UPDATE;
use openzt_detour::generated::ztresearchprogram;
use proptest::prelude::*;
use tracing::{error, info};

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::util::{get_from_memory, ZTBufferString, ZTString};
use crate::ztmapview::BFTile;
use crate::ztresearch::research_save_reimplementation::{self, live_support, SaveRecord};
use crate::ztresearch::{predict_branch_progress, predict_update, ZTResearchBranch, ZTResearchEffectKind, ZTResearchMgr};
use crate::ztworldmgr::IVec3;

use super::ztmarketing::funding_level_case_strategy;

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

/// Same as `generated_branches` - also pins `effect_kind_raw` to `-1` (unset), same rationale.
/// Used only by `ZTRESEARCHMGR_LOAD`/`ZTRESEARCHMGR_LOAD_CORRUPT_STREAM`, whose tail calls
/// `on_completion()` for any program whose loaded `current_progress` ends up `>= target_cost`.
///
/// This used to thread each program's generated `effect_kind_raw` through instead of pinning it,
/// specifically to exercise that dispatch - but a valid `effect_kind` (`0..=7`) makes
/// `on_completion()` call straight into real vanilla code (`SET_AVAIL`/`SET_*_CHARACTERISTIC`/
/// `SET_TRICK_AVAILABLE`/`SET_EFFECT_DISCOUNT`), and every one of those unconditionally walks
/// `GLOBAL_ZTWorldMgr`'s entity-type list - which is null at this early `LOAD_LANG_DLLS`-hooked
/// injection point (confirmed live via `crash-capture`: an access violation inside
/// `SET_EFFECT_DISCOUNT`, at exactly the instruction its decompile shows dereferencing
/// `GLOBAL_ZTWorldMgr->BFWorldMgr.field_0x98`). Pinning `-1` here trades away that dispatch
/// coverage, but it was never actually verified by this test's own assertions anyway -
/// `predict_load`/`funding_levels`/`enabled`/`progress_bits` are all fully determined by the read
/// loop that runs *before* this tail, so `on_completion()`'s real-vanilla side effects, even when
/// they succeed, never touch a field this test checks.
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
                            effect_kind_raw: -1,
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

/// BFTILE_GET_LOCAL_ELEVATION: registered as `early_tests`' first entry - the position this
/// battery's formerly hand-inlined version occupied (it ran straight after `detour_target` created
/// the log, before every other early test). Compares the real vanilla `GET_LOCAL_ELEVATION` against
/// the reimplemented `BFTile::get_local_elevation` for every known value of the tile's
/// not-yet-fully-understood `unknown_byte_2` byte, over x/y in `0..1000`.
pub(crate) fn run_bftile_get_local_elevation_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
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

            let result = unsafe { GET_LOCAL_ELEVATION.original()(&raw const tile as *const u32, &raw const pos as *const u32) };
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
                    let result = unsafe { GET_LOCAL_ELEVATION.original()(&raw const tile as *const u32, &raw const pos as *const u32) };
                    let failure_line = format!("unknown_byte_2: {}, x: {}, y: {}, real: {}, reimplemented: {}\n", unknown_byte_2, x, y, result, reimplemented_result);

                    if let Some(log_file) = failure_log
                        && let Err(write_err) = log_file.write_all(failure_line.as_bytes())
                    {
                        error!("Failed to write to failure log: {}", write_err);
                    }

                    info!("Failed case ({}): x: {}, y: {}", r, x, y);
                    fail_flag = true;
                }
            }
        }
    }

    if fail_flag {
        error!("Proptest failed for some cases, check the failure log");
    } else {
        write_success_line(failure_log, test_name);
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
pub(crate) fn run_research_branch_funding_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
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
pub(crate) fn run_research_branch_pct_days_remaining_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
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

/// ZTRESEARCHMGR_SAVE: compares the real ZTResearchMgr::save's captured output against
/// research_save_reimplementation::serialize(&snapshot_mgr(mgr)) for generated synthetic trees.
pub(crate) fn run_ztresearchmgr_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTRESEARCHMGR_SAVE";
    let mut fail_flag = false;

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

/// ZTRESEARCHMGR_LOAD: compares the real ZTResearchMgr::load's effect on funding
/// levels/enabled flags/current progress against research_save_reimplementation::predict_load,
/// for generated synthetic trees, generated save-stream records, and versions spanning both
/// sides of the 0x28 threshold that gates whether the stream is read at all.
pub(crate) fn run_ztresearchmgr_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTRESEARCHMGR_LOAD";
    let mut fail_flag = false;

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

/// ZTRESEARCHMGR_LOAD_CORRUPT_STREAM: exercises `load`'s corrupt-stream abort path - a
/// malformed `kind` tag (anything other than `0`/`1`/`2`/the `-1` terminator) partway through
/// the stream, which `detours::load`'s reading loop treats the same as a genuine I/O read
/// failure: `load` returns `false` without running the `on_completion`/`pick_random_program`
/// tail, but every record read *before* the corruption point has already been applied (per
/// `ZTResearchMgr_load.c`/`.asm`'s "any read failure aborts the whole load" semantics - not a
/// rollback of already-applied records). `SaveRecord`/`serialize` can only ever produce
/// well-formed `kind` tags, so this path has no coverage anywhere else in the battery. `version`
/// is fixed `>= 0x28` (the threshold gating whether the stream is read at all - below it,
/// there's nothing to corrupt); `raw_truncate_at` is reduced modulo `records.len() + 1` to land
/// in `0..=records.len()`, choosing how many well-formed records precede the injected
/// corruption. The expected state is `predict_load` fed only the *prefix* of records (before the
/// corruption point) - the unconditional reset always applies, and every record before the
/// corruption point was already parsed and applied before the abort, so this needs no new
/// prediction logic.
pub(crate) fn run_ztresearchmgr_load_corrupt_stream_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTRESEARCHMGR_LOAD_CORRUPT_STREAM";
    let mut fail_flag = false;

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

/// ZTRESEARCHMGR_UPDATE: compares the real ZTResearchMgr::update's effect on `elapsed_ticks`
/// against the reimplemented `ZTResearchMgr::update`, for a synthetic manager with zero
/// branches - so `ZTResearchBranch::update` (still a call into the original implementation)
/// never actually runs, keeping this comparison independent of branch-level state.
pub(crate) fn run_ztresearchmgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTRESEARCHMGR_UPDATE";
    let mut fail_flag = false;

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

/// ZTRESEARCHMGR_FORCE_RESEARCH: compares the real ZTResearchMgr::forceResearch's effect on
/// every program's `current_progress` against the reimplemented `ZTResearchMgr::force_research`,
/// for generated synthetic trees with exactly one program per category (see
/// `force_research_generated_branches`'s doc comment) and both `continue_program` values. Every
/// generated program's `effect_kind_raw` is fixed to `-1` (unset) by `live_support::GeneratedProgram`,
/// making `on_completion` a guaranteed no-op that needs no `GLOBAL_ZTWorldMgr`, so - like SAVE/LOAD/
/// UPDATE above - this can run at this early injection point rather than waiting for `updateSim`.
pub(crate) fn run_ztresearchmgr_force_research_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTRESEARCHMGR_FORCE_RESEARCH";
    let mut fail_flag = false;

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

/// ZTRESEARCHMGR_LOOKUPS: compares the real ZTResearchMgr::getBranch/getCategory/getProgram
/// against the reimplemented get_branch/get_category/get_program, on one shared generated tree
/// (read-only, so - unlike SET_EFFECT_DISCOUNT below - both sides query the very same tree
/// instance, no need for two independently-built ones), with small overlapping ids (see
/// `lookup_branch_case_strategy`'s doc comment) to exercise duplicate/colliding-id traversal
/// order, plus a lookup id that may or may not be present in the tree.
pub(crate) fn run_ztresearchmgr_lookups_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTRESEARCHMGR_LOOKUPS";
    let mut fail_flag = false;

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

/// Same rationale as `days_approximately_eq` above: real vanilla's `setEffectDiscount` runs
/// `(100 - discount_pct) * target_cost * 0.01` on the x87 stack (80-bit extended precision
/// internally, rounded to `f32` only once at the end), while the reimplementation's strict `f32`
/// arithmetic rounds after each operation - a single-ULP-scale difference, not a logic bug.
/// Confirmed live: e.g. `target_cost=150191.47, discount_pct=60, kind_raw=4` produced bit patterns
/// `1198173334` vs `1198173335` (real vs reimplemented) - exactly one bit apart.
fn target_costs_approximately_eq(real: &[u32], reimpl: &[u32]) -> bool {
    real.len() == reimpl.len()
        && real.iter().zip(reimpl).all(|(&real_bits, &reimpl_bits)| {
            let real = f32::from_bits(real_bits);
            let reimpl = f32::from_bits(reimpl_bits);
            if real.is_nan() && reimpl.is_nan() {
                return true;
            }
            let tolerance = 64.0 * f32::EPSILON * real.abs().max(reimpl.abs()).max(1.0);
            (real - reimpl).abs() <= tolerance
        })
}

/// ZTRESEARCHMGR_SET_EFFECT_DISCOUNT: compares the real ZTResearchMgr::setEffectDiscount's
/// effect on every program's `target_cost` against the reimplemented `set_effect_discount`, for
/// two independently-constructed but structurally identical trees
/// (`effect_discount_generated_branches` is called once per side) - mutating, unlike LOOKUPS
/// above, so each side needs its own tree rather than sharing one. `kind` spans every valid
/// `ZTResearchEffectKind`; each generated program's own `effect_kind_raw` spans `-1..=8` (see
/// `effect_discount_program_case_strategy`'s doc comment), covering both matching and
/// non-matching programs within the same tree; `discount_pct` spans below `0`, within `0..=100`,
/// and above `100` - the reimplementation applies the raw arithmetic unconditionally, with no
/// range clamp on either side. Uses a relative-tolerance comparison (`target_costs_approximately_eq`)
/// rather than exact bit equality - see its own doc comment.
pub(crate) fn run_ztresearchmgr_set_effect_discount_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTRESEARCHMGR_SET_EFFECT_DISCOUNT";
    let mut fail_flag = false;

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

            prop_assert!(
                target_costs_approximately_eq(&real_target_cost_bits, &reimpl_target_cost_bits),
                "target_cost mismatch for kind_raw={}, discount_pct={} cases: {:?}: real={:?}, reimpl={:?}",
                kind_raw,
                discount_pct,
                cases,
                real_target_cost_bits,
                reimpl_target_cost_bits
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

/// ZTRESEARCHPROGRAM_ON_COMPLETION_RESET: registered as `always_late_tests`' first entry. Needs
/// `GLOBAL_ZTWorldMgr` initialized (every underlying effect call it exercises walks its
/// entity-type list), which - confirmed live via the Lua console's `get_zt_world_mgr()` - isn't
/// true yet at `detour_target`'s `LOAD_LANG_DLLS` injection point (that early, the language DLLs
/// haven't even loaded) but *is* true by the time `ZTApp::updateSim` starts ticking (entity types
/// load during app init, before the main loop starts). Compares the real
/// `ZTResearchProgram::onCompletion`/`reset` against `ztresearch::dispatch_on_completion`/
/// `dispatch_reset` (via the public `on_completion()`/`reset()` wrappers) for every
/// `effect_kind_raw` from `-1` (unset) through `8` (one past the last valid kind), on freestanding
/// programs built with `live_support::build_standalone_program` (safe no-op target/entity ids -
/// see its own doc comment). Only the low byte of the real return value is compared - the rest is
/// undefined garbage left over in EAX from whatever vanilla called last (see
/// `ZTResearchProgram::on_completion`'s doc comment).
pub(crate) fn run_ztresearchprogram_on_completion_reset_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTRESEARCHPROGRAM_ON_COMPLETION_RESET";
    if globals().ztworldmgr_ptr().is_null() {
        info!("Skipping {}: GLOBAL_ZTWorldMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTWorldMgr not initialized)", test_name));
        return false;
    }

    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let mut fail_flag = false;

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

/// ZTRESEARCHBRANCH_FUNDING_TEXT: compares the real `ZTResearchBranch::getFundingText`'s output
/// against the reimplemented `ZTResearchBranch::funding_text`, for a standalone branch (not
/// spliced into any `ZTResearchMgr` - see `live_support::build_standalone_funding_branch`'s doc
/// comment) with a generated funding table and `current_funding_level` spanning negative/in-range/
/// out-of-range relative to the table's length, to exercise the "no active level" empty-string path
/// alongside the normal formatted-text path. This test runs at the `updateSim` first-tick injection
/// point specifically so the funding-level name string ids are actually resolvable (language DLLs
/// load during app init, same reasoning as `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET` above).
pub(crate) fn run_funding_text_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
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
pub(crate) fn run_branch_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
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
pub(crate) fn run_branch_update_reimpl_boundary_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
pub(crate) fn run_research_mgr_update_branches_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
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

/// ZTRESEARCHMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE: same "real, live-loaded data" round-trip philosophy as
/// `ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE` (`tests/ztshowscriptmgr.rs`), applied to
/// `research_save_reimplementation`
/// (`ZTRESEARCHMGR_SAVE`'s own coverage is proptest-generated synthetic trees only). Captures the
/// real, hooked `ZTResearchMgr::save`'s output for the real, live `globals().ztresearchmgr()`
/// singleton (real branches/categories/programs with real funding levels/progress values - richer
/// than any hand-built test tree), then asserts `research_save_reimplementation::parse` recovers
/// exactly what `snapshot_mgr` independently read straight from that same live memory. `save()` has
/// no side effects (a pure `WriteBytesToFile` call), so this is safe to run against the real
/// singleton directly - no standalone-instance plumbing needed.
pub(crate) fn run_ztresearchmgr_real_zoo_save_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
