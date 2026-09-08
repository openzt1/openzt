//! Compares real vanilla `ZTSoundscape` against its Rust reimplementation (production file
//! `openzt/src/ztsoundscape.rs`): detour wiring/trampoline-routing checks, the standalone
//! constructor byte-diff, the `.rdata` fade-constant pins, and the live-zoo init/update
//! three-twin comparisons.

use openzt_detour::FunctionDef;
use openzt_detour::generated::bfscenariomgr::{
    GET_CROWD_AMBIENTS_NAME, GET_CROWD_CONFIG_NAME, GET_WORLD_AMBIENTS_NAME, GET_WORLD_CONFIG_NAME,
};
use std::ffi::{c_void, CStr};
use std::io::Write;
use std::mem::size_of;
use tracing::{error, info};

use crate::globals::{get_module_base, globals};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::util::{get_from_memory, save_to_memory};
use crate::ztsoundscape::{live_support as soundscape_live_support, ZTSoundscape};

/// `ZTSOUNDSCAPE_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `ztsoundscape::init()`, and this asserts all three of its detours actually report enabled.
/// Without it, a silently-failed `init_detours()` (error logged, game continues on vanilla) would
/// leave the whole battery green while every hooked production path runs vanilla - the
/// trampoline-based comparisons below can't distinguish that from a working hook. Runs before the
/// other `ZTSOUNDSCAPE_*` tests so a wiring failure is visible first.
pub(crate) fn run_ztsoundscape_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSOUNDSCAPE_DETOURS_ENABLED";
    let mut disabled: Vec<&'static str> = Vec::new();
    for (name, enabled) in soundscape_live_support::detour_status() {
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

/// `ZTSOUNDSCAPE_ORIGINAL_ROUTES_TO_TRAMPOLINE` - debug-only anti-regression for
/// `openzt-detour`'s hook registry, same shape as
/// [`run_menumusichandler_original_routes_to_trampoline_test`] over this class's three hooked
/// addresses: `FunctionDef::original()` must return the *real vanilla* function (routed through the
/// detour's trampoline) even for the addresses this battery has itself hooked, not silently re-enter
/// our own Rust detours. See that test's doc comment for the full rationale (pointer equality vs.
/// the raw address, registry-overflow fail-open check, release cfg-out).
#[cfg(debug_assertions)]
pub(crate) fn run_ztsoundscape_original_routes_to_trampoline_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztsoundscape as gen_ztsoundscape;

    /// `.original()`'s return value as a raw pointer value. The pointer is only inspected,
    /// never called.
    fn original_ptr<T>(def: &FunctionDef<T>) -> usize
    where
        T: retour::Function,
    {
        let original = unsafe { def.original() };
        original.to_ptr() as usize
    }

    let test_name = "ZTSOUNDSCAPE_ORIGINAL_ROUTES_TO_TRAMPOLINE";
    let hooked: [(&'static str, u32, usize); 3] = [
        ("UPDATE", gen_ztsoundscape::UPDATE.address, original_ptr(&gen_ztsoundscape::UPDATE)),
        ("INIT", gen_ztsoundscape::INIT.address, original_ptr(&gen_ztsoundscape::INIT)),
        ("CONSTRUCTOR", gen_ztsoundscape::CONSTRUCTOR.address, original_ptr(&gen_ztsoundscape::CONSTRUCTOR)),
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

/// `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP` - builds two fresh `0x54`-byte standalone `ZTSoundscape`
/// blocks, runs the real vanilla constructor on one and the Rust reimplementation
/// (`ZTSoundscape::construct`) on the other, then byte-diffs the full struct. The constructor is
/// pure constant writes, so the compare is meaningful with no exclusions.
///
/// Both blocks are pre-zeroed before either constructor runs: the ctor writes only 32 of the `0x54`
/// bytes (the three embedded slots' `{vtable, inner}` dwords and the two `Ambients` pointers) - the
/// scalars, the filename/atten tables, and the `+0xb` pad byte stay heap garbage on both sides until
/// `init` writes them. Comparing uninitialized memory would diff `operator_new` leftovers, not
/// constructor behavior (same "operator_new doesn't zero" precedent as
/// `MENUMUSICHANDLER_STANDALONE_ROUNDTRIP` and `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS`).
///
/// Pole note: the vanilla side goes through `soundscape_live_support::real_constructor` (a
/// `CONSTRUCTOR_DETOUR.call` trampoline) - a release build's raw-cast `.original()` would re-enter
/// the Rust detour and degenerate the test into Rust-vs-Rust.
pub(crate) fn run_ztsoundscape_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP";

    let real_ptr = soundscape_live_support::allocate_uninitialized();
    let reimpl_ptr = soundscape_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        if !real_ptr.is_null() {
            soundscape_live_support::destroy_standalone(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            soundscape_live_support::destroy_standalone(reimpl_ptr);
        }
        return true;
    }

    let struct_size = size_of::<ZTSoundscape>();
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);

        soundscape_live_support::real_constructor(real_ptr as *const c_void);
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

    soundscape_live_support::destroy_standalone(real_ptr);
    soundscape_live_support::destroy_standalone(reimpl_ptr);
    failed
}

/// re-declared here per the repo's no-shared-consts precedent (`ztsoundscape.rs` carries the
/// originals it actually uses). Values match zoo.exe's `.rdata` (see `ztsoundscape.rs`'s doc
/// comment for the derivation and the `fade_atten_a`/`fade_atten_b` unit tests that bake them in
/// as literals).
const FADE_DAT_0063542C_RVA: u32 = 0x0063542c - 0x400000;
const FADE_DAT_00635428_RVA: u32 = 0x00635428 - 0x400000;
const FADE_DAT_00635490_RVA: u32 = 0x00635490 - 0x400000;

/// `ZTSOUNDSCAPE_FADE_CONSTANTS` - `ZTSoundscape::update`'s fade-attenuation math
/// (`fade_atten_a`/`fade_atten_b`) reads three `.rdata` floats live via `get_module_base + RVA`, and
/// the whole f64-truncation-parity argument for those functions rests on those constants holding the
/// exact values confirmed by a PE-section parse (`DAT_0063542c` = f32 `0x38D1B717`, `DAT_00635428` =
/// `4500.0`, `DAT_00635490` = `1.0`). Nothing else in the battery would catch drift here:
/// `SET_FADE_ATTENUATION` is called on an opaque real `SNDSound` object, so `ZTSOUNDSCAPE_UPDATE`'s
/// struct-only compare never observes the actual attenuation argument the port computes from these
/// constants. This test closes that gap directly - no live zoo/game state needed, just the loaded
/// module's `.rdata`, so it runs in `always_late_tests()` with the other standalone-only tests.
pub(crate) fn run_ztsoundscape_fade_constants_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSOUNDSCAPE_FADE_CONSTANTS";

    let base = get_module_base("zoo.exe") as u32;
    let c1: f32 = get_from_memory(base + FADE_DAT_0063542C_RVA);
    let c2: f32 = get_from_memory(base + FADE_DAT_00635428_RVA);
    let c3: f32 = get_from_memory(base + FADE_DAT_00635490_RVA);

    let expected_c1 = f32::from_bits(0x38D1_B717);
    let expected_c2 = 4500.0_f32;
    let expected_c3 = 1.0_f32;

    let mut mismatches: Vec<String> = Vec::new();
    if c1.to_bits() != expected_c1.to_bits() {
        mismatches.push(format!(
            "DAT_0063542c: expected {:#010x} ({expected_c1}), got {:#010x} ({c1})",
            expected_c1.to_bits(),
            c1.to_bits()
        ));
    }
    if c2 != expected_c2 {
        mismatches.push(format!("DAT_00635428: expected {expected_c2}, got {c2}"));
    }
    if c3 != expected_c3 {
        mismatches.push(format!("DAT_00635490: expected {expected_c3}, got {c3}"));
    }

    if mismatches.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        error!("{}: mismatch(es): {:?}", test_name, mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
        }
        true
    }
}

