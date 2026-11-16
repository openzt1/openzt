//! `ZTAIMgr`/`BFAIMgr` reimplementation: the AI manager and the `executeCall` handlers it dispatches to.
//!
//! `ZTAIMgr` extends `BFAIMgr` (`BFAIMgr` -> `ZTAIMgr`, vtable `0x00635324`). The AI manager is a
//! singleton (`GLOBAL_ZTAIMgr`) that un-ported vanilla code reaches through its vtable and reads by raw
//! offset, so nothing here replaces the object: each detour reads and writes vanilla's own memory in place
//! (style 1), and no struct is declared because only fixed dwords are touched (the offsets below).
//!
//! Reimplemented:
//! - Manager methods: `getRandomX`/`getRandomY`, `isSlow`/`isMedium`/`isFastMoveFunction`,
//!   `convertConstantParam` (partial override), `removeEntity`, `removeEntities`, `callDone`, the tile
//!   `getPath` overload, `checkPath`, `initAIParams` (`ZTAIMgr`), `update` (both classes), `save`, `load`,
//!   `loadData`, `create`, the `BFAIMgr` constructor and destructors, and the listener overrides
//!   `sendEvent`/`getEvents`.
//! - Both `executeCall` dispatch chains and every handler they reach, wrappers and behavior halves alike
//!   (`fMove`, `fPlaySpecial`, `fPlaySet`, `fFly`, `fRandomWalk`, ...).
//!
//! Still vanilla: `BFAIMgr::fPlay` (`0x0042296d`) and its mixer helpers, the position-based `getPath`, the
//! entity `fRoutine` bodies, `ZTAIMgr::init` (a bare `jmp [vtable+0x38]` thunk), and `BFAIMgr::init` with the
//! `BFAIMgr::initAIParams` it dispatches to. The constructor calls those two once per manager, while the
//! `BFAIMgr` vtable is still installed. Handlers defer to vanilla for inputs on
//! which vanilla faults (typically a null or wrong-class `this`).
//!
//! Windows layout differs from the macOS build (`ZTAIMgr` fields sit `+4` later, entity-type fields `+0x18`
//! later), so every offset here comes from the Windows decompile, never the macOS one.

use std::{
    ffi::{c_void, CStr},
    mem,
};

use openzt_detour::generated::{
    bfaimgr::{F_FLY, F_MOVE_REVERSE_F, F_PLAY as BFAIMGR_F_PLAY, F_PLAY_TIME},
    bfcoggoal::REACHED_TARGET as BFCOGGOAL_REACHED_TARGET,
    gxmixerlink::DURATION as LINK_DURATION,
    ztanimal::SET_PENDING_BS_0X4300F6 as ZTANIMAL_SET_PENDING_BS,
    ztcheat::ENTITY_REMOVED as ZTCHEAT_ENTITY_REMOVED,
    ztguide::F_ROUTINE as ZTGUIDE_F_ROUTINE,
    ztkeeper::F_ROUTINE as ZTKEEPER_F_ROUTINE,
    standalone::{ATOI, TIME},
    bfbehaviorset::INIT_CHILD as BFBEHAVIORSET_INIT_CHILD,
    bfbscall::{CONSTRUCTOR as BFBSCALL_CONSTRUCTOR, COPY_0X43E90E as BFBSCALL_COPY},
    bfentity::SET_BS as BFENTITY_SET_BS,
    msvc_std_basic_string::{ASSIGN_RANGE_0X406C55, BASIC_STRING_0},
    msvc_std_vectorbasic_string::{ERASE_RANGE_0X410AC3, PUSH_BACK_0X4FCEF5},
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
        CALL_DONE as BFAIMGR_CALL_DONE, CHECK_PATH as BFAIMGR_CHECK_PATH, GET_EVENTS as BFAIMGR_GET_EVENTS, CONSTRUCTOR as BFAIMGR_CONSTRUCTOR, DESTRUCTOR_0 as BFAIMGR_DESTRUCTOR_0, DESTRUCTOR_1 as BFAIMGR_DESTRUCTOR_1, INIT as BFAIMGR_INIT, INIT_AIPARAMS as BFAIMGR_INIT_AIPARAMS, F_PLAY_SET, GET_PATH_1 as BFAIMGR_GET_PATH_1,
        LOAD as BFAIMGR_LOAD, LOAD_DATA as BFAIMGR_LOAD_DATA, REMOVE_ENTITIES as BFAIMGR_REMOVE_ENTITIES, SAVE as BFAIMGR_SAVE, SEND_EVENT as BFAIMGR_SEND_EVENT, UPDATE as BFAIMGR_UPDATE,
    },
    bfinifile::READ as BFINIFILE_READ,
    gxmixer::{CLEAR_ALL as GXMIXER_CLEAR_ALL, GET_ANIM as GXMIXER_GET_ANIM},
    bfpathfinder::SET_NODE_MAX,
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    bfpathfinder::{CONSTRUCTOR as BFPATHFINDER_CONSTRUCTOR, DESTRUCTOR as BFPATHFINDER_DESTRUCTOR},
    gxlleanim::DESTRUCTOR as GXLLEANIM_DESTRUCTOR,
    gxlleanimset::RELEASE_0 as GXLLEANIMSET_RELEASE,
    msvc_std_tree20::CLEAR as TREE20_CLEAR,
    msvc_std_vector_t_28::VECTOR_T_28,
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE_N_28},
    ztaimgr::{CREATE as ZTAIMGR_CREATE, INIT_AIPARAMS as ZTAIMGR_INIT_AIPARAMS},
    bfmap::MOVE_ENTITY_DELTA as BFMAP_MOVE_ENTITY_DELTA,
    bfunit::{GET_PATH_COST as BFUNIT_GET_PATH_COST, SET_MOVING, STOP as BFUNIT_STOP},
    bfmap::GET_NEIGHBORS as BFMAP_GET_NEIGHBORS,
    msvc_std::RAND,
    poolalloc::DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4,
    bfentity::{FIND_ANIM as BFENTITY_FIND_ANIM, FIND_SOUND as BFENTITY_FIND_SOUND},
    gxmixer::{SET_BASE_0 as GXMIXER_SET_BASE, SET_SET as GXMIXER_SET_SET},
    bfeventmgr::{CONSTRUCTOR as BFEVENTMGR_CONSTRUCTOR, GET_EVENTS as BFEVENTMGR_GET_EVENTS, LOAD as BFEVENTMGR_LOAD, POST_EVENT as BFEVENTMGR_POST_EVENT, SAVE as BFEVENTMGR_SAVE, UPDATE as BFEVENTMGR_UPDATE},
    standalone::{DEALLOCATE as READ_BYTES_FROM_FILE, WRITE_BYTES_TO_FILE},
    ztaimgr::UPDATE as ZTAIMGR_UPDATE,
    ztmainttaskpool::LISTEN as ZTMAINTTASKPOOL_LISTEN,
    bfaimgr::{
        EXECUTE_CALL as BFAIMGR_EXECUTE_CALL, F_MOVE, F_PLAY_SPECIAL, F_FACE_DIRECTION_F, F_FLY_F, F_FLY_REVERSE_F, F_HALT_F, F_PLAY_F, F_PLAY_WITH_SOUND, F_PLAY_SOUND, F_MOVE_ENTITY_DELTA_F, F_MOVE_ENTITY_SPECIAL_F, F_MOVE_F,
        F_MOVE_FAST_F, F_MOVE_MEDIUM_F, F_MOVE_SLOW_F, F_MOVE_WITH_SOUND_F, F_PLAY_PING_PONG_F, F_PLAY_REVERSE_F, F_PLAY_SET_F, F_PLAY_SET_PROB, F_PLAY_SET_PROB_F, F_RANDOM_WALK, F_PLAY_SOUND_F,
        F_PLAY_SPECIAL_F, F_PLAY_STOP_AT_END_F, F_PLAY_TIME_F, F_PLAY_WITH_BASE_F, F_PLAY_WITH_SOUND_F, F_SET_BASE_F,
    },
    ztaimgr::F_PLAY_SET_TERRAIN_F as ZTAIMGR_F_PLAY_SET_TERRAIN_F,
    ztanimal::TARGET_BUILDING as ZTANIMAL_TARGET_BUILDING,
    bfentitytype::{FIND_ANIM as BFENTITYTYPE_FIND_ANIM, FIND_SOUND as BFENTITYTYPE_FIND_SOUND},
    ztaimgr::{EXECUTE_CALL, F_DUST_BALL_F, F_FACE_TOWARD_FOOD, F_FACE_TOWARD_FOOD_F, F_FACE_TOWARD_TARGET, F_FACE_TOWARD_TARGET_F, F_DUST_BALL, F_PLAY_SET_TANK_F, F_TARGET_BUILDING, F_TARGET_BUILDING_F},
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
    util::{get_from_memory, low_byte_bool, save_to_memory, LiveMemory},
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

/// A `{begin, end, capacity}` string as `BFFunctionCall` stores its name and each parameter.
#[repr(C)]
struct RawCallString {
    begin: *const u8,
    end: *const u8,
    capacity: *const u8,
    _live: LiveMemory,
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
    _live: LiveMemory,
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
        let is_guide = unit_type != 0 && unsafe { type_tag_check(unit_type, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUIDE) };
        return Some(if is_guide { get_from_memory::<i32>(unit_type + GUIDE_TYPE_INFORM_GUEST_TIME_OFFSET) } else { 0 });
    }
    None
}

/// Removes `entity` from the world: runs the cheat hook, lets the map view drop any reference to it, then
/// dispatches the world manager's own `removeEntity(entity, true)`. Returns `false` if there is no world.
pub unsafe fn remove_entity(entity: u32) -> bool {
    let world = globals().ztworldmgr_ptr() as u32;
    unsafe { ZTCHEAT_ENTITY_REMOVED.original()(entity as i32) };
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
    unsafe { building_transition_al(unit, done, slot) }.is_some()
}

/// [`building_transition`] keeping the virtual's `AL`, which vanilla's handler returns unchanged (the virtuals
/// are not bool functions, so it can be any byte). `None` when it defers to vanilla.
unsafe fn building_transition_al(unit: u32, done: *const bool, slot: u32) -> Option<u8> {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        return None;
    }
    let vtable = get_from_memory::<u32>(unit);
    let target = get_from_memory::<u32>(vtable + slot);
    let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> u8>(target) };
    let al = method(unit);
    unsafe { set_done(done) };
    Some(al)
}

pub unsafe fn f_enter_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_ENTER_BUILDING) }
}

pub unsafe fn f_return_to_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_RETURN_TO_BUILDING) }
}

/// [`f_enter_building`] returning the entity virtual's `AL`.
pub unsafe fn f_enter_building_al(unit: u32, done: *const bool) -> Option<u8> {
    unsafe { building_transition_al(unit, done, ENTITY_VTABLE_ENTER_BUILDING) }
}

/// [`f_return_to_building`] returning the entity virtual's `AL`.
pub unsafe fn f_return_to_building_al(unit: u32, done: *const bool) -> Option<u8> {
    unsafe { building_transition_al(unit, done, ENTITY_VTABLE_RETURN_TO_BUILDING) }
}

/// [`f_exit_building`] returning the entity virtual's `AL`.
pub unsafe fn f_exit_building_al(unit: u32, done: *const bool) -> Option<u8> {
    unsafe { building_transition_al(unit, done, ENTITY_VTABLE_EXIT_BUILDING) }
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
    unsafe { ZTGUIDE_F_ROUTINE.original()(unit as *const u32, done) };
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
    unsafe { ZTKEEPER_F_ROUTINE.original()(unit as *const u32, done) }
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

/// The handler a dispatched call name reaches: one variant per `BFAIMgr`/`ZTAIMgr` `...F` wrapper.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handler {
    MoveSlowF,
    MoveMediumF,
    MoveFastF,
    SprayF,
    KeeperRoutineF,
    GuestRoutineF,
    GuideRoutineF,
    MaintRoutineF,
    FaceTowardFoodF,
    FaceTowardTargetF,
    ShowPredatorF,
    HidePredatorF,
    EnterBuildingF,
    ExitBuildingF,
    DustBallF,
    KillPreyF,
    CaughtPreyF,
    DieF,
    TargetBuildingF,
    HatchF,
    EndTrickF,
    ReturnToBuildingF,
    PlaySetTankF,
    PlayF,
    SetBaseF,
    PlayWithBaseF,
    MoveEntityDeltaF,
    MoveEntitySpecialF,
    PlayTimeF,
    PlaySoundF,
    PlaySpecialF,
    PlaySetProbF,
    PlayReverseF,
    PlayPingPongF,
    PlayWithSoundF,
    PlayStopAtEndF,
    PlaySetF,
    MoveF,
    MoveReverseF,
    MoveWithSoundF,
    FaceDirectionF,
    FlyF,
    FlyReverseF,
    HaltF,
}

/// Where the dispatchers hand a call to a handler, so a test can record what they decide. `invoke(handler,
/// this, call, unit, trailing)` runs the handler with the unit it was given and the constant trailing
/// arguments the vanilla chain passes (`fPlayF`/`fPlayWithBaseF` `0, 0`, `fMoveF` `false`, `fFlyF` `0`; unused
/// slots are `0`), and returns its result (`0` for a handler with none).
pub trait Handlers {
    unsafe fn invoke(&self, handler: Handler, this: u32, call: u32, unit: u32, trailing: [u32; 2]) -> u32;
}

/// The vanilla handlers. A handler this crate has ported is reached through its wrapper address (`hooked()`,
/// what a vanilla caller would run), so its own deferral to vanilla for faulting inputs still applies; the rest
/// call the real wrappers.
struct VanillaHandlers;

impl Handlers for VanillaHandlers {
    unsafe fn invoke(&self, handler: Handler, this: u32, call: u32, unit: u32, trailing: [u32; 2]) -> u32 {
        let (t, c, u) = (this as *const u32, call as *const u32, unit as *const u32);
        let (ci, ui) = (call as i32, unit as *const i32);
        unsafe {
            match handler {
                Handler::MoveSlowF => F_MOVE_SLOW_F.hooked()(t, ci, ui),
                Handler::MoveMediumF => F_MOVE_MEDIUM_F.hooked()(t, ci, ui),
                Handler::MoveFastF => F_MOVE_FAST_F.hooked()(t, ci, ui),
                Handler::SprayF => F_SPRAY_F.hooked()(ci, u) as u32,
                Handler::KeeperRoutineF => F_KEEPER_ROUTINE_F.hooked()(call, ui) as u32,
                Handler::GuestRoutineF => F_GUEST_ROUTINE_F.hooked()(call, unit) as u32,
                Handler::GuideRoutineF => F_GUIDE_ROUTINE_F.hooked()(call, unit) as u32,
                Handler::MaintRoutineF => F_MAINT_ROUTINE_F.hooked()(call, unit) as u32,
                Handler::FaceTowardFoodF => F_FACE_TOWARD_FOOD_F.hooked()(ci, unit as *const u8) as u32,
                Handler::FaceTowardTargetF => F_FACE_TOWARD_TARGET_F.hooked()(ci, unit as *const c_void) as u32,
                Handler::ShowPredatorF => F_SHOW_PREDATOR_F.hooked()(ci, ui) as u32,
                Handler::HidePredatorF => F_HIDE_PREDATOR_F.hooked()(ci, unit as *const u8) as u32,
                Handler::EnterBuildingF => F_ENTER_BUILDING_F.hooked()(ci, ui) as u32,
                Handler::ExitBuildingF => F_EXIT_BUILDING_F.hooked()(ci, ui) as u32,
                Handler::DustBallF => F_DUST_BALL_F.hooked()(t, ci, ui) as u32,
                Handler::KillPreyF => F_KILL_PREY_F.hooked()(ci, unit as *const u8) as u32,
                Handler::CaughtPreyF => F_CAUGHT_PREY_F.hooked()(ci, ui) as u32,
                Handler::DieF => F_DIE_F.hooked()(ci, ui) as u32,
                Handler::TargetBuildingF => F_TARGET_BUILDING_F.hooked()(t, ci, ui) as u32,
                Handler::HatchF => F_HATCH_F.hooked()(ci, ui) as u32,
                Handler::EndTrickF => F_END_TRICK_F.hooked()(ci, ui) as u32,
                Handler::ReturnToBuildingF => F_RETURN_TO_BUILDING_F.hooked()(ci, ui) as u32,
                Handler::PlaySetTankF => match f_play_set_tank(call, unit, play_set_vanilla) {
                    Some(result) => result,
                    None => F_PLAY_SET_TANK_F.original()(ci, unit as *const c_void),
                },
                Handler::PlayF => F_PLAY_F.hooked()(t, c, u, trailing[0] as i8, trailing[1] as i8) as u32,
                Handler::SetBaseF => F_SET_BASE_F.hooked()(t, c, u) as u32,
                Handler::PlayWithBaseF => F_PLAY_WITH_BASE_F.hooked()(t, c, u, trailing[0] as i8, trailing[1] as i8) as u32,
                Handler::MoveEntityDeltaF => F_MOVE_ENTITY_DELTA_F.hooked()(t, c, u) as u32,
                Handler::MoveEntitySpecialF => F_MOVE_ENTITY_SPECIAL_F.hooked()(t, c, u) as u32,
                Handler::PlayTimeF => F_PLAY_TIME_F.hooked()(t, c, u) as u32,
                Handler::PlaySoundF => F_PLAY_SOUND_F.hooked()(t, c, u) as u32,
                Handler::PlaySpecialF => F_PLAY_SPECIAL_F.hooked()(t, c, u) as u32,
                Handler::PlaySetProbF => F_PLAY_SET_PROB_F.hooked()(c, u) as u32,
                Handler::PlayReverseF => F_PLAY_REVERSE_F.hooked()(t, c, u) as u32,
                Handler::PlayPingPongF => F_PLAY_PING_PONG_F.hooked()(t, c, u) as u32,
                Handler::PlayWithSoundF => F_PLAY_WITH_SOUND_F.hooked()(c, u) as u32,
                Handler::PlayStopAtEndF => F_PLAY_STOP_AT_END_F.hooked()(t, c, u) as u32,
                Handler::PlaySetF => F_PLAY_SET_F.hooked()(c, u) as u32,
                Handler::MoveF => F_MOVE_F.hooked()(t, c, u, trailing[0] != 0),
                Handler::MoveReverseF => F_MOVE_REVERSE_F.hooked()(t, c, u) as u32,
                Handler::MoveWithSoundF => F_MOVE_WITH_SOUND_F.hooked()(t, c, u),
                Handler::FaceDirectionF => F_FACE_DIRECTION_F.hooked()(c, u) as u32,
                Handler::FlyF => F_FLY_F.hooked()(t, c, u, trailing[0] as i8) as u32,
                Handler::FlyReverseF => F_FLY_REVERSE_F.hooked()(t, c, u) as u32,
                Handler::HaltF => match f_halt_f(call, unit) {
                    Some(result) => u32::from(result),
                    None => F_HALT_F.original()(c, u) as u32,
                },
            }
        }
    }
}

/// The handler for a `ZTAIMgr` call name.
pub fn zt_handler(which: ZtCall) -> Handler {
    match which {
        ZtCall::MoveSlow => Handler::MoveSlowF,
        ZtCall::MoveMedium => Handler::MoveMediumF,
        ZtCall::MoveFast => Handler::MoveFastF,
        ZtCall::Spray => Handler::SprayF,
        ZtCall::KeeperRoutine => Handler::KeeperRoutineF,
        ZtCall::GuestRoutine => Handler::GuestRoutineF,
        ZtCall::GuideRoutine => Handler::GuideRoutineF,
        ZtCall::MaintRoutine => Handler::MaintRoutineF,
        ZtCall::FaceTowardFood => Handler::FaceTowardFoodF,
        ZtCall::FaceTowardTarget => Handler::FaceTowardTargetF,
        ZtCall::ShowPredator => Handler::ShowPredatorF,
        ZtCall::HidePredator => Handler::HidePredatorF,
        ZtCall::EnterBuilding => Handler::EnterBuildingF,
        ZtCall::ExitBuilding => Handler::ExitBuildingF,
        ZtCall::DustBall => Handler::DustBallF,
        ZtCall::KillPrey => Handler::KillPreyF,
        ZtCall::CaughtPrey => Handler::CaughtPreyF,
        ZtCall::Die => Handler::DieF,
        ZtCall::TargetBuilding => Handler::TargetBuildingF,
        ZtCall::Hatch => Handler::HatchF,
        ZtCall::EndTrick => Handler::EndTrickF,
        ZtCall::ReturnToBuilding => Handler::ReturnToBuildingF,
        ZtCall::PlaySetTank => Handler::PlaySetTankF,
    }
}

/// `ZTAIMgr::executeCall`: dispatches the ZT-specific names; anything else goes to the `BFAIMgr` chain
/// ([`execute_base_call`]) with the uncast unit. Afterwards the call's result flag is copied to `done`. Returns
/// the handler's own result (vanilla leaves it in `AL`, and `BFBehaviorSet::executeCall` reads it).
pub unsafe fn execute_call(this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    unsafe { execute_call_with(&VanillaHandlers, this, call, unit, done) }
}

/// [`execute_call`] over an injectable handler table. ZT handlers receive the unit only if its type is a
/// `ZTUnitType`, else null.
pub unsafe fn execute_call_with(handlers: &dyn Handlers, this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    let name = unsafe { &*(call as *const RawCallString) }.bytes();
    let unit_cast = if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } { unit } else { 0 };
    let result = match lookup_zt_call(name) {
        Some(which) => (unsafe { handlers.invoke(zt_handler(which), this, call, unit_cast, [0, 0]) }) as u8,
        None => unsafe { execute_base_call_with(handlers, this, call, unit, done) },
    };
    unsafe { done.write_volatile(get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET)) };
    result
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
    unsafe { execute_base_call_with(&VanillaHandlers, this, call, unit, done) }
}

/// [`execute_base_call`] over an injectable handler table.
pub unsafe fn execute_base_call_with(handlers: &dyn Handlers, this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    let name = unsafe { &*(call as *const RawCallString) }.bytes();
    let unit_cast = if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } { unit } else { 0 };
    let result = match lookup_base_call(name) {
        Some(which) => unsafe { dispatch_base_call(handlers, this, which, call, unit, unit_cast) },
        None => 0,
    };
    unsafe { done.write_volatile(get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET)) };
    result
}

/// How a base call name reaches its handler: which handler, whether it gets the cast unit (move, fly and halt
/// handlers) or the unit as given (the play family), and the constant trailing arguments.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BaseRoute {
    pub handler: Handler,
    pub cast_unit: bool,
    pub trailing: [u32; 2],
}

/// The route for a base call name; `None` for the two names the dispatcher handles itself
/// (`fPlaySetTerrain`, through the manager's vtable, and `fKeepControl`).
pub fn base_route(which: BaseCall) -> Option<BaseRoute> {
    let route = |handler, cast_unit, trailing| Some(BaseRoute { handler, cast_unit, trailing });
    match which {
        BaseCall::Play => route(Handler::PlayF, false, [0, 0]),
        BaseCall::SetBase => route(Handler::SetBaseF, false, [0, 0]),
        BaseCall::PlayWithBase => route(Handler::PlayWithBaseF, false, [0, 0]),
        BaseCall::MoveEntityDelta => route(Handler::MoveEntityDeltaF, false, [0, 0]),
        BaseCall::MoveEntitySpecial => route(Handler::MoveEntitySpecialF, false, [0, 0]),
        BaseCall::PlayTime => route(Handler::PlayTimeF, false, [0, 0]),
        BaseCall::PlaySound => route(Handler::PlaySoundF, false, [0, 0]),
        BaseCall::PlaySpecial => route(Handler::PlaySpecialF, false, [0, 0]),
        BaseCall::PlaySetProb => route(Handler::PlaySetProbF, false, [0, 0]),
        BaseCall::PlaySetTerrain => None,
        BaseCall::PlayReverse => route(Handler::PlayReverseF, false, [0, 0]),
        BaseCall::PlayPingPong => route(Handler::PlayPingPongF, false, [0, 0]),
        BaseCall::PlayWithSound => route(Handler::PlayWithSoundF, false, [0, 0]),
        BaseCall::PlayStopAtEnd => route(Handler::PlayStopAtEndF, false, [0, 0]),
        BaseCall::PlaySet => route(Handler::PlaySetF, false, [0, 0]),
        BaseCall::MoveSlow => route(Handler::MoveSlowF, true, [0, 0]),
        BaseCall::MoveMedium => route(Handler::MoveMediumF, true, [0, 0]),
        BaseCall::Move => route(Handler::MoveF, true, [0, 0]),
        BaseCall::MoveReverse => route(Handler::MoveReverseF, true, [0, 0]),
        BaseCall::MoveFast => route(Handler::MoveFastF, true, [0, 0]),
        BaseCall::MoveWithSound => route(Handler::MoveWithSoundF, true, [0, 0]),
        BaseCall::FaceDirection => route(Handler::FaceDirectionF, false, [0, 0]),
        BaseCall::Fly => route(Handler::FlyF, true, [0, 0]),
        BaseCall::FlyReverse => route(Handler::FlyReverseF, true, [0, 0]),
        BaseCall::KeepControl => None,
        BaseCall::Halt => route(Handler::HaltF, true, [0, 0]),
    }
}

