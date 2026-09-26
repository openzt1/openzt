//! `ZTGameMgr` reimplementation. Follows a third named pattern variant, distinct from
//! `ztadvterrainmgr.rs`'s "thin-shell whole-class" call-through and `ztmegatilemgr.rs`'s "100%
//! vanilla-owned" style: **per-method delegation to an embedded sub-object**.
//! `ZTGameMgr`'s own vtable/non-virtual logic is fully ported to Rust, and each method that needs the
//! `ZooStatus` finance/rating tracker embedded inline at `this+0x10` (confirmed via
//! `_CreateZTGameMgr.c`'s `OOAnalyzer::ZooStatus::init((ZooStatus *)(puVar2 + 4),...)`) calls directly
//! into the reimplemented `impl ZooStatus` methods in `zoostatus.rs`. Because `ZTGameMgr` stays
//! vanilla-layout-compatible and `ZooStatus` lives inline in the *same* memory block rather than a
//! separate allocation, reading/writing that live memory from within a Rust `ZTGameMgr` method body is
//! always safe - nothing is freed or reallocated in place, so none of `CLAUDE.md`'s cross-allocator
//! hazards apply here.
//!
//! No dynamic containers live in `ZTGameMgr`'s own memory - only scalars and fixed-size arrays - so the
//! vanilla-layout-compatible struct below never needs to model a `Vec`/map/tree. (The embedded
//! `ZooStatus` region's own array fields are modeled field-by-field in `zoostatus.rs`.)
//!
//! # Construction/destruction (`plans/ztgamemgr-vanilla-storage-migration-plan.md`)
//!
//! Unlike every other method below, `CREATE_ZTGAME_MGR`/`DESTRUCTOR_0`/`DESTRUCTOR_1` **are** detoured
//! (`gamemgr_allocator_detours`, that module's own doc comment has the full detail) - Stage 5 of the
//! migration plan swaps `GLOBAL_ZTGameMgr`'s own allocator from vanilla `operator_new`/`operator_delete`
//! to Rust's global allocator (`Box`), confirmed safe by two independent live `cdb` traces plus a full
//! decompile-corpus sweep each finding exactly one construct and one destroy per process.
//!
//! # Methods deliberately not detoured
//!
//! Each item is confirmed against the decompiles (`private/resources/decompiles/ZTGameMgr_*`):
//!
//! - **`removedZooDoo`**: not ported. The logic itself is comprehensible and portable in principle -
//!   real entry point `0x004a2c98` (`generated.rs`'s `ztgamemgr::REMOVED_ZOO_DOO` carries the corrected
//!   address/signature; the mangled 11-parameter export Ghidra first produced for it came from analysis
//!   starting at an internal `JMP` target, `0x004a2ee1`, mistaken for the function boundary - and
//!   `ztworldmgr::GET_BUILDING_LIST`'s entry is a genuine `thiscall`, both confirmed via `.asm`
//!   tracing): a tile-distance search over `ZTWorldMgr::getBuildingList("compost")`,
//!   `ZTBuilding::receiveIncome`, `ZooStatus::refundConstruction`/`addCash`. The only known-working,
//!   live-tested call shape builds the "compost" tag string through a real vanilla `std::string`
//!   constructor/destructor; a simpler, self-owned (non-vanilla-allocated) string reproducibly crashes
//!   `getBuildingList` for a reason never root-caused. A working path depending on unexplained vanilla
//!   behavior, on top of an already-nontrivial chain of hand-derived ABI facts (parameter order,
//!   list-node layout, a small-object-free address), is too much unverified surface, so the port is not
//!   wired up. The `.asm`-traced corrections above remain ground truth for whoever revisits this.
//! - **`gotoStart(...)`**: `ZTGameMgr_gotoStart.c` is genuinely decompiler-mangled, not just verbose -
//!   `unaff_EBX`/`unaff_ESI`/`unaff_EDI` register-allocation artifacts stand in for real
//!   parameters/locals, and the recovered signature (14 params, mostly untyped `undefined`) doesn't
//!   match any real call site. `generated.rs`'s own `GOTO_START` entry (`u8×12, u32×2`) reflects the
//!   same automatic-signature-recovery confusion. Not faithfully portable from this decompile - left
//!   un-detoured.
//! - **`startMenuMusic()`/`startMenuMusicFade()`**: `startMenuMusicFade` compiles to two distinct
//!   calling-convention instances of the *same* logical method - `_0` (thiscall, `0x004c9d67`) and `_2`
//!   (fastcall, `0x004cc59d`) have identical trivial bodies (forward to `MenuMusicHandler::startFade`
//!   when `menu_music_handler_ptr != 0`), and both `.meta`s list `startFade` as a called function - not
//!   duplicates worth deduping here. `_1` (fastcall, `0x004ca478`) is a **different function**
//!   entirely: its body directly implements a vtable dispatch (`(**(code**)(*vtable+0x50))()`) plus a
//!   fade-state flag/counter reset, calls nothing named (empty `calling_functions` in its `.meta`,
//!   unlike `_0`/`_2`), and is almost certainly `MenuMusicHandler::startFade`'s own real body,
//!   mislabeled with the `ZTGameMgr::` name by the decompile corpus's automated naming pass. The
//!   wrappers stay un-detoured regardless: they are pure call-throughs whose target address
//!   `ztgamemgr_menumusichandler.rs` already detours, so vanilla callers already reach the
//!   reimplemented logic through them, and the single-address `startMenuMusic()` additionally calls
//!   `initMenuMusic` (below).
//! - **`initMenuMusic()`**: confirmed Windows address (`ztgamemgr::INIT_MENU_MUSIC`, `0x00521e18` - the
//!   same address the `startMenuMusic` decompile's `FUN_00521e18` lead points at). Constructs a
//!   `BFIniFile` (still an untouched dependency) in addition to a `MenuMusicHandler` (itself
//!   reimplemented, `ztgamemgr_menumusichandler.rs`); `BFIniFile` is what keeps it un-detoured.
//!
//! `menuMusicAttenToScrollbarVal`/`scrollbarValToMenuMusicAtten` (macOS-only, no Windows address)
//! also remain out of scope - see `menu_music_max_attenuation` in the struct below.

use std::ffi::c_void;

use openzt_detour::generated::{
    bfscenariomgr::{GET_CROWD_AMBIENTS_NAME, GET_CROWD_CONFIG_NAME, GET_WORLD_AMBIENTS_NAME, GET_WORLD_CONFIG_NAME},
    standalone::{DEALLOCATE, OPERATOR_DELETE, OPERATOR_NEW, WRITE_BYTES_TO_FILE},
    ztsoundscape::DESTRUCTOR as ZTSOUNDSCAPE_DESTRUCTOR,
    ztui_main::{
        SET_ANIMAL_RATING as ZTUI_MAIN_SET_ANIMAL_RATING, SET_DATE_TEXT as ZTUI_MAIN_SET_DATE_TEXT, SET_GUEST_RATING as ZTUI_MAIN_SET_GUEST_RATING,
        SET_MONEY_TEXT as ZTUI_MAIN_SET_MONEY_TEXT, SET_ZOO_RATING as ZTUI_MAIN_SET_ZOO_RATING, UNPAUSE_GAME as ZTUI_MAIN_UNPAUSE_GAME,
    },
    bfaimgr::LOAD_DATA as BFAIMGR_LOAD_DATA,
    bfinifile::READ,
};
use tracing::info;
use windows::Win32::{
    Foundation::{FILETIME, SYSTEMTIME},
    System::{SystemInformation::GetSystemTime, Time::{FileTimeToSystemTime, SystemTimeToFileTime}},
};