/// `GLOBAL_ZTScenarioMgr`'s global-slot RVA (`ZTGameMgr_start.c`/`.asm` ground truth). Re-declared
/// here per the repo's no-shared-consts precedent (each file declares its own copy;
/// `ztgamemgr.rs` carries the original).
const GLOBAL_ZTSCENARIOMGR_RVA: u32 = 0x00638ff8 - 0x400000;

/// The shared game RNG state's RVA (`DAT_00638060`) that `ZTSoundscape::update`'s position jitter
/// advances through the classic MSVC LCG. Re-declared here per the same no-shared-consts precedent
/// (`ztsoundscape.rs` carries the original).
const GAME_RNG_RVA: u32 = 0x00638060 - 0x400000;

/// `ZTSOUNDSCAPE_INIT` - builds two fresh standalone `ZTSoundscape` blocks (real vanilla ctor /
/// [`ZTSoundscape::construct`]), **0xAA-fills both** before constructing, then calls `init` on
/// each - real vanilla (via the `real_init` trampoline) vs. the Rust port - and compares.
///
/// Inputs come from the live `GLOBAL_ZTScenarioMgr` singleton via its four real getter
/// call-throughs, captured **once** up front and handed to both poles, so the poles can't drift
/// apart on getter results. Runs in `live_zoo_tests()`, after `run_load_live_zoo`: pre-zoo the
/// scenario registry is non-null-but-uninitialized (the hazard class
/// `ZTGAMEMGR_START_STOP_SMOKE`'s doc describes), both `BFConfigFile::attempt`s would fail, and
/// the test would silently cover only the defaults/tail while looking green.
///
/// **Vanilla pole first, then a full snapshot of its block plus owned copies of every string its
/// pointer fields reference, and only then the reimpl pole.** The snapshot is load-bearing: both
/// poles' `init` calls reuse the same two *global* `BFConfigFile` instances, and the second
/// pole's `release`+`attempt`+parse frees/reallocates the parsed storage the first pole's
/// `crowd_filename`/`world_name` pointers point into - comparing both sides live would diff the
/// vanilla side against the *second* parse's buffers, not the values its own `init` stored. The
/// reimpl side is compared while its own parse is still live.
///
/// Comparison set (snapshot vs. the live reimpl block; masked regions re-covered by replacements):
/// - `+0x09` (`fade_step_in`, one of the two deliberately-uninitialized bytes): asserted still
///   `0xAA` on **both** sides - the byte-diff alone can't catch a port that wrongly *writes* a
///   byte both sides leave alone, but a raw filler assert does.
/// - `+0x1c..=0x2b` (`crowd_filename`): per-slot CStr **content** compare - the pointers
///   legitimately differ across the two parses.
/// - `+0x40..=0x43` (`world_snd.inner`): null-ness parity only. Per-attempt vanilla-owned
///   resource object (the same attempted name hands the two poles different handles), so a value
///   compare is wrong by construction.
/// - `+0x44..=0x47` (`world_name`): null-ness parity, then content compare when the vanilla
///   snapshot's is non-zero - its pointer also legitimately differs across parses.
/// - `+0x48..=0x4b` (`world_atten`, the second deliberately-uninitialized byte - untouched when
///   no world sound is configured): compared only when the vanilla snapshot's `world_name`
///   (`+0x44`) is non-zero.
/// - `+0x4c..=0x53` (both `Ambients*`, real heap addresses): null-ness parity only.
///
/// Everything else - the scalars, both idle crowd `SNDSound` slots, the world slot's vtable, and
/// the four `crowd_atten` values - is byte-compared as-is.
///
/// Distinctness probe: the four vanilla-side `crowd_filename` pointers are checked for pairwise
/// distinctness (all equal/overlapping would mean `getString` reuses a scratch buffer, parity
/// still holds, and the content compare degenerates to trivial). The result is recorded in this
/// test's own success line (direct file write - `info!` lines placed mid-test are lost to the
/// battery's tracing lossiness under `std::process::exit`). Relevant to `update`'s later filename
/// reads: parsed names live in per-key storage, not one scratch buffer.
///
/// Audible caveat: both poles run `init` for real, so the battery briefly plays the world sound
/// **twice, overlapping** (each side loops one until teardown stops it) - documented, not a
/// failure.
///
/// Teardown goes through [`soundscape_live_support::destroy_standalone_after_init`] on both
/// blocks, which releases the vanilla-allocated `Ambients` blocks via the real destructor and
/// stops each side's sound (see its doc comment for the cross-allocator reasoning).
///
/// Pole note: the vanilla side goes through `soundscape_live_support::real_init` (an
/// `INIT_DETOUR.call` trampoline) - a release build's raw-cast `.original()` would re-enter the
/// Rust detour and degenerate this test into Rust-vs-Rust. The four `bfscenariomgr` getter
/// captures above stay `.original()` - none of those is detoured.
///
/// Accepted gaps: `OPERATOR_NEW`'s failure paths (the store-`0` + skip-ctor propagation) can't be
/// exercised live; `world_snd.inner` is only null-ness-compared (see above).
pub(crate) fn run_ztsoundscape_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSOUNDSCAPE_INIT";

    let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
    if scenariomgr_ptr == 0 {
        info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
        return false;
    }

    // Capture the four getter results once; both poles get exactly these.
    let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
    let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
    let crowd_config = unsafe { GET_CROWD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };
    let world_config = unsafe { GET_WORLD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };

    let real_ptr = soundscape_live_support::allocate_uninitialized();
    let reimpl_ptr = soundscape_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes());
        }
        // Nothing was constructed or init'ed on this path, so a plain free is complete - the
        // dtor-aware path would walk 0xAA garbage.
        if !real_ptr.is_null() {
            soundscape_live_support::destroy_standalone(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            soundscape_live_support::destroy_standalone(reimpl_ptr);
        }
        return true;
    }

    // Owned copy of one pointed-to name; `None` = no usable pointer (null or 0xAA filler).
    fn copy_cstr(p: u32) -> Option<Vec<u8>> {
        if p == 0 || p == 0xAAAA_AAAA {
            return None;
        }
        Some(unsafe { CStr::from_ptr(p as *const i8) }.to_bytes().to_vec())
    }

    let struct_size = size_of::<ZTSoundscape>();
    let mut mismatches: Vec<String> = Vec::new();

    unsafe {
        // 0xAA fill (not zero) so every deliberately-uninitialized byte is detectably garbage.
        std::ptr::write_bytes(real_ptr as *mut u8, 0xAA, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0xAA, struct_size);

        soundscape_live_support::real_constructor(real_ptr as *const c_void);
        (*reimpl_ptr).construct();

        // Vanilla pole first: the generated INIT's params 2/3 carry a `*const u32` wart - cast
        // here, in the test, not inside the port (see `ZTSoundscape::init`'s doc comment).
        soundscape_live_support::real_init(
            real_ptr as *const c_void,
            crowd_ambients as *const u32,
            world_ambients as *const u32,
            crowd_config,
            world_config,
        );

    }

    // Snapshot before the reimpl pole re-parses the shared global config instances (see this
    // test's doc comment): the whole block; the pointer-valued fields get owned string copies
    // extracted below, before anything can invalidate the storage they reference.
    let vanilla_snap = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) }.to_vec();

    // Vanilla-side snapshot extraction (all safe - vanilla_snap is an owned copy), plus the
    // distinctness probe (see this test's doc comment): pointers equal -> getString shares a
    // scratch buffer and the content compare below degenerates to trivial.
    let dword = |bytes: &[u8], off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    let vanilla_name_ptrs: [u32; 4] = std::array::from_fn(|s| dword(&vanilla_snap, 0x1c + s * 4));
    let vanilla_crowd_names: [Option<Vec<u8>>; 4] = std::array::from_fn(|s| copy_cstr(vanilla_name_ptrs[s]));
    let vanilla_world_name_ptr = dword(&vanilla_snap, 0x44);
    let vanilla_world_name = copy_cstr(vanilla_world_name_ptr);
    let vanilla_distinct = (0..4).all(|a| (a + 1..4).all(|b| vanilla_name_ptrs[a] != vanilla_name_ptrs[b]));

    unsafe {
        // Reimpl pole second; its own parse is still live at compare time.
        (*reimpl_ptr).init(crowd_ambients, world_ambients, crowd_config, world_config);
        let reimpl_bytes = std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size);

        // fade_step_in (+0x09): must still be raw filler on both sides.
        if vanilla_snap[0x09] != 0xAA {
            mismatches.push("fade_step_in (+0x09) written on the vanilla side (expected untouched 0xAA filler)".to_string());
        }
        if reimpl_bytes[0x09] != 0xAA {
            mismatches.push("fade_step_in (+0x09) written on the reimpl side (expected untouched 0xAA filler)".to_string());
        }

        // Whole-block byte diff, minus the masked regions - crowd_filename, world_snd.inner,
        // world_name, world_atten, both Ambients pointers - each re-covered by a replacement
        // check below.
        for i in 0..struct_size {
            if matches!(i, 0x1c..=0x2b | 0x40..=0x53) || vanilla_snap[i] == reimpl_bytes[i] {
                continue;
            }
            mismatches.push(format!("byte +{i:#04x}: vanilla={:#04x}, reimpl={:#04x}", vanilla_snap[i], reimpl_bytes[i]));
        }

        // crowd_filename: per-slot content compare of snapshot vs. the live reimpl pointers.
        for (slot, vanilla_name) in vanilla_crowd_names.iter().enumerate() {
            let reimpl_ptr_val = dword(reimpl_bytes, 0x1c + slot * 4);
            match (vanilla_name.as_deref(), copy_cstr(reimpl_ptr_val).as_deref()) {
                (Some(v), Some(r)) if v == r => {}
                (v, r) => mismatches.push(format!(
                    "crowd_filename[{slot}] content: vanilla={:?}, reimpl={:?}",
                    v.map(|b| String::from_utf8_lossy(b).into_owned()),
                    r.map(|b| String::from_utf8_lossy(b).into_owned()),
                )),
            }
        }

        // world_snd.inner: null-ness parity only (per-attempt resource object - see doc).
        let (vanilla_inner, reimpl_inner) = (dword(&vanilla_snap, 0x40), dword(reimpl_bytes, 0x40));
        if (vanilla_inner != 0) != (reimpl_inner != 0) {
            mismatches.push(format!("world_snd.inner null-ness: vanilla={vanilla_inner:#010x}, reimpl={reimpl_inner:#010x}"));
        }

        // world_name: null-ness parity, then content compare when configured.
        let reimpl_world_name_ptr = dword(reimpl_bytes, 0x44);
        if (vanilla_world_name_ptr != 0) != (reimpl_world_name_ptr != 0) {
            mismatches.push(format!("world_name null-ness: vanilla={vanilla_world_name_ptr:#010x}, reimpl={reimpl_world_name_ptr:#010x}"));
        } else if let (Some(v), Some(r)) = (vanilla_world_name.as_deref(), copy_cstr(reimpl_world_name_ptr).as_deref())
            && v != r
        {
            mismatches.push(format!(
                "world_name content: vanilla={:?}, reimpl={:?}",
                String::from_utf8_lossy(v),
                String::from_utf8_lossy(r),
            ));
        }

        // world_atten: only comparable when a world sound was actually configured.
        if vanilla_world_name_ptr != 0 {
            let (v, r) = (dword(&vanilla_snap, 0x48), dword(reimpl_bytes, 0x48));
            if v != r {
                mismatches.push(format!("world_atten: vanilla={v:#010x}, reimpl={r:#010x}"));
            }
        }

        // Both Ambients pointers: null-ness parity only.
        for (name, off) in [("crowd_ambients", 0x4c), ("world_ambients", 0x50)] {
            let (v, r) = (dword(&vanilla_snap, off), dword(reimpl_bytes, off));
            if (v != 0) != (r != 0) {
                mismatches.push(format!("{name} null-ness: vanilla={v:#010x}, reimpl={r:#010x}"));
            }
        }
    }

    if !mismatches.is_empty() {
        error!("{}: mismatch(es): {:?}", test_name, mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
        }
    } else {
        write_success_line(failure_log, &format!("{} (vanilla crowd_filename pointers pairwise distinct: {})", test_name, vanilla_distinct));
    }

    soundscape_live_support::destroy_standalone_after_init(real_ptr);
    soundscape_live_support::destroy_standalone_after_init(reimpl_ptr);
    !mismatches.is_empty()
}

