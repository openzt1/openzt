//! Compares real vanilla `ZTShowMgr` against its Rust reimplementation (production file
//! `openzt/src/ztshowmgr.rs`): the standalone constructor round-trip, `initShowParams`,
//! register/unregister show/script and id-assignment store semantics, `getShowInfo`/
//! `getScriptId`/`getScript`, `enterNewMonth`, the `update` walk (via sentinel vtables),
//! save/load round-trips, `isDoingShow`/`isShowScriptDone`, and the live-zoo store/tree
//! consistency check. Fixture allocation/teardown helpers live in
//! `crate::ztshowmgr::live_support` (`showmgr_live_support`); most teardown here is leak-only -
//! see that module's doc comment for why.

use std::io::Write;
use std::mem::size_of;

use tracing::{error, info};

use openzt_detour::generated::bfapp::GET_INSTALLED_EXPANSION as BFAPP_GET_INSTALLED_EXPANSION;
use openzt_detour::generated::standalone;
use openzt_detour::generated::ztshowmgr::{
    CONSTRUCTOR as ZTSHOWMGR_CONSTRUCTOR, ENTER_NEW_MONTH as ZTSHOWMGR_ENTER_NEW_MONTH, GET_SCRIPT as ZTSHOWMGR_GET_SCRIPT,
    GET_SCRIPT_ID as ZTSHOWMGR_GET_SCRIPT_ID, GET_SHOW_INFO as ZTSHOWMGR_GET_SHOW_INFO, IS_DOING_SHOW as ZTSHOWMGR_IS_DOING_SHOW,
    IS_SHOW_SCRIPT_DONE as ZTSHOWMGR_IS_SHOW_SCRIPT_DONE, LOAD as ZTSHOWMGR_LOAD, REGISTER_SCRIPT as ZTSHOWMGR_REGISTER_SCRIPT,
    REGISTER_SHOW as ZTSHOWMGR_REGISTER_SHOW, SAVE as ZTSHOWMGR_SAVE, UNREGISTER_SCRIPT as ZTSHOWMGR_UNREGISTER_SCRIPT,
    UNREGISTER_SHOW as ZTSHOWMGR_UNREGISTER_SHOW, UPDATE as ZTSHOWMGR_UPDATE,
};
use openzt_detour::generated::ztshowscript::CONSTRUCTOR as ZTSHOWSCRIPT_CONSTRUCTOR;

use crate::globals::get_module_base;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, save_to_memory};
use crate::ztshow::live_support as ztshow_live_support;
use crate::ztshowmgr::{self, live_support as showmgr_live_support, ZTShowMgr};
use crate::ztshowscriptmgr;

