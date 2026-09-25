//! Compares the reimplemented `ZTHabitatMgr`/`ZTHabitat` methods (production file
//! `openzt/src/zthabitatmgr.rs`) against real vanilla over the live, loaded zoo's own habitat grid.

use openzt_detour::generated::{
    bfaimgr::CHECK_PATH as BFAIMGR_CHECK_PATH,
    bfentity,
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0, WORLD_TO_TILE},
    standalone::OPERATOR_NEW,
    ztanimal::CAN_SERVICE as ZTANIMAL_CAN_SERVICE,
    zthabitat, zthabitatmgr, ztviewingarea,
};
use std::fmt::Debug;
use std::io::Write;
use tracing::error;

use crate::globals::{get_module_base, globals};
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, low_byte_bool, mut_from_memory, ref_from_memory, save_to_memory};
use crate::ztmapview::BFTile;
use crate::ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG};
use crate::ztshow::RVA_ANIMAL_TYPE_CHECK;
use crate::zthabitatmgr::{
    animal_food_target, call_bfunit_tile_cost_vtable_slot, call_vtable_slot_noargs_ret_bool, entity_name_bytes, free_event_vector_buffer,
    hooks_zthabitatmgr, walk_neighbor_tree, walk_tile_list, ZTHabitat, ZTHabitatMgr, MAX_PATH_COST_RVA, RVA_KEEPER_TYPE_CHECK_ARG,
    RVA_ZTFOOD_TYPE_CHECK_ARG,
};

/// `ZTHABITATMGR_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `zthabitatmgr::init()`, and this asserts all of its detours actually report enabled. Without
/// it, a silently-failed `init_detours()` (error logged, game continues on vanilla) would leave the
/// whole battery green while every hooked production path runs vanilla. Runs before the other
/// `ZTHABITATMGR_*`/`ZTHABITAT_*` tests so a wiring failure is visible first.
pub(crate) fn run_zthabitatmgr_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = hooks_zthabitatmgr::status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
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
        |ptr| unsafe { zthabitat::HAS_KEEPER_ASSIGNED.original()(ptr) },
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

/// Comparison test for `ZTHabitat::getGate`: real vs. reimplemented, over every real habitat in the
/// loaded zoo's own `exhibit_array`. Safe as a read-only comparison - real vanilla's own body only reads
/// `entrance_tile_ptr`/`entrance_rotation` and the entrance tile's own fence slots, no mutation.
pub(crate) fn run_habitat_get_gate_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_GATE_LIVE",
        |ptr| unsafe { zthabitat::GET_GATE.original()(ptr) } as u32,
        |habitat| habitat.get_gate(),
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

/// `ZTHabitat::isTank` comparison: the real pole is the vtable `+0x20` slot dispatch itself (exactly
/// what real vanilla's own `isTank` call sites execute - the `ZTTankExhibit` override's
/// constant-`true` stub for tanks, the shared `VF_RETURN_FALSE` stub for everything else), compared
/// against the vtable-identity pointer check [`ZTHabitat::is_tank`] over every real habitat.
/// Read-only - both slot poles are 2-instruction constant-return stubs that never touch `this` or
/// any game state. The base slot itself is deliberately not detoured (3-byte stub, no patch area -
/// see `hooks_zthabitatmgr`'s own un-hooked `is_tank`).
pub(crate) fn run_habitat_is_tank_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_IS_TANK_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut tanks = 0u32;
    let mut non_tanks = 0u32;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let real = unsafe { call_vtable_slot_noargs_ret_bool(ptr, 0x20) };
        if real {
            tanks += 1;
        } else {
            non_tanks += 1;
        }
        let value = habitat.is_tank();
        if value != real {
            failures.push(format!(
                "habitat {} ({:#010x}): real +0x20 slot dispatch = {}, is_tank() = {}",
                i, ptr, real, value
            ));
        }
    }
    // Non-vacuity: the loaded zoo must contain at least one of each kind, or one pole of the
    // virtual dispatch (and one arm of the pointer check) was never actually exercised.
    if tanks == 0 {
        failures.push("non-vacuous assert failed: no tank exhibits in the loaded zoo".to_string());
    }
    if non_tanks == 0 {
        failures.push("non-vacuous assert failed: no non-tank habitats in the loaded zoo".to_string());
    }
    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!("{} (habitats: {}, tanks: {}, non-tanks: {})", test_name, tanks + non_tanks, tanks, non_tanks),
        );
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
        let real = unsafe { bfentity::VF_RETURN1_1.original()(ptr as *const u32, 0) };
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
///   `reviseSpeciesList`, `recalculateCharacteristics`, `ZTViewingArea::updateAmbients`, `updatePortals`,
///   `listen`) is itself either a real vanilla call-through or already covered by its own dedicated live
///   test elsewhere in this file, so there's no independent "real" pole left to diff a return value
///   against without double-driving those side effects. Calls the reimplementation with a small,
///   realistic tick (`16` ms, one frame at 60Hz) on every real, non-tank habitat and only confirms the
///   battery is still alive afterward - the `ambients_begin`/`_end` and `viewing_areas_begin`/`_end`
///   vector walks are the two field offsets this test exists to exercise: a wrong offset there would
///   either read garbage pointers (likely crashing `Ambients::play`/`updateAmbients`) or, if the
///   begin/end pair happened to compare equal by coincidence, silently skip the walk entirely rather
///   than prove anything - so this is a real crash-or-hang check, not a no-op. Skips tanks, matching the
///   etour's own real invocation domain: `ZTTankExhibit` overrides this vtable slot at a separate
///   address (see `ZTHabitat::update`'s own doc comment), so real vanilla never dispatches a tank's tick
///   through the address this file detours.
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

/// Real called before reimpl deliberately, same `characteristics_dirty` ordering rationale as
/// [`run_habitat_get_attractiveness_live_test`] - both `false` (direct-occupant count alone) and `true`
/// (additionally summing every amphibious neighbor's own count) are exercised over every live habitat.
pub(crate) fn run_habitat_get_num_adult_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let direct = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_ADULT_ANIMALS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_ADULT_ANIMALS_0.original()(ptr, false) },
        |habitat| habitat.get_num_adult_animals(false),
    );
    let with_neighbors = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_ADULT_ANIMALS_WITH_NEIGHBORS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_ADULT_ANIMALS_0.original()(ptr, true) },
        |habitat| habitat.get_num_adult_animals(true),
    );
    direct || with_neighbors
}

