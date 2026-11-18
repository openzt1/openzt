//! `GXMixer`'s read-side methods: `getAnimSet` (`0x4010e7`), `setSet` (`0x40165f`), `setTime` (`0x4340e1`) and
//! `getAnim` (`0x411fed`).
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

use std::ffi::c_void;

use openzt_detour::generated::{
    gxlleanim::ACQUIRE as GXLLEANIM_ACQUIRE,
    gxmixer::{GET_ANIM, GET_ANIM_SET, SET_SET, SET_TIME, UPDATE},
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

/// `GXMixer::setTime(time, flag)`: clears the time state and runs `update`.
pub(crate) unsafe fn set_time(mixer: u32, time: i32, flag: i8) {
    for offset in [TIME_A, TIME_B, TIME_C] {
        save_to_memory(mixer + offset, 0u32);
    }
    unsafe { UPDATE.original()(mixer as *const u32, time, flag) };
}

/// `GXMixer::getAnim`: the current animation of the current set, or null. When the animation is not loaded (no
/// data record, an empty record word, or its frame count equals the count byte) `acquire(anim, name)` runs
/// first, and a set flagged `+0x24` adds the animation's size to the cache's running total. `cache` supplies the
/// cache object only if that total is touched.
pub(crate) fn get_anim_with(mixer: u32, acquire: impl FnOnce(u32, u32), cache: impl FnOnce() -> u32) -> u32 {
    let set = get_anim_set(mixer);
    if set == 0 {
        return 0;
    }
    let index = get_from_memory::<u32>(mixer + ANIM_INDEX);
    let anims_begin = get_from_memory::<u32>(set + SET_ANIMS_BEGIN);
    if index >= element_count(anims_begin, get_from_memory::<u32>(set + SET_ANIMS_END), ANIM_SIZE) {
        return 0;
    }
    let anim = anims_begin + index * ANIM_SIZE;
    let frames = (get_from_memory::<u32>(anim + ANIM_FRAMES_END).wrapping_sub(get_from_memory::<u32>(anim + ANIM_FRAMES_BEGIN)) as i32) >> 2;
    let counted = i32::from(get_from_memory::<u8>(anim + ANIM_COUNT_BYTE));
    let data = get_from_memory::<u32>(anim + ANIM_DATA);
    let loaded = frames != counted && data != 0 && DATA_WORDS.iter().all(|offset| get_from_memory::<u32>(data + offset) != 0);
    if !loaded {
        let name = if data == 0 { 0 } else { get_from_memory::<u32>(data + DATA_NAME) };
        acquire(anim, name);
        if get_from_memory::<u8>(set + SET_COUNTS_SIZE_FLAG) != 0 {
            let data = get_from_memory::<u32>(anim + ANIM_DATA);
            let size = if data == 0 { 0 } else { get_from_memory::<u32>(data + DATA_SIZE) };
            let total = cache() + CACHE_TOTAL;
            save_to_memory(total, get_from_memory::<u32>(total).wrapping_add(size));
        }
    }
    anim
}

pub(crate) unsafe fn get_anim(mixer: u32) -> u32 {
    get_anim_with(
        mixer,
        |anim, name| unsafe { GXLLEANIM_ACQUIRE.original()(anim as *const u32, name as *const i8) },
        || get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_ANIM_CACHE),
    )
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
    use super::fakes::build_fake_mixer;
    use super::*;
    use std::cell::{Cell, RefCell};

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
        let acquired = Cell::new(false);
        let anim = get_anim_with(fake.mixer, |_, _| acquired.set(true), || panic!("cache is only touched after an acquire"));
        assert_eq!(anim, fake.anim_base + 0x1c);
        assert!(!acquired.get());
    }

    #[test]
    fn get_anim_is_null_for_a_missing_set_or_index() {
        let fake = build_fake_mixer(1, &[(3, 0, true)], 0);
        save_to_memory(fake.mixer + ANIM_INDEX, 1u32);
        assert_eq!(get_anim_with(fake.mixer, |_, _| panic!(), || 0), 0);
        save_to_memory(fake.mixer + ANIM_INDEX, 0u32);
        save_to_memory(fake.mixer + SET_INDEX, 3u32);
        assert_eq!(get_anim_with(fake.mixer, |_, _| panic!(), || 0), 0);
    }

    #[test]
    fn get_anim_acquires_when_the_frame_count_matches_the_count_byte_or_data_is_missing() {
        // Frame count (3) equals the count byte: acquire runs even though the data record is complete.
        let fake = build_fake_mixer(1, &[(3, 3, true)], 0);
        let calls = RefCell::new(Vec::new());
        let anim = get_anim_with(fake.mixer, |anim, name| calls.borrow_mut().push((anim, name)), || panic!("size flag clear"));
        assert_eq!(anim, fake.anim_base);
        assert_eq!(*calls.borrow(), [(fake.anim_base, 0x1008)]);

        // No data record: acquire gets a null name.
        let missing = build_fake_mixer(1, &[(3, 0, false)], 0);
        let calls = RefCell::new(Vec::new());
        get_anim_with(missing.mixer, |anim, name| calls.borrow_mut().push((anim, name)), || panic!());
        assert_eq!(*calls.borrow(), [(missing.anim_base, 0)]);

        // An empty word in the data record also forces an acquire.
        let partial = build_fake_mixer(1, &[(3, 0, true)], 0);
        save_to_memory(partial.data + 0x0c, 0u32);
        let calls = RefCell::new(Vec::new());
        get_anim_with(partial.mixer, |anim, name| calls.borrow_mut().push((anim, name)), || panic!());
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn get_anim_adds_the_animation_size_to_the_cache_total_for_a_flagged_set() {
        let fake = build_fake_mixer(1, &[(3, 3, true)], 1);
        let cache = Box::leak(vec![10u32; 8].into_boxed_slice()).as_ptr() as u32;
        get_anim_with(fake.mixer, |_, _| {}, || cache);
        assert_eq!(get_from_memory::<u32>(cache + CACHE_TOTAL), 10 + 4096);
    }
}
