//! `ZTSoundscape` reimplementation - the game's crowd/world ambient-audio crossfade state machine, one
//! of `ZTGameMgr`'s pointed-to sub-object classes: `ztgamemgr.rs` holds it as `soundscape_ptr`
//! (`this+0x1190`, explicitly zeroed by `CreateZTGameMgr`) and drives it through this port (`start`
//! allocates + constructs + `init`s it, `update_sim` calls `update`, `stop` runs the destructor + free).
//! This file is the full in-scope port (the two `#[repr(C)]` structs, the pure-write constructor, the
//! [`ZTSoundscape::init`]/[`ZTSoundscape::update`]/[`ZTSoundscape::destruct`] ports, verified live by
//! the `ZTSOUNDSCAPE_*` battery tests) plus the [`soundscape_detours`] block hooking the class's three
//! hooked entries (`CONSTRUCTOR`/`INIT`/`UPDATE`),
//! so any caller reaching those addresses runs the Rust code - which is also why `ztgamemgr.rs`'s own
//! three call sites call the Rust methods directly rather than through the addresses. The destructor
//! entry (`ztsoundscape::DESTRUCTOR`) stays deliberately un-detoured (see below), so `stop`'s
//! call-through to it stays `.original()` - the same teardown also exists as the Rust
//! [`ZTSoundscape::destruct`] for ported callers, while the address keeps serving vanilla ones.
//!
//! The class has no vtable of its own (confirmed - every vtable dispatch inside its decompiled bodies
//! belongs to an *embedded* `SNDSound` member or a pointed-to `Ambients` object). Its whole method
//! surface is 4 fixed addresses, all already `generated.rs` `ztsoundscape::*` entries: constructor
//! `0x00592596`, `init` `0x005922fd`, `update` `0x004352dd`, and the destructor `0x005003e2`
//! (`ztsoundscape::DESTRUCTOR`, confirmed via `ZTSoundscape_~ZTSoundscape.meta`'s matching address;
//! `ztgamemgr.rs` imports it aliased as `ZTSOUNDSCAPE_DESTRUCTOR`).
//!
//! Scope boundary: `ambients`/`ambientsgroup` (`openzt/src/ambients.rs`,
//! `openzt/plans/ztsoundscape-ambients-full-port-plan.md`) are the full Rust reimplementation and
//! detoured - [`ZTSoundscape::init`]/[`ZTSoundscape::update`] call their Rust methods directly
//! (reimplemented-to-reimplemented) rather than through `generated.rs` addresses, and the
//! shared-ownership consumers (`ZTViewingArea`/`ZTHabitat`, which each hold their own `Ambients*` and
//! call `Ambients::play` on it independently) pick up the Rust implementation automatically once
//! `ambients.rs`'s detours are installed, without any changes of their own. `sndsound`/`bfsndmgr` stay
//! out of scope: all sound-device interaction still runs through the embedded slots' vtables as real
//! vanilla dispatch.
//!
//! Vanilla-layout-compatible (style 1, per `CLAUDE.md`'s two-style split), forced by the teardown:
//! the teardown walks the raw fields - stopping the world sound and both crowd slots through the
//! embedded objects' vtables, running `~Ambients` (the Rust `Ambients::destruct` reimplementation in
//! `ambients.rs`; the vanilla address routes there through that module's `ambients::DESTRUCTOR` detour,
//! the port calls the method directly) + vanilla `operator delete` on both `Ambients` blocks (behind
//! vanilla's own both-non-null pairing quirk, preserved by not touching it), and swapping the embedded
//! slots' vtables down to `SNDSoundBase`'s - so the fields must at all times hold vanilla-shaped
//! content, which falls out naturally as long as the port constructs everything exactly the way
//! vanilla's own ctor/`init` do. The teardown is ported in full as [`ZTSoundscape::destruct`], while
//! its `generated.rs` entry (`ztsoundscape::DESTRUCTOR`) stays deliberately un-detoured (straight-line
//! teardown, no decision logic) and is still what both `ztgamemgr.rs`'s `stop` and un-ported vanilla
//! `~ZTGameMgr` run.
//!
//! Cross-allocator contract (per `CLAUDE.md`): the `0x54` block and both `0x18` `Ambients` blocks are
//! vanilla `OPERATOR_NEW`/`operator delete` on both sides - the `0x54` block is allocated by
//! `ztgamemgr.rs`'s `start` and freed by `stop` via vanilla `OPERATOR_DELETE` (unchanged by this port),
//! and the `init` port allocates the two `Ambients` blocks through the same vanilla `OPERATOR_NEW`
//! for the destructor to free (either pole - the vanilla address or [`ZTSoundscape::destruct`]). Never
//! a Rust `Box` on vanilla-owned memory.
//!
//! Shared-RNG constraint: `update`'s position jitter
//! advances the real global RNG state at VA `0x00638060` through the classic MSVC LCG
//! (`state = state * 0x343fd + 0x269ec3`), 4 chained advances per `Ambients` object, crowd first. The
//! port reads/advances/writes **that exact address** in vanilla's exact advance order - never a
//! Rust-side RNG, or every other vanilla consumer's stream (`BFAIMgr_fRandomWalk`, `ZooStatus_fChance`,
//! `ZTAnimal_*`, `SoundGroup_play`, ...) desyncs.

/// `SNDSound`'s real vtable VA (`private/docs/vtables/SNDSound.md`), written into each of the three
/// embedded `SNDSound` slots by [`ZTSoundscape::construct`], mirroring vanilla's own ctor writes. A raw
/// constant, not RVA'd, matching `ztgamemgr_menumusichandler.rs`'s own `SNDSOUND_VTABLE` precedent -
/// re-declared per-file (no shared consts module); zoo.exe has no ASLR and always loads at its
/// preferred base, so a vtable's Ghidra VA already equals its runtime VA.
const SNDSOUND_VTABLE: u32 = 0x00630bc0;

/// `SNDSoundBase`'s real vtable VA (`private/docs/vtables/SNDSoundBase.md`) - `SNDSound`'s own base
/// class. The destructor's terminal state: each embedded slot's `vtable` holds this once
/// [`ZTSoundscape::destruct`]'s swapdown (or the vanilla dtor's) has run. Same raw-constant shape as
/// [`SNDSOUND_VTABLE`].
const SNDSOUNDBASE_VTABLE: u32 = 0x00635268;

use std::ffi::c_void;

use crate::ambients::Ambients;
use crate::globals::{get_module_base, globals};
use crate::util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory};
use openzt_detour::generated::bfconfigfile::{ATTEMPT_0, GET_INT, GET_STRING_1, RELEASE};
use openzt_detour::generated::bfsndmgr::GET_SCREEN_CENTER;
use openzt_detour::generated::sndsound::{
    ATTEMPT as SNDSOUND_ATTEMPT, PLAY_LOOPED_1 as SNDSOUND_PLAY_LOOPED_1, RELEASE as SNDSOUND_RELEASE,
    SET_BASE_ATTENUATION as SNDSOUND_SET_BASE_ATTENUATION,
    SET_FADE_ATTENUATION as SNDSOUND_SET_FADE_ATTENUATION, SET_VOLUME as SNDSOUND_SET_VOLUME,
    STOP as SNDSOUND_STOP, VALID as SNDSOUND_VALID,
};
use openzt_detour::generated::standalone::{OPERATOR_DELETE, OPERATOR_NEW};
use openzt_detour::generated::ztsoundscape::{CONSTRUCTOR, INIT, UPDATE};
use openzt_detour_macro::detour_mod;
use tracing::error;

