//! Live checks for `ztkeeper.rs`. Each comparison builds a keeper with the real vanilla constructor and one with
//! the Rust one over the same real keeper type, runs the real body on one and the port on the other, and compares
//! return values and the keepers' whole memory (heap pointers may differ, shared pointers may not).

use std::sync::atomic::{AtomicU32, Ordering};

use openzt_detour::generated::{
    standalone::OPERATOR_DELETE,
    ztunit::UPDATE_GOALS as ZTUNIT_UPDATE_GOALS,
    ztkeeper::{CLEANS_UP, DESTRUCTOR_0, F_ESCAPED_ANIMAL_ROUTINE, F_HABITAT_ROUTINE, F_ROUTINE, GET_TERRAIN_COST, IN_TARGET_HABITAT, REMOVE_FROM_MAP, SET_SPEED},
};

use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::{harness::{finish_test, write_success_line}, io_redirect},
    util::{get_from_memory, save_to_memory},
    ztmegatilemgr::entity_type_matches,
    ztkeeper::{
        live_support::{self, real},
        unit_offsets, ZTKeeper, KEEPER_SIZE, KEEPER_TYPE_TAG_RVA, KEEPER_VTABLE_RVA, OWN_DWORD_OFFSETS, PATHING_TO_HABITAT_GATE_OFFSET,
    },
};

use openzt_detour::generated::{ztfence::CLOSE_GATE as ARMS_CLOSE_GATE, ztshow::ABORT_SHOW as ARMS_ABORT_SHOW, ztstaff::DO_CONDITIONALS as ARMS_STAFF_DO_CONDITIONALS};

const SPEED_CHEAT_FLAG_RVA: u32 = 0x0023_c2e4;
const VTABLE_SLOTS: u32 = 204;
/// The game's `rand` state (`DAT_00638060`), advanced by `ZTUnit::resetAI`.
const RNG_STATE_RVA: u32 = 0x0023_8060;
const UNIT_POS_X: u32 = 0x114;
const UNIT_POS_Y: u32 = 0x118;
const UNIT_POS_Z: u32 = 0x11c;

/// `ZTKEEPER_DETOURS_ENABLED`: every `ZTKeeper` detour reports enabled, so the comparisons below exercise the hooked
/// path rather than silently running vanilla.
pub(crate) fn run_ztkeeper_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

/// `ZTKEEPER_FIXTURE_LAYOUT_LIVE`: a keeper built by the vanilla constructor over a real keeper type has the
/// `ZTKeeper` vtable and a clear `+0x2a8` byte, the typed accessors read and write the offsets they name, and the
/// real `initStatusVars` zeroes exactly the five own dwords and leaves the `+0x2a8` byte alone. The last check is
/// what ties the named offsets to vanilla's own writes.
pub(crate) fn run_ztkeeper_fixture_layout_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_FIXTURE_LAYOUT_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let keeper = live_support::build_standalone_keeper(keeper_type);

    let vtable: u32 = get_from_memory(keeper.0);
    let expected_vtable = get_module_base("zoo.exe") as u32 + KEEPER_VTABLE_RVA;
    if vtable != expected_vtable {
        failures.push(format!("vtable {vtable:#x}, expected {expected_vtable:#x}"));
    }
    if keeper.pathing_to_habitat_gate() != 0 {
        failures.push("the constructor left the +0x2a8 byte set".to_string());
    }

    keeper.set_food_to_leave(11);
    keeper.set_sick_animal_count(22);
    keeper.set_dirt_tile_count(33);
    keeper.set_habitat_select_disabled(44);
    keeper.set_food_type_to_leave(55);
    keeper.set_pathing_to_habitat_gate(0xa5);
    let written = [11u32, 22, 33, 44, 55];
    for (offset, want) in OWN_DWORD_OFFSETS.iter().zip(written) {
        let got: u32 = get_from_memory(keeper.0 + offset);
        if got != want {
            failures.push(format!("accessor for +{offset:#x} wrote {got}, expected {want}"));
        }
    }
    let gate_byte: u8 = get_from_memory(keeper.0 + PATHING_TO_HABITAT_GATE_OFFSET);
    if gate_byte != 0xa5 {
        failures.push(format!("the +0x2a8 accessor wrote {gate_byte:#x}, expected 0xa5"));
    }

    real::init_status_vars(keeper.0);

    for offset in OWN_DWORD_OFFSETS {
        let got: u32 = get_from_memory(keeper.0 + offset);
        if got != 0 {
            failures.push(format!("real initStatusVars left +{offset:#x} at {got}, expected 0"));
        }
    }
    if keeper.pathing_to_habitat_gate() != 0xa5 {
        failures.push("real initStatusVars changed the +0x2a8 byte".to_string());
    }

    live_support::destroy_standalone_keeper(ZTKeeper(keeper.0));
    finish_test(test_name, failures, failure_log)
}

fn words(address: u32) -> Vec<u32> {
    (0..KEEPER_SIZE / 4).map(|i| get_from_memory::<u32>(address + i * 4)).collect()
}

/// Whether two words agree: equal, or both heap pointers neither of which is a shared pointer.
fn equivalent(real: u32, port: u32, shared: &[u32]) -> bool {
    real == port || (real >= 0x10000 && port >= 0x10000 && !shared.contains(&real) && !shared.contains(&port))
}

fn compare_memory(label: &str, real: ZTKeeper, port: ZTKeeper, shared: &[u32], failures: &mut Vec<String>) {
    for (i, (r, p)) in words(real.0).into_iter().zip(words(port.0)).enumerate() {
        if !equivalent(r, p, shared) {
            failures.push(format!("{label}: +{:#x} real {r:#x}, port {p:#x}", i * 4));
        }
    }
}

fn free(keeper: ZTKeeper) {
    unsafe { OPERATOR_DELETE.original()(keeper.0) };
}

/// `ZTKEEPER_LIFECYCLE_LIVE`: the Rust constructor and non-deleting destructor leave the same memory as the real
/// ones, and the hooked scalar-deleting destructor frees a keeper with and without the delete bit.
pub(crate) fn run_ztkeeper_lifecycle_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_LIFECYCLE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };

    let real_keeper = live_support::build_standalone_keeper(keeper_type);
    let port_keeper = live_support::build_standalone_keeper_port(keeper_type);
    compare_memory("constructor", real_keeper, port_keeper, &[keeper_type], &mut failures);

    let real_result = real::destructor(real_keeper.0);
    let port_result = port_keeper.destruct();
    if real_result as u32 != real_keeper.0 || port_result as u32 != port_keeper.0 {
        failures.push(format!(
            "destructor returned real {:#x} (keeper {:#x}), port {:#x} (keeper {:#x})",
            real_result as u32, real_keeper.0, port_result as u32, port_keeper.0
        ));
    }
    compare_memory("destructor", real_keeper, port_keeper, &[keeper_type], &mut failures);
    free(real_keeper);
    free(port_keeper);

    let kept = live_support::build_standalone_keeper_port(keeper_type);
    let kept_result = unsafe { DESTRUCTOR_0.hooked()(kept.0 as *const u32, 0) };
    if kept_result as u32 != kept.0 {
        failures.push(format!("hooked destructor without the delete bit returned {:#x}, expected {:#x}", kept_result as u32, kept.0));
    }
    free(kept);

    let deleted = live_support::build_standalone_keeper(keeper_type);
    let deleted_result = unsafe { DESTRUCTOR_0.hooked()(deleted.0 as *const u32, 1) };
    if deleted_result as u32 != deleted.0 {
        failures.push(format!("hooked destructor with the delete bit returned {:#x}, expected {:#x}", deleted_result as u32, deleted.0));
    }

    finish_test(test_name, failures, failure_log)
}

static FAKE_TYPE_ID: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_type_id(_this: u32) -> u32 {
    FAKE_TYPE_ID.load(Ordering::SeqCst)
}

/// `ZTKEEPER_SET_SPEED_LIVE`: `setSpeed` with the cheat flag clear and set, for the boosted type id and another.
/// The keeper's type pointer is swapped for a fake type whose id getter returns the case's id.
pub(crate) fn run_ztkeeper_set_speed_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_SET_SPEED_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };

    let mut fake_vtable = vec![0u32; 0x40];
    fake_vtable[(0x20 / 4) as usize] = fake_type_id as *const () as usize as u32;
    let fake_type = [fake_vtable.as_ptr() as u32, 0u32];

    let real_keeper = live_support::build_standalone_keeper(keeper_type);
    let port_keeper = live_support::build_standalone_keeper_port(keeper_type);
    let cheat_address = get_module_base("zoo.exe") as u32 + SPEED_CHEAT_FLAG_RVA;
    let original_cheat: u8 = get_from_memory(cheat_address);

    for (cheat, type_id, speed, expected) in [(0u8, 0x254f_u32, 7u32, 7u32), (1, 0x254f, 7, 14), (1, 0x1, 7, 7), (0, 0x1, 9, 9), (1, 0x254f, 0x9000_0000, 0x2000_0000)] {
        save_to_memory(cheat_address, cheat);
        FAKE_TYPE_ID.store(type_id, Ordering::SeqCst);
        save_to_memory(real_keeper.0 + unit_offsets::ENTITY_TYPE, fake_type.as_ptr() as u32);
        save_to_memory(port_keeper.0 + unit_offsets::ENTITY_TYPE, fake_type.as_ptr() as u32);
        real::set_speed(real_keeper.0, speed);
        port_keeper.set_speed(speed);
        let (r, p): (u32, u32) = (get_from_memory(real_keeper.0 + unit_offsets::SPEED), get_from_memory(port_keeper.0 + unit_offsets::SPEED));
        if r != p || r != expected {
            failures.push(format!("cheat {cheat} id {type_id:#x} speed {speed:#x}: real {r:#x}, port {p:#x}, expected {expected:#x}"));
        }
        unsafe { SET_SPEED.hooked()(port_keeper.0 as *const u32, speed) };
        let hooked: u32 = get_from_memory(port_keeper.0 + unit_offsets::SPEED);
        if hooked != expected {
            failures.push(format!("hooked setSpeed cheat {cheat} id {type_id:#x}: {hooked:#x}, expected {expected:#x}"));
        }
    }

    save_to_memory(cheat_address, original_cheat);
    save_to_memory(real_keeper.0 + unit_offsets::ENTITY_TYPE, keeper_type);
    save_to_memory(port_keeper.0 + unit_offsets::ENTITY_TYPE, keeper_type);
    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);
    finish_test(test_name, failures, failure_log)
}

static EVENT_CALLS: [AtomicU32; 5] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

extern "thiscall" fn recording_get_events(this: u32, out: u32, id: u32, kind: u32) {
    EVENT_CALLS[0].fetch_add(1, Ordering::SeqCst);
    EVENT_CALLS[1].store(this, Ordering::SeqCst);
    EVENT_CALLS[2].store(out, Ordering::SeqCst);
    EVENT_CALLS[3].store(id, Ordering::SeqCst);
    EVENT_CALLS[4].store(kind, Ordering::SeqCst);
}

fn recorded_event_call() -> [u32; 5] {
    [0, 1, 2, 3, 4].map(|i| EVENT_CALLS[i].load(Ordering::SeqCst))
}

