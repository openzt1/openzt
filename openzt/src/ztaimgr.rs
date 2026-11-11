//! `ZTAIMgr` leaf-method reimplementation: the vtable methods that read a handful of fields and return a
//! value, with no allocation and no manager-owned state beyond a few `ZTAIMgr` dwords.
//!
//! `ZTAIMgr` extends `BFAIMgr` (`BFAIMgr` -> `ZTAIMgr`, vtable `0x00635324`). The AI manager is a
//! singleton (`GLOBAL_ZTAIMgr`) that un-ported vanilla code reaches through its vtable and reads by raw
//! offset, so nothing here replaces the object: each detour reads and writes vanilla's own memory in place
//! (style 1), and no struct is declared because only a few fixed dwords are touched (the offsets below).
//!
//! Also reimplemented: the thin handler halves `fEndTrick`, `fEnterBuilding`, `fReturnToBuilding`,
//! `fHidePredator`, `fGuestRoutine`/`fMaintRoutine`/`fGuideRoutine`/`fKeeperRoutine`, their `F` wrappers and
//! `fSprayF`. The entity `fRoutine` bodies stay vanilla; cases where vanilla faults on a null `this` defer to it.
//!
//! Reimplemented: `getRandomX`/`getRandomY`, `isSlowMoveFunction`/`isMediumMoveFunction`/
//! `isFastMoveFunction`, `convertConstantParam` (partial override, see below) and `removeEntity`.
//! Not detoured: `init` (a bare `jmp [vtable+0x38]` thunk and the never-reached `BFAIMgr::init`),
//! `removeEntities`, `callDone`, `initAIParams` and everything the `executeCall` dispatch chain reaches.
//!
//! Windows layout differs from the macOS build (`ZTAIMgr` fields sit `+4` later, entity-type fields `+0x18`
//! later), so every offset here comes from the Windows decompile, never the macOS one.

use std::{ffi::c_void, mem};

use openzt_detour::generated::{
    ztaimgr::{
        CONVERT_CONSTANT_PARAM, F_END_TRICK, F_END_TRICK_F, F_ENTER_BUILDING, F_ENTER_BUILDING_F, F_GUEST_ROUTINE, F_GUEST_ROUTINE_F, F_GUIDE_ROUTINE, F_GUIDE_ROUTINE_F,
        F_HIDE_PREDATOR, F_HIDE_PREDATOR_F, F_KEEPER_ROUTINE, F_KEEPER_ROUTINE_F, F_MAINT_ROUTINE, F_MAINT_ROUTINE_F, F_RETURN_TO_BUILDING, F_RETURN_TO_BUILDING_F, F_SPRAY_F,
        GET_RANDOM_X, GET_RANDOM_Y, IS_FAST_MOVE_FUNCTION, IS_MEDIUM_MOVE_FUNCTION, IS_SLOW_MOVE_FUNCTION, REMOVE_ENTITY,
    },
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    ztaimgr::{
        F_CAUGHT_PREY, F_CAUGHT_PREY_F, F_DIE, F_DIE_F, F_EXIT_BUILDING, F_EXIT_BUILDING_F, F_HATCH, F_HATCH_F, F_KILL_PREY, F_KILL_PREY_F, F_SHOW_PREDATOR, F_SHOW_PREDATOR_F,
    },
    bfaimgr::{EXECUTE_CALL as BFAIMGR_EXECUTE_CALL, F_MOVE_FAST_F, F_MOVE_MEDIUM_F, F_MOVE_SLOW_F},
    ztaimgr::{EXECUTE_CALL, F_DUST_BALL_F, F_FACE_TOWARD_FOOD, F_FACE_TOWARD_FOOD_F, F_FACE_TOWARD_TARGET, F_FACE_TOWARD_TARGET_F, F_PLAY_SET_TANK_F, F_TARGET_BUILDING_F},
    ztanimal::{CONSUME as ZTANIMAL_CONSUME, FACE_TOWARD_FOOD, FINISH_HATCH},
    ztguest::F_ROUTINE as ZTGUEST_F_ROUTINE,
    zthabitat::TRIGGER_DEATH_ARRIVED,
    ztunit::{FACE_TOWARD_TARGET, FACE_TOWARD_TARGET_UNIT, F_ZOO_MESSAGE_TILE_0, GET_PREDATOR_UNIT, GET_TARGET_UNIT as ZTUNIT_GET_TARGET_UNIT, SET_IS_REMOVED as ZTUNIT_SET_IS_REMOVED},
    ztmapview::PREPARE_TO_DELETE_ENTITY,
    ztui_general::GET_MAPVIEW,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    util::get_from_memory,
};

/// `ZTAIMgr` dword read by [`get_random_x`] for a guest.
const OFFSET_GUEST_RANDOM_X: u32 = 0x140;
/// `ZTAIMgr` dword read by [`get_random_y`] for a guest.
const OFFSET_GUEST_RANDOM_Y: u32 = 0x144;
/// `ZTAIMgr` dword read by [`get_random_x`] for staff.
const OFFSET_STAFF_RANDOM_X: u32 = 0x148;
/// `ZTAIMgr` dword read by [`get_random_y`] for staff.
const OFFSET_STAFF_RANDOM_Y: u32 = 0x14c;
/// `ZTAIMgr` dword read by [`get_random_x`] for any other entity (animals).
const OFFSET_DEFAULT_RANDOM_X: u32 = 0xdc;
/// `ZTAIMgr` dword read by [`get_random_y`] for any other entity (animals).
const OFFSET_DEFAULT_RANDOM_Y: u32 = 0xe0;

/// `BFEntity`'s pointer to its `BFEntityType`.
const OFFSET_ENTITY_TYPE: u32 = 0x128;
/// `BFEntityType` vtable slot `isClass(tag)`: exact-class test.
const TYPE_VTABLE_IS_CLASS: u32 = 0x18;
/// `BFEntityType` vtable slot `isCastClass(tag)`: is-or-derives-from test.
const TYPE_VTABLE_IS_CAST_CLASS: u32 = 0x1c;
/// `BFEntityWorldMgr` vtable slot `removeEntity(entity, bool)`.
const WORLD_MGR_VTABLE_REMOVE_ENTITY: u32 = 0x1c;

/// Class-tag constants (`&CAST_ZT*`) the type checks take, as RVAs into the `0x10`-spaced tag table.
const RVA_TAG_ZTGUEST: u32 = 0x0023_8700;
const RVA_TAG_ZTSTAFF: u32 = 0x0023_8710;
const RVA_TAG_ZTUNITTYPE: u32 = 0x0023_86b0;
const RVA_TAG_ZTANIMAL: u32 = 0x0023_8690;
const RVA_TAG_ZTKEEPER: u32 = 0x0023_8760;
const RVA_TAG_ZTGUIDE: u32 = 0x0023_8770;
const RVA_TAG_ZTMAINT: u32 = 0x0023_8780;

