//! Live checks of `ZTBuilding`'s lifecycle and persistence (`ztbuilding/lifecycle.rs`): the ported method against the
//! vanilla body, on twin fixtures or on the same live building.
//!
//! Where a test is about what the building does around its `ZTScenery` base or another manager, those are scripted here
//! and record their calls. The fixture guests and the sound, water and exit stubs are those of `ztbuilding_users`.

use std::{
    ffi::c_void,
    mem::{offset_of, size_of},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Once,
    },
};

use glam::IVec3;
use openzt_detour::{
    generated::{
        bfentity::CREATE_NAME as BFENTITY_CREATE_NAME,
        bfentitytype::GET_UNIT_COUNT,
        standalone::{OPERATOR_DELETE, OPERATOR_NEW},
        ztscenery::{ADD_TO_MAP as SCENERY_ADD_TO_MAP, INIT_AFTER_LOAD as SCENERY_INIT_AFTER_LOAD, LOAD as SCENERY_LOAD, REMOVE_FROM_MAP as SCENERY_REMOVE_FROM_MAP, SAVE as SCENERY_SAVE},
        ztui_showpanel::UPDATE_AVAILABLE_TRICK_LIST,
        ztworldmgr::{ADD_TO_ALL_BUILDING_LISTS, CREATE_SCENERY, REMOVE_FROM_ALL_BUILDING_LISTS},
    },
};

use super::ztbuilding_users::{
    building_types, buildings_with_slots, fake_send_event, fake_set_visible, install, log, pin_rng, read_words, rng, scripted_vtable,
    take_log, vector, word, Guests, ENTITIES, STUBBING, WATER,
};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::globals,
    reimplementation_tests::{harness::finish_test, io_redirect},
    util::{get_from_memory, save_to_memory},
    ztbuilding::{
        building_offsets,
        users::ZTSlot,
        live_support::{build_standalone_building, destroy_standalone_building, intercept, lifecycle_real},
        ZTBuilding, ZTBuildingSlot, BUILDING_SIZE,
    },
    ztguest::{live_support::find_guest_type, GUEST_SIZE},
};


const TRASH_CAN_TYPE_ID: u32 = 0x1779;
const ENTITY_TYPE: u32 = 0x128;
const ENTITY_ID: u32 = 0x124;
/// A save version past every gate of `ZTBuilding::load`.
const LATEST_VERSION: u32 = 0x100;
/// An id the scripted and the real `getEntity` both know nothing of.
const MISSING_ID: u32 = 0x7f00_00ff;

// ---- scripted collaborators ----------------------------------------------------------------------------------------

static SCRIPTING: AtomicBool = AtomicBool::new(false);
static BASE_ADDED: AtomicU32 = AtomicU32::new(1);
static LISTS_ADDED: AtomicU32 = AtomicU32::new(1);
pub(super) static SCENERY_CREATED: AtomicU32 = AtomicU32::new(1);
static BASE_LOADED: AtomicU32 = AtomicU32::new(1);
static SOUND_PLAYING: AtomicU32 = AtomicU32::new(1);
pub(super) static TYPE_ID: AtomicU32 = AtomicU32::new(0);

fn scripting() -> bool {
    SCRIPTING.load(Ordering::SeqCst)
}

#[openzt_detour_macro::detour_mod]
mod stubs {
    use super::*;

    #[detour(SCENERY_ADD_TO_MAP)]
    unsafe extern "thiscall" fn scenery_add_to_map(this: *const u32) -> u32 {
        if scripting() {
            log("ZTScenery::addToMap".to_string());
            BASE_ADDED.load(Ordering::SeqCst)
        } else {
            unsafe { SCENERY_ADD_TO_MAP_DETOUR.call(this) }
        }
    }

    #[detour(ADD_TO_ALL_BUILDING_LISTS)]
    unsafe extern "thiscall" fn add_to_lists(this: *const u32, building: *const u32) -> u32 {
        if scripting() {
            log("addToAllBuildingLists".to_string());
            LISTS_ADDED.load(Ordering::SeqCst)
        } else {
            unsafe { ADD_TO_ALL_BUILDING_LISTS_DETOUR.call(this, building) }
        }
    }

    #[detour(UPDATE_AVAILABLE_TRICK_LIST)]
    unsafe extern "cdecl" fn trick_list(argument: *const i32) {
        if scripting() {
            log(format!("updateAvailableTrickList {:#x}", argument as u32));
        } else {
            unsafe { UPDATE_AVAILABLE_TRICK_LIST_DETOUR.call(argument) }
        }
    }

