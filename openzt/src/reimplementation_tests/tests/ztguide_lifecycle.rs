//! Live checks for `ztguide.rs`'s follower-list and lifecycle methods: `removeGuest`, `releaseGuests`, `clearGoal`,
//! `removeFromMap`, `initStatusVars`, `save` and `load`. Same shape as `ztguide.rs`'s other tests: twin fixtures built
//! by the real vanilla constructor, one running the real body and one the port, compared on results and whole-guide
//! memory.
//!
//! The follower nodes are vanilla pool nodes (`list<uint>::insert` pops the 12-byte freelist), so no test frees a node
//! through Rust's allocator. `releaseGuests` and `removeFromMap(false)` run against a fake world entity vector that
//! holds only fake guests, so no live guest is ever touched.

use std::sync::Mutex;

use openzt_detour::generated::{
    msvc_std_listuint::INSERT as LIST_UINT_INSERT,
    ztguide::{ADD_GUEST, CLEAR_GOAL, INIT_STATUS_VARS, LOAD, RELEASE_GUESTS, REMOVE_FROM_MAP, REMOVE_GUEST, SAVE},
};

use super::ztguide::{base, compare_memory, pin_rng, rng};
use crate::{
    reimplementation_tests::{harness::finish_test, io_redirect},
    util::{get_from_memory, save_to_memory},
    ztguide::{
        live_support::{self, real},
        ZTGuide, FIELD_0X298_OFFSET, FOLLOWERS_OFFSET, VIEWING_AREA_INFORMED_OFFSET,
    },
    ztkeeper::unit_offsets,
};

use crate::vanilla_list::FREELIST_12_RVA;
/// `GLOBAL_ZTWorldMgr`.
const WORLD_MGR_RVA: u32 = 0x0023_8040;
const WORLD_ENTITY_BEGIN: u32 = 0x80;
const WORLD_ENTITY_END: u32 = 0x84;
/// `ZTGuideType` `cCrowdCheck`.
const TYPE_CROWD_CHECK_OFFSET: u32 = 0x1fc;
const GUEST_GUIDE_ID: u32 = 0x2e4;
const GUEST_FOLLOWING_FLAG: u32 = 0x337;

/// Appends `ids` to the guide's follower list through vanilla's `list<uint>::insert`.
fn set_followers(guide: ZTGuide, ids: &[u32]) {
    let sentinel: u32 = get_from_memory(guide.0 + FOLLOWERS_OFFSET);
    for id in ids {
        let mut result: u32 = 0;
        unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, id as *const u32 as *const i32) };
    }
}

/// The ids in list order, plus a check that the previous links mirror the next links.
pub(super) fn follower_ids(guide: ZTGuide, failures: &mut Vec<String>, label: &str) -> Vec<u32> {
    let sentinel: u32 = get_from_memory(guide.0 + FOLLOWERS_OFFSET);
    let mut ids = Vec::new();
    let mut node: u32 = get_from_memory(sentinel);
    let mut previous = sentinel;
    while node != sentinel && ids.len() < 64 {
        if get_from_memory::<u32>(node + 4) != previous {
            failures.push(format!("{label}: node {node:#x} has a wrong previous link"));
        }
        ids.push(get_from_memory(node + 8));
        previous = node;
        node = get_from_memory(node);
    }
    if get_from_memory::<u32>(sentinel + 4) != previous {
        failures.push(format!("{label}: the sentinel's previous link is wrong"));
    }
    ids
}

/// How many nodes were pushed on the 12-byte freelist since its head was `before`.
fn freelist_pushed(before: u32) -> usize {
    let mut node: u32 = get_from_memory(base() + FREELIST_12_RVA);
    let mut count = 0;
    while node != before && node != 0 && count < 64 {
        count += 1;
        node = get_from_memory(node);
    }
    count
}

fn freelist_head() -> u32 {
    get_from_memory(base() + FREELIST_12_RVA)
}