use super::ztshow::find_real_show_tank_habitat;
use super::ztshowscriptmgr::make_registered_show_script;
/// `ZTSHOWMGR_STANDALONE_ROUNDTRIP` - `ztshowmgr-implementation-plan.md` stage 1: builds two fresh
/// `0x44`-byte standalone `ZTShowMgr` blocks, runs the real vanilla constructor on one and the Rust
/// reimplementation ([`ZTShowMgr::construct`]) on the other, then compares them field-by-field.
///
/// Not a whole-struct byte-diff, deliberately - three field groups *can't* legally compare equal
/// across two separately-constructed instances, each excluded for its own documented reason:
/// - `+0x8..+0x28` (the eight `initShowParams` thresholds): the real ctor's post-default values are
///   config-dependent (`BFConfigFile`/`shows.cfg`, gated on expansion pack 2) - stage 2 ports that
///   half. The Rust side's pre-config defaults are asserted exactly instead.
/// - `+0x28`/`+0x38` (both maps' `DAT_00638008`-freelist header pointers): per-instance
///   allocations, and `construct` deliberately leaves them null (see its doc comment). The real
///   side's nodes are instead *shape*-checked (color `0`, null parent, left/right self-referential
///   - the standard empty MSVC `_Tree` header), and the reimpl side's nullness is asserted.
/// - `+0x30`/`+0x40` (the two tag bytes): each instance writes the high byte of *its own* address
///   there, so each side is checked against its own address rather than against the other.
///
/// Everything else compares byte-identical: both vtables (`+0x0`/`+0x34`), `+0x4` and every padding
/// byte, and both map sizes. Also asserts the Rust registered-shows store stays empty (standalone
/// construction must never touch it).
///
/// Pole note: `ztshowmgr::CONSTRUCTOR` is deliberately never detoured (see `ztshowmgr.rs`'s module
/// doc comment), so `.original()` reaches real vanilla in every build profile. If that ever changes,
/// this test needs the `*_DETOUR.call` trampoline treatment instead - see
/// `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP`'s own doc comment for that exact failure mode. Since stage 2,
/// though, the real ctor's *internal* tail-call into `initShowParams` now lands in the stage-2 Rust
/// detour (the address itself is patched, in every profile) - harmless here because everything that
/// detour writes (the eight threshold fields) is excluded from the byte-compare groups below and
/// identical to what vanilla's own `initShowParams` would have written; noted so the exclusion isn't
/// mistaken for a vanilla-vs-vanilla guarantee. Teardown is
/// leak-only: the real-ctor side owns freelist nodes with no safe Rust-side return path (see
/// `showmgr_live_support::allocate_uninitialized`'s doc comment), so both buffers stay allocated for
/// the one-shot test process's lifetime.
pub(crate) fn run_ztshowmgr_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_STANDALONE_ROUNDTRIP";

    let real_ptr = showmgr_live_support::allocate_uninitialized();
    let reimpl_ptr = showmgr_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr).as_bytes(),
            );
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();
    let struct_size = size_of::<ZTShowMgr>();
    let real_addr = real_ptr as u32;
    let reimpl_addr = reimpl_ptr as u32;

    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, struct_size);

        ZTSHOWMGR_CONSTRUCTOR.original()(real_ptr as *const u32);
        (*reimpl_ptr).construct();

        // Byte-identical regions: everything except the three exclusion groups above.
        let real_bytes = std::slice::from_raw_parts(real_ptr as *const u8, struct_size);
        let reimpl_bytes = std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size);
        const COMPARED: [(usize, usize); 5] = [(0x00, 0x08), (0x2c, 0x30), (0x34, 0x38), (0x3c, 0x40), (0x41, 0x44)];
        for (start, end) in COMPARED {
            for i in start..end {
                if real_bytes[i] != reimpl_bytes[i] {
                    failures.push(format!("byte {:#04x}: real={:#04x} reimpl={:#04x}", i, real_bytes[i], reimpl_bytes[i]));
                }
            }
        }

        // Both tag bytes carry each instance's own high address byte.
        for (addr, name) in [(real_addr, "real"), (reimpl_addr, "reimpl")] {
            for offset in [0x30, 0x40] {
                let tag = get_from_memory::<u8>(addr + offset);
                if tag != (addr >> 24) as u8 {
                    failures.push(format!("{} +{:#x}: tag byte {:#04x}, expected {:#04x}", name, offset, tag, (addr >> 24) as u8));
                }
            }
        }

        // Real side: both maps get a real freelist node, shaped like a standard empty MSVC
        // `_Tree` header. Reimpl side: both stay null by design.
        for (offset, name) in [(0x28, "ZTShowMgr map"), (0x38, "embedded ZTShowScriptMgr map")] {
            let header = get_from_memory::<u32>(real_addr + offset);
            if header == 0 {
                failures.push(format!("real {name} header is null"));
            } else {
                let color = get_from_memory::<u8>(header);
                let parent = get_from_memory::<u32>(header + 4);
                let left = get_from_memory::<u32>(header + 8);
                let right = get_from_memory::<u32>(header + 0xc);
                if color != 0 || parent != 0 || left != header || right != header {
                    failures.push(format!(
                        "real {name} header {header:#x} is not a self-referential empty _Tree header (color={color}, parent={parent:#x}, left={left:#x}, right={right:#x})"
                    ));
                }
            }
            let reimpl_header = get_from_memory::<u32>(reimpl_addr + offset);
            if reimpl_header != 0 {
                failures.push(format!("reimpl {name} header should be null, got {reimpl_header:#x}"));
            }
        }
    }

    // The Rust side's pre-config threshold defaults (`initShowParams`'s own writes before its
    // expansion-gated `shows.cfg` override).
    const DEFAULT_THRESHOLDS: [(u32, u32); 8] =
        [(0x8, 0), (0xc, 3), (0x10, 6), (0x14, 0x19), (0x18, 0x32), (0x1c, 0x4b), (0x20, 6), (0x24, 6)];
    for (offset, expected) in DEFAULT_THRESHOLDS {
        let actual = get_from_memory::<u32>(reimpl_addr + offset);
        if actual != expected {
            failures.push(format!("reimpl +{offset:#x}: {actual}, expected default {expected}"));
        }
    }

    if ztshowmgr::registered_show_count() != 0 {
        failures.push(format!(
            "Rust registered-shows store should stay empty across standalone construction, has {} entries",
            ztshowmgr::registered_show_count()
        ));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWMGR_INIT_SHOW_PARAMS` - `ztshowmgr-implementation-plan.md` stage 2: builds two fresh
/// standalone `ZTShowMgr` blocks, runs the real vanilla `initShowParams` on one and the Rust
/// reimplementation ([`ZTShowMgr::init_show_params`]) on the other, then byte-compares the eight
/// config-loaded threshold fields at `+0x8..+0x28` (the only memory this function touches).
///
/// Environment-sensitive by design: both sides share the same expansion-2 gate (real
/// `BFApp::getInstalledExpansion` on the live `GLOBAL_ZTApp`) and the same real `shows.cfg`, so
/// the comparison is meaningful in either state - on a machine with expansion pack 2 installed
/// both sides must carry identical config-override values, otherwise both must carry identical
/// defaults. The gate state and both sides' final values are logged so a run can tell which path
/// it exercised. Vanilla-allocator side effects are balanced on both sides (each constructs and
/// releases its own stack-local `BFConfigFile`; the Rust port also returns the config's tree-root
/// node to the freelist it came from - see the method's doc comment).
///
/// Pole note: the vanilla side goes through `showmgr_live_support::call_real_init_show_params`
/// (the `INIT_SHOW_PARAMS_DETOUR.call` trampoline) - a release build's raw-cast `.original()`
/// would silently re-enter the Rust detour and degenerate the test into Rust-vs-Rust, the exact
/// mode `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP`'s doc comment describes. Teardown is leak-only (see
/// `showmgr_live_support::allocate_uninitialized`).
pub(crate) fn run_ztshowmgr_init_show_params_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_INIT_SHOW_PARAMS";

    let real_ptr = showmgr_live_support::allocate_uninitialized();
    let reimpl_ptr = showmgr_live_support::allocate_uninitialized();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        error!("{}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", test_name, real_ptr, reimpl_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(
                format!("Test Failed {}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})\n", test_name, real_ptr, reimpl_ptr)
                    .as_bytes(),
            );
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();

    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0, size_of::<ZTShowMgr>());
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0, size_of::<ZTShowMgr>());

        let real_return = showmgr_live_support::call_real_init_show_params(real_ptr as *const u32);
        let reimpl_return = (*reimpl_ptr).init_show_params();
        // The real body's only return write is `MOV AL, 0x1` - the upper 24 bits of EAX are
        // leftover register garbage (the `.c` decompile's own `CONCAT31(...,1)` return), so only
        // the low byte is compared.
        if real_return & 0xff != 1 || reimpl_return != 1 {
            failures.push(format!(
                "returns differ: real={:#x} (low byte {:#04x}) reimpl={reimpl_return}",
                real_return,
                real_return & 0xff
            ));
        }

        let real_bytes = std::slice::from_raw_parts(real_ptr as *const u8, size_of::<ZTShowMgr>());
        let reimpl_bytes = std::slice::from_raw_parts(reimpl_ptr as *const u8, size_of::<ZTShowMgr>());
        for i in 0x8..0x28 {
            if real_bytes[i] != reimpl_bytes[i] {
                failures.push(format!("byte {:#04x}: real={:#04x} reimpl={:#04x}", i, real_bytes[i], reimpl_bytes[i]));
            }
        }
    }

    // Log which path the environment exercised, and surface both sides' final values so a run
    // can tell config-override from defaults at a glance. `GLOBAL_ZTApp`'s RVA is
    // `ztshowmgr.rs`'s own private `GLOBAL_ZTAPP_RVA` - re-declared here per the repo's
    // no-shared-consts convention.
    let global_ztapp_rva: u32 = 0x00638154 - 0x400000;
    let ztapp_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + global_ztapp_rva);
    let expansion_2_installed = ztapp_ptr != 0 && unsafe { BFAPP_GET_INSTALLED_EXPANSION.original()(ztapp_ptr as *const u32, 2) };
    let threshold_names = [
        ("badTrick", 0x8),
        ("goodTrick", 0xc),
        ("greatTrick", 0x10),
        ("badShow", 0x14),
        ("goodShow", 0x18),
        ("greatShow", 0x1c),
        ("minIdealLength", 0x20),
        ("maxIdealLength", 0x24),
    ];
    let values: Vec<String> = threshold_names
        .iter()
        .map(|(name, offset)| {
            format!("{}={}", name, get_from_memory::<u32>(real_ptr as u32 + offset))
        })
        .collect();
    let gate_state = if expansion_2_installed {
        "OPEN (config override expected)"
    } else {
        "closed (defaults expected)"
    };

    if failures.is_empty() {
        write_success_line(failure_log, &format!("{} (expansion-2 gate {}, {})", test_name, gate_state, values.join(", ")));
        false
    } else {
        for msg in &failures {
            error!("{}: {}", test_name, msg);
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
        }
        true
    }
}

/// `ZTSHOWMGR_REGISTER_UNREGISTER_SHOW` - `ztshowmgr-implementation-plan.md` stages 3+9: drives
/// the full `registerShow`/`unregisterShow` ports (stage 9 dropped the stage-3 shadow/mirror
/// call-throughs, so the hooked addresses are pure Rust now) through every branch of both
/// vanilla bodies (`.asm`/`.c`-verified), pinning the store content after each op. The old
/// cross-store diff oracle - the real vanilla tree agreeing with the store after every hooked
/// mutation - is gone with the dual-write it verified: hooked writers stopped maintaining the
/// tree in stage 9, which this test now pins *positively* (the tree must stay empty under
/// hooked writes, through the `GET_SHOW_INFO` trampoline).
///
/// The op matrix covers: null-show register, preset-id register (the no-force reuse path -
/// counter untouched), the already-registered early return (with and without force, and never
/// consuming the counter), the id-0 fresh-id assignment (deterministic via a seeded counter:
/// the id is the post-increment counter masked to 16 bits, here exactly `0x0101`), the fresh-id
/// setter's embedded-`ZTShow` sync (`+0x6` id copy, `+0x10` back-pointer), force over an
/// unregistered preset id (fresh counter id, the preset value never entering the store), the
/// insert-or-assign **collision** (a force-assigned fresh id landing exactly on a registered
/// key overwrites that entry's value in place - the stolen show keeps its stale `field_0x70`
/// but is unreachable by id), path-A/B/C unregisters (both clear-flag states - `clear=true`
/// really executes the real `clearShowScriptStates` through path C, which targets the show
/// directly with no lookup), double unregister (absent-key silent success), the null+null
/// `AL=0` return, and the counter **wrap** semantics (`0xffff` is never assigned - counter
/// `0xfffe` assigns id `0`; a counter of `0xffff` wraps to `0` and also assigns id `0`; and a
/// `field_0x70 == 0` show whose store already holds key `0` early-returns even with force,
/// because vanilla's find runs before the fresh-id branch).
///
/// `clear=true` on a standalone `ZTShowInfo` needs one piece of setup: the embedded `ZTShow`'s
/// script-state map header at `show_info+0x38` (read unconditionally by the real
/// `ZTShowState::clear`). Real vanilla keeps it as a separate `0x18`-byte freelist node
/// (self-referential when empty), and a zeroed buffer would crash the first clear-flag op, so
/// each show gets a real, leak-only, empty-tree header node allocated there - the same shape
/// `ZTSHOWMGR_STANDALONE_ROUNDTRIP` verifies on the real constructor's own map headers. All
/// teardown is leak-only (`showmgr_live_support::allocate_uninitialized`'s doc comment); the
/// store is drained through the hooked unregister path and asserted empty, and the counter is
/// restored, since both are process-global state shared with the rest of the battery.
pub(crate) fn run_ztshowmgr_register_unregister_show_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_REGISTER_UNREGISTER_SHOW";

    if ztshowmgr::registered_show_count() != 0 {
        let msg = "Rust registered-shows store should be empty when the test starts";
        error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
        }
        return true;
    }

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }

    /// Pre-initializes `show_info`'s embedded `ZTShow` script-state map header (`show_info+0x38`)
    /// with a real, empty, self-referential `_Tree` header node - see the test's doc comment for
    /// why a zeroed buffer can't survive the clear-flag ops. Leak-only, like everything else
    /// this test allocates.
    fn init_script_state_header(show_info: u32) {
        let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
        unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
        save_to_memory(node + 0x8, node);
        save_to_memory(node + 0xc, node);
        save_to_memory(show_info + 0x38, node);
    }

    let mut failures: Vec<String> = Vec::new();
    let mgr_addr = mgr as u32;

    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    let show_a = ztshow_live_support::build_standalone_show_info();
    let show_b = ztshow_live_support::build_standalone_show_info();
    let show_c = ztshow_live_support::build_standalone_show_info();
    let show_d = ztshow_live_support::build_standalone_show_info();
    for show in [show_a, show_b, show_c, show_d] {
        init_script_state_header(show);
    }

    // Preset ids; show_b keeps its zero-init `field_0x70` (the counter-assignment case).
    const PRESET_ID_A: u16 = 0x1234;
    const PRESET_ID_C: u16 = 0x4321;
    const PRESET_ID_D: u16 = 0x9999;
    save_to_memory(show_a + 0x70, PRESET_ID_A);
    save_to_memory(show_c + 0x70, PRESET_ID_C);
    save_to_memory(show_d + 0x70, PRESET_ID_D);

    // All ops go through the hooked addresses (the raw function address - the detour itself,
    // installed by `reimplementation_tests::init`'s `crate::ztshowmgr::init()`), so what is
    // exercised is the promoted live path, not a test-side shortcut.
    let register =
        |show: u32, force: bool| -> bool { unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show as *const u32, force) } };
    let unregister =
        |id: u16, show: u32, clear: bool| -> bool { unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, id, show as *const u32, clear) } };

    // Stage 9 pin: the standalone vanilla tree stays inert under hooked writes. Nothing in this
    // test plants through the raw trampoline, so every id must read back empty through the real
    // `getShowInfo` walk even while the store holds registrations - if a writer call-through
    // ever came back, this is where it shows.
    let assert_tree_inert = |step: &str, ids: &[u16], failures: &mut Vec<String>| {
        for id in ids {
            if !showmgr_live_support::call_real_get_show_info(mgr_addr as *const u32, *id).is_null() {
                failures.push(format!("{step}: id {id:#06x} - the standalone vanilla tree should be inert (stage 9 dropped the writer call-throughs), but the real getShowInfo walk found an entry"));
            }
        }
    };

    // Restore point for the process-global counter; every block below seeds exact values, so
    // the pins hold regardless of what earlier battery stages left here.
    let counter_start = showmgr_live_support::show_id_counter();

    // Null-show register: vanilla's AL=0 early return, nothing written.
    if register(0, false) {
        failures.push("register(null, false) should return 0".to_string());
    }

    // Preset-id register - the no-force reuse path: the id is kept and the counter is left
    // untouched (the complementary pin to the force-fresh split below).
    let counter_before = showmgr_live_support::show_id_counter();
    if !register(show_a, false) {
        failures.push("register(A, false) with preset id should return 1".to_string());
    }
    if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a) {
        failures.push("store[A's preset id] should be show A after register(A, false)".to_string());
    }
    if showmgr_live_support::show_id_counter() != counter_before {
        failures.push("the no-force reuse path must leave the counter untouched".to_string());
    }

    // Already-registered early return - vanilla's find on the *current* field_0x70 hits,
    // nothing written, with or without force (the force flag is only read after the miss), and
    // the counter is never consumed.
    if register(show_a, false) {
        failures.push("re-register(A, false) should return 0 (already registered)".to_string());
    }
    if register(show_a, true) {
        failures.push("re-register(A, true) should return 0 (already registered; force must not reach the counter)".to_string());
    }
    if get_from_memory::<u16>(show_a + 0x70) != PRESET_ID_A {
        failures.push("A's field_0x70 should be untouched by the already-registered early returns".to_string());
    }
    if showmgr_live_support::show_id_counter() != counter_before {
        failures.push("the already-registered early returns must leave the counter untouched".to_string());
    }
    assert_tree_inert("after A's ops", &[PRESET_ID_A], &mut failures);

    // Id-0 fresh-id assignment, made deterministic by seeding the counter: the assigned id is
    // the post-increment counter, exactly 0x0101 here - no wrap ambiguity.
    showmgr_live_support::set_show_id_counter(0x0100);
    if !register(show_b, false) {
        failures.push("register(B, false) with id 0 should return 1".to_string());
    }
    let id_b = get_from_memory::<u16>(show_b + 0x70);
    if id_b != 0x0101 {
        failures.push(format!("register(B, false) from a seeded counter of 0x0100 should assign exactly 0x0101 (INC then 16-bit read), got {id_b:#06x}"));
    }
    if showmgr_live_support::show_id_counter() != 0x0101 {
        failures.push(format!(
            "the fresh-id path should have advanced the counter 0x0100 -> 0x0101, got {:#06x}",
            showmgr_live_support::show_id_counter()
        ));
    }
    if ztshowmgr::registered_show_for_id(id_b) != Some(show_b) {
        failures.push(format!("store[assigned id {id_b:#06x}] should be show B after register(B, false)"));
    }
    // The fresh id went through the ported `ZTShowInfo::setShowInfoID`: the embedded `ZTShow`'s
    // `+0x6` id copy and `+0x10` back-pointer (was zero) must be in sync, not just `field_0x70`.
    if get_from_memory::<u16>(show_b + 0x4 + 0x6) != id_b {
        failures.push("the embedded ZTShow's +0x6 id copy should carry the fresh id".to_string());
    }
    if get_from_memory::<u32>(show_b + 0x4 + 0x10) != show_b {
        failures.push("the embedded ZTShow's +0x10 back-pointer should point at the show".to_string());
    }

    // Force over an unregistered preset id: a fresh counter id is assigned even though C's
    // field_0x70 was non-zero, and the preset value itself never enters the store.
    if !register(show_c, true) {
        failures.push("register(C, true) should return 1".to_string());
    }
    let id_c = get_from_memory::<u16>(show_c + 0x70);
    if id_c != 0x0102 {
        failures.push(format!("register(C, true) should have force-assigned exactly the next counter id 0x0102, got {id_c:#06x}"));
    }
    if ztshowmgr::registered_show_for_id(id_c) != Some(show_c) {
        failures.push(format!("store[force-assigned id {id_c:#06x}] should be show C after register(C, true)"));
    }
    if ztshowmgr::registered_show_for_id(PRESET_ID_C).is_some() {
        failures.push("store should not hold C's preset id - force reassignment never inserts it".to_string());
    }
    assert_tree_inert("after B+C registers", &[PRESET_ID_A, id_b, id_c, 0x0000, 0xffff, 0x7fff, 0x8000], &mut failures);

    // Insert-or-assign collision: seed the counter so D's force-assigned fresh id lands exactly
    // on A's registered key. Vanilla's tree write overwrites the existing entry's value in
    // place - D steals A's slot; A keeps its stale field_0x70 but is unreachable by id.
    showmgr_live_support::set_show_id_counter(PRESET_ID_A - 1);
    if !register(show_d, true) {
        failures.push("register(D, true) should return 1".to_string());
    }
    if get_from_memory::<u16>(show_d + 0x70) != PRESET_ID_A {
        failures.push(format!(
            "D's force-assigned id should be exactly {PRESET_ID_A:#06x} (the seeded collision), got {:#06x}",
            get_from_memory::<u16>(show_d + 0x70)
        ));
    }
    if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_d) {
        failures.push("store[A's id] should now be show D - the collision overwrites the entry's value in place".to_string());
    }
    if get_from_memory::<u16>(show_a + 0x70) != PRESET_ID_A {
        failures.push("stolen show A must keep its stale field_0x70 (vanilla never repairs it)".to_string());
    }
    if ztshowmgr::registered_show_for_id(PRESET_ID_D).is_some() {
        failures.push("store should not hold D's preset id - the force-fresh path replaced it before the insert".to_string());
    }
    if showmgr_live_support::show_id_counter() != PRESET_ID_A {
        failures.push("the collision register should have consumed exactly one counter step".to_string());
    }

    // Path A (show == null, id != 0): erase by id alone, using B's id read back above.
    if !unregister(id_b, 0, false) {
        failures.push(format!("unregister({id_b:#06x}, null, false) should return 1"));
    }

    // Path B (show != null, id != 0), both clear-flag states. (The clear target here is picked
    // off the store, where each op's id has already been removed by its preceding unregister -
    // so neither flag state reaches a real clearShowScriptStates; that stays exercised through
    // path C below.)
    if !unregister(PRESET_ID_A, show_a, false) {
        failures.push("unregister(A's id, A, false) should return 1".to_string());
    }
    if !unregister(PRESET_ID_A, show_a, true) {
        failures.push("unregister(A's id, A, true) should return 1 (absent-key erase is still success)".to_string());
    }

    // Absent-key id unregister: silent no-op success.
    const ABSENT_PROBE_ID: u16 = 0x0bb7;
    if !unregister(ABSENT_PROBE_ID, 0, false) {
        failures.push(format!("unregister(absent id {ABSENT_PROBE_ID:#06x}, null, false) should return 1 (silent no-op)"));
    }

    // Path C (show != null, id == 0): the id is derived from the show's own field_0x70 -
    // deliberately stale after a prior unregister, since vanilla never zeroes that field.
    if !unregister(0, show_a, false) {
        failures.push("unregister(0, A, false) should return 1".to_string());
    }

    // Re-register A: with field_0x70 still carrying the stale preset id and that id no longer
    // in the store, the preset path re-registers under the very same id.
    if !register(show_a, false) {
        failures.push("re-register(A, false) after unregister should return 1 (stale field_0x70 is reusable)".to_string());
    }
    if get_from_memory::<u16>(show_a + 0x70) != PRESET_ID_A {
        failures.push("A's field_0x70 should still carry the stale preset id after unregister + re-register".to_string());
    }
    if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a) {
        failures.push("store[A's preset id] should be show A again after the re-register".to_string());
    }

    // clear=true through path C: the one op that really executes the real
    // `clearShowScriptStates` (targeted directly at the show, no lookup), running it over the
    // pre-initialized empty header node at show+0x38.
    if !unregister(0, show_b, true) {
        failures.push("unregister(0, B, true) should return 1".to_string());
    }

    // Double unregister: absent-key silent success again, store unchanged.
    if !unregister(0, show_b, false) {
        failures.push("double unregister(0, B, false) should still return 1 (silent no-op)".to_string());
    }

    // Null show + null id: vanilla's AL=0 early return.
    if unregister(0, 0, false) {
        failures.push("unregister(0, null, false) should return 0".to_string());
    }
    assert_tree_inert("after the unregister matrix", &[PRESET_ID_A, id_b, id_c, ABSENT_PROBE_ID], &mut failures);

    // Cleanup of the main matrix: drain through the hooked unregister (path C derives each
    // show's own stale field_0x70, which is exactly the key each insert used).
    for show in [show_a, show_b, show_c, show_d] {
        unregister(0, show, false);
    }

    // Counter wrap semantics, both directions of the boundary:
    // - from 0xfffe the increment lands on 0xffff, and 0xffff % 0xffff == 0 - so id 0xffff is
    //   never assigned; the show registers under key 0 with field_0x70 left 0;
    // - the next id-0 register then finds key 0 already held by the *current* field_0x70 value
    //   (0) and early-returns - even with force, which vanilla only reads after that find;
    // - after key 0 drains, a counter of 0xffff itself increments (word wrap) to 0 and again
    //   assigns id 0.
    let show_x = ztshow_live_support::build_standalone_show_info();
    let show_y = ztshow_live_support::build_standalone_show_info();
    showmgr_live_support::set_show_id_counter(0xfffe);
    if !register(show_x, false) {
        failures.push("wrap: register(X, false) at counter 0xfffe should return 1".to_string());
    }
    if get_from_memory::<u16>(show_x + 0x70) != 0 || ztshowmgr::registered_show_for_id(0) != Some(show_x) {
        failures.push("wrap: counter 0xfffe must assign id 0 (0xffff is never assigned) - field_0x70 and store[0] should both say so".to_string());
    }
    if showmgr_live_support::show_id_counter() != 0xffff {
        failures.push(format!(
            "wrap: the counter should now sit at 0xffff, got {:#06x}",
            showmgr_live_support::show_id_counter()
        ));
    }
    if register(show_y, false) || register(show_y, true) {
        failures.push("wrap: an id-0 register while key 0 is held must early-return 0, with or without force".to_string());
    }
    if showmgr_live_support::show_id_counter() != 0xffff {
        failures.push("wrap: the early returns must not consume the counter".to_string());
    }
    if !unregister(0, show_x, false) {
        failures.push("wrap: unregister(0, X) should return 1".to_string());
    }
    showmgr_live_support::set_show_id_counter(0xffff);
    if !register(show_y, false) {
        failures.push("wrap: register(Y, false) at counter 0xffff should return 1".to_string());
    }
    if showmgr_live_support::show_id_counter() != 0 {
        failures.push(format!(
            "wrap: the counter should have word-wrapped 0xffff -> 0, got {:#06x}",
            showmgr_live_support::show_id_counter()
        ));
    }
    if get_from_memory::<u16>(show_y + 0x70) != 0 || ztshowmgr::registered_show_for_id(0) != Some(show_y) {
        failures.push("wrap: the wrapped counter must assign id 0 - field_0x70 and store[0] should both say so".to_string());
    }
    if !unregister(0, show_y, false) {
        failures.push("wrap: unregister(0, Y) should return 1".to_string());
    }
    showmgr_live_support::set_show_id_counter(counter_start);

    // Hygiene: the process-global store must be empty - nothing may leak into the rest of the
    // battery.
    let remaining = ztshowmgr::registered_show_count();
    if remaining != 0 {
        failures.push(format!("store should be empty after cleanup, has {} entries", remaining));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID` - `ztshowmgr-implementation-plan.md` stage 4 (the
/// read cutover): drives the store-backed `getShowInfo`/`getScriptID` detours against a
/// standalone, real-constructor `ZTShowMgr` (`this` stand-in - the readers ignore it) and two
/// standalone `ZTShowInfo`s registered through the hooked `REGISTER_SHOW`. Since stage 9 the
/// hooked writers maintain only the store, so that is the only copy that populates.
///
/// Poles per probe:
/// - the hooked addresses (the promoted live path): `GET_SHOW_INFO.hooked()` must round-trip
///   each registered show's pointer and return `0` for every absent/boundary id;
///   `GET_SCRIPT_ID.hooked()` must return the found show's `+0x8` assigned-script-id u16
///   zero-extended (a high-bit-set value pins the zero-extension - real vanilla leaves EAX's
///   upper half as register garbage there, which no caller observes), return `0` for a *found*
///   show whose `+0x8` is `0` (cross-checked against `GET_SHOW_INFO` still finding it - the
///   found-but-zero vs. miss ambiguity vanilla itself has), and `0` for a miss;
/// - the real vanilla `getScriptID` through its own trampoline - half-real by construction: its
///   body reaches `getShowInfo` by raw address (`ZTShowMgr_getScriptID.asm`'s
///   `CALL ZTShowMgr::getShowInfo`), which is the detoured, store-backed reader, so it exercises
///   the real ABI glue and the real `+0x8` read on top of the store's answer. Compared through a
///   16-bit mask (see `cross_check_poles` - the real found path leaves the show-info pointer's
///   high bits in EAX's upper half, which the port's clean zero-extension contract doesn't
///   reproduce). (The sibling real-`getShowInfo` tree-walk pole this test carried during the
///   dual-write phase is gone with it: since stage 9 stopped the writers maintaining the tree,
///   that walk answers only raw-planted entries and can no longer agree with hooked
///   registrations - `ZTSHOWMGR_REGISTER_UNREGISTER_SHOW` now pins the tree's inertness
///   instead.)
///
/// Also pins the cutover's one deliberate benign divergence: vanilla's `getShowInfo` faults on a
/// null `this` (unguarded `[ECX+0x28]` read); the detour never touches `this`, so a
/// null-manager lookup returns the store's answer instead.
///
/// Teardown is leak-only (see `showmgr_live_support::allocate_uninitialized`'s doc comment); the
/// store is drained through the hooked unregister path and asserted empty.
pub(crate) fn run_ztshowmgr_get_show_info_get_script_id_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID";

    if ztshowmgr::registered_show_count() != 0 {
        let msg = "Rust registered-shows store should be empty when the test starts";
        error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
        }
        return true;
    }

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }

    /// Cross-pole agreement for every id the test has touched: the hooked reader must answer
    /// exactly what the store holds, and the real `getScriptID` must agree with the hooked one
    /// - through a 16-bit mask, because the real body's found path (`MOV %AX, word ptr
    ///   [EAX+0x8]`, no `movzx`) leaves the upper EAX holding the upper half of the `getShowInfo`
    ///   return (the show-info pointer's high bits), which the port's clean zero-extension
    ///   contract deliberately does not reproduce.
    fn cross_check_poles(mgr_addr: u32, step: &str, touched_ids: &[u16], failures: &mut Vec<String>) {
        for id in touched_ids {
            let store = ztshowmgr::registered_show_for_id(*id).unwrap_or(0);
            let hooked = unsafe { ZTSHOWMGR_GET_SHOW_INFO.hooked()(mgr_addr as *const u32, *id) };
            if hooked as u32 != store {
                failures.push(format!("{step}: id {id:#06x} - hooked={:#010x}, store={store:#010x}", hooked as u32));
            }
            let hooked_script = unsafe { ZTSHOWMGR_GET_SCRIPT_ID.hooked()(mgr_addr as *const u32, *id) };
            let real_script = showmgr_live_support::call_real_get_script_id(mgr_addr as *const u32, *id);
            if hooked_script != real_script {
                failures.push(format!(
                    "{step}: id {id:#06x} - getScriptId hooked={hooked_script:#010x}, real(trampoline)={real_script:#010x}"
                ));
            }
        }
    }

    let mut failures: Vec<String> = Vec::new();
    let mgr_addr = mgr as u32;

    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    let show_a = ztshow_live_support::build_standalone_show_info();
    let show_b = ztshow_live_support::build_standalone_show_info();

    // Preset id; show_b keeps its zero-init `field_0x70` (the counter-assignment case).
    const PRESET_ID_A: u16 = 0x1234;
    // High bit set on purpose: proves the `+0x8` u16 comes back zero-extended, not
    // sign-extended (vanilla's `MOV %AX` would leave the upper EAX as garbage).
    const SCRIPT_ID_A: u16 = 0x8abc;
    save_to_memory(show_a + 0x70, PRESET_ID_A);
    save_to_memory(show_a + 0x8, SCRIPT_ID_A);
    // show_b keeps its zero-init `+0x8` (the found-but-zero case).

    // All reads go through the hooked addresses (the promoted live path); the real-side poles
    // go through the stage-4 trampolines.
    let get_show_info = |id: u16| -> u32 { (unsafe { ZTSHOWMGR_GET_SHOW_INFO.hooked()(mgr as *const u32, id) }) as u32 };
    let get_script_id = |id: u16| -> u16 { unsafe { ZTSHOWMGR_GET_SCRIPT_ID.hooked()(mgr as *const u32, id) } };

    // Everything both poles and the store must agree on, seeded with the boundary probes and
    // A's preset id; B's fresh id gets pushed as the test discovers it.
    let mut touched_ids: Vec<u16> = vec![0x0000, 0xffff, 0x7fff, 0x8000, PRESET_ID_A];

    // Before any registration every pole reads empty.
    cross_check_poles(mgr_addr, "before any registration", &touched_ids, &mut failures);

    // Null-manager read: pins the cutover's one deliberate benign divergence - vanilla's own
    // body faults here (unguarded `[ECX+0x28]`); the detour ignores `this` and answers from the
    // store, which is empty for this id.
    if !unsafe { ZTSHOWMGR_GET_SHOW_INFO.hooked()(std::ptr::null(), PRESET_ID_A) }.is_null() {
        failures.push("hooked getShowInfo(null mgr, absent id) should return 0".to_string());
    }

    // Register A (preset id) and B (id-0 counter assignment) through the hooked REGISTER_SHOW.
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } {
        failures.push("register(A, false) with preset id should return 1".to_string());
    }
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } {
        failures.push("register(B, false) with id 0 should return 1".to_string());
    }
    let id_b = get_from_memory::<u16>(show_b + 0x70);
    if id_b == 0 || id_b == PRESET_ID_A {
        failures.push(format!("register(B, false) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
    }
    touched_ids.push(id_b);
    if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a) {
        failures.push("store[A's preset id] should be show A after register(A)".to_string());
    }
    if ztshowmgr::registered_show_for_id(id_b) != Some(show_b) {
        failures.push(format!("store[assigned id {id_b:#06x}] should be show B after register(B)"));
    }
    cross_check_poles(mgr_addr, "after register(A)+register(B)", &touched_ids, &mut failures);

    // Hooked reads: the cutover path round-trips each registration; the boundary probes stay
    // absent.
    if get_show_info(PRESET_ID_A) != show_a {
        failures.push(format!("hooked getShowInfo(A's id) should be {show_a:#010x}, got {:#010x}", get_show_info(PRESET_ID_A)));
    }
    if get_show_info(id_b) != show_b {
        failures.push(format!("hooked getShowInfo(B's id) should be {show_b:#010x}, got {:#010x}", get_show_info(id_b)));
    }
    for id in [0x0000u16, 0xffff, 0x7fff, 0x8000] {
        if get_show_info(id) != 0 {
            failures.push(format!("hooked getShowInfo(absent boundary id {id:#06x}) should return 0"));
        }
    }

    // Hooked getScriptID: A's +0x8 script id, zero-extended; B's is the found-but-zero case.
    if get_script_id(PRESET_ID_A) != SCRIPT_ID_A {
        failures.push(format!(
            "hooked getScriptId(A's id) should be {SCRIPT_ID_A:#06x} (zero-extended), got {:#010x}",
            get_script_id(PRESET_ID_A)
        ));
    }
    if get_script_id(id_b) != 0 {
        failures.push(format!("hooked getScriptId(B's id) should be 0 (B's +0x8 is zero), got {:#010x}", get_script_id(id_b)));
    }
    if get_show_info(id_b) != show_b {
        failures.push("hooked getShowInfo(B's id) should still find B - the zero script id must not read as a miss".to_string());
    }
    if get_script_id(0xffff) != 0 {
        failures.push("hooked getScriptId(absent id) should return 0".to_string());
    }

    // Cleanup: drain through the hooked unregister path (path C derives each show's own stale
    // `field_0x70`, which is exactly the key each insert used), then assert both the store and
    // the standalone vanilla tree drained.
    for show in [show_a, show_b] {
        unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
    }
    cross_check_poles(mgr_addr, "after cleanup", &touched_ids, &mut failures);
    let remaining = ztshowmgr::registered_show_count();
    if remaining != 0 {
        failures.push(format!("store should be empty after cleanup, has {} entries", remaining));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZTSHOWMGR_ENTER_NEW_MONTH` - `ztshowmgr-implementation-plan.md` stage 5: the Rust walk must
/// visit exactly the store's registered, non-null shows and run the real, untouched
/// `ZTShowInfo::enterNewMonth` on each, while the real vanilla body (through the stage-5
/// trampoline) walks the standalone vanilla tree planted alongside the store (stage 9 stopped
/// the hooked writers maintaining the tree, so the tree-side registrations are made explicitly
/// through the raw-body trampoline).
///
/// Both poles run the same vanilla visitor, so per-show verification rests on its observable
/// transform (`ZTShowInfo_enterNewMonth.c`): copy `+0x7c` into `+0x80` and zero `+0x7c`, copy
/// `+0x94` into `+0x98` and zero `+0x94`, copy `+0x88` into `+0x8c`, then recompute `+0x88`
/// through the opaque `FUN_0059e8f0` - fed the show's `field_0x70` id and, per the decompile's
/// `unaff_SI`, whatever the walk left in `%ESI`, so genuinely call-dependent and unpredictible
/// here - and add it into `+0x90`. Seeding `+0x90` to `0.0` makes that final sum exact in any
/// precision (`F + 0.0` stores back bit-for-bit as the same float the `+0x88` store rounded,
/// even through the real body's x87 float10 arithmetic), so one visit is fully characterized by
/// the two copy-and-zero pairs, the `+0x8c` copy, and `+0x90 == +0x88`. A second visit (the real
/// pole re-walking A/B) can't reuse the sum identity once `+0x90` is non-zero - its float10
/// double-rounding isn't reproducible from Rust - so it asserts the six precision-free fields
/// only.
///
/// The differential set: A (preset id) and B (counter-assigned id) register through the hooked
/// `REGISTER_SHOW` into the store and are then planted into the standalone vanilla tree through
/// the raw `call_real_register_show` trampoline as well (hooked first, so B's store-assigned id
/// is already in `field_0x70` when the raw body reads it and both stores key B identically);
/// C (its own preset id) plants into the vanilla tree only, never the store - the Rust pole
/// must leave it untouched (post-cutover its reads come from the store, not the tree: the
/// property this pins), while the real pole must visit it. A never-registered control show must
/// be untouched by both poles. Each show needs a real, empty, self-referential `0x18` header
/// node at `+0x44`: the real callee starts its embedded pending-scripts walk at the header's
/// *leftmost* pointer (`header+0x8`), which `build_standalone_show_info`'s zeroed embedded
/// self-header leaves null - fine for `ztshow.rs`'s own root-based Rust walks, a null deref for
/// the real body (same node shape the stage-3 test builds for `+0x38`'s clear path). All
/// teardown is leak-only (`showmgr_live_support::allocate_uninitialized`'s doc comment); the
/// store drains through the hooked unregister path, the tree-side plants (A, B, C) drain
/// through the raw unregister trampoline, and a final both-poles pass over the emptied map must
/// produce zero deltas - the walk's empty-map no-op on both sides.
pub(crate) fn run_ztshowmgr_enter_new_month_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_ENTER_NEW_MONTH";

    if ztshowmgr::registered_show_count() != 0 {
        let msg = "Rust registered-shows store should be empty when the test starts";
        error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
        }
        return true;
    }

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }

    /// Builds a real, empty, self-referential `_Tree` header node and stores it at
    /// `show_info+0x44` - see the test's doc comment for why the stock standalone shape can't
    /// survive the real callee's leftmost-based walk. Leak-only, like everything else here.
    fn init_pending_script_header(show_info: u32) {
        let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
        unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
        save_to_memory(node + 0x8, node);
        save_to_memory(node + 0xc, node);
        save_to_memory(show_info + 0x44, node);
    }

    /// The seven fields the real `ZTShowInfo::enterNewMonth` transform touches (f32s compared
    /// as raw bits throughout).
    #[derive(Clone, Copy, PartialEq)]
    struct ShowAccumulators {
        f7c: u32,
        f80: u32,
        f94: u32,
        f98: u32,
        f88: u32,
        f8c: u32,
        f90: u32,
    }

    fn snap(show: u32) -> ShowAccumulators {
        ShowAccumulators {
            f7c: get_from_memory(show + 0x7c),
            f80: get_from_memory(show + 0x80),
            f94: get_from_memory(show + 0x94),
            f98: get_from_memory(show + 0x98),
            f88: get_from_memory(show + 0x88),
            f8c: get_from_memory(show + 0x8c),
            f90: get_from_memory(show + 0x90),
        }
    }

    fn push_mismatch(label: &str, detail: String, failures: &mut Vec<String>) {
        failures.push(format!("{label}: {detail}"));
    }

    /// One more application of the transform on top of `prev`. `check_sum` additionally asserts
    /// the `+0x90 += F` recompute via the seed-`0.0` identity - only valid while `prev.f90`
    /// was zero.
    fn assert_visited(label: &str, show: u32, prev: &ShowAccumulators, cur: &ShowAccumulators, check_sum: bool, failures: &mut Vec<String>) {
        if cur.f80 != prev.f7c {
            push_mismatch(label, format!("show {show:#010x} +0x80 should be old +0x7c ({:#010x}), got {:#010x}", prev.f7c, cur.f80), failures);
        }
        if cur.f7c != 0 {
            push_mismatch(label, format!("show {show:#010x} +0x7c should be zeroed, got {:#010x}", cur.f7c), failures);
        }
        if cur.f98 != prev.f94 {
            push_mismatch(label, format!("show {show:#010x} +0x98 should be old +0x94 ({:#010x}), got {:#010x}", prev.f94, cur.f98), failures);
        }
        if cur.f94 != 0 {
            push_mismatch(label, format!("show {show:#010x} +0x94 should be zeroed, got {:#010x}", cur.f94), failures);
        }
        if cur.f8c != prev.f88 {
            push_mismatch(label, format!("show {show:#010x} +0x8c should be old +0x88 ({:#010x}), got {:#010x}", prev.f88, cur.f8c), failures);
        }
        if check_sum && cur.f90 != cur.f88 {
            push_mismatch(label, format!("show {show:#010x} +0x90 should equal the recomputed +0x88 ({:#010x}) with the 0.0 seed, got {:#010x}", cur.f88, cur.f90), failures);
        }
    }

    fn assert_untouched(label: &str, show: u32, prev: &ShowAccumulators, cur: &ShowAccumulators, failures: &mut Vec<String>) {
        if *cur != *prev {
            push_mismatch(
                label,
                format!(
                    "show {show:#010x} must be untouched by this pole (+0x7c {:#010x}->{:#010x}, +0x80 {:#010x}->{:#010x}, +0x94 {:#010x}->{:#010x}, +0x98 {:#010x}->{:#010x}, +0x88 {:#010x}->{:#010x}, +0x8c {:#010x}->{:#010x}, +0x90 {:#010x}->{:#010x})",
                    prev.f7c, cur.f7c, prev.f80, cur.f80, prev.f94, cur.f94, prev.f98, cur.f98, prev.f88, cur.f88, prev.f8c, cur.f8c, prev.f90, cur.f90
                ),
                failures,
            );
        }
    }

    let mut failures: Vec<String> = Vec::new();

    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    // Distinct seeds per show so a copy from the wrong show can never pass; `+0x90`'s explicit
    // 0.0 documents the sum-identity premise the first-application assertions rest on.
    let show_a = ztshow_live_support::build_standalone_show_info();
    let show_b = ztshow_live_support::build_standalone_show_info();
    let show_c = ztshow_live_support::build_standalone_show_info();
    let control = ztshow_live_support::build_standalone_show_info();
    const SEEDS: [(u32, u32, f32); 4] = [(0x2a, 0x2b, 1.5), (0x3c, 0x3d, 2.5), (0x4e, 0x4f, 3.5), (0x5f, 0x60, 4.5)];
    for (show, (seed_7c, seed_94, seed_88)) in [show_a, show_b, show_c, control].into_iter().zip(SEEDS) {
        init_pending_script_header(show);
        save_to_memory(show + 0x7c, seed_7c);
        save_to_memory(show + 0x94, seed_94);
        save_to_memory(show + 0x88, seed_88.to_bits());
        save_to_memory(show + 0x90, 0.0f32.to_bits());
    }

    const PRESET_ID_A: u16 = 0x1234;
    const PRESET_ID_C: u16 = 0x4321;
    save_to_memory(show_a + 0x70, PRESET_ID_A);
    save_to_memory(show_c + 0x70, PRESET_ID_C);
    // show_b keeps its zero-init `field_0x70` (the counter-assignment case); its fresh id is
    // read back after registering.

    // A and B go through the hooked register (store) and are then planted into the vanilla tree
    // through the raw vanilla-body trampoline (hooked first: B's store-assigned id must be in
    // field_0x70 before the raw body reads it, so both stores key B identically); C plants into
    // the vanilla tree only - the store must never see it.
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } {
        failures.push("register(A, false) should return 1".to_string());
    }
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } {
        failures.push("register(B, false) should return 1".to_string());
    }
    if !showmgr_live_support::call_real_register_show(mgr as *const u32, show_a as *const u32, false) {
        failures.push("raw register(A, false) should return 1 (tree-side plant)".to_string());
    }
    if !showmgr_live_support::call_real_register_show(mgr as *const u32, show_b as *const u32, false) {
        failures.push("raw register(B, false) should return 1 (tree-side plant under B's already-assigned id)".to_string());
    }
    if !showmgr_live_support::call_real_register_show(mgr as *const u32, show_c as *const u32, false) {
        failures.push("raw register(C, false) should return 1".to_string());
    }
    let id_b = get_from_memory::<u16>(show_b + 0x70);
    if id_b == 0 || id_b == PRESET_ID_A || id_b == PRESET_ID_C {
        failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
    }
    if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a)
        || ztshowmgr::registered_show_for_id(id_b) != Some(show_b)
        || ztshowmgr::registered_show_for_id(PRESET_ID_C).is_some()
    {
        failures.push("store should hold exactly A and B after the three registrations".to_string());
    }

    // Rust pole: visits exactly the store's pair; C (tree-only) and the control stay untouched.
    let pre_rust = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
    unsafe { ZTSHOWMGR_ENTER_NEW_MONTH.hooked()(mgr as *const u32) };
    let post_rust = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
    assert_visited("rust pole", show_a, &pre_rust[0], &post_rust[0], true, &mut failures);
    assert_visited("rust pole", show_b, &pre_rust[1], &post_rust[1], true, &mut failures);
    assert_untouched("rust pole", show_c, &pre_rust[2], &post_rust[2], &mut failures);
    assert_untouched("rust pole", control, &pre_rust[3], &post_rust[3], &mut failures);

    // Real pole: the planted vanilla tree holds A+B+C, so A/B get a second application
    // (precision-free fields only - see the doc comment) and C its first (sum identity valid).
    let pre_real = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
    showmgr_live_support::call_real_enter_new_month(mgr as *const u32);
    let post_real = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
    assert_visited("real pole", show_a, &pre_real[0], &post_real[0], false, &mut failures);
    assert_visited("real pole", show_b, &pre_real[1], &post_real[1], false, &mut failures);
    assert_visited("real pole", show_c, &pre_real[2], &post_real[2], true, &mut failures);
    assert_untouched("real pole", control, &pre_real[3], &post_real[3], &mut failures);

    // Cleanup: drain A/B from the store through the hooked unregister and A/B/C from the tree
    // through the raw one (stage 9: the hooked path no longer touches the tree), then both
    // poles over the emptied map must be no-ops.
    for show in [show_a, show_b] {
        unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
    }
    for show in [show_a, show_b, show_c] {
        showmgr_live_support::call_real_unregister_show(mgr as *const u32, 0, show as *const u32, false);
    }
    let remaining = ztshowmgr::registered_show_count();
    if remaining != 0 {
        failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
    }
    let pre_empty = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
    unsafe { ZTSHOWMGR_ENTER_NEW_MONTH.hooked()(mgr as *const u32) };
    showmgr_live_support::call_real_enter_new_month(mgr as *const u32);
    let post_empty = [snap(show_a), snap(show_b), snap(show_c), snap(control)];
    for (i, show) in [show_a, show_b, show_c, control].into_iter().enumerate() {
        assert_untouched("empty-map poles", show, &pre_empty[i], &post_empty[i], &mut failures);
    }

    finish_test(test_name, failures, failure_log)
}
/// The `ZTSHOWMGR_UPDATE` sentinel's visit log - recorded `this` values in call order.
/// Module-level so the sentinel fn can reach it; only that test touches it, and the battery is
/// single-threaded.
static ZTSHOWMGR_UPDATE_VISITS: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

/// Sentinel the `ZTSHOWMGR_UPDATE` test plants in each standalone show's fake vtable at slot
/// `+0x20` - the exact slot both `ZTShowMgr::update` bodies virtually dispatch through - and
/// which records the `this` it was called with.
unsafe extern "thiscall" fn ztshowmgr_update_sentinel(this: *const u32) {
    ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().push(this as u32);
}

/// `ZTSHOWMGR_UPDATE` - stage 5's second walk (`ztshowmgr-implementation-plan.md`), verified
/// through sentinel vtables rather than the real `ZTShowInfo::update` callee: that callee
/// virtual-calls the show's own `listen`/`cleanupEvents` vtable slots plus a slot on the
/// embedded `ZTShow` (`ZTShowInfo_update.c`), the whole event-list subsystem a zeroed
/// standalone show can't back. The sentinel measures exactly the part stage 5 owns - the walk
/// set, the ascending-key order, the size guard, and the dispatch convention - while the callee
/// behind slot `+0x20` stays real vanilla in the live game, untouched.
///
/// The differential set is the stage-5 standard: A (preset id) and B (counter-assigned id)
/// register through the hooked `REGISTER_SHOW` (store) and are planted into the vanilla tree
/// through the raw `call_real_register_show` trampoline as well (hooked first, so B's
/// store-assigned id keys both stores identically - stage 9 stopped the hooked writers
/// maintaining the tree), C (its own preset id) through the raw trampoline only (vanilla tree),
/// plus a never-registered control. The Rust pole must record exactly A+B in ascending-id order
/// (the `BTreeMap` iteration order substituting for vanilla's in-order walk) and nothing else;
/// the real pole - the vanilla body through the stage-5 trampoline, walking the planted
/// standalone tree - must record A+B+C in ascending-id order through the *same* sentinel, which
/// also pins that vanilla really dispatches slot `+0x20` with `this` = the show pointer. With
/// the store drained and the tree emptied, both poles must record nothing - the port's
/// `!is_empty()` guard and vanilla's `mbr_0x2c > 0` guard both on an empty map. All teardown is
/// leak-only.
pub(crate) fn run_ztshowmgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_UPDATE";

    if ztshowmgr::registered_show_count() != 0 {
        let msg = "Rust registered-shows store should be empty when the test starts";
        error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
        }
        return true;
    }

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }

    /// Gives `show` a fake vtable whose only populated slot is `+0x20` (the dispatched one) -
    /// a walk that read any other slot would call null and crash the test process rather than
    /// pass. Leak-only, like everything else here.
    fn install_sentinel_vtable(show: u32) {
        let vtable = unsafe { standalone::OPERATOR_NEW.original()(0x24) } as u32;
        unsafe { std::ptr::write_bytes(vtable as *mut u8, 0, 0x24) };
        save_to_memory(vtable + 0x20, ztshowmgr_update_sentinel as *const () as usize as u32);
        save_to_memory(show, vtable);
    }

    let mut failures: Vec<String> = Vec::new();

    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    let show_a = ztshow_live_support::build_standalone_show_info();
    let show_b = ztshow_live_support::build_standalone_show_info();
    let show_c = ztshow_live_support::build_standalone_show_info();
    let control = ztshow_live_support::build_standalone_show_info();
    for show in [show_a, show_b, show_c, control] {
        install_sentinel_vtable(show);
    }

    const PRESET_ID_A: u16 = 0x1234;
    const PRESET_ID_C: u16 = 0x4321;
    save_to_memory(show_a + 0x70, PRESET_ID_A);
    save_to_memory(show_c + 0x70, PRESET_ID_C);
    // show_b keeps its zero-init `field_0x70` (the counter-assignment case).

    // A and B through the hooked register (store) plus the raw trampoline plant (vanilla tree,
    // hooked first so B's store-assigned id keys both stores identically); C through the raw
    // vanilla-body trampoline only (vanilla tree).
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } {
        failures.push("register(A, false) should return 1".to_string());
    }
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } {
        failures.push("register(B, false) should return 1".to_string());
    }
    if !showmgr_live_support::call_real_register_show(mgr as *const u32, show_a as *const u32, false) {
        failures.push("raw register(A, false) should return 1 (tree-side plant)".to_string());
    }
    if !showmgr_live_support::call_real_register_show(mgr as *const u32, show_b as *const u32, false) {
        failures.push("raw register(B, false) should return 1 (tree-side plant under B's already-assigned id)".to_string());
    }
    if !showmgr_live_support::call_real_register_show(mgr as *const u32, show_c as *const u32, false) {
        failures.push("raw register(C, false) should return 1 (the raw body only guarantees AL; the hooked path returns the port's cleaned 0/1)".to_string());
    }
    let id_b = get_from_memory::<u16>(show_b + 0x70);
    if id_b == 0 || id_b == PRESET_ID_A || id_b == PRESET_ID_C {
        failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
    }
    if ztshowmgr::registered_show_for_id(PRESET_ID_A) != Some(show_a)
        || ztshowmgr::registered_show_for_id(id_b) != Some(show_b)
        || ztshowmgr::registered_show_for_id(PRESET_ID_C).is_some()
    {
        failures.push("store should hold exactly A and B after the three registrations".to_string());
    }

    // Expected visit sequences, ascending by the id each show registered under (vanilla's
    // in-order walk order, which the BTreeMap's ascending-u16 iteration reproduces).
    let mut store_ids: Vec<(u16, u32)> = vec![(PRESET_ID_A, show_a), (id_b, show_b)];
    store_ids.sort_unstable();
    let mut tree_ids = store_ids.clone();
    tree_ids.push((PRESET_ID_C, show_c));
    tree_ids.sort_unstable();
    let pointers = |pairs: &[(u16, u32)]| -> Vec<u32> { pairs.iter().map(|(_, ptr)| *ptr).collect() };

    // Rust pole: exactly the store's pair, in ascending-id order - C (tree-only) and the
    // control must not appear.
    ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clear();
    unsafe { ZTSHOWMGR_UPDATE.hooked()(mgr as *const u32) };
    let rust_visits = ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clone();
    if rust_visits != pointers(&store_ids) {
        failures.push(format!(
            "rust pole visits should be exactly the store's registrations in ascending-id order ({:#010x?}), got {rust_visits:#010x?}",
            pointers(&store_ids)
        ));
    }

    // Real pole: the vanilla walk over the planted tree must dispatch slot +0x20 with
    // this = the show pointer for A+B+C, through the same sentinel.
    ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clear();
    showmgr_live_support::call_real_update(mgr as *const u32);
    let real_visits = ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clone();
    if real_visits != pointers(&tree_ids) {
        failures.push(format!(
            "real pole visits (vanilla walk dispatching slot +0x20) should be exactly the tree's registrations in ascending-id order ({:#010x?}), got {real_visits:#010x?}",
            pointers(&tree_ids)
        ));
    }

    // Cleanup: drain A/B from the store through the hooked unregister and A/B/C from the tree
    // through the raw one (stage 9: the hooked path no longer touches the tree), then both
    // poles over the emptied map must record nothing.
    for show in [show_a, show_b] {
        unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
    }
    for show in [show_a, show_b, show_c] {
        showmgr_live_support::call_real_unregister_show(mgr as *const u32, 0, show as *const u32, false);
    }
    let remaining = ztshowmgr::registered_show_count();
    if remaining != 0 {
        failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
    }
    ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clear();
    unsafe { ZTSHOWMGR_UPDATE.hooked()(mgr as *const u32) };
    showmgr_live_support::call_real_update(mgr as *const u32);
    let leftover_visits = ZTSHOWMGR_UPDATE_VISITS.lock().unwrap().clone();
    if !leftover_visits.is_empty() {
        failures.push(format!("both poles on the emptied map must visit nothing, got {leftover_visits:#010x?}"));
    }

    finish_test(test_name, failures, failure_log)
}
/// `ZTSHOWMGR_SAVE_LOAD` - stage 6 (`ztshowmgr-implementation-plan.md`): `ZTShowMgr::save`/
/// `load` wrap exactly two pieces - the embedded `ZTShowScriptMgr`'s own save/load (already
/// `ztshowscriptmgr`'s Rust store, reached through the same direct `CALL` vanilla makes) and
/// the 2-byte show-id-counter persistence - so the test drives those with `io_redirect`
/// standing in for the real file. One script is seeded into the script store and a known
/// counter value into **both** copies of the counter (stage 9 moved the live counter into the
/// Rust store, but the real save/load bodies reached through the stage-6 trampolines still
/// read/write the vanilla global in place - so the Rust poles exercise the store copy and the
/// real poles the global, seeded identically so their streams still compare). Then:
/// - the Rust save and the real body through the stage-6 trampoline must capture
///   byte-identical streams (both delegations reach the same Rust script-store save, so the
///   pole isolates each body's own tail: the Rust port's store read vs. vanilla's counter
///   global read in place, and the 2-bytes/count-1 write shape). The real pole's *return
///   byte* is deliberately not asserted: vanilla's body computes it via a full-EAX
///   `CMP %EAX, 1` on `WriteBytesToFile`'s return (`ZTShowMgr_save.asm`), and inside a
///   capture window that callee is `io_redirect`'s detour, whose Rust `bool` return defines
///   only `AL` - upper EAX is register garbage, so the compare reads as failure. A pure
///   test-harness artifact: the redirect path exists only inside capture windows, while the
///   passthrough path (and the un-hooked real function the live game calls) returns a
///   full-width 0/1. The load pole's real body returning 1 live-verifies the identical
///   `SETZ`/`AND` return tail - there the other redirected callee (`DEALLOCATE`) returns a
///   full-width `u32`, so vanilla's compare succeeds;
/// - the stream must be exactly the script store's own payload plus the counter's 2 LE bytes
///   on the end - proven by replaying the prefix through `ztshowscriptmgr::load_mgr` (which
///   also must recover the seeded script) and comparing the tail against the seeded value;
/// - replaying the full stream at version 0x100 through both poles must restore the script
///   store and the counter (the store copy through the hooked pole, the global through the
///   real one) - and the hooked pole's restored store counter must **continue**: a subsequent
///   id-0 register through the hooked `REGISTER_SHOW` must assign exactly
///   `SEEDED_COUNTER + 1`, pinning the register-after-load counter continuity stage 9 created
///   (one owner, both consumers Rust-side now);
/// - replaying at version 0x60 (at/under the gate) must restore the scripts but leave the
///   counter untouched, through both poles (each in its own copy);
/// - replaying only the store payload (counter bytes stripped) at version 0x100 must fail on
///   `ZTShowMgr`'s own counter read and return failure with the counter untouched - the
///   scriptmgr's own trailing counter inside that payload satisfies its loader, so the
///   failure is specifically the outer read.
///
/// Both counter copies are saved and restored around the whole test; the script store
/// is reset before and after (successful replays also restore its own persisted counter -
/// reset away again), and the registered-shows store must still be empty at the end.
pub(crate) fn run_ztshowmgr_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_SAVE_LOAD";
    const CURRENT_VERSION: u32 = 0x100;
    const GATED_VERSION: u32 = 0x60;
    const SEEDED_COUNTER: u16 = 0x0ABC;

    if ztshowmgr::registered_show_count() != 0 {
        let msg = "Rust registered-shows store should be empty when the test starts";
        error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
        }
        return true;
    }

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }
    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    let counter_addr = showmgr_live_support::show_id_counter_addr();
    let original_counter = get_from_memory::<u16>(counter_addr);
    let original_store_counter = showmgr_live_support::show_id_counter();

    let mut failures: Vec<String> = Vec::new();
    let dummy_file: u32 = 0;
    let file_ptr = &dummy_file as *const u32;

    // Seed: one script in the script store, a known show-id counter in both copies (the Rust
    // poles read the store's, the real bodies the vanilla global).
    ztshowscriptmgr::live_support::reset_state();
    let script_a = make_registered_show_script(11, 101);
    save_to_memory(counter_addr, SEEDED_COUNTER);
    showmgr_live_support::set_show_id_counter(SEEDED_COUNTER);

    // Rust save, then the real body's save - captures must be byte-identical. The real
    // pole's return byte is deliberately unread (harness artifact, see this test's doc
    // comment): vanilla's full-EAX compare against io_redirect's bool-returning write
    // detour cannot succeed inside a capture window.
    io_redirect::begin_capture();
    let rust_save_ret = unsafe { ZTSHOWMGR_SAVE.hooked()(mgr as *const u32, file_ptr as *const i8) };
    let rust_bytes = io_redirect::end_capture();
    io_redirect::begin_capture();
    let _real_save_ret = showmgr_live_support::call_real_save(mgr as *const u32, file_ptr as *const i8);
    let real_bytes = io_redirect::end_capture();
    if !rust_save_ret {
        failures.push(format!("hooked save should return true, got {rust_save_ret}"));
    }
    if real_bytes != rust_bytes {
        failures.push(format!(
            "real and rust save captures must be byte-identical ({} vs {} bytes)",
            real_bytes.len(),
            rust_bytes.len()
        ));
    }

    // Shape: the stream is the script store's payload (prefix decodes via load_mgr and
    // recovers the seeded script) plus the counter's 2 LE bytes on the end.
    if rust_bytes.len() < 6 {
        failures.push(format!("save capture implausibly small ({} bytes), expected script payload + 2 counter bytes", rust_bytes.len()));
    } else {
        let (payload, counter_tail) = rust_bytes.split_at(rust_bytes.len() - 2);
        if counter_tail != SEEDED_COUNTER.to_le_bytes() {
            failures.push(format!(
                "save capture must end with the counter's 2 LE bytes ({SEEDED_COUNTER:#06x}), got {counter_tail:02x?}"
            ));
        }
        ztshowscriptmgr::live_support::reset_state();
        io_redirect::begin_replay(payload.to_vec());
        let payload_ok = ztshowscriptmgr::load_mgr(file_ptr, CURRENT_VERSION);
        io_redirect::end_replay();
        if !payload_ok || !ztshowscriptmgr::script_exists_by_id(script_a) {
            failures.push("save capture's prefix should decode as the script store's payload containing the seeded script".to_string());
        }
    }

    // Full-stream load, version over the gate: scripts + counter restored. Rust pole first
    // (its counter lands in the store), then the real body through the trampoline (its counter
    // lands in the vanilla global). Both copies are clobbered to distinct sentinels before
    // each pole so a no-op restore is detectable on the side that pole owns.
    ztshowscriptmgr::live_support::reset_state();
    save_to_memory(counter_addr, 1_u16);
    showmgr_live_support::set_show_id_counter(1);
    io_redirect::begin_replay(rust_bytes.clone());
    let rust_load_ret = unsafe { ZTSHOWMGR_LOAD.hooked()(mgr as *const u32, file_ptr, CURRENT_VERSION) };
    io_redirect::end_replay();
    if !rust_load_ret {
        failures.push(format!("hooked load should return true, got {rust_load_ret}"));
    }
    if !ztshowscriptmgr::script_exists_by_id(script_a) {
        failures.push("hooked load should have restored the seeded script".to_string());
    }
    if showmgr_live_support::show_id_counter() != SEEDED_COUNTER {
        failures.push(format!(
            "hooked load should have restored the store counter to {SEEDED_COUNTER:#06x}, got {:#06x}",
            showmgr_live_support::show_id_counter()
        ));
    }

    // Register-after-load counter continuity: the restored store counter is the live one, so
    // the next id-0 register must continue from it - exactly SEEDED_COUNTER + 1 (no wrap at
    // this seed).
    let continuity_show = ztshow_live_support::build_standalone_show_info();
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, continuity_show as *const u32, false) } {
        failures.push("register after load should return 1".to_string());
    }
    if get_from_memory::<u16>(continuity_show + 0x70) != SEEDED_COUNTER + 1 {
        failures.push(format!(
            "register after load should assign exactly {:#06x} (the restored counter + 1), got {:#06x}",
            SEEDED_COUNTER + 1,
            get_from_memory::<u16>(continuity_show + 0x70)
        ));
    }
    if !unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, continuity_show as *const u32, false) } {
        failures.push("unregistering the continuity show should return 1".to_string());
    }

    ztshowscriptmgr::live_support::reset_state();
    save_to_memory(counter_addr, 2_u16);
    showmgr_live_support::set_show_id_counter(2);
    io_redirect::begin_replay(real_bytes.clone());
    let real_load_ret = showmgr_live_support::call_real_load(mgr as *const u32, file_ptr, CURRENT_VERSION);
    io_redirect::end_replay();
    if !real_load_ret {
        failures.push(format!("real load should return true, got {real_load_ret}"));
    }
    if !ztshowscriptmgr::script_exists_by_id(script_a) {
        failures.push("real load should have restored the seeded script".to_string());
    }
    if get_from_memory::<u16>(counter_addr) != SEEDED_COUNTER {
        failures.push(format!(
            "real load should have restored the counter global to {SEEDED_COUNTER:#06x}, got {:#06x}",
            get_from_memory::<u16>(counter_addr)
        ));
    }

    // Version gate: at/under 0x60 the scripts still load but neither pole touches the counter.
    ztshowscriptmgr::live_support::reset_state();
    save_to_memory(counter_addr, 3_u16);
    showmgr_live_support::set_show_id_counter(3);
    io_redirect::begin_replay(rust_bytes.clone());
    let gated_ret = unsafe { ZTSHOWMGR_LOAD.hooked()(mgr as *const u32, file_ptr, GATED_VERSION) };
    io_redirect::end_replay();
    if !gated_ret {
        failures.push(format!("hooked load at version 0x60 should still return true, got {gated_ret}"));
    }
    if !ztshowscriptmgr::script_exists_by_id(script_a) {
        failures.push("hooked load at version 0x60 should still have restored the seeded script".to_string());
    }
    if showmgr_live_support::show_id_counter() != 3 {
        failures.push("hooked load at version 0x60 must not touch the store counter (gate not passed)".to_string());
    }

    ztshowscriptmgr::live_support::reset_state();
    save_to_memory(counter_addr, 4_u16);
    showmgr_live_support::set_show_id_counter(4);
    io_redirect::begin_replay(real_bytes.clone());
    let real_gated_ret = showmgr_live_support::call_real_load(mgr as *const u32, file_ptr, GATED_VERSION);
    io_redirect::end_replay();
    if !real_gated_ret {
        failures.push(format!("real load at version 0x60 should still return true, got {real_gated_ret}"));
    }
    if get_from_memory::<u16>(counter_addr) != 4 {
        failures.push("real load at version 0x60 must not touch the counter global (gate not passed)".to_string());
    }

    // Short read: the store payload alone (its own trailing counter satisfies the scriptmgr's
    // loader) leaves nothing for ZTShowMgr's own 2-byte read - both poles must report
    // failure and leave their counter copy untouched.
    let stripped = rust_bytes[..rust_bytes.len() - 2].to_vec();
    ztshowscriptmgr::live_support::reset_state();
    save_to_memory(counter_addr, 5_u16);
    showmgr_live_support::set_show_id_counter(5);
    io_redirect::begin_replay(stripped.clone());
    let short_ret = unsafe { ZTSHOWMGR_LOAD.hooked()(mgr as *const u32, file_ptr, CURRENT_VERSION) };
    io_redirect::end_replay();
    if short_ret {
        failures.push(format!("hooked load on a stream missing its counter bytes should return false, got {short_ret}"));
    }
    if showmgr_live_support::show_id_counter() != 5 {
        failures.push("a failed counter read must leave the store counter untouched".to_string());
    }

    ztshowscriptmgr::live_support::reset_state();
    save_to_memory(counter_addr, 6_u16);
    showmgr_live_support::set_show_id_counter(6);
    io_redirect::begin_replay(stripped);
    let real_short_ret = showmgr_live_support::call_real_load(mgr as *const u32, file_ptr, CURRENT_VERSION);
    io_redirect::end_replay();
    if real_short_ret {
        failures.push(format!("real load on a stream missing its counter bytes should return false, got {real_short_ret}"));
    }
    if get_from_memory::<u16>(counter_addr) != 6 {
        failures.push("a failed real counter read must leave the counter global untouched".to_string());
    }

    // Hygiene: reset the script store, put both counter copies back, and confirm the
    // registered-shows store was never touched.
    ztshowscriptmgr::live_support::reset_state();
    save_to_memory(counter_addr, original_counter);
    showmgr_live_support::set_show_id_counter(original_store_counter);
    let remaining = ztshowmgr::registered_show_count();
    if remaining != 0 {
        failures.push(format!("registered-shows store should still be empty, has {remaining} entries"));
    }

    finish_test(test_name, failures, failure_log)
}
/// `ZTSHOWMGR_IS_DOING_SHOW` - stage 7 (`ztshowmgr-implementation-plan.md`): `ZTShowMgr::
/// isDoingShow` composes the store-backed `getShowInfo` lookup (stage 4) with one real,
/// untouched `ZTShow::getShowScriptState` walk over the found show's embedded `ZTShow`, so
/// both poles read the same registrations out of the same store - the real body's internal
/// `getShowInfo` `CALL` lands in the stage-4 detour - and the differential is the glue each
/// pole owns: the port's clean 0/1 (macOS's body normalizes to the same full-width predicate)
/// against vanilla's `SETNZ %AL`-only return plus the `LEA %ECX, [EAX + 0x4]` embedded-show
/// hand-off.
///
/// The state side is built by hand on a standalone show: the embedded `ZTShow`'s script-state
/// map is the self-referential header object at `show_info+0x38` (the same header shape the
/// stage-3 test builds for `ZTShowState::clear`), with one zero-padded `0x18` node hung off
/// its root slot at `+0x3c` - key (the unit id) at node `+0x10`, a non-null value at `+0x14`
/// (the real callee returns `[node+0x14]` without dereferencing it, so an opaque non-null
/// buffer stands in for the `ZTShowScriptState`). `AI_cls_0x404fd6::find`'s key compare is a
/// full 32-bit `CMP dword ptr [EAX+0x10]` (`AI_cls_0x404fd6_find.asm`), which the zeroed node
/// padding satisfies and which a probe crossing bit 16 must NOT match - both poles answer
/// through the same real walk, so that probe pins the effective key width rather than
/// differentiating the poles. All teardown is leak-only; the store drains through the hooked
/// unregister path and a final both-poles pass confirms the registration is really gone.
pub(crate) fn run_ztshowmgr_is_doing_show_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_IS_DOING_SHOW";

    if ztshowmgr::registered_show_count() != 0 {
        let msg = "Rust registered-shows store should be empty when the test starts";
        error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
        }
        return true;
    }

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }

    /// Both poles over one (unit, show) probe. The Rust pole must return exactly `expected`
    /// (the port's clean 0/1); the real body defines only its `AL` byte (upper EAX holds the
    /// state pointer's high bits on a hit), so its return is compared through the low-byte
    /// mask.
    fn check(label: &str, mgr: *mut ZTShowMgr, unit_id: u32, show_id: u16, expected: bool, failures: &mut Vec<String>) {
        let rust_ret = unsafe { ZTSHOWMGR_IS_DOING_SHOW.hooked()(mgr as *const u32, unit_id, show_id) };
        if rust_ret != expected {
            failures.push(format!("{label}: rust pole should return {expected}, got {rust_ret}"));
        }
        let real_ret = showmgr_live_support::call_real_is_doing_show(mgr as *const u32, unit_id, show_id);
        if real_ret != expected {
            failures.push(format!("{label}: real pole should return {expected}, got {real_ret}"));
        }
    }

    let mut failures: Vec<String> = Vec::new();

    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    // show_a carries the seeded state (unit UNIT_A -> a non-null script-state pointer) inside
    // its embedded ZTShow's script-state map; show_b stays stateless (empty self-referential
    // header only). Leak-only allocations throughout.
    const UNIT_A: u32 = 0x2a;
    let show_a = ztshow_live_support::build_standalone_show_info();
    let show_b = ztshow_live_support::build_standalone_show_info();
    for show in [show_a, show_b] {
        // The map object at show_info+0x38 doubles as its own empty header (self-sentinel) -
        // a zeroed +0x38 would leave `find`'s header load null and crash its root read.
        save_to_memory(show + 0x38, show + 0x38);
    }
    let fake_state = unsafe { standalone::OPERATOR_NEW.original()(0x20) } as u32;
    unsafe { std::ptr::write_bytes(fake_state as *mut u8, 0, 0x20) };
    let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
    unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
    save_to_memory(node + 0x4, show_a + 0x38); // parent = header (hygiene; find never reads it)
    save_to_memory(node + 0x10, UNIT_A); // key: the unit id; node stays zero-padded at +0x12
    save_to_memory(node + 0x14, fake_state); // value: only null-tested by the real callee
    save_to_memory(show_a + 0x3c, node); // the header's root slot

    const PRESET_ID_A: u16 = 0x1234;
    save_to_memory(show_a + 0x70, PRESET_ID_A);
    // show_b keeps its zero-init `field_0x70` (the counter-assignment case).
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } {
        failures.push("register(A, false) should return 1".to_string());
    }
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } {
        failures.push("register(B, false) should return 1".to_string());
    }
    let id_b = get_from_memory::<u16>(show_b + 0x70);
    if id_b == 0 || id_b == PRESET_ID_A {
        failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
    }

    // Unregistered probes: both poles miss on the store-backed lookup.
    for probe in [0x0000u16, 0x8000, 0xffff] {
        check(&format!("unregistered id {probe:#06x}"), mgr, UNIT_A, probe, false, &mut failures);
    }
    // The seeded hit: UNIT_A is doing show A through both poles.
    check("seeded state hit", mgr, UNIT_A, PRESET_ID_A, true, &mut failures);
    // Misses against the seeded show: unknown unit, the unit-id-0 boundary, and a key that
    // matches only below bit 16 (the 32-bit key-compare pin).
    check("unknown unit on seeded show", mgr, UNIT_A + 1, PRESET_ID_A, false, &mut failures);
    check("unit id 0 on seeded show", mgr, 0, PRESET_ID_A, false, &mut failures);
    check("key differing above bit 16", mgr, UNIT_A | 0x1_0000, PRESET_ID_A, false, &mut failures);
    // Registered but stateless: the walk runs over an empty map and misses for every unit.
    check("stateless show, seeded unit", mgr, UNIT_A, id_b, false, &mut failures);
    check("stateless show, unit id 0", mgr, 0, id_b, false, &mut failures);

    // Cleanup: drain both through the hooked unregister (clear=false - the clear path is the
    // stage-3 tests' concern), then the hit probe must miss through both poles.
    for show in [show_a, show_b] {
        unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
    }
    let remaining = ztshowmgr::registered_show_count();
    if remaining != 0 {
        failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
    }
    check("unregistered after cleanup", mgr, UNIT_A, PRESET_ID_A, false, &mut failures);

    finish_test(test_name, failures, failure_log)
}
/// `ZTSHOWMGR_IS_SHOW_SCRIPT_DONE` - stage 8 (`ztshowmgr-implementation-plan.md`): `ZTShowMgr::
/// isShowScriptDone` is structurally stage 7's sibling - the same store-backed `getShowInfo`
/// lookup (whose stage-4 detour the real body's raw `CALL` lands in) and the same real
/// `ZTShow::getShowScriptState` walk - so both poles again read the same registrations out of
/// the same store, and the differential is the glue each pole owns: the port's clean
/// zero-extended done byte against vanilla's `MOV %AL, byte ptr [EAX + 0x13]` read plus its
/// AL-only return.
///
/// The state side is built exactly like the stage-7 test's (self-referential header at
/// `show_info+0x38`, one zero-padded `0x18` node hung off its root slot, key at `+0x10`,
/// non-null value at `+0x14`) with one extra dimension: unlike `isDoingShow`, stage 8
/// *dereferences* the state pointer, so the stand-in's `+0x13` byte is meaningful. It is swept
/// through `0x00`/`0x37`/`0xff`: the `0x37` probe pins that the port returns the raw byte (a
/// wrongly normalized 0/1 would return `1` there, and the real pole's AL-masked compare would
/// catch the mirror-image mistake), and `0x00` pins that a found state with a zero done byte is
/// indistinguishable from a miss through the observable return, exactly like vanilla. All
/// teardown is leak-only; the store drains through the hooked unregister path and a final
/// both-poles pass confirms the registration is really gone.
pub(crate) fn run_ztshowmgr_is_show_script_done_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_IS_SHOW_SCRIPT_DONE";

    if ztshowmgr::registered_show_count() != 0 {
        let msg = "Rust registered-shows store should be empty when the test starts";
        error!("{}: {} (has {} entries)", test_name, msg, ztshowmgr::registered_show_count());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} (has {} entries)\n", test_name, msg, ztshowmgr::registered_show_count()).as_bytes());
        }
        return true;
    }

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }

    /// Both poles return the raw done byte, zero-extended - not a normalized 0/1 (see
    /// `ZTShowMgr::is_show_script_done`'s own doc comment: "the byte is the contract"). Compared
    /// through truthiness, matching every real caller's own `TEST AL, AL`.
    fn check(label: &str, mgr: *mut ZTShowMgr, script_id: u32, show_id: u16, expected: bool, failures: &mut Vec<String>) {
        let rust_ret = unsafe { ZTSHOWMGR_IS_SHOW_SCRIPT_DONE.hooked()(mgr as *const u32, script_id, show_id) };
        if (rust_ret != 0) != expected {
            failures.push(format!("{label}: rust pole should return {expected}, got {rust_ret:#04x}"));
        }
        let real_ret = showmgr_live_support::call_real_is_show_script_done(mgr as *const u32, script_id, show_id);
        if (real_ret != 0) != expected {
            failures.push(format!("{label}: real pole should return {expected}, got {real_ret:#04x}"));
        }
    }

    let mut failures: Vec<String> = Vec::new();

    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    // show_a carries the seeded state (unit UNIT_A -> a script-state stand-in whose +0x13 done
    // byte is swept below) inside its embedded ZTShow's script-state map; show_b stays
    // stateless (empty self-referential header only). Leak-only allocations throughout.
    const UNIT_A: u32 = 0x2a;
    let show_a = ztshow_live_support::build_standalone_show_info();
    let show_b = ztshow_live_support::build_standalone_show_info();
    for show in [show_a, show_b] {
        // The map object at show_info+0x38 doubles as its own empty header (self-sentinel) -
        // a zeroed +0x38 would leave `find`'s header load null and crash its root read.
        save_to_memory(show + 0x38, show + 0x38);
    }
    let fake_state = unsafe { standalone::OPERATOR_NEW.original()(0x20) } as u32;
    unsafe { std::ptr::write_bytes(fake_state as *mut u8, 0, 0x20) };
    let node = unsafe { standalone::OPERATOR_NEW.original()(0x18) } as u32;
    unsafe { std::ptr::write_bytes(node as *mut u8, 0, 0x18) };
    save_to_memory(node + 0x4, show_a + 0x38); // parent = header (hygiene; find never reads it)
    save_to_memory(node + 0x10, UNIT_A); // key: the unit id; node stays zero-padded at +0x12
    save_to_memory(node + 0x14, fake_state); // value: dereferenced at +0x13 by both poles
    save_to_memory(show_a + 0x3c, node); // the header's root slot

    const PRESET_ID_A: u16 = 0x1234;
    save_to_memory(show_a + 0x70, PRESET_ID_A);
    // show_b keeps its zero-init `field_0x70` (the counter-assignment case).
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_a as *const u32, false) } {
        failures.push("register(A, false) should return 1".to_string());
    }
    if !unsafe { ZTSHOWMGR_REGISTER_SHOW.hooked()(mgr as *const u32, show_b as *const u32, false) } {
        failures.push("register(B, false) should return 1".to_string());
    }
    let id_b = get_from_memory::<u16>(show_b + 0x70);
    if id_b == 0 || id_b == PRESET_ID_A {
        failures.push(format!("register(B) should have assigned a fresh non-colliding id, got {id_b:#06x}"));
    }

    // Unregistered probes: both poles miss on the store-backed lookup.
    for probe in [0x0000u16, 0x8000, 0xffff] {
        check(&format!("unregistered id {probe:#06x}"), mgr, UNIT_A, probe, false, &mut failures);
    }
    // The seeded hit, with the done byte swept: any nonzero byte must read as done on both poles.
    for byte in [0x00u8, 0x37, 0xff] {
        save_to_memory(fake_state + 0x13, byte);
        check(&format!("done byte {byte:#04x}"), mgr, UNIT_A, PRESET_ID_A, byte != 0, &mut failures);
    }
    // Misses against the seeded show: unknown unit, the unit-id-0 boundary, and a key that
    // matches only below bit 16 (the 32-bit key-compare pin).
    check("unknown unit on seeded show", mgr, UNIT_A + 1, PRESET_ID_A, false, &mut failures);
    check("unit id 0 on seeded show", mgr, 0, PRESET_ID_A, false, &mut failures);
    check("key differing above bit 16", mgr, UNIT_A | 0x1_0000, PRESET_ID_A, false, &mut failures);
    // Registered but stateless: the walk runs over an empty map and misses for every unit.
    check("stateless show, seeded unit", mgr, UNIT_A, id_b, false, &mut failures);
    check("stateless show, unit id 0", mgr, 0, id_b, false, &mut failures);

    // Cleanup: drain both through the hooked unregister (clear=false - the clear path is the
    // stage-3 tests' concern), then the hit probe must miss through both poles.
    for show in [show_a, show_b] {
        unsafe { ZTSHOWMGR_UNREGISTER_SHOW.hooked()(mgr as *const u32, 0, show as *const u32, false) };
    }
    let remaining = ztshowmgr::registered_show_count();
    if remaining != 0 {
        failures.push(format!("store should be empty after cleanup, has {remaining} entries"));
    }
    check("unregistered after cleanup", mgr, UNIT_A, PRESET_ID_A, false, &mut failures);

    finish_test(test_name, failures, failure_log)
}
/// `ZTSHOWMGR_REGISTER_UNREGISTER_GET_SCRIPT` - review follow-up, not part of the original
/// `ztshowmgr-implementation-plan.md`: `ZTShowMgr::registerScript`/`unregisterScript`/`getScript`
/// (the outer `ztshowmgr::REGISTER_SCRIPT`/`UNREGISTER_SCRIPT`/`GET_SCRIPT`,
/// `0x0046e89c`/`0x00473120`/`0x005a25b7`) are confirmed via `.asm` (`ADD ECX,0x34` + tail `CALL`)
/// to be genuine, un-detoured delegations into the embedded `ZTShowScriptMgr` sub-object's own
/// already-detoured addresses (`ztshowscriptmgr::{REGISTER_SCRIPT, UNREGISTER_SCRIPT, GET_SCRIPT}`,
/// exercised directly elsewhere in this battery). No prior live test drove these *outer*
/// addresses, so this closes that gap. The fourth delegation-shaped method,
/// `getShowScriptItems`, is deliberately excluded: its callee ignores the passed sub-object
/// pointer entirely and instead reads `GLOBAL_ZTWorldMgr`/`ZTUnitType::getTrickList`, so there is
/// no Rust-owned behavior for it to reach (see the implementation plan doc's "Composition with
/// ZTShowScriptMgr" section).
///
/// These three outer addresses are never detoured anywhere in the repo, so `.original()` on them
/// is always safe here and correctly routes through vanilla into the (hooked) inner address.
pub(crate) fn run_ztshowmgr_register_unregister_get_script_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_REGISTER_UNREGISTER_GET_SCRIPT";

    ztshowscriptmgr::live_support::reset_state();

    let mgr = showmgr_live_support::allocate_uninitialized();
    if mgr.is_null() {
        error!("{}: OPERATOR_NEW returned null for the ZTShowMgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: OPERATOR_NEW returned null for the ZTShowMgr\n", test_name).as_bytes());
        }
        return true;
    }

    let mut failures: Vec<String> = Vec::new();

    unsafe {
        std::ptr::write_bytes(mgr as *mut u8, 0, size_of::<ZTShowMgr>());
        ZTSHOWMGR_CONSTRUCTOR.original()(mgr as *const u32);
    }

    const SCRIPT_TYPE: u32 = 0x1357;
    let alloc = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
    let script_ptr = unsafe { ZTSHOWSCRIPT_CONSTRUCTOR.original()(alloc as *const u32, SCRIPT_TYPE, false) } as u32;
    if script_ptr == 0 {
        failures.push("ZTShowScript CONSTRUCTOR returned null".to_string());
    }

    // Null-show register: rejected before it ever reaches the embedded sub-object.
    if unsafe { ZTSHOWMGR_REGISTER_SCRIPT.original()(mgr as *const u32, std::ptr::null()) } != 0 {
        failures.push("REGISTER_SCRIPT(mgr, null) should return 0".to_string());
    }

    if unsafe { ZTSHOWMGR_REGISTER_SCRIPT.original()(mgr as *const u32, script_ptr as *const u32) } != 1 {
        failures.push("REGISTER_SCRIPT(mgr, script) should return 1".to_string());
    }

    let assigned_id = get_from_memory::<u16>(script_ptr + 0x4);
    if !ztshowscriptmgr::script_exists_by_id(assigned_id) {
        failures.push(format!("assigned id {assigned_id:#06x} should exist in the ztshowscriptmgr store after REGISTER_SCRIPT"));
    }

    let outer_handle = unsafe { ZTSHOWMGR_GET_SCRIPT.original()(mgr as *const u32, assigned_id) };
    let inner_handle = ztshowscriptmgr::get_script(assigned_id);
    if outer_handle == 0 || outer_handle != inner_handle {
        failures.push(format!(
            "GET_SCRIPT(mgr, {assigned_id:#06x}) should match ztshowscriptmgr::get_script and be non-null, got outer={outer_handle:#010x} inner={inner_handle:#010x}"
        ));
    }

    if unsafe { ZTSHOWMGR_UNREGISTER_SCRIPT.original()(mgr as *const u32, script_ptr as *const u32) } != 1 {
        failures.push("UNREGISTER_SCRIPT(mgr, script) should return 1".to_string());
    }
    if ztshowscriptmgr::script_exists_by_id(assigned_id) {
        failures.push(format!("id {assigned_id:#06x} should no longer exist in the store after UNREGISTER_SCRIPT"));
    }
    if unsafe { ZTSHOWMGR_GET_SCRIPT.original()(mgr as *const u32, assigned_id) } != 0 {
        failures.push(format!("GET_SCRIPT(mgr, {assigned_id:#06x}) should return 0 after unregister"));
    }

    if unsafe { ZTSHOWMGR_UNREGISTER_SCRIPT.original()(mgr as *const u32, script_ptr as *const u32) } != 0 {
        failures.push("double UNREGISTER_SCRIPT(mgr, script) should return 0".to_string());
    }

    finish_test(test_name, failures, failure_log)
}
/// ZTSHOWMGR_REAL_ZOO_STORE_CONSISTENCY_LIVE: diagnosing a real save-corruption report. Real vanilla
/// `ZTShowInfo::updateFromLoad` (`private/resources/decompiles/ZTShowInfo_updateFromLoad.c`) calls
/// `ZTShowMgr::registerShow(mgr, this, false)` for every show as the zoo loads, then - if applying
/// the loaded data changed `this`'s own id - `unregisterShow`s the *old* id. If
/// `ZTShowMgr::register_show`/`unregister_show` (the Rust ports) ever mishandle that dance, the
/// store would end up either missing a real show, or holding a stale entry (the same real
/// `show_addr` reachable under two different ids) - both invisible to the player (the game keeps
/// running normally) until the next save serializes whatever's now wrong into the file. Checks,
/// against every show `run_load_live_zoo` actually loaded:
/// 1. No two store entries share the same `show_addr` (a stale leftover from an old id).
/// 2. Every store entry's key equals the real, live object's own `field_0x70` id - i.e. the store
///    and the real `ZTShowInfo` it points at still agree on that show's id.
/// 3. The known show-tank habitat's real `ZTShowInfo*` ([`find_real_show_tank_habitat`]) is
///    registered in the store under its own real, live id.
pub(crate) fn run_ztshowmgr_real_zoo_store_consistency_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWMGR_REAL_ZOO_STORE_CONSISTENCY_LIVE";
    let mut fail_flag = false;

    let entries = ztshowmgr::all_registered_shows();
    if entries.is_empty() {
        info!("{}: no real shows registered from the loaded zoo - nothing to check, skipping", test_name);
        write_success_line(failure_log, &format!("{} (skipped: no real shows)", test_name));
        return false;
    }
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} entries={:?}\n", test_name, entries).as_bytes());
    }

    let mut seen_addrs = std::collections::HashSet::new();
    for &(id, addr) in &entries {
        if !seen_addrs.insert(addr) {
            error!("{}: show_addr {:#010x} is registered under more than one id (store={:?})", test_name, addr, entries);
            fail_flag = true;
        }
        let real_id = get_from_memory::<u16>(addr + 0x70);
        if real_id != id {
            error!(
                "{}: store key {:#x} points at show {:#010x} whose own live field_0x70 says id {:#x} - store/object disagree",
                test_name, id, addr, real_id
            );
            fail_flag = true;
        }
    }

    if let Some((_, show_info_ptr)) = find_real_show_tank_habitat() {
        let real_id = get_from_memory::<u16>(show_info_ptr + 0x70);
        match ztshowmgr::registered_show_for_id(real_id) {
            Some(addr) if addr == show_info_ptr => {}
            other => {
                error!(
                    "{}: known show-tank's real ZTShowInfo {:#010x} (id {:#x}) not found under that id in the store (got {:?})",
                    test_name, show_info_ptr, real_id, other
                );
                fail_flag = true;
            }
        }
    }

    if !fail_flag {
        info!("{}: {} real show(s) all consistent between the store and their live objects", test_name, entries.len());
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