    #[detour(SCENERY_REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn scenery_remove_from_map(this: *const c_void, keep: i8) {
        if scripting() {
            log(format!("ZTScenery::removeFromMap {keep}"));
        } else {
            unsafe { SCENERY_REMOVE_FROM_MAP_DETOUR.call(this, keep) }
        }
    }

    #[detour(REMOVE_FROM_ALL_BUILDING_LISTS)]
    unsafe extern "thiscall" fn remove_from_lists(this: *const u32, building: *const i32) -> u32 {
        if scripting() {
            log("removeFromAllBuildingLists".to_string());
            1
        } else {
            unsafe { REMOVE_FROM_ALL_BUILDING_LISTS_DETOUR.call(this, building) }
        }
    }

    #[detour(CREATE_SCENERY)]
    unsafe extern "thiscall" fn create_scenery(this: *const u32, scenery_type: *const u32, flag: i32, tile: *const u32, offset: IVec3) -> *const u32 {
        if scripting() {
            log(format!("createScenery {:#x} {flag} {:#x} {offset:?}", scenery_type as u32, tile as u32));
            SCENERY_CREATED.load(Ordering::SeqCst) as *const u32
        } else {
            unsafe { CREATE_SCENERY_DETOUR.call(this, scenery_type, flag, tile, offset) }
        }
    }

    #[detour(GET_UNIT_COUNT)]
    unsafe extern "cdecl" fn get_unit_count(name: *const u8, flag: *const i8) -> i32 {
        if scripting() {
            log(format!("getUnitCount {:#x} {:#x}", name as u32, flag as u32));
            7
        } else {
            unsafe { GET_UNIT_COUNT_DETOUR.call(name, flag) }
        }
    }

    #[detour(BFENTITY_CREATE_NAME)]
    unsafe extern "thiscall" fn entity_create_name(this: *const u32) {
        if scripting() {
            log("BFEntity::createName".to_string());
        } else {
            unsafe { BFENTITY_CREATE_NAME_DETOUR.call(this) }
        }
    }

    #[detour(SCENERY_INIT_AFTER_LOAD)]
    unsafe extern "thiscall" fn scenery_init_after_load(this: *const c_void, version: u32) -> bool {
        if scripting() {
            log(format!("ZTScenery::initAfterLoad {version:#x}"));
            true
        } else {
            unsafe { SCENERY_INIT_AFTER_LOAD_DETOUR.call(this, version) }
        }
    }

    #[detour(SCENERY_SAVE)]
    unsafe extern "thiscall" fn scenery_save(this: *const c_void, file: *const i8) -> bool {
        if scripting() {
            log("ZTScenery::save".to_string());
            BASE_LOADED.load(Ordering::SeqCst) != 0
        } else {
            unsafe { SCENERY_SAVE_DETOUR.call(this, file) }
        }
    }

    #[detour(SCENERY_LOAD)]
    unsafe extern "thiscall" fn scenery_load(this: *const c_void, file: *const u8, version: u32) -> bool {
        if scripting() {
            BASE_LOADED.load(Ordering::SeqCst) != 0
        } else {
            unsafe { SCENERY_LOAD_DETOUR.call(this, file, version) }
        }
    }
}

pub(super) fn install_stubs() -> Result<(), String> {
    install()?;
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { stubs::init_detours() } {
            result = Err(format!("could not install the lifecycle stubs: {e:?}"));
        }
    });
    result
}

/// Turns the scripted collaborators of this module and of `ztbuilding_users` on for as long as it lives.
pub(super) struct Scripted {
    rng: u32,
}

impl Scripted {
    pub(super) fn begin() -> Scripted {
        take_log();
        SCRIPTING.store(true, Ordering::SeqCst);
        STUBBING.store(true, Ordering::SeqCst);
        Scripted { rng: rng() }
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        SCRIPTING.store(false, Ordering::SeqCst);
        STUBBING.store(false, Ordering::SeqCst);
        pin_rng(self.rng);
        take_log();
    }
}