/// Runs `body` on three twin fixtures (real body, port, hooked address) after `prepare` has set each up, and compares
/// the followers, the freelist growth and the whole guide memory. Returns the three fixtures' follower lists.
fn three_poles(
    guide_type: u32,
    label: &str,
    failures: &mut Vec<String>,
    prepare: &dyn Fn(ZTGuide),
    real_body: &dyn Fn(ZTGuide),
    port_body: &dyn Fn(ZTGuide),
    hooked_body: &dyn Fn(ZTGuide),
) -> Vec<u32> {
    three_poles_with(guide_type, true, label, failures, prepare, real_body, port_body, hooked_body)
}

/// `three_poles`, with the freelist-push comparison optional: a body that pops nodes (an insert) moves the head away
/// from its starting node, so counting pushes since then means nothing.
#[allow(clippy::too_many_arguments)]
fn three_poles_with(
    guide_type: u32,
    check_freelist: bool,
    label: &str,
    failures: &mut Vec<String>,
    prepare: &dyn Fn(ZTGuide),
    real_body: &dyn Fn(ZTGuide),
    port_body: &dyn Fn(ZTGuide),
    hooked_body: &dyn Fn(ZTGuide),
) -> Vec<u32> {
    let fixtures: Vec<ZTGuide> = (0..3).map(|_| live_support::build_standalone_guide(guide_type)).collect();
    for guide in &fixtures {
        prepare(*guide);
    }
    let shared = vec![guide_type];
    let mut pushed = Vec::new();
    for (guide, body) in fixtures.iter().zip([real_body, port_body, hooked_body]) {
        let before = freelist_head();
        body(*guide);
        pushed.push(freelist_pushed(before));
    }
    if check_freelist && (pushed[0] != pushed[1] || pushed[0] != pushed[2]) {
        failures.push(format!("{label}: freelist pushes real {}, port {}, hooked {}", pushed[0], pushed[1], pushed[2]));
    }
    let real_ids = follower_ids(fixtures[0], failures, &format!("{label} real"));
    let port_ids = follower_ids(fixtures[1], failures, &format!("{label} port"));
    let hooked_ids = follower_ids(fixtures[2], failures, &format!("{label} hooked"));
    if real_ids != port_ids || real_ids != hooked_ids {
        failures.push(format!("{label}: followers real {real_ids:?}, port {port_ids:?}, hooked {hooked_ids:?}"));
    }
    compare_memory(label, fixtures[0], fixtures[1], &shared, failures);
    compare_memory(&format!("{label} hooked"), fixtures[0], fixtures[2], &shared, failures);
    real_ids
}

