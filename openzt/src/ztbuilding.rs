//! `ZTBuilding`, the building instance class (`ZTScenery` subclass, vtable `0x0062e0b0`, object size `0x1d0`).
//! This module holds the layout, the handle ported code reads building fields through, and the ported leaf accessors
//! and bookkeeping methods; `users` holds the users and slots, `geometry` the slot positions and exits,
//! `lifecycle` construction, the map, persistence and the name, `tick` the update, animations, sounds, water and
//! events, and `rubble` the building's destruction (`plans/ztbuilding-scoping-plan.md`).
//!
//! The object stays in vanilla layout because the `ZTScenery`/`BFEntity` base, saves, the UI and the goal classes read
//! it directly. `0x000`-`0x157` is that base. The three vectors and each slot's draw-user vector are vanilla pool
//! allocations: never free or grow them through Rust's allocator.
//!
//! The type side (capacity, flags and the per-slot records) is `ZTBuildingType` in `bfentitytype/scenery.rs`.

use std::{
    ffi::c_void,
    mem::{offset_of, size_of, transmute},
};

use openzt_detour::{
    generated::{
        poolalloc::DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4,
        ztbuilding::{
            ADD_MEMBER, CHECK_USER, DO_CHANGES, ENTER_NEW_MONTH, GET_ADULT_CHANGE, GET_CHILD_CHANGE, GET_CURRENT_UPGRADE, GET_EVENTS,
            GET_LAST_MONTH_USERS, GET_THIS_MONTH_USERS, GET_TOTAL_USERS, GET_UPKEEP, RECEIVE_INCOME, REMOVE_MEMBER, RESET_CONSTRUCTION_DATE,
            SEND_EVENT, USE_UPGRADE,
        },
        ztbuildingtype::GET_UPGRADE,
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    bfentitytype::{ZTBuildingType, ZTBuildingUpgrade},
    globals::globals,
    systemtime::ticks_to_filetime,
    util::{get_from_memory, save_to_memory},
    vanilla_vector::{erase_element, push_word},
    ztmegatilemgr::entity_type_matches,
};

pub mod geometry;
pub mod lifecycle;
pub mod rubble;
pub mod tick;
pub mod users;

/// What `ZTBuildingType::createEntity` allocates.
pub const BUILDING_SIZE: u32 = 0x1d0;
/// Where the tail starts: the end of the `ZTScenery` base.
const TAIL_START: u32 = 0x158;
/// The `BFEntity` word holding the entity type.
const ENTITY_TYPE: u32 = 0x128;
/// The `BFEntity` entity id.
const ENTITY_ID: u32 = 0x124;

/// The `isCastClass` tag of `ZTBuilding`, as an RVA.
const BUILDING_TAG_RVA: u32 = 0x0023_8680;
/// The type's vtable slot returning its type id, and the one id that has no upkeep.
const TYPE_ID_SLOT: u32 = 0x20;
const NO_UPKEEP_TYPE_ID: i32 = 0x1f5f;
/// The unit vtable slot `doChanges` hands the six need changes to.
const UNIT_ADD_CHANGES_SLOT: u32 = 0x1fc;
/// The entity vtable slot of the three-argument `getEvents`.
const GET_EVENTS_SLOT: u32 = 0xc;
/// The tag a building passes with its id to `getEvents` and `sendEvent`.
const EVENT_SOURCE_TAG: u32 = 0x42;

/// `BFUnit::sendEvent`, which `ZTBuilding::sendEvent` calls directly. The sixth stack argument is a full dword (it
/// reaches the `BFAIMgr` listener's `u32` value), which the generated entry types as `u16`.
const BFUNIT_SEND_EVENT: FunctionDef<unsafe extern "thiscall" fn(*const u32, u16, u32, u8, u32, u32, u32, u32, u32)> =
    FunctionDef::new(0x0059_def9);

/// The `ZTBuilding` fields past the `ZTScenery` base. Vectors are `{begin, end, capacity}` addresses.
#[repr(C)]
pub struct ZTBuildingTail {
    /// `vector<u32>`: ids of the animals and keepers living in the building.
    pub members: [u32; 3],
    /// `vector<{unit id, slot index}>`: the users inside.
    pub users: [u32; 3],
    /// `vector<ZTBuildingSlot>`: one per slot record in the type.
    pub slots: [u32; 3],
    _gap_17c: [u8; 4],
    /// A `FILETIME` holding the game date the building was built.
    pub construction_date: [u32; 2],
    pub upgrade_index: u32,
    pub occupancy: i32,
    pub total_users: i32,
    pub last_month_users: i32,
    pub this_month_users: i32,
    /// Depends on the type: the trash count of a trash can (type id `0x1779`), the show-running flag of a grandstand.
    pub type_state: i32,
    pub total_income: f32,
    pub last_month_income: f32,
    pub this_month_income: f32,
    pub price: f32,
    pub total_upkeep: f32,
    pub use_sound_volume: i32,
    pub ambient_sound_volume: i32,
    /// `SNDSound*`, null when the type names no use sound.
    pub use_sound: u32,
    /// `SNDSound*`, null when the type names no ambient sound.
    pub ambient_sound: u32,
    /// Id of the show associated with the building, `0` for none.
    pub show_id: u16,
    _gap_1c6: [u8; 2],
    pub water_capacity: i32,
    _gap_1cc: [u8; 4],
}

/// One element of a building's slot vector (`ZTBuilding::Slot`).
#[repr(C)]
pub struct ZTBuildingSlot {
    /// Index of this slot, and of its record in the type.
    pub index: u32,
    pub user_count: i32,
    pub draw_slot_count: u32,
    /// Non-zero when `draw_slot_count` is.
    pub has_draw_slots: u8,
    _pad_0d: [u8; 3],
    /// The `ZTBuilding` that owns the slot.
    pub building: u32,
    pub top_user: u32,
    /// `vector<BFUnit*>`, one entry (possibly null) per draw slot.
    pub draw_users: [u32; 3],
}

/// Offsets from the start of a building.
pub mod building_offsets {
    use super::{ZTBuildingTail, TAIL_START};
    use std::mem::offset_of;

    pub const MEMBERS: u32 = TAIL_START + offset_of!(ZTBuildingTail, members) as u32;
    pub const USERS: u32 = TAIL_START + offset_of!(ZTBuildingTail, users) as u32;
    pub const SLOTS: u32 = TAIL_START + offset_of!(ZTBuildingTail, slots) as u32;
    pub const CONSTRUCTION_DATE: u32 = TAIL_START + offset_of!(ZTBuildingTail, construction_date) as u32;
    pub const UPGRADE_INDEX: u32 = TAIL_START + offset_of!(ZTBuildingTail, upgrade_index) as u32;
    pub const OCCUPANCY: u32 = TAIL_START + offset_of!(ZTBuildingTail, occupancy) as u32;
    pub const TOTAL_USERS: u32 = TAIL_START + offset_of!(ZTBuildingTail, total_users) as u32;
    pub const LAST_MONTH_USERS: u32 = TAIL_START + offset_of!(ZTBuildingTail, last_month_users) as u32;
    pub const THIS_MONTH_USERS: u32 = TAIL_START + offset_of!(ZTBuildingTail, this_month_users) as u32;
    pub const TYPE_STATE: u32 = TAIL_START + offset_of!(ZTBuildingTail, type_state) as u32;
    pub const TOTAL_INCOME: u32 = TAIL_START + offset_of!(ZTBuildingTail, total_income) as u32;
    pub const LAST_MONTH_INCOME: u32 = TAIL_START + offset_of!(ZTBuildingTail, last_month_income) as u32;
    pub const THIS_MONTH_INCOME: u32 = TAIL_START + offset_of!(ZTBuildingTail, this_month_income) as u32;
    pub const PRICE: u32 = TAIL_START + offset_of!(ZTBuildingTail, price) as u32;
    pub const TOTAL_UPKEEP: u32 = TAIL_START + offset_of!(ZTBuildingTail, total_upkeep) as u32;
    pub const USE_SOUND_VOLUME: u32 = TAIL_START + offset_of!(ZTBuildingTail, use_sound_volume) as u32;
    pub const AMBIENT_SOUND_VOLUME: u32 = TAIL_START + offset_of!(ZTBuildingTail, ambient_sound_volume) as u32;
    pub const USE_SOUND: u32 = TAIL_START + offset_of!(ZTBuildingTail, use_sound) as u32;
    pub const AMBIENT_SOUND: u32 = TAIL_START + offset_of!(ZTBuildingTail, ambient_sound) as u32;
    pub const SHOW_ID: u32 = TAIL_START + offset_of!(ZTBuildingTail, show_id) as u32;
    pub const WATER_CAPACITY: u32 = TAIL_START + offset_of!(ZTBuildingTail, water_capacity) as u32;
}

/// The addresses vanilla reads and writes, as the decompiles show them: a change to a struct that moves a field fails
/// the build here.
const _: () = {
    use building_offsets::*;
    assert!(size_of::<ZTBuildingTail>() as u32 == BUILDING_SIZE - TAIL_START);
    assert!(MEMBERS == 0x158 && USERS == 0x164 && SLOTS == 0x170 && CONSTRUCTION_DATE == 0x180);
    assert!(UPGRADE_INDEX == 0x188 && OCCUPANCY == 0x18c && TOTAL_USERS == 0x190 && TYPE_STATE == 0x19c);
    assert!(LAST_MONTH_USERS == 0x194 && THIS_MONTH_USERS == 0x198 && LAST_MONTH_INCOME == 0x1a4 && THIS_MONTH_INCOME == 0x1a8);
    assert!(TOTAL_INCOME == 0x1a0 && PRICE == 0x1ac && TOTAL_UPKEEP == 0x1b0 && USE_SOUND_VOLUME == 0x1b4);
    assert!(AMBIENT_SOUND_VOLUME == 0x1b8 && AMBIENT_SOUND == 0x1c0);
    assert!(USE_SOUND == 0x1bc && SHOW_ID == 0x1c4 && WATER_CAPACITY == 0x1c8);

    assert!(size_of::<ZTBuildingSlot>() == 0x24);
    assert!(offset_of!(ZTBuildingSlot, has_draw_slots) == 0x0c && offset_of!(ZTBuildingSlot, building) == 0x10);
    assert!(offset_of!(ZTBuildingSlot, top_user) == 0x14 && offset_of!(ZTBuildingSlot, draw_users) == 0x18);
};

/// A building in vanilla memory, addressed by its raw pointer. All reads and writes are volatile through the address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTBuilding(pub u32);

impl ZTBuilding {
    fn read<T>(self, offset: u32) -> T {
        get_from_memory(self.0 + offset)
    }

    fn write<T>(self, offset: u32, value: T) {
        save_to_memory(self.0 + offset, value);
    }

    /// The building's entity type pointer (a `ZTBuildingType` for every `ZTBuilding`).
    pub fn entity_type(self) -> u32 {
        self.read(ENTITY_TYPE)
    }

    /// Number of slots, with vanilla's signed division of the byte length.
    pub fn slot_count(self) -> i32 {
        let begin: i32 = self.read(building_offsets::SLOTS);
        let end: i32 = self.read(building_offsets::SLOTS + 4);
        end.wrapping_sub(begin) / size_of::<ZTBuildingSlot>() as i32
    }

    /// Number of users inside.
    pub fn occupancy(self) -> i32 {
        self.read(building_offsets::OCCUPANCY)
    }

    /// The trash count of a trash can, or the show-running flag of a grandstand.
    pub fn type_state(self) -> i32 {
        self.read(building_offsets::TYPE_STATE)
    }

    pub fn set_type_state(self, value: i32) {
        self.write(building_offsets::TYPE_STATE, value);
    }

    /// What the building charges.
    pub fn price(self) -> f32 {
        self.read(building_offsets::PRICE)
    }

    /// Id of the associated show, `0` for none.
    pub fn show_id(self) -> u16 {
        self.read(building_offsets::SHOW_ID)
    }

    fn id(self) -> u32 {
        self.read(ENTITY_ID)
    }

    /// A field of the building's type.
    fn type_field<T>(self, offset: usize) -> T {
        get_from_memory(self.entity_type() + offset as u32)
    }

    /// Whether the entity type casts to `ZTBuildingType`.
    fn is_building_type(self) -> bool {
        unsafe { entity_type_matches(self.0, BUILDING_TAG_RVA) }
    }

    /// `ZTBuilding::getTotalUsers` (`0x006174fa`).
    pub fn total_users(self) -> i32 {
        self.read(building_offsets::TOTAL_USERS)
    }

    /// `ZTBuilding::getLastMonthUsers` (`0x00617501`).
    pub fn last_month_users(self) -> i32 {
        self.read(building_offsets::LAST_MONTH_USERS)
    }

    /// `ZTBuilding::getThisMonthUsers` (`0x00617508`).
    pub fn this_month_users(self) -> i32 {
        self.read(building_offsets::THIS_MONTH_USERS)
    }

    /// `ZTBuilding::getCurrentUpgrade` (`0x00423497`): the `ZTBuildingUpgrade` at the upgrade index in the type's list,
    /// `0` past its end. A building without a type has none (vanilla faults).
    pub fn current_upgrade(self) -> u32 {
        let building_type = self.entity_type();
        if building_type == 0 {
            return 0;
        }
        let index: i32 = self.read(building_offsets::UPGRADE_INDEX);
        unsafe { GET_UPGRADE.original()(building_type as *const u32, index) as u32 }
    }

    /// `ZTBuilding::useUpgrade` (`0x00611b9f`): sets the upgrade index when the type has more upgrades than it.
    pub fn use_upgrade(self, index: u32) -> bool {
        if self.entity_type() == 0 {
            return false;
        }
        let sentinel: u32 = self.type_field(offset_of!(ZTBuildingType, upgrades));
        let mut count = 0u32;
        let mut node: u32 = get_from_memory(sentinel);
        while node != sentinel {
            node = get_from_memory(node);
            count += 1;
        }
        if index >= count {
            return false;
        }
        self.write(building_offsets::UPGRADE_INDEX, index);
        true
    }

    /// The upkeep at the width vanilla returns it, so a sum with it rounds once.
    fn upkeep_value(self) -> f64 {
        let upgrade = self.current_upgrade();
        if upgrade != 0 {
            return f64::from(get_from_memory::<i32>(upgrade + offset_of!(ZTBuildingUpgrade, upkeep) as u32));
        }
        if !self.is_building_type() {
            return 0.0;
        }
        let building_type = self.entity_type();
        let vtable: u32 = get_from_memory(building_type);
        let type_id = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> i32>(get_from_memory::<u32>(vtable + TYPE_ID_SLOT)) };
        if type_id(building_type) == NO_UPKEEP_TYPE_ID {
            return 0.0;
        }
        f64::from(self.type_field::<f32>(offset_of!(ZTBuildingType, upkeep)))
    }

    /// `ZTBuilding::getUpkeep` (`0x0059e17c`).
    pub fn upkeep(self) -> f32 {
        self.upkeep_value() as f32
    }

    /// A need change: the current upgrade's, else the type's. A building without a type changes nothing (vanilla
    /// faults).
    fn need_change(self, upgrade_offset: usize, type_offset: usize) -> i32 {
        if self.entity_type() == 0 {
            return 0;
        }
        match self.current_upgrade() {
            0 => self.type_field(type_offset),
            upgrade => get_from_memory(upgrade + upgrade_offset as u32),
        }
    }

    /// `ZTBuilding::getAdultChange` (`0x0042dda7`).
    pub fn adult_change(self) -> i32 {
        self.need_change(offset_of!(ZTBuildingUpgrade, adult_change), offset_of!(ZTBuildingType, adult_change))
    }

    /// `ZTBuilding::getChildChange` (`0x0042dd07`).
    pub fn child_change(self) -> i32 {
        self.need_change(offset_of!(ZTBuildingUpgrade, child_change), offset_of!(ZTBuildingType, child_change))
    }

    /// `ZTBuilding::doChanges` (`0x0042dd23`): hands the unit `{adult, child, hunger, thirst, bathroom, energy}` through
    /// its vtable. A null unit, or a building without a type, does nothing.
    pub fn do_changes(self, unit: u32) {
        if unit == 0 || self.entity_type() == 0 {
            return;
        }
        let changes: [i32; 6] = [
            self.adult_change(),
            self.child_change(),
            self.type_field(offset_of!(ZTBuildingType, hunger_change)),
            self.type_field(offset_of!(ZTBuildingType, thirst_change)),
            self.type_field(offset_of!(ZTBuildingType, bathroom_change)),
            self.type_field(offset_of!(ZTBuildingType, energy_change)),
        ];
        let vtable: u32 = get_from_memory(unit);
        let add_changes = unsafe {
            transmute::<u32, extern "thiscall" fn(u32, *const [i32; 6])>(get_from_memory::<u32>(vtable + UNIT_ADD_CHANGES_SLOT))
        };
        add_changes(unit, &changes);
    }

    /// `ZTBuilding::receiveIncome` (`0x004a2e96`): adds the price times the type's price factor to the total and this
    /// month's income. A type that is not a building type earns nothing (vanilla faults).
    pub fn receive_income(self) {
        if !self.is_building_type() {
            return;
        }
        let income = f64::from(self.price()) * f64::from(self.type_field::<f32>(offset_of!(ZTBuildingType, price_factor)));
        for offset in [building_offsets::TOTAL_INCOME, building_offsets::THIS_MONTH_INCOME] {
            let total = (income + f64::from(self.read::<f32>(offset))) as f32;
            self.write(offset, total);
        }
    }

    /// `ZTBuilding::enterNewMonth` (`0x004842b7`): rolls this month's users and income into last month's and adds the
    /// upkeep to the total upkeep.
    pub fn enter_new_month(self) {
        self.write(building_offsets::LAST_MONTH_INCOME, self.read::<u32>(building_offsets::THIS_MONTH_INCOME));
        self.write(building_offsets::LAST_MONTH_USERS, self.this_month_users());
        self.write(building_offsets::THIS_MONTH_INCOME, 0u32);
        self.write(building_offsets::THIS_MONTH_USERS, 0i32);
        let total = self.upkeep_value() + f64::from(self.read::<f32>(building_offsets::TOTAL_UPKEEP));
        self.write(building_offsets::TOTAL_UPKEEP, total as f32);
    }

    /// `ZTBuilding::checkUser` (`0x004f9b0b`): whether the unit is among the users inside. Vanilla also leaves the id it
    /// searched for in a scratch global (`0x0063afe8`) that each of its readers writes before reading. A null unit is
    /// not a user (vanilla faults when the building has users).
    pub fn check_user(self, unit: u32) -> bool {
        let begin: u32 = self.read(building_offsets::USERS);
        let end: u32 = self.read(building_offsets::USERS + 4);
        if unit == 0 || end.wrapping_sub(begin) >> 3 == 0 {
            return false;
        }
        let id: u32 = get_from_memory(unit + ENTITY_ID);
        (begin..end).step_by(8).any(|user| get_from_memory::<u32>(user) == id)
    }

    /// `ZTBuilding::addMember` (`0x0048cd9b`): appends the unit's id to the members.
    pub fn add_member(self, unit: u32) {
        if unit == 0 {
            return;
        }
        let id: u32 = get_from_memory(unit + ENTITY_ID);
        push_word(self.0 + building_offsets::MEMBERS, id, |begin, bytes| unsafe {
            POOLALLOC_DEALLOCATE_N_4.original()(begin as *const u32, (bytes >> 2) as i32)
        });
    }

    /// `ZTBuilding::removeMember` (`0x005a08ac`): erases the first member with the unit's id.
    pub fn remove_member(self, unit: u32) {
        #[cfg(feature = "reimplementation-tests")]
        if live_support::intercept::remove_member(self.0, unit) {
            return;
        }
        if unit == 0 {
            return;
        }
        let id: u32 = get_from_memory(unit + ENTITY_ID);
        let begin: u32 = self.read(building_offsets::MEMBERS);
        let end: u32 = self.read(building_offsets::MEMBERS + 4);
        if let Some(member) = (begin..end).step_by(4).find(|&member| get_from_memory::<u32>(member) == id) {
            erase_element(self.0 + building_offsets::MEMBERS, member, 4);
        }
    }

    /// `ZTBuilding::resetConstructionDate` (`0x00450edd`): stores the current game date.
    pub fn reset_construction_date(self) {
        let date = ticks_to_filetime(globals().ztgamemgr().get_date());
        self.write(building_offsets::CONSTRUCTION_DATE, date.dwLowDateTime);
        self.write(building_offsets::CONSTRUCTION_DATE + 4, date.dwHighDateTime);
    }

    /// `ZTBuilding::getEvents` (`0x00434d14`): asks the three-argument vtable slot for the events about this building.
    pub fn get_events(self, events: u32) {
        let vtable: u32 = get_from_memory(self.0);
        let get_events =
            unsafe { transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32)>(get_from_memory::<u32>(vtable + GET_EVENTS_SLOT)) };
        get_events(self.0, events, self.id(), EVENT_SOURCE_TAG);
    }

    /// `ZTBuilding::sendEvent` (`0x0059dfc7`): `BFUnit::sendEvent` with this building's id and tag as the source.
    pub fn send_event(self, kind: u16, target: u32, flag: u8, value: u32, extra: u16, count: u16) {
        let source = self.0 as *const u32;
        unsafe {
            BFUNIT_SEND_EVENT.original()(source, kind, target, flag, self.id(), EVENT_SOURCE_TAG, value, extra.into(), count.into())
        };
    }
}