extern "thiscall" fn fake_sound_delete(this: u32, flag: u32) {
    log(format!("sound {} delete {}", word(this + 4), flag & 0xff));
}
extern "thiscall" fn fake_sound_is_playing(this: u32) -> u32 {
    log(format!("sound {} isPlaying", word(this + 4)));
    SOUND_PLAYING.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_sound_stop(this: u32) {
    log(format!("sound {} stop", word(this + 4)));
}
pub(super) extern "thiscall" fn fake_set_removed(this: u32, removed: u32, other: u32) {
    log(format!("setIsRemoved {this:#x} {} {}", removed & 0xff, other & 0xff));
}
extern "thiscall" fn fake_enter_slot(this: u32, tile: u32, slot: u32, x: i32, y: i32, z: i32) {
    log(format!("enterSlot {this:#x} {tile:#x} {slot} {x} {y} {z}"));
}
extern "thiscall" fn fake_restore_use(this: u32, _building: u32) {
    log(format!("restoreBuildingUse {this:#x}"));
}
extern "thiscall" fn fake_force_anim(this: u32, name: u32) {
    let (begin, end): (u32, u32) = (get_from_memory(name), get_from_memory(name + 4));
    let text: String = (begin..end).map(|address| get_from_memory::<u8>(address) as char).collect();
    log(format!("forceAnim {this:#x} {text:?}"));
}
extern "thiscall" fn fake_set_name(_this: u32, name: u32) {
    let (begin, end): (u32, u32) = (get_from_memory(name), get_from_memory(name + 4));
    let text: String = (begin..end).map(|address| get_from_memory::<u8>(address) as char).collect();
    log(format!("setName {text:?}"));
}
pub(super) extern "thiscall" fn fake_type_id(_this: u32) -> u32 {
    TYPE_ID.load(Ordering::SeqCst)
}

// ---- fixtures ------------------------------------------------------------------------------------------------------

/// Words of live memory (a type, a vtable pointer) overwritten for a scenario and put back when dropped.
#[derive(Default)]
pub(super) struct Patches(Vec<(u32, u32)>);

impl Patches {
    pub(super) fn set(&mut self, address: u32, value: u32) {
        self.0.push((address, word(address)));
        save_to_memory(address, value);
    }

    pub(super) fn set_byte(&mut self, address: u32, value: u8) {
        let aligned = address & !3;
        let mut bytes = word(aligned).to_le_bytes();
        bytes[(address - aligned) as usize] = value;
        self.set(aligned, u32::from_le_bytes(bytes));
    }

    /// Points the `{begin, end}` of the string at `address` to `text`.
    pub(super) fn set_string(&mut self, address: u32, text: &'static [u8]) {
        self.set(address, text.as_ptr() as u32);
        self.set(address + 4, text.as_ptr() as u32 + text.len() as u32);
    }
}

impl Drop for Patches {
    fn drop(&mut self) {
        for &(address, value) in self.0.iter().rev() {
            save_to_memory(address, value);
        }
    }
}

fn sound_vtable() -> u32 {
    let mut vtable = vec![0u32; 32];
    vtable[0] = fake_sound_delete as *const () as u32;
    vtable[0x50 / 4] = fake_sound_is_playing as *const () as u32;
    vtable[0x60 / 4] = fake_sound_stop as *const () as u32;
    Box::leak(vtable.into_boxed_slice()).as_ptr() as u32
}

/// Replaces the building's own sounds with scripted ones, numbered 1 (use) and 2 (ambient).
fn fake_sounds(building: ZTBuilding) {
    let vtable = sound_vtable();
    for (number, offset) in [(1u32, building_offsets::USE_SOUND), (2, building_offsets::AMBIENT_SOUND)] {
        let own = word(building.0 + offset);
        if own != 0 {
            let delete: extern "thiscall" fn(u32, u32) = unsafe { std::mem::transmute(word(word(own))) };
            delete(own, 1);
        }
        let sound = Box::leak(Box::new([vtable, number])).as_ptr() as u32;
        save_to_memory(building.0 + offset, sound);
    }
}

/// Takes the scripted sounds out again, so the real destructor does not see them.
fn drop_sounds(building: ZTBuilding) {
    save_to_memory(building.0 + building_offsets::USE_SOUND, 0u32);
    save_to_memory(building.0 + building_offsets::AMBIENT_SOUND, 0u32);
}

/// Everything a lifecycle method leaves in a building: the tail past the containers, the containers' contents, the
/// slots and the base's per-animation-list flags. The sound pointers are each building's own.
fn observe(building: ZTBuilding) -> Vec<(String, Vec<u32>)> {
    let tail = (building_offsets::CONSTRUCTION_DATE..BUILDING_SIZE)
        .step_by(4)
        .filter(|&offset| offset != building_offsets::USE_SOUND && offset != building_offsets::AMBIENT_SOUND)
        .map(|offset| word(building.0 + offset))
        .collect();
    let (flags_begin, flags_end) = (word(building.0 + 0xfc), word(building.0 + 0x100));
    let mut state = vec![
        ("tail".to_string(), tail),
        ("users".to_string(), vector(building.0 + building_offsets::USERS)),
        ("members".to_string(), vector(building.0 + building_offsets::MEMBERS)),
        ("flags".to_string(), (flags_begin..flags_end).map(|address| u32::from(get_from_memory::<u8>(address))).collect()),
        ("rng".to_string(), vec![rng()]),
    ];
    let slots = word(building.0 + building_offsets::SLOTS);
    for index in 0..building.slot_count().max(0) as u32 {
        let slot = slots + index * size_of::<ZTBuildingSlot>() as u32;
        let owner = word(slot + offset_of!(ZTBuildingSlot, building) as u32);
        let mut words = vec![
            word(slot),
            word(slot + 4),
            word(slot + 8),
            u32::from(get_from_memory::<u8>(slot + 0x0c)),
            u32::from(owner == building.0),
            word(slot + 0x14),
        ];
        words.extend(vector(slot + 0x18));
        state.push((format!("slot {index}"), words));
    }
    state
}

pub(super) fn check<T: PartialEq + std::fmt::Debug>(failures: &mut Vec<String>, label: &str, real: T, port: T) {
    if real != port && failures.len() < 40 {
        failures.push(format!("{label}: real {real:x?}, port {port:x?}"));
    }
}

fn check_state(failures: &mut Vec<String>, label: &str, real: ZTBuilding, port: ZTBuilding) {
    let (real, port) = (observe(real), observe(port));
    check(failures, &format!("{label} parts"), real.len(), port.len());
    for ((name, real), (_, port)) in real.into_iter().zip(port) {
        check(failures, &format!("{label} {name}"), real, port);
    }
}

/// A fixture with its slots built.
pub(super) fn ready_building(building_type: u32) -> ZTBuilding {
    let building = build_standalone_building(building_type);
    lifecycle_real::init_status_vars(building.0);
    building
}

/// Appends `{id, slot}` pairs and member ids through the real loader, so the buffers are vanilla's.
fn fill(building: ZTBuilding, users: &[(u32, u32)], members: &[u32]) {
    for &(id, slot) in users {
        crate::vanilla_vector::push_pair(building.0 + building_offsets::USERS, id, slot, |begin, bytes| {
            if begin != 0 {
                unsafe { openzt_detour::generated::poolalloc::DEALLOCATE.original()(begin as *const u32, bytes) }
            }
        });
    }
    for &id in members {
        let mut unit = [0u32; 0x4a];
        unit[(ENTITY_ID / 4) as usize] = id;
        building.add_member(unit.as_ptr() as u32);
    }
}

fn clear_users(building: ZTBuilding) {
    save_to_memory(building.0 + building_offsets::USERS + 4, word(building.0 + building_offsets::USERS));
}

fn allocate_building() -> ZTBuilding {
    let memory = unsafe { OPERATOR_NEW.original()(BUILDING_SIZE) } as u32;
    unsafe { std::ptr::write_bytes(memory as *mut u8, 0, BUILDING_SIZE as usize) };
    ZTBuilding(memory)
}

// ---- tests ---------------------------------------------------------------------------------------------------------

/// `ZTBUILDING_CTOR_LIVE`: the constructor over every building type, with and without sound names, and the destructor
/// over a filled building with playing and silent sounds.
pub(crate) fn run_ztbuilding_ctor_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_CTOR_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = building_types();
    if types.is_empty() {
        failures.push("no building type".to_string());
    }
    let (mut sounded, mut stopped) = (0, 0);
    for &building_type in &types {
        for names in [None, Some((&b""[..], &b""[..])), Some((&b"a.wav"[..], &b"b.wav"[..])), Some((&b""[..], &b"b.wav"[..]))] {
            let mut patches = Patches::default();
            if let Some((use_name, ambient_name)) = names {
                patches.set_string(building_type + 0x1dc, use_name);
                patches.set_string(building_type + 0x1f0, ambient_name);
            }
            let label = format!("type {building_type:#x} sounds {names:?}");
            let (real, port) = (allocate_building(), allocate_building());
            lifecycle_real::construct(real.0, building_type);
            port.construct(building_type);
            check(&mut failures, &format!("{label} vtable"), word(real.0), word(port.0));
            check(&mut failures, &format!("{label} type"), word(real.0 + ENTITY_TYPE), word(port.0 + ENTITY_TYPE));
            let containers = |building: ZTBuilding| read_words(building.0 + building_offsets::MEMBERS, 0x24);
            check(&mut failures, &format!("{label} containers"), containers(real), containers(port));
            check_state(&mut failures, &label, real, port);
            for offset in [building_offsets::USE_SOUND, building_offsets::AMBIENT_SOUND] {
                let sound = |building: ZTBuilding| {
                    let sound = word(building.0 + offset);
                    if sound == 0 { None } else { Some((word(sound), word(sound + 4))) }
                };
                sounded += u32::from(sound(port).is_some());
                check(&mut failures, &format!("{label} sound at {offset:#x}"), sound(real), sound(port));
            }

            for (playing, loops) in [(1u32, 1u8), (0, 1), (1, 0)] {
                let (real, port) = if (playing, loops) == (1, 1) {
                    (real, port)
                } else {
                    let (real, port) = (allocate_building(), allocate_building());
                    lifecycle_real::construct(real.0, building_type);
                    lifecycle_real::construct(port.0, building_type);
                    (real, port)
                };
                patches.set_byte(building_type + 0x1e8, loops);
                for building in [real, port] {
                    lifecycle_real::init_status_vars(building.0);
                    fill(building, &[(1, 0), (2, 0), (3, 1)], &(100..140).collect::<Vec<_>>());
                    fake_sounds(building);
                }
                SOUND_PLAYING.store(playing, Ordering::SeqCst);
                take_log();
                lifecycle_real::destruct(real.0);
                let real_log = take_log();
                port.destruct();
                let port_log = take_log();
                stopped += port_log.iter().filter(|entry| entry.ends_with("stop")).count();
                check(&mut failures, &format!("{label} destructor playing {playing} loops {loops}"), real_log, port_log);
                for building in [real, port] {
                    unsafe { OPERATOR_DELETE.original()(building.0) };
                }
            }
        }
    }
    if sounded == 0 || stopped == 0 {
        failures.push(format!("{sounded} sounds made and {stopped} stopped"));
    }
    SOUND_PLAYING.store(1, Ordering::SeqCst);
    finish_test(test_name, failures, failure_log)
}

