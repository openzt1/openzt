//! Compares real vanilla `Ambients`/`AmbientsGroup` against their Rust reimplementation (production
//! file `openzt/src/ambients.rs`): detour wiring check, a standalone construct/destruct byte-compare
//! for the no-config path, a live-zoo group-by-group comparison (name, range, and the borrowed
//! `SoundGroup*` itself) for the config-driven path, and a standalone `AmbientsGroup`-level
//! construction compare - see `openzt/plans/ztsoundscape-ambients-full-port-plan.md`'s stage 5.
//!
//! `Ambients::play`/`AmbientsGroup::play`'s own dispatch logic is deliberately not exercised by a
//! dedicated live comparison here: both are pure forwards with no return value and no field writes
//! (`ambients.rs`'s `play` doc comments), so the only thing a live call could observe is whether the
//! real, opaque `SoundGroup::play` was reached - unobservable without instrumenting that class, which
//! is out of scope (`ztsoundscape.rs`'s own `ZTSOUNDSCAPE_UPDATE` doc comment hits the identical wall
//! for its own `Ambients::play` calls and settles for a struct-only compare plus a "state changed"
//! sanity, for the same reason). The pure range-match/dispatch logic itself
//! ([`group_matches_level`]) is already unit-tested directly in `ambients.rs`.
//!
//! **Division of labor between the two group-level comparisons**: [`run_ambients_live_group_compare_test`]
//! verifies the *outer* `Ambients`-level loop (group count, name/range parsing per `"ambientlevels"`
//! entry) against real vanilla `Ambients::Ambients`. It does **not** independently verify
//! `AmbientsGroup::construct`'s own logic, despite comparing `sound_group` pointers across poles: real
//! vanilla `Ambients::Ambients`'s own internal call to `AmbientsGroup::AmbientsGroup` lands on the
//! *patched* address (this module's `ambients::init()` detours `ambientsgroup::CONSTRUCTOR` too), so
//! both of *this* test's poles actually execute the Rust [`AmbientsGroup::construct`] for the
//! group-building portion - the `sound_group` equality here is a sanity that the outer loop threads
//! data through correctly, not proof `AmbientsGroup::construct` itself matches vanilla.
//! [`run_ambientsgroup_standalone_compare_test`] is what independently verifies that: it calls
//! `AmbientsGroup::AmbientsGroup`'s own trampoline directly (not nested inside a different detoured
//! caller), so its real-vanilla pole is genuinely real vanilla.

use std::ffi::CStr;
use std::io::Write;

use openzt_detour::generated::bfconfigfile::{ATTEMPT_0, CONSTRUCTOR_0 as CONFIG_CONSTRUCTOR};
use openzt_detour::generated::bfscenariomgr::{GET_CROWD_AMBIENTS_NAME, GET_WORLD_AMBIENTS_NAME};
use tracing::{error, info};

use crate::ambients::{live_support as ambients_live_support, Ambients};
use crate::bfconfigfile::BFConfigFile;
use crate::globals::get_module_base;
use crate::reimplementation_tests::harness::write_success_line;
use crate::util::get_from_memory;