/// Data globals/`.rdata` literals `init` touches, all re-declared per-file after [`SNDSOUND_VTABLE`]
/// (no shared consts module). Unlike a vtable VA, **data** addresses are not identity-mapped: each is
/// stored as its Ghidra VA minus the preferred base, then resolved at runtime as
/// `get_module_base("zoo.exe") + RVA` (same shape as `ztgamemgr.rs`'s `GLOBAL_ZTSCENARIOMGR_RVA`).
/// Sound-device singleton: a global *pointer* (`MOV %ECX, dword ptr GLOBAL_DX8SndMgr` in
/// `ZTSoundscape_init.asm:141`), so its *value* is read, not the address itself.
const GLOBAL_DX8SNDMGR_RVA: u32 = 0x006380a8 - 0x400000;
/// The two live `BFConfigFile` instances are **inline objects** at these addresses, not pointers to
/// them - `MOV %ECX, DAT_00641850` with no `dword ptr` qualifier (`ZTSoundscape_init.asm:41`/`:61`):
/// the resolved address itself is passed as `this`.
const CROWD_CONFIG_INSTANCE_RVA: u32 = 0x00641850 - 0x400000;
const WORLD_CONFIG_INSTANCE_RVA: u32 = 0x00641840 - 0x400000;
/// Per-crowd-level default `.wav` filenames (quiet/small/medium/large, matching `crowd_filename`'s
/// indices), the `.rdata` C-string addresses vanilla stores on the init-defaults path.
const DEFAULT_CROWD_FILENAME_RVAS: [u32; 4] = [
    0x0064194c - 0x400000, // "sounds/quiet.wav"
    0x00641938 - 0x400000, // "sounds/crowds.wav"
    0x00641924 - 0x400000, // "sounds/crowdm.wav"
    0x00641910 - 0x400000, // "sounds/crowdl.wav"
];
/// Per-crowd-level `(filename_key, atten_key)` pairs within the crowd config section. All section/key
/// literals are all-lowercase in the binary (`ZTSoundscape_init.asm` label spellings); the raw VAs pass
/// through unchanged, so spelling only matters for these comments.
const CROWD_KEY_RVAS: [(u32, u32); 4] = [
    (0x006419ac - 0x400000, 0x00641994 - 0x400000), // "quiet" / "quietatten"
    (0x00638dc4 - 0x400000, 0x00641988 - 0x400000), // "small" / "smallatten"
    (0x00641980 - 0x400000, 0x00641974 - 0x400000), // "medium" / "medatten"
    (0x0064196c - 0x400000, 0x00641960 - 0x400000), // "large" / "largeatten"
];
const CROWD_SECTION_RVA: u32 = 0x006419a0 - 0x400000; // "crowdsound"
const WORLD_SECTION_RVA: u32 = 0x006419b4 - 0x400000; // "worldsound"
const WORLD_NAME_KEY_RVA: u32 = 0x00638d8c - 0x400000; // "name"
const WORLD_ATTEN_KEY_RVA: u32 = 0x006419c0 - 0x400000; // "atten"
/// Default base attenuation vanilla writes into every `crowd_atten` slot (1500). The world path's
/// default is a plain `0`, written inline - it is *not* this constant.
const DEFAULT_CROWD_ATTEN: i32 = 0x5dc;

/// The shared game RNG state (`DAT_00638060`) `update`'s position jitter advances - a raw dword LCG
/// state shared with a long list of un-ported vanilla consumers (see the module doc's shared-RNG
/// constraint). Read/advanced/written via `base + RVA`, exactly like the config-instance addresses.
const GAME_RNG_RVA: u32 = 0x00638060 - 0x400000;
/// The three fade constants `update` reads as `f32` at runtime through data RVAs (the
/// binary-confirmed values below are what the unit tests pass as arguments). Read from zoo.exe's
/// `.rdata` via a PE-section parse:
/// - `DAT_0063542c` = `9.999999747378752e-05` (f32 `0x38D1B717`, the f32 nearest to 0.0001) - the
///   fade-scalar scale.
/// - `DAT_00635428` = `4500.0` - the attenuation range, equal to the start block's `0x1194` push.
/// - `DAT_00635490` = `1.0` - the complement base (slot B's formula reads `1.0 - fade*c1`).
///
/// (`_DAT_00635420` is MenuMusicHandler's own f64 `0.5` constant - its low dword alone reads as 0,
/// which is what a 4-byte PE peek at that address shows.)
const DAT_00635428_RVA: u32 = 0x00635428 - 0x400000;
const DAT_0063542C_RVA: u32 = 0x0063542c - 0x400000;
const DAT_00635490_RVA: u32 = 0x00635490 - 0x400000;
/// The start block's one-shot fade attenuation (`PUSH 0x1194` before the `SET_FADE_ATTENUATION`
/// vtable call, `ZTSoundscape_update.asm` start block) - equal to the `DAT_00635428` constant's
/// value, i.e. the incoming loop starts at the fully-faded-out end of the ramp.
const START_FADE_ATTEN: i32 = 0x1194;

/// One embedded `SNDSound` member, `{vtable, inner}` exactly as vanilla's ctor writes it. `vtable` is
/// `SNDSound`'s while live and `SNDSoundBase`'s (`0x00635268`) once the destructor has run;
/// `inner` is a vanilla-owned inner sound resource handle that never travels through this port.
#[repr(C)]
struct SndSlot {
    vtable: u32, // SNDSound's vtable (0x00630bc0) while live; SNDSoundBase's (0x00635268) after the destructor runs
    inner: u32,  // vanilla-owned inner sound resource handle; never read or written by the port
}

const _: () = assert!(std::mem::size_of::<SndSlot>() == 0x8);

/// Real allocation size `0x54`, confirmed directly by `ZTGameMgr_start.c`'s `operator_new(0x54)` call
/// (not merely inferred from the constructor's own field writes, which stop at offset `0x50`).
#[repr(C)]
pub struct ZTSoundscape {
    current_track: i32,       // 0x00 - crowd track index: -1 = none, else 0..=3 into the crowd tables
    fade: i32,                // 0x04 - crossfade scalar, clamped 0..=10000
    next_slot_is_b: u8,       // 0x08 - which crowd slot the *next* start uses (0 -> crowd_snd_a, != 0 -> crowd_snd_b); toggles per start
    // Crossfade direction (`!= 0` -> `fade` rises per tick, `0` -> falls). Deliberately never written by
    // ctor/`init`, vanilla-faithfully (leftover uninitialized allocator memory, like `bfconfigfile.rs`'s
    // `pad_kind_tag`): it is only ever read while `fading` is set, and every path that sets `fading`
    // writes this first - so the garbage never escapes. Preserve; do not "fix" by zeroing.
    fade_step_in: u8,         // 0x09
    fading: u8,               // 0x0a
    _pad: [u8; 1],            // 0x0b - never referenced by any code path
    crowd_snd_a: SndSlot,     // 0x0c
    crowd_snd_b: SndSlot,     // 0x14
    crowd_filename: [u32; 4], // 0x1c - char* per crowd level 0..=3 (quiet/small/medium/large); init defaults to .rdata ".wav" pointers
    crowd_atten: [i32; 4],    // 0x2c - base attenuation per crowd level; init defaults each to 0x5dc (1500)
    world_snd: SndSlot,       // 0x3c
    world_name: u32,          // 0x44 - char*, 0 = no world sound
    // World sound's base attenuation. Deliberately left uninitialized when no world sound is
    // configured, vanilla-faithfully: `init` writes it only when the config supplied a `world_name`,
    // and the only read sits behind the same `world_name != 0` gate, so the garbage never escapes.
    // Preserve; do not "fix" by zeroing.
    world_atten: i32,         // 0x48
    crowd_ambients: u32,      // 0x4c - Ambients*, 0 = none (ctor-zeroed; init fills via vanilla operator_new(0x18))
    world_ambients: u32,      // 0x50 - Ambients*, 0 = none (same)
}

const _: () = assert!(std::mem::size_of::<ZTSoundscape>() == 0x54);

/// `update` step 2's ambient crowd level as a pure band table over the guest count (`update.asm:32-49`;
/// the Windows `.c` renders the same bands through a goto chain; the macOS export agrees). The
/// `guests < 0` band is unreachable in practice - vanilla reads a *dword* at `ZTGameMgr+0x54`, and the
/// live field is a non-negative `u16` count, so negatives require non-zero pad bytes at `+0x56` - but
/// it is preserved anyway, exactly as vanilla branches (`TEST %EAX, %EAX` / `JL` on the full dword).
fn ambient_level(guests: i32) -> i32 {
    if !(0..100).contains(&guests) {
        90
    } else if guests < 10 {
        -100
    } else if guests < 50 {
        10
    } else {
        40
    }
}

