//! Live checks for `ztguest/viewing.rs` against the loaded zoo: `checkViewingArea`, `findViewingArea` and
//! `startPuking`. Guests come from the real vanilla constructor; the real body, the port and the hooked address run on
//! the same guest and tile and results, the guest's viewing-area fields and the shared RNG are compared. The scripted
//! cases (`followGuide`, `viewNearbyExhibit`, `doBuilding`, `setVisible`, `runScreaming`) are in `ztguest_checks.rs`.

use openzt_detour::generated::{
    ztguest::{
        CHECK_VIEWING_AREA, FIND_ANIMAL_BY_HAPPINESS, FIND_VIEWING_AREA, F_CROWD_DENSITY, F_DIFFERENT_SPECIES, F_NUM_ANIMALS, F_NUM_SICK_ANIMALS, F_SICK_ANIMALS, START_PUKING,
    },
    ztunit::GET_HABITAT_FROM_TILE,
};

use super::{
    ztguest_leaf::compare_guests,
    ztguide::{pin_rng, rng},
};
use crate::{
    globals::globals,
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguest::{
        live_support::{self, viewing_real as real},
        ZTGuest,
    },
};

const MAP_LIMIT: u32 = 120;
const VIEWING_AREA_DIRECTION: u32 = 0x328;
const VIEWING_AREA: u32 = 0x32c;
const ANIMATION_BEGIN: u32 = 0x54;
const ANIMATION_END: u32 = 0x58;
const SENTINEL: u32 = 0xaaaa_aaaa;

fn place_guest(guest: ZTGuest, tile: (u32, u32)) {
    save_to_memory(guest.0 + 0x114, tile.0 * 64 + 32);
    save_to_memory(guest.0 + 0x118, tile.1 * 64 + 32);
    save_to_memory(guest.0 + 0x11c, 0u32);
}

/// Tiles of the live map inside a habitat, which is what vanilla's viewing-area code needs under the guest.
fn habitat_tiles() -> Vec<(u32, u32)> {
    let world = globals().ztworldmgr();
    let mut tiles = Vec::new();
    for y in 0..world.map_y_size.min(MAP_LIMIT) {
        for x in 0..world.map_x_size.min(MAP_LIMIT) {
            if unsafe { GET_HABITAT_FROM_TILE.hooked()(std::ptr::null(), world.get_tile_ptr(x, y) as *const u32) } != 0 {
                tiles.push((x, y));
            }
        }
    }
    tiles
}

/// `GLOBAL_ZTHabitatMgr` as an RVA; the per-column grid of habitat rows sits at `+0x28`.
const HABITAT_MGR_RVA: u32 = 0x0023_805c;

/// A fake `ZTViewingArea`: clean characteristics (`+0x4c`) and an observable-area vector (`+0x4`/`+0x8`) with one entry
/// (`has_oa`) or none.
fn fake_viewing_area(has_oa: bool) -> u32 {
    let area = Box::leak(vec![0u8; 0x80].into_boxed_slice()).as_ptr() as u32;
    let oa = Box::leak(vec![0u8; 8].into_boxed_slice()).as_ptr() as u32;
    save_to_memory(area + 0x4, oa);
    save_to_memory(area + 0x8, if has_oa { oa + 4 } else { oa });
    area
}

/// Gives `area` a crowd-density vector of `slots` entries and a count of `count`, the two inputs `fCrowdDensity` divides.
fn plant_crowd(area: u32, slots: u32, count: i32) {
    let buffer = Box::leak(vec![0u8; 4 * slots.max(1) as usize].into_boxed_slice()).as_ptr() as u32;
    save_to_memory(area + 0x40, buffer);
    save_to_memory(area + 0x44, buffer + slots * 4);
    save_to_memory(area + 0x54, count);
}

static HABITAT_ANSWER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