/// `AMBIENTS_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `ambients::init()`, and this asserts all five of its detours (`Ambients`'
/// `CONSTRUCTOR`/`PLAY`/`DESTRUCTOR`, `AmbientsGroup`'s `CONSTRUCTOR`/`PLAY`) actually report enabled.
/// Without it, a silently-failed `init_detours()` (error logged, game continues on vanilla) would leave
/// the battery green while `ztsoundscape.rs`'s direct Rust calls into these same methods look identical
/// either way - this is the only check in the battery that catches a missing-wiring gap directly,
/// per `CLAUDE.md`'s "live-test detours must be wired into `reimplementation_tests::init()`" warning.
pub(crate) fn run_ambients_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "AMBIENTS_DETOURS_ENABLED";
    let mut disabled: Vec<&'static str> = Vec::new();
    for (name, enabled) in ambients_live_support::detour_status() {
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

/// `AMBIENTS_STANDALONE_ROUNDTRIP` - builds two fresh `0x18`-byte standalone `Ambients` blocks, runs
/// the real vanilla constructor on one (via `ambients_live_support::real_construct`, a
/// `CONSTRUCTOR_DETOUR.call` trampoline - a release build's raw-cast `.original()` would re-enter the
/// Rust detour and degenerate this into Rust-vs-Rust) and [`Ambients::construct`] on the other, both
/// with a null `name` and a non-zero, non-trivial position, then byte-diffs the full `0x18` struct.
///
/// Unlike `ZTSoundscape::construct` (which leaves most of its `0x54` bytes untouched), the null-name
/// path of `Ambients::construct` writes every one of its `0x18` bytes - the three group-array dwords
/// (always `0` when `name` is null/empty, since `build_groups` never runs) and the `x`/`y`/`z` triple
/// read from `position` - so no pre-zeroing or masking is needed: a full match is the correct
/// expectation on every byte. No live zoo needed (doesn't touch any config file or the sound-device
/// singleton), so this runs in `always_late_tests()` alongside `ZTSOUNDSCAPE_STANDALONE_ROUNDTRIP`.
///
/// Teardown: both poles constructed with `name = null` never allocate a group array, so
/// `destruct`/the real destructor's group-freeing loop is a no-op on either - `destroy_real_ambients`/
/// `destroy_reimpl_ambients` still go through it for parity with every other standalone-roundtrip
/// test's teardown shape.
pub(crate) fn run_ambients_standalone_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "AMBIENTS_STANDALONE_ROUNDTRIP";

    let real_ptr = ambients_live_support::allocate_uninitialized_ambients();
    let reimpl_ptr = ambients_live_support::allocate_uninitialized_ambients();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        let msg = format!("OPERATOR_NEW returned null (real={:?}, reimpl={:?})", real_ptr, reimpl_ptr);
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        if !real_ptr.is_null() {
            ambients_live_support::destroy_real_ambients(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            ambients_live_support::destroy_reimpl_ambients(reimpl_ptr);
        }
        return true;
    }

    let struct_size = std::mem::size_of::<Ambients>();
    let position: [i32; 3] = [111, -222, 333];
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0xAA, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0xAA, struct_size);

        ambients_live_support::real_construct(real_ptr, std::ptr::null(), position.as_ptr());
        (*reimpl_ptr).construct(std::ptr::null(), position.as_ptr());
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

    ambients_live_support::destroy_real_ambients(real_ptr);
    ambients_live_support::destroy_reimpl_ambients(reimpl_ptr);
    failed
}

/// `GLOBAL_ZTScenarioMgr`'s global-slot RVA. Re-declared here per the repo's no-shared-consts
/// precedent - `ztgamemgr.rs`/`reimplementation_tests/tests/ztsoundscape.rs` carry their own copies.
const GLOBAL_ZTSCENARIOMGR_RVA: u32 = 0x00638ff8 - 0x400000;

/// One config-driven `Ambients` block's worth of group data, read back out of real vanilla memory by
/// raw offset (the fields themselves are module-private in `ambients.rs` - reading by offset matches
/// this codebase's existing convention for vanilla-layout structs in tests, e.g.
/// `reimplementation_tests/tests/ztsoundscape.rs`'s own `dword`/offset reads). `None` name = a null
/// group pointer at that slot (a failed `operator_new` inside `build_groups`).
struct GroupSnapshot {
    name: Option<Vec<u8>>,
    range_lo: i32,
    range_hi: i32,
    sound_group: u32,
}

/// Snapshots a single `AmbientsGroup` block's name/range/`sound_group` fields by raw offset - `None`
/// name for a null pointer (a failed `operator_new`).
fn snapshot_group(group_ptr: u32) -> GroupSnapshot {
    if group_ptr == 0 {
        return GroupSnapshot { name: None, range_lo: 0, range_hi: 0, sound_group: 0 };
    }
    let name = unsafe { CStr::from_ptr(group_ptr as *const i8) }.to_bytes().to_vec();
    GroupSnapshot {
        name: Some(name),
        range_lo: get_from_memory(group_ptr + 0x200),
        range_hi: get_from_memory(group_ptr + 0x204),
        sound_group: get_from_memory(group_ptr + 0x208),
    }
}

/// Reads the group-pointer array of an `Ambients` block at `ambients_ptr` (offsets `0x0`/`0x4`, per
/// `ambients.rs`'s struct doc comment) and snapshots every entry.
fn snapshot_groups(ambients_ptr: u32) -> Vec<GroupSnapshot> {
    let begin: u32 = get_from_memory(ambients_ptr);
    let end: u32 = get_from_memory(ambients_ptr + 4);
    let mut groups = Vec::new();
    let mut p = begin;
    while p != end {
        let group_ptr: u32 = get_from_memory(p);
        groups.push(snapshot_group(group_ptr));
        p += 4;
    }
    groups
}