/// `update` step 7's track-selection hysteresis as a pure first-match-wins chain (`ZTSoundscape_update`
/// macOS export's control flow, boundaries cross-checked against the Windows `.asm` constants
/// `0xf`/`0x15`/`0x4b`/`0x55`/`0x56`/`0x96`/`0xa0`/`0xa1`). `None` is the decompile's `bVar7 = false`
/// (no change - the caller skips the whole start block, vanilla-faithfully, so a track stays on its
/// hold band forever at a constant guest count). `current_track` may be `-1` (fresh `init`): every
/// branch reachable with `-1` produces only `0`/`1` - the `g > 20` arm catches `t ∈ {0, -1}` before the
/// deeper `t ∈ {1, -1}`/`t ∈ {2, -1}` arms can fire, so a fresh soundscape never jumps straight to 2/3.
///
/// Net effect (pinned by `tests::select_target_track_matches_the_vanilla_grid`):
/// up-switches `0→1` at `g >= 21`, `1→2` at `g >= 86`, `2→3` at `g >= 161`; down-switches `1..3→0` at
/// `g <= 14`, `2→1` at `g <= 74`, `3→2` at `g <= 149`; hold bands `t=0`: `g <= 20`, `t=1`: `15..=85`,
/// `t=2`: `75..=160`, `t=3`: `g >= 150`; fresh (`t=-1`): `0` at `g <= 14`, `1` at any `g >= 15`.
fn select_target_track(current_track: i32, guests: i32) -> Option<u32> {
    let (t, g) = (current_track, guests);
    if g <= 14 && t != 0 {
        return Some(0);
    }
    if g <= 20 || !(t == 0 || t == -1) {
        if g <= 85 || !(t == 1 || t == -1) {
            if g <= 160 || !(t == 2 || t == -1) {
                if g <= 74 && (t == 2 || t == -1) {
                    return Some(1);
                }
                if g <= 149 && (t == 3 || t == -1) {
                    return Some(2);
                }
                return None;
            }
            return Some(3);
        }
        return Some(2);
    }
    Some(1)
}

/// One MSVC LCG advance over the shared game RNG state: `state = state * 0x343fd + 0x269ec3` with full
/// 32-bit wrap, exactly the `IMUL`/`ADD` dword pair vanilla runs at every `DAT_00638060` touch.
fn lcg_next(state: u32) -> u32 {
    state.wrapping_mul(0x343fd).wrapping_add(0x269ec3)
}

/// `update` step 4's position jitter for one `Ambients` block as a pure function of the RNG seed and
/// the screen-center coordinates (`update.asm:59-160`). Advances the seed exactly 4 times (`r1..r4`)
/// and returns the re-jittered `(x, y, z)` position plus the final state to store back:
/// - the **x** pair is `r3`/`r4` with modulus **400**, the **y** pair is `r1`/`r2` with modulus **300**
///   (the axis/modulus crossover - vanilla samples in `y, y, x, x` order), and `z` passes through.
/// - each sample is `(r >> 16) & 0x7fff`: on a `u32` this equals the asm's `SAR 0x10` + `AND 0x7fff`
///   (the mask clears the bits where the two shifts disagree), and the remainders are plain `%`
///   because the sampled values are `0..=32767` - vanilla's `CDQ`/`IDIV` then only sees non-negatives.
/// - the center coordinates are combined with `wrapping_add` for dword-add parity with vanilla's
///   plain `ADD`.
fn ambients_jitter(seed: u32, x: i32, y: i32, z: i32) -> ([i32; 3], u32) {
    let r1 = lcg_next(seed);
    let r2 = lcg_next(r1);
    let r3 = lcg_next(r2);
    let r4 = lcg_next(r3);
    let sample = |r: u32| ((r >> 16) & 0x7fff) as i32;
    let x_offset = sample(r3) % 400 - sample(r4) % 400;
    let y_offset = sample(r1) % 300 - sample(r2) % 300;
    ([x.wrapping_add(x_offset), y.wrapping_add(y_offset), z], r4)
}