/// `ZTKEEPER_GET_EVENTS_LIVE`: the `+0x8` `getEvents` dispatches through the keeper's own vtable `+0xc` with
/// `(out, entity id, 0x4b)` (checked against a recording vtable, real and port), and the `+0xc` one forwards its
/// three arguments to `ZTAIMgr`'s listener (the events appended for an unknown id agree between real and port).
pub(crate) fn run_ztkeeper_get_events_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_GET_EVENTS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };

    let real_keeper = live_support::build_standalone_keeper(keeper_type);
    let port_keeper = live_support::build_standalone_keeper_port(keeper_type);
    let real_vtable: u32 = get_from_memory(real_keeper.0);
    let mut fake_vtable: Vec<u32> = (0..VTABLE_SLOTS).map(|i| get_from_memory::<u32>(real_vtable + i * 4)).collect();
    fake_vtable[3] = recording_get_events as *const () as usize as u32;
    for keeper in [real_keeper, port_keeper] {
        save_to_memory(keeper.0 + unit_offsets::ENTITY_ID, 0x55u32);
        save_to_memory(keeper.0, fake_vtable.as_ptr() as u32);
    }

    EVENT_CALLS[0].store(0, Ordering::SeqCst);
    real::get_events(real_keeper.0, 0x1234);
    let real_call = recorded_event_call();
    EVENT_CALLS[0].store(0, Ordering::SeqCst);
    port_keeper.get_events(0x1234);
    let port_call = recorded_event_call();
    if real_call[0] != 1 || real_call[2..] != [0x1234, 0x55, 0x4b] || real_call[1] != real_keeper.0 {
        failures.push(format!("real getEvents call record {real_call:x?}"));
    }
    if port_call[0] != 1 || port_call[2..] != [0x1234, 0x55, 0x4b] || port_call[1] != port_keeper.0 {
        failures.push(format!("port getEvents call record {port_call:x?}"));
    }

    for keeper in [real_keeper, port_keeper] {
        save_to_memory(keeper.0, real_vtable);
    }

    if globals().ztaimgr_ptr().is_null() {
        failures.push("GLOBAL_ZTAIMgr is null in the live zoo".to_string());
    } else {
        let real_out = [0u32; 4];
        let port_out = [0u32; 4];
        real::get_events_from_aimgr(real_out.as_ptr() as u32, 0x7fff_0001, 0x4b);
        ZTKeeper::get_events_from_aimgr(port_out.as_ptr() as u32, 0x7fff_0001, 0x4b);
        if real_out[2].wrapping_sub(real_out[1]) != port_out[2].wrapping_sub(port_out[1]) {
            failures.push(format!("listener forwarding: real out {real_out:x?}, port out {port_out:x?}"));
        }
    }

    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);
    finish_test(test_name, failures, failure_log)
}

fn place_at_tile(keeper: ZTKeeper, tile: (u32, u32)) {
    save_to_memory(keeper.0 + UNIT_POS_X, tile.0 * 64 + 32);
    save_to_memory(keeper.0 + UNIT_POS_Y, tile.1 * 64 + 32);
    save_to_memory(keeper.0 + UNIT_POS_Z, 0u32);
}

/// `ZTKEEPER_BEHAVIOUR_LIVE`: `setTargetTile` (same tile, then a changed one), `resetAI` and `initAfterLoad` (a keeper
/// on a tile with no habitat, with the show goal state clear and set; its result byte is compared too) against the real
/// bodies on twin fixtures. `resetAI` draws a random number, so the generator state is restored between the two poles.
pub(crate) fn run_ztkeeper_behaviour_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_BEHAVIOUR_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let tile_a = world.get_tile_ptr(10, 10);
    let tile_b = world.get_tile_ptr(11, 10);
    let shared = [keeper_type, tile_a, tile_b];

    let new_pair = || (live_support::build_standalone_keeper(keeper_type), live_support::build_standalone_keeper_port(keeper_type));

    let (real_keeper, port_keeper) = new_pair();
    for keeper in [real_keeper, port_keeper] {
        place_at_tile(keeper, (10, 10));
        save_to_memory(keeper.0 + unit_offsets::TARGET_TILE, tile_a);
    }
    real::set_target_tile(real_keeper.0, tile_a, false);
    port_keeper.set_target_tile(tile_a, false);
    compare_memory("setTargetTile (same tile)", real_keeper, port_keeper, &shared, &mut failures);
    real::set_target_tile(real_keeper.0, tile_b, true);
    port_keeper.set_target_tile(tile_b, true);
    compare_memory("setTargetTile (changed tile)", real_keeper, port_keeper, &shared, &mut failures);
    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);

    let (real_keeper, port_keeper) = new_pair();
    for keeper in [real_keeper, port_keeper] {
        place_at_tile(keeper, (10, 10));
    }
    let rng_address = get_module_base("zoo.exe") as u32 + RNG_STATE_RVA;
    let rng_seed: u32 = get_from_memory(rng_address);
    real::reset_ai(real_keeper.0);
    save_to_memory(rng_address, rng_seed);
    port_keeper.reset_ai();
    compare_memory("resetAI", real_keeper, port_keeper, &shared, &mut failures);
    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);

    for goal_state in [0u32, 0x13] {
        let (real_keeper, port_keeper) = new_pair();
        for keeper in [real_keeper, port_keeper] {
            place_at_tile(keeper, (10, 10));
            save_to_memory(keeper.0 + unit_offsets::COG_GOAL_STATE, goal_state);
        }
        let habitat: u32 = {
            let vtable: u32 = get_from_memory(port_keeper.0);
            let get_habitat = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(vtable + 0x24c)) };
            get_habitat(port_keeper.0)
        };
        if habitat != 0 {
            live_support::destroy_standalone_keeper(real_keeper);
            live_support::destroy_standalone_keeper(port_keeper);
            failures.push("tile (10, 10) is inside a habitat; initAfterLoad would register with a live show".to_string());
            continue;
        }
        let real_result = live_support::real::init_after_load(real_keeper.0, 1);
        let port_result = port_keeper.init_after_load(1);
        if real_result != port_result {
            failures.push(format!("initAfterLoad (goal state {goal_state:#x}): real returned {real_result}, port {port_result}"));
        }
        compare_memory(&format!("initAfterLoad (goal state {goal_state:#x})"), real_keeper, port_keeper, &shared, &mut failures);
        live_support::destroy_standalone_keeper(real_keeper);
        live_support::destroy_standalone_keeper(port_keeper);
    }

    finish_test(test_name, failures, failure_log)
}

fn call_vtable_slot_1(this: u32, offset: u32, arg: u32) -> u32 {
    let vtable: u32 = get_from_memory(this);
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(get_from_memory::<u32>(vtable + offset)) };
    function(this, arg)
}

/// A tile that belongs to a habitat, and a tile that does not, as seen through `keeper`'s `getHabitatFromTile`.
fn habitat_and_plain_tiles(keeper: ZTKeeper) -> (Option<(u32, u32)>, Option<(u32, u32)>) {
    let world = globals().ztworldmgr();
    let (mut habitat, mut plain) = (None, None);
    for y in 0..world.map_y_size.min(200) {
        for x in 0..world.map_x_size.min(200) {
            let in_habitat = call_vtable_slot_1(keeper.0, 0x248, world.get_tile_ptr(x, y)) != 0;
            if in_habitat && habitat.is_none() {
                habitat = Some((x, y));
            } else if !in_habitat && plain.is_none() {
                plain = Some((x, y));
            }
            if habitat.is_some() && plain.is_some() {
                return (habitat, plain);
            }
        }
    }
    (habitat, plain)
}

fn entity_type_ptrs() -> Vec<u32> {
    let world = globals().ztworldmgr();
    (world.entity_type_array_start..world.entity_type_array_end).step_by(4).map(get_from_memory::<u32>).filter(|&ty| ty != 0).collect()
}

fn is_keeper_type(ty: u32) -> bool {
    let vtable: u32 = get_from_memory(ty);
    let is_cast_class = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
    is_cast_class(ty, get_module_base("zoo.exe") as u32 + KEEPER_TYPE_TAG_RVA)
}

/// `ZTKEEPER_CLEANS_UP_LIVE`: `cleansUp` over every entity type the zoo holds (a bare entity whose type pointer is
/// set), a null entity, an entity with no type, and a keeper whose own type is not a keeper type.
pub(crate) fn run_ztkeeper_cleans_up_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_CLEANS_UP_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let keeper = live_support::build_standalone_keeper(keeper_type);
    let mut entity = vec![0u32; 0x80];

    let types = entity_type_ptrs();
    let mut true_results = 0;
    for ty in &types {
        entity[(unit_offsets::ENTITY_TYPE / 4) as usize] = *ty;
        let target = entity.as_ptr() as u32;
        let real_result = real::cleans_up(keeper.0, target);
        let port_result = keeper.cleans_up(target);
        let hooked = unsafe { CLEANS_UP.hooked()(keeper.0 as *const u32, target as *const u32) };
        if real_result != port_result || real_result != hooked {
            failures.push(format!("type {ty:#x}: real {real_result}, port {port_result}, hooked {hooked}"));
        }
        true_results += real_result as u32;
    }
    if true_results == 0 {
        failures.push("no entity type was cleaned up, so the true path was not exercised".to_string());
    }

    entity[(unit_offsets::ENTITY_TYPE / 4) as usize] = 0;
    for (label, target) in [("null entity", 0u32), ("entity without a type", entity.as_ptr() as u32)] {
        let (real_result, port_result) = (real::cleans_up(keeper.0, target), keeper.cleans_up(target));
        if real_result != port_result {
            failures.push(format!("{label}: real {real_result}, port {port_result}"));
        }
    }

    if let Some(other_type) = types.iter().copied().find(|&ty| !is_keeper_type(ty)) {
        save_to_memory(keeper.0 + unit_offsets::ENTITY_TYPE, other_type);
        entity[(unit_offsets::ENTITY_TYPE / 4) as usize] = keeper_type;
        let target = entity.as_ptr() as u32;
        let (real_result, port_result) = (real::cleans_up(keeper.0, target), keeper.cleans_up(target));
        if real_result || port_result {
            failures.push(format!("non-keeper type: real {real_result}, port {port_result}, expected false"));
        }
        save_to_memory(keeper.0 + unit_offsets::ENTITY_TYPE, keeper_type);
    }

    live_support::destroy_standalone_keeper(keeper);
    finish_test(test_name, failures, failure_log)
}

/// `ZTKEEPER_IN_TARGET_HABITAT_LIVE`: a keeper standing in a habitat or on plain ground, targeting a tile in a
/// habitat or on plain ground.
pub(crate) fn run_ztkeeper_in_target_habitat_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_IN_TARGET_HABITAT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let keeper = live_support::build_standalone_keeper(keeper_type);
    let (habitat, plain) = habitat_and_plain_tiles(keeper);
    let (Some(habitat), Some(plain)) = (habitat, plain) else {
        failures.push(format!("need a habitat tile and a plain tile, found {habitat:?} and {plain:?}"));
        live_support::destroy_standalone_keeper(keeper);
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let mut results = Vec::new();
    for (position, target) in [(habitat, habitat), (plain, habitat), (habitat, plain), (plain, plain)] {
        place_at_tile(keeper, position);
        save_to_memory(keeper.0 + unit_offsets::TARGET_TILE, world.get_tile_ptr(target.0, target.1));
        let real_result = real::in_target_habitat(keeper.0);
        let port_result = keeper.in_target_habitat();
        let hooked = unsafe { IN_TARGET_HABITAT.hooked()(keeper.0 as *const std::ffi::c_void) };
        if real_result != port_result || real_result != hooked {
            failures.push(format!("at {position:?} targeting {target:?}: real {real_result}, port {port_result}, hooked {hooked}"));
        }
        results.push(real_result);
    }
    if !results.contains(&true) || !results.contains(&false) {
        failures.push(format!("the cases did not produce both outcomes: {results:?}"));
    }
    live_support::destroy_standalone_keeper(keeper);
    finish_test(test_name, failures, failure_log)
}

fn assigned_payloads(keeper: u32) -> Vec<u32> {
    let sentinel: u32 = get_from_memory(keeper + unit_offsets::ASSIGNED_HABITATS);
    let mut payloads = Vec::new();
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel && payloads.len() < 1000 {
        payloads.push(get_from_memory(node + 8));
        node = get_from_memory(node);
    }
    payloads
}

