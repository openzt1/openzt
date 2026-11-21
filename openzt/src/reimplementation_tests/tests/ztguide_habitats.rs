//! Live checks for `ztguide.rs`'s habitat queries: `getHabFromVA`, `getRandomAssignedHabitat` and `getNeediestVA`.
//! Twin fixtures built by the real vanilla constructor, one running the real body and one the port (and the hooked
//! address), compared on results, generator state and whole-guide memory. `getHabFromVA` draws from the CRT `rand()`;
//! a recording hook on that function scripts the draw so real and port see the same values.

use std::sync::{
    atomic::{AtomicU32, AtomicUsize, Ordering},
    Mutex, Once,
};

use openzt_detour::generated::{
    msvc_std::RAND,
    msvc_std_listuint::INSERT as LIST_UINT_INSERT,
    ztguide::{GET_HAB_FROM_VA, GET_NEEDIEST_VA, GET_RANDOM_ASSIGNED_HABITAT},
};

use super::ztguide::{base, compare_memory, pin_rng, rng};
use crate::{
    globals::globals,
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguide::{
        live_support::{self, real},
        ZTGuide, FIELD_0X298_OFFSET,
    },
    ztkeeper::unit_offsets,
};

/// `GLOBAL_ZTHabitatMgr`, as an RVA.
const HABITAT_MGR_RVA: u32 = 0x0023_805c;

static RAND_INTERCEPT: AtomicU32 = AtomicU32::new(0);
static RAND_VALUES: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static RAND_POSITION: AtomicUsize = AtomicUsize::new(0);

#[openzt_detour_macro::detour_mod]
mod rand_hook {
    use super::*;

    #[detour(RAND)]
    unsafe extern "stdcall" fn rand() -> u32 {
        if RAND_INTERCEPT.load(Ordering::SeqCst) != 0 {
            let values = RAND_VALUES.lock().unwrap();
            let position = RAND_POSITION.fetch_add(1, Ordering::SeqCst);
            values[position % values.len()]
        } else {
            unsafe { RAND_DETOUR.call() }
        }
    }
}

fn install_rand_hook() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { rand_hook::init_detours() } {
            result = Err(format!("could not install the rand hook: {e:?}"));
        }
    });
    result
}

fn script_rand(values: &[u32]) {
    *RAND_VALUES.lock().unwrap() = values.to_vec();
    RAND_POSITION.store(0, Ordering::SeqCst);
}

/// The live zoo's habitats, in `exhibit_array` order.
fn live_habitats() -> Vec<u32> {
    let exhibits = globals().zthabitatmgr().exhibit_array();
    (0..exhibits.len()).map(|index| exhibits.get_ptr(index)).collect()
}

/// A guide with `+0x298` set to `gate` and the habitats in `assigned` on its assignment list.
fn fixture(guide_type: u32, assigned: &[u32], gate: u32) -> ZTGuide {
    let guide = live_support::build_standalone_guide(guide_type);
    save_to_memory(guide.0 + FIELD_0X298_OFFSET, gate);
    let sentinel: u32 = get_from_memory(guide.0 + unit_offsets::ASSIGNED_HABITATS);
    for habitat in assigned {
        let mut result: u32 = 0;
        unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, habitat as *const u32 as *const i32) };
    }
    guide
}

/// Runs `body` with the live `GLOBAL_ZTHabitatMgr` pointer nulled.
fn without_habitat_mgr(body: &dyn Fn()) {
    let address = base() + HABITAT_MGR_RVA;
    let saved: u32 = get_from_memory(address);
    save_to_memory(address, 0u32);
    body();
    save_to_memory(address, saved);
}

/// A leaked fake viewing area for habitat `habitat`: its observable-area tile vector holds one entry when `has_oa`
/// (`hasOA` then answers true without recalculating) and none otherwise.
fn fake_viewing_area(habitat: u32, has_oa: bool) -> u32 {
    let area = Box::leak(vec![0u8; 0x100].into_boxed_slice());
    let address = area.as_ptr() as u32;
    save_to_memory(address, habitat);
    save_to_memory(address + 0x04, 0x1000u32);
    save_to_memory(address + 0x08, if has_oa { 0x1004u32 } else { 0x1000 });
    save_to_memory(address + 0x4c, 0u8);
    address
}

