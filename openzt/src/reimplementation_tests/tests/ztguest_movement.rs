//! Live checks for `ztguest/movement.rs` against the loaded zoo: the pathfinder callbacks, the building-need
//! predicates, `pickRandomDest`, `findNearbyNeedSatisfier` and the grandstand helpers. Guests come from the real
//! vanilla constructor; the real body, the port and the hooked address run on the same guest and tile, and results,
//! out values, tile bytes, the shared RNG and the guest's memory are compared. The scripted-environment searches
//! (`findPlaceToSit`, `findTrashCan`, `gotoNearbyNeed`, `drop`, `addTrash`) are in `ztguest_checks.rs`.

use std::sync::atomic::Ordering;

use glam::i32::IVec3;
use openzt_detour::generated::{ztunit::GET_HABITAT_FROM_TILE, ztguest::{
    FENCES_OK, FIND_NEARBY_NEED_SATISFIER, GET_TERRAIN_COST, IN_GRANDSTAND, IS_NEED_SATISFIER, IS_PRESSING_NEED_SATISFIER, PICK_RANDOM_DEST, TILE_FILTER,
    TRIGGER_GRANDSTAND_ESCAPE_0, TRIGGER_GRANDSTAND_ESCAPE_1,
}};

use super::{
    ztguide::{base, make_fake_fence, pin_rng, rng, FENCE_CAST_RESULT},
    ztguide_habitats::{install_rand_hook, rand_calls, script_rand, set_rand_intercept},
};
use crate::{
    globals::globals,
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguest::{
        live_support::{self, movement_real as real},
        ZTGuest,
    },
    ztkeeper::unit_offsets,
    ztmegatilemgr::entity_type_matches,
};

const TILE_SIZE: usize = 0x8c;
const MAP_LIMIT: u32 = 64;
const BUILDING_TAG_RVA: u32 = 0x0023_8680;
/// `ZTGuest` fields the movement code reads.
const GOAL_ID: u32 = 0x170;
const HELD_TILE_ENTITY: u32 = 0x158;
const WATER_FLAG: u32 = 0x210;
const TILE_FILTER_MODE: u32 = 0x1bc;
const RETURN_BUILDING_BITS: u32 = 0x284;
/// `{begin, end, cap}` of the candidate vector `findNearbyNeedSatisfier` fills (`DAT_0063c538`).
const CANDIDATE_VECTOR_RVA: u32 = 0x0023_c538;

fn place_guest(guest: ZTGuest, tile: (u32, u32)) {
    save_to_memory(guest.0 + 0x114u32, tile.0 * 64 + 32);
    save_to_memory(guest.0 + 0x118u32, tile.1 * 64 + 32);
    save_to_memory(guest.0 + 0x11cu32, 0u32);
}

fn leaked(bytes: usize) -> u32 {
    Box::leak(vec![0u8; bytes].into_boxed_slice()).as_ptr() as u32
}

fn live_entities() -> Vec<u32> {
    let world = globals().ztworldmgr();
    let mut addr = world.entity_array_start();
    let end = world.entity_array_end();
    let mut entities = Vec::new();
    while addr < end {
        let entity: u32 = get_from_memory(addr);
        if entity != 0 {
            entities.push(entity);
        }
        addr += 4;
    }
    entities
}

fn live_buildings() -> Vec<u32> {
    live_entities().into_iter().filter(|&entity| unsafe { entity_type_matches(entity, BUILDING_TAG_RVA) }).collect()
}

fn tile_positions(limit: u32) -> Vec<(u32, u32)> {
    let world = globals().ztworldmgr();
    (0..world.map_y_size.min(limit)).flat_map(|y| (0..world.map_x_size.min(limit)).map(move |x| (x, y))).collect()
}