unsafe fn dispatch_base_call(handlers: &dyn Handlers, this: u32, which: BaseCall, call: u32, unit: u32, unit_cast: u32) -> u8 {
    if let Some(route) = base_route(which) {
        let target_unit = if route.cast_unit { unit_cast } else { unit };
        return unsafe { handlers.invoke(route.handler, this, call, target_unit, route.trailing) } as u8;
    }
    match which {
        BaseCall::PlaySetTerrain => unsafe {
            let vtable = get_from_memory::<u32>(this);
            let target = get_from_memory::<u32>(vtable + BFAIMGR_VTABLE_PLAY_SET_TERRAIN);
            mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target)(this, call, unit) as u8
        },
        _ => (unit_cast != 0) as u8,
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
    unsafe { BFAIMGR_CONSTRUCTOR.hooked()(this as *const u32) };
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(this, base + RVA_ZTAIMGR_VTABLE);
    save_to_memory(this + 8, base + RVA_ZTAIMGR_LISTENER_TABLE);
    unsafe { call_virtual_args(this, AI_VTABLE_INIT_AI_PARAMS, []) };
    this
}

/// `BFAIMgr`'s vtable and listener table (RVAs), and the `BFMgr` vtable its destructor restores.
const RVA_BFAIMGR_VTABLE: u32 = 0x0023_536c;
const RVA_BFAIMGR_LISTENER_TABLE: u32 = 0x0023_2270;
const RVA_BFMGR_VTABLE: u32 = 0x0023_55c0;
/// Manager fields the constructor and destructor touch: the `GXLLEAnimSet` pointer, the embedded `BFEventMgr`
/// (`+0xa0`; second event vector `+0xac`, listener tree `+0xc0`), and the path cache (`+0x12c`: head, size at
/// `+0x130`, one flag byte at `+0x134`).
const OFFSET_ANIM_SET: u32 = 0xd0;
const OFFSET_EVENT_VECTOR_B: u32 = 0xac;
const OFFSET_LISTENER_TREE: u32 = 0xc0;
const OFFSET_PATH_CACHE_SIZE: u32 = 0x130;
const OFFSET_PATH_CACHE_FLAG: u32 = 0x134;
/// Pool freelist bucket for sizes `9..=0x18` (`DAT_00638008`): tree heads of `0x14`/`0x18` bytes.
const RVA_POOL_FREELIST_BUCKET_2: u32 = 0x0023_8008;
/// `GXLLEAnimSet` allocation size, and its animation elements (`+0x10` begin, `+0x14` end, `+0x18` capacity end).
const ANIM_SET_SIZE: u32 = 0x28;
const ANIM_ELEMENT_SIZE: u32 = 0x1c;
const PATH_CACHE_HEAD_SIZE: u32 = 0x18;

/// `BFAIMgr::BFAIMgr()` (`0x00525e90`): constructs the embedded `BFPathFinder` and `BFEventMgr`, builds the empty
/// path-cache tree (head from `PoolAlloc`, size `0`), installs the `BFAIMgr` vtable and listener table (the
/// `BFMgr` ones vanilla writes first are overwritten before anything reads them), allocates a zeroed
/// `GXLLEAnimSet` (`0x28` bytes, `operator new`) into `+0xd0`, and runs `BFAIMgr::init`. A failed allocation
/// stores a null set. `+0x134` is a stale stack byte in vanilla; it is zeroed here.
pub unsafe fn bfaimgr_construct(this: u32) -> u32 {
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(this + 0x4, 0u8);
    unsafe {
        BFPATHFINDER_CONSTRUCTOR.original()((this + OFFSET_PATHFINDER) as *const u32, 0, 2);
        BFEVENTMGR_CONSTRUCTOR.original()((this + OFFSET_EVENT_MGR) as *const u32);
    }
    let head = unsafe { POOLALLOC_ALLOCATE.original()(PATH_CACHE_HEAD_SIZE) } as u32;
    save_to_memory(this + OFFSET_PATH_CACHE, head);
    save_to_memory(this + OFFSET_PATH_CACHE_SIZE, 0u32);
    save_to_memory(this + OFFSET_PATH_CACHE_FLAG, 0u8);
    save_to_memory(head, 0u8);
    save_to_memory(head + 4, 0u32);
    save_to_memory(head + 8, head);
    save_to_memory(head + 0xc, head);
    save_to_memory(this, base + RVA_BFAIMGR_VTABLE);
    save_to_memory(this + 8, base + RVA_BFAIMGR_LISTENER_TABLE);
    let anim_set = unsafe { OPERATOR_NEW.original()(ANIM_SET_SIZE) } as u32;
    if anim_set != 0 {
        for offset in (0x10..=0x20).step_by(4) {
            save_to_memory(anim_set + offset, 0u32);
        }
        save_to_memory(anim_set + 0x24, 0u8);
        save_to_memory(anim_set + 0x25, 0u8);
    }
    save_to_memory(this + OFFSET_ANIM_SET, anim_set);
    unsafe { bfaimgr_init(this) };
    this
}

/// Returns a tree head of the `0x14`/`0x18` size class to vanilla's pool freelist (null is skipped).
fn pool_free_bucket_2(node: u32) {
    if node == 0 {
        return;
    }
    let freelist = get_module_base("zoo.exe") as u32 + RVA_POOL_FREELIST_BUCKET_2;
    save_to_memory(node, get_from_memory::<u32>(freelist));
    save_to_memory(freelist, node);
}

/// `BFAIMgr::~BFAIMgr()` (`0x0050374b`): drops back to the `BFAIMgr` vtable, releases and frees the
/// `GXLLEAnimSet` (`release`, the element destructor loop over what `release` left, the element buffer through
/// `PoolAlloc::deallocate_n_28`, then `operator delete`), clears the path cache and returns its head to the
/// freelist, runs the inlined `BFEventMgr` teardown (listener tree cleared and its head freed, both event
/// vectors), destroys the `BFPathFinder`, and ends on the `BFMgr` vtable.
pub unsafe fn bfaimgr_destruct(this: u32) -> u32 {
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(this, base + RVA_BFAIMGR_VTABLE);
    save_to_memory(this + 8, base + RVA_BFAIMGR_LISTENER_TABLE);
    let anim_set = get_from_memory::<u32>(this + OFFSET_ANIM_SET);
    if anim_set != 0 {
        unsafe { GXLLEANIMSET_RELEASE.original()(anim_set as *const c_void) };
        let begin = get_from_memory::<u32>(anim_set + 0x10);
        let end = get_from_memory::<u32>(anim_set + 0x14);
        let mut element = begin;
        while element != end {
            unsafe { GXLLEANIM_DESTRUCTOR.original()(element as *const u32) };
            element += ANIM_ELEMENT_SIZE;
        }
        let capacity = (get_from_memory::<u32>(anim_set + 0x18).wrapping_sub(begin) as i32) / ANIM_ELEMENT_SIZE as i32;
        unsafe { DEALLOCATE_N_28.original()(begin as *const u32, capacity) };
        unsafe { OPERATOR_DELETE.original()(anim_set) };
    }
    crate::vanilla_rbtree::rb_tree_clear(this + OFFSET_PATH_CACHE, PATH_CACHE_NODE_SIZE);
    pool_free_bucket_2(get_from_memory::<u32>(this + OFFSET_PATH_CACHE));
    unsafe { TREE20_CLEAR.original()((this + OFFSET_LISTENER_TREE) as *const u32) };
    pool_free_bucket_2(get_from_memory::<u32>(this + OFFSET_LISTENER_TREE));
    unsafe {
        VECTOR_T_28.original()((this + OFFSET_EVENT_VECTOR_B) as *const u32);
        VECTOR_T_28.original()((this + OFFSET_EVENT_MGR) as *const u32);
        BFPATHFINDER_DESTRUCTOR.original()((this + OFFSET_PATHFINDER) as *const u32);
    }
    save_to_memory(this, base + RVA_BFMGR_VTABLE);
    this
}

/// `BFAIMgr`'s deleting destructor (`0x00503857`, vtable slot 0): runs the destructor, then frees the object
/// with `operator delete` when bit 0 of `flags` is set. Returns `this`.
pub unsafe fn bfaimgr_deleting_destruct(this: u32, flags: u8) -> u32 {
    unsafe { bfaimgr_destruct(this) };
    if flags & 1 != 0 {
        unsafe { OPERATOR_DELETE.original()(this) };
    }
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

/// `BFAIMgr::fPlaySet(name, unit, 1, 1)` (stdcall; it ignores `ECX`).
unsafe fn play_set_vanilla(name: u32, unit: u32) -> u32 {
    u32::from(unsafe { F_PLAY_SET.hooked()(name as *const i32, unit as *const c_void, 1, 1) })
}

unsafe fn type_find_anim_vanilla(entity_type: u32, name: *const u8) -> bool {
    !unsafe { BFENTITYTYPE_FIND_ANIM.original()(entity_type as *const u32, name.cast()) }.is_null()
}

unsafe fn type_find_sound_vanilla(entity_type: u32, name: *const u8) -> bool {
    !unsafe { BFENTITYTYPE_FIND_SOUND.original()(entity_type as *const u32, name.cast()) }.is_null()
}

/// The string vanilla stores for "no animation / no sound" (`DAT_0063ba4c`).
const NO_NAME: &[u8] = b"0";
/// `ZTUnit` selector `fDustBall` reads (`3` surface ball, `4` submerged ball).
const UNIT_BALL_SELECTOR_OFFSET: u32 = 0x258;

/// The vanilla calls `fDustBall` makes, so a test can substitute recording stand-ins.
pub struct DustBallOps {
    pub tile_flags: unsafe fn(u32) -> u8,
    /// `BFEntityType::findAnim(type, name)`.
    pub find_anim: unsafe fn(u32, *const u8) -> bool,
    /// `BFEntityType::findSound(type, name)`.
    pub find_sound: unsafe fn(u32, *const u8) -> bool,
    /// `BFAIMgr::fPlaySpecial(this, anim, sound, 0, 1, extra, unit, done)` with vanilla strings.
    pub play_special: unsafe fn(u32, u32, u32, u32, u32, u32) -> bool,
    /// `fCaughtPrey(unit, done)`.
    pub caught_prey: unsafe fn(u32, *const bool),
}

/// `ZTAIMgr::fDustBall(unit, done)`: plays the unit's dust/water ball effect. The effect name is `surfaceball`
/// or `submergedball` for selectors `3`/`4`, else `dustball` on land and `waterball` on water. The animation
/// and sound names are that name when the unit's type has it, else `dustball` when it has that, else `"0"`
/// (none); `fPlaySpecial` plays them and, if it left the done flag set, the unit catches its prey.
///
/// `None` (defer to vanilla) for a null or non-`ZTUnitType` unit, where vanilla reads the selector through null.
pub unsafe fn f_dust_ball(this: u32, unit: u32, done: *const bool, ops: &DustBallOps) -> Option<bool> {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        return None;
    }
    let base: &'static CStr = match get_from_memory::<u32>(unit + UNIT_BALL_SELECTOR_OFFSET) {
        3 => c"surfaceball",
        4 => c"submergedball",
        _ if unsafe { (ops.tile_flags)(unit) } & 3 == 0 => c"dustball",
        _ => c"waterball",
    };
    let entity_type = get_from_memory::<u32>(unit + OFFSET_ENTITY_TYPE);
    let pick = |find: unsafe fn(u32, *const u8) -> bool| -> &'static [u8] {
        if unsafe { find(entity_type, base.as_ptr().cast()) } {
            base.to_bytes()
        } else if unsafe { find(entity_type, c"dustball".as_ptr().cast()) } {
            b"dustball"
        } else {
            NO_NAME
        }
    };
    let anim = VanillaString::from_bytes(pick(ops.find_anim));
    let sound = VanillaString::from_bytes(pick(ops.find_sound));
    let extra = VanillaString::from_bytes(NO_NAME);
    let result = unsafe { (ops.play_special)(this, anim.as_ptr() as u32, sound.as_ptr() as u32, extra.as_ptr() as u32, unit, done as u32) };
    drop(extra);
    if unsafe { done.cast::<u8>().read_volatile() } != 0 {
        unsafe { (ops.caught_prey)(unit, done) };
    }
    Some(result)
}

unsafe fn play_special_vanilla(this: u32, anim: u32, sound: u32, extra: u32, unit: u32, done: u32) -> bool {
    unsafe { F_PLAY_SPECIAL.hooked()(this as *const u32, anim as *const i8, sound as *const i8, 0, 1, extra as *const i8, unit as *const i32, done as *const u8) }
}

unsafe fn caught_prey_hooked(unit: u32, done: *const bool) {
    unsafe { F_CAUGHT_PREY.hooked()(unit as *const i32, done as *const u8) };
}

const DUST_BALL_VANILLA: DustBallOps = DustBallOps {
    tile_flags: unit_tile_flags_vanilla,
    find_anim: type_find_anim_vanilla,
    find_sound: type_find_sound_vanilla,
    play_special: play_special_vanilla,
    caught_prey: caught_prey_hooked,
};

/// `ZTAnimal` byte `fTargetBuildingF` tests before calling `fTargetBuilding`.
const ANIMAL_TARGETING_FLAG_OFFSET: u32 = 0x3b0;
/// `ZTAnimal` byte recording that a building target was acquired.
const ANIMAL_TARGET_ACQUIRED_OFFSET: u32 = 0x3b1;
/// `ZTUnitType` dword passed to `fMove` as its speed argument.
const UNIT_TYPE_MOVE_SPEED_OFFSET: u32 = 0x108;
/// Entity vtable slot returning the animal's target-building name (a `{begin, end}` string).
const ENTITY_VTABLE_TARGET_BUILDING_NAME: u32 = 0x128;
/// `fMove`'s trailing constants for a building walk.
const TARGET_BUILDING_MOVE_FLAG: u32 = 0;
const TARGET_BUILDING_MOVE_LIMIT: u32 = 100;

/// The vanilla calls `fTargetBuilding` makes, so a test can substitute recording stand-ins.
pub struct TargetBuildingOps {
    /// `ZTAnimal::targetBuilding(animal, true)`.
    pub target_building: unsafe fn(u32) -> bool,
    /// `BFAIMgr::fMove(this, none, 0, unit, done, name, speed, empty, 0, 100)` with vanilla strings.
    pub f_move: unsafe fn(u32, u32, u32, u32, u32, u32, u32) -> bool,
}

/// `ZTAIMgr::fTargetBuilding(unit, done)`: finishes immediately (`done` set, result `0`) for a non-animal; for
/// an animal without an acquired target, tries to acquire one (finishing with result `1` if that fails). Once a
/// target is acquired, a `ZTUnitType` animal walks to the building named by its virtual `+0x128` through
/// `fMove`, with the type's speed; any other type finishes with result `0`. Never defers.
pub unsafe fn f_target_building(this: u32, unit: u32, done: *const bool, ops: &TargetBuildingOps) -> bool {
    if unit == 0 || !unsafe { is_animal(unit) } {
        unsafe { set_done(done) };
        return false;
    }
    if get_from_memory::<u8>(unit + ANIMAL_TARGET_ACQUIRED_OFFSET) == 0 {
        if !unsafe { (ops.target_building)(unit) } {
            save_to_memory(unit + ANIMAL_TARGET_ACQUIRED_OFFSET, 0u8);
            unsafe { set_done(done) };
            return true;
        }
        save_to_memory(unit + ANIMAL_TARGET_ACQUIRED_OFFSET, 1u8);
    }
    let entity_type = get_from_memory::<u32>(unit + OFFSET_ENTITY_TYPE);
    if entity_type == 0 || !unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        unsafe { set_done(done) };
        save_to_memory(unit + ANIMAL_TARGET_ACQUIRED_OFFSET, 0u8);
        return false;
    }
    let name = unsafe { call_entity_virtual_ret(unit, ENTITY_VTABLE_TARGET_BUILDING_NAME) };
    let speed = get_from_memory::<u32>(entity_type + UNIT_TYPE_MOVE_SPEED_OFFSET);
    let none = VanillaString::from_bytes(NO_NAME);
    let empty = EmptyName::new();
    unsafe { (ops.f_move)(this, none.as_ptr() as u32, unit, done as u32, name, speed, empty.as_ptr()) }
}

/// `ZTAIMgr::fTargetBuildingF(call, unit)`: finishes with result `0` for a non-animal; for an animal whose
/// `+0x3b0` flag is set runs [`f_target_building`] on the call's result flag; otherwise clears the acquired
/// flag, finishes and returns `1`.
pub unsafe fn f_target_building_f(this: u32, call: u32, unit: u32, ops: &TargetBuildingOps) -> bool {
    let done = call_done_flag(call);
    if unit == 0 || !unsafe { is_animal(unit) } {
        unsafe { set_done(done) };
        return false;
    }
    if get_from_memory::<u8>(unit + ANIMAL_TARGETING_FLAG_OFFSET) != 0 {
        return unsafe { f_target_building(this, unit, done, ops) };
    }
    unsafe { set_done(done) };
    save_to_memory(unit + ANIMAL_TARGET_ACQUIRED_OFFSET, 0u8);
    true
}

/// Which of the three fixed-speed `fMove` wrappers is running. Each reads the same members at consecutive
/// slots/offsets: slow, medium, fast.
#[derive(Clone, Copy)]
pub enum MoveSpeed {
    Slow = 0,
    Medium = 1,
    Fast = 2,
}

/// Entity vtable slot of the slow variant's move-name string (`{begin, end}`); medium/fast follow at `+4`/`+8`.
const ENTITY_VTABLE_MOVE_NAME_SLOW: u32 = 0x120;
/// Entity vtable slot of the slow variant's move limit; medium/fast follow at `+4`/`+8`.
const ENTITY_VTABLE_MOVE_LIMIT_SLOW: u32 = 0x140;
/// `ZTUnitType` speed dword of the slow variant; medium/fast follow at `+4`/`+8`.
const UNIT_TYPE_SPEED_SLOW_OFFSET: u32 = 0x100;

/// The vanilla call `fMoveSlowF`/`fMoveMediumF`/`fMoveFastF` make, so a test can substitute a recorder.
/// `BFAIMgr::fMove(this, first_param, seconds, unit, done, name, speed, empty, 0, limit)`.
pub struct MoveSpeedOps {
    pub f_move: unsafe fn(u32, u32, u32, u32, u32, u32, u32, u32, u32) -> bool,
}

/// `BFAIMgr::fMoveSlowF`/`fMoveMediumF`/`fMoveFastF(call, unit)`: needs exactly two parameters, the first
/// non-empty, a non-null unit and a `ZTUnitType` type; converts the second parameter through the manager's
/// vtable `+0x34`, passes the unit's per-speed move name as is and calls `fMove` with the
/// type's per-speed speed, an empty string (vanilla builds a `vector<byte>` with capacity 8 and size 0, which
/// `fMove` reads the same way), `0`, and the unit's per-speed limit. Every failed gate returns `false`
/// without side effects; vanilla never defers.
pub unsafe fn f_move_speed_f(this: u32, call: u32, unit: u32, speed: MoveSpeed, ops: &MoveSpeedOps) -> bool {
    if unit == 0 || unsafe { call_param_count(call) } != 2 {
        return false;
    }
    let begin = unsafe { call_params_begin(call) };
    if unsafe { &*(begin as *const RawCallString) }.bytes().is_empty() {
        return false;
    }
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + AI_VTABLE_CONVERT_CONSTANT_PARAM);
    let convert = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target) };
    let seconds = convert(this, begin + CALL_PARAM_SIZE, unit);
    let entity_type = get_from_memory::<u32>(unit + OFFSET_ENTITY_TYPE);
    if entity_type == 0 || !unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        return false;
    }
    let step = speed as u32 * 4;
    let name = unsafe { call_entity_virtual_ret(unit, ENTITY_VTABLE_MOVE_NAME_SLOW + step) };
    let speed_value = get_from_memory::<u32>(entity_type + UNIT_TYPE_SPEED_SLOW_OFFSET + step);
    let limit = unsafe { call_entity_virtual_ret(unit, ENTITY_VTABLE_MOVE_LIMIT_SLOW + step) };
    let empty = EmptyName::new();
    unsafe { (ops.f_move)(this, begin, seconds, unit, call + CALL_DONE_FLAG_OFFSET, name, speed_value, empty.as_ptr(), limit) }
}

unsafe fn f_move_vanilla_speed(this: u32, first: u32, seconds: u32, unit: u32, done: u32, name: u32, speed: u32, empty: u32, limit: u32) -> bool {
    unsafe { F_MOVE.hooked()(this as *const u32, first as i32, seconds as i32, unit as *const i32, done as *const u8, name as i32, speed, empty as *const i8, 0, limit) }
}

const MOVE_SPEED_VANILLA: MoveSpeedOps = MoveSpeedOps { f_move: f_move_vanilla_speed };

/// The vanilla call `fMoveF`/`fMoveWithSoundF` make, so a test can substitute a recorder.
/// `BFAIMgr::fMove(this, first_param, a2, unit, done, name, speed, extra, reverse, limit)`; `name` and `extra`
/// are vanilla strings.
pub struct MoveOps {
    pub f_move: unsafe fn(u32, u32, u32, u32, u32, u32, u32, u32, u32, u32) -> bool,
}

unsafe fn f_move_vanilla_full(this: u32, first: u32, a2: u32, unit: u32, done: u32, name: u32, speed: u32, extra: u32, reverse: u32, limit: u32) -> bool {
    unsafe { F_MOVE.hooked()(this as *const u32, first as i32, a2 as i32, unit as *const i32, done as *const u8, name as i32, speed, extra as *const i8, reverse, limit) }
}

const MOVE_VANILLA: MoveOps = MoveOps { f_move: f_move_vanilla_full };

/// `fMove` limit when the call carries no limit parameter.
const DEFAULT_MOVE_LIMIT: u32 = 100;
/// Parameter-entry offsets read by `fMoveF`/`fMoveWithSoundF`.
const PARAM_1: u32 = CALL_PARAM_SIZE;
const PARAM_2: u32 = CALL_PARAM_SIZE * 2;
const PARAM_3: u32 = CALL_PARAM_SIZE * 3;
const PARAM_4: u32 = CALL_PARAM_SIZE * 4;
const PARAM_5: u32 = CALL_PARAM_SIZE * 5;

unsafe fn convert_param(this: u32, param: u32, unit: u32) -> u32 {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + AI_VTABLE_CONVERT_CONSTANT_PARAM);
    let convert = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target) };
    convert(this, param, unit)
}

unsafe fn param_string(param: u32) -> VanillaString {
    VanillaString::from_bytes(unsafe { &*(param as *const RawCallString) }.bytes())
}

/// An empty `{begin, end, cap}` string over a static NUL byte. Consumers only read `begin`/`end` (and `begin`
/// as a C string), so it stands in for a vanilla empty string without a vanilla allocation.
struct EmptyName([u32; 3]);

static EMPTY_NAME_BYTE: u8 = 0;

impl EmptyName {
    fn new() -> Self {
        let byte = &EMPTY_NAME_BYTE as *const u8 as u32;
        EmptyName([byte, byte, byte])
    }

    fn as_ptr(&self) -> u32 {
        self.0.as_ptr() as u32
    }
}

/// `BFAIMgr::fMoveF(call, unit, reverse)`: four or five parameters, the first non-empty, a non-null unit.
/// Converts parameter 1 (times 1000), 3 and (when present) 4, passes parameter 2 as is and
/// calls `fMove(params, ms, unit, done, name, speed, empty, reverse, limit)` with `limit` defaulting to 100.
/// Vanilla also builds a string from the unit's `+0x120` virtual and tests the type twice; neither result is
/// used, so the port skips them. Failed gates return false.
pub unsafe fn f_move_f(this: u32, call: u32, unit: u32, reverse: bool, ops: &MoveOps) -> bool {
    if unit == 0 {
        return false;
    }
    let count = unsafe { call_param_count(call) } as u32;
    let begin = unsafe { call_params_begin(call) };
    if !(4..=5).contains(&count) || unsafe { &*(begin as *const RawCallString) }.bytes().is_empty() {
        return false;
    }
    let millis = unsafe { convert_param(this, begin + PARAM_1, unit) }.wrapping_mul(MILLIS_PER_SECOND);
    let name = begin + PARAM_2;
    let speed = unsafe { convert_param(this, begin + PARAM_3, unit) };
    let limit = if count == 5 { unsafe { convert_param(this, begin + PARAM_4, unit) } } else { DEFAULT_MOVE_LIMIT };
    let empty = EmptyName::new();
    unsafe { (ops.f_move)(this, begin, millis, unit, call + CALL_DONE_FLAG_OFFSET, name, speed, empty.as_ptr(), u32::from(reverse), limit) }
}

/// `BFAIMgr::fMoveWithSoundF(call, unit)`: five or six parameters, the first non-empty, a non-null unit.
/// Converts parameter 1 (seconds, unscaled), 3 and (when present) 5, passes parameters 2 and 4 as is as
/// strings and calls `fMove(params, seconds, unit, done, name, speed, sound, 0, limit)`.
pub unsafe fn f_move_with_sound_f(this: u32, call: u32, unit: u32, ops: &MoveOps) -> bool {
    if unit == 0 {
        return false;
    }
    let count = unsafe { call_param_count(call) } as u32;
    let begin = unsafe { call_params_begin(call) };
    if !(5..=6).contains(&count) || unsafe { &*(begin as *const RawCallString) }.bytes().is_empty() {
        return false;
    }
    let seconds = unsafe { convert_param(this, begin + PARAM_1, unit) };
    let name = begin + PARAM_2;
    let speed = unsafe { convert_param(this, begin + PARAM_3, unit) };
    let sound = begin + PARAM_4;
    let limit = if count == 6 { unsafe { convert_param(this, begin + PARAM_5, unit) } } else { DEFAULT_MOVE_LIMIT };
    unsafe { (ops.f_move)(this, begin, seconds, unit, call + CALL_DONE_FLAG_OFFSET, name, speed, sound, 0, limit) }
}