/// `getNumAdultAnimals`'s species overload is only non-trivial for species actually present, so this
/// derives each habitat's own distinct species ids from its animals (`entity_type+0x1ec`, the same read
/// real vanilla `getSpeciesAnimals` compares against) plus one guaranteed-absent id to exercise the
/// empty-scratch-vector path, then compares real vs reimpl for every id under both `include_neighbors`
/// values - same per-habitat exhaustive shape as [`run_habitat_get_amount_keeper_food_live_test`].
pub(crate) fn run_habitat_get_num_adult_animals_by_species_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NUM_ADULT_ANIMALS_BY_SPECIES_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut species_ids: Vec<i32> = Vec::new();
        for animal_ptr in habitat.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let species_id: i32 = get_from_memory(animal_type_ptr + 0x1ec);
            if !species_ids.contains(&species_id) {
                species_ids.push(species_id);
            }
        }
        species_ids.push(i32::MAX);
        for species_id in species_ids {
            for include_neighbors in [false, true] {
                let real = unsafe { zthabitat::GET_NUM_ADULT_ANIMALS_1.original()(ptr as *const u32, species_id, include_neighbors) };
                let reimpl = habitat.get_num_adult_animals_by_species(species_id, include_neighbors);
                if real != reimpl {
                    failures.push(format!(
                        "habitat {} ({:#010x}), species_id={}, include_neighbors={}: real={}, reimpl={}",
                        i, ptr, species_id, include_neighbors, real, reimpl
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

/// Content comparison (sorted, not pointer-identity) for `getSpeciesAnimals`' own out-param vector,
/// per habitat over every distinct species id present in its animals (`entity_type+0x1ec`) plus one
/// guaranteed-absent id to exercise the empty-vector path - same per-habitat exhaustive shape as
/// [`run_habitat_get_num_adult_animals_by_species_live_test`]. Also asserts the species-matching
/// invariant directly over real vanilla's own output: every animal it returns matches the requested
/// species id. Both scratch vectors are freed afterward via [`free_event_vector_buffer`] (matching
/// each side's own real tail exactly - not a `PoolAlloc::deallocate` call).
pub(crate) fn run_habitat_get_species_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_SPECIES_ANIMALS_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut species_ids: Vec<i32> = Vec::new();
        for animal_ptr in habitat.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let species_id: i32 = get_from_memory(animal_type_ptr + 0x1ec);
            if !species_ids.contains(&species_id) {
                species_ids.push(species_id);
            }
        }
        species_ids.push(i32::MAX);
        for species_id in species_ids {
            let mut real_vector = [0u32; 3];
            unsafe { zthabitat::GET_SPECIES_ANIMALS.original()(ptr as *const u32, species_id, real_vector.as_mut_ptr() as *const i32) };
            let real_animals: Vec<u32> = (real_vector[0]..real_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
            free_event_vector_buffer(real_vector[0], real_vector[2] - real_vector[0]);

            let mut reimpl_vector = [0u32; 3];
            habitat.get_species_animals(species_id, reimpl_vector.as_mut_ptr() as u32);
            let reimpl_animals: Vec<u32> = (reimpl_vector[0]..reimpl_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
            free_event_vector_buffer(reimpl_vector[0], reimpl_vector[2] - reimpl_vector[0]);

            for &animal_ptr in &real_animals {
                let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
                let animal_species: i32 = get_from_memory(animal_type_ptr + 0x1ec);
                if animal_species != species_id {
                    failures.push(format!(
                        "habitat {} ({:#010x}), species_id={}: real returned animal {:#010x} of species {}",
                        i, ptr, species_id, animal_ptr, animal_species
                    ));
                }
            }
            let mut real_sorted = real_animals.clone();
            real_sorted.sort_unstable();
            let mut reimpl_sorted = reimpl_animals.clone();
            reimpl_sorted.sort_unstable();
            if real_sorted != reimpl_sorted {
                failures.push(format!(
                    "habitat {} ({:#010x}), species_id={}: real={:?}, reimpl={:?}",
                    i, ptr, species_id, real_animals, reimpl_animals
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

/// Content comparison (sorted, not pointer-identity) for `getAdultGenderSpeciesAnimals`' own out-param
/// vector, per habitat over every distinct species id present in its animals (`entity_type+0x1ec`)
/// plus one guaranteed-absent id to exercise the empty-vector path - same per-habitat exhaustive
/// shape as [`run_habitat_get_species_animals_live_test`]. Each species is queried under three
/// requested gender strings - `"Female"`/`"Male"` (the two real caller `ZTAnimal::fGetMate` builds
/// from the asking animal's own gender text) and `""` (the zero-length-compare path, matching every
/// adult of the species). The requested string object is built here with the
/// `{start_ptr, end_ptr, buffer_end_ptr}` header layout both sides read (real vanilla's callee reads
/// only the first two dwords). Also asserts the filters' invariants directly over real vanilla's own
/// output: every animal it returns is an adult (`'m'`/`'f'` at `entity_type+0xa4`), matches the
/// requested species id, and carries the requested gender text in its own `animal+0x26c`/`+0x270`
/// span. Both scratch vectors are freed afterward via [`free_event_vector_buffer`] (matching each
/// side's own real tail exactly - not a `PoolAlloc::deallocate` call).
pub(crate) fn run_habitat_get_adult_gender_species_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_ADULT_GENDER_SPECIES_ANIMALS_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut species_ids: Vec<i32> = Vec::new();
        for animal_ptr in habitat.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let species_id: i32 = get_from_memory(animal_type_ptr + 0x1ec);
            if !species_ids.contains(&species_id) {
                species_ids.push(species_id);
            }
        }
        species_ids.push(i32::MAX);
        for species_id in species_ids {
            for gender_text in [&b"Female"[..], &b"Male"[..], &b""[..]] {
                let mut gender_buf = gender_text.to_vec();
                gender_buf.push(0);
                let gender_start = gender_buf.as_ptr() as u32;
                let gender_header: [u32; 3] = [
                    gender_start,
                    gender_start + gender_text.len() as u32,
                    gender_start + gender_text.len() as u32 + 1,
                ];

                let mut real_vector = [0u32; 3];
                unsafe {
                    zthabitat::GET_ADULT_GENDER_SPECIES_ANIMALS.original()(
                        ptr as *const u32,
                        gender_header.as_ptr() as *const i8,
                        species_id,
                        real_vector.as_mut_ptr() as *const i32,
                    )
                };
                let real_animals: Vec<u32> = (real_vector[0]..real_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
                free_event_vector_buffer(real_vector[0], real_vector[2] - real_vector[0]);

                let mut reimpl_vector = [0u32; 3];
                habitat.get_adult_gender_species_animals(gender_header.as_ptr() as u32, species_id, reimpl_vector.as_mut_ptr() as u32);
                let reimpl_animals: Vec<u32> = (reimpl_vector[0]..reimpl_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
                free_event_vector_buffer(reimpl_vector[0], reimpl_vector[2] - reimpl_vector[0]);

                for &animal_ptr in &real_animals {
                    let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
                    let animal_species: i32 = get_from_memory(animal_type_ptr + 0x1ec);
                    let gender_tag: u8 = get_from_memory(get_from_memory::<u32>(animal_type_ptr + 0xa4));
                    let text_start: u32 = get_from_memory(animal_ptr + 0x26c);
                    let text_end: u32 = get_from_memory(animal_ptr + 0x270);
                    let own_text: Vec<u8> = (text_start..text_end).map(get_from_memory::<u8>).collect();
                    if !(gender_tag == b'm' || gender_tag == b'f')
                        || animal_species != species_id
                        || own_text != gender_text
                    {
                        failures.push(format!(
                            "habitat {} ({:#010x}), species_id={}, gender={:?}: real returned animal {:#010x} (species {}, gender tag {}, text {:?})",
                            i, ptr, species_id, gender_text, animal_ptr, animal_species, gender_tag as char, own_text
                        ));
                    }
                }
                let mut real_sorted = real_animals.clone();
                real_sorted.sort_unstable();
                let mut reimpl_sorted = reimpl_animals.clone();
                reimpl_sorted.sort_unstable();
                if real_sorted != reimpl_sorted {
                    failures.push(format!(
                        "habitat {} ({:#010x}), species_id={}, gender={:?}: real={:?}, reimpl={:?}",
                        i, ptr, species_id, gender_text, real_animals, reimpl_animals
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

/// The shared game RNG state's RVA (`DAT_00638060`) that `ZTHabitat::update` and `get_random_animal`
/// advance through the classic MSVC LCG. Re-declared here per the repo's no-shared-consts precedent
/// (`zthabitatmgr.rs` carries the original).
const GAME_RNG_RVA: u32 = 0x00638060 - 0x400000;

/// One MSVC LCG advance over the shared game RNG state: `state = state * 0x343fd + 0x269ec3` with
/// full 32-bit wrap - `zthabitatmgr.rs`'s own `lcg_next`, duplicated per the same no-shared-consts
/// precedent.
fn lcg_next(state: u32) -> u32 {
    state.wrapping_mul(0x343fd).wrapping_add(0x269ec3)
}

/// Shared assertion for [`run_habitat_get_random_animal_live_test`]: one observed draw (`side` is
/// `"real"` or `"reimpl"`) over the habitat's settled `all_animals` array must return the pointer at
/// index `(lcg_next(seed_before) >> 0x10 & 0x7fff) % count`, and the shared game RNG state must read
/// exactly `lcg_next(seed_before)` afterward - or, when `count == 0`, return null and leave the seed
/// untouched. Returns whether the draw failed.
fn assert_habitat_random_animal_draw(
    failure_log: &mut Option<std::fs::File>,
    test_name: &str,
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    begin: u32,
    count: u32,
    seed_before: u32,
    seed_after: u32,
    drawn_ptr: u32,
) -> bool {
    let expected_rng = lcg_next(seed_before);
    let (expected_ptr, rng_ok) = if count == 0 {
        (0, seed_after == seed_before)
    } else {
        let index = ((expected_rng >> 0x10) & 0x7fff) % count;
        (get_from_memory::<u32>(begin + index * 4), seed_after == expected_rng)
    };
    if drawn_ptr == expected_ptr && rng_ok {
        return false;
    }
    let msg = format!(
        "habitat {} ({:#010x}) {} draw: got {:#010x}, expected {:#010x}; rng {:#010x} -> {:#010x}, expected {:#010x}",
        habitat_index, habitat_ptr, side, drawn_ptr, expected_ptr, seed_before, seed_after, expected_rng
    );
    error!("{}: {}", test_name, msg);
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
    }
    true
}

/// Per live habitat, settles any pending `characteristics_dirty` recalculate with one unasserted real
/// vanilla draw first, then checks one reimplementation draw and one further real draw against the
/// shared game RNG state directly: each must return the animal pointer at exactly the
/// `(lcg_next(seed) >> 0x10 & 0x7fff) % count` slot of the settled `all_animals` array and leave
/// `DAT_00638060` at exactly `lcg_next(seed)`. Real vanilla's own draws are asserted against the same
/// formula, so the formula itself is validated against vanilla behavior, not just cross-agreement.
/// The empty-habitat path (`count == 0`: null return, seed untouched) is covered only when the loaded
/// zoo has an animal-free exhibit.
pub(crate) fn run_habitat_get_random_animal_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_RANDOM_ANIMAL_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { zthabitat::GET_RANDOM_ANIMAL.original()(ptr as *const std::ffi::c_void) };
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let count = (end - begin) >> 2;
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };

        let seed_before: u32 = get_from_memory(rng_addr);
        let reimpl_ptr = habitat.get_random_animal();
        fail_flag |= assert_habitat_random_animal_draw(
            failure_log,
            test_name,
            "reimpl",
            i,
            ptr,
            begin,
            count,
            seed_before,
            get_from_memory(rng_addr),
            reimpl_ptr,
        );

        let seed_before: u32 = get_from_memory(rng_addr);
        let real_ptr = unsafe { zthabitat::GET_RANDOM_ANIMAL.original()(ptr as *const std::ffi::c_void) };
        fail_flag |= assert_habitat_random_animal_draw(
            failure_log,
            test_name,
            "real",
            i,
            ptr,
            begin,
            count,
            seed_before,
            get_from_memory(rng_addr),
            real_ptr,
        );
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Shared assertion for [`run_habitat_get_random_tile_live_test`]: one observed draw (`side` is
/// `"real"` or `"reimpl"`) over the habitat's snapshotted owned-tile list must return the payload at
/// index `(lcg_next(seed_before) >> 0x10 & 0x7fff) % count`, and the shared game RNG state must read
/// exactly `lcg_next(seed_before)` afterward - or, when `count == 0`, return null and leave the seed
/// untouched. Returns whether the draw failed.
fn assert_habitat_random_tile_draw(
    failure_log: &mut Option<std::fs::File>,
    test_name: &str,
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    tiles: &[u32],
    seed_before: u32,
    seed_after: u32,
    drawn_ptr: u32,
) -> bool {
    let expected_rng = lcg_next(seed_before);
    let count = tiles.len() as u32;
    let (expected_ptr, rng_ok) = if count == 0 {
        (0, seed_after == seed_before)
    } else {
        let index = ((expected_rng >> 0x10) & 0x7fff) % count;
        (tiles[index as usize], seed_after == expected_rng)
    };
    if drawn_ptr == expected_ptr && rng_ok {
        return false;
    }
    let msg = format!(
        "habitat {} ({:#010x}) {} draw: got {:#010x}, expected {:#010x}; rng {:#010x} -> {:#010x}, expected {:#010x}",
        habitat_index, habitat_ptr, side, drawn_ptr, expected_ptr, seed_before, seed_after, expected_rng
    );
    error!("{}: {}", test_name, msg);
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
    }
    true
}

/// Per live habitat, checks one reimplementation draw and one real vanilla draw against the shared
/// game RNG state directly: each must return the `BFTile*` payload at exactly the
/// `(lcg_next(seed) >> 0x10 & 0x7fff) % count` slot of the habitat's owned-tile list and leave
/// `DAT_00638060` at exactly `lcg_next(seed)`. The list is snapshotted once per habitat via the same
/// sentinel walk real vanilla's `getSize(this, false)` counts (both draws read that synchronous,
/// unmutated list, so the snapshot is both sides' shared ground truth). Real vanilla's own draws are
/// asserted against the same formula, so the formula itself is validated against vanilla behavior,
/// not just cross-agreement. Unlike the animal draw test there is no settle draw - `getRandomTile`'s
/// count path (`getSize`, false arm) touches no `characteristics_dirty` recalculate. The
/// empty-habitat path (`count == 0`: null return, seed untouched) is covered only when the loaded zoo
/// has a tile-free exhibit.
pub(crate) fn run_habitat_get_random_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_RANDOM_TILE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };

        let seed_before: u32 = get_from_memory(rng_addr);
        let reimpl_ptr = habitat.get_random_tile();
        fail_flag |= assert_habitat_random_tile_draw(
            failure_log,
            test_name,
            "reimpl",
            i,
            ptr,
            &tiles,
            seed_before,
            get_from_memory(rng_addr),
            reimpl_ptr,
        );

        let seed_before: u32 = get_from_memory(rng_addr);
        let real_ptr = unsafe { zthabitat::GET_RANDOM_TILE.original()(ptr as *const u32) };
        fail_flag |= assert_habitat_random_tile_draw(
            failure_log,
            test_name,
            "real",
            i,
            ptr,
            &tiles,
            seed_before,
            get_from_memory(rng_addr),
            real_ptr,
        );
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// This test's own oracle for `BFMap::isCloseDirection` (`BFMap_isCloseDirection.c`): `actual` sits
/// within one step of `reference` on the 8-direction compass. Written out from the decompile's own
/// per-direction chain rather than a modular-distance formula - the decompile compares exact integer
/// values, so an out-of-range `reference` (the `-1`/`0xffffffff` sentinel the rotation fields use)
/// matches nothing, which a `rem_euclid` formulation would get wrong.
fn is_close_direction(reference: i32, actual: i32) -> bool {
    match reference {
        0 => actual == 0 || actual == 1 || actual == 7,
        1 => actual == 0 || actual == 1 || actual == 2,
        2 => actual == 1 || actual == 2 || actual == 3,
        3 => actual == 2 || actual == 3 || actual == 4,
        4 => actual == 3 || actual == 4 || actual == 5,
        5 => actual == 4 || actual == 5 || actual == 6,
        6 => actual == 5 || actual == 6 || actual == 7,
        7 => actual == 6 || actual == 7 || actual == 0,
        _ => false,
    }
}

/// Shared assertion for the random tile-draw tests: one observed draw (`side` is `"real"` or
/// `"reimpl"`) must return the tile pointer at index `(lcg_next(seed_before) >> 0x10 & 0x7fff) % count`
/// of `expected_list` and leave the shared game RNG state at exactly `lcg_next(seed_before)` afterward.
/// `expected_list` is the side's own candidate set - for the directional tests, the full owned-tile
/// list both sides' `getRandomTile` fallback draws from when that set comes out empty; for
/// `getRandomClearTile` overload 1, the pool as-is (an empty pool expects a null return and an
/// untouched seed - no fallback exists there). Returns whether the draw failed.
fn assert_lcg_tile_draw(
    failure_log: &mut Option<std::fs::File>,
    test_name: &str,
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    expected_list: &[u32],
    seed_before: u32,
    seed_after: u32,
    drawn_ptr: u32,
) -> bool {
    let expected_rng = lcg_next(seed_before);
    let (expected_ptr, rng_ok) = if expected_list.is_empty() {
        (0, seed_after == seed_before)
    } else {
        let index = ((expected_rng >> 0x10) & 0x7fff) % expected_list.len() as u32;
        (expected_list[index as usize], seed_after == expected_rng)
    };
    if drawn_ptr == expected_ptr && rng_ok {
        return false;
    }
    let msg = format!(
        "habitat {} ({:#010x}) {} draw: got {:#010x}, expected {:#010x} (of {} candidates); rng {:#010x} -> {:#010x}, expected {:#010x}",
        habitat_index, habitat_ptr, side, drawn_ptr, expected_ptr, expected_list.len(), seed_before, seed_after, expected_rng
    );
    error!("{}: {}", test_name, msg);
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
    }
    true
}

/// Builds one side's expected candidate list for [`run_habitat_get_random_tile_in_direction_live_test`]:
/// every snapshotted owned tile whose real `BFMap::getDirection` direction from `from_tile` passes the
/// [`is_close_direction`] oracle for `reference`.
fn directional_tile_candidates(tiles: &[u32], from_tile: u32, reference: i32) -> Vec<u32> {
    tiles
        .iter()
        .copied()
        .filter(|&tile| {
            let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(from_tile as i32, tile as i32) };
            is_close_direction(reference, dir)
        })
        .collect()
}

/// Per live habitat, sweeps every real `EDirection` (0-7) plus the `-1`/`0xffffffff` "no direction"
/// sentinel the rotation fields carry, checking one reimplementation draw and one real vanilla draw per
/// direction against the shared game RNG state directly. Each side's expected candidate set is rebuilt
/// from the [`is_close_direction`] oracle over the real `BFMap::getDirection` results (undetoured
/// vanilla, the same call both sides make) over the habitat's snapshotted owned-tile list, and each
/// draw must return the `(lcg_next(seed) >> 0x10 & 0x7fff) % count` slot of that set - or, when the
/// set is empty, of the full list both sides' `getRandomTile` fallback draws from - leaving
/// `DAT_00638060` at exactly `lcg_next(seed)`. The from-tile is the habitat's own first owned tile, so
/// `getDirection` always sees a real `BFTile*` (the null-from-tile path is not forced: real vanilla
/// resolves it to the same empty candidate set this oracle already covers).
pub(crate) fn run_habitat_get_random_tile_in_direction_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_RANDOM_TILE_IN_DIRECTION_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let from_tile = tiles.first().copied().unwrap_or(0);
        for direction in [0u32, 1, 2, 3, 4, 5, 6, 7, 0xffff_ffff] {
            let candidates = directional_tile_candidates(&tiles, from_tile, direction as i32);
            let expected_list: &[u32] = if candidates.is_empty() { &tiles } else { &candidates };

            let seed_before: u32 = get_from_memory(rng_addr);
            let reimpl_ptr = habitat.get_random_tile_in_direction(from_tile, direction);
            fail_flag |= assert_lcg_tile_draw(
                failure_log,
                test_name,
                "reimpl",
                i,
                ptr,
                expected_list,
                seed_before,
                get_from_memory(rng_addr),
                reimpl_ptr,
            );

            let seed_before: u32 = get_from_memory(rng_addr);
            let real_ptr =
                unsafe { zthabitat::GET_RANDOM_TILE_IN_DIRECTION.original()(ptr as *const u32, from_tile as *const u32, direction) } as u32;
            fail_flag |= assert_lcg_tile_draw(
                failure_log,
                test_name,
                "real",
                i,
                ptr,
                expected_list,
                seed_before,
                get_from_memory(rng_addr),
                real_ptr,
            );
        }
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Per live habitat, drives both sides with each of the habitat's own animals (real `ZTUnit`
/// subclasses, the function's real caller shape - `ZTUnit_getRandomClearTileAhead.c`'s own parameter),
/// checking one reimplementation draw and one real vanilla draw per unit against the shared game RNG
/// state directly. Each side's expected candidate set is rebuilt from the [`is_close_direction`]
/// oracle over the unit's real state - its `+0x12c` facing (the `(rotation - 4) & 7` opposite, the
/// `0xffffffff` sentinel left as-is), its real current tile (`BFEntity::getTile`, undetoured), the
/// real vtable `+0x164` path-cost dispatch ([`call_bfunit_tile_cost_vtable_slot`], real
/// `getTerrainCost`-family vanilla) against the shared [`MAX_PATH_COST_RVA`] bound - over the
/// habitat's snapshotted owned-tile list. Each draw must return the
/// `(lcg_next(seed) >> 0x10 & 0x7fff) % count` slot of that set - or, when empty, of the full list
/// both sides' `getRandomTile` fallback draws from - leaving `DAT_00638060` at exactly
/// `lcg_next(seed)`. Animal-free habitats contribute no draws (staff/keeper units are not iterated -
/// they are not members of `all_animals`).
pub(crate) fn run_habitat_get_random_clear_tile_ahead_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_RANDOM_CLEAR_TILE_AHEAD_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        for u in 0..(end.wrapping_sub(begin)) / 4 {
            let unit: u32 = get_from_memory(begin + u * 4);
            if unit == 0 {
                continue;
            }
            let rotation: u32 = get_from_memory(unit + 0x12c);
            let heading = if rotation == 0xffff_ffff { rotation } else { rotation.wrapping_sub(4) & 7 };
            let unit_tile = unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) };
            let candidates: Vec<u32> = tiles
                .iter()
                .copied()
                .filter(|&tile| {
                    let cost = unsafe { call_bfunit_tile_cost_vtable_slot(unit, tile) };
                    if cost >= max_cost {
                        return false;
                    }
                    let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(unit_tile, tile as i32) };
                    !is_close_direction(heading as i32, dir)
                })
                .collect();
            let expected_list: &[u32] = if candidates.is_empty() { &tiles } else { &candidates };

            let seed_before: u32 = get_from_memory(rng_addr);
            let reimpl_ptr = habitat.get_random_clear_tile_ahead(unit);
            fail_flag |= assert_lcg_tile_draw(
                failure_log,
                test_name,
                "reimpl",
                i,
                ptr,
                expected_list,
                seed_before,
                get_from_memory(rng_addr),
                reimpl_ptr,
            );

            let seed_before: u32 = get_from_memory(rng_addr);
            let real_ptr = unsafe { zthabitat::GET_RANDOM_CLEAR_TILE_AHEAD.original()(ptr as *const u32, unit as *const u32) } as u32;
            fail_flag |= assert_lcg_tile_draw(
                failure_log,
                test_name,
                "real",
                i,
                ptr,
                expected_list,
                seed_before,
                get_from_memory(rng_addr),
                real_ptr,
            );
        }
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Per live habitat, drives both sides of `addClearTiles` over the full `(animal, check_path)` cross
/// product - the null animal (real vanilla's own short-circuit: neither the cost gate nor the path
/// check runs) plus each of the habitat's own animals (real `ZTAnimal` subclasses, the function's real
/// caller shape via `getRandomClearTile`), each with `check_path` false and true. The extracted tile
/// lists are compared element-for-element in owned-tile-list order - the walk both sides iterate is
/// the same synchronous list, so order is deterministic. Also asserts the decompile's own filter
/// contract directly over real vanilla's output: every returned tile's four direct-entity slots
/// (`+0x4..+0x10`) are null and its entity list at `+0x0` is empty. `check_path=true` runs real
/// vanilla's own `BFAIMgr::checkPath` pathfinder on both sides - the same consecutive-call shape
/// real vanilla's own `getRandomClearTile` already performs per subhabitat, so back-to-back runs
/// can't shift either side's result. Both scratch vectors are freed afterward via
/// [`free_event_vector_buffer`] (matching each side's own real tail exactly - not a
/// `PoolAlloc::deallocate` call).
pub(crate) fn run_habitat_add_clear_tiles_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_ADD_CLEAR_TILES_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let animals: Vec<u32> = (0..(end.wrapping_sub(begin)) / 4)
            .map(|u| get_from_memory::<u32>(begin + u * 4))
            .filter(|&a| a != 0)
            .collect();
        for animal in std::iter::once(0u32).chain(animals) {
            for check_path in [false, true] {
                let mut real_vector = [0u32; 3];
                unsafe {
                    zthabitat::ADD_CLEAR_TILES.original()(ptr as *const u32, real_vector.as_mut_ptr() as i32, animal as *const u32, check_path)
                };
                let real_tiles: Vec<u32> = (real_vector[0]..real_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
                free_event_vector_buffer(real_vector[0], real_vector[2] - real_vector[0]);

                let mut reimpl_vector = [0u32; 3];
                habitat.add_clear_tiles(reimpl_vector.as_mut_ptr() as u32, animal, check_path);
                let reimpl_tiles: Vec<u32> = (reimpl_vector[0]..reimpl_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
                free_event_vector_buffer(reimpl_vector[0], reimpl_vector[2] - reimpl_vector[0]);

                for &tile in &real_tiles {
                    let entity_slots_clear = [0x4u32, 0x8, 0xc, 0x10].iter().all(|&off| get_from_memory::<u32>(tile + off) == 0);
                    let head: u32 = get_from_memory(tile);
                    if !entity_slots_clear || get_from_memory::<u32>(head) != head {
                        failures.push(format!(
                            "habitat {} ({:#010x}), animal={:#010x}, check_path={}: real returned occupied tile {:#010x}",
                            i, ptr, animal, check_path, tile
                        ));
                    }
                }
                if real_tiles != reimpl_tiles {
                    failures.push(format!(
                        "habitat {} ({:#010x}), animal={:#010x}, check_path={}: real ({:?}) != reimpl ({:?})",
                        i, ptr, animal, check_path, real_tiles, reimpl_tiles
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

/// Builds one expected candidate pool for the `getRandomClearTile` overload tests: real vanilla
/// `addClearTiles` (`.original()` call-through - the trampoline in the debug battery, the raw address
/// in release, where it re-enters the port that `ZTHABITAT_ADD_CLEAR_TILES_MATCHES_REAL_LIVE`
/// cross-validated) appended into one shared scratch vector exactly as vanilla overload 1 builds its
/// own - `habitat_ptr` first, then each amphibious neighbor in `neighbors` ([`walk_neighbor_tree`])
/// order when `subhabs` is set. The scratch buffer is freed via [`free_event_vector_buffer`]
/// (vanilla's own tail shape) before the extracted list is returned.
fn vanilla_clear_tile_pool(habitat_ptr: u32, neighbors: &[u32], animal: u32, check_path: bool, subhabs: bool) -> Vec<u32> {
    let mut scratch_vector = [0u32; 3];
    unsafe {
        zthabitat::ADD_CLEAR_TILES.original()(habitat_ptr as *const u32, scratch_vector.as_mut_ptr() as i32, animal as *const u32, check_path);
    }
    if subhabs {
        for &neighbor in neighbors {
            unsafe {
                zthabitat::ADD_CLEAR_TILES.original()(neighbor as *const u32, scratch_vector.as_mut_ptr() as i32, animal as *const u32, check_path);
            }
        }
    }
    let pool: Vec<u32> = (scratch_vector[0]..scratch_vector[1]).step_by(4).map(get_from_memory::<u32>).collect();
    free_event_vector_buffer(scratch_vector[0], scratch_vector[2].wrapping_sub(scratch_vector[0]));
    pool
}

/// Per live habitat, drives both sides of `getRandomClearTile` overload 1 over the full
/// `(animal, check_path, subhabs)` cross product - the null animal plus each of the habitat's own
/// animals (the real caller `ZTAnimal::fCheckReproduction` passes a real animal; the null shape
/// reaches it through overload 0's delegation on animal-free exhibits), each with `check_path` false
/// and true and `subhabs` false and true. Each side's expected pool is rebuilt per draw via
/// [`vanilla_clear_tile_pool`] - the same synchronous, unmutated tile state both draws walk - and
/// each draw must return the `(lcg_next(seed) >> 0x10 & 0x7fff) % count` slot of that pool, or null
/// with the seed untouched when the pool comes out empty, leaving `DAT_00638060` at exactly
/// `lcg_next(seed)` ([`assert_lcg_tile_draw`]).
pub(crate) fn run_habitat_get_random_clear_tile_for_animal_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_RANDOM_CLEAR_TILE_FOR_ANIMAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let neighbors: Vec<u32> = walk_neighbor_tree(habitat.amphibious_neighbors_head)
            .map(|node| get_from_memory::<u32>(node + 0x10))
            .collect();
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let animals: Vec<u32> = (0..(end.wrapping_sub(begin)) / 4)
            .map(|u| get_from_memory::<u32>(begin + u * 4))
            .filter(|&a| a != 0)
            .collect();
        for animal in std::iter::once(0u32).chain(animals) {
            for check_path in [false, true] {
                for subhabs in [false, true] {
                    let pool = vanilla_clear_tile_pool(ptr, &neighbors, animal, check_path, subhabs);

                    let seed_before: u32 = get_from_memory(rng_addr);
                    let reimpl_ptr = habitat.get_random_clear_tile_for_animal(animal, check_path, subhabs);
                    if assert_lcg_tile_draw(
                        failure_log,
                        test_name,
                        "reimpl",
                        i,
                        ptr,
                        &pool,
                        seed_before,
                        get_from_memory(rng_addr),
                        reimpl_ptr,
                    ) {
                        failures.push(format!("habitat {} (pool of {}), animal={:#010x}, check_path={}, subhabs={}", i, pool.len(), animal, check_path, subhabs));
                    }

                    let seed_before: u32 = get_from_memory(rng_addr);
                    let real_ptr =
                        unsafe { zthabitat::GET_RANDOM_CLEAR_TILE_1.original()(ptr as *const u32, animal as *const u32, check_path, subhabs) };
                    if assert_lcg_tile_draw(
                        failure_log,
                        test_name,
                        "real",
                        i,
                        ptr,
                        &pool,
                        seed_before,
                        get_from_memory(rng_addr),
                        real_ptr,
                    ) {
                        failures.push(format!("habitat {} (pool of {}), animal={:#010x}, check_path={}, subhabs={}", i, pool.len(), animal, check_path, subhabs));
                    }
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

/// Asserts one observed overload-0 composition draw against the full two-step LCG chain over
/// `seed_before`: the `getRandomAnimal` draw (its `(seed >> 0x10 & 0x7fff) % count` slot of the
/// settled `all_animals` array - raw slots, nulls included, exactly what real vanilla indexes - or
/// null with no advance when the array is empty), then the overload-1 pick over the
/// [`vanilla_clear_tile_pool`] built from exactly that animal. The expected final seed is one advance
/// per non-empty stage: two total when both drew, one when only the pool was non-empty, zero when
/// neither was. The pool is built inside the assertion (the draws themselves mutate no tile state, so
/// it sees the same state both sides drew from). Returns whether the draw failed.
fn assert_clear_tile_composition_draw(
    failure_log: &mut Option<std::fs::File>,
    test_name: &str,
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    animals: &[u32],
    neighbors: &[u32],
    seed_before: u32,
    seed_after: u32,
    drawn_ptr: u32,
) -> bool {
    let count = animals.len() as u32;
    let (seed_after_animal, animal) = if count != 0 {
        let seed = lcg_next(seed_before);
        let index = ((seed >> 0x10) & 0x7fff) % count;
        (seed, animals[index as usize])
    } else {
        (seed_before, 0)
    };
    let pool = vanilla_clear_tile_pool(habitat_ptr, neighbors, animal, false, true);
    let (expected_ptr, expected_seed) = if pool.is_empty() {
        (0, seed_after_animal)
    } else {
        let seed = lcg_next(seed_after_animal);
        let index = ((seed >> 0x10) & 0x7fff) % pool.len() as u32;
        (pool[index as usize], seed)
    };
    if drawn_ptr == expected_ptr && seed_after == expected_seed {
        return false;
    }
    let msg = format!(
        "habitat {} ({:#010x}) {} composition draw: got {:#010x}, expected {:#010x} (pool of {}, animal {:#010x} of {} slots); rng {:#010x} -> {:#010x}, expected {:#010x}",
        habitat_index, habitat_ptr, side, drawn_ptr, expected_ptr, pool.len(), animal, count, seed_before, seed_after, expected_seed
    );
    error!("{}: {}", test_name, msg);
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
    }
    true
}

/// Per live habitat, drives both sides of `getRandomClearTile` overload 0 - the delegation
/// composition: one `getRandomAnimal` draw forwarded into overload 1 (driven with `subhabs` set so
/// the pool spans the amphibious neighbors too; `check_path` clear, matching both real callers' own
/// `(false, false)` arguments). One unasserted real draw settles any pending `characteristics_dirty`
/// recalculate first - the recalc can rebuild `all_animals`, so the test's count snapshot must be
/// post-recalc - then each side's observed draw is asserted against the full two-step chain
/// ([`assert_clear_tile_composition_draw`]): returned tile AND final `DAT_00638060`. The real side's
/// picked tile is `zthabitat::GET_RANDOM_CLEAR_TILE_0`'s own (EAX-riding) return.
pub(crate) fn run_habitat_get_random_clear_tile_default_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_RANDOM_CLEAR_TILE_DEFAULT_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut fail_flag = false;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let neighbors: Vec<u32> = walk_neighbor_tree(habitat.amphibious_neighbors_head)
            .map(|node| get_from_memory::<u32>(node + 0x10))
            .collect();

        // Settle: one unasserted real draw triggers any pending lazy recalculate and leaves
        // `all_animals` exactly as both observed draws will see it.
        unsafe { zthabitat::GET_RANDOM_CLEAR_TILE_0.original()(ptr as *const u32, false, true) };

        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let animals: Vec<u32> = (0..(end.wrapping_sub(begin)) / 4).map(|u| get_from_memory::<u32>(begin + u * 4)).collect();

        let seed_before: u32 = get_from_memory(rng_addr);
        let reimpl_ptr = habitat.get_random_clear_tile_default(false, true);
        fail_flag |= assert_clear_tile_composition_draw(
            failure_log,
            test_name,
            "reimpl",
            i,
            ptr,
            &animals,
            &neighbors,
            seed_before,
            get_from_memory(rng_addr),
            reimpl_ptr,
        );

        let seed_before: u32 = get_from_memory(rng_addr);
        let real_ptr = unsafe { zthabitat::GET_RANDOM_CLEAR_TILE_0.original()(ptr as *const u32, false, true) } as u32;
        fail_flag |= assert_clear_tile_composition_draw(
            failure_log,
            test_name,
            "real",
            i,
            ptr,
            &animals,
            &neighbors,
            seed_before,
            get_from_memory(rng_addr),
            real_ptr,
        );
    }
    if !fail_flag {
        write_success_line(failure_log, test_name);
    }
    fail_flag
}

/// Shared assertion for [`run_habitat_get_adjacent_clear_tile_live_test`]: unlike [`assert_lcg_tile_draw`],
/// an empty candidate set expects `base_tile` itself back with the seed untouched (this function's own
/// RNG-free pass-through), not a fallback draw from a wider list. A non-empty set expects the
/// `(lcg_next(seed_before) >> 0x10 & 0x7fff) % count` slot and the seed advanced to exactly that value.
fn assert_adjacent_clear_tile_draw(
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    base_tile: u32,
    candidates: &[u32],
    seed_before: u32,
    seed_after: u32,
    drawn_ptr: u32,
) -> Option<String> {
    let (expected_ptr, rng_ok) = if candidates.is_empty() {
        (base_tile, seed_after == seed_before)
    } else {
        let expected_rng = lcg_next(seed_before);
        let index = ((expected_rng >> 0x10) & 0x7fff) % candidates.len() as u32;
        (candidates[index as usize], seed_after == expected_rng)
    };
    if drawn_ptr == expected_ptr && rng_ok {
        return None;
    }
    Some(format!(
        "habitat {} ({:#010x}) {} draw at base_tile {:#010x}: got {:#010x}, expected {:#010x} (of {} candidates); rng {:#010x} -> {:#010x}",
        habitat_index, habitat_ptr, side, base_tile, drawn_ptr, expected_ptr, candidates.len(), seed_before, seed_after
    ))
}

/// Per live habitat with at least one real animal (`unit` = the habitat's own first animal - a real
/// `ZTUnit`-derived entity, matching real vanilla's own unchecked dereference of it; animal-free
/// habitats are skipped, same rationale as this file's other unit-taking tests), sweeps every owned
/// tile as `base_tile` and checks one reimplementation call and one real vanilla call directly against
/// the shared game RNG state via [`assert_adjacent_clear_tile_draw`]. Each side's expected candidate set
/// is rebuilt from a plain re-scan of `base_tile`'s 8 neighbor coordinates (real vanilla's own
/// dx-outer/dy-inner loop order) against the live map's own bounds, [`ZTHabitatMgr::get_habitat_ptr`]
/// (both sides, undetoured - the same raw "0 == 0 ownerless tiles match" comparison real vanilla
/// performs), and the real vtable `+0x164` path-cost dispatch ([`call_bfunit_tile_cost_vtable_slot`])
/// against the shared [`MAX_PATH_COST_RVA`] sentinel by exact equality - the same oracle
/// [`ZTHabitat::get_adjacent_clear_tile`]'s own doc comment describes.
pub(crate) fn run_habitat_get_adjacent_clear_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_ADJACENT_CLEAR_TILE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let Some(unit) = (0..(end.wrapping_sub(begin)) / 4).map(|u| get_from_memory::<u32>(begin + u * 4)).find(|&a| a != 0) else {
            continue;
        };
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        for &base_tile in &tiles {
            let base_x: i32 = get_from_memory(base_tile + 0x34);
            let base_y: i32 = get_from_memory(base_tile + 0x38);
            let base_habitat = habitat_mgr.get_habitat_ptr(base_x, base_y);
            let mut candidates: Vec<u32> = Vec::new();
            for dx in -1i32..=1 {
                for dy in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let cand_x = base_x + dx;
                    let cand_y = base_y + dy;
                    if cand_x < 0 || cand_y < 0 || cand_x as u32 >= world.map_x_size || cand_y as u32 >= world.map_y_size {
                        continue;
                    }
                    if habitat_mgr.get_habitat_ptr(cand_x, cand_y) != base_habitat {
                        continue;
                    }
                    let candidate_tile_ptr = world.get_tile_ptr(cand_x as u32, cand_y as u32);
                    let cost = unsafe { call_bfunit_tile_cost_vtable_slot(unit, candidate_tile_ptr) };
                    if cost == max_cost {
                        continue;
                    }
                    candidates.push(candidate_tile_ptr);
                }
            }

            let seed_before: u32 = get_from_memory(rng_addr);
            let reimpl_ptr = ZTHabitat::get_adjacent_clear_tile(unit, base_tile);
            if let Some(msg) =
                assert_adjacent_clear_tile_draw("reimpl", i, ptr, base_tile, &candidates, seed_before, get_from_memory(rng_addr), reimpl_ptr)
            {
                failures.push(msg);
            }

            let seed_before: u32 = get_from_memory(rng_addr);
            let real_ptr = unsafe { zthabitat::GET_ADJACENT_CLEAR_TILE.original()(unit as *const u32, base_tile as *const u32) } as u32;
            if let Some(msg) =
                assert_adjacent_clear_tile_draw("real", i, ptr, base_tile, &candidates, seed_before, get_from_memory(rng_addr), real_ptr)
            {
                failures.push(msg);
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

/// Oracle for [`run_habitat_get_nearest_clear_tile_live_test`]: the first owned tile in walk order
/// achieving the strict minimum squared distance to `unit_tile` among tiles passing the full filter -
/// 4 direct-entity slots null, entity list empty, the real path-cost dispatch strictly below
/// [`MAX_PATH_COST_RVA`], and - when `random_animal` is non-null - real `BFAIMgr::checkPath` from the
/// drawn animal's tile with that animal as mover. Returns the expected tile (`0` when nothing
/// qualifies). `random_animal` is the caller's *predicted* `getRandomAnimal` slot - the port and real
/// vanilla both draw it internally, so the oracle must gate on exactly that animal.
fn nearest_clear_tile_oracle(tiles: &[u32], unit_ptr: u32, unit_tile: u32, random_animal: u32, max_cost: i32) -> u32 {
    let ai_mgr_ptr = globals().ztaimgr_ptr() as u32;
    let animal_tile = if random_animal != 0 {
        (unsafe { BFENTITY_GET_TILE.original()(random_animal as *const u32) }) as u32
    } else {
        0
    };
    let mut best_tile = 0u32;
    let mut best_dist = 0x7fff_ffffi32;
    for &tile in tiles {
        let dist: i32 = if tile == 0 || unit_tile == 0 {
            0x7fff_ffff
        } else {
            let dx: i32 = get_from_memory::<i32>(unit_tile + 0x34).wrapping_sub(get_from_memory::<i32>(tile + 0x34));
            let dy: i32 = get_from_memory::<i32>(unit_tile + 0x38).wrapping_sub(get_from_memory::<i32>(tile + 0x38));
            dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
        };
        if dist >= best_dist {
            continue;
        }
        let entity_slots_clear = [0x4u32, 0x8, 0xc, 0x10].iter().all(|&off| get_from_memory::<u32>(tile + off) == 0);
        if !entity_slots_clear {
            continue;
        }
        let head: u32 = get_from_memory(tile);
        if get_from_memory::<u32>(head) != head {
            continue;
        }
        if unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, tile) } >= max_cost {
            continue;
        }
        if random_animal != 0 {
            let reachable = low_byte_bool(unsafe {
                BFAIMGR_CHECK_PATH.original()(ai_mgr_ptr as *const u32, animal_tile as *const u32, tile as *const u32, random_animal as *const u32)
            });
            if !reachable {
                continue;
            }
        }
        best_tile = tile;
        best_dist = dist;
    }
    best_tile
}

/// Per live habitat with at least one real animal (`unit` = the habitat's own first animal, a real
/// `ZTUnit`-derived entity; animal-free habitats are skipped, same rationale as this file's other
/// unit-taking tests), checks one reimplementation call and one restored-seed real vanilla call
/// against a fully independent oracle: settles any pending `characteristics_dirty` recalculate with
/// one unasserted real vanilla draw first (the function's own internal `getRandomAnimal` would
/// otherwise recalculate inside the asserted draws), predicts the internal `getRandomAnimal` slot
/// from the current seed without burning a draw (raw slots, nulls included - exactly what vanilla
/// indexes; null with no advance when the array is empty), rebuilds the expected best tile with
/// [`nearest_clear_tile_oracle`], and requires both sides to return it while leaving `DAT_00638060`
/// at exactly `lcg_next(seed)` when the habitat has animals - unchanged otherwise (the guards
/// precede the draw). Real vanilla's own call is asserted against the same oracle, so the oracle
/// itself is validated against vanilla behavior, not just cross-agreement.
pub(crate) fn run_habitat_get_nearest_clear_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NEAREST_CLEAR_TILE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { zthabitat::GET_RANDOM_ANIMAL.original()(ptr as *const std::ffi::c_void) };
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let count = (end.wrapping_sub(begin)) / 4;
        let Some(unit) = (0..count).map(|u| get_from_memory::<u32>(begin + u * 4)).find(|&a| a != 0) else {
            continue;
        };
        let unit_tile = unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) } as u32;
        if unit_tile == 0 {
            continue;
        }
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        let seed_at_predict: u32 = get_from_memory(rng_addr);
        let expected_rng = lcg_next(seed_at_predict);
        let random_animal = if count == 0 {
            0
        } else {
            let index = ((expected_rng >> 0x10) & 0x7fff) % count;
            get_from_memory::<u32>(begin + index * 4)
        };
        let expected_tile = nearest_clear_tile_oracle(&tiles, unit, unit_tile, random_animal, max_cost);

        let seed_before: u32 = get_from_memory(rng_addr);
        let reimpl_ptr = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_nearest_clear_tile(unit);
        let seed_after: u32 = get_from_memory(rng_addr);
        let rng_ok = if count == 0 { seed_after == seed_before } else { seed_after == expected_rng };
        if reimpl_ptr != expected_tile || !rng_ok {
            failures.push(format!(
                "habitat {} ({:#010x}) reimpl: got {:#010x}, expected {:#010x}; rng {:#010x} -> {:#010x} (expected {:#010x}); animals {}, predicted animal {:#010x}",
                i, ptr, reimpl_ptr, expected_tile, seed_before, seed_after, expected_rng, count, random_animal
            ));
        }

        save_to_memory(rng_addr, seed_before);
        let real_ptr = unsafe { zthabitat::GET_NEAREST_CLEAR_TILE.original()(ptr as *const u32, unit as *const u32) } as u32;
        let seed_after: u32 = get_from_memory(rng_addr);
        let rng_ok = if count == 0 { seed_after == seed_before } else { seed_after == expected_rng };
        if real_ptr != expected_tile || !rng_ok {
            failures.push(format!(
                "habitat {} ({:#010x}) real: got {:#010x}, expected {:#010x}; rng {:#010x} -> {:#010x} (expected {:#010x}); animals {}, predicted animal {:#010x}",
                i, ptr, real_ptr, expected_tile, seed_before, seed_after, expected_rng, count, random_animal
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

/// Oracle candidate set for [`run_habitat_get_near_clear_tile_live_test`]: the owned tiles passing
/// all 8 `.asm` checks in walk order - the raw `unit+0x27c..+0x280` reserved-vector scan
/// (`ZTStaff::isInvalidTile`'s Windows inline), 4 direct-entity slots null, entity list empty, the
/// real path-cost dispatch full-width-unequal to [`MAX_PATH_COST_RVA`], not the resolved gate tile
/// (a null gate tile passes every candidate), squared distance to `unit_tile` below 10 (unsigned),
/// real `BFAIMgr::checkPath` from `animal_tile` when the mover is non-null, and the `tile+0x85 & 4`
/// flag clear. Seed-independent - the function draws nothing before its pick.
fn near_clear_tile_candidates(tiles: &[u32], unit_ptr: u32, unit_tile: u32, animal_ptr: u32, gate_tile_ptr: u32, max_cost: i32) -> Vec<u32> {
    let ai_mgr_ptr = globals().ztaimgr_ptr() as u32;
    let animal_tile = if animal_ptr != 0 {
        (unsafe { BFENTITY_GET_TILE.original()(animal_ptr as *const u32) }) as u32
    } else {
        0
    };
    let reserved_begin: u32 = get_from_memory(unit_ptr + 0x27c);
    let reserved_end: u32 = get_from_memory(unit_ptr + 0x280);
    let mut candidates = Vec::new();
    for &tile in tiles {
        let mut reserved = false;
        let mut cursor = reserved_begin;
        while cursor != reserved_end {
            if get_from_memory::<u32>(cursor) == tile {
                reserved = true;
                break;
            }
            cursor += 4;
        }
        if reserved {
            continue;
        }
        let entity_slots_clear = [0x4u32, 0x8, 0xc, 0x10].iter().all(|&off| get_from_memory::<u32>(tile + off) == 0);
        if !entity_slots_clear {
            continue;
        }
        let head: u32 = get_from_memory(tile);
        if get_from_memory::<u32>(head) != head {
            continue;
        }
        if unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, tile) } == max_cost {
            continue;
        }
        if tile == gate_tile_ptr {
            continue;
        }
        let dx: i32 = get_from_memory::<i32>(unit_tile + 0x34).wrapping_sub(get_from_memory::<i32>(tile + 0x34));
        let dy: i32 = get_from_memory::<i32>(unit_tile + 0x38).wrapping_sub(get_from_memory::<i32>(tile + 0x38));
        let dist = (dx.wrapping_mul(dx) as u32).wrapping_add(dy.wrapping_mul(dy) as u32);
        if dist >= 10 {
            continue;
        }
        if animal_ptr != 0 {
            let reachable = low_byte_bool(unsafe {
                BFAIMGR_CHECK_PATH.original()(ai_mgr_ptr as *const u32, animal_tile as *const u32, tile as *const u32, animal_ptr as *const u32)
            });
            if !reachable {
                continue;
            }
        }
        if get_from_memory::<u8>(tile + 0x85) & 4 != 0 {
            continue;
        }
        candidates.push(tile);
    }
    candidates
}

/// With `unit` = the first live `ZTKeeper` in the world's `entity_array` (real caller shape
/// `ZTGoalPutFood::decide`: the keeper plus its target animal - and required, not just faithful: the
/// function reads the `ZTStaff`-only reserved-tile vector at `unit+0x27c..+0x280`, which on any
/// non-staff unit is unrelated data; the test skips when the zoo has no keeper), drives both sides
/// of `getNearClearTile` per live habitat over the null mover plus each of the habitat's own animals. Each side's expected candidate set is rebuilt per draw via [`near_clear_tile_candidates`]
/// with the port's own resolved gate tile - seed-independent, since the function draws nothing before
/// its pick. A non-empty set expects the `(lcg_next(seed) >> 0x10 & 0x7fff) % count` slot and
/// `DAT_00638060` at exactly `lcg_next(seed)` on both sides, each asserted absolutely; an empty set
/// exercises the fallback chain (`getNearestClearTile` -> `getRandomClearTile(false, false)`, whose
/// own draws the Stage 6/11 tests plus [`run_habitat_get_nearest_clear_tile_live_test`] already
/// validate absolutely) and asserts restored-seed cross-agreement of both the returned tile and the
/// final seed. A pending `characteristics_dirty` recalculate is settled with one unasserted real
/// vanilla draw per habitat first, same as [`run_habitat_get_nearest_clear_tile_live_test`].
pub(crate) fn run_habitat_get_near_clear_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NEAR_CLEAR_TILE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);

    let keeper_ptr = world.entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) });
    let Some(unit) = keeper_ptr else {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    };
    let unit_tile = (unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) }) as u32;
    if unit_tile == 0 {
        write_success_line(failure_log, &format!("{} (skipped: live ZTKeeper has no tile)", test_name));
        return false;
    }
    // `unit+0x27c..+0x280` is a `ZTStaff`-only vector; fail loudly rather than let a malformed one
    // send the reserved-tile scan (both sides') walking unbounded memory.
    let reserved_begin: u32 = get_from_memory(unit + 0x27c);
    let reserved_end: u32 = get_from_memory(unit + 0x280);
    if reserved_end < reserved_begin || (reserved_end - reserved_begin) % 4 != 0 || reserved_end - reserved_begin > 0x10000 {
        let msg = format!("keeper {:#010x} reserved-tile vector malformed: {:#010x}..{:#010x}", unit, reserved_begin, reserved_end);
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { zthabitat::GET_RANDOM_ANIMAL.original()(ptr as *const std::ffi::c_void) };
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let animals: Vec<u32> = (0..(end.wrapping_sub(begin)) / 4)
            .map(|u| get_from_memory::<u32>(begin + u * 4))
            .filter(|&a| a != 0)
            .collect();
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let gate_tile_ptr = habitat
            .get_gate_tile_in()
            .map(|tile| world.get_ptr_from_bftile(&tile))
            .unwrap_or(0);
        for animal in std::iter::once(0u32).chain(animals) {
            let candidates = near_clear_tile_candidates(&tiles, unit, unit_tile, animal, gate_tile_ptr, max_cost);

            let seed_before: u32 = get_from_memory(rng_addr);
            let reimpl_ptr = habitat.get_near_clear_tile(unit, animal);
            let reimpl_seed: u32 = get_from_memory(rng_addr);
            save_to_memory(rng_addr, seed_before);
            let real_ptr = unsafe { zthabitat::GET_NEAR_CLEAR_TILE.original()(ptr as *const u32, unit as *const u32, animal as *const u32) } as u32;
            let real_seed: u32 = get_from_memory(rng_addr);

            if !candidates.is_empty() {
                let expected_rng = lcg_next(seed_before);
                let expected_tile = candidates[(((expected_rng >> 0x10) & 0x7fff) % candidates.len() as u32) as usize];
                for (side, drawn, seed_after) in [("reimpl", reimpl_ptr, reimpl_seed), ("real", real_ptr, real_seed)] {
                    let rng_ok = seed_after == expected_rng;
                    if drawn != expected_tile || !rng_ok {
                        failures.push(format!(
                            "habitat {} ({:#010x}), animal={:#010x}: {} got {:#010x}, expected {:#010x} (of {} candidates); rng {:#010x} -> {:#010x} (expected {:#010x})",
                            i, ptr, animal, side, drawn, expected_tile, candidates.len(), seed_before, seed_after, expected_rng
                        ));
                    }
                }
            } else if reimpl_ptr != real_ptr || reimpl_seed != real_seed {
                failures.push(format!(
                    "habitat {} ({:#010x}), animal={:#010x}: empty fallback disagreement - reimpl ({:#010x}, rng {:#010x}) vs real ({:#010x}, rng {:#010x})",
                    i, ptr, animal, reimpl_ptr, reimpl_seed, real_ptr, real_seed
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

/// Oracle minimum for [`run_habitat_get_nearest_clear_water_tile_live_test`]: the owned tiles
/// passing both `.asm` filters in walk order (`tile+0x83 & 3 != 0` water-class,
/// `tile+0x80 != 0xa`), minimized by squared Cartesian distance from `ref_tile` (`0x7fffffff` when
/// either pointer is null). The first candidate is kept unconditionally and replacements need
/// strictly smaller distance, so a null ref tile - every candidate at `0x7fffffff` - selects the
/// first filter-passing tile in walk order. Seed-independent - the function draws nothing.
fn nearest_clear_water_tile_oracle(tiles: &[u32], ref_tile: u32) -> u32 {
    let mut best_tile = 0u32;
    let mut best_dist = 0x7fff_ffffi32;
    for &tile in tiles {
        if get_from_memory::<u8>(tile + 0x83) & 3 == 0 || get_from_memory::<u8>(tile + 0x80) == 0xa {
            continue;
        }
        let dist: i32 = if tile == 0 || ref_tile == 0 {
            0x7fff_ffff
        } else {
            let dx: i32 = get_from_memory::<i32>(ref_tile + 0x34).wrapping_sub(get_from_memory::<i32>(tile + 0x34));
            let dy: i32 = get_from_memory::<i32>(ref_tile + 0x38).wrapping_sub(get_from_memory::<i32>(tile + 0x38));
            dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
        };
        if best_tile == 0 || dist < best_dist {
            best_tile = tile;
            best_dist = dist;
        }
    }
    best_tile
}

/// Per live habitat, checks one reimplementation call and one real vanilla call per reference tile
/// against the fully independent [`nearest_clear_water_tile_oracle`] - deterministic on both sides
/// (no RNG draw, no `characteristics_dirty` recalculate, so no seed juggling or settle draws).
/// Two reference tiles per habitat: a real `BFTile*` (the first animal's own tile - the real
/// caller `ZTGoalDrinkWater::decide` passes `BFEntity::getTile(entity)` - falling back to the
/// habitat's first owned tile on animal-free habitats, since the distance math needs only a real
/// tile, no unit) and null (the `.asm`'s `0x7fffffff` path - every candidate ties and the first
/// filter-passing tile wins). Habitats with no water-class tiles exercise the null return on both
/// sides (the plan's "dry land habitats return null without hanging"). Real vanilla's own call is
/// asserted against the same oracle, so the oracle itself is validated against vanilla behavior,
/// not just cross-agreement.
pub(crate) fn run_habitat_get_nearest_clear_water_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NEAREST_CLEAR_WATER_TILE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut water_hits = 0u32;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let animal_tile = (0..(end.wrapping_sub(begin)) / 4)
            .map(|u| get_from_memory::<u32>(begin + u * 4))
            .find(|&a| a != 0)
            .map(|animal| (unsafe { BFENTITY_GET_TILE.original()(animal as *const u32) }) as u32)
            .unwrap_or(0);
        let real_ref_tile = if animal_tile != 0 {
            animal_tile
        } else {
            tiles.iter().copied().find(|&tile| tile != 0).unwrap_or(0)
        };
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for (case, ref_tile) in [("tile", real_ref_tile), ("null", 0u32)] {
            let expected = nearest_clear_water_tile_oracle(&tiles, ref_tile);
            if expected != 0 {
                water_hits += 1;
            }
            let reimpl_ptr = habitat.get_nearest_clear_water_tile(ref_tile);
            let real_ptr =
                unsafe { zthabitat::GET_NEAREST_CLEAR_WATER_TILE.original()(ptr as *const u32, ref_tile as *const u32) } as u32;
            if reimpl_ptr != expected {
                failures.push(format!(
                    "habitat {} ({:#010x}), ref={} ({:#010x}): reimpl got {:#010x}, expected {:#010x}",
                    i, ptr, case, ref_tile, reimpl_ptr, expected
                ));
            }
            if real_ptr != expected {
                failures.push(format!(
                    "habitat {} ({:#010x}), ref={} ({:#010x}): real got {:#010x}, expected {:#010x}",
                    i, ptr, case, ref_tile, real_ptr, expected
                ));
            }
        }
    }
    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (non-null picks: {} habitat/tile draws)", test_name, water_hits));
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

/// Shared driver for the two gate-pass resolver tests ([`run_habitat_get_gate_tile_pass_in_live_test`] /
/// [`run_habitat_get_gate_tile_pass_out_live_test`]): per habitat with at least one real animal
/// (`unit` = the habitat's own first animal, a real `ZTUnit`-derived entity - matching the functions'
/// own real callers `ZTGoalKeeperHabitat`/`ZTGoalTrickFood::decide` and
/// `ZTGuest`/`ZTGuide`/`ZTStaff::pickRandomDest`, which all pass a live unit; animal-free habitats are
/// skipped, same rationale as this file's other unit-taking tests), resolves the expected gate tile
/// with the port's own getter, rebuilds the expected candidate set from that tile's own coordinates
/// with the same undetoured oracle [`run_habitat_get_adjacent_clear_tile_live_test`] uses, and checks
/// one reimplementation call and one real vanilla call directly against the shared game RNG state via
/// [`assert_adjacent_clear_tile_draw`]. Real vanilla's composition re-enters the same detoured
/// gate-tile-getter and `getAdjacentClearTile` ports under the battery, so this cross-checks the Pass
/// functions' own plumbing - argument marshaling, the ride-through `EAX` return, detour enablement -
/// against the shared callee behavior; a habitat without a usable gate contributes the null-gate path
/// (null return, seed untouched) on both sides via the helper's own empty-set branch.
fn run_gate_tile_pass_live_test(
    failure_log: &mut Option<std::fs::File>,
    test_name: &str,
    gate_of: impl Fn(&ZTHabitat) -> Option<BFTile>,
    pass_of: impl Fn(&ZTHabitat, u32) -> u32,
    real: impl Fn(u32, u32) -> u32,
) -> bool {
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let Some(unit) = (0..(end.wrapping_sub(begin)) / 4).map(|u| get_from_memory::<u32>(begin + u * 4)).find(|&a| a != 0) else {
            continue;
        };
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let gate_tile = gate_of(habitat);
        let (gate_ptr, candidates): (u32, Vec<u32>) = match &gate_tile {
            None => (0, Vec::new()),
            Some(tile) => {
                let base_x = tile.pos.x;
                let base_y = tile.pos.y;
                let base_habitat = habitat_mgr.get_habitat_ptr(base_x, base_y);
                let mut candidates = Vec::new();
                for dx in -1i32..=1 {
                    for dy in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let cand_x = base_x + dx;
                        let cand_y = base_y + dy;
                        if cand_x < 0 || cand_y < 0 || cand_x as u32 >= world.map_x_size || cand_y as u32 >= world.map_y_size {
                            continue;
                        }
                        if habitat_mgr.get_habitat_ptr(cand_x, cand_y) != base_habitat {
                            continue;
                        }
                        let candidate_tile_ptr = world.get_tile_ptr(cand_x as u32, cand_y as u32);
                        let cost = unsafe { call_bfunit_tile_cost_vtable_slot(unit, candidate_tile_ptr) };
                        if cost == max_cost {
                            continue;
                        }
                        candidates.push(candidate_tile_ptr);
                    }
                }
                (world.get_ptr_from_bftile(tile), candidates)
            }
        };

        let seed_before: u32 = get_from_memory(rng_addr);
        let reimpl_ptr = pass_of(habitat, unit);
        if let Some(msg) =
            assert_adjacent_clear_tile_draw("reimpl", i, ptr, gate_ptr, &candidates, seed_before, get_from_memory(rng_addr), reimpl_ptr)
        {
            failures.push(msg);
        }

        let seed_before: u32 = get_from_memory(rng_addr);
        let real_ptr = real(ptr, unit);
        if let Some(msg) =
            assert_adjacent_clear_tile_draw("real", i, ptr, gate_ptr, &candidates, seed_before, get_from_memory(rng_addr), real_ptr)
        {
            failures.push(msg);
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

pub(crate) fn run_habitat_get_gate_tile_pass_in_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_gate_tile_pass_live_test(
        failure_log,
        "ZTHABITAT_GET_GATE_TILE_PASS_IN_LIVE",
        |h| h.get_gate_tile_in(),
        |h, unit| h.get_gate_tile_pass_in(unit),
        |habitat_ptr, unit| unsafe { zthabitat::GET_GATE_TILE_PASS_IN.original()(habitat_ptr as *const u32, unit as *const u32) } as u32,
    )
}

pub(crate) fn run_habitat_get_gate_tile_pass_out_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_gate_tile_pass_live_test(
        failure_log,
        "ZTHABITAT_GET_GATE_TILE_PASS_OUT_LIVE",
        |h| h.get_gate_tile_out(),
        |h, unit| h.get_gate_tile_pass_out(unit),
        |habitat_ptr, unit| unsafe { zthabitat::GET_GATE_TILE_PASS_OUT.original()(habitat_ptr as *const u32, unit as *const u32) } as u32,
    )
}
/// happiness-change accumulator (`animal+0x2ac`), invokes one side with `type_ptr`, asserts the
/// decompile's own contract against the pristine snapshot - every animal whose entity-type species id
/// (read from `entity_type+0x1ec`, real vanilla's own filter) matches `type_ptr`'s gains exactly
/// `type_ptr`'s `baby_born_change` (`+0x31c`) and every non-member is untouched - then restores the
/// snapshot unconditionally, so both sides see identical input state and the live game is left exactly
/// as the pass found it.
fn assert_baby_born_bonus_pass(
    failures: &mut Vec<String>,
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    animal_ptrs: &[u32],
    type_ptr: u32,
    call: impl FnOnce(),
) {
    let species_id: i32 = get_from_memory(type_ptr + 0x1ec);
    let bonus: i32 = get_from_memory(type_ptr + 0x31c);
    let before: Vec<i32> = animal_ptrs.iter().map(|&a| get_from_memory(a + 0x2ac)).collect();
    call();
    for (index, &animal_ptr) in animal_ptrs.iter().enumerate() {
        let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
        let expected = if get_from_memory::<i32>(animal_type_ptr + 0x1ec) == species_id {
            before[index].wrapping_add(bonus)
        } else {
            before[index]
        };
        let actual: i32 = get_from_memory(animal_ptr + 0x2ac);
        if actual != expected {
            failures.push(format!(
                "habitat {} ({:#010x}) {}, type {:#010x} (species {}, bonus {}): animal {:#010x} accumulator {}, expected {}",
                habitat_index, habitat_ptr, side, type_ptr, species_id, bonus, animal_ptr, actual, expected
            ));
        }
    }
    for (index, &animal_ptr) in animal_ptrs.iter().enumerate() {
        save_to_memory(animal_ptr + 0x2ac, before[index]);
    }
}

/// `addBabyBornBonus` mutates live animal state, so this verifies each side against the decompile's own
/// contract over pristine input rather than cross-comparing after the fact. Per habitat, every
/// `ZTAnimalType` present anywhere in the zoo (real type pointers, so both sides read a real bonus
/// value) is passed to real vanilla first, then - after [`assert_baby_born_bonus_pass`] restored every
/// accumulator - to the reimplementation. The habitat/type cross-product also covers the
/// empty-scratch-vector path on both sides: whenever the species is absent from this habitat, the pass
/// asserts nothing changed at all. Real vanilla's internal `getSpeciesAnimals` re-enters the Rust port
/// (same detour shape as [`run_habitat_get_num_adult_animals_by_species_live_test`]), and
/// `recalculateCharacteristics` (triggered by either side's `characteristics_dirty` lazy-recalc) never
/// writes `+0x2ac`, so the snapshots stay valid within the synchronous, single-threaded pass - no game
/// tick can interleave.
pub(crate) fn run_habitat_add_baby_born_bonus_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_ADD_BABY_BORN_BONUS_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    // Union of the zoo's real `ZTAnimalType` pointers, in habitat order.
    let mut type_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        for animal_ptr in unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            if !type_ptrs.contains(&animal_type_ptr) {
                type_ptrs.push(animal_type_ptr);
            }
        }
    }

    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let animal_ptrs: Vec<u32> = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_all_animals(false).collect();
        for &type_ptr in &type_ptrs {
            assert_baby_born_bonus_pass(&mut failures, "real", i, ptr, &animal_ptrs, type_ptr, || unsafe {
                zthabitat::ADD_BABY_BORN_BONUS.original()(ptr as *const u32, type_ptr as *const u32)
            });
            assert_baby_born_bonus_pass(&mut failures, "reimpl", i, ptr, &animal_ptrs, type_ptr, || {
                unsafe { ref_from_memory::<ZTHabitat>(ptr) }.add_baby_born_bonus(type_ptr)
            });
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

/// One snapshot→call→assert→restore pass of [`run_habitat_trigger_keeper_arrived_live_test`] for a
/// single (side, habitat, `scheduled`) combination. Snapshots the habitat's `scheduled_service_counter`
/// (`+0xf4`) and every animal's keeper-arrives flag byte (`+0x39c`) - plus the same pair for every
/// amphibious neighbor on **both** `scheduled` values, so the recursion gate is asserted directly
/// rather than only implicitly and the restore covers the function's full write set even if a future
/// regression recursed when it should not - calls one side, asserts the decompile's contract over the
/// pristine input, then restores every snapshot unconditionally so the next pass and the live game see
/// exactly the state found. Expected values, straight from the `.asm`/`.c`: every entry the pass
/// actually writes (the habitat itself always, a neighbor only under `scheduled`'s recursion) takes
/// the contract - counter `(before.wrapping_sub(1)).max(0)`, exactly vanilla's `DEC`/`JNS`
/// clamp-at-0 pair, and animal flag `1` when its own `canService` low byte passes else unchanged
/// (`setKeeperArrives` *assigns* 1, it never ORs or clears) - while every non-involved entry must be
/// unchanged. `canService` is a read-only predicate over animal/keeper fields nothing in the pass
/// writes, so re-evaluating it per animal during the assert phase cannot perturb what the pass under
/// test wrote.
fn assert_trigger_keeper_arrived_pass(
    failures: &mut Vec<String>,
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    keeper_ptr: u32,
    scheduled: bool,
    call: impl FnOnce(),
) {
    let mut involved_habitats: Vec<u32> = vec![habitat_ptr];
    for node in walk_neighbor_tree(get_from_memory(habitat_ptr + 0x8)) {
        involved_habitats.push(get_from_memory(node + 0x10));
    }
    let mut counter_snapshots: Vec<(u32, i32)> = Vec::new(); // (counter address, value before)
    let mut flag_snapshots: Vec<(u32, u8, bool)> = Vec::new(); // (animal pointer, flag byte before, whether this pass writes it)
    for (index, &habitat) in involved_habitats.iter().enumerate() {
        let written = index == 0 || scheduled;
        counter_snapshots.push((habitat + 0xf4, get_from_memory(habitat + 0xf4)));
        for addr in (get_from_memory::<u32>(habitat + 0x6c)..get_from_memory::<u32>(habitat + 0x70)).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            flag_snapshots.push((animal_ptr, get_from_memory(animal_ptr + 0x39c), written));
        }
    }

    call();

    for (index, &(addr, before)) in counter_snapshots.iter().enumerate() {
        let expected = if index == 0 || scheduled {
            (before.wrapping_sub(1)).max(0)
        } else {
            before
        };
        let actual: i32 = get_from_memory(addr);
        if actual != expected {
            failures.push(format!(
                "habitat {} ({:#010x}) {}, scheduled={}: counter at {:#010x} is {}, expected {}",
                habitat_index, habitat_ptr, side, scheduled, addr, actual, expected
            ));
        }
    }
    for &(animal_ptr, before, written) in &flag_snapshots {
        let passes = low_byte_bool(unsafe { ZTANIMAL_CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) });
        let expected = if written && passes { 1 } else { before };
        let actual: u8 = get_from_memory(animal_ptr + 0x39c);
        if actual != expected {
            failures.push(format!(
                "habitat {} ({:#010x}) {}, scheduled={}: animal {:#010x} keeper-arrives flag is {:#04x}, expected {:#04x} (canService={})",
                habitat_index, habitat_ptr, side, scheduled, animal_ptr, actual, expected, passes
            ));
        }
    }
    for &(addr, before) in &counter_snapshots {
        save_to_memory(addr, before);
    }
    for &(animal_ptr, before, _) in &flag_snapshots {
        save_to_memory(animal_ptr + 0x39c, before);
    }
}

/// `triggerKeeperArrived` mutates live habitat/animal state, so this verifies each side against the
/// decompile's own contract over pristine input rather than cross-comparing after the fact (the same
/// snapshot→call→assert→restore shape as [`run_habitat_add_baby_born_bonus_live_test`]). One live
/// `ZTKeeper` is found in the real, loaded zoo's own `entity_array` (via [`RVA_KEEPER_TYPE_CHECK_ARG`],
/// the same pattern as [`run_habitat_block_service_matches_real_live_test`]); real vanilla is called
/// first, then the reimplementation, per habitat per `scheduled` value - both values, so the
/// no-recursion path and the amphibious-neighbor recursion path are each exercised over every live
/// habitat. Real vanilla's own neighbor recursion re-enters the Rust port under both sides (its
/// self-call sits at the detoured address, same documented semantics as the Stage 2 counters), while
/// `canService`/`setKeeperArrives`/`recalculateCharacteristics` are undetoured `.original()`
/// call-throughs. Nothing the pass or the `characteristics_dirty` lazy recalculate writes feeds any
/// snapshot (same reasoning as Stage 7's test), so no settle draw is needed within the synchronous,
/// single-threaded pass.
pub(crate) fn run_habitat_trigger_keeper_arrived_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_TRIGGER_KEEPER_ARRIVED_LIVE";

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
        for scheduled in [false, true] {
            assert_trigger_keeper_arrived_pass(&mut failures, "real", i, ptr, keeper_ptr, scheduled, || unsafe {
                zthabitat::TRIGGER_KEEPER_ARRIVED.original()(ptr as *const u32, keeper_ptr as *const u32, scheduled)
            });
            assert_trigger_keeper_arrived_pass(&mut failures, "reimpl", i, ptr, keeper_ptr, scheduled, || {
                unsafe { mut_from_memory::<ZTHabitat>(ptr) }.trigger_keeper_arrived(keeper_ptr, scheduled)
            });
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

/// Real called before reimpl deliberately, same `characteristics_dirty` ordering rationale as
/// [`run_habitat_get_attractiveness_live_test`] - both `false` (direct-occupant count alone) and `true`
/// (additionally summing every amphibious neighbor's own count) are exercised over every live habitat.
pub(crate) fn run_habitat_get_num_angry_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let direct = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_ANGRY_ANIMALS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_ANGRY_ANIMALS.original()(ptr, false) },
        |habitat| habitat.get_num_angry_animals(false),
    );
    let with_neighbors = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_ANGRY_ANIMALS_WITH_NEIGHBORS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_ANGRY_ANIMALS.original()(ptr, true) },
        |habitat| habitat.get_num_angry_animals(true),
    );
    direct || with_neighbors
}

/// Real called before reimpl deliberately, same `characteristics_dirty` ordering rationale as
/// [`run_habitat_get_attractiveness_live_test`] - both `false` (direct-occupant count alone) and `true`
/// (additionally summing every amphibious neighbor's own count) are exercised over every live habitat.
pub(crate) fn run_habitat_get_num_sick_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let direct = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_SICK_ANIMALS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_SICK_ANIMALS.original()(ptr, false) },
        |habitat| habitat.get_num_sick_animals(false),
    );
    let with_neighbors = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_SICK_ANIMALS_WITH_NEIGHBORS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_SICK_ANIMALS.original()(ptr, true) },
        |habitat| habitat.get_num_sick_animals(true),
    );
    direct || with_neighbors
}

