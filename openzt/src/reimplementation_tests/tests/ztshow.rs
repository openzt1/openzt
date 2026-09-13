//! Compares real vanilla `ZTShow`/`ZTShowInfo` behavior against the Rust reimplementations in
//! production file `openzt/src/ztshow.rs` (which also owns `ZTShowInfo`): the pure-Rust
//! `get_show_script_state` reader, `ZTShowInfo::addScript`/`checkPendingScripts`, the
//! pending-script tree BST logic and its real-zoo integrity, the real save/load byte-count
//! diagnosis, `check_owning_habitat`, and the Group-3 trick path (`doCurrentItem`/
//! `validateItem`/`doTrickEvent`). The `find_real_*` habitat/unit scanners are shared with the
//! `ztshowmgr`/`ztshowui` test files.

use std::io::Write;

use tracing::{error, info};

use openzt_detour::generated::ztshow::GET_SHOW_SCRIPT_STATE;
use openzt_detour::generated::ztshowinfo;
use openzt_detour::generated::ztshowinfo::GET_NUM_UNITS as ZTSHOWINFO_GET_NUM_UNITS;

use crate::globals::globals;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, save_to_memory};
use crate::zthabitatmgr::ZTHabitat;
use crate::ztshow::{self, live_support as ztshow_live_support};
use crate::ztshowscriptmgr;
use crate::ztshowscriptstate::live_support as ztshowscriptstate_live_support;

