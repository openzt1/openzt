//! `ZTBuilding`'s lifecycle and persistence: construction and destruction, `initStatusVars` and `resetInfo`, joining and
//! leaving the map, saving and loading, the name, and the units a building is born with.
//!
//! The `ZTScenery` base is still vanilla's. The
//! containers and the two `SNDSound` objects come from, and go back to, vanilla's allocators.
//!
//! Where vanilla faults or writes out of bounds, the port does this instead:
//! - a building without a type: no sounds are looped, there is no name, nobody is born and no slots are built;
//! - `initAfterLoad` for a saved user whose slot index is past the slots: the user is not counted into any slot;
//! - `initAfterLoad` for a saved user that is not a unit: no unit method is called on it;
//! - `drop` for a birth type the world does not know: nobody is born;
//! - `load` when the user count cannot be read: the occupancy is left alone (vanilla compares it with stack garbage).

use std::{
    ffi::c_void,
    mem::{offset_of, size_of},
};

use glam::IVec3;
use openzt_detour::{
    generated::{
        bfapp::LOAD_STRING,
        bfentity::{CREATE_NAME as BFENTITY_CREATE_NAME, GET_TILE as BFENTITY_GET_TILE, INIT_STATUS_VARS as BFENTITY_INIT_STATUS_VARS},
        bfentitytype::GET_UNIT_COUNT,
        bfinternat::GET_NUMBER_TEXT,
        msvc_std_vector_t_4::VECTOR_T_4 as VECTOR_T_4_DESTRUCTOR,
        msvc_std_vector_ztbuilding_slot::INSERT_N as SLOT_VECTOR_INSERT,
        msvc_std_vectoruchar::INSERT_N as BYTE_VECTOR_INSERT,
        msvc_std_vectoruint::INSERT_N as WORD_VECTOR_INSERT,
        poolalloc::{DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4},
        standalone::{OPERATOR_DELETE, OPERATOR_NEW},
        ztbuilding::{
            ADD_TO_MAP, CONSTRUCTOR, CREATE_NAME, DESTRUCTOR_0, DESTRUCTOR_1, DROP, INIT_AFTER_LOAD as BUILDING_INIT_AFTER_LOAD,
            INIT_STATUS_VARS, LOAD, REMOVE_FROM_MAP, RESET_INFO, SAVE,
        },
        ztbuilding_slot::SLOT_1 as SLOT_DESTRUCTOR,
        ztscenery::{
            ADD_TO_MAP as ZTSCENERY_ADD_TO_MAP, CONSTRUCTOR as ZTSCENERY_CONSTRUCTOR, DESTRUCTOR_0 as ZTSCENERY_DESTRUCTOR,
            INIT_AFTER_LOAD as SCENERY_INIT_AFTER_LOAD, LOAD as ZTSCENERY_LOAD, REMOVE_FROM_MAP as ZTSCENERY_REMOVE_FROM_MAP, SAVE as ZTSCENERY_SAVE,
        },
        ztui_showpanel::UPDATE_AVAILABLE_TRICK_LIST,
        ztunittype::GET_TRASH_TYPE,
        ztworldmgr::{ADD_TO_ALL_BUILDING_LISTS, CREATE_SCENERY, REMOVE_FROM_ALL_BUILDING_LISTS},
    },
};
use openzt_detour_macro::detour_mod;

use super::{
    building_offsets,
    users::{call0, call1, cast, virtual_method, word, world_entity, ZTSlot, ANIMAL_TAG_RVA, BUILDING_USE_GOALS, GUEST_TAG_RVA, UNIT_TAG_RVA},
    ZTBuilding, ZTBuildingSlot,
};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, save_to_memory},
    vanilla_string::{c_string_bytes, object_bytes, VanillaString},
    vanilla_vector::{erase_element, push_pair, push_word},
    ztkeeper::{read_field, write_field},
    ztunit::{map_tile, ZTUnit},
};


/// RVAs: the `ZTBuilding` and `SNDSound` vtables and the separator between a building's name and its number.
const BUILDING_VTABLE_RVA: u32 = 0x0022_e0b0;
const SOUND_VTABLE_RVA: u32 = 0x0023_0bc0;
const NAME_SEPARATOR_RVA: u32 = 0x0023_9110;
/// The app object `loadString` is asked on, the word guarding its lazy set-up and the value set-up stores there.
const APP: u32 = 0x0063_8148;
const APP_GUARD: u32 = 0x0063_8154;
const APP_GUARD_INIT: u32 = 0x0044_1880;
const NAME_BUFFER: usize = 0x208;

/// `BFEntityType` / `ZTBuildingType` fields not in the structs.
const TYPE_ANIM_LISTS: u32 = 0x28;
const TYPE_NUMBERED_NAME: u32 = 0x5b;
const TYPE_SHOW_TRICKS: u32 = 0x6f;
const TYPE_NAME_KEY: u32 = 0xa4;
/// The name of the animation a unit inside is put into.
const TYPE_USE_ANIMATION: u32 = 0x1d0;
const TYPE_USE_SOUND: u32 = 0x1dc;
const TYPE_USE_SOUND_LOOPS: usize = 0x1e8;
const TYPE_AMBIENT_SOUND: u32 = 0x1f0;
/// The type's per-animation-list flags, copied to each instance.
const TYPE_ANIM_FLAGS: u32 = 0x230;