/// The habitat manager's eight viewing-area slots of the tile at `(x, y)`, as an address.
fn viewing_area_slots(x: u32, y: u32) -> u32 {
    globals().zthabitatmgr().get_habitat_cell_addr(x as i32, y as i32).expect("the tile is inside the manager's grid") + 4
}

/// `ZTGUIDE_GET_HAB_FROM_VA_LIVE`: `getHabFromVA` with the scripted CRT draw at every start slot (`0..8`, plus values
/// past 8 and a negative one) on a live tile whose eight slots are planted with fake viewing areas (habitat A with
/// observable area in slot 0, one without in slot 1, habitat B in slot 3, habitat A again in slot 6, the rest empty),
/// for a guide with no assignments, A assigned, B assigned, and only an unrelated habitat assigned; then every live
/// tile with one draw (all slots empty), a null tile and a null habitat manager. Real against port, and the hooked
/// address on a sample. The planted slots are zeroed afterwards.
pub(crate) fn run_ztguide_get_hab_from_va_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_GET_HAB_FROM_VA_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(message) = install_rand_hook() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let shared = vec![guide_type];
    let (habitat_a, habitat_b, unrelated) = (0xa0a0_a0a0u32, 0xb0b0_b0b0u32, 0xc0c0_c0c0u32);
    let (tile_x, tile_y) = (world.map_x_size / 2, world.map_y_size / 2);
    let tile = world.get_tile_ptr(tile_x, tile_y);
    let slots = viewing_area_slots(tile_x, tile_y);
    let planted = [
        (0u32, fake_viewing_area(habitat_a, true)),
        (1, fake_viewing_area(habitat_a, false)),
        (3, fake_viewing_area(habitat_b, true)),
        (6, fake_viewing_area(habitat_a, true)),
    ];
    for (slot, area) in planted {
        save_to_memory(slots + slot * 4, area);
    }

    let mut results = std::collections::BTreeSet::new();
    let mut mismatches = 0;
    RAND_INTERCEPT.store(1, Ordering::SeqCst);
    let configurations: [(&str, Vec<u32>); 4] =
        [("none assigned", vec![]), ("A assigned", vec![habitat_a]), ("B assigned", vec![habitat_b]), ("unrelated assigned", vec![unrelated])];
    for (label, assigned) in &configurations {
        let (real_guide, port_guide, hooked_guide) = (fixture(guide_type, assigned, 0), fixture(guide_type, assigned, 0), fixture(guide_type, assigned, 0));
        for draw in [0u32, 1, 2, 3, 4, 5, 6, 7, 8, 13, 0xffff_fffb] {
            script_rand(&[draw]);
            let real_result = real::get_hab_from_va(real_guide.0, tile) as u32;
            script_rand(&[draw]);
            let port_result = port_guide.get_hab_from_va(tile);
            script_rand(&[draw]);
            let hooked_result = unsafe { GET_HAB_FROM_VA.hooked()(hooked_guide.0 as *const u32, tile as *const u32) } as u32;
            if real_result != port_result || real_result != hooked_result {
                mismatches += 1;
                failures.push(format!("{label}, draw {draw:#x}: real {real_result:#x}, port {port_result:#x}, hooked {hooked_result:#x}"));
            }
            results.insert((*label, real_result));
        }
        compare_memory(label, real_guide, port_guide, &shared, &mut failures);
    }
    for expected in [("none assigned", habitat_a), ("none assigned", habitat_b), ("A assigned", habitat_a), ("B assigned", habitat_b), ("unrelated assigned", 0)] {
        if !results.contains(&expected) {
            failures.push(format!("the sweep never produced {expected:#x?}; got {results:#x?}"));
        }
    }
    for (slot, _) in planted {
        save_to_memory(slots + slot * 4, 0u32);
    }

    // Every live tile with its slots empty: all zero, nothing planted.
    let guide_real = fixture(guide_type, &[], 0);
    let guide_port = fixture(guide_type, &[], 0);
    script_rand(&[5]);
    for y in 0..world.map_y_size.min(200) {
        for x in 0..world.map_x_size.min(200) {
            let tile = world.get_tile_ptr(x, y);
            let real_result = real::get_hab_from_va(guide_real.0, tile) as u32;
            let port_result = guide_port.get_hab_from_va(tile);
            if real_result != port_result {
                mismatches += 1;
                if mismatches <= 10 {
                    failures.push(format!("tile ({x},{y}): real {real_result:#x}, port {port_result:#x}"));
                }
            }
        }
    }
    if real::get_hab_from_va(guide_real.0, 0) != 0 || guide_port.get_hab_from_va(0) != 0 {
        failures.push("a null tile did not give 0".to_string());
    }
    without_habitat_mgr(&|| {
        let tile = world.get_tile_ptr(world.map_x_size / 2, world.map_y_size / 2);
        if real::get_hab_from_va(guide_real.0, tile) != 0 || guide_port.get_hab_from_va(tile) != 0 {
            NULL_MANAGER_FAILED.store(1, Ordering::SeqCst);
        }
    });
    RAND_INTERCEPT.store(0, Ordering::SeqCst);
    if NULL_MANAGER_FAILED.swap(0, Ordering::SeqCst) == 1 {
        failures.push("with no habitat manager the result was not 0".to_string());
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    finish_test(test_name, failures, failure_log)
}

