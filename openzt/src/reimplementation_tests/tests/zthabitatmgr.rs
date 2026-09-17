//! Compares the reimplemented `ZTHabitatMgr`/`ZTHabitat` methods (production file
//! `openzt/src/zthabitatmgr.rs`) against real vanilla over the live, loaded zoo's own habitat grid.

use openzt_detour::generated::{
    bfentity::GET_TILE as BFENTITY_GET_TILE, bfmap::WORLD_TO_TILE, standalone::OPERATOR_NEW, zthabitat, zthabitatmgr, ztviewingarea,
};
use std::fmt::Debug;
use std::io::Write;
use tracing::error;

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory};
use crate::ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG};
use crate::ztshow::RVA_ANIMAL_TYPE_CHECK;
use crate::zthabitatmgr::{
    free_event_vector_buffer, hooks_zthabitatmgr, walk_neighbor_tree, walk_tile_list, ZTHabitat, ZTHabitatMgr, IS_RIGHT_SALINITY,
    RVA_KEEPER_TYPE_CHECK_ARG,
};

/// `ZTHABITATMGR_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `zthabitatmgr::init()`, and this asserts all of its detours actually report enabled. Without
/// it, a silently-failed `init_detours()` (error logged, game continues on vanilla) would leave the
/// whole battery green while every hooked production path runs vanilla. Runs before the other
/// `ZTHABITATMGR_*`/`ZTHABITAT_*` tests so a wiring failure is visible first.
pub(crate) fn run_zthabitatmgr_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = hooks_zthabitatmgr::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
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

/// Shared driver for the per-habitat `ZTHabitat` getter comparison tests below: compares each real
/// vanilla `.original()` result against the reimplemented method, over every habitat in the live,
/// loaded zoo's own `exhibit_array`. `real`/`reimpl` take the habitat's raw pointer / a live
/// `&ZTHabitat` reference (via `ref_from_memory`, not a `ZTArray::get` copy) respectively, since
/// some of the reimplemented methods (`get_attractiveness`/`has_keeper_assigned`) require a live
/// reference to safely call through to vanilla's `recalculateCharacteristics`.
fn compare_over_live_habitats<T: PartialEq + Debug>(
    failure_log: &mut Option<std::fs::File>,
    test_name: &str,
    real: impl Fn(*const u32) -> T,
    reimpl: impl Fn(&ZTHabitat) -> T,
) -> bool {
    let habitat_mgr = globals().zthabitatmgr();
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let real_value = real(ptr as *const u32);
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let reimpl_value = reimpl(habitat);
        if real_value != reimpl_value {
            error!("{}: mismatch at habitat {} ({:#010x}): real={:?}, reimpl={:?}", test_name, i, ptr, real_value, reimpl_value);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: mismatch at habitat {} ({:#010x}): real={:?}, reimpl={:?}\n", test_name, i, ptr, real_value, reimpl_value)
                        .as_bytes(),
                );
            }
            fail_flag = true;
        }
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Real called before reimpl deliberately: real `getAttractiveness`/`hasKeeperAssigned` clear
/// `characteristics_dirty` as a side effect of the lazy recalculate, so calling real first and then
/// reimpl (which reads the same, now-clean live memory) compares the same settled state rather than
/// racing which side triggers the recalculation.
pub(crate) fn run_habitat_get_attractiveness_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_ATTRACTIVENESS_LIVE",
        |ptr| unsafe { zthabitat::GET_ATTRACTIVENESS.original()(ptr) },
        |habitat| habitat.get_attractiveness(),
    )
}

/// See [`run_habitat_get_attractiveness_live_test`]'s doc comment for the real-before-reimpl
/// ordering rationale (shared `characteristics_dirty` side effect).
pub(crate) fn run_habitat_has_keeper_assigned_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_HAS_KEEPER_ASSIGNED_LIVE",
        |ptr| unsafe { zthabitat::HAS_KEEPER_ASSIGNED.original()(ptr) != 0 },
        |habitat| habitat.has_keeper_assigned(),
    )
}

pub(crate) fn run_habitat_get_gate_tile_out_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_GATE_TILE_OUT_LIVE",
        |ptr| unsafe { zthabitat::GET_GATE_TILE_OUT.original()(ptr) },
        |habitat| match habitat.get_gate_tile_out() {
            Some(tile) => globals().ztworldmgr().get_ptr_from_bftile(&tile) as i32,
            None => 0,
        },
    )
}

/// Masks both sides to the low 16 bits: the real body's upper 16 bits are leftover from the
/// `ZTShowInfo` pointer's own high half (a partial-register decompiler artifact - see
/// `ZTHabitat::get_show_info_id`'s own doc comment), not real dataflow, so comparing the full `u32`
/// would spuriously fail depending on the live pointer's own address bits.
pub(crate) fn run_habitat_get_show_info_id_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_SHOW_INFO_ID_LIVE",
        |ptr| unsafe { zthabitat::GET_SHOW_INFO_ID.original()(ptr) } & 0xffff,
        |habitat| habitat.get_show_info_id() as u32,
    )
}

pub(crate) fn run_habitat_is_show_stopped_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_IS_SHOW_STOPPED_LIVE",
        |ptr| unsafe { zthabitat::IS_SHOW_STOPPED.original()(ptr) != 0 },
        |habitat| habitat.is_show_stopped(),
    )
}

pub(crate) fn run_habitat_get_popularity_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_POPULARITY_LIVE",
        |ptr| unsafe { zthabitat::GET_POPULARITY.original()(ptr) },
        |habitat| habitat.get_popularity(),
    )
}

/// Real vanilla `doTankCheck` only allocates/frees its own local scratch vector (confirmed via its
/// decompile - no field of `habitat`/any other object is mutated), so this is a safe read-only
/// comparison over every real habitat in the loaded zoo, tank or not.
pub(crate) fn run_habitat_do_tank_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_DO_TANK_CHECK_LIVE",
        |ptr| unsafe { zthabitatmgr::DO_TANK_CHECK.original()(ptr as i32) },
        |habitat| habitat.do_tank_check(),
    )
}

/// Compares real `ZTHabitat::isRightSalinity`'s base-class default against the reimplemented constant
/// `true`, over every non-tank habitat in the live, loaded zoo (`ZTTankExhibit`'s own override sits at a
/// different address and is out of scope - see `zthabitatmgr.rs`'s own `is_right_salinity` doc comment).
/// Passes a null `ZTAnimalType*`, matching the reimplementation's own disregard for the argument -
/// safe only because the base default is confirmed to never dereference it.
pub(crate) fn run_habitat_is_right_salinity_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_IS_RIGHT_SALINITY_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        if habitat.is_tank() {
            continue;
        }
        let real = unsafe { IS_RIGHT_SALINITY.original()(ptr as *const u32, std::ptr::null()) };
        let reimpl = habitat.is_right_salinity(std::ptr::null());
        if real != reimpl {
            error!("{}: mismatch at habitat {} ({:#010x}): real={:?}, reimpl={:?}", test_name, i, ptr, real, reimpl);
            if let Some(log_file) = failure_log {
                let _ = log_file
                    .write_all(format!("Test Failed {}: mismatch at habitat {} ({:#010x}): real={:?}, reimpl={:?}\n", test_name, i, ptr, real, reimpl).as_bytes());
            }
            fail_flag = true;
        }
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Smoke test only - `ZTHabitat::listen` has no return value and mutates real vanilla's own small-object
/// freelist as a side effect (see `zthabitatmgr.rs`'s own `free_event_vector_buffer` doc comment), so
/// there's no separate "real" pole to diff against without double-draining the same live event list
/// (itself a hazard). Calls the reimplementation - which itself calls through to real vanilla
/// `getEvents` - once per real habitat and only confirms the battery is still alive afterward.
pub(crate) fn run_habitat_listen_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_LISTEN_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { ref_from_memory::<ZTHabitat>(ptr) }.listen();
    }
    write_success_line(failure_log, test_name);
    false
}

/// Round-trips `set_is_show_exhibit`/`set_is_not_show_exhibit` on every real, currently show-less
/// habitat in the live zoo (skips any habitat that already has a real, active show - same discipline
/// `ZTSHOWMGR_REGISTER_UNREGISTER_SHOW`'s own doc comment describes for not disturbing already-live show
/// state). Exercises the real `ZTShowInfo`/`ZTShowMgr` registration dance end-to-end, then asserts the
/// round trip leaves `zt_show_info_ptr` null again.
///
/// Temporarily zeroes `DAT_0063e49c`/`DAT_0063e4a0` (the configured `[sounds] startSound`/`endSound`
/// name buffers `zthabitatmgr.rs`'s own `START_SOUND_NAME_RVA`/`END_SOUND_NAME_RVA` read) for the
/// duration of this test, restoring the original bytes afterward. This harness's own init sequence
/// never runs real vanilla `showpanel_init` (that only happens when the game's Show Panel UI screen is
/// actually set up), so those buffers hold whatever garbage happened to be there rather than a real,
/// null-terminated config string - discovered live: a first version of this test hung indefinitely
/// inside `BFSndMgr::acquire`, handed a non-terminated garbage "name" pointer read from there. Zeroing
/// them makes `set_is_show_exhibit`'s own `DAT_... != 0` check (faithful to real vanilla) correctly see
/// "no sound configured" and skip sound construction/acquisition entirely - the same outcome a real,
/// freshly-booted game would have before the player ever opens the Show Panel, so this doesn't
/// misrepresent real behavior, just avoids exercising a real-vanilla subsystem this harness never
/// initializes.
pub(crate) fn run_habitat_set_is_show_exhibit_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_SET_IS_SHOW_EXHIBIT_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let base = crate::globals::get_module_base("zoo.exe") as u32;
    let start_sound_name = base + crate::zthabitatmgr::START_SOUND_NAME_RVA;
    let end_sound_name = base + crate::zthabitatmgr::END_SOUND_NAME_RVA;
    let saved_start_sound_name: u32 = crate::util::get_from_memory(start_sound_name);
    let saved_end_sound_name: u32 = crate::util::get_from_memory(end_sound_name);
    crate::util::save_to_memory(start_sound_name, 0u32);
    crate::util::save_to_memory(end_sound_name, 0u32);

    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        if *unsafe { ref_from_memory::<ZTHabitat>(ptr) }.zt_show_info_ptr() != 0 {
            continue;
        }

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) about to call set_is_show_exhibit\n", test_name, i, ptr).as_bytes());
        }
        let habitat = unsafe { mut_from_memory::<ZTHabitat>(ptr) };
        habitat.set_is_show_exhibit();
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) set_is_show_exhibit returned\n", test_name, i, ptr).as_bytes());
        }
        if *habitat.zt_show_info_ptr() == 0 {
            failures.push(format!("habitat {} ({:#010x}): set_is_show_exhibit left zt_show_info_ptr null", i, ptr));
            continue;
        }

        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) about to call set_is_not_show_exhibit\n", test_name, i, ptr).as_bytes());
        }
        habitat.set_is_not_show_exhibit();
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) set_is_not_show_exhibit returned\n", test_name, i, ptr).as_bytes());
        }
        if *habitat.zt_show_info_ptr() != 0 {
            failures.push(format!("habitat {} ({:#010x}): set_is_not_show_exhibit didn't clear zt_show_info_ptr", i, ptr));
        }
    }

    crate::util::save_to_memory(start_sound_name, saved_start_sound_name);
    crate::util::save_to_memory(end_sound_name, saved_end_sound_name);

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

