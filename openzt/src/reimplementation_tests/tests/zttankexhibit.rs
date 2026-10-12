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
        unsafe { SET_WATER_PURITY.hooked()(this, original_purity) };
    }
    finish_test(test_name, failures, failure_log)
}

const TANK_WATER_LEVEL_OFFSET: u32 = 0x188;
const TANK_PENDING_WATER_TYPE_OFFSET: u32 = 0x190;
const TANK_FILL_SUPPRESSED_OFFSET: u32 = 0x199;
const WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA: u32 = 0x0023_90ac;

/// The tank fields the water-state setters write, in one comparable record.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct WaterState {
    water_level: i32,
    water_purity: i32,
    is_filled: u8,
    current_water_type: i32,
    pending_water_type: i32,
}

fn read_water_state(tank_ptr: u32) -> WaterState {
    WaterState {
        water_level: get_from_memory(tank_ptr + TANK_WATER_LEVEL_OFFSET),
        water_purity: get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET),
        is_filled: get_from_memory(tank_ptr + TANK_IS_FILLED_OFFSET),
        current_water_type: get_from_memory(tank_ptr + TANK_CURRENT_WATER_TYPE_OFFSET),
        pending_water_type: get_from_memory(tank_ptr + TANK_PENDING_WATER_TYPE_OFFSET),
    }
}

/// `setWaterLevel` against real vanilla. Levels that cross zero on a tank holding water are skipped
/// (draining to zero removes the tank's owned transients, which a later restore cannot bring back); an
/// empty tank is tried at several levels, including negative and above the clamp. Each pole starts from
/// the original level and both are settled back through the real setter.
pub(crate) fn run_tankexhibit_set_water_level_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_SET_WATER_LEVEL_MATCHES_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let rise_delta: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original = read_water_state(tank_ptr);
        let height: i32 = get_from_memory(tank_ptr + TANK_HEIGHT_OFFSET);
        let max_level = height + rise_delta;
        let mut levels = vec![original.water_level, original.water_level + 1, max_level, max_level + 7, i32::MAX];
        if original.water_level > 1 {
            levels.push(original.water_level - 1);
        }
        if original.water_level == 0 {
            levels.extend([-5, 1, 2]);
        }
        for level in levels {
            detours::set_water_level_real(this, level);
            let real = read_water_state(tank_ptr);
            detours::set_water_level_real(this, original.water_level);
            tank.set_water_level(level);
            let port = read_water_state(tank_ptr);
            detours::set_water_level_real(this, original.water_level);
            if real != port {
                failures.push(format!("tank {tank_ptr:#010x} setWaterLevel({level}) from {}: real={real:?}, port={port:?}", original.water_level));
            }
        }
        if read_water_state(tank_ptr) != original {
            failures.push(format!("tank {tank_ptr:#010x}: not restored, now {:?}, was {original:?}", read_water_state(tank_ptr)));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `setWaterPurity` against real vanilla over values straddling both thresholds and the clamps, both poles
/// started from the original purity and settled back through the real setter. Crossing the
/// extremely-murky threshold shows a toast in each pole.
pub(crate) fn run_tankexhibit_set_water_purity_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_SET_WATER_PURITY_MATCHES_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let probes = purity_probe_values();
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
        for &value in &probes {
            detours::set_water_purity_real(this, value);
            let real: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
            detours::set_water_purity_real(this, original);
            tank.set_water_purity(value);
            let port: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
            detours::set_water_purity_real(this, original);
            if real != port {
                failures.push(format!("tank {tank_ptr:#010x} setWaterPurity({value}) from {original}: real={real}, port={port}"));
            }
        }
        let restored: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
        if restored != original {
            failures.push(format!("tank {tank_ptr:#010x}: purity not restored, now {restored}, was {original}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `checkEntitiesForFill`/`checkEntitiesForDrain` (pure scans of the tank's entity set) against real
/// vanilla on every live tank.
pub(crate) fn run_tankexhibit_check_entities_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_CHECK_ENTITIES_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let mut drain_blockers = 0;
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let (real_fill, port_fill) = (detours::check_entities_for_fill_real(this), tank.check_entities_for_fill());
        if real_fill != port_fill {
            failures.push(format!("tank {tank_ptr:#010x}: checkEntitiesForFill real={real_fill:#x}, port={port_fill:#x}"));
        }
        let (real_drain, port_drain) = (detours::check_entities_for_drain_real(this) as u32, tank.check_entities_for_drain());
        drain_blockers += (real_drain != 0) as u32;
        if real_drain != port_drain {
            failures.push(format!("tank {tank_ptr:#010x}: checkEntitiesForDrain real={real_drain:#x}, port={port_drain:#x}"));
        }
    }
    let label = format!("{test_name} (tanks: {}, tanks with a drain blocker: {drain_blockers})", tanks.len());
    finish_test(&label, failures, failure_log)
}

/// `fill`'s and `setSalinity`'s branches that leave the zoo unchanged, against real vanilla: `fill` with
/// `fill_suppressed` set, `fill` on a tank that is already filled to its target (it re-stages the pending
/// water type and refreshes the neighbours, nothing else), and `setSalinity` with the current water type.
/// The tank state is compared after each call and restored afterwards.
pub(crate) fn run_tankexhibit_fill_and_salinity_noop_branches_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_FILL_AND_SALINITY_NOOP_BRANCHES_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let mut filled_tanks = 0;
    let mut tank_summaries: Vec<String> = Vec::new();
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original = read_water_state(tank_ptr);
        tank_summaries.push(format!("filled={} level={} height={}", original.is_filled, original.water_level, get_from_memory::<i32>(tank_ptr + TANK_HEIGHT_OFFSET)));
        let original_suppressed: u8 = get_from_memory(tank_ptr + TANK_FILL_SUPPRESSED_OFFSET);

        save_to_memory(tank_ptr + TANK_FILL_SUPPRESSED_OFFSET, 1u8);
        detours::fill_real(this);
        let real = read_water_state(tank_ptr);
        tank.fill();
        let port = read_water_state(tank_ptr);
        save_to_memory(tank_ptr + TANK_FILL_SUPPRESSED_OFFSET, original_suppressed);
        if real != original || port != original {
            failures.push(format!("tank {tank_ptr:#010x}: suppressed fill changed state, real={real:?}, port={port:?}, was {original:?}"));
        }

        let height: i32 = get_from_memory(tank_ptr + TANK_HEIGHT_OFFSET);
        let rise_delta: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);
        if original.is_filled != 0 && original.water_level >= height + rise_delta {
            filled_tanks += 1;
            detours::fill_real(this);
            let real = read_water_state(tank_ptr);
            save_to_memory(tank_ptr + TANK_PENDING_WATER_TYPE_OFFSET, original.pending_water_type);
            tank.fill();
            let port = read_water_state(tank_ptr);
            save_to_memory(tank_ptr + TANK_PENDING_WATER_TYPE_OFFSET, original.pending_water_type);
            if real != port {
                failures.push(format!("tank {tank_ptr:#010x}: fill on a full tank real={real:?}, port={port:?}"));
            }
        }

        let real_ret = detours::set_salinity_real(this, original.current_water_type) & 0xff;
        let real = read_water_state(tank_ptr);
        let port_ret = tank.set_salinity(original.current_water_type) as u32;
        let port = read_water_state(tank_ptr);
        if real_ret != port_ret || real != port || port != original {
            failures.push(format!("tank {tank_ptr:#010x}: setSalinity(current) real=({real_ret}, {real:?}), port=({port_ret}, {port:?})"));
        }
    }
    let label = format!("{test_name} (tanks: {}, full tanks compared: {filled_tanks}, states: {tank_summaries:?})", tanks.len());
    finish_test(&label, failures, failure_log)
}