/// `ZTSOUNDSCAPE_UPDATE` - runs `ZTSoundscape::update` on **three** standalone twins - A = real
/// vanilla (via the `real_update` trampoline), B = the Rust port, C = vanilla again (determinism
/// control, so "port diverged" is distinguishable from "environment nondeterministic") - and
/// compares. Needs the live zoo (scenario-registry config names + real crowd `.wav`s); runs in
/// `live_zoo_tests()` right after [`run_ztsoundscape_init_test`].
///
/// Twins are built exactly as `ZTSOUNDSCAPE_INIT` builds its two: `allocate_uninitialized` ->
/// `0xAA` fill -> ctor (vanilla A/C / Rust B) -> `init` with the same four captured
/// `GLOBAL_ZTScenarioMgr` getter strings.
///
/// **Pre-equalization** (needed because each pole's `init` re-parses the two shared *global*
/// `BFConfigFile` instances, so pointer-valued fields legitimately drift - the same discovery
/// `ZTSOUNDSCAPE_INIT` documented; `update` never re-reads configs): `crowd_filename[4]`,
/// `crowd_atten[4]`, `world_name`, and `world_atten` are copied from B (the freshest live parse)
/// into A and C, and `fade_step_in` (+0x09, deliberately-uninitialized filler) is written `0` on
/// all three - only ever read while `fading`, but this makes +0x09 byte-comparable once the start
/// block (which rewrites it identically from the equal `next_slot_is_b = 0`) has run. Each side's
/// own `world_snd.inner` is left alone (teardown still releases its own handle).
///
/// **Guest-count override**: the hysteresis holds a track forever at a constant guest count, so
/// the phase script (mid-fade tick, then a clamp-to-endpoint tick with a same-tick
/// restart, then an endpoint tick that really stops the playing slot and restarts again) is only
/// reachable in a guest band where every phase's selection lands on a *new* target: `>= 161`
/// (`-1 -> 1` on the start tick, `1 -> 2` on phase 2's fall-through restart, `2 -> 3` on phase
/// 3's). The dword at `ZTGameMgr+0x54` is therefore forced to 200 for the test (vanilla reads a
/// full dword there) and restored afterwards; the live value is recorded in the success line.
///
/// **RNG discipline** (update jitters both `Ambients` blocks through the shared global game RNG):
/// the state at VA `0x00638060` is snapshotted before phase 1; A runs the phase's ticks, the RNG
/// is rewound, B runs the same ticks, rewound again, C runs them; the snapshot is restored at test
/// end (no net stream shift for the vanilla consumers). The rewinds preserve that stream
/// discipline but do not make `Ambients` positions comparable across poles - see below. Phases
/// 2-3 run A then B without rewinds.
///
/// **Why `Ambients` positions aren't compared across poles**: rewinding the RNG between poles
/// doesn't make positions deterministic - the real sound subsystem's asynchronous response to
/// `Ambients::play` (itself a vanilla shared-RNG consumer) draws the global state from its own
/// thread, so the first pole runs from the clean snapshot while every later pole runs from a
/// state its rewind can't undo. A port jitter bug reads A == C != B, so the vanilla A-vs-C
/// determinism control can't isolate one either. The compare is therefore struct-only (the state
/// machine never holds jitter values) plus a per-pole "ambients positions changed" sanity assert;
/// the jitter math itself is pinned by the hand-computed seed vectors in `ztsoundscape.rs`'s unit
/// tests.
///
/// Phase script (both fade ticks land mid-script by construction - the start tick sets
/// `fade = 10000` with `fading = 1` and no fade block):
/// - **Phase 1** - two `delta = 1000` ticks per pole: tick 1 starts the crowd loop on slot A
///   (`fading = 1`, `fade = 10000`, `fade_step_in = 0`), tick 2 steps the ramp to 9000 (real
///   `SET_FADE_ATTENUATION`/`SET_VOLUME` on the live slot A at a mid-ramp value). Compare: masked
///   struct A vs B and A vs C, plus the per-pole "ambients positions changed" sanity.
/// - **Phase 2** - one `delta = 10000` tick on A then B: fade 9000 -> wraps past 0 -> clamps to 0
///   -> endpoint stop gated off (slot B never started, `VALID` false) -> `fading` cleared ->
///   fall-through restart on slot B (`fade_step_in = 1`, `fade = 0`). Covers advance+clamp, the
///   endpoint logic's gated-off arm, and the same-tick restart.
/// - **Phase 3** - one `delta = 10000` tick on A then B: fade 0 -> 10000 -> endpoint **really
///   stops the playing slot A** (`VALID` true -> `STOP` + `RELEASE`) and restarts on slot A
///   (target 3, `next_slot_is_b` back at 0).
///
/// Masked compare regions (same rationale as `ZTSOUNDSCAPE_INIT`): per-attempt vanilla-owned
/// inner handles - `+0x10..=0x13`, `+0x18..=0x1b` (crowd slots, both firing attempts from phase 1
/// on) and `+0x40..=0x43` (world) - null-ness parity only, plus `+0x4c..=0x53` (each twin's own
/// `Ambients*`, real per-twin heap addresses - null-ness only; they can never be byte-equal across
/// twins). Everything else is byte-compared, including `+0x09`.
///
/// Pole note: both vanilla poles go through `soundscape_live_support::real_update` (an
/// `UPDATE_DETOUR.call` trampoline) - a release build's raw-cast `.original()` would re-enter the
/// Rust detour and degenerate them into Rust-vs-Rust, taking the A/C determinism control down
/// with them.
///
/// Audible caveat: real crowd loops are started/stopped across the phases and several overlap
/// (A/B/C each loop one world + crowd sound until teardown) - documented, not a failure.
///
/// Teardown: `destroy_standalone_after_init` x3 (real vanilla destructor + `operator delete` -
/// see [`soundscape_live_support::destroy_standalone_after_init`]'s cross-allocator reasoning).
pub(crate) fn run_ztsoundscape_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSOUNDSCAPE_UPDATE";

    let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
    if scenariomgr_ptr == 0 {
        info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
        return false;
    }
    let gamemgr_ptr = globals().ztgamemgr_ptr();
    if gamemgr_ptr.is_null() {
        info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
        return false;
    }

    // Capture the four getter results once; all three poles get exactly these.
    let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
    let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
    let crowd_config = unsafe { GET_CROWD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };
    let world_config = unsafe { GET_WORLD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };

    let vanilla_a = soundscape_live_support::allocate_uninitialized();
    let reimpl_b = soundscape_live_support::allocate_uninitialized();
    let vanilla_c = soundscape_live_support::allocate_uninitialized();
    if vanilla_a.is_null() || reimpl_b.is_null() || vanilla_c.is_null() {
        error!(
            "{}: OPERATOR_NEW returned null (a={:?}, b={:?}, c={:?})",
            test_name, vanilla_a, reimpl_b, vanilla_c
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!(
                    "Test Failed {}: OPERATOR_NEW returned null (a={:?}, b={:?}, c={:?})\n",
                    test_name, vanilla_a, reimpl_b, vanilla_c
                )
                .as_bytes(),
            );
        }
        // Nothing was constructed or init'ed on this path, so a plain free is complete.
        for ptr in [vanilla_a, reimpl_b, vanilla_c] {
            if !ptr.is_null() {
                soundscape_live_support::destroy_standalone(ptr);
            }
        }
        return true;
    }

    let base = get_module_base("zoo.exe") as u32;
    let rng_addr = base + GAME_RNG_RVA;
    let guests_addr = gamemgr_ptr as u32 + 0x54;
    let live_guests: i32 = get_from_memory(guests_addr);
    const FORCED_GUESTS: i32 = 200; // >= 161: every phase's hysteresis selection lands on a new target

    let struct_size = size_of::<ZTSoundscape>();
    let dword = |bytes: &[u8], off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    let mut mismatches: Vec<String> = Vec::new();

    unsafe {
        // Build all three twins: 0xAA fill, ctor, init (vanilla A/C, Rust B). The generated
        // INIT's params 2/3 carry a `*const u32` wart - cast here, in the test, not inside the
        // port (see `ZTSoundscape::init`'s doc comment).
        for ptr in [vanilla_a, reimpl_b, vanilla_c] {
            std::ptr::write_bytes(ptr as *mut u8, 0xAA, struct_size);
        }
        soundscape_live_support::real_constructor(vanilla_a as *const c_void);
        soundscape_live_support::real_constructor(vanilla_c as *const c_void);
        (*reimpl_b).construct();
        for ptr in [vanilla_a, vanilla_c] {
            soundscape_live_support::real_init(
                ptr as *const c_void,
                crowd_ambients as *const u32,
                world_ambients as *const u32,
                crowd_config,
                world_config,
            );
        }
        (*reimpl_b).init(crowd_ambients, world_ambients, crowd_config, world_config);

        // Pre-equalization: B's freshest live parse into A and C (see this test's doc comment).
        for twin in [vanilla_a, vanilla_c] {
            for off in (0x1c..0x3c).step_by(4) {
                // crowd_filename[4] + crowd_atten[4]
                let v: u32 = get_from_memory(reimpl_b as u32 + off);
                save_to_memory(twin as u32 + off, v);
            }
            for off in [0x44, 0x48] {
                // world_name, world_atten
                let v: u32 = get_from_memory(reimpl_b as u32 + off);
                save_to_memory(twin as u32 + off, v);
            }
        }
        for twin in [vanilla_a, reimpl_b, vanilla_c] {
            save_to_memory(twin as u32 + 0x09, 0u8); // fade_step_in
        }
    }

    // Masked struct compare (see this test's doc comment for the masked regions). The Ambients
    // position triples are deliberately not compared across poles (see this test's doc comment);
    // their only live check is the "positions changed" sanity below.
    let compare = |label: &'static str,
                   a_ptr: *const ZTSoundscape,
                   b_ptr: *const ZTSoundscape,
                   mismatches: &mut Vec<String>| {
        let a = unsafe { std::slice::from_raw_parts(a_ptr as *const u8, struct_size) };
        let b = unsafe { std::slice::from_raw_parts(b_ptr as *const u8, struct_size) };
        for i in 0..struct_size {
            if matches!(i, 0x10..=0x13 | 0x18..=0x1b | 0x40..=0x43 | 0x4c..=0x53) || a[i] == b[i] {
                continue;
            }
            mismatches.push(format!("{label}: byte +{i:#04x}: {:#04x} vs {:#04x}", a[i], b[i]));
        }
        for (name, off) in [
            ("crowd_snd_a.inner", 0x10),
            ("crowd_snd_b.inner", 0x18),
            ("world_snd.inner", 0x40),
            ("crowd_ambients", 0x4c),
            ("world_ambients", 0x50),
        ] {
            let (x, y) = (dword(a, off), dword(b, off));
            if (x != 0) != (y != 0) {
                mismatches.push(format!("{label}: {name} null-ness: {x:#010x} vs {y:#010x}"));
            }
        }
    };

    // One pole's `(crowd, world)` Ambients position triples, read from its own blocks.
    let ambients_triple = |ptr: *const ZTSoundscape| -> [(i32, i32, i32); 2] {
        [0x4c, 0x50].map(|off| {
            let p = dword(unsafe { std::slice::from_raw_parts(ptr as *const u8, struct_size) }, off);
            if p == 0 {
                (0, 0, 0)
            } else {
                (
                    get_from_memory::<i32>(p + 0xc),
                    get_from_memory::<i32>(p + 0x10),
                    get_from_memory::<i32>(p + 0x14),
                )
            }
        })
    };

    // The guest-count override covers every update call below; both orders are safe because the
    // vanilla pole reads the same dword the Rust port does.
    save_to_memory(guests_addr, FORCED_GUESTS);

    // "Positions changed" sanity (the applied fallback - see doc comment): capture each pole's
    // pre-update triples to diff against after phase 1.
    let poles = [("A", vanilla_a), ("B", reimpl_b), ("C", vanilla_c)];
    let pre_positions: Vec<_> = poles.iter().map(|(_, p)| ambients_triple(*p)).collect();

    // Phase 1: two delta=1000 ticks per pole, RNG-rewound between poles (see doc comment).
    let s0: u32 = get_from_memory(rng_addr);
    // Pole A: real vanilla twice.
    soundscape_live_support::real_update(vanilla_a as *const c_void, 1000);
    soundscape_live_support::real_update(vanilla_a as *const c_void, 1000);
    save_to_memory(rng_addr, s0);
    unsafe {
        // Pole B: the Rust port twice.
        (*reimpl_b).update(1000);
        (*reimpl_b).update(1000);
    }
    save_to_memory(rng_addr, s0);
    // Pole C: real vanilla again (determinism control).
    soundscape_live_support::real_update(vanilla_c as *const c_void, 1000);
    soundscape_live_support::real_update(vanilla_c as *const c_void, 1000);
    compare("phase 1 A/B", vanilla_a, reimpl_b, &mut mismatches);
    compare("phase 1 A/C", vanilla_a, vanilla_c, &mut mismatches);

    // "Positions changed" sanity: each pole's jitter + write path must have moved at least one
    // Ambients block off its pre-update triple (a zero-jitter coincidence on one block is
    // possible but both blocks standing still means step 4 never ran on that pole).
    for ((name, ptr), pre) in poles.iter().zip(&pre_positions) {
        let post = ambients_triple(*ptr);
        if post == *pre {
            mismatches.push(format!(
                "phase 1: ambients positions did not change on pole {name}: {pre:?} -> {post:?}"
            ));
        }
    }

    // Phase 2: fade 9000 -> clamp 0 -> gated-off endpoint -> fall-through restart on slot B.
    soundscape_live_support::real_update(vanilla_a as *const c_void, 10000);
    unsafe {
        (*reimpl_b).update(10000);
    }
    compare("phase 2 A/B", vanilla_a, reimpl_b, &mut mismatches);

    // Phase 3: fade 0 -> 10000 -> endpoint really stops slot A -> fall-through restart on slot A.
    soundscape_live_support::real_update(vanilla_a as *const c_void, 10000);
    unsafe {
        (*reimpl_b).update(10000);
    }
    compare("phase 3 A/B", vanilla_a, reimpl_b, &mut mismatches);

    // Teardown: restore the shared global state first, then release all three twins through the
    // real vanilla destructor (stops every sound they started).
    save_to_memory(rng_addr, s0);
    save_to_memory(guests_addr, live_guests);
    soundscape_live_support::destroy_standalone_after_init(vanilla_a);
    soundscape_live_support::destroy_standalone_after_init(reimpl_b);
    soundscape_live_support::destroy_standalone_after_init(vanilla_c);

    if !mismatches.is_empty() {
        error!("{}: mismatch(es): {:?}", test_name, mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
        }
    } else {
        write_success_line(
            failure_log,
            &format!("{} (live guest count {}, forced to {} for the phase script)", test_name, live_guests, FORCED_GUESTS),
        );
    }
    !mismatches.is_empty()
}

