//! Live checks of `ZTBuilding`'s users and slots (`ztbuilding/users.rs`). The ported method and the vanilla body run one
//! after the other over the same live building and the same fixture guests, whose memory is put back between the two, so
//! the answers, the calls made and every word written can be compared directly.
//!
//! The exit, sound, animation and water collaborators are scripted, and so are the guest's virtual slots and the world
//! manager's `getEntity`, which must find the fixtures. The slot positions are the ported ones on both sides
//! (`ztbuilding_geometry.rs` compares those).

use std::{
    mem::{offset_of, size_of},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex,
    },
};

use super::ztguest_checks::{self, SANDBOX, SANDBOX_LOG};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::{get_module_base, globals},
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztbuilding::{
        building_offsets,
        live_support::{build_standalone_building, destroy_standalone_building, intercept, users_real},
        users::{slot_type_can_add, ZTSlot},
        ZTBuilding, ZTBuildingSlot, BUILDING_SIZE,
    },
    ztguest::{
        live_support::{build_standalone_guest, destroy_standalone_guest, find_guest_type, goals_overrides::THOUGHT},
        GUEST_SIZE,
    },
    ztmegatilemgr::entity_type_matches,
};

const BUILDING_TAG_RVA: u32 = 0x0023_8680;
const RNG_STATE_RVA: u32 = 0x0023_8060;
const TRASH_CAN_TYPE_ID: u32 = 0x1779;
/// How much of `ZTGameMgr` a payment can write: cash and the embedded `ZooStatus`.
const GAME_BYTES: u32 = 0x1200;
const GUESTS: usize = 3;
/// How many vtable entries a scripted copy of a guest's or a building's vtable keeps.
const VTABLE_SLOTS: u32 = 220;
/// The guest's carried-item vector.
const GUEST_ITEMS: u32 = 0x26c;

// ---- scripted collaborators ----------------------------------------------------------------------------------------

pub(super) static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub(super) static STUBBING: AtomicBool = AtomicBool::new(false);
static VALID_EXIT: AtomicU32 = AtomicU32::new(1);
pub(super) static RETURNING: AtomicU32 = AtomicU32::new(0);
static RETURN_ID: AtomicU32 = AtomicU32::new(0);
pub(super) static WATER: AtomicU32 = AtomicU32::new(0);
/// `(id, entity)` pairs the scripted `getEntity` answers.
pub(super) static ENTITIES: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

pub(super) fn log(entry: String) {
    LOG.lock().unwrap().push(entry);
}

pub(super) fn take_log() -> Vec<String> {
    let mut log = std::mem::take(&mut *LOG.lock().unwrap());
    log.append(&mut SANDBOX_LOG.lock().unwrap());
    log
}

fn stubbing() -> bool {
    STUBBING.load(Ordering::SeqCst)
}

fn scripted_use_sound(_building: u32) -> bool {
    let scripted = stubbing();
    if scripted {
        log("mightStartUseSound".to_string());
    }
    scripted
}

fn scripted_use_animation(_building: u32) -> bool {
    let scripted = stubbing();
    if scripted {
        log("mightStartUseAnimation".to_string());
    }
    scripted
}

fn scripted_return_bs(_building: u32) -> Option<Vec<u8>> {
    stubbing().then(|| {
        log("getReturnBS".to_string());
        Vec::new()
    })
}

fn scripted_water_capacity(_building: u32) -> Option<i32> {
    stubbing().then(|| {
        log("calculateWaterCapacity".to_string());
        WATER.load(Ordering::SeqCst) as i32
    })
}

fn scripted_valid_exit(_building: u32, unit: u32, slot: u32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    log(format!("getValidExit {} {slot}", unit != 0));
    Some(VALID_EXIT.load(Ordering::SeqCst) != 0)
}

fn scripted_exit_via_exit(_building: u32, unit: u32, slot: u32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    log(format!("exitViaExit {unit:#x} {slot}"));
    Some(true)
}

pub(super) fn install() -> Result<(), String> {
    ztguest_checks::install_hooks()?;
    // All answer only while this module is stubbing.
    *intercept::VALID_EXIT.lock().unwrap() = Some(scripted_valid_exit);
    *intercept::EXIT_VIA_EXIT.lock().unwrap() = Some(scripted_exit_via_exit);
    *intercept::MIGHT_START_USE_SOUND.lock().unwrap() = Some(scripted_use_sound);
    *intercept::MIGHT_START_USE_ANIMATION.lock().unwrap() = Some(scripted_use_animation);
    *intercept::GET_RETURN_BS.lock().unwrap() = Some(scripted_return_bs);
    *intercept::CALCULATE_WATER_CAPACITY.lock().unwrap() = Some(scripted_water_capacity);
    Ok(())
}

