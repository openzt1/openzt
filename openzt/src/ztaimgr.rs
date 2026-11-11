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
    ztguest::F_ROUTINE as ZTGUEST_F_ROUTINE,
    ztunit::GET_PREDATOR_UNIT,
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

    pub(crate) use super::{f_enter_building, f_return_to_building};
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