use crate::{
    command_console::CommandError,
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, mut_from_memory, ref_from_memory, save_to_memory},
    ztgame::menu_music_handler::MenuMusicHandler,
    ztsoundscape::ZTSoundscape,
    zoostatus::{self, ZooStatus},
};
use crate::systemtime::{filetime_to_ticks, ticks_to_filetime, Systemtime};
use crate::vanilla_string::VanillaString;

/// `DAT_006394b8`'s RVA (Ghidra VA `0x006394b8` minus the default load base `0x400000`) - a raw, signed
/// tick accumulator `ZTGameMgr::updateSim` reads/writes directly (not a pointer, so no
/// `CachedGlobalInstance` chain-walk needed - just `get_module_base("zoo.exe") + RVA` each call, same
/// pattern as this codebase's other raw-global accesses, e.g. `ztresearch.rs`).
const DAT_006394B8_RVA: u32 = 0x006394b8 - 0x400000;

/// `GLOBAL_ZTScenarioMgr`'s RVA - a raw pointer-typed global (one dereference gives the live
/// `ZTScenarioMgr*` singleton), read by [`ZTGameMgr::start`] as the `this` for
/// `BFScenarioMgr::getCrowdAmbientsName`/`getWorldAmbientsName`/`getCrowdConfigName`/
/// `getWorldConfigName` (`ZTGameMgr_start.asm`'s `MOV ECX, dword ptr GLOBAL_ZTScenarioMgr` before each
/// call). Same one-level-of-indirection shape as `GLOBAL_ZTApp` below - neither is a `CachedGlobalInstance`
/// entry in `globals.rs` since both are single-purpose to this module's `start`/`stop` port, not shared
/// elsewhere yet.
const GLOBAL_ZTSCENARIOMGR_RVA: u32 = 0x00638ff8 - 0x400000;

/// `GLOBAL_ZTApp`'s RVA - a raw pointer-typed global (one dereference gives the live `ZTApp*` singleton),
/// read by [`ZTGameMgr::stop`] to check its `+0x440` byte field (`appInitSuccess`) before tail-calling
/// `ZTUI::main::unpauseGame` (`ZTGameMgr_stop.asm`'s `MOV EAX, GLOBAL_ZTApp` / `MOV CL, byte ptr [EAX +
/// 0x440]` / `JNZ main::unpauseGame`). See [`ZTGameMgr::stop`]'s own doc comment for why the real body's
/// "if null, lazily assign a bogus function-pointer sentinel" defensive branch is deliberately not
/// reproduced here.
const GLOBAL_ZTAPP_RVA: u32 = 0x00638154 - 0x400000;

/// ZTGameMgr struct. Real allocation size `0x11b0` (`_CreateZTGameMgr.c`, `operator_new(0x11b0)`).
#[derive(Debug)]
#[repr(C)]
pub struct ZTGameMgr {
    vtable: u32, // 0x0
    /// `start`/`stop`/`gotoStart`/`~ZTGameMgr`'s own "already started" guard flag
    /// (`if (*(char*)(this+4) != 0) stop(this);`). Explicitly zeroed by `CreateZTGameMgr`
    /// (`*(undefined1 *)(puVar2 + 1) = 0;`).
    started: bool, // 0x4
    _pad1b: [u8; 3],
    /// `BFGameMgr::save`/`load`/`setNewGameDefaults`/`updateSim`'s own raw elapsed-simulation-ticks
    /// accumulator (`this->mbr_0x8 += param_1` every `updateSim` call, reset to `0` in
    /// `setNewGameDefaults`).
    elapsed_sim_ticks: u32, // 0x8
    cash: f32,                // 0x0C
    pad2a: [u8; 0x28 - 0x10], // 0x10
    /// Set to `true` by `updateSim` when the game date's `w_month` field changes across its
    /// `FILETIME` round-trip (`ZTGameMgr_updateSim.c`/`.asm`: `this->field_0x28 = 1` when the
    /// `.asm`'s `word ptr [ESI+0x1196]` compare - `w_month` at `+2`, not `w_day_of_week` at
    /// `+4` - differs before/after). Never cleared by `updateSim` itself; whatever consumes it
    /// is out of scope for this reimplementation.
    day_changed_flag: bool,   // 0x28
    pad2b: [u8; 0x30 - 0x29], // 0x29
    num_animals: u16,              // 0x30
    pad3: [u8; 0x38 - 0x32],       // 0x30
    num_species: u16,              // 0x38
    pad4: [u8; 0x3C - 0x3A],       // 0x38
    num_tired_guests: u16,         // 0x3C
    pad5: [u8; 0x40 - 0x3E],       // 0x3C
    num_hungry_guests: u16,        // 0x40
    pad6: [u8; 0x44 - 0x42],       // 0x40
    num_thirst_guests: u16,        // 0x44
    pad7: [u8; 0x48 - 0x46],       // 0x44
    num_guests_restroom_need: u16, // 0x48
    pad8: [u8; 0x54 - 0x4A],       // 0x48
    /// A live guest-tile count from `ZooStatus::calculateSums`' world walk - see `zoostatus.rs`'s
    /// `ZooStatus::guest_tile_count` doc comment. Same underlying bytes as `ZooStatus`-relative
    /// `+0x44`, shifted to `ZTGameMgr`-relative `+0x54` by the `+0x10` embedding offset.
    guest_tile_count: i32, // 0x54
    // This pad also covers the embedded `ZooStatus`'s history/flat-totals array region - modeled
    // field-by-field in `zoostatus.rs` (`monthly_history`/`yearly_history`/`flat_totals`), left as pad
    // here since no `ZTGameMgr` method reads into it directly.
    pad9: [u8; 0x1160 - 0x58], // 0x58
    zoo_admission_cost: f32,   // 0x1160
    pad10: [u8; 0x1190 - 0x1164], // 0x1164 - includes `removedZooDoo`'s refund-per-item base amount at
                                   // `+0x117c` (`ZTGameMgr_removedZooDoo.c`/`.asm`), unnamed since that
                                   // method is not ported - see the module doc comment
    /// `ZTSoundscape*`, read/written by `start`/`stop`/`updateSim`/the destructor. Explicitly zeroed
    /// by `CreateZTGameMgr` (`puVar2[0x464] = 0;`, dword index `0x464` = byte offset `0x1190`).
    soundscape_ptr: u32, // 0x1190
    date: Systemtime,    // 0x1194
    /// `ZTGameMgr::MenuMusicHandler*`, read by `update`/`startMenuMusic`/`startMenuMusicFade`/the
    /// destructor. Explicitly zeroed by `CreateZTGameMgr` (`puVar2[0x469] = 0;`, dword index `0x469` =
    /// byte offset `0x11A4`).
    menu_music_handler_ptr: u32, // 0x11A4
    /// `BFIniFile::read("UI", "menuMusicMaxAttenuation", -1000)`'s result, computed by
    /// [`ZTGameMgr::construct`] (`_CreateZTGameMgr.c`'s `puVar2[0x46a] = uVar3;`). The macOS-only
    /// `menuMusicAttenToScrollbarVal`/`scrollbarValToMenuMusicAtten` (see the module doc comment) are its
    /// only readers/writers - still out of this reimplementation's scope, but the field's own default
    /// value's source is now known.
    menu_music_max_attenuation: i32, // 0x11A8
    pad11: [u8; 0x11b0 - 0x11AC], // 0x11AC - trailing unaccounted space
}

