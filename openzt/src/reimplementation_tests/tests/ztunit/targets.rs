//! Live checks for the movement (`ztunit/movement.rs`) and satisfier (`ztunit/satisfiers.rs`) groups of `ZTUnit`: twin
//! units, one run through the real body and one through the port, with the unit's virtual slots and the `BFUnit` tails
//! scripted so the calls made can be compared as well as the memory.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex, Once,
    },
};

use openzt_detour::generated::bfunit::{
    CHECK_ELEVATION as BFUNIT_CHECK_ELEVATION, CLIFFS_OK as BFUNIT_CLIFFS_OK, FACE_TOWARD_ENTITY as BFUNIT_FACE_TOWARD_ENTITY, FACE_TOWARD_TILE as BFUNIT_FACE_TOWARD_TILE,
    GET_PATH_0 as BFUNIT_GET_PATH, VALIDATE_POSITION as BFUNIT_VALIDATE_POSITION,
};

use super::super::ztanimal::{live_animal_types, Lcg};
use super::{
    check, compare_log, compare_memory, install_stubs, leaked_words, log, pin_rng, place_at_tile, rng_state, sample_tiles, take_log, Twins, STUB_BFUNIT,
};
use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztmegatilemgr::entity_type_matches,
    ztunit::{
        live_support::{real_movement, real_satisfiers},
        unit_offsets, ZTUnit,
    },
};

/// The thread running the current test: the game thread keeps calling the stubbed `BFUnit` methods on live units, and
/// those calls must reach the real bodies.
static TEST_THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);

fn stubbing() -> bool {
    STUB_BFUNIT.load(Ordering::SeqCst) != 0 && *TEST_THREAD.lock().unwrap() == Some(std::thread::current().id())
}

fn start_stubbing() {
    *TEST_THREAD.lock().unwrap() = Some(std::thread::current().id());
    STUB_BFUNIT.store(1, Ordering::SeqCst);
}

fn stop_stubbing() {
    STUB_BFUNIT.store(0, Ordering::SeqCst);
    *TEST_THREAD.lock().unwrap() = None;
}

/// What the stubbed `BFUnit` fallbacks answer.
static STUB_ANSWER: AtomicU32 = AtomicU32::new(0);
/// When non-negative, the scripted `BFUnit::getPath` answers true on exactly that call (counted from one) and false on the others.
static GET_PATH_SUCCEED_AT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
static GET_PATH_CALLS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
/// What the scripted virtual slots answer.
static VALIDATE_BUILDING_ANSWER: AtomicU32 = AtomicU32::new(0);
static CAN_WALK_ANSWER: AtomicU32 = AtomicU32::new(0);
static PREDATOR_ANSWER: AtomicU32 = AtomicU32::new(0);
static NEED_ANSWER: AtomicU32 = AtomicU32::new(0);
/// The habitat each tile maps to for the scripted `getHabitatFromTile` slot.
static HABITAT_OF: Mutex<Option<HashMap<u32, u32>>> = Mutex::new(None);
/// What the scripted `fRandomWalk` and `goToDest` collaborators answer.
static WALK_ANSWERS: Mutex<(bool, bool)> = Mutex::new((false, false));

const WORLD_MGR_RVA: u32 = 0x0023_8040;
const ANIMAL_TAG_RVA: u32 = 0x0023_8690;
const UNIT_TAG_RVA: u32 = 0x0023_86b0;
const BUILDING_TAG_RVA: u32 = 0x0023_8680;
const APP_LOADING_BYTE_RVA: u32 = 0x0023_8148 + 0x441;
const AIMGR_RVA: u32 = 0x0023_8098;

#[openzt_detour_macro::detour_mod]
mod target_stubs {
    use super::*;

    #[detour(BFUNIT_VALIDATE_POSITION)]
    unsafe extern "thiscall" fn validate_position(this: *const u32, flag: bool) {
        if stubbing() {
            log(format!("BFUnit::validatePosition {flag}"));
        } else {
            unsafe { BFUNIT_VALIDATE_POSITION_DETOUR.call(this, flag) }
        }
    }

    #[detour(BFUNIT_FACE_TOWARD_TILE)]
    unsafe extern "thiscall" fn face_toward_tile(this: *const u32, tile: i32) {
        if stubbing() {
            log(format!("BFUnit::faceTowardTile {tile:#x}"));
        } else {
            unsafe { BFUNIT_FACE_TOWARD_TILE_DETOUR.call(this, tile) }
        }
    }

    #[detour(BFUNIT_FACE_TOWARD_ENTITY)]
    unsafe extern "thiscall" fn face_toward_entity(this: *const u32, entity: i32) {
        if stubbing() {
            log(format!("BFUnit::faceTowardEntity {entity:#x}"));
        } else {
            unsafe { BFUNIT_FACE_TOWARD_ENTITY_DETOUR.call(this, entity) }
        }
    }

    #[detour(BFUNIT_CHECK_ELEVATION)]
    unsafe extern "thiscall" fn check_elevation(this: *const u32, from: *const u32, from_height: i32, to: *const u32, height: i32) -> bool {
        if stubbing() {
            log(format!("BFUnit::checkElevation {from_height} {height}"));
            STUB_ANSWER.load(Ordering::SeqCst) != 0
        } else {
            unsafe { BFUNIT_CHECK_ELEVATION_DETOUR.call(this, from, from_height, to, height) }
        }
    }

    #[detour(BFUNIT_GET_PATH)]
    unsafe extern "thiscall" fn get_path(this: *const u32, destination: *const i32, flag: u32) -> u32 {
        if stubbing() {
            let words: Vec<i32> = (0..3).map(|i| get_from_memory::<i32>(destination as u32 + i * 4)).collect();
            log(format!("BFUnit::getPath {words:?} {}", flag & 0xff));
            let succeed_at = GET_PATH_SUCCEED_AT.load(Ordering::SeqCst);
            if succeed_at >= 0 {
                let call = GET_PATH_CALLS.fetch_add(1, Ordering::SeqCst) + 1;
                return u32::from(call == succeed_at);
            }
            STUB_ANSWER.load(Ordering::SeqCst)
        } else {
            unsafe { BFUNIT_GET_PATH_DETOUR.call(this, destination, flag) }
        }
    }

    #[detour(BFUNIT_CLIFFS_OK)]
    unsafe extern "thiscall" fn cliffs_ok(this: *const u32, tile: *const u32, direction: u32) -> bool {
        if stubbing() {
            log(format!("BFUnit::cliffsOK {direction}"));
            STUB_ANSWER.load(Ordering::SeqCst) != 0
        } else {
            unsafe { BFUNIT_CLIFFS_OK_DETOUR.call(this, tile, direction) }
        }
    }
}

fn install_target_stubs() -> Result<(), String> {
    install_stubs()?;
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { target_stubs::init_detours() } {
            result = Err(format!("could not install the BFUnit target stubs: {e:?}"));
        }
        *crate::ztunit::live_support::intercept::RANDOM_WALK.lock().unwrap() = Some(scripted_random_walk);
    });
    result
}

fn scripted_random_walk(_unit: u32, go_to_dest: bool) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    let answers = *WALK_ANSWERS.lock().unwrap();
    let answer = if go_to_dest { answers.1 } else { answers.0 };
    log(format!("{} {answer}", if go_to_dest { "goToDest" } else { "fRandomWalk" }));
    Some(answer)
}

// ---- scripted virtual slots ------------------------------------------------------------------------------------

