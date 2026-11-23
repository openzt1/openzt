//! Live checks for the `ZTAnimal` lifecycle methods (`clearGoal`, `clearEscapeInfo`, `clearStateVariables`,
//! `setDeathCheck`, `setEgg`, `setEscaped`, `abortGoal`). Fixtures are animals built by the vanilla constructor over a
//! live animal type, with a recorded vtable for the slots these bodies dispatch through, so the calls (and their
//! arguments) are compared along with the resulting state: real body, Rust port and the hooked address.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use openzt_detour::generated::{bfentity::GET_TILE, standalone::OPERATOR_NEW, ztanimal as generated, ztworldmgr::GET_NEED_INDEX};

use super::ztanimal::{base, build_animal, destroy_animal, live_animal_types, live_animals, Lcg};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztanimal::{animal_offsets, live_support::real, ZTAnimal},
};

/// What the scripted vtable slots record: `(slot, argument bits)` in call order.
static CALLS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
static PREDICATE: AtomicBool = AtomicBool::new(false);

extern "thiscall" fn fake_clear_goal(_this: u32) {
    CALLS.lock().unwrap().push((0x174, 0));
}

extern "thiscall" fn fake_abort_trick(_this: u32) {
    CALLS.lock().unwrap().push((0x2c0, 0));
}

extern "thiscall" fn fake_is_doing_show(_this: u32) -> u32 {
    CALLS.lock().unwrap().push((0x22c, 0));
    PREDICATE.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_clear_prey(_this: u32) {
    CALLS.lock().unwrap().push((0x258, 0));
}

extern "thiscall" fn fake_stop_everything(_this: u32, a: u32, b: u32, c: u32) {
    CALLS.lock().unwrap().push((0x198, (a & 0xff) << 16 | (b & 0xff) << 8 | (c & 0xff)));
}

extern "thiscall" fn fake_update_animation(_this: u32, arg: u32) {
    CALLS.lock().unwrap().push((0x40, arg));
}

/// Replaces the scripted slots of the animal's vtable with recorders, keeping the real vtable for the rest. The copy
/// is leaked; fixtures are not destroyed through it.
fn install_scripted_vtable(animal: ZTAnimal) {
    let real_vtable: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x174 / 4] = fake_clear_goal as *const () as usize as u32;
    vtable[0x2c0 / 4] = fake_abort_trick as *const () as usize as u32;
    vtable[0x22c / 4] = fake_is_doing_show as *const () as usize as u32;
    vtable[0x258 / 4] = fake_clear_prey as *const () as usize as u32;
    vtable[0x198 / 4] = fake_stop_everything as *const () as usize as u32;
    vtable[0x40 / 4] = fake_update_animation as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// The state words of an animal, with the pool pointers left out and the escape record reduced to whether it is set.
fn snapshot(animal: ZTAnimal) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (start, end) in [(0x22cu32, 0x230u32), (0x24c, 0x250), (0x290, 0x384), (0x388, 0x3b4)] {
        for offset in start..end {
            bytes.push(get_from_memory::<u8>(animal.0 + offset));
        }
    }
    bytes.push((get_from_memory::<u32>(animal.0 + animal_offsets::ESCAPE_RECORD) != 0) as u8);
    let length = get_from_memory::<u32>(animal.0 + 0x264).wrapping_sub(get_from_memory::<u32>(animal.0 + 0x260));
    bytes.extend(length.to_le_bytes());
    bytes
}

/// Runs `body` (pole 0 real, 1 port, 2 hooked) on identically prepared scripted fixtures and compares the recorded
/// slot calls and the animals' state.
fn poles(ty: u32, label: &str, prepare: &dyn Fn(ZTAnimal), body: &dyn Fn(ZTAnimal, usize), failures: &mut Vec<String>) {
    let mut logs = Vec::new();
    let mut snapshots = Vec::new();
    for pole in 0..3usize {
        let animal = build_animal(ty);
        install_scripted_vtable(animal);
        prepare(animal);
        CALLS.lock().unwrap().clear();
        body(animal, pole);
        logs.push(CALLS.lock().unwrap().clone());
        snapshots.push(snapshot(animal));
    }
    if logs[0] != logs[1] || logs[0] != logs[2] {
        failures.push(format!("{label}: slot calls real {:?}, port {:?}, hooked {:?}", logs[0], logs[1], logs[2]));
    }
    for (pole, name) in [(1usize, "port"), (2, "hooked")] {
        if snapshots[0] != snapshots[pole] {
            let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
            failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
        }
    }
}