#[detour_mod]
mod building_detours {
    use super::*;

    #[detour(GET_TOTAL_USERS)]
    unsafe extern "thiscall" fn get_total_users(this: *const c_void) -> u32 {
        ZTBuilding(this as u32).total_users() as u32
    }

    #[detour(GET_LAST_MONTH_USERS)]
    unsafe extern "thiscall" fn get_last_month_users(this: *const c_void) -> u32 {
        ZTBuilding(this as u32).last_month_users() as u32
    }

    #[detour(GET_THIS_MONTH_USERS)]
    unsafe extern "thiscall" fn get_this_month_users(this: *const c_void) -> u32 {
        ZTBuilding(this as u32).this_month_users() as u32
    }

    #[detour(GET_CURRENT_UPGRADE)]
    unsafe extern "thiscall" fn get_current_upgrade(this: *const u32) -> *const u32 {
        ZTBuilding(this as u32).current_upgrade() as *const u32
    }

    #[detour(USE_UPGRADE)]
    unsafe extern "thiscall" fn use_upgrade(this: *const c_void, index: u32) -> u32 {
        u32::from(ZTBuilding(this as u32).use_upgrade(index))
    }

    #[detour(GET_UPKEEP)]
    unsafe extern "thiscall" fn get_upkeep(this: *const u32) -> f32 {
        ZTBuilding(this as u32).upkeep()
    }