/// Entity vtable slots the building/trick handlers dispatch through (thiscall, no arguments).
const ENTITY_VTABLE_END_TRICK: u32 = 0x2bc;
const ENTITY_VTABLE_ENTER_BUILDING: u32 = 0x2a8;
const ENTITY_VTABLE_RETURN_TO_BUILDING: u32 = 0x2b0;
/// Entity vtable slot `fHidePredator` calls on the predator with `0`.
const ENTITY_VTABLE_HIDE_PREDATOR: u32 = 0xcc;
/// Offset of the `done` flag inside the call object the `F` wrappers receive.
const CALL_DONE_FLAG_OFFSET: u32 = 0x18;

/// `ZTGuide::fRoutine` and `ZTKeeper::fRoutine` (`thiscall(this, done)`): no `generated.rs` entry, so they
/// are called by Ghidra address.
const ZTGUIDE_F_ROUTINE_ADDRESS: u32 = 0x0058_a929;
const ZTKEEPER_F_ROUTINE_ADDRESS: u32 = 0x004a_86ba;
const RVA_TAG_ZTGUIDETYPE: u32 = 0x0023_8770;

/// `ZTUnitType` dwords `convertConstantParam` returns, in `RANDOM_TIME_NAMES` order.
const UNIT_TYPE_RANDOM_TIME_OFFSETS: [u32; 6] = [0x138, 0x13c, 0x140, 0x144, 0x148, 0x14c];
/// `ZTGuideType` dword `convertConstantParam` returns for `cInformGuestTime`.
const GUIDE_TYPE_INFORM_GUEST_TIME_OFFSET: u32 = 0x1f4;
const INFORM_GUEST_TIME_NAME: &[u8] = b"cInformGuestTime";
const RANDOM_TIME_NAMES: [&[u8]; 6] = [
    b"cWalkRandomlyTime1",
    b"cWalkRandomlyTime2",
    b"cWalkRandomlyTime3",
    b"cRunRandomlyTime1",
    b"cRunRandomlyTime2",
    b"cRunRandomlyTime3",
];

/// `ZTCheat::entityRemoved` (`cdecl`, one entity argument), the first thing `removeEntity` calls. Not in
/// `generated.rs`, so it is called by its Ghidra address (code addresses need no base adjustment).
const ZTCHEAT_ENTITY_REMOVED_ADDRESS: u32 = 0x0041_e1fd;

/// A `{begin, end, capacity}` string as `BFFunctionCall` stores its name and each parameter.
#[repr(C)]
struct RawCallString {
    begin: *const u8,
    end: *const u8,
    capacity: *const u8,
}

impl RawCallString {
    fn bytes(&self) -> &[u8] {
        if self.begin.is_null() || self.end < self.begin {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.begin, self.end as usize - self.begin as usize) }
    }
}

/// `BFFunctionCall`: the function name plus its parameter vector (0xc-byte entries).
#[repr(C)]
struct RawFunctionCall {
    name: RawCallString,
    params_begin: *const RawCallString,
    params_end: *const RawCallString,
    params_capacity: *const RawCallString,
}

impl RawFunctionCall {
    /// Parameter `index`, or `None` if the call has fewer parameters.
    fn param(&self, index: usize) -> Option<&RawCallString> {
        if self.params_begin.is_null() {
            return None;
        }
        let count = (self.params_end as usize - self.params_begin as usize) / mem::size_of::<RawCallString>();
        if index < count {
            Some(unsafe { &*self.params_begin.add(index) })
        } else {
            None
        }
    }

    /// `name == base`, or `name == "fMove"` with the third parameter equal to `variant`. Vanilla reads the
    /// third parameter without a count check; a call with fewer parameters compares as unequal here.
    fn is_move_function(&self, base: &[u8], also: &[&[u8]], variant: &[u8]) -> bool {
        let name = self.name.bytes();
        if name == base || also.contains(&name) {
            return true;
        }
        name == b"fMove" && self.param(2).is_some_and(|p| p.bytes() == variant)
    }
}

/// Calls `BFEntityType` vtable `slot` with a class tag (`this = type_ptr`).
unsafe fn type_tag_check(type_ptr: u32, slot: u32, tag_rva: u32) -> bool {
    let vtable = get_from_memory::<u32>(type_ptr);
    let target = get_from_memory::<u32>(vtable + slot);
    let check = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(target) };
    check(type_ptr, get_module_base("zoo.exe") as u32 + tag_rva)
}

/// Picks `guest`, `staff` or `default` by the entity's type. Vanilla dereferences the type without a null
/// check; a null type falls to `default` here.
unsafe fn random_extent(this: u32, entity: u32, guest: u32, staff: u32, default: u32) -> i32 {
    let entity_type = get_from_memory::<u32>(entity + OFFSET_ENTITY_TYPE);
    if entity_type == 0 {
        return get_from_memory::<i32>(this + default);
    }
    if unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CLASS, RVA_TAG_ZTGUEST) } {
        get_from_memory::<i32>(this + guest)
    } else if unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CLASS, RVA_TAG_ZTSTAFF) } {
        get_from_memory::<i32>(this + staff)
    } else {
        get_from_memory::<i32>(this + default)
    }
}

pub unsafe fn get_random_x(this: u32, entity: u32) -> i32 {
    unsafe { random_extent(this, entity, OFFSET_GUEST_RANDOM_X, OFFSET_STAFF_RANDOM_X, OFFSET_DEFAULT_RANDOM_X) }
}

pub unsafe fn get_random_y(this: u32, entity: u32) -> i32 {
    unsafe { random_extent(this, entity, OFFSET_GUEST_RANDOM_Y, OFFSET_STAFF_RANDOM_Y, OFFSET_DEFAULT_RANDOM_Y) }
}

pub unsafe fn is_slow_move_function(call: *const u32) -> bool {
    unsafe { &*(call as *const RawFunctionCall) }.is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk")
}

pub unsafe fn is_medium_move_function(call: *const u32) -> bool {
    unsafe { &*(call as *const RawFunctionCall) }.is_move_function(b"fMoveMedium", &[b"fTrot"], b"trot")
}

pub unsafe fn is_fast_move_function(call: *const u32) -> bool {
    unsafe { &*(call as *const RawFunctionCall) }.is_move_function(b"fMoveFast", &[b"fRun"], b"run")
}