/// `ZTBUILDING_STATUS_LIVE`: `initStatusVars` on fresh and used fixtures of every type, and `resetInfo` over buildings
/// with users and members.
pub(crate) fn run_ztbuilding_status_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_STATUS_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    let guests = Guests::new(guest_type);
    let saved_rng = rng();
    let mut slots_seen = 0;
    for building_type in building_types() {
        let (real, port) = (build_standalone_building(building_type), build_standalone_building(building_type));
        for pass in 0..3 {
            let label = format!("type {building_type:#x} pass {pass}");
            pin_rng(0x1234 + pass);
            lifecycle_real::init_status_vars(real.0);
            let real_rng = rng();
            pin_rng(0x1234 + pass);
            port.init_status_vars();
            check(&mut failures, &format!("{label} rng after"), real_rng, rng());
            check_state(&mut failures, &label, real, port);
            slots_seen += port.slot_count();

            // Dirty what the next pass must reset.
            for building in [real, port] {
                for offset in (building_offsets::OCCUPANCY..=building_offsets::TOTAL_UPKEEP).step_by(4) {
                    save_to_memory(building.0 + offset, 0x4000_0000 + offset);
                }
                fill(building, &[], &[5, 6, 7]);
                let slots = word(building.0 + building_offsets::SLOTS);
                for index in 0..building.slot_count() as u32 {
                    save_to_memory(slots + index * 0x24 + 4, 3u32);
                    save_to_memory(slots + index * 0x24 + 0x14, 0x55u32);
                }
            }
        }

        // `resetInfo` alone: users that are fixture guests, a missing one, and members.
        let ids: Vec<u32> = guests.guests.iter().map(|&guest| word(guest + ENTITY_ID)).collect();
        for building in [real, port] {
            fill(building, &[(ids[0], 0), (MISSING_ID, 0), (ids[1], 0), (ids[2], 0)], &[8, 9]);
        }
        let before: Vec<Vec<u32>> = guests.guests.iter().map(|&guest| read_words(guest, GUEST_SIZE)).collect();
        let restore = || {
            for (&guest, words) in guests.guests.iter().zip(&before) {
                for (index, &value) in words.iter().enumerate() {
                    save_to_memory(guest + index as u32 * 4, value);
                }
            }
        };
        let after = || -> Vec<Vec<u32>> { guests.guests.iter().map(|&guest| read_words(guest, GUEST_SIZE)).collect() };
        pin_rng(99);
        lifecycle_real::reset_info(real.0);
        let (real_guests, real_rng) = (after(), rng());
        restore();
        pin_rng(99);
        port.reset_info();
        let label = format!("type {building_type:#x} resetInfo");
        check(&mut failures, &format!("{label} guests"), real_guests, after());
        check(&mut failures, &format!("{label} rng after"), real_rng, rng());
        check_state(&mut failures, &label, real, port);
        restore();
        destroy_standalone_building(real);
        destroy_standalone_building(port);
    }
    pin_rng(saved_rng);
    guests.destroy();
    if slots_seen == 0 {
        failures.push("no type with slots".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// A `ZTBuilding` stream of `version` without its base, with `occupancy` stored and three users.
fn stream(version: u32, occupancy: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut put = |words: &[u32]| words.iter().for_each(|word| bytes.extend(word.to_le_bytes()));
    if version >= 0x1c {
        put(&[occupancy, 70, 2.5f32.to_bits(), 3.5f32.to_bits(), 3, 101, 0, 102, 1, 103, 0]);
    }
    if version >= 0x20 {
        put(&[9]);
    }
    if version >= 0x24 {
        put(&[1.5f32.to_bits(), 4.5f32.to_bits(), 12, 13, 0xaaaa_0000, 0x01bb_0000]);
    }
    if (0x2d..0x43).contains(&version) {
        if version >= 0x30 {
            put(&[6]);
            bytes.extend([11u8, 12, 13, 14, 15, 16]);
        } else {
            bytes.extend([21u8, 22]);
        }
    }
    let mut put = |words: &[u32]| words.iter().for_each(|word| bytes.extend(word.to_le_bytes()));
    if version >= 0x44 {
        put(&[8.25f32.to_bits()]);
    }
    if version >= 0x4a {
        put(&[2]);
    }
    if version >= 0x4b {
        put(&[19]);
        put(&(200..219).collect::<Vec<_>>());
    }
    bytes
}

/// `ZTBUILDING_SAVE_LOAD_LIVE`: `save` of every live building and of filled fixtures, byte for byte; a round trip of
/// those bytes through both loaders; and `load` over every version gate and over truncated streams.
pub(crate) fn run_ztbuilding_save_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_SAVE_LOAD_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let mut file = [0u32; 8];
    let file = file.as_mut_ptr() as u32;
    let save_both = |building: ZTBuilding, failures: &mut Vec<String>, label: &str| -> Vec<u8> {
        io_redirect::begin_capture();
        let real = lifecycle_real::save(building.0, file);
        let real_bytes = io_redirect::end_capture();
        io_redirect::begin_capture();
        let port = building.save(file);
        let port_bytes = io_redirect::end_capture();
        check(failures, &format!("{label} save result"), real, port);
        check(failures, &format!("{label} save bytes"), &real_bytes, &port_bytes);
        port_bytes
    };

    let live = buildings_with_slots();
    if live.is_empty() {
        failures.push("no live building with slots".to_string());
    }
    let mut round_trips = 0;
    for &building in &live {
        save_both(building, &mut failures, &format!("building {:#x}", building.0));

        let building_type = building.entity_type();
        let source = ready_building(building_type);
        for (index, offset) in (building_offsets::CONSTRUCTION_DATE..=building_offsets::TOTAL_UPKEEP).step_by(4).enumerate() {
            save_to_memory(source.0 + offset, 0x100 + index as u32);
        }
        fill(source, &[(301, 0), (302, 0), (303, 0)], &(400..437).collect::<Vec<_>>());
        let bytes = save_both(source, &mut failures, &format!("type {building_type:#x} fixture"));

        let (real, port) = (ready_building(building_type), ready_building(building_type));
        io_redirect::begin_replay(bytes.clone());
        let real_result = lifecycle_real::load(real.0, file, LATEST_VERSION);
        let real_position = io_redirect::replay_position();
        io_redirect::end_replay();
        io_redirect::begin_replay(bytes.clone());
        let port_result = port.load(file, LATEST_VERSION);
        let port_position = io_redirect::replay_position();
        io_redirect::end_replay();
        let label = format!("type {building_type:#x} round trip");
        check(&mut failures, &format!("{label} result"), real_result, port_result);
        check(&mut failures, &format!("{label} bytes read"), real_position, port_position);
        check_state(&mut failures, &label, real, port);
        round_trips += u32::from(port_result && port_position == bytes.len());
        for building in [source, real, port] {
            destroy_standalone_building(building);
        }
    }
    if round_trips == 0 {
        failures.push("no round trip read its whole stream".to_string());
    }

    if let Some(building_type) = live.first().map(|building| building.entity_type()) {
        let scripted = Scripted::begin();
        for version in [0, 0x1b, 0x1c, 0x1f, 0x20, 0x23, 0x24, 0x2c, 0x2d, 0x2f, 0x30, 0x42, 0x43, 0x44, 0x49, 0x4a, 0x4b, LATEST_VERSION] {
            for occupancy in [1u32, 10] {
                let full = stream(version, occupancy);
                let lengths = [full.len(), 0, 3, 18, 22, 30, full.len() / 2, full.len().saturating_sub(1)];
                for (case, &length) in lengths.iter().enumerate() {
                    let length = length.min(full.len());
                    let base_loads = u32::from(case != 1);
                    BASE_LOADED.store(base_loads, Ordering::SeqCst);
                    let (real, port) = (ready_building(building_type), ready_building(building_type));
                    for building in [real, port] {
                        for offset in (building_offsets::UPGRADE_INDEX..=building_offsets::TOTAL_UPKEEP).step_by(4) {
                            save_to_memory(building.0 + offset, 0u32);
                        }
                        save_to_memory(building.0 + building_offsets::LAST_MONTH_USERS, 0x77u32);
                        fill(building, &[(99, 0)], &[98]);
                        let slots = word(building.0 + building_offsets::SLOTS);
                        save_to_memory(slots + 4, 5u32);
                        save_to_memory(slots + 0x14, 6u32);
                    }
                    io_redirect::begin_replay(full[..length].to_vec());
                    let real_result = lifecycle_real::load(real.0, file, version);
                    let real_position = io_redirect::replay_position();
                    io_redirect::end_replay();
                    io_redirect::begin_replay(full[..length].to_vec());
                    let port_result = port.load(file, version);
                    let port_position = io_redirect::replay_position();
                    io_redirect::end_replay();
                    let label = format!("version {version:#x} occupancy {occupancy} length {length} base {base_loads}");
                    check(&mut failures, &format!("{label} result"), real_result, port_result);
                    check(&mut failures, &format!("{label} bytes read"), real_position, port_position);
                    // A stream cut before the user count leaves vanilla comparing the occupancy with stack garbage.
                    if base_loads != 0 && version >= 0x1c && length < 20 {
                        for building in [real, port] {
                            save_to_memory(building.0 + building_offsets::OCCUPANCY, 0u32);
                        }
                    }
                    check_state(&mut failures, &label, real, port);
                    destroy_standalone_building(real);
                    destroy_standalone_building(port);
                }
            }
        }
        BASE_LOADED.store(1, Ordering::SeqCst);
        drop(scripted);
    }
    finish_test(test_name, failures, failure_log)
}

/// Runs `real` and `port` with the same RNG state and compares what they answered and logged.
fn compare_calls<T: PartialEq + std::fmt::Debug>(failures: &mut Vec<String>, label: &str, real: impl FnOnce() -> T, port: impl FnOnce() -> T) -> Vec<String> {
    take_log();
    pin_rng(0x5eed);
    let real_answer = real();
    let (real_log, real_rng) = (take_log(), rng());
    pin_rng(0x5eed);
    let port_answer = port();
    let port_log = take_log();
    check(failures, &format!("{label} answer"), real_answer, port_answer);
    check(failures, &format!("{label} calls"), &real_log, &port_log);
    check(failures, &format!("{label} rng after"), real_rng, rng());
    port_log
}

/// [`compare_calls`] over one building, set up afresh for each side, for methods whose calls name the building. Also
/// compares what they leave in the building and in whatever `extra` reads.
fn compare_on_one<T: PartialEq + std::fmt::Debug>(
    failures: &mut Vec<String>,
    label: &str,
    building: ZTBuilding,
    setup: impl Fn(),
    extra: impl Fn() -> Vec<Vec<u32>>,
    real: impl FnOnce() -> T,
    port: impl FnOnce() -> T,
) -> Vec<String> {
    setup();
    take_log();
    pin_rng(0x5eed);
    let real_answer = real();
    let (real_log, real_state, real_extra) = (take_log(), observe(building), extra());
    setup();
    take_log();
    pin_rng(0x5eed);
    let port_answer = port();
    let port_log = take_log();
    check(failures, &format!("{label} answer"), real_answer, port_answer);
    check(failures, &format!("{label} calls"), &real_log, &port_log);
    for ((name, real), (_, port)) in real_state.into_iter().zip(observe(building)) {
        check(failures, &format!("{label} {name}"), real, port);
    }
    check(failures, &format!("{label} other memory"), real_extra, extra());
    port_log
}

/// `ZTBUILDING_MAP_LIVE`: `addToMap` over each answer of its callees, and `removeFromMap` with members, sounds and a
/// trash can's contents, in the middle and at the corner of the map.
pub(crate) fn run_ztbuilding_map_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_MAP_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let (Some(guest_type), Some(building_type)) = (find_guest_type(), building_types().first().copied()) else {
        return finish_test(test_name, vec!["no guest or building type".to_string()], failure_log);
    };
    let guests = Guests::new(guest_type);
    let mut patches = Patches::default();
    for &guest in &guests.guests {
        // `Guests::destroy` puts the guests' own vtables back.
        let vtable = scripted_vtable(guest, 220, &[(0xa8, fake_set_removed as *const () as u32), (0xcc, fake_set_visible as *const () as u32)]);
        save_to_memory(guest, vtable);
    }
    let own_type_id = {
        let function: extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(word(word(building_type) + 0x20)) };
        function(building_type)
    };
    patches.set(building_type, scripted_vtable(building_type, 40, &[(0x20, fake_type_id as *const () as u32)]));
    let (width, height) = (globals().ztworldmgr().map_x_size as i32, globals().ztworldmgr().map_y_size as i32);
    let scripted = Scripted::begin();

    let (real, port) = (ready_building(building_type), ready_building(building_type));
    for base_added in [0u32, 1, 0x100, 0xab01] {
        for lists_added in [0u32, 1] {
            for tricks in [0u8, 1] {
                BASE_ADDED.store(base_added, Ordering::SeqCst);
                LISTS_ADDED.store(lists_added, Ordering::SeqCst);
                patches.set_byte(building_type + 0x6f, tricks);
                let label = format!("addToMap base {base_added:#x} lists {lists_added} tricks {tricks}");
                compare_calls(&mut failures, &label, || lifecycle_real::add_to_map(real.0), || port.add_to_map());
            }
        }
    }

    let own_vtable = word(real.0);
    save_to_memory(real.0, scripted_vtable(real.0, 78, &[(0, fake_send_event as *const () as u32)]));
    // The real sounds go; each run gets scripted ones.
    fake_sounds(real);
    drop_sounds(real);
    let ids: Vec<u32> = guests.guests.iter().map(|&guest| word(guest + ENTITY_ID)).collect();
    let middle = [width / 2 * 64 + 32, height / 2 * 64 + 32, 0];
    let corner = [5, 7, -5];
    let far = [width * 64 - 1, height * 64 - 1, -32];
    // `(keep, tracks users, trash can, trash, scenery created, sounds loop, sounds playing, position)`.
    let scenarios = [
        (false, 0u8, false, 0, 1u32, 1u8, 1u32, middle),
        (false, 1, false, 4, 1, 0, 1, middle),
        (true, 0, true, 4, 1, 1, 1, middle),
        (false, 0, true, 0, 1, 1, 0, middle),
        (false, 0, true, 3, 1, 1, 1, middle),
        (false, 1, true, 30, 1, 0, 0, middle),
        (false, 0, true, 5, 0, 1, 1, middle),
        (false, 0, true, 12, 1, 1, 1, corner),
        (false, 0, true, 12, 1, 1, 1, far),
    ];
    let (mut scattered, mut removed) = (0, 0);
    for (keep, tracks, trash_can, trash, created, loops, playing, position) in scenarios {
        patches.set_byte(building_type + offset_of!(ZTBuildingType, user_tracker) as u32, tracks);
        patches.set_byte(building_type + 0x1e8, loops);
        TYPE_ID.store(if trash_can { TRASH_CAN_TYPE_ID } else { own_type_id }, Ordering::SeqCst);
        SCENERY_CREATED.store(created, Ordering::SeqCst);
        SOUND_PLAYING.store(playing, Ordering::SeqCst);
        let setup = || {
            for (axis, value) in position.into_iter().enumerate() {
                save_to_memory(real.0 + 0x114 + axis as u32 * 4, value);
            }
            save_to_memory(real.0 + building_offsets::MEMBERS + 4, word(real.0 + building_offsets::MEMBERS));
            fill(real, &[], &[ids[0], MISSING_ID, ids[1]]);
            clear_users(real);
            real.set_type_state(trash);
            drop_sounds(real);
            fake_sounds(real);
        };
        let label = format!("removeFromMap keep {keep} tracks {tracks} trash {trash_can}/{trash} created {created} at {position:?}");
        let calls = compare_on_one(
            &mut failures,
            &label,
            real,
            setup,
            Vec::new,
            || lifecycle_real::remove_from_map(real.0, keep),
            || real.remove_from_map(keep),
        );
        scattered += calls.iter().filter(|entry| entry.starts_with("createScenery")).count();
        removed += calls.iter().filter(|entry| entry.starts_with("setIsRemoved")).count();
    }
    save_to_memory(real.0, own_vtable);
    drop_sounds(real);
    if scattered == 0 || removed == 0 {
        failures.push(format!("removeFromMap scattered {scattered} pieces and removed {removed} members"));
    }
    SCENERY_CREATED.store(1, Ordering::SeqCst);
    SOUND_PLAYING.store(1, Ordering::SeqCst);
    BASE_ADDED.store(1, Ordering::SeqCst);
    LISTS_ADDED.store(1, Ordering::SeqCst);
    drop(scripted);
    destroy_standalone_building(real);
    destroy_standalone_building(port);
    drop(patches);
    guests.destroy();
    finish_test(test_name, failures, failure_log)
}

