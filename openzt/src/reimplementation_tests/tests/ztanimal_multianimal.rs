//! `ZTAnimal::addToMap`, `removeFromMap`, `setDying` and `setName` against the real bodies. The multi-animal panel's
//! `std::set<ZTAnimal*>` (`DAT_00639460`) is shared state, so each case runs one pole at a time on its own animal and
//! compares what the pole left behind: whether and where the animal sits in the set, the set's size, the escaped-animal
//! list, and the animal's own bytes.

use std::ffi::c_void;

use openzt_detour::generated::{
    ai_cls_0x404fd6::{TREE_ERASE_RANGE_0X4FE4AF, TREE_INSERT_UNIQUE_0X453BCC, TREE_LOWER_BOUND_0X4FE4F5},
    bfentity::GET_TILE,
    msvc_std_listuint::INSERT as LIST_UINT_INSERT,
    ztanimal::{ADD_TO_MAP, REMOVE_FROM_MAP, SET_DYING, SET_NAME},
    ztui_multianimal::REMOVE_ENTITY as UPPER_BOUND,
};

use super::ztanimal::{base, build_animal, live_animal_types, live_animals};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    vanilla_string::VanillaString,
    ztanimal::{live_support::real, ZTAnimal},
};

const SET_RVA: u32 = 0x0023_9460;
const ESCAPE_LIST_RVA: u32 = 0x0023_8fb0;

fn set_object() -> u32 {
    base() + SET_RVA
}

/// The set's animals in order.
fn set_values() -> Vec<u32> {
    fn walk(node: u32, out: &mut Vec<u32>, depth: usize) {
        if node == 0 || depth > 64 {
            return;
        }
        walk(get_from_memory(node + 8), out, depth + 1);
        out.push(get_from_memory(node + 0x10));
        walk(get_from_memory(node + 0xc), out, depth + 1);
    }
    let head: u32 = get_from_memory(set_object());
    let mut out = Vec::new();
    walk(get_from_memory(head + 4), &mut out, 0);
    out
}

fn set_len() -> u32 {
    get_from_memory(set_object() + 4)
}

fn insert(animal: ZTAnimal) {
    let mut result = [0u32; 2];
    let key = animal.0;
    unsafe { TREE_INSERT_UNIQUE_0X453BCC.original()(set_object() as *const u32, result.as_mut_ptr() as *const i8, &key as *const u32 as *const c_void) };
}

fn erase(animal: ZTAnimal) {
    let (mut upper, mut lower): (u32, u32) = (0, 0);
    let key = animal.0;
    unsafe {
        UPPER_BOUND.original()(set_object() as *const c_void, &mut upper as *mut u32 as *const i32, &key as *const u32 as *const c_void);
        TREE_LOWER_BOUND_0X4FE4F5.original()(set_object() as *const u32, &mut lower as *mut u32 as *const i32, &key as *const u32);
        TREE_ERASE_RANGE_0X4FE4AF.original()(set_object() as *const u32, lower as i32, upper as i32);
    }
}

fn escape_sentinel() -> u32 {
    get_from_memory(base() + ESCAPE_LIST_RVA)
}

fn escape_insert(animal: ZTAnimal) {
    let mut result: u32 = 0;
    unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, escape_sentinel() as i32, &animal.0 as *const u32 as *const i32) };
}

fn escape_count(animal: ZTAnimal) -> usize {
    let sentinel = escape_sentinel();
    let mut count = 0;
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        if get_from_memory::<u32>(node + 8) == animal.0 {
            count += 1;
        }
        node = get_from_memory(node);
    }
    count
}

/// Unlinks every escaped-list node of `animal` (the nodes are leaked).
fn escape_clear(animal: ZTAnimal) {
    let sentinel = escape_sentinel();
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        let next: u32 = get_from_memory(node);
        if get_from_memory::<u32>(node + 8) == animal.0 {
            let previous: u32 = get_from_memory(node + 4);
            save_to_memory(previous, next);
            save_to_memory(next + 4, previous);
        }
        node = next;
    }
}

/// What a pole left behind for `animal`, with the set's size relative to `len_before`.
type Observed = (Option<usize>, usize, i64, usize, u8, u32, u8);

fn observe(animal: ZTAnimal, len_before: u32) -> Observed {
    let values = set_values();
    let rank = values.iter().position(|&value| value == animal.0);
    (
        rank,
        values.iter().filter(|&&value| value == animal.0).count(),
        i64::from(set_len()) - i64::from(len_before),
        escape_count(animal),
        get_from_memory(animal.0 + 0x3a6),
        get_from_memory(animal.0 + 0x398),
        get_from_memory(animal.0 + 0x3a7),
    )
}

fn cleanup(animal: ZTAnimal) {
    erase(animal);
    escape_clear(animal);
}