/// The vanilla mixer calls the animation-link helpers make, so a test can substitute recorders: `fPlay`, the
/// link's duration, `GXMixer::clearAll`/`setSet`/`setBase`.
pub(crate) struct MixerOps {
    pub(crate) ai_play: unsafe fn(u32, u32, u32, u32, bool) -> bool,
    pub(crate) duration: unsafe fn(u32) -> i32,
    pub(crate) clear_all: unsafe fn(u32),
    pub(crate) set_set: unsafe fn(u32, u32, u8),
    pub(crate) set_base: unsafe fn(u32, u32),
}

unsafe fn link_duration_vanilla(link: u32) -> i32 {
    unsafe { LINK_DURATION.original()(link as *const u32, 0) }
}

unsafe fn mixer_clear_all_vanilla(mixer: u32) {
    unsafe { GXMIXER_CLEAR_ALL.hooked()(mixer as *const u32) };
}

unsafe fn mixer_set_set_vanilla(mixer: u32, set: u32, out: u8) {
    unsafe { GXMIXER_SET_SET.hooked()(mixer as *const u32, set, out) };
}

unsafe fn mixer_set_base_vanilla(mixer: u32, vector: u32) {
    unsafe { GXMIXER_SET_BASE.hooked()(mixer as *const u32, vector as *const i32, 1) };
}

pub(crate) const VANILLA_MIXER: MixerOps =
    MixerOps { ai_play, duration: link_duration_vanilla, clear_all: mixer_clear_all_vanilla, set_set: mixer_set_set_vanilla, set_base: mixer_set_base_vanilla };

/// `BFAIMgr::fMove` (`0x0043646e`) unit fields. Flag bytes: `+0xf8` moving, `+0xf9` stopped/arrived,
/// `+0xd7`, `+0x20c`, `+0x20d` (finished at once), `+0x20e` (cog goal invalid). `+0x154` is the pointer to the
/// target list's sentinel node, `+0x15c` the embedded `BFCogGoal`, `+0x170`/`+0x1bc` state words, `+0x74` the
/// `GXMixer` and `+0x8c`/`+0x9c` a pair of values compared before restarting a move.
const UNIT_MOVING: u32 = 0xf8;
const UNIT_STOPPED: u32 = 0xf9;
const UNIT_FLAG_D7: u32 = 0xd7;
const UNIT_FLAG_20C: u32 = 0x20c;
const UNIT_FINISHED: u32 = 0x20d;
const UNIT_COG_INVALID: u32 = 0x20e;
const UNIT_TARGET_LIST: u32 = 0x154;
const UNIT_COG_GOAL: u32 = 0x15c;
const UNIT_STATE: u32 = 0x170;
const UNIT_STATE_2: u32 = 0x1bc;
const UNIT_MIXER: u32 = 0x74;
const UNIT_MIXER_FLAG: u32 = 0xd4;
const UNIT_POS_A: u32 = 0x8c;
const UNIT_POS_B: u32 = 0x9c;
const COG_GOAL_FLAG_OFFSET: u32 = 0x4c;
/// Entity vtable slots `fMove` calls.
const ENTITY_VT_HAS_TARGET: u32 = 0x180;
const ENTITY_VT_CAN_PLAY: u32 = 0x184;
const ENTITY_VT_PREPARE: u32 = 0x1b0;
const ENTITY_VT_FIND_ANIM: u32 = 0x130;
const ENTITY_VT_FIND_ANIM_SECOND: u32 = 0x134;
const ENTITY_VT_SET_SPEED: u32 = 0x1c0;
const ENTITY_VT_SET_LIMIT: u32 = 0x1c4;
/// `BFCogGoal` vtable slots: `valid`, `clearTarget`, `clearTargetSub`.
const COG_VT_VALID: u32 = 0x0;
const COG_VT_CLEAR_TARGET: u32 = 0xc;
const COG_VT_CLEAR_TARGET_SUB: u32 = 0x1c;
/// `BFAIMgr::fPlay(this, second, first, unit, scratch, with_base)`. Vanilla writes `1` to `scratch` and
/// returns `true`; nothing reads the scratch byte, so it lives here.
unsafe fn ai_play(this: u32, second: u32, first: u32, unit: u32, with_base: bool) -> bool {
    let mut scratch = false;
    unsafe { BFAIMGR_F_PLAY.original()(this as *const u32, second as *const c_void, first as *const c_void, unit as *const u32, &mut scratch, with_base) }
}
/// `GLOBAL_BFAnimCache` cell and the string cell `fMove` compares the extra string against.
const RVA_ANIM_CACHE: u32 = 0x0023_8044;
const RVA_NO_SOUND_STRING: u32 = 0x0023_950c;
const ANIM_REFCOUNT_OFFSET: u32 = 0x20;
const ANIM_STAMP_OFFSET: u32 = 0x1c;
const ANIM_CACHE_STAMP_OFFSET: u32 = 0x20;
const LINK_SIZE: u32 = 0xc;

/// The arguments of `BFAIMgr::fMove` in stack order.
pub struct FMoveArgs {
    pub this: u32,
    pub first_param: u32,
    pub seconds: i32,
    pub unit: u32,
    pub done: *const u8,
    pub name: u32,
    pub speed: u32,
    pub extra: u32,
    pub reverse: u32,
    pub limit: u32,
}

/// The vanilla pieces `fMove` leans on, so a test can substitute recorders. The animation-link and mixer work
/// (`GXMixerLink`s in `PoolAlloc`-backed vectors) lives behind `play_single` and `play_pair`.
pub struct FMoveOps {
    pub stop: unsafe fn(u32, i8, i8),
    pub set_moving: unsafe fn(u32, i8, i8),
    pub reached_target: unsafe fn(u32) -> bool,
    /// The extra string equals vanilla's "no sound" string.
    pub is_no_sound: unsafe fn(u32) -> bool,
    pub find_sound: unsafe fn(u32, u32) -> u32,
    /// `(unit, anim, flag, sound)`: sets the unit's mixer base to a single link.
    pub play_single: unsafe fn(u32, u32, u8, u32),
    /// `(this, unit, first_anim, second_anim, flag, sound, seconds)`: builds the two link vectors, repeats the
    /// second link to fill `seconds` and plays them; the result is `fPlay`'s low byte.
    pub play_pair: unsafe fn(u32, u32, u32, u32, u8, u32, i32) -> bool,
}

unsafe fn vcall0(object: u32, slot: u32) -> u32 {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(object) + slot);
    let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target) };
    method(object)
}

unsafe fn vcall1(object: u32, slot: u32, arg: u32) -> u32 {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(object) + slot);
    let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(target) };
    method(object, arg)
}

/// Number of nodes between the target list's first node and its sentinel.
unsafe fn target_list_count(unit: u32) -> u32 {
    let sentinel = get_from_memory::<u32>(unit + UNIT_TARGET_LIST);
    let mut node = get_from_memory::<u32>(sentinel);
    let mut count = 0;
    while node != sentinel {
        count += 1;
        node = get_from_memory::<u32>(node);
    }
    count
}

/// Vanilla ends `fMove` through the same four-write sequence in several places.
unsafe fn finish_stopped(unit: u32, done: *const u8) {
    unsafe { vcall1(unit, ENTITY_VT_SET_SPEED, 0) };
    save_to_memory(unit + UNIT_STOPPED, 1u8);
    unsafe { (done as *mut u8).write_volatile(1) };
    save_to_memory(unit + UNIT_MOVING, 0u8);
}

/// `BFAIMgr::fMove`: drives a unit's movement state machine and starts its move animation. Returns the low byte
/// vanilla leaves in `AL`.
///
/// The paths, in order: a finished unit completes at once; a stopped unit with an empty target list that can
/// play re-checks its cog goal (an invalid goal sets `+0x20e` and clears the done flag); a stopped unit with
/// targets starts the move by resolving the animation (and, for a nonzero duration, a second one), looking the
/// extra string up as a sound, mixing the animations and storing the speed and limit; a moving unit with too
/// few targets, or a reached target, stops; otherwise it keeps moving. Vanilla also builds a `vector<byte>`
/// argument the port does not need.
pub unsafe fn f_move(args: &FMoveArgs, ops: &FMoveOps) -> bool {
    let (unit, done) = (args.unit, args.done);
    let cog = unit + UNIT_COG_GOAL;
    let state = get_from_memory::<u32>(unit + UNIT_STATE);
    let has_target = unsafe { vcall0(unit, ENTITY_VT_HAS_TARGET) } as u8 != 0;
    if state == 1 {
        unsafe { vcall1(unit, ENTITY_VT_PREPARE, args.first_param) };
    }
    if get_from_memory::<u8>(unit + UNIT_FINISHED) != 0 {
        unsafe { (done as *mut u8).write_volatile(1) };
        return true;
    }
    let invalid_cog = || -> bool {
        save_to_memory(unit + UNIT_COG_INVALID, 1u8);
        unsafe { (done as *mut u8).write_volatile(0) };
        true
    };
    if get_from_memory::<u8>(unit + UNIT_STOPPED) != 0
        && get_from_memory::<u8>(unit + UNIT_COG_INVALID) == 0
        && unsafe { target_list_count(unit) } == 0
        && unsafe { vcall0(unit, ENTITY_VT_CAN_PLAY) } as u8 != 0
    {
        if unsafe { vcall0(cog, COG_VT_VALID) } as u8 == 0 {
            return invalid_cog();
        }
        if get_from_memory::<u8>(cog + COG_GOAL_FLAG_OFFSET) == 0 || state == 1 {
            return true;
        }
        return invalid_cog();
    }

    if get_from_memory::<u8>(unit + UNIT_STOPPED) != 0 && unsafe { target_list_count(unit) } != 0 {
        if unsafe { target_list_count(unit) } <= 1 && get_from_memory::<u32>(unit + UNIT_STATE_2) != 3 {
            unsafe { finish_stopped(unit, done) };
            unsafe { (ops.stop)(unit, 1, 1) };
            return true;
        }
        unsafe { (done as *mut u8).write_volatile(0) };
        let (anim_second, anim_first) = if args.seconds == 0 {
            (0, unsafe { vcall1(unit, ENTITY_VT_FIND_ANIM, args.name) })
        } else {
            let second = unsafe { vcall1(unit, ENTITY_VT_FIND_ANIM, args.name) };
            (second, unsafe { vcall0(unit, ENTITY_VT_FIND_ANIM_SECOND) })
        };
        if anim_first == 0 {
            return false;
        }
        let extra_empty = get_from_memory::<u32>(args.extra) == get_from_memory::<u32>(args.extra + 4);
        let sound = if unsafe { (ops.is_no_sound)(args.extra) } || extra_empty { 0 } else { unsafe { (ops.find_sound)(unit, args.extra) } };
        let flag = args.reverse as u8;
        let played = if anim_second == 0 {
            unsafe { (ops.play_single)(unit, anim_first, flag, sound) };
            true
        } else {
            unsafe { (ops.play_pair)(args.this, unit, anim_first, anim_second, flag, sound, args.seconds) }
        };
        unsafe { vcall1(unit, ENTITY_VT_SET_SPEED, args.speed) };
        unsafe { vcall1(unit, ENTITY_VT_SET_LIMIT, args.limit) };
        save_to_memory(unit + UNIT_MOVING, 1u8);
        save_to_memory(unit + UNIT_STOPPED, 0u8);
        return played;
    }

    if get_from_memory::<u8>(unit + UNIT_MOVING) == 0 {
        return true;
    }
    if has_target {
        if unsafe { target_list_count(unit) } <= 1 {
            unsafe { vcall0(cog, COG_VT_CLEAR_TARGET) };
            unsafe { vcall0(cog, COG_VT_CLEAR_TARGET_SUB) };
            save_to_memory(unit + UNIT_STOPPED, 1u8);
            unsafe { (done as *mut u8).write_volatile(1) };
            save_to_memory(unit + UNIT_MOVING, 0u8);
            return true;
        }
    } else if unsafe { target_list_count(unit) } == 0 || unsafe { (ops.reached_target)(cog) } {
        unsafe { finish_stopped(unit, done) };
        unsafe { (ops.stop)(unit, 1, 0) };
        return true;
    }
    if get_from_memory::<u8>(unit + UNIT_FLAG_20C) == 0
        && (get_from_memory::<u8>(unit + UNIT_FLAG_D7) != 0 || (args.seconds != 0 && get_from_memory::<u32>(unit + UNIT_POS_B) == get_from_memory::<u32>(unit + UNIT_POS_A)))
    {
        unsafe { (ops.set_moving)(unit, 1, 0) };
    }
    true
}

unsafe fn stop_vanilla(unit: u32, a: i8, b: i8) {
    unsafe { BFUNIT_STOP.hooked()(unit as *const u32, a, b) }
}

unsafe fn set_moving_vanilla(unit: u32, a: i8, b: i8) {
    unsafe { SET_MOVING.hooked()(unit as *const u32, a, b) }
}

unsafe fn reached_target_vanilla(cog: u32) -> bool {
    unsafe { BFCOGGOAL_REACHED_TARGET.original()(cog as *const u32) }
}

/// Whether the string at `string` has exactly the bytes of vanilla's "no sound" string.
unsafe fn is_no_sound_vanilla(string: u32) -> bool {
    let cell = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_NO_SOUND_STRING);
    if cell == 0 {
        return false;
    }
    let (begin, end) = (get_from_memory::<u32>(string), get_from_memory::<u32>(string + 4));
    let (none_begin, none_end) = (get_from_memory::<u32>(cell), get_from_memory::<u32>(cell + 4));
    if end.wrapping_sub(begin) != none_end.wrapping_sub(none_begin) {
        return false;
    }
    let len = (end - begin) as usize;
    len == 0 || unsafe { std::slice::from_raw_parts(begin as *const u8, len) == std::slice::from_raw_parts(none_begin as *const u8, len) }
}

unsafe fn find_sound_vanilla(unit: u32, string: u32) -> u32 {
    unsafe { BFENTITY_FIND_SOUND.hooked()(unit as *const u32, string as *const std::ffi::c_void) as u32 }
}

/// `GXMixerLink(anim, flag, sound)`: `{anim, sound, flag}`, taking a reference on the animation and stamping it
/// with the animation cache's current counter.
fn make_link(anim: u32, flag: u8, sound: u32) -> [u32; 3] {
    save_to_memory(anim + ANIM_REFCOUNT_OFFSET, get_from_memory::<u32>(anim + ANIM_REFCOUNT_OFFSET).wrapping_add(1));
    let cache = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_ANIM_CACHE);
    save_to_memory(anim + ANIM_STAMP_OFFSET, get_from_memory::<u32>(cache + ANIM_CACHE_STAMP_OFFSET));
    [anim, sound, u32::from(flag)]
}

fn release_anim(anim: u32) {
    if anim != 0 {
        save_to_memory(anim + ANIM_REFCOUNT_OFFSET, get_from_memory::<u32>(anim + ANIM_REFCOUNT_OFFSET).wrapping_sub(1));
    }
}

/// A `vector<GXMixerLink>` for vanilla's `fPlay`/`GXMixer::setBase`, which only read its `begin`/`end` and
/// copy the links out. The links live in a Rust buffer; `header` is the `{begin, end, cap}` triple vanilla
/// reads, refreshed after every push. Each stored link holds one animation reference (vanilla's copy
/// constructor does the same), released on drop.
struct LinkVector {
    header: [u32; 3],
    links: Vec<[u32; 3]>,
}

impl LinkVector {
    fn empty() -> Self {
        let mut vector = LinkVector { header: [0; 3], links: Vec::new() };
        vector.refresh_header();
        vector
    }

    fn with(link: &[u32; 3]) -> Self {
        let mut vector = Self::empty();
        vector.push(link);
        vector
    }

    fn refresh_header(&mut self) {
        let begin = self.links.as_ptr() as u32;
        self.header = [begin, begin + self.links.len() as u32 * LINK_SIZE, begin + self.links.capacity() as u32 * LINK_SIZE];
    }

    fn ptr(&self) -> u32 {
        self.header.as_ptr() as u32
    }

    /// Appends a copy of `link`: a new animation reference and the cache stamp, as vanilla's link copy does.
    fn push(&mut self, link: &[u32; 3]) {
        self.links.push(make_link(link[0], link[2] as u8, link[1]));
        self.refresh_header();
    }

    fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

impl Drop for LinkVector {
    fn drop(&mut self) {
        for link in &self.links {
            release_anim(link[0]);
        }
    }
}

/// Which of an animation's two links a repeat step appends: the main link, or the reversed tail a ping-pong
/// adds after it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LinkKind {
    Main,
    Tail,
}

/// The links appended after the first to fill `total` milliseconds with an animation of `duration`. Starts from
/// `total - duration` (minus another `duration` for ping-pong) and appends a main link, then a tail when
/// ping-pong, subtracting `duration` for each, while the remainder is positive. `stop_after_one_on_zero`
/// ends the sequence after one main link when `duration` is zero (`fMove`, `fPlayTime`); without it a zero
/// duration never ends (`fPlaySpecial`, as in vanilla), so callers that do not trust the duration must bound
/// the iteration.
fn timed_repeat(total: i32, duration: i32, ping_pong: bool, stop_after_one_on_zero: bool) -> impl Iterator<Item = LinkKind> {
    let mut remaining = total.wrapping_sub(duration);
    if ping_pong {
        remaining = remaining.wrapping_sub(duration);
    }
    let mut pending_tail = false;
    let mut finished = false;
    std::iter::from_fn(move || {
        if pending_tail {
            pending_tail = false;
            remaining = remaining.wrapping_sub(duration);
            return Some(LinkKind::Tail);
        }
        if finished || remaining <= 0 {
            return None;
        }
        remaining = remaining.wrapping_sub(duration);
        finished = stop_after_one_on_zero && duration == 0;
        pending_tail = ping_pong;
        Some(LinkKind::Main)
    })
}

/// The links appended after the first when `fPlaySpecial` is given a repeat count: `count - 1` main links (none
/// for a count below two), each followed by a tail when ping-pong.
fn counted_repeat(count: i32, ping_pong: bool) -> impl Iterator<Item = LinkKind> {
    (0..count.wrapping_sub(1).max(0)).flat_map(move |_| std::iter::once(LinkKind::Main).chain(ping_pong.then_some(LinkKind::Tail)))
}

pub(crate) unsafe fn play_single_with(m: &MixerOps, unit: u32, anim: u32, flag: u8, sound: u32) {
    let link = make_link(anim, flag, sound);
    {
        let vector = LinkVector::with(&link);
        save_to_memory(unit + UNIT_MIXER_FLAG, 0u8);
        unsafe { (m.set_base)(unit + UNIT_MIXER, vector.ptr()) };
    }
    release_anim(link[0]);
}

unsafe fn play_single_vanilla(unit: u32, anim: u32, flag: u8, sound: u32) {
    unsafe { play_single_with(&VANILLA_MIXER, unit, anim, flag, sound) }
}

pub(crate) unsafe fn play_pair_with(m: &MixerOps, this: u32, unit: u32, anim_first: u32, anim_second: u32, flag: u8, sound: u32, seconds: i32) -> bool {
    let first_link = make_link(anim_first, 0, 0);
    let first = LinkVector::with(&first_link);
    let second_link = make_link(anim_second, flag, sound);
    let mut second = LinkVector::with(&second_link);
    let mut played = false;
    if !second.is_empty() && !first.is_empty() {
        let duration = unsafe { (m.duration)(second_link.as_ptr() as u32) };
        for _ in timed_repeat(seconds, duration, false, true) {
            second.push(&second_link);
        }
        played = unsafe { (m.ai_play)(this, second.ptr(), first.ptr(), unit, false) };
    }
    drop(second);
    release_anim(second_link[0]);
    drop(first);
    release_anim(first_link[0]);
    played
}

unsafe fn play_pair_vanilla(this: u32, unit: u32, anim_first: u32, anim_second: u32, flag: u8, sound: u32, seconds: i32) -> bool {
    unsafe { play_pair_with(&VANILLA_MIXER, this, unit, anim_first, anim_second, flag, sound, seconds) }
}

pub(crate) const F_MOVE_VANILLA_OPS: FMoveOps = FMoveOps {
    stop: stop_vanilla,
    set_moving: set_moving_vanilla,
    reached_target: reached_target_vanilla,
    is_no_sound: is_no_sound_vanilla,
    find_sound: find_sound_vanilla,
    play_single: play_single_vanilla,
    play_pair: play_pair_vanilla,
};

/// The three vanilla strings `fPlaySpecial` compares its string arguments against: the "no animation/sound"
/// string (`[0x0063950c]`) and the two special play-mode strings (`[0x00643620]` reverse, `[0x00643624]`
/// ping-pong).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpecialString {
    None,
    Reverse,
    PingPong,
    /// The vanilla strings at `[0x0064bdac]` and `[0x0064bda8]`, the two plain play-mode names `fPlaySpecialF`
    /// compares its fifth parameter against (the second also sets the call's second flag). Their text is not
    /// identified.
    ModeA,
    ModeB,
}

const RVA_PLAY_REVERSE_STRING: u32 = 0x0024_3620;
const RVA_PLAY_PING_PONG_STRING: u32 = 0x0024_3624;
const RVA_PLAY_MODE_A_STRING: u32 = 0x0024_bdac;
const RVA_PLAY_MODE_B_STRING: u32 = 0x0024_bda8;
const UNIT_TYPE: u32 = 0x128;
const AI_DEFAULT_ANIM: u32 = 0xd0;
const ENTITY_VT_DEFAULT_ANIM: u32 = 0x8c;

/// The arguments of `BFAIMgr::fPlaySpecial` in stack order. `seconds` is the duration (unsigned in vanilla's
/// zero test, signed in its arithmetic) and `count` the repeat count.
pub struct FPlaySpecialArgs {
    pub this: u32,
    pub anim_name: u32,
    pub sound_name: u32,
    pub seconds: u32,
    pub count: i32,
    pub special: u32,
    pub unit: u32,
    pub done: *const u8,
}

/// The vanilla pieces `fPlaySpecial` leans on. `acquire_first` builds the first link and its vector from the
/// unit's default animation (kept alive until `release_first`, as vanilla keeps it through every exit);
/// `play_links` builds the second vector, repeats it to fill the duration or count and plays both, returning
/// `None` when either vector ends up empty.
pub struct FPlaySpecialOps {
    pub is_string: unsafe fn(SpecialString, u32) -> bool,
    pub find_anim: unsafe fn(u32, u32) -> u32,
    pub find_sound: unsafe fn(u32, u32) -> u32,
    pub acquire_first: unsafe fn(u32) -> u32,
    pub release_first: unsafe fn(u32),
    /// `(this, unit, first handle, anim, reverse, sound, ping_pong, seconds, count)`.
    pub play_links: unsafe fn(u32, u32, u32, u32, bool, u32, bool, u32, i32) -> Option<bool>,
}

fn string_is_empty(string: u32) -> bool {
    get_from_memory::<u32>(string) == get_from_memory::<u32>(string + 4)
}

/// `BFAIMgr::fPlaySpecial(anim_name, sound_name, seconds, count, special, unit, done)`.
///
/// Gate: unless the animation name is the "none" string, a call with no repeat count and either no sound
/// name or no duration must name a special mode (`special` equal to the reverse or ping-pong string), else it
/// fails. A unit that is not stopped just finishes (marking itself stopped when it is moving and its two
/// position words match). Otherwise the unit's default animation (virtual `+0x8c`) seeds the first link; the
/// animation is `findAnim(anim_name)`, falling back to the manager's default animation (`this + 0xd0`) only when
/// the name is the "none" string and the sound name is a real, non-empty name; the sound is `findSound(sound_name)`
/// unless it is "none" or empty. The result is `fPlay`'s, and the unit is marked moving and not stopped.
///
/// A positive duration with a zero animation duration never ends, as in vanilla.
pub unsafe fn f_play_special(args: &FPlaySpecialArgs, ops: &FPlaySpecialOps) -> bool {
    let (unit, done) = (args.unit, args.done);
    let is = |which, string| unsafe { (ops.is_string)(which, string) };
    if !is(SpecialString::None, args.anim_name) {
        let sound_is_none = is(SpecialString::None, args.sound_name);
        let gated = args.count == 0 && (sound_is_none || args.seconds == 0);
        if gated && !(is(SpecialString::Reverse, args.special) || is(SpecialString::PingPong, args.special)) {
            return false;
        }
    }
    let reverse = is(SpecialString::Reverse, args.special);
    let ping_pong = !reverse && is(SpecialString::PingPong, args.special);

    if get_from_memory::<u8>(unit + UNIT_STOPPED) == 0 {
        if get_from_memory::<u8>(unit + UNIT_MOVING) != 0 && get_from_memory::<u32>(unit + UNIT_POS_B) == get_from_memory::<u32>(unit + UNIT_POS_A) {
            save_to_memory(unit + UNIT_STOPPED, 1u8);
            unsafe { (done as *mut u8).write_volatile(1) };
            save_to_memory(unit + UNIT_MOVING, 0u8);
        }
        return true;
    }
    if get_from_memory::<u32>(unit + UNIT_TYPE) == 0 {
        return false;
    }
    let first_anim = unsafe { vcall1(unit, ENTITY_VT_DEFAULT_ANIM, 0) };
    if first_anim == 0 {
        return false;
    }
    let first = unsafe { (ops.acquire_first)(first_anim) };
    let named = !is(SpecialString::None, args.anim_name);
    let mut anim = if named { unsafe { (ops.find_anim)(unit, args.anim_name) } } else { 0 };
    if anim == 0 && (named || is(SpecialString::None, args.sound_name) || string_is_empty(args.sound_name)) {
        unsafe { (ops.release_first)(first) };
        return false;
    }
    let sound = if is(SpecialString::None, args.sound_name) || string_is_empty(args.sound_name) { 0 } else { unsafe { (ops.find_sound)(unit, args.sound_name) } };
    if anim == 0 {
        anim = get_from_memory::<u32>(args.this + AI_DEFAULT_ANIM);
    }
    let played = unsafe { (ops.play_links)(args.this, unit, first, anim, reverse, sound, ping_pong, args.seconds, args.count) };
    unsafe { (ops.release_first)(first) };
    match played {
        None => false,
        Some(result) => {
            save_to_memory(unit + UNIT_MOVING, 1u8);
            save_to_memory(unit + UNIT_STOPPED, 0u8);
            unsafe { (done as *mut u8).write_volatile(0) };
            result
        }
    }
}

