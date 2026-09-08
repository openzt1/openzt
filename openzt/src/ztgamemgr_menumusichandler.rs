//! `ZTGameMgr::MenuMusicHandler` reimplementation - see
//! `openzt/plans/menumusichandler-implementation-plan.md` for the full investigation. A **self-contained
//! leaf class**: every method is called through a fixed address (there is no `MenuMusicHandler` vtable),
//! so detouring this class's own 6 addresses redirects every *non-inlined* caller - all in un-ported,
//! un-detoured `ZTGameMgr` methods (`initMenuMusic`/`startMenuMusic`/`startMenuMusicFade_*`) that stay
//! real vanilla code and never need to change. The exceptions are the sites where the compiler inlined a
//! method body outright instead of calling its address: `startMenuMusicFade_1` contains a full `startFade`
//! copy (reads/writes `fading`/`fade_counter` and makes the vtable calls directly), `startMenuMusic`
//! inlines `shouldNotRestart`, and `~ZTGameMgr` inlines the destructor (its tail-merge block is what
//! `generated.rs` mislabels `MENU_MUSIC_HANDLER_0`). Those never touch the hooked addresses; they stay
//! correct only because this port maintains the struct's raw fields in place (style 1) - and by the same
//! token they would *not* pick up any future change to Rust `start_fade`'s semantics until that wrapper
//! is itself ported.
//!
//! Vanilla-layout-compatible (style 1, per `CLAUDE.md`'s two-style split): real, un-ported vanilla code
//! still walks this struct's raw memory directly to tear it down - `this+0x0` must therefore always hold a
//! real, vanilla-allocated `SNDSound*` (or `0`), which falls out naturally as long as every mutator here
//! writes through vanilla's own allocator/constructors rather than substituting Rust-owned state.
//!
//! All four in-scope stages (struct + constructor + `init` + `startPlay`/`startFade` + `update`); the
//! destructor needs no port or detour - Stage 5 resolved that it is inlined into `~ZTGameMgr` (whose
//! tail-merge block is what `generated.rs` mislabels `MENU_MUSIC_HANDLER_0`) and has no standalone Windows
//! address, so there is nothing to hook.

use std::ffi::c_void;

