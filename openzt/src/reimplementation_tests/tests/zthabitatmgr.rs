//! Compares the reimplemented `ZTHabitatMgr`/`ZTHabitat` methods (production file
//! `openzt/src/zthabitatmgr.rs`) against real vanilla over the live, loaded zoo's own habitat grid.

use openzt_detour::generated::{
    bfaimgr::CHECK_PATH as BFAIMGR_CHECK_PATH,
    bfentity,
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0, WORLD_TO_TILE},
    msvc_std_mapint_habitatsuitability::{TREE as MSVC_MAP_INT_HABITATSUITABILITY_TREE, TREE_DTOR as MSVC_MAP_INT_HABITATSUITABILITY_TREE_DTOR},
    standalone::OPERATOR_NEW,
    ztanimal::CAN_SERVICE as ZTANIMAL_CAN_SERVICE,
    zthabitat, zthabitatmgr, ztui_general::GET_MAPVIEW as ZTUI_GENERAL_GET_MAPVIEW, ztviewingarea,
};
use std::fmt::Debug;
use std::io::Write;
use tracing::error;

use crate::globals::{get_module_base, globals};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::portal_dispatch_recorder;
use crate::util::{get_from_memory, low_byte_bool, mut_from_memory, ref_from_memory, save_to_memory};
use crate::zthabitat::support::{call_vtable_slot_ptr_ptr_ptr_u32_ret_bool, call_vtable_slot_with_ptr_ret_bool};
use crate::ztmapview::BFTile;
use crate::ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG};
use crate::ztshow::{call_entity_vtable_noargs, call_entity_vtable_u32_noargs, RVA_ANIMAL_TYPE_CHECK};
use crate::ztshowinfo::needs_keeper;
use crate::zthabitatmgr::{
    animal_food_target, call_bfunit_tile_cost_vtable_slot, call_vtable_slot_noargs_ret_bool, entity_name_bytes, free_event_vector_buffer,
    hooks_zthabitatmgr, map_int_habitatsuitability_find_or_insert, walk_neighbor_tree, walk_tile_list, TileListNode, ZTHabitat, ZTHabitatMgr, ZTTankExhibit,
    MAX_PATH_COST_RVA, MORPH_EXHIBIT_CALL_LOG, RVA_KEEPER_TYPE_CHECK_ARG, RVA_ZTFOOD_TYPE_CHECK_ARG, TILE_LIST_NODE_FREELIST_HEAD_RVA,
};
use crate::zthabitatmgr::tank_exhibit::detours;

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
        |ptr| {
            sync_real_boundary_pairs(ptr as u32);
            unsafe { zthabitatmgr::DO_TANK_CHECK.original()(ptr as i32) }
        },
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

/// Compares `hasPortalAnimal` (`ZTHabitat_hasPortalAnimal.c`/`.asm`) for every (habitat, target)
/// pair over the live zoo's own habitats, plus a null target - exercising vanilla's own
/// null-destination-tile == null-parameter arm and the full `all_animals` walk on both sides. The
/// plan's "false on stationary exhibits" expectation is asserted directly: a habitat whose
/// `all_animals` vector is empty must answer `false` for every non-null target on both sides.
pub(crate) fn run_habitat_has_portal_animal_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_HAS_PORTAL_ANIMAL_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut stationary_habitats = 0u32;
    let mut comparisons = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let stationary = habitat.all_animals_begin == habitat.all_animals_end;
        for &target_ptr in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
            let real = unsafe { zthabitat::HAS_PORTAL_ANIMAL.original()(habitat_ptr as *const u32, target_ptr as *const u32) };
            let reimpl = habitat.has_portal_animal(target_ptr);
            comparisons += 1;
            if real != reimpl {
                failures.push(format!(
                    "habitat {:#010x}, target {:#010x}: real={}, reimpl={}",
                    habitat_ptr, target_ptr, real, reimpl
                ));
            }
            if stationary && target_ptr != 0 && (real || reimpl) {
                failures.push(format!(
                    "stationary exhibit {:#010x} (no animals) answered true for target {:#010x}",
                    habitat_ptr, target_ptr
                ));
            }
        }
        if stationary {
            stationary_habitats += 1;
        }
    }
    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, comparisons: {}, animal-free habitats: {})",
                test_name,
                habitat_ptrs.len(),
                comparisons,
                stationary_habitats
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

/// Frees a `getTilesCopy`-produced list's own nodes back to the shared bucket-1 freelist
/// ([`TILE_LIST_NODE_FREELIST_HEAD_RVA`]) - every node in it, sentinel included, was allocated by real
/// vanilla's own `PoolAlloc::allocate`, so pushing it back is exactly what vanilla's own free path does
/// (no cross-allocator hazard, unlike a `Box`-walking cleanup). Mirrors
/// [`run_habitat_remove_habitat_tiles_live_test`]'s own teardown loop.
fn free_tile_list_copy(sentinel: u32) {
    let freelist_head_addr = get_module_base("zoo.exe") as u32 + TILE_LIST_NODE_FREELIST_HEAD_RVA;
    for node in walk_tile_list(sentinel) {
        let old_head: u32 = get_from_memory(freelist_head_addr);
        save_to_memory(node, old_head);
        save_to_memory(freelist_head_addr, node);
    }
    let old_head: u32 = get_from_memory(freelist_head_addr);
    save_to_memory(sentinel, old_head);
    save_to_memory(freelist_head_addr, sentinel);
}

/// Compares `getTilesCopy` (`ZTHabitat_getTilesCopy.c`/`.asm`) against the reimplementation over every
/// live habitat's own owned-tile list: real vanilla's copy first, then the reimplementation's, each into
/// its own out-param slot, verifying both return the pointer passed in (RVO return), both copies walk to
/// the exact same payload sequence as a snapshot of the source list taken before either call, and the
/// source list itself is untouched afterward (catches a `where`/`first` argument swap splicing the
/// source's own nodes into the copy instead of inserting a copy of them). Every node either copy produces
/// is freed back to the shared freelist ([`free_tile_list_copy`]) - never through `Box`.
pub(crate) fn run_habitat_get_tiles_copy_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_TILES_COPY_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut total_tiles = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let source_payloads: Vec<u32> = walk_tile_list(*habitat.owned_tiles_ptr())
            .map(|node| get_from_memory::<TileListNode>(node).payload)
            .collect();
        total_tiles += source_payloads.len() as u32;

        let mut real_out: u32 = 0;
        let real_out_addr = &mut real_out as *mut u32 as u32;
        let real_ret = hooks_zthabitatmgr::get_tiles_copy_real(habitat_ptr as *const u32, real_out_addr as *const i32) as u32;

        let mut reimpl_out: u32 = 0;
        let reimpl_out_addr = &mut reimpl_out as *mut u32 as u32;
        let reimpl_ret = habitat.get_tiles_copy(reimpl_out_addr);

        if real_ret != real_out_addr {
            failures.push(format!("habitat {:#010x}: real getTilesCopy returned {:#010x}, expected out-param address {:#010x}", habitat_ptr, real_ret, real_out_addr));
        }
        if reimpl_ret != reimpl_out_addr {
            failures.push(format!("habitat {:#010x}: reimpl get_tiles_copy returned {:#010x}, expected out-param address {:#010x}", habitat_ptr, reimpl_ret, reimpl_out_addr));
        }

        let real_sentinel = real_out;
        let reimpl_sentinel = reimpl_out;
        if real_sentinel == 0 {
            failures.push(format!("habitat {:#010x}: real getTilesCopy produced a null sentinel", habitat_ptr));
        }
        if reimpl_sentinel == 0 {
            failures.push(format!("habitat {:#010x}: reimpl get_tiles_copy produced a null sentinel", habitat_ptr));
        }

        if real_sentinel != 0 && reimpl_sentinel != 0 {
            let real_payloads: Vec<u32> = walk_tile_list(real_sentinel).map(|node| get_from_memory::<TileListNode>(node).payload).collect();
            let reimpl_payloads: Vec<u32> = walk_tile_list(reimpl_sentinel).map(|node| get_from_memory::<TileListNode>(node).payload).collect();

            if real_payloads != source_payloads {
                failures.push(format!(
                    "habitat {:#010x}: real copy payloads {:?} != source snapshot {:?}",
                    habitat_ptr, real_payloads, source_payloads
                ));
            }
            if reimpl_payloads != source_payloads {
                failures.push(format!(
                    "habitat {:#010x}: reimpl copy payloads {:?} != source snapshot {:?}",
                    habitat_ptr, reimpl_payloads, source_payloads
                ));
            }

            free_tile_list_copy(real_sentinel);
            free_tile_list_copy(reimpl_sentinel);
        } else {
            if real_sentinel != 0 {
                free_tile_list_copy(real_sentinel);
            }
            if reimpl_sentinel != 0 {
                free_tile_list_copy(reimpl_sentinel);
            }
        }

        let source_after: Vec<u32> = walk_tile_list(*habitat.owned_tiles_ptr())
            .map(|node| get_from_memory::<TileListNode>(node).payload)
            .collect();
        if source_after != source_payloads {
            failures.push(format!(
                "habitat {:#010x}: source list changed by getTilesCopy - before {:?}, after {:?}",
                habitat_ptr, source_payloads, source_after
            ));
        }
    }

    if total_tiles == 0 {
        let msg = "no owned tiles found across any live habitat - insert_range's non-empty arm never ran".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (habitats: {}, total tiles: {})", test_name, habitat_ptrs.len(), total_tiles));
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

/// Compares `isShowNeighbor` (`ZTHabitat_isShowNeighbor.c`/`.asm`) for every (habitat, target) pair
/// over the live zoo's own habitats, plus a null target, cross-checking both sides against an
/// independent membership oracle - a [`walk_neighbor_tree`] linear scan of the same
/// `show_neighbors_head` tree the port binary-searches (each node's `+0x10` payload, the same read
/// `hilite_show_neighbors`' own walk performs), so a descent bug cannot agree with itself.
/// The real vanilla return carries garbage upper bytes, so it is masked with [`low_byte_bool`]
/// before comparing. Coverage counters (habitats, comparisons, non-empty show-neighbor trees, true
/// hits) are logged so a save with no show tanks is visibly comparison-only rather than silently
/// green - the populated-tree paths are then carried by habitat.rs's own host unit tests.
pub(crate) fn run_habitat_is_show_neighbor_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_IS_SHOW_NEIGHBOR_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut populated_trees = 0u32;
    let mut true_hits = 0u32;
    let mut comparisons = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let tree_members: Vec<u32> = walk_neighbor_tree(*habitat.show_neighbors_head())
            .map(|node| get_from_memory::<u32>(node + 0x10))
            .collect();
        if !tree_members.is_empty() {
            populated_trees += 1;
        }
        for &target_ptr in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
            let real = hooks_zthabitatmgr::is_show_neighbor_real(habitat_ptr as *const u32, target_ptr as *const u32);
            let reimpl = habitat.is_show_neighbor(target_ptr);
            let oracle = tree_members.contains(&target_ptr);
            comparisons += 1;
            if real != reimpl {
                failures.push(format!(
                    "habitat {:#010x}, target {:#010x}: real={}, reimpl={}",
                    habitat_ptr, target_ptr, real, reimpl
                ));
            }
            if oracle != real {
                failures.push(format!(
                    "habitat {:#010x}, target {:#010x}: real={} != tree-walk oracle {}",
                    habitat_ptr, target_ptr, real, oracle
                ));
            }
            if real {
                true_hits += 1;
            }
        }
    }
    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, comparisons: {}, non-empty show-neighbor trees: {}, true hits: {})",
                test_name,
                habitat_ptrs.len(),
                comparisons,
                populated_trees,
                true_hits
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

/// Compares `getShowPortal` (`generated.rs`'s `GET_SHOW_PORTAL`, `0x0059e0a9`) for every (habitat,
/// other) pair over the live zoo's own habitats, plus a null `other`, covering both the tank
/// (map-descent) and non-tank (amphibious-neighbor recursion) branches on either side of the pair.
/// For tank habitats, additionally cross-checks against an independent oracle - a
/// [`walk_neighbor_tree`] linear scan of the same `show_portal_map_head` tree the port
/// binary-searches, reading each node's `+0x10` key / `+0x14` value pair directly - so a descent bug
/// cannot agree with itself. Real vanilla is a pure read (see [`ZTHabitat::get_show_portal`]'s own
/// doc comment for why the plan's original find-or-insert sketch was wrong), so this never mutates
/// habitat state. Coverage counters (habitats, comparisons, non-empty portal maps, non-null hits)
/// are logged so a save with no show tanks is visibly comparison-only.
pub(crate) fn run_habitat_get_show_portal_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_SHOW_PORTAL_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut populated_maps = 0u32;
    let mut non_null_hits = 0u32;
    let mut comparisons = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let is_tank = habitat.is_tank();
        let map_entries: Vec<(u32, u32)> = if is_tank {
            walk_neighbor_tree(*habitat.show_portal_map_head())
                .map(|node| (get_from_memory::<u32>(node + 0x10), get_from_memory::<u32>(node + 0x14)))
                .collect()
        } else {
            Vec::new()
        };
        if !map_entries.is_empty() {
            populated_maps += 1;
        }
        for &other_ptr in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
            let real = hooks_zthabitatmgr::get_show_portal_real(habitat_ptr as *const u32, other_ptr as *const u32) as u32;
            let reimpl = habitat.get_show_portal(other_ptr);
            comparisons += 1;
            if real != reimpl {
                failures.push(format!(
                    "habitat {:#010x}, other {:#010x}: real={:#010x}, reimpl={:#010x}",
                    habitat_ptr, other_ptr, real, reimpl
                ));
            }
            if is_tank {
                let oracle = map_entries.iter().find(|(key, _)| *key == other_ptr).map(|(_, value)| *value).unwrap_or(0);
                if oracle != real {
                    failures.push(format!(
                        "habitat {:#010x}, other {:#010x}: real={:#010x} != map-walk oracle {:#010x}",
                        habitat_ptr, other_ptr, real, oracle
                    ));
                }
            }
            if real != 0 {
                non_null_hits += 1;
            }
        }
    }
    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, comparisons: {}, non-empty portal maps: {}, non-null hits: {})",
                test_name,
                habitat_ptrs.len(),
                comparisons,
                populated_maps,
                non_null_hits
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

/// Compares real vanilla `ZTHabitat::updatePortals` (`generated.rs`'s `UPDATE_PORTALS`, `0x0043578f`)
/// against the reimplementation's [`ZTHabitat::portal_dispatch_plan`] over every live tank-show
/// habitat. Real `updatePortals`'s only observable effect is one `+0x13c` vtable dispatch per
/// show-neighbor pair reaching `ZTTankWall::setIsOpenPortal` - `portal_dispatch_recorder` intercepts
/// that address for the duration of the real call and records `(fence_ptr, is_open, play_sound)`
/// instead of letting it run (which would flip real fence state and fire a sound), so the two sides
/// can be diffed without mutating anything. The real pole goes through
/// `hooks_zthabitatmgr::update_portals_real` (the `UPDATE_PORTALS_DETOUR.call()` release-safe
/// trampoline), never `.original()` directly, since `UPDATE_PORTALS` is itself detoured in this build.
/// Sequences are compared in walk order (not sorted) - this also proves `portal_dispatch_plan`'s walk
/// visits neighbors in the same order real vanilla's `show_neighbors_head` descent does, after
/// dropping any reimpl-only entries whose fence target is a plain `ZTFence` (`NULLSUB` slot - the
/// recorder can never see those fire, so they carry no observable disagreement). Coverage counters
/// (tank-show habitats, dispatches recorded) are logged so a save with no show tanks is visibly
/// comparison-only.
pub(crate) fn run_habitat_update_portals_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_UPDATE_PORTALS_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }

    let mut failures: Vec<String> = Vec::new();
    let mut tank_show_habitats = 0u32;
    let mut total_dispatches = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        if !habitat.is_tank() || !habitat.is_show_tank() {
            continue;
        }
        tank_show_habitats += 1;

        portal_dispatch_recorder::begin_capture();
        hooks_zthabitatmgr::update_portals_real(habitat_ptr as *const u32);
        let recorded = portal_dispatch_recorder::end_capture();

        let recorded_fences: std::collections::HashSet<u32> = recorded.iter().map(|(fence, _, _)| *fence).collect();
        let expected: Vec<(u32, bool, bool)> = habitat
            .portal_dispatch_plan()
            .into_iter()
            .map(|d| (d.fence_ptr, d.is_open, d.play_sound))
            .filter(|(fence, _, _)| recorded_fences.contains(fence))
            .collect();

        total_dispatches += recorded.len() as u32;
        if recorded != expected {
            failures.push(format!("habitat {:#010x}: real dispatches {:?} != reimpl plan {:?}", habitat_ptr, recorded, expected));
        }
    }
    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (tank-show habitats: {}, dispatches: {})", test_name, tank_show_habitats, total_dispatches));
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

/// Compares `addToBuildingList` (`ZTHabitat_addToBuildingList.c`/`.asm`) against the reimplementation
/// over every live habitat's own owned-tile list: each side merges the *same* real habitat's occupants
/// into its own fresh, empty "other" building-list buffer (`other_ptr = out_vector_ptr - 0x78`, so the
/// dedup scan and the push both land in the same freshly zeroed 3-word vector - `self` itself is never
/// mutated by this function, so both sides can safely read the one live habitat), then the two resulting
/// lists are compared for exact order/contents equality. Growth buffers are freed back through
/// [`free_event_vector_buffer`] - the same allocator [`crate::zthabitatmgr::vector_push_pool_alloc4`]
/// itself uses, so this is real-vanilla-allocator-safe even when a push actually grows the buffer.
pub(crate) fn run_habitat_add_to_building_list_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_ADD_TO_BUILDING_LIST_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut compared_habitats = 0u32;
    let mut total_pushed = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        if walk_tile_list(*habitat.owned_tiles_ptr()).count() == 0 {
            continue;
        }
        compared_habitats += 1;

        let mut real_other_buf: [u32; 3] = [0, 0, 0];
        let mut reimpl_other_buf: [u32; 3] = [0, 0, 0];
        let real_out_vector_ptr = real_other_buf.as_mut_ptr() as u32;
        let reimpl_out_vector_ptr = reimpl_other_buf.as_mut_ptr() as u32;
        let real_other_ptr = real_out_vector_ptr - 0x78;
        let reimpl_other_ptr = reimpl_out_vector_ptr - 0x78;

        hooks_zthabitatmgr::add_to_building_list_real(habitat_ptr as *const u32, real_out_vector_ptr as *const i32, real_other_ptr as *const u32);
        habitat.add_to_building_list(reimpl_out_vector_ptr, reimpl_other_ptr);

        let real_entries: Vec<u32> = (real_other_buf[0]..real_other_buf[1]).step_by(4).map(get_from_memory::<u32>).collect();
        let reimpl_entries: Vec<u32> = (reimpl_other_buf[0]..reimpl_other_buf[1]).step_by(4).map(get_from_memory::<u32>).collect();
        total_pushed += real_entries.len() as u32;

        if real_entries != reimpl_entries {
            failures.push(format!("habitat {:#010x}: real building list {:?} != reimpl {:?}", habitat_ptr, real_entries, reimpl_entries));
        }

        free_event_vector_buffer(real_other_buf[0], real_other_buf[2] - real_other_buf[0]);
        free_event_vector_buffer(reimpl_other_buf[0], reimpl_other_buf[2] - reimpl_other_buf[0]);
    }

    if compared_habitats == 0 {
        let msg = "no live habitat had any owned tiles".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (habitats: {}, buildings pushed: {})", test_name, compared_habitats, total_pushed));
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

/// Compares `additionalScenerySuitabilityChange` (`ZTHabitat_additionalScenerySuitabilityChange.c`/`.asm`)
/// against the reimplementation over every live habitat's own owned-tile list and species list: each side
/// populates its own fresh, empty real-vanilla `msvc_std::map<int, ZTHabitatSuitabilityRecord>`
/// ([`MSVC_MAP_INT_HABITATSUITABILITY_TREE`], real vanilla's own `operator_new`-backed head node - torn
/// down afterward through [`MSVC_MAP_INT_HABITATSUITABILITY_TREE_DTOR`], never `Box`, so there is no
/// cross-allocator hazard even though every node either side inserts is real vanilla heap memory), then
/// every species passing the reimplementation's own `+0xcc` gate is looked up
/// ([`map_int_habitatsuitability_find_or_insert`] - a pure lookup here, since the key must already exist
/// on both sides) and its record's `occurrence_count`/`category_score_sum`/`tiles_with_match_count`/
/// `matching_item_count` fields diffed byte-for-byte.
pub(crate) fn run_habitat_additional_scenery_suitability_change_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_ADDITIONAL_SCENERY_SUITABILITY_CHANGE_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut compared_habitats = 0u32;
    let mut compared_species = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let species: Vec<u32> = habitat.species_list().collect();
        if species.is_empty() {
            continue;
        }

        let species_vector = [*habitat.species_list_begin(), *habitat.species_list_end()];
        let species_vector_ptr = species_vector.as_ptr() as u32;

        let comparator_byte: i32 = 0;
        let allocator_byte: i8 = 0;
        let mut real_map: [u32; 4] = [0; 4];
        let mut reimpl_map: [u32; 4] = [0; 4];
        unsafe {
            MSVC_MAP_INT_HABITATSUITABILITY_TREE.original()(real_map.as_mut_ptr() as *const i32, &comparator_byte as *const i32, &allocator_byte as *const i8);
            MSVC_MAP_INT_HABITATSUITABILITY_TREE.original()(
                reimpl_map.as_mut_ptr() as *const i32,
                &comparator_byte as *const i32,
                &allocator_byte as *const i8,
            );
        }
        let real_map_ptr = real_map.as_ptr() as u32;
        let reimpl_map_ptr = reimpl_map.as_ptr() as u32;

        hooks_zthabitatmgr::additional_scenery_suitability_change_real(
            habitat_ptr as *const u32,
            species_vector_ptr as *const i32,
            real_map_ptr as *const i32,
        );
        habitat.additional_scenery_suitability_change(species_vector_ptr, reimpl_map_ptr);

        let mut any_species_compared = false;
        for &species_ptr in &species {
            if !unsafe { call_entity_vtable_noargs(species_ptr, 0xcc) } {
                continue;
            }
            any_species_compared = true;
            compared_species += 1;
            let key: i32 = get_from_memory(species_ptr + 0x1ec);
            let real_record = map_int_habitatsuitability_find_or_insert(real_map_ptr, key);
            let reimpl_record = map_int_habitatsuitability_find_or_insert(reimpl_map_ptr, key);
            for &field_offset in &[0x0u32, 0x10, 0x14, 0x1c] {
                let real_val: u32 = get_from_memory(real_record + field_offset);
                let reimpl_val: u32 = get_from_memory(reimpl_record + field_offset);
                if real_val != reimpl_val {
                    failures.push(format!(
                        "habitat {:#010x}, species {:#010x}, key {}, field +{:#x}: real={:#010x}, reimpl={:#010x}",
                        habitat_ptr, species_ptr, key, field_offset, real_val, reimpl_val
                    ));
                }
            }
        }
        if any_species_compared {
            compared_habitats += 1;
        }

        unsafe {
            MSVC_MAP_INT_HABITATSUITABILITY_TREE_DTOR.original()(real_map.as_mut_ptr() as *const i32);
            MSVC_MAP_INT_HABITATSUITABILITY_TREE_DTOR.original()(reimpl_map.as_mut_ptr() as *const i32);
        }
    }

    if compared_species == 0 {
        let msg = "no species passed the +0xcc gate across any live habitat".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (habitats: {}, species compared: {})", test_name, compared_habitats, compared_species));
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
/// `true`, over every non-tank habitat in the live, loaded zoo (`ZTTankExhibit`'s own override is
/// compared separately by `ZTTANKEXHIBIT_IS_RIGHT_SALINITY_MATCHES_REAL_LIVE` below). Passes a null
/// `ZTAnimalType*`, matching the reimplementation's own disregard for the argument - safe only because
/// the base default is confirmed to never dereference it.
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

/// `ZTTANKEXHIBIT_DETOURS_ENABLED` - wiring check for `tank_exhibit::detours` (see
/// [`run_zthabitatmgr_detours_enabled_test`]): `zthabitat::init()` installs the tank exhibit class's
/// own detours alongside the manager/habitat ones, and this asserts they actually report enabled.
pub(crate) fn run_tankexhibit_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = detours::status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
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

/// Compares `ZTTankExhibit::isRightSalinity`'s port against real vanilla over every (tank habitat,
/// animal type present in the loaded save) pair - the same cross-product shape
/// `run_habitat_add_baby_born_bonus_live_test` uses for its type enumeration, with the tank set
/// replacing the habitat set. The "real" pole goes through the release-safe
/// `tank_exhibit::detours::is_right_salinity_real` trampoline rather than `.original()` (the address is
/// detoured, so a release `.original()` would re-enter the Rust detour and compare the port against
/// itself); the reimplementation pole calls the ported method directly. Both sides are pure reads, so
/// nothing needs restoring. Fails on an empty tank set or an empty animal-type set so the cross-product
/// can never pass vacuously.
pub(crate) fn run_tankexhibit_is_right_salinity_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_IS_RIGHT_SALINITY_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let mut tank_ptrs: Vec<u32> = Vec::new();
    // Union of the zoo's real `ZTAnimalType` pointers, in habitat order.
    let mut type_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        if habitat.is_tank() {
            tank_ptrs.push(ptr);
        }
        for animal_ptr in habitat.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            if animal_type_ptr != 0 && !type_ptrs.contains(&animal_type_ptr) {
                type_ptrs.push(animal_type_ptr);
            }
        }
    }
    if tank_ptrs.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    if type_ptrs.is_empty() {
        failures.push("no animal types found in the loaded save".to_string());
    }

    for &tank_ptr in &tank_ptrs {
        for &type_ptr in &type_ptrs {
            let real = detours::is_right_salinity_real(tank_ptr as *const u32, type_ptr as *const u32);
            let reimpl = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) }.is_right_salinity(type_ptr);
            if real != reimpl {
                failures.push(format!("mismatch at tank {tank_ptr:#010x} x animal type {type_ptr:#010x}: real={real:?}, reimpl={reimpl:?}"));
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

/// The `+0x184`-onward scalars `ZTTankExhibit::update`'s tick reads and writes, snapshotted/restored
/// through raw memory at the offsets the struct's own `offset_of!` asserts pin (same raw-offset
/// convention as the `+0x68` exhibit-number counter test - the fields are private to `tank_exhibit.rs`,
/// so there is nothing to `offset_of!` at this use site). Deliberately excludes every pointer field:
/// real vanilla may realloc a sparkle vector or free dead sparkle entities inside
/// `removeDeadSparkles`/`addRandomSparkle`, and restoring pre-mutation pointers afterward would
/// resurrect dangling references - whatever vanilla does to the vectors during a pole persists.
#[derive(Clone, Copy, PartialEq, Debug)]
struct TankUpdateScalars {
    water_level: u32,
    current_water_type: i32,
    pending_water_type: i32,
    is_filled: u8,
    water_purity: i32,
    water_purity_timer: i32,
    sparkle_spawn_timer: i32,
}

const TANK_WATER_LEVEL_OFFSET: u32 = 0x188;
const TANK_CURRENT_WATER_TYPE_OFFSET: u32 = 0x18c;
const TANK_PENDING_WATER_TYPE_OFFSET: u32 = 0x190;
const TANK_IS_FILLED_OFFSET: u32 = 0x198;
const TANK_WATER_PURITY_OFFSET: u32 = 0x1a8;
const TANK_WATER_PURITY_TIMER_OFFSET: u32 = 0x1ac;
const TANK_SPARKLE_ENTITIES_BEGIN_OFFSET: u32 = 0x1c4;
const TANK_SPARKLE_ENTITIES_END_OFFSET: u32 = 0x1c8;
const TANK_SPARKLE_ENTITY_IDS_BEGIN_OFFSET: u32 = 0x1d0;
const TANK_SPARKLE_ENTITY_IDS_END_OFFSET: u32 = 0x1d4;
const TANK_SPARKLE_SPAWN_TIMER_OFFSET: u32 = 0x1e4;

/// The water-purity settings globals (`DAT_006390b0` murky / `DAT_006390a4` clean thresholds,
/// `DAT_0063af04` purity-timer reload, `DAT_006390b8` sparkle-spawn numerator) the forced-expiry phases
/// below pin expectations against, plus the rise-target delta (`DAT_006390ac`). Re-declared per the
/// repo's no-shared-consts precedent (see this file's own `GAME_RNG_RVA`); resting values are context
/// only - all are read live.
const ZTTANKEXHIBIT_MURKY_THRESHOLD_RVA: u32 = 0x006390b0 - 0x400000;
const ZTTANKEXHIBIT_CLEAN_THRESHOLD_RVA: u32 = 0x006390a4 - 0x400000;
const ZTTANKEXHIBIT_PURITY_TIMER_RELOAD_RVA: u32 = 0x0063af04 - 0x400000;
const ZTTANKEXHIBIT_SPARKLE_NUMERATOR_RVA: u32 = 0x006390b8 - 0x400000;
const ZTTANKEXHIBIT_RISE_TARGET_DELTA_RVA: u32 = 0x006390ac - 0x400000;

impl TankUpdateScalars {
    fn read(tank_ptr: u32) -> Self {
        Self {
            water_level: get_from_memory(tank_ptr + TANK_WATER_LEVEL_OFFSET),
            current_water_type: get_from_memory(tank_ptr + TANK_CURRENT_WATER_TYPE_OFFSET),
            pending_water_type: get_from_memory(tank_ptr + TANK_PENDING_WATER_TYPE_OFFSET),
            is_filled: get_from_memory(tank_ptr + TANK_IS_FILLED_OFFSET),
            water_purity: get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET),
            water_purity_timer: get_from_memory(tank_ptr + TANK_WATER_PURITY_TIMER_OFFSET),
            sparkle_spawn_timer: get_from_memory(tank_ptr + TANK_SPARKLE_SPAWN_TIMER_OFFSET),
        }
    }

    /// Writes all seven scalars back - both the restore between poles and the per-phase pinning base.
    fn save_to(&self, tank_ptr: u32) {
        save_to_memory(tank_ptr + TANK_WATER_LEVEL_OFFSET, self.water_level);
        save_to_memory(tank_ptr + TANK_CURRENT_WATER_TYPE_OFFSET, self.current_water_type);
        save_to_memory(tank_ptr + TANK_PENDING_WATER_TYPE_OFFSET, self.pending_water_type);
        save_to_memory(tank_ptr + TANK_IS_FILLED_OFFSET, self.is_filled != 0);
        save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, self.water_purity);
        save_to_memory(tank_ptr + TANK_WATER_PURITY_TIMER_OFFSET, self.water_purity_timer);
        save_to_memory(tank_ptr + TANK_SPARKLE_SPAWN_TIMER_OFFSET, self.sparkle_spawn_timer);
    }
}

/// Runs one pole of the tank-update comparison: restores `snapshot`, applies `pin` on top of it, runs
/// `ticks` x `update(16)` (real vanilla through the release-safe `update_real` trampoline, or the
/// reimplementation directly), capturing all seven scalars after each tick, then restores `snapshot`
/// again so the next pole (or phase) starts from an identical state.
fn run_tank_update_pole(
    tank_ptr: u32,
    snapshot: &TankUpdateScalars,
    ticks: usize,
    real: bool,
    pin: impl Fn(u32),
) -> Vec<TankUpdateScalars> {
    snapshot.save_to(tank_ptr);
    pin(tank_ptr);
    let mut results = Vec::new();
    for _ in 0..ticks {
        if real {
            detours::update_real(tank_ptr as *const u32, 16);
        } else {
            unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) }.update(16);
        }
        results.push(TankUpdateScalars::read(tank_ptr));
    }
    snapshot.save_to(tank_ptr);
    results
}

fn compare_tank_update_poles(phase: &str, real: &[TankUpdateScalars], reimpl: &[TankUpdateScalars]) -> Vec<String> {
    real.iter()
        .zip(reimpl.iter())
        .enumerate()
        .filter(|(_, (r, p))| r != p)
        .map(|(tick, (r, p))| format!("{phase} tick {}: real={r:?}, reimpl={p:?}", tick + 1))
        .collect()
}

