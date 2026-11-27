//! Live checks for the building-use group of `ZTUnit` (`ztunit/buildings.rs`): twin units, one run through the real body
//! and one through the port, with the unit's virtual slots and the `ZTBuilding`/`BFUnit::goToDest` collaborators scripted so
//! the calls made can be compared as well as the memory and the answers.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU8, Ordering},
    Mutex, Once,
};

use openzt_detour::generated::{
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    ztbuilding::{ADD_USER, CAN_ADD_USER, FIND_NEAREST_OPEN_SLOT, REMOVE_MEMBER, REMOVE_USER, REMOVE_USER_IF_PRESENT, YANK_USER_IF_PRESENT},
    ztworldmgr::GET_NEED_INDEX,
};

use super::{
    check, compare_log, compare_memory, leaked_words, log, pin_rng, place_at_tile, rng_state, sample_tiles,
    targets::{live_entities, no_units, scripted_twins},
    take_log, Twins,
};
use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::{harness::finish_test, tests::ztanimal::live_animal_types},
    util::{get_from_memory, save_to_memory},
    ztunit::{live_support::real_buildings, unit_offsets, ZTUnit},
};

const BUILDING_TAG_RVA: u32 = 0x0023_8680;
const BUILDING_NEED_NAME_RVA: u32 = 0x0023_afd8;
const RETURN_NEED_NAME_RVA: u32 = 0x0023_af90;
const MAX_EDGE_COST_RVA: u32 = 0x0023_5494;
/// Unit fields these tests set up directly.
const OFFSET: u32 = unit_offsets::BUILDING_OFFSET;
const BUILDING_SLOT: u32 = unit_offsets::BUILDING_SLOT;

// ---- scripted answers ------------------------------------------------------------------------------------------

static VALIDATE_ANSWER: AtomicU32 = AtomicU32::new(0);
static VALIDATE_FLAG: AtomicU8 = AtomicU8::new(0);
static EDGE_COST: AtomicI32 = AtomicI32::new(0);
static IS_RETURNING: AtomicU32 = AtomicU32::new(0);
static NAME_ID: AtomicU32 = AtomicU32::new(0);
static RETURN_STRING: AtomicU32 = AtomicU32::new(0);
static ADD_USER_ANSWER: AtomicU32 = AtomicU32::new(0);
static CAN_ADD_ANSWER: AtomicU32 = AtomicU32::new(0);
static REMOVE_IF_PRESENT_ANSWER: AtomicU32 = AtomicU32::new(0);
static OPEN_SLOT_ANSWER: AtomicU32 = AtomicU32::new(0);
static OPEN_SLOT_BYTE: AtomicU8 = AtomicU8::new(0);
static GO_TO_DEST_ANSWER: AtomicU32 = AtomicU32::new(0);
static NEED_OR_FUN_ANSWER: AtomicU32 = AtomicU32::new(0);
static FIND_ANY_BUILDING: AtomicU32 = AtomicU32::new(0);
static SET_BS_ANSWER: AtomicU32 = AtomicU32::new(0);
static STUBBING: AtomicBool = AtomicBool::new(false);
static TEST_THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);

fn stubbing() -> bool {
    STUBBING.load(Ordering::SeqCst) && *TEST_THREAD.lock().unwrap() == Some(std::thread::current().id())
}

fn start_stubbing() {
    *TEST_THREAD.lock().unwrap() = Some(std::thread::current().id());
    STUBBING.store(true, Ordering::SeqCst);
}

fn stop_stubbing() {
    STUBBING.store(false, Ordering::SeqCst);
    *TEST_THREAD.lock().unwrap() = None;
}

/// `BFUnit::goToDest` answers, reached through the detours `ztguest_checks.rs` owns for those addresses.
pub(in crate::reimplementation_tests::tests) fn stub_go_to_dest_none(_this: u32, flag: bool, arg: u32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    log(format!("goToDest none {flag} {arg}"));
    Some(GO_TO_DEST_ANSWER.load(Ordering::SeqCst) != 0)
}

pub(in crate::reimplementation_tests::tests) fn stub_go_to_dest_string(_this: u32, name: u32, arg: u32, flag: i8) -> Option<u32> {
    if !stubbing() {
        return None;
    }
    let (begin, end): (u32, u32) = (get_from_memory(name), get_from_memory(name + 4));
    let text: String = (begin..end).map(|address| get_from_memory::<u8>(address) as char).collect();
    log(format!("goToDest {text:?} {arg} {flag}"));
    Some(GO_TO_DEST_ANSWER.load(Ordering::SeqCst))
}

#[openzt_detour_macro::detour_mod]
mod building_stubs {
    use super::*;

    #[detour(ADD_USER)]
    unsafe extern "thiscall" fn add_user(this: *const u32, unit: *const i32, slot: *const i32) -> u32 {
        if stubbing() {
            log(format!("addUser {}", slot as u32 & 0xff));
            ADD_USER_ANSWER.load(Ordering::SeqCst)
        } else {
            unsafe { ADD_USER_DETOUR.call(this, unit, slot) }
        }
    }