/// `ZTANIMAL_MULTIANIMAL_SET_LIVE`: `setDying`, `removeFromMap(true)` and `setName` over fixtures, with and without the
/// animal in the panel's set (and the escaped list), for each exclusion byte and egg state. Real, port and hooked agree.
pub(crate) fn run_ztanimal_multianimal_set_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_MULTIANIMAL_SET_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(1);
    let Some(&ty) = types.first() else {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut cases = 0usize;
    let mut seen_removed = false;
    // Runs `body` per pole on a fresh fixture prepared by `prepare` and compares the observations.
    let mut run = |label: String, prepare: &dyn Fn(ZTAnimal), body: &dyn Fn(ZTAnimal, usize), failures: &mut Vec<String>| {
        let mut outcomes: Vec<Observed> = Vec::new();
        let mut before_ranks = None;
        for pole in 0..3usize {
            let animal = build_animal(ty);
            prepare(animal);
            let len_before = set_len();
            before_ranks = before_ranks.or(Some(observe(animal, len_before).0));
            body(animal, pole);
            outcomes.push(observe(animal, len_before));
            cleanup(animal);
        }
        seen_removed |= before_ranks.flatten().is_some() && outcomes[0].0.is_none();
        if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
            failures.push(format!("{label}: (rank, count, set delta, escaped, +0x3a6, +0x398, +0x3a7) real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
        }
        cases += 1;
    };

    for flag in [false, true] {
        for egg in [0u8, 1] {
            for listed in [false, true] {
                run(
                    format!("setDying({flag}) egg {egg} listed {listed}"),
                    &|animal| {
                        save_to_memory(animal.0 + 0x395, egg);
                        save_to_memory(animal.0 + 0x398, 7u32);
                        if listed {
                            insert(animal);
                        }
                    },
                    &|animal, pole| match pole {
                        0 => real::set_dying(animal.0, flag),
                        1 => animal.set_dying(flag),
                        _ => unsafe { SET_DYING.hooked()(animal.0 as *const u32, flag) },
                    },
                    &mut failures,
                );
            }
        }
    }
    for (a, b) in [(0u8, 0u8), (1, 0), (0, 1)] {
        for listed in [false, true] {
            for escaped in [false, true] {
                run(
                    format!("removeFromMap(true) +0x142 {a} +0x143 {b} listed {listed} escaped {escaped}"),
                    &|animal| {
                        save_to_memory(animal.0 + 0x142, a);
                        save_to_memory(animal.0 + 0x143, b);
                        if listed {
                            insert(animal);
                        }
                        if escaped {
                            escape_insert(animal);
                        }
                    },
                    &|animal, pole| match pole {
                        0 => real::remove_from_map(animal.0, true),
                        1 => animal.remove_from_map(true),
                        _ => unsafe { REMOVE_FROM_MAP.hooked()(animal.0 as *const u32, true) },
                    },
                    &mut failures,
                );
            }
        }
    }
    for name in ["Zed", "Aardvark", "Mm"] {
        for listed in [false, true] {
            run(
                format!("setName({name}) listed {listed}"),
                &|animal| {
                    if listed {
                        insert(animal);
                    }
                },
                &|animal, pole| {
                    let text = VanillaString::new(name);
                    let pointer = text.as_ptr() as u32;
                    match pole {
                        0 => real::set_name(animal.0, pointer),
                        1 => animal.set_name(pointer),
                        _ => unsafe { SET_NAME.hooked()(animal.0 as *const u32, pointer as *const u32) },
                    }
                },
                &mut failures,
            );
        }
    }
    if !seen_removed {
        failures.push("no case removed a listed animal from the set".to_string());
    }
    failures.truncate(40);
    finish_test(&format!("{test_name} ({cases} cases)"), failures, failure_log)
}

/// `ZTANIMAL_ADD_REMOVE_LIVE`: one live animal is taken off the map by the real body, put back by each pole, then taken
/// off by each pole (with the flag clear and set) and put back by the real body. The set, the escaped list, the animal's
/// own bytes and the tile bytes `addToMap` edits agree between real, port and hooked.
pub(crate) fn run_ztanimal_add_remove_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_ADD_REMOVE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(&animal_address) = live_animals().first() else {
        failures.push("the live world holds no animal".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let animal = ZTAnimal(animal_address);
    let tile = unsafe { GET_TILE.original()(animal.0 as *const u32) } as u32;
    if tile == 0 {
        failures.push("the live animal has no tile".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let snapshot = |len_before: u32| -> (Observed, Vec<u8>, Vec<u8>) {
        (
            observe(animal, len_before),
            (0x288u32..0x3b8).filter(|o| !(0x26c..0x290).contains(o) && !(0x384..0x390).contains(o)).map(|o| get_from_memory::<u8>(animal.0 + o)).collect(),
            (0x80u32..0x88).map(|o| get_from_memory::<u8>(tile + o)).collect(),
        )
    };
    for flag in [false, true] {
        let mut outcomes = Vec::new();
        for pole in 0..3usize {
            real::remove_from_map(animal.0, true);
            let len_before = set_len();
            match pole {
                0 => {
                    real::add_to_map(animal.0);
                }
                1 => {
                    animal.add_to_map();
                }
                _ => {
                    unsafe { ADD_TO_MAP.hooked()(animal.0 as *const u32) };
                }
            }
            let added = snapshot(len_before);
            let len_before = set_len();
            match pole {
                0 => real::remove_from_map(animal.0, flag),
                1 => animal.remove_from_map(flag),
                _ => unsafe { REMOVE_FROM_MAP.hooked()(animal.0 as *const u32, flag) },
            }
            let removed = snapshot(len_before);
            real::add_to_map(animal.0);
            outcomes.push((added, removed));
        }
        // The real body must have put the animal in the set and taken it out again, or the comparison is empty.
        let ((added_state, _, _), (removed_state, _, _)) = (&outcomes[0].0, &outcomes[0].1);
        let animal_excluded = get_from_memory::<u8>(animal.0 + 0x142) != 0 || get_from_memory::<u8>(animal.0 + 0x143) != 0 || get_from_memory::<u8>(animal.0 + 0x3a6) != 0;
        if !animal_excluded && (added_state.0.is_none() || removed_state.0.is_some()) {
            failures.push(format!("flag {flag}: added rank {:?}, removed rank {:?}", added_state.0, removed_state.0));
        }
        if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
            failures.push(format!("flag {flag}: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
        }
    }
    finish_test(test_name, failures, failure_log)
}
