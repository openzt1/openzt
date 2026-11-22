//! Live checks for `ztguest/checks.rs`: the need and environment checks, `doConditionals`, `updateStatusVariables`
//! and the leave-zoo predicates. Each case builds three guests with the real vanilla constructor over a cloned guest
//! type whose check fields are randomised, plants identical pseudo-random state, and runs the real body, the port and
//! the hooked address with the shared RNG pinned. The collaborators the checks call (thoughts, satisfier searches,
//! trash and seat searches, tiles, water tests and the guest's own virtual slots) are scripted by a deterministic
//! generator and logged, so the three poles must agree on the whole call sequence, the RNG state, the return value and
//! the guest's memory.

use std::{
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, Once,
    },
};

use openzt_detour::{
    generated::{
    bfentity::GET_TILE,
    bfunit::GO_TO_DEST_0,
    ztanimal::PREYS_ON_MAN,
    ztguest::{
        CHECK_LEAVE_ZOO, CHECK_PREDATOR, DO_BATHROOM_CHECK, DO_BUY_SOUVENIR_CHECK, DO_CHASE_CHECK, DO_CONDITIONALS, DO_ENERGY_CHECK, DO_ENVIRONMENT_EFFECT_CHECK,
        DO_HUNGER_CHECK, DO_LEAVE_ZOO_CHECK, DO_THIRSTY_CHECK, DO_TRASH_CHECK, F_GUEST_THOUGHT, LEAVE_OK,
        PICK_RANDOM_BUILDING_OF_TYPE, UPDATE_STATUS_VARIABLES,
    },
    zthabitat::GET_ALL_ANIMALS,
    ztui_guestinfo::SET_GUEST_VISIBLE,
    ztunit::{
        DO_BUILDING as ZTUNIT_DO_BUILDING, FIND_ANY_SATISFIER, FIND_NEAREST_SATISFIER, IS_FUN_SATISFIER, IS_NEED_OR_FUN_SATISFIER, IS_ON_WATER,
        UPDATE_BUILDING_TILES,
    },
    ztworldmgr::GET_NEED_INDEX,
    },
    FunctionDef,
};

use super::{
    ztguest_leaf::{compare_guests, Lcg},
    ztguide::{base, pin_rng, rng},
};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguest::{
        guest_offsets,
        live_support::{self, checks_real as real, movement_real, viewing_real},
        ZTGuest,
    },
    ztkeeper::unit_offsets,
};

/// Scripted collaborator state: an LCG drives every answer, every call is logged.
struct Script {
    on: bool,
    state: u32,
    log: Vec<String>,
    /// Addresses `BFEntity::getTile` answers for; every other entity gets the real answer.
    fixtures: Vec<u32>,
}

/// Vanilla `leaveOK` dereferences the target slot's entity without a null check, so that case never sees a null one.
static NON_NULL_TARGET: AtomicBool = AtomicBool::new(false);

/// Set while the movement searches themselves run, so the override slots stand aside and the real search body runs.
static SEARCH_UNDER_TEST: AtomicBool = AtomicBool::new(false);

/// `ZTGuest::throwTrash(this, 0)` and `ZTWorldMgr::removeFromMemberLists(guest)`, which `generated.rs` does not name.
const THROW_TRASH_DEF: FunctionDef<unsafe extern "thiscall" fn(*const u32, u32) -> bool> = FunctionDef::new(0x0052_da40);
const REMOVE_FROM_MEMBER_LISTS_DEF: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32)> = FunctionDef::new(0x0046_b5bf);

/// `BFUnit::getPath(tile, flag)` (which leaves its answer in `AL`), `BFUnit::goToDest(0, 0)`, `ZTGuide::addGuest(id)`
/// and `habitatFacing(guest)` (the habitat the guest faces), which `generated.rs` does not describe exactly.
const GET_PATH_DEF: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32, bool) -> bool> = FunctionDef::new(0x0042_2238);
const GO_TO_DEST_NONE_DEF: FunctionDef<unsafe extern "thiscall" fn(*const u32, u32, u32) -> u32> = FunctionDef::new(0x0042_2127);
const GUIDE_ADD_GUEST_DEF: FunctionDef<unsafe extern "thiscall" fn(*const u32, u32) -> u32> = FunctionDef::new(0x0058_dad9);
const HABITAT_FACING_DEF: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef::new(0x005a_0774);

static SCRIPT: Mutex<Script> = Mutex::new(Script { on: false, state: 0, log: Vec::new(), fixtures: Vec::new() });

fn roll(modulus: u32) -> u32 {
    let mut script = SCRIPT.lock().unwrap();
    script.state = script.state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (script.state >> 8) % modulus
}

fn record(entry: String) {
    SCRIPT.lock().unwrap().log.push(entry);
}

fn scripted() -> bool {
    SCRIPT.lock().unwrap().on
}

/// Leaked zeroed memory: a tile the scripted `getTile` hands out, a building and an entity the slots return.
fn leaked(bytes: usize) -> u32 {
    Box::leak(vec![0u8; bytes].into_boxed_slice()).as_ptr() as u32
}

fn fake_tile() -> u32 {
    static TILE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *TILE.get_or_init(|| leaked(0x200))
}

extern "thiscall" fn entity_predicate(_this: u32) -> u32 {
    let answer = roll(3) == 0;
    record(format!("entity predicate {answer}"));
    answer as u32
}

/// A fake entity: vtable slot `+0x20` is a scripted predicate; `+0x2c` and `+0x19c` hold values planted per case.
fn fake_entity() -> u32 {
    static ENTITY: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *ENTITY.get_or_init(|| {
        let mut vtable = vec![0u32; 0x100];
        vtable[0x20 / 4] = entity_predicate as *const () as usize as u32;
        let vtable = Box::leak(vtable.into_boxed_slice());
        let entity = leaked(0x400);
        save_to_memory(entity, vtable.as_ptr() as u32);
        entity
    })
}