    #[detour(CAN_ADD_USER)]
    unsafe extern "thiscall" fn can_add_user(this: *const u32, unit: *const i32, slot: *const i32) -> u32 {
        if stubbing() {
            log(format!("canAddUser {}", slot as u32 & 0xff));
            CAN_ADD_ANSWER.load(Ordering::SeqCst)
        } else {
            unsafe { CAN_ADD_USER_DETOUR.call(this, unit, slot) }
        }
    }

    #[detour(REMOVE_USER)]
    unsafe extern "thiscall" fn remove_user(this: *const u32, unit: *const i32, flag: i8) -> u32 {
        if stubbing() {
            log(format!("removeUser {flag}"));
            0
        } else {
            unsafe { REMOVE_USER_DETOUR.call(this, unit, flag) }
        }
    }

    #[detour(REMOVE_USER_IF_PRESENT)]
    unsafe extern "thiscall" fn remove_user_if_present(this: *const u32, unit: *const i32) -> u32 {
        if stubbing() {
            log("removeUserIfPresent".to_string());
            REMOVE_IF_PRESENT_ANSWER.load(Ordering::SeqCst)
        } else {
            unsafe { REMOVE_USER_IF_PRESENT_DETOUR.call(this, unit) }
        }
    }

    #[detour(YANK_USER_IF_PRESENT)]
    unsafe extern "thiscall" fn yank_user_if_present(this: *const u32, unit: i32) -> u32 {
        if stubbing() {
            log("yankUserIfPresent".to_string());
            0
        } else {
            unsafe { YANK_USER_IF_PRESENT_DETOUR.call(this, unit) }
        }
    }

    #[detour(REMOVE_MEMBER)]
    unsafe extern "thiscall" fn remove_member(this: *const u32, unit: i32) {
        if stubbing() {
            log("removeMember".to_string());
        } else {
            unsafe { REMOVE_MEMBER_DETOUR.call(this, unit) }
        }
    }

    #[detour(FIND_NEAREST_OPEN_SLOT)]
    unsafe extern "thiscall" fn find_nearest_open_slot(this: *const u32, unit: *const i32, out: *const u32, position: u32) -> u32 {
        if stubbing() {
            let words: Vec<i32> = (0..3).map(|i| get_from_memory::<i32>(position + i * 4)).collect();
            log(format!("findNearestOpenSlot {words:?}"));
            save_to_memory(out as u32, OPEN_SLOT_BYTE.load(Ordering::SeqCst));
            OPEN_SLOT_ANSWER.load(Ordering::SeqCst)
        } else {
            unsafe { FIND_NEAREST_OPEN_SLOT_DETOUR.call(this, unit, out, position) }
        }
    }
}

fn install_stubs() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { building_stubs::init_detours() } {
            result = Err(format!("could not install the building stubs: {e:?}"));
        }
    });
    result
}

// ---- scripted virtual slots ------------------------------------------------------------------------------------