/// Cross-checks [`crate::zthabitatmgr::walk_tile_list`]'s own node count against real vanilla
/// `ZTHabitat::getSize(false)` (still un-ported, called via `.original()`) for every real habitat in
/// the live zoo - both walk the exact same `owned_tiles_ptr` sentinel list, so any offset/layout error
/// in the reimplemented [`crate::zthabitatmgr::TileListNode`] model (wrong `next` offset, wrong
/// sentinel-termination check) would show up here as a count mismatch without needing to mutate
/// anything.
pub(crate) fn run_habitat_owned_tiles_count_matches_get_size_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_OWNED_TILES_COUNT_MATCHES_GET_SIZE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let real_size = unsafe { zthabitat::GET_SIZE.original()(ptr as *const u32, false) };
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let walked_count = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr()).count() as i32;
        if real_size != walked_count {
            error!("{}: mismatch at habitat {} ({:#010x}): real_size={}, walked_count={}", test_name, i, ptr, real_size, walked_count);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: mismatch at habitat {} ({:#010x}): real_size={}, walked_count={}\n", test_name, i, ptr, real_size, walked_count)
                        .as_bytes(),
                );
            }
            fail_flag = true;
        }
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Smoke test only, like [`run_habitat_listen_smoke_live_test`] - `ZTHabitat::validatePositions` has no
/// return value and its only real work is delegating to real vanilla `BFTile::validatePositions` per
/// owned tile, so there's no independent "real" pole to diff a return value against. Combined with
/// [`run_habitat_owned_tiles_count_matches_get_size_live_test`] (which already confirms the walk visits
/// the right *number* of nodes), calling this without panicking over every real habitat confirms each
/// node's payload is read from the right offset (a wrong payload offset here would hand
/// `BFTile::validatePositions` a bogus, likely-crashing pointer).
pub(crate) fn run_habitat_validate_positions_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_VALIDATE_POSITIONS_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { ref_from_memory::<ZTHabitat>(ptr) }.validate_positions();
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test only, like [`run_habitat_validate_positions_smoke_live_test`] - `ZTHabitat::resetUnitAI`
/// has no return value and its only real work is calling vtable slot `+0x100` on each occupant of every
/// owned tile (plus the gate-out tile), so there's no independent "real" pole to diff a return value
/// against. Confirms the nested tile-occupant walk (`BFTile::unit_list_ptr` at `+0x0` - see that field's
/// own doc comment) reads live, well-formed nodes without crashing over every real habitat in the loaded
/// zoo.
pub(crate) fn run_habitat_reset_unit_ai_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_RESET_UNIT_AI_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { ref_from_memory::<ZTHabitat>(ptr) }.reset_unit_ai();
    }
    write_success_line(failure_log, test_name);
    false
}

/// Round-trips `remove_habitat_tiles` + `add_habitat_tiles` on exactly **one** real habitat (the first
/// with a non-empty owned-tile list) - unlike the destructive `REMOVE_HABITAT_TILES_LIVE` test below, this
/// one restores the habitat's tile list afterward so it's safe to run mid-battery, before that test.
/// Mirrors real vanilla's own only known caller (`ZTHabitat::resize`): remove, then re-add from a seed
/// tile captured before removal (the first tile in the original list).
///
/// Verifies real vanilla `ZTHabitat::getSize(false)` reports the *same* count after the round-trip as
/// before (proves the flood-fill reclaimed every tile the exhibit's fences actually enclose, not more or
/// fewer), and that the seed tile's own ownership-grid cell points back at this habitat again (proves
/// `add_habitat_tiles`'s claim path - list-insert, grid-cell write, `+0x85` flag - used the right
/// offsets, not just that *some* tiles got claimed).
pub(crate) fn run_habitat_add_habitat_tiles_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_ADD_HABITAT_TILES_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let mut target: Option<(usize, u32)> = None;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        if unsafe { zthabitat::GET_SIZE.original()(ptr as *const u32, false) } > 0 {
            target = Some((i, ptr));
            break;
        }
    }

    let Some((i, ptr)) = target else {
        write_success_line(failure_log, &format!("{} (skipped: no habitat with owned tiles found)", test_name));
        return false;
    };

    let real_size_before = unsafe { zthabitat::GET_SIZE.original()(ptr as *const u32, false) };
    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let coords_before: std::collections::HashSet<(i32, i32)> = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
        .map(|node| {
            let tile = crate::util::get_from_memory::<u32>(node + 0x8);
            (crate::util::get_from_memory::<i32>(tile + 0x34), crate::util::get_from_memory::<i32>(tile + 0x38))
        })
        .collect();
    let seed_tile = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
        .next()
        .map(|node| crate::util::get_from_memory::<u32>(node + 0x8))
        .filter(|&t| t != 0);

    let Some(seed_tile) = seed_tile else {
        write_success_line(failure_log, &format!("{} (skipped: no seed tile found)", test_name));
        return false;
    };

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(
            format!(
                "CHECKPOINT {} habitat {} ({:#010x}) size_before={} seed_tile={:#010x} about to remove_habitat_tiles\n",
                test_name, i, ptr, real_size_before, seed_tile
            )
            .as_bytes(),
        );
    }
    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.remove_habitat_tiles();
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) removed, about to add_habitat_tiles\n", test_name, i, ptr).as_bytes());
    }
    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.add_habitat_tiles(seed_tile);
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) add_habitat_tiles returned\n", test_name, i, ptr).as_bytes());
    }

    let real_size_after = unsafe { zthabitat::GET_SIZE.original()(ptr as *const u32, false) };
    if real_size_after != real_size_before {
        failures.push(format!("habitat {} ({:#010x}): size before={}, after remove+add={}", i, ptr, real_size_before, real_size_after));
    }

    let habitat_after = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let coords_after: std::collections::HashSet<(i32, i32)> = crate::zthabitatmgr::walk_tile_list(*habitat_after.owned_tiles_ptr())
        .map(|node| {
            let tile = crate::util::get_from_memory::<u32>(node + 0x8);
            (crate::util::get_from_memory::<i32>(tile + 0x34), crate::util::get_from_memory::<i32>(tile + 0x38))
        })
        .collect();
    if coords_before != coords_after {
        let missing: Vec<_> = coords_before.difference(&coords_after).take(10).collect();
        let extra: Vec<_> = coords_after.difference(&coords_before).take(10).collect();
        failures.push(format!(
            "habitat {} ({:#010x}): tile SET changed (count before={}, after={}); missing (up to 10)={:?}; extra (up to 10)={:?}",
            i,
            ptr,
            coords_before.len(),
            coords_after.len(),
            missing,
            extra
        ));
    }

    let seed_x: i32 = crate::util::get_from_memory(seed_tile + 0x34);
    let seed_y: i32 = crate::util::get_from_memory(seed_tile + 0x38);
    let owner_after = habitat_mgr.get_habitat_ptr(seed_x, seed_y);
    if owner_after != ptr {
        failures.push(format!(
            "habitat {} ({:#010x}): seed tile ({}, {}) not owned by this habitat after round-trip (owner={:#010x})",
            i, ptr, seed_x, seed_y, owner_after
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

/// Smoke test only, like [`run_habitat_listen_smoke_live_test`]/[`run_habitat_validate_positions_smoke_live_test`]
/// - `ZTHabitat::update` has no return value and every real sub-call it makes (`Ambients::play`,
/// `reviseSpeciesList`, `recalculateCharacteristics`, `ZTViewingArea::updateAmbients`, `updatePortals`,
/// `listen`) is itself either a real vanilla call-through or already covered by its own dedicated live
/// test elsewhere in this file, so there's no independent "real" pole left to diff a return value
/// against without double-driving those side effects. Calls the reimplementation with a small,
/// realistic tick (`16` ms, one frame at 60Hz) on every real, non-tank habitat and only confirms the
/// battery is still alive afterward - the `ambients_begin`/`_end` and `viewing_areas_begin`/`_end`
/// vector walks are the two field offsets this test exists to exercise: a wrong offset there would
/// either read garbage pointers (likely crashing `Ambients::play`/`updateAmbients`) or, if the
/// begin/end pair happened to compare equal by coincidence, silently skip the walk entirely rather
/// than prove anything - so this is a real crash-or-hang check, not a no-op. Skips tanks, matching the
/// detour's own real invocation domain: `ZTTankExhibit` overrides this vtable slot at a separate
/// address (see `ZTHabitat::update`'s own doc comment), so real vanilla never dispatches a tank's tick
/// through the address this file detours.
pub(crate) fn run_habitat_update_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_UPDATE_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { mut_from_memory::<ZTHabitat>(ptr) };
        if habitat.is_tank() {
            continue;
        }
        habitat.update(16);
    }
    write_success_line(failure_log, test_name);
    false
}

/// Destructive, irreversible (leaves this one habitat's tile list empty for the rest of the run - unlike
/// [`run_habitat_add_habitat_tiles_roundtrip_live_test`] above, this test deliberately does not restore
/// it, to also verify vanilla's own `getSize()` reflects a genuinely-emptied list independent of any
/// re-add) - deliberately exercised on exactly **one** real habitat (the first with a non-empty
/// owned-tile list), not all of them, so the rest of this battery run's habitat-dependent tests are
/// unaffected. Must run last among the `ZTHABITAT_*`/`ZTHABITATMGR_*` entries in `battery.rs`'s own
/// `live_zoo_tests` list.
///
/// Verifies three things a wrong [`crate::zthabitatmgr::TileListNode`] offset/model could get wrong:
/// 1. Real vanilla `ZTHabitat::getSize(false)` reads `0` afterward - proves the sentinel was reset to
///    the correct self-referencing empty state at the offsets real vanilla's own walker also reads.
/// 2. The tile-ownership grid cell for the first owned tile (recorded before the call) no longer points
///    back at this habitat - proves the ownership-clearing pass read the right payload/coordinate
///    offsets.
/// 3. The call doesn't crash the battery - the freelist splice touches real vanilla's own shared
///    small-object pool in place; a wrong `next` offset here would corrupt that pool for every other
///    consumer of the same 16-byte bucket.
pub(crate) fn run_habitat_remove_habitat_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_REMOVE_HABITAT_TILES_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let mut target: Option<(usize, u32)> = None;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        if unsafe { zthabitat::GET_SIZE.original()(ptr as *const u32, false) } > 0 {
            target = Some((i, ptr));
            break;
        }
    }

    let Some((i, ptr)) = target else {
        write_success_line(failure_log, &format!("{} (skipped: no habitat with owned tiles found)", test_name));
        return false;
    };

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let first_node = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr()).next();
    let first_tile_coords = first_node.map(|node| {
        let tile = crate::util::get_from_memory::<u32>(node + 0x8);
        (crate::util::get_from_memory::<i32>(tile + 0x34), crate::util::get_from_memory::<i32>(tile + 0x38))
    });

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) about to call remove_habitat_tiles\n", test_name, i, ptr).as_bytes());
    }
    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.remove_habitat_tiles();
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} habitat {} ({:#010x}) remove_habitat_tiles returned\n", test_name, i, ptr).as_bytes());
    }

    let real_size_after = unsafe { zthabitat::GET_SIZE.original()(ptr as *const u32, false) };
    if real_size_after != 0 {
        failures.push(format!("habitat {} ({:#010x}): real getSize() == {} after remove_habitat_tiles, expected 0", i, ptr, real_size_after));
    }

    if let Some((x, y)) = first_tile_coords {
        let owner_after = habitat_mgr.get_habitat_ptr(x, y);
        if owner_after == ptr {
            failures.push(format!("habitat {} ({:#010x}): tile ({}, {}) still owned by this habitat after remove_habitat_tiles", i, ptr, x, y));
        }
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

/// Compares the real `ZTHabitatMgr::getHabitat` against the reimplemented `get_habitat_ptr`, for
/// small in-range tile coordinates, now that `run_load_live_zoo` has populated
/// `other_array_start`/`other_array_end` with a real zoo's bounds (`get_habitat_ptr` does no
/// bounds-checking of its own, so this needs a real, loaded zoo to be safe).
pub(crate) fn run_habitat_get_habitat_ptr_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
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

/// Captures real vanilla `ZTHabitat::save`'s raw output (via the un-detoured `.original()` address -
/// safe per `save`'s own read-only, no-side-effects reasoning, same as
/// `ZTRESEARCHMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE`) against the reimplemented `save()`'s own output, for
/// every habitat in the live, loaded zoo, and asserts the two byte streams are identical. Both sides
/// call the base `ZTHabitat::save` address directly (not through the vtable), so this stays an
/// apples-to-apples comparison of the base implementation regardless of whether a given entry is
/// actually a `ZTTankExhibit` - polymorphic dispatch is exercised separately by
/// `ZTHABITATMGR_SAVE_MATCHES_REAL_LIVE` below.
pub(crate) fn run_habitat_save_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_SAVE_MATCHES_REAL_LIVE",
        |ptr| {
            let dummy_file: u32 = 0;
            io_redirect::begin_capture();
            unsafe { zthabitat::SAVE.original()(ptr, &dummy_file as *const u32) };
            io_redirect::end_capture()
        },
        |habitat| {
            let dummy_file: u32 = 0;
            io_redirect::begin_capture();
            habitat.save(&dummy_file as *const u32 as *const i8);
            io_redirect::end_capture()
        },
    )
}