use super::ztshowscriptmgr::make_registered_show_script;
/// `ZTSHOW_GET_SHOW_SCRIPT_STATE` - review follow-up: diffs the new pure Rust
/// [`ztshow::get_show_script_state`] reader against the real, never-hooked
/// `ztshow::GET_SHOW_SCRIPT_STATE.original()` (`0x0059eb99`) over synthetic `Box::leak`'d
/// fixtures (read-only on both sides, so no `standalone::OPERATOR_NEW` is needed). Covers an
/// empty tree (self-referential header, per the same trick `ZTSHOWMGR_IS_SHOW_SCRIPT_DONE`'s
/// fixture uses at `show_info+0x38`), a single node (exact hit, near misses either side, and a
/// probe crossing bit 16 that pins the 32-bit-width key compare - a wrongly 16-bit-masked
/// implementation would false-hit there), and a 3-node tree (root + left + right children, exact
/// hits on all three plus an in-between miss - pinning this is exact-match `find`, not a
/// nearest/lower-bound return).
pub(crate) fn run_ztshow_get_show_script_state_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOW_GET_SHOW_SCRIPT_STATE";
    let mut failures: Vec<String> = Vec::new();

    fn leaked_bytes(size: usize) -> u32 {
        let buf: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
        buf.as_mut_ptr() as u32
    }

    fn make_ztshow(header: u32) -> u32 {
        let addr = leaked_bytes(0x38);
        save_to_memory(addr + 0x34, header);
        addr
    }

    fn make_header(root: u32) -> u32 {
        let addr = leaked_bytes(0x18);
        save_to_memory(addr + 0x4, root);
        addr
    }

    fn make_node(key: u32, value: u32, left: u32, right: u32) -> u32 {
        let addr = leaked_bytes(0x18);
        save_to_memory(addr + 0x8, left);
        save_to_memory(addr + 0xc, right);
        save_to_memory(addr + 0x10, key);
        save_to_memory(addr + 0x14, value);
        addr
    }

    fn check(label: &str, ztshow_ptr: u32, key: u32, expected: u32, failures: &mut Vec<String>) {
        let rust_ret = ztshow::get_show_script_state(ztshow_ptr, key);
        if rust_ret != expected {
            failures.push(format!("{label}: rust pole should return {expected:#010x}, got {rust_ret:#010x}"));
        }
        let real_ret = unsafe { GET_SHOW_SCRIPT_STATE.original()(ztshow_ptr as *const u32, key) };
        if real_ret != expected {
            failures.push(format!("{label}: real pole should return {expected:#010x}, got {real_ret:#010x}"));
        }
        // Confirms the now-hooked real address itself (not just the internal helper directly) matches -
        // catches `ztshow::init()` going missing before it could produce a silent false positive above.
        let hooked_ret = unsafe { GET_SHOW_SCRIPT_STATE.hooked()(ztshow_ptr as *const u32, key) };
        if hooked_ret != expected {
            failures.push(format!("{label}: hooked address should return {expected:#010x}, got {hooked_ret:#010x}"));
        }
    }

    // Empty tree: the header field doubles as the header node itself (self-referential), so its
    // own root slot (at header+4, i.e. ztshow+0x38) is naturally 0 out of the zeroed allocation.
    let empty_show = leaked_bytes(0x40);
    save_to_memory(empty_show + 0x34, empty_show + 0x34);
    check("empty tree, key 0", empty_show, 0, 0, &mut failures);
    check("empty tree, key 0xffffffff", empty_show, 0xffff_ffff, 0, &mut failures);

    // Single node, key chosen above bit 16 to pin the 32-bit-width compare.
    const OPAQUE_VALUE: u32 = 0xdead_beef;
    let node = make_node(0x1_0007, OPAQUE_VALUE, 0, 0);
    let header = make_header(node);
    let show = make_ztshow(header);
    check("single node exact hit", show, 0x1_0007, OPAQUE_VALUE, &mut failures);
    check("single node near miss below", show, 0x1_0006, 0, &mut failures);
    check("single node near miss above", show, 0x1_0008, 0, &mut failures);
    check("single node low-16-bits-only match", show, 0x0007, 0, &mut failures);

    // 3-node tree: root + left + right children, exact hits plus an in-between miss.
    let left = make_node(5, 0x1111, 0, 0);
    let right = make_node(15, 0x3333, 0, 0);
    let root = make_node(10, 0x2222, left, right);
    let header3 = make_header(root);
    let show3 = make_ztshow(header3);
    check("3-node tree root hit", show3, 10, 0x2222, &mut failures);
    check("3-node tree left hit", show3, 5, 0x1111, &mut failures);
    check("3-node tree right hit", show3, 15, 0x3333, &mut failures);
    check("3-node tree in-between miss", show3, 7, 0, &mut failures);

    finish_test(test_name, failures, failure_log)
}
/// ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE: `ZTShowInfo::addScript`/`checkPendingScripts`
/// (`ztshowinfo::ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS`) are full-replacement detours over Stage 1's
/// independent `ZTShowScriptMgr` store, so - unlike `ZTAWARDMGR_SHOW_AWARDS`/
/// `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT` above, which each still have a real vanilla trampoline to
/// diff against - there's no real-vs-reimplementation diff oracle to compare against here at all (no
/// vanilla-layout struct backs Stage 1's store). Instead: builds a standalone `ZTShowInfo` (`ztshow_live_support::
/// build_standalone_show_info` - a zeroed `OPERATOR_NEW(0xb0)` buffer, **not** the real
/// `ZTShowInfo::ZTShowInfo` ctor, which unconditionally dereferences an unconfirmed `GLOBAL_ZTAIMgr`
/// field - see that helper's own doc comment), registers two real `ZTShowScript`s (via
/// [`make_registered_show_script`], each with one matching-type item so `has_items` is true for both),
/// then calls through `ADD_SCRIPT`'s and `CHECK_PENDING_SCRIPTS`'s own real, now-*hooked* addresses
/// directly (same "call the patched address via `transmute`" technique as
/// `ZTSCENARIOSIMPLEGOAL_EVAL_AWARD_COUNT` above) and asserts the resulting state directly in the
/// standalone buffer's own pending-scripts-tree memory and in Stage 1's store.
///
/// Runs after `run_load_live_zoo` for parity with the other real-zoo-dependent tests, though this one
/// deliberately doesn't actually depend on `GLOBAL_ZTGameMgr` being live - see the next paragraph.
///
/// **Real, live-crash-reproducing finding from this test's first run**: `GLOBAL_ZTGameMgr` is *still*
/// null at every injection point in this test battery, confirmed directly by `ZTSCENARIOSIMPLEGOAL_
/// EVAL_AWARD_COUNT`'s own "(skipped: ZTGameMgr not initialized)" log line appearing even *after*
/// `run_load_live_zoo` (`run_load_live_zoo`'s `FOPEN`/`LOAD_FILE`/`FCLOSE` sequence loads the world/
/// habitat data directly, bypassing the normal scenario-start flow that would otherwise construct
/// `ZTGameMgr`). `add_script`'s `was_inserted` branch calls `GET_DATE.original()(globals().
/// ztgamemgr_ptr(), ...)` unconditionally on a first-ever insert - matching real vanilla `addScript`'s
/// own decompile exactly (`ZTShowInfo_addScript.c`'s `ZTGameMgr::getDate(GLOBAL_ZTGameMgr, ...)`, also
/// unconditional) - which crashed this whole test process outright the first time this test actually
/// ran (no earlier stage-2 live test had ever exercised a first-ever pending-scripts insert before, so
/// this went undetected until now). Real vanilla's own lack of a null check isn't a vanilla bug - a
/// real game session always has a live `ZTGameMgr` by the time a habitat can have a show, `addScript`
/// can be called at all - it's specifically this test harness's own early injection point that can
/// reach `addScript` before `ZTGameMgr` exists. Worked around here (not "fixed" in `ztshow.rs`, since
/// there's nothing wrong with the reimplementation) by pre-inserting the pending-scripts node directly
/// via `find_or_insert_pending_script_node` *before* calling the hooked `ADD_SCRIPT`, so its own
/// internal `find_or_insert` call finds an existing node (`was_inserted == false`) and never reaches
/// the `GET_DATE` call at all.
///
/// **Second, independent bug found and fixed while building this test** (see `ztshow.rs`'s
/// `find_or_insert_pending_script_node` for the full writeup): that function used to also maintain a
/// "rightmost" cache at the pending-scripts tree header's `+0xc`, which actually aliases `ZTShowInfo`'s
/// own real `addShow`/`removeShow` dynamic array's `begin` pointer (`+0x50`) - corrupting it on every
/// first-ever insert for a given `ZTShowInfo`, which would crash the very next real `ADD_SHOW`/
/// `REMOVE_SHOW` call (an unbounded scan against a corrupted `begin`/still-null `end`). This is a
/// genuine, previously-unexercised live gameplay-corruption bug - Stage 2's `addScript`/
/// `checkPendingScripts` had no live test until now (the plan's own open item 11) - fixed by dropping
/// the "rightmost" cache concept entirely (no real vanilla consumer of it was ever found).
pub(crate) fn run_ztshowinfo_add_script_check_pending_scripts_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE";

    let show_info = ztshow_live_support::build_standalone_show_info();
    const UNIT_TYPE_ID: u32 = 0x7fff_1234;
    const SCRIPT_TYPE: u32 = 7;

    // Pre-insert the pending-scripts node ourselves - see this function's own doc comment on why
    // `ADD_SCRIPT`'s own internal insert can't be allowed to run with GLOBAL_ZTGameMgr still null.
    let _ = ztshow::find_or_insert_pending_script_node(show_info, UNIT_TYPE_ID);

    let script_a = make_registered_show_script(SCRIPT_TYPE, 1);
    let script_b = make_registered_show_script(SCRIPT_TYPE, 2);

    let mut fail_flag = false;

    // Call through ADD_SCRIPT's own real, now-hooked address directly (0x0046e8b5, ztshowinfo::ADD_SCRIPT).
    let add_script_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, u32, u16) -> bool>(0x0046e8b5u32) };
    let add_ok = add_script_hooked(show_info as *const u32, UNIT_TYPE_ID, script_a);
    if !add_ok {
        error!("{}: ADD_SCRIPT returned false", test_name);
        fail_flag = true;
    }

    // The very first insert for UNIT_TYPE_ID becomes the pending-scripts tree's root directly.
    let header = get_from_memory::<u32>(show_info + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    if root == 0 {
        error!("{}: pending-scripts tree has no root after ADD_SCRIPT", test_name);
        fail_flag = true;
    } else {
        let current = get_from_memory::<u16>(root + 0x1c);
        let pending = get_from_memory::<u16>(root + 0x1e);
        if current != script_a {
            error!("{}: expected current={}, got {} after ADD_SCRIPT", test_name, script_a, current);
            fail_flag = true;
        }
        if pending != 0xffff {
            error!("{}: expected pending reset to 0xffff after ADD_SCRIPT, got {:#x}", test_name, pending);
            fail_flag = true;
        }
        if !ztshowscriptmgr::script_exists_by_id(script_a) {
            error!("{}: script_a {} should exist in the store after ADD_SCRIPT", test_name, script_a);
            fail_flag = true;
        }

        // Simulate a queued pending change (the same state `add_script` itself would leave behind if
        // the show were already started - simpler/more direct to poke it here than to also fake
        // `isStarted()`'s own real precondition chain) and exercise CHECK_PENDING_SCRIPTS through its
        // own real, now-hooked address (0x005a876a, ztshowinfo::CHECK_PENDING_SCRIPTS).
        save_to_memory(root + 0x1e, script_b);
        let check_pending_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32)>(0x005a876au32) };
        check_pending_hooked(show_info as *const u32);

        let current_after = get_from_memory::<u16>(root + 0x1c);
        let pending_after = get_from_memory::<u16>(root + 0x1e);
        if current_after != script_b {
            error!("{}: expected current={} after CHECK_PENDING_SCRIPTS, got {}", test_name, script_b, current_after);
            fail_flag = true;
        }
        if pending_after != 0xffff {
            error!("{}: expected pending reset to 0xffff after CHECK_PENDING_SCRIPTS, got {:#x}", test_name, pending_after);
            fail_flag = true;
        }
        if ztshowscriptmgr::script_exists_by_id(script_a) {
            error!("{}: old current script_a {} should have been dropped from the store after CHECK_PENDING_SCRIPTS", test_name, script_a);
            fail_flag = true;
        }
        if !ztshowscriptmgr::script_exists_by_id(script_b) {
            error!("{}: script_b {} should still exist in the store after CHECK_PENDING_SCRIPTS", test_name, script_b);
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

/// ZTSHOWINFO_PENDING_SCRIPT_TREE_STRESS_LIVE: stress-tests `find_or_insert_pending_script_node`'s BST
/// insert logic (`ztshow.rs` - the exact function `ztshowscriptmgr-open-items.md`'s bug 1, the phantom
/// "rightmost" cache corruption, was found and fixed in) against a real, standalone `ZTShowInfo`
/// (`ztshow_live_support::build_standalone_show_info`). Generates a fixed-seed-shuffled sequence of
/// distinct `unit_type_id`s (a trivial inline xorshift32, no new crate dependency), inserts each in
/// turn, and after every insert asserts the header's `leftmost` cache (`+0x8`) matches the running
/// minimum key inserted so far - the exact invariant bug 1 violated. Finishes by re-inserting every id
/// a second time (asserting `was_inserted == false` and the same node address each time) and a full
/// in-order walk (`ztshow_live_support::collect_pending_script_nodes`) asserting strictly ascending key
/// order.
pub(crate) fn run_ztshowinfo_pending_script_tree_stress_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_PENDING_SCRIPT_TREE_STRESS_LIVE";
    let show_info = ztshow_live_support::build_standalone_show_info();

    // Trivial fixed-seed xorshift32 - deterministic across runs, no new crate dependency.
    let mut state: u32 = 0x9e3779b9;
    let mut next_rand = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };

    const COUNT: usize = 256;
    // Distinct unit_type_ids in a small range, Fisher-Yates shuffled via the xorshift generator above.
    let mut ids: Vec<u32> = (1..=COUNT as u32).collect();
    for i in (1..ids.len()).rev() {
        let j = (next_rand() as usize) % (i + 1);
        ids.swap(i, j);
    }

    let mut fail_flag = false;
    let mut min_seen: Option<u32> = None;
    let mut node_by_id: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();

    for &id in &ids {
        let (node, was_inserted) = ztshow::find_or_insert_pending_script_node(show_info, id);
        if !was_inserted {
            error!("{}: first insert of id {} unexpectedly reported was_inserted=false", test_name, id);
            fail_flag = true;
        }
        node_by_id.insert(id, node);
        min_seen = Some(min_seen.map_or(id, |m| m.min(id)));

        let header = get_from_memory::<u32>(show_info + 0x44);
        let leftmost = get_from_memory::<u32>(header + 8);
        let leftmost_key = get_from_memory::<u32>(leftmost + 0x10);
        if leftmost_key != min_seen.unwrap() {
            error!(
                "{}: after inserting id {}, leftmost cache key is {} but running minimum is {}",
                test_name, id, leftmost_key, min_seen.unwrap()
            );
            fail_flag = true;
        }
    }

    // Re-inserting every id should now be a no-op find, returning the same node and was_inserted=false.
    for &id in &ids {
        let (node, was_inserted) = ztshow::find_or_insert_pending_script_node(show_info, id);
        if was_inserted {
            error!("{}: re-inserting already-seen id {} reported was_inserted=true", test_name, id);
            fail_flag = true;
        }
        let expected = node_by_id.get(&id).copied().unwrap_or(0);
        if node != expected {
            error!("{}: re-inserting id {} returned a different node address ({:#010x} vs {:#010x})", test_name, id, node, expected);
            fail_flag = true;
        }
    }

    let in_order = ztshow_live_support::collect_pending_script_nodes(show_info);
    if in_order.len() != COUNT {
        error!("{}: in-order walk found {} nodes, expected {}", test_name, in_order.len(), COUNT);
        fail_flag = true;
    }
    let mut prev_key: Option<u32> = None;
    for &node in &in_order {
        let key = get_from_memory::<u32>(node + 0x10);
        if let Some(prev) = prev_key
            && key <= prev
        {
            error!("{}: in-order walk not strictly ascending: {} then {}", test_name, prev, key);
            fail_flag = true;
            break;
        }
        prev_key = Some(key);
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
/// Scans the live, loaded test zoo's real habitats (`globals().zthabitatmgr().exhibit_array()`) for
/// one that is both a real tank exhibit (`ZTHabitat::is_tank`) **with water** (`water_level() > 0`) -
/// i.e. NOT `ztshow::check_owning_habitat`'s blocking predicate - **and** already has a real
/// `ZTShowInfo*` attached (`ZTHabitat::is_show_tank`), i.e. a genuinely-configured, already-working
/// show tank that real vanilla would let a show start on. Returns `(habitat_ptr, show_info_ptr)` for
/// the first match, `None` if the test zoo has none.
/// TEMPORARY diagnostic - dumps every real habitat's `is_tank`/`is_show_tank`/`water_level`/`getSize`
/// and the [`find_real_show_tank_habitat`] result, to determine whether a show-tank exists right after
/// load (before any `ZTHABITAT_*` tile-list test runs) - remove once the "no qualifying show-tank
/// habitat found" investigation is done.
pub(crate) fn run_diag_show_tank_probe_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "DIAG_SHOW_TANK_PROBE";
    let habitat_mgr = globals().zthabitatmgr();
    let exhibits = habitat_mgr.exhibit_array();
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} exhibit_count={}\n", test_name, exhibits.len()).as_bytes());
        for i in 0..exhibits.len() {
            let ptr = exhibits.get_ptr(i);
            if ptr == 0 {
                continue;
            }
            let habitat = get_from_memory::<ZTHabitat>(ptr);
            let is_tank = habitat.is_tank();
            let is_show_tank = habitat.is_show_tank();
            let water = if is_tank {
                let tank = get_from_memory::<crate::zthabitatmgr::ZTTankExhibit>(ptr);
                *tank.water_level()
            } else {
                0
            };
            let size = unsafe { openzt_detour::generated::zthabitat::GET_SIZE.original()(ptr as *const u32, false) };
            let _ = log_file.write_all(
                format!(
                    "CHECKPOINT {} habitat {} ({:#010x}) is_tank={} is_show_tank={} water_level={} size={}\n",
                    test_name, i, ptr, is_tank, is_show_tank, water, size
                )
                .as_bytes(),
            );
        }
        let found = find_real_show_tank_habitat();
        let _ = log_file.write_all(format!("CHECKPOINT {} find_real_show_tank_habitat()={:?}\n", test_name, found).as_bytes());
    }
    write_success_line(failure_log, test_name);
    false
}