/// `BFEntity` position, and its `vector<byte>` of per-animation-list flags.
const ENTITY_POS: u32 = 0x114;
const ENTITY_ANIM_FLAGS: u32 = 0xfc;
const UNIT_GOAL: u32 = 0x170;

/// Vtable slots.
mod slots {
    /// `SNDSound`.
    pub const SOUND_DELETE: u32 = 0x0;
    pub const SOUND_IS_PLAYING: u32 = 0x50;
    pub const SOUND_STOP: u32 = 0x60;
    /// `ZTBuilding`.
    pub const SEND_EVENT: u32 = 0x0;
    pub const SET_NAME: u32 = 0x1c;
    pub const RESET_INFO: u32 = 0x134;
    /// `BFWorldMgr`.
    pub const GET_TYPE: u32 = 0xc;
    pub const ADD_ENTITY: u32 = 0x18;
    /// Entity types.
    pub const CREATE_ENTITY: u32 = 0x24;
    /// Units.
    pub const SET_IS_REMOVED: u32 = 0xa8;
    pub const SET_VISIBLE: u32 = 0xcc;
    pub const FORCE_ANIM: u32 = 0x104;
    /// `(tile, slot, offset in the tile)`: puts the unit into the building's slot.
    pub const ENTER_SLOT: u32 = 0x284;
    /// `(building)`: sends a unit of an old save back to the building.
    pub const RESTORE_BUILDING_USE: u32 = 0x2b4;
}

/// Save versions from which a field is in the stream.
mod versions {
    pub const USERS: u32 = 0x1c;
    pub const TYPE_STATE: u32 = 0x20;
    pub const MONTHLY: u32 = 0x24;
    pub const LEGACY_FLAGS: u32 = 0x2d;
    pub const LEGACY_FLAG_COUNT: u32 = 0x30;
    pub const LEGACY_FLAGS_END: u32 = 0x43;
    pub const UPKEEP: u32 = 0x44;
    pub const UPGRADE: u32 = 0x4a;
    pub const MEMBERS: u32 = 0x4b;
    /// Older saves send the users of a building that animates them back through their own goals.
    pub const SLOTTED_USERS: u32 = 0x50;
}

const TRASH_CAN_TYPE_ID: u32 = 0x1779;
/// The fill level past which a loaded trash can calls for a keeper again, and the events a filled and an emptied can
/// send.
const TRASH_CAN_NOTICE_PASSED: i32 = 0xc;
const TRASH_FULL_EVENT: u32 = 0x272e;
const TRASH_SCATTERED_EVENT: u32 = 0x272f;
const TRASH_EVENT_FLAG: u32 = 0x4d;

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn vector_bounds(vector: u32) -> (u32, u32) {
    (word(vector), word(vector + 4))
}

fn call2(object: u32, slot: u32, first: u32, second: u32) {
    unsafe { virtual_method::<extern "thiscall" fn(u32, u32, u32)>(object, slot)(object, first, second) }
}

fn position_of(entity: u32) -> [i32; 3] {
    std::array::from_fn(|axis| get_from_memory(entity + ENTITY_POS + axis as u32 * 4))
}

/// A world position's offset within its tile, as `BFEntity` placement takes it. A negative height counts down from the
/// top of its level, so a whole negative level is `16`.
pub(super) fn offset_in_tile([x, y, z]: [i32; 3]) -> IVec3 {
    IVec3::new(x % 64, y % 64, if z < 0 { z % 16 + 16 } else { z % 16 })
}

/// Stops `sound` when there is one and it is playing.
fn stop_sound(sound: u32) {
    if sound != 0 && low_byte_bool(call0(sound, slots::SOUND_IS_PLAYING)) {
        call0(sound, slots::SOUND_STOP);
    }
}

/// Sets the app object up on first use, as vanilla's inlined accessor does.
fn ensure_app() {
    if word(APP_GUARD) == 0 {
        save_to_memory(APP_GUARD, APP_GUARD_INIT);
    }
}

/// A byte of the app object.
pub(super) fn app_byte(offset: u32) -> u8 {
    ensure_app();
    get_from_memory(APP + offset)
}

/// A new, empty `SNDSound` on vanilla's heap.
pub(super) fn new_sound() -> u32 {
    let sound = unsafe { OPERATOR_NEW.original()(8) } as u32;
    if sound != 0 {
        save_to_memory(sound + 4, 0u32);
        save_to_memory(sound, base() + SOUND_VTABLE_RVA);
    }
    sound
}

/// Cuts or zero-fills the vanilla byte vector at `vector` to `length` entries.
fn resize_bytes(vector: u32, length: u32) {
    let (begin, end) = vector_bounds(vector);
    let size = end.wrapping_sub(begin);
    if length < size {
        save_to_memory(vector + 4, begin + length);
    } else {
        let zero = 0u8;
        unsafe { BYTE_VECTOR_INSERT.original()(vector as *const u8, end as *const u32, (length - size) as *const u32, std::ptr::from_ref(&zero).cast()) };
    }
}

/// Cuts or zero-fills the vanilla word vector at `vector` to `length` entries.
fn resize_words(vector: u32, length: u32) {
    let (begin, end) = vector_bounds(vector);
    let size = end.wrapping_sub(begin) >> 2;
    if length < size {
        save_to_memory(vector + 4, begin + length * 4);
    } else {
        let zero = 0u32;
        unsafe { WORD_VECTOR_INSERT.original()(vector as *const u32, end as *const u32, (length - size) as *const u32, &zero) };
    }
}

