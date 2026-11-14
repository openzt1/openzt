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
    bfaimgr::{
        CALL_DONE as BFAIMGR_CALL_DONE, CHECK_PATH as BFAIMGR_CHECK_PATH, GET_EVENTS as BFAIMGR_GET_EVENTS, CONSTRUCTOR as BFAIMGR_CONSTRUCTOR, F_PLAY_SET, GET_PATH_1 as BFAIMGR_GET_PATH_1,
        LOAD as BFAIMGR_LOAD, LOAD_DATA as BFAIMGR_LOAD_DATA, REMOVE_ENTITIES as BFAIMGR_REMOVE_ENTITIES, SAVE as BFAIMGR_SAVE, SEND_EVENT as BFAIMGR_SEND_EVENT, UPDATE as BFAIMGR_UPDATE,
    },
    bfinifile::READ as BFINIFILE_READ,
    gxmixer::GET_ANIM as GXMIXER_GET_ANIM,
    bfpathfinder::SET_NODE_MAX,
    standalone::OPERATOR_NEW,
    ztaimgr::{CREATE as ZTAIMGR_CREATE, INIT_AIPARAMS as ZTAIMGR_INIT_AIPARAMS},
    bfunit::SET_MOVING,
    bfeventmgr::{GET_EVENTS as BFEVENTMGR_GET_EVENTS, LOAD as BFEVENTMGR_LOAD, POST_EVENT as BFEVENTMGR_POST_EVENT, SAVE as BFEVENTMGR_SAVE, UPDATE as BFEVENTMGR_UPDATE},
    standalone::{DEALLOCATE as READ_BYTES_FROM_FILE, WRITE_BYTES_TO_FILE},
    ztaimgr::UPDATE as ZTAIMGR_UPDATE,
    ztmainttaskpool::LISTEN as ZTMAINTTASKPOOL_LISTEN,
    bfaimgr::{
        EXECUTE_CALL as BFAIMGR_EXECUTE_CALL, F_FACE_DIRECTION_F, F_FLY_F, F_FLY_REVERSE_F, F_HALT_F, F_PLAY_F, F_PLAY_WITH_SOUND, F_PLAY_SOUND, F_MOVE_ENTITY_DELTA_F, F_MOVE_ENTITY_SPECIAL_F, F_MOVE_F, F_MOVE_REVERSE_F,
        F_MOVE_FAST_F, F_MOVE_MEDIUM_F, F_MOVE_SLOW_F, F_MOVE_WITH_SOUND_F, F_PLAY_PING_PONG_F, F_PLAY_REVERSE_F, F_PLAY_SET_F, F_PLAY_SET_PROB_F, F_PLAY_SOUND_F,
        F_PLAY_SPECIAL_F, F_PLAY_STOP_AT_END_F, F_PLAY_TIME_F, F_PLAY_WITH_BASE_F, F_PLAY_WITH_SOUND_F, F_SET_BASE_F,
    },
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
    util::{get_from_memory, low_byte_bool, save_to_memory},
    vanilla_string::VanillaString,
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
    let remove = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, bool) -> u32>(target) };
    low_byte_bool(remove(world, entity, true))
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
            ZtCall::PlaySetTank => {
                if f_play_set_tank(call, unit, play_set_vanilla).is_none() {
                    let _ = F_PLAY_SET_TANK_F.original()(c, unit as *const c_void);
                }
            }
        }
    }
}

/// `ZTAIMgr::executeCall`: dispatches the ZT-specific names; anything else goes to the `BFAIMgr` chain
/// ([`execute_base_call`]) with the uncast unit. Afterwards the call's result flag is copied to `done`.
pub unsafe fn execute_call(this: u32, call: u32, unit: u32, done: *mut u8) {
    let name = unsafe { &*(call as *const RawCallString) }.bytes();
    let unit_cast = if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } { unit } else { 0 };
    match lookup_zt_call(name) {
        Some(which) => unsafe { dispatch_zt_call(this, which, call, unit_cast) },
        None => {
            unsafe { execute_base_call(this, call, unit, done) };
        }
    }
    unsafe { done.write_volatile(get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET)) };
}

/// Function names `BFAIMgr::executeCall` handles. Two literals alias `fPlaySpecial` (`fPlaySp`), `fPlayPingPong`
/// (`fPingPong`) and `fPlayWithSound` (`fPlaySFX`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BaseCall {
    Play,
    SetBase,
    PlayWithBase,
    MoveEntityDelta,
    MoveEntitySpecial,
    PlayTime,
    PlaySound,
    PlaySpecial,
    PlaySetProb,
    PlaySetTerrain,
    PlayReverse,
    PlayPingPong,
    PlayWithSound,
    PlayStopAtEnd,
    PlaySet,
    MoveSlow,
    MoveMedium,
    Move,
    MoveReverse,
    MoveFast,
    MoveWithSound,
    FaceDirection,
    Fly,
    FlyReverse,
    KeepControl,
    Halt,
}

/// Exact, case-sensitive match of a call name against the names the base chain tests.
pub fn lookup_base_call(name: &[u8]) -> Option<BaseCall> {
    Some(match name {
        b"fPlay" => BaseCall::Play,
        b"fSetBase" => BaseCall::SetBase,
        b"fPlayWithBase" => BaseCall::PlayWithBase,
        b"fMoveEntityDelta" => BaseCall::MoveEntityDelta,
        b"fMoveEntitySpecial" => BaseCall::MoveEntitySpecial,
        b"fPlayTime" => BaseCall::PlayTime,
        b"fPlaySound" => BaseCall::PlaySound,
        b"fPlaySpecial" | b"fPlaySp" => BaseCall::PlaySpecial,
        b"fPlaySetProb" => BaseCall::PlaySetProb,
        b"fPlaySetTerrain" => BaseCall::PlaySetTerrain,
        b"fPlayReverse" => BaseCall::PlayReverse,
        b"fPlayPingPong" | b"fPingPong" => BaseCall::PlayPingPong,
        b"fPlayWithSound" | b"fPlaySFX" => BaseCall::PlayWithSound,
        b"fPlayStopAtEnd" => BaseCall::PlayStopAtEnd,
        b"fPlaySet" => BaseCall::PlaySet,
        b"fMoveSlow" => BaseCall::MoveSlow,
        b"fMoveMedium" => BaseCall::MoveMedium,
        b"fMove" => BaseCall::Move,
        b"fMoveReverse" => BaseCall::MoveReverse,
        b"fMoveFast" => BaseCall::MoveFast,
        b"fMoveWithSound" => BaseCall::MoveWithSound,
        b"fFaceDirection" => BaseCall::FaceDirection,
        b"fFly" => BaseCall::Fly,
        b"fFlyReverse" => BaseCall::FlyReverse,
        b"fKeepControl" => BaseCall::KeepControl,
        b"fHalt" => BaseCall::Halt,
        _ => return None,
    })
}

/// `BFAIMgr` vtable slot `fPlaySetTerrainF(call, unit)`, which `ZTAIMgr` overrides.
const BFAIMGR_VTABLE_PLAY_SET_TERRAIN: u32 = 0x14;

/// `BFAIMgr::executeCall`: dispatches a call by name and returns the low byte of the handler's result (`0`
/// for an unknown name). The play family receives the unit as given; move/fly/halt handlers get the unit
/// only if its type is a `ZTUnitType`, else null. The call's result flag is copied to `done`.
pub unsafe fn execute_base_call(this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    let name = unsafe { &*(call as *const RawCallString) }.bytes();
    let unit_cast = if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } { unit } else { 0 };
    let result = match lookup_base_call(name) {
        Some(which) => unsafe { dispatch_base_call(this, which, call, unit, unit_cast) },
        None => 0,
    };
    unsafe { done.write_volatile(get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET)) };
    result
}