/// Two fixed positions plus the first tile of up to two distinct habitats, so the habitat rules see both a guest
/// outside any habitat and guests inside one.
fn guest_positions() -> Vec<(u32, u32)> {
    let world = globals().ztworldmgr();
    let mut positions = vec![(0, 0), (world.map_x_size / 2, world.map_y_size / 2)];
    let mut habitats = Vec::new();
    for y in 0..world.map_y_size {
        for x in 0..world.map_x_size {
            let habitat = unsafe { GET_HABITAT_FROM_TILE.hooked()(std::ptr::null(), world.get_tile_ptr(x, y) as *const u32) };
            if habitat != 0 && !habitats.contains(&habitat) && habitats.len() < 2 {
                habitats.push(habitat);
                positions.push((x, y));
            }
        }
    }
    positions
}

/// A copy of a live tile in leaked memory: it keeps its coordinates, so neighbour lookups find the real neighbours
/// while its own slots and flags are ours.
fn copied_tile(tile: u32) -> u32 {
    let copy = leaked(TILE_SIZE);
    unsafe { std::ptr::copy_nonoverlapping(tile as *const u8, copy as *mut u8, TILE_SIZE) };
    copy
}

/// Gives a tile a list of `entities` at `+0x0` (`{sentinel -> nodes}`) and sets its `0x8` flag at `+0x85`.
fn tile_with_entity_list(tile: u32, entities: &[u32]) -> u32 {
    let copy = copied_tile(tile);
    let sentinel = leaked(12);
    let nodes: Vec<u32> = entities.iter().map(|_| leaked(12)).collect();
    let mut previous = sentinel;
    for (&node, &entity) in nodes.iter().zip(entities) {
        save_to_memory(previous, node);
        save_to_memory(node + 4, previous);
        save_to_memory(node + 8, entity);
        previous = node;
    }
    save_to_memory(previous, sentinel);
    save_to_memory(sentinel + 4, previous);
    save_to_memory(copy, sentinel);
    save_to_memory(copy + 0x85, get_from_memory::<u8>(copy + 0x85) | 8);
    copy
}

struct Standalone {
    guest: ZTGuest,
}

fn standalone(failures: &mut Vec<String>) -> Option<Standalone> {
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return None;
    };
    Some(Standalone { guest: live_support::build_standalone_guest(guest_type) })
}

