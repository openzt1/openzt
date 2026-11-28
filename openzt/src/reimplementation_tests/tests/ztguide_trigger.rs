//! Live checks for `ztguide.rs`'s `triggerNearbyGuests`, plus the follow-the-guide harness the `doConditionals` test
//! shares. A recording override on `ZTGuest::followGuide` stands in for it: it logs the guest's id and appends that id
//! to the guide's follower list (vanilla's `addGuest`), so the "group still has room" re-check sees the group grow.
//! Fake guests are planted in the entity lists of real map tiles and removed again. `followGuide` itself is covered
//! by `ztguest_viewing.rs`.

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

use openzt_detour::generated::{msvc_std_listuint::INSERT as LIST_UINT_INSERT, ztguide::TRIGGER_NEARBY_GUESTS};

use super::{
    ztguide::{base, compare_memory, pin_rng, place_at_tile, rng},
    ztguide_lifecycle::follower_ids,
};
use crate::{
    globals::globals,
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguide::{
        live_support::{self, real},
        ZTGuide, FOLLOWERS_OFFSET,
    },
    ztkeeper::unit_offsets,
};

use crate::vanilla_list::FREELIST_12_RVA;
/// The fake guests' and the guide's own entity ids.
pub(super) const GUIDE_ID: u32 = 0x1234;
const GUEST_STATE: u32 = 0x320;
const GUEST_FOLLOWING_FLAG: u32 = 0x337;
const GUEST_GUIDE_ID: u32 = 0x2e4;
/// How many bytes of the real guide type a clone copies: everything up to and including `cMaxGroupSize`.
const GUIDE_TYPE_COPY_BYTES: usize = 0x20c;

static FOLLOW_INTERCEPT: AtomicU32 = AtomicU32::new(0);
static FOLLOW_LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// The `followGuide` override: while the intercept is on it logs the guest's id and appends it to the guide's follower
/// list (vanilla's `addGuest`) instead of running the guest's follow logic.
fn follow_override(this: u32, guide: u32) -> bool {
    if FOLLOW_INTERCEPT.load(Ordering::SeqCst) == 0 {
        return false;
    }
    let guest_id: u32 = get_from_memory(this + unit_offsets::ENTITY_ID);
    FOLLOW_LOG.lock().unwrap().push(guest_id);
    let sentinel: u32 = get_from_memory(guide + FOLLOWERS_OFFSET);
    let mut result: u32 = 0;
    unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, &guest_id as *const u32 as *const i32) };
    true
}

/// Installs the `followGuide` override once; returns an error message on failure.
pub(super) fn install_follow_hook() -> Result<(), String> {
    *crate::ztguest::live_support::viewing_overrides::FOLLOW.lock().map_err(|e| format!("follow override lock poisoned: {e}"))? = Some(follow_override);
    Ok(())
}

pub(super) fn set_follow_intercept(on: bool) {
    FOLLOW_INTERCEPT.store(on as u32, Ordering::SeqCst);
}

/// The guest ids `followGuide` was called with since the last call, in order.
pub(super) fn take_follow_log() -> Vec<u32> {
    std::mem::take(&mut *FOLLOW_LOG.lock().unwrap())
}

extern "thiscall" fn fake_cast_true(_this: u32, _tag: u32) -> u32 {
    1
}

extern "thiscall" fn fake_cast_false(_this: u32, _tag: u32) -> u32 {
    0
}

fn fake_entity_type(cast: bool) -> u32 {
    let mut vtable = vec![0u32; 16];
    vtable[7] = if cast { fake_cast_true as *const () as usize as u32 } else { fake_cast_false as *const () as usize as u32 };
    let vtable = Box::leak(vtable.into_boxed_slice());
    let ty = Box::leak(vec![0u32; 0x80].into_boxed_slice());
    ty[0] = vtable.as_ptr() as u32;
    ty.as_ptr() as u32
}

