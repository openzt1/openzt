//! `GXMixer`'s read-side methods and playback: `getAnimSet` (`0x4010e7`), `setSet` (`0x40165f`), `setTime`
//! (`0x4340e1`), `getAnim` (`0x411fed`), `update` (`0x40e214`), `totalClear` (`0x40df80`) and `clearAll`
//! (`0x4b1e62`).
//!
//! The mixer is shared by `BFEntity` and about a dozen un-ported UI and map classes that build and destroy it with
//! inlined vanilla code (see `plans/gxmixer-scoping-plan.md`), so its layout and every internal buffer stay vanilla's
//! (style 1): these methods read and write the mixer in place and allocate nothing.
//!
//! ## Layout used here
//! - `+0x00`/`+0x04`: begin/end of an array of 12-byte entries whose first word is an animation set
//!   (`GXLLEAnimSet`); `+0x58` is the current entry's index.
//! - An animation set holds an array of 0x1c-byte `GXLLEAnim`s at `+0x10..+0x14` and a flag byte at `+0x24`.
//! - `+0x4c` is the current animation's index in that array, `+0x64` the byte `setSet` stores, and `+0x50`, `+0x54`
//!   and `+0x5c` the playback time state `setTime` clears.
//! - A `GXLLEAnim` keeps the begin/end of a frame array at `+0x08`/`+0x0c`, a data record pointer at `+0x14` (its
//!   words at `+0x04`, `+0x08`, `+0x0c` are non-null once loaded; `+0x08` is the animation's name, `+0x10` its size)
//!   and a count byte at `+0x18`.
//! - `+0x0c`/`+0x10` is a second array of 12-byte `{anim, sound, flag}` link entries (`GXMixerLink`s; the first word's
//!   `+0x20` is a reference count). `+0x18` is a `deque<GXMixerChain>`. `+0x61` is the "finished" byte, `+0x62` "the
//!   links changed" and `+0x63` "hold the last frame".
//!
//! Vanilla helpers the port still calls: `GXMixer::advanceSet` (`0x4074de`) and `getTotalTime` (`0x412096`), the
//! deque clear (`0x41bf8f`), `setPlaying` (`0x40dd08`) and the entry's chain object's virtual `+0x24`. Ghidra labels
//! them `ZTScenarioTimer::meth_0x4074de`, `FUN_00412096`, `ZTScenarioTimer::meth_0x40dd08` and
//! `cls_0x41bf8f::meth_0x41bf8f`; the macOS binary calls them `advanceSet`, `getTotalTime`, `setPlaying` and the
//! deque's `clear`. (The `FUN_005ef450` in Ghidra's `update` is the shared acquire block, not a call.)

use std::ffi::c_void;

use openzt_detour::generated::{
    gxlleanim::ACQUIRE as GXLLEANIM_ACQUIRE,
    gxmixer::{CLEAR_ALL, GET_ANIM, GET_ANIM_SET, SET_SET, SET_TIME, TOTAL_CLEAR, UPDATE},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::get_module_base,
    util::{get_from_memory, save_to_memory},
};

const SETS_BEGIN: u32 = 0x00;
const SETS_END: u32 = 0x04;
const ANIM_INDEX: u32 = 0x4c;
const TIME_A: u32 = 0x50;
const TIME_B: u32 = 0x54;
const SET_INDEX: u32 = 0x58;
const TIME_C: u32 = 0x5c;
const SET_FLAG: u32 = 0x64;
const SET_ENTRY_SIZE: u32 = 12;

const SET_ANIMS_BEGIN: u32 = 0x10;
const SET_ANIMS_END: u32 = 0x14;
const SET_COUNTS_SIZE_FLAG: u32 = 0x24;
const ANIM_SIZE: u32 = 0x1c;

const ANIM_FRAMES_BEGIN: u32 = 0x08;
const ANIM_FRAMES_END: u32 = 0x0c;
const ANIM_DATA: u32 = 0x14;
const ANIM_COUNT_BYTE: u32 = 0x18;
const DATA_WORDS: [u32; 3] = [0x04, 0x08, 0x0c];
const DATA_NAME: u32 = 0x08;
const DATA_SIZE: u32 = 0x10;

/// `GLOBAL_BFAnimCache`'s cell and the running size total at `+0xc` of the cache it points to.
const RVA_ANIM_CACHE: u32 = 0x0023_8044;
const CACHE_TOTAL: u32 = 0x0c;