/// Compares `ZTTankExhibit::update`'s port against real vanilla over every tank in the live, loaded
/// zoo, in three phases per tank (the real pole always runs after the reimplementation pole, from a
/// re-pinned identical scalar state; only the seven scalars are compared/restored - see
/// [`TankUpdateScalars`] for why the pointer fields are never saved):
///
/// - **Phase A (natural state)**: only tanks whose state stays static-safe across a 3-tick burst (no
///   sparkle present, `sparkle_spawn_timer > 48`, the purity countdown inert for 3 x 16, and - for a
///   draining tank with a different pending water type - too much water left to reach the drain-to-zero
///   transition within 3 ticks, keeping the heavy `updateAdjustmentCosts`+`fill` path out of the
///   battery entirely). Skipped tanks are logged in the count summary, not failed.
/// - **Phase B (forced purity expiry)**: pins the tank filled at its rise target with mid purity
///   (`murky + (clean - murky) / 2`, thresholds read live - a value real `setWaterPurity` crosses no
///   ripple/OA/UI threshold at) and `water_purity_timer = 1`, so one tick forces the real
///   `setWaterPurity` call-through. Asserts purity drops by exactly one and the timer reloads from
///   `DAT_0063af04` on both poles. Skipped when the live clean-murky gap is degenerate (< 4).
/// - **Phase C (forced sparkle expiry)**: same pinning with the purity timer parked at its reload and
///   `sparkle_spawn_timer = 1`, so one tick forces the real `addRandomSparkle` call-through - which
///   early-returns at its own `water_purity >= clean` gate at mid purity, so no entity is ever spawned
///   under test - and the timer reloads from `DAT_006390b8 / owned_tile_count`. Skipped when the
///   thresholds are degenerate or the tank owns no tiles (vanilla's `DIV` would fault).
///
/// Sparkle-vector lengths are asserted only for tanks whose vectors were empty at snapshot (they cannot
/// shrink, and a spawn would mean the purity gate failed to hold); non-empty vectors are left alone.
/// The drain-completion transition (`updateAdjustmentCosts` + `fill`, heavy real world mutation) is not
/// forced anywhere - same "no synthetic-safe input" disposition as `RESIZE`.
pub(crate) fn run_tankexhibit_update_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_UPDATE_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let base = get_module_base("zoo.exe") as u32;
    let murky: i32 = get_from_memory(base + ZTTANKEXHIBIT_MURKY_THRESHOLD_RVA);
    let clean: i32 = get_from_memory(base + ZTTANKEXHIBIT_CLEAN_THRESHOLD_RVA);
    let purity_reload: i32 = get_from_memory(base + ZTTANKEXHIBIT_PURITY_TIMER_RELOAD_RVA);
    let sparkle_numerator: u32 = get_from_memory(base + ZTTANKEXHIBIT_SPARKLE_NUMERATOR_RVA);
    let rise_delta: i32 = get_from_memory(base + ZTTANKEXHIBIT_RISE_TARGET_DELTA_RVA);
    let mid_purity = murky + (clean - murky) / 2;
    let thresholds_degenerate = clean - murky < 4;

    let mut tank_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        if unsafe { ref_from_memory::<ZTHabitat>(ptr) }.is_tank() {
            tank_ptrs.push(ptr);
        }
    }
    if tank_ptrs.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }

    let mut phase_a_runs = 0usize;
    let mut phase_b_runs = 0usize;
    let mut phase_c_runs = 0usize;

    for &tank_ptr in &tank_ptrs {
        let snapshot = TankUpdateScalars::read(tank_ptr);
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let owned_tile_count = walk_tile_list(*tank.owned_tiles_ptr()).count() as u32;
        let sparkles_present = get_from_memory::<u32>(tank_ptr + TANK_SPARKLE_ENTITIES_BEGIN_OFFSET)
            != get_from_memory::<u32>(tank_ptr + TANK_SPARKLE_ENTITIES_END_OFFSET);
        let rise_target = (*tank.tank_height()) as i32 + rise_delta;
        let forced_phases_usable = !thresholds_degenerate && rise_target > 0;

        // Phase A: natural state, 3 ticks per pole.
        let purity_would_fire =
            snapshot.water_level as i32 > 0 && snapshot.water_purity > 0 && snapshot.water_purity_timer <= 48;
        let transition_reachable = snapshot.is_filled == 0
            && snapshot.water_level as i32 > 0
            && snapshot.current_water_type != snapshot.pending_water_type
            && snapshot.water_level as i32 <= 3;
        if snapshot.sparkle_spawn_timer > 48 && !sparkles_present && !purity_would_fire && !transition_reachable {
            let reimpl = run_tank_update_pole(tank_ptr, &snapshot, 3, false, |_| {});
            let real = run_tank_update_pole(tank_ptr, &snapshot, 3, true, |_| {});
            for msg in compare_tank_update_poles("phase A", &real, &reimpl) {
                failures.push(format!("tank {tank_ptr:#010x}: {msg}"));
            }
            phase_a_runs += 1;
        }

        // Phases B/C pin the tank filled at its rise target (rise branch inert, purity gate satisfied).
        if forced_phases_usable {
            let pin_filled_at_target = |ptr: u32| {
                save_to_memory(ptr + TANK_IS_FILLED_OFFSET, true);
                save_to_memory(ptr + TANK_WATER_LEVEL_OFFSET, rise_target as u32);
                save_to_memory(ptr + TANK_WATER_PURITY_OFFSET, mid_purity);
            };

            // Phase B: purity countdown forced to expire on the first tick.
            let pin_purity_expiry = |ptr: u32| {
                pin_filled_at_target(ptr);
                save_to_memory(ptr + TANK_WATER_PURITY_TIMER_OFFSET, 1i32);
                save_to_memory(ptr + TANK_SPARKLE_SPAWN_TIMER_OFFSET, 48i32);
            };
            let reimpl = run_tank_update_pole(tank_ptr, &snapshot, 1, false, pin_purity_expiry);
            let real = run_tank_update_pole(tank_ptr, &snapshot, 1, true, pin_purity_expiry);
            for msg in compare_tank_update_poles("phase B", &real, &reimpl) {
                failures.push(format!("tank {tank_ptr:#010x}: {msg}"));
            }
            for (label, results) in [("reimpl", &reimpl), ("real", &real)] {
                if results[0].water_purity != mid_purity - 1 {
                    failures.push(format!(
                        "tank {tank_ptr:#010x}: phase B {label}: expected purity {} after one expiry tick, got {}",
                        mid_purity - 1,
                        results[0].water_purity
                    ));
                }
                if results[0].water_purity_timer != purity_reload {
                    failures.push(format!(
                        "tank {tank_ptr:#010x}: phase B {label}: expected purity timer reload {}, got {}",
                        purity_reload, results[0].water_purity_timer
                    ));
                }
            }
            phase_b_runs += 1;

            // Phase C: sparkle countdown forced to expire on the first tick (mid purity keeps real
            // `addRandomSparkle`'s own gate closed, so no entity is ever spawned under test). Skipped
            // for a tile-less tank - real vanilla's own `DIV` would fault there, so the port's 0
            // reload has no vanilla pole to diff against.
            if owned_tile_count > 0 {
                let expected_sparkle_reload = sparkle_numerator.checked_div(owned_tile_count).unwrap_or(0) as i32;
                let pin_sparkle_expiry = |ptr: u32| {
                    pin_filled_at_target(ptr);
                    save_to_memory(ptr + TANK_WATER_PURITY_TIMER_OFFSET, purity_reload);
                    save_to_memory(ptr + TANK_SPARKLE_SPAWN_TIMER_OFFSET, 1i32);
                };
                let reimpl = run_tank_update_pole(tank_ptr, &snapshot, 1, false, pin_sparkle_expiry);
                let real = run_tank_update_pole(tank_ptr, &snapshot, 1, true, pin_sparkle_expiry);
                for msg in compare_tank_update_poles("phase C", &real, &reimpl) {
                    failures.push(format!("tank {tank_ptr:#010x}: {msg}"));
                }
                for (label, results) in [("reimpl", &reimpl), ("real", &real)] {
                    if results[0].sparkle_spawn_timer != expected_sparkle_reload {
                        failures.push(format!(
                            "tank {tank_ptr:#010x}: phase C {label}: expected sparkle timer reload {expected_sparkle_reload}, got {}",
                            results[0].sparkle_spawn_timer
                        ));
                    }
                    if results[0].water_purity != mid_purity {
                        failures.push(format!(
                            "tank {tank_ptr:#010x}: phase C {label}: purity moved to {} with the countdown parked",
                            results[0].water_purity
                        ));
                    }
                }
                phase_c_runs += 1;
            }
        }

        // A snapshot-empty sparkle vector must still be empty: shrinking is impossible with nothing in
        // it, and any growth means a real spawn fired despite the mid-purity pinning above.
        if !sparkles_present {
            let entities_grew = get_from_memory::<u32>(tank_ptr + TANK_SPARKLE_ENTITIES_BEGIN_OFFSET)
                != get_from_memory::<u32>(tank_ptr + TANK_SPARKLE_ENTITIES_END_OFFSET);
            let ids_grew = get_from_memory::<u32>(tank_ptr + TANK_SPARKLE_ENTITY_IDS_BEGIN_OFFSET)
                != get_from_memory::<u32>(tank_ptr + TANK_SPARKLE_ENTITY_IDS_END_OFFSET);
            if entities_grew || ids_grew {
                failures.push(format!(
                    "tank {tank_ptr:#010x}: sparkle vector grew from empty (entities grew: {entities_grew}, ids grew: {ids_grew}) - a spawn fired under test"
                ));
            }
        }

        // Leave the tank exactly as found (scalar-wise).
        snapshot.save_to(tank_ptr);
    }

    if failures.is_empty() && phase_a_runs == 0 && phase_b_runs == 0 && phase_c_runs == 0 && !tank_ptrs.is_empty() {
        failures.push(format!(
            "every one of the {} tank(s) was skipped in every phase - nothing compared",
            tank_ptrs.len()
        ));
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (tanks: {}, phase A runs: {}, purity-expiry runs: {}, sparkle-expiry runs: {})",
                test_name,
                tank_ptrs.len(),
                phase_a_runs,
                phase_b_runs,
                phase_c_runs
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

/// `ZTHabitat::setDeterioration`'s own 3-way clamp expectation, straight from the `.c`/`.asm`:
/// `0` forces `0`, `1` applies only when the current value isn't already `2` (never lowers a `2`
/// to a `1`), `2` always forces `2`, any other level is a no-op - and the return is always the
/// resulting field value (real vanilla's full-`EAX` `dword` return).
fn expected_deterioration(current: u32, level: u32) -> u32 {
    match level {
        0 => 0,
        1 => {
            if current != 2 {
                1
            } else {
                2
            }
        }
        2 => 2,
        _ => current,
    }
}

/// Compares the reimplemented `ZTHabitat::set_deterioration` against real vanilla (via the
/// release-safe `_real` helper - `.original()` on the now-detoured address would re-enter the
/// detour in release) over every real habitat in the live zoo's `exhibit_array`, for every
/// `(pinned_current, level)` combination in `0..=2` squared plus the no-op arm (`3`/`u32::MAX`).
/// Each cell pins the field to a known current value first (restored to its original value
/// afterward), then asserts both the return value and the live field equal the expectation after
/// each pole - the `current == 2, level == 1` cell is the one that actually exercises the
/// never-lowers rule against real vanilla. A fresh-save zoo pins its own deterioration levels, so
/// this never relies on live values being nonzero.
pub(crate) fn run_habitat_set_deterioration_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_SET_DETERIORATION_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    let deterioration_offset = std::mem::offset_of!(ZTHabitat, deterioration) as u32;

    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let field_addr = ptr + deterioration_offset;
        let original: u32 = get_from_memory(field_addr);

        // (current in 0..=2) x (level in 0..=2), plus the no-op arm at pinned current 2.
        let mut cells: Vec<(u32, u32)> = Vec::new();
        for current in 0..=2u32 {
            for level in 0..=2u32 {
                cells.push((current, level));
            }
        }
        cells.push((2, 3));
        cells.push((2, u32::MAX));

        for (pinned_current, level) in cells {
            let expected = expected_deterioration(pinned_current, level);

            save_to_memory(field_addr, pinned_current);
            let reimpl_returned = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.set_deterioration(level);
            let reimpl_field: u32 = get_from_memory(field_addr);

            save_to_memory(field_addr, pinned_current);
            let real_returned = hooks_zthabitatmgr::set_deterioration_real(ptr as *const u32, level);
            let real_field: u32 = get_from_memory(field_addr);

            if reimpl_returned != expected || reimpl_field != expected {
                failures.push(format!(
                    "habitat {} ({:#010x}): reimpl current {pinned_current} level {level}: expected {expected}, returned {reimpl_returned}, field {reimpl_field}",
                    i, ptr
                ));
            }
            if real_returned != expected || real_field != expected {
                failures.push(format!(
                    "habitat {} ({:#010x}): real current {pinned_current} level {level}: expected {expected}, returned {real_returned}, field {real_field}",
                    i, ptr
                ));
            }
        }

        save_to_memory(field_addr, original);
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

/// Cross-checks [`crate::zthabitatmgr::walk_tile_list`]'s own node count against real vanilla
/// `ZTHabitat::getSize(false)` (reached through [`hooks_zthabitatmgr::get_size_real`], since `GET_SIZE`
/// is detoured) for every real habitat in
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
        let real_size = hooks_zthabitatmgr::get_size_real(ptr as *const u32, false);
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

/// Two-pole comparison of the ported [`ZTHabitat::get_size`] against real vanilla `ZTHabitat::getSize`
/// (reached through [`hooks_zthabitatmgr::get_size_real`], since `GET_SIZE` is detoured) over every real
/// habitat in the live zoo, for both `subhabs` values. Pure read on both poles (no
/// `characteristics_dirty` recalculate, no RNG), so there's no state to restore and no ordering
/// constraint on where this runs.
///
/// Coverage counters ride along in the success line (same pattern as
/// [`run_check_exhibit_morph_live_test`]'s own probe suffix): a save where no habitat has amphibious
/// neighbors makes the `subhabs=true` arm a vacuous `0 == 0` comparison on both poles - the counter
/// makes that visible instead of letting the pass look stronger than it is.
pub(crate) fn run_habitat_get_size_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_SIZE_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut with_neighbors = 0usize;
    let mut total_neighbor_visits = 0i64;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        checked += 1;
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for subhabs in [false, true] {
            let real = hooks_zthabitatmgr::get_size_real(ptr as *const u32, subhabs);
            let reimpl = habitat.get_size(subhabs);
            if real != reimpl {
                failures.push(format!("habitat {} ({:#010x}): getSize(subhabs={}) real={}, reimpl={}", i, ptr, subhabs, real, reimpl));
            }
        }
        if walk_neighbor_tree(*habitat.amphibious_neighbors_head()).next().is_some() {
            with_neighbors += 1;
        }
        total_neighbor_visits += habitat.get_size(true) as i64 - habitat.get_size(false) as i64;
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats checked: {}, with amphibious neighbors: {}, total subhabitat visits: {})",
                test_name, checked, with_neighbors, total_neighbor_visits
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
        if hooks_zthabitatmgr::get_size_real(ptr as *const u32, false) > 0 {
            target = Some((i, ptr));
            break;
        }
    }

    let Some((i, ptr)) = target else {
        write_success_line(failure_log, &format!("{} (skipped: no habitat with owned tiles found)", test_name));
        return false;
    };

    let real_size_before = hooks_zthabitatmgr::get_size_real(ptr as *const u32, false);
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

    let real_size_after = hooks_zthabitatmgr::get_size_real(ptr as *const u32, false);
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
        if hooks_zthabitatmgr::get_size_real(ptr as *const u32, false) > 0 {
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

    let real_size_after = hooks_zthabitatmgr::get_size_real(ptr as *const u32, false);
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

/// Compares real `ZTHabitatMgr::getNumNonShowNonWorldHabitats` against the reimplemented
/// `get_num_non_show_non_world_habitats`, over the live, loaded zoo's own manager singleton. The
/// independent oracle reads raw fields only (vtable == [`ZTHabitat::TANK_VTABLE_PTR`] && the `+0x4`
/// show-info dword nonzero), standing in for the `isTank() && zt_show_info_ptr != 0` test real vanilla
/// inlines at the loop site - so a vtable-identity (`is_tank`) bug cannot agree with itself. Also pins
/// the census identity every `exhibit_array` scan satisfies: non-show count + show tanks == scanned ==
/// `exhibit_array().len()` (the core assertion the Stage 37 global exhibit census builds on).
pub(crate) fn run_zthabitatmgr_get_num_non_show_non_world_habitats_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_GET_NUM_NON_SHOW_NON_WORLD_HABITATS_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mgr = globals().zthabitatmgr();

    let exhibit_len = mgr.exhibit_array().len();
    if exhibit_len == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no habitats loaded)", test_name));
        return false;
    }

    let real = hooks_zthabitatmgr::get_num_non_show_non_world_habitats_real(mgr_ptr);
    let reimpl = mgr.get_num_non_show_non_world_habitats();

    let mut oracle = 0i32;
    let mut show_tanks = 0i32;
    for i in 0..exhibit_len {
        let habitat_ptr = mgr.exhibit_array().get_ptr(i);
        let is_show_tank =
            get_from_memory::<u32>(habitat_ptr) == ZTHabitat::TANK_VTABLE_PTR && get_from_memory::<u32>(habitat_ptr + 4) != 0;
        if is_show_tank {
            show_tanks += 1;
        } else {
            oracle += 1;
        }
    }

    let mut failures: Vec<String> = Vec::new();
    if real != reimpl {
        failures.push(format!("real={}, reimpl={}", real, reimpl));
    }
    if oracle != reimpl {
        failures.push(format!("reimpl={}, raw-field oracle={}", reimpl, oracle));
    }
    if oracle + show_tanks != exhibit_len as i32 {
        failures.push(format!(
            "census identity broken: non-show {} + show tanks {} != scanned {}",
            oracle, show_tanks, exhibit_len
        ));
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!("{} (habitats scanned: {}, show tanks: {}, non-show count: {})", test_name, exhibit_len, show_tanks, oracle),
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
        if hooks_zthabitatmgr::get_size_real(ptr as *const u32, false) > 0 {
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

/// Mutates the manager's own running exhibit-number counter (`+0x68`, offset pinned by the struct's own
/// `offset_of!` assert) and restores it - same rationale as [`run_zthabitatmgr_enter_new_month_roundtrip_live_test`]:
/// the counter's only consumer is `nameHabitat`'s own `getNextNum` read-then-increment, so snapshot/call/
/// restore between ticks leaves the live zoo's state unchanged for the rest of the battery run. Calls both
/// poles - the reimplementation, then real vanilla through the release-safe `_real` helper - and compares
/// each against an independently computed expectation (the `ZTMapView` `+0x378` undoing gate, read exactly
/// the way production reads it), so a pass simultaneously proves the port honors the gate, both poles agree
/// on the same live state, and the raw `+0x378`/`+0x68` offsets are correct.
pub(crate) fn run_zthabitatmgr_decrement_habitat_num_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_DECREMENT_HABITAT_NUM_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as u32;
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();

    // Gate expectation read independently, exactly as production reads it.
    let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() } as u32;
    let gate_set = mapview_ptr != 0 && get_from_memory::<u8>(mapview_ptr + 0x378) != 0;
    let expected_delta: u32 = if gate_set { 0 } else { u32::MAX }; // wrapping -1

    let before: u32 = get_from_memory(mgr_ptr + 0x68);

    habitat_mgr.decrement_habitat_num();
    let after_reimpl: u32 = get_from_memory(mgr_ptr + 0x68);
    save_to_memory(mgr_ptr + 0x68, before);

    hooks_zthabitatmgr::decrement_habitat_num_real(mgr_ptr as *const u32);
    let after_real: u32 = get_from_memory(mgr_ptr + 0x68);
    save_to_memory(mgr_ptr + 0x68, before);

    if after_reimpl.wrapping_sub(before) != expected_delta {
        failures.push(format!(
            "reimpl: gate_set={} expected delta {}, got {}",
            gate_set,
            expected_delta as i32,
            after_reimpl.wrapping_sub(before) as i32
        ));
    }
    if after_real.wrapping_sub(before) != expected_delta {
        failures.push(format!(
            "real: gate_set={} expected delta {}, got {}",
            gate_set,
            expected_delta as i32,
            after_real.wrapping_sub(before) as i32
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

/// Payload `ZTHabitat*` addresses of a neighbor `std::set` head (`amphibious_neighbors_head`/
/// `show_neighbors_head`'s value), in the set's own in-order order. Shared by every neighbor-set
/// real-vs-port comparison below.
fn snapshot_neighbor_set(head: u32) -> Vec<u32> {
    walk_neighbor_tree(head).map(|node| get_from_memory::<u32>(node + 0x10)).collect()
}

/// `(habitat, amphibious set, show set)` for every live habitat.
fn snapshot_all_neighbor_sets() -> Vec<(u32, Vec<u32>, Vec<u32>)> {
    let habitat_mgr = globals().zthabitatmgr();
    (0..habitat_mgr.exhibit_array().len())
        .map(|i| habitat_mgr.exhibit_array().get_ptr(i))
        .filter(|&ptr| ptr != 0)
        .map(|ptr| {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
            (ptr, snapshot_neighbor_set(*habitat.amphibious_neighbors_head()), snapshot_neighbor_set(*habitat.show_neighbors_head()))
        })
        .collect()
}

fn diff_neighbor_snapshots(label: &str, real: &[(u32, Vec<u32>, Vec<u32>)], port: &[(u32, Vec<u32>, Vec<u32>)], failures: &mut Vec<String>) {
    for (r, p) in real.iter().zip(port) {
        if r != p {
            failures.push(format!(
                "{label}: habitat {:#010x}: real amphibious={:x?} show={:x?}, port amphibious={:x?} show={:x?}",
                r.0, r.1, r.2, p.1, p.2
            ));
        }
    }
}

/// Real-vs-port comparison of `ZTHabitatMgr::check_amphibious_neighbor` over every habitat's boundary
/// tile-pairs. The real call's side effects (`addAmphibiousNeighbor` set inserts, `setIsCombinedConnector`
/// clear-then-set) are idempotent, so running the real function first and the port second from the
/// resulting state must yield the same return value and the same neighbor sets. In release `.original()`
/// reaches our own detour (see `FunctionDef::original`), making this a self-comparison there.
pub(crate) fn run_zthabitatmgr_check_amphibious_neighbor_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CHECK_AMPHIBIOUS_NEIGHBOR_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for (tile_a, tile_b) in habitat.boundary_tile_pairs() {
            let real = low_byte_bool(unsafe { zthabitatmgr::CHECK_AMPHIBIOUS_NEIGHBOR.original()(mgr_ptr, ptr as *const u32, tile_a as *const u32, tile_b as *const u32) });
            let real_sets = snapshot_all_neighbor_sets();
            let port = habitat_mgr.check_amphibious_neighbor(ptr, tile_a, tile_b);
            let port_sets = snapshot_all_neighbor_sets();
            if real != port {
                failures.push(format!("habitat {:#010x} tiles {:#010x}/{:#010x}: real={}, port={}", ptr, tile_a, tile_b, real, port));
            }
            diff_neighbor_snapshots("check_amphibious_neighbor", &real_sets, &port_sets, &mut failures);
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// Real-vs-port comparison of `ZTHabitatMgr::update_amphibious_neighbors` (clear + re-derive, so the
/// result is independent of the starting set contents): every habitat's amphibious/show sets after real
/// must equal the sets after the port.
pub(crate) fn run_zthabitatmgr_update_amphibious_neighbors_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_UPDATE_AMPHIBIOUS_NEIGHBORS_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        sync_real_boundary_pairs(ptr);
        unsafe { zthabitatmgr::UPDATE_AMPHIBIOUS_NEIGHBORS_1.original()(mgr_ptr, ptr as *const u32) };
        let real_sets = snapshot_all_neighbor_sets();
        habitat_mgr.update_amphibious_neighbors(ptr);
        let port_sets = snapshot_all_neighbor_sets();
        diff_neighbor_snapshots("update_amphibious_neighbors", &real_sets, &port_sets, &mut failures);
    }
    finish_test(test_name, failures, failure_log)
}

/// Same shape as [`run_zthabitatmgr_check_amphibious_neighbor_matches_real_live_test`]: `addShowNeighbor`
/// (set inserts) and the portal registration (`addShowPortal` only when no portal exists yet) are
/// idempotent, so real-then-port from the same starting state must agree on return value and sets.
pub(crate) fn run_zthabitatmgr_check_show_neighbor_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CHECK_SHOW_NEIGHBOR_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for (tile_a, tile_b) in habitat.boundary_tile_pairs() {
            let real = low_byte_bool(unsafe { zthabitatmgr::CHECK_SHOW_NEIGHBOR.original()(mgr_ptr, ptr as *const u32, tile_a as *const u32, tile_b as *const u32) });
            let real_sets = snapshot_all_neighbor_sets();
            let port = habitat_mgr.check_show_neighbor(ptr, tile_a, tile_b);
            let port_sets = snapshot_all_neighbor_sets();
            if real != port {
                failures.push(format!("habitat {:#010x} tiles {:#010x}/{:#010x}: real={}, port={}", ptr, tile_a, tile_b, real, port));
            }
            diff_neighbor_snapshots("check_show_neighbor", &real_sets, &port_sets, &mut failures);
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// Real-vs-port comparison of `ZTHabitatMgr::update_show_neighbors` (the recursive worker), over every
/// habitat, comparing every habitat's amphibious/show sets after real vs after the port.
pub(crate) fn run_zthabitatmgr_update_show_neighbors_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_UPDATE_SHOW_NEIGHBORS_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        sync_real_boundary_pairs(ptr);
        unsafe { zthabitatmgr::UPDATE_SHOW_NEIGHBORS_1.original()(mgr_ptr, ptr as *const u32) };
        let real_sets = snapshot_all_neighbor_sets();
        habitat_mgr.update_show_neighbors(ptr);
        let port_sets = snapshot_all_neighbor_sets();
        diff_neighbor_snapshots("update_show_neighbors", &real_sets, &port_sets, &mut failures);
    }
    finish_test(test_name, failures, failure_log)
}

/// Real-vs-port comparison of `ZTHabitatMgr::do_show_check` (`remove_illegal = false`, which avoids
/// `removeIllegalEntities`) over every live habitat, via the release-safe
/// `hooks_zthabitatmgr::do_show_check_real`. The call sets show-exhibit state to a value derived purely from
/// the habitat's boundary fences, so it is idempotent: running the real function first and the port second
/// from the resulting state must return the same verdict. Replaced the earlier no-crash smoke test.
pub(crate) fn run_zthabitatmgr_do_show_check_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_DO_SHOW_CHECK_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    let (mut compared, mut passed) = (0, 0);
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        sync_real_boundary_pairs(ptr);
        let real = hooks_zthabitatmgr::do_show_check_real(mgr_ptr, ptr as *const i32, 0);
        let port = habitat_mgr.do_show_check(ptr, false);
        compared += 1;
        if real {
            passed += 1;
        }
        if real != port {
            failures.push(format!("habitat {} ({:#010x}): real={}, port={}", i, ptr, real, port));
        }
    }
    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (habitats: {}, show checks passing: {})", test_name, compared, passed));
        false
    } else {
        finish_test(test_name, failures, failure_log)
    }
}


/// Live `ZTBuilding`s that are safe to hand to real vanilla's `canSeeHabitatFromBuilding` family: real
/// reads each resolved tile's fields unguarded, so a building whose 3/4/5-steps-out line along its
/// facing (`+0x12c`) leaves the map would null-page-read in real vanilla. Requires a non-null tile and
/// a cardinal facing, and all three probe positions inside the map.
fn find_live_buildings_with_in_map_probes() -> Vec<u32> {
    let world = globals().ztworldmgr();
    let mut buildings = Vec::new();
    for entity_ptr in world.entity_array() {
        if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, crate::zthabitatmgr::RVA_BUILDING_TYPE_CHECK_ARG) } {
            continue;
        }
        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(entity_ptr as *const u32) } as u32;
        if tile_ptr == 0 {
            continue;
        }
        let facing: u32 = get_from_memory(entity_ptr + 0x12c);
        let (dx, dy) = match facing {
            0 => (0i32, -1i32),
            2 => (1, 0),
            4 => (0, 1),
            6 => (-1, 0),
            _ => continue,
        };
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let in_map = (3..=5).all(|steps| {
            let (x, y) = (tile.pos.x + dx * steps, tile.pos.y + dy * steps);
            x >= 0 && y >= 0 && (x as u32) < world.map_x_size && (y as u32) < world.map_y_size
        });
        if in_map {
            buildings.push(entity_ptr);
        }
    }
    buildings
}

/// Real-vs-port comparison of `habitatSeenFromBuilding`, `canSeeShowFromBuilding` and the shared
/// `canSeeHabitatFromBuilding` (every building x every habitat) over the live save's own buildings - all
/// pure queries. Reports how many comparisons hit a non-null/true result so a save where nothing faces a
/// habitat is visible in the log rather than passing vacuously.
pub(crate) fn run_zthabitatmgr_building_visibility_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_BUILDING_VISIBILITY_MATCHES_REAL_LIVE";
    let buildings = find_live_buildings_with_in_map_probes();
    if buildings.is_empty() {
        write_success_line(failure_log, &format!("{} (skipped: no live building with in-map probe line)", test_name));
        return false;
    }

    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let habitats: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    let mut failures: Vec<String> = Vec::new();
    let (mut seen_hits, mut show_hits, mut los_hits) = (0, 0, 0);

    for &building in &buildings {
        let real_seen = unsafe { zthabitatmgr::HABITAT_SEEN_FROM_BUILDING.original()(mgr_ptr, building as i32) } as u32;
        let port_seen = habitat_mgr.habitat_seen_from_building(building);
        if real_seen != 0 {
            seen_hits += 1;
        }
        if real_seen != port_seen {
            failures.push(format!("habitat_seen_from_building {:#010x}: real={:#010x}, port={:#010x}", building, real_seen, port_seen));
        }

        let real_show = unsafe { zthabitatmgr::CAN_SEE_SHOW_FROM_BUILDING.original()(mgr_ptr, building as *const u32) } & 0xffff;
        let port_show = habitat_mgr.can_see_show_from_building(building);
        if real_show != 0 {
            show_hits += 1;
        }
        if real_show != port_show {
            failures.push(format!("can_see_show_from_building {:#010x}: real={:#x}, port={:#x}", building, real_show, port_show));
        }

        for &habitat in &habitats {
            let real = low_byte_bool(unsafe { zthabitatmgr::CAN_SEE_HABITAT_FROM_BUILDING.original()(habitat, building as i32) });
            let port = low_byte_bool(unsafe { ZTHabitatMgr::can_see_habitat_from_building(habitat, building) });
            if real {
                los_hits += 1;
            }
            if real != port {
                failures.push(format!("can_see_habitat_from_building habitat {:#010x} building {:#010x}: real={}, port={}", habitat, building, real, port));
            }
        }
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (buildings: {}, habitats: {}, seen-habitat hits: {}, show-id hits: {}, line-of-sight hits: {})",
                test_name,
                buildings.len(),
                habitats.len(),
                seen_hits,
                show_hits,
                los_hits
            ),
        );
        false
    } else {
        finish_test(test_name, failures, failure_log)
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
        |ptr| unsafe { zthabitat::GET_NUM_ANIMALS_0.original()(ptr, false) },
        |habitat| habitat.get_num_animals(false),
    );
    let with_neighbors = compare_over_live_habitats(
        failure_log,
        "ZTHABITAT_GET_NUM_ANIMALS_WITH_NEIGHBORS_LIVE",
        |ptr| unsafe { zthabitat::GET_NUM_ANIMALS_0.original()(ptr, true) },
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

/// Multi-reimplementation integration test (stage 32 of
/// `openzt/plans/zthabitat-additional-functions-plan.md`): drives the population/demographics
/// reimplementations together over the live zoo's own habitats, so a disagreement between two getters
/// of one side surfaces as a failed cross-getter invariant rather than only a real-vs-reimpl diff.
/// Real vanilla is called before the reimplementation everywhere (the established
/// `characteristics_dirty` ordering). Per habitat, over its own direct occupants:
///
/// 1. Adult partition: `getNumAdultAnimals(false)` equals the sum of per-species
///    `getNumAdultAnimals(species, false)` over the habitat's own distinct species ids plus one
///    guaranteed-absent probe (`i32::MAX`), per side, and both totals agree. The
///    `include_neighbors = true` arm is deliberately not partitioned - the neighbor closure's species
///    are not enumerable from this habitat's own occupants, so the sum invariant does not hold there;
///    with-neighbors real-vs-reimpl equality is already pinned by
///    `ZTHABITAT_GET_NUM_ADULT_ANIMALS_WITH_NEIGHBORS_LIVE`.
/// 2. Species membership: `getSpeciesAnimals(species)`'s out-param length equals the count of that
///    species within `getAllAnimals`, per species per side.
/// 3. Gender partition: for each species, `getAdultGenderSpeciesAnimals("Female")` +
///    `getAdultGenderSpeciesAnimals("Male")` lengths equal that species' adult count from assertion 1,
///    per side. The spec's `"m"`/`"f"` request strings are not usable here - the filter compares the
///    requested string against each animal's own gender text (`"Female"`/`"Male"`), so `"m"`/`"f"`
///    match nothing and the invariant would be vacuously 0 + 0 == 0 for every species; an assumption
///    check first asserts every adult's own gender text actually corresponds to its
///    `entity_type+0xa4` tag (`'f'` ↔ "Female", `'m'` ↔ "Male") so a tag/text drift fails loudly
///    instead of silently breaking the partition.
/// 4. Bounded counters: `getNumAngryAnimals(false)` and `getNumSickAnimals(false)` never exceed the
///    habitat's direct-occupant count, per side (the `true` arm legitimately exceeds one habitat's own
///    count by summing neighbors, so it is not bounded this way).
/// 5. Baby-bonus/average consistency: `getAvgAnimalHappiness` equals the recomputed population mean
///    (`sum(animal+0x2a8) / num_animals`, plain truncating division - the census
///    `ZTHabitat_recalculateCharacteristics.c` sums each animal's `happiness` field into per-species
///    suitability records and divides the total by `num_animals`; 0 when `num_animals` is 0) on both
///    sides, then every `ZTAnimalType` present anywhere in the zoo is swept through
///    [`assert_baby_born_bonus_pass`] on both sides (the habitat × type cross-product re-exercises the
///    empty-scratch path for species absent from this habitat), and the average is re-read and must
///    still equal the same oracle. The spec's "getAvgAnimalHappiness shifts consistently with the
///    population average" cannot mean a shift inside a synchronous pass: `addBabyBornBonus` writes the
///    pending accumulator `animal+0x2ac`, which `ZTAnimal::updateStatusVariables` drains into the
///    happiness value at `+0x2a8` one tick later, while `getAvgAnimalHappiness` returns the cached
///    census average of `animal+0x2a8` - so the faithful equivalent is that the average stays equal to
///    the recomputed population mean across both sides' bonus sweeps, with the exact-increment
///    contract itself carried by [`assert_baby_born_bonus_pass`].
///
/// Also asserts `num_animals` against the `getAllAnimals` enumeration every invariant above is built
/// on. Everything is read-only or exactly restored ([`assert_baby_born_bonus_pass`] snapshots and
/// restores every `+0x2ac` accumulator), so no game state survives the test. Non-vacuousness: the
/// save must contain at least one animal zoo-wide, or every invariant above holds vacuously.
pub(crate) fn run_habitat_population_metrics_multi_reimpl_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_POPULATION_METRICS_MULTI_REIMPL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }
    let mut failures: Vec<String> = Vec::new();

    // Union of the zoo's real `ZTAnimalType` pointers, in habitat order (same shape as
    // [`run_habitat_add_baby_born_bonus_live_test`]).
    let mut type_ptrs: Vec<u32> = Vec::new();
    for &ptr in &habitat_ptrs {
        for animal_ptr in unsafe { ref_from_memory::<ZTHabitat>(ptr) }.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            if !type_ptrs.contains(&animal_type_ptr) {
                type_ptrs.push(animal_type_ptr);
            }
        }
    }

    let mut total_animals = 0usize;
    let mut species_queries = 0usize;
    let mut adult_gender_queries = 0usize;
    let mut bonus_passes = 0usize;

    for (i, &ptr) in habitat_ptrs.iter().enumerate() {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let animal_ptrs: Vec<u32> = habitat.get_all_animals(false).collect();
        total_animals += animal_ptrs.len();
        let num_animals = habitat.num_animals;
        if num_animals != animal_ptrs.len() as i32 {
            failures.push(format!(
                "habitat {} ({:#010x}): num_animals {} != getAllAnimals length {}",
                i,
                ptr,
                num_animals,
                animal_ptrs.len()
            ));
        }

        let mut species_of: Vec<i32> = Vec::new();
        let mut species_ids: Vec<i32> = Vec::new();
        for &animal_ptr in &animal_ptrs {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let species_id: i32 = get_from_memory(animal_type_ptr + 0x1ec);
            species_of.push(species_id);
            if !species_ids.contains(&species_id) {
                species_ids.push(species_id);
            }
        }
        species_ids.push(i32::MAX);

        // Assertion 1 - adult partition (false arm only, see the doc comment).
        let adult_real = unsafe { zthabitat::GET_NUM_ADULT_ANIMALS_0.original()(ptr as *const u32, false) };
        let adult_reimpl = habitat.get_num_adult_animals(false);
        let mut adult_sum_real = 0i32;
        let mut adult_sum_reimpl = 0i32;
        let mut per_species_adults: Vec<(i32, i32, i32)> = Vec::new();
        for &species_id in &species_ids {
            let real = unsafe { zthabitat::GET_NUM_ADULT_ANIMALS_1.original()(ptr as *const u32, species_id, false) };
            let reimpl = habitat.get_num_adult_animals_by_species(species_id, false);
            adult_sum_real += real;
            adult_sum_reimpl += reimpl;
            per_species_adults.push((species_id, real, reimpl));
        }
        if adult_sum_real != adult_real {
            failures.push(format!(
                "habitat {} ({:#010x}) real: sum of per-species adult counts {} != getNumAdultAnimals(false) {}",
                i, ptr, adult_sum_real, adult_real
            ));
        }
        if adult_sum_reimpl != adult_reimpl {
            failures.push(format!(
                "habitat {} ({:#010x}) reimpl: sum of per-species adult counts {} != get_num_adult_animals(false) {}",
                i, ptr, adult_sum_reimpl, adult_reimpl
            ));
        }
        if adult_real != adult_reimpl {
            failures.push(format!(
                "habitat {} ({:#010x}): real getNumAdultAnimals(false) {}, reimpl {}",
                i, ptr, adult_real, adult_reimpl
            ));
        }

        // Assertion 2 - species membership lengths.
        for &species_id in &species_ids {
            let expected_len = species_of.iter().filter(|&&s| s == species_id).count() as i32;

            let mut real_vector = [0u32; 3];
            unsafe { zthabitat::GET_SPECIES_ANIMALS.original()(ptr as *const u32, species_id, real_vector.as_mut_ptr() as *const i32) };
            let real_len = ((real_vector[1] - real_vector[0]) >> 2) as i32;
            free_event_vector_buffer(real_vector[0], real_vector[2] - real_vector[0]);

            let mut reimpl_vector = [0u32; 3];
            habitat.get_species_animals(species_id, reimpl_vector.as_mut_ptr() as u32);
            let reimpl_len = ((reimpl_vector[1] - reimpl_vector[0]) >> 2) as i32;
            free_event_vector_buffer(reimpl_vector[0], reimpl_vector[2] - reimpl_vector[0]);

            species_queries += 2;
            if real_len != expected_len {
                failures.push(format!(
                    "habitat {} ({:#010x}) real: getSpeciesAnimals({}) length {} != getAllAnimals count {}",
                    i, ptr, species_id, real_len, expected_len
                ));
            }
            if reimpl_len != expected_len {
                failures.push(format!(
                    "habitat {} ({:#010x}) reimpl: get_species_animals({}) length {} != get_all_animals count {}",
                    i, ptr, species_id, reimpl_len, expected_len
                ));
            }
        }

        // Assertion 3, assumption leg: an adult's own gender text must correspond to its type's
        // gender tag, or the partition below could only agree with a broken premise.
        for &animal_ptr in &animal_ptrs {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let gender_tag: u8 = get_from_memory(get_from_memory::<u32>(animal_type_ptr + 0xa4));
            if gender_tag == b'm' || gender_tag == b'f' {
                let text_start: u32 = get_from_memory(animal_ptr + 0x26c);
                let text_end: u32 = get_from_memory(animal_ptr + 0x270);
                let own_text: Vec<u8> = (text_start..text_end).map(get_from_memory::<u8>).collect();
                let expected_text: &[u8] = if gender_tag == b'f' { b"Female" } else { b"Male" };
                if own_text != expected_text {
                    failures.push(format!(
                        "habitat {} ({:#010x}): animal {:#010x} adult gender tag {} carries gender text {:?}, expected {:?} - the gender partition invariant cannot hold",
                        i,
                        ptr,
                        animal_ptr,
                        gender_tag as char,
                        String::from_utf8_lossy(&own_text),
                        String::from_utf8_lossy(expected_text)
                    ));
                }
            }
        }

        // Assertion 3, partition leg: "Female" + "Male" lengths account for every adult of the
        // species, per side.
        for &(species_id, adult_real, adult_reimpl) in &per_species_adults {
            let mut gender_lens = [(0i32, 0i32); 2];
            for (slot, gender_text) in [&b"Female"[..], &b"Male"[..]].into_iter().enumerate() {
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
                let real_len = ((real_vector[1] - real_vector[0]) >> 2) as i32;
                free_event_vector_buffer(real_vector[0], real_vector[2] - real_vector[0]);

                let mut reimpl_vector = [0u32; 3];
                habitat.get_adult_gender_species_animals(gender_header.as_ptr() as u32, species_id, reimpl_vector.as_mut_ptr() as u32);
                let reimpl_len = ((reimpl_vector[1] - reimpl_vector[0]) >> 2) as i32;
                free_event_vector_buffer(reimpl_vector[0], reimpl_vector[2] - reimpl_vector[0]);

                gender_lens[slot] = (real_len, reimpl_len);
                adult_gender_queries += 2;
            }
            let (female_real, female_reimpl) = gender_lens[0];
            let (male_real, male_reimpl) = gender_lens[1];
            if female_real + male_real != adult_real {
                failures.push(format!(
                    "habitat {} ({:#010x}) real: getAdultGenderSpeciesAnimals({}, \"Female\") {} + (\"Male\") {} != adult count {}",
                    i, ptr, species_id, female_real, male_real, adult_real
                ));
            }
            if female_reimpl + male_reimpl != adult_reimpl {
                failures.push(format!(
                    "habitat {} ({:#010x}) reimpl: get_adult_gender_species_animals({}, \"Female\") {} + (\"Male\") {} != adult count {}",
                    i, ptr, species_id, female_reimpl, male_reimpl, adult_reimpl
                ));
            }
        }

        // Assertion 4 - the cached angry/sick tallies are per-habitat counts, so neither may exceed
        // the direct-occupant population they are tallied from.
        let bound = animal_ptrs.len() as i32;
        for (side, angry, sick) in [
            (
                "real",
                unsafe { zthabitat::GET_NUM_ANGRY_ANIMALS.original()(ptr as *const u32, false) },
                unsafe { zthabitat::GET_NUM_SICK_ANIMALS.original()(ptr as *const u32, false) },
            ),
            ("reimpl", habitat.get_num_angry_animals(false), habitat.get_num_sick_animals(false)),
        ] {
            if angry > bound {
                failures.push(format!(
                    "habitat {} ({:#010x}) {}: getNumAngryAnimals(false) {} exceeds the direct-occupant count {}",
                    i, ptr, side, angry, bound
                ));
            }
            if sick > bound {
                failures.push(format!(
                    "habitat {} ({:#010x}) {}: getNumSickAnimals(false) {} exceeds the direct-occupant count {}",
                    i, ptr, side, sick, bound
                ));
            }
        }

        // Assertion 5 - average consistency plus the bonus sweep. The census average's own formula
        // (sum of `animal+0x18` over `num_animals`, 0 when empty) is the oracle both sides must match
        // before and after the sweep; nothing the sweep writes feeds it (see the doc comment).
        let oracle = if num_animals == 0 {
            0
        } else {
            animal_ptrs.iter().map(|&a| get_from_memory::<i32>(a + 0x2a8)).fold(0i32, |acc, v| acc.wrapping_add(v)) / num_animals
        };
        for (side, avg) in [
            ("real", unsafe { zthabitat::GET_AVG_ANIMAL_HAPPINESS.original()(ptr as *const u32) }),
            ("reimpl", habitat.get_avg_animal_happiness()),
        ] {
            if avg != oracle {
                failures.push(format!(
                    "habitat {} ({:#010x}) {}: getAvgAnimalHappiness {} != recomputed population mean {}",
                    i, ptr, side, avg, oracle
                ));
            }
        }

        for &type_ptr in &type_ptrs {
            bonus_passes += 2;
            assert_baby_born_bonus_pass(&mut failures, "real", i, ptr, &animal_ptrs, type_ptr, || unsafe {
                zthabitat::ADD_BABY_BORN_BONUS.original()(ptr as *const u32, type_ptr as *const u32)
            });
            assert_baby_born_bonus_pass(&mut failures, "reimpl", i, ptr, &animal_ptrs, type_ptr, || {
                unsafe { ref_from_memory::<ZTHabitat>(ptr) }.add_baby_born_bonus(type_ptr)
            });
        }

        for (side, avg) in [
            ("real", unsafe { zthabitat::GET_AVG_ANIMAL_HAPPINESS.original()(ptr as *const u32) }),
            ("reimpl", habitat.get_avg_animal_happiness()),
        ] {
            if avg != oracle {
                failures.push(format!(
                    "habitat {} ({:#010x}) {}: getAvgAnimalHappiness {} after the bonus sweep, expected the same recomputed population mean {}",
                    i, ptr, side, avg, oracle
                ));
            }
        }
    }

    if total_animals == 0 {
        let msg = "all invariants were vacuous: no animals found across any live habitat".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, animals: {}, species queries: {}, adult-gender queries: {}, bonus passes: {})",
                test_name, habitat_ptrs.len(), total_animals, species_queries, adult_gender_queries, bonus_passes
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

