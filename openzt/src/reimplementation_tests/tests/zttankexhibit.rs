//! Compares the reimplemented `ZTTankExhibit` methods (production file
//! `openzt/src/zthabitat/tank_exhibit.rs`) against real vanilla over the live, loaded zoo's tanks. The
//! `isRightSalinity`/`update`/`setTankHeight` tests predate this file and live in `zthabitatmgr.rs`.

use crate::globals::{get_module_base, globals};
use crate::reimplementation_tests::harness::finish_test;
use crate::util::{get_from_memory, ref_from_memory, save_to_memory};
use crate::zthabitatmgr::tank_exhibit::detours;
use crate::zthabitatmgr::{ZTHabitat, ZTTankExhibit};

const TANK_WATER_PURITY_OFFSET: u32 = 0x1a8;
const MURKY_WATER_PURITY_RVA: u32 = 0x0023_90a4;
const EXTREMELY_MURKY_WATER_PURITY_RVA: u32 = 0x0023_90b0;

/// Every live tank exhibit's address, in `exhibit_array` order.
fn live_tank_ptrs() -> Vec<u32> {
    let habitat_mgr = globals().zthabitatmgr();
    (0..habitat_mgr.exhibit_array().len())
        .map(|i| habitat_mgr.exhibit_array().get_ptr(i))
        .filter(|&ptr| ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(ptr) }.is_tank())
        .collect()
}

/// Purity values straddling both settings thresholds, so the signed `>=`/`<=` boundaries of
/// `isPureWater`/`isExtremelyMurkyWater` are each hit from below, on, and above, plus the signed extremes.
fn purity_probe_values() -> Vec<i32> {
    let base = get_module_base("zoo.exe") as u32;
    let murky: i32 = get_from_memory(base + MURKY_WATER_PURITY_RVA);
    let extremely_murky: i32 = get_from_memory(base + EXTREMELY_MURKY_WATER_PURITY_RVA);
    let mut values = vec![i32::MIN, -1, 0, 1, i32::MAX];
    for threshold in [murky, extremely_murky] {
        values.extend([threshold.wrapping_sub(1), threshold, threshold.wrapping_add(1)]);
    }
    values
}

fn compare_leaf_getters(tank_ptr: u32, label: &str, failures: &mut Vec<String>) {
    let this = tank_ptr as *const u32;
    let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
    let checks: [(&str, u32, u32); 5] = [
        ("getWaterLevel", detours::get_water_level_real(this), tank.get_water_level()),
        ("getTankHeight", detours::get_tank_height_real(this), tank.get_tank_height()),
        ("getWaterPurity", detours::get_water_purity_real(this), tank.get_water_purity() as u32),
        ("isPureWater", detours::is_pure_water_real(this) as u32, tank.is_pure_water() as u32),
        ("isExtremelyMurkyWater", detours::is_extremely_murky_water_real(this) as u32, tank.is_extremely_murky_water() as u32),
    ];
    for (name, real, reimpl) in checks {
        if real != reimpl {
            failures.push(format!("tank {tank_ptr:#010x} {label}: {name} real={real:#x}, reimpl={reimpl:#x}"));
        }
    }
}

/// `getWaterLevel`/`getTankHeight`/`getWaterPurity`/`isPureWater`/`isExtremelyMurkyWater` against real
/// vanilla for every live tank, first as-is and then with `water_purity` overwritten across both
/// thresholds (restored afterwards). The real pole goes through the release-safe `*_real` trampolines.
pub(crate) fn run_tankexhibit_leaf_getters_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_LEAF_GETTERS_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let probes = purity_probe_values();
    for &tank_ptr in &tanks {
        compare_leaf_getters(tank_ptr, "as loaded", &mut failures);
        let original_purity: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
        for &purity in &probes {
            save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, purity);
            compare_leaf_getters(tank_ptr, &format!("purity={purity}"), &mut failures);
        }
        save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, original_purity);
    }
    finish_test(test_name, failures, failure_log)
}

const TANK_HEIGHT_OFFSET: u32 = 0x184;