/// Signed 32-bit element count between two pointers, reinterpreted as the unsigned bound vanilla compares
/// indices against.
fn element_count(begin: u32, end: u32, size: u32) -> u32 {
    (end.wrapping_sub(begin) as i32 / size as i32) as u32
}

/// The animation set of the current entry, or null when the index is out of range.
pub(crate) fn get_anim_set(mixer: u32) -> u32 {
    let begin = get_from_memory::<u32>(mixer + SETS_BEGIN);
    let count = element_count(begin, get_from_memory::<u32>(mixer + SETS_END), SET_ENTRY_SIZE);
    let index = get_from_memory::<u32>(mixer + SET_INDEX);
    if index < count {
        get_from_memory::<u32>(begin + index * SET_ENTRY_SIZE)
    } else {
        0
    }
}

/// `GXMixer::setSet(anim_index, flag)`: stores `flag` at `+0x64` and selects `anim_index` in the current set
/// when it is non-zero and below the set's animation count; otherwise the index becomes 0.
pub(crate) fn set_set(mixer: u32, anim_index: u32, flag: u8) {
    save_to_memory(mixer + ANIM_INDEX, 0u32);
    save_to_memory(mixer + SET_FLAG, flag);
    if anim_index == 0 {
        return;
    }
    let set = get_anim_set(mixer);
    if get_from_memory::<u32>(mixer + SET_INDEX) >= element_count(get_from_memory::<u32>(mixer + SETS_BEGIN), get_from_memory::<u32>(mixer + SETS_END), SET_ENTRY_SIZE) {
        return;
    }
    let anims = element_count(get_from_memory::<u32>(set + SET_ANIMS_BEGIN), get_from_memory::<u32>(set + SET_ANIMS_END), ANIM_SIZE);
    if anim_index < anims {
        save_to_memory(mixer + ANIM_INDEX, anim_index);
    }
}

/// What the playback code needs from vanilla, so the state machine can run over fake mixers.
pub(crate) trait MixerOps {
    /// `GXLLEAnim::acquire(anim, name)`.
    fn acquire(&mut self, anim: u32, name: u32);
    /// The `BFAnimCache` object whose running size total (`+0xc`) a flagged set adds to.
    fn cache(&mut self) -> u32;
    /// `GXMixer::advanceSet`.
    fn advance_set(&mut self, mixer: u32);
    /// `GXMixer::getTotalTime`.
    fn total_time(&mut self, mixer: u32) -> u32;
    /// The virtual at `+0x24` of an entry's chain object, called with `argument` (the mixer's `+0x40`).
    fn release_chain(&mut self, chain: u32, argument: u32);
}

/// The vanilla implementations.
pub(crate) struct VanillaOps;

const ADVANCE_SET: u32 = 0x0040_74de;
const GET_TOTAL_TIME: u32 = 0x0041_2096;
const SET_PLAYING: u32 = 0x0040_dd08;
const DEQUE_CLEAR: u32 = 0x0041_bf8f;
const CHAIN_RELEASE_SLOT: u32 = 0x24;
const MIXER_DEQUE: u32 = 0x18;
const MIXER_LINKS_A: u32 = 0x00;
const MIXER_LINKS_B: u32 = 0x0c;
const MIXER_CHAIN_ARG: u32 = 0x40;
const MIXER_FINISHED: u32 = 0x61;
const MIXER_LINKS_CHANGED: u32 = 0x62;
const MIXER_HOLD_LAST: u32 = 0x63;
const DEQUE_CURSOR: u32 = 0x28;
const DEQUE_FIRST: u32 = 0x18;
const LINK_REFCOUNT: u32 = 0x20;

impl MixerOps for VanillaOps {
    fn acquire(&mut self, anim: u32, name: u32) {
        unsafe { GXLLEANIM_ACQUIRE.original()(anim as *const u32, name as *const i8) };
    }

    fn cache(&mut self) -> u32 {
        get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_ANIM_CACHE)
    }

    fn advance_set(&mut self, mixer: u32) {
        unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32)>(ADVANCE_SET)(mixer) };
    }

    fn total_time(&mut self, mixer: u32) -> u32 {
        unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32) -> u32>(GET_TOTAL_TIME)(mixer) }
    }

    fn release_chain(&mut self, chain: u32, argument: u32) {
        let target = get_from_memory::<u32>(get_from_memory::<u32>(chain) + CHAIN_RELEASE_SLOT);
        unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32, u32)>(target)(chain, argument) };
    }
}