/// Real called before reimpl deliberately, same `characteristics_dirty` ordering rationale as
/// [`run_habitat_get_attractiveness_live_test`]. Unlike the sibling count getters this takes no
/// `include_neighbors` flag (real vanilla reads the single cached field and returns), so there is
/// no with-neighbors variant - one comparison per live habitat.
pub(crate) fn run_habitat_get_avg_animal_happiness_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_AVG_ANIMAL_HAPPINESS_LIVE",
        |ptr| unsafe { zthabitat::GET_AVG_ANIMAL_HAPPINESS.original()(ptr) },
        |habitat| habitat.get_avg_animal_happiness(),
    )
}

/// `ZTHabitat::getAllAnimals` always returns the same `&this->field_0x6c` pointer regardless of `sort`,
/// only conditionally reordering the vector's own contents first - so this compares sorted *contents*.
/// Per habitat, real vanilla sorts first (`sort = true`) and its output is asserted non-decreasing
/// under [`entity_name_bytes`] ordering - validating the reimplemented name comparator against the
/// real one at `0x004690cd`. The live array is then **reversed** in place so the reimplementation
/// has real sorting work to do, sorted by the port, and compared to vanilla's result: element-for-element
/// when every name is distinct, by name sequence when names repeat (vanilla's introsort and Rust's stable
/// sort may order equal names differently). Vanilla's own ordering is written back afterward, so later
/// tests see the array exactly as vanilla left it.
pub(crate) fn run_habitat_get_all_animals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_ALL_ANIMALS_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        unsafe { zthabitat::GET_ALL_ANIMALS.original()(ptr as *const u32, 1) };
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let real_animals: Vec<u32> = (begin..end).step_by(4).map(get_from_memory::<u32>).collect();
        let real_names: Vec<Vec<u8>> = real_animals.iter().map(|&a| entity_name_bytes(a)).collect();
        if real_names.windows(2).any(|pair| pair[0] > pair[1]) {
            failures.push(format!(
                "habitat {i} ({ptr:#010x}): real vanilla's order is not name-sorted under the ported comparator: {:?}",
                real_names.iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>()
            ));
        }

        for (slot, &animal) in real_animals.iter().rev().enumerate() {
            save_to_memory(begin + slot as u32 * 4, animal);
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let reimpl_animals: Vec<u32> = habitat.get_all_animals(true).collect();
        let reimpl_names: Vec<Vec<u8>> = reimpl_animals.iter().map(|&a| entity_name_bytes(a)).collect();
        let names_distinct = real_names.windows(2).all(|pair| pair[0] != pair[1]);
        let matches = if names_distinct { reimpl_animals == real_animals } else { reimpl_names == real_names };
        if !matches {
            failures.push(format!("habitat {i} ({ptr:#010x}): real={real_animals:?}, reimpl={reimpl_animals:?}"));
        }

        for (slot, &animal) in real_animals.iter().enumerate() {
            save_to_memory(begin + slot as u32 * 4, animal);
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

/// Smoke test for `ZTHabitat::remove_food_target_for_all` (which internally calls
/// `ZTHabitat::remove_food_target` on every matching animal, exercising both ports at once): scans every
/// live habitat's animals (`ZTHabitat::get_all_animals`) for the first one with a live food target
/// (`animal_food_target`) - skips if none is found (not every loaded zoo necessarily has an animal
/// mid-eat). Calls `remove_food_target_for_all` with that target entity, then asserts the animal's own
/// `animal_food_target` reads back as `0`. Does not compare against real vanilla's own `.original()` -
/// like `remove_species`, this is destructive (it clears the animal's food target and tears down the food
/// entity itself via `setVisible`/`setIsRemoved`), so there is no way to run both poles against the same
/// starting state.
pub(crate) fn run_habitat_remove_food_target_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_REMOVE_FOOD_TARGET_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();

    let found = (0..habitat_mgr.exhibit_array().len()).find_map(|i| {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            return None;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        habitat.get_all_animals(false).find_map(|animal_ptr| {
            let target = animal_food_target(animal_ptr);
            if target != 0 {
                Some((ptr, animal_ptr, target))
            } else {
                None
            }
        })
    });

    let Some((habitat_ptr, animal_ptr, target_ptr)) = found else {
        write_success_line(failure_log, &format!("{} (skipped: no live animal with an active food target)", test_name));
        return false;
    };

    let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
    habitat.remove_food_target_for_all(target_ptr);

    let still_targeting = animal_food_target(animal_ptr);
    if still_targeting == 0 {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("animal {animal_ptr:#010x} still targets {still_targeting:#010x} after remove_food_target_for_all({target_ptr:#010x})");
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

/// Comparison test for `ZTHabitatMgr::leadsTo`: real vs. reimplemented, over every ordered pair of real
/// habitats in the loaded zoo's own `exhibit_array` (including self-pairs) plus the null-pointer edge
/// cases. Unlike `ZTHABITAT_GET_OUTERMOST_TANK_LIVE`'s own smoke-test-only precedent, real vanilla
/// `leadsTo` explicitly null-checks `getGateTileOut`'s own result before dereferencing it - no known
/// unguarded-null-deref risk, so a direct `.original()` comparison is safe here. Both sides reset
/// `tank_walk_visited_marker` internally on every call, so back-to-back real/reimplemented calls don't
/// interfere with each other.
pub(crate) fn run_zthabitatmgr_leads_to_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_LEADS_TO_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut fail_flag = false;

    let mut check = |habitat_a: u32, habitat_b: u32| {
        let real_value = low_byte_bool(unsafe {
            zthabitatmgr::LEADS_TO.original()(habitat_mgr as *const _ as *const u32, habitat_a as *const u32, habitat_b as *const u32)
        });
        let reimpl_value = habitat_mgr.leads_to(habitat_a, habitat_b);
        if real_value != reimpl_value {
            let msg = format!("mismatch for ({:#010x}, {:#010x}): real={}, reimpl={}", habitat_a, habitat_b, real_value, reimpl_value);
            error!("{}: {}", test_name, msg);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
            }
            fail_flag = true;
        }
    };

    check(0, 0);
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr_a = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr_a == 0 {
            continue;
        }
        check(ptr_a, 0);
        check(0, ptr_a);
        for j in 0..habitat_mgr.exhibit_array().len() {
            let ptr_b = habitat_mgr.exhibit_array().get_ptr(j);
            if ptr_b == 0 {
                continue;
            }
            check(ptr_a, ptr_b);
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

/// Smoke test for the species-rating-cache producer/consumer pair `ZTHabitatMgr::terrainAboutToBeChanged`/
/// `terrainChanged` (see `species-rating-cache-identification-handover.md`). Calls
/// `terrain_about_to_be_changed` over each real habitat's first owned tile (`size = 1`, so the scan finds
/// exactly that one habitat) to populate the cache, then calls `terrain_changed` once to exercise the
/// consumer path over whatever's left cached (each `terrain_about_to_be_changed` call clears the cache at
/// its own top, matching real vanilla, so only the last habitat processed survives to the consumer call -
/// still a meaningful exercise of both functions). `ZTHabitatMgr::beforeEntityChange` (the third function
/// in this cluster) is already exercised indirectly by
/// [`run_zthabitatmgr_entity_about_to_be_placed_smoke_live_test`]/
/// [`run_zthabitatmgr_entity_placed_smoke_live_test`], which now call through `Self::before_entity_change`
/// on every live entity's own habitat.
pub(crate) fn run_zthabitatmgr_terrain_changed_cluster_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_TERRAIN_CHANGED_CLUSTER_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut any_habitat_exercised = false;

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
        let tile = get_from_memory::<crate::ztmapview::BFTile>(tile_ptr);
        habitat_mgr.terrain_about_to_be_changed(tile.pos.x, tile.pos.y, 1);
        any_habitat_exercised = true;
    }

    habitat_mgr.terrain_changed();

    if any_habitat_exercised {
        write_success_line(failure_log, test_name);
    } else {
        write_success_line(failure_log, &format!("{} (skipped: no habitat with an owned tile found)", test_name));
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

/// `getNumKeeperFoodTiles` reads the owned-tile list directly rather than any cached tally, so the driver
/// sweeps categories the same exhaustive way as [`run_habitat_get_amount_keeper_food_live_test`] and adds
/// an independently-built oracle: an in-test re-walk of each habitat's own tile list through the same
/// gate/field the port documents. Also asserts non-vacuousness - on a foodless zoo a wrong gate or wrong
/// field offset reads as 0 == 0 and passes silently, so the oracle must find at least one food tile
/// somewhere in the zoo.
pub(crate) fn run_habitat_get_num_keeper_food_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NUM_KEEPER_FOOD_TILES_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut tiles_walked = 0usize;
    let mut food_tiles_found = 0usize;
    let mut categories_seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();

    // Phase 1: the oracle's own walk, one traversal per habitat - every gate-passing occupant's
    // `entity_type+0x168` word is collected so the per-category counts below come out of a single list
    // walk instead of one per swept category.
    let mut per_habitat_food_types: Vec<(usize, u32, Vec<u32>)> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut food_types: Vec<u32> = Vec::new();
        for node in walk_tile_list(*habitat.owned_tiles_ptr()) {
            tiles_walked += 1;
            let tile_ptr = get_from_memory::<u32>(node + 0x8);
            let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
            if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_ZTFOOD_TYPE_CHECK_ARG) } {
                continue;
            }
            let food_type: u32 = get_from_memory(get_from_memory::<u32>(entity_ptr + 0x128) + 0x168);
            food_types.push(food_type);
            categories_seen.insert(food_type);
            food_tiles_found += 1;
        }
        per_habitat_food_types.push((i, ptr, food_types));
    }

    if per_habitat_food_types.is_empty() {
        write_success_line(failure_log, &format!("{} (skipped: no live habitats)", test_name));
        return false;
    }

    // Sweep bound: the 16-entry category enum `getAmountKeeperFood`'s tally array indexes, a
    // guaranteed-miss value, plus any out-of-range food-category word the oracle walk saw - so every
    // gate-passing tile the zoo contains is guaranteed at least one matching swept category.
    let mut categories: Vec<u32> = (0..16).collect();
    categories.push(0xFFFF_FFFF);
    for &value in &categories_seen {
        if !(0..16).contains(&value) {
            categories.push(value);
        }
    }

    for (i, ptr, food_types) in &per_habitat_food_types {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(*ptr) };
        for &category in &categories {
            let oracle = food_types.iter().filter(|&&food_type| food_type == category).count() as i32;
            let real = unsafe { zthabitat::GET_NUM_KEEPER_FOOD_TILES.original()(*ptr as *const u32, category) };
            let port = habitat.get_num_keeper_food_tiles(category);
            if real != oracle || port != oracle {
                failures.push(format!(
                    "habitat {} ({:#010x}), category={:#x}: oracle={}, real={}, port={}",
                    i, ptr, category, oracle, real, port
                ));
            }
        }
    }

    if food_tiles_found == 0 {
        failures.push(format!(
            "save contains no keeper food tiles - update the save (walked {} owned tiles across {} habitats, \
             the oracle's ZTFood gate never passed; a wrong gate or field offset would read as 0 == 0 here)",
            tiles_walked,
            per_habitat_food_types.len()
        ));
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} ({} habitats, {} owned tiles walked, {} food tiles found, distinct food categories on tiles: {:?})",
                test_name,
                per_habitat_food_types.len(),
                tiles_walked,
                food_tiles_found,
                categories_seen
            ),
        );
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