fn fake_habitat() -> u32 {
    static HABITAT: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *HABITAT.get_or_init(|| leaked(0x100))
}

fn fake_guide() -> u32 {
    static GUIDE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *GUIDE.get_or_init(|| leaked(0x400))
}

const FAKE_ANIMALS: usize = 6;

/// The animal list `getAllAnimals` hands out (`{begin, end, cap}`) and its animals.
fn fake_animals() -> (u32, Vec<u32>) {
    static ANIMALS: std::sync::OnceLock<(u32, Vec<u32>)> = std::sync::OnceLock::new();
    ANIMALS
        .get_or_init(|| {
            let animals: Vec<u32> = (0..FAKE_ANIMALS).map(|_| leaked(0x400)).collect();
            let array = leaked(FAKE_ANIMALS * 4);
            for (index, animal) in animals.iter().enumerate() {
                save_to_memory(array + index as u32 * 4, *animal);
            }
            let list = leaked(12);
            save_to_memory(list, array);
            save_to_memory(list + 4, array + FAKE_ANIMALS as u32 * 4);
            save_to_memory(list + 8, array + FAKE_ANIMALS as u32 * 4);
            (list, animals)
        })
        .clone()
}

fn fake_building() -> u32 {
    static BUILDING: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *BUILDING.get_or_init(|| leaked(0x400))
}

extern "thiscall" fn slot_busy(_this: u32) -> u32 {
    let answer = roll(7) == 0;
    record(format!("slot busy {answer}"));
    answer as u32
}

extern "thiscall" fn slot_target(_this: u32) -> u32 {
    let answer = if roll(6) == 0 && !NON_NULL_TARGET.load(Ordering::SeqCst) { 0 } else { fake_entity() };
    record(format!("slot target {}", answer != 0));
    answer
}

extern "thiscall" fn slot_predator(_this: u32, _arg: u32) -> u32 {
    let answer = match roll(3) {
        0 => 0,
        1 => live_entity_id(),
        _ => 0xdead,
    };
    record(format!("slot predator {answer:#x}"));
    answer
}

extern "thiscall" fn slot_go_to_tile(_this: u32, tile: u32) -> u32 {
    record(format!("slot goToTile {}", tile != 0));
    0
}

extern "thiscall" fn slot_trash_target(_this: u32, _flag: u32, arg: u32) -> u32 {
    let answer = if roll(4) == 0 { 0 } else { fake_entity() };
    record(format!("slot trashTarget {arg} {}", answer != 0));
    answer
}

extern "thiscall" fn slot_clear_goal(_this: u32) -> u32 {
    record("slot clearGoal".to_string());
    0
}

extern "thiscall" fn slot_goal_set(_this: u32, goal: u32, flag: u32) -> u32 {
    record(format!("slot goalSet {goal:#x} {flag}"));
    0
}

extern "thiscall" fn slot_set_return_id(_this: u32, id: u32) -> u32 {
    record(format!("slot setReturnBuildingNameID {id}"));
    0
}

extern "thiscall" fn slot_set_return_string(_this: u32, string: u32) -> u32 {
    let (begin, end): (u32, u32) = (get_from_memory(string), get_from_memory(string + 4));
    record(format!("slot setReturnBuildingString {}", end.wrapping_sub(begin)));
    0
}

extern "thiscall" fn slot_has_pressing_need(_this: u32) -> u32 {
    let answer = roll(2) == 0;
    record(format!("slot hasPressingNeed {answer}"));
    answer as u32
}

extern "thiscall" fn slot_goto_nearby_need(_this: u32) -> u32 {
    let answer = roll(2) == 0;
    record(format!("slot gotoNearbyNeed {answer}"));
    answer as u32
}

extern "thiscall" fn slot_find_nearby(_this: u32, tile: u32, out_need: u32, out_building: u32, range: u32) -> u32 {
    let found = roll(3) != 0;
    let need = [0xffu8, 3, 7][roll(3) as usize];
    let building = if roll(3) == 0 { 0 } else { fake_building() };
    if found {
        save_to_memory(out_need, need);
        save_to_memory(out_building, building);
    }
    record(format!("slot findNearbyNeedSatisfier {} {found} {need} {} {range}", tile != 0, building != 0 && found));
    found as u32
}

extern "thiscall" fn slot_is_doing_something(_this: u32) -> u32 {
    let answer = roll(4) == 0;
    record(format!("slot isDoingSomething {answer}"));
    answer as u32
}

extern "thiscall" fn slot_habitat_from_tile(_this: u32, tile: u32) -> u32 {
    let answer = if roll(3) == 0 && !NON_NULL_TARGET.load(Ordering::SeqCst) { 0 } else { fake_habitat() };
    record(format!("slot habitatFromTile {} {}", tile != 0, answer != 0));
    answer
}

extern "thiscall" fn slot_set_target_tile(_this: u32, tile: u32, flag: u32) -> u32 {
    record(format!("slot setTargetTile {} {flag}", tile != 0));
    0
}

extern "thiscall" fn slot_stop_everything(_this: u32, a: u32, b: u32, c: u32) -> u32 {
    record(format!("slot stopEverything {a} {b} {c}"));
    0
}

extern "thiscall" fn slot_init_status_vars(_this: u32) -> u32 {
    record("slot initStatusVars".to_string());
    0
}

/// The id of some live entity, so `getEntity` has something to find.
fn live_entity_id() -> u32 {
    static ID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *ID.get_or_init(|| live_support::find_live_guests().first().map(|(entity, _)| get_from_memory::<u32>(*entity + unit_offsets::ENTITY_ID)).unwrap_or(0xdead))
}

#[openzt_detour_macro::detour_mod]
mod check_hooks {
    use super::*;