fn live_keepers() -> Vec<u32> {
    globals().ztworldmgr().entity_array().filter(|&ptr| unsafe { entity_type_matches(ptr, KEEPER_TYPE_TAG_RVA) }).take(8).collect()
}

/// `ZTKEEPER_HABITAT_QUERIES_LIVE`: `getRandomAssignedHabitat` and `checkAssignedHabitats` against the real bodies, on a
/// fixture with no assignments (the all-habitats branch), with the `+0x2a0` gate set, and on the zoo's own keepers
/// (their assigned lists). The game RNG is pinned before each pole and must end in the same state. The live keepers'
/// lists must be unchanged by `checkAssignedHabitats`, which is a no-op for a healthy zoo.
pub(crate) fn run_ztkeeper_habitat_queries_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_HABITAT_QUERIES_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let rng_address = get_module_base("zoo.exe") as u32 + RNG_STATE_RVA;
    let original_rng: u32 = get_from_memory(rng_address);

    let compare_random = |label: &str, keeper: ZTKeeper, seed: u32, failures: &mut Vec<String>| {
        save_to_memory(rng_address, seed);
        let real_result = real::get_random_assigned_habitat(keeper.0);
        let real_rng: u32 = get_from_memory(rng_address);
        save_to_memory(rng_address, seed);
        let port_result = keeper.get_random_assigned_habitat();
        let port_rng: u32 = get_from_memory(rng_address);
        if real_result != port_result || real_rng != port_rng {
            failures.push(format!("{label} seed {seed:#x}: real {real_result:#x} (rng {real_rng:#x}), port {port_result:#x} (rng {port_rng:#x})"));
        }
    };

    let fixture = live_support::build_standalone_keeper(keeper_type);
    for seed in [1u32, 0x1234_5678, 0xdead_beef] {
        compare_random("fixture, all habitats", fixture, seed, &mut failures);
    }
    fixture.set_habitat_select_disabled(1);
    compare_random("fixture, gate set", fixture, 7, &mut failures);
    save_to_memory(rng_address, 7u32);
    if fixture.get_random_assigned_habitat() != 0 || get_from_memory::<u32>(rng_address) != 7 {
        failures.push("fixture with the gate set returned a habitat or advanced the RNG".to_string());
    }
    fixture.set_habitat_select_disabled(0);
    real::check_assigned_habitats(fixture.0);
    fixture.check_assigned_habitats();
    if !assigned_payloads(fixture.0).is_empty() {
        failures.push("checkAssignedHabitats gave the fixture an assignment".to_string());
    }
    live_support::destroy_standalone_keeper(fixture);

    let keepers = live_keepers();
    if keepers.is_empty() {
        failures.push("the live zoo holds no keeper".to_string());
    }
    for keeper in keepers.iter().map(|&ptr| ZTKeeper(ptr)) {
        let before = assigned_payloads(keeper.0);
        for seed in [3u32, 0x0bad_f00d] {
            compare_random(&format!("keeper {:#x} ({} assigned)", keeper.0, before.len()), keeper, seed, &mut failures);
        }
        real::check_assigned_habitats(keeper.0);
        let after_real = assigned_payloads(keeper.0);
        keeper.check_assigned_habitats();
        let after_port = assigned_payloads(keeper.0);
        if after_real != before || after_port != before {
            failures.push(format!("keeper {:#x}: assigned habitats {before:x?}, after real {after_real:x?}, after port {after_port:x?}", keeper.0));
        }
    }
    save_to_memory(rng_address, original_rng);
    finish_test(test_name, failures, failure_log)
}

/// `ZTKEEPER_DROP_LIVE`: `drop` on twin fixtures standing on a habitat tile and on plain ground, comparing the whole
/// keeper afterwards. (`doConditionals` is not run here: `ZTStaff::doConditionals` dereferences state only a running
/// game provides and crashes in vanilla on a bare fixture and on a keeper outside the game loop.)
pub(crate) fn run_ztkeeper_drop_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_DROP_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let probe = live_support::build_standalone_keeper(keeper_type);
    let (habitat, plain) = habitat_and_plain_tiles(probe);
    live_support::destroy_standalone_keeper(probe);
    let (Some(habitat), Some(plain)) = (habitat, plain) else {
        failures.push(format!("need a habitat tile and a plain tile, found {habitat:?} and {plain:?}"));
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let shared = [keeper_type, world.get_tile_ptr(habitat.0, habitat.1), world.get_tile_ptr(plain.0, plain.1)];

    for (label, tile) in [("habitat tile", habitat), ("plain tile", plain)] {
        let real_keeper = live_support::build_standalone_keeper(keeper_type);
        let port_keeper = live_support::build_standalone_keeper_port(keeper_type);
        for keeper in [real_keeper, port_keeper] {
            place_at_tile(keeper, tile);
        }
        real::drop_keeper(real_keeper.0);
        port_keeper.drop_keeper();
        compare_memory(&format!("drop on a {label}"), real_keeper, port_keeper, &shared, &mut failures);
        live_support::destroy_standalone_keeper(real_keeper);
        live_support::destroy_standalone_keeper(port_keeper);
    }
    finish_test(test_name, failures, failure_log)
}

/// Tiles where a keeper's `getHabitat` returns nothing and a real habitat.
struct RoutineTiles {
    none: Option<(u32, u32)>,
    habitat: Option<(u32, u32)>,
}

fn routine_tiles(keeper: ZTKeeper) -> RoutineTiles {
    let world = globals().ztworldmgr();
    let mut tiles = RoutineTiles { none: None, habitat: None };
    for y in 0..world.map_y_size.min(200) {
        for x in 0..world.map_x_size.min(200) {
            place_at_tile(keeper, (x, y));
            let habitat = call_vtable_slot_0(keeper.0, 0x24c);
            let slot = if habitat == 0 { &mut tiles.none } else { &mut tiles.habitat };
            slot.get_or_insert((x, y));
            if tiles.none.is_some() && tiles.habitat.is_some() {
                return tiles;
            }
        }
    }
    tiles
}

fn call_vtable_slot_0(this: u32, offset: u32) -> u32 {
    let vtable: u32 = get_from_memory(this);
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(vtable + offset)) };
    function(this)
}

static FAKE_HABITAT: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_get_habitat(_this: u32) -> u32 {
    FAKE_HABITAT.load(Ordering::SeqCst)
}

/// `ZTKEEPER_F_ROUTINE_LIVE`: `fRoutine` on twin fixtures over every combination of tile kind (no habitat, a habitat,
/// a world habitat), goal id (habitat duties, escaped animal, another), goal flag and stopped flag, comparing the
/// result, the `done` byte and the whole keeper (`BFUnit::idle` runs on the fall-through cases). Also a keeper whose
/// type is not a keeper type (returns false, leaves `done` alone), the hooked address, and the two routine halves.
pub(crate) fn run_ztkeeper_f_routine_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_F_ROUTINE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let probe = live_support::build_standalone_keeper(keeper_type);
    let tiles = routine_tiles(probe);
    live_support::destroy_standalone_keeper(probe);
    let world = globals().ztworldmgr();
    let mut shared = vec![keeper_type];
    let mut cases: Vec<(&str, (u32, u32), bool)> = Vec::new();
    for (label, tile) in [("no habitat", tiles.none), ("habitat", tiles.habitat)] {
        match tile {
            Some(tile) => {
                shared.push(world.get_tile_ptr(tile.0, tile.1));
                cases.push((label, tile, false));
            }
            None => failures.push(format!("no tile with {label} found")),
        }
    }
    // No tile in the zoo maps to a world habitat (flag `+0x2c` set), so that case swaps the keeper's `getHabitat` slot
    // for one returning a fake habitat with the flag set. Both poles see the same fake.
    let mut fake_habitat = vec![0u32; 0x40];
    fake_habitat[0x2c / 4] = 1;
    FAKE_HABITAT.store(fake_habitat.as_ptr() as u32, Ordering::SeqCst);
    if let Some(tile) = tiles.none {
        cases.push(("world habitat (fake)", tile, true));
    }
    let real_vtable_address = get_module_base("zoo.exe") as u32 + KEEPER_VTABLE_RVA;
    let mut fake_vtable: Vec<u32> = (0..204).map(|i| get_from_memory::<u32>(real_vtable_address + i * 4)).collect();
    fake_vtable[0x24c / 4] = fake_get_habitat as *const () as usize as u32;

    for (label, tile, fake_world) in cases {
        for goal in [0x6e_u32, 0x6f, 0x01] {
            for goal_flag in [0u8, 1] {
                for stopped in [0u8, 1] {
                    let real_keeper = live_support::build_standalone_keeper(keeper_type);
                    let port_keeper = live_support::build_standalone_keeper_port(keeper_type);
                    for keeper in [real_keeper, port_keeper] {
                        place_at_tile(keeper, tile);
                        if fake_world {
                            save_to_memory(keeper.0, fake_vtable.as_ptr() as u32);
                        }
                        save_to_memory(keeper.0 + unit_offsets::GOAL_ID, goal);
                        save_to_memory(keeper.0 + unit_offsets::COG_GOAL_FLAG, goal_flag);
                        save_to_memory(keeper.0 + unit_offsets::STOPPED, stopped);
                    }
                    let (mut real_done, mut port_done) = (0xaa_u8, 0xaa_u8);
                    let real_result = real::f_routine(real_keeper.0, &mut real_done as *mut u8 as *const bool);
                    let port_result = port_keeper.f_routine(&mut port_done as *mut u8 as *const bool);
                    let case = format!("{label}, goal {goal:#x}, goal flag {goal_flag}, stopped {stopped}");
                    if real_result != port_result {
                        failures.push(format!("{case}: real returned {real_result}, port {port_result}"));
                    }
                    let (real_done, port_done) = (unsafe { std::ptr::read_volatile(&real_done) }, unsafe { std::ptr::read_volatile(&port_done) });
                    if real_done != port_done {
                        failures.push(format!("{case}: real done {real_done:#x}, port done {port_done:#x}"));
                    }
                    for keeper in [real_keeper, port_keeper] {
                        save_to_memory(keeper.0, real_vtable_address);
                    }
                    compare_memory(&case, real_keeper, port_keeper, &shared, &mut failures);
                    live_support::destroy_standalone_keeper(real_keeper);
                    live_support::destroy_standalone_keeper(port_keeper);
                }
            }
        }
    }

    let types = entity_type_ptrs();
    if let Some(other_type) = types.iter().copied().find(|&ty| !is_keeper_type(ty)) {
        let keeper = live_support::build_standalone_keeper(keeper_type);
        save_to_memory(keeper.0 + unit_offsets::ENTITY_TYPE, other_type);
        let (mut real_done, mut port_done) = (0xaa_u8, 0xaa_u8);
        let real_result = real::f_routine(keeper.0, &mut real_done as *mut u8 as *const bool);
        let port_result = keeper.f_routine(&mut port_done as *mut u8 as *const bool);
        if real_result || port_result || real_done != 0xaa || port_done != 0xaa {
            failures.push(format!("non-keeper type: real {real_result}/{real_done:#x}, port {port_result}/{port_done:#x}, expected false and untouched"));
        }
        save_to_memory(keeper.0 + unit_offsets::ENTITY_TYPE, keeper_type);
        live_support::destroy_standalone_keeper(keeper);
    }

    let keeper = live_support::build_standalone_keeper_port(keeper_type);
    save_to_memory(keeper.0 + unit_offsets::STOPPED, 1u8);
    for (label, halves) in [("fHabitatRoutine", true), ("fEscapedAnimalRoutine", false)] {
        let (mut real_done, mut port_done) = (0xaa_u8, 0xaa_u8);
        let (real_result, port_result) = if halves {
            (real::f_habitat_routine(keeper.0, &mut real_done), unsafe { F_HABITAT_ROUTINE.hooked()(keeper.0 as *const std::ffi::c_void, &mut port_done) })
        } else {
            (real::f_escaped_animal_routine(keeper.0, &mut real_done), unsafe { F_ESCAPED_ANIMAL_ROUTINE.hooked()(keeper.0 as *const std::ffi::c_void, &mut port_done) })
        };
        if real_result & 0xff != port_result & 0xff || real_done != port_done || port_done != 1 {
            failures.push(format!("{label}: real {real_result:#x}/{real_done:#x}, hooked {port_result:#x}/{port_done:#x}, expected 1/1"));
        }
    }
    live_support::destroy_standalone_keeper(keeper);
    finish_test(test_name, failures, failure_log)
}