/// Which of the three keeper-food targeting heuristics a [`run_habitat_keeper_food_pick_live_test`] run
/// exercises - the `ZTGoalKeeperFood::decide` triplet, sharing one own-walk gate
/// (non-null `ZTFood` occupant whose `entity_type+0x168` word equals `category`) and differing only in
/// how the matching pool is picked (min `entity+0x154` amount / min squared distance to the reference
/// tile / one-LCG-step uniform index) and in the world-global guard (absent only on Random).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KeeperFoodPickKind {
    Smallest,
    Nearest,
    Random,
}

impl KeeperFoodPickKind {
    fn test_name(self) -> &'static str {
        match self {
            KeeperFoodPickKind::Smallest => "ZTHABITAT_GET_SMALLEST_KEEPER_FOOD_LIVE",
            KeeperFoodPickKind::Nearest => "ZTHABITAT_GET_NEAREST_KEEPER_FOOD_LIVE",
            KeeperFoodPickKind::Random => "ZTHABITAT_GET_RANDOM_KEEPER_FOOD_LIVE",
        }
    }

    /// Real vanilla call-through (debug: the trampoline; release: the raw address, which re-enters
    /// the port - same release semantics as [`BiomeTileKind::real`]). `generated.rs`'s `-> i32` return
    /// is the pointer-as-integer wart (EAX carries the picked food entity or null) - cast back here.
    fn real(self, habitat_ptr: u32, ref_tile: u32, category: u32, include_neighbors: bool) -> u32 {
        let raw = match self {
            KeeperFoodPickKind::Smallest => unsafe {
                zthabitat::GET_SMALLEST_KEEPER_FOOD.original()(habitat_ptr as *const u32, ref_tile as *const u32, category, include_neighbors)
            },
            KeeperFoodPickKind::Nearest => unsafe {
                zthabitat::GET_NEAREST_KEEPER_FOOD.original()(habitat_ptr as *const u32, ref_tile as *const u32, category, include_neighbors)
            },
            KeeperFoodPickKind::Random => unsafe {
                zthabitat::GET_RANDOM_KEEPER_FOOD.original()(habitat_ptr as *const u32, ref_tile as *const u32, category, include_neighbors)
            },
        };
        raw as u32
    }

    /// The port under test.
    fn port(self, habitat: &ZTHabitat, ref_tile: u32, category: u32, include_neighbors: bool) -> u32 {
        match self {
            KeeperFoodPickKind::Smallest => habitat.get_smallest_keeper_food(ref_tile, category, include_neighbors),
            KeeperFoodPickKind::Nearest => habitat.get_nearest_keeper_food(ref_tile, category, include_neighbors),
            KeeperFoodPickKind::Random => habitat.get_random_keeper_food(ref_tile, category, include_neighbors),
        }
    }
}

