//! Live checks of `ZTBuilding`'s slot geometry and exits (`ztbuilding/geometry.rs`) against the vanilla bodies: the
//! positions over fixtures of every building type at every rotation, the exit tests over the live buildings and units,
//! and `exitViaExit` and `findNearestOpenSlot` with the unit's, the AI's and `moveEntity`'s effects scripted.

use std::{
    mem::{offset_of, size_of},
    sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
        Mutex, Once,
    },
};

use glam::IVec3;
use openzt_detour::generated::bfmap::MOVE_ENTITY;

use super::ztguide_habitats::{install_rand_hook, rand_calls, script_rand, set_rand_intercept};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::{get_module_base, globals},
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztaimgr::target_list::{TargetList, VanillaPool},
    ztbuilding::{
        live_support::{build_standalone_building, destroy_standalone_building, geometry_real, intercept},
        ZTBuilding,
    },
    ztguest::{
        live_support::{build_standalone_guest, destroy_standalone_guest, find_guest_type},
        ZTGuest,
    },
    ztmegatilemgr::entity_type_matches,
};

const BUILDING_TAG_RVA: u32 = 0x0023_8680;
const ANIMAL_TAG_RVA: u32 = 0x0023_8690;
const GUEST_TAG_RVA: u32 = 0x0023_8700;
const MAX_PATH_COST_RVA: u32 = 0x0023_5494;
const POS: u32 = 0x114;
const ROTATION: u32 = 0x12c;
const MAX_FAILURES: usize = 40;

