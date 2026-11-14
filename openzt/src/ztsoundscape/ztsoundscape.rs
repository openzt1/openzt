use crate::ambients::Ambients;
use crate::globals::{get_module_base, globals};
use crate::util::{get_from_memory, ref_from_memory, save_to_memory, write_live_ptr};
use crate::write_live;
use crate::bfconfigfile::ini_compat;
use openzt_configparser::ini::Ini;
use std::ffi::{CStr, CString};
use std::sync::Mutex;
use openzt_detour::generated::bfsndmgr::GET_SCREEN_CENTER;
use glam::IVec3;
use openzt_detour::generated::sndsound::{
    ATTEMPT as SNDSOUND_ATTEMPT, PLAY_LOOPED_1 as SNDSOUND_PLAY_LOOPED_1, RELEASE as SNDSOUND_RELEASE,
    SET_BASE_ATTENUATION as SNDSOUND_SET_BASE_ATTENUATION,
    SET_FADE_ATTENUATION as SNDSOUND_SET_FADE_ATTENUATION, SET_VOLUME as SNDSOUND_SET_VOLUME,
    STOP as SNDSOUND_STOP, VALID as SNDSOUND_VALID,
};
use openzt_detour::generated::standalone::{OPERATOR_DELETE, OPERATOR_NEW};

use super::snd_slot::{
    advance_fade, destruct_slot, fade_atten_a, fade_atten_b, SndSlot, SNDSOUND_VTABLE, START_FADE_ATTEN,
};

/// Data globals/`.rdata` literals `init` touches, all re-declared per-file after [`SNDSOUND_VTABLE`]
/// (no shared consts module). Unlike a vtable VA, **data** addresses are not identity-mapped: each is
/// stored as its Ghidra VA minus the preferred base, then resolved at runtime as
/// `get_module_base("zoo.exe") + RVA` (same shape as `ztgamemgr.rs`'s `GLOBAL_ZTSCENARIOMGR_RVA`).
/// Sound-device singleton: a global *pointer* (`MOV %ECX, dword ptr GLOBAL_DX8SndMgr` in
/// `ZTSoundscape_init.asm:141`), so its *value* is read, not the address itself.
pub const GLOBAL_DX8SNDMGR_RVA: u32 = 0x006380a8 - 0x400000;
// Vanilla `init` reads its two config files through static `BFConfigFile` objects at `0x00641850` (crowd) and
// `0x00641840` (world), referenced by no other decompiled function. This port reads the files through
// `ini_compat` and never touches them; only the live battery's real-vanilla pole still drives them.
/// Per-crowd-level default `.wav` filenames (quiet/small/medium/large, matching `crowd_filename`'s
/// indices), the `.rdata` C-string addresses vanilla stores on the init-defaults path.
pub const DEFAULT_CROWD_FILENAME_RVAS: [u32; 4] = [
    0x0064194c - 0x400000, // "sounds/quiet.wav"
    0x00641938 - 0x400000, // "sounds/crowds.wav"
    0x00641924 - 0x400000, // "sounds/crowdm.wav"
    0x00641910 - 0x400000, // "sounds/crowdl.wav"
];
/// Per-crowd-level `(filename_key, atten_key)` pairs within the crowd config section. All section/key
/// literals are all-lowercase in the binary (`ZTSoundscape_init.asm` label spellings); the raw VAs pass
/// through unchanged, so spelling only matters for these comments.
pub const CROWD_KEY_RVAS: [(u32, u32); 4] = [
    (0x006419ac - 0x400000, 0x00641994 - 0x400000), // "quiet" / "quietatten"
    (0x00638dc4 - 0x400000, 0x00641988 - 0x400000), // "small" / "smallatten"
    (0x00641980 - 0x400000, 0x00641974 - 0x400000), // "medium" / "medatten"
    (0x0064196c - 0x400000, 0x00641960 - 0x400000), // "large" / "largeatten"
];
pub const CROWD_SECTION_RVA: u32 = 0x006419a0 - 0x400000; // "crowdsound"
pub const WORLD_SECTION_RVA: u32 = 0x006419b4 - 0x400000; // "worldsound"
pub const WORLD_NAME_KEY_RVA: u32 = 0x00638d8c - 0x400000; // "name"
pub const WORLD_ATTEN_KEY_RVA: u32 = 0x006419c0 - 0x400000; // "atten"
/// Default base attenuation vanilla writes into every `crowd_atten` slot (1500). The world path's
/// default is a plain `0`, written inline - it is *not* this constant.
pub const DEFAULT_CROWD_ATTEN: i32 = 0x5dc;