/// Rebuilds `getAdjacentClearTile`'s expected candidate set for one base tile - the decompile's own
/// dx-outer/dy-inner 8-neighborhood scan: bounds against the live map, owner equality with the base
/// tile's own owner ([`ZTHabitatMgr::get_habitat_ptr`], the same raw "0 == 0 ownerless tiles match"
/// comparison both sides perform), and the real vtable `+0x164` path-cost dispatch
/// ([`call_bfunit_tile_cost_vtable_slot`]) by exact equality against the shared [`MAX_PATH_COST_RVA`]
/// sentinel. Extracted for [`run_habitat_terrain_passability_multi_reimpl_live_test`]; the
/// per-function tests keep their own inline copies of the same scan.
fn adjacent_clear_candidates(habitat_mgr: &ZTHabitatMgr, world: &crate::ztworldmgr::ZTWorldMgr, unit: u32, base_tile: u32, max_cost: i32) -> Vec<u32> {
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
    candidates
}

/// Multi-reimplementation integration test (stage 33 of
/// `openzt/plans/zthabitat-additional-functions-plan.md`): drives the terrain-passability /
/// clear-tile navigation reimplementations together over the live zoo's own habitats, asserting
/// cross-getter contracts per side rather than re-asserting the per-draw LCG slot fidelity each
/// per-function live test already pins (stage 11's "may only need the remaining cross-function
/// assertions" scoping). Real vanilla is called before the reimplementation everywhere except the
/// seed-juggled legs, which must restore the shared seed between the sides so both draw the same
/// internal animal slot (the established `characteristics_dirty` ordering still holds: one
/// unasserted real `getRandomAnimal` settles any pending recalculate per habitat first). Per
/// habitat, over its own owned-tile list:
///
/// 1. Clear-tile pool: the null-animal `(false, false)` `addClearTiles` pool agrees
///    element-for-element between the sides, and every pooled tile is an owned tile. In release,
///    `ADD_CLEAR_TILES.original()` re-enters the port, so this leg is port-vs-port there - the
///    real-vs-reimpl diff is `ZTHABITAT_ADD_CLEAR_TILES_MATCHES_REAL_LIVE`'s coverage, same note as
///    [`vanilla_clear_tile_pool`]'s own doc.
/// 2. Random-clear-tile membership: 100 `getRandomClearTile` overload-0 draws per side, in the real
///    callers' own `(false, false)` shape, must each be null (the internally drawn animal's cost
///    gate can legitimately empty the pool) or a member of that null-animal pool - the animal gate
///    only removes tiles. Membership-level only: the exact two-step LCG composition is
///    `ZTHABITAT_GET_RANDOM_CLEAR_TILE_DEFAULT_LIVE`'s coverage.
/// 3. Adjacency: for every distinct drawn tile, both sides' `getAdjacentClearTile` returns the base
///    tile itself when the rebuilt 8-neighborhood candidate set ([`adjacent_clear_candidates`]) is
///    empty (the RNG-free pass-through), else a member of it. The spec's "within 1 tile Manhattan
///    distance" is wrong - the decompile scans the 8-neighborhood, so diagonal picks are Manhattan
///    2; the faithful contract is Chebyshev-1 candidacy, which the candidate rebuild encodes.
/// 4. Nearest/near: `getNearestClearTile` is asserted exactly - seed-controlled prediction of the
///    internal `getRandomAnimal` slot against the [`nearest_clear_tile_oracle`] (the mathematically
///    nearest tile matching all passability constraints; the oracle is the per-function test's own,
///    reused, not re-derived). `getNearClearTile` asserts membership in the seed-independent
///    [`near_clear_tile_candidates`] set per side, and restored-seed cross-agreement of (tile,
///    seed) when the set is empty (the fallback chain `getNearestClearTile` ->
///    `getRandomClearTile(false, false)` is deterministic given the seed; its absolute correctness
///    is the per-function tests'). The spec's "getNearClearTile selects the mathematically nearest
///    tile" is wrong - stage 14 established it as a random pick among `dist^2 < 10` candidates with
///    a fallback chain, not a nearest search. The near leg is skipped zoo-wide (not a failure) when
///    the zoo has no keeper - the only caller shape the `ZTStaff`-only reserved-tile vector read is
///    safe for.
/// 5. Raycast within the exhibit: per direction (0-7 plus the `-1`/`0xffffffff` sentinel),
///    `getRandomTileInDirection` returns a direction-candidate tile or - through vanilla's own
///    `getRandomTile` fallback - still an owned tile; `getRandomClearTileAhead` obeys the same
///    contract over its ahead-of-heading candidate set. The spec's "strictly within the exhibit
///    perimeter" is read as owned-tile membership: both functions' candidate sets and their shared
///    fallback draw exclusively from the owned-tile list.
///
/// Everything is read-only or exactly restored (scratch vectors freed via
/// [`free_event_vector_buffer`], seeds restored where cross-agreement is asserted; advancing the
/// shared game RNG is what every live test already does). Non-vacuousness: the save must own at
/// least one tile across all habitats, or every invariant above holds vacuously.
pub(crate) fn run_habitat_terrain_passability_multi_reimpl_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_TERRAIN_PASSABILITY_MULTI_REIMPL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    // Leg 4's near half resolves its keeper zoo-wide once (real caller shape `ZTGoalPutFood::decide`):
    // no keeper (or a tile-less one) skips that leg, not a failure; a malformed `ZTStaff`-only
    // reserved-tile vector fails loudly rather than letting the reserved-tile scan (both sides') walk
    // unbounded memory.
    let mut near_skip_note = String::new();
    let keeper = world
        .entity_array()
        .find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) })
        .map(|unit| {
            let unit_tile = (unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) }) as u32;
            (unit, unit_tile)
        });
    let keeper = match keeper {
        None => {
            near_skip_note = "near leg skipped: no live ZTKeeper found".to_string();
            None
        }
        Some((_, 0)) => {
            near_skip_note = "near leg skipped: live ZTKeeper has no tile".to_string();
            None
        }
        Some((unit, unit_tile)) => {
            let reserved_begin: u32 = get_from_memory(unit + 0x27c);
            let reserved_end: u32 = get_from_memory(unit + 0x280);
            let reserved_len = reserved_end.wrapping_sub(reserved_begin);
            if reserved_end < reserved_begin || !reserved_len.is_multiple_of(4) || reserved_len > 0x10000 {
                let msg = format!("keeper {:#010x} reserved-tile vector malformed: {:#010x}..{:#010x}", unit, reserved_begin, reserved_end);
                error!("{}: {}", test_name, msg);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                }
                return true;
            }
            Some((unit, unit_tile))
        }
    };

    let mut failures: Vec<String> = Vec::new();
    let mut total_owned_tiles = 0usize;
    let mut clear_pools = 0usize;
    let mut random_draws = 0usize;
    let mut random_non_null = 0usize;
    let mut adjacency_checks = 0usize;
    let mut adjacency_skipped = 0usize;
    let mut nearest_checks = 0usize;
    let mut nearest_skipped = 0usize;
    let mut near_checks = 0usize;
    let mut near_fallback_agreements = 0usize;
    let mut near_skipped = 0usize;
    let mut raycast_draws = 0usize;

    for (i, &ptr) in habitat_ptrs.iter().enumerate() {
        // Settle: one unasserted real draw triggers any pending lazy recalculate and leaves
        // `all_animals` exactly as every snapshot below sees it.
        unsafe { zthabitat::GET_RANDOM_ANIMAL.original()(ptr as *const std::ffi::c_void) };
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let tiles: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        total_owned_tiles += tiles.len();
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let count = (end.wrapping_sub(begin)) / 4;
        let animals: Vec<u32> = (0..count).map(|u| get_from_memory::<u32>(begin + u * 4)).filter(|&a| a != 0).collect();
        let unit_and_tile = animals.first().copied().and_then(|unit| {
            let unit_tile = (unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) }) as u32;
            (unit_tile != 0).then_some((unit, unit_tile))
        });
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let from_tile = tiles.first().copied().unwrap_or(0);
        let gate_tile_ptr = habitat.get_gate_tile_in().map(|tile| world.get_ptr_from_bftile(&tile)).unwrap_or(0);

        // Leg 1 - clear-tile pool: both sides, element-for-element, owned-tile membership.
        let real_pool = vanilla_clear_tile_pool(ptr, &[], 0, false, false);
        let mut reimpl_scratch = [0u32; 3];
        habitat.add_clear_tiles(reimpl_scratch.as_mut_ptr() as u32, 0, false);
        let reimpl_pool: Vec<u32> = (reimpl_scratch[0]..reimpl_scratch[1]).step_by(4).map(get_from_memory::<u32>).collect();
        free_event_vector_buffer(reimpl_scratch[0], reimpl_scratch[2].wrapping_sub(reimpl_scratch[0]));
        clear_pools += 2;
        if real_pool != reimpl_pool {
            failures.push(format!(
                "habitat {} ({:#010x}): clear-tile pools disagree - real ({:?}) vs reimpl ({:?})",
                i, ptr, real_pool, reimpl_pool
            ));
        }
        for (side, pool) in [("real", &real_pool), ("reimpl", &reimpl_pool)] {
            for &tile in pool {
                if !tiles.contains(&tile) {
                    failures.push(format!("habitat {} ({:#010x}) {}: clear pool tile {:#010x} is not an owned tile", i, ptr, side, tile));
                }
            }
        }

        // Leg 2 - random-clear-tile membership, 100 draws per side.
        let mut drawn_tiles: Vec<u32> = Vec::new();
        for _ in 0..100 {
            for (side, drawn) in [
                ("real", (unsafe { zthabitat::GET_RANDOM_CLEAR_TILE_0.original()(ptr as *const u32, false, false) }) as u32),
                ("reimpl", habitat.get_random_clear_tile_default(false, false)),
            ] {
                random_draws += 1;
                if drawn == 0 {
                    continue;
                }
                random_non_null += 1;
                if !drawn_tiles.contains(&drawn) {
                    drawn_tiles.push(drawn);
                }
                let pool = if side == "real" { &real_pool } else { &reimpl_pool };
                if !pool.contains(&drawn) {
                    failures.push(format!(
                        "habitat {} ({:#010x}) {}: getRandomClearTile drew {:#010x} outside the null-animal clear pool of {} tiles",
                        i, ptr, side, drawn, pool.len()
                    ));
                }
            }
        }

        // Leg 3 - adjacency of the drawn tiles.
        if let Some((unit, _)) = unit_and_tile {
            for &base in &drawn_tiles {
                let candidates = adjacent_clear_candidates(habitat_mgr, world, unit, base, max_cost);
                for (side, returned) in [
                    ("real", (unsafe { zthabitat::GET_ADJACENT_CLEAR_TILE.original()(unit as *const u32, base as *const u32) }) as u32),
                    ("reimpl", ZTHabitat::get_adjacent_clear_tile(unit, base)),
                ] {
                    adjacency_checks += 1;
                    let ok = if candidates.is_empty() { returned == base } else { candidates.contains(&returned) };
                    if !ok {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: getAdjacentClearTile at base {:#010x} returned {:#010x}, expected {} (of {} candidates)",
                            i,
                            ptr,
                            side,
                            base,
                            returned,
                            if candidates.is_empty() { "the base tile back".to_string() } else { "a candidate".to_string() },
                            candidates.len()
                        ));
                    }
                }
            }
        } else {
            adjacency_skipped += 1;
        }

        // Leg 4a - nearest, exact against the shared oracle; the shared seed is restored between the
        // sides so both draw the same internal animal slot.
        if let Some((unit, unit_tile)) = unit_and_tile {
            let seed_at_predict: u32 = get_from_memory(rng_addr);
            let expected_rng = lcg_next(seed_at_predict);
            let random_animal = if count == 0 {
                0
            } else {
                let index = ((expected_rng >> 0x10) & 0x7fff) % count;
                get_from_memory::<u32>(begin + index * 4)
            };
            let expected_tile = nearest_clear_tile_oracle(&tiles, unit, unit_tile, random_animal, max_cost);
            let real_ptr = (unsafe { zthabitat::GET_NEAREST_CLEAR_TILE.original()(ptr as *const u32, unit as *const u32) }) as u32;
            save_to_memory(rng_addr, seed_at_predict);
            let reimpl_ptr = habitat.get_nearest_clear_tile(unit);
            nearest_checks += 2;
            for (side, returned) in [("real", real_ptr), ("reimpl", reimpl_ptr)] {
                if returned != expected_tile {
                    failures.push(format!(
                        "habitat {} ({:#010x}) {}: getNearestClearTile returned {:#010x}, oracle expected {:#010x} (predicted animal {:#010x} of {} slots)",
                        i, ptr, side, returned, expected_tile, random_animal, count
                    ));
                }
            }
        } else {
            nearest_skipped += 1;
        }

        // Leg 4b - near: membership when the candidate set is non-empty, restored-seed cross-agreement
        // of (tile, seed) through the fallback chain when it is empty.
        if let Some((keeper, keeper_tile)) = keeper {
            for animal in std::iter::once(0u32).chain(animals.first().copied()) {
                let candidates = near_clear_tile_candidates(&tiles, keeper, keeper_tile, animal, gate_tile_ptr, max_cost);
                let seed_before: u32 = get_from_memory(rng_addr);
                let real_ptr =
                    (unsafe { zthabitat::GET_NEAR_CLEAR_TILE.original()(ptr as *const u32, keeper as *const u32, animal as *const u32) }) as u32;
                let real_seed: u32 = get_from_memory(rng_addr);
                save_to_memory(rng_addr, seed_before);
                let reimpl_ptr = habitat.get_near_clear_tile(keeper, animal);
                let reimpl_seed: u32 = get_from_memory(rng_addr);
                near_checks += 2;
                if !candidates.is_empty() {
                    for (side, drawn) in [("real", real_ptr), ("reimpl", reimpl_ptr)] {
                        if !candidates.contains(&drawn) {
                            failures.push(format!(
                                "habitat {} ({:#010x}) {}: getNearClearTile(animal={:#010x}) returned {:#010x}, outside the {}-candidate near set",
                                i, ptr, side, animal, drawn, candidates.len()
                            ));
                        }
                    }
                } else if reimpl_ptr == real_ptr && reimpl_seed == real_seed {
                    near_fallback_agreements += 1;
                } else {
                    failures.push(format!(
                        "habitat {} ({:#010x}), animal={:#010x}: empty fallback disagreement - reimpl ({:#010x}, rng {:#010x}) vs real ({:#010x}, rng {:#010x})",
                        i, ptr, animal, reimpl_ptr, reimpl_seed, real_ptr, real_seed
                    ));
                }
            }
        } else {
            near_skipped += 1;
        }

        // Leg 5 - raycast within the exhibit (skipped for tile-less habitats).
        if !tiles.is_empty() {
            for direction in [0u32, 1, 2, 3, 4, 5, 6, 7, 0xffff_ffff] {
                let candidates = directional_tile_candidates(&tiles, from_tile, direction as i32);
                for (side, returned) in [
                    ("real", (unsafe { zthabitat::GET_RANDOM_TILE_IN_DIRECTION.original()(ptr as *const u32, from_tile as *const u32, direction) }) as u32),
                    ("reimpl", habitat.get_random_tile_in_direction(from_tile, direction)),
                ] {
                    raycast_draws += 1;
                    let ok = if candidates.is_empty() { tiles.contains(&returned) } else { candidates.contains(&returned) };
                    if !ok {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: getRandomTileInDirection(dir {:#x}) returned {:#010x}, expected {}",
                            i,
                            ptr,
                            side,
                            direction,
                            returned,
                            if candidates.is_empty() { "an owned tile (empty-set getRandomTile fallback)".to_string() } else { "a direction candidate".to_string() }
                        ));
                    }
                }
            }
            if let Some((unit, unit_tile)) = unit_and_tile {
                let rotation: u32 = get_from_memory(unit + 0x12c);
                let heading = if rotation == 0xffff_ffff { rotation } else { rotation.wrapping_sub(4) & 7 };
                let candidates: Vec<u32> = tiles
                    .iter()
                    .copied()
                    .filter(|&tile| {
                        let cost = unsafe { call_bfunit_tile_cost_vtable_slot(unit, tile) };
                        if cost >= max_cost {
                            return false;
                        }
                        let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(unit_tile as i32, tile as i32) };
                        !is_close_direction(heading as i32, dir)
                    })
                    .collect();
                for (side, returned) in [
                    ("real", (unsafe { zthabitat::GET_RANDOM_CLEAR_TILE_AHEAD.original()(ptr as *const u32, unit as *const u32) }) as u32),
                    ("reimpl", habitat.get_random_clear_tile_ahead(unit)),
                ] {
                    raycast_draws += 1;
                    let ok = if candidates.is_empty() { tiles.contains(&returned) } else { candidates.contains(&returned) };
                    if !ok {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: getRandomClearTileAhead returned {:#010x}, expected {}",
                            i,
                            ptr,
                            side,
                            returned,
                            if candidates.is_empty() { "an owned tile (empty-set getRandomTile fallback)".to_string() } else { "an ahead candidate".to_string() }
                        ));
                    }
                }
            }
        }
    }

    if total_owned_tiles == 0 {
        let msg = "all invariants were vacuous: no owned tiles across any live habitat".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    if failures.is_empty() {
        let mut summary = format!(
            "{} (habitats: {}, owned tiles: {}, clear pools: {}, random draws: {} (non-null {}), adjacency checks: {} (skipped: {}), nearest checks: {} (skipped: {}), near checks: {} (fallback agreements: {}, skipped: {}), raycast draws: {}",
            test_name, habitat_ptrs.len(), total_owned_tiles, clear_pools, random_draws, random_non_null, adjacency_checks, adjacency_skipped, nearest_checks, nearest_skipped, near_checks, near_fallback_agreements, near_skipped, raycast_draws
        );
        if !near_skip_note.is_empty() {
            summary.push_str(&format!(", {near_skip_note}"));
        }
        summary.push(')');
        write_success_line(failure_log, &summary);
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

/// Multi-reimplementation integration test (stage 34 of
/// `openzt/plans/zthabitat-additional-functions-plan.md`): drives the gate-navigation
/// reimplementations together over the live zoo's own habitats, asserting cross-getter contracts
/// per side rather than re-asserting the per-draw LCG slot fidelity the per-function live tests
/// already pin (the same scoping [`run_habitat_terrain_passability_multi_reimpl_live_test`] used).
/// Real vanilla is called before the reimplementation throughout; every address involved
/// (`getGateTileIn`/`getGateTileOut`/both gate-pass resolvers/`getHabitat`/`leadsTo`) is detoured,
/// so in release `.original()` re-enters the ports and those legs are port-vs-port there - the
/// real-vs-reimpl diffs stay with `ZTHABITAT_GET_GATE_TILE_OUT_LIVE`,
/// `ZTHABITAT_GET_GATE_TILE_PASS_IN/OUT_LIVE` and `ZTHABITATMGR_LEADS_TO_LIVE`. Gate-in has no
/// dedicated comparison test anywhere, so leg 1 gives it its first live real-vs-reimpl coverage
/// (debug). Per habitat, over `exhibit_array`:
///
/// 1. Gate resolution: real `getGateTileIn`/`getGateTileOut` agree with the ports
///    `get_gate_tile_in()`/`get_gate_tile_out()` mapped through `get_ptr_from_bftile` (0 for
///    `None`).
/// 2. Interior/exterior ownership: when a gate tile resolves, the gate-in tile is owned by this
///    habitat and the gate-out tile is not ([`ZTHabitatMgr::get_habitat_ptr`]). The spec's "sits
///    inside the exhibit interior"/"sits on the exterior public path" have no path-surface notion
///    in the real functions - the faithful contract is tile ownership plus the same-owner
///    candidacy rule leg 3 rebuilds.
/// 3. Pass resolvers (needs a unit - the habitat's own first live animal, the per-function tests'
///    driver convention; animal-free habitats are skipped, not failures): with the
///    8-neighborhood candidate set rebuilt once from the port gate tile
///    ([`adjacent_clear_candidates`]; leg 1 pins real==port gate tiles, so one rebuild serves both
///    sides), a gate-less habitat must return null with the shared RNG seed untouched, and a
///    gate-having habitat must return a non-null member of the candidate set, or the gate tile
///    itself when that set is empty. Also asserts the pass-out result's own chain link: same owner
///    as its gate-out base tile (the same-owner candidacy rule the rebuild encodes).
/// 4. Interior/exterior of the pass results: the pass-in result is owned by this habitat, the
///    pass-out result is not (follows from legs 2+3, asserted explicitly).
/// 5. Gate-out chain continuity vs `leadsTo`: per non-world habitat, the gate-out walk chain is
///    derived independently - exactly as the `leadsTo` decompile walks it: reset the shared
///    scratch markers, mark `a`, then repeatedly step gate-out -> `getHabitatPtr(tile pos)`,
///    stopping at a null tile, an already-visited habitat, or a "world" habitat
///    (`unknown_flag_0x2c`), pushing each new step. The chain is derived twice (real
///    `GET_GATE_TILE_OUT` + `GET_HABITAT` poles; port getters) and the two must agree
///    element-for-element; then every `b` over `exhibit_array` plus null must satisfy
///    `leadsTo(a, b) == (b != 0 && world_flag(b) == 0 && chain.contains(&b))` on both sides. The
///    spec's "pathfinding continuity from getGateTilePassOut to the zoo entrance via leadsTo" is
///    wrong on two counts: `leadsTo` takes two habitat pointers, not tiles, and has no
///    zoo-entrance involvement whatsoever (the entrance tile is `ZTHabitatMgr`'s own global,
///    covered by `ZTHABITATMGR_GET_ZOO_ENTRANCE_TILE_LIVE`); the faithful contract is the chain
///    containment check here.
///
/// Spec deviations, restated: the spec's blanket "both return non-null tiles" (assertion 3) only
/// holds for habitats that resolve a gate pair - a gate-less habitat's null gate tile passes
/// through `getAdjacentClearTile`'s own null guard as null, by design; "interior"/"exterior
/// public path" (assertions 4/5) are read as tile ownership plus same-owner candidacy, not path
/// surfaces; assertion 6's zoo-entrance continuity does not exist in the real function.
///
/// Everything is read-only except the shared `tank_walk_visited_marker` scratch flag (each real
/// and port `leadsTo` call and each derivation resets it itself) and the shared game RNG the pass
/// draws advance exactly as every live test already does. Non-vacuousness: the save must resolve
/// at least one gate pair and contain at least one non-world habitat, or every invariant above
/// holds vacuously.
pub(crate) fn run_habitat_gate_pass_traversal_multi_reimpl_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GATE_PASS_TRAVERSAL_MULTI_REIMPL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut gate_habitats = 0usize;
    let mut gateless_habitats = 0usize;
    let mut pass_checks = 0usize;
    let mut pass_skipped = 0usize;
    let mut base_back_picks = 0usize;
    let mut membership_picks = 0usize;
    let mut continuity_pairs = 0usize;
    let mut non_empty_chains = 0usize;

    let tile_owner = |tile_ptr: u32| -> u32 {
        let x: i32 = get_from_memory(tile_ptr + 0x34);
        let y: i32 = get_from_memory(tile_ptr + 0x38);
        habitat_mgr.get_habitat_ptr(x, y)
    };
    let world_flag = |habitat_ptr: u32| -> bool { unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.unknown_flag_0x2c != 0 };

    for (i, &ptr) in habitat_ptrs.iter().enumerate() {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };

        // Leg 1 - gate resolution agreement (gate-in's first live real-vs-reimpl coverage).
        let real_in = (unsafe { zthabitat::GET_GATE_TILE_IN.original()(ptr as *const u32) }) as u32;
        let real_out = (unsafe { zthabitat::GET_GATE_TILE_OUT.original()(ptr as *const u32) }) as u32;
        let port_in = habitat.get_gate_tile_in().map(|tile| world.get_ptr_from_bftile(&tile)).unwrap_or(0);
        let port_out = habitat.get_gate_tile_out().map(|tile| world.get_ptr_from_bftile(&tile)).unwrap_or(0);
        if real_in != port_in {
            failures.push(format!("habitat {} ({:#010x}): getGateTileIn real {:#010x} != port {:#010x}", i, ptr, real_in, port_in));
        }
        if real_out != port_out {
            failures.push(format!("habitat {} ({:#010x}): getGateTileOut real {:#010x} != port {:#010x}", i, ptr, real_out, port_out));
        }

        // Leg 2 - interior/exterior ownership of the resolved pair.
        if port_in == 0 {
            gateless_habitats += 1;
        } else {
            gate_habitats += 1;
            let owner = tile_owner(port_in);
            if owner != ptr {
                failures.push(format!(
                    "habitat {} ({:#010x}): gate-in tile {:#010x} is owned by {:#010x}, expected this habitat (the interior/exterior ownership invariant)",
                    i, ptr, port_in, owner
                ));
            }
        }
        if port_out != 0 {
            let owner = tile_owner(port_out);
            if owner == ptr {
                failures.push(format!(
                    "habitat {} ({:#010x}): gate-out tile {:#010x} is owned by this habitat, expected an exterior tile",
                    i, ptr, port_out
                ));
            }
        }

        // Legs 3+4 - pass resolvers (membership-level) and the pass results' own interior/exterior
        // ownership.
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let Some(unit) = (0..(end.wrapping_sub(begin)) / 4).map(|u| get_from_memory::<u32>(begin + u * 4)).find(|&a| a != 0) else {
            pass_skipped += 1;
            continue;
        };
        for direction in [0usize, 1] {
            let (direction, gate_ptr) = if direction == 0 { ("in", port_in) } else { ("out", port_out) };
            let seed_before: u32 = get_from_memory(rng_addr);
            let (real_pass, port_pass) = if direction == "in" {
                (
                    (unsafe { zthabitat::GET_GATE_TILE_PASS_IN.original()(ptr as *const u32, unit as *const u32) }) as u32,
                    habitat.get_gate_tile_pass_in(unit),
                )
            } else {
                (
                    (unsafe { zthabitat::GET_GATE_TILE_PASS_OUT.original()(ptr as *const u32, unit as *const u32) }) as u32,
                    habitat.get_gate_tile_pass_out(unit),
                )
            };
            let candidates = if gate_ptr == 0 { Vec::new() } else { adjacent_clear_candidates(habitat_mgr, world, unit, gate_ptr, max_cost) };
            for (side, returned) in [("real", real_pass), ("port", port_pass)] {
                pass_checks += 1;
                if gate_ptr == 0 {
                    // Null-gate pass-through: the null gate tile rides through
                    // `getAdjacentClearTile`'s own null guard - null return, seed untouched.
                    if returned != 0 {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: gate-less {} pass returned {:#010x}, expected null",
                            i, ptr, side, direction, returned
                        ));
                    }
                    let seed_after: u32 = get_from_memory(rng_addr);
                    if seed_after != seed_before {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: gate-less {} pass moved the shared RNG seed ({:#010x} -> {:#010x}), expected it untouched",
                            i, ptr, side, direction, seed_before, seed_after
                        ));
                    }
                    continue;
                }
                if candidates.is_empty() {
                    if returned == gate_ptr {
                        base_back_picks += 1;
                    } else {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: {} pass returned {:#010x}, expected the gate tile itself back ({:#010x}; empty candidate set)",
                            i, ptr, side, direction, returned, gate_ptr
                        ));
                    }
                } else if candidates.contains(&returned) {
                    membership_picks += 1;
                } else {
                    failures.push(format!(
                        "habitat {} ({:#010x}) {}: {} pass returned {:#010x}, outside the {}-candidate set",
                        i, ptr, side, direction, returned, candidates.len()
                    ));
                }
                // The pass result's own interior/exterior ownership, guarded on the non-null check
                // leg 3 already asserts (a null result failed above; nothing to look up).
                if returned != 0 {
                    let owner = tile_owner(returned);
                    let expected_owned = direction == "in";
                    if (owner == ptr) != expected_owned {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: {} pass result {:#010x} is owned by {:#010x}, expected {}",
                            i,
                            ptr,
                            side,
                            direction,
                            returned,
                            owner,
                            if expected_owned { "this habitat (the interior side)" } else { "a different owner (the exterior side)" }
                        ));
                    }
                }
            }
            // The pass-out result's own chain link: same owner as its gate-out base tile.
            if direction == "out" && gate_ptr != 0 {
                let gate_owner = tile_owner(gate_ptr);
                for (side, returned) in [("real", real_pass), ("port", port_pass)] {
                    if returned == 0 {
                        continue;
                    }
                    let owner = tile_owner(returned);
                    if owner != gate_owner {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: pass-out result {:#010x} is owned by {:#010x}, but its gate-out base tile {:#010x} is owned by {:#010x}",
                            i, ptr, side, returned, owner, gate_ptr, gate_owner
                        ));
                    }
                }
            }
        }
    }

    // Leg 5 - gate-out chain continuity vs leadsTo, derived independently per non-world habitat.
    // Each derivation resets the shared scratch markers and mirrors the `leadsTo` decompile's own
    // walk (visited bookkeeping included); the real-pole derivation and the port derivation must
    // agree, and both `leadsTo` sides must equal the chain-containment expectation for every `b`.
    let derive_chain = |use_real: bool, a: u32| -> Vec<u32> {
        habitat_mgr.clear_tank_walk_markers();
        unsafe { mut_from_memory::<ZTHabitat>(a) }.tank_walk_visited_marker = 1;
        let mut chain: Vec<u32> = Vec::new();
        let mut current = a;
        loop {
            let gate = if use_real {
                (unsafe { zthabitat::GET_GATE_TILE_OUT.original()(current as *const u32) }) as u32
            } else {
                unsafe { ref_from_memory::<ZTHabitat>(current) }
                    .get_gate_tile_out()
                    .map(|tile| world.get_ptr_from_bftile(&tile))
                    .unwrap_or(0)
            };
            if gate == 0 {
                break;
            }
            let x: i32 = get_from_memory(gate + 0x34);
            let y: i32 = get_from_memory(gate + 0x38);
            let next = if use_real {
                unsafe { zthabitatmgr::GET_HABITAT.original()(mgr_ptr, x, y) }
            } else {
                habitat_mgr.get_habitat_ptr(x, y)
            };
            if next == 0 {
                break;
            }
            if unsafe { ref_from_memory::<ZTHabitat>(next) }.tank_walk_visited_marker != 0 {
                break;
            }
            unsafe { mut_from_memory::<ZTHabitat>(next) }.tank_walk_visited_marker = 1;
            if world_flag(next) {
                break;
            }
            chain.push(next);
            current = next;
        }
        chain
    };
    let non_world_habitats = habitat_ptrs.iter().copied().filter(|ptr| !world_flag(*ptr)).count();
    for (i, &a) in habitat_ptrs.iter().enumerate() {
        if world_flag(a) {
            continue;
        }
        let real_chain = derive_chain(true, a);
        let port_chain = derive_chain(false, a);
        if real_chain != port_chain {
            failures.push(format!(
                "habitat {} ({:#010x}): gate-out walk chain disagrees - real {:?} vs port {:?}",
                i, a, real_chain, port_chain
            ));
        }
        if !port_chain.is_empty() {
            non_empty_chains += 1;
        }
        for b in habitat_ptrs.iter().copied().chain(std::iter::once(0)) {
            let expected = b != 0 && !world_flag(b) && port_chain.contains(&b);
            continuity_pairs += 1;
            let real_value = low_byte_bool(unsafe { zthabitatmgr::LEADS_TO.original()(mgr_ptr, a as *const u32, b as *const u32) });
            if real_value != expected {
                failures.push(format!(
                    "habitat {} ({:#010x}) -> {:#010x}: leadsTo real {} != expected {} (chain {:?})",
                    i, a, b, real_value, expected, port_chain
                ));
            }
            let port_value = habitat_mgr.leads_to(a, b);
            if port_value != expected {
                failures.push(format!(
                    "habitat {} ({:#010x}) -> {:#010x}: leads_to port {} != expected {} (chain {:?})",
                    i, a, b, port_value, expected, port_chain
                ));
            }
        }
    }

    if gate_habitats == 0 || non_world_habitats == 0 {
        let msg = format!(
            "all invariants were vacuous: gate habitats {}, non-world habitats {} (of {} live habitats)",
            gate_habitats,
            non_world_habitats,
            habitat_ptrs.len()
        );
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, gate habitats: {}, gateless habitats: {}, pass checks: {} (skipped: {}), base-back picks: {}, membership picks: {}, continuity pairs: {} (non-empty chains: {}))",
                test_name,
                habitat_ptrs.len(),
                gate_habitats,
                gateless_habitats,
                pass_checks,
                pass_skipped,
                base_back_picks,
                membership_picks,
                continuity_pairs,
                non_empty_chains
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

