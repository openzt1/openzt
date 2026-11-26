//! `ZTAnimal::drop` and `ZTAnimal::setVisible` against the real bodies.
//!
//! `drop` only dispatches: the animal's habitat slot, the habitat's query slot, the predator and prey searches, the
//! world manager's `getUnit` and the unit's type cast and release slot. Every one of them is replaced by a recorder, so
//! the three poles (real body, Rust port, hooked address) are compared on the call sequence with arguments and on the
//! animal's memory. The world manager's vtable is swapped for a copy for the duration of each case and restored.
//! `setVisible` writes one byte and asks the animal panel to refresh; the panel only reacts to the selected entity,
//! which a fixture never is.

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

use openzt_detour::generated::ztanimal as generated;

use super::ztanimal::{base, build_animal, destroy_animal, live_animal_types};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztanimal::{live_support::real, ZTAnimal},
};

const WORLD_MGR_RVA: u32 = 0x0023_8040;

static CALLS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
static HABITAT: AtomicU32 = AtomicU32::new(0);
static HABITAT_ANSWER: AtomicU32 = AtomicU32::new(0);
static PREDATOR: AtomicU32 = AtomicU32::new(0);
static PREY: AtomicU32 = AtomicU32::new(0);
static UNIT: AtomicU32 = AtomicU32::new(0);
static CAST_ANSWER: AtomicU32 = AtomicU32::new(0);

fn log(slot: u32, argument: u32) {
    CALLS.lock().unwrap().push((slot, argument));
}

