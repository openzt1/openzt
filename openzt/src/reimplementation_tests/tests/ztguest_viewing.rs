//! Live checks for `ztguest/viewing.rs` against the loaded zoo: `checkViewingArea`, `findViewingArea` and
//! `startPuking`. Guests come from the real vanilla constructor; the real body, the port and the hooked address run on
//! the same guest and tile and results, the guest's viewing-area fields and the shared RNG are compared. The scripted
//! cases (`followGuide`, `viewNearbyExhibit`, `doBuilding`, `setVisible`, `runScreaming`) are in `ztguest_checks.rs`.

use openzt_detour::generated::{
    ztguest::{CHECK_VIEWING_AREA, FIND_VIEWING_AREA, START_PUKING},
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