/// The value vanilla returns for the eight named constants, or `None` when the name is not one of them
/// (vanilla then parses it as a number) or when a named constant is requested from an entity whose type is
/// not a `ZTUnitType` (vanilla then dereferences null); the caller defers to the real function for `None`,
/// so those cases behave exactly as vanilla.
pub unsafe fn convert_constant_param(name: *const c_void, entity: u32) -> Option<i32> {
    let name = unsafe { &*(name as *const RawCallString) }.bytes();
    let entity_type = get_from_memory::<u32>(entity + OFFSET_ENTITY_TYPE);
    let unit_type = if entity_type != 0 && unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        entity_type
    } else {
        0
    };

    if let Some(index) = RANDOM_TIME_NAMES.iter().position(|candidate| *candidate == name) {
        return (unit_type != 0).then(|| get_from_memory::<i32>(unit_type + UNIT_TYPE_RANDOM_TIME_OFFSETS[index]));
    }
    if name == INFORM_GUEST_TIME_NAME {
        let is_guide = unit_type != 0 && unsafe { type_tag_check(unit_type, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUIDETYPE) };
        return Some(if is_guide { get_from_memory::<i32>(unit_type + GUIDE_TYPE_INFORM_GUEST_TIME_OFFSET) } else { 0 });
    }
    None
}

/// Removes `entity` from the world: runs the cheat hook, lets the map view drop any reference to it, then
/// dispatches the world manager's own `removeEntity(entity, true)`. Returns `false` if there is no world.
pub unsafe fn remove_entity(entity: u32) -> bool {
    let world = globals().ztworldmgr_ptr() as u32;
    let entity_removed = unsafe { mem::transmute::<u32, extern "cdecl" fn(u32)>(ZTCHEAT_ENTITY_REMOVED_ADDRESS) };
    entity_removed(entity);
    if world == 0 {
        return false;
    }
    let mapview = unsafe { GET_MAPVIEW.original()() };
    if !mapview.is_null() {
        unsafe { PREPARE_TO_DELETE_ENTITY.original()(mapview, entity as *const u32) };
    }
    let vtable = get_from_memory::<u32>(world);
    let target = get_from_memory::<u32>(vtable + WORLD_MGR_VTABLE_REMOVE_ENTITY);
    let remove = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, bool) -> bool>(target) };
    remove(world, entity, true)
}

/// Calls a no-argument thiscall virtual at `slot` of the entity's own vtable.
unsafe fn call_entity_virtual(entity: u32, slot: u32) {
    let vtable = get_from_memory::<u32>(entity);
    let target = get_from_memory::<u32>(vtable + slot);
    let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32)>(target) };
    method(entity);
}

unsafe fn set_done(done: *const bool) {
    unsafe { (done as *mut bool).write_volatile(true) };
}

/// `unit` is non-null and its entity type passes the type's `slot` test against `tag_rva`.
unsafe fn unit_is(unit: u32, slot: u32, tag_rva: u32) -> bool {
    if unit == 0 {
        return false;
    }
    let entity_type = get_from_memory::<u32>(unit + OFFSET_ENTITY_TYPE);
    unsafe { type_tag_check(entity_type, slot, tag_rva) }
}

/// `fEndTrick`: ends the trick on a unit, or just finishes for a null/non-unit target.
pub unsafe fn f_end_trick(unit: u32, done: *const bool) -> bool {
    if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        unsafe { call_entity_virtual(unit, ENTITY_VTABLE_END_TRICK) };
    }
    unsafe { set_done(done) };
    true
}

/// `fEnterBuilding`/`fReturnToBuilding`. Returns `false` without acting when `unit` is null or not a unit
/// type: vanilla then calls a virtual through a null object and faults, so the caller defers to it.
unsafe fn building_transition(unit: u32, done: *const bool, slot: u32) -> bool {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        return false;
    }
    unsafe { call_entity_virtual(unit, slot) };
    unsafe { set_done(done) };
    true
}

pub unsafe fn f_enter_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_ENTER_BUILDING) }
}

pub unsafe fn f_return_to_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_RETURN_TO_BUILDING) }
}

/// `fHidePredator` for a guest: hides the guest's predator if it is an animal, then finishes. Returns
/// `false` without acting for a null/non-guest `unit`, which vanilla passes to `getPredatorUnit` as null.
pub unsafe fn f_hide_predator(unit: u32, done: *const bool) -> bool {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUEST) } {
        return false;
    }
    let predator = unsafe { GET_PREDATOR_UNIT.original()(unit as *const u32) } as u32;
    if predator != 0 && unsafe { unit_is(predator, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTANIMAL) } {
        let vtable = get_from_memory::<u32>(predator);
        let target = get_from_memory::<u32>(vtable + ENTITY_VTABLE_HIDE_PREDATOR);
        let hide = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32)>(target) };
        hide(predator, 0);
    }
    unsafe { set_done(done) };
    true
}

/// `fGuestRoutine`/`fMaintRoutine`/`fGuideRoutine`: run the entity's `fRoutine` when its type is exactly
/// `tag_rva`. Always reports success.
unsafe fn exact_class_routine(unit: u32, done: *const bool, tag_rva: u32, routine: unsafe fn(u32, *const bool)) -> bool {
    if unsafe { unit_is(unit, TYPE_VTABLE_IS_CLASS, tag_rva) } {
        unsafe { routine(unit, done) };
    }
    true
}

unsafe fn guest_f_routine(unit: u32, done: *const bool) {
    unsafe { ZTGUEST_F_ROUTINE.original()(unit as *const u32, done) };
}

unsafe fn guide_f_routine(unit: u32, done: *const bool) {
    let routine = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, *const bool) -> u32>(ZTGUIDE_F_ROUTINE_ADDRESS) };
    routine(unit, done);
}

pub unsafe fn f_guest_routine(unit: u32, done: *const bool) -> bool {
    unsafe { exact_class_routine(unit, done, RVA_TAG_ZTGUEST, guest_f_routine) }
}

/// `ZTMaint` runs the same (ICF-folded) `fRoutine` body as `ZTGuest`.
pub unsafe fn f_maint_routine(unit: u32, done: *const bool) -> bool {
    unsafe { exact_class_routine(unit, done, RVA_TAG_ZTMAINT, guest_f_routine) }
}

pub unsafe fn f_guide_routine(unit: u32, done: *const bool) -> bool {
    unsafe { exact_class_routine(unit, done, RVA_TAG_ZTGUIDE, guide_f_routine) }
}

/// `fKeeperRoutine`: the keeper's `fRoutine` result, or `false` for a non-keeper.
pub unsafe fn f_keeper_routine(unit: u32, done: *const bool) -> bool {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTKEEPER) } {
        return false;
    }
    let routine = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, *const bool) -> u32>(ZTKEEPER_F_ROUTINE_ADDRESS) };
    routine(unit, done) as u8 != 0
}