/// Compares the five position getters (written vector and the `EAX` out-pointer return) and
/// `getMinDepth` against real vanilla on one tank.
fn compare_positions_and_min_depth(tank_ptr: u32, label: &str, failures: &mut Vec<String>) {
    const UNWRITTEN: [i32; 3] = [0x5a5a_5a5a; 3];
    let this = tank_ptr as *const u32;
    let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
    type RealFn = fn(*const u32, *const i32) -> *const i32;
    let getters: [(&str, RealFn, Option<glam::i32::IVec3>); 5] = [
        ("getPlatformPos", detours::get_platform_pos_real, tank.get_platform_pos()),
        ("getLadderBaseOutsidePos", detours::get_ladder_base_outside_pos_real, tank.get_ladder_base_outside_pos()),
        ("getLadderTopOutsidePos", detours::get_ladder_top_outside_pos_real, tank.get_ladder_top_outside_pos()),
        ("getLadderBaseInsidePos", detours::get_ladder_base_inside_pos_real, tank.get_ladder_base_inside_pos()),
        ("getLadderTopInsidePos", detours::get_ladder_top_inside_pos_real, tank.get_ladder_top_inside_pos()),
    ];
    for (name, real_fn, reimpl) in getters {
        let Some(reimpl) = reimpl else {
            failures.push(format!("tank {tank_ptr:#010x} {label}: {name} reimpl returned None (missing gate tile)"));
            continue;
        };
        let mut real_out = UNWRITTEN;
        let returned = real_fn(this, real_out.as_mut_ptr());
        if returned != real_out.as_ptr() {
            failures.push(format!("tank {tank_ptr:#010x} {label}: {name} real did not return its out-pointer"));
        }
        if real_out != [reimpl.x, reimpl.y, reimpl.z] {
            failures.push(format!("tank {tank_ptr:#010x} {label}: {name} real={real_out:?}, reimpl={reimpl:?}"));
        }
    }
    let real_depth = detours::get_min_depth_real(this);
    let reimpl_depth = tank.get_min_depth();
    if real_depth != reimpl_depth {
        failures.push(format!("tank {tank_ptr:#010x} {label}: getMinDepth real={real_depth}, reimpl={reimpl_depth}"));
    }
}