/// Stage 35 of the zthabitat-additional-functions plan (multi-reimplementation integration test):
/// cross-validates the three biome layers' four function families against each other over the live
/// zoo. Test-only - no production code, no new detours. All twelve biome functions are detoured, so
/// in release every `.original()` pole re-enters the ports and the real-vs-reimpl equality legs are
/// port-vs-port there - the genuine real-vs-reimpl diffs stay with the Stage 16-19 per-function
/// tests (same documented shape as [`BiomeTileKind::real`]'s own doc). No leg needs a
/// `characteristics_dirty` settling draw - none of the twelve bodies recalculates. Per habitat over
/// `exhibit_array`:
///
/// 1. Subhabitat inheritance (leg 1): for each of the three kinds, both sides' `get*Tiles` must
///    equal, in order, the habitat's own real `add*Tiles` plus each amphibious neighbor's own real
///    `add*Tiles` in [`walk_neighbor_tree`] order - one flat level of fan-in, the decompiled
///    aggregation shape ([`ZTHabitat::get_tiles_aggregating`]).
/// 2. num/len cross-getter contract (legs 2-4): real `getNum*Tiles` == the real `get*Tiles` list
///    length, port `get_num_*` == the port `get_*_tiles` list length, and real num == port num.
/// 3. Random membership sweep (leg 5): 50 draws per kind per side; a non-empty aggregate must yield
///    a member of that side's own aggregate, an empty pool must yield null with the shared RNG seed
///    untouched (the `.asm`'s `JLE` gate precedes any RNG touch). Membership level only - the exact
///    `(seed >> 0x10 & 0x7fff) % count` slot contract stays with the Stage 19 tests - and the
///    draws advance the shared game RNG unrestored (nothing downstream asserts a seed value).
/// 4. Partition identity + strict sum (leg 6): on every habitat and both sides,
///    `land + water + underwater == universe + |land ∩ water|` - the identity the decompiles
///    actually guarantee (the filters are independent bits, so underwater is the complement of
///    land-union-water and land/water overlap is legal) - then the spec's strict
///    `getNumLandTiles + getNumWaterTiles == universe` only for a qualifying "standard non-tank
///    exhibit" (no amphibious neighbors, not a tank, empty underwater aggregate, zero land/water
///    overlap). A non-qualifying habitat is counted per first-failing gate (neighbors / tank /
///    underwater / overlap) in the success line, never a failure, and zero standard exhibits on a
///    save is a documented skip note rather than a failure - the identity assert carries the leg
///    everywhere.
///
/// The aggregated tile universe is each habitat's own owned tiles plus every neighbor's own owned
/// tiles (deduped by pointer containment; habitats never share tiles - the dedup is defensive).
/// Everything is read-only except the shared game RNG the sweep advances; scratch vectors are freed
/// via [`free_event_vector_buffer`]. Non-vacuousness: the save must contain at least one genuine
/// amphibious connection contributing at least one tile, and at least one non-empty aggregate per
/// kind, else every invariant above holds vacuously.
pub(crate) fn run_habitat_biome_aggregation_multi_reimpl_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_BIOME_AGGREGATION_MULTI_REIMPL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut habitats_with_neighbors = 0usize;
    let mut neighbor_tiles_total = 0usize;
    let mut num_len_checks = 0usize;
    let mut num_cross_checks = 0usize;
    let mut random_draws = 0usize;
    let mut random_non_null = 0usize;
    let mut empty_pool_draws = 0usize;
    let mut identity_checks = 0usize;
    let mut strict_sum_checks = 0usize;
    let mut standard_exhibits = 0usize;
    let mut excluded_neighbors = 0usize;
    let mut excluded_tanks = 0usize;
    let mut excluded_underwater = 0usize;
    let mut excluded_overlap = 0usize;
    let mut kind_non_empty = [0usize; 3];

    for (i, &ptr) in habitat_ptrs.iter().enumerate() {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let neighbor_ptrs: Vec<u32> =
            walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        if !neighbor_ptrs.is_empty() {
            habitats_with_neighbors += 1;
        }

        // The aggregated tile universe: this habitat's own owned tiles plus every neighbor's own
        // owned tiles (deduped by pointer containment; habitats never share tiles - the dedup is
        // defensive).
        let sentinel: u32 = get_from_memory(ptr + 0x40);
        let mut universe: Vec<u32> = walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 0x8)).collect();
        for &neighbor in &neighbor_ptrs {
            let neighbor_sentinel: u32 = get_from_memory(neighbor + 0x40);
            let neighbor_tiles: Vec<u32> = walk_tile_list(neighbor_sentinel)
                .map(|node| get_from_memory::<u32>(node + 0x8))
                .filter(|tile| !universe.contains(tile))
                .collect();
            universe.extend(neighbor_tiles);
        }

        // Expected aggregation per kind (Stage 17's assembly shape): the habitat's own real
        // add*Tiles, then each neighbor's own real add*Tiles in walk order - one flat level of
        // fan-in, exactly what real vanilla's get*Tiles and the port's get_tiles_aggregating both
        // produce.
        let mut expected: [Vec<u32>; 3] = Default::default();
        for (k, &kind) in ALL_BIOME_KINDS.iter().enumerate() {
            expected[k] = extract_vector(|scratch| kind.real(ptr, scratch));
            for &neighbor in &neighbor_ptrs {
                let part = extract_vector(|scratch| kind.real(neighbor, scratch));
                neighbor_tiles_total += part.len();
                expected[k].extend(part);
            }
            if !expected[k].is_empty() {
                kind_non_empty[k] += 1;
            }
        }

        let mut real_lists: [Vec<u32>; 3] = Default::default();
        let mut port_lists: [Vec<u32>; 3] = Default::default();
        let mut real_nums = [0i32; 3];
        let mut port_nums = [0i32; 3];
        for (k, &kind) in ALL_BIOME_KINDS.iter().enumerate() {
            real_lists[k] = extract_vector(|scratch| kind.get_real(ptr, scratch));
            port_lists[k] = extract_vector(|scratch| kind.get_port(habitat, scratch.as_mut_ptr() as u32));

            // Leg 1 - subhabitat inheritance, element-for-element (ordered equality pins the walk
            // order; strictly stronger than the spec's containment wording).
            for (side, list) in [("real", &real_lists[k]), ("port", &port_lists[k])] {
                if list != &expected[k] {
                    let first_diff = list.iter().zip(expected[k].iter()).position(|(a, b)| a != b).unwrap_or(list.len().min(expected[k].len()));
                    failures.push(format!(
                        "habitat {} ({:#010x}) {}: {:?} get*Tiles disagrees with the own-add + per-neighbor-add aggregation - {} tiles vs expected {}, first difference at index {}",
                        i, ptr, side, kind, list.len(), expected[k].len(), first_diff
                    ));
                }
            }

            // Legs 2-4 - num/len cross-getter contract.
            real_nums[k] = kind.num_real(ptr);
            port_nums[k] = kind.num_port(habitat);
            num_len_checks += 2;
            if real_nums[k] != real_lists[k].len() as i32 {
                failures.push(format!(
                    "habitat {} ({:#010x}) {:?}: real getNum*Tiles {} != real get*Tiles len {}",
                    i, ptr, kind, real_nums[k], real_lists[k].len()
                ));
            }
            if port_nums[k] != port_lists[k].len() as i32 {
                failures.push(format!(
                    "habitat {} ({:#010x}) {:?}: port get_num_* {} != port get_*_tiles len {}",
                    i, ptr, kind, port_nums[k], port_lists[k].len()
                ));
            }
            num_cross_checks += 1;
            if real_nums[k] != port_nums[k] {
                failures.push(format!("habitat {} ({:#010x}) {:?}: real num {} != port num {}", i, ptr, kind, real_nums[k], port_nums[k]));
            }

            // Leg 5 - random membership sweep (50 draws per side). The seed is read immediately
            // before each draw; only the empty-pool arm asserts seed-untouched. Draws advance the
            // shared game RNG and are not restored.
            for _ in 0..50 {
                for side in ["real", "port"] {
                    let seed_before: u32 = get_from_memory(rng_addr);
                    let (drawn, pool) = if side == "real" {
                        (kind.random_real(ptr), &real_lists[k])
                    } else {
                        (kind.random_port(habitat), &port_lists[k])
                    };
                    random_draws += 1;
                    if pool.is_empty() {
                        empty_pool_draws += 1;
                        if drawn != 0 {
                            failures.push(format!(
                                "habitat {} ({:#010x}) {} {:?}: draw {:#010x} from an empty aggregate, expected null",
                                i, ptr, side, kind, drawn
                            ));
                        }
                        let seed_after: u32 = get_from_memory(rng_addr);
                        if seed_after != seed_before {
                            failures.push(format!(
                                "habitat {} ({:#010x}) {} {:?}: empty-pool draw moved the shared RNG seed ({:#010x} -> {:#010x}), expected it untouched",
                                i, ptr, side, kind, seed_before, seed_after
                            ));
                        }
                    } else if drawn != 0 && pool.contains(&drawn) {
                        random_non_null += 1;
                    } else {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {} {:?}: draw {:#010x} outside the aggregated biome vector ({} tiles){}",
                            i,
                            ptr,
                            side,
                            kind,
                            drawn,
                            pool.len(),
                            if drawn == 0 { " (null draw from a non-empty pool)" } else { "" }
                        ));
                    }
                }
            }
        }

        // Leg 6 - the decompile-guaranteed partition identity (underwater is the complement of
        // land-union-water by predicate construction, so the only double-counted tiles are
        // land+water), then the spec's strict sum restricted to qualifying standard exhibits.
        let overlap = real_lists[0].iter().filter(|tile| real_lists[1].contains(tile)).count();
        for (side, lists) in [("real", &real_lists), ("port", &port_lists)] {
            identity_checks += 1;
            let sum = lists[0].len() + lists[1].len() + lists[2].len();
            let identity_total = universe.len() + overlap;
            if sum != identity_total {
                failures.push(format!(
                    "habitat {} ({:#010x}) {}: land {} + water {} + underwater {} = {}, expected universe {} + land/water overlap {} = {}",
                    i, ptr, side, lists[0].len(), lists[1].len(), lists[2].len(), sum, universe.len(), overlap, identity_total
                ));
            }
        }
        if neighbor_ptrs.is_empty() && !habitat.is_tank() && real_lists[2].is_empty() && overlap == 0 {
            standard_exhibits += 1;
            strict_sum_checks += 2;
            let expected_total = universe.len() as i32;
            if real_nums[0] + real_nums[1] != expected_total {
                failures.push(format!(
                    "habitat {} ({:#010x}): standard exhibit strict sum - real getNumLandTiles {} + getNumWaterTiles {} != universe {}",
                    i, ptr, real_nums[0], real_nums[1], expected_total
                ));
            }
            if port_nums[0] + port_nums[1] != expected_total {
                failures.push(format!(
                    "habitat {} ({:#010x}): standard exhibit strict sum - port get_num_land_tiles {} + get_num_water_tiles {} != universe {}",
                    i, ptr, port_nums[0], port_nums[1], expected_total
                ));
            }
        } else if !neighbor_ptrs.is_empty() {
            excluded_neighbors += 1;
        } else if habitat.is_tank() {
            excluded_tanks += 1;
        } else if !real_lists[2].is_empty() {
            excluded_underwater += 1;
        } else {
            excluded_overlap += 1;
        }
    }

    if habitats_with_neighbors == 0 || neighbor_tiles_total == 0 {
        failures.push(format!(
            "non-vacuous-recursion assert failed: habitats with amphibious neighbors: {}, neighbor-contributed tiles: {} - the live save must contain a genuine amphibious connection for this test to exercise the aggregation",
            habitats_with_neighbors, neighbor_tiles_total
        ));
    }
    for (k, &kind) in ALL_BIOME_KINDS.iter().enumerate() {
        if kind_non_empty[k] == 0 {
            failures.push(format!(
                "non-vacuous-draw assert failed: every habitat's {:?} aggregate is empty - the get/count/draw paths never ran on a non-empty pool on this save",
                kind
            ));
        }
    }

    if failures.is_empty() {
        let summary = format!(
            "{} (habitats: {}, with amphibious neighbors: {}, neighbor-contributed tiles: {}, num/len checks: {}, cross num checks: {}, random draws: {} (non-null {}, empty-pool {}), identity checks: {}, strict-sum checks: {} (standard exhibits: {}, exclusions: neighbors {} / tanks {} / underwater {} / overlap {}){}",
            test_name,
            habitat_ptrs.len(),
            habitats_with_neighbors,
            neighbor_tiles_total,
            num_len_checks,
            num_cross_checks,
            random_draws,
            random_non_null,
            empty_pool_draws,
            identity_checks,
            strict_sum_checks,
            standard_exhibits,
            excluded_neighbors,
            excluded_tanks,
            excluded_underwater,
            excluded_overlap,
            if standard_exhibits == 0 { ", strict-sum leg skipped: no standard non-tank exhibits on this save" } else { "" }
        );
        write_success_line(failure_log, &summary);
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

/// Stage 36 of the zthabitat-additional-functions plan (multi-reimplementation integration test):
/// drives the keeper-service / food-management / dirt-maintenance reimplementations (Stages 20-25)
/// together over the live zoo. Test-only - no production code, no new detours. Every involved
/// function is detoured, so in release each `.original()` pole re-enters the ports and the
/// real-vs-port agreement legs are port-vs-port there - the decompile-derived oracle legs carry both
/// profiles (same documented shape as the Stage 32-35 integration tests).
///
/// One shared phase-1 oracle walk per habitat (own owned tiles only) collects each habitat's
/// [`KeeperFoodOracleList`] plus the zoo-wide food-tile/category tallies, and the keeper is resolved
/// once from the live world entity array (legs 1/4 and leg 5's decomposition arm need one; they are
/// skipped with explicit summary notes, not failures, when the save has none). Legs, in run order:
///
/// 1. (spec 2) nearest-vs-smallest over each habitat's own food piles, own-exhibit scope
///    (`include_neighbors = false`; the subhab recursion is Stage 22's turf), per zoo-wide-seen
///    keeper-food category - so a habitat lacking a category forms an empty own pool and exercises
///    the null arms, while the full 0..16 sweep stays with the per-function tests. Real and port
///    picks must equal [`keeper_food_oracle_list_min`]'s argmin for both kinds; whenever the own
///    pool is non-empty, the smallest pick's `entity+0x154` amount must not exceed the nearest
///    pick's and the nearest pick's distance must not exceed the smallest pick's (the spec's literal
///    point, asserted over both sides' picks); pools of >= 2 count as multi-pile rows. The
///    [`KeeperFoodPickKind::Random`] triplet member draws 10 picks per side: a non-empty own pool
///    must yield a member of that pool (membership level per the Stage 11 note), an empty own pool
///    must yield null with the shared RNG seed untouched. Draws advance the shared game RNG
///    unrestored; exact per-draw slot fidelity stays with the Stage 22 tests.
/// 2. (spec 4) the dirt-pile locator against an oracle-min walk through the port's own documented
///    gate chain (see [`keeper_dirt_pile_oracle_min`]), per habitat per `check_can_see`. "Place a
///    dirt entity" is not fabricatable from the battery (entity creation is un-ported; the real gate
///    is the keeper's own live vtable `+0x324` dispatch, not a static type check), so the faithful
///    contract is the oracle-min equality - expected `0 == 0 == 0` on a save without dirt piles,
///    asserted and counted, never skipped - upgrading automatically on saves that contain dirt
///    piles. Real vanilla makes the same `+0x324` dispatches internally inside its own `.original()`
///    call; the oracle's own dispatch count is the non-vacuousness witness that the filter walk
///    genuinely ran.
/// 3. (spec 5) `needsShowKeeper` decomposition. The null-keeper arm runs without a live keeper (both
///    sides' own null early-out); per habitat with the live keeper, the oracle is the port's own
///    documented decomposition reproduced read-only - show info attached and
///    `ztshowinfo::needs_keeper(show_info, keeper_type_id)` passing, with `keeper_type_id` the
///    keeper's entity-type vtable `+0x20` call real vanilla makes unconditionally before the
///    show-info check - and real (low-byte masked) == port == oracle. "Show tanks with pending
///    performances return true" cannot be forced at battery time: habitats with no attached show
///    info must yield false on both sides (>= 1 required), true results are counted but not
///    required, and the true branch upgrades automatically on saves with a keeperless scheduled
///    show.
/// 4. (spec 1) keeper arrival alert state via [`assert_keeper_arrival_flagged_set_pass`], per
///    habitat per `scheduled` per pole (real first, then port): snapshot, call, counter +
///    flagged-set contract, restore - with the returned flagged sets additionally required to agree
///    between the poles, a direct real-vs-port set-granularity comparison the Stage 20 byte-level
///    test implies but never asserts.
/// 5. (spec 3, last - destructive-with-restore and therefore port-only, no way to run both poles
///    against one starting state, the Stage 23 smoke precedent)
///    [`ZTHabitat::remove_food_target_for_all`] over the phase-1 walks' first own food entity: the
///    zoo-wide targeting scan partitions every habitat's animals into targeting-F (this habitat:
///    must read target 0 after; other habitats: must be untouched - the function only walks its own
///    habitat's animals, so cross-habitat non-interference is a genuine integration check) and
///    others (all unchanged). F's teardown flag bytes (`+0x104`/`+0x105`/`+0x107`, what the real
///    `BFEntity::setVisible`/`setIsRemoved` base bodies write) are snapshot and restored, never
///    asserted - the vtable slots could resolve to an override writing more. Expected vacuous on a
///    save with no mid-eat animal - a skip note, not a failure (same shape as the Stage 35
///    strict-sum skip); upgrades automatically on saves with a targeting animal.
///
/// Non-vacuousness (fail loudly): at least one live habitat; at least one keeper food tile zoo-wide
/// (on a foodless zoo a wrong gate reads as 0 == 0); at least one leg-2 row with a non-empty own
/// pool; with a live keeper, at least one leg-4 oracle `+0x324` dispatch and at least one habitat
/// without an attached show info.
pub(crate) fn run_habitat_keeper_maintenance_flow_multi_reimpl_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_KEEPER_MAINTENANCE_FLOW_MULTI_REIMPL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let keeper_ptr = globals().ztworldmgr().entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) });

    // Shared phase-1 oracle walk: one pass over each habitat's own owned tiles, plus the reference
    // tile both pick kinds are driven with (the first animal's own tile, else the habitat's first
    // owned tile - the keeper-food pick tests' pattern).
    let mut tiles_walked = 0usize;
    let mut food_tiles_found = 0usize;
    let mut categories_seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut per_habitat: Vec<(usize, u32, KeeperFoodOracleList, u32)> = Vec::new();
    for (i, &ptr) in habitat_ptrs.iter().enumerate() {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let mut own_foods = KeeperFoodOracleList::new();
        keeper_food_oracle_walk(*habitat.owned_tiles_ptr(), &mut own_foods, &mut categories_seen, &mut tiles_walked, &mut food_tiles_found);
        let begin: u32 = get_from_memory(ptr + 0x6c);
        let end: u32 = get_from_memory(ptr + 0x70);
        let animal_tile = (0..(end.wrapping_sub(begin)) / 4)
            .map(|u| get_from_memory::<u32>(begin + u * 4))
            .find(|&a| a != 0)
            .map(|animal| (unsafe { BFENTITY_GET_TILE.original()(animal as *const u32) }) as u32)
            .unwrap_or(0);
        let ref_tile = if animal_tile != 0 {
            animal_tile
        } else {
            walk_tile_list(*habitat.owned_tiles_ptr())
                .map(|node| get_from_memory::<u32>(node + 0x8))
                .find(|&tile| tile != 0)
                .unwrap_or(0)
        };
        per_habitat.push((i, ptr, own_foods, ref_tile));
    }

    let mut failures: Vec<String> = Vec::new();

    // Leg 2 (spec 2) - nearest vs smallest over each habitat's own food piles.
    let mut nearest_smallest_rows = 0usize;
    let mut multi_pile_rows = 0usize;
    let mut nonempty_pool_rows = 0usize;
    let mut random_draws = 0usize;
    let mut random_pool_draws = 0usize;
    let mut random_empty_pool_draws = 0usize;
    let categories: Vec<u32> = categories_seen.iter().copied().collect();
    for (i, ptr, own_foods, ref_tile) in &per_habitat {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(*ptr) };
        for &category in &categories {
            let own_pool: Vec<u32> = own_foods.iter().filter(|&&(_, _, c)| c == category).map(|&(entity, _, _)| entity).collect();
            nearest_smallest_rows += 1;
            if !own_pool.is_empty() {
                nonempty_pool_rows += 1;
            }
            if own_pool.len() >= 2 {
                multi_pile_rows += 1;
            }
            let (oracle_smallest, _) = keeper_food_oracle_list_min(KeeperFoodPickKind::Smallest, own_foods, *ref_tile, category);
            let (oracle_nearest, _) = keeper_food_oracle_list_min(KeeperFoodPickKind::Nearest, own_foods, *ref_tile, category);
            let real_smallest =
                (unsafe { zthabitat::GET_SMALLEST_KEEPER_FOOD.original()(*ptr as *const u32, *ref_tile as *const u32, category, false) }) as u32;
            let real_nearest =
                (unsafe { zthabitat::GET_NEAREST_KEEPER_FOOD.original()(*ptr as *const u32, *ref_tile as *const u32, category, false) }) as u32;
            let port_smallest = habitat.get_smallest_keeper_food(*ref_tile, category, false);
            let port_nearest = habitat.get_nearest_keeper_food(*ref_tile, category, false);
            for (side, smallest, nearest) in [("real", real_smallest, real_nearest), ("port", port_smallest, port_nearest)] {
                if smallest != oracle_smallest {
                    failures.push(format!(
                        "habitat {} ({:#010x}) {}: getSmallestKeeperFood(category {category:#x}, subhabs=false) got {smallest:#010x}, expected oracle argmin {oracle_smallest:#010x}",
                        i, ptr, side
                    ));
                }
                if nearest != oracle_nearest {
                    failures.push(format!(
                        "habitat {} ({:#010x}) {}: getNearestKeeperFood(category {category:#x}, subhabs=false) got {nearest:#010x}, expected oracle argmin {oracle_nearest:#010x}",
                        i, ptr, side
                    ));
                }
            }
            if !own_pool.is_empty() {
                // Cross-getter contract (the spec's literal point): smallest minimizes the remaining
                // food units, nearest the geometric distance, so neither pick can lose to the other
                // on its own metric - asserted over both sides' picks.
                for (side, smallest, nearest) in [("real", real_smallest, real_nearest), ("port", port_smallest, port_nearest)] {
                    if smallest == 0 || nearest == 0 {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: null pick from a non-empty own pool (smallest {smallest:#010x}, nearest {nearest:#010x})",
                            i, ptr, side
                        ));
                        continue;
                    }
                    let smallest_amount: i32 = get_from_memory(smallest + 0x154);
                    let nearest_amount: i32 = get_from_memory(nearest + 0x154);
                    if smallest_amount > nearest_amount {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: smallest pick {smallest:#010x} (amount {smallest_amount}) holds more food than the nearest pick {nearest:#010x} (amount {nearest_amount})",
                            i, ptr, side
                        ));
                    }
                    let smallest_dist =
                        keeper_food_tile_dist_squared(*ref_tile, (unsafe { BFENTITY_GET_TILE.original()(smallest as *const u32) }) as u32);
                    let nearest_dist =
                        keeper_food_tile_dist_squared(*ref_tile, (unsafe { BFENTITY_GET_TILE.original()(nearest as *const u32) }) as u32);
                    if nearest_dist > smallest_dist {
                        failures.push(format!(
                            "habitat {} ({:#010x}) {}: nearest pick {nearest:#010x} (dist {nearest_dist}) is farther than the smallest pick {smallest:#010x} (dist {smallest_dist})",
                            i, ptr, side
                        ));
                    }
                }
            }

            // Stage 22's third function, membership level - 10 draws per side, both pool arms.
            for side in ["real", "port"] {
                for _ in 0..10 {
                    random_draws += 1;
                    let seed_before: u32 = get_from_memory(rng_addr);
                    let drawn = if side == "real" {
                        (unsafe { zthabitat::GET_RANDOM_KEEPER_FOOD.original()(*ptr as *const u32, *ref_tile as *const u32, category, false) }) as u32
                    } else {
                        habitat.get_random_keeper_food(*ref_tile, category, false)
                    };
                    if own_pool.is_empty() {
                        random_empty_pool_draws += 1;
                        let seed_after: u32 = get_from_memory(rng_addr);
                        if drawn != 0 || seed_after != seed_before {
                            failures.push(format!(
                                "habitat {} ({:#010x}) {}: empty own pool drew {drawn:#010x} with rng {seed_before:#010x} -> {seed_after:#010x}, expected null and an untouched seed",
                                i, ptr, side
                            ));
                        }
                    } else {
                        random_pool_draws += 1;
                        if !own_pool.contains(&drawn) {
                            failures.push(format!(
                                "habitat {} ({:#010x}) {}: draw {drawn:#010x} outside the own pool ({} piles)",
                                i, ptr, side,
                                own_pool.len()
                            ));
                        }
                    }
                }
            }
        }
    }

    // Leg 4 (spec 4) - the dirt-pile locator vs the oracle-min walk.
    let mut dirt_rows = 0usize;
    let mut dirt_filter_dispatches = 0usize;
    let mut dirt_null_gate_rows = 0usize;
    if let Some(keeper) = keeper_ptr {
        let ai_mgr = globals().ztaimgr_ptr() as u32;
        let keeper_tile = (unsafe { BFENTITY_GET_TILE.original()(keeper as *const u32) }) as u32;
        for (i, ptr, _, _) in &per_habitat {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(*ptr) };
            for check_can_see in [false, true] {
                dirt_rows += 1;
                let real = (unsafe { zthabitat::GET_NEAREST_DIRT_PILE.original()(*ptr as *const u32, keeper as *const u32, check_can_see) }) as u32;
                let port = habitat.get_nearest_dirt_pile(keeper, check_can_see);
                let oracle = if ai_mgr == 0 || keeper_tile == 0 {
                    // Both sides' own null early-outs (the port's documented guard chain head).
                    dirt_null_gate_rows += 1;
                    0
                } else {
                    keeper_dirt_pile_oracle_min(*ptr, keeper, keeper_tile, ai_mgr, check_can_see, &mut dirt_filter_dispatches)
                };
                if real != oracle {
                    failures.push(format!(
                        "habitat {} ({:#010x}), check_can_see={check_can_see}: real getNearestDirtPile {real:#010x} != oracle {oracle:#010x}",
                        i, ptr
                    ));
                }
                if port != oracle {
                    failures.push(format!(
                        "habitat {} ({:#010x}), check_can_see={check_can_see}: port get_nearest_dirt_pile {port:#010x} != oracle {oracle:#010x}",
                        i, ptr
                    ));
                }
            }
        }
    }

    // Leg 5 (spec 5) - needsShowKeeper decomposition + directional claims.
    let mut needs_show_rows = 0usize;
    let mut no_show_info_habitats = 0usize;
    let mut needs_show_true_results = 0usize;
    for (i, ptr, _, _) in &per_habitat {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(*ptr) };
        let real = low_byte_bool(unsafe { zthabitat::NEEDS_SHOW_KEEPER.original()(*ptr as *const u32, std::ptr::null()) });
        let port = habitat.needs_show_keeper(0);
        needs_show_rows += 1;
        if real || port {
            failures.push(format!(
                "habitat {} ({:#010x}): needsShowKeeper(null keeper) must be false on both sides - real {real}, port {port}",
                i, ptr
            ));
        }
    }
    if let Some(keeper) = keeper_ptr {
        let keeper_type_id = unsafe { call_entity_vtable_u32_noargs(get_from_memory::<u32>(keeper + 0x128), 0x20) };
        for (i, ptr, _, _) in &per_habitat {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(*ptr) };
            let show_info = habitat.zt_show_info_ptr;
            let expected = show_info != 0 && needs_keeper(show_info, keeper_type_id);
            let real = low_byte_bool(unsafe { zthabitat::NEEDS_SHOW_KEEPER.original()(*ptr as *const u32, keeper as *const u32) });
            let port = habitat.needs_show_keeper(keeper);
            needs_show_rows += 1;
            if real != port || port != expected {
                failures.push(format!(
                    "habitat {} ({:#010x}): needsShowKeeper disagreement - real {real}, port {port}, oracle {expected} (show_info {show_info:#010x}, keeper_type_id {keeper_type_id:#x})",
                    i, ptr
                ));
            }
            if expected {
                needs_show_true_results += 1;
            }
            if show_info == 0 {
                no_show_info_habitats += 1;
            }
        }
    }

    // Leg 1 (spec 1) - keeper arrival alert state, per habitat per scheduled per pole (real first,
    // then port, each pass restoring its snapshots), with the two poles' flagged sets required to
    // agree.
    let mut arrival_passes = 0usize;
    if let Some(keeper) = keeper_ptr {
        for (i, &ptr) in habitat_ptrs.iter().enumerate() {
            for scheduled in [false, true] {
                let real_set = assert_keeper_arrival_flagged_set_pass(&mut failures, "real", i, ptr, keeper, scheduled, || unsafe {
                    zthabitat::TRIGGER_KEEPER_ARRIVED.original()(ptr as *const u32, keeper as *const u32, scheduled)
                });
                let port_set = assert_keeper_arrival_flagged_set_pass(&mut failures, "port", i, ptr, keeper, scheduled, || {
                    unsafe { ref_from_memory::<ZTHabitat>(ptr) }.trigger_keeper_arrived(keeper, scheduled)
                });
                arrival_passes += 1;
                if real_set != port_set {
                    failures.push(format!(
                        "habitat {} ({:#010x}), scheduled={scheduled}: real and port flagged sets disagree - real {real_set:?} vs port {port_set:?}",
                        i, ptr
                    ));
                }
            }
        }
    }

    // Leg 3 (spec 3), last - destructive-with-restore, port-only. See the doc comment for the
    // leg's contract; F's teardown flags are restore-only, never asserted.
    let mut leg3_note = String::new();
    let food_target =
        per_habitat.iter().find_map(|(_, ptr, own_foods, _)| own_foods.first().map(|&(entity, _, _)| (*ptr, entity)));
    if let Some((food_habitat_ptr, food_entity)) = food_target {
        let mut targeting_here: Vec<u32> = Vec::new();
        let mut targeting_elsewhere: Vec<u32> = Vec::new();
        let mut others: Vec<(u32, u32)> = Vec::new();
        for &habitat_ptr in &habitat_ptrs {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
            for animal_ptr in habitat.get_all_animals(false) {
                let target = animal_food_target(animal_ptr);
                if target == food_entity {
                    if habitat_ptr == food_habitat_ptr {
                        targeting_here.push(animal_ptr);
                    } else {
                        targeting_elsewhere.push(animal_ptr);
                    }
                } else {
                    others.push((animal_ptr, target));
                }
            }
        }
        let leg3_targeting_animals = targeting_here.len() + targeting_elsewhere.len();
        let teardown_before: [u8; 3] = [
            get_from_memory(food_entity + 0x104),
            get_from_memory(food_entity + 0x105),
            get_from_memory(food_entity + 0x107),
        ];
        let removed = unsafe { mut_from_memory::<ZTHabitat>(food_habitat_ptr) }.remove_food_target_for_all(food_entity);
        if !removed {
            failures.push(format!("leg 3: remove_food_target_for_all({food_entity:#010x}) returned false, expected true"));
        }
        for &animal_ptr in &targeting_here {
            let target = animal_food_target(animal_ptr);
            if target != 0 {
                failures.push(format!(
                    "leg 3: animal {animal_ptr:#010x} in the food's own habitat still targets {target:#010x} after remove_food_target_for_all({food_entity:#010x})"
                ));
            }
        }
        for &animal_ptr in &targeting_elsewhere {
            let target = animal_food_target(animal_ptr);
            if target != food_entity {
                failures.push(format!(
                    "leg 3: cross-habitat animal {animal_ptr:#010x} target changed to {target:#010x} - remove_food_target_for_all must not touch other habitats' animals"
                ));
            }
        }
        for &(animal_ptr, before) in &others {
            let target = animal_food_target(animal_ptr);
            if target != before {
                failures.push(format!("leg 3: uninvolved animal {animal_ptr:#010x} target changed from {before:#010x} to {target:#010x}"));
            }
        }
        for (offset, before) in [(0x104u32, teardown_before[0]), (0x105, teardown_before[1]), (0x107, teardown_before[2])] {
            save_to_memory(food_entity + offset, before);
        }
        if leg3_targeting_animals == 0 {
            leg3_note = "; leg 3 vacuous on this save: no live animal with an active food target (the matched-animal branch has no save coverage - \
                         the Stage 23 smoke test skipped for the same reason; upgrades automatically on a save with a mid-eat animal)"
                .to_string();
        }
    } else {
        leg3_note = "; leg 3 skipped: no owned keeper-food entity found".to_string();
    }

    // Non-vacuousness guards (fail loudly, Stage 32/35 precedent).
    if food_tiles_found == 0 {
        failures.push(format!(
            "save contains no keeper food tiles - update the save (walked {} owned tiles across {} habitats; \
             a wrong gate or field offset would read as 0 == 0 here)",
            tiles_walked,
            per_habitat.len()
        ));
    }
    if nonempty_pool_rows == 0 {
        failures.push(format!(
            "non-vacuous assert failed: every leg-2 (habitat, category) row has an empty own pool across {} food tiles, categories {:?}",
            food_tiles_found, categories_seen
        ));
    }
    if let Some(keeper) = keeper_ptr {
        if dirt_filter_dispatches == 0 {
            failures.push(format!(
                "non-vacuous assert failed: the leg-4 oracle never dispatched the keeper vtable +0x324 filter across {} rows \
                 (keeper {keeper:#010x}) - the filter walk did not genuinely run",
                dirt_rows
            ));
        }
        if no_show_info_habitats == 0 {
            failures.push(format!(
                "non-vacuous assert failed: no habitat without an attached show info - the leg-5 false arm never ran against the live keeper {keeper:#010x}"
            ));
        }
    }

    if failures.is_empty() {
        let keeper_desc = if keeper_ptr.is_some() { "found" } else { "none (legs 1/4 skipped, leg 5 null-keeper arm only)" };
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, keeper: {}, owned tiles walked: {}, food tiles: {}, categories: {:?}, \
                 nearest/smallest rows: {} (multi-pile: {}), random draws: {} (non-empty-pool {}, empty-pool {}), \
                 dirt rows: {} (filter dispatches: {}, null-gate rows: {}), needs-show rows: {} (no-show-info habitats: {}, true results: {}), \
                 arrival passes: {}{}",
                test_name,
                habitat_ptrs.len(),
                keeper_desc,
                tiles_walked,
                food_tiles_found,
                categories_seen,
                nearest_smallest_rows,
                multi_pile_rows,
                random_draws,
                random_pool_draws,
                random_empty_pool_draws,
                dirt_rows,
                dirt_filter_dispatches,
                dirt_null_gate_rows,
                needs_show_rows,
                no_show_info_habitats,
                needs_show_true_results,
                arrival_passes,
                leg3_note
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

/// Leg 4's oracle-min walk for [`run_habitat_keeper_maintenance_flow_multi_reimpl_live_test`]:
/// replicates [`ZTHabitat::get_nearest_dirt_pile`]'s own documented gate chain, in order, over the
/// habitat's owned tiles - occupant non-null -> not in [`ZTHabitat::keeper_has_invalid_tile`]'s
/// exclusion list -> the keeper vtable `+0x324` entity-target filter (every dispatch counted - the
/// leg's non-vacuousness witness) -> the `tile+0x85 & 4` flag clear -> the `check_can_see` AI-mgr
/// vtable `+0x1c` dispatch - tracking the best candidate by [`keeper_food_tile_dist_squared`] with
/// the port's own `best_entity == 0 || dist < best_dist` shape. All dispatches are the read-only
/// predicates real vanilla's own `.original()` call makes internally on the same data.
fn keeper_dirt_pile_oracle_min(
    habitat_ptr: u32,
    keeper_ptr: u32,
    keeper_tile: u32,
    ai_mgr_ptr: u32,
    check_can_see: bool,
    filter_dispatches: &mut usize,
) -> u32 {
    let mut best_entity = 0u32;
    let mut best_dist = i32::MAX;
    for node in walk_tile_list(get_from_memory(habitat_ptr + 0x40)) {
        let tile_ptr = get_from_memory::<u32>(node + 0x8);
        let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
        if entity_ptr == 0 || ZTHabitat::keeper_has_invalid_tile(keeper_ptr, tile_ptr) {
            continue;
        }
        *filter_dispatches += 1;
        if !unsafe { call_vtable_slot_with_ptr_ret_bool(keeper_ptr, 0x324, entity_ptr) } {
            continue;
        }
        if get_from_memory::<u8>(tile_ptr + 0x85) & 4 != 0 {
            continue;
        }
        if check_can_see && !unsafe { call_vtable_slot_ptr_ptr_ptr_u32_ret_bool(ai_mgr_ptr, 0x1c, keeper_tile, tile_ptr, keeper_ptr, 0) } {
            continue;
        }
        let dist = keeper_food_tile_dist_squared(keeper_tile, tile_ptr);
        if best_entity == 0 || dist < best_dist {
            best_entity = entity_ptr;
            best_dist = dist;
        }
    }
    best_entity
}

/// One snapshot→call→assert→restore pass of Stage 36's leg-1 keeper-arrival flow for a single
/// (side, habitat, `scheduled`) combination, returning the flagged animal set it produced.
/// [`assert_trigger_keeper_arrived_pass`]'s snapshot discipline - each involved habitat (the habitat
/// itself plus every amphibious neighbor in [`walk_neighbor_tree`] order) has its
/// `scheduled_service_counter` (`+0xf4`) snapshotted, and every animal's keeper-arrives flag byte
/// (`+0x39c`) over each involved habitat's `+0x6c..+0x70` vector, no null-animal skip (faithful to
/// vanilla's loop) - but the alert state is asserted at *set* granularity rather than per-animal
/// byte (that contract stays with the Stage 20 test): the set of animals reading flag 1 post-call
/// must equal `{animals whose byte was already 1} ∪ {written animals whose own `canService` low
/// byte passes}`. `canService` is read-only, so assert-phase re-evaluation cannot perturb what the
/// pass wrote. Every snapshot is restored unconditionally (also on the failure path) so the other
/// pole and the live game see exactly the state found.
fn assert_keeper_arrival_flagged_set_pass(
    failures: &mut Vec<String>,
    side: &str,
    habitat_index: usize,
    habitat_ptr: u32,
    keeper_ptr: u32,
    scheduled: bool,
    call: impl FnOnce(),
) -> Vec<u32> {
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
                "habitat {} ({:#010x}) {}, scheduled={}: counter at {:#010x} is {actual}, expected {expected}",
                habitat_index, habitat_ptr, side, scheduled, addr
            ));
        }
    }
    let mut flagged_set: Vec<u32> = Vec::new();
    let mut expected_set: Vec<u32> = Vec::new();
    for &(animal_ptr, before, written) in &flag_snapshots {
        if get_from_memory::<u8>(animal_ptr + 0x39c) == 1 && !flagged_set.contains(&animal_ptr) {
            flagged_set.push(animal_ptr);
        }
        if before == 1 && !expected_set.contains(&animal_ptr) {
            expected_set.push(animal_ptr);
        }
        if written
            && !expected_set.contains(&animal_ptr)
            && low_byte_bool(unsafe { ZTANIMAL_CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) })
        {
            expected_set.push(animal_ptr);
        }
    }
    flagged_set.sort_unstable();
    expected_set.sort_unstable();
    if flagged_set != expected_set {
        failures.push(format!(
            "habitat {} ({:#010x}) {}, scheduled={}: flagged set {flagged_set:?} != expected set {expected_set:?}",
            habitat_index, habitat_ptr, side, scheduled
        ));
    }
    for &(addr, before) in &counter_snapshots {
        save_to_memory(addr, before);
    }
    for &(animal_ptr, before, _) in &flag_snapshots {
        save_to_memory(animal_ptr + 0x39c, before);
    }
    flagged_set
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