/// `ZTUnit` field `fShowPredator` clears and `fCaughtPrey` stores the hunter's id into on the prey.
const UNIT_SHOW_PREDATOR_FLAG_OFFSET: u32 = 0x20d;
const UNIT_PREY_HUNTER_FIELD_OFFSET: u32 = 0x22c;
/// Entity field read by `fCaughtPrey` and stored into the prey's [`UNIT_PREY_HUNTER_FIELD_OFFSET`].
const ENTITY_ID_OFFSET: u32 = 0x124;
/// Entity vtable slots: refresh after the predator/prey change (`+0x254`), find a fallback prey (`+0x25c`),
/// the fallback when there is none (`+0x258`), the unit's habitat (`+0x24c`), exit building (`+0x2ac`).
const ENTITY_VTABLE_REFRESH: u32 = 0x254;
const ENTITY_VTABLE_FIND_PREY: u32 = 0x25c;
const ENTITY_VTABLE_NO_PREY: u32 = 0x258;
const ENTITY_VTABLE_GET_HABITAT: u32 = 0x24c;
const ENTITY_VTABLE_EXIT_BUILDING: u32 = 0x2ac;
/// Message id and flags `fDie` posts at the animal's tile.
const DEATH_MESSAGE_ID: u32 = 0x2802;
const DEATH_MESSAGE_KIND: u32 = 2;

unsafe fn call_entity_virtual_ret(entity: u32, slot: u32) -> u32 {
    let vtable = get_from_memory::<u32>(entity);
    let target = get_from_memory::<u32>(vtable + slot);
    let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target) };
    method(entity)
}

unsafe fn is_animal(unit: u32) -> bool {
    unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTANIMAL) }
}

/// `fShowPredator`: has a guest's predator consume the guest, then refreshes and clears the show flag.
/// `None` (defer to vanilla) for a null/non-guest unit, which vanilla then dereferences as null.
pub unsafe fn f_show_predator(unit: u32, done: *const bool) -> Option<bool> {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUEST) } {
        return None;
    }
    let predator = unsafe { GET_PREDATOR_UNIT.original()(unit as *const u32) } as u32;
    if predator != 0 && unsafe { is_animal(predator) } {
        unsafe { ZTANIMAL_CONSUME.original()(predator as *const u32, unit as *const i32) };
    }
    unsafe { call_entity_virtual(unit, ENTITY_VTABLE_REFRESH) };
    unsafe { ((unit + UNIT_SHOW_PREDATOR_FLAG_OFFSET) as *mut u8).write_volatile(0) };
    unsafe { set_done(done) };
    Some(true)
}

/// `fCaughtPrey`: the animal consumes its target (or the fallback prey vanilla looks up), the prey records
/// the hunter, and the prey refreshes. Always reports success.
pub unsafe fn f_caught_prey(unit: u32, done: *const bool) -> bool {
    if unit != 0 && unsafe { is_animal(unit) } {
        let mut prey = unsafe { ZTUNIT_GET_TARGET_UNIT.original()(unit as *const u32) } as u32;
        if prey == 0 {
            prey = unsafe { call_entity_virtual_ret(unit, ENTITY_VTABLE_FIND_PREY) };
            if prey == 0 {
                unsafe { call_entity_virtual(unit, ENTITY_VTABLE_NO_PREY) };
                unsafe { set_done(done) };
                return true;
            }
        }
        unsafe { ZTANIMAL_CONSUME.original()(unit as *const u32, prey as *const i32) };
        let hunter_id = get_from_memory::<u32>(unit + ENTITY_ID_OFFSET);
        unsafe { ((prey + UNIT_PREY_HUNTER_FIELD_OFFSET) as *mut u32).write_volatile(hunter_id) };
        unsafe { call_entity_virtual(prey, ENTITY_VTABLE_REFRESH) };
    }
    unsafe { set_done(done) };
    true
}

/// `fKillPrey`: removes the animal's target. `true` only when a target was killed.
pub unsafe fn f_kill_prey(unit: u32, done: *const bool) -> bool {
    let mut killed = false;
    if unit != 0 && unsafe { is_animal(unit) } {
        let prey = unsafe { ZTUNIT_GET_TARGET_UNIT.original()(unit as *const u32) } as u32;
        if prey != 0 {
            let vtable = get_from_memory::<u32>(prey);
            let target = get_from_memory::<u32>(vtable + ENTITY_VTABLE_HIDE_PREDATOR);
            let hide = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32)>(target) };
            hide(prey, 0);
            unsafe { ZTUNIT_SET_IS_REMOVED.original()(prey as *const u32, 1) };
            killed = true;
        }
    }
    unsafe { set_done(done) };
    killed
}

/// `fDie`: removes a dying animal, posts the death message at its tile and tells its habitat. `true` only
/// when the animal was removed here.
pub unsafe fn f_die(unit: u32, done: *const bool) -> bool {
    let mut removed = false;
    if unit != 0 && unsafe { is_animal(unit) } {
        if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
            unsafe { ZTUNIT_SET_IS_REMOVED.original()(unit as *const u32, 1) };
            let tile = unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) };
            unsafe { F_ZOO_MESSAGE_TILE_0.original()(unit as *const u32, DEATH_MESSAGE_ID, DEATH_MESSAGE_KIND, tile as u32, 0, 1) };
            removed = true;
        }
        let habitat = unsafe { call_entity_virtual_ret(unit, ENTITY_VTABLE_GET_HABITAT) };
        if habitat != 0 {
            unsafe { TRIGGER_DEATH_ARRIVED.hooked()(habitat as *const u32, 0) };
        }
    }
    unsafe { set_done(done) };
    removed
}

/// `fHatch`: finishes the animal's hatch. `false` (defer to vanilla) for a null/non-animal unit, where
/// vanilla calls `finishHatch` with a null `this`.
pub unsafe fn f_hatch(unit: u32, done: *const bool) -> bool {
    if !unsafe { is_animal(unit) } {
        return false;
    }
    unsafe { FINISH_HATCH.original()(unit as *const u32) };
    unsafe { set_done(done) };
    true
}

pub unsafe fn f_exit_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_EXIT_BUILDING) }
}

/// `fFaceTowardFood`: an animal faces its food; anything else just finishes. Always reports success.
pub unsafe fn f_face_toward_food(unit: u32, done: *const bool) -> bool {
    if unsafe { is_animal(unit) } {
        unsafe { FACE_TOWARD_FOOD.original()(unit as *const u32) };
    }
    unsafe { set_done(done) };
    true
}