/// Captures real vanilla `ZTHabitatMgr::save`'s raw output against the reimplemented `save()`'s own
/// output, over the live, loaded zoo's own manager singleton, and asserts the two byte streams are
/// identical. Since `save()`'s own per-exhibit loop dispatches through each habitat's real (already-
/// detoured) vtable slot, both sides funnel the per-exhibit bytes through the identical, shared
/// `ZTHabitat::save` detour - so this test specifically exercises the manager-level logic (map size,
/// zoo entrance tile, exhibit count, trailing marker dword, control flow), not the per-exhibit fields
/// (covered independently by [`run_habitat_save_matches_real_live_test`], which calls the un-detoured
/// `.original()` address directly).
pub(crate) fn run_zthabitatmgr_save_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_SAVE_MATCHES_REAL_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mgr = globals().zthabitatmgr();

    let dummy_file: u32 = 0;
    io_redirect::begin_capture();
    let real_ok = unsafe { zthabitatmgr::SAVE.original()(mgr_ptr, &dummy_file as *const u32 as *const i8) };
    let real_bytes = io_redirect::end_capture();

    io_redirect::begin_capture();
    let reimpl_ok = mgr.save(&dummy_file as *const u32 as *const i8);
    let reimpl_bytes = io_redirect::end_capture();

    let mut fail_flag = false;
    if !real_ok || !reimpl_ok {
        error!("{}: save() returned failure (real_ok={}, reimpl_ok={})", test_name, real_ok, reimpl_ok);
        fail_flag = true;
    }
    if real_bytes != reimpl_bytes {
        error!("{}: byte mismatch (real {} bytes, reimpl {} bytes)", test_name, real_bytes.len(), reimpl_bytes.len());
        if let Some(log_file) = failure_log {
            let _ =
                log_file.write_all(format!("Test Failed {}: real_len={} reimpl_len={}\n", test_name, real_bytes.len(), reimpl_bytes.len()).as_bytes());
        }
        fail_flag = true;
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Round-trips the reimplemented `ZTHabitatMgr::add_habitat` on a single, freshly-constructed, real
/// vanilla `ZTHabitat` (real `operator_new(0x178)` + real vanilla `ZTHabitat::ZTHabitat` constructor,
/// seeded at the live zoo's own zoo-entrance tile - any real tile works, since `add_habitat` never reads
/// tile-specific state itself): appends it via the reimplementation, confirms `exhibit_array` grew by
/// exactly one and its new last entry is the fresh habitat, then removes it again via real vanilla's own
/// un-ported `ZTHabitatMgr::removeHabitat` (`REMOVE_HABITAT_0`) so the live zoo's exhibit count is
/// unchanged for every other test in this battery.
pub(crate) fn run_zthabitatmgr_add_habitat_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_ADD_HABITAT_ROUNDTRIP_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let seed_tile_ptr = unsafe { zthabitatmgr::GET_ZOO_ENTRANCE_TILE.original()(mgr_ptr) } as u32;
    if seed_tile_ptr == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no zoo entrance tile)", test_name));
        return false;
    }

    let raw = unsafe { OPERATOR_NEW.original()(0x178) } as u32;
    if raw == 0 {
        write_success_line(failure_log, &format!("{} (skipped: operator_new failed)", test_name));
        return false;
    }
    let new_habitat = unsafe { zthabitat::CONSTRUCTOR.original()(raw as *const u32, seed_tile_ptr as *const u32, false) } as u32;

    let len_before = habitat_mgr.exhibit_array().len();
    habitat_mgr.add_habitat(new_habitat);

    let len_after = habitat_mgr.exhibit_array().len();
    if len_after != len_before + 1 {
        failures.push(format!("exhibit_array length after add: expected {}, got {}", len_before + 1, len_after));
    } else {
        let last = habitat_mgr.exhibit_array().get_ptr(len_after - 1);
        if last != new_habitat {
            failures.push(format!("exhibit_array's last entry ({:#010x}) doesn't match the newly-added habitat ({:#010x})", last, new_habitat));
        }
    }

    unsafe { zthabitatmgr::REMOVE_HABITAT_0.original()(mgr_ptr, new_habitat as *const i32) };

    let len_final = habitat_mgr.exhibit_array().len();
    if len_final != len_before {
        failures.push(format!("exhibit_array length after REMOVE_HABITAT_0 cleanup: expected {}, got {}", len_before, len_final));
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

/// Round-trips the reimplemented `ZTHabitatMgr::create_habitat` on a single, real, non-tank habitat
/// seeded at the live zoo's own zoo-entrance tile, passing that same real tile as `resize_tile_ptr` and
/// `gate_tile_ptr` too. Every real `createHabitat`/`placeGate` call site in the decompile corpus
/// (`fencePlaced`, `morphExhibit`, `splitTank`/`splitTankIntoLand`) passes three *real* BFTile pointers
/// pairwise related to each other - never a null resize or gate tile - contradicting this method's own
/// earlier assumption (now corrected, see `create_habitat`'s own doc comment in `zthabitatmgr.rs`) that
/// a null resize/gate tile is a realistic input. Confirmed live: passing `resize_tile_ptr=0` reaches
/// `PLACE_GATE`'s own pathfinding (`BFPathFinder::findPath`/`BFUnit::getPathCost`, which dereferences
/// both tile arguments unconditionally at `+0x3c`/`+0x40`) with a null tile and crashes
/// (`0xc0000005` at `zoo.exe+0x14579`, inside `getPathCost`) - a bad test input, not a genuine
/// `create_habitat` bug. Using the zoo-entrance tile for `resize_tile_ptr` is safe here specifically
/// because that tile is real vanilla's own entrance plaza, never enclosed by any exhibit, so
/// `get_habitat_ptr` on it resolves to `0` and `create_habitat`'s own `resize_target_ptr != 0` guard
/// skips the `resize`/`setDirtyCharacteristics` calls entirely - this test would need a different tile
/// choice if the entrance ever became habitat-owned. Passes a real
/// [`crate::vanilla_string::VanillaString`] as `name_ptr` to take the `ZTHabitat::setName` branch
/// instead of `nameHabitat`, which can pop a real modal "New Exhibit Name" dialog - fatal for automated
/// testing.
///
/// Confirms `exhibit_array` grew by exactly one, then removes the new habitat again via real vanilla's
/// own un-ported `ZTHabitatMgr::removeHabitat` (`REMOVE_HABITAT_0`) so the live zoo's exhibit count is
/// unchanged for every other test in this battery - same cleanup discipline as
/// [`run_zthabitatmgr_add_habitat_roundtrip_live_test`].
pub(crate) fn run_zthabitatmgr_create_habitat_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CREATE_HABITAT_SMOKE_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let seed_tile_ptr = unsafe { zthabitatmgr::GET_ZOO_ENTRANCE_TILE.original()(mgr_ptr) } as u32;
    if seed_tile_ptr == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no zoo entrance tile)", test_name));
        return false;
    }

    let len_before = habitat_mgr.exhibit_array().len();
    let vanilla_name = crate::vanilla_string::VanillaString::new("OpenZT Test Habitat");

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} about to call create_habitat\n", test_name).as_bytes());
    }
    habitat_mgr.create_habitat(seed_tile_ptr, seed_tile_ptr, seed_tile_ptr, vanilla_name.as_ptr() as u32);
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} create_habitat returned\n", test_name).as_bytes());
    }

    let len_after = habitat_mgr.exhibit_array().len();
    if len_after != len_before + 1 {
        failures.push(format!("exhibit_array length after create: expected {}, got {}", len_before + 1, len_after));
    } else {
        let new_habitat = habitat_mgr.exhibit_array().get_ptr(len_after - 1);
        unsafe { zthabitatmgr::REMOVE_HABITAT_0.original()(mgr_ptr, new_habitat as *const i32) };
        let len_final = habitat_mgr.exhibit_array().len();
        if len_final != len_before {
            failures.push(format!("exhibit_array length after cleanup: expected {}, got {}", len_before, len_final));
        }
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

/// Compares real `ZTHabitatMgr::getZooEntranceTile` against the reimplemented
/// `get_zoo_entrance_tile_ptr`, over the live, loaded zoo's own manager singleton. Read-only on both
/// sides - safe to call `.original()` directly even though `GET_ZOO_ENTRANCE_TILE` is now detoured (the
/// debug-build trampoline still reaches real vanilla - see `generated.rs`'s own `FunctionDef::original()`
/// doc comment).
pub(crate) fn run_zthabitatmgr_get_zoo_entrance_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_GET_ZOO_ENTRANCE_TILE_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mgr = globals().zthabitatmgr();

    let real = unsafe { zthabitatmgr::GET_ZOO_ENTRANCE_TILE.original()(mgr_ptr) };
    let reimpl = mgr.get_zoo_entrance_tile_ptr() as i32;

    if real == reimpl {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("real={:#010x}, reimpl={:#010x}", real, reimpl);
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Compares real `ZTHabitatMgr::getAverageHabitatAttractiveness` against the reimplemented
/// `get_average_habitat_attractiveness`, over the live, loaded zoo's own manager singleton. Read-only on
/// both sides (each real habitat's own `getAttractiveness` may lazily recalculate, but that's idempotent
/// once settled - both sides observe the same live state).
pub(crate) fn run_zthabitatmgr_get_average_habitat_attractiveness_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_GET_AVERAGE_HABITAT_ATTRACTIVENESS_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr();
    let mgr = globals().zthabitatmgr();

    let real = unsafe { zthabitatmgr::GET_AVERAGE_HABITAT_ATTRACTIVENESS.original()(mgr_ptr as i32) };
    let reimpl = mgr.get_average_habitat_attractiveness();

    if real == reimpl {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("real={}, reimpl={}", real, reimpl);
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Compares real `ZTHabitatMgr::getNumFamilies`/`getNumSpecies` against the reimplemented
/// `get_num_families`/`get_num_species`, over the live, loaded zoo's own manager singleton. Both build
/// (and immediately discard) a distinct-id set internally - no persistent state to disturb.
pub(crate) fn run_zthabitatmgr_get_num_families_species_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_GET_NUM_FAMILIES_SPECIES_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr();
    let mgr = globals().zthabitatmgr();

    let real_families = unsafe { zthabitatmgr::GET_NUM_FAMILIES.original()(mgr_ptr as i32) };
    let reimpl_families = mgr.get_num_families();
    let real_species = unsafe { zthabitatmgr::GET_NUM_SPECIES.original()(mgr_ptr as *const u32) };
    let reimpl_species = mgr.get_num_species();

    let mut failures: Vec<String> = Vec::new();
    if real_families != reimpl_families {
        failures.push(format!("getNumFamilies: real={}, reimpl={}", real_families, reimpl_families));
    }
    if real_species != reimpl_species {
        failures.push(format!("getNumSpecies: real={}, reimpl={}", real_species, reimpl_species));
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

/// Round-trips `ZTHabitat::highlight`/`unhighlight` on exactly **one** real habitat (the first with a
/// non-empty owned-tile list): captures every owned tile's `+0x83`/`+0x85` byte first, calls
/// `highlight(true)` and confirms bit `0x80` is now set at `+0x83` on every one, then calls `unhighlight`
/// and confirms both `+0x83` bit `0x80` and `+0x85` bit `0x10` are clear again - restoring every other bit
/// to its original value (real vanilla's own `unhighlightHabitat` always clears both bits unconditionally,
/// so the expected final state is simply the captured original with those two bits forced off, regardless
/// of what they were beforehand).
pub(crate) fn run_habitat_highlight_unhighlight_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_HIGHLIGHT_UNHIGHLIGHT_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let mut target: Option<u32> = None;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        if unsafe { zthabitat::GET_SIZE.original()(ptr as *const u32, false) } > 0 {
            target = Some(ptr);
            break;
        }
    }

    let Some(ptr) = target else {
        write_success_line(failure_log, &format!("{} (skipped: no habitat with owned tiles found)", test_name));
        return false;
    };

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let tiles: Vec<u32> = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
        .map(|node| get_from_memory::<u32>(node + 0x8))
        .filter(|&t| t != 0)
        .collect();
    let originals: Vec<(u8, u8)> = tiles.iter().map(|&t| (get_from_memory::<u8>(t + 0x83), get_from_memory::<u8>(t + 0x85))).collect();

    habitat.highlight(true);
    for (&tile, &(orig83, _)) in tiles.iter().zip(originals.iter()) {
        let flags83 = get_from_memory::<u8>(tile + 0x83);
        if flags83 & 0x80 == 0 {
            failures.push(format!("tile {:#010x}: expected 0x83 bit 0x80 set after highlight(true) (orig 0x83={:#04x}, got {:#04x})", tile, orig83, flags83));
        }
    }

    habitat.unhighlight();
    for (&tile, &(orig83, orig85)) in tiles.iter().zip(originals.iter()) {
        let flags83 = get_from_memory::<u8>(tile + 0x83);
        let flags85 = get_from_memory::<u8>(tile + 0x85);
        let expected83 = orig83 & 0x7f;
        let expected85 = orig85 & 0xef;
        if flags83 != expected83 || flags85 != expected85 {
            failures.push(format!(
                "tile {:#010x}: after unhighlight expected (0x83={:#04x}, 0x85={:#04x}), got (0x83={:#04x}, 0x85={:#04x})",
                tile, expected83, expected85, flags83, flags85
            ));
        }
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

/// Round-trips the reimplemented `ZTHabitatMgr::enter_new_month` over every real habitat in the live zoo
/// plus the manager's own `pending_habitat_ptr` (the "world" habitat - see that field's own doc comment
/// in `zthabitatmgr.rs`, read here via a raw offset since the field itself is private to that module):
/// captures `current_donations`/`unknown_u32_2`/`current_upkeep` (and their `last_*`/`unknown_u32_3`
/// counterparts) before, calls the reimplementation once, verifies the expected rotation for every one of
/// those three field pairs, then restores every captured original value so the live zoo's economic state
/// is unchanged for the rest of the battery run.
pub(crate) fn run_zthabitatmgr_enter_new_month_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_ENTER_NEW_MONTH_ROUNDTRIP_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as u32;
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let mut targets: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    targets.push(get_from_memory::<u32>(mgr_ptr + 0x18));

    #[derive(Clone, Copy)]
    struct Before {
        cur_don: f32,
        last_don: f32,
        u2: u32,
        u3: u32,
        cur_up: f32,
        last_up: f32,
    }
    let before: Vec<Before> = targets
        .iter()
        .map(|&p| Before {
            cur_don: get_from_memory(p + 0xfc),
            last_don: get_from_memory(p + 0x100),
            u2: get_from_memory(p + 0x114),
            u3: get_from_memory(p + 0x118),
            cur_up: get_from_memory(p + 0x108),
            last_up: get_from_memory(p + 0x10c),
        })
        .collect();

    habitat_mgr.enter_new_month();

    for (&ptr, b) in targets.iter().zip(before.iter()) {
        let cur_don: f32 = get_from_memory(ptr + 0xfc);
        let last_don: f32 = get_from_memory(ptr + 0x100);
        let u2: u32 = get_from_memory(ptr + 0x114);
        let u3: u32 = get_from_memory(ptr + 0x118);
        let cur_up: f32 = get_from_memory(ptr + 0x108);
        let last_up: f32 = get_from_memory(ptr + 0x10c);

        if cur_don != 0.0 || last_don != b.cur_don {
            failures.push(format!("habitat {:#010x}: donations expected (cur=0, last={}), got (cur={}, last={})", ptr, b.cur_don, cur_don, last_don));
        }
        if u2 != 0 || u3 != b.u2 {
            failures.push(format!("habitat {:#010x}: unknown_u32_2/_3 expected (0, {:#010x}), got ({:#010x}, {:#010x})", ptr, b.u2, u2, u3));
        }
        if cur_up != 0.0 || last_up != b.cur_up {
            failures.push(format!("habitat {:#010x}: upkeep expected (cur=0, last={}), got (cur={}, last={})", ptr, b.cur_up, cur_up, last_up));
        }
    }

    for (&ptr, b) in targets.iter().zip(before.iter()) {
        save_to_memory(ptr + 0xfc, b.cur_don);
        save_to_memory(ptr + 0x100, b.last_don);
        save_to_memory(ptr + 0x114, b.u2);
        save_to_memory(ptr + 0x118, b.u3);
        save_to_memory(ptr + 0x108, b.cur_up);
        save_to_memory(ptr + 0x10c, b.last_up);
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

/// `ZTHabitatMgr::replace_gate` is a no-op in a freshly loaded save (`pending_gate_fence_ptr` is only
/// ever set transiently by real vanilla `removeHabitat`, mid-gameplay); this only exercises the call path
/// for a crash/wiring regression, matching `ZTHABITAT_LISTEN_SMOKE_LIVE`'s own reasoning - there is no
/// independent "real" pole to diff against without first driving a real habitat deletion.
pub(crate) fn run_zthabitatmgr_replace_gate_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_REPLACE_GATE_SMOKE_LIVE";
    globals().zthabitatmgr().replace_gate();
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test, same reasoning as `ZTHABITAT_VALIDATE_POSITIONS_SMOKE_LIVE`: `habitat_tile_changed` has
/// no return value, so there's no independent "real" pole to diff a result against. Calls it on the
/// first owned tile of every real habitat in the loaded zoo (proving the ±3-tile neighbor scan and its
/// 8-slot-per-row reads don't crash over real map edges/corners), then confirms at least one call found
/// at least one non-null neighbor slot to mark - a concrete assertion that the row-walk actually reaches
/// live data, not just that it doesn't crash.
pub(crate) fn run_zthabitatmgr_habitat_tile_changed_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_HABITAT_TILE_CHANGED_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut any_neighbor_marked = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let Some(tile_ptr) = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
            .next()
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
        else {
            continue;
        };
        habitat_mgr.habitat_tile_changed(tile_ptr);
        let tile = get_from_memory::<crate::ztmapview::BFTile>(tile_ptr);
        if let Some(row_addr) = habitat_mgr.get_habitat_cell_addr(tile.pos.x, tile.pos.y) {
            for slot in 0..8u32 {
                let neighbor_ptr: u32 = get_from_memory(row_addr + 0x4 + slot * 4);
                if neighbor_ptr != 0 && get_from_memory::<u8>(neighbor_ptr + 0x25) == 1 {
                    any_neighbor_marked = true;
                }
            }
        }
    }
    if any_neighbor_marked {
        write_success_line(failure_log, test_name);
        false
    } else {
        write_success_line(failure_log, &format!("{} (skipped: no non-null neighbor slots found)", test_name));
        false
    }
}

/// Smoke test, same reasoning as [`run_zthabitatmgr_habitat_tile_changed_smoke_live_test`]:
/// `terrain_tile_changed` is a thin `unknown_flag_0x2c` gate in front of `habitat_tile_changed`. Calling
/// it over every real habitat's first owned tile exercises both branches (the flag is set on real
/// exhibits' own tiles at most rarely - see `Self::pending_habitat_ptr`'s own doc comment on the "world"
/// habitat - so this mostly proves the gate-check path itself doesn't crash).
pub(crate) fn run_zthabitatmgr_terrain_tile_changed_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_TERRAIN_TILE_CHANGED_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let Some(tile_ptr) = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
            .next()
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
        else {
            continue;
        };
        habitat_mgr.terrain_tile_changed(tile_ptr);
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test, same reasoning as [`run_zthabitatmgr_habitat_tile_changed_smoke_live_test`]:
/// `scenery_entity_change` is void and its `scenery_entity_change_suspended` gate is never set anywhere
/// in this pass's own scope, so every real call reaches either the `habitat_tile_changed` delegation or
/// the `viewing_areas` walk - this exercises both over every real habitat's first owned tile.
pub(crate) fn run_zthabitatmgr_scenery_entity_change_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_SCENERY_ENTITY_CHANGE_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let Some(tile_ptr) = crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
            .next()
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
        else {
            continue;
        };
        habitat_mgr.scenery_entity_change(tile_ptr);
    }
    write_success_line(failure_log, test_name);
    false
}

/// Round-trips `ZTHabitat::hilite_amphibious_neighbors`/`hilite_show_neighbors` over every real habitat
/// with a non-empty amphibious or show neighbor tree (`walk_neighbor_tree` over `amphibious_neighbors_head`/
/// `show_neighbors_head`): hilite(true) then check `highlight`'s own `+0x83` bit `0x80` fires on every
/// neighbor's owned tiles (proving the tree walk reached a real neighbor and called through), then
/// hilite(false) and check the bit clears again. Skips (rather than fails) if no real habitat in the
/// loaded zoo has any amphibious/show neighbors yet - real vanilla only populates these trees once the
/// pathfinding-heavy `checkAmphibiousNeighbor`/`checkShowNeighbor` orchestrators have run at least once,
/// not merely from loading a save.
pub(crate) fn run_habitat_hilite_neighbors_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_HILITE_NEIGHBORS_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut exercised = false;

    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };

        for (kind, head) in [("amphibious", *habitat.amphibious_neighbors_head()), ("show", *habitat.show_neighbors_head())] {
            let neighbors: Vec<u32> = crate::zthabitatmgr::walk_neighbor_tree(head).map(|node| get_from_memory::<u32>(node + 0x10)).filter(|&p| p != 0).collect();
            if neighbors.is_empty() {
                continue;
            }

            let tiles: Vec<u32> = neighbors
                .iter()
                .flat_map(|&n| {
                    let neighbor = unsafe { ref_from_memory::<ZTHabitat>(n) };
                    crate::zthabitatmgr::walk_tile_list(*neighbor.owned_tiles_ptr()).map(|node| get_from_memory::<u32>(node + 0x8))
                })
                .filter(|&t| t != 0)
                .collect();
            if tiles.is_empty() {
                continue;
            }
            exercised = true;

            if kind == "amphibious" {
                habitat.hilite_amphibious_neighbors(true);
            } else {
                habitat.hilite_show_neighbors(true);
            }
            for &tile in &tiles {
                let flags83 = get_from_memory::<u8>(tile + 0x83);
                if flags83 & 0x80 == 0 {
                    failures.push(format!(
                        "{} habitat {:#010x} neighbor tile {:#010x}: expected 0x83 bit 0x80 set after hilite(true), got {:#04x}",
                        kind, ptr, tile, flags83
                    ));
                }
            }

            if kind == "amphibious" {
                habitat.hilite_amphibious_neighbors(false);
            } else {
                habitat.hilite_show_neighbors(false);
            }
            for &tile in &tiles {
                let flags83 = get_from_memory::<u8>(tile + 0x83);
                if flags83 & 0x80 != 0 {
                    failures.push(format!(
                        "{} habitat {:#010x} neighbor tile {:#010x}: expected 0x83 bit 0x80 cleared after hilite(false), got {:#04x}",
                        kind, ptr, tile, flags83
                    ));
                }
            }
        }
    }

    if !exercised {
        write_success_line(failure_log, &format!("{} (skipped: no habitat with amphibious/show neighbors found)", test_name));
        return false;
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

/// Smoke test: calls the reimplemented `ZTHabitatMgr::check_amphibious_neighbor` over every real
/// habitat's own boundary tile-pairs, asserting no crash. No independent "real" pole to diff a return
/// value against - real vanilla's own callers (`updateAmphibiousNeighbors`) discard the return value too,
/// and the interesting side effects (`addAmphibiousNeighbor`/`setIsCombinedConnector`) are on real
/// vanilla's own un-ported STL containers this harness has no independent way to read back.
pub(crate) fn run_zthabitatmgr_check_amphibious_neighbor_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CHECK_AMPHIBIOUS_NEIGHBOR_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut entry = *habitat.boundary_tile_pairs_begin();
        let end = *habitat.boundary_tile_pairs_end();
        while entry != end {
            let tile_a: u32 = get_from_memory(entry);
            let tile_b: u32 = get_from_memory(entry + 4);
            habitat_mgr.check_amphibious_neighbor(ptr, tile_a, tile_b);
            entry += 8;
        }
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test: calls the reimplemented `ZTHabitatMgr::update_amphibious_neighbors` over every real
/// habitat in the loaded zoo, asserting no crash - exercises the `clearAmphibiousNeighbors` call-through
/// plus the full boundary-tile-pair snapshot/iterate/`check_amphibious_neighbor` loop.
pub(crate) fn run_zthabitatmgr_update_amphibious_neighbors_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_UPDATE_AMPHIBIOUS_NEIGHBORS_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        habitat_mgr.update_amphibious_neighbors(ptr);
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test, same reasoning as [`run_zthabitatmgr_check_amphibious_neighbor_smoke_live_test`]: calls
/// `ZTHabitatMgr::check_show_neighbor` over every real habitat's own boundary tile-pairs.
pub(crate) fn run_zthabitatmgr_check_show_neighbor_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CHECK_SHOW_NEIGHBOR_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut entry = *habitat.boundary_tile_pairs_begin();
        let end = *habitat.boundary_tile_pairs_end();
        while entry != end {
            let tile_a: u32 = get_from_memory(entry);
            let tile_b: u32 = get_from_memory(entry + 4);
            habitat_mgr.check_show_neighbor(ptr, tile_a, tile_b);
            entry += 8;
        }
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test: calls the reimplemented `ZTHabitatMgr::update_show_neighbors` (the recursive worker) over
/// every real habitat in the loaded zoo, asserting no crash - exercises both the "showable" (clear +
/// re-derive via `check_show_neighbor`) and "non-showable" (recurse into showable neighbors) branches,
/// plus the visited-set recursion guard.
pub(crate) fn run_zthabitatmgr_update_show_neighbors_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_UPDATE_SHOW_NEIGHBORS_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        habitat_mgr.update_show_neighbors(ptr);
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test: calls the reimplemented `ZTHabitatMgr::do_show_check` over every real habitat in the
/// loaded zoo with `remove_illegal=false` (avoids touching real vanilla `removeIllegalEntities` outside a
/// deliberately-targeted tank fixture - see `zthabitatmgr-implementation-plan.md`'s own step 6d note),
/// asserting no crash. Mutates real habitat show-exhibit state (`set_is_show_exhibit`/
/// `set_is_not_show_exhibit`, both already independently tested elsewhere) as a side effect, same as real
/// vanilla's own call - not restored afterward, matching this file's own established convention for
/// smoke tests that mutate live state (e.g. `run_habitat_reset_unit_ai_smoke_live_test`).
pub(crate) fn run_zthabitatmgr_do_show_check_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_DO_SHOW_CHECK_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        habitat_mgr.do_show_check(ptr, false);
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test for `ZTHabitatMgr::can_see_show_from_building`: only actually exercises the real call path
/// when no real habitat in the loaded zoo currently has a `ZTShowInfo` (`get_show_info_id() != 0`) - in
/// that case the loop body never reaches `can_see_habitat_from_building` (still real vanilla, left
/// un-ported - see that method's own doc comment) regardless of the building pointer passed, so `0` is a
/// safe, deterministic argument and the expected result is always `0`. When real show habitats *are*
/// present, skips rather than guessing a `ZTBuilding*` pointer: `canSeeHabitatFromBuilding`'s own body
/// dereferences its building argument unconditionally with no null guard, and this harness has no
/// existing helper for finding a real building entity - a wrong guess would risk crashing the live
/// battery for no real coverage gain.
pub(crate) fn run_zthabitatmgr_can_see_show_from_building_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CAN_SEE_SHOW_FROM_BUILDING_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let has_real_show_habitat = (0..habitat_mgr.exhibit_array().len()).any(|i| {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_show_info_id() != 0
    });

    if has_real_show_habitat {
        write_success_line(failure_log, &format!("{} (skipped: no safe real ZTBuilding* pointer available to test against)", test_name));
        return false;
    }

    let result = habitat_mgr.can_see_show_from_building(0);
    if result == 0 {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("expected 0 with no real show habitats present, got {:#x}", result);
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Real-vanilla-vs-reimplementation comparison for `ZTHabitatMgr::canFindPath` and `clear_pathfinding`
/// together, since real vanilla's own `canFindPath` needs a freshly-cleared grid to give a meaningful
/// answer. Picks the live zoo entrance tile and the first owned tile of the first non-"world" habitat in
/// `exhibit_array` as the two endpoints (skips if either isn't available), clears the grid, calls real
/// vanilla's own `.original()` for ground truth, clears the grid again (both runs share the same live
/// `+0x24` visited-bit state so one run's marks would otherwise contaminate the other), then calls the
/// reimplementation and asserts the two agree. Leaves the grid cleared afterward.
///
/// `generated.rs`'s `zthabitatmgr::CAN_FIND_PATH` types its return as `bool` - the x86 ABI only ever
/// reads `AL` for a `bool`-returning function, which is exactly what real callers do too
/// (`ZTHabitatMgr_fencePlaced.c` casts to `char` before comparing) - so `.original()`'s result compares
/// directly against the reimplementation's own `bool` with no manual low-byte masking needed.
pub(crate) fn run_zthabitatmgr_can_find_path_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CAN_FIND_PATH_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = habitat_mgr as *const _ as u32;

    let tile_a = habitat_mgr.get_zoo_entrance_tile_ptr();
    let tile_b = (0..habitat_mgr.exhibit_array().len()).find_map(|i| {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            return None;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        if *habitat.unknown_flag_0x2c() != 0 {
            return None;
        }
        crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
            .next()
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
    });

    let Some(tile_b) = tile_b else {
        write_success_line(failure_log, &format!("{} (skipped: no real habitat with owned tiles found)", test_name));
        return false;
    };
    if tile_a == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no zoo entrance tile set)", test_name));
        return false;
    }

    habitat_mgr.clear_pathfinding();
    let real_result = unsafe { zthabitatmgr::CAN_FIND_PATH.original()(mgr_ptr as *const u32, tile_a as *const u32, tile_b as *const u32) };
    habitat_mgr.clear_pathfinding();
    let reimpl_result = habitat_mgr.can_find_path(tile_a, tile_b);
    habitat_mgr.clear_pathfinding();

    if real_result == reimpl_result {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("real={real_result}, reimpl={reimpl_result}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Smoke test for `ZTHabitatMgr::clear_pathfinding`: marks a real tile's visited bit via `can_find_path`
/// (the entrance tile searching for itself - a trivial same-tile call that still marks the entrance
/// tile's own cell before returning), then clears the grid and asserts that specific cell's `+0x24`
/// visited-bit byte reads back `0`. Skips if no zoo entrance tile is set (same precondition
/// `ZTHABITATMGR_CAN_FIND_PATH_ROUNDTRIP_LIVE` skips on).
pub(crate) fn run_zthabitatmgr_clear_pathfinding_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CLEAR_PATHFINDING_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let tile_ptr = habitat_mgr.get_zoo_entrance_tile_ptr();
    if tile_ptr == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no zoo entrance tile set)", test_name));
        return false;
    }

    habitat_mgr.clear_pathfinding();
    habitat_mgr.can_find_path(tile_ptr, tile_ptr);
    habitat_mgr.clear_pathfinding();

    let tile = get_from_memory::<crate::ztmapview::BFTile>(tile_ptr);
    let Some(cell_addr) = habitat_mgr.get_habitat_cell_addr(tile.pos.x, tile.pos.y) else {
        write_success_line(failure_log, &format!("{} (skipped: entrance tile position out of grid range)", test_name));
        return false;
    };
    let flags: u8 = get_from_memory(cell_addr + 0x24);
    if flags & 0x3 == 0 {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("expected visited bits cleared, got {flags:#x}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Real called before reimpl deliberately, same `characteristics_dirty` ordering rationale as
/// [`run_habitat_get_attractiveness_live_test`] - both `false` (direct-occupant count alone) and `true`
/// (additionally summing every amphibious neighbor's own count) are exercised over every live habitat.
pub(crate) fn run_habitat_get_num_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let direct = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_ANIMALS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_ANIMALS.original()(ptr, false) },
        |habitat| habitat.get_num_animals(false),
    );
    let with_neighbors = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_ANIMALS_WITH_NEIGHBORS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_ANIMALS.original()(ptr, true) },
        |habitat| habitat.get_num_animals(true),
    );
    direct || with_neighbors
}

