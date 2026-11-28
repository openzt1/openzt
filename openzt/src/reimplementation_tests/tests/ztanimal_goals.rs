//! Live checks for `ZTAnimal`'s goal pipeline: `updateGoals`, `decide` and `intoNewTile`. Each case runs the real vanilla
//! body, the Rust port and the hooked address on three identically prepared animals (built by the vanilla constructor
//! over a live animal type). Every collaborator the three bodies reach is scripted by a deterministic generator and
//! logged - the animal's own vtable slots, the cog goal's vtable, the `BFCogGoal`/`BFUnit`/`ZTUnit` calls, the world
//! tiles (`getTile`/`getNeighbor` answer fake tiles for the fixtures) and the habitats - so the three poles must agree
//! on the whole call sequence, the return value, the displayed messages and the animal's memory. Nothing here touches the
//! live map: `ZTUnit::intoNewTile` and every goal mutation are recorded instead of run.

use std::{
    ffi::c_void,
    sync::{
        atomic::Ordering,
        Mutex, Once, OnceLock,
    },
};

use openzt_detour::generated::{
    bfaimgr::F_RANDOM_WALK,
    bfcoggoal::{CLEAR as COG_CLEAR, SET as COG_SET, SET_ID as COG_SET_ID, SET_TARGET_POS as COG_SET_TARGET_POS},
    bfentity::GET_TILE,
    bfmap::GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR,
    bfunit::{DECIDE as BFUNIT_DECIDE, GO_TO_DEST_0},
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    ztanimal as generated,
};

use super::{
    ztanimal::{base, build_animal, destroy_animal, live_animal_types, Lcg},
    ztkeeper::{install_goals_hook, GOALS_INTERCEPT, GOALS_LOG},
};
use crate::{
    reimplementation_tests::{display_message_recorder, harness::finish_test},
    util::{get_from_memory, save_to_memory},
    ztanimal::{animal_offsets, live_support::real, ZTAnimal},
};

/// Scripted collaborator state: an LCG drives every answer, every call is logged.
struct Script {
    on: bool,
    state: u32,
    log: Vec<String>,
    /// The fixtures `getTile` answers a fake tile for.
    fixtures: Vec<u32>,
    /// The fake tile `getTile` hands out, and the tiles `getNeighbor` answers for.
    tile: u32,
    neighbour: u32,
    tiles: Vec<u32>,
}

static SCRIPT: Mutex<Script> = Mutex::new(Script { on: false, state: 0, log: Vec::new(), fixtures: Vec::new(), tile: 0, neighbour: 0, tiles: Vec::new() });

fn roll(modulus: u32) -> u32 {
    let mut script = SCRIPT.lock().unwrap();
    script.state = script.state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (script.state >> 8) % modulus
}

fn record(entry: String) {
    SCRIPT.lock().unwrap().log.push(entry);
}

fn scripted() -> bool {
    SCRIPT.lock().unwrap().on
}

fn leaked(bytes: usize) -> u32 {
    Box::leak(vec![0u8; bytes].into_boxed_slice()).as_ptr() as u32
}

fn leaked_c_string(text: &[u8]) -> u32 {
    let mut bytes = text.to_vec();
    bytes.push(0);
    Box::leak(bytes.into_boxed_slice()).as_ptr() as u32
}

fn leaked_vtable(words: Vec<u32>) -> u32 {
    Box::leak(words.into_boxed_slice()).as_ptr() as u32
}

fn text_hash(begin: u32, end: u32) -> u32 {
    let bytes = unsafe { std::slice::from_raw_parts(begin as *const u8, end.wrapping_sub(begin) as usize) };
    bytes.iter().fold(0x811c_9dc5u32, |hash, byte| (hash ^ *byte as u32).wrapping_mul(0x0100_0193))
}

// ---- fake animal vtable slots -------------------------------------------------------------------------------------

extern "thiscall" fn fake_force_anim(_this: u32, text: u32) -> u32 {
    record(format!("forceAnim {:#x}", text_hash(get_from_memory(text), get_from_memory(text + 4))));
    0
}

extern "thiscall" fn fake_set_goal(_this: u32, goal: u32, flag: u32) -> u32 {
    record(format!("setGoal {goal:#x} {}", flag & 0xff));
    0
}

extern "thiscall" fn fake_habitat_from_tile(_this: u32, tile: u32) -> u32 {
    let index = roll(3);
    record(format!("habitatFromTile {index} {}", tile != 0));
    fake_habitats()[index as usize]
}

extern "thiscall" fn fake_habitat(_this: u32) -> u32 {
    let index = roll(3);
    record(format!("habitat {index}"));
    fake_habitats()[index as usize]
}

extern "thiscall" fn fake_find_predator(_this: u32, arg: u32) -> u32 {
    let pick = roll(3);
    record(format!("findPredator {arg} {pick}"));
    if pick == 0 {
        0
    } else {
        0x4000 + pick + arg * 0x10
    }
}