/// `fFaceTowardTarget`: staff and guests face their target by position, other units face their target
/// unit; a non-unit just finishes. `false` (defer to vanilla) for a staff/guest target that is not a unit
/// type, where vanilla calls `faceTowardTarget` with a null `this`.
pub unsafe fn f_face_toward_target(unit: u32, done: *const bool) -> bool {
    let is_staff = unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTSTAFF) };
    let is_guest = unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUEST) };
    let is_unit = unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) };
    if !is_unit {
        if is_staff || is_guest {
            return false;
        }
    } else if is_staff || is_guest {
        unsafe { FACE_TOWARD_TARGET.original()(unit as *const u32) };
    } else {
        unsafe { FACE_TOWARD_TARGET_UNIT.original()(unit as *const u32) };
    }
    unsafe { set_done(done) };
    true
}

/// Function names `ZTAIMgr::executeCall` handles itself; every other name goes to `BFAIMgr::executeCall`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZtCall {
    MoveSlow,
    MoveMedium,
    MoveFast,
    Spray,
    KeeperRoutine,
    GuestRoutine,
    GuideRoutine,
    MaintRoutine,
    FaceTowardFood,
    FaceTowardTarget,
    ShowPredator,
    HidePredator,
    EnterBuilding,
    ExitBuilding,
    DustBall,
    KillPrey,
    CaughtPrey,
    Die,
    TargetBuilding,
    Hatch,
    EndTrick,
    ReturnToBuilding,
    PlaySetTank,
}

/// Exact, case-sensitive match of a call name against the 24 names the ZT chain tests.
pub fn lookup_zt_call(name: &[u8]) -> Option<ZtCall> {
    Some(match name {
        b"fWalk" | b"fSwim" => ZtCall::MoveSlow,
        b"fTrot" => ZtCall::MoveMedium,
        b"fRun" => ZtCall::MoveFast,
        b"fSpray" => ZtCall::Spray,
        b"fKeeperRoutine" => ZtCall::KeeperRoutine,
        b"fGuestRoutine" => ZtCall::GuestRoutine,
        b"fGuideRoutine" => ZtCall::GuideRoutine,
        b"fMaintRoutine" => ZtCall::MaintRoutine,
        b"fFaceTowardFood" => ZtCall::FaceTowardFood,
        b"fFaceTowardTarget" => ZtCall::FaceTowardTarget,
        b"fShowPredator" => ZtCall::ShowPredator,
        b"fHidePredator" => ZtCall::HidePredator,
        b"fEnterBuilding" => ZtCall::EnterBuilding,
        b"fExitBuilding" => ZtCall::ExitBuilding,
        b"fDustBall" => ZtCall::DustBall,
        b"fKillPrey" => ZtCall::KillPrey,
        b"fCaughtPrey" => ZtCall::CaughtPrey,
        b"fDie" => ZtCall::Die,
        b"fTargetBuilding" => ZtCall::TargetBuilding,
        b"fHatch" => ZtCall::Hatch,
        b"fEndTrick" => ZtCall::EndTrick,
        b"fReturnToBuilding" => ZtCall::ReturnToBuilding,
        b"fPlaySetTank" => ZtCall::PlaySetTank,
        _ => return None,
    })
}

/// Runs the handler for `which` the way the vanilla chain does. Handlers this crate has ported are reached
/// through their wrapper address (`hooked()`, what a vanilla caller would run), so their own deferral to
/// vanilla for faulting inputs still applies; the rest call the real wrappers.
unsafe fn dispatch_zt_call(this: u32, which: ZtCall, call: u32, unit: u32) {
    let (c, u) = (call as i32, unit as *const i32);
    unsafe {
        match which {
            ZtCall::MoveSlow => drop(F_MOVE_SLOW_F.original()(this as *const u32, c, u)),
            ZtCall::MoveMedium => drop(F_MOVE_MEDIUM_F.original()(this as *const u32, c, u)),
            ZtCall::MoveFast => drop(F_MOVE_FAST_F.original()(this as *const u32, c, u)),
            ZtCall::Spray => F_SPRAY_F.hooked()(c, unit as *const u32),
            ZtCall::KeeperRoutine => F_KEEPER_ROUTINE_F.hooked()(call, u),
            ZtCall::GuestRoutine => F_GUEST_ROUTINE_F.hooked()(call, unit),
            ZtCall::GuideRoutine => F_GUIDE_ROUTINE_F.hooked()(call, unit),
            ZtCall::MaintRoutine => F_MAINT_ROUTINE_F.hooked()(call, unit),
            ZtCall::FaceTowardFood => F_FACE_TOWARD_FOOD_F.hooked()(c, unit as *const u8),
            ZtCall::FaceTowardTarget => F_FACE_TOWARD_TARGET_F.hooked()(c, unit as *const c_void),
            ZtCall::ShowPredator => F_SHOW_PREDATOR_F.hooked()(c, u),
            ZtCall::HidePredator => F_HIDE_PREDATOR_F.hooked()(c, unit as *const u8),
            ZtCall::EnterBuilding => F_ENTER_BUILDING_F.hooked()(c, u),
            ZtCall::ExitBuilding => F_EXIT_BUILDING_F.hooked()(c, u),
            ZtCall::DustBall => F_DUST_BALL_F.original()(this as *const u32, c, u),
            ZtCall::KillPrey => F_KILL_PREY_F.hooked()(c, unit as *const u8),
            ZtCall::CaughtPrey => F_CAUGHT_PREY_F.hooked()(c, u),
            ZtCall::Die => F_DIE_F.hooked()(c, u),
            ZtCall::TargetBuilding => drop(F_TARGET_BUILDING_F.original()(c, u)),
            ZtCall::Hatch => F_HATCH_F.hooked()(c, u),
            ZtCall::EndTrick => F_END_TRICK_F.hooked()(c, u),
            ZtCall::ReturnToBuilding => F_RETURN_TO_BUILDING_F.hooked()(c, u),
            ZtCall::PlaySetTank => drop(F_PLAY_SET_TANK_F.original()(c, unit as *const c_void)),
        }
    }
}

/// `ZTAIMgr::executeCall`: dispatches the ZT-specific names; anything else is passed to `BFAIMgr`'s chain
/// (`fall_through`) with the uncast unit. Afterwards the call's result flag is copied to `done`.
pub unsafe fn execute_call(this: u32, call: u32, unit: u32, done: *mut u8, fall_through: impl FnOnce()) {
    let name = unsafe { &*(call as *const RawCallString) }.bytes();
    let unit_cast = if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } { unit } else { 0 };
    match lookup_zt_call(name) {
        Some(which) => unsafe { dispatch_zt_call(this, which, call, unit_cast) },
        None => fall_through(),
    }
    unsafe { done.write_volatile(get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET)) };
}