unsafe fn special_string_is(which: SpecialString, string: u32) -> bool {
    let rva = match which {
        SpecialString::None => RVA_NO_SOUND_STRING,
        SpecialString::Reverse => RVA_PLAY_REVERSE_STRING,
        SpecialString::PingPong => RVA_PLAY_PING_PONG_STRING,
        SpecialString::ModeA => RVA_PLAY_MODE_A_STRING,
        SpecialString::ModeB => RVA_PLAY_MODE_B_STRING,
    };
    let cell = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + rva);
    if cell == 0 {
        return false;
    }
    let (begin, end) = (get_from_memory::<u32>(string), get_from_memory::<u32>(string + 4));
    let (other_begin, other_end) = (get_from_memory::<u32>(cell), get_from_memory::<u32>(cell + 4));
    if end.wrapping_sub(begin) != other_end.wrapping_sub(other_begin) {
        return false;
    }
    let len = (end - begin) as usize;
    len == 0 || unsafe { std::slice::from_raw_parts(begin as *const u8, len) == std::slice::from_raw_parts(other_begin as *const u8, len) }
}

unsafe fn entity_find_anim_vanilla(unit: u32, name: u32) -> u32 {
    unsafe { BFENTITY_FIND_ANIM.hooked()(unit as *const u32, name as *const std::ffi::c_void) as u32 }
}

/// The first link and its one-element vector, held across `fPlaySpecial`'s body.
struct FirstLinks {
    link: [u32; 3],
    vector: Option<LinkVector>,
    second: LinkVector,
}

impl Drop for FirstLinks {
    fn drop(&mut self) {
        self.second = LinkVector::empty();
        self.vector = None;
        release_anim(self.link[0]);
    }
}

pub(crate) unsafe fn acquire_first_vanilla(anim: u32) -> u32 {
    let link = make_link(anim, 0, 0);
    let vector = Some(LinkVector::with(&link));
    Box::into_raw(Box::new(FirstLinks { link, vector, second: LinkVector::empty() })) as u32
}

pub(crate) unsafe fn release_first_vanilla(handle: u32) {
    drop(unsafe { Box::from_raw(handle as *mut FirstLinks) });
}

#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn play_links_with(m: &MixerOps, this: u32, unit: u32, first: u32, anim: u32, reverse: bool, sound: u32, ping_pong: bool, seconds: u32, count: i32) -> Option<bool> {
    let first = unsafe { &*(first as *const FirstLinks) };
    let first_vector = first.vector.as_ref()?;
    let main_link = make_link(anim, u8::from(reverse), sound);
    let mut second = LinkVector::with(&main_link);
    let tail_link = make_link(anim, 1, 0);
    if ping_pong {
        second.push(&tail_link);
    }
    let mut played = None;
    if !second.is_empty() && !first_vector.is_empty() {
        if seconds != 0 {
            let duration = unsafe { (m.duration)(main_link.as_ptr() as u32) };
            for kind in timed_repeat(seconds as i32, duration, ping_pong, false) {
                second.push(if kind == LinkKind::Main { &main_link } else { &tail_link });
            }
        } else {
            for kind in counted_repeat(count, ping_pong) {
                second.push(if kind == LinkKind::Main { &main_link } else { &tail_link });
            }
        }
        played = Some(unsafe { (m.ai_play)(this, second.ptr(), first_vector.ptr(), unit, false) });
    }
    drop(second);
    release_anim(tail_link[0]);
    release_anim(main_link[0]);
    played
}

#[allow(clippy::too_many_arguments)]
unsafe fn play_links_vanilla(this: u32, unit: u32, first: u32, anim: u32, reverse: bool, sound: u32, ping_pong: bool, seconds: u32, count: i32) -> Option<bool> {
    unsafe { play_links_with(&VANILLA_MIXER, this, unit, first, anim, reverse, sound, ping_pong, seconds, count) }
}

const F_PLAY_SPECIAL_VANILLA_OPS: FPlaySpecialOps = FPlaySpecialOps {
    is_string: special_string_is,
    find_anim: entity_find_anim_vanilla,
    find_sound: find_sound_vanilla,
    acquire_first: acquire_first_vanilla,
    release_first: release_first_vanilla,
    play_links: play_links_vanilla,
};

/// `BFAIMgr` field the play handlers use as their loop cursor: the pointer to the parameter being resolved.
const AI_PARAM_CURSOR: u32 = 0xcc;
const UNIT_DIRECTION: u32 = 0x12c;
const ENTITY_VT_DIR_TO_SET: u32 = 0x80;
const MIXER_COUNT_A: u32 = 0x28;
const MIXER_COUNT_B: u32 = 0x18;
const CALL_RESULT_FLAG: u32 = 0x18;

/// The vanilla pieces `fPlayF`/`fPlayWithBaseF`/`fSetBaseF` lean on. `acquire_first` seeds the first link vector
/// (`first_anim`, flag 0, sound 0) and returns a handle that owns it and the second vector; `add_anim` appends
/// `(anim, reverse, 0)` (and, for ping-pong, `(anim, 1, 0)`) to the second vector; `play` runs `fPlay(this,
/// second, first, unit, scratch, with_base)` and returns `None` when either vector is empty.
pub struct PlayOps {
    pub find_anim: unsafe fn(u32, u32) -> u32,
    pub mixer_anim: unsafe fn(u32) -> u32,
    pub acquire_first: unsafe fn(u32) -> u32,
    pub release_first: unsafe fn(u32),
    pub add_anim: unsafe fn(u32, u32, bool, bool),
    pub play: unsafe fn(u32, u32, u32, bool) -> Option<bool>,
    /// `fSetBaseF`'s mixer work: `(unit, anim)` builds the one-link vector, clears the mixer, resolves the set
    /// through the unit's `+0x80` virtual and installs set and base.
    pub set_base: unsafe fn(u32, u32),
}

/// The "not stopped" prefix shared by `fPlayF` and `fPlayWithBaseF`: `Some(result)` when the unit is not
/// stopped. A moving unit whose mixer counters match (or that has no animation) is stopped and finishes the
/// call; either way the result is true.
unsafe fn play_not_stopped(call: u32, unit: u32, ops: &PlayOps) -> Option<bool> {
    if get_from_memory::<u8>(unit + UNIT_STOPPED) != 0 {
        return None;
    }
    if get_from_memory::<u8>(unit + UNIT_MOVING) != 0 {
        let mixer = unit + UNIT_MIXER;
        if get_from_memory::<u32>(mixer + MIXER_COUNT_A) == get_from_memory::<u32>(mixer + MIXER_COUNT_B) || unsafe { (ops.mixer_anim)(mixer) } == 0 {
            save_to_memory(unit + UNIT_STOPPED, 1u8);
            save_to_memory(call + CALL_RESULT_FLAG, 1u8);
            save_to_memory(unit + UNIT_MOVING, 0u8);
        }
    }
    Some(true)
}

/// Resolves every parameter from `from` on with `findAnim`, adding each hit to the second vector. The cursor
/// field in the manager follows the walk, as in vanilla.
unsafe fn add_param_anims(this: u32, call: u32, unit: u32, handle: u32, from: u32, reverse: bool, ping_pong: bool, ops: &PlayOps) {
    let end = unsafe { call_params_end(call) };
    save_to_memory(this + AI_PARAM_CURSOR, from);
    while get_from_memory::<u32>(this + AI_PARAM_CURSOR) != end {
        let anim = unsafe { (ops.find_anim)(unit, get_from_memory::<u32>(this + AI_PARAM_CURSOR)) };
        if anim != 0 {
            unsafe { (ops.add_anim)(handle, anim, reverse, ping_pong) };
        }
        save_to_memory(this + AI_PARAM_CURSOR, get_from_memory::<u32>(this + AI_PARAM_CURSOR) + CALL_PARAM_SIZE);
    }
}

unsafe fn finish_play(call: u32, unit: u32, handle: u32, with_base: bool, this: u32, ops: &PlayOps) -> bool {
    let played = unsafe { (ops.play)(this, unit, handle, with_base) };
    unsafe { (ops.release_first)(handle) };
    match played {
        None => false,
        Some(result) => {
            save_to_memory(unit + UNIT_MOVING, 1u8);
            save_to_memory(unit + UNIT_STOPPED, 0u8);
            save_to_memory(call + CALL_RESULT_FLAG, 0u8);
            result
        }
    }
}

/// `BFAIMgr::fPlayF(call, unit, reverse, ping_pong)`: needs at least one parameter. A stopped unit with a type
/// seeds the first link from its default animation (virtual `+0x8c`), resolves each parameter as an
/// animation into the second vector and plays them; see [`play_not_stopped`] for a unit that is not stopped.
pub unsafe fn f_play_f(this: u32, call: u32, unit: u32, reverse: bool, ping_pong: bool, ops: &PlayOps) -> bool {
    if unsafe { call_param_count(call) } < 1 {
        return false;
    }
    if let Some(result) = unsafe { play_not_stopped(call, unit, ops) } {
        return result;
    }
    if get_from_memory::<u32>(unit + UNIT_TYPE) == 0 {
        return false;
    }
    let default_anim = unsafe { vcall1(unit, ENTITY_VT_DEFAULT_ANIM, 0) };
    if default_anim == 0 {
        return false;
    }
    let handle = unsafe { (ops.acquire_first)(default_anim) };
    unsafe { add_param_anims(this, call, unit, handle, call_params_begin(call), reverse, ping_pong, ops) };
    unsafe { finish_play(call, unit, handle, false, this, ops) }
}

/// `BFAIMgr::fPlayWithBaseF(call, unit, reverse, ping_pong)`: needs at least two parameters. The first names
/// the base animation (the first link; the call fails if it does not resolve), the rest are played over it.
pub unsafe fn f_play_with_base_f(this: u32, call: u32, unit: u32, reverse: bool, ping_pong: bool, ops: &PlayOps) -> bool {
    if unsafe { call_param_count(call) } < 2 {
        return false;
    }
    if let Some(result) = unsafe { play_not_stopped(call, unit, ops) } {
        return result;
    }
    if get_from_memory::<u32>(unit + UNIT_TYPE) == 0 {
        return false;
    }
    let begin = unsafe { call_params_begin(call) };
    save_to_memory(this + AI_PARAM_CURSOR, begin);
    let base = unsafe { (ops.find_anim)(unit, begin) };
    if base == 0 {
        return false;
    }
    let handle = unsafe { (ops.acquire_first)(base) };
    unsafe { add_param_anims(this, call, unit, handle, begin + CALL_PARAM_SIZE, reverse, ping_pong, ops) };
    unsafe { finish_play(call, unit, handle, true, this, ops) }
}

/// `BFAIMgr::fSetBaseF(call, unit)`: exactly one parameter. A unit that is not stopped returns true untouched.
/// Otherwise the parameter is resolved and, when found, installed as the mixer's set and base; the unit is
/// then marked stopped and not moving and the call finished whether or not the lookup found anything, and the
/// result is whether it did.
pub unsafe fn f_set_base_f(this: u32, call: u32, unit: u32, ops: &PlayOps) -> bool {
    if unsafe { call_param_count(call) } != 1 {
        return false;
    }
    if get_from_memory::<u8>(unit + UNIT_STOPPED) == 0 {
        return true;
    }
    if get_from_memory::<u32>(unit + UNIT_TYPE) == 0 {
        return false;
    }
    let param = unsafe { call_params_begin(call) };
    save_to_memory(this + AI_PARAM_CURSOR, param);
    let anim = unsafe { (ops.find_anim)(unit, param) };
    let found = anim != 0;
    if found {
        unsafe { (ops.set_base)(unit, anim) };
    }
    save_to_memory(unit + UNIT_MOVING, 0u8);
    save_to_memory(unit + UNIT_STOPPED, 1u8);
    save_to_memory(call + CALL_RESULT_FLAG, 1u8);
    found
}

unsafe fn mixer_anim_vanilla(mixer: u32) -> u32 {
    unsafe { GXMIXER_GET_ANIM.hooked()(mixer as *const u32) as u32 }
}

pub(crate) unsafe fn add_anim_vanilla(handle: u32, anim: u32, reverse: bool, ping_pong: bool) {
    let links = unsafe { &mut *(handle as *mut FirstLinks) };
    let link = make_link(anim, u8::from(reverse), 0);
    links.second.push(&link);
    if ping_pong {
        let tail = make_link(anim, 1, 0);
        links.second.push(&tail);
        release_anim(tail[0]);
    }
    release_anim(link[0]);
}

pub(crate) unsafe fn play_with(m: &MixerOps, this: u32, unit: u32, handle: u32, with_base: bool) -> Option<bool> {
    let links = unsafe { &*(handle as *const FirstLinks) };
    let first = links.vector.as_ref()?;
    if links.second.is_empty() || first.is_empty() {
        return None;
    }
    Some(unsafe { (m.ai_play)(this, links.second.ptr(), first.ptr(), unit, with_base) })
}

unsafe fn play_vanilla(this: u32, unit: u32, handle: u32, with_base: bool) -> Option<bool> {
    unsafe { play_with(&VANILLA_MIXER, this, unit, handle, with_base) }
}

pub(crate) unsafe fn set_base_with(m: &MixerOps, unit: u32, anim: u32) {
    let link = make_link(anim, 0, 0);
    {
        let vector = LinkVector::with(&link);
        let mixer = unit + UNIT_MIXER;
        unsafe { (m.clear_all)(mixer) };
        let direction = get_from_memory::<u32>(unit + UNIT_DIRECTION);
        let mut out = 0u32;
        let target = get_from_memory::<u32>(get_from_memory::<u32>(unit) + ENTITY_VT_DIR_TO_SET);
        let dir_to_set = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, *mut u32, u32, u32) -> u32>(target) };
        let set = dir_to_set(unit, direction, &mut out, anim, 0);
        save_to_memory(unit + UNIT_MIXER_FLAG, 0u8);
        unsafe { (m.set_set)(mixer, set, out as u8) };
        unsafe { (m.set_base)(mixer, vector.ptr()) };
    }
    release_anim(link[0]);
}

unsafe fn set_base_vanilla(unit: u32, anim: u32) {
    unsafe { set_base_with(&VANILLA_MIXER, unit, anim) }
}

const PLAY_VANILLA_OPS: PlayOps = PlayOps {
    find_anim: entity_find_anim_vanilla,
    mixer_anim: mixer_anim_vanilla,
    acquire_first: acquire_first_vanilla,
    release_first: release_first_vanilla,
    add_anim: add_anim_vanilla,
    play: play_vanilla,
    set_base: set_base_vanilla,
};

/// The vanilla call `fFlyF` makes: `fFly(this, unit, name, value, extra, done, reverse, limit)` with `name`
/// and `extra` vanilla strings.
pub struct FlyOps {
    pub f_fly: unsafe fn(u32, u32, u32, u32, u32, u32, u32, u32) -> bool,
}

unsafe fn f_fly_vanilla(this: u32, unit: u32, name: u32, value: u32, extra: u32, done: u32, reverse: u32, limit: u32) -> bool {
    unsafe { F_FLY.hooked()(this as *const u32, unit as *const u32, name as *const c_void, value as i32, extra as *const c_void, done as *const i8, reverse as i32, limit as i32) }
}

const FLY_VANILLA: FlyOps = FlyOps { f_fly: f_fly_vanilla };

/// The arguments of `BFAIMgr::fFly` in stack order (`this` is unused by vanilla).
pub struct FFlyArgs {
    pub unit: u32,
    pub name: u32,
    pub value: u32,
    pub extra: u32,
    pub done: *const u8,
    pub reverse: u32,
    pub limit: u32,
}

/// The vanilla pieces `fFly` leans on: `BFUnit::stop`/`setMoving(unit, 1, 0)`, whether the extra string is the
/// "no sound" string, `findSound(unit, extra)` and `(unit, anim, flag, sound)` setting the mixer base to one link.
pub struct FlyCoreOps {
    pub stop: unsafe fn(u32, i8, i8),
    pub set_moving: unsafe fn(u32, i8, i8),
    pub is_no_sound: unsafe fn(u32) -> bool,
    pub find_sound: unsafe fn(u32, u32) -> u32,
    pub play_single: unsafe fn(u32, u32, u8, u32),
}

/// A flying unit's position and destination words (`+0x114..` and `+0x1dc..`).
const UNIT_FLY_POSITION: u32 = 0x114;
const UNIT_FLY_TARGET: u32 = 0x1dc;

/// `BFAIMgr::fFly(unit, name, value, extra, done, reverse, limit)`.
///
/// A unit that is not stopped and is moving: a unit not flagged `+0x20c` but flagged `+0xd7` is set moving; one
/// at its destination (three words) has its speed set to `0`, is marked stopped and not moving, flags `done` and
/// is stopped. Returns true.
/// A stopped unit starts the move: the animation is `findAnim(name)` (false if none); the sound is
/// `findSound(extra)` unless `extra` is the "no sound" string or empty; the mixer base becomes one link
/// `(anim, sound, reverse)`; the speed and limit virtuals take `value` and `limit`; the unit is marked moving and
/// not stopped. Returns true.
pub unsafe fn f_fly(args: &FFlyArgs, ops: &FlyCoreOps) -> bool {
    let unit = args.unit;
    if get_from_memory::<u8>(unit + UNIT_STOPPED) == 0 {
        if get_from_memory::<u8>(unit + UNIT_MOVING) != 0 {
            if get_from_memory::<u8>(unit + UNIT_FLAG_20C) == 0 && get_from_memory::<u8>(unit + UNIT_FLAG_D7) != 0 {
                unsafe { (ops.set_moving)(unit, 1, 0) };
                return true;
            }
            if (0..3).all(|word| get_from_memory::<u32>(unit + UNIT_FLY_POSITION + word * 4) == get_from_memory::<u32>(unit + UNIT_FLY_TARGET + word * 4)) {
                unsafe { vcall1(unit, ENTITY_VT_SET_SPEED, 0) };
                save_to_memory(unit + UNIT_STOPPED, 1u8);
                unsafe { (args.done as *mut u8).write_volatile(1) };
                save_to_memory(unit + UNIT_MOVING, 0u8);
                unsafe { (ops.stop)(unit, 1, 0) };
            }
        }
        return true;
    }
    let anim = unsafe { vcall1(unit, ENTITY_VT_FIND_ANIM, args.name) };
    if anim == 0 {
        return false;
    }
    let extra_empty = string_is_empty(args.extra);
    let sound = if unsafe { (ops.is_no_sound)(args.extra) } || extra_empty { 0 } else { unsafe { (ops.find_sound)(unit, args.extra) } };
    unsafe { (ops.play_single)(unit, anim, args.reverse as u8, sound) };
    unsafe { vcall1(unit, ENTITY_VT_SET_SPEED, args.value) };
    unsafe { vcall1(unit, ENTITY_VT_SET_LIMIT, args.limit) };
    save_to_memory(unit + UNIT_MOVING, 1u8);
    save_to_memory(unit + UNIT_STOPPED, 0u8);
    true
}

const FLY_CORE_VANILLA: FlyCoreOps =
    FlyCoreOps { stop: stop_vanilla, set_moving: set_moving_vanilla, is_no_sound: is_no_sound_vanilla, find_sound: find_sound_vanilla, play_single: play_single_vanilla };

/// The vanilla pieces `fPlayWithSound` leans on: `BFEntity::findAnim(unit, name)`, `findSound(unit, name)` and
/// `(unit, first_anim, second_anim, sound)`, which builds the two one-link vectors (`first_anim` with no flag or
/// sound, `second_anim` with the sound) and plays them with `fPlay`, returning `None` when either vector ends up
/// empty.
pub struct PlayWithSoundOps {
    pub find_anim: unsafe fn(u32, u32) -> u32,
    pub find_sound: unsafe fn(u32, u32) -> u32,
    pub play_two: unsafe fn(u32, u32, u32, u32) -> Option<bool>,
}

/// Unit field `fPlayWithSound` requires to be set before it plays anything (the entity type).
const UNIT_TYPE_POINTER: u32 = 0x128;

/// `BFAIMgr::fPlayWithSound(anim_name, sound_name, unit, done)` (`thiscall`, `this` unused, `RET 0x10`).
///
/// A unit that is not stopped: a moving unit whose two position words match is marked stopped and not moving and
/// flags `done`; returns true either way. A stopped unit: false without an entity type (`+0x128`), without a
/// default animation (virtual `+0x8c`) or when `anim_name` has no animation; otherwise plays the default
/// animation and the named one with `findSound(sound_name)` through `fPlay`. When it played (both vectors
/// non-empty) the unit is marked moving and not stopped and `done` is cleared. Returns `fPlay`'s result, false if
/// nothing played.
pub unsafe fn f_play_with_sound(anim_name: u32, sound_name: u32, unit: u32, done: *const u8, ops: &PlayWithSoundOps) -> bool {
    if get_from_memory::<u8>(unit + UNIT_STOPPED) == 0 {
        if get_from_memory::<u8>(unit + UNIT_MOVING) != 0 && get_from_memory::<u32>(unit + UNIT_POS_B) == get_from_memory::<u32>(unit + UNIT_POS_A) {
            save_to_memory(unit + UNIT_STOPPED, 1u8);
            unsafe { (done as *mut u8).write_volatile(1) };
            save_to_memory(unit + UNIT_MOVING, 0u8);
        }
        return true;
    }
    if get_from_memory::<u32>(unit + UNIT_TYPE_POINTER) == 0 {
        return false;
    }
    let first = unsafe { vcall1(unit, ENTITY_VT_DEFAULT_ANIM, 0) };
    if first == 0 {
        return false;
    }
    let second = unsafe { (ops.find_anim)(unit, anim_name) };
    if second == 0 {
        return false;
    }
    let sound = unsafe { (ops.find_sound)(unit, sound_name) };
    let Some(result) = (unsafe { (ops.play_two)(unit, first, second, sound) }) else {
        return false;
    };
    save_to_memory(unit + UNIT_MOVING, 1u8);
    save_to_memory(unit + UNIT_STOPPED, 0u8);
    unsafe { (done as *mut u8).write_volatile(0) };
    result
}

pub(crate) unsafe fn play_two_with(m: &MixerOps, unit: u32, first_anim: u32, second_anim: u32, sound: u32) -> Option<bool> {
    let first_link = make_link(first_anim, 0, 0);
    let first = LinkVector::with(&first_link);
    let second_link = make_link(second_anim, 0, sound);
    let second = LinkVector::with(&second_link);
    let result = if second.is_empty() || first.is_empty() {
        None
    } else {
        Some(unsafe { (m.ai_play)(0, second.ptr(), first.ptr(), unit, false) })
    };
    drop(second);
    release_anim(second_link[0]);
    drop(first);
    release_anim(first_link[0]);
    result
}

unsafe fn play_two_vanilla(unit: u32, first_anim: u32, second_anim: u32, sound: u32) -> Option<bool> {
    unsafe { play_two_with(&VANILLA_MIXER, unit, first_anim, second_anim, sound) }
}

const PLAY_WITH_SOUND_VANILLA: PlayWithSoundOps = PlayWithSoundOps { find_anim: entity_find_anim_vanilla, find_sound: find_sound_vanilla, play_two: play_two_vanilla };

/// The vanilla pieces `fPlayTime` leans on: `BFEntity::findAnim(unit, name)` and `(unit, first_anim,
/// second_anim, millis)`, which builds the two one-link vectors, repeats the second link while time remains and
/// plays them with `fPlay`, returning `None` when either vector ends up empty.
pub struct PlayTimeOps {
    pub find_anim: unsafe fn(u32, u32) -> u32,
    pub play_timed: unsafe fn(u32, u32, u32, i32) -> Option<bool>,
}