/// `update` step 6's fade-scalar advance for one tick: promotes a `0` delta to `1` for this advance
/// only (asm `.1cd227`, so a zero-delta tick still creeps the ramp), then moves `fade` toward the
/// `fade_step_in != 0` (rising) or `== 0` (falling) endpoint by `delta` in dword arithmetic, clamped
/// to `0..=10000` (`JS` and `CMP 0x2710`/`JG` in the `.asm`).
fn advance_fade(fade: i32, fade_step_in: u8, delta: i32) -> i32 {
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
fn fade_atten_a(fade: i32, c1: f32, c2: f32) -> i32 {
    let t = (fade as f64 * c1 as f64) as f32;
    (t as f64 * c2 as f64) as i32
}

/// `update` step 6's slot-B per-tick fade attenuation: `trunc((c3 - fade * c1) * c2)` where `c3` is
/// `DAT_00635490` (1.0) - slot B's complement of slot A's ramp, sharing `t`'s f32 intermediate (asm
/// `.2aa0a` reads the stored `[ESP+0x34]`, it does not recompute). Same precision contract as
/// [`fade_atten_a`]; the two are complementary within the truncation pair (sum 4499 mid-ramp, exactly
/// 4500 at both endpoints - `fade = 60` yields 4472 this way but 4473 through all-f32).
fn fade_atten_b(fade: i32, c1: f32, c2: f32, c3: f32) -> i32 {
    let t = (fade as f64 * c1 as f64) as f32;
    ((c3 as f64 - t as f64) * c2 as f64) as i32
}

/// `destruct`'s per-slot swapdown, one embedded `SNDSound` slot at a time. The inner object's
/// concrete class is whatever `attempt` installed - unknowable statically - so the release
/// dispatches dynamically through the inner's own vtable slot 0 with `bDelete = 1`, vanilla's
/// `if (obj[1] != 0) (*(code*)**obj[1])(1); obj[1] = 0;` idiom (SNDSound.md's last paragraph; same
/// live-vtable call-through shape as `ztguest.rs`'s `entity_category_id`), then the slot's vtable
/// lands on [`SNDSOUNDBASE_VTABLE`] - `SNDSound`'s base class, the terminal state the destructor
/// leaves every embedded slot in.
fn destruct_slot(slot: &mut SndSlot) {
    if slot.inner != 0 {
        let inner = slot.inner;
        let deleting_dtor: unsafe extern "thiscall" fn(*const u32, u32) =
            unsafe { std::mem::transmute(get_from_memory::<u32>(get_from_memory::<u32>(inner))) };
        unsafe { deleting_dtor(inner as *const u32, 1) };
        slot.inner = 0;
    }
    slot.vtable = SNDSOUNDBASE_VTABLE;
}

impl ZTSoundscape {
    /// Reimplementation of `ZTSoundscape::ZTSoundscape` (`0x00592596`), per
    /// `ZTSoundscape_ZTSoundscape.c`/`.asm`. Pure constant writes, no calls: `{vtable:
    /// SNDSOUND_VTABLE, inner: 0}` into the three embedded `SNDSound` slots and `0` into both
    /// `Ambients` pointers. The scalar state and the filename/atten tables are deliberately left
    /// uninitialized exactly like vanilla (`operator_new` doesn't zero - `init` writes everything the
    /// rest of the class reads), and the ctor's post-assignment `if inner != 0` release idiom is a dead
    /// no-op at construction time (`inner` was just written `0`) and is not reproduced.
    pub fn construct(&mut self) {
        self.crowd_snd_a = SndSlot { vtable: SNDSOUND_VTABLE, inner: 0 };
        self.crowd_snd_b = SndSlot { vtable: SNDSOUND_VTABLE, inner: 0 };
        self.world_snd = SndSlot { vtable: SNDSOUND_VTABLE, inner: 0 };
        self.crowd_ambients = 0;
        self.world_ambients = 0;
    }

    /// One `(crowd_filename[index]`, `crowd_atten[index])` config lookup, shared by all four crowd
    /// levels: real vanilla `BFConfigFile::getString` into the filename slot, and - only if that
    /// succeeded - the vanilla default `0x5dc` into the atten slot followed by a real
    /// `BFConfigFile::getInt` into it (return ignored, vanilla-faithfully). The two Ambients
    /// constructs and the world name/atten pair are deliberately *not* routed through here (the
    /// former are cross-allocator-critical and stay inline in [`ZTSoundscape::init`]; the latter
    /// writes two different fields, so the helper's shape doesn't fit - and the asymmetry is the
    /// point).
    fn get_crowd_config_pair(&mut self, config: *const u32, section: u32, index: usize, key: u32, atten_key: u32) {
        let got = unsafe {
            GET_STRING_1.original()(
                config,
                section as *const u32,
                key as *const u32,
                &raw mut self.crowd_filename[index] as *const u32,
            )
        };
        if got {
            self.crowd_atten[index] = DEFAULT_CROWD_ATTEN;
            unsafe {
                GET_INT.original()(config, section, atten_key, &raw mut self.crowd_atten[index] as *const u32);
            }
        }
    }

    /// Reimplementation of `ZTSoundscape::init` (`0x005922fd`), per `ZTSoundscape_init.asm` - the
    /// Windows `.asm` is ground truth here, the shipped `.c` garbles the three world-sound vtable
    /// calls. Vanilla is void; the four arguments are exactly what `ztgamemgr.rs`'s `start` gets from
    /// its four `BFScenarioMgr` getters (`*const u8`), passed through uncasted.
    ///
    /// Vanilla parity points preserved (each mapped from the `.asm`):
    /// - **Allocation order is load-bearing**: both `Ambients` blocks are allocated via vanilla
    ///   `OPERATOR_NEW(0x18)` + constructed via real vanilla `Ambients::Ambients` *before* either
    ///   `BFConfigFile` is released/re-parsed (`.asm` lines 5-34 land before the first
    ///   `BFConfigFile::release`), so a null allocation stores `0` and skips its ctor while the other
    ///   still runs. Both constructs stay deliberately inline (not a helper): each block vanilla
    ///   allocates here is freed by vanilla's own destructor, so the call shape itself is the
    ///   cross-allocator contract - one duplicated copy is the price of keeping that visible.
    /// - Defaults in vanilla's write order: `world_name = 0`, the four `.rdata` `.wav` pointers into
    ///   `crowd_filename`, `0x5dc` into all four `crowd_atten`.
    /// - Crowd config: unconditional `BFConfigFile::release` on the inline instance at
    ///   `0x00641850`, then a real `BFConfigFile::attempt` on the caller's config name gating all four
    ///   [`get_crowd_config_pair`] lookups.
    /// - World config: same on the instance at `0x00641840`; `world_atten`'s pre-`getInt` default is
    ///   a plain `0` (not `0x5dc`) and is only written when the `name` key parsed - it stays
    ///   uninitialized otherwise, vanilla-faithfully (see the field doc).
    /// - World sound: `GLOBAL_DX8SndMgr` is read unconditionally *before* the `world_name != 0` test
    ///   (vanilla hoists the load) and no null guard is added; the three vtable calls are made
    ///   through the fixed `sndsound` addresses with `&self.world_snd` as the slot `this`.
    /// - Tail in vanilla's write order: `current_track`/`fade`/`fading`/`next_slot_is_b`.
    ///   `fade_step_in` is deliberately untouched (see its field doc).
    pub fn init(
        &mut self,
        crowd_ambients_name: *const u8,
        world_ambients_name: *const u8,
        crowd_config_name: *const u8,
        world_config_name: *const u8,
    ) {
        // zoo.exe has no ASLR and always loads at its preferred base, so base + RVA is stable for the
        // whole process life - the same assumption `MenuMusicHandler::init` makes.
        let base = get_module_base("zoo.exe") as u32;
        let crowd_config = (base + CROWD_CONFIG_INSTANCE_RVA) as *const u32;
        let world_config = (base + WORLD_CONFIG_INSTANCE_RVA) as *const u32;

        unsafe {
            // Both Ambients blocks, crowd then world - allocated + constructed before any config
            // parsing (vanilla order, see this method's doc comment). Vanilla zeroes a 3-dword stack
            // local and hands its address as the ctor's second argument. Constructed via a direct Rust
            // call (reimplemented-to-reimplemented, see the module doc's scope-boundary note) rather
            // than through `generated.rs`'s `ambients::CONSTRUCTOR` address.
            let crowd_block = OPERATOR_NEW.original()(0x18);
            self.crowd_ambients = if crowd_block.is_null() {
                0
            } else {
                let mut ctor_data = [0u32; 3];
                (*(crowd_block as *mut Ambients)).construct(crowd_ambients_name, ctor_data.as_mut_ptr() as *const i32);
                crowd_block as u32
            };

            let world_block = OPERATOR_NEW.original()(0x18);
            self.world_ambients = if world_block.is_null() {
                0
            } else {
                let mut ctor_data = [0u32; 3];
                (*(world_block as *mut Ambients)).construct(world_ambients_name, ctor_data.as_mut_ptr() as *const i32);
                world_block as u32
            };

            // Defaults, in vanilla's write order.
            self.world_name = 0;
            for (slot, &rva) in self.crowd_filename.iter_mut().zip(DEFAULT_CROWD_FILENAME_RVAS.iter()) {
                *slot = base + rva;
            }
            self.crowd_atten = [DEFAULT_CROWD_ATTEN; 4];

            // Crowd config: unconditional release, then attempt gates all four key pairs.
            RELEASE.original()(crowd_config);
            if ATTEMPT_0.original()(crowd_config, crowd_config_name as *const i8) {
                let section = base + CROWD_SECTION_RVA;
                for (index, &(key, atten_key)) in CROWD_KEY_RVAS.iter().enumerate() {
                    self.get_crowd_config_pair(crowd_config, section, index, base + key, base + atten_key);
                }
            }

            // World config: same shape; the name lookup writes world_name, and only on success does
            // the atten lookup run (behind a plain-0 default, per this method's doc comment).
            RELEASE.original()(world_config);
            if ATTEMPT_0.original()(world_config, world_config_name as *const i8) {
                let section = base + WORLD_SECTION_RVA;
                if GET_STRING_1.original()(
                    world_config,
                    section as *const u32,
                    (base + WORLD_NAME_KEY_RVA) as *const u32,
                    &raw mut self.world_name as *const u32,
                ) {
                    self.world_atten = 0;
                    GET_INT.original()(
                        world_config,
                        section,
                        base + WORLD_ATTEN_KEY_RVA,
                        &raw mut self.world_atten as *const u32,
                    );
                }
            }
        }

        // World sound: the sndmgr singleton is loaded unconditionally before the world_name test
        // (vanilla hoists the load; no null guard added), and each call result is masked to the low
        // byte because vanilla tests only AL (`TEST %AL, %AL`). `attempt` is vtable +0x8,
        // `setBaseAttenuation` +0x48, `playLooped` +0x3c - the fixed `sndsound` addresses dispatch
        // identically for the live `SNDSound` vtable the ctor wrote into the slot.
        let world_name = self.world_name;
        let sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        if world_name != 0 {
            let slot = &self.world_snd as *const SndSlot as *const u32;
            unsafe {
                if (SNDSOUND_ATTEMPT.original()(slot, sndmgr as *const u32, world_name as *const i8) & 0xff) != 0 {
                    SNDSOUND_SET_BASE_ATTENUATION.original()(slot, self.world_atten);
                    SNDSOUND_PLAY_LOOPED_1.original()(slot);
                }
            }
        }

        // Tail, in vanilla's write order.
        self.current_track = -1;
        self.fade = 0;
        self.fading = 0;
        self.next_slot_is_b = 0;
    }

    /// Reimplementation of `ZTSoundscape::update` (`0x004352dd`), per `ZTSoundscape_update.asm` (the
    /// Windows `.c` garbles the control flow into a goto chain; the macOS export is the cleaner
    /// reference and agrees on every boundary). Vanilla is void; `delta` is the raw game-tick scalar
    /// `update_sim` passes through. The band table, hysteresis, LCG jitter, and fade math live in the
    /// pure helpers above - this method is the orchestration, in vanilla's exact order:
    ///
    /// 1. Guest count: a raw **i32 dword** read at `GLOBAL_ZTGameMgr_deref + 0x54` - vanilla reads a
    ///    full dword (so the `< 0` band in [`ambient_level`] stays reachable only through non-zero pad
    ///    bytes; preserved anyway), not the typed `u16` field. No null guard, matching vanilla (a
    ///    running game implies a live manager; `update_sim` gates the call).
    /// 2. Ambient level via [`ambient_level`].
    /// 3. Screen center via real vanilla `GET_SCREEN_CENTER` on the `DX8SndMgr` singleton, reading
    ///    `x`/`y`/`z` from the **returned** pointer (vanilla reads `[EAX]`/`[EAX+4]`/`[EAX+8]`), not
    ///    from the caller-supplied out buffer.
    /// 4. Both `Ambients` blocks (crowd first) re-jittered through the **shared game RNG**:
    ///    read `DAT_00638060`, run [`ambients_jitter`], store only the final state back - net-identical
    ///    to vanilla's store-every-intermediate (nothing reads the state in between) - then raw-write
    ///    the three dwords at `ambients+0xc/+0x10/+0x14`, the same fields vanilla's own
    ///    `Ambients::play` reads (style-1 in-place access to the out-of-scope class's memory; safe for
    ///    the `ZTViewingArea`/`ZTHabitat` consumers because the identical fields are written). No null
    ///    guard on either pointer, exactly like vanilla's `MOV ECX, [ESI+0x4c]` / write-through.
    /// 5. Real vanilla `Ambients::play` on both blocks: crowd `(delta, level)`, world `(delta, 0x32)`.
    /// 6. Crossfade block (only if `fading`): advance via [`advance_fade`] (the raw `delta` went to
    ///    step 5; the `0`→`1` promotion is local to this block), then per slot - crowd A (+0xc) with
    ///    [`fade_atten_a`], crowd B (+0x14) with [`fade_atten_b`], each behind its real-vanilla
    ///    `VALID` gate - `SET_FADE_ATTENUATION` + `SET_VOLUME(0)`. Endpoints: `fade == 0` stops slot B
    ///    (`STOP` + `RELEASE` behind `VALID`), `fade == 10000` stops slot A; `fading` clears only on an
    ///    endpoint hit (mid-fade leaves it set). The block falls through either way, so a crossfade
    ///    that completes this tick lets the start block below fire in the same tick.
    /// 7. Track selection via [`select_target_track`], computed **unconditionally** (pure; vanilla
    ///    computes it mid-fade too) - only its *use* is gated.
    /// 8. Start block (`!fading` + a target): slot = `next_slot_is_b == 0` ? crowd A : crowd B; on a
    ///    successful real-vanilla `ATTEMPT(sndmgr, crowd_filename[target])`:
    ///    `SET_BASE_ATTENUATION(crowd_atten[target])`, `SET_FADE_ATTENUATION(0x1194)` (the incoming
    ///    loop starts at its silent end of the ramp until the next fade tick overwrites it),
    ///    `PLAY_LOOPED_1`, then `fading = 1`, `fade_step_in = old next_slot_is_b`,
    ///    `fade = old != 0 ? 0 : 10000`, `next_slot_is_b = !old`. `current_track = target` sits
    ///    **outside** the attempt gate: the index updates even when no sound started, so a failed
    ///    attempt goes silent until the guest band changes (vanilla behavior, preserved).
    ///
    /// All dispatch is by fixed address through the `sndsound` `FunctionDef`s (established idiom -
    /// valid because the destructor's vtable swapdown only happens after the last `update` call),
    /// and boolean call results are masked to the low byte (`& 0xff`) exactly like `init`'s calls,
    /// because vanilla tests only AL.
    pub fn update(&mut self, delta: i32) {
        let base = get_module_base("zoo.exe") as u32;

        // Step 1: guests - raw dword at the live manager + 0x54 (see this method's doc comment).
        let guests: i32 = get_from_memory(globals().ztgamemgr_ptr() as u32 + 0x54);

        // Step 2: ambient crowd level.
        let level = ambient_level(guests);

        // Step 3: screen center - x/y/z come from the returned pointer, not the out buffer.
        let sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        let mut out = [0u32; 3];
        let center = unsafe { GET_SCREEN_CENTER.original()(sndmgr as *const u32, out.as_mut_ptr()) };
        let (x, y, z) = (
            get_from_memory::<i32>(center as u32),
            get_from_memory::<i32>(center as u32 + 4),
            get_from_memory::<i32>(center as u32 + 8),
        );

        // Step 4: re-jitter both Ambients blocks through the shared game RNG, crowd first. Only the
        // final LCG state is stored back (net-identical to vanilla's per-advance stores).
        for ambients in [self.crowd_ambients, self.world_ambients] {
            let seed: u32 = get_from_memory(base + GAME_RNG_RVA);
            let (position, state) = ambients_jitter(seed, x, y, z);
            save_to_memory(base + GAME_RNG_RVA, state);
            save_to_memory(ambients + 0xc, position[0]);
            save_to_memory(ambients + 0x10, position[1]);
            save_to_memory(ambients + 0x14, position[2]);
        }

        // Step 5: both Ambients blocks play through the Rust reimplementation, crowd (delta, level),
        // world (delta, 0x32) - reimplemented-to-reimplemented, matching the direct Rust construct
        // calls in `init` above rather than `generated.rs`'s `ambients::PLAY` address.
        unsafe {
            ref_from_memory::<Ambients>(self.crowd_ambients).play(delta, level);
            ref_from_memory::<Ambients>(self.world_ambients).play(delta, 0x32);
        }

        // Step 6: crossfade block, only while fading (see this method's doc comment).
        if self.fading != 0 {
            let c1: f32 = get_from_memory(base + DAT_0063542C_RVA);
            let c2: f32 = get_from_memory(base + DAT_00635428_RVA);
            let c3: f32 = get_from_memory(base + DAT_00635490_RVA);
            self.fade = advance_fade(self.fade, self.fade_step_in, delta);

            let slot_a = &self.crowd_snd_a as *const SndSlot as *const u32;
            let slot_b = &self.crowd_snd_b as *const SndSlot as *const u32;
            unsafe {
                if (SNDSOUND_VALID.original()(slot_a) & 0xff) != 0 {
                    SNDSOUND_SET_FADE_ATTENUATION.original()(slot_a, fade_atten_a(self.fade, c1, c2));
                    SNDSOUND_SET_VOLUME.original()(slot_a, 0);
                }
                if (SNDSOUND_VALID.original()(slot_b) & 0xff) != 0 {
                    SNDSOUND_SET_FADE_ATTENUATION.original()(slot_b, fade_atten_b(self.fade, c1, c2, c3));
                    SNDSOUND_SET_VOLUME.original()(slot_b, 0);
                }
                if self.fade == 0 {
                    if (SNDSOUND_VALID.original()(slot_b) & 0xff) != 0 {
                        SNDSOUND_STOP.original()(slot_b);
                        SNDSOUND_RELEASE.original()(slot_b);
                    }
                    self.fading = 0;
                } else if self.fade == 10000 {
                    if (SNDSOUND_VALID.original()(slot_a) & 0xff) != 0 {
                        SNDSOUND_STOP.original()(slot_a);
                        SNDSOUND_RELEASE.original()(slot_a);
                    }
                    self.fading = 0;
                }
            }
        }

        // Steps 7-8: selection computed unconditionally, start block gated on !fading (which the fade
        // block above may just have cleared, vanilla-faithfully).
        let target = select_target_track(self.current_track, guests);
        if self.fading == 0
            && let Some(target) = target
        {
            let index = target as usize;
            let slot = if self.next_slot_is_b == 0 {
                &self.crowd_snd_a as *const SndSlot as *const u32
            } else {
                &self.crowd_snd_b as *const SndSlot as *const u32
            };
            if (unsafe {
                SNDSOUND_ATTEMPT.original()(slot, sndmgr as *const u32, self.crowd_filename[index] as *const i8)
            } & 0xff) != 0
            {
                unsafe {
                    SNDSOUND_SET_BASE_ATTENUATION.original()(slot, self.crowd_atten[index]);
                    SNDSOUND_SET_FADE_ATTENUATION.original()(slot, START_FADE_ATTEN);
                    SNDSOUND_PLAY_LOOPED_1.original()(slot);
                }
                let old = self.next_slot_is_b;
                self.fading = 1;
                self.fade_step_in = old;
                self.fade = if old != 0 { 0 } else { 10000 };
                self.next_slot_is_b = (old == 0) as u8;
            }
            self.current_track = target as i32;
        }
    }

    /// Reimplementation of `ZTSoundscape::~ZTSoundscape` (`0x005003e2`), per
    /// `ZTSoundscape_~ZTSoundscape.asm`. The `generated.rs` entry (`ztsoundscape::DESTRUCTOR`) stays
    /// deliberately un-detoured - straight-line teardown, no decision logic - and is still what both
    /// `ztgamemgr.rs`'s `stop` and un-ported vanilla `~ZTGameMgr` run (`.original()` on the
    /// un-detoured address is the real vanilla body in every profile); this method is the same
    /// teardown for ported callers. The outer `0x54` block's own free stays the caller's job, exactly
    /// as with the vanilla address. Three passes, in vanilla's order:
    ///
    /// 1. **valid→release pass** - world slot, then crowd A, then crowd B (the crowd loop iterates
    ///    `+0xc` upward): per slot, real vanilla `VALID` (`+0x14`, low-byte-masked) gates a real
    ///    vanilla `RELEASE` (`+0x10`). **No `STOP` (`+0x60`) call anywhere in this dtor** - releasing
    ///    a still-live sound *is* the whole stop action (`SNDSound.md`'s `+0x10` row documents
    ///    exactly this reading). Dispatch is by fixed `sndsound` address rather than live vtable
    ///    dispatch on the same grounds [`ZTSoundscape::update`]'s doc states: each slot's vtable is
    ///    guaranteed to still be the live `SNDSound` vtable until this method's own swapdown below.
    /// 2. **Ambients frees**, behind vanilla's both-non-null pairing quirk, preserved verbatim: gate
    ///    1 reads `crowd_ambients` first, then `world_ambients`, and when both are non-null frees the
    ///    **world** block; gate 2 re-reads `world_ambients` first, then `crowd_ambients`, and frees
    ///    the **crowd** block the same way. **Neither pointer is zeroed afterward** (both left
    ///    dangling, vanilla-faithfully), so with exactly one non-null pointer *neither* block is
    ///    freed - a one-block leak vanilla ships with, not "fixed" here. `~Ambients` is the Rust
    ///    [`Ambients::destruct`] called directly (reimplemented-to-reimplemented, matching `init`'s
    ///    direct `construct` calls) and the block free is vanilla `OPERATOR_DELETE` (the same
    ///    allocator `init` allocated both blocks with).
    /// 3. **Per-slot swapdown** - world slot, then crowd B, then crowd A (the crowd loop iterates
    ///    downward from `+0x1c`), one [`destruct_slot`] call each: release the slot's inner sound
    ///    object through *its own* vtable slot 0 with `bDelete = 1`, zero `inner`, then drop the
    ///    slot's vtable to [`SNDSOUNDBASE_VTABLE`]. Vanilla's transient intermediate vtable write
    ///    (re-writing the live `SNDSound` vtable, immediately overwritten before any dispatch) is a
    ///    no-op in practice and is not reproduced.
    ///
    /// `.asm` annotation note: Ghidra renders both vtable writes as `MOV dword ptr [ESI],
    /// SNDSound::~SNDSound` / `SNDSoundBase::~SNDSoundBase`, which reads like code addresses; those
    /// annotations are the vtable **array** addresses (Ghidra labels a vtable pointer-array by its
    /// slot-0 destructor entry). The constructor's `.asm` writes the same `SNDSound::~SNDSound`
    /// annotation and provably writes the live `SNDSound` vtable `0x00630bc0` (pinned byte-for-byte
    /// by `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP`), so `.asm` and `.c` agree and the swapdown's final
    /// value per slot is exactly [`SNDSOUNDBASE_VTABLE`], matching [`SndSlot`]'s field doc.
    pub fn destruct(&mut self) {
        // Pass 1: valid→release - world slot first, then the two crowd slots (vanilla's +0xc-upward
        // loop). Fixed-address dispatch and low-byte masking, the established `update` idiom.
        for slot in [
            &self.world_snd as *const SndSlot as *const u32,
            &self.crowd_snd_a as *const SndSlot as *const u32,
            &self.crowd_snd_b as *const SndSlot as *const u32,
        ] {
            if (unsafe { SNDSOUND_VALID.original()(slot) } & 0xff) != 0 {
                unsafe { SNDSOUND_RELEASE.original()(slot) };
            }
        }

        // Pass 2: both Ambients frees behind the both-non-null pairing quirk, written as vanilla's
        // two separate gates with their own read order (see this method's doc comment).
        unsafe {
            if self.crowd_ambients != 0 {
                let world = self.world_ambients;
                if world != 0 {
                    (*(world as *mut Ambients)).destruct();
                    OPERATOR_DELETE.original()(world);
                }
            }
            if self.world_ambients != 0 {
                let crowd = self.crowd_ambients;
                if crowd != 0 {
                    (*(crowd as *mut Ambients)).destruct();
                    OPERATOR_DELETE.original()(crowd);
                }
            }
        }

        // Pass 3: per-slot swapdown - world, then crowd B, then crowd A (vanilla's downward loop
        // from +0x1c).
        destruct_slot(&mut self.world_snd);
        destruct_slot(&mut self.crowd_snd_b);
        destruct_slot(&mut self.crowd_snd_a);
    }
}

/// Hooks `ZTSoundscape`'s three hooked entries (`CONSTRUCTOR`/`INIT`/`UPDATE`) so any caller reaching
/// those addresses runs the Rust code above. Every detour fully replaces its entry with the
/// corresponding Rust method (each verified equivalent live by the `ZTSOUNDSCAPE_*` battery tests)
/// and never calls vanilla. The class's fourth entry - the destructor, `generated.rs`'s
/// `ztsoundscape::DESTRUCTOR` - is deliberately **not** hooked (see the module doc comment
/// and [`init`]).
#[detour_mod]
mod soundscape_detours {
    use super::*;

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor(this: *const c_void) -> *const u32 {
        unsafe { mut_from_memory::<ZTSoundscape>(this) }.construct();
        this as *const u32
    }

    #[detour(INIT)]
    unsafe extern "thiscall" fn init(
        this: *const c_void,
        crowd_ambients_name: *const u32, // regenerated pointer-as-integer wart - the Rust method takes `*const u8` for all four
        world_ambients_name: *const u32, // "
        crowd_config_name: *const u8,
        world_config_name: *const u8,
    ) {
        unsafe { mut_from_memory::<ZTSoundscape>(this) }.init(
            crowd_ambients_name as *const u8,
            world_ambients_name as *const u8,
            crowd_config_name,
            world_config_name,
        );
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const c_void, delta: i32) {
        unsafe { mut_from_memory::<ZTSoundscape>(this) }.update(delta);
    }

    /// Live-test access to the real vanilla bodies and to the detours' installation state. Once
    /// `init_detours()` has patched these three addresses, `.original()` on them re-enters the Rust
    /// detours above instead of reaching vanilla in release builds (it's a raw address cast there -
    /// the exact trap `reimplementation_tests::init()`'s ztawardmgr comment documents); debug builds
    /// route `.original()` through the registry's trampolines, but these `*_DETOUR.call` trampolines
    /// stay because the battery's "real vanilla" pole must be genuine vanilla in **every** profile.
    /// This lives inside the detour module because the generated `*_DETOUR` statics are
    /// module-private.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn constructor(this: *const c_void) -> *const u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) fn init(
            this: *const c_void,
            crowd_ambients_name: *const u32,
            world_ambients_name: *const u32,
            crowd_config_name: *const u8,
            world_config_name: *const u8,
        ) {
            unsafe {
                super::INIT_DETOUR.call(this, crowd_ambients_name, world_ambients_name, crowd_config_name, world_config_name)
            }
        }

        pub(crate) fn update(this: *const c_void, delta: i32) {
            unsafe { super::UPDATE_DETOUR.call(this, delta) }
        }

        /// `(name, is_enabled)` per detour - the battery asserts all three to catch a silently-failed
        /// `init_detours()` (error logged, game continues on vanilla).
        pub(crate) fn status() -> [(&'static str, bool); 3] {
            [
                ("CONSTRUCTOR", super::CONSTRUCTOR_DETOUR.is_enabled()),
                ("INIT", super::INIT_DETOUR.is_enabled()),
                ("UPDATE", super::UPDATE_DETOUR.is_enabled()),
            ]
        }
    }
}

