//! Compares real vanilla `ZTGameMgr` against its Rust reimplementation (production file
//! `openzt/src/ztgamemgr.rs`): standalone ctor round-trip, the `set_new_game_defaults`
//! byte-diff, save/load + update_sim + finance/date proptests, start/stop and is_new_game
//! smoke tests, and the real-zoo save/load round-trip registered last in `live_zoo_tests`.

use std::io::Write;
use std::mem::size_of;

use proptest::prelude::*;
use tracing::{error, info};
use windows::Win32::Foundation::FILETIME;

use openzt_detour::generated::bfconfigfile::{
    CONSTRUCTOR_0 as BFCONFIGFILE_CONSTRUCTOR_0, RELEASE as BFCONFIGFILE_RELEASE,
};
use openzt_detour::generated::standalone::OPERATOR_DELETE;
use openzt_detour::generated::ztgamemgr::{
    ADD_CASH as ZTGAMEMGR_ADD_CASH, ANIMAL_TIME_AGO as ZTGAMEMGR_ANIMAL_TIME_AGO,
    GET_DATE as ZTGAMEMGR_GET_DATE,
    HOURS_AGO as ZTGAMEMGR_HOURS_AGO, IS_GAME_DATE as ZTGAMEMGR_IS_GAME_DATE,
    IS_REAL_WORLD_DATE as ZTGAMEMGR_IS_REAL_WORLD_DATE, LOAD as ZTGAMEMGR_LOAD,
    OVERRIDE_NEW_GAME_DEFAULTS as ZTGAMEMGR_OVERRIDE_NEW_GAME_DEFAULTS,
    PEOPLE_TIME_AGO as ZTGAMEMGR_PEOPLE_TIME_AGO, SAVE as ZTGAMEMGR_SAVE,
    SET_NEW_GAME_DEFAULTS as ZTGAMEMGR_SET_NEW_GAME_DEFAULTS, SUBTRACT_CASH as ZTGAMEMGR_SUBTRACT_CASH,
    TIME_AGO as ZTGAMEMGR_TIME_AGO, UPDATE as ZTGAMEMGR_UPDATE, UPDATE_SIM as ZTGAMEMGR_UPDATE_SIM,
};

use crate::globals::{get_module_base, globals};
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::util::save_to_memory;
use crate::ztgamemgr::{self, live_support as gamemgr_live_support};
use crate::ztgamemgr_menumusichandler::live_support as menumusichandler_live_support;
use crate::zoostatus::ZooStatus;

/// `ZTGAMEMGR_STANDALONE_ROUNDTRIP` - builds one standalone `ZTGameMgr` via the real vanilla
/// free-function constructor (`ztgamemgr::live_support::build_standalone_mgr`, wrapping
/// `standalone::CREATE_ZTGAME_MGR`), confirms it's non-null, dumps its raw bytes and logs which
/// offsets are non-zero (resolves the "does `operator_new` zero the block" caveat empirically -
/// `_CreateZTGameMgr.c` explicitly zeroes `started`/`soundscape_ptr`/`menu_music_handler_ptr` but
/// says nothing about the rest), then immediately destroys it. No comparison logic - this only
/// proves the construct/destroy harness itself is safe before the `SET_NEW_GAME_DEFAULTS` test
/// builds on it. Doesn't need a live zoo (`GLOBAL_ZTWorldMgr`/`GLOBAL_ZTGameMgr`), so it runs
/// alongside the other standalone-only tests above, before `run_load_live_zoo`.
pub(crate) fn run_gamemgr_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_STANDALONE_ROUNDTRIP";
    let ptr = gamemgr_live_support::build_standalone_mgr();
    if ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
        }
        return true;
    }

    let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, struct_size) };
    let non_zero_offsets: Vec<usize> = bytes.iter().enumerate().filter(|(_, b)| **b != 0).map(|(offset, _)| offset).collect();
    info!(
        "{}: freshly-constructed standalone ZTGameMgr has {} non-zero bytes out of {}; offsets: {:?}",
        test_name,
        non_zero_offsets.len(),
        struct_size,
        non_zero_offsets
    );

    gamemgr_live_support::destroy_standalone_mgr(ptr);
    write_success_line(failure_log, test_name);
    false
}

/// `ZTGAMEMGR_CONSTRUCT` - builds one standalone `ZTGameMgr` via the real vanilla free-function
/// constructor (`gamemgr_live_support::build_standalone_mgr`) and one via the new Rust-native
/// `ztgamemgr::ZTGameMgr::construct` (`gamemgr_live_support::construct_standalone_via_rust`), then diffs
/// the full `0x11b0`-byte block. Unlike `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS`, this test can't pre-zero both
/// blocks to a shared starting point: each side's own constructor call performs its own real vanilla
/// `operator_new` internally, so whatever the allocator's freelist happened to hand back is already
/// live before either constructor body runs.
///
/// The exclusion ranges below were derived by first running this test with no exclusions, then
/// classifying every observed mismatch offset against `ztgamemgr.rs`'s struct/`construct()` and
/// `zoostatus.rs`'s struct/`init()` (read in full) - each one is either compiler-inserted alignment
/// padding (a `_padN`/`pad_0x*` field, never explicitly written by *any* code, real or reimplemented) or
/// a named field neither `CreateZTGameMgr` nor `ZooStatus::init` ever assigns. Every other byte in the
/// `0x11b0` block - the vast majority of the embedded `ZooStatus` region included - *is* deterministically
/// written by one side or the other and stays in the diff:
///
/// - `0x8..0x10` - `elapsed_sim_ticks`/`cash`: real `ZTGameMgr`-own fields `_CreateZTGameMgr.c` never
///   touches (only `started`/`soundscape_ptr`/`menu_music_handler_ptr`/the ini-read result are written
///   outside the embedded `ZooStatus` call).
/// - `0x29..0x2c` - `ZooStatus::_pad_0x19` (compiler padding after `finance_check_pending`).
/// - `0x32..0x3c` - `ZooStatus::_pad_0x22` + `animal_condition_counter_1` + `_pad_0x26` + `num_species` +
///   `_pad_0x2a`: `init`'s own doc comment states `num_species`/`animal_condition_counter_1` (`+0x24`/
///   `+0x28`, ZooStatus-relative) are deliberately left untouched (recomputed later by `calculateSums`),
///   and the two padding halves flanking them are never written either.
/// - `0x3e..0x40`, `0x42..0x44`, `0x46..0x48`, `0x4a..0x4c`, `0x4e..0x50`, `0x52..0x54` - `ZooStatus`'s
///   `_pad_0x2e`/`_pad_0x32`/`_pad_0x36`/`_pad_0x3a`/`_pad_0x3e`/`_pad_0x42`: compiler padding between
///   the `num_tired_guests`/`num_hungry_guests`/`num_thirst_guests`/`num_guests_restroom_need`/
///   `guest_condition_counter_1`/`guest_condition_counter_2` `u16` fields - every one of those named
///   fields itself *is* written (`= 0`) by `init` and correctly stays in the diff.
/// - `0x1160..0x1164` - `zoo_admission_cost` (`ZooStatus::admission_price`): `init`'s own doc comment
///   states this is deliberately never written (`set_adult_admission_price` clamps whatever was already
///   there into `[admission_price_min, admission_price_max]` = `[0.0, 100.0]`, but a fresh instance's
///   pre-clamp value is raw heap leftover).
/// - `0x1194..0x11a4` - `date`: a real `ZTGameMgr`-own field, never touched by `_CreateZTGameMgr.c`.
/// - `0x11ac..0x11b0` - `pad11`, the struct's trailing unaccounted space (see its own field comment).
///
/// Notably `menu_music_max_attenuation` (`0x11a8..0x11ac`) is *not* excluded and does compare
/// byte-identical between both sides - both call the same real `BFIniFile::read("UI",
/// "menuMusicMaxAttenuation", -1000)`, so this is a genuine, deterministic confirmation that
/// [`ztgamemgr::ZTGameMgr::construct`]'s ini-read port matches vanilla exactly.
pub(crate) fn run_gamemgr_construct_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_CONSTRUCT";

    let real_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_ptr = gamemgr_live_support::construct_standalone_via_rust();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: constructor returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: constructor returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
        }
        return true;
    }

    let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
    let real_bytes = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) };

    let excluded_ranges: [std::ops::Range<usize>; 12] = [
        0x8..0x10,
        0x29..0x2c,
        0x32..0x3c,
        0x3e..0x40,
        0x42..0x44,
        0x46..0x48,
        0x4a..0x4c,
        0x4e..0x50,
        0x52..0x54,
        0x1160..0x1164,
        0x1194..0x11a4,
        0x11ac..0x11b0,
    ];

    let mismatches: Vec<(usize, u8, u8)> = (0..struct_size)
        .filter(|i| !excluded_ranges.iter().any(|r| r.contains(i)))
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
    failed
}