/// `BFAIMgr::fPlayTime(name, millis, unit, done, flag)` (`thiscall`, `this` unused, `RET 0x14`; the fifth
/// argument's address is only `fPlay`'s scratch slot).
///
/// A unit that is not stopped: a moving unit whose two position words match is marked stopped and not moving and
/// flags `done`; returns true either way. A stopped unit: false without an entity type (`+0x128`), without a
/// default animation (virtual `+0x8c`) or when `name` has no animation; otherwise plays the default animation and
/// the named one repeated to fill `millis`. When it played the unit is marked moving and not stopped and `done`
/// is cleared. Returns `fPlay`'s result, false if nothing played.
pub unsafe fn f_play_time(name: u32, millis: u32, unit: u32, done: *const u8, ops: &PlayTimeOps) -> bool {
    if get_from_memory::<u8>(unit + UNIT_STOPPED) == 0 {
        if get_from_memory::<u8>(unit + UNIT_MOVING) != 0 && get_from_memory::<u32>(unit + UNIT_POS_B) == get_from_memory::<u32>(unit + UNIT_POS_A) {
            save_to_memory(unit + UNIT_STOPPED, 1u8);
            unsafe { (done as *mut u8).write_volatile(1) };
            save_to_memory(unit + UNIT_MOVING, 0u8);
        }
        return true;
    }
    if get_from_memory::<u32>(unit + UNIT_TYPE_POINTER) == 0 {
        return false;
    }
    let first = unsafe { vcall1(unit, ENTITY_VT_DEFAULT_ANIM, 0) };
    if first == 0 {
        return false;
    }
    let second = unsafe { (ops.find_anim)(unit, name) };
    if second == 0 {
        return false;
    }
    let Some(result) = (unsafe { (ops.play_timed)(unit, first, second, millis as i32) }) else {
        return false;
    };
    save_to_memory(unit + UNIT_MOVING, 1u8);
    save_to_memory(unit + UNIT_STOPPED, 0u8);
    unsafe { (done as *mut u8).write_volatile(0) };
    result
}

/// Repeats `second_anim`'s link while time remains: after the first, `remaining = millis - duration`, and each
/// further link subtracts another `duration`; a zero duration stops after one extra link (unlike `fPlaySpecial`).
pub(crate) unsafe fn play_timed_with(m: &MixerOps, unit: u32, first_anim: u32, second_anim: u32, millis: i32) -> Option<bool> {
    let first_link = make_link(first_anim, 0, 0);
    let first = LinkVector::with(&first_link);
    let second_link = make_link(second_anim, 0, 0);
    let mut second = LinkVector::with(&second_link);
    let result = if second.is_empty() || first.is_empty() {
        None
    } else {
        let duration = unsafe { (m.duration)(second_link.as_ptr() as u32) };
        for _ in timed_repeat(millis, duration, false, true) {
            second.push(&second_link);
        }
        Some(unsafe { (m.ai_play)(0, second.ptr(), first.ptr(), unit, false) })
    };
    drop(second);
    release_anim(second_link[0]);
    drop(first);
    release_anim(first_link[0]);
    result
}

unsafe fn play_timed_vanilla(unit: u32, first_anim: u32, second_anim: u32, millis: i32) -> Option<bool> {
    unsafe { play_timed_with(&VANILLA_MIXER, unit, first_anim, second_anim, millis) }
}

const PLAY_TIME_VANILLA_OPS: PlayTimeOps = PlayTimeOps { find_anim: entity_find_anim_vanilla, play_timed: play_timed_vanilla };

/// `BFAIMgr::fFlyF(call, unit, reverse)`: two or three parameters and a non-null unit. Passes parameter 0 as is, converts parameter 1 and (when present) parameter 2 through the manager's vtable `+0x34`
/// and calls `fFly(unit, name, value, empty, done, reverse, limit)` with `limit` defaulting to 100. Vanilla also
/// builds a string from the unit's `+0x120` virtual and tests the type twice with unused results, and passes
/// a `vector<byte>` the port replaces with an empty string.
pub unsafe fn f_fly_f(this: u32, call: u32, unit: u32, reverse: u32, ops: &FlyOps) -> bool {
    if unit == 0 {
        return false;
    }
    let count = unsafe { call_param_count(call) } as u32;
    if !(2..=3).contains(&count) {
        return false;
    }
    let begin = unsafe { call_params_begin(call) };
    let value = unsafe { convert_param(this, begin + PARAM_1, unit) };
    let limit = if count >= 3 { unsafe { convert_param(this, begin + PARAM_2, unit) } } else { DEFAULT_MOVE_LIMIT };
    let empty = EmptyName::new();
    unsafe { (ops.f_fly)(this, unit, begin, value, empty.as_ptr(), call + CALL_DONE_FLAG_OFFSET, reverse, limit) }
}

/// A `(name, probability)` entry of the vector `BFAIMgr::fPlaySetProb` reads: a vanilla string and a weight,
/// 16 bytes.
#[repr(C)]
struct SetProbEntry {
    name: VanillaString,
    probability: u32,
}

/// The vanilla calls `fPlaySetProbF` makes: the C-string-to-int conversion (`0x00405218`, `cdecl`) and
/// `fPlaySetProb(vector, unit)` (`stdcall`).
pub struct SetProbOps {
    pub parse_int: unsafe fn(*const u8) -> u32,
    pub play_set_prob: unsafe fn(u32, u32) -> bool,
}

unsafe fn parse_int_vanilla(text: *const u8) -> u32 {
    unsafe { ATOI.original()(text as *const i8) as u32 }
}

unsafe fn play_set_prob_vanilla(vector: u32, unit: u32) -> bool {
    (unsafe { F_PLAY_SET_PROB.hooked()(vector as *const i32, unit as *const std::ffi::c_void) }) as u8 != 0
}

const SET_PROB_VANILLA: SetProbOps = SetProbOps { parse_int: parse_int_vanilla, play_set_prob: play_set_prob_vanilla };

/// The vanilla pieces `fPlaySetProb` leans on: the game's `rand()` (`0..=0x7fff`) and `fPlaySet(name, unit)`
/// (called with the "child" flag and the `1` argument, returning its low byte).
pub struct SetProbPickOps {
    pub rand: unsafe fn() -> u32,
    pub play_set: unsafe fn(u32, u32) -> u32,
}

const SET_PROB_ENTRY_SIZE: u32 = 16;
const SET_PROB_WEIGHT_OFFSET: u32 = 0xc;
/// The weights must sum to this for `fPlaySetProb` to pick anything.
const SET_PROB_TOTAL: i32 = 100;

/// `BFAIMgr::fPlaySetProb(entries, unit)`: `entries` is a `{begin, end, cap}` vector of 16-byte `(name, weight)`
/// entries. With at least one entry and weights summing to exactly 100, rolls `rand() % 100 + 1` and walks the
/// entries: the first whose weight covers the roll is played with `fPlaySet`; each miss subtracts that weight
/// from the remaining total and, while the remainder is in `1..=100`, rerolls `rand() % remaining + 1`. Returns
/// `fPlaySet`'s low byte, or false when nothing is played.
pub unsafe fn f_play_set_prob(entries: u32, unit: u32, ops: &SetProbPickOps) -> bool {
    let begin = get_from_memory::<u32>(entries);
    let count = (get_from_memory::<u32>(entries + 4).wrapping_sub(begin) as i32) >> 4;
    let weights: Vec<i32> = (0..count.max(0)).map(|index| get_from_memory::<i32>(begin + index as u32 * SET_PROB_ENTRY_SIZE + SET_PROB_WEIGHT_OFFSET)).collect();
    let Some(index) = pick_set_prob(&weights, || unsafe { (ops.rand)() }) else {
        return false;
    };
    let name = unsafe { param_string(begin + index as u32 * SET_PROB_ENTRY_SIZE) };
    low_byte_bool(unsafe { (ops.play_set)(name.as_ptr() as u32, unit) })
}

/// The entry `fPlaySetProb` plays, given the entries' weights and the game's `rand()`. Needs at least one entry
/// and weights summing to exactly 100. Rolls `rand() % 100 + 1` and walks the entries: the first whose weight
/// covers the roll is chosen; each miss subtracts that weight from the remaining total and, while the remainder
/// is in `1..=100`, rerolls `rand() % remaining + 1`. `None` when nothing is chosen.
fn pick_set_prob(weights: &[i32], mut rand: impl FnMut() -> u32) -> Option<usize> {
    let total = weights.iter().fold(0i32, |sum, weight| sum.wrapping_add(*weight));
    if weights.is_empty() || total != SET_PROB_TOTAL {
        return None;
    }
    let mut roll = (rand() as i32).wrapping_rem(100).wrapping_add(1);
    let mut remaining = SET_PROB_TOTAL;
    let mut current = weights[0];
    for index in 0..weights.len() {
        if roll <= current {
            return Some(index);
        }
        remaining = remaining.wrapping_sub(current);
        if 0 < remaining && remaining < SET_PROB_TOTAL + 1 {
            roll = (rand() as i32).wrapping_rem(remaining).wrapping_add(1);
        }
        if index + 1 < weights.len() {
            current = weights[index + 1];
        }
    }
    None
}

const SET_PROB_PICK_VANILLA: SetProbPickOps = SetProbPickOps { rand: random_vanilla, play_set: play_set_vanilla };

/// A `BFBSCall` (a behavior-set name call): a vanilla `{begin, end, cap}` string plus a flag byte at `+0xc`,
/// 16 bytes, owned through vanilla's allocator. Built by the vanilla constructor (`0x004109fe`, `RET 0x10`:
/// the string by value, which the constructor copies and frees, then the flag) and freed like a string.
#[repr(C)]
pub struct BsCall {
    words: [u32; 4],
}

impl BsCall {
    unsafe fn new(name: &[u8], flag: u32) -> Self {
        let moved = VanillaString::from_bytes(name);
        let string = moved.as_ptr();
        let (w0, w1, w2) = (get_from_memory::<u32>(string as u32), get_from_memory::<u32>(string as u32 + 4), get_from_memory::<u32>(string as u32 + 8));
        mem::forget(moved);
        let mut call = BsCall { words: [0; 4] };
        unsafe { BFBSCALL_CONSTRUCTOR.original()(call.words.as_mut_ptr(), w0 as *const u32, w1 as *const u32, w2 as *const u32, flag as i8) };
        call
    }

    pub fn as_ptr(&self) -> u32 {
        self.words.as_ptr() as u32
    }
}

impl Drop for BsCall {
    fn drop(&mut self) {
        unsafe { BASIC_STRING_0.original()(self.words.as_mut_ptr() as *const c_void) };
    }
}

/// Entity fields `fPlaySet` reads: the "has a pending set" byte (`+0x6d`) and the pending `BFBSCall` (`+0xdc`,
/// begin and end words at `+0xdc`/`+0xe0`); the behavior set embedded at `+0x14`.
const ENTITY_HAS_PENDING_SET: u32 = 0x6d;
const ENTITY_PENDING_BS_CALL: u32 = 0xdc;
const ENTITY_BEHAVIOR_SET: u32 = 0x14;
/// `setBS` clears the mixer by mode: `0` total, `1` everything, anything else leaves it. `fPlaySet` passes `2`.
const SET_BS_NO_MIXER_CLEAR: u32 = 2;

/// The vanilla calls `fPlaySet` makes on an entity, each taking `BFBSCall` pointers it copies (the callee frees
/// its copy): `init_child(unit, name)` (`BFBehaviorSet::initChild` on the embedded set), `set_bs(unit, call,
/// force, mode)` (`BFEntity::setBS`, `RET 0x18`) and `set_pending(unit, call, force)` (`ZTAnimal` pending-set
/// setter, `RET 0x14`; returns whether it stored the call).
pub struct PlaySetOps {
    pub init_child: unsafe fn(u32, u32) -> bool,
    pub set_bs: unsafe fn(u32, u32, u32, u32),
    pub set_pending: unsafe fn(u32, u32, u32),
}

/// `BFAIMgr::fPlaySet(name, unit, child, flag)` (`stdcall`, `RET 0x10`): switches `unit` to the behavior set
/// `name`. False for a null unit or an empty name. With `child` set, hands the name to the unit's behavior set
/// (`initChild`) and returns its result. Otherwise builds a `BFBSCall` from the name and `flag` and returns true
/// after: a unit with a pending set (`+0x6d`) and an empty pending call storing the call as pending; a unit
/// without one and an empty pending call running `setBS(call)`; a unit without one and a non-empty pending call
/// running `setBS(pending call)` and then storing the new call as pending. `force` is `0` throughout.
pub unsafe fn f_play_set(name: u32, unit: u32, child: bool, flag: u32, ops: &PlaySetOps) -> bool {
    if unit == 0 {
        return false;
    }
    let name_text = unsafe { &*(name as *const RawCallString) }.bytes();
    if name_text.is_empty() {
        return false;
    }
    if child {
        return unsafe { (ops.init_child)(unit, name) };
    }
    let call = unsafe { BsCall::new(name_text, flag) };
    let pending = unit + ENTITY_PENDING_BS_CALL;
    let pending_empty = get_from_memory::<u32>(pending) == get_from_memory::<u32>(pending + 4);
    unsafe {
        if get_from_memory::<u8>(unit + ENTITY_HAS_PENDING_SET) != 0 {
            if pending_empty {
                (ops.set_pending)(unit, call.as_ptr(), 0);
            }
        } else if pending_empty {
            (ops.set_bs)(unit, call.as_ptr(), 0, SET_BS_NO_MIXER_CLEAR);
        } else {
            (ops.set_bs)(unit, pending, 0, SET_BS_NO_MIXER_CLEAR);
            (ops.set_pending)(unit, call.as_ptr(), 0);
        }
    }
    true
}

/// Copies a `BFBSCall` through vanilla's copy constructor, for a callee that takes it by value and frees it.
unsafe fn copy_bs_call(source: u32) -> [u32; 4] {
    let mut copy = [0u32; 4];
    unsafe { BFBSCALL_COPY.original()(copy.as_mut_ptr(), source as *const u32) };
    copy
}

unsafe fn init_child_vanilla(unit: u32, name: u32) -> bool {
    unsafe { BFBEHAVIORSET_INIT_CHILD.original()((unit + ENTITY_BEHAVIOR_SET) as *const u32, unit as i32, name as *const i32) }
}

unsafe fn set_bs_vanilla(unit: u32, call: u32, force: u32, mode: u32) {
    let [w0, w1, w2, w3] = unsafe { copy_bs_call(call) };
    unsafe { BFENTITY_SET_BS.original()(unit as *const u32, w0 as *const u32, w1 as *const u32, w2 as *const u32, w3 as i8, force as i8, mode as i32) };
}

unsafe fn set_pending_vanilla(unit: u32, call: u32, force: u32) {
    let [w0, w1, w2, w3] = unsafe { copy_bs_call(call) };
    unsafe { ZTANIMAL_SET_PENDING_BS.original()(unit as *const u32, w0 as *const u32, w1 as *const u32, w2 as *const u32, w3 as i8, force as i8) };
}

const PLAY_SET_VANILLA: PlaySetOps = PlaySetOps { init_child: init_child_vanilla, set_bs: set_bs_vanilla, set_pending: set_pending_vanilla };

/// `BFAIMgr::fPlaySetProbF(call, unit)`: an even number (at least two) of parameters, read as
/// `(name, weight)` pairs. Each name is the parameter's C string (up to its NUL, not its recorded end) and
/// each weight its converted text; `fPlaySetProb` then picks among them. The entry vector lives in Rust memory
/// (vanilla only reads it) and each name string is built and freed through vanilla's allocator.
pub unsafe fn f_play_set_prob_f(call: u32, unit: u32, ops: &SetProbOps) -> bool {
    let count = unsafe { call_param_count(call) };
    if count < 2 || count % 2 != 0 {
        return false;
    }
    let begin = unsafe { call_params_begin(call) };
    let mut entries: Vec<SetProbEntry> = Vec::with_capacity(count as usize / 2);
    for pair in 0..count as u32 / 2 {
        let name_text = get_from_memory::<u32>(begin + pair * 2 * CALL_PARAM_SIZE) as *const std::ffi::c_char;
        let weight_text = get_from_memory::<u32>(begin + (pair * 2 + 1) * CALL_PARAM_SIZE) as *const u8;
        let name = VanillaString::from_bytes(unsafe { std::ffi::CStr::from_ptr(name_text) }.to_bytes());
        let probability = unsafe { (ops.parse_int)(weight_text) };
        entries.push(SetProbEntry { name, probability });
    }
    let start = entries.as_ptr() as u32;
    let header = [start, start + entries.len() as u32 * 16, start + entries.capacity() as u32 * 16];
    unsafe { (ops.play_set_prob)(header.as_ptr() as u32, unit) }
}

/// `BFAIMgr::fRandomWalk` fields. Unit: position words (`+0x114`/`+0x118` in 1/64-tile units, `+0x11c` in
/// 1/16-tile height units), the walking guard byte `+0x20f`. Manager: the extra random extents `+0xd4`/`+0xd8`.
const UNIT_POS_Z: u32 = 0x11c;
const UNIT_WALKING_GUARD: u32 = 0x20f;
const AI_RANDOM_EXTRA_X: u32 = 0xd4;
const AI_RANDOM_EXTRA_Y: u32 = 0xd8;
/// `BFMap` fields: width (`+0x2c`), height (`+0x30`), tile array (`+0x38`, 0x8c-byte tiles).
const MAP_WIDTH: u32 = 0x2c;
const MAP_HEIGHT: u32 = 0x30;
const MAP_TILES: u32 = 0x38;
const MAP_TILE_SIZE: u32 = 0x8c;
/// A tile's three position words.
const TILE_POSITION_OFFSET: u32 = 0x34;
/// Manager vtable slots: `getPath(start, goal, unit, flag)` and the two random extents of a unit.
const AI_VT_GET_PATH: u32 = 0x18;
const AI_VT_RANDOM_X: u32 = 0x20;
const AI_VT_RANDOM_Y: u32 = 0x24;
/// Unit vtable slots: `canWalkOn(tile, flag)`, `getDestination(&position)` and `setDestination(&position)`.
const ENTITY_VT_CAN_WALK_ON: u32 = 0x168;
const ENTITY_VT_GET_DESTINATION: u32 = 0x1f4;
const ENTITY_VT_SET_DESTINATION: u32 = 0x1f8;
/// RVAs of the "no destination" position triple and the path-cost limit.
const RVA_NO_DESTINATION: u32 = 0x0023_9370;
const RVA_PATH_COST_LIMIT: u32 = 0x0023_5494;

/// The vanilla pieces `fRandomWalk` leans on, so a test can substitute recorders.
pub struct RandomWalkOps {
    /// The game's `rand()` (`0..=0x7fff`).
    pub rand: unsafe fn() -> u32,
    /// The world manager's pointer (the map is at `+8`).
    pub world: unsafe fn() -> u32,
    /// `BFMap::getNeighbors(map, tile)` as a plain list (the vanilla buffer is freed by the implementation).
    pub neighbors: unsafe fn(u32, u32) -> Vec<u32>,
    /// `BFUnit::getPathCost(unit, from_tile, to_tile)`.
    pub path_cost: unsafe fn(u32, u32, u32) -> i32,
    pub path_cost_limit: unsafe fn() -> i32,
    pub no_destination: unsafe fn() -> [i32; 3],
}

unsafe fn vcall_walk(unit: u32, tile: u32) -> bool {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(unit) + ENTITY_VT_CAN_WALK_ON);
    let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target) };
    method(unit, tile, 1) as u8 != 0
}

unsafe fn vcall_path(this: u32, start: &[i32; 3], goal: &[i32; 3], unit: u32, flag: u32) -> bool {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + AI_VT_GET_PATH);
    let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32>(target) };
    method(this, start.as_ptr() as u32, goal.as_ptr() as u32, unit, flag) as u8 != 0
}

/// The map's tile at tile coordinates `(x, y)`, or `0` outside the map.
fn map_tile(map: u32, x: i32, y: i32) -> u32 {
    let width = get_from_memory::<i32>(map + MAP_WIDTH);
    if x < 0 || y < 0 || x >= width || y >= get_from_memory::<i32>(map + MAP_HEIGHT) {
        return 0;
    }
    get_from_memory::<u32>(map + MAP_TILES).wrapping_add((y.wrapping_mul(width).wrapping_add(x) as u32).wrapping_mul(MAP_TILE_SIZE))
}

fn tile_position(tile: u32) -> [i32; 3] {
    [get_from_memory::<i32>(tile + TILE_POSITION_OFFSET), get_from_memory::<i32>(tile + TILE_POSITION_OFFSET + 4), get_from_memory::<i32>(tile + TILE_POSITION_OFFSET + 8)]
}

/// The tile offsets on the border of the box `[-half_x, half_x] x [-half_y, half_y]`, in the order `fRandomWalk`
/// tests them: `dx` ascending, then `dy` ascending (the centre and the interior are skipped).
fn border_offsets(half_x: i32, half_y: i32) -> impl Iterator<Item = (i32, i32)> {
    (-half_x..=half_x).flat_map(move |dx| (-half_y..=half_y).map(move |dy| (dx, dy))).filter(move |&(dx, dy)| dx == half_x || dx == -half_x || dy == half_y || dy == -half_y)
}

/// The neighbour indices `fRandomWalk` tries: from the random start index up to the end of the list. The scan
/// does not wrap, so the entries before the start index are never tried.
fn neighbor_scan(first: usize, len: usize) -> std::ops::Range<usize> {
    first..len
}

/// `BFAIMgr::fRandomWalk(unit)`: for an idle unit (no queued targets), tries in turn to resume a standing
/// destination (when it is not the unit's own tile), to walk to a random walkable tile on the border of a
/// random-sized box around it, to walk to a random walkable neighbour whose path cost is under the limit, and
/// finally to "walk" to its own position. The unit's `+0x20f` guard is set for the duration. The result is
/// whether a path was started.
///
/// `None` (defer to vanilla) where vanilla divides by zero (a zero random extent or an empty neighbour list)
/// or hands a null tile to `getNeighbors` (a unit off the map).
pub unsafe fn f_random_walk(this: u32, unit: u32, ops: &RandomWalkOps) -> Option<bool> {
    if unit == 0 || unsafe { target_list_count(unit) } != 0 {
        return Some(false);
    }
    let map = unsafe { (ops.world)() }.wrapping_add(OFFSET_WORLD_MAP);
    if map == 0 {
        return Some(false);
    }
    save_to_memory(unit + UNIT_WALKING_GUARD, 1u8);

    let div64 = |v: i32| v.wrapping_add((v >> 31) & 0x3f) >> 6;
    let mut height = get_from_memory::<i32>(unit + UNIT_POS_Z);
    if height < 0 {
        height = height.wrapping_sub(15);
    }
    let start = [div64(get_from_memory::<i32>(unit + OFFSET_UNIT_POS_X)), div64(get_from_memory::<i32>(unit + OFFSET_UNIT_POS_Y)), height.wrapping_add((height >> 31) & 0xf) >> 4];
    let start_tile = map_tile(map, start[0], start[1]);

    let mut goal = unsafe { (ops.no_destination)() };
    let get_destination = get_from_memory::<u32>(get_from_memory::<u32>(unit) + ENTITY_VT_GET_DESTINATION);
    let get_destination = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(get_destination) };
    let mut has_destination = get_destination(unit, goal.as_mut_ptr() as u32) as u8 != 0;
    let queued = unsafe { target_list_count(unit) } != 0;
    if has_destination && goal == start {
        has_destination = false;
    }
    let set_destination = |goal: &[i32; 3]| {
        let target = get_from_memory::<u32>(get_from_memory::<u32>(unit) + ENTITY_VT_SET_DESTINATION);
        let method = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(target) };
        method(unit, goal.as_ptr() as u32);
    };

    let mut result = queued;
    if !queued && has_destination {
        result = unsafe { vcall_path(this, &start, &goal, unit, 1) };
    }

    if !result && start_tile != 0 {
        let range_x = unsafe { vcall1(this, AI_VT_RANDOM_X, unit) } as i32;
        let range_y = unsafe { vcall1(this, AI_VT_RANDOM_Y, unit) } as i32;
        if range_x == 0 || range_y == 0 {
            return None;
        }
        let half_x = (unsafe { (ops.rand)() } as i32).wrapping_rem(range_x).wrapping_add(1).wrapping_add(get_from_memory::<i32>(this + AI_RANDOM_EXTRA_X));
        let half_y = (unsafe { (ops.rand)() } as i32).wrapping_rem(range_y).wrapping_add(1).wrapping_add(get_from_memory::<i32>(this + AI_RANDOM_EXTRA_Y));
        let mut candidates: Vec<u32> = Vec::new();
        for (dx, dy) in border_offsets(half_x, half_y) {
            let tile = map_tile(map, start[0].wrapping_add(dx), start[1].wrapping_add(dy));
            if tile != 0 && unsafe { vcall_walk(unit, tile) } {
                candidates.push(tile);
            }
        }
        if !candidates.is_empty() {
            let pick = (unsafe { (ops.rand)() } as i32).wrapping_rem(candidates.len() as i32) as usize;
            let tile = candidates[pick];
            if tile != 0 {
                goal = tile_position(tile);
                set_destination(&goal);
                result = unsafe { vcall_path(this, &start, &goal, unit, 0) };
            }
        }
    }

    if !result {
        if start_tile == 0 {
            return None;
        }
        let neighbors = unsafe { (ops.neighbors)(map, start_tile) };
        if neighbors.is_empty() {
            return None;
        }
        let first = (unsafe { (ops.rand)() } as i32).wrapping_rem(neighbors.len() as i32) as usize;
        for index in neighbor_scan(first, neighbors.len()) {
            let tile = neighbors[index];
            if unsafe { vcall_walk(unit, tile) } && unsafe { (ops.path_cost)(unit, start_tile, tile) } < unsafe { (ops.path_cost_limit)() } {
                goal = tile_position(tile);
                set_destination(&goal);
                result = unsafe { vcall_path(this, &start, &goal, unit, 0) };
            }
            if result {
                break;
            }
        }
    }
    if !result {
        result = unsafe { vcall_path(this, &start, &start, unit, 0) };
    }
    save_to_memory(unit + UNIT_WALKING_GUARD, 0u8);
    Some(result)
}