const TANK_TERRAIN_RVA: u32 = 0x0023_af08;
const APP_INIT_SUCCESS_LOAD_IN_PROGRESS_RVA: u32 = 0x0023_8148 + 0x441;
const TILE_SNAPSHOT_BYTES: u32 = 0x90;

fn snapshot_owned_tiles(tank_ptr: u32) -> Vec<(u32, Vec<u8>)> {
    let owned_tiles: u32 = get_from_memory(tank_ptr + 0x40);
    crate::zthabitatmgr::walk_tile_list(owned_tiles)
        .map(|node| {
            let tile: u32 = get_from_memory(node + 0x8);
            (tile, (0..TILE_SNAPSHOT_BYTES).map(|i| get_from_memory::<u8>(tile + i)).collect())
        })
        .collect()
}

fn restore_owned_tiles(snapshot: &[(u32, Vec<u8>)]) {
    for (tile, bytes) in snapshot {
        for (i, &byte) in bytes.iter().enumerate() {
            save_to_memory(tile + i as u32, byte);
        }
    }
}

/// `setBaseTerrainType` against real vanilla: both poles repaint a tank with the `tankTerrain` setting and
/// the raw bytes of every owned tile must match afterwards; the tiles are restored between poles and at
/// the end. The label reports whether the load-in-progress guard made both poles a no-op.
pub(crate) fn run_tankexhibit_set_base_terrain_type_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_SET_BASE_TERRAIN_TYPE_MATCHES_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let base = get_module_base("zoo.exe") as u32;
    let terrain: i32 = get_from_memory(base + TANK_TERRAIN_RVA);
    let load_in_progress: u8 = get_from_memory(base + APP_INIT_SUCCESS_LOAD_IN_PROGRESS_RVA);
    let mut painted_tiles = 0;
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original = snapshot_owned_tiles(tank_ptr);
        painted_tiles += original.len();
        for terrain_type in [terrain] {
            detours::set_base_terrain_type_real(this, terrain_type);
            let real = snapshot_owned_tiles(tank_ptr);
            restore_owned_tiles(&original);
            tank.set_base_terrain_type(terrain_type);
            let port = snapshot_owned_tiles(tank_ptr);
            restore_owned_tiles(&original);
            if real != port {
                failures.push(format!("tank {tank_ptr:#010x} setBaseTerrainType({terrain_type}): owned tile bytes differ between real and port"));
            }
        }
    }
    let label = format!("{test_name} (tanks: {}, tiles: {painted_tiles}, load_in_progress: {load_in_progress})", tanks.len());
    finish_test(&label, failures, failure_log)
}

const DRAIN_TEST_TANK_NAME: &str = "Drain Tank";

/// The dedicated destructive-test tank in the test save, found by exhibit name.
fn drain_test_tank_ptr() -> Option<u32> {
    use crate::util::ZTString;
    live_tank_ptrs().into_iter().find(|&ptr| unsafe { ref_from_memory::<ZTHabitat>(ptr) }.exhibit_name.copy_to_string() == DRAIN_TEST_TANK_NAME)
}

/// Everything the destructive tests compare after one pole has run.
#[derive(Debug, PartialEq)]
struct TankSnapshot {
    water: WaterState,
    cost_fields: [u32; 3],
    tiles: Vec<(u32, Vec<u8>)>,
    events: Vec<(u32, u16, u8, u32)>,
    messages: Vec<crate::reimplementation_tests::display_message_recorder::RecordedMessage>,
    habitat_flag_0x30: u8,
}

fn snapshot_tank(tank_ptr: u32, events: Vec<(u32, u16, u8, u32)>, messages: Vec<crate::reimplementation_tests::display_message_recorder::RecordedMessage>) -> TankSnapshot {
    TankSnapshot {
        water: read_water_state(tank_ptr),
        cost_fields: read_cost_fields(tank_ptr),
        tiles: snapshot_owned_tiles(tank_ptr),
        events,
        messages,
        habitat_flag_0x30: get_from_memory(tank_ptr + 0x30),
    }
}

/// Runs `pole` with `sendEvent` and `displayMessage` calls recorded (not delivered), then snapshots the tank.
fn run_pole_and_snapshot(tank_ptr: u32, pole: impl FnOnce()) -> TankSnapshot {
    use crate::reimplementation_tests::{display_message_recorder, send_event_recorder};
    send_event_recorder::begin_capture();
    display_message_recorder::begin_capture();
    pole();
    let messages = display_message_recorder::end_capture();
    let events = send_event_recorder::end_capture();
    snapshot_tank(tank_ptr, events, messages)
}

/// Puts the tank back into `original`'s water state through the real functions: salinity first (which
/// drains and refills), then the fill state and level.
fn settle_tank(tank_ptr: u32, original: &WaterState) {
    let this = tank_ptr as *const u32;
    if read_water_state(tank_ptr).current_water_type != original.current_water_type {
        detours::set_salinity_real(this, original.current_water_type);
    }
    if original.is_filled != 0 {
        detours::fill_real(this);
    } else {
        detours::drain_real(this);
    }
    detours::set_water_level_real(this, original.water_level);
    detours::set_water_purity_real(this, original.water_purity);
}

/// Runs `real` then `port` from the tank's state as found (settling back after each) and fails on any
/// difference in the resulting snapshots (water fields, cost fields, owned tile bytes, recorded events).
fn compare_poles(tank_ptr: u32, what: &str, real: impl FnOnce(), port: impl FnOnce(), failures: &mut Vec<String>) -> TankSnapshot {
    let original = read_water_state(tank_ptr);
    let real_snapshot = run_pole_and_snapshot(tank_ptr, real);
    settle_tank(tank_ptr, &original);
    let port_snapshot = run_pole_and_snapshot(tank_ptr, port);
    settle_tank(tank_ptr, &original);
    if real_snapshot != port_snapshot {
        failures.push(format!("{what}: real water={:?} costs={:x?} events={:x?} messages={:x?}; port water={:?} costs={:x?} events={:x?} messages={:x?}", real_snapshot.water, real_snapshot.cost_fields, real_snapshot.events, real_snapshot.messages, port_snapshot.water, port_snapshot.cost_fields, port_snapshot.events, port_snapshot.messages));
        if real_snapshot.tiles != port_snapshot.tiles {
            failures.push(format!("{what}: owned tile bytes differ"));
        }
    }
    if read_water_state(tank_ptr) != original {
        failures.push(format!("{what}: tank not restored, now {:?}, was {original:?}", read_water_state(tank_ptr)));
    }
    real_snapshot
}