    #[detour(F_GUEST_THOUGHT)]
    unsafe extern "thiscall" fn thought(this: *const u32, id: u32, subject: *const i32, other: *const i32) {
        if scripted() {
            record(format!("thought {id:#x} {} {}", subject as u32 != 0, other as u32 != 0));
        } else {
            unsafe { F_GUEST_THOUGHT_DETOUR.call(this, id, subject, other) }
        }
    }

    #[detour(PICK_RANDOM_BUILDING_OF_TYPE)]
    unsafe extern "stdcall" fn pick(name: *const i32, out: *const i32) {
        if scripted() {
            let building = if roll(2) == 0 { 0 } else { fake_building() };
            save_to_memory(out as u32, building);
            record(format!("pickRandomBuildingOfType {:#x} {}", name as u32 - base(), building != 0));
        } else {
            unsafe { PICK_RANDOM_BUILDING_OF_TYPE_DETOUR.call(name, out) }
        }
    }

    #[detour(IS_NEED_OR_FUN_SATISFIER)]
    unsafe extern "thiscall" fn satisfier(this: *const u32, grid: i32, record_ptr: *const u32) -> bool {
        if scripted() {
            let answer = roll(4) == 0;
            let words: Vec<u32> = (0..3).map(|i| get_from_memory::<u32>(record_ptr as u32 + i * 4)).collect();
            record(format!("isNeedOrFunSatisfier {grid:#x} {words:x?} {answer}"));
            answer
        } else {
            unsafe { IS_NEED_OR_FUN_SATISFIER_DETOUR.call(this, grid, record_ptr) }
        }
    }

    #[detour(FIND_NEAREST_SATISFIER)]
    unsafe extern "thiscall" fn nearest(this: *const u32, tile: i32, name: *const i32, out: *const u32, range: i32, other: i32) -> bool {
        if scripted() {
            let found = roll(3) != 0;
            let building = if roll(3) == 0 && !NON_NULL_TARGET.load(Ordering::SeqCst) { 0 } else { fake_building() };
            if found {
                save_to_memory(out as u32, building);
            }
            record(format!("findNearestSatisfier {} {:#x} {found} {} {range:#x} {other}", tile != 0, name as u32 - base(), building != 0 && found));
            found
        } else {
            unsafe { FIND_NEAREST_SATISFIER_DETOUR.call(this, tile, name, out, range, other) }
        }
    }

    #[detour(FIND_ANY_SATISFIER)]
    unsafe extern "thiscall" fn any(this: *const u32, tile: i32, name: *const i32, out: *const i32, range: i32, other: i32) -> bool {
        if scripted() {
            let found = roll(3) != 0;
            let building = if roll(3) == 0 && !NON_NULL_TARGET.load(Ordering::SeqCst) { 0 } else { fake_building() };
            if found {
                save_to_memory(out as u32, building);
            }
            record(format!("findAnySatisfier {} {:#x} {found} {} {range:#x} {other}", tile != 0, name as u32 - base(), building != 0 && found));
            found
        } else {
            unsafe { FIND_ANY_SATISFIER_DETOUR.call(this, tile, name, out, range, other) }
        }
    }

    #[detour(UPDATE_BUILDING_TILES)]
    unsafe extern "thiscall" fn update_tiles(this: *const u32, flag: bool) -> u32 {
        if scripted() {
            let answer = roll(3) != 0;
            record(format!("updateBuildingTiles {flag} {answer}"));
            answer as u32
        } else {
            unsafe { UPDATE_BUILDING_TILES_DETOUR.call(this, flag) }
        }
    }

    #[detour(THROW_TRASH_DEF)]
    unsafe extern "thiscall" fn throw_trash(this: *const u32, arg: u32) -> bool {
        if scripted() {
            record(format!("throwTrash {arg}"));
            true
        } else {
            unsafe { THROW_TRASH_DEF_DETOUR.call(this, arg) }
        }
    }

    #[detour(REMOVE_FROM_MEMBER_LISTS_DEF)]
    unsafe extern "thiscall" fn remove_from_member_lists(this: *const u32, guest: *const u32) {
        if scripted() {
            record("removeFromMemberLists".to_string());
        } else {
            unsafe { REMOVE_FROM_MEMBER_LISTS_DEF_DETOUR.call(this, guest) }
        }
    }

    #[detour(GET_PATH_DEF)]
    unsafe extern "thiscall" fn get_path(this: *const u32, tile: *const u32, flag: bool) -> bool {
        if scripted() {
            let answer = roll(3) == 0;
            record(format!("getPath {} {flag} {answer}", tile as u32 != 0));
            answer
        } else {
            unsafe { GET_PATH_DEF_DETOUR.call(this, tile, flag) }
        }
    }

    #[detour(GO_TO_DEST_NONE_DEF)]
    unsafe extern "thiscall" fn go_to_dest_none(this: *const u32, name: u32, arg: u32) -> u32 {
        if scripted() {
            let answer = roll(3) != 0;
            record(format!("goToDest {name} {arg} {answer}"));
            answer as u32
        } else {
            unsafe { GO_TO_DEST_NONE_DEF_DETOUR.call(this, name, arg) }
        }
    }

    #[detour(GUIDE_ADD_GUEST_DEF)]
    unsafe extern "thiscall" fn guide_add_guest(this: *const u32, id: u32) -> u32 {
        if scripted() {
            record(format!("addGuest {id:#x}"));
            0
        } else {
            unsafe { GUIDE_ADD_GUEST_DEF_DETOUR.call(this, id) }
        }
    }

    #[detour(HABITAT_FACING_DEF)]
    unsafe extern "cdecl" fn habitat_in_front(guest: u32) -> u32 {
        if scripted() {
            let answer = if roll(4) == 0 { 0 } else { fake_habitat() };
            record(format!("habitatFacing {}", answer != 0));
            answer
        } else {
            unsafe { HABITAT_FACING_DEF_DETOUR.call(guest) }
        }
    }

