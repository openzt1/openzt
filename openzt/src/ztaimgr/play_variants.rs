//! `fFly`, `fPlayWithSound`, `fPlayTime`, `fPlaySetProb` and `fPlaySet`.

use super::*;

/// The vanilla call `fFlyF` makes: `fFly(this, unit, name, value, extra, done, reverse, limit)` with `name`
/// and `extra` vanilla strings.
pub struct FlyOps {
    pub f_fly: unsafe fn(u32, u32, u32, u32, u32, u32, u32, u32) -> bool,
}

pub(super) unsafe fn f_fly_vanilla(this: u32, unit: u32, name: u32, value: u32, extra: u32, done: u32, reverse: u32, limit: u32) -> bool {
    unsafe { F_FLY.hooked()(this as *const u32, unit as *const u32, name as *const c_void, value as i32, extra as *const c_void, done as *const i8, reverse as i32, limit as i32) }
}

pub(super) const FLY_VANILLA: FlyOps = FlyOps { f_fly: f_fly_vanilla };

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
pub(super) const UNIT_FLY_POSITION: u32 = 0x114;
pub(super) const UNIT_FLY_TARGET: u32 = 0x1dc;

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

pub(super) const FLY_CORE_VANILLA: FlyCoreOps =
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
pub(super) const UNIT_TYPE_POINTER: u32 = 0x128;

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

pub(super) unsafe fn play_two_vanilla(unit: u32, first_anim: u32, second_anim: u32, sound: u32) -> Option<bool> {
    unsafe { play_two_with(&VANILLA_MIXER, unit, first_anim, second_anim, sound) }
}

pub(super) const PLAY_WITH_SOUND_VANILLA: PlayWithSoundOps = PlayWithSoundOps { find_anim: entity_find_anim_vanilla, find_sound: find_sound_vanilla, play_two: play_two_vanilla };

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

pub(super) unsafe fn play_timed_vanilla(unit: u32, first_anim: u32, second_anim: u32, millis: i32) -> Option<bool> {
    unsafe { play_timed_with(&VANILLA_MIXER, unit, first_anim, second_anim, millis) }
}

pub(super) const PLAY_TIME_VANILLA_OPS: PlayTimeOps = PlayTimeOps { find_anim: entity_find_anim_vanilla, play_timed: play_timed_vanilla };

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

/// A `(name, probability)` entry of the vector `BFAIMgr::fPlaySetProb` reads: a vanilla-layout string (Rust-owned
/// buffer) and a weight, 16 bytes.
#[repr(C)]
pub(super) struct SetProbEntry {
    pub(super) name: OwnedString,
    pub(super) probability: u32,
}

/// The vanilla calls `fPlaySetProbF` makes: the C-string-to-int conversion (`0x00405218`, `cdecl`) and
/// `fPlaySetProb(vector, unit)` (`stdcall`).
pub struct SetProbOps {
    pub parse_int: unsafe fn(*const u8) -> u32,
    pub play_set_prob: unsafe fn(u32, u32) -> bool,
}

pub(super) unsafe fn parse_int_vanilla(text: *const u8) -> u32 {
    unsafe { ATOI.original()(text as *const i8) as u32 }
}

pub(super) unsafe fn play_set_prob_vanilla(vector: u32, unit: u32) -> bool {
    (unsafe { F_PLAY_SET_PROB.hooked()(vector as *const i32, unit as *const std::ffi::c_void) }) as u8 != 0
}

pub(super) const SET_PROB_VANILLA: SetProbOps = SetProbOps { parse_int: parse_int_vanilla, play_set_prob: play_set_prob_vanilla };

/// The vanilla pieces `fPlaySetProb` leans on: the game's `rand()` (`0..=0x7fff`) and `fPlaySet(name, unit)`
/// (called with the "child" flag and the `1` argument, returning its low byte).
pub struct SetProbPickOps {
    pub rand: unsafe fn() -> u32,
    pub play_set: unsafe fn(u32, u32) -> u32,
}

