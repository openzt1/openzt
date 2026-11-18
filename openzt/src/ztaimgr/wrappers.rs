//! The thin `...F` wrappers that convert parameters and forward: face direction, halt, move-entity, play set/stop-at-end/time/sound and `fPlaySpecialF`.

use super::*;

/// The call's parameter count, computed as vanilla does (signed byte span over the entry size).
pub(super) unsafe fn call_param_count(call: u32) -> i32 {
    let function_call = unsafe { &*(call as *const RawFunctionCall) };
    (function_call.params_end as u32).wrapping_sub(function_call.params_begin as u32) as i32 / CALL_PARAM_SIZE as i32
}

pub(super) unsafe fn call_params_begin(call: u32) -> u32 {
    unsafe { &*(call as *const RawFunctionCall) }.params_begin as u32
}

pub(super) unsafe fn call_params_end(call: u32) -> u32 {
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
pub(super) const OFFSET_UNIT_DIRECTION: u32 = 0x12c;
pub(super) const OFFSET_UNIT_POS_X: u32 = 0x114;
pub(super) const OFFSET_UNIT_POS_Y: u32 = 0x118;
/// Byte cleared by `fMoveEntityDeltaF` on a `ZTUnitType` unit.
pub(super) const OFFSET_UNIT_MOVE_FLAG: u32 = 0x20d;
/// `GLOBAL_ZTWorldMgr`'s embedded `BFMap` sits at `+8`.
pub(super) const OFFSET_WORLD_MAP: u32 = 8;

pub(super) unsafe fn move_entity_delta_vanilla(map: u32, unit: u32, delta: *const [i32; 3]) {
    unsafe { BFMAP_MOVE_ENTITY_DELTA.original()(map as *const u32, unit as *const u32, delta as *const i32) };
}

/// The three call parameters converted through the manager's `convertConstantParam` virtual, in order, or
/// `None` unless the call has exactly three.
pub(super) unsafe fn convert_three_params(this: u32, call: u32, unit: u32) -> Option<[i32; 3]> {
    if unsafe { call_param_count(call) } != 3 {
        return None;
    }
    let begin = unsafe { call_params_begin(call) };
    let convert = |param: u32| unsafe { call_virtual_args(this, AI_VTABLE_CONVERT_CONSTANT_PARAM, [param, unit]) } as i32;
    Some([convert(begin), convert(begin + 0xc), convert(begin + 0x18)])
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

pub(super) unsafe fn clear_mixer_vanilla(mixer: u32) {
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
    unsafe { call_virtual_args(unit, ENTITY_VTABLE_SET_STOP_AT_END, [1]) };
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
    let seconds = unsafe { call_virtual_args(this, AI_VTABLE_CONVERT_CONSTANT_PARAM, [begin + CALL_PARAM_SIZE, unit]) };
    let flag_b = u32::from(get_from_memory::<u8>(call + CALL_FLAG_B_OFFSET));
    unsafe { play_time(this, begin, seconds.wrapping_mul(MILLIS_PER_SECOND), unit, call + CALL_DONE_FLAG_OFFSET, flag_b) }
}

/// The animation name `fPlaySound` hands to `fPlaySpecial`: the text `0` (vanilla builds it in a temporary
/// byte vector).
pub(super) const PLAY_SOUND_ANIM_NAME: &str = "0";

/// `BFAIMgr::fPlaySound(sound, unit, done)`: plays a sound alone by calling `fPlaySpecial(anim "0", sound, 0, 0,
/// plain-mode string)`. `mode_a` is the value of the `[0x0064bdac]` global (the string pointer vanilla passes);
/// `play_special` is `fPlaySpecial`. Returns its result.
pub unsafe fn f_play_sound(this: u32, sound: u32, unit: u32, done: *const u8, mode_a: u32, play_special: unsafe fn(&FPlaySpecialArgs) -> bool) -> bool {
    let anim = OwnedString::new(PLAY_SOUND_ANIM_NAME);
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

    let is = |which, string: &OwnedString| unsafe { (ops.is_string)(which, string.as_ptr() as u32) };
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
pub(super) struct PlaySpecialFFacts {
    pub(super) seconds: bool,
    pub(super) repeat: bool,
    pub(super) sound_none: bool,
    pub(super) anim_none: bool,
    pub(super) special_mode_a: bool,
    pub(super) special_mode_b: bool,
    pub(super) special_none: bool,
    pub(super) special_reverse: bool,
    pub(super) special_ping_pong: bool,
}

/// How a call with no duration, no repeat count and the "none" sound is rewritten: by the fifth parameter, the
/// plain modes and "none" give `fPlay`, then reverse and ping-pong their own names; any other value keeps the
/// name. Every kind ends by forwarding to `fPlayPingPongF`, whose result is returned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum NoSoundPlay {
    Plain,
    Reverse,
    PingPong,
    Other,
}

/// The action `fPlaySpecialF` takes for a five-parameter call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum PlaySpecialFAction {
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

pub(super) fn decide_play_special_f(facts: PlaySpecialFFacts) -> PlaySpecialFAction {
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

pub(super) unsafe fn convert_param_vanilla(this: u32, param: u32, unit: u32) -> i32 {
    (unsafe { call_virtual_args(this, AI_VTABLE_CONVERT_CONSTANT_PARAM, [param, unit]) }) as i32
}

pub(super) unsafe fn replace_params_vanilla(call: u32, strings: &[u32]) {
    // Copy the bytes first: a string may be one of the call's own parameters, which the rewrite frees.
    let copies: Vec<Vec<u8>> = strings.iter().map(|&string| unsafe { &*(string as *const RawCallString) }.bytes().to_vec()).collect();
    let views: Vec<&[u8]> = copies.iter().map(Vec::as_slice).collect();
    unsafe { crate::bfscriptmgr::set_call_args(call, &views) };
}

pub(super) unsafe fn set_call_name_vanilla(call: u32, text: &[u8]) {
    unsafe { crate::bfscriptmgr::set_call_name(call, text) };
}

pub(super) unsafe fn forward_special_vanilla(handler: SpecialHandler, this: u32, call: u32, unit: u32) -> bool {
    let (this, call, unit) = (this as *const u32, call as *const u32, unit as *const u32);
    match handler {
        SpecialHandler::Play => unsafe { F_PLAY_F.hooked()(this, call, unit, 0, 0) },
        SpecialHandler::PlayReverse => unsafe { F_PLAY_REVERSE_F.hooked()(this, call, unit) },
        SpecialHandler::PlayPingPong => unsafe { F_PLAY_PING_PONG_F.hooked()(this, call, unit) },
        SpecialHandler::PlayWithSound => unsafe { F_PLAY_WITH_SOUND_F.hooked()(call, unit) },
        SpecialHandler::PlayTime => unsafe { F_PLAY_TIME_F.hooked()(this, call, unit) },
    }
}

pub(super) unsafe fn play_special_f_forward(args: &FPlaySpecialArgs) -> bool {
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

pub(super) const PLAY_SPECIAL_F_VANILLA: PlaySpecialFOps = PlaySpecialFOps {
    is_string: special_string_is,
    convert: convert_param_vanilla,
    replace_params: replace_params_vanilla,
    set_name: set_call_name_vanilla,
    forward: forward_special_vanilla,
    play_special: play_special_f_forward,
};

pub(super) unsafe fn play_time_vanilla(this: u32, name: u32, millis: u32, unit: u32, done: u32, flag_b: u32) -> u32 {
    u32::from(unsafe { F_PLAY_TIME.hooked()(this as *const u32, name as *const c_void, millis as i32, unit as *const u32, done as *const i8, flag_b as i8) })
}

/// `ZTAIMgr` fields `initAIParams` fills: guest and staff random-walk extents (x then y), then the animal and
/// guest reset intervals.
pub(super) const OFFSET_ANIMAL_RESET_INTERVAL: u32 = 0x138;
pub(super) const OFFSET_GUEST_RESET_INTERVAL: u32 = 0x13c;
/// Default for each random-walk extent, and the section the keys live in.
pub(super) const RANDOM_WALK_DEFAULT: i32 = 9;
pub(super) const AI_INI_SECTION: &str = "AI";