/// `ZTANIMAL_LIFECYCLE_LIVE`: the lifecycle methods on scripted fixtures over several live animal types.
pub(crate) fn run_ztanimal_lifecycle_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_LIFECYCLE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let live_tile = live_animals().first().map(|&a| unsafe { GET_TILE.original()(a as *const u32) } as u32).filter(|&t| t != 0);
    let need_index = |rva: u32| unsafe { GET_NEED_INDEX.hooked()((base() + rva) as *const i8) as u8 };
    let (need_a, need_b) = (need_index(0x0023_afc8), need_index(0x0024_63a0));
    let mut rng = Lcg(0x5eed);

    for &ty in &types {
        for has_flag in [0u8, 1] {
            for has_record in [false, true] {
                let prepare = |animal: ZTAnimal| {
                    save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, has_flag);
                    if has_record {
                        let record = unsafe { OPERATOR_NEW.original()(0x10) } as u32;
                        save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, record);
                    }
                };
                poles(
                    ty,
                    &format!("clearEscapeInfo flag {has_flag} record {has_record}"),
                    &prepare,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::clear_escape_info(animal.0),
                            1 => animal.clear_escape_info(),
                            _ => generated::CLEAR_ESCAPE_INFO.hooked()(animal.0 as *const std::ffi::c_void),
                        }
                    },
                    &mut failures,
                );
                let seed = rng.next();
                let prepare_goal = |animal: ZTAnimal| {
                    prepare(animal);
                    let mut local = Lcg(seed);
                    for offset in [0x3a2u32, 0x39f, 0x3a0, 0x3a1, 0x3a4, 0x3a5, 0x24c, 0x3a7, 0x3aa] {
                        save_to_memory(animal.0 + offset, (local.next() & 0x7f) as u8 | 1);
                    }
                    save_to_memory(animal.0 + animal_offsets::PREY_ID, local.next());
                    save_to_memory(animal.0 + 0x22c, local.next());
                };
                poles(
                    ty,
                    &format!("clearGoal flag {has_flag} record {has_record}"),
                    &prepare_goal,
                    &|animal, pole| unsafe {
                        match pole {
                            0 => real::clear_goal(animal.0),
                            1 => animal.clear_goal(),
                            _ => generated::CLEAR_GOAL.hooked()(animal.0 as *const std::ffi::c_void),
                        }
                    },
                    &mut failures,
                );
            }
        }
        let seed = rng.next();
        poles(
            ty,
            "clearStateVariables",
            &|animal| save_to_memory(animal.0 + animal_offsets::PREY_ID, seed),
            &|animal, pole| unsafe {
                match pole {
                    0 => real::clear_state_variables(animal.0),
                    1 => animal.clear_state_variables(),
                    _ => generated::CLEAR_STATE_VARIABLES.hooked()(animal.0 as *const u32),
                }
            },
            &mut failures,
        );
        for change_type in [-1, 0, 1, 2, 3] {
            for value in [-50, -1, 0, 7, 100, 250, i32::MAX] {
                for current in [-5, 0, 1, 1000, i32::MAX, i32::MIN] {
                    poles(
                        ty,
                        &format!("setDeathCheck({value}, {change_type}) from {current}"),
                        &|animal| save_to_memory(animal.0 + 0x34c, current),
                        &|animal, pole| unsafe {
                            match pole {
                                0 => real::set_death_check(animal.0, value, change_type),
                                1 => animal.set_death_check(value, change_type),
                                _ => generated::SET_DEATH_CHECK.hooked()(animal.0 as *const u32, value, change_type),
                            }
                        },
                        &mut failures,
                    );
                }
            }
        }
        for reset_timer in [false, true] {
            for override_byte in [0u8, 1] {
                for timer in [0i32, 1, 5, 400, -3] {
                    let flag_address = base() + 0x0023_cfa1;
                    let saved: u8 = get_from_memory(flag_address);
                    save_to_memory(flag_address, override_byte);
                    poles(
                        ty,
                        &format!("setEgg({reset_timer}) override {override_byte} timer {timer}"),
                        &|animal| {
                            save_to_memory(animal.0 + 0x398, timer);
                            save_to_memory(animal.0 + 0x260, 0x1000u32);
                            save_to_memory(animal.0 + 0x264, 0x1010u32);
                        },
                        &|animal, pole| unsafe {
                            match pole {
                                0 => real::set_egg(animal.0, reset_timer),
                                1 => animal.set_egg(reset_timer),
                                _ => generated::SET_EGG.hooked()(animal.0 as *const u32, reset_timer),
                            }
                        },
                        &mut failures,
                    );
                    save_to_memory(flag_address, saved);
                }
            }
        }
        for flag in [0u8, 1] {
            for answer in [false, true] {
                for need in [0xffu8, need_a, need_b] {
                    PREDICATE.store(answer, Ordering::SeqCst);
                    poles(
                        ty,
                        &format!("abortGoal({flag}) predicate {answer} need {need:#x}"),
                        &|animal| {
                            save_to_memory(animal.0 + 0x24e, need);
                            if let Some(tile) = live_tile {
                                save_to_memory(animal.0 + 0x244, tile);
                            }
                            save_to_memory(animal.0 + 0x24c, 1u8);
                            save_to_memory(animal.0 + 0x3a4, 1u8);
                            save_to_memory(animal.0 + 0x22c, 0x77u32);
                        },
                        &|animal, pole| unsafe {
                            match pole {
                                0 => real::abort_goal(animal.0, flag != 0),
                                1 => animal.abort_goal(flag != 0),
                                _ => generated::ABORT_GOAL.hooked()(animal.0 as *const u32, flag != 0),
                            }
                        },
                        &mut failures,
                    );
                }
            }
        }
        PREDICATE.store(false, Ordering::SeqCst);

        // `setEscaped`: the escaped-animal list after each step, with this animal's pointer normalised.
        let list_values = |animal: ZTAnimal| -> Vec<u32> {
            let sentinel: u32 = get_from_memory(get_from_memory::<u32>(base() + 0x0023_8fb0));
            let mut node: u32 = get_from_memory(sentinel);
            let mut values = Vec::new();
            while node != sentinel && values.len() < 100_000 {
                let value: u32 = get_from_memory(node + 8);
                values.push(if value == animal.0 { 0xaaaa_aaaa } else { value });
                node = get_from_memory(node);
            }
            values
        };
        let mut outcomes: Vec<Vec<(Vec<u32>, Vec<u8>)>> = Vec::new();
        for pole in 0..3usize {
            let animal = build_animal(ty);
            let mut steps = Vec::new();
            for flag in [1u8, 1, 3, 0, 0] {
                unsafe {
                    match pole {
                        0 => real::set_escaped(animal.0, flag),
                        1 => animal.set_escaped(flag),
                        _ => generated::SET_ESCAPED.hooked()(animal.0 as *const u32, flag as i8),
                    }
                }
                let bytes: Vec<u8> = [0x3a1u32, 0x3a2, 0x3a3].iter().map(|&o| get_from_memory::<u8>(animal.0 + o)).collect();
                steps.push((list_values(animal), bytes));
            }
            outcomes.push(steps);
            destroy_animal(animal);
        }
        if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
            failures.push(format!("setEscaped sequence: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
        }
        if failures.len() > 40 {
            break;
        }
    }
    failures.truncate(40);
    finish_test(&format!("{test_name} ({} type(s))", types.len()), failures, failure_log)
}
