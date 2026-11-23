//! Live checks for the small `ZTAnimal` feeding and trick members (`addChanges`, `dropTestFenceChance`, `doFoodTrick`,
//! `checkDoTrick`, `setFood`, `setKeeperArrives`, `faceTowardFood`, `removeFoodTargetForAll`, `fReduceFood`,
//! `fRemoveAnimal`). Fixtures are animals built by the vanilla constructor over live animal types with the vtable slots
//! the bodies dispatch through replaced by recorders; real body, Rust port and hooked address are compared on state,
//! result, recorded calls and the shared RNG word.
//!
//! Not covered, because they would run real vanilla code over a fixture: `fReduceFood` with a real food entity,
//! `fRemoveAnimal` removing the animal, `faceTowardFood` with a context.

use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex,
    },
};

use openzt_detour::generated::ztanimal as generated;

use super::ztanimal::{base, build_animal, live_animal_types, live_animals, Lcg};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztanimal::{animal_offsets, live_support::real, ZTAnimal},
    ztkeeper::call_slot_0,
};

static CALLS: Mutex<Vec<(u32, u32, u32)>> = Mutex::new(Vec::new());
static TRICK_LIMIT: AtomicU32 = AtomicU32::new(0);
static TRICK_PERCENT: AtomicU32 = AtomicU32::new(0);
static HABITAT: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_set_goal(_this: u32, goal: u32, flag: u32) -> u32 {
    CALLS.lock().unwrap().push((0x1d8, goal, flag));
    0xdead_0000
}