/// `drain`, `setWaterLevel` to zero and back, `setSalinity` to a different type and `setBaseLevel`
/// against real vanilla on the dedicated, entity-free `Drain Tank` of the test save. Each pole starts from
/// the tank as found and the tank is settled back through the real functions afterwards. Compared:
/// water fields, cost fields, owned-tile bytes and the `sendEvent` calls made (recorded, not delivered).
pub(crate) fn run_tankexhibit_drain_tank_destructive_branches_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_DRAIN_TANK_DESTRUCTIVE_BRANCHES_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(tank_ptr) = drain_test_tank_ptr() else {
        failures.push(format!("no tank named {DRAIN_TEST_TANK_NAME:?} in the loaded save"));
        return finish_test(test_name, failures, failure_log);
    };
    let this = tank_ptr as *const u32;
    let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
    let start = read_water_state(tank_ptr);
    if start.is_filled == 0 || start.water_level <= 0 {
        failures.push(format!("Drain Tank must start filled with water, found {start:?}"));
    }
    if tank.check_entities_for_drain() != 0 || tank.check_entities_for_fill() != 0 {
        failures.push("Drain Tank must hold no entities".to_string());
    }

    let drained = compare_poles(tank_ptr, "drain", || detours::drain_real(this), || tank.drain(), &mut failures);
    // The water level only follows `drain` once the app is initialised (`ZTApp +0x440`), which it is not
    // under the battery; the cleared fill flag and the drain event are what is observable here.
    if drained.water.is_filled != 0 || drained.events.is_empty() {
        failures.push(format!("drain did not drain (cannot detect a gap): {:?}, events {:x?}", drained.water, drained.events));
    }
    for level in [0, 1, start.water_level + 1] {
        let result = compare_poles(tank_ptr, &format!("setWaterLevel({level})"), || detours::set_water_level_real(this, level), || tank.set_water_level(level), &mut failures);
        if result.water.water_level != level.clamp(0, start.water_level + 1) && level != start.water_level + 1 {
            failures.push(format!("setWaterLevel({level}) left level {} (cannot detect a gap)", result.water.water_level));
        }
    }
    let other_type = if start.current_water_type == 0 { 100 } else { 0 };
    let staged = compare_poles(
        tank_ptr,
        &format!("setSalinity({other_type})"),
        || {
            detours::set_salinity_real(this, other_type);
        },
        || {
            tank.set_salinity(other_type);
        },
        &mut failures,
    );
    // With water still in the tank the new type is only staged (applied once it has drained).
    if staged.water.pending_water_type != other_type || staged.water.is_filled != 0 {
        failures.push(format!("setSalinity({other_type}) did not stage the new type (cannot detect a gap): {:?}", staged.water));
    }
    // From an empty tank the new type is applied at once: costs recomputed, owned tiles flagged.
    detours::set_water_level_real(this, 0);
    let before_flip = read_cost_fields(tank_ptr);
    let applied = compare_poles(
        tank_ptr,
        &format!("setSalinity({other_type}) from an empty tank"),
        || {
            detours::set_salinity_real(this, other_type);
        },
        || {
            tank.set_salinity(other_type);
        },
        &mut failures,
    );
    if applied.water.current_water_type != other_type || applied.cost_fields == before_flip {
        failures.push(format!("setSalinity({other_type}) from an empty tank did not apply (cannot detect a gap): {:?}", applied.water));
    }
    detours::set_water_level_real(this, start.water_level);
    // Dropping the purity to zero crosses the extremely-murky threshold, which shows the murky-water toast.
    let murky = compare_poles(tank_ptr, "setWaterPurity(0)", || detours::set_water_purity_real(this, 0), || tank.set_water_purity(0), &mut failures);
    if murky.messages.len() != 1 || murky.water.water_purity != 0 {
        failures.push(format!("setWaterPurity(0) did not show exactly one toast (cannot detect a gap): {:x?}", murky.messages));
    }
    for level in [1, -1, 40] {
        let result = compare_poles(
            tank_ptr,
            &format!("setBaseLevel({level})"),
            || {
                save_to_memory(tank_ptr + 0x30, 0u8);
                detours::set_base_level_real(this, level);
            },
            || {
                save_to_memory(tank_ptr + 0x30, 0u8);
                tank.set_base_level(level);
            },
            &mut failures,
        );
        if result.habitat_flag_0x30 != 1 {
            failures.push(format!("setBaseLevel({level}) did not run to completion (flag +0x30 = {}; cannot detect a gap)", result.habitat_flag_0x30));
        }
        detours::set_base_level_real(this, -level.clamp(-12, 12));
    }
    finish_test(test_name, failures, failure_log)
}

/// `drain` and `fill` blocked by an entity inside the tank (the save's dolphin and shark tanks hold
/// animals), against real vanilla: nothing changes and the same blocked-toast is shown. `drain` is blocked
/// by any animal; `fill` only by an entity `canBeInTank` rejects, so it is compared on whichever live tanks
/// have one (the label reports how many).
pub(crate) fn run_tankexhibit_blocked_fill_and_drain_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::display_message_recorder;
    let test_name = "ZTTANKEXHIBIT_BLOCKED_FILL_AND_DRAIN_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    let (mut blocked_drains, mut blocked_fills) = (0, 0);
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original = read_water_state(tank_ptr);
        let has_gate = tank.gate_tile_in_ptr() != 0;

        if tank.check_entities_for_drain() != 0 {
            blocked_drains += 1;
            display_message_recorder::begin_capture();
            detours::drain_real(this);
            let real = (read_water_state(tank_ptr), display_message_recorder::end_capture());
            display_message_recorder::begin_capture();
            tank.drain();
            let port = (read_water_state(tank_ptr), display_message_recorder::end_capture());
            if real != port || real.0 != original || real.1.len() != has_gate as usize {
                failures.push(format!("tank {tank_ptr:#010x}: blocked drain real={real:x?}, port={port:x?}, was {original:?}, gate={has_gate}"));
            }
        }

        if tank.check_entities_for_fill() != 0 {
            blocked_fills += 1;
            save_to_memory(tank_ptr + TANK_IS_FILLED_OFFSET, 0u8);
            display_message_recorder::begin_capture();
            detours::fill_real(this);
            let real = (read_water_state(tank_ptr), display_message_recorder::end_capture());
            display_message_recorder::begin_capture();
            tank.fill();
            let port = (read_water_state(tank_ptr), display_message_recorder::end_capture());
            save_to_memory(tank_ptr + TANK_IS_FILLED_OFFSET, original.is_filled);
            if real != port || real.0.is_filled != 0 || real.1.len() != has_gate as usize {
                failures.push(format!("tank {tank_ptr:#010x}: blocked fill real={real:x?}, port={port:x?}, gate={has_gate}"));
            }
        }
    }
    if blocked_drains == 0 {
        failures.push("no live tank had a drain blocker (cannot detect a gap)".to_string());
    }
    let label = format!("{test_name} (blocked drains: {blocked_drains}, blocked fills: {blocked_fills})");
    finish_test(&label, failures, failure_log)
}