fn word(address: u32) -> u32 {
    get_from_memory::<u32>(address)
}

fn set_count(mixer: u32) -> u32 {
    element_count(word(mixer + SETS_BEGIN), word(mixer + SETS_END), SET_ENTRY_SIZE)
}

/// The animation at `index` in `set`, or `None` when the index is out of range.
fn anim_in(set: u32, index: u32) -> Option<u32> {
    let begin = word(set + SET_ANIMS_BEGIN);
    (index < element_count(begin, word(set + SET_ANIMS_END), ANIM_SIZE)).then(|| begin + index * ANIM_SIZE)
}

/// Frames the animation still has to play: its frame array's length less its count byte, signed.
fn frames_left(anim: u32) -> i32 {
    ((word(anim + ANIM_FRAMES_END).wrapping_sub(word(anim + ANIM_FRAMES_BEGIN)) as i32) >> 2) - i32::from(get_from_memory::<u8>(anim + ANIM_COUNT_BYTE))
}

/// Acquires `anim` unless it is loaded (frames left, and a data record whose three words are set), and adds its
/// size to the cache total when `set` is flagged.
fn ensure_loaded(ops: &mut impl MixerOps, set: u32, anim: u32) {
    let data = word(anim + ANIM_DATA);
    let loaded = frames_left(anim) != 0 && data != 0 && DATA_WORDS.iter().all(|offset| word(data + offset) != 0);
    if loaded {
        return;
    }
    let name = if data == 0 { 0 } else { word(data + DATA_NAME) };
    ops.acquire(anim, name);
    if get_from_memory::<u8>(set + SET_COUNTS_SIZE_FLAG) != 0 {
        let data = word(anim + ANIM_DATA);
        let size = if data == 0 { 0 } else { word(data + DATA_SIZE) };
        let total = ops.cache() + CACHE_TOTAL;
        save_to_memory(total, word(total).wrapping_add(size));
    }
}

/// `GXMixer::getAnim`: the current animation of the current set, or null (see [`ensure_loaded`]).
pub(crate) fn get_anim_with(mixer: u32, ops: &mut impl MixerOps) -> u32 {
    let set = get_anim_set(mixer);
    if set == 0 {
        return 0;
    }
    let Some(anim) = anim_in(set, word(mixer + ANIM_INDEX)) else {
        return 0;
    };
    ensure_loaded(ops, set, anim);
    anim
}

pub(crate) unsafe fn get_anim(mixer: u32) -> u32 {
    get_anim_with(mixer, &mut VanillaOps)
}

/// Whether the two link arrays (`+0x00` and `+0x0c`) still differ: a different count, or any entry whose first two
/// words or flag byte differ.
fn links_differ(mixer: u32) -> bool {
    let (a, b) = (word(mixer + MIXER_LINKS_A), word(mixer + MIXER_LINKS_B));
    let count = element_count(a, word(mixer + MIXER_LINKS_A + 4), SET_ENTRY_SIZE);
    if count != element_count(b, word(mixer + MIXER_LINKS_B + 4), SET_ENTRY_SIZE) {
        return true;
    }
    (0..count).any(|index| {
        let (x, y) = (a + index * SET_ENTRY_SIZE, b + index * SET_ENTRY_SIZE);
        word(x) != word(y) || word(x + 4) != word(y + 4) || get_from_memory::<u8>(x + 8) != get_from_memory::<u8>(y + 8)
    })
}