use openzt_detour::generated::{
    bfinifile::READ,
    // `DX8SndMgr::attempt` (this class's vtable `+0x14` override, `0x004070b0` - per
    // `private/docs/vtables/DX8SndMgr.md`'s override table; `BFSndMgr`'s own `+0x14` `attempt` is the other
    // address, `bfsndmgr::ATTEMPT_1`). Ghidra's pass filed the function under the parent class, so it lives
    // in `bfsndmgr` despite being the `DX8SndMgr` slot this code dispatches.
    bfsndmgr::ATTEMPT_0 as DX8SNDMGR_ATTEMPT,
    sndsound::{
        IS_PLAYING, PLAY_LOOPED_1, SET_BASE_ATTENUATION, SET_FADE_ATTENUATION, SET_VOLUME,
        SNDSOUND_1, STOP, VALID,
    },
    msvc_std_basic_string::{BASIC_STRING_0, BASIC_STRING_2},
    standalone::OPERATOR_NEW,
    ztgamemgr_menumusichandler::{
        INIT, MENU_MUSIC_HANDLER_1 as CONSTRUCTOR, START_FADE, START_PLAY, UPDATE,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::get_module_base,
    util::{get_from_memory, mut_from_memory, save_to_memory},
};

/// `GLOBAL_DX8SndMgr`'s RVA - a raw pointer-typed global (one dereference gives the live `DX8SndMgr*`
/// singleton), resolved by the user directly (`0x006380a8`). Used by [`MenuMusicHandler::init`] as the
/// `this` for `DX8SndMgr::attempt` (`MenuMusicHandler_init.asm`'s `MOV ECX, dword ptr GLOBAL_DX8SndMgr`
/// before that call) - same one-level-of-indirection shape as `ztgamemgr.rs`'s own
/// `GLOBAL_ZTSCENARIOMGR_RVA`/`GLOBAL_ZTAPP_RVA`.
const GLOBAL_DX8SNDMGR_RVA: u32 = 0x006380a8 - 0x400000;

/// `SNDSound`'s real vtable VA (`private/docs/vtables/SNDSound.md`), written directly into a freshly
/// `operator_new`-allocated `SNDSound` by [`MenuMusicHandler::init`], mirroring vanilla's own inlined
/// `SNDSound` construction (`MenuMusicHandler_init.c`'s `pSVar2->vftptr_0x0 = &SNDSound__vtable_00630bc0;`)
/// rather than any `SNDSound` constructor `FunctionDef` - there isn't one at a fixed address at this call
/// site, it's inlined. A raw constant, not RVA'd, matching `bfentitytype.rs`'s own `VTABLE_PTR` precedent -
/// zoo.exe has no ASLR and always loads at its preferred base, so a vtable's Ghidra VA already equals its
/// runtime VA.
const SNDSOUND_VTABLE: u32 = 0x00630bc0;

/// Real allocation size `0x14`, confirmed directly by `ZTGameMgr_initMenuMusic.c`'s
/// `operator_new(0x14)` call (not merely inferred from the constructor's own field writes, which stop at
/// offset `0x10`) - see the implementation plan's struct-layout table for the per-field evidence.
#[repr(C)]
pub struct MenuMusicHandler {
    sound_ptr: u32,               // 0x0 - SNDSound*, 0 when none
    fading: u8,                   // 0x4
    _pad1: [u8; 3],
    fade_counter: i32,            // 0x8
    ini_menu_music_disabled: u8,  // 0xc
    _pad2: [u8; 3],
    warmup_ticks: i32,            // 0x10
}

const _: () = assert!(std::mem::size_of::<MenuMusicHandler>() == 0x14);

impl MenuMusicHandler {
    /// Reimplementation of `ZTGameMgr::MenuMusicHandler::MenuMusicHandler`, per
    /// `MenuMusicHandler_MenuMusicHandler.c`/`.asm`. Zeroes `sound_ptr`/`fading`/`fade_counter`/
    /// `warmup_ticks`, then resolves `ini_menu_music_disabled` from a real
    /// `BFIniFile::read("UI", "noMenuMusic", 0)` call - built from two temporary, real-vanilla-constructed
    /// `std::string`s ([`VanillaString`]), exactly matching vanilla's own stack-string construction, torn
    /// down again before returning.
    pub fn construct(&mut self) {
        self.sound_ptr = 0;
        self.fading = 0;
        self.fade_counter = 0;
        self.warmup_ticks = 0;

        let section = VanillaString::new("UI");
        let key = VanillaString::new("noMenuMusic");
        let result = unsafe { READ.original()(section.as_ptr(), key.as_ptr(), 0) };
        self.ini_menu_music_disabled = (result == 1) as u8;
    }

    /// Reimplementation of `ZTGameMgr::MenuMusicHandler::init`, per `MenuMusicHandler_init.c`/`.asm`.
    ///
    /// The real function takes two stack args (`RET 0x8`), not the one `generated.rs`'s auto-derived
    /// `INIT` originally carried - surfaced for the generator pass, which now emits both: `filename`
    /// (a real vanilla C string, typed `u32` in the generated entry, so cast at the detour boundary)
    /// and `attenuation` (an `i32`, forwarded unchanged to `SNDSound::setBaseAttenuation` on success).
    ///
    /// Faithfully reproduces one vanilla oddity: releasing an existing, currently-*playing* `sound_ptr`
    /// (`stop()` + slot-0 `release(1)`) does **not** clear `self.sound_ptr` afterwards - if
    /// `ini_menu_music_disabled` is set, this function returns early with `sound_ptr` still pointing at
    /// the just-released (and potentially now-dangling) `SNDSound`, exactly matching vanilla's own control
    /// flow. Not fixed here - this port preserves vanilla behavior verbatim, bugs included.
    pub fn init(&mut self, filename: *const i8, attenuation: i32) -> bool {
        if self.sound_ptr != 0 && unsafe { IS_PLAYING.original()(self.sound_ptr as *const u32) } != 0 {
            unsafe { STOP.original()(self.sound_ptr as *const u32) };
            if self.sound_ptr != 0 {
                unsafe { SNDSOUND_1.original()(self.sound_ptr as *const u32, 1) };
            }
        }

        if self.ini_menu_music_disabled != 0 {
            return false;
        }

        let new_sound = unsafe { OPERATOR_NEW.original()(0x8) } as u32;
        let new_sound = if new_sound != 0 {
            // Inlined `SNDSound` construction, matching vanilla's own field-write order exactly.
            save_to_memory(new_sound + 0x4, 0u32);
            save_to_memory(new_sound, SNDSOUND_VTABLE);
            new_sound
        } else {
            0
        };

        self.sound_ptr = new_sound;
        if new_sound == 0 {
            return false;
        }
        self.fade_counter = 0;
        self.fading = 0;

        let dx8sndmgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_DX8SNDMGR_RVA);
        let success = unsafe { DX8SNDMGR_ATTEMPT.original()(dx8sndmgr_ptr as *const u32, new_sound as *const u32, filename) } != 0;
        if success {
            unsafe { SET_BASE_ATTENUATION.original()(new_sound as *const u32, attenuation) };
        }
        success
    }

    /// Reimplementation of `ZTGameMgr::MenuMusicHandler::startPlay`, per
    /// `MenuMusicHandler_startPlay.c`/`.asm` (vtable offsets per the implementation plan's call table).
    ///
    /// No-op when `ini_menu_music_disabled` is set or `sound_ptr` is null. Otherwise: if the `SNDSound` is
    /// still valid ([`VALID`], vtable `+0x14`) but not currently playing ([`IS_PLAYING`], `+0x50`),
    /// (re)starts it looped ([`PLAY_LOOPED_1`], `+0x3c`). Then - unconditionally, including when `VALID`
    /// returned false, exactly matching vanilla's control flow - clears `fading`/`fade_counter` and pushes
    /// a `0` through [`SET_FADE_ATTENUATION`] (`+0x4c`) and [`SET_VOLUME`] (`+0x40`).
    ///
    /// Boolean call results are masked to the low byte (`& 0xff`) because vanilla's own call sites test
    /// only `AL` (`TEST AL, AL` in the `.asm`) and the two thunks' bodies aren't in the decompile corpus
    /// to confirm what they leave in the upper EAX bits.
    pub fn start_play(&mut self) {
        if self.ini_menu_music_disabled != 0 || self.sound_ptr == 0 {
            return;
        }
        let sound = self.sound_ptr as *const u32;
        if (unsafe { VALID.original()(sound) } & 0xff) != 0
            && (unsafe { IS_PLAYING.original()(sound) } & 0xff) == 0
        {
            unsafe { PLAY_LOOPED_1.original()(sound) };
        }
        self.fading = 0;
        self.fade_counter = 0;
        unsafe { SET_FADE_ATTENUATION.original()(sound, 0) };
        unsafe { SET_VOLUME.original()(sound, 0) };
    }

    /// Reimplementation of `ZTGameMgr::MenuMusicHandler::startFade`, per
    /// `MenuMusicHandler_startFade.c`/`.asm`. Arms the fade (`fading = 1`, `fade_counter = 0`) - but only
    /// when not already fading, `sound_ptr` is non-null, and the sound reports currently playing
    /// ([`IS_PLAYING`], `+0x50`); otherwise a complete no-op. Same low-byte masking as [`start_play`].
    pub fn start_fade(&mut self) {
        if self.fading != 0 || self.sound_ptr == 0 {
            return;
        }
        let sound = self.sound_ptr as *const u32;
        if (unsafe { IS_PLAYING.original()(sound) } & 0xff) != 0 {
            self.fading = 1;
            self.fade_counter = 0;
        }
    }

    /// Reimplementation of `ZTGameMgr::MenuMusicHandler::update`, per `MenuMusicHandler_update.c`/`.asm`.
    /// A no-op unless a fade is armed (`fading != 0` - the gate the plan's Stage-4 summary omits, but the
    /// `.asm`'s very first test, `MOV AL,[ESI+4]` / `TEST AL,AL`) and a `SNDSound` is present. Then a
    /// two-phase state machine:
    ///
    /// 1. A fixed 5-tick warm-up delay (`warmup_ticks < 5` -> increment and return, signed `JL`), matching
    ///    `ztadvterrainmgr.rs`'s own precedent for this kind of tick-gated state machine.
    /// 2. Once warm: `delta >= 2000` returns untouched (unsigned `JNC`); otherwise accumulate
    ///    [`fade_increment`] into `fade_counter`, and either complete the fade (`fade_counter > 3000`,
    ///    signed `JG`) or push the updated counter through [`SET_FADE_ATTENUATION`] (`+0x4c`) plus a
    ///    constant `0` through [`SET_VOLUME`] (`+0x40`).
    ///
    /// The completion branch is gated harder than the plan's Stage-4 summary states: **every** field clear
    /// and the sound teardown sit *inside* the [`IS_PLAYING`] check (`JZ` past the whole block in the
    /// `.asm`), not after it - if the sound reports not playing, a `fade_counter` past 3000 is left as-is
    /// and nothing at all happens. Only inside that gate: `fading` = 0, `fade_counter` = 0, [`STOP`]
    /// (`+0x60`), then - preserving vanilla's own redundant re-check of `sound_ptr`, exactly like
    /// [`init`]'s release branch - the slot-0 [`SNDSOUND_1`] `release(1)` idiom, then `sound_ptr` = 0.
    /// That final gate is unreachable live in the test battery without genuinely playing audio (see
    /// `MENUMUSICHANDLER_UPDATE`'s doc comment), but is the same [`SNDSOUND_1`] release shape
    /// [`init`]/[`destroy_standalone_after_init`] already exercise for real.
    pub fn update(&mut self, delta: u32) {
        if self.fading == 0 || self.sound_ptr == 0 {
            return;
        }
        if self.warmup_ticks < 5 {
            self.warmup_ticks += 1;
            return;
        }
        if delta >= 2000 {
            return;
        }

        self.fade_counter += fade_increment(delta);
        if self.fade_counter > 3000 {
            let sound = self.sound_ptr as *const u32;
            if (unsafe { IS_PLAYING.original()(sound) } & 0xff) != 0 {
                self.fading = 0;
                self.fade_counter = 0;
                unsafe { STOP.original()(sound) };
                if self.sound_ptr != 0 {
                    unsafe { SNDSOUND_1.original()(sound, 1) };
                }
                self.sound_ptr = 0;
            }
            return;
        }

        let sound = self.sound_ptr as *const u32;
        unsafe { SET_FADE_ATTENUATION.original()(sound, self.fade_counter) };
        unsafe { SET_VOLUME.original()(sound, 0) };
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn sound_ptr(&self) -> u32 {
        self.sound_ptr
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn fading(&self) -> u8 {
        self.fading
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn fade_counter(&self) -> i32 {
        self.fade_counter
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn ini_menu_music_disabled(&self) -> u8 {
        self.ini_menu_music_disabled
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn warmup_ticks(&self) -> i32 {
        self.warmup_ticks
    }
}

/// `update`'s per-tick fade increment: `(int)((double)delta * 0.5)`. `MenuMusicHandler_update.asm` shows
/// MSVC's classic `_ftol` double-to-int sequence (`FSTCW` / `OR AH,0xc` / `FLDCW` / `FISTP` / restore) -
/// the `OR AH,0xc` sets the x87 rounding control to `11b` = truncate toward zero, i.e. a plain `(int)`
/// cast - while `MenuMusicHandler_update.c` renders the same idiom as `ROUND(...)`: Ghidra doesn't model
/// the control-word change and renders every `FISTP` as round-to-nearest. The `.asm` is ground truth, so
/// this is a truncate, not a round - they differ for every odd `delta` (1999 * 0.5 = 999.5 -> 999, not
/// 1000), which is exactly what the unit tests below pin. Rust's `f64 as i32` truncates toward zero,
/// matching `FISTP`-chop for every value this can see (`delta < 2000` at the only call site, so the
/// result is far inside `i32` range).
fn fade_increment(delta: u32) -> i32 {
    (delta as f64 * 0.5) as i32
}

/// A real, vanilla-allocator-owned `std::string` - `{char* ptr; u32 len; u32 capacity}`, 12 bytes. This
/// build's std::string has no small-string-optimization buffer (confirmed directly from
/// `MenuMusicHandler_MenuMusicHandler.asm`'s stack-slot accounting: each of the constructor's two
/// temporary strings occupies exactly 3 stack dwords, matching this layout exactly), so it always
/// allocates its character buffer on the heap - built and torn down through the real vanilla
/// constructor/destructor (`std_basic_string::BASIC_STRING_2`/`BASIC_STRING_0`) rather than a hand-rolled
/// stand-in, matching `ztgamemgr-implementation-plan.md`'s `VanillaTagString` precedent (live-tested, then
/// reverted for unrelated reasons - see that plan's `removedZooDoo` section). Never write `ptr`/`len`/
/// `capacity` directly from Rust: construction and destruction always go through vanilla's own allocator,
/// so a Rust-side write here would risk exactly the cross-allocator hazard `CLAUDE.md` warns about.
#[repr(C)]
struct VanillaString {
    ptr: *mut u8,
    len: u32,
    capacity: u32,
}

impl VanillaString {
    /// Constructs from an iterator range `[str.as_ptr(), str.as_ptr() + str.len())`, matching
    /// `BASIC_STRING_2`'s real vanilla calling convention exactly (`this, first, last, allocator` - the
    /// trailing allocator argument is an uninitialized/unused stack slot in vanilla's own call site too,
    /// passed as `0` here).
    fn new(s: &str) -> Self {
        let mut this = VanillaString { ptr: std::ptr::null_mut(), len: 0, capacity: 0 };
        let start = s.as_ptr();
        let end = unsafe { start.add(s.len()) };
        unsafe {
            BASIC_STRING_2.original()(&mut this as *mut VanillaString as *const c_void, start as *const u32, end as i32, 0);
        }
        this
    }

    fn as_ptr(&self) -> *const u32 {
        self as *const VanillaString as *const u32
    }
}

impl Drop for VanillaString {
    fn drop(&mut self) {
        unsafe { BASIC_STRING_0.original()(self as *mut VanillaString as *const c_void) };
    }
}

#[detour_mod]
mod menu_music_handler_detours {
    use super::*;

    #[detour(CONSTRUCTOR)]
    unsafe extern "fastcall" fn constructor(this: *const u32) -> *const u32 {
        unsafe { mut_from_memory::<MenuMusicHandler>(this) }.construct();
        this
    }

    #[detour(INIT)]
    unsafe extern "thiscall" fn init(this: *const u32, filename: u32, attenuation: i32) -> u32 {
        unsafe { mut_from_memory::<MenuMusicHandler>(this) }.init(filename as *const i8, attenuation) as u32
    }

    #[detour(START_PLAY)]
    unsafe extern "thiscall" fn start_play(this: *const u32) {
        unsafe { mut_from_memory::<MenuMusicHandler>(this) }.start_play();
    }

    #[detour(START_FADE)]
    unsafe extern "fastcall" fn start_fade(this: *const u32) {
        unsafe { mut_from_memory::<MenuMusicHandler>(this) }.start_fade();
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, delta: u32) {
        unsafe { mut_from_memory::<MenuMusicHandler>(this) }.update(delta);
    }

    /// Live-test access to the real vanilla bodies and to the detours' installation state. Once
    /// `init_detours()` has patched these five addresses, `.original()` on them re-enters the Rust
    /// detours above instead of reaching vanilla in release builds (it's a raw address cast there -
    /// the exact trap `reimplementation_tests::init()`'s ztawardmgr comment documents); debug builds
    /// route `.original()` through the registry's trampolines, but these `*_DETOUR.call` trampolines
    /// stay because the battery's "real vanilla" pole must be genuine vanilla in **every** profile.
    /// This lives inside the detour module because the generated `*_DETOUR` statics are
    /// module-private.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn constructor(this: *const u32) -> *const u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) fn init(this: *const u32, filename: u32, attenuation: i32) -> u32 {
            unsafe { super::INIT_DETOUR.call(this, filename, attenuation) }
        }

        pub(crate) fn start_play(this: *const u32) {
            unsafe { super::START_PLAY_DETOUR.call(this) }
        }

        pub(crate) fn start_fade(this: *const u32) {
            unsafe { super::START_FADE_DETOUR.call(this) }
        }

        pub(crate) fn update(this: *const u32, delta: u32) {
            unsafe { super::UPDATE_DETOUR.call(this, delta) }
        }

        /// `(name, is_enabled)` per detour - the battery asserts all five to catch a silently-failed
        /// `init_detours()` (error logged, game continues on vanilla).
        pub(crate) fn status() -> [(&'static str, bool); 5] {
            [
                ("CONSTRUCTOR", super::CONSTRUCTOR_DETOUR.is_enabled()),
                ("INIT", super::INIT_DETOUR.is_enabled()),
                ("START_PLAY", super::START_PLAY_DETOUR.is_enabled()),
                ("START_FADE", super::START_FADE_DETOUR.is_enabled()),
                ("UPDATE", super::UPDATE_DETOUR.is_enabled()),
            ]
        }
    }
}

/// Registers this module's live detours. Does **not** detour the destructor - there is no destructor
/// function to detour: `MENU_MUSIC_HANDLER_0` in `generated.rs` (`0x00504e27`) is actually `~ZTGameMgr`'s
/// tail-merge block containing the inlined dtor (see the module doc comment and the implementation plan's
/// Stage 5), so hooking or calling it would run mid-`~ZTGameMgr` code under a wrong register contract.
pub fn init() {
    if let Err(e) = unsafe { menu_music_handler_detours::init_detours() } {
        error!("Failed to initialise ztgamemgr_menumusichandler detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::standalone::OPERATOR_DELETE;

    use super::*;

    /// Allocates a fresh, uninitialized `0x14`-byte block via the real vanilla allocator - callers must
    /// run either the real constructor (`CONSTRUCTOR.original()`) or [`MenuMusicHandler::construct`] on it
    /// before reading any field.
    pub(crate) fn allocate_uninitialized() -> *mut MenuMusicHandler {
        unsafe { OPERATOR_NEW.original()(0x14) as *mut MenuMusicHandler }
    }

    /// Frees a standalone instance built via [`allocate_uninitialized`] plus (real or reimplemented)
    /// construction, **without calling any vanilla destructor path**. There is no real vanilla
    /// `MenuMusicHandler` destructor to call: the export Ghidra labels `~MenuMusicHandler`
    /// (`generated.rs`'s `MENU_MUSIC_HANDLER_0`, `0x00504e27`) is actually `~ZTGameMgr`'s tail-merge
    /// block with the dtor inlined (plan Stage 5) - its first lines do the real, structurally-sound
    /// `sound_ptr` teardown (`if (sound_ptr) { if (isPlaying()) stop(); release(); }`), but the rest is
    /// `~ZTGameMgr`'s own tail (`operator_delete` on `EDI` = the handler pointer, a `BFMgr` vtable store
    /// through `ESI` = `~ZTGameMgr`'s `this`, entered via `~ZTGameMgr`'s `JMP`/`JZ` with its own stack
    /// layout). Calling `.original()` on this address outside that context would run `~ZTGameMgr`'s tail
    /// against whatever `EDI`/`ESI` happen to hold - unpredictable memory corruption - so this helper
    /// never does. Safe only because every caller in this module's own tests goes through `construct()`,
    /// which never calls `init()` and therefore always leaves `sound_ptr` at `0` - tests that *do*
    /// exercise `init()` must tear down through [`destroy_standalone_after_init`] instead (mirroring the
    /// export's *sound* teardown lines, not its `~ZTGameMgr` tail).
    pub(crate) fn destroy_standalone(ptr: *mut MenuMusicHandler) {
        if ptr.is_null() {
            return;
        }
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Frees a standalone instance built via [`allocate_uninitialized`] plus construction **and** a
    /// successful call to [`MenuMusicHandler::init`] (real or reimplemented) - both sides allocate
    /// `sound_ptr`'s `SNDSound` through the same real vanilla `OPERATOR_NEW`, so it's always safe to
    /// release it through real vanilla `SNDSound`'s own slot-0 `release(1)` idiom (the same call
    /// `MenuMusicHandler::init` itself uses to tear down a pre-existing sound) before freeing the outer
    /// block - like [`destroy_standalone`], this never touches the misattributed `MENU_MUSIC_HANDLER_0`
    /// export (really `~ZTGameMgr`'s tail-merge block - see [`destroy_standalone`]).
    pub(crate) fn destroy_standalone_after_init(ptr: *mut MenuMusicHandler) {
        if ptr.is_null() {
            return;
        }
        let sound_ptr = unsafe { (*ptr).sound_ptr };
        if sound_ptr != 0 {
            unsafe { SNDSOUND_1.original()(sound_ptr as *const u32, 1) };
        }
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Trampolines to the real vanilla bodies for the battery's "real vanilla" pole once
    /// `reimplementation_tests::init()` has installed this module's detours - `.original()` on the
    /// five hooked addresses is a raw cast in release, so it would re-enter the Rust detours there
    /// (debug `.original()` routes correctly, but the vanilla pole must hold in every profile - see
    /// `menu_music_handler_detours::test_real`'s doc comment).
    pub(crate) fn real_constructor(this: *const u32) -> *const u32 {
        menu_music_handler_detours::test_real::constructor(this)
    }

    pub(crate) fn real_init(this: *const u32, filename: u32, attenuation: i32) -> u32 {
        menu_music_handler_detours::test_real::init(this, filename, attenuation)
    }

    pub(crate) fn real_start_play(this: *const u32) {
        menu_music_handler_detours::test_real::start_play(this)
    }

    pub(crate) fn real_start_fade(this: *const u32) {
        menu_music_handler_detours::test_real::start_fade(this)
    }

    pub(crate) fn real_update(this: *const u32, delta: u32) {
        menu_music_handler_detours::test_real::update(this, delta)
    }

    /// `(name, is_enabled)` per detour - see `menu_music_handler_detours::test_real::status`.
    pub(crate) fn detour_status() -> [(&'static str, bool); 5] {
        menu_music_handler_detours::test_real::status()
    }
}

#[cfg(test)]
mod tests {
    use super::fade_increment;

    /// Pins [`fade_increment`]'s truncate-toward-zero semantics against `MenuMusicHandler_update.c`'s
    /// `ROUND(...)` misrendering - the odd-`delta` cases are the discriminators (a real `.round()` would
    /// produce 1 and 1000 for the middle two), per the helper's own doc comment.
    #[test]
    fn fade_increment_truncates_toward_zero() {
        assert_eq!(fade_increment(0), 0);
        assert_eq!(fade_increment(1), 0);
        assert_eq!(fade_increment(2), 1);
        assert_eq!(fade_increment(1999), 999);
        assert_eq!(fade_increment(2000), 1000);
    }
}