const _: () = assert!(std::mem::size_of::<ZTGameMgr>() == 0x11b0);

/// `BFGameMgr`'s real vtable VA (`private/docs/vtables/BFGameMgr.md`) - written into a freshly
/// allocated block by [`ZTGameMgr::init_fields`] before `ZooStatus::init` runs, then
/// immediately overwritten by [`ZTGAMEMGR_VTABLE`] once the base subobject's construction step
/// completes - genuine C++ base-then-derived construction order, not a redundant double-write. A raw
/// constant, not RVA'd - zoo.exe has no ASLR (same reasoning as this file's other RVA consts / vtable
/// constants elsewhere, e.g. `ztgamemgr_menumusichandler.rs`'s `SNDSOUND_VTABLE`).
const BFGAMEMGR_VTABLE: u32 = 0x006350d0;

/// `ZTGameMgr`'s own real vtable VA (`private/docs/vtables/ZTGameMgr.md`).
const ZTGAMEMGR_VTABLE: u32 = 0x00630b9c;

/// `BFMgr`'s real vtable VA (`private/docs/vtables/BFMgr.md`) - the final vtable state
/// [`ZTGameMgr::destruct`] leaves `self` in, continuing the tail-chain into `~BFMgr`'s own base-class
/// teardown (left as a permanent call-through, matching `ZTAdvTerrainMgr`/`ZTMegatileMgr`'s own
/// precedent for this level of the hierarchy - see [`ZTGameMgr::destruct`]'s doc comment).
const BFMGR_VTABLE: u32 = 0x006355c0;

/// The animal/guest UI rating formula `ZTGameMgr::updateSim` feeds `ZooStatus`'s raw metric through
/// before calling `ZTUI::main::set{Animal,Guest}Rating` - `0` outright if `population` (the corresponding
/// `num_animals`/`guest_tile_count` count, matching vanilla's own byte-identical read - see
/// `zoostatus.rs`) is `0`, otherwise
/// `(metric + 100) * 100 / 200` (via [`zoostatus::scaled_rating_metric`], shared with `ZooStatus::ratingChecks`'
/// own animal/guest score contributions - same formula, different scale constant). Pulled out as its own
/// pure function because the live `ZTGAMEMGR_UPDATE_SIM` comparison test can never actually exercise this
/// branch: it drives a standalone instance whose `delta` is bounded `0..=0x3e9` against a tick accumulator
/// reset to `0` immediately before each call, so the accumulator can only ever equal `delta` itself - never
/// enough to cross the `> 0x3e9` UI-refresh threshold this formula lives behind (see that test's own doc
/// comment in `reimplementation_tests/tests/ztgamemgr.rs`). Covered by a `#[cfg(test)]` unit test below
/// instead.
pub(crate) fn rating_from_metric(metric: i32, population: i32) -> i32 {
    if population == 0 {
        0
    } else {
        zoostatus::scaled_rating_metric(metric, 100)
    }
}

impl ZTGameMgr {
    /// enables or disables dev mode
    pub(crate) fn enable_dev_mode(enable: bool) {
        let enable_dev_mode_address = 0x63858A;
        unsafe {
            *(enable_dev_mode_address as *mut bool) = enable;
        }
    }

    /// The current budget, in dollars.
    pub fn cash(&self) -> f32 {
        self.cash
    }