/// `GXMixer::update(step, stop_at_end)`: advances the playback position by `step`, moving through frames,
/// animations and sets, acquiring animations as they come up. `stop_at_end` freezes on the last frame instead of
/// moving on to the next set. A finished mixer (`+0x61`) is left alone.
pub(crate) fn update_with(ops: &mut impl MixerOps, mixer: u32, step: u32, stop_at_end: bool) {
    if get_from_memory::<u8>(mixer + MIXER_FINISHED) != 0 {
        return;
    }
    if get_from_memory::<u8>(mixer + MIXER_LINKS_CHANGED) != 0 && (word(mixer + DEQUE_CURSOR) != word(mixer + DEQUE_FIRST) || links_differ(mixer)) {
        save_to_memory(mixer + MIXER_LINKS_CHANGED, 0u8);
        ops.advance_set(mixer);
    }

    let index = word(mixer + SET_INDEX);
    if index >= set_count(mixer) {
        ops.advance_set(mixer);
        return;
    }
    let entry = word(mixer + SETS_BEGIN) + index * SET_ENTRY_SIZE;
    let mut set = word(entry);
    let chain = word(entry + 4);
    if chain != 0 && step != 0 {
        ops.release_chain(chain, mixer + MIXER_CHAIN_ARG);
        save_to_memory(entry + 4, 0u32);
    }
    let Some(mut anim) = anim_in(set, word(mixer + ANIM_INDEX)) else {
        ops.advance_set(mixer);
        return;
    };
    ensure_loaded(ops, set, anim);

    if get_from_memory::<u8>(mixer + MIXER_HOLD_LAST) != 0 && frames_left(anim) == 1 && word(mixer + DEQUE_CURSOR) == word(mixer + DEQUE_FIRST) {
        return;
    }
    save_to_memory(mixer + TIME_A, word(mixer + TIME_A).wrapping_add(step));
    save_to_memory(mixer + TIME_B, word(mixer + TIME_B).wrapping_add(step));
    if word(mixer + TIME_B) > ops.total_time(mixer) {
        let total = ops.total_time(mixer);
        save_to_memory(mixer + TIME_B, word(mixer + TIME_B).wrapping_sub(total));
    }
    if word(anim) == 0 {
        return;
    }

    loop {
        let elapsed = word(mixer + TIME_A);
        let duration = word(anim);
        if elapsed < duration {
            return;
        }
        save_to_memory(mixer + TIME_A, elapsed - duration);
        let frame = word(mixer + TIME_C).wrapping_add(1);
        save_to_memory(mixer + TIME_C, frame);
        if frame >= frames_left(anim) as u32 {
            if stop_at_end {
                save_to_memory(mixer + MIXER_FINISHED, 1u8);
                save_to_memory(mixer + TIME_A, 0u32);
                save_to_memory(mixer + TIME_C, (frames_left(anim) as u32).wrapping_sub(1));
                return;
            }
            // Moves to the next set's animation at the same index, then keeps skipping whole animations the
            // remaining time covers.
            let mut next = word(mixer + SET_INDEX).wrapping_add(1);
            save_to_memory(mixer + SET_INDEX, next);
            if next >= set_count(mixer) {
                ops.advance_set(mixer);
                return;
            }
            save_to_memory(mixer + TIME_C, 0u32);
            set = word(word(mixer + SETS_BEGIN) + next * SET_ENTRY_SIZE);
            let Some(first) = anim_in(set, word(mixer + ANIM_INDEX)) else {
                return;
            };
            anim = first;
            ensure_loaded(ops, set, anim);
            if word(anim) == 0 {
                return;
            }
            loop {
                let whole = (frames_left(anim) as u32).wrapping_mul(word(anim));
                let elapsed = word(mixer + TIME_A);
                if elapsed <= whole {
                    break;
                }
                save_to_memory(mixer + TIME_A, elapsed - whole);
                next = word(mixer + SET_INDEX).wrapping_add(1);
                save_to_memory(mixer + SET_INDEX, next);
                if next >= set_count(mixer) {
                    ops.advance_set(mixer);
                    return;
                }
                set = word(word(mixer + SETS_BEGIN) + next * SET_ENTRY_SIZE);
                let Some(following) = anim_in(set, word(mixer + ANIM_INDEX)) else {
                    return;
                };
                anim = following;
                ensure_loaded(ops, set, anim);
                if word(anim) == 0 {
                    return;
                }
            }
        }
        if word(anim) == 0 {
            return;
        }
    }
}

pub(crate) unsafe fn update(mixer: u32, step: i32, stop_at_end: i8) {
    update_with(&mut VanillaOps, mixer, step as u32, stop_at_end != 0);
}

/// `GXMixer::setTime(time, flag)`: clears the time state and runs `update`.
pub(crate) unsafe fn set_time(mixer: u32, time: i32, flag: i8) {
    for offset in [TIME_A, TIME_B, TIME_C] {
        save_to_memory(mixer + offset, 0u32);
    }
    unsafe { update(mixer, time, flag) };
}

