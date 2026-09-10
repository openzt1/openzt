//! Compares real vanilla `ZTShowScriptMgr` behavior against its Rust reimplementation
//! (production file `openzt/src/ztshowscriptmgr.rs` - a full-replacement detour over an
//! independent Rust store, so these tests pin store content rather than diffing vanilla
//! memory): the wire-format save/load round-trips (early battery + real-zoo), the load
//! version gates, and the real `ZTShowScript` ctor's `auto_register` registration. Also owns
//! the `make_registered_show_script`/`add_matching_item` fixture helpers shared with the
//! `ztshowmgr`/`ztshow` test files.

use std::io::Write;

use tracing::{error, info};

use openzt_detour::generated::standalone;
use openzt_detour::generated::ztshowscript::CONSTRUCTOR as ZTSHOWSCRIPT_CONSTRUCTOR;

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::util::get_from_memory;
use crate::ztshowscriptmgr;
/// Builds a raw `ZTShowScriptItemRaw` (via `ztshowscriptmgr::live_support::raw_item_matching_type`)
/// and hands it to `ztshowscriptmgr::add_item`, for a script constructed via `make_registered_show_script`.
fn add_matching_item(script_ptr: u32, script_type: u32, trick_id: u16) {
    let item = ztshowscriptmgr::live_support::raw_item_matching_type(script_type, trick_id);
    ztshowscriptmgr::add_item(script_ptr, &item);
}

/// Registers one script directly via `ztshowscriptmgr::register_script` (the exact Rust function
/// Stage 1's `REGISTER_SCRIPT` detour itself calls into) and adds one matching-type item via
/// [`add_matching_item`]. Returns the assigned script id.
///
/// **Deliberately does not go through the real `ZTShowScript::ZTShowScript` ctor's own
/// `auto_register=true` path** (`ztshowscript::CONSTRUCTOR`) - not because that path is broken, but
/// because `register_script` directly is simpler for a helper called dozens of times across the
/// show-system test files' batteries. An earlier session found that calling the ctor live at *this* injection point
/// (before `run_load_live_zoo`) left the id it wrote back at `+0x4` unregistered in Stage 1's store,
/// and flagged it as an open, possibly-significant reimplementation gap. `ZTSHOWSCRIPT_CTOR_
/// REGISTRATION_LIVE` (this file, runs after `run_load_live_zoo`) resolved that: with a live
/// `GLOBAL_ZTShowMgr` (confirmed via `globals().ztshowmgr_ptr()`), the real ctor's `auto_register=true`
/// path registers correctly - the earlier finding was this harness's own early-injection-point timing,
/// per `ZTShowScript_ZTShowScript.c:25`'s `GLOBAL_ZTShowMgr != 0` guard, the same class of gap already
/// documented here for `GLOBAL_ZTGameMgr`. `ztshowui::copy_list_to_script`'s own identical ctor call
/// (the one real, confirmed production consumer of this exact path) is therefore genuinely safe, not
/// just assumed so - see that function's own doc comment for the pointer to this confirmation.
pub(crate) fn make_registered_show_script(script_type: u32, trick_id: u16) -> u16 {
    let alloc = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
    let id = ztshowscriptmgr::register_script(alloc, script_type).expect("register_script should never reject a non-null ctor_ptr");
    add_matching_item(alloc, script_type, trick_id);
    id
}

/// ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE: `ZTShowScriptMgr::save`/`load` (Stage 1's `SAVE`/`LOAD`
/// detours, `ztshowscriptmgr::save_mgr`/`load_mgr`) are, like `ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS`
/// above, full-replacement detours over an independent Rust store with no vanilla-layout struct to
/// diff against - but unlike those, `load_mgr`'s own *read* side (`read_item`/`read_script`) had zero
/// coverage before this: the existing `#[cfg(test)]` tests in `ztshowscriptmgr.rs` only pin
/// `encode_item`/`encode_mgr`'s byte offsets on the write side. Registers two scripts (one with two
/// items, one with one) via `make_registered_show_script`/`add_matching_item`, calls `SAVE`'s own
/// real, now-hooked address (`0x00479f44`) with `io_redirect` capturing the write, resets the store,
/// replays the captured bytes through `LOAD`'s own real, now-hooked address (`0x004c6ebd`), and
/// asserts every script/item field round-tripped. Uses a fixed literal version (`0x100`) comfortably
/// above all three save-format gates (`read_item`/`read_script` gate at `0x58`/`0x66`, the counter
/// restore gates at `0x60`) - no "current save version" constant exists elsewhere in this codebase to
/// reuse.
pub(crate) fn run_ztshowscriptmgr_save_load_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE";
    const CURRENT_VERSION: u32 = 0x100;

    ztshowscriptmgr::live_support::reset_state();
    const SCRIPT_TYPE_A: u32 = 11;
    const SCRIPT_TYPE_B: u32 = 22;
    let script_a = make_registered_show_script(SCRIPT_TYPE_A, 101);
    add_matching_item(ztshowscriptmgr::get_script(script_a), SCRIPT_TYPE_A, 102);
    let script_b = make_registered_show_script(SCRIPT_TYPE_B, 201);

    let mut fail_flag = false;
    let dummy_file: u32 = 0;

    let save_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, *const i8) -> u32>(0x00479f44u32) };
    io_redirect::begin_capture();
    save_hooked(&dummy_file as *const u32, &dummy_file as *const u32 as *const i8);
    let captured_bytes = io_redirect::end_capture();

    ztshowscriptmgr::live_support::reset_state();

    let load_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, *const u32, u32) -> u32>(0x004c6ebdu32) };
    io_redirect::begin_replay(captured_bytes);
    let load_ok = (load_hooked(&dummy_file as *const u32, &dummy_file as *const u32, CURRENT_VERSION) & 0xff) != 0;
    io_redirect::end_replay();

    if !load_ok {
        error!("{}: LOAD returned failure", test_name);
        fail_flag = true;
    }
    if !ztshowscriptmgr::script_exists_by_id(script_a) || !ztshowscriptmgr::script_exists_by_id(script_b) {
        error!("{}: one or both scripts missing after round-trip (a={}, b={})", test_name, script_a, script_b);
        fail_flag = true;
    }
    if ztshowscriptmgr::script_type_by_id(script_a) != Some(SCRIPT_TYPE_A) {
        error!("{}: script_a type mismatch after round-trip", test_name);
        fail_flag = true;
    }
    if ztshowscriptmgr::script_type_by_id(script_b) != Some(SCRIPT_TYPE_B) {
        error!("{}: script_b type mismatch after round-trip", test_name);
        fail_flag = true;
    }
    if ztshowscriptmgr::script_item_count_by_id(script_a) != 2 {
        error!("{}: script_a item count mismatch: expected 2, got {}", test_name, ztshowscriptmgr::script_item_count_by_id(script_a));
        fail_flag = true;
    }
    if ztshowscriptmgr::script_item_count_by_id(script_b) != 1 {
        error!("{}: script_b item count mismatch: expected 1, got {}", test_name, ztshowscriptmgr::script_item_count_by_id(script_b));
        fail_flag = true;
    }
    match ztshowscriptmgr::item_full_by_id(script_a, 0) {
        Some(item) if item.id == 101 && item.item_type == SCRIPT_TYPE_A => {}
        other => {
            error!("{}: script_a item 0 mismatch after round-trip: {:?}", test_name, other);
            fail_flag = true;
        }
    }
    match ztshowscriptmgr::item_full_by_id(script_a, 1) {
        Some(item) if item.id == 102 && item.item_type == SCRIPT_TYPE_A => {}
        other => {
            error!("{}: script_a item 1 mismatch after round-trip: {:?}", test_name, other);
            fail_flag = true;
        }
    }
    match ztshowscriptmgr::item_full_by_id(script_b, 0) {
        Some(item) if item.id == 201 && item.item_type == SCRIPT_TYPE_B => {}
        other => {
            error!("{}: script_b item 0 mismatch after round-trip: {:?}", test_name, other);
            fail_flag = true;
        }
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}

/// ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE: diagnosing a real save-corruption report (load a real
/// zoo, save, reload -> "corrupted saved game"/a capacity-overflow panic). Unlike
/// `ZTSHOWSCRIPTMGR_SAVE_LOAD_ROUNDTRIP_LIVE` above, which only ever round-trips two small synthetic
/// scripts, this snapshots whatever *real* scripts/items `run_load_live_zoo` just populated from
/// `reimplementation-test-zoo.zoo` (real string content, real field values - not the hand-built
/// matching-type-only items every other live test in this group uses), encodes them via
/// `ztshowscriptmgr::encode_mgr` (through `snapshot_encoded`, bypassing `WriteBytesToFile`/
/// `io_redirect` entirely - only the *read* side needs the hooked-address replay mechanism), decodes
/// them back via the real `load_mgr` (through `io_redirect::begin_replay`, since `read_bytes`
/// internally calls `DEALLOCATE.hooked()`), and asserts every script's type and every item's full
/// field set is byte-identical before/after. Registered first in `live_zoo_tests` (before any other
/// entry that adds/mutates scripts) so the snapshot reflects the zoo file's own as-loaded data, not
/// this battery's own synthetic additions.
pub(crate) fn run_ztshowscriptmgr_real_zoo_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE";
    const CURRENT_VERSION: u32 = 0x100;
    let mut fail_flag = false;

    let before_ids = ztshowscriptmgr::live_support::all_script_ids();
    if before_ids.is_empty() {
        info!("{}: no real scripts registered from the loaded zoo - nothing to round-trip, skipping", test_name);
        write_success_line(failure_log, &format!("{} (skipped: no real scripts)", test_name));
        return false;
    }

    fn snapshot(ids: &[u16]) -> Vec<(u16, Option<u32>, Vec<ztshowscriptmgr::ShowScriptItem>)> {
        ids.iter()
            .map(|&id| {
                let script_type = ztshowscriptmgr::script_type_by_id(id);
                let count = ztshowscriptmgr::script_item_count_by_id(id) as u16;
                let items = (0..count).filter_map(|i| ztshowscriptmgr::item_full_by_id(id, i)).collect();
                (id, script_type, items)
            })
            .collect()
    }

    let before = snapshot(&before_ids);
    let encoded = ztshowscriptmgr::live_support::snapshot_encoded();
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(
            format!("CHECKPOINT {} scripts={} encoded_len={}\n", test_name, before_ids.len(), encoded.len()).as_bytes(),
        );
    }

    let dummy_file: u32 = 0;
    let file_ptr = &dummy_file as *const u32;
    io_redirect::begin_replay(encoded);
    let load_ok = ztshowscriptmgr::load_mgr(file_ptr, CURRENT_VERSION);
    io_redirect::end_replay();

    if !load_ok {
        error!("{}: load_mgr returned failure re-decoding the real zoo's own encoded script data", test_name);
        fail_flag = true;
    }

    let after_ids = ztshowscriptmgr::live_support::all_script_ids();
    let after = snapshot(&after_ids);

    if before != after {
        error!("{}: real zoo script data did not round-trip byte-identically through encode_mgr/load_mgr", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: before={:?}\nafter={:?}\n", test_name, before, after).as_bytes());
        }
        fail_flag = true;
    }

    if !fail_flag {
        info!("{}: {} real script(s) round-tripped byte-identically", test_name, before_ids.len());
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}