/// A leaked fake guest (`is_guest`) or non-guest entity with id `id`, `+0x320` set to `state`, `+0x337` to `flag` and
/// following the guide `guide_id`.
pub(super) fn fake_guest(id: u32, is_guest: bool, state: u32, flag: u8, guide_id: u32) -> u32 {
    let entity = Box::leak(vec![0u8; 0x400].into_boxed_slice());
    let address = entity.as_ptr() as u32;
    save_to_memory(address + unit_offsets::ENTITY_ID, id);
    save_to_memory(address + unit_offsets::ENTITY_TYPE, fake_entity_type(is_guest));
    save_to_memory(address + GUEST_STATE, state);
    save_to_memory(address + GUEST_FOLLOWING_FLAG, flag);
    save_to_memory(address + GUEST_GUIDE_ID, guide_id);
    address
}

/// Appends `entity` to `tile`'s entity list through vanilla's `list<uint>::insert`.
pub(super) fn plant(tile: u32, entity: u32) {
    let sentinel: u32 = get_from_memory(tile);
    let mut result: u32 = 0;
    unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, &entity as *const u32 as *const i32) };
}

/// Unlinks `entity`'s node from `tile`'s entity list and returns the node to vanilla's freelist.
pub(super) fn unplant(tile: u32, entity: u32) {
    let sentinel: u32 = get_from_memory(tile);
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        let next: u32 = get_from_memory(node);
        if get_from_memory::<u32>(node + 8) == entity {
            let previous: u32 = get_from_memory(node + 4);
            save_to_memory(previous, next);
            save_to_memory(next + 4, previous);
            let head = base() + FREELIST_12_RVA;
            save_to_memory(node, get_from_memory::<u32>(head));
            save_to_memory(head, node);
            return;
        }
        node = next;
    }
}

/// A leaked copy of the live guide type with its name set to `name` and `cCrowdCheck`, `cCrowdRadius`,
/// `cFollowChance` and `cMaxGroupSize` set as given, so a test never edits the real type.
pub(super) fn cloned_guide_type(real_type: u32, name: &[u8], period: i32, radius: i32, chance: i32, max_group: i32) -> u32 {
    let clone = Box::leak(vec![0u8; 0x400].into_boxed_slice());
    let address = clone.as_ptr() as u32;
    unsafe { std::ptr::copy_nonoverlapping(real_type as *const u8, clone.as_mut_ptr(), GUIDE_TYPE_COPY_BYTES) };
    let name: &'static [u8] = Box::leak(name.to_vec().into_boxed_slice());
    save_to_memory(address + 0x98, name.as_ptr() as u32);
    save_to_memory(address + 0x9c, name.as_ptr() as u32 + name.len() as u32);
    save_to_memory(address + 0x1fc, period);
    save_to_memory(address + 0x200, radius);
    save_to_memory(address + 0x204, chance);
    save_to_memory(address + 0x208, max_group);
    address
}

