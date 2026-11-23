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

use openzt_detour::generated::ztanimal as generated;

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