    #[detour(GET_ALL_ANIMALS)]
    unsafe extern "thiscall" fn all_animals(this: *const u32, flag: i8) -> *const i32 {
        if scripted() {
            record(format!("getAllAnimals {flag}"));
            fake_animals().0 as *const i32
        } else {
            unsafe { GET_ALL_ANIMALS_DETOUR.call(this, flag) }
        }
    }

    #[detour(IS_FUN_SATISFIER)]
    unsafe extern "stdcall" fn is_fun_satisfier(building: i32, position: *const u32) -> u32 {
        if scripted() {
            let answer = roll(2) == 0 && building != 0;
            let words: Vec<u32> = (0..3).map(|i| get_from_memory::<u32>(position as u32 + i * 4)).collect();
            record(format!("isFunSatisfier {} {words:x?} {answer}", building != 0));
            answer as u32
        } else {
            unsafe { IS_FUN_SATISFIER_DETOUR.call(building, position) }
        }
    }

    #[detour(ZTUNIT_DO_BUILDING)]
    unsafe extern "thiscall" fn unit_do_building(this: *const u32) {
        if scripted() {
            record("ZTUnit::doBuilding".to_string());
        } else {
            unsafe { ZTUNIT_DO_BUILDING_DETOUR.call(this) }
        }
    }

    #[detour(SET_GUEST_VISIBLE)]
    unsafe extern "cdecl" fn set_guest_visible(this: *const u32, flag: i8) {
        if scripted() {
            record(format!("setGuestVisible {flag}"));
        } else {
            unsafe { SET_GUEST_VISIBLE_DETOUR.call(this, flag) }
        }
    }

    #[detour(IS_ON_WATER)]
    unsafe extern "fastcall" fn on_water(this: *const u32) -> bool {
        if scripted() {
            let answer = roll(5) == 0;
            record(format!("isOnWater {answer}"));
            answer
        } else {
            unsafe { IS_ON_WATER_DETOUR.call(this) }
        }
    }

    #[detour(GET_TILE)]
    unsafe extern "thiscall" fn tile(this: *const u32) -> i32 {
        let mine = {
            let script = SCRIPT.lock().unwrap();
            script.on && script.fixtures.contains(&(this as u32))
        };
        if mine {
            fake_tile() as i32
        } else {
            unsafe { GET_TILE_DETOUR.call(this) }
        }
    }

    #[detour(GO_TO_DEST_0)]
    unsafe extern "thiscall" fn go_to_dest(this: *const u32, name: *const u32, arg: u32, flag: i8) -> u32 {
        if scripted() {
            let begin: u32 = get_from_memory(name as u32);
            let end: u32 = get_from_memory(name as u32 + 4);
            let text: String = (begin..end).map(|address| get_from_memory::<u8>(address) as char).collect();
            record(format!("goToDest {text:?} {arg} {flag}"));
            1
        } else {
            unsafe { GO_TO_DEST_0_DETOUR.call(this, name, arg, flag) }
        }
    }

    #[detour(PREYS_ON_MAN)]
    unsafe extern "thiscall" fn preys(this: *const u32) -> bool {
        if scripted() {
            let answer = roll(2) == 0;
            record(format!("preysOnMan {answer}"));
            answer
        } else {
            unsafe { PREYS_ON_MAN_DETOUR.call(this) }
        }
    }
}

/// The movement port's override slots: while a check runs scripted, `findPlaceToSit`, `findTrashCan` and
/// `triggerGrandstandEscape` answer from the script instead of searching the zoo.
fn scripted_find_place(_this: u32, arg: u32) -> Option<bool> {
    if !scripted() || SEARCH_UNDER_TEST.load(Ordering::SeqCst) {
        return None;
    }
    let found = roll(2) == 0;
    record(format!("findPlaceToSit {arg} {found}"));
    Some(found)
}

fn scripted_find_trash(_this: u32, arg: u32) -> Option<bool> {
    if !scripted() || SEARCH_UNDER_TEST.load(Ordering::SeqCst) {
        return None;
    }
    let found = roll(2) == 0;
    record(format!("findTrashCan {arg} {found}"));
    Some(found)
}

fn scripted_escape(_this: u32, arg: i32) -> bool {
    if !scripted() || SEARCH_UNDER_TEST.load(Ordering::SeqCst) {
        return false;
    }
    record(format!("triggerGrandstandEscape {arg:#x}"));
    true
}

fn install_hooks() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        *live_support::movement_overrides::FIND_PLACE.lock().unwrap() = Some(scripted_find_place);
        *live_support::movement_overrides::FIND_TRASH.lock().unwrap() = Some(scripted_find_trash);
        *live_support::movement_overrides::ESCAPE.lock().unwrap() = Some(scripted_escape);
        if let Err(e) = unsafe { check_hooks::init_detours() } {
            result = Err(format!("could not install the check hooks: {e:?}"));
        }
    });
    result
}

/// `ZTGuestType` bytes cloned for a fixture: every field the checks read lies below this.
const TYPE_COPY_BYTES: usize = 0x2d0;
/// The range of `ZTGuestType` integers the clone randomises: periods, level ranges, rates, thresholds and chances.
const TYPE_RANDOM_RANGE: (u32, u32) = (0x1b4, 0x2b4);

fn cloned_type(guest_type: u32, rng: &mut Lcg) -> u32 {
    let copy = leaked(TYPE_COPY_BYTES + 0x30);
    for offset in 0..TYPE_COPY_BYTES as u32 {
        save_to_memory(copy + offset, get_from_memory::<u8>(guest_type + offset));
    }
    let mut offset = TYPE_RANDOM_RANGE.0;
    while offset < TYPE_RANDOM_RANGE.1 {
        let value = match rng.next() % 5 {
            0 => 0,
            1 => 100,
            _ => (rng.next() % 130) as i32 - 20,
        };
        save_to_memory(copy + offset, value);
        offset += 4;
    }
    copy
}

extern "thiscall" fn fake_predicate_false(_this: u32) -> u32 {
    0
}