pub(crate) fn find_real_show_tank_habitat() -> Option<(u32, u32)> {
    let habitat_mgr = globals().zthabitatmgr();
    let exhibits = habitat_mgr.exhibit_array();
    for i in 0..exhibits.len() {
        let habitat_ptr = exhibits.get_ptr(i);
        if habitat_ptr == 0 {
            continue;
        }
        let habitat = get_from_memory::<ZTHabitat>(habitat_ptr);
        if habitat.is_tank() && habitat.is_show_tank() {
            // Only safe to read as a `ZTTankExhibit` because `is_tank()` above already confirmed
            // this pointer is really a 0x1e8-byte `ZTTankExhibit`, not a plain 0x178-byte `ZTHabitat`.
            let tank = get_from_memory::<crate::zthabitatmgr::ZTTankExhibit>(habitat_ptr);
            if *tank.water_level() > 0 {
                return Some((habitat_ptr, *habitat.zt_show_info_ptr()));
            }
        }
    }
    None
}

/// ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE: diagnosing a real save-corruption report.
/// The pending-scripts BST at the known show-tank's `ZTShowInfo+0x44`
/// (`ztshow::find_or_insert_pending_script_node`) is real, live vanilla memory this crate's code
/// writes to directly (unlike `ZTShowScriptMgr`'s independent store, already proven clean by
/// `ZTSHOWSCRIPTMGR_REAL_ZOO_ROUNDTRIP_LIVE` above) - a corrupted node/cache here would silently
/// keep the game running (nothing reads it except real, un-reimplemented `checkPendingScripts`/
/// `enterNewMonth`/etc.) until the next save serializes it. Bounded-iteration walk (matching this
/// codebase's own "diagnose a BST before trusting it" convention - see
/// `find_trick_by_id`'s doc comment for the prior real bug this exact style of check caught) over
/// whatever real tree `run_load_live_zoo` already populated: collects every node via `left`(`+8`)/
/// `right`(`+0xc`), asserting (1) the walk terminates within a generous bound (no cycle), (2) an
/// in-order traversal's keys (`+0x10`) come out strictly ascending (the BST invariant, not just "no
/// cycle"), and (3) the header's own leftmost cache (`+0x8`) - what real, un-reimplemented
/// `enterNewMonth`/`checkPendingScripts` start their own walk from - equals the address of whichever
/// node the walk found with the smallest key (the exact invariant a previous version of this
/// function's cache-maintenance code broke, per `find_or_insert_pending_script_node`'s own doc
/// comment).
pub(crate) fn run_ztshow_pending_script_tree_real_zoo_integrity_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE";
    let mut fail_flag = false;

    let Some((_, show_info_ptr)) = find_real_show_tank_habitat() else {
        error!("{}: BLOCKED - no real show-tank habitat found in test zoo", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no qualifying show-tank habitat found\n", test_name).as_bytes());
        }
        return false;
    };

    let header = get_from_memory::<u32>(show_info_ptr + 0x44);
    let root = get_from_memory::<u32>(header + 4);

    const MAX_NODES: usize = 10_000;
    let mut in_order: Vec<(u32, u32)> = Vec::new(); // (addr, key)
    let mut stack: Vec<u32> = Vec::new();
    let mut node = root;
    let mut iterations = 0usize;
    // Standard iterative in-order walk: push left spine, visit, descend right.
    while (node != 0 && node != header) || !stack.is_empty() {
        iterations += 1;
        if iterations > MAX_NODES {
            error!("{}: walk exceeded {} iterations without terminating - likely a cycle in the tree", test_name, MAX_NODES);
            fail_flag = true;
            break;
        }
        if node != 0 && node != header {
            stack.push(node);
            node = get_from_memory::<u32>(node + 8); // left
        } else if let Some(top) = stack.pop() {
            let key = get_from_memory::<u32>(top + 0x10);
            in_order.push((top, key));
            node = get_from_memory::<u32>(top + 0xc); // right
        }
    }

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} header={:#010x} root={:#010x} node_count={}\n", test_name, header, root, in_order.len()).as_bytes());
    }

    if !fail_flag {
        for pair in in_order.windows(2) {
            if pair[0].1 >= pair[1].1 {
                error!(
                    "{}: in-order keys not strictly ascending ({:#x} @ {:#010x} then {:#x} @ {:#010x}) - BST invariant violated",
                    test_name, pair[0].1, pair[0].0, pair[1].1, pair[1].0
                );
                fail_flag = true;
            }
        }
    }

    if !fail_flag && !in_order.is_empty() {
        let real_leftmost = get_from_memory::<u32>(header + 8);
        let expected_leftmost = in_order[0].0; // smallest key, since in-order is ascending
        if real_leftmost != expected_leftmost {
            error!(
                "{}: header leftmost cache is {:#010x} but the smallest real key ({:#x}) lives at {:#010x} - stale cache (the class of bug find_or_insert_pending_script_node's own doc comment already found once)",
                test_name, real_leftmost, in_order[0].1, expected_leftmost
            );
            fail_flag = true;
        }
    }

    if !fail_flag {
        info!("{}: pending-script tree ({} real node(s)) is well-formed and leftmost cache is correct", test_name, in_order.len());
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
/// ZTSHOWINFO_REAL_SAVE_LOAD_BYTE_COUNT_LIVE: diagnosing a real save-corruption report. Real, un-
/// reimplemented `ZTShowInfo::save`/`load` (`ztshowinfo::SAVE`/`LOAD`) walk the pending-scripts tree
/// at `ZTShowInfo+0x44` - the same tree `ztshow.rs`'s `find_or_insert_pending_script_node`/
/// `allocate_pending_script_node` builds. This tests the real, un-reimplemented pair directly against
/// each other on the real show-tank habitat's real `ZTShowInfo` (already carrying 3 real pending-
/// script nodes from actual gameplay, per `ZTSHOW_PENDING_SCRIPT_TREE_REAL_ZOO_INTEGRITY_LIVE` above -
/// deliberately not a synthetic/standalone object, which would confound the result with zeroed-out
/// unrelated fields `ZTShowInfo::save` also reads): captures real `SAVE`'s output
/// (`io_redirect::begin_capture`), then real-`LOAD`s those exact bytes back into the same live object
/// (`io_redirect::begin_replay`), and compares `io_redirect::replay_position()` (bytes `LOAD` actually
/// consumed) against the captured buffer's own length (bytes `SAVE` actually wrote). A mismatch here
/// pinpoints a genuine save/load byte-count asymmetry for this exact real data - and since both `SAVE`
/// and `LOAD` are real, untouched vanilla code, a mismatch would mean our own node construction
/// (`allocate_pending_script_node`'s simplified `+0x18` sub-structure, standing in for whatever real
/// vanilla's own node constructor builds there) makes vanilla's real save/load disagree about how much
/// data it wrote - not a defect in vanilla's own save/load pairing itself. Uses `version=106` (`0x6a`)
/// - not an arbitrary/future value - to match the exact version boundary a real save actually uses
///   (confirmed live via `DIAG LOAD_ENTER ZTShowMgr version=106` this session), since some of
///   `ZTShowInfo::load`'s per-field reads are version-gated and a different version would exercise a
///   different, non-representative code path. Mutates the live show-tank's `ZTShowInfo` in place (real
///   `LOAD` writes directly into it) - acceptable since this is a one-shot test process that exits after
///   the battery, matching the battery's own established precedent elsewhere.
pub(crate) fn run_ztshowinfo_real_save_load_byte_count_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOWINFO_REAL_SAVE_LOAD_BYTE_COUNT_LIVE";
    let mut fail_flag = false;

    let Some((_, show_info_ptr)) = find_real_show_tank_habitat() else {
        error!("{}: BLOCKED - no real show-tank habitat found in test zoo", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no qualifying show-tank habitat found\n", test_name).as_bytes());
        }
        return false;
    };

    const REAL_VERSION: u32 = 106;
    let dummy_file: u32 = 0;

    io_redirect::begin_capture();
    let save_ok = unsafe { ztshowinfo::SAVE.original()(show_info_ptr as *const u32, &dummy_file as *const u32 as *const i8) };
    let captured_bytes = io_redirect::end_capture();

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(
            format!("CHECKPOINT {} show_info={:#010x} save_ok={} bytes_written={}\n", test_name, show_info_ptr, save_ok, captured_bytes.len())
                .as_bytes(),
        );
    }
    if (save_ok & 0xff) == 0 {
        error!("{}: real ZTShowInfo::save returned failure", test_name);
        fail_flag = true;
    }

    let written_len = captured_bytes.len();
    io_redirect::begin_replay(captured_bytes);
    let load_ok = unsafe { ztshowinfo::LOAD.original()(show_info_ptr as *const u32, &dummy_file as *const u32, REAL_VERSION) };
    let consumed_len = io_redirect::replay_position();
    io_redirect::end_replay();

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(
            format!("CHECKPOINT {} load_ok={} bytes_consumed={} bytes_written={}\n", test_name, load_ok, consumed_len, written_len).as_bytes(),
        );
    }
    if load_ok == 0 {
        error!("{}: real ZTShowInfo::load returned failure replaying its own save's bytes", test_name);
        fail_flag = true;
    }
    if consumed_len != written_len {
        error!(
            "{}: byte-count mismatch - real save() wrote {} bytes but real load() consumed {} bytes ({}) for the same real ZTShowInfo",
            test_name,
            written_len,
            consumed_len,
            if consumed_len > written_len { "load read PAST what save wrote" } else { "load read LESS than save wrote" }
        );
        fail_flag = true;
    }

    if !fail_flag {
        info!("{}: real save()/load() agree exactly on {} bytes for the real show-tank's ZTShowInfo", test_name, written_len);
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
/// Scans for a real habitat that *does* satisfy `check_owning_habitat`'s blocking predicate (a real
/// tank exhibit with zero water level) - lets `ZTSHOW_CHECK_OWNING_HABITAT_LIVE` exercise the blocking
/// path against real `GLOBAL_ZTHabitatMgr`-owned memory too, not just the "should proceed" one.
fn find_real_empty_tank_habitat() -> Option<u32> {
    let habitat_mgr = globals().zthabitatmgr();
    let exhibits = habitat_mgr.exhibit_array();
    for i in 0..exhibits.len() {
        let habitat_ptr = exhibits.get_ptr(i);
        if habitat_ptr == 0 {
            continue;
        }
        let habitat = get_from_memory::<ZTHabitat>(habitat_ptr);
        if habitat.is_tank() {
            // Only safe to read as a `ZTTankExhibit` because `is_tank()` above already confirmed
            // this pointer is really a 0x1e8-byte `ZTTankExhibit`, not a plain 0x178-byte `ZTHabitat`.
            let tank = get_from_memory::<crate::zthabitatmgr::ZTTankExhibit>(habitat_ptr);
            if *tank.water_level() == 0 {
                return Some(habitat_ptr);
            }
        }
    }
    None
}

/// ZTSHOW_CHECK_OWNING_HABITAT_LIVE: `ztshow::check_owning_habitat` (factored out of `ZTShow::start`'s
/// own inlined `checkOwningHabitat` logic specifically so it could be live-tested directly - see its
/// own doc comment in `ztshow.rs`) is exercised here against real `GLOBAL_ZTHabitatMgr`-owned habitat
/// memory (Route A from the implementation plan, preferred over hand-building a fake `ZTHabitat` with
/// a copied vtable pointer) wrapped in a small, local, stack-allocated stand-in for a `ZTShowInfo*`
/// (only `+0xa0`, the habitat back-pointer `check_owning_habitat` reads, needs to be populated -
/// unlike `ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS` above, `check_owning_habitat` touches no other field,
/// so there's no need for `ztshow_live_support::build_standalone_show_info`'s full `0xb0` buffer or
/// its allocator-lifetime concerns here).
///
/// `start`'s own full pipeline (which is what actually calls `check_owning_habitat` in real gameplay)
/// isn't exercised end-to-end here: reaching the habitat check via the real `START` entry point needs
/// `RESOLVE_NEXT_SCHEDULED_SCRIPT_ID` to already return a genuinely-scheduled real script id first,
/// which depends on `ZTShow`'s own scheduling-vector data - a structure this plan never reverse
/// -engineered (out of scope for this session, flagged as a residual gap in the plan doc). Testing
/// `check_owning_habitat` directly sidesteps that gap entirely while still exercising the exact logic
/// `start` relies on, against real habitat memory.
pub(crate) fn run_ztshow_check_owning_habitat_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOW_CHECK_OWNING_HABITAT_LIVE";

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} entry\n", test_name).as_bytes());
    }

    let Some((qualifying_habitat_ptr, real_show_info)) = find_real_show_tank_habitat() else {
        info!("Skipping {}: no real tank habitat with water_level()>0 and a real ZTShowInfo* attached found in test zoo", test_name);
        write_success_line(failure_log, &format!("{} (skipped: no qualifying real show-tank habitat found)", test_name));
        return false;
    };

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} found qualifying_habitat={:#010x} real_show_info={:#010x}\n", test_name, qualifying_habitat_ptr, real_show_info).as_bytes());
    }

    let mut fail_flag = false;

    // Positive case: a real, working show tank (has water) must NOT be blocked - check_owning_habitat
    // mirrors vanilla's checkOwningHabitat returning its blocking code only for an *empty* tank, so a
    // filled one must return false here (see ztshow.rs's check_owning_habitat doc comment).
    let mut positive_buf = [0u8; 0xa4];
    let positive_show_info = positive_buf.as_mut_ptr() as u32;
    save_to_memory(positive_show_info + 0xa0, qualifying_habitat_ptr);
    if ztshow::check_owning_habitat(positive_show_info) {
        error!("{}: check_owning_habitat returned true (blocked) for a real working tank habitat ({:#010x}, water_level>0)", test_name, qualifying_habitat_ptr);
        fail_flag = true;
    }
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} positive case done\n", test_name).as_bytes());
    }

    // Negative case: a null habitat pointer means there's no tank gating this show at all, so it must
    // not be blocked either.
    let null_buf = [0u8; 0xa4];
    let null_show_info = null_buf.as_ptr() as u32;
    if ztshow::check_owning_habitat(null_show_info) {
        error!("{}: check_owning_habitat returned true (blocked) for a null habitat pointer", test_name);
        fail_flag = true;
    }
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} negative case done\n", test_name).as_bytes());
    }

    // Blocking case: a real tank exhibit with zero water level must be blocked, if the test zoo has
    // one.
    if let Some(empty_tank_habitat_ptr) = find_real_empty_tank_habitat() {
        let mut blocking_buf = [0u8; 0xa4];
        let blocking_show_info = blocking_buf.as_mut_ptr() as u32;
        save_to_memory(blocking_show_info + 0xa0, empty_tank_habitat_ptr);
        if !ztshow::check_owning_habitat(blocking_show_info) {
            error!("{}: check_owning_habitat returned false (not blocked) for a real empty tank habitat ({:#010x})", test_name, empty_tank_habitat_ptr);
            fail_flag = true;
        }
    } else {
        info!("{}: no real empty tank habitat found in test zoo, skipping that half of the check", test_name);
    }
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} blocking case done\n", test_name).as_bytes());
    }

    // Best-effort smoke test of the full START entry point (which is what actually calls
    // check_owning_habitat in real gameplay) against the real show already attached to the qualifying
    // habitat - calling through START's own real, now-hooked address (0x005a3db4, ztshow::START).
    // Not asserted beyond "doesn't crash": whether it proceeds past the habitat check depends on real,
    // un-inspected scheduling data this session didn't reverse-engineer (see this function's own doc
    // comment), so an early return here is just as valid an outcome as a full run.
    if real_show_info != 0 {
        let real_show = real_show_info + 4;
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} about to call START on real_show={:#010x}\n", test_name, real_show).as_bytes());
        }
        let start_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32)>(0x005a3db4u32) };
        start_hooked(real_show as *const u32);
        info!("{}: START smoke-test against real show {:#010x} completed without crashing", test_name, real_show);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("CHECKPOINT {} START returned\n", test_name).as_bytes());
        }
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
/// Scans the live, loaded test zoo's real world entities (`globals().ztworldmgr()`'s entity array) for
/// one whose type passes `ztshow::RVA_SHOW_TRICK_TYPE_CHECK` (the same `entity_type_matches` gate
/// `do_current_item`/`validate_item` both apply) - i.e. a genuinely trick-eligible animal. Returns
/// `(entity_ptr, entity_id)` for the first match, `None` if the test zoo has none.
pub(crate) fn find_real_trick_eligible_unit() -> Option<(u32, u32)> {
    let world = globals().ztworldmgr();
    let start = world.entity_array_start();
    let end = world.entity_array_end();
    let mut i = start;
    while i < end {
        let entity_ptr = get_from_memory::<u32>(i);
        i += 0x4;
        if entity_ptr == 0 {
            continue;
        }
        if unsafe { crate::ztmegatilemgr::entity_type_matches(entity_ptr, ztshow::RVA_SHOW_TRICK_TYPE_CHECK) } {
            let id = get_from_memory::<u32>(entity_ptr + 0x124);
            return Some((entity_ptr, id));
        }
    }
    None
}

