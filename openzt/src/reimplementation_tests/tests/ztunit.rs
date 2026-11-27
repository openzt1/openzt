//! Live checks for `ztunit.rs`. Each comparison builds standalone units with the real vanilla constructor over live
//! animal types, runs the real body (through the detour trampoline) on one and the Rust port on the other, and compares
//! return values and the unit's whole memory.

mod targets;

use openzt_detour::generated::ztunit as generated;
pub(crate) use targets::{run_ztunit_blocking_live_test, run_ztunit_paths_live_test, run_ztunit_satisfiers_live_test, run_ztunit_targets_live_test, run_ztunit_f_was_caught_live_test, run_ztunit_find_any_live_test, run_ztunit_find_nearest_live_test, run_ztunit_find_predator_live_test, run_ztunit_path_away_live_test, run_ztunit_z_dest_live_test};

use super::ztanimal::{live_animal_types, Lcg};
use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::harness::{finish_test, write_success_line},
    util::{get_from_memory, save_to_memory},
    ztunit::{
        live_support::{self, real},
        unit_offsets, ZTUnit, UNIT_SIZE,
    },
};

const POS_X: u32 = 0x114;
const POS_Y: u32 = 0x118;
const POS_Z: u32 = 0x11c;
/// The game's `rand` state (`DAT_00638060`).
const RNG_STATE_RVA: u32 = 0x0023_8060;

fn pin_rng(seed: u32) {
    save_to_memory(get_module_base("zoo.exe") as u32 + RNG_STATE_RVA, seed);
}

fn words(unit: ZTUnit) -> Vec<u32> {
    (0..UNIT_SIZE / 4).map(|i| get_from_memory::<u32>(unit.0 + i * 4)).collect()
}

/// Whether two words agree: equal, or both heap pointers.
fn equivalent(real: u32, port: u32) -> bool {
    real == port || (real >= 0x10000 && port >= 0x10000)
}

fn compare_memory(label: &str, real: ZTUnit, port: ZTUnit, failures: &mut Vec<String>) {
    for (i, (r, p)) in words(real).into_iter().zip(words(port)).enumerate() {
        if !equivalent(r, p) {
            failures.push(format!("{label}: +{:#x} real {r:#x}, port {p:#x}", i * 4));
        }
    }
}

fn place_at_tile(unit: ZTUnit, x: u32, y: u32) {
    save_to_memory(unit.0 + POS_X, x * 64 + 32);
    save_to_memory(unit.0 + POS_Y, y * 64 + 32);
    save_to_memory(unit.0 + POS_Z, 0u32);
}

/// Tiles to sample: the corners, the centre and a pseudo-random spread over the live map.
fn sample_tiles(count: usize) -> Vec<(u32, u32)> {
    let world = globals().ztworldmgr();
    let (width, height) = (world.map_x_size, world.map_y_size);
    let mut rng = Lcg(0x5eed);
    let mut tiles = vec![(0, 0), (width - 1, height - 1), (width / 2, height / 2)];
    while tiles.len() < count {
        tiles.push((rng.next() % width, rng.next() % height));
    }
    tiles
}

fn check<T: PartialEq + std::fmt::Debug>(failures: &mut Vec<String>, label: &str, real: T, port: T) {
    if real != port {
        failures.push(format!("{label}: real {real:?}, port {port:?}"));
    }
}

/// `ZTUNIT_DETOURS_ENABLED`: every `ZTUnit` detour reports enabled.
pub(crate) fn run_ztunit_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

/// Units over up to three live animal types, built by the real constructor.
fn build_units() -> Vec<ZTUnit> {
    live_animal_types(3).into_iter().map(live_support::build_standalone_unit).collect()
}