    #[detour(GET_ADULT_CHANGE)]
    unsafe extern "thiscall" fn get_adult_change(this: *const u32) -> u32 {
        ZTBuilding(this as u32).adult_change() as u32
    }

    #[detour(GET_CHILD_CHANGE)]
    unsafe extern "thiscall" fn get_child_change(this: *const u32) -> u32 {
        ZTBuilding(this as u32).child_change() as u32
    }

    #[detour(DO_CHANGES)]
    unsafe extern "thiscall" fn do_changes(this: *const u32, unit: *const u32) {
        ZTBuilding(this as u32).do_changes(unit as u32);
    }

    #[detour(RECEIVE_INCOME)]
    unsafe extern "fastcall" fn receive_income(this: i32) {
        ZTBuilding(this as u32).receive_income();
    }

    #[detour(ENTER_NEW_MONTH)]
    unsafe extern "thiscall" fn enter_new_month(this: *const c_void) {
        ZTBuilding(this as u32).enter_new_month();
    }

    #[detour(CHECK_USER)]
    unsafe extern "thiscall" fn check_user(this: *const c_void, unit: i32) -> u32 {
        u32::from(ZTBuilding(this as u32).check_user(unit as u32))
    }

    #[detour(ADD_MEMBER)]
    unsafe extern "thiscall" fn add_member(this: *const u32, unit: i32) {
        ZTBuilding(this as u32).add_member(unit as u32);
    }