/// The inverse of [`find_real_trick_eligible_unit`] - scans the same real entity array for one whose type
/// *fails* `ztshow::RVA_SHOW_TRICK_TYPE_CHECK` (e.g. a guest or staff member, not an animal) - for
/// `ZTSHOWINFO_CHECK_UNIT_LIVE`'s own coverage of `checkUnit`'s ineligible-type return branch. Returns just
/// the entity's own numeric id (`+0x124`), matching that test's own `unit_id`-shaped inputs.
pub(crate) fn find_real_non_trick_eligible_unit() -> Option<u32> {
    let world = globals().ztworldmgr();
    let start = world.entity_array_start();
    let end = world.entity_array_end();
    let mut i = start;
    while i < end {
        let entity_ptr = get_from_memory::<u32>(i);
        i += 0x4;
        if entity_ptr == 0 {
            continue;
        }
        if !unsafe { crate::ztmegatilemgr::entity_type_matches(entity_ptr, ztshow::RVA_SHOW_TRICK_TYPE_CHECK) } {
            let id = get_from_memory::<u32>(entity_ptr + 0x124);
            return Some(id);
        }
    }
    None
}

/// ZTSHOW_GROUP3_TRICK_LIVE: `ZTShow::doCurrentItem`/`validateItem`/`doTrickEvent` are, like
/// `ADD_SCRIPT`/`CHECK_PENDING_SCRIPTS` above, full-replacement detours with no real-vs-reimplementation
/// diff oracle - this calls through their own real, now-hooked addresses directly against real,
/// `run_load_live_zoo`-populated `GLOBAL_ZTWorldMgr`/habitat data (they all internally call
/// `GET_UNIT.original()`, which needs a real, resolvable unit), asserting no crash plus a few structural
/// invariants.
///
/// Needs: (1) a real, already-configured show-tank habitat (`find_real_show_tank_habitat`, same
/// discovery `ZTSHOW_CHECK_OWNING_HABITAT_LIVE` uses) to get a real `ZTShow*`/`ZTShowInfo*` pair, and
/// (2) a real, trick-eligible animal somewhere in the test zoo (`find_real_trick_eligible_unit`) to
/// resolve via `GET_UNIT`. If either is missing, this is a genuine coverage gap, reported clearly
/// rather than skipped silently - see the `else` branch below.
pub(crate) fn run_ztshow_group3_trick_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSHOW_GROUP3_TRICK_LIVE";

    let Some((_habitat_ptr, real_show_info)) = find_real_show_tank_habitat() else {
        error!("{}: BLOCKED - no real, already-configured show-tank habitat (ZTHabitat::is_show_tank) found in test zoo; do_current_item/validate_item/do_trick_event have no live coverage", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - no real show-tank habitat found in test zoo\n", test_name).as_bytes());
        }
        return false;
    };

    let Some((_unit_ptr, unit_id)) = find_real_trick_eligible_unit() else {
        error!(
            "{}: BLOCKED - test zoo has no animal whose type passes RVA_SHOW_TRICK_TYPE_CHECK; would need a new zoo asset (a trick-eligible animal) to cover do_current_item/validate_item/do_trick_event's real unit-resolution path",
            test_name
        );
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: BLOCKED - test zoo has no trick-eligible animal\n", test_name).as_bytes());
        }
        return false;
    };

    let real_show = real_show_info + 4;
    let mut fail_flag = false;

    // Create a real ZTShowScriptState for our chosen unit against this real, properly-constructed
    // ZTShow's own `+0x34` state map, then fetch it back the same way `do_current_item`'s own body
    // does. `CREATE_SHOW_SCRIPT_STATE` is detoured onto the Rust port now, so the vanilla pole goes
    // through the live_support trampoline (`_DETOUR.call`) - real vanilla in every build profile.
    let create_result = ztshowscriptstate_live_support::real_create_show_script_state(real_show, unit_id);
    if create_result != 0 {
        info!("{}: CREATE_SHOW_SCRIPT_STATE returned {} (nonzero/failure) for unit {:#x}; do_current_item/do_trick_event will still be exercised via their early-return paths", test_name, create_result, unit_id);
    }
    let state_ptr = unsafe { GET_SHOW_SCRIPT_STATE.original()(real_show as *const u32, unit_id) };

    // DO_CURRENT_ITEM (0x005a2508, ztshow::DO_CURRENT_ITEM): safe for any unit_id regardless of
    // whether a state/eligible unit resolved - it handles state==0/unit_ptr==0/ineligible-type
    // internally, returning 5/-1 respectively rather than crashing.
    let do_current_item_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, u32) -> i32>(0x005a2508u32) };
    let current_item_result = do_current_item_hooked(real_show as *const u32, unit_id);
    info!("{}: do_current_item({:#x}) = {} (no crash)", test_name, unit_id, current_item_result);

    // VALIDATE_ITEM (0x005a6d70, ztshow::VALIDATE_ITEM): only safe to call once the real show's own
    // configured unit_type_id (`real_show+0x8`) genuinely has at least one real unit assigned
    // (`GET_SHOW_UNIT_LIST`'s own documented lack of an empty-list check - see `validate_item`'s doc
    // comment in `ztshow.rs`) - checked via GET_NUM_UNITS first rather than risking that dereference
    // speculatively.
    let show_unit_type_id = get_from_memory::<u32>(real_show + 0x8);
    // `.hooked()`: `GET_NUM_UNITS` is now detoured by `ztshowinfo.rs` (Stage 7) - see `ztshow.rs`'s own
    // `validate_item` comment for the rule this follows.
    let assigned_unit_count = unsafe { ZTSHOWINFO_GET_NUM_UNITS.hooked()(real_show_info as *const u32, show_unit_type_id) };
    if assigned_unit_count >= 1 {
        let validate_item_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, u16) -> u32>(0x005a6d70u32) };
        let validate_result = validate_item_hooked(real_show as *const u32, 0);
        info!("{}: validate_item(0) = {} (real show unit_type_id={:#x}, {} unit(s) assigned)", test_name, validate_result, show_unit_type_id, assigned_unit_count);
    } else {
        info!("{}: skipping validate_item - real show's own unit_type_id {:#x} has {} assigned units (validate_item's own empty-list dereference isn't guarded, see its doc comment)", test_name, show_unit_type_id, assigned_unit_count);
    }

    // DO_TRICK_EVENT (0x005a6894, ztshow::DO_TRICK_EVENT): needs a real, non-null ZTShowScriptState* -
    // only call it if GET_SHOW_SCRIPT_STATE actually resolved one above. Registers a fresh synthetic
    // script/item per case, points the real ZTShow at it (snapshotting/restoring `real_show+0x4` and
    // `state_ptr+0xc`/`+0xf` around each call so this doesn't permanently disturb the real, live
    // objects other tests later in this chain still use), and asserts the `+0x28`/`+0x2c`/`+0x30`
    // accumulator deltas match `do_trick_event`'s own accounting - see this function's own inline
    // comments for why the three threshold branches (low/mid/high relative to `ZTShowMgr`'s real
    // `threshold_a`/`threshold_b`/`threshold_c`) aren't distinguishable via those three fields alone
    // (the threshold dispatch only changes which `SEND_EVENT`/`DO_KEEPER_EVENT` calls fire, not the
    // accumulator writes, which all happen unconditionally before the dispatch) - exercising each is
    // still valuable as real-call-path coverage/crash safety, just not as a three-way accumulator diff.
    if state_ptr != 0 {
        let do_trick_event_hooked = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(*const u32, *const u32)>(0x005a6894u32) };

        let original_script_id = get_from_memory::<u16>(real_show + 0x4);
        let original_trick_index = get_from_memory::<u16>(state_ptr + 0xc);
        let original_skip_scoring = get_from_memory::<u8>(state_ptr + 0xf);

        let mgr_ptr = globals().ztshowmgr_ptr();
        let mirror_cases: Vec<(&str, u32)> = if mgr_ptr.is_null() {
            info!(
                "{}: GLOBAL_ZTShowMgr not initialized at this injection point - threshold-branch coverage skipped, only the skip_scoring/item_type==3 cases below run (same class of gap as ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE's own null check)",
                test_name
            );
            Vec::new()
        } else {
            let mgr = unsafe { &*mgr_ptr };
            vec![
                ("low (<=threshold_a)", mgr.threshold_a),
                ("mid (between threshold_b and threshold_c)", mgr.threshold_b.wrapping_add(mgr.threshold_c) / 2),
                ("high (>=threshold_c)", mgr.threshold_c),
            ]
        };

        // One case per real threshold branch, plus skip_scoring and the item_type==3 short-circuit.
        struct Case {
            label: String,
            item_type: u32,
            satisfaction: u32,
            satisfaction_mirror: u32,
            skip_scoring: bool,
        }
        let mut cases: Vec<Case> = mirror_cases
            .into_iter()
            .map(|(label, mirror)| Case { label: label.to_string(), item_type: 1, satisfaction: 7, satisfaction_mirror: mirror, skip_scoring: false })
            .collect();
        cases.push(Case { label: "skip_scoring".to_string(), item_type: 1, satisfaction: 7, satisfaction_mirror: 7, skip_scoring: true });
        cases.push(Case {
            label: "item_type==3 short-circuit".to_string(),
            item_type: 3,
            satisfaction: 7,
            satisfaction_mirror: 7,
            skip_scoring: false,
        });

        for (case_index, case) in cases.iter().enumerate() {
            // `add_item` only inserts when the item's own `item_type` matches the script's - register
            // each script with `case.item_type` itself as its type (rather than a fixed `SCRIPT_TYPE`)
            // so every case's item actually gets inserted, including the `item_type==3` short-circuit
            // case, which needs a genuine hit against `item_snapshot_by_id` to reach `do_trick_event`'s
            // own `item.item_type == 3` check at all.
            let script_id = ztshowscriptmgr::register_script(0x8000_0000 | case_index as u32, case.item_type)
                .expect("register_script should never reject a non-null ctor_ptr");
            let item = ztshowscriptmgr::live_support::raw_item_with_mirror(case.item_type, 1, case.satisfaction, case.satisfaction_mirror);
            ztshowscriptmgr::add_item(0x8000_0000 | case_index as u32, &item);

            save_to_memory(real_show + 0x4, script_id);
            save_to_memory(state_ptr + 0xc, 0u16); // trick_index 0, our only item
            save_to_memory(state_ptr + 0xf, case.skip_scoring as u8);

            let count_before = get_from_memory::<i32>(real_show + 0x28);
            let sum_before = get_from_memory::<i32>(real_show + 0x2c);
            let mirror_sum_before = get_from_memory::<i32>(real_show + 0x30);

            do_trick_event_hooked(real_show as *const u32, state_ptr as *const u32);

            let count_after = get_from_memory::<i32>(real_show + 0x28);
            let sum_after = get_from_memory::<i32>(real_show + 0x2c);
            let mirror_sum_after = get_from_memory::<i32>(real_show + 0x30);

            // Restore before asserting, so a failure doesn't also leave the real objects corrupted for
            // later tests.
            save_to_memory(real_show + 0x4, original_script_id);
            save_to_memory(state_ptr + 0xc, original_trick_index);
            save_to_memory(state_ptr + 0xf, original_skip_scoring);

            if case.item_type == 3 {
                if sum_after != sum_before || count_after != count_before || mirror_sum_after != mirror_sum_before {
                    error!("{}: case '{}' (item_type==3) should leave all three accumulators unchanged, got count {}->{}, sum {}->{}, mirror_sum {}->{}",
                        test_name, case.label, count_before, count_after, sum_before, sum_after, mirror_sum_before, mirror_sum_after);
                    fail_flag = true;
                }
                continue;
            }
            if sum_after != sum_before.wrapping_add(case.satisfaction as i32) {
                error!("{}: case '{}' expected sum {} -> {}, got {}", test_name, case.label, sum_before, sum_before.wrapping_add(case.satisfaction as i32), sum_after);
                fail_flag = true;
            }
            if case.skip_scoring {
                if count_after != count_before || mirror_sum_after != mirror_sum_before {
                    error!("{}: case '{}' (skip_scoring) should leave count/mirror_sum unchanged, got count {}->{}, mirror_sum {}->{}",
                        test_name, case.label, count_before, count_after, mirror_sum_before, mirror_sum_after);
                    fail_flag = true;
                }
            } else {
                // count/mirror_sum are written unconditionally before the threshold dispatch (which
                // itself only changes which SEND_EVENT/DO_KEEPER_EVENT calls fire, not these two
                // fields), so the same expectation holds whether or not GLOBAL_ZTShowMgr is live.
                let expected_mirror_sum = mirror_sum_before.wrapping_add(case.satisfaction_mirror as i32);
                if count_after != count_before + 1 || mirror_sum_after != expected_mirror_sum {
                    error!("{}: case '{}' expected count {} -> {}, mirror_sum {} -> {}, got count={}, mirror_sum={}",
                        test_name, case.label, count_before, count_before + 1, mirror_sum_before, expected_mirror_sum, count_after, mirror_sum_after);
                    fail_flag = true;
                }
            }
            info!("{}: case '{}' completed without crashing (mirror={})", test_name, case.label, case.satisfaction_mirror);
        }
    } else {
        info!("{}: skipping do_trick_event - no real ZTShowScriptState resolved for unit {:#x}", test_name, unit_id);
    }

    if !fail_flag {
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