/// The shared game RNG state (`DAT_00638060`) `update`'s position jitter advances - a raw dword LCG
/// state shared with a long list of un-ported vanilla consumers (see the module doc's shared-RNG
/// constraint). Read/advanced/written via `base + RVA`, exactly like the config-instance addresses.
pub const GAME_RNG_RVA: u32 = 0x00638060 - 0x400000;
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
pub const DAT_00635428_RVA: u32 = 0x00635428 - 0x400000;
pub const DAT_0063542C_RVA: u32 = 0x0063542c - 0x400000;
pub const DAT_00635490_RVA: u32 = 0x00635490 - 0x400000;

/// Real allocation size `0x54`, confirmed directly by `ZTGameMgr_start.c`'s `operator_new(0x54)` call
/// (not merely inferred from the constructor's own field writes, which stop at offset `0x50`).
#[repr(C)]
pub struct ZTSoundscape {
    pub current_track: i32,       // 0x00 - crowd track index: -1 = none, else 0..=3 into the crowd tables
    pub fade: i32,                // 0x04 - crossfade scalar, clamped 0..=10000
    pub next_slot_is_b: u8,       // 0x08 - which crowd slot the *next* start uses (0 -> crowd_snd_a, != 0 -> crowd_snd_b); toggles per start
    // Crossfade direction (`!= 0` -> `fade` rises per tick, `0` -> falls). Deliberately never written by
    // ctor/`init`, vanilla-faithfully (leftover uninitialized allocator memory, like `bfconfigfile.rs`'s
    // `pad_kind_tag`): it is only ever read while `fading` is set, and every path that sets `fading`
    // writes this first - so the garbage never escapes. Preserve; do not "fix" by zeroing.
    pub fade_step_in: u8,         // 0x09
    pub fading: u8,               // 0x0a
    pub _pad: [u8; 1],            // 0x0b - never referenced by any code path
    pub crowd_snd_a: SndSlot,     // 0x0c
    pub crowd_snd_b: SndSlot,     // 0x14
    pub crowd_filename: [u32; 4], // 0x1c - char* per crowd level 0..=3 (quiet/small/medium/large); init defaults to .rdata ".wav" pointers
    pub crowd_atten: [i32; 4],    // 0x2c - base attenuation per crowd level; init defaults each to 0x5dc (1500)
    pub world_snd: SndSlot,       // 0x3c
    pub world_name: u32,          // 0x44 - char*, 0 = no world sound
    // World sound's base attenuation. Deliberately left uninitialized when no world sound is
    // configured, vanilla-faithfully: `init` writes it only when the config supplied a `world_name`,
    // and the only read sits behind the same `world_name != 0` gate, so the garbage never escapes.
    // Preserve; do not "fix" by zeroing.
    pub world_atten: i32,         // 0x48
    pub crowd_ambients: u32,      // 0x4c - Ambients*, 0 = none (ctor-zeroed; init fills via vanilla operator_new(0x18))
    pub world_ambients: u32,      // 0x50 - Ambients*, 0 = none (same)
    pub _live: crate::util::LiveMemory,
}

const _: () = assert!(std::mem::size_of::<ZTSoundscape>() == 0x54);