unsafe fn random_vanilla() -> u32 {
    unsafe { RAND.original()() }
}

unsafe fn world_vanilla() -> u32 {
    read_global_pointer(RVA_GLOBAL_WORLD_MGR)
}

unsafe fn neighbors_vanilla(map: u32, tile: u32) -> Vec<u32> {
    let mut header = [0u32; 3];
    unsafe { BFMAP_GET_NEIGHBORS.original()(map as *const u32, header.as_mut_ptr() as *const u32, tile as i32) };
    let (begin, end, cap) = (header[0], header[1], header[2]);
    let list = (begin..end).step_by(4).map(get_from_memory::<u32>).collect();
    if begin != 0 {
        unsafe { POOLALLOC_DEALLOCATE_N_4.original()(begin as *const u32, ((cap - begin) >> 2) as i32) };
    }
    list
}

unsafe fn path_cost_vanilla(unit: u32, from: u32, to: u32) -> i32 {
    (unsafe { BFUNIT_GET_PATH_COST.original()(unit as *const u32, from as i32, to as i32) }) as i32
}

unsafe fn path_cost_limit_vanilla() -> i32 {
    get_from_memory::<i32>(get_module_base("zoo.exe") as u32 + RVA_PATH_COST_LIMIT)
}

unsafe fn no_destination_vanilla() -> [i32; 3] {
    let base = get_module_base("zoo.exe") as u32 + RVA_NO_DESTINATION;
    [get_from_memory::<i32>(base), get_from_memory::<i32>(base + 4), get_from_memory::<i32>(base + 8)]
}

const RANDOM_WALK_VANILLA: RandomWalkOps = RandomWalkOps {
    rand: random_vanilla,
    world: world_vanilla,
    neighbors: neighbors_vanilla,
    path_cost: path_cost_vanilla,
    path_cost_limit: path_cost_limit_vanilla,
    no_destination: no_destination_vanilla,
};

unsafe fn animal_target_building_vanilla(unit: u32) -> bool {
    unsafe { ZTANIMAL_TARGET_BUILDING.original()(unit as *const u32, true) }
}

unsafe fn f_move_vanilla(this: u32, none: u32, unit: u32, done: u32, name: u32, speed: u32, empty: u32) -> bool {
    unsafe {
        F_MOVE.hooked()(
            this as *const u32,
            none as i32,
            0,
            unit as *const i32,
            done as *const u8,
            name as i32,
            speed,
            empty as *const i8,
            TARGET_BUILDING_MOVE_FLAG,
            TARGET_BUILDING_MOVE_LIMIT,
        )
    }
}

const TARGET_BUILDING_VANILLA: TargetBuildingOps = TargetBuildingOps { target_building: animal_target_building_vanilla, f_move: f_move_vanilla };

/// `BFTile` byte whose low two bits mark a water tile.
const TILE_WATER_FLAGS_OFFSET: u32 = 0x83;

/// Terrain flag byte of the unit's tile, through the vanilla `BFEntity::getTile`.
unsafe fn unit_tile_flags_vanilla(unit: u32) -> u8 {
    let tile = unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) } as u32;
    get_from_memory::<u8>(tile + TILE_WATER_FLAGS_OFFSET)
}

/// `ZTAIMgr::fPlaySetTerrainF(call, unit)` for the four-parameter form: clears the call's result flag, picks a
/// set name by the unit's `+0x258` selector (`4` the fourth parameter, `3` the third, otherwise the first on a
/// land tile and the second on a water tile) and plays it with `play_set(name, unit)`, returning its low byte.
/// The name is a fresh vanilla string holding a copy of the parameter (vanilla starts from an empty string and
/// assigns the parameter into it, so the contents match).
///
/// `None` (defer to vanilla) for any other parameter count, where vanilla forwards to `BFAIMgr`'s handler, and
/// for a unit that is null or not a `ZTUnitType`, where vanilla reads the selector through null.
pub unsafe fn f_play_set_terrain_f(call: u32, unit: u32, tile_flags: unsafe fn(u32) -> u8, play_set: unsafe fn(u32, u32) -> u32) -> Option<bool> {
    if unsafe { call_param_count(call) } != 4 || !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        return None;
    }
    save_to_memory(call + CALL_DONE_FLAG_OFFSET, 0u8);
    let index = match get_from_memory::<u32>(unit + UNIT_TANK_SELECTOR_OFFSET) {
        4 => 3,
        3 => 2,
        _ if unsafe { tile_flags(unit) } & 3 == 0 => 0,
        _ => 1,
    };
    let name = unsafe { call_params_begin(call) } + index * CALL_PARAM_SIZE;
    Some(low_byte_bool(unsafe { play_set(name, unit) }))
}

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

/// Unit fields read by `fMoveEntitySpecialF`: facing direction and tile-fraction position pair.
const OFFSET_UNIT_DIRECTION: u32 = 0x12c;
const OFFSET_UNIT_POS_X: u32 = 0x114;
const OFFSET_UNIT_POS_Y: u32 = 0x118;
/// Byte cleared by `fMoveEntityDeltaF` on a `ZTUnitType` unit.
const OFFSET_UNIT_MOVE_FLAG: u32 = 0x20d;
/// `GLOBAL_ZTWorldMgr`'s embedded `BFMap` sits at `+8`.
const OFFSET_WORLD_MAP: u32 = 8;

unsafe fn move_entity_delta_vanilla(map: u32, unit: u32, delta: *const [i32; 3]) {
    unsafe { BFMAP_MOVE_ENTITY_DELTA.original()(map as *const u32, unit as *const u32, delta as *const i32) };
}

/// The three call parameters converted through the manager's `convertConstantParam` virtual, in order, or
/// `None` unless the call has exactly three.
unsafe fn convert_three_params(this: u32, call: u32, unit: u32) -> Option<[i32; 3]> {
    if unsafe { call_param_count(call) } != 3 {
        return None;
    }
    let begin = unsafe { call_params_begin(call) };
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + AI_VTABLE_CONVERT_CONSTANT_PARAM);
    let convert = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> i32>(target) };
    Some([convert(this, begin, unit), convert(this, begin + 0xc, unit), convert(this, begin + 0x18, unit)])
}

/// `fMoveEntityDeltaF(call, unit)`: clears the unit's mixer (and, on a `ZTUnitType` unit, its `+0x20d` byte),
/// then with exactly three parameters moves the unit by that delta. Returns `0` or `1`. `None` (defer) for a
/// null unit, where vanilla faults inside `clearAll`.
pub unsafe fn f_move_entity_delta_f(
    this: u32, call: u32, unit: u32, clear_all: unsafe fn(u32), move_delta: unsafe fn(u32, u32, *const [i32; 3]),
) -> Option<u8> {
    if unit == 0 {
        return None;
    }
    if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        save_to_memory(unit + OFFSET_UNIT_MOVE_FLAG, 0u8);
    }
    unsafe { clear_all(unit + 0x74) };
    let map = read_global_pointer(RVA_GLOBAL_WORLD_MGR).wrapping_add(OFFSET_WORLD_MAP);
    if map == 0 {
        return Some(0);
    }
    let Some(delta) = (unsafe { convert_three_params(this, call, unit) }) else {
        return Some(0);
    };
    unsafe { move_delta(map, unit, &delta) };
    Some(1)
}

/// The delta `fMoveEntitySpecialF` passes to `moveEntityDelta`: the first two parameters rotated by the
/// unit's facing (`2`: `(b, -a)`, `6`: `(-b, a)`, `0`: `(-a, -b)`, anything else `(a, b)`), re-centered on the
/// unit's sub-tile position (`pos % 64`, C remainder) around `0x20`; the third parameter passes through.
pub fn special_move_delta(params: [i32; 3], direction: i32, pos_x: i32, pos_y: i32) -> [i32; 3] {
    let [a, b, c] = params;
    let (x, y) = match direction {
        2 => (b, a.wrapping_neg()),
        6 => (b.wrapping_neg(), a),
        0 => (a.wrapping_neg(), b.wrapping_neg()),
        _ => (a, b),
    };
    [x.wrapping_sub(pos_x % 64).wrapping_add(0x20), y.wrapping_sub(pos_y % 64).wrapping_add(0x20), c]
}

/// `fMoveEntitySpecialF(call, unit)`: with exactly three parameters, moves the unit by `special_move_delta`.
/// Returns `0` or `1`. `None` (defer) for a null unit when the move would run (vanilla reads the unit's fields).
pub unsafe fn f_move_entity_special_f(this: u32, call: u32, unit: u32, move_delta: unsafe fn(u32, u32, *const [i32; 3])) -> Option<u8> {
    if unsafe { call_param_count(call) } != 3 {
        return Some(0);
    }
    let map = read_global_pointer(RVA_GLOBAL_WORLD_MGR).wrapping_add(OFFSET_WORLD_MAP);
    if map == 0 {
        return Some(0);
    }
    if unit == 0 {
        return None;
    }
    let params = unsafe { convert_three_params(this, call, unit) }?;
    let delta = special_move_delta(
        params,
        get_from_memory::<i32>(unit + OFFSET_UNIT_DIRECTION),
        get_from_memory::<i32>(unit + OFFSET_UNIT_POS_X),
        get_from_memory::<i32>(unit + OFFSET_UNIT_POS_Y),
    );
    unsafe { move_delta(map, unit, &delta) };
    Some(1)
}

unsafe fn clear_mixer_vanilla(mixer: u32) {
    unsafe { GXMIXER_CLEAR_ALL.original()(mixer as *const u32) };
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

/// The animation name `fPlaySound` hands to `fPlaySpecial`: the text `0` (vanilla builds it in a temporary
/// byte vector).
const PLAY_SOUND_ANIM_NAME: &str = "0";

/// `BFAIMgr::fPlaySound(sound, unit, done)`: plays a sound alone by calling `fPlaySpecial(anim "0", sound, 0, 0,
/// plain-mode string)`. `mode_a` is the value of the `[0x0064bdac]` global (the string pointer vanilla passes);
/// `play_special` is `fPlaySpecial`. Returns its result.
pub unsafe fn f_play_sound(this: u32, sound: u32, unit: u32, done: *const u8, mode_a: u32, play_special: unsafe fn(&FPlaySpecialArgs) -> bool) -> bool {
    let anim = VanillaString::new(PLAY_SOUND_ANIM_NAME);
    let args = FPlaySpecialArgs { this, anim_name: anim.as_ptr() as u32, sound_name: sound, seconds: 0, count: 0, special: mode_a, unit, done };
    unsafe { play_special(&args) }
}

/// The handlers `fPlaySpecialF` rewrites its call for and forwards to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpecialHandler {
    Play,
    PlayReverse,
    PlayPingPong,
    PlayWithSound,
    PlayTime,
}

/// The vanilla pieces `fPlaySpecialF` leans on. `convert(this, param, unit)` is the manager's
/// `convertConstantParam` virtual; `replace_params(call, strings)` erases the call's parameters and appends a
/// copy of each string; `set_name(call, text)` assigns the call's name; `forward` runs the handler (`fPlayF` with
/// both flags clear for `Play`) on the rewritten call and returns its result; `play_special` is `fPlaySpecial`.
pub struct PlaySpecialFOps {
    pub is_string: unsafe fn(SpecialString, u32) -> bool,
    pub convert: unsafe fn(u32, u32, u32) -> i32,
    pub replace_params: unsafe fn(u32, &[u32]),
    pub set_name: unsafe fn(u32, &[u8]),
    pub forward: unsafe fn(SpecialHandler, u32, u32, u32) -> bool,
    pub play_special: unsafe fn(&FPlaySpecialArgs) -> bool,
}

/// `BFAIMgr::fPlaySpecialF(call, unit)`: the five-parameter `(anim, sound, seconds, count, special)` form.
///
/// Fails unless the call has five parameters or its result flag is clear. Converts `seconds` (to milliseconds)
/// and `count` through the manager, works on copies of parameters 0, 1 and 4, and then rewrites the call:
/// - both numbers set: fails;
/// - `count` set: `fPlaySpecial` as is;
/// - `seconds` set, sound "none" and special one of the plain modes or "none": the call becomes
///   `fPlayTime(anim, seconds)` with its second flag set when special is `ModeB`;
/// - neither set, sound "none": the call becomes `fPlay`/`fPlayReverse`/`fPlayPingPong` by special (plain modes
///   and "none" play, then every path ends in `fPlayPingPongF`, whose result is returned);
/// - neither set, sound named, anim named: `fPlayWithSound(anim, sound)`;
/// - neither set, sound named, anim "none": the second flag is set when special is `ModeB` and `fPlaySpecial`
///   runs with no duration or count.
/// Anything else is `fPlaySpecial(anim, sound, seconds, count, special)`.
///
/// `None` (defer to vanilla) for a call with fewer than five parameters and a clear flag, where vanilla reads
/// past the parameter vector.
pub unsafe fn f_play_special_f(this: u32, call: u32, unit: u32, ops: &PlaySpecialFOps) -> Option<bool> {
    let count = unsafe { call_param_count(call) };
    if count != 5 && get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET) != 0 {
        return Some(false);
    }
    if count < 5 {
        return None;
    }
    let begin = unsafe { call_params_begin(call) };
    let param = |index: u32| begin + index * CALL_PARAM_SIZE;
    let duration_text = unsafe { param_string(param(2)) };
    let seconds = unsafe { (ops.convert)(this, param(2), unit) }.wrapping_mul(MILLIS_PER_SECOND as i32);
    let repeat = unsafe { (ops.convert)(this, param(3), unit) };
    let (anim, sound, special) = unsafe { (param_string(param(0)), param_string(param(1)), param_string(param(4))) };

    let is = |which, string: &VanillaString| unsafe { (ops.is_string)(which, string.as_ptr() as u32) };
    let play_special = |seconds: i32, repeat: i32| {
        let args = FPlaySpecialArgs {
            this,
            anim_name: anim.as_ptr() as u32,
            sound_name: sound.as_ptr() as u32,
            seconds: seconds as u32,
            count: repeat,
            special: special.as_ptr() as u32,
            unit,
            done: (call + CALL_DONE_FLAG_OFFSET) as *const u8,
        };
        unsafe { (ops.play_special)(&args) }
    };
    let rewrite = |strings: &[u32], name: &[u8], handler| unsafe {
        (ops.replace_params)(call, strings);
        (ops.set_name)(call, name);
        (ops.forward)(handler, this, call, unit)
    };

    let facts = PlaySpecialFFacts {
        seconds: seconds != 0,
        repeat: repeat != 0,
        sound_none: is(SpecialString::None, &sound),
        anim_none: is(SpecialString::None, &anim),
        special_mode_a: is(SpecialString::ModeA, &special),
        special_mode_b: is(SpecialString::ModeB, &special),
        special_none: is(SpecialString::None, &special),
        special_reverse: is(SpecialString::Reverse, &special),
        special_ping_pong: is(SpecialString::PingPong, &special),
    };
    match decide_play_special_f(facts) {
        PlaySpecialFAction::Fail => Some(false),
        PlaySpecialFAction::PlaySpecial { use_seconds, use_repeat } => Some(play_special(if use_seconds { seconds } else { 0 }, if use_repeat { repeat } else { 0 })),
        PlaySpecialFAction::PlayTime { second_flag } => {
            unsafe { (ops.replace_params)(call, &[anim.as_ptr() as u32, duration_text.as_ptr() as u32]) };
            unsafe { (ops.set_name)(call, b"fPlayTime") };
            save_to_memory(call + CALL_FLAG_B_OFFSET, u8::from(second_flag));
            Some(unsafe { (ops.forward)(SpecialHandler::PlayTime, this, call, unit) })
        }
        PlaySpecialFAction::NoSound(kind) => {
            unsafe { (ops.replace_params)(call, &[anim.as_ptr() as u32]) };
            match kind {
                NoSoundPlay::Plain => {
                    unsafe { (ops.set_name)(call, b"fPlay") };
                    unsafe { (ops.forward)(SpecialHandler::Play, this, call, unit) };
                }
                NoSoundPlay::Reverse => {
                    unsafe { (ops.set_name)(call, b"fPlayReverse") };
                    unsafe { (ops.forward)(SpecialHandler::PlayReverse, this, call, unit) };
                }
                NoSoundPlay::PingPong => unsafe { (ops.set_name)(call, b"fPlayPingPong") },
                NoSoundPlay::Other => {}
            }
            Some(unsafe { (ops.forward)(SpecialHandler::PlayPingPong, this, call, unit) })
        }
        PlaySpecialFAction::PlayWithSound => Some(rewrite(&[anim.as_ptr() as u32, sound.as_ptr() as u32], b"fPlayWithSound", SpecialHandler::PlayWithSound)),
        PlaySpecialFAction::PlaySpecialNoAnim { second_flag } => {
            save_to_memory(call + CALL_FLAG_B_OFFSET, u8::from(second_flag));
            Some(play_special(0, 0))
        }
    }
}

/// What `fPlaySpecialF` has to decide on, with the string comparisons already made.
#[derive(Clone, Copy, Default, Debug)]
struct PlaySpecialFFacts {
    seconds: bool,
    repeat: bool,
    sound_none: bool,
    anim_none: bool,
    special_mode_a: bool,
    special_mode_b: bool,
    special_none: bool,
    special_reverse: bool,
    special_ping_pong: bool,
}

/// How a call with no duration, no repeat count and the "none" sound is rewritten: by the fifth parameter, the
/// plain modes and "none" give `fPlay`, then reverse and ping-pong their own names; any other value keeps the
/// name. Every kind ends by forwarding to `fPlayPingPongF`, whose result is returned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NoSoundPlay {
    Plain,
    Reverse,
    PingPong,
    Other,
}

/// The action `fPlaySpecialF` takes for a five-parameter call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PlaySpecialFAction {
    /// A duration and a repeat count together fail.
    Fail,
    /// `fPlaySpecial(anim, sound, seconds?, repeat?, special)`; an unused number is passed as `0`.
    PlaySpecial { use_seconds: bool, use_repeat: bool },
    /// Rewritten to `fPlayTime(anim, duration)`, with the call's second flag set for the second plain mode.
    PlayTime { second_flag: bool },
    NoSound(NoSoundPlay),
    /// Rewritten to `fPlayWithSound(anim, sound)`.
    PlayWithSound,
    /// The "none" animation with a named sound: the second flag is set for the second plain mode and
    /// `fPlaySpecial` runs with no duration or count.
    PlaySpecialNoAnim { second_flag: bool },
}

fn decide_play_special_f(facts: PlaySpecialFFacts) -> PlaySpecialFAction {
    let plain_special = facts.special_mode_a || facts.special_mode_b || facts.special_none;
    if facts.seconds && facts.repeat {
        return PlaySpecialFAction::Fail;
    }
    if facts.repeat {
        return PlaySpecialFAction::PlaySpecial { use_seconds: false, use_repeat: true };
    }
    if facts.seconds {
        if facts.sound_none && plain_special {
            return PlaySpecialFAction::PlayTime { second_flag: facts.special_mode_b };
        }
        return PlaySpecialFAction::PlaySpecial { use_seconds: true, use_repeat: false };
    }
    if facts.sound_none {
        return PlaySpecialFAction::NoSound(if plain_special {
            NoSoundPlay::Plain
        } else if facts.special_reverse {
            NoSoundPlay::Reverse
        } else if facts.special_ping_pong {
            NoSoundPlay::PingPong
        } else {
            NoSoundPlay::Other
        });
    }
    if !facts.anim_none {
        return PlaySpecialFAction::PlayWithSound;
    }
    PlaySpecialFAction::PlaySpecialNoAnim { second_flag: facts.special_mode_b }
}

/// The call's parameter vector (`call + 0xc`) is a vanilla `vector<string>`.
const CALL_PARAM_VECTOR_OFFSET: u32 = 0xc;

unsafe fn convert_param_vanilla(this: u32, param: u32, unit: u32) -> i32 {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + AI_VTABLE_CONVERT_CONSTANT_PARAM);
    let convert = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> i32>(target) };
    convert(this, param, unit)
}

unsafe fn replace_params_vanilla(call: u32, strings: &[u32]) {
    let vector = call + CALL_PARAM_VECTOR_OFFSET;
    unsafe { ERASE_RANGE_0X410AC3.original()(vector as *const u32, call_params_begin(call) as *const u32, call_params_end(call) as i32) };
    for &string in strings {
        unsafe { PUSH_BACK_0X4FCEF5.original()(vector as *const u32, string as *const i32) };
    }
}

unsafe fn set_call_name_vanilla(call: u32, text: &[u8]) {
    let begin = text.as_ptr() as u32;
    unsafe { ASSIGN_RANGE_0X406C55.original()(call as *const c_void, begin as *const u32, (begin + text.len() as u32) as *const u32, call) };
}

unsafe fn forward_special_vanilla(handler: SpecialHandler, this: u32, call: u32, unit: u32) -> bool {
    let (this, call, unit) = (this as *const u32, call as *const u32, unit as *const u32);
    match handler {
        SpecialHandler::Play => unsafe { F_PLAY_F.hooked()(this, call, unit, 0, 0) },
        SpecialHandler::PlayReverse => unsafe { F_PLAY_REVERSE_F.hooked()(this, call, unit) },
        SpecialHandler::PlayPingPong => unsafe { F_PLAY_PING_PONG_F.hooked()(this, call, unit) },
        SpecialHandler::PlayWithSound => unsafe { F_PLAY_WITH_SOUND_F.hooked()(call, unit) },
        SpecialHandler::PlayTime => unsafe { F_PLAY_TIME_F.hooked()(this, call, unit) },
    }
}

unsafe fn play_special_f_forward(args: &FPlaySpecialArgs) -> bool {
    unsafe {
        F_PLAY_SPECIAL.hooked()(
            args.this as *const u32,
            args.anim_name as *const i8,
            args.sound_name as *const i8,
            args.seconds as i32,
            args.count,
            args.special as *const i8,
            args.unit as *const i32,
            args.done,
        )
    }
}

const PLAY_SPECIAL_F_VANILLA: PlaySpecialFOps = PlaySpecialFOps {
    is_string: special_string_is,
    convert: convert_param_vanilla,
    replace_params: replace_params_vanilla,
    set_name: set_call_name_vanilla,
    forward: forward_special_vanilla,
    play_special: play_special_f_forward,
};

unsafe fn play_time_vanilla(this: u32, name: u32, millis: u32, unit: u32, done: u32, flag_b: u32) -> u32 {
    u32::from(unsafe { F_PLAY_TIME.hooked()(this as *const u32, name as *const c_void, millis as i32, unit as *const u32, done as *const i8, flag_b as i8) })
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

/// The `[AI]` parameters `BFAIMgr::initAIParams` loads. The first four fill the random-walk extents
/// (`base_*` is the extra added to the roll, `random_*` the roll's range); the `show_*`/`ai_info_nth` flags drive
/// the debug overlay; `clear_unreachable_freq` and `min_unreachable_count` go to two globals rather than the
/// manager.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BfAiParams {
    pub base_x: i32,
    pub base_y: i32,
    pub random_x: i32,
    pub random_y: i32,
    pub how_often: i32,
    pub tick: i32,
    pub status_period: i32,
    pub goal_period: i32,
    pub show_ai_info: i32,
    pub show_name: i32,
    pub show_position: i32,
    pub show_status_vars: i32,
    pub show_function_call: i32,
    pub show_events: i32,
    pub ai_info_nth: i32,
    pub show_selected: i32,
    pub show_frame: i32,
    pub show_goal: i32,
    pub variable_unit_speeds: i32,
    pub allow_diagonal_moves: bool,
    pub clear_unreachable_freq: i32,
    pub min_unreachable_count: u8,
}