extern "thiscall" fn fake_find_playmate(_this: u32, arg: u32) -> u32 {
    let pick = roll(3);
    record(format!("findPlaymate {arg} {pick}"));
    if pick == 0 {
        0
    } else {
        0x5000 + pick + arg * 0x10
    }
}

/// The upper bytes are deliberately dirty: callers test `AL` only.
extern "thiscall" fn fake_pressing_need(_this: u32) -> u32 {
    let answer = roll(3) == 0;
    record(format!("pressingNeed {answer}"));
    0xabcd_ef00 | answer as u32
}

extern "thiscall" fn fake_notify(_this: u32) {
    record("notify".to_string());
}

extern "thiscall" fn fake_is_doing_show(_this: u32) -> u32 {
    let answer = roll(3) == 0;
    record(format!("isDoingShow {answer}"));
    0xabcd_ef00 | answer as u32
}

extern "thiscall" fn fake_cog_has_tile(_cog: u32) -> u32 {
    let answer = roll(2) == 0;
    record(format!("cogHasTile {answer}"));
    0xabcd_ef00 | answer as u32
}

extern "thiscall" fn fake_habitat_is_tank(_habitat: u32) -> u32 {
    let answer = roll(2) == 0;
    record(format!("habitatIsTank {answer}"));
    0xabcd_ef00 | answer as u32
}

extern "thiscall" fn fake_trample(entity: u32) {
    let index = fake_entities().iter().position(|&candidate| candidate == entity).unwrap_or(99);
    record(format!("trample {index}"));
}

extern "thiscall" fn fake_type_is_cast(this: u32, _tag: u32) -> u32 {
    0xabcd_ef00 | get_from_memory::<u8>(this + FAKE_TYPE_CAST_BYTE) as u32
}

// ---- fake memory ------------------------------------------------------------------------------------------------

const FAKE_TYPE_CAST_BYTE: u32 = 0x60;
const FAKE_TYPE_TRAMPLE_BYTE: u32 = 0x59;
const FAKE_TILE_SIZE: usize = 0x90;
const SLOT_COUNT: u32 = 197;

fn fake_habitats() -> &'static Vec<u32> {
    static HABITATS: OnceLock<Vec<u32>> = OnceLock::new();
    HABITATS.get_or_init(|| {
        let mut vtable = vec![0u32; 0x40];
        vtable[0x20 / 4] = fake_habitat_is_tank as *const () as usize as u32;
        let vtable = leaked_vtable(vtable);
        (0..3)
            .map(|index| {
                let habitat = leaked(0x180);
                save_to_memory(habitat, vtable);
                save_to_memory(habitat + 0x2d, 1u8);
                save_to_memory(habitat + 0x154, leaked_c_string(format!("Habitat{index}").as_bytes()));
                habitat
            })
            .collect()
    })
}

fn fake_entities() -> &'static Vec<u32> {
    static ENTITIES: OnceLock<Vec<u32>> = OnceLock::new();
    ENTITIES.get_or_init(|| {
        let mut entity_vtable = vec![0u32; 0x100];
        entity_vtable[0x124 / 4] = fake_trample as *const () as usize as u32;
        let entity_vtable = leaked_vtable(entity_vtable);
        let mut type_vtable = vec![0u32; 0x40];
        type_vtable[0x1c / 4] = fake_type_is_cast as *const () as usize as u32;
        let type_vtable = leaked_vtable(type_vtable);
        (0..3)
            .map(|_| {
                let entity = leaked(0x140);
                let entity_type = leaked(0x80);
                save_to_memory(entity, entity_vtable);
                save_to_memory(entity + 0x128, entity_type);
                save_to_memory(entity_type, type_vtable);
                entity
            })
            .collect()
    })
}

fn fake_tiles() -> &'static Vec<u32> {
    static TILES: OnceLock<Vec<u32>> = OnceLock::new();
    TILES.get_or_init(|| (0..3).map(|_| leaked(FAKE_TILE_SIZE)).collect())
}

/// Plants per-case habitat and entity state from `rng`.
fn plant_world(rng: &mut Lcg) {
    for &habitat in fake_habitats() {
        save_to_memory(habitat + 0x2c, (rng.next() % 3 == 0) as u8);
        save_to_memory(habitat + 4, if rng.next() % 2 == 0 { 0u32 } else { 0x77 });
        save_to_memory(habitat + 0x18, if rng.next() % 2 == 0 { 0u32 } else { 5 });
    }
    for &entity in fake_entities() {
        let entity_type: u32 = get_from_memory(entity + 0x128);
        save_to_memory(entity_type + FAKE_TYPE_CAST_BYTE, (rng.next() % 4 != 0) as u8);
        save_to_memory(entity_type + FAKE_TYPE_TRAMPLE_BYTE, (rng.next() % 4 != 0) as u8);
    }
    for &tile in fake_tiles() {
        for slot in 0..4u32 {
            let pick = rng.next() % 5;
            save_to_memory(tile + 4 + slot * 4, if pick >= 3 { 0 } else { fake_entities()[pick as usize] });
        }
    }
}