unsafe fn dispatch_base_call(this: u32, which: BaseCall, call: u32, unit: u32, unit_cast: u32) -> u8 {
    let (t, c) = (this as *const u32, call as *const u32);
    let (u, uc) = (unit as *const u32, unit_cast as *const u32);
    unsafe {
        match which {
            BaseCall::Play => F_PLAY_F.original()(t, c, u, 0, 0) as u8,
            BaseCall::SetBase => F_SET_BASE_F.original()(t, c, u) as u8,
            BaseCall::PlayWithBase => F_PLAY_WITH_BASE_F.original()(t, c, u, 0, 0) as u8,
            BaseCall::MoveEntityDelta => F_MOVE_ENTITY_DELTA_F.original()(t, c, u) as u8,
            BaseCall::MoveEntitySpecial => F_MOVE_ENTITY_SPECIAL_F.original()(t, c, u) as u8,
            BaseCall::PlayTime => F_PLAY_TIME_F.hooked()(t, c, u) as u8,
            BaseCall::PlaySound => F_PLAY_SOUND_F.hooked()(t, c, u) as u8,
            BaseCall::PlaySpecial => F_PLAY_SPECIAL_F.original()(t, c, u) as u8,
            BaseCall::PlaySetProb => F_PLAY_SET_PROB_F.original()(c, u) as u8,
            BaseCall::PlaySetTerrain => {
                let vtable = get_from_memory::<u32>(this);
                let target = get_from_memory::<u32>(vtable + BFAIMGR_VTABLE_PLAY_SET_TERRAIN);
                mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target)(this, call, unit) as u8
            }
            BaseCall::PlayReverse => F_PLAY_REVERSE_F.hooked()(t, c, u) as u8,
            BaseCall::PlayPingPong => F_PLAY_PING_PONG_F.hooked()(t, c, u) as u8,
            BaseCall::PlayWithSound => F_PLAY_WITH_SOUND_F.hooked()(c, u) as u8,
            BaseCall::PlayStopAtEnd => F_PLAY_STOP_AT_END_F.hooked()(t, c, u) as u8,
            BaseCall::PlaySet => F_PLAY_SET_F.hooked()(c, u) as u8,
            BaseCall::MoveSlow => F_MOVE_SLOW_F.original()(t, call as i32, uc as *const i32) as u8,
            BaseCall::MoveMedium => F_MOVE_MEDIUM_F.original()(t, call as i32, uc as *const i32) as u8,
            BaseCall::Move => F_MOVE_F.original()(t, c, uc, false) as u8,
            BaseCall::MoveReverse => {
                let reverse = mem::transmute::<u32, unsafe extern "thiscall" fn(*const u32, *const u32, *const u32) -> u32>(F_MOVE_REVERSE_F.address);
                reverse(t, c, uc) as u8
            }
            BaseCall::MoveFast => F_MOVE_FAST_F.original()(t, call as i32, uc as *const i32) as u8,
            BaseCall::MoveWithSound => F_MOVE_WITH_SOUND_F.original()(t, c, uc) as u8,
            BaseCall::FaceDirection => F_FACE_DIRECTION_F.hooked()(c, u) as u8,
            BaseCall::Fly => F_FLY_F.original()(t, c, uc, 0) as u8,
            BaseCall::FlyReverse => F_FLY_REVERSE_F.hooked()(t, c, uc) as u8,
            BaseCall::KeepControl => (unit_cast != 0) as u8,
            BaseCall::Halt => match f_halt_f(call, unit_cast) {
                Some(result) => result,
                None => {
                    let halt = mem::transmute::<u32, unsafe extern "stdcall" fn(*const u32, *const u32) -> u32>(F_HALT_F.address);
                    halt(c, uc) as u8
                }
            },
        }
    }
}

fn call_done_flag(call: u32) -> *const bool {
    (call + CALL_DONE_FLAG_OFFSET) as *const bool
}

/// `BFAIMgr` tick accumulator remainder (`+0xe8`), tick counter (`+0xec`) and divisor (`+0xe4`).
const OFFSET_AI_TICK_REMAINDER: u32 = 0xe8;
const OFFSET_AI_TICK_COUNT: u32 = 0xec;
const OFFSET_AI_TICK_DIVISOR: u32 = 0xe4;
/// `BFAIMgr`'s embedded `BFEventMgr` (`+0xa0`) and the path-cache tree container (`{head*, size}` at `+0x12c`).
const OFFSET_EVENT_MGR: u32 = 0xa0;
const OFFSET_PATH_CACHE: u32 = 0x12c;
/// Pool size class of the path-cache tree nodes.
const PATH_CACHE_NODE_SIZE: u32 = 0x18;
/// Globals (RVAs): next tick at which the path cache is flushed, and the flush interval in ticks.
const RVA_PATH_CACHE_NEXT_FLUSH: u32 = 0x0023_94b4;
const RVA_PATH_CACHE_FLUSH_INTERVAL: u32 = 0x0023_9028;
/// `ZTMaintTaskPool` instance (`DAT_00638f99 + 7`) that `ZTAIMgr::update` pumps.
const RVA_MAINT_TASK_POOL: u32 = 0x0023_8fa0;
/// `BFAIMgr` vtable slot `removeEntities`, `BFEntity` vtable slot `update(dt)`, `ZTShowMgr` vtable slot `update`.
const AI_VTABLE_REMOVE_ENTITIES: u32 = 0x40;
const ENTITY_VTABLE_UPDATE: u32 = 0x28;
const SHOW_MGR_VTABLE_UPDATE: u32 = 0x4;
/// `BFWorldMgr` entity vector begin/end pointers.
const WORLD_ENTITY_BEGIN: u32 = 0x80;
const WORLD_ENTITY_END: u32 = 0x84;

/// RVAs of the `GLOBAL_ZTWorldMgr` and `GLOBAL_ZTShowMgr` pointer cells. Read on every call, as vanilla
/// does, so the live tests can point them at fakes for one call.
const RVA_GLOBAL_WORLD_MGR: u32 = 0x0023_8040;
const RVA_GLOBAL_SHOW_MGR: u32 = 0x0023_9008;

fn read_global_pointer(rva: u32) -> u32 {
    get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + rva)
}