const TANK_RIPPLE_IDS_OFFSET: u32 = 0x1b4;
const TANK_RIPPLE_TYPE_OFFSET: u32 = 0x1c0;
const TANK_SPARKLES_OFFSET: u32 = 0x1c4;
const TANK_SPARKLE_IDS_OFFSET: u32 = 0x1d0;
const TANK_SALT_SPARKLE_TYPE_OFFSET: u32 = 0x1dc;
const TANK_FRESH_SPARKLE_TYPE_OFFSET: u32 = 0x1e0;
const SPARKLE_MAX_AGE_RVA: u32 = 0x0023_90b4;
const GAME_RNG_RVA: u32 = 0x0023_8060;
const RNG_SEED: u32 = 0x1357_9bdf;
const SPARKLES_PER_RUN: usize = 3;

/// A spawned entity's observable shape; ids and pointers differ between runs, these do not.
#[derive(Debug, PartialEq)]
struct EntityShape {
    pos: [i32; 3],
    rotation: u32,
}

fn entity_shape(entity: u32) -> Option<EntityShape> {
    (entity != 0).then(|| EntityShape {
        pos: [get_from_memory(entity + 0x114), get_from_memory(entity + 0x118), get_from_memory(entity + 0x11c)],
        rotation: get_from_memory(entity + 0x12c),
    })
}

/// A vanilla `{begin, end, cap_end}` vector's elements as words (`element_words` per element).
fn read_vector_words(header_addr: u32, element_words: u32) -> Vec<Vec<u32>> {
    let begin: u32 = get_from_memory(header_addr);
    let end: u32 = get_from_memory(header_addr + 4);
    if begin == 0 || end < begin {
        return Vec::new();
    }
    (begin..end)
        .step_by(element_words as usize * 4)
        .map(|element| (0..element_words).map(|word| get_from_memory::<u32>(element + word * 4)).collect())
        .collect()
}

fn read_id_vector(header_addr: u32) -> Vec<u32> {
    read_vector_words(header_addr, 1).into_iter().map(|element| element[0]).collect()
}

fn resolve_entity(id: u32) -> u32 {
    globals().ztworldmgr().resolve_entity_by_id(id) as u32
}

fn seed_game_rng() {
    save_to_memory(get_module_base("zoo.exe") as u32 + GAME_RNG_RVA, RNG_SEED);
}

fn game_rng() -> u32 {
    get_from_memory(get_module_base("zoo.exe") as u32 + GAME_RNG_RVA)
}

/// Retires and destroys an entity through the same vtable slots the removal functions use, to clean up
/// what a scenario spawned.
fn destroy_entity(entity: u32) {
    use crate::zthabitatmgr::support::{call_vtable_slot_with_u8, call_vtable_slot_with_u8_u8};
    unsafe {
        call_vtable_slot_with_u8_u8(entity, 0xa8, 1, 0);
        call_vtable_slot_with_u8(entity, 0xcc, 0);
    }
}

#[derive(Debug, PartialEq)]
struct RippleSnapshot {
    ripples: Vec<Option<EntityShape>>,
    ids_len: usize,
    type_cache: u32,
    rng: u32,
}

fn snapshot_ripples(tank_ptr: u32) -> (RippleSnapshot, Vec<u32>) {
    let ids = read_id_vector(tank_ptr + TANK_RIPPLE_IDS_OFFSET);
    let ripples = ids.iter().map(|&id| entity_shape(resolve_entity(id))).collect();
    let snapshot = RippleSnapshot { ripples, ids_len: ids.len(), type_cache: get_from_memory(tank_ptr + TANK_RIPPLE_TYPE_OFFSET), rng: game_rng() };
    (snapshot, ids)
}

/// `addWaterRipples`/`removeWaterRipples` (and through them `addWaterRipple`) against real vanilla on one
/// tank: the ripple count, each ripple's position and rotation, the cached ripple type and the RNG state
/// after, then that removal retires every ripple and empties the vector. The tank is left with ripples
/// again afterwards when it had any.
fn compare_ripples(tank_ptr: u32, failures: &mut Vec<String>, ripples_seen: &mut usize) {
    let this = tank_ptr as *const u32;
    let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
    let had_ripples = !read_id_vector(tank_ptr + TANK_RIPPLE_IDS_OFFSET).is_empty();
    let original_type: u32 = get_from_memory(tank_ptr + TANK_RIPPLE_TYPE_OFFSET);

    let mut poles: Vec<(RippleSnapshot, usize, usize)> = Vec::new();
    for port in [false, true] {
        save_to_memory(tank_ptr + TANK_RIPPLE_TYPE_OFFSET, 0u32);
        seed_game_rng();
        if port {
            tank.add_water_ripples();
        } else {
            detours::add_water_ripples_real(this);
        }
        let (snapshot, ids) = snapshot_ripples(tank_ptr);
        if port {
            tank.remove_water_ripples();
        } else {
            detours::remove_water_ripples_real(this);
        }
        let resolvable_after_remove = ids.iter().filter(|&&id| resolve_entity(id) != 0).count();
        let ids_left = read_id_vector(tank_ptr + TANK_RIPPLE_IDS_OFFSET).len();
        poles.push((snapshot, resolvable_after_remove, ids_left));
    }
    let (real, port) = (&poles[0], &poles[1]);
    if real != port {
        failures.push(format!("tank {tank_ptr:#010x} ripples: real={real:x?}, port={port:x?}"));
    }
    if real.2 != 0 {
        failures.push(format!("tank {tank_ptr:#010x} removeWaterRipples left {} ids behind", real.2));
    }
    *ripples_seen += real.0.ids_len;

    if had_ripples {
        detours::add_water_ripples_real(this);
    }
    save_to_memory(tank_ptr + TANK_RIPPLE_TYPE_OFFSET, original_type);
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SparkleScenario {
    Spawn,
    RemoveDead,
    RemoveAll,
    RemoveAllById,
}

#[derive(Debug, PartialEq)]
struct SparkleSnapshot {
    ages: Vec<u32>,
    sparkles: Vec<Option<EntityShape>>,
    ids_match_entities: bool,
    still_resolvable: Vec<bool>,
    spawned: usize,
    type_caches: [u32; 2],
    rng: u32,
}

/// Runs one sparkle scenario from a clean slate (sparkle vectors emptied, both type caches cleared, RNG
/// seeded): spawns [`SPARKLES_PER_RUN`] sparkles through the chosen pole, applies the scenario's removal
/// through the same pole, snapshots, and destroys whatever is left.
fn run_sparkle_scenario(tank_ptr: u32, port: bool, scenario: SparkleScenario) -> SparkleSnapshot {
    let this = tank_ptr as *const u32;
    let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
    detours::remove_all_sparkles_by_id_real(this);
    save_to_memory(tank_ptr + TANK_SALT_SPARKLE_TYPE_OFFSET, 0u32);
    save_to_memory(tank_ptr + TANK_FRESH_SPARKLE_TYPE_OFFSET, 0u32);
    seed_game_rng();
    for _ in 0..SPARKLES_PER_RUN {
        if port {
            tank.add_random_sparkle();
        } else {
            detours::add_random_sparkle_real(this as i32);
        }
    }
    let spawned_ids = read_id_vector(tank_ptr + TANK_SPARKLE_IDS_OFFSET);

    match scenario {
        SparkleScenario::Spawn => {}
        SparkleScenario::RemoveDead => {
            let max_age: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + SPARKLE_MAX_AGE_RVA);
            let begin: u32 = get_from_memory(tank_ptr + TANK_SPARKLES_OFFSET);
            for (index, age) in [0, max_age - 1, max_age].into_iter().enumerate().take(spawned_ids.len()) {
                save_to_memory(begin + index as u32 * 8, age);
            }
            if port {
                tank.remove_dead_sparkles(1);
            } else {
                detours::remove_dead_sparkles_real(this, 1);
            }
        }
        SparkleScenario::RemoveAll => {
            if port {
                tank.remove_all_sparkles();
            } else {
                detours::remove_all_sparkles_real(this);
            }
        }
        SparkleScenario::RemoveAllById => {
            if port {
                tank.remove_all_sparkles_by_id();
            } else {
                detours::remove_all_sparkles_by_id_real(this);
            }
        }
    }

    let pairs = read_vector_words(tank_ptr + TANK_SPARKLES_OFFSET, 2);
    let ids = read_id_vector(tank_ptr + TANK_SPARKLE_IDS_OFFSET);
    let pair_entity_ids: Vec<u32> = pairs.iter().map(|pair| if pair[1] == 0 { 0 } else { get_from_memory(pair[1] + 0x124) }).collect();
    let snapshot = SparkleSnapshot {
        ages: pairs.iter().map(|pair| pair[0]).collect(),
        sparkles: pairs.iter().map(|pair| entity_shape(pair[1])).collect(),
        ids_match_entities: ids == pair_entity_ids,
        still_resolvable: spawned_ids.iter().map(|&id| resolve_entity(id) != 0).collect(),
        spawned: spawned_ids.len(),
        type_caches: [get_from_memory(tank_ptr + TANK_SALT_SPARKLE_TYPE_OFFSET), get_from_memory(tank_ptr + TANK_FRESH_SPARKLE_TYPE_OFFSET)],
        rng: game_rng(),
    };

    detours::remove_all_sparkles_by_id_real(this);
    for id in spawned_ids {
        let entity = resolve_entity(id);
        if entity != 0 {
            destroy_entity(entity);
        }
    }
    snapshot
}