extern "thiscall" fn fake_validate_tank_position(_this: u32) {
    log("validateTankPosition".to_string());
}
extern "thiscall" fn fake_set_target_pos(_this: u32, position: u32) -> u32 {
    let words: Vec<i32> = (0..3).map(|i| get_from_memory::<i32>(position + i * 4)).collect();
    log(format!("setTargetPos {words:?}"));
    0
}
extern "thiscall" fn fake_validate_building(_this: u32, _out: u32, flag: u32) -> u32 {
    log(format!("validateBuilding {}", flag & 0xff));
    VALIDATE_BUILDING_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_habitat_from_tile(_this: u32, tile: u32) -> u32 {
    log("getHabitatFromTile".to_string());
    HABITAT_OF.lock().unwrap().as_ref().and_then(|map| map.get(&tile)).copied().unwrap_or(0)
}
extern "thiscall" fn fake_can_walk(_this: u32) -> u32 {
    log("canRandomWalk".to_string());
    CAN_WALK_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_abort_walk(_this: u32, arg: u32) {
    log(format!("abortWalk {}", arg & 0xff));
}
extern "thiscall" fn fake_find_predator(_this: u32, flag: u32) -> u32 {
    log(format!("findPredator {}", flag & 0xff));
    PREDATOR_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_is_need_satisfier(_this: u32, quadrants: u32, position: u32) -> u32 {
    log(format!("isNeedSatisfier {} {}", quadrants != 0, position != 0));
    NEED_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_set_visible(_this: u32, flag: u32) -> u32 {
    log(format!("setVisible {}", flag & 0xff));
    0
}
/// Order-sensitive digest of the tiles the scripted `canWalkTo` slot saw, and how many it saw.
static CAN_WALK_TO_DIGEST: AtomicU32 = AtomicU32::new(0);
static CAN_WALK_TO_CALLS: AtomicU32 = AtomicU32::new(0);
extern "thiscall" fn fake_can_walk_to(_this: u32, tile: u32, flag: u32) -> u32 {
    CAN_WALK_TO_CALLS.fetch_add(1, Ordering::SeqCst);
    let digest = CAN_WALK_TO_DIGEST.load(Ordering::SeqCst);
    CAN_WALK_TO_DIGEST.store(digest.rotate_left(5) ^ tile ^ (flag & 0xff), Ordering::SeqCst);
    u32::from((tile >> 2) % 5 < 2)
}
extern "thiscall" fn fake_can_walk_by_index(_this: u32, tile: u32, _flag: u32) -> u32 {
    u32::from((tile / 0x8c) % 3 != 0)
}
extern "thiscall" fn fake_set_destination(_this: u32, position: u32) -> u32 {
    let words: Vec<i32> = (0..3).map(|i| get_from_memory::<i32>(position + i * 4)).collect();
    log(format!("setDestination {words:?}"));
    0
}
extern "thiscall" fn fake_tank_yes(_this: u32) -> u32 {
    1
}
extern "thiscall" fn fake_tank_no(_this: u32) -> u32 {
    0
}

/// Layers more scripted slots over the unit's current vtable.
fn script_more(unit: ZTUnit) {
    let current: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    let scripted: [(u32, u32); 11] = [
        (0x168, fake_can_walk_to as *const () as u32),
        (0x1f8, fake_set_destination as *const () as u32),
        (0x178, fake_abort_walk as *const () as u32),
        (0x184, fake_can_walk as *const () as u32),
        (0x248, fake_habitat_from_tile as *const () as u32),
        (0x278, fake_is_need_satisfier as *const () as u32),
        (0x2d4, fake_validate_tank_position as *const () as u32),
        (0x2dc, fake_set_target_pos as *const () as u32),
        (0x2e8, fake_validate_building as *const () as u32),
        (0x2fc, fake_find_predator as *const () as u32),
        (0xcc, fake_set_visible as *const () as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    save_to_memory(unit.0, leaked_words(vtable));
}

/// Twins with every slot these tests dispatch through scripted. `Twins::destroy` restores the real vtables.
pub(super) fn scripted_twins(unit_type: u32) -> Twins {
    let twins = Twins::new(unit_type, true);
    for unit in twins.both() {
        script_more(unit);
    }
    twins
}

/// A fake habitat: a block with the tank fields `checkElevation`/`cliffsOK` read and a vtable whose `isTank` answers.
fn fake_habitat(is_tank: bool, ladder_a: i32, ladder_b: i32, base: i32) -> u32 {
    let source = leaked_words(vec![0; 0x20]);
    save_to_memory(source + 0x3c, base);
    let record = leaked_words(vec![0, 0, source, 0]);
    let walls = leaked_words(vec![record, 0]);
    let mut vtable = vec![0u32; 16];
    vtable[(0x20 / 4) as usize] = if is_tank { fake_tank_yes as *const () as u32 } else { fake_tank_no as *const () as u32 };
    let habitat = leaked_words(vec![0; 0x80]);
    save_to_memory(habitat, leaked_words(vtable));
    save_to_memory(habitat + 0x40, walls);
    save_to_memory(habitat + 0x184, ladder_a);
    save_to_memory(habitat + 0x188, ladder_b);
    habitat
}

fn set_habitats(pairs: &[(u32, u32)]) {
    *HABITAT_OF.lock().unwrap() = Some(pairs.iter().copied().collect());
}

pub(super) fn live_entities(tag_rva: u32) -> Vec<u32> {
    let manager: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + WORLD_MGR_RVA);
    let (begin, end): (u32, u32) = (get_from_memory(manager + 0x80), get_from_memory(manager + 0x84));
    (begin..end).step_by(4).map(get_from_memory::<u32>).filter(|entity| *entity != 0 && unsafe { entity_type_matches(*entity, tag_rva) }).collect()
}

pub(super) fn entity_id(entity: u32) -> u32 {
    get_from_memory(entity + unit_offsets::ENTITY_ID)
}

pub(super) fn no_units(test_name: &str, failure_log: &mut Option<std::fs::File>) -> bool {
    finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log)
}

/// `ZTUNIT_TARGETS_LIVE`: the target setters, the facing helpers, `validatePosition`, `getNonPathCost` and the two
/// target-unit lookups against the real bodies.
pub(crate) fn run_ztunit_targets_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_TARGETS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_target_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    let world = globals().ztworldmgr();
    let tiles = sample_tiles(24);
    let units = live_entities(UNIT_TAG_RVA);
    start_stubbing();
    let loading_byte = get_module_base("zoo.exe") as u32 + APP_LOADING_BYTE_RVA;
    let saved_loading: u8 = get_from_memory(loading_byte);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        let label = format!("type {type_index}");

        for loading in [0u8, 1] {
            save_to_memory(loading_byte, loading);
            for flag in [false, true] {
                take_log();
                real_movement::validate_position(twins.real.0, flag);
                let real_calls = take_log();
                twins.port.validate_position(flag);
                let port_calls = take_log();
                compare_log(&format!("{label} validatePosition({flag}) loading {loading}"), &real_calls, &port_calls, &mut failures);
            }
        }
        save_to_memory(loading_byte, saved_loading);

        for &(x, y) in &tiles {
            let tile = world.get_tile_ptr(x, y);
            for candidate in [0, tile] {
                for update in [false, true] {
                    for unit in twins.both() {
                        save_to_memory(unit.0 + unit_offsets::TARGET_TILE, 0u32);
                        for offset in 0..3 {
                            save_to_memory(unit.0 + unit_offsets::TARGET_POSITION + offset * 4, 0x5555u32 + offset);
                        }
                    }
                    real_movement::set_target_tile(twins.real.0, candidate, update);
                    twins.port.set_target_tile(candidate, update);
                    compare_memory(&format!("{label} setTargetTile({candidate:#x}, {update}) at ({x},{y})"), twins.real, twins.port, &mut failures);
                }
            }
            // setTargetPos: the tile centre, an offset position, the default position, and positions off the map.
            let positions: [[i32; 3]; 4] = [
                [x as i32 * 64 + 32, y as i32 * 64 + 32, 0],
                [x as i32 * 64 + 5, y as i32 * 64 + 63, 48],
                [world.map_x_size as i32 * 64 + 3, 10, 0],
                [-5, 100, 7],
            ];
            for position in positions {
                take_log();
                real_movement::set_target_pos(twins.real.0, &position);
                let real_calls = take_log();
                twins.port.set_target_pos(&position);
                let port_calls = take_log();
                compare_memory(&format!("{label} setTargetPos({position:?})"), twins.real, twins.port, &mut failures);
                compare_log(&format!("{label} setTargetPos({position:?})"), &real_calls, &port_calls, &mut failures);
            }
            let grid = [x as i32, y as i32, (x % 3) as i32];
            take_log();
            real_movement::set_target_grid_pos(twins.real.0, &grid);
            let real_calls = take_log();
            twins.port.set_target_grid_pos(&grid);
            let port_calls = take_log();
            compare_log(&format!("{label} setTargetGridPos({grid:?})"), &real_calls, &port_calls, &mut failures);
            if port_calls.len() != 1 {
                failures.push(format!("{label} setTargetGridPos made {} slot calls", port_calls.len()));
            }

            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::TARGET_TILE, if x % 2 == 0 { tile } else { 0 });
            }
            take_log();
            real_movement::face_toward_target(twins.real.0);
            let real_calls = take_log();
            twins.port.face_toward_target();
            let port_calls = take_log();
            compare_log(&format!("{label} faceTowardTarget at ({x},{y})"), &real_calls, &port_calls, &mut failures);
        }
        // The default position is "no position": setTargetPos must hand the virtual setter no tile.
        let default_at = get_module_base("zoo.exe") as u32 + 0x0023_9370;
        let default_position: [i32; 3] = [get_from_memory(default_at), get_from_memory(default_at + 4), get_from_memory(default_at + 8)];
        take_log();
        real_movement::set_target_pos(twins.real.0, &default_position);
        let real_calls = take_log();
        twins.port.set_target_pos(&default_position);
        let port_calls = take_log();
        compare_log(&format!("{label} setTargetPos(default)"), &real_calls, &port_calls, &mut failures);
        if port_calls != ["setTargetTile 0x0 0".to_string()] {
            failures.push(format!("{label} setTargetPos(default) calls: {port_calls:?}"));
        }

        for raw in [0u32, 1, 0x7fff_ffff, 12345] {
            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::NON_PATH_COST, raw);
            }
            check(&mut failures, &format!("{label} getNonPathCost({raw})"), real_movement::get_non_path_cost(twins.real.0), twins.port.get_non_path_cost());
        }

        // Target and predator lookups over every live unit id, a non-unit id and unknown ids.
        let mut ids: Vec<u32> = vec![0, 0x7fff_ffff, 1];
        ids.extend(units.iter().take(40).map(|&unit| entity_id(unit)));
        ids.extend(live_entities(BUILDING_TAG_RVA).iter().take(3).map(|&building| entity_id(building)));
        for id in ids {
            for unit in twins.both() {
                save_to_memory(unit.0 + 0x178, id);
                save_to_memory(unit.0 + unit_offsets::PREDATOR, id);
            }
            check(&mut failures, &format!("{label} getTargetUnit({id:#x})"), real_satisfiers::get_target_unit(twins.real.0), twins.port.get_target_unit());
            check(&mut failures, &format!("{label} getPredatorUnit({id:#x})"), real_satisfiers::get_predator_unit(twins.real.0), twins.port.get_predator_unit());
            take_log();
            crate::ztunit::live_support::real_movement::face_toward_target_unit(twins.real.0);
            let real_calls = take_log();
            twins.port.face_toward_target_unit();
            let port_calls = take_log();
            compare_log(&format!("{label} faceTowardTargetUnit({id:#x})"), &real_calls, &port_calls, &mut failures);
            check(&mut failures, &format!("{label} inFriendlyChase({id:#x})"), real_satisfiers::in_friendly_chase(twins.real.0), twins.port.in_friendly_chase());
        }
        // The hooked addresses reach the port.
        for unit in twins.both() {
            save_to_memory(unit.0 + unit_offsets::NON_PATH_COST, 77u32);
        }
        let hooked = unsafe { openzt_detour::generated::ztunit::GET_NON_PATH_COST.hooked()(twins.port.0 as *const u32) };
        check(&mut failures, &format!("{label} hooked getNonPathCost"), hooked, 77);
        twins.destroy();
    }
    stop_stubbing();
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_BLOCKING_LIVE`: `canIgnoreBlocking`, `checkElevation`, `cliffsOK` and `doRandomWalk` against the real bodies,
/// over synthetic ramp/tank layouts written onto live map tiles (restored afterwards).
pub(crate) fn run_ztunit_blocking_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_BLOCKING_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_target_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    let world = globals().ztworldmgr();
    let (width, height) = (world.map_x_size, world.map_y_size);
    let buildings = live_entities(BUILDING_TAG_RVA);
    start_stubbing();
    let mut rng = Lcg(0x51ed);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        let label = format!("type {type_index}");

        // canIgnoreBlocking.
        let spots: Vec<(u32, u32)> = (0..24).map(|_| (1 + rng.next() % (width - 2), 1 + rng.next() % (height - 2))).collect();
        for &(x, y) in &spots {
            let own_tile = world.get_tile_ptr(x, y);
            let other_tile = world.get_tile_ptr(x + 1, y);
            for unit in twins.both() {
                place_at_tile(unit, x, y);
            }
            for ignores in [0u8, 1] {
                for unit in twins.both() {
                    save_to_memory(unit.0 + 0x210, ignores);
                }
                for target in [0, own_tile, other_tile] {
                    for working_at in std::iter::once(0).chain(buildings.iter().take(3).copied()) {
                        VALIDATE_BUILDING_ANSWER.store(working_at, Ordering::SeqCst);
                        take_log();
                        let real_answer = real_movement::can_ignore_blocking(twins.real.0, target);
                        let real_calls = take_log();
                        let port_answer = twins.port.can_ignore_blocking(target);
                        let port_calls = take_log();
                        let case = format!("{label} canIgnoreBlocking({target:#x}) ignores {ignores} working_at {working_at:#x} at ({x},{y})");
                        check(&mut failures, &case, real_answer, port_answer);
                        compare_log(&case, &real_calls, &port_calls, &mut failures);
                    }
                }
            }
        }

        // checkElevation over fake tank habitats: ramp flags set on two live tiles for the duration of each case.
        let tiles = [world.get_tile_ptr(3, 3), world.get_tile_ptr(4, 3), world.get_tile_ptr(3, 4)];
        let saved_flags: Vec<u8> = tiles.iter().map(|&tile| get_from_memory(tile + 0x85)).collect();
        let tank_a = fake_habitat(true, 0, 3, 2);
        let tank_b = fake_habitat(true, 1, 5, 4);
        let plain = fake_habitat(false, 0, 0, 0);
        let habitats = [0, tank_a, tank_b, plain];
        for case in 0..400u32 {
            let ramp = [rng.next() % 2 == 0, rng.next() % 2 == 0, rng.next() % 2 == 0];
            for (index, &tile) in tiles.iter().enumerate() {
                let flags = saved_flags[index];
                save_to_memory(tile + 0x85, if ramp[index] { flags | 0x20 } else { flags & !0x20 });
            }
            let pick = |rng: &mut Lcg| habitats[(rng.next() % 4) as usize];
            set_habitats(&[(tiles[0], pick(&mut rng)), (tiles[1], pick(&mut rng)), (tiles[2], pick(&mut rng))]);
            STUB_ANSWER.store(rng.next() % 2, Ordering::SeqCst);
            let (from_height, height_arg) = ((rng.next() % 8) as i32 - 1, (rng.next() % 8) as i32 - 1);
            let (from_index, to_index) = ((rng.next() % 3) as usize, (rng.next() % 3) as usize);
            let (from, to) = (tiles[from_index], tiles[to_index]);
            // Vanilla dereferences a null source habitat when the destination is a tank ramp with a different habitat.
            let habitat_of = |tile: u32| HABITAT_OF.lock().unwrap().as_ref().and_then(|map| map.get(&tile)).copied().unwrap_or(0);
            let (from_habitat, to_habitat) = (habitat_of(from), habitat_of(to));
            if ramp[to_index] && (to_habitat == tank_a || to_habitat == tank_b) && from_habitat == 0 && from_habitat != to_habitat {
                continue;
            }
            take_log();
            let real_answer = real_movement::check_elevation(twins.real.0, from, from_height, to, height_arg);
            let real_calls = take_log();
            let port_answer = twins.port.check_elevation(from, from_height, to, height_arg);
            let port_calls = take_log();
            let name = format!("{label} checkElevation case {case} ramp {ramp:?} heights {from_height} {height_arg}");
            check(&mut failures, &name, real_answer, port_answer);
            compare_log(&name, &real_calls, &port_calls, &mut failures);
        }

        // cliffsOK: a live tile and each direction's neighbour, with ramp flags and habitats scripted on the pair.
        let mut cliff_cases = 0;
        for _ in 0..30 {
            let (x, y) = (1 + rng.next() % (width - 2), 1 + rng.next() % (height - 2));
            let tile = world.get_tile_ptr(x, y);
            for direction in 0..8u32 {
                let neighbour = unsafe { openzt_detour::generated::bfmap::GET_NEIGHBOR_0.hooked()((get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + WORLD_MGR_RVA) + 8) as *const u32, tile as *const u32, direction) };
                if neighbour == 0 {
                    continue;
                }
                let saved = [get_from_memory::<u8>(tile + 0x85), get_from_memory::<u8>(neighbour + 0x85)];
                for ramps in 0..4u32 {
                    save_to_memory(tile + 0x85, if ramps & 1 != 0 { saved[0] | 0x20 } else { saved[0] & !0x20 });
                    save_to_memory(neighbour + 0x85, if ramps & 2 != 0 { saved[1] | 0x20 } else { saved[1] & !0x20 });
                    for _ in 0..4 {
                        // A ladder level equal to a real corner elevation of the neighbour/tile makes the equal branches reachable.
                        let corner = unsafe { openzt_detour::generated::bftile::GET_CORNER_ELEVATION.hooked()(tile as *const u32, (direction & 7) as i32) };
                        let level_tank = fake_habitat(true, corner, 0, 0);
                        let habitats = [0, tank_a, tank_b, plain, level_tank, level_tank];
                        set_habitats(&[(tile, habitats[(rng.next() % 6) as usize]), (neighbour, habitats[(rng.next() % 6) as usize])]);
                        STUB_ANSWER.store(rng.next() % 2, Ordering::SeqCst);
                        take_log();
                        let real_answer = real_movement::cliffs_ok(twins.real.0, tile, direction);
                        let real_calls = take_log();
                        let port_answer = twins.port.cliffs_ok(tile, direction);
                        let port_calls = take_log();
                        let name = format!("{label} cliffsOK({direction}) at ({x},{y}) ramps {ramps}");
                        check(&mut failures, &name, real_answer, port_answer);
                        compare_log(&name, &real_calls, &port_calls, &mut failures);
                        cliff_cases += 1;
                    }
                }
                save_to_memory(tile + 0x85, saved[0]);
                save_to_memory(neighbour + 0x85, saved[1]);
            }
        }
        for (index, &tile) in tiles.iter().enumerate() {
            save_to_memory(tile + 0x85, saved_flags[index]);
        }
        set_habitats(&[]);
        tracing::info!("{test_name}: {cliff_cases} cliffsOK cases");

        // doRandomWalk: with the walk refused the real body compares directly; otherwise vanilla runs the (ported)
        // `fRandomWalk` itself, so the port's collaborator calls are checked against the expected sequence instead.
        for can_walk in [0u32, 1] {
            for walk in [false, true] {
                for to_dest in [false, true] {
                    CAN_WALK_ANSWER.store(can_walk, Ordering::SeqCst);
                    *WALK_ANSWERS.lock().unwrap() = (walk, to_dest);
                    for unit in twins.both() {
                        save_to_memory(unit.0 + unit_offsets::COG_INVALID, 1u8);
                    }
                    let name = format!("{label} doRandomWalk can_walk {can_walk} walk {walk} to_dest {to_dest}");
                    take_log();
                    let port_answer = twins.port.do_random_walk();
                    let port_calls = take_log();
                    let mut expected: Vec<String> = vec!["canRandomWalk".to_string()];
                    if can_walk != 0 {
                        expected.extend(["clearGoal".to_string(), "goalSet 0x1 4".to_string(), format!("fRandomWalk {walk}")]);
                        if walk {
                            expected.push(format!("goToDest {to_dest}"));
                        }
                        if !(walk && to_dest) {
                            expected.push("abortWalk 0".to_string());
                        }
                    }
                    check(&mut failures, &name, port_answer, can_walk != 0 && walk && to_dest);
                    compare_log(&name, &expected, &port_calls, &mut failures);
                    check(&mut failures, &format!("{name} +0x20e"), get_from_memory::<u8>(twins.port.0 + unit_offsets::COG_INVALID), if can_walk != 0 { 0 } else { 1 });
                    if can_walk == 0 {
                        take_log();
                        let real_answer = real_movement::do_random_walk(twins.real.0);
                        let real_calls = take_log();
                        check(&mut failures, &name, real_answer, port_answer);
                        compare_log(&name, &real_calls, &port_calls, &mut failures);
                    }
                }
            }
        }
        twins.destroy();
    }
    stop_stubbing();
    pin_rng(0x1234_5678);
    let _ = rng_state();
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_SATISFIERS_LIVE`: the predator and chase checks, `isFunSatisfier`/`isNeedOrFunSatisfier`, `caughtDone`,
/// `findNearbyNeedSatisfier` and `releasePredator` against the real bodies.
pub(crate) fn run_ztunit_satisfiers_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_SATISFIERS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_target_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = super::live_unit_types(3);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    let base = get_module_base("zoo.exe") as u32;
    let animals = live_entities(ANIMAL_TAG_RVA);
    let units = live_entities(UNIT_TAG_RVA);
    let buildings = live_entities(BUILDING_TAG_RVA);
    start_stubbing();
    let (mut staff_twins, mut staff_hits, mut animal_releases) = (0u32, 0u32, 0u32);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        let label = format!("type {type_index}");

        // checkPredator over every live animal, a non-animal and null, with the id the chase check compares.
        let mut candidates: Vec<u32> = vec![0];
        candidates.extend(animals.iter().take(30));
        candidates.extend(units.iter().filter(|unit| !animals.contains(unit)).take(3));
        candidates.extend(buildings.iter().take(2));
        for &candidate in &candidates {
            for chasing in [false, true] {
                for matching_id in [false, true] {
                    let own_id = if matching_id && candidate != 0 { get_from_memory::<u32>(candidate + 0x2e4) } else { 7 };
                    for unit in twins.both() {
                        save_to_memory(unit.0 + unit_offsets::ENTITY_ID, own_id);
                    }
                    check(
                        &mut failures,
                        &format!("{label} checkPredator({candidate:#x}, {chasing}, matching {matching_id})"),
                        real_satisfiers::check_predator(twins.real.0, candidate, chasing),
                        twins.port.check_predator(candidate, chasing),
                    );
                }
            }
        }

        // checkPredator's staff branch: for a staff unit, candidates are made to hunt its type (a prey list holding the unit
        // type's id, and the any-type flag) and the unit is given a world habitat, so the branch decides by the habitat's
        // world flag and the candidate's exemption byte. Everything patched is restored.
        if unsafe { entity_type_matches(twins.port.0, STAFF_TAG_RVA) } {
            staff_twins += 1;
            let own_type: u32 = get_from_memory(twins.port.0 + unit_offsets::ENTITY_TYPE);
            let own_type_id = crate::ztkeeper::call_slot_0(own_type, 0x20);
            let prey_list = leaked_words(vec![own_type_id]);
            let world_flag_off = leaked_words(vec![0; 0x20]);
            let world_flag_on = leaked_words(vec![0; 0x20]);
            save_to_memory(world_flag_on + 0x2c, 1u8);
            let saved_slots: Vec<u32> = twins.both().iter().map(|unit| get_from_memory::<u32>(get_from_memory::<u32>(unit.0) + 0x24c)).collect();
            for unit in twins.both() {
                save_to_memory(get_from_memory::<u32>(unit.0) + 0x24c, fake_world_habitat as *const () as u32);
            }
            for &candidate in animals.iter().take(4) {
                let candidate_type: u32 = get_from_memory(candidate + unit_offsets::ENTITY_TYPE);
                let saved = [
                    get_from_memory::<u32>(candidate_type + 0x2e4),
                    get_from_memory::<u32>(candidate_type + 0x2e8),
                    u32::from(get_from_memory::<u8>(candidate_type + 0x3d4)),
                    u32::from(get_from_memory::<u8>(candidate + 0x393)),
                ];
                save_to_memory(candidate_type + 0x2e4, prey_list);
                save_to_memory(candidate_type + 0x2e8, prey_list + 4);
                save_to_memory(candidate_type + 0x3d4, 1u8);
                // Vanilla dereferences a null habitat here (the port answers false), so that case is checked on the port alone.
                FAKE_WORLD_HABITAT.store(0, Ordering::SeqCst);
                check(&mut failures, &format!("{label} staff checkPredator({candidate:#x}, no habitat)"), false, twins.port.check_predator(candidate, false));
                for habitat in [world_flag_off, world_flag_on] {
                    FAKE_WORLD_HABITAT.store(habitat, Ordering::SeqCst);
                    for exempt in [0u8, 1] {
                        save_to_memory(candidate + 0x393, exempt);
                        let real_answer = real_satisfiers::check_predator(twins.real.0, candidate, false);
                        let port_answer = twins.port.check_predator(candidate, false);
                        check(&mut failures, &format!("{label} staff checkPredator({candidate:#x}, habitat {habitat:#x}, exempt {exempt})"), real_answer, port_answer);
                        staff_hits += u32::from(port_answer);
                    }
                }
                save_to_memory(candidate_type + 0x2e4, saved[0]);
                save_to_memory(candidate_type + 0x2e8, saved[1]);
                save_to_memory(candidate_type + 0x3d4, saved[2] as u8);
                save_to_memory(candidate + 0x393, saved[3] as u8);
            }
            FAKE_WORLD_HABITAT.store(0, Ordering::SeqCst);
            for (unit, slot) in twins.both().iter().zip(saved_slots) {
                save_to_memory(get_from_memory::<u32>(unit.0) + 0x24c, slot);
            }
        }

        // isFunSatisfier / isNeedOrFunSatisfier over fake quadrant tables of live buildings.
        let mut positions: Vec<[i32; 3]> = vec![[0, 0, 0], [31, 31, 0], [32, 0, 0], [0, 32, 0], [63, 63, 0], [64, 0, 0], [0, 64, 0], [-1, 5, 0], [5, -1, 0]];
        let mut rng = Lcg(0xfa11 + type_index as u32);
        positions.extend((0..12).map(|_| [(rng.next() % 70) as i32 - 3, (rng.next() % 70) as i32 - 3, 0]));
        let mut quadrant_sets: Vec<u32> = vec![0];
        for _ in 0..12 {
            let pick = |rng: &mut Lcg| -> u32 {
                match rng.next() % 4 {
                    0 => 0,
                    1 => animals.first().copied().unwrap_or(0),
                    _ => buildings.get((rng.next() as usize) % buildings.len().max(1)).copied().unwrap_or(0),
                }
            };
            quadrant_sets.push(leaked_words(vec![0, pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), 0]));
        }
        for &quadrants in &quadrant_sets {
            for position in &positions {
                check(
                    &mut failures,
                    &format!("{label} isFunSatisfier({quadrants:#x}, {position:?})"),
                    real_satisfiers::is_fun_satisfier(quadrants, position.as_ptr()),
                    ZTUnit::is_fun_satisfier(quadrants, position.as_ptr()),
                );
                for need in [0u32, 1] {
                    NEED_ANSWER.store(need, Ordering::SeqCst);
                    take_log();
                    let real_answer = real_satisfiers::is_need_or_fun_satisfier(twins.real.0, quadrants, position.as_ptr());
                    let real_calls = take_log();
                    let port_answer = twins.port.is_need_or_fun_satisfier(quadrants, position.as_ptr());
                    let port_calls = take_log();
                    let name = format!("{label} isNeedOrFunSatisfier({quadrants:#x}, {position:?}, need {need})");
                    check(&mut failures, &name, real_answer, port_answer);
                    compare_log(&name, &real_calls, &port_calls, &mut failures);
                }
            }
        }

        // avoidDone / chaseDone: threat memory expired or not, friendly chase or not, scripted findPredator.
        let aimgr: u32 = get_from_memory(base + AIMGR_RVA);
        let tick: u32 = get_from_memory(aimgr + 0xec);
        let memory: u32 = get_from_memory(base + 0x0023_9524);
        for last in [tick, tick.wrapping_sub(memory.wrapping_add(1)), tick.wrapping_sub(memory)] {
            for &target in animals.iter().take(3).chain(units.iter().take(2)) {
                for unit in twins.both() {
                    save_to_memory(unit.0 + 0x138, last);
                    save_to_memory(unit.0 + 0x178, entity_id(target));
                }
                for predator in [0u32, 5] {
                    PREDATOR_ANSWER.store(predator, Ordering::SeqCst);
                    take_log();
                    let real_answer = real_satisfiers::avoid_done(twins.real.0, 0);
                    let real_calls = take_log();
                    let port_answer = twins.port.avoid_done();
                    let port_calls = take_log();
                    let name = format!("{label} avoidDone last {last:#x} target {target:#x} predator {predator}");
                    check(&mut failures, &name, real_answer, port_answer);
                    compare_log(&name, &real_calls, &port_calls, &mut failures);
                }
                for &other in units.iter().take(4) {
                    place_at_tile(twins.real, 5, 5);
                    place_at_tile(twins.port, 5, 5);
                    check(
                        &mut failures,
                        &format!("{label} chaseDone last {last:#x} target {target:#x} other {other:#x}"),
                        real_satisfiers::chase_done(twins.real.0, other),
                        twins.port.chase_done(other),
                    );
                }
            }
        }

        // caughtDone over behaviour names. The string words are vanilla-owned: put them back before the unit is destroyed.
        let saved_name: Vec<[u32; 3]> = twins.both().iter().map(|unit| [get_from_memory(unit.0 + 0x54), get_from_memory(unit.0 + 0x58), get_from_memory(unit.0 + 0x5c)]).collect();
        for name in ["", "idle", "bCaughtBy", "xbCaughtByz", "bDefense", "bDefens", "walk bDefense run", "BCAUGHTBY"] {
            let mut bytes = name.as_bytes().to_vec();
            bytes.push(0);
            let begin = Box::leak(bytes.into_boxed_slice()).as_ptr() as u32;
            for unit in twins.both() {
                save_to_memory(unit.0 + 0x54, begin);
                save_to_memory(unit.0 + 0x58, begin + name.len() as u32);
            }
            check(&mut failures, &format!("{label} caughtDone({name:?})"), real_satisfiers::caught_done(twins.real.0), twins.port.caught_done());
        }

        for (unit, words) in twins.both().iter().zip(&saved_name) {
            for (index, word) in words.iter().enumerate() {
                save_to_memory(unit.0 + 0x54 + index as u32 * 4, *word);
            }
        }

        // findNearbyNeedSatisfier writes the same out values.
        let (mut real_need, mut real_building) = (0x11u8, 0x2222u32);
        let (mut port_need, mut port_building) = (0x11u8, 0x2222u32);
        let real_value = real_satisfiers::find_nearby_need_satisfier(twins.real.0, &mut real_need, &mut real_building);
        let port_value = ZTUnit::find_nearby_need_satisfier(&mut port_need, &mut port_building);
        check(&mut failures, &format!("{label} findNearbyNeedSatisfier outputs"), (real_need, real_building), (port_need, port_building));
        // Each answer is its own out-pointer with the low byte cleared, whatever the stack layout puts it at.
        check(&mut failures, &format!("{label} findNearbyNeedSatisfier value"), (real_value, port_value), (&real_building as *const u32 as u32 & 0xffff_ff00, &port_building as *const u32 as u32 & 0xffff_ff00));

        // releasePredator with no predator and with an unknown id leaves the same unit memory.
        for id in [0u32, 0x7fff_fff0] {
            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::PREDATOR, id);
            }
            take_log();
            real_satisfiers::release_predator(twins.real.0);
            let real_calls = take_log();
            twins.port.release_predator();
            let port_calls = take_log();
            compare_memory(&format!("{label} releasePredator({id:#x})"), twins.real, twins.port, &mut failures);
            compare_log(&format!("{label} releasePredator({id:#x})"), &real_calls, &port_calls, &mut failures);
        }
        // With a live non-animal unit as the predator both bodies only make it visible.
        if let Some(&other) = units.iter().find(|unit| !animals.contains(unit)) {
            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::PREDATOR, entity_id(other));
            }
            real_satisfiers::release_predator(twins.real.0);
            twins.port.release_predator();
            compare_memory(&format!("{label} releasePredator(non-animal unit)"), twins.real, twins.port, &mut failures);
        }
        // With a live animal as the predator both bodies make it visible, clear its caught flags and clear its goal. The animal's
        // vtable is swapped for a copy whose `setVisible` and `clearGoal` only log, so the live animal is left alone.
        if let Some(&animal) = animals.first() {
            let real_vtable: u32 = get_from_memory(animal);
            let copy = leaked_words((0..0x100u32).map(|index| get_from_memory::<u32>(real_vtable + index * 4)).collect());
            save_to_memory(copy + 0xcc, fake_set_visible as *const () as u32);
            save_to_memory(copy + 0x174, fake_clear_goal as *const () as u32);
            save_to_memory(animal, copy);
            let saved_flags = [get_from_memory::<u8>(animal + 0x3a5), get_from_memory::<u8>(animal + 0x20d)];
            let mut outcomes = Vec::new();
            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::PREDATOR, entity_id(animal));
            }
            for poles in 0..2 {
                save_to_memory(animal + 0x3a5, 1u8);
                save_to_memory(animal + 0x20d, 1u8);
                take_log();
                if poles == 0 {
                    real_satisfiers::release_predator(twins.real.0);
                } else {
                    twins.port.release_predator();
                }
                let calls = take_log();
                outcomes.push((calls, get_from_memory::<u8>(animal + 0x3a5), get_from_memory::<u8>(animal + 0x20d)));
            }
            save_to_memory(animal, real_vtable);
            save_to_memory(animal + 0x3a5, saved_flags[0]);
            save_to_memory(animal + 0x20d, saved_flags[1]);
            check(&mut failures, &format!("{label} releasePredator(animal)"), &outcomes[0], &outcomes[1]);
            compare_memory(&format!("{label} releasePredator(animal)"), twins.real, twins.port, &mut failures);
            animal_releases += u32::from(outcomes[1].0.len() == 2 && outcomes[1].1 == 0 && outcomes[1].2 == 0);
        }
        twins.destroy();
    }
    stop_stubbing();
    if staff_twins == 0 || staff_hits == 0 {
        failures.push(format!("checkPredator's staff branch never answered true (staff units {staff_twins}, hits {staff_hits})"));
    }
    if animal_releases == 0 {
        failures.push("releasePredator's animal branch was never exercised".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

extern "thiscall" fn fake_clear_goal(_this: u32) {
    log("clearGoal".to_string());
}

/// `ZTUNIT_PATHS_LIVE`: `getPathToTarget` and `pickRandomDest` against the real bodies with the game RNG pinned,
/// `BFUnit::getPath` recorded and the destination slots scripted.
pub(crate) fn run_ztunit_paths_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_PATHS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_target_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    let world = globals().ztworldmgr();
    let (width, height) = (world.map_x_size, world.map_y_size);
    let units = live_entities(UNIT_TAG_RVA);
    start_stubbing();
    let mut with_steps = 0;
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        let label = format!("type {type_index}");
        let mut targets: Vec<u32> = vec![0];
        // Vanilla dereferences a null unit for a non-unit target, so only units (and no target) are compared.
        targets.extend(units.iter().take(40));
        let tiles: Vec<(u32, u32)> = sample_tiles(10).into_iter().filter(|&(x, y)| x >= 2 && y >= 2 && x + 2 < width && y + 2 < height).collect();
        for &target in &targets {
            let id = if target == 0 { 0 } else { entity_id(target) };
            if target != 0 && get_from_memory::<u32>(target + 0x154) != 0 {
                with_steps += 1;
            }
            for unit in twins.both() {
                save_to_memory(unit.0 + 0x178, id);
            }
            for &(x, y) in &tiles {
                place_at_tile(twins.real, x, y);
                place_at_tile(twins.port, x, y);
                for flag in [false, true] {
                    for seed in [0x1111u32, 0x7a7a_7a7a] {
                        take_log();
                        pin_rng(seed);
                        let real_value = real_movement::get_path_to_target(twins.real.0, flag);
                        let real_calls = take_log();
                        let real_rng = rng_state();
                        pin_rng(seed);
                        let port_value = twins.port.get_path_to_target(flag);
                        let port_calls = take_log();
                        let port_rng = rng_state();
                        let name = format!("{label} getPathToTarget target {target:#x} at ({x},{y}) flag {flag} seed {seed:#x}");
                        check(&mut failures, &format!("{name} value"), real_value & 0xff, port_value & 0xff);
                        check(&mut failures, &format!("{name} rng"), real_rng, port_rng);
                        compare_log(&name, &real_calls, &port_calls, &mut failures);
                    }
                }
            }
        }

        // pickRandomDest around interior tiles.
        let mut picked = 0;
        for &(x, y) in &tiles {
          // Outside the zoo no tile carries the walkable bit; the second pass sets it on the whole neighbourhood (restored
          // afterwards) so the scripted `tileFilter` decides.
          for walkable in [false, true] {
            let neighbourhood: Vec<u32> = (-6i32..=6)
                .flat_map(|dy| (-6i32..=6).map(move |dx| (dx, dy)))
                .filter(|&(dx, dy)| x as i32 + dx >= 0 && y as i32 + dy >= 0 && ((x as i32 + dx) as u32) < width && ((y as i32 + dy) as u32) < height)
                .map(|(dx, dy)| world.get_tile_ptr((x as i32 + dx) as u32, (y as i32 + dy) as u32))
                .collect();
            let saved: Vec<u8> = neighbourhood.iter().map(|&tile| get_from_memory(tile + 0x83)).collect();
            if walkable {
                for &tile in &neighbourhood {
                    save_to_memory(tile + 0x83, get_from_memory::<u8>(tile + 0x83) | 8);
                }
            }
            for seed in [1u32, 0x5eed_0001, 0xdead_beef] {
                for unit in twins.both() {
                    place_at_tile(unit, x, y);
                }
                let (mut real_out, mut port_out) = ([0x77i32; 3], [0x77i32; 3]);
                CAN_WALK_TO_DIGEST.store(0, Ordering::SeqCst);
                CAN_WALK_TO_CALLS.store(0, Ordering::SeqCst);
                take_log();
                pin_rng(seed);
                let real_value = real_movement::pick_random_dest(twins.real.0, &mut real_out);
                let real_calls = take_log();
                let real_rng = rng_state();
                let real_walk = (CAN_WALK_TO_DIGEST.load(Ordering::SeqCst), CAN_WALK_TO_CALLS.load(Ordering::SeqCst));
                CAN_WALK_TO_DIGEST.store(0, Ordering::SeqCst);
                CAN_WALK_TO_CALLS.store(0, Ordering::SeqCst);
                pin_rng(seed);
                let port_value = twins.port.pick_random_dest(&mut port_out);
                let port_calls = take_log();
                let port_rng = rng_state();
                let port_walk = (CAN_WALK_TO_DIGEST.load(Ordering::SeqCst), CAN_WALK_TO_CALLS.load(Ordering::SeqCst));
                let name = format!("{label} pickRandomDest at ({x},{y}) walkable {walkable} seed {seed:#x}");
                check(&mut failures, &format!("{name} value"), real_value, port_value);
                check(&mut failures, &format!("{name} out"), real_out, port_out);
                check(&mut failures, &format!("{name} rng"), real_rng, port_rng);
                check(&mut failures, &format!("{name} candidates"), real_walk, port_walk);
                compare_log(&name, &real_calls, &port_calls, &mut failures);
                picked += u32::from(port_value);
            }
            for (&tile, &flags) in neighbourhood.iter().zip(&saved) {
                save_to_memory(tile + 0x83, flags);
            }
          }
        }
        if picked == 0 {
            failures.push(format!("{label} pickRandomDest never found a tile"));
        }
        twins.destroy();
    }
    tracing::info!("{test_name}: {with_steps} targets with path steps");
    stop_stubbing();
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_Z_DEST_LIVE`: `pickZDest` over the depth modes, water levels, type minimums and unit heights, with a scripted
/// `getHabitatFromTile` answering tank, non-tank or no habitat for the destination's tile.
pub(crate) fn run_ztunit_z_dest_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_Z_DEST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_target_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    let world = globals().ztworldmgr();
    let tile = world.get_tile_ptr(world.map_x_size / 2, world.map_y_size / 2);
    let (tile_x, tile_y) = ((world.map_x_size / 2) as i32, (world.map_y_size / 2) as i32);
    let modes: [&[u8]; 9] = [b"surface", b"dive", b"rise", b"level", b"floor", b"other", b"divx", b"", b"surfaces"];
    let mode_buffers: Vec<(u32, u32)> = modes
        .iter()
        .map(|mode| {
            let buffer = leaked_words(vec![0; mode.len() / 4 + 2]);
            for (index, byte) in mode.iter().enumerate() {
                save_to_memory(buffer + index as u32, *byte);
            }
            (buffer, buffer + mode.len() as u32)
        })
        .collect();
    start_stubbing();
    let (mut compared, mut moved_off_surface) = (0u32, 0u32);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        let original_minimum: i32 = get_from_memory(unit_type + 0x68);
        // The unit's destructor frees the depth-mode buffer, so the leaked test buffers must be unhooked again first.
        let original_modes: Vec<(u32, u32)> = twins.both().iter().map(|unit| (get_from_memory(unit.0 + 0x1ec), get_from_memory(unit.0 + 0x1f0))).collect();
        for (kind, is_tank) in [("tank", true), ("pool", false), ("none", true)] {
            for water in [0i32, 3, 10] {
                let habitat = fake_habitat(is_tank, 0, water, 0);
                set_habitats(&if kind == "none" { vec![] } else { vec![(tile, habitat)] });
                for minimum in [-3i32, 0, 2, 5] {
                    save_to_memory(unit_type + 0x68, minimum);
                    for (mode_index, &(begin, end)) in mode_buffers.iter().enumerate() {
                        for unit in twins.both() {
                            save_to_memory(unit.0 + 0x1ec, begin);
                            save_to_memory(unit.0 + 0x1f0, end);
                        }
                        for grid_z in [-2i32, 0, 3, 8, 20] {
                            for unit in twins.both() {
                                place_at_tile(unit, tile_x as u32, tile_y as u32);
                                save_to_memory(unit.0 + unit_offsets::POS_Z, grid_z.wrapping_mul(16));
                            }
                            for dest_z in [-3i32, 0, 5] {
                                for seed in [0x1234u32, 0x6000_0001] {
                                    let name = format!("type {type_index} {kind} water {water} min {minimum} mode {:?} gridZ {grid_z} destZ {dest_z} seed {seed:#x}", String::from_utf8_lossy(modes[mode_index]));
                                    let (mut real_pos, mut port_pos) = ([tile_x, tile_y, dest_z], [tile_x, tile_y, dest_z]);
                                    take_log();
                                    pin_rng(seed);
                                    real_movement::pick_z_dest(twins.real.0, &mut real_pos);
                                    let real_calls = take_log();
                                    let real_rng = rng_state();
                                    pin_rng(seed);
                                    twins.port.pick_z_dest(&mut port_pos);
                                    let port_calls = take_log();
                                    let port_rng = rng_state();
                                    check(&mut failures, &format!("{name} position"), real_pos, port_pos);
                                    check(&mut failures, &format!("{name} rng"), real_rng, port_rng);
                                    compare_log(&name, &real_calls, &port_calls, &mut failures);
                                    compared += 1;
                                    if is_tank && kind == "tank" && modes[mode_index] != b"surface" && port_pos[2] != dest_z.wrapping_add(water) {
                                        moved_off_surface += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // A destination off the map has no tile at all.
        for unit in twins.both() {
            save_to_memory(unit.0 + 0x1ec, mode_buffers[1].0);
            save_to_memory(unit.0 + 0x1f0, mode_buffers[1].1);
        }
        set_habitats(&[(tile, fake_habitat(true, 0, 6, 0))]);
        let (mut real_pos, mut port_pos) = ([-4, -4, 2], [-4, -4, 2]);
        real_movement::pick_z_dest(twins.real.0, &mut real_pos);
        twins.port.pick_z_dest(&mut port_pos);
        check(&mut failures, &format!("type {type_index} off-map position"), real_pos, port_pos);
        save_to_memory(unit_type + 0x68, original_minimum);
        for (unit, (begin, end)) in twins.both().iter().zip(original_modes) {
            save_to_memory(unit.0 + 0x1ec, begin);
            save_to_memory(unit.0 + 0x1f0, end);
        }
        twins.destroy();
    }
    tracing::info!("{test_name}: {compared} comparisons, {moved_off_surface} off the surface height");
    if moved_off_surface == 0 {
        failures.push("no case reached a branch other than the surface height".to_string());
    }
    stop_stubbing();
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_PATH_AWAY_LIVE`: `getPathAwayFromTarget` against the real body. `BFUnit::getPath` succeeds on a chosen call, so
/// the real body never reaches its `fRandomWalk` fallback; the fallback is checked on the port alone, through the
/// scripted random walk, including the path-step nodes returning to vanilla's freelist.
pub(crate) fn run_ztunit_path_away_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_PATH_AWAY_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_target_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    let units = live_entities(UNIT_TAG_RVA);
    let freelist = get_module_base("zoo.exe") as u32 + 0x0023_8008;
    *WALK_ANSWERS.lock().unwrap() = (true, false);
    start_stubbing();
    let (mut real_compared, mut deepest) = (0u32, 0i32);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        // `fake_can_walk_to` answers the same for every tile (the tile stride is a multiple of five words), so the ring
        // search gets a predicate that varies with the tile's index instead.
        for unit in twins.both() {
            let vtable: u32 = get_from_memory(unit.0);
            save_to_memory(vtable + 0x168, fake_can_walk_by_index as *const () as u32);
        }
        for &target in units.iter().take(4) {
            for unit in twins.both() {
                save_to_memory(unit.0 + 0x178, entity_id(target));
            }
            for &(x, y) in &sample_tiles(10) {
                for unit in twins.both() {
                    place_at_tile(unit, x, y);
                }
                let name = format!("type {type_index} target {target:#x} at ({x},{y})");
                // Port alone, never succeeding: counts the candidates and runs the fallback.
                GET_PATH_SUCCEED_AT.store(0, Ordering::SeqCst);
                GET_PATH_CALLS.store(0, Ordering::SeqCst);
                let sentinel: u32 = get_from_memory(twins.port.0 + 0x154);
                let nodes: Vec<u32> = (0..3).map(|_| leaked_words(vec![0; 5])).collect();
                let old_head: u32 = get_from_memory(freelist);
                save_to_memory(sentinel, nodes[0]);
                save_to_memory(sentinel + 4, nodes[2]);
                for (index, &node) in nodes.iter().enumerate() {
                    save_to_memory(node, if index == 2 { sentinel } else { nodes[index + 1] });
                    save_to_memory(node + 4, if index == 0 { sentinel } else { nodes[index - 1] });
                }
                take_log();
                let reached = twins.port.get_path_away_from_target();
                let port_calls = take_log();
                let total = GET_PATH_CALLS.load(Ordering::SeqCst);
                let reaches_fallback = port_calls.last().is_some_and(|entry| entry.starts_with("fRandomWalk"));
                check(&mut failures, &format!("{name} fallback answer"), reached, reaches_fallback && WALK_ANSWERS.lock().unwrap().0);
                if reaches_fallback {
                    check(&mut failures, &format!("{name} steps cleared"), (get_from_memory::<u32>(sentinel), get_from_memory::<u32>(sentinel + 4)), (sentinel, sentinel));
                    check(&mut failures, &format!("{name} freelist"), get_from_memory::<u32>(freelist), nodes[2]);
                    check(&mut failures, &format!("{name} freelist chain"), (get_from_memory::<u32>(nodes[2]), get_from_memory::<u32>(nodes[1]), get_from_memory::<u32>(nodes[0])), (nodes[1], nodes[0], old_head));
                }
                save_to_memory(freelist, old_head);
                deepest = deepest.max(total);
                if total == 0 {
                    continue;
                }
                for succeed_at in [1, 2, total / 2, total - 1, total] {
                    if succeed_at < 1 {
                        continue;
                    }
                    GET_PATH_SUCCEED_AT.store(succeed_at, Ordering::SeqCst);
                    GET_PATH_CALLS.store(0, Ordering::SeqCst);
                    take_log();
                    let real_value = real_movement::get_path_away_from_target(twins.real.0);
                    let real_calls = take_log();
                    GET_PATH_CALLS.store(0, Ordering::SeqCst);
                    let port_value = twins.port.get_path_away_from_target();
                    let port_calls = take_log();
                    let label = format!("{name} succeeding at {succeed_at} of {total}");
                    check(&mut failures, &format!("{label} value"), real_value, port_value);
                    compare_log(&label, &real_calls, &port_calls, &mut failures);
                    compare_memory(&label, twins.real, twins.port, &mut failures);
                    real_compared += 1;
                }
            }
        }
        twins.destroy();
    }
    GET_PATH_SUCCEED_AT.store(-1, Ordering::SeqCst);
    tracing::info!("{test_name}: {real_compared} real comparisons, up to {deepest} candidates");
    if real_compared == 0 {
        failures.push(format!("no case had a candidate to compare ({} units)", units.len()));
    }
    stop_stubbing();
    finish_test(test_name, failures, failure_log)
}