/// `BFAIMgr::update(dt)`: advances the tick accumulator, flushes the path cache on its interval, removes
/// flagged entities, ticks every entity and pumps the event manager. A zero divisor is a divide fault in
/// vanilla; here it skips the tick instead.
pub unsafe fn bfaimgr_update(this: u32, dt: u32) {
    let divisor = get_from_memory::<u32>(this + OFFSET_AI_TICK_DIVISOR);
    if divisor == 0 {
        return;
    }
    let base = get_module_base("zoo.exe") as u32;
    let total = dt.wrapping_add(get_from_memory::<u32>(this + OFFSET_AI_TICK_REMAINDER));
    save_to_memory(this + OFFSET_AI_TICK_REMAINDER, total % divisor);
    let ticks = get_from_memory::<u32>(this + OFFSET_AI_TICK_COUNT).wrapping_add(total / divisor);
    save_to_memory(this + OFFSET_AI_TICK_COUNT, ticks);
    if ticks >= get_from_memory::<u32>(base + RVA_PATH_CACHE_NEXT_FLUSH) {
        crate::vanilla_rbtree::rb_tree_clear(this + OFFSET_PATH_CACHE, PATH_CACHE_NODE_SIZE);
        let interval = get_from_memory::<u32>(base + RVA_PATH_CACHE_FLUSH_INTERVAL);
        save_to_memory(base + RVA_PATH_CACHE_NEXT_FLUSH, get_from_memory::<u32>(this + OFFSET_AI_TICK_COUNT).wrapping_add(interval));
    }
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    unsafe { call_entity_virtual(this, AI_VTABLE_REMOVE_ENTITIES) };
    let count = (get_from_memory::<u32>(world + WORLD_ENTITY_END).wrapping_sub(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN)) as i32) >> 2;
    for index in 0..count.max(0) as u32 {
        let entity = get_from_memory::<u32>(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN) + index * 4);
        if entity != 0 {
            let vtable = get_from_memory::<u32>(entity);
            let target = get_from_memory::<u32>(vtable + ENTITY_VTABLE_UPDATE);
            let update = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32)>(target) };
            update(entity, dt);
        }
    }
    unsafe { BFEVENTMGR_UPDATE.original()((this + OFFSET_EVENT_MGR) as *const u32) };
}

/// `ZTAIMgr::update(dt)`: pumps the maintenance task pool and the show manager, then the base update.
pub unsafe fn ztaimgr_update(this: u32, dt: u32) {
    let base = get_module_base("zoo.exe") as u32;
    unsafe { ZTMAINTTASKPOOL_LISTEN.original()((base + RVA_MAINT_TASK_POOL) as *const i32) };
    let show_mgr = read_global_pointer(RVA_GLOBAL_SHOW_MGR);
    unsafe { call_entity_virtual(show_mgr, SHOW_MGR_VTABLE_UPDATE) };
    unsafe { bfaimgr_update(this, dt) };
}

/// `BFAIMgr::save(file)`: writes the tick remainder and counter, then the event manager. Every write is
/// attempted; the result is true only if all succeed.
pub unsafe fn bfaimgr_save(this: u32, file: *const i8) -> bool {
    let write = |offset: u32| unsafe { WRITE_BYTES_TO_FILE.hooked()((this + offset) as *const u32, 4, 1, file) == 1 };
    let first = write(OFFSET_AI_TICK_REMAINDER);
    let second = write(OFFSET_AI_TICK_COUNT);
    let events = unsafe { BFEVENTMGR_SAVE.hooked()((this + OFFSET_EVENT_MGR) as *const u32, file) };
    events && first && second
}

/// `BFWorldMgr` habitat-pointer vector begin/end, and the `BFWorldMgr` vtable slot run first when loading.
const WORLD_HABITAT_BEGIN: u32 = 0x98;
const WORLD_HABITAT_END: u32 = 0x9c;
const WORLD_VTABLE_PRE_LOAD: u32 = 0x4;
/// Habitat vtable slots: a check returning a success byte, then a no-argument finish step.
const HABITAT_VTABLE_CHECK: u32 = 0x10;
const HABITAT_VTABLE_FINISH: u32 = 0x58;
/// Entity vtable slots run for each entity on load: `(0, 0)` reset, no-argument refresh, `(1, 0, 1)` unit fixup.
const ENTITY_VTABLE_RESET: u32 = 0x110;
const ENTITY_VTABLE_LOAD_REFRESH: u32 = 0x10c;
const ENTITY_VTABLE_UNIT_FIXUP: u32 = 0x198;
/// Entity fields touched on a full load: a byte vector's begin/end (`+0xdc`/`+0xe0`) and a flag byte (`+0xe8`).
const OFFSET_ENTITY_VECTOR_BEGIN: u32 = 0xdc;
const OFFSET_ENTITY_VECTOR_END: u32 = 0xe0;
const OFFSET_ENTITY_FLAG: u32 = 0xe8;

/// Calls the thiscall virtual at `slot` of `object`s vtable with `N` (at most three) `u32` arguments.
unsafe fn call_virtual_args<const N: usize>(object: u32, slot: u32, args: [u32; N]) -> u32 {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(object) + slot);
    unsafe {
        match N {
            0 => mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target)(object),
            1 => mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(target)(object, args[0]),
            2 => mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target)(object, args[0], args[1]),
            _ => mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32) -> u32>(target)(object, args[0], args[1], args[2]),
        }
    }
}

/// `BFAIMgr::loadData(full)`: post-load pass over the world. When `full`, runs the world managers pre-load
/// step first. Every habitat is checked (result ANDed over the low bytes) and finished, then every entity is
/// reset and, on a full load, has its byte vector truncated, its flag set, is refreshed, and gets the unit
/// fixup if its type is a `ZTUnitType`. The `this` pointer is unused.
pub unsafe fn bfaimgr_load_data(full: bool) -> bool {
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    let mut ok = true;
    if full {
        unsafe { call_virtual_args(world, WORLD_VTABLE_PRE_LOAD, [0]) };
    }
    let mut cursor = get_from_memory::<u32>(world + WORLD_HABITAT_BEGIN);
    while cursor != get_from_memory::<u32>(world + WORLD_HABITAT_END) {
        let habitat = get_from_memory::<u32>(cursor);
        if habitat != 0 {
            ok &= unsafe { call_virtual_args(habitat, HABITAT_VTABLE_CHECK, []) } & 0xff != 0;
        }
        cursor += 4;
    }
    cursor = get_from_memory::<u32>(world + WORLD_HABITAT_BEGIN);
    while cursor != get_from_memory::<u32>(world + WORLD_HABITAT_END) {
        unsafe { call_virtual_args(get_from_memory::<u32>(cursor), HABITAT_VTABLE_FINISH, []) };
        cursor += 4;
    }
    cursor = get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN);
    while cursor != get_from_memory::<u32>(world + WORLD_ENTITY_END) {
        let entity = get_from_memory::<u32>(cursor);
        if entity != 0 {
            unsafe { call_virtual_args(entity, ENTITY_VTABLE_RESET, [0, 0]) };
            if full {
                let begin = get_from_memory::<u32>(entity + OFFSET_ENTITY_VECTOR_BEGIN);
                if begin != get_from_memory::<u32>(entity + OFFSET_ENTITY_VECTOR_END) {
                    save_to_memory(begin, 0u8);
                    save_to_memory(entity + OFFSET_ENTITY_VECTOR_END, begin);
                }
                save_to_memory(entity + OFFSET_ENTITY_FLAG, 1u8);
                unsafe { call_virtual_args(entity, ENTITY_VTABLE_LOAD_REFRESH, []) };
                let entity_type = get_from_memory::<u32>(entity + OFFSET_ENTITY_TYPE);
                let tag = get_module_base("zoo.exe") as u32 + RVA_TAG_ZTUNITTYPE;
                if unsafe { call_virtual_args(entity_type, TYPE_VTABLE_IS_CAST_CLASS, [tag]) } & 0xff != 0 {
                    unsafe { call_virtual_args(entity, ENTITY_VTABLE_UNIT_FIXUP, [1, 0, 1]) };
                }
            }
        }
        cursor += 4;
    }
    ok
}

/// Allocation size of a `ZTAIMgr`, its vtable and `BFEventListenerInterface` table (RVAs), and the vtable slot
/// of `initAIParams`.
const ZTAIMGR_SIZE: u32 = 0x150;
const RVA_ZTAIMGR_VTABLE: u32 = 0x0023_5324;
const RVA_ZTAIMGR_LISTENER_TABLE: u32 = 0x0023_2288;
const AI_VTABLE_INIT_AI_PARAMS: u32 = 0x38;