/// `(entity, host_tile, food_category)` oracle walk entries, in owned-tile-list order - the ordered
/// walk all three picks iterate, so first-wins tie behavior falls out of the Vec order.
type KeeperFoodOracleList = Vec<(u32, u32, u32)>;

/// The squared Cartesian distance between two raw tiles, `0x7fffffff` when either pointer is null -
/// the exact math both [`KeeperFoodPickKind::Nearest`] comparisons use.
fn keeper_food_tile_dist_squared(ref_tile: u32, tile: u32) -> i32 {
    if ref_tile == 0 || tile == 0 {
        return 0x7fff_ffff;
    }
    let dx: i32 = get_from_memory::<i32>(ref_tile + 0x34).wrapping_sub(get_from_memory::<i32>(tile + 0x34));
    let dy: i32 = get_from_memory::<i32>(ref_tile + 0x38).wrapping_sub(get_from_memory::<i32>(tile + 0x38));
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
}

/// One list's oracle minimum for [`keeper_food_pick_oracle`]: `(entity, metric)` over `category`
/// matches in walk order, strict `<` against the `0x7fffffff` sentinel (first-wins ties). Nearest
/// measures from the reference tile to each candidate's host tile - the own/neighbor walks' own
/// comparison; only the parent's re-distance of a neighbor *result* goes through
/// `BFEntity::getTile` (see [`keeper_food_pick_oracle`]).
fn keeper_food_oracle_list_min(kind: KeeperFoodPickKind, foods: &KeeperFoodOracleList, ref_tile: u32, category: u32) -> (u32, i32) {
    let mut best = (0u32, 0x7fff_ffffi32);
    for &(entity, tile, food_category) in foods {
        if food_category != category {
            continue;
        }
        let (candidate, metric) = match kind {
            KeeperFoodPickKind::Smallest => (entity, get_from_memory::<i32>(entity + 0x154)),
            KeeperFoodPickKind::Nearest => (entity, keeper_food_tile_dist_squared(ref_tile, tile)),
            KeeperFoodPickKind::Random => unreachable!("Random has no list minimum"),
        };
        if metric < best.1 {
            best = (candidate, metric);
        }
    }
    best
}

