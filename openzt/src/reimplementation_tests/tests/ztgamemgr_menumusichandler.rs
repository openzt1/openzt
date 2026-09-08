//! Compares real vanilla `MenuMusicHandler` against its Rust reimplementation (production file
//! `openzt/src/ztgamemgr_menumusichandler.rs`): detour wiring/trampoline-routing checks, the
//! standalone constructor byte-diff, and ctor/init/start_play/start_fade/update comparisons.

use openzt_detour::FunctionDef;
use std::io::Write;
use std::mem::size_of;
use tracing::error;

use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::ztgamemgr_menumusichandler::{self, live_support as menumusichandler_live_support};

/// `MENUMUSICHANDLER_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `ztgamemgr_menumusichandler::init()`, and this asserts all five of its detours actually report
/// enabled. Without it, a silently-failed `init_detours()` (error logged, game continues on
/// vanilla) would leave the whole battery green while every hooked production path runs vanilla -
/// the trampoline-based comparisons below can't distinguish that from a working hook. Runs before
/// the other `MENUMUSICHANDLER_*` tests so a wiring failure is visible first.
pub(crate) fn run_menumusichandler_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "MENUMUSICHANDLER_DETOURS_ENABLED";
    let mut disabled: Vec<&'static str> = Vec::new();
    for (name, enabled) in menumusichandler_live_support::detour_status() {
        if !enabled {
            disabled.push(name);
        }
    }
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("detours not enabled: {disabled:?}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// `MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE` - debug-only anti-regression for
/// `openzt-detour`'s hook registry: `FunctionDef::original()` must return the *real vanilla*
/// function (routed through the detour's trampoline) even for the five addresses this battery
/// has itself hooked, not silently re-enter our own Rust detours. For each of them, asserts the
/// registry holds a trampoline, that `.original()` returns exactly that pointer value, and that
/// it differs from the raw address (zoo.exe has no ASLR, so an un-routed raw cast would compare
/// equal - pointer equality can't pass vacuously here the way an `.original() == .original()`
/// self-comparison would). Also asserts zero registry overflows: a full slot array fails open into
/// exactly the raw-cast behavior this test guards against. Release builds cfg this out (the raw
/// cast is release's documented `.original()`); the release battery is still run once-off since
/// its vanilla poles go through the `real_*` trampolines instead.
#[cfg(debug_assertions)]
pub(crate) fn run_menumusichandler_original_routes_to_trampoline_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztgamemgr_menumusichandler as mmh;

    /// `.original()`'s return value as a raw pointer value. The pointer is only inspected,
    /// never called.
    fn original_ptr<T>(def: &FunctionDef<T>) -> usize
    where
        T: retour::Function,
    {
        let original = unsafe { def.original() };
        original.to_ptr() as usize
    }

    let test_name = "MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE";
    let hooked: [(&'static str, u32, usize); 5] = [
        ("CONSTRUCTOR", mmh::MENU_MUSIC_HANDLER_1.address, original_ptr(&mmh::MENU_MUSIC_HANDLER_1)),
        ("INIT", mmh::INIT.address, original_ptr(&mmh::INIT)),
        ("START_PLAY", mmh::START_PLAY.address, original_ptr(&mmh::START_PLAY)),
        ("START_FADE", mmh::START_FADE.address, original_ptr(&mmh::START_FADE)),
        ("UPDATE", mmh::UPDATE.address, original_ptr(&mmh::UPDATE)),
    ];

    let mut failures: Vec<String> = Vec::new();
    for (name, address, original) in hooked {
        match openzt_detour::trampoline_for(address) {
            Some(trampoline) => {
                if original != trampoline {
                    failures.push(format!(
                        "{name} ({address:#010x}): .original() = {original:#010x} != registered trampoline {trampoline:#010x}"
                    ));
                }
                if original == address as usize {
                    failures.push(format!(
                        "{name} ({address:#010x}): .original() equals the raw address - routing fell back to the raw cast"
                    ));
                }
            }
            None => failures.push(format!(
                "{name} ({address:#010x}): no trampoline registered - detour() did not publish, or the registry overflowed"
            )),
        }
    }
    let overflow = openzt_detour::registry_overflow_count();
    if overflow != 0 {
        failures.push(format!("{overflow} address(es) failed to register in the hook registry (capacity overflow - fail-open raw casts)"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `MENUMUSICHANDLER_STANDALONE_ROUNDTRIP` - builds
/// two fresh `0x14`-byte standalone `MenuMusicHandler` blocks, runs the real vanilla constructor
/// (via the `real_constructor` trampoline) on one and the Rust reimplementation
/// (`MenuMusicHandler::construct`) directly on the other, then byte-diffs the full struct. Both sides
/// call through to the same real `BFIniFile::read("UI", "noMenuMusic", 0)`, so `ini_menu_music_disabled`
/// should come out identical too - with one acknowledged blind spot: on a default install the key is
/// absent, so both sides read 0 and a section/key argument-order bug would pass silently (the `.c`/
/// `.asm` confirm the order; setting `noMenuMusic=1` in `UI.ini` and re-running would exercise the
/// real disabled path instead of the marker-forced one). No exclusions needed for the fields the
/// constructor actually touches. Both blocks are pre-zeroed before either constructor runs: neither
/// the real constructor
/// nor `MenuMusicHandler::construct` writes the `_pad1`/`_pad2` bytes (the real side's padding comes
/// back as raw `operator_new` heap leftovers, not zeros) - same "operator_new doesn't zero memory"
/// caveat `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` documents.
pub(crate) fn run_menumusichandler_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "MENUMUSICHANDLER_STANDALONE_ROUNDTRIP";

    let real_ptr = menumusichandler_live_support::allocate_uninitialized();
    let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(reimpl_ptr);
        }
        return true;
    }

    let struct_size = size_of::<ztgamemgr_menumusichandler::MenuMusicHandler>();
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);

        menumusichandler_live_support::real_constructor(real_ptr as *const u32);
        (*reimpl_ptr).construct();
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) };
    let mismatches: Vec<(usize, u8, u8)> =
        (0..struct_size).filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None }).collect();

    let failed = !mismatches.is_empty();
    if failed {
        error!("{}: byte mismatch(es) (offset, real, reimpl): {:?}", test_name, mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: byte mismatch(es) (offset, real, reimpl): {:?}\n", test_name, mismatches).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    menumusichandler_live_support::destroy_standalone(real_ptr);
    menumusichandler_live_support::destroy_standalone(reimpl_ptr);
    failed
}

/// `MENUMUSICHANDLER_INIT` - constructs two fresh
/// standalone `MenuMusicHandler`s (real vanilla ctor / `MenuMusicHandler::construct`), then calls
/// `init` on each (real vanilla via the `real_init` trampoline / `MenuMusicHandler::init` - see
/// `run_menumusichandler_detours_enabled_test` for why `.original()` can't be used here) with a
/// guaranteed-missing filename - this avoids the "plays audio during the
/// test battery" side effect a real `sounds/*.wav` path would have, at the cost of only exercising
/// `init`'s allocation/attenuation-call shape, not `DX8SndMgr::attempt`'s success path. Compares the
/// `bool` return and the fields in [`menumusichandler_field_mismatches`]; `sound_ptr` is compared
/// for null-ness only (a real heap address, expected to differ between the two independent
/// allocations; both sides go through the same real `operator_new`, so a real vs. reimplemented
/// `SNDSound` should end up either both allocated or both null together, matching `init`'s own
/// success/failure branching).
///
/// A second phase calls `init` again on the already-init'ed pair - entering the "existing sound,
/// not playing" branch (`sound_ptr != 0`, `IS_PLAYING` false -> skip stop/release), then
/// re-allocating and re-running the failed attempt. The first init's `SNDSound` is deliberately
/// leaked on both sides (vanilla's own re-init path leaks it identically - 8 bytes once per battery
/// run); the teardown below releases only the current `sound_ptr`.
pub(crate) fn run_menumusichandler_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "MENUMUSICHANDLER_INIT";

    let real_ptr = menumusichandler_live_support::allocate_uninitialized();
    let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(reimpl_ptr);
        }
        return true;
    }

    const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
    const ATTENUATION: i32 = 0;
    let mut failed = false;

    let (real, reimpl) = unsafe { (&*real_ptr, &*reimpl_ptr) };
    let mut mismatches: Vec<String> = Vec::new();

    unsafe {
        menumusichandler_live_support::real_constructor(real_ptr as *const u32);
        (*reimpl_ptr).construct();

        // Phase 1: first init on a fresh constructor.
        let real_result = menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
        let reimpl_result = (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION) as u32;

        if (real_result != 0) != (reimpl_result != 0) {
            mismatches.push(format!("return value: real={real_result}, reimpl={reimpl_result}"));
        }
        mismatches.extend(menumusichandler_field_mismatches(real, reimpl));
        if !mismatches.is_empty() {
            error!("{}: mismatch(es): {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
            }
            failed = true;
        }

        // Phase 2: second init on the already-init'ed pair (see this test's doc comment).
        if !failed {
            let real_result = menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
            let reimpl_result = (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION) as u32;

            mismatches.clear();
            if (real_result != 0) != (reimpl_result != 0) {
                mismatches.push(format!("second-init return value: real={real_result}, reimpl={reimpl_result}"));
            }
            mismatches.extend(menumusichandler_field_mismatches(real, reimpl));
            if !mismatches.is_empty() {
                error!("{}: second-init mismatch(es): {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: second-init mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            }
        }
    }

    if !failed {
        write_success_line(failure_log, test_name);
    }

    menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
    menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
    failed
}

/// Field-by-field comparison shared by the `MenuMusicHandler` comparison tests - same comparison set
/// as `MENUMUSICHANDLER_INIT` (`sound_ptr` by null-ness only, everything else exactly).
fn menumusichandler_field_mismatches(
    real: &ztgamemgr_menumusichandler::MenuMusicHandler,
    reimpl: &ztgamemgr_menumusichandler::MenuMusicHandler,
) -> Vec<String> {
    let mut mismatches: Vec<String> = Vec::new();
    if (real.sound_ptr() != 0) != (reimpl.sound_ptr() != 0) {
        mismatches.push(format!("sound_ptr null-ness: real={:#x}, reimpl={:#x}", real.sound_ptr(), reimpl.sound_ptr()));
    }
    if real.fading() != reimpl.fading() {
        mismatches.push(format!("fading: real={}, reimpl={}", real.fading(), reimpl.fading()));
    }
    if real.fade_counter() != reimpl.fade_counter() {
        mismatches.push(format!("fade_counter: real={}, reimpl={}", real.fade_counter(), reimpl.fade_counter()));
    }
    if real.ini_menu_music_disabled() != reimpl.ini_menu_music_disabled() {
        mismatches.push(format!("ini_menu_music_disabled: real={}, reimpl={}", real.ini_menu_music_disabled(), reimpl.ini_menu_music_disabled()));
    }
    if real.warmup_ticks() != reimpl.warmup_ticks() {
        mismatches.push(format!("warmup_ticks: real={}, reimpl={}", real.warmup_ticks(), reimpl.warmup_ticks()));
    }
    mismatches
}

/// Writes marker values through raw field offsets (the struct's fields are private outside
/// `ztgamemgr_menumusichandler`, and its layout is `const`-asserted): `fading` (+0x4) = `fading`,
/// `fade_counter` (+0x8) = `fade_counter`, `ini_menu_music_disabled` (+0xc) = `ini_disabled`,
/// `warmup_ticks` (+0x10) = `warmup`.
///
/// # Safety
/// `ptr` must point at a live, `0x14`-byte standalone `MenuMusicHandler` allocation.
unsafe fn write_menumusichandler_markers(ptr: *mut ztgamemgr_menumusichandler::MenuMusicHandler, fading: u8, fade_counter: i32, ini_disabled: u8, warmup: i32) {
    let base = ptr as *mut u8;
    unsafe {
        base.add(0x4).write(fading);
        (base.add(0x8) as *mut i32).write(fade_counter);
        base.add(0xc).write(ini_disabled);
        (base.add(0x10) as *mut i32).write(warmup);
    }
}

/// `MENUMUSICHANDLER_START_PLAY` - constructs and
/// `init`s two fresh standalone `MenuMusicHandler`s (real vanilla / Rust reimplementation, same
/// guaranteed-missing filename as `MENUMUSICHANDLER_INIT` - see that test's doc comment for why no
/// real `.wav` path), then calls `startPlay` on each (real vanilla via the `real_start_play`
/// trampoline / `MenuMusicHandler::start_play`) and field-diffs. With `attempt` known to have failed, the
/// `VALID`-gated play branch is skipped on both sides while the unconditional tail (clear
/// `fading`/`fade_counter`, `SET_FADE_ATTENUATION(0)`/`SET_VOLUME(0)`) still runs - vanilla makes
/// those last two calls even on a failed-attempt sound, so both sides exercise the same calls.
/// A second phase then covers `startPlay`'s first gate: with `ini_menu_music_disabled` forced to 1
/// and non-zero marker values written into `fading`/`fade_counter` on both sides, another `startPlay`
/// on each must leave both untouched (without the gate, `startPlay` would have cleared them to 0).
pub(crate) fn run_menumusichandler_start_play_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "MENUMUSICHANDLER_START_PLAY";

    let real_ptr = menumusichandler_live_support::allocate_uninitialized();
    let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(reimpl_ptr);
        }
        return true;
    }

    const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
    const ATTENUATION: i32 = 0;
    let mut failed = false;

    unsafe {
        menumusichandler_live_support::real_constructor(real_ptr as *const u32);
        (*reimpl_ptr).construct();
        menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
        (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION);

        // Phase 1: the normal path (gates open, VALID-gated play branch skipped by the failed
        // attempt, unconditional tail runs).
        menumusichandler_live_support::real_start_play(real_ptr as *const u32);
        (*reimpl_ptr).start_play();
        let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
        if !mismatches.is_empty() {
            error!("{}: startPlay phase mismatches: {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: startPlay phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
            }
            failed = true;
        }

        // Phase 2: the ini gate - forced-disabled plus non-zero markers must survive untouched.
        if !failed {
            write_menumusichandler_markers(real_ptr, 1, 77, 1, 0);
            write_menumusichandler_markers(reimpl_ptr, 1, 77, 1, 0);
            menumusichandler_live_support::real_start_play(real_ptr as *const u32);
            (*reimpl_ptr).start_play();
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: ini-gate phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: ini-gate phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            } else if (*real_ptr).fading() != 1 || (*real_ptr).fade_counter() != 77 || (*real_ptr).ini_menu_music_disabled() != 1 {
                // Both sides agreeing isn't enough here - the gate's whole point is that nothing
                // changes, so also check the markers really did survive on the real side (a broken
                // port and a broken gate would agree with each other just as well as two working
                // sides would).
                let msg = format!(
                    "ini-gate markers did not survive: fading={}, fade_counter={}, ini={}",
                    (*real_ptr).fading(), (*real_ptr).fade_counter(), (*real_ptr).ini_menu_music_disabled()
                );
                error!("{}: {}", test_name, msg);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                }
                failed = true;
            }
        }

        if !failed {
            write_success_line(failure_log, test_name);
        }
    }

    menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
    menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
    failed
}