/// `addRandomSparkle`, `removeDeadSparkles`, `removeAllSparkles` and `removeAllSparklesById` against real
/// vanilla on every live tank, once per water type (the tank's own, then the other one, to exercise both
/// sparkle types and their caches), plus a murky-water run where nothing may spawn. Each scenario spawns
/// three sparkles through one pole from the same RNG seed and compares the resulting vectors, entity
/// positions and rotations, cached types and RNG state after the scenario's removal. The ripple half is
/// [`compare_ripples`].
pub(crate) fn run_tankexhibit_sparkles_and_ripples_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_SPARKLES_AND_RIPPLES_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let mut ripples_seen = 0usize;
    let mut sparkles_spawned = 0usize;
    let mut sparkles_killed = 0usize;
    let mut sparkles_removed_all = 0usize;
    for &tank_ptr in &tanks {
        compare_ripples(tank_ptr, &mut failures, &mut ripples_seen);

        let original_type: i32 = get_from_memory(tank_ptr + 0x18c);
        let original_purity: i32 = get_from_memory(tank_ptr + TANK_WATER_PURITY_OFFSET);
        let original_caches: [u32; 2] = [get_from_memory(tank_ptr + TANK_SALT_SPARKLE_TYPE_OFFSET), get_from_memory(tank_ptr + TANK_FRESH_SPARKLE_TYPE_OFFSET)];
        let water_types = [original_type, if original_type == 0 { 1 } else { 0 }];
        for (case, water_type) in water_types.into_iter().enumerate() {
            save_to_memory(tank_ptr + 0x18c, water_type);
            for scenario in [SparkleScenario::Spawn, SparkleScenario::RemoveDead, SparkleScenario::RemoveAll, SparkleScenario::RemoveAllById] {
                if case == 1 && scenario != SparkleScenario::Spawn {
                    continue;
                }
                let real = run_sparkle_scenario(tank_ptr, false, scenario);
                let port = run_sparkle_scenario(tank_ptr, true, scenario);
                if real != port {
                    failures.push(format!("tank {tank_ptr:#010x} {scenario:?} water_type={water_type}: real={real:x?}, port={port:x?}"));
                }
                match scenario {
                    SparkleScenario::Spawn => sparkles_spawned += real.spawned,
                    SparkleScenario::RemoveDead => sparkles_killed += real.spawned.saturating_sub(real.ages.len()),
                    SparkleScenario::RemoveAll | SparkleScenario::RemoveAllById => {
                        if real.ages.is_empty() && real.spawned > 0 {
                            sparkles_removed_all += real.spawned;
                        }
                    }
                }
            }
        }
        save_to_memory(tank_ptr + 0x18c, original_type);

        // Murky water: nothing may spawn, in either pole.
        save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, i32::MIN);
        let real = run_sparkle_scenario(tank_ptr, false, SparkleScenario::Spawn);
        let port = run_sparkle_scenario(tank_ptr, true, SparkleScenario::Spawn);
        save_to_memory(tank_ptr + TANK_WATER_PURITY_OFFSET, original_purity);
        if real != port || real.spawned != 0 {
            failures.push(format!("tank {tank_ptr:#010x} murky spawn: real={real:x?}, port={port:x?}"));
        }
        save_to_memory(tank_ptr + TANK_SALT_SPARKLE_TYPE_OFFSET, original_caches[0]);
        save_to_memory(tank_ptr + TANK_FRESH_SPARKLE_TYPE_OFFSET, original_caches[1]);
    }
    if ripples_seen == 0 {
        failures.push("no live tank spawned a ripple (cannot detect a gap)".to_string());
    }
    if sparkles_spawned == 0 {
        failures.push("no live tank spawned a sparkle (cannot detect a gap)".to_string());
    }
    if sparkles_killed == 0 {
        failures.push("removeDeadSparkles never killed a sparkle (cannot detect a gap)".to_string());
    }
    if sparkles_removed_all == 0 {
        failures.push("removeAllSparkles/ById never emptied a spawned set (cannot detect a gap)".to_string());
    }
    let label = format!("{test_name} (ripples: {ripples_seen}, sparkles: {sparkles_spawned}, killed: {sparkles_killed})");
    finish_test(&label, failures, failure_log)
}

/// A standalone vanilla `std::set<BFEntity*>` (`{head*, size}` container over a `PoolAlloc` header),
/// the shape `getAllEntities`/`getIllegalEntities` fill. Both poles insert through `PoolAlloc`, so
/// freeing through it on drop is correct for either.
struct VanillaEntitySet {
    container: Box<[u32; 3]>,
}

const ENTITY_SET_NODE_SIZE: u32 = 0x14;

impl VanillaEntitySet {
    fn new() -> Self {
        use openzt_detour::generated::poolalloc::ALLOCATE;
        let head = unsafe { ALLOCATE.original()(ENTITY_SET_NODE_SIZE) } as u32;
        save_to_memory(head, 0u32);
        save_to_memory(head + 0x4, 0u32);
        save_to_memory(head + 0x8, head);
        save_to_memory(head + 0xc, head);
        Self { container: Box::new([head, 0, 0]) }
    }

    fn addr(&self) -> u32 {
        self.container.as_ptr() as u32
    }