// ---- scripted calls ---------------------------------------------------------------------------------------------

#[openzt_detour_macro::detour_mod]
mod goal_seams {
    use super::*;

    #[detour(COG_SET)]
    unsafe extern "thiscall" fn cog_set(this: *const u32, goal: i32, sub: i32, target: u32, arg: *const i32, flag: i32) {
        if scripted() {
            record(format!("cogSet {goal:#x} {sub} {target:#x} {:#x} {flag}", arg as u32 - base()));
        } else {
            unsafe { COG_SET_DETOUR.call(this, goal, sub, target, arg, flag) }
        }
    }

    #[detour(COG_SET_ID)]
    unsafe extern "thiscall" fn cog_set_id(this: *const u32, id: u32, arg: *const i32) {
        if scripted() {
            record(format!("cogSetId {id:#x} {:#x}", arg as u32 - base()));
        } else {
            unsafe { COG_SET_ID_DETOUR.call(this, id, arg) }
        }
    }

    #[detour(COG_CLEAR)]
    unsafe extern "fastcall" fn cog_clear(this: *const c_void) {
        if scripted() {
            record("cogClear".to_string());
        } else {
            unsafe { COG_CLEAR_DETOUR.call(this) }
        }
    }

    #[detour(COG_SET_TARGET_POS)]
    unsafe extern "thiscall" fn cog_set_target_pos(this: *const u32, position: *const i32) {
        if scripted() {
            let words: Vec<u32> = (0..3).map(|i| get_from_memory::<u32>(position as u32 + i * 4)).collect();
            record(format!("cogSetTargetPos {words:x?}"));
        } else {
            unsafe { COG_SET_TARGET_POS_DETOUR.call(this, position) }
        }
    }

    #[detour(GO_TO_DEST_0)]
    unsafe extern "thiscall" fn go_to_dest(this: *const u32, name: *const u32, flag_a: u32, flag_b: i8) -> u32 {
        if scripted() {
            let succeeded = roll(3) == 0;
            let hash = text_hash(get_from_memory(name as u32), get_from_memory(name as u32 + 4));
            record(format!("goToDest {hash:#x} {flag_a} {flag_b} {succeeded}"));
            0xdead_be00 | succeeded as u32
        } else {
            unsafe { GO_TO_DEST_0_DETOUR.call(this, name, flag_a, flag_b) }
        }
    }

    #[detour(BFUNIT_DECIDE)]
    unsafe extern "thiscall" fn unit_decide(this: *const u32) -> u32 {
        if scripted() {
            let answer = roll(3);
            record(format!("BFUnit decide {answer}"));
            0xcafe_0000 | answer
        } else {
            unsafe { BFUNIT_DECIDE_DETOUR.call(this) }
        }
    }

    #[detour(F_RANDOM_WALK)]
    unsafe extern "thiscall" fn random_walk(this: *const u32, unit: *const i32) -> u32 {
        if scripted() {
            let answer = roll(4);
            let none_at = base() + 0x0023_9370;
            let words: Vec<u32> = if roll(2) == 0 { (0..3).map(|i| get_from_memory::<u32>(none_at + i * 4)).collect() } else { (0..3).map(|_| roll(1000)).collect() };
            for (i, word) in words.iter().enumerate() {
                save_to_memory(unit as u32 + 0x1dc + i as u32 * 4, *word);
            }
            record(format!("fRandomWalk {answer} {words:x?}"));
            0xabcd_ef00 | answer
        } else {
            unsafe { F_RANDOM_WALK_DETOUR.call(this, unit) }
        }
    }

    #[detour(GET_TILE)]
    unsafe extern "thiscall" fn get_tile(this: *const u32) -> i32 {
        if scripted() && SCRIPT.lock().unwrap().fixtures.contains(&(this as u32)) {
            SCRIPT.lock().unwrap().tile as i32
        } else {
            unsafe { GET_TILE_DETOUR.call(this) }
        }
    }

    #[detour(BFMAP_GET_NEIGHBOR)]
    unsafe extern "thiscall" fn get_neighbor(this: *const u32, tile: *const u32, direction: u32) -> u32 {
        let fake = scripted() && SCRIPT.lock().unwrap().tiles.contains(&(tile as u32));
        if fake {
            record(format!("getNeighbor {direction}"));
            SCRIPT.lock().unwrap().neighbour
        } else {
            unsafe { BFMAP_GET_NEIGHBOR_DETOUR.call(this, tile, direction) }
        }
    }
}