/// `MENUMUSICHANDLER_START_FADE` - calls
/// `startFade` (real vanilla via the `real_start_fade` trampoline / `MenuMusicHandler::start_fade`)
/// on warm (already-`init`ed)
/// standalone instances and compares `fading`/`fade_counter`. The positive branch (`IS_PLAYING` true
/// -> arm the fade) needs genuinely playing audio, so - same missing-filename caveat as
/// `MENUMUSICHANDLER_INIT`/`_START_PLAY` - only the gate paths are exercised live: with a
/// failed-attempt sound, `IS_PLAYING` reads false and `startFade` must leave everything untouched;
/// with `fading` marker-forced to 1, the already-fading gate must also leave everything untouched
/// (markers checked for survival, not just real-vs-reimpl equality); and a construct-only pair
/// (`sound_ptr` still 0) covers the null-sound gate.
pub(crate) fn run_menumusichandler_start_fade_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "MENUMUSICHANDLER_START_FADE";

    let real_ptr = menumusichandler_live_support::allocate_uninitialized();
    let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(reimpl_ptr);
        }
        return true;
    }

    // A second, construct-only pair for the null-`sound_ptr` gate - no `init`, so these tear down
    // through plain `destroy_standalone`.
    let nullgate_real_ptr = menumusichandler_live_support::allocate_uninitialized();
    let nullgate_reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
    if nullgate_real_ptr.is_null() || nullgate_reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null for null-gate pair (real={:?}, reimpl={:?})", test_name, nullgate_real_ptr, nullgate_reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for null-gate pair (real={:?}, reimpl={:?})\n", test_name, nullgate_real_ptr, nullgate_reimpl_ptr).as_bytes());
        }
        if !nullgate_real_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(nullgate_real_ptr);
        }
        if !nullgate_reimpl_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(nullgate_reimpl_ptr);
        }
        menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
        menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
        return true;
    }

    const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
    const ATTENUATION: i32 = 0;
    let mut failed = false;

    unsafe {
        menumusichandler_live_support::real_constructor(real_ptr as *const u32);
        (*reimpl_ptr).construct();
        menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
        (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION);

        // Phase 1: warm instance, sound not playing - the IS_PLAYING gate must keep everything at 0.
        menumusichandler_live_support::real_start_fade(real_ptr as *const u32);
        (*reimpl_ptr).start_fade();
        let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
        if !mismatches.is_empty() {
            error!("{}: not-playing phase mismatches: {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: not-playing phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
            }
            failed = true;
        }

        // Phase 2: the already-fading gate - a marker-forced `fading` = 1 must survive untouched.
        if !failed {
            write_menumusichandler_markers(real_ptr, 1, 0x55, 0, 0);
            write_menumusichandler_markers(reimpl_ptr, 1, 0x55, 0, 0);
            menumusichandler_live_support::real_start_fade(real_ptr as *const u32);
            (*reimpl_ptr).start_fade();
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: already-fading phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: already-fading phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            } else if (*real_ptr).fading() != 1 || (*real_ptr).fade_counter() != 0x55 {
                let msg = format!("already-fading markers did not survive: fading={}, fade_counter={}", (*real_ptr).fading(), (*real_ptr).fade_counter());
                error!("{}: {}", test_name, msg);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                }
                failed = true;
            }
        }

        // Phase 3: the null-sound gate - construct-only instances (`sound_ptr` still 0).
        menumusichandler_live_support::real_constructor(nullgate_real_ptr as *const u32);
        (*nullgate_reimpl_ptr).construct();
        menumusichandler_live_support::real_start_fade(nullgate_real_ptr as *const u32);
        (*nullgate_reimpl_ptr).start_fade();
        let mismatches = menumusichandler_field_mismatches(&*nullgate_real_ptr, &*nullgate_reimpl_ptr);
        if !mismatches.is_empty() {
            error!("{}: null-sound phase mismatches: {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: null-sound phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
            }
            failed = true;
        }

        if !failed {
            write_success_line(failure_log, test_name);
        }
    }

    menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
    menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
    menumusichandler_live_support::destroy_standalone(nullgate_real_ptr);
    menumusichandler_live_support::destroy_standalone(nullgate_reimpl_ptr);
    failed
}