/// Multi-reimplementation integration test (stage 37 of
/// `openzt/plans/zthabitat-additional-functions-plan.md`, the plan's last): drives the Stage 26-31
/// reimplementations together over the live zoo's own habitats - global exhibit census, show
/// topology, portal transit - so a disagreement between two reimplementations of one side surfaces
/// as a failed cross-getter invariant rather than only a real-vs-reimpl diff. Real vanilla is called
/// before the reimplementation everywhere; legs run in order, the mutating leg last (Stage 36
/// convention):
///
/// 1. Global exhibit census (read-only): every `exhibit_array` habitat is classified as a show tank
///    three independent ways that must all agree - the raw-field oracle (vtable ==
///    [`ZTHabitat::TANK_VTABLE_PTR`] && the `+0x4` show-info dword nonzero, exactly
///    `ZTHABITATMGR_GET_NUM_NON_SHOW_NON_WORLD_HABITATS_LIVE`'s independent oracle, so a
///    vtable-identity bug cannot agree with itself), the port's [`ZTHabitat::is_tank`] &&
///    [`ZTHabitat::is_show_tank`], and real vanilla's own `isTank` vtable `+0x20` dispatch (the
///    un-hooked constant stubs [`ZTHABITAT_IS_TANK_LIVE`] dispatches) && the same raw `+0x4` read.
///    The census identity is then pinned: `getNumNonShowNonWorldHabitats` release-safe real pole ==
///    port == the oracle's non-show count, and non-show + show tanks == scanned ==
///    `exhibit_array().len()`.
/// 2. Show topology (read-only): for every ordered (habitat, target) pair plus a null target,
///    `isShowNeighbor` (release-safe real pole vs port) must equal the independent
///    [`walk_neighbor_tree`] membership oracle over the same `show_neighbors_head` tree the port
///    binary-searches - which pins literal truth for every oracle-member pair and literal falsity
///    for every non-member on both sides at once. No symmetry assertion between (a, b) and (b, a) -
///    the decompiles do not promise it; direction is covered by the oracle.
/// 3. Portal transit (synthetic, mutating-with-restore): the first zoo-wide animal found over the
///    raw `+0x6c..+0x70` vectors in `exhibit_array` order (never `get_all_animals`, which sorts in
///    place) is transitioned into each accepted portal state (`0x85`, `0x86`) with its destination
///    tile (`+0x234`) pointed at one of the target exhibit's own owned tiles. The target is the live
///    show tank from leg 1, and the tile is obtained through `getTilesCopy` itself - release-safe
///    real pole vs port vs a pre-call snapshot of the source list (both copies reproducing the
///    snapshot exactly pins them to each other too), so the Stage 28 port is exercised where the leg
///    genuinely needs it. An assumption check first resolves the chosen tile independently through
///    the habitat grid (`+0x34`/`+0x38` coordinates -> [`ZTHabitatMgr::get_habitat_ptr`]) so a wrong
///    tile cannot silently break the leg. Per state, `hasPortalAnimal` must answer literally true
///    for (source, target) on both sides, stay real == port across the full (habitat, target + null)
///    matrix, and stay literally false on every unrelated row ((source, u) with u != target and
///    (u, target) with u != source) that was already all-false at baseline. Both mutated fields are
///    snapshotted up front and restored after each state pass; after the final restore the source
///    row is re-run and must equal its recorded baseline row - restoration proven, not assumed. The
///    pass is synchronous single-threaded on the game thread (no tick can interleave), the same
///    reasoning as [`assert_baby_born_bonus_pass`]'s live mutation.
///
/// Spec deviations (all pre-declared): (a) "Transition an animal into portal state" is
/// synthetic-with-restore - real portal transit needs the un-ported goal/portal machinery and
/// interactive play to reach (same reasoning as Stage 36's deviation (d)), so exactly the two fields
/// the port's documented read path consumes (`animal+0x170` state, `animal+0x234` destination tile)
/// are written directly and every other byte of the animal is left alone. (b) "true between linked
/// tanks" is asserted as show-neighbor-set membership for every ordered pair, target-class-agnostic
/// (the live mutual pair need not be tank x tank). (c) The required list's `addToBuildingList`/
/// `additionalScenerySuitabilityChange` (Stage 30) appear in none of the spec's three assertions;
/// their real-vs-port fidelity stays with their own
/// `ZTHABITAT_ADD_TO_BUILDING_LIST_MATCHES_REAL_LIVE`/
/// `ZTHABITAT_ADDITIONAL_SCENERY_SUITABILITY_CHANGE_MATCHES_REAL_LIVE` tests - duplicating those
/// fresh-map/fresh-buffer fixture shapes here would add vanilla-heap teardown risk for zero new
/// invariants. `getTilesCopy` is folded in where the portal leg genuinely needs it (the destination
/// tile), `isTank` as leg 1's classification. (d) `HAS_PORTAL_ANIMAL` is detoured, so in release its
/// `.original()` pole re-enters the port (the established Stage 36 deviation-(f) note) and the
/// portal leg's agreement is port-vs-port there; the census/show-neighbor/tiles-copy legs use the
/// release-safe `_real` accessors and the `isTank` slot is un-hooked, so those stay real-vs-port in
/// both profiles. A save with no show tank at all logs an explicit summary note and falls back to
/// the first other habitat as the portal target rather than failing (the spec's "linked show tank"
/// wording degrades to "a different exhibit"); a save with no non-show habitat, no animal zoo-wide,
/// or a target exhibit with no owned tiles fails loudly instead - each would leave a whole assertion
/// class unexercised.
pub(crate) fn run_habitat_topology_show_census_multi_reimpl_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_TOPOLOGY_SHOW_CENSUS_MULTI_REIMPL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut habitat_ptrs: Vec<u32> = Vec::new();
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr != 0 {
            habitat_ptrs.push(ptr);
        }
    }
    if habitat_ptrs.is_empty() {
        let msg = "no live habitats found".to_string();
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();

    // Leg 1 (spec 1) - global exhibit census, read-only: three independent show-tank
    // classifications must agree per habitat, then the census identity across the manager getter,
    // its release-safe real pole, and the raw-field oracle.
    let mut show_tanks = 0usize;
    let mut non_show = 0usize;
    let mut first_show_tank = 0u32;
    for (i, &ptr) in habitat_ptrs.iter().enumerate() {
        let raw_is_show_tank = get_from_memory::<u32>(ptr) == ZTHabitat::TANK_VTABLE_PTR && get_from_memory::<u32>(ptr + 4) != 0;
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let port_is_show_tank = habitat.is_tank() && habitat.is_show_tank();
        let real_is_show_tank = unsafe { call_vtable_slot_noargs_ret_bool(ptr, 0x20) } && get_from_memory::<u32>(ptr + 4) != 0;
        if raw_is_show_tank {
            show_tanks += 1;
            if first_show_tank == 0 {
                first_show_tank = ptr;
            }
        } else {
            non_show += 1;
        }
        if port_is_show_tank != raw_is_show_tank || real_is_show_tank != raw_is_show_tank {
            failures.push(format!(
                "leg 1: habitat {} ({:#010x}) show-tank classification disagreement - raw-field oracle {}, port is_tank/is_show_tank {}, real +0x20 dispatch {}",
                i, ptr, raw_is_show_tank, port_is_show_tank, real_is_show_tank
            ));
        }
    }
    let real_non_show = hooks_zthabitatmgr::get_num_non_show_non_world_habitats_real(mgr_ptr);
    let port_non_show = habitat_mgr.get_num_non_show_non_world_habitats();
    if real_non_show != port_non_show {
        failures.push(format!("leg 1: real getNumNonShowNonWorldHabitats {real_non_show} != port {port_non_show}"));
    }
    if port_non_show as usize != non_show {
        failures.push(format!(
            "leg 1: port get_num_non_show_non_world_habitats {port_non_show} != raw-field oracle non-show count {non_show}"
        ));
    }
    if non_show + show_tanks != habitat_ptrs.len() || habitat_ptrs.len() != habitat_mgr.exhibit_array().len() {
        failures.push(format!(
            "leg 1: census identity broken: non-show {non_show} + show tanks {show_tanks} != scanned {} (exhibit_array len {})",
            habitat_ptrs.len(),
            habitat_mgr.exhibit_array().len()
        ));
    }
    if non_show == 0 {
        failures.push("non-vacuous assert failed: no non-show habitats in the loaded zoo".to_string());
    }
    let mut leg3_note = String::new();
    if show_tanks == 0 {
        leg3_note = "; no live show tank found - leg 3's portal target falls back to the first other habitat".to_string();
    }

    // Leg 2 (spec 2) - show topology, read-only: real == port == tree-walk membership oracle for
    // every ordered pair plus a null target (the oracle read is the same one
    // ZTHABITAT_IS_SHOW_NEIGHBOR_MATCHES_REAL_LIVE uses, so a descent bug cannot agree with itself).
    let mut show_neighbor_comparisons = 0usize;
    let mut populated_trees = 0usize;
    let mut true_hits = 0usize;
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let tree_members: Vec<u32> = walk_neighbor_tree(*habitat.show_neighbors_head())
            .map(|node| get_from_memory::<u32>(node + 0x10))
            .collect();
        if !tree_members.is_empty() {
            populated_trees += 1;
        }
        for &target_ptr in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
            let oracle = tree_members.contains(&target_ptr);
            let real = hooks_zthabitatmgr::is_show_neighbor_real(habitat_ptr as *const u32, target_ptr as *const u32);
            let port = habitat.is_show_neighbor(target_ptr);
            show_neighbor_comparisons += 1;
            if real != oracle || port != oracle {
                failures.push(format!(
                    "leg 2: habitat {habitat_ptr:#010x}, target {target_ptr:#010x}: show-neighbor membership disagreement - oracle {oracle}, real {real}, port {port}"
                ));
            }
            if oracle {
                true_hits += 1;
            }
        }
    }

    // Leg 3 (spec 3) - portal transit, synthetic and mutating-with-restore, last. Source = first
    // habitat whose raw all_animals vector yields a non-null animal; the raw field reads keep the
    // vector exactly as both poles' own walks see it (get_all_animals would sort it in place).
    let mut source_ptr = 0u32;
    let mut animal_ptr = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let begin: u32 = get_from_memory(habitat_ptr + 0x6c);
        let end: u32 = get_from_memory(habitat_ptr + 0x70);
        if let Some(found) = (0..(end.wrapping_sub(begin)) / 4).map(|u| get_from_memory::<u32>(begin + u * 4)).find(|&a| a != 0) {
            source_ptr = habitat_ptr;
            animal_ptr = found;
            break;
        }
    }
    let mut portal_states_exercised = 0usize;
    let mut portal_literal_trues = 0usize;
    let mut portal_literal_falses = 0usize;
    let mut baseline_comparisons = 0usize;
    let mut baseline_trues = 0usize;
    if animal_ptr == 0 {
        failures.push("leg 3: no live animal found in any habitat - nothing to transition into portal state".to_string());
    } else {
        // Baseline matrix, read-only: real == port for every (habitat, target + null) row, recorded
        // so the literal-false requirements apply only where the row was already all-false and the
        // post-restore proof has its reference.
        let mut baseline: std::collections::HashMap<(u32, u32), (bool, bool)> = std::collections::HashMap::new();
        for &habitat_ptr in &habitat_ptrs {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
            for &target_ptr in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
                let real = unsafe { zthabitat::HAS_PORTAL_ANIMAL.original()(habitat_ptr as *const u32, target_ptr as *const u32) };
                let port = habitat.has_portal_animal(target_ptr);
                baseline_comparisons += 1;
                if real {
                    baseline_trues += 1;
                }
                if real != port {
                    failures.push(format!(
                        "leg 3 baseline: habitat {habitat_ptr:#010x}, target {target_ptr:#010x}: real {real} != port {port}"
                    ));
                }
                baseline.insert((habitat_ptr, target_ptr), (real, port));
            }
        }

        // Portal target: the live show tank from leg 1, else the first other habitat with an
        // explicit summary note. If the target == the source (the animal lives in the show tank)
        // the structure is unchanged - "unrelated" simply means targets != target.
        let mut target_ptr = first_show_tank;
        if target_ptr == 0 {
            target_ptr = habitat_ptrs.iter().copied().find(|&ptr| ptr != source_ptr).unwrap_or(0);
        }
        if target_ptr == 0 {
            failures.push("leg 3: no portal target available (single-habitat zoo)".to_string());
        } else {
            // Destination tile through the Stage 28 port: snapshot the target's source list first,
            // then both sides' getTilesCopy into their own out-slots must reproduce it exactly; the
            // port copy's first payload is the tile the synthetic transit aims at. Every copy node
            // is vanilla-allocated - freed back through free_tile_list_copy, never Box.
            let target_habitat = unsafe { ref_from_memory::<ZTHabitat>(target_ptr) };
            let source_payloads: Vec<u32> = walk_tile_list(*target_habitat.owned_tiles_ptr())
                .map(|node| get_from_memory::<TileListNode>(node).payload)
                .collect();
            let mut dest_tile = 0u32;
            if source_payloads.is_empty() {
                failures.push(format!(
                    "leg 3: portal target {target_ptr:#010x} has no owned tiles - no destination tile to aim the synthetic transit at"
                ));
            } else {
                let mut real_out: u32 = 0;
                let real_out_addr = &mut real_out as *mut u32 as u32;
                let real_ret = hooks_zthabitatmgr::get_tiles_copy_real(target_ptr as *const u32, real_out_addr as *const i32) as u32;
                let mut port_out: u32 = 0;
                let port_out_addr = &mut port_out as *mut u32 as u32;
                let port_ret = target_habitat.get_tiles_copy(port_out_addr);
                if real_ret != real_out_addr {
                    failures.push(format!("leg 3: real getTilesCopy returned {real_ret:#010x}, expected out-param address {real_out_addr:#010x}"));
                }
                if port_ret != port_out_addr {
                    failures.push(format!("leg 3: port get_tiles_copy returned {port_ret:#010x}, expected out-param address {port_out_addr:#010x}"));
                }
                let real_payloads: Option<Vec<u32>> = if real_out != 0 {
                    Some(walk_tile_list(real_out).map(|node| get_from_memory::<TileListNode>(node).payload).collect())
                } else {
                    None
                };
                let port_payloads: Option<Vec<u32>> = if port_out != 0 {
                    Some(walk_tile_list(port_out).map(|node| get_from_memory::<TileListNode>(node).payload).collect())
                } else {
                    None
                };
                if real_out == 0 {
                    failures.push(format!("leg 3: real getTilesCopy produced a null sentinel for target {target_ptr:#010x}"));
                }
                if port_out == 0 {
                    failures.push(format!("leg 3: port get_tiles_copy produced a null sentinel for target {target_ptr:#010x}"));
                }
                if let (Some(real_payloads), Some(port_payloads)) = (&real_payloads, &port_payloads) {
                    if real_payloads != &source_payloads {
                        failures.push(format!("leg 3: real copy payloads {real_payloads:?} != source snapshot {source_payloads:?}"));
                    }
                    if port_payloads != &source_payloads {
                        failures.push(format!("leg 3: port copy payloads {port_payloads:?} != source snapshot {source_payloads:?}"));
                    }
                    dest_tile = port_payloads[0];
                }
                if real_out != 0 {
                    free_tile_list_copy(real_out);
                }
                if port_out != 0 {
                    free_tile_list_copy(port_out);
                }

                // Assumption check: resolve the destination tile independently through the habitat
                // grid so a wrong tile cannot silently break the leg's assertions.
                let resolved = if dest_tile != 0 {
                    let dest_x: i32 = get_from_memory(dest_tile + 0x34);
                    let dest_y: i32 = get_from_memory(dest_tile + 0x38);
                    habitat_mgr.get_habitat_ptr(dest_x, dest_y)
                } else {
                    0
                };
                if resolved != target_ptr {
                    failures.push(format!(
                        "leg 3: destination tile {dest_tile:#010x} resolves to habitat {resolved:#010x}, expected the portal target {target_ptr:#010x}"
                    ));
                    dest_tile = 0;
                }
            }

            if dest_tile != 0 {
                let state_before: u32 = get_from_memory(animal_ptr + 0x170);
                let tile_before: u32 = get_from_memory(animal_ptr + 0x234);
                for &state in &[0x85u32, 0x86] {
                    save_to_memory(animal_ptr + 0x170, state);
                    save_to_memory(animal_ptr + 0x234, dest_tile);
                    portal_states_exercised += 1;

                    // (source, target) must be literally true on both sides.
                    let source_habitat = unsafe { ref_from_memory::<ZTHabitat>(source_ptr) };
                    let real_hit = unsafe { zthabitat::HAS_PORTAL_ANIMAL.original()(source_ptr as *const u32, target_ptr as *const u32) };
                    let port_hit = source_habitat.has_portal_animal(target_ptr);
                    if !real_hit || !port_hit {
                        failures.push(format!(
                            "leg 3: state {state:#x}: hasPortalAnimal({source_ptr:#010x}, {target_ptr:#010x}) must be literally true with the synthetic transit in place - real {real_hit}, port {port_hit}"
                        ));
                    } else {
                        portal_literal_trues += 1;
                    }

                    // Full matrix real == port, plus literal falsity on every unrelated row that
                    // was already all-false at baseline (equality-only elsewhere).
                    for &matrix_self in &habitat_ptrs {
                        let habitat = unsafe { ref_from_memory::<ZTHabitat>(matrix_self) };
                        for &matrix_target in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
                            let real = unsafe { zthabitat::HAS_PORTAL_ANIMAL.original()(matrix_self as *const u32, matrix_target as *const u32) };
                            let port = habitat.has_portal_animal(matrix_target);
                            if real != port {
                                failures.push(format!(
                                    "leg 3: state {state:#x}: habitat {matrix_self:#010x}, target {matrix_target:#010x}: real {real} != port {port}"
                                ));
                            }
                            let unrelated = (matrix_self == source_ptr && matrix_target != target_ptr)
                                || (matrix_self != source_ptr && matrix_target == target_ptr);
                            if unrelated && matches!(baseline.get(&(matrix_self, matrix_target)), Some((false, false))) {
                                if real || port {
                                    failures.push(format!(
                                        "leg 3: state {state:#x}: unrelated row ({matrix_self:#010x}, {matrix_target:#010x}) must be literally false - real {real}, port {port}"
                                    ));
                                } else {
                                    portal_literal_falses += 1;
                                }
                            }
                        }
                    }

                    save_to_memory(animal_ptr + 0x170, state_before);
                    save_to_memory(animal_ptr + 0x234, tile_before);
                }

                // Restoration proof: the source row must read exactly its recorded baseline again.
                let source_habitat = unsafe { ref_from_memory::<ZTHabitat>(source_ptr) };
                for &matrix_target in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
                    let real = unsafe { zthabitat::HAS_PORTAL_ANIMAL.original()(source_ptr as *const u32, matrix_target as *const u32) };
                    let port = source_habitat.has_portal_animal(matrix_target);
                    let Some(&(baseline_real, baseline_port)) = baseline.get(&(source_ptr, matrix_target)) else {
                        failures.push(format!("leg 3: baseline missing the ({source_ptr:#010x}, {matrix_target:#010x}) row"));
                        continue;
                    };
                    if real != baseline_real || port != baseline_port {
                        failures.push(format!(
                            "leg 3: source row not restored after the final restore - ({source_ptr:#010x}, {matrix_target:#010x}): real {real} (baseline {baseline_real}), port {port} (baseline {baseline_port})"
                        ));
                    }
                }
            }
        }
    }

    if failures.is_empty() {
        write_success_line(
            failure_log,
            &format!(
                "{} (habitats: {}, show tanks: {}, non-show: {}, show-neighbor comparisons: {} (non-empty trees: {}, true hits: {}), \
                 baseline comparisons: {} (trues: {}), portal states exercised: {}, literal-true outcomes: {}, \
                 literal-false outcomes: {}{})",
                test_name,
                habitat_ptrs.len(),
                show_tanks,
                non_show,
                show_neighbor_comparisons,
                populated_trees,
                true_hits,
                baseline_comparisons,
                baseline_trues,
                portal_states_exercised,
                portal_literal_trues,
                portal_literal_falses,
                leg3_note
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
                unsafe { ref_from_memory::<ZTHabitat>(ptr) }.trigger_keeper_arrived(keeper_ptr, scheduled)
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
        unsafe { ref_from_memory::<ZTHabitat>(ptr) }.set_dirty_characteristics();
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

    unsafe { ref_from_memory::<ZTHabitat>(ptr) }.set_time_last_serviced(SENTINEL, true);
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

/// Fills `ptr`'s raw `+0x48`/`+0x4c` boundary-pair vector, which real vanilla readers
/// (`doTankCheck`, `updateAmphibiousNeighbors`, ...) consume but the reimplementation leaves empty.
/// Call before invoking a real reader on a live habitat.
fn sync_real_boundary_pairs(ptr: u32) {
    hooks_zthabitatmgr::create_edge_pairs_real(ptr as *const u32);
}

/// Reads the vector real vanilla `createEdgePairs` writes at `ptr`'s raw `+0x48`/`+0x4c` slots into a
/// plain `Vec` for [`run_habitat_create_edge_pairs_matches_real_live_test`]'s comparison. The
/// reimplementation writes [`ZTHabitat::boundary_tile_pairs`] instead and leaves these slots alone; the
/// buffer real vanilla allocates here is freed by `ZTHabitat::destruct` through the raw capacity slot.
fn real_raw_boundary_pairs(ptr: u32) -> Vec<(u32, u32)> {
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
/// habitat, called back-to-back: `createEdgePairs` fully rebuilds the pair list from
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

    hooks_zthabitatmgr::create_edge_pairs_real(ptr as *const u32);
    let real_pairs = real_raw_boundary_pairs(ptr);

    unsafe { ref_from_memory::<ZTHabitat>(ptr) }.create_edge_pairs();
    let reimpl_pairs = unsafe { ref_from_memory::<ZTHabitat>(ptr) }.boundary_tile_pairs();

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
        for (tile_a, tile_b) in habitat.boundary_tile_pairs() {
            crate::zthabitatmgr::ZTHabitatMgr::break_amphibious_connection(tile_a, tile_b);
        }
    }
    write_success_line(failure_log, test_name);
    false
}

/// Real-vs-port comparison of `ZTHabitatMgr::recalculate_deterioration`: the function zeroes every
/// habitat's `deterioration` then recomputes it from fence state, so it is independent of the starting
/// values. Records every habitat's `deterioration` (`+0x134`) and the manager's `+0x6c` flag after the
/// real call and after the port, and compares them.
pub(crate) fn run_zthabitatmgr_recalculate_deterioration_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_RECALCULATE_DETERIORATION_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let record = || -> (Vec<(u32, u32)>, u8) {
        let levels = (0..habitat_mgr.exhibit_array().len())
            .map(|i| habitat_mgr.exhibit_array().get_ptr(i))
            .filter(|&ptr| ptr != 0)
            .map(|ptr| (ptr, get_from_memory::<u32>(ptr + 0x134)))
            .collect();
        (levels, get_from_memory::<u8>(mgr_ptr as u32 + 0x6c))
    };

    unsafe { zthabitatmgr::RECALCULATE_DETERIORATION.original()(mgr_ptr) };
    let real = record();
    habitat_mgr.recalculate_deterioration();
    let port = record();

    let mut failures: Vec<String> = Vec::new();
    if real != port {
        failures.push(format!("real={:x?}, port={:x?}", real, port));
    }
    finish_test(test_name, failures, failure_log)
}

/// Real-vs-port comparison of `ZTHabitatMgr::mark_zoo_exterior`. The flood fill only clears the `0x40`
/// ("in zoo") bit at tile `+0x83`, and a loaded save has already had its exterior cleared, so the test
/// first sets `0x40` on every map tile (the pre-fill state), runs real, snapshots every tile's `+0x83`,
/// resets to that same pre-fill state, runs the port, snapshots again, compares, and finally restores every
/// tile's original byte - so the live zoo is left exactly as it was found.
pub(crate) fn run_zthabitatmgr_mark_zoo_exterior_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_MARK_ZOO_EXTERIOR_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;

    if habitat_mgr.get_zoo_entrance_tile_ptr() == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no zoo entrance tile set)", test_name));
        return false;
    }

    let tile_ptrs: Vec<u32> = (0..world.map_x_size)
        .flat_map(|x| (0..world.map_y_size).map(move |y| (x, y)))
        .map(|(x, y)| world.get_tile_ptr(x, y))
        .filter(|&t| t != 0)
        .collect();
    let original: Vec<u8> = tile_ptrs.iter().map(|&t| get_from_memory::<u8>(t + 0x83)).collect();
    let set_all_in_zoo = || {
        for (&t, &b) in tile_ptrs.iter().zip(&original) {
            save_to_memory::<u8>(t + 0x83, b | 0x40);
        }
    };
    let snapshot = || -> Vec<u8> { tile_ptrs.iter().map(|&t| get_from_memory::<u8>(t + 0x83)).collect() };

    set_all_in_zoo();
    let pre_fill = snapshot();
    hooks_zthabitatmgr::mark_zoo_exterior_real(mgr_ptr);
    let real_after = snapshot();

    set_all_in_zoo();
    habitat_mgr.mark_zoo_exterior();
    let port_after = snapshot();

    for (&t, &b) in tile_ptrs.iter().zip(&original) {
        save_to_memory::<u8>(t + 0x83, b);
    }

    let cleared = pre_fill.iter().zip(&real_after).filter(|(a, b)| a != b).count();
    let mut failures: Vec<String> = Vec::new();
    if real_after != port_after {
        let diff_count = real_after.iter().zip(&port_after).filter(|(r, p)| r != p).count();
        let first: Vec<String> = tile_ptrs
            .iter()
            .zip(real_after.iter().zip(&port_after))
            .filter(|(_, (r, p))| r != p)
            .take(8)
            .map(|(t, (r, p))| format!("tile {:#010x}: real={:#04x}, port={:#04x}", t, r, p))
            .collect();
        failures.push(format!("{} tiles differ; first: {}", diff_count, first.join(", ")));
    }
    if cleared == 0 {
        failures.push("real mark_zoo_exterior cleared no tiles from the all-in-zoo pre-fill state; test would be vacuous".to_string());
    }
    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (tiles: {}, exterior tiles cleared: {})", test_name, tile_ptrs.len(), cleared));
        false
    } else {
        finish_test(test_name, failures, failure_log)
    }
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

fn morph_exhibit_log_snapshot() -> Vec<(u32, u32, u32)> {
    MORPH_EXHIBIT_CALL_LOG.lock().map(|log| log.clone()).unwrap_or_default()
}

fn clear_morph_exhibit_log() {
    if let Ok(mut log) = MORPH_EXHIBIT_CALL_LOG.lock() {
        log.clear();
    }
}

