//! Live checks for `ztguest/inventory.rs`. Guests built by the real constructor over (cloned) guest types are planted
//! with identical pseudo-random state; the real body, the port and the hooked address run on three of them and the
//! results, the shared RNG state and the guests' memory and containers are compared.

use std::fmt::Debug;

use openzt_detour::generated::ztguest::{
    ADD_CHANGES, ADD_VISITED_SITE, GET_GAWK_TIME, GET_ITEM, GET_SCENERY, LEAVING_SHOW, REMOVE_ITEM, SEEN_ATTRACTION, SEEN_HABITAT, SEEN_SHOW,
};

use super::{
    ztguest_leaf::{compare_guests, Lcg},
    ztguest_lifecycle::container_contents,
    ztguide::{base, pin_rng, rng},
};
use crate::{
    globals::globals,
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguest::{
        guest_offsets,
        live_support::{self, inventory_real as real},
        ZTGuest,
    },
    ztkeeper::unit_offsets,
};

const ROUNDS: u32 = 300;

fn leaked(bytes: usize) -> u32 {
    Box::leak(vec![0u8; bytes].into_boxed_slice()).as_ptr() as u32
}

extern "thiscall" fn slot_false(_this: u32) -> u32 {
    0
}

static HABITAT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

extern "thiscall" fn slot_habitat(_this: u32) -> u32 {
    HABITAT.load(std::sync::atomic::Ordering::SeqCst)
}

fn install_fake_vtable(guest: ZTGuest) {
    let real_vtable: u32 = get_from_memory(guest.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x264 / 4] = slot_false as *const () as usize as u32;
    vtable[0x240 / 4] = slot_habitat as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(guest.0, vtable.as_ptr() as u32);
}

extern "thiscall" fn cast_true(_this: u32, _tag: u32) -> u32 {
    1
}

extern "thiscall" fn cast_false(_this: u32, _tag: u32) -> u32 {
    0
}

/// A leaked fake entity whose type answers every class test with `cast`.
fn fake_entity(cast: bool) -> u32 {
    let mut vtable = vec![0u32; 0x100];
    vtable[7] = if cast { cast_true as *const () as usize as u32 } else { cast_false as *const () as usize as u32 };
    let vtable = Box::leak(vtable.into_boxed_slice());
    let entity_type = leaked(0x40);
    save_to_memory(entity_type, vtable.as_ptr() as u32);
    let entity = leaked(0x400);
    save_to_memory(entity + unit_offsets::ENTITY_TYPE, entity_type);
    entity
}

fn plant(guest: ZTGuest, guest_type: u32, rng: &mut Lcg) {
    save_to_memory(guest.0 + unit_offsets::ENTITY_TYPE, guest_type);
    save_to_memory(guest.0 + 0x324, (rng.next() % 3) as i32);
    save_to_memory(guest.0 + guest_offsets::VIEWING_AREA_ID, -1i32);
    live_support::forget_standalone_guest(guest);
    let items = leaked(0x30);
    for index in 0..5u32 {
        save_to_memory(items + index * 4, rng.next() % 4);
    }
    save_to_memory(guest.0 + guest_offsets::ITEMS, items);
    save_to_memory(guest.0 + guest_offsets::ITEMS + 4, items + 20);
    save_to_memory(guest.0 + guest_offsets::ITEMS + 8, items + 20);
    for offset in [0x2acu32, 0x2b4, 0x2bc, 0x2c4, 0x2cc] {
        save_to_memory(guest.0 + offset, (rng.next() % 50) as i32 - 20);
    }
    // A two-by-two grid of buildings and sceneries; entries are fakes that do or do not pass the class test.
    for offset in [0x244u32, 0x234] {
        let grid = leaked(0x20);
        for cell in 0..4u32 {
            let entity = match rng.next() % 3 {
                0 => 0,
                1 => fake_entity(true),
                _ => fake_entity(false),
            };
            save_to_memory(grid + 4 + cell * 4, entity);
        }
        save_to_memory(guest.0 + offset, grid);
    }
    for offset in [0x214u32, 0x218, 0x21c] {
        save_to_memory(guest.0 + offset, (rng.next() % 90) as i32 - 5);
    }
    install_fake_vtable(guest);
}

/// A cloned guest type whose happiness bound and gawk tables are random.
fn cloned_type(guest_type: u32, rng: &mut Lcg) -> u32 {
    let copy = leaked(0x300);
    for offset in 0..0x2e0u32 {
        save_to_memory(copy + offset, get_from_memory::<u8>(guest_type + offset));
    }
    save_to_memory(copy + 0x1ec, (rng.next() % 200) as i32 - 100);
    for pointer in [0x2d0u32, 0x2dc] {
        let table = leaked(0x40);
        for index in 0..6u32 {
            save_to_memory(table + index * 4, (rng.next() % 50) as i32);
        }
        save_to_memory(copy + pointer, table);
    }
    copy
}