/// Releases every link in the array at `array_offset` (one reference each) and empties it, keeping its buffer.
fn release_links(mixer: u32, array_offset: u32) {
    let (begin, end) = (word(mixer + array_offset), word(mixer + array_offset + 4));
    let mut link = begin;
    while link != end {
        let anim = word(link);
        if anim != 0 {
            save_to_memory(anim + LINK_REFCOUNT, word(anim + LINK_REFCOUNT).wrapping_sub(1));
        }
        link += SET_ENTRY_SIZE;
    }
    save_to_memory(mixer + array_offset + 4, begin);
}

/// `GXMixer::totalClear`: clears the chain deque, runs vanilla's `setPlaying` with the second link array, and
/// flags "hold the last frame".
pub(crate) unsafe fn total_clear(mixer: u32) {
    unsafe {
        std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32)>(DEQUE_CLEAR)(mixer + MIXER_DEQUE);
        std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32, u32) -> u32>(SET_PLAYING)(mixer, mixer + MIXER_LINKS_B);
    }
    save_to_memory(mixer + MIXER_HOLD_LAST, 1u8);
}

/// `GXMixer::clearAll`: clears the chain deque, resets the time state and set index, and releases and empties both
/// link arrays (vanilla assigns each from an empty vector, which keeps their buffers).
pub(crate) unsafe fn clear_all(mixer: u32) {
    unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32)>(DEQUE_CLEAR)(mixer + MIXER_DEQUE) };
    for offset in [TIME_A, TIME_B, SET_INDEX, TIME_C] {
        save_to_memory(mixer + offset, 0u32);
    }
    release_links(mixer, MIXER_LINKS_B);
    release_links(mixer, MIXER_LINKS_A);
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_ANIM_SET)]
    unsafe extern "thiscall" fn get_anim_set_detour(this: *const u32) -> u32 {
        get_anim_set(this as u32)
    }

    #[detour(SET_SET)]
    unsafe extern "thiscall" fn set_set_detour(this: *const u32, anim_index: u32, flag: u8) {
        set_set(this as u32, anim_index, flag)
    }

    #[detour(SET_TIME)]
    unsafe extern "thiscall" fn set_time_detour(this: *const u32, time: i32, flag: i8) {
        unsafe { set_time(this as u32, time, flag) }
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update_detour(this: *const u32, step: i32, stop_at_end: i8) {
        unsafe { update(this as u32, step, stop_at_end) }
    }

    #[detour(TOTAL_CLEAR)]
    unsafe extern "thiscall" fn total_clear_detour(this: *const u32) {
        unsafe { total_clear(this as u32) }
    }

    #[detour(CLEAR_ALL)]
    unsafe extern "thiscall" fn clear_all_detour(this: *const u32) {
        unsafe { clear_all(this as u32) }
    }

    #[detour(GET_ANIM)]
    unsafe extern "thiscall" fn get_anim_detour(this: *const u32) -> *const c_void {
        unsafe { get_anim(this as u32) as *const c_void }
    }

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn get_anim_set(this: u32) -> u32 {
            unsafe { super::GET_ANIM_SET_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn set_set(this: u32, anim_index: u32, flag: u8) {
            unsafe { super::SET_SET_DETOUR.call(this as *const u32, anim_index, flag) }
        }
        pub(crate) fn set_time(this: u32, time: i32, flag: i8) {
            unsafe { super::SET_TIME_DETOUR.call(this as *const u32, time, flag) }
        }
        pub(crate) fn get_anim(this: u32) -> u32 {
            unsafe { super::GET_ANIM_DETOUR.call(this as *const u32) as u32 }
        }
        pub(crate) fn update(this: u32, step: i32, stop_at_end: i8) {
            unsafe { super::UPDATE_DETOUR.call(this as *const u32, step, stop_at_end) }
        }
        pub(crate) fn total_clear(this: u32) {
            unsafe { super::TOTAL_CLEAR_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn clear_all(this: u32) {
            unsafe { super::CLEAR_ALL_DETOUR.call(this as *const u32) }
        }
    }
}

/// Installs this module's detours. Called from both `lib.rs`'s boot cascade and
/// `reimplementation_tests::init()`.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise gxmixer detours: {e:?}");
    }
}

/// Standalone fake mixers for the host tests and the live battery.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) mod fakes {
    use super::*;

    /// A standalone mixer, its entries, one animation set and its animations, all leaked, for fixture tests.
    /// `anims` is `(frames, counted, has_data)` per animation; the entry count is `entries` copies of the set.
    pub(crate) struct FakeMixer {
        pub(crate) mixer: u32,
        pub(crate) set: u32,
        pub(crate) anim_base: u32,
        pub(crate) data: u32,
    }

    pub(crate) fn build_fake_mixer(entries: usize, anims: &[(u32, u8, bool)], size_flag: u8) -> FakeMixer {
        let leak = |words: usize| Box::leak(vec![0u32; words].into_boxed_slice()).as_ptr() as u32;
        let mixer = leak(0x1a);
        let data = leak(8);
        for offset in [0x04u32, 0x08, 0x0c] {
            save_to_memory(data + offset, 0x1000 + offset);
        }
        save_to_memory(data + 0x10, 4096u32);
        let anim_base = leak(anims.len().max(1) * 7);
        for (index, (frames, counted, has_data)) in anims.iter().enumerate() {
            let anim = anim_base + index as u32 * 0x1c;
            let frame_array = leak(*frames as usize + 1);
            save_to_memory(anim + 0x08, frame_array);
            save_to_memory(anim + 0x0c, frame_array + frames * 4);
            save_to_memory(anim + 0x14, if *has_data { data } else { 0 });
            save_to_memory(anim + 0x18, *counted);
        }
        let set = leak(12);
        save_to_memory(set + 0x10, anim_base);
        save_to_memory(set + 0x14, anim_base + anims.len() as u32 * 0x1c);
        save_to_memory(set + 0x24, size_flag);
        let entry_base = leak(entries.max(1) * 3);
        for index in 0..entries as u32 {
            save_to_memory(entry_base + index * 12, set);
        }
        save_to_memory(mixer, entry_base);
        save_to_memory(mixer + 4, entry_base + entries as u32 * 12);
        FakeMixer { mixer, set, anim_base, data }
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    pub(crate) use super::detours::test_real as real;
    pub(crate) use super::fakes::build_fake_mixer;
    pub(crate) use super::{get_anim_set, set_set};

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        detours::status()
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::{build_fake_mixer, FakeMixer};
    use super::*;

    /// Records what the playback code asks vanilla for.
    struct Recorder {
        acquired: Vec<(u32, u32)>,
        advanced: u32,
        released: Vec<(u32, u32)>,
        cache: u32,
        total: u32,
    }

    impl Recorder {
        fn new() -> Self {
            Recorder { acquired: Vec::new(), advanced: 0, released: Vec::new(), cache: Box::leak(vec![10u32; 8].into_boxed_slice()).as_ptr() as u32, total: 1000 }
        }
    }

    impl MixerOps for Recorder {
        fn acquire(&mut self, anim: u32, name: u32) {
            self.acquired.push((anim, name));
        }
        fn cache(&mut self) -> u32 {
            self.cache
        }
        fn advance_set(&mut self, _mixer: u32) {
            self.advanced += 1;
        }
        fn total_time(&mut self, _mixer: u32) -> u32 {
            self.total
        }
        fn release_chain(&mut self, chain: u32, argument: u32) {
            self.released.push((chain, argument));
        }
    }

    fn set_duration(fake: &FakeMixer, index: u32, duration: u32) {
        save_to_memory(fake.anim_base + index * ANIM_SIZE, duration);
    }

    #[test]
    fn get_anim_set_follows_the_set_index_and_bounds() {
        let fake = build_fake_mixer(2, &[(3, 0, true)], 0);
        assert_eq!(get_anim_set(fake.mixer), fake.set);
        save_to_memory(fake.mixer + SET_INDEX, 1u32);
        assert_eq!(get_anim_set(fake.mixer), fake.set);
        save_to_memory(fake.mixer + SET_INDEX, 2u32);
        assert_eq!(get_anim_set(fake.mixer), 0);
        let empty = build_fake_mixer(0, &[], 0);
        assert_eq!(get_anim_set(empty.mixer), 0);
    }

    #[test]
    fn set_set_accepts_only_a_non_zero_index_below_the_animation_count() {
        let fake = build_fake_mixer(1, &[(3, 0, true), (3, 0, true), (3, 0, true)], 0);
        for (requested, expected) in [(0u32, 0u32), (1, 1), (2, 2), (3, 0), (99, 0)] {
            save_to_memory(fake.mixer + ANIM_INDEX, 77u32);
            set_set(fake.mixer, requested, 5);
            assert_eq!(get_from_memory::<u32>(fake.mixer + ANIM_INDEX), expected, "requested {requested}");
            assert_eq!(get_from_memory::<u8>(fake.mixer + SET_FLAG), 5);
        }
        // No current set: the index stays 0 whatever is asked for.
        save_to_memory(fake.mixer + SET_INDEX, 9u32);
        set_set(fake.mixer, 1, 0);
        assert_eq!(get_from_memory::<u32>(fake.mixer + ANIM_INDEX), 0);
    }

    #[test]
    fn get_anim_returns_a_loaded_animation_without_acquiring() {
        let fake = build_fake_mixer(1, &[(3, 0, true), (3, 0, true)], 1);
        save_to_memory(fake.mixer + ANIM_INDEX, 1u32);
        let mut ops = Recorder::new();
        assert_eq!(get_anim_with(fake.mixer, &mut ops), fake.anim_base + 0x1c);
        assert!(ops.acquired.is_empty());
    }

    #[test]
    fn get_anim_is_null_for_a_missing_set_or_index() {
        let fake = build_fake_mixer(1, &[(3, 0, true)], 0);
        let mut ops = Recorder::new();
        save_to_memory(fake.mixer + ANIM_INDEX, 1u32);
        assert_eq!(get_anim_with(fake.mixer, &mut ops), 0);
        save_to_memory(fake.mixer + ANIM_INDEX, 0u32);
        save_to_memory(fake.mixer + SET_INDEX, 3u32);
        assert_eq!(get_anim_with(fake.mixer, &mut ops), 0);
        assert!(ops.acquired.is_empty());
    }

    #[test]
    fn get_anim_acquires_when_the_frame_count_matches_the_count_byte_or_data_is_missing() {
        // Frame count (3) equals the count byte: acquire runs even though the data record is complete.
        let fake = build_fake_mixer(1, &[(3, 3, true)], 0);
        let mut ops = Recorder::new();
        assert_eq!(get_anim_with(fake.mixer, &mut ops), fake.anim_base);
        assert_eq!(ops.acquired, [(fake.anim_base, 0x1008)]);

        // No data record: acquire gets a null name.
        let missing = build_fake_mixer(1, &[(3, 0, false)], 0);
        let mut ops = Recorder::new();
        get_anim_with(missing.mixer, &mut ops);
        assert_eq!(ops.acquired, [(missing.anim_base, 0)]);

        // An empty word in the data record also forces an acquire.
        let partial = build_fake_mixer(1, &[(3, 0, true)], 0);
        save_to_memory(partial.data + 0x0c, 0u32);
        let mut ops = Recorder::new();
        get_anim_with(partial.mixer, &mut ops);
        assert_eq!(ops.acquired.len(), 1);
    }

    #[test]
    fn get_anim_adds_the_animation_size_to_the_cache_total_for_a_flagged_set() {
        let fake = build_fake_mixer(1, &[(3, 3, true)], 1);
        let mut ops = Recorder::new();
        get_anim_with(fake.mixer, &mut ops);
        assert_eq!(get_from_memory::<u32>(ops.cache + CACHE_TOTAL), 10 + 4096);
    }

    #[test]
    fn update_leaves_a_finished_mixer_alone() {
        let fake = build_fake_mixer(1, &[(4, 0, true)], 0);
        set_duration(&fake, 0, 10);
        save_to_memory(fake.mixer + MIXER_FINISHED, 1u8);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 50, false);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_A), 0);
        assert_eq!(ops.advanced, 0);
    }

    #[test]
    fn update_consumes_whole_frames_and_keeps_the_remainder() {
        let fake = build_fake_mixer(1, &[(4, 0, true)], 0);
        set_duration(&fake, 0, 10);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 25, false);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_A), 5);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_C), 2);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_B), 25);
    }

    #[test]
    fn update_with_stop_at_end_freezes_on_the_last_frame() {
        let fake = build_fake_mixer(1, &[(2, 0, true)], 0);
        set_duration(&fake, 0, 10);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 25, true);
        assert_eq!(get_from_memory::<u8>(fake.mixer + MIXER_FINISHED), 1);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_A), 0);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_C), 1);
        assert_eq!(ops.advanced, 0);
    }

    #[test]
    fn update_moves_on_to_the_next_set_at_the_end_of_an_animation() {
        let fake = build_fake_mixer(2, &[(2, 0, true)], 0);
        set_duration(&fake, 0, 10);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 25, false);
        // Two frames use 20; the second set starts with 5 left, below one frame's 10.
        assert_eq!(get_from_memory::<u32>(fake.mixer + SET_INDEX), 1);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_C), 0);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_A), 5);
        assert_eq!(ops.advanced, 0);
    }

    #[test]
    fn update_past_the_last_set_advances_the_set() {
        let fake = build_fake_mixer(1, &[(2, 0, true)], 0);
        set_duration(&fake, 0, 10);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 25, false);
        assert_eq!(ops.advanced, 1);
        assert_eq!(get_from_memory::<u32>(fake.mixer + SET_INDEX), 1);
        // An out-of-range set index advances at once.
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 1, false);
        assert_eq!(ops.advanced, 1);
    }

    #[test]
    fn update_skips_whole_animations_the_remaining_time_covers() {
        // Three sets of one two-frame animation (duration 10): 70 covers set 0 (20), set 1 (20), set 2 (20) and runs
        // out of sets.
        let fake = build_fake_mixer(3, &[(2, 0, true)], 0);
        set_duration(&fake, 0, 10);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 70, false);
        assert_eq!(ops.advanced, 1);
        assert_eq!(get_from_memory::<u32>(fake.mixer + SET_INDEX), 3);
    }

    #[test]
    fn update_releases_an_entrys_chain_only_for_a_non_zero_step() {
        let fake = build_fake_mixer(1, &[(4, 0, true)], 0);
        set_duration(&fake, 0, 10);
        let entry = get_from_memory::<u32>(fake.mixer);
        save_to_memory(entry + 4, 0xabcdu32);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 0, false);
        assert!(ops.released.is_empty());
        assert_eq!(get_from_memory::<u32>(entry + 4), 0xabcd);
        update_with(&mut ops, fake.mixer, 1, false);
        assert_eq!(ops.released, [(0xabcd, fake.mixer + MIXER_CHAIN_ARG)]);
        assert_eq!(get_from_memory::<u32>(entry + 4), 0);
    }

    #[test]
    fn update_holds_the_last_frame_when_flagged() {
        let fake = build_fake_mixer(1, &[(1, 0, true)], 0);
        set_duration(&fake, 0, 10);
        save_to_memory(fake.mixer + MIXER_HOLD_LAST, 1u8);
        let mut ops = Recorder::new();
        update_with(&mut ops, fake.mixer, 50, false);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_A), 0);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_B), 0);
    }

    #[test]
    fn update_wraps_the_second_clock_past_the_total_time() {
        let fake = build_fake_mixer(1, &[(4, 0, true)], 0);
        set_duration(&fake, 0, 10);
        let mut ops = Recorder::new();
        ops.total = 30;
        update_with(&mut ops, fake.mixer, 40, false);
        assert_eq!(get_from_memory::<u32>(fake.mixer + TIME_B), 10);
    }

    #[test]
    fn clear_all_releases_both_link_arrays_and_keeps_their_buffers() {
        // The deque clear is a vanilla call, so only the link release is exercised here.
        let fake = build_fake_mixer(2, &[(2, 0, true)], 0);
        let counts = Box::leak(vec![5u32; 16].into_boxed_slice()).as_ptr() as u32;
        let links = Box::leak(vec![0u32; 6].into_boxed_slice()).as_ptr() as u32;
        save_to_memory(links, counts);
        save_to_memory(links + 12, counts);
        save_to_memory(fake.mixer + MIXER_LINKS_B, links);
        save_to_memory(fake.mixer + MIXER_LINKS_B + 4, links + 24);
        release_links(fake.mixer, MIXER_LINKS_B);
        assert_eq!(get_from_memory::<u32>(counts + LINK_REFCOUNT), 3, "two links released one reference each");
        assert_eq!(get_from_memory::<u32>(fake.mixer + MIXER_LINKS_B + 4), links);
        assert_eq!(get_from_memory::<u32>(fake.mixer + MIXER_LINKS_B), links);
    }
}