/// One `(label, config_name)` pole run of `AMBIENTS_LIVE_GROUP_COMPARE`: builds a real-vanilla and a
/// Rust standalone `Ambients` against the same live config name/zero position, compares their group
/// lists field-by-field, runs a "play doesn't mutate" sanity on each, and tears both down. Appends
/// every mismatch (prefixed with `label`) into `mismatches`.
fn compare_one_ambients_name(label: &str, config_name: *const u8, mismatches: &mut Vec<String>) {
    let real_ptr = ambients_live_support::allocate_uninitialized_ambients();
    let reimpl_ptr = ambients_live_support::allocate_uninitialized_ambients();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        mismatches.push(format!("{label}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", real_ptr, reimpl_ptr));
        if !real_ptr.is_null() {
            ambients_live_support::destroy_real_ambients(real_ptr);
        }
        if !reimpl_ptr.is_null() {
            ambients_live_support::destroy_reimpl_ambients(reimpl_ptr);
        }
        return;
    }

    let struct_size = std::mem::size_of::<Ambients>();
    let position: [i32; 3] = [0, 0, 0];
    unsafe {
        std::ptr::write_bytes(real_ptr as *mut u8, 0xAA, struct_size);
        std::ptr::write_bytes(reimpl_ptr as *mut u8, 0xAA, struct_size);

        ambients_live_support::real_construct(real_ptr, config_name, position.as_ptr());
        (*reimpl_ptr).construct(config_name, position.as_ptr());
    }

    let real_groups = snapshot_groups(real_ptr as u32);
    let reimpl_groups = snapshot_groups(reimpl_ptr as u32);

    if real_groups.len() != reimpl_groups.len() {
        mismatches.push(format!("{label}: group count: real={}, reimpl={}", real_groups.len(), reimpl_groups.len()));
    } else {
        for (i, (real, reimpl)) in real_groups.iter().zip(reimpl_groups.iter()).enumerate() {
            match (&real.name, &reimpl.name) {
                (None, None) => mismatches.push(format!("{label}: group[{i}]: both poles built a null group pointer")),
                (Some(_), None) | (None, Some(_)) => {
                    mismatches.push(format!("{label}: group[{i}]: null-ness mismatch (real={:?}, reimpl={:?})", real.name.is_some(), reimpl.name.is_some()))
                }
                (Some(real_name), Some(reimpl_name)) => {
                    if real_name != reimpl_name {
                        mismatches.push(format!(
                            "{label}: group[{i}] name: real={:?}, reimpl={:?}",
                            String::from_utf8_lossy(real_name),
                            String::from_utf8_lossy(reimpl_name),
                        ));
                    }
                    if real.range_lo != reimpl.range_lo || real.range_hi != reimpl.range_hi {
                        mismatches.push(format!(
                            "{label}: group[{i}] ({:?}) range: real=({}, {}), reimpl=({}, {})",
                            String::from_utf8_lossy(real_name),
                            real.range_lo,
                            real.range_hi,
                            reimpl.range_lo,
                            reimpl.range_hi,
                        ));
                    }
                    // sound_group equality here is a sanity that the outer loop threads each entry's
                    // (name, range) through to the right AmbientsGroup, not independent proof of
                    // AmbientsGroup::construct's own correctness - see this file's module doc comment
                    // and run_ambientsgroup_standalone_compare_test, which verifies that directly.
                    if real.sound_group != reimpl.sound_group {
                        mismatches.push(format!(
                            "{label}: group[{i}] ({:?}) sound_group: real={:#010x}, reimpl={:#010x}",
                            String::from_utf8_lossy(real_name),
                            real.sound_group,
                            reimpl.sound_group,
                        ));
                    }
                }
            }
        }
    }

    // "play doesn't mutate" sanity: neither pole's play call may write back into its own Ambients
    // block (see this file's module doc comment on why play's dispatch itself isn't separately
    // compared). Level chosen from the first group's range when one exists, else an arbitrary miss.
    let level = real_groups.first().map(|g| g.range_lo).unwrap_or(0);
    let before_real = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) }.to_vec();
    let before_reimpl = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) }.to_vec();
    ambients_live_support::real_play(real_ptr, 1000, level);
    unsafe { (*reimpl_ptr).play(1000, level) };
    let after_real = unsafe { std::slice::from_raw_parts(real_ptr as *const u8, struct_size) };
    let after_reimpl = unsafe { std::slice::from_raw_parts(reimpl_ptr as *const u8, struct_size) };
    if before_real != after_real {
        mismatches.push(format!("{label}: real Ambients::play mutated its own block (level={level})"));
    }
    if before_reimpl != after_reimpl {
        mismatches.push(format!("{label}: reimpl Ambients::play mutated its own block (level={level})"));
    }

    ambients_live_support::destroy_real_ambients(real_ptr);
    ambients_live_support::destroy_reimpl_ambients(reimpl_ptr);
}