extern "thiscall" fn fake_before_place(this: u32, flag: u32) {
    log(format!("beforePlace {this:#x} {}", flag & 0xff));
}
extern "thiscall" fn fake_after_place(this: u32) {
    log(format!("afterPlace {this:#x}"));
}
pub(super) extern "thiscall" fn fake_set_visible(this: u32, flag: u32) {
    log(format!("setVisible {this:#x} {}", flag & 0xff));
}
extern "thiscall" fn fake_is_returning(_this: u32) -> u32 {
    RETURNING.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_set_returning(this: u32, flag: u32) {
    log(format!("setReturning {this:#x} {}", flag & 0xff));
}
extern "thiscall" fn fake_set_return_id(this: u32, id: u32) {
    log(format!("setReturnBuildingID {this:#x} {id:#x}"));
}
extern "thiscall" fn fake_get_return_id(_this: u32) -> u32 {
    RETURN_ID.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_set_return_name(this: u32, name: u32) {
    let (begin, end): (u32, u32) = (get_from_memory(name), get_from_memory(name + 4));
    let text: String = (begin..end).map(|address| get_from_memory::<u8>(address) as char).collect();
    log(format!("setReturnBuildingString {this:#x} {text:?}"));
}
extern "thiscall" fn fake_walk_out(this: u32) {
    log(format!("walkOut {this:#x}"));
}
fn scripted_thought(_this: u32, id: u32, subject: u32, _habitat: u32) -> bool {
    if !stubbing() {
        return false;
    }
    log(format!("thought {id:#x} {subject:#x}"));
    true
}

/// A leaked copy of the vtable at `object`, `slots` entries long, with `scripted` slots replaced.
pub(super) fn scripted_vtable(object: u32, slots: u32, scripted: &[(u32, u32)]) -> u32 {
    let real: u32 = get_from_memory(object);
    let mut vtable: Vec<u32> = (0..slots).map(|slot| get_from_memory(real + slot * 4)).collect();
    for &(offset, function) in scripted {
        vtable[(offset / 4) as usize] = function;
    }
    Box::leak(vtable.into_boxed_slice()).as_ptr() as u32
}

static REAL_GET_ENTITY: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_get_entity(this: u32, id: u32, flag: u32) -> u32 {
    if let Some(&(_, entity)) = ENTITIES.lock().unwrap().iter().find(|&&(known, _)| known == id) {
        return entity;
    }
    let real: extern "thiscall" fn(u32, u32, u32) -> u32 = unsafe { std::mem::transmute(REAL_GET_ENTITY.load(Ordering::SeqCst)) };
    real(this, id, flag)
}

// ---- fixtures ------------------------------------------------------------------------------------------------------

pub(super) fn word(address: u32) -> u32 {
    get_from_memory(address)
}

pub(super) fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

pub(super) fn read_words(address: u32, bytes: u32) -> Vec<u32> {
    (0..bytes / 4).map(|index| word(address + index * 4)).collect()
}

fn write_words(address: u32, words: &[u32]) {
    for (index, &value) in words.iter().enumerate() {
        save_to_memory(address + index as u32 * 4, value);
    }
}

pub(super) fn rng() -> u32 {
    word(base() + RNG_STATE_RVA)
}

pub(super) fn pin_rng(seed: u32) {
    save_to_memory(base() + RNG_STATE_RVA, seed);
}

/// One live building per distinct type that has slots.
pub(super) fn buildings_with_slots() -> Vec<ZTBuilding> {
    let world = globals().ztworldmgr();
    let mut seen = Vec::new();
    (world.entity_array_start()..world.entity_array_end())
        .step_by(4)
        .map(word)
        .filter(|&entity| entity != 0 && unsafe { entity_type_matches(entity, BUILDING_TAG_RVA) })
        .map(ZTBuilding)
        .filter(|building| {
            let fresh = building.slot_count() > 0 && !seen.contains(&building.entity_type());
            seen.push(building.entity_type());
            fresh
        })
        .collect()
}

/// The fixture guests, their vtables scripted and their ids known to the scripted `getEntity`.
pub(super) struct Guests {
    pub(super) guests: Vec<u32>,
    vtables: Vec<u32>,
}

impl Guests {
    pub(super) fn new(guest_type: u32) -> Guests {
        let guests: Vec<u32> = (0..GUESTS).map(|_| build_standalone_guest(guest_type).0).collect();
        let vtables: Vec<u32> = guests.iter().map(|&guest| word(guest)).collect();
        let world = globals().ztworldmgr_ptr() as u32;
        let (width, height) = (globals().ztworldmgr().map_x_size, globals().ztworldmgr().map_y_size);
        for (index, &guest) in guests.iter().enumerate() {
            let vtable = scripted_vtable(
                guest,
                VTABLE_SLOTS,
                &[
                    (0x74, fake_after_place as *const () as u32),
                    (0x78, fake_before_place as *const () as u32),
                    (0xcc, fake_set_visible as *const () as u32),
                    (0x264, fake_is_returning as *const () as u32),
                    (0x274, fake_set_returning as *const () as u32),
                    (0x294, fake_set_return_id as *const () as u32),
                    (0x298, fake_get_return_id as *const () as u32),
                    (0x29c, fake_set_return_name as *const () as u32),
                    (0x2c0, fake_walk_out as *const () as u32),
                ],
            );
            save_to_memory(guest, vtable);
            save_to_memory(guest + 0x124, 0x7f00_0001 + index as u32);
            save_to_memory(guest + 0x114, width / 2 * 64 + 32);
            save_to_memory(guest + 0x118, height / 2 * 64 + 32);
            save_to_memory(guest + 0x11c, 0u32);
        }
        *ENTITIES.lock().unwrap() = guests.iter().map(|&guest| (word(guest + 0x124), guest)).collect();
        let world_vtable = scripted_vtable(world, 64, &[(0x24, fake_get_entity as *const () as u32)]);
        REAL_GET_ENTITY.store(word(word(world) + 0x24), Ordering::SeqCst);
        let mut all_vtables = vtables;
        all_vtables.push(word(world));
        save_to_memory(world, world_vtable);
        Guests { guests, vtables: all_vtables }
    }

    pub(super) fn destroy(self) {
        save_to_memory(globals().ztworldmgr_ptr() as u32, self.vtables[GUESTS]);
        ENTITIES.lock().unwrap().clear();
        for (index, guest) in self.guests.into_iter().enumerate() {
            save_to_memory(guest, self.vtables[index]);
            destroy_standalone_guest(crate::ztguest::ZTGuest(guest));
        }
    }
}

/// Everything a user method may write, put back after each run.
struct Saved {
    regions: Vec<(u32, Vec<u32>)>,
}

impl Saved {
    fn take(building: ZTBuilding, guests: &[u32]) -> Saved {
        let mut regions = vec![(building.0, read_words(building.0, BUILDING_SIZE))];
        let slots = word(building.0 + building_offsets::SLOTS);
        let slot_bytes = building.slot_count() as u32 * size_of::<ZTBuildingSlot>() as u32;
        regions.push((slots, read_words(slots, slot_bytes)));
        for index in 0..building.slot_count() as u32 {
            let slot = slots + index * size_of::<ZTBuildingSlot>() as u32;
            let (begin, end) = (word(slot + 0x18), word(slot + 0x1c));
            regions.push((begin, read_words(begin, end - begin)));
        }
        for &guest in guests {
            regions.push((guest, read_words(guest, GUEST_SIZE)));
            let (begin, end) = (word(guest + GUEST_ITEMS), word(guest + GUEST_ITEMS + 4));
            regions.push((begin, read_words(begin, end - begin)));
        }
        let game = globals().ztgamemgr_ptr() as u32;
        regions.push((game, read_words(game, GAME_BYTES)));
        Saved { regions }
    }

    /// Puts the memory back. Only the tail of the building is restored: the base holds the mixer and other containers
    /// vanilla may have reallocated.
    fn restore(&self, building: ZTBuilding) {
        for (address, words) in &self.regions {
            if *address == building.0 {
                let tail = building_offsets::MEMBERS;
                write_words(building.0 + tail, &words[(tail / 4) as usize..]);
                save_to_memory(building.0 + 0xd6, get_from_memory::<u8>(words.as_ptr() as u32 + 0xd6));
            } else {
                write_words(*address, words);
            }
        }
    }
}

/// An empty building: no users or members (the headers are put back by [`Saved::restore`]), and empty slots.
fn empty_building(building: ZTBuilding) {
    for offset in [building_offsets::MEMBERS, building_offsets::USERS] {
        write_words(building.0 + offset, &[0, 0, 0]);
    }
    save_to_memory(building.0 + building_offsets::OCCUPANCY, 0u32);
    save_to_memory(building.0 + building_offsets::SHOW_ID, 0u16);
    let slots = word(building.0 + building_offsets::SLOTS);
    for index in 0..building.slot_count() as u32 {
        let slot = slots + index * size_of::<ZTBuildingSlot>() as u32;
        save_to_memory(slot + 4, 0u32);
        save_to_memory(slot + 0x14, 0u32);
        ZTSlot(slot).clear_draw_users();
    }
}

pub(super) fn vector(address: u32) -> Vec<u32> {
    let (begin, end) = (word(address), word(address + 4));
    (begin..end).step_by(4).map(word).collect()
}

/// What an operation left behind, labelled.
fn observe(building: ZTBuilding, guests: &[u32]) -> Vec<(String, Vec<u32>)> {
    let mut state = vec![
        (
            "tail".to_string(),
            (building_offsets::UPGRADE_INDEX..BUILDING_SIZE)
                .step_by(4)
                .filter(|&offset| offset != building_offsets::USE_SOUND && offset != building_offsets::USE_SOUND + 4)
                .map(|offset| word(building.0 + offset))
                .collect(),
        ),
        ("users".to_string(), vector(building.0 + building_offsets::USERS)),
        ("members".to_string(), vector(building.0 + building_offsets::MEMBERS)),
        ("redraw".to_string(), vec![u32::from(get_from_memory::<u8>(building.0 + 0xd6))]),
        ("rng".to_string(), vec![rng()]),
    ];
    let slots = word(building.0 + building_offsets::SLOTS);
    for index in 0..building.slot_count() as u32 {
        let slot = slots + index * size_of::<ZTBuildingSlot>() as u32;
        let mut words = vec![word(slot + 4), word(slot + 0x14)];
        words.extend(vector(slot + 0x18));
        state.push((format!("slot {index}"), words));
    }
    for (index, &guest) in guests.iter().enumerate() {
        state.push((format!("guest {index}"), read_words(guest, GUEST_SIZE)));
        state.push((format!("items {index}"), vector(guest + GUEST_ITEMS)));
    }
    let game = globals().ztgamemgr_ptr() as u32;
    state.push(("game".to_string(), read_words(game, GAME_BYTES)));
    state
}

fn compare_states(label: &str, real: &[(String, Vec<u32>)], port: &[(String, Vec<u32>)], failures: &mut Vec<String>) {
    for ((name, real), (_, port)) in real.iter().zip(port) {
        // Words that differ only in being two separate heap blocks (the guests' strings) agree.
        let agree = real.len() == port.len() && real.iter().zip(port).all(|(&r, &p)| r == p || (name.starts_with("guest") && r >= 0x10000 && p >= 0x10000));
        if !agree {
            let first = real.iter().zip(port).position(|(r, p)| r != p);
            let detail = match first {
                Some(at) => format!("word {at}: real {:#x}, port {:#x}", real[at], port[at]),
                None => format!("lengths {} and {}", real.len(), port.len()),
            };
            failures.push(format!("{label} {name}: {detail}"));
        }
    }
}

// ---- operations ----------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Op {
    Add(usize, u32),
    CanAdd(usize, u32),
    HasOpenSlot(usize),
    Remove(usize, bool),
    RemoveIfPresent(usize),
    Yank(usize),
    YankIfPresent(usize),
    RemoveAll,
    BuyItem(usize),
    RemoveItems(usize),
    Gift(usize),
}

/// Runs one operation through the vanilla body (`real`) or the port, answering as a number.
fn run(op: Op, real: bool, building: ZTBuilding, guests: &[u32]) -> i64 {
    let this = building.0;
    let guest = |index: usize| guests.get(index).copied().unwrap_or(0);
    let pick = |real_answer: &dyn Fn() -> bool, port_answer: &dyn Fn() -> bool| i64::from(if real { real_answer() } else { port_answer() });
    match op {
        Op::Add(g, slot) => pick(&|| users_real::add_user(this, guest(g), slot), &|| building.add_user(guest(g), slot)),
        Op::CanAdd(g, slot) => pick(&|| users_real::can_add_user(this, guest(g), slot), &|| building.can_add_user(guest(g), slot)),
        Op::HasOpenSlot(g) => pick(&|| users_real::has_open_slot(this, guest(g)), &|| building.has_open_slot(guest(g))),
        Op::Remove(g, eject) => pick(&|| users_real::remove_user(this, guest(g), eject), &|| building.remove_user(guest(g), eject)),
        Op::RemoveIfPresent(g) => pick(&|| users_real::remove_user_if_present(this, guest(g)), &|| building.remove_user_if_present(guest(g))),
        Op::Yank(g) => {
            let answer = if real { users_real::yank_user(this, guest(g)) } else { building.yank_user(guest(g)) };
            answer.map_or(i64::MIN, i64::from)
        }
        Op::YankIfPresent(g) => pick(&|| users_real::yank_user_if_present(this, guest(g)), &|| building.yank_user_if_present(guest(g))),
        Op::RemoveAll => {
            if real {
                users_real::remove_all_users(this);
            } else {
                building.remove_all_users();
            }
            0
        }
        Op::BuyItem(g) => pick(&|| users_real::buy_item(this, guest(g)), &|| building.buy_item(guest(g))),
        Op::RemoveItems(g) => pick(&|| users_real::remove_items(this, guest(g)), &|| building.remove_items(guest(g))),
        Op::Gift(g) => pick(&|| users_real::get_fun_gift_if_available(this, guest(g)), &|| building.get_fun_gift_if_available(guest(g))),
    }
}

const SEQUENCE: [Op; 33] = [
    Op::HasOpenSlot(0),
    Op::CanAdd(0, 0),
    Op::CanAdd(0, 1),
    Op::CanAdd(0, 0xff),
    Op::Add(0, 0),
    Op::Add(1, 0),
    Op::Add(2, 0),
    Op::Add(0, 1),
    Op::HasOpenSlot(1),
    Op::Remove(1, false),
    Op::Add(1, 0),
    Op::Remove(2, false),
    Op::RemoveIfPresent(0),
    Op::RemoveIfPresent(2),
    Op::Add(2, 0),
    Op::Add(0, 0),
    Op::Yank(0),
    Op::Yank(9),
    Op::YankIfPresent(9),
    Op::YankIfPresent(1),
    Op::YankIfPresent(1),
    Op::Add(1, 1),
    Op::Remove(1, true),
    Op::Add(0, 0),
    Op::Add(1, 0),
    Op::RemoveAll,
    Op::Remove(0, false),
    Op::Add(0, 0),
    Op::BuyItem(0),
    Op::BuyItem(1),
    Op::RemoveItems(0),
    Op::Gift(0),
    Op::RemoveAll,
];

/// The type flags the user methods branch on, as one scenario each.
#[derive(Clone, Copy, Debug)]
struct Variant {
    name: &'static str,
    flags: &'static [(usize, u8)],
    returning: bool,
    /// The goal every guest has: `0x68` is a building-use goal, so a guest can become a slot's top user.
    goal: u32,
    show_running: u32,
    price: f32,
}

const COMMERCE: usize = 0x23d;
const VARIANTS: [Variant; 6] = [
    Variant { name: "hidden shop", flags: &[(offset_of!(ZTBuildingType, hide_user), 1), (COMMERCE, 1), (0x240, 0)], returning: false, goal: 0x68, show_running: 1, price: 0.5 },
    Variant { name: "dear shop", flags: &[(offset_of!(ZTBuildingType, hide_user), 1), (COMMERCE, 1), (0x23c, 1), (0x240, 0)], returning: false, goal: 0, show_running: 1, price: 5000.0 },
    Variant {
        name: "open building",
        flags: &[(offset_of!(ZTBuildingType, hide_user), 0), (offset_of!(ZTBuildingType, user_stays_outside), 0), (offset_of!(ZTBuildingType, user_uses_exit), 0), (COMMERCE, 0), (0x240, 0)],
        returning: false,
        goal: 0x69,
        show_running: 0,
        price: 1.0,
    },
    Variant {
        name: "facing outside",
        flags: &[(offset_of!(ZTBuildingType, hide_user), 0), (offset_of!(ZTBuildingType, user_stays_outside), 1), (offset_of!(ZTBuildingType, set_letter_facing), 1), (offset_of!(ZTBuildingType, user_uses_exit), 1), (0x240, 0)],
        returning: false,
        goal: 0x6a,
        show_running: 1,
        price: 0.0,
    },
    Variant { name: "rental", flags: &[(offset_of!(ZTBuildingType, hide_user), 1), (offset_of!(ZTBuildingType, user_tracker), 1), (0x240, 1)], returning: false, goal: 0x68, show_running: 1, price: 1.0 },
    Variant { name: "rental return", flags: &[(offset_of!(ZTBuildingType, hide_user), 1), (offset_of!(ZTBuildingType, user_tracker), 0), (0x240, 1)], returning: true, goal: 0x68, show_running: 1, price: 1.0 },
];

/// `ZTBUILDING_USERS_LIVE`: the user methods over live buildings under every type variant.
pub(crate) fn run_ztbuilding_users_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_USERS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let buildings = buildings_with_slots();
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    if buildings.is_empty() {
        return finish_test(test_name, vec!["no live building with slots".to_string()], failure_log);
    }
    let guests = Guests::new(guest_type);
    let saved_hook = THOUGHT.lock().unwrap().replace(scripted_thought);
    let saved_rng = rng();
    STUBBING.store(true, Ordering::SeqCst);
    SANDBOX.store(true, Ordering::SeqCst);
    let (mut added, mut removed, mut placed, mut rented) = (0u32, 0u32, 0u32, 0u32);

    for (building_index, &building) in buildings.iter().take(16).enumerate() {
        let building_type = building.entity_type();
        let saved_type = read_words(building_type, size_of::<ZTBuildingType>() as u32);
        for variant in VARIANTS {
            let saved = Saved::take(building, &guests.guests);
            write_words(building_type, &saved_type);
            for &(offset, value) in variant.flags {
                save_to_memory(building_type + offset as u32, value);
            }
            RETURNING.store(u32::from(variant.returning), Ordering::SeqCst);
            // The rental return admits only a guest carrying this building's type id.
            let type_id = {
                let slot = word(word(building_type) + 0x20);
                let function: extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(slot) };
                function(building_type)
            };
            RETURN_ID.store(type_id, Ordering::SeqCst);
            WATER.store(2, Ordering::SeqCst);
            VALID_EXIT.store(1, Ordering::SeqCst);
            let prepare = || {
                saved.restore(building);
                empty_building(building);
                save_to_memory(building.0 + building_offsets::PRICE, variant.price);
                save_to_memory(building.0 + building_offsets::WATER_CAPACITY, 2u32);
                building.set_type_state(variant.show_running as i32);
                for &guest in &guests.guests {
                    save_to_memory(guest + 0x170, variant.goal);
                }
                pin_rng(0x4000 + building_index as u32);
                take_log();
            };
            let mut results = [Vec::new(), Vec::new()];
            for (side, result) in results.iter_mut().enumerate() {
                prepare();
                for op in SEQUENCE {
                    let answer = run(op, side == 0, building, &guests.guests);
                    result.push((answer, take_log(), observe(building, &guests.guests)));
                }
            }
            saved.restore(building);
            let [real, port] = results;
            for ((op, real), port) in SEQUENCE.iter().zip(&real).zip(&port) {
                let label = format!("building type {building_type:#x} {} {op:?}", variant.name);
                if real.0 != port.0 {
                    failures.push(format!("{label}: real answers {}, port {}", real.0, port.0));
                }
                if real.1 != port.1 {
                    failures.push(format!("{label}: real calls {:?}, port calls {:?}", real.1, port.1));
                }
                compare_states(&label, &real.2, &port.2, &mut failures);
                added += u32::from(matches!(op, Op::Add(..)) && port.0 == 1);
                removed += u32::from(matches!(op, Op::Remove(..)) && port.0 == 1);
                placed += u32::from(port.1.iter().any(|call| call.starts_with("beforePlace")));
                rented += u32::from(port.1.iter().any(|call| call.starts_with("getReturnBS")));
                if failures.len() > 40 {
                    break;
                }
            }
            if failures.len() > 40 {
                break;
            }
        }
        write_words(building_type, &saved_type);
    }

    SANDBOX.store(false, Ordering::SeqCst);
    STUBBING.store(false, Ordering::SeqCst);
    *THOUGHT.lock().unwrap() = saved_hook;
    pin_rng(saved_rng);
    take_log();
    guests.destroy();
    tracing::info!("{test_name}: {added} added, {removed} removed, {placed} placed, {rented} rented");
    if added == 0 || removed == 0 || placed == 0 || rented == 0 {
        failures.push(format!("a branch was never exercised (added {added}, removed {removed}, placed {placed}, rented {rented})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTBUILDING_SLOT_HELPERS_LIVE`: the draw-user helpers and `SlotType::canAdd` over fake records and stand-in units.
pub(crate) fn run_ztbuilding_slot_helpers_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_SLOT_HELPERS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    let guest = build_standalone_guest(guest_type).0;
    let real_vtable = word(guest);
    save_to_memory(guest, scripted_vtable(guest, VTABLE_SLOTS, &[(0x264, fake_is_returning as *const () as u32)]));

    // A slot `{index, user count, draw count, flag, building, top user, draw users}` over three draw entries.
    let mut entries = [0u32; 3];
    let begin = entries.as_mut_ptr() as u32;
    let mut slot = [0u32, 0, 3, 1, 0, 0, begin, begin + 12, begin + 12];
    let slot_address = slot.as_mut_ptr() as u32;
    let handle = ZTSlot(slot_address);
    for goal in [0u32, 0x67, 0x68, 0x69, 0x6a, 0x6b] {
        save_to_memory(guest + 0x170, goal);
        for top in [0u32, guest] {
            save_to_memory(slot_address + 0x14, top);
            let (real, port) = (users_real::get_top_user(slot_address), handle.top_user());
            if real != port {
                failures.push(format!("getTopUser goal {goal:#x} top {top:#x}: real {real:x?}, port {port:x?}"));
            }
        }
        for unit in [0u32, guest] {
            let mut results = [(false, 0u32); 2];
            for (side, result) in results.iter_mut().enumerate() {
                save_to_memory(slot_address + 0x14, 0xabcd_u32);
                let answer = if side == 0 { users_real::set_top_user(slot_address, unit) } else { handle.set_top_user(unit) };
                *result = (answer, word(slot_address + 0x14));
            }
            if results[0] != results[1] {
                failures.push(format!("setTopUser goal {goal:#x} unit {unit:#x}: {results:x?}"));
            }
        }
    }
    let contents = [[0u32; 3], [5, 0, 5], [1, 5, 2], [7, 8, 9]];
    for start in contents {
        for unit in [0u32, 5, 9, 100] {
            let mut results = [Vec::new(), Vec::new()];
            for (side, result) in results.iter_mut().enumerate() {
                write_words(begin, &start);
                if side == 0 {
                    users_real::remove_draw_user(slot_address, unit);
                } else {
                    handle.remove_draw_user(unit);
                }
                *result = read_words(begin, 12);
                write_words(begin, &start);
                if side == 0 {
                    users_real::clear_draw_users(slot_address);
                } else {
                    handle.clear_draw_users();
                }
                result.extend(read_words(begin, 12));
            }
            if results[0] != results[1] {
                failures.push(format!("remove/clearDrawUsers {start:?} {unit}: {results:?}"));
            }
        }
    }

    // `SlotType::canAdd` over a scripted record.
    let id = {
        let function: extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(word(word(guest_type) + 0x20)) };
        function(guest_type)
    };
    let name: Vec<u8> = crate::vanilla_string::c_string_bytes(word(guest_type + 0xa4));
    let leak_text = |text: Vec<u8>| Box::leak(text.into_iter().chain([0]).collect::<Vec<u8>>().into_boxed_slice()).as_ptr() as u32;
    let filters = [
        0u32,
        leak_text(Vec::new()),
        leak_text(name.clone()),
        leak_text([b"x ", name.as_slice(), b" y"].concat()),
        leak_text(b"somethingelse".to_vec()),
        leak_text(name[..name.len().saturating_sub(1)].to_vec()),
    ];
    let ids: [u32; 3] = [id.wrapping_add(1), id, id.wrapping_add(2)];
    let ids_at = ids.as_ptr() as u32;
    let mut record = vec![0u32; size_of::<ZTBuildingSlotType>() / 4];
    let record_address = record.as_mut_ptr() as u32;
    let (mut admitted, mut refused) = (0u32, 0u32);
    for (first, last) in [(0u32, 0u32), (ids_at, ids_at + 4), (ids_at, ids_at + 8), (ids_at, ids_at + 12)] {
        for is_return in [0u8, 1] {
            for returning in [0u32, 1] {
                for &filter in &filters {
                    save_to_memory(record_address + offset_of!(ZTBuildingSlotType, user_type_ids) as u32, first);
                    save_to_memory(record_address + offset_of!(ZTBuildingSlotType, user_type_ids) as u32 + 4, last);
                    save_to_memory(record_address + offset_of!(ZTBuildingSlotType, is_return_slot) as u32, is_return);
                    save_to_memory(record_address + offset_of!(ZTBuildingSlotType, filter) as u32, filter);
                    RETURNING.store(returning, Ordering::SeqCst);
                    let real = users_real::slot_type_can_add(record_address, guest_type, guest);
                    let port = slot_type_can_add(record_address, guest_type, guest);
                    if real != port {
                        failures.push(format!("canAdd ids {} return {is_return} returning {returning} filter {filter:#x}: real {real}, port {port}", (last - first) / 4));
                    }
                    admitted += u32::from(port);
                    refused += u32::from(!port);
                }
            }
        }
    }
    if admitted == 0 || refused == 0 {
        failures.push(format!("canAdd was one-sided: {admitted} admitted, {refused} refused"));
    }
    save_to_memory(guest, real_vtable);
    destroy_standalone_guest(crate::ztguest::ZTGuest(guest));
    finish_test(test_name, failures, failure_log)
}

/// The mixer's base links as `(anim, reference count)`.
fn base_links(building: ZTBuilding) -> Vec<(u32, u32)> {
    let mixer = building.0 + 0x74;
    [mixer, mixer + 0x0c]
        .into_iter()
        .flat_map(|array| (word(array)..word(array + 4)).step_by(12).map(word).collect::<Vec<_>>())
        .map(|anim| (anim, if anim == 0 { 0 } else { word(anim + 0x20) }))
        .collect()
}

/// Every loaded building type.
pub(super) fn building_types() -> Vec<u32> {
    let world = globals().ztworldmgr();
    let tag = base() + BUILDING_TAG_RVA;
    (world.entity_type_array_start..world.entity_type_array_end)
        .step_by(4)
        .map(word)
        .filter(|&entity_type| {
            entity_type != 0 && {
                let slot = word(word(entity_type) + 0x1c);
                let is_cast_class: extern "thiscall" fn(u32, u32) -> bool = unsafe { std::mem::transmute(slot) };
                is_cast_class(entity_type, tag)
            }
        })
        .collect()
}

fn type_id(entity_type: u32) -> u32 {
    let function: extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(word(word(entity_type) + 0x20)) };
    function(entity_type)
}