/// `ZTAIMgr::create()`: allocates the `0x150`-byte manager through vanilla `operator new` (vanilla frees it
/// with the matching delete), runs the `BFAIMgr` constructor, installs the `ZTAIMgr` vtable and listener
/// table, and runs `initAIParams` through the vtable. Returns the manager, or `0` if allocation failed.
pub unsafe fn ztaimgr_create() -> u32 {
    let this = unsafe { OPERATOR_NEW.original()(ZTAIMGR_SIZE) } as u32;
    if this == 0 {
        return 0;
    }
    unsafe { BFAIMGR_CONSTRUCTOR.original()(this as *const u32) };
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(this, base + RVA_ZTAIMGR_VTABLE);
    save_to_memory(this + 8, base + RVA_ZTAIMGR_LISTENER_TABLE);
    unsafe { call_virtual_args(this, AI_VTABLE_INIT_AI_PARAMS, []) };
    this
}

/// `BFAIMgr`'s embedded `BFPathFinder` (`+0x10`): the map pointer, map row count and tile count it is pointed at
/// before a search, and its search-node limit (`+0x4c` of the manager is the limit `checkPath` restores).
const OFFSET_PATHFINDER: u32 = 0x10;
const PATHFINDER_MAP: u32 = 0x8;
const PATHFINDER_MAP_ROWS: u32 = 0xc;
const PATHFINDER_TILE_COUNT: u32 = 0x10;
const OFFSET_PATH_NODE_LIMIT: u32 = 0x4c;
/// The pathfinder's result list head pointer (`+0x70` of the pathfinder, `+0x80` of the manager).
const OFFSET_PATH_RESULT: u32 = 0x80;
/// Node limit `checkPath` searches with.
const CHECK_PATH_NODE_LIMIT: u32 = 100;
/// `BFWorldMgr` map block (`+0x8`) and its column and row counts (`+0x2c`/`+0x30` into it).
const WORLD_MAP: u32 = 0x8;
const MAP_COLUMNS: u32 = 0x2c;
const MAP_ROWS: u32 = 0x30;
/// Offset of the position inside a `BFTile`, and the `BFPathFinder` vtable slot `findPath`.
const TILE_POSITION: u32 = 0x34;
const PATHFINDER_VTABLE_FIND_PATH: u32 = 0x0;

/// `BFAIMgr::checkPath(from, to, unit)`: whether a path exists. Points the pathfinder at the world map, runs a
/// 100-node `findPath` (virtual) between the two tile positions, restores the node limit and reports whether the
/// pathfinder's result list is non-empty. Vanilla copies the result list into a temporary pool list before
/// counting it, then frees the copy; the copy has no other effect, so the port tests the source list directly.
/// The result is the low byte only (`AL`), like the vanilla function.
pub unsafe fn bfaimgr_check_path(this: u32, from: u32, to: u32, unit: u32) -> u32 {
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    let finder = this + OFFSET_PATHFINDER;
    let map = world.wrapping_add(WORLD_MAP);
    if map != 0 {
        save_to_memory(finder + PATHFINDER_MAP, map);
        let rows = get_from_memory::<u32>(map + MAP_ROWS);
        save_to_memory(finder + PATHFINDER_MAP_ROWS, rows);
        let columns = get_from_memory::<u32>(map + MAP_COLUMNS);
        save_to_memory(finder + PATHFINDER_TILE_COUNT, columns.wrapping_mul(rows));
    }
    let saved_limit = get_from_memory::<u32>(this + OFFSET_PATH_NODE_LIMIT);
    unsafe { SET_NODE_MAX.hooked()(finder as *const u32, CHECK_PATH_NODE_LIMIT) };
    let find_path_target = get_from_memory::<u32>(get_from_memory::<u32>(finder) + PATHFINDER_VTABLE_FIND_PATH);
    let find_path = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32>(find_path_target) };
    find_path(finder, from + TILE_POSITION, to + TILE_POSITION, unit, 0);
    unsafe { SET_NODE_MAX.hooked()(finder as *const u32, saved_limit) };
    let list = get_from_memory::<u32>(this + OFFSET_PATH_RESULT);
    u32::from(get_from_memory::<u32>(list) != list)
}

/// `BFEntity` fields `callDone` reads: the flag byte at `+0xf8`, the embedded `GXMixer` at `+0x74`, and inside the
/// mixer the flag byte at `+0x60` and the two dwords at `+0x28`/`+0x18` that `callDone` compares.
const OFFSET_ENTITY_CALL_DONE_FLAG: u32 = 0xf8;
const OFFSET_ENTITY_MIXER: u32 = 0x74;
const MIXER_FLAG: u32 = 0x60;
const MIXER_CURSOR: u32 = 0x28;
const MIXER_END: u32 = 0x18;

/// `BFAIMgr::callDone(entity)`: true when the entity is flagged and its animation mixer has run out of
/// animations (the cursor equals its end, or `GXMixer::getAnim` finds none).
pub unsafe fn bfaimgr_call_done(entity: u32) -> bool {
    if entity == 0 || get_from_memory::<u8>(entity + OFFSET_ENTITY_CALL_DONE_FLAG) == 0 {
        return false;
    }
    let mixer = entity + OFFSET_ENTITY_MIXER;
    if get_from_memory::<u8>(mixer + MIXER_FLAG) == 0 {
        return false;
    }
    if get_from_memory::<u32>(mixer + MIXER_CURSOR) == get_from_memory::<u32>(mixer + MIXER_END) {
        return true;
    }
    unsafe { GXMIXER_GET_ANIM.hooked()(mixer as *const u32) }.is_null()
}

/// Entity vtable slots `removeEntities` tests and clears: `isRemoved`, `isRemovedUndo` and `setIsRemoved(a, b)`.
const ENTITY_VTABLE_IS_REMOVED: u32 = 0xac;
const ENTITY_VTABLE_IS_REMOVED_UNDO: u32 = 0xb0;
const ENTITY_VTABLE_SET_IS_REMOVED: u32 = 0xa8;
/// `BFAIMgr` vtable slot `removeEntity(entity)`.
const AI_VTABLE_REMOVE_ENTITY: u32 = 0x44;

/// `BFAIMgr::removeEntities()`: sweeps the world's entity list. An entity flagged removed is handed to the
/// manager's `removeEntity` (which takes it out of the list, so the index and count step back), unless it is
/// flagged undo-removed, in which case the world removes it without destroying it and the flag is cleared.
/// The count is taken once; the list pointer is re-read for every entity.
pub unsafe fn bfaimgr_remove_entities(this: u32) {
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    let mut count = (get_from_memory::<u32>(world + WORLD_ENTITY_END).wrapping_sub(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN)) as i32) >> 2;
    let mut index: i32 = 0;
    if count <= 0 {
        return;
    }
    loop {
        let entity = get_from_memory::<u32>(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN).wrapping_add((index as u32).wrapping_mul(4)));
        if entity != 0 && unsafe { call_virtual_args(entity, ENTITY_VTABLE_IS_REMOVED, []) } & 0xff != 0 {
            if unsafe { call_virtual_args(entity, ENTITY_VTABLE_IS_REMOVED_UNDO, []) } & 0xff == 0 {
                unsafe { call_virtual_args(this, AI_VTABLE_REMOVE_ENTITY, [entity]) };
                index -= 1;
                count -= 1;
            } else {
                let current_world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
                unsafe { call_virtual_args(current_world, WORLD_MGR_VTABLE_REMOVE_ENTITY, [entity, 0]) };
                unsafe { call_virtual_args(entity, ENTITY_VTABLE_SET_IS_REMOVED, [0, 0]) };
            }
        }
        index += 1;
        if index >= count {
            break;
        }
    }
}

