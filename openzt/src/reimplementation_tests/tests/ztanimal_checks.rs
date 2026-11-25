//! Live checks for the `ZTAnimal` per-tick check helpers (`doBreathCheck`, `doKeeperCheck`, `doOtherCheck`,
//! `doEggCheck`). Fixtures are animals built by the vanilla constructor over live animal types, with the vtable slots
//! these bodies dispatch through replaced by recorders (`isDoingShow` at `+0x22c`, the habitat at `+0x24c`), so the
//! calls are compared along with the state: real body, Rust port and the hooked address.
//!
//! `hatchEgg` and `setIsRemoved` run real vanilla code over a fixture, so the egg check is only driven into branches
//! that stop before them (no habitat, or a habitat whose no-hatch byte is set).

use std::sync::{
    atomic::{AtomicU32, AtomicBool, Ordering},
    Mutex,
};

use openzt_detour::generated::{bfentity as generated_bfentity, ztanimal as generated};

use super::ztanimal::{build_animal, live_animal_types, live_animals, Lcg};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztanimal::{animal_offsets, live_support::real, ZTAnimal},
};

static CALLS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static DOING_SHOW: AtomicBool = AtomicBool::new(false);
static HABITAT: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_is_doing_show(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x22c);
    // The upper bytes are deliberately dirty: callers test `AL` only.
    0xabcd_ef00 | DOING_SHOW.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_habitat(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x24c);
    HABITAT.load(Ordering::SeqCst)
}

fn install_scripted_vtable(animal: ZTAnimal) {
    let real_vtable: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x22c / 4] = fake_is_doing_show as *const () as usize as u32;
    vtable[0x24c / 4] = fake_habitat as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// The state bytes of an animal from `+0x260` to the end of the animal, plus the happiness and health words.
fn snapshot(animal: ZTAnimal) -> Vec<u8> {
    (0x290u32..0x3b4).map(|offset| get_from_memory::<u8>(animal.0 + offset)).collect()
}

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
            failures.push(format!("{label}: state differs from real for {name} (first differing offset {:?})", first.map(|i| i + 0x290)));
        }
    }
}