    fn members(&self) -> Vec<u32> {
        crate::zthabitatmgr::support::walk_neighbor_tree(self.container[0]).map(|node| get_from_memory::<u32>(node + 0x10)).collect()
    }

    fn size(&self) -> u32 {
        get_from_memory(self.addr() + 4)
    }
}

impl Drop for VanillaEntitySet {
    fn drop(&mut self) {
        use openzt_detour::generated::poolalloc::DEALLOCATE;
        crate::zthabitatmgr::support::rb_tree_clear(self.addr(), ENTITY_SET_NODE_SIZE);
        unsafe { DEALLOCATE.original()(self.container[0] as *const u32, ENTITY_SET_NODE_SIZE) };
    }
}

/// `getAllEntities`, `getIllegalEntities` (every mode), `isIllegalEntity` and `canBeInTank` against real
/// vanilla on every live tank: the filled sets' members and sizes, and the two predicates on every entity
/// the tank holds (plus a null entity) for modes `-1` through `3`.
pub(crate) fn run_tankexhibit_entity_scans_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_ENTITY_SCANS_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let (mut entities_seen, mut illegal_seen) = (0usize, 0usize);
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };

        let real = VanillaEntitySet::new();
        detours::get_all_entities_real(this, real.addr() as *const std::ffi::c_void);
        let port = VanillaEntitySet::new();
        tank.get_all_entities_into(port.addr());
        if real.members() != port.members() || real.size() != port.size() {
            failures.push(format!("tank {tank_ptr:#010x}: getAllEntities real={:x?} ({}), port={:x?} ({})", real.members(), real.size(), port.members(), port.size()));
        }
        if tank.all_entities().into_iter().collect::<Vec<_>>() != real.members() {
            failures.push(format!("tank {tank_ptr:#010x}: all_entities() differs from real getAllEntities"));
        }
        entities_seen += real.members().len();

        for mode in -1..=3 {
            let real_set = VanillaEntitySet::new();
            detours::get_illegal_entities_real(this, real_set.addr() as *const std::ffi::c_void, mode);
            let port_set = VanillaEntitySet::new();
            tank.get_illegal_entities_into(port_set.addr(), mode);
            if real_set.members() != port_set.members() || real_set.size() != port_set.size() {
                failures.push(format!(
                    "tank {tank_ptr:#010x}: getIllegalEntities(mode {mode}) real={:x?} ({}), port={:x?} ({})",
                    real_set.members(),
                    real_set.size(),
                    port_set.members(),
                    port_set.size()
                ));
            }
            illegal_seen += real_set.members().len();
        }

        for entity in std::iter::once(0).chain(real.members()) {
            let (real_can, port_can) = (detours::can_be_in_tank_real(entity) != 0, ZTTankExhibit::can_be_in_tank(entity));
            if real_can != port_can {
                failures.push(format!("tank {tank_ptr:#010x} entity {entity:#010x}: canBeInTank real={real_can}, port={port_can}"));
            }
            for mode in -1..=3 {
                let (real_illegal, port_illegal) = (detours::is_illegal_entity_real(entity as *const u32, mode) & 0xff != 0, ZTTankExhibit::is_illegal_entity(entity, mode));
                if real_illegal != port_illegal {
                    failures.push(format!("tank {tank_ptr:#010x} entity {entity:#010x}: isIllegalEntity(mode {mode}) real={real_illegal}, port={port_illegal}"));
                }
            }
        }
    }
    if entities_seen == 0 {
        failures.push("no live tank held any entity (cannot detect a gap)".to_string());
    }
    if illegal_seen == 0 {
        failures.push("no live tank held an illegal entity in any mode (cannot detect a gap)".to_string());
    }
    let label = format!("{test_name} (tanks: {}, entities: {entities_seen}, illegal hits: {illegal_seen})", tanks.len());
    finish_test(&label, failures, failure_log)
}

const TANK_WALL_IDS_OFFSET: u32 = 0x178;
const TANK_FILTER_ID_OFFSET: u32 = 0x1b0;
const WALL_HEIGHT_OFFSET: u32 = 0x454;
const WALL_HEIGHT_FLAG_OFFSET: u32 = 0x464;
const WALL_HEIGHT_SENTINEL: i32 = 99;

fn wall_states_for(ids: &[u32]) -> Vec<(i32, u8)> {
    ids.iter()
        .map(|&id| resolve_entity(id))
        .filter(|&wall| wall != 0)
        .map(|wall| (get_from_memory::<i32>(wall + WALL_HEIGHT_OFFSET), get_from_memory::<u8>(wall + WALL_HEIGHT_FLAG_OFFSET)))
        .collect()
}

fn wall_states(tank_ptr: u32) -> Vec<(i32, u8)> {
    wall_states_for(&read_id_vector(tank_ptr + TANK_WALL_IDS_OFFSET))
}

fn set_all_wall_heights(tank_ptr: u32, height: i32) {
    use openzt_detour::generated::zttankwall::SET_WALL_HEIGHT;
    for id in read_id_vector(tank_ptr + TANK_WALL_IDS_OFFSET) {
        let wall = resolve_entity(id);
        if wall != 0 {
            unsafe { SET_WALL_HEIGHT.original()(wall as *const u32, height) };
        }
    }
}