impl ZTBuilding {
    /// The `ZTBuilding` constructor (`0x00451152`) over memory of [`super::BUILDING_SIZE`] bytes.
    pub fn construct(self, building_type: u32) {
        unsafe { ZTSCENERY_CONSTRUCTOR.hooked()(self.0 as *const u32, building_type as i32) };
        for offset in (building_offsets::MEMBERS..building_offsets::SLOTS + 12).step_by(4) {
            self.write(offset, 0u32);
        }
        self.write(building_offsets::UPGRADE_INDEX, 0u32);
        self.write(building_offsets::USE_SOUND, 0u32);
        self.write(building_offsets::AMBIENT_SOUND, 0u32);
        self.write(building_offsets::SHOW_ID, 0u16);
        self.write(building_offsets::WATER_CAPACITY, 0i32);
        self.write(0, base() + BUILDING_VTABLE_RVA);
        if building_type != 0 {
            for (name, field) in [(TYPE_USE_SOUND, building_offsets::USE_SOUND), (TYPE_AMBIENT_SOUND, building_offsets::AMBIENT_SOUND)] {
                let (begin, end) = vector_bounds(building_type + name);
                if begin != end {
                    self.write(field, new_sound());
                }
            }
        }
        self.reset_construction_date();
    }

    /// The `ZTBuilding` destructor (`0x004f9e36`): the sounds, the three containers and the slots' draw users, then the
    /// base. Answers what the base destructor leaves in `EAX`.
    pub fn destruct(self) -> u32 {
        self.write(0, base() + BUILDING_VTABLE_RVA);
        let ambient: u32 = self.read(building_offsets::AMBIENT_SOUND);
        stop_sound(ambient);
        let ambient: u32 = self.read(building_offsets::AMBIENT_SOUND);
        if ambient != 0 {
            call1(ambient, slots::SOUND_DELETE, 1);
        }
        self.write(building_offsets::AMBIENT_SOUND, 0u32);
        if self.entity_type() != 0 && self.type_field::<u8>(TYPE_USE_SOUND_LOOPS) != 0 {
            stop_sound(self.read(building_offsets::USE_SOUND));
        }
        let use_sound: u32 = self.read(building_offsets::USE_SOUND);
        if use_sound != 0 {
            call1(use_sound, slots::SOUND_DELETE, 1);
        }
        self.write(building_offsets::USE_SOUND, 0u32);

        let slot_size = size_of::<ZTBuildingSlot>() as u32;
        let (begin, end) = vector_bounds(self.0 + building_offsets::SLOTS);
        let mut slot = begin;
        while slot != end {
            unsafe { VECTOR_T_4_DESTRUCTOR.original()((slot + offset_of!(ZTBuildingSlot, draw_users) as u32) as *const u32) };
            slot += slot_size;
        }
        let capacity_end = |offset: u32| -> u32 { self.read(offset + 8) };
        if begin != 0 {
            let bytes = capacity_end(building_offsets::SLOTS).wrapping_sub(begin) / slot_size * slot_size;
            unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) };
        }
        let users: u32 = self.read(building_offsets::USERS);
        if users != 0 {
            let bytes = capacity_end(building_offsets::USERS).wrapping_sub(users) & !7;
            unsafe { POOLALLOC_DEALLOCATE.original()(users as *const u32, bytes) };
        }
        let members: u32 = self.read(building_offsets::MEMBERS);
        if members != 0 {
            let count = capacity_end(building_offsets::MEMBERS).wrapping_sub(members) >> 2;
            unsafe { POOLALLOC_DEALLOCATE_N_4.original()(members as *const u32, count as i32) };
        }
        unsafe { ZTSCENERY_DESTRUCTOR.hooked()(self.0 as *const u32) as u32 }
    }

    /// `ZTBuilding::initStatusVars` (`0x00450f17`): resets the building and rebuilds one slot per slot record of its
    /// type.
    pub fn init_status_vars(self) {
        unsafe { BFENTITY_INIT_STATUS_VARS.hooked()(self.0 as *const u32) };
        call0(self.0, slots::RESET_INFO);
        let building_type = self.entity_type();
        if building_type == 0 {
            return;
        }
        self.write(building_offsets::PRICE, word(building_type + offset_of!(ZTBuildingType, default_cost) as u32));
        self.rebuild_slots(building_type);
        self.write(building_offsets::USE_SOUND_VOLUME, 0i32);
        self.write(building_offsets::AMBIENT_SOUND_VOLUME, 0i32);
        self.user_data_changed();

        let (begin, end) = vector_bounds(building_type + TYPE_ANIM_LISTS);
        let count = (end.wrapping_sub(begin) as i32 / 12) as u32;
        resize_bytes(self.0 + ENTITY_ANIM_FLAGS, count);
        let source = word(building_type + TYPE_ANIM_FLAGS);
        if source != 0 {
            for index in 0..count {
                let flags: u32 = self.read(ENTITY_ANIM_FLAGS);
                save_to_memory(flags + index, get_from_memory::<u8>(source + index));
            }
        }
    }

    /// Empties the slot vector, keeping its buffer, and appends a slot for each of the type's slot records.
    fn rebuild_slots(self, building_type: u32) {
        let slot_size = size_of::<ZTBuildingSlot>() as u32;
        let record_size = size_of::<ZTBuildingSlotType>() as u32;
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        let (records_begin, records_end) = vector_bounds(records);
        let count = (records_end.wrapping_sub(records_begin) as i32 / record_size as i32) as u32;

        let vector = self.0 + building_offsets::SLOTS;
        let (begin, end) = vector_bounds(vector);
        let mut slot = begin;
        while slot != end {
            unsafe { SLOT_DESTRUCTOR.original()(slot as *const u32, 0) };
            slot += slot_size;
        }
        save_to_memory(vector + 4, begin);

        for index in 0..count {
            let end: u32 = get_from_memory(vector + 4);
            if end == get_from_memory(vector + 8) {
                let mut empty = [0u32; 9];
                let empty = empty.as_mut_ptr();
                unsafe {
                    SLOT_VECTOR_INSERT.original()(vector as *const u32, end as *const u32, empty, 1);
                    VECTOR_T_4_DESTRUCTOR.original()(empty.add(offset_of!(ZTBuildingSlot, draw_users) / 4));
                }
            } else {
                for offset in (0..slot_size).step_by(4) {
                    save_to_memory(end + offset, 0u32);
                }
                save_to_memory(vector + 4, end + slot_size);
            }
            let slot = word(vector) + index * slot_size;
            let field = |offset: usize| slot + offset as u32;
            save_to_memory(field(offset_of!(ZTBuildingSlot, index)), index);
            save_to_memory(field(offset_of!(ZTBuildingSlot, building)), self.0);
            let record = word(records) + index * record_size;
            let has_draw: u8 = get_from_memory(record + offset_of!(ZTBuildingSlotType, has_draw_slots) as u32);
            let draw_count = if has_draw != 0 { word(record + offset_of!(ZTBuildingSlotType, capacity) as u32) } else { 0 };
            save_to_memory(field(offset_of!(ZTBuildingSlot, draw_slot_count)), draw_count);
            if draw_count != 0 {
                save_to_memory(field(offset_of!(ZTBuildingSlot, has_draw_slots)), 1u8);
            }
            resize_words(field(offset_of!(ZTBuildingSlot, draw_users)), draw_count);
            ZTSlot(slot).clear_draw_users();
        }
    }

    /// `ZTBuilding::resetInfo` (`0x00450dbf`): zeroes the counters, removes the units inside and forgets the members.
    pub fn reset_info(self) {
        for offset in [
            building_offsets::TYPE_STATE,
            building_offsets::OCCUPANCY,
            building_offsets::TOTAL_USERS,
            building_offsets::LAST_MONTH_USERS,
            building_offsets::THIS_MONTH_USERS,
            building_offsets::TOTAL_INCOME,
            building_offsets::LAST_MONTH_INCOME,
            building_offsets::THIS_MONTH_INCOME,
            building_offsets::TOTAL_UPKEEP,
        ] {
            self.write(offset, 0u32);
        }
        self.reset_construction_date();

        let slot_size = size_of::<ZTBuildingSlot>() as u32;
        let mut slot: u32 = self.read(building_offsets::SLOTS);
        while slot != self.read(building_offsets::SLOTS + 4) {
            save_to_memory(slot + offset_of!(ZTBuildingSlot, user_count) as u32, 0u32);
            save_to_memory(slot + offset_of!(ZTBuildingSlot, top_user) as u32, 0u32);
            slot += slot_size;
        }

        let world = globals().ztworldmgr_ptr() as u32;
        let mut user: u32 = self.read(building_offsets::USERS);
        while user != self.read(building_offsets::USERS + 4) {
            let unit = world_entity(world, word(user));
            if cast(unit, UNIT_TAG_RVA) != 0 {
                ZTUnit(unit).set_is_removed(1);
            }
            user += 8;
        }
        self.write(building_offsets::USERS + 4, self.read::<u32>(building_offsets::USERS));

        let mut index = 0;
        while index < self.slot_count() as u32 {
            let slots: u32 = self.read(building_offsets::SLOTS);
            ZTSlot(slots + index * slot_size).clear_draw_users();
            index += 1;
        }
        self.write(building_offsets::MEMBERS + 4, self.read::<u32>(building_offsets::MEMBERS));
        self.init_random_use_timer();
    }

    /// `ZTBuilding::addToMap` (`0x0045158c`): puts the building on the map and into the world's building lists.
    pub fn add_to_map(self) -> bool {
        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 || !low_byte_bool(unsafe { ZTSCENERY_ADD_TO_MAP.hooked()(self.0 as *const u32) }) {
            return false;
        }
        let listed = low_byte_bool(unsafe { ADD_TO_ALL_BUILDING_LISTS.hooked()(world as *const u32, self.0 as *const u32) });
        if self.type_field::<u8>(TYPE_SHOW_TRICKS as usize) != 0 {
            unsafe { UPDATE_AVAILABLE_TRICK_LIST.hooked()(std::ptr::null()) };
        }
        listed
    }

    /// `ZTBuilding::removeFromMap` (`0x0050023a`). Unless `keep_contents` is set, the users are ejected, the members of
    /// a building that does not track its users are removed from the world and a trash can spills its trash.
    pub fn remove_from_map(self, keep_contents: bool) {
        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 {
            return;
        }
        let building_type = self.entity_type();
        unsafe {
            REMOVE_FROM_ALL_BUILDING_LISTS.hooked()(world as *const u32, self.0 as *const i32);
            ZTSCENERY_REMOVE_FROM_MAP.hooked()(self.0 as *const c_void, i8::from(keep_contents));
        }
        if !keep_contents {
            self.remove_all_users();
            if building_type != 0 && get_from_memory::<u8>(building_type + offset_of!(ZTBuildingType, user_tracker) as u32) == 0 {
                self.remove_members(world);
            }
            if self.entity_type() != 0 && self.type_id() == TRASH_CAN_TYPE_ID {
                self.spill_trash(world);
            }
        }
        if building_type != 0 && get_from_memory::<u8>(building_type + TYPE_USE_SOUND_LOOPS as u32) != 0 {
            stop_sound(self.read(building_offsets::USE_SOUND));
        }
        stop_sound(self.read(building_offsets::AMBIENT_SOUND));
    }

    /// Takes every member out of the world, last first.
    fn remove_members(self, world: u32) {
        loop {
            let (begin, end) = vector_bounds(self.0 + building_offsets::MEMBERS);
            if end.wrapping_sub(begin) >> 2 == 0 {
                return;
            }
            let id = word(end - 4);
            self.write(building_offsets::MEMBERS + 4, end - 4);
            let unit = world_entity(world, id);
            if unit != 0 {
                call1(unit, slots::SET_VISIBLE, 0);
                call2(unit, slots::SET_IS_REMOVED, 1, 0);
            }
        }
    }

    /// Drops one piece of trash on each of the eight points half a tile around the can, until it is empty.
    fn spill_trash(self, world: u32) {
        let trash_type = unsafe { GET_TRASH_TYPE.original()() } as u32;
        for column in -1..=1 {
            for dy in [-32, 0, 32] {
                if self.type_state() == 0 {
                    break;
                }
                if column == 0 && dy == 0 {
                    continue;
                }
                let [x, y, z] = position_of(self.0);
                let (x, y) = (x.wrapping_add(column * 32), y.wrapping_add(dy));
                let tile = map_tile(x / 64, y / 64);
                let trash = unsafe {
                    CREATE_SCENERY.hooked()(world as *const u32, trash_type as *const u32, 0, tile as *const u32, offset_in_tile([x, y, z]))
                };
                if !trash.is_null() {
                    self.send_value_event(TRASH_SCATTERED_EVENT, tile);
                    self.set_type_state(self.type_state().wrapping_sub(1));
                }
            }
        }
        self.set_type_state(0);
        self.user_data_changed();
    }

    /// Sends a trash event through the building's own `sendEvent` slot.
    fn send_value_event(self, event: u32, value: u32) {
        type SendEvent = extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u32);
        let send = unsafe { virtual_method::<SendEvent>(self.0, slots::SEND_EVENT) };
        send(self.0, event, 0, TRASH_EVENT_FLAG, value, 0, 1);
    }

    /// `ZTBuilding::save` (`0x00477147`). Every write is attempted whatever came before.
    pub fn save(self, file: u32) -> bool {
        let mut ok = unsafe { ZTSCENERY_SAVE.hooked()(self.0 as *const c_void, file as *const i8) };
        let file = file as *const u32;
        let write = |address: u32, size: u32| write_field(address, size, file);
        let at = |offset: u32| self.0 + offset;
        let local = |value: &u32| write(value as *const u32 as u32, 4);

        for offset in [building_offsets::OCCUPANCY, building_offsets::TOTAL_USERS, building_offsets::TOTAL_INCOME, building_offsets::PRICE] {
            ok &= write(at(offset), 4);
        }
        let (begin, end) = vector_bounds(at(building_offsets::USERS));
        ok &= local(&((end.wrapping_sub(begin) as i32 >> 3) as u32));
        let mut user = begin;
        while user != word(at(building_offsets::USERS + 4)) {
            let (id, slot) = (word(user), word(user + 4));
            ok &= local(&id);
            ok &= local(&slot);
            user += 8;
        }
        for offset in [
            building_offsets::TYPE_STATE,
            building_offsets::LAST_MONTH_INCOME,
            building_offsets::THIS_MONTH_INCOME,
            building_offsets::LAST_MONTH_USERS,
            building_offsets::THIS_MONTH_USERS,
        ] {
            ok &= write(at(offset), 4);
        }
        ok &= write(at(building_offsets::CONSTRUCTION_DATE), 8);
        ok &= write(at(building_offsets::TOTAL_UPKEEP), 4);
        ok &= write(at(building_offsets::UPGRADE_INDEX), 4);
        let (begin, end) = vector_bounds(at(building_offsets::MEMBERS));
        ok &= local(&((end.wrapping_sub(begin) as i32 >> 2) as u32));
        let mut member = begin;
        while member != word(at(building_offsets::MEMBERS + 4)) {
            let id = word(member);
            ok &= local(&id);
            member += 4;
        }
        ok
    }

    /// `ZTBuilding::load` (`0x0045184a`). The users and members read are appended to what the vectors hold.
    pub fn load(self, file: u32, version: u32) -> bool {
        if !unsafe { ZTSCENERY_LOAD.hooked()(self.0 as *const c_void, file as *const u8, version) } {
            return false;
        }
        let file = file as *const u32;
        let read = |offset: u32, size: u32| read_field(self.0 + offset, size, file);
        let read_local = |value: &mut u32, size: u32| read_field(value as *mut u32 as u32, size, file);
        let mut ok = true;

        if version < versions::USERS {
            self.write(building_offsets::OCCUPANCY, 0u32);
            self.write(building_offsets::TOTAL_USERS, 0u32);
            self.write(building_offsets::TOTAL_INCOME, 0u32);
            self.write(building_offsets::PRICE, self.type_field::<u32>(offset_of!(ZTBuildingType, default_cost)));
        } else {
            for offset in [building_offsets::OCCUPANCY, building_offsets::TOTAL_USERS, building_offsets::TOTAL_INCOME, building_offsets::PRICE] {
                ok &= read(offset, 4);
            }
            let slot_size = size_of::<ZTBuildingSlot>() as u32;
            for index in 0..self.slot_count() as u32 {
                let slot = self.read::<u32>(building_offsets::SLOTS) + index * slot_size;
                save_to_memory(slot + offset_of!(ZTBuildingSlot, user_count) as u32, 0u32);
                save_to_memory(slot + offset_of!(ZTBuildingSlot, top_user) as u32, 0u32);
            }
            let mut count = 0u32;
            let counted = read_local(&mut count, 4);
            ok &= counted;
            if counted && self.occupancy() < count as i32 {
                self.write(building_offsets::OCCUPANCY, count);
            }
            if !ok {
                return false;
            }
            for _ in 0..count {
                let (mut id, mut slot) = (0u32, 0u32);
                ok &= read_local(&mut id, 4);
                ok &= read_local(&mut slot, 4);
                if !ok {
                    return false;
                }
                push_pair(self.0 + building_offsets::USERS, id, slot, |begin, bytes| {
                    if begin != 0 {
                        unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) }
                    }
                });
            }
        }

        if version < versions::TYPE_STATE {
            self.write(building_offsets::TYPE_STATE, 0u32);
        } else {
            ok &= read(building_offsets::TYPE_STATE, 4);
        }

        let monthly = [
            building_offsets::LAST_MONTH_INCOME,
            building_offsets::THIS_MONTH_INCOME,
            building_offsets::LAST_MONTH_USERS,
            building_offsets::THIS_MONTH_USERS,
        ];
        if version < versions::MONTHLY {
            for offset in monthly {
                self.write(offset, 0u32);
            }
            self.reset_construction_date();
        } else {
            for offset in monthly {
                ok &= read(offset, 4);
            }
            ok &= read(building_offsets::CONSTRUCTION_DATE, 8);
        }

        if (versions::LEGACY_FLAGS..versions::LEGACY_FLAGS_END).contains(&version) {
            let (begin, end) = vector_bounds(self.0 + ENTITY_ANIM_FLAGS);
            let length = end.wrapping_sub(begin);
            let mut count = 2u32;
            if version >= versions::LEGACY_FLAG_COUNT {
                ok &= read_local(&mut count, 4);
            }
            for index in 0..count {
                let mut discarded = 0u32;
                ok &= if index < length {
                    read_field(self.read::<u32>(ENTITY_ANIM_FLAGS) + index, 1, file)
                } else {
                    read_local(&mut discarded, 1)
                };
            }
        }

        if version < versions::UPKEEP {
            self.write(building_offsets::TOTAL_UPKEEP, 0u32);
        } else {
            ok &= read(building_offsets::TOTAL_UPKEEP, 4);
        }
        if version >= versions::UPGRADE {
            ok &= read(building_offsets::UPGRADE_INDEX, 4);
        }
        if version >= versions::MEMBERS {
            let mut count = 0u32;
            ok &= read_local(&mut count, 4);
            if !ok {
                return false;
            }
            for _ in 0..(count as i32).max(0) {
                let mut id = 0u32;
                ok &= read_local(&mut id, 4);
                if !ok {
                    return false;
                }
                push_word(self.0 + building_offsets::MEMBERS, id, |begin, bytes| unsafe {
                    POOLALLOC_DEALLOCATE_N_4.original()(begin as *const u32, (bytes >> 2) as i32)
                });
            }
        }
        ok
    }

    /// Whether the units inside stand in the building's slots, so they are drawn there.
    fn animates_users(self) -> bool {
        self.type_field::<u8>(offset_of!(ZTBuildingType, hide_user)) == 0
            && self.type_field::<u8>(offset_of!(ZTBuildingType, user_stays_outside)) == 0
    }

    /// `ZTBuilding::initAfterLoad` (`0x00593fd3`): puts the loaded users back into their slots, dropping those that no
    /// longer exist, then runs the base. Answers the base's result.
    pub fn init_after_load(self, version: u32) -> bool {
        if self.occupancy() > 0 {
            self.might_start_use_sound();
        }
        let world = globals().ztworldmgr_ptr() as u32;
        let mut user: u32 = self.read(building_offsets::USERS);
        while user != self.read(building_offsets::USERS + 4) {
            let slot = word(user + 4);
            let unit = world_entity(world, word(user));
            if unit == 0 {
                erase_element(self.0 + building_offsets::USERS, user, 8);
                continue;
            }
            if version < versions::SLOTTED_USERS && self.animates_users() {
                let unit = cast(unit, UNIT_TAG_RVA);
                if unit != 0 {
                    call1(unit, slots::RESTORE_BUILDING_USE, self.0);
                }
            } else {
                self.restore_user(unit, slot);
            }
            user += 8;
        }
        if version < versions::SLOTTED_USERS && self.entity_type() != 0 && self.animates_users() {
            let (begin, end) = vector_bounds(self.0 + building_offsets::USERS);
            let count = end.wrapping_sub(begin) as i32 >> 3;
            self.write(building_offsets::USERS + 4, begin);
            self.write(building_offsets::OCCUPANCY, 0i32);
            for offset in [building_offsets::TOTAL_USERS, building_offsets::LAST_MONTH_USERS, building_offsets::THIS_MONTH_USERS] {
                self.write(offset, self.read::<i32>(offset).wrapping_sub(count));
            }
        }
        self.user_data_changed();
        let capacity = self.calculate_water_capacity();
        self.write(building_offsets::WATER_CAPACITY, capacity);
        if self.entity_type() != 0 && self.type_id() == TRASH_CAN_TYPE_ID && self.type_state() > TRASH_CAN_NOTICE_PASSED {
            let tile = unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) } as u32;
            self.send_value_event(TRASH_FULL_EVENT, tile);
        }
        unsafe { SCENERY_INIT_AFTER_LOAD.hooked()(self.0 as *const c_void, version) }
    }

    /// Counts a loaded user into its slot and puts it back where a user of this building is.
    fn restore_user(self, unit: u32, slot: u32) {
        if slot < self.slot_count() as u32 {
            let entry = self.read::<u32>(building_offsets::SLOTS) + slot * size_of::<ZTBuildingSlot>() as u32;
            let count = entry + offset_of!(ZTBuildingSlot, user_count) as u32;
            save_to_memory(count, word(count).wrapping_add(1));
            save_to_memory(entry + offset_of!(ZTBuildingSlot, top_user) as u32, unit);
            if self.animates_users() && get_from_memory::<u8>(entry + offset_of!(ZTBuildingSlot, has_draw_slots) as u32) != 0 {
                ZTSlot(entry).add_draw_user(unit, -1);
            }
        }
        let visitor = cast(unit, GUEST_TAG_RVA) != 0 || cast(unit, ANIMAL_TAG_RVA) != 0;
        let unit = cast(unit, UNIT_TAG_RVA);
        if unit == 0 {
            return;
        }
        if visitor && !BUILDING_USE_GOALS.contains(&word(unit + UNIT_GOAL)) {
            self.enter_slot(unit, slot);
        }
        let building_type = self.entity_type();
        if building_type != 0 {
            let name = object_bytes(building_type + TYPE_USE_ANIMATION);
            if !name.is_empty() {
                let name = VanillaString::from_bytes(&name);
                call1(unit, slots::FORCE_ANIM, name.as_ptr() as u32);
            }
        }
    }

    /// Hands `unit` its place in slot `slot`: the building's tile and the building's offset within it.
    fn enter_slot(self, unit: u32, slot: u32) {
        let offset = offset_in_tile(position_of(self.0));
        let tile = unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) } as u32;
        type EnterSlot = extern "thiscall" fn(u32, u32, u32, IVec3);
        let enter = unsafe { virtual_method::<EnterSlot>(unit, slots::ENTER_SLOT) };
        enter(unit, tile, slot, offset);
    }

    /// `ZTBuilding::createName` (`0x004e530d`): the type's name string, numbered for the types that number theirs. A type
    /// without one is named as any entity is.
    // The `1` passed to `getUnitCount` is a flag in a pointer-typed argument, not a dangling pointer.
    #[allow(clippy::manual_dangling_ptr)]
    pub fn create_name(self) {
        let building_type = self.entity_type();
        if building_type == 0 {
            return;
        }
        let id = self.type_id();
        ensure_app();
        let mut buffer = [0u8; NAME_BUFFER];
        unsafe { LOAD_STRING.hooked()(APP as *const u32, id as *const u32, buffer.as_mut_ptr()) };
        let mut name: Vec<u8> = buffer.iter().copied().take_while(|&byte| byte != 0).collect();
        if name.is_empty() {
            unsafe { BFENTITY_CREATE_NAME.hooked()(self.0 as *const u32) };
            return;
        }
        if get_from_memory::<u8>(building_type + TYPE_NUMBERED_NAME) != 0 {
            let number = unsafe { GET_UNIT_COUNT.hooked()((building_type + TYPE_NAME_KEY) as *const u8, 1 as *const i8) };
            name.extend(c_string_bytes(base() + NAME_SEPARATOR_RVA));
            let text = VanillaString::rvo_target();
            unsafe { GET_NUMBER_TEXT.hooked()(text.as_ptr(), number as u32, 0) };
            name.extend(text.bytes());
        }
        let name = VanillaString::from_bytes(&name);
        call1(self.0, slots::SET_NAME, name.as_ptr() as u32);
    }

    /// `ZTBuilding::drop` (`0x004e5282`): creates the unit each slot record is born with, and puts it into that slot as
    /// a user and a member.
    pub fn drop_residents(self) {
        let building_type = self.entity_type();
        if building_type == 0 {
            return;
        }
        let world = globals().ztworldmgr_ptr() as u32;
        let record_size = size_of::<ZTBuildingSlotType>() as u32;
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        let (begin, end) = vector_bounds(records);
        let count = end.wrapping_sub(begin) as i32 / record_size as i32;
        for slot in 0..count.max(0) as u32 {
            let birth = word(word(records) + slot * record_size + offset_of!(ZTBuildingSlotType, birth_user_type_id) as u32);
            if birth == 0 {
                continue;
            }
            let unit_type = unsafe { virtual_method::<extern "thiscall" fn(u32, u32) -> u32>(world, slots::GET_TYPE)(world, birth) };
            if unit_type == 0 {
                continue;
            }
            let unit = unsafe { virtual_method::<extern "thiscall" fn(u32, u32) -> u32>(unit_type, slots::CREATE_ENTITY)(unit_type, 0) };
            if cast(unit, UNIT_TAG_RVA) == 0 {
                continue;
            }
            call1(world, slots::ADD_ENTITY, unit);
            self.enter_slot(unit, slot);
            self.add_user(unit, slot);
            self.add_member(unit);
        }
    }
}