/// `MENUMUSICHANDLER_UPDATE` - constructs and
/// `init`s two fresh standalone `MenuMusicHandler`s (real vanilla / Rust reimplementation, same
/// guaranteed-missing filename as the other `MENUMUSICHANDLER_*` tests - see `MENUMUSICHANDLER_INIT`'s
/// doc comment for why no real `.wav` path), marker-forces `fading` = 1 (the one thing `startFade`
/// can't arm live without playing audio, per `MENUMUSICHANDLER_START_FADE`'s doc comment), then drives
/// `update` (real vanilla via the `real_update` trampoline / `MenuMusicHandler::update`) through its
/// state machine in phases:
///
/// 1. **Warm-up gating**: five `update(1000)` calls must each just increment `warmup_ticks` (0 -> 5)
///    and never touch `fade_counter` - checked for survival, not just real-vs-reimpl equality.
/// 2. **Accumulation**: the sixth `update(1000)` runs the accumulation path -
///    `fade_counter = trunc(1000 * 0.5)` = 500 - which also makes both sides push the new counter
///    through `SET_FADE_ATTENUATION`/`SET_VOLUME` on their failed-attempt `SNDSound`s, the same call
///    shape `MENUMUSICHANDLER_START_PLAY` already exercises live.
/// 3. **Delta gate**: `update(2000)` (the unsigned `>=` boundary itself) must leave `fade_counter` at
///    500.
/// 4. **Completion branch, not-playing sound**: with `fade_counter` marker-forced to 2995, one
///    `update(100)` crosses the 3000 threshold (3045) - and because the failed-attempt sound reports
///    not playing, the decompile leaves *everything* untouched (`IS_PLAYING` gates the clears, not the
///    other way round; see `MenuMusicHandler::update`'s doc comment). Survival-checked on
///    `fading`/`fade_counter`/`sound_ptr`.
///
/// The `IS_PLAYING`-**true** completion path (`STOP` + slot-0 release + `sound_ptr` = 0) needs genuinely
/// playing audio to enter live, so it isn't exercised here - its teardown calls are the exact
/// [`SNDSOUND_1`] release idiom `MENUMUSICHANDLER_INIT`'s teardown path already runs for real.
pub(crate) fn run_menumusichandler_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "MENUMUSICHANDLER_UPDATE";

    let real_ptr = menumusichandler_live_support::allocate_uninitialized();
    let reimpl_ptr = menumusichandler_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            menumusichandler_live_support::destroy_standalone(reimpl_ptr);
        }
        return true;
    }

    const FILENAME: &[u8] = b"__openzt_test_nonexistent_menu_music.wav\0";
    const ATTENUATION: i32 = 0;
    let mut failed = false;

    unsafe {
        menumusichandler_live_support::real_constructor(real_ptr as *const u32);
        (*reimpl_ptr).construct();
        menumusichandler_live_support::real_init(real_ptr as *const u32, FILENAME.as_ptr() as u32, ATTENUATION);
        (*reimpl_ptr).init(FILENAME.as_ptr() as *const i8, ATTENUATION);

        // Arm the fade marker-wise: `init` leaves `fading` = 0 and only a real playing sound would
        // let `startFade` arm it, so write it directly on both sides.
        write_menumusichandler_markers(real_ptr, 1, 0, 0, 0);
        write_menumusichandler_markers(reimpl_ptr, 1, 0, 0, 0);

        // Phase 1: five warm-up calls - `warmup_ticks` counts 0 -> 5, `fade_counter` untouched.
        for _ in 0..5 {
            menumusichandler_live_support::real_update(real_ptr as *const u32, 1000);
            (*reimpl_ptr).update(1000);
        }
        let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
        if !mismatches.is_empty() {
            error!("{}: warm-up phase mismatches: {:?}", test_name, mismatches);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: warm-up phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
            }
            failed = true;
        } else if (*real_ptr).warmup_ticks() != 5 || (*real_ptr).fade_counter() != 0 {
            let msg = format!(
                "warm-up markers did not advance as expected: warmup_ticks={}, fade_counter={}",
                (*real_ptr).warmup_ticks(), (*real_ptr).fade_counter()
            );
            error!("{}: {}", test_name, msg);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
            }
            failed = true;
        }

        // Phase 2: the sixth call (now warm) accumulates trunc(1000 * 0.5) = 500.
        if !failed {
            menumusichandler_live_support::real_update(real_ptr as *const u32, 1000);
            (*reimpl_ptr).update(1000);
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: accumulation phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: accumulation phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            } else if (*real_ptr).fade_counter() != 500 {
                let msg = format!("accumulation did not reach 500: fade_counter={}", (*real_ptr).fade_counter());
                error!("{}: {}", test_name, msg);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                }
                failed = true;
            }
        }

        // Phase 3: the delta gate - 2000 (the unsigned boundary itself) must leave everything alone.
        if !failed {
            menumusichandler_live_support::real_update(real_ptr as *const u32, 2000);
            (*reimpl_ptr).update(2000);
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: delta-gate phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: delta-gate phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            } else if (*real_ptr).fade_counter() != 500 {
                let msg = format!("delta gate did not hold: fade_counter={}", (*real_ptr).fade_counter());
                error!("{}: {}", test_name, msg);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                }
                failed = true;
            }
        }

        // Phase 4: completion branch with a not-playing sound - everything must survive untouched.
        if !failed {
            write_menumusichandler_markers(real_ptr, 1, 2995, 0, 5);
            write_menumusichandler_markers(reimpl_ptr, 1, 2995, 0, 5);
            menumusichandler_live_support::real_update(real_ptr as *const u32, 100);
            (*reimpl_ptr).update(100);
            let mismatches = menumusichandler_field_mismatches(&*real_ptr, &*reimpl_ptr);
            if !mismatches.is_empty() {
                error!("{}: not-playing completion phase mismatches: {:?}", test_name, mismatches);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: not-playing completion phase mismatches: {:?}\n", test_name, mismatches).as_bytes());
                }
                failed = true;
            } else if (*real_ptr).fading() != 1 || (*real_ptr).fade_counter() != 3045 || (*real_ptr).sound_ptr() == 0 {
                let msg = format!(
                    "not-playing completion markers did not survive: fading={}, fade_counter={}, sound_ptr={:#x}",
                    (*real_ptr).fading(), (*real_ptr).fade_counter(), (*real_ptr).sound_ptr()
                );
                error!("{}: {}", test_name, msg);
                if let Some(log_file) = failure_log {
                    let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
                }
                failed = true;
            }
        }

        if !failed {
            write_success_line(failure_log, test_name);
        }
    }

    menumusichandler_live_support::destroy_standalone_after_init(real_ptr);
    menumusichandler_live_support::destroy_standalone_after_init(reimpl_ptr);
    failed
}