/// `ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE` - `ZTSOUNDSCAPE_UPDATE`'s phase script always plays real,
/// present crowd
/// `.wav`s, so it never reaches `update`'s start block's `ATTEMPT`-fails branch. Per
/// `ZTSoundscape::update`'s doc comment, `current_track` updates to the selected target even when
/// the attempt to start that track's sound fails - only the `fading`/`fade_step_in`/`fade`/
/// `next_slot_is_b` crossfade-state-machine advance is gated on success. This test forces that
/// branch directly and pins both halves of that contract.
///
/// Builds two standalone twins (real vanilla / reimpl) exactly as `ZTSOUNDSCAPE_INIT`/`_UPDATE` do
/// (`allocate_uninitialized` -> `0xAA` fill -> ctor -> `init` with the same four captured
/// `GLOBAL_ZTScenarioMgr` getter strings), then - because each pole's `init` re-parses the shared
/// global `BFConfigFile` instances and so legitimately ends up with different pointer values in
/// `crowd_filename`/`crowd_atten`/`world_name`/`world_atten` (`ZTSOUNDSCAPE_INIT`'s documented
/// discovery) - pre-equalizes those fields from the reimpl pole into the vanilla pole exactly as
/// `ZTSOUNDSCAPE_UPDATE` does, so the later struct compare isn't comparing two independent parses.
///
/// Only then does it overwrite `crowd_filename[0]` on **both** twins with a filename engineered to
/// fail, and forces the live `GLOBAL_ZTGameMgr` guest count to `0`: with `current_track` fresh at
/// `-1` after `init`, `select_target_track(-1, 0)` deterministically picks target `0` (the
/// `g <= 14 && t != 0` arm), so the start block's `ATTEMPT` is guaranteed to run against the bogus
/// filename on both poles.
///
/// **The filename can't just be "nonexistent" - it has to fail `SNDSound::attempt`'s own gate.**
/// `SNDSound_attempt.asm` shows `attempt` never touches the filesystem or `DX8SndMgr` for a
/// same-vtable check first: it reads the filename's **last character** and short-circuits to
/// `false` with no allocation at all unless that character is `'v'`/`'V'` (a crude `.wav`-extension
/// sniff, `CMP %CL, 0x76` / `0x56` at `.1ece9e`/`.1ecebc`; `.asm:14`'s `MOV %CL, [ECX + EBX - 1]`
/// loads the string's *last* byte, so a name like `"...notawav"` still passes). And a `.wav` name
/// that passes the gate doesn't work either: attempting a missing file still reports success
/// (`fading = 1, fade = 10000, next_slot_is_b = 1`) - the deeper `DX8Sound`/`BFSndMgr` load
/// doesn't fail synchronously (presumably async/deferred). The filename below ends in `.txt`,
/// which fails deterministically at the string-shape gate alone - no dependency on any real
/// sound-loading behavior, and clear of the extension check.
///
/// One `update` tick runs on each pole, then: a masked struct compare (same per-attempt inner-handle
/// and `Ambients*` null-ness-only regions as `ZTSOUNDSCAPE_UPDATE` - no RNG rewind needed since
/// those are the only RNG-sensitive bytes in the compared struct, and they're masked) catches any
/// unexpected divergence, and four explicit assertions per pole pin the contract itself:
/// `current_track == 0` (updated despite the failed attempt), `fading == 0`, `fade == 0`, and
/// `next_slot_is_b == 0` (the crossfade state machine never armed).
///
/// Teardown via `destroy_standalone_after_init` on both, same cross-allocator reasoning as the
/// other `ZTSOUNDSCAPE_*` tests.
pub(crate) fn run_ztsoundscape_update_attempt_failure_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSOUNDSCAPE_UPDATE_ATTEMPT_FAILURE";

    let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
    if scenariomgr_ptr == 0 {
        info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
        return false;
    }
    let gamemgr_ptr = globals().ztgamemgr_ptr();
    if gamemgr_ptr.is_null() {
        info!("Skipping {}: GLOBAL_ZTGameMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTGameMgr not initialized)", test_name));
        return false;
    }

    let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
    let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
    let crowd_config = unsafe { GET_CROWD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };
    let world_config = unsafe { GET_WORLD_CONFIG_NAME.original()(scenariomgr_ptr as i32) };

    let vanilla_a = soundscape_live_support::allocate_uninitialized();
    let reimpl_b = soundscape_live_support::allocate_uninitialized();
    if vanilla_a.is_null() || reimpl_b.is_null() {
        error!("{}: OPERATOR_NEW returned null (a={:?}, b={:?})", test_name, vanilla_a, reimpl_b);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("Test Failed {}: OPERATOR_NEW returned null (a={:?}, b={:?})\n", test_name, vanilla_a, reimpl_b).as_bytes(),
            );
        }
        for ptr in [vanilla_a, reimpl_b] {
            if !ptr.is_null() {
                soundscape_live_support::destroy_standalone(ptr);
            }
        }
        return true;
    }

    let struct_size = size_of::<ZTSoundscape>();
    let guests_addr = gamemgr_ptr as u32 + 0x54;
    let live_guests: i32 = get_from_memory(guests_addr);
    const FORCED_GUESTS: i32 = 0; // <= 14: select_target_track(-1, 0) deterministically picks target 0
    // Last byte before the nul is deliberately NOT 'v'/'V' - SNDSound::attempt (SNDSound_attempt.asm)
    // reads only that byte and short-circuits to false with no allocation and no BFSndMgr/DX8Sound
    // call at all otherwise (see this test's doc comment for why a "*.wav" name doesn't work here).
    // ".txt" ends in 't', clear of the gate.
    const BOGUS_FILENAME: &[u8] = b"__openzt_test_forced_attempt_failure.txt\0";

    let mut mismatches: Vec<String> = Vec::new();

    unsafe {
        std::ptr::write_bytes(vanilla_a as *mut u8, 0xAA, struct_size);
        std::ptr::write_bytes(reimpl_b as *mut u8, 0xAA, struct_size);

        soundscape_live_support::real_constructor(vanilla_a as *const c_void);
        (*reimpl_b).construct();

        // Generated INIT's params 2/3 carry a `*const u32` wart - cast here, in the test, not
        // inside the port (see `ZTSoundscape::init`'s doc comment).
        soundscape_live_support::real_init(
            vanilla_a as *const c_void,
            crowd_ambients as *const u32,
            world_ambients as *const u32,
            crowd_config,
            world_config,
        );
        (*reimpl_b).init(crowd_ambients, world_ambients, crowd_config, world_config);

        // Pre-equalization: B's freshest live parse into A, same shape as ZTSOUNDSCAPE_UPDATE's
        // (both poles' `init` re-parse the shared global config instances, so these pointer-valued
        // fields legitimately drift between independent parses - see that test's doc comment).
        for off in (0x1c..0x3c).step_by(4) {
            // crowd_filename[4] + crowd_atten[4]
            let v: u32 = get_from_memory(reimpl_b as u32 + off);
            save_to_memory(vanilla_a as u32 + off, v);
        }
        for off in [0x44, 0x48] {
            // world_name, world_atten
            let v: u32 = get_from_memory(reimpl_b as u32 + off);
            save_to_memory(vanilla_a as u32 + off, v);
        }

        // Overwrite crowd_filename[0] on both twins with a filename that cannot resolve, forcing
        // the update start block's ATTEMPT to fail on the branch this test targets. Applied after
        // pre-equalization so it isn't clobbered by the copy above.
        let bogus_ptr = BOGUS_FILENAME.as_ptr() as u32;
        save_to_memory(vanilla_a as u32 + 0x1c, bogus_ptr);
        save_to_memory(reimpl_b as u32 + 0x1c, bogus_ptr);
    }

    save_to_memory(guests_addr, FORCED_GUESTS);
    unsafe {
        soundscape_live_support::real_update(vanilla_a as *const c_void, 1000);
        (*reimpl_b).update(1000);
    }
    save_to_memory(guests_addr, live_guests);

    let dword = |bytes: &[u8], off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    unsafe {
        let a = std::slice::from_raw_parts(vanilla_a as *const u8, struct_size);
        let b = std::slice::from_raw_parts(reimpl_b as *const u8, struct_size);

        // Masked struct compare - same per-attempt inner-handle / Ambients-pointer null-ness-only
        // regions as ZTSOUNDSCAPE_UPDATE.
        for i in 0..struct_size {
            if matches!(i, 0x10..=0x13 | 0x18..=0x1b | 0x40..=0x43 | 0x4c..=0x53) || a[i] == b[i] {
                continue;
            }
            mismatches.push(format!("byte +{i:#04x}: vanilla={:#04x}, reimpl={:#04x}", a[i], b[i]));
        }
        for (name, off) in [
            ("crowd_snd_a.inner", 0x10),
            ("crowd_snd_b.inner", 0x18),
            ("world_snd.inner", 0x40),
            ("crowd_ambients", 0x4c),
            ("world_ambients", 0x50),
        ] {
            let (x, y) = (dword(a, off), dword(b, off));
            if (x != 0) != (y != 0) {
                mismatches.push(format!("{name} null-ness: vanilla={x:#010x}, reimpl={y:#010x}"));
            }
        }

        // The contract this test exists to pin: current_track updates to the selected target on
        // BOTH poles even though the attempt failed, while the crossfade state machine (fading/
        // fade/next_slot_is_b) must NOT have advanced from init's tail values.
        let current_track = |p: &[u8]| i32::from_le_bytes(p[0x0..0x4].try_into().unwrap());
        for (label, bytes) in [("vanilla", a), ("reimpl", b)] {
            if current_track(bytes) != 0 {
                mismatches.push(format!("{label}: current_track = {} (expected 0, the selected target)", current_track(bytes)));
            }
            if bytes[0xa] != 0 {
                mismatches.push(format!("{label}: fading = {} (expected 0 - the attempt-gated block must not have run)", bytes[0xa]));
            }
            if dword(bytes, 0x4) != 0 {
                mismatches.push(format!("{label}: fade = {} (expected 0, untouched from init's tail)", dword(bytes, 0x4) as i32));
            }
            if bytes[0x8] != 0 {
                mismatches.push(format!("{label}: next_slot_is_b = {} (expected 0, untouched from init's tail)", bytes[0x8]));
            }
        }
    }

    if !mismatches.is_empty() {
        error!("{}: mismatch(es): {:?}", test_name, mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
        }
    } else {
        write_success_line(failure_log, &format!("{} (live guest count {}, forced to {} for one tick)", test_name, live_guests, FORCED_GUESTS));
    }

    soundscape_live_support::destroy_standalone_after_init(vanilla_a);
    soundscape_live_support::destroy_standalone_after_init(reimpl_b);
    !mismatches.is_empty()
}