fn word(address: u32) -> u32 {
    get_from_memory(address)
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn read_position(address: u32) -> [i32; 3] {
    std::array::from_fn(|axis| get_from_memory(address + axis as u32 * 4))
}

fn write_position(address: u32, position: [i32; 3]) {
    for (axis, value) in position.into_iter().enumerate() {
        save_to_memory(address + axis as u32 * 4, value);
    }
}

fn entities() -> impl Iterator<Item = u32> {
    let world = globals().ztworldmgr();
    (world.entity_array_start()..world.entity_array_end()).step_by(4).map(word).filter(|&entity| entity != 0)
}

fn entities_of(tag_rva: u32, limit: usize) -> Vec<u32> {
    entities().filter(|&entity| unsafe { entity_type_matches(entity, tag_rva) }).take(limit).collect()
}

fn building_types() -> Vec<u32> {
    let world = globals().ztworldmgr();
    let tag = base() + BUILDING_TAG_RVA;
    (world.entity_type_array_start..world.entity_type_array_end)
        .step_by(4)
        .map(word)
        .filter(|&entity_type| {
            entity_type != 0 && {
                let is_cast_class: extern "thiscall" fn(u32, u32) -> bool = unsafe { std::mem::transmute(word(word(entity_type) + 0x1c)) };
                is_cast_class(entity_type, tag)
            }
        })
        .collect()
}

/// One live building per distinct type that has slots.
fn buildings_with_slots(limit: usize) -> Vec<ZTBuilding> {
    let mut seen = Vec::new();
    entities_of(BUILDING_TAG_RVA, usize::MAX)
        .into_iter()
        .map(ZTBuilding)
        .filter(|building| {
            let fresh = building.slot_count() > 0 && !seen.contains(&building.entity_type());
            seen.push(building.entity_type());
            fresh
        })
        .take(limit)
        .collect()
}

fn slot_record(building: ZTBuilding, slot: u32) -> u32 {
    word(building.entity_type() + offset_of!(ZTBuildingType, slots) as u32) + slot * size_of::<ZTBuildingSlotType>() as u32
}

fn vector_len(vector: u32, element: u32) -> u32 {
    word(vector + 4).wrapping_sub(word(vector)) / element
}

/// The slot indices to ask about: every slot, and some past the end.
fn slot_arguments(building: ZTBuilding) -> Vec<u32> {
    let count = building.slot_count() as u32;
    (0..count).chain([count, count + 1, 0xff]).collect()
}

fn check<T: PartialEq + std::fmt::Debug>(failures: &mut Vec<String>, label: impl FnOnce() -> String, real: T, port: T) {
    if real != port && failures.len() < MAX_FAILURES {
        failures.push(format!("{}: real {real:?}, port {port:?}", label()));
    }
}

/// `ZTBUILDING_GEOMETRY_LIVE`: the position getters over a fixture of every building type, turned each way.
pub(crate) fn run_ztbuilding_geometry_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_GEOMETRY_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let (mut slots_seen, mut entrances_seen, mut draws_seen) = (0u32, 0u32, 0u32);
    let offsets = [[0, 0, 0], [10, 3, 5], [-64, 128, -16], [i32::MAX, i32::MIN, 1]];
    for building_type in building_types() {
        let building = build_standalone_building(building_type);
        let this = building.0;
        // The fixture has no slots of its own: the getters read only the type's records.
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        let count = vector_len(records, size_of::<ZTBuildingSlotType>() as u32);
        write_position(this + POS, [4000, 3000, 48]);
        for rotation in [0u32, 1, 2, 3, 4, 5, 6, 7, 8] {
            save_to_memory(this + ROTATION, rotation);
            let label = |what: &str| format!("type {building_type:#x} rotation {rotation} {what}");
            for local in offsets {
                check(&mut failures, || label(&format!("gxPos {local:?}")), geometry_real::gx_pos_from_xy_pos(this, local), building.gx_pos_from_xy_pos(local));
            }
            let moved: Vec<[i32; 3]> = offsets.iter().map(|&local| building.gx_pos_from_xy_pos(local)).collect();
            check(&mut failures, || label("gxPos vector"), geometry_real::gx_positions(this, &offsets), moved);
            check(&mut failures, || label("gxPos empty vector"), geometry_real::gx_positions(this, &[]), Vec::new());
            for slot in (0..count).chain([count, 0xff]) {
                let label = |what: &str| format!("type {building_type:#x} rotation {rotation} slot {slot} {what}");
                check(&mut failures, || label("loc"), geometry_real::slot_loc(this, slot), building.slot_loc(slot));
                check(&mut failures, || label("exit"), geometry_real::slot_exit(this, slot), building.slot_exit(slot));
                check(&mut failures, || label("facing"), geometry_real::slot_facing(this, slot), building.slot_facing(slot));
                check(&mut failures, || label("hasDrawSlots"), geometry_real::slot_has_draw_slots(this, slot), building.slot_has_draw_slots(slot));
                let entrances = building.slot_entrances(slot);
                entrances_seen += entrances.len() as u32;
                check(&mut failures, || label("entrances"), geometry_real::slot_entrances(this, slot), entrances);
                // Vanilla reads past the draw positions without a check, so only the ones there are compared.
                let draws = if slot < count {
                    slots_seen += 1;
                    vector_len(word(records) + slot * size_of::<ZTBuildingSlotType>() as u32 + offset_of!(ZTBuildingSlotType, draw_positions) as u32, 12)
                } else {
                    1
                };
                for index in 0..draws {
                    draws_seen += u32::from(slot < count);
                    check(&mut failures, || label(&format!("drawLoc {index}")), geometry_real::slot_draw_loc(this, slot, index as i32), building.slot_draw_loc(slot, index as i32));
                }
            }
        }
        destroy_standalone_building(building);
        if failures.len() >= MAX_FAILURES {
            break;
        }
    }
    tracing::info!("{test_name}: {slots_seen} slot cases, {entrances_seen} entrances, {draws_seen} draw positions");
    if slots_seen == 0 || entrances_seen == 0 || draws_seen == 0 {
        failures.push(format!("a branch was never exercised (slots {slots_seen}, entrances {entrances_seen}, draws {draws_seen})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// Positions around a building's slots: their exits and entrances, tiles around the building and one off the map.
fn probe_positions(building: ZTBuilding) -> Vec<[i32; 3]> {
    let mut positions = vec![[-1000, -1000, 0]];
    for slot in 0..building.slot_count() as u32 {
        positions.push(building.slot_exit(slot));
        positions.extend(building.slot_entrances(slot));
    }
    let [x, y, z] = read_position(building.0 + POS);
    for dx in -2..=2 {
        for dy in -2..=2 {
            positions.push([x + dx * 64, y + dy * 64, z]);
        }
    }
    positions.sort_unstable();
    positions.dedup();
    positions
}

/// `ZTBUILDING_EXITS_LIVE`: `isValidExit`, `getValidEntranceForExit` and `getValidExit` over the live buildings, for no
/// unit and for live guests and animals, with the type using its exit and not.
pub(crate) fn run_ztbuilding_exits_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_EXITS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let mut units = vec![0u32];
    units.extend(entities_of(GUEST_TAG_RVA, 3));
    units.extend(entities_of(ANIMAL_TAG_RVA, 6));
    let (mut valid, mut invalid, mut found, mut missing) = (0u32, 0u32, 0u32, 0u32);
    let uses_exit = offset_of!(ZTBuildingType, user_uses_exit) as u32;
    for building in buildings_with_slots(24) {
        let this = building.0;
        let building_type = building.entity_type();
        for position in probe_positions(building) {
            for &unit in &units {
                for slot in std::iter::once(-1).chain(slot_arguments(building).into_iter().map(|slot| slot as i32)) {
                    let real = geometry_real::is_valid_exit(this, unit, position, slot);
                    let port = building.is_valid_exit(unit, position, slot);
                    valid += u32::from(port);
                    invalid += u32::from(!port);
                    check(&mut failures, || format!("type {building_type:#x} isValidExit unit {unit:#x} {position:?} slot {slot}"), real, port);
                }
            }
        }
        let saved: u8 = get_from_memory(building_type + uses_exit);
        for flag in [0u8, 1] {
            save_to_memory(building_type + uses_exit, flag);
            for &unit in &units {
                for slot in slot_arguments(building).into_iter().chain([i32::MIN as u32]) {
                    let label = |what: &str| format!("type {building_type:#x} uses exit {flag} {what} unit {unit:#x} slot {slot:#x}");
                    for entrance_only in [false, true] {
                        let (mut real_out, mut port_out) = ([7i32; 3], [7i32; 3]);
                        let (real, port) = if entrance_only {
                            (
                                geometry_real::valid_entrance_for_exit(this, &mut real_out, unit, slot),
                                building.valid_entrance_for_exit(&mut port_out, unit, slot),
                            )
                        } else {
                            (geometry_real::valid_exit(this, &mut real_out, unit, slot), building.valid_exit(&mut port_out, unit, slot))
                        };
                        found += u32::from(port);
                        missing += u32::from(!port);
                        let what = if entrance_only { "getValidEntranceForExit" } else { "getValidExit" };
                        check(&mut failures, || label(what), (real, real_out), (port, port_out));
                    }
                }
            }
        }
        save_to_memory(building_type + uses_exit, saved);
        if failures.len() >= MAX_FAILURES {
            break;
        }
    }
    tracing::info!("{test_name}: {} units, {valid} valid, {invalid} invalid, {found} exits found, {missing} not", units.len() - 1);
    if valid == 0 || invalid == 0 || found == 0 || missing == 0 || units.len() < 2 {
        failures.push(format!("a branch was never exercised (valid {valid}, invalid {invalid}, found {found}, missing {missing})"));
    }
    finish_test(test_name, failures, failure_log)
}

// ---- scripted collaborators ----------------------------------------------------------------------------------------

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
static STUBBING: AtomicBool = AtomicBool::new(false);
static EDGE_COST: AtomicI32 = AtomicI32::new(0);
/// Which `getPath` calls (by order) succeed, and which slots take the unit.
static PATH_ANSWERS: AtomicU32 = AtomicU32::new(0);
static PATH_CALLS: AtomicU32 = AtomicU32::new(0);
static OPEN_SLOTS: AtomicU32 = AtomicU32::new(0);

fn log(entry: String) {
    LOG.lock().unwrap().push(entry);
}

fn take_log() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

#[openzt_detour_macro::detour_mod]
mod stubs {
    use super::*;

    #[detour(MOVE_ENTITY)]
    unsafe extern "stdcall" fn move_entity(entity: *const u32, position: *const IVec3) {
        if STUBBING.load(Ordering::SeqCst) {
            log(format!("moveEntity {:#x} {:?}", entity as u32, read_position(position as u32)));
        } else {
            unsafe { MOVE_ENTITY_DETOUR.call(entity, position) }
        }
    }
}

fn install() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { stubs::init_detours() } {
            result = Err(format!("could not install the exit stubs: {e:?}"));
        }
    });
    result
}