/// `ZTKEEPER_INIT_STATUS_VARS_LIVE`: `initStatusVars` on twin fixtures whose own dwords, `+0x64` and `+0x68` are
/// dirtied first, comparing the result byte and the whole keeper, and the `+0x68` link against the type's `+0x84`.
pub(crate) fn run_ztkeeper_init_status_vars_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_INIT_STATUS_VARS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![keeper_type, keeper_type + 0x80];
    let block_present = get_from_memory::<u32>(keeper_type + 0x84) != 0;
    let real_keeper = live_support::build_standalone_keeper(keeper_type);
    let port_keeper = live_support::build_standalone_keeper_port(keeper_type);
    for keeper in [real_keeper, port_keeper] {
        for offset in OWN_DWORD_OFFSETS {
            save_to_memory(keeper.0 + offset, 0x1234_5678u32);
        }
        save_to_memory(keeper.0 + 0x64, 0xdead_beefu32);
        save_to_memory(keeper.0 + 0x68, 0xdead_beefu32);
    }
    let rng_address = get_module_base("zoo.exe") as u32 + RNG_STATE_RVA;
    let rng_seed: u32 = get_from_memory(rng_address);
    let real_result = real::init_status_vars(real_keeper.0);
    save_to_memory(rng_address, rng_seed);
    let port_result = port_keeper.init_status_vars();
    save_to_memory(rng_address, rng_seed);
    if real_result & 0xff != port_result & 0xff {
        failures.push(format!("real returned {real_result:#x}, port {port_result:#x}"));
    }
    compare_memory("initStatusVars", real_keeper, port_keeper, &shared, &mut failures);
    let link: u32 = get_from_memory(port_keeper.0 + 0x68);
    let expected = if block_present { keeper_type + 0x80 } else { 0 };
    if link != expected {
        failures.push(format!("+0x68 is {link:#x}, expected {expected:#x}"));
    }
    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);
    finish_test(test_name, failures, failure_log)
}

/// `ZTKEEPER_TERRAIN_COST_LIVE`: `getTerrainCost` over every live tile (up to 200x200) from two keeper positions and
/// both settings of the `+0x2a8` gate-pathing byte, real against port, plus the hooked address on a sample. Fails if
/// every tile cost the same, which would mean the grid exercised a single branch.
pub(crate) fn run_ztkeeper_terrain_cost_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_TERRAIN_COST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let keeper = live_support::build_standalone_keeper(keeper_type);
    let mut seen = std::collections::BTreeSet::new();
    let mut mismatches = 0;
    for gate in [0u8, 1] {
        keeper.set_pathing_to_habitat_gate(gate);
        for position in [(0u32, 0u32), (world.map_x_size / 2, world.map_y_size / 2)] {
            place_at_tile(keeper, position);
            for y in 0..world.map_y_size.min(200) {
                for x in 0..world.map_x_size.min(200) {
                    let tile = world.get_tile_ptr(x, y);
                    let real_cost = real::get_terrain_cost(keeper.0, tile);
                    let port_cost = keeper.get_terrain_cost(tile);
                    seen.insert(real_cost);
                    if real_cost != port_cost {
                        mismatches += 1;
                        if mismatches <= 10 {
                            failures.push(format!("gate {gate}, keeper at {position:?}, tile ({x},{y}): real {real_cost}, port {port_cost}"));
                        }
                    }
                    if x == y && x % 17 == 0 {
                        let hooked = unsafe { GET_TERRAIN_COST.hooked()(keeper.0 as *const u32, tile as *const u32) };
                        if hooked != real_cost {
                            failures.push(format!("gate {gate}, tile ({x},{y}): hooked {hooked}, real {real_cost}"));
                        }
                    }
                }
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if seen.len() < 2 {
        failures.push(format!("only the costs {seen:?} occurred, so the grid covered a single branch"));
    }
    live_support::destroy_standalone_keeper(keeper);
    finish_test(test_name, failures, failure_log)
}

/// `ZTKEEPER_NOTIFY_REMOVE_LIVE`: `notifyHabitatNotComing` against a habitat counter of 3, 1, 0 and -2 (decrement,
/// clamp, untouched, clamp), a target tile with no habitat, and a gate pointer that is left alone when the keep flag
/// is set; then `removeFromMap` on a keeper with no show, through the real body, the port and the hooked address,
/// comparing the keepers and the habitat counter. The gate-closing arm needs a real fence gate and is not exercised.
pub(crate) fn run_ztkeeper_notify_remove_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_NOTIFY_REMOVE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let probe = live_support::build_standalone_keeper(keeper_type);
    let (habitat_tile, plain_tile) = habitat_and_plain_tiles(probe);
    let habitat = habitat_tile.map(|tile| call_vtable_slot_1(probe.0, 0x248, world.get_tile_ptr(tile.0, tile.1)));
    live_support::destroy_standalone_keeper(probe);
    let (Some(habitat_tile), Some(habitat), Some(plain_tile)) = (habitat_tile, habitat, plain_tile) else {
        failures.push("the zoo has no habitat tile and plain tile".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let counter_address = habitat + 0xf4;
    let original_counter: i32 = get_from_memory(counter_address);
    let tile_of = |tile: (u32, u32)| world.get_tile_ptr(tile.0, tile.1);
    let fixture = || live_support::build_standalone_keeper(keeper_type);

    for (counter, expected) in [(3i32, 2i32), (1, 0), (0, 0), (-2, 0)] {
        for (label, run_real) in [("real", true), ("port", false)] {
            let keeper = fixture();
            save_to_memory(keeper.0 + unit_offsets::TARGET_TILE, tile_of(habitat_tile));
            save_to_memory(counter_address, counter);
            if run_real {
                real::notify_habitat_not_coming(keeper.0, 1);
            } else {
                keeper.notify_habitat_not_coming(1);
            }
            let after: i32 = get_from_memory(counter_address);
            if after != expected {
                failures.push(format!("{label}: counter {counter} became {after}, expected {expected}"));
            }
            live_support::destroy_standalone_keeper(keeper);
        }
    }

    let keeper = fixture();
    save_to_memory(keeper.0 + unit_offsets::TARGET_TILE, tile_of(plain_tile));
    save_to_memory(counter_address, 3i32);
    keeper.notify_habitat_not_coming(1);
    if get_from_memory::<i32>(counter_address) != 3 {
        failures.push("a target tile with no habitat changed a habitat counter".to_string());
    }
    save_to_memory(keeper.0 + unit_offsets::GATE, 0x1000u32);
    keeper.notify_habitat_not_coming(1);
    if get_from_memory::<u32>(keeper.0 + unit_offsets::GATE) != 0x1000 {
        failures.push("a keep-gate call cleared the gate".to_string());
    }
    save_to_memory(keeper.0 + unit_offsets::GATE, 0u32);
    keeper.notify_habitat_not_coming(0);
    if get_from_memory::<u32>(keeper.0 + unit_offsets::GATE) != 0 {
        failures.push("a close call with no gate set one".to_string());
    }
    live_support::destroy_standalone_keeper(keeper);

    // removeFromMap on a keeper with no show: `notifyHabitatNotComing` (while `+0x142` is clear), then the staff body.
    let shared = vec![keeper_type, tile_of(habitat_tile), tile_of(plain_tile)];
    for flag in [false, true] {
        for mark_142 in [0u8, 1] {
            let (real_keeper, port_keeper, hooked_keeper) = (fixture(), live_support::build_standalone_keeper_port(keeper_type), fixture());
            let mut counters = Vec::new();
            for (index, keeper) in [real_keeper, port_keeper, hooked_keeper].into_iter().enumerate() {
                place_at_tile(keeper, habitat_tile);
                save_to_memory(keeper.0 + unit_offsets::TARGET_TILE, tile_of(habitat_tile));
                save_to_memory(keeper.0 + 0x142, mark_142);
                save_to_memory(counter_address, 3i32);
                match index {
                    0 => real::remove_from_map(keeper.0, flag),
                    1 => keeper.remove_from_map(flag),
                    _ => unsafe { REMOVE_FROM_MAP.hooked()(keeper.0 as *const u32, flag) },
                }
                counters.push(get_from_memory::<i32>(counter_address));
            }
            let case = format!("removeFromMap(flag {flag}, +0x142 {mark_142})");
            if counters[0] != counters[1] || counters[0] != counters[2] {
                failures.push(format!("{case}: habitat counters real {}, port {}, hooked {}", counters[0], counters[1], counters[2]));
            }
            compare_memory(&case, real_keeper, port_keeper, &shared, &mut failures);
            compare_memory(&format!("{case} (hooked)"), real_keeper, hooked_keeper, &shared, &mut failures);
            for keeper in [real_keeper, port_keeper, hooked_keeper] {
                live_support::destroy_standalone_keeper(keeper);
            }
        }
    }
    save_to_memory(counter_address, original_counter);
    finish_test(test_name, failures, failure_log)
}

/// Bytes of keeper data `load` consumes before `ZTStaff::load` for save version `version`.
fn keeper_load_bytes(version: u32) -> usize {
    let mut length = 0;
    if version >= 0x10 {
        length += 4;
    }
    if version == 0x10 {
        length += 25;
    } else {
        if version > 0x22 {
            length += 17;
        }
        if (0x34..=0x4a).contains(&version) {
            length += 4;
        }
    }
    length
}

fn run_load(keeper: ZTKeeper, bytes: Vec<u8>, version: u32, real_body: bool) -> (bool, usize) {
    io_redirect::begin_replay(bytes);
    let result = if real_body { real::load(keeper.0, std::ptr::null(), version) } else { keeper.load(std::ptr::null(), version) };
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed)
}

/// `ZTKEEPER_SAVE_LOAD_LIVE`: `save` writes the same bytes and result as the real body for dirtied fixtures; the
/// bytes are the five dwords in save order, the `+0x2a8` byte, then the `ZTStaff` data. `load` is compared over every
/// version boundary (below `0x10`, `0x10`, `0x11`, `0x22`, `0x23`, `0x33`, `0x34`, `0x4a`, `0x4b`) with a stream
/// holding exactly the keeper's data (so the `ZTStaff` read finds nothing left) and with that stream one byte short
/// (so a keeper read fails and `ZTStaff::load` must not run), comparing result, bytes consumed and the whole keeper.
/// A last round trip loads a real save of a dirtied keeper into twin fixtures.
pub(crate) fn run_ztkeeper_save_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_SAVE_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![keeper_type];
    let dirty = |keeper: ZTKeeper| {
        keeper.set_habitat_select_disabled(0x0102_0304);
        keeper.set_food_to_leave(0x1112_1314);
        keeper.set_sick_animal_count(0x2122_2324);
        keeper.set_dirt_tile_count(0x3132_3334);
        keeper.set_food_type_to_leave(0x4142_4344);
        keeper.set_pathing_to_habitat_gate(0x51);
    };

    let (real_keeper, port_keeper) = (live_support::build_standalone_keeper(keeper_type), live_support::build_standalone_keeper_port(keeper_type));
    dirty(real_keeper);
    dirty(port_keeper);
    io_redirect::begin_capture();
    let real_ok = real::save(real_keeper.0, std::ptr::null());
    let real_bytes = io_redirect::end_capture();
    io_redirect::begin_capture();
    let port_ok = port_keeper.save(std::ptr::null());
    let port_bytes = io_redirect::end_capture();
    if real_ok != port_ok {
        failures.push(format!("save: real returned {real_ok}, port {port_ok}"));
    }
    if real_bytes != port_bytes {
        failures.push(format!("save: real wrote {} bytes, port {}, or the bytes differ", real_bytes.len(), port_bytes.len()));
    }
    let expected_prefix = [0x04u8, 0x03, 0x02, 0x01, 0x14, 0x13, 0x12, 0x11, 0x24, 0x23, 0x22, 0x21, 0x34, 0x33, 0x32, 0x31, 0x44, 0x43, 0x42, 0x41, 0x51];
    if !port_bytes.starts_with(&expected_prefix) {
        failures.push(format!("save: leading bytes {:02x?}, expected {:02x?}", &port_bytes[..port_bytes.len().min(21)], expected_prefix));
    }
    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);

    for version in [0x0f_u32, 0x10, 0x11, 0x22, 0x23, 0x33, 0x34, 0x4a, 0x4b] {
        let length = keeper_load_bytes(version);
        let stream: Vec<u8> = (0..length).map(|i| 0x61 + i as u8).collect();
        let mut streams = vec![("exact", stream.clone())];
        if length > 0 {
            streams.push(("short", stream[..length - 1].to_vec()));
        }
        for (label, bytes) in streams {
            let (real_keeper, port_keeper) = (live_support::build_standalone_keeper(keeper_type), live_support::build_standalone_keeper_port(keeper_type));
            let (real_result, real_consumed) = run_load(real_keeper, bytes.clone(), version, true);
            let (port_result, port_consumed) = run_load(port_keeper, bytes, version, false);
            let case = format!("load v{version:#x} ({label})");
            if real_result != port_result {
                failures.push(format!("{case}: real returned {real_result}, port {port_result}"));
            }
            if real_consumed != port_consumed {
                failures.push(format!("{case}: real consumed {real_consumed} bytes, port {port_consumed}"));
            }
            compare_memory(&case, real_keeper, port_keeper, &shared, &mut failures);
            live_support::destroy_standalone_keeper(real_keeper);
            live_support::destroy_standalone_keeper(port_keeper);
        }
    }

    let source = live_support::build_standalone_keeper(keeper_type);
    dirty(source);
    io_redirect::begin_capture();
    real::save(source.0, std::ptr::null());
    let saved = io_redirect::end_capture();
    live_support::destroy_standalone_keeper(source);
    let (real_keeper, port_keeper) = (live_support::build_standalone_keeper(keeper_type), live_support::build_standalone_keeper_port(keeper_type));
    let (real_result, real_consumed) = run_load(real_keeper, saved.clone(), 0x4b, true);
    let (port_result, port_consumed) = run_load(port_keeper, saved, 0x4b, false);
    if real_result != port_result || real_consumed != port_consumed {
        failures.push(format!("round trip: real {real_result}/{real_consumed}, port {port_result}/{port_consumed}"));
    }
    compare_memory("round trip", real_keeper, port_keeper, &shared, &mut failures);
    if port_keeper.food_type_to_leave() != 0x4142_4344 || port_keeper.pathing_to_habitat_gate() != 0x51 {
        failures.push("round trip: the port did not restore the saved fields".to_string());
    }
    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);
    finish_test(test_name, failures, failure_log)
}

