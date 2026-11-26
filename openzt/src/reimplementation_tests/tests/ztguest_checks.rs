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
    bfunit::{GET_PATH_1 as GET_PATH, GO_TO_DEST_0, GO_TO_DEST_1 as GO_TO_DEST},
    standalone::HABITAT_FACING,
    ztanimal::PREYS_ON_MAN,
    ztguest::{
        CHECK_LEAVE_ZOO, CHECK_PREDATOR, DO_BATHROOM_CHECK, DO_BUY_SOUVENIR_CHECK, DO_CHASE_CHECK, DO_CONDITIONALS, DO_ENERGY_CHECK, DO_ENVIRONMENT_EFFECT_CHECK,
        DO_HUNGER_CHECK, DO_LEAVE_ZOO_CHECK, DO_THIRSTY_CHECK, DO_TRASH_CHECK, LEAVE_OK,
        PICK_RANDOM_BUILDING_OF_TYPE, THROW_TRASH, UPDATE_STATUS_VARIABLES,
    },
    zthabitat::GET_ALL_ANIMALS,
    ztui_guestinfo::SET_GUEST_VISIBLE,
    ztunit::{
        DO_BUILDING as ZTUNIT_DO_BUILDING, FIND_ANY_SATISFIER, FIND_NEAREST_SATISFIER, IS_FUN_SATISFIER, IS_NEED_OR_FUN_SATISFIER, IS_ON_WATER,
        UPDATE_BUILDING_TILES, FIND_PREDATOR,
    },
    ztworldmgr::{GET_NEED_INDEX, REMOVE_FROM_MEMBER_LISTS},
    },
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

/// Set while `fGuestThought` itself runs, so the thought override stands aside and the real thought manager is reached.
static THOUGHT_UNDER_TEST: AtomicBool = AtomicBool::new(false);