pub(super) const SET_PROB_ENTRY_SIZE: u32 = 16;
pub(super) const SET_PROB_WEIGHT_OFFSET: u32 = 0xc;
/// The weights must sum to this for `fPlaySetProb` to pick anything.
pub(super) const SET_PROB_TOTAL: i32 = 100;

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
pub(super) fn pick_set_prob(weights: &[i32], mut rand: impl FnMut() -> u32) -> Option<usize> {
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

pub(super) const SET_PROB_PICK_VANILLA: SetProbPickOps = SetProbPickOps { rand: random_vanilla, play_set: play_set_vanilla };

/// A `BFBSCall` (a behavior-set name call): a `{begin, end, cap}` string plus a flag byte at `+0xc`, 16 bytes,
/// with a Rust-owned string buffer. It is only handed to ported code, which takes the string by value and
/// releases it through `owned_strings`.
#[repr(C)]
pub struct BsCall {
    pub(super) words: [u32; 4],
}

impl BsCall {
    pub(super) unsafe fn new(name: &[u8], flag: u32) -> Self {
        let mut call = BsCall { words: [0; 4] };
        unsafe { string_fill_new(call.words.as_mut_ptr() as StringSlot, name, 1) };
        call.words[3] = flag & 0xff;
        call
    }

    pub fn as_ptr(&self) -> u32 {
        self.words.as_ptr() as u32
    }
}

impl Drop for BsCall {
    fn drop(&mut self) {
        unsafe { string_release(self.words.as_mut_ptr() as StringSlot) };
    }
}

/// Entity fields `fPlaySet` reads: the "has a pending set" byte (`+0x6d`) and the pending `BFBSCall` (`+0xdc`,
/// begin and end words at `+0xdc`/`+0xe0`); the behavior set embedded at `+0x14`.
pub(super) const ENTITY_HAS_PENDING_SET: u32 = 0x6d;
pub(super) const ENTITY_PENDING_BS_CALL: u32 = 0xdc;
pub(super) const ENTITY_BEHAVIOR_SET: u32 = 0x14;
/// `setBS` clears the mixer by mode: `0` total, `1` everything, anything else leaves it. `fPlaySet` passes `2`.
pub(super) const SET_BS_NO_MIXER_CLEAR: u32 = 2;

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

/// Copies a `BFBSCall` into a Rust-owned one, for a callee that takes it by value and releases it.
pub(super) unsafe fn copy_bs_call(source: u32) -> [u32; 4] {
    let text = unsafe { owned_strings::string_bytes(source as *const [*mut u8; 3]) };
    let mut copy = [0u32; 4];
    unsafe { string_fill_new(copy.as_mut_ptr() as StringSlot, text, 1) };
    copy[3] = get_from_memory::<u8>(source + 0xc) as u32;
    copy
}

pub(super) unsafe fn init_child_vanilla(unit: u32, name: u32) -> bool {
    unsafe { crate::bfbehaviorset::init_child((unit + ENTITY_BEHAVIOR_SET) as *const u32, unit, name) }
}

pub(super) unsafe fn set_bs_vanilla(unit: u32, call: u32, force: u32, mode: u32) {
    let [w0, w1, w2, w3] = unsafe { copy_bs_call(call) };
    let name = crate::bfscriptmgr::RawString { begin: w0 as *const u8, end: w1 as *const u8, capacity: w2 as *const u8 };
    unsafe { crate::bfentity::set_bs(unit, name, w3 as u8, force as u8, mode as i32) };
}

pub(super) unsafe fn set_pending_vanilla(unit: u32, call: u32, force: u32) {
    let [w0, w1, w2, w3] = unsafe { copy_bs_call(call) };
    let name = crate::bfscriptmgr::RawString { begin: w0 as *const u8, end: w1 as *const u8, capacity: w2 as *const u8 };
    unsafe { crate::bfentity::set_pending_bs(unit, name, w3 as u8, force as u8) };
}

pub(super) const PLAY_SET_VANILLA: PlaySetOps = PlaySetOps { init_child: init_child_vanilla, set_bs: set_bs_vanilla, set_pending: set_pending_vanilla };

/// `BFAIMgr::fPlaySetProbF(call, unit)`: an even number (at least two) of parameters, read as
/// `(name, weight)` pairs. Each name is the parameter's C string (up to its NUL, not its recorded end) and
/// each weight its converted text; `fPlaySetProb` then picks among them. The entry vector lives in Rust memory
/// (vanilla only reads it) and each name string is Rust-owned and freed when the entries drop.
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
        let name = OwnedString::from_bytes(unsafe { std::ffi::CStr::from_ptr(name_text) }.to_bytes());
        let probability = unsafe { (ops.parse_int)(weight_text) };
        entries.push(SetProbEntry { name, probability });
    }
    let start = entries.as_ptr() as u32;
    let header = [start, start + entries.len() as u32 * 16, start + entries.capacity() as u32 * 16];
    unsafe { (ops.play_set_prob)(header.as_ptr() as u32, unit) }
}

/// `BFAIMgr::fRandomWalk` fields. Unit: position words (`+0x114`/`+0x118` in 1/64-tile units, `+0x11c` in
/// 1/16-tile height units), the walking guard byte `+0x20f`. Manager: the extra random extents `+0xd4`/`+0xd8`.
pub(super) const UNIT_POS_Z: u32 = 0x11c;
pub(super) const UNIT_WALKING_GUARD: u32 = 0x20f;
pub(super) const AI_RANDOM_EXTRA_X: u32 = 0xd4;
pub(super) const AI_RANDOM_EXTRA_Y: u32 = 0xd8;
/// `BFMap` fields: width (`+0x2c`), height (`+0x30`), tile array (`+0x38`, 0x8c-byte tiles).
pub(super) const MAP_WIDTH: u32 = 0x2c;
pub(super) const MAP_HEIGHT: u32 = 0x30;
pub(super) const MAP_TILES: u32 = 0x38;
pub(super) const MAP_TILE_SIZE: u32 = 0x8c;
/// A tile's three position words.
pub(super) const TILE_POSITION_OFFSET: u32 = 0x34;
/// Manager vtable slots: `getPath(start, goal, unit, flag)` and the two random extents of a unit.
pub(super) const AI_VT_GET_PATH: u32 = 0x18;
pub(super) const AI_VT_RANDOM_X: u32 = 0x20;
pub(super) const AI_VT_RANDOM_Y: u32 = 0x24;
/// Unit vtable slots: `canWalkOn(tile, flag)`, `getDestination(&position)` and `setDestination(&position)`.
pub(super) const ENTITY_VT_CAN_WALK_ON: u32 = 0x168;
pub(super) const ENTITY_VT_GET_DESTINATION: u32 = 0x1f4;
pub(super) const ENTITY_VT_SET_DESTINATION: u32 = 0x1f8;
/// RVAs of the "no destination" position triple and the path-cost limit.
pub(super) const RVA_NO_DESTINATION: u32 = 0x0023_9370;
pub(super) const RVA_PATH_COST_LIMIT: u32 = 0x0023_5494;
