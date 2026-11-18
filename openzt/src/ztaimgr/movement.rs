//! `fMove` and the mixer and target-list pieces it leans on.

use super::*;

pub(super) unsafe fn link_duration_vanilla(link: u32) -> i32 {
    unsafe { LINK_DURATION.original()(link as *const u32, 0) }
}

pub(super) unsafe fn mixer_clear_all_vanilla(mixer: u32) {
    unsafe { GXMIXER_CLEAR_ALL.hooked()(mixer as *const u32) };
}

pub(super) unsafe fn mixer_set_set_vanilla(mixer: u32, set: u32, out: u8) {
    crate::gxmixer::set_set(mixer, set, out);
}

pub(super) unsafe fn mixer_set_base_vanilla(mixer: u32, vector: u32) {
    unsafe { GXMIXER_SET_BASE.hooked()(mixer as *const u32, vector as *const i32, 1) };
}

pub(crate) const VANILLA_MIXER: MixerOps =
    MixerOps { ai_play, duration: link_duration_vanilla, clear_all: mixer_clear_all_vanilla, set_set: mixer_set_set_vanilla, set_base: mixer_set_base_vanilla };

/// `BFAIMgr::fMove` (`0x0043646e`) unit fields. Flag bytes: `+0xf8` moving, `+0xf9` stopped/arrived,
/// `+0xd7`, `+0x20c`, `+0x20d` (finished at once), `+0x20e` (cog goal invalid). `+0x154` is the pointer to the
/// target list's sentinel node, `+0x15c` the embedded `BFCogGoal`, `+0x170`/`+0x1bc` state words, `+0x74` the
/// `GXMixer` and `+0x8c`/`+0x9c` a pair of values compared before restarting a move.
pub(super) const UNIT_MOVING: u32 = 0xf8;
pub(super) const UNIT_STOPPED: u32 = 0xf9;
pub(super) const UNIT_FLAG_D7: u32 = 0xd7;
pub(super) const UNIT_FLAG_20C: u32 = 0x20c;
pub(super) const UNIT_FINISHED: u32 = 0x20d;
pub(super) const UNIT_COG_INVALID: u32 = 0x20e;
pub(super) const UNIT_COG_GOAL: u32 = 0x15c;
pub(super) const UNIT_STATE: u32 = 0x170;
pub(super) const UNIT_STATE_2: u32 = 0x1bc;
pub(super) const UNIT_MIXER: u32 = 0x74;
pub(super) const UNIT_MIXER_FLAG: u32 = 0xd4;
pub(super) const UNIT_POS_A: u32 = 0x8c;
pub(super) const UNIT_POS_B: u32 = 0x9c;
pub(super) const COG_GOAL_FLAG_OFFSET: u32 = 0x4c;
/// Entity vtable slots `fMove` calls.
pub(super) const ENTITY_VT_HAS_TARGET: u32 = 0x180;
pub(super) const ENTITY_VT_CAN_PLAY: u32 = 0x184;
pub(super) const ENTITY_VT_PREPARE: u32 = 0x1b0;
pub(super) const ENTITY_VT_FIND_ANIM: u32 = 0x130;
pub(super) const ENTITY_VT_FIND_ANIM_SECOND: u32 = 0x134;
pub(super) const ENTITY_VT_SET_SPEED: u32 = 0x1c0;
pub(super) const ENTITY_VT_SET_LIMIT: u32 = 0x1c4;
/// `BFCogGoal` vtable slots: `valid`, `clearTarget`, `clearTargetSub`.
pub(super) const COG_VT_VALID: u32 = 0x0;
pub(super) const COG_VT_CLEAR_TARGET: u32 = 0xc;
pub(super) const COG_VT_CLEAR_TARGET_SUB: u32 = 0x1c;
/// `BFAIMgr::fPlay(this, second, first, unit, scratch, with_base)`. Vanilla writes `1` to `scratch` and
/// returns `true`; nothing reads the scratch byte, so it lives here.
pub(super) unsafe fn ai_play(this: u32, second: u32, first: u32, unit: u32, with_base: bool) -> bool {
    let mut scratch = false;
    unsafe { BFAIMGR_F_PLAY.original()(this as *const u32, second as *const c_void, first as *const c_void, unit as *const u32, &mut scratch, with_base) }
}
/// `GLOBAL_BFAnimCache` cell and the string cell `fMove` compares the extra string against.
pub(super) const RVA_ANIM_CACHE: u32 = 0x0023_8044;
pub(super) const RVA_NO_SOUND_STRING: u32 = 0x0023_950c;
pub(super) const ANIM_REFCOUNT_OFFSET: u32 = 0x20;
pub(super) const ANIM_STAMP_OFFSET: u32 = 0x1c;
pub(super) const ANIM_CACHE_STAMP_OFFSET: u32 = 0x20;
pub(super) const LINK_SIZE: u32 = 0xc;

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

pub(super) unsafe fn vcall0(object: u32, slot: u32) -> u32 {
    unsafe { call_virtual_args(object, slot, []) }
}

pub(super) unsafe fn vcall1(object: u32, slot: u32, arg: u32) -> u32 {
    unsafe { call_virtual_args(object, slot, [arg]) }
}

/// Vanilla ends `fMove` through the same four-write sequence in several places.
pub(super) unsafe fn finish_stopped(unit: u32, done: *const u8) {
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
        && TargetList::of_unit(unit).is_empty()
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

    if get_from_memory::<u8>(unit + UNIT_STOPPED) != 0 && !TargetList::of_unit(unit).is_empty() {
        if TargetList::of_unit(unit).len() <= 1 && get_from_memory::<u32>(unit + UNIT_STATE_2) != 3 {
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
        if TargetList::of_unit(unit).len() <= 1 {
            unsafe { vcall0(cog, COG_VT_CLEAR_TARGET) };
            unsafe { vcall0(cog, COG_VT_CLEAR_TARGET_SUB) };
            save_to_memory(unit + UNIT_STOPPED, 1u8);
            unsafe { (done as *mut u8).write_volatile(1) };
            save_to_memory(unit + UNIT_MOVING, 0u8);
            return true;
        }
    } else if TargetList::of_unit(unit).is_empty() || unsafe { (ops.reached_target)(cog) } {
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

pub(super) unsafe fn stop_vanilla(unit: u32, a: i8, b: i8) {
    unsafe { crate::bfunit::stop(unit, a as u8, b as u8) }
}

pub(super) unsafe fn set_moving_vanilla(unit: u32, a: i8, b: i8) {
    unsafe { crate::bfunit::set_moving(unit, a as u8, b as u8) }
}

pub(super) unsafe fn reached_target_vanilla(cog: u32) -> bool {
    unsafe { BFCOGGOAL_REACHED_TARGET.original()(cog as *const u32) }
}

/// Whether the string at `string` has exactly the bytes of vanilla's "no sound" string.
pub(super) unsafe fn is_no_sound_vanilla(string: u32) -> bool {
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

pub(super) unsafe fn find_sound_vanilla(unit: u32, string: u32) -> u32 {
    unsafe { BFENTITY_FIND_SOUND.hooked()(unit as *const u32, string as *const std::ffi::c_void) as u32 }
}