/// `ZTBUILDING_AFTER_LOAD_LIVE`: `initAfterLoad` over loaded users, one of them gone, for each kind of building, before
/// and after the version that keeps users in their slots.
pub(crate) fn run_ztbuilding_after_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_AFTER_LOAD_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    let types: Vec<u32> = buildings_with_slots().iter().map(|building| building.entity_type()).take(4).collect();
    if types.is_empty() {
        return finish_test(test_name, vec!["no live building with slots".to_string()], failure_log);
    }
    let guests = Guests::new(guest_type);
    for &guest in &guests.guests {
        let vtable = scripted_vtable(
            guest,
            220,
            &[
                (0x104, fake_force_anim as *const () as u32),
                (0x284, fake_enter_slot as *const () as u32),
                (0x2b4, fake_restore_use as *const () as u32),
            ],
        );
        save_to_memory(guest, vtable);
    }
    let ids: Vec<u32> = guests.guests.iter().map(|&guest| word(guest + ENTITY_ID)).collect();
    let (mut restored, mut drawn) = (0, 0);
    for &building_type in &types {
        for (hide, outside, version, animation, goal, trash) in [
            (0u8, 0u8, 0x4fu32, &b""[..], 0x68u32, 0),
            (0, 0, 0x50, &b""[..], 0x68, 13),
            (0, 0, LATEST_VERSION, &b"use"[..], 0, 12),
            (1, 0, 0x4f, &b"use"[..], 0, 13),
            (1, 0, LATEST_VERSION, &b""[..], 0x6a, 0),
            (0, 1, 0x4f, &b""[..], 0x69, 0),
        ] {
            let mut patches = Patches::default();
            patches.set_byte(building_type + offset_of!(ZTBuildingType, hide_user) as u32, hide);
            patches.set_byte(building_type + offset_of!(ZTBuildingType, user_stays_outside) as u32, outside);
            patches.set_string(building_type + 0x1d0, animation);
            let own_type_id = {
                let function: extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(word(word(building_type) + 0x20)) };
                function(building_type)
            };
            TYPE_ID.store(if trash > 0 { TRASH_CAN_TYPE_ID } else { own_type_id }, Ordering::SeqCst);
            patches.set(building_type, scripted_vtable(building_type, 40, &[(0x20, fake_type_id as *const () as u32)]));
            WATER.store(17, Ordering::SeqCst);

            let building = ready_building(building_type);
            let last_slot = (building.slot_count() - 1).max(0) as u32;
            let users = [(ids[0], 0), (MISSING_ID, 0), (ids[1], last_slot), (ids[2], 0)];
            let own_vtable = word(building.0);
            save_to_memory(building.0, scripted_vtable(building.0, 78, &[(0, fake_send_event as *const () as u32)]));
            let before: Vec<Vec<u32>> = guests.guests.iter().map(|&guest| read_words(guest, GUEST_SIZE)).collect();
            let set_guests = |words: &[Vec<u32>]| {
                for (&guest, words) in guests.guests.iter().zip(words) {
                    for (index, &value) in words.iter().enumerate() {
                        save_to_memory(guest + index as u32 * 4, value);
                    }
                }
            };
            for &guest in &guests.guests {
                save_to_memory(guest + 0x170, goal);
            }
            let prepared: Vec<Vec<u32>> = guests.guests.iter().map(|&guest| read_words(guest, GUEST_SIZE)).collect();
            let setup = || {
                set_guests(&prepared);
                clear_users(building);
                fill(building, &users, &[]);
                save_to_memory(building.0 + building_offsets::OCCUPANCY, 4u32);
                save_to_memory(building.0 + building_offsets::TOTAL_USERS, 10u32);
                save_to_memory(building.0 + building_offsets::LAST_MONTH_USERS, 2u32);
                save_to_memory(building.0 + building_offsets::THIS_MONTH_USERS, 5u32);
                building.set_type_state(trash);
                let slots = word(building.0 + building_offsets::SLOTS);
                for index in 0..building.slot_count() as u32 {
                    let slot = slots + index * size_of::<ZTBuildingSlot>() as u32;
                    save_to_memory(slot + 4, 0u32);
                    save_to_memory(slot + 0x14, 0u32);
                    ZTSlot(slot).clear_draw_users();
                }
            };
            let scripted = Scripted::begin();
            let label = format!("type {building_type:#x} hide {hide} outside {outside} version {version:#x} goal {goal:#x} trash {trash}");
            let calls = compare_on_one(
                &mut failures,
                &label,
                building,
                setup,
                || guests.guests.iter().map(|&guest| read_words(guest, GUEST_SIZE)).collect(),
                || lifecycle_real::init_after_load(building.0, version),
                || building.init_after_load(version),
            );
            drop(scripted);
            restored += calls.iter().filter(|entry| entry.starts_with("enterSlot") || entry.starts_with("restoreBuildingUse")).count();
            drawn += calls.iter().filter(|entry| entry.starts_with("beforePlace")).count();
            set_guests(&before);
            save_to_memory(building.0, own_vtable);
            clear_users(building);
            destroy_standalone_building(building);
        }
    }
    guests.destroy();
    if restored == 0 {
        failures.push("no user was restored through its unit".to_string());
    }
    tracing::info!("{test_name}: {restored} units restored, {drawn} placed in draw slots");
    finish_test(test_name, failures, failure_log)
}