/// `ZTUNIT_LEAF_LIVE`: the tank-mode and tile-flag getters, the habitat lookups and `getDepth` against the real bodies
/// over every tank mode and a spread of live tiles, and through the hooked addresses.
pub(crate) fn run_ztunit_leaf_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_LEAF_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let units = build_units();
    if units.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    let world = globals().ztworldmgr();
    let tiles = sample_tiles(40);
    let mut seen_tank = false;
    let mut seen_water = false;
    for (unit_index, unit) in units.iter().enumerate() {
        for mode in 0..6u32 {
            unit.set_tank_mode(mode);
            check(&mut failures, &format!("unit {unit_index} getTankMode({mode})"), real::get_tank_mode(unit.0), unit.get_tank_mode());
            check(&mut failures, &format!("unit {unit_index} noShadowPart({mode})"), real::no_shadow_part(unit.0), unit.no_shadow_part());
            check(&mut failures, &format!("unit {unit_index} noUnderPart({mode})"), real::no_under_part(unit.0), unit.no_under_part());
            check(&mut failures, &format!("unit {unit_index} isOnPlatform({mode})"), real::is_on_platform(unit.0), unit.is_on_platform());
            check(&mut failures, &format!("unit {unit_index} isWaterTraveling"), real::is_water_traveling(unit.0), unit.is_water_traveling());
            check(&mut failures, &format!("unit {unit_index} getSwims"), real::get_swims(unit.0), unit.get_swims());
            check(&mut failures, &format!("unit {unit_index} getUnderwater"), real::get_underwater(unit.0), unit.get_underwater());
            check(&mut failures, &format!("unit {unit_index} getOnlyUnderwater"), real::get_only_underwater(unit.0), unit.get_only_underwater());
            let hooked = unsafe { generated::GET_TANK_MODE.hooked()(unit.0 as *const u32) };
            check(&mut failures, &format!("unit {unit_index} hooked getTankMode({mode})"), hooked, unit.get_tank_mode());
        }
        // setTankMode through the hooked address writes the same word.
        unsafe { generated::SET_TANK_MODE.hooked()(unit.0 as *const u32, 4) };
        check(&mut failures, "setTankMode writes +0x258", get_from_memory::<u32>(unit.0 + unit_offsets::TANK_MODE), 4);

        for &(x, y) in &tiles {
            place_at_tile(*unit, x, y);
            let tile = world.get_tile_ptr(x, y);
            let label = format!("unit {unit_index} at ({x},{y})");
            let (real_tank, port_tank) = (real::is_in_tank(unit.0), unit.is_in_tank());
            check(&mut failures, &format!("{label} isInTank"), real_tank, port_tank);
            let (real_water, port_water) = (real::is_on_water(unit.0), unit.is_on_water());
            check(&mut failures, &format!("{label} isOnWater"), real_water, port_water);
            seen_tank |= real_tank != 0;
            seen_water |= real_water;
            check(&mut failures, &format!("{label} getHabitatFromTile"), real::get_habitat_from_tile(unit.0, tile), unit.get_habitat_from_tile(tile));
            check(&mut failures, &format!("{label} getHabitatFromGate"), real::get_habitat_from_gate(unit.0, tile), unit.get_habitat_from_gate(tile));
            check(&mut failures, &format!("{label} getHabitat"), real::get_habitat(unit.0), unit.get_habitat());
            for flag in [false, true] {
                check(&mut failures, &format!("{label} getTank({flag})"), real::get_tank(unit.0, flag), unit.get_tank(flag));
            }
            check(&mut failures, &format!("{label} getDepth"), real::get_depth(unit.0, tile), unit.get_depth(tile));
            check(&mut failures, &format!("{label} getDepth(null)"), real::get_depth(unit.0, 0), unit.get_depth(0));
            let hooked_water = unsafe { generated::IS_ON_WATER.hooked()(unit.0 as *const u32) };
            check(&mut failures, &format!("{label} hooked isOnWater"), hooked_water, real_water);
            let hooked_habitat = unsafe { generated::GET_HABITAT.hooked()(unit.0 as *const u32) } as u32;
            check(&mut failures, &format!("{label} hooked getHabitat"), hooked_habitat, real::get_habitat(unit.0));
        }
    }
    if !seen_water && !seen_tank {
        // A map with neither water nor tanks still passes the comparisons; only note it.
        tracing::info!("{test_name}: the sampled tiles held no water or tank tile");
    }
    for unit in units {
        live_support::destroy_standalone_unit(unit);
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_ANIM_LIVE`: the idle/icon animation getters, the move-symbol getters and the speeds against the real bodies
/// over the tank modes, both dive directions and a spread of live tiles, and through the hooked addresses.
pub(crate) fn run_ztunit_anim_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_ANIM_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let units = build_units();
    if units.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    let tiles = sample_tiles(16);
    let mut symbols = std::collections::BTreeSet::new();
    for (unit_index, unit) in units.iter().enumerate() {
        for &(x, y) in &tiles {
            place_at_tile(*unit, x, y);
            for mode in [0u32, 2, 3, 4] {
                for direction in [-1i32, 0, 1] {
                    unit.set_tank_mode(mode);
                    save_to_memory(unit.0 + unit_offsets::VERTICAL_DIRECTION, direction);
                    let label = format!("unit {unit_index} at ({x},{y}) mode {mode} direction {direction}");
                    for flag in [false, true] {
                        check(&mut failures, &format!("{label} getIdleAnim({flag})"), real::get_idle_anim(unit.0, flag), unit.get_idle_anim(flag));
                    }
                    check(&mut failures, &format!("{label} getIconAnim"), real::get_icon_anim(unit.0), unit.get_icon_anim());
                    let idle = real::get_idle_anim_symbol(unit.0);
                    symbols.insert(idle);
                    check(&mut failures, &format!("{label} getIdleAnimSymbol"), idle, unit.get_idle_anim_symbol());
                    let slow = real::get_slow_anim_symbol(unit.0);
                    symbols.insert(slow);
                    check(&mut failures, &format!("{label} getSlowAnimSymbol"), slow, unit.get_slow_anim_symbol());
                    check(&mut failures, &format!("{label} getMediumAnimSymbol"), real::get_medium_anim_symbol(unit.0), unit.get_medium_anim_symbol());
                    check(&mut failures, &format!("{label} getFastAnimSymbol"), real::get_fast_anim_symbol(unit.0), unit.get_fast_anim_symbol());
                    check(&mut failures, &format!("{label} getSlowAnimSpeed"), real::get_slow_anim_speed(unit.0) & 0xffff, unit.get_slow_anim_speed() & 0xffff);
                    check(&mut failures, &format!("{label} getMediumAnimSpeed"), real::get_medium_anim_speed(unit.0) & 0xffff, unit.get_medium_anim_speed() & 0xffff);
                    check(&mut failures, &format!("{label} getFastAnimSpeed"), real::get_fast_anim_speed(unit.0) & 0xffff, unit.get_fast_anim_speed() & 0xffff);
                }
            }
        }
        // The hooked addresses reach the port.
        unit.set_tank_mode(4);
        save_to_memory(unit.0 + unit_offsets::VERTICAL_DIRECTION, -1i32);
        let hooked = unsafe { generated::GET_SLOW_ANIM_SYMBOL.hooked()(unit.0 as *const u32) } as u32;
        check(&mut failures, &format!("unit {unit_index} hooked getSlowAnimSymbol"), hooked, real::get_slow_anim_symbol(unit.0));
        let hooked = unsafe { generated::GET_FAST_ANIM_SPEED.hooked()(unit.0 as *const u32) } & 0xffff;
        check(&mut failures, &format!("unit {unit_index} hooked getFastAnimSpeed"), hooked, real::get_fast_anim_speed(unit.0) & 0xffff);
    }
    if symbols.len() < 2 {
        failures.push(format!("the sweep only produced {} distinct symbols", symbols.len()));
    }
    for unit in units {
        live_support::destroy_standalone_unit(unit);
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_AMBIENT_LIVE`: `doAmbientAnims` and `initAmbientAnims` leave twin units in the same state, with the game
/// RNG pinned so the ambient timers draw the same values.
pub(crate) fn run_ztunit_ambient_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_AMBIENT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(3);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    let tiles = sample_tiles(6);
    for (type_index, &unit_type) in types.iter().enumerate() {
        for mode in [0u32, 2, 3, 4] {
            for &(x, y) in &tiles {
                let real_unit = live_support::build_standalone_unit(unit_type);
                let port_unit = live_support::build_standalone_unit(unit_type);
                for unit in [real_unit, port_unit] {
                    unit.set_tank_mode(mode);
                    place_at_tile(unit, x, y);
                }
                let label = format!("type {type_index} mode {mode} at ({x},{y})");
                pin_rng(0x1234_5678);
                real::do_ambient_anims(real_unit.0, 0);
                pin_rng(0x1234_5678);
                port_unit.do_ambient_anims(0);
                compare_memory(&format!("{label} doAmbientAnims"), real_unit, port_unit, &mut failures);
                pin_rng(0x1234_5678);
                real::init_ambient_anims(real_unit.0, 0, c"AmbientAnims".as_ptr());
                pin_rng(0x1234_5678);
                port_unit.init_ambient_anims(0, c"AmbientAnims".as_ptr());
                compare_memory(&format!("{label} initAmbientAnims"), real_unit, port_unit, &mut failures);
                live_support::destroy_standalone_unit(real_unit);
                live_support::destroy_standalone_unit(port_unit);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

// ---- stages 2 and 3: lifecycle, persistence and goals -----------------------------------------------------------

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex, Once,
};

use openzt_detour::generated::bfunit::{LOAD as BFUNIT_LOAD, INTO_NEW_TILE as BFUNIT_INTO_NEW_TILE, REMOVE_FROM_MAP as BFUNIT_REMOVE_FROM_MAP, UPDATE_GOALS as BFUNIT_UPDATE_GOALS};

use crate::reimplementation_tests::io_redirect;

/// The `RNG_STATE_RVA` of the game `rand` state, as `pin_rng` uses.
fn rng_state() -> u32 {
    get_from_memory(get_module_base("zoo.exe") as u32 + RNG_STATE_RVA)
}

/// Calls the scripted vtable slots and the stubbed `BFUnit` tails record, in order.
static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// What the scripted `isDoingShow` slot answers.
static SHOW_ANSWER: AtomicU32 = AtomicU32::new(0);
/// While set, `BFUnit::intoNewTile`/`updateGoals`/`removeFromMap` are recorded instead of run.
static STUB_BFUNIT: AtomicU32 = AtomicU32::new(0);
/// While set, `BFUnit::load` is recorded and answers success without reading.
static STUB_BFUNIT_LOAD: AtomicU32 = AtomicU32::new(0);

fn log(entry: String) {
    LOG.lock().unwrap().push(entry);
}

fn take_log() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

#[openzt_detour_macro::detour_mod]
mod bfunit_stubs {
    use super::*;

    #[detour(BFUNIT_INTO_NEW_TILE)]
    unsafe extern "thiscall" fn into_new_tile(this: *const u32, direction: u32) {
        if STUB_BFUNIT.load(Ordering::SeqCst) != 0 {
            log(format!("BFUnit::intoNewTile {direction}"));
        } else {
            unsafe { BFUNIT_INTO_NEW_TILE_DETOUR.call(this, direction) }
        }
    }

    #[detour(BFUNIT_UPDATE_GOALS)]
    unsafe extern "thiscall" fn update_goals(this: *const u32) {
        if STUB_BFUNIT.load(Ordering::SeqCst) != 0 {
            log("BFUnit::updateGoals".to_string());
        } else {
            unsafe { BFUNIT_UPDATE_GOALS_DETOUR.call(this) }
        }
    }

    #[detour(BFUNIT_LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        if STUB_BFUNIT_LOAD.load(Ordering::SeqCst) != 0 {
            log(format!("BFUnit::load {version:#x}"));
            true
        } else {
            unsafe { BFUNIT_LOAD_DETOUR.call(this, file, version) }
        }
    }

    #[detour(BFUNIT_REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, keep: bool) {
        if STUB_BFUNIT.load(Ordering::SeqCst) != 0 {
            log(format!("BFUnit::removeFromMap {keep}"));
        } else {
            unsafe { BFUNIT_REMOVE_FROM_MAP_DETOUR.call(this, keep) }
        }
    }
}