/// The full oracle pick for the two deterministic kinds, mirroring the
/// own-early-return / per-neighbor-internal-min-then-parent-compare recursion exactly: the own list
/// is minimized first and returned immediately when non-empty; only an empty own pool consults the
/// neighbors in tree order, each resolving its own internal minimum through the same minimizer, with
/// the parent comparing the *returned candidate* against its running best (strict `<`) - re-distanced
/// through real `BFEntity::getTile` for Nearest, mirroring real vanilla's own call.
fn keeper_food_pick_oracle(
    kind: KeeperFoodPickKind,
    own_foods: &KeeperFoodOracleList,
    neighbor_foods: &[KeeperFoodOracleList],
    ref_tile: u32,
    category: u32,
    include_neighbors: bool,
) -> u32 {
    let (mut best_entity, mut best_metric) = keeper_food_oracle_list_min(kind, own_foods, ref_tile, category);
    if best_entity != 0 || !include_neighbors {
        return best_entity;
    }
    for neighbor in neighbor_foods {
        let (candidate, candidate_metric) = keeper_food_oracle_list_min(kind, neighbor, ref_tile, category);
        if candidate == 0 {
            continue;
        }
        let metric = match kind {
            KeeperFoodPickKind::Nearest => {
                let candidate_tile = (unsafe { BFENTITY_GET_TILE.original()(candidate as *const u32) }) as u32;
                keeper_food_tile_dist_squared(ref_tile, candidate_tile)
            }
            _ => candidate_metric,
        };
        if metric < best_metric {
            best_metric = metric;
            best_entity = candidate;
        }
    }
    best_entity
}

/// Phase-1 oracle walk over one habitat's (or neighbor's) owned-tile list, collecting every
/// gate-passing occupant as `(entity, host tile, entity_type+0x168 category)` in walk order.
fn keeper_food_oracle_walk(
    sentinel: u32,
    foods: &mut KeeperFoodOracleList,
    categories_seen: &mut std::collections::BTreeSet<u32>,
    tiles_walked: &mut usize,
    food_tiles_found: &mut usize,
) {
    for node in walk_tile_list(sentinel) {
        *tiles_walked += 1;
        let tile_ptr = get_from_memory::<u32>(node + 0x8);
        let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
        if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_ZTFOOD_TYPE_CHECK_ARG) } {
            continue;
        }
        let food_category: u32 = get_from_memory(get_from_memory::<u32>(entity_ptr + 0x128) + 0x168);
        foods.push((entity_ptr, tile_ptr, food_category));
        categories_seen.insert(food_category);
        *food_tiles_found += 1;
    }
}