static WORLD_UNIT_TYPE: AtomicU32 = AtomicU32::new(0);
static BORN: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_get_type(_this: u32, id: u32) -> u32 {
    log(format!("getType {id:#x}"));
    if id == 0xbad {
        0
    } else {
        WORLD_UNIT_TYPE.load(Ordering::SeqCst)
    }
}
extern "thiscall" fn fake_create_entity(_this: u32, argument: u32) -> u32 {
    log(format!("createEntity {argument}"));
    BORN.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_add_entity(_this: u32, entity: u32) {
    log(format!("addEntity {entity:#x}"));
}

fn recorded_add_user(_building: u32, unit: u32, slot: u32) -> Option<bool> {
    if !scripting() {
        return None;
    }
    log(format!("addUser {unit:#x} {slot}"));
    Some(true)
}

/// `ZTBUILDING_BIRTH_NAME_LIVE`: `drop` over slot records with and without a birth type, and `createName` for numbered,
/// plain and unnamed types.
pub(crate) fn run_ztbuilding_birth_name_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_BIRTH_NAME_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    let types: Vec<u32> = buildings_with_slots().iter().map(|building| building.entity_type()).collect();
    if types.is_empty() {
        return finish_test(test_name, vec!["no live building with slots".to_string()], failure_log);
    }
    let guests = Guests::new(guest_type);
    let guest = guests.guests[0];
    save_to_memory(guest, scripted_vtable(guest, 220, &[(0x284, fake_enter_slot as *const () as u32)]));
    let world = globals().ztworldmgr_ptr() as u32;
    let mut world_patches = Patches::default();
    world_patches.set(world, scripted_vtable(world, 64, &[(0xc, fake_get_type as *const () as u32), (0x18, fake_add_entity as *const () as u32)]));
    let mut unit_type_vtable = vec![0u32; 16];
    unit_type_vtable[0x24 / 4] = fake_create_entity as *const () as u32;
    let unit_type = [unit_type_vtable.as_ptr() as u32];
    WORLD_UNIT_TYPE.store(unit_type.as_ptr() as u32, Ordering::SeqCst);
    *intercept::ADD_USER.lock().unwrap() = Some(recorded_add_user);

    let mut born = 0;
    for &building_type in types.iter().take(4) {
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        let record_size = size_of::<ZTBuildingSlotType>() as u32;
        let record_count = (word(records + 4) - word(records)) / record_size;
        let not_a_unit = ready_building(building_type);
        // `0xbad` is a type the world does not know, where vanilla faults: only the port runs it.
        for (birth, entity) in [(0u32, guest), (0x2329, guest), (0xbad, guest), (0x2329, 0), (0x2329, not_a_unit.0)] {
            let mut patches = Patches::default();
            for index in 0..record_count {
                // Every other record gives birth, so both kinds are seen where there are several.
                let id = if index % 2 == 0 { birth } else { 0 };
                patches.set(word(records) + index * record_size + offset_of!(ZTBuildingSlotType, birth_user_type_id) as u32, id);
            }
            BORN.store(entity, Ordering::SeqCst);
            let (real, port) = (ready_building(building_type), ready_building(building_type));
            for building in [real, port] {
                save_to_memory(building.0 + 0x114, 10 * 64 + 5);
                save_to_memory(building.0 + 0x118, 11 * 64 + 9);
                save_to_memory(building.0 + 0x11c, -3i32);
            }
            let scripted = Scripted::begin();
            let label = format!("drop type {building_type:#x} birth {birth:#x} entity {entity:#x}");
            let calls = if birth == 0xbad {
                take_log();
                port.drop_residents();
                let calls = take_log();
                check(&mut failures, &format!("{label} nobody born"), true, calls.iter().all(|entry| entry.starts_with("getType")));
                calls
            } else {
                compare_calls(&mut failures, &label, || lifecycle_real::drop_residents(real.0), || port.drop_residents())
            };
            drop(scripted);
            born += calls.iter().filter(|entry| entry.starts_with("addUser")).count();
            check_state(&mut failures, &label, real, port);
            destroy_standalone_building(real);
            destroy_standalone_building(port);
        }
        destroy_standalone_building(not_a_unit);
    }
    *intercept::ADD_USER.lock().unwrap() = None;
    drop(world_patches);
    guests.destroy();
    ENTITIES.lock().unwrap().clear();
    if born == 0 {
        failures.push("no unit was born".to_string());
    }

    let (mut named, mut numbered, mut unnamed) = (0, 0, 0);
    for building_type in building_types() {
        let own_type_id = {
            let function: extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(word(word(building_type) + 0x20)) };
            function(building_type)
        };
        let building = build_standalone_building(building_type);
        let own_vtable = word(building.0);
        save_to_memory(building.0, scripted_vtable(building.0, 78, &[(0x1c, fake_set_name as *const () as u32)]));
        let mut patches = Patches::default();
        patches.set(building_type, scripted_vtable(building_type, 40, &[(0x20, fake_type_id as *const () as u32)]));
        let scripted = Scripted::begin();
        for id in [own_type_id, 0xfffe] {
            for number in [0u8, 1] {
                TYPE_ID.store(id, Ordering::SeqCst);
                patches.set_byte(building_type + 0x5b, number);
                let label = format!("createName type {building_type:#x} id {id:#x} numbered {number}");
                let calls = compare_calls(&mut failures, &label, || lifecycle_real::create_name(building.0), || building.create_name());
                named += calls.iter().filter(|entry| entry.starts_with("setName")).count();
                numbered += calls.iter().filter(|entry| entry.starts_with("getUnitCount")).count();
                unnamed += calls.iter().filter(|entry| entry.starts_with("BFEntity")).count();
            }
        }
        drop(scripted);
        drop(patches);
        save_to_memory(building.0, own_vtable);
        destroy_standalone_building(building);
    }
    if named == 0 || numbered == 0 || unnamed == 0 {
        failures.push(format!("createName: {named} named, {numbered} numbered, {unnamed} left to BFEntity"));
    }
    finish_test(test_name, failures, failure_log)
}