    /// Exposed for the live `reimplementation_tests` comparison harness, to pin the real, live
    /// `ZTGameMgr` singleton's budget to a known value around a `ZTResearchBranch::update` comparison
    /// call - see `ztresearch::reimplementation_tests` support for why this writes the real singleton
    /// rather than a synthetic instance.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_cash(&mut self, value: f32) {
        self.cash = value;
    }

    /// Test-only accessors for the `ZTGAMEMGR_SAVE_LOAD` live test, letting it seed/read the
    /// three fields `save`/`load` actually touch (`cash`/`date`/`elapsed_sim_ticks`) without exposing
    /// the private `Systemtime` type outside this module.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_elapsed_sim_ticks(&mut self, value: u32) {
        self.elapsed_sim_ticks = value;
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn elapsed_sim_ticks(&self) -> u32 {
        self.elapsed_sim_ticks
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_date_bytes(&mut self, bytes: [u8; 0x10]) {
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), &mut self.date as *mut Systemtime as *mut u8, 0x10) };
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn date_bytes(&self) -> [u8; 0x10] {
        let mut out = [0u8; 0x10];
        unsafe { std::ptr::copy_nonoverlapping(&self.date as *const Systemtime as *const u8, out.as_mut_ptr(), 0x10) };
        out
    }

    /// Ports `ZTGameMgr::subtractCash` (`ZTGameMgr_subtractCash.c`): subtracts `amount`
    /// from the budget, then refreshes the on-screen money display (`ZTUI::main::setMoneyText`). Used
    /// by `ztresearch::ZTResearchBranch::update`'s native reimplementation of the branch funding cost,
    /// among other callers. Vanilla-side callers of the real function address (not just Rust API
    /// callers) route through this same logic via the `SUBTRACT_CASH` detour below. The real signature
    /// has a trailing, unread `bool` parameter (`ZTGameMgr::subtractCash(float,
    /// bool)`, per the `.asm`'s `RET 8`) - not part of this method's own logic, so not part of this
    /// method's own signature either; the detour wrapper below supplies/discards it.
    pub fn subtract_cash(&mut self, amount: f32) {
        self.cash -= amount;
        unsafe { ZTUI_MAIN_SET_MONEY_TEXT.original()() };
    }

    /// Ports `ZTGameMgr::addCash` (`ZTGameMgr_addCash.c`): adds `amount` to the budget,
    /// then refreshes the on-screen money display (`ZTUI::main::setMoneyText`).
    pub fn add_cash(&mut self, amount: f32) {
        self.cash += amount;
        unsafe { ZTUI_MAIN_SET_MONEY_TEXT.original()() };
    }

    /// Ports `ZTGameMgr::getDate` (`ZTGameMgr_getDate.c`/`.asm`): converts `date` to a
    /// `FILETIME` via `SystemTimeToFileTime` and returns it as a raw 64-bit tick count. Per the real
    /// body, a conversion failure is never checked - the decompile just proceeds with whatever ended up
    /// in the (potentially-uninitialized) local `FILETIME`. This port can't reproduce genuine stack
    /// garbage, so it substitutes a zeroed `FILETIME` on failure instead (same reasoning as
    /// `update_sim`'s own `SystemTimeToFileTime` call, which ignores failure identically).
    pub fn get_date(&self) -> u64 {
        let mut file_time = FILETIME::default();
        let _ = unsafe { SystemTimeToFileTime(&self.date.to_win32(), &mut file_time) };
        filetime_to_ticks(file_time)
    }

    /// Ports `ZTGameMgr::isGameDate` (`ZTGameMgr_isGameDate.c`/`.asm` - the `.asm` is the
    /// clean read; the `.c`'s messy `CONCAT`/register-reuse noise around the return value is decompiler
    /// artifact, not real extra logic). Round-trips `get_date()` back through `FileTimeToSystemTime` and
    /// compares `day`/`month` against the result, `0xffffffff` acting as a per-field wildcard. Returns
    /// `false` outright if the round-trip fails. Real parameter order is `(day, month)`, confirmed
    /// independently from both the `.c`'s offset math (`local_10._6_4_` = `wDay` compared against
    /// `param_1`; `local_10._2_4_` = `wMonth` compared against `param_2`) and the `.asm`'s stack-offset
    /// reads.
    pub fn is_game_date(&self, day: u32, month: u32) -> bool {
        let file_time = ticks_to_filetime(self.get_date());
        let mut sys_time = SYSTEMTIME::default();
        if unsafe { FileTimeToSystemTime(&file_time, &mut sys_time) }.is_err() {
            return false;
        }
        (day == 0xffffffff || sys_time.wDay as u32 == day) && (month == 0xffffffff || sys_time.wMonth as u32 == month)
    }

    /// Ports `ZTGameMgr::isRealWorldDate` (`ZTGameMgr_isRealWorldDate.c`). Unlike
    /// `is_game_date`, this has no `this` dependency at all (confirmed by its `stdcall`/no-`this`
    /// `IS_REAL_WORLD_DATE` entry) - it calls `GetSystemTime` directly and has no `0xffffffff` wildcard
    /// handling. Real parameter order is `(day, month)`, same as [`Self::is_game_date`].
    pub fn is_real_world_date(day: u32, month: u32) -> bool {
        let sys_time = unsafe { GetSystemTime() };
        sys_time.wDay as u32 == day && sys_time.wMonth as u32 == month
    }

    /// Ports `ZTGameMgr::timeAgo` (`ZTGameMgr_timeAgo.c`/`.asm`): `get_date() - reference`
    /// as a 64-bit subtraction (the real body's `SUB`/`SBB` pair over the two `FILETIME` dwords is
    /// exactly a wrapping `u64` subtraction).
    pub fn time_ago(&self, reference: u64) -> u64 {
        self.get_date().wrapping_sub(reference)
    }

    /// Ports `ZTGameMgr::hoursAgo` (`ZTGameMgr_hoursAgo.c`/`.asm`): same `get_date() -
    /// reference` 64-bit subtraction as `time_ago`, divided by `36_000_000_000` (100ns intervals per
    /// hour) via the real body's own unsigned 64-bit division (`_aulldiv`). The real return is a
    /// full `u64` EDX:EAX register pair (see the `HOURS_AGO` entry's doc comment in `generated.rs`).
    pub fn hours_ago(&self, reference: u64) -> u64 {
        self.get_date().wrapping_sub(reference) / 36_000_000_000
    }

    /// Ports `ZTGameMgr::animalTimeAgo` (`ZTGameMgr_animalTimeAgo.c`/`.asm`), a genuine standalone
    /// Windows function at `0x00467a14` (own `RET 0x8`, not inlined into any caller). Buckets
    /// [`Self::hours_ago`]'s
    /// result into one of three values: `0` (< 1440 hours / 60 days), `1` (1440-8640h), or `2` (> 8640h /
    /// 360 days) - only **two** thresholds. **Not** the three-threshold/four-bucket shape
    /// [`Self::people_time_ago`] uses, despite both being named `*TimeAgo` and sharing the same
    /// `hoursAgo` base - confirmed independently per-function against each one's own `.asm` (`animalTimeAgo`:
    /// `CMP EAX,0x5a0` / `CMP EAX,0x21c0`, two compares; `peopleTimeAgo`: three). The real return is a
    /// register pair (`ulonglong`) whose low dword is the bucket and whose high dword is `hoursAgo`'s own
    /// leftover EDX half, not part of the bucket value - only the low dword is meaningful here, matching
    /// this codebase's established `TIME_AGO`/`HOURS_AGO` EDX:EAX convention.
    pub fn animal_time_ago(&self, reference: u64) -> u32 {
        let hours = (self.hours_ago(reference) as u32) as i32;
        if hours < 0x5a0 {
            0
        } else if hours > 0x21c0 {
            2
        } else {
            1
        }
    }

    /// Ports `ZTGameMgr::peopleTimeAgo` (`ZTGameMgr_peopleTimeAgo.c`/`.asm`), a genuine standalone
    /// Windows function at `0x0046a791`, same as [`Self::animal_time_ago`]'s doc comment notes for its
    /// own address. Same `hoursAgo` base, but **three**
    /// thresholds -> four buckets: `0` (<1440h), `1` (1440-5759h), `2` (5760-8639h), `3` (>8639h) - this
    /// is the "1440/5760/8640 = 60/240/360 days" shape, distinct from `animal_time_ago`'s own two-bucket
    /// shape.
    pub fn people_time_ago(&self, reference: u64) -> u32 {
        let hours = (self.hours_ago(reference) as u32) as i32;
        if hours < 0x5a0 {
            0
        } else if hours <= 0x167f {
            1
        } else if hours <= 0x21bf {
            2
        } else {
            3
        }
    }

    /// Calls the vanilla `ZooStatus::spendResearch` on the embedded `ZooStatus` finance-tracker at
    /// `self + 0x10` (per `resources/decompiles/ZTResearchBranch_update.c`, which calls it right
    /// before `subtractCash`; the sibling `ZTMarketing::update` confirms the same `&GameMgr->field_0x10`
    /// `ZooStatus` sub-object at the exact same call shape with its own `spendMarketing`). Per
    /// `resources/decompiles/ZooStatus_spendResearch.c`, this only ever writes running-total fields on
    /// `this` itself - no further calls, no other side effects. Used by
    /// `ztresearch::ZTResearchBranch::update`'s native reimplementation, called before `subtract_cash`
    /// to match vanilla's own call order.
    pub fn spend_research(&mut self, amount: f32) {
        let zoostatus_ptr = (self as *mut Self as u32 + 0x10) as *const u32;
        unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.spend_research(amount)
    }

    /// Calls the reimplemented `ZooStatus::spendMarketing` on the same embedded `ZooStatus` sub-object as
    /// `spend_research`. Used by `ztmarketing::ZTMarketing::update`, called before `subtract_cash` to
    /// match vanilla's own call order.
    pub fn spend_marketing(&mut self, amount: f32) {
        let zoostatus_ptr = (self as *mut Self as u32 + 0x10) as *const u32;
        unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.spend_marketing(amount)
    }

    /// Ports `ZTGameMgr::setNewGameDefaults` (vtable `+0x4`), with `BFGameMgr::setNewGameDefaults`'s own
    /// base-class body (just zeroing `elapsed_sim_ticks`) inlined directly - see the module doc comment
    /// for why there's no separate `BFGameMgr`-vs-`ZTGameMgr` split kept in this port.
    ///
    /// Per the decompile/`.asm` (`ZTGameMgr_setNewGameDefaults.c`/`.asm`), the real order is (the first
    /// write is `[this+0xc] = 0`, i.e. `cash`, not `elapsed_sim_ticks` - confirmed independently by both
    /// the Windows `LEA ECX,[ESI+0xc]` / `MOV [ECX],EAX` `.asm` and the macOS decompile's
    /// `ZTEcon__init((float *)(param_1 + 0xc))`; `elapsed_sim_ticks` is only ever zeroed once, at the
    /// very end):
    /// 1. `cash = 0.0`
    /// 2. `ZooStatus::init(&self.zoo_status, config)` (reimplemented, embedded sub-object)
    /// 3. set `date` to the hardcoded new-game default (2001-01-01, a Monday, 00:00:00.000)
    /// 4. if `is_new_game`: call through `GLOBAL_ZTAIMgr`'s real vtable slot `+0x4` (`0x0058f269`,
    ///    inherited unchanged from `BFAIMgr` - see `private/docs/vtables/BFAIMgr.md`), identified by the
    ///    Ghidra pass as `BFAIMgr::loadData` (`thiscall fn(this, bool) -> u32`), with `false`, matching
    ///    the real thiscall/1-arg shape confirmed by both this function's and `_setCursorQuality`'s own
    ///    `.asm`
    /// 5. `ZooStatus::ratingChecks(&self.zoo_status)` (reimplemented, embedded sub-object)
    /// 6. `elapsed_sim_ticks = 0`
    pub fn set_new_game_defaults(&mut self, config: *const u32, is_new_game: bool) {
        self.cash = 0.0;

        let zoostatus_ptr = (self as *mut Self as u32 + 0x10) as *const u32;
        unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.init(config as *const c_void);

        self.date = Systemtime {
            w_year: 0x7d1,
            w_month: 1,
            w_day_of_week: 1,
            w_day: 1,
            w_hour: 0,
            w_minute: 0,
            w_second: 0,
            w_milliseconds: 0,
        };

        if is_new_game {
            unsafe { BFAIMGR_LOAD_DATA.original()(globals().ztaimgr_ptr(), false) };
        }

        unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.rating_checks();

        self.elapsed_sim_ticks = 0;
    }

    /// Ports `ZTGameMgr::overrideNewGameDefaults` (`ZTGameMgr_overrideNewGameDefaults.c`/`.asm`), a
    /// genuine standalone Windows function at `0x00591a40` (own `RET 0x4`, not inlined into any caller).
    /// One-line call-through to the
    /// reimplemented `ZooStatus::override` on the embedded sub-object at `self+0x10`, exactly
    /// the same shape as [`Self::spend_research`]/[`Self::spend_marketing`].
    pub fn override_new_game_defaults(&mut self, config: *const u32) {
        let zoostatus_ptr = (self as *mut Self as u32 + 0x10) as *const u32;
        unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.override_config(config as *const c_void)
    }

    /// Ports `ZTGameMgr::save` (vtable `+0x8`), with `BFGameMgr::save`'s own base-class body (just
    /// writing `elapsed_sim_ticks`) inlined directly - see the module doc comment for why there's no
    /// separate `BFGameMgr`-vs-`ZTGameMgr` split kept in this port.
    ///
    /// Per the decompile (`ZTGameMgr_save.c`), write order is: a fixed `0` marker dword
    /// (`local_8` - pure format padding, no real state); `ZooStatus::save` on the embedded sub-object at
    /// `self+0x10` (reimplemented); `date` raw (16 bytes); `cash` (captured into a local
    /// before the marker write in the decompile, but reading it here instead is equivalent - nothing in
    /// between can mutate `cash`); then chain to base (just `elapsed_sim_ticks`). Every step's success is
    /// ANDed together, matching `ztawardmgr.rs`'s own `save`.
    pub fn save(&self, file: *const u32) -> bool {
        let marker: u32 = 0;
        let mut ok = unsafe { WRITE_BYTES_TO_FILE.hooked()(&marker as *const u32, 4, 1, file as *const i8) } == 1;

        let zoostatus_ptr = (self as *const Self as u32 + 0x10) as *const u32;
        let zoostatus_result = unsafe { ref_from_memory::<ZooStatus>(zoostatus_ptr) }.save(file as *const i8);
        ok &= zoostatus_result == 1;

        ok &= unsafe { WRITE_BYTES_TO_FILE.hooked()(&self.date as *const Systemtime as *const u32, 0x10, 1, file as *const i8) } == 1;

        ok &= unsafe { WRITE_BYTES_TO_FILE.hooked()(&self.cash as *const f32 as *const u32, 4, 1, file as *const i8) } == 1;

        // BFGameMgr::save inlined: writes the raw elapsed_sim_ticks dword.
        ok &= unsafe { WRITE_BYTES_TO_FILE.hooked()(&self.elapsed_sim_ticks as *const u32, 4, 1, file as *const i8) } == 1;

        ok
    }

    /// Ports `ZTGameMgr::load` (vtable `+0xc`), with `BFGameMgr::load`'s own base-class body inlined
    /// directly (see [`Self::save`]'s doc comment for why).
    ///
    /// Per the decompile (`ZTGameMgr_load.c`), read order mirrors `save`'s write order: a
    /// `0` marker dword (read and discarded - only its success/failure matters) -> `ZooStatus::load`
    /// (reimplemented, embedded sub-object) -> `date` raw (16 bytes, written directly into
    /// `self.date` regardless of what happens next, matching the decompile's direct-into-field read) ->
    /// `cash` (read into a local first). **`cash` is only assigned from that local after every earlier
    /// read has succeeded** - matching the decompile's `if (bVar4 != 0) { this->field_0xc = local_8; ...
    /// }` gating: if the marker or `ZooStatus::load` fails, this returns `false` immediately without
    /// touching `cash` or even attempting the `date`/`cash` reads; if `date` or `cash` fails, `cash` is
    /// left untouched (but `date` may already have been partially overwritten, exactly as vanilla's own
    /// read-in-place would leave it).
    ///
    /// `BFGameMgr::load`'s own inlined base body (`BFGameMgr_load.c`) only reads `elapsed_sim_ticks` when
    /// `version > 0x48`; older saves leave it zeroed instead.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        let mut marker: u32 = 0;
        let marker_ok = unsafe { DEALLOCATE.hooked()(&mut marker as *mut u32 as *const u32, 4, 1, file as *const u8) } == 1;
        if !marker_ok {
            return false;
        }

        let zoostatus_ptr = (self as *mut Self as u32 + 0x10) as *const u32;
        let zoostatus_result = unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.load(file, version);
        if !low_byte_bool(zoostatus_result) {
            return false;
        }

        let date_ok = unsafe { DEALLOCATE.hooked()(&mut self.date as *mut Systemtime as *const u32, 0x10, 1, file as *const u8) } == 1;
        let mut cash: f32 = 0.0;
        let cash_ok = unsafe { DEALLOCATE.hooked()(&mut cash as *mut f32 as *const u32, 4, 1, file as *const u8) } == 1;

        if !(date_ok && cash_ok) {
            return false;
        }

        self.cash = cash;

        // BFGameMgr::load inlined: only reads elapsed_sim_ticks for saves newer than version 0x48.
        if version > 0x48 {
            unsafe { DEALLOCATE.hooked()(&mut self.elapsed_sim_ticks as *mut u32 as *const u32, 4, 1, file as *const u8) == 1 }
        } else {
            self.elapsed_sim_ticks = 0;
            true
        }
    }

    /// Ports `ZTGameMgr::update` (vtable `+0x10`). Per the decompile (`ZTGameMgr_update.c`) this is a
    /// pure call-through to the embedded `MenuMusicHandler` when present - no logic of
    /// `ZTGameMgr`'s own. Calls the reimplemented [`MenuMusicHandler::update`] directly rather than
    /// going through `UPDATE`'s address (`.original()`/`.hooked()`): keeping the old address-based
    /// call-through would route around this caller's own reimplementation differently per hook state,
    /// while every other caller reaches the Rust detour through its hooked address - two paths that
    /// could diverge for no benefit.
    pub fn update(&self, delta: u32) {
        if self.menu_music_handler_ptr != 0 {
            unsafe { mut_from_memory::<MenuMusicHandler>(self.menu_music_handler_ptr) }.update(delta);
        }
    }

    /// Ports `ZTGameMgr::updateSim` (vtable `+0x14`). Per the decompile/`.asm` (`ZTGameMgr_updateSim.c`/
    /// `.asm`), in order:
    /// 1. `elapsed_sim_ticks += delta` (`BFGameMgr::updateSim`'s own base body, inlined - see the module
    ///    doc comment for why there's no separate `BFGameMgr`-vs-`ZTGameMgr` split kept in this port).
    /// 2. The raw global tick accumulator `DAT_006394b8` (`this->mbr_0x8`'s sibling, not part of
    ///    `ZTGameMgr`'s own memory) `+= delta`.
    /// 3. `ZooStatus::update(&self.zoo_status, delta)` (reimplemented, embedded sub-object).
    /// 4. If the accumulator now exceeds `0x3e9` (1001): reduce it `%= 0x3e9`, then recompute and push
    ///    animal/guest/zoo ratings plus the money/date UI text (`ZTUI::main::set{Animal,Guest,Zoo}Rating`/
    ///    `setMoneyText`/`setDateText`) - the animal/guest metrics live inside the embedded `ZooStatus`
    ///    sub-object (confirmed via the `.asm`: `EBX` = `&this->field_0x10` for these reads, not `this`
    ///    directly, resolving what looked like a `ZTGameMgr`- vs `ZooStatus`-relative offset mismatch
    ///    between the `.c` and `.asm`), each fed through `((metric + 100) * 100) / 200` unless the
    ///    corresponding count (`num_animals`/`guest_tile_count`) is `0`, in which case the
    ///    rating is `0` outright; zoo rating is read directly, no formula.
    /// 5. If `soundscape_ptr` is non-null, call the reimplemented [`ZTSoundscape::update`] directly
    ///    (same no-address-call-through rationale as [`Self::update`] below).
    /// 6. Advance `date` by `delta` simulation ticks via a real `SystemTimeToFileTime`/`FileTimeToSystemTime`
    ///    round-trip (`delta * 72000000` 100ns-intervals added to the `FILETIME` value - a
    ///    `SystemTimeToFileTime` failure is intentionally ignored here, matching the decompile's own
    ///    `GetLastError()`-then-continue error path, which has no further observable effect; a
    ///    `FileTimeToSystemTime` failure aborts the rest of the method, also matching), then sets
    ///    `day_changed_flag` if the round-trip changed `date.w_month` (see that field's own doc comment -
    ///    **not** `w_day_of_week`).
    pub fn update_sim(&mut self, delta: u32) {
        self.elapsed_sim_ticks = self.elapsed_sim_ticks.wrapping_add(delta);

        let dat_addr = get_module_base("zoo.exe") as u32 + DAT_006394B8_RVA;
        let mut tick_accumulator: i32 = get_from_memory(dat_addr);
        tick_accumulator = tick_accumulator.wrapping_add(delta as i32);
        save_to_memory(dat_addr, tick_accumulator);

        let zoostatus_ptr = (self as *mut Self as u32 + 0x10) as *const u32;
        unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.update(delta as i32);

        if tick_accumulator > 0x3e9 {
            tick_accumulator %= 0x3e9;
            save_to_memory(dat_addr, tick_accumulator);

            let zoostatus = unsafe { &*(zoostatus_ptr as *const ZooStatus) };

            let animal_rating = rating_from_metric(zoostatus.animal_rating_metric, self.num_animals as i32);
            unsafe { ZTUI_MAIN_SET_ANIMAL_RATING.original()(animal_rating) };

            let guest_rating = rating_from_metric(zoostatus.guest_rating_metric, self.guest_tile_count);
            unsafe { ZTUI_MAIN_SET_GUEST_RATING.original()(guest_rating) };

            unsafe { ZTUI_MAIN_SET_ZOO_RATING.original()(zoostatus.zoo_rating_current) };
            unsafe { ZTUI_MAIN_SET_MONEY_TEXT.original()() };
            unsafe { ZTUI_MAIN_SET_DATE_TEXT.original()() };
        }

        if self.soundscape_ptr != 0 {
            unsafe { mut_from_memory::<ZTSoundscape>(self.soundscape_ptr) }.update(delta as i32);
        }

        let previous_month = self.date.w_month;

        let mut file_time = FILETIME::default();
        // A SystemTimeToFileTime failure is intentionally ignored (matches the decompile's own
        // GetLastError()-then-continue path, which has no further observable effect).
        let _ = unsafe { SystemTimeToFileTime(&self.date.to_win32(), &mut file_time) };

        let file_time_ticks = ((file_time.dwHighDateTime as u64) << 32) | file_time.dwLowDateTime as u64;
        let new_file_time_ticks = file_time_ticks.wrapping_add((delta as u64) * 72000000);
        file_time.dwLowDateTime = new_file_time_ticks as u32;
        file_time.dwHighDateTime = (new_file_time_ticks >> 32) as u32;

        let mut new_sys_time = SYSTEMTIME::default();
        if unsafe { FileTimeToSystemTime(&file_time, &mut new_sys_time) }.is_err() {
            return;
        }
        self.date = Systemtime::from_win32(new_sys_time);

        if self.date.w_month != previous_month {
            self.day_changed_flag = true;
        }
    }

    /// Test-only accessors for the `ZTGAMEMGR_UPDATE_SIM` live test, letting it seed/read
    /// `day_changed_flag` without exposing it as public API.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_day_changed_flag(&mut self, value: bool) {
        self.day_changed_flag = value;
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn day_changed_flag(&self) -> bool {
        self.day_changed_flag
    }

    /// Test-only accessors for `ZTGAMEMGR_START_STOP_SMOKE`, letting it confirm `start`/`stop` toggled
    /// `started`/`soundscape_ptr` as expected without exposing either as public API.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn started(&self) -> bool {
        self.started
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn soundscape_ptr(&self) -> u32 {
        self.soundscape_ptr
    }

    /// Ports `CreateZTGameMgr` (`standalone::CREATE_ZTGAME_MGR`, `0x00527dc7`)'s field-initialization
    /// logic, read in full from `_CreateZTGameMgr.c`: zero `started`; write the base `BFGameMgr` vtable,
    /// run `ZooStatus::init` on the embedded sub-object, then overwrite with the real `ZTGameMgr` vtable;
    /// zero `soundscape_ptr`/`menu_music_handler_ptr`; resolve `menu_music_max_attenuation` from a real
    /// `BFIniFile::read("UI", "menuMusicMaxAttenuation", -1000)` call via two temporary
    /// vanilla-constructed strings. Assumes `this` points at a fresh, otherwise-uninitialized `0x11b0`-
    /// byte block - shared by [`Self::construct`] (vanilla-allocated, for the standalone-instance test
    /// harness) and the production `CREATE_ZTGAME_MGR` detour (`gamemgr_allocator_detours`, Rust-
    /// allocated via `Box` - see that module's own doc comment for why the allocator differs per caller).
    pub(crate) unsafe fn init_fields(this: *mut Self) {
        unsafe {
            (*this).started = false;

            (*this).vtable = BFGAMEMGR_VTABLE;
            let zoostatus_ptr = (this as u32 + 0x10) as *const u32;
            mut_from_memory::<ZooStatus>(zoostatus_ptr).init(std::ptr::null());
            (*this).vtable = ZTGAMEMGR_VTABLE;

            (*this).soundscape_ptr = 0;
            (*this).menu_music_handler_ptr = 0;

            let section = VanillaString::new("UI");
            let key = VanillaString::new("menuMusicMaxAttenuation");
            let result = READ.original()(section.as_ptr(), key.as_ptr(), (-1000i32) as u32);
            (*this).menu_music_max_attenuation = result as i32;
        }
    }

    /// Ports `CreateZTGameMgr`'s allocation (`operator_new(0x11b0)`, kept vanilla-allocated here) plus
    /// [`Self::init_fields`]. Used only by the standalone-instance test harness
    /// (`live_support::construct_standalone_via_rust`) - the production address is the
    /// `CREATE_ZTGAME_MGR` detour in `gamemgr_allocator_detours`, which allocates via `Box` instead.
    /// Returns null on allocation failure, matching the real body's own null propagation.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn construct() -> *mut Self {
        let block = unsafe { OPERATOR_NEW.original()(0x11b0) };
        if block.is_null() {
            return std::ptr::null_mut();
        }

        let this = block as *mut Self;
        unsafe { Self::init_fields(this) };
        this
    }

    /// Test-only accessor for `ZTGAMEMGR_DESTRUCT`, letting it populate `menu_music_handler_ptr` on a
    /// standalone instance directly - `start()`/`set_new_game_defaults()` never touch this field
    /// (`initMenuMusic`/`startMenuMusic*` stay un-detoured, real vanilla-only, per the module doc
    /// comment's "Methods deliberately not detoured" section), so there's no other way to exercise
    /// [`Self::destruct`]'s non-null handler branch from a synthetic instance.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_menu_music_handler_ptr(&mut self, value: u32) {
        self.menu_music_handler_ptr = value;
    }

    /// Ports `~ZTGameMgr` (`ztgamemgr::DESTRUCTOR_0`/`DESTRUCTOR_1`'s shared body). Still leaves the
    /// block itself vanilla-owned - freeing `self` is the caller's job, mirroring
    /// `live_support::destroy_standalone_mgr`'s existing `bDelete`-gated shape and `~ZTGameMgr_0` vs
    /// `_1`'s own split.
    ///
    /// Confirmed against `ZTGameMgr_~ZTGameMgr_0.asm`/`.c` directly, in order:
    /// 1. Entry vtable self-stamp to `ZTGameMgr`'s own vtable ([`ZTGAMEMGR_VTABLE`]) - genuine,
    ///    unconditional MSVC destructor codegen (the standard "restamp to this level's vtable at entry,
    ///    then to the base vtable before chaining to the base destructor" idiom), executed before either
    ///    pointer below is even tested. A no-op for this port (nothing here dispatches virtually through
    ///    `self`), reproduced for byte-for-byte fidelity with vanilla's memory shape.
    /// 2. If `soundscape_ptr != 0`: the reimplemented [`ZTSoundscape::destruct`], then free the block
    ///    (`standalone::OPERATOR_DELETE`), then zero the pointer.
    /// 3. If `menu_music_handler_ptr != 0`: the real body tail-jumps into `MenuMusicHandler`'s own
    ///    destructor address (`generated.rs`'s `MENU_MUSIC_HANDLER_0`, which is actually this tail-merge
    ///    block viewed from the other side - see the module's migration plan's re-verification section)
    ///    - whose body does the handler's own [`MenuMusicHandler::destruct`] sound teardown,
    ///      `operator_delete`s the handler block, *and* writes the final `BFMgr` vtable itself, returning
    ///      without this function ever reaching step 4. Reproduced here as three explicit steps instead of
    ///      a tail-call, since this port never calls that misattributed address directly.
    /// 4. `self.vtable = BFMGR_VTABLE` - the real body's own direct write when `menu_music_handler_ptr ==
    ///    0` (`ZTGameMgr_~ZTGameMgr_0.c`'s final line); when the handler branch *is* taken, the real
    ///    tail-merged callee performs the equivalent write instead (see step 3) - either way `self`'s
    ///    vtable ends up here, so this write stays unconditional rather than gated on the branch not
    ///    being taken.
    ///
    /// Called from both `gamemgr_allocator_detours`' `DESTRUCTOR_0`/`DESTRUCTOR_1` detours (Stage 5's
    /// production wiring) and the standalone-instance test harness.
    pub(crate) fn destruct(&mut self) {
        self.vtable = ZTGAMEMGR_VTABLE;

        if self.soundscape_ptr != 0 {
            unsafe { mut_from_memory::<ZTSoundscape>(self.soundscape_ptr) }.destruct();
            unsafe { OPERATOR_DELETE.original()(self.soundscape_ptr) };
            self.soundscape_ptr = 0;
        }

        if self.menu_music_handler_ptr != 0 {
            unsafe { mut_from_memory::<MenuMusicHandler>(self.menu_music_handler_ptr) }.destruct();
            unsafe { OPERATOR_DELETE.original()(self.menu_music_handler_ptr) };
            self.menu_music_handler_ptr = 0;
        }

        self.vtable = BFMGR_VTABLE;
    }

    /// Ports `ZTGameMgr::start` (`ZTGameMgr_start.c`/`.asm`, both agree cleanly - unlike [`Self::stop`],
    /// no decompiler corruption here). Per the real body, in order:
    /// 1. If already `started`: call [`Self::stop`] first (the real body's `stop(this)` recursive call -
    ///    ported directly rather than via `STOP.original()`, since once `STOP` is detoured this method's
    ///    own reimplementation is the real logic vanilla callers now run).
    /// 2. `operator_new(0x54)` a fresh `ZTSoundscape` block, constructed on success by the reimplemented
    ///    [`ZTSoundscape::construct`] called directly (same no-address-call-through rationale as
    ///    [`Self::update`] below) - `soundscape_ptr` stays `0` on allocation failure, matching the real
    ///    body's own null-propagation (`pcVar1 = 0` when `operator_new` fails).
    /// 3. Pull the four ambient-sound name/config strings from the live `GLOBAL_ZTScenarioMgr` singleton
    ///    (real vanilla `BFScenarioMgr` getter call-throughs - see [`GLOBAL_ZTSCENARIOMGR_RVA`]), pass all
    ///    four into the reimplemented [`ZTSoundscape::init`] on the new soundscape (direct call, as above;
    ///    no null guard on `soundscape_ptr`, matching the real body).
    /// 4. `started = true`.
    pub fn start(&mut self) {
        if self.started {
            self.stop();
        }

        let new_block = unsafe { OPERATOR_NEW.original()(0x54) };
        self.soundscape_ptr = if new_block.is_null() {
            0
        } else {
            unsafe { mut_from_memory::<ZTSoundscape>(new_block) }.construct();
            new_block as u32
        };

        let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
        let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
        let crowd_config = unsafe { GET_CROWD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };
        let world_config = unsafe { GET_WORLD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };

        unsafe {
            mut_from_memory::<ZTSoundscape>(self.soundscape_ptr).init(
                crowd_ambients,
                world_ambients,
                crowd_config,
                world_config,
            )
        };

        self.started = true;
    }

    /// Ports `ZTGameMgr::stop`. **The `.c` export for this method is corrupted** - it inlines the entire
    /// body of a different, tail-called function (`ZTUI::main::unpauseGame`) as if it were part of
    /// `stop()` itself. Ground truth is the `.asm`, cross-checked
    /// against `ZTGameMgr_stop.meta`'s own `calling_functions` list (exactly two callees:
    /// `~ZTSoundscape`/`FUN_00402629`) and `main_unpauseGame.c`/`.meta` (whose body matches the "extra"
    /// material in the corrupted `stop.c` export almost verbatim - same `BFUIMgr::getElement(0x430/0x42f)`/
    /// hide/show/`GLOBAL_DX8SndMgr` block, confirming it belongs to `unpauseGame`, not `stop`).
    ///
    /// Real body, in order:
    /// 1. If `soundscape_ptr != 0`: real vanilla destructor call-through (`ZTSoundscape::~ZTSoundscape`,
    ///    `generated.rs`'s `ztsoundscape::DESTRUCTOR` entry, imported here as [`ZTSOUNDSCAPE_DESTRUCTOR`]),
    ///    then free the block (`standalone::OPERATOR_DELETE`, typed `u32` in `generated.rs`), then zero the
    ///    pointer.
    /// 2. `started = false`.
    /// 3. Read the live `GLOBAL_ZTApp` singleton's `+0x440` byte field (`appInitSuccess` - see
    ///    [`GLOBAL_ZTAPP_RVA`]); if non-zero, tail-call real vanilla `ZTUI::main::unpauseGame`
    ///    (`ztui_main::UNPAUSE_GAME`).
    ///
    /// **Deliberately not reproduced**: the real body's "if `GLOBAL_ZTApp` is null, lazily assign it a
    /// bogus `ZTApp::handleMessages`-function-pointer sentinel before re-reading it" defensive branch -
    /// `GLOBAL_ZTApp` is the top-level app singleton, already constructed long before any live
    /// `ZTGameMgr::stop()` call can happen, so this branch is dead in every real-game scenario; if it were
    /// somehow null anyway, this port just treats that as "app not ready" and skips the `unpauseGame` call,
    /// rather than writing a nonsensical code-address-as-data-pointer into live global state to match a
    /// real but never-taken vanilla path.
    pub fn stop(&mut self) {
        if self.soundscape_ptr != 0 {
            unsafe { ZTSOUNDSCAPE_DESTRUCTOR.original()(self.soundscape_ptr as *const c_void) };
            unsafe { OPERATOR_DELETE.original()(self.soundscape_ptr) };
            self.soundscape_ptr = 0;
        }

        self.started = false;

        let ztapp_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTAPP_RVA);
        if ztapp_ptr != 0 {
            let app_init_success: u8 = get_from_memory(ztapp_ptr + 0x440);
            if app_init_success != 0 {
                unsafe { ZTUI_MAIN_UNPAUSE_GAME.original()() };
            }
        }
    }
}