/// Shared driver for the keeper-food targeting triplet: per live habitat, builds the oracle once
/// (phase-1 walks over the habitat's own tiles plus each amphibious neighbor's, in
/// [`walk_neighbor_tree`] order), then sweeps a real `BFTile*` reference tile (the first animal's own
/// tile - the real caller `ZTGoalKeeperFood::decide` resolves its unit to a tile - falling back to the
/// habitat's first owned tile) and null, every category in `0..16` plus the `0xffffffff` guaranteed
/// miss plus every distinct category word the oracle observed, and both `include_neighbors` values.
///
/// Smallest/Nearest are fully deterministic three-way matches (oracle / real `.original()` / port).
/// Random's oracle is the RNG state itself: each side's draw must return the
/// `((lcg_next(seed) >> 0x10) & 0x7fff) % pool.len()`-th own-pool entity (via
/// [`assert_lcg_tile_draw`]) or, when the own pool is empty, the first non-empty neighbor's own
/// one-LCG-step pick with the seed advanced exactly once (a small custom branch -
/// [`assert_lcg_tile_draw`]'s empty-list arm only covers the no-hit-at-all case) or 0 with the seed
/// untouched. Everything is read-only apart from those exactly-asserted advances, so no
/// snapshot/restore and no settle draws.
///
/// Non-vacuousness: the save must carry at least one keeper food tile zoo-wide (on a foodless zoo a
/// wrong gate reads as 0 == 0 - [`run_habitat_get_num_keeper_food_tiles_live_test`]'s assert), and for
/// Random at least one draw must come from a non-empty own pool (the pick path itself ran).
fn run_habitat_keeper_food_pick_live_test(failure_log: &mut Option<std::fs::File>, kind: KeeperFoodPickKind) -> bool {
    let test_name = kind.test_name();
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut failures: Vec<String> = Vec::new();
    let mut fail_flag = false;
    let mut tiles_walked = 0usize;
    let mut food_tiles_found = 0usize;
    let mut categories_seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut rows_checked = 0u32;
    let mut own_pool_draws = 0u32;
    let mut neighbor_resolved_rows = 0u32;
    let mut habitats_checked = 0u32;

    // Phase 1: oracle walks - per habitat, its own foods plus each amphibious neighbor's (walked
    // directly through the neighbor's own owned-tile sentinel, not via the exhibit array), and the
    // reference tile both sides are driven with.
    let mut per_habitat: Vec<(usize, u32, KeeperFoodOracleList, Vec<KeeperFoodOracleList>, u32)> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        habitats_checked += 1;
        let mut own_foods = KeeperFoodOracleList::new();
        keeper_food_oracle_walk(*habitat.owned_tiles_ptr(), &mut own_foods, &mut categories_seen, &mut tiles_walked, &mut food_tiles_found);
        let neighbor_foods: Vec<KeeperFoodOracleList> = walk_neighbor_tree(habitat.amphibious_neighbors_head)
            .map(|node| {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                let mut foods = KeeperFoodOracleList::new();
                keeper_food_oracle_walk(
                    get_from_memory(neighbor_ptr + 0x40),
                    &mut foods,
                    &mut categories_seen,
                    &mut tiles_walked,
                    &mut food_tiles_found,
                );
                foods
            })
            .collect();
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let animal_tile = (0..(end.wrapping_sub(begin)) / 4)
            .map(|u| get_from_memory::<u32>(begin + u * 4))
            .find(|&a| a != 0)
            .map(|animal| (unsafe { BFENTITY_GET_TILE.original()(animal as *const u32) }) as u32)
            .unwrap_or(0);
        let tiles: Vec<u32> = walk_tile_list(*habitat.owned_tiles_ptr()).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        let ref_tile = if animal_tile != 0 { animal_tile } else { tiles.iter().copied().find(|&tile| tile != 0).unwrap_or(0) };
        per_habitat.push((i, ptr, own_foods, neighbor_foods, ref_tile));
    }

    if per_habitat.is_empty() {
        write_success_line(failure_log, &format!("{} (skipped: no live habitats)", test_name));
        return false;
    }

    // Sweep bound: the 16-entry category enum `getAmountKeeperFood`'s tally array indexes, a
    // guaranteed-miss value, plus any out-of-range food-category word the oracle walk saw.
    let mut categories: Vec<u32> = (0..16).collect();
    categories.push(0xFFFF_FFFF);
    for &value in &categories_seen {
        if !(0..16).contains(&value) {
            categories.push(value);
        }
    }

    for (i, ptr, own_foods, neighbor_foods, ref_tile) in &per_habitat {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(*ptr) };
        for (case, case_ref_tile) in [("tile", *ref_tile), ("null", 0u32)] {
            for &category in &categories {
                for include_neighbors in [false, true] {
                    rows_checked += 1;
                    let own_pool: Vec<u32> = own_foods.iter().filter(|&&(_, _, c)| c == category).map(|&(entity, _, _)| entity).collect();
                    match kind {
                        KeeperFoodPickKind::Random => {
                            if !own_pool.is_empty() {
                                own_pool_draws += 1;
                            }
                            for side in ["reimpl", "real"] {
                                let seed_before: u32 = get_from_memory(rng_addr);
                                let drawn = if side == "reimpl" {
                                    kind.port(habitat, case_ref_tile, category, include_neighbors)
                                } else {
                                    kind.real(*ptr, case_ref_tile, category, include_neighbors)
                                };
                                let seed_after: u32 = get_from_memory(rng_addr);
                                if !own_pool.is_empty() {
                                    fail_flag |= assert_lcg_tile_draw(
                                        failure_log,
                                        test_name,
                                        side,
                                        *i,
                                        *ptr,
                                        &own_pool,
                                        seed_before,
                                        seed_after,
                                        drawn,
                                    );
                                } else if include_neighbors {
                                    // Expected: the first neighbor in tree order whose own pool is
                                    // non-empty resolves the hit with its own one-LCG-step pick over
                                    // `seed_before` (the parent advanced nothing); a pool-holding
                                    // neighbor never returns null, so a non-zero expectation is exact.
                                    // No such neighbor -> 0 with the seed untouched.
                                    let expected_hit = neighbor_foods
                                        .iter()
                                        .find_map(|foods| {
                                            let pool: Vec<u32> =
                                                foods.iter().filter(|&&(_, _, c)| c == category).map(|&(entity, _, _)| entity).collect();
                                            if pool.is_empty() {
                                                None
                                            } else {
                                                let index = ((lcg_next(seed_before) >> 0x10) & 0x7fff) % pool.len() as u32;
                                                Some(pool[index as usize])
                                            }
                                        })
                                        .unwrap_or(0);
                                    let expected_seed = if expected_hit != 0 { lcg_next(seed_before) } else { seed_before };
                                    if drawn != expected_hit || seed_after != expected_seed {
                                        failures.push(format!(
                                            "habitat {} ({:#010x}), ref={} ({:#010x}), category={:#x}, {} draw: got {:#010x}, \
                                             expected {:#010x} (first non-empty neighbor pick); rng {:#010x} -> {:#010x}, expected {:#010x}",
                                            i, ptr, case, case_ref_tile, category, side, drawn, expected_hit, seed_before, seed_after, expected_seed
                                        ));
                                    }
                                } else {
                                    // Empty own pool, no subhabs -> 0 with the seed untouched (the
                                    // helper's empty-list arm).
                                    fail_flag |= assert_lcg_tile_draw(
                                        failure_log,
                                        test_name,
                                        side,
                                        *i,
                                        *ptr,
                                        &[],
                                        seed_before,
                                        seed_after,
                                        drawn,
                                    );
                                }
                            }
                        }
                        _ => {
                            let expected =
                                keeper_food_pick_oracle(kind, own_foods, neighbor_foods, case_ref_tile, category, include_neighbors);
                            if expected != 0 && own_pool.is_empty() {
                                neighbor_resolved_rows += 1;
                            }
                            let port_pick = kind.port(habitat, case_ref_tile, category, include_neighbors);
                            let real_pick = kind.real(*ptr, case_ref_tile, category, include_neighbors);
                            if port_pick != expected {
                                failures.push(format!(
                                    "habitat {} ({:#010x}), ref={} ({:#010x}), category={:#x}, subhabs={}: port got {:#010x}, expected {:#010x}",
                                    i, ptr, case, case_ref_tile, category, include_neighbors, port_pick, expected
                                ));
                            }
                            if real_pick != expected {
                                failures.push(format!(
                                    "habitat {} ({:#010x}), ref={} ({:#010x}), category={:#x}, subhabs={}: real got {:#010x}, expected {:#010x}",
                                    i, ptr, case, case_ref_tile, category, include_neighbors, real_pick, expected
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if food_tiles_found == 0 {
        failures.push(format!(
            "save contains no keeper food tiles - update the save (walked {} owned tiles across {} habitats, \
             the oracle's ZTFood gate never passed; a wrong gate or field offset would read as 0 == 0 here)",
            tiles_walked,
            per_habitat.len()
        ));
    }
    if kind == KeeperFoodPickKind::Random && own_pool_draws == 0 {
        failures.push(
            "non-vacuous-draw assert failed: no category/habitat row drew from a non-empty own pool - the pick path never ran on this save"
                .to_string(),
        );
    }

    if failures.is_empty() && !fail_flag {
        let random_stats = if kind == KeeperFoodPickKind::Random {
            format!("non-empty-own-pool draws: {}, ", own_pool_draws)
        } else {
            String::new()
        };
        write_success_line(
            failure_log,
            &format!(
                "{} ({} habitats, {} owned tiles walked (incl. neighbors), {} food tiles found, {} rows checked, \
                 {}neighbor-resolved rows: {}, categories: {:?})",
                test_name,
                habitats_checked,
                tiles_walked,
                food_tiles_found,
                rows_checked,
                random_stats,
                neighbor_resolved_rows,
                categories_seen
            ),
        );
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

pub(crate) fn run_habitat_get_smallest_keeper_food_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_keeper_food_pick_live_test(failure_log, KeeperFoodPickKind::Smallest)
}

pub(crate) fn run_habitat_get_nearest_keeper_food_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_keeper_food_pick_live_test(failure_log, KeeperFoodPickKind::Nearest)
}

pub(crate) fn run_habitat_get_random_keeper_food_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_keeper_food_pick_live_test(failure_log, KeeperFoodPickKind::Random)
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

/// Same "find a live `ZTKeeper`, compare over every habitat" shape, for `getNearestDirtPile`'s own
/// returned pointer.
pub(crate) fn run_habitat_get_nearest_dirt_pile_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_NEAREST_DIRT_PILE_MATCHES_REAL_LIVE";

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
            let real = unsafe { zthabitat::GET_NEAREST_DIRT_PILE.original()(ptr as *const u32, keeper_ptr as *const u32, check_can_see) } as u32;
            let reimpl = habitat.get_nearest_dirt_pile(keeper_ptr, check_can_see);
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

/// Same "find a live `ZTKeeper`, compare over every habitat" shape, for `needsShowKeeper`'s own boolean
/// result. Real vanilla's return is low-byte-only meaningful, so the real side is masked with
/// `low_byte_bool` before comparing.
pub(crate) fn run_habitat_needs_show_keeper_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_NEEDS_SHOW_KEEPER_MATCHES_REAL_LIVE";

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
        let real = low_byte_bool(unsafe { zthabitat::NEEDS_SHOW_KEEPER.original()(ptr as *const u32, keeper_ptr as *const u32) });
        let reimpl = habitat.needs_show_keeper(keeper_ptr);
        if real != reimpl {
            failures.push(format!("habitat {} ({:#010x}): real={}, reimpl={}", i, ptr, real, reimpl));
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

pub(crate) fn run_format_habitat_message_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_FORMAT_HABITAT_MESSAGE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }

        let string_id = 0x2523;
        let mut real_out = [0u32; 3];
        unsafe { zthabitatmgr::FORMAT_HABITAT_MESSAGE.original()(real_out.as_mut_ptr(), string_id, ptr as i32) };
        let real_bytes: Vec<u8> = if real_out[0] != 0 && real_out[1] >= real_out[0] {
            (real_out[0]..real_out[1]).map(get_from_memory::<u8>).collect()
        } else {
            Vec::new()
        };
        if real_out[0] != 0 {
            free_event_vector_buffer(real_out[0], real_out[2] - real_out[0]);
        }

        let mut reimpl_out = [0u32; 3];
        ZTHabitatMgr::format_habitat_message(reimpl_out.as_mut_ptr(), string_id, ptr);
        let reimpl_bytes: Vec<u8> = if reimpl_out[0] != 0 && reimpl_out[1] >= reimpl_out[0] {
            (reimpl_out[0]..reimpl_out[1]).map(get_from_memory::<u8>).collect()
        } else {
            Vec::new()
        };
        if reimpl_out[0] != 0 {
            free_event_vector_buffer(reimpl_out[0], reimpl_out[2] - reimpl_out[0]);
        }

        if real_bytes != reimpl_bytes {
            let real_str = String::from_utf8_lossy(&real_bytes);
            let reimpl_str = String::from_utf8_lossy(&reimpl_bytes);
            failures.push(format!("habitat {} ({:#010x}): real={:?}, reimpl={:?}", i, ptr, real_str, reimpl_str));
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

pub(crate) fn run_find_better_gates_for_neighbors_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_FIND_BETTER_GATES_FOR_NEIGHBORS_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    habitat_mgr.find_better_gates_for_neighbors(0);

    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(ptr) }.is_tank() {
            habitat_mgr.find_better_gates_for_neighbors(ptr);
        }
    }

    write_success_line(failure_log, test_name);
    false
}

pub(crate) fn run_update_gates_smoke_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_UPDATE_GATES_SMOKE_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    habitat_mgr.update_gates();
    write_success_line(failure_log, test_name);
    false
}

/// Which of the three biome-classification filters a [`run_habitat_add_biome_tiles_live_test`] run
/// exercises - the `.asm`-confirmed per-tile predicates, shared verbatim by the port, the real
/// vanilla call, and the oracle below.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BiomeTileKind {
    Land,
    Water,
    Underwater,
}

const ALL_BIOME_KINDS: [BiomeTileKind; 3] = [BiomeTileKind::Land, BiomeTileKind::Water, BiomeTileKind::Underwater];

impl BiomeTileKind {
    fn test_name(self) -> &'static str {
        match self {
            BiomeTileKind::Land => "ZTHABITAT_ADD_LAND_TILES_LIVE",
            BiomeTileKind::Water => "ZTHABITAT_ADD_WATER_TILES_LIVE",
            BiomeTileKind::Underwater => "ZTHABITAT_ADD_UNDERWATER_TILES_LIVE",
        }
    }

    /// The `.asm` predicate - the one instruction the three otherwise byte-for-byte identical
    /// Windows bodies differ by.
    fn qualifies(self, tile: u32) -> bool {
        match self {
            BiomeTileKind::Land => get_from_memory::<u8>(tile + 0x85) & 0x20 != 0,
            BiomeTileKind::Water => get_from_memory::<u8>(tile + 0x83) & 3 != 0,
            BiomeTileKind::Underwater => get_from_memory::<u8>(tile + 0x83) & 3 == 0 && get_from_memory::<u8>(tile + 0x85) & 0x20 == 0,
        }
    }

    /// Real vanilla call-through (debug: the trampoline; release: the raw address, which re-enters
    /// the port - same release semantics as [`vanilla_clear_tile_pool`]).
    fn real(self, habitat_ptr: u32, out_vector: &mut [u32; 3]) {
        let out = out_vector.as_mut_ptr() as *const i32;
        match self {
            BiomeTileKind::Land => unsafe { zthabitat::ADD_LAND_TILES.original()(habitat_ptr as *const u32, out) },
            BiomeTileKind::Water => unsafe { zthabitat::ADD_WATER_TILES.original()(habitat_ptr as *const u32, out) },
            BiomeTileKind::Underwater => unsafe { zthabitat::ADD_UNDERWATER_TILES.original()(habitat_ptr as *const u32, out) },
        }
    }

    /// The port under test.
    fn port(self, habitat: &ZTHabitat, out_vector_ptr: u32) {
        match self {
            BiomeTileKind::Land => habitat.add_land_tiles(out_vector_ptr),
            BiomeTileKind::Water => habitat.add_water_tiles(out_vector_ptr),
            BiomeTileKind::Underwater => habitat.add_underwater_tiles(out_vector_ptr),
        }
    }

    /// Get-side (recursive-aggregator) test name - same filter identity, Stage 17's functions.
    fn get_test_name(self) -> &'static str {
        match self {
            BiomeTileKind::Land => "ZTHABITAT_GET_LAND_TILES_LIVE",
            BiomeTileKind::Water => "ZTHABITAT_GET_WATER_TILES_LIVE",
            BiomeTileKind::Underwater => "ZTHABITAT_GET_UNDERWATER_TILES_LIVE",
        }
    }

    /// Real vanilla get-side call-through (same routing notes as [`BiomeTileKind::real`]).
    fn get_real(self, habitat_ptr: u32, out_vector: &mut [u32; 3]) {
        let out = out_vector.as_mut_ptr() as *const i32;
        match self {
            BiomeTileKind::Land => unsafe { zthabitat::GET_LAND_TILES.original()(habitat_ptr as *const u32, out) },
            BiomeTileKind::Water => unsafe { zthabitat::GET_WATER_TILES.original()(habitat_ptr as *const u32, out) },
            BiomeTileKind::Underwater => unsafe { zthabitat::GET_UNDERWATER_TILES.original()(habitat_ptr as *const u32, out) },
        }
    }

    /// The get-side port under test.
    fn get_port(self, habitat: &ZTHabitat, out_vector_ptr: u32) {
        match self {
            BiomeTileKind::Land => habitat.get_land_tiles(out_vector_ptr),
            BiomeTileKind::Water => habitat.get_water_tiles(out_vector_ptr),
            BiomeTileKind::Underwater => habitat.get_underwater_tiles(out_vector_ptr),
        }
    }

    /// Count-getter-side test name - same filter identity, the count-only wrappers.
    fn num_test_name(self) -> &'static str {
        match self {
            BiomeTileKind::Land => "ZTHABITAT_GET_NUM_LAND_TILES_LIVE",
            BiomeTileKind::Water => "ZTHABITAT_GET_NUM_WATER_TILES_LIVE",
            BiomeTileKind::Underwater => "ZTHABITAT_GET_NUM_UNDERWATER_TILES_LIVE",
        }
    }

    /// Real vanilla count-getter call-through (same routing notes as [`BiomeTileKind::real`]; the
    /// per-entry `*const u32`/`*const c_void` parameter spread in `generated.rs` is matched
    /// verbatim).
    fn num_real(self, habitat_ptr: u32) -> i32 {
        match self {
            BiomeTileKind::Land => unsafe { zthabitat::GET_NUM_LAND_TILES.original()(habitat_ptr as *const u32) },
            BiomeTileKind::Water => unsafe { zthabitat::GET_NUM_WATER_TILES.original()(habitat_ptr as *const std::ffi::c_void) },
            BiomeTileKind::Underwater => unsafe { zthabitat::GET_NUM_UNDERWATER_TILES.original()(habitat_ptr as *const std::ffi::c_void) },
        }
    }

    /// The count-getter port under test.
    fn num_port(self, habitat: &ZTHabitat) -> i32 {
        match self {
            BiomeTileKind::Land => habitat.get_num_land_tiles(),
            BiomeTileKind::Water => habitat.get_num_water_tiles(),
            BiomeTileKind::Underwater => habitat.get_num_underwater_tiles(),
        }
    }

    /// Random-getter-side test name - same filter identity, the random-draw wrappers.
    fn random_test_name(self) -> &'static str {
        match self {
            BiomeTileKind::Land => "ZTHABITAT_GET_RANDOM_LAND_TILE_LIVE",
            BiomeTileKind::Water => "ZTHABITAT_GET_RANDOM_WATER_TILE_LIVE",
            BiomeTileKind::Underwater => "ZTHABITAT_GET_RANDOM_UNDERWATER_TILE_LIVE",
        }
    }

    /// Real vanilla random-getter call-through (same routing notes as [`BiomeTileKind::real`]; the
    /// per-entry `*const u32`/`*const c_void` parameter spread in `generated.rs` is matched verbatim,
    /// and the `-> i32` return is the known pointer-as-integer wart - cast back to `u32` here).
    fn random_real(self, habitat_ptr: u32) -> u32 {
        match self {
            BiomeTileKind::Land => (unsafe { zthabitat::GET_RANDOM_LAND_TILE.original()(habitat_ptr as *const u32) }) as u32,
            BiomeTileKind::Water => (unsafe { zthabitat::GET_RANDOM_WATER_TILE.original()(habitat_ptr as *const std::ffi::c_void) }) as u32,
            BiomeTileKind::Underwater => (unsafe { zthabitat::GET_RANDOM_UNDERWATER_TILE.original()(habitat_ptr as *const u32) }) as u32,
        }
    }

    /// The random-getter port under test.
    fn random_port(self, habitat: &ZTHabitat) -> u32 {
        match self {
            BiomeTileKind::Land => habitat.get_random_land_tile(),
            BiomeTileKind::Water => habitat.get_random_water_tile(),
            BiomeTileKind::Underwater => habitat.get_random_underwater_tile(),
        }
    }
}