#[detour_mod]
mod lifecycle_detours {
    use super::*;

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor(this: *const u32, building_type: *const u32) -> *const u32 {
        ZTBuilding(this as u32).construct(building_type as u32);
        this
    }

    #[detour(DESTRUCTOR_0)]
    unsafe extern "thiscall" fn destructor(this: *const u32) -> *const u32 {
        ZTBuilding(this as u32).destruct() as *const u32
    }

    #[detour(DESTRUCTOR_1)]
    unsafe extern "thiscall" fn deleting_destructor(this: *const u32, flags: u8) -> *const u32 {
        ZTBuilding(this as u32).destruct();
        if flags & 1 != 0 {
            unsafe { OPERATOR_DELETE.original()(this as u32) };
        }
        this
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const u32) {
        ZTBuilding(this as u32).init_status_vars();
    }

    #[detour(RESET_INFO)]
    unsafe extern "thiscall" fn reset_info(this: *const u32) {
        ZTBuilding(this as u32).reset_info();
    }

    #[detour(ADD_TO_MAP)]
    unsafe extern "thiscall" fn add_to_map(this: *const u32) -> u8 {
        u8::from(ZTBuilding(this as u32).add_to_map())
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, keep_contents: bool) {
        ZTBuilding(this as u32).remove_from_map(keep_contents);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        ZTBuilding(this as u32).save(file as u32)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        ZTBuilding(this as u32).load(file as u32, version)
    }