/// `ZTHABITATMGR_CHECK_EXHIBIT_MORPH_MATCHES_REAL_LIVE` - compares the ported
/// `ZTHabitatMgr::check_exhibit_morph` boundary orchestrator against real vanilla over real in-map
/// east/south tile pairs from the loaded zoo. Each pole runs against the same independently computed
/// expectation (owners differ -> `[(a, tile, nbr)]`/`[(b, nbr, tile)]` for each side that is non-null
/// and not the "world" habitat, in vanilla's own argument order; owners equal -> empty), so a port and
/// real vanilla that both diverge the same way can't cancel out.
///
/// The morph calls themselves are observed through [`MORPH_EXHIBIT_CALL_LOG`], which the ported
/// `morph_exhibit` fills for both poles (real vanilla's own `morphExhibit` calls hit the `MORPH_EXHIBIT`
/// detour and forward into that same method). Probes are pre-screened so neither pole can execute a
/// real morph: `morph_exhibit` early-returns unless a habitat's `do_tank_check()` disagrees with its
/// `is_tank()`, so every probe's non-"world" owner is required to be tank-consistent first - a real
/// morph would destroy/recreate the habitat in place (same reasoning as the ZTHABITATMGR_FENCE_REPLACED
/// deferral note in battery.rs). Pairs with a null owner on either side are excluded too - real vanilla
/// dereferences a differing non-null owner's `+0x2c` flag unconditionally - that case is covered
/// synthetically by the host-safe unit tests instead.
pub(crate) fn run_check_exhibit_morph_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CHECK_EXHIBIT_MORPH_MATCHES_REAL_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as u32;
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let mut failures: Vec<String> = Vec::new();

    struct Probe {
        x: u32,
        y: u32,
        direction: u32,
        tile_ptr: u32,
        neighbour_ptr: u32,
    }

    // In-map east (2) / south (4) pairs only: an out-of-map neighbour resolves to null, and real
    // vanilla's own latent null-deref on such inputs must never be driven through the real pole.
    let mut both_real: Vec<Probe> = Vec::new();
    let mut one_world: Vec<Probe> = Vec::new();
    let mut same_owner: Vec<Probe> = Vec::new();
    let mut skipped_null_owner = 0usize;
    let mut skipped_morph_risk = 0usize;
    'scan: for x in 0..world.map_x_size {
        for y in 0..world.map_y_size {
            'dirs: for direction in [2u32, 4] {
                match direction {
                    2 if x + 1 >= world.map_x_size => continue,
                    4 if y + 1 >= world.map_y_size => continue,
                    _ => {}
                }
                if both_real.len() >= 8 && one_world.len() >= 8 && same_owner.len() >= 8 {
                    break 'scan;
                }
                let tile_ptr = world.get_tile_ptr(x, y);
                let neighbour_ptr = world.get_neighbour_ptr_raw(tile_ptr, direction);
                if tile_ptr == 0 || neighbour_ptr == 0 {
                    continue;
                }
                let tile = get_from_memory::<BFTile>(tile_ptr);
                let neighbour = get_from_memory::<BFTile>(neighbour_ptr);
                let owner_a = habitat_mgr.get_habitat_ptr(tile.pos.x, tile.pos.y);
                let owner_b = habitat_mgr.get_habitat_ptr(neighbour.pos.x, neighbour.pos.y);
                let probe = Probe { x, y, direction, tile_ptr, neighbour_ptr };
                if owner_a == owner_b {
                    if owner_a != 0 && same_owner.len() < 8 {
                        same_owner.push(probe);
                    }
                    continue;
                }
                if owner_a == 0 || owner_b == 0 {
                    skipped_null_owner += 1;
                    continue;
                }
                let a_is_world = unsafe { ref_from_memory::<ZTHabitat>(owner_a) }.unknown_flag_0x2c != 0;
                let b_is_world = unsafe { ref_from_memory::<ZTHabitat>(owner_b) }.unknown_flag_0x2c != 0;
                let bucket = if a_is_world != b_is_world { &mut one_world } else { &mut both_real };
                if bucket.len() >= 8 {
                    continue;
                }
                // `morph_exhibit` destroys/recreates a habitat only when its tank-consistency check
                // disagrees with what it is; require agreement on every side that would actually
                // receive a morph call so neither pole can mutate the live zoo.
                for owner in [owner_a, owner_b] {
                    let habitat = unsafe { ref_from_memory::<ZTHabitat>(owner) };
                    if habitat.unknown_flag_0x2c == 0 && habitat.do_tank_check() != habitat.is_tank() {
                        skipped_morph_risk += 1;
                        continue 'dirs;
                    }
                }
                bucket.push(probe);
            }
        }
    }

    let expected_calls = |tile_ptr: u32, neighbour_ptr: u32| -> Vec<(u32, u32, u32)> {
        let owner_of = |p: u32| -> u32 {
            if p == 0 {
                0
            } else {
                let t = get_from_memory::<BFTile>(p);
                habitat_mgr.get_habitat_ptr(t.pos.x, t.pos.y)
            }
        };
        let owner_a = owner_of(tile_ptr);
        let owner_b = owner_of(neighbour_ptr);
        let mut calls = Vec::new();
        if owner_a != owner_b {
            if owner_a != 0 && unsafe { ref_from_memory::<ZTHabitat>(owner_a) }.unknown_flag_0x2c == 0 {
                calls.push((owner_a, tile_ptr, neighbour_ptr));
            }
            if owner_b != 0 && unsafe { ref_from_memory::<ZTHabitat>(owner_b) }.unknown_flag_0x2c == 0 {
                calls.push((owner_b, neighbour_ptr, tile_ptr));
            }
        }
        calls
    };

    let mut checked = 0usize;
    for (kind, probes) in [("habitat-vs-habitat", &both_real), ("habitat-vs-world", &one_world), ("same-owner", &same_owner)] {
        for probe in probes {
            checked += 1;
            let label = format!("{} (dir {}, at {},{}): ", kind, probe.direction, probe.x, probe.y);
            let expected = expected_calls(probe.tile_ptr, probe.neighbour_ptr);

            clear_morph_exhibit_log();
            habitat_mgr.check_exhibit_morph(probe.tile_ptr, probe.direction);
            let port_calls = morph_exhibit_log_snapshot();

            clear_morph_exhibit_log();
            hooks_zthabitatmgr::check_exhibit_morph_real(mgr_ptr as *const u32, probe.tile_ptr as *const u32, probe.direction);
            let real_calls = morph_exhibit_log_snapshot();

            if port_calls != expected {
                failures.push(format!("{}reimpl produced {:?}, expected {:?}", label, port_calls, expected));
            }
            if real_calls != expected {
                failures.push(format!("{}real produced {:?}, expected {:?}", label, real_calls, expected));
            }
        }
    }
    clear_morph_exhibit_log();

    if checked == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no in-map boundary/same-owner pairs found)", test_name));
        return false;
    }

    if failures.is_empty() {
        // Coverage counts ride along in the success line (same pattern as
        // ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE's own guest-count suffix) so a vacuous same-owner-only
        // pass is visible from the log alone.
        write_success_line(
            failure_log,
            &format!(
                "{} ({} habitat-vs-habitat, {} habitat-vs-world, {} same-owner probes; {} skipped: {} null-owner, {} tank-mismatched owner)",
                test_name,
                both_real.len(),
                one_world.len(),
                same_owner.len(),
                skipped_null_owner + skipped_morph_risk,
                skipped_null_owner,
                skipped_morph_risk
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

/// Smoke test, no independent pole by design: `find_better_gates_for_neighbors` moves/places gate fences,
/// a mutator on the plan's excluded-by-design list (gate/fence mutators have no safe real-vs-port fixture).
/// Calls it with a null habitat and on every tank, asserting only that nothing crashes.
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

/// Smoke test, no independent pole by design: `update_gates` drains deferred gate placement/replacement
/// requests, a gate/fence mutator on the plan's excluded-by-design list. In a freshly loaded save the
/// pending-request fields are empty, so this only checks the call path.
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

/// Everything `ZTHabitat::recalculateCharacteristics` writes that a comparison can read back without
/// following allocator-owned pointers: the counters/flags/food tally, every persisted suitability record
/// (`+0x148` tree, key plus all `0x70` record bytes), and the condition words of every census animal.
fn recalculate_characteristics_snapshot(ptr: u32) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for off in [0x2d_u32, 0x2e, 0x2f, 0x30, 0x130, 0x131, 0x132] {
        out.push((format!("+{off:#x}"), get_from_memory::<u8>(ptr + off) as u32));
    }
    for off in (0x94_u32..=0xa4).step_by(4).chain((0xa8..0xe8).step_by(4)).chain([0xf0, 0xf8, 0x138]) {
        out.push((format!("+{off:#x}"), get_from_memory::<u32>(ptr + off)));
    }
    for (name, off) in [("all_animals", 0x6c_u32), ("building_list", 0x78), ("surrounding_species", 0x13c)] {
        let begin = get_from_memory::<u32>(ptr + off);
        let end = get_from_memory::<u32>(ptr + off + 4);
        out.push((format!("{name}.len"), (end - begin) / 4));
        for (i, addr) in (begin..end).step_by(4).enumerate() {
            out.push((format!("{name}[{i}]"), get_from_memory::<u32>(addr)));
        }
    }
    let all_begin = get_from_memory::<u32>(ptr + 0x6c);
    let all_end = get_from_memory::<u32>(ptr + 0x70);
    for animal in (all_begin..all_end).step_by(4).map(get_from_memory::<u32>) {
        for off in (0x298_u32..0x2a8).step_by(4) {
            out.push((format!("animal {animal:#010x}+{off:#x}"), get_from_memory::<u32>(animal + off)));
        }
    }
    for node in walk_neighbor_tree(get_from_memory::<u32>(ptr + 0x148)) {
        let key: i32 = get_from_memory(node + 0x10);
        for off in (0..0x70_u32).step_by(4) {
            let raw = get_from_memory::<u32>(node + 0x14 + off);
            // Bytes 0x6e/0x6f are struct padding.
            out.push((format!("record[{key}]+{off:#x}"), if off == 0x6c { raw & 0xffff } else { raw }));
        }
    }
    out
}

/// Sets every terrain-category condition bit (21-56, both the low `+0x298` and critical `+0x2a0` words) on
/// every animal of the habitat, so a recalculation that only ever sets those bits - instead of clearing
/// each category first, as vanilla does - leaves them behind and the real-vs-port diff catches it.
fn seed_stale_category_condition_bits(ptr: u32) {
    const CATEGORY_MASK: u64 = ((1u64 << 36) - 1) << 21;
    let all_begin = get_from_memory::<u32>(ptr + 0x6c);
    let all_end = get_from_memory::<u32>(ptr + 0x70);
    for animal in (all_begin..all_end).step_by(4).map(get_from_memory::<u32>) {
        for word in [0x298_u32, 0x2a0] {
            let value = get_from_memory::<u32>(animal + word) as u64 | (get_from_memory::<u32>(animal + word + 4) as u64) << 32;
            let seeded = value | CATEGORY_MASK;
            save_to_memory::<u32>(animal + word, seeded as u32);
            save_to_memory::<u32>(animal + word + 4, (seeded >> 32) as u32);
        }
    }
}

/// `ZTHABITAT_RECALCULATE_CHARACTERISTICS_MATCHES_REAL_LIVE`: over every real habitat, runs real vanilla
/// (`recalculate_characteristics_real`) and the port from the same dirty state and diffs
/// [`recalculate_characteristics_snapshot`], with every animal's terrain-category condition bits seeded
/// before each run ([`seed_stale_category_condition_bits`]). `unknown_flag_0x30` is reset before each run
/// since it is both an output and `checkEscapability`'s rate-limit input.
pub(crate) fn run_habitat_recalculate_characteristics_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_RECALCULATE_CHARACTERISTICS_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut records = 0usize;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        checked += 1;
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };

        seed_stale_category_condition_bits(ptr);
        save_to_memory::<u8>(ptr + 0x30, 0);
        save_to_memory::<u8>(ptr + 0x2d, 1);
        hooks_zthabitatmgr::recalculate_characteristics_real(ptr as *const u32);
        let real = recalculate_characteristics_snapshot(ptr);

        seed_stale_category_condition_bits(ptr);
        save_to_memory::<u8>(ptr + 0x30, 0);
        save_to_memory::<u8>(ptr + 0x2d, 1);
        habitat.recalculate_characteristics();
        let reimpl = recalculate_characteristics_snapshot(ptr);

        records += real.iter().filter(|(name, _)| name.starts_with("record[") && name.ends_with("+0x0")).count();
        if real.len() != reimpl.len() {
            failures.push(format!("habitat {} ({:#010x}): snapshot length real={}, reimpl={}", i, ptr, real.len(), reimpl.len()));
            continue;
        }
        for ((name, real_value), (_, reimpl_value)) in real.iter().zip(reimpl.iter()) {
            if real_value != reimpl_value {
                failures.push(format!("habitat {} ({:#010x}) {}: real={:#010x}, reimpl={:#010x}", i, ptr, name, real_value, reimpl_value));
            }
        }
    }

    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (habitats checked: {}, records compared: {})", test_name, checked, records));
        false
    } else {
        for msg in &failures {
            error!("{}: {}", test_name, msg);
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}
", test_name, failures.join("; ")).as_bytes());
        }
        true
    }
}

/// Real-vs-port comparison of `ZTHabitatMgr::get_next_fence_pair` (pure query: out-params are the
/// only thing it writes). For every habitat's own boundary tile-pairs and both `check_in_zoo` values,
/// runs each side on its own copy of the `(cand_a, cand_b)` out-params and compares the return value and
/// both out values. The port bails after 512 iterations; real vanilla has no such guard, but boundary
/// pairs of a placed habitat terminate in practice.
pub(crate) fn run_zthabitatmgr_get_next_fence_pair_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_GET_NEXT_FENCE_PAIR_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    let mut compared = 0;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for (tile_a, tile_b) in habitat.boundary_tile_pairs() {
            for check_in_zoo in [false, true] {
                let (mut real_a, mut real_b) = (tile_a, tile_b);
                let real = low_byte_bool(unsafe {
                    zthabitatmgr::GET_NEXT_FENCE_PAIR.original()(
                        mgr_ptr,
                        ptr as *const u32,
                        &mut real_a as *mut u32 as u32,
                        &mut real_b as *mut u32 as u32,
                        check_in_zoo,
                    )
                });
                let (mut port_a, mut port_b) = (tile_a, tile_b);
                let port = habitat_mgr.get_next_fence_pair(ptr, &mut port_a, &mut port_b, check_in_zoo);
                compared += 1;
                if real != port || real_a != port_a || real_b != port_b {
                    failures.push(format!(
                        "habitat {:#010x} start {:#010x}/{:#010x} check_in_zoo={}: real=({}, {:#010x}, {:#010x}), port=({}, {:#010x}, {:#010x})",
                        ptr, tile_a, tile_b, check_in_zoo, real, real_a, real_b, port, port_a, port_b
                    ));
                }
            }
        }
    }
    if compared == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no habitat boundary tile-pairs found)", test_name));
        return false;
    }
    finish_test(test_name, failures, failure_log)
}

/// Real-vs-port comparison of `ZTHabitatMgr::check_gate` over every habitat's boundary tile-pairs (both
/// orderings), comparing the returned status and the `out_cost` out-param. Needs a live `ZTKeeper`.
pub(crate) fn run_zthabitatmgr_check_gate_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_CHECK_GATE_MATCHES_REAL_LIVE";

    let keeper_ptr = globals().ztworldmgr().entity_array().find(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) });
    let Some(keeper_ptr) = keeper_ptr else {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    };

    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    let mut compared = 0;
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for (tile_a, tile_b) in habitat.boundary_tile_pairs() {
            for (a, b) in [(tile_a, tile_b), (tile_b, tile_a)] {
                for flag in [false, true] {
                    let mut real_cost = i32::MIN;
                    let real = unsafe {
                        zthabitatmgr::CHECK_GATE.original()(mgr_ptr, a as *const u32, b as *const u32, keeper_ptr as *const u32, &mut real_cost as *mut i32 as *const i32, flag)
                    };
                    let mut port_cost = i32::MIN;
                    let port = habitat_mgr.check_gate(a, b, keeper_ptr, &mut port_cost as *mut i32, flag);
                    compared += 1;
                    if real != port || real_cost != port_cost {
                        failures.push(format!(
                            "habitat {:#010x} tiles {:#010x}/{:#010x} flag={}: real=({}, cost {}), port=({}, cost {})",
                            ptr, a, b, flag, real, real_cost, port, port_cost
                        ));
                    }
                }
            }
        }
    }
    if compared == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no habitat boundary tile-pairs found)", test_name));
        return false;
    }
    finish_test(test_name, failures, failure_log)
}

/// Real-vs-port comparison of `ZTHabitatMgr::update_amphibious_neighbors_from_tile` /
/// `update_show_neighbors_from_tile` (the `_0` tile+direction overloads): for each habitat boundary
/// tile-pair, derives the direction from `tile_a` to `tile_b`, runs real, snapshots every habitat's
/// neighbor sets, runs the port, snapshots again, and compares. Both wrappers bottom out in the
/// clear-and-rederive `update_*_neighbors` worker, so the result is independent of the starting sets.
pub(crate) fn run_zthabitatmgr_update_neighbors_from_tile_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITATMGR_UPDATE_NEIGHBORS_FROM_TILE_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let mut failures: Vec<String> = Vec::new();
    let (mut probed, mut skipped) = (0, 0);
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        for (tile_a, tile_b) in habitat.boundary_tile_pairs() {
            let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a as i32, tile_b as i32) };
            if tile_a == 0 || tile_b == 0 || direction < 0 {
                continue;
            }
            let direction = direction as u32;

            // Real vanilla dereferences both resolved habitats' `+0x2c` with no null guard, so a
            // boundary pair whose outer tile is unowned would crash it - only probe fully-owned pairs.
            let (a_pos, b_pos) = (get_from_memory::<BFTile>(tile_a).pos, get_from_memory::<BFTile>(tile_b).pos);
            if habitat_mgr.get_habitat_ptr(a_pos.x, a_pos.y) == 0 || habitat_mgr.get_habitat_ptr(b_pos.x, b_pos.y) == 0 {
                skipped += 1;
                continue;
            }
            probed += 1;

            unsafe { zthabitatmgr::UPDATE_AMPHIBIOUS_NEIGHBORS_0.original()(mgr_ptr, tile_a as i32, direction) };
            let real_sets = snapshot_all_neighbor_sets();
            habitat_mgr.update_amphibious_neighbors_from_tile(tile_a, direction);
            let port_sets = snapshot_all_neighbor_sets();
            diff_neighbor_snapshots("update_amphibious_neighbors_from_tile", &real_sets, &port_sets, &mut failures);

            unsafe { zthabitatmgr::UPDATE_SHOW_NEIGHBORS_0.original()(mgr_ptr, tile_a as i32, direction) };
            let real_sets = snapshot_all_neighbor_sets();
            habitat_mgr.update_show_neighbors_from_tile(tile_a, direction);
            let port_sets = snapshot_all_neighbor_sets();
            diff_neighbor_snapshots("update_show_neighbors_from_tile", &real_sets, &port_sets, &mut failures);
        }
    }
    if probed == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no boundary pair with both tiles owned; {} unowned)", test_name, skipped));
        return false;
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTHABITAT_SEND_MAINT_WORKER_CLEANUP_EVENTS_MATCHES_REAL_LIVE` - the real call's only observable effect
/// is one `ZTHabitat::sendEvent` per qualifying scenery tile, so `send_event_recorder` intercepts that
/// address for the duration of the real call (via `hooks_zthabitatmgr::send_maint_worker_cleanup_events_real`,
/// the release-safe `_DETOUR.call()` path) and records `(habitat, event_id, category, tile)` instead of
/// delivering anything. The sequence is compared in walk order against
/// [`ZTHabitat::maint_worker_cleanup_tiles`], over every live habitat (tanks included - they must record
/// nothing). Coverage counters are logged so a save with no qualifying scenery is visibly comparison-only.
pub(crate) fn run_habitat_send_maint_worker_cleanup_events_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_SEND_MAINT_WORKER_CLEANUP_EVENTS_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let (mut habitats, mut tanks, mut total_sends) = (0u32, 0u32, 0u32);
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        habitats += 1;
        if habitat.is_tank() {
            tanks += 1;
        }

        crate::reimplementation_tests::send_event_recorder::begin_capture();
        hooks_zthabitatmgr::send_maint_worker_cleanup_events_real(ptr as *const u32);
        let recorded = crate::reimplementation_tests::send_event_recorder::end_capture();

        let expected: Vec<(u32, u16, u8, u32)> = habitat.maint_worker_cleanup_tiles().map(|tile| (ptr, 0x2730u16, 0x4du8, tile)).collect();
        total_sends += recorded.len() as u32;
        if recorded != expected {
            failures.push(format!("habitat {:#010x}: real sends {:x?} != reimpl plan {:x?}", ptr, recorded, expected));
        }
    }
    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (habitats: {}, tanks: {}, sends recorded: {})", test_name, habitats, tanks, total_sends));
        false
    } else {
        finish_test(test_name, failures, failure_log)
    }
}

// ---------------------------------------------------------------------------------------------------
// Viewing-area mutator comparisons (`removeFromAllVAs`, `pathRemoved`, `pathPlaced` x2)
//
// These mutate real structures (viewing-area tile vectors, habitat VA vectors, tile cell slots, tile
// flags) and free vanilla-allocated `ZTViewingArea`s, so each scenario runs twice - once through real
// vanilla, once through the port - from an identical, self-built starting state, and tears down after
// each run. The fixture habitat starts with no viewing areas and its probe tiles are untouched by any
// live viewing area (cell slots clear, in no VA's tile vector), so nothing pre-existing is ever mutated.
// Every VA created during a run is allocated by vanilla (`operator_new` + the real constructor) and freed
// only through `remove_viewing_area` (real destructor + `operator_delete`) - no cross-allocator frees.
// ---------------------------------------------------------------------------------------------------

/// Per-habitat VA vectors (as `(normalized va id, tile vector, +0x25, +0x4c)`), per-habitat `+0x2d`, and
/// each probe tile's 8 cached-VA cell slots plus `+0x85` flags. VA pointers not present in the baseline
/// are normalised to `0xffff_ff00 + n` by order of appearance, so two runs that allocate at different
/// addresses still compare equal.
#[derive(PartialEq, Debug)]
struct VaWorld {
    habitats: Vec<(u32, Vec<(u32, Vec<u32>, u8, u8)>, u8)>,
    tiles: Vec<(u32, [u32; 8], u8)>,
}

fn va_tile_vec(va_ptr: u32) -> Vec<u32> {
    let begin: u32 = get_from_memory(va_ptr + 0x40);
    let end: u32 = get_from_memory(va_ptr + 0x44);
    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
}

fn habitat_va_list(habitat_ptr: u32) -> Vec<u32> {
    let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
    (*habitat.viewing_areas_begin()..*habitat.viewing_areas_end()).step_by(4).map(get_from_memory::<u32>).collect()
}

fn all_habitat_ptrs() -> Vec<u32> {
    let habitat_mgr = globals().zthabitatmgr();
    (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect()
}

fn tile_cell_slots(tile_ptr: u32) -> [u32; 8] {
    let tile = get_from_memory::<BFTile>(tile_ptr);
    let mut slots = [0u32; 8];
    if let Some(cell) = globals().zthabitatmgr().get_habitat_cell_addr(tile.pos.x, tile.pos.y) {
        for (i, slot) in slots.iter_mut().enumerate() {
            *slot = get_from_memory(cell + 4 + i as u32 * 4);
        }
    }
    slots
}

fn snapshot_va_world(baseline_vas: &[u32], tiles: &[u32]) -> VaWorld {
    let mut new_ids: Vec<u32> = Vec::new();
    let mut normalise = |va: u32| -> u32 {
        if va == 0 || baseline_vas.contains(&va) {
            return va;
        }
        let idx = new_ids.iter().position(|&n| n == va).unwrap_or_else(|| {
            new_ids.push(va);
            new_ids.len() - 1
        });
        0xffff_ff00 + idx as u32
    };
    let habitats = all_habitat_ptrs()
        .into_iter()
        .map(|h| {
            let vas = habitat_va_list(h)
                .into_iter()
                .map(|va| (normalise(va), va_tile_vec(va), get_from_memory::<u8>(va + 0x25), get_from_memory::<u8>(va + 0x4c)))
                .collect();
            (h, vas, get_from_memory::<u8>(h + 0x2d))
        })
        .collect();
    let tile_states = tiles
        .iter()
        .map(|&t| {
            let mut slots = tile_cell_slots(t);
            for s in slots.iter_mut() {
                *s = normalise(*s);
            }
            (t, slots, get_from_memory::<u8>(t + 0x85))
        })
        .collect();
    VaWorld { habitats, tiles: tile_states }
}

/// A habitat with no viewing areas plus `tile_count` of its owned tiles that no live viewing area
/// references: in the zoo, all 8 cell slots clear, absent from every VA's tile vector, and none of the 4
/// cardinal neighbours a path tile (so `pathPlaced` finds no candidate and takes its new-VA branch).
fn find_va_fixture(tile_count: usize) -> Option<(u32, Vec<u32>)> {
    let habitat_mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let referenced: Vec<u32> = all_habitat_ptrs().into_iter().flat_map(habitat_va_list).flat_map(va_tile_vec).collect();
    for habitat_ptr in all_habitat_ptrs() {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        if *habitat.viewing_areas_begin() != *habitat.viewing_areas_end() {
            continue;
        }
        let tiles: Vec<u32> = walk_tile_list(*habitat.owned_tiles_ptr())
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
            .filter(|&t| unsafe { openzt_detour::generated::bftile::IS_IN_ZOO.original()(t as *const u32, 1) } & 0xff != 0)
            .filter(|&t| tile_cell_slots(t).iter().all(|&s| s == 0) && !referenced.contains(&t))
            .filter(|&t| {
                [0u32, 2, 4, 6].iter().all(|&dir| {
                    let n = world.get_neighbour_ptr_raw(t, dir);
                    n == 0 || get_from_memory::<u8>(n + 0x83) & 8 == 0
                })
            })
            .take(tile_count)
            .collect();
        let _ = habitat_mgr;
        if tiles.len() == tile_count {
            return Some((habitat_ptr, tiles));
        }
    }
    None
}

/// Runs `act(habitat, tiles, va)` on a freshly built fixture (`va` is a synthetic VA over `tiles` already
/// added to the habitat when `build_va` is set, else `0`) and returns the world snapshot afterward, then
/// tears every VA created during the run back down and restores the probe tiles' `+0x85` flags and every
/// habitat's `+0x2d` byte. `Err` if the baseline wasn't restored (the caller must treat the live state as
/// suspect and fail loudly).
fn run_va_scenario(habitat_ptr: u32, tiles: &[u32], build_va: bool, act: &dyn Fn(u32, &[u32], u32)) -> Result<(VaWorld, bool), String> {
    let baseline_vas: Vec<u32> = all_habitat_ptrs().into_iter().flat_map(habitat_va_list).collect();
    let baseline_world = snapshot_va_world(&baseline_vas, tiles);
    let saved_flags: Vec<(u32, u8)> = tiles.iter().map(|&t| (t, get_from_memory::<u8>(t + 0x85))).collect();
    let saved_dirty: Vec<(u32, u8)> = all_habitat_ptrs().into_iter().map(|h| (h, get_from_memory::<u8>(h + 0x2d))).collect();

    let va = if build_va {
        let va = unsafe { OPERATOR_NEW.original()(0x5c) } as u32;
        if va == 0 {
            return Err("operator_new failed".to_string());
        }
        unsafe { ztviewingarea::CONSTRUCTOR.original()(va as *const u32, habitat_ptr as *const std::ffi::c_void, tiles[0] as *const std::ffi::c_void) };
        for &t in &tiles[1..] {
            unsafe { ztviewingarea::ADD_TILE.original()(va as *const u32, t as *const std::ffi::c_void) };
        }
        unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.add_viewing_area(va);
        va
    } else {
        0
    };

    let pre_act = snapshot_va_world(&baseline_vas, tiles);
    act(habitat_ptr, tiles, va);
    let after = snapshot_va_world(&baseline_vas, tiles);
    // Whether `act` visibly changed anything relative to the state just before it ran (the synthetic VA
    // included), so a scenario where both sides silently do nothing is reported, not passed vacuously.
    let changed = after != pre_act;

    for h in all_habitat_ptrs() {
        for created in habitat_va_list(h).into_iter().filter(|v| !baseline_vas.contains(v)) {
            unsafe { ref_from_memory::<ZTHabitat>(h) }.remove_viewing_area(created);
        }
    }
    for (t, flags) in saved_flags {
        save_to_memory::<u8>(t + 0x85, flags);
    }
    for (h, dirty) in saved_dirty {
        save_to_memory::<u8>(h + 0x2d, dirty);
    }
    let restored = snapshot_va_world(&baseline_vas, tiles);
    if restored != baseline_world {
        return Err(format!("baseline not restored after teardown: before={:x?}, after={:x?}", baseline_world, restored));
    }
    Ok((after, changed))
}

/// Shared driver: runs `real` then `port` through [`run_va_scenario`] on the same fixture and compares the
/// post-act snapshots. `tile_count`/`build_va` pick the scenario.
fn compare_va_scenario(
    failure_log: &mut Option<std::fs::File>,
    test_name: &str,
    scenarios: &[(&str, usize, bool)],
    real: &dyn Fn(u32, &[u32], u32),
    port: &dyn Fn(u32, &[u32], u32),
) -> bool {
    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0;
    let mut effective = 0;
    for &(label, tile_count, build_va) in scenarios {
        let Some((habitat_ptr, tiles)) = find_va_fixture(tile_count) else {
            continue;
        };
        ran += 1;
        let real_after = run_va_scenario(habitat_ptr, &tiles, build_va, real);
        let port_after = run_va_scenario(habitat_ptr, &tiles, build_va, port);
        match (real_after, port_after) {
            (Ok((r, r_changed)), Ok((p, p_changed))) => {
                if r_changed {
                    effective += 1;
                }
                if r_changed != p_changed {
                    failures.push(format!("{label}: real changed state={r_changed}, port changed state={p_changed}"));
                }
                if r != p {
                    failures.push(format!("{label}: habitat {habitat_ptr:#010x} tiles {tiles:x?}: real={r:x?}, port={p:x?}"));
                }
            }
            (r, p) => failures.push(format!("{label}: scenario error: real={:?}, port={:?}", r.err(), p.err())),
        }
    }
    if ran == 0 {
        write_success_line(failure_log, &format!("{} (skipped: no VA-free habitat with clean probe tiles)", test_name));
        return false;
    }
    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (scenarios run: {}, state-changing: {})", test_name, ran, effective));
        false
    } else {
        finish_test(test_name, failures, failure_log)
    }
}

/// `ZTHabitat::removeFromAllVAs`: a synthetic VA over one tile (removal empties it, so the VA itself is
/// destroyed and the vector shifted) and over two tiles (the VA survives with one tile left).
pub(crate) fn run_habitat_remove_from_all_vas_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_va_scenario(
        failure_log,
        "ZTHABITAT_REMOVE_FROM_ALL_VAS_MATCHES_REAL_LIVE",
        &[("one-tile VA emptied", 1, true), ("two-tile VA shrinks", 2, true)],
        &|h, tiles, _| hooks_zthabitatmgr::remove_from_all_vas_real(h as *const u32, tiles[0] as i32),
        &|h, tiles, _| unsafe { ref_from_memory::<ZTHabitat>(h) }.remove_from_all_vas(tiles[0]),
    )
}

/// `ZTHabitatMgr::pathRemoved` over the same two synthetic-VA scenarios (cached-slot `removeTile` pass,
/// then `removeFromAllVAs` on every habitat).
pub(crate) fn run_zthabitatmgr_path_removed_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    compare_va_scenario(
        failure_log,
        "ZTHABITATMGR_PATH_REMOVED_MATCHES_REAL_LIVE",
        &[("one-tile VA emptied", 1, true), ("two-tile VA shrinks", 2, true)],
        &|_, tiles, _| hooks_zthabitatmgr::zthabitatmgr_path_removed_real(mgr_ptr, tiles[0] as i32),
        &|_, tiles, _| globals().zthabitatmgr().path_removed(tiles[0]),
    )
}

/// `ZTHabitat::pathPlaced` on a tile with no path-tile neighbours: no candidate, so both sides take the
/// "construct a brand-new `ZTViewingArea` and append it" branch. The candidate-extension branches need a
/// real path network and are not covered (see the plan's "no live test, by design" note).
pub(crate) fn run_habitat_path_placed_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    compare_va_scenario(
        failure_log,
        "ZTHABITAT_PATH_PLACED_MATCHES_REAL_LIVE",
        &[("no candidates: new VA", 1, false)],
        &|h, tiles, _| hooks_zthabitatmgr::zthabitat_path_placed_real(h as *const u32, tiles[0] as *const std::ffi::c_void),
        &|h, tiles, _| unsafe { ref_from_memory::<ZTHabitat>(h) }.path_placed(tiles[0]),
    )
}

/// `ZTHabitatMgr::pathPlaced`: same no-candidate scenario, through the manager's 5x5 habitat scan - every
/// distinct non-world habitat within 2 tiles of the probe tile gets its own new VA on both sides.
pub(crate) fn run_zthabitatmgr_path_placed_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    compare_va_scenario(
        failure_log,
        "ZTHABITATMGR_PATH_PLACED_MATCHES_REAL_LIVE",
        &[("no candidates: new VAs", 1, false)],
        &|_, tiles, _| hooks_zthabitatmgr::zthabitatmgr_path_placed_real(mgr_ptr, tiles[0] as *const u32),
        &|_, tiles, _| globals().zthabitatmgr().path_placed(tiles[0]),
    )
}

/// `std::set<ZTHabitat*>` header + `{head*, size}` container backing a standalone neighbour-set fixture.
/// The header node and container live in Rust-owned memory (vanilla never frees a header); every real
/// node is allocated and freed through `PoolAlloc` on both sides, so no cross-allocator hazard.
struct SetFixture {
    header: Box<[u32; 5]>,
    container: Box<[u32; 2]>,
}

impl SetFixture {
    fn new() -> Self {
        let mut header = Box::new([0u32; 5]);
        let head = header.as_ptr() as u32;
        header[2] = head; // _Left (leftmost)
        header[3] = head; // _Right (rightmost)
        let container = Box::new([head, 0]);
        SetFixture { header, container }
    }

    fn container_addr(&self) -> u32 {
        self.container.as_ptr() as u32
    }

    fn head(&self) -> u32 {
        self.header.as_ptr() as u32
    }
}

/// Compares two tree shapes node for node (colour, key, children), returning the node count or a
/// description of the first mismatch. Parent links are checked against the walk's own call stack.
fn compare_tree_shapes(a: u32, a_parent: u32, b: u32, b_parent: u32) -> Result<u32, String> {
    if a == 0 || b == 0 {
        return if a == b { Ok(0) } else { Err("one side has a child the other lacks".to_string()) };
    }
    let (ka, kb): (u32, u32) = (get_from_memory(a + 0x10), get_from_memory(b + 0x10));
    let (ca, cb): (u8, u8) = (get_from_memory(a), get_from_memory(b));
    if ka != kb || ca != cb {
        return Err(format!("node mismatch: key {ka:#x}/{kb:#x}, colour {ca}/{cb}"));
    }
    if get_from_memory::<u32>(a + 4) != a_parent || get_from_memory::<u32>(b + 4) != b_parent {
        return Err(format!("parent link broken at key {ka:#x}"));
    }
    let left = compare_tree_shapes(get_from_memory(a + 8), a, get_from_memory(b + 8), b)?;
    let right = compare_tree_shapes(get_from_memory(a + 0xc), a, get_from_memory(b + 0xc), b)?;
    Ok(1 + left + right)
}

/// `ZTHABITAT_NEIGHBOR_SET_INSERT_CLEAR_MATCHES_REAL`: drives the port's `rb_set_insert`/`rb_tree_clear`
/// (behind `addAmphibiousNeighbor`/`addShowNeighbor`/`clear*Neighbors`) against real vanilla's generic
/// `std::set<ptr>::insert` (`AI_cls_0x404fd6::meth_0x5b355e`, `0x005b355e`, `thiscall(container, out*,
/// key*)`, `RET 8`) on two standalone sets fed the same ascending, descending and pseudo-random key
/// sequences (with duplicates). After every insert the inserted flag and the whole tree shape (colours,
/// keys, links, header min/max, size) must agree; afterwards the port clears both and the empty header
/// shape is checked. Vanilla's header is never touched by the allocator, and every node on both sides
/// comes from and returns to `PoolAlloc`.
pub(crate) fn run_neighbor_set_insert_clear_matches_real_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_insert, rb_tree_clear};
    let test_name = "ZTHABITAT_NEIGHBOR_SET_INSERT_CLEAR_MATCHES_REAL";
    type VanillaInsert = unsafe extern "thiscall" fn(u32, *mut u32, *const u32) -> u32;
    let vanilla_insert: VanillaInsert = unsafe { std::mem::transmute::<usize, VanillaInsert>(0x005b355e) };

    let mut sequences: Vec<(&str, Vec<u32>)> = Vec::new();
    sequences.push(("ascending", (1..=64).map(|i| 0x1000 + i * 0x10).collect()));
    sequences.push(("descending", (1..=64).rev().map(|i| 0x1000 + i * 0x10).collect()));
    let mut state = 0x2545_f491u32;
    let random: Vec<u32> = (0..300)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            0x1000 + ((state >> 16) % 96) * 0x10
        })
        .collect();
    sequences.push(("random-with-duplicates", random));

    let mut failures: Vec<String> = Vec::new();
    let mut total_inserts = 0usize;
    for (label, keys) in &sequences {
        let port = SetFixture::new();
        let real = SetFixture::new();
        for (index, &key) in keys.iter().enumerate() {
            let port_inserted = rb_set_insert(port.container_addr(), key);
            let mut out = [0u32; 2];
            unsafe { vanilla_insert(real.container_addr(), out.as_mut_ptr(), &key as *const u32) };
            let real_inserted = (out[1] & 0xff) != 0;
            total_inserts += 1;
            if port_inserted != real_inserted {
                failures.push(format!("{label}[{index}] key {key:#x}: inserted port={port_inserted} real={real_inserted}"));
                break;
            }
            let (port_size, real_size): (u32, u32) = (port.container[1], real.container[1]);
            let root_p: u32 = get_from_memory(port.head() + 4);
            let root_r: u32 = get_from_memory(real.head() + 4);
            match compare_tree_shapes(root_p, port.head(), root_r, real.head()) {
                Ok(count) if count == port_size && port_size == real_size => {}
                Ok(count) => {
                    failures.push(format!("{label}[{index}]: node count {count} vs size port={port_size} real={real_size}"));
                    break;
                }
                Err(msg) => {
                    failures.push(format!("{label}[{index}] key {key:#x}: {msg}"));
                    break;
                }
            }
            let min_max = |fx: &SetFixture| -> (u32, u32) {
                let l: u32 = get_from_memory(fx.head() + 8);
                let r: u32 = get_from_memory(fx.head() + 0xc);
                (get_from_memory(l + 0x10), get_from_memory(r + 0x10))
            };
            if min_max(&port) != min_max(&real) {
                failures.push(format!("{label}[{index}]: header min/max keys differ port={:?} real={:?}", min_max(&port), min_max(&real)));
                break;
            }
        }
        // The walker used by every production reader must see the same sorted, unique keys.
        let port_keys: Vec<u32> = walk_neighbor_tree(port.head()).map(|n| get_from_memory::<u32>(n + 0x10)).collect();
        let real_keys: Vec<u32> = walk_neighbor_tree(real.head()).map(|n| get_from_memory::<u32>(n + 0x10)).collect();
        if port_keys != real_keys || !port_keys.windows(2).all(|w| w[0] < w[1]) {
            failures.push(format!("{label}: in-order walk differs or is not strictly increasing"));
        }
        for (side, fx) in [("port", &port), ("real", &real)] {
            rb_tree_clear(fx.container_addr(), 0x14);
            let head = fx.head();
            if fx.container[1] != 0 || get_from_memory::<u32>(head + 4) != 0 || get_from_memory::<u32>(head + 8) != head || get_from_memory::<u32>(head + 0xc) != head {
                failures.push(format!("{label}: {side} set not in empty shape after rb_tree_clear"));
            }
        }
    }
    let _ = total_inserts;
    finish_test(test_name, failures, failure_log)
}

