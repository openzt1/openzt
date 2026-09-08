//! `ZTGameMgr` reimplementation. Follows a third named pattern variant, distinct from
//! `ztadvterrainmgr.rs`'s "thin-shell whole-class" call-through and `ztmegatilemgr.rs`'s "100%
//! vanilla-owned" style: **per-method delegation to an embedded sub-object**.
//! `ZTGameMgr`'s own vtable/non-virtual logic is (or, stage by stage, will be) fully ported to Rust,
//! and each method that needs the `ZooStatus` finance/rating tracker embedded inline at `this+0x10`
//! (confirmed via `_CreateZTGameMgr.c`'s `OOAnalyzer::ZooStatus::init((ZooStatus *)(puVar2 + 4),...)`)
//! now calls directly into the reimplemented `impl ZooStatus` methods in `zoostatus.rs`
//! (`openzt/plans/zoostatus-implementation-plan.md`'s Stage 8 rewiring - `ZooStatus`'s own ~30-method
//! surface is that plan's concern, not this file's). Because `ZTGameMgr` stays vanilla-layout-compatible
//! and `ZooStatus` lives inline in the *same* memory block rather than a separate allocation, reading/
//! writing that live memory from within a Rust `ZTGameMgr` method body is always safe - nothing is freed
//! or reallocated in place, so none of `CLAUDE.md`'s cross-allocator hazards apply here.
//!
//! No dynamic containers live in `ZTGameMgr`'s or the real `ZooStatus`'s own memory - only scalars and
//! fixed-size arrays (see the implementation plan's "No dynamic containers" section), so the
//! vanilla-layout-compatible struct below never needs to model a `Vec`/map/tree.

use std::ffi::c_void;

use openzt_detour::generated::{
    bfscenariomgr::{GET_CROWD_AMBIENTS_NAME, GET_CROWD_CONFIG_NAME, GET_WORLD_AMBIENTS_NAME, GET_WORLD_CONFIG_NAME},
    standalone::{DEALLOCATE, OPERATOR_DELETE, OPERATOR_NEW, WRITE_BYTES_TO_FILE},
    ztsoundscape::ZTSOUNDSCAPE as ZTSOUNDSCAPE_DESTRUCTOR,
    ztui_main::{
        SET_ANIMAL_RATING as ZTUI_MAIN_SET_ANIMAL_RATING, SET_DATE_TEXT as ZTUI_MAIN_SET_DATE_TEXT, SET_GUEST_RATING as ZTUI_MAIN_SET_GUEST_RATING,
        SET_MONEY_TEXT as ZTUI_MAIN_SET_MONEY_TEXT, SET_ZOO_RATING as ZTUI_MAIN_SET_ZOO_RATING, UNPAUSE_GAME as ZTUI_MAIN_UNPAUSE_GAME,
    },
    bfaimgr::LOAD_DATA as BFAIMGR_LOAD_DATA,
    ztgamemgr::{
        ADD_CASH, ANIMAL_TIME_AGO, GET_DATE, HOURS_AGO, IS_GAME_DATE, IS_REAL_WORLD_DATE, LOAD, OVERRIDE_NEW_GAME_DEFAULTS, PEOPLE_TIME_AGO,
        SAVE, SET_NEW_GAME_DEFAULTS, START, STOP, SUBTRACT_CASH, TIME_AGO, UPDATE, UPDATE_SIM,
    },
};
// `ZooStatus`'s own 8 call-through sites below (spend_research/spend_marketing/set_new_game_defaults'
// init+ratingChecks/override_new_game_defaults/save/load/update_sim) now call the reimplemented
// `impl ZooStatus` methods directly (Stage 8 of `zoostatus-implementation-plan.md`) rather than going
// through `zoostatus::*`'s real-vanilla `.original()` - see `zoostatus.rs`'s own `zoostatus_detours`
// module for the address-level detours this rewiring pairs with.
#[cfg(feature = "reimplementation-tests")]
use openzt_detour::generated::{standalone::CREATE_ZTGAME_MGR, ztgamemgr::ZTGAME_MGR_1};
use openzt_detour_macro::detour_mod;
use tracing::{error, info};
use windows::Win32::{
    Foundation::{FILETIME, SYSTEMTIME},
    System::{SystemInformation::GetSystemTime, Time::{FileTimeToSystemTime, SystemTimeToFileTime}},
};