fn compare<R: PartialEq + Debug>(
    name: &str,
    guest_type: u32,
    failures: &mut Vec<String>,
    prepare: impl Fn(ZTGuest, &mut Lcg),
    real_body: impl Fn(ZTGuest) -> R,
    port_body: impl Fn(ZTGuest) -> R,
    hooked_body: impl Fn(ZTGuest) -> R,
) {
    for round in 0..ROUNDS {
        let seed = 0x500 + round * 31;
        let mut guests = Vec::new();
        let mut outcomes = Vec::new();
        for pole in 0..3 {
            let mut generator = Lcg(seed);
            let type_clone = cloned_type(guest_type, &mut generator);
            let guest = live_support::build_standalone_guest(guest_type);
            plant(guest, type_clone, &mut generator);
            prepare(guest, &mut generator);
            pin_rng(seed ^ 0x77);
            let result = match pole {
                0 => real_body(guest),
                1 => port_body(guest),
                _ => hooked_body(guest),
            };
            outcomes.push((result, rng(), container_contents(guest)));
            guests.push(guest);
        }
        let label = format!("{name} round {round}");
        if outcomes[0] != outcomes[1] {
            failures.push(format!("{label}: real {:?}, port {:?}", outcomes[0], outcomes[1]));
        }
        if outcomes[0] != outcomes[2] {
            failures.push(format!("{label}: real {:?}, hooked {:?}", outcomes[0], outcomes[2]));
        }
        compare_guests(&label, guests[0], guests[1], &[], failures);
        compare_guests(&format!("{label} hooked"), guests[0], guests[2], &[], failures);
        if failures.len() > 8 {
            return;
        }
    }
}

/// A leaked `{begin, end}` string object holding `text`.
fn string_object(text: &[u8]) -> u32 {
    let bytes = leaked(text.len() + 1);
    for (i, byte) in text.iter().enumerate() {
        save_to_memory(bytes + i as u32, *byte);
    }
    let object = leaked(0x10);
    save_to_memory(object, bytes);
    save_to_memory(object + 4, bytes + text.len() as u32);
    object
}

