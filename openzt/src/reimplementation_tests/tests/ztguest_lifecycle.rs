//! Live checks for `ztguest.rs`'s lifecycle methods: `save`, `clearGoal`, `init`, `resetAI`, `removeFromMap` and
//! `abortGoal`. Twin fixtures built by the real vanilla constructor run the real body, the port and the hooked address
//! on identical state and are compared on results, captured bytes and whole-guest memory.
//!
//! `addToMap` is not exercised: `ZTUnit::addToMap` places the fixture on the live map. `abortGoal`'s satisfier
//! branches need a building grid and are only driven up to the satisfier check (a null grid fails it). The fixtures'
//! vtables are copies of the real one with the scripted slots replaced.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use openzt_detour::generated::{
    msvc_std_listuint::INSERT as LIST_UINT_INSERT,
    ztguest::{ABORT_GOAL, CLEAR_GOAL, INIT, LOAD, REMOVE_FROM_MAP, RESET_AI, SAVE},
};

use super::{
    ztguest_leaf::{compare_guests, Lcg},
    ztguide::{base, pin_rng, rng},
};
use crate::{
    globals::get_module_base,
    reimplementation_tests::{harness::finish_test, io_redirect},
    util::{get_from_memory, save_to_memory},
    ztguest::{
        guest_offsets,
        live_support::{self, real},
        ZTGuest,
    },
    ztkeeper::unit_offsets,
};

/// `DAT_00639118`: pointer to the sentinel of the multi-guest panel's list.
const MULTIGUEST_LIST_RVA: u32 = 0x0023_9118;
/// `DAT_00638004`: head of vanilla's 12-byte freelist.
const FREELIST_12_RVA: u32 = 0x0023_8004;

fn leak_words(words: &[u32]) -> u32 {
    Box::leak(words.to_vec().into_boxed_slice()).as_ptr() as u32
}

fn leak_bytes(bytes: &[u8]) -> u32 {
    Box::leak(bytes.to_vec().into_boxed_slice()).as_ptr() as u32
}

/// Fills the saved scalar fields with pseudo-random values, then points the four containers at leaked buffers of the
/// requested sizes (`items`/`ids` dword counts, `sites` 12-byte records, `string` bytes).
fn plant_save_state(guest: ZTGuest, seed: u32, items: usize, ids: usize, sites: usize, string: usize) {
    let mut rng = Lcg(seed);
    for offset in [
        0x284u32, 0x298, 0x29c, 0x2a0, 0x2a4, 0x2a8, 0x2b0, 0x2b8, 0x2c0, 0x2c8, 0x2d0, 0x2d4, 0x2d8, 0x2dc, 0x2e0, 0x2e4, 0x2e8, 0x2ec, 0x2f0, 0x2f4, 0x2f8,
        0x2fc, 0x300, 0x304, 0x308, 0x30c, 0x310, 0x314, 0x31c, 0x320, 0x324, 0x334,
    ] {
        save_to_memory(guest.0 + offset, rng.next() | 0x0101_0101);
    }
    for offset in [0x336u32, 0x337, 0x338, 0x339, 0x33a, 0x33b, 0x33c, 0x33e, 0x33f] {
        save_to_memory(guest.0 + offset, (rng.next() & 0x7f) as u8 + 1);
    }
    let item_words: Vec<u32> = (0..items).map(|_| rng.next()).collect();
    let id_words: Vec<u32> = (0..ids).map(|_| rng.next()).collect();
    let site_words: Vec<u32> = (0..sites * 3).map(|_| rng.next()).collect();
    let string_bytes: Vec<u8> = (0..string).map(|_| rng.next() as u8).collect();
    let (item_begin, id_begin, site_begin, string_begin) = (leak_words(&item_words), leak_words(&id_words), leak_words(&site_words), leak_bytes(&string_bytes));
    save_to_memory(guest.0 + guest_offsets::ITEMS, item_begin);
    save_to_memory(guest.0 + guest_offsets::ITEMS + 4, item_begin + items as u32 * 4);
    save_to_memory(guest.0 + guest_offsets::SAVED_IDS, id_begin);
    save_to_memory(guest.0 + guest_offsets::SAVED_IDS + 4, id_begin + ids as u32 * 4);
    save_to_memory(guest.0 + guest_offsets::VISITED_SITES, site_begin);
    save_to_memory(guest.0 + guest_offsets::VISITED_SITES + 4, site_begin + sites as u32 * 12);
    save_to_memory(guest.0 + guest_offsets::RETURN_BUILDING_STRING, string_begin);
    save_to_memory(guest.0 + guest_offsets::RETURN_BUILDING_STRING + 4, string_begin + string as u32);
}