/// `update` step 2's ambient crowd level as a pure band table over the guest count (`update.asm:32-49`;
/// the Windows `.c` renders the same bands through a goto chain; the macOS export agrees). The
/// `guests < 0` band is unreachable in practice - vanilla reads a *dword* at `ZTGameMgr+0x54`, and the
/// live field is a non-negative `u16` count, so negatives require non-zero pad bytes at `+0x56` - but
/// it is preserved anyway, exactly as vanilla branches (`TEST %EAX, %EAX` / `JL` on the full dword).
pub fn ambient_level(guests: i32) -> i32 {
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
pub fn select_target_track(current_track: i32, guests: i32) -> Option<u32> {
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
pub fn lcg_next(state: u32) -> u32 {
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
pub fn ambients_jitter(seed: u32, x: i32, y: i32, z: i32) -> ([i32; 3], u32) {
    let r1 = lcg_next(seed);
    let r2 = lcg_next(r1);
    let r3 = lcg_next(r2);
    let r4 = lcg_next(r3);
    let sample = |r: u32| ((r >> 16) & 0x7fff) as i32;
    let x_offset = sample(r3) % 400 - sample(r4) % 400;
    let y_offset = sample(r1) % 300 - sample(r2) % 300;
    ([x.wrapping_add(x_offset), y.wrapping_add(y_offset), z], r4)
}

/// The filename strings each initialised soundscape points at, owned here so they outlive `init`: vanilla
/// stores the pointers in the struct (`crowd_filename`, `world_name`) and `update` reads them on every
/// crowd-track start. Keyed by the soundscape's address so initialising one soundscape never frees
/// strings another live one still points at; re-initialising the same address replaces that entry's
/// strings (the struct's pointers are overwritten with the new ones first). The soundscape's destructor
/// is real vanilla code, so an entry for an object that has been destroyed stays until its address is
/// initialised again - at most a handful of short strings per game start.
static OWNED_FILENAMES: Mutex<Vec<(u32, Vec<CString>)>> = Mutex::new(Vec::new());

fn own_filenames(owner: u32, strings: Vec<CString>) {
    let mut entries = OWNED_FILENAMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match entries.iter_mut().find(|(address, _)| *address == owner) {
        Some(entry) => entry.1 = strings,
        None => entries.push((owner, strings)),
    }
}

/// A C string holding `text` up to its first NUL, which is what vanilla's `char*` view of the value would
/// read.
fn c_string_up_to_nul(text: &str) -> CString {
    let bytes = text.as_bytes();
    let end = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
    CString::new(&bytes[..end]).expect("truncated at the first NUL")
}

/// Reads the `.cfg` resource named by a `BFScenarioMgr` getter result. A null or empty name is a failed
/// `BFConfigFile::attempt` in vanilla, so it yields `None` without logging.
fn read_soundscape_config(name: *const u8) -> Option<Ini> {
    if name.is_null() || get_from_memory::<u8>(name as u32) == 0 {
        return None;
    }
    let name = unsafe { CStr::from_ptr(name as *const i8) }.to_string_lossy().into_owned();
    ini_compat::read_cfg(&name)
}

/// A NUL-terminated string literal in `zoo.exe`'s data (the section and key names `init` looks up).
fn rdata_string(address: u32) -> String {
    unsafe { CStr::from_ptr(address as *const i8) }.to_string_lossy().into_owned()
}

impl ZTSoundscape {
    /// Reimplementation of `ZTSoundscape::ZTSoundscape` (`0x00592596`), per
    /// `ZTSoundscape_ZTSoundscape.c`/`.asm`. Pure constant writes, no calls: `{vtable:
    /// SNDSOUND_VTABLE, inner: 0}` into the three embedded `SNDSound` slots and `0` into both
    /// `Ambients` pointers. The scalar state and the filename/atten tables are deliberately left
    /// uninitialized exactly like vanilla (`operator_new` doesn't zero - `init` writes everything the
    /// rest of the class reads), and the ctor's post-assignment `if inner != 0` release idiom is a dead
    /// no-op at construction time (`inner` was just written `0`) and is not reproduced.
    pub fn construct(&self) {
        write_live!(self, crowd_snd_a, SndSlot { vtable: SNDSOUND_VTABLE, inner: 0 });
        write_live!(self, crowd_snd_b, SndSlot { vtable: SNDSOUND_VTABLE, inner: 0 });
        write_live!(self, world_snd, SndSlot { vtable: SNDSOUND_VTABLE, inner: 0 });
        write_live!(self, crowd_ambients, 0);
        write_live!(self, world_ambients, 0);
    }

    /// Reimplementation of `ZTSoundscape::init` (`0x005922fd`), per `ZTSoundscape_init.asm` - the
    /// Windows `.asm` is ground truth here, the shipped `.c` garbles the three world-sound vtable
    /// calls. Vanilla is void; the four arguments are exactly what `ztgamemgr.rs`'s `start` gets from
    /// its four `BFScenarioMgr` getters (`*const u8`), passed through uncasted.
    ///
    /// The crowd and world config files are read through `bfconfigfile::ini_compat`, not the two static
    /// vanilla `BFConfigFile` objects. The filename pointers stored in `crowd_filename`/`world_name` point
    /// at `CString`s this object's entry in [`OWNED_FILENAMES`] keeps alive (see there), because `update`
    /// reads them long after `init` returns.
    pub fn init(
        &self,
        crowd_ambients_name: *const u8,
        world_ambients_name: *const u8,
        crowd_config_name: *const u8,
        world_config_name: *const u8,
    ) {
        let base = get_module_base("zoo.exe") as u32;

        let crowd_block = unsafe { OPERATOR_NEW.original()(0x18) };
        let crowd_ambients = if crowd_block.is_null() {
            0
        } else {
            let mut ctor_data = [0u32; 3];
            unsafe { (*(crowd_block as *mut Ambients)).construct(crowd_ambients_name, ctor_data.as_mut_ptr() as *const i32) };
            crowd_block as u32
        };
        write_live!(self, crowd_ambients, crowd_ambients);

        let world_block = unsafe { OPERATOR_NEW.original()(0x18) };
        let world_ambients = if world_block.is_null() {
            0
        } else {
            let mut ctor_data = [0u32; 3];
            unsafe { (*(world_block as *mut Ambients)).construct(world_ambients_name, ctor_data.as_mut_ptr() as *const i32) };
            world_block as u32
        };
        write_live!(self, world_ambients, world_ambients);

        // Defaults, in vanilla's write order.
        write_live!(self, world_name, 0);
        for (index, &rva) in DEFAULT_CROWD_FILENAME_RVAS.iter().enumerate() {
            unsafe { write_live_ptr(core::ptr::addr_of!(self.crowd_filename[index]).cast_mut(), base + rva) };
        }
        write_live!(self, crowd_atten, [DEFAULT_CROWD_ATTEN; 4]);

        // Both config files are read through `ini_compat` into Rust memory. Vanilla stores the interned `char*`
        // values `getString` returns directly in this struct, and `update` passes `crowd_filename[n]` to
        // `SNDSound::attempt` long after `init` returns, so each value is copied into a `CString` that
        // `own_filenames` keeps alive in a static, keyed by this object's address.
        let mut owned: Vec<CString> = Vec::new();
        let mut take = |text: &str| -> u32 {
            let c_string = c_string_up_to_nul(text);
            let pointer = c_string.as_ptr() as u32;
            owned.push(c_string);
            pointer
        };

        // Crowd config: a successful read gates all four key pairs; a found filename resets its
        // attenuation to the default before the (optional) attenuation key overrides it.
        if let Some(ini) = read_soundscape_config(crowd_config_name) {
            let section = rdata_string(base + CROWD_SECTION_RVA);
            for (index, &(key, atten_key)) in CROWD_KEY_RVAS.iter().enumerate() {
                if let Some(name) = ini_compat::first(&ini, &section, &rdata_string(base + key)) {
                    unsafe { write_live_ptr(core::ptr::addr_of!(self.crowd_filename[index]).cast_mut(), take(&name)) };
                    let atten = ini_compat::int_or(&ini, &section, &rdata_string(base + atten_key), DEFAULT_CROWD_ATTEN);
                    unsafe { write_live_ptr(core::ptr::addr_of!(self.crowd_atten[index]).cast_mut(), atten) };
                }
            }
        }

        // World config: same shape; the name lookup writes world_name, and only when it is found does
        // the attenuation lookup run (over a plain-0 default, per this method's doc comment).
        if let Some(ini) = read_soundscape_config(world_config_name) {
            let section = rdata_string(base + WORLD_SECTION_RVA);
            if let Some(name) = ini_compat::first(&ini, &section, &rdata_string(base + WORLD_NAME_KEY_RVA)) {
                write_live!(self, world_name, take(&name));
                let atten = ini_compat::int_or(&ini, &section, &rdata_string(base + WORLD_ATTEN_KEY_RVA), 0);
                write_live!(self, world_atten, atten);
            }
        }
        own_filenames(self as *const Self as u32, owned);

        // World sound: the sndmgr singleton is loaded unconditionally before the world_name test.
        let world_name = self.world_name;
        let sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        if world_name != 0 {
            let slot = &self.world_snd as *const SndSlot as *const u32;
            unsafe {
                if SNDSOUND_ATTEMPT.original()(slot, sndmgr as *const u32, world_name as *const i8) {
                    SNDSOUND_SET_BASE_ATTENUATION.original()(slot, self.world_atten);
                    SNDSOUND_PLAY_LOOPED_1.original()(slot);
                }
            }
        }

        // Tail, in vanilla's write order.
        write_live!(self, current_track, -1);
        write_live!(self, fade, 0);
        write_live!(self, fading, 0);
        write_live!(self, next_slot_is_b, 0);
    }

    /// Reimplementation of `ZTSoundscape::update` (`0x004352dd`), per `ZTSoundscape_update.asm`.
    pub fn update(&self, delta: i32) {
        let base = get_module_base("zoo.exe") as u32;

        // Step 1: guests - raw dword at the live manager + 0x54.
        let guests: i32 = get_from_memory(globals().ztgamemgr_ptr() as u32 + 0x54);

        // Step 2: ambient crowd level.
        let level = ambient_level(guests);

        // Step 3: screen center.
        let sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        let out = IVec3::ZERO;
        let center = unsafe { GET_SCREEN_CENTER.original()(sndmgr as *const u32, &out as *const IVec3) };
        let (x, y, z) = (
            get_from_memory::<i32>(center as u32),
            get_from_memory::<i32>(center as u32 + 4),
            get_from_memory::<i32>(center as u32 + 8),
        );

        // Step 4: re-jitter both Ambients blocks through the shared game RNG, crowd first.
        for ambients in [self.crowd_ambients, self.world_ambients] {
            let seed: u32 = get_from_memory(base + GAME_RNG_RVA);
            let (position, state) = ambients_jitter(seed, x, y, z);
            save_to_memory(base + GAME_RNG_RVA, state);
            save_to_memory(ambients + 0xc, position[0]);
            save_to_memory(ambients + 0x10, position[1]);
            save_to_memory(ambients + 0x14, position[2]);
        }

        // Step 5: both Ambients blocks play through the Rust reimplementation.
        unsafe {
            ref_from_memory::<Ambients>(self.crowd_ambients).play(delta, level);
            ref_from_memory::<Ambients>(self.world_ambients).play(delta, 0x32);
        }

        // Step 6: crossfade block, only while fading.
        if self.fading != 0 {
            let c1: f32 = get_from_memory(base + DAT_0063542C_RVA);
            let c2: f32 = get_from_memory(base + DAT_00635428_RVA);
            let c3: f32 = get_from_memory(base + DAT_00635490_RVA);
            write_live!(self, fade, advance_fade(self.fade, self.fade_step_in, delta));

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
            }
            if self.fade == 0 {
                if (unsafe { SNDSOUND_VALID.original()(slot_b) } & 0xff) != 0 {
                    unsafe {
                        SNDSOUND_STOP.original()(slot_b);
                        SNDSOUND_RELEASE.original()(slot_b);
                    }
                }
                write_live!(self, fading, 0);
            } else if self.fade == 10000 {
                if (unsafe { SNDSOUND_VALID.original()(slot_a) } & 0xff) != 0 {
                    unsafe {
                        SNDSOUND_STOP.original()(slot_a);
                        SNDSOUND_RELEASE.original()(slot_a);
                    }
                }
                write_live!(self, fading, 0);
            }
        }

        // Steps 7-8: selection computed unconditionally, start block gated on !fading.
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
            if unsafe { SNDSOUND_ATTEMPT.original()(slot, sndmgr as *const u32, self.crowd_filename[index] as *const i8) } {
                unsafe {
                    SNDSOUND_SET_BASE_ATTENUATION.original()(slot, self.crowd_atten[index]);
                    SNDSOUND_SET_FADE_ATTENUATION.original()(slot, START_FADE_ATTEN);
                    SNDSOUND_PLAY_LOOPED_1.original()(slot);
                }
                let old = self.next_slot_is_b;
                write_live!(self, fading, 1);
                write_live!(self, fade_step_in, old);
                write_live!(self, fade, if old != 0 { 0 } else { 10000 });
                write_live!(self, next_slot_is_b, (old == 0) as u8);
            }
            write_live!(self, current_track, target as i32);
        }
    }

    /// Reimplementation of `ZTSoundscape::~ZTSoundscape` (`0x005003e2`), per `ZTSoundscape_~ZTSoundscape.asm`.
    pub fn destruct(&self) {
        // Pass 1: valid→release.
        for slot in [
            &self.world_snd as *const SndSlot as *const u32,
            &self.crowd_snd_a as *const SndSlot as *const u32,
            &self.crowd_snd_b as *const SndSlot as *const u32,
        ] {
            if (unsafe { SNDSOUND_VALID.original()(slot) } & 0xff) != 0 {
                unsafe { SNDSOUND_RELEASE.original()(slot) };
            }
        }

        // Pass 2: both Ambients frees.
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

        // Pass 3: per-slot swapdown.
        destruct_slot(core::ptr::addr_of!(self.world_snd));
        destruct_slot(core::ptr::addr_of!(self.crowd_snd_b));
        destruct_slot(core::ptr::addr_of!(self.crowd_snd_a));
    }
}

#[cfg(test)]
mod filename_ownership_tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn c_string_stops_at_the_first_nul() {
        assert_eq!(c_string_up_to_nul("sounds/crowds.wav").to_str().unwrap(), "sounds/crowds.wav");
        assert_eq!(c_string_up_to_nul("abc def").to_str().unwrap(), "abc");
        assert_eq!(c_string_up_to_nul("").to_bytes().len(), 0);
    }

    #[test]
    fn owned_strings_stay_valid_across_moves_and_other_owners() {
        let first = c_string_up_to_nul("first.wav");
        let pointer = first.as_ptr();
        own_filenames(0x1000_0001, vec![first]);
        // Registering and replacing other owners must not touch this owner's strings.
        own_filenames(0x1000_0002, vec![c_string_up_to_nul("other.wav")]);
        own_filenames(0x1000_0002, vec![c_string_up_to_nul("replaced.wav")]);
        assert_eq!(unsafe { CStr::from_ptr(pointer) }.to_str().unwrap(), "first.wav");
    }

    #[test]
    fn reinitialising_an_owner_replaces_only_its_own_entry() {
        own_filenames(0x1000_0003, vec![c_string_up_to_nul("a.wav")]);
        own_filenames(0x1000_0003, vec![c_string_up_to_nul("b.wav")]);
        let entries = OWNED_FILENAMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let owned: Vec<_> = entries.iter().filter(|(address, _)| *address == 0x1000_0003).collect();
        assert_eq!(owned.len(), 1, "one entry per owner address");
        assert_eq!(owned[0].1[0].to_str().unwrap(), "b.wav");
    }
}
