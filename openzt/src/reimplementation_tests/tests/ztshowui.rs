//! Live coverage for the show-editor UI consumers reimplemented in production file
//! `openzt/src/ztshowui.rs`: `fill_trick_lists`/`copy_list_to_script` driven through their
//! real, hooked addresses against a real, loaded zoo's habitats/units (see the test's own
//! doc comment for why selection globals are written directly rather than driving the UI).

use std::io::Write;

use tracing::{error, info};

use openzt_detour::generated::bfworldmgr::GET_TYPE as BFWORLDMGR_GET_TYPE;

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::util::get_from_memory;
use crate::ztshow::live_support as ztshow_live_support;
use crate::ztshowui;

use super::ztshow::{find_real_show_tank_habitat, find_real_trick_eligible_unit};
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
/// exercise real vanilla code instead, per the finding documented at the battery's `run_load_live_zoo`
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
pub(crate) fn run_ztshowui_fill_trick_lists_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWUI_FILL_TRICK_LISTS_LIVE";

    let Some((habitat_ptr, show_info_ptr)) = find_real_show_tank_habitat() else {
        error!("{}: BLOCKED - no real, already-configured show-tank habitat found in test zoo; fill_trick_lists/copy_list_to_script have no live coverage", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no qualifying show-tank habitat found\n", test_name).as_bytes());
        }
        return true;
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
        return true;
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
        return true;
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
    // early-return before touching it otherwise, per `ztshowui.rs`'s own doc comment).
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