/// `ZTGUEST_SAVE_LIVE`: `save` writes the same bytes and returns the same result through the real body, the port and
/// the hooked address, for empty and populated containers and for return-building strings either side of the
/// `0x1000` limit.
pub(crate) fn run_ztguest_save_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_SAVE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut lengths = std::collections::BTreeSet::new();
    for (seed, items, ids, sites, string) in [
        (1u32, 0usize, 0usize, 0usize, 0usize),
        (2, 5, 0, 0, 0),
        (3, 0, 4, 0, 0),
        (4, 0, 0, 3, 0),
        (5, 0, 0, 0, 9),
        (6, 5, 4, 3, 9),
        (7, 40, 40, 40, 300),
        (8, 1, 1, 1, 0xfff),
        (9, 1, 1, 1, 0x1000),
        (10, 2, 2, 2, 0x1001),
    ] {
        let label = format!("save items {items} ids {ids} sites {sites} string {string}");
        let fixtures: Vec<ZTGuest> = (0..3).map(|_| live_support::build_standalone_guest(guest_type)).collect();
        for guest in &fixtures {
            plant_save_state(*guest, seed, items, ids, sites, string);
        }
        io_redirect::begin_capture();
        let real_ok = real::save(fixtures[0].0, std::ptr::null());
        let real_bytes = io_redirect::end_capture();
        io_redirect::begin_capture();
        let port_ok = fixtures[1].save(std::ptr::null());
        let port_bytes = io_redirect::end_capture();
        io_redirect::begin_capture();
        let hooked_ok = unsafe { SAVE.hooked()(fixtures[2].0 as *const u32, std::ptr::null()) };
        let hooked_bytes = io_redirect::end_capture();
        if real_ok != port_ok || real_ok != hooked_ok {
            failures.push(format!("{label}: result real {real_ok}, port {port_ok}, hooked {hooked_ok}"));
        }
        if real_bytes != port_bytes || real_bytes != hooked_bytes {
            failures.push(format!("{label}: bytes real {}, port {}, hooked {} (or contents differ)", real_bytes.len(), port_bytes.len(), hooked_bytes.len()));
        }
        lengths.insert(real_bytes.len());
        if string >= 0x1000 && real_ok {
            failures.push(format!("{label}: a string of {string} bytes saved successfully"));
        }
    }
    if lengths.len() < 8 {
        failures.push(format!("only {} distinct stream lengths occurred", lengths.len()));
    }
    finish_test(test_name, failures, failure_log)
}

/// What the scripted vtable slots record: `(slot, argument)` in call order.
static CALLS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
static PREDICATE_ANSWER: AtomicBool = AtomicBool::new(false);

extern "thiscall" fn fake_clear_goal(_this: u32) {
    CALLS.lock().unwrap().push((0x174, 0));
}

extern "thiscall" fn fake_abort_action(_this: u32) {
    CALLS.lock().unwrap().push((0x2c0, 0));
}