extern "thiscall" fn fake_stop_everything(this: u32, a: u32, b: u32, c: u32) {
    log(format!("stopEverything {this:#x} {} {} {}", a & 0xff, b & 0xff, c & 0xff));
}
extern "thiscall" fn fake_remove_from_map(this: u32, flag: u32) {
    log(format!("removeFromMap {this:#x} {}", flag & 0xff));
}
extern "thiscall" fn fake_reset_idle_anim(this: u32) {
    log(format!("resetIdleAnim {this:#x}"));
}
extern "thiscall" fn fake_set_visible(this: u32, flag: u32) {
    log(format!("setVisible {this:#x} {}", flag & 0xff));
}
extern "thiscall" fn fake_get_edge_cost(this: u32, tile: u32, direction: u32) -> i32 {
    log(format!("getEdgeCost {this:#x} {tile:#x} {direction}"));
    EDGE_COST.load(Ordering::SeqCst)
}

/// `BFAIMgr::getPath`: writes a path naming the call into the unit's target list, whether or not it succeeds.
extern "thiscall" fn fake_get_path(_this: u32, start: u32, end: u32, unit: u32, flag: u32) -> u32 {
    let call = PATH_CALLS.fetch_add(1, Ordering::SeqCst);
    let (start, end) = (read_position(start), read_position(end));
    log(format!("getPath {start:?} {end:?} {unit:#x} {}", flag & 0xff));
    let steps: Vec<[i32; 3]> = (0..=call % 4).map(|step| [call as i32, step as i32, end[0]]).collect();
    unsafe { TargetList::of_unit(unit).assign(&mut VanillaPool, &steps) };
    (PATH_ANSWERS.load(Ordering::SeqCst) >> (call % 32)) & 1
}

