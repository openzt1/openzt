//! Animation-link vectors and the repeat/fill helpers the play family shares.

use super::*;

/// `GXMixerLink(anim, flag, sound)`: `{anim, sound, flag}`, taking a reference on the animation and stamping it
/// with the animation cache's current counter.
pub(super) fn make_link(anim: u32, flag: u8, sound: u32) -> [u32; 3] {
    save_to_memory(anim + ANIM_REFCOUNT_OFFSET, get_from_memory::<u32>(anim + ANIM_REFCOUNT_OFFSET).wrapping_add(1));
    let cache = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_ANIM_CACHE);
    save_to_memory(anim + ANIM_STAMP_OFFSET, get_from_memory::<u32>(cache + ANIM_CACHE_STAMP_OFFSET));
    [anim, sound, u32::from(flag)]
}

pub(super) fn release_anim(anim: u32) {
    if anim != 0 {
        save_to_memory(anim + ANIM_REFCOUNT_OFFSET, get_from_memory::<u32>(anim + ANIM_REFCOUNT_OFFSET).wrapping_sub(1));
    }
}

/// A `vector<GXMixerLink>` for vanilla's `fPlay`/`GXMixer::setBase`, which only read its `begin`/`end` and
/// copy the links out. The links live in a Rust buffer; `header` is the `{begin, end, cap}` triple vanilla
/// reads, refreshed after every push. Each stored link holds one animation reference (vanilla's copy
/// constructor does the same), released on drop.
pub(super) struct LinkVector {
    pub(super) header: [u32; 3],
    pub(super) links: Vec<[u32; 3]>,
}

impl LinkVector {
    pub(super) fn empty() -> Self {
        let mut vector = LinkVector { header: [0; 3], links: Vec::new() };
        vector.refresh_header();
        vector
    }

    pub(super) fn with(link: &[u32; 3]) -> Self {
        let mut vector = Self::empty();
        vector.push(link);
        vector
    }

    pub(super) fn refresh_header(&mut self) {
        let begin = self.links.as_ptr() as u32;
        self.header = [begin, begin + self.links.len() as u32 * LINK_SIZE, begin + self.links.capacity() as u32 * LINK_SIZE];
    }

    pub(super) fn ptr(&self) -> u32 {
        self.header.as_ptr() as u32
    }

    /// Appends a copy of `link`: a new animation reference and the cache stamp, as vanilla's link copy does.
    pub(super) fn push(&mut self, link: &[u32; 3]) {
        self.links.push(make_link(link[0], link[2] as u8, link[1]));
        self.refresh_header();
    }

    pub(super) fn is_empty(&self) -> bool {
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
pub(super) enum LinkKind {
    Main,
    Tail,
}

/// The links appended after the first to fill `total` milliseconds with an animation of `duration`. Starts from
/// `total - duration` (minus another `duration` for ping-pong) and appends a main link, then a tail when
/// ping-pong, subtracting `duration` for each, while the remainder is positive. `stop_after_one_on_zero`
/// ends the sequence after one main link when `duration` is zero (`fMove`, `fPlayTime`); without it a zero
/// duration never ends (`fPlaySpecial`, as in vanilla), so callers that do not trust the duration must bound
/// the iteration.
pub(super) fn timed_repeat(total: i32, duration: i32, ping_pong: bool, stop_after_one_on_zero: bool) -> impl Iterator<Item = LinkKind> {
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
pub(super) fn counted_repeat(count: i32, ping_pong: bool) -> impl Iterator<Item = LinkKind> {
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

pub(super) unsafe fn play_single_vanilla(unit: u32, anim: u32, flag: u8, sound: u32) {
    unsafe { play_single_with(&VANILLA_MIXER, unit, anim, flag, sound) }
}

#[allow(clippy::too_many_arguments)]
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

pub(super) unsafe fn play_pair_vanilla(this: u32, unit: u32, anim_first: u32, anim_second: u32, flag: u8, sound: u32, seconds: i32) -> bool {
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