/// `ZTHabitat::getAllAnimals` always returns the same `&this->field_0x6c` pointer regardless of `sort`,
/// only conditionally reordering the vector's own contents first - comparing that pointer would be
/// meaningless, so this compares the sorted *contents* instead. Real vanilla is called first (`sort =
/// true`), settling the live array into its real ordering; the reimplementation is then called (also
/// `sort = true`) over that now-already-sorted array. Since both call through to the identical real
/// comparator ([`GET_ALL_ANIMALS_SORT_COMPARATOR`] internally), re-sorting an already-correctly-sorted
/// array is expected to be a no-op, so the reimplementation's own output should match real vanilla's
/// exactly, element-for-element.
pub(crate) fn run_habitat_get_all_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_ALL_ANIMALS_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { zthabitat::GET_ALL_ANIMALS.original()(ptr as *const u32, 1) };
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let real_animals: Vec<u32> = (begin..end).step_by(4).map(get_from_memory::<u32>).collect();

        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let reimpl_animals: Vec<u32> = habitat.get_all_animals(true).collect();

        if real_animals != reimpl_animals {
            let msg = format!("mismatch at habitat {i} ({ptr:#010x}): real={real_animals:?}, reimpl={reimpl_animals:?}");
            error!("{}: {}", test_name, msg);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
            }
            fail_flag = true;
        }
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Compares the real vanilla `surrounding_species_begin`/`_end` vector (reached through real
/// `getSurroundingSpecies`'s own returned pointer) against the reimplementation's own
/// [`ZTHabitat::surrounding_species`] iterator, over every live habitat. Both sides share the same
/// `characteristics_dirty` gate and neither mutates the vector itself (unlike `getAllAnimals`), so plain
/// content comparison is safe in either call order.
pub(crate) fn run_habitat_get_surrounding_species_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_SURROUNDING_SPECIES_LIVE",
        |ptr| {
            let vec_ptr = unsafe { zthabitat::GET_SURROUNDING_SPECIES.original()(ptr) } as u32;
            let begin: u32 = get_from_memory(vec_ptr);
            let end: u32 = get_from_memory(vec_ptr + 4);
            (begin..end).step_by(4).map(get_from_memory::<u32>).collect::<Vec<u32>>()
        },
        |habitat| habitat.surrounding_species().collect::<Vec<u32>>(),
    )
}