/// `addTankWall`, `updateTankWallInfo`, `clearWallVector`, `validateFilterPosition` and
/// `removeOwnedTransients`/`restoreOwnedTransients` against real vanilla on every live tank. Walls are set
/// to a sentinel height before each pole so a pole that does nothing cannot match; the real functions
/// put the tank's walls, wall vector, filter id and ripples back afterwards.
pub(crate) fn run_tankexhibit_walls_filter_and_transients_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_WALLS_FILTER_AND_TRANSIENTS_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let (mut walls_seen, mut ripples_seen) = (0usize, 0usize);
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let original_ids = read_id_vector(tank_ptr + TANK_WALL_IDS_OFFSET);
        walls_seen += original_ids.len();

        // addTankWall: append an existing wall's id through each pole, dropping it again after.
        if let Some(&first_id) = original_ids.first() {
            let wall = resolve_entity(first_id);
            let mut after = Vec::new();
            for port in [false, true] {
                if port {
                    tank.add_tank_wall(wall);
                } else {
                    detours::add_tank_wall_real(this, wall as *const u32);
                }
                after.push(read_id_vector(tank_ptr + TANK_WALL_IDS_OFFSET));
                let begin: u32 = get_from_memory(tank_ptr + TANK_WALL_IDS_OFFSET);
                save_to_memory(tank_ptr + TANK_WALL_IDS_OFFSET + 4, begin + original_ids.len() as u32 * 4);
            }
            let mut expected = original_ids.clone();
            expected.push(first_id);
            if after[0] != expected || after[1] != expected {
                failures.push(format!("tank {tank_ptr:#010x}: addTankWall real={:x?}, port={:x?}, expected={expected:x?}", after[0], after[1]));
            }
        }

        // updateTankWallInfo at several heights, then clearWallVector.
        let original_height: i32 = get_from_memory(tank_ptr + TANK_HEIGHT_OFFSET);
        for height in [0, original_height + 2, original_height] {
            save_to_memory(tank_ptr + TANK_HEIGHT_OFFSET, height);
            let mut results = Vec::new();
            for port in [false, true] {
                set_all_wall_heights(tank_ptr, WALL_HEIGHT_SENTINEL);
                save_to_memory(tank_ptr + 0x19c, 0u32);
                save_to_memory(tank_ptr + 0x1a0, 0u32);
                save_to_memory(tank_ptr + 0x1a4, 0u32);
                if port {
                    tank.update_tank_wall_info();
                } else {
                    detours::update_tank_wall_info_real(this);
                }
                results.push((wall_states(tank_ptr), read_cost_fields(tank_ptr)));
            }
            if results[0] != results[1] {
                failures.push(format!("tank {tank_ptr:#010x}: updateTankWallInfo(height {height}) real={:x?}, port={:x?}", results[0], results[1]));
            }
            if results[0].0.iter().any(|&(wall_height, _)| wall_height != height) {
                failures.push(format!("tank {tank_ptr:#010x}: real updateTankWallInfo(height {height}) left walls at {:x?}", results[0].0));
            }
        }
        save_to_memory(tank_ptr + TANK_HEIGHT_OFFSET, original_height);
        detours::update_tank_wall_info_real(this);

        let wall_end: u32 = get_from_memory(tank_ptr + TANK_WALL_IDS_OFFSET + 4);
        let mut cleared = Vec::new();
        for port in [false, true] {
            set_all_wall_heights(tank_ptr, WALL_HEIGHT_SENTINEL);
            if port {
                tank.clear_wall_vector();
            } else {
                detours::clear_wall_vector_real(tank_ptr as i32);
            }
            let end: u32 = get_from_memory(tank_ptr + TANK_WALL_IDS_OFFSET + 4);
            let begin: u32 = get_from_memory(tank_ptr + TANK_WALL_IDS_OFFSET);
            cleared.push((end == begin, wall_states_for(&original_ids)));
            save_to_memory(tank_ptr + TANK_WALL_IDS_OFFSET + 4, wall_end);
        }
        if cleared[0] != cleared[1] || !cleared[0].0 {
            failures.push(format!("tank {tank_ptr:#010x}: clearWallVector real={:?}, port={:?}", cleared[0], cleared[1]));
        }
        detours::update_tank_wall_info_real(this);

        // validateFilterPosition: no filter, and an id that names no entity.
        let original_filter: i32 = get_from_memory(tank_ptr + TANK_FILTER_ID_OFFSET);
        for probe in [-1, 0x7fff_0000, original_filter] {
            let mut results = Vec::new();
            for port in [false, true] {
                save_to_memory(tank_ptr + TANK_FILTER_ID_OFFSET, probe);
                if port {
                    tank.validate_filter_position();
                } else {
                    detours::validate_filter_position_real(this);
                }
                results.push(get_from_memory::<i32>(tank_ptr + TANK_FILTER_ID_OFFSET));
            }
            if results[0] != results[1] {
                failures.push(format!("tank {tank_ptr:#010x}: validateFilterPosition(filter {probe:#x}) real={:#x}, port={:#x}", results[0], results[1]));
            }
        }
        save_to_memory(tank_ptr + TANK_FILTER_ID_OFFSET, original_filter);

        // restoreOwnedTransients then removeOwnedTransients, from the same RNG seed.
        let had_ripples = !read_id_vector(tank_ptr + TANK_RIPPLE_IDS_OFFSET).is_empty();
        let original_type: u32 = get_from_memory(tank_ptr + TANK_RIPPLE_TYPE_OFFSET);
        detours::remove_water_ripples_real(this);
        let mut poles = Vec::new();
        for port in [false, true] {
            save_to_memory(tank_ptr + TANK_RIPPLE_TYPE_OFFSET, 0u32);
            seed_game_rng();
            if port {
                tank.restore_owned_transients();
            } else {
                detours::restore_owned_transients_real(this);
            }
            let (snapshot, _) = snapshot_ripples(tank_ptr);
            if port {
                tank.remove_owned_transients();
            } else {
                detours::remove_owned_transients_real(this);
            }
            let left = (read_id_vector(tank_ptr + TANK_RIPPLE_IDS_OFFSET).len(), read_vector_words(tank_ptr + TANK_SPARKLES_OFFSET, 2).len());
            poles.push((snapshot, left));
        }
        if poles[0] != poles[1] {
            failures.push(format!("tank {tank_ptr:#010x}: restore/removeOwnedTransients real={:x?}, port={:x?}", poles[0], poles[1]));
        }
        if poles[0].1 != (0, 0) {
            failures.push(format!("tank {tank_ptr:#010x}: removeOwnedTransients left ripples/sparkles behind: {:?}", poles[0].1));
        }
        ripples_seen += poles[0].0.ids_len;
        if had_ripples {
            detours::add_water_ripples_real(this);
        }
        save_to_memory(tank_ptr + TANK_RIPPLE_TYPE_OFFSET, original_type);
    }
    if walls_seen == 0 {
        failures.push("no live tank had wall ids (cannot detect a gap)".to_string());
    }
    if ripples_seen == 0 {
        failures.push("restoreOwnedTransients never spawned a ripple (cannot detect a gap)".to_string());
    }
    let label = format!("{test_name} (tanks: {}, walls: {walls_seen}, ripples: {ripples_seen})", tanks.len());
    finish_test(&label, failures, failure_log)
}

/// The habitat manager ownership cell of every owned tile, as `(cell address, value)`.
fn ownership_cells(tank_ptr: u32) -> Vec<(u32, u32)> {
    let habitat_mgr = globals().zthabitatmgr();
    snapshot_owned_tiles(tank_ptr)
        .into_iter()
        .filter_map(|(tile, _)| {
            let (x, y): (i32, i32) = (get_from_memory(tile + 0x34), get_from_memory(tile + 0x38));
            habitat_mgr.get_habitat_cell_addr(x, y).map(|cell| (cell, get_from_memory::<u32>(cell)))
        })
        .collect()
}

fn restore_ownership_cells(cells: &[(u32, u32)]) {
    for &(cell, value) in cells {
        save_to_memory(cell, value);
    }
}