/// A copy of the guest's real vtable with the slots the checks dispatch replaced by scripted fakes.
fn install_fake_vtable(guest: ZTGuest) {
    let real_vtable: u32 = get_from_memory(guest.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    let address = |function: usize| function as u32;
    vtable[0x268 / 4] = address(slot_busy as *const () as usize);
    vtable[0x24c / 4] = address(slot_target as *const () as usize);
    vtable[0x2fc / 4] = address(slot_predator as *const () as usize);
    vtable[0x2e4 / 4] = address(slot_go_to_tile as *const () as usize);
    vtable[0x2e8 / 4] = address(slot_trash_target as *const () as usize);
    vtable[0x10c / 4] = address(slot_init_status_vars as *const () as usize);
    vtable[0x264 / 4] = address(fake_predicate_false as *const () as usize);
    vtable[0x174 / 4] = address(slot_clear_goal as *const () as usize);
    vtable[0x288 / 4] = address(slot_is_doing_something as *const () as usize);
    vtable[0x248 / 4] = address(slot_habitat_from_tile as *const () as usize);
    vtable[0x2d8 / 4] = address(slot_set_target_tile as *const () as usize);
    vtable[0x198 / 4] = address(slot_stop_everything as *const () as usize);
    vtable[0x1d8 / 4] = address(slot_goal_set as *const () as usize);
    vtable[0x294 / 4] = address(slot_set_return_id as *const () as usize);
    vtable[0x29c / 4] = address(slot_set_return_string as *const () as usize);
    vtable[0x28c / 4] = address(slot_has_pressing_need as *const () as usize);
    vtable[0x290 / 4] = address(slot_goto_nearby_need as *const () as usize);
    vtable[0x280 / 4] = address(slot_find_nearby as *const () as usize);
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(guest.0, vtable.as_ptr() as u32);
}

/// Plants pseudo-random state the checks read.
fn plant_state(guest: ZTGuest, guest_type: u32, rng: &mut Lcg) {
    save_to_memory(guest.0 + unit_offsets::ENTITY_TYPE, guest_type);
    let goals = [0x66u32, 0x67, 0x68, 0x69, 0x6a, 0x71, 0x75, 0x7f, 1, 0];
    save_to_memory(guest.0 + unit_offsets::GOAL_ID, goals[rng.next() as usize % goals.len()]);
    save_to_memory(guest.0 + guest_offsets::HAPPINESS, (rng.next() % 260) as i32 - 130);
    for offset in [guest_offsets::HUNGER, guest_offsets::THIRST, guest_offsets::ENERGY, guest_offsets::BATHROOM] {
        save_to_memory(guest.0 + offset, (rng.next() % 160) as i32 - 30);
    }
    for offset in [0x2acu32, 0x2b4, 0x2bc, 0x2c4, 0x2cc] {
        save_to_memory(guest.0 + offset, (rng.next() % 40) as i32 - 10);
    }
    for offset in [0x2d4u32, 0x2d8, 0x2dc, 0x2e0] {
        save_to_memory(guest.0 + offset, (rng.next() % 12) as i32 - 3);
    }
    for slot in (0x2e8u32..=0x310).step_by(4) {
        let value = match rng.next() % 4 {
            0 | 1 => 0,
            2 => 1,
            _ => (rng.next() % 50) as i32,
        };
        save_to_memory(guest.0 + slot, value);
    }
    save_to_memory(guest.0 + 0x30c, 7i32);
    let period = [0i32, 1, 5, -2];
    save_to_memory(guest.0 + guest_offsets::CHECK_COUNTDOWN, period[rng.next() as usize % period.len()]);
    save_to_memory(guest.0 + 0x318, period[rng.next() as usize % period.len()]);
    for offset in [0x336u32, 0x337, 0x339, 0x33a, 0x33c, 0x33d, 0x33e, 0x33f, 0x24c, 0x20c, 0x25c] {
        save_to_memory(guest.0 + offset, (rng.next() % 3 == 0) as u8);
    }
    let name_rvas = [0x23c4c8u32, 0x23c4d8, 0x23c4e8, 0x23c4f8, 0x23c508, 0x23c518, 0x23c528];
    let need_index = match rng.next() % 3 {
        0 => 0xffu8,
        1 => unsafe { GET_NEED_INDEX.hooked()((base() + name_rvas[rng.next() as usize % name_rvas.len()]) as *const i8) as u8 },
        _ => rng.next() as u8,
    };
    save_to_memory(guest.0 + 0x24e, need_index);
    for offset in [0x214u32, 0x218, 0x21c] {
        save_to_memory(guest.0 + offset, (rng.next() % 900) as i32 - 100);
    }
    save_to_memory(guest.0 + 0x244, if rng.next() % 3 == 0 { 0 } else { fake_building() });
    save_to_memory(guest.0 + 0x124, 0x500 + rng.next() % 4);
    save_to_memory(guest.0 + 0x2e4, 0x500 + rng.next() % 4);
    let items = leaked(0x30);
    for index in 0..5u32 {
        save_to_memory(items + index * 4, (rng.next() % 3 == 0) as u32);
    }
    save_to_memory(guest.0 + guest_offsets::ITEMS, items);
    save_to_memory(guest.0 + guest_offsets::ITEMS + 4, items + 20);
    save_to_memory(guest.0 + guest_offsets::ITEMS + 8, items + 20);
    install_fake_vtable(guest);
}

/// Plants the per-case values the shared fakes expose.
fn plant_shared(rng: &mut Lcg) {
    save_to_memory(fake_entity() + 0x2c, (rng.next() % 2) as u8);
    save_to_memory(fake_entity() + 0x19c, (rng.next() % 0x30) as i32);
    for offset in [0x114u32, 0x118, 0x11c] {
        save_to_memory(fake_building() + offset, (rng.next() % 1400) as i32 - 700);
    }
    save_to_memory(fake_building() + 0x19c, (rng.next() % 0x30) as i32);
    save_to_memory(fake_tile() + 0x83, rng.next() as u8);
    save_to_memory(fake_habitat() + 0x2c, (rng.next() % 2) as u8);
    let guide = fake_guide();
    save_to_memory(guide + 0x124, 0x500 + rng.next() % 4);
    save_to_memory(guide + 0x170, [0x76u32, 0x77, 0x75, 0x76][rng.next() as usize % 4]);
    let world = crate::globals::globals().ztworldmgr();
    let (x, y) = (rng.next() % world.map_x_size, rng.next() % world.map_y_size);
    save_to_memory(guide + 0x180, if rng.next() % 6 == 0 { 0 } else { world.get_tile_ptr(x, y) });
    let (list, animals) = fake_animals();
    for animal in &animals {
        save_to_memory(animal + 0x2a8, (rng.next() % 0x40) as i32 + 0x40);
    }
    let array: u32 = get_from_memory(list);
    save_to_memory(list + 4, array + (rng.next() as usize % (FAKE_ANIMALS + 1)) as u32 * 4);
}

#[derive(PartialEq, Debug)]
struct Outcome<R> {
    result: R,
    rng: u32,
    log: Vec<String>,
}

/// Runs `body` on a fresh fixture with the script and RNG reset.
fn run_pole<R>(guest: ZTGuest, script_seed: u32, rng_seed: u32, body: impl FnOnce(u32) -> R) -> Outcome<R> {
    {
        let mut script = SCRIPT.lock().unwrap();
        script.on = true;
        script.state = script_seed;
        script.log.clear();
        script.fixtures = vec![guest.0, fake_building()];
    }
    pin_rng(rng_seed);
    let result = body(guest.0);
    let log = {
        let mut script = SCRIPT.lock().unwrap();
        script.on = false;
        std::mem::take(&mut script.log)
    };
    Outcome { result, rng: rng(), log }
}

const ROUNDS: u32 = 400;

/// Compares the real body, the port and the hooked address over `ROUNDS` planted cases. `real`, `port` and `hooked` get
/// the fixture address and return a value to compare. Returns how many cases logged at least one call and how many
/// logged a given marker, so the caller can see the branches were reached.
fn compare_check<R: PartialEq + Debug>(
    name: &str,
    guest_type: u32,
    failures: &mut Vec<String>,
    real_body: impl Fn(u32) -> R,
    port_body: impl Fn(u32) -> R,
    hooked_body: impl Fn(u32) -> R,
) -> Vec<Vec<String>> {
    let mut logs = Vec::new();
    for round in 0..ROUNDS {
        let seed = 0x1000 + round * 77;
        let label = format!("{name} round {round}");
        let mut guests = Vec::new();
        let mut outcomes = Vec::new();
        for pole in 0..3 {
            let mut rng = Lcg(seed);
            let type_clone = cloned_type(guest_type, &mut rng);
            let guest = live_support::build_standalone_guest(guest_type);
            plant_state(guest, type_clone, &mut rng);
            plant_shared(&mut rng);
            let outcome = match pole {
                0 => run_pole(guest, seed, seed ^ 0x5bd1, &real_body),
                1 => run_pole(guest, seed, seed ^ 0x5bd1, &port_body),
                _ => run_pole(guest, seed, seed ^ 0x5bd1, &hooked_body),
            };
            guests.push(guest);
            outcomes.push(outcome);
        }
        if outcomes[0] != outcomes[1] {
            failures.push(format!("{label}: real {:?}, port {:?}", outcomes[0], outcomes[1]));
        }
        if outcomes[0] != outcomes[2] {
            failures.push(format!("{label}: real {:?}, hooked {:?}", outcomes[0], outcomes[2]));
        }
        compare_guests(&label, guests[0], guests[1], &[], failures);
        compare_guests(&format!("{label} hooked"), guests[0], guests[2], &[], failures);
        logs.push(std::mem::take(&mut outcomes[0].log));
        if failures.len() > 8 {
            break;
        }
    }
    logs
}

fn count_logs(logs: &[Vec<String>], marker: &str) -> usize {
    logs.iter().filter(|log| log.iter().any(|entry| entry.starts_with(marker))).count()
}

/// `ZTGUEST_CHECKS_LIVE`: every check, `doConditionals` and `updateStatusVariables` against the real bodies, plus a
/// reach check that the interesting branches (thoughts, searches, escapes, walking) were taken.
pub(crate) fn run_ztguest_checks_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_CHECKS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_hooks() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let saved_rng = rng();
    let this = |address: u32| address as *const u32;
    macro_rules! check {
        ($name:expr, $real:path, $port:ident, $hooked:ident) => {
            compare_check($name, guest_type, &mut failures, |g| $real(g), |g| ZTGuest(g).$port(), |g| unsafe { $hooked.hooked()(this(g)) })
        };
    }
    let bathroom = check!("doBathroomCheck", real::do_bathroom_check, do_bathroom_check, DO_BATHROOM_CHECK);
    let hunger = check!("doHungerCheck", real::do_hunger_check, do_hunger_check, DO_HUNGER_CHECK);
    let thirsty = check!("doThirstyCheck", real::do_thirsty_check, do_thirsty_check, DO_THIRSTY_CHECK);
    let energy = check!("doEnergyCheck", real::do_energy_check, do_energy_check, DO_ENERGY_CHECK);
    let trash = check!("doTrashCheck", real::do_trash_check, do_trash_check, DO_TRASH_CHECK);
    let souvenir = check!("doBuySouvenirCheck", real::do_buy_souvenir_check, do_buy_souvenir_check, DO_BUY_SOUVENIR_CHECK);
    let leave_zoo = check!("doLeaveZooCheck", real::do_leave_zoo_check, do_leave_zoo_check, DO_LEAVE_ZOO_CHECK);
    let chase = check!("doChaseCheck", real::do_chase_check, do_chase_check, DO_CHASE_CHECK);
    let environment = check!("doEnvironmentEffectCheck", real::do_environment_effect_check, do_environment_effect_check, DO_ENVIRONMENT_EFFECT_CHECK);
    // The reset countdown only runs while the AI manager's reset interval (`+0x13c`) is non-zero.
    let aimgr = crate::globals::globals().ztaimgr_ptr() as u32;
    let saved_interval: u32 = get_from_memory(aimgr + 0x13c);
    save_to_memory(aimgr + 0x13c, 5u32);
    let conditionals = check!("doConditionals", real::do_conditionals, do_conditionals, DO_CONDITIONALS);
    save_to_memory(aimgr + 0x13c, saved_interval);
    check!("updateStatusVariables", real::update_status_variables, update_status_variables, UPDATE_STATUS_VARIABLES);
    NON_NULL_TARGET.store(true, Ordering::SeqCst);
    let leave_ok = compare_check("leaveOK", guest_type, &mut failures, real::leave_ok, |g| ZTGuest(g).leave_ok(), |g| unsafe { LEAVE_OK.hooked()(this(g)) });
    NON_NULL_TARGET.store(false, Ordering::SeqCst);
    let leave_zoo_roll = compare_check("checkLeaveZoo", guest_type, &mut failures, real::check_leave_zoo, |g| ZTGuest(g).check_leave_zoo(), |g| unsafe {
        CHECK_LEAVE_ZOO.hooked()(this(g))
    });
    for unit_kind in 0..3u32 {
        let unit = match unit_kind {
            0 => 0,
            _ => {
                let unit = leaked(0x400);
                let vtable = leaked(0x100);
                let cast = if unit_kind == 1 { fake_cast_true as *const () as usize as u32 } else { fake_cast_false as *const () as usize as u32 };
                save_to_memory(vtable + 0x1c, cast);
                let ty = leaked(0x40);
                save_to_memory(ty, vtable);
                save_to_memory(unit + 0x128, ty);
                unit
            }
        };
        compare_check(
            &format!("checkPredator kind {unit_kind}"),
            guest_type,
            &mut failures,
            |g| real::check_predator(g, unit),
            |_| ZTGuest::check_predator(unit),
            |g| unsafe { CHECK_PREDATOR.hooked()(this(g), this(unit), false) },
        );
    }
    pin_rng(saved_rng);
    for (name, logs, markers) in [
        ("doBathroomCheck", &bathroom, vec!["thought 0x2764", "findNearestSatisfier", "pickRandomBuildingOfType", "slot goToTile"]),
        ("doHungerCheck", &hunger, vec!["thought 0x275f", "findNearestSatisfier", "slot goToTile"]),
        ("doThirstyCheck", &thirsty, vec!["thought 0x2762", "findNearestSatisfier"]),
        ("doEnergyCheck", &energy, vec!["thought 0x276c", "findPlaceToSit"]),
        ("doTrashCheck", &trash, vec!["thought 0x27fd", "findTrashCan", "slot trashTarget"]),
        ("doBuySouvenirCheck", &souvenir, vec!["thought 0x276a", "findNearestSatisfier"]),
        ("doLeaveZooCheck", &leave_zoo, vec!["thought 0x2767", "thought 0x2768", "thought 0x2769"]),
        ("doChaseCheck", &chase, vec!["triggerGrandstandEscape", "thought 0x2805", "thought 0x2808"]),
        ("doEnvironmentEffectCheck", &environment, vec!["thought 0x2777", "thought 0x2778", "thought 0x2801"]),
        ("doConditionals", &conditionals, vec!["slot initStatusVars", "goToDest"]),
    ] {
        for marker in markers {
            if count_logs(logs, marker) == 0 {
                failures.push(format!("{name}: no case ever logged `{marker}`"));
            }
        }
    }
    let _ = (leave_ok, leave_zoo_roll);
    finish_test(test_name, failures, failure_log)
}