fn unit_into_new_tile_hook(_this: u32, direction: u32) -> bool {
    if scripted() {
        record(format!("ZTUnit intoNewTile {direction}"));
        true
    } else {
        false
    }
}

fn scripted_is_need_or_fun_satisfier(_this: u32, need: u32, position: *const i32) -> Option<bool> {
    if !scripted() {
        return None;
    }
    let answer = roll(2) == 0;
    let words: Vec<u32> = (0..3).map(|i| get_from_memory::<u32>(position as u32 + i * 4)).collect();
    record(format!("isNeedOrFunSatisfier {need:#x} {words:x?} {answer}"));
    Some(answer)
}

fn scripted_update_building_tiles(_this: u32, flag: bool) -> Option<bool> {
    if !scripted() {
        return None;
    }
    let answer = roll(2) == 0;
    record(format!("updateBuildingTiles {flag} {answer}"));
    Some(answer)
}

fn scripted_has_building_use_goal(_this: u32) -> Option<bool> {
    if !scripted() {
        return None;
    }
    let answer = roll(2) == 0;
    record(format!("hasBuildingUseGoal {answer}"));
    Some(answer)
}

fn install_seams() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { goal_seams::init_detours() } {
            result = Err(format!("could not install the goal-pipeline seams: {e:?}"));
        }
    });
    *crate::ztunit::live_support::intercept::INTO_NEW_TILE.lock().unwrap() = Some(unit_into_new_tile_hook);
    *crate::ztunit::live_support::intercept::UPDATE_BUILDING_TILES.lock().unwrap() = Some(scripted_update_building_tiles);
    *crate::ztunit::live_support::intercept::HAS_BUILDING_USE_GOAL.lock().unwrap() = Some(scripted_has_building_use_goal);
    *crate::ztunit::live_support::intercept::IS_NEED_OR_FUN_SATISFIER.lock().unwrap() = Some(scripted_is_need_or_fun_satisfier);
    result.and_then(|()| install_goals_hook())
}

// ---- fixtures ---------------------------------------------------------------------------------------------------

/// What `install_vtables` replaced on a fixture, put back before the vanilla destructor runs (it frees the name block
/// through vanilla's allocator, which must never see Rust-owned memory).
static SAVED: Mutex<Vec<(u32, [u32; 3])>> = Mutex::new(Vec::new());

/// Gives `animal` the scripted vtable, a scripted cog-goal vtable and a name block of its own.
fn install_vtables(animal: ZTAnimal) {
    let real_vtable: u32 = get_from_memory(animal.0);
    SAVED.lock().unwrap().push((animal.0, [real_vtable, get_from_memory(animal.0 + 0x15c), get_from_memory(animal.0 + 0x108)]));
    let mut vtable: Vec<u32> = (0..SLOT_COUNT).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    let scripted_slots: [(u32, usize); 9] = [
        (0x104, fake_force_anim as *const () as usize),
        (0x1d8, fake_set_goal as *const () as usize),
        (0x248, fake_habitat_from_tile as *const () as usize),
        (0x24c, fake_habitat as *const () as usize),
        (0x2fc, fake_find_predator as *const () as usize),
        (0x2f8, fake_find_playmate as *const () as usize),
        (0x288, fake_pressing_need as *const () as usize),
        (0x2c4, fake_notify as *const () as usize),
        (0x22c, fake_is_doing_show as *const () as usize),
    ];
    for (slot, function) in scripted_slots {
        vtable[(slot / 4) as usize] = function as u32;
    }
    save_to_memory(animal.0, leaked_vtable(vtable));
    let cog = animal.0 + 0x15c;
    let real_cog_vtable: u32 = get_from_memory(cog);
    let mut cog_vtable: Vec<u32> = (0..16u32).map(|slot| get_from_memory(real_cog_vtable + slot * 4)).collect();
    cog_vtable[2] = fake_cog_has_tile as *const () as usize as u32;
    save_to_memory(cog, leaked_vtable(cog_vtable));
    save_to_memory(animal.0 + 0x108, leaked_c_string(b"Fixture"));
}

fn build_fixtures(ty: u32) -> [ZTAnimal; 3] {
    let animals = [build_animal(ty), build_animal(ty), build_animal(ty)];
    for animal in animals {
        install_vtables(animal);
    }
    animals
}

fn destroy_fixtures(animals: [ZTAnimal; 3]) {
    for animal in animals {
        reset_list(animal);
        free_record(animal);
        let saved = {
            let mut all = SAVED.lock().unwrap();
            all.iter().position(|(address, _)| *address == animal.0).map(|index| all.remove(index).1)
        };
        if let Some([vtable, cog_vtable, name]) = saved {
            save_to_memory(animal.0, vtable);
            save_to_memory(animal.0 + 0x15c, cog_vtable);
            save_to_memory(animal.0 + 0x108, name);
        }
        destroy_animal(animal);
    }
}