/// `ZTGUEST_TERRAIN_COST_LIVE`: `getTerrainCost` over live tiles and tiles carrying entity lists, from two guest
/// positions and over the guest states the cost depends on (goal, water flags, the held tile entity, the water byte),
/// real against port with the tile's flag byte compared afterwards, plus the hooked address on a sample.
pub(crate) fn run_ztguest_terrain_cost_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_TERRAIN_COST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(Standalone { guest }) = standalone(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let entities = live_entities();
    let mut seen = std::collections::BTreeSet::new();
    let mut mismatches = 0;
    let mut check = |label: &str, tile: u32, failures: &mut Vec<String>, hooked: bool| {
        let flags_before: u8 = get_from_memory(tile + 0x85);
        let real_cost = real::get_terrain_cost(guest.0, tile);
        let real_flags: u8 = get_from_memory(tile + 0x85);
        save_to_memory(tile + 0x85, flags_before);
        let port_cost = guest.get_terrain_cost(tile);
        let port_flags: u8 = get_from_memory(tile + 0x85);
        save_to_memory(tile + 0x85, flags_before);
        seen.insert(real_cost);
        if real_cost != port_cost || real_flags != port_flags {
            mismatches += 1;
            if mismatches <= 10 {
                failures.push(format!("{label}: real {real_cost} (flags {real_flags:#x}), port {port_cost} (flags {port_flags:#x})"));
            }
        }
        if hooked {
            let hooked_cost = unsafe { GET_TERRAIN_COST.hooked()(guest.0 as *const u32, tile as *const u32) };
            save_to_memory(tile + 0x85, flags_before);
            if hooked_cost != real_cost {
                failures.push(format!("{label}: hooked {hooked_cost}, real {real_cost}"));
            }
        }
    };
    let tiles = tile_positions(MAP_LIMIT);
    for position in guest_positions() {
        place_guest(guest, position);
        let own = world.get_tile_ptr(position.0, position.1);
        for goal in [0u32, 0x71, 0x75] {
            for bits in [0u32, 0x7_ffff, 0xfff8_0000, 0xffff_ffff] {
                for water in [0u8, 1] {
                    for held in [0u32, own, world.get_tile_ptr(world.map_x_size / 3, world.map_y_size / 3)] {
                        save_to_memory(guest.0 + GOAL_ID, goal);
                        save_to_memory(guest.0 + RETURN_BUILDING_BITS, bits);
                        save_to_memory(guest.0 + WATER_FLAG, water);
                        save_to_memory(guest.0 + HELD_TILE_ENTITY, held);
                        for (index, &(x, y)) in tiles.iter().enumerate() {
                            let tile = world.get_tile_ptr(x, y);
                            check(&format!("guest {position:?} goal {goal:#x} bits {bits:#x} water {water} held {held:#x} tile ({x},{y})"), tile, &mut failures, index % 211 == 0);
                        }
                        // Tiles with a predator-flag list: each live entity alone, then three together.
                        for chunk in entities.chunks(3).take(12) {
                            let tile = tile_with_entity_list(own, chunk);
                            check(&format!("guest {position:?} goal {goal:#x} bits {bits:#x} list of {}", chunk.len()), tile, &mut failures, true);
                        }
                    }
                }
            }
        }
    }
    save_to_memory(guest.0 + RETURN_BUILDING_BITS, 0u32);
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if seen.len() < 3 {
        failures.push(format!("only the costs {seen:?} occurred, so the grid covered too few branches"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_FENCES_FILTER_LIVE`: `fencesOK` and `tileFilter` over live tiles, then over a copy of a live tile whose
/// edge slots hold fake fences (open, gate, wall, not a fence).
pub(crate) fn run_ztguest_fences_filter_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_FENCES_FILTER_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(Standalone { guest }) = standalone(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let mut mismatches = 0;
    let mut blocked = 0;
    let mut skipped = 0;
    let mut check_fences = |label: String, tile: u32, direction: u32, failures: &mut Vec<String>| -> u32 {
        if live_support::movement_fences_would_fault(guest, tile, direction as i32) {
            skipped += 1;
            return 1;
        }
        let (mut real_a, mut real_b, mut port_a, mut port_b, mut hooked_a, mut hooked_b) = (0xaau8, 0xbbu8, 0xaau8, 0xbbu8, 0xaau8, 0xbbu8);
        let real_result = real::fences_ok(guest.0, tile, direction, &mut real_a as *mut u8 as u32, &mut real_b as *mut u8 as u32) & 0xff;
        let port_result = guest.fences_ok(tile, direction as i32, &mut port_a as *mut u8 as u32, &mut port_b as *mut u8 as u32) as u32;
        let hooked_result = unsafe {
            FENCES_OK.hooked()(guest.0 as *const u32, tile as *const u32, direction, &mut hooked_a as *mut u8 as *const bool, &mut hooked_b as *mut u8 as *const bool)
        } & 0xff;
        let outs = [real_a, real_b, port_a, port_b, hooked_a, hooked_b];
        if real_result != port_result || real_result != hooked_result || outs != [0, 0, 0, 0, 0, 0] {
            mismatches += 1;
            if mismatches <= 10 {
                failures.push(format!("{label}: results real {real_result}, port {port_result}, hooked {hooked_result}; out bytes {outs:?}"));
            }
        }
        real_result
    };
    for position in guest_positions() {
        place_guest(guest, position);
        for (x, y) in tile_positions(MAP_LIMIT) {
            let tile = world.get_tile_ptr(x, y);
            for direction in (0u32..8).chain([0xffff_ffff]) {
                if check_fences(format!("tile ({x},{y}) direction {direction:#x}"), tile, direction, &mut failures) == 0 {
                    blocked += 1;
                }
            }
        }
    }
    check_fences("null tile".to_string(), 0, 2, &mut failures);
    let live_tile = world.get_tile_ptr(world.map_x_size / 2, world.map_y_size / 2);
    for (label, type_name, passable, is_fence) in
        [("open fence", &b"wall\0"[..], 1u8, 1u32), ("closed gate", b"gate\0", 0, 1), ("closed wall", b"wall\0", 0, 1), ("not a fence", b"wall\0", 0, 0)]
    {
        FENCE_CAST_RESULT.store(is_fence, Ordering::SeqCst);
        let fence = make_fake_fence(Box::leak(type_name.to_vec().into_boxed_slice()), passable);
        let other = make_fake_fence(Box::leak(b"gate\0".to_vec().into_boxed_slice()), 0);
        for direction in [0u32, 2, 4, 6] {
            for far in [false, true] {
                let fake = copied_tile(live_tile);
                save_to_memory(fake + 0x14 + (direction / 2) * 4, fence);
                if far {
                    save_to_memory(fake + 0x14 + (((direction + 4) & 7) / 2) * 4, other);
                }
                check_fences(format!("{label} direction {direction} far {far}"), fake, direction, &mut failures);
            }
        }
    }
    FENCE_CAST_RESULT.store(1, Ordering::SeqCst);

    let mut filter_results = std::collections::BTreeSet::new();
    for position in guest_positions() {
        place_guest(guest, position);
        for mode in [0i32, 3] {
            save_to_memory(guest.0 + TILE_FILTER_MODE, mode);
            for flag in [0u32, 1, 0x100, 0x1ff] {
                for (x, y) in tile_positions(MAP_LIMIT) {
                    let tile = world.get_tile_ptr(x, y);
                    let real_result = real::tile_filter(guest.0, tile, flag);
                    let port_result = guest.tile_filter(tile, flag as u8 != 0);
                    filter_results.insert(real_result);
                    let hooked_result = unsafe { TILE_FILTER.hooked()(guest.0 as *const u32, tile as *const u32, std::ptr::without_provenance::<u32>(flag as usize)) };
                    if real_result != port_result || real_result != hooked_result {
                        mismatches += 1;
                        if mismatches <= 10 {
                            failures.push(format!(
                                "tileFilter guest {position:?} mode {mode} flag {flag:#x} tile ({x},{y}): real {real_result}, port {port_result}, hooked {hooked_result}"
                            ));
                        }
                    }
                }
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if filter_results.len() < 2 {
        failures.push(format!("tileFilter only ever answered {filter_results:?}"));
    }
    tracing::info!("{test_name}: {blocked} live tile/direction pairs were blocked, {skipped} skipped because vanilla would fault");
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_NEED_PREDICATES_LIVE`: `isNeedSatisfier` and `isPressingNeedSatisfier` over live tiles and grid positions
/// (inside the grid, on its edges and outside it), real against port and hooked.
pub(crate) fn run_ztguest_need_predicates_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_NEED_PREDICATES_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(Standalone { guest }) = standalone(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let coordinates = [-1i32, 0, 5, 31, 32, 33, 63, 64];
    let mut mismatches = 0;
    let (mut satisfied, mut pressing) = (0, 0);
    let mut tiles: Vec<u32> = tile_positions(MAP_LIMIT * 2).into_iter().map(|(x, y)| world.get_tile_ptr(x, y)).collect();
    tiles.push(0);
    for tile in tiles {
        for &x in &coordinates {
            for &y in &coordinates {
                let position = IVec3::new(x, y, 0);
                let real_result = real::is_need_satisfier(guest.0, tile, &position);
                let port_result = guest.is_need_satisfier(tile, &position as *const IVec3 as u32);
                let hooked_result = unsafe { IS_NEED_SATISFIER.hooked()(guest.0 as *const u32, tile as *const u32, &position) };
                satisfied += real_result as u32;
                if real_result != port_result || real_result != hooked_result {
                    mismatches += 1;
                    if mismatches <= 10 {
                        failures.push(format!("isNeedSatisfier tile {tile:#x} ({x},{y}): real {real_result}, port {port_result}, hooked {hooked_result}"));
                    }
                }
                let (mut real_out, mut port_out, mut hooked_out) = (0xaau8, 0xaau8, 0xaau8);
                let real_result = real::is_pressing_need_satisfier(guest.0, &mut real_out as *mut u8 as u32, tile, &position);
                let port_result = guest.is_pressing_need_satisfier(&mut port_out as *mut u8 as u32, tile, &position as *const IVec3 as u32);
                let hooked_result =
                    unsafe { IS_PRESSING_NEED_SATISFIER.hooked()(guest.0 as *const u32, &mut hooked_out as *mut u8 as *const u8, tile as *const u32, &position) };
                pressing += real_result as u32;
                if real_result != port_result || real_result != hooked_result || real_out != port_out || real_out != hooked_out {
                    mismatches += 1;
                    if mismatches <= 10 {
                        failures.push(format!(
                            "isPressingNeedSatisfier tile {tile:#x} ({x},{y}): real {real_result}/{real_out:#x}, port {port_result}/{port_out:#x}, hooked {hooked_result}/{hooked_out:#x}"
                        ));
                    }
                }
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    tracing::info!("{test_name}: {satisfied} isNeedSatisfier and {pressing} isPressingNeedSatisfier positions were true");
    if satisfied == 0 {
        failures.push("isNeedSatisfier was never true: the live zoo holds no satisfying building in the scanned tiles".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

fn candidate_vector() -> Vec<u32> {
    let vector = base() + CANDIDATE_VECTOR_RVA;
    let (begin, end): (u32, u32) = (get_from_memory(vector), get_from_memory(vector + 4));
    if begin == 0 {
        return Vec::new();
    }
    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
}

/// `ZTGUEST_NEARBY_NEED_LIVE`: `pickRandomDest` (RNG pinned) and `findNearbyNeedSatisfier` (scripted `rand`) at a grid
/// of guest positions, real against port and hooked: results, out values, the shared candidate vector and the draws.
pub(crate) fn run_ztguest_nearby_need_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_NEARBY_NEED_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(Standalone { guest }) = standalone(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_rand_hook() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let world = globals().ztworldmgr();
    let saved_rng = rng();
    let mut mismatches = 0;
    let (mut found_any, mut dest_any) = (0, 0);
    let step_x = (world.map_x_size / 10).max(1) as usize;
    let step_y = (world.map_y_size / 10).max(1) as usize;
    for y in (0..world.map_y_size).step_by(step_y) {
        for x in (0..world.map_x_size).step_by(step_x) {
            place_guest(guest, (x, y));
            let tile = world.get_tile_ptr(x, y);
            for range in [0i32, 5, 10, 40, 200] {
                let mut poles = Vec::new();
                for pole in 0..3 {
                    script_rand(&[7, 3, 11, 2]);
                    set_rand_intercept(true);
                    let (mut need, mut building) = (0xaau8, 0xaau32);
                    let result = match pole {
                        0 => real::find_nearby_need_satisfier(guest.0, tile, &mut need as *mut u8 as u32, &mut building as *mut u32 as u32, range),
                        1 => guest.find_nearby_need_satisfier(tile, &mut need as *mut u8 as u32, &mut building as *mut u32 as u32, range),
                        _ => unsafe {
                            FIND_NEARBY_NEED_SATISFIER.hooked()(guest.0 as *const u32, tile as *const u32, &mut need as *mut u8 as *const u8, &mut building as *mut u32 as *const u32, range)
                        },
                    };
                    set_rand_intercept(false);
                    poles.push((result, need, building, candidate_vector(), rand_calls()));
                }
                found_any += poles[0].0 as u32;
                if poles[0] != poles[1] || poles[0] != poles[2] {
                    mismatches += 1;
                    if mismatches <= 10 {
                        failures.push(format!("findNearbyNeedSatisfier ({x},{y}) range {range}: real {:?}, port {:?}, hooked {:?}", poles[0], poles[1], poles[2]));
                    }
                }
            }
            for seed in [1u32, 0x1234_5678] {
                let mut poles = Vec::new();
                for pole in 0..3 {
                    pin_rng(seed);
                    let mut position = IVec3::new(-5, -5, -5);
                    let result = match pole {
                        0 => real::pick_random_dest(guest.0, &mut position),
                        1 => guest.pick_random_dest(&position),
                        _ => unsafe { PICK_RANDOM_DEST.hooked()(guest.0 as *const u32, &position) },
                    };
                    poles.push((result, position, rng()));
                }
                dest_any += poles[0].0 as u32;
                if poles[0] != poles[1] || poles[0] != poles[2] {
                    mismatches += 1;
                    if mismatches <= 10 {
                        failures.push(format!("pickRandomDest ({x},{y}) seed {seed:#x}: real {:?}, port {:?}, hooked {:?}", poles[0], poles[1], poles[2]));
                    }
                }
            }
        }
    }
    pin_rng(saved_rng);
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    tracing::info!("{test_name}: findNearbyNeedSatisfier found a building in {found_any} cases, pickRandomDest a destination in {dest_any}");
    finish_test(test_name, failures, failure_log)
}

static BUILDING_SLOT_ANSWER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

extern "thiscall" fn slot_validate_building(_this: u32, _flag: u32, _arg: u32) -> u32 {
    BUILDING_SLOT_ANSWER.load(Ordering::SeqCst)
}

/// `ZTGUEST_GRANDSTAND_LIVE`: `inGrandstand` and both `triggerGrandstandEscape` overloads with the validated-building
/// slot (`+0x2e8`) answering each live building, a non-building entity and nothing, real against port and hooked.
pub(crate) fn run_ztguest_grandstand_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_GRANDSTAND_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(Standalone { guest }) = standalone(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let real_vtable: u32 = get_from_memory(guest.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x2e8 / 4] = slot_validate_building as *const () as usize as u32;
    save_to_memory(guest.0, Box::leak(vtable.into_boxed_slice()).as_ptr() as u32);
    let entities = live_entities();
    let mut answers: Vec<u32> = live_buildings();
    answers.push(0);
    answers.extend(entities.iter().copied().take(5));
    let mut in_grandstand = 0;
    for answer in answers {
        BUILDING_SLOT_ANSWER.store(answer, Ordering::SeqCst);
        let real_result = real::in_grandstand(guest.0);
        let port_result = guest.in_grandstand();
        let hooked_result = unsafe { IN_GRANDSTAND.hooked()(guest.0 as *const u32) };
        in_grandstand += real_result as u32;
        if real_result != port_result || real_result != hooked_result {
            failures.push(format!("inGrandstand building {answer:#x}: real {real_result}, port {port_result}, hooked {hooked_result}"));
        }
        real::trigger_grandstand_escape_0(guest.0);
        guest.trigger_grandstand_escape();
        unsafe { TRIGGER_GRANDSTAND_ESCAPE_0.hooked()(guest.0 as *const u32) };
        for &entity in entities.iter().take(30) {
            let id: i32 = get_from_memory(entity + unit_offsets::ENTITY_ID);
            real::trigger_grandstand_escape_1(guest.0, id);
            guest.trigger_grandstand_escape_for(id);
            unsafe { TRIGGER_GRANDSTAND_ESCAPE_1.hooked()(guest.0 as *const u32, id) };
        }
    }
    tracing::info!("{test_name}: {in_grandstand} building answers were grandstands");
    finish_test(test_name, failures, failure_log)
}
