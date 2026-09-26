#[allow(clippy::module_inception)]
pub mod ztgamemgr;
pub use ztgamemgr::*;

use openzt_detour::generated::{
    standalone::CREATE_ZTGAME_MGR,
    ztgamemgr::{
        ADD_CASH, ANIMAL_TIME_AGO, DESTRUCTOR_0, DESTRUCTOR_1, GET_DATE, HOURS_AGO, IS_GAME_DATE, IS_REAL_WORLD_DATE, LOAD,
        OVERRIDE_NEW_GAME_DEFAULTS, PEOPLE_TIME_AGO, SAVE, SET_NEW_GAME_DEFAULTS, START, STOP, SUBTRACT_CASH, TIME_AGO, UPDATE, UPDATE_SIM,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;
use windows::Win32::Foundation::FILETIME;

use crate::{
    lua_fn,
    systemtime::{filetime_to_ticks, ticks_to_filetime},
    util::{mut_from_memory, ref_from_memory},
};

/// Registers the Lua commands and installs this module's detours (see the module doc comment's
/// "Methods deliberately not detoured" section for what is intentionally left to vanilla).
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

    if let Err(e) = unsafe { gamemgr_allocator_detours::init_detours() } {
        error!("Failed to initialise ZTGameMgr allocator detours: {e:?}");
    }
}

/// The vtable/lifecycle detours (`setNewGameDefaults`/`save`/`load`/`update`/`updateSim`/`start`/
/// `stop`); named `lifecycle` to distinguish from `gamemgr_finance_detours` below, which covers the
/// non-virtual finance/date methods.
#[detour_mod]
mod gamemgr_lifecycle_detours {
    use super::*;

    #[detour(SET_NEW_GAME_DEFAULTS)]
    unsafe extern "thiscall" fn set_new_game_defaults(this: *const u32, config: *const u32, is_new_game: bool) {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.set_new_game_defaults(config, is_new_game);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.save(file)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.load(file, version)
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

/// The non-virtual finance/date detours (`addCash`/`subtractCash`/`getDate`/`isGameDate`/
/// `isRealWorldDate`/`timeAgo`/`hoursAgo`/`animalTimeAgo`/`peopleTimeAgo`/`overrideNewGameDefaults`).
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
    unsafe extern "thiscall" fn is_game_date(this: *const u32, day: u32, month: u32) -> bool {
        unsafe { ref_from_memory::<ZTGameMgr>(this) }.is_game_date(day, month)
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

/// Stage 5 of `plans/ztgamemgr-vanilla-storage-migration-plan.md`: swaps `ZTGameMgr`'s own allocator
/// from vanilla `operator_new`/`operator_delete` to Rust's global allocator (`Box`) - the change that
/// actually moves `GLOBAL_ZTGameMgr` off vanilla storage, rather than porting equivalent logic that
/// still runs on top of it (Stages 1-4). Confirmed safe to pursue before landing this: a full static
/// sweep of the decompile corpus plus two independent live `cdb` traces (breakpoints on
/// `CREATE_ZTGAME_MGR`/`DESTRUCTOR_0`/`DESTRUCTOR_1` across full launch-to-close sessions) each found
/// exactly one construct and one destroy per process, both inside `ZTApp`'s own one-shot init/
/// exit_override path - see the plan's own "go/no-go" section for the full evidence.
///
/// `CREATE_ZTGAME_MGR`'s real body (`operator_new(0x11b0)` then [`ZTGameMgr::init_fields`]) is replaced
/// with a `Box`-allocated equivalent; `DESTRUCTOR_0`/`DESTRUCTOR_1` are replaced with [`ZTGameMgr::destruct`]
/// followed by a `Box::from_raw` + `drop` in place of vanilla's own `operator_delete` (gated on
/// `DESTRUCTOR_1`'s `bDelete` byte, matching vanilla's own non-deleting/deleting split). A zeroed initial
/// value (`std::mem::zeroed`) is used rather than genuinely uninitialized memory - unlike [`ZTGameMgr::construct`]'s
/// vanilla-allocated pole (which deliberately leaves fields `CreateZTGameMgr` never writes as real heap
/// leftover, for `ZTGAMEMGR_CONSTRUCT`'s byte-diff fidelity), the production singleton is never byte-diffed
/// against real vanilla, and a zeroed `bool` field (`day_changed_flag`) is the only way to avoid genuine
/// undefined-behavior on an eventual read - see that field's own doc comment.
#[detour_mod]
mod gamemgr_allocator_detours {
    use super::*;

    #[detour(CREATE_ZTGAME_MGR)]
    unsafe extern "stdcall" fn create_zt_game_mgr() -> *const u32 {
        let this = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<ZTGameMgr>() }));
        unsafe { ZTGameMgr::init_fields(this) };
        this as *const u32
    }

    #[detour(DESTRUCTOR_0)]
    unsafe extern "thiscall" fn destructor_0(this: *const u32) -> *const u32 {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.destruct();
        this
    }

    #[detour(DESTRUCTOR_1)]
    unsafe extern "thiscall" fn destructor_1(this: *const u32, delete_flag: u8) -> *const u32 {
        unsafe { mut_from_memory::<ZTGameMgr>(this) }.destruct();
        if delete_flag != 0 {
            drop(unsafe { Box::from_raw(this as *mut ZTGameMgr) });
        }
        this
    }

    /// Trampolines to the real vanilla bodies for the reimplementation-test battery's "real vanilla"
    /// pole, once `init_detours()` has patched these three addresses: `.original()` on them is a raw
    /// address cast in release builds, so it would re-enter the Rust detours above instead of reaching
    /// vanilla (debug builds route `.original()` through the registry's trampolines correctly, but the
    /// battery's vanilla pole must stay genuine in **every** profile - same reasoning as
    /// `ztgamemgr_menumusichandler.rs`'s own `test_real` module). Lives inside the detour module because
    /// the generated `*_DETOUR` statics are module-private.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn create_zt_game_mgr() -> *const u32 {
            unsafe { super::CREATE_ZTGAME_MGR_DETOUR.call() }
        }

        pub(crate) fn destructor_1(this: *const u32, delete_flag: u8) -> *const u32 {
            unsafe { super::DESTRUCTOR_1_DETOUR.call(this, delete_flag) }
        }
    }
}