/// `ZTGUIDE_REMOVE_GUEST_LIVE`: `removeGuest` over a list with a repeated id, removing a middle id, the repeated id,
/// the first, the last, and an absent one: the remaining ids, the links, and how many nodes reach the freelist match
/// the real body and the hooked address.
pub(crate) fn run_ztguide_remove_guest_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_REMOVE_GUEST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut remaining = std::collections::BTreeSet::new();
    for target in [3i32, 2, 1, 4, 99] {
        let ids = three_poles(
            guide_type,
            &format!("remove {target}"),
            &mut failures,
            &|guide| set_followers(guide, &[1, 2, 3, 2, 4]),
            &|guide| real::remove_guest(guide.0, target),
            &|guide| guide.remove_guest(target),
            &|guide| unsafe { REMOVE_GUEST.hooked()(guide.0 as *const u32, target) },
        );
        remaining.insert(ids.len());
    }
    if remaining.len() < 3 {
        failures.push(format!("only {remaining:?} remaining lengths occurred"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUIDE_ADD_GUEST_LIVE`: `addGuest` onto an empty list, a new id, a repeated first/middle/last id and a repeat
/// of the just-added id: the lists, links and whole-guide memory match the real body and the hooked address.
pub(crate) fn run_ztguide_add_guest_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_ADD_GUEST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut lengths = std::collections::BTreeSet::new();
    for (initial, target) in [(&[][..], 7i32), (&[1, 2, 3][..], 9), (&[1, 2, 3][..], 1), (&[1, 2, 3][..], 2), (&[1, 2, 3][..], 3)] {
        let ids = three_poles_with(
            guide_type,
            false,
            &format!("add {target} to {initial:?}"),
            &mut failures,
            &|guide| set_followers(guide, initial),
            &|guide| real::add_guest(guide.0, target),
            &|guide| guide.add_guest(target),
            &|guide| unsafe { ADD_GUEST.hooked()(guide.0 as *const u32, target) },
        );
        if ids.iter().filter(|id| **id == target as u32).count() != 1 {
            failures.push(format!("add {target} to {initial:?}: ids {ids:?} do not hold the target exactly once"));
        }
        lengths.insert(ids.len());
    }
    if lengths.len() < 3 {
        failures.push(format!("only {lengths:?} resulting lengths occurred"));
    }
    finish_test(test_name, failures, failure_log)
}

/// While `releaseGuests` runs against the fake world, the guests' `clearGoal` slot records the guest's id here.
static CLEAR_LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());

extern "thiscall" fn fake_guest_clear_goal(this: u32) {
    CLEAR_LOG.lock().unwrap().push(get_from_memory(this + unit_offsets::ENTITY_ID));
}

extern "thiscall" fn fake_cast_true(_this: u32, _tag: u32) -> u32 {
    1
}

extern "thiscall" fn fake_cast_false(_this: u32, _tag: u32) -> u32 {
    0
}

/// A leaked fake type whose vtable's `isCastClass` (`+0x1c`) answers `cast`.
fn fake_entity_type(cast: bool) -> u32 {
    let mut vtable = vec![0u32; 16];
    vtable[7] = if cast { fake_cast_true as *const () as usize as u32 } else { fake_cast_false as *const () as usize as u32 };
    let vtable = Box::leak(vtable.into_boxed_slice());
    let ty = Box::leak(vec![0u32; 0x80].into_boxed_slice());
    ty[0] = vtable.as_ptr() as u32;
    ty.as_ptr() as u32
}

/// A leaked fake entity: own id `id`, type `ty`, following guide `guide_id`, `+0x337` set, and a vtable whose
/// `clearGoal` slot (`+0x174`) is [`fake_guest_clear_goal`].
fn fake_entity(id: u32, ty: u32, guide_id: u32) -> u32 {
    let mut vtable = vec![0u32; 128];
    vtable[0x174 / 4] = fake_guest_clear_goal as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    let entity = Box::leak(vec![0u8; 0x400].into_boxed_slice());
    let address = entity.as_ptr() as u32;
    save_to_memory(address, vtable.as_ptr() as u32);
    save_to_memory(address + unit_offsets::ENTITY_ID, id);
    save_to_memory(address + unit_offsets::ENTITY_TYPE, ty);
    save_to_memory(address + GUEST_GUIDE_ID, guide_id);
    save_to_memory(address + GUEST_FOLLOWING_FLAG, 1u8);
    address
}

/// Points the live world's entity vector at a fake one (null, a non-guest, and guests of this guide and another) for
/// the duration of `body`, restoring the real vector afterwards. Returns the guests' final `+0x337` bytes in order.
fn with_fake_world(guide_id: u32, body: &dyn Fn()) -> Vec<u8> {
    let guest_type = fake_entity_type(true);
    let other_type = fake_entity_type(false);
    let entities = [
        0,
        fake_entity(0x2001, other_type, guide_id),
        fake_entity(0x2002, guest_type, guide_id),
        fake_entity(0x2003, guest_type, guide_id + 1),
        fake_entity(0x2004, guest_type, guide_id),
        fake_entity(0x2005, guest_type, guide_id),
        fake_entity(0x2006, guest_type, guide_id),
    ];
    let vector = Box::leak(entities.to_vec().into_boxed_slice());
    let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
    let (saved_begin, saved_end): (u32, u32) = (get_from_memory(world + WORLD_ENTITY_BEGIN), get_from_memory(world + WORLD_ENTITY_END));
    save_to_memory(world + WORLD_ENTITY_BEGIN, vector.as_ptr() as u32);
    save_to_memory(world + WORLD_ENTITY_END, vector.as_ptr() as u32 + (vector.len() * 4) as u32);
    body();
    save_to_memory(world + WORLD_ENTITY_BEGIN, saved_begin);
    save_to_memory(world + WORLD_ENTITY_END, saved_end);
    entities.iter().map(|&entity| if entity == 0 { 0xff } else { get_from_memory::<u8>(entity + GUEST_FOLLOWING_FLAG) }).collect()
}

/// `ZTGUIDE_RELEASE_GUESTS_LIVE`: `releaseGuests` and `removeFromMap(false)` against a fake world (a null slot, a
/// non-guest following the guide's id, five guests of which one follows another guide) with the generator pinned,
/// for 0, 40 (three seeds), 60 and 100 percent: the guests that get `clearGoal` (in order), their `+0x337` bytes, the follower list, the
/// freelist growth and the generator state match the real body and the hooked address. `removeFromMap(true)` must
/// not release anyone.
pub(crate) fn run_ztguide_release_guests_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_RELEASE_GUESTS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let seed = rng();
    let guide_id = 0x1234u32;
    let mut released_counts = std::collections::BTreeSet::new();
    let followers = [0x2001u32, 0x2002, 0x2004, 0x2005, 0x2006, 0x7777];
    for (percent, seed) in [(0i32, seed), (40, seed), (40, seed ^ 0x1234_5678), (40, seed.wrapping_add(0x9e37_79b9)), (60, seed ^ 0x0bad_cafe), (100, seed)] {
        let mut results: Vec<(Vec<u32>, Vec<u32>, Vec<u8>, u32)> = Vec::new();
        for pole in 0..3 {
            let fixture = live_support::build_standalone_guide(guide_type);
            save_to_memory(fixture.0 + unit_offsets::ENTITY_ID, guide_id);
            set_followers(fixture, &followers);
            pin_rng(seed);
            CLEAR_LOG.lock().unwrap().clear();
            let bytes = with_fake_world(guide_id, &|| match pole {
                0 => real::release_guests(fixture.0, percent),
                1 => fixture.release_guests(percent),
                _ => unsafe { RELEASE_GUESTS.hooked()(fixture.0 as *const u32, percent) },
            });
            let cleared = std::mem::take(&mut *CLEAR_LOG.lock().unwrap());
            let ids = follower_ids(fixture, &mut failures, &format!("release {percent}% pole {pole}"));
            results.push((cleared, ids, bytes, rng()));
        }
        if results[0] != results[1] || results[0] != results[2] {
            failures.push(format!("releaseGuests {percent}%: real {:?}, port {:?}, hooked {:?}", results[0], results[1], results[2]));
        }
        released_counts.insert(results[0].0.len());
    }
    if released_counts.len() < 4 {
        failures.push(format!("only {released_counts:?} result sizes occurred, so the percent sweep covered too few branches"));
    }

    // removeFromMap(false) releases everyone, removeFromMap(true) no one; +0x2a0 clears either way.
    for param in [false, true] {
        let mut results: Vec<(Vec<u32>, Vec<u8>, u32, u8)> = Vec::new();
        for pole in 0..3 {
            let fixture = live_support::build_standalone_guide(guide_type);
            save_to_memory(fixture.0 + unit_offsets::ENTITY_ID, guide_id);
            save_to_memory(fixture.0 + VIEWING_AREA_INFORMED_OFFSET, 1u8);
            set_followers(fixture, &followers);
            pin_rng(seed);
            CLEAR_LOG.lock().unwrap().clear();
            let bytes = with_fake_world(guide_id, &|| match pole {
                0 => real::remove_from_map(fixture.0, param),
                1 => fixture.remove_from_map(param),
                _ => unsafe { REMOVE_FROM_MAP.hooked()(fixture.0 as *const u32, param) },
            });
            let cleared = std::mem::take(&mut *CLEAR_LOG.lock().unwrap());
            results.push((cleared, bytes, rng(), get_from_memory(fixture.0 + VIEWING_AREA_INFORMED_OFFSET)));
        }
        if results[0] != results[1] || results[0] != results[2] {
            failures.push(format!("removeFromMap({param}): real {:?}, port {:?}, hooked {:?}", results[0], results[1], results[2]));
        }
        if results[0].3 != 0 {
            failures.push(format!("removeFromMap({param}) left +0x2a0 at {}", results[0].3));
        }
        if param && !results[0].0.is_empty() {
            failures.push(format!("removeFromMap(true) released {:?}", results[0].0));
        }
        if !param && results[0].0.len() != 4 {
            failures.push(format!("removeFromMap(false) released {:?}, expected the four guests of this guide", results[0].0));
        }
    }
    pin_rng(seed);
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUIDE_CLEAR_GOAL_LIVE`: `clearGoal` empties a three-node list (three nodes reach the freelist) and clears
/// `+0x2a0`; on an empty list it pushes nothing. Real body, port and hooked address agree on the whole guide.
pub(crate) fn run_ztguide_clear_goal_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_CLEAR_GOAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    for ids in [&[5u32, 6, 7][..], &[]] {
        let label = format!("clearGoal with {} followers", ids.len());
        let remaining = three_poles(
            guide_type,
            &label,
            &mut failures,
            &|guide| {
                set_followers(guide, ids);
                save_to_memory(guide.0 + VIEWING_AREA_INFORMED_OFFSET, 1u8);
            },
            &|guide| real::clear_goal(guide.0),
            &|guide| guide.clear_goal(),
            &|guide| unsafe { CLEAR_GOAL.hooked()(guide.0 as *const u32) },
        );
        if !remaining.is_empty() {
            failures.push(format!("{label}: followers left {remaining:?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUIDE_INIT_STATUS_VARS_LIVE`: `initStatusVars` with a dirtied guide (`+0x298`, countdown, `+0x2a0` set, three
/// followers) over a type whose `cCrowdCheck` is its own value, `7` and `0`, then over a unit whose type is not a
/// guide type; real body, port and hooked address agree on the result's low byte, the generator state, the follower
/// list, and the whole guide. The real type's period is restored afterwards.
pub(crate) fn run_ztguide_init_status_vars_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_INIT_STATUS_VARS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let seed = rng();
    let original_period: i32 = get_from_memory(guide_type + TYPE_CROWD_CHECK_OFFSET);
    let mut countdowns = std::collections::BTreeSet::new();
    for period in [original_period, 7, 0] {
        save_to_memory(guide_type + TYPE_CROWD_CHECK_OFFSET, period);
        let results: std::cell::RefCell<Vec<(u32, u32)>> = std::cell::RefCell::new(Vec::new());
        let record = |result: bool| results.borrow_mut().push((result as u32, rng()));
        let label = format!("period {period}");
        let remaining = three_poles(
            guide_type,
            &label,
            &mut failures,
            &|guide| {
                save_to_memory(guide.0 + FIELD_0X298_OFFSET, 5u32);
                guide.set_crowd_check_countdown(123);
                save_to_memory(guide.0 + VIEWING_AREA_INFORMED_OFFSET, 1u8);
                set_followers(guide, &[1, 2, 3]);
            },
            &|guide| {
                pin_rng(seed);
                record(real::init_status_vars(guide.0));
            },
            &|guide| {
                pin_rng(seed);
                record(guide.init_status_vars());
            },
            &|guide| {
                pin_rng(seed);
                record(unsafe { INIT_STATUS_VARS.hooked()(guide.0 as *const u32) });
            },
        );
        let results = results.into_inner();
        if results[0] != results[1] || results[0] != results[2] {
            failures.push(format!("{label}: (AL, RNG) real {:x?}, port {:x?}, hooked {:x?}", results[0], results[1], results[2]));
        }
        if results[0].0 != 1 {
            failures.push(format!("{label}: result {:#x}, expected 1", results[0].0));
        }
        if !remaining.is_empty() {
            failures.push(format!("{label}: followers left {remaining:?}"));
        }
        let probe = live_support::build_standalone_guide(guide_type);
        pin_rng(seed);
        probe.init_status_vars();
        countdowns.insert(probe.crowd_check_countdown());
    }
    save_to_memory(guide_type + TYPE_CROWD_CHECK_OFFSET, original_period);
    pin_rng(seed);
    if countdowns.len() < 2 {
        failures.push(format!("the countdown took only {countdowns:?}"));
    }
    finish_test(test_name, failures, failure_log)
}

/// Bytes the guide's own data occupies in a stream of `version` with `count` followers.
fn guide_stream_length(version: u32, count: usize) -> usize {
    let mut length = 0;
    if version >= 0x10 {
        length += 4;
        if version == 0x10 {
            return length + 2;
        }
    }
    if version >= 0x36 {
        length += 4;
        if version < 0x41 {
            length += 4;
        } else {
            length += 4 + 4 * count;
            if version >= 0x42 {
                length += 1;
            }
        }
    }
    length
}

fn guide_stream(version: u32, ids: &[u32]) -> Vec<u8> {
    let mut stream = Vec::new();
    if version >= 0x10 {
        stream.extend_from_slice(&0x0a0b_0c0du32.to_le_bytes());
        if version == 0x10 {
            stream.extend_from_slice(&[0x55, 0x66]);
            return stream;
        }
    }
    if version >= 0x36 {
        stream.extend_from_slice(&0x1112_1314u32.to_le_bytes());
        if version < 0x41 {
            stream.extend_from_slice(&0x2122_2324u32.to_le_bytes());
        } else {
            stream.extend_from_slice(&(ids.len() as u32).to_le_bytes());
            for id in ids {
                stream.extend_from_slice(&id.to_le_bytes());
            }
            if version >= 0x42 {
                stream.push(1);
            }
        }
    }
    stream
}

fn run_load(guide: ZTGuide, bytes: Vec<u8>, version: u32, body: u32) -> (bool, usize) {
    io_redirect::begin_replay(bytes);
    let result = match body {
        0 => real::load(guide.0, std::ptr::null(), version),
        1 => guide.load(std::ptr::null(), version),
        _ => unsafe { LOAD.hooked()(guide.0 as *const u32, std::ptr::null(), version) },
    };
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed)
}

/// `ZTGUIDE_SAVE_LOAD_LIVE`: `save` writes the same bytes and result as the real body for a dirtied guide (`+0x298`,
/// countdown, count, ids, `+0x2a0`, then the `ZTStaff` data); `load` is compared across every version boundary
/// (`0x0f`, `0x10`, `0x11`, `0x35`, `0x36`, `0x40`, `0x41`, `0x42`, `0x43`) with streams that hold exactly the guide's
/// data (so the `ZTStaff` read finds nothing left), one byte short, cut inside the follower list, and claiming more
/// ids than follow, comparing result, bytes consumed, follower list and the whole guide (the freelist is not compared: `list<uint>::insert` pops it as the old nodes are pushed). A last round
/// trip loads a real save.
pub(crate) fn run_ztguide_save_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_SAVE_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![guide_type];
    let dirty = |guide: ZTGuide| {
        save_to_memory(guide.0 + FIELD_0X298_OFFSET, 0x0102_0304u32);
        guide.set_crowd_check_countdown(0x1112_1314);
        save_to_memory(guide.0 + VIEWING_AREA_INFORMED_OFFSET, 1u8);
        set_followers(guide, &[11, 22, 33]);
    };

    let (real_guide, port_guide) = (live_support::build_standalone_guide(guide_type), live_support::build_standalone_guide(guide_type));
    dirty(real_guide);
    dirty(port_guide);
    io_redirect::begin_capture();
    let real_ok = real::save(real_guide.0, std::ptr::null());
    let real_bytes = io_redirect::end_capture();
    io_redirect::begin_capture();
    let port_ok = port_guide.save(std::ptr::null());
    let port_bytes = io_redirect::end_capture();
    io_redirect::begin_capture();
    let hooked_ok = unsafe { SAVE.hooked()(port_guide.0 as *const u32, std::ptr::null()) };
    let hooked_bytes = io_redirect::end_capture();
    if real_ok != port_ok || real_ok != hooked_ok {
        failures.push(format!("save: real returned {real_ok}, port {port_ok}, hooked {hooked_ok}"));
    }
    if real_bytes != port_bytes || real_bytes != hooked_bytes {
        failures.push(format!("save: real wrote {} bytes, port {}, hooked {}, or the bytes differ", real_bytes.len(), port_bytes.len(), hooked_bytes.len()));
    }
    let expected_prefix = [
        0x04u8, 0x03, 0x02, 0x01, 0x14, 0x13, 0x12, 0x11, 3, 0, 0, 0, 11, 0, 0, 0, 22, 0, 0, 0, 33, 0, 0, 0, 1,
    ];
    if !port_bytes.starts_with(&expected_prefix) {
        failures.push(format!("save: leading bytes {:02x?}, expected {:02x?}", &port_bytes[..port_bytes.len().min(25)], expected_prefix));
    }

    let ids = [101u32, 102, 103];
    for version in [0x0fu32, 0x10, 0x11, 0x35, 0x36, 0x40, 0x41, 0x42, 0x43] {
        let exact = guide_stream(version, &ids);
        debug_assert_eq!(exact.len(), guide_stream_length(version, ids.len()));
        let mut streams = vec![("exact", exact.clone())];
        if !exact.is_empty() {
            streams.push(("short", exact[..exact.len() - 1].to_vec()));
        }
        if version >= 0x41 {
            streams.push(("cut in list", exact[..exact.len().min(4 + 4 + 4 + 4 + 2)].to_vec()));
            let mut claims_more = guide_stream(version, &ids);
            claims_more[8..12].copy_from_slice(&5u32.to_le_bytes());
            streams.push(("claims five", claims_more));
            let mut cut_at_count = guide_stream(version, &ids);
            cut_at_count.truncate(10);
            streams.push(("cut in count", cut_at_count));
        }
        for (stream_label, bytes) in streams {
            let case = format!("load v{version:#x} ({stream_label})");
            let mut outcomes = Vec::new();
            let mut fixtures = Vec::new();
            for body in 0..3u32 {
                let guide = live_support::build_standalone_guide(guide_type);
                set_followers(guide, &[9, 9]);
                let (result, consumed) = run_load(guide, bytes.clone(), version, body);
                let ids = follower_ids(guide, &mut failures, &format!("{case} body {body}"));
                outcomes.push((result, consumed, ids));
                fixtures.push(guide);
            }
            if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                failures.push(format!("{case}: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
            }
            compare_memory(&case, fixtures[0], fixtures[1], &shared, &mut failures);
            compare_memory(&format!("{case} hooked"), fixtures[0], fixtures[2], &shared, &mut failures);
        }
    }

    let source = live_support::build_standalone_guide(guide_type);
    dirty(source);
    io_redirect::begin_capture();
    real::save(source.0, std::ptr::null());
    let saved = io_redirect::end_capture();
    let (real_guide, port_guide) = (live_support::build_standalone_guide(guide_type), live_support::build_standalone_guide(guide_type));
    let (real_result, real_consumed) = run_load(real_guide, saved.clone(), 0x5d, 0);
    let (port_result, port_consumed) = run_load(port_guide, saved, 0x5d, 1);
    if real_result != port_result || real_consumed != port_consumed {
        failures.push(format!("round trip: real {real_result}/{real_consumed}, port {port_result}/{port_consumed}"));
    }
    let restored = follower_ids(port_guide, &mut failures, "round trip port");
    if restored != [11, 22, 33] || port_guide.crowd_check_countdown() != 0x1112_1314 {
        failures.push(format!("round trip: the port restored followers {restored:?}, countdown {:#x}", port_guide.crowd_check_countdown()));
    }
    compare_memory("round trip", real_guide, port_guide, &shared, &mut failures);
    finish_test(test_name, failures, failure_log)
}