fn call_done_flag(call: u32) -> *const bool {
    (call + CALL_DONE_FLAG_OFFSET) as *const bool
}

#[detour_mod]
mod ztaimgr_detours {
    use super::*;

    // Behavior halves: `(unit, done)`.
    #[detour(F_END_TRICK)]
    unsafe extern "stdcall" fn f_end_trick_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_end_trick(unit as u32, done) }
    }

    #[detour(F_ENTER_BUILDING)]
    unsafe extern "stdcall" fn f_enter_building_detour(unit: *const u32, done: *const bool) {
        if !unsafe { f_enter_building(unit as u32, done) } {
            unsafe { F_ENTER_BUILDING_DETOUR.call(unit, done) };
        }
    }

    #[detour(F_RETURN_TO_BUILDING)]
    unsafe extern "stdcall" fn f_return_to_building_detour(unit: *const u32, done: *const bool) {
        if !unsafe { f_return_to_building(unit as u32, done) } {
            unsafe { F_RETURN_TO_BUILDING_DETOUR.call(unit, done) };
        }
    }

    #[detour(F_HIDE_PREDATOR)]
    unsafe extern "stdcall" fn f_hide_predator_detour(unit: *const u32, done: *const bool) -> bool {
        if unsafe { f_hide_predator(unit as u32, done) } {
            true
        } else {
            unsafe { F_HIDE_PREDATOR_DETOUR.call(unit, done) }
        }
    }

    #[detour(F_GUEST_ROUTINE)]
    unsafe extern "stdcall" fn f_guest_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_guest_routine(unit as u32, done) }
    }

    #[detour(F_MAINT_ROUTINE)]
    unsafe extern "stdcall" fn f_maint_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_maint_routine(unit as u32, done) }
    }

    #[detour(F_GUIDE_ROUTINE)]
    unsafe extern "stdcall" fn f_guide_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_guide_routine(unit as u32, done) }
    }

    #[detour(F_KEEPER_ROUTINE)]
    unsafe extern "stdcall" fn f_keeper_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_keeper_routine(unit as u32, done) }
    }

    // `F` wrappers: `(call, unit)`, with the `done` flag at `call + 0x18`.
    #[detour(F_END_TRICK_F)]
    unsafe extern "stdcall" fn f_end_trick_f_detour(call: i32, unit: *const i32) {
        unsafe { f_end_trick(unit as u32, call_done_flag(call as u32)) };
    }

    #[detour(F_ENTER_BUILDING_F)]
    unsafe extern "stdcall" fn f_enter_building_f_detour(call: i32, unit: *const i32) {
        if !unsafe { f_enter_building(unit as u32, call_done_flag(call as u32)) } {
            unsafe { F_ENTER_BUILDING_F_DETOUR.call(call, unit) };
        }
    }

    #[detour(F_RETURN_TO_BUILDING_F)]
    unsafe extern "stdcall" fn f_return_to_building_f_detour(call: i32, unit: *const i32) {
        if !unsafe { f_return_to_building(unit as u32, call_done_flag(call as u32)) } {
            unsafe { F_RETURN_TO_BUILDING_F_DETOUR.call(call, unit) };
        }
    }

    #[detour(F_HIDE_PREDATOR_F)]
    unsafe extern "stdcall" fn f_hide_predator_f_detour(call: i32, unit: *const u8) {
        if !unsafe { f_hide_predator(unit as u32, call_done_flag(call as u32)) } {
            unsafe { F_HIDE_PREDATOR_F_DETOUR.call(call, unit) };
        }
    }

    #[detour(F_GUEST_ROUTINE_F)]
    unsafe extern "stdcall" fn f_guest_routine_f_detour(call: u32, unit: u32) {
        unsafe { f_guest_routine(unit, call_done_flag(call)) };
    }

    #[detour(F_MAINT_ROUTINE_F)]
    unsafe extern "stdcall" fn f_maint_routine_f_detour(call: u32, unit: u32) {
        unsafe { f_maint_routine(unit, call_done_flag(call)) };
    }

    #[detour(F_GUIDE_ROUTINE_F)]
    unsafe extern "stdcall" fn f_guide_routine_f_detour(call: u32, unit: u32) {
        unsafe { f_guide_routine(unit, call_done_flag(call)) };
    }

    #[detour(F_KEEPER_ROUTINE_F)]
    unsafe extern "stdcall" fn f_keeper_routine_f_detour(call: u32, unit: *const i32) {
        unsafe { f_keeper_routine(unit as u32, call_done_flag(call)) };
    }

    // Prey/predator/death/hatch/exit handlers. Halves are `(unit, done)`; `F` wrappers are `(call, unit)`.
    #[detour(F_SHOW_PREDATOR)]
    unsafe extern "stdcall" fn f_show_predator_detour(unit: *const i32, done: *const u8) -> u32 {
        match unsafe { f_show_predator(unit as u32, done as *const bool) } {
            Some(result) => result as u32,
            None => unsafe { F_SHOW_PREDATOR_DETOUR.call(unit, done) },
        }
    }

    #[detour(F_SHOW_PREDATOR_F)]
    unsafe extern "stdcall" fn f_show_predator_f_detour(call: i32, unit: *const i32) {
        if unsafe { f_show_predator(unit as u32, call_done_flag(call as u32)) }.is_none() {
            unsafe { F_SHOW_PREDATOR_F_DETOUR.call(call, unit) };
        }
    }

    #[detour(F_CAUGHT_PREY)]
    unsafe extern "stdcall" fn f_caught_prey_detour(unit: *const i32, done: *const u8) -> u32 {
        unsafe { f_caught_prey(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_CAUGHT_PREY_F)]
    unsafe extern "stdcall" fn f_caught_prey_f_detour(call: i32, unit: *const i32) {
        unsafe { f_caught_prey(unit as u32, call_done_flag(call as u32)) };
    }

    #[detour(F_KILL_PREY)]
    unsafe extern "stdcall" fn f_kill_prey_detour(unit: *const u8, done: *const u8) -> u32 {
        unsafe { f_kill_prey(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_KILL_PREY_F)]
    unsafe extern "stdcall" fn f_kill_prey_f_detour(call: i32, unit: *const u8) {
        unsafe { f_kill_prey(unit as u32, call_done_flag(call as u32)) };
    }

    #[detour(F_DIE)]
    unsafe extern "stdcall" fn f_die_detour(unit: *const i32, done: *const u8) -> u32 {
        unsafe { f_die(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_DIE_F)]
    unsafe extern "stdcall" fn f_die_f_detour(call: i32, unit: *const i32) {
        unsafe { f_die(unit as u32, call_done_flag(call as u32)) };
    }

    #[detour(F_HATCH)]
    unsafe extern "stdcall" fn f_hatch_detour(unit: *const i32, done: *const u8) -> u32 {
        if unsafe { f_hatch(unit as u32, done as *const bool) } {
            1
        } else {
            unsafe { F_HATCH_DETOUR.call(unit, done) }
        }
    }

    #[detour(F_HATCH_F)]
    unsafe extern "stdcall" fn f_hatch_f_detour(call: i32, unit: *const i32) {
        if !unsafe { f_hatch(unit as u32, call_done_flag(call as u32)) } {
            unsafe { F_HATCH_F_DETOUR.call(call, unit) };
        }
    }

    #[detour(F_EXIT_BUILDING)]
    unsafe extern "stdcall" fn f_exit_building_detour(unit: *const i32, done: *const u8) {
        if !unsafe { f_exit_building(unit as u32, done as *const bool) } {
            unsafe { F_EXIT_BUILDING_DETOUR.call(unit, done) };
        }
    }

    #[detour(F_EXIT_BUILDING_F)]
    unsafe extern "stdcall" fn f_exit_building_f_detour(call: i32, unit: *const i32) {
        if !unsafe { f_exit_building(unit as u32, call_done_flag(call as u32)) } {
            unsafe { F_EXIT_BUILDING_F_DETOUR.call(call, unit) };
        }
    }

    #[detour(F_FACE_TOWARD_FOOD)]
    unsafe extern "stdcall" fn f_face_toward_food_detour(unit: *const u8, done: *const u8) -> u32 {
        unsafe { f_face_toward_food(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_FACE_TOWARD_FOOD_F)]
    unsafe extern "stdcall" fn f_face_toward_food_f_detour(call: i32, unit: *const u8) {
        unsafe { f_face_toward_food(unit as u32, call_done_flag(call as u32)) };
    }

    #[detour(F_FACE_TOWARD_TARGET)]
    unsafe extern "stdcall" fn f_face_toward_target_detour(unit: *const c_void, done: *const u8) -> u32 {
        if unsafe { f_face_toward_target(unit as u32, done as *const bool) } {
            1
        } else {
            unsafe { F_FACE_TOWARD_TARGET_DETOUR.call(unit, done) }
        }
    }

    #[detour(F_FACE_TOWARD_TARGET_F)]
    unsafe extern "stdcall" fn f_face_toward_target_f_detour(call: i32, unit: *const c_void) {
        if !unsafe { f_face_toward_target(unit as u32, call_done_flag(call as u32)) } {
            unsafe { F_FACE_TOWARD_TARGET_F_DETOUR.call(call, unit) };
        }
    }

    #[detour(EXECUTE_CALL)]
    unsafe extern "thiscall" fn execute_call_detour(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) {
        unsafe {
            execute_call(this as u32, call as u32, unit as u32, done as *mut u8, || {
                BFAIMGR_EXECUTE_CALL.original()(this, call, unit, done);
            })
        }
    }

    /// `fSprayF` only forwards to the `return 1` stub `BFAIMgr::fFaceDirection`: nothing to do.
    #[detour(F_SPRAY_F)]
    unsafe extern "stdcall" fn f_spray_f_detour(_call: i32, _unit: *const u32) {}

    #[detour(GET_RANDOM_X)]
    unsafe extern "thiscall" fn get_random_x_detour(this: *const u32, entity: *const u32) -> i32 {
        unsafe { get_random_x(this as u32, entity as u32) }
    }

    #[detour(GET_RANDOM_Y)]
    unsafe extern "thiscall" fn get_random_y_detour(this: *const u32, entity: *const u32) -> i32 {
        unsafe { get_random_y(this as u32, entity as u32) }
    }

    #[detour(IS_SLOW_MOVE_FUNCTION)]
    unsafe extern "thiscall" fn is_slow_move_function_detour(_this: *const u32, call: *const u32) -> bool {
        unsafe { is_slow_move_function(call) }
    }

    #[detour(IS_MEDIUM_MOVE_FUNCTION)]
    unsafe extern "thiscall" fn is_medium_move_function_detour(_this: *const u32, call: *const u32) -> bool {
        unsafe { is_medium_move_function(call) }
    }

    #[detour(IS_FAST_MOVE_FUNCTION)]
    unsafe extern "thiscall" fn is_fast_move_function_detour(_this: *const u32, call: *const u32) -> bool {
        unsafe { is_fast_move_function(call) }
    }

    #[detour(CONVERT_CONSTANT_PARAM)]
    unsafe extern "thiscall" fn convert_constant_param_detour(this: *const u32, name: *const c_void, entity: *const u32) -> i32 {
        match unsafe { convert_constant_param(name, entity as u32) } {
            Some(value) => value,
            None => unsafe { CONVERT_CONSTANT_PARAM_DETOUR.call(this, name, entity) },
        }
    }

    #[detour(REMOVE_ENTITY)]
    unsafe extern "thiscall" fn remove_entity_detour(_this: *const u32, entity: *const u32) -> bool {
        unsafe { remove_entity(entity as u32) }
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn get_random_x(this: *const u32, entity: *const u32) -> i32 {
            unsafe { super::GET_RANDOM_X_DETOUR.call(this, entity) }
        }

        pub(crate) fn get_random_y(this: *const u32, entity: *const u32) -> i32 {
            unsafe { super::GET_RANDOM_Y_DETOUR.call(this, entity) }
        }

        pub(crate) fn is_slow_move_function(this: *const u32, call: *const u32) -> bool {
            unsafe { super::IS_SLOW_MOVE_FUNCTION_DETOUR.call(this, call) }
        }

        pub(crate) fn is_medium_move_function(this: *const u32, call: *const u32) -> bool {
            unsafe { super::IS_MEDIUM_MOVE_FUNCTION_DETOUR.call(this, call) }
        }

        pub(crate) fn is_fast_move_function(this: *const u32, call: *const u32) -> bool {
            unsafe { super::IS_FAST_MOVE_FUNCTION_DETOUR.call(this, call) }
        }

        pub(crate) fn convert_constant_param(this: *const u32, name: *const c_void, entity: *const u32) -> i32 {
            unsafe { super::CONVERT_CONSTANT_PARAM_DETOUR.call(this, name, entity) }
        }

        pub(crate) fn f_end_trick(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_END_TRICK_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_enter_building(unit: *const u32, done: *const bool) {
            unsafe { super::F_ENTER_BUILDING_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_return_to_building(unit: *const u32, done: *const bool) {
            unsafe { super::F_RETURN_TO_BUILDING_DETOUR.call(unit, done) }
        }

        pub(crate) fn execute_call(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) {
            unsafe { super::EXECUTE_CALL_DETOUR.call(this, call, unit, done) }
        }

        pub(crate) fn f_exit_building(unit: *const i32, done: *const u8) {
            unsafe { super::F_EXIT_BUILDING_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_kill_prey(unit: *const u8, done: *const u8) -> u32 {
            unsafe { super::F_KILL_PREY_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_caught_prey(unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::F_CAUGHT_PREY_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_die(unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::F_DIE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_face_toward_food(unit: *const u8, done: *const u8) -> u32 {
            unsafe { super::F_FACE_TOWARD_FOOD_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_face_toward_target(unit: *const std::ffi::c_void, done: *const u8) -> u32 {
            unsafe { super::F_FACE_TOWARD_TARGET_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_guest_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_GUEST_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_maint_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_MAINT_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_guide_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_GUIDE_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_keeper_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_KEEPER_ROUTINE_DETOUR.call(unit, done) }
        }
    }
}

/// Installs the `ZTAIMgr` detours.
pub fn init() {
    if let Err(e) = unsafe { ztaimgr_detours::init_detours() } {
        error!("Failed to initialise ztaimgr detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use std::ffi::c_void;

    use super::*;

    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        ztaimgr_detours::status()
    }

    pub(crate) fn real_get_random_x(this: *const u32, entity: *const u32) -> i32 {
        ztaimgr_detours::test_real::get_random_x(this, entity)
    }

    pub(crate) fn real_get_random_y(this: *const u32, entity: *const u32) -> i32 {
        ztaimgr_detours::test_real::get_random_y(this, entity)
    }

    pub(crate) fn real_is_slow_move_function(this: *const u32, call: *const u32) -> bool {
        ztaimgr_detours::test_real::is_slow_move_function(this, call)
    }

    pub(crate) fn real_is_medium_move_function(this: *const u32, call: *const u32) -> bool {
        ztaimgr_detours::test_real::is_medium_move_function(this, call)
    }

    pub(crate) fn real_is_fast_move_function(this: *const u32, call: *const u32) -> bool {
        ztaimgr_detours::test_real::is_fast_move_function(this, call)
    }

    pub(crate) fn real_convert_constant_param(this: *const u32, name: *const c_void, entity: *const u32) -> i32 {
        ztaimgr_detours::test_real::convert_constant_param(this, name, entity)
    }

    pub(crate) use super::{f_enter_building, f_exit_building, f_hatch, f_return_to_building, f_show_predator};
    pub(crate) use ztaimgr_detours::test_real::{
        execute_call as real_execute_call, f_caught_prey as real_f_caught_prey, f_die as real_f_die, f_face_toward_food as real_f_face_toward_food, f_face_toward_target as real_f_face_toward_target, f_exit_building as real_f_exit_building, f_kill_prey as real_f_kill_prey,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        f_end_trick as real_f_end_trick, f_enter_building as real_f_enter_building, f_guest_routine as real_f_guest_routine, f_guide_routine as real_f_guide_routine,
        f_keeper_routine as real_f_keeper_routine, f_maint_routine as real_f_maint_routine, f_return_to_building as real_f_return_to_building,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_of(bytes: &'static [u8]) -> RawCallString {
        let range = bytes.as_ptr_range();
        RawCallString { begin: range.start, end: range.end, capacity: range.end }
    }

    fn call_with(name: &'static [u8], params: &[RawCallString]) -> RawFunctionCall {
        let range = params.as_ptr_range();
        RawFunctionCall { name: string_of(name), params_begin: range.start, params_end: range.end, params_capacity: range.end }
    }

    #[test]
    fn zt_call_lookup_covers_every_name_and_nothing_near() {
        let names: [(&[u8], ZtCall); 23] = [
            (b"fWalk", ZtCall::MoveSlow),
            (b"fSwim", ZtCall::MoveSlow),
            (b"fTrot", ZtCall::MoveMedium),
            (b"fRun", ZtCall::MoveFast),
            (b"fSpray", ZtCall::Spray),
            (b"fKeeperRoutine", ZtCall::KeeperRoutine),
            (b"fGuestRoutine", ZtCall::GuestRoutine),
            (b"fGuideRoutine", ZtCall::GuideRoutine),
            (b"fMaintRoutine", ZtCall::MaintRoutine),
            (b"fFaceTowardFood", ZtCall::FaceTowardFood),
            (b"fFaceTowardTarget", ZtCall::FaceTowardTarget),
            (b"fShowPredator", ZtCall::ShowPredator),
            (b"fHidePredator", ZtCall::HidePredator),
            (b"fEnterBuilding", ZtCall::EnterBuilding),
            (b"fExitBuilding", ZtCall::ExitBuilding),
            (b"fDustBall", ZtCall::DustBall),
            (b"fKillPrey", ZtCall::KillPrey),
            (b"fCaughtPrey", ZtCall::CaughtPrey),
            (b"fDie", ZtCall::Die),
            (b"fTargetBuilding", ZtCall::TargetBuilding),
            (b"fHatch", ZtCall::Hatch),
            (b"fEndTrick", ZtCall::EndTrick),
            (b"fReturnToBuilding", ZtCall::ReturnToBuilding),
        ];
        for (name, expected) in names {
            assert_eq!(lookup_zt_call(name), Some(expected), "{}", String::from_utf8_lossy(name));
            let mut longer = name.to_vec();
            longer.push(b'x');
            assert_eq!(lookup_zt_call(&longer), None);
            assert_eq!(lookup_zt_call(&name[..name.len() - 1]), None);
            assert_eq!(lookup_zt_call(&name.to_ascii_lowercase()), None);
        }
        assert_eq!(lookup_zt_call(b"fPlaySetTank"), Some(ZtCall::PlaySetTank));
        for name in [&b""[..], b"fMove", b"fPlay", b"fHalt", b"fMoveSlow", b"fWalkx"] {
            assert_eq!(lookup_zt_call(name), None);
        }
    }

    #[test]
    fn move_function_names() {
        let none: [RawCallString; 0] = [];
        let call = |name: &'static [u8]| call_with(name, &none);
        assert!(call(b"fMoveSlow").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
        assert!(call(b"fWalk").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
        assert!(!call(b"fRun").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
        assert!(!call(b"fMove").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
    }

    #[test]
    fn plain_move_matches_on_third_parameter() {
        let params = [string_of(b"x"), string_of(b"y"), string_of(b"trot")];
        let call = call_with(b"fMove", &params);
        assert!(call.is_move_function(b"fMoveMedium", &[b"fTrot"], b"trot"));
        assert!(!call.is_move_function(b"fMoveFast", &[b"fRun"], b"run"));
        let short = call_with(b"fMove", &params[..2]);
        assert!(!short.is_move_function(b"fMoveMedium", &[b"fTrot"], b"trot"));
    }
}