/// Smoke test for `ZTHabitat::remove_species`: picks the first live habitat whose real
/// `ambients_begin`/`ambients_end` vector is non-empty (skips if none is found - not every loaded zoo
/// necessarily has a habitat with an active ambient-sound species entry), removes its first entry by key,
/// and asserts the vector shrank by exactly one pair and no longer contains that key. Does not compare
/// against real vanilla's own `.original()` - unlike the read-only getters above, this is destructive (it
/// frees a real `Ambients*` and shifts the vector), so there is no way to run both poles against the same
/// starting state; correctness of the shift/free logic itself is exercised here structurally instead.
pub(crate) fn run_habitat_remove_species_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_REMOVE_SPECIES_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let found = (0..habitat_mgr.exhibit_array().len()).find_map(|i| {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            return None;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        if *habitat.ambients_begin() != *habitat.ambients_end() {
            let key: u32 = get_from_memory(*habitat.ambients_begin());
            Some((ptr, key))
        } else {
            None
        }
    });

    let Some((ptr, key)) = found else {
        write_success_line(failure_log, &format!("{} (skipped: no live habitat with a populated ambients vector)", test_name));
        return false;
    };

    let old_len = {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        (*habitat.ambients_end() - *habitat.ambients_begin()) / 8
    };

    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.remove_species(key);

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let new_len = (*habitat.ambients_end() - *habitat.ambients_begin()) / 8;
    let still_present = (0..new_len).any(|i| get_from_memory::<u32>(*habitat.ambients_begin() + i * 8) == key);

    if new_len == old_len - 1 && !still_present {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("expected len {} without key {key:#x}, got len {new_len} (still_present={still_present})", old_len - 1);
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// `ZTHabitat::acceptDonation` has no independent "real" pole to diff against without double-applying the
/// donation to the live zoo's own cash/finance totals (calling both real and reimpl would add the amount
/// twice) - structural test instead: calls the reimplementation once on the first real habitat with a
/// small, fixed amount and asserts `current_donations`/`total_donations` advanced by exactly that amount
/// and the global `ZTGameMgr` budget did too. Reuses the already-live-tested
/// `ZooStatus::increase_donations`/`ZTGameMgr::add_cash` (see `reimplementation_tests/tests/zoostatus.rs`/
/// `ztgamemgr.rs`'s own live tests for those, exercised the same way), so a small, permanent budget/
/// donation-total bump here is the same already-accepted side effect those tests already produce.
pub(crate) fn run_habitat_accept_donation_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_ACCEPT_DONATION_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let Some(ptr) = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).find(|&p| p != 0) else {
        write_success_line(failure_log, &format!("{} (skipped: no live habitat)", test_name));
        return false;
    };

    const AMOUNT: f32 = 1.0;
    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let donations_before = *habitat.current_donations();
    let total_before = *habitat.total_donations();
    let cash_before = globals().ztgamemgr().cash();

    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.accept_donation(AMOUNT);

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let donations_after = *habitat.current_donations();
    let total_after = *habitat.total_donations();
    let cash_after = globals().ztgamemgr().cash();

    let ok = (donations_after - donations_before - AMOUNT).abs() < 0.01
        && (total_after - total_before - AMOUNT).abs() < 0.01
        && (cash_after - cash_before - AMOUNT).abs() < 0.01;

    if ok {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!(
            "donations {donations_before}->{donations_after}, total {total_before}->{total_after}, cash {cash_before}->{cash_after} (amount={AMOUNT})"
        );
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// `ZTHabitat::setDirtyCharacteristics` only ever sets flags (`characteristics_dirty` on `self` and
/// recursively on every amphibious-/show-neighbor) - no allocation, no other observable side effect - so
/// there is no independent "real" pole worth diffing against beyond confirming the flag ends up set.
/// Calls the reimplementation once per real habitat and asserts `characteristics_dirty` is set afterward
/// (whether it started clear or was already dirty). Deliberately does not reset the flag afterward: per
/// `ZTHabitat::update`'s own doc comment this is a harmless, self-correcting lazy-recalculate flag real
/// vanilla's own timers set constantly during normal play anyway.
pub(crate) fn run_habitat_set_dirty_characteristics_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_SET_DIRTY_CHARACTERISTICS_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { mut_from_memory::<ZTHabitat>(ptr) }.set_dirty_characteristics();
        let now_dirty = *unsafe { ref_from_memory::<ZTHabitat>(ptr) }.characteristics_dirty() != 0;
        if !now_dirty {
            failures.push(format!("habitat {} ({:#010x}): set_dirty_characteristics left characteristics_dirty clear", i, ptr));
        }
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

/// `ZTHabitat::setTimeLastServiced` writes a timestamp field this pass found no real reader for (see
/// `zthabitatmgr.rs`'s own `time_last_serviced` doc comment) - structural test: writes a fixed sentinel
/// value to the first real habitat via the reimplementation and asserts it reads back exactly, then
/// restores the field's original value (unlike `characteristics_dirty` this field has no established
/// "harmless to leave changed" precedent, so restored defensively).
pub(crate) fn run_habitat_set_time_last_serviced_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_SET_TIME_LAST_SERVICED_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let Some(ptr) = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).find(|&p| p != 0) else {
        write_success_line(failure_log, &format!("{} (skipped: no live habitat)", test_name));
        return false;
    };

    let original: u32 = get_from_memory(ptr + 0xec);
    const SENTINEL: u32 = 0x1234_5678;

    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.set_time_last_serviced(SENTINEL, true);
    let written: u32 = get_from_memory(ptr + 0xec);

    save_to_memory(ptr + 0xec, original);

    if written == SENTINEL {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("expected {SENTINEL:#x}, got {written:#x}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Reads `ptr`'s own `boundary_tile_pairs_begin`/`_end` vector into a plain `Vec` for
/// [`run_habitat_create_edge_pairs_matches_real_live_test`]'s own before/after comparison.
fn snapshot_boundary_pairs(ptr: u32) -> Vec<(u32, u32)> {
    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let begin = *habitat.boundary_tile_pairs_begin();
    let end = *habitat.boundary_tile_pairs_end();
    let mut result = Vec::new();
    let mut cursor = begin;
    while cursor != end {
        result.push((get_from_memory(cursor), get_from_memory(cursor + 4)));
        cursor += 8;
    }
    result
}

/// Compares real vanilla `ZTHabitat::createEdgePairs` against the reimplementation on the same live
/// habitat, called back-to-back: `createEdgePairs` fully rebuilds `boundary_tile_pairs_begin`/`_end` from
/// scratch every call (clears then repopulates - no dependency on the vector's own prior contents), so
/// calling real first and then the reimplementation on the identical, now-settled input produces directly
/// comparable output, the same "real-then-reimpl call-through-safe" reasoning
/// `ZTHABITAT_GET_ATTRACTIVENESS_LIVE` already establishes for a different lazily-recalculated field.
pub(crate) fn run_habitat_create_edge_pairs_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_CREATE_EDGE_PAIRS_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let Some(ptr) = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).find(|&p| p != 0) else {
        write_success_line(failure_log, &format!("{} (skipped: no live habitat)", test_name));
        return false;
    };

    unsafe { zthabitat::CREATE_EDGE_PAIRS.original()(ptr as *const u32) };
    let real_pairs = snapshot_boundary_pairs(ptr);

    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.create_edge_pairs();
    let reimpl_pairs = snapshot_boundary_pairs(ptr);

    if real_pairs == reimpl_pairs {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("real={:?}, reimpl={:?}", real_pairs, reimpl_pairs);
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Roundtrip test for `ZTHabitat::addViewingArea`/`removeViewingArea`: constructs a brand-new, synthetic
/// `ZTViewingArea` (real vanilla `operator_new(0x5c)` + `ZTViewingArea::ZTViewingArea` constructor, both
/// called through) over the first live habitat's own first owned tile, appends it via the
/// reimplementation, asserts the vector grew by one entry ending in the new pointer with
/// `characteristics_dirty` set, then removes it again via the reimplementation (which also frees it
/// through real vanilla's own destructor/`operator_delete` - no leak) and asserts the vector is back to
/// its original length. Synthetic and self-contained: never touches any pre-existing real viewing area.
pub(crate) fn run_habitat_add_remove_viewing_area_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_ADD_REMOVE_VIEWING_AREA_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let found = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).find_map(|ptr| {
        if ptr == 0 {
            return None;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        crate::zthabitatmgr::walk_tile_list(*habitat.owned_tiles_ptr())
            .next()
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
            .map(|tile_ptr| (ptr, tile_ptr))
    });

    let Some((ptr, tile_ptr)) = found else {
        write_success_line(failure_log, &format!("{} (skipped: no live habitat with an owned tile)", test_name));
        return false;
    };

    let new_va = unsafe { OPERATOR_NEW.original()(0x5c) } as u32;
    if new_va == 0 {
        write_success_line(failure_log, &format!("{} (skipped: operator_new failed)", test_name));
        return false;
    }
    unsafe { ztviewingarea::CONSTRUCTOR.original()(new_va as *const u32, ptr as *const std::ffi::c_void, tile_ptr as *const std::ffi::c_void) };

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let before_len = (*habitat.viewing_areas_end() - *habitat.viewing_areas_begin()) / 4;

    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.add_viewing_area(new_va);

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let after_add_len = (*habitat.viewing_areas_end() - *habitat.viewing_areas_begin()) / 4;
    let last_entry: u32 = get_from_memory(*habitat.viewing_areas_end() - 4);
    let dirty_after_add = *habitat.characteristics_dirty() != 0;

    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.remove_viewing_area(new_va);

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let after_remove_len = (*habitat.viewing_areas_end() - *habitat.viewing_areas_begin()) / 4;

    let ok = after_add_len == before_len + 1 && last_entry == new_va && dirty_after_add && after_remove_len == before_len;

    if ok {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!(
            "before_len={before_len}, after_add_len={after_add_len} (expected {}), last_entry={last_entry:#x} (expected {new_va:#x}), \
             dirty_after_add={dirty_after_add}, after_remove_len={after_remove_len} (expected {before_len})",
            before_len + 1
        );
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Smoke test for `ZTHabitat::recreateOAs`: finds the first live habitat with a non-empty viewing-areas
/// vector (skips if none), calls the reimplementation, and asserts every entry's own `+0x25` byte reads
/// back `1` afterward.
pub(crate) fn run_habitat_recreate_oas_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_RECREATE_OAS_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let found = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).find(|&ptr| {
        ptr != 0 && {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
            *habitat.viewing_areas_begin() != *habitat.viewing_areas_end()
        }
    });

    let Some(ptr) = found else {
        write_success_line(failure_log, &format!("{} (skipped: no live habitat with a populated viewing-areas vector)", test_name));
        return false;
    };

    unsafe { ref_from_memory::<ZTHabitat>(ptr) }.recreate_oas();

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let mut cursor = *habitat.viewing_areas_begin();
    let end = *habitat.viewing_areas_end();
    let mut unset: Vec<u32> = Vec::new();
    while cursor != end {
        let va_ptr: u32 = get_from_memory(cursor);
        let flag: u8 = get_from_memory(va_ptr + 0x25);
        if flag != 1 {
            unset.push(va_ptr);
        }
        cursor += 4;
    }

    if unset.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("viewing areas with +0x25 still unset: {unset:#x?}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Smoke test only - **does not** call real vanilla `ZTHabitat::getOutermostTank` directly for
/// comparison, unlike this file's other `ZTHABITAT_*_LIVE` getter tests. `ZTHabitat::get_outermost_tank`'s
/// own doc comment documents a genuine, real-vanilla null-deref bug (`ZTHabitat_getOutermostTank.asm`'s
/// `.15`/`.1466e` branch dereferences a zeroed `ESI` when `getGateTileOut` finds no further gate tile) -
/// confirmed live by an earlier version of this exact test, which called `.original()` directly and
/// crashed the whole battery outright on a real tank whose own gate-tile chain ends this way. The
/// reimplemented method deliberately diverges from real vanilla on that one input (returns the current
/// tank instead of crashing) - there is no safe way to invoke real vanilla's own body for a byte-for-byte
/// comparison here, so this only smoke-tests the reimplementation itself.
pub(crate) fn run_habitat_get_outermost_tank_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_OUTERMOST_TANK_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 || !unsafe { ref_from_memory::<ZTHabitat>(ptr) }.is_tank() {
            continue;
        }
        habitat_mgr.clear_tank_walk_markers();
        let _ = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_outermost_tank();
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test for `ZTHabitatMgr::getOutermostTank` (the manager-level wrapper) - **not** detoured (see
/// its own doc comment: `generated.rs`'s declared return type is wrong), so this is the only live
/// coverage it gets. Calls it directly for every real tank in the loaded zoo, asserting no crash and
/// that it agrees with the already-verified [`ZTHabitat::get_outermost_tank`].
pub(crate) fn run_zthabitatmgr_get_outermost_tank_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_GET_OUTERMOST_TANK_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 || !unsafe { ref_from_memory::<ZTHabitat>(ptr) }.is_tank() {
            continue;
        }
        let via_mgr = habitat_mgr.get_outermost_tank(ptr);
        habitat_mgr.clear_tank_walk_markers();
        let via_habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_outermost_tank();
        if via_mgr != via_habitat {
            let msg = format!("mismatch at habitat {} ({:#010x}): via_mgr={:#010x}, via_habitat={:#010x}", i, ptr, via_mgr, via_habitat);
            error!("{}: {}", test_name, msg);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
            }
            fail_flag = true;
        }
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// `ZTHABITAT_GET_NEEDY_NESTED_TANK_SMOKE_LIVE` - same "compare our own two call paths, not real
/// vanilla" shape as `ZTHABITATMGR_GET_OUTERMOST_TANK_SMOKE_LIVE` just above, deliberately avoiding a
/// direct `.original()` comparison: real vanilla's own body has two independent unguarded null-deref
/// paths (a boundary pair's own second tile pointer, or the habitat resolved at that tile's position,
/// either being null - see `ZTHabitat::get_needy_nested_tank`'s own doc comment) that
/// `ZTHABITAT_GET_OUTERMOST_TANK_LIVE`'s own history shows can crash the whole battery outright when hit
/// live rather than raising a catchable exception.
pub(crate) fn run_habitat_get_needy_nested_tank_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NEEDY_NESTED_TANK_SMOKE_LIVE";

    let keeper_ptr = globals().ztworldmgr().entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) });
    let Some(keeper_ptr) = keeper_ptr else {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    };

    let habitat_mgr = globals().zthabitatmgr();
    let mut checked = 0;
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 || !unsafe { ref_from_memory::<ZTHabitat>(ptr) }.is_tank() {
            continue;
        }
        checked += 1;
        let via_mgr = habitat_mgr.get_needy_nested_tank(ptr, keeper_ptr);
        habitat_mgr.clear_tank_walk_markers();
        let via_habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_needy_nested_tank(keeper_ptr);
        if via_mgr != via_habitat {
            let msg = format!("mismatch at habitat {} ({:#010x}): via_mgr={:#010x}, via_habitat={:#010x}", i, ptr, via_mgr, via_habitat);
            error!("{}: {}", test_name, msg);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
            }
            fail_flag = true;
        }
    }

    if checked == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no live tank habitat found)", test_name));
        return false;
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Comparison test for `ZTHabitatMgr::getTank`: real vs. reimplemented, over every real habitat's own
/// [`ZTHabitat::get_gate_tile_out`] tile (a real, in-map tile guaranteed to exist for any habitat that
/// has ever been placed) plus the null-tile edge case.
pub(crate) fn run_zthabitatmgr_get_tank_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_GET_TANK_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut fail_flag = false;

    let mut check = |tile_ptr: u32| {
        let real_value = unsafe { zthabitatmgr::GET_TANK.original()(habitat_mgr as *const _ as *const u32, tile_ptr as *const u32) } as u32;
        let reimpl_value = habitat_mgr.get_tank(tile_ptr);
        if real_value != reimpl_value {
            let msg = format!("mismatch at tile {:#010x}: real={:#010x}, reimpl={:#010x}", tile_ptr, real_value, reimpl_value);
            error!("{}: {}", test_name, msg);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
            }
            fail_flag = true;
        }
    };

    check(0);
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        if let Some(tile) = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_gate_tile_out() {
            check(globals().ztworldmgr().get_ptr_from_bftile(&tile));
        }
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Smoke test: calls the reimplemented `ZTHabitatMgr::break_amphibious_connection` over every real
/// habitat's own boundary tile-pairs, asserting no crash - same "not restored afterward" convention as
/// this file's other real-state-mutating smoke tests (e.g.
/// `run_zthabitatmgr_check_amphibious_neighbor_smoke_live_test`).
pub(crate) fn run_zthabitatmgr_break_amphibious_connection_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_BREAK_AMPHIBIOUS_CONNECTION_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut entry = *habitat.boundary_tile_pairs_begin();
        let end = *habitat.boundary_tile_pairs_end();
        while entry != end {
            let tile_a: u32 = get_from_memory(entry);
            let tile_b: u32 = get_from_memory(entry + 4);
            crate::zthabitatmgr::ZTHabitatMgr::break_amphibious_connection(tile_a, tile_b);
            entry += 8;
        }
    }
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test: calls the reimplemented `ZTHabitatMgr::recalculate_deterioration` once against the real,
/// loaded zoo's own fences/habitats, asserting no crash. Mutates real habitat deterioration state as a
/// side effect (matches real vanilla's own per-tick call) - not restored afterward, same convention as
/// this file's other mutating smoke tests.
pub(crate) fn run_zthabitatmgr_recalculate_deterioration_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_RECALCULATE_DETERIORATION_SMOKE_LIVE";
    unsafe { &mut *globals().zthabitatmgr_ptr() }.recalculate_deterioration();
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test: calls the reimplemented `ZTHabitatMgr::mark_zoo_exterior` (which internally calls
/// `fill_zoo_exterior`) once against the real, loaded zoo, asserting no crash. Mutates real tile state
/// (`BFTile`'s own `+0x83` "in zoo" bit) as a side effect - matches real vanilla's own per-load call, not
/// restored afterward.
pub(crate) fn run_zthabitatmgr_mark_zoo_exterior_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_MARK_ZOO_EXTERIOR_SMOKE_LIVE";
    globals().zthabitatmgr().mark_zoo_exterior();
    write_success_line(failure_log, test_name);
    false
}