    #[detour(REMOVE_MEMBER)]
    unsafe extern "thiscall" fn remove_member(this: *const u32, unit: i32) {
        ZTBuilding(this as u32).remove_member(unit as u32);
    }

    #[detour(RESET_CONSTRUCTION_DATE)]
    unsafe extern "thiscall" fn reset_construction_date(this: *const u32) {
        ZTBuilding(this as u32).reset_construction_date();
    }

    #[detour(GET_EVENTS)]
    unsafe extern "thiscall" fn get_events(this: *const u32, events: u32) {
        ZTBuilding(this as u32).get_events(events);
    }

    #[detour(SEND_EVENT)]
    unsafe extern "thiscall" fn send_event(this: *const u32, kind: u16, target: u32, flag: u8, value: u32, extra: u16, count: u16) {
        ZTBuilding(this as u32).send_event(kind, target, flag, value, extra, count);
    }

    /// The vanilla bodies, for the live tests.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn total_users(this: u32) -> i32 {
            unsafe { super::GET_TOTAL_USERS_DETOUR.call(this as *const c_void) as i32 }
        }

        pub(crate) fn last_month_users(this: u32) -> i32 {
            unsafe { super::GET_LAST_MONTH_USERS_DETOUR.call(this as *const c_void) as i32 }
        }

        pub(crate) fn this_month_users(this: u32) -> i32 {
            unsafe { super::GET_THIS_MONTH_USERS_DETOUR.call(this as *const c_void) as i32 }
        }

        pub(crate) fn current_upgrade(this: u32) -> u32 {
            unsafe { super::GET_CURRENT_UPGRADE_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn use_upgrade(this: u32, index: u32) -> bool {
            unsafe { super::USE_UPGRADE_DETOUR.call(this as *const c_void, index) & 0xff != 0 }
        }

        pub(crate) fn upkeep(this: u32) -> f32 {
            unsafe { super::GET_UPKEEP_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn adult_change(this: u32) -> i32 {
            unsafe { super::GET_ADULT_CHANGE_DETOUR.call(this as *const u32) as i32 }
        }

        pub(crate) fn child_change(this: u32) -> i32 {
            unsafe { super::GET_CHILD_CHANGE_DETOUR.call(this as *const u32) as i32 }
        }

        pub(crate) fn do_changes(this: u32, unit: u32) {
            unsafe { super::DO_CHANGES_DETOUR.call(this as *const u32, unit as *const u32) }
        }

        pub(crate) fn receive_income(this: u32) {
            unsafe { super::RECEIVE_INCOME_DETOUR.call(this as i32) }
        }

        pub(crate) fn enter_new_month(this: u32) {
            unsafe { super::ENTER_NEW_MONTH_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn check_user(this: u32, unit: u32) -> bool {
            unsafe { super::CHECK_USER_DETOUR.call(this as *const c_void, unit as i32) & 0xff != 0 }
        }

        pub(crate) fn add_member(this: u32, unit: u32) {
            unsafe { super::ADD_MEMBER_DETOUR.call(this as *const u32, unit as i32) }
        }

        pub(crate) fn remove_member(this: u32, unit: u32) {
            unsafe { super::REMOVE_MEMBER_DETOUR.call(this as *const u32, unit as i32) }
        }

        pub(crate) fn reset_construction_date(this: u32) {
            unsafe { super::RESET_CONSTRUCTION_DATE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_events(this: u32, events: u32) {
            unsafe { super::GET_EVENTS_DETOUR.call(this as *const u32, events) }
        }

        pub(crate) fn send_event(this: u32, kind: u16, target: u32, flag: u8, value: u32, extra: u16, count: u16) {
            unsafe { super::SEND_EVENT_DETOUR.call(this as *const u32, kind, target, flag, value, extra, count) }
        }
    }
}

/// Installs `ZTBuilding`'s detours. Called once at DLL load (via `lib.rs`) and before the `ZTBUILDING_*` tests in
/// `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { building_detours::init_detours() } {
        error!("Failed to initialise ztbuilding detours: {e:?}");
    }
    users::init();
    geometry::init();
    lifecycle::init();
    tick::init();
    rubble::init();
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::standalone::OPERATOR_NEW;

    pub(crate) use super::building_detours::test_real;
    pub(crate) use super::geometry::test_real as geometry_real;
    pub(crate) use super::lifecycle::test_real as lifecycle_real;
    pub(crate) use super::rubble::test_real as rubble_real;
    pub(crate) use super::tick::test_real as tick_real;
    pub(crate) use super::users::test_real as users_real;
    use super::{ZTBuilding, BUILDING_SIZE};

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        let mut status = super::building_detours::status();
        status.extend(super::users::status());
        status.extend(super::geometry::status());
        status.extend(super::lifecycle::status());
        status.extend(super::tick::status());
        status.extend(super::rubble::status());
        status
    }

    /// Test hooks that stand in for a building method a live test wants recorded instead of run: each returns `true`
    /// when it handled the call.
    pub(crate) mod intercept {
        use std::sync::Mutex;

        /// `(building, unit)`.
        pub(crate) static REMOVE_MEMBER: Mutex<Option<fn(u32, u32) -> bool>> = Mutex::new(None);

        pub(in crate::ztbuilding) fn remove_member(building: u32, unit: u32) -> bool {
            let hook = *REMOVE_MEMBER.lock().unwrap();
            hook.is_some_and(|hook| hook(building, unit))
        }

        macro_rules! skip_hooks {
            ($($hook:ident / $function:ident;)*) => {$(
                /// `(building)`; `true` means the hook handled the call.
                pub(crate) static $hook: Mutex<Option<fn(u32) -> bool>> = Mutex::new(None);

                pub(in crate::ztbuilding) fn $function(building: u32) -> bool {
                    let hook = *$hook.lock().unwrap();
                    hook.is_some_and(|hook| hook(building))
                }
            )*};
        }

        skip_hooks! {
            MIGHT_START_USE_SOUND / might_start_use_sound;
            MIGHT_START_USE_ANIMATION / might_start_use_animation;
        }

        macro_rules! value_hooks {
            ($($hook:ident / $function:ident -> $value:ty;)*) => {$(
                /// `(building)`; `None` runs the method.
                pub(crate) static $hook: Mutex<Option<fn(u32) -> Option<$value>>> = Mutex::new(None);

                pub(in crate::ztbuilding) fn $function(building: u32) -> Option<$value> {
                    let hook = *$hook.lock().unwrap();
                    hook.and_then(|hook| hook(building))
                }
            )*};
        }

        /// `(building)`: the show a building can see, in place of `ZTHabitatMgr::canSeeShowFromBuilding`'s answer.
        pub(crate) static SEEN_SHOW: Mutex<Option<fn(u32) -> Option<u32>>> = Mutex::new(None);

        pub(crate) fn seen_show(building: u32) -> Option<u32> {
            let hook = *SEEN_SHOW.lock().unwrap();
            hook.and_then(|hook| hook(building))
        }

        value_hooks! {
            GET_RETURN_BS / get_return_bs -> Vec<u8>;
            CALCULATE_WATER_CAPACITY / calculate_water_capacity -> i32;
        }

        macro_rules! answer_hooks {
            ($($hook:ident / $function:ident($($argument:ident: $type:ty),*);)*) => {$(
                pub(crate) static $hook: Mutex<Option<fn($($type),*) -> Option<bool>>> = Mutex::new(None);

                pub(in crate::ztbuilding) fn $function($($argument: $type),*) -> Option<bool> {
                    let hook = *$hook.lock().unwrap();
                    hook.and_then(|hook| hook($($argument),*))
                }
            )*};
        }

        // `(building, unit, ...)`; `None` runs the method.
        answer_hooks! {
            VALID_EXIT / valid_exit(building: u32, unit: u32, slot: u32);
            EXIT_VIA_EXIT / exit_via_exit(building: u32, unit: u32, slot: u32);
            FIND_NEAREST_OPEN_SLOT / find_nearest_open_slot(building: u32, unit: u32, slot_out: u32, position_out: u32);
            CAN_ADD_USER / can_add_user(building: u32, unit: u32, slot: u32);
            ADD_USER / add_user(building: u32, unit: u32, slot: u32);
            REMOVE_USER / remove_user(building: u32, unit: u32, eject: bool);
            REMOVE_USER_IF_PRESENT / remove_user_if_present(building: u32, unit: u32);
            YANK_USER_IF_PRESENT / yank_user_if_present(building: u32, unit: u32);
        }
    }

    /// A `ZTBuilding` built by the real vanilla constructor over `building_type` (a `ZTBuildingType` pointer), in memory
    /// zeroed first. It is not on the map and has no slots until `initStatusVars` runs.
    pub fn build_standalone_building(building_type: u32) -> ZTBuilding {
        let memory = unsafe { OPERATOR_NEW.original()(BUILDING_SIZE) } as u32;
        unsafe {
            std::ptr::write_bytes(memory as *mut u8, 0, BUILDING_SIZE as usize);
        }
        lifecycle_real::construct(memory, building_type);
        ZTBuilding(memory)
    }

    /// Tears a building down through the real scalar-deleting destructor, which also frees the memory.
    pub fn destroy_standalone_building(building: ZTBuilding) {
        lifecycle_real::delete(building.0);
    }
}