use crate::{
    command_console::CommandError,
    globals::{get_module_base, globals},
    lua_fn,
    util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory},
    ztgamemgr_menumusichandler::MenuMusicHandler,
    ztsoundscape::ZTSoundscape,
    zoostatus::{self, ZooStatus},
};

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
    /// `FILETIME` round-trip (`ZTGameMgr_updateSim.c`/`.asm`: `this->field_0x28 = 1` when
    /// `*(short*)&this->field_0x1196` - i.e. `date.w_month`, confirmed against the `.asm`'s
    /// `word ptr [ESI+0x1196]` compare, **not** `w_day_of_week` as an earlier pass of this plan
    /// mis-labelled it - `Systemtime`'s own field order puts `w_day_of_week` at `+4`, not `+2` -
    /// differs before/after). Never cleared by `updateSim` itself; whatever consumes it is out of
    /// scope for this reimplementation.
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
    /// `ZooStatus::guest_tile_count` doc comment for the full naming history (this field was originally
    /// `num_guests`, then briefly `escaped_animal_tile_count` on a mistaken "escape counter" reading
    /// that Stage 5's full `calculateSums.asm` read overturned back in the guest direction - same
    /// underlying bytes throughout, `ZooStatus`-relative `+0x44`, `ZTGameMgr`-relative `+0x54` after the
    /// `+0x10` embedding offset). Renaming this is a pure documentation fix - the bytes read here are
    /// unchanged.
    guest_tile_count: u16, // 0x54
    // This pad also covers the embedded `ZooStatus`'s "monthly history" fixed-size float-array region
    // (see `ZooStatus`'s own doc comment below) - offsets guessed at, unconfirmed, and pending a real
    // shape resolution:
    // admissions_income_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x254),
    // concessions_benefit_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x29c),
    // recycling_benefit_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x340),
    // // net_income maybe?: get_from_memory::<i32>(zt_game_mgr_prt + 0x404),
    // income_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x404),
    // income_expense_totals_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x44c),
    // zoo_rating_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x464),
    // unknown_array: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x4c4),
    // construction_cost_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x824),
    pad9: [u8; 0x1160 - 0x56], // 0x54
    zoo_admission_cost: f32,   // 0x1160
    pad10: [u8; 0x1190 - 0x1164], // 0x1164 - includes `removedZooDoo`'s refund-per-item base amount at
                                   // `+0x117c` (`ZTGameMgr_removedZooDoo.c`/`.asm`), unnamed since that
                                   // method is not currently ported - see the module's Stage-5 doc comment
    /// `ZTSoundscape*`, read/written by `start`/`stop`/`updateSim`/the destructor. Explicitly zeroed
    /// by `CreateZTGameMgr` (`puVar2[0x464] = 0;`, dword index `0x464` = byte offset `0x1190`).
    soundscape_ptr: u32, // 0x1190
    date: Systemtime,    // 0x1194
    /// `ZTGameMgr::MenuMusicHandler*`, read by `update`/`startMenuMusic`/`startMenuMusicFade`/the
    /// destructor. Explicitly zeroed by `CreateZTGameMgr` (`puVar2[0x469] = 0;`, dword index `0x469` =
    /// byte offset `0x11A4`).
    menu_music_handler_ptr: u32, // 0x11A4
    pad11: [u8; 0x11b0 - 0x11A8], // 0x11A8 - a menu-music-ini read result read/written by the
                                   // macOS-only `menuMusicAttenToScrollbarVal`/`scrollbarValToMenuMusicAtten`
                                   // (see this module's Stage-5 doc comment) - not untouched, just out of
                                   // this reimplementation's current scope
}

const _: () = assert!(std::mem::size_of::<ZTGameMgr>() == 0x11b0);

/// Vanilla's embedded `SYSTEMTIME` (same field order/size as `windows::Win32::Foundation::SYSTEMTIME`,
/// confirmed against that crate's own definition) - kept as a distinct, private type rather than the
/// real `windows` struct directly so `ZTGameMgr`'s own field remains `#[repr(C)]`-stable independent of
/// that crate's own attributes; `updateSim` (Stage 3) converts to/from the real `SYSTEMTIME`/`FILETIME`
/// via [`Systemtime::to_win32`]/[`Systemtime::from_win32`] for its `SystemTimeToFileTime`/
/// `FileTimeToSystemTime` round-trip.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
struct Systemtime {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

impl Systemtime {
    fn to_win32(self) -> SYSTEMTIME {
        SYSTEMTIME {
            wYear: self.w_year,
            wMonth: self.w_month,
            wDayOfWeek: self.w_day_of_week,
            wDay: self.w_day,
            wHour: self.w_hour,
            wMinute: self.w_minute,
            wSecond: self.w_second,
            wMilliseconds: self.w_milliseconds,
        }
    }

    fn from_win32(value: SYSTEMTIME) -> Self {
        Self {
            w_year: value.wYear,
            w_month: value.wMonth,
            w_day_of_week: value.wDayOfWeek,
            w_day: value.wDay,
            w_hour: value.wHour,
            w_minute: value.wMinute,
            w_second: value.wSecond,
            w_milliseconds: value.wMilliseconds,
        }
    }
}

/// Packs a real `FILETIME`'s two dwords into a single raw 64-bit tick count, matching the real
/// `SUB`/`SBB`-pair arithmetic `timeAgo`/`hoursAgo` do over the two halves directly.
fn filetime_to_ticks(file_time: FILETIME) -> u64 {
    ((file_time.dwHighDateTime as u64) << 32) | file_time.dwLowDateTime as u64
}

/// Inverse of [`filetime_to_ticks`].
fn ticks_to_filetime(ticks: u64) -> FILETIME {
    FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    }
}

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
/// comment in `reimplementation_tests/mod.rs`). Covered by a `#[cfg(test)]` unit test below instead.
fn rating_from_metric(metric: i32, population: u16) -> i32 {
    if population == 0 {
        0
    } else {
        zoostatus::scaled_rating_metric(metric, 100)
    }
}