fn install_stubs() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { bfunit_stubs::init_detours() } {
            result = Err(format!("could not install the BFUnit stubs: {e:?}"));
        }
    });
    result
}

extern "thiscall" fn fake_is_doing_show(_this: u32) -> u32 {
    log("isDoingShow".to_string());
    SHOW_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_is_show_unit(_this: u32) -> u32 {
    log("isShowUnit".to_string());
    0
}
extern "thiscall" fn fake_get_habitat(_this: u32) -> u32 {
    log("getHabitat".to_string());
    0
}
extern "thiscall" fn fake_get_tank(_this: u32, keep: u32) -> u32 {
    log(format!("getTank {}", keep & 0xff));
    0
}
extern "thiscall" fn fake_abort_trick(_this: u32) {
    log("abortTrick".to_string());
}
extern "thiscall" fn fake_end_trick(_this: u32) {
    log("endTrick".to_string());
}
extern "thiscall" fn fake_clear_goal(_this: u32) {
    log("clearGoal".to_string());
}
extern "thiscall" fn fake_stop_everything(_this: u32, a: u32, b: u32, c: u32) {
    log(format!("stopEverything {} {} {}", a & 0xff, b & 0xff, c & 0xff));
}
extern "thiscall" fn fake_goal_set(_this: u32, goal: u32, argument: u32) {
    log(format!("goalSet {goal:#x} {argument}"));
}
extern "thiscall" fn fake_set_target_tile(_this: u32, tile: u32, flag: u32) {
    log(format!("setTargetTile {tile:#x} {}", flag & 0xff));
}
extern "thiscall" fn fake_set_building_tile(_this: u32, tile: u32) {
    log(format!("setBuildingTile {}", tile != 0));
}
extern "thiscall" fn fake_update_sub_goals(_this: u32) {
    log("updateSubGoals".to_string());
}
extern "thiscall" fn fake_reset_move_anim(_this: u32) {
    log("resetMoveAnim".to_string());
}
extern "thiscall" fn fake_force_ui_update(_this: u32) {
    log("forceUIUpdate".to_string());
}
extern "thiscall" fn fake_cog_update(_this: u32) {
    log("cogGoal update".to_string());
}

fn leaked_words(words: Vec<u32>) -> u32 {
    Box::leak(words.into_boxed_slice()).as_ptr() as u32
}

/// Replaces the unit's vtable and cog-goal vtable with copies whose goal-related slots record instead of acting.
fn script_unit(unit: ZTUnit) {
    let real_vtable: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    let scripted: [(u32, u32); 14] = [
        (0x3c, fake_update_sub_goals as *const () as u32),
        (0x138, fake_reset_move_anim as *const () as u32),
        (0x174, fake_clear_goal as *const () as u32),
        (0x198, fake_stop_everything as *const () as u32),
        (0x1d8, fake_goal_set as *const () as u32),
        (0x228, fake_is_show_unit as *const () as u32),
        (0x22c, fake_is_doing_show as *const () as u32),
        (0x24c, fake_get_habitat as *const () as u32),
        (0x250, fake_get_tank as *const () as u32),
        (0x2bc, fake_end_trick as *const () as u32),
        (0x2c0, fake_abort_trick as *const () as u32),
        (0x2d0, fake_force_ui_update as *const () as u32),
        (0x2d8, fake_set_target_tile as *const () as u32),
        (0x2e4, fake_set_building_tile as *const () as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    save_to_memory(unit.0, leaked_words(vtable));
    let cog = unit.0 + unit_offsets::COG_GOAL;
    let real_cog: u32 = get_from_memory(cog);
    let mut cog_vtable: Vec<u32> = (0..16u32).map(|slot| get_from_memory(real_cog + slot * 4)).collect();
    cog_vtable[(0x24 / 4) as usize] = fake_cog_update as *const () as u32;
    save_to_memory(cog, leaked_words(cog_vtable));
}

/// Puts the unit's real vtables back before the vanilla destructor runs.
fn unscript_unit(unit: ZTUnit, real_vtable: u32, real_cog: u32) {
    save_to_memory(unit.0, real_vtable);
    save_to_memory(unit.0 + unit_offsets::COG_GOAL, real_cog);
}

struct Twins {
    real: ZTUnit,
    port: ZTUnit,
    vtables: (u32, u32),
}

impl Twins {
    fn new(unit_type: u32, scripted: bool) -> Twins {
        let (real, port) = (live_support::build_standalone_unit(unit_type), live_support::build_standalone_unit(unit_type));
        let vtables = (get_from_memory(real.0), get_from_memory(real.0 + unit_offsets::COG_GOAL));
        if scripted {
            script_unit(real);
            script_unit(port);
        }
        Twins { real, port, vtables }
    }

    fn both(&self) -> [ZTUnit; 2] {
        [self.real, self.port]
    }

    fn destroy(self) {
        for unit in [self.real, self.port] {
            unscript_unit(unit, self.vtables.0, self.vtables.1);
            live_support::destroy_standalone_unit(unit);
        }
    }
}

/// Fills a unit's own fields (`0x214..0x260`) with the same pseudo-random words, keeping the pointer fields clear.
fn randomize_own_fields(unit: ZTUnit, seed: u32) {
    let mut rng = Lcg(seed);
    for offset in (0x214..UNIT_SIZE).step_by(4) {
        let value = match offset {
            0x234 | 0x244 | 0x248 => 0,
            0x258 => rng.next() % 6,
            _ => rng.next(),
        };
        save_to_memory(unit.0 + offset, value);
    }
    save_to_memory(unit.0 + unit_offsets::NEED_INDEX, (seed & 0xff) as u8);
}

fn compare_log(label: &str, real: &[String], port: &[String], failures: &mut Vec<String>) {
    if real != port {
        failures.push(format!("{label}: real calls {real:?}, port calls {port:?}"));
    }
}

/// `ZTUNIT_LIFECYCLE_LIVE`: the constructor, `initStatusVars`, `clearStateVariables` and `setIsRemoved` against the
/// real bodies, and through the hooked addresses.
pub(crate) fn run_ztunit_lifecycle_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_LIFECYCLE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(3);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    for (type_index, &unit_type) in types.iter().enumerate() {
        // Constructor: the port builds over zeroed memory, the real one over its own.
        let real = live_support::build_standalone_unit(unit_type);
        let port_memory = live_support::allocate_unit_memory();
        let returned = ZTUnit(port_memory).construct(unit_type);
        let port = ZTUnit(port_memory);
        check(&mut failures, &format!("type {type_index} constructor returns this"), returned, port_memory);
        compare_memory(&format!("type {type_index} constructor"), real, port, &mut failures);
        live_support::destroy_standalone_unit(real);
        live_support::destroy_standalone_unit(port);

        let twins = Twins::new(unit_type, false);
        for seed in 1..=6u32 {
            for unit in twins.both() {
                randomize_own_fields(unit, seed * 0x9e37);
            }
            real::init_status_vars(twins.real.0);
            twins.port.init_status_vars();
            compare_memory(&format!("type {type_index} seed {seed} initStatusVars"), twins.real, twins.port, &mut failures);
            for unit in twins.both() {
                randomize_own_fields(unit, seed * 0x7f4a);
            }
            real::clear_state_variables(twins.real.0);
            twins.port.clear_state_variables();
            compare_memory(&format!("type {type_index} seed {seed} clearStateVariables"), twins.real, twins.port, &mut failures);
            for unit in twins.both() {
                randomize_own_fields(unit, seed * 0x1357);
            }
            unsafe { generated::INIT_STATUS_VARS.hooked()(twins.port.0 as *const u32) };
            real::init_status_vars(twins.real.0);
            compare_memory(&format!("type {type_index} seed {seed} hooked initStatusVars"), twins.real, twins.port, &mut failures);
        }
        // setIsRemoved needs no map view; with one the map view's undo buffer would change under both poles.
        if unsafe { openzt_detour::generated::ztui_general::GET_MAPVIEW.hooked()() } as u32 == 0 {
            for flag in [0u8, 1, 7] {
                real::set_is_removed(twins.real.0, flag);
                twins.port.set_is_removed(flag);
                compare_memory(&format!("type {type_index} setIsRemoved({flag})"), twins.real, twins.port, &mut failures);
            }
        }
        twins.destroy();
    }
    finish_test(test_name, failures, failure_log)
}

fn capture_save(unit: ZTUnit, port: bool) -> (bool, Vec<u8>) {
    io_redirect::begin_capture();
    let result = if port { unit.save(0) } else { real::save(unit.0, 0) };
    (result, io_redirect::end_capture())
}

fn replay_load(unit: ZTUnit, bytes: &[u8], version: u32, port: bool) -> (bool, usize) {
    io_redirect::begin_replay(bytes.to_vec());
    let result = if port { unit.load(0, version) } else { real::load(unit.0, 0, version) };
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed)
}

/// `ZTUNIT_SAVE_LOAD_LIVE`: the bytes `save` writes, and what `load` reads and leaves behind for several versions and
/// for truncated streams, against the real bodies.
pub(crate) fn run_ztunit_save_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_SAVE_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    let world = globals().ztworldmgr();
    let tile = world.get_tile_ptr(world.map_x_size / 2, world.map_y_size / 2);
    for (type_index, &unit_type) in types.iter().enumerate() {
        for seed in 1..=8u32 {
            let source = live_support::build_standalone_unit(unit_type);
            randomize_own_fields(source, seed * 0x5bd1);
            // Positions that map back into the zoo, a building tile on odd seeds, trick words on most seeds.
            save_to_memory(source.0 + unit_offsets::TARGET_POSITION, 64 * 5 + 7);
            save_to_memory(source.0 + unit_offsets::TARGET_POSITION + 4, 64 * 6 + 9);
            save_to_memory(source.0 + unit_offsets::BUILDING_TILE, if seed % 2 == 1 { tile } else { 0 });
            save_to_memory(source.0 + unit_offsets::TRICK_TYPE, if seed % 3 == 0 { 0u16 } else { seed as u16 });
            save_to_memory(source.0 + unit_offsets::SHOW_INFO_ID, if seed % 4 == 0 { 0u16 } else { seed as u16 + 100 });
            let label = format!("type {type_index} seed {seed}");
            let (real_ok, real_bytes) = capture_save(source, false);
            let (port_ok, port_bytes) = capture_save(source, true);
            check(&mut failures, &format!("{label} save result"), real_ok, port_ok);
            if real_bytes != port_bytes {
                failures.push(format!("{label} save bytes differ: real {} bytes, port {} bytes", real_bytes.len(), port_bytes.len()));
            }
            // `BFUnit::load` is stubbed for the version sweep: only the current layout matches the saved bytes.
            STUB_BFUNIT_LOAD.store(1, Ordering::SeqCst);
            for version in [0x5bu32, 0x5f, 0x60, 0x61, 0x62, 0x64, 0x65] {
                let twins = Twins::new(unit_type, false);
                take_log();
                let (real_result, real_consumed) = replay_load(twins.real, &real_bytes, version, false);
                let real_calls = take_log();
                let (port_result, port_consumed) = replay_load(twins.port, &real_bytes, version, true);
                let port_calls = take_log();
                let load_label = format!("{label} load v{version:#x}");
                check(&mut failures, &format!("{load_label} result"), real_result, port_result);
                check(&mut failures, &format!("{load_label} bytes consumed"), real_consumed, port_consumed);
                compare_log(&load_label, &real_calls, &port_calls, &mut failures);
                compare_memory(&load_label, twins.real, twins.port, &mut failures);
                twins.destroy();
            }
            STUB_BFUNIT_LOAD.store(0, Ordering::SeqCst);
            {
                // The whole chain at the current layout.
                let twins = Twins::new(unit_type, false);
                let (real_result, real_consumed) = replay_load(twins.real, &real_bytes, 0x65, false);
                let (port_result, port_consumed) = replay_load(twins.port, &real_bytes, 0x65, true);
                check(&mut failures, &format!("{label} full load result"), real_result, port_result);
                check(&mut failures, &format!("{label} full load bytes consumed"), real_consumed, port_consumed);
                compare_memory(&format!("{label} full load"), twins.real, twins.port, &mut failures);
                twins.destroy();
            }
            // Truncated streams fail at the same place.
            STUB_BFUNIT_LOAD.store(1, Ordering::SeqCst);
            for cut in [0usize, 1, 5, 9, 20, 33, 47] {
                let twins = Twins::new(unit_type, false);
                let truncated = &real_bytes[..cut.min(real_bytes.len())];
                let (real_result, real_consumed) = replay_load(twins.real, truncated, 0x65, false);
                let (port_result, port_consumed) = replay_load(twins.port, truncated, 0x65, true);
                let load_label = format!("{label} load truncated at {cut}");
                check(&mut failures, &format!("{load_label} result"), real_result, port_result);
                check(&mut failures, &format!("{load_label} bytes consumed"), real_consumed, port_consumed);
                compare_memory(&load_label, twins.real, twins.port, &mut failures);
                twins.destroy();
            }
            STUB_BFUNIT_LOAD.store(0, Ordering::SeqCst);
            live_support::destroy_standalone_unit(source);
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_GOALS_LIVE`: the goal predicates and mutators against the real bodies with the unit's goal-related virtual
/// slots scripted, comparing the unit's memory and the calls made through the scripted slots.
pub(crate) fn run_ztunit_goals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_GOALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    STUB_BFUNIT.store(1, Ordering::SeqCst);
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, true);
        for goal in (0..0x90u32).chain([0xffff_ffff]) {
            for busy in [0u8, 1] {
                for show in [0u32, 1] {
                    SHOW_ANSWER.store(show, Ordering::SeqCst);
                    for unit in twins.both() {
                        save_to_memory(unit.0 + unit_offsets::GOAL_ID, goal);
                        save_to_memory(unit.0 + unit_offsets::BUSY, busy);
                    }
                    let label = format!("type {type_index} goal {goal:#x} busy {busy} show {show}");
                    take_log();
                    let real_doing = real::is_doing_something(twins.real.0) & 0xff != 0;
                    let real_calls = take_log();
                    let port_doing = twins.port.is_doing_something();
                    let port_calls = take_log();
                    check(&mut failures, &format!("{label} isDoingSomething"), real_doing, port_doing);
                    compare_log(&format!("{label} isDoingSomething"), &real_calls, &port_calls, &mut failures);
                    let hooked = unsafe { generated::IS_DOING_SOMETHING.hooked()(twins.port.0 as *const u32) } & 0xff != 0;
                    take_log();
                    check(&mut failures, &format!("{label} hooked isDoingSomething"), hooked, real_doing);
                    check(&mut failures, &format!("{label} getRandomOK"), real::get_random_ok(twins.real.0) & 0xff, u32::from(twins.port.get_random_ok()));
                }
            }
        }
        SHOW_ANSWER.store(0, Ordering::SeqCst);

        // doBuilding, abortGoal, clearGoal, goalSet and resetAI over random fields.
        for seed in 1..=10u32 {
            for show in [0u32, 1] {
                SHOW_ANSWER.store(show, Ordering::SeqCst);
                for unit in twins.both() {
                    randomize_own_fields(unit, seed * 0x2545);
                }
                let label = format!("type {type_index} seed {seed} show {show}");
                let calls: [(&str, fn(ZTUnit), fn(ZTUnit)); 4] = [
                    ("doBuilding", |u| real::do_building(u.0), |u| u.do_building()),
                    ("abortGoal", |u| real::abort_goal(u.0), |u| u.abort_goal()),
                    ("clearGoal", |u| real::clear_goal(u.0), |u| u.clear_goal()),
                    ("goalSet", |u| real::goal_set(u.0, 0x69, 1), |u| u.goal_set(0x69, 1)),
                ];
                for (name, real_call, port_call) in calls {
                    take_log();
                    real_call(twins.real);
                    let real_calls = take_log();
                    port_call(twins.port);
                    let port_calls = take_log();
                    compare_memory(&format!("{label} {name}"), twins.real, twins.port, &mut failures);
                    compare_log(&format!("{label} {name}"), &real_calls, &port_calls, &mut failures);
                }
                // A need index equal to the building need exercises doBuilding's slot call.
                let building_need = unsafe { openzt_detour::generated::ztworldmgr::GET_NEED_INDEX.hooked()((get_module_base("zoo.exe") as u32 + 0x0023_afd8) as *const i8) } as u8;
                for unit in twins.both() {
                    save_to_memory(unit.0 + unit_offsets::NEED_INDEX, building_need);
                }
                take_log();
                real::do_building(twins.real.0);
                let real_calls = take_log();
                twins.port.do_building();
                let port_calls = take_log();
                compare_log(&format!("{label} doBuilding(need match)"), &real_calls, &port_calls, &mut failures);
                if port_calls != ["endTrick".to_string()] {
                    failures.push(format!("{label} doBuilding(need match) did not end the trick: {port_calls:?}"));
                }
                // resetAI: the type id slot is the entity type's, so both poles read the live type.
                for countdown in [0u32, 7] {
                    for unit in twins.both() {
                        save_to_memory(unit.0 + unit_offsets::RESET_COUNTDOWN, countdown);
                    }
                    pin_rng(0x1234_5678 ^ seed);
                    take_log();
                    let real_value = real::reset_ai(twins.real.0);
                    let real_calls = take_log();
                    let real_rng = rng_state();
                    pin_rng(0x1234_5678 ^ seed);
                    let port_value = twins.port.reset_ai();
                    let port_calls = take_log();
                    let port_rng = rng_state();
                    check(&mut failures, &format!("{label} resetAI({countdown}) value"), real_value, port_value);
                    check(&mut failures, &format!("{label} resetAI({countdown}) rng"), real_rng, port_rng);
                    compare_log(&format!("{label} resetAI({countdown})"), &real_calls, &port_calls, &mut failures);
                    compare_memory(&format!("{label} resetAI({countdown})"), twins.real, twins.port, &mut failures);
                }
            }
        }
        SHOW_ANSWER.store(0, Ordering::SeqCst);

        // The hooked addresses reach the port.
        take_log();
        unsafe { generated::GOAL_SET.hooked()(twins.port.0 as *const u32, 0x69, 1) };
        unsafe { generated::CLEAR_GOAL.hooked()(twins.port.0 as *const u32) };
        let hooked_calls = take_log();
        if !hooked_calls.contains(&"setBuildingTile false".to_string()) {
            failures.push(format!("hooked clearGoal made no scripted calls: {hooked_calls:?}"));
        }
        twins.destroy();
    }
    STUB_BFUNIT.store(0, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}

/// `ZTUNIT_TICK_LIVE`: `updateGoals`, `intoNewTile` and `removeFromMap` against the real bodies, with the `BFUnit` tails
/// and the unit's goal slots recorded, over a spread of live tiles and directions.
pub(crate) fn run_ztunit_tick_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_TICK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    let world = globals().ztworldmgr();
    // Vanilla `getGate` reads the neighbour tile unchecked, so the comparison keeps off the map edge.
    let (width, height) = (world.map_x_size, world.map_y_size);
    let tiles: Vec<(u32, u32)> = sample_tiles(40).into_iter().filter(|&(x, y)| x >= 1 && y >= 1 && x + 1 < width && y + 1 < height).collect();
    STUB_BFUNIT.store(1, Ordering::SeqCst);
    let mut building_cases = 0;
    let roll_seed = |taken: bool| {
        (0u32..)
            .find(|seed| {
                let next = seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
                (((next >> 16) & 0x7fff) % 15 == 0) == taken
            })
            .unwrap()
    };
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, true);
        for &(x, y) in &tiles {
            for unit in twins.both() {
                place_at_tile(unit, x, y);
            }
            let label = format!("type {type_index} at ({x},{y})");
            let tile = world.get_tile_ptr(x, y);
            if get_from_memory::<u32>(tile + 0x10) != 0 {
                building_cases += 1;
            }
            // updateGoals: one pinned roll that takes the once-in-fifteen branch and one that does not.
            for taken in [true, false] {
                let seed = roll_seed(taken);
                for unit in twins.both() {
                    save_to_memory(unit.0 + unit_offsets::BUILDING_STATE, 0u32);
                    save_to_memory(unit.0 + unit_offsets::GOAL_ID, 0x10u32);
                }
                pin_rng(seed);
                take_log();
                real::update_goals(twins.real.0);
                let real_calls = take_log();
                pin_rng(seed);
                twins.port.update_goals();
                let port_calls = take_log();
                compare_memory(&format!("{label} updateGoals(rolled {taken})"), twins.real, twins.port, &mut failures);
                compare_log(&format!("{label} updateGoals(rolled {taken})"), &real_calls, &port_calls, &mut failures);
            }
            for direction in 0..8u32 {
                let real_gate = unsafe { crate::ztunit::ZTUNIT_GET_GATE.hooked()(twins.real.0 as *const u32, tile as *const u32, direction) };
                check(&mut failures, &format!("{label} getGate({direction})"), real_gate, ZTUnit::get_gate(tile, direction));
                for unit in twins.both() {
                    place_at_tile(unit, x, y);
                    save_to_memory(unit.0 + unit_offsets::MOVE_ANIM_DIRTY, 0u8);
                }
                take_log();
                real::into_new_tile(twins.real.0, direction);
                let real_calls = take_log();
                twins.port.into_new_tile(direction);
                let port_calls = take_log();
                compare_memory(&format!("{label} intoNewTile({direction})"), twins.real, twins.port, &mut failures);
                compare_log(&format!("{label} intoNewTile({direction})"), &real_calls, &port_calls, &mut failures);
            }
            // removeFromMap, with and without keeping, and with the lock byte set.
            for keep in [false, true] {
                for locked in [0u8, 1] {
                    for unit in twins.both() {
                        place_at_tile(unit, x, y);
                        save_to_memory(unit.0 + unit_offsets::MBR_0X142, locked);
                        save_to_memory(unit.0 + unit_offsets::TANK_MODE, 3u32);
                    }
                    take_log();
                    real::remove_from_map(twins.real.0, keep);
                    let real_calls = take_log();
                    twins.port.remove_from_map(keep);
                    let port_calls = take_log();
                    let case = format!("{label} removeFromMap(keep {keep}, locked {locked})");
                    compare_memory(&case, twins.real, twins.port, &mut failures);
                    compare_log(&case, &real_calls, &port_calls, &mut failures);
                }
            }
        }
        twins.destroy();
    }
    STUB_BFUNIT.store(0, Ordering::SeqCst);
    tracing::info!("{test_name}: {building_cases} sampled tiles held an entity in slot 0x10");
    finish_test(test_name, failures, failure_log)
}

// ---- validateTankPosition ---------------------------------------------------------------------------------------

static TANK_IN_TANK: AtomicU32 = AtomicU32::new(0);
static TANK_ANSWER: AtomicU32 = AtomicU32::new(0);
static TANK_HABITAT: AtomicU32 = AtomicU32::new(0);
static TANK_ONLY_UNDERWATER: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn tank_is_in_tank(_this: u32) -> u32 {
    log("isInTank".to_string());
    TANK_IN_TANK.load(Ordering::SeqCst)
}
extern "thiscall" fn tank_get_tank(_this: u32, flag: u32) -> u32 {
    log(format!("getTank {}", flag & 0xff));
    TANK_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn tank_habitat_from_tile(_this: u32, tile: u32) -> u32 {
    log(format!("getHabitatFromTile {}", tile != 0));
    TANK_HABITAT.load(Ordering::SeqCst)
}
extern "thiscall" fn tank_only_underwater(_this: u32) -> u32 {
    log("getOnlyUnderwater".to_string());
    TANK_ONLY_UNDERWATER.load(Ordering::SeqCst)
}
extern "thiscall" fn tank_set_height(_this: u32, height: u32) {
    log(format!("setHeightAboveTerrain {}", height as i32));
}
extern "thiscall" fn tank_set_mode(_this: u32, mode: u32) {
    log(format!("setTankMode {mode}"));
}
extern "thiscall" fn tank_reset_idle(_this: u32) {
    log("resetIdleAnim".to_string());
}
extern "thiscall" fn tank_invalidate_target(_this: u32) {
    log("invalidateTarget".to_string());
}

fn script_unit_for_tank(unit: ZTUnit) {
    let real_vtable: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    let scripted: [(u32, u32); 9] = [
        (0x138, fake_reset_move_anim as *const () as u32),
        (0x13c, tank_reset_idle as *const () as u32),
        (0x170, tank_only_underwater as *const () as u32),
        (0x17c, tank_invalidate_target as *const () as u32),
        (0x1cc, tank_set_height as *const () as u32),
        (0x248, tank_habitat_from_tile as *const () as u32),
        (0x250, tank_get_tank as *const () as u32),
        (0x268, tank_is_in_tank as *const () as u32),
        (0x2cc, tank_set_mode as *const () as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    save_to_memory(unit.0, leaked_words(vtable));
}

/// Every live tank exhibit's address.
fn live_tanks() -> Vec<u32> {
    let habitat_mgr = globals().zthabitatmgr();
    (0..habitat_mgr.exhibit_array().len())
        .map(|i| habitat_mgr.exhibit_array().get_ptr(i))
        .filter(|&ptr| ptr != 0 && unsafe { crate::util::ref_from_memory::<crate::zthabitatmgr::ZTHabitat>(ptr) }.is_tank())
        .collect()
}

/// `ZTUNIT_TANK_LIVE`: `validateTankPosition` against the real body with the unit's tank-related virtual slots
/// scripted, over the live zoo's tanks (their gate tiles and ladders) and over a unit outside every tank.
pub(crate) fn run_ztunit_tank_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_TANK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(2);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    let tanks = live_tanks();
    let mut ladder_cases = 0;
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, false);
        for unit in twins.both() {
            script_unit_for_tank(unit);
        }
        let mut rng = Lcg(0x7a4b + type_index as u32);
        // Candidate tiles: every tank's gate tiles, plus a few other tiles.
        let mut tiles: Vec<(u32, u32, u32)> = vec![(0, 0, 0)];
        for &tank in &tanks {
            let gate_in = unsafe { openzt_detour::generated::zthabitat::GET_GATE_TILE_IN.hooked()(tank as *const u32) } as u32;
            let gate_out = unsafe { openzt_detour::generated::zthabitat::GET_GATE_TILE_OUT.hooked()(tank as *const u32) } as u32;
            for tile in [gate_in, gate_out] {
                if tile != 0 {
                    let (x, y): (u32, u32) = (get_from_memory(tile + 0x34), get_from_memory(tile + 0x38));
                    tiles.push((tank, x, y));
                }
            }
        }
        for &(tank, x, y) in &tiles {
            for _ in 0..24 {
                let label = format!("type {type_index} tank {tank:#x} at ({x},{y})");
                place_at_tile(twins.real, x, y);
                place_at_tile(twins.port, x, y);
                TANK_IN_TANK.store(rng.next() % 2, Ordering::SeqCst);
                TANK_ANSWER.store(if tank != 0 && rng.next() % 4 != 0 { tank } else { 0 }, Ordering::SeqCst);
                TANK_HABITAT.store(if tank != 0 && rng.next() % 2 == 0 { tank } else { 0 }, Ordering::SeqCst);
                TANK_ONLY_UNDERWATER.store(rng.next() % 2, Ordering::SeqCst);
                let goal_state = [0u32, 0x14, 0x15, 0x1c, 0x1d, 0x13][(rng.next() % 6) as usize];
                let ladder_height = (rng.next() % 4) as i32 * 16 - 16 + (rng.next() % 3) as i32;
                let (tank_mode, target, direction, animating) = (rng.next() % 6, if rng.next() % 2 == 0 { 0u32 } else { 0x1000u32 }, (rng.next() % 3) as i32 - 1, (rng.next() % 2) as u8);
                for unit in twins.both() {
                    save_to_memory(unit.0 + unit_offsets::COG_GOAL_STATE, goal_state);
                    save_to_memory(unit.0 + unit_offsets::TANK_MODE, tank_mode);
                    save_to_memory(unit.0 + unit_offsets::HEIGHT, ladder_height);
                    save_to_memory(unit.0 + unit_offsets::POS_Z, ladder_height);
                    save_to_memory(unit.0 + unit_offsets::TARGET_TILE, target);
                    save_to_memory(unit.0 + unit_offsets::VERTICAL_DIRECTION, direction);
                    save_to_memory(unit.0 + unit_offsets::MOVE_ANIM_ENABLED, animating);
                    save_to_memory(unit.0 + unit_offsets::BUILDING_STATE, 0u32);
                }
                take_log();
                real::validate_tank_position(twins.real.0);
                let real_calls = take_log();
                twins.port.validate_tank_position();
                let port_calls = take_log();
                if real_calls.iter().any(|call| call.starts_with("invalidateTarget")) {
                    ladder_cases += 1;
                }
                compare_memory(&label, twins.real, twins.port, &mut failures);
                compare_log(&label, &real_calls, &port_calls, &mut failures);
            }
        }
        twins.destroy();
    }
    tracing::info!("{test_name}: {} live tank(s), {ladder_cases} ladder repositions", tanks.len());
    finish_test(test_name, failures, failure_log)
}

// ---- addToMap and createName ------------------------------------------------------------------------------------

use openzt_detour::generated::bfunit::ADD_TO_MAP as BFUNIT_ADD_TO_MAP;

static AM_SHOW: AtomicU32 = AtomicU32::new(0);
/// While set, `BFUnit::addToMap` is recorded and answers `1` instead of adding the standalone unit to the map.
static STUB_ADD_TO_MAP: AtomicU32 = AtomicU32::new(0);
static STUB_ADD_RESULT: AtomicU32 = AtomicU32::new(1);

#[openzt_detour_macro::detour_mod]
mod add_to_map_stub {
    use super::*;

    #[detour(BFUNIT_ADD_TO_MAP)]
    unsafe extern "thiscall" fn add_to_map(this: *const u32) -> bool {
        if STUB_ADD_TO_MAP.load(Ordering::SeqCst) != 0 {
            log("BFUnit::addToMap".to_string());
            STUB_ADD_RESULT.load(Ordering::SeqCst) != 0
        } else {
            unsafe { BFUNIT_ADD_TO_MAP_DETOUR.call(this) }
        }
    }
}

extern "thiscall" fn am_is_show_unit(_this: u32) -> u32 {
    log("isShowUnit".to_string());
    AM_SHOW.load(Ordering::SeqCst)
}
extern "thiscall" fn am_validate_position(_this: u32, flag: u32) {
    log(format!("validatePosition {}", flag & 0xff));
}
extern "thiscall" fn am_run_behavior_set(_this: u32) {
    log("runBehaviorSet".to_string());
}
extern "thiscall" fn am_set_name(_this: u32, string: u32) {
    let (begin, end): (u32, u32) = (get_from_memory(string), get_from_memory(string + 4));
    let bytes: Vec<u8> = (begin..end).map(get_from_memory::<u8>).collect();
    log(format!("setName {}", String::from_utf8_lossy(&bytes)));
}
fn am_set_bs(_this: u32, begin: u32, end: u32, call_flag: i8, force: i8, clear_mode: i32) -> Option<bool> {
    let bytes: Vec<u8> = (begin..end).map(get_from_memory::<u8>).collect();
    log(format!("setBS {} {call_flag} {force} {clear_mode}", String::from_utf8_lossy(&bytes)));
    Some(true)
}
extern "thiscall" fn mermaid_type_id(_this: u32) -> u32 {
    0x1530
}

/// A vtable copy of the unit with the slots `addToMap` and `createName` reach scripted.
fn script_unit_for_add_to_map(unit: ZTUnit) {
    let real_vtable: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    let scripted: [(u32, u32); 8] = [
        (0x1c, am_set_name as *const () as u32),
        (0x34, am_run_behavior_set as *const () as u32),
        (0x84, am_validate_position as *const () as u32),
        (0x138, fake_reset_move_anim as *const () as u32),
        (0x13c, tank_reset_idle as *const () as u32),
        (0x228, am_is_show_unit as *const () as u32),
        (0x24c, fake_get_habitat as *const () as u32),
        (0x250, tank_get_tank as *const () as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    save_to_memory(unit.0, leaked_words(vtable));
}

/// `ZTUNIT_ADD_TO_MAP_LIVE`: `addToMap` against the real body with `BFUnit::addToMap` and the unit's virtual slots
/// scripted, including a stand-in entity type that reports the mermaid type id so the transform tail runs.
pub(crate) fn run_ztunit_add_to_map_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_ADD_TO_MAP_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    static ADD_STUB_INSTALLED: Once = Once::new();
    let mut install_error = None;
    ADD_STUB_INSTALLED.call_once(|| {
        if let Err(e) = unsafe { add_to_map_stub::init_detours() } {
            install_error = Some(format!("could not install the BFUnit::addToMap stub: {e:?}"));
        }
    });
    if let Some(e) = install_error {
        return finish_test(test_name, vec![e], failure_log);
    }
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(am_set_bs);
    let types = live_animal_types(2);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    STUB_ADD_TO_MAP.store(1, Ordering::SeqCst);
    // A stand-in entity type: a pointer to a vtable whose type-id slot answers the mermaid id.
    let real_type_vtable: Vec<u32> = (0..0x40u32).map(|_| mermaid_type_id as *const () as u32).collect();
    let fake_type = leaked_words(vec![leaked_words(real_type_vtable)]);
    let mut mermaid_runs = 0;
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, false);
        for unit in twins.both() {
            script_unit_for_add_to_map(unit);
        }
        for added in [0u32, 1] {
            STUB_ADD_RESULT.store(added, Ordering::SeqCst);
            for show in [0u32, 1] {
                for tank in [0u32, 1] {
                    for locked in [0u8, 1] {
                        for animating in [0u8, 1] {
                            for mermaid in [false, true] {
                                AM_SHOW.store(show, Ordering::SeqCst);
                                TANK_ANSWER.store(tank, Ordering::SeqCst);
                                for unit in twins.both() {
                                    save_to_memory(unit.0 + unit_offsets::MBR_0X142, locked);
                                    save_to_memory(unit.0 + unit_offsets::MOVE_ANIM_ENABLED, animating);
                                    save_to_memory(unit.0 + 0x143, 0u8);
                                    save_to_memory(unit.0 + unit_offsets::ENTITY_TYPE, if mermaid { fake_type } else { unit_type });
                                }
                                let label = format!("type {type_index} added {added} show {show} tank {tank} locked {locked} animating {animating} mermaid {mermaid}");
                                take_log();
                                let real_result = real::add_to_map(twins.real.0);
                                let real_calls = take_log();
                                let port_result = twins.port.add_to_map();
                                let port_calls = take_log();
                                if port_calls.iter().any(|call| call == "runBehaviorSet") {
                                    mermaid_runs += 1;
                                }
                                check(&mut failures, &format!("{label} result"), real_result, port_result);
                                compare_log(&label, &real_calls, &port_calls, &mut failures);
                                compare_memory(&label, twins.real, twins.port, &mut failures);
                            }
                        }
                    }
                }
            }
        }
        for unit in twins.both() {
            save_to_memory(unit.0 + unit_offsets::ENTITY_TYPE, unit_type);
        }
        twins.destroy();
    }
    STUB_ADD_TO_MAP.store(0, Ordering::SeqCst);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = None;
    tracing::info!("{test_name}: the mermaid transform ran in {mermaid_runs} port cases");
    finish_test(test_name, failures, failure_log)
}

/// The text before a trailing run of digits, and the number it ends in.
fn split_number(name: &str) -> (&str, Option<u32>) {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        (name, None)
    } else {
        (&name[..name.len() - digits], name[name.len() - digits..].parse().ok())
    }
}

/// `ZTUNIT_CREATE_NAME_LIVE`: `createName` against the real body with the virtual `setName` recorded, for each live
/// type as it is and with its numbered-names byte set. Numbered names draw the type's next unit count, so the port's
/// number is the real one plus one.
pub(crate) fn run_ztunit_create_name_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_CREATE_NAME_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    for (type_index, &unit_type) in types.iter().enumerate() {
        let original: u8 = get_from_memory(unit_type + 0x5b);
        for numbered in [0u8, 1] {
            save_to_memory(unit_type + 0x5b, numbered);
            let twins = Twins::new(unit_type, false);
            for unit in twins.both() {
                script_unit_for_add_to_map(unit);
            }
            let label = format!("type {type_index} numbered {numbered}");
            take_log();
            real::create_name(twins.real.0);
            let real_calls = take_log();
            twins.port.create_name();
            let port_calls = take_log();
            if numbered == 0 {
                compare_log(&label, &real_calls, &port_calls, &mut failures);
            } else {
                match (real_calls.as_slice(), port_calls.as_slice()) {
                    ([real], [port]) => {
                        let (real_text, real_number) = split_number(real);
                        let (port_text, port_number) = split_number(port);
                        if real_text != port_text || real_number.zip(port_number).is_none_or(|(r, p)| p != r + 1) {
                            failures.push(format!("{label}: real {real:?}, port {port:?} (expected the same text and a number one higher)"));
                        }
                    }
                    _ => failures.push(format!("{label}: real calls {real_calls:?}, port calls {port_calls:?}")),
                }
            }
            compare_memory(&label, twins.real, twins.port, &mut failures);
            twins.destroy();
        }
        save_to_memory(unit_type + 0x5b, original);
    }
    finish_test(test_name, failures, failure_log)
}

// ---- initAfterLoad ----------------------------------------------------------------------------------------------

static IAL_VALIDATE_ANSWER: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn ial_validate_building(_this: u32, confirmed: u32, flag: u32) -> u32 {
    log(format!("validateBuilding {} {}", get_from_memory::<u8>(confirmed), flag & 0xff));
    IAL_VALIDATE_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn ial_validate_tank_position(_this: u32) {
    log("validateTankPosition".to_string());
}
extern "thiscall" fn ial_set_visible(_this: u32, visible: u32) {
    log(format!("setVisible {}", visible & 0xff));
}

fn script_unit_for_init_after_load(unit: ZTUnit) {
    let real_vtable: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    let scripted: [(u32, u32); 5] = [
        (0xcc, ial_set_visible as *const () as u32),
        (0x138, fake_reset_move_anim as *const () as u32),
        (0x198, fake_stop_everything as *const () as u32),
        (0x2d4, ial_validate_tank_position as *const () as u32),
        (0x2e8, ial_validate_building as *const () as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    save_to_memory(unit.0, leaked_words(vtable));
}

/// A zeroed block (`0x1f8` bytes: forty-two empty `vector<GXMixerLink>`) from vanilla's allocator.
fn vanilla_block() -> u32 {
    let block = unsafe { openzt_detour::generated::standalone::OPERATOR_NEW.original()(0x1f8) } as u32;
    unsafe { std::ptr::write_bytes(block as *mut u8, 0, 0x1f8) };
    block
}

/// Builds the mixer queue `initAfterLoad` empties: `split` gives it two distinct block tables (so the front-block
/// ranges are destroyed and the second block freed), otherwise both tables coincide. Returns the blocks the body
/// does not free, for the caller to release.
fn build_mixer_queue(unit: ZTUnit, split: bool) -> Vec<u32> {
    let first_block = vanilla_block();
    let table = leaked_words(vec![vanilla_block(), vanilla_block(), vanilla_block(), vanilla_block()]);
    let mut kept = vec![first_block];
    save_to_memory(unit.0 + 0x8c, first_block);
    save_to_memory(unit.0 + 0x90, first_block + 0x18);
    save_to_memory(unit.0 + 0x94, first_block + 0x24);
    if split {
        let second_block = vanilla_block();
        save_to_memory(unit.0 + 0x98, table);
        save_to_memory(unit.0 + 0xa8, table + 16);
        save_to_memory(unit.0 + 0x9c, second_block + 0x24);
        save_to_memory(unit.0 + 0xa0, second_block);
        save_to_memory(unit.0 + 0xa4, second_block + 0x30);
    } else {
        // The table's first entry is not visited: the walk starts one entry in and stops at the table's end.
        save_to_memory(unit.0 + 0x98, table);
        save_to_memory(unit.0 + 0xa8, table);
        save_to_memory(unit.0 + 0x9c, first_block + 0x30);
        save_to_memory(unit.0 + 0xa0, first_block + 0x18);
        save_to_memory(unit.0 + 0xa4, first_block + 0x24);
    }
    // The table entries the walk frees are the body's; the first one never is, and a table whose ends coincide is not walked.
    if split {
        kept.push(get_from_memory(table));
    } else {
        kept.extend((0..4u32).map(|entry| get_from_memory::<u32>(table + entry * 4)));
    }
    kept
}

/// `ZTUNIT_INIT_AFTER_LOAD_LIVE`: `initAfterLoad` against the real body with the unit's virtual slots scripted, over the
/// goal ids around the building-use range and both block-table shapes, comparing memory and calls.
pub(crate) fn run_ztunit_init_after_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_INIT_AFTER_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(2);
    if types.is_empty() {
        return finish_test(test_name, vec!["the live world holds no animal type".to_string()], failure_log);
    }
    let mut emptied = 0;
    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, false);
        let saved: Vec<(ZTUnit, Vec<u32>)> = twins.both().into_iter().map(|unit| (unit, (0x8c..=0xa8u32).step_by(4).map(|offset| get_from_memory(unit.0 + offset)).collect())).collect();
        for unit in twins.both() {
            script_unit_for_init_after_load(unit);
        }
        for goal in [0x10u32, 0x67, 0x68, 0x69, 0x6a, 0x6b] {
            for answer in [0u32, 1, 0x100] {
                for animating in [0u8, 1] {
                    for split in [false, true] {
                        IAL_VALIDATE_ANSWER.store(answer, Ordering::SeqCst);
                        let mut kept = Vec::new();
                        for unit in twins.both() {
                            save_to_memory(unit.0 + unit_offsets::GOAL_ID, goal);
                            save_to_memory(unit.0 + unit_offsets::MOVE_ANIM_ENABLED, animating);
                            kept.extend(build_mixer_queue(unit, split));
                        }
                        let label = format!("type {type_index} goal {goal:#x} validate {answer:#x} animating {animating} split {split}");
                        take_log();
                        let real_result = real::init_after_load(twins.real.0, 0x65);
                        let real_calls = take_log();
                        let port_result = twins.port.init_after_load(0x65);
                        let port_calls = take_log();
                        if real_calls.iter().any(|call| call.starts_with("setVisible")) {
                            emptied += 1;
                        }
                        check(&mut failures, &format!("{label} result"), real_result, port_result);
                        compare_log(&label, &real_calls, &port_calls, &mut failures);
                        compare_memory(&label, twins.real, twins.port, &mut failures);
                        // The blocks the body leaves are the first block of each twin and the table's first entry.
                        for block in kept {
                            unsafe { openzt_detour::generated::standalone::OPERATOR_DELETE.original()(block) };
                        }
                    }
                }
            }
        }
        // The hooked address reaches the port.
        IAL_VALIDATE_ANSWER.store(1, Ordering::SeqCst);
        save_to_memory(twins.port.0 + unit_offsets::GOAL_ID, 0x69u32);
        take_log();
        let hooked = unsafe { generated::INIT_AFTER_LOAD.hooked()(twins.port.0 as *const u32, 0x65) };
        let hooked_calls = take_log();
        check(&mut failures, "hooked initAfterLoad result", hooked, true);
        if !hooked_calls.iter().any(|call| call.starts_with("validateBuilding")) {
            failures.push(format!("hooked initAfterLoad made no scripted calls: {hooked_calls:?}"));
        }
        // Put the unit's own queue back for the destructor to walk.
        for (unit, words) in saved {
            for (index, word) in words.into_iter().enumerate() {
                save_to_memory(unit.0 + 0x8c + index as u32 * 4, word);
            }
        }
        twins.destroy();
    }
    tracing::info!("{test_name}: the queue was emptied in {emptied} real cases");
    finish_test(test_name, failures, failure_log)
}
