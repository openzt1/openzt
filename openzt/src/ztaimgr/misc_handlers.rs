//! `fPlaySetTank`, `fDustBall`, `fTargetBuilding` and the speed-selecting move wrappers.

use super::*;

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
pub(super) unsafe fn play_set_vanilla(name: u32, unit: u32) -> u32 {
    u32::from(unsafe { F_PLAY_SET.hooked()(name as *const i32, unit as *const c_void, 1, 1) })
}

pub(super) unsafe fn type_find_anim_vanilla(entity_type: u32, name: *const u8) -> bool {
    !unsafe { BFENTITYTYPE_FIND_ANIM.original()(entity_type as *const u32, name.cast()) }.is_null()
}

pub(super) unsafe fn type_find_sound_vanilla(entity_type: u32, name: *const u8) -> bool {
    !unsafe { BFENTITYTYPE_FIND_SOUND.original()(entity_type as *const u32, name.cast()) }.is_null()
}

/// The string vanilla stores for "no animation / no sound" (`DAT_0063ba4c`).
pub(super) const NO_NAME: &[u8] = b"0";
/// `ZTUnit` selector `fDustBall` reads (`3` surface ball, `4` submerged ball).
pub(super) const UNIT_BALL_SELECTOR_OFFSET: u32 = 0x258;

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
    let anim = OwnedString::from_bytes(pick(ops.find_anim));
    let sound = OwnedString::from_bytes(pick(ops.find_sound));
    let extra = OwnedString::from_bytes(NO_NAME);
    let result = unsafe { (ops.play_special)(this, anim.as_ptr() as u32, sound.as_ptr() as u32, extra.as_ptr() as u32, unit, done as u32) };
    drop(extra);
    if unsafe { done.cast::<u8>().read_volatile() } != 0 {
        unsafe { (ops.caught_prey)(unit, done) };
    }
    Some(result)
}

pub(super) unsafe fn play_special_vanilla(this: u32, anim: u32, sound: u32, extra: u32, unit: u32, done: u32) -> bool {
    unsafe { F_PLAY_SPECIAL.hooked()(this as *const u32, anim as *const i8, sound as *const i8, 0, 1, extra as *const i8, unit as *const i32, done as *const u8) }
}

pub(super) unsafe fn caught_prey_hooked(unit: u32, done: *const bool) {
    unsafe { F_CAUGHT_PREY.hooked()(unit as *const i32, done as *const u8) };
}

pub(super) const DUST_BALL_VANILLA: DustBallOps = DustBallOps {
    tile_flags: unit_tile_flags_vanilla,
    find_anim: type_find_anim_vanilla,
    find_sound: type_find_sound_vanilla,
    play_special: play_special_vanilla,
    caught_prey: caught_prey_hooked,
};

/// `ZTAnimal` byte `fTargetBuildingF` tests before calling `fTargetBuilding`.
pub(super) const ANIMAL_TARGETING_FLAG_OFFSET: u32 = 0x3b0;
/// `ZTAnimal` byte recording that a building target was acquired.
pub(super) const ANIMAL_TARGET_ACQUIRED_OFFSET: u32 = 0x3b1;
/// `ZTUnitType` dword passed to `fMove` as its speed argument.
pub(super) const UNIT_TYPE_MOVE_SPEED_OFFSET: u32 = 0x108;
/// Entity vtable slot returning the animal's target-building name (a `{begin, end}` string).
pub(super) const ENTITY_VTABLE_TARGET_BUILDING_NAME: u32 = 0x128;
/// `fMove`'s trailing constants for a building walk.
pub(super) const TARGET_BUILDING_MOVE_FLAG: u32 = 0;
pub(super) const TARGET_BUILDING_MOVE_LIMIT: u32 = 100;

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
    let none = OwnedString::from_bytes(NO_NAME);
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
pub(super) const ENTITY_VTABLE_MOVE_NAME_SLOW: u32 = 0x120;
/// Entity vtable slot of the slow variant's move limit; medium/fast follow at `+4`/`+8`.
pub(super) const ENTITY_VTABLE_MOVE_LIMIT_SLOW: u32 = 0x140;
/// `ZTUnitType` speed dword of the slow variant; medium/fast follow at `+4`/`+8`.
pub(super) const UNIT_TYPE_SPEED_SLOW_OFFSET: u32 = 0x100;

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
    let seconds = unsafe { call_virtual_args(this, AI_VTABLE_CONVERT_CONSTANT_PARAM, [begin + CALL_PARAM_SIZE, unit]) };
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

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn f_move_vanilla_speed(this: u32, first: u32, seconds: u32, unit: u32, done: u32, name: u32, speed: u32, empty: u32, limit: u32) -> bool {
    unsafe { F_MOVE.hooked()(this as *const u32, first as i32, seconds as i32, unit as *const i32, done as *const u8, name as i32, speed, empty as *const i8, 0, limit) }
}

pub(super) const MOVE_SPEED_VANILLA: MoveSpeedOps = MoveSpeedOps { f_move: f_move_vanilla_speed };

/// The vanilla call `fMoveF`/`fMoveWithSoundF` make, so a test can substitute a recorder.
/// `BFAIMgr::fMove(this, first_param, a2, unit, done, name, speed, extra, reverse, limit)`; `name` and `extra`
/// are vanilla strings.
/// `fMove`'s ten dword parameters.
pub type FMoveFn = unsafe fn(u32, u32, u32, u32, u32, u32, u32, u32, u32, u32) -> bool;

pub struct MoveOps {
    pub f_move: FMoveFn,
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn f_move_vanilla_full(this: u32, first: u32, a2: u32, unit: u32, done: u32, name: u32, speed: u32, extra: u32, reverse: u32, limit: u32) -> bool {
    unsafe { F_MOVE.hooked()(this as *const u32, first as i32, a2 as i32, unit as *const i32, done as *const u8, name as i32, speed, extra as *const i8, reverse, limit) }
}

pub(super) const MOVE_VANILLA: MoveOps = MoveOps { f_move: f_move_vanilla_full };

/// `fMove` limit when the call carries no limit parameter.
pub(super) const DEFAULT_MOVE_LIMIT: u32 = 100;
/// Parameter-entry offsets read by `fMoveF`/`fMoveWithSoundF`.
pub(super) const PARAM_1: u32 = CALL_PARAM_SIZE;
pub(super) const PARAM_2: u32 = CALL_PARAM_SIZE * 2;
pub(super) const PARAM_3: u32 = CALL_PARAM_SIZE * 3;
pub(super) const PARAM_4: u32 = CALL_PARAM_SIZE * 4;
pub(super) const PARAM_5: u32 = CALL_PARAM_SIZE * 5;

pub(super) unsafe fn convert_param(this: u32, param: u32, unit: u32) -> u32 {
    unsafe { call_virtual_args(this, AI_VTABLE_CONVERT_CONSTANT_PARAM, [param, unit]) }
}

pub(super) unsafe fn param_string(param: u32) -> OwnedString {
    OwnedString::from_bytes(unsafe { &*(param as *const RawCallString) }.bytes())
}

/// An empty `{begin, end, cap}` string over a static NUL byte. Consumers only read `begin`/`end` (and `begin`
/// as a C string), so it stands in for a vanilla empty string without a vanilla allocation.
pub(super) struct EmptyName([u32; 3]);

pub(super) static EMPTY_NAME_BYTE: u8 = 0;

impl EmptyName {
    pub(super) fn new() -> Self {
        let byte = &EMPTY_NAME_BYTE as *const u8 as u32;
        EmptyName([byte, byte, byte])
    }

    pub(super) fn as_ptr(&self) -> u32 {
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