/// Set by the cloned-guest tick test: the world-mutating collaborators (`throwTrash`, `removeFromMemberLists`) log
/// into [`SANDBOX_LOG`] and do nothing, and everything else runs for real.
/// While set, `BFUnit::getPath` and `goToDest` succeed without touching the (fake) guest, so `followGuide` runs to the end.
pub(super) static FOLLOW_SANDBOX: AtomicBool = AtomicBool::new(false);
pub(super) static SANDBOX: AtomicBool = AtomicBool::new(false);
pub(super) static SANDBOX_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

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
    unsafe extern "thiscall" fn update_tiles(this: *const u32, flag: bool) -> bool {
        if scripted() {
            let answer = roll(3) != 0;
            record(format!("updateBuildingTiles {flag} {answer}"));
            answer
        } else {
            unsafe { UPDATE_BUILDING_TILES_DETOUR.call(this, flag) }
        }
    }

    /// `ZTUnit::findPredator` reads the unit's tile, which a clone that is not on the map does not have.
    #[detour(FIND_PREDATOR)]
    unsafe extern "thiscall" fn find_predator(this: *const u32, flag: bool) -> i32 {
        if SANDBOX.load(Ordering::SeqCst) {
            SANDBOX_LOG.lock().unwrap().push(format!("findPredator {flag}"));
            0
        } else {
            unsafe { FIND_PREDATOR_DETOUR.call(this, flag) }
        }
    }

    #[detour(THROW_TRASH)]
    unsafe extern "thiscall" fn throw_trash(this: *const u32, arg: *const std::ffi::c_void) -> u32 {
        if scripted() {
            record(format!("throwTrash {}", arg as u32));
            1
        } else if SANDBOX.load(Ordering::SeqCst) {
            SANDBOX_LOG.lock().unwrap().push(format!("throwTrash {}", arg as u32));
            1
        } else {
            unsafe { THROW_TRASH_DETOUR.call(this, arg) }
        }
    }

    #[detour(REMOVE_FROM_MEMBER_LISTS)]
    unsafe extern "thiscall" fn remove_from_member_lists(this: *const u32, guest: i32) {
        if scripted() {
            record("removeFromMemberLists".to_string());
        } else if SANDBOX.load(Ordering::SeqCst) {
            SANDBOX_LOG.lock().unwrap().push("removeFromMemberLists".to_string());
        } else {
            unsafe { REMOVE_FROM_MEMBER_LISTS_DETOUR.call(this, guest) }
        }
    }

    #[detour(GET_PATH)]
    unsafe extern "thiscall" fn get_path(this: *const u32, tile: *const u32, flag: bool) -> bool {
        if scripted() {
            let answer = roll(3) == 0;
            record(format!("getPath {} {flag} {answer}", tile as u32 != 0));
            answer
        } else if FOLLOW_SANDBOX.load(Ordering::SeqCst) {
            true
        } else {
            unsafe { GET_PATH_DETOUR.call(this, tile, flag) }
        }
    }

    #[detour(GO_TO_DEST)]
    unsafe extern "thiscall" fn go_to_dest_none(this: *const u32, name: bool, arg: u32) -> bool {
        if scripted() {
            let answer = roll(3) != 0;
            record(format!("goToDest {} {arg} {answer}", name as u32));
            answer
        } else if FOLLOW_SANDBOX.load(Ordering::SeqCst) {
            true
        } else {
            unsafe { GO_TO_DEST_DETOUR.call(this, name, arg) }
        }
    }

    #[detour(HABITAT_FACING)]
    unsafe extern "cdecl" fn habitat_in_front(guest: *const u32) -> *const std::ffi::c_void {
        if scripted() {
            let answer = if roll(4) == 0 { 0 } else { fake_habitat() };
            record(format!("habitatFacing {}", answer != 0));
            answer as *const std::ffi::c_void
        } else {
            unsafe { HABITAT_FACING_DETOUR.call(guest) }
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
        if let Some(tile) = scenery_tile(this as u32) {
            return tile as i32;
        }
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
fn scripted_add_guest(_guide: u32, id: i32) -> bool {
    if !scripted() {
        return false;
    }
    record(format!("addGuest {:#x}", id as u32));
    true
}

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

fn scripted_thought(_this: u32, id: u32, subject: u32, habitat: u32) -> bool {
    if !scripted() || THOUGHT_UNDER_TEST.load(Ordering::SeqCst) {
        return false;
    }
    record(format!("thought {id:#x} {} {}", subject != 0, habitat != 0));
    true
}

fn scripted_in_grandstand(_this: u32) -> Option<bool> {
    if !scripted() || SEARCH_UNDER_TEST.load(Ordering::SeqCst) {
        return None;
    }
    let answer = roll(3) == 0;
    record(format!("inGrandstand {answer}"));
    Some(answer)
}

fn scripted_escape(_this: u32, arg: i32) -> bool {
    if !scripted() || SEARCH_UNDER_TEST.load(Ordering::SeqCst) {
        return false;
    }
    record(format!("triggerGrandstandEscape {arg:#x}"));
    true
}

pub(super) fn install_hooks() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        *live_support::viewing_overrides::ADD_GUEST.lock().unwrap() = Some(scripted_add_guest);
        *live_support::movement_overrides::FIND_PLACE.lock().unwrap() = Some(scripted_find_place);
        *live_support::movement_overrides::FIND_TRASH.lock().unwrap() = Some(scripted_find_trash);
        *live_support::movement_overrides::ESCAPE.lock().unwrap() = Some(scripted_escape);
        *live_support::movement_overrides::IN_GRANDSTAND.lock().unwrap() = Some(scripted_in_grandstand);
        *live_support::goals_overrides::THOUGHT.lock().unwrap() = Some(scripted_thought);
        *live_support::check_overrides::FIND_VIEWING_AREA.lock().unwrap() = Some(scripted_find_viewing_area);
        *live_support::sites_overrides::NUM_SITES.lock().unwrap() = Some(scripted_num_sites);
        *live_support::check_overrides::ANIMAL_HAPPY.lock().unwrap() = Some(scripted_animal_happy);
        *live_support::check_overrides::NUM_ANIMALS.lock().unwrap() = Some(scripted_num_animals);
        *live_support::check_overrides::SICK_ANIMALS.lock().unwrap() = Some(scripted_sick_animals);
        *live_support::check_overrides::NUM_SICK_ANIMALS.lock().unwrap() = Some(scripted_num_sick_animals);
        *live_support::check_overrides::DIFFERENT_SPECIES.lock().unwrap() = Some(scripted_different_species);
        *live_support::check_overrides::FIND_ANIMAL.lock().unwrap() = Some(scripted_find_animal);
        *live_support::check_overrides::CROWD_DENSITY.lock().unwrap() = Some(scripted_crowd_density);
        if let Err(e) = unsafe { check_hooks::init_detours() } {
            result = Err(format!("could not install the check hooks: {e:?}"));
        }
        if let Err(e) = unsafe { goal_hooks::init_detours() } {
            result = Err(format!("could not install the goal hooks: {e:?}"));
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
    vtable[0x184 / 4] = address(slot_release_predicate as *const () as usize);
    vtable[0x8 / 4] = address(slot_events as *const () as usize);
    vtable[0x1fc / 4] = address(slot_mood_change as *const () as usize);
    vtable[0x30 / 4] = address(slot_after_mood as *const () as usize);
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
    if GOALS_MODE.load(Ordering::SeqCst) {
        plant_goals(guest, guest_type, rng);
    }
    if LISTEN_MODE.load(Ordering::SeqCst) {
        plant_listen(guest, rng);
    }
    if SITES_MODE.load(Ordering::SeqCst) {
        plant_sites(guest, rng);
    }
    if ITEMS_MODE.load(Ordering::SeqCst) {
        plant_items(guest, rng);
    }
    if VIEWING_CHECK_MODE.load(Ordering::SeqCst) {
        let countdown = if rng.next() % 3 == 0 { 0i32 } else { 1 + (rng.next() % 3) as i32 };
        save_to_memory(guest.0 + 0x30c, countdown);
        save_to_memory(guest.0 + 0x2ac, (rng.next() % 40) as i32 - 10);
    }
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
    if GOALS_MODE.load(Ordering::SeqCst) {
        for offset in [0x114u32, 0x118, 0x11c] {
            save_to_memory(fake_show() + offset, (rng.next() % 1400) as i32 - 700);
        }
        save_to_memory(fake_tile() + 0x85, rng.next() as u8);
    }
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
        script.fixtures = vec![guest.0, fake_building(), fake_show()];
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

/// Offset added to every case seed, so a check can be repeated over fresh cases.
static ROUND_BASE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

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
        let seed = 0x1000 + round * 77 + ROUND_BASE.load(Ordering::SeqCst);
        let label = format!("{name} round {round}");
        let mut guests = Vec::new();
        let mut outcomes = Vec::new();
        for pole in 0..3 {
            let mut rng = Lcg(seed);
            let type_clone = cloned_type(guest_type, &mut rng);
            let guest = live_support::build_standalone_guest(guest_type);
            live_support::forget_standalone_guest(guest);
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


/// Set while the goal-ladder cases plant their extra state (goal ids, cog-goal flags, tile flags, source bits).
static GOALS_MODE: AtomicBool = AtomicBool::new(false);

fn fake_show() -> u32 {
    static SHOW: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *SHOW.get_or_init(|| leaked(0x400))
}

extern "thiscall" fn slot_release_predicate(_this: u32) -> u32 {
    let answer = roll(2) == 0;
    record(format!("slot releasePredicate {answer}"));
    answer as u32
}

extern "thiscall" fn cog_valid(_this: u32) -> u32 {
    let answer = roll(3) != 0;
    record(format!("cog valid {answer}"));
    answer as u32
}

extern "thiscall" fn cog_has_tile(_this: u32) -> u32 {
    let answer = roll(2) == 0;
    record(format!("cog hasTile {answer}"));
    answer as u32
}

fn fake_cog_vtable() -> u32 {
    static VTABLE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *VTABLE.get_or_init(|| {
        let mut vtable = vec![0u32; 0x20];
        vtable[0] = cog_valid as *const () as usize as u32;
        vtable[2] = cog_has_tile as *const () as usize as u32;
        Box::leak(vtable.into_boxed_slice()).as_ptr() as u32
    })
}

/// A `list<uint>` head whose first node is itself (empty) or one leaked node.
fn fake_list_head(non_empty: bool) -> u32 {
    let head = leaked(12);
    if non_empty {
        let node = leaked(12);
        save_to_memory(head, node);
        save_to_memory(node, head);
    } else {
        save_to_memory(head, head);
    }
    head
}

const LADDER_GOALS: [u32; 17] = [0x64, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x71, 0x75, 0x79, 0x7f, 0x84, 0, 1, 0x69];

fn plant_goals(guest: ZTGuest, guest_type: u32, rng: &mut Lcg) {
    live_support::forget_standalone_guest(guest);
    save_to_memory(guest.0 + unit_offsets::GOAL_ID, LADDER_GOALS[rng.next() as usize % LADDER_GOALS.len()]);
    for offset in [0x1a8u32, 0x20e, 0x336, 0x339, 0x33e, 0x33f, 0x24c, 0x25c] {
        save_to_memory(guest.0 + offset, (rng.next() % 6 == 0) as u8);
    }
    save_to_memory(guest.0 + 0x22c, if rng.next() % 2 == 0 { 0u32 } else { 1 + rng.next() % 5 });
    save_to_memory(guest.0 + 0x154, fake_list_head(rng.next() % 2 == 0));
    save_to_memory(guest.0 + 0x15c, fake_cog_vtable());
    save_to_memory(guest_type + 0x2b4, (rng.next() % 101) as i32);
    save_to_memory(guest.0 + 0x340, rng.next() as u8);
    for offset in [0x142u32, 0x143] {
        save_to_memory(guest.0 + offset, (rng.next() % 8 == 0) as u8);
    }
}

#[openzt_detour_macro::detour_mod]
mod goal_hooks {
    use std::ffi::c_void;

    use openzt_detour::generated::{
        bfcoggoal::{SET as BFCOGGOAL_SET, SET_ID as BFCOGGOAL_SET_ID},
        ztthoughtmgr::ADD_THOUGHT,
        ztunit::{IN_FRIENDLY_CHASE, INIT_AFTER_LOAD as ZTUNIT_INIT_AFTER_LOAD, UPDATE_GOALS as ZTUNIT_UPDATE_GOALS},
        ztworldmgr::{FIND_NEAREST_UNSEEN_STARTING_SHOW, GET_ATTRACTIONS as GET_ATTRACTIONS_WORLD, GET_GAWK_SCENERY as GET_GAWK_SCENERY_WORLD, SATISFIES as SATISFIES_WORLD},
    };

    use super::*;

    #[detour(GET_ATTRACTIONS_WORLD)]
    unsafe extern "thiscall" fn world_attractions(this: *const u32, slot: *const i32) {
        if scripted() {
            fill_scenery_list(slot as u32, "getAttractions");
        } else {
            unsafe { GET_ATTRACTIONS_WORLD_DETOUR.call(this, slot) }
        }
    }

    #[detour(GET_GAWK_SCENERY_WORLD)]
    unsafe extern "thiscall" fn world_gawk_scenery(this: *const u32, slot: *const i32) {
        if scripted() {
            fill_scenery_list(slot as u32, "getGawkScenery");
        } else {
            unsafe { GET_GAWK_SCENERY_WORLD_DETOUR.call(this, slot) }
        }
    }

    #[detour(SATISFIES_WORLD)]
    unsafe extern "cdecl" fn world_satisfies(building: i32, name: *const i8, other: *const u32) -> u32 {
        if scripted() {
            let answer = roll(2) == 0;
            record(format!("satisfies {} {answer}", building as u32 == fake_item_building()));
            answer as u32
        } else {
            unsafe { SATISFIES_WORLD_DETOUR.call(building, name, other) }
        }
    }

    #[detour(ZTUNIT_UPDATE_GOALS)]
    unsafe extern "thiscall" fn unit_update_goals(this: *const u32) {
        if scripted() {
            record("ZTUnit updateGoals".to_string());
        } else if SANDBOX.load(Ordering::SeqCst) {
            SANDBOX_LOG.lock().unwrap().push("ZTUnit updateGoals".to_string());
        } else {
            unsafe { ZTUNIT_UPDATE_GOALS_DETOUR.call(this) }
        }
    }

    #[detour(FIND_NEAREST_UNSEEN_STARTING_SHOW)]
    unsafe extern "stdcall" fn nearest_show(guest: *const c_void) -> u32 {
        if scripted() {
            let show = if roll(2) == 0 { 0 } else { fake_show() };
            record(format!("findNearestUnseenStartingShow {}", show != 0));
            show
        } else {
            unsafe { FIND_NEAREST_UNSEEN_STARTING_SHOW_DETOUR.call(guest) }
        }
    }

    #[detour(IN_FRIENDLY_CHASE)]
    unsafe extern "thiscall" fn friendly_chase(this: *const u32) -> u32 {
        if scripted() {
            let answer = roll(2) == 0;
            record(format!("inFriendlyChase {answer}"));
            answer as u32
        } else {
            unsafe { IN_FRIENDLY_CHASE_DETOUR.call(this) }
        }
    }

    #[detour(BFCOGGOAL_SET_ID)]
    unsafe extern "thiscall" fn cog_set_id(this: *const u32, id: u32, arg: *const i32) {
        if scripted() {
            record(format!("cog setID {id:#x} {:#x}", arg as u32 - base()));
        } else {
            unsafe { BFCOGGOAL_SET_ID_DETOUR.call(this, id, arg) }
        }
    }

    #[detour(BFCOGGOAL_SET)]
    unsafe extern "thiscall" fn cog_set(this: *const u32, goal: i32, kind: i32, id: u32, arg: *const i32, flag: i32) {
        if scripted() {
            record(format!("cog set {goal:#x} {kind} {id:#x} {:#x} {flag}", arg as u32 - base()));
        } else {
            unsafe { BFCOGGOAL_SET_DETOUR.call(this, goal, kind, id, arg, flag) }
        }
    }

    #[detour(ADD_THOUGHT)]
    unsafe extern "thiscall" fn add_thought(this: *const u32, id: u32, thinker: *const u32, subject: *const u32, habitat: *const u32) {
        if scripted() {
            let fixture = SCRIPT.lock().unwrap().fixtures.first().copied();
            record(format!("addThought {id:#x} {} {:#x} {:#x}", fixture == Some(thinker as u32), subject as u32, habitat as u32));
        } else {
            unsafe { ADD_THOUGHT_DETOUR.call(this, id, thinker, subject, habitat) }
        }
    }

    #[detour(ZTUNIT_INIT_AFTER_LOAD)]
    unsafe extern "thiscall" fn unit_init_after_load(this: *const u32, version: u32) -> bool {
        if scripted() {
            let answer = roll(2) == 0;
            record(format!("ZTUnit initAfterLoad {version:#x} {answer}"));
            answer
        } else {
            unsafe { ZTUNIT_INIT_AFTER_LOAD_DETOUR.call(this, version) }
        }
    }
}

/// `ZTGUEST_GOALS_LIVE`: `updateGoals` and `initAfterLoad` against the real bodies in the scripted environment, with
/// goal ids, flags, cog-goal answers and collaborator answers planted per case. A reach check confirms every goal
/// the ladder can set was set at least once.
pub(crate) fn run_ztguest_goals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::{INIT_AFTER_LOAD, UPDATE_GOALS};

    let test_name = "ZTGUEST_GOALS_LIVE";
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
    GOALS_MODE.store(true, Ordering::SeqCst);
    NON_NULL_TARGET.store(true, Ordering::SeqCst);
    let mut ladder = Vec::new();
    for base in 0..6u32 {
        ROUND_BASE.store(base * 1_000_003, Ordering::SeqCst);
        ladder.extend(compare_check(
            "updateGoals",
            guest_type,
            &mut failures,
            live_support::goals_real::update_goals,
            |g| ZTGuest(g).update_goals(),
            |g| unsafe { UPDATE_GOALS.hooked()(this(g)) },
        ));
    }
    ROUND_BASE.store(0, Ordering::SeqCst);
    NON_NULL_TARGET.store(false, Ordering::SeqCst);
    let mut after_load = Vec::new();
    for version in [0x40u32, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x60] {
        after_load.extend(compare_check(
            &format!("initAfterLoad {version:#x}"),
            guest_type,
            &mut failures,
            |g| live_support::goals_real::init_after_load(g, version),
            |g| ZTGuest(g).init_after_load(version),
            |g| unsafe { INIT_AFTER_LOAD.hooked()(this(g), version) },
        ));
    }
    GOALS_MODE.store(false, Ordering::SeqCst);
    pin_rng(saved_rng);
    for marker in [
        "slot goalSet 0x7f",
        "slot goalSet 0x71",
        "slot goalSet 0x75",
        "slot goalSet 0x6b",
        "slot goalSet 0x6c",
        "slot goalSet 0x64",
        "slot goalSet 0x66",
        "slot goalSet 0x1 1",
        "cog set 0x79",
        "cog setID",
        "findNearestUnseenStartingShow true",
        "slot releasePredicate",
        "triggerGrandstandEscape",
        "ZTUnit updateGoals",
    ] {
        if count_logs(&ladder, marker) == 0 {
            failures.push(format!("updateGoals: no case ever logged `{marker}`"));
        }
    }
    if count_logs(&after_load, "slot clearGoal") == 0 {
        failures.push("initAfterLoad: no case ever cleared the goal".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_GUEST_THOUGHT_LIVE`: `fGuestThought` with no subject and with scripted subjects (an animal-typed one with
/// the flag byte `+0x395` clear and set, and a non-animal one) hands the thought manager the same thought through the
/// real body, the port and the hooked address. The manager's `addThought` is scripted; the battery installs no
/// thought-manager detours, so the address is vanilla's.
pub(crate) fn run_ztguest_guest_thought_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::F_GUEST_THOUGHT;

    let test_name = "ZTGUEST_GUEST_THOUGHT_LIVE";
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
    let subject = |cast: extern "thiscall" fn(u32, u32) -> u32, flag: u8| {
        let unit = leaked(0x400);
        let vtable = leaked(0x100);
        save_to_memory(vtable + 0x1c, cast as usize as u32);
        let ty = leaked(0x40);
        save_to_memory(ty, vtable);
        save_to_memory(unit + 0x128, ty);
        save_to_memory(unit + 0x395, flag);
        unit
    };
    let cases = [
        ("no subject", 0u32, None),
        ("animal flag clear", subject(fake_cast_true, 0), None),
        ("animal flag set", subject(fake_cast_true, 1), Some(0x280au32)),
        ("non-animal flag set", subject(fake_cast_false, 1), None),
    ];
    THOUGHT_UNDER_TEST.store(true, Ordering::SeqCst);
    for (label, subject, replaced) in cases {
        for thought in [0x1234u32, 0x280a] {
            for habitat in [0u32, 0x4444] {
                let mut outcomes = Vec::new();
                for pole in 0..3 {
                    let guest = live_support::build_standalone_guest(guest_type);
                    outcomes.push(run_pole(guest, 1, 1, |g| match pole {
                        0 => live_support::goals_real::f_guest_thought(g, thought, subject, habitat),
                        1 => ZTGuest(g).guest_thought(thought, subject, habitat),
                        _ => unsafe { F_GUEST_THOUGHT.hooked()(g as *const u32, thought, subject as *const i32, habitat as *const i32) },
                    }));
                }
                if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                    failures.push(format!("fGuestThought {label} {thought:#x} habitat {habitat:#x}: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
                }
                let expected = format!("addThought {:#x} true {subject:#x} {habitat:#x}", replaced.unwrap_or(thought));
                if outcomes[0].log != [expected.clone()] {
                    failures.push(format!("fGuestThought {label} {thought:#x}: expected {expected:?}, real logged {:?}", outcomes[0].log));
                }
            }
        }
    }
    THOUGHT_UNDER_TEST.store(false, Ordering::SeqCst);
    pin_rng(saved_rng);
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_UPDATE_LIVE`: `update` against the real body with a scripted tile whose list holds the guest itself, a null
/// entry, a non-guest entity and three guests with scripted type names and mark vectors. The puking, screaming and mark
/// effects land on the neighbours, so each pole's neighbour memory is compared as well as the guest's own.
pub(crate) fn run_ztguest_update_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::UPDATE;

    use super::ztmaint::{install_entity_update_hook, UPDATE_INTERCEPT, UPDATE_LOG};

    let test_name = "ZTGUEST_UPDATE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_hooks().and_then(|()| install_entity_update_hook()) {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let saved_rng = rng();
    let neighbours: Vec<u32> = (0..3)
        .map(|_| {
            let neighbour = live_support::build_standalone_guest(guest_type);
            live_support::forget_standalone_guest(neighbour);
            neighbour.0
        })
        .collect();
    let non_guest = {
        let unit = leaked(0x400);
        let vtable = leaked(0x100);
        save_to_memory(vtable + 0x1c, fake_cast_false as *const () as usize as u32);
        let ty = leaked(0x40);
        save_to_memory(ty, vtable);
        save_to_memory(unit + 0x128, ty);
        unit
    };
    let animation = Box::leak(b"bSick".to_vec().into_boxed_slice()).as_ptr() as u32;
    let names: [u32; 4] = [b'w', b'b', b'g', b'm'].map(|initial| Box::leak(vec![initial, 0u8].into_boxed_slice()).as_ptr() as u32);
    let words = |entity: u32| -> Vec<u32> { (0..0x348u32 / 4).map(|index| get_from_memory::<u32>(entity + index * 4)).collect() };
    GOALS_MODE.store(true, Ordering::SeqCst);
    UPDATE_INTERCEPT.store(1, Ordering::SeqCst);
    let mut touched_neighbours = 0;
    for round in 0..ROUNDS {
        let seed = 0x2000 + round * 91;
        let label = format!("update round {round}");
        let mut guests = Vec::new();
        let mut outcomes = Vec::new();
        let mut snapshots: Vec<Vec<Vec<u32>>> = Vec::new();
        let mut touched = false;
        for pole in 0..3 {
            let mut rng = Lcg(seed);
            let type_clone = cloned_type(guest_type, &mut rng);
            let guest = live_support::build_standalone_guest(guest_type);
            live_support::forget_standalone_guest(guest);
            plant_state(guest, type_clone, &mut rng);
            plant_shared(&mut rng);
            let mut neighbour_rng = Lcg(seed ^ 0x77);
            for &neighbour in &neighbours {
                let neighbour_type = cloned_type(guest_type, &mut neighbour_rng);
                save_to_memory(neighbour_type + 0x98, names[neighbour_rng.next() as usize % names.len()]);
                save_to_memory(neighbour + 0x128, neighbour_type);
                save_to_memory(neighbour + 0x54, animation);
                save_to_memory(neighbour + 0x58, animation + 5);
                save_to_memory(neighbour + 0x33b, 0u8);
                save_to_memory(neighbour + 0x339, 0u8);
                let marks = leaked(4);
                save_to_memory(neighbour + 0xfc, marks);
                save_to_memory(neighbour + 0x100, marks + (neighbour_rng.next() % 2));
            }
            let members = [guest.0, 0, non_guest, neighbours[0], guest.0, neighbours[1], neighbours[2]];
            let head = leaked(12);
            let mut previous = head;
            for member in members {
                let node = leaked(12);
                save_to_memory(previous, node);
                save_to_memory(node + 8, member);
                previous = node;
            }
            save_to_memory(previous, head);
            save_to_memory(fake_tile(), head);
            let elapsed = 5 + round;
            let body = |g: u32| {
                UPDATE_LOG.lock().unwrap().clear();
                match pole {
                    0 => live_support::goals_real::update(g, elapsed),
                    1 => ZTGuest(g).update(elapsed),
                    _ => unsafe { UPDATE.hooked()(g as *const u32, elapsed) },
                }
                UPDATE_LOG.lock().unwrap().clone()
            };
            outcomes.push(run_pole(guest, seed, seed ^ 0x5bd1, body));
            snapshots.push(neighbours.iter().map(|&neighbour| words(neighbour)).collect());
            if pole == 0 {
                touched = neighbours.iter().any(|&neighbour| {
                    let mark_begin: u32 = get_from_memory(neighbour + 0xfc);
                    get_from_memory::<u8>(neighbour + 0x339) != 0 || get_from_memory::<u8>(neighbour + 0x33b) != 0 || get_from_memory::<u8>(mark_begin) != 0
                });
            }
            guests.push(guest);
        }
        if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
            failures.push(format!("{label}: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
        }
        compare_guests(&label, guests[0], guests[1], &[], &mut failures);
        compare_guests(&format!("{label} hooked"), guests[0], guests[2], &[], &mut failures);
        for (pole, snapshot) in snapshots.iter().enumerate().skip(1) {
            for (index, (real_words, other_words)) in snapshots[0].iter().zip(snapshot).enumerate() {
                for (offset, (r, p)) in real_words.iter().zip(other_words).enumerate() {
                    if r != p && !(*r >= 0x10000 && *p >= 0x10000) {
                        failures.push(format!("{label}: neighbour {index} pole {pole} +{:#x} real {r:#x}, other {p:#x}", offset * 4));
                    }
                }
            }
        }
        if touched {
            touched_neighbours += 1;
        }
        if failures.len() > 8 {
            break;
        }
    }
    UPDATE_INTERCEPT.store(0, Ordering::SeqCst);
    GOALS_MODE.store(false, Ordering::SeqCst);
    save_to_memory(fake_tile(), 0u32);
    pin_rng(saved_rng);
    if touched_neighbours == 0 {
        failures.push("no case ever made a neighbour scream or leave".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// Set while the `listen` cases plant pending messages, a scripted show manager and the current animation.
static LISTEN_MODE: AtomicBool = AtomicBool::new(false);

extern "thiscall" fn slot_events(_this: u32, vector: u32) -> u32 {
    record(format!("slot events {}", vector != 0));
    0
}

extern "thiscall" fn slot_mood_change(_this: u32, record_ptr: u32) -> u32 {
    let words: Vec<u32> = (0..6).map(|i| get_from_memory::<u32>(record_ptr + i * 4)).collect();
    record(format!("slot moodChange {words:x?}"));
    roll(3)
}

extern "thiscall" fn slot_after_mood(_this: u32) -> u32 {
    record("slot afterMood".to_string());
    0
}

fn scripted_set_bs(_this: u32, begin: u32, end: u32, call_flag: i8, force: i8, mode: i32) -> Option<bool> {
    if !scripted() {
        return None;
    }
    let name: Vec<u8> = (begin..end).map(get_from_memory::<u8>).collect();
    record(format!("setBS {} {call_flag} {force} {mode}", String::from_utf8_lossy(&name)));
    Some(true)
}

fn plant_listen(guest: ZTGuest, rng: &mut Lcg) {
    live_support::forget_standalone_guest(guest);
    let ids = [0x2715u16, 0x2717, 0x272a, 0x272b, 0x272c, 0x272d, 0x2716, 0x2700, 0x2730];
    let times = [0u32, 5, 10, 11, 19, 20, 25, 29, 30, 31, 100];
    let count = (rng.next() % 7) as usize;
    let begin = leaked(0x1c * 8);
    for index in 0..count {
        let record = begin + index as u32 * 0x1c;
        save_to_memory(record + 0x10, times[rng.next() as usize % times.len()]);
        save_to_memory(record + 0x14, (rng.next() % 0x500) as u16);
        save_to_memory(record + 0x16, ids[rng.next() as usize % ids.len()]);
        save_to_memory(record + 0x18, (rng.next() % 3) as u16);
    }
    save_to_memory(guest.0 + 0x4, begin);
    save_to_memory(guest.0 + 0x8, begin + count as u32 * 0x1c);
    let animation: &'static [u8] = if rng.next() % 4 == 0 { b"bExitShowBad" } else { b"bWalk" };
    let start = Box::leak(animation.to_vec().into_boxed_slice()).as_ptr() as u32;
    save_to_memory(guest.0 + 0x54, start);
    save_to_memory(guest.0 + 0x58, start + animation.len() as u32);
    let manager = if rng.next() % 4 == 0 {
        0
    } else {
        let manager = leaked(0x40);
        let low = rng.next() % 20;
        save_to_memory(manager + 0x14, low);
        save_to_memory(manager + 0x18, low + rng.next() % 12);
        save_to_memory(manager + 0x1c, low + 12 + rng.next() % 12);
        manager
    };
    save_to_memory(base() + 0x0023_9008, manager);
}

/// `ZTGUEST_LISTEN_LIVE`: `listen` over planted message vectors, animations and show-manager thresholds against the
/// real body; `setBS`, the thought, the mood change and the events slot are scripted. A reach check confirms every
/// behaviour-set name and thought was produced.
pub(crate) fn run_ztguest_listen_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::LISTEN;

    let test_name = "ZTGUEST_LISTEN_LIVE";
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
    let saved_manager: u32 = get_from_memory(base() + 0x0023_9008);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_set_bs);
    LISTEN_MODE.store(true, Ordering::SeqCst);
    let mut logs = Vec::new();
    for round_base in 0..4u32 {
        ROUND_BASE.store(round_base * 1_000_003, Ordering::SeqCst);
        logs.extend(compare_check("listen", guest_type, &mut failures, live_support::goals_real::listen, |g| ZTGuest(g).listen(), |g| unsafe {
            LISTEN.hooked()(g as *const u32)
        }));
    }
    ROUND_BASE.store(0, Ordering::SeqCst);
    LISTEN_MODE.store(false, Ordering::SeqCst);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = None;
    save_to_memory(base() + 0x0023_9008, saved_manager);
    pin_rng(saved_rng);
    for marker in [
        "setBS bExitShow 1 1 2",
        "setBS bExitShowBad",
        "setBS bExitShowGreat",
        "setBS bExitShowGood",
        "setBS bBadTrick",
        "setBS bGoodTrick",
        "setBS bGreatTrick",
        "thought 0x282f",
        "thought 0x2830",
        "thought 0x2831",
        "thought 0x2832",
        "thought 0x2972",
        "thought 0x2974",
        "thought 0x2975",
        "slot moodChange",
        "slot afterMood",
    ] {
        if count_logs(&logs, marker) == 0 {
            failures.push(format!("listen: no case ever logged `{marker}`"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// Set while the viewing-area check cases plant their countdown.
static VIEWING_CHECK_MODE: AtomicBool = AtomicBool::new(false);

fn scripted_num_sites(_this: u32) -> Option<i32> {
    if !scripted() || SITES_MODE.load(Ordering::SeqCst) {
        return None;
    }
    let answer = roll(3) as i32 - 1;
    record(format!("getNumNonseenSites {answer}"));
    Some(answer)
}

fn scripted_find_viewing_area(_this: u32) -> Option<bool> {
    if !scripted() {
        return None;
    }
    let found = roll(4) != 0;
    record(format!("findViewingArea {found}"));
    Some(found)
}

fn scripted_num_animals(_this: u32) -> Option<u32> {
    scripted().then(|| {
        let answer = roll(4);
        record(format!("fNumAnimals {answer}"));
        answer
    })
}

fn scripted_sick_animals(_this: u32) -> Option<u32> {
    scripted().then(|| {
        let answer = (roll(3) == 0) as u32;
        record(format!("fSickAnimals {answer}"));
        answer
    })
}

fn scripted_num_sick_animals(_this: u32) -> Option<u32> {
    scripted().then(|| {
        let answer = roll(4);
        record(format!("fNumSickAnimals {answer}"));
        answer
    })
}

fn scripted_different_species(_this: u32) -> Option<u32> {
    scripted().then(|| {
        let answer = roll(5);
        record(format!("fDifferentSpecies {answer}"));
        answer
    })
}

fn scripted_crowd_density(_this: u32) -> Option<i64> {
    scripted().then(|| {
        let answer = roll(60) as i64;
        record(format!("fCrowdDensity {answer}"));
        answer
    })
}

fn scripted_find_animal(_this: u32, low: i32, high: i32) -> Option<u32> {
    scripted().then(|| {
        let animal = if roll(3) == 0 { 0 } else { fake_building() };
        record(format!("findAnimalByHappiness {low} {high} {}", animal != 0));
        animal
    })
}

fn scripted_animal_happy(_this: u32) -> Option<i32> {
    if !scripted() {
        return None;
    }
    let values = [-100, -0x51, -0x50, -0x3d, -0x3c, -1, 0, 1, 0x3c, 0x3d, 0x50, 0x51, 100];
    let answer = values[roll(values.len() as u32) as usize];
    record(format!("fAnimalHappy {answer}"));
    Some(answer)
}

/// `ZTGUEST_VIEWING_AREA_CHECK_LIVE`: `doViewingAreaCheck` against the real body with the viewing-area search, the
/// animal queries and the average happiness scripted. A reach check confirms every happiness band, the sick, variety
/// and crowd branches ran.
pub(crate) fn run_ztguest_viewing_area_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::DO_VIEWING_AREA_CHECK;

    let test_name = "ZTGUEST_VIEWING_AREA_CHECK_LIVE";
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
    VIEWING_CHECK_MODE.store(true, Ordering::SeqCst);
    let mut logs = Vec::new();
    for round_base in 0..6u32 {
        ROUND_BASE.store(round_base * 1_000_003, Ordering::SeqCst);
        logs.extend(compare_check(
            "doViewingAreaCheck",
            guest_type,
            &mut failures,
            live_support::checks_real::do_viewing_area_check,
            |g| ZTGuest(g).do_viewing_area_check(),
            |g| unsafe { DO_VIEWING_AREA_CHECK.hooked()(g as *const u32) },
        ));
    }
    ROUND_BASE.store(0, Ordering::SeqCst);
    VIEWING_CHECK_MODE.store(false, Ordering::SeqCst);
    pin_rng(saved_rng);
    for marker in [
        "thought 0x2931",
        "thought 0x2770",
        "thought 0x2772",
        "thought 0x2829",
        "thought 0x282a",
        "thought 0x282c",
        "thought 0x282b",
        "thought 0x2773",
        "thought 0x2774",
        "thought 0x2776",
        "findAnimalByHappiness -100 100",
    ] {
        if count_logs(&logs, marker) == 0 {
            failures.push(format!("doViewingAreaCheck: no case ever logged `{marker}`"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// Set while the unseen-site cases plant visited sites and the fake scenery.
static SITES_MODE: AtomicBool = AtomicBool::new(false);

const FAKE_SCENERY: usize = 6;

extern "thiscall" fn scenery_type_id(type_ptr: u32) -> u32 {
    get_from_memory(type_ptr + 0x124)
}

/// Fake gawk scenery: `(entity, type, tile)` triples whose type id, gawk value (`+0x120`) and tile position are
/// replanted per case.
fn fake_scenery() -> Vec<(u32, u32, u32)> {
    static SCENERY: std::sync::OnceLock<Vec<(u32, u32, u32)>> = std::sync::OnceLock::new();
    SCENERY
        .get_or_init(|| {
            (0..FAKE_SCENERY)
                .map(|_| {
                    let mut vtable = vec![0u32; 0x100];
                    vtable[0x20 / 4] = scenery_type_id as *const () as usize as u32;
                    let vtable = Box::leak(vtable.into_boxed_slice()).as_ptr() as u32;
                    let entity_type = leaked(0x200);
                    save_to_memory(entity_type, vtable);
                    let entity = leaked(0x400);
                    save_to_memory(entity + 0x128, entity_type);
                    (entity, entity_type, leaked(0x100))
                })
                .collect()
        })
        .clone()
}

fn scenery_tile(entity: u32) -> Option<u32> {
    if !SITES_MODE.load(Ordering::SeqCst) || !scripted() {
        return None;
    }
    fake_scenery().iter().find(|(candidate, _, _)| *candidate == entity).map(|(_, _, tile)| *tile)
}

/// Pushes a scripted subset of the fake scenery (and sometimes a null entry) onto the vanilla list whose handle slot
/// is `slot`.
fn fill_scenery_list(slot: u32, label: &str) {
    use openzt_detour::generated::poolalloc::ALLOCATE;

    let head: u32 = get_from_memory(slot);
    let mut pushed = Vec::new();
    for (entity, _, _) in fake_scenery() {
        if roll(3) == 0 {
            continue;
        }
        pushed.push(entity);
        if roll(8) == 0 {
            pushed.push(0);
        }
    }
    for value in &pushed {
        let node = unsafe { ALLOCATE.hooked()(12) } as u32;
        let last: u32 = get_from_memory(head + 4);
        save_to_memory(node, head);
        save_to_memory(node + 4, last);
        save_to_memory(node + 8, *value);
        save_to_memory(last, node);
        save_to_memory(head + 4, node);
    }
    record(format!("{label} {}", pushed.len()));
}

/// Tiles of the live map that resolve to a habitat, for planting visited sites the unseen-habitat query must skip.
fn habitat_tiles() -> Vec<(i32, i32)> {
    use crate::globals::globals;
    use openzt_detour::generated::zthabitatmgr::GET_HABITAT;

    static TILES: std::sync::OnceLock<Vec<(i32, i32)>> = std::sync::OnceLock::new();
    TILES
        .get_or_init(|| {
            let manager = globals().zthabitatmgr_ptr() as u32;
            let world = globals().ztworldmgr();
            let (size_x, size_y) = (world.map_x_size as i32, world.map_y_size as i32);
            let mut seen = std::collections::BTreeSet::new();
            let mut tiles = Vec::new();
            if manager == 0 {
                return tiles;
            }
            for y in 0..size_y {
                for x in 0..size_x {
                    let habitat = unsafe { GET_HABITAT.hooked()(manager as *const u32, x, y) } as u32;
                    if habitat != 0 && seen.insert(habitat) {
                        tiles.push((x, y));
                    }
                }
            }
            tiles
        })
        .clone()
}

fn plant_sites(guest: ZTGuest, rng: &mut Lcg) {
    let tiles = habitat_tiles();
    live_support::forget_standalone_guest(guest);
    let count = (rng.next() % 4) as usize;
    let begin = leaked(12 * 6);
    for index in 0..count {
        let (x, y) = if !tiles.is_empty() && rng.next() % 2 == 0 { tiles[rng.next() as usize % tiles.len()] } else { (-1, -1) };
        save_to_memory(begin + index as u32 * 12, x);
        save_to_memory(begin + index as u32 * 12 + 4, y);
    }
    save_to_memory(guest.0 + guest_offsets::VISITED_SITES, begin);
    save_to_memory(guest.0 + guest_offsets::VISITED_SITES + 4, begin + count as u32 * 12);
    for (index, (entity, entity_type, tile)) in fake_scenery().into_iter().enumerate() {
        save_to_memory(entity + 0x124, index as u32);
        save_to_memory(entity_type + 0x124, [0u32, 3, 4, 5, 3, 0][rng.next() as usize % 6]);
        save_to_memory(entity_type + 0x120, (rng.next() % 400) as i32 - 50);
        save_to_memory(tile + 0x34, (rng.next() % 60) as i32);
        save_to_memory(tile + 0x38, (rng.next() % 60) as i32);
    }
    save_to_memory(fake_tile() + 0x34, (rng.next() % 60) as i32);
    save_to_memory(fake_tile() + 0x38, (rng.next() % 60) as i32);
}

/// Reads and frees a vanilla list result.
fn drain_list(slot: u32) -> Vec<u32> {
    use openzt_detour::generated::poolalloc::DEALLOCATE_N_12;

    let head: u32 = get_from_memory(slot);
    let mut values = Vec::new();
    let mut node: u32 = get_from_memory(head);
    while node != head {
        values.push(get_from_memory(node + 8));
        let next: u32 = get_from_memory(node);
        unsafe { DEALLOCATE_N_12.hooked()(node as *const u32, 1) };
        node = next;
    }
    unsafe { DEALLOCATE_N_12.hooked()(head as *const u32, 1) };
    values
}

/// Reads and frees a vanilla `vector<int>` result.
fn drain_vector(vector: [u32; 3]) -> Vec<u32> {
    use openzt_detour::generated::poolalloc::DEALLOCATE;

    let values = (0..vector[1].wrapping_sub(vector[0]) / 4).map(|index| get_from_memory::<u32>(vector[0] + index * 4)).collect();
    if vector[0] != 0 {
        unsafe { DEALLOCATE.hooked()(vector[0] as *const u32, vector[2].wrapping_sub(vector[0])) };
    }
    values
}

/// `ZTGUEST_SITES_LIVE`: the unseen-site queries against the real bodies, with the world's attraction and gawk lists
/// scripted (a random subset of fake scenery, sometimes a null entry), visited sites planted on habitat tiles and
/// off the map, and fake scenery whose type id, gawk value and tile position vary. List and vector results are compared by
/// contents.
pub(crate) fn run_ztguest_sites_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::{
        GET_NONSEEN_ATTRACTIONS, GET_NONSEEN_GAWK_SCENERY, GET_NONSEEN_HABITATS, GET_NUM_NONSEEN_GAWK_SCENERY, GET_NUM_NONSEEN_SITES, PICK_GAWK_SCENERY,
    };

    let test_name = "ZTGUEST_SITES_LIVE";
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
    SITES_MODE.store(true, Ordering::SeqCst);
    let this = |address: u32| address as *const u32;
    let mut run_list = |name: &str, real_fn: fn(u32, u32) -> u32, port: fn(ZTGuest, u32) -> u32, hooked: &dyn Fn(u32, u32) -> u32| {
        let mut logs = Vec::new();
        for round_base in 0..3u32 {
            ROUND_BASE.store(round_base * 1_000_003, Ordering::SeqCst);
            let body = |pole: usize| {
                move |g: u32| {
                    let mut slot = 0u32;
                    let slot_address = &mut slot as *mut u32 as u32;
                    match pole {
                        0 => real_fn(g, slot_address),
                        1 => port(ZTGuest(g), slot_address),
                        _ => hooked(g, slot_address),
                    };
                    drain_list(slot_address)
                }
            };
            logs.extend(compare_check(name, guest_type, &mut failures, body(0), body(1), body(2)));

        }
        ROUND_BASE.store(0, Ordering::SeqCst);
        logs
    };
    let attractions = run_list(
        "getNonseenAttractions",
        live_support::sites_real::get_nonseen_attractions,
        |g, out| g.get_nonseen_attractions(out),
        &|g, out| unsafe { GET_NONSEEN_ATTRACTIONS.hooked()(this(g), out as *const i32) as u32 },
    );
    let gawk = run_list(
        "getNonseenGawkScenery",
        live_support::sites_real::get_nonseen_gawk_scenery,
        |g, out| g.get_nonseen_gawk_scenery(out),
        &|g, out| unsafe { GET_NONSEEN_GAWK_SCENERY.hooked()(this(g), out as *const i32) as u32 },
    );
    for (from_world, scaled) in [(true, true), (true, false), (false, false), (false, true)] {
        ROUND_BASE.store(7_000_003 * (from_world as u32 + 2 * scaled as u32), Ordering::SeqCst);
        compare_check(
            &format!("getNumNonseenGawkScenery {from_world} {scaled}"),
            guest_type,
            &mut failures,
            |g| live_support::sites_real::get_num_nonseen_gawk_scenery(g, from_world, scaled),
            |g| ZTGuest(g).get_num_nonseen_gawk_scenery(from_world, scaled),
            |g| unsafe { GET_NUM_NONSEEN_GAWK_SCENERY.hooked()(this(g), from_world, scaled) as i32 },
        );
    }
    ROUND_BASE.store(0, Ordering::SeqCst);
    let habitat_body = |pole: usize| {
        move |g: u32| {
            let mut vector = [0u32; 3];
            let out = vector.as_mut_ptr() as u32;
            match pole {
                0 => live_support::sites_real::get_nonseen_habitats(g, out),
                1 => ZTGuest(g).get_nonseen_habitats(out),
                _ => unsafe { GET_NONSEEN_HABITATS.hooked()(this(g), out as *const u32) as u32 },
            };
            drain_vector(vector)
        }
    };
    compare_check("getNonseenHabitats", guest_type, &mut failures, habitat_body(0), habitat_body(1), habitat_body(2));
    compare_check(
        "getNumNonseenSites",
        guest_type,
        &mut failures,
        live_support::sites_real::get_num_nonseen_sites,
        |g| ZTGuest(g).get_num_nonseen_sites(),
        |g| unsafe { GET_NUM_NONSEEN_SITES.hooked()(this(g)) },
    );
    let pick = compare_check(
        "pickGawkScenery",
        guest_type,
        &mut failures,
        live_support::sites_real::pick_gawk_scenery,
        |g| ZTGuest(g).pick_gawk_scenery(),
        |g| unsafe { PICK_GAWK_SCENERY.hooked()(this(g)) as u32 },
    );
    SITES_MODE.store(false, Ordering::SeqCst);
    pin_rng(saved_rng);
    for (name, logs) in [("getNonseenAttractions", &attractions), ("getNonseenGawkScenery", &gawk)] {
        if count_logs(logs, "getAttractions") + count_logs(logs, "getGawkScenery") == 0 {
            failures.push(format!("{name}: the scripted world list was never asked for"));
        }
    }
    let _ = pick;
    finish_test(test_name, failures, failure_log)
}

/// Set while the item cases plant their grid, position and item record.
static ITEMS_MODE: AtomicBool = AtomicBool::new(false);

fn fake_item_building() -> u32 {
    static BUILDING: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *BUILDING.get_or_init(|| {
        let building = leaked(0x400);
        let vtable = leaked(0x100);
        save_to_memory(vtable + 0x1c, fake_cast_true as *const () as usize as u32);
        let ty = leaked(0x40);
        save_to_memory(ty, vtable);
        save_to_memory(building + 0x128, ty);
        building
    })
}

fn fake_item_grid() -> u32 {
    static GRID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *GRID.get_or_init(|| leaked(0x40))
}

fn fake_item_record() -> u32 {
    static RECORD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *RECORD.get_or_init(|| leaked(0x40))
}

fn plant_items(guest: ZTGuest, rng: &mut Lcg) {
    live_support::forget_standalone_guest(guest);
    let grid = fake_item_grid();
    for index in 0..4u32 {
        save_to_memory(grid + 4 + index * 4, if rng.next() % 3 == 0 { 0 } else { fake_item_building() });
    }
    save_to_memory(guest.0 + 0x244, if rng.next() % 6 == 0 { 0 } else { grid });
    for offset in [0x214u32, 0x218] {
        save_to_memory(guest.0 + offset, (rng.next() % 72) as i32 - 4);
    }
    save_to_memory(guest.0 + 0x24d, if rng.next() % 4 == 0 { 0xffu8 } else { 3 });
    // `getNeedFromIndex` is only defined for the real need indices.
    let name_rvas = [0x23c4c8u32, 0x23c4d8, 0x23c4e8, 0x23c4f8, 0x23c508, 0x23c518, 0x23c528];
    let need_index = unsafe { GET_NEED_INDEX.hooked()((base() + name_rvas[rng.next() as usize % name_rvas.len()]) as *const i8) as u8 };
    save_to_memory(guest.0 + 0x24e, need_index);
    let record = fake_item_record();
    save_to_memory(record, [9211u32, 0x18a6, 5][rng.next() as usize % 3]);
    for index in 0..6u32 {
        save_to_memory(record + 4 + index * 4, (rng.next() % 30) as i32 - 5);
    }
    save_to_memory(record + 0x1d, (rng.next() % 2) as u8);
    save_to_memory(record + 0x1e, (rng.next() % 2) as u8);
    save_to_memory(record + 0x20, if rng.next() % 2 == 0 { 0u32 } else { 0x2764 });
}

fn item_slots(guest: u32) -> Vec<u32> {
    let begin: u32 = get_from_memory(guest + guest_offsets::ITEMS);
    (0..5u32).map(|slot| get_from_memory(begin + slot * 4)).collect()
}

/// A vanilla pool string holding `bytes` (and a terminator), as `{begin, end, capacity}`; `(0, 0, 0)` for none.
fn pool_string(bytes: &[u8]) -> (u32, u32, u32) {
    use openzt_detour::generated::poolalloc::ALLOCATE;

    let begin = unsafe { ALLOCATE.hooked()(bytes.len() as u32 + 1) } as u32;
    for (index, byte) in bytes.iter().enumerate() {
        save_to_memory(begin + index as u32, *byte);
    }
    save_to_memory(begin + bytes.len() as u32, 0u8);
    (begin, begin + bytes.len() as u32, begin + bytes.len() as u32 + 1)
}

/// `ZTGUEST_ITEMS_LIVE`: `addItem` (each of the five kinds, an unknown name, an empty name taken from the need index),
/// `consumeItem` and `getNeedItem` against the real bodies, with the item record, grid, building and satisfier scripted.
pub(crate) fn run_ztguest_items_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::{ADD_ITEM, CONSUME_ITEM, GET_NEED_ITEM};

    let test_name = "ZTGUEST_ITEMS_LIVE";
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
    ITEMS_MODE.store(true, Ordering::SeqCst);
    let this = |address: u32| address as *const u32;
    let item_name = |rva: u32| -> Vec<u8> {
        let base = crate::globals::get_module_base("zoo.exe") as u32;
        let (begin, end): (u32, u32) = (get_from_memory(base + rva), get_from_memory(base + rva + 4));
        (begin..end).map(get_from_memory::<u8>).collect()
    };
    let names: Vec<(String, Vec<u8>)> = vec![
        ("hunger".to_string(), item_name(0x23c4e8)),
        ("thirst".to_string(), item_name(0x23c4c8)),
        ("slot2".to_string(), item_name(0x23b058)),
        ("souvenir".to_string(), item_name(0x23c528)),
        ("trash".to_string(), item_name(0x23c508)),
        ("other".to_string(), b"zzz".to_vec()),
        ("empty".to_string(), Vec::new()),
    ];
    let mut stored_cases = 0;
    for (label, bytes) in &names {
        for variant in 0..2u32 {
            if (bytes.is_empty() && variant == 1) || (!bytes.is_empty() && variant == 0) {
                let outcomes = compare_check(
                    &format!("addItem {label} {variant}"),
                    guest_type,
                    &mut failures,
                    |g| {
                        let (b, e, c) = pool_string(bytes);
                        let result = live_support::inventory_real::add_item(g, 0x1234, b, e, c);
                        (result, item_slots(g), get_from_memory::<u8>(g + 0x33c))
                    },
                    |g| {
                        let (b, e, c) = pool_string(bytes);
                        let result = ZTGuest(g).add_item(0x1234, b, e, c);
                        (result, item_slots(g), get_from_memory::<u8>(g + 0x33c))
                    },
                    |g| {
                        let (b, e, c) = pool_string(bytes);
                        let result = unsafe { ADD_ITEM.hooked()(this(g), 0x1234 as *const u8, b as *const u32, e as *const u32, c as i32) };
                        (result, item_slots(g), get_from_memory::<u8>(g + 0x33c))
                    },
                );
                stored_cases += outcomes.len();
            }
        }
    }
    let _ = stored_cases;
    let record = fake_item_record();
    let mut consumed = Vec::new();
    for round_base in 0..2u32 {
        ROUND_BASE.store(round_base * 1_000_003, Ordering::SeqCst);
        consumed.extend(compare_check(
            "consumeItem",
            guest_type,
            &mut failures,
            |g| (live_support::inventory_real::consume_item(g, record) & 0xff, item_slots(g)),
            |g| (ZTGuest(g).consume_item(record) as u32, item_slots(g)),
            |g| unsafe { (CONSUME_ITEM.hooked()(this(g), this(record)) & 0xff, item_slots(g)) },
        ));
    }
    ROUND_BASE.store(0, Ordering::SeqCst);
    let mut needed = Vec::new();
    for round_base in 0..2u32 {
        ROUND_BASE.store(round_base * 1_000_003, Ordering::SeqCst);
        needed.extend(compare_check(
            "getNeedItem",
            guest_type,
            &mut failures,
            live_support::inventory_real::get_need_item,
            |g| ZTGuest(g).get_need_item(),
            |g| unsafe { GET_NEED_ITEM.hooked()(this(g)) as u32 },
        ));
    }
    ROUND_BASE.store(0, Ordering::SeqCst);
    ITEMS_MODE.store(false, Ordering::SeqCst);
    pin_rng(saved_rng);
    if count_logs(&consumed, "slot moodChange") == 0 {
        failures.push("consumeItem: the mood slot was never reached".to_string());
    }
    if count_logs(&needed, "satisfies true") == 0 {
        failures.push("getNeedItem: the satisfier was never asked about the building".to_string());
    }
    finish_test(test_name, failures, failure_log)
}