/// Reads every `BFAIMgr::initAIParams` key in vanilla's order: `read(key, default)` reads one integer. The
/// two base extents are clamped to at least `0` and the two random extents to at least `1`;
/// `AllowDiagonalMoves` is true only for the value `1`, and `MinUnreachableCount` keeps its low byte.
pub fn compute_bf_ai_params(mut read: impl FnMut(&str, i32) -> i32) -> BfAiParams {
    let base_x = read("BaseX", 1);
    let base_y = read("BaseY", 1);
    let random_x = read("RandomX", 5);
    let random_y = read("RandomY", 5);
    BfAiParams {
        base_x: base_x.max(0),
        base_y: base_y.max(0),
        random_x: random_x.max(1),
        random_y: random_y.max(1),
        how_often: read("mAIHowOften", 1),
        tick: read("mAITick", 1000),
        status_period: read("mStatusPeriod", 1000),
        goal_period: read("mGoalPeriod", 0),
        show_ai_info: read("ShowAIInfo", 0),
        show_name: read("ShowName", 1),
        show_position: read("ShowPosition", 1),
        show_status_vars: read("ShowStatusVars", 1),
        show_function_call: read("ShowFunctionCall", 1),
        show_events: read("ShowEvents", 1),
        ai_info_nth: read("AIInfoNth", 100),
        show_selected: read("ShowSelected", 0),
        show_frame: read("ShowFrame", 0),
        show_goal: read("ShowGoal", 0),
        variable_unit_speeds: read("VariableUnitSpeeds", 1),
        allow_diagonal_moves: read("AllowDiagonalMoves", 1) == 1,
        clear_unreachable_freq: read("ClearUnreachableFreq", 5),
        min_unreachable_count: read("MinUnreachableCount", 5) as u8,
    }
}

/// Globals `initAIParams` writes: the clear-unreachable frequency (a dword) and the minimum unreachable count (a
/// byte).
const RVA_CLEAR_UNREACHABLE_FREQ: u32 = 0x0023_9028;
const RVA_MIN_UNREACHABLE_COUNT: u32 = 0x0023_9532;
/// `BFAIMgr` fields `initAIParams` fills.
const BF_AI_PARAM_OFFSETS: [(u32, fn(&BfAiParams) -> i32); 19] = [
    (0xd4, |p| p.base_x),
    (0xd8, |p| p.base_y),
    (0xdc, |p| p.random_x),
    (0xe0, |p| p.random_y),
    (0xf0, |p| p.how_often),
    (0xe4, |p| p.tick),
    (0x124, |p| p.status_period),
    (0x128, |p| p.goal_period),
    (0xf4, |p| p.show_ai_info),
    (0xf8, |p| p.show_name),
    (0xfc, |p| p.show_position),
    (0x100, |p| p.show_status_vars),
    (0x108, |p| p.show_function_call),
    (0x10c, |p| p.show_events),
    (0x110, |p| p.ai_info_nth),
    (0x104, |p| p.show_selected),
    (0x114, |p| p.show_frame),
    (0x118, |p| p.show_goal),
    (0x11c, |p| p.variable_unit_speeds),
];
const OFFSET_ALLOW_DIAGONAL_MOVES: u32 = 0x120;

/// `BFAIMgr::initAIParams()` (`0x005267ec`): loads the `[AI]` parameters (see [`compute_bf_ai_params`]) into the
/// manager and the two globals. Vanilla writes each field's default before reading it, so a failed read keeps
/// the default; the result is the same. Returns `1`.
pub unsafe fn bfaimgr_init_ai_params(this: u32) -> u32 {
    let section = VanillaString::new(AI_INI_SECTION);
    let params = compute_bf_ai_params(|key, default| {
        let key = VanillaString::new(key);
        unsafe { BFINIFILE_READ.original()(section.as_ptr(), key.as_ptr(), default as u32) as i32 }
    });
    for (offset, field) in BF_AI_PARAM_OFFSETS {
        save_to_memory(this + offset, field(&params));
    }
    save_to_memory(this + OFFSET_ALLOW_DIAGONAL_MOVES, u8::from(params.allow_diagonal_moves));
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(base + RVA_CLEAR_UNREACHABLE_FREQ, params.clear_unreachable_freq);
    save_to_memory(base + RVA_MIN_UNREACHABLE_COUNT, params.min_unreachable_count);
    1
}

/// The game's `rand` state: one dword.
const RVA_RAND_STATE: u32 = 0x0023_8060;
/// `BFAIMgr` vtable slot `initAIParams`.
const BFAIMGR_VTABLE_INIT_AI_PARAMS: u32 = 0x38;