/// `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` - builds two standalone `ZTGameMgr` instances (the
/// `ZTGAMEMGR_STANDALONE_ROUNDTRIP` harness), runs the real
/// `SET_NEW_GAME_DEFAULTS.original()` against one and the Rust
/// `ztgamemgr::ZTGameMgr::set_new_game_defaults` against the other (one shared, real vanilla-
/// constructed `BFConfigFile` passed to both - see below for why a zeroed/`Default` one crashes),
/// then diffs the full `0x11b0`-byte block.
///
/// **`config` must be built via the real vanilla constructor, not a zeroed `BFConfigFile::default()`.**
/// A zeroed instance reliably crashes inside vanilla `ZooStatus::init`'s
/// tail call into `BFConfigFile::getString` (`bfconfigfile::GET_STRING_1`) ->
/// `standalone::SEARCH_CONFIG_METHOD`, a null-pointer dereference (`mov edi,[edx+4]` with `edx=0`,
/// confirmed via `./openzt.bat crash-capture`). `BFConfigFile_BFConfigFile_0.c` shows why: a real
/// constructor allocates a red-black-tree sentinel node and links it to itself
/// (`node->left = node; node->right = node`) as `tree_root` - a *raw* `0` there (what `#[derive(Default)]`
/// produces) isn't a valid "empty tree", it's a dangling sentinel the search code doesn't guard against.
/// So this builds a real one via `BFCONFIGFILE_CONSTRUCTOR_0.original()` and tears it down via
/// `BFCONFIGFILE_RELEASE.original()` - matching the real `BFConfigFile::BFConfigFile`/`::release`
/// pair, entirely vanilla-allocator-owned (its tree node comes from vanilla's own small-object
/// freelist - see `BFConfigFile_BFConfigFile_0.c`'s `FUN_00402f85`/freelist-pop shape), so there's no
/// cross-allocator hazard freeing it via the matching real `release` call.
///
/// `is_new_game` is pinned to `false` on both sides rather than proptested: the `true` branch calls
/// through `GLOBAL_ZTAIMgr`'s real vtable slot `+0x4` (`openzt_detour::generated::ztaimgr::VIRT_METH_0X58F269`),
/// the *global*, shared AI manager singleton - not part of either standalone instance's own memory -
/// so triggering it here would be a real side effect on live game state, the same risk class as a
/// `ZTUI::main::set*`/`ZTSoundscape::update` call-through.
pub(crate) fn run_gamemgr_set_new_game_defaults_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS";

    let real_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
        }
        return true;
    }

    // Resolves the "operator_new doesn't zero memory" caveat the standalone harness leaves open:
    // ZooStatus::init reads at
    // least one field (`this[0xd].field_0xc`, per `ZooStatus_init.c`) before ever writing it in this
    // function - genuine uninitialized-read behavior in the real decompile, not a porting bug - so
    // two independently-allocated standalone instances can carry different heap leftovers there and
    // diverge downstream. Zeroing both blocks first (matching a fresh page from a clean process heap,
    // the same assumption vanilla's own single real construction relies on) makes both sides start
    // identical, so the diff below only ever reflects a genuine `set_new_game_defaults` difference.
    let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
    }

    let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
    let config_ptr = config.as_mut_ptr() as *const u32;
    let kind_tag_byte: u8 = 0;
    unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };

    unsafe {
        ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
        (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
    }

    unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

    let real_bytes = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) };

    // soundscape_ptr (0x1190)/menu_music_handler_ptr (0x11A4): both null pre-start() on a freshly
    // constructed instance, so these should already match - excluded only defensively.
    let excluded_ranges: [std::ops::Range<usize>; 2] = [0x1190..0x1194, 0x11A4..0x11A8];

    let mismatches: Vec<(usize, u8, u8)> = (0..struct_size)
        .filter(|i| !excluded_ranges.iter().any(|r| r.contains(i)))
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
    failed
}

