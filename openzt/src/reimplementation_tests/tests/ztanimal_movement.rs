//! Live checks for the `ZTAnimal` movement, pathing and escape methods: `tileFilter`, `getTerrainCost`,
//! `setTerrainMode`, `findPlaceToRest`, `targetBuilding`, `stopEverything`, `invalidateTarget`, `breakEscapeFence`
//! and the three `setReachedEscape*Tile` handlers. Each case runs the real vanilla body, the Rust port and the hooked
//! address on identically prepared fixtures (animals built by the vanilla constructor over live animal types, standing
//! on a live animal's position) with the vtable slots these bodies dispatch through replaced by recorders, and compares
//! the answers, the recorded slot calls and the resulting state.
//!
//! Limits: the walking branch of `setTerrainMode` (a random tile, `stopEverything`, `goToDest`) and the found-building
//! branches of `findPlaceToRest`/`targetBuilding` run real vanilla path and grandstand code over a fixture, so they are
//! only reached when the live zoo offers a building; `breakEscapeFence` is run with equal escape tiles (direction `-1`)
//! so no live fence is broken.

use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex,
    },
};

use openzt_detour::generated::{
    bfentity::GET_TILE,
    bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION, GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR},
    poolalloc::ALLOCATE as POOLALLOC_ALLOCATE,
    standalone::OPERATOR_NEW,
    ztanimal as generated,
};

use super::ztanimal::{base, build_animal, live_animal_types, live_animals, Lcg};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztanimal::{animal_offsets, live_support::real, ZTAnimal},
};

/// What the scripted vtable slots record: `(slot, value)` in call order.
static CALLS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
static SWIMS: AtomicBool = AtomicBool::new(false);
static UNDERWATER: AtomicBool = AtomicBool::new(false);
static ONLY_UNDERWATER: AtomicBool = AtomicBool::new(false);
static HABITAT: AtomicU32 = AtomicU32::new(0);
static PREY_SEED: AtomicU32 = AtomicU32::new(0);
static PREY_ACCEPT: AtomicU32 = AtomicU32::new(0);

fn record(slot: u32, value: u32) {
    CALLS.lock().unwrap().push((slot, value));
}