/// `AMBIENTS_LIVE_GROUP_COMPARE` - the config-driven counterpart to
/// `AMBIENTS_STANDALONE_ROUNDTRIP`: for both the crowd and world ambients config names off the live
/// `GLOBAL_ZTScenarioMgr` singleton (same getters `ZTSOUNDSCAPE_INIT`/`_UPDATE` use), builds a
/// real-vanilla and a Rust standalone `Ambients` against that name and compares every constructed
/// `AmbientsGroup`'s name/range/borrowed-`SoundGroup*`. Verifies the *outer* `Ambients`-level loop
/// (group count, name/range parsing per `"ambientlevels"` entry) - see this file's module doc comment
/// for why this does **not** also independently verify `AmbientsGroup::construct` itself (that's
/// `run_ambientsgroup_standalone_compare_test`'s job). Runs in `live_zoo_tests()`, needing the same
/// live scenario registry `ZTSOUNDSCAPE_INIT` gates on (pre-zoo it's non-null-but-uninitialized and
/// the config `attempt` would fail, silently covering only the empty-group path).
///
/// Both poles' `Ambients::construct` also parses the shared **global** `BFConfigFile` instances
/// `AmbientsGroup::construct` reads its own per-level sections from - but unlike `ZTSoundscape::init`
/// (whose crowd/world config instances are genuinely shared, forcing a snapshot-then-compare
/// ordering), each `Ambients::construct` call here opens and fully releases its own local, on-stack
/// `BFConfigFile` (see `Ambients::construct`'s doc comment) - there is no cross-pole aliasing hazard,
/// so both poles can be constructed and compared directly with no snapshot step.
pub(crate) fn run_ambients_live_group_compare_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "AMBIENTS_LIVE_GROUP_COMPARE";

    let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
    if scenariomgr_ptr == 0 {
        info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
        return false;
    }

    let crowd_ambients = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };
    let world_ambients = unsafe { GET_WORLD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };

    let mut mismatches: Vec<String> = Vec::new();
    compare_one_ambients_name("crowd", crowd_ambients, &mut mismatches);
    compare_one_ambients_name("world", world_ambients, &mut mismatches);

    if !mismatches.is_empty() {
        error!("{}: mismatch(es): {:?}", test_name, mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }
    !mismatches.is_empty()
}

/// One `(label, name, range_lo, range_hi)` sub-case of `AMBIENTSGROUP_STANDALONE_COMPARE`: builds a
/// real-vanilla and a Rust `AmbientsGroup` directly (not nested inside `Ambients::construct`) against
/// the same `(config, name, range_lo, range_hi)`, compares the name buffer's copied prefix,
/// `range_lo`/`range_hi`, and `sound_group`, then tears both down. Appends every mismatch (prefixed
/// with `label`) into `mismatches`.
fn compare_one_ambientsgroup(label: &str, config_ptr: *const u32, name_ptr: *const u8, range_lo: i32, range_hi: i32, mismatches: &mut Vec<String>) {
    let real_ptr = ambients_live_support::allocate_uninitialized_ambientsgroup();
    let reimpl_ptr = ambients_live_support::allocate_uninitialized_ambientsgroup();
    if real_ptr.is_null() || reimpl_ptr.is_null() {
        mismatches.push(format!("{label}: OPERATOR_NEW returned null (real={:?}, reimpl={:?})", real_ptr, reimpl_ptr));
        ambients_live_support::destroy_ambientsgroup(real_ptr);
        ambients_live_support::destroy_ambientsgroup(reimpl_ptr);
        return;
    }

    ambients_live_support::real_ambientsgroup_construct(real_ptr, config_ptr, name_ptr, range_lo, range_hi);
    unsafe { (*reimpl_ptr).construct(config_ptr, name_ptr, range_lo, range_hi) };

    let real = snapshot_group(real_ptr as u32);
    let reimpl = snapshot_group(reimpl_ptr as u32);

    let real_name = real.name.as_deref().unwrap_or(&[]);
    let reimpl_name = reimpl.name.as_deref().unwrap_or(&[]);
    if real_name != reimpl_name {
        mismatches.push(format!("{label}: name: real={:?}, reimpl={:?}", String::from_utf8_lossy(real_name), String::from_utf8_lossy(reimpl_name)));
    }
    if real.range_lo != reimpl.range_lo || real.range_hi != reimpl.range_hi {
        mismatches.push(format!("{label}: range: real=({}, {}), reimpl=({}, {})", real.range_lo, real.range_hi, reimpl.range_lo, reimpl.range_hi));
    }
    if real.sound_group != reimpl.sound_group {
        mismatches.push(format!("{label}: sound_group: real={:#010x}, reimpl={:#010x}", real.sound_group, reimpl.sound_group));
    }

    ambients_live_support::destroy_ambientsgroup(real_ptr);
    ambients_live_support::destroy_ambientsgroup(reimpl_ptr);
}