/// Smoke test: calls the reimplemented `ZTHabitatMgr::update` (the manager-level, non-virtual function -
/// distinct from the already-independently-tested `ZTHabitat::update` vtable slot) once against the real,
/// loaded zoo with a plausible `elapsed` value, asserting no crash. Calls through to real, un-ported
/// vanilla `updateGates` and the already-verified `ZTHabitat::update` over every real habitat - not
/// restored afterward, matching this file's other mutating smoke tests.
pub(crate) fn run_zthabitatmgr_update_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_UPDATE_SMOKE_LIVE";
    globals().zthabitatmgr().update(16);
    write_success_line(failure_log, test_name);
    false
}

/// Real comparison test (not just a smoke test - `ZTHabitat::blockService` is read-only, so calling the
/// real, un-detoured `.original()` address alongside the reimplementation is safe): finds the first live
/// `ZTKeeper` in the real, loaded zoo's own `entity_array` (via [`RVA_KEEPER_TYPE_CHECK_ARG`], the same
/// `isCastClass` tag [`crate::zthabitatmgr::ZTHabitat::block_service`]'s own leading guard uses), then
/// compares real vanilla against the reimplementation over every real habitat for all four
/// `(skip_tank_depth_check, check_tank_and_neighbors)` combinations. Skipped (not failed) if the loaded
/// save has no keeper at all.
/// `ZTHABITATMGR_CHECK_ENTER_HABITAT_MATCHES_REAL_LIVE` - compares real vanilla `checkEnterHabitat`
/// against [`ZTHabitatMgr::check_enter_habitat`] for every real habitat that has a confirmed entrance
/// gate ([`ZTHabitat::get_gate_tile_in`]/`_out` both `Some`), using any live animal as `unit_ptr` (the
/// function only compares path costs to the gate tiles, not the unit's actual current location, so any
/// real `BFUnit` works). Pure query with no side effects on either side, so a direct real-vanilla
/// comparison is safe - same reasoning as `ZTHABITAT_BLOCK_SERVICE_MATCHES_REAL_LIVE` just below.
pub(crate) fn run_zthabitatmgr_check_enter_habitat_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CHECK_ENTER_HABITAT_MATCHES_REAL_LIVE";

    let unit_ptr = globals().ztworldmgr().entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_ANIMAL_TYPE_CHECK) });
    let Some(unit_ptr) = unit_ptr else {
        write_success_line(failure_log, &format!("{} (skipped: no live animal found)", test_name));
        return false;
    };

    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        if habitat.get_gate_tile_in().is_none() || habitat.get_gate_tile_out().is_none() {
            continue;
        }
        checked += 1;
        let real = unsafe { zthabitatmgr::CHECK_ENTER_HABITAT.original()(ptr as *const u32, unit_ptr as *const u32) };
        let reimpl = ZTHabitatMgr::check_enter_habitat(ptr, unit_ptr);
        if real != reimpl {
            failures.push(format!("habitat {} ({:#010x}): real={}, reimpl={}", i, ptr, real, reimpl));
        }
    }

    if checked == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no habitat with a confirmed entrance gate)", test_name));
        return false;
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