const LISTEN_SHOW_ID: u16 = 0x7e01;
/// The events `fake_list_events` hands back, as `(show id, item index, kind)`.
static LISTEN_EVENTS: std::sync::Mutex<Vec<(u32, u16, u16)>> = std::sync::Mutex::new(Vec::new());

/// Stands in for the keeper's `getEvents` list slot: a vanilla-allocated vector of `0x1c`-byte records with room for
/// five, which is over the pool's small-block limit so the caller's release goes through `operator_delete`.
extern "thiscall" fn fake_list_events(_this: u32, out: u32) {
    let events = LISTEN_EVENTS.lock().unwrap().clone();
    let small = LISTEN_SMALL_VECTOR.load(Ordering::Relaxed) != 0;
    let (size, capacity_bytes) = if small { (LISTEN_SMALL_BYTES, LISTEN_SMALL_BYTES) } else { (0x8c, 0x8c) };
    let begin = if small {
        unsafe { openzt_detour::generated::poolalloc::ALLOCATE.original()(size) as u32 }
    } else {
        unsafe { openzt_detour::generated::standalone::OPERATOR_NEW.original()(size) as u32 }
    };
    LISTEN_LAST_BLOCK.store(begin, Ordering::Relaxed);
    unsafe { std::ptr::write_bytes(begin as *mut u8, 0, size as usize) };
    for (i, (show, index, kind)) in events.iter().enumerate() {
        let record = begin + i as u32 * 0x1c;
        save_to_memory(record + 0x8, *show);
        save_to_memory(record + 0x14, *index);
        save_to_memory(record + 0x16, *kind);
    }
    save_to_memory(out, begin);
    save_to_memory(out + 4, begin + events.len() as u32 * 0x1c);
    save_to_memory(out + 8, begin + capacity_bytes);
}

/// When non-zero, `fake_list_events` allocates its vector from vanilla's pool with `LISTEN_SMALL_BYTES` (four records,
/// at the pool's small-block limit) so `listen`'s release takes the freelist branch of `PoolAlloc::deallocate`.
static LISTEN_SMALL_VECTOR: AtomicU32 = AtomicU32::new(0);
const LISTEN_SMALL_BYTES: u32 = 4 * 0x1c;
static LISTEN_LAST_BLOCK: AtomicU32 = AtomicU32::new(0);
/// Freelist head of the pool bucket covering `LISTEN_SMALL_BYTES`: `DAT_00638000 + ((bytes - 1) >> 3) * 4`.
fn small_bucket_head() -> u32 {
    get_from_memory(get_module_base("zoo.exe") as u32 + 0x0023_8000 + ((LISTEN_SMALL_BYTES - 1) >> 3) * 4)
}

fn listen_name(keeper: ZTKeeper) -> Vec<u8> {
    let begin: u32 = get_from_memory(keeper.0 + 0x54);
    let end: u32 = get_from_memory(keeper.0 + 0x58);
    if begin == 0 || end < begin { Vec::new() } else { (begin..end).map(get_from_memory::<u8>).collect() }
}