extern "thiscall" fn fake_trick_limit(_this: u32) -> u32 {
    CALLS.lock().unwrap().push((0x238, 0, 0));
    TRICK_LIMIT.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_trick_percent(_this: u32) -> u32 {
    CALLS.lock().unwrap().push((0x23c, 0, 0));
    TRICK_PERCENT.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_habitat(_this: u32) -> u32 {
    CALLS.lock().unwrap().push((0x24c, 0, 0));
    HABITAT.load(Ordering::SeqCst)
}

static PREY: AtomicU32 = AtomicU32::new(0);
static PREY_TYPE_ID: AtomicU32 = AtomicU32::new(0);

macro_rules! recorder0 {
    ($name:ident, $slot:expr, $ret:expr) => {
        extern "thiscall" fn $name(_this: u32) -> u32 {
            CALLS.lock().unwrap().push(($slot, 0, 0));
            $ret
        }
    };
}
macro_rules! recorder1 {
    ($name:ident, $slot:expr) => {
        extern "thiscall" fn $name(_this: u32, a: u32) -> u32 {
            CALLS.lock().unwrap().push(($slot, a, 0));
            0xbeef_0000
        }
    };
}
recorder1!(fake_set_map_entry, 0x78);
recorder0!(fake_refresh_a, 0xe0, 0x1111_0000);
recorder0!(fake_refresh_b, 0x74, 0x2222_0000);
recorder0!(fake_clear_goal, 0x174, 0x3333_0000);
recorder0!(fake_after_box, 0x254, 0x4444_0000);
recorder0!(fake_clear_prey, 0x258, 0x5555_0000);
recorder1!(fake_set_visible, 0xcc);

extern "thiscall" fn fake_get_prey(_this: u32) -> u32 {
    CALLS.lock().unwrap().push((0x25c, 0, 0));
    PREY.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_stop_everything(_this: u32, a: u32, b: u32, c: u32) -> u32 {
    CALLS.lock().unwrap().push((0x198, a, b | (c << 8)));
    0x6666_0000
}

extern "thiscall" fn fake_prey_type_id(_this: u32) -> u32 {
    CALLS.lock().unwrap().push((0x20, 0, 0));
    PREY_TYPE_ID.load(Ordering::SeqCst)
}

fn install_vtable(animal: ZTAnimal) {
    let real_vtable: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x24c / 4] = fake_habitat as *const () as usize as u32;
    vtable[0x1d8 / 4] = fake_set_goal as *const () as usize as u32;
    vtable[0x238 / 4] = fake_trick_limit as *const () as usize as u32;
    vtable[0x23c / 4] = fake_trick_percent as *const () as usize as u32;
    vtable[0x78 / 4] = fake_set_map_entry as *const () as usize as u32;
    vtable[0xe0 / 4] = fake_refresh_a as *const () as usize as u32;
    vtable[0x74 / 4] = fake_refresh_b as *const () as usize as u32;
    vtable[0x174 / 4] = fake_clear_goal as *const () as usize as u32;
    vtable[0x254 / 4] = fake_after_box as *const () as usize as u32;
    vtable[0x258 / 4] = fake_clear_prey as *const () as usize as u32;
    vtable[0x25c / 4] = fake_get_prey as *const () as usize as u32;
    vtable[0x198 / 4] = fake_stop_everything as *const () as usize as u32;
    vtable[0xcc / 4] = fake_set_visible as *const () as usize as u32;
    save_to_memory(animal.0, Box::leak(vtable.into_boxed_slice()).as_ptr() as u32);
}

/// State of an animal (`+0x250..0x260` and `+0x290` to the end, skipping the per-fixture pool-string pointers), with the result and the shared RNG word appended.
fn snapshot(animal: ZTAnimal, result: u32, rng_address: u32) -> Vec<u8> {
    let mut bytes: Vec<u8> = (0xe8u32..0xec).chain(0x250u32..0x260).chain(0x290u32..0x3b8).map(|offset| get_from_memory::<u8>(animal.0 + offset)).collect();
    bytes.extend(result.to_le_bytes());
    bytes.extend(get_from_memory::<u32>(rng_address).to_le_bytes());
    bytes
}

fn poles(ty: u32, label: &str, seed: u32, prepare: &dyn Fn(ZTAnimal), body: &dyn Fn(ZTAnimal, usize) -> u32, failures: &mut Vec<String>) {
    let rng_address = base() + 0x0023_8060;
    let mut logs = Vec::new();
    let mut snapshots = Vec::new();
    for pole in 0..3usize {
        let animal = build_animal(ty);
        install_vtable(animal);
        prepare(animal);
        CALLS.lock().unwrap().clear();
        save_to_memory(rng_address, seed);
        let result = body(animal, pole);
        logs.push(CALLS.lock().unwrap().clone());
        snapshots.push(snapshot(animal, result, rng_address));
    }
    if logs[0] != logs[1] || logs[0] != logs[2] {
        failures.push(format!("{label}: slot calls real {:?}, port {:?}, hooked {:?}", logs[0], logs[1], logs[2]));
    }
    for (pole, name) in [(1usize, "port"), (2, "hooked")] {
        if snapshots[0] != snapshots[pole] {
            let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
            failures.push(format!("{label}: state/result differs from real for {name} (first differing index {first:?}, offsets skip +0x260..+0x290)"));
        }
    }
}

/// `ZTANIMAL_FEEDING_HELPERS_LIVE`.
pub(crate) fn run_ztanimal_feeding_helpers_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_FEEDING_HELPERS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(6);
    let animals = live_animals();
    if types.is_empty() || animals.is_empty() {
        failures.push("the live world holds no animal type or animal".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let live_habitat = animals.iter().map(|&animal| call_slot_0(animal, 0x24c)).find(|&h| h != 0);
    let mut rng = Lcg(0xfeed);

    for &ty in &types {
        for round in 0..24u32 {
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x55aa;
            let changes: Vec<i32> = (0..8).map(|_| (rng.next() & 0x3ff) as i32 - 512).collect();
            let changes_ptr = Box::leak(changes.clone().into_boxed_slice()).as_ptr() as u32;
            let pending = [(rng.next() & 0xff) as i32 - 100, (rng.next() & 0xff) as i32 - 100, (rng.next() & 0xff) as i32 - 100];
            let prepare = |animal: ZTAnimal| {
                save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, pending[0]);
                save_to_memory(animal.0 + 0x2b4, pending[1]);
                save_to_memory(animal.0 + 0x2c4, pending[2]);
            };
            poles(
                ty,
                &format!("addChanges round {round} changes {changes:?}"),
                seed,
                &prepare,
                &|animal, pole| unsafe {
                    // Vanilla returns `AL = 1` over dirty upper bytes; callers test `AL` only.
                    (match pole {
                        0 => real::add_changes(animal.0, changes_ptr),
                        1 => animal.add_changes(changes_ptr) as u32,
                        _ => generated::ADD_CHANGES.hooked()(animal.0 as *const u32, changes_ptr as *const u32),
                    }) & 0xff
                },
                &mut failures,
            );

            let counter = (rng.next() & 0x3ff) as i32 - 200;
            let prepare = |animal: ZTAnimal| save_to_memory(animal.0 + 0x3ac, counter);
            poles(
                ty,
                &format!("dropTestFenceChance round {round} counter {counter}"),
                seed,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::drop_test_fence_chance(animal.0),
                        1 => animal.drop_test_fence_chance(),
                        _ => generated::DROP_TEST_FENCE_CHANCE.hooked()(animal.0 as *const u32),
                    }
                    0
                },
                &mut failures,
            );

            let (word_a, word_b) = ((rng.next() & 0xffff) as u16, (rng.next() & 0xffff) as u16);
            poles(
                ty,
                &format!("doFoodTrick round {round} words {word_a:#x}/{word_b:#x}"),
                seed,
                &|_| {},
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_food_trick(animal.0, word_a, word_b),
                        1 => {
                            animal.do_food_trick(word_a, word_b);
                            0
                        }
                        _ => generated::DO_FOOD_TRICK.hooked()(animal.0 as *const u32, word_a, word_b),
                    }
                },
                &mut failures,
            );

            let happiness = (rng.next() & 0xff) as i32 - 100;
            let limit = [happiness - 1, happiness, happiness + 1][(rng.next() as usize) % 3];
            let percent = [0i32, 1, 50, 100][(rng.next() as usize) % 4];
            let prepare = |animal: ZTAnimal| {
                TRICK_LIMIT.store(limit as u32, Ordering::SeqCst);
                TRICK_PERCENT.store(percent as u32, Ordering::SeqCst);
                save_to_memory(animal.0 + animal_offsets::HAPPINESS, happiness);
            };
            poles(
                ty,
                &format!("checkDoTrick round {round} happiness {happiness} limit {limit} percent {percent}"),
                seed,
                &prepare,
                &|animal, pole| unsafe {
                    let result = match pole {
                        0 => real::check_do_trick(animal.0),
                        1 => animal.check_do_trick() as u32,
                        _ => generated::CHECK_DO_TRICK.hooked()(animal.0 as *const u32),
                    };
                    // Callers test `AL` only.
                    result & 0xff
                },
                &mut failures,
            );
        }

        // `setFood`: null leaves the context alone, a live entity replaces it with its tile.
        for &food in std::iter::once(&0u32).chain(animals.iter().take(3)) {
            let prepare = |animal: ZTAnimal| save_to_memory(animal.0 + animal_offsets::EAT_CONTEXT, 0x1234_5678u32);
            poles(
                ty,
                &format!("setFood food {food:#x}"),
                1,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::set_food(animal.0, food),
                        1 => animal.set_food(food),
                        _ => generated::SET_FOOD.hooked()(animal.0 as *const u32, food as *const u32),
                    }
                    0
                },
                &mut failures,
            );
        }

        // `setKeeperArrives`: live animals stand in for the keeper (only the type comparison inside `canService` runs).
        for &staff in animals.iter().take(3) {
            poles(
                ty,
                &format!("setKeeperArrives staff {staff:#x}"),
                1,
                &|_| {},
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::set_keeper_arrives(animal.0, staff),
                        1 => animal.set_keeper_arrives(staff),
                        _ => generated::SET_KEEPER_ARRIVES.hooked()(animal.0 as *const u32, staff as *const u32),
                    }
                    0
                },
                &mut failures,
            );
        }

        poles(
            ty,
            "faceTowardFood without context",
            1,
            &|animal| save_to_memory(animal.0 + animal_offsets::EAT_CONTEXT, 0u32),
            &|animal, pole| unsafe {
                match pole {
                    0 => real::face_toward_food(animal.0),
                    1 => animal.face_toward_food(),
                    _ => generated::FACE_TOWARD_FOOD.hooked()(animal.0 as *const u32),
                }
                0
            },
            &mut failures,
        );

        // A stand-in habitat: zeroed, byte `+0x2d` clear and an animal count of 5, so `fRemoveAnimal` leaves the animal alone.
        let stand_in = Box::leak(vec![0u8; 0x200].into_boxed_slice()).as_ptr() as u32;
        save_to_memory(stand_in + 0xf0, 5i32);
        let habitats: Vec<u32> = std::iter::once(0).chain(live_habitat).chain(std::iter::once(stand_in)).collect();
        for &habitat in &habitats {
            let prepare = |animal: ZTAnimal| {
                HABITAT.store(habitat, Ordering::SeqCst);
                save_to_memory(animal.0 + animal_offsets::EAT_CONTEXT, 0u32);
            };
            if habitat != stand_in {
                poles(
                    ty,
                    &format!("removeFoodTargetForAll habitat {habitat:#x}"),
                    1,
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::remove_food_target_for_all(animal.0, 0),
                            1 => animal.remove_food_target_for_all(0),
                            _ => generated::REMOVE_FOOD_TARGET_FOR_ALL.hooked()(animal.0 as *const u32, std::ptr::null()),
                        }
                    },
                    &mut failures,
                );
            }
            poles(
                ty,
                &format!("fReduceFood habitat {habitat:#x} without context"),
                1,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::f_reduce_food(animal.0, 3) as u32,
                        1 => animal.f_reduce_food(3) as u32,
                        _ => generated::F_REDUCE_FOOD.hooked()(animal.0 as *const u32, 3) as u32,
                    }
                },
                &mut failures,
            );
            if habitat == 0 || habitat == stand_in {
                poles(
                    ty,
                    &format!("fRemoveAnimal habitat {habitat:#x}"),
                    1,
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::f_remove_animal(animal.0),
                            1 => animal.f_remove_animal(),
                            _ => generated::F_REMOVE_ANIMAL.hooked()(animal.0 as *const c_void),
                        }
                        0
                    },
                    &mut failures,
                );
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_HANDLING_LIVE`: `stopEating`, `stopChase`, `box`, `unbox` and `changeToDirt` (tile-less fixtures only:
/// `changeToDirt` with a tile would paint the live map) on scripted fixtures. Not covered: `stopChase` is run with a
/// prey stand-in whose type answers the id slot, so the real `ZTUnit` prey bookkeeping is not exercised.
pub(crate) fn run_ztanimal_handling_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_HANDLING_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(6);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let mut rng = Lcg(0x4a2d);

    // A prey stand-in: a zeroed entity block whose type vtable answers slot `0x20`.
    let mut type_vtable = vec![0u32; 0x40];
    type_vtable[0x20 / 4] = fake_prey_type_id as *const () as usize as u32;
    let type_vtable = Box::leak(type_vtable.into_boxed_slice());
    let prey_type = Box::leak(vec![type_vtable.as_ptr() as u32; 4].into_boxed_slice()).as_ptr() as u32;
    let prey = Box::leak(vec![0u8; 0x400].into_boxed_slice()).as_ptr() as u32;
    save_to_memory(prey + 0x128, prey_type);

    for &ty in &types {
        let own_id = call_slot_0(ty, 0x20);
        for flag in [0u8, 1, 0x80] {
            for named in [false, true] {
                let name = Box::leak(vec![b'x'; 8].into_boxed_slice()).as_ptr() as u32;
                let prepare = |animal: ZTAnimal| {
                    save_to_memory(animal.0 + 0x3a9, 1u8);
                    save_to_memory(animal.0 + 0xdc, name);
                    save_to_memory(animal.0 + 0xe0, if named { name + 3 } else { name });
                    save_to_memory(name, b'x');
                };
                poles(
                    ty,
                    &format!("stopEating flag {flag} named {named}"),
                    1,
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::stop_eating(animal.0, flag),
                            1 => animal.stop_eating(flag),
                            _ => generated::STOP_EATING.hooked()(animal.0 as *const u32, flag),
                        }
                        get_from_memory::<u32>(animal.0 + 0xe0).wrapping_sub(get_from_memory::<u32>(animal.0 + 0xdc)) | (get_from_memory::<u8>(name) as u32) << 8
                    },
                    &mut failures,
                );
            }
        }

        for (prey_ptr, id_delta, force) in [(0u32, 0u32, false), (0, 0, true), (prey, 0, false), (prey, 1, false), (prey, 1, true)] {
            let prepare = |animal: ZTAnimal| {
                PREY.store(prey_ptr, Ordering::SeqCst);
                PREY_TYPE_ID.store(own_id.wrapping_add(id_delta), Ordering::SeqCst);
                for offset in [0x3a4u32, 0x3a5, 0x20d] {
                    save_to_memory(animal.0 + offset, 1u8);
                }
                save_to_memory(prey + 0x22c, 0xabcdu32);
            };
            poles(
                ty,
                &format!("stopChase prey {prey_ptr:#x} id delta {id_delta} force {force}"),
                1,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::stop_chase(animal.0, force),
                        1 => animal.stop_chase(force),
                        _ => generated::STOP_CHASE.hooked()(animal.0 as *const u32, force),
                    }
                    get_from_memory::<u32>(prey + 0x22c)
                },
                &mut failures,
            );
        }

        for stage in [0u8, 1] {
            let prepare = |animal: ZTAnimal| {
                PREY.store(0, Ordering::SeqCst);
                save_to_memory(animal.0 + 0x393, stage);
                save_to_memory(animal.0 + 0x374, 0x77i32);
                save_to_memory(animal.0 + 0x3a3, 0u8);
                // A short `(id, age)` vector: begin, then an end past it.
                let vector = Box::leak(vec![0u64; 4].into_boxed_slice()).as_ptr() as u32;
                save_to_memory(animal.0 + 0x260, vector);
                save_to_memory(animal.0 + 0x264, vector + 16);
            };
            let _ = rng.next();
            poles(
                ty,
                &format!("box stage {stage}"),
                1,
                &prepare,
                &|animal, pole| unsafe { (match pole { 0 => real::box_animal(animal.0), 1 => animal.box_animal() as u32, _ => generated::BOX.hooked()(animal.0 as *const u32) }) & 0xff },
                &mut failures,
            );
            poles(
                ty,
                &format!("unbox stage {stage}"),
                1,
                &|animal| {
                    save_to_memory(animal.0 + 0x394, 1u8);
                    save_to_memory(animal.0 + 0x211, 1u8);
                },
                &|animal, pole| unsafe { (match pole { 0 => real::unbox(animal.0), 1 => animal.unbox() as u32, _ => generated::UNBOX.hooked()(animal.0 as *const i32) }) & 0xff },
                &mut failures,
            );
        }

        poles(
            ty,
            "changeToDirt without a tile",
            1,
            &|_| {},
            &|animal, pole| unsafe {
                match pole {
                    0 => real::change_to_dirt(animal.0),
                    1 => animal.change_to_dirt(),
                    _ => generated::CHANGE_TO_DIRT.hooked()(animal.0 as i32),
                }
                0
            },
            &mut failures,
        );
    }
    finish_test(test_name, failures, failure_log)
}

/// A fingerprint of the `(face, age)` vector at `+0x260`: entry count, each entry, and whether the capacity is exactly the
/// entry count's next doubling (pointers are per-fixture, so only the shape is compared).
fn face_fingerprint(animal: ZTAnimal) -> Vec<u32> {
    let begin: u32 = get_from_memory(animal.0 + 0x260);
    let end: u32 = get_from_memory(animal.0 + 0x264);
    let cap: u32 = get_from_memory(animal.0 + 0x268);
    let count = end.wrapping_sub(begin) / 8;
    let mut out = vec![count, cap.wrapping_sub(begin) / 8];
    for index in 0..count.min(64) {
        out.push(get_from_memory::<u8>(begin + index * 8) as u32);
        out.push(get_from_memory::<u32>(begin + index * 8 + 4));
    }
    out
}

/// `ZTANIMAL_FACES_PREY_LIVE`: `addFace` (growth through the vanilla pool) and `checkPrey` over fixtures and live animals.
pub(crate) fn run_ztanimal_faces_prey_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_FACES_PREY_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(6);
    let animals = live_animals();
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let mut rng = Lcg(0xface);

    for &ty in &types {
        for pushes in [1usize, 2, 3, 4, 5, 9, 17] {
            let faces: Vec<bool> = (0..pushes).map(|_| rng.next() & 1 == 1).collect();
            let mut fingerprints = Vec::new();
            let mut snapshots = Vec::new();
            for pole in 0..3usize {
                let animal = build_animal(ty);
                for &smile in &faces {
                    unsafe {
                        match pole {
                            0 => real::add_face(animal.0, smile),
                            1 => animal.add_face(smile),
                            _ => generated::ADD_FACE.hooked()(animal.0 as *const u32, smile),
                        }
                    }
                }
                fingerprints.push(face_fingerprint(animal));
                snapshots.push((0x290u32..0x3b8).map(|offset| get_from_memory::<u8>(animal.0 + offset)).collect::<Vec<u8>>());
            }
            if fingerprints[0] != fingerprints[1] || fingerprints[0] != fingerprints[2] {
                failures.push(format!("addFace pushes {pushes}: vectors real {:?}, port {:?}, hooked {:?}", fingerprints[0], fingerprints[1], fingerprints[2]));
            }
            if snapshots[0] != snapshots[1] || snapshots[0] != snapshots[2] {
                failures.push(format!("addFace pushes {pushes}: animal state differs"));
            }
        }

        // `checkPrey`: fixtures of every type in the egg, boxed and plain states, plus the live animals, both modes.
        let mut targets: Vec<(String, u32)> = vec![("null".to_string(), 0)];
        for &other in &types {
            for (label, egg, boxed) in [("plain", 0u8, 0u8), ("egg", 1, 0), ("boxed", 0, 1)] {
                let target = build_animal(other);
                save_to_memory(target.0 + 0x395, egg);
                save_to_memory(target.0 + 0x394, boxed);
                targets.push((format!("{label} fixture of type {other:#x}"), target.0));
            }
        }
        for &animal in animals.iter().take(6) {
            targets.push((format!("live animal {animal:#x}"), animal));
        }
        for (label, target) in &targets {
            for force in [false, true] {
                let mut results = Vec::new();
                for pole in 0..3usize {
                    let own = build_animal(ty);
                    results.push(unsafe {
                        match pole {
                            0 => real::check_prey(own.0, *target, force) as u32,
                            1 => animal_check_prey(own, *target, force) as u32,
                            _ => generated::CHECK_PREY.hooked()(own.0 as *const u32, *target as *const u32, force) as u32,
                        }
                    } & 0xff);
                }
                if results[0] != results[1] || results[0] != results[2] {
                    failures.push(format!("checkPrey own {ty:#x} target {label} force {force}: real {}, port {}, hooked {}", results[0], results[1], results[2]));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn animal_check_prey(own: ZTAnimal, target: u32, force: bool) -> bool {
    own.check_prey(target, force)
}

/// The mate-relevant words of a live animal: status words `+0x298..+0x2a8`, the mate id and the avoid ids.
fn mate_words(animal: u32) -> Vec<u32> {
    [0x298u32, 0x29c, 0x2a0, 0x2a4, 0x2e8, 0x2ec, 0x2f0].into_iter().map(|offset| get_from_memory::<u32>(animal + offset)).collect()
}

fn restore_mate_words(animal: u32, words: &[u32]) {
    for (offset, &word) in [0x298u32, 0x29c, 0x2a0, 0x2a4, 0x2e8, 0x2ec, 0x2f0].into_iter().zip(words) {
        save_to_memory(animal + offset, word);
    }
}

/// `ZTANIMAL_GET_MATE_LIVE`: `fGetMate` over fixtures that stand in the live zoo's habitat. The live animals it may pair
/// with are snapshotted before and restored after every pole, and their words compared across poles.
pub(crate) fn run_ztanimal_get_mate_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_GET_MATE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(6);
    let animals = live_animals();
    let Some(habitat) = animals.iter().map(|&animal| call_slot_0(animal, 0x24c)).find(|&h| h != 0) else {
        failures.push("the live world holds no animal in a habitat".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let originals: Vec<(u32, Vec<u32>)> = animals.iter().map(|&animal| (animal, mate_words(animal))).collect();
    let rng_address = base() + 0x0023_8060;
    let mut rng = Lcg(0x3a7e);
    let stand_in = Box::leak(vec![0u8; 0x200].into_boxed_slice()).as_ptr() as u32;
    let flagged = Box::leak(vec![0u8; 0x200].into_boxed_slice()).as_ptr() as u32;
    save_to_memory(flagged + 0x2c, 1u8);

    for &ty in &types {
        for round in 0..12u32 {
            let target_habitat = [habitat, 0, stand_in, flagged][(rng.next() as usize) % 4];
            let female = rng.next() & 1 == 1;
            let mate = if rng.next() & 3 == 0 { animals[(rng.next() as usize) % animals.len()] } else { 0 };
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x77;
            let mut results: Vec<(u32, u8, Vec<u8>, Vec<Vec<u32>>, u32)> = Vec::new();
            for pole in 0..3usize {
                let own = build_animal(ty);
                install_vtable(own);
                HABITAT.store(target_habitat, Ordering::SeqCst);
                save_to_memory(own.0 + 0x124, 0x7000_0000u32 + round);
                if female {
                    let text = Box::leak(b"Female\0".to_vec().into_boxed_slice()).as_ptr() as u32;
                    save_to_memory(own.0 + 0x26c, text);
                    save_to_memory(own.0 + 0x270, text + 6);
                }
                if mate != 0 {
                    save_to_memory(own.0 + 0x2e8, get_from_memory::<u32>(mate + 0x124));
                }
                CALLS.lock().unwrap().clear();
                save_to_memory(rng_address, seed);
                let mut found = 0u8;
                let found_ptr = &mut found as *mut u8 as u32;
                let result = unsafe {
                    match pole {
                        0 => real::f_get_mate(own.0, found_ptr),
                        1 => own.f_get_mate(found_ptr) as i32,
                        _ => generated::F_GET_MATE.hooked()(own.0 as *const u32, found_ptr as *const bool),
                    }
                } as u32;
                let picked_id = if result != 0 { get_from_memory::<u32>(result + 0x124) } else { 0 };
                let live_after: Vec<Vec<u32>> = animals.iter().map(|&animal| mate_words(animal)).collect();
                let own_state: Vec<u8> = (0x290u32..0x3b8).map(|offset| get_from_memory::<u8>(own.0 + offset)).collect();
                results.push((picked_id, found, own_state, live_after, get_from_memory::<u32>(rng_address)));
                for (animal, words) in &originals {
                    restore_mate_words(*animal, words);
                }
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if results[0] != results[pole] {
                    failures.push(format!(
                        "fGetMate type {ty:#x} round {round} habitat {target_habitat:#x} female {female} mate {mate:#x}: {name} differs from real (picked {:#x} vs {:#x}, found {} vs {})",
                        results[pole].0, results[0].0, results[pole].1, results[0].1
                    ));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