/// `AMBIENTSGROUP_STANDALONE_COMPARE` - the genuinely-independent counterpart to
/// `AMBIENTS_LIVE_GROUP_COMPARE` (see this file's module doc comment for why that test's own
/// `sound_group` comparison isn't independent proof of `AmbientsGroup::construct`'s correctness):
/// calls `AmbientsGroup::AmbientsGroup`'s own trampoline directly - not nested inside a different
/// detoured caller - against the Rust [`AmbientsGroup::construct`], for the same real
/// `(config, name, range_lo, range_hi)` inputs on both poles. Two sub-cases: a real level name off the
/// live crowd ambients config (matching config section present), and a synthetic name with no matching
/// section at all (chance/sound/prob all absent) - the all-keys-absent edge case the plan's own live
/// `cdb` capture observed but nothing previously tested.
///
/// Needs the same live `GLOBAL_ZTScenarioMgr` registry as `AMBIENTS_LIVE_GROUP_COMPARE` to open a real
/// config (pre-zoo it's non-null-but-uninitialized and `attempt` would fail), but otherwise doesn't
/// touch `Ambients` at all - runs in `live_zoo_tests()` alongside it for that reason.
pub(crate) fn run_ambientsgroup_standalone_compare_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "AMBIENTSGROUP_STANDALONE_COMPARE";

    let scenariomgr_ptr: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + GLOBAL_ZTSCENARIOMGR_RVA);
    if scenariomgr_ptr == 0 {
        info!("Skipping {}: GLOBAL_ZTScenarioMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTScenarioMgr not initialized)", test_name));
        return false;
    }

    let crowd_ambients_name = unsafe { GET_CROWD_AMBIENTS_NAME.original()(scenariomgr_ptr as i32) };

    // Real vanilla constructor + real teardown tail, same pattern `Ambients::construct` uses for its
    // own local scratch config (see `ambients.rs`'s `release_scratch_config` doc comment).
    let config = std::mem::MaybeUninit::<BFConfigFile>::uninit();
    let config_ptr = config.as_ptr() as *const u32;
    let kind_tag_seed: u8 = 0;
    unsafe { CONFIG_CONSTRUCTOR.original()(config_ptr, &kind_tag_seed as *const u8) };

    let attempted = unsafe { ATTEMPT_0.original()(config_ptr, crowd_ambients_name as *const i8) };
    if !attempted {
        ambients_live_support::release_scratch_config(config_ptr);
        info!("Skipping {}: unable to open the live crowd ambients config", test_name);
        write_success_line(failure_log, &format!("{} (skipped: config attempt failed)", test_name));
        return false;
    }

    let mut mismatches: Vec<String> = Vec::new();

    match ambients_live_support::first_ambient_level(config_ptr) {
        Some((name_ptr, range_lo, range_hi)) => {
            compare_one_ambientsgroup("real-level", config_ptr, name_ptr, range_lo, range_hi, &mut mismatches);
        }
        None => {
            info!("{}: no \"ambientlevels\" entries in the live crowd ambients config, skipping that sub-case", test_name);
        }
    }

    // Edge case: a level name with no matching config section at all - chance/sound/prob all absent.
    let missing_section_name = c"openzt_test_nonexistent_level";
    compare_one_ambientsgroup("missing-section", config_ptr, missing_section_name.as_ptr() as *const u8, 0, 999, &mut mismatches);

    ambients_live_support::release_scratch_config(config_ptr);

    if !mismatches.is_empty() {
        error!("{}: mismatch(es): {:?}", test_name, mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: mismatch(es): {:?}\n", test_name, mismatches).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }
    !mismatches.is_empty()
}