static FAKE_PREDATOR_MODE: AtomicU32 = AtomicU32::new(0);
static FAKE_WORLD_HABITAT: AtomicU32 = AtomicU32::new(0);

/// A scripted `checkPredator`: accepts a candidate by a bit of its address (mode `0..=2`), none (`3`) or every non-null one.
extern "thiscall" fn fake_check_predator(_this: u32, candidate: u32, flag: u32) -> u32 {
    let id = if candidate == 0 { 0 } else { get_from_memory::<u32>(candidate + unit_offsets::ENTITY_ID) };
    log(format!("checkPredator {id:#x} {}", flag & 0xff));
    let mode = FAKE_PREDATOR_MODE.load(Ordering::SeqCst);
    u32::from(candidate != 0 && (mode == 4 || (mode < 3 && (candidate >> 4) % 3 == mode)))
}

/// `isCastClass` tag for `ZTStaff` (`DAT_00638710`).
const STAFF_TAG_RVA: u32 = 0x0023_8710;

extern "thiscall" fn fake_world_habitat(_this: u32) -> u32 {
    FAKE_WORLD_HABITAT.load(Ordering::SeqCst)
}

/// `ZTUNIT_FIND_PREDATOR_LIVE`: `findPredator` against the real body with twins standing on live animals' tiles. The
/// habitat slots are the real ones (so the tile scan walks the animal's own habitat), `checkPredator` is scripted, and a
/// second pass swaps in a world habitat and a temporary escaped-animal list to reach the list branch.
pub(crate) fn run_ztunit_find_predator_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_FIND_PREDATOR_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_target_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    let animals = live_entities(ANIMAL_TAG_RVA);
    if types.is_empty() || animals.is_empty() {
        return no_units(test_name, failure_log);
    }
    let anchors: Vec<u32> = animals.iter().copied().step_by((animals.len() / 12).max(1)).take(12).collect();
    let escaped_global = get_module_base("zoo.exe") as u32 + 0x0023_8fb0;
    let (mut tile_hits, mut list_hits, mut shortcut_hits) = (0u32, 0u32, 0u32);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        for unit in twins.both() {
            let vtable: u32 = get_from_memory(unit.0);
            save_to_memory(vtable + 0x248, get_from_memory::<u32>(twins.vtables.0 + 0x248));
            save_to_memory(vtable + 0x24c, get_from_memory::<u32>(twins.vtables.0 + 0x24c));
            save_to_memory(vtable + 0x304, fake_check_predator as *const () as u32);
        }
        for world_pass in [false, true] {
            let saved_list: u32 = get_from_memory(escaped_global);
            if world_pass {
                let habitat = leaked_words(vec![0; 0x80]);
                save_to_memory(habitat + 0x2c, 1u8);
                FAKE_WORLD_HABITAT.store(habitat, Ordering::SeqCst);
                let sentinel = leaked_words(vec![0; 3]);
                let nodes: Vec<u32> = animals.iter().map(|&animal| leaked_words(vec![0, 0, animal])).collect();
                let mut previous = sentinel;
                for &node in &nodes {
                    save_to_memory(previous, node);
                    save_to_memory(node + 4, previous);
                    previous = node;
                }
                save_to_memory(previous, sentinel);
                save_to_memory(sentinel + 4, previous);
                save_to_memory(escaped_global, sentinel);
                for unit in twins.both() {
                    let vtable: u32 = get_from_memory(unit.0);
                    save_to_memory(vtable + 0x24c, fake_world_habitat as *const () as u32);
                }
            }
            for &anchor in &anchors {
                let (x, y) = (get_from_memory::<i32>(anchor + 0x114) / 64, get_from_memory::<i32>(anchor + 0x118) / 64);
                for unit in twins.both() {
                    place_at_tile(unit, x as u32, y as u32);
                }
                for predator in [0, entity_id(anchor)] {
                    for mode in [0u32, 1, 2, 3, 4] {
                        for flag in [false, true] {
                            FAKE_PREDATOR_MODE.store(mode, Ordering::SeqCst);
                            for unit in twins.both() {
                                save_to_memory(unit.0 + unit_offsets::PREDATOR, predator);
                            }
                            take_log();
                            let real_value = real_satisfiers::find_predator(twins.real.0, flag);
                            let real_calls = take_log();
                            let port_value = twins.port.find_predator(flag);
                            let port_calls = take_log();
                            let name = format!("type {type_index} world {world_pass} anchor {anchor:#x} ({x},{y}) predator {predator:#x} mode {mode} flag {flag}");
                            check(&mut failures, &format!("{name} value"), real_value, port_value);
                            compare_log(&name, &real_calls, &port_calls, &mut failures);
                            compare_memory(&name, twins.real, twins.port, &mut failures);
                            if port_value != 0 {
                                if world_pass {
                                    list_hits += 1;
                                } else if port_value as u32 == predator {
                                    shortcut_hits += 1;
                                } else {
                                    tile_hits += 1;
                                }
                            }
                        }
                    }
                }
            }
            if world_pass {
                save_to_memory(escaped_global, saved_list);
                for unit in twins.both() {
                    let vtable: u32 = get_from_memory(unit.0);
                    save_to_memory(vtable + 0x24c, get_from_memory::<u32>(twins.vtables.0 + 0x24c));
                }
            }
        }
        twins.destroy();
    }
    tracing::info!("{test_name}: {tile_hits} tile-scan answers, {list_hits} list answers, {shortcut_hits} predator shortcuts");
    if tile_hits == 0 || list_hits == 0 || shortcut_hits == 0 {
        failures.push(format!("a branch was never exercised (tile {tile_hits}, list {list_hits}, shortcut {shortcut_hits})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_FIND_NEAREST_LIVE`: `findNearestSatisfier` against the real body with unscripted twins standing on the tiles
/// of live animals, over two building-list names (`building` and the nearby-building name), ranges, type-id filters and (with the building types temporarily set
/// to scale by happiness) the threshold-scaled distances. Both sides must leave the shared candidate vector, the chosen
/// building, the `rand` state and the unit alike.
pub(crate) fn run_ztunit_find_nearest_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::vanilla_string::VanillaString;

    let test_name = "ZTUNIT_FIND_NEAREST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(2);
    let animals = live_entities(ANIMAL_TAG_RVA);
    let buildings = live_entities(BUILDING_TAG_RVA);
    if types.is_empty() || animals.is_empty() || buildings.is_empty() {
        return no_units(test_name, failure_log);
    }
    let world = globals().ztworldmgr();
    let module = get_module_base("zoo.exe") as u32;
    let candidates = module + 0x0023_c538;
    let game_manager = globals().ztgamemgr() as *const _ as u32;
    let building_name = VanillaString::new("building");
    let nearby_static = module + 0x0024_6390;
    let anchors: Vec<u32> = buildings.iter().copied().step_by((buildings.len() / 12).max(1)).take(12).collect();
    // Building types and the fields the scaling variants overwrite, to put back afterwards.
    let building_types: Vec<u32> = {
        let mut unique: Vec<u32> = buildings.iter().map(|&b| get_from_memory::<u32>(b + unit_offsets::ENTITY_TYPE)).filter(|&t| t != 0).collect();
        unique.sort_unstable();
        unique.dedup();
        unique
    };
    let saved_types: Vec<(u32, u8, u32)> = building_types.iter().map(|&t| (t, get_from_memory::<u8>(t + 0x23d), get_from_memory::<u32>(t + 0x178))).collect();
    let saved_fills: Vec<(u32, u32)> = buildings.iter().map(|&b| (b, get_from_memory::<u32>(b + 0x1ac))).collect();
    let saved_thresholds: (u32, u32) = (get_from_memory(game_manager + 0x114), get_from_memory(game_manager + 0x118));
    let type_id = |entity_type: u32| crate::ztkeeper::call_slot_0(entity_type, 0x20) as i32;
    let first_type_id = type_id(building_types[0]);
    let (mut found_cases, mut scaled_cases, mut compared) = (0u32, 0u32, 0u32);
    let variants: [(bool, f32, f32, f32); 5] = [(false, 0.0, 0.0, 0.0), (true, 0.3, 0.7, 0.1), (true, 0.3, 0.7, 0.5), (true, 0.3, 0.7, 0.9), (true, f32::NAN, 0.5, 0.5)];
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, false);
        for &anchor in &anchors {
            let (x, y) = (get_from_memory::<i32>(anchor + 0x114) / 64, get_from_memory::<i32>(anchor + 0x118) / 64);
            for unit in twins.both() {
                place_at_tile(unit, x as u32, y as u32);
            }
            let tile = world.get_tile_ptr(x as u32, y as u32);
            for (variant, &(scales, low, high, fill)) in variants.iter().enumerate() {
                if scales {
                    for &t in &building_types {
                        save_to_memory(t + 0x23d, 1u8);
                        save_to_memory(t + 0x178, 1.0f32);
                    }
                    for &b in &buildings {
                        save_to_memory(b + 0x1ac, fill);
                    }
                    save_to_memory(game_manager + 0x114, low);
                    save_to_memory(game_manager + 0x118, high);
                }
                for (name_index, name) in [building_name.as_ptr() as u32, nearby_static].into_iter().enumerate() {
                    for range in [3i32, 10, 40, 300] {
                        for filter in [-1, first_type_id, first_type_id.wrapping_add(1000)] {
                            for seed in [0x11u32, 0x7777_1234] {
                                let label = format!("type {type_index} anchor {anchor:#x} ({x},{y}) variant {variant} name {name_index} range {range} filter {filter} seed {seed:#x}");
                                let (mut real_out, mut port_out) = (0xdeadu32, 0xdeadu32);
                                pin_rng(seed);
                                let real_found = real_satisfiers::find_nearest_satisfier(twins.real.0, tile, name, &mut real_out as *mut u32 as u32, range, filter);
                                let real_rng = rng_state();
                                let real_vector: Vec<u32> = {
                                    let (begin, end): (u32, u32) = (get_from_memory(candidates), get_from_memory(candidates + 4));
                                    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
                                };
                                pin_rng(seed);
                                let port_found = twins.port.find_nearest_satisfier(tile, name, &mut port_out as *mut u32 as u32, range, filter);
                                let port_rng = rng_state();
                                let port_vector: Vec<u32> = {
                                    let (begin, end): (u32, u32) = (get_from_memory(candidates), get_from_memory(candidates + 4));
                                    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
                                };
                                check(&mut failures, &format!("{label} found"), real_found, port_found);
                                check(&mut failures, &format!("{label} out"), real_out, port_out);
                                check(&mut failures, &format!("{label} rng"), real_rng, port_rng);
                                check(&mut failures, &format!("{label} candidates"), real_vector, port_vector);
                                compare_memory(&label, twins.real, twins.port, &mut failures);
                                compared += 1;
                                found_cases += u32::from(port_found);
                                scaled_cases += u32::from(port_found && scales);
                            }
                        }
                    }
                }
            }
        }
        twins.destroy();
    }
    for (t, scale_byte, scale) in saved_types {
        save_to_memory(t + 0x23d, scale_byte);
        save_to_memory(t + 0x178, scale);
    }
    for (b, fill) in saved_fills {
        save_to_memory(b + 0x1ac, fill);
    }
    save_to_memory(game_manager + 0x114, saved_thresholds.0);
    save_to_memory(game_manager + 0x118, saved_thresholds.1);
    tracing::info!("{test_name}: {compared} comparisons, {found_cases} found, {scaled_cases} found with scaling");
    if found_cases == 0 || scaled_cases == 0 {
        failures.push(format!("never found a satisfier (found {found_cases}, scaled {scaled_cases})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_FIND_ANY_LIVE`: `findAnySatisfier` against the real body with unscripted twins standing on the tiles
/// of live animals, over two building-list names, ranges, type-id filters and (with the building types temporarily set
/// to scale by happiness) the threshold-scaled distances. Both sides must leave the shared candidate vector, the chosen
/// building, the `rand` state and the unit alike.
pub(crate) fn run_ztunit_find_any_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::vanilla_string::VanillaString;

    let test_name = "ZTUNIT_FIND_ANY_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(2);
    let animals = live_entities(ANIMAL_TAG_RVA);
    let buildings = live_entities(BUILDING_TAG_RVA);
    if types.is_empty() || animals.is_empty() || buildings.is_empty() {
        return no_units(test_name, failure_log);
    }
    let world = globals().ztworldmgr();
    let module = get_module_base("zoo.exe") as u32;
    let candidates = module + 0x0023_c538;
    let game_manager = globals().ztgamemgr() as *const _ as u32;
    let building_name = VanillaString::new("building");
    let nearby_static = module + 0x0024_6390;
    let anchors: Vec<u32> = buildings.iter().copied().step_by((buildings.len() / 12).max(1)).take(12).collect();
    // Building types and the fields the scaling variants overwrite, to put back afterwards.
    let building_types: Vec<u32> = {
        let mut unique: Vec<u32> = buildings.iter().map(|&b| get_from_memory::<u32>(b + unit_offsets::ENTITY_TYPE)).filter(|&t| t != 0).collect();
        unique.sort_unstable();
        unique.dedup();
        unique
    };
    let saved_types: Vec<(u32, u8, u32)> = building_types.iter().map(|&t| (t, get_from_memory::<u8>(t + 0x23d), get_from_memory::<u32>(t + 0x178))).collect();
    let saved_fills: Vec<(u32, u32)> = buildings.iter().map(|&b| (b, get_from_memory::<u32>(b + 0x1ac))).collect();
    let saved_thresholds: (u32, u32) = (get_from_memory(game_manager + 0x114), get_from_memory(game_manager + 0x118));
    let type_id = |entity_type: u32| crate::ztkeeper::call_slot_0(entity_type, 0x20) as i32;
    let first_type_id = type_id(building_types[0]);
    let (mut found_cases, mut scaled_cases, mut compared) = (0u32, 0u32, 0u32);
    let variants: [(bool, f32, f32, f32); 1] = [(false, 0.0, 0.0, 0.0)];
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, false);
        for &anchor in &anchors {
            let (x, y) = (get_from_memory::<i32>(anchor + 0x114) / 64, get_from_memory::<i32>(anchor + 0x118) / 64);
            for unit in twins.both() {
                place_at_tile(unit, x as u32, y as u32);
            }
            let tile = world.get_tile_ptr(x as u32, y as u32);
            for (variant, &(scales, low, high, fill)) in variants.iter().enumerate() {
                if scales {
                    for &t in &building_types {
                        save_to_memory(t + 0x23d, 1u8);
                        save_to_memory(t + 0x178, 1.0f32);
                    }
                    for &b in &buildings {
                        save_to_memory(b + 0x1ac, fill);
                    }
                    save_to_memory(game_manager + 0x114, low);
                    save_to_memory(game_manager + 0x118, high);
                }
                for (name_index, name) in [building_name.as_ptr() as u32, nearby_static].into_iter().enumerate() {
                    for range in [3i32, 10, 40, 300] {
                        for filter in [-1, first_type_id, first_type_id.wrapping_add(1000)] {
                            for seed in [0x11u32, 0x7777_1234] {
                                let label = format!("type {type_index} anchor {anchor:#x} ({x},{y}) variant {variant} name {name_index} range {range} filter {filter} seed {seed:#x}");
                                let (mut real_out, mut port_out) = (0xdeadu32, 0xdeadu32);
                                pin_rng(seed);
                                let real_found = real_satisfiers::find_any_satisfier(twins.real.0, tile, name, &mut real_out as *mut u32 as u32, range, filter);
                                let real_rng = rng_state();
                                let real_vector: Vec<u32> = {
                                    let (begin, end): (u32, u32) = (get_from_memory(candidates), get_from_memory(candidates + 4));
                                    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
                                };
                                pin_rng(seed);
                                let port_found = twins.port.find_any_satisfier(tile, name, &mut port_out as *mut u32 as u32, range, filter);
                                let port_rng = rng_state();
                                let port_vector: Vec<u32> = {
                                    let (begin, end): (u32, u32) = (get_from_memory(candidates), get_from_memory(candidates + 4));
                                    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
                                };
                                check(&mut failures, &format!("{label} found"), real_found, port_found);
                                check(&mut failures, &format!("{label} out"), real_out, port_out);
                                check(&mut failures, &format!("{label} rng"), real_rng, port_rng);
                                check(&mut failures, &format!("{label} candidates"), real_vector, port_vector);
                                compare_memory(&label, twins.real, twins.port, &mut failures);
                                compared += 1;
                                found_cases += u32::from(port_found);
                                scaled_cases += u32::from(port_found && scales);
                            }
                        }
                    }
                }
            }
        }
        twins.destroy();
    }
    for (t, scale_byte, scale) in saved_types {
        save_to_memory(t + 0x23d, scale_byte);
        save_to_memory(t + 0x178, scale);
    }
    for (b, fill) in saved_fills {
        save_to_memory(b + 0x1ac, fill);
    }
    save_to_memory(game_manager + 0x114, saved_thresholds.0);
    save_to_memory(game_manager + 0x118, saved_thresholds.1);
    tracing::info!("{test_name}: {compared} comparisons, {found_cases} found, {scaled_cases} found with scaling");
    if found_cases == 0 {
        failures.push(format!("never found a satisfier (found {found_cases})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// The `setBS` answers `fWasCaught`'s queue calls get, in order, and how many calls were made.
static CAUGHT_ANSWERS: Mutex<Vec<bool>> = Mutex::new(Vec::new());

/// A scripted `BFEntity::setBS`: records the name and flags and answers from [`CAUGHT_ANSWERS`] (false when exhausted).
fn scripted_caught_set_bs(_this: u32, begin: u32, end: u32, call_flag: i8, force: i8, clear_mode: i32) -> Option<bool> {
    let name: Vec<u8> = if begin == 0 || end <= begin { Vec::new() } else { (begin..end).map(get_from_memory::<u8>).collect() };
    log(format!("setBS {} {call_flag} {force} {clear_mode}", String::from_utf8_lossy(&name)));
    let mut answers = CAUGHT_ANSWERS.lock().unwrap();
    Some(if answers.is_empty() { false } else { answers.remove(0) })
}

/// `ZTUNIT_F_WAS_CAUGHT_LIVE`: `fWasCaught` against the real body with scripted twins whose recorded predator is a live
/// animal (of every live type), a non-animal unit or nothing, over the goals it leaves alone and the `setBS` outcomes
/// (queued at once, queued by the lion fallback, never queued).
pub(crate) fn run_ztunit_f_was_caught_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_F_WAS_CAUGHT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(2);
    let animals = live_entities(ANIMAL_TAG_RVA);
    if types.is_empty() || animals.is_empty() {
        return no_units(test_name, failure_log);
    }
    // One animal per distinct type, so each name pair is seen; plus a non-animal unit and no predator at all.
    let mut seen_types: Vec<u32> = Vec::new();
    let mut predators: Vec<u32> = Vec::new();
    for &animal in &animals {
        let animal_type: u32 = get_from_memory(animal + unit_offsets::ENTITY_TYPE);
        if !seen_types.contains(&animal_type) {
            seen_types.push(animal_type);
            predators.push(animal);
        }
        if predators.len() >= 10 {
            break;
        }
    }
    let non_animal = live_entities(UNIT_TAG_RVA).into_iter().find(|&unit| !unsafe { entity_type_matches(unit, ANIMAL_TAG_RVA) });
    let mut cases: Vec<u32> = predators.iter().map(|&animal| entity_id(animal)).collect();
    cases.extend(non_animal.map(entity_id));
    cases.push(0);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_caught_set_bs);
    let answer_sets: [&[bool]; 3] = [&[true], &[false, true], &[false, false]];
    let (mut queued_first, mut queued_fallback, mut retargeted, mut fallback_female, mut fallback_male) = (0u32, 0u32, 0u32, 0u32, 0u32);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = scripted_twins(unit_type);
        for &predator_id in &cases {
            for goal in [0x40u32, CAUGHT_GOAL_FOR_TEST, 0x68, 0x6a] {
                for (answer_index, answers) in answer_sets.iter().enumerate() {
                    for unit in twins.both() {
                        save_to_memory(unit.0 + unit_offsets::PREDATOR, predator_id);
                        save_to_memory(unit.0 + unit_offsets::GOAL_ID, goal);
                        save_to_memory(unit.0 + unit_offsets::BUSY, 0u8);
                        save_to_memory(unit.0 + unit_offsets::COG_GOAL_FLAG, 7u8);
                    }
                    let label = format!("type {type_index} predator {predator_id:#x} goal {goal:#x} answers {answer_index}");
                    *CAUGHT_ANSWERS.lock().unwrap() = answers.to_vec();
                    take_log();
                    let real_value = real_satisfiers::f_was_caught(twins.real.0);
                    let real_calls = take_log();
                    *CAUGHT_ANSWERS.lock().unwrap() = answers.to_vec();
                    let port_value = twins.port.f_was_caught();
                    let port_calls = take_log();
                    check(&mut failures, &format!("{label} value"), real_value, port_value);
                    compare_log(&label, &real_calls, &port_calls, &mut failures);
                    compare_memory(&label, twins.real, twins.port, &mut failures);
                    if port_value {
                        if port_calls.iter().any(|call| call.starts_with("setBS bCaughtBylion")) {
                            queued_fallback += 1;
                            fallback_female += u32::from(port_calls.iter().any(|call| call.starts_with("setBS bCaughtBylionf")));
                            fallback_male += u32::from(port_calls.iter().any(|call| call.starts_with("setBS bCaughtBylionm")));
                        } else {
                            queued_first += 1;
                        }
                    }
                    retargeted += u32::from(port_calls.iter().any(|call| call == "clearGoal"));
                }
            }
        }
        twins.destroy();
    }
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = None;
    CAUGHT_ANSWERS.lock().unwrap().clear();
    tracing::info!("{test_name}: {queued_first} queued first, {queued_fallback} by fallback (f {fallback_female}, m {fallback_male}), {retargeted} retargeted");
    if queued_first == 0 || queued_fallback == 0 || retargeted == 0 {
        failures.push(format!("a branch was never exercised (first {queued_first}, fallback {queued_fallback}, retargeted {retargeted})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// The goal id that marks a unit as already caught.
const CAUGHT_GOAL_FOR_TEST: u32 = 0x75;