/// Frees the nodes of the animal's goal list and resets it to empty.
fn reset_list(animal: ZTAnimal) {
    let head: u32 = get_from_memory(animal.0 + 0x154);
    let mut node: u32 = get_from_memory(head);
    while node != head {
        let next: u32 = get_from_memory(node);
        unsafe { POOLALLOC_DEALLOCATE.original()(node as *const u32, 24) };
        node = next;
    }
    save_to_memory(head, head);
    save_to_memory(head + 4, head);
}

/// Pushes `count` pool nodes onto the animal's goal list.
fn fill_list(animal: ZTAnimal, count: u32) {
    let head: u32 = get_from_memory(animal.0 + 0x154);
    for index in 0..count {
        let node = unsafe { POOLALLOC_ALLOCATE.original()(24) } as u32;
        let last: u32 = get_from_memory(head + 4);
        save_to_memory(node, head);
        save_to_memory(node + 4, last);
        save_to_memory(node + 8, index);
        save_to_memory(last, node);
        save_to_memory(head + 4, node);
    }
}

/// A description of the goal list when it is not a circular list of at most `limit` nodes.
fn list_problem(animal: ZTAnimal, limit: u32) -> Option<String> {
    let head: u32 = get_from_memory(animal.0 + 0x154);
    if head < 0x10000 {
        return Some(format!("goal list head pointer {head:#x}"));
    }
    let mut node: u32 = get_from_memory(head);
    for _ in 0..=limit {
        if node == head {
            return None;
        }
        if node < 0x10000 {
            return Some(format!("goal list node {node:#x} (head {head:#x}, head+4 {:#x})", get_from_memory::<u32>(head + 4)));
        }
        node = get_from_memory(node);
    }
    let mut walk = Vec::new();
    let mut node: u32 = get_from_memory(head);
    for _ in 0..10 {
        walk.push(node);
        if node < 0x10000 {
            break;
        }
        node = get_from_memory(node);
    }
    let mut free_chain = Vec::new();
    let mut free: u32 = get_from_memory(base() + 0x0023_8008);
    for _ in 0..8 {
        free_chain.push(free);
        if free < 0x10000 {
            break;
        }
        free = get_from_memory(free);
    }
    Some(format!("goal list longer than {limit} (head {head:#x}, walk {walk:x?}, bucket-2 freelist {free_chain:x?})"))
}

fn free_record(animal: ZTAnimal) {
    let record: u32 = get_from_memory(animal.0 + animal_offsets::ESCAPE_RECORD);
    if record != 0 {
        unsafe { OPERATOR_DELETE.original()(record) };
    }
    save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, 0u32);
    save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, 0u8);
}

/// A fresh escape record whose tiles are chosen by `tile_of`.
fn plant_record(animal: ZTAnimal, rng: &mut Lcg, tile_of: &dyn Fn(u32) -> u32) {
    free_record(animal);
    let record = unsafe { OPERATOR_NEW.original()(0x10) } as u32;
    unsafe { std::ptr::write_bytes(record as *mut u8, 0, 0x10) };
    save_to_memory(record + 1, (rng.next() % 2) as u8);
    save_to_memory(record + 2, (rng.next() % 2) as u8);
    save_to_memory(record + 3, (rng.next() % 2) as u8);
    save_to_memory(record + 4, tile_of(rng.next()));
    save_to_memory(record + 8, tile_of(rng.next()));
    save_to_memory(record + 0xc, (rng.next() % 6) as u32);
    save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, record);
    save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, 1u8);
}

const SNAPSHOT_RANGES: [(u32, u32); 6] = [(0x138, 0x13c), (0x1dc, 0x1e8), (0x214, 0x260), (0x290, 0x384), (0x388, 0x3b4), (0x170, 0x174)];

/// The state words of an animal, with every pool pointer left out; the goal list and escape record reduced to whether
/// they hold anything and the record to its contents.
fn snapshot(animal: ZTAnimal) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (start, end) in SNAPSHOT_RANGES {
        for offset in start..end {
            bytes.push(get_from_memory::<u8>(animal.0 + offset));
        }
    }
    let head: u32 = get_from_memory(animal.0 + 0x154);
    bytes.push((get_from_memory::<u32>(head) != head) as u8);
    let record: u32 = get_from_memory(animal.0 + animal_offsets::ESCAPE_RECORD);
    bytes.push((record != 0) as u8);
    if record != 0 {
        for offset in 0..0x10u32 {
            let byte = get_from_memory::<u8>(record + offset);
            // Offsets 4..12 hold fake-tile pointers shared by all poles.
            bytes.push(byte);
        }
    }
    bytes
}

