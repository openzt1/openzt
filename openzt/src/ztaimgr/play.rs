//! `fPlaySpecial`, `fPlayF`, `fPlayWithBaseF` and `fSetBaseF`.

use super::*;

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

pub(super) const RVA_PLAY_REVERSE_STRING: u32 = 0x0024_3620;
pub(super) const RVA_PLAY_PING_PONG_STRING: u32 = 0x0024_3624;
pub(super) const RVA_PLAY_MODE_A_STRING: u32 = 0x0024_bdac;
pub(super) const RVA_PLAY_MODE_B_STRING: u32 = 0x0024_bda8;
pub(super) const UNIT_TYPE: u32 = 0x128;
pub(super) const AI_DEFAULT_ANIM: u32 = 0xd0;
pub(super) const ENTITY_VT_DEFAULT_ANIM: u32 = 0x8c;

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

pub(super) fn string_is_empty(string: u32) -> bool {
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

pub(super) unsafe fn special_string_is(which: SpecialString, string: u32) -> bool {
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

pub(super) unsafe fn entity_find_anim_vanilla(unit: u32, name: u32) -> u32 {
    unsafe { BFENTITY_FIND_ANIM.hooked()(unit as *const u32, name as *const std::ffi::c_void) as u32 }
}

/// The first link and its one-element vector, held across `fPlaySpecial`'s body.
pub(super) struct FirstLinks {
    pub(super) link: [u32; 3],
    pub(super) vector: Option<LinkVector>,
    pub(super) second: LinkVector,
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
pub(super) unsafe fn play_links_vanilla(this: u32, unit: u32, first: u32, anim: u32, reverse: bool, sound: u32, ping_pong: bool, seconds: u32, count: i32) -> Option<bool> {
    unsafe { play_links_with(&VANILLA_MIXER, this, unit, first, anim, reverse, sound, ping_pong, seconds, count) }
}

pub(super) const F_PLAY_SPECIAL_VANILLA_OPS: FPlaySpecialOps = FPlaySpecialOps {
    is_string: special_string_is,
    find_anim: entity_find_anim_vanilla,
    find_sound: find_sound_vanilla,
    acquire_first: acquire_first_vanilla,
    release_first: release_first_vanilla,
    play_links: play_links_vanilla,
};

/// `BFAIMgr` field the play handlers use as their loop cursor: the pointer to the parameter being resolved.
pub(super) const AI_PARAM_CURSOR: u32 = 0xcc;
pub(super) const UNIT_DIRECTION: u32 = 0x12c;
pub(super) const ENTITY_VT_DIR_TO_SET: u32 = 0x80;
pub(super) const MIXER_COUNT_A: u32 = 0x28;
pub(super) const MIXER_COUNT_B: u32 = 0x18;
pub(super) const CALL_RESULT_FLAG: u32 = 0x18;

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
pub(super) unsafe fn play_not_stopped(call: u32, unit: u32, ops: &PlayOps) -> Option<bool> {
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
pub(super) unsafe fn add_param_anims(this: u32, call: u32, unit: u32, handle: u32, from: u32, reverse: bool, ping_pong: bool, ops: &PlayOps) {
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

pub(super) unsafe fn finish_play(call: u32, unit: u32, handle: u32, with_base: bool, this: u32, ops: &PlayOps) -> bool {
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

pub(super) unsafe fn mixer_anim_vanilla(mixer: u32) -> u32 {
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

pub(super) unsafe fn play_vanilla(this: u32, unit: u32, handle: u32, with_base: bool) -> Option<bool> {
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
        let set = unsafe { call_virtual_args(unit, ENTITY_VT_DIR_TO_SET, [direction, &mut out as *mut u32 as u32, anim, 0]) };
        save_to_memory(unit + UNIT_MIXER_FLAG, 0u8);
        unsafe { (m.set_set)(mixer, set, out as u8) };
        unsafe { (m.set_base)(mixer, vector.ptr()) };
    }
    release_anim(link[0]);
}

pub(super) unsafe fn set_base_vanilla(unit: u32, anim: u32) {
    unsafe { set_base_with(&VANILLA_MIXER, unit, anim) }
}

pub(super) const PLAY_VANILLA_OPS: PlayOps = PlayOps {
    find_anim: entity_find_anim_vanilla,
    mixer_anim: mixer_anim_vanilla,
    acquire_first: acquire_first_vanilla,
    release_first: release_first_vanilla,
    add_anim: add_anim_vanilla,
    play: play_vanilla,
    set_base: set_base_vanilla,
};