/// `ZTHABITAT_NEIGHBOR_SET_ERASE_MATCHES_REAL`: the port's `rb_set_erase` (behind
/// `removeAmphibiousNeighbor`) against real vanilla `removeAmphibiousNeighbor` (`0x00507a90`, reached
/// through the release-safe `remove_amphibious_neighbor_real`). Vanilla reads its set from `this+8`
/// (head) / `this+0xc` (size), so the real side's fixture is a fake four-word habitat whose `+8`/`+0xc`
/// words are that pair. Both sides are built with `rb_set_insert` (shape-checked against vanilla's
/// insert by the sibling test), then erased key by key in ascending, descending and pseudo-random
/// orders - including absent keys, a null key and repeated keys - down to empty. After every erase the
/// return value (non-null key => true), size, header min/max and the whole tree shape must agree.
pub(crate) fn run_neighbor_set_erase_matches_real_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_erase, rb_set_insert, rb_tree_clear};
    let test_name = "ZTHABITAT_NEIGHBOR_SET_ERASE_MATCHES_REAL";

    let mut state = 0x9e37_79b9u32;
    let mut next_random = move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state >> 16
    };
    let ascending: Vec<u32> = (1..=64).map(|i| 0x1000 + i * 0x10).collect();
    let descending: Vec<u32> = ascending.iter().rev().copied().collect();
    let shuffled = |rng: &mut dyn FnMut() -> u32| {
        let mut keys = ascending.clone();
        for i in (1..keys.len()).rev() {
            keys.swap(i, rng() as usize % (i + 1));
        }
        keys
    };
    let mut scenarios: Vec<(&str, Vec<u32>, Vec<u32>)> = vec![
        ("ascending/ascending", ascending.clone(), ascending.clone()),
        ("ascending/descending", ascending.clone(), descending.clone()),
        ("descending/ascending", descending.clone(), ascending.clone()),
    ];
    for round in 0..4 {
        let insert_order = shuffled(&mut next_random);
        let mut erase_order = shuffled(&mut next_random);
        // Sprinkle absent keys, a null key and repeats among the real ones.
        for n in 0..12u32 {
            let position = next_random() as usize % (erase_order.len() + 1);
            let extra = match n % 3 {
                0 => 0x2000 + n * 0x10,
                1 => 0,
                _ => erase_order[next_random() as usize % erase_order.len()],
            };
            erase_order.insert(position, extra);
        }
        let label: &'static str = ["random/random A", "random/random B", "random/random C", "random/random D"][round];
        scenarios.push((label, insert_order, erase_order));
    }

    let mut failures: Vec<String> = Vec::new();
    let mut erases = 0usize;
    'scenarios: for (label, insert_order, erase_order) in &scenarios {
        let port = SetFixture::new();
        let real_set = SetFixture::new();
        let mut fake_habitat = Box::new([0u32; 4]);
        fake_habitat[2] = real_set.head();
        let real_container = fake_habitat.as_ptr() as u32 + 8;
        for &key in insert_order {
            rb_set_insert(port.container_addr(), key);
            rb_set_insert(real_container, key);
        }
        for (index, &key) in erase_order.iter().enumerate() {
            rb_set_erase(port.container_addr(), key);
            let real_ret = hooks_zthabitatmgr::remove_amphibious_neighbor_real(fake_habitat.as_ptr(), key as *const u32);
            erases += 1;
            if (real_ret & 0xff != 0) != (key != 0) {
                failures.push(format!("{label}[{index}] key {key:#x}: real return {real_ret:#x}"));
                break 'scenarios;
            }
            let (port_size, real_size) = (port.container[1], fake_habitat[3]);
            let root_p: u32 = get_from_memory(port.head() + 4);
            let root_r: u32 = get_from_memory(real_set.head() + 4);
            match compare_tree_shapes(root_p, port.head(), root_r, real_set.head()) {
                Ok(count) if count == port_size && port_size == real_size => {}
                Ok(count) => {
                    failures.push(format!("{label}[{index}] key {key:#x}: node count {count} vs size port={port_size} real={real_size}"));
                    break 'scenarios;
                }
                Err(msg) => {
                    failures.push(format!("{label}[{index}] key {key:#x}: {msg}"));
                    break 'scenarios;
                }
            }
            let ends = |head: u32| -> (u32, u32) {
                let (min, max): (u32, u32) = (get_from_memory(head + 8), get_from_memory(head + 0xc));
                if min == head { (0, 0) } else { (get_from_memory(min + 0x10), get_from_memory(max + 0x10)) }
            };
            if ends(port.head()) != ends(real_set.head()) {
                failures.push(format!("{label}[{index}] key {key:#x}: header min/max differ port={:?} real={:?}", ends(port.head()), ends(real_set.head())));
                break 'scenarios;
            }
        }
        for (side, container, head) in [("port", port.container_addr(), port.head()), ("real", real_container, real_set.head())] {
            if get_from_memory::<u32>(container + 4) != 0 {
                failures.push(format!("{label}: {side} set not empty after erasing every key"));
            }
            rb_tree_clear(container, 0x14);
            if get_from_memory::<u32>(head + 4) != 0 {
                failures.push(format!("{label}: {side} root not null after clear"));
            }
        }
    }
    finish_test(&format!("{} (erases compared: {}, scenarios: {})", test_name, erases, scenarios.len()), failures, failure_log)
}