fn snapshot_position(index: usize) -> String {
    let mut remaining = index as u32;
    for (start, end) in SNAPSHOT_RANGES {
        if remaining < end - start {
            return format!("+{:#x}", start + remaining);
        }
        remaining -= end - start;
    }
    format!("list/record byte {remaining}")
}

/// What one pole of one case produced.
struct Outcome {
    answer: u32,
    log: Vec<String>,
    goals: Vec<[u32; 4]>,
    messages: Vec<(String, i32, u32, u32)>,
    state: Vec<u8>,
}

/// Runs `body` on `animal` with the script seeded by `seed`, capturing the logs, messages and state.
fn run_pole(animal: ZTAnimal, seed: u32, body: &dyn Fn(ZTAnimal) -> u32) -> Outcome {
    {
        let mut script = SCRIPT.lock().unwrap();
        script.state = seed;
        script.log.clear();
    }
    GOALS_LOG.lock().unwrap().clear();
    display_message_recorder::begin_capture();
    let answer = body(animal);
    let messages = display_message_recorder::end_capture();
    let normalised = messages.into_iter().map(|(text, priority, tile, entity)| (text, priority, tile, (entity == animal.0) as u32)).collect();
    Outcome { answer, log: SCRIPT.lock().unwrap().log.clone(), goals: GOALS_LOG.lock().unwrap().clone(), messages: normalised, state: snapshot(animal) }
}

/// Compares the three poles of one case, recording a failure for each disagreement.
fn compare(label: &str, outcomes: &[Outcome; 3], mask: u32, failures: &mut Vec<String>) {
    let (real, port, hooked) = (&outcomes[0], &outcomes[1], &outcomes[2]);
    if real.answer & mask != port.answer & mask || real.answer & mask != hooked.answer & mask {
        failures.push(format!("{label}: answer real {:#x}, port {:#x}, hooked {:#x}", real.answer & mask, port.answer & mask, hooked.answer & mask));
    }
    for (name, other) in [("port", port), ("hooked", hooked)] {
        if real.log != other.log {
            failures.push(format!("{label}: calls real {:?}, {name} {:?}", real.log, other.log));
        }
        if real.goals != other.goals {
            failures.push(format!("{label}: ZTUnit::updateGoals calls real {:?}, {name} {:?}", real.goals, other.goals));
        }
        if real.messages != other.messages {
            failures.push(format!("{label}: messages real {:?}, {name} {:?}", real.messages, other.messages));
        }
        if real.state != other.state {
            let first = real.state.iter().zip(&other.state).position(|(a, b)| a != b);
            failures.push(format!("{label}: state differs from real for {name} (first differing byte {:?})", first.map(snapshot_position)));
        }
    }
}

/// Starts the scripted run: the fixtures answer a fake tile, and `ZTUnit::updateGoals` is recorded.
fn begin_script(fixtures: &[ZTAnimal; 3]) {
    let tiles = fake_tiles();
    let mut script = SCRIPT.lock().unwrap();
    script.on = true;
    script.fixtures = fixtures.iter().map(|animal| animal.0).collect();
    script.tile = tiles[0];
    script.neighbour = tiles[1];
    script.tiles = tiles.clone();
    GOALS_INTERCEPT.store(1, Ordering::SeqCst);
}

fn end_script() {
    SCRIPT.lock().unwrap().on = false;
    GOALS_INTERCEPT.store(0, Ordering::SeqCst);
}

fn fixture_type(failures: &mut Vec<String>) -> Option<u32> {
    let types = live_animal_types(1);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return None;
    }
    if let Err(message) = install_seams() {
        failures.push(message);
        return None;
    }
    Some(types[0])
}

/// A single fixed flag byte planted per case: `probability` out of `of` draws set it.
fn draw_byte(rng: &mut Lcg, probability: u32, of: u32) -> u8 {
    (rng.next() % of < probability) as u8
}

const GOAL_IDS: [u32; 18] = [0, 1, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x72, 0x73, 0x74, 0x75, 0x7a, 0x7c, 0x85, 0x86, 0x70, 0x71, 0x99];