/// The upper bytes are deliberately dirty: callers test `AL` only.
extern "thiscall" fn fake_get_swims(_this: u32) -> u32 {
    record(0xbc, 0);
    0xabcd_ef00 | SWIMS.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_get_underwater(_this: u32) -> u32 {
    record(0x16c, 0);
    0xabcd_ef00 | UNDERWATER.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_only_underwater(_this: u32) -> u32 {
    record(0x170, 0);
    0xabcd_ef00 | ONLY_UNDERWATER.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_habitat(_this: u32) -> u32 {
    record(0x24c, 0);
    HABITAT.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_force_anim(_this: u32, text: u32) -> u32 {
    let begin: u32 = get_from_memory(text);
    let end: u32 = get_from_memory(text + 4);
    let bytes = unsafe { std::slice::from_raw_parts(begin as *const u8, end.wrapping_sub(begin) as usize) };
    record(0x104, bytes.iter().fold(0x811c_9dc5u32, |hash, byte| (hash ^ *byte as u32).wrapping_mul(0x0100_0193)));
    0
}

/// Vtable `+0x300` (`checkPrey`): accepts an entity by a seeded hash so the best-prey selection is exercised.
extern "thiscall" fn fake_check_prey(_this: u32, entity: u32, force: u32) -> u32 {
    record(0x300, entity ^ (force & 1));
    let accept = entity != 0 && ((entity >> 3 ^ PREY_SEED.load(Ordering::SeqCst)).wrapping_mul(0x9e37_79b1) >> 28) < PREY_ACCEPT.load(Ordering::SeqCst);
    0xabcd_ef00 | accept as u32
}

extern "thiscall" fn fake_set_dest_tile(_this: u32, tile: u32) -> u32 {
    record(0x1a8, tile);
    0
}

extern "thiscall" fn fake_set_building_tile(_this: u32, tile: u32) -> u32 {
    record(0x2e4, tile);
    0
}

extern "thiscall" fn fake_set_goal(_this: u32, goal: u32, flag: u32) -> u32 {
    record(0x1d8, goal << 8 | (flag & 0xff));
    0
}

extern "thiscall" fn fake_clear_goal(_this: u32) {
    record(0x174, 0);
}

extern "thiscall" fn fake_abort_trick(_this: u32) {
    record(0x2c0, 0);
}

extern "thiscall" fn fake_is_doing_show(_this: u32) -> u32 {
    record(0x22c, 0);
    0
}

extern "thiscall" fn fake_clear_prey(_this: u32) {
    record(0x258, 0);
}

extern "thiscall" fn fake_stop_everything(_this: u32, a: u32, b: u32, c: u32) {
    record(0x198, (a & 0xff) << 16 | (b & 0xff) << 8 | (c & 0xff));
}

extern "thiscall" fn fake_update_animation(_this: u32, arg: u32) {
    record(0x40, arg);
}

/// Replaces the scripted slots of the animal's vtable with recorders. The copy is leaked.
fn install_scripted_vtable(animal: ZTAnimal) {
    let real_vtable: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    let scripted: [(u32, u32); 15] = [
        (0xbc, fake_get_swims as *const () as usize as u32),
        (0x16c, fake_get_underwater as *const () as usize as u32),
        (0x170, fake_only_underwater as *const () as usize as u32),
        (0x24c, fake_habitat as *const () as usize as u32),
        (0x104, fake_force_anim as *const () as usize as u32),
        (0x1a8, fake_set_dest_tile as *const () as usize as u32),
        (0x2e4, fake_set_building_tile as *const () as usize as u32),
        (0x1d8, fake_set_goal as *const () as usize as u32),
        (0x174, fake_clear_goal as *const () as usize as u32),
        (0x2c0, fake_abort_trick as *const () as usize as u32),
        (0x22c, fake_is_doing_show as *const () as usize as u32),
        (0x258, fake_clear_prey as *const () as usize as u32),
        (0x198, fake_stop_everything as *const () as usize as u32),
        (0x40, fake_update_animation as *const () as usize as u32),
        (0x300, fake_check_prey as *const () as usize as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

fn own_tile(entity: u32) -> u32 {
    unsafe { GET_TILE.original()(entity as *const u32) as u32 }
}

/// The tile's neighbours that exist.
fn neighbours(tile: u32) -> Vec<u32> {
    let world: u32 = get_from_memory(base() + 0x0023_8040);
    (0..8u32)
        .map(|direction| unsafe { BFMAP_GET_NEIGHBOR.original()((world + 8) as *const u32, tile as *const u32, direction) })
        .filter(|&neighbour| neighbour != 0)
        .collect()
}

/// Puts the fixture on `live`'s position, so its own tile is the live animal's.
fn stand_on(animal: ZTAnimal, live: u32) {
    for offset in [0x114u32, 0x118, 0x11c] {
        save_to_memory(animal.0 + offset, get_from_memory::<u32>(live + offset));
    }
}

/// Gives the fixture its own pool block as behaviour name, since the stop paths free the name through vanilla's pool.
fn give_name(animal: ZTAnimal) {
    let buffer = unsafe { POOLALLOC_ALLOCATE.original()(16) } as u32;
    for (i, byte) in b"bIdle".iter().enumerate() {
        save_to_memory(buffer + i as u32, *byte);
    }
    save_to_memory(animal.0 + 0x54, buffer);
    save_to_memory(animal.0 + 0x58, buffer + 5);
    save_to_memory(animal.0 + 0x5c, buffer + 16);
}

const SNAPSHOT_RANGES: [(u32, u32); 5] = [(0x150, 0x154), (0x170, 0x180), (0x210, 0x260), (0x290, 0x384), (0x388, 0x3b4)];

/// Which animal offset the `index`th snapshot byte is (or a record byte past the animal's own).
fn snapshot_position(index: usize) -> String {
    let mut remaining = index as u32;
    for (start, end) in SNAPSHOT_RANGES {
        if remaining < end - start {
            return format!("+{:#x}", start + remaining);
        }
        remaining -= end - start;
    }
    format!("escape record byte {}", remaining.saturating_sub(1))
}

/// The state words of an animal, with the pool pointers left out and the escape record reduced to its contents.
fn snapshot(animal: ZTAnimal) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (start, end) in SNAPSHOT_RANGES {
        for offset in start..end {
            bytes.push(get_from_memory::<u8>(animal.0 + offset));
        }
    }
    let record: u32 = get_from_memory(animal.0 + animal_offsets::ESCAPE_RECORD);
    bytes.push((record != 0) as u8);
    if record != 0 {
        for offset in 0..0x10u32 {
            // Offsets 4 and 8 hold live tile pointers shared by all poles.
            bytes.push(get_from_memory::<u8>(record + offset));
        }
    }
    bytes
}

/// Runs `body` (pole 0 real, 1 port, 2 hooked) on identically prepared fixtures and compares the answers, the recorded
/// slot calls and the state.
fn poles(ty: u32, label: &str, prepare: &dyn Fn(ZTAnimal), body: &dyn Fn(ZTAnimal, usize) -> i64, failures: &mut Vec<String>) {
    let mut logs = Vec::new();
    let mut snapshots = Vec::new();
    let mut answers = Vec::new();
    for pole in 0..3usize {
        let animal = build_animal(ty);
        install_scripted_vtable(animal);
        prepare(animal);
        CALLS.lock().unwrap().clear();
        answers.push(body(animal, pole));
        logs.push(CALLS.lock().unwrap().clone());
        snapshots.push(snapshot(animal));
    }
    if answers[0] != answers[1] || answers[0] != answers[2] {
        failures.push(format!("{label}: answer real {:#x}, port {:#x}, hooked {:#x}", answers[0], answers[1], answers[2]));
    }
    if logs[0] != logs[1] || logs[0] != logs[2] {
        failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
    }
    for (pole, name) in [(1usize, "port"), (2, "hooked")] {
        if snapshots[0] != snapshots[pole] {
            let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
            failures.push(format!("{label}: state differs from real for {name} (first differing byte {:?})", first.map(snapshot_position)));
        }
    }
}

/// A scripted, fixture-ready animal type set and a live position source, or `None` with a failure recorded.
fn fixture_inputs(failures: &mut Vec<String>) -> Option<(Vec<u32>, Vec<u32>)> {
    let types = live_animal_types(4);
    let live = live_animals();
    if types.is_empty() || live.is_empty() {
        failures.push("the live world holds no animal type or animal".to_string());
        return None;
    }
    Some((types, live))
}

fn random_bool(rng: &mut Lcg) -> bool {
    rng.next() & 1 == 1
}

/// `ZTANIMAL_MOVEMENT_TILES_LIVE`: `tileFilter` and `getTerrainCost` over live animals and their neighbouring tiles, and
/// over fixtures standing on a live animal's tile with random terrain mode, goal, counter and scripted slot answers.
pub(crate) fn run_ztanimal_movement_tiles_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_MOVEMENT_TILES_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };

    // The real live animals, answered by the three poles as they stand.
    for &animal in live.iter().take(24) {
        let tile = own_tile(animal);
        if tile == 0 {
            continue;
        }
        let mut tiles = neighbours(tile);
        tiles.push(tile);
        for &target in &tiles {
            for flag in [false, true] {
                let real_answer = real::tile_filter(animal, target, flag);
                let port_answer = ZTAnimal(animal).tile_filter(target, flag);
                let hooked_answer = unsafe { generated::TILE_FILTER.hooked()(animal as *const c_void, target as *const c_void, flag) };
                if real_answer != port_answer || real_answer != hooked_answer {
                    failures.push(format!("tileFilter live animal {animal:#x} tile {target:#x} flag {flag}: real {real_answer}, port {port_answer}, hooked {hooked_answer}"));
                }
            }
            let real_cost = real::get_terrain_cost(animal, target);
            let port_cost = ZTAnimal(animal).get_terrain_cost(target);
            let hooked_cost = unsafe { generated::GET_TERRAIN_COST.hooked()(animal as *const u32, target as *const u32) } as i32;
            if real_cost != port_cost || real_cost != hooked_cost {
                failures.push(format!("getTerrainCost live animal {animal:#x} tile {target:#x}: real {real_cost}, port {port_cost}, hooked {hooked_cost}"));
            }
        }
        if failures.len() > 20 {
            return finish_test(test_name, failures, failure_log);
        }
    }

    let ids: Vec<u32> = live.iter().map(|&animal| get_from_memory::<u32>(animal + 0x124)).collect();
    let mut rng = Lcg(0x71e5);
    for &ty in &types {
        for round in 0..300 {
            let source = live[(rng.next() as usize) % live.len()];
            let source_tile = own_tile(source);
            if source_tile == 0 {
                continue;
            }
            let mut tiles = neighbours(source_tile);
            tiles.push(source_tile);
            let target = tiles[(rng.next() as usize) % tiles.len()];
            let mode = (rng.next() % 3) as i32;
            let goal = [0i32, 0x74, 0x85, 0x86, 7][(rng.next() as usize) % 5];
            let goal_target = if random_bool(&mut rng) { ids[(rng.next() as usize) % ids.len()] } else { 0 };
            let counter = [0i32, 3, 4, 9][(rng.next() as usize) % 4];
            let escape_flag = random_bool(&mut rng);
            let (swims, underwater, only_underwater) = (random_bool(&mut rng), random_bool(&mut rng), random_bool(&mut rng));
            let flag = random_bool(&mut rng);
            let prepare = |animal: ZTAnimal| {
                SWIMS.store(swims, Ordering::SeqCst);
                UNDERWATER.store(underwater, Ordering::SeqCst);
                ONLY_UNDERWATER.store(only_underwater, Ordering::SeqCst);
                stand_on(animal, source);
                save_to_memory(animal.0 + animal_offsets::ACTIVITY_MODE, mode);
                save_to_memory(animal.0 + 0x170, goal);
                save_to_memory(animal.0 + 0x178, goal_target);
                save_to_memory(animal.0 + 0x150, counter);
                save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, escape_flag as u8);
            };
            let label = format!("round {round} mode {mode} goal {goal:#x} target-id {goal_target} counter {counter} escape {escape_flag} swims {swims} underwater {underwater} only-underwater {only_underwater}");
            poles(
                ty,
                &format!("tileFilter {label} flag {flag}"),
                &prepare,
                &|animal, pole| unsafe {
                    (match pole {
                        0 => real::tile_filter(animal.0, target, flag),
                        1 => animal.tile_filter(target, flag),
                        _ => generated::TILE_FILTER.hooked()(animal.0 as *const c_void, target as *const c_void, flag),
                    }) as i64
                },
                &mut failures,
            );
            poles(
                ty,
                &format!("getTerrainCost {label}"),
                &prepare,
                &|animal, pole| unsafe {
                    (match pole {
                        0 => real::get_terrain_cost(animal.0, target),
                        1 => animal.get_terrain_cost(target),
                        _ => generated::GET_TERRAIN_COST.hooked()(animal.0 as *const u32, target as *const u32) as i32,
                    }) as i64
                },
                &mut failures,
            );
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_MOVEMENT_MODE_LIVE`: `setTerrainMode` over random current modes, requested modes (including out-of-range
/// ones), the underwater-only answer and a null or absent habitat; the walking branch is not reached (see the module doc).
pub(crate) fn run_ztanimal_movement_mode_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_MOVEMENT_MODE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let mut rng = Lcg(0x40de);
    for &ty in &types {
        for round in 0..300 {
            let source = live[(rng.next() as usize) % live.len()];
            let current = (rng.next() % 4) as i32;
            let requested = [-1i32, 0, 1, 2, 3, 7][(rng.next() as usize) % 6];
            let only_underwater = random_bool(&mut rng);
            // With the walk flag clear no random tile is requested; with it set only a null habitat avoids the walk.
            let flag = random_bool(&mut rng) as i8;
            let habitat = if flag != 0 { 0 } else { [0u32, source][(rng.next() as usize) % 2] };
            let prepare = |animal: ZTAnimal| {
                ONLY_UNDERWATER.store(only_underwater, Ordering::SeqCst);
                HABITAT.store(habitat, Ordering::SeqCst);
                stand_on(animal, source);
                save_to_memory(animal.0 + animal_offsets::ACTIVITY_MODE, current);
            };
            poles(
                ty,
                &format!("setTerrainMode round {round} current {current} requested {requested} only-underwater {only_underwater} flag {flag} habitat {habitat:#x}"),
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::set_terrain_mode(animal.0, requested, flag),
                        1 => animal.set_terrain_mode(requested, flag),
                        _ => generated::SET_TERRAIN_MODE.hooked()(animal.0 as *const u32, requested, flag),
                    }
                    0
                },
                &mut failures,
            );
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_MOVEMENT_ESCAPE_LIVE`: the three `setReachedEscape*Tile` handlers and `breakEscapeFence` over every escape
/// kind and every reached/broken byte combination.
pub(crate) fn run_ztanimal_movement_escape_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_MOVEMENT_ESCAPE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let source = live[0];
    let tile = own_tile(source);
    if tile == 0 {
        failures.push("the first live animal has no tile".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let direction = unsafe { BFMAP_GET_DIRECTION.original()(tile as i32, tile as i32) };
    if direction != -1 {
        failures.push(format!("getDirection of a tile with itself is {direction}, not -1; the breakEscapeFence cases would break a live fence"));
        return finish_test(test_name, failures, failure_log);
    }
    for &ty in &types {
        for kind in 0..=5i32 {
            for (reached, broken) in [(0u8, 0u8), (1, 0), (0, 1), (1, 1)] {
                let prepare = |animal: ZTAnimal| {
                    stand_on(animal, source);
                    let record = unsafe { OPERATOR_NEW.original()(0x10) } as u32;
                    unsafe { std::ptr::write_bytes(record as *mut u8, 0, 0x10) };
                    save_to_memory(record + 1, reached);
                    save_to_memory(record + 3, broken);
                    save_to_memory(record + 4, tile);
                    save_to_memory(record + 8, tile);
                    save_to_memory(record + 0xc, kind);
                    save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, record);
                };
                let label = format!("kind {kind} reached {reached} broken {broken}");
                poles(
                    ty,
                    &format!("setReachedEscapeTile {label}"),
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::set_reached_escape_tile(animal.0),
                            1 => animal.set_reached_escape_tile(),
                            _ => generated::SET_REACHED_ESCAPE_TILE.hooked()(animal.0 as *const c_void),
                        }
                        0
                    },
                    &mut failures,
                );
                poles(
                    ty,
                    &format!("setReachedEscapeTransitionTile {label}"),
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::set_reached_escape_transition_tile(animal.0),
                            1 => animal.set_reached_escape_transition_tile(),
                            _ => generated::SET_REACHED_ESCAPE_TRANSITION_TILE.hooked()(animal.0 as i32),
                        }
                        0
                    },
                    &mut failures,
                );
                poles(
                    ty,
                    &format!("setReachedEscapeDestinationTile {label}"),
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::set_reached_escape_destination_tile(animal.0),
                            1 => animal.set_reached_escape_destination_tile(),
                            _ => generated::SET_REACHED_ESCAPE_DESTINATION_TILE.hooked()(animal.0 as *const c_void),
                        }
                        0
                    },
                    &mut failures,
                );
                poles(
                    ty,
                    &format!("breakEscapeFence {label}"),
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::break_escape_fence(animal.0),
                            1 => animal.break_escape_fence(),
                            _ => generated::BREAK_ESCAPE_FENCE.hooked()(animal.0 as *const u32),
                        }
                        0
                    },
                    &mut failures,
                );
                if failures.len() > 20 {
                    return finish_test(test_name, failures, failure_log);
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_MOVEMENT_TARGETS_LIVE`: `stopEverything`, `invalidateTarget`, `findPlaceToRest` and `targetBuilding` on
/// fixtures standing on live positions, with and without an escape record and the target flag.
pub(crate) fn run_ztanimal_movement_targets_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_MOVEMENT_TARGETS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let mut rng = Lcg(0x5a17);
    for &ty in &types {
        for round in 0..120 {
            let source = live[(rng.next() as usize) % live.len()];
            let has_record = random_bool(&mut rng);
            let has_flag = random_bool(&mut rng);
            let target_flag = random_bool(&mut rng);
            let (a, b, c) = (random_bool(&mut rng), random_bool(&mut rng), random_bool(&mut rng));
            let nearest = random_bool(&mut rng);
            let prepare = |animal: ZTAnimal| {
                stand_on(animal, source);
                give_name(animal);
                save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, has_flag as u8);
                save_to_memory(animal.0 + 0x3b0, target_flag as u8);
                save_to_memory(animal.0 + 0x3a8, 1u8);
                if has_record {
                    let record = unsafe { OPERATOR_NEW.original()(0x10) } as u32;
                    unsafe { std::ptr::write_bytes(record as *mut u8, 0, 0x10) };
                    save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, record);
                }
            };
            let label = format!("round {round} record {has_record} escape-flag {has_flag} target-flag {target_flag}");
            poles(
                ty,
                &format!("stopEverything {label} args {a}/{b}/{c}"),
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::stop_everything(animal.0, a, b, c),
                        1 => animal.stop_everything(a, b, c),
                        _ => generated::STOP_EVERYTHING.hooked()(animal.0 as *const u32, a, b, c),
                    }
                    0
                },
                &mut failures,
            );
            poles(
                ty,
                &format!("invalidateTarget {label}"),
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::invalidate_target(animal.0),
                        1 => animal.invalidate_target(),
                        _ => generated::INVALIDATE_TARGET.hooked()(animal.0 as *const c_void),
                    }
                    0
                },
                &mut failures,
            );
            poles(
                ty,
                &format!("findPlaceToRest {label} nearest {nearest}"),
                &prepare,
                &|animal, pole| unsafe {
                    (match pole {
                        0 => real::find_place_to_rest(animal.0, nearest),
                        1 => animal.find_place_to_rest(nearest),
                        _ => (generated::FIND_PLACE_TO_REST.hooked()(animal.0 as *const u32, nearest) as u8) != 0,
                    }) as i64
                },
                &mut failures,
            );
            poles(
                ty,
                &format!("targetBuilding {label} nearest {nearest}"),
                &prepare,
                &|animal, pole| unsafe {
                    (match pole {
                        0 => real::target_building(animal.0, nearest),
                        1 => animal.target_building(nearest),
                        _ => generated::TARGET_BUILDING.hooked()(animal.0 as *const u32, nearest),
                    }) as i64
                },
                &mut failures,
            );
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_MOVEMENT_PREY_LIVE`: `findPrey` on fixtures standing on live positions, with a scripted `checkPrey` that
/// accepts entities by a seeded hash, over held-prey ids, the target flag, the forced flag and a null or live habitat.
pub(crate) fn run_ztanimal_movement_prey_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_MOVEMENT_PREY_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let mut rng = Lcg(0xfa11);
    for &ty in &types {
        for round in 0..60 {
            let source = live[(rng.next() as usize) % live.len()];
            let held = if random_bool(&mut rng) { get_from_memory::<u32>(live[(rng.next() as usize) % live.len()] + 0x124) } else { 0 };
            let seed = rng.next();
            let accept = [0u32, 1, 4, 8, 16][(rng.next() as usize) % 5];
            let target_flag = random_bool(&mut rng);
            let force = random_bool(&mut rng);
            let habitat_answer = if random_bool(&mut rng) { 0 } else { crate::ztkeeper::call_slot_0(source, 0x24c) };
            let prepare = |animal: ZTAnimal| {
                PREY_SEED.store(seed, Ordering::SeqCst);
                PREY_ACCEPT.store(accept, Ordering::SeqCst);
                HABITAT.store(habitat_answer, Ordering::SeqCst);
                stand_on(animal, source);
                save_to_memory(animal.0 + animal_offsets::PREY_ID, held);
                save_to_memory(animal.0 + 0x3b0, target_flag as u8);
            };
            poles(
                ty,
                &format!("findPrey round {round} held {held} seed {seed:#x} accept {accept} target-flag {target_flag} force {force} habitat {habitat_answer:#x}"),
                &prepare,
                &|animal, pole| unsafe {
                    (match pole {
                        0 => real::find_prey(animal.0, force),
                        1 => animal.find_prey(force),
                        _ => generated::FIND_PREY.hooked()(animal.0 as *const u32, force),
                    }) as i64
                },
                &mut failures,
            );
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