/// `ZTHABITAT_NEIGHBOR_QUERIES_MATCH_REAL_LIVE`: real vanilla vs port for `isAmphibiousNeighbor`,
/// `getSurroundingAnimals` and `getCloseOutsideTile` on every habitat in the loaded zoo.
/// - `isAmphibiousNeighbor` over every habitat pointer plus null, also against a [`walk_neighbor_tree`]
///   membership oracle.
/// - `getSurroundingAnimals`: identical element lists and identical buffer capacity; both buffers are
///   `PoolAlloc` output and are released with [`free_event_vector_buffer`].
/// - `getCloseOutsideTile`: the shared game RNG is rewound to the same seed before each side; the
///   returned tile and the RNG state afterwards must agree.
/// Coverage counters are logged so a save without amphibious neighbours or outside tiles is visibly
/// comparison-only.
pub(crate) fn run_habitat_neighbor_queries_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_NEIGHBOR_QUERIES_MATCH_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    if habitat_ptrs.is_empty() {
        return finish_test(test_name, vec!["no live habitats found".to_string()], failure_log);
    }
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;

    let mut failures: Vec<String> = Vec::new();
    let (mut neighbor_hits, mut nonempty_surrounding, mut outside_tiles) = (0u32, 0u32, 0u32);
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };

        let members: Vec<u32> = walk_neighbor_tree(*habitat.amphibious_neighbors_head()).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for &target in habitat_ptrs.iter().chain(std::iter::once(&0u32)) {
            let real = hooks_zthabitatmgr::is_amphibious_neighbor_real(habitat_ptr as *const std::ffi::c_void, target);
            let port = habitat.is_amphibious_neighbor(target);
            if real != port || members.contains(&target) != real {
                failures.push(format!("habitat {habitat_ptr:#010x} target {target:#010x}: isAmphibiousNeighbor real={real} port={port} oracle={}", members.contains(&target)));
            }
            neighbor_hits += real as u32;
        }

        let mut real_vec = [0u32; 3];
        let mut port_vec = [0u32; 3];
        hooks_zthabitatmgr::get_surrounding_animals_real(habitat_ptr as *const u32, real_vec.as_mut_ptr());
        habitat.get_surrounding_animals(port_vec.as_mut_ptr() as u32);
        let read_vec = |v: &[u32; 3]| -> Vec<u32> { (v[0]..v[1]).step_by(4).map(get_from_memory::<u32>).collect() };
        let (real_animals, port_animals) = (read_vec(&real_vec), read_vec(&port_vec));
        if real_animals != port_animals || real_vec[2] - real_vec[0] != port_vec[2] - port_vec[0] || (real_vec[0] == 0) != (port_vec[0] == 0) {
            failures.push(format!(
                "habitat {habitat_ptr:#010x}: getSurroundingAnimals real={real_animals:x?} (cap {}) port={port_animals:x?} (cap {})",
                real_vec[2] - real_vec[0],
                port_vec[2] - port_vec[0]
            ));
        }
        nonempty_surrounding += !real_animals.is_empty() as u32;
        for v in [real_vec, port_vec] {
            free_event_vector_buffer(v[0], v[2] - v[0]);
        }

        let seed: u32 = get_from_memory(rng_addr);
        let real_tile = hooks_zthabitatmgr::get_close_outside_tile_real(habitat_ptr as *const std::ffi::c_void);
        let real_seed: u32 = get_from_memory(rng_addr);
        save_to_memory(rng_addr, seed);
        let port_tile = habitat.get_close_outside_tile() as i32;
        let port_seed: u32 = get_from_memory(rng_addr);
        if real_tile != port_tile || real_seed != port_seed {
            failures.push(format!(
                "habitat {habitat_ptr:#010x}: getCloseOutsideTile real={real_tile:#x} (seed {real_seed:#x}) port={port_tile:#x} (seed {port_seed:#x})"
            ));
        }
        outside_tiles += (real_tile != 0) as u32;
    }
    let summary = format!(
        "{} (habitats: {}, amphibious-neighbor hits: {}, non-empty surrounding-animal lists: {}, habitats with an outside tile: {})",
        test_name,
        habitat_ptrs.len(),
        neighbor_hits,
        nonempty_surrounding,
        outside_tiles
    );
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_NEEDS_SERVICE_MATCHES_REAL_LIVE`: real vanilla (vanilla-only mode, so its neighbour
/// recursion stays vanilla) vs port for `needsService` over every live keeper x live habitat x
/// `include_neighbors`, comparing the low-byte result. Variants: the habitat's `+0xec` last-serviced
/// timestamp (live, `0`, a far-future-clock-minus-one value) is rewritten and restored so the timing arm
/// is covered both ways, and empty amphibious sets are seeded with every other habitat for the neighbour
/// search. Counters log true/false results.
pub(crate) fn run_needs_service_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_insert, rb_tree_clear};
    let test_name = "ZTHABITAT_NEEDS_SERVICE_MATCHES_REAL_LIVE";
    let keepers: Vec<u32> = globals().ztworldmgr().entity_array().filter(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) }).take(6).collect();
    if keepers.is_empty() {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    }
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    let ai_clock: u32 = get_from_memory(globals().ztaimgr_ptr() as u32 + 0xec);

    let mut failures: Vec<String> = Vec::new();
    let (mut calls, mut true_results) = (0u32, 0u32);
    let mut compare = |habitat_ptr: u32, keeper: u32, include_neighbors: bool, context: &str, failures: &mut Vec<String>| {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let real = hooks_zthabitatmgr::needs_service_real(habitat_ptr as *const u32, keeper as *const u32, include_neighbors);
        let port = habitat.needs_service(keeper, include_neighbors);
        if real != port {
            failures.push(format!("{context} habitat {habitat_ptr:#010x} keeper {keeper:#010x} neighbors={include_neighbors}: real {real} != port {port}"));
        }
        calls += 1;
        true_results += real as u32;
    };

    for &habitat_ptr in &habitat_ptrs {
        let live_timestamp: u32 = get_from_memory(habitat_ptr + 0xec);
        for (label, timestamp) in [("live", live_timestamp), ("never-serviced", 0), ("just-serviced", ai_clock)] {
            save_to_memory(habitat_ptr + 0xec, timestamp);
            for &keeper in &keepers {
                for include_neighbors in [false, true] {
                    compare(habitat_ptr, keeper, include_neighbors, label, &mut failures);
                }
            }
        }
        save_to_memory(habitat_ptr + 0xec, live_timestamp);
    }

    let mut seeded = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        if get_from_memory::<u32>(habitat_ptr + 0xc) != 0 {
            continue;
        }
        for &other in habitat_ptrs.iter().filter(|&&p| p != habitat_ptr) {
            rb_set_insert(habitat_ptr + 0x8, other);
        }
        seeded += 1;
        for &keeper in &keepers {
            compare(habitat_ptr, keeper, true, "seeded", &mut failures);
        }
        rb_tree_clear(habitat_ptr + 0x8, 0x14);
    }
    let summary = format!(
        "{} (keepers: {}, habitats: {}, calls: {}, true results: {}, seeded habitats: {})",
        test_name,
        keepers.len(),
        habitat_ptrs.len(),
        calls,
        true_results,
        seeded
    );
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_GET_NUM_ANIMALS_BY_SPECIES_MATCHES_REAL_LIVE`: real vanilla (vanilla-only mode) vs port for
/// `getNumAnimals(species_id, include_neighbors)` over every live habitat x (each species id present plus
/// one absent id) x both flags, with empty amphibious sets seeded with every other habitat for a second
/// pass. The absent-id call default-inserts a record, so a repeat pass over the same ids must leave the
/// cache's size (`+0x14c`) unchanged.
pub(crate) fn run_get_num_animals_by_species_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_insert, rb_tree_clear};
    let test_name = "ZTHABITAT_GET_NUM_ANIMALS_BY_SPECIES_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();

    let mut failures: Vec<String> = Vec::new();
    let (mut calls, mut nonzero) = (0u32, 0u32);
    let mut compare = |habitat_ptr: u32, species_id: i32, include_neighbors: bool, context: &str, failures: &mut Vec<String>| {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let real = hooks_zthabitatmgr::get_num_animals_by_species_real(habitat_ptr as *const u32, species_id, include_neighbors);
        let port = habitat.get_num_animals_by_species(species_id, include_neighbors);
        if real != port {
            failures.push(format!("{context} habitat {habitat_ptr:#010x} species {species_id} neighbors={include_neighbors}: real {real} != port {port}"));
        }
        calls += 1;
        nonzero += (real != 0) as u32;
    };

    let species_for = |habitat_ptr: u32| -> Vec<i32> {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let mut ids: Vec<i32> = Vec::new();
        for animal_ptr in habitat.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let id: i32 = get_from_memory(animal_type_ptr + 0x1ec);
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.push(i32::MAX);
        ids
    };
    let cache_size = |habitat_ptr: u32| -> u32 { get_from_memory(habitat_ptr + 0x14c) };

    for &habitat_ptr in &habitat_ptrs {
        for species_id in species_for(habitat_ptr) {
            for include_neighbors in [false, true] {
                compare(habitat_ptr, species_id, include_neighbors, "live", &mut failures);
            }
        }
        let size_a = cache_size(habitat_ptr);
        for species_id in species_for(habitat_ptr) {
            compare(habitat_ptr, species_id, false, "repeat", &mut failures);
        }
        if cache_size(habitat_ptr) != size_a {
            failures.push(format!("habitat {habitat_ptr:#010x}: cache size changed on repeat lookups ({size_a} -> {})", cache_size(habitat_ptr)));
        }
    }

    let mut seeded = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        if get_from_memory::<u32>(habitat_ptr + 0xc) != 0 {
            continue;
        }
        for &other in habitat_ptrs.iter().filter(|&&p| p != habitat_ptr) {
            rb_set_insert(habitat_ptr + 0x8, other);
        }
        seeded += 1;
        for species_id in species_for(habitat_ptr) {
            compare(habitat_ptr, species_id, true, "seeded", &mut failures);
        }
        rb_tree_clear(habitat_ptr + 0x8, 0x14);
    }
    let summary = format!("{} (habitats: {}, calls: {}, nonzero results: {}, seeded habitats: {})", test_name, habitat_ptrs.len(), calls, nonzero, seeded);
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_GET_RANDOM_HUNGRY_ANIMAL_MATCHES_REAL_LIVE`: real vanilla (vanilla-only mode, so its
/// neighbour recursion stays vanilla) vs port for `getRandomHungryAnimal` over every live keeper x live
/// habitat x `include_neighbors` x `home_habitat` in `{null, self, each other habitat}`. The shared RNG
/// state is reset to the same seed before each side; the returned animal and the RNG state afterwards must
/// match (one step iff the candidate list was non-empty). A second pass seeds empty amphibious sets with
/// every other habitat so the neighbour search runs. Counters log how many calls returned an animal and how
/// many consumed an RNG step.
pub(crate) fn run_get_random_hungry_animal_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_insert, rb_tree_clear, GAME_RNG_RVA};
    let test_name = "ZTHABITAT_GET_RANDOM_HUNGRY_ANIMAL_MATCHES_REAL_LIVE";
    let keepers: Vec<u32> = globals().ztworldmgr().entity_array().filter(|&ptr| unsafe { entity_type_matches(ptr, RVA_KEEPER_TYPE_CHECK_ARG) }).take(6).collect();
    if keepers.is_empty() {
        write_success_line(failure_log, &format!("{} (skipped: no live ZTKeeper found)", test_name));
        return false;
    }
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
    let seed: u32 = get_from_memory(rng_addr);

    let mut failures: Vec<String> = Vec::new();
    let (mut calls, mut returned, mut rng_steps) = (0u32, 0u32, 0u32);
    let mut compare = |habitat_ptr: u32, keeper: u32, include_neighbors: bool, home: u32, context: &str, failures: &mut Vec<String>| {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        save_to_memory(rng_addr, seed);
        let real = hooks_zthabitatmgr::get_random_hungry_animal_real(habitat_ptr as *const u32, keeper as *const u32, include_neighbors, home as *const u32) as u32;
        let real_rng: u32 = get_from_memory(rng_addr);
        save_to_memory(rng_addr, seed);
        let port = habitat.get_random_hungry_animal(keeper, include_neighbors, home);
        let port_rng: u32 = get_from_memory(rng_addr);
        if real != port || real_rng != port_rng {
            failures.push(format!(
                "{context} habitat {habitat_ptr:#010x} keeper {keeper:#010x} neighbors={include_neighbors} home {home:#010x}: real {real:#x} (rng {real_rng:#x}) != port {port:#x} (rng {port_rng:#x})"
            ));
        }
        calls += 1;
        returned += (real != 0) as u32;
        rng_steps += (real_rng != seed) as u32;
    };

    for &keeper in &keepers {
        for &habitat_ptr in &habitat_ptrs {
            for include_neighbors in [false, true] {
                for home in [0, habitat_ptr].into_iter().chain(habitat_ptrs.iter().copied().filter(|&p| p != habitat_ptr)) {
                    compare(habitat_ptr, keeper, include_neighbors, home, "live", &mut failures);
                }
            }
        }
    }

    let mut seeded = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        if get_from_memory::<u32>(habitat_ptr + 0xc) != 0 {
            continue;
        }
        for &other in habitat_ptrs.iter().filter(|&&p| p != habitat_ptr) {
            rb_set_insert(habitat_ptr + 0x8, other);
        }
        seeded += 1;
        for &keeper in &keepers {
            for home in [0, habitat_ptr] {
                compare(habitat_ptr, keeper, true, home, "seeded", &mut failures);
            }
        }
        rb_tree_clear(habitat_ptr + 0x8, 0x14);
    }
    save_to_memory(rng_addr, seed);
    let summary = format!(
        "{} (keepers: {}, habitats: {}, calls: {}, returned an animal: {}, consumed an rng step: {}, seeded habitats: {})",
        test_name,
        keepers.len(),
        habitat_ptrs.len(),
        calls,
        returned,
        rng_steps,
        seeded
    );
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_GENERATE_FACES_MATCHES_REAL_LIVE`: real vanilla vs port for `generateFaces` over every live
/// habitat x every live animal type x `other_habitat` in `{null, each live habitat}` x both `smile`
/// values. The per-animal `FUN_004d9a2f` requests are recorded rather than executed
/// (`generate_faces_recorder::begin_animal_face_capture`); the return value and the ordered request log
/// must match. A second pass seeds the amphibious set of each land habitat that has none with every other
/// live habitat (then clears it) so the neighbour recursion runs. Real runs in
/// `generate_faces_real`'s vanilla-only mode, so its recursion stays vanilla too.
pub(crate) fn run_generate_faces_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::generate_faces_recorder as recorder;
    use crate::zthabitat::support::{rb_set_insert, rb_tree_clear};
    let test_name = "ZTHABITAT_GENERATE_FACES_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    if habitat_ptrs.is_empty() {
        return finish_test(test_name, vec!["no live habitats found".to_string()], failure_log);
    }
    let mut species_types: Vec<u32> = Vec::new();
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        for animal in habitat.get_all_animals(false).collect::<Vec<u32>>() {
            let entity_type: u32 = get_from_memory(animal + 0x128);
            if entity_type != 0 && !species_types.contains(&entity_type) {
                species_types.push(entity_type);
            }
        }
    }
    if species_types.is_empty() {
        return finish_test(test_name, vec!["no live animals found".to_string()], failure_log);
    }

    let mut failures: Vec<String> = Vec::new();
    let (mut calls, mut matched_calls, mut recorded_requests) = (0u32, 0u32, 0usize);
    let mut compare = |habitat_ptr: u32, species: u32, smile: bool, other: u32, context: &str, failures: &mut Vec<String>| {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        recorder::begin_animal_face_capture();
        let real_result = hooks_zthabitatmgr::generate_faces_real(habitat_ptr as *const u32, species as *const u32, smile, other as *const u32);
        let real_log = recorder::end_animal_face_capture();
        recorder::begin_animal_face_capture();
        let port_result = habitat.generate_faces(species, smile, other);
        let port_log = recorder::end_animal_face_capture();
        if real_result != port_result || real_log != port_log {
            failures.push(format!(
                "{context} habitat {habitat_ptr:#010x} species {species:#010x} smile {smile} other {other:#010x}: real {real_result} {real_log:x?} != port {port_result} {port_log:x?}"
            ));
        }
        calls += 1;
        matched_calls += real_result as u32;
        recorded_requests += real_log.len();
    };

    for &habitat_ptr in &habitat_ptrs {
        for &species in &species_types {
            for smile in [true, false] {
                for other in std::iter::once(0).chain(habitat_ptrs.iter().copied()) {
                    compare(habitat_ptr, species, smile, other, "live", &mut failures);
                }
            }
        }
    }

    let mut seeded = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let is_tank = unsafe { crate::zthabitat::support::call_vtable_slot_noargs_ret_bool(habitat_ptr, 0x20) };
        if is_tank || get_from_memory::<u32>(habitat_ptr + 0xc) != 0 {
            continue;
        }
        for &other in habitat_ptrs.iter().filter(|&&p| p != habitat_ptr) {
            rb_set_insert(habitat_ptr + 0x8, other);
        }
        seeded += 1;
        for &species in &species_types {
            for smile in [true, false] {
                compare(habitat_ptr, species, smile, 0, "seeded", &mut failures);
            }
        }
        rb_tree_clear(habitat_ptr + 0x8, 0x14);
    }
    let summary = format!(
        "{} (habitats: {}, species types: {}, calls: {}, matched: {}, recorded requests: {}, seeded land habitats: {})",
        test_name,
        habitat_ptrs.len(),
        species_types.len(),
        calls,
        matched_calls,
        recorded_requests,
        seeded
    );
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_GET_HABITAT_RATING_MATCHES_REAL_LIVE`: real vanilla vs port for `getHabitatRating` over
/// every live habitat x up to 40 live animals x both `include_neighbors` values, comparing the `f32` bit
/// pattern. A second pass seeds the amphibious set (`+0x8`) of each tank that has none with every other
/// live habitat (then clears it again) so the max-over-neighbours branch is covered, plus the null
/// animal arm.
pub(crate) fn run_get_habitat_rating_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_insert, rb_tree_clear};
    let test_name = "ZTHABITAT_GET_HABITAT_RATING_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    if habitat_ptrs.is_empty() {
        return finish_test(test_name, vec!["no live habitats found".to_string()], failure_log);
    }
    let mut animals: Vec<u32> = Vec::new();
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        animals.extend(habitat.get_all_animals(false).collect::<Vec<u32>>());
    }
    animals.truncate(40);
    if animals.is_empty() {
        return finish_test(test_name, vec!["no live animals found".to_string()], failure_log);
    }

    let mut failures: Vec<String> = Vec::new();
    let mut calls = 0u32;
    let mut compare = |habitat_ptr: u32, animal: u32, include_neighbors: bool, context: &str, failures: &mut Vec<String>| {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let real = hooks_zthabitatmgr::get_habitat_rating_real(habitat_ptr as *const u32, animal as i32, include_neighbors as i8);
        let port = habitat.get_habitat_rating(animal, include_neighbors);
        if real.to_bits() != port.to_bits() {
            failures.push(format!(
                "{context} habitat {habitat_ptr:#010x} animal {animal:#010x} neighbors={include_neighbors}: getHabitatRating real={real} ({:#x}) port={port} ({:#x})",
                real.to_bits(),
                port.to_bits()
            ));
        }
        calls += 1;
    };

    for &habitat_ptr in &habitat_ptrs {
        for &animal in animals.iter().chain(std::iter::once(&0)) {
            for include_neighbors in [false, true] {
                compare(habitat_ptr, animal, include_neighbors, "live", &mut failures);
            }
        }
    }

    let mut seeded_tanks = 0u32;
    for &habitat_ptr in &habitat_ptrs {
        let is_tank = unsafe { crate::zthabitat::support::call_vtable_slot_noargs_ret_bool(habitat_ptr, 0x20) };
        if !is_tank || get_from_memory::<u32>(habitat_ptr + 0xc) != 0 {
            continue;
        }
        for &other in habitat_ptrs.iter().filter(|&&p| p != habitat_ptr) {
            rb_set_insert(habitat_ptr + 0x8, other);
        }
        seeded_tanks += 1;
        for &animal in &animals {
            compare(habitat_ptr, animal, true, "seeded", &mut failures);
        }
        rb_tree_clear(habitat_ptr + 0x8, 0x14);
    }
    let summary = format!("{} (habitats: {}, calls: {}, seeded tanks: {})", test_name, habitat_ptrs.len(), calls, seeded_tanks);
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_MOST_SUITABLE_HABITAT_MATCHES_REAL_LIVE`: real vanilla vs port for `getMostSuitableHabitat`
/// (every live habitat x up to 40 live animals) and `findBestRating` (standalone sets of live habitats -
/// all, tanks only, land only and each singleton - for every sampled animal, both `bool` values). The
/// returned habitat pointer must agree. Counters log how many results came from a neighbour branch and
/// how many `findBestRating` winners were tanks vs land habitats.
pub(crate) fn run_most_suitable_habitat_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_insert, rb_tree_clear};
    let test_name = "ZTHABITAT_MOST_SUITABLE_HABITAT_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    if habitat_ptrs.is_empty() {
        return finish_test(test_name, vec!["no live habitats found".to_string()], failure_log);
    }
    let mut animals: Vec<u32> = Vec::new();
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        animals.extend(habitat.get_all_animals(false).collect::<Vec<u32>>());
    }
    animals.truncate(40);
    if animals.is_empty() {
        return finish_test(test_name, vec!["no live animals found".to_string()], failure_log);
    }

    let mut failures: Vec<String> = Vec::new();
    let (mut neighbor_results, mut most_suitable_calls) = (0u32, 0u32);
    for &habitat_ptr in &habitat_ptrs {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        for &animal in &animals {
            let real = hooks_zthabitatmgr::get_most_suitable_habitat_real(habitat_ptr as *const u32, animal as *const u32) as u32;
            let port = habitat.get_most_suitable_habitat(animal);
            if real != port {
                failures.push(format!("habitat {habitat_ptr:#010x} animal {animal:#010x}: getMostSuitableHabitat real={real:#x} port={port:#x}"));
            }
            neighbor_results += (real != habitat_ptr) as u32;
            most_suitable_calls += 1;
        }
    }

    let is_tank = |ptr: u32| unsafe { crate::zthabitat::support::call_vtable_slot_noargs_ret_bool(ptr, 0x20) };
    let mut subsets: Vec<Vec<u32>> = vec![habitat_ptrs.clone()];
    subsets.push(habitat_ptrs.iter().copied().filter(|&p| is_tank(p)).collect());
    subsets.push(habitat_ptrs.iter().copied().filter(|&p| !is_tank(p)).collect());
    subsets.extend(habitat_ptrs.iter().map(|&p| vec![p]));
    let (mut rating_calls, mut tank_winners, mut land_winners) = (0u32, 0u32, 0u32);
    for subset in subsets.iter().filter(|s| !s.is_empty()) {
        let set = SetFixture::new();
        for &member in subset {
            rb_set_insert(set.container_addr(), member);
        }
        for &animal in &animals {
            for is_show_set in [false, true] {
                let real = hooks_zthabitatmgr::find_best_rating_real(animal as *const u32, set.container_addr() as *const i32, is_show_set) as u32;
                let port = ZTHabitat::find_best_rating(animal, set.container_addr(), is_show_set);
                if real != port {
                    failures.push(format!("set of {} animal {animal:#010x} show={is_show_set}: findBestRating real={real:#x} port={port:#x}", subset.len()));
                }
                rating_calls += 1;
                if is_tank(real) {
                    tank_winners += 1;
                } else {
                    land_winners += 1;
                }
            }
        }
        rb_tree_clear(set.container_addr(), 0x14);
    }
    let summary = format!(
        "{} (getMostSuitableHabitat calls: {}, neighbour results: {}, findBestRating calls: {}, tank winners: {}, land winners: {})",
        test_name, most_suitable_calls, neighbor_results, rating_calls, tank_winners, land_winners
    );
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_AFTER_ENTITY_CHANGE_MATCHES_REAL_LIVE`: real vanilla `afterEntityChange` vs the port on every
/// live habitat. `generate_faces_recorder` intercepts `generateFaces` and the smile/frown sound stubs for
/// both calls and records one ordered log of `(habitat, species type, smile)` face requests and sound
/// plays; the logs must be equal. (The faces themselves are vanilla's un-ported `generateFaces`; what the
/// port owns is deciding when to request them, which the log covers.) Each case seeds the pre-change rating
/// store identically on both sides (real vanilla's own `map<int,float>` via real `beforeEntityChange` plus
/// a raw map write, the port's [`PRE_CHANGE_SPECIES_RATINGS`]) in one of four states - the live ratings
/// (exact tie), absent (default `0.0`), far below and far above the live ratings - then runs the changed
/// entity type over `{none, a live animal type, a live non-fence scenery type, a live fence type}` x
/// `removal` x `neighbor_pass` x scripted `generateFaces` results `{all false, argument-derived}`. Both
/// sounds are captured, so nothing is audible. Coverage is counted per `(mode, entity type)` - face
/// requests, smile sounds and frown sounds - and logged in the pass line; a tie row for an animal or
/// scenery type that recorded no face request at all (its category-sum arm never fired on this save) fails
/// the test rather than passing silently.
pub(crate) fn run_after_entity_change_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::generate_faces_recorder::{self as recorder, Recorded};
    use crate::zthabitat::support::{lock_pre_change_species_ratings, rb_find, rb_map_find_or_insert, RVA_FENCE_TYPE_CHECK_ARG};
    use crate::ztmegatilemgr::RVA_SCENERY_TYPE_CHECK_ARG;
    use std::collections::BTreeMap;

    let test_name = "ZTHABITAT_AFTER_ENTITY_CHANGE_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    if habitat_ptrs.is_empty() {
        return finish_test(test_name, vec!["no live habitats found".to_string()], failure_log);
    }

    let entity_type_of = |entity: u32| get_from_memory::<u32>(entity + 0x128);
    let world = globals().ztworldmgr();
    let scenery_type = world
        .entity_array()
        .find(|&e| unsafe { entity_type_matches(e, RVA_SCENERY_TYPE_CHECK_ARG) && !entity_type_matches(e, RVA_FENCE_TYPE_CHECK_ARG) })
        .map(entity_type_of)
        .unwrap_or(0);
    let fence_type = world.entity_array().find(|&e| unsafe { entity_type_matches(e, RVA_FENCE_TYPE_CHECK_ARG) }).map(entity_type_of).unwrap_or(0);
    let animal_type = habitat_ptrs
        .iter()
        .find_map(|&h| unsafe { ref_from_memory::<ZTHabitat>(h) }.surrounding_species().next())
        .unwrap_or(0);
    let entity_types: [(&str, u32); 4] = [("none", 0), ("animal", animal_type), ("scenery", scenery_type), ("fence", fence_type)];

    let module_base = get_module_base("zoo.exe") as u32;
    let real_map_container = module_base + 0x0023_b998;
    let mut failures: Vec<String> = Vec::new();
    let mut cases = 0u32;
    // (mode, entity type) -> (face requests, smile sounds, frown sounds)
    let mut coverage: BTreeMap<(String, String), (u32, u32, u32)> = BTreeMap::new();

    let seed_stores = |habitat_ptr: u32, mode: &str| {
        hooks_zthabitatmgr::before_entity_change_real(habitat_ptr as *const i32);
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        ZTHabitatMgr::before_entity_change(habitat_ptr);
        let species_keys: Vec<i32> = habitat.surrounding_species().map(|s| get_from_memory::<i32>(s + 0x1ec)).collect();
        for key in species_keys {
            let live = lock_pre_change_species_ratings().get(&key).copied().unwrap_or(0.0);
            let seeded = match mode {
                "tie" => Some(live),
                "low" => Some(-1.0e9),
                "high" => Some(1.0e9),
                _ => None,
            };
            match seeded {
                Some(value) => {
                    let node = rb_map_find_or_insert(real_map_container, key as u32);
                    save_to_memory::<f32>(node + 0x14, value);
                    lock_pre_change_species_ratings().insert(key, value);
                }
                None => {
                    lock_pre_change_species_ratings().remove(&key);
                    if let Some(node) = rb_find(real_map_container, key as u32) {
                        save_to_memory::<f32>(node + 0x14, 0.0);
                    }
                }
            }
        }
    };

    let mut run_case = |habitat_ptr: u32, mode: &str, label: &str, entity_type: u32, removal: bool, neighbor_pass: u8, scripted: bool| {
        seed_stores(habitat_ptr, mode);
        recorder::begin_capture(scripted);
        hooks_zthabitatmgr::after_entity_change_real(habitat_ptr as *const u32, entity_type as *const u32, removal, neighbor_pass);
        let real = recorder::end_capture();

        seed_stores(habitat_ptr, mode);
        recorder::begin_capture(scripted);
        ZTHabitatMgr::after_entity_change(habitat_ptr, entity_type, removal, neighbor_pass != 0);
        let port = recorder::end_capture();

        cases += 1;
        let entry = coverage.entry((mode.to_string(), label.to_string())).or_default();
        for event in &real {
            match event {
                Recorded::GenerateFaces { .. } => entry.0 += 1,
                Recorded::SmileSound { .. } => entry.1 += 1,
                Recorded::FrownSound { .. } => entry.2 += 1,
                Recorded::AnimalFace { .. } => {}
            }
        }
        if real != port {
            failures.push(format!(
                "habitat {habitat_ptr:#010x} mode {mode} type {label} removal {removal} neighbor_pass {neighbor_pass} scripted {scripted}: real {real:x?} != port {port:x?}"
            ));
        }
    };

    let missing_types: Vec<&str> = entity_types.iter().filter(|&&(label, ptr)| ptr == 0 && label != "none").map(|&(label, _)| label).collect();
    for &habitat_ptr in &habitat_ptrs {
        for mode in ["tie", "absent", "low", "high"] {
            for &(label, entity_type) in &entity_types {
                if entity_type == 0 && label != "none" {
                    continue;
                }
                for removal in [false, true] {
                    for neighbor_pass in [0u8, 1] {
                        for scripted in [false, true] {
                            run_case(habitat_ptr, mode, label, entity_type, removal, neighbor_pass, scripted);
                        }
                    }
                }
            }
        }
    }

    // The tie rows are where the category-sum arms live (the live ratings are an exact tie, so only the
    // sums can request a face; a fence has no sum at all).
    let mut uncovered: Vec<String> = Vec::new();
    for label in ["animal", "scenery"] {
        if missing_types.contains(&label) {
            continue;
        }
        let faces = coverage.get(&("tie".to_string(), label.to_string())).map(|c| c.0).unwrap_or(0);
        if faces == 0 {
            uncovered.push(format!("tie/{label}"));
        }
    }
    if !uncovered.is_empty() {
        failures.push(format!("tie arms never fired on this save: {uncovered:?}"));
    }
    let coverage_text: Vec<String> =
        coverage.iter().map(|((mode, label), (faces, smiles, frowns))| format!("{mode}/{label}: {faces}f {smiles}s {frowns}x")).collect();
    let summary = format!(
        "{} (cases: {}, entity types missing: {:?}; per row faces/smile/frown: [{}])",
        test_name,
        cases,
        missing_types,
        coverage_text.join(", ")
    );
    finish_test(&summary, failures, failure_log)
}

/// `ZTHABITAT_SCENARIO_GOAL_EVAL06_MATCHES_REAL_LIVE`: real vanilla `ZTScenarioSimpleGoal::eval06` vs the
/// port over the live zoo. Sweeps goal kinds `0..=7` (kinds `1..=6` filter on an entity-type field, so the
/// compared value is taken from live animals' own types, plus a value no type has), rating thresholds
/// (fixed values plus every live per-animal habitat rating and its neighbours, so the average-vs-threshold
/// compare lands on and either side of real boundaries) and habitat-count thresholds; the return
/// (`goal+0x18` or `0`) must agree.
///
/// `eval06` reads only goal fields `+0x10/+0x18/+0x1c/+0x20` (`.asm`), so no constructed
/// `ZTScenarioSimpleGoal` is needed: every call runs against two buffers, one zeroed and one poisoned with
/// `0xA5` outside those fields, and real(zero) == real(poison) == port is required - a read of any other
/// goal field would show up as a mismatch.
///
/// The live zoo's neighbour sets are mostly empty, so the sweep repeats over five neighbour-set scenarios:
/// the live sets, then raw `rb_set_insert`s that make every habitat an amphibious neighbour of every
/// other, a show neighbour of every other, both (exercising the union's de-duplication), and a mixed
/// pattern with partial overlap. The inserted nodes are erased afterwards and the sets are checked equal to
/// their pre-test contents. Test-side counters per scenario log the habitats with a non-empty neighbour
/// union, the de-duplicated overlap, and how many neighbour animals pass the home-habitat gate
/// (`home == 0 || home == habitat`) versus are filtered by it.
pub(crate) fn run_scenario_goal_eval06_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::{rb_set_erase, rb_set_insert, walk_neighbor_tree};
    use openzt_detour::generated::ztanimal::GET_HABITAT_RATING as ZTANIMAL_GET_HABITAT_RATING;
    use std::collections::{BTreeMap, BTreeSet};

    let test_name = "ZTHABITAT_SCENARIO_GOAL_EVAL06_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
    let animals_of = |habitat_ptr: u32| unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.get_all_animals(false).collect::<Vec<u32>>();
    let all_animals: Vec<u32> = habitat_ptrs.iter().flat_map(|&h| animals_of(h)).collect();
    if all_animals.is_empty() {
        return finish_test(test_name, vec!["no live animals found".to_string()], failure_log);
    }
    let animal_home = |animal: u32| unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(0x004161da)(animal) };
    let neighbor_keys = |habitat_ptr: u32, set_offset: u32| -> BTreeSet<u32> {
        walk_neighbor_tree(get_from_memory::<u32>(habitat_ptr + set_offset)).map(|node| get_from_memory::<u32>(node + 0x10)).collect()
    };
    let snapshot = || -> BTreeMap<(u32, u32), BTreeSet<u32>> {
        habitat_ptrs.iter().flat_map(|&h| [0x8u32, 0x14].map(|off| ((h, off), neighbor_keys(h, off)))).collect()
    };

    let type_field_offsets = [0u32, 0x1e4, 0x1e8, 0x1ec, 0x1f4, 0x1f8, 0x1f0];
    let mut values_per_kind: Vec<Vec<i32>> = Vec::new();
    for kind in 0..=7usize {
        let mut values: Vec<i32> = vec![i32::MIN + 1];
        if let Some(&offset) = type_field_offsets.get(kind).filter(|_| kind != 0) {
            for &animal in all_animals.iter().take(40) {
                let entity_type: u32 = get_from_memory(animal + 0x128);
                if entity_type != 0 {
                    let value: i32 = get_from_memory(entity_type + offset);
                    if !values.contains(&value) {
                        values.push(value);
                    }
                }
            }
        }
        values_per_kind.push(values);
    }

    let mut failures: Vec<String> = Vec::new();
    let before = snapshot();

    // Edges to add per scenario: (habitat, set offset, neighbour).
    let n = habitat_ptrs.len();
    let mut scenarios: Vec<(&str, Vec<(u32, u32, u32)>)> = vec![("live", Vec::new())];
    for (name, pattern) in [("amphibious-all", 0u8), ("show-all", 1), ("both-all", 2), ("mixed-overlap", 3)] {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let (h, o) = (habitat_ptrs[i], habitat_ptrs[j]);
                let (amph, show) = match pattern {
                    0 => (true, false),
                    1 => (false, true),
                    2 => (true, true),
                    _ => match (i + j) % 3 {
                        0 => (true, false),
                        1 => (false, true),
                        _ => (true, true),
                    },
                };
                if amph {
                    edges.push((h, 0x8, o));
                }
                if show {
                    edges.push((h, 0x14, o));
                }
            }
        }
        scenarios.push((name, edges));
    }

    let zero_goal: &'static mut [u32; 0x10] = Box::leak(Box::new([0u32; 0x10]));
    let poison_goal: &'static mut [u32; 0x10] = Box::leak(Box::new([0xA5A5_A5A5u32; 0x10]));
    let (zero, poison) = (zero_goal.as_ptr() as u32, poison_goal.as_ptr() as u32);

    let (mut total_calls, mut total_nonzero) = (0u32, 0u32);
    let mut scenario_notes: Vec<String> = Vec::new();
    for (scenario, edges) in &scenarios {
        let mut inserted: Vec<(u32, u32)> = Vec::new();
        for &(habitat_ptr, set_offset, neighbor) in edges {
            if rb_set_insert(habitat_ptr + set_offset, neighbor) {
                inserted.push((habitat_ptr + set_offset, neighbor));
            }
        }

        // Test-side coverage for this scenario's neighbour structure.
        let (mut nonempty_unions, mut overlap, mut eligible, mut filtered, mut dedupe_sensitive) = (0u32, 0u32, 0u32, 0u32, 0u32);
        let mut ratings: BTreeSet<i32> = BTreeSet::new();
        for &habitat_ptr in &habitat_ptrs {
            let amphibious = neighbor_keys(habitat_ptr, 0x8);
            let show = neighbor_keys(habitat_ptr, 0x14);
            overlap += amphibious.intersection(&show).count() as u32;
            let union: BTreeSet<u32> = amphibious.union(&show).copied().collect();
            nonempty_unions += !union.is_empty() as u32;
            for &neighbor in &union {
                for animal in animals_of(neighbor) {
                    let home = animal_home(animal);
                    if home == 0 || home == habitat_ptr {
                        eligible += 1;
                    } else {
                        filtered += 1;
                    }
                }
            }
            for animal in animals_of(habitat_ptr).into_iter().chain(union.iter().flat_map(|&nb| animals_of(nb))) {
                ratings.insert(unsafe { ZTANIMAL_GET_HABITAT_RATING.original()(animal as *const u32, habitat_ptr as *const u32) });
            }
            // Would a missing de-duplication change this habitat's goal-free average? Average of the
            // home-gated animals with the union counted once vs with overlapping neighbours counted twice.
            let average = |neighbors: Vec<u32>| -> Option<i32> {
                let (mut count, mut sum) = (0i32, 0i32);
                for animal in animals_of(habitat_ptr).into_iter().chain(neighbors.into_iter().flat_map(&animals_of)) {
                    let home = animal_home(animal);
                    if home == 0 || home == habitat_ptr {
                        count += 1;
                        sum += unsafe { ZTANIMAL_GET_HABITAT_RATING.original()(animal as *const u32, habitat_ptr as *const u32) };
                    }
                }
                (count > 0).then(|| sum / count)
            };
            let duplicated: Vec<u32> = amphibious.iter().chain(show.iter()).copied().collect();
            dedupe_sensitive += (average(union.iter().copied().collect()) != average(duplicated)) as u32;
        }
        let mut thresholds: BTreeSet<i32> = [i32::MIN, -1, 0, 25, 50, 75, 100, i32::MAX].into_iter().collect();
        for &rating in &ratings {
            thresholds.extend([rating.saturating_sub(1), rating, rating.saturating_add(1)]);
        }
        // Where the neighbour sets overlap, a duplicated neighbour shifts a habitat's average by less than
        // one rating step, so land a threshold on every integer in the live rating range: that is what
        // makes a missing de-duplication change some call's result.
        let dense = scenario.ends_with("overlap") || *scenario == "both-all";
        if dense && let (Some(&low), Some(&high)) = (ratings.first(), ratings.last()) {
            thresholds.extend(low.saturating_sub(1)..=high.saturating_add(1));
        }
        let count_thresholds: &[i32] = if dense { &[1, 2, 3] } else { &[i32::MIN, 0, 1, 2, 3, 100] };

        let (mut calls, mut nonzero) = (0u32, 0u32);
        for kind in 0..=7i32 {
            for &value in &values_per_kind[kind as usize] {
                for &rating_threshold in &thresholds {
                    for &count_threshold in count_thresholds {
                        for goal in [zero, poison] {
                            save_to_memory::<i32>(goal + 0x10, kind);
                            save_to_memory::<i32>(goal + 0x18, rating_threshold);
                            save_to_memory::<i32>(goal + 0x1c, count_threshold);
                            save_to_memory::<i32>(goal + 0x20, value);
                        }
                        let real_zero = hooks_zthabitatmgr::scenario_goal_eval06_real(zero as i32);
                        let real_poison = hooks_zthabitatmgr::scenario_goal_eval06_real(poison as i32);
                        let port = ZTHabitat::scenario_goal_eval06(poison);
                        if real_zero != port || real_poison != port {
                            failures.push(format!(
                                "{scenario}: kind {kind} value {value} rating>={rating_threshold} habitats>={count_threshold}: real(zero)={real_zero:#x} real(poison)={real_poison:#x} port={port:#x}"
                            ));
                        }
                        calls += 1;
                        nonzero += (real_zero != 0) as u32;
                    }
                }
            }
        }
        total_calls += calls;
        total_nonzero += nonzero;
        scenario_notes.push(format!(
            "{scenario}: {calls} calls/{nonzero} non-zero, {nonempty_unions} non-empty unions, {overlap} overlap, {eligible} eligible/{filtered} filtered neighbour animals, {dedupe_sensitive} habitats whose average a missing de-duplication would change"
        ));

        for (container, neighbor) in inserted {
            rb_set_erase(container, neighbor);
        }
    }

    if snapshot() != before {
        failures.push("neighbour sets were not restored to their pre-test contents".to_string());
    }
    if n >= 2 {
        // A scenario that is supposed to overlap the two sets must have, or the union's de-duplication
        // went unexercised.
        if !scenario_notes.iter().any(|note| note.starts_with("both-all") && !note.contains(" 0 overlap")) {
            failures.push("both-all scenario produced no overlapping neighbours".to_string());
        }
    }
    let summary = format!("{} (calls: {}, non-zero: {}; {})", test_name, total_calls, total_nonzero, scenario_notes.join("; "));
    finish_test(&summary, failures, failure_log)
}

/// Heap-leaked, zeroed fake game objects (habitat, animal, unit, entity type) with fake vtables whose
/// `+0x20` slot is a fixed reply and whose `+0x228` slot always answers true. Real vanilla and the port
/// only read these objects through the offsets the show-unit cluster touches.
mod show_unit_fixture {
    use super::*;

    extern "thiscall" fn reply_false(_this: u32) -> bool {
        false
    }
    extern "thiscall" fn reply_true(_this: u32) -> bool {
        true
    }
    pub const UNIT_TYPE_ID: u32 = 0x7f3;
    extern "thiscall" fn reply_unit_type_id(_this: u32) -> u32 {
        UNIT_TYPE_ID
    }

    fn leak_vtable(slot_20: u32) -> u32 {
        let mut table = Box::new([0u32; 0x90]);
        table[0x20 / 4] = slot_20;
        table[0x228 / 4] = reply_true as *const () as u32;
        Box::leak(table).as_ptr() as u32
    }

    fn leak_object(vtable: u32) -> u32 {
        let mut object = Box::new([0u32; 0x100]);
        object[0] = vtable;
        Box::leak(object).as_ptr() as u32
    }

    pub fn new_entity_type() -> u32 {
        leak_object(leak_vtable(reply_unit_type_id as *const () as u32))
    }

    pub fn new_unit(entity_type: u32, id: u32) -> u32 {
        let unit = leak_object(leak_vtable(reply_false as *const () as u32));
        save_to_memory(unit + 0x124, id);
        save_to_memory(unit + 0x128, entity_type);
        unit
    }

    pub fn new_habitat(is_tank: bool, show_info: u32, animals: &[u32]) -> u32 {
        let slot = if is_tank { reply_true as *const () as u32 } else { reply_false as *const () as u32 };
        let habitat = leak_object(leak_vtable(slot));
        let sets = [SetFixture::new(), SetFixture::new()];
        save_to_memory(habitat + 4, show_info);
        save_to_memory(habitat + 8, sets[0].head());
        save_to_memory(habitat + 0x14, sets[1].head());
        let buffer: &'static mut [u32] = Box::leak(animals.to_vec().into_boxed_slice());
        let begin = buffer.as_ptr() as u32;
        let end = begin + 4 * animals.len() as u32;
        save_to_memory(habitat + 0x6c, begin);
        save_to_memory(habitat + 0x70, end);
        save_to_memory(habitat + 0x74, end);
        // The set headers live as long as the habitat.
        for set in sets {
            std::mem::forget(set);
        }
        habitat
    }
}

/// `ZTHABITAT_SHOW_UNIT_CLUSTER_MATCHES_REAL`: real vanilla `addShowUnit`/`removeShowUnit`/
/// `removeShowNeighbor` against the ports on two identical graphs of fake habitats seeded with show and
/// amphibious neighbour sets (a show tank with a standalone `ZTShowInfo`, land habitats forwarding to it
/// directly or through an amphibious neighbour, a no-op land habitat and an info-less tank). The same
/// operation sequence runs on both graphs; after each step the return low byte, the show info's unit list
/// and the habitats' set sizes must agree.
pub(crate) fn run_show_unit_cluster_matches_real_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::rb_set_insert;
    use crate::ztshow::{find_or_insert_pending_script_node, live_support as ztshow_live_support};
    use show_unit_fixture::*;
    let test_name = "ZTHABITAT_SHOW_UNIT_CLUSTER_MATCHES_REAL";

    struct Graph {
        node: u32,
        tank: u32,
        land1: u32,
        land2: u32,
        land3: u32,
        bare_tank: u32,
        units: [u32; 3],
    }
    let build = || {
        let info = ztshow_live_support::build_standalone_show_info();
        let (node, _) = find_or_insert_pending_script_node(info, UNIT_TYPE_ID);
        let entity_type = new_entity_type();
        let units = [new_unit(entity_type, 0x1001), new_unit(entity_type, 0x1002), new_unit(entity_type, 0x1003)];
        let tank = new_habitat(true, info, &[]);
        let land1 = new_habitat(false, 0, &[units[0], units[1]]);
        let land2 = new_habitat(false, 0, &[units[2]]);
        let land3 = new_habitat(false, 0, &[]);
        let bare_tank = new_habitat(true, 0, &[]);
        rb_set_insert(land1 + 0x14, tank);
        rb_set_insert(land2 + 0x8, land1);
        rb_set_insert(bare_tank + 0x8, land1);
        Graph { node, tank, land1, land2, land3, bare_tank, units }
    };
    let (port, real) = (build(), build());

    let list_contents = |node: u32| -> Vec<u32> {
        let sentinel = get_from_memory::<u32>(node + 0x18);
        let mut values = Vec::new();
        let mut cursor = get_from_memory::<u32>(sentinel);
        while cursor != sentinel {
            values.push(get_from_memory::<u32>(cursor + 0x8));
            cursor = get_from_memory::<u32>(cursor);
        }
        values
    };

    type Pick = fn(&Graph) -> u32;
    enum Op {
        Add(Pick, usize),
        AddNull(Pick),
        Remove(Pick, usize),
        RemoveNeighbor(Pick, Pick),
        RemoveNeighborNull(Pick),
    }
    let tank: Pick = |g| g.tank;
    let land1: Pick = |g| g.land1;
    let land2: Pick = |g| g.land2;
    let land3: Pick = |g| g.land3;
    let bare_tank: Pick = |g| g.bare_tank;
    let ops: Vec<(&str, Op)> = vec![
        ("add u0 -> tank (leaf)", Op::Add(tank, 0)),
        ("add u1 -> land1 (show neighbour)", Op::Add(land1, 1)),
        ("add u2 -> land2 (amphibious -> land1 -> tank)", Op::Add(land2, 2)),
        ("add u0 -> land3 (no neighbours)", Op::Add(land3, 0)),
        ("add u0 -> bare tank (tank, no info)", Op::Add(bare_tank, 0)),
        ("add null -> land1", Op::AddNull(land1)),
        ("remove u2 -> land2", Op::Remove(land2, 2)),
        ("remove u0 -> tank (leaf)", Op::Remove(tank, 0)),
        ("remove u1 -> land1", Op::Remove(land1, 1)),
        ("remove u0 -> land3", Op::Remove(land3, 0)),
        ("re-add u0 -> tank", Op::Add(tank, 0)),
        ("re-add u1 -> tank", Op::Add(tank, 1)),
        ("re-add u2 -> tank", Op::Add(tank, 2)),
        ("removeShowNeighbor(land2, tank): not a member, drops u2 and land1's units", Op::RemoveNeighbor(land2, tank)),
        ("re-add u0 -> tank", Op::Add(tank, 0)),
        ("re-add u1 -> tank", Op::Add(tank, 1)),
        ("re-add u2 -> tank", Op::Add(tank, 2)),
        ("removeShowNeighbor(land1, tank): member", Op::RemoveNeighbor(land1, tank)),
        ("removeShowNeighbor(land1, land3): not a show tank", Op::RemoveNeighbor(land1, land3)),
        ("removeShowNeighbor(land1, null)", Op::RemoveNeighborNull(land1)),
    ];

    let mut failures: Vec<String> = Vec::new();
    let mut nonempty_steps = 0usize;
    for (label, op) in &ops {
        let (port_ret, real_ret) = match op {
            Op::Add(habitat, unit) => (
                unsafe { ref_from_memory::<ZTHabitat>(habitat(&port)) }.add_show_unit(port.units[*unit]),
                hooks_zthabitatmgr::add_show_unit_real(habitat(&real) as *const u32, real.units[*unit]),
            ),
            Op::AddNull(habitat) => (
                unsafe { ref_from_memory::<ZTHabitat>(habitat(&port)) }.add_show_unit(0),
                hooks_zthabitatmgr::add_show_unit_real(habitat(&real) as *const u32, 0),
            ),
            Op::Remove(habitat, unit) => (
                unsafe { ref_from_memory::<ZTHabitat>(habitat(&port)) }.remove_show_unit(port.units[*unit]) as u32,
                hooks_zthabitatmgr::remove_show_unit_real(habitat(&real) as *const u32, real.units[*unit] as *const u32) as u32,
            ),
            Op::RemoveNeighbor(habitat, other) => (
                unsafe { ref_from_memory::<ZTHabitat>(habitat(&port)) }.remove_show_neighbor(other(&port)) as u32,
                hooks_zthabitatmgr::remove_show_neighbor_real(habitat(&real) as *const u32, other(&real) as *const u32),
            ),
            Op::RemoveNeighborNull(habitat) => (
                unsafe { ref_from_memory::<ZTHabitat>(habitat(&port)) }.remove_show_neighbor(0) as u32,
                hooks_zthabitatmgr::remove_show_neighbor_real(habitat(&real) as *const u32, std::ptr::null()),
            ),
        };
        let (port_list, real_list) = (list_contents(port.node), list_contents(real.node));
        if port_ret & 0xff != real_ret & 0xff {
            failures.push(format!("{label}: return port={port_ret:#x} real={real_ret:#x}"));
        }
        if port_list != real_list {
            failures.push(format!("{label}: unit list port={port_list:x?} real={real_list:x?}"));
        }
        let set_sizes = |g: &Graph| -> Vec<u32> {
            [g.tank, g.land1, g.land2, g.land3, g.bare_tank].iter().flat_map(|&h| [get_from_memory::<u32>(h + 0xc), get_from_memory::<u32>(h + 0x18)]).collect()
        };
        if set_sizes(&port) != set_sizes(&real) {
            failures.push(format!("{label}: neighbour set sizes port={:?} real={:?}", set_sizes(&port), set_sizes(&real)));
        }
        nonempty_steps += !port_list.is_empty() as usize;
        if !failures.is_empty() {
            break;
        }
    }
    finish_test(&format!("{} (steps: {}, steps with a non-empty unit list: {})", test_name, ops.len(), nonempty_steps), failures, failure_log)
}

/// Fake objects for the show-portal map tests: a habitat whose `+0x20` `{head*, size}` map container is
/// real tree memory, fences whose vtable `+0x138` slot records its calls, tiles carrying only the fields
/// `addShowPortal` and `BFMap::getDirection` read.
mod show_portal_fixture {
    use super::*;
    use std::sync::Mutex;

    /// `(fence, argument)` of every recorded vtable `+0x138` call, in order.
    pub static PORTAL_CALLS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

    extern "thiscall" fn record_portal_call(this: u32, arg: u32) {
        PORTAL_CALLS.lock().unwrap().push((this, arg));
    }
    extern "thiscall" fn type_check_pass(_this: u32, _arg: u32) -> bool {
        true
    }
    extern "thiscall" fn type_check_fail(_this: u32, _arg: u32) -> bool {
        false
    }

    fn leak_zeroed(words: usize) -> u32 {
        Box::leak(vec![0u32; words].into_boxed_slice()).as_ptr() as u32
    }

    /// A fence-like entity. `type_name` is the first byte of its entity type's name (`'g'` = gate);
    /// `family_member` is the answer its entity type gives to the fence-family type check.
    pub fn new_fence(family_member: bool, type_name: u8) -> u32 {
        let name = Box::leak(Box::new([type_name, 0u8])).as_ptr() as u32;
        let type_vtable = leak_zeroed(0x90);
        let check = if family_member { type_check_pass as *const () as u32 } else { type_check_fail as *const () as u32 };
        save_to_memory(type_vtable + 0x1c, check);
        let entity_type = leak_zeroed(0x60);
        save_to_memory(entity_type, type_vtable);
        save_to_memory(entity_type + 0xa4, name);
        let vtable = leak_zeroed(0x90);
        save_to_memory(vtable + 0x138, record_portal_call as *const () as u32);
        let fence = leak_zeroed(0x100);
        save_to_memory(fence, vtable);
        save_to_memory(fence + 0x128, entity_type);
        fence
    }

    pub fn new_tile(x: i32, y: i32) -> u32 {
        let tile = leak_zeroed(0x40);
        save_to_memory(tile + 0x34, x);
        save_to_memory(tile + 0x38, y);
        tile
    }

    /// Fake habitat memory with an empty `+0x20` map (`{head*, size}`; the head node points at itself).
    pub fn new_portal_habitat() -> u32 {
        let habitat = leak_zeroed(0x200);
        let head = leak_zeroed(5);
        save_to_memory(head + 0x8, head);
        save_to_memory(head + 0xc, head);
        save_to_memory(habitat + 0x20, head);
        habitat
    }

    pub fn map_head(habitat: u32) -> u32 {
        get_from_memory(habitat + 0x20)
    }
}

/// `ZTHABITAT_SHOW_PORTAL_ERASE_MATCHES_REAL`: the port's `removeShowPortal` (map find + erase) against
/// real vanilla `removeShowPortal` (`0x005aa4d9`, via `remove_show_portal_real`) on two fake habitats
/// whose `+0x20` maps hold the same keys (ascending, descending and pseudo-random insert orders; values
/// are non-world pointers, so neither side issues a vtable call), erased in ascending, descending and
/// random orders including absent and repeated keys down to empty. After every erase the node count,
/// tree shape, per-node value and header min/max must agree.
pub(crate) fn run_show_portal_erase_matches_real_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::zthabitat::support::rb_map_find_or_insert;
    use show_portal_fixture::*;
    let test_name = "ZTHABITAT_SHOW_PORTAL_ERASE_MATCHES_REAL";

    let mut state = 0x1357_9bdfu32;
    let mut next_random = move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state >> 16
    };
    let ascending: Vec<u32> = (1..=48).map(|i| 0x4000 + i * 0x10).collect();
    let descending: Vec<u32> = ascending.iter().rev().copied().collect();
    let shuffled = |rng: &mut dyn FnMut() -> u32| {
        let mut keys = ascending.clone();
        for i in (1..keys.len()).rev() {
            keys.swap(i, rng() as usize % (i + 1));
        }
        keys
    };
    let mut scenarios: Vec<(String, Vec<u32>, Vec<u32>)> = vec![
        ("ascending/ascending".into(), ascending.clone(), ascending.clone()),
        ("ascending/descending".into(), ascending.clone(), descending.clone()),
        ("descending/ascending".into(), descending.clone(), ascending.clone()),
    ];
    for round in 0..4 {
        let insert_order = shuffled(&mut next_random);
        let mut erase_order = shuffled(&mut next_random);
        for n in 0..10u32 {
            let position = next_random() as usize % (erase_order.len() + 1);
            let extra = if n % 2 == 0 { 0x9000 + n * 0x10 } else { erase_order[next_random() as usize % erase_order.len()] };
            erase_order.insert(position, extra);
        }
        scenarios.push((format!("random/random {round}"), insert_order, erase_order));
    }

    let value_of = |key: u32| 0x7000_0000 + key;
    let ends = |head: u32| -> (u32, u32) {
        let (min, max): (u32, u32) = (get_from_memory(head + 8), get_from_memory(head + 0xc));
        if min == head { (0, 0) } else { (get_from_memory(min + 0x10), get_from_memory(max + 0x10)) }
    };
    fn values_in_order(head: u32) -> Vec<(u32, u32)> {
        walk_neighbor_tree(head).map(|node| (get_from_memory(node + 0x10), get_from_memory(node + 0x14))).collect()
    }

    let mut failures: Vec<String> = Vec::new();
    let mut erases = 0usize;
    'scenarios: for (label, insert_order, erase_order) in &scenarios {
        let (port, real) = (new_portal_habitat(), new_portal_habitat());
        for &key in insert_order {
            for habitat in [port, real] {
                let node = rb_map_find_or_insert(habitat + 0x20, key);
                save_to_memory(node + 0x14, value_of(key));
            }
        }
        for (index, &key) in erase_order.iter().enumerate() {
            unsafe { ref_from_memory::<ZTHabitat>(port) }.remove_show_portal(key);
            hooks_zthabitatmgr::remove_show_portal_real(real as *const u32, key as *const u32);
            erases += 1;
            let (port_head, real_head) = (map_head(port), map_head(real));
            let (port_size, real_size): (u32, u32) = (get_from_memory(port + 0x24), get_from_memory(real + 0x24));
            match compare_tree_shapes(get_from_memory(port_head + 4), port_head, get_from_memory(real_head + 4), real_head) {
                Ok(count) if count == port_size && port_size == real_size => {}
                Ok(count) => {
                    failures.push(format!("{label}[{index}] key {key:#x}: node count {count} vs size port={port_size} real={real_size}"));
                    break 'scenarios;
                }
                Err(msg) => {
                    failures.push(format!("{label}[{index}] key {key:#x}: {msg}"));
                    break 'scenarios;
                }
            }
            if values_in_order(port_head) != values_in_order(real_head) || ends(port_head) != ends(real_head) {
                failures.push(format!("{label}[{index}] key {key:#x}: node values or header min/max differ"));
                break 'scenarios;
            }
        }
        if get_from_memory::<u32>(port + 0x24) != 0 || get_from_memory::<u32>(real + 0x24) != 0 {
            failures.push(format!("{label}: map not empty after erasing every key"));
        }
    }
    if !PORTAL_CALLS.lock().unwrap().is_empty() {
        failures.push("a vtable +0x138 call was made for a non-world fence".to_string());
    }
    finish_test(&format!("{} (erases compared: {}, scenarios: {})", test_name, erases, scenarios.len()), failures, failure_log)
}

/// `ZTHABITAT_ADD_SHOW_PORTAL_MATCHES_REAL_LIVE`: real vanilla `addShowPortal` (`0x005ab354`) and
/// `removeShowPortal` against the ports, driven in lockstep on two fake habitats. The portal owners are
/// the live zoo's real habitats (`tile_b` is a fake tile placed on a tile each real habitat owns, so the
/// vanilla habitat-table lookup resolves a real owner); `tile_a` sits one step away in a direction
/// `BFMap::getDirection` accepts, each tile carrying one fake fence in the connecting slot. Compared per
/// step: the return low byte, the recorded fence vtable `+0x138` calls (fence role, argument), the map's
/// tree shape and per-node fence role. Covers a first add per owner, replacing an existing entry's
/// fence, the bail-outs (null `tile_b`, a second fence on a tile, a gate fence, a non-fence entity) and
/// erasing every owner (plus an absent one). **Gap**: a previous fence that is a live world entity (the
/// `+0x138(0)` call on replacement/removal) is not exercised - it would call into a real fence.
pub(crate) fn run_add_show_portal_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use show_portal_fixture::*;
    let test_name = "ZTHABITAT_ADD_SHOW_PORTAL_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let habitat_ptrs: Vec<u32> = (0..habitat_mgr.exhibit_array().len()).map(|i| habitat_mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();

    // One owned tile position per live habitat that the vanilla table lookup attributes back to it.
    let mut owner_positions: Vec<(u32, i32, i32)> = Vec::new();
    for &habitat_ptr in &habitat_ptrs {
        let sentinel: u32 = get_from_memory(habitat_ptr + 0x40);
        if let Some(node) = walk_tile_list(sentinel).next() {
            let tile = get_from_memory::<TileListNode>(node).payload;
            if tile != 0 {
                let (x, y): (i32, i32) = (get_from_memory(tile + 0x34), get_from_memory(tile + 0x38));
                if habitat_mgr.get_habitat_ptr(x, y) == habitat_ptr {
                    owner_positions.push((habitat_ptr, x, y));
                }
            }
        }
    }
    if owner_positions.is_empty() {
        return finish_test(test_name, vec!["no live habitat with an owned tile found".to_string()], failure_log);
    }

    struct Side {
        habitat: u32,
        fences: Vec<u32>,
    }
    let mut sides = [Side { habitat: new_portal_habitat(), fences: Vec::new() }, Side { habitat: new_portal_habitat(), fences: Vec::new() }];
    let role = |side: &Side, ptr: u32| -> usize { side.fences.iter().position(|&f| f == ptr).map_or(0, |i| i + 1) };
    let mut failures: Vec<String> = Vec::new();
    let (mut adds_true, mut adds_false) = (0usize, 0usize);

    // Builds one `(tile_a, tile_b)` pair per side for `owner_pos`, with fence roles allocated in the same
    // order on both sides. `a_fences`/`b_fences` are `(family_member, type_name)` per tile (empty = no fence).
    let build_pair = |sides: &mut [Side; 2], owner_pos: (i32, i32), a_fences: &[(bool, u8)], b_fences: &[(bool, u8)]| -> Option<[(u32, u32); 2]> {
        let mut pairs = [(0u32, 0u32); 2];
        for (index, side) in sides.iter_mut().enumerate() {
            let tile_b = new_tile(owner_pos.0, owner_pos.1);
            let mut found = None;
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let tile_a = new_tile(owner_pos.0 + dx, owner_pos.1 + dy);
                let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a as i32, tile_b as i32) };
                if direction != -1 {
                    found = Some((tile_a, direction));
                    break;
                }
            }
            let (tile_a, direction) = found?;
            let slots_a = [direction / 2, ((direction - 4) & 7) / 2];
            for (tile, fences, slot) in [(tile_a, a_fences, slots_a[0]), (tile_b, b_fences, slots_a[1])] {
                for (n, &(family_member, type_name)) in fences.iter().enumerate() {
                    let fence = new_fence(family_member, type_name);
                    side.fences.push(fence);
                    // Extra fences land in other slots so the tile's fence count rises without
                    // touching the connecting slot.
                    save_to_memory(tile + 0x14 + (((slot + n as i32) % 4) as u32) * 4, fence);
                }
            }
            pairs[index] = (tile_a, tile_b);
        }
        Some(pairs)
    };

    let normalized_calls = |sides: &[Side; 2]| -> [Vec<(usize, u32)>; 2] {
        let calls = std::mem::take(&mut *PORTAL_CALLS.lock().unwrap());
        // Calls were recorded in port-then-real order for each step; split by which side owns the fence.
        let mut per_side = [Vec::new(), Vec::new()];
        for (fence, arg) in calls {
            for (i, side) in sides.iter().enumerate() {
                let r = role(side, fence);
                if r != 0 {
                    per_side[i].push((r, arg));
                }
            }
        }
        per_side
    };
    let map_roles = |side: &Side| -> Vec<(u32, usize)> {
        walk_neighbor_tree(map_head(side.habitat)).map(|node| (get_from_memory::<u32>(node + 0x10), role(side, get_from_memory::<u32>(node + 0x14)))).collect()
    };

    enum Step {
        Add { owner: usize, a: Vec<(bool, u8)>, b: Vec<(bool, u8)>, null_b: bool },
        Remove(u32),
    }
    let mut steps: Vec<(String, Step)> = Vec::new();
    for (i, &(habitat_ptr, _, _)) in owner_positions.iter().enumerate() {
        steps.push((format!("first add for owner {habitat_ptr:#x}"), Step::Add { owner: i, a: vec![(true, b'w')], b: vec![(true, b'w')], null_b: false }));
    }
    steps.push(("replace existing entry's fence".into(), Step::Add { owner: 0, a: vec![(true, b'w')], b: vec![(true, b'w')], null_b: false }));
    steps.push(("null tile_b".into(), Step::Add { owner: 0, a: vec![(true, b'w')], b: vec![], null_b: true }));
    steps.push(("second fence on tile_a".into(), Step::Add { owner: 0, a: vec![(true, b'w'), (true, b'w')], b: vec![(true, b'w')], null_b: false }));
    steps.push(("gate fence on tile_a".into(), Step::Add { owner: 0, a: vec![(true, b'g')], b: vec![(true, b'w')], null_b: false }));
    steps.push(("gate fence on tile_b".into(), Step::Add { owner: 0, a: vec![(true, b'w')], b: vec![(true, b'g')], null_b: false }));
    steps.push(("non-fence entity on tile_a".into(), Step::Add { owner: 0, a: vec![(false, b'w')], b: vec![(true, b'w')], null_b: false }));
    steps.push(("no fence on tile_b".into(), Step::Add { owner: 0, a: vec![(true, b'w')], b: vec![], null_b: false }));
    for &(habitat_ptr, _, _) in owner_positions.iter().rev() {
        steps.push((format!("remove owner {habitat_ptr:#x}"), Step::Remove(habitat_ptr)));
    }
    steps.push(("remove absent owner".into(), Step::Remove(0x1234_5670)));

    for (label, step) in &steps {
        let results: [u32; 2] = match step {
            Step::Add { owner, a, b, null_b } => {
                let (_, x, y) = owner_positions[*owner];
                let Some(pairs) = build_pair(&mut sides, (x, y), a, b) else {
                    failures.push(format!("{label}: BFMap::getDirection rejected every neighbouring fake tile"));
                    break;
                };
                let tile_b = |pair: (u32, u32)| if *null_b { 0 } else { pair.1 };
                let port_result = unsafe { ref_from_memory::<ZTHabitat>(sides[0].habitat) }.add_show_portal(pairs[0].0, tile_b(pairs[0])) as u32;
                let real_result = hooks_zthabitatmgr::add_show_portal_real(sides[1].habitat as *const u32, pairs[1].0 as *const u32, tile_b(pairs[1]) as *const u32);
                [port_result, real_result]
            }
            Step::Remove(key) => {
                unsafe { ref_from_memory::<ZTHabitat>(sides[0].habitat) }.remove_show_portal(*key);
                hooks_zthabitatmgr::remove_show_portal_real(sides[1].habitat as *const u32, *key as *const u32);
                [0, 0]
            }
        };
        if let Step::Add { .. } = step {
            if results[0] & 0xff != results[1] & 0xff {
                failures.push(format!("{label}: return port={:#x} real={:#x}", results[0], results[1]));
            }
            if results[1] & 0xff != 0 { adds_true += 1 } else { adds_false += 1 }
        }
        let calls = normalized_calls(&sides);
        if calls[0] != calls[1] {
            failures.push(format!("{label}: fence vtable +0x138 calls port={:?} real={:?}", calls[0], calls[1]));
        }
        let (port_head, real_head) = (map_head(sides[0].habitat), map_head(sides[1].habitat));
        let (port_size, real_size): (u32, u32) = (get_from_memory(sides[0].habitat + 0x24), get_from_memory(sides[1].habitat + 0x24));
        match compare_tree_shapes(get_from_memory(port_head + 4), port_head, get_from_memory(real_head + 4), real_head) {
            Ok(count) if count == port_size && port_size == real_size => {}
            Ok(count) => failures.push(format!("{label}: node count {count} vs size port={port_size} real={real_size}")),
            Err(msg) => failures.push(format!("{label}: {msg}")),
        }
        let (port_roles, real_roles) = (map_roles(&sides[0]), map_roles(&sides[1]));
        if port_roles != real_roles {
            failures.push(format!("{label}: map contents port={port_roles:?} real={real_roles:?}"));
        }
        if !failures.is_empty() {
            break;
        }
    }
    if adds_true == 0 {
        failures.push("no addShowPortal call succeeded on the real side - the fixture never reached the map code".to_string());
    }
    finish_test(
        &format!("{} (owners: {}, steps: {}, adds accepted: {}, adds rejected: {})", test_name, owner_positions.len(), steps.len(), adds_true, adds_false),
        failures,
        failure_log,
    )
}

/// `ZTHABITAT_DESTRUCTOR_REACHED`: the `~ZTHabitat` port ([`ZTHabitat::destruct`]) must have been run by
/// vanilla's own teardown paths (habitat removal / zoo clear) earlier in the battery - the destructor
/// has no standalone-fixture comparison, so this guards against the detour silently never firing.
pub(crate) fn run_habitat_destructor_reached_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_DESTRUCTOR_REACHED";
    let calls = crate::zthabitat::habitat::DESTRUCT_CALLS.load(std::sync::atomic::Ordering::Relaxed);
    let mut failures = Vec::new();
    if calls == 0 {
        failures.push("ZTHabitat::destruct never ran during the battery".to_string());
    }
    tracing::info!("{}: destruct ran {} time(s)", test_name, calls);
    finish_test(test_name, failures, failure_log)
}