pub(super) extern "thiscall" fn fake_send_event(this: u32, kind: u32, target: u32, flag: u32, value: u32, extra: u32, count: u32) {
    log(format!("sendEvent {this:#x} {:#x} {target:#x} {:#x} {value:#x} {} {}", kind & 0xffff, flag & 0xff, extra & 0xffff, count & 0xffff));
}

/// `ZTBUILDING_ITEMS_LIVE`: `removeItems`, `buyItem` and `getFunGiftIfAvailable` over a fixture of every building type,
/// for a guest carrying different things, at each side of a trash can's fill levels and of the price reactions.
pub(crate) fn run_ztbuilding_items_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_ITEMS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    let guests = Guests::new(guest_type);
    let guest = guests.guests[0];
    let saved_hook = THOUGHT.lock().unwrap().replace(scripted_thought);
    let saved_rng = rng();
    STUBBING.store(true, Ordering::SeqCst);
    SANDBOX.store(true, Ordering::SeqCst);
    let (mut sold, mut taken, mut noticed, mut thrown, mut thoughts) = (0u32, 0u32, 0u32, 0u32, 0u32);

    for (type_index, building_type) in building_types().into_iter().enumerate() {
        let building = build_standalone_building(building_type);
        let real_vtable = word(building.0);
        save_to_memory(building.0, scripted_vtable(building.0, 78, &[(0, fake_send_event as *const () as u32)]));
        let count = |offset: u32| (word(building_type + offset + 4) - word(building_type + offset)) / 4;
        sold += u32::from(count(0x200) > 0);
        taken += u32::from(count(0x20c) > 0);
        for price in [0.0f32, 1.0, 5000.0] {
            for fill in [0i32, 12, 13, 24, 25] {
                for carried in [[0u32; 5], [0, 0, 0, 0, 0x18a6], [0x18a6, 7, 8, 9, 10]] {
                    let saved = Saved::take(building, &guests.guests);
                    let mut results = Vec::new();
                    for real in [true, false] {
                        saved.restore(building);
                        save_to_memory(building.0 + building_offsets::PRICE, price);
                        building.set_type_state(fill);
                        let items = word(guest + GUEST_ITEMS);
                        if items != 0 {
                            write_words(items, &carried);
                        }
                        pin_rng(0x7000 + type_index as u32);
                        take_log();
                        let mut run_results = Vec::new();
                        for op in [Op::RemoveItems(0), Op::BuyItem(0), Op::Gift(0), Op::RemoveItems(9), Op::BuyItem(9), Op::Gift(9)] {
                            let answer = run(op, real, building, &guests.guests);
                            run_results.push((op, answer, take_log(), observe(building, &guests.guests)));
                        }
                        results.push(run_results);
                    }
                    saved.restore(building);
                    for ((op, real_answer, real_calls, real_state), (_, port_answer, port_calls, port_state)) in results[0].iter().zip(&results[1]) {
                        let label = format!("type {building_type:#x} price {price} fill {fill} carrying {carried:x?} {op:?}");
                        if real_answer != port_answer {
                            failures.push(format!("{label}: real answers {real_answer}, port {port_answer}"));
                        }
                        if real_calls != port_calls {
                            failures.push(format!("{label}: real calls {real_calls:?}, port calls {port_calls:?}"));
                        }
                        compare_states(&label, real_state, port_state, &mut failures);
                        noticed += u32::from(port_calls.iter().any(|call| call.starts_with("sendEvent")));
                        thrown += u32::from(port_calls.iter().any(|call| call.starts_with("throwTrash")));
                        thoughts += u32::from(port_calls.iter().any(|call| call.starts_with("thought")));
                    }
                    if failures.len() > 40 {
                        break;
                    }
                }
            }
        }
        save_to_memory(building.0, real_vtable);
        destroy_standalone_building(building);
        if failures.len() > 40 {
            break;
        }
    }

    SANDBOX.store(false, Ordering::SeqCst);
    STUBBING.store(false, Ordering::SeqCst);
    *THOUGHT.lock().unwrap() = saved_hook;
    pin_rng(saved_rng);
    take_log();
    guests.destroy();
    tracing::info!("{test_name}: {sold} types sell, {taken} take, {noticed} trash notices, {thrown} trash thrown, {thoughts} thoughts");
    if sold == 0 || taken == 0 || noticed == 0 || thrown == 0 || thoughts == 0 {
        failures.push(format!("a branch was never exercised (sell {sold}, take {taken}, notices {noticed}, thrown {thrown}, thoughts {thoughts})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTBUILDING_TRASH_LIVE`: `userDataChanged` over fixtures of every building type, and `emptyGrandstands` over every
/// live building with the show cleared, so that neither side has a show to abort.
pub(crate) fn run_ztbuilding_trash_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_TRASH_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let (mut cans, mut animated) = (0u32, 0u32);
    for building_type in building_types() {
        cans += u32::from(type_id(building_type) == TRASH_CAN_TYPE_ID);
        let (real, port) = (build_standalone_building(building_type), build_standalone_building(building_type));
        for state in [0i32, 12, 13, 24, 25, 100, -1, 0] {
            real.set_type_state(state);
            port.set_type_state(state);
            users_real::user_data_changed(real.0);
            port.user_data_changed();
            let anims = |building| base_links(building).iter().map(|link| link.0).collect::<Vec<_>>();
            if anims(real) != anims(port) {
                failures.push(format!("userDataChanged type {building_type:#x} state {state}: real {:x?}, port {:x?}", anims(real), anims(port)));
            }
        }
        animated += u32::from(type_id(building_type) == TRASH_CAN_TYPE_ID && !base_links(port).is_empty());
        // Both mixers hold the same animations, so equal changes mean neither side leaks or over-releases a reference.
        let counts = |building| base_links(building).iter().map(|link| i64::from(link.1)).collect::<Vec<_>>();
        let delta = |building: ZTBuilding, change: &dyn Fn()| {
            let before = counts(building);
            change();
            counts(building).iter().zip(&before).map(|(&after, &before)| after - before).collect::<Vec<i64>>()
        };
        let real_delta = delta(real, &|| users_real::user_data_changed(real.0));
        let port_delta = delta(port, &|| port.user_data_changed());
        if real_delta != port_delta {
            failures.push(format!("userDataChanged type {building_type:#x} reference counts: real {real_delta:?}, port {port_delta:?}"));
        }
        destroy_standalone_building(real);
        destroy_standalone_building(port);
    }
    let world = globals().ztworldmgr();
    for building in (world.entity_array_start()..world.entity_array_end())
        .step_by(4)
        .map(word)
        .filter(|&entity| entity != 0 && unsafe { entity_type_matches(entity, BUILDING_TAG_RVA) })
        .map(ZTBuilding)
    {
        let (saved_show, state) = (building.show_id(), building.type_state());
        save_to_memory(building.0 + building_offsets::SHOW_ID, 0u16);
        users_real::empty_grandstands(building.0);
        let real_state = building.type_state();
        building.empty_grandstands();
        if (real_state, building.type_state()) != (state, state) {
            failures.push(format!("emptyGrandstands without a show changed the state of {:#x}", building.0));
        }
        save_to_memory(building.0 + building_offsets::SHOW_ID, saved_show);
    }
    tracing::info!("{test_name}: {cans} trash can types, {animated} with a fill animation");
    if cans == 0 || animated == 0 {
        failures.push(format!("no trash can was animated ({cans} trash can types)"));
    }
    finish_test(test_name, failures, failure_log)
}