/// `reclaimTiles` and `doDepthSuitability` against real vanilla on every live tank (raw tank bytes, owned
/// tile bytes and ownership cells compared; each pole starts from the same perturbed state, and the original
/// state is restored afterwards). `reclaimTiles` starts with the first tile's ownership cell and every owned
/// tile's owner back-pointer and flags cleared, so both branches run. `doDepthSuitability` runs for every
/// tank that is filled and has a gate-in tile.
pub(crate) fn run_tankexhibit_reclaim_tiles_and_depth_suitability_match_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_RECLAIM_TILES_AND_DEPTH_SUITABILITY_MATCH_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let (mut reclaimed_tiles, mut depth_runs) = (0usize, 0usize);
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };

        let original_tiles = snapshot_owned_tiles(tank_ptr);
        let original_cells = ownership_cells(tank_ptr);
        let mut poles = Vec::new();
        for port in [false, true] {
            for (index, (tile, _)) in original_tiles.iter().enumerate() {
                save_to_memory(tile + 0x88, 0u32);
                save_to_memory(tile + 0x85, get_from_memory::<u8>(tile + 0x85) & !0x21);
                if index == 0 {
                    if let Some(&(cell, _)) = original_cells.first() {
                        save_to_memory(cell, 0xdead_0000u32);
                    }
                }
            }
            if port {
                tank.reclaim_tiles();
            } else {
                detours::reclaim_tiles_real(this);
            }
            poles.push((snapshot_owned_tiles(tank_ptr), ownership_cells(tank_ptr)));
        }
        restore_owned_tiles(&original_tiles);
        restore_ownership_cells(&original_cells);
        reclaimed_tiles += original_tiles.len();
        if poles[0] != poles[1] {
            failures.push(format!("tank {tank_ptr:#010x}: reclaimTiles real and port left different tiles/cells"));
        } else if let Some((tile, bytes)) = poles[0].0.first() {
            if bytes[0x85] & 0x21 != 0x21 || bytes[0x88..0x8c] == [0, 0, 0, 0] {
                failures.push(format!("tank {tank_ptr:#010x}: real reclaimTiles did not reclaim its first tile {tile:#010x}"));
            }
        }

        if *tank.is_filled() && tank.habitat.gate_tile_in_ptr() != 0 {
            let original_height: i32 = get_from_memory(tank_ptr + TANK_HEIGHT_OFFSET);
            for new_height in [0, original_height, original_height + 3] {
                let dirty_addr = tank_ptr + std::mem::offset_of!(ZTHabitat, characteristics_dirty) as u32;
                let mut results = Vec::new();
                for port in [false, true] {
                    save_to_memory(dirty_addr, 0u8);
                    if port {
                        tank.do_depth_suitability(new_height);
                    } else {
                        detours::do_depth_suitability_real(this, new_height);
                    }
                    results.push((get_from_memory::<u32>(tank_ptr + 0x188), get_from_memory::<u32>(tank_ptr + TANK_HEIGHT_OFFSET), get_from_memory::<u8>(dirty_addr)));
                }
                depth_runs += 1;
                if results[0] != results[1] || results[0].2 != 1 {
                    failures.push(format!("tank {tank_ptr:#010x}: doDepthSuitability({new_height}) real={:?}, port={:?} (water level, tank height, dirty flag)", results[0], results[1]));
                }
            }
        }
    }
    let label = format!("{test_name} (tanks: {}, tiles: {reclaimed_tiles}, depth runs: {depth_runs})", tanks.len());
    finish_test(&label, failures, failure_log)
}

/// `levelAllTiles` against real vanilla on the entity-free `Drain Tank`, after raising its ground by 4
/// through the real `setBaseLevel`: the tiles around it (raw bytes) must match after each pole, must
/// differ from the raised terrain (so a no-op cannot pass), and are restored between poles and at the end.
pub(crate) fn run_tankexhibit_level_all_tiles_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_LEVEL_ALL_TILES_MATCHES_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(tank_ptr) = drain_test_tank_ptr() else {
        failures.push(format!("no tank named {DRAIN_TEST_TANK_NAME:?} in the loaded save"));
        return finish_test(test_name, failures, failure_log);
    };
    let this = tank_ptr as *const u32;
    let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };

    let world = globals().ztworldmgr();
    let positions: Vec<(i32, i32)> = snapshot_owned_tiles(tank_ptr).iter().map(|(tile, _)| (get_from_memory(tile + 0x34), get_from_memory(tile + 0x38))).collect();
    let (min_x, max_x) = (positions.iter().map(|p| p.0).min().unwrap_or(0) - 2, positions.iter().map(|p| p.0).max().unwrap_or(0) + 2);
    let (min_y, max_y) = (positions.iter().map(|p| p.1).min().unwrap_or(0) - 2, positions.iter().map(|p| p.1).max().unwrap_or(0) + 2);
    let region: Vec<u32> = (min_x.max(0)..=max_x)
        .flat_map(|x| (min_y.max(0)..=max_y).map(move |y| (x, y)))
        .filter(|&(x, y)| (x as u32) < world.map_x_size && (y as u32) < world.map_y_size)
        .map(|(x, y)| world.get_tile_ptr(x as u32, y as u32))
        .collect();
    let snapshot_region = || -> Vec<(u32, Vec<u8>)> { region.iter().map(|&tile| (tile, (0..0x8c).map(|i| get_from_memory::<u8>(tile + i)).collect())).collect() };

    let untouched = snapshot_region();
    detours::set_base_level_real(this, 4);
    let original = snapshot_region();
    let mut poles = Vec::new();
    for port in [false, true] {
        if port {
            tank.level_all_tiles();
        } else {
            detours::level_all_tiles_real(this);
        }
        poles.push(snapshot_region());
        restore_owned_tiles(&original);
    }
    restore_owned_tiles(&untouched);
    if poles[0] != poles[1] {
        failures.push("levelAllTiles real and port left different terrain".to_string());
    }
    if poles[0] == original {
        failures.push("real levelAllTiles did not change the terrain (cannot detect a gap)".to_string());
    }
    let label = format!("{test_name} (region tiles: {})", region.len());
    finish_test(&label, failures, failure_log)
}

/// `removeIllegalEntities`' no-op paths against real vanilla: modes that flag nothing on every tank, and
/// every mode on the entity-free `Drain Tank` with `confirm` set (nothing flagged means the sell-off
/// dialog never opens). Each run is guarded by real `getIllegalEntities` returning empty sets first, so the
/// destructive and modal branches are never reached; those are covered by the `.asm` reading and interactive
/// play only.
pub(crate) fn run_tankexhibit_remove_illegal_entities_empty_set_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES_EMPTY_SET_MATCHES_REAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let tanks = live_tank_ptrs();
    if tanks.is_empty() {
        failures.push("no tank exhibits found in the loaded save".to_string());
    }
    let drain_tank = drain_test_tank_ptr();
    if drain_tank.is_none() {
        failures.push(format!("no tank named {DRAIN_TEST_TANK_NAME:?} in the loaded save"));
    }
    let (mut runs, mut skipped) = (0usize, 0usize);
    for &tank_ptr in &tanks {
        let this = tank_ptr as *const u32;
        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(tank_ptr) };
        let modes: Vec<i32> = if Some(tank_ptr) == drain_tank { vec![-1, 0, 1, 2, 3, 4] } else { vec![-1, 3, 4] };
        for mode in modes {
            let flagged_in = |mode: i32| {
                let set = VanillaEntitySet::new();
                detours::get_illegal_entities_real(this, set.addr() as *const std::ffi::c_void, mode);
                set.members().len()
            };
            if flagged_in(mode) != 0 || (mode == 1 && flagged_in(2) != 0) {
                skipped += 1;
                continue;
            }
            for confirm in [false, true] {
                let (real, port) = (detours::remove_illegal_entities_real(this, mode, confirm), tank.remove_illegal_entities(mode, confirm));
                runs += 1;
                if !real || !port {
                    failures.push(format!("tank {tank_ptr:#010x}: removeIllegalEntities({mode}, {confirm}) real={real}, port={port}, expected true"));
                }
            }
        }
    }
    if runs == 0 {
        failures.push("no mode ran (cannot detect a gap)".to_string());
    }
    let label = format!("{test_name} (tanks: {}, runs: {runs}, modes skipped because they flag entities: {skipped})", tanks.len());
    finish_test(&label, failures, failure_log)
}