impl ZTGameMgr {
    /// enables or disables dev mode
    fn enable_dev_mode(enable: bool) {
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

    /// Test-only accessors for Stage 2's `ZTGAMEMGR_SAVE_LOAD` live test, letting it seed/read the
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

    /// Ports `ZTGameMgr::subtractCash` (`ZTGameMgr_subtractCash.c`, read in full): subtracts `amount`
    /// from the budget, then refreshes the on-screen money display (`ZTUI::main::setMoneyText`). Used
    /// by `ztresearch::ZTResearchBranch::update`'s native reimplementation of the branch funding cost,
    /// among other callers.
    ///
    /// Now that Stage 4 wires a real `#[detour(SUBTRACT_CASH)]`, this ports the actual body rather than
    /// calling through to `.original()` (as it did through Stages 0-3) - vanilla-side callers of the
    /// real function address (not just Rust API callers) now also route through this same logic once
    /// detoured. The real signature has a trailing, unread `bool` parameter (`ZTGameMgr::subtractCash(float,
    /// bool)`, per the `.asm`'s `RET 8`) - not part of this method's own logic, so not part of this
    /// method's own signature either; the detour wrapper below supplies/discards it.
    pub fn subtract_cash(&mut self, amount: f32) {
        self.cash -= amount;
        unsafe { ZTUI_MAIN_SET_MONEY_TEXT.original()() };
    }

    /// Ports `ZTGameMgr::addCash` (`ZTGameMgr_addCash.c`, read in full): adds `amount` to the budget,
    /// then refreshes the on-screen money display (`ZTUI::main::setMoneyText`).
    pub fn add_cash(&mut self, amount: f32) {
        self.cash += amount;
        unsafe { ZTUI_MAIN_SET_MONEY_TEXT.original()() };
    }

    /// Ports `ZTGameMgr::getDate` (`ZTGameMgr_getDate.c`/`.asm`, read in full): converts `date` to a
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

    /// Ports `ZTGameMgr::isGameDate` (`ZTGameMgr_isGameDate.c`/`.asm`, read in full - the `.asm` is the
    /// clean read; the `.c`'s messy `CONCAT`/register-reuse noise around the return value is decompiler
    /// artifact, not real extra logic). Round-trips `get_date()` back through `FileTimeToSystemTime` and
    /// compares `day`/`month` against the result, `0xffffffff` acting as a per-field wildcard. Returns
    /// `false` outright if the round-trip fails.
    ///
    /// **Correction from the implementation plan**, which described this as `isGameDate(month, day)`:
    /// the real parameter order, confirmed independently from both the `.c`'s offset math
    /// (`local_10._6_4_` = `wDay` compared against `param_1`; `local_10._2_4_` = `wMonth` compared
    /// against `param_2`) and the `.asm`'s stack-offset reads, is `(day, month)`.
    pub fn is_game_date(&self, day: u32, month: u32) -> bool {
        let file_time = ticks_to_filetime(self.get_date());
        let mut sys_time = SYSTEMTIME::default();
        if unsafe { FileTimeToSystemTime(&file_time, &mut sys_time) }.is_err() {
            return false;
        }
        (day == 0xffffffff || sys_time.wDay as u32 == day) && (month == 0xffffffff || sys_time.wMonth as u32 == month)
    }

    /// Ports `ZTGameMgr::isRealWorldDate` (`ZTGameMgr_isRealWorldDate.c`, read in full). Unlike
    /// `is_game_date`, this has no `this` dependency at all (confirmed by its `stdcall`/no-`this`
    /// `IS_REAL_WORLD_DATE` entry) - it calls `GetSystemTime` directly and has no `0xffffffff` wildcard
    /// handling. **Same parameter-order correction as `is_game_date`**: real order is `(day, month)`,
    /// not the plan's `(month, day)`.
    pub fn is_real_world_date(day: u32, month: u32) -> bool {
        let sys_time = unsafe { GetSystemTime() };
        sys_time.wDay as u32 == day && sys_time.wMonth as u32 == month
    }

    /// Ports `ZTGameMgr::timeAgo` (`ZTGameMgr_timeAgo.c`/`.asm`, read in full): `get_date() - reference`
    /// as a 64-bit subtraction (the real body's `SUB`/`SBB` pair over the two `FILETIME` dwords is
    /// exactly a wrapping `u64` subtraction). See the `TIME_AGO` `FunctionDef`'s own doc comment in
    /// `generated.rs` for why the auto-generated entry needed hand-correcting before this could be
    /// wired to a real detour at all.
    pub fn time_ago(&self, reference: u64) -> u64 {
        self.get_date().wrapping_sub(reference)
    }

    /// Ports `ZTGameMgr::hoursAgo` (`ZTGameMgr_hoursAgo.c`/`.asm`, read in full): same `get_date() -
    /// reference` 64-bit subtraction as `time_ago`, divided by `36_000_000_000` (100ns intervals per
    /// hour) via the real body's own unsigned 64-bit division (`_aulldiv`). See the `HOURS_AGO`
    /// `FunctionDef`'s own doc comment in `generated.rs` for the separate return-type correction this
    /// needed (the auto-generated `*const u64` would have silently dropped the high dword of a genuine
    /// EDX:EAX register-pair return).
    pub fn hours_ago(&self, reference: u64) -> u64 {
        self.get_date().wrapping_sub(reference) / 36_000_000_000
    }

    /// Ports `ZTGameMgr::animalTimeAgo` (`ZTGameMgr_animalTimeAgo.c`/`.asm`, read in full - macOS-only
    /// method, no Windows decompile existed until this pass; see the module's Stage-5 doc comment for
    /// the other five macOS-only methods this same investigation covered). Buckets [`Self::hours_ago`]'s
    /// result into one of three values: `0` (< 1440 hours / 60 days), `1` (1440-8640h), or `2` (> 8640h /
    /// 360 days) - only **two** thresholds. **Not** the three-threshold/four-bucket shape
    /// [`Self::people_time_ago`] uses, despite both being named `*TimeAgo` and sharing the same
    /// `hoursAgo` base - confirmed independently per-function against each one's own `.asm` (`animalTimeAgo`:
    /// `CMP EAX,0x5a0` / `CMP EAX,0x21c0`, two compares; `peopleTimeAgo`: three). The real return is a
    /// register pair (`ulonglong`) whose low dword is the bucket and whose high dword is `hoursAgo`'s own
    /// leftover EDX half, not part of the bucket value - only the low dword is meaningful here, matching
    /// this codebase's established `TIME_AGO`/`HOURS_AGO` EDX:EAX correction precedent.
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

    /// Ports `ZTGameMgr::peopleTimeAgo` (`ZTGameMgr_peopleTimeAgo.c`/`.asm`, read in full - macOS-only
    /// method, see [`Self::animal_time_ago`]'s doc comment). Same `hoursAgo` base, but **three**
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
    /// Per the decompile/`.asm` (`ZTGameMgr_setNewGameDefaults.c`/`.asm`, read in full - **the plan this
    /// was scoped from mis-described this method's first step as "zero `elapsed_sim_ticks`"; the real
    /// first write is `[this+0xc] = 0`, i.e. `cash`, confirmed independently by both the Windows
    /// `LEA ECX,[ESI+0xc]` / `MOV [ECX],EAX` `.asm` and the macOS decompile's `ZTEcon__init((float
    /// *)(param_1 + 0xc))` - `elapsed_sim_ticks` is only ever zeroed once, at the very end**), the real
    /// order is:
    /// 1. `cash = 0.0`
    /// 2. `ZooStatus::init(&self.zoo_status, config)` (reimplemented, embedded sub-object - Stage 8)
    /// 3. set `date` to the hardcoded new-game default (2001-01-01, a Monday, 00:00:00.000)
    /// 4. if `is_new_game`: call through `GLOBAL_ZTAIMgr`'s real vtable slot `+0x4` (`0x0058f269`,
    ///    inherited unchanged from `BFAIMgr` - see `private/docs/vtables/BFAIMgr.md`), now identified by a
    ///    Ghidra regen as `BFAIMgr::loadData` (`thiscall fn(this, bool) -> u32`), with `false`, matching
    ///    the real thiscall/1-arg shape confirmed by both this function's and `_setCursorQuality`'s own
    ///    `.asm`
    /// 5. `ZooStatus::ratingChecks(&self.zoo_status)` (reimplemented, embedded sub-object - Stage 8)
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

    /// Ports `ZTGameMgr::overrideNewGameDefaults` (`ZTGameMgr_overrideNewGameDefaults.c`, read in full -
    /// macOS-only method, see the module's Stage-5 doc comment). One-line call-through to the
    /// reimplemented `ZooStatus::override` on the embedded sub-object at `self+0x10` (Stage 8), exactly
    /// the same shape as [`Self::spend_research`]/[`Self::spend_marketing`].
    pub fn override_new_game_defaults(&mut self, config: *const u32) {
        let zoostatus_ptr = (self as *mut Self as u32 + 0x10) as *const u32;
        unsafe { mut_from_memory::<ZooStatus>(zoostatus_ptr) }.override_config(config as *const c_void)
    }

    /// Ports `ZTGameMgr::save` (vtable `+0x8`), with `BFGameMgr::save`'s own base-class body (just
    /// writing `elapsed_sim_ticks`) inlined directly - see the module doc comment for why there's no
    /// separate `BFGameMgr`-vs-`ZTGameMgr` split kept in this port.
    ///
    /// Per the decompile (`ZTGameMgr_save.c`, read in full), write order is: a fixed `0` marker dword
    /// (`local_8` - pure format padding, no real state); `ZooStatus::save` on the embedded sub-object at
    /// `self+0x10` (reimplemented - Stage 8); `date` raw (16 bytes); `cash` (captured into a local
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
    /// Per the decompile (`ZTGameMgr_load.c`, read in full), read order mirrors `save`'s write order: a
    /// `0` marker dword (read and discarded - only its success/failure matters) -> `ZooStatus::load`
    /// (reimplemented, embedded sub-object - Stage 8) -> `date` raw (16 bytes, written directly into
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
        if (zoostatus_result & 0xff) == 0 {
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

    /// Ports `ZTGameMgr::update` (vtable `+0x10`). Per the decompile (`ZTGameMgr_update.c`, read in
    /// full) this is a pure call-through to the embedded `MenuMusicHandler` when present - no logic of
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
    /// `.asm`, read in full), in order:
    /// 1. `elapsed_sim_ticks += delta` (`BFGameMgr::updateSim`'s own base body, inlined - see the module
    ///    doc comment for why there's no separate `BFGameMgr`-vs-`ZTGameMgr` split kept in this port).
    /// 2. The raw global tick accumulator `DAT_006394b8` (`this->mbr_0x8`'s sibling, not part of
    ///    `ZTGameMgr`'s own memory) `+= delta`.
    /// 3. `ZooStatus::update(&self.zoo_status, delta)` (reimplemented, embedded sub-object - Stage 8).
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
    ///    `day_changed_flag` if the round-trip changed `date.w_month` (see that field's own doc comment
    ///    for why - **not** `w_day_of_week`, correcting an earlier pass of the implementation plan).
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

            let animal_rating = rating_from_metric(zoostatus.animal_rating_metric, self.num_animals);
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

    /// Test-only accessors for Stage 3's `ZTGAMEMGR_UPDATE_SIM` live test, letting it seed/read
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
                crowd_ambients as *const u8,
                world_ambients as *const u8,
                crowd_config,
                world_config,
            )
        };