/// Registers this module's live detours. Does **not** detour the destructor - `generated.rs`'s
/// `ztsoundscape::DESTRUCTOR` entry (`0x005003e2`) is real vanilla
/// `~ZTSoundscape` (confirmed via `ZTSoundscape_~ZTSoundscape.meta`'s matching address), whose
/// straight-line teardown both `ztgamemgr.rs`'s `stop` and un-ported vanilla `~ZTGameMgr` still run
/// through `.original()`, even though the same teardown is ported as [`ZTSoundscape::destruct`] (see
/// the module doc comment).
pub fn init() {
    if let Err(e) = unsafe { soundscape_detours::init_detours() } {
        error!("Failed to initialise ztsoundscape detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::standalone::OPERATOR_DELETE;
    use openzt_detour::generated::ztsoundscape::DESTRUCTOR as ZTSOUNDSCAPE_DESTRUCTOR;

    use super::*;

    /// Allocates a fresh, uninitialized `0x54`-byte block via the real vanilla allocator - callers must
    /// run either the real vanilla constructor ([`real_constructor`], or the bare
    /// `soundscape_detours::test_real::constructor` in this module) or [`ZTSoundscape::construct`] on it
    /// before reading any field.
    pub(crate) fn allocate_uninitialized() -> *mut ZTSoundscape {
        unsafe { OPERATOR_NEW.original()(0x54) as *mut ZTSoundscape }
    }

    /// Frees a standalone instance built via [`allocate_uninitialized`] plus (real or reimplemented)
    /// construction - vanilla allocator both sides, per `CLAUDE.md`'s cross-allocator rule -
    /// **without** calling the vanilla destructor: a constructor-only instance owns nothing the dtor
    /// would release (every embedded slot's `inner` is `0`, both `Ambients` pointers are `0`), so a
    /// plain `operator delete` is complete. Tests that run [`ZTSoundscape::init`] must tear down
    /// through [`destroy_standalone_after_init`] instead - this helper's plain free would leak both
    /// `Ambients` blocks and any started sound.
    pub(crate) fn destroy_standalone(ptr: *mut ZTSoundscape) {
        if ptr.is_null() {
            return;
        }
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Frees a standalone instance built via [`allocate_uninitialized`] plus construction **and** a
    /// call to [`ZTSoundscape::init`] (real vanilla or reimplemented): the real vanilla destructor
    /// (`ztsoundscape::DESTRUCTOR` - confirmed to actually
    /// be the destructor via `ZTSoundscape_~ZTSoundscape.meta`; the same two-call shape
    /// `ztgamemgr.rs`'s `stop` uses) followed by vanilla `OPERATOR_DELETE`. Clean per `CLAUDE.md`'s
    /// cross-allocator rule because *everything* an init'ed block holds - the `0x54` block itself, both
    /// `0x18` `Ambients` blocks, the embedded slots' vanilla-owned inner sound handles - is
    /// vanilla-allocated, and the destructor also stops any sound the `init` call started.
    pub(crate) fn destroy_standalone_after_init(ptr: *mut ZTSoundscape) {
        if ptr.is_null() {
            return;
        }
        unsafe { ZTSOUNDSCAPE_DESTRUCTOR.original()(ptr as *const std::ffi::c_void) };
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Trampolines to the real vanilla bodies for the battery's "real vanilla" pole once
    /// `reimplementation_tests::init()` has installed this module's detours - `.original()` on the
    /// three hooked addresses is a raw cast in release, so it would re-enter the Rust detours there
    /// (debug `.original()` routes correctly, but the vanilla pole must hold in every profile - see
    /// `soundscape_detours::test_real`'s doc comment).
    pub(crate) fn real_constructor(this: *const c_void) -> *const u32 {
        soundscape_detours::test_real::constructor(this)
    }

    pub(crate) fn real_init(
        this: *const c_void,
        crowd_ambients_name: *const u32,
        world_ambients_name: *const u32,
        crowd_config_name: *const u8,
        world_config_name: *const u8,
    ) {
        soundscape_detours::test_real::init(this, crowd_ambients_name, world_ambients_name, crowd_config_name, world_config_name)
    }

    pub(crate) fn real_update(this: *const c_void, delta: i32) {
        soundscape_detours::test_real::update(this, delta)
    }

    /// `(name, is_enabled)` per detour - see `soundscape_detours::test_real::status`.
    pub(crate) fn detour_status() -> [(&'static str, bool); 3] {
        soundscape_detours::test_real::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write-exhaustiveness pin on [`ZTSoundscape::construct`]: runs it over an `0xAA`-filled scratch
    /// buffer and asserts exactly the vanilla ctor's written bytes change - the three embedded slots'
    /// `{vtable, inner}` dwords plus the two zeroed `Ambients` pointers - with every other byte still
    /// filler. Proves the ctor initializes nothing beyond vanilla's set (the scalars and the filename/
    /// atten tables must stay garbage until `init`, per [`ZTSoundscape::construct`]'s doc comment) and
    /// pins each written field's offset.
    #[test]
    fn construct_writes_exactly_the_vanilla_set() {
        #[repr(align(4))]
        struct Scratch([u8; 0x54]);

        let mut scratch = Scratch([0xAA; 0x54]);
        let soundscape = unsafe { &mut *(scratch.0.as_mut_ptr() as *mut ZTSoundscape) };
        soundscape.construct();

        let bytes = &scratch.0;
        let dword = |off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());

        // The three embedded SNDSound slots: {SNDSound vtable, inner = 0} each.
        for off in [0x0c, 0x14, 0x3c] {
            assert_eq!(dword(off), SNDSOUND_VTABLE, "vtable dword at +{off:#x}");
            assert_eq!(dword(off + 4), 0, "inner dword at +{:#x}", off + 4);
        }
        // Both Ambients pointers zeroed.
        assert_eq!(dword(0x4c), 0, "crowd_ambients at +0x4c");
        assert_eq!(dword(0x50), 0, "world_ambients at +0x50");

        // Every byte outside those eight dwords must still be the 0xAA filler.
        for (i, &b) in bytes.iter().enumerate() {
            let written = matches!(i, 0x0c..=0x1b | 0x3c..=0x43 | 0x4c..=0x53);
            if written {
                assert_ne!(b, 0xAA, "byte at +{i:#x} inside a written region still 0xAA");
            } else {
                assert_eq!(b, 0xAA, "ctor wrote outside the vanilla set at +{i:#x}");
            }
        }
    }

    /// [`ambient_level`] against the vanilla band table (`update.asm:32-49` + `.78d61`), at every
    /// boundary and just inside each band, plus the signed extremes (the `guests < 0` arm is dead in
    /// practice but must keep branching like vanilla's `TEST`/`JL` on the full dword).
    #[test]
    fn ambient_level_matches_the_vanilla_band_table() {
        let expected = |g: i32| -> i32 {
            if g < 0 || g >= 100 {
                90
            } else if g <= 9 {
                -100
            } else if g <= 49 {
                10
            } else {
                40
            }
        };
        // Every band edge ±1, the extremes, and a mid-band sample each.
        for g in [-2, -1, 0, 1, 8, 9, 10, 11, 48, 49, 50, 51, 98, 99, 100, 101, i32::MIN, i32::MAX] {
            assert_eq!(ambient_level(g), expected(g), "guests = {g}");
        }
        // The cross-check table itself is pinned, not just self-consistent.
        assert_eq!(ambient_level(-1), 90);
        assert_eq!(ambient_level(0), -100);
        assert_eq!(ambient_level(9), -100);
        assert_eq!(ambient_level(10), 10);
        assert_eq!(ambient_level(49), 10);
        assert_eq!(ambient_level(50), 40);
        assert_eq!(ambient_level(99), 40);
        assert_eq!(ambient_level(100), 90);
    }

    /// [`select_target_track`] against the full vanilla net-effect grid (`update.asm` `.1e848` chain,
    /// re-verified branch by branch): 5 tracks x 7 guest bands, both endpoints of every band - dead
    /// bands (`t = 0`'s `<= 20` hold, etc.) and the `t = -1` column included. The selection only
    /// depends on band membership, so band endpoints pin it.
    #[test]
    fn select_target_track_matches_the_vanilla_grid() {
        // (guest band as inclusive (lo, hi), then the expected target per track -1/0/1/2/3).
        let bands: [(i32, i32); 7] = [(-5, 14), (15, 20), (21, 74), (75, 85), (86, 149), (150, 160), (161, 500_000)];
        let grid: [[Option<u32>; 7]; 5] = [
            // t = -1 (fresh init): 0 below 15, else always 1 - never 2/3 (the `g > 20` arm catches
            // `t in {0, -1}` before the deeper `t in {1, -1}`/`t in {2, -1}` arms can fire).
            [Some(0), Some(1), Some(1), Some(1), Some(1), Some(1), Some(1)],
            // t = 0: hold forever at a constant guest count.
            [None, None, Some(1), Some(1), Some(1), Some(1), Some(1)],
            // t = 1: down at <= 14, up at >= 86.
            [Some(0), None, None, None, Some(2), Some(2), Some(2)],
            // t = 2: down at <= 14 (to 0) or <= 74 (to 1), up at >= 161.
            [Some(0), Some(1), Some(1), None, None, None, Some(3)],
            // t = 3: down at <= 14 (to 0), <= 74 (to 1), <= 149 (to 2); hold above.
            [Some(0), Some(2), Some(2), Some(2), Some(2), None, None],
        ];
        let tracks = [-1i32, 0, 1, 2, 3];
        for (band, &(lo, hi)) in bands.iter().enumerate() {
            for (track_row, &t) in tracks.iter().enumerate() {
                for g in [lo, hi] {
                    assert_eq!(
                        select_target_track(t, g),
                        grid[track_row][band],
                        "current_track = {t}, guests = {g}"
                    );
                }
            }
        }
    }

    /// [`lcg_next`]/[`ambients_jitter`] against a hand-computed seed vector (seed 1, the classic MSVC
    /// test seed) - pins the advance constants, the sampling (`(r >> 16) & 0x7fff` then `%`), the
    /// **axis/modulus crossover** (the x pair is r3/r4 with modulus 400, the y pair is r1/r2 with
    /// modulus 300), the subtraction order, the `z` passthrough, the dword-wrap advance (seed
    /// `u32::MAX`), and the final-state == r4 return contract. r1..r4(1) = 2745024, 3357800067,
    /// 415139642, 3884216597; samples 41/18467/6334/26500 -> x offset 334-100 = +234, y offset
    /// 41-167 = -126.
    #[test]
    fn lcg_and_jitter_match_the_vanilla_sequence() {
        assert_eq!(lcg_next(1), 2_745_024);
        assert_eq!(lcg_next(2_745_024), 3_357_800_067);
        assert_eq!(lcg_next(3_357_800_067), 415_139_642);
        assert_eq!(lcg_next(415_139_642), 3_884_216_597);
        // 32-bit wrap parity with the dword IMUL/ADD pair.
        assert_eq!(lcg_next(u32::MAX), 0xFFFF_FFFFu32.wrapping_mul(0x343FD).wrapping_add(0x269EC3));

        let (position, state) = ambients_jitter(1, 1000, 2000, 7);
        assert_eq!(position, [1000 + 234, 2000 - 126, 7], "x from r3/r4 %400, y from r1/r2 %300, z passthrough");
        assert_eq!(state, 3_884_216_597, "final state must be r4, not an earlier advance");

        // The offset signs are driven by the sample pair, not fixed: seed 3 gives both axes
        // negative - r1..r4 = 3173050, 471647893, 2756632324, 2743275959 with samples
        // 48/7196/9294/9091 -> x = 9294%400 - 9091%400 = 94 - 291 = -197,
        // y = 48%300 - 7196%300 = 48 - 296 = -248. (Also pins lcg_next(0) = 2531011.)
        assert_eq!(lcg_next(0), 2_531_011);
        assert_eq!(lcg_next(3), 3_173_050);
        assert_eq!(lcg_next(3_173_050), 471_647_893);
        assert_eq!(lcg_next(471_647_893), 2_756_632_324);
        assert_eq!(lcg_next(2_756_632_324), 2_743_275_959);
        let (position, state) = ambients_jitter(3, 0, 0, -1);
        assert_eq!(position, [-197, -248, -1]);
        assert_eq!(state, 2_743_275_959);
    }

    /// [`advance_fade`]: both directions, both clamps, and the `0` -> `1` delta promotion (asm
    /// `.1cd227` - a zero-delta tick still creeps the ramp by exactly 1).
    #[test]
    fn advance_fade_matches_the_vanilla_clamps() {
        // Rising (fade_step_in != 0) and falling (== 0).
        assert_eq!(advance_fade(5000, 1, 1000), 6000);
        assert_eq!(advance_fade(5000, 0, 1000), 4000);
        assert_eq!(advance_fade(0, 1, 7), 7);
        assert_eq!(advance_fade(7, 0, 7), 0);
        // Clamps: 0..=10000, whichever side the advance overshoots.
        assert_eq!(advance_fade(500, 0, 1000), 0);
        assert_eq!(advance_fade(0, 0, 1), 0);
        assert_eq!(advance_fade(9500, 1, 1000), 10000);
        assert_eq!(advance_fade(10000, 1, 1), 10000);
        assert_eq!(advance_fade(9999, 1, i32::MAX), 0, "dword-add wrap parity: vanilla's ADD wraps past i32::MAX and its JS clamp catches the negative sign");
        assert_eq!(advance_fade(1, 0, i32::MAX), 0);
        assert_eq!(advance_fade(1, 0, i32::MIN), 0, "wrapping_neg parity with the dword IMUL");
        // delta = 0 promotes to 1 for this advance only.
        assert_eq!(advance_fade(5000, 1, 0), 5001);
        assert_eq!(advance_fade(5000, 0, 0), 4999);
        assert_eq!(advance_fade(10000, 0, 0), 9999);
    }

    /// [`fade_atten_a`]/[`fade_atten_b`] with the binary-confirmed constants (`DAT_0063542c` =
    /// f32 `0x38D1B717`, `DAT_00635428` = 4500.0, `DAT_00635490` = 1.0): the endpoints, the
    /// all-`f32`-divergence discriminators (where a naive `f32` chain rounds across an integer the
    /// x87 sequence truncates on the other side of), full-domain complementarity, and monotonicity.
    /// Expected values computed from an exact-rational emulation of the x87 sequence (the f64
    /// intermediates are provably exact over the whole `fade` domain, see [`fade_atten_a`]'s doc).
    #[test]
    fn fade_attenuators_match_the_x87_sequence() {
        let c1 = f32::from_bits(0x38D1_B717);
        let c2 = 4500.0_f32;
        let c3 = 1.0_f32;

        // Endpoints: silent-start / full-range ramps.
        assert_eq!(fade_atten_a(0, c1, c2), 0);
        assert_eq!(fade_atten_b(0, c1, c2, c3), 4500);
        assert_eq!(fade_atten_a(10000, c1, c2), 4500);
        assert_eq!(fade_atten_b(10000, c1, c2, c3), 0);

        // Truncation discriminators: an all-f32 chain yields 45/4050/4473 here - the x87
        // single-truncation values are one lower.
        assert_eq!(fade_atten_a(100, c1, c2), 44);
        assert_eq!(fade_atten_a(9000, c1, c2), 4049);
        assert_eq!(fade_atten_b(60, c1, c2, c3), 4472);
        // Non-discriminating truncation values.
        assert_eq!(fade_atten_a(3333, c1, c2), 1499);
        assert_eq!(fade_atten_b(3333, c1, c2, c3), 3000);

        // Full domain: monotone ramps, and complementarity within the truncation pair (sum 4499
        // mid-ramp, exactly 4500 at both ends - `t` rounds to 1.0 at fade = 10000, so b reaches 0
        // one ramp-step early at fade = 9999).
        let (mut prev_a, mut prev_b) = (0, 4500);
        for fade in 0..=10000 {
            let (a, b) = (fade_atten_a(fade, c1, c2), fade_atten_b(fade, c1, c2, c3));
            assert!(a >= prev_a, "fade_atten_a decreased at fade = {fade}");
            assert!(b <= prev_b, "fade_atten_b increased at fade = {fade}");
            assert!((4500 - a - b) == 0 || (4500 - a - b) == 1, "a+b = {} at fade = {fade}", a + b);
            prev_a = a;
            prev_b = b;
        }
        assert_eq!(prev_a, 4500);
        assert_eq!(prev_b, 0);
        assert_eq!(fade_atten_b(9999, c1, c2, c3), 0);
    }
}
