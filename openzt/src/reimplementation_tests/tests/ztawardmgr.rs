//! Compares real vanilla `ZTAwardMgr` against its Rust reimplementation (production file
//! `openzt/src/ztawardmgr.rs`): addAward save/load, start, getAward, showAwards, and the real-zoo
//! singleton save/load round-trip. Also holds `reset_awardmgr_both_sides` - a reset helper shared
//! beyond this class's own tests (`tests/ztscenariosimplegoal.rs` needs it too), kept here where
//! the class owns it.

use openzt_detour::generated::bfuimgr::GET_ELEMENT_0 as BFUIMGR_GET_ELEMENT_0;
use openzt_detour::generated::uilistbox as gen_uilistbox;
use openzt_detour::generated::ztawardmgr as gen_ztawardmgr;
use proptest::prelude::*;
use std::io::Write;
use tracing::{error, info};

use crate::globals::get_module_base;
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::util::get_from_memory;
use crate::ztawardmgr;
use crate::ztawardmgr::live_support as award_live_support;

/// Resets both the real vanilla singleton's earned-id vector and the Rust-side store to empty.
/// Exploits `ZTAwardMgr::load`'s own "reset-then-fill" semantics as a safe, allocator-agnostic clear
/// for the real side (feeding a single `0i32` count via `io_redirect::begin_replay` means the real
/// `load` resets the vector, reads a `0` count, and returns immediately without calling `addAward`) -
/// there's no dedicated clear method and no way to build a second standalone instance for this class
/// (see `ztawardmgr.rs`'s module doc comment).
pub(crate) fn reset_awardmgr_both_sides() {
    let real_ptr = award_live_support::real_ptr();
    let file_buffer = [0u32; 4];
    io_redirect::begin_replay(0u32.to_le_bytes().to_vec());
    unsafe { gen_ztawardmgr::LOAD.original()(real_ptr, file_buffer.as_ptr(), 0) };
    io_redirect::end_replay();
    award_live_support::reset_reimplemented_store();
}

// ============================================================================================
// ZTAwardMgr - see openzt/src/ztawardmgr.rs.
// `_ADD_AWARD_SAVE_LOAD`/`_START`/`_GET_AWARD` and `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT` are
// self-contained (resources are already loaded by this early injection point, and none of them
// need `GLOBAL_ZTWorldMgr`), so they run from `early_tests()`. `_SHOW_AWARDS` and
// `ZTAWARDMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE` need the live zoo (a live `BFUIMgr` element /
// the loaded real zoo's earned awards), so they are registered in `live_zoo_tests()`.
// ============================================================================================

/// ZTAWARDMGR_ADD_AWARD_SAVE_LOAD: for a generated sequence of ids, feeds the same sequence through
/// the real `ADD_AWARD.original()` and the reimplemented `ztawardmgr::add_award` independently (both
/// sides reset to empty first via `reset_awardmgr_both_sides`), then compares the real
/// `SAVE.original()`'s captured output (via `io_redirect`) against the reimplemented `save`'s own
/// output - should be byte-identical, since both reproduce the exact `i32` count + `i32[count]` wire
/// format.
pub(crate) fn run_awardmgr_add_award_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
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
pub(crate) fn run_awardmgr_start_test(failure_log: &mut Option<std::fs::File>) -> bool {
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
pub(crate) fn run_awardmgr_get_award_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

/// ZTAWARDMGR_SHOW_AWARDS: diff-oracle comparison of the reimplemented `_showAwards` detour against
/// real vanilla. Seeds both the real singleton and the Rust store with the same two catalogue award
/// ids (from `ZTAWARDMGR_START`, which has already run earlier in this battery), clears the listbox
/// and populates it via real vanilla (`ztawardmgr::show_awards_detour::call_real` - see its doc
/// comment for why not `SHOW_AWARDS.original()`), counts items via [`listbox_item_count`], then
/// repeats against the hooked address (our detour, driven by the Rust store) and compares counts.
/// Runs after `run_load_live_zoo` since it needs a live `BFUIMgr` element `0x101c` to exist.
///
/// Only item *counts* are compared, not per-item content - the icon-buffer/color-argument shape and
/// the `load_string_by_id`-vs-`buildString` text equivalence remain open items needing separate
/// manual live verification, since `UIListBoxItem`'s internal field layout for those isn't
/// decompile-confirmed. A count mismatch still catches real bugs (wrong catalogue filtering, an id
/// silently dropped, an off-by-one in the population loop).
///
/// **Resets both sides, then re-runs `ztawardmgr::start()`, in that order** - not the reverse.
/// `reset_awardmgr_both_sides` clears the Rust-side catalogue too (`reset_reimplemented_store` clears
/// both `earned_ids` and `awards`, not just earned-ids), while real vanilla's own catalogue tree is
/// untouched by that reset (`ZTAwardMgr::load` only resets the earned-ids vector) - so only the Rust
/// side needs repopulating, and populating it before the reset would leave it empty.
pub(crate) fn run_awardmgr_show_awards_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

/// ZTAWARDMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE: round-trips the loaded real zoo's earned awards
/// through the reimplementation. This test build never installs `ztawardmgr::award_mgr_detours` (see
/// `reimplementation_tests::init()`'s comment on why - `.original()` needs to stay reachable for the
/// other `ZTAWARDMGR_*` tests' real-vanilla comparisons), so the real zoo's own `ZTAwardMgr::load`
/// runs genuine, undetoured vanilla code against the real singleton's `+0xc` vector, never touching
/// the Rust store. This reads that real vector directly
/// (`award_live_support::read_vanilla_earned_ids`), captures real vanilla `save()`'s own output for
/// it (`.original()`, since `SAVE` is undetoured here too), replays those bytes into the Rust
/// reimplementation's `load()` (`crate::ztawardmgr::load`, a plain function - there's no hooked
/// address to go through), and asserts the reimplementation's resulting `earned_ids()` matches the
/// real vector, compared as sorted sets (`load` re-inserts via `add_award`'s sorted-unique insert,
/// which may reorder).
pub(crate) fn run_ztawardmgr_real_zoo_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

    if !save_ok {
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