/// Oracle for [`run_habitat_add_biome_tiles_live_test`]: `tiles` filtered by `kind`'s `.asm`
/// predicate, in walk order, reading the two flag bytes directly off each tile - the same
/// oracle-reads-real-flags shape as [`nearest_clear_water_tile_oracle`]. Seed-independent - none of
/// the three functions draws.
fn biome_tile_oracle(tiles: &[u32], kind: BiomeTileKind) -> Vec<u32> {
    tiles.iter().copied().filter(|&tile| kind.qualifies(tile)).collect()
}

/// Extracts a real vanilla `std::vector<T*>` out-param's contents from a fresh zero-initialized
/// 3-word scratch (`begin`/`end`/`cap_end`), freeing the buffer by capacity afterward - the same
/// scratch management [`run_habitat_add_clear_tiles_matches_real_live_test`] uses
/// ([`free_event_vector_buffer`], [`vector_push_pool_alloc4`]'s own teardown shape).
fn extract_vector(fill: impl FnOnce(&mut [u32; 3])) -> Vec<u32> {
    let mut scratch = [0u32; 3];
    fill(&mut scratch);
    let tiles: Vec<u32> = (scratch[0]..scratch[1]).step_by(4).map(get_from_memory::<u32>).collect();
    free_event_vector_buffer(scratch[0], scratch[2].wrapping_sub(scratch[0]));
    tiles
}

/// Shared driver for the three biome-tile-filter tests: per live habitat, snapshots the owned-tile
/// list once (the same synchronous walk all three functions iterate on both sides), then checks one
/// real vanilla call and one port call per `kind` element-for-element against the fully independent
/// [`biome_tile_oracle`] - deterministic on both sides (no RNG draw, no `characteristics_dirty`
/// recalculate). The other two kinds' real-vanilla lists are computed anyway and folded into the
/// success line: the union of the three real lists must cover every owned tile by pointer identity
/// (the tri-partite guarantee the decompiles actually support - the two bits are independent, so
/// land/water overlap is possible in principle), and the count of tiles landing in 2+ lists is
/// reported rather than asserted, since exclusivity is only empirical.
fn run_habitat_add_biome_tiles_live_test(failure_log: &mut Option<std::fs::File>, kind: BiomeTileKind) -> bool {
    let test_name = kind.test_name();
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut habitats_checked = 0u32;
    let mut owned_total = 0u32;
    let mut real_counts = [0u32; 3];
    let mut overlap_tiles = 0u32;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        habitats_checked += 1;
        owned_total += tiles.len() as u32;

        let mut real_lists: [Vec<u32>; 3] = Default::default();
        for (k, k_kind) in ALL_BIOME_KINDS.iter().enumerate() {
            real_lists[k] = extract_vector(|scratch| k_kind.real(ptr, scratch));
            real_counts[k] += real_lists[k].len() as u32;
        }
        let reimpl_tiles = extract_vector(|scratch| kind.port(unsafe { ref_from_memory::<ZTHabitat>(ptr) }, scratch.as_mut_ptr() as u32));

        let expected = biome_tile_oracle(&tiles, kind);
        let real_tiles = &real_lists[ALL_BIOME_KINDS.iter().position(|&k| k == kind).unwrap()];
        if *real_tiles != expected {
            failures.push(format!("habitat {} ({:#010x}): real ({:?}) != oracle ({:?})", i, ptr, real_tiles, expected));
        }
        if reimpl_tiles != expected {
            failures.push(format!("habitat {} ({:#010x}): reimpl ({:?}) != oracle ({:?})", i, ptr, reimpl_tiles, expected));
        }

        for &tile in &tiles {
            let lists_containing = real_lists.iter().filter(|list| list.contains(&tile)).count();
            if lists_containing == 0 {
                failures.push(format!("habitat {} ({:#010x}): owned tile {:#010x} in no biome list", i, ptr, tile));
            }
            if lists_containing >= 2 {
                overlap_tiles += 1;
            }
        }
    }
    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, owned tiles: {}, land/water/underwater: {}/{}/{}, in >=2 lists: {})",
                test_name, habitats_checked, owned_total, real_counts[0], real_counts[1], real_counts[2], overlap_tiles
            ),
        );
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

pub(crate) fn run_habitat_add_land_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_add_biome_tiles_live_test(failure_log, BiomeTileKind::Land)
}

pub(crate) fn run_habitat_add_water_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_add_biome_tiles_live_test(failure_log, BiomeTileKind::Water)
}

pub(crate) fn run_habitat_add_underwater_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_add_biome_tiles_live_test(failure_log, BiomeTileKind::Underwater)
}

/// Shared driver for the three recursive biome-tile-aggregator tests (Stage 17's `get*Tiles`): per
/// live habitat, builds the expected accumulation independently of both get-sides - real vanilla's
/// own `add*Tiles` for `this` ([`BiomeTileKind::real`]), then the same call per amphibious neighbor
/// in `walk_neighbor_tree` order with the payload read from node `+0x10`, the decompile's own
/// aggregation shape - then checks one real vanilla call ([`BiomeTileKind::get_real`]) and one port
/// call ([`BiomeTileKind::get_port`]) element-for-element against it. Deterministic on all sides
/// (no RNG draw, no `characteristics_dirty` recalculate); the real vanilla getters' internal
/// `add*Tiles` calls hit their now-detoured addresses, so they re-enter the Stage 16 ports under
/// the battery - the same release-re-entry shape [`BiomeTileKind::real`] documents.
///
/// Ends with a non-vacuous-recursion assert: at least one habitat must carry a non-empty amphibious
/// neighbor tree contributing at least one tile, else a walk-order or neighbor-payload bug would
/// pass silently on a degenerate save - this needs the live save's genuine tank↔habitat amphibious
/// connection (the same dependency the `ZTHABITATMGR_ENTITY_*_SMOKE_LIVE` tests stopped skipping
/// for).
fn run_habitat_get_biome_tiles_live_test(failure_log: &mut Option<std::fs::File>, kind: BiomeTileKind) -> bool {
    let test_name = kind.get_test_name();
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut habitats_checked = 0u32;
    let mut habitats_with_neighbors = 0u32;
    let mut own_tiles_total = 0u32;
    let mut neighbor_tiles_total = 0u32;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        habitats_checked += 1;

        let own = extract_vector(|scratch| kind.real(ptr, scratch));
        own_tiles_total += own.len() as u32;
        let mut expected = own;
        let neighbor_ptrs: Vec<u32> =
            walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for &neighbor in &neighbor_ptrs {
            let part = extract_vector(|scratch| kind.real(neighbor, scratch));
            neighbor_tiles_total += part.len() as u32;
            expected.extend(part);
        }
        if !neighbor_ptrs.is_empty() {
            habitats_with_neighbors += 1;
        }

        let real_out = extract_vector(|scratch| kind.get_real(ptr, scratch));
        if real_out != expected {
            failures.push(format!(
                "habitat {} ({:#010x}, {} neighbors): real ({:?}) != expected ({:?})",
                i,
                ptr,
                neighbor_ptrs.len(),
                real_out,
                expected
            ));
        }
        let reimpl_out = extract_vector(|scratch| kind.get_port(habitat, scratch.as_mut_ptr() as u32));
        if reimpl_out != expected {
            failures.push(format!(
                "habitat {} ({:#010x}, {} neighbors): reimpl ({:?}) != expected ({:?})",
                i,
                ptr,
                neighbor_ptrs.len(),
                reimpl_out,
                expected
            ));
        }
    }
    if habitats_with_neighbors == 0 || neighbor_tiles_total == 0 {
        failures.push(format!(
            "non-vacuous-recursion assert failed: habitats with amphibious neighbors: {}, neighbor-contributed tiles: {} - the live save must contain a genuine amphibious connection for this test to exercise the recursion",
            habitats_with_neighbors, neighbor_tiles_total
        ));
    }
    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, with amphibious neighbors: {}, own tiles: {}, neighbor-contributed tiles: {})",
                test_name, habitats_checked, habitats_with_neighbors, own_tiles_total, neighbor_tiles_total
            ),
        );
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

pub(crate) fn run_habitat_get_land_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_biome_tiles_live_test(failure_log, BiomeTileKind::Land)
}

pub(crate) fn run_habitat_get_water_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_biome_tiles_live_test(failure_log, BiomeTileKind::Water)
}

pub(crate) fn run_habitat_get_underwater_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_biome_tiles_live_test(failure_log, BiomeTileKind::Underwater)
}

/// Shared driver for the three biome-tile count-getter tests (`getNum*Tiles`): per live habitat,
/// builds the expected count independently of both count-sides - the same expected construction as
/// [`run_habitat_get_biome_tiles_live_test`] (real vanilla's own `add*Tiles` via
/// [`BiomeTileKind::real`] for `this`, then the same call per amphibious neighbor in
/// `walk_neighbor_tree` order), with the expected count summed from the per-part element counts -
/// then checks one real vanilla call ([`BiomeTileKind::num_real`]) and one port call
/// ([`BiomeTileKind::num_port`]) against it. Deterministic on all sides (no RNG draw, no
/// `characteristics_dirty` recalculate); the real vanilla count getters' internal `get*Tiles` calls
/// hit their now-detoured addresses, so they re-enter the aggregator ports under the battery, whose
/// own `add*Tiles` calls re-enter the filter ports - the same release-re-entry shape
/// [`BiomeTileKind::real`] documents.
///
/// Ends with the same non-vacuous-recursion assert as the aggregator driver: at least one habitat
/// must carry a non-empty amphibious neighbor tree contributing at least one tile, else a
/// neighbor-walk bug would pass silently on a degenerate save.
fn run_habitat_get_num_biome_tiles_live_test(failure_log: &mut Option<std::fs::File>, kind: BiomeTileKind) -> bool {
    let test_name = kind.num_test_name();
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut habitats_checked = 0u32;
    let mut habitats_with_neighbors = 0u32;
    let mut own_tiles_total = 0u32;
    let mut neighbor_tiles_total = 0u32;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        habitats_checked += 1;

        let own = extract_vector(|scratch| kind.real(ptr, scratch));
        own_tiles_total += own.len() as u32;
        let mut expected = own.len() as i32;
        let neighbor_ptrs: Vec<u32> =
            walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for &neighbor in &neighbor_ptrs {
            let part = extract_vector(|scratch| kind.real(neighbor, scratch));
            neighbor_tiles_total += part.len() as u32;
            expected += part.len() as i32;
        }
        if !neighbor_ptrs.is_empty() {
            habitats_with_neighbors += 1;
        }

        let real_count = kind.num_real(ptr);
        if real_count != expected {
            failures.push(format!(
                "habitat {} ({:#010x}, {} neighbors): real count {} != expected {}",
                i, ptr, neighbor_ptrs.len(), real_count, expected
            ));
        }
        let reimpl_count = kind.num_port(habitat);
        if reimpl_count != expected {
            failures.push(format!(
                "habitat {} ({:#010x}, {} neighbors): reimpl count {} != expected {}",
                i, ptr, neighbor_ptrs.len(), reimpl_count, expected
            ));
        }
    }
    if habitats_with_neighbors == 0 || neighbor_tiles_total == 0 {
        failures.push(format!(
            "non-vacuous-recursion assert failed: habitats with amphibious neighbors: {}, neighbor-contributed tiles: {} - the live save must contain a genuine amphibious connection for this test to exercise the recursion",
            habitats_with_neighbors, neighbor_tiles_total
        ));
    }
    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, with amphibious neighbors: {}, own tiles: {}, neighbor-contributed tiles: {})",
                test_name, habitats_checked, habitats_with_neighbors, own_tiles_total, neighbor_tiles_total
            ),
        );
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

pub(crate) fn run_habitat_get_num_land_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_num_biome_tiles_live_test(failure_log, BiomeTileKind::Land)
}

pub(crate) fn run_habitat_get_num_water_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_num_biome_tiles_live_test(failure_log, BiomeTileKind::Water)
}

pub(crate) fn run_habitat_get_num_underwater_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_num_biome_tiles_live_test(failure_log, BiomeTileKind::Underwater)
}

/// Shared driver for the three biome-tile random-getter tests: per live habitat, builds the expected
/// aggregate once - the real vanilla filter ([`BiomeTileKind::real`]) over `this` plus the same per
/// amphibious neighbor in [`walk_neighbor_tree`] order, exactly what both real vanilla's `get*Tiles`
/// callee and the port's `get_tiles_aggregating` produce - then snapshots the shared game RNG state
/// and checks one reimpl draw and one real vanilla draw, each against [`assert_lcg_tile_draw`] over
/// that same oracle list. Asserting real vanilla against the oracle validates the draw formula
/// itself, not just cross-agreement. An empty pool expects a null return and an untouched seed (the
/// `.asm`'s `JLE` gate sits before any RNG touch) and is exercised naturally per habitat - water on
/// dry habitats, underwater on non-tanks. Deterministic apart from the one LCG advance: no
/// `characteristics_dirty` recalculate anywhere in these bodies, so no settling draw is needed.
///
/// Non-vacuity (mirroring [`run_habitat_get_num_biome_tiles_live_test`]): the save must carry at
/// least one habitat with a non-empty amphibious neighbor tree, and at least one non-empty aggregate
/// for this kind - so the draw path itself actually ran on at least one habitat (save-dependent like
/// the sibling tests).
fn run_habitat_get_random_biome_tile_live_test(failure_log: &mut Option<std::fs::File>, kind: BiomeTileKind) -> bool {
    let test_name = kind.random_test_name();
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut fail_flag = false;
    let mut vacuity_failures: Vec<String> = Vec::new();
    let mut habitats_checked = 0u32;
    let mut habitats_with_neighbors = 0u32;
    let mut non_empty_aggregates = 0u32;
    let mut empty_pools = 0u32;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        habitats_checked += 1;

        let mut expected = extract_vector(|scratch| kind.real(ptr, scratch));
        let neighbor_ptrs: Vec<u32> =
            walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for &neighbor in &neighbor_ptrs {
            expected.extend(extract_vector(|scratch| kind.real(neighbor, scratch)));
        }
        if !neighbor_ptrs.is_empty() {
            habitats_with_neighbors += 1;
        }
        if expected.is_empty() {
            empty_pools += 1;
        } else {
            non_empty_aggregates += 1;
        }

        let seed_before: u32 = get_from_memory(rng_addr);
        let reimpl_draw = kind.random_port(habitat);
        fail_flag |= assert_lcg_tile_draw(
            failure_log,
            test_name,
            "reimpl",
            i,
            ptr,
            &expected,
            seed_before,
            get_from_memory(rng_addr),
            reimpl_draw,
        );

        let seed_before: u32 = get_from_memory(rng_addr);
        let real_draw = kind.random_real(ptr);
        fail_flag |= assert_lcg_tile_draw(
            failure_log,
            test_name,
            "real",
            i,
            ptr,
            &expected,
            seed_before,
            get_from_memory(rng_addr),
            real_draw,
        );
    }
    if habitats_with_neighbors == 0 {
        vacuity_failures.push(
            "non-vacuous-recursion assert failed: no habitat has amphibious neighbors - the live save must contain a genuine amphibious connection for this test to exercise the neighbor walk".to_string(),
        );
    }
    if non_empty_aggregates == 0 {
        vacuity_failures.push(format!(
            "non-vacuous-draw assert failed: every habitat's {:?} aggregate is empty - the draw path never ran on this save",
            kind
        ));
    }
    if fail_flag || !vacuity_failures.is_empty() {
        for msg in &vacuity_failures {
            error!("{}: {}", test_name, msg);
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, vacuity_failures.join("; ")).as_bytes());
        }
        true
    } else {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, with amphibious neighbors: {}, non-empty aggregates: {}, empty pools: {})",
                test_name, habitats_checked, habitats_with_neighbors, non_empty_aggregates, empty_pools
            ),
        );
        false
    }
}

pub(crate) fn run_habitat_get_random_land_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_random_biome_tile_live_test(failure_log, BiomeTileKind::Land)
}

pub(crate) fn run_habitat_get_random_water_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_random_biome_tile_live_test(failure_log, BiomeTileKind::Water)
}

pub(crate) fn run_habitat_get_random_underwater_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    run_habitat_get_random_biome_tile_live_test(failure_log, BiomeTileKind::Underwater)
}