extern "thiscall" fn fake_habitat(_this: u32) -> u32 {
    log(0x24c, 0);
    HABITAT.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_predator(_this: u32, argument: u32) -> u32 {
    log(0x2fc, argument);
    PREDATOR.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_prey(_this: u32, argument: u32) -> u32 {
    log(0x2f8, argument);
    PREY.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_habitat_query(_this: u32) -> u32 {
    log(0x20, 0);
    HABITAT_ANSWER.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_get_unit(_world: u32, id: u32) -> u32 {
    log(0x2c, id);
    UNIT.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_is_cast(_type: u32, tag: u32) -> u32 {
    log(0x1c, tag);
    CAST_ANSWER.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_release(_unit: u32) -> u32 {
    log(0x2c4, 0);
    0xfeed_0000
}

fn leak_object(vtable_slots: &[(u32, u32)]) -> u32 {
    let mut vtable = vec![0u32; 0x100];
    for &(slot, function) in vtable_slots {
        vtable[(slot / 4) as usize] = function;
    }
    let vtable = Box::leak(vtable.into_boxed_slice()).as_ptr() as u32;
    let mut object = vec![0u8; 0x400];
    object[0..4].copy_from_slice(&vtable.to_le_bytes());
    Box::leak(object.into_boxed_slice()).as_ptr() as u32
}

fn install_animal_vtable(animal: ZTAnimal) -> u32 {
    let real_vtable: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x24c / 4] = fake_habitat as *const () as usize as u32;
    vtable[0x2fc / 4] = fake_predator as *const () as usize as u32;
    vtable[0x2f8 / 4] = fake_prey as *const () as usize as u32;
    save_to_memory(animal.0, Box::leak(vtable.into_boxed_slice()).as_ptr() as u32);
    real_vtable
}

/// Animal state without the per-fixture pointers: the same ranges the other animal tests compare, plus the visible byte.
fn snapshot(animal: ZTAnimal) -> Vec<u8> {
    (0xe8u32..0xec).chain(0x13fu32..0x140).chain(0x250u32..0x260).chain(0x290u32..0x3b8).map(|offset| get_from_memory::<u8>(animal.0 + offset)).collect()
}

/// `ZTANIMAL_DROP_LIVE`.
pub(crate) fn run_ztanimal_drop_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_DROP_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(3);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }

    let habitat = leak_object(&[(0x20, fake_habitat_query as *const () as usize as u32)]);
    let unit_type = leak_object(&[(0x1c, fake_is_cast as *const () as usize as u32)]);
    let unit = leak_object(&[(0x2c4, fake_release as *const () as usize as u32)]);
    save_to_memory(unit + 0x128, unit_type);
    HABITAT.store(habitat, Ordering::SeqCst);
    UNIT.store(unit, Ordering::SeqCst);

    let world_pointer = base() + WORLD_MGR_RVA;
    let world: u32 = get_from_memory(world_pointer);
    let real_world_vtable: u32 = get_from_memory(world);
    let mut world_vtable: Vec<u32> = (0..0x80u32).map(|slot| get_from_memory(real_world_vtable + slot * 4)).collect();
    world_vtable[0x2c / 4] = fake_get_unit as *const () as usize as u32;
    let fake_world_vtable = Box::leak(world_vtable.into_boxed_slice()).as_ptr() as u32;

    // The query's upper bytes are junk on purpose: callers test `AL` only.
    let habitats: [(u32, u32, &str); 4] = [(0, 0, "no habitat"), (habitat, 0xabcd_ef00, "query false"), (habitat, 0x0000_0001, "query true"), (habitat, 0x1234_5601, "query true, dirty upper")];
    let ids = [0u32, 9];
    let units: [(u32, u32, &str); 3] = [(0, 0, "no unit"), (unit, 0, "unit not castable"), (unit, 1, "unit castable")];
    let mut branch_released = 0usize;
    let mut branch_predator = 0usize;
    let mut branch_prey = 0usize;

    save_to_memory(world, fake_world_vtable);
    for &ty in &types {
        for &(habitat_pointer, answer, habitat_label) in &habitats {
            for &predator in &ids {
                for &prey in &ids {
                    for &(unit_pointer, cast, unit_label) in &units {
                        let label = format!("{habitat_label} predator {predator} prey {prey} {unit_label}");
                        let mut logs = Vec::new();
                        let mut snapshots = Vec::new();
                        for pole in 0..3usize {
                            let animal = build_animal(ty);
                            let real_vtable = install_animal_vtable(animal);
                            HABITAT.store(habitat_pointer, Ordering::SeqCst);
                            HABITAT_ANSWER.store(answer, Ordering::SeqCst);
                            PREDATOR.store(predator, Ordering::SeqCst);
                            PREY.store(prey, Ordering::SeqCst);
                            UNIT.store(unit_pointer, Ordering::SeqCst);
                            CAST_ANSWER.store(cast, Ordering::SeqCst);
                            CALLS.lock().unwrap().clear();
                            match pole {
                                0 => real::drop_target(animal.0),
                                1 => animal.drop_target(),
                                _ => unsafe { generated::DROP.hooked()(animal.0 as *const u32) },
                            }
                            logs.push(CALLS.lock().unwrap().clone());
                            snapshots.push(snapshot(animal));
                            save_to_memory(animal.0, real_vtable);
                            if pole == 0 {
                                let calls = &logs[0];
                                branch_released += usize::from(calls.iter().any(|&(slot, _)| slot == 0x2c4));
                                branch_predator += usize::from(calls.iter().any(|&(slot, _)| slot == 0x2c) && predator != 0);
                                branch_prey += usize::from(calls.iter().any(|&(slot, _)| slot == 0x2c) && predator == 0 && prey != 0);
                            }
                            destroy_animal(animal);
                        }
                        if logs[0] != logs[1] || logs[0] != logs[2] {
                            failures.push(format!("{label}: calls real {:?}, port {:?}, hooked {:?}", logs[0], logs[1], logs[2]));
                        }
                        if snapshots[0] != snapshots[1] || snapshots[0] != snapshots[2] {
                            failures.push(format!("{label}: animal memory differs between poles"));
                        }
                    }
                }
            }
        }
    }
    save_to_memory(world, real_world_vtable);
    save_to_memory(world_pointer, world);

    if branch_released == 0 {
        failures.push("no case reached the unit's release slot under the real body".to_string());
    }
    if branch_predator == 0 || branch_prey == 0 {
        failures.push(format!("coverage: predator-id branch {branch_predator}, prey-id branch {branch_prey}"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_SET_VISIBLE_LIVE`.
pub(crate) fn run_ztanimal_set_visible_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_SET_VISIBLE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    for &ty in &types {
        for before in [0u8, 1, 0x7f] {
            for visible in [false, true] {
                let mut snapshots = Vec::new();
                let mut stored = Vec::new();
                for pole in 0..3usize {
                    let animal = build_animal(ty);
                    save_to_memory(animal.0 + 0x13f, before);
                    match pole {
                        0 => real::set_visible(animal.0, visible),
                        1 => animal.set_visible(visible),
                        _ => unsafe { generated::SET_VISIBLE.hooked()(animal.0 as *const u32, visible) },
                    }
                    snapshots.push(snapshot(animal));
                    stored.push(get_from_memory::<u8>(animal.0 + 0x13f));
                    destroy_animal(animal);
                }
                if snapshots[0] != snapshots[1] || snapshots[0] != snapshots[2] {
                    failures.push(format!("type {ty:#x} before {before:#x} visible {visible}: animal memory differs between poles"));
                }
                if stored != [u8::from(visible); 3] {
                    failures.push(format!("type {ty:#x}: the visible byte after the poles is {stored:?}, expected {visible}"));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