/// Canonicalizes a `cash` bit pattern for `ZTGAMEMGR_SAVE_LOAD`'s comparison: any NaN collapses to a
/// single representative bit pattern, sidestepping both IEEE-754 `NaN != NaN` on direct equality and
/// a real x87-vs-SSE2 asymmetry: the real `load` quiets signaling NaNs, the reimplementation
/// preserves their bits.
///
/// A `cash` written as a *signaling* NaN (mantissa MSB `0`) is bit-identical in both sides' `save`
/// output (so `ZooStatus::save`, which never touches `cash`, isn't involved), but after `load` the
/// real side comes back as a *quiet* NaN (mantissa MSB `1`, i.e. `real_bits == reimpl_bits |
/// 0x0040_0000`) while the reimplemented side keeps the original signaling bits. `ZTGameMgr_load.asm`
/// pins this to `ZTGameMgr::load`'s own `this->cash =
/// local_8;` line: it compiles to `FLD float ptr [ESP+0x10]` / `FSTP float ptr [ESP]` (the field is
/// genuinely `float`-typed, even though the decompiler shows a raw `undefined4` dword copy) - x87
/// silences a signaling NaN by setting its quiet bit on any load/store through the FPU stack. This
/// reimplementation's `self.cash = cash;` is a plain SSE2 move with no FPU round-trip, so it preserves
/// the raw bits unchanged - not a port bug, just not bit-for-bit identical to a real `load` that
/// happens to touch a signaling NaN, which real gameplay never produces from a legitimate cash value.
fn normalize_cash_bits(cash: f32) -> u32 {
    if cash.is_nan() {
        0x7fc0_0000
    } else {
        cash.to_bits()
    }
}