extern "thiscall" fn fake_abort_predicate(_this: u32) -> u32 {
    CALLS.lock().unwrap().push((0x22c, 0));
    PREDICATE_ANSWER.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_reset_tail(_this: u32, arg: u32) {
    CALLS.lock().unwrap().push((0x294, arg));
}

/// Replaces the scripted slots of `guest`'s vtable with fakes, keeping a copy of the real vtable for the rest.
fn install_scripted_vtable(guest: ZTGuest) {
    let real_vtable: u32 = get_from_memory(guest.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x174 / 4] = fake_clear_goal as *const () as usize as u32;
    vtable[0x2c0 / 4] = fake_abort_action as *const () as usize as u32;
    vtable[0x22c / 4] = fake_abort_predicate as *const () as usize as u32;
    vtable[0x294 / 4] = fake_reset_tail as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(guest.0, vtable.as_ptr() as u32);
}

/// A leaked fake entity-type for pole fixtures is not needed: the fixtures keep the real guest type.
type Body<'a> = &'a dyn Fn(ZTGuest, usize);

/// Runs `body` (pole 0 real, 1 port, 2 hooked) against identically prepared scripted fixtures and compares the
/// recorded slot calls, RNG state afterwards and whole-guest memory.
fn three_poles(guest_type: u32, label: &str, seed: u32, prepare: &dyn Fn(ZTGuest), body: Body, failures: &mut Vec<String>) {
    let fixtures: Vec<ZTGuest> = (0..3).map(|_| live_support::build_standalone_guest(guest_type)).collect();
    let mut logs = Vec::new();
    let mut rngs = Vec::new();
    for (pole, guest) in fixtures.iter().enumerate() {
        install_scripted_vtable(*guest);
        prepare(*guest);
        CALLS.lock().unwrap().clear();
        pin_rng(seed);
        body(*guest, pole);
        rngs.push(rng());
        logs.push(CALLS.lock().unwrap().clone());
    }
    if logs[0] != logs[1] || logs[0] != logs[2] {
        failures.push(format!("{label}: slot calls real {:?}, port {:?}, hooked {:?}", logs[0], logs[1], logs[2]));
    }
    if rngs[0] != rngs[1] || rngs[0] != rngs[2] {
        failures.push(format!("{label}: RNG after real {:#x}, port {:#x}, hooked {:#x}", rngs[0], rngs[1], rngs[2]));
    }
    let shared = [guest_type];
    compare_guests(label, fixtures[0], fixtures[1], &shared, failures);
    compare_guests(&format!("{label} hooked"), fixtures[0], fixtures[2], &shared, failures);
}

/// `ZTGUEST_CLEAR_GOAL_LIVE`: `clearGoal` with an empty and a non-empty scratch string, with no guide id, a missing
/// id and the id of an entity that is not a guide.
pub(crate) fn run_ztguest_clear_goal_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_CLEAR_GOAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let live_guest = live_support::find_live_guests().first().map(|(pointer, _)| *pointer);
    let not_a_guide_id = live_guest.map(|guest| get_from_memory::<u32>(guest + unit_offsets::ENTITY_ID));
    for scratch in [false, true] {
        for guide_id in [Some(0u32), Some(0x7fff_0001), not_a_guide_id] {
            let Some(guide_id) = guide_id else { continue };
            let label = format!("clearGoal scratch {scratch} guide id {guide_id:#x}");
            three_poles(
                guest_type,
                &label,
                1,
                &|guest| {
                    let mut rng = Lcg(77);
                    for offset in [0x24du32, 0x24e, 0x337, 0x338, 0x33f, 0x1d4] {
                        save_to_memory(guest.0 + offset, (rng.next() & 0x7f) as u8);
                    }
                    save_to_memory(guest.0 + guest_offsets::GUIDE_ID, guide_id);
                    save_to_memory(guest.0 + unit_offsets::ENTITY_ID, 0x4242u32);
                    if scratch {
                        let buffer = leak_bytes(&[7, 8, 9, 0]);
                        save_to_memory(guest.0 + 0x1c8, buffer);
                        save_to_memory(guest.0 + 0x1cc, buffer + 3);
                    }
                },
                &|guest, pole| unsafe {
                    match pole {
                        0 => real::clear_goal(guest.0),
                        1 => guest.clear_goal(),
                        _ => CLEAR_GOAL.hooked()(guest.0 as *const u32),
                    }
                },
                &mut failures,
            );
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_INIT_LIVE`: `init` with both argument values leaves the same memory, the same RNG state and the same return
/// value, over a typed and an untyped guest.
pub(crate) fn run_ztguest_init_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_INIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    for seed in [1u32, 0x1234_5678, 0xdead_beef, 42] {
        for param in [false, true] {
            let results = Mutex::new(Vec::new());
            three_poles(
                guest_type,
                &format!("init seed {seed:#x} param {param}"),
                seed,
                &|_| {},
                &|guest, pole| unsafe {
                    let result = match pole {
                        0 => real::init(guest.0, param),
                        1 => guest.init(param),
                        _ => INIT.hooked()(guest.0 as *const u32, param),
                    };
                    results.lock().unwrap().push(result);
                },
                &mut failures,
            );
            let results = results.into_inner().unwrap();
            if results[0] != results[1] || results[0] != results[2] {
                failures.push(format!("init seed {seed:#x} param {param}: results real {:#x}, port {:#x}, hooked {:#x}", results[0], results[1], results[2]));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_RESET_AI_LIVE`: `resetAI` runs `ZTUnit::resetAI` and then the virtual `+0x294` with `0`.
pub(crate) fn run_ztguest_reset_ai_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_RESET_AI_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    three_poles(
        guest_type,
        "resetAI",
        1,
        &|_| {},
        &|guest, pole| unsafe {
            match pole {
                0 => real::reset_ai(guest.0),
                1 => guest.reset_ai(),
                _ => RESET_AI.hooked()(guest.0 as *const u32),
            }
        },
        &mut failures,
    );
    let calls_seen = CALLS.lock().unwrap().iter().any(|call| *call == (0x294, 0));
    if !calls_seen {
        failures.push("the virtual +0x294 slot was never called with 0".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_ABORT_GOAL_LIVE`: `abortGoal` for both argument values, with the abort predicate answering either way and
/// with the guest's need index matching neither, the first or the second need (the building grid is null, so the
/// satisfier checks fail and the common tail runs).
pub(crate) fn run_ztguest_abort_goal_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_ABORT_GOAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let need_index = |rva: u32| unsafe { openzt_detour::generated::ztworldmgr::GET_NEED_INDEX.hooked()((get_module_base("zoo.exe") as u32 + rva) as *const i8) as u8 };
    let (need_a, need_b) = (need_index(0x0023_c518), need_index(0x0023_c508));
    for param in [0u32, 1] {
        for answer in [false, true] {
            for need in [0xffu8, need_a, need_b] {
                PREDICATE_ANSWER.store(answer, Ordering::SeqCst);
                three_poles(
                    guest_type,
                    &format!("abortGoal param {param} predicate {answer} need {need:#x}"),
                    1,
                    &|guest| save_to_memory(guest.0 + 0x24e, need),
                    &|guest, pole| unsafe {
                        match pole {
                            0 => real::abort_goal(guest.0, param),
                            1 => guest.abort_goal(param != 0),
                            _ => ABORT_GOAL.hooked()(guest.0 as *const u32, param),
                        }
                    },
                    &mut failures,
                );
            }
        }
    }
    PREDICATE_ANSWER.store(false, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}

fn freelist_head() -> u32 {
    get_from_memory(base() + FREELIST_12_RVA)
}

fn on_freelist(nodes: &[u32]) -> usize {
    let mut node = freelist_head();
    let mut found = 0;
    let mut steps = 0;
    while node != 0 && steps < 4096 {
        if nodes.contains(&node) {
            found += 1;
        }
        steps += 1;
        node = get_from_memory(node);
    }
    found
}

fn guest_nodes(guest: u32) -> Vec<u32> {
    let sentinel: u32 = get_from_memory(get_from_memory::<u32>(base() + MULTIGUEST_LIST_RVA));
    let mut node: u32 = get_from_memory(sentinel);
    let mut nodes = Vec::new();
    while node != sentinel && nodes.len() < 100_000 {
        if get_from_memory::<u32>(node + 8) == guest {
            nodes.push(node);
        }
        node = get_from_memory(node);
    }
    nodes
}

fn multiguest_nodes_for(guest: u32) -> usize {
    let sentinel: u32 = get_from_memory(get_from_memory::<u32>(base() + MULTIGUEST_LIST_RVA));
    let mut node: u32 = get_from_memory(sentinel);
    let mut count = 0;
    while node != sentinel && count < 100_000 {
        if get_from_memory::<u32>(node + 8) == guest {
            count += 1;
        }
        node = get_from_memory(node);
    }
    count
}

/// `ZTGUEST_REMOVE_FROM_MAP_LIVE`: a guest listed twice in the multi-guest panel's list is unlinked both times by the
/// real body, the port and the hooked address, the freelist grows by the same number of nodes, and a guest excluded by
/// `+0x142` leaves the list alone.
pub(crate) fn run_ztguest_remove_from_map_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_REMOVE_FROM_MAP_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    for excluded in [false, true] {
        for param in [false, true] {
            let mut outcomes = vec![(0usize, 0usize); 3];
            for pole in [1usize, 2, 0] {
                let guest = live_support::build_standalone_guest(guest_type);
                save_to_memory(guest.0 + 0x142, excluded as u8);
                let sentinel: u32 = get_from_memory(get_from_memory::<u32>(base() + MULTIGUEST_LIST_RVA));
                for _ in 0..2 {
                    let mut result: u32 = 0;
                    unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, &guest.0 as *const u32 as *const i32) };
                }
                let listed = guest_nodes(guest.0);
                unsafe {
                    match pole {
                        0 => real::remove_from_map(guest.0, param),
                        1 => guest.remove_from_map(param),
                        _ => REMOVE_FROM_MAP.hooked()(guest.0 as *const u32, param),
                    }
                }
                outcomes[pole] = (multiguest_nodes_for(guest.0), on_freelist(&listed));
                // Leave no stale node behind for the next pole when the guest was excluded.
                if excluded {
                    let sentinel: u32 = get_from_memory(get_from_memory::<u32>(base() + MULTIGUEST_LIST_RVA));
                    let mut node: u32 = get_from_memory(sentinel);
                    while node != sentinel {
                        let next: u32 = get_from_memory(node);
                        if get_from_memory::<u32>(node + 8) == guest.0 {
                            save_to_memory(get_from_memory::<u32>(node + 4), next);
                            save_to_memory(next + 4, get_from_memory::<u32>(node + 4));
                        }
                        node = next;
                    }
                }
            }
            let label = format!("removeFromMap excluded {excluded} param {param}");
            if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                failures.push(format!("{label}: (nodes left, own nodes on the freelist) real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
            }
            if !excluded && (outcomes[0].0 != 0 || outcomes[0].1 == 0) {
                failures.push(format!("{label}: {} nodes left, {} own nodes freed (expected 0 and at least 1)", outcomes[0].0, outcomes[0].1));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// The contents behind a guest's pool containers: carried items, saved ids, visited-site records and the
/// return-building string bytes.
pub(super) fn container_contents(guest: ZTGuest) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u8>) {
    let words = |vector: u32, element: u32| -> Vec<u32> {
        let begin: u32 = get_from_memory(guest.0 + vector);
        let end: u32 = get_from_memory(guest.0 + vector + 4);
        if begin == 0 || end < begin {
            return Vec::new();
        }
        (0..(end - begin) / 4).map(|i| get_from_memory::<u32>(begin + i * 4)).take((element * 64) as usize).collect()
    };
    let begin: u32 = get_from_memory(guest.0 + guest_offsets::RETURN_BUILDING_STRING);
    let end: u32 = get_from_memory(guest.0 + guest_offsets::RETURN_BUILDING_STRING + 4);
    let string = if begin == 0 || end < begin { Vec::new() } else { (begin..end).map(get_from_memory::<u8>).collect() };
    (words(guest_offsets::ITEMS, 1), words(guest_offsets::SAVED_IDS, 1), words(guest_offsets::VISITED_SITES, 3), string)
}

fn run_load(guest: ZTGuest, bytes: &[u8], version: u32, pole: usize) -> (bool, usize, u32) {
    pin_rng(0x00c0_ffee);
    io_redirect::begin_replay(bytes.to_vec());
    let result = match pole {
        0 => real::load(guest.0, std::ptr::null(), version),
        1 => guest.load(std::ptr::null(), version),
        _ => unsafe { LOAD.hooked()(guest.0 as *const u32, std::ptr::null(), version) },
    };
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed, rng())
}

/// Streams carry five carried-item slots, as every real save does: vanilla reads `items[3]` unconditionally, so a guest
/// whose item vector is empty faults in the real body.
/// `ZTGUEST_LOAD_LIVE`: a stream `save` wrote (with item, id, site and string counts that exceed what `load` keeps,
/// and one with an over-long string) is loaded through the real body, the port and the hooked address, whole and cut at
/// every byte offset, then a second stream is loaded over the first. Result, bytes consumed, whole-guest memory and
/// the container contents must agree. Versions below `0x5e` go to the real body through the hooked address.
pub(crate) fn run_ztguest_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = [guest_type];
    let make_stream = |seed: u32, items: usize, ids: usize, sites: usize, string: usize| -> Vec<u8> {
        let source = live_support::build_standalone_guest(guest_type);
        plant_save_state(source, seed, items, ids, sites, string);
        io_redirect::begin_capture();
        real::save(source.0, std::ptr::null());
        io_redirect::end_capture()
    };
    let streams = [
        ("small", make_stream(21, 5, 1, 2, 5)),
        ("capped", make_stream(22, 12, 9, 17, 40)),
        ("long string", make_stream(23, 5, 1, 1, 0x1000)),
        ("empty", make_stream(24, 5, 0, 0, 0)),
    ];
    let mut compared = 0usize;
    let mut full_reads = 0usize;
    let mut full_summary: Vec<String> = Vec::new();
    for version in [0x5eu32, 0x5f, 0x60, 0x61] {
        for (name, bytes) in &streams {
            let mut cuts: Vec<usize> = (0..bytes.len()).collect();
            cuts.push(bytes.len());
            for cut in cuts {
                let label = format!("load v{version:#x} {name} cut {cut}");
                let fixtures: Vec<ZTGuest> = (0..3).map(|_| live_support::build_standalone_guest(guest_type)).collect();
                let mut outcomes = Vec::new();
                for (pole, guest) in fixtures.iter().enumerate() {
                    outcomes.push(run_load(*guest, &bytes[..cut], version, pole));
                }
                if cut == bytes.len() {
                    if outcomes[0].1 + 1 >= bytes.len() {
                        full_reads += 1;
                    }
                    full_summary.push(format!("v{version:#x} {name}: consumed {} of {}", outcomes[0].1, bytes.len()));
                }
                if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                    failures.push(format!("{label}: (result, consumed, RNG) real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
                }
                compare_guests(&label, fixtures[0], fixtures[1], &shared, &mut failures);
                compare_guests(&format!("{label} hooked"), fixtures[0], fixtures[2], &shared, &mut failures);
                let reference = container_contents(fixtures[0]);
                if reference != container_contents(fixtures[1]) || reference != container_contents(fixtures[2]) {
                    failures.push(format!("{label}: container contents differ"));
                }
                compared += 1;
                if failures.len() > 20 {
                    return finish_test(test_name, failures, failure_log);
                }
            }
        }
    }

    // A second load over a populated guest clears and refills the containers the same way.
    let (first, second) = (&streams[1].1, &streams[0].1);
    let fixtures: Vec<ZTGuest> = (0..3).map(|_| live_support::build_standalone_guest(guest_type)).collect();
    let mut outcomes = Vec::new();
    for (pole, guest) in fixtures.iter().enumerate() {
        run_load(*guest, first, 0x60, pole);
        outcomes.push(run_load(*guest, second, 0x60, pole));
    }
    if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
        failures.push(format!("reload: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
    }
    compare_guests("reload", fixtures[0], fixtures[1], &shared, &mut failures);
    let reference = container_contents(fixtures[0]);
    if reference != container_contents(fixtures[1]) || reference != container_contents(fixtures[2]) {
        failures.push("reload: container contents differ".to_string());
    }

    // An older version is read by the real body through every pole that reaches the hooked address.
    for version in [0x5du32, 0x42, 0x10] {
        let a = live_support::build_standalone_guest(guest_type);
        let b = live_support::build_standalone_guest(guest_type);
        let real_outcome = run_load(a, &streams[0].1, version, 0);
        let hooked_outcome = run_load(b, &streams[0].1, version, 2);
        if real_outcome != hooked_outcome {
            failures.push(format!("v{version:#x}: real {real_outcome:?}, hooked {hooked_outcome:?}"));
        }
        compare_guests(&format!("v{version:#x}"), a, b, &shared, &mut failures);
    }
    if full_reads == 0 {
        failures.push(format!("no complete stream was consumed in full over {compared} cases: {full_summary:?}"));
    }
    finish_test(test_name, failures, failure_log)
}