/// `BFAIMgr` vtable slot of the tile-pointer `getPath` overload's target, the position-based `getPath`.
const AI_VTABLE_GET_PATH: u32 = 0x18;

/// `BFAIMgr::getPath(from, to, unit, flag)` for tiles: forwards to the position-based `getPath` through the
/// manager's vtable with both tiles replaced by their positions.
pub unsafe fn bfaimgr_get_path_tiles(this: u32, from: u32, to: u32, unit: u32, flag: bool) -> bool {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + AI_VTABLE_GET_PATH);
    let get_path = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32>(target) };
    low_byte_bool(get_path(this, from + TILE_POSITION, to + TILE_POSITION, unit, u32::from(flag)))
}

/// `ZTUnit` field `fPlaySetTank` compares against `4` to choose which parameter to play.
const UNIT_TANK_SELECTOR_OFFSET: u32 = 0x258;
/// Byte size of one `BFFunctionCall` parameter.
const CALL_PARAM_SIZE: u32 = 0xc;

/// `ZTAIMgr::fPlaySetTankF(call, unit)` with the `ZTUnitType` cast already applied to `unit`. A call with
/// exactly two parameters clears the call's result flag and plays the last parameter's set when the unit's
/// selector is `4`, the first otherwise (through `play_set(name, unit)`); any other parameter count returns
/// `0` without touching the call. `None` (defer to vanilla) for a null `unit` with two parameters, where
/// vanilla reads the selector through null.
pub unsafe fn f_play_set_tank(call: u32, unit: u32, play_set: unsafe fn(u32, u32) -> u32) -> Option<u32> {
    let function_call = unsafe { &*(call as *const RawFunctionCall) };
    let (begin, end) = (function_call.params_begin as u32, function_call.params_end as u32);
    if (end.wrapping_sub(begin) as i32) / CALL_PARAM_SIZE as i32 != 2 {
        return Some(0);
    }
    save_to_memory(call + CALL_DONE_FLAG_OFFSET, 0u8);
    if unit == 0 {
        return None;
    }
    let name = if get_from_memory::<u32>(unit + UNIT_TANK_SELECTOR_OFFSET) == 4 { end - CALL_PARAM_SIZE } else { begin };
    Some(unsafe { play_set(name, unit) })
}

/// `BFAIMgr::fPlaySet(name, unit, 1, 1)`, vanilla (stdcall; it ignores `ECX`).
unsafe fn play_set_vanilla(name: u32, unit: u32) -> u32 {
    unsafe { F_PLAY_SET.original()(name as *const i32, unit as *const c_void, 1, 1) }
}

/// `BFAIMgr` handler wrappers (stage 9, first slice): the `F` handlers that only unpack a call and forward.
/// Their behavior halves (`fPlayF`, `fFlyF`, `fPlayTime`, `fPlaySound`, `fPlayWithSound`, `fPlaySet`,
/// `fFaceDirection`) stay vanilla.
///
/// `BFAIMgr::fPlayTime(name, ms, unit, done, flag)` takes five stack arguments (`RET 0x14`); its `generated.rs`
/// entry lists four, so it is called at its address with the real shape. `this` is passed in `ECX` as vanilla does.
const F_PLAY_TIME_ADDRESS: u32 = 0x0042_4ee7;
type PlayTimeFn = unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, u32) -> u32;
/// `BFAIMgr` vtable slot `convertConstantParam(param, unit)`.
const AI_VTABLE_CONVERT_CONSTANT_PARAM: u32 = 0x34;
/// Entity vtable slot `setStopAtEnd(bool)`.
const ENTITY_VTABLE_SET_STOP_AT_END: u32 = 0xdc;
/// Offset of the call's second flag byte, which `fPlayTimeF` forwards to `fPlayTime`.
const CALL_FLAG_B_OFFSET: u32 = 0x19;
/// Milliseconds per second: `fPlayTimeF` converts its duration parameter to milliseconds.
const MILLIS_PER_SECOND: u32 = 1000;

/// The call's parameter count, computed as vanilla does (signed byte span over the entry size).
unsafe fn call_param_count(call: u32) -> i32 {
    let function_call = unsafe { &*(call as *const RawFunctionCall) };
    (function_call.params_end as u32).wrapping_sub(function_call.params_begin as u32) as i32 / CALL_PARAM_SIZE as i32
}

unsafe fn call_params_begin(call: u32) -> u32 {
    unsafe { &*(call as *const RawFunctionCall) }.params_begin as u32
}

unsafe fn call_params_end(call: u32) -> u32 {
    unsafe { &*(call as *const RawFunctionCall) }.params_end as u32
}

/// `fFaceDirectionF(call, unit)`: true for exactly one non-empty parameter. The real `fFaceDirection` it calls
/// is a `return 1` stub, so nothing else happens (the call's flag is untouched).
pub unsafe fn f_face_direction_f(call: u32) -> bool {
    unsafe { call_param_count(call) == 1 && !(*(call_params_begin(call) as *const RawCallString)).bytes().is_empty() }
}

/// `fHaltF(call, unit)`: stops the unit's movement and finishes. `None` (defer to vanilla) for a null unit,
/// where vanilla calls `setMoving` on null. Returns `1`, the low byte vanilla leaves in `AL`.
pub unsafe fn f_halt_f(call: u32, unit: u32) -> Option<u8> {
    if unit == 0 {
        return None;
    }
    unsafe { SET_MOVING.hooked()(unit as *const u32, 0, 0) };
    unsafe { set_done(call_done_flag(call)) };
    Some(1)
}

/// `fPlaySetF(call, unit)`: exactly one parameter clears the call's result flag and plays that set (through
/// `play_set(name, unit)`); any other count returns `0` without touching the call.
pub unsafe fn f_play_set_f(call: u32, unit: u32, play_set: unsafe fn(u32, u32) -> u32) -> u32 {
    if unsafe { call_param_count(call) } != 1 {
        return 0;
    }
    save_to_memory(call + CALL_DONE_FLAG_OFFSET, 0u8);
    unsafe { play_set(call_params_begin(call), unit) }
}

/// `fPlayStopAtEndF`'s first step: tells the unit to stop at the animation's end. `false` (defer) for a null unit.
pub unsafe fn f_play_stop_at_end_prepare(unit: u32) -> bool {
    if unit == 0 {
        return false;
    }
    let target = get_from_memory::<u32>(get_from_memory::<u32>(unit) + ENTITY_VTABLE_SET_STOP_AT_END);
    let set_stop_at_end = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32)>(target) };
    set_stop_at_end(unit, 1);
    true
}

/// `fPlayTimeF(this, call, unit)`: exactly two parameters convert the second one (through the manager's
/// `convertConstantParam` virtual) to milliseconds and call `fPlayTime(first, ms, unit, done, flagB)`; any other
/// count returns `0`.
pub unsafe fn f_play_time_f(this: u32, call: u32, unit: u32, play_time: unsafe fn(u32, u32, u32, u32, u32, u32) -> u32) -> u32 {
    if unsafe { call_param_count(call) } != 2 {
        return 0;
    }
    let begin = unsafe { call_params_begin(call) };
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + AI_VTABLE_CONVERT_CONSTANT_PARAM);
    let convert = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target) };
    let seconds = convert(this, begin + CALL_PARAM_SIZE, unit);
    let flag_b = u32::from(get_from_memory::<u8>(call + CALL_FLAG_B_OFFSET));
    unsafe { play_time(this, begin, seconds.wrapping_mul(MILLIS_PER_SECOND), unit, call + CALL_DONE_FLAG_OFFSET, flag_b) }
}