/// `BFAIMgr::init()` (`0x005267b5`): zeroes the tick remainder and counter, calls the manager's virtual
/// `initAIParams` (during construction that is still the `BFAIMgr` one, so it must go through the vtable) and
/// seeds the game's `rand` state from the game's own `time(NULL)`. Returns the virtual's low byte.
pub unsafe fn bfaimgr_init(this: u32) -> bool {
    save_to_memory(this + OFFSET_AI_TICK_REMAINDER, 0u32);
    save_to_memory(this + OFFSET_AI_TICK_COUNT, 0u32);
    let target = get_from_memory::<u32>(get_from_memory::<u32>(this) + BFAIMGR_VTABLE_INIT_AI_PARAMS);
    let result = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target) }(this);
    let seconds = unsafe { TIME.original()(std::ptr::null()) } as u32;
    save_to_memory(get_module_base("zoo.exe") as u32 + RVA_RAND_STATE, seconds);
    low_byte_bool(result)
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
    unsafe extern "stdcall" fn f_end_trick_f_detour(call: i32, unit: *const i32) -> bool {
        unsafe { f_end_trick(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_ENTER_BUILDING_F)]
    unsafe extern "stdcall" fn f_enter_building_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_enter_building_al(unit as u32, call_done_flag(call as u32)) } {
            Some(al) => al != 0,
            None => unsafe { F_ENTER_BUILDING_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_RETURN_TO_BUILDING_F)]
    unsafe extern "stdcall" fn f_return_to_building_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_return_to_building_al(unit as u32, call_done_flag(call as u32)) } {
            Some(al) => al != 0,
            None => unsafe { F_RETURN_TO_BUILDING_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_HIDE_PREDATOR_F)]
    unsafe extern "stdcall" fn f_hide_predator_f_detour(call: i32, unit: *const u8) -> bool {
        if unsafe { f_hide_predator(unit as u32, call_done_flag(call as u32)) } {
            true
        } else {
            unsafe { F_HIDE_PREDATOR_F_DETOUR.call(call, unit) }
        }
    }

    #[detour(F_GUEST_ROUTINE_F)]
    unsafe extern "stdcall" fn f_guest_routine_f_detour(call: u32, unit: u32) -> bool {
        unsafe { f_guest_routine(unit, call_done_flag(call)) }
    }

    #[detour(F_MAINT_ROUTINE_F)]
    unsafe extern "stdcall" fn f_maint_routine_f_detour(call: u32, unit: u32) -> bool {
        unsafe { f_maint_routine(unit, call_done_flag(call)) }
    }

    #[detour(F_GUIDE_ROUTINE_F)]
    unsafe extern "stdcall" fn f_guide_routine_f_detour(call: u32, unit: u32) -> bool {
        unsafe { f_guide_routine(unit, call_done_flag(call)) }
    }

    #[detour(F_KEEPER_ROUTINE_F)]
    unsafe extern "stdcall" fn f_keeper_routine_f_detour(call: u32, unit: *const i32) -> bool {
        unsafe { f_keeper_routine(unit as u32, call_done_flag(call)) }
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
    unsafe extern "stdcall" fn f_show_predator_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_show_predator(unit as u32, call_done_flag(call as u32)) } {
            Some(result) => result,
            None => unsafe { F_SHOW_PREDATOR_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_CAUGHT_PREY)]
    unsafe extern "stdcall" fn f_caught_prey_detour(unit: *const i32, done: *const u8) -> u32 {
        unsafe { f_caught_prey(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_CAUGHT_PREY_F)]
    unsafe extern "stdcall" fn f_caught_prey_f_detour(call: i32, unit: *const i32) -> bool {
        unsafe { f_caught_prey(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_KILL_PREY)]
    unsafe extern "stdcall" fn f_kill_prey_detour(unit: *const u8, done: *const u8) -> u32 {
        unsafe { f_kill_prey(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_KILL_PREY_F)]
    unsafe extern "stdcall" fn f_kill_prey_f_detour(call: i32, unit: *const u8) -> bool {
        unsafe { f_kill_prey(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_DIE)]
    unsafe extern "stdcall" fn f_die_detour(unit: *const i32, done: *const u8) -> u32 {
        unsafe { f_die(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_DIE_F)]
    unsafe extern "stdcall" fn f_die_f_detour(call: i32, unit: *const i32) -> bool {
        unsafe { f_die(unit as u32, call_done_flag(call as u32)) }
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
    unsafe extern "stdcall" fn f_hatch_f_detour(call: i32, unit: *const i32) -> bool {
        if unsafe { f_hatch(unit as u32, call_done_flag(call as u32)) } {
            true
        } else {
            unsafe { F_HATCH_F_DETOUR.call(call, unit) }
        }
    }

    #[detour(F_EXIT_BUILDING)]
    unsafe extern "stdcall" fn f_exit_building_detour(unit: *const i32, done: *const u8) {
        if !unsafe { f_exit_building(unit as u32, done as *const bool) } {
            unsafe { F_EXIT_BUILDING_DETOUR.call(unit, done) };
        }
    }

    #[detour(F_EXIT_BUILDING_F)]
    unsafe extern "stdcall" fn f_exit_building_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_exit_building_al(unit as u32, call_done_flag(call as u32)) } {
            Some(al) => al != 0,
            None => unsafe { F_EXIT_BUILDING_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_FACE_TOWARD_FOOD)]
    unsafe extern "stdcall" fn f_face_toward_food_detour(unit: *const u8, done: *const u8) -> u32 {
        unsafe { f_face_toward_food(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_FACE_TOWARD_FOOD_F)]
    unsafe extern "stdcall" fn f_face_toward_food_f_detour(call: i32, unit: *const u8) -> bool {
        unsafe { f_face_toward_food(unit as u32, call_done_flag(call as u32)) }
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
    unsafe extern "stdcall" fn f_face_toward_target_f_detour(call: i32, unit: *const c_void) -> bool {
        if unsafe { f_face_toward_target(unit as u32, call_done_flag(call as u32)) } {
            true
        } else {
            unsafe { F_FACE_TOWARD_TARGET_F_DETOUR.call(call, unit) }
        }
    }

    #[detour(ZTAIMGR_F_PLAY_SET_TERRAIN_F)]
    unsafe extern "thiscall" fn f_play_set_terrain_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        match unsafe { f_play_set_terrain_f(call as u32, unit as u32, unit_tile_flags_vanilla, play_set_vanilla) } {
            Some(result) => result,
            None => unsafe { ZTAIMGR_F_PLAY_SET_TERRAIN_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_DUST_BALL)]
    unsafe extern "thiscall" fn f_dust_ball_detour(this: *const u32, unit: *const i32, done: *const i8) -> bool {
        match unsafe { f_dust_ball(this as u32, unit as u32, done as *const bool, &DUST_BALL_VANILLA) } {
            Some(result) => result,
            None => unsafe { F_DUST_BALL_DETOUR.call(this, unit, done) },
        }
    }

    #[detour(F_DUST_BALL_F)]
    unsafe extern "thiscall" fn f_dust_ball_f_detour(this: *const u32, call: i32, unit: *const i32) -> bool {
        match unsafe { f_dust_ball(this as u32, unit as u32, call_done_flag(call as u32), &DUST_BALL_VANILLA) } {
            Some(result) => result,
            None => unsafe { F_DUST_BALL_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_RANDOM_WALK)]
    unsafe extern "thiscall" fn f_random_walk_detour(this: *const u32, unit: *const i32) -> u32 {
        match unsafe { f_random_walk(this as u32, unit as u32, &RANDOM_WALK_VANILLA) } {
            Some(result) => u32::from(result),
            None => unsafe { F_RANDOM_WALK_DETOUR.call(this, unit) },
        }
    }

    #[detour(F_FLY_F)]
    unsafe extern "thiscall" fn f_fly_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: i8) -> bool {
        unsafe { f_fly_f(this as u32, call as u32, unit as u32, u32::from(reverse as u8), &FLY_VANILLA) }
    }

    #[detour(F_PLAY_SET_PROB_F)]
    unsafe extern "stdcall" fn f_play_set_prob_f_detour(call: *const u32, unit: *const u32) -> bool {
        unsafe { f_play_set_prob_f(call as u32, unit as u32, &SET_PROB_VANILLA) }
    }

    #[detour(F_PLAY_F)]
    unsafe extern "thiscall" fn f_play_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
        unsafe { f_play_f(this as u32, call as u32, unit as u32, reverse != 0, ping_pong != 0, &PLAY_VANILLA_OPS) }
    }

    #[detour(F_PLAY_WITH_BASE_F)]
    unsafe extern "thiscall" fn f_play_with_base_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
        unsafe { f_play_with_base_f(this as u32, call as u32, unit as u32, reverse != 0, ping_pong != 0, &PLAY_VANILLA_OPS) }
    }

    #[detour(F_SET_BASE_F)]
    unsafe extern "thiscall" fn f_set_base_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { f_set_base_f(this as u32, call as u32, unit as u32, &PLAY_VANILLA_OPS) }
    }

    #[detour(F_PLAY_SPECIAL)]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "thiscall" fn f_play_special_detour(this: *const u32, anim: *const i8, sound: *const i8, seconds: i32, count: i32, special: *const i8, unit: *const i32, done: *const u8) -> bool {
        let args = FPlaySpecialArgs { this: this as u32, anim_name: anim as u32, sound_name: sound as u32, seconds: seconds as u32, count, special: special as u32, unit: unit as u32, done };
        unsafe { f_play_special(&args, &F_PLAY_SPECIAL_VANILLA_OPS) }
    }

    #[detour(F_MOVE)]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "thiscall" fn f_move_detour(this: *const u32, first_param: i32, seconds: i32, unit: *const i32, done: *const u8, name: i32, speed: u32, extra: *const i8, reverse: u32, limit: u32) -> bool {
        let args = FMoveArgs { this: this as u32, first_param: first_param as u32, seconds, unit: unit as u32, done, name: name as u32, speed, extra: extra as u32, reverse, limit };
        unsafe { f_move(&args, &F_MOVE_VANILLA_OPS) }
    }

    #[detour(F_MOVE_F)]
    unsafe extern "thiscall" fn f_move_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: bool) -> u32 {
        u32::from(unsafe { f_move_f(this as u32, call as u32, unit as u32, reverse, &MOVE_VANILLA) })
    }

    #[detour(F_MOVE_REVERSE_F)]
    unsafe extern "thiscall" fn f_move_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { F_MOVE_F.hooked()(this, call, unit, true) })
    }

    #[detour(F_MOVE_WITH_SOUND_F)]
    unsafe extern "thiscall" fn f_move_with_sound_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
        u32::from(unsafe { f_move_with_sound_f(this as u32, call as u32, unit as u32, &MOVE_VANILLA) })
    }

    #[detour(F_MOVE_SLOW_F)]
    unsafe extern "thiscall" fn f_move_slow_f_detour(this: *const u32, call: i32, unit: *const i32) -> u32 {
        u32::from(unsafe { f_move_speed_f(this as u32, call as u32, unit as u32, MoveSpeed::Slow, &MOVE_SPEED_VANILLA) })
    }

    #[detour(F_MOVE_MEDIUM_F)]
    unsafe extern "thiscall" fn f_move_medium_f_detour(this: *const u32, call: i32, unit: *const i32) -> u32 {
        u32::from(unsafe { f_move_speed_f(this as u32, call as u32, unit as u32, MoveSpeed::Medium, &MOVE_SPEED_VANILLA) })
    }

    #[detour(F_MOVE_FAST_F)]
    unsafe extern "thiscall" fn f_move_fast_f_detour(this: *const u32, call: i32, unit: *const i32) -> u32 {
        u32::from(unsafe { f_move_speed_f(this as u32, call as u32, unit as u32, MoveSpeed::Fast, &MOVE_SPEED_VANILLA) })
    }

    #[detour(F_TARGET_BUILDING)]
    unsafe extern "thiscall" fn f_target_building_detour(this: *const u32, unit: *const u32, done: *const u8) -> bool {
        unsafe { f_target_building(this as u32, unit as u32, done as *const bool, &TARGET_BUILDING_VANILLA) }
    }

    #[detour(F_TARGET_BUILDING_F)]
    unsafe extern "thiscall" fn f_target_building_f_detour(this: *const u32, call: i32, unit: *const i32) -> bool {
        unsafe { f_target_building_f(this as u32, call as u32, unit as u32, &TARGET_BUILDING_VANILLA) }
    }

    #[detour(F_FACE_DIRECTION_F)]
    unsafe extern "stdcall" fn f_face_direction_f_detour(call: *const u32, _unit: *const u32) -> bool {
        unsafe { f_face_direction_f(call as u32) }
    }

    #[detour(F_MOVE_ENTITY_DELTA_F)]
    unsafe extern "thiscall" fn f_move_entity_delta_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
        match unsafe { f_move_entity_delta_f(this as u32, call as u32, unit as u32, clear_mixer_vanilla, move_entity_delta_vanilla) } {
            Some(result) => result as u32,
            None => unsafe { F_MOVE_ENTITY_DELTA_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_MOVE_ENTITY_SPECIAL_F)]
    unsafe extern "thiscall" fn f_move_entity_special_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
        match unsafe { f_move_entity_special_f(this as u32, call as u32, unit as u32, move_entity_delta_vanilla) } {
            Some(result) => result as u32,
            None => unsafe { F_MOVE_ENTITY_SPECIAL_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_PLAY_SET_F)]
    unsafe extern "stdcall" fn f_play_set_f_detour(call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { f_play_set_f(call as u32, unit as u32, play_set_vanilla) })
    }

    #[detour(F_PLAY_REVERSE_F)]
    unsafe extern "thiscall" fn f_play_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_PLAY_F.hooked()(this, call, unit, 1, 0) }
    }

    #[detour(F_PLAY_PING_PONG_F)]
    unsafe extern "thiscall" fn f_play_ping_pong_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_PLAY_F.hooked()(this, call, unit, 0, 1) }
    }

    #[detour(F_PLAY_STOP_AT_END_F)]
    unsafe extern "thiscall" fn f_play_stop_at_end_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        if !unsafe { f_play_stop_at_end_prepare(unit as u32) } {
            return unsafe { F_PLAY_STOP_AT_END_F_DETOUR.call(this, call, unit) };
        }
        unsafe { F_PLAY_F.hooked()(this, call, unit, 0, 0) }
    }

    #[detour(F_FLY_REVERSE_F)]
    unsafe extern "thiscall" fn f_fly_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_FLY_F.hooked()(this, call, unit, 1) }
    }

    #[detour(F_PLAY_WITH_SOUND_F)]
    unsafe extern "stdcall" fn f_play_with_sound_f_detour(call: *const u32, unit: *const u32) -> bool {
        let call = call as u32;
        unsafe {
            F_PLAY_WITH_SOUND.hooked()(
                call_params_begin(call) as *const c_void,
                call_params_end(call).wrapping_sub(CALL_PARAM_SIZE) as *const c_void,
                unit,
                (call + CALL_DONE_FLAG_OFFSET) as *const i8,
            )
        }
    }

    #[detour(F_PLAY_SOUND_F)]
    unsafe extern "thiscall" fn f_play_sound_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        let call = call as u32;
        if unsafe { call_param_count(call) } != 1 {
            return false;
        }
        let result = unsafe { F_PLAY_SOUND.hooked()(this, call_params_begin(call) as *const i8, unit as *const i32, (call + CALL_DONE_FLAG_OFFSET) as *const u8) };
        low_byte_bool(result)
    }

    #[detour(F_PLAY_TIME_F)]
    unsafe extern "thiscall" fn f_play_time_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { f_play_time_f(this as u32, call as u32, unit as u32, play_time_vanilla) })
    }

    #[detour(F_FLY)]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "thiscall" fn f_fly_detour(
        _this: *const u32, unit: *const u32, name: *const c_void, value: i32, extra: *const c_void, done: *const i8, reverse: i32, limit: i32,
    ) -> bool {
        let args = FFlyArgs { unit: unit as u32, name: name as u32, value: value as u32, extra: extra as u32, done: done as *const u8, reverse: reverse as u32, limit: limit as u32 };
        unsafe { f_fly(&args, &FLY_CORE_VANILLA) }
    }

    #[detour(F_PLAY_WITH_SOUND)]
    unsafe extern "stdcall" fn f_play_with_sound_detour(anim_name: *const c_void, sound_name: *const c_void, unit: *const u32, done: *const i8) -> bool {
        unsafe { f_play_with_sound(anim_name as u32, sound_name as u32, unit as u32, done as *const u8, &PLAY_WITH_SOUND_VANILLA) }
    }

    #[detour(F_PLAY_TIME)]
    unsafe extern "thiscall" fn f_play_time_detour(_this: *const u32, name: *const c_void, millis: i32, unit: *const u32, done: *const i8, _flag_b: i8) -> bool {
        unsafe { f_play_time(name as u32, millis as u32, unit as u32, done as *const u8, &PLAY_TIME_VANILLA_OPS) }
    }

    #[detour(F_PLAY_SET)]
    unsafe extern "stdcall" fn f_play_set_detour(name: *const i32, unit: *const c_void, child: i8, flag: u32) -> bool {
        unsafe { f_play_set(name as u32, unit as u32, child != 0, flag, &PLAY_SET_VANILLA) }
    }

    #[detour(F_PLAY_SET_PROB)]
    unsafe extern "stdcall" fn f_play_set_prob_detour(entries: *const i32, unit: *const std::ffi::c_void) -> u32 {
        u32::from(unsafe { f_play_set_prob(entries as u32, unit as u32, &SET_PROB_PICK_VANILLA) })
    }

    #[detour(F_PLAY_SOUND)]
    unsafe extern "thiscall" fn f_play_sound_detour(this: *const u32, sound: *const i8, unit: *const i32, done: *const u8) -> u32 {
        let mode_a = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_PLAY_MODE_A_STRING);
        u32::from(unsafe { f_play_sound(this as u32, sound as u32, unit as u32, done, mode_a, play_special_f_forward) })
    }

    #[detour(F_PLAY_SPECIAL_F)]
    unsafe extern "thiscall" fn f_play_special_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        match unsafe { f_play_special_f(this as u32, call as u32, unit as u32, &PLAY_SPECIAL_F_VANILLA) } {
            Some(result) => result,
            None => low_byte_bool(unsafe { F_PLAY_SPECIAL_F_DETOUR.call(this, call, unit) } as u32),
        }
    }

    #[detour(EXECUTE_CALL)]
    unsafe extern "thiscall" fn execute_call_detour(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> bool {
        unsafe { execute_call(this as u32, call as u32, unit as u32, done as *mut u8) != 0 }
    }

    #[detour(BFAIMGR_EXECUTE_CALL)]
    unsafe extern "thiscall" fn bfaimgr_execute_call_detour(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> u32 {
        unsafe { execute_base_call(this as u32, call as u32, unit as u32, done as *mut u8) as u32 }
    }

    /// `fSprayF` only forwards to the `return 1` stub `BFAIMgr::fFaceDirection`, so it returns 1.
    #[detour(F_SPRAY_F)]
    unsafe extern "stdcall" fn f_spray_f_detour(_call: i32, _unit: *const u32) -> bool {
        true
    }

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

    #[detour(BFAIMGR_INIT)]
    unsafe extern "thiscall" fn bfaimgr_init_detour(this: *const u32) -> bool {
        unsafe { bfaimgr_init(this as u32) }
    }

    #[detour(BFAIMGR_INIT_AIPARAMS)]
    unsafe extern "thiscall" fn bfaimgr_init_ai_params_detour(this: *const u32) -> u32 {
        unsafe { bfaimgr_init_ai_params(this as u32) }
    }

    #[detour(BFAIMGR_CONSTRUCTOR)]
    unsafe extern "thiscall" fn bfaimgr_constructor_detour(this: *const u32) -> *const u32 {
        unsafe { bfaimgr_construct(this as u32) as *const u32 }
    }

    #[detour(BFAIMGR_DESTRUCTOR_0)]
    unsafe extern "thiscall" fn bfaimgr_destructor_0_detour(this: *const u32) -> *const u32 {
        unsafe { bfaimgr_destruct(this as u32) as *const u32 }
    }

    #[detour(BFAIMGR_DESTRUCTOR_1)]
    unsafe extern "thiscall" fn bfaimgr_destructor_1_detour(this: *const u32, flags: u8) -> *const u32 {
        unsafe { bfaimgr_deleting_destruct(this as u32, flags) as *const u32 }
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

        pub(crate) fn execute_call(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> bool {
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

        pub(crate) fn bfaimgr_init(this: *const u32) -> bool {
            unsafe { super::BFAIMGR_INIT_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_init_ai_params(this: *const u32) -> u32 {
            unsafe { super::BFAIMGR_INIT_AIPARAMS_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_construct(this: *const u32) -> *const u32 {
            unsafe { super::BFAIMGR_CONSTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_destruct(this: *const u32) -> *const u32 {
            unsafe { super::BFAIMGR_DESTRUCTOR_0_DETOUR.call(this) }
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

        pub(crate) fn f_move_entity_special_f(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
            unsafe { super::F_MOVE_ENTITY_SPECIAL_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_keeper_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_KEEPER_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_random_walk(this: *const u32, unit: *const i32) -> u32 {
            unsafe { super::F_RANDOM_WALK_DETOUR.call(this, unit) }
        }

        pub(crate) fn f_play_set_prob(entries: *const i32, unit: *const c_void) -> u32 {
            unsafe { super::F_PLAY_SET_PROB_DETOUR.call(entries, unit) }
        }

        pub(crate) fn f_move_f(this: *const u32, call: *const u32, unit: *const u32, reverse: bool) -> u32 {
            unsafe { super::F_MOVE_F_DETOUR.call(this, call, unit, reverse) }
        }

        pub(crate) fn f_move_with_sound_f(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
            unsafe { super::F_MOVE_WITH_SOUND_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_move_slow_f(this: *const u32, call: i32, unit: *const i32) -> u32 {
            unsafe { super::F_MOVE_SLOW_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_move_medium_f(this: *const u32, call: i32, unit: *const i32) -> u32 {
            unsafe { super::F_MOVE_MEDIUM_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_move_fast_f(this: *const u32, call: i32, unit: *const i32) -> u32 {
            unsafe { super::F_MOVE_FAST_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_target_building(this: *const u32, unit: *const u32, done: *const u8) -> bool {
            unsafe { super::F_TARGET_BUILDING_DETOUR.call(this, unit, done) }
        }

        pub(crate) fn f_target_building_f(this: *const u32, call: i32, unit: *const i32) -> bool {
            unsafe { super::F_TARGET_BUILDING_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_set_base_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_SET_BASE_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_set(name: *const i32, unit: *const c_void, child: i8, flag: u32) -> bool {
            unsafe { super::F_PLAY_SET_DETOUR.call(name, unit, child, flag) }
        }

        pub(crate) fn f_play_special_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_SPECIAL_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_special(this: *const u32, anim: *const i8, sound: *const i8, seconds: i32, count: i32, special: *const i8, unit: *const i32, done: *const u8) -> bool {
            unsafe { super::F_PLAY_SPECIAL_DETOUR.call(this, anim, sound, seconds, count, special, unit, done) }
        }

        pub(crate) fn f_play_f(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
            unsafe { super::F_PLAY_F_DETOUR.call(this, call, unit, reverse, ping_pong) }
        }

        pub(crate) fn f_play_with_base_f(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
            unsafe { super::F_PLAY_WITH_BASE_F_DETOUR.call(this, call, unit, reverse, ping_pong) }
        }

        pub(crate) fn f_play_time(this: *const u32, name: *const c_void, millis: i32, unit: *const u32, done: *const i8, flag_b: i8) -> bool {
            unsafe { super::F_PLAY_TIME_DETOUR.call(this, name, millis, unit, done, flag_b) }
        }

        pub(crate) fn f_play_with_sound(anim_name: *const c_void, sound_name: *const c_void, unit: *const u32, done: *const i8) -> bool {
            unsafe { super::F_PLAY_WITH_SOUND_DETOUR.call(anim_name, sound_name, unit, done) }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn f_fly(this: *const u32, unit: *const u32, name: *const c_void, value: i32, extra: *const c_void, done: *const i8, reverse: i32, limit: i32) -> bool {
            unsafe { super::F_FLY_DETOUR.call(this, unit, name, value, extra, done, reverse, limit) }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn f_move(this: *const u32, first_param: i32, seconds: i32, unit: *const i32, done: *const u8, name: i32, speed: u32, extra: *const i8, reverse: u32, limit: u32) -> bool {
            unsafe { super::F_MOVE_DETOUR.call(this, first_param, seconds, unit, done, name, speed, extra, reverse, limit) }
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
        bfaimgr_remove_entities as real_bfaimgr_remove_entities, ztaimgr_init_ai_params as real_ztaimgr_init_ai_params, bfaimgr_load_data as real_bfaimgr_load_data, bfaimgr_save as real_bfaimgr_save, bfaimgr_update as real_bfaimgr_update, ztaimgr_create as real_ztaimgr_create, bfaimgr_construct as real_bfaimgr_construct, bfaimgr_destruct as real_bfaimgr_destruct,
        ztaimgr_update as real_ztaimgr_update,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        base_execute_call as real_base_execute_call, execute_call as real_execute_call, f_caught_prey as real_f_caught_prey, f_die as real_f_die, f_face_toward_food as real_f_face_toward_food, f_face_toward_target as real_f_face_toward_target, f_exit_building as real_f_exit_building, f_kill_prey as real_f_kill_prey,
    };
    pub(crate) use ztaimgr_detours::test_real::{bfaimgr_init as real_bfaimgr_init, bfaimgr_init_ai_params as real_bfaimgr_init_ai_params};
    pub(crate) use ztaimgr_detours::test_real::{f_play_set_prob as real_f_play_set_prob, f_random_walk as real_f_random_walk};
    pub(crate) use ztaimgr_detours::test_real::{
        f_move_f as real_f_move_f, f_move_fast_f as real_f_move_fast_f, f_move_medium_f as real_f_move_medium_f, f_move_slow_f as real_f_move_slow_f, f_move_with_sound_f as real_f_move_with_sound_f, f_play_set as real_f_play_set, f_play_special_f as real_f_play_special_f,
        f_set_base_f as real_f_set_base_f, f_target_building as real_f_target_building, f_target_building_f as real_f_target_building_f,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        f_fly as real_f_fly, f_move as real_f_move, f_play_f as real_f_play_f, f_play_special as real_f_play_special, f_play_time as real_f_play_time, f_play_with_base_f as real_f_play_with_base_f, f_play_with_sound as real_f_play_with_sound,
    };
    pub(crate) use super::{f_move, FMoveArgs, F_MOVE_VANILLA_OPS};
    pub(crate) use super::{acquire_first_vanilla, add_anim_vanilla, play_links_with, play_pair_with, play_single_with, play_timed_with, play_two_with, play_with, release_first_vanilla, set_base_with};
    pub(crate) use ztaimgr_detours::test_real::{
        f_end_trick as real_f_end_trick, f_enter_building as real_f_enter_building, f_guest_routine as real_f_guest_routine, f_guide_routine as real_f_guide_routine,
        f_keeper_routine as real_f_keeper_routine, f_maint_routine as real_f_maint_routine, f_return_to_building as real_f_return_to_building,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        f_face_direction_f as real_f_face_direction_f, f_fly_reverse_f as real_f_fly_reverse_f, f_move_entity_special_f as real_f_move_entity_special_f, f_play_ping_pong_f as real_f_play_ping_pong_f, f_play_reverse_f as real_f_play_reverse_f,
        f_play_set_f as real_f_play_set_f, f_play_sound_f as real_f_play_sound_f, f_play_stop_at_end_f as real_f_play_stop_at_end_f, f_play_time_f as real_f_play_time_f,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_move_delta_rotates_by_direction_and_recenters() {
        assert_eq!(special_move_delta([3, 5, 9], 1, 0, 0), [3 + 0x20, 5 + 0x20, 9]);
        assert_eq!(special_move_delta([3, 5, 9], 2, 0, 0), [5 + 0x20, -3 + 0x20, 9]);
        assert_eq!(special_move_delta([3, 5, 9], 6, 0, 0), [-5 + 0x20, 3 + 0x20, 9]);
        assert_eq!(special_move_delta([3, 5, 9], 0, 0, 0), [-3 + 0x20, -5 + 0x20, 9]);
        // C remainder: negative positions keep their sign, and only the low six bits of magnitude count.
        assert_eq!(special_move_delta([0, 0, 0], 1, 70, -70), [-6 + 0x20, 6 + 0x20, 0]);
        assert_eq!(special_move_delta([0, 0, 0], 1, -64, 64), [0x20, 0x20, 0]);
    }

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
        RawCallString { begin: range.start, end: range.end, capacity: range.end, _live: LiveMemory::default() }
    }

    fn call_with(name: &'static [u8], params: &[RawCallString]) -> RawFunctionCall {
        let range = params.as_ptr_range();
        RawFunctionCall { name: string_of(name), params_begin: range.start, params_end: range.end, params_capacity: range.end, _live: LiveMemory::default() }
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

    /// The game's `rand()`: an LCG over one dword (`state * 0x343fd + 0x269ec3`, result bits 16..30).
    struct GameRand(u32);

    impl GameRand {
        fn next(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            (self.0 >> 16) & 0x7fff
        }
    }

    #[test]
    fn game_rand_matches_the_documented_sequence() {
        let mut rand = GameRand(1);
        assert_eq!(rand.next(), 41);
        assert_eq!(rand.next(), 18467);
        assert_eq!(rand.next(), 6334);
    }

    use LinkKind::{Main, Tail};

    fn timed(total: i32, duration: i32, ping_pong: bool, stop: bool) -> Vec<LinkKind> {
        timed_repeat(total, duration, ping_pong, stop).take(1000).collect()
    }

    #[test]
    fn timed_repeat_fills_the_duration() {
        // An exact multiple: 300 ms of 100 ms links is the first link plus two more.
        assert_eq!(timed(300, 100, false, true), vec![Main, Main]);
        assert_eq!(timed(301, 100, false, true), vec![Main, Main, Main]);
        assert_eq!(timed(299, 100, false, true), vec![Main, Main]);
        // The duration is covered by the first link alone, or not at all.
        assert_eq!(timed(100, 100, false, true), vec![]);
        assert_eq!(timed(50, 100, false, true), vec![]);
        assert_eq!(timed(0, 100, false, true), vec![]);
        assert_eq!(timed(-5, 100, false, true), vec![]);
    }

    #[test]
    fn timed_repeat_with_zero_duration_differs_between_callers() {
        // `fMove` and `fPlayTime` stop after one extra link; `fPlaySpecial` never ends (bounded here).
        assert_eq!(timed(500, 0, false, true), vec![Main]);
        assert_eq!(timed(0, 0, false, true), vec![]);
        assert_eq!(timed(-1, 0, false, true), vec![]);
        assert_eq!(timed(500, 0, false, false).len(), 1000);
        assert_eq!(timed(0, 0, false, false), vec![]);
        assert_eq!(timed(500, 0, true, false).len(), 1000);
    }

    #[test]
    fn timed_repeat_ping_pong_subtracts_twice() {
        // 500 ms of 100 ms: the first main and tail are paid up front (500 - 200 = 300), then each pair costs 200.
        assert_eq!(timed(500, 100, true, false), vec![Main, Tail, Main, Tail]);
        assert_eq!(timed(300, 100, true, false), vec![Main, Tail]);
        assert_eq!(timed(200, 100, true, false), vec![]);
        assert_eq!(timed(150, 100, true, false), vec![]);
        // A pair is always appended whole, even when the remainder covers only its first half.
        assert_eq!(timed(301, 100, true, false), vec![Main, Tail]);
    }

    #[test]
    fn timed_repeat_wraps_like_the_32_bit_arithmetic() {
        // `total - duration` wraps to a positive remainder for the most negative total, so it keeps appending.
        assert_eq!(timed(i32::MIN, 100, false, true).len(), 1000);
        // A negative duration only grows the remainder; the sequence is unbounded (bounded here).
        assert_eq!(timed(5, -1, false, true).len(), 1000);
    }

    #[test]
    fn counted_repeat_appends_count_minus_one() {
        let counted = |count, ping_pong| counted_repeat(count, ping_pong).collect::<Vec<_>>();
        // `count - 1` wraps for the most negative count, which asks for 2^31 - 1 links (bounded here).
        assert_eq!(counted_repeat(i32::MIN, false).take(5).count(), 5);
        assert_eq!(counted(-3, false), vec![]);
        assert_eq!(counted(0, false), vec![]);
        assert_eq!(counted(1, false), vec![]);
        assert_eq!(counted(2, false), vec![Main]);
        assert_eq!(counted(4, false), vec![Main, Main, Main]);
        assert_eq!(counted(1, true), vec![]);
        assert_eq!(counted(3, true), vec![Main, Tail, Main, Tail]);
    }

    #[test]
    fn set_prob_picker_follows_the_scripted_rolls() {
        let pick = |weights: &[i32], rolls: &[u32]| {
            let mut rolls = rolls.iter().copied();
            let picked = pick_set_prob(weights, || rolls.next().expect("rand called more often than scripted"));
            (picked, rolls.count())
        };
        // A roll is `rand % 100 + 1`; 0 -> 1, 29 -> 30.
        assert_eq!(pick(&[30, 70], &[0, 9]), (Some(0), 1));
        assert_eq!(pick(&[30, 70], &[29, 9]), (Some(0), 1));
        // 40 -> roll 41 misses entry 0 (weight 30); the remainder 70 rerolls `5 % 70 + 1 = 6`.
        assert_eq!(pick(&[30, 70], &[40, 5]), (Some(1), 0));
        assert_eq!(pick(&[10, 10, 80], &[95, 89, 79]), (Some(2), 0));
        // An entry with weight 0 is passed over, and a single entry always hits.
        assert_eq!(pick(&[0, 100], &[0, 0]), (Some(1), 0));
        assert_eq!(pick(&[100], &[99]), (Some(0), 0));
        // Weights that do not sum to 100, and no entries, roll nothing.
        assert_eq!(pick(&[30, 60], &[]), (None, 0));
        assert_eq!(pick(&[], &[]), (None, 0));
        assert_eq!(pick(&[60, 60], &[]), (None, 0));
        // A negative weight can make the sum 100; the first entry then covers every roll.
        assert_eq!(pick(&[110, -10], &[50]), (Some(0), 0));
    }

    #[test]
    fn set_prob_picker_matches_the_weights_over_the_game_rand() {
        let weights = [20, 30, 50];
        let mut rand = GameRand(0x1234_5678);
        let mut counts = [0u32; 3];
        let draws = 20_000u32;
        for _ in 0..draws {
            let picked = pick_set_prob(&weights, || rand.next()).expect("weights sum to 100, so something is always picked");
            counts[picked] += 1;
        }
        for (count, weight) in counts.iter().zip(weights) {
            let expected = draws * weight as u32 / 100;
            assert!(count.abs_diff(expected) < draws / 25, "{counts:?} against {weights:?}");
        }
    }

    fn facts(f: impl FnOnce(&mut PlaySpecialFFacts)) -> PlaySpecialFFacts {
        let mut facts = PlaySpecialFFacts::default();
        f(&mut facts);
        facts
    }

    #[test]
    fn play_special_f_decisions() {
        use NoSoundPlay as N;
        use PlaySpecialFAction as A;
        let decide = decide_play_special_f;
        assert_eq!(decide(facts(|f| f.seconds = true)), A::PlaySpecial { use_seconds: true, use_repeat: false });
        assert_eq!(decide(facts(|f| (f.seconds, f.repeat) = (true, true))), A::Fail);
        assert_eq!(decide(facts(|f| f.repeat = true)), A::PlaySpecial { use_seconds: false, use_repeat: true });
        // A repeat count wins over every string predicate.
        assert_eq!(decide(facts(|f| (f.repeat, f.sound_none, f.special_reverse) = (true, true, true))), A::PlaySpecial { use_seconds: false, use_repeat: true });
        // A duration with the "none" sound and a plain special becomes `fPlayTime`; the second flag follows mode B.
        assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_mode_a) = (true, true, true))), A::PlayTime { second_flag: false });
        assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_mode_b) = (true, true, true))), A::PlayTime { second_flag: true });
        assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_none) = (true, true, true))), A::PlayTime { second_flag: false });
        // Not plain, or a named sound: `fPlaySpecial` with the duration.
        assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_reverse) = (true, true, true))), A::PlaySpecial { use_seconds: true, use_repeat: false });
        assert_eq!(decide(facts(|f| (f.seconds, f.special_mode_a) = (true, true))), A::PlaySpecial { use_seconds: true, use_repeat: false });
        // Neither number, the "none" sound.
        assert_eq!(decide(facts(|f| (f.sound_none, f.special_mode_a) = (true, true))), A::NoSound(N::Plain));
        assert_eq!(decide(facts(|f| (f.sound_none, f.special_none) = (true, true))), A::NoSound(N::Plain));
        assert_eq!(decide(facts(|f| (f.sound_none, f.special_reverse) = (true, true))), A::NoSound(N::Reverse));
        assert_eq!(decide(facts(|f| (f.sound_none, f.special_ping_pong) = (true, true))), A::NoSound(N::PingPong));
        assert_eq!(decide(facts(|f| f.sound_none = true)), A::NoSound(N::Other));
        // A plain mode wins over reverse and ping-pong when a string matches several.
        assert_eq!(decide(facts(|f| (f.sound_none, f.special_mode_a, f.special_reverse) = (true, true, true))), A::NoSound(N::Plain));
        // A named sound: with a named animation `fPlayWithSound`, with the "none" animation `fPlaySpecial`.
        assert_eq!(decide(facts(|_| {})), A::PlayWithSound);
        assert_eq!(decide(facts(|f| f.anim_none = true)), A::PlaySpecialNoAnim { second_flag: false });
        assert_eq!(decide(facts(|f| (f.anim_none, f.special_mode_b) = (true, true))), A::PlaySpecialNoAnim { second_flag: true });
    }

    #[test]
    fn play_special_f_decision_invariants_over_every_combination() {
        for bits in 0u32..512 {
            let f = PlaySpecialFFacts {
                seconds: bits & 1 != 0,
                repeat: bits & 2 != 0,
                sound_none: bits & 4 != 0,
                anim_none: bits & 8 != 0,
                special_mode_a: bits & 16 != 0,
                special_mode_b: bits & 32 != 0,
                special_none: bits & 64 != 0,
                special_reverse: bits & 128 != 0,
                special_ping_pong: bits & 256 != 0,
            };
            let action = decide_play_special_f(f);
            assert_eq!(action == PlaySpecialFAction::Fail, f.seconds && f.repeat, "{f:?}");
            if f.repeat && !f.seconds {
                assert_eq!(action, PlaySpecialFAction::PlaySpecial { use_seconds: false, use_repeat: true }, "{f:?}");
            }
            if !f.seconds && !f.repeat && f.sound_none {
                assert!(matches!(action, PlaySpecialFAction::NoSound(_)), "{f:?}");
            }
            if !f.seconds && !f.repeat && !f.sound_none {
                assert_eq!(matches!(action, PlaySpecialFAction::PlayWithSound), !f.anim_none, "{f:?}");
            }
            if let PlaySpecialFAction::PlayTime { second_flag } = action {
                assert!(f.seconds && !f.repeat && f.sound_none && (f.special_mode_a || f.special_mode_b || f.special_none), "{f:?}");
                assert_eq!(second_flag, f.special_mode_b);
            }
        }
    }

    #[test]
    fn random_walk_border_is_walked_in_column_order() {
        let border = |hx, hy| border_offsets(hx, hy).collect::<Vec<_>>();
        assert_eq!(border(1, 1), vec![(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]);
        assert_eq!(border(0, 0), vec![(0, 0)]);
        assert_eq!(border(2, 1), vec![(-2, -1), (-2, 0), (-2, 1), (-1, -1), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 0), (2, 1)]);
        for (hx, hy) in [(1, 1), (2, 3), (5, 2), (4, 4), (1, 6)] {
            let offsets = border(hx, hy);
            let interior = ((2 * hx - 1).max(0) * (2 * hy - 1).max(0)) as usize;
            assert_eq!(offsets.len(), ((2 * hx + 1) * (2 * hy + 1)) as usize - interior, "({hx},{hy})");
            assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]), "({hx},{hy}) is dx-major, dy ascending");
            assert!(offsets.iter().all(|&(dx, dy)| dx.abs() == hx || dy.abs() == hy));
        }
    }

    #[test]
    fn random_walk_neighbour_scan_starts_at_the_roll_and_does_not_wrap() {
        assert_eq!(neighbor_scan(0, 4).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_eq!(neighbor_scan(2, 4).collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(neighbor_scan(3, 4).collect::<Vec<_>>(), vec![3]);
        assert_eq!(neighbor_scan(0, 1).collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn route_tables_are_total_and_distinct() {
        let zt_names: [&[u8]; 24] = [
            b"fWalk", b"fSwim", b"fTrot", b"fRun", b"fSpray", b"fKeeperRoutine", b"fGuestRoutine", b"fGuideRoutine", b"fMaintRoutine", b"fFaceTowardFood", b"fFaceTowardTarget", b"fShowPredator", b"fHidePredator", b"fEnterBuilding", b"fExitBuilding", b"fDustBall",
            b"fKillPrey", b"fCaughtPrey", b"fDie", b"fTargetBuilding", b"fHatch", b"fEndTrick", b"fReturnToBuilding", b"fPlaySetTank",
        ];
        let mut zt_handlers: Vec<Handler> = zt_names.iter().map(|name| zt_handler(lookup_zt_call(name).unwrap())).collect();
        zt_handlers.sort_by_key(|h| *h as u32);
        zt_handlers.dedup();
        // `fWalk`/`fSwim` share a handler; every other name has its own.
        assert_eq!(zt_handlers.len(), 23);

        let base_names: [&[u8]; 28] = [
            b"fPlay", b"fSetBase", b"fPlayWithBase", b"fMoveEntityDelta", b"fMoveEntitySpecial", b"fPlayTime", b"fPlaySound", b"fPlaySpecial", b"fPlaySp", b"fPlaySetProb", b"fPlaySetTerrain", b"fPlayReverse", b"fPlayPingPong", b"fPingPong", b"fPlayWithSound", b"fPlaySFX",
            b"fPlayStopAtEnd", b"fPlaySet", b"fMoveSlow", b"fMoveMedium", b"fMove", b"fMoveReverse", b"fMoveFast", b"fMoveWithSound", b"fFaceDirection", b"fFly", b"fFlyReverse", b"fKeepControl",
        ];
        let mut routes = Vec::new();
        let mut unrouted = Vec::new();
        for name in base_names.iter().chain([&b"fHalt"[..]].iter()) {
            let which = lookup_base_call(name).unwrap();
            match base_route(which) {
                Some(route) => routes.push(route.handler),
                None => unrouted.push(which),
            }
        }
        assert_eq!(unrouted, vec![BaseCall::PlaySetTerrain, BaseCall::KeepControl]);
        routes.sort_by_key(|h| *h as u32);
        routes.dedup();
        // Three aliases share handlers (`fPlaySp`, `fPingPong`, `fPlaySFX`).
        assert_eq!(routes.len(), 29 - 2 - 3);
        // The trailing constants the vanilla chain passes (`fPlayF` `0, 0`, `fMoveF` `false`, `fFlyF` `0`) are all zero.
        assert!(base_names.iter().all(|name| base_route(lookup_base_call(name).unwrap()).map_or(true, |r| r.trailing == [0, 0])));
    }

    #[test]
    fn bf_ai_params_use_defaults_keys_and_clamps() {
        let mut keys = Vec::new();
        let defaults = compute_bf_ai_params(|key, default| {
            keys.push((key.to_string(), default));
            default
        });
        let expected: Vec<(&str, i32)> = vec![
            ("BaseX", 1), ("BaseY", 1), ("RandomX", 5), ("RandomY", 5), ("mAIHowOften", 1), ("mAITick", 1000), ("mStatusPeriod", 1000), ("mGoalPeriod", 0), ("ShowAIInfo", 0), ("ShowName", 1), ("ShowPosition", 1), ("ShowStatusVars", 1), ("ShowFunctionCall", 1),
            ("ShowEvents", 1), ("AIInfoNth", 100), ("ShowSelected", 0), ("ShowFrame", 0), ("ShowGoal", 0), ("VariableUnitSpeeds", 1), ("AllowDiagonalMoves", 1), ("ClearUnreachableFreq", 5), ("MinUnreachableCount", 5),
        ];
        assert_eq!(keys, expected.iter().map(|(key, default)| (key.to_string(), *default)).collect::<Vec<_>>());
        assert_eq!(
            defaults,
            BfAiParams {
                base_x: 1, base_y: 1, random_x: 5, random_y: 5, how_often: 1, tick: 1000, status_period: 1000, goal_period: 0, show_ai_info: 0, show_name: 1, show_position: 1, show_status_vars: 1, show_function_call: 1, show_events: 1, ai_info_nth: 100,
                show_selected: 0, show_frame: 0, show_goal: 0, variable_unit_speeds: 1, allow_diagonal_moves: true, clear_unreachable_freq: 5, min_unreachable_count: 5,
            }
        );
        // The base extents clamp up to 0, the random extents up to 1; positive values pass through.
        let clamped = compute_bf_ai_params(|_, _| -7);
        assert_eq!((clamped.base_x, clamped.base_y, clamped.random_x, clamped.random_y), (0, 0, 1, 1));
        let zero = compute_bf_ai_params(|_, _| 0);
        assert_eq!((zero.base_x, zero.random_x, zero.allow_diagonal_moves), (0, 1, false));
        let big = compute_bf_ai_params(|_, _| 300);
        assert_eq!((big.base_x, big.random_y, big.tick, big.clear_unreachable_freq), (300, 300, 300, 300));
        // Unclamped fields keep negative values, `AllowDiagonalMoves` is true only for exactly 1, and the
        // unreachable count keeps its low byte.
        assert_eq!((clamped.tick, clamped.ai_info_nth, clamped.clear_unreachable_freq), (-7, -7, -7));
        assert_eq!(compute_bf_ai_params(|key, d| if key == "AllowDiagonalMoves" { 2 } else { d }).allow_diagonal_moves, false);
        assert_eq!(compute_bf_ai_params(|key, d| if key == "MinUnreachableCount" { 0x1ff } else { d }).min_unreachable_count, 0xff);
        let by_key = compute_bf_ai_params(|key, d| match key {
            "RandomY" => 8,
            "mGoalPeriod" => 42,
            _ => d,
        });
        assert_eq!((by_key.random_x, by_key.random_y, by_key.goal_period, by_key.status_period), (5, 8, 42, 1000));
    }
}