/// `ZTGAMEMGR_SAVE_LOAD` - builds two standalone `ZTGameMgr` instances seeded via
/// `set_new_game_defaults` (real `SET_NEW_GAME_DEFAULTS.original()` run on both, via the
/// same real `BFConfigFile` construction `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` uses),
/// then for a generated `(cash, date_bytes, elapsed_sim_ticks, version)` seeds both instances
/// identically via the test-only `set_cash`/`set_date_bytes`/`set_elapsed_sim_ticks` accessors
/// (`ztgamemgr.rs`'s `Systemtime` is private, so the raw 16-byte `date` blob is generated/compared
/// byte-for-byte rather than field-by-field) and runs the real `SAVE.original()` against one and the
/// reimplemented `ztgamemgr::ZTGameMgr::save` against the other, capturing each side's
/// `WRITE_BYTES_TO_FILE` output via `io_redirect` - both should be byte-identical, since
/// `ZooStatus::save`'s own contribution is the *same* real function running against identically-seeded
/// memory on both sides. Then replays each side's captured bytes back into a fresh, zeroed third
/// standalone instance (real `LOAD.original()`/reimplemented `load()` respectively) and compares the
/// resulting `cash`/`date`/`elapsed_sim_ticks` fields - `version` is generated from both sides of the
/// `BFGameMgr::load` `0x48` threshold (`BFGameMgr_load.c`) so both the "read elapsed_sim_ticks" and
/// "zero it instead" branches get exercised. `cash` is compared via [`normalize_cash_bits`] rather
/// than raw `to_bits()` - see that function's doc comment for why.
pub(crate) fn run_gamemgr_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_SAVE_LOAD";

    let real_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
        }
        return true;
    }

    let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
    }

    let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
    let config_ptr = config.as_mut_ptr() as *const u32;
    let kind_tag_byte: u8 = 0;
    unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };
    unsafe {
        ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
        (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
    }
    unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let mut fail_flag = false;

    let dummy_file: u32 = 0;
    let date_bytes_strategy = prop::collection::vec(any::<u8>(), 16).prop_map(|v| {
        let mut out = [0u8; 0x10];
        out.copy_from_slice(&v);
        out
    });
    let version_strategy = prop_oneof![0u32..0x49, 0x49u32..0x1000];

    let result = runner.run(&(any::<f32>(), date_bytes_strategy, any::<u32>(), version_strategy), |(cash, date_bytes, elapsed_sim_ticks, version)| {
        unsafe {
            (*real_ptr).set_cash(cash);
            (*real_ptr).set_date_bytes(date_bytes);
            (*real_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
            (*reimpl_ptr).set_cash(cash);
            (*reimpl_ptr).set_date_bytes(date_bytes);
            (*reimpl_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
        }

        io_redirect::begin_capture();
        unsafe { ZTGAMEMGR_SAVE.original()(real_ptr as *const u32, &dummy_file as *const u32) };
        let real_bytes = io_redirect::end_capture();

        io_redirect::begin_capture();
        let _ = unsafe { (*reimpl_ptr).save(&dummy_file as *const u32) };
        let reimpl_bytes = io_redirect::end_capture();

        prop_assert_eq!(
            &real_bytes,
            &reimpl_bytes,
            "save byte mismatch for cash={}, date_bytes={:?}, elapsed_sim_ticks={}",
            cash,
            date_bytes,
            elapsed_sim_ticks
        );

        let real_load_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_load_ptr = gamemgr_live_support::build_standalone_mgr();
        prop_assume!(!real_load_ptr.is_null() && !reimpl_load_ptr.is_null());
        unsafe {
            std::ptr::write_bytes(real_load_ptr as *mut u8, 0, struct_size);
            std::ptr::write_bytes(reimpl_load_ptr as *mut u8, 0, struct_size);
        }

        io_redirect::begin_replay(real_bytes.clone());
        let real_load_ok = unsafe { ZTGAMEMGR_LOAD.original()(real_load_ptr as *const u32, &dummy_file as *const u32, version) };
        io_redirect::end_replay();

        io_redirect::begin_replay(reimpl_bytes.clone());
        let reimpl_load_ok = unsafe { (*reimpl_load_ptr).load(&dummy_file as *const u32, version) };
        io_redirect::end_replay();

        let real_result = unsafe { (normalize_cash_bits((*real_load_ptr).cash()), (*real_load_ptr).date_bytes(), (*real_load_ptr).elapsed_sim_ticks()) };
        let reimpl_result = unsafe { (normalize_cash_bits((*reimpl_load_ptr).cash()), (*reimpl_load_ptr).date_bytes(), (*reimpl_load_ptr).elapsed_sim_ticks()) };

        gamemgr_live_support::destroy_standalone_mgr(real_load_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_load_ptr);

        prop_assert_eq!(real_load_ok != 0, reimpl_load_ok, "load ok mismatch for version={}", version);
        prop_assert_eq!(real_result, reimpl_result, "load result mismatch for version={}", version);

        Ok(())
    });

    match result {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    gamemgr_live_support::destroy_standalone_mgr(real_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

    fail_flag
}

/// `ZTGAMEMGR_UPDATE_SIM` - builds two standalone `ZTGameMgr` instances seeded via
/// `set_new_game_defaults`, then for a generated `(delta, valid date fields,
/// elapsed_sim_ticks)` seeds both instances identically (`set_date_bytes`/`set_elapsed_sim_ticks`/
/// `set_day_changed_flag(false)`) and runs the real `UPDATE_SIM.original()` against one and the
/// reimplemented `ztgamemgr::ZTGameMgr::update_sim` against the other, comparing the resulting
/// `date`/`elapsed_sim_ticks`/`day_changed_flag` (via the same accessor methods - real and reimpl
/// memory share the same layout, so `(*real_ptr).date_bytes()` etc. work identically on either
/// pointer, no separate raw-offset reads needed).
///
/// Generated dates are constrained to valid `SYSTEMTIME` field ranges (year `1601..=9999`, month
/// `1..=12`, day `1..=28`, etc.) rather than arbitrary byte garbage: an invalid `SYSTEMTIME` makes
/// `SystemTimeToFileTime` fail, and vanilla's own decompiled body (`ZTGameMgr_updateSim.c`/`.asm`)
/// then proceeds with whatever garbage bytes happened to be on its stack in that case - not
/// reproducible from this side, and not the interesting path this test means to exercise (the real
/// date-arithmetic round-trip). `delta` is bounded to `0..=0x3e9` (1001) and the shared global tick
/// accumulator (`DAT_006394b8`) is reset to `0` immediately before *each* side's call - both
/// standalone instances' `updateSim` reads/writes the *same* process-wide global, so without this
/// reset the two sides would race each other into (and out of) the `ZTUI::main::set*`-refresh branch
/// depending purely on call order. This branch is never exercised here: calling those UI-refresh
/// functions against a standalone, non-globally-registered
/// `ZTGameMgr` risks corrupting real, unrelated live UI state (the rating-formula arithmetic that
/// branch also gates is covered separately, live-independent, by `ztgamemgr.rs`'s own
/// `rating_from_metric` unit tests).
///
/// Two more branches also get zero live exercise here, worth calling out explicitly rather than
/// leaving implicit: `soundscape_ptr`/`menu_music_handler_ptr` (the latter via [`Self::update`] below,
/// not `update_sim` itself) stay null on every standalone instance this battery ever builds - nothing
/// in the `set_new_game_defaults` seeding path or anywhere else in this port ever sets either
/// field (only `start()` does, per `ztgamemgr.rs`'s module doc comment) - so their
/// `ZTSoundscape::update`/`MenuMusicHandler::update` call-through branches never run, live or
/// otherwise. Acceptable since neither branch carries any `ZTGameMgr` logic of its own - both
/// delegate entirely to the embedded class's own `update` - but genuinely untested rather than
/// intentionally skipped.
///
/// One more branch is deliberately disabled rather than left to chance: `update_sim` calls
/// `ZooStatus::update` unconditionally every tick, which can roll
/// a real vanilla `fChance` and fire `ZooStatus::f_grant_donation` - a genuine UI-dialog path in the
/// same family the paragraph above already avoids, just reached through a different call chain
/// (`ZooStatus::update`, not `update_sim` itself). Both standalone instances get
/// `donation_chance_percent` forced to `0` right after `set_new_game_defaults`, before the proptest
/// loop starts, so that roll is always a deterministic no - `finance_check_pending` is forced `false`
/// alongside it, defensively, since `ZooStatus::update`'s `financeChecks` call-through carries the same
/// cross-allocator risk class. See the field-zeroing code's own comment for the crash this reproduces
/// and root-causes.
pub(crate) fn run_gamemgr_update_sim_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_UPDATE_SIM";

    let real_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
        }
        return true;
    }

    let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
    }

    let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
    let config_ptr = config.as_mut_ptr() as *const u32;
    let kind_tag_byte: u8 = 0;
    unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };
    unsafe {
        ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
        (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
    }
    unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

    // `set_new_game_defaults`'s real `economy.cfg`-driven `override` gives both instances a
    // genuinely non-zero `donation_chance_percent`, and `ZooStatus::update` (called unconditionally
    // every `update_sim`) rolls it via real vanilla `fChance` whenever `cash` is below
    // `DAT_00635128` - true for a fresh instance's starting `cash`. Across up to 256 proptest cases
    // that roll eventually lands, firing `f_grant_donation`, which - per its own doc comment - grants
    // through the *live* `GLOBAL_ZTGameMgr` (not `self`) and loads real localized strings via
    // `ZTApp::getApp`/`BFApp::loadString`. Both are unsafe to exercise here: `always_late_tests` (this
    // test's own group) runs on `ZTApp::updateSim`'s first tick, before `run_load_live_zoo` loads a
    // real zoo - `GLOBAL_ZTGameMgr` isn't the live game's own manager yet, and the string
    // registry/`BFApp` singleton isn't necessarily fully populated. Zeroing
    // `donation_chance_percent` on both instances makes every `fChance(0)` roll deterministically
    // false, so `f_grant_donation` never fires - the same "keep a real UI-dialog condition false"
    // discipline `ZOOSTATUS_CHECKS`'s own doc comment follows for this exact hazard.
    //
    // `finance_check_pending` is forced `false` alongside it for the same reason, defensively:
    // `set_new_game_defaults`/`init` already leave it `false` and nothing in this test's call path
    // sets it `true`, but `ZooStatus::update`'s `financeChecks` call-through walks `ZTWorldMgr`'s
    // freelist-backed building list - the same cross-allocator risk class as the donation-roll guard
    // above. An explicit write here, rather than relying on it
    // staying false by construction, means a future caller change can't silently reintroduce that
    // hazard into this test unnoticed.
    unsafe {
        (*((real_ptr as u32 + 0x10) as *mut ZooStatus)).donation_chance_percent = 0;
        (*((reimpl_ptr as u32 + 0x10) as *mut ZooStatus)).donation_chance_percent = 0;
        (*((real_ptr as u32 + 0x10) as *mut ZooStatus)).finance_check_pending = false;
        (*((reimpl_ptr as u32 + 0x10) as *mut ZooStatus)).finance_check_pending = false;
    }

    let dat_006394b8_addr = get_module_base("zoo.exe") as u32 + (0x006394b8u32 - 0x400000u32);

    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);

    let date_fields_strategy = (1601u16..=9999, 1u16..=12, 1u16..=28, 0u16..=23, 0u16..=59, 0u16..=59, 0u16..=999);

    let result = runner.run(&(0u32..=0x3e9, date_fields_strategy, any::<u32>()), |(delta, (year, month, day, hour, minute, second, milliseconds), elapsed_sim_ticks)| {
        let mut date_bytes = [0u8; 0x10];
        date_bytes[0..2].copy_from_slice(&year.to_le_bytes());
        date_bytes[2..4].copy_from_slice(&month.to_le_bytes());
        // date_bytes[4..6] (w_day_of_week) intentionally left 0 - ignored on input by SystemTimeToFileTime.
        date_bytes[6..8].copy_from_slice(&day.to_le_bytes());
        date_bytes[8..10].copy_from_slice(&hour.to_le_bytes());
        date_bytes[10..12].copy_from_slice(&minute.to_le_bytes());
        date_bytes[12..14].copy_from_slice(&second.to_le_bytes());
        date_bytes[14..16].copy_from_slice(&milliseconds.to_le_bytes());

        unsafe {
            (*real_ptr).set_date_bytes(date_bytes);
            (*real_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
            (*real_ptr).set_day_changed_flag(false);
            (*reimpl_ptr).set_date_bytes(date_bytes);
            (*reimpl_ptr).set_elapsed_sim_ticks(elapsed_sim_ticks);
            (*reimpl_ptr).set_day_changed_flag(false);
        }

        save_to_memory(dat_006394b8_addr, 0i32);
        unsafe { ZTGAMEMGR_UPDATE_SIM.original()(real_ptr as *const u32, delta) };

        save_to_memory(dat_006394b8_addr, 0i32);
        unsafe { (*reimpl_ptr).update_sim(delta) };

        let real_result = unsafe { ((*real_ptr).date_bytes(), (*real_ptr).elapsed_sim_ticks(), (*real_ptr).day_changed_flag()) };
        let reimpl_result = unsafe { ((*reimpl_ptr).date_bytes(), (*reimpl_ptr).elapsed_sim_ticks(), (*reimpl_ptr).day_changed_flag()) };

        prop_assert_eq!(
            real_result,
            reimpl_result,
            "updateSim mismatch for delta={}, date=({},{},{},{},{},{},{}), elapsed_sim_ticks={}",
            delta,
            year,
            month,
            day,
            hour,
            minute,
            second,
            milliseconds,
            elapsed_sim_ticks
        );

        Ok(())
    });

    let mut fail_flag = false;
    match result {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    // A small, dedicated `update(delta)` check - both instances have a null menu_music_handler_ptr
    // (never set by set_new_game_defaults or anything above), so this should be a pure no-op on both
    // sides with nothing to diff, but still gets *some* live coverage.
    unsafe {
        ZTGAMEMGR_UPDATE.original()(real_ptr as *const u32, 16);
        (*reimpl_ptr).update(16);
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

    fail_flag
}

/// `ZTGAMEMGR_FINANCE_DATE_HELPERS` - builds two standalone `ZTGameMgr` instances seeded via
/// `set_new_game_defaults`, then proptests
/// `addCash`/`subtractCash`/`getDate`/`isGameDate`/`isRealWorldDate`/`timeAgo`/`hoursAgo`/
/// `animalTimeAgo`/`peopleTimeAgo`/`overrideNewGameDefaults` real `.original()` vs the reimplemented
/// methods. `removedZooDoo` itself is not ported/detoured - see `ztgamemgr.rs`'s module doc comment
/// for why - so there's no test for it here.
///
/// `addCash`/`subtractCash` mutate `cash`, so both sides are reseeded to the same generated `cash`
/// value (`set_cash`) before each call and compared via `normalize_cash_bits` (NaN-safe, see that
/// helper's own doc comment). The date-family helpers (`getDate`/`isGameDate`/`timeAgo`/`hoursAgo`/
/// `animalTimeAgo`/`peopleTimeAgo`) don't mutate `this`, so both sides are seeded with the same
/// generated `date` bytes (`set_date_bytes`) and compared purely on return value - `isGameDate`'s real
/// return only defines `AL` (the upper EAX bits are undefined leftover), so only the low byte is
/// compared; `animalTimeAgo`/`peopleTimeAgo` similarly only compare the low dword/byte (see
/// their own doc comments for why the rest is undefined leftover). `isRealWorldDate` takes no
/// `this`/seeded state at all (calls `GetSystemTime` directly on both sides, independently,
/// microseconds apart) - comparing real vs reimpl booleans here only risks a spurious mismatch in the
/// astronomically unlikely case a call lands exactly on a day/month rollover between the two calls.
/// `overrideNewGameDefaults` mutates the embedded `ZooStatus` via the same real vanilla function on
/// both sides, so the whole `0x10..0x1160` region is byte-diffed afterward rather than compared
/// field-by-field.
///
/// Calls the real `TIME_AGO.original()`/`HOURS_AGO.original()` with their corrected signatures (see
/// those `FunctionDef`s' own doc comments) - the auto-generated ones would corrupt the stack/drop the
/// high dword.
pub(crate) fn run_gamemgr_finance_date_helpers_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_FINANCE_DATE_HELPERS";

    let real_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
        }
        return true;
    }

    let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
    }

    let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
    let config_ptr = config.as_mut_ptr() as *const u32;
    let kind_tag_byte: u8 = 0;
    unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };
    unsafe {
        ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, false);
        (*reimpl_ptr).set_new_game_defaults(config_ptr, false);
    }
    unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

    // Dedicated, kept-alive config for the overrideNewGameDefaults comparison below - the one above
    // is released immediately after seeding set_new_game_defaults, matching the rest of this test's
    // existing pattern.
    let mut override_config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
    let override_config_ptr = override_config.as_mut_ptr() as *const u32;
    unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(override_config_ptr, &kind_tag_byte as *const u8) };

    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);

    let date_fields_strategy = (1601u16..=9999, 1u16..=12, 1u16..=28, 0u16..=23, 0u16..=59, 0u16..=59, 0u16..=999);
    let day_strategy = prop_oneof![Just(0xffffffffu32), 1u32..=28];
    let month_strategy = prop_oneof![Just(0xffffffffu32), 1u32..=12];

    let result = runner.run(
        &(
            any::<f32>(),
            any::<f32>(),
            date_fields_strategy,
            day_strategy,
            month_strategy,
            any::<u64>(),
        ),
        |(add_amount, sub_amount, (year, month, day, hour, minute, second, milliseconds), game_day, game_month, reference)| {
            // addCash/subtractCash: reseed cash identically on both sides, then compare.
            unsafe {
                (*real_ptr).set_cash(0.0);
                (*reimpl_ptr).set_cash(0.0);
                ZTGAMEMGR_ADD_CASH.original()(real_ptr as *const u32, add_amount);
                (*reimpl_ptr).add_cash(add_amount);
            }
            prop_assert_eq!(
                normalize_cash_bits(unsafe { (*real_ptr).cash() }),
                normalize_cash_bits(unsafe { (*reimpl_ptr).cash() }),
                "addCash mismatch for amount={}",
                add_amount
            );

            unsafe {
                (*real_ptr).set_cash(0.0);
                (*reimpl_ptr).set_cash(0.0);
                ZTGAMEMGR_SUBTRACT_CASH.original()(real_ptr as *const u32, sub_amount, false);
                (*reimpl_ptr).subtract_cash(sub_amount);
            }
            prop_assert_eq!(
                normalize_cash_bits(unsafe { (*real_ptr).cash() }),
                normalize_cash_bits(unsafe { (*reimpl_ptr).cash() }),
                "subtractCash mismatch for amount={}",
                sub_amount
            );

            // getDate/isGameDate/timeAgo/hoursAgo: reseed date identically, then compare.
            let mut date_bytes = [0u8; 0x10];
            date_bytes[0..2].copy_from_slice(&year.to_le_bytes());
            date_bytes[2..4].copy_from_slice(&month.to_le_bytes());
            date_bytes[6..8].copy_from_slice(&day.to_le_bytes());
            date_bytes[8..10].copy_from_slice(&hour.to_le_bytes());
            date_bytes[10..12].copy_from_slice(&minute.to_le_bytes());
            date_bytes[12..14].copy_from_slice(&second.to_le_bytes());
            date_bytes[14..16].copy_from_slice(&milliseconds.to_le_bytes());
            unsafe {
                (*real_ptr).set_date_bytes(date_bytes);
                (*reimpl_ptr).set_date_bytes(date_bytes);
            }

            let mut real_date_out = FILETIME::default();
            let real_date_ptr = unsafe { ZTGAMEMGR_GET_DATE.original()(real_ptr as *const u32, &mut real_date_out as *const FILETIME) };
            prop_assert_eq!(real_date_ptr, &real_date_out as *const FILETIME, "getDate should return the out-pointer it was given");
            let real_date_ticks = ((real_date_out.dwHighDateTime as u64) << 32) | real_date_out.dwLowDateTime as u64;
            let reimpl_date = unsafe { (*reimpl_ptr).get_date() };
            prop_assert_eq!(real_date_ticks, reimpl_date, "getDate mismatch for date=({},{},{},{},{},{},{})", year, month, day, hour, minute, second, milliseconds);

            let real_is_game_date = unsafe { ZTGAMEMGR_IS_GAME_DATE.original()(real_ptr as *const u32, game_day, game_month) };
            let reimpl_is_game_date = unsafe { (*reimpl_ptr).is_game_date(game_day, game_month) };
            prop_assert_eq!(
                real_is_game_date & 0xff,
                reimpl_is_game_date as u32,
                "isGameDate mismatch for day={}, month={}, date=({},{},{},{},{},{},{})",
                game_day,
                game_month,
                year,
                month,
                day,
                hour,
                minute,
                second,
                milliseconds
            );

            let reference_low = reference as u32;
            let reference_high = (reference >> 32) as u32;
            let reference_filetime = FILETIME {
                dwLowDateTime: reference_low,
                dwHighDateTime: reference_high,
            };
            let mut real_time_ago_out = FILETIME::default();
            unsafe {
                ZTGAMEMGR_TIME_AGO.original()(real_ptr as *const u32, &mut real_time_ago_out as *const FILETIME, reference_filetime);
            }
            let real_time_ago_ticks = ((real_time_ago_out.dwHighDateTime as u64) << 32) | real_time_ago_out.dwLowDateTime as u64;
            let reimpl_time_ago = unsafe { (*reimpl_ptr).time_ago(reference) };
            prop_assert_eq!(real_time_ago_ticks, reimpl_time_ago, "timeAgo mismatch for reference={}", reference);

            let real_hours_ago = unsafe { ZTGAMEMGR_HOURS_AGO.original()(real_ptr as *const u32, reference_low, reference_high as i32) };
            let reimpl_hours_ago = unsafe { (*reimpl_ptr).hours_ago(reference) };
            prop_assert_eq!(real_hours_ago, reimpl_hours_ago, "hoursAgo mismatch for reference={}", reference);

            // animalTimeAgo/peopleTimeAgo: same seeded date/reference as timeAgo/hoursAgo above - only
            // the low dword of the real register-pair return is meaningful (see each method's own doc
            // comment), so only that half is compared.
            let real_animal_time_ago = unsafe { ZTGAMEMGR_ANIMAL_TIME_AGO.original()(real_ptr as *const u32, reference_low, reference_high as i32) };
            let reimpl_animal_time_ago = unsafe { (*reimpl_ptr).animal_time_ago(reference) };
            prop_assert_eq!(
                real_animal_time_ago as u32,
                reimpl_animal_time_ago,
                "animalTimeAgo mismatch for reference={}",
                reference
            );

            let real_people_time_ago = unsafe { ZTGAMEMGR_PEOPLE_TIME_AGO.original()(real_ptr as *const u32, reference_low, reference_high as i32) };
            let reimpl_people_time_ago = unsafe { (*reimpl_ptr).people_time_ago(reference) };
            prop_assert_eq!(
                real_people_time_ago as u8 as u32,
                reimpl_people_time_ago,
                "peopleTimeAgo mismatch for reference={}",
                reference
            );
            // overrideNewGameDefaults: the same real ZooStatus::override runs against both sides'
            // embedded ZooStatus with the same config, so the whole embedded region (0x10..0x1160)
            // should stay byte-identical afterward - nothing else in this test's proptest body touches
            // that region.
            unsafe {
                ZTGAMEMGR_OVERRIDE_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, override_config_ptr);
                (*reimpl_ptr).override_new_game_defaults(override_config_ptr);
            }
            let real_zoostatus_bytes = unsafe { std::slice::from_raw_parts((real_ptr as *const u8).add(0x10), 0x1150) };
            let reimpl_zoostatus_bytes = unsafe { std::slice::from_raw_parts((reimpl_ptr as *const u8).add(0x10), 0x1150) };
            prop_assert_eq!(real_zoostatus_bytes, reimpl_zoostatus_bytes, "overrideNewGameDefaults: embedded ZooStatus diverged");

            // isRealWorldDate: no seeded state - both sides call GetSystemTime independently.
            let real_is_real_world = unsafe { ZTGAMEMGR_IS_REAL_WORLD_DATE.original()(game_day as i32, game_month) };
            let reimpl_is_real_world = ztgamemgr::ZTGameMgr::is_real_world_date(game_day, game_month);
            prop_assert_eq!(
                (real_is_real_world & 0xff) != 0,
                reimpl_is_real_world,
                "isRealWorldDate mismatch for day={}, month={}",
                game_day,
                game_month
            );

            Ok(())
        },
    );

    let mut fail_flag = false;
    match result {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    unsafe { BFCONFIGFILE_RELEASE.original()(override_config_ptr) };

    if !fail_flag {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

    fail_flag
}

/// `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS_IS_NEW_GAME_SMOKE` - one-shot wiring check for
/// `set_new_game_defaults`'s `is_new_game=true` branch, which `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` above
/// deliberately never exercises (that test pins `is_new_game=false` on both sides - see its own doc
/// comment for why: the `true` branch calls through `GLOBAL_ZTAIMgr`'s real vtable slot, the live,
/// shared AI manager singleton, so exercising it is a real side effect on live game state, not
/// something a synthetic standalone instance can safely absorb before a zoo has even loaded). Deferred
/// to here, after `run_load_live_zoo`: a global pointer being
/// non-null (`GLOBAL_ZTAIMgr` is set well before this point) is not the same guarantee as the global's
/// *internal* state being genuinely constructed, and calling into a still-uninitialized manager's real
/// vtable slot pre-zoo-load is exactly how a call-through like `getBuildingList`'s crashes. Once a
/// real zoo is loaded, calling this is no different from what real "start new game" gameplay already
/// does.
///
/// Not a byte-diff: both sides call through to the *same* live `GLOBAL_ZTAIMgr` singleton, so a memory
/// comparison between the two standalone instances wouldn't reflect anything meaningful about either
/// side's own logic. This only confirms the call wiring (`this`/args passed into
/// `BFAIMGR_LOAD_DATA.original()`) doesn't crash on either side - a wrong-`this`/wrong-arg bug there is
/// exactly what `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` (pinned `false`) structurally cannot see. Registered
/// near the end of `battery.rs`'s `live_zoo_tests`, since `BFAIMgr::loadData` may have real side
/// effects on live AI state that earlier tests shouldn't have to account for.
pub(crate) fn run_gamemgr_set_new_game_defaults_is_new_game_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_SET_NEW_GAME_DEFAULTS_IS_NEW_GAME_SMOKE";

    let real_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
        }
        return true;
    }

    let struct_size = size_of::<ztgamemgr::ZTGameMgr>();
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);
    }

    let mut config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
    let config_ptr = config.as_mut_ptr() as *const u32;
    let kind_tag_byte: u8 = 0;
    unsafe { BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, &kind_tag_byte as *const u8) };

    unsafe {
        ZTGAMEMGR_SET_NEW_GAME_DEFAULTS.original()(real_ptr as *const u32, config_ptr, true);
        (*reimpl_ptr).set_new_game_defaults(config_ptr, true);
    }

    unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };

    info!("{}: is_new_game=true call-through completed without crashing on both sides", test_name);
    write_success_line(failure_log, test_name);

    gamemgr_live_support::destroy_standalone_mgr(real_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);

    false
}