    #[detour(BUILDING_INIT_AFTER_LOAD)]
    unsafe extern "thiscall" fn init_after_load(this: *const u32, version: u32) -> bool {
        ZTBuilding(this as u32).init_after_load(version)
    }

    #[detour(CREATE_NAME)]
    unsafe extern "thiscall" fn create_name(this: *const u32) {
        ZTBuilding(this as u32).create_name();
    }

    #[detour(DROP)]
    unsafe extern "thiscall" fn drop_residents(this: *const u32) {
        ZTBuilding(this as u32).drop_residents();
    }

    /// The vanilla bodies, for the live tests.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn construct(this: u32, building_type: u32) {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this as *const u32, building_type as *const u32) };
        }

        pub(crate) fn destruct(this: u32) {
            unsafe { super::DESTRUCTOR_0_DETOUR.call(this as *const u32) };
        }

        pub(crate) fn delete(this: u32) {
            unsafe { super::DESTRUCTOR_1_DETOUR.call(this as *const u32, 1) };
        }

        pub(crate) fn init_status_vars(this: u32) {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn reset_info(this: u32) {
            unsafe { super::RESET_INFO_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn add_to_map(this: u32) -> bool {
            unsafe { super::ADD_TO_MAP_DETOUR.call(this as *const u32) != 0 }
        }

        pub(crate) fn remove_from_map(this: u32, keep_contents: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const u32, keep_contents) }
        }

        pub(crate) fn save(this: u32, file: u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file as *const u32) }
        }

        pub(crate) fn load(this: u32, file: u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file as *const u32, version) }
        }

        pub(crate) fn init_after_load(this: u32, version: u32) -> bool {
            unsafe { super::BUILDING_INIT_AFTER_LOAD_DETOUR.call(this as *const u32, version) }
        }

        pub(crate) fn create_name(this: u32) {
            unsafe { super::CREATE_NAME_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn drop_residents(this: u32) {
            unsafe { super::DROP_DETOUR.call(this as *const u32) }
        }
    }
}

/// Installs the lifecycle detours.
pub(super) fn init() {
    if let Err(e) = unsafe { lifecycle_detours::init_detours() } {
        tracing::error!("Failed to initialise ztbuilding lifecycle detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    lifecycle_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use lifecycle_detours::test_real;

#[cfg(test)]
mod tests {
    use super::offset_in_tile;
    use glam::IVec3;

    #[test]
    fn offsets_within_a_tile_truncate_toward_zero() {
        assert_eq!(offset_in_tile([130, 65, 18]), IVec3::new(2, 1, 2));
        assert_eq!(offset_in_tile([-1, -65, -1]), IVec3::new(-1, -1, 15));
        assert_eq!(offset_in_tile([0, 64, -16]), IVec3::new(0, 0, 16));
    }
}