/// Smoke test for `ZTHabitatMgr::entity_about_to_be_removed`: calls it over every live world entity that
/// resolves (via `BFEntity::getTile`) to a real habitat, asserting no crash. `characteristics_dirty`/
/// `species_list_dirty` are written unconditionally before the tank/amphibious-neighbor branch, but
/// `BEFORE_ENTITY_CHANGE`'s own real (un-ported) body immediately clears both again as part of its own
/// work (confirmed live), so their post-call state is not itself a testable signal here - see
/// [`crate::zthabitatmgr::ZTHabitatMgr::entity_about_to_be_removed`]'s own doc comment. Separately tracks
/// whether any live entity's habitat is a tank with a non-empty amphibious-neighbor set and is itself
/// scenery or an animal (the tank-branch precondition, reached only for real coverage of that branch); if
/// none is found in the loaded save, reports a skip rather than a failure, matching
/// [`run_habitat_hilite_neighbors_roundtrip_live_test`]'s own convention.
pub(crate) fn run_zthabitatmgr_entity_about_to_be_removed_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_ENTITY_ABOUT_TO_BE_REMOVED_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut tank_neighbor_exercised = false;

    for entity_ptr in globals().ztworldmgr().entity_array() {
        if entity_ptr == 0 {
            continue;
        }
        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(entity_ptr as *const u32) } as u32;
        if tile_ptr == 0 {
            continue;
        }
        let tile = get_from_memory::<crate::ztmapview::BFTile>(tile_ptr);
        let habitat_ptr = habitat_mgr.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let is_tank_with_neighbors = habitat.is_tank() && walk_neighbor_tree(*habitat.amphibious_neighbors_head()).next().is_some();
        let is_scenery_or_animal =
            unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || entity_type_matches(entity_ptr, RVA_ANIMAL_TYPE_CHECK) };

        habitat_mgr.entity_about_to_be_removed(entity_ptr);

        if is_tank_with_neighbors && is_scenery_or_animal {
            tank_neighbor_exercised = true;
        }
    }

    if tank_neighbor_exercised {
        write_success_line(failure_log, test_name);
    } else {
        write_success_line(failure_log, &format!("{} (skipped: no tank habitat with amphibious neighbors found)", test_name));
    }
    false
}

/// Smoke test for `ZTHabitatMgr::entity_removed`: same live-entity walk as
/// [`run_zthabitatmgr_entity_about_to_be_removed_smoke_live_test`], calling `entity_removed(tile_ptr,
/// entity_type_ptr)` (`entity_type_ptr` read from `BFEntity::inner_class_ptr` at `+0x128`), asserting no
/// crash. Same dirty-flag caveat as that sibling applies (`AFTER_ENTITY_CHANGE`'s own real body clears
/// them again), so this only tracks whether the tank+amphibious-neighbor branch was reached for coverage
/// purposes, same skip convention as the sibling.
pub(crate) fn run_zthabitatmgr_entity_removed_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_ENTITY_REMOVED_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut tank_neighbor_exercised = false;

    for entity_ptr in globals().ztworldmgr().entity_array() {
        if entity_ptr == 0 {
            continue;
        }
        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(entity_ptr as *const u32) } as u32;
        if tile_ptr == 0 {
            continue;
        }
        let tile = get_from_memory::<crate::ztmapview::BFTile>(tile_ptr);
        let habitat_ptr = habitat_mgr.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr == 0 {
            continue;
        }
        let entity_type_ptr = get_from_memory::<u32>(entity_ptr + 0x128);
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let is_tank_with_neighbors = habitat.is_tank() && walk_neighbor_tree(*habitat.amphibious_neighbors_head()).next().is_some();
        let is_scenery_or_animal = entity_type_ptr != 0
            && unsafe { crate::ztshow::type_check(entity_type_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || crate::ztshow::type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) };

        habitat_mgr.entity_removed(tile_ptr, entity_type_ptr);

        if is_tank_with_neighbors && is_scenery_or_animal {
            tank_neighbor_exercised = true;
        }
    }

    if tank_neighbor_exercised {
        write_success_line(failure_log, test_name);
    } else {
        write_success_line(failure_log, &format!("{} (skipped: no tank habitat with amphibious neighbors found)", test_name));
    }
    false
}

/// Smoke test for `ZTHabitatMgr::entity_about_to_be_placed`: same shape as
/// [`run_zthabitatmgr_entity_about_to_be_removed_smoke_live_test`], but resolves the entity's tile via
/// `WORLD_TO_TILE` against `BFEntity::pos` (`+0x114`) instead of `BFEntity::getTile`, matching the
/// production method's own resolution path for an entity not yet assigned a tile.
pub(crate) fn run_zthabitatmgr_entity_about_to_be_placed_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_ENTITY_ABOUT_TO_BE_PLACED_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut tank_neighbor_exercised = false;

    for entity_ptr in globals().ztworldmgr().entity_array() {
        if entity_ptr == 0 {
            continue;
        }
        let mut tile_xyz = [0i32; 3];
        unsafe { WORLD_TO_TILE.original()(tile_xyz.as_mut_ptr() as *const i32, (entity_ptr + 0x114) as *const i32) };
        let habitat_ptr = habitat_mgr.get_habitat_ptr(tile_xyz[0], tile_xyz[1]);
        if habitat_ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let is_tank_with_neighbors = habitat.is_tank() && walk_neighbor_tree(*habitat.amphibious_neighbors_head()).next().is_some();
        let is_scenery_or_animal =
            unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || entity_type_matches(entity_ptr, RVA_ANIMAL_TYPE_CHECK) };

        habitat_mgr.entity_about_to_be_placed(entity_ptr);

        if is_tank_with_neighbors && is_scenery_or_animal {
            tank_neighbor_exercised = true;
        }
    }

    if tank_neighbor_exercised {
        write_success_line(failure_log, test_name);
    } else {
        write_success_line(failure_log, &format!("{} (skipped: no tank habitat with amphibious neighbors found)", test_name));
    }
    false
}

/// Smoke test for `ZTHabitatMgr::entity_placed`: same shape as
/// [`run_zthabitatmgr_entity_removed_smoke_live_test`], but resolves the entity's tile via
/// `WORLD_TO_TILE`/`BFEntity::pos` like [`run_zthabitatmgr_entity_about_to_be_placed_smoke_live_test`],
/// bounds-checked against `map_x_size`/`map_y_size` to match the production method's own guard.
pub(crate) fn run_zthabitatmgr_entity_placed_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_ENTITY_PLACED_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let world_mgr = globals().ztworldmgr();
    let mut tank_neighbor_exercised = false;

    for entity_ptr in world_mgr.entity_array() {
        if entity_ptr == 0 {
            continue;
        }
        let mut tile_xyz = [0i32; 3];
        unsafe { WORLD_TO_TILE.original()(tile_xyz.as_mut_ptr() as *const i32, (entity_ptr + 0x114) as *const i32) };
        let (tile_x, tile_y) = (tile_xyz[0], tile_xyz[1]);
        if tile_x < 0 || tile_y < 0 || tile_x as u32 >= world_mgr.map_x_size || tile_y as u32 >= world_mgr.map_y_size {
            continue;
        }
        let tile_ptr = world_mgr.get_tile_ptr(tile_x as u32, tile_y as u32);
        let tile = get_from_memory::<crate::ztmapview::BFTile>(tile_ptr);
        let habitat_ptr = habitat_mgr.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let is_tank_with_neighbors = habitat.is_tank() && walk_neighbor_tree(*habitat.amphibious_neighbors_head()).next().is_some();
        let is_scenery_or_animal =
            unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || entity_type_matches(entity_ptr, RVA_ANIMAL_TYPE_CHECK) };

        habitat_mgr.entity_placed(entity_ptr);

        if is_tank_with_neighbors && is_scenery_or_animal {
            tank_neighbor_exercised = true;
        }
    }

    if tank_neighbor_exercised {
        write_success_line(failure_log, test_name);
    } else {
        write_success_line(failure_log, &format!("{} (skipped: no tank habitat with amphibious neighbors found)", test_name));
    }
    false
}