fn scripted_can_add_user(_building: u32, _unit: u32, slot: u32) -> Option<bool> {
    if !STUBBING.load(Ordering::SeqCst) {
        return None;
    }
    log(format!("canAddUser {slot}"));
    Some(OPEN_SLOTS.load(Ordering::SeqCst) >> (slot % 32) & 1 != 0)
}

fn scripted_valid_exit(_building: u32, _unit: u32, slot: u32) -> Option<bool> {
    if !STUBBING.load(Ordering::SeqCst) {
        return None;
    }
    log(format!("getValidExit refused {slot}"));
    Some(false)
}

/// A leaked copy of the vtable at `object`, `slots` entries long, with `scripted` slots replaced.
fn scripted_vtable(object: u32, slots: u32, scripted: &[(u32, u32)]) -> u32 {
    let real: u32 = get_from_memory(object);
    let mut vtable: Vec<u32> = (0..slots).map(|slot| get_from_memory(real + slot * 4)).collect();
    for &(offset, function) in scripted {
        vtable[(offset / 4) as usize] = function;
    }
    Box::leak(vtable.into_boxed_slice()).as_ptr() as u32
}

/// A fixture guest standing on `position` with the unit slots the exit methods call scripted.
fn scripted_guest(guest_type: u32) -> (ZTGuest, u32) {
    let guest = build_standalone_guest(guest_type);
    let real_vtable = word(guest.0);
    let vtable = scripted_vtable(
        guest.0,
        220,
        &[
            (0x78, fake_remove_from_map as *const () as u32),
            (0xcc, fake_set_visible as *const () as u32),
            (0x13c, fake_reset_idle_anim as *const () as u32),
            (0x160, fake_get_edge_cost as *const () as u32),
            (0x198, fake_stop_everything as *const () as u32),
        ],
    );
    save_to_memory(guest.0, vtable);
    (guest, real_vtable)
}

