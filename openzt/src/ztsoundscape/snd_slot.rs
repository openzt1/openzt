use crate::util::get_from_memory;

/// `SNDSound`'s real vtable VA (`private/docs/vtables/SNDSound.md`), written into each of the three
/// embedded `SNDSound` slots by [`ZTSoundscape::construct`], mirroring vanilla's own ctor writes. A raw
/// constant, not RVA'd, matching `ztgamemgr_menumusichandler.rs`'s own `SNDSOUND_VTABLE` precedent -
/// re-declared per-file (no shared consts module); zoo.exe has no ASLR and always loads at its
/// preferred base, so a vtable's Ghidra VA already equals its runtime VA.
pub const SNDSOUND_VTABLE: u32 = 0x00630bc0;

/// `SNDSoundBase`'s real vtable VA (`private/docs/vtables/SNDSoundBase.md`) - `SNDSound`'s own base
/// class. The destructor's terminal state: each embedded slot's `vtable` holds this once
/// [`ZTSoundscape::destruct`]'s swapdown (or the vanilla dtor's) has run. Same raw-constant shape as
/// [`SNDSOUND_VTABLE`].
pub const SNDSOUNDBASE_VTABLE: u32 = 0x00635268;

/// The start block's one-shot fade attenuation (`PUSH 0x1194` before the `SET_FADE_ATTENUATION`
/// vtable call, `ZTSoundscape_update.asm` start block) - equal to the `DAT_00635428` constant's
/// value, i.e. the incoming loop starts at the fully-faded-out end of the ramp.
pub const START_FADE_ATTEN: i32 = 0x1194;

/// One embedded `SNDSound` member, `{vtable, inner}` exactly as vanilla's ctor writes it. `vtable` is
/// `SNDSound`'s while live and `SNDSoundBase`'s (`0x00635268`) once the destructor has run;
/// `inner` is a vanilla-owned inner sound resource handle that never travels through this port.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SndSlot {
    pub vtable: u32, // SNDSound's vtable (0x00630bc0) while live; SNDSoundBase's (0x00635268) after the destructor runs
    pub inner: u32,  // vanilla-owned inner sound resource handle; never read or written by the port
}

const _: () = assert!(std::mem::size_of::<SndSlot>() == 0x8);

/// `destruct`'s per-slot swapdown, one embedded `SNDSound` slot at a time. The inner object's
/// concrete class is whatever `attempt` installed - unknowable statically - so the release
/// dispatches dynamically through the inner's own vtable slot 0 with `bDelete = 1`, vanilla's
/// `if (obj[1] != 0) (*(code*)**obj[1])(1); obj[1] = 0;` idiom (SNDSound.md's last paragraph; same
/// live-vtable call-through shape as `ztguest.rs`'s `entity_category_id`), then the slot's vtable
/// lands on [`SNDSOUNDBASE_VTABLE`] - `SNDSound`'s base class, the terminal state the destructor
/// leaves every embedded slot in.
pub fn destruct_slot(slot: &mut SndSlot) {
    if slot.inner != 0 {
        let inner = slot.inner;
        let deleting_dtor: unsafe extern "thiscall" fn(*const u32, u32) =
            unsafe { std::mem::transmute(get_from_memory::<u32>(get_from_memory::<u32>(inner))) };
        unsafe { deleting_dtor(inner as *const u32, 1) };
        slot.inner = 0;
    }
    slot.vtable = SNDSOUNDBASE_VTABLE;
}

/// `update` step 6's fade-scalar advance for one tick: promotes a `0` delta to `1` for this advance
/// only (asm `.1cd227`, so a zero-delta tick still creeps the ramp), then moves `fade` toward the
/// `fade_step_in != 0` (rising) or `== 0` (falling) endpoint by `delta` in dword arithmetic, clamped
/// to `0..=10000` (`JS` and `CMP 0x2710`/`JG` in the `.asm`).
pub fn advance_fade(fade: i32, fade_step_in: u8, delta: i32) -> i32 {
    let delta = if delta == 0 { 1 } else { delta };
    let advanced = fade.wrapping_add(if fade_step_in != 0 { delta } else { delta.wrapping_neg() });
    advanced.clamp(0, 10000)
}

/// `update` step 6's slot-A per-tick fade attenuation: `trunc(fade * c1 * c2)` where `c1` is
/// `DAT_0063542c` (~0.0001) and `c2` is `DAT_00635428` (4500.0), passed in as runtime-read arguments.
///
/// Precision parity with the x87 sequence (asm `.2a960`): `FILD`/`FMUL` multiply in 80-bit and `FSTP`
/// stores the intermediate as **f32** (round-to-nearest); the second multiply runs in 80-bit off that
/// stored f32 and `FISTP` (under the `OR AH,0xc` truncate control word) truncates **once**, at the end.
/// Every product here (a ≤14-bit `fade` times f32 mantissas) fits f64's 53-bit mantissa exactly, so
/// computing the intermediates in `f64` and truncating with `as i32` reproduces the 80-bit sequence
/// bit-for-bit. An all-`f32` chain double-rounds and crosses integer boundaries x87 wouldn't - e.g.
/// `fade = 100` yields 44 this way but 45 through all-f32 (pinned by the unit tests).
pub fn fade_atten_a(fade: i32, c1: f32, c2: f32) -> i32 {
    let t = (fade as f64 * c1 as f64) as f32;
    (t as f64 * c2 as f64) as i32
}

/// `update` step 6's slot-B per-tick fade attenuation: `trunc((c3 - fade * c1) * c2)` where `c3` is
/// `DAT_00635490` (1.0) - slot B's complement of slot A's ramp, sharing `t`'s f32 intermediate (asm
/// `.2aa0a` reads the stored `[ESP+0x34]`, it does not recompute). Same precision contract as
/// [`fade_atten_a`]; the two are complementary within the truncation pair (sum 4499 mid-ramp, exactly
/// 4500 at both endpoints - `fade = 60` yields 4472 this way but 4473 through all-f32).
pub fn fade_atten_b(fade: i32, c1: f32, c2: f32, c3: f32) -> i32 {
    let t = (fade as f64 * c1 as f64) as f32;
    ((c3 as f64 - t as f64) * c2 as f64) as i32
}