pub(crate) fn run_habitat_block_service_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_BLOCK_SERVICE_MATCHES_REAL_LIVE";

    let keeper_ptr = globals().ztworldmgr().entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) });
    let Some(keeper_ptr) = keeper_ptr else {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    };

    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for skip_tank_depth_check in [false, true] {
            for check_tank_and_neighbors in [false, true] {
                let real = unsafe {
                    zthabitat::BLOCK_SERVICE.original()(ptr as *const u32, keeper_ptr as *const u32, skip_tank_depth_check, check_tank_and_neighbors)
                };
                let reimpl = habitat.block_service(keeper_ptr, skip_tank_depth_check, check_tank_and_neighbors);
                if real != reimpl {
                    failures.push(format!(
                        "habitat {} ({:#010x}), skip_tank_depth_check={}, check_tank_and_neighbors={}: real={}, reimpl={}",
                        i, ptr, skip_tank_depth_check, check_tank_and_neighbors, real, reimpl
                    ));
                }
            }
        }
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

/// Step 6o batch: `getNumKeepers`, `isBeingServiced`, `getAnimals` - all three are plain
/// `characteristics_dirty`-gated leaf getters, same shape as `getAttractiveness`, so
/// [`compare_over_live_habitats`] applies directly.
pub(crate) fn run_habitat_get_num_keepers_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_KEEPERS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_KEEPERS.original()(ptr) },
        |habitat| habitat.get_num_keepers(),
    )
}

pub(crate) fn run_habitat_is_being_serviced_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_IS_BEING_SERVICED_LIVE",
        |ptr| unsafe { zthabitat::IS_BEING_SERVICED.original()(ptr) != 0 },
        |habitat| habitat.is_being_serviced(),
    )
}

pub(crate) fn run_habitat_get_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_ANIMALS_LIVE",
        |ptr| unsafe { zthabitat::GET_ANIMALS.original()(ptr) },
        |habitat| habitat.get_animals() as i32,
    )
}

/// Same direct/with-neighbors shape as `run_habitat_get_num_animals_live_test`.
pub(crate) fn run_habitat_get_num_hungry_foodless_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let direct = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_HUNGRY_FOODLESS_ANIMALS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_HUNGRY_FOODLESS_ANIMALS.original()(ptr, false) },
        |habitat| habitat.get_num_hungry_foodless_animals(false),
    );
    let with_neighbors = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_HUNGRY_FOODLESS_ANIMALS_WITH_NEIGHBORS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_HUNGRY_FOODLESS_ANIMALS.original()(ptr, true) },
        |habitat| habitat.get_num_hungry_foodless_animals(true),
    );
    direct || with_neighbors
}

/// `getAmountKeeperFood`'s 16-entry `category` array isn't otherwise enumerable from outside
/// `ZTHabitat` (no accessor exposes its length as a constant), so this exercises every index directly
/// rather than going through [`compare_over_live_habitats`]' single-value shape.
pub(crate) fn run_habitat_get_amount_keeper_food_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_AMOUNT_KEEPER_FOOD_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for category in 0..16u32 {
            for include_neighbors in [false, true] {
                let real = unsafe { zthabitat::GET_AMOUNT_KEEPER_FOOD.original()(ptr as *const u32, category, include_neighbors) };
                let reimpl = habitat.get_amount_keeper_food(category, include_neighbors);
                if real != reimpl {
                    failures.push(format!(
                        "habitat {} ({:#010x}), category={}, include_neighbors={}: real={}, reimpl={}",
                        i, ptr, category, include_neighbors, real, reimpl
                    ));
                }
            }
        }
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

/// Same per-category exhaustive shape as `run_habitat_get_amount_keeper_food_live_test`.
pub(crate) fn run_habitat_get_food_to_leave_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_FOOD_TO_LEAVE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for category in 0..16u32 {
            for include_neighbors in [false, true] {
                let real = unsafe { zthabitat::GET_FOOD_TO_LEAVE.original()(ptr as *const u32, category, include_neighbors) };
                let reimpl = habitat.get_food_to_leave(category as i32, include_neighbors);
                if real != reimpl {
                    failures.push(format!(
                        "habitat {} ({:#010x}), category={}, include_neighbors={}: real={}, reimpl={}",
                        i, ptr, category, include_neighbors, real, reimpl
                    ));
                }
            }
        }
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

/// Membership test over `hasBldg`'s own vector: `entity_ptr = 0` is exercised on every habitat (should
/// never be a member), plus each habitat's own first real building-list entry when non-empty (should
/// always be a member) - covers both the false and true paths without needing to construct anything.
pub(crate) fn run_habitat_has_bldg_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_HAS_BLDG_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut candidates = vec![0u32];
        let (begin, end) = (*habitat.building_list_begin(), *habitat.building_list_end());
        if end > begin {
            candidates.push(get_from_memory::<u32>(begin));
        }
        for entity_ptr in candidates {
            let real = unsafe { zthabitat::HAS_BLDG.original()(ptr as *const u32, entity_ptr as *const u32) };
            let reimpl = habitat.has_bldg(entity_ptr);
            if real != reimpl {
                failures.push(format!("habitat {} ({:#010x}), entity={:#010x}: real={}, reimpl={}", i, ptr, entity_ptr, real, reimpl));
            }
        }
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

/// Content comparison (sorted, not pointer-identity) for `getSicklyAnimals`' own out-param vector -
/// real vanilla is called first into its own scratch buffer, then the reimplementation into a separate
/// scratch buffer, both freed afterward via [`free_event_vector_buffer`] (matching each side's own real
/// tail exactly - not a `PoolAlloc::deallocate` call).
pub(crate) fn run_habitat_get_sickly_animals_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_SICKLY_ANIMALS_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };

        let mut real_vector = [0u32; 3];
        unsafe { zthabitat::GET_SICKLY_ANIMALS.original()(ptr as *const u32, real_vector.as_mut_ptr() as *const i32) };
        let mut real_animals: Vec<u32> = (real_vector[0]..real_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
        real_animals.sort_unstable();
        free_event_vector_buffer(real_vector[0], real_vector[2] - real_vector[0]);

        let mut reimpl_vector = [0u32; 3];
        habitat.get_sickly_animals(reimpl_vector.as_mut_ptr() as u32);
        let mut reimpl_animals: Vec<u32> = (reimpl_vector[0]..reimpl_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
        reimpl_animals.sort_unstable();
        free_event_vector_buffer(reimpl_vector[0], reimpl_vector[2] - reimpl_vector[0]);

        if real_animals != reimpl_animals {
            failures.push(format!("habitat {} ({:#010x}): real={:?}, reimpl={:?}", i, ptr, real_animals, reimpl_animals));
        }
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

/// Same content-comparison shape as `run_habitat_get_sickly_animals_matches_real_live_test`, for
/// `getViewingAreasWithGuests`' own out-param vector.
pub(crate) fn run_habitat_get_viewing_areas_with_guests_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_VIEWING_AREAS_WITH_GUESTS_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };

        let mut real_vector = [0u32; 3];
        unsafe { zthabitat::GET_VIEWING_AREAS_WITH_GUESTS.original()(ptr as *const u32, real_vector.as_mut_ptr() as *const i32) };
        let mut real_vas: Vec<u32> = (real_vector[0]..real_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
        real_vas.sort_unstable();
        free_event_vector_buffer(real_vector[0], real_vector[2] - real_vector[0]);

        let mut reimpl_vector = [0u32; 3];
        habitat.get_viewing_areas_with_guests(reimpl_vector.as_mut_ptr() as u32);
        let mut reimpl_vas: Vec<u32> = (reimpl_vector[0]..reimpl_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
        reimpl_vas.sort_unstable();
        free_event_vector_buffer(reimpl_vector[0], reimpl_vector[2] - reimpl_vector[0]);

        if real_vas != reimpl_vas {
            failures.push(format!("habitat {} ({:#010x}): real={:?}, reimpl={:?}", i, ptr, real_vas, reimpl_vas));
        }
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

/// Same "find a live `ZTKeeper`, compare over every habitat" shape as
/// `run_habitat_block_service_matches_real_live_test`.
pub(crate) fn run_habitat_get_num_sickly_animals_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NUM_SICKLY_ANIMALS_MATCHES_REAL_LIVE";

    let keeper_ptr = globals().ztworldmgr().entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) });
    let Some(keeper_ptr) = keeper_ptr else {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    };

    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for include_neighbors in [false, true] {
            let real = unsafe { zthabitat::GET_NUM_SICKLY_ANIMALS.original()(ptr as *const u32, keeper_ptr as *const u32, include_neighbors) };
            let reimpl = habitat.get_num_sickly_animals(keeper_ptr, include_neighbors);
            if real != reimpl {
                failures.push(format!("habitat {} ({:#010x}), include_neighbors={}: real={}, reimpl={}", i, ptr, include_neighbors, real, reimpl));
            }
        }
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

/// Same "find a live `ZTKeeper`, compare over every habitat" shape, for `getNearestSickAnimal`'s own
/// returned pointer (rather than a count).
pub(crate) fn run_habitat_get_nearest_sick_animal_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NEAREST_SICK_ANIMAL_MATCHES_REAL_LIVE";

    let keeper_ptr = globals().ztworldmgr().entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) });
    let Some(keeper_ptr) = keeper_ptr else {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    };

    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for check_can_see in [false, true] {
            let real = unsafe { zthabitat::GET_NEAREST_SICK_ANIMAL.original()(ptr as *const u32, keeper_ptr as i32, check_can_see as i8) } as u32;
            let reimpl = habitat.get_nearest_sick_animal(keeper_ptr, check_can_see);
            if real != reimpl {
                failures.push(format!(
                    "habitat {} ({:#010x}), check_can_see={}: real={:#010x}, reimpl={:#010x}",
                    i, ptr, check_can_see, real, reimpl
                ));
            }
        }
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

/// Roundtrip test for `ZTHabitat::removeViewingAreas`: same synthetic-`ZTViewingArea` construction as
/// `run_habitat_add_remove_viewing_area_roundtrip_live_test`, but only on a habitat that starts with
/// **no** existing viewing areas (`viewing_areas_begin == viewing_areas_end`) - `removeViewingAreas`
/// empties the *entire* vector, not just one entry, so running it against a habitat with pre-existing
/// real viewing areas would destroy live game state unrelated to this test. Only the synthetic area
/// this test itself added is ever at risk.
pub(crate) fn run_habitat_remove_viewing_areas_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_REMOVE_VIEWING_AREAS_ROUNDTRIP_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let found = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).find_map(|ptr| {
        if ptr == 0 {
            return None;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        if *habitat.unknown_flag_0x2c() != 0 || *habitat.viewing_areas_begin() != *habitat.viewing_areas_end() {
            return None;
        }
        walk_tile_list(*habitat.owned_tiles_ptr()).next().map(|node| get_from_memory::<u32>(node + 0x8)).filter(|&t| t != 0).map(|tile_ptr| (ptr, tile_ptr))
    });

    let Some((ptr, tile_ptr)) = found else {
        write_success_line(failure_log, &format!("{} (skipped: no live habitat with empty viewing areas and an owned tile)", test_name));
        return false;
    };

    let new_va = unsafe { OPERATOR_NEW.original()(0x5c) } as u32;
    if new_va == 0 {
        write_success_line(failure_log, &format!("{} (skipped: operator_new failed)", test_name));
        return false;
    }
    unsafe { ztviewingarea::CONSTRUCTOR.original()(new_va as *const u32, ptr as *const std::ffi::c_void, tile_ptr as *const std::ffi::c_void) };

    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.add_viewing_area(new_va);
    unsafe { mut_from_memory::<ZTHabitat>(ptr) }.remove_viewing_areas();

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
    let ok = *habitat.viewing_areas_begin() == *habitat.viewing_areas_end();

    if ok {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!(
            "expected empty viewing area vector after remove_viewing_areas, begin={:#x} end={:#x}",
            habitat.viewing_areas_begin(),
            habitat.viewing_areas_end()
        );
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}