/// `ZTBUILDING_EXIT_VIA_LIVE`: `exitViaExit` over the live buildings with the type's exit flags each way, a passable and
/// an impassable step, and an exit refused.
pub(crate) fn run_ztbuilding_exit_via_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_EXIT_VIA_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    let (guest, real_vtable) = scripted_guest(guest_type);
    let limit: i32 = get_from_memory(base() + MAX_PATH_COST_RVA);
    let flags = [
        offset_of!(ZTBuildingType, user_uses_exit) as u32,
        offset_of!(ZTBuildingType, user_uses_entrance_as_emergency_exit) as u32,
        offset_of!(ZTBuildingType, hide_user) as u32,
    ];
    let (mut moved, mut removed, mut stepped) = (0u32, 0u32, 0u32);
    STUBBING.store(true, Ordering::SeqCst);
    for building in buildings_with_slots(16) {
        let building_type = building.entity_type();
        let saved: Vec<u8> = flags.iter().map(|&flag| get_from_memory(building_type + flag)).collect();
        write_position(guest.0 + POS, read_position(building.0 + POS));
        for combination in 0u32..8 {
            for (bit, &flag) in flags.iter().enumerate() {
                save_to_memory(building_type + flag, (combination >> bit & 1) as u8);
            }
            for (cost, refuse) in [(0, false), (limit.saturating_sub(1), false), (limit, false), (0, true)] {
                EDGE_COST.store(cost, Ordering::SeqCst);
                *intercept::VALID_EXIT.lock().unwrap() = refuse.then_some(scripted_valid_exit as fn(u32, u32, u32) -> Option<bool>);
                for slot in slot_arguments(building) {
                    for unit in [0u32, guest.0] {
                        take_log();
                        let real = geometry_real::exit_via_exit(building.0, unit, slot);
                        let real_log = take_log();
                        let port = building.exit_via_exit(unit, slot);
                        let port_log = take_log();
                        moved += u32::from(port_log.iter().any(|call| call.starts_with("moveEntity")));
                        removed += u32::from(port_log.iter().any(|call| call.starts_with("removeFromMap")));
                        stepped += u32::from(port_log.iter().any(|call| call.starts_with("getEdgeCost")));
                        check(
                            &mut failures,
                            || format!("type {building_type:#x} flags {combination:#b} cost {cost} refuse {refuse} slot {slot} unit {unit:#x}"),
                            (real, real_log),
                            (port, port_log),
                        );
                    }
                }
            }
        }
        *intercept::VALID_EXIT.lock().unwrap() = None;
        for (&flag, value) in flags.iter().zip(saved) {
            save_to_memory(building_type + flag, value);
        }
        if failures.len() >= MAX_FAILURES {
            break;
        }
    }
    STUBBING.store(false, Ordering::SeqCst);
    save_to_memory(guest.0, real_vtable);
    destroy_standalone_guest(guest);
    tracing::info!("{test_name}: {moved} moved, {removed} removed, {stepped} steps priced");
    if moved == 0 || removed == 0 || stepped == 0 {
        failures.push(format!("a branch was never exercised (moved {moved}, removed {removed}, steps {stepped})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTBUILDING_FIND_SLOT_LIVE`: `findNearestOpenSlot` over the live buildings with a scripted `getPath`, `canAddUser` and
/// `rand`: which paths succeed, which slots are open, ordered and random slot choice, and the path left in the unit.
pub(crate) fn run_ztbuilding_find_slot_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_FIND_SLOT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_rand_hook() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    let ai = globals().ztaimgr_ptr() as u32;
    if ai == 0 {
        return finish_test(test_name, vec!["no AI manager".to_string()], failure_log);
    }
    let (guest, real_vtable) = scripted_guest(guest_type);
    let ai_vtable = word(ai);
    save_to_memory(ai, scripted_vtable(ai, 32, &[(0x18, fake_get_path as *const () as u32)]));
    let saved_can_add = intercept::CAN_ADD_USER.lock().unwrap().replace(scripted_can_add_user);
    let path = TargetList::of_unit(guest.0);
    let original_path = path.positions();
    let random_flag = offset_of!(ZTBuildingType, use_random_slot) as u32;
    let (mut found, mut not_found, mut restored, mut random_picks) = (0u32, 0u32, 0u32, 0u32);
    STUBBING.store(true, Ordering::SeqCst);
    set_rand_intercept(true);

    for building in buildings_with_slots(16) {
        let building_type = building.entity_type();
        let saved: u8 = get_from_memory(building_type + random_flag);
        let [x, y, z] = read_position(building.0 + POS);
        let has_entrances = (0..building.slot_count() as u32)
            .any(|slot| vector_len(slot_record(building, slot) + offset_of!(ZTBuildingSlotType, entrances) as u32, 12) > 0);
        for position in [[x, y, z], [x - 200, y + 90, z], [x + 500, y - 500, z]] {
            write_position(guest.0 + POS, position);
            for random in [0u8, 1] {
                save_to_memory(building_type + random_flag, random);
                for open in [0u32, u32::MAX, 0b0101, 0b1010] {
                    for answers in [0u32, u32::MAX, 0b0001, 0b0110_1010, 0b1111_1100] {
                        let mut results = Vec::new();
                        for real in [true, false] {
                            OPEN_SLOTS.store(open, Ordering::SeqCst);
                            PATH_ANSWERS.store(answers, Ordering::SeqCst);
                            PATH_CALLS.store(0, Ordering::SeqCst);
                            script_rand(&[3, 1, 4, 1, 5]);
                            unsafe { path.assign(&mut VanillaPool, &[[9, 9, 9], [8, 8, 8]]) };
                            take_log();
                            let (mut slot, mut entrance) = (0xeeee_u32, [7i32; 3]);
                            let answer = if real {
                                geometry_real::find_nearest_open_slot(building.0, guest.0, &mut slot, &mut entrance)
                            } else {
                                // The search writes both through their addresses.
                                let answer = building.find_nearest_open_slot(guest.0, std::ptr::from_mut(&mut slot) as u32, entrance.as_mut_ptr() as u32);
                                slot = get_from_memory(std::ptr::from_ref(&slot) as u32);
                                entrance = read_position(entrance.as_ptr() as u32);
                                answer
                            };
                            results.push((answer, slot, entrance, path.positions(), take_log(), rand_calls()));
                        }
                        let port = &results[1];
                        found += u32::from(port.0);
                        not_found += u32::from(!port.0 && has_entrances);
                        restored += u32::from(port.3 == [[9, 9, 9], [8, 8, 8]] && port.4.iter().any(|call| call.starts_with("getPath")));
                        random_picks += port.5 as u32;
                        check(
                            &mut failures,
                            || format!("type {building_type:#x} at {position:?} random {random} open {open:#b} answers {answers:#b}"),
                            &results[0],
                            &results[1],
                        );
                    }
                }
            }
        }
        save_to_memory(building_type + random_flag, saved);
        if failures.len() >= MAX_FAILURES {
            break;
        }
    }

    // A null unit finds nothing. Vanilla asks it for its grid position before checking, so only the port is run.
    if buildings_with_slots(1).first().is_some_and(|building| building.find_nearest_open_slot(0, 0, 0)) {
        failures.push("a null unit found a slot".to_string());
    }

    set_rand_intercept(false);
    STUBBING.store(false, Ordering::SeqCst);
    unsafe { path.assign(&mut VanillaPool, &original_path) };
    *intercept::CAN_ADD_USER.lock().unwrap() = saved_can_add;
    save_to_memory(ai, ai_vtable);
    save_to_memory(guest.0, real_vtable);
    destroy_standalone_guest(guest);
    tracing::info!("{test_name}: {found} found, {not_found} not found, {restored} paths restored, {random_picks} random picks");
    if found == 0 || not_found == 0 || restored == 0 || random_picks == 0 {
        failures.push(format!("a branch was never exercised (found {found}, not found {not_found}, restored {restored}, random {random_picks})"));
    }
    finish_test(test_name, failures, failure_log)
}