/// ZTSHOWSCRIPTMGR_LOAD_VERSION_GATES_LIVE: `ztshowscriptmgr::load_mgr` is already `pub fn`, so unlike
/// the round-trip test above, this hand-builds byte buffers directly and calls it directly - no need
/// to go through the hooked address (real `SAVE` always writes the current format, so it can never
/// naturally produce an old-version stream). Exercises the format's version gates directly:
/// - `version <= 0x58`: the store is cleared but the stream is never read at all (empty buffer,
///   `load_mgr` still returns `true`).
/// - `0x58 < version <= 0x66`: an item's base fields are read but `normalHelpID`/`grayedHelpID`/icon
///   strings are left at [`crate::ztshowscriptmgr::ShowScriptItem::default`]'s values (that half of
///   the buffer is never written/read).
/// - `version > 0x60` (independent of the `0x66` gate above): the trailing `makeID` counter is read
///   and restored - checked via `ztshowscriptmgr::live_support::next_id_counter`; a version at/under
///   the gate with no trailing bytes still succeeds (never attempts the read), while a version over
///   the gate with the trailing bytes missing is a genuine short read and `load_mgr` returns `false`.
/// - A string length prefix `>= STRING_LENGTH_CAP` makes `load_mgr` return `false` immediately (the
///   same guard `read_string` applies to every string field).
pub(crate) fn run_ztshowscriptmgr_load_version_gates_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPTMGR_LOAD_VERSION_GATES_LIVE";
    let mut fail_flag = false;
    let dummy_file: u32 = 0;
    let file_ptr = &dummy_file as *const u32;

    macro_rules! check {
        ($cond:expr, $msg:expr) => {
            if !($cond) {
                error!("{}: {}", test_name, $msg);
                fail_flag = true;
            }
        };
    }

    // version <= 0x58: store cleared, stream never read, still reports success.
    ztshowscriptmgr::live_support::reset_state();
    let _ = make_registered_show_script(1, 1);
    io_redirect::begin_replay(Vec::new());
    let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x58);
    io_redirect::end_replay();
    check!(ok, "version<=0x58 should return true");
    check!(ztshowscriptmgr::live_support::registered_script_count() == 0, "version<=0x58 should clear the store");

    // 0x58 < version <= 0x66: base fields read, extended fields stay default. Also stays <= 0x60, so
    // no trailing counter bytes are needed/read.
    ztshowscriptmgr::live_support::reset_state();
    {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_le_bytes()); // script count
        buf.extend_from_slice(&55u16.to_le_bytes()); // script id
        buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
        buf.extend_from_slice(&9u32.to_le_bytes()); // script_type
        buf.extend_from_slice(&1u32.to_le_bytes()); // item count
        buf.push(1); // default_available
        buf.push(1); // visible
        buf.extend_from_slice(&77u16.to_le_bytes()); // id
        buf.extend_from_slice(&9u32.to_le_bytes()); // item_type
        buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
        for _ in 0..4 {
            buf.extend_from_slice(&0u32.to_le_bytes()); // name/anim/keeperPreTrick/keeperPostTrick, all empty
        }
        buf.extend_from_slice(&5u32.to_le_bytes()); // building
        buf.extend_from_slice(&3u32.to_le_bytes()); // complexity
        buf.push(0); // return_to_keeper
        buf.extend_from_slice(&10u32.to_le_bytes()); // satisfaction
        buf.extend_from_slice(&11u32.to_le_bytes()); // satisfaction_delta
        buf.extend_from_slice(&12u32.to_le_bytes()); // satisfaction_mirror
        buf.extend_from_slice(&13u32.to_le_bytes()); // minimum_depth
        io_redirect::begin_replay(buf);
        let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x60);
        io_redirect::end_replay();
        check!(ok, "0x58<version<=0x66 should return true");
        match ztshowscriptmgr::item_full_by_id(55, 0) {
            Some(item) => {
                check!(item.building == 5 && item.satisfaction == 10, "base fields should have been read");
                check!(
                    item.normal_help_id == 0 && item.grayed_help_id == 0 && item.normal_icon.is_empty() && item.grayed_icon.is_empty(),
                    "extended fields should stay at their default for version<=0x66"
                );
            }
            None => check!(false, "expected script 55 item 0 to exist after load"),
        }
    }

    // version > 0x60 with the trailing counter present: counter restored.
    ztshowscriptmgr::live_support::reset_state();
    {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0u32.to_le_bytes()); // 0 scripts
        buf.extend_from_slice(&0x1234u16.to_le_bytes()); // makeID counter
        io_redirect::begin_replay(buf);
        let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x70);
        io_redirect::end_replay();
        check!(ok, "version>0x60 with a trailing counter should return true");
        check!(ztshowscriptmgr::live_support::next_id_counter() == 0x1234, "counter should have been restored for version>0x60");
    }

    // version > 0x60 with the trailing counter bytes missing: a genuine short read, load_mgr fails.
    ztshowscriptmgr::live_support::reset_state();
    {
        let buf = 0u32.to_le_bytes().to_vec(); // 0 scripts, no counter bytes follow
        io_redirect::begin_replay(buf);
        let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x70);
        io_redirect::end_replay();
        check!(!ok, "version>0x60 missing its trailing counter bytes should return false");
    }

    // A string length prefix >= STRING_LENGTH_CAP fails immediately - no script/item header even
    // needed after it, `read_string` returns `None` before attempting to read the (absent) bytes.
    ztshowscriptmgr::live_support::reset_state();
    {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_le_bytes()); // script count
        buf.extend_from_slice(&1u16.to_le_bytes()); // script id
        buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
        buf.extend_from_slice(&1u32.to_le_bytes()); // script_type
        buf.extend_from_slice(&1u32.to_le_bytes()); // item count
        buf.push(0); // default_available
        buf.push(1); // visible
        buf.extend_from_slice(&1u16.to_le_bytes()); // id
        buf.extend_from_slice(&1u32.to_le_bytes()); // item_type
        buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // sentinel
        buf.extend_from_slice(&0x1000u32.to_le_bytes()); // name length prefix == STRING_LENGTH_CAP
        io_redirect::begin_replay(buf);
        let ok = ztshowscriptmgr::load_mgr(file_ptr, 0x70);
        io_redirect::end_replay();
        check!(!ok, "a string length prefix >= STRING_LENGTH_CAP should return false");
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
/// ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE: resolves the open question `make_registered_show_script`'s
/// own doc comment flags - whether the real `ZTShowScript::ZTShowScript` ctor's `auto_register=true`
/// path (`ztshowscript::CONSTRUCTOR`, `0x0059f837`, intentionally left un-detoured per
/// `ztshowscriptmgr.rs`'s module doc comment) actually reaches Stage 1's `REGISTER_SCRIPT` detour
/// (`0x0046e774`) and registers into the store, or whether the earlier "doesn't register" finding was
/// a harness-timing artifact of `GLOBAL_ZTShowMgr` not yet being resolved this early. Root cause per
/// `private/resources/decompiles/ZTShowScript_ZTShowScript.c:25`: the real ctor only calls
/// `ZTShowMgr::registerScript` when `GLOBAL_ZTShowMgr != 0` (`globals().ztshowmgr_ptr()`) - the same
/// class of "global not yet resolved at this early test-injection point" issue this file already
/// documents for `GLOBAL_ZTGameMgr` (see `run_ztscenariosimplegoal_eval_award_count_test`).
///
/// Skips gracefully (not a failure) if `GLOBAL_ZTShowMgr` is still null here, matching that same
/// convention. Otherwise allocates a real `0x14`-byte object (matching `ztshowui::
/// copy_list_to_script`'s own identical allocation) and calls the real, un-detoured ctor directly via
/// `.original()` with `auto_register=true`, then asserts the id it writes back at `ctor_ptr+0x4` is
/// genuinely present in Stage 1's store.
pub(crate) fn run_ztshowscript_ctor_registration_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE";

    if globals().ztshowmgr_ptr().is_null() {
        info!("Skipping {}: GLOBAL_ZTShowMgr not initialized at this injection point", test_name);
        write_success_line(failure_log, &format!("{} (skipped: ZTShowMgr not initialized)", test_name));
        return false;
    }

    const SCRIPT_TYPE: u32 = 0x7ace;
    let alloc = unsafe { standalone::OPERATOR_NEW.original()(0x14) } as u32;
    let ctor_ptr = unsafe { ZTSHOWSCRIPT_CONSTRUCTOR.original()(alloc as *const u32, SCRIPT_TYPE, true) } as u32;

    let mut fail_flag = false;
    if ctor_ptr == 0 {
        error!("{}: CONSTRUCTOR returned null", test_name);
        fail_flag = true;
    } else {
        let assigned_id = get_from_memory::<u16>(ctor_ptr + 0x4);
        if !ztshowscriptmgr::script_exists_by_id(assigned_id) {
            error!(
                "{}: ctor's auto_register=true path did NOT register id {} (ctor_ptr={:#010x}) into Stage 1's store - GLOBAL_ZTShowMgr was live, so this is a genuine reimplementation gap",
                test_name, assigned_id, ctor_ptr
            );
            fail_flag = true;
        } else {
            info!("{}: ctor's auto_register=true path correctly registered id {} into Stage 1's store", test_name, assigned_id);
        }
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