extern "thiscall" fn fake_cast_true(_this: u32, _tag: u32) -> u32 {
    1
}

extern "thiscall" fn fake_cast_false(_this: u32, _tag: u32) -> u32 {
    0
}


/// `ZTGUEST_SEARCH_LIVE`: `findPlaceToSit`, `findTrashCan`, `gotoNearbyNeed`, `drop` and `addTrash` against the real
/// bodies in the scripted environment. The seat and can searches run with every argument shape (low byte clear, set,
/// and garbage in the upper bytes); a reach check confirms each branch logged its markers.
pub(crate) fn run_ztguest_search_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::{ADD_TRASH, DROP, FIND_PLACE_TO_SIT, FIND_TRASH_CAN, GOTO_NEARBY_NEED};

    let test_name = "ZTGUEST_SEARCH_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_hooks() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let saved_rng = rng();
    SEARCH_UNDER_TEST.store(true, Ordering::SeqCst);
    let this = |address: u32| address as *const u32;
    let mut sit_logs = Vec::new();
    let mut trash_logs = Vec::new();
    for arg in [0u32, 1, 0x100, 0x101] {
        sit_logs.extend(compare_check(
            &format!("findPlaceToSit {arg:#x}"),
            guest_type,
            &mut failures,
            |g| movement_real::find_place_to_sit(g, arg),
            |g| ZTGuest(g).find_place_to_sit(arg),
            |g| unsafe { FIND_PLACE_TO_SIT.hooked()(this(g), std::ptr::without_provenance::<u32>(arg as usize)) },
        ));
        // Vanilla's seat-and-can search dereferences a found building without a null check.
        NON_NULL_TARGET.store(true, Ordering::SeqCst);
        trash_logs.extend(compare_check(
            &format!("findTrashCan {arg:#x}"),
            guest_type,
            &mut failures,
            |g| movement_real::find_trash_can(g, arg),
            |g| ZTGuest(g).find_trash_can(arg),
            |g| unsafe { FIND_TRASH_CAN.hooked()(this(g), arg) },
        ));
        NON_NULL_TARGET.store(false, Ordering::SeqCst);
    }
    let goto = compare_check(
        "gotoNearbyNeed",
        guest_type,
        &mut failures,
        movement_real::goto_nearby_need,
        |g| ZTGuest(g).goto_nearby_need(),
        |g| unsafe { GOTO_NEARBY_NEED.hooked()(this(g)) },
    );
    let dropped = compare_check("drop", guest_type, &mut failures, movement_real::drop_guest, |g| ZTGuest(g).drop_guest(), |g| unsafe { DROP.hooked()(this(g)) });
    compare_check("addTrash", guest_type, &mut failures, movement_real::add_trash, |g| ZTGuest(g).add_trash(), |g| unsafe { ADD_TRASH.hooked()(this(g)) });
    SEARCH_UNDER_TEST.store(false, Ordering::SeqCst);
    pin_rng(saved_rng);
    for (name, logs, markers) in [
        (
            "findPlaceToSit",
            &sit_logs,
            vec!["findNearestSatisfier", "findAnySatisfier", "updateBuildingTiles true", "updateBuildingTiles false", "slot clearGoal", "slot goToTile", "thought 0x276d"],
        ),
        ("findTrashCan", &trash_logs, vec!["findNearestSatisfier", "findAnySatisfier", "throwTrash", "updateBuildingTiles", "slot clearGoal", "slot goToTile"]),
        ("gotoNearbyNeed", &goto, vec!["slot findNearbyNeedSatisfier", "slot goToTile"]),
        ("drop", &dropped, vec!["removeFromMemberLists", "slot setReturnBuildingString", "slot goalSet", "slot hasPressingNeed", "slot gotoNearbyNeed", "thought 0x2948"]),
    ] {
        for marker in markers {
            if count_logs(logs, marker) == 0 {
                failures.push(format!("{name}: no case ever logged `{marker}`"));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_VIEWING_LIVE`: `followGuide`, `viewNearbyExhibit`, `doBuilding`, `setVisible` and `runScreaming` against the
/// real bodies in the scripted environment (scripted `rand` for `viewNearbyExhibit`), with a reach check that each
/// branch logged its markers.
pub(crate) fn run_ztguest_viewing_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::ffi::c_void;

    use openzt_detour::generated::ztguest::{DO_BUILDING, FOLLOW_GUIDE, RUN_SCREAMING, SET_VISIBLE, VIEW_NEARBY_EXHIBIT};

    use super::ztguide_habitats::{install_rand_hook, rand_calls, script_rand, set_rand_intercept};

    let test_name = "ZTGUEST_VIEWING_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_hooks().and_then(|()| install_rand_hook()) {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let saved_rng = rng();
    let this = |address: u32| address as *const u32;
    let guide = fake_guide();
    // Vanilla's follow search dereferences the habitat a neighbouring tile reports without a null check.
    NON_NULL_TARGET.store(true, Ordering::SeqCst);
    let follow = compare_check(
        "followGuide",
        guest_type,
        &mut failures,
        |g| viewing_real::follow_guide(g, guide),
        |g| ZTGuest(g).follow_guide(guide),
        |g| unsafe { FOLLOW_GUIDE.hooked()(this(g), guide as *const c_void) },
    );
    NON_NULL_TARGET.store(false, Ordering::SeqCst);
    let scripted_rand = |g: u32| {
        script_rand(&[get_from_memory::<u32>(g + 0x2a8)]);
        set_rand_intercept(true);
    };
    let nearby = compare_check(
        "viewNearbyExhibit",
        guest_type,
        &mut failures,
        |g| {
            scripted_rand(g);
            viewing_real::view_nearby_exhibit(g);
            set_rand_intercept(false);
            rand_calls()
        },
        |g| {
            scripted_rand(g);
            ZTGuest(g).view_nearby_exhibit();
            set_rand_intercept(false);
            rand_calls()
        },
        |g| {
            scripted_rand(g);
            unsafe { VIEW_NEARBY_EXHIBIT.hooked()(this(g)) };
            set_rand_intercept(false);
            rand_calls()
        },
    );
    let building = compare_check("doBuilding", guest_type, &mut failures, viewing_real::do_building, |g| ZTGuest(g).do_building(), |g| unsafe { DO_BUILDING.hooked()(this(g)) });
    let mut visible = Vec::new();
    for flag in [false, true] {
        visible.extend(compare_check(
            &format!("setVisible {flag}"),
            guest_type,
            &mut failures,
            |g| viewing_real::set_visible(g, flag),
            |g| ZTGuest(g).set_visible(flag),
            |g| unsafe { SET_VISIBLE.hooked()(this(g), flag) },
        ));
    }
    let screaming = compare_check("runScreaming", guest_type, &mut failures, viewing_real::run_screaming, |g| ZTGuest(g).run_screaming(), |g| unsafe {
        RUN_SCREAMING.hooked()(this(g))
    });
    pin_rng(saved_rng);
    for (name, logs, markers) in [
        (
            "followGuide",
            &follow,
            vec!["slot isDoingSomething", "slot hasPressingNeed", "getPath", "slot habitatFromTile", "slot goalSet 0x76 3", "goToDest", "slot setTargetTile", "addGuest"],
        ),
        ("viewNearbyExhibit", &nearby, vec!["habitatFacing true", "getAllAnimals", "thought 0x2772"]),
        ("doBuilding", &building, vec!["isFunSatisfier", "ZTUnit::doBuilding"]),
        ("setVisible", &visible, vec!["setGuestVisible 0", "setGuestVisible 1"]),
        ("runScreaming", &screaming, vec!["isOnWater true", "isOnWater false"]),
    ] {
        for marker in markers {
            if count_logs(logs, marker) == 0 {
                failures.push(format!("{name}: no case ever logged `{marker}`"));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
