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
    let scripted: [(u32, u32); 17] = [
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
        (0x1f8, fake_set_destination as *const () as usize as u32),
        (0x1a0, fake_set_position as *const () as usize as u32),
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

/// A fake fence type's `isCastClass`: the tank-wall tag answers the byte at type `+0x1a0`, every other tag (the fence
/// tag) answers true. The upper bytes are dirty on purpose.
extern "thiscall" fn fence_type_is_cast(this: u32, tag: u32) -> u32 {
    if tag == base() + 0x0023_8720 {
        0xabcd_ef00 | get_from_memory::<u8>(this + 0x1a0) as u32
    } else {
        0xabcd_ef01
    }
}

/// The fake type's virtual `+0x30`, which `isZooGate` searches through: no name list.
extern "thiscall" fn fence_type_no_gate_list(_this: u32) -> u32 {
    0
}

/// The fence entity's virtual `+0x144` (show-gate predicate): the byte at entity `+0x5f0`.
extern "thiscall" fn fence_show_gate(this: u32) -> u32 {
    0xabcd_ef00 | get_from_memory::<u8>(this + 0x5f0) as u32
}

/// A fake fence entity with a random type: the fence-type flags, name, height, strength, open word, electrified
/// state, show-gate answer and tank-wall cast and bytes `fencesOK` reads. The buffers are leaked.
fn make_random_fence(rng: &mut Lcg) -> u32 {
    let mut type_vtable = vec![0u32; 0x40];
    type_vtable[7] = fence_type_is_cast as *const () as usize as u32;
    type_vtable[12] = fence_type_no_gate_list as *const () as usize as u32;
    let type_vtable = Box::leak(type_vtable.into_boxed_slice());
    let name: &[u8] = if rng.next() % 4 == 0 { b"gate\0" } else { b"wall\0" };
    let name = Box::leak(name.to_vec().into_boxed_slice());
    let fence_type = Box::leak(vec![0u8; 0x200].into_boxed_slice()).as_mut_ptr() as u32;
    save_to_memory(fence_type, type_vtable.as_ptr() as u32);
    save_to_memory(fence_type + 0xa4, name.as_ptr() as u32);
    save_to_memory(fence_type + 0xa8, name.as_ptr() as u32 + 4);
    save_to_memory(fence_type + 0x64, [0i32, 5, 50][(rng.next() % 3) as usize]);
    save_to_memory(fence_type + 0x170, [0i32, 3][(rng.next() % 2) as usize]);
    for (offset, odds) in [(0x190u32, 5), (0x191, 5), (0x193, 2), (0x195, 2), (0x196, 2), (0x198, 2), (0x1a0, 2)] {
        save_to_memory(fence_type + offset, (rng.next() % odds == 0) as u8);
    }
    let mut entity_vtable = vec![0u32; 0x60];
    entity_vtable[0x144 / 4] = fence_show_gate as *const () as usize as u32;
    let entity_vtable = Box::leak(entity_vtable.into_boxed_slice());
    let fence = Box::leak(vec![0u8; 0x600].into_boxed_slice()).as_mut_ptr() as u32;
    save_to_memory(fence, entity_vtable.as_ptr() as u32);
    save_to_memory(fence + 0x128, fence_type);
    save_to_memory(fence + 0x15c, [0i32, 2, 9][(rng.next() % 3) as usize]);
    save_to_memory(fence + 0x160, [0i32, 1, 5][(rng.next() % 3) as usize]);
    save_to_memory(fence + 0x164, (rng.next() % 3 == 0) as u8);
    for offset in [0x464u32, 0x466, 0x5f0] {
        save_to_memory(fence + offset, (rng.next() % 2 == 0) as u8);
    }
    fence
}

/// `ZTANIMAL_FENCES_OK_LIVE`: `fencesOK` over null and live tiles in every direction, then over a copy of a live
/// animal's tile whose near edge, and whose real neighbour's far edge, hold random fake fences, with random escape
/// state, show goal and animal-type climb/jump/strength/height fields (restored afterwards). Real against port and the
/// hooked address: answers and both output bytes.
pub(crate) fn run_ztanimal_fences_ok_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_FENCES_OK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let run_poles = |animal: ZTAnimal, pole: usize, tile: u32, direction: i32| -> i64 {
        let (mut out_a, mut out_b) = (0xaau8, 0xaau8);
        let (a, b) = (&mut out_a as *mut u8 as u32, &mut out_b as *mut u8 as u32);
        let result = match pole {
            0 => real::fences_ok(animal.0, tile, direction, a, b),
            1 => animal.fences_ok(tile, direction, a, b),
            _ => unsafe { generated::FENCES_OK.hooked()(animal.0 as *const u32, tile as *const u32, direction as u32, a as *const bool, b as *const bool) & 0xff != 0 },
        };
        result as i64 | (get_from_memory::<u8>(a) as i64) << 8 | (get_from_memory::<u8>(b) as i64) << 16
    };
    for &animal_address in live.iter().take(12) {
        let tile = own_tile(animal_address);
        for direction in (0i32..8).chain([-1]) {
            let results: Vec<i64> = (0..3).map(|pole| run_poles(ZTAnimal(animal_address), pole, tile, direction)).collect();
            if results[0] != results[1] || results[0] != results[2] {
                failures.push(format!("live animal {animal_address:#x} direction {direction}: real {:#x}, port {:#x}, hooked {:#x}", results[0], results[1], results[2]));
            }
        }
    }
    let null_results: Vec<i64> = (0..3).map(|pole| run_poles(ZTAnimal(live[0]), pole, 0, 2)).collect();
    if null_results.iter().any(|&result| result != null_results[0]) {
        failures.push(format!("null tile: {null_results:x?}"));
    }

    let mut rng = Lcg(0xfe7c);
    let (mut blocked, mut passed, mut plain_fence_marked) = (0, 0, 0);
    for &ty in &types {
        for round in 0..400 {
            let source = live[(rng.next() as usize) % live.len()];
            let live_tile = own_tile(source);
            if live_tile == 0 {
                continue;
            }
            let copy = Box::leak(vec![0u8; 0x8c].into_boxed_slice());
            unsafe { std::ptr::copy_nonoverlapping(live_tile as *const u8, copy.as_mut_ptr(), 0x8c) };
            let copy_address = copy.as_ptr() as u32;
            let direction = (rng.next() % 8) as i32;
            let far_edge = (direction - 4) & 7;
            let world: u32 = get_from_memory(base() + 0x0023_8040);
            let neighbour = unsafe { BFMAP_GET_NEIGHBOR.original()((world + 8) as *const u32, copy_address as *const u32, direction as u32) };
            if neighbour == 0 {
                continue;
            }
            let near_slot = copy_address + 0x14 + (direction as u32 / 2) * 4;
            let far_slot = neighbour + 0x14 + (far_edge as u32 / 2) * 4;
            let (near_saved, far_saved): (u32, u32) = (get_from_memory(near_slot), get_from_memory(far_slot));
            let near_fence = if rng.next() % 4 != 0 { make_random_fence(&mut rng) } else { 0 };
            let far_fence = if rng.next() % 4 != 0 { make_random_fence(&mut rng) } else { 0 };
            save_to_memory(near_slot, near_fence);
            save_to_memory(far_slot, far_fence);
            let saved_flags: [u8; 2] = [get_from_memory(ty + 0x3cc), get_from_memory(ty + 0x3cd)];
            let (strength_saved, height_saved): (i32, i32) = (get_from_memory(ty + 0x3b4), get_from_memory(ty + 0x3dc));
            save_to_memory(ty + 0x3cc, (rng.next() % 2 == 0) as u8);
            save_to_memory(ty + 0x3cd, (rng.next() % 2 == 0) as u8);
            save_to_memory(ty + 0x3b4, [0i32, 3, 9][(rng.next() % 3) as usize]);
            save_to_memory(ty + 0x3dc, [0i32, 5, 80][(rng.next() % 3) as usize]);
            let escaping = rng.next() % 2 == 0;
            let goal = [0i32, 0x85, 0x86][(rng.next() % 3) as usize];
            let prepare = |animal: ZTAnimal| {
                stand_on(animal, source);
                save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, escaping as u8);
                save_to_memory(animal.0 + 0x170, goal);
            };
            poles(
                ty,
                &format!("fencesOK round {round} direction {direction} escaping {escaping} goal {goal:#x} near {near_fence:#x} far {far_fence:#x}"),
                &prepare,
                &|animal, pole| run_poles(animal, pole, copy_address, direction),
                &mut failures,
            );
            let probe = build_animal(ty);
            prepare(probe);
            let probe_result = run_poles(probe, 0, copy_address, direction);
            if probe_result & 1 == 0 {
                blocked += 1;
            } else {
                passed += 1;
            }
            // `out_b` starts at 0xaa; the plain-fence marker is the byte written to 1.
            plain_fence_marked += usize::from((probe_result >> 16) & 0xff == 1);
            save_to_memory(ty + 0x3cc, saved_flags[0]);
            save_to_memory(ty + 0x3cd, saved_flags[1]);
            save_to_memory(ty + 0x3b4, strength_saved);
            save_to_memory(ty + 0x3dc, height_saved);
            save_to_memory(near_slot, near_saved);
            save_to_memory(far_slot, far_saved);
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    if blocked == 0 || passed == 0 {
        failures.push(format!("the random fence cases only reached one answer (blocked {blocked}, passed {passed})"));
    }
    if plain_fence_marked == 0 {
        failures.push("coverage: no random fence case made the real body mark out_b (the plain-fence case)".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// Vtable `+0x1f8` (the destination setter `pickRandomDest` ends with): records a hash of the three position words.
extern "thiscall" fn fake_set_destination(_this: u32, position: u32) -> u32 {
    let words: [u32; 3] = [get_from_memory(position), get_from_memory(position + 4), get_from_memory(position + 8)];
    record(0x1f8, words[0] ^ words[1].rotate_left(7) ^ words[2].rotate_left(14));
    0xabcd_ef00
}

/// Vtable `+0x1a0`: the escape destination hand-off.
extern "thiscall" fn fake_set_position(_this: u32, position: u32) -> u32 {
    let words: [u32; 3] = [get_from_memory(position), get_from_memory(position + 4), get_from_memory(position + 8)];
    record(0x1a0, words[0] ^ words[1].rotate_left(7) ^ words[2].rotate_left(14));
    0xabcd_ef00
}

/// `ZTANIMAL_PICK_RANDOM_DEST_LIVE`: `pickRandomDest` on fixtures standing on live animals' tiles, in the live
/// animal's own habitat, over random pending directions, activity modes, swim/underwater answers and RNG seeds; the
/// position written, the answer, the destination-setter call and the animal's state agree between real, port and the
/// hooked address. A round is skipped when the escape branch would run (`evaluateEscape` runs the real pathing over a
/// fixture) or when a failed pick would call `setTerrainMode` into its walking branch.
pub(crate) fn run_ztanimal_pick_random_dest_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_PICK_RANDOM_DEST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let rng_address = base() + 0x0023_8060;
    let mut rng = Lcg(0x5eed);
    let (mut ran, mut found, mut missed) = (0, 0, 0);
    for &ty in &types {
        for round in 0..300 {
            let source = live[(rng.next() as usize) % live.len()];
            let habitat = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(source) + 0x24c))(source) };
            if habitat == 0 || get_from_memory::<u8>(habitat + 0x2c) != 0 {
                continue;
            }
            let habitat_ref = unsafe { crate::util::ref_from_memory::<crate::zthabitat::ZTHabitat>(habitat) };
            let pending = rng.next() % 3 == 0;
            let direction: i32 = [-1i32, -1, 0, 2, 4, 6][(rng.next() as usize) % 6];
            let mode = [0i32, 1, 2, 5][(rng.next() as usize) % 4];
            let (swims, underwater) = (random_bool(&mut rng), random_bool(&mut rng));
            let seed = rng.next();
            let key = ty;
            let entry = unsafe {
                openzt_detour::generated::msvc_std_tree36::OPERATOR_INDEX.original()((habitat + 0x16c) as *const u32, &key as *const u32)
            } as u32;
            if !pending && get_from_memory::<u8>(entry) != 0 {
                continue;
            }
            // Dry run with the same seed: skip rounds that would end in `setTerrainMode`'s walking branch.
            save_to_memory(rng_address, seed);
            let predicted = if direction != -1 {
                1
            } else {
                match mode {
                    1 => habitat_ref.get_random_water_tile(),
                    2 => {
                        let land_roamer = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(habitat) + 0x20))(habitat) } & 0xff != 0;
                        if land_roamer { 1 } else { habitat_ref.get_random_land_tile() }
                    }
                    _ => habitat_ref.get_random_underwater_tile(),
                }
            };
            let may_set_mode = predicted == 0 && (matches!(mode, 1 | 2) || swims || underwater);
            if may_set_mode {
                continue;
            }
            let prepare = |animal: ZTAnimal| {
                SWIMS.store(swims, Ordering::SeqCst);
                UNDERWATER.store(underwater, Ordering::SeqCst);
                HABITAT.store(habitat, Ordering::SeqCst);
                stand_on(animal, source);
                save_to_memory(animal.0 + animal_offsets::ACTIVITY_MODE, mode);
                save_to_memory(animal.0 + 0x37c, direction);
                save_to_memory(animal.0 + 0x395, pending as u8);
                save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, 0u8);
                save_to_memory(rng_address, seed);
            };
            poles(
                ty,
                &format!("pickRandomDest round {round} pending {pending} direction {direction} mode {mode} swims {swims} underwater {underwater} seed {seed:#x}"),
                &prepare,
                &|animal, pole| {
                    let mut position = [0xaau32; 3];
                    let address = position.as_mut_ptr() as u32;
                    let result = match pole {
                        0 => real::pick_random_dest(animal.0, address),
                        1 => animal.pick_random_dest(address),
                        _ => unsafe { generated::PICK_RANDOM_DEST.hooked()(animal.0 as *const u32, address as *const glam::i32::IVec3) & 0xff != 0 },
                    };
                    result as i64 | ((position[0] ^ position[1].rotate_left(7) ^ position[2].rotate_left(14) ^ get_from_memory::<u32>(rng_address).rotate_left(21)) as i64) << 1
                },
                &mut failures,
            );
            ran += 1;
            if predicted != 0 {
                found += 1;
            } else {
                missed += 1;
            }
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    if ran == 0 || found == 0 {
        failures.push(format!("the comparison cases never found a destination (ran {ran}, found {found}, missed {missed})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// The unit's path list (`+0x154`): the `(x, y, z)` words of every node, in order.
fn path_list(animal: u32) -> Vec<[i32; 3]> {
    let sentinel: u32 = get_from_memory(animal + 0x154);
    let mut nodes = Vec::new();
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel && nodes.len() < 4096 {
        nodes.push([get_from_memory(node + 8), get_from_memory(node + 0xc), get_from_memory(node + 0x10)]);
        node = get_from_memory(node);
    }
    nodes
}

/// `ZTANIMAL_FEEDING_PATH_LIVE`: `getFeedingPath` on fixtures standing on live animals' positions, towards the grid
/// positions of other live animals, with and without the shortcut flag and over several RNG seeds (the shortcut roll
/// below and above `0x32` both occur). Each pole gets a fresh fixture because `getPath` answers by the unit's current path
/// state. Real, port and the hooked address must agree on the answer, the RNG state afterwards and the path list.
pub(crate) fn run_ztanimal_feeding_path_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_FEEDING_PATH_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let rng_address = base() + 0x0023_8060;
    let mut rng = Lcg(0xfeed);
    for &ty in types.iter().take(2) {
        for round in 0..60 {
            let source = live[(rng.next() as usize) % live.len()];
            let other = live[(rng.next() as usize) % live.len()];
            let habitat = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(source) + 0x24c))(source) };
            let mut scratch = [0i32; 3];
            let grid = unsafe { openzt_detour::generated::bfentity::GET_GRID_POS.hooked()(other as *const u32, scratch.as_mut_ptr()) } as u32;
            let destination: &'static [i32; 3] = Box::leak(Box::new([get_from_memory(grid), get_from_memory(grid + 4), get_from_memory(grid + 8)]));
            let destination_address = destination.as_ptr() as u32;
            let shortcut = random_bool(&mut rng);
            let seed = rng.next();
            let prepare = |animal: ZTAnimal| {
                HABITAT.store(habitat, Ordering::SeqCst);
                stand_on(animal, source);
                save_to_memory(rng_address, seed);
            };
            poles(
                ty,
                &format!("getFeedingPath round {round} destination {destination:?} shortcut {shortcut} seed {seed:#x}"),
                &prepare,
                &|animal, pole| {
                    let result = match pole {
                        0 => real::get_feeding_path(animal.0, destination_address, shortcut),
                        1 => animal.get_feeding_path(destination_address, shortcut),
                        _ => unsafe { generated::GET_FEEDING_PATH.hooked()(animal.0 as *const c_void, destination_address as *const i32, shortcut as u8) & 0xff != 0 },
                    };
                    let path = path_list(animal.0);
                    let hash = path.iter().flatten().fold(path.len() as u32, |hash, word| (hash ^ *word as u32).wrapping_mul(0x0100_0193));
                    result as i64 | ((hash ^ get_from_memory::<u32>(rng_address).rotate_left(11)) as i64) << 1 | (path.len() as i64) << 40
                },
                &mut failures,
            );
            if failures.len() > 10 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// The scripted answers of the AI manager's path search (`ZTAIMgr + 0x10`, first virtual): one `(found, path)` per call, in
/// call order; a call past the script fails.
static SEARCH_SCRIPT: Mutex<Vec<(bool, Vec<[i32; 3]>)>> = Mutex::new(Vec::new());
static SEARCH_NEXT: AtomicU32 = AtomicU32::new(0);
/// `ZTAIMgr`'s address while `ZTANIMAL_EVALUATE_ESCAPE_LIVE` runs, for the scripted search to write its result list into.
static SEARCH_AIMGR: AtomicU32 = AtomicU32::new(0);

/// The scripted search: records its arguments and the unit's "searching" byte, then publishes the scripted path as the
/// AI manager's result list (a leaked sentinel and nodes `{next, prev, x, y, z}`).
extern "thiscall" fn fake_path_search(_this: u32, from: u32, to: u32, unit: u32, flag: u32) -> u32 {
    let words = |position: u32| -> u32 {
        let [x, y, z]: [u32; 3] = [get_from_memory(position), get_from_memory(position + 4), get_from_memory(position + 8)];
        x ^ y.rotate_left(7) ^ z.rotate_left(14)
    };
    record(0x1000, words(from) ^ words(to).rotate_left(3) ^ (get_from_memory::<u8>(unit + 0x391) as u32) << 24 ^ flag << 25);
    let index = SEARCH_NEXT.fetch_add(1, Ordering::SeqCst) as usize;
    let (found, path) = SEARCH_SCRIPT.lock().unwrap().get(index).cloned().unwrap_or((false, Vec::new()));
    let sentinel = Box::leak(Box::new([0u32; 5])) as *mut [u32; 5] as u32;
    let mut previous = sentinel;
    for position in &path {
        let node = Box::leak(Box::new([0u32, previous, position[0] as u32, position[1] as u32, position[2] as u32])) as *mut [u32; 5] as u32;
        save_to_memory(previous, node);
        previous = node;
    }
    save_to_memory(previous, sentinel);
    save_to_memory(sentinel + 4, previous);
    save_to_memory(SEARCH_AIMGR.load(Ordering::SeqCst) + 0x80, sentinel);
    0xabcd_ef00 | found as u32
}

/// Swaps the path-search object's vtable for a copy with the scripted first virtual and restores both it and the result
/// list head on drop.
struct ScriptedSearch {
    pather: u32,
    vtable: u32,
    result_head: u32,
}

impl ScriptedSearch {
    fn install(aimgr: u32) -> Self {
        let pather = aimgr + 0x10;
        let vtable: u32 = get_from_memory(pather);
        let result_head: u32 = get_from_memory(aimgr + 0x80);
        let mut copy: Vec<u32> = (0..32u32).map(|slot| get_from_memory(vtable + slot * 4)).collect();
        copy[0] = fake_path_search as *const () as usize as u32;
        save_to_memory(pather, Box::leak(copy.into_boxed_slice()).as_ptr() as u32);
        SEARCH_AIMGR.store(aimgr, Ordering::SeqCst);
        Self { pather, vtable, result_head }
    }
}

impl Drop for ScriptedSearch {
    fn drop(&mut self) {
        save_to_memory(self.pather, self.vtable);
        save_to_memory(SEARCH_AIMGR.load(Ordering::SeqCst) + 0x80, self.result_head);
    }
}

/// `ZTANIMAL_EVALUATE_ESCAPE_LIVE`: `evaluateEscape` on fixtures standing on fenced tiles of the live map (in a habitat, as
/// vanilla's search requires), with an escape record (allocated through vanilla's `operator_new`) towards the tile across
/// the fence, over random record kinds. The AI manager's path search is scripted (a walk across the fenced edge with
/// random extra steps, then a second-search answer of none, two or three positions), since vanilla's search needs a
/// species that can escape. Real, port and the hooked address must agree on the path list left in the unit, the record's
/// bytes, the recorded search arguments, destination hand-off and slot calls, the unit's state (including the "searching"
/// byte) and the search object's scratch words.
pub(crate) fn run_ztanimal_evaluate_escape_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_EVALUATE_ESCAPE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let world: u32 = get_from_memory(base() + 0x0023_8040);
    let map = world + 8;
    let (width, height): (i32, i32) = (get_from_memory(map + 0x2c), get_from_memory(map + 0x30));
    let tiles: u32 = get_from_memory(map + 0x38);
    let aimgr: u32 = get_from_memory(base() + 0x0023_8098);
    let tile_position = |tile: u32| -> [i32; 3] { [get_from_memory(tile + 0x34), get_from_memory(tile + 0x38), get_from_memory(tile + 0x3c)] };
    let neighbour = |tile: u32, direction: u32| unsafe { BFMAP_GET_NEIGHBOR.original()(map as *const u32, tile as *const u32, direction) };
    // Every (tile, direction, tile across a fenced edge) of the live map whose starting tile lies in a habitat.
    let mut fenced: Vec<(u32, u32, u32)> = Vec::new();
    for index in 0..(width * height) as u32 {
        let tile = tiles + index * 0x8c;
        let tile_habitat = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(live[0]) + 0x248))(live[0], tile) };
        if tile_habitat == 0 {
            continue;
        }
        for direction in (0..8u32).step_by(2) {
            if get_from_memory::<u32>(tile + 0x14 + direction / 2 * 4) != 0 {
                let across = neighbour(tile, direction);
                if across != 0 {
                    fenced.push((tile, direction, across));
                }
            }
        }
    }
    if fenced.is_empty() {
        failures.push("the live map holds no fenced edge inside a habitat".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let search = ScriptedSearch::install(aimgr);
    let mut rng = Lcg(0xe5c4);
    let (mut ran, mut handed_off, mut with_kind, mut second_search) = (0, 0, 0, 0);
    for &ty in types.iter() {
        for round in 0..60 {
            let source = live[(rng.next() as usize) % live.len()];
            let habitat = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(source) + 0x24c))(source) };
            if habitat == 0 {
                continue;
            }
            let (from, direction, across) = fenced[(rng.next() as usize) % fenced.len()];
            let stand = [tile_position(from)[0] * 64 + 32, tile_position(from)[1] * 64 + 32, tile_position(from)[2]];
            let kind = (rng.next() % 5) as i32;
            // First search: optionally a lead-in step, the fenced crossing, then up to two more steps.
            let mut path: Vec<[i32; 3]> = Vec::new();
            if rng.next() % 3 == 0 {
                let lead = neighbour(from, (rng.next() % 4) * 2);
                if lead != 0 {
                    path.push(tile_position(lead));
                }
            }
            path.push(tile_position(from));
            path.push(tile_position(across));
            let mut tail = across;
            for _ in 0..rng.next() % 3 {
                let next = neighbour(tail, (rng.next() % 4) * 2);
                if next != 0 {
                    path.push(tile_position(next));
                    tail = next;
                }
            }
            let first_found = rng.next() % 8 != 0;
            let short_first = rng.next() % 10 == 0;
            let first = if short_first { vec![tile_position(from)] } else { path.clone() };
            let second_kind = rng.next() % 4;
            let second: (bool, Vec<[i32; 3]>) = match second_kind {
                0 => (false, Vec::new()),
                1 => (true, vec![tile_position(from), tile_position(across)]),
                2 => (true, vec![tile_position(from), tile_position(across), tile_position(across)]),
                _ => (true, vec![tile_position(across), tile_position(from)]),
            };
            let prepare = |animal: ZTAnimal| {
                HABITAT.store(habitat, Ordering::SeqCst);
                for (offset, word) in [0x114u32, 0x118, 0x11c].into_iter().zip(stand) {
                    save_to_memory(animal.0 + offset, word);
                }
                let record = unsafe { OPERATOR_NEW.original()(0x10) } as u32;
                for offset in 0..4u32 {
                    save_to_memory(record + offset, (offset == 0) as u8);
                }
                save_to_memory(record + 4, from);
                save_to_memory(record + 8, across);
                save_to_memory(record + 0xc, kind);
                save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, record);
                *SEARCH_SCRIPT.lock().unwrap() = vec![(first_found, first.clone()), second.clone()];
                SEARCH_NEXT.store(0, Ordering::SeqCst);
            };
            poles(
                ty,
                &format!("evaluateEscape round {round} from {from:#x} direction {direction} across {across:#x} kind {kind} second {second_kind} first found {first_found} short {short_first}"),
                &prepare,
                &|animal, pole| {
                    match pole {
                        0 => real::evaluate_escape(animal.0),
                        1 => animal.evaluate_escape(),
                        _ => unsafe { generated::EVALUATE_ESCAPE.hooked()(animal.0 as *const c_void) },
                    }
                    let path = path_list(animal.0);
                    let record: u32 = get_from_memory(animal.0 + animal_offsets::ESCAPE_RECORD);
                    let mut hash = path.iter().flatten().fold(path.len() as u32, |hash, word| (hash ^ *word as u32).wrapping_mul(0x0100_0193));
                    for offset in 0..0x10u32 {
                        hash = (hash ^ get_from_memory::<u8>(record + offset) as u32).wrapping_mul(0x0100_0193);
                    }
                    for offset in [0x18u32, 0x1c, 0x20, 0x5c] {
                        hash = (hash ^ get_from_memory::<u32>(aimgr + offset)).wrapping_mul(0x0100_0193);
                    }
                    path.len() as i64 | (hash as i64) << 8
                },
                &mut failures,
            );
            ran += 1;
            let calls = CALLS.lock().unwrap().clone();
            handed_off += calls.iter().any(|(slot, _)| *slot == 0x1a0) as u32;
            with_kind += (kind != 0 && calls.iter().any(|(slot, _)| *slot == 0x1a0)) as u32;
            second_search += (calls.iter().filter(|(slot, _)| *slot == 0x1000).count() == 2) as u32;
            if failures.len() > 10 {
                break;
            }
        }
    }
    drop(search);
    if ran == 0 || handed_off == 0 || second_search == 0 {
        failures.push(format!("the comparison cases did not cover the fence branches (ran {ran}, fence cases {handed_off}, with a kind {with_kind}, second searches {second_search})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_PICK_RANDOM_DEST_ESCAPE_LIVE`: `pickRandomDest`'s escape branch. The habitat's escape tree entry for the fixture's
/// type (found or created through vanilla's own `operator[]`, its 16 bytes restored after every round) is set to "can escape"
/// with a fenced edge of the live map as source and destination, so the real body copies the entry into a record allocated
/// through vanilla's `operator_new` and runs `evaluateEscape` over the scripted AI-manager search (as
/// `ZTANIMAL_EVALUATE_ESCAPE_LIVE` does). The record copy, the escape animation, the destination hand-off, the path list and the
/// fixture agree between real, port and the hooked address.
pub(crate) fn run_ztanimal_pick_random_dest_escape_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_PICK_RANDOM_DEST_ESCAPE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let world: u32 = get_from_memory(base() + 0x0023_8040);
    let map = world + 8;
    let (width, height): (i32, i32) = (get_from_memory(map + 0x2c), get_from_memory(map + 0x30));
    let tiles: u32 = get_from_memory(map + 0x38);
    let aimgr: u32 = get_from_memory(base() + 0x0023_8098);
    let tile_position = |tile: u32| -> [i32; 3] { [get_from_memory(tile + 0x34), get_from_memory(tile + 0x38), get_from_memory(tile + 0x3c)] };
    let neighbour = |tile: u32, direction: u32| unsafe { BFMAP_GET_NEIGHBOR.original()(map as *const u32, tile as *const u32, direction) };
    let mut fenced: Vec<(u32, u32)> = Vec::new();
    for index in 0..(width * height) as u32 {
        let tile = tiles + index * 0x8c;
        for direction in (0..8u32).step_by(2) {
            if get_from_memory::<u32>(tile + 0x14 + direction / 2 * 4) != 0 {
                let across = neighbour(tile, direction);
                if across != 0 {
                    fenced.push((tile, across));
                }
            }
        }
    }
    // Habitats that take part: not exhibits (`+0x2c` clear), as the real body requires.
    let habitats: Vec<u32> = live
        .iter()
        .map(|&source| unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(source) + 0x24c))(source) })
        .filter(|&habitat| habitat != 0 && get_from_memory::<u8>(habitat + 0x2c) == 0)
        .collect();
    if fenced.is_empty() || habitats.is_empty() {
        failures.push(format!("the live map holds no usable fenced edge or non-exhibit habitat (fenced {}, habitats {})", fenced.len(), habitats.len()));
        return finish_test(test_name, failures, failure_log);
    }
    let search = ScriptedSearch::install(aimgr);
    let rng_address = base() + 0x0023_8060;
    let mut rng = Lcg(0x91c7);
    let (mut ran, mut destinations, mut animations) = (0u32, 0u32, 0u32);
    for &ty in &types {
        for round in 0..60 {
            let habitat = habitats[(rng.next() as usize) % habitats.len()];
            let (from, across) = fenced[(rng.next() as usize) % fenced.len()];
            let stand = [tile_position(from)[0] * 64 + 32, tile_position(from)[1] * 64 + 32, tile_position(from)[2]];
            let kind = [0i32, 1, 2, 3, 4][(rng.next() as usize) % 5];
            let first_found = rng.next() % 6 != 0;
            let first = vec![tile_position(from), tile_position(across)];
            let second: (bool, Vec<[i32; 3]>) = match rng.next() % 3 {
                0 => (false, Vec::new()),
                1 => (true, vec![tile_position(from), tile_position(across)]),
                _ => (true, vec![tile_position(across), tile_position(from)]),
            };
            let seed = rng.next();
            let key = ty;
            let entry = unsafe { openzt_detour::generated::msvc_std_tree36::OPERATOR_INDEX.original()((habitat + 0x16c) as *const u32, &key as *const u32) } as u32;
            let saved: [u32; 4] = [get_from_memory(entry), get_from_memory(entry + 4), get_from_memory(entry + 8), get_from_memory(entry + 12)];
            save_to_memory(entry, 1u32);
            save_to_memory(entry + 4, from);
            save_to_memory(entry + 8, across);
            save_to_memory(entry + 12, kind);
            let prepare = |animal: ZTAnimal| {
                HABITAT.store(habitat, Ordering::SeqCst);
                SWIMS.store(false, Ordering::SeqCst);
                UNDERWATER.store(false, Ordering::SeqCst);
                for (offset, word) in [0x114u32, 0x118, 0x11c].into_iter().zip(stand) {
                    save_to_memory(animal.0 + offset, word);
                }
                save_to_memory(animal.0 + 0x395, 0u8);
                save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, 0u8);
                save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, 0u32);
                *SEARCH_SCRIPT.lock().unwrap() = vec![(first_found, first.clone()), second.clone()];
                SEARCH_NEXT.store(0, Ordering::SeqCst);
                save_to_memory(rng_address, seed);
            };
            poles(
                ty,
                &format!("pickRandomDest escape round {round} from {from:#x} across {across:#x} kind {kind} first found {first_found} seed {seed:#x}"),
                &prepare,
                &|animal, pole| {
                    let mut position = [0xaau32; 3];
                    let address = position.as_mut_ptr() as u32;
                    let result = match pole {
                        0 => real::pick_random_dest(animal.0, address),
                        1 => animal.pick_random_dest(address),
                        _ => unsafe { generated::PICK_RANDOM_DEST.hooked()(animal.0 as *const u32, address as *const glam::i32::IVec3) & 0xff != 0 },
                    };
                    let path = path_list(animal.0);
                    let record: u32 = get_from_memory(animal.0 + animal_offsets::ESCAPE_RECORD);
                    let mut hash = path.iter().flatten().fold(path.len() as u32, |hash, word| (hash ^ *word as u32).wrapping_mul(0x0100_0193));
                    if record != 0 {
                        for offset in 0..0x10u32 {
                            hash = (hash ^ get_from_memory::<u8>(record + offset) as u32).wrapping_mul(0x0100_0193);
                        }
                    }
                    hash ^= position[0] ^ position[1].rotate_left(7) ^ position[2].rotate_left(14) ^ get_from_memory::<u32>(rng_address).rotate_left(21);
                    result as i64 | (hash as i64) << 1
                },
                &mut failures,
            );
            for (i, word) in saved.iter().enumerate() {
                save_to_memory(entry + 4 * i as u32, *word);
            }
            ran += 1;
            let calls = CALLS.lock().unwrap().clone();
            destinations += calls.iter().any(|(slot, _)| *slot == 0x1f8) as u32;
            animations += calls.iter().any(|(slot, _)| *slot == 0x104) as u32;
            if failures.len() > 10 {
                break;
            }
        }
    }
    drop(search);
    if destinations == 0 || animations == 0 {
        failures.push(format!("coverage: the escape branch did not run to its destination and animation (ran {ran}, destinations {destinations}, animations {animations})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_PERF_COMPARE_LIVE`: per-call cost of the pathfinder callbacks `getTerrainCost`, `fencesOK` and `tileFilter`,
/// vanilla (through the detour's call-through) against the Rust port called directly and the hooked address the game itself
/// calls, in the release battery only (skipped in debug builds). A plain fixture animal (the vanilla constructor's own
/// vtable, no recorders) stands on a live animal's position; each function runs over the interior `62x62` tiles of the live
/// map's top-left block (every neighbour exists), the pathfinder's access pattern, for the two activity modes the
/// callbacks branch on. The timings are logged on the pass line; the test fails only when the hooked address is more
/// than `PERF_FAIL_RATIO` times vanilla. Run with page heap off.
pub(crate) fn run_ztanimal_perf_compare_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::hint::black_box;

    use super::ztkeeper::{median_ns_per_call, PoleTimes, PERF_FAIL_RATIO};
    use crate::reimplementation_tests::harness::write_success_line;

    let test_name = "ZTANIMAL_PERF_COMPARE_LIVE";
    if cfg!(debug_assertions) {
        write_success_line(failure_log, &format!("{test_name} (skipped: debug build)"));
        return false;
    }
    let mut failures: Vec<String> = Vec::new();
    let Some((types, live)) = fixture_inputs(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let animal = build_animal(types[0]);
    stand_on(animal, live[0]);
    let world: u32 = get_from_memory(base() + 0x0023_8040);
    let map = world + 8;
    let (width, height): (i32, i32) = (get_from_memory(map + 0x2c), get_from_memory(map + 0x30));
    let tiles_base: u32 = get_from_memory(map + 0x38);
    let limit = 63.min(width).min(height);
    let tiles: Vec<u32> = (1..limit).flat_map(|y| (1..limit).map(move |x| tiles_base + ((y * width + x) as u32) * 0x8c)).collect();
    let saved_flags: Vec<u8> = tiles.iter().map(|&tile| get_from_memory(tile + 0x85)).collect();
    let mut results: Vec<PoleTimes> = Vec::new();

    for mode in [0i32, 2] {
        save_to_memory(animal.0 + animal_offsets::ACTIVITY_MODE, mode);
        let calls = tiles.len();
        let mut vanilla = || {
            for &tile in &tiles {
                black_box(real::get_terrain_cost(animal.0, black_box(tile)));
            }
        };
        let mut port = || {
            for &tile in &tiles {
                black_box(animal.get_terrain_cost(black_box(tile)));
            }
        };
        let mut hooked = || {
            for &tile in &tiles {
                black_box(unsafe { generated::GET_TERRAIN_COST.hooked()(animal.0 as *const u32, black_box(tile) as *const u32) });
            }
        };
        let times = median_ns_per_call(&mut [&mut vanilla, &mut port, &mut hooked], 9, calls);
        results.push(PoleTimes { label: format!("getTerrainCost mode {mode} ({calls} tiles)"), vanilla: times[0], port: times[1], hooked: times[2] });
        for (&tile, &flags) in tiles.iter().zip(&saved_flags) {
            save_to_memory(tile + 0x85, flags);
        }
    }

    let pairs: Vec<(u32, u32)> = tiles.iter().flat_map(|&tile| (0u32..8).map(move |direction| (tile, direction))).collect();
    let calls = pairs.len();
    let (mut out_a, mut out_b) = (0u8, 0u8);
    let (a, b) = (&mut out_a as *mut u8 as u32, &mut out_b as *mut u8 as u32);
    let mut vanilla = || {
        for &(tile, direction) in &pairs {
            black_box(real::fences_ok(animal.0, black_box(tile), direction as i32, a, b));
        }
    };
    let mut port = || {
        for &(tile, direction) in &pairs {
            black_box(animal.fences_ok(black_box(tile), direction as i32, a, b));
        }
    };
    let mut hooked = || {
        for &(tile, direction) in &pairs {
            black_box(unsafe { generated::FENCES_OK.hooked()(animal.0 as *const u32, black_box(tile) as *const u32, direction, a as *const bool, b as *const bool) });
        }
    };
    let times = median_ns_per_call(&mut [&mut vanilla, &mut port, &mut hooked], 9, calls);
    results.push(PoleTimes { label: format!("fencesOK ({calls} tile/direction pairs)"), vanilla: times[0], port: times[1], hooked: times[2] });

    for mode in [0i32, 2] {
        save_to_memory(animal.0 + animal_offsets::ACTIVITY_MODE, mode);
        let calls = tiles.len();
        let mut vanilla = || {
            for &tile in &tiles {
                black_box(real::tile_filter(animal.0, black_box(tile), false));
            }
        };
        let mut port = || {
            for &tile in &tiles {
                black_box(animal.tile_filter(black_box(tile), false));
            }
        };
        let mut hooked = || {
            for &tile in &tiles {
                black_box(unsafe { generated::TILE_FILTER.hooked()(animal.0 as *const c_void, black_box(tile) as *const c_void, false) });
            }
        };
        let times = median_ns_per_call(&mut [&mut vanilla, &mut port, &mut hooked], 9, calls);
        results.push(PoleTimes { label: format!("tileFilter mode {mode} ({calls} tiles)"), vanilla: times[0], port: times[1], hooked: times[2] });
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