extern "thiscall" fn fake_validate_building(_this: u32, out: u32, flag: u32) -> u32 {
    log(format!("validateBuilding {}", flag & 0xff));
    if out != 0 {
        save_to_memory(out, VALIDATE_FLAG.load(Ordering::SeqCst));
    }
    VALIDATE_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_do_building(_this: u32) {
    log("doBuilding".to_string());
}
extern "thiscall" fn fake_edge_cost(_this: u32, tile: u32, direction: u32) -> u32 {
    log(format!("getEdgeCost {} {direction}", tile != 0));
    EDGE_COST.load(Ordering::SeqCst) as u32
}
extern "thiscall" fn fake_abort_goal(_this: u32, flag: u32) {
    log(format!("abortGoal {}", flag & 0xff));
}
extern "thiscall" fn fake_set_grid_dest(_this: u32, position: u32) {
    let words: Vec<i32> = (0..3).map(|i| get_from_memory::<i32>(position + i * 4)).collect();
    log(format!("setGridDest {words:?}"));
}
extern "thiscall" fn fake_is_returning(_this: u32) -> u32 {
    log("isReturning".to_string());
    IS_RETURNING.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_name_id(_this: u32) -> u32 {
    log("getReturnBuildingNameID".to_string());
    NAME_ID.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_return_string(_this: u32) -> u32 {
    log("getReturnBuildingString".to_string());
    RETURN_STRING.load(Ordering::SeqCst)
}

/// Layers the building slots over the unit's current (already scripted) vtable.
fn script_building_slots(unit: ZTUnit) {
    let current: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    let scripted: [(u32, u32); 8] = [
        (0x160, fake_edge_cost as *const () as u32),
        (0x178, fake_abort_goal as *const () as u32),
        (0x1a0, fake_set_grid_dest as *const () as u32),
        (0x264, fake_is_returning as *const () as u32),
        (0x298, fake_name_id as *const () as u32),
        (0x2a0, fake_return_string as *const () as u32),
        (0x2e8, fake_validate_building as *const () as u32),
        (0x2ec, fake_do_building as *const () as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    save_to_memory(unit.0, leaked_words(vtable));
}

fn building_twins(unit_type: u32) -> Twins {
    let twins = scripted_twins(unit_type);
    for unit in twins.both() {
        script_building_slots(unit);
    }
    twins
}

// ---- scripted collaborators of the port and the real body ------------------------------------------------------

fn scripted_need_or_fun(_unit: u32, quadrants: u32, position: *const i32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    let words: Vec<i32> = (0..3).map(|i| get_from_memory::<i32>(position as u32 + i * 4)).collect();
    log(format!("isNeedOrFunSatisfier {} {words:?}", quadrants != 0));
    Some(NEED_OR_FUN_ANSWER.load(Ordering::SeqCst) != 0)
}

fn scripted_find_any(_unit: u32, tile: u32, name: u32, out: u32, range: i32, filter: i32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    let building = FIND_ANY_BUILDING.load(Ordering::SeqCst);
    log(format!("findAnySatisfier {} {} {range:#x} {filter}", tile != 0, name == get_module_base("zoo.exe") as u32 + RETURN_NEED_NAME_RVA));
    if building != 0 {
        save_to_memory(out, building);
    }
    Some(building != 0)
}

fn scripted_set_bs(_this: u32, begin: u32, end: u32, call_flag: i8, force: i8, clear_mode: i32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    let name: Vec<u8> = if begin == 0 || end <= begin { Vec::new() } else { (begin..end).map(get_from_memory::<u8>).collect() };
    log(format!("setBS {} {call_flag} {force} {clear_mode}", String::from_utf8_lossy(&name)));
    Some(SET_BS_ANSWER.load(Ordering::SeqCst) != 0)
}

type SavedHooks = (
    Option<fn(u32, u32, *const i32) -> Option<bool>>,
    Option<fn(u32, u32, u32, u32, i32, i32) -> Option<bool>>,
    Option<crate::bfentity::set_bs_override::Hook>,
);

fn install_hooks() -> SavedHooks {
    use crate::ztunit::live_support::intercept;
    let saved = (*intercept::IS_NEED_OR_FUN_SATISFIER.lock().unwrap(), *intercept::FIND_ANY_SATISFIER.lock().unwrap(), *crate::bfentity::set_bs_override::HOOK.lock().unwrap());
    *intercept::IS_NEED_OR_FUN_SATISFIER.lock().unwrap() = Some(scripted_need_or_fun);
    *intercept::FIND_ANY_SATISFIER.lock().unwrap() = Some(scripted_find_any);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_set_bs);
    saved
}

fn restore_hooks(saved: SavedHooks) {
    use crate::ztunit::live_support::intercept;
    *intercept::IS_NEED_OR_FUN_SATISFIER.lock().unwrap() = saved.0;
    *intercept::FIND_ANY_SATISFIER.lock().unwrap() = saved.1;
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = saved.2;
}

// ---- fixtures --------------------------------------------------------------------------------------------------

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn building_tile(building: u32) -> u32 {
    unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) as u32 }
}

fn need_index(rva: u32) -> u8 {
    unsafe { GET_NEED_INDEX.hooked()((base() + rva) as *const i8) as u8 }
}

/// One live building per distinct building type (up to `limit`), each with the tile it stands on.
fn distinct_buildings(limit: usize) -> Vec<(u32, u32)> {
    let mut seen_types: Vec<u32> = Vec::new();
    let mut found = Vec::new();
    for building in live_entities(BUILDING_TAG_RVA) {
        let building_type: u32 = get_from_memory(building + unit_offsets::ENTITY_TYPE);
        let tile = building_tile(building);
        if tile != 0 && !seen_types.contains(&building_type) {
            seen_types.push(building_type);
            found.push((building, tile));
        }
        if found.len() >= limit {
            break;
        }
    }
    found
}

/// A leaked `std::list` of path steps: the sentinel address, with the nodes `{next, prev, x, y, z}` in `steps` order.
fn leaked_path(steps: &[[i32; 3]]) -> u32 {
    let sentinel = leaked_words(vec![0; 5]);
    let nodes: Vec<u32> = steps.iter().map(|step| leaked_words(vec![0, 0, step[0] as u32, step[1] as u32, step[2] as u32])).collect();
    let mut chain = vec![sentinel];
    chain.extend(&nodes);
    chain.push(sentinel);
    for window in 1..chain.len() - 1 {
        save_to_memory(chain[window], chain[window + 1]);
        save_to_memory(chain[window] + 4, chain[window - 1]);
    }
    let (first, last) = (nodes.first().copied().unwrap_or(sentinel), nodes.last().copied().unwrap_or(sentinel));
    save_to_memory(sentinel, first);
    save_to_memory(sentinel + 4, last);
    sentinel
}

/// The steps of a path list, as the log records them after a call.
fn path_steps(sentinel: u32) -> Vec<[i32; 3]> {
    let mut steps = Vec::new();
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel && steps.len() < 16 {
        steps.push([get_from_memory(node + 8), get_from_memory(node + 12), get_from_memory(node + 16)]);
        node = get_from_memory(node);
    }
    steps
}

fn set_offset(unit: ZTUnit, tile: u32, offset: [i32; 3]) {
    save_to_memory(unit.0 + unit_offsets::BUILDING_TILE, tile);
    for (index, value) in offset.into_iter().enumerate() {
        save_to_memory(unit.0 + OFFSET + index as u32 * 4, value);
    }
}

/// The four quadrant offsets of a tile.
const QUADRANTS: [[i32; 3]; 4] = [[0, 0, 5], [0, 32, 5], [32, 0, 5], [32, 32, 5]];

// ---- tests -----------------------------------------------------------------------------------------------------

/// `ZTUNIT_BUILDINGS_LIVE`: the building-use methods against the real bodies with scripted twins.
pub(crate) fn run_ztunit_buildings_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_BUILDINGS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    let buildings = distinct_buildings(8);
    if types.is_empty() || buildings.is_empty() {
        return no_units(test_name, failure_log);
    }
    let saved = install_hooks();
    let saved_rng = rng_state();
    start_stubbing();
    let building_need = need_index(BUILDING_NEED_NAME_RVA);
    let return_need = need_index(RETURN_NEED_NAME_RVA);
    let need_indices = [0u8, 1, building_need, return_need];
    let world = globals().ztworldmgr();
    let some_tile = world.get_tile_ptr(world.map_x_size / 2, world.map_y_size / 2);
    let (mut validated, mut validated_checked, mut entered, mut exited, mut removed, mut returned, mut named, mut named_show) = (0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32);

    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = building_twins(unit_type);
        let label = format!("type {type_index}");

        // hasBuildingUseGoal, setBuildingTile and the hooked addresses.
        for goal in [0u32, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x83] {
            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::GOAL_ID, goal);
            }
            check(&mut failures, &format!("{label} hasBuildingUseGoal({goal:#x})"), real_buildings::has_building_use_goal(twins.real.0), twins.port.has_building_use_goal());
            let hooked = unsafe { openzt_detour::generated::ztunit::HAS_BUILDING_USE_GOAL.hooked()(twins.port.0 as *const u32) };
            check(&mut failures, &format!("{label} hooked hasBuildingUseGoal({goal:#x})"), hooked, twins.port.has_building_use_goal());
        }
        real_buildings::set_building_tile(twins.real.0, 0x1234_5678);
        twins.port.set_building_tile(0x1234_5678);
        compare_memory(&format!("{label} setBuildingTile"), twins.real, twins.port, &mut failures);

        // setBuildingGoal.
        for (slot, offset) in [(0u8, [1, 2, 3]), (7, [-4, 70, 9]), (0xff, [0, 0, 0])] {
            take_log();
            real_buildings::set_building_goal(twins.real.0, some_tile, slot, offset);
            let real_calls = take_log();
            twins.port.set_building_goal(some_tile, slot, offset);
            let port_calls = take_log();
            let case = format!("{label} setBuildingGoal({slot}, {offset:?})");
            compare_log(&case, &real_calls, &port_calls, &mut failures);
            compare_memory(&case, twins.real, twins.port, &mut failures);
        }

        // validateBuilding over the quadrants of the live buildings' tiles, a tile with no building and no tile at all.
        let mut tiles: Vec<u32> = buildings.iter().map(|&(_, tile)| tile).collect();
        tiles.push(some_tile);
        tiles.push(0);
        for &tile in &tiles {
            for offset in QUADRANTS.into_iter().chain([[-1, 0, 0], [0, 64, 0], [40, -3, 0]]) {
                for slot in [0u8, 3, 0xff] {
                    for &need in &need_indices {
                        for check_flag in [false, true] {
                            for can_add in [0u32, 1] {
                                for unit in twins.both() {
                                    set_offset(unit, tile, offset);
                                    save_to_memory(unit.0 + BUILDING_SLOT, slot);
                                    save_to_memory(unit.0 + unit_offsets::NEED_INDEX, need);
                                }
                                CAN_ADD_ANSWER.store(can_add, Ordering::SeqCst);
                                let (mut real_out, mut port_out) = (0xaau8, 0xaau8);
                                take_log();
                                let real_value = real_buildings::validate_building(twins.real.0, &mut real_out as *mut u8 as *mut bool, check_flag);
                                let real_calls = take_log();
                                let port_value = twins.port.validate_building(&mut port_out as *mut u8 as *mut bool, check_flag);
                                let port_calls = take_log();
                                let case = format!("{label} validateBuilding tile {} {offset:?} slot {slot} need {need} check {check_flag} canAdd {can_add}", tile != 0);
                                check(&mut failures, &format!("{case} value"), real_value, port_value);
                                check(&mut failures, &format!("{case} out"), real_out, port_out);
                                compare_log(&case, &real_calls, &port_calls, &mut failures);
                                validated += u32::from(port_value != 0);
                                validated_checked += u32::from(port_value != 0 && check_flag && port_out != 0xaa);
                            }
                        }
                    }
                }
            }
        }
        let hooked_building = unsafe {
            let mut out = false;
            openzt_detour::generated::ztunit::VALIDATE_BUILDING.hooked()(twins.port.0 as *const u32, &mut out as *mut bool, false) as u32
        };
        check(&mut failures, &format!("{label} hooked validateBuilding"), hooked_building, twins.port.validate_building(&mut false, false));

        // fEnterBuilding / fExitBuilding / removeFromBuildingIfPresent over the scripted slot answers.
        let building = buildings[0].0;
        for validate in [0u32, building] {
            for flag in [0u8, 1] {
                for add in [0u32, 1] {
                    for need in [0u8, building_need] {
                        for goal in [0x40u32, 0x68, 0x6a] {
                            for unit in twins.both() {
                                save_to_memory(unit.0 + unit_offsets::NEED_INDEX, need);
                                save_to_memory(unit.0 + BUILDING_SLOT, 2u8);
                                save_to_memory(unit.0 + unit_offsets::GOAL_ID, goal);
                            }
                            VALIDATE_ANSWER.store(validate, Ordering::SeqCst);
                            VALIDATE_FLAG.store(flag, Ordering::SeqCst);
                            ADD_USER_ANSWER.store(add, Ordering::SeqCst);
                            REMOVE_IF_PRESENT_ANSWER.store(add, Ordering::SeqCst);
                            let case = format!("{label} building {} flag {flag} add {add} need {need} goal {goal:#x}", validate != 0);
                            take_log();
                            let real_enter = real_buildings::f_enter_building(twins.real.0);
                            let real_calls = take_log();
                            let port_enter = twins.port.f_enter_building();
                            let port_calls = take_log();
                            check(&mut failures, &format!("{case} fEnterBuilding"), real_enter, port_enter);
                            compare_log(&format!("{case} fEnterBuilding"), &real_calls, &port_calls, &mut failures);
                            entered += u32::from(port_enter);
                            take_log();
                            let real_exit = real_buildings::f_exit_building(twins.real.0);
                            let real_calls = take_log();
                            let port_exit = twins.port.f_exit_building();
                            let port_calls = take_log();
                            check(&mut failures, &format!("{case} fExitBuilding"), real_exit, port_exit);
                            compare_log(&format!("{case} fExitBuilding"), &real_calls, &port_calls, &mut failures);
                            exited += u32::from(port_exit);
                            take_log();
                            real_buildings::remove_from_building_if_present(twins.real.0);
                            let real_calls = take_log();
                            twins.port.remove_from_building_if_present();
                            let port_calls = take_log();
                            compare_log(&format!("{case} removeFromBuildingIfPresent"), &real_calls, &port_calls, &mut failures);
                            removed += u32::from(port_calls.iter().any(|call| call.starts_with("removeUserIfPresent")));
                            compare_memory(&case, twins.real, twins.port, &mut failures);
                        }
                    }
                }
            }
        }

        // fReturnToBuilding: found (every live building) or not, over the name id the slot answers.
        for &(found, _) in buildings.iter().map(|b| b).chain([&(0u32, 0u32)]) {
            for name_id in [0u32, 7] {
                for unit in twins.both() {
                    place_at_tile(unit, world.map_x_size / 3, world.map_y_size / 3);
                    save_to_memory(unit.0 + unit_offsets::NEED_INDEX, 0xeeu8);
                }
                FIND_ANY_BUILDING.store(found, Ordering::SeqCst);
                NAME_ID.store(name_id, Ordering::SeqCst);
                let case = format!("{label} fReturnToBuilding building {} name id {name_id}", found != 0);
                take_log();
                let real_value = real_buildings::f_return_to_building(twins.real.0);
                let real_calls = take_log();
                let port_value = twins.port.f_return_to_building();
                let port_calls = take_log();
                check(&mut failures, &format!("{case} value"), real_value, port_value);
                compare_log(&case, &real_calls, &port_calls, &mut failures);
                compare_memory(&case, twins.real, twins.port, &mut failures);
                returned += u32::from(port_value);
            }
        }
        FIND_ANY_BUILDING.store(0, Ordering::SeqCst);

        // setExitReturnBuildingBS and setBuildingUseBS.
        for answer in [0u32, 1] {
            SET_BS_ANSWER.store(answer, Ordering::SeqCst);
            for text in [&b"bExitBuilding"[..], b"", b"bReturnToBuilding"] {
                let name = leaked_text(text);
                take_log();
                let real_value = real_buildings::set_exit_return_building_bs(twins.real.0, name);
                let real_calls = take_log();
                let port_value = twins.port.set_exit_return_building_bs(name as *const u32);
                let port_calls = take_log();
                let case = format!("{label} setExitReturnBuildingBS {:?} answer {answer}", String::from_utf8_lossy(text));
                check(&mut failures, &format!("{case} value"), real_value, port_value);
                compare_log(&case, &real_calls, &port_calls, &mut failures);
                named += 1;
            }
            for &(building, _) in &buildings {
                for need in need_indices {
                    for slot in [0u8, 5, 200] {
                        for (trick, info) in [(0u16, 0u16), (0x2711, 3), (1, 1)] {
                            for unit in twins.both() {
                                save_to_memory(unit.0 + unit_offsets::NEED_INDEX, need);
                                save_to_memory(unit.0 + BUILDING_SLOT, slot);
                                save_to_memory(unit.0 + unit_offsets::TRICK_TYPE, trick);
                                save_to_memory(unit.0 + unit_offsets::SHOW_INFO_ID, info);
                            }
                            take_log();
                            let real_value = real_buildings::set_building_use_bs(twins.real.0, building);
                            let real_calls = take_log();
                            let port_value = twins.port.set_building_use_bs(building);
                            let port_calls = take_log();
                            let case = format!("{label} setBuildingUseBS building {building:#x} need {need} slot {slot} trick {trick:#x}/{info} answer {answer}");
                            check(&mut failures, &format!("{case} value"), real_value, port_value);
                            compare_log(&case, &real_calls, &port_calls, &mut failures);
                            named += 1;
                            named_show += u32::from(need == building_need && trick != 0);
                        }
                    }
                }
            }
        }

        // getHabitatFromVA over a spread of tiles, with the game's rand pinned.
        for (index, &(x, y)) in sample_tiles(40).iter().enumerate() {
            for unit in twins.both() {
                place_at_tile(unit.clone(), x, y);
            }
            pin_rng(0x1000 + index as u32 * 77);
            let real_value = real_buildings::get_habitat_from_va(twins.real.0);
            let real_state = rng_state();
            pin_rng(0x1000 + index as u32 * 77);
            let port_value = twins.port.get_habitat_from_va();
            let case = format!("{label} getHabitatFromVA ({x},{y})");
            check(&mut failures, &format!("{case} value"), real_value, port_value);
            check(&mut failures, &format!("{case} rand state"), real_state, rng_state());
        }
        twins.destroy();
    }
    stop_stubbing();
    restore_hooks(saved);
    pin_rng(saved_rng);
    tracing::info!(
        "{test_name}: {validated} validated ({validated_checked} checked), {entered} entered, {exited} exited, {removed} removed, {returned} returned, {named} names ({named_show} show items)"
    );
    if validated == 0 || validated_checked == 0 || entered == 0 || exited == 0 || removed == 0 || returned == 0 {
        failures.push(format!(
            "a branch was never exercised (validated {validated}, checked {validated_checked}, entered {entered}, exited {exited}, removed {removed}, returned {returned})"
        ));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_UPDATE_BUILDING_TILES_LIVE`: `updateBuildingTiles` against the real body: every outcome of the walk to the
/// building (no building, no open slot, no entrance tile, an edge too costly, `goToDest` failing or succeeding) and every
/// tail (abort, clear, leave), with the path list rebuilt for each case so the dropped step is compared too.
pub(crate) fn run_ztunit_update_building_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_UPDATE_BUILDING_TILES_LIVE";
    let mut failures: Vec<String> = Vec::new();
    // The `goToDest` stubs live in the detours `ztguest_checks.rs` owns; installing those hooks early would change the
    // guest tests that run before this one, so this test is registered after them.
    if let Err(e) = install_stubs().and_then(|()| super::super::ztguest_checks::install_hooks()) {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = super::live_unit_types(2);
    let buildings = distinct_buildings(3);
    if types.is_empty() || buildings.is_empty() {
        return no_units(test_name, failure_log);
    }
    let saved = install_hooks();
    start_stubbing();
    let building_need = need_index(BUILDING_NEED_NAME_RVA);
    let world = globals().ztworldmgr();
    let threshold: i32 = get_from_memory(base() + MAX_EDGE_COST_RVA);
    RETURN_STRING.store(leaked_text(b"returning"), Ordering::SeqCst);
    let (width, height) = (world.map_x_size as i32, world.map_y_size as i32);
    let path_options: Vec<Vec<[i32; 3]>> = vec![
        vec![],
        vec![[width / 2, height / 2, 0]],
        vec![[width / 2, height / 2, 0], [width / 2 + 1, height / 2, 0]],
        vec![[width / 2, height / 2, 0], [width / 2 + 1, height / 2, 0], [width / 2 + 1, height / 2 + 1, 0]],
        vec![[-1, 3, 0]],
        vec![[2, 2, 0], [width + 5, 4, 0]],
    ];
    let mut exercised = std::collections::BTreeMap::<&'static str, u32>::new();
    let (mut guest_gave_up, mut guest_thoughts) = (0u32, 0u32);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = building_twins(unit_type);
        let is_guest_type = Some(unit_type) == crate::ztguest::live_support::find_guest_type();
        let real_path_list: [u32; 2] = [get_from_memory(twins.real.0 + unit_offsets::PATH_LIST), get_from_memory(twins.port.0 + unit_offsets::PATH_LIST)];
        for &(building, tile) in &buildings {
            for offset in [QUADRANTS[0], QUADRANTS[3]] {
                if failures.len() > 100 {
                    break;
                }
                for (path_index, path) in path_options.iter().enumerate() {
                    for (open, edge, returning, going, goal, abort, need_building, satisfier) in scenarios() {
                        // Vanilla reads through the destination tile unchecked, so it faults on a step off the map once a slot
                        // was found; the port takes the tail instead (documented in the plan).
                        let last_step = path.last().copied().unwrap_or([0, 0, 0]);
                        if open && (last_step[0] < 0 || last_step[1] < 0 || last_step[0] >= width || last_step[1] >= height) {
                            continue;
                        }
                        let slot_byte = 1u8;
                        for unit in twins.both() {
                            set_offset(unit, tile, offset);
                            save_to_memory(unit.0 + unit_offsets::GOAL_ID, goal);
                            save_to_memory(unit.0 + unit_offsets::NEED_INDEX, if need_building { building_need } else { 0 });
                            save_to_memory(unit.0 + BUILDING_SLOT, 0xffu8);
                            save_to_memory(unit.0 + unit_offsets::NON_PATH_COST, 0u32);
                            save_to_memory(unit.0 + GUEST_GAVE_UP, 0u8);
                        }
                        OPEN_SLOT_ANSWER.store(u32::from(open), Ordering::SeqCst);
                        OPEN_SLOT_BYTE.store(slot_byte, Ordering::SeqCst);
                        EDGE_COST.store(threshold + edge, Ordering::SeqCst);
                        IS_RETURNING.store(u32::from(returning), Ordering::SeqCst);
                        GO_TO_DEST_ANSWER.store(u32::from(going), Ordering::SeqCst);
                        NEED_OR_FUN_ANSWER.store(u32::from(satisfier), Ordering::SeqCst);
                        let case = format!(
                            "type {type_index} building {building:#x} offset {offset:?} path {path_index} open {open} edge {edge:+} returning {returning} going {going} goal {goal:#x} abort {abort} building need {need_building} satisfier {satisfier}"
                        );
                        let lists = [leaked_path(path), leaked_path(path)];
                        save_to_memory(twins.real.0 + unit_offsets::PATH_LIST, lists[0]);
                        save_to_memory(twins.port.0 + unit_offsets::PATH_LIST, lists[1]);
                        take_log();
                        let real_value = real_buildings::update_building_tiles(twins.real.0, abort);
                        let real_calls = take_log();
                        let port_value = twins.port.update_building_tiles(abort);
                        let port_calls = take_log();
                        check(&mut failures, &format!("{case} value"), real_value, port_value);
                        compare_log(&case, &real_calls, &port_calls, &mut failures);
                        compare_memory(&case, twins.real, twins.port, &mut failures);
                        let gave_up = [get_from_memory::<u8>(twins.real.0 + GUEST_GAVE_UP), get_from_memory::<u8>(twins.port.0 + GUEST_GAVE_UP)];
                        check(&mut failures, &format!("{case} guest gave-up byte"), gave_up[0], gave_up[1]);
                        guest_gave_up += u32::from(is_guest_type && gave_up[1] != 0);
                        check(&mut failures, &format!("{case} path steps"), path_steps(lists[0]), path_steps(lists[1]));
                        let key = match port_calls.last() {
                            Some(last) if last.starts_with("goalSet 0x83") => "left",
                            Some(last) if last.starts_with("abortGoal") => "aborted",
                            Some(last) if last == "clearGoal" => "cleared",
                            _ if port_calls.len() <= 1 => "declined",
                            _ if port_value => "started",
                            _ => "stopped",
                        };
                        *exercised.entry(key).or_default() += 1;
                    }
                }
            }
        }
        if is_guest_type {
            guest_thoughts += guest_thought_cases(&twins, &buildings, &mut failures);
        }
        // The vanilla destructor frees the constructor's lists, not the leaked ones.
        save_to_memory(twins.real.0 + unit_offsets::PATH_LIST, real_path_list[0]);
        save_to_memory(twins.port.0 + unit_offsets::PATH_LIST, real_path_list[1]);
        twins.destroy();
    }
    stop_stubbing();
    restore_hooks(saved);
    tracing::info!("{test_name}: {exercised:?}");
    tracing::info!("{test_name}: guest gave up {guest_gave_up}, guest thoughts {guest_thoughts}");
    if guest_gave_up == 0 {
        failures.push("the guest gave-up branch was never exercised".to_string());
    }
    if guest_thoughts == 0 {
        failures.push("the guest building-full thought was never exercised".to_string());
    }
    for outcome in ["started", "aborted", "cleared", "left", "declined"] {
        if exercised.get(outcome).copied().unwrap_or(0) == 0 {
            failures.push(format!("outcome {outcome:?} was never exercised: {exercised:?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `(open slot, edge cost relative to the threshold, returning, goToDest answer, goal, abort, building need, satisfier)`.
#[allow(clippy::type_complexity)]
fn scenarios() -> Vec<(bool, i32, bool, bool, u32, bool, bool, bool)> {
    let mut cases = Vec::new();
    for satisfier in [false, true] {
        for goal in [0x40u32, 0x66, 0x67, 0x6a] {
            for open in [false, true] {
                for edge in [-1, 0, 1] {
                    for returning in [false, true] {
                        for going in [false, true] {
                            for abort in [false, true] {
                                for need_building in [false, true] {
                                    cases.push((open, edge, returning, going, goal, abort, need_building, satisfier));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    cases
}

/// A leaked string object `{begin, end, cap}` holding `text`.
fn leaked_text(text: &[u8]) -> u32 {
    let bytes: &'static mut [u8] = Box::leak(text.to_vec().into_boxed_slice());
    let begin = bytes.as_ptr() as u32;
    leaked_words(vec![begin, begin + text.len() as u32, begin + text.len() as u32])
}

/// `ZTBuilding` / `ZTBuildingType` fields `updateBuildingTiles` reads to decide a full building turned a guest away.
const BUILDING_USERS_BEGIN: u32 = 0x170;
const BUILDING_USERS_END: u32 = 0x174;
const BUILDING_OCCUPANCY: u32 = 0x18c;
const TYPE_FULL_COUNT: u32 = 0x16c;
/// `BFUnit` word the guest-thought check reads, and the `ZTGuest` byte set when a guest gives up on a building.
const UNIT_THOUGHT_STATE: u32 = 0x180;
const GUEST_GAVE_UP: u32 = 0x33e;

/// The "building full" branch of `updateBuildingTiles` for a unit that is a guest: the building is made to look full (one
/// user record, occupancy at the type's limit; restored afterwards) and the guest-thought override logs the call, so the
/// real body and the port are compared on the thought itself, with and without a thought state. Returns how many cases
/// raised the thought.
fn guest_thought_cases(twins: &Twins, buildings: &[(u32, u32)], failures: &mut Vec<String>) -> u32 {
    use crate::ztguest::live_support::goals_overrides::THOUGHT;
    fn log_thought(_this: u32, id: u32, subject: u32, _habitat: u32) -> bool {
        if !stubbing() {
            return false;
        }
        log(format!("guestThought {id:#x} {subject:#x}"));
        true
    }
    let saved_hook = THOUGHT.lock().unwrap().replace(log_thought);
    let mut raised = 0;
    for &(building, tile) in buildings {
        let building_type: u32 = get_from_memory(building + unit_offsets::ENTITY_TYPE);
        if building_type == 0 {
            continue;
        }
        let saved_fields = [get_from_memory::<u32>(building + BUILDING_USERS_BEGIN), get_from_memory(building + BUILDING_USERS_END), get_from_memory(building + BUILDING_OCCUPANCY)];
        let records = leaked_words(vec![0; 9]);
        save_to_memory(building + BUILDING_USERS_BEGIN, records);
        save_to_memory(building + BUILDING_USERS_END, records + 0x24);
        save_to_memory(building + BUILDING_OCCUPANCY, get_from_memory::<u32>(building_type + TYPE_FULL_COUNT));
        for thought_state in [0u32, 1] {
            for abort in [false, true] {
                for unit in twins.both() {
                    set_offset(unit, tile, QUADRANTS[0]);
                    save_to_memory(unit.0 + unit_offsets::GOAL_ID, 0x40u32);
                    save_to_memory(unit.0 + unit_offsets::NEED_INDEX, 0u8);
                    save_to_memory(unit.0 + UNIT_THOUGHT_STATE, thought_state);
                    save_to_memory(unit.0 + GUEST_GAVE_UP, 0u8);
                }
                OPEN_SLOT_ANSWER.store(0, Ordering::SeqCst);
                NEED_OR_FUN_ANSWER.store(1, Ordering::SeqCst);
                IS_RETURNING.store(0, Ordering::SeqCst);
                let case = format!("guest full building {building:#x} thought state {thought_state} abort {abort}");
                take_log();
                let real_value = real_buildings::update_building_tiles(twins.real.0, abort);
                let real_calls = take_log();
                let port_value = twins.port.update_building_tiles(abort);
                let port_calls = take_log();
                check(failures, &format!("{case} value"), real_value, port_value);
                compare_log(&case, &real_calls, &port_calls, failures);
                compare_memory(&case, twins.real, twins.port, failures);
                let raised_here = port_calls.iter().any(|call| call.starts_with("guestThought 0x2803"));
                check(failures, &format!("{case} thought raised"), thought_state != 0, raised_here);
                raised += u32::from(raised_here);
            }
        }
        save_to_memory(building + BUILDING_USERS_BEGIN, saved_fields[0]);
        save_to_memory(building + BUILDING_USERS_END, saved_fields[1]);
        save_to_memory(building + BUILDING_OCCUPANCY, saved_fields[2]);
    }
    *THOUGHT.lock().unwrap() = saved_hook;
    raised
}