/// `ZTGUIDE_TRIGGER_NEARBY_GUESTS_LIVE`: `triggerNearbyGuests` on a guide at the middle of the map and at the corner,
/// over a fake type named `"f"`, `"m"`, `"x"` and `"ff"`, follow chances `0`/`40`/`100`, group sizes `1`/`3`/`6`, radii
/// `-1`/`0`/`1`/`2` and two generator seeds, with six fake entities planted around the guide (a guest in each state, a
/// guest already following this guide, one outside the radius, one that is not a guest). Real body, port and hooked
/// address log the same `followGuide` calls in the same order and leave the same followers, generator state and guide
/// memory. Fails if fewer than six distinct follow sequences occurred.
pub(crate) fn run_ztguide_trigger_nearby_guests_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_TRIGGER_NEARBY_GUESTS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(message) = install_follow_hook() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let Some(real_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let (cx, cy) = (world.map_x_size / 2, world.map_y_size / 2);
    let seed = rng();

    let center = world.get_tile_ptr(cx, cy);
    let corner = world.get_tile_ptr(0, 0);
    let planted: Vec<(u32, u32)> = vec![
        (center, fake_guest(0x3001, true, 0, 1, 0)),
        (center, fake_guest(0x3002, false, 0, 1, 0)),
        (world.get_tile_ptr(cx + 1, cy), fake_guest(0x3003, true, 1, 1, 0)),
        (world.get_tile_ptr(cx, cy - 1), fake_guest(0x3004, true, 0, 0, GUIDE_ID)),
        (world.get_tile_ptr(cx + 1, cy + 1), fake_guest(0x3005, true, 0, 1, 0)),
        (world.get_tile_ptr(cx + 3, cy), fake_guest(0x3006, true, 1, 1, 0)),
        (corner, fake_guest(0x3007, true, 1, 1, 0)),
        (world.get_tile_ptr(1, 1), fake_guest(0x3008, true, 0, 0, GUIDE_ID)),
    ];
    for (tile, entity) in &planted {
        plant(*tile, *entity);
    }

    set_follow_intercept(true);
    let mut sequences = std::collections::BTreeSet::new();
    let mut run = |label: String, position: (u32, u32), guide_type: u32, pole_seed: u32, failures: &mut Vec<String>| {
        let fixtures: Vec<ZTGuide> = (0..3).map(|_| live_support::build_standalone_guide(guide_type)).collect();
        for guide in &fixtures {
            save_to_memory(guide.0 + unit_offsets::ENTITY_ID, GUIDE_ID);
            place_at_tile(*guide, position);
        }
        let mut outcomes = Vec::new();
        for (pole, guide) in fixtures.iter().enumerate() {
            pin_rng(pole_seed);
            take_follow_log();
            match pole {
                0 => real::trigger_nearby_guests(guide.0),
                1 => guide.trigger_nearby_guests(),
                _ => unsafe { TRIGGER_NEARBY_GUESTS.hooked()(guide.0 as *const u32) },
            }
            let log = take_follow_log();
            let followers = follower_ids(*guide, failures, &format!("{label} pole {pole}"));
            outcomes.push((log, followers, rng()));
        }
        if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
            failures.push(format!("{label}: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
        }
        compare_memory(&label, fixtures[0], fixtures[1], &[guide_type], failures);
        compare_memory(&format!("{label} hooked"), fixtures[0], fixtures[2], &[guide_type], failures);
        sequences.insert(outcomes[0].0.clone());
    };

    for name in [&b"f"[..], b"m", b"x", b"ff"] {
        for chance in [0, 40, 100] {
            for max_group in [1, 3, 6] {
                for pole_seed in [seed, seed ^ 0x5a5a_a5a5] {
                    let guide_type = cloned_guide_type(real_type, name, 7, 1, chance, max_group);
                    run(
                        format!("name {:?}, chance {chance}, max {max_group}, seed {pole_seed:#x}", String::from_utf8_lossy(name)),
                        (cx, cy),
                        guide_type,
                        pole_seed,
                        &mut failures,
                    );
                }
            }
        }
    }
    for radius in [-1, 0, 2] {
        let guide_type = cloned_guide_type(real_type, b"f", 7, radius, 50, 6);
        run(format!("radius {radius} at the center"), (cx, cy), guide_type, seed, &mut failures);
    }
    for radius in [1, 2] {
        let guide_type = cloned_guide_type(real_type, b"m", 7, radius, 50, 6);
        run(format!("radius {radius} at the corner"), (0, 0), guide_type, seed, &mut failures);
    }

    set_follow_intercept(false);
    for (tile, entity) in &planted {
        unplant(*tile, *entity);
    }
    pin_rng(seed);
    if sequences.len() < 6 {
        failures.push(format!("only {} distinct follow sequences occurred: {sequences:?}", sequences.len()));
    }
    finish_test(test_name, failures, failure_log)
}

/// What the fake guests' virtual slots saw during one `followGuide` run: `(slot name, guest id, argument)`.
static REAL_FOLLOW_LOG: Mutex<Vec<(&'static str, u32, u32)>> = Mutex::new(Vec::new());
static REAL_FOLLOW_HABITAT: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn slot_returns_false(_this: u32) -> u32 {
    0
}

extern "thiscall" fn slot_goal_set(this: u32, goal: u32, subtype: u32) {
    let id: u32 = get_from_memory(this + unit_offsets::ENTITY_ID);
    REAL_FOLLOW_LOG.lock().unwrap().push(("goalSet", id, goal * 0x100 + subtype));
}

extern "thiscall" fn slot_set_target_tile(this: u32, tile: u32, flag: u32) {
    let id: u32 = get_from_memory(this + unit_offsets::ENTITY_ID);
    REAL_FOLLOW_LOG.lock().unwrap().push(("setTargetTile", id, tile + flag));
}

extern "thiscall" fn slot_habitat_from_tile(_this: u32, _tile: u32) -> u32 {
    REAL_FOLLOW_HABITAT.load(Ordering::SeqCst)
}

/// A fake guest whose virtual slots `followGuide` calls (`goalSet`, `getHabitatFromTile`, `isDoingSomething`,
/// `hasPressingNeed`, `setTargetTile`) answer from the functions above.
fn fake_following_guest(id: u32, state: u32, flag: u8, guide_id: u32) -> u32 {
    let guest = fake_guest(id, true, state, flag, guide_id);
    let mut vtable = vec![0u32; 0xc0];
    vtable[0x1d8 / 4] = slot_goal_set as *const () as usize as u32;
    vtable[0x248 / 4] = slot_habitat_from_tile as *const () as usize as u32;
    vtable[0x288 / 4] = slot_returns_false as *const () as usize as u32;
    vtable[0x28c / 4] = slot_returns_false as *const () as usize as u32;
    vtable[0x2d8 / 4] = slot_set_target_tile as *const () as usize as u32;
    save_to_memory(guest, Box::leak(vtable.into_boxed_slice()).as_ptr() as u32);
    guest
}

/// `ZTGUIDE_TRIGGER_REAL_FOLLOW_LIVE`: `triggerNearbyGuests` with no `followGuide` override, so the ported
/// `followGuide` runs on every guest the trigger offers a guide to. The guests are fakes with a recording vtable,
/// planted on real map tiles; `getPath` and `goToDest` succeed without touching them, and the guide's destination is a
/// real tile so the neighbour search runs against the live map. The guide is on guide duties. Each pole starts from a
/// restored copy of every guest, and the test compares what the guests' slots saw, the guests' memory afterwards, the
/// guide's follower list (grown by the real `addGuest` through the real `followGuide`), the generator state and the
/// guide's memory, for the real body, the port and the hooked address. Fails if fewer than three distinct follower
/// lists occurred, or no case followed anyone.
pub(crate) fn run_ztguide_trigger_real_follow_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use super::ztguest_checks::{install_hooks, FOLLOW_SANDBOX};

    const GUEST_BYTES: usize = 0x400;
    const GUIDE_DESTINATION: u32 = 0x180;
    const GOAL_GUIDE_DUTIES: u32 = 0x76;
    let test_name = "ZTGUIDE_TRIGGER_REAL_FOLLOW_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(message) = install_hooks().and_then(|()| install_follow_hook()) {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let Some(real_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let (cx, cy) = (world.map_x_size / 2, world.map_y_size / 2);
    let seed = rng();
    let destination = world.get_tile_ptr(cx + 5, cy + 5);

    let mut exhibit = vec![0u32; 0x40];
    exhibit[0x2c / 4] = 1;
    REAL_FOLLOW_HABITAT.store(exhibit.as_ptr() as u32, Ordering::SeqCst);

    let planted: Vec<(u32, u32)> = vec![
        (world.get_tile_ptr(cx, cy), fake_following_guest(0x3001, 0, 0, 0)),
        (world.get_tile_ptr(cx, cy), fake_following_guest(0x3002, 0, 0, 0)),
        (world.get_tile_ptr(cx + 1, cy), fake_following_guest(0x3003, 1, 0, 0)),
        (world.get_tile_ptr(cx, cy - 1), fake_following_guest(0x3004, 0, 0, GUIDE_ID)),
        (world.get_tile_ptr(cx + 1, cy + 1), fake_following_guest(0x3005, 0, 0, 0)),
        (world.get_tile_ptr(cx - 1, cy), fake_following_guest(0x3006, 0, 1, 0x9999)),
        (world.get_tile_ptr(cx + 3, cy), fake_following_guest(0x3007, 1, 0, 0)),
    ];
    for (tile, entity) in &planted {
        plant(*tile, *entity);
    }
    let pristine: Vec<Vec<u8>> = planted.iter().map(|&(_, guest)| (0..GUEST_BYTES as u32).map(|i| get_from_memory::<u8>(guest + i)).collect()).collect();
    let restore = |snapshots: &[Vec<u8>]| {
        for (&(_, guest), bytes) in planted.iter().zip(snapshots) {
            for (i, byte) in bytes.iter().enumerate() {
                save_to_memory(guest + i as u32, *byte);
            }
        }
    };
    let snapshot = || -> Vec<Vec<u8>> { planted.iter().map(|&(_, guest)| (0..GUEST_BYTES as u32).map(|i| get_from_memory::<u8>(guest + i)).collect()).collect() };

    set_follow_intercept(false);
    FOLLOW_SANDBOX.store(true, Ordering::SeqCst);
    let mut follower_sets = std::collections::BTreeSet::new();
    let mut followed_anyone = false;
    for name in [&b"f"[..], b"x"] {
        for chance in [40, 100] {
            for max_group in [1, 3, 6] {
                for pole_seed in [seed, seed ^ 0x5a5a_a5a5] {
                    let guide_type = cloned_guide_type(real_type, name, 7, 1, chance, max_group);
                    let label = format!("name {:?}, chance {chance}, max {max_group}, seed {pole_seed:#x}", String::from_utf8_lossy(name));
                    let fixtures: Vec<ZTGuide> = (0..3).map(|_| live_support::build_standalone_guide(guide_type)).collect();
                    for guide in &fixtures {
                        save_to_memory(guide.0 + unit_offsets::ENTITY_ID, GUIDE_ID);
                        save_to_memory(guide.0 + unit_offsets::GOAL_ID, GOAL_GUIDE_DUTIES);
                        save_to_memory(guide.0 + GUIDE_DESTINATION, destination);
                        place_at_tile(*guide, (cx, cy));
                    }
                    let mut outcomes = Vec::new();
                    for (pole, guide) in fixtures.iter().enumerate() {
                        restore(&pristine);
                        pin_rng(pole_seed);
                        REAL_FOLLOW_LOG.lock().unwrap().clear();
                        match pole {
                            0 => real::trigger_nearby_guests(guide.0),
                            1 => guide.trigger_nearby_guests(),
                            _ => unsafe { TRIGGER_NEARBY_GUESTS.hooked()(guide.0 as *const u32) },
                        }
                        let log = std::mem::take(&mut *REAL_FOLLOW_LOG.lock().unwrap());
                        let followers = follower_ids(*guide, &mut failures, &format!("{label} pole {pole}"));
                        outcomes.push((log, followers, rng(), snapshot()));
                    }
                    restore(&pristine);
                    if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                        failures.push(format!("{label}: real {:?}/{:?}, port {:?}/{:?}, hooked {:?}/{:?}", outcomes[0].0, outcomes[0].1, outcomes[1].0, outcomes[1].1, outcomes[2].0, outcomes[2].1));
                    }
                    compare_memory(&label, fixtures[0], fixtures[1], &[guide_type], &mut failures);
                    compare_memory(&format!("{label} hooked"), fixtures[0], fixtures[2], &[guide_type], &mut failures);
                    followed_anyone |= !outcomes[0].1.is_empty();
                    follower_sets.insert(outcomes[0].1.clone());
                }
            }
        }
    }
    FOLLOW_SANDBOX.store(false, Ordering::SeqCst);
    for (tile, entity) in &planted {
        unplant(*tile, *entity);
    }
    pin_rng(seed);
    drop(exhibit);
    if !followed_anyone {
        failures.push("no case made a guest follow the guide".to_string());
    }
    if follower_sets.len() < 3 {
        failures.push(format!("only {} distinct follower lists occurred: {follower_sets:?}", follower_sets.len()));
    }
    finish_test(test_name, failures, failure_log)
}