/// `ZTGAMEMGR_START_STOP_SMOKE` - one-shot wiring check for the reimplemented `start`/`stop`
/// (`ztgamemgr.rs`'s module doc comment covers the full port). Not a byte-diff: `start`/`stop` read
/// the live `GLOBAL_ZTScenarioMgr`/`GLOBAL_ZTApp` singletons and call through to real vanilla
/// `ZTSoundscape`/`ZTUI::main::unpauseGame` - side effects on shared global/audio state, not something
/// a standalone instance's own memory can meaningfully diff against a second standalone instance. This
/// only confirms the call sequence (allocate/construct/init the soundscape, read the two new raw
/// globals, tail-call `unpauseGame`) doesn't crash. Deferred to run last, after `run_load_live_zoo` and
/// after the `is_new_game=true` smoke test above, for the same reason that one is deferred:
/// `GLOBAL_ZTScenarioMgr`/`GLOBAL_ZTApp` being non-null pointers is not the same guarantee as their
/// internal state being genuinely constructed pre-zoo-load (the same "non-null but uninitialized
/// registry" hazard class that crashes `getBuildingList`).
pub(crate) fn run_gamemgr_start_stop_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_START_STOP_SMOKE";

    let ptr = gamemgr_live_support::build_standalone_mgr();
    if ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
        }
        return true;
    }

    unsafe { (*ptr).start() };
    let started_after_start = unsafe { (*ptr).started() };
    let soundscape_after_start = unsafe { (*ptr).soundscape_ptr() };

    unsafe { (*ptr).stop() };
    let started_after_stop = unsafe { (*ptr).started() };
    let soundscape_after_stop = unsafe { (*ptr).soundscape_ptr() };

    let mut fail_flag = false;
    if !started_after_start || soundscape_after_start == 0 {
        fail_flag = true;
        error!(
            "{}: expected started=true and a non-null soundscape_ptr after start(), got started={} soundscape_ptr={:#x}",
            test_name, started_after_start, soundscape_after_start
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "Test Failed {}: after start(), started={} soundscape_ptr={:#x}\n",
                    test_name, started_after_start, soundscape_after_start
                )
                .as_bytes(),
            );
        }
    }
    if started_after_stop || soundscape_after_stop != 0 {
        fail_flag = true;
        error!(
            "{}: expected started=false and a null soundscape_ptr after stop(), got started={} soundscape_ptr={:#x}",
            test_name, started_after_stop, soundscape_after_stop
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "Test Failed {}: after stop(), started={} soundscape_ptr={:#x}\n",
                    test_name, started_after_stop, soundscape_after_stop
                )
                .as_bytes(),
            );
        }
    }

    if !fail_flag {
        info!("{}: start()/stop() call sequence completed without crashing, flags/pointer toggled as expected", test_name);
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(ptr);

    fail_flag
}