/// a command that prints the SYSTEMTIME struct in memory in a human-readable format
/// usage: `get_date`
pub fn command_get_date_str(_args: Vec<&str>) -> Result<String, CommandError> {
    let ztgamemgr = globals().ztgamemgr();
    let date = ztgamemgr.date;
    info!("Date: {:#?}", date);

    Ok(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.w_year, date.w_month, date.w_day, date.w_hour, date.w_minute, date.w_second
    ))
}

/// a command that adds cash to the player's account
/// usage: `add_cash <amount>`
pub fn command_add_cash(args: Vec<&str>) -> Result<String, CommandError> {
    let ptr = globals().ztgamemgr_ptr();
    let amount = args[0].parse::<f32>()?;
    unsafe { (*ptr).add_cash(amount) };
    Ok(format!("Added ${}", args[0]))
}

/// a command that enables or disables dev mode
/// usage: `enable_dev_mode <true/false>`
pub fn command_enable_dev_mode(args: Vec<&str>) -> Result<String, CommandError> {
    let enable = args[0].parse()?;
    ZTGameMgr::enable_dev_mode(enable);
    Ok(format!("Dev mode enabled: {}", enable))
}

/// a command that prints various stats about the zoo
/// usage: `zoostats`
pub fn command_zoostats(_args: Vec<&str>) -> Result<String, CommandError> {
    let ztgamemgr = globals().ztgamemgr();
    Ok(format!("\nBudget: {}\nAnimals: {}\nSpecies: {}\nTired Guests: {}\nHungry Guests: {}\nThirsty Guests: {}\nGuests Need Restroom: {}\nGuest Tiles: {}\nZoo Admission Cost: ${}", ztgamemgr.cash, ztgamemgr.num_animals, ztgamemgr.num_species, ztgamemgr.num_tired_guests, ztgamemgr.num_hungry_guests, ztgamemgr.num_thirst_guests, ztgamemgr.num_guests_restroom_need, ztgamemgr.guest_tile_count, ztgamemgr.zoo_admission_cost))
}