extern "thiscall" fn slot_habitat_from_tile(_this: u32, _tile: u32) -> u32 {
    HABITAT_ANSWER.load(std::sync::atomic::Ordering::SeqCst)
}

/// A guest whose habitat slot (`+0x248`) answers [`HABITAT_ANSWER`].
fn guest_with_habitat_slot(guest_type: u32) -> ZTGuest {
    let guest = live_support::build_standalone_guest(guest_type);
    let real_vtable: u32 = get_from_memory(guest.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x248 / 4] = slot_habitat_from_tile as *const () as usize as u32;
    save_to_memory(guest.0, Box::leak(vtable.into_boxed_slice()).as_ptr() as u32);
    guest
}

/// `ZTGUEST_VIEWING_AREA_LIVE`: `checkViewingArea` over every direction (and `-1`) and `findViewingArea` with pinned RNG
/// seeds, real against port and hooked. Guests stand on the live zoo's habitat tiles (real habitat lookup) and on a
/// sample of tiles whose cell in the live habitat grid holds planted viewing areas (with and without an observable
/// area, none in direction 7), the latter with the habitat slot answering a fake exhibit habitat (`+0x2c` set) so the
/// lookup and `hasOA` run. The grid cells are restored afterwards.
pub(crate) fn run_ztguest_viewing_area_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_VIEWING_AREA_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let saved_rng = rng();
    let world = globals().ztworldmgr();
    let manager: u32 = get_from_memory(super::ztguide::base() + HABITAT_MGR_RVA);
    if manager == 0 {
        failures.push("the live world has no habitat manager".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let columns: u32 = get_from_memory(manager + 0x28);
    let fakes = [fake_viewing_area(true), fake_viewing_area(false)];
    let mut area_tiles = vec![(0, 0), (world.map_x_size / 2, world.map_y_size / 2), (world.map_x_size - 1, world.map_y_size - 1)];
    area_tiles.extend(habitat_tiles().into_iter().step_by(15).take(40));
    let mut planted = Vec::new();
    for &(x, y) in &area_tiles {
        let row: u32 = get_from_memory(columns + x * 12);
        for direction in 0..8u32 {
            let slot = row + 4 + (direction + y * 10) * 4;
            planted.push((slot, get_from_memory::<u32>(slot)));
            save_to_memory(slot, if direction == 7 { 0 } else { fakes[(direction % 2) as usize] });
        }
    }
    let fake_habitats = [(1u8, leaked_habitat(1)), (0u8, leaked_habitat(0))];
    let real_guest = live_support::build_standalone_guest(guest_type);
    let slot_guest = guest_with_habitat_slot(guest_type);
    let mut cases: Vec<(ZTGuest, (u32, u32), u32)> = habitat_tiles().into_iter().map(|tile| (real_guest, tile, 0)).collect();
    for &tile in &area_tiles {
        for &(_, habitat) in &fake_habitats {
            cases.push((slot_guest, tile, habitat));
        }
    }
    let mut mismatches = 0;
    let (mut found_checks, mut found_finds) = (0, 0);
    let fields = |guest: ZTGuest| (get_from_memory::<u32>(guest.0 + VIEWING_AREA_DIRECTION), get_from_memory::<u32>(guest.0 + VIEWING_AREA));
    let reset = |guest: ZTGuest| {
        save_to_memory(guest.0 + VIEWING_AREA_DIRECTION, SENTINEL);
        save_to_memory(guest.0 + VIEWING_AREA, SENTINEL);
    };
    for &(guest, (x, y), habitat) in &cases {
        place_guest(guest, (x, y));
        HABITAT_ANSWER.store(habitat, std::sync::atomic::Ordering::SeqCst);
        for direction in (0u32..8).chain([u32::MAX]) {
            reset(guest);
            let real_result = real::check_viewing_area(guest.0, direction);
            let real_fields = fields(guest);
            reset(guest);
            let port_result = guest.check_viewing_area(direction);
            let port_fields = fields(guest);
            reset(guest);
            let hooked_result = unsafe { CHECK_VIEWING_AREA.hooked()(guest.0 as *const u32, direction) };
            let hooked_fields = fields(guest);
            found_checks += real_result as u32;
            if real_result != port_result || real_result != hooked_result || real_fields != port_fields || real_fields != hooked_fields {
                mismatches += 1;
                if mismatches <= 10 {
                    failures.push(format!(
                        "checkViewingArea ({x},{y}) habitat {habitat:#x} direction {direction:#x}: real {real_result} {real_fields:x?}, port {port_result} {port_fields:x?}, hooked {hooked_result} {hooked_fields:x?}"
                    ));
                }
            }
        }
        for seed in [1u32, 0xdead_beef, 0x1234_5678] {
            let mut poles = Vec::new();
            for pole in 0..3 {
                reset(guest);
                pin_rng(seed);
                let result = match pole {
                    0 => real::find_viewing_area(guest.0),
                    1 => guest.find_viewing_area(),
                    _ => unsafe { FIND_VIEWING_AREA.hooked()(guest.0 as *const u32) },
                };
                poles.push((result, fields(guest), rng()));
            }
            found_finds += poles[0].0 as u32;
            if poles[0] != poles[1] || poles[0] != poles[2] {
                mismatches += 1;
                if mismatches <= 10 {
                    failures.push(format!("findViewingArea ({x},{y}) habitat {habitat:#x} seed {seed:#x}: real {:?}, port {:?}, hooked {:?}", poles[0], poles[1], poles[2]));
                }
            }
        }
    }
    for (slot, original) in planted.into_iter().rev() {
        save_to_memory(slot, original);
    }
    pin_rng(saved_rng);
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if found_checks == 0 || found_finds == 0 {
        failures.push(format!("no viewing area was ever found ({found_checks} checks, {found_finds} finds) over {} planted tiles", area_tiles.len()));
    }
    tracing::info!("{test_name}: {found_checks} checkViewingArea and {found_finds} findViewingArea calls found an area over {} viewing-area tiles", area_tiles.len());
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_VIEWING_STATS_LIVE`: `fNumAnimals`, `fSickAnimals`, `fNumSickAnimals`, `fDifferentSpecies` and
/// `findAnimalByHappiness` (pinned RNG seeds, several happiness ranges) against the real body and the hooked address,
/// for a guest standing on live habitat tiles and looking in every direction. The queries refresh the live viewing
/// area's characteristics when dirty, so the same calls on the same area must agree whichever pole runs first.
pub(crate) fn run_ztguest_viewing_stats_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::ztguest::live_support::checks_real;

    let test_name = "ZTGUEST_VIEWING_STATS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let saved_rng = rng();
    let world = globals().ztworldmgr();
    let manager: u32 = get_from_memory(super::ztguide::base() + HABITAT_MGR_RVA);
    if manager == 0 {
        failures.push("the live world has no habitat manager".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    // The live zoo holds no viewing areas, so plant fakes with known statistics. Their habitat is a live habitat (so
    // `findAnimalByHappiness` walks real animals) or, last, a bare one with none.
    let (habitats_begin, habitats_end): (u32, u32) = (get_from_memory(manager + 0x1c), get_from_memory(manager + 0x20));
    let mut habitats: Vec<u32> = (habitats_begin..habitats_end).step_by(4).map(get_from_memory::<u32>).filter(|habitat| *habitat != 0).take(4).collect();
    habitats.push(leaked_habitat(0));
    let mut fakes = Vec::new();
    for (index, &(animals, sick, species)) in [(0u32, 0u32, 0u32), (3, 0, 2), (5, 2, 4), (1, 1, 1)].iter().enumerate() {
        let area = fake_viewing_area(true);
        save_to_memory(area, habitats[index % habitats.len()]);
        save_to_memory(area + 0x10, species);
        save_to_memory(area + 0x14, animals);
        save_to_memory(area + 0x1c, sick);
        plant_crowd(area, [0u32, 3, 2, 8][index], [5i32, 7, 1_000_000, -4][index]);
        fakes.push(area);
    }
    for &habitat in &habitats {
        let area = fake_viewing_area(true);
        save_to_memory(area, habitat);
        save_to_memory(area + 0x14, 2u32);
        plant_crowd(area, 4, 9);
        fakes.push(area);
    }
    let columns: u32 = get_from_memory(manager + 0x28);
    let mut tiles = vec![(0, 0), (world.map_x_size / 2, world.map_y_size / 2), (world.map_x_size - 1, world.map_y_size - 1)];
    tiles.extend(habitat_tiles().into_iter().step_by(40).take(10));
    let mut planted = Vec::new();
    let mut sites: Vec<(u32, u32, u32)> = Vec::new();
    for &(x, y) in &tiles {
        let row: u32 = get_from_memory(columns + x * 12);
        for direction in 0..8u32 {
            let slot = row + 4 + (direction + y * 10) * 4;
            planted.push((slot, get_from_memory::<u32>(slot)));
            save_to_memory(slot, fakes[(x + y + direction) as usize % fakes.len()]);
            sites.push((x, y, direction));
        }
    }
    let live_sites = sites.len();
    let exhibit = leaked_habitat(1);
    let guest = guest_with_habitat_slot(guest_type);
    HABITAT_ANSWER.store(exhibit, std::sync::atomic::Ordering::SeqCst);
    let ranges = [(-100, 100), (0x51, 100), (0x3d, 0x50), (0, 0x3c), (-0x3d, -1), (-0x50, -0x3d), (-100, -0x51)];
    let (mut mismatches, mut with_area, mut with_animals, mut with_sick, mut found) = (0, 0u32, 0u32, 0u32, 0u32);
    let mut crowd_ported = 0u32;
    let mut crowd_values = std::collections::BTreeSet::new();
    let this = guest.0 as *const std::ffi::c_void;
    {
        for &(x, y, direction) in &sites {
            place_guest(guest, (x, y));
            save_to_memory(guest.0 + VIEWING_AREA_DIRECTION, direction);
            for _pass in 0..2 {
                // The first pass may refresh a dirty area (real runs first); the second sees it clean.
                let queries: [(&str, u32, u32, u32); 4] = [
                    ("fNumAnimals", checks_real::num_animals(guest.0), guest.num_animals(), unsafe { F_NUM_ANIMALS.hooked()(this) }),
                    ("fSickAnimals", checks_real::sick_animals(guest.0) & 0xff, guest.sick_animals() & 0xff, unsafe { F_SICK_ANIMALS.hooked()(this) } & 0xff),
                    ("fNumSickAnimals", checks_real::num_sick_animals(guest.0), guest.num_sick_animals(), unsafe { F_NUM_SICK_ANIMALS.hooked()(this) }),
                    ("fDifferentSpecies", checks_real::different_species(guest.0), guest.different_species(), unsafe { F_DIFFERENT_SPECIES.hooked()(this) }),
                ];
                for (label, real_value, port_value, hooked_value) in queries {
                    if real_value != port_value || real_value != hooked_value {
                        mismatches += 1;
                        if mismatches <= 10 {
                            failures.push(format!("{label} ({x},{y}) direction {direction}: real {real_value}, port {port_value}, hooked {hooked_value}"));
                        }
                    }
                }
                let (real_crowd, port_crowd, hooked_crowd) = (checks_real::crowd_density(guest.0), guest.crowd_density(), unsafe { F_CROWD_DENSITY.hooked()(this) });
                // The fallback path leaves whatever vanilla leaves in the upper dword; callers read the low one.
                let port_low = port_crowd.map_or(real_crowd as i32, |value| value as i32);
                let full_match = port_crowd.is_none_or(|value| value == real_crowd);
                if real_crowd as i32 != port_low || real_crowd as i32 != hooked_crowd as i32 || !full_match {
                    mismatches += 1;
                    if mismatches <= 10 {
                        failures.push(format!("fCrowdDensity ({x},{y}) direction {direction}: real {real_crowd:#x}, port {port_crowd:x?}, hooked {hooked_crowd:#x}"));
                    }
                }
                crowd_ported += port_crowd.is_some() as u32;
                crowd_values.insert(real_crowd as i32);
                with_animals += (queries[0].1 > 0) as u32;
                with_sick += (queries[2].1 > 0) as u32;
            }
            with_area += (get_from_memory::<u32>(guest.0 + VIEWING_AREA) != 0 && get_from_memory::<u32>(guest.0 + VIEWING_AREA) != SENTINEL) as u32;
            for &(low, high) in &ranges {
                for seed in [1u32, 0xdead_beef] {
                    let mut poles = Vec::new();
                    for pole in 0..3 {
                        pin_rng(seed);
                        let animal = match pole {
                            0 => checks_real::find_animal_by_happiness(guest.0, low, high) as u32,
                            1 => guest.find_animal_by_happiness(low, high),
                            _ => unsafe { FIND_ANIMAL_BY_HAPPINESS.hooked()(guest.0 as *const u32, low, high) as u32 },
                        };
                        poles.push((animal, rng()));
                    }
                    found += (poles[0].0 != 0) as u32;
                    if poles[0] != poles[1] || poles[0] != poles[2] {
                        mismatches += 1;
                        if mismatches <= 10 {
                            failures.push(format!("findAnimalByHappiness ({x},{y}) direction {direction} {low}..{high} seed {seed:#x}: real {:x?}, port {:x?}, hooked {:x?}", poles[0], poles[1], poles[2]));
                        }
                    }
                }
            }
        }
    }
    for (slot, original) in planted.into_iter().rev() {
        save_to_memory(slot, original);
    }
    pin_rng(saved_rng);
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if crowd_ported == 0 || crowd_values.len() < 3 {
        failures.push(format!("fCrowdDensity coverage too thin: {crowd_ported} ported-path calls, values {crowd_values:?}"));
    }
    if with_area == 0 || with_animals == 0 || found == 0 {
        failures.push(format!("coverage too thin: {with_area} areas, {with_animals} with animals, {with_sick} with sick animals, {found} animals found over {live_sites} planted viewing-area slots"));
    }
    tracing::info!("{test_name}: {with_area} viewing areas, {with_animals} with animals, {with_sick} with sick animals, {found} finds over {live_sites} planted viewing-area slots");
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_DO_AMBIENT_ANIMS_LIVE`: twin guests with the `+0x33b` byte clear and set run the real body, the port and the
/// hooked address; the whole guest (including the `+0x339` flag the set byte raises) must match.
pub(crate) fn run_ztguest_do_ambient_anims_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztguest::DO_AMBIENT_ANIMS;

    use crate::ztguest::live_support::checks_real;

    let test_name = "ZTGUEST_DO_AMBIENT_ANIMS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    for trigger in [0u8, 1, 0xff] {
        let guests: Vec<ZTGuest> = (0..3).map(|_| live_support::build_standalone_guest(guest_type)).collect();
        for guest in &guests {
            save_to_memory(guest.0 + 0x33b, trigger);
        }
        checks_real::do_ambient_anims(guests[0].0);
        guests[1].do_ambient_anims();
        unsafe { DO_AMBIENT_ANIMS.hooked()(guests[2].0 as *const u32, std::ptr::null()) };
        let shared = [guest_type];
        compare_guests(&format!("trigger {trigger:#x} port"), guests[0], guests[1], &shared, &mut failures);
        compare_guests(&format!("trigger {trigger:#x} hooked"), guests[0], guests[2], &shared, &mut failures);
        let raised = get_from_memory::<u8>(guests[0].0 + 0x339) != 0;
        if raised != (trigger != 0) {
            failures.push(format!("trigger {trigger:#x}: real body left +0x339 {raised}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn leaked_habitat(exhibit_flag: u8) -> u32 {
    let habitat = Box::leak(vec![0u8; 0x100].into_boxed_slice()).as_ptr() as u32;
    save_to_memory(habitat + 0x2c, exhibit_flag);
    habitat
}

static PUKING_LOG: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

extern "thiscall" fn slot_stop_everything(_this: u32, a: u32, b: u32, c: u32) -> u32 {
    PUKING_LOG.lock().unwrap().push(format!("stopEverything {a} {b} {c}"));
    0
}

fn record_set_bs(_this: u32, name: &[u8], call_flag: i8, force: i8, clear_mode: i32) {
    PUKING_LOG.lock().unwrap().push(format!("setBSNew {} {call_flag} {force} {clear_mode}", String::from_utf8_lossy(name)));
}

fn guest_playing(guest_type: u32, name: &[u8]) -> ZTGuest {
    let guest = live_support::build_standalone_guest(guest_type);
    live_support::forget_standalone_guest(guest);
    let text = Box::leak(name.to_vec().into_boxed_slice());
    save_to_memory(guest.0 + ANIMATION_BEGIN, text.as_ptr() as u32);
    save_to_memory(guest.0 + ANIMATION_END, text.as_ptr() as u32 + name.len() as u32);
    guest
}

/// `ZTGUEST_START_PUKING_LIVE`: a guest already playing `"bSick"` is left alone by the real body, the port and the
/// hooked address alike. For every other animation name the real body would run a behaviour-set change that the bare
/// fixture cannot take, so the port runs with `setBSNew` and the stop slot recorded and must stop everything with
/// `(1, 0, 1)` and then switch to `"bSick"` with flags `(1, 1, 2)`.
pub(crate) fn run_ztguest_start_puking_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_START_PUKING_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let sick = [guest_playing(guest_type, b"bSick"), guest_playing(guest_type, b"bSick"), guest_playing(guest_type, b"bSick")];
    real::start_puking(sick[0].0);
    sick[1].start_puking();
    unsafe { START_PUKING.hooked()(sick[2].0 as *const u32) };
    compare_guests("startPuking bSick", sick[0], sick[1], &[], &mut failures);
    compare_guests("startPuking bSick hooked", sick[0], sick[2], &[], &mut failures);

    *crate::ztguest::live_support::viewing_overrides::SET_BS.lock().unwrap() = Some(record_set_bs);
    for name in [&b""[..], b"bSick2", b"bsick", b"bSic", b"bSick\0"] {
        for pole in 0..2 {
            let guest = guest_playing(guest_type, name);
            let real_vtable: u32 = get_from_memory(guest.0);
            let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
            vtable[0x198 / 4] = slot_stop_everything as *const () as usize as u32;
            save_to_memory(guest.0, Box::leak(vtable.into_boxed_slice()).as_ptr() as u32);
            PUKING_LOG.lock().unwrap().clear();
            if pole == 0 {
                guest.start_puking();
            } else {
                unsafe { START_PUKING.hooked()(guest.0 as *const u32) };
            }
            let log = std::mem::take(&mut *PUKING_LOG.lock().unwrap());
            let expected = vec!["stopEverything 1 0 1".to_string(), "setBSNew bSick 1 1 2".to_string()];
            if log != expected {
                failures.push(format!("startPuking over {:?} (pole {pole}): logged {log:?}, expected {expected:?}", String::from_utf8_lossy(name)));
            }
        }
    }
    *crate::ztguest::live_support::viewing_overrides::SET_BS.lock().unwrap() = None;
    finish_test(test_name, failures, failure_log)
}