/// `ZTGAMEMGR_DESTRUCT` - the migration plan's Stage 4 live test (`plans/ztgamemgr-vanilla-storage-
/// migration-plan.md`). Builds two standalone `ZTGameMgr` instances, `start()`s both (populating a real
/// `soundscape_ptr` the same way `ZTGAMEMGR_START_STOP_SMOKE` does) and manually populates
/// `menu_music_handler_ptr` on both with a real, zero-initialized `MenuMusicHandler` block (via
/// `set_menu_music_handler_ptr` - `start()`/`set_new_game_defaults()` never touch this field, see that
/// accessor's own doc comment), then tears one down via the real vanilla deleting destructor
/// (`gamemgr_live_support::real_destructor_1`, `bDelete=1`) and the other via the new Rust-native
/// `ztgamemgr::ZTGameMgr::destruct` followed by a matching `OPERATOR_DELETE` - mirroring the real
/// deleting destructor's own destruct-then-free split. Repeated across several iterations in one process,
/// same reasoning as `ztthoughtmgr.rs`'s leak-only-teardown tests and this migration plan's own live-test
/// section: a real double-free/use-after-free/cross-allocator bug is likely to crash or corrupt heap
/// state detectably within a handful of iterations, and there's no second "real" pole left to byte-diff
/// against once both sides have been freed. Deferred to run after `ZTGAMEMGR_START_STOP_SMOKE`, since
/// `start()` needs a genuinely-loaded live zoo (`GLOBAL_ZTScenarioMgr`) for the same reason that test is
/// deferred.
pub(crate) fn run_gamemgr_destruct_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_DESTRUCT";
    const ITERATIONS: usize = 8;

    for i in 0..ITERATIONS {
        let real_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_ptr.is_null() || reimpl_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null on iteration {} (real={:?}, reimpl={:?})", test_name, i, real_ptr, reimpl_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null on iteration {}\n", test_name, i).as_bytes());
            }
            if !real_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_ptr);
            }
            if !reimpl_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
            }
            return true;
        }

        unsafe {
            (*real_ptr).start();
            (*reimpl_ptr).start();
        }

        let real_handler = menumusichandler_live_support::allocate_uninitialized();
        let reimpl_handler = menumusichandler_live_support::allocate_uninitialized();
        if real_handler.is_null() || reimpl_handler.is_null() {
            error!("{}: MenuMusicHandler allocation returned null on iteration {} (real={:?}, reimpl={:?})", test_name, i, real_handler, reimpl_handler);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: MenuMusicHandler allocation returned null on iteration {}\n", test_name, i).as_bytes());
            }
            gamemgr_live_support::destroy_standalone_mgr(real_ptr);
            gamemgr_live_support::destroy_standalone_mgr(reimpl_ptr);
            return true;
        }

        unsafe {
            (*real_handler).construct();
            (*reimpl_handler).construct();
            (*real_ptr).set_menu_music_handler_ptr(real_handler as u32);
            (*reimpl_ptr).set_menu_music_handler_ptr(reimpl_handler as u32);
        }

        // Real pole: the real vanilla deleting destructor - tears down the soundscape and menu-music-
        // handler through its own tail-merge shape (see the migration plan's re-verification section),
        // then frees `real_ptr` itself. Routed through the test_real trampoline (not `.original()`
        // directly) since Stage 5 detours this address - see `gamemgr_live_support::real_destructor_1`'s
        // own doc comment for why a raw `.original()` call isn't safe here in every build profile.
        gamemgr_live_support::real_destructor_1(real_ptr as *const u32, 1);

        // Reimpl pole: the new Rust-native destruct() followed by a matching OPERATOR_DELETE, mirroring
        // the real deleting destructor's own destruct-then-free split.
        unsafe {
            (*reimpl_ptr).destruct();
            OPERATOR_DELETE.original()(reimpl_ptr as u32);
        }
    }

    info!("{}: {} construct/start/populate-handler/destruct cycles completed without crashing on either pole", test_name, ITERATIONS);
    write_success_line(failure_log, test_name);
    false
}

/// ZTGAMEMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE: real-zoo save/load round-trip. Snapshots
/// `cash`/`date`/`elapsed_sim_ticks` directly off the real, live `globals().ztgamemgr()` singleton,
/// captures its own `save()`'s bytes, replays them into `load()` **in place on that same singleton**
/// (there's no cheap standalone copy of a fully-populated real `ZTGameMgr` to load into instead),
/// and asserts the three fields match afterward. Real `ZooStatus::save`/`load`
/// (`.original()`, an opaque un-reimplemented vanilla sub-object at `self+0x10`) run as a side
/// effect of both calls - presumed safe (persisted zoo-status counters only) but not independently
/// verified. Mutates the live singleton in place, so this is registered
/// last in `live_zoo_tests` - nothing later in the battery depends on these three fields being
/// untouched.
pub(crate) fn run_ztgamemgr_real_zoo_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGAMEMGR_REAL_ZOO_SAVE_LOAD_ROUNDTRIP_LIVE";
    const CURRENT_VERSION: u32 = 0x100;
    let mut fail_flag = false;

    let mgr_ptr = globals().ztgamemgr_ptr();
    if mgr_ptr.is_null() {
        info!("{}: GLOBAL_ZTGameMgr is null - nothing to round-trip, skipping", test_name);
        write_success_line(failure_log, &format!("{} (skipped: no live ZTGameMgr)", test_name));
        return false;
    }
    let mgr = unsafe { &mut *mgr_ptr };

    let cash_before = mgr.cash();
    let date_before = mgr.date_bytes();
    let ticks_before = mgr.elapsed_sim_ticks();

    let dummy_file: u32 = 0;
    io_redirect::begin_capture();
    let save_ok = mgr.save(&dummy_file as *const u32);
    let captured_bytes = io_redirect::end_capture();

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(
            format!("CHECKPOINT {} cash_before={} ticks_before={} bytes={}\n", test_name, cash_before, ticks_before, captured_bytes.len()).as_bytes(),
        );
    }

    if !save_ok {
        error!("{}: real save() returned failure", test_name);
        fail_flag = true;
    }

    io_redirect::begin_replay(captured_bytes);
    let load_ok = mgr.load(&dummy_file as *const u32, CURRENT_VERSION);
    io_redirect::end_replay();

    if !load_ok {
        error!("{}: real load() returned failure replaying its own save bytes", test_name);
        fail_flag = true;
    }

    let cash_after = mgr.cash();
    let date_after = mgr.date_bytes();
    let ticks_after = mgr.elapsed_sim_ticks();

    if cash_after != cash_before || date_after != date_before || ticks_after != ticks_before {
        error!(
            "{}: real zoo state didn't round-trip byte-identically (cash {}->{}, ticks {}->{}, date {:?}->{:?})",
            test_name, cash_before, cash_after, ticks_before, ticks_after, date_before, date_after
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "Test Failed {}: cash_before={} cash_after={} ticks_before={} ticks_after={} date_before={:?} date_after={:?}\n",
                    test_name, cash_before, cash_after, ticks_before, ticks_after, date_before, date_after
                )
                .as_bytes(),
            );
        }
        fail_flag = true;
    }

    if !fail_flag {
        info!("{}: real ZTGameMgr cash/date/elapsed_sim_ticks round-tripped byte-identically", test_name);
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