        self.started = true;
    }

    /// Ports `ZTGameMgr::stop`. **The `.c` export for this method is corrupted** - it inlines the entire
    /// body of a different, tail-called function (`ZTUI::main::unpauseGame`) as if it were part of
    /// `stop()` itself, the same species of decompiler-boundary bug already found and fixed in
    /// `ztshowscriptstate::CONSTRUCTOR` (see the roadmap/review history) and originally suspected (then
    /// disproven the other direction) in `removedZooDoo`. Ground truth is the `.asm`, cross-checked
    /// against `ZTGameMgr_stop.meta`'s own `calling_functions` list (exactly two callees:
    /// `~ZTSoundscape`/`FUN_00402629`) and `main_unpauseGame.c`/`.meta` (whose body matches the "extra"
    /// material in the corrupted `stop.c` export almost verbatim - same `BFUIMgr::getElement(0x430/0x42f)`/
    /// hide/show/`GLOBAL_DX8SndMgr` block, confirming it belongs to `unpauseGame`, not `stop`).
    ///
    /// Real body, in order:
    /// 1. If `soundscape_ptr != 0`: real vanilla destructor call-through (`ZTSoundscape::~ZTSoundscape`,
    ///    `generated.rs`'s misleadingly-named `ztsoundscape::ZTSOUNDSCAPE` entry - confirmed to actually be
    ///    the destructor, not the constructor, via `ZTSoundscape_~ZTSoundscape.meta`'s matching address;
    ///    left un-renamed since it's an existing, non-hand-added `generated.rs` entry - see `CLAUDE.md`),
    ///    then free the block (`standalone::OPERATOR_DELETE`, typed `u32` in `generated.rs`), then zero the
    ///    pointer.
    /// 2. `started = false`.
    /// 3. Read the live `GLOBAL_ZTApp` singleton's `+0x440` byte field (`appInitSuccess` - see
    ///    [`GLOBAL_ZTAPP_RVA`]); if non-zero, tail-call real vanilla `ZTUI::main::unpauseGame` (already a
    ///    plain, cleanly-addressed `generated.rs` entry - `ztui_main::UNPAUSE_GAME`, no hand-add needed).
    ///
    /// **Deliberately not reproduced**: the real body's "if `GLOBAL_ZTApp` is null, lazily assign it a
    /// bogus `ZTApp::handleMessages`-function-pointer sentinel before re-reading it" defensive branch -
    /// `GLOBAL_ZTApp` is the top-level app singleton, already constructed long before any live
    /// `ZTGameMgr::stop()` call can happen, so this branch is dead in every real-game scenario; if it were
    /// somehow null anyway, this port just treats that as "app not ready" and skips the `unpauseGame` call,
    /// rather than writing a nonsensical code-address-as-data-pointer into live global state to match a
    /// real but never-taken vanilla path (`CLAUDE.md`: don't handle scenarios that can't happen).
    pub fn stop(&mut self) {
        if self.soundscape_ptr != 0 {
            unsafe { ZTSOUNDSCAPE_DESTRUCTOR.original()(self.soundscape_ptr as *const c_void) };
            unsafe { OPERATOR_DELETE.original()(self.soundscape_ptr as u32) };
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
    let date = ztgamemgr.date.clone();
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

/// Stage 5 of `openzt/plans/ztgamemgr-implementation-plan.md`: the destructor and the
/// `gotoStart`/`removedZooDoo`/`startMenuMusic*` call-through tier are deliberately **not** detoured. No
/// porting, no new live tests - only a documented decision per item, all confirmed directly against the
/// decompiles (`private/resources/decompiles/ZTGameMgr_*`) this session.
///
/// **`removedZooDoo`'s decompile is no longer the blocker, but the method is still un-ported.** A
/// follow-up pass to the review that produced this section found the decompile export this bullet
/// originally cited was itself broken (analysis had started at an internal `JMP` target, `0x004a2ee1`,
/// mistaking it for the function boundary, producing the mangled 11-parameter/`unaff_*`-artifact signature
/// this bullet used to describe). The corrected export, real entry point `0x004a2c98`, is clean and its
/// logic is comprehensible (tile-distance search over `ZTWorldMgr::getBuildingList("compost")`,
/// `ZTBuilding::receiveIncome`, `ZooStatus::refundConstruction`/`addCash`) - `generated.rs`'s
/// `ztgamemgr::REMOVED_ZOO_DOO` entry reflects this corrected address/signature, and `ztworldmgr::
/// GET_BUILDING_LIST`'s signature was corrected to a genuine `thiscall` while investigating this
/// (both confirmed via careful `.asm` tracing, independent of the point below). The regeneration first
/// reverted `GET_BUILDING_LIST` to its wrong auto-derived `stdcall` shape, but after the correction was
/// re-surfaced to the generator pass the entry now carries the right shape natively (verified against the
/// function's own `.c`/`.asm` and the caller's `MOV %ECX, %EBX` immediately before the `CALL`). A full port was attempted
/// and got as far as passing its own live smoke test - but only by constructing the "compost" tag string
/// via a real vanilla `std::string` constructor/destructor call, after a simpler, self-owned
/// (non-vanilla-allocated) string reproducibly crashed `getBuildingList` live for a reason never fully
/// root-caused. That result - a working path that depends on unexplained vanilla behavior, sitting on top
/// of an already-nontrivial chain of hand-derived ABI facts (parameter order, list-node layout, a
/// small-object-free address) - was judged too much unverified surface for a single pass, so
/// the port was backed out. The `.asm`-traced corrections remain confirmed ground truth for whoever
/// revisits this; the Rust port itself is not wired up. Likewise, `animalTimeAgo`/`peopleTimeAgo`/
/// `overrideNewGameDefaults` (macOS-only methods that never entered this plan's original scope at all -
/// see [`ZTGameMgr::animal_time_ago`]/[`ZTGameMgr::people_time_ago`]/
/// [`ZTGameMgr::override_new_game_defaults`]) picked up real Windows addresses/decompiles and are now
/// ported too, not part of this "left un-detoured" tier. `initMenuMusic` (also macOS-only-scoped
/// originally) picked up a confirmed Windows address too (`ztgamemgr::INIT_MENU_MUSIC`, `0x00521e18` -
/// the same address the `startMenuMusic` decompile's `FUN_00521e18` lead pointed at) but **stays
/// out of scope**, unlike its three siblings: it pulls in `BFIniFile` construction (still an untouched
/// dependency) plus a `MenuMusicHandler` - which has since been reimplemented itself (see
/// `ztgamemgr_menumusichandler.rs`), so that half of the original blocker is gone; `BFIniFile` is what
/// keeps it un-detoured.
///
/// **`start()`/`stop()` are now ported**, closing out a review-flagged candidate that turned out to need a
/// second pass: `start()`'s `.c`/`.asm` agree cleanly, but `stop()`'s `.c` export is corrupted the same way
/// the original (pre-correction) `removedZooDoo` export was - it inlines the entire body of a *different*,
/// tail-called function (`ZTUI::main::unpauseGame`) as if it were part of `stop()` itself. The real `.asm`
/// (cross-checked against `stop.meta`'s own 2-callee list and `main_unpauseGame.c`/`.meta`, whose body
/// matches the corrupted export's "extra" material almost verbatim) is simple: soundscape teardown, clear
/// `started`, then a conditional tail-call into the real, separately-addressed `unpauseGame()` - see
/// [`ZTGameMgr::start`]/[`ZTGameMgr::stop`]'s own doc comments for the full account. Closing this out also
/// needed four raw *global data* addresses (`GLOBAL_ZTScenarioMgr`, `GLOBAL_ZTApp`, plus `GLOBAL_BFUIMgr`/
/// `GLOBAL_DX8SndMgr`, the latter two turning out to be internal to `unpauseGame()` rather than something
/// this port touches directly) that don't exist anywhere in `generated.rs` (which only carries function
/// addresses) or `globals.rs` - unlike every other address in this file, these came from the user directly
/// rather than the local decompile/vtable corpus, the same class of blocker `menuMusicAttenToScrollbarVal`/
/// `scrollbarValToMenuMusicAtten` below remain stuck on.
///
/// - **Destructor** (`ZTGAME_MGR_0`/`ZTGAME_MGR_1`): `~ZTGameMgr_0.c` tears down the embedded
///   `ZTSoundscape` (if `soundscape_ptr != 0`) and `MenuMusicHandler` (if `menu_music_handler_ptr != 0`,
///   both out of scope, see the module doc comment) then swaps the vtable pointer to `BFMgr_vftable` and
///   returns, continuing into `BFMgr`'s own base-class teardown; `~ZTGameMgr_1.c` is just the deleting
///   variant (`bDelete` byte gating a `FUN_00402629` free after the same body runs). Since this
///   reimplementation stays vanilla-layout-compatible (style 1: same memory, no independent Rust-owned
///   heap state), there is nothing this detour would do differently from vanilla's own body - mirrors
///   `ztmegatilemgr.rs`'s and `ztadvterrainmgr.rs`'s own stated reasoning for skipping their destructors.
/// - **`removedZooDoo(...)`**: see the paragraph above - decompile/`generated.rs` corrected and confirmed
///   portable in principle, but the actual port was attempted and backed out this session after live
///   testing surfaced a crash risk that wasn't fully understood, not because the logic itself is
///   unclear. Left un-detoured.
/// - **`gotoStart(...)`**: `ZTGameMgr_gotoStart.c` is confirmed genuinely decompiler-mangled, not just
///   verbose - `unaff_EBX`/`unaff_ESI`/`unaff_EDI` register-allocation artifacts stand in for real
///   parameters/locals, and the recovered signature (14 params, mostly untyped `undefined`) doesn't match
///   any real call site. `generated.rs`'s own `GOTO_START` entry (`u8×12, u32×2`) reflects the same
///   automatic-signature-recovery confusion. Not faithfully portable from this decompile - left
///   un-detoured.
/// - **`startMenuMusic()`/`startMenuMusicFade()`**: confirmed by reading all three
///   `startMenuMusicFade_{0,1,2}` decompiles and `.meta` files directly (resolving the plan's open
///   question about whether they're 3 redundant call sites or 3 real variants - they're neither, cleanly).
///   `_0` (thiscall, `0x004c9d67`) and `_2` (fastcall, `0x004cc59d`) have identical trivial bodies -
///   forward to `MenuMusicHandler::startFade` when `menu_music_handler_ptr != 0` - and both `.meta`s list
///   `startFade` as a called function: these are two distinct compiled/calling-convention instances of
///   the *same* logical `ZTGameMgr::startMenuMusicFade`, not duplicates worth deduping here. `_1`
///   (fastcall, `0x004ca478`) is a **different function** entirely - its body directly implements a
///   vtable dispatch (`(**(code**)(*vtable+0x50))()`) plus a fade-state flag/counter reset, calls nothing
///   named (empty `calling_functions` in its `.meta`, unlike `_0`/`_2`), and is almost certainly
///   `MenuMusicHandler::startFade`'s own real body, mislabeled with the `ZTGameMgr::` name by the
///   decompile corpus's automated naming pass. All three stay out of scope regardless (either
///   `ZTGameMgr::startMenuMusicFade` itself - a pure call-through into the out-of-scope
///   `MenuMusicHandler` - or `MenuMusicHandler::startFade` itself, squarely inside that same out-of-scope
///   class), as does the single-address `startMenuMusic()` (same `menu_music_handler_ptr`-gated
///   call-through shape, plus a call to `initMenuMusic` - see above). Left un-detoured.
///
/// registers the Lua functions
pub fn init() {
    // get_date() - no args
    lua_fn!("get_date", "Returns current in-game date/time", "get_date()", || {
        match command_get_date_str(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // add_cash(amount) - single f32 arg
    lua_fn!("add_cash", "Adds cash to player's budget", "add_cash(amount)", |amount: f32| {
        let amount_str = amount.to_string();
        match command_add_cash(vec![&amount_str]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // enable_dev_mode(enabled) - bool arg
    lua_fn!(
        "enable_dev_mode",
        "Enables/disables developer mode",
        "enable_dev_mode(true/false)",
        |enabled: bool| {
            let enabled_str = enabled.to_string();
            match command_enable_dev_mode(vec![&enabled_str]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // zoostats() - no args
    lua_fn!("zoostats", "Returns zoo statistics", "zoostats()", || {
        match command_zoostats(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    if let Err(e) = unsafe { gamemgr_lifecycle_detours::init_detours() } {
        error!("Failed to initialise ZTGameMgr lifecycle detours: {e:?}");
    }

    if let Err(e) = unsafe { gamemgr_finance_detours::init_detours() } {
        error!("Failed to initialise ZTGameMgr finance/date detours: {e:?}");
    }
}

/// Stage 1-3's vtable detours (`setNewGameDefaults`/`save`/`load`/`update`/`updateSim`, per
/// `openzt/plans/ztgamemgr-implementation-plan.md`); named `lifecycle` to distinguish from
/// `gamemgr_finance_detours` below, which covers Stage 4's non-virtual `addCash`/`subtractCash`/
/// date-family helpers.
#[detour_mod]
mod gamemgr_lifecycle_detours {
    use super::*;

    #[detour(SET_NEW_GAME_DEFAULTS)]
    unsafe extern "thiscall" fn set_new_game_defaults(this: *const u32, config: *const u32, is_new_game: bool) {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.set_new_game_defaults(config, is_new_game);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.save(file) as u32
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> u8 {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.load(file, version) as u8
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, delta: u32) {
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.update(delta);
    }

    #[detour(UPDATE_SIM)]
    unsafe extern "thiscall" fn update_sim(this: *const u32, delta: u32) {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.update_sim(delta);
    }

    /// `START`/`STOP`'s real `generated.rs` entries are `extern "fastcall" fn(i32)` (OOAnalyzer's
    /// single-int-param `__fastcall` recovery for these two, same ECX-passed-`this` shape as `thiscall`
    /// for a one-arg call) - the detour signature below matches that exactly, per this codebase's existing
    /// `ztshow.rs::CALCULATE_PERCENT_ADJUSTMENT` precedent for a fastcall single-param detour.
    #[detour(START)]
    unsafe extern "fastcall" fn start(this: i32) {
        unsafe { mut_from_memory::<ZTGameMgr>(this as *const u32) }.start();
    }

    #[detour(STOP)]
    unsafe extern "fastcall" fn stop(this: i32) {
        unsafe { mut_from_memory::<ZTGameMgr>(this as *const u32) }.stop();
    }
}

/// Stage 4's non-virtual finance/date detours (`addCash`/`subtractCash`/`getDate`/`isGameDate`/
/// `isRealWorldDate`/`timeAgo`/`hoursAgo`, per `openzt/plans/ztgamemgr-implementation-plan.md`).
#[detour_mod]
mod gamemgr_finance_detours {
    use super::*;

    #[detour(ADD_CASH)]
    unsafe extern "thiscall" fn add_cash(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.add_cash(amount);
    }

    /// The real signature carries a trailing `bool` (`ZTGameMgr::subtractCash(float, bool)`, per the
    /// `.asm`'s `RET 8`) that neither platform's compiled body reads - discarded here, only present so
    /// the detour's stack accounting matches the real function's.
    #[detour(SUBTRACT_CASH)]
    unsafe extern "thiscall" fn subtract_cash(this: *const u32, amount: f32, _unused: bool) {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.subtract_cash(amount);
    }

    #[detour(GET_DATE)]
    unsafe extern "thiscall" fn get_date(this: *const u32, out: *const FILETIME) -> *const FILETIME {
        let ticks = unsafe { ref_from_memory::<ZTGameMgr>(this) }.get_date();
        unsafe { *(out as *mut FILETIME) = ticks_to_filetime(ticks) };
        out
    }

    #[detour(IS_GAME_DATE)]
    unsafe extern "thiscall" fn is_game_date(this: *const u32, day: u32, month: u32) -> u32 {
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.is_game_date(day, month) as u32
    }

    #[detour(IS_REAL_WORLD_DATE)]
    unsafe extern "stdcall" fn is_real_world_date(day: i32, month: u32) -> u32 {
        ZTGameMgr::is_real_world_date(day as u32, month) as u32
    }

    #[detour(TIME_AGO)]
    unsafe extern "thiscall" fn time_ago(this: *const u32, out: *const FILETIME, reference: FILETIME) -> *const FILETIME {
        let result = unsafe { ref_from_memory::<ZTGameMgr>(this) }.time_ago(filetime_to_ticks(reference));
        unsafe { *(out as *mut FILETIME) = ticks_to_filetime(result) };
        out
    }

    #[detour(HOURS_AGO)]
    unsafe extern "thiscall" fn hours_ago(this: *const u32, reference_low: u32, reference_high: i32) -> u64 {
        let reference = ((reference_high as u32 as u64) << 32) | reference_low as u64;
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.hours_ago(reference)
    }

    /// Packs [`ZTGameMgr::animal_time_ago`]'s bucket into the low dword of the real `ulonglong`
    /// register-pair return - the high dword is `hoursAgo`'s own leftover, not part of the bucket value,
    /// so `0` here (rather than trying to reproduce genuine register leftover) matches this codebase's
    /// existing convention of not fabricating undefined upper bits.
    #[detour(ANIMAL_TIME_AGO)]
    unsafe extern "thiscall" fn animal_time_ago(this: *const u32, reference_low: u32, reference_high: i32) -> u64 {
        let reference = ((reference_high as u32 as u64) << 32) | reference_low as u64;
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.animal_time_ago(reference) as u64
    }

    #[detour(PEOPLE_TIME_AGO)]
    unsafe extern "thiscall" fn people_time_ago(this: *const u32, reference_low: u32, reference_high: i32) -> i8 {
        let reference = ((reference_high as u32 as u64) << 32) | reference_low as u64;
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.people_time_ago(reference) as i8
    }

    #[detour(OVERRIDE_NEW_GAME_DEFAULTS)]
    unsafe extern "thiscall" fn override_new_game_defaults(this: *const u32, config: *const u32) {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.override_new_game_defaults(config);
    }
}

/// Live-comparison test support for `reimplementation_tests`. Unlike `ZTAwardMgr` (fixed global
/// address, no standalone-instance capability), `ZTGameMgr` has a genuine free-function constructor
/// (`standalone::CREATE_ZTGAME_MGR`) that `operator_new`s a fresh `0x11b0`-byte block and returns it,
/// entirely independent of the real `GLOBAL_ZTGameMgr` singleton - enabling the same "build a second
/// standalone instance, drive real vanilla `.original()` calls against one and the Rust
/// reimplementation against the other" pattern `ztthoughtmgr`/`ztmegatilemgr` use.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// Builds a standalone `ZTGameMgr` via the real vanilla free-function constructor. Confirmed via
    /// `_CreateZTGameMgr.c` that construction explicitly zeroes `started` (offset `0x4`),
    /// `soundscape_ptr` (`0x1190`), and `menu_music_handler_ptr` (`0x11A4`) - `operator_new` itself does
    /// not guarantee zeroed memory, so any *other* field a later stage's test reads must either be
    /// confirmed as genuinely initialized by this constructor plus `set_new_game_defaults` (Stage 1), or
    /// the block explicitly `memset` to `0` first.
    pub(crate) fn build_standalone_mgr() -> *mut ZTGameMgr {
        unsafe { CREATE_ZTGAME_MGR.original()() as *mut ZTGameMgr }
    }

    /// Tears down a standalone instance built by [`build_standalone_mgr`] via the real vanilla deleting
    /// destructor (`ZTGAME_MGR_1`, `bDelete=1`) - safe here since this reimplementation stays
    /// vanilla-layout-compatible with no independent Rust-owned heap state to worry about
    /// double-freeing, unlike `ztthoughtmgr`'s intrusive list nodes (see `CLAUDE.md`'s cross-allocator
    /// warning).
    pub(crate) fn destroy_standalone_mgr(ptr: *mut ZTGameMgr) {
        if ptr.is_null() {
            return;
        }
        unsafe { ZTGAME_MGR_1.original()(ptr as *const u32, 1) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ztgamemgr_size_matches_real_allocation() {
        assert_eq!(std::mem::size_of::<ZTGameMgr>(), 0x11b0);
    }

    #[test]
    fn zoostatus_size_matches_embedded_region() {
        assert_eq!(std::mem::size_of::<ZooStatus>(), 0x1180);
    }

    #[test]
    fn rating_from_metric_zero_population_short_circuits() {
        assert_eq!(rating_from_metric(500, 0), 0);
        assert_eq!(rating_from_metric(-500, 0), 0);
    }

    #[test]
    fn rating_from_metric_applies_the_real_formula() {
        assert_eq!(rating_from_metric(0, 1), 50);
        assert_eq!(rating_from_metric(100, 1), 100);
        assert_eq!(rating_from_metric(-100, 1), 0);
        assert_eq!(rating_from_metric(300, 1), 200);
    }
}