/// Live-comparison test support for `reimplementation_tests`. Unlike `ZTAwardMgr` (fixed global
/// address, no standalone-instance capability), `ZTGameMgr` has a genuine free-function constructor
/// (`standalone::CREATE_ZTGAME_MGR`) that `operator_new`s a fresh `0x11b0`-byte block and returns it,
/// entirely independent of the real `GLOBAL_ZTGameMgr` singleton - enabling the same "build a second
/// standalone instance, drive real vanilla `.original()` calls against one and the Rust
/// reimplementation against the other" pattern `ztthoughtmgr`/`ztmegatilemgr` use.
#[cfg(feature = "reimplementation-tests")]
pub mod live_support {
    use super::*;
    use openzt_detour::generated::standalone::OPERATOR_DELETE;

    /// Builds a standalone `ZTGameMgr` via the real vanilla free-function constructor. Confirmed via
    /// `_CreateZTGameMgr.c` that construction explicitly zeroes `started` (offset `0x4`),
    /// `soundscape_ptr` (`0x1190`), and `menu_music_handler_ptr` (`0x11A4`) - `operator_new` itself does
    /// not guarantee zeroed memory, so any *other* field a later test reads must either be
    /// confirmed as genuinely initialized by this constructor plus `set_new_game_defaults`, or
    /// the block explicitly `memset` to `0` first.
    ///
    /// Routes through `gamemgr_allocator_detours::test_real` rather than `CREATE_ZTGAME_MGR.original()`
    /// directly - Stage 5 detours that address, so a raw `.original()` call would re-enter our own
    /// `Box`-allocated detour in release builds instead of reaching real vanilla (see that module's own
    /// `test_real` doc comment).
    pub(crate) fn build_standalone_mgr() -> *mut ZTGameMgr {
        gamemgr_allocator_detours::test_real::create_zt_game_mgr() as *mut ZTGameMgr
    }

    /// Real-vanilla-reaching trampoline for `ZTGAMEMGR_DESTRUCT`'s own real pole - see
    /// `gamemgr_allocator_detours::test_real`'s doc comment for why a raw `DESTRUCTOR_1.original()` call
    /// isn't safe once that address is detoured (Stage 5).
    pub(crate) fn real_destructor_1(this: *const u32, delete_flag: u8) -> *const u32 {
        gamemgr_allocator_detours::test_real::destructor_1(this, delete_flag)
    }

    /// Tears down a standalone instance built by [`build_standalone_mgr`] via the new Rust-native
    /// [`ZTGameMgr::destruct`] followed by a matching `OPERATOR_DELETE` - the migration plan's Stage 4,
    /// confirmed equivalent to the real vanilla deleting destructor (`DESTRUCTOR_1`, `bDelete=1`) by
    /// `ZTGAMEMGR_DESTRUCT`'s own dedicated live test before this switch was made. Safe here since this
    /// reimplementation stays vanilla-layout-compatible with no independent Rust-owned heap state to
    /// worry about double-freeing, unlike `ztthoughtmgr`'s intrusive list nodes (see `CLAUDE.md`'s
    /// cross-allocator warning). `ptr` itself is always a real vanilla-`operator_new`-allocated block
    /// (from [`build_standalone_mgr`]), never a `Box`-allocated one, so freeing it via vanilla
    /// `OPERATOR_DELETE` here stays correct regardless of Stage 5's production allocator swap.
    pub(crate) fn destroy_standalone_mgr(ptr: *mut ZTGameMgr) {
        if ptr.is_null() {
            return;
        }
        unsafe {
            (*ptr).destruct();
            OPERATOR_DELETE.original()(ptr as u32);
        }
    }

    /// Builds a standalone `ZTGameMgr` via the new Rust-native [`ZTGameMgr::construct`], for
    /// `ZTGAMEMGR_CONSTRUCT`'s comparison against [`build_standalone_mgr`]'s real vanilla pole. Torn down
    /// the same way as `build_standalone_mgr` (both still vanilla-allocated in this stage) via
    /// `destroy_standalone_mgr`.
    pub(crate) fn construct_standalone_via_rust() -> *mut ZTGameMgr {
        ZTGameMgr::construct()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zoostatus::ZooStatus;

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