static NULL_MANAGER_FAILED: AtomicU32 = AtomicU32::new(0);

/// `ZTGUIDE_GET_RANDOM_ASSIGNED_HABITAT_LIVE`: `getRandomAssignedHabitat` with `+0x298` clear and set, no
/// assignments, one and two assigned habitats, and a null habitat manager, over several generator seeds; real body,
/// port and hooked address agree on the result and the generator state. Fails if only one result occurred.
pub(crate) fn run_ztguide_get_random_assigned_habitat_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_GET_RANDOM_ASSIGNED_HABITAT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let habitats = live_habitats();
    let seed = rng();
    let mut seen = std::collections::BTreeSet::new();
    let assignments: [(&str, Vec<u32>); 3] =
        [("none", vec![]), ("one", habitats.iter().take(1).copied().collect()), ("two", habitats.iter().take(2).copied().collect())];
    for (label, assigned) in &assignments {
        for gate in [0u32, 1] {
            for pole_seed in [seed, seed ^ 0x1234_5678, seed.wrapping_add(0x9e37_79b9), seed ^ 0x0bad_cafe] {
                let (real_guide, port_guide, hooked_guide) = (fixture(guide_type, assigned, gate), fixture(guide_type, assigned, gate), fixture(guide_type, assigned, gate));
                pin_rng(pole_seed);
                let real_result = real::get_random_assigned_habitat(real_guide.0);
                let real_rng = rng();
                pin_rng(pole_seed);
                let port_result = port_guide.get_random_assigned_habitat();
                let port_rng = rng();
                pin_rng(pole_seed);
                let hooked_result = unsafe { GET_RANDOM_ASSIGNED_HABITAT.hooked()(hooked_guide.0 as *const u32) };
                if real_result != port_result || real_result != hooked_result || real_rng != port_rng {
                    failures.push(format!(
                        "{label} assigned, +0x298 = {gate}: results real {real_result:#x}, port {port_result:#x}, hooked {hooked_result:#x}; RNG real {real_rng:#x}, port {port_rng:#x}"
                    ));
                }
                if gate != 0 && real_result != 0 {
                    failures.push(format!("{label} assigned: a set +0x298 still gave {real_result:#x}"));
                }
                seen.insert(real_result);
                compare_memory(label, real_guide, port_guide, &[guide_type], &mut failures);
            }
        }
    }
    without_habitat_mgr(&|| {
        let guide = fixture(guide_type, &[], 0);
        pin_rng(seed);
        let real_result = real::get_random_assigned_habitat(guide.0);
        let real_rng = rng();
        pin_rng(seed);
        let port_result = guide.get_random_assigned_habitat();
        if real_result != 0 || port_result != 0 || real_rng != rng() {
            NULL_MANAGER_FAILED.store(2, Ordering::SeqCst);
        }
    });
    if NULL_MANAGER_FAILED.swap(0, Ordering::SeqCst) == 2 {
        failures.push("with no habitat manager the result was not 0 or the generator moved".to_string());
    }
    pin_rng(seed);
    if seen.len() < 2 {
        failures.push(format!("only the results {seen:#x?} occurred"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUIDE_GET_NEEDIEST_VA_LIVE`: `getNeediestVA` over generator seeds (so both parities of the branch draw occur),
/// for each of no, one and two assigned habitats, `+0x298` clear and set, and a null habitat manager: the viewing area,
/// the habitat written through the out pointer (a sentinel when left untouched), and the generator state agree between
/// real body, port and hooked address. Fails if neither branch ever produced a viewing area.
pub(crate) fn run_ztguide_get_neediest_va_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_GET_NEEDIEST_VA_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let habitats = live_habitats();
    let seed = rng();
    let sentinel = 0xdead_beefu32;
    // Vanilla asks for viewing areas with guests; the test zoo has no viewing areas at all, so every habitat gets one fake
    // area (a guest present, an observable area, one tile near the middle of the map) for the duration of the test.
    let world = globals().ztworldmgr();
    let area_tile = world.get_tile_ptr(world.map_x_size / 2, world.map_y_size / 2);
    let planted: Vec<(u32, u32)> = habitats
        .iter()
        .map(|&habitat| {
            let area = fake_viewing_area(habitat, true);
            let tiles = Box::leak(vec![area_tile].into_boxed_slice());
            save_to_memory(area + 0x40, tiles.as_ptr() as u32);
            save_to_memory(area + 0x44, tiles.as_ptr() as u32 + 4);
            save_to_memory(area + 0x54, 1u32);
            crate::zthabitat::viewing_areas::push_viewing_area(habitat, area);
            (habitat, area)
        })
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let assignments: [(&str, Vec<u32>); 3] =
        [("none", vec![]), ("one", habitats.iter().take(1).copied().collect()), ("two", habitats.iter().take(2).copied().collect())];
    for (label, assigned) in &assignments {
        for gate in [0u32, 1] {
            for step in 0..12u32 {
                let pole_seed = seed.wrapping_add(step.wrapping_mul(0x9e37_79b9)) ^ step;
                let (real_guide, port_guide, hooked_guide) = (fixture(guide_type, assigned, gate), fixture(guide_type, assigned, gate), fixture(guide_type, assigned, gate));
                let (mut real_out, mut port_out, mut hooked_out) = (sentinel, sentinel, sentinel);
                pin_rng(pole_seed);
                let real_result = real::get_neediest_va(real_guide.0, &mut real_out as *mut u32 as u32) as u32;
                let real_rng = rng();
                pin_rng(pole_seed);
                let port_result = port_guide.get_neediest_va(&mut port_out as *mut u32 as u32);
                let port_rng = rng();
                pin_rng(pole_seed);
                let hooked_result = unsafe { GET_NEEDIEST_VA.hooked()(hooked_guide.0 as *const u32, &mut hooked_out as *mut u32 as u32) } as u32;
                if real_result != port_result || real_result != hooked_result || real_out != port_out || real_out != hooked_out || real_rng != port_rng {
                    failures.push(format!(
                        "{label} assigned, +0x298 = {gate}, seed {pole_seed:#x}: VA real {real_result:#x}, port {port_result:#x}, hooked {hooked_result:#x}; out real {real_out:#x}, port {port_out:#x}, hooked {hooked_out:#x}; RNG real {real_rng:#x}, port {port_rng:#x}"
                    ));
                }
                if gate != 0 && (real_result != 0 || real_out != sentinel) {
                    failures.push(format!("{label} assigned: a set +0x298 still gave VA {real_result:#x}, out {real_out:#x}"));
                }
                seen.insert(real_result != 0);
                compare_memory(label, real_guide, port_guide, &[guide_type], &mut failures);
            }
        }
    }
    without_habitat_mgr(&|| {
        let guide = fixture(guide_type, &[], 0);
        let (mut real_out, mut port_out) = (sentinel, sentinel);
        let real_result = real::get_neediest_va(guide.0, &mut real_out as *mut u32 as u32);
        let port_result = guide.get_neediest_va(&mut port_out as *mut u32 as u32);
        if real_result != 0 || port_result != 0 || real_out != sentinel || port_out != sentinel {
            NULL_MANAGER_FAILED.store(3, Ordering::SeqCst);
        }
    });
    if NULL_MANAGER_FAILED.swap(0, Ordering::SeqCst) == 3 {
        failures.push("with no habitat manager the result was not 0 or the out pointer was written".to_string());
    }
    for (habitat, area) in &planted {
        crate::zthabitat::viewing_areas::remove_viewing_area_entry(*habitat, *area);
    }
    pin_rng(seed);
    if !seen.contains(&true) {
        failures.push(format!("no sweep produced a viewing area ({} habitats, {} viewing areas), so neither branch was exercised", habitats.len(), planted.len()));
    }
    finish_test(test_name, failures, failure_log)
}