/// `ZTKEEPER_LISTEN_LIVE`: `listen` on twin fixtures whose `getEvents` list slot returns scripted events, with a show
/// and a two-item script registered in the stores. Sound A (`0x271d`) plays the item's pre-trick name, sound B
/// (`0x271f`) the post-trick name and wins when non-empty, `0x2715` alone and events for another show do nothing, and
/// nothing plays unless the cog-goal state is `0x13`. Compares the behavior-set name `setBS` stored, the whole keeper,
/// and the hooked address.
pub(crate) fn run_ztkeeper_listen_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::{
        util::ZTBufferString,
        ztshow::{
            script::{
                live_support::raw_item_matching_type,
                mgr::{add_item, get_script, register_script, unregister_script_by_id},
            },
            mgr::ztshowmgr::lock_show_store,
        },
    };

    let test_name = "ZTKEEPER_LISTEN_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };

    let leaked = |text: &str| -> ZTBufferString {
        let bytes: &'static mut [u8] = Box::leak(format!("{text}\0").into_bytes().into_boxed_slice());
        let start = bytes.as_ptr() as u32;
        ZTBufferString::from_raw_parts(start, start + text.len() as u32, start + bytes.len() as u32)
    };
    let Some(script_id) = register_script(0x1234_5678, 0) else {
        failures.push("could not register a show script".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let handle = get_script(script_id);
    for (index, (pre, post)) in [("bSoundA", "bSoundB"), ("bOnlyA", "")].into_iter().enumerate() {
        let mut item = raw_item_matching_type(0, index as u16);
        item.keeper_pre_trick = leaked(pre);
        item.keeper_post_trick = leaked(post);
        add_item(handle, &item);
    }
    let mut show_info = vec![0u32; 0x20];
    show_info[0x8 / 4] = script_id as u32;
    lock_show_store().registered_shows.insert(LISTEN_SHOW_ID, show_info.as_ptr() as u32);

    let show = LISTEN_SHOW_ID as u32;
    let other = show + 1;
    // (label, events, cog-goal state, expected behavior-set name)
    let cases: Vec<(&str, Vec<(u32, u16, u16)>, u32, &str)> = vec![
        ("sound A", vec![(show, 0, 0x271d)], 0x13, "bSoundA"),
        ("sound B", vec![(show, 0, 0x271f)], 0x13, "bSoundB"),
        ("A then B: B wins", vec![(show, 0, 0x271d), (show, 0, 0x271f)], 0x13, "bSoundB"),
        ("empty B falls back to A", vec![(show, 1, 0x271d), (show, 1, 0x271f)], 0x13, "bOnlyA"),
        ("last A index wins", vec![(show, 0, 0x271d), (show, 1, 0x271d)], 0x13, "bOnlyA"),
        ("0x2715 sets B's index", vec![(show, 1, 0x2715), (show, 0, 0x271f)], 0x13, "bSoundB"),
        ("0x2715 alone", vec![(show, 0, 0x2715)], 0x13, ""),
        ("another show", vec![(other, 0, 0x271f)], 0x13, ""),
        ("unknown kind", vec![(show, 0, 0x2716)], 0x13, ""),
        ("wrong cog-goal state", vec![(show, 0, 0x271f)], 0x12, ""),
        ("no events", vec![], 0x13, ""),
        ("missing item", vec![(show, 7, 0x271f)], 0x13, ""),
    ];

    let real_vtable_address = get_module_base("zoo.exe") as u32 + KEEPER_VTABLE_RVA;
    let mut fake_vtable: Vec<u32> = (0..VTABLE_SLOTS).map(|i| get_from_memory::<u32>(real_vtable_address + i * 4)).collect();
    fake_vtable[2] = fake_list_events as *const () as usize as u32;
    let shared = vec![keeper_type];

    for (label, events, state, expected) in cases {
        for pole in ["real", "port", "hooked"] {
            *LISTEN_EVENTS.lock().unwrap() = events.clone();
            let keeper = if pole == "port" { live_support::build_standalone_keeper_port(keeper_type) } else { live_support::build_standalone_keeper(keeper_type) };
            save_to_memory(keeper.0 + unit_offsets::SHOW_INFO_ID, LISTEN_SHOW_ID);
            save_to_memory(keeper.0 + unit_offsets::COG_GOAL_STATE, state);
            save_to_memory(keeper.0, fake_vtable.as_ptr() as u32);
            match pole {
                "real" => real::listen(keeper.0),
                "port" => keeper.listen(),
                _ => unsafe { openzt_detour::generated::ztkeeper::LISTEN.hooked()(keeper.0 as *const std::ffi::c_void) },
            }
            let name = listen_name(keeper);
            if name != expected.as_bytes() {
                failures.push(format!("{label} ({pole}): behavior set {:?}, expected {expected:?}", String::from_utf8_lossy(&name)));
            }
            save_to_memory(keeper.0, real_vtable_address);
            if pole != "real" {
                // Compare against a fresh real run of the same case.
                *LISTEN_EVENTS.lock().unwrap() = events.clone();
                let reference = live_support::build_standalone_keeper(keeper_type);
                save_to_memory(reference.0 + unit_offsets::SHOW_INFO_ID, LISTEN_SHOW_ID);
                save_to_memory(reference.0 + unit_offsets::COG_GOAL_STATE, state);
                save_to_memory(reference.0, fake_vtable.as_ptr() as u32);
                real::listen(reference.0);
                save_to_memory(reference.0, real_vtable_address);
                compare_memory(&format!("{label} ({pole})"), reference, keeper, &shared, &mut failures);
                live_support::destroy_standalone_keeper(reference);
            }
            live_support::destroy_standalone_keeper(keeper);
        }
    }

    // A vector small enough for the pool freelist: the block goes back onto its bucket, never to `operator_delete`.
    LISTEN_SMALL_VECTOR.store(1, Ordering::Relaxed);
    for pole in ["real", "port", "hooked"] {
        *LISTEN_EVENTS.lock().unwrap() = vec![(show, 0, 0x271f)];
        let keeper = if pole == "port" { live_support::build_standalone_keeper_port(keeper_type) } else { live_support::build_standalone_keeper(keeper_type) };
        save_to_memory(keeper.0 + unit_offsets::SHOW_INFO_ID, LISTEN_SHOW_ID);
        save_to_memory(keeper.0 + unit_offsets::COG_GOAL_STATE, 0x13u32);
        save_to_memory(keeper.0, fake_vtable.as_ptr() as u32);
        match pole {
            "real" => real::listen(keeper.0),
            "port" => keeper.listen(),
            _ => unsafe { openzt_detour::generated::ztkeeper::LISTEN.hooked()(keeper.0 as *const std::ffi::c_void) },
        }
        save_to_memory(keeper.0, real_vtable_address);
        let block = LISTEN_LAST_BLOCK.load(Ordering::Relaxed);
        if small_bucket_head() != block {
            failures.push(format!("small vector ({pole}): freelist head {:#x}, expected the released block {block:#x}", small_bucket_head()));
        }
        if listen_name(keeper) != b"bSoundB" {
            failures.push(format!("small vector ({pole}): behavior set {:?}", String::from_utf8_lossy(&listen_name(keeper))));
        }
        live_support::destroy_standalone_keeper(keeper);
    }
    LISTEN_SMALL_VECTOR.store(0, Ordering::Relaxed);

    lock_show_store().registered_shows.remove(&LISTEN_SHOW_ID);
    unregister_script_by_id(script_id);
    drop(show_info);
    finish_test(test_name, failures, failure_log)
}

/// While set, `ZTUnit::updateGoals` (the tail every `ZTKeeper::updateGoals` path ends with) is recorded instead of run: the
/// real one evaluates the cog goal's `decide`, which reaches habitat code that cannot take the fake habitats below.
pub(crate) static GOALS_INTERCEPT: AtomicU32 = AtomicU32::new(0);

#[openzt_detour_macro::detour_mod]
mod unit_goals_hook {
    use super::*;

    #[detour(ZTUNIT_UPDATE_GOALS)]
    unsafe extern "thiscall" fn unit_update_goals(this: *const u32) {
        if GOALS_INTERCEPT.load(Ordering::SeqCst) != 0 {
            goals_record(6, 0, 0, 0);
        } else {
            unsafe { ZTUNIT_UPDATE_GOALS_DETOUR.call(this) }
        }
    }
}

pub(crate) static GOALS_LOG: std::sync::Mutex<Vec<[u32; 4]>> = std::sync::Mutex::new(Vec::new());
static GOALS_HABITAT: AtomicU32 = AtomicU32::new(0);
pub(crate) static GOALS_PREDATOR: AtomicU32 = AtomicU32::new(0);
static GOALS_ESCAPED: AtomicU32 = AtomicU32::new(0);
static GOALS_TANK: AtomicU32 = AtomicU32::new(0);
/// When set, `getHabitatFromTile` answers differently for the target tile and the current tile.
static GOALS_DIFFERENT_TILES: AtomicU32 = AtomicU32::new(0);
static GOALS_TILE_CALLS: AtomicU32 = AtomicU32::new(0);

pub(crate) fn goals_record(kind: u32, a: u32, b: u32, c: u32) {
    GOALS_LOG.lock().unwrap().push([kind, a, b, c]);
}

extern "thiscall" fn goals_get_habitat(_this: u32) -> u32 {
    GOALS_HABITAT.load(Ordering::SeqCst)
}

extern "thiscall" fn goals_get_habitat_from_tile(_this: u32, _tile: u32) -> u32 {
    let call = GOALS_TILE_CALLS.fetch_add(1, Ordering::SeqCst);
    if GOALS_DIFFERENT_TILES.load(Ordering::SeqCst) != 0 && call % 2 == 1 {
        return 0x1111;
    }
    GOALS_HABITAT.load(Ordering::SeqCst)
}

pub(crate) extern "thiscall" fn goals_find_predator(_this: u32, arg: u32) -> u32 {
    goals_record(1, arg, 0, 0);
    GOALS_PREDATOR.load(Ordering::SeqCst)
}

extern "thiscall" fn goals_found_escaped(_this: u32) -> u32 {
    goals_record(2, 0, 0, 0);
    GOALS_ESCAPED.load(Ordering::SeqCst)
}

extern "thiscall" fn goals_stop_everything(_this: u32, a: u32, b: u32, c: u32) {
    goals_record(3, a, b, c);
}

pub(crate) extern "thiscall" fn goals_goal_set(_this: u32, goal: u32, subtype: u32) {
    goals_record(4, goal, subtype, 0);
}

pub(crate) extern "thiscall" fn goals_set_target_tile(_this: u32, tile: u32, flag: u32) {
    goals_record(5, tile, flag, 0);
}

extern "thiscall" fn goals_is_tank(_habitat: u32) -> u32 {
    GOALS_TANK.load(Ordering::SeqCst)
}

/// Installs the `ZTUnit::updateGoals` recording hook once; returns an error message on failure.
pub(crate) fn install_goals_hook() -> Result<(), String> {
    static HOOK_INSTALLED: std::sync::Once = std::sync::Once::new();
    let mut result = Ok(());
    HOOK_INSTALLED.call_once(|| {
        if let Err(e) = unsafe { unit_goals_hook::init_detours() } {
            result = Err(format!("could not install the ZTUnit::updateGoals hook: {e:?}"));
        }
    });
    result
}

/// `ZTKEEPER_UPDATE_GOALS_LIVE`: `updateGoals` on twin fixtures over the cross product of every input the branches
/// read - the predator-check flag, goal id, predator and escaped-animal results, cog-goal validity and flag, the
/// habitat (none, plain, world, tank), whether the target and current tile share a habitat, and whether the habitat is
/// serviced by one or two keepers. The virtual calls (`getHabitat`, `findPredator`, `foundEscapedAnimal`,
/// `stopEverything`, `goalSet`, `setTargetTile`, `getHabitatFromTile`, the habitat's `isTank`) go to recording fakes,
/// so the test compares the exact call sequence as well as the keeper's memory. Fails if the sweep produced fewer than
/// ten distinct call sequences, which would mean most branches were not reached.
pub(crate) fn run_ztkeeper_update_goals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_UPDATE_GOALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_goals_hook() {
        failures.push(message);
    }
    GOALS_INTERCEPT.store(1, Ordering::SeqCst);
    let shared = vec![keeper_type];
    let real_vtable_address = get_module_base("zoo.exe") as u32 + KEEPER_VTABLE_RVA;
    let mut fake_vtable: Vec<u32> = (0..VTABLE_SLOTS).map(|i| get_from_memory::<u32>(real_vtable_address + i * 4)).collect();
    for (offset, function) in [
        (0x24c, goals_get_habitat as *const () as usize),
        (0x248, goals_get_habitat_from_tile as *const ()as usize),
        (0x2fc, goals_find_predator as *const () as usize),
        (0x32c, goals_found_escaped as *const () as usize),
        (0x198, goals_stop_everything as *const () as usize),
        (0x1d8, goals_goal_set as *const () as usize),
        (0x2d8, goals_set_target_tile as *const () as usize),
    ] {
        fake_vtable[offset / 4] = function as u32;
    }
    let mut habitat_vtable = vec![0u32; 64];
    habitat_vtable[0x20 / 4] = goals_is_tank as *const () as usize as u32;
    let mut habitats: Vec<Vec<u32>> = Vec::new();
    for world in [0u8, 1] {
        for keepers in [1i32, 2] {
            for serviced in [0u8, 1] {
                let mut habitat = vec![0u32; 0x60];
                habitat[0] = habitat_vtable.as_ptr() as u32;
                save_to_memory(habitat.as_mut_ptr() as u32 + 0x2c, world);
                save_to_memory(habitat.as_mut_ptr() as u32 + 0xf0, keepers);
                save_to_memory(habitat.as_mut_ptr() as u32 + 0x132, serviced);
                habitats.push(habitat);
            }
        }
    }

    let real_keeper = live_support::build_standalone_keeper(keeper_type);
    let port_keeper = live_support::build_standalone_keeper_port(keeper_type);
    for keeper in [real_keeper, port_keeper] {
        place_at_tile(keeper, (10, 10));
        save_to_memory(keeper.0, fake_vtable.as_ptr() as u32);
        save_to_memory(keeper.0 + unit_offsets::ESCAPED_ANIMAL, 0x77u32);
    }
    let rng_address = get_module_base("zoo.exe") as u32 + RNG_STATE_RVA;
    let rng_seed: u32 = get_from_memory(rng_address);

    let mut sequences = std::collections::BTreeSet::new();
    let mut mismatches = 0;
    let mut cases = 0;
    // Habitat choices: none, then every fake habitat; a tank answer applies to the non-world ones.
    let mut habitat_choices: Vec<(String, u32, u32)> = vec![("none".to_string(), 0, 0)];
    for (index, habitat) in habitats.iter().enumerate() {
        for tank in [0u32, 1] {
            habitat_choices.push((format!("habitat {index} tank {tank}"), habitat.as_ptr() as u32, tank));
        }
    }
    for pending in [0u8, 1] {
        for goal in [0x75u32, 0x6f, 0x6e] {
            for predator in [0u32, 0x1234] {
                for escaped in [0u32, 1] {
                    for cog_valid_byte in [0u8, 1] {
                        for goal_flag in [0u8, 1] {
                            for different_tiles in [0u32, 1] {
                                for (habitat_label, habitat, tank) in &habitat_choices {
                                    cases += 1;
                                    GOALS_HABITAT.store(*habitat, Ordering::SeqCst);
                                    GOALS_PREDATOR.store(predator, Ordering::SeqCst);
                                    GOALS_ESCAPED.store(escaped, Ordering::SeqCst);
                                    GOALS_TANK.store(*tank, Ordering::SeqCst);
                                    GOALS_DIFFERENT_TILES.store(different_tiles, Ordering::SeqCst);
                                    let mut logs = Vec::new();
                                    for (pole, keeper) in [real_keeper, port_keeper].into_iter().enumerate() {
                                        save_to_memory(keeper.0 + unit_offsets::PREDATOR_CHECK_PENDING, pending);
                                        save_to_memory(keeper.0 + unit_offsets::GOAL_ID, goal);
                                        save_to_memory(keeper.0 + unit_offsets::COG_INVALID, cog_valid_byte);
                                        save_to_memory(keeper.0 + unit_offsets::COG_GOAL_FLAG, goal_flag);
                                        GOALS_LOG.lock().unwrap().clear();
                                        GOALS_TILE_CALLS.store(0, Ordering::SeqCst);
                                        save_to_memory(rng_address, rng_seed);
                                        if pole == 0 {
                                            real::update_goals(keeper.0);
                                        } else {
                                            keeper.update_goals();
                                        }
                                        logs.push(GOALS_LOG.lock().unwrap().clone());
                                    }
                                    let case = format!(
                                        "pending {pending}, goal {goal:#x}, predator {predator:#x}, escaped {escaped}, cog byte {cog_valid_byte}, goal flag {goal_flag}, different tiles {different_tiles}, {habitat_label}"
                                    );
                                    if logs[0] != logs[1] {
                                        mismatches += 1;
                                        if mismatches <= 10 {
                                            failures.push(format!("{case}: real calls {:?}, port calls {:?}", logs[0], logs[1]));
                                        }
                                    }
                                    sequences.insert(format!("{:?}", logs[0]));
                                    if mismatches <= 10 {
                                        compare_memory(&case, real_keeper, port_keeper, &shared, &mut failures);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} call-sequence mismatches in {cases} cases"));
    }
    if sequences.len() < 10 {
        failures.push(format!("only {} distinct call sequences in {cases} cases", sequences.len()));
    }
    // No habitat manager at all (the global vanilla and the port both read): the no-habitat goal.
    let manager_global = get_module_base("zoo.exe") as u32 + 0x0023_805c;
    let manager: u32 = get_from_memory(manager_global);
    save_to_memory(manager_global, 0u32);
    // `habitats[4]` is the first fake world habitat; with the goal flag set it takes the invalid-goal branch.
    GOALS_HABITAT.store(habitats[4].as_ptr() as u32, Ordering::SeqCst);
    GOALS_TANK.store(0, Ordering::SeqCst);
    GOALS_PREDATOR.store(0, Ordering::SeqCst);
    GOALS_ESCAPED.store(0, Ordering::SeqCst);
    for pending in [0u8, 1] {
        for cog_valid_byte in [0u8, 1] {
            for goal_flag in [0u8, 1] {
                let mut logs = Vec::new();
                for (pole, keeper) in [real_keeper, port_keeper].into_iter().enumerate() {
                    save_to_memory(keeper.0 + unit_offsets::PREDATOR_CHECK_PENDING, pending);
                    save_to_memory(keeper.0 + unit_offsets::GOAL_ID, 0x6eu32);
                    save_to_memory(keeper.0 + unit_offsets::COG_INVALID, cog_valid_byte);
                    save_to_memory(keeper.0 + unit_offsets::COG_GOAL_FLAG, goal_flag);
                    GOALS_LOG.lock().unwrap().clear();
                    save_to_memory(rng_address, rng_seed);
                    if pole == 0 {
                        real::update_goals(keeper.0);
                    } else {
                        keeper.update_goals();
                    }
                    logs.push(GOALS_LOG.lock().unwrap().clone());
                }
                if logs[0] != logs[1] {
                    failures.push(format!("no manager, pending {pending}, cog byte {cog_valid_byte}, goal flag {goal_flag}: real {:?}, port {:?}", logs[0], logs[1]));
                }
                compare_memory("no manager", real_keeper, port_keeper, &shared, &mut failures);
                sequences.insert(format!("{:?}", logs[0]));
            }
        }
    }
    save_to_memory(manager_global, manager);
    for (label, wanted) in [
        ("predator goal", "[4, 117, 3, 0]"),
        ("tank goal", "[4, 127, 0, 0]"),
        ("no-habitat goal", "[4, 1, 1, 0]"),
        ("habitats-exist goal", "[4, 101, 4, 0]"),
        ("habitat duties goal", "[4, 110, 0, 0]"),
        ("stop everything", "[3, 1, 0, 1]"),
        ("retarget", "[5, 0, 1, 0]"),
    ] {
        if !sequences.iter().any(|sequence| sequence.contains(wanted)) {
            failures.push(format!("no case reached the {label} ({wanted})"));
        }
    }
    // The hooked address runs the port.
    GOALS_HABITAT.store(0, Ordering::SeqCst);
    GOALS_ESCAPED.store(1, Ordering::SeqCst);
    GOALS_LOG.lock().unwrap().clear();
    unsafe { openzt_detour::generated::ztkeeper::UPDATE_GOALS.hooked()(port_keeper.0 as *const u32) };
    if GOALS_LOG.lock().unwrap().is_empty() {
        failures.push("the hooked address made no virtual calls".to_string());
    }
    for keeper in [real_keeper, port_keeper] {
        save_to_memory(keeper.0, real_vtable_address);
    }
    live_support::destroy_standalone_keeper(real_keeper);
    live_support::destroy_standalone_keeper(port_keeper);
    save_to_memory(rng_address, rng_seed);
    GOALS_INTERCEPT.store(0, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}

/// Median nanoseconds per call for each pole. Each pole's `pass` makes `calls_per_pass` calls; the poles alternate
/// within every round so slow drift in machine load affects them equally, after one untimed warm-up pass each.
pub(super) fn median_ns_per_call(poles: &mut [&mut dyn FnMut()], rounds: usize, calls_per_pass: usize) -> Vec<f64> {
    for pass in poles.iter_mut() {
        pass();
    }
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); poles.len()];
    for _ in 0..rounds {
        for (index, pass) in poles.iter_mut().enumerate() {
            let start = std::time::Instant::now();
            pass();
            samples[index].push(start.elapsed().as_nanos() as f64 / calls_per_pass as f64);
        }
    }
    samples
        .into_iter()
        .map(|mut values| {
            values.sort_by(|a, b| a.total_cmp(b));
            values[values.len() / 2]
        })
        .collect()
}

/// A per-call timing for one hot function: vanilla body (through the detour's call-through), the Rust body called
/// directly, and the hooked address the game itself calls.
pub(super) struct PoleTimes {
    pub(super) label: String,
    pub(super) vanilla: f64,
    pub(super) port: f64,
    pub(super) hooked: f64,
}

impl PoleTimes {
    pub(super) fn summary(&self) -> String {
        format!(
            "{}: vanilla {:.1}ns, port {:.1}ns ({:.2}x), hooked {:.1}ns ({:.2}x)",
            self.label,
            self.vanilla,
            self.port,
            self.port / self.vanilla,
            self.hooked,
            self.hooked / self.vanilla
        )
    }
}

/// Ratio of hooked to vanilla time above which the benchmark fails. Timing on a shared battery is noisy, so this only
/// catches a gross regression; the logged numbers are the actual result.
pub(super) const PERF_FAIL_RATIO: f64 = 5.0;

/// `ZTKEEPER_PERF_COMPARE_LIVE`: per-call cost of the two hot `ZTKeeper` functions, vanilla against the Rust port,
/// run in the release battery only (skipped in debug builds). `getTerrainCost` runs over every live tile (up to 200x200) for both settings of the
/// gate-pathing byte, the pathfinder's access pattern; `fRoutine` runs on the habitat-duties path and on the idle
/// fall-through, where the shared `BFUnit::idle` dominates both poles. The timings are logged on the pass line.
/// Run with page heap off.
pub(crate) fn run_ztkeeper_perf_compare_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::hint::black_box;

    let test_name = "ZTKEEPER_PERF_COMPARE_LIVE";
    // A debug build's `.original()` looks the address up in the hook registry on every call, which dwarfs the
    // ported bodies' own cost, so the comparison is only meaningful in release.
    if cfg!(debug_assertions) {
        write_success_line(failure_log, &format!("{test_name} (skipped: debug build)"));
        return false;
    }
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let mut results: Vec<PoleTimes> = Vec::new();

    let keeper = live_support::build_standalone_keeper(keeper_type);
    place_at_tile(keeper, (world.map_x_size / 2, world.map_y_size / 2));
    let mut tiles: Vec<u32> = Vec::new();
    for y in 0..world.map_y_size.min(200) {
        for x in 0..world.map_x_size.min(200) {
            tiles.push(world.get_tile_ptr(x, y));
        }
    }
    for gate in [0u8, 1] {
        keeper.set_pathing_to_habitat_gate(gate);
        let calls = tiles.len();
        let mut vanilla = || {
            for &tile in &tiles {
                black_box(real::get_terrain_cost(keeper.0, black_box(tile)));
            }
        };
        let mut port = || {
            for &tile in &tiles {
                black_box(keeper.get_terrain_cost(black_box(tile)));
            }
        };
        let mut hooked = || {
            for &tile in &tiles {
                black_box(unsafe { GET_TERRAIN_COST.hooked()(keeper.0 as *const u32, black_box(tile) as *const u32) });
            }
        };
        let times = median_ns_per_call(&mut [&mut vanilla, &mut port, &mut hooked], 9, calls);
        results.push(PoleTimes { label: format!("getTerrainCost gate {gate} ({calls} tiles)"), vanilla: times[0], port: times[1], hooked: times[2] });
    }
    keeper.set_pathing_to_habitat_gate(0);
    live_support::destroy_standalone_keeper(keeper);

    let probe = live_support::build_standalone_keeper(keeper_type);
    let tiles = routine_tiles(probe);
    live_support::destroy_standalone_keeper(probe);
    for (label, tile, goal) in [("fRoutine habitat duties", tiles.habitat, 0x6e_u32), ("fRoutine idle fall-through", tiles.none, 0x01)] {
        let Some(tile) = tile else {
            failures.push(format!("no tile found for {label}"));
            continue;
        };
        let keeper = live_support::build_standalone_keeper(keeper_type);
        place_at_tile(keeper, tile);
        save_to_memory(keeper.0 + unit_offsets::GOAL_ID, goal);
        save_to_memory(keeper.0 + unit_offsets::COG_GOAL_FLAG, 0u8);
        save_to_memory(keeper.0 + unit_offsets::STOPPED, 1u8);
        const CALLS: usize = 20_000;
        let mut done_byte = 0u8;
        let done = &mut done_byte as *mut u8 as *const bool;
        let mut vanilla = || {
            for _ in 0..CALLS {
                black_box(real::f_routine(keeper.0, done));
            }
        };
        let mut port = || {
            for _ in 0..CALLS {
                black_box(keeper.f_routine(done));
            }
        };
        let mut hooked = || {
            for _ in 0..CALLS {
                black_box(unsafe { F_ROUTINE.hooked()(keeper.0 as *const u32, done) });
            }
        };
        let times = median_ns_per_call(&mut [&mut vanilla, &mut port, &mut hooked], 9, CALLS);
        results.push(PoleTimes { label: label.to_string(), vanilla: times[0], port: times[1], hooked: times[2] });
        live_support::destroy_standalone_keeper(keeper);
    }

    for result in &results {
        if result.hooked / result.vanilla > PERF_FAIL_RATIO {
            failures.push(format!("hooked is more than {PERF_FAIL_RATIO}x vanilla: {}", result.summary()));
        }
    }
    if failures.is_empty() {
        let summary = results.iter().map(PoleTimes::summary).collect::<Vec<_>>().join("; ");
        write_success_line(failure_log, &format!("{test_name} ({summary})"));
        false
    } else {
        finish_test(test_name, failures, failure_log)
    }
}

/// While set, the three vanilla calls `ZTKeeper`'s conditionals/removal paths end in (`ZTStaff::doConditionals`,
/// `ZTFence::closeGate`, `ZTShow::abortShow`) are recorded instead of run: each reaches live-game state a bare fixture
/// does not have.
static ARMS_INTERCEPT: AtomicU32 = AtomicU32::new(0);
static ARMS_LOG: std::sync::Mutex<Vec<(&'static str, u32)>> = std::sync::Mutex::new(Vec::new());

#[openzt_detour_macro::detour_mod]
mod arms_hooks {
    use super::*;

    #[detour(ARMS_STAFF_DO_CONDITIONALS)]
    unsafe extern "thiscall" fn staff_do_conditionals(this: *const u32) {
        if ARMS_INTERCEPT.load(Ordering::SeqCst) != 0 {
            ARMS_LOG.lock().unwrap().push(("staff", 0));
        } else {
            unsafe { ARMS_STAFF_DO_CONDITIONALS_DETOUR.call(this) }
        }
    }

    #[detour(ARMS_CLOSE_GATE)]
    unsafe extern "thiscall" fn close_gate(this: *const std::ffi::c_void) {
        if ARMS_INTERCEPT.load(Ordering::SeqCst) != 0 {
            ARMS_LOG.lock().unwrap().push(("close_gate", this as u32));
        } else {
            unsafe { ARMS_CLOSE_GATE_DETOUR.call(this) }
        }
    }

    #[detour(ARMS_ABORT_SHOW)]
    unsafe extern "thiscall" fn abort_show(this: *const u32) -> bool {
        if ARMS_INTERCEPT.load(Ordering::SeqCst) != 0 {
            ARMS_LOG.lock().unwrap().push(("abort_show", this as u32));
            true
        } else {
            unsafe { ARMS_ABORT_SHOW_DETOUR.call(this) }
        }
    }
}

fn install_arms_hooks() -> Result<(), String> {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { arms_hooks::init_detours() } {
            result = Err(format!("could not install the keeper conditionals/removal hooks: {e:?}"));
        }
    });
    result
}

/// A zeroed habitat whose owned-tile list is empty, so `ZTHabitat::blockService` returns `1` (fewer than two tiles)
/// for any keeper. Leaked: the only reader is `blockService` and vanilla's `removeAssignedHabitat` never dereferences
/// the payload.
fn blocking_habitat() -> u32 {
    let sentinel: &'static mut [u32; 3] = Box::leak(Box::new([0u32; 3]));
    let sentinel_address = sentinel.as_ptr() as u32;
    sentinel[0] = sentinel_address;
    sentinel[1] = sentinel_address;
    let habitat: &'static mut [u32; 0x80] = Box::leak(Box::new([0u32; 0x80]));
    habitat[0x40 / 4] = sentinel_address;
    habitat.as_ptr() as u32
}

/// `ZTKEEPER_DO_CONDITIONALS_LIVE`: `doConditionals` and `checkAssignedHabitats` over assigned lists whose nodes come
/// from vanilla's allocator (`list<uint>::insert`), mixing blocking fake habitats (`blockService == 1`) with a live
/// habitat, so the removal arm and its restart-from-the-head walk run. Real, port and hooked address all end with the
/// same assigned list, the same `ZTStaff::doConditionals` call (recorded, run once), and the same keeper memory.
pub(crate) fn run_ztkeeper_do_conditionals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTKEEPER_DO_CONDITIONALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(message) = install_arms_hooks() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let probe = live_support::build_standalone_keeper(keeper_type);
    let (habitat_tile, _) = habitat_and_plain_tiles(probe);
    let live_habitat = habitat_tile.map(|tile| call_vtable_slot_1(probe.0, 0x248, world.get_tile_ptr(tile.0, tile.1)));
    live_support::destroy_standalone_keeper(probe);
    let Some(live_habitat) = live_habitat.filter(|&h| h != 0) else {
        failures.push("the zoo has no habitat tile".to_string());
        return finish_test(test_name, failures, failure_log);
    };

    let (b1, b2, b3) = (blocking_habitat(), blocking_habitat(), blocking_habitat());
    let cases: Vec<(&str, Vec<u32>)> = vec![
        ("empty", vec![]),
        ("one blocking", vec![b1]),
        ("three blocking", vec![b1, b2, b3]),
        ("live then blocking", vec![live_habitat, b1]),
        ("blocking, live, blocking", vec![b1, live_habitat, b2]),
    ];
    let shared = vec![keeper_type, live_habitat, b1, b2, b3];
    ARMS_INTERCEPT.store(1, Ordering::SeqCst);
    for (label, habitats) in &cases {
        for method in ["doConditionals", "checkAssignedHabitats"] {
            let fixtures = [live_support::build_standalone_keeper(keeper_type), live_support::build_standalone_keeper_port(keeper_type), live_support::build_standalone_keeper(keeper_type)];
            let mut outcomes = Vec::new();
            for (pole, keeper) in fixtures.iter().enumerate() {
                let sentinel: u32 = get_from_memory(keeper.0 + unit_offsets::ASSIGNED_HABITATS);
                for &habitat in habitats {
                    crate::vanilla_list::push_back(sentinel, habitat);
                }
                ARMS_LOG.lock().unwrap().clear();
                match (method, pole) {
                    ("doConditionals", 0) => real::do_conditionals(keeper.0),
                    ("doConditionals", 1) => keeper.do_conditionals(),
                    ("doConditionals", _) => unsafe { openzt_detour::generated::ztkeeper::DO_CONDITIONALS.hooked()(keeper.0 as *const u32) },
                    (_, 0) => real::check_assigned_habitats(keeper.0),
                    (_, 1) => keeper.check_assigned_habitats(),
                    _ => unsafe { openzt_detour::generated::ztkeeper::CHECK_ASSIGNED_HABITATS.hooked()(keeper.0 as *const u32) },
                }
                outcomes.push((assigned_payloads(keeper.0), std::mem::take(&mut *ARMS_LOG.lock().unwrap())));
            }
            let case = format!("{method}, {label}");
            let expected_staff = usize::from(method == "doConditionals");
            if outcomes[0].1.len() != expected_staff {
                failures.push(format!("{case}: real ran ZTStaff::doConditionals {} time(s), expected {expected_staff}", outcomes[0].1.len()));
            }
            if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                failures.push(format!("{case}: real {:x?}, port {:x?}, hooked {:x?}", outcomes[0], outcomes[1], outcomes[2]));
            }
            if outcomes[0].0.iter().any(|&h| h == b1 || h == b2 || h == b3) {
                failures.push(format!("{case}: a blocking habitat survived: {:x?}", outcomes[0].0));
            }
            compare_memory(&case, fixtures[0], fixtures[1], &shared, &mut failures);
            compare_memory(&format!("{case} (hooked)"), fixtures[0], fixtures[2], &shared, &mut failures);
            for keeper in fixtures {
                live_support::destroy_standalone_keeper(keeper);
            }
        }
    }
    ARMS_INTERCEPT.store(0, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}

/// `ZTKEEPER_REMOVE_ARMS_LIVE`: the gate-closing arm of `notifyHabitatNotComing` and the show arm of `removeFromMap`.
/// A fake gate pointer is closed through a recording `ZTFence::closeGate` hook (cleared afterwards, kept when the
/// keep-gate flag's low byte is set); the show arm runs against a standalone `ZTShowInfo` registered in the show store,
/// with the keeper's id seeded in the show's unit list and `ZTShow::abortShow` recorded. Real, port and hooked
/// addresses must agree on both logs, the list contents left behind and the keeper memory.
pub(crate) fn run_ztkeeper_remove_arms_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::ztshow::{find_or_insert_pending_script_node, get_num_units, live_support as ztshow_live_support};
    let test_name = "ZTKEEPER_REMOVE_ARMS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(message) = install_arms_hooks() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let Some(keeper_type) = live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    const GATE: u32 = 0x1000;
    const SHOW_ID: u16 = 0x7e02;
    const KEEPER_ID: u32 = 0x7777;
    ARMS_INTERCEPT.store(1, Ordering::SeqCst);
    let shared = vec![keeper_type];
    let build = |pole: usize| if pole == 1 { live_support::build_standalone_keeper_port(keeper_type) } else { live_support::build_standalone_keeper(keeper_type) };

    // Gate arm of notifyHabitatNotComing, with the target tile left null (no habitat counter to touch).
    for keep_gate in [0u32, 1, 0x100, 0xff] {
        let mut outcomes = Vec::new();
        let fixtures: Vec<ZTKeeper> = (0..3).map(build).collect();
        for (pole, keeper) in fixtures.iter().enumerate() {
            save_to_memory(keeper.0 + unit_offsets::GATE, GATE);
            ARMS_LOG.lock().unwrap().clear();
            match pole {
                0 => real::notify_habitat_not_coming(keeper.0, keep_gate),
                1 => keeper.notify_habitat_not_coming(keep_gate),
                _ => unsafe { openzt_detour::generated::ztkeeper::NOTIFY_HABITAT_NOT_COMING.hooked()(keeper.0 as *const u32, keep_gate) },
            }
            outcomes.push((get_from_memory::<u32>(keeper.0 + unit_offsets::GATE), std::mem::take(&mut *ARMS_LOG.lock().unwrap())));
        }
        let expect_closed = keep_gate & 0xff == 0;
        let expected = if expect_closed { (0, vec![("close_gate", GATE)]) } else { (GATE, vec![]) };
        for (pole, outcome) in outcomes.iter().enumerate() {
            if *outcome != expected {
                failures.push(format!("gate arm, keep_gate {keep_gate:#x}, pole {pole}: {outcome:x?}, expected {expected:x?}"));
            }
        }
        compare_memory(&format!("gate arm {keep_gate:#x}"), fixtures[0], fixtures[1], &shared, &mut failures);
        for keeper in fixtures {
            live_support::destroy_standalone_keeper(keeper);
        }
    }

    // Show arm of removeFromMap.
    let probe = build(0);
    let entity_type: u32 = get_from_memory(probe.0 + unit_offsets::ENTITY_TYPE);
    let type_id = call_vtable_slot_0(entity_type, 0x20);
    live_support::destroy_standalone_keeper(probe);
    for param in [false, true] {
        for mark_142 in [0u8, 1] {
            let fixtures: Vec<ZTKeeper> = (0..3).map(build).collect();
            let mut outcomes = Vec::new();
            for (pole, keeper) in fixtures.iter().enumerate() {
                let show_info = ztshow_live_support::build_standalone_show_info();
                crate::ztshow::mgr::ztshowmgr::lock_show_store().registered_shows.insert(SHOW_ID, show_info);
                let (node, _) = find_or_insert_pending_script_node(show_info, type_id);
                let sentinel: u32 = get_from_memory(node + 0x18);
                let entry = unsafe { openzt_detour::generated::standalone::OPERATOR_NEW.original()(0xc) } as u32;
                let prev: u32 = get_from_memory(sentinel + 4);
                save_to_memory(entry, sentinel);
                save_to_memory(entry + 4, prev);
                save_to_memory(entry + 8, KEEPER_ID);
                save_to_memory(prev, entry);
                save_to_memory(sentinel + 4, entry);

                save_to_memory(keeper.0 + unit_offsets::SHOW_INFO_ID, SHOW_ID);
                save_to_memory(keeper.0 + unit_offsets::ENTITY_ID, KEEPER_ID);
                save_to_memory(keeper.0 + 0x142, mark_142);
                save_to_memory(keeper.0 + unit_offsets::GATE, GATE);
                ARMS_LOG.lock().unwrap().clear();
                match pole {
                    0 => real::remove_from_map(keeper.0, param),
                    1 => keeper.remove_from_map(param),
                    _ => unsafe { REMOVE_FROM_MAP.hooked()(keeper.0 as *const u32, param) },
                }
                let log: Vec<(&'static str, u32)> = std::mem::take(&mut *ARMS_LOG.lock().unwrap())
                    .into_iter()
                    .map(|(kind, argument)| (kind, if kind == "abort_show" { argument - show_info } else { argument }))
                    .collect();
                outcomes.push((log, get_num_units(show_info, type_id)));
                crate::ztshow::mgr::ztshowmgr::lock_show_store().registered_shows.remove(&SHOW_ID);
            }
            let case = format!("removeFromMap(param {param}, +0x142 {mark_142})");
            let units_left = if param { 1 } else { 0 };
            let mut expected_log: Vec<(&'static str, u32)> = Vec::new();
            if !param {
                expected_log.push(("abort_show", 4));
            }
            if mark_142 == 0 && !param {
                expected_log.push(("close_gate", GATE));
            }
            for (pole, outcome) in outcomes.iter().enumerate() {
                if outcome.0 != expected_log || outcome.1 != units_left {
                    failures.push(format!("{case}, pole {pole}: {outcome:x?}, expected log {expected_log:x?}, {units_left} unit(s) left"));
                }
            }
            compare_memory(&case, fixtures[0], fixtures[1], &shared, &mut failures);
            compare_memory(&format!("{case} (hooked)"), fixtures[0], fixtures[2], &shared, &mut failures);
            for keeper in fixtures {
                live_support::destroy_standalone_keeper(keeper);
            }
        }
    }
    ARMS_INTERCEPT.store(0, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}