unsafe fn play_time_vanilla(this: u32, name: u32, millis: u32, unit: u32, done: u32, flag_b: u32) -> u32 {
    unsafe { mem::transmute::<u32, PlayTimeFn>(F_PLAY_TIME_ADDRESS)(this, name, millis, unit, done, flag_b) }
}

/// `ZTAIMgr` fields `initAIParams` fills: guest and staff random-walk extents (x then y), then the animal and
/// guest reset intervals.
const OFFSET_ANIMAL_RESET_INTERVAL: u32 = 0x138;
const OFFSET_GUEST_RESET_INTERVAL: u32 = 0x13c;
/// Default for each random-walk extent, and the section the keys live in.
const RANDOM_WALK_DEFAULT: i32 = 9;
const AI_INI_SECTION: &str = "AI";

/// The six `[AI]` parameters `ZTAIMgr::initAIParams` loads, in field order: guest x/y, staff x/y, animal reset
/// interval, guest reset interval. `read(key, default)` reads one integer. Extents are at least `1`, intervals
/// at least `0`.
pub fn compute_ai_params(mut read: impl FnMut(&str, i32) -> i32) -> [i32; 6] {
    let guest_x = read("mRandomXGuest", RANDOM_WALK_DEFAULT);
    let guest_y = read("mRandomYGuest", RANDOM_WALK_DEFAULT);
    let staff_x = read("mRandomXStaff", RANDOM_WALK_DEFAULT);
    let staff_y = read("mRandomYStaff", RANDOM_WALK_DEFAULT);
    let animal_reset = read("AnimalResetInterval", 0);
    let guest_reset = read("GuestResetInterval", 0);
    [guest_x.max(1), guest_y.max(1), staff_x.max(1), staff_y.max(1), animal_reset.max(0), guest_reset.max(0)]
}

/// `ZTAIMgr::initAIParams()`: loads the six parameters from the `[AI]` section of the game configuration.
/// Returns `0` like vanilla (the low byte is cleared).
pub unsafe fn ztaimgr_init_ai_params(this: u32) -> u32 {
    let section = VanillaString::new(AI_INI_SECTION);
    let values = compute_ai_params(|key, default| {
        let key = VanillaString::new(key);
        unsafe { BFINIFILE_READ.original()(section.as_ptr(), key.as_ptr(), default as u32) as i32 }
    });
    let offsets = [OFFSET_GUEST_RANDOM_X, OFFSET_GUEST_RANDOM_Y, OFFSET_STAFF_RANDOM_X, OFFSET_STAFF_RANDOM_Y, OFFSET_ANIMAL_RESET_INTERVAL, OFFSET_GUEST_RESET_INTERVAL];
    for (offset, value) in offsets.into_iter().zip(values) {
        save_to_memory(this + offset, value);
    }
    0
}

/// First save versions that carry the manager's tick fields (`>= 0x23`) and its event manager (`>= 0x61`).
const SAVE_VERSION_TICKS: u32 = 0x23;
const SAVE_VERSION_EVENTS: u32 = 0x61;

/// `BFAIMgr::load(file, version)`: the mirror of `save`. From version `0x23` reads the tick remainder and counter
/// (both reads are always attempted) and succeeds only if both returned one item; from version `0x61` also loads the
/// embedded event manager, whose result is ignored. Earlier versions read nothing and succeed.
pub unsafe fn bfaimgr_load(this: u32, file: *const u8, version: u32) -> u32 {
    let mut ok = true;
    if version >= SAVE_VERSION_TICKS {
        let read = |offset: u32| unsafe { READ_BYTES_FROM_FILE.hooked()((this + offset) as *const u32, 4, 1, file) == 1 };
        let remainder_read = read(OFFSET_AI_TICK_REMAINDER);
        let counter_read = read(OFFSET_AI_TICK_COUNT);
        ok = remainder_read && counter_read;
    }
    if version >= SAVE_VERSION_EVENTS {
        unsafe { BFEVENTMGR_LOAD.hooked()((this + OFFSET_EVENT_MGR) as *const u32, file as *const u32, version) };
    }
    u32::from(ok)
}

/// `BFEventInfo`'s vtable (RVA) and size; the info block is the `BFEvent` record without its leading vtable word.
const RVA_BFEVENT_INFO_VTABLE: u32 = 0x0023_5568;
const BFEVENT_INFO_SIZE: usize = 0x18;
/// The `BFAIMgr` listener overrides receive the `BFEventListenerInterface` subobject, `8` bytes into the manager;
/// its event manager is at `+0x98` from there (`+0xa0` of the manager).
const LISTENER_EVENT_MGR: u32 = 0x98;

/// `BFAIMgr::sendEvent(...)` (the listener override, `this` = the subobject): packs its eight arguments into a
/// `BFEventInfo` and posts it to the manager's event manager.
#[allow(clippy::too_many_arguments)]
pub unsafe fn bfaimgr_send_event(this: u32, kind: u16, target: u32, flag_b: u8, value_a: u32, flag_a: u8, value_b: u32, extra: u16, count: u16) {
    let mut info = [0u8; BFEVENT_INFO_SIZE];
    let base = info.as_mut_ptr();
    unsafe {
        (base as *mut u32).write_unaligned(get_module_base("zoo.exe") as u32 + RVA_BFEVENT_INFO_VTABLE);
        (base.add(0x04) as *mut u32).write_unaligned(value_a);
        (base.add(0x08) as *mut u32).write_unaligned(target);
        (base.add(0x0c) as *mut u32).write_unaligned(value_b);
        (base.add(0x10) as *mut u16).write_unaligned(extra);
        (base.add(0x12) as *mut u16).write_unaligned(kind);
        (base.add(0x14) as *mut u16).write_unaligned(count);
        base.add(0x16).write(flag_a);
        base.add(0x17).write(flag_b);
        BFEVENTMGR_POST_EVENT.hooked()((this + LISTENER_EVENT_MGR) as *const u32, base as *const u32);
    }
}