/// The five position getters and `getMinDepth` against real vanilla on every live tank, as loaded and with
/// `tank_height` rewritten (restored afterwards) so the platform's `z` term and the ladder `z` clamp see
/// different values. Also checks each real getter returns its out-pointer, which vanilla callers read.
pub(crate) fn run_tankexhibit_positions_and_min_depth_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_POSITIONS_AND_MIN_DEPTH_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    for &tank_ptr in &tanks {
        compare_positions_and_min_depth(tank_ptr, "as loaded", &mut failures);
        let original_height: u32 = get_from_memory(tank_ptr + TANK_HEIGHT_OFFSET);
        for height in [0u32, 1, 3, 10, 40] {
            save_to_memory(tank_ptr + TANK_HEIGHT_OFFSET, height);
            compare_positions_and_min_depth(tank_ptr, &format!("tank_height={height}"), &mut failures);
        }
        save_to_memory(tank_ptr + TANK_HEIGHT_OFFSET, original_height);
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTHabitatMgr::doShowCheck` must reach each live tank's own `setIsShowExhibit`/`setIsNotShowExhibit`
/// override (vanilla dispatches through vtable `+0x2c`/`+0x30`), whose only visible effect is a
/// `ZTTankExhibit::setBaseTerrainType` call. `base_terrain_recorder` redirects that call into a record
/// (nothing is repainted), and the real and ported `doShowCheck` must record the same calls on every tank.
/// Fails if the port calls the base `ZTHabitat` ports directly, which skips the override.
pub(crate) fn run_zthabitatmgr_do_show_check_dispatches_tank_overrides_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::base_terrain_recorder;
    use crate::reimplementation_tests::tests::zthabitatmgr::sync_real_boundary_pairs;
    use crate::zthabitatmgr::hooks_zthabitatmgr;

    let test_name = "ZTHABITATMGR_DO_SHOW_CHECK_DISPATCHES_TANK_OVERRIDES_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let habitat_mgr = globals().zthabitatmgr();
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let mut total_calls = 0;
    for &tank_ptr in &tanks {
        sync_real_boundary_pairs(tank_ptr);
        base_terrain_recorder::begin_capture();
        hooks_zthabitatmgr::do_show_check_real(mgr_ptr, tank_ptr as *const i32, 0);
        let real = base_terrain_recorder::end_capture();
        base_terrain_recorder::begin_capture();
        habitat_mgr.do_show_check(tank_ptr, false);
        let port = base_terrain_recorder::end_capture();
        total_calls += real.len();
        if real.is_empty() {
            failures.push(format!("tank {tank_ptr:#010x}: real doShowCheck recorded no setBaseTerrainType call (test cannot detect a dispatch gap)"));
        }
        if real != port {
            failures.push(format!("tank {tank_ptr:#010x}: setBaseTerrainType calls real={real:x?}, port={port:x?}"));
        }
    }
    let label = format!("{test_name} (tanks: {}, recorded calls: {total_calls})", tanks.len());
    finish_test(&label, failures, failure_log)
}

const TANK_CURRENT_WATER_TYPE_OFFSET: u32 = 0x18c;
const TANK_IS_FILLED_OFFSET: u32 = 0x198;
const TANK_COST_FIELDS_OFFSET: u32 = 0x19c;
const INITIAL_HEIGHT_RVA: u32 = 0x0023_af20;
const INITIAL_HEIGHT_SHOW_RVA: u32 = 0x0023_af1c;

/// `getCostToChangeHeight` over height pairs that straddle both initial-height settings, with the tank
/// filled and drained, compared bit-for-bit as `f32`.
fn compare_cost_to_change_height(tank_ptr: u32, label: &str, failures: &mut Vec<String>) {
    let this = tank_ptr as *const u32;
    let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
    let base = get_module_base("zoo.exe") as u32;
    let mut heights = vec![-3, 0, 1, 2, 5, 12, 40];
    for rva in [INITIAL_HEIGHT_RVA, INITIAL_HEIGHT_SHOW_RVA] {
        let initial: i32 = get_from_memory(base + rva);
        heights.extend([initial - 1, initial, initial + 1]);
    }
    let original_filled: u8 = get_from_memory(tank_ptr + TANK_IS_FILLED_OFFSET);
    for filled in [0u8, 1] {
        save_to_memory(tank_ptr + TANK_IS_FILLED_OFFSET, filled);
        for &from in &heights {
            for &to in &heights {
                let real = detours::get_cost_to_change_height_real(this, from, to);
                let port = tank.get_cost_to_change_height(from, to);
                if real.to_bits() != port.to_bits() {
                    failures.push(format!("tank {tank_ptr:#010x} {label}: filled={filled} getCostToChangeHeight({from}, {to}) real={real}, port={port}"));
                }
            }
        }
    }
    save_to_memory(tank_ptr + TANK_IS_FILLED_OFFSET, original_filled);
}

fn read_cost_fields(tank_ptr: u32) -> [u32; 3] {
    get_from_memory(tank_ptr + TANK_COST_FIELDS_OFFSET)
}

/// `getCostToChangeHeight` and `updateAdjustmentCosts` against real vanilla on every live tank, with the
/// water type flipped across the fresh/salt boundary (restored afterwards). The cost fields are poisoned
/// before each recompute so a field the port forgets to write shows up, and restored at the end.
pub(crate) fn run_tankexhibit_costs_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_COSTS_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let mut saw_wall_cost = false;
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original_fields = read_cost_fields(tank_ptr);
        let original_water_type: i32 = get_from_memory(tank_ptr + TANK_CURRENT_WATER_TYPE_OFFSET);

        for water_type in [original_water_type, 0, 1, -1] {
            save_to_memory(tank_ptr + TANK_CURRENT_WATER_TYPE_OFFSET, water_type);
            save_to_memory(tank_ptr + TANK_COST_FIELDS_OFFSET, [0x7fc0_dead_u32; 3]);
            detours::update_adjustment_costs_real(this);
            let real = read_cost_fields(tank_ptr);
            saw_wall_cost |= f32::from_bits(real[0]) > 0.0;
            save_to_memory(tank_ptr + TANK_COST_FIELDS_OFFSET, [0x7fc0_dead_u32; 3]);
            tank.update_adjustment_costs();
            let port = read_cost_fields(tank_ptr);
            if real != port {
                failures.push(format!("tank {tank_ptr:#010x} water_type={water_type}: updateAdjustmentCosts fields real={real:x?}, port={port:x?}"));
            }
            compare_cost_to_change_height(tank_ptr, &format!("water_type={water_type}"), &mut failures);
        }

        save_to_memory(tank_ptr + TANK_CURRENT_WATER_TYPE_OFFSET, original_water_type);
        save_to_memory(tank_ptr + TANK_COST_FIELDS_OFFSET, original_fields);
    }
    if !tanks.is_empty() && !saw_wall_cost {
        failures.push("no live tank produced a nonzero wall cost, so the wall loop was not exercised".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `cleanWaterAmount` against real vanilla: both poles start from the same purity and must leave the same
/// purity behind (the real `setWaterPurity` does the clamping). The tank is settled back to its original
/// purity through the real setter afterwards.
pub(crate) fn run_tankexhibit_clean_water_amount_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::zttankexhibit::SET_WATER_PURITY;

    let test_name = "ZTTANKEXHIBIT_CLEAN_WATER_AMOUNT_MATCHES_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original_purity: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
        for amount in [0, 1, -2, 7, 1000] {
            save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, original_purity);
            detours::clean_water_amount_real(this, amount);
            let real: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
            save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, original_purity);
            tank.clean_water_amount(amount);
            let port: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
            if real != port {
                failures.push(format!("tank {tank_ptr:#010x} amount={amount} (purity {original_purity}): cleanWaterAmount real={real}, port={port}"));
            }
        }
        save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, original_purity);
        unsafe { SET_WATER_PURITY.original()(this, original_purity) };
    }
    finish_test(test_name, failures, failure_log)
}