/// `ZTANIMAL_CHECKS_LIVE`: the four small per-tick checks on scripted fixtures over several live animal types.
pub(crate) fn run_ztanimal_checks_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_CHECKS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let live_habitat = live_animals().first().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).filter(|&h| h != 0);
    let mut rng = Lcg(0xc4ec);

    for &ty in &types {
        for countdown in [0i32, 1, 7, -3] {
            for mode in [4u32, 3, 0] {
                for breath in [-50i32, 0, 10, 100_000] {
                    let prepare = |animal: ZTAnimal| {
                        save_to_memory(animal.0 + 0x364, countdown);
                        save_to_memory(animal.0 + 0x258, mode);
                        save_to_memory(animal.0 + animal_offsets::BREATH, breath);
                    };
                    poles(
                        ty,
                        &format!("doBreathCheck countdown {countdown} mode {mode} breath {breath}"),
                        &prepare,
                        &|animal, pole| unsafe {
                            match pole {
                                0 => real::do_breath_check(animal.0),
                                1 => animal.do_breath_check(),
                                _ => generated::DO_BREATH_CHECK.hooked()(animal.0 as *const u32),
                            }
                        },
                        &mut failures,
                    );
                }
            }
            for keepers in [-1i32, 0, 3, 100] {
                for egg in [0u8, 1] {
                    for show in [false, true] {
                        let pending = (rng.next() & 0xff) as i32 - 128;
                        let prepare = |animal: ZTAnimal| {
                            DOING_SHOW.store(show, Ordering::SeqCst);
                            save_to_memory(animal.0 + 0x30c, countdown);
                            save_to_memory(animal.0 + 0x378, keepers);
                            save_to_memory(animal.0 + animal_offsets::EGG, egg);
                            save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, pending);
                        };
                        poles(
                            ty,
                            &format!("doKeeperCheck countdown {countdown} keepers {keepers} egg {egg} show {show}"),
                            &prepare,
                            &|animal, pole| unsafe {
                                match pole {
                                    0 => real::do_keeper_check(animal.0),
                                    1 => animal.do_keeper_check(),
                                    _ => generated::DO_KEEPER_CHECK.hooked()(animal.0 as *const u32),
                                }
                            },
                            &mut failures,
                        );
                    }
                }
            }
            let habitats: Vec<u32> = std::iter::once(0).chain(live_habitat).collect();
            for &habitat in &habitats {
                for words in [0u32, 0xffff_ffff, 0x8, 0xa5a5_a5a5] {
                    for angry in [0u8, 1] {
                        let seed = rng.next();
                        let prepare = |animal: ZTAnimal| {
                            HABITAT.store(habitat, Ordering::SeqCst);
                            save_to_memory(animal.0 + 0x308, countdown);
                            for (i, offset) in [0x298u32, 0x29c, 0x2a0, 0x2a4].into_iter().enumerate() {
                                save_to_memory(animal.0 + offset, words ^ (seed.wrapping_mul(i as u32 + 1) & 0x0f0f));
                            }
                            save_to_memory(animal.0 + animal_offsets::ANGRY, angry);
                            save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, (seed & 0xff) as i32);
                        };
                        poles(
                            ty,
                            &format!("doOtherCheck countdown {countdown} habitat {habitat:#x} words {words:#x} angry {angry}"),
                            &prepare,
                            &|animal, pole| unsafe {
                                match pole {
                                    0 => real::do_other_check(animal.0),
                                    1 => animal.do_other_check(),
                                    _ => generated::DO_OTHER_CHECK.hooked()(animal.0 as *const u32),
                                }
                            },
                            &mut failures,
                        );
                    }
                }
            }
            // The egg check: only the branches that stop before `hatchEgg`/`setIsRemoved`.
            let no_hatch = Box::leak(vec![1u8; 0x40].into_boxed_slice());
            for habitat in [0u32, no_hatch.as_ptr() as u32] {
                for not_selectable in [0u8, 1] {
                    for health in [-5i32, 0, 30, 1000] {
                        for egg in [0u8, 1] {
                            for timer in [-1i32, 0, 1, 2] {
                                let prepare = |animal: ZTAnimal| {
                                    HABITAT.store(habitat, Ordering::SeqCst);
                                    save_to_memory(animal.0 + animal_offsets::NOT_SELECTABLE, not_selectable);
                                    save_to_memory(animal.0 + animal_offsets::HEALTH, health);
                                    save_to_memory(animal.0 + animal_offsets::EGG, egg);
                                    save_to_memory(animal.0 + 0x398, timer);
                                };
                                poles(
                                    ty,
                                    &format!("doEggCheck habitat {habitat:#x} not_selectable {not_selectable} health {health} egg {egg} timer {timer}"),
                                    &prepare,
                                    &|animal, pole| unsafe {
                                        match pole {
                                            0 => real::do_egg_check(animal.0),
                                            1 => animal.do_egg_check(),
                                            _ => generated::DO_EGG_CHECK.hooked()(animal.0 as *const u32),
                                        }
                                    },
                                    &mut failures,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static IS_TANK: AtomicBool = AtomicBool::new(false);
static TYPE_OK: AtomicBool = AtomicBool::new(false);

extern "thiscall" fn fake_is_tank(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x20);
    0x1234_5600 | IS_TANK.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_type_ok(_this: u32, ty: u32) -> u32 {
    CALLS.lock().unwrap().push(0x28 + (ty != 0) as u32 * 0x1000);
    0x1234_5600 | TYPE_OK.load(Ordering::SeqCst) as u32
}

/// A habitat stand-in: a zeroed block whose vtable answers the two predicates `doMurkyWaterCheck` asks.
fn build_fake_habitat() -> u32 {
    let mut vtable = vec![0u32; 0x30];
    vtable[0x20 / 4] = fake_is_tank as *const () as usize as u32;
    vtable[0x28 / 4] = fake_type_ok as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    let block = Box::leak(vec![0u32; 0x1c0 / 4].into_boxed_slice());
    block[0] = vtable.as_ptr() as u32;
    block.as_ptr() as u32
}

fn murky_body(animal: ZTAnimal, pole: usize) {
    unsafe {
        match pole {
            0 => real::do_murky_water_check(animal.0),
            1 => animal.do_murky_water_check(),
            _ => generated::DO_MURKY_WATER_CHECK.hooked()(animal.0 as *const u32),
        }
    }
}

fn habitat_body(animal: ZTAnimal, pole: usize) {
    unsafe {
        match pole {
            0 => real::do_habitat_check(animal.0),
            1 => animal.do_habitat_check(),
            _ => generated::DO_HABITAT_CHECK.hooked()(animal.0 as *const u32),
        }
    }
}

/// `ZTANIMAL_HABITAT_CHECKS_LIVE`: `doHabitatCheck` over a live habitat and `doMurkyWaterCheck` over a scripted one.
pub(crate) fn run_ztanimal_habitat_checks_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_HABITAT_CHECKS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let live_habitat = live_animals().first().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).filter(|&h| h != 0);
    let fake = build_fake_habitat();
    let mut rng = Lcg(0x4a17);

    for &ty in &types {
        if let Some(habitat) = live_habitat {
            let probe = build_animal(ty);
            install_scripted_vtable(probe);
            HABITAT.store(habitat, Ordering::SeqCst);
            let rating = real::get_habitat_rating(probe.0, habitat);
            let minimum: i32 = get_from_memory(ty + 0x2bc);
            let dislike: i32 = get_from_memory(ty + 0x310);
            let worst_band = rating < minimum && rating.wrapping_mul(100) < dislike.wrapping_mul(minimum) && get_from_memory::<u8>(habitat + 0x2c) == 0;
            for countdown in [0i32, 1, 4] {
                for egg in [0u8, 1] {
                    for show in [false, true] {
                        // The worst band posts a zoo message through real vanilla code; keep fixtures out of it.
                        if worst_band && countdown == 0 && egg == 0 && !show {
                            continue;
                        }
                        for words in [0u32, 0xffff_ffff] {
                            let prepare = |animal: ZTAnimal| {
                                HABITAT.store(habitat, Ordering::SeqCst);
                                DOING_SHOW.store(show, Ordering::SeqCst);
                                save_to_memory(animal.0 + 0x314, countdown);
                                save_to_memory(animal.0 + animal_offsets::EGG, egg);
                                for offset in [0x298u32, 0x29c, 0x2a0, 0x2a4] {
                                    save_to_memory(animal.0 + offset, words);
                                }
                                save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, 17i32);
                            };
                            poles(
                                ty,
                                &format!("doHabitatCheck countdown {countdown} egg {egg} show {show} words {words:#x} rating {rating}"),
                                &prepare,
                                &habitat_body,
                                &mut failures,
                            );
                        }
                    }
                }
            }
        }

        let thresholds: Vec<i32> = [0x42cu32, 0x428, 0x424, 0x35c, 0x360].iter().map(|&offset| get_from_memory::<i32>(ty + offset)).collect();
        let mut murks: Vec<i32> = vec![0, i32::MIN, i32::MAX];
        murks.extend(thresholds.iter().flat_map(|&t| [t.wrapping_sub(1), t, t.wrapping_add(1)]));
        for &murk in &murks {
            for tank in [false, true] {
                for type_ok in [false, true] {
                    for show in [false, true] {
                        for field4 in [0u32, 1] {
                            let value188 = if rng.next() & 1 == 0 { thresholds[3].wrapping_sub(1) } else { thresholds[4].wrapping_add(1) };
                            let prepare = |animal: ZTAnimal| {
                                IS_TANK.store(tank, Ordering::SeqCst);
                                TYPE_OK.store(type_ok, Ordering::SeqCst);
                                DOING_SHOW.store(show, Ordering::SeqCst);
                                HABITAT.store(fake, Ordering::SeqCst);
                                save_to_memory(fake + 0x1a8, murk);
                                save_to_memory(fake + 0x188, value188);
                                save_to_memory(fake + 4, field4);
                                save_to_memory(animal.0 + 0x2fc, 0i32);
                                save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, 5i32);
                                save_to_memory(animal.0 + animal_offsets::HEALTH_PENDING, 9i32);
                            };
                            poles(
                                ty,
                                &format!("doMurkyWaterCheck murk {murk} tank {tank} type_ok {type_ok} show {show} field4 {field4}"),
                                &prepare,
                                &murky_body,
                                &mut failures,
                            );
                        }
                    }
                }
            }
        }
        for countdown in [1i32, 3, -2] {
            let prepare = |animal: ZTAnimal| {
                IS_TANK.store(true, Ordering::SeqCst);
                HABITAT.store(fake, Ordering::SeqCst);
                save_to_memory(animal.0 + 0x2fc, countdown);
            };
            poles(ty, &format!("doMurkyWaterCheck countdown {countdown}"), &prepare, &murky_body, &mut failures);
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn hunger_body(animal: ZTAnimal, pole: usize) {
    unsafe {
        match pole {
            0 => real::do_hunger_check(animal.0),
            1 => animal.do_hunger_check(),
            _ => generated::DO_HUNGER_CHECK.hooked()(animal.0 as *const u32),
        }
    }
}

/// `ZTANIMAL_HUNGER_CHECK_LIVE`: `doHungerCheck` over a live habitat. `fEat`, `putDownDirt` and the zoo message run real
/// vanilla code, so fixtures keep `+0x3a9` clear, the dirt counter below its threshold and the message gate closed.
pub(crate) fn run_ztanimal_hunger_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_HUNGER_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    let live_habitat = live_animals().first().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).filter(|&h| h != 0);
    let Some(habitat) = live_habitat.filter(|_| !types.is_empty()) else {
        failures.push("the live world holds no animal type or habitat".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    for &ty in &types {
        let hungry_at: i32 = get_from_memory(ty + 0x20c);
        let starving_at: i32 = get_from_memory(ty + 0x240);
        let hungers = [hungry_at.wrapping_sub(1), hungry_at.wrapping_add(1), starving_at.wrapping_add(1)];
        for egg in [0u8, 1] {
            for countdown in [0i32, 2] {
                for eating in [0u8, 1] {
                    for &hunger in &hungers {
                        for keeper_food_flag in [0u8, 1] {
                            for blocked in [0u8, 1] {
                                for goal in [1i32, 0x72, 0x85] {
                                    for no_food_seen in [0u8, 1] {
                                        for show in [false, true] {
                                            for gate_byte in [0u8, 1] {
                                                let would_message = !show && goal != 0x85 && gate_byte == 0 && no_food_seen == 1;
                                                if would_message {
                                                    continue;
                                                }
                                                let prepare = |animal: ZTAnimal| {
                                                    HABITAT.store(habitat, Ordering::SeqCst);
                                                    DOING_SHOW.store(show, Ordering::SeqCst);
                                                    save_to_memory(animal.0 + animal_offsets::EGG, egg);
                                                    save_to_memory(animal.0 + 0x2f4, countdown);
                                                    save_to_memory(animal.0 + 0x3a8, eating);
                                                    save_to_memory(animal.0 + 0x3a9, 0u8);
                                                    save_to_memory(animal.0 + animal_offsets::HUNGER, hunger);
                                                    save_to_memory(animal.0 + 0x39f, keeper_food_flag);
                                                    save_to_memory(animal.0 + 0x3b0, blocked);
                                                    save_to_memory(animal.0 + 0x170, goal);
                                                    save_to_memory(animal.0 + 0x39e, no_food_seen);
                                                    save_to_memory(animal.0 + 0x24f, gate_byte);
                                                    save_to_memory(animal.0 + 0x2d4, 3i32);
                                                    save_to_memory(animal.0 + 0x2c8, i32::MIN);
                                                    save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, 11i32);
                                                    save_to_memory(animal.0 + animal_offsets::HEALTH_PENDING, 13i32);
                                                };
                                                poles(
                                                    ty,
                                                    &format!(
                                                        "doHungerCheck egg {egg} countdown {countdown} eating {eating} hunger {hunger} keeper_flag {keeper_food_flag} blocked {blocked} goal {goal:#x} no_food_seen {no_food_seen} show {show} gate {gate_byte}"
                                                    ),
                                                    &prepare,
                                                    &hunger_body,
                                                    &mut failures,
                                                );
                                                if failures.len() > 40 {
                                                    return finish_test(test_name, failures, failure_log);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn status_body(animal: ZTAnimal, pole: usize) {
    unsafe {
        match pole {
            0 => real::update_status_variables(animal.0),
            1 => animal.update_status_variables(),
            _ => generated::UPDATE_STATUS_VARIABLES.hooked()(animal.0 as *const u32),
        }
    }
}

/// `ZTANIMAL_UPDATE_STATUS_LIVE`: `updateStatusVariables` over random values around each clamp. Health never reaches the
/// type minimum, which would run the still-vanilla `setTerrainMode` over a fixture.
pub(crate) fn run_ztanimal_update_status_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_UPDATE_STATUS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let mut rng = Lcg(0x57a7);
    // (value, pending, type max, type min) for happiness, hunger, the two unnamed needs.
    let needs = [(0x2a8u32, 0x2acu32, 0x230u32, 0x22cu32), (0x2b0, 0x2b4, 0x240, 0x244), (0x2c0, 0x2c4, 0x248, 0x24c), (0x2c8, 0x2cc, 0x250, 0x254)];
    for &ty in &types {
        for round in 0..400 {
            let egg = (rng.next() & 1) as u8;
            let mut picks: Vec<(u32, i32, u32, i32)> = Vec::new();
            for &(value, pending, max, min) in &needs {
                let (max_v, min_v): (i32, i32) = (get_from_memory(ty + max), get_from_memory(ty + min));
                let anchors = [min_v.wrapping_sub(5), min_v, max_v.wrapping_div(2), max_v, max_v.wrapping_add(5), i32::MAX, i32::MIN];
                let deltas = [-7i32, 0, 7, 1_000_000, -1_000_000, i32::MAX];
                picks.push((value, anchors[(rng.next() as usize) % anchors.len()], pending, deltas[(rng.next() as usize) % deltas.len()]));
            }
            let health_max: i32 = get_from_memory(ty + 0x234);
            let health_min: i32 = get_from_memory(ty + 0x238);
            let health = [health_min.wrapping_add(5), health_max, health_max.wrapping_add(9)][(rng.next() as usize) % 3];
            let health_delta = [-2i32, 0, 3, 1_000_000][(rng.next() as usize) % 4];
            let prepare = |animal: ZTAnimal| {
                save_to_memory(animal.0 + animal_offsets::EGG, egg);
                for &(value, v, pending, d) in &picks {
                    save_to_memory(animal.0 + value, v);
                    save_to_memory(animal.0 + pending, d);
                }
                save_to_memory(animal.0 + animal_offsets::HEALTH, health);
                save_to_memory(animal.0 + animal_offsets::HEALTH_PENDING, health_delta);
            };
            poles(ty, &format!("updateStatusVariables round {round} egg {egg} needs {picks:?} health {health} delta {health_delta}"), &prepare, &status_body, &mut failures);
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static PRESSING: AtomicBool = AtomicBool::new(false);
static FIND0: AtomicU32 = AtomicU32::new(0);
static FIND1: AtomicU32 = AtomicU32::new(0);
static PLAYMATE: AtomicU32 = AtomicU32::new(0);
static CAN_DRINK: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_pressing(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x288);
    0x5500 | PRESSING.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_find_predator(_this: u32, arg: u32) -> u32 {
    CALLS.lock().unwrap().push(0x2fc | arg << 16);
    if arg == 0 { FIND0.load(Ordering::SeqCst) } else { FIND1.load(Ordering::SeqCst) }
}

extern "thiscall" fn fake_find_playmate(_this: u32, arg: u32) -> u32 {
    CALLS.lock().unwrap().push(0x2f8 | arg << 16);
    PLAYMATE.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_can_drink(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x2c8);
    CAN_DRINK.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_clear_goal_slot(_this: u32) {
    CALLS.lock().unwrap().push(0x174);
}

fn install_bored_vtable(animal: ZTAnimal) {
    install_scripted_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    vtable[0x288 / 4] = fake_pressing as *const () as usize as u32;
    vtable[0x2fc / 4] = fake_find_predator as *const () as usize as u32;
    vtable[0x2f8 / 4] = fake_find_playmate as *const () as usize as u32;
    vtable[0x2c8 / 4] = fake_can_drink as *const () as usize as u32;
    vtable[0x174 / 4] = fake_clear_goal_slot as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

fn bored_body(animal: ZTAnimal, pole: usize) {
    unsafe {
        match pole {
            0 => real::do_bored_check(animal.0),
            1 => animal.do_bored_check(),
            _ => generated::DO_BORED_CHECK.hooked()(animal.0 as *const u32),
        }
    }
}

/// `ZTANIMAL_BORED_CHECK_LIVE`: `doBoredCheck` with scripted answers for the vtable slots it asks, over a live habitat (and
/// none). The sleep hand-over and the chase-playmate abort run real vanilla code, so fixtures stay below the type's sleep
/// need and off goal `0x74`. The shared RNG state is seeded per scenario and compared afterwards.
pub(crate) fn run_ztanimal_bored_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_BORED_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let live_habitat = live_animals().first().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).filter(|&h| h != 0);
    let habitats: Vec<u32> = std::iter::once(0).chain(live_habitat).collect();
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let mut rng = Lcg(0xb04e);
    for &ty in &types {
        let sleep_at: i32 = get_from_memory(ty + 0x324);
        let hungry_at: i32 = get_from_memory(ty + 0x20c);
        for round in 0..300 {
            let habitat = habitats[(rng.next() as usize) % habitats.len()];
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let (egg, show, sick, fleeing, playmate, escaped, blocked, pressing) =
                (pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
            let (find0, find1, find_playmate, can_drink) = (rng.next() & 1, rng.next() & 1, rng.next() & 1, rng.next() & 1);
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let hunger = if pick(&mut rng) { hungry_at.wrapping_sub(1) } else { hungry_at.wrapping_add(1) };
            let goal = [1i32, 0x75, 0x20][(rng.next() as usize) % 3];
            let mode = [0i32, 1, 2, 3][(rng.next() as usize) % 4];
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x1234;
            let words = rng.next();
            let prepare = |animal: ZTAnimal| {
                install_bored_vtable(animal);
                HABITAT.store(habitat, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                PRESSING.store(pressing, Ordering::SeqCst);
                FIND0.store(find0, Ordering::SeqCst);
                FIND1.store(find1, Ordering::SeqCst);
                PLAYMATE.store(find_playmate, Ordering::SeqCst);
                CAN_DRINK.store(can_drink, Ordering::SeqCst);
                save_to_memory(rng_address, seed);
                save_to_memory(animal.0 + animal_offsets::EGG, egg as u8);
                save_to_memory(animal.0 + 0x318, countdown);
                save_to_memory(animal.0 + 0x2c0, sleep_at.wrapping_sub(5));
                save_to_memory(animal.0 + animal_offsets::SICK, sick as u8);
                save_to_memory(animal.0 + 0x24c, fleeing as u8);
                save_to_memory(animal.0 + 0x3a4, playmate as u8);
                save_to_memory(animal.0 + 0x3a3, escaped as u8);
                save_to_memory(animal.0 + 0x3b0, blocked as u8);
                save_to_memory(animal.0 + 0x2b0, hunger);
                save_to_memory(animal.0 + 0x170, goal);
                save_to_memory(animal.0 + 0x380, mode);
                for offset in [0x298u32, 0x29c, 0x2a0, 0x2a4] {
                    save_to_memory(animal.0 + offset, words);
                }
            };
            // `poles` builds its own fixtures with the plain scripted vtable, so run the three poles here.
            let mut logs = Vec::new();
            let mut snapshots = Vec::new();
            for pole in 0..3usize {
                let animal = build_animal(ty);
                prepare(animal);
                CALLS.lock().unwrap().clear();
                bored_body(animal, pole);
                logs.push(CALLS.lock().unwrap().clone());
                let mut snap = snapshot(animal);
                snap.extend(get_from_memory::<u32>(rng_address).to_le_bytes());
                snapshots.push(snap);
            }
            let label = format!(
                "doBoredCheck round {round} habitat {habitat:#x} egg {egg} show {show} sick {sick} fleeing {fleeing} playmate {playmate} escaped {escaped} blocked {blocked} pressing {pressing} find {find0}/{find1} mate {find_playmate} drink {can_drink} countdown {countdown} goal {goal:#x} mode {mode} seed {seed:#x}"
            );
            if logs[0] != logs[1] || logs[0] != logs[2] {
                failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if snapshots[0] != snapshots[pole] {
                    let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
                    failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
                }
            }
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn social_body(animal: ZTAnimal, pole: usize) {
    unsafe {
        match pole {
            0 => real::do_social_check(animal.0),
            1 => animal.do_social_check(),
            _ => generated::DO_SOCIAL_CHECK.hooked()(animal.0 as *const u32),
        }
    }
}

/// `ZTANIMAL_SOCIAL_CHECK_LIVE`: `doSocialCheck` over a live habitat, with random flag words, guest crowding, direction
/// components and egg/show states.
pub(crate) fn run_ztanimal_social_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_SOCIAL_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    let live_habitat = live_animals().first().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).filter(|&h| h != 0);
    let Some(habitat) = live_habitat.filter(|_| !types.is_empty()) else {
        failures.push("the live world holds no animal type or habitat".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut rng = Lcg(0x50c1);
    for &ty in &types {
        let tolerance: i32 = get_from_memory(ty + 0x268);
        for round in 0..300 {
            let egg = (rng.next() & 1) as u8;
            let show = rng.next() & 1 == 1;
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let guests = [tolerance.wrapping_sub(1), tolerance, tolerance.wrapping_add(1)][(rng.next() as usize) % 3];
            let a = [-3i32, 0, 4][(rng.next() as usize) % 3];
            let b = [-2i32, 0, 9][(rng.next() as usize) % 3];
            let words = [rng.next() << 8 | rng.next(), rng.next() << 8 | rng.next(), rng.next() << 8 | rng.next(), rng.next() << 8 | rng.next()];
            let prepare = |animal: ZTAnimal| {
                HABITAT.store(habitat, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                save_to_memory(animal.0 + animal_offsets::EGG, egg);
                save_to_memory(animal.0 + 0x324, countdown);
                save_to_memory(animal.0 + 0x368, guests);
                save_to_memory(animal.0 + 0x36c, a);
                save_to_memory(animal.0 + 0x370, b);
                save_to_memory(animal.0 + 0x37c, 0xaai32);
                for (offset, word) in [0x298u32, 0x29c, 0x2a0, 0x2a4].into_iter().zip(words) {
                    save_to_memory(animal.0 + offset, word);
                }
                save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, 3i32);
            };
            poles(
                ty,
                &format!("doSocialCheck round {round} egg {egg} show {show} countdown {countdown} guests {guests} dir {a}/{b} words {words:x?}"),
                &prepare,
                &social_body,
                &mut failures,
            );
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static WATER_SLOTS: [AtomicU32; 4] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

extern "thiscall" fn fake_wants_land(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x16c);
    0x7700 | WATER_SLOTS[0].load(Ordering::SeqCst)
}

extern "thiscall" fn fake_wants_water(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0xbc);
    0x7700 | WATER_SLOTS[1].load(Ordering::SeqCst)
}

extern "thiscall" fn fake_mode1_block(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0xc0);
    0x7700 | WATER_SLOTS[2].load(Ordering::SeqCst)
}

extern "thiscall" fn fake_mode2_block(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x170);
    0x7700 | WATER_SLOTS[3].load(Ordering::SeqCst)
}

fn install_water_vtable(animal: ZTAnimal) {
    install_scripted_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    vtable[0x16c / 4] = fake_wants_land as *const () as usize as u32;
    vtable[0xbc / 4] = fake_wants_water as *const () as usize as u32;
    vtable[0xc0 / 4] = fake_mode1_block as *const () as usize as u32;
    vtable[0x170 / 4] = fake_mode2_block as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

fn water_body(animal: ZTAnimal, pole: usize) {
    match pole {
        0 => real::do_water_check(animal.0),
        1 => animal.do_water_check(),
        _ => {
            unsafe { generated::DO_WATER_CHECK.hooked()(animal.0 as *const u32) };
        }
    }
}

/// `ZTANIMAL_WATER_CHECK_LIVE`: `doWaterCheck` with scripted slot answers and a seeded RNG (state compared afterwards).
/// `setTerrainMode` is still vanilla, so the habitat is the live one only when none of its tile counts exceeds the type's
/// thresholds; otherwise the habitat slot answers null and every switch falls through.
pub(crate) fn run_ztanimal_water_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_WATER_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let live_habitat = live_animals().first().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).filter(|&h| h != 0);
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let mut rng = Lcg(0x3a7e);
    for &ty in &types {
        let habitats: Vec<u32> = match live_habitat {
            Some(h) => {
                let habitat = unsafe { crate::util::ref_from_memory::<crate::zthabitat::ZTHabitat>(h) };
                let quiet = habitat.get_num_underwater_tiles() <= get_from_memory::<i32>(ty + 0x398)
                    && habitat.get_num_water_tiles() <= get_from_memory::<i32>(ty + 0x390)
                    && habitat.get_num_land_tiles() <= get_from_memory::<i32>(ty + 0x394);
                if quiet { vec![0, h] } else { vec![0] }
            }
            None => vec![0],
        };
        for round in 0..400 {
            let habitat = habitats[(rng.next() as usize) % habitats.len()];
            let pick = |rng: &mut Lcg| (rng.next() & 1) as u32;
            let slots = [pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng)];
            let egg = pick(&mut rng) as u8;
            let show = pick(&mut rng) == 1;
            let (sick, escaped) = (pick(&mut rng) as u8, pick(&mut rng) as u8);
            let mode = [0i32, 1, 2, 3][(rng.next() as usize) % 4];
            let countdown = [0i32, 0, 0, 1][(rng.next() as usize) % 4];
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x4321;
            let prepare = |animal: ZTAnimal| {
                install_water_vtable(animal);
                HABITAT.store(habitat, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                for (slot, value) in WATER_SLOTS.iter().zip(slots) {
                    slot.store(value, Ordering::SeqCst);
                }
                save_to_memory(rng_address, seed);
                save_to_memory(animal.0 + animal_offsets::EGG, egg);
                save_to_memory(animal.0 + 0x320, countdown);
                save_to_memory(animal.0 + 0x380, mode);
                save_to_memory(animal.0 + animal_offsets::SICK, sick);
                save_to_memory(animal.0 + 0x3a3, escaped);
            };
            let mut logs = Vec::new();
            let mut snapshots = Vec::new();
            for pole in 0..3usize {
                let animal = build_animal(ty);
                prepare(animal);
                CALLS.lock().unwrap().clear();
                water_body(animal, pole);
                logs.push(CALLS.lock().unwrap().clone());
                let mut snap = snapshot(animal);
                snap.extend(get_from_memory::<u32>(rng_address).to_le_bytes());
                snapshots.push(snap);
            }
            let label = format!(
                "doWaterCheck round {round} habitat {habitat:#x} slots {slots:?} egg {egg} show {show} sick {sick} escaped {escaped} mode {mode} countdown {countdown} seed {seed:#x}"
            );
            if logs[0] != logs[1] || logs[0] != logs[2] {
                failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if snapshots[0] != snapshots[pole] {
                    let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
                    failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
                }
            }
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

extern "thiscall" fn record0<const SLOT: u32>(_this: u32) {
    CALLS.lock().unwrap().push(SLOT);
}

extern "thiscall" fn record1<const SLOT: u32>(_this: u32, arg: u32) {
    CALLS.lock().unwrap().push(SLOT | arg << 16);
}

/// The slots `BFEntity::update` dispatches through become recorders (`0x40`/`0x44` take one argument), so the real
/// `BFEntity::update` runs over a fixture without touching animation or movement state.
fn install_update_vtable(animal: ZTAnimal) {
    install_scripted_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    vtable[0x10 / 4] = record0::<0x10> as *const () as usize as u32;
    vtable[0x14 / 4] = record0::<0x14> as *const () as usize as u32;
    vtable[0x2c / 4] = record0::<0x2c> as *const () as usize as u32;
    vtable[0x30 / 4] = record0::<0x30> as *const () as usize as u32;
    vtable[0x34 / 4] = record0::<0x34> as *const () as usize as u32;
    vtable[0x38 / 4] = record0::<0x38> as *const () as usize as u32;
    vtable[0x40 / 4] = record1::<0x40> as *const () as usize as u32;
    vtable[0x44 / 4] = record1::<0x44> as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

fn entries_snapshot(animal: ZTAnimal) -> Vec<u8> {
    let begin: u32 = get_from_memory(animal.0 + 0x260);
    let end: u32 = get_from_memory(animal.0 + 0x264);
    let mut bytes = (end.wrapping_sub(begin)).to_le_bytes().to_vec();
    for offset in (0..end.wrapping_sub(begin)).step_by(4) {
        bytes.extend(get_from_memory::<u32>(begin + offset).to_le_bytes());
    }
    bytes
}

/// `ZTANIMAL_UPDATE_LIVE`: `update` over random entry vectors and `updatePosition` for non-predator types. The real
/// `BFEntity::update` and `BFUnit::updatePosition` run with the entity's own vtable slots recorded.
pub(crate) fn run_ztanimal_update_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_UPDATE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let mut rng = Lcg(0x0dd5);
    for &ty in &types {
        for round in 0..300 {
            let count = (rng.next() % 6) as usize;
            let entries: Vec<(u32, i32)> = (0..count).map(|i| (i as u32 + 100, [0i32, 0x4e1, 0x4e0, 0x4e2, 1000, 0x7fff_fff0][(rng.next() as usize) % 6])).collect();
            let flag = (rng.next() & 3 != 0) as u8;
            let dt = [0u32, 1, 2, 5, 20][(rng.next() as usize) % 5];
            let prepare_entries = |animal: ZTAnimal| {
                let block: &'static mut [u32] = Box::leak(vec![0u32; entries.len() * 2 + 2].into_boxed_slice());
                for (i, &(id, age)) in entries.iter().enumerate() {
                    block[i * 2] = id;
                    block[i * 2 + 1] = age as u32;
                }
                let begin = block.as_ptr() as u32;
                save_to_memory(animal.0 + 0x260, begin);
                save_to_memory(animal.0 + 0x264, begin + entries.len() as u32 * 8);
                save_to_memory(animal.0 + 0x392, flag);
                save_to_memory(animal.0 + 0x14c, 0i32);
                save_to_memory(animal.0 + 0x148, 0i32);
            };
            let mut logs = Vec::new();
            let mut snapshots = Vec::new();
            for pole in 0..3usize {
                let animal = build_animal(ty);
                install_update_vtable(animal);
                prepare_entries(animal);
                CALLS.lock().unwrap().clear();
                match pole {
                    0 => real::update(animal.0, dt),
                    1 => animal.update(dt),
                    _ => unsafe { generated::UPDATE.hooked()(animal.0 as *const u32, dt) },
                }
                logs.push(CALLS.lock().unwrap().clone());
                let mut snap = snapshot(animal);
                snap.extend(entries_snapshot(animal));
                snapshots.push(snap);
            }
            let label = format!("update round {round} entries {entries:x?} flag {flag} dt {dt}");
            if logs[0] != logs[1] || logs[0] != logs[2] {
                failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if snapshots[0] != snapshots[pole] {
                    let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
                    failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
                }
            }
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
        // `updatePosition` marks the shared map tile for a man-eater, so only types that do not prey on man run it.
        if !ZTAnimal(build_animal(ty).0).preys_on_man() {
            for arg in [0u32, 1] {
                let mut snapshots = Vec::new();
                for pole in 0..3usize {
                    let animal = build_animal(ty);
                    install_update_vtable(animal);
                    match pole {
                        0 => real::update_position(animal.0, arg),
                        1 => animal.update_position(arg),
                        _ => unsafe { generated::UPDATE_POSITION.hooked()(animal.0 as *const u32, arg) },
                    }
                    snapshots.push(snapshot(animal));
                }
                if snapshots[0] != snapshots[1] || snapshots[0] != snapshots[2] {
                    failures.push(format!("updatePosition arg {arg}: state differs between real, port and hooked"));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static SLOT_AC: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_slot_ac(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0xac);
    0x3300 | SLOT_AC.load(Ordering::SeqCst)
}

fn install_conditionals_vtable(animal: ZTAnimal) {
    install_scripted_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    vtable[0x48 / 4] = record1::<0x48> as *const () as usize as u32;
    vtable[0x10c / 4] = record0::<0x10c> as *const () as usize as u32;
    vtable[0xac / 4] = fake_slot_ac as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// `ZTANIMAL_CONDITIONALS_LIVE`: `doConditionals` over a live habitat. Every callee check runs for real (ported or
/// vanilla) on the fixture, so each check's own countdown is set far from firing and `+0x3a9` (which reaches the
/// still-vanilla `fEat`/`stopEating`) stays clear; the three countdowns `doConditionals` owns, the boxed/egg/not-selectable
/// states, the keeper-visit and death-arrived bytes and the vtable answers are varied. Happiness stays high so the dirt
/// and message branches (real vanilla calls) are not reached.
pub(crate) fn run_ztanimal_conditionals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_CONDITIONALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    let live_habitat = live_animals().first().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).filter(|&h| h != 0);
    let Some(habitat) = live_habitat.filter(|_| !types.is_empty()) else {
        failures.push("the live world holds no animal type or habitat".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let live_position = live_animals()[0];
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let die: &'static [u8; 4] = Box::leak(Box::new(*b"bDie"));
    let mut rng = Lcg(0xc07d);
    let far_countdowns = [0x2f4u32, 0x2f8, 0x2fc, 0x300, 0x304, 0x308, 0x30c, 0x314, 0x318, 0x31c, 0x320, 0x324, 0x328, 0x32c, 0x330, 0x344, 0x354, 0x358, 0x35c, 0x364];
    for &ty in &types {
        for round in 0..300 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let boxed = rng.next() % 5 == 0;
            let egg = rng.next() % 4 == 0;
            let not_selectable = rng.next() % 4 == 0;
            let (keeper_visit, death_arrived, show, slot_ac) = (pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
            let pressure = [0i32, 0, 4][(rng.next() as usize) % 3];
            let message = [0i32, 3][(rng.next() as usize) % 2];
            let aimgr_countdown = [0i32, 1, 5][(rng.next() as usize) % 3];
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x6677;
            let prepare = |animal: ZTAnimal| {
                install_conditionals_vtable(animal);
                HABITAT.store(habitat, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                SLOT_AC.store(slot_ac as u32, Ordering::SeqCst);
                save_to_memory(rng_address, seed);
                for offset in far_countdowns {
                    save_to_memory(animal.0 + offset, 1_000_000i32);
                }
                save_to_memory(animal.0 + 0x310, pressure);
                save_to_memory(animal.0 + 0x33c, message);
                save_to_memory(animal.0 + 0x340, aimgr_countdown);
                save_to_memory(animal.0 + 0x398, 1_000_000i32);
                save_to_memory(animal.0 + animal_offsets::BOXED, boxed as u8);
                save_to_memory(animal.0 + animal_offsets::EGG, egg as u8);
                save_to_memory(animal.0 + animal_offsets::NOT_SELECTABLE, not_selectable as u8);
                save_to_memory(animal.0 + 0x39c, keeper_visit as u8);
                save_to_memory(animal.0 + 0x39d, death_arrived as u8);
                save_to_memory(animal.0 + 0x3a9, 0u8);
                save_to_memory(animal.0 + 0x3a8, 0u8);
                save_to_memory(animal.0 + animal_offsets::HAPPINESS, 10_000i32);
                save_to_memory(animal.0 + animal_offsets::HAPPINESS_PENDING, 7i32);
                // Vanilla dereferences the habitat cell of the animal's tile, so the fixture stands where a live animal does.
                for offset in [0x114u32, 0x118, 0x11c] {
                    save_to_memory(animal.0 + offset, get_from_memory::<u32>(live_position + offset));
                }
                save_to_memory(animal.0 + 0x54, die.as_ptr() as u32);
                save_to_memory(animal.0 + 0x58, die.as_ptr() as u32 + 4);
                for offset in [0x298u32, 0x29c, 0x2a0, 0x2a4] {
                    save_to_memory(animal.0 + offset, 0xffff_ffffu32);
                }
            };
            let mut logs = Vec::new();
            let mut snapshots = Vec::new();
            for pole in 0..3usize {
                let animal = build_animal(ty);
                prepare(animal);
                CALLS.lock().unwrap().clear();
                match pole {
                    0 => real::do_conditionals(animal.0),
                    1 => animal.do_conditionals(),
                    _ => unsafe { generated::DO_CONDITIONALS.hooked()(animal.0 as *const u32) },
                }
                logs.push(CALLS.lock().unwrap().clone());
                let mut snap = snapshot(animal);
                snap.extend(get_from_memory::<u32>(rng_address).to_le_bytes());
                snapshots.push(snap);
            }
            let label = format!(
                "doConditionals round {round} boxed {boxed} egg {egg} not_selectable {not_selectable} keeper {keeper_visit} death {death_arrived} show {show} slot_ac {slot_ac} countdowns {pressure}/{message}/{aimgr_countdown}"
            );
            if logs[0] != logs[1] || logs[0] != logs[2] {
                failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if snapshots[0] != snapshots[pole] {
                    let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
                    failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
                }
            }
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn install_building_vtable(animal: ZTAnimal) {
    install_scripted_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    vtable[0x2e4 / 4] = record1::<0x2e4> as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// `ZTANIMAL_BUILDING_USE_LIVE`: `doBuildingUseCheck` with the fixture standing on a live animal's position, over random
/// seeds, need words and destination positions. `isNeedOrFunSatisfier` and `findNearestSatisfier` run for real on both
/// sides; vtable `+0x2e4` is recorded.
pub(crate) fn run_ztanimal_building_use_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_BUILDING_USE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    let live = live_animals();
    if types.is_empty() || live.is_empty() {
        failures.push("the live world holds no animal type or animal".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let mut rng = Lcg(0xb11d);
    for &ty in &types {
        for round in 0..400 {
            let source = live[(rng.next() as usize) % live.len()];
            let countdown = [0i32, 0, 0, 1][(rng.next() as usize) % 4];
            let need_word = (rng.next() % 6) as i32;
            let position = [(rng.next() % 4000) as i32 - 2000, (rng.next() % 4000) as i32 - 2000, (rng.next() % 200) as i32 - 100];
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x9911;
            let prepare = |animal: ZTAnimal| {
                install_building_vtable(animal);
                save_to_memory(rng_address, seed);
                for offset in [0x114u32, 0x118, 0x11c] {
                    save_to_memory(animal.0 + offset, get_from_memory::<u32>(source + offset));
                }
                save_to_memory(animal.0 + 0x344, countdown);
                save_to_memory(animal.0 + 0x244, need_word);
                for (i, value) in position.iter().enumerate() {
                    save_to_memory(animal.0 + 0x214 + i as u32 * 4, *value);
                }
                save_to_memory(animal.0 + 0x24e, 0x77u8);
            };
            let mut logs = Vec::new();
            let mut snapshots = Vec::new();
            for pole in 0..3usize {
                let animal = build_animal(ty);
                prepare(animal);
                CALLS.lock().unwrap().clear();
                match pole {
                    0 => real::do_building_use_check(animal.0),
                    1 => animal.do_building_use_check(),
                    _ => {
                        unsafe { generated::DO_BUILDING_USE_CHECK.hooked()(animal.0 as *const u32) };
                    }
                }
                logs.push(CALLS.lock().unwrap().clone());
                let mut snap = snapshot(animal);
                snap.extend((0x210u32..0x260).map(|offset| get_from_memory::<u8>(animal.0 + offset)));
                snap.extend(get_from_memory::<u32>(rng_address).to_le_bytes());
                snapshots.push(snap);
            }
            let label = format!("doBuildingUseCheck round {round} countdown {countdown} need {need_word} position {position:?} seed {seed:#x}");
            if logs[0] != logs[1] || logs[0] != logs[2] {
                failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if snapshots[0] != snapshots[pole] {
                    let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
                    failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
                }
            }
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn install_escaped_vtable(animal: ZTAnimal) {
    install_bored_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    vtable[0x2c4 / 4] = record0::<0x2c4> as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// `ZTANIMAL_ESCAPED_CHECK_LIVE`: `doEscapedCheck` with scripted predator/playmate/pressing-need answers, over random
/// rampage state, happiness, countdown and the behaviour name (including the two rampage scripts, whose stop path runs
/// the real `clearQueue`/`clearBS`/`stop` on both sides). The habitat slot answers null or a block with its `+0x2c`
/// byte clear and the escaped byte stays clear, so `setEscaped`'s escaped list, `setTerrainMode` and `ZooStatus` are not
/// touched; rampage starts happen only for types whose chance is positive.
pub(crate) fn run_ztanimal_escaped_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_ESCAPED_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let outside: u32 = Box::leak(vec![0u32; 0x40].into_boxed_slice()).as_ptr() as u32;
    let names: [&'static [u8]; 3] = [Box::leak(Box::new(*b"bEscapedRampage")), Box::leak(Box::new(*b"bHabitatRampage")), Box::leak(Box::new(*b"bIdle"))];
    let mut rng = Lcg(0xe5c4);
    for &ty in &types {
        let threshold: i32 = get_from_memory(ty + 0x3f4);
        let max: i32 = get_from_memory(ty + 0x400);
        for round in 0..400 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let habitat = [0u32, outside][(rng.next() as usize) % 2];
            let rampaging = pick(&mut rng);
            let counter = [0i32, max, max.wrapping_add(1), 3][(rng.next() as usize) % 4];
            let happiness = [threshold.wrapping_sub(1), threshold, threshold.wrapping_add(1)][(rng.next() as usize) % 3];
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let name = names[(rng.next() as usize) % 3];
            let (egg, show, fleeing, playmate, chase_aux, stage, pressing) = (
                rng.next() % 6 == 0,
                pick(&mut rng),
                pick(&mut rng),
                pick(&mut rng),
                pick(&mut rng),
                pick(&mut rng),
                pick(&mut rng),
            );
            let (find0, find1, mate) = (rng.next() & 1, rng.next() & 1, rng.next() & 1);
            let goal = [1i32, 0x74, 0x75][(rng.next() as usize) % 3];
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x5151;
            let prepare = |animal: ZTAnimal| {
                install_escaped_vtable(animal);
                HABITAT.store(habitat, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                PRESSING.store(pressing, Ordering::SeqCst);
                FIND0.store(find0, Ordering::SeqCst);
                FIND1.store(find1, Ordering::SeqCst);
                PLAYMATE.store(mate, Ordering::SeqCst);
                save_to_memory(rng_address, seed);
                save_to_memory(animal.0 + 0x3b0, rampaging as u8);
                save_to_memory(animal.0 + 0x3b1, 1u8);
                save_to_memory(animal.0 + 0x35c, counter);
                save_to_memory(animal.0 + animal_offsets::HAPPINESS, happiness);
                save_to_memory(animal.0 + 0x304, countdown);
                save_to_memory(animal.0 + animal_offsets::EGG, egg as u8);
                save_to_memory(animal.0 + 0x24c, fleeing as u8);
                save_to_memory(animal.0 + 0x3a4, playmate as u8);
                save_to_memory(animal.0 + 0x3a5, chase_aux as u8);
                save_to_memory(animal.0 + 0x393, stage as u8);
                save_to_memory(animal.0 + 0x3a3, 0u8);
                save_to_memory(animal.0 + 0x170, goal);
                save_to_memory(animal.0 + 0x22c, 0xdeadu32);
                // The stop path frees the name through vanilla's pool, so each fixture gets its own pool block.
                let buffer = unsafe { openzt_detour::generated::poolalloc::ALLOCATE.original()(16) } as u32;
                for (i, byte) in name.iter().enumerate() {
                    save_to_memory(buffer + i as u32, *byte);
                }
                save_to_memory(animal.0 + 0x54, buffer);
                save_to_memory(animal.0 + 0x58, buffer + name.len() as u32);
                save_to_memory(animal.0 + 0x5c, buffer + 16);
            };
            let mut logs = Vec::new();
            let mut snapshots = Vec::new();
            for pole in 0..3usize {
                let animal = build_animal(ty);
                prepare(animal);
                CALLS.lock().unwrap().clear();
                match pole {
                    0 => real::do_escaped_check(animal.0),
                    1 => animal.do_escaped_check(),
                    _ => unsafe { generated::DO_ESCAPED_CHECK.hooked()(animal.0 as *const u32) },
                }
                logs.push(CALLS.lock().unwrap().clone());
                let mut snap = snapshot(animal);
                snap.extend(get_from_memory::<u32>(animal.0 + 0x22c).to_le_bytes());
                snap.extend(get_from_memory::<u32>(rng_address).to_le_bytes());
                snapshots.push(snap);
            }
            let label = format!(
                "doEscapedCheck round {round} habitat {habitat:#x} rampaging {rampaging} counter {counter} happiness {happiness} countdown {countdown} name {} egg {egg} show {show} fleeing {fleeing} playmate {playmate} aux {chase_aux} stage {stage} pressing {pressing} find {find0}/{find1} mate {mate} goal {goal:#x} seed {seed:#x}",
                String::from_utf8_lossy(name)
            );
            if logs[0] != logs[1] || logs[0] != logs[2] {
                failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if snapshots[0] != snapshots[pole] {
                    let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
                    failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
                }
            }
            if failures.len() > 20 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static SLEEP_GATE: AtomicBool = AtomicBool::new(false);

extern "thiscall" fn fake_sleep_gate(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x170);
    0x7700 | SLEEP_GATE.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_stop_everything(_this: u32, a: u32, b: u32, c: u32) {
    CALLS.lock().unwrap().push(0x198 | a << 12 | b << 13 | c << 14);
}

fn install_sleep_vtable(animal: ZTAnimal) {
    install_scripted_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory::<u32>(current + slot * 4)).collect();
    vtable[0x170 / 4] = fake_sleep_gate as *const () as usize as u32;
    vtable[0x288 / 4] = fake_pressing as *const () as usize as u32;
    vtable[0x198 / 4] = fake_stop_everything as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// The queued call's text, the set name's text and the queued call's flag byte.
fn queue_snapshot(animal: ZTAnimal) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (begin, end) in [(0xdcu32, 0xe0u32), (0x54, 0x58)] {
        let (b, e): (u32, u32) = (get_from_memory(animal.0 + begin), get_from_memory(animal.0 + end));
        bytes.extend((0..e.wrapping_sub(b)).map(|i| get_from_memory::<u8>(b + i)));
        bytes.push(0xff);
    }
    bytes.push(get_from_memory::<u8>(animal.0 + 0xe8));
    bytes
}

fn plant_set_name(animal: ZTAnimal, text: &[u8]) {
    let buffer: &'static mut [u8] = Box::leak(text.to_vec().into_boxed_slice());
    let begin = buffer.as_ptr() as u32;
    save_to_memory(animal.0 + 0x54, begin);
    save_to_memory(animal.0 + 0x58, begin + text.len() as u32);
}

fn plant_queued_call(animal: ZTAnimal, text: &[u8]) {
    let begin: u32 = get_from_memory(animal.0 + 0xdc);
    for (i, byte) in text.iter().chain(std::iter::once(&0u8)).enumerate() {
        save_to_memory(begin + i as u32, *byte);
    }
    save_to_memory(animal.0 + 0xe0, begin + text.len() as u32);
}

/// Runs the three poles over fixtures built by `install` and `prepare`, and records any difference in the slot calls,
/// the state, the queued call and the shared RNG word. `seed` is written to the RNG before each pole.
fn compare_three_poles(
    ty: u32,
    label: &str,
    seed: u32,
    install: &dyn Fn(ZTAnimal),
    prepare: &dyn Fn(ZTAnimal),
    body: &dyn Fn(ZTAnimal, usize),
    failures: &mut Vec<String>,
) {
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let mut logs = Vec::new();
    let mut snapshots = Vec::new();
    for pole in 0..3usize {
        let animal = build_animal(ty);
        install(animal);
        prepare(animal);
        save_to_memory(rng_address, seed);
        CALLS.lock().unwrap().clear();
        body(animal, pole);
        logs.push(CALLS.lock().unwrap().clone());
        let mut snap = snapshot(animal);
        snap.extend(queue_snapshot(animal));
        snap.extend(get_from_memory::<u32>(animal.0 + 0x128).to_le_bytes());
        snap.extend(get_from_memory::<u32>(rng_address).to_le_bytes());
        snapshots.push(snap);
    }
    if logs[0] != logs[1] || logs[0] != logs[2] {
        failures.push(format!("{label}: slot calls real {:x?}, port {:x?}, hooked {:x?}", logs[0], logs[1], logs[2]));
    }
    for (pole, name) in [(1usize, "port"), (2, "hooked")] {
        if snapshots[0] != snapshots[pole] {
            let first = snapshots[0].iter().zip(&snapshots[pole]).position(|(a, b)| a != b);
            failures.push(format!("{label}: state differs from real for {name} (first differing snapshot byte {first:?})"));
        }
    }
}

/// `ZTANIMAL_SLEEP_CHECK_LIVE`: `doSleepCheck` over awake and sleeping fixtures with the sleep gate, need, escape and
/// playmate bytes, the set name and the queued call varied. The queued text and the RNG word are compared.
pub(crate) fn run_ztanimal_sleep_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_SLEEP_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let mut rng = Lcg(0x51ee);
    let names: [&[u8]; 3] = [b"", b"bSleep", b"bWalk"];
    for &ty in &types {
        let sleep_at: i32 = get_from_memory(ty + 0x324);
        for round in 0..400 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let (asleep, gate, egg, show, escaped, arrived, playmate) =
                (pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let need = [sleep_at.wrapping_sub(1), sleep_at, sleep_at.wrapping_add(7)][(rng.next() as usize) % 3];
            let rate = (rng.next() % 9) as i32 - 3;
            let set_name = names[(rng.next() as usize) % 3];
            let queued = names[(rng.next() as usize) % 3];
            let queued_flag = (rng.next() & 1) as u8;
            let prepare = |animal: ZTAnimal| {
                SLEEP_GATE.store(gate, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                save_to_memory(animal.0 + 0x3a8, asleep as u8);
                save_to_memory(animal.0 + 0x300, countdown);
                save_to_memory(animal.0 + 0x2c0, need);
                save_to_memory(animal.0 + 0x2d8, rate);
                save_to_memory(animal.0 + animal_offsets::EGG, egg as u8);
                save_to_memory(animal.0 + 0x3a3, escaped as u8);
                save_to_memory(animal.0 + 0x393, arrived as u8);
                save_to_memory(animal.0 + 0x3a4, playmate as u8);
                plant_set_name(animal, set_name);
                plant_queued_call(animal, queued);
                save_to_memory(animal.0 + 0xe8, queued_flag);
            };
            compare_three_poles(
                ty,
                &format!("doSleepCheck round {round} asleep {asleep} gate {gate} egg {egg} show {show} escaped {escaped}/{arrived} playmate {playmate} countdown {countdown} need {need} set {set_name:?} queued {queued:?}/{queued_flag}"),
                0x1357,
                &install_sleep_vtable,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_sleep_check(animal.0),
                        1 => animal.do_sleep_check(),
                        _ => generated::DO_SLEEP_CHECK.hooked()(animal.0 as *const u32),
                    }
                },
                &mut failures,
            );
            if failures.len() > 20 {
                break;
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// Answers `setBS` for `doZapCheck`'s `setBSNew`, recording the name length and flags; `SLEEP_GATE` is the answer.
fn scripted_zap_set_bs(_this: u32, begin: u32, end: u32, call_flag: i8, force: i8, mode: i32) -> Option<bool> {
    CALLS.lock().unwrap().push(0xb500_0000 | (end.wrapping_sub(begin) << 12) | (call_flag as u8 as u32) << 8 | (force as u8 as u32) << 4 | (mode as u32 & 0xf));
    Some(SLEEP_GATE.load(Ordering::SeqCst))
}

/// A position on which `FUN_004a3383` finds a direction for an animal of `ty`, if any tile of the loaded map offers one.
fn find_zap_position(ty: u32) -> Option<[i32; 3]> {
    let world: u32 = get_from_memory(super::ztanimal::base() + 0x0023_8040);
    let find = unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32, u32, u32) -> u32>(0x004a_3383) };
    let probe = build_animal(ty);
    for tile_y in 0..125i32 {
        for tile_x in 0..125i32 {
            let position = [tile_x * 64 + 32, tile_y * 64 + 32, 0];
            for (i, word) in position.iter().enumerate() {
                save_to_memory(probe.0 + 0x114 + 4 * i as u32, *word);
            }
            if unsafe { find(world, probe.0, probe.0 + 0x114) } != 0xffff_ffff {
                return Some(position);
            }
        }
    }
    None
}

/// `ZTANIMAL_ZAP_CHECK_LIVE`: `doZapCheck` with the pressing-need slot, armed word, countdown and RNG varied; where the
/// loaded map offers a position that finds a fence direction, the branch through `stopEverything`, `setDirection` and
/// `setBSNew` (with `setBS` answered by a script) is compared too.
pub(crate) fn run_ztanimal_zap_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_ZAP_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let mut rng = Lcg(0x2a9c);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_zap_set_bs);
    for &ty in &types {
        let positions: Vec<[i32; 3]> = std::iter::once([0, 0, 0]).chain(find_zap_position(ty)).collect();
        for round in 0..300 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let (egg, show, pressing, took) = (pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
            let armed = [0i32, 1, 5][(rng.next() as usize) % 3];
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let position = positions[(rng.next() as usize) % positions.len()];
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x4321;
            let prepare = |animal: ZTAnimal| {
                PRESSING.store(pressing, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                SLEEP_GATE.store(took, Ordering::SeqCst);
                save_to_memory(animal.0 + animal_offsets::EGG, egg as u8);
                save_to_memory(animal.0 + 0x31c, countdown);
                save_to_memory(animal.0 + 0x3ac, armed);
                for (i, word) in position.iter().enumerate() {
                    save_to_memory(animal.0 + 0x114 + 4 * i as u32, *word);
                }
            };
            compare_three_poles(
                ty,
                &format!("doZapCheck round {round} egg {egg} show {show} pressing {pressing} armed {armed} countdown {countdown} position {position:?} took {took} seed {seed:#x}"),
                seed,
                &install_sleep_vtable,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_zap_check(animal.0),
                        1 => animal.do_zap_check(),
                        _ => {
                            generated::DO_ZAP_CHECK.hooked()(animal.0 as *const u32);
                        }
                    }
                },
                &mut failures,
            );
            if failures.len() > 20 {
                break;
            }
        }
    }
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = None;
    finish_test(test_name, failures, failure_log)
}

static TANK: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_tank(_this: u32, arg: u32) -> u32 {
    CALLS.lock().unwrap().push(0x250 | arg << 16);
    TANK.load(Ordering::SeqCst)
}

extern "thiscall" fn fake_remove_from_map(_this: u32, arg: u32) {
    CALLS.lock().unwrap().push(0x78 | arg << 16);
}

extern "thiscall" fn fake_add_to_map(_this: u32) {
    CALLS.lock().unwrap().push(0x74);
}

fn install_boxed_vtable(animal: ZTAnimal) {
    install_scripted_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory::<u32>(current + slot * 4)).collect();
    vtable[0x170 / 4] = fake_sleep_gate as *const () as usize as u32;
    vtable[0x250 / 4] = fake_tank as *const () as usize as u32;
    vtable[0x78 / 4] = fake_remove_from_map as *const () as usize as u32;
    vtable[0x74 / 4] = fake_add_to_map as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// `ZTANIMAL_BOXED_CHECK_LIVE`: `doBoxedCheck` with scripted habitat, tank and map slots over fixtures at a live animal's
/// position and at the origin, with the boxed, on-map and countdown bytes, the tank's state and the rattle roll varied.
/// `unbox` runs for real on the fixture in the release branches, as the handling test already does.
pub(crate) fn run_ztanimal_boxed_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_BOXED_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(3);
    let live = live_animals();
    let (Some(&live_animal), false) = (live.first(), types.is_empty()) else {
        failures.push("the live world holds no animal".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let live_position: [i32; 3] = [get_from_memory(live_animal + 0x114), get_from_memory(live_animal + 0x118), get_from_memory(live_animal + 0x11c)];
    let habitat_memory: &'static mut [u8] = Box::leak(vec![0u8; 0x100].into_boxed_slice());
    let habitat_address = habitat_memory.as_ptr() as u32;
    let habitats: Vec<u32> = vec![0, habitat_address];
    let tank_memory: &'static mut [u8] = Box::leak(vec![0u8; 0x200].into_boxed_slice());
    let tank_address = tank_memory.as_ptr() as u32;
    let mut rng = Lcg(0xb0c5);
    for &ty in &types {
        for round in 0..300 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let (boxed, on_map, has_tank, filled, gate) = (pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
            let water = [-1i32, 0, 5][(rng.next() as usize) % 3];
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let habitat = habitats[(rng.next() as usize) % habitats.len()];
            let position = if pick(&mut rng) { live_position } else { [0, 0, 0] };
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x777;
            let habitat_block = pick(&mut rng);
            let prepare = |animal: ZTAnimal| {
                save_to_memory(habitat_address + 0x2c, habitat_block as u8);
                HABITAT.store(habitat, Ordering::SeqCst);
                SLEEP_GATE.store(gate, Ordering::SeqCst);
                TANK.store(if has_tank { tank_address } else { 0 }, Ordering::SeqCst);
                save_to_memory(tank_address + 0x198, filled as u8);
                save_to_memory(tank_address + 0x188, water);
                save_to_memory(animal.0 + 0x394, boxed as u8);
                save_to_memory(animal.0 + 0x211, on_map as u8);
                save_to_memory(animal.0 + 0x354, countdown);
                for (i, word) in position.iter().enumerate() {
                    save_to_memory(animal.0 + 0x114 + 4 * i as u32, *word);
                }
            };
            compare_three_poles(
                ty,
                &format!("doBoxedCheck round {round} boxed {boxed} on_map {on_map} tank {has_tank}/{filled}/{water} gate {gate} habitat {habitat:#x}/{habitat_block} countdown {countdown} position {position:?} seed {seed:#x}"),
                seed,
                &install_boxed_vtable,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_boxed_check(animal.0),
                        1 => animal.do_boxed_check(),
                        _ => {
                            generated::DO_BOXED_CHECK.hooked()(animal.0 as *const u32);
                        }
                    }
                },
                &mut failures,
            );
            if failures.len() > 20 {
                break;
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static REMOVED_MESSAGE: AtomicBool = AtomicBool::new(false);

extern "thiscall" fn fake_removed_message(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0xac);
    0x3300 | REMOVED_MESSAGE.load(Ordering::SeqCst) as u32
}

fn install_health_vtable(animal: ZTAnimal) {
    install_sleep_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory::<u32>(current + slot * 4)).collect();
    vtable[0xac / 4] = fake_removed_message as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// `ZTANIMAL_HEALTH_CHECK_LIVE`: `doHealthCheck` over health values from zero to full, the escape and playmate bytes, the
/// sleep gate and the removal-message slot varied, with many RNG seeds so the contagion and sickness rolls both fire. The
/// consecutive-zero-health counter stays below the type's limit, since `fRemoveAnimal` runs real vanilla code.
pub(crate) fn run_ztanimal_health_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_HEALTH_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let mut rng = Lcg(0x4ea1);
    for &ty in &types {
        let limit: i32 = get_from_memory(ty + 0x330);
        let zero_hunger: i32 = get_from_memory(ty + 0x240);
        for round in 0..600 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let (egg, show, escaped, playmate, gate, removed) = (pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
            let health = [0i32, 1, 50, 100_000][(rng.next() as usize) % 4];
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let hunger = if limit > 1 && pick(&mut rng) { zero_hunger } else { zero_hunger.wrapping_add(1) };
            let was_sick = pick(&mut rng);
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x5151;
            let prepare = |animal: ZTAnimal| {
                SLEEP_GATE.store(gate, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                REMOVED_MESSAGE.store(removed, Ordering::SeqCst);
                save_to_memory(animal.0 + 0x2f8, countdown);
                save_to_memory(animal.0 + 0x2b8, health);
                save_to_memory(animal.0 + 0x2b0, hunger);
                save_to_memory(animal.0 + 0x358, 0i32);
                save_to_memory(animal.0 + animal_offsets::EGG, egg as u8);
                save_to_memory(animal.0 + 0x3a3, escaped as u8);
                save_to_memory(animal.0 + 0x3a4, playmate as u8);
                save_to_memory(animal.0 + 0x3a7, was_sick as u8);
                for offset in [0x298u32, 0x29c, 0x2a0, 0x2a4] {
                    save_to_memory(animal.0 + offset, 0x0f0fu32 ^ seed);
                }
            };
            compare_three_poles(
                ty,
                &format!("doHealthCheck round {round} health {health} hunger {hunger} egg {egg} show {show} escaped {escaped} playmate {playmate} gate {gate} removed {removed} sick {was_sick} countdown {countdown} seed {seed:#x}"),
                seed,
                &install_health_vtable,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_health_check(animal.0),
                        1 => animal.do_health_check(),
                        _ => generated::DO_HEALTH_CHECK.hooked()(animal.0 as *const u32),
                    }
                },
                &mut failures,
            );
            if failures.len() > 20 {
                break;
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_REPRODUCE_CHECK_LIVE`: `doReproduceCheck` over fixtures with a far-off death timer and a cooldown that has not
/// elapsed, so neither the dying branch (`setIsRemoved`, `loadBSFunctions`, `setBS`) nor the reproduction branch
/// (`fCheckReproduction` spawns babies) runs real code on a fixture. Varies the countdowns, mate id and wait, the habitat,
/// the show slot, the dying byte and happiness; compares the state, slot calls and the shared RNG.
pub(crate) fn run_ztanimal_reproduce_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_REPRODUCE_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types: Vec<u32> = live_animal_types(4).into_iter().filter(|&ty| get_from_memory::<i32>(ty + 0x27c) > 0).collect();
    let live = live_animals();
    let Some(&live_animal) = live.first().filter(|_| !types.is_empty()) else {
        failures.push("the live world holds no animal or suitable type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let live_mate_id: u32 = get_from_memory(live_animal + 0x124);
    let live_habitat = crate::ztkeeper::call_slot_0(live_animal, 0x24c);
    let habitats: Vec<u32> = std::iter::once(0).chain(Some(live_habitat).filter(|&h| h != 0)).collect();
    let base = super::ztanimal::base();
    let period: i32 = get_from_memory(base + 0x0023_9454);
    let mut rng = Lcg(0x7e90);
    for &ty in &types {
        let happy_at: i32 = get_from_memory(ty + 0x378);
        for round in 0..500 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let (show, dying) = (pick(&mut rng), pick(&mut rng));
            let countdown = [0i32, 0, 1][(rng.next() as usize) % 3];
            let mate_id = [0u32, live_mate_id, 0x7ffe_0001][(rng.next() as usize) % 3];
            let wait = [period.wrapping_sub(1), 5, 0, 1][(rng.next() as usize) % 4];
            let happiness = [happy_at.wrapping_sub(1), happy_at.wrapping_add(1)][(rng.next() as usize) % 2];
            let habitat = habitats[(rng.next() as usize) % habitats.len()];
            let seed = rng.next().wrapping_mul(0x9e37_79b1) ^ 0x2468;
            let prepare = |animal: ZTAnimal| {
                HABITAT.store(habitat, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                save_to_memory(animal.0 + 0x328, countdown);
                save_to_memory(animal.0 + 0x34c, 1_000_000i32);
                save_to_memory(animal.0 + 0x330, 0i32);
                save_to_memory(animal.0 + 0x3a6, dying as u8);
                save_to_memory(animal.0 + 0x2e8, mate_id);
                save_to_memory(animal.0 + 0x348, wait);
                save_to_memory(animal.0 + 0x2a8, happiness);
                save_to_memory(animal.0 + 0x2ac, 0i32);
            };
            compare_three_poles(
                ty,
                &format!("doReproduceCheck round {round} show {show} dying {dying} countdown {countdown} mate {mate_id:#x} wait {wait} happiness {happiness} habitat {habitat:#x} seed {seed:#x}"),
                seed,
                &install_scripted_vtable,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_reproduce_check(animal.0),
                        1 => animal.do_reproduce_check(),
                        _ => generated::DO_REPRODUCE_CHECK.hooked()(animal.0 as *const u32),
                    }
                },
                &mut failures,
            );
            if failures.len() > 20 {
                break;
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

static AMBIENT_SHOW_ACTIVE: AtomicBool = AtomicBool::new(false);
static ON_LAND: AtomicBool = AtomicBool::new(false);
static AT_EDGE: AtomicBool = AtomicBool::new(false);

extern "thiscall" fn fake_show_active(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x230);
    0x1100 | AMBIENT_SHOW_ACTIVE.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_on_land(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0xbc);
    0x2200 | ON_LAND.load(Ordering::SeqCst) as u32
}

extern "thiscall" fn fake_at_edge(_this: u32) -> u32 {
    CALLS.lock().unwrap().push(0x16c);
    0x4400 | AT_EDGE.load(Ordering::SeqCst) as u32
}

fn install_ambient_vtable(animal: ZTAnimal) {
    install_sleep_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory::<u32>(current + slot * 4)).collect();
    vtable[0x230 / 4] = fake_show_active as *const () as usize as u32;
    vtable[0xbc / 4] = fake_on_land as *const () as usize as u32;
    vtable[0x16c / 4] = fake_at_edge as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// `ZTANIMAL_AMBIENT_ANIMS_LIVE`: `doAmbientAnims` over fixtures standing on the tiles of live animals, with the gate bytes,
/// goal, set name, queued call, rampage and escape bytes, sleep need and the scripted slots (`+0x22c`/`+0x230`/`+0xbc`/`+0x16c`/
/// `+0x24c`) varied. `setBS` is answered by a script (the `SLEEP_GATE` flag); `ZTUnit::doAmbientAnims` and `doSleepCheck` run for real.
pub(crate) fn run_ztanimal_ambient_anims_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_AMBIENT_ANIMS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(2);
    let live = live_animals();
    if live.is_empty() || types.is_empty() {
        failures.push("the live world holds no animal".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let positions: Vec<[i32; 3]> = live.iter().take(6).map(|&a| [get_from_memory(a + 0x114), get_from_memory(a + 0x118), get_from_memory(a + 0x11c)]).collect();
    let habitat_memory: &'static mut [u8] = Box::leak(vec![0u8; 0x100].into_boxed_slice());
    let habitat_address = habitat_memory.as_ptr() as u32;
    let names: [&[u8]; 7] = [b"", b"bSleep", b"bSick", b"bHatch", b"bHabitatRampage", b"bEscapedRampage", b"bWalk"];
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_zap_set_bs);
    let mut rng = Lcg(0xa3b1);
    for &ty in &types {
        let sleep_at: i32 = get_from_memory(ty + 0x324);
        for round in 0..600 {
            let pick = |rng: &mut Lcg| rng.next() & 1 == 1;
            let rare = |rng: &mut Lcg| rng.next() % 8 == 0;
            let gates: [bool; 6] = [rare(&mut rng), rare(&mut rng), rare(&mut rng), rare(&mut rng), rare(&mut rng), rare(&mut rng)];
            let (show, show_active, goal_chase) = (rare(&mut rng), pick(&mut rng), rare(&mut rng));
            let (rampage, escaped, no_escape_set, pending, gate, land, edge, habitat_block) =
                (pick(&mut rng), pick(&mut rng), pick(&mut rng), rare(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
            let (name, queued) = (names[(rng.next() as usize) % 7], names[(rng.next() as usize) % 7]);
            let need = [sleep_at.wrapping_sub(1), sleep_at.wrapping_add(1)][(rng.next() as usize) % 2];
            let sick = rare(&mut rng);
            let position = positions[(rng.next() as usize) % positions.len()];
            let prepare = |animal: ZTAnimal| {
                SLEEP_GATE.store(gate, Ordering::SeqCst);
                DOING_SHOW.store(show, Ordering::SeqCst);
                AMBIENT_SHOW_ACTIVE.store(show_active, Ordering::SeqCst);
                ON_LAND.store(land, Ordering::SeqCst);
                AT_EDGE.store(edge, Ordering::SeqCst);
                HABITAT.store(habitat_address, Ordering::SeqCst);
                save_to_memory(habitat_address + 0x2c, habitat_block as u8);
                for (offset, gate) in [0x3a6u32, 0x394, 0x3a4, 0x24c, 0x393, 0x395].into_iter().zip(gates) {
                    save_to_memory(animal.0 + offset, gate as u8);
                }
                save_to_memory(animal.0 + 0x170, if goal_chase { 0x74i32 } else { 1 });
                save_to_memory(animal.0 + 0x3b0, rampage as u8);
                save_to_memory(animal.0 + 0x3b1, 1u8);
                save_to_memory(animal.0 + 0x3a3, escaped as u8);
                save_to_memory(animal.0 + 0x20c, no_escape_set as u8);
                save_to_memory(animal.0 + 0x3a7, sick as u8);
                save_to_memory(animal.0 + 0x2c0, need);
                for (i, word) in position.iter().enumerate() {
                    save_to_memory(animal.0 + 0x114 + 4 * i as u32, *word);
                }
                plant_set_name(animal, name);
                unsafe { crate::bfentity::set_pending_bs(animal.0, crate::bfentity::new_owned_name(queued), 1, 1) };
                save_to_memory(animal.0 + 0x6d, pending as u8);
            };
            compare_three_poles(
                ty,
                &format!("doAmbientAnims round {round} gates {gates:?} show {show}/{show_active} chase {goal_chase} rampage {rampage} escaped {escaped} no_escape_set {no_escape_set} pending {pending} gate {gate} land {land} edge {edge} block {habitat_block} name {name:?} queued {queued:?} need {need} sick {sick} position {position:?}"),
                0x1b2b,
                &install_ambient_vtable,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_ambient_anims(animal.0),
                        1 => animal.do_ambient_anims(),
                        _ => generated::DO_AMBIENT_ANIMS.hooked()(animal.0 as *const u32, std::ptr::null()),
                    }
                },
                &mut failures,
            );
            if failures.len() > 20 {
                break;
            }
        }
    }
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = None;
    finish_test(test_name, failures, failure_log)
}

extern "thiscall" fn fake_slot_110(_this: u32, a: u32, b: u32) {
    CALLS.lock().unwrap().push(0x110 | a << 12 | b << 13);
}

fn install_baby_vtable(animal: ZTAnimal) {
    install_sleep_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory::<u32>(current + slot * 4)).collect();
    vtable[0x110 / 4] = fake_slot_110 as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// `ZTANIMAL_BABY_TO_ADULT_CHECK_LIVE`: `doBabyToAdultCheck` on fixtures of every live baby type (name starting with `y`) and of
/// adult types, with the countdown and the gender text varied. `findTypes` and the new type's vtable `+0x10` run for real; the animal's
/// `+0x110` and `+0x198` are recorded. Compares the animal's type and state and the slot calls.
pub(crate) fn run_ztanimal_baby_to_adult_check_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_BABY_TO_ADULT_CHECK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let all_types = live_animal_types(400);
    let is_baby = |ty: u32| get_from_memory::<u8>(get_from_memory::<u32>(ty + 0xa4)) == b'y';
    let mut types: Vec<u32> = all_types.iter().copied().filter(|&ty| is_baby(ty)).take(6).collect();
    types.extend(all_types.iter().copied().filter(|&ty| !is_baby(ty)).take(2));
    if !types.iter().any(|&ty| is_baby(ty)) {
        failures.push("the live world holds no baby animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let texts: [&'static [u8]; 3] = [b"Male\0", b"Female\0", b"\0"];
    let text_addresses: Vec<u32> = texts.iter().map(|text| text.as_ptr() as u32).collect();
    let morphs = |ty: u32| {
        let animal = build_animal(ty);
        install_baby_vtable(animal);
        save_to_memory(animal.0 + 0x32c, 0i32);
        save_to_memory(animal.0 + 0x26c, text_addresses[0]);
        real::do_baby_to_adult_check(animal.0);
        get_from_memory::<u32>(animal.0 + 0x128) != ty
    };
    if !types.iter().filter(|&&ty| is_baby(ty)).any(|&ty| morphs(ty)) {
        failures.push("no baby type morphed under the real body, so the findTypes branch was not exercised".to_string());
    }
    for &ty in &types {
        for (round, countdown) in [0i32, 0, 1, 5].into_iter().cycle().take(12).enumerate() {
            let text = text_addresses[round % 3];
            let prepare = |animal: ZTAnimal| {
                save_to_memory(animal.0 + 0x32c, countdown);
                save_to_memory(animal.0 + 0x26c, text);
            };
            compare_three_poles(
                ty,
                &format!("doBabyToAdultCheck type {ty:#x} baby {} round {round} countdown {countdown} text {:?}", is_baby(ty), texts[round % 3]),
                0x77,
                &install_baby_vtable,
                &prepare,
                &|animal, pole| unsafe {
                    match pole {
                        0 => real::do_baby_to_adult_check(animal.0),
                        1 => animal.do_baby_to_adult_check(),
                        _ => generated::DO_BABY_TO_ADULT_CHECK.hooked()(animal.0 as *const u32),
                    }
                },
                &mut failures,
            );
            if failures.len() > 20 {
                break;
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_HATCH_EAT_LIVE`: `hatchEgg` (`setBSNew` answered by a script) and `fEat`'s stop-eating exits (an escaped animal, a
/// fixture without food, with the current set already `bEat` or not) over scripted fixtures; real, port and hooked agree. The
/// food-eating branch and `finishHatch` (it posts a zoo message naming the fixture) are not run against vanilla; the mixer
/// step of `finishHatch` is checked structurally: one link, on the idle animation (the reference count is not compared: there is no vanilla pole for the step alone).
pub(crate) fn run_ztanimal_hatch_eat_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_HATCH_EAT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_zap_set_bs);
    for &ty in &types {
        for answer in [false, true] {
            for current in [&b""[..], b"bHatch", b"bEat"] {
                compare_three_poles(
                    ty,
                    &format!("hatchEgg answer {answer} current {:?}", String::from_utf8_lossy(current)),
                    1,
                    &install_sleep_vtable,
                    &|animal| {
                        SLEEP_GATE.store(answer, Ordering::SeqCst);
                        if !current.is_empty() {
                            plant_set_name(animal, current);
                        }
                    },
                    &|animal, pole| match pole {
                        0 => real::hatch_egg(animal.0),
                        1 => animal.hatch_egg(),
                        _ => unsafe { generated::HATCH_EGG.hooked()(animal.0 as *const u32) },
                    },
                    &mut failures,
                );
            }
        }
        for escaped in [0u8, 1] {
            for current in [&b""[..], b"bEat"] {
                compare_three_poles(
                    ty,
                    &format!("fEat escaped {escaped} current {:?}", String::from_utf8_lossy(current)),
                    2,
                    &install_sleep_vtable,
                    &|animal| {
                        SLEEP_GATE.store(true, Ordering::SeqCst);
                        save_to_memory(animal.0 + 0x3a3, escaped);
                        save_to_memory(animal.0 + 0x39e, 1u8);
                        if !current.is_empty() {
                            plant_set_name(animal, current);
                        }
                    },
                    &|animal, pole| {
                        let _ = match pole {
                            0 => real::f_eat(animal.0),
                            1 => animal.f_eat(),
                            _ => unsafe { generated::F_EAT.hooked()(animal.0 as *const u32) as u32 },
                        };
                    },
                    &mut failures,
                );
            }
        }
        let animal = build_animal(ty);
        let idle = crate::ztkeeper::call_slot_1(animal.0, 0x8c, 0);
        animal.rebase_mixer_on_idle_anim();
        let (begin, end): (u32, u32) = (get_from_memory(animal.0 + 0x80), get_from_memory(animal.0 + 0x84));
        if end.wrapping_sub(begin) != 12 || get_from_memory::<u32>(begin) != idle {
            failures.push(format!("finishHatch mixer step: link array {begin:#x}..{end:#x}, first word differs from the idle animation {idle:#x}"));
        }
    }
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = None;
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_CHECK_REPRODUCTION_LIVE`: `fCheckReproduction`'s exits that create nothing, over scripted fixtures: a failed roll (birth
/// percent 0), the unhappy-reproduction gate with a type id that does not allow it, and a passed roll over a missing or flagged
/// habitat. Real, port and hooked agree on the answer, the game RNG word and the fixture state. A passing roll with a clear habitat
/// would spawn animals into the live zoo and is not run against vanilla.
pub(crate) fn run_ztanimal_check_reproduction_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_CHECK_REPRODUCTION_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let flagged_habitat = Box::leak(vec![0u8; 0x200].into_boxed_slice()).as_mut_ptr() as u32;
    save_to_memory(flagged_habitat + 0x2c, 1u8);
    let gate_address = super::ztanimal::base() + 0x0023_c2e3;
    for &ty in &types {
        let original_percent: i32 = get_from_memory(ty + 0x278);
        let original_gate: u8 = get_from_memory(gate_address);
        for percent in [0i32, 100] {
            for gate in [0u8, 1] {
                for habitat in [0u32, flagged_habitat] {
                    for seed in [1u32, 0x1234_5678] {
                        save_to_memory(ty + 0x278, percent);
                        compare_three_poles(
                            ty,
                            &format!("fCheckReproduction percent {percent} gate {gate} habitat {habitat:#x} seed {seed:#x}"),
                            seed,
                            &install_sleep_vtable,
                            &|animal| {
                                HABITAT.store(habitat, Ordering::SeqCst);
                                save_to_memory(gate_address, gate);
                                save_to_memory(animal.0 + 0x2e8, 0u32);
                            },
                            &|animal, pole| {
                                let answer = match pole {
                                    0 => real::f_check_reproduction(animal.0),
                                    1 => animal.f_check_reproduction(),
                                    _ => unsafe { generated::F_CHECK_REPRODUCTION.hooked()(animal.0 as *const i32) },
                                };
                                CALLS.lock().unwrap().push(0xcc00_0000 | answer as u32);
                                save_to_memory(gate_address, gate);
                            },
                            &mut failures,
                        );
                    }
                }
            }
        }
        save_to_memory(ty + 0x278, original_percent);
        save_to_memory(gate_address, original_gate);
    }
    HABITAT.store(0, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}

/// Every entity pointer the live world holds, in array order.
fn world_entities() -> Vec<u32> {
    let world = crate::globals::globals().ztworldmgr();
    let mut entities = Vec::new();
    let mut slot = world.entity_array_start();
    while slot != world.entity_array_end() {
        entities.push(get_from_memory::<u32>(slot));
        slot += 4;
    }
    entities
}

/// Removes `entity` from the live world through `BFWorldMgr::removeEntity` (slot `+0x1c`, delete flag set), the path the game takes
/// when it removes an animal: array, id map, `removeFromMap(0)` and the destructor.
fn remove_world_entity(entity: u32) {
    let world = crate::globals::globals().ztworldmgr_ptr() as u32;
    crate::ztkeeper::call_slot_2(world, 0x1c, entity, 1);
}

/// The fields a freshly born animal differs in: type, position, parent id and mate, the growth countdown, the egg byte and the hatch timer.
fn baby_snapshot(entity: u32) -> Vec<u32> {
    let mut words = vec![get_from_memory::<u32>(entity + 0x128)];
    words.extend((0..3).map(|i| get_from_memory::<u32>(entity + 0x114 + 4 * i)));
    for offset in [0x2f0u32, 0x2ec, 0x32c, 0x398] {
        words.push(get_from_memory::<u32>(entity + offset));
    }
    words.push(get_from_memory::<u8>(entity + 0x395) as u32);
    words
}

/// `ZTANIMAL_CHECK_REPRODUCTION_SPAWN_LIVE`: `fCheckReproduction` over real live animals whose habitat accepts births, with the birth
/// percent forced to 100 and the litter to at most 2. For each pole the game RNG is re-seeded, the call's answer, the RNG word and every
/// animal it added (found by diffing the world's entity array) are recorded, and the new animals are removed again through the world's
/// own `removeEntity`; the entity count must return to where it started. Animals vanilla and the port create both come from the type's
/// own `createEntity`, so each is torn down by the allocator that made it. The first (real) run is a discarded warm-up: the first cycle
/// on a type draws one more game-RNG value than every later cycle (a vanilla cache that is built lazily and then persists; whichever
/// implementation runs first pays it, so it is not something the call leaves behind and it cannot be undone), and the compared poles
/// all run after it. The coin flip that picks the gendered search text uses the C runtime's `rand`, which the test does not seed. Each
/// type also runs with its egg-laying byte (`+0x404`) forced on, so the egg branch is compared too.
pub(crate) fn run_ztanimal_check_reproduction_spawn_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_CHECK_REPRODUCTION_SPAWN_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let gate_address = super::ztanimal::base() + 0x0023_c2e3;
    let mut covered_births = 0usize;
    let mut covered_eggs = 0usize;
    let mut seen_types: Vec<u32> = Vec::new();
    for parent in live_animals() {
        let animal = ZTAnimal(parent);
        let Some(ty) = animal.animal_type() else { continue };
        if seen_types.contains(&ty) || seen_types.len() >= 6 {
            continue;
        }
        let habitat = crate::ztkeeper::call_slot_0(parent, 0x24c);
        if habitat == 0 || get_from_memory::<u8>(habitat + 0x2c) != 0 {
            continue;
        }
        seen_types.push(ty);
        let original = (get_from_memory::<i32>(ty + 0x278), get_from_memory::<i32>(ty + 0x284), get_from_memory::<u8>(gate_address));
        save_to_memory(ty + 0x278, 100i32);
        save_to_memory(ty + 0x284, original.1.clamp(1, 2));
        let original_egg: u8 = get_from_memory(ty + 0x404);
        for force_egg in [false, true] {
        save_to_memory(ty + 0x404, if force_egg { 1u8 } else { original_egg });
        let mut results: Vec<(bool, u32, Vec<Vec<u32>>)> = Vec::new();
        for pole in 0..4usize {
            let before = world_entities();
            save_to_memory(gate_address, 0u8);
            save_to_memory(rng_address, 0x5eed_u32);
            let answer = match pole {
                0 | 1 => real::f_check_reproduction(parent),
                2 => animal.f_check_reproduction(),
                _ => unsafe { generated::F_CHECK_REPRODUCTION.hooked()(parent as *const i32) },
            };
            let rng: u32 = get_from_memory(rng_address);
            let added: Vec<u32> = world_entities().into_iter().filter(|entity| !before.contains(entity)).collect();
            let snapshots: Vec<Vec<u32>> = added.iter().map(|&entity| baby_snapshot(entity)).collect();
            for &entity in added.iter().rev() {
                remove_world_entity(entity);
            }
            if world_entities() != before {
                failures.push(format!("type {ty:#x} pole {pole}: the world holds {} entities after removing the births, expected {}", world_entities().len(), before.len()));
            }
            results.push((answer, rng, snapshots));
        }
        for (pole, name) in [(2usize, "port"), (3, "hooked")] {
            if results[1].0 != results[pole].0 || results[1].1 != results[pole].1 || results[1].2 != results[pole].2 {
                failures.push(format!(
                    "type {ty:#x} forced egg {force_egg}: {name} differs from real: answer {} vs {}, rng {:#x} vs {:#x}, babies {:?} vs {:?}",
                    results[pole].0, results[1].0, results[pole].1, results[1].1, results[pole].2, results[1].2
                ));
            }
        }
        covered_births += results[1].2.len();
        covered_eggs += results[1].2.iter().filter(|snapshot| snapshot[8] != 0).count();
        }
        save_to_memory(ty + 0x404, original_egg);
        save_to_memory(ty + 0x278, original.0);
        save_to_memory(ty + 0x284, original.1);
        save_to_memory(gate_address, original.2);
    }
    if seen_types.is_empty() {
        failures.push("the live zoo holds no animal in a habitat that accepts births".to_string());
    } else if covered_births == 0 {
        failures.push("no birth was placed by real vanilla in any live habitat, so nothing was compared".to_string());
    }
    if covered_births > 0 && covered_eggs == 0 {
        failures.push("no egg was placed by real vanilla, so the egg branch was not compared".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_PUT_DOWN_DIRT_LIVE`: `putDownDirt` on the live zoo. Dirt: for live animals standing on a tile that is neither water nor
/// blocked, the call's answer, the game RNG word, the scenery it adds (type, position, rotation) and the tile's flag bytes are recorded
/// for real, port and hooked; the scenery is then removed through the world's `removeEntity` and the entity count must return. Tank:
/// for live animals in a tank on a water tile, the answer and the tank's purity and its timer are compared and restored afterwards.
/// The first real run is a discarded warm-up (it primes the cached dirt type and settles tile state).
pub(crate) fn run_ztanimal_put_down_dirt_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_PUT_DOWN_DIRT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let rng_address = super::ztanimal::base() + 0x0023_8060;
    let mut dirt_cases = 0usize;
    let mut tank_cases = 0usize;
    for parent in live_animals() {
        let animal = ZTAnimal(parent);
        if animal.animal_type().is_none() {
            continue;
        }
        let tile = (unsafe { generated_bfentity::GET_TILE.original()(parent as *const u32) }) as u32;
        if tile == 0 {
            continue;
        }
        let water = get_from_memory::<u8>(tile + 0x85) & 0x20 != 0;
        let blocked = get_from_memory::<u8>(tile + 0x83) & 3 != 0;
        let habitat = crate::ztkeeper::call_slot_0(parent, 0x24c);
        let tank = water && habitat != 0 && crate::util::low_byte_bool(crate::ztkeeper::call_slot_0(habitat, 0x20));
        if tank {
            if tank_cases >= 3 {
                continue;
            }
            tank_cases += 1;
            let original = (get_from_memory::<i32>(habitat + 0x1a8), get_from_memory::<i32>(habitat + 0x1ac));
            let mut results: Vec<(bool, i32, i32)> = Vec::new();
            for pole in 0..4usize {
                let answer = match pole {
                    0 | 1 => real::put_down_dirt(parent, tile),
                    2 => animal.put_down_dirt(tile),
                    _ => unsafe { generated::PUT_DOWN_DIRT.hooked()(parent as *const u32, tile as *const u32) },
                };
                results.push((answer, get_from_memory(habitat + 0x1a8), get_from_memory(habitat + 0x1ac)));
                save_to_memory(habitat + 0x1a8, original.0);
                save_to_memory(habitat + 0x1ac, original.1);
            }
            for (pole, name) in [(2usize, "port"), (3, "hooked")] {
                if results[1] != results[pole] {
                    failures.push(format!("tank animal {parent:#x}: {name} {:?} differs from real {:?}", results[pole], results[1]));
                }
            }
            continue;
        }
        if water || blocked || dirt_cases >= 6 {
            continue;
        }
        dirt_cases += 1;
        let mut results: Vec<(bool, u32, Vec<Vec<u32>>, Vec<u8>)> = Vec::new();
        for pole in 0..4usize {
            let before = world_entities();
            save_to_memory(rng_address, 0xd157_u32);
            let answer = match pole {
                0 | 1 => real::put_down_dirt(parent, tile),
                2 => animal.put_down_dirt(tile),
                _ => unsafe { generated::PUT_DOWN_DIRT.hooked()(parent as *const u32, tile as *const u32) },
            };
            let rng: u32 = get_from_memory(rng_address);
            let added: Vec<u32> = world_entities().into_iter().filter(|entity| !before.contains(entity)).collect();
            let snapshots: Vec<Vec<u32>> = added
                .iter()
                .map(|&entity| {
                    let mut words = vec![get_from_memory::<u32>(entity + 0x128)];
                    words.extend((0..3).map(|i| get_from_memory::<u32>(entity + 0x114 + 4 * i)));
                    words.push(get_from_memory::<u32>(entity + 0x12c));
                    words
                })
                .collect();
            let tile_bytes: Vec<u8> = (0x80..0x90).map(|offset| get_from_memory::<u8>(tile + offset)).collect();
            for &entity in added.iter().rev() {
                remove_world_entity(entity);
            }
            if world_entities() != before {
                failures.push(format!("animal {parent:#x} pole {pole}: the world holds {} entities after removing the dirt, expected {}", world_entities().len(), before.len()));
            }
            results.push((answer, rng, snapshots, tile_bytes));
        }
        for (pole, name) in [(2usize, "port"), (3, "hooked")] {
            if results[1] != results[pole] {
                failures.push(format!("animal {parent:#x}: {name} {:?} differs from real {:?}", results[pole], results[1]));
            }
        }
    }
    if dirt_cases == 0 {
        failures.push("no live animal stands on a free land tile, so the dirt branch was not compared".to_string());
    }
    if tank_cases == 0 {
        failures.push("no live animal is in a tank on a water tile, so the tank branch was not compared".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

extern "thiscall" fn fake_start_eating(_this: u32, a: u32, b: u32) {
    CALLS.lock().unwrap().push(0x1d8 | a << 12 | b << 13);
}

static EAT_REACHED: AtomicU32 = AtomicU32::new(0);

fn install_eat_vtable(animal: ZTAnimal) {
    install_sleep_vtable(animal);
    let current: u32 = get_from_memory(animal.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory::<u32>(current + slot * 4)).collect();
    vtable[0x1d8 / 4] = fake_start_eating as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(animal.0, vtable.as_ptr() as u32);
}

/// The first live entity whose type casts to the food-entity class, with the tile it stands on.
fn live_food_entity() -> Option<(u32, u32)> {
    let tag = super::ztanimal::base() + 0x0023_86c0;
    for entity in world_entities() {
        if entity == 0 {
            continue;
        }
        let entity_type: u32 = get_from_memory(entity + 0x128);
        if entity_type == 0 {
            continue;
        }
        let is_food = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + 0x1c)) };
        if is_food(entity_type, tag) {
            let tile = (unsafe { generated_bfentity::GET_TILE.original()(entity as *const u32) }) as u32;
            if tile != 0 {
                return Some((entity, tile));
            }
        }
    }
    None
}

/// `ZTANIMAL_F_EAT_LIVE`: `fEat`'s eating branch. A scripted fixture animal stands on the tile of a real live food entity and its eat
/// context points at it, so `fFoodUnitsActuallyEaten`, `fReduceFood` (the live habitat, `setAmountFoodUnitsDelta`) and `setBS`
/// (answered by a script) run for real. The food's units are set high so the food is never used up (that would retarget live animals)
/// and restored after every pole. Real, port and hooked agree on the answer, the food units left, the game RNG word, the fixture's state
/// and the slot calls, across the type's no-eat-animation byte, the set name and queued call already being `bEat`, and `setBS`'s answer.
pub(crate) fn run_ztanimal_f_eat_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_F_EAT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(3);
    let Some((food, tile)) = live_food_entity() else {
        failures.push("the live zoo holds no food entity, so the eating branch was not compared".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    if get_from_memory::<u8>(tile + 0x83) & 3 != 0 {
        failures.push("the live food entity stands on a blocked tile, so the eating branch is unreachable".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let habitat = live_animals().iter().map(|&animal| crate::ztkeeper::call_slot_0(animal, 0x24c)).find(|&h| h != 0).unwrap_or(0);
    let context = Box::leak(vec![0u8; 0x40].into_boxed_slice()).as_mut_ptr() as u32;
    save_to_memory(context + 0x10, food);
    let position: Vec<u32> = (0..3).map(|i| get_from_memory::<u32>(food + 0x114 + 4 * i)).collect();
    let original_units: i32 = get_from_memory(food + 0x154);
    EAT_REACHED.store(0, Ordering::SeqCst);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_zap_set_bs);
    for &ty in &types {
        let original_flag: u8 = get_from_memory(ty + 0x44c);
        for no_eat_anim in [0u8, 1] {
            for current in [&b""[..], b"bEat"] {
                for queued in [&b""[..], b"bEat"] {
                    for set_bs_answer in [false, true] {
                        for can_sleep in [false, true] {
                            save_to_memory(ty + 0x44c, no_eat_anim);
                            compare_three_poles(
                                ty,
                                &format!("fEat no-eat-anim {no_eat_anim} current {:?} queued {:?} setBS {set_bs_answer} sleepGate {can_sleep}", String::from_utf8_lossy(current), String::from_utf8_lossy(queued)),
                                3,
                                &install_eat_vtable,
                                &|animal| {
                                    HABITAT.store(habitat, Ordering::SeqCst);
                                    SLEEP_GATE.store(set_bs_answer, Ordering::SeqCst);
                                    for (i, word) in position.iter().enumerate() {
                                        save_to_memory(animal.0 + 0x114 + 4 * i as u32, *word);
                                    }
                                    save_to_memory(animal.0 + 0x38c, context);
                                    save_to_memory(animal.0 + 0x3a3, 0u8);
                                    save_to_memory(animal.0 + 0x3a9, 0u8);
                                    save_to_memory(food + 0x154, 1000i32);
                                    if !current.is_empty() {
                                        plant_set_name(animal, current);
                                    }
                                    if !queued.is_empty() {
                                        plant_queued_call(animal, queued);
                                    }
                                    let _ = can_sleep;
                                },
                                &|animal, pole| {
                                    let answer = match pole {
                                        0 => real::f_eat(animal.0),
                                        1 => animal.f_eat(),
                                        _ => unsafe { generated::F_EAT.hooked()(animal.0 as *const u32) as u32 },
                                    };
                                    let left: i32 = get_from_memory(food + 0x154);
                                    if pole == 0 && left < 1000 {
                                        EAT_REACHED.fetch_add(1, Ordering::SeqCst);
                                    }
                                    CALLS.lock().unwrap().push(0xea70_0000 | (answer & 0xffff));
                                    CALLS.lock().unwrap().push(0xea71_0000 | (left as u32 & 0xffff));
                                    save_to_memory(food + 0x154, original_units);
                                },
                                &mut failures,
                            );
                            if failures.len() > 20 {
                                break;
                            }
                        }
                    }
                }
            }
        }
        save_to_memory(ty + 0x44c, original_flag);
    }
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = None;
    save_to_memory(food + 0x154, original_units);
    if EAT_REACHED.load(Ordering::SeqCst) == 0 {
        failures.push("real vanilla never reached the eating branch (the food's units never dropped), so nothing was compared".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// Whether `entity`'s type casts to the class tagged at `tag_rva`.
fn entity_casts(entity: u32, tag_rva: u32) -> bool {
    let entity_type: u32 = get_from_memory(entity + 0x128);
    if entity_type == 0 {
        return false;
    }
    let cast = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + 0x1c)) };
    cast(entity_type, super::ztanimal::base() + tag_rva)
}

/// `ZTANIMAL_CONSUME_LIVE`: `consume` on live units: up to two guests, two staff and one other animal, each with the consuming animal's
/// "eats guests" type byte (`+0x3d5`) forced off and on. The toast is captured by the display-message recorder (so nothing is shown);
/// the unit's memory after the call (removed flags, whatever slot `+0xcc(0)` changed) is compared and then restored byte for byte.
/// Real, port and hooked agree on the toasts and the unit's memory.
pub(crate) fn run_ztanimal_consume_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::display_message_recorder;
    const SNAPSHOT: u32 = 0x2e0;
    let test_name = "ZTANIMAL_CONSUME_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let animals = live_animals();
    let Some(&eater) = animals.first() else {
        failures.push("the live world holds no animal".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let Some(eater_type) = ZTAnimal(eater).animal_type() else {
        failures.push("the first live animal has no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let (mut guests, mut staff, mut others) = (Vec::new(), Vec::new(), Vec::new());
    for entity in world_entities() {
        if entity == 0 || entity == eater {
            continue;
        }
        if entity_casts(entity, 0x0023_8700) {
            guests.push(entity);
        } else if entity_casts(entity, 0x0023_8710) {
            staff.push(entity);
        } else if animals.contains(&entity) {
            others.push(entity);
        }
    }
    let mut units: Vec<(&str, u32)> = Vec::new();
    units.extend(guests.iter().take(2).map(|&unit| ("guest", unit)));
    units.extend(staff.iter().take(2).map(|&unit| ("staff", unit)));
    units.extend(others.iter().take(1).map(|&unit| ("animal", unit)));
    if !units.iter().any(|(kind, _)| *kind == "staff") {
        failures.push("the live zoo holds no staff member, so the toast branch was not compared".to_string());
    }
    if !units.iter().any(|(kind, _)| *kind == "guest") {
        failures.push("the live zoo holds no guest, so the guest branches were not compared".to_string());
    }
    let original_flag: u8 = get_from_memory(eater_type + 0x3d5);
    for &(kind, unit) in &units {
        for eats_guests in [0u8, 1] {
            save_to_memory(eater_type + 0x3d5, eats_guests);
            let saved: Vec<u8> = (0..SNAPSHOT).map(|offset| get_from_memory::<u8>(unit + offset)).collect();
            let mut results: Vec<(Vec<(String, i32, u32, u32)>, Vec<u8>)> = Vec::new();
            for pole in 0..3usize {
                display_message_recorder::begin_capture();
                match pole {
                    0 => real::consume(eater, unit),
                    1 => ZTAnimal(eater).consume(unit),
                    _ => unsafe { generated::CONSUME.hooked()(eater as *const u32, unit as *const i32) },
                }
                let messages = display_message_recorder::end_capture();
                let after: Vec<u8> = (0..SNAPSHOT).map(|offset| get_from_memory::<u8>(unit + offset)).collect();
                for (offset, byte) in saved.iter().enumerate() {
                    save_to_memory(unit + offset as u32, *byte);
                }
                results.push((messages, after));
            }
            for (pole, name) in [(1usize, "port"), (2, "hooked")] {
                if results[0] != results[pole] {
                    failures.push(format!("{kind} {unit:#x} eats-guests {eats_guests}: {name} differs from real (toasts {:?} vs {:?}, memory equal: {})", results[pole].0, results[0].0, results[0].1 == results[pole].1));
                }
            }
        }
    }
    save_to_memory(eater_type + 0x3d5, original_flag);
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_ESCAPED_ALERT_LIVE`: `showEscapedAnimalAlert` on live animals with the UI calls captured by the alert recorder (two fake
/// elements, no dialog opens). Real, port and hooked agree on the element ids looked up, the icon animation, the caption text built from
/// the animal's name and the confirm-dialog arguments, and the record must hold the full sequence.
pub(crate) fn run_ztanimal_escaped_alert_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::alert_recorder::{self, AlertCall};
    let test_name = "ZTANIMAL_ESCAPED_ALERT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let animals = live_animals();
    if animals.is_empty() {
        failures.push("the live world holds no animal".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    for &parent in animals.iter().take(4) {
        let mut results: Vec<Vec<AlertCall>> = Vec::new();
        for pole in 0..3usize {
            alert_recorder::begin_capture();
            match pole {
                0 => real::show_escaped_animal_alert(parent),
                1 => ZTAnimal(parent).show_escaped_animal_alert(),
                _ => unsafe { generated::SHOW_ESCAPED_ANIMAL_ALERT.hooked()(parent as *const u32) },
            }
            results.push(alert_recorder::end_capture());
        }
        let complete = results[0].len() == 5
            && matches!(results[0].get(2), Some(AlertCall::SetAnimation { .. }))
            && matches!(results[0].get(3), Some(AlertCall::Caption { .. }))
            && matches!(results[0].get(4), Some(AlertCall::ConfirmDialog { entity, .. }) if *entity == parent);
        if !complete {
            failures.push(format!("animal {parent:#x}: real vanilla's UI sequence is not the expected five calls: {:?}", results[0]));
        }
        for (pole, name) in [(1usize, "port"), (2, "hooked")] {
            if results[0] != results[pole] {
                failures.push(format!("animal {parent:#x}: {name} {:?} differs from real {:?}", results[pole], results[0]));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