/// `BFAIMgr::getEvents(events, target, kind)` (the listener override, `this` = the subobject): collects the
/// event manager's events for `target` and `kind` into `events`.
pub unsafe fn bfaimgr_get_events(this: u32, events: *const c_void, target: u32, kind: i8) {
    unsafe { BFEVENTMGR_GET_EVENTS.hooked()((this + LISTENER_EVENT_MGR) as *const u32, events, target as i32, kind) };
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

    #[detour(F_FACE_DIRECTION_F)]
    unsafe extern "stdcall" fn f_face_direction_f_detour(call: *const u32, _unit: *const u32) -> bool {
        unsafe { f_face_direction_f(call as u32) }
    }

    #[detour(F_PLAY_SET_F)]
    unsafe extern "stdcall" fn f_play_set_f_detour(call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { f_play_set_f(call as u32, unit as u32, play_set_vanilla) })
    }

    #[detour(F_PLAY_REVERSE_F)]
    unsafe extern "thiscall" fn f_play_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_PLAY_F.original()(this, call, unit, 1, 0) }
    }

    #[detour(F_PLAY_PING_PONG_F)]
    unsafe extern "thiscall" fn f_play_ping_pong_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_PLAY_F.original()(this, call, unit, 0, 1) }
    }

    #[detour(F_PLAY_STOP_AT_END_F)]
    unsafe extern "thiscall" fn f_play_stop_at_end_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        if !unsafe { f_play_stop_at_end_prepare(unit as u32) } {
            return unsafe { F_PLAY_STOP_AT_END_F_DETOUR.call(this, call, unit) };
        }
        unsafe { F_PLAY_F.original()(this, call, unit, 0, 0) }
    }

    #[detour(F_FLY_REVERSE_F)]
    unsafe extern "thiscall" fn f_fly_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_FLY_F.original()(this, call, unit, 1) }
    }

    #[detour(F_PLAY_WITH_SOUND_F)]
    unsafe extern "stdcall" fn f_play_with_sound_f_detour(call: *const u32, unit: *const u32) -> bool {
        let call = call as u32;
        let result = unsafe {
            F_PLAY_WITH_SOUND.original()(call_params_begin(call), call_params_end(call).wrapping_sub(CALL_PARAM_SIZE), unit as *const i32, (call + CALL_DONE_FLAG_OFFSET) as *const u8)
        };
        low_byte_bool(result)
    }

    #[detour(F_PLAY_SOUND_F)]
    unsafe extern "thiscall" fn f_play_sound_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        let call = call as u32;
        if unsafe { call_param_count(call) } != 1 {
            return false;
        }
        let result = unsafe { F_PLAY_SOUND.original()(this, call_params_begin(call) as *const i8, unit as *const i32, (call + CALL_DONE_FLAG_OFFSET) as *const u8) };
        low_byte_bool(result)
    }

    #[detour(F_PLAY_TIME_F)]
    unsafe extern "thiscall" fn f_play_time_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { f_play_time_f(this as u32, call as u32, unit as u32, play_time_vanilla) })
    }

    #[detour(EXECUTE_CALL)]
    unsafe extern "thiscall" fn execute_call_detour(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) {
        unsafe { execute_call(this as u32, call as u32, unit as u32, done as *mut u8) }
    }

    #[detour(BFAIMGR_EXECUTE_CALL)]
    unsafe extern "thiscall" fn bfaimgr_execute_call_detour(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> u32 {
        unsafe { execute_base_call(this as u32, call as u32, unit as u32, done as *mut u8) as u32 }
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

    #[detour(ZTAIMGR_UPDATE)]
    unsafe extern "thiscall" fn ztaimgr_update_detour(this: *const u32, dt: u32) {
        unsafe { ztaimgr_update(this as u32, dt) }
    }

    #[detour(BFAIMGR_UPDATE)]
    unsafe extern "thiscall" fn bfaimgr_update_detour(this: *const u32, dt: u32) {
        unsafe { bfaimgr_update(this as u32, dt) }
    }

    #[detour(BFAIMGR_SAVE)]
    unsafe extern "thiscall" fn bfaimgr_save_detour(this: *const u32, file: *const i8) -> bool {
        unsafe { bfaimgr_save(this as u32, file) }
    }

    #[detour(BFAIMGR_LOAD)]
    unsafe extern "thiscall" fn bfaimgr_load_detour(this: *const u32, file: *const u8, version: u32) -> u32 {
        unsafe { bfaimgr_load(this as u32, file, version) }
    }

    #[detour(BFAIMGR_SEND_EVENT)]
    unsafe extern "thiscall" fn bfaimgr_send_event_detour(
        this: *const u32, kind: u16, target: u32, flag_b: u8, value_a: u32, flag_a: u8, value_b: u32, extra: u16, count: u16,
    ) {
        unsafe { bfaimgr_send_event(this as u32, kind, target, flag_b, value_a, flag_a, value_b, extra, count) }
    }

    #[detour(BFAIMGR_GET_EVENTS)]
    unsafe extern "thiscall" fn bfaimgr_get_events_detour(this: *const u32, events: *const c_void, target: u32, kind: i8) {
        unsafe { bfaimgr_get_events(this as u32, events, target, kind) }
    }

    #[detour(BFAIMGR_CALL_DONE)]
    unsafe extern "thiscall" fn bfaimgr_call_done_detour(_this: *const u32, entity: *const u32) -> bool {
        unsafe { bfaimgr_call_done(entity as u32) }
    }

    #[detour(BFAIMGR_REMOVE_ENTITIES)]
    unsafe extern "thiscall" fn bfaimgr_remove_entities_detour(this: *const u32) {
        unsafe { bfaimgr_remove_entities(this as u32) }
    }

    #[detour(BFAIMGR_GET_PATH_1)]
    unsafe extern "thiscall" fn bfaimgr_get_path_tiles_detour(this: *const u32, from: *const u32, to: *const u32, unit: *const u32, flag: bool) -> bool {
        unsafe { bfaimgr_get_path_tiles(this as u32, from as u32, to as u32, unit as u32, flag) }
    }

    #[detour(ZTAIMGR_INIT_AIPARAMS)]
    unsafe extern "thiscall" fn ztaimgr_init_ai_params_detour(this: *const u32) -> u32 {
        unsafe { ztaimgr_init_ai_params(this as u32) }
    }

    #[detour(BFAIMGR_CHECK_PATH)]
    unsafe extern "thiscall" fn bfaimgr_check_path_detour(this: *const u32, from: *const u32, to: *const u32, unit: *const u32) -> u32 {
        unsafe { bfaimgr_check_path(this as u32, from as u32, to as u32, unit as u32) }
    }

    #[detour(BFAIMGR_LOAD_DATA)]
    unsafe extern "thiscall" fn bfaimgr_load_data_detour(_this: *const u32, full: bool) -> bool {
        unsafe { bfaimgr_load_data(full) }
    }

    #[detour(ZTAIMGR_CREATE)]
    unsafe extern "stdcall" fn ztaimgr_create_detour() -> u32 {
        unsafe { ztaimgr_create() }
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

        pub(crate) fn base_execute_call(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::BFAIMGR_EXECUTE_CALL_DETOUR.call(this, call, unit, done) }
        }

        pub(crate) fn bfaimgr_update(this: *const u32, dt: u32) {
            unsafe { super::BFAIMGR_UPDATE_DETOUR.call(this, dt) }
        }

        pub(crate) fn ztaimgr_update(this: *const u32, dt: u32) {
            unsafe { super::ZTAIMGR_UPDATE_DETOUR.call(this, dt) }
        }

        pub(crate) fn bfaimgr_save(this: *const u32, file: *const i8) -> bool {
            unsafe { super::BFAIMGR_SAVE_DETOUR.call(this, file) }
        }

        pub(crate) fn bfaimgr_load_data(this: *const u32, full: bool) -> bool {
            unsafe { super::BFAIMGR_LOAD_DATA_DETOUR.call(this, full) }
        }

        pub(crate) fn bfaimgr_load(this: *const u32, file: *const u8, version: u32) -> u32 {
            unsafe { super::BFAIMGR_LOAD_DETOUR.call(this, file, version) }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn bfaimgr_send_event(this: *const u32, kind: u16, target: u32, flag_b: u8, value_a: u32, flag_a: u8, value_b: u32, extra: u16, count: u16) {
            unsafe { super::BFAIMGR_SEND_EVENT_DETOUR.call(this, kind, target, flag_b, value_a, flag_a, value_b, extra, count) }
        }

        pub(crate) fn bfaimgr_get_events(this: *const u32, events: *const c_void, target: u32, kind: i8) {
            unsafe { super::BFAIMGR_GET_EVENTS_DETOUR.call(this, events, target, kind) }
        }

        pub(crate) fn bfaimgr_call_done(this: *const u32, entity: *const u32) -> bool {
            unsafe { super::BFAIMGR_CALL_DONE_DETOUR.call(this, entity) }
        }

        pub(crate) fn bfaimgr_remove_entities(this: *const u32) {
            unsafe { super::BFAIMGR_REMOVE_ENTITIES_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_get_path_tiles(this: *const u32, from: *const u32, to: *const u32, unit: *const u32, flag: bool) -> bool {
            unsafe { super::BFAIMGR_GET_PATH_1_DETOUR.call(this, from, to, unit, flag) }
        }

        pub(crate) fn ztaimgr_init_ai_params(this: *const u32) -> u32 {
            unsafe { super::ZTAIMGR_INIT_AIPARAMS_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_check_path(this: *const u32, from: *const u32, to: *const u32, unit: *const u32) -> u32 {
            unsafe { super::BFAIMGR_CHECK_PATH_DETOUR.call(this, from, to, unit) }
        }

        pub(crate) fn ztaimgr_create() -> u32 {
            unsafe { super::ZTAIMGR_CREATE_DETOUR.call() }
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

        pub(crate) fn f_face_direction_f(call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_FACE_DIRECTION_F_DETOUR.call(call, unit) }
        }

        pub(crate) fn f_play_set_f(call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_SET_F_DETOUR.call(call, unit) }
        }

        pub(crate) fn f_play_reverse_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_REVERSE_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_ping_pong_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_PING_PONG_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_stop_at_end_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_STOP_AT_END_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_fly_reverse_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_FLY_REVERSE_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_sound_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_SOUND_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_time_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_TIME_F_DETOUR.call(this, call, unit) }
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
        bfaimgr_get_events as real_bfaimgr_get_events, bfaimgr_load as real_bfaimgr_load, bfaimgr_send_event as real_bfaimgr_send_event, bfaimgr_call_done as real_bfaimgr_call_done, bfaimgr_check_path as real_bfaimgr_check_path, bfaimgr_get_path_tiles as real_bfaimgr_get_path_tiles,
        bfaimgr_remove_entities as real_bfaimgr_remove_entities, ztaimgr_init_ai_params as real_ztaimgr_init_ai_params, bfaimgr_load_data as real_bfaimgr_load_data, bfaimgr_save as real_bfaimgr_save, bfaimgr_update as real_bfaimgr_update, ztaimgr_create as real_ztaimgr_create,
        ztaimgr_update as real_ztaimgr_update,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        base_execute_call as real_base_execute_call, execute_call as real_execute_call, f_caught_prey as real_f_caught_prey, f_die as real_f_die, f_face_toward_food as real_f_face_toward_food, f_face_toward_target as real_f_face_toward_target, f_exit_building as real_f_exit_building, f_kill_prey as real_f_kill_prey,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        f_end_trick as real_f_end_trick, f_enter_building as real_f_enter_building, f_guest_routine as real_f_guest_routine, f_guide_routine as real_f_guide_routine,
        f_keeper_routine as real_f_keeper_routine, f_maint_routine as real_f_maint_routine, f_return_to_building as real_f_return_to_building,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        f_face_direction_f as real_f_face_direction_f, f_fly_reverse_f as real_f_fly_reverse_f, f_play_ping_pong_f as real_f_play_ping_pong_f, f_play_reverse_f as real_f_play_reverse_f,
        f_play_set_f as real_f_play_set_f, f_play_sound_f as real_f_play_sound_f, f_play_stop_at_end_f as real_f_play_stop_at_end_f, f_play_time_f as real_f_play_time_f,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_params_use_defaults_keys_and_clamps() {
        let mut keys = Vec::new();
        let defaults = compute_ai_params(|key, default| {
            keys.push((key.to_string(), default));
            default
        });
        assert_eq!(defaults, [9, 9, 9, 9, 0, 0]);
        let expected: Vec<(String, i32)> = [("mRandomXGuest", 9), ("mRandomYGuest", 9), ("mRandomXStaff", 9), ("mRandomYStaff", 9), ("AnimalResetInterval", 0), ("GuestResetInterval", 0)]
            .iter()
            .map(|(key, default)| (key.to_string(), *default))
            .collect();
        assert_eq!(keys, expected);
        // Extents clamp up to 1, intervals up to 0, positive values pass through.
        assert_eq!(compute_ai_params(|_, _| -5), [1, 1, 1, 1, 0, 0]);
        assert_eq!(compute_ai_params(|_, _| 0), [1, 1, 1, 1, 0, 0]);
        assert_eq!(compute_ai_params(|_, _| 25), [25; 6]);
        let by_key = compute_ai_params(|key, _| match key {
            "mRandomXGuest" => 3,
            "mRandomYStaff" => -1,
            "GuestResetInterval" => 40,
            _ => 7,
        });
        assert_eq!(by_key, [3, 7, 7, 1, 7, 40]);
    }

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
    fn base_call_lookup_covers_every_name_and_nothing_near() {
        let names: [(&[u8], BaseCall); 28] = [
            (b"fPlay", BaseCall::Play),
            (b"fSetBase", BaseCall::SetBase),
            (b"fPlayWithBase", BaseCall::PlayWithBase),
            (b"fMoveEntityDelta", BaseCall::MoveEntityDelta),
            (b"fMoveEntitySpecial", BaseCall::MoveEntitySpecial),
            (b"fPlayTime", BaseCall::PlayTime),
            (b"fPlaySound", BaseCall::PlaySound),
            (b"fPlaySpecial", BaseCall::PlaySpecial),
            (b"fPlaySp", BaseCall::PlaySpecial),
            (b"fPlaySetProb", BaseCall::PlaySetProb),
            (b"fPlaySetTerrain", BaseCall::PlaySetTerrain),
            (b"fPlayReverse", BaseCall::PlayReverse),
            (b"fPlayPingPong", BaseCall::PlayPingPong),
            (b"fPingPong", BaseCall::PlayPingPong),
            (b"fPlayWithSound", BaseCall::PlayWithSound),
            (b"fPlaySFX", BaseCall::PlayWithSound),
            (b"fPlayStopAtEnd", BaseCall::PlayStopAtEnd),
            (b"fPlaySet", BaseCall::PlaySet),
            (b"fMoveSlow", BaseCall::MoveSlow),
            (b"fMoveMedium", BaseCall::MoveMedium),
            (b"fMove", BaseCall::Move),
            (b"fMoveReverse", BaseCall::MoveReverse),
            (b"fMoveFast", BaseCall::MoveFast),
            (b"fMoveWithSound", BaseCall::MoveWithSound),
            (b"fFaceDirection", BaseCall::FaceDirection),
            (b"fFly", BaseCall::Fly),
            (b"fKeepControl", BaseCall::KeepControl),
            (b"fHalt", BaseCall::Halt),
        ];
        for (name, expected) in names {
            assert_eq!(lookup_base_call(name), Some(expected), "{}", String::from_utf8_lossy(name));
            assert_eq!(lookup_base_call(&name.to_ascii_lowercase()), None);
            let mut longer = name.to_vec();
            longer.push(b'x');
            assert!(lookup_base_call(&longer).is_none(), "{}", String::from_utf8_lossy(&longer));
        }
        assert_eq!(lookup_base_call(b"fFlyReverse"), Some(BaseCall::FlyReverse));
        // Neighbouring names that are prefixes of a handled name, or ZT-only names, are not base calls.
        for name in [&b""[..], b"fPla", b"fPlayS", b"fPlaySe", b"fMov", b"fWalk", b"fRun", b"fDie", b"fHal"] {
            assert_eq!(lookup_base_call(name), None, "{}", String::from_utf8_lossy(name));
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