/// `ZTGUEST_INVENTORY_LIVE`: the inventory and seen-list methods against the real bodies.
pub(crate) fn run_ztguest_inventory_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_INVENTORY_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let saved_rng = rng();
    let this = |guest: ZTGuest| guest.0 as *const u32;
    let habitat_manager = globals().zthabitatmgr();
    let habitat = if habitat_manager.exhibit_array.len() > 0 { habitat_manager.exhibit_array.get_ptr(0) } else { 0 };
    HABITAT.store(habitat, std::sync::atomic::Ordering::SeqCst);
    let map_x: i32 = get_from_memory(globals().ztworldmgr_ptr() as u32 + 0x34);
    let map_y: i32 = get_from_memory(globals().ztworldmgr_ptr() as u32 + 0x38);
    let live_entity = live_support::find_live_guests().first().map(|(entity, _)| *entity).unwrap_or(0);

    // A record of six words (happiness for modes 0 and 1, then hunger, thirst, bathroom, energy).
    let records: Vec<u32> = (0..ROUNDS).map(|_| leaked(0x20)).collect();
    for (index, record) in records.iter().enumerate() {
        let mut generator = Lcg(index as u32 + 9);
        for word in 0..6u32 {
            save_to_memory(record + word * 4, (generator.next() % 60) as i32 - 30);
        }
    }
    let record_for = |guest: ZTGuest| records[(get_from_memory::<i32>(guest.0 + 0x2ac).unsigned_abs() as usize) % records.len()];
    compare(
        "addChanges",
        guest_type,
        &mut failures,
        |_, _| {},
        |guest| real::add_changes(guest.0, record_for(guest)),
        |guest| guest.add_changes(record_for(guest)),
        |guest| unsafe { ADD_CHANGES.hooked()(this(guest), record_for(guest) as *const u32) },
    );

    // Visited sites: a run of appends, enough to pass the ten-record cap.
    let appends = |guest: ZTGuest, call: &dyn Fn(ZTGuest, u32, u32, u32)| {
        let mut generator = Lcg(guest.0 ^ 0); // replaced below: sites derive from the round via the counter word
        let _ = &mut generator;
        for n in 0..14u32 {
            call(guest, n * 3, n * 5 + 1, n * 7 + 2);
        }
    };
    compare(
        "addVisitedSite",
        guest_type,
        &mut failures,
        |_, _| {},
        |guest| {
            appends(guest, &|g, a, b, c| real::add_visited_site(g.0, a, b, c));
            get_from_memory::<u16>(guest.0 + 0x334)
        },
        |guest| {
            appends(guest, &|g, a, b, c| g.add_visited_site(a, b, c));
            get_from_memory::<u16>(guest.0 + 0x334)
        },
        |guest| {
            appends(guest, &|g, a, b, c| unsafe { ADD_VISITED_SITE.hooked()(this(g), a, b, c) });
            get_from_memory::<u16>(guest.0 + 0x334)
        },
    );

    let names: Vec<u32> = [0x23c4e8u32, 0x23c4f8, 0x23c4c8, 0x23b058, 0x23c528, 0x23c508].iter().map(|rva| base() + rva).chain([string_object(b"nothing"), string_object(b"")]).collect();
    let name_for = |guest: ZTGuest| names[(get_from_memory::<u32>(guest.0 + 0x214) as usize) % names.len()];
    compare(
        "getItem",
        guest_type,
        &mut failures,
        |_, _| {},
        |guest| real::get_item(guest.0, name_for(guest)),
        |guest| guest.get_item(name_for(guest)),
        |guest| unsafe { GET_ITEM.hooked()(this(guest), name_for(guest) as *const i8) },
    );
    let item_for = |guest: ZTGuest| get_from_memory::<i32>(guest.0 + 0x218) % 5;
    compare(
        "removeItem",
        guest_type,
        &mut failures,
        |_, _| {},
        |guest| real::remove_item(guest.0, item_for(guest)),
        |guest| guest.remove_item(item_for(guest)),
        |guest| unsafe { REMOVE_ITEM.hooked()(this(guest), item_for(guest)) },
    );

    // Seen lists over sites inside the map.
    let sites = |guest: ZTGuest, generator: &mut Lcg| {
        for _ in 0..(generator.next() % 6) {
            let (x, y) = ((generator.next() as i32).rem_euclid(map_x.max(1)) as u32, (generator.next() as i32).rem_euclid(map_y.max(1)) as u32);
            guest.add_visited_site(x, y, 0);
        }
    };
    let manager = globals().zthabitatmgr_ptr() as u32;
    let habitat_arg = |guest: ZTGuest| -> u32 {
        match get_from_memory::<u32>(guest.0 + 0x214) % 3 {
            0 => 0,
            1 => habitat,
            _ => {
                let record = guest.0 + guest_offsets::VISITED_SITES;
                let first: u32 = get_from_memory(record);
                if first == 0 {
                    habitat
                } else {
                    unsafe { openzt_detour::generated::zthabitatmgr::GET_HABITAT.hooked()(manager as *const u32, get_from_memory(first), get_from_memory(first + 4)) }
                }
            }
        }
    };
    compare(
        "seenHabitat",
        guest_type,
        &mut failures,
        sites,
        |guest| real::seen_habitat(guest.0, habitat_arg(guest)),
        |guest| guest.seen_habitat(habitat_arg(guest)),
        |guest| unsafe { SEEN_HABITAT.hooked()(this(guest), habitat_arg(guest) as *const u32) },
    );
    compare(
        "seenShow",
        guest_type,
        &mut failures,
        sites,
        |guest| real::seen_show(guest.0, live_entity as i32),
        |guest| guest.seen_show(live_entity as i32),
        |guest| unsafe { SEEN_SHOW.hooked()(this(guest), live_entity as i32) },
    );
    let scenery_arg = |guest: ZTGuest| if get_from_memory::<u32>(guest.0 + 0x214) % 2 == 0 { 0 } else { live_entity };
    compare(
        "seenAttraction",
        guest_type,
        &mut failures,
        sites,
        |guest| real::seen_attraction(guest.0, scenery_arg(guest)),
        |guest| guest.seen_attraction(scenery_arg(guest)),
        |guest| unsafe { SEEN_ATTRACTION.hooked()(this(guest), scenery_arg(guest) as *const u32) },
    );
    compare("leavingShow", guest_type, &mut failures, |_, _| {}, |guest| real::leaving_show(guest.0), |guest| guest.leaving_show(), |guest| unsafe {
        LEAVING_SHOW.hooked()(this(guest))
    });
    compare("getScenery", guest_type, &mut failures, |_, _| {}, |guest| real::get_scenery(guest.0) != 0, |guest| guest.get_scenery() != 0, |guest| unsafe {
        GET_SCENERY.hooked()(this(guest)) != 0
    });
    compare("getGawkTime", guest_type, &mut failures, |_, _| {}, |guest| real::get_gawk_time(guest.0), |guest| guest.get_gawk_time(), |guest| unsafe {
        GET_GAWK_TIME.hooked()(this(guest))
    });
    pin_rng(saved_rng);
    finish_test(test_name, failures, failure_log)
}