/// `ZTANIMAL_UPDATE_GOALS_LIVE`: `updateGoals` over random combinations of every flag, goal id and scripted answer the
/// threat and needs stages read. Fails when the sweep never starts one of the goals it can start.
pub(crate) fn run_ztanimal_update_goals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_UPDATE_GOALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(ty) = fixture_type(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let fixtures = build_fixtures(ty);
    begin_script(&fixtures);
    let mut seen = std::collections::BTreeSet::new();
    let mut mismatches = 0usize;
    for case in 0..6000u32 {
        let seed = case.wrapping_mul(2_654_435_761).wrapping_add(17);
        let mut outcomes = Vec::new();
        for (pole, animal) in fixtures.iter().enumerate() {
            let mut rng = Lcg(seed);
            plant_world(&mut rng);
            let early = rng.next() % 6 == 0;
            for offset in [0x3a6u32, 0x393, 0x394, 0x395] {
                save_to_memory(animal.0 + offset, if early { draw_byte(&mut rng, 1, 3) } else { 0 });
            }
            for offset in [0x24cu32, 0x25c, 0x3a3, 0x3a4, 0x24f, 0x250, 0x39f, 0x3a0, 0x3a1, 0x3a2] {
                save_to_memory(animal.0 + offset, draw_byte(&mut rng, 1, 3));
            }
            save_to_memory(animal.0 + 0x170, GOAL_IDS[(rng.next() as usize) % GOAL_IDS.len()]);
            save_to_memory(animal.0 + 0x22c, if rng.next() % 2 == 0 { 0u32 } else { 0x4001 });
            let mate_like = [0u32, 0x4001, 0x5001, 0x5002, 0x4002][(rng.next() % 5) as usize];
            save_to_memory(animal.0 + 0x2e4, mate_like);
            save_to_memory(animal.0 + 0x1a8, rng.next() % 2);
            save_to_memory(animal.0 + 0x15c + 0x4c, (rng.next() % 2) as u8);
            save_to_memory(animal.0 + 0x138, 0u32);
            for offset in 0..3u32 {
                save_to_memory(animal.0 + 0x214 + offset * 4, rng.next() % 50);
            }
            save_to_memory(animal.0 + 0x244, rng.next() % 40);
            save_to_memory(animal.0 + 0x258, if rng.next() % 2 == 0 { 4u32 } else { 0 });
            save_to_memory(animal.0 + 0x360, if rng.next() % 2 == 0 { 0i32 } else { 1_000_000 });
            reset_list(*animal);
            fill_list(*animal, rng.next() % 3);
            outcomes.push(run_pole(*animal, seed ^ 0x5a5a, &|animal| match pole {
                0 => {
                    real::update_goals(animal.0);
                    0
                }
                1 => {
                    animal.update_goals();
                    0
                }
                _ => {
                    unsafe { generated::UPDATE_GOALS.hooked()(animal.0 as *const u32) };
                    0
                }
            }));
        }
        let outcomes: [Outcome; 3] = outcomes.try_into().ok().unwrap();
        if mismatches < 10 {
            let before = failures.len();
            compare(&format!("case {case}"), &outcomes, 0, &mut failures);
            if failures.len() > before {
                mismatches += 1;
            }
        }
        for entry in &outcomes[0].log {
            seen.insert(entry.split(' ').take(2).collect::<Vec<_>>().join(" "));
        }
        for call in &outcomes[0].goals {
            seen.insert(format!("goals {call:?}"));
        }
    }
    for wanted in [
        "setGoal 0x75",
        "setGoal 0x85",
        "setGoal 0x86",
        "setGoal 0x72",
        "setGoal 0x73",
        "setGoal 0x7a",
        "setGoal 0x7c",
        "cogSet 0x74",
        "cogSetId 0x4001",
        "cogClear",
        "updateBuildingTiles false",
        "goals [6, 0, 0, 0]",
        "notify",
    ] {
        if !seen.iter().any(|entry| entry.starts_with(wanted)) {
            failures.push(format!("the sweep never reached {wanted:?}"));
        }
    }
    end_script();
    destroy_fixtures(fixtures);
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_DECIDE_LIVE`: `decide` over the goal id, the cog goal's tile answer, the goal list, the scripted random
/// walk (answer and target, equal to the no-position value or not), the scripted `goToDest` answers, the escape record
/// and a missing `ZTAIMgr`. Fails when the sweep never reaches the escape-record release or the target being set.
pub(crate) fn run_ztanimal_decide_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_DECIDE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(ty) = fixture_type(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let fixtures = build_fixtures(ty);
    begin_script(&fixtures);
    let aimgr_global = base() + 0x0023_8098;
    let real_aimgr: u32 = get_from_memory(aimgr_global);
    let mut seen = std::collections::BTreeSet::new();
    let mut mismatches = 0usize;
    for case in 0..3000u32 {
        let seed = case.wrapping_mul(2_654_435_761).wrapping_add(91);
        let no_manager = case % 25 == 7;
        save_to_memory(aimgr_global, if no_manager { 0u32 } else { real_aimgr });
        let mut outcomes = Vec::new();
        for (pole, animal) in fixtures.iter().enumerate() {
            let mut rng = Lcg(seed);
            plant_world(&mut rng);
            save_to_memory(animal.0 + 0x170, if rng.next() % 5 == 0 { [0u32, 2, 0x74][(rng.next() % 3) as usize] } else { 1 });
            if let Some(problem) = list_problem(*animal, 8) {
                failures.push(format!("case {case} pole {pole} before reset: {problem}"));
                break;
            }
            reset_list(*animal);
            fill_list(*animal, rng.next() % 3);
            if let Some(problem) = list_problem(*animal, 8) {
                failures.push(format!("case {case} pole {pole} after fill: {problem}"));
                break;
            }
            free_record(*animal);
            if rng.next() % 2 == 0 {
                plant_record(*animal, &mut rng, &|draw| fake_tiles()[(draw % 3) as usize]);
            }
            for offset in 0..3u32 {
                save_to_memory(animal.0 + 0x1dc + offset * 4, rng.next() % 100);
            }
            outcomes.push(run_pole(*animal, seed ^ 0xa5a5, &|animal| match pole {
                0 => real::decide(animal.0),
                1 => animal.decide(),
                _ => unsafe { generated::DECIDE.hooked()(animal.0 as *const u32) },
            }));
        }
        let Ok(outcomes) = <[Outcome; 3]>::try_from(outcomes) else { break };
        if mismatches < 10 {
            let before = failures.len();
            compare(&format!("case {case}{}", if no_manager { " (no manager)" } else { "" }), &outcomes, 0xff, &mut failures);
            if failures.len() > before {
                mismatches += 1;
            }
        }
        for entry in &outcomes[0].log {
            seen.insert(entry.split(' ').take(2).collect::<Vec<_>>().join(" "));
        }
        if outcomes[0].log.iter().filter(|entry| entry.starts_with("goToDest") && entry.ends_with("false")).count() == 2 {
            seen.insert("released record".to_string());
        }
        for fixture in &fixtures {
            reset_list(*fixture);
            free_record(*fixture);
        }
    }
    save_to_memory(aimgr_global, real_aimgr);
    for wanted in ["BFUnit decide", "cogHasTile", "fRandomWalk", "cogSetTargetPos", "goToDest", "released record"] {
        if !seen.iter().any(|entry| entry.starts_with(wanted)) {
            failures.push(format!("the sweep never reached {wanted:?}"));
        }
    }
    end_script();
    destroy_fixtures(fixtures);
    finish_test(test_name, failures, failure_log)
}

/// `ZTANIMAL_INTO_NEW_TILE_LIVE`: `intoNewTile` over a planted escape record (its two tiles among the fixture tile, its
/// neighbour and a third; the reached/transition bytes and the kind), a rampage flag, scenery entities on the tile's
/// four slots (some repeated, some not scenery) and scripted habitats for the tile and its neighbour. The tile, its
/// neighbour and the habitats are fakes, so no live map state changes; the displayed "left habitat" message is captured.
pub(crate) fn run_ztanimal_into_new_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_INTO_NEW_TILE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(ty) = fixture_type(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let fixtures = build_fixtures(ty);
    begin_script(&fixtures);
    let mut seen = std::collections::BTreeSet::new();
    let mut messages = 0usize;
    let mut mismatches = 0usize;
    for case in 0..4000u32 {
        let seed = case.wrapping_mul(2_654_435_761).wrapping_add(5);
        let direction = case % 8;
        let mut outcomes = Vec::new();
        for (pole, animal) in fixtures.iter().enumerate() {
            let mut rng = Lcg(seed);
            plant_world(&mut rng);
            free_record(*animal);
            if rng.next() % 3 != 0 {
                plant_record(*animal, &mut rng, &|draw| fake_tiles()[(draw % 3) as usize]);
            }
            save_to_memory(animal.0 + 0x3b0, draw_byte(&mut rng, 2, 3));
            outcomes.push(run_pole(*animal, seed ^ 0x3c3c, &|animal| {
                match pole {
                    0 => real::into_new_tile(animal.0, direction),
                    1 => animal.into_new_tile(direction),
                    _ => unsafe { generated::INTO_NEW_TILE.hooked()(animal.0 as *const u32, direction) },
                }
                0
            }));
        }
        let outcomes: [Outcome; 3] = outcomes.try_into().ok().unwrap();
        if mismatches < 10 {
            let before = failures.len();
            compare(&format!("case {case}"), &outcomes, 0, &mut failures);
            if failures.len() > before {
                mismatches += 1;
            }
        }
        messages += outcomes[0].messages.len();
        for entry in &outcomes[0].log {
            seen.insert(entry.split(' ').take(1).collect::<Vec<_>>().join(" "));
        }
        for fixture in &fixtures {
            free_record(*fixture);
        }
    }
    for wanted in ["trample", "forceAnim", "ZTUnit", "habitatFromTile"] {
        if !seen.iter().any(|entry| entry.starts_with(wanted)) {
            failures.push(format!("the sweep never reached {wanted:?}"));
        }
    }
    if messages == 0 {
        failures.push("the sweep never displayed the left-habitat message".to_string());
    }
    end_script();
    destroy_fixtures(fixtures);
    finish_test(test_name, failures, failure_log)
}
