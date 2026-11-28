//! `ZTBuilding`'s users and slots: admission, entry and exit, the per-slot draw users, what a guest buys or drops off
//! inside, and the animations and shows that follow the user count.
//!
//! These methods re-enter building code: the unit slots they call can add or remove users, and `ZTBuilding::draw` calls
//! `yankUser` while it walks the draw users. Every vector header is read again after a call.
//!
//! Where vanilla faults or never returns, the port does this instead:
//! - a null unit, a unit or building without a type, or a failed cast to `ZTBuildingType`: the method answers `false`
//!   (or does nothing);
//! - `removeAllUsers` over a user id with no entity: the entry is dropped (vanilla loops for ever);
//! - `addDrawUser` with no index given and no free entry: nobody is placed (vanilla loops for ever);
//! - `userDataChanged` when the type has no animation of that name: nothing changes.

use std::{
    ffi::c_void,
    mem::{offset_of, size_of},
};

use glam::IVec3;
use openzt_detour::{
    generated::{
        bfentity::{GET_TILE as BFENTITY_GET_TILE, SET_DIRECTION, SET_WORLD_POS},
        bfentitytype::FIND_ANIM,
        bfmap::WORLD_TO_VIRTUAL_0 as WORLD_TO_VIRTUAL,
        bfunit::FACE_TOWARD_ENTITY,
        poolalloc::DEALLOCATE as POOLALLOC_DEALLOCATE,
        ztbuilding::{
            ADD_USER, BUY_ITEM, CAN_ADD_USER, EMPTY_GRANDSTANDS, GET_FUN_GIFT_IF_AVAILABLE, HAS_OPEN_SLOT, REMOVE_ALL_USERS, REMOVE_ITEMS,
            REMOVE_USER, REMOVE_USER_IF_PRESENT, USER_DATA_CHANGED, YANK_USER, YANK_USER_IF_PRESENT,
        },
        ztbuilding_slot::{ADD_DRAW_USER, CLEAR_DRAW_USERS, GET_TOP_USER, REMOVE_DRAW_USER, SET_TOP_USER},
        ztbuildingtype_slottype::CAN_ADD,
        ztguest::THROW_TRASH,
        ztitem::GET_ITEM as ZTITEM_GET_ITEM,
        ztshow::ABORT_SHOW,
        ztworldmgr::{GET_NEED_FROM_INDEX, SATISFIES},
    },
};
use openzt_detour_macro::detour_mod;

use super::{building_offsets, ZTBuilding, ZTBuildingSlot, ENTITY_ID, ENTITY_TYPE};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    vanilla_string::{c_string_bytes, object_bytes, VanillaString},
    vanilla_vector::{erase_element, push_pair},
    zoostatus::ZooStatus,
    ztgame::mgr::ZTGameMgr,
    zthabitat::{next_game_random, ZTHabitatMgr},
    ztguest::ZTGuest,
    ztmegatilemgr::entity_type_matches,
    ztshow::{
        info::{increment_attendance, increment_receipts},
        mgr::ZTShowMgr,
    },
};

/// `isCastClass` tags, as RVAs.
pub(super) const GUEST_TAG_RVA: u32 = 0x0023_8700;
pub(super) const ANIMAL_TAG_RVA: u32 = 0x0023_8690;
pub(super) const UNIT_TAG_RVA: u32 = 0x0023_86b0;
const SCENERY_TAG_RVA: u32 = 0x0023_8670;

/// Need-name `string` objects, as RVAs: the grandstand need and the fun-gift need.
const GRANDSTAND_NEED_RVA: u32 = 0x0023_af80;
const GIFT_NEED_RVA: u32 = 0x0023_b040;
/// The empty C string a leaving guest's return-building name is reset to.
const EMPTY_TEXT_RVA: u32 = 0x0023_8084;
/// `char*[3]`: the fill-level animation names of a trash can.
const TRASH_ANIM_NAMES_RVA: u32 = 0x0023_b168;
/// A byte that doubles a building's income when set.
const DOUBLE_INCOME_RVA: u32 = 0x0023_aff0;

/// `ZTGameMgr` fields: the multiples of the default price at which a guest finds a building cheap or dear.
const CHEAP_FACTOR: u32 = 0x114;
const DEAR_FACTOR: u32 = 0x118;
/// The `ZooStatus` embedded in `ZTGameMgr`.
const ZOO_STATUS: u32 = 0x10;
/// The `ZTAIMgr` clock a unit's use time is stamped from.
const AI_CLOCK: u32 = 0xec;

/// `ZTBuildingType` fields not in the struct: the redraw-on-leaving flag, and the `char*` vectors of the items sold and
/// the items taken from a guest.
const TYPE_REDRAW_ON_LEAVE: u32 = 0x23c;
const TYPE_ITEMS_SOLD: u32 = 0x200;
const TYPE_ITEMS_TAKEN: u32 = 0x20c;

/// `BFEntity` / unit fields.
const ENTITY_FLAG: u32 = 0x11;
const ENTITY_REDRAW: u32 = 0xd6;
const UNIT_VIRTUAL_POS: u32 = 0xb4;
const UNIT_USE_TIME: u32 = 0x138;
const UNIT_OFF_SLOT: u32 = 0x140;
const UNIT_PLACING: u32 = 0x142;
const UNIT_GOAL: u32 = 0x170;
const GUEST_NEED_INDEX: u32 = 0x24e;
/// The goals a unit has while it uses a building.
pub(super) const BUILDING_USE_GOALS: [u32; 3] = [0x68, 0x69, 0x6a];

/// Vtable slots.
mod slots {
    /// `ZTBuilding`: `sendEvent`.
    pub const SEND_EVENT: u32 = 0x0;
    /// Units: placement around a move into a slot.
    pub const BEFORE_PLACE: u32 = 0x78;
    pub const AFTER_PLACE: u32 = 0x74;
    /// Units: `setVisible(bool)`.
    pub const SET_VISIBLE: u32 = 0xcc;
    pub const IS_RETURNING: u32 = 0x264;
    pub const SET_RETURNING: u32 = 0x274;
    pub const SET_RETURN_BUILDING_ID: u32 = 0x294;
    pub const GET_RETURN_BUILDING_ID: u32 = 0x298;
    pub const SET_RETURN_BUILDING_NAME: u32 = 0x29c;
    /// Units: the step `removeUser` runs when it makes the unit walk out.
    pub const WALK_OUT: u32 = 0x2c0;
    /// Types: type id, and the thought a guest has on using the scenery.
    pub const TYPE_ID: u32 = 0x20;
    pub const TYPE_THOUGHT: u32 = 0x54;
    /// `BFWorldMgr::getEntity(id, bool)`.
    pub const WORLD_GET_ENTITY: u32 = 0x24;
}

/// Type id of the trash can, the item id of trash, and the fill level at which a can takes no more and the one at which
/// it calls for a keeper.
pub(super) const TRASH_CAN_TYPE_ID: u32 = 0x1779;
const TRASH_ITEM_ID: i32 = 0x18a6;
const TRASH_CAN_FULL: i32 = 0x19;
const TRASH_CAN_NOTICE: i32 = 0xd;
/// The event a filling trash can sends, with its tag.
const TRASH_EVENT: u16 = 0x272e;
const TRASH_EVENT_FLAG: u8 = 0x4d;
/// Guest thoughts about the price.
const THOUGHT_CHEAP: u32 = 0x2758;
const THOUGHT_DEAR: u32 = 0x2759;

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

pub(super) fn word(address: u32) -> u32 {
    get_from_memory(address)
}

/// The virtual method at `slot` of `object`'s vtable, as the function type `F`.
///
/// # Safety
/// `F` must be the slot's real signature.
pub(super) unsafe fn virtual_method<F: Copy>(object: u32, slot: u32) -> F {
    let address = word(word(object) + slot);
    unsafe { std::mem::transmute_copy::<u32, F>(&address) }
}

pub(super) fn call0(object: u32, slot: u32) -> u32 {
    unsafe { virtual_method::<extern "thiscall" fn(u32) -> u32>(object, slot)(object) }
}

pub(super) fn call1(object: u32, slot: u32, argument: u32) {
    unsafe { virtual_method::<extern "thiscall" fn(u32, u32)>(object, slot)(object, argument) }
}

fn is_returning(unit: u32) -> bool {
    low_byte_bool(call0(unit, slots::IS_RETURNING))
}

/// `unit` when its type passes the class test, else `0`.
pub(super) fn cast(unit: u32, tag_rva: u32) -> u32 {
    if unit != 0 && unsafe { entity_type_matches(unit, tag_rva) } {
        unit
    } else {
        0
    }
}

pub(super) fn world_entity(world: u32, id: u32) -> u32 {
    unsafe { virtual_method::<extern "thiscall" fn(u32, u32, u32) -> u32>(world, slots::WORLD_GET_ENTITY)(world, id, 1) }
}

/// `ZTWorldMgr::satisfies(building, need, 0)` for the need-name string at `need_rva`.
fn satisfies(building: u32, need_rva: u32) -> bool {
    low_byte_bool(unsafe { SATISFIES.hooked()(building as i32, (base() + need_rva) as *const i8, std::ptr::null()) })
}

fn guest_thought(guest: u32, thought: u32, subject: u32) {
    ZTGuest(guest).guest_thought(thought, subject, 0);
}

/// Moves a unit to `position` facing `facing`, as a slot places its user.
fn place_unit(unit: u32, position: [i32; 3], facing: u32) {
    save_to_memory(unit + UNIT_PLACING, 1u8);
    call1(unit, slots::BEFORE_PLACE, 0);
    let map = globals().ztworldmgr_ptr() as u32 + 8;
    let mut virtual_position = [0i32; 3];
    unsafe { WORLD_TO_VIRTUAL.hooked()(map as *const u32, virtual_position.as_mut_ptr(), position.as_ptr()) };
    for (index, value) in virtual_position.into_iter().enumerate() {
        save_to_memory(unit + UNIT_VIRTUAL_POS + index as u32 * 4, value);
    }
    let position = IVec3::from_array(position);
    unsafe {
        SET_WORLD_POS.hooked()(unit as *const u32, &position);
        SET_DIRECTION.hooked()(unit as *const u32, facing);
    }
    call0(unit, slots::AFTER_PLACE);
    save_to_memory(unit + UNIT_PLACING, 0u8);
}

/// The item record named by the `index`th string of the `char*` vector at `vector`, `0` for an unknown name.
fn item_named_at(vector: u32, index: u32) -> u32 {
    let text = word(word(vector) + index * 4);
    let name = VanillaString::from_bytes(&c_string_bytes(text));
    unsafe { ZTITEM_GET_ITEM.hooked()(name.as_ptr() as *const u8) }
}

/// A random one of the `count` items in the `char*` vector at `vector`, drawn from the game's `rand` state.
fn random_item(vector: u32, count: u32) -> u32 {
    item_named_at(vector, next_game_random() % count)
}

fn vector_len(vector: u32, element_size: u32) -> u32 {
    (word(vector + 4).wrapping_sub(word(vector)) as i32 / element_size as i32) as u32
}

/// The `ZTGameMgr` at `game`.
fn game_mgr<'a>(game: u32) -> &'a ZTGameMgr {
    unsafe { ref_from_memory(game as *const u32) }
}

/// The `ZooStatus` embedded in the `ZTGameMgr` at `game`.
fn zoo_status<'a>(game: u32) -> &'a ZooStatus {
    unsafe { ref_from_memory((game + ZOO_STATUS) as *const u32) }
}

/// One slot of a building (`ZTBuilding::Slot`), addressed by its raw pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTSlot(pub u32);

impl ZTSlot {
    fn read<T>(self, offset: usize) -> T {
        get_from_memory(self.0 + offset as u32)
    }

    fn write<T>(self, offset: usize, value: T) {
        save_to_memory(self.0 + offset as u32, value);
    }

    /// The `{begin, end}` of the draw-user entries.
    pub(super) fn draw_users(self) -> (u32, u32) {
        let offset = offset_of!(ZTBuildingSlot, draw_users);
        (self.read(offset), self.read(offset + 4))
    }

    /// `ZTBuilding::Slot::removeDrawUser` (`0x0042dcd7`): clears the first entry holding `unit`.
    pub fn remove_draw_user(self, unit: u32) {
        let (begin, end) = self.draw_users();
        if let Some(entry) = (begin..end).step_by(4).find(|&entry| word(entry) == unit) {
            save_to_memory(entry, 0u32);
        }
    }

    /// `ZTBuilding::Slot::clearDrawUsers` (`0x00450f08`).
    pub fn clear_draw_users(self) {
        let (begin, end) = self.draw_users();
        for entry in (begin..end).step_by(4) {
            save_to_memory(entry, 0u32);
        }
    }

    /// `ZTBuilding::Slot::getTopUser` (`0x004a7412`): the top user, and whether it may be drawn there: `false` only for a
    /// unit that no longer has a building-use goal.
    pub fn top_user(self) -> (u32, bool) {
        let top: u32 = self.read(offset_of!(ZTBuildingSlot, top_user));
        let stale = top != 0 && cast(top, UNIT_TAG_RVA) != 0 && !BUILDING_USE_GOALS.contains(&word(top + UNIT_GOAL));
        (top, !stale)
    }

    /// `ZTBuilding::Slot::setTopUser` (`0x005ac37c`): takes `unit` when it is a unit with a building-use goal.
    pub fn set_top_user(self, unit: u32) -> bool {
        if cast(unit, UNIT_TAG_RVA) == 0 || !BUILDING_USE_GOALS.contains(&word(unit + UNIT_GOAL)) {
            return false;
        }
        self.write(offset_of!(ZTBuildingSlot, top_user), unit);
        true
    }

    /// `ZTBuilding::Slot::addDrawUser` (`0x005a93fe`): places `unit` at draw position `index` of the slot and records it
    /// there. An index of `-1` draws random entries until a free one comes up.
    pub fn add_draw_user(self, unit: u32, mut index: i32) {
        if unit == 0 {
            return;
        }
        if index == -1 {
            let count: i32 = self.read(offset_of!(ZTBuildingSlot, draw_slot_count));
            let begin = self.draw_users().0;
            if !(0..count).any(|entry| word(begin + entry as u32 * 4) == 0) {
                return;
            }
            loop {
                index = next_game_random() as i32 % count;
                if word(self.draw_users().0 + index as u32 * 4) == 0 {
                    break;
                }
            }
        }
        save_to_memory(unit + UNIT_OFF_SLOT, 0u8);
        let building: u32 = self.read(offset_of!(ZTBuildingSlot, building));
        let slot: u32 = self.read(offset_of!(ZTBuildingSlot, index));
        let building = ZTBuilding(building);
        let position = building.slot_draw_loc(slot, index);
        let facing = building.slot_facing(self.read(offset_of!(ZTBuildingSlot, index)));
        place_unit(unit, position, facing);
        save_to_memory(self.draw_users().0 + index as u32 * 4, unit);
    }
}

/// `ZTBuildingType::SlotType::canAdd` (`0x0042ae68`): whether the slot record at `record` admits `unit`: its type id is
/// listed (or the list is empty), it is returning exactly when this is a return slot, and its type name is not found in
/// the record's filter string.
pub fn slot_type_can_add(record: u32, unit_type: u32, unit: u32) -> bool {
    if unit_type == 0 || unit == 0 {
        return false;
    }
    let ids = record + offset_of!(ZTBuildingSlotType, user_type_ids) as u32;
    let (begin, end) = (word(ids), word(ids + 4));
    if begin != end {
        let id = call0(unit_type, slots::TYPE_ID);
        if !(begin..end).step_by(4).any(|entry| word(entry) == id) {
            return false;
        }
    }
    let is_return_slot: u8 = get_from_memory(record + offset_of!(ZTBuildingSlotType, is_return_slot) as u32);
    if is_returning(unit) != (is_return_slot != 0) {
        return false;
    }
    let filter = word(record + offset_of!(ZTBuildingSlotType, filter) as u32);
    if filter == 0 {
        return true;
    }
    let name = c_string_bytes(word(unit_type + 0xa4));
    !name.is_empty() && !c_string_bytes(filter).windows(name.len()).any(|window| window == name)
}

impl ZTBuilding {
    /// The slot at `index`. The index must be below [`Self::slot_count`].
    pub(super) fn slot(self, index: u32) -> ZTSlot {
        let begin: u32 = self.read(building_offsets::SLOTS);
        ZTSlot(begin.wrapping_add(index.wrapping_mul(size_of::<ZTBuildingSlot>() as u32)))
    }

    /// The `{begin, end}` of the users vector of `{unit id, slot index}` pairs.
    fn users(self) -> (u32, u32) {
        (self.read(building_offsets::USERS), self.read(building_offsets::USERS + 4))
    }

    /// The user entry holding `id`.
    fn find_user(self, id: u32) -> Option<u32> {
        let (begin, end) = self.users();
        (begin..end).step_by(8).find(|&entry| word(entry) == id)
    }

    /// The type when it casts to `ZTBuildingType`, else `0`.
    pub(super) fn building_type(self) -> u32 {
        if self.is_building_type() {
            self.entity_type()
        } else {
            0
        }
    }

    fn type_byte(self, offset: usize) -> bool {
        self.type_field::<u8>(offset) != 0
    }

    pub(super) fn type_id(self) -> u32 {
        call0(self.entity_type(), slots::TYPE_ID)
    }

    /// `ZTBuilding::canAddUser` (`0x0042ac87`): whether `unit` may enter slot `slot`.
    pub fn can_add_user(self, unit: u32, slot: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::can_add_user(self.0, unit, slot) {
            return answer;
        }
        let building_type = self.entity_type();
        if unit == 0 || building_type == 0 || self.read::<u8>(ENTITY_FLAG) != 0 {
            return false;
        }
        if self.occupancy() >= self.type_field(offset_of!(ZTBuildingType, i_capacity)) {
            return false;
        }
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        if slot >= self.slot_count() as u32 || slot >= vector_len(records, size_of::<ZTBuildingSlotType>() as u32) {
            return false;
        }
        let record = word(records) + slot * size_of::<ZTBuildingSlotType>() as u32;
        let capacity: i32 = get_from_memory(record + offset_of!(ZTBuildingSlotType, capacity) as u32);
        if self.slot(slot).read::<i32>(offset_of!(ZTBuildingSlot, user_count)) >= capacity {
            return false;
        }
        let unit_type = word(unit + ENTITY_TYPE);
        if cast(unit, UNIT_TAG_RVA) == 0 {
            return false;
        }
        let guest = cast(unit, GUEST_TAG_RVA);
        if guest != 0 {
            let need = VanillaString::rvo_target();
            unsafe { GET_NEED_FROM_INDEX.hooked()(need.as_ptr(), get_from_memory(guest + GUEST_NEED_INDEX)) };
            if need.bytes() == object_bytes(base() + GRANDSTAND_NEED_RVA) && self.type_state() == 0 {
                return false;
            }
        }
        if !slot_type_can_add(record, unit_type, unit) {
            return false;
        }
        if !self.valid_exit(&mut [0; 3], unit, slot) {
            return false;
        }
        if !self.type_byte(offset_of!(ZTBuildingType, return_building)) || guest == 0 {
            return true;
        }
        if !is_returning(guest) {
            let members = self.read::<u32>(building_offsets::MEMBERS + 4).wrapping_sub(self.read(building_offsets::MEMBERS)) as i32 >> 2;
            if members >= self.read::<i32>(building_offsets::WATER_CAPACITY) {
                return false;
            }
            let strict: i32 = self.type_field(offset_of!(ZTBuildingType, strict_building_capacity));
            if strict != 0 && self.occupancy().wrapping_add(1) > strict {
                return false;
            }
        }
        if !is_returning(guest) {
            return true;
        }
        if self.type_id() != call0(guest, slots::GET_RETURN_BUILDING_ID) {
            return false;
        }
        if !self.type_byte(offset_of!(ZTBuildingType, user_tracker)) {
            return true;
        }
        let id = word(unit + ENTITY_ID);
        let (begin, end): (u32, u32) = (self.read(building_offsets::MEMBERS), self.read(building_offsets::MEMBERS + 4));
        (begin..end).step_by(4).any(|member| word(member) == id)
    }

    /// `ZTBuilding::hasOpenSlot` (`0x00429330`): whether some slot would take `unit`.
    pub fn has_open_slot(self, unit: u32) -> bool {
        (0..self.slot_count() as u32).any(|slot| self.can_add_user(unit, slot))
    }

    /// `ZTBuilding::addUser` (`0x0042ce0a`): takes `unit` into slot `slot`: counts it, places it (hidden, facing the
    /// building or at the slot, as the type says), lets a visiting guest shop, and credits an associated show.
    pub fn add_user(self, unit: u32, slot: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::add_user(self.0, unit, slot) {
            return answer;
        }
        if !self.can_add_user(unit, slot) {
            return false;
        }
        self.write(building_offsets::OCCUPANCY, self.occupancy().wrapping_add(1));
        let guest = cast(unit, GUEST_TAG_RVA);
        if guest != 0 && !is_returning(unit) {
            self.write(building_offsets::TOTAL_USERS, self.total_users().wrapping_add(1));
            self.write(building_offsets::THIS_MONTH_USERS, self.this_month_users().wrapping_add(1));
        }
        self.might_start_use_sound();
        self.might_start_use_animation();
        push_pair(self.0 + building_offsets::USERS, word(unit + ENTITY_ID), slot, |begin, bytes| {
            if begin != 0 {
                unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) }
            }
        });
        if self.slot(slot).read::<u8>(offset_of!(ZTBuildingSlot, has_draw_slots)) != 0 {
            self.slot(slot).add_draw_user(unit, -1);
        }
        let entry = self.slot(slot);
        let count: i32 = entry.read(offset_of!(ZTBuildingSlot, user_count));
        entry.write(offset_of!(ZTBuildingSlot, user_count), count.wrapping_add(1));
        entry.write(offset_of!(ZTBuildingSlot, top_user), unit);
        let ai = globals().ztaimgr_ptr() as u32;
        if ai != 0 {
            save_to_memory(unit + UNIT_USE_TIME, word(ai + AI_CLOCK));
        }
        if self.type_byte(offset_of!(ZTBuildingType, hide_user)) {
            call1(unit, slots::SET_VISIBLE, 0);
        } else if !self.type_byte(offset_of!(ZTBuildingType, user_stays_outside)) {
            if self.slot_has_draw_slots(slot) == 0 {
                place_unit(unit, self.slot_loc(slot), self.slot_facing(slot));
            }
        } else if self.type_byte(offset_of!(ZTBuildingType, set_letter_facing)) {
            unsafe { FACE_TOWARD_ENTITY.hooked()(cast(unit, UNIT_TAG_RVA) as *const u32, self.0 as i32) };
        }
        if guest != 0 {
            if !is_returning(guest) {
                self.serve_guest(guest);
            }
            if is_returning(guest) {
                self.remove_member(unit);
            }
        }
        self.credit_show();
        true
    }

    /// What a building does for a guest that is not returning to it: the guest's thought about it, the items it takes
    /// from them, and a purchase or a gift.
    fn serve_guest(self, guest: u32) {
        let building_type = self.entity_type();
        if building_type != 0 && unsafe { entity_type_matches(self.0, SCENERY_TAG_RVA) } {
            let thought = call0(building_type, slots::TYPE_THOUGHT);
            if thought != 0 {
                guest_thought(guest, thought, 0);
            }
        }
        self.remove_items(guest);
        if satisfies(self.0, GIFT_NEED_RVA) {
            self.get_fun_gift_if_available(guest);
        } else {
            self.buy_item(guest);
        }
    }

    /// Adds one admission at the building's price to the associated show and the zoo.
    fn credit_show(self) {
        let show: u16 = self.show_id();
        if show == 0 {
            return;
        }
        let info = ZTShowMgr::get_show_info(show);
        if info == 0 || self.building_type() == 0 {
            return;
        }
        let amount = (f64::from(self.type_field::<f32>(offset_of!(ZTBuildingType, price_factor))) * f64::from(self.price())) as f32;
        increment_receipts(info, amount);
        increment_attendance(info, 1);
        let game = globals().ztgamemgr_ptr() as u32;
        if game != 0 {
            zoo_status(game).increase_show_admission(amount);
            game_mgr(game).add_cash(amount);
        }
    }

    /// `ZTBuilding::yankUser` (`0x0042da64`): takes `unit` out of the user list without any of leaving's effects, and
    /// answers the slot it was in (`-1` when it was in none), or `None` for a null unit. The occupancy drops either way.
    /// When it was a slot's top user, the next user of that slot that qualifies takes its place; one that does not is
    /// yanked too.
    pub fn yank_user(self, unit: u32) -> Option<i32> {
        if unit == 0 {
            return None;
        }
        save_to_memory(unit + UNIT_OFF_SLOT, 1u8);
        let mut occupancy = self.occupancy().wrapping_sub(1);
        self.write(building_offsets::OCCUPANCY, occupancy);
        let (begin, end) = self.users();
        let others = (end.wrapping_sub(begin) as i32 >> 3) - 1;
        if occupancy < others {
            occupancy = others.max(0);
            self.write(building_offsets::OCCUPANCY, occupancy);
        }
        self.might_start_idle_animation();
        let id = word(unit + ENTITY_ID);
        let (begin, end) = self.users();
        let found = self.find_user(id);
        let count = end.wrapping_sub(begin) as i32 >> 3;
        let Some(found) = found else {
            return Some(-1);
        };
        let mut check_next = (count as u32) > 1;
        let mut replaced = count == 0;
        let slot_index = word(found + 4);
        let slot = self.slot(slot_index);
        let remaining = slot.read::<i32>(offset_of!(ZTBuildingSlot, user_count)).wrapping_sub(1);
        slot.write(offset_of!(ZTBuildingSlot, user_count), remaining);
        let was_top = if remaining == 0 {
            slot.write(offset_of!(ZTBuildingSlot, top_user), 0u32);
            false
        } else {
            unit == slot.read(offset_of!(ZTBuildingSlot, top_user))
        };
        let mut next = 0;
        if was_top {
            let world = globals().ztworldmgr_ptr() as u32;
            let mut entry = self.users().0;
            while entry != self.users().1 {
                if word(entry + 4) == slot_index {
                    next = world_entity(world, word(entry));
                    if next != 0 {
                        replaced = self.slot(slot_index).set_top_user(next);
                        break;
                    }
                }
                entry += 8;
            }
        } else {
            check_next = false;
        }
        erase_element(self.0 + building_offsets::USERS, found, 8);
        self.slot(slot_index).remove_draw_user(unit);
        if check_next && !replaced {
            self.yank_user(next);
        }
        Some(slot_index as i32)
    }

    /// `ZTBuilding::yankUserIfPresent` (`0x00611a98`).
    pub fn yank_user_if_present(self, unit: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::yank_user_if_present(self.0, unit) {
            return answer;
        }
        unit != 0 && self.find_user(word(unit + ENTITY_ID)).is_some() && self.yank_user(unit).is_some()
    }

    /// `ZTBuilding::removeUserIfPresent` (`0x0059be7b`).
    pub fn remove_user_if_present(self, unit: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::remove_user_if_present(self.0, unit) {
            return answer;
        }
        unit != 0 && self.find_user(word(unit + ENTITY_ID)).is_some() && self.remove_user(unit, false)
    }

    /// `ZTBuilding::removeUser` (`0x0042d858`): lets `unit` leave. A commerce building takes its price and a guest reacts
    /// to it; guests and animals get the building's need changes; the unit is walked out when `eject` is set; and a
    /// return building (a rental or a ride) either sends the guest off with it or takes it back.
    pub fn remove_user(self, unit: u32, eject: bool) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::remove_user(self.0, unit, eject) {
            return answer;
        }
        let Some(slot) = self.yank_user(unit) else {
            return false;
        };
        let guest = cast(unit, GUEST_TAG_RVA);
        if self.entity_type() == 0 {
            return true;
        }
        if self.type_byte(offset_of!(ZTBuildingType, commerce)) && self.building_type() != 0 {
            self.take_payment(guest);
        }
        let animal = cast(unit, ANIMAL_TAG_RVA);
        let unit_as_unit = cast(unit, UNIT_TAG_RVA);
        if guest != 0 || animal != 0 {
            let returns = self.building_type() != 0 && self.type_byte(offset_of!(ZTBuildingType, return_building));
            if !returns || (unit_as_unit != 0 && is_returning(unit_as_unit)) {
                self.do_changes(unit_as_unit);
            }
        }
        if unit_as_unit != 0 && eject {
            call0(unit_as_unit, slots::WALK_OUT);
        }
        if get_from_memory::<u8>(self.entity_type() + TYPE_REDRAW_ON_LEAVE) != 0 {
            self.write(ENTITY_REDRAW, 1u8);
        }
        if self.type_byte(offset_of!(ZTBuildingType, user_uses_exit)) || eject {
            self.exit_via_exit(unit, slot as u32);
        } else if self.type_byte(offset_of!(ZTBuildingType, hide_user)) {
            call1(unit, slots::SET_VISIBLE, 1);
        }
        if guest != 0 && self.building_type() != 0 && self.type_byte(offset_of!(ZTBuildingType, return_building)) {
            if !is_returning(guest) && !eject {
                self.send_off_with_return(guest);
            } else {
                call1(guest, slots::SET_RETURN_BUILDING_ID, 0);
                let text = VanillaString::from_bytes(&c_string_bytes(base() + EMPTY_TEXT_RVA));
                call1(guest, slots::SET_RETURN_BUILDING_NAME, text.as_ptr() as u32);
                drop(text);
                call1(guest, slots::SET_RETURNING, 0);
            }
        }
        true
    }

    /// Takes a leaving user's payment in a commerce building. A building that sells no items also draws the guest's
    /// reaction to its price here.
    // The negation is vanilla's: an unordered price comparison counts as dear.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn take_payment(self, guest: u32) {
        let sold = self.entity_type() + TYPE_ITEMS_SOLD;
        let game = globals().ztgamemgr_ptr() as u32;
        if vector_len(sold, 4) == 0 && guest != 0 {
            let default = f64::from(self.type_field::<f32>(offset_of!(ZTBuildingType, default_cost)));
            let price = f64::from(self.price());
            if !(f64::from(get_from_memory::<f32>(game + DEAR_FACTOR)) * default > price) {
                guest_thought(guest, THOUGHT_DEAR, self.0);
                ZTGuest(guest).do_angry_price_change();
            } else if f64::from(get_from_memory::<f32>(game + CHEAP_FACTOR)) * default >= price {
                guest_thought(guest, THOUGHT_CHEAP, self.0);
                ZTGuest(guest).do_happy_price_change();
            }
        }
        let mut income = (f64::from(self.type_field::<f32>(offset_of!(ZTBuildingType, price_factor))) * f64::from(self.price())) as f32;
        if get_from_memory::<u8>(base() + DOUBLE_INCOME_RVA) != 0 {
            income += income;
        }
        for offset in [building_offsets::TOTAL_INCOME, building_offsets::THIS_MONTH_INCOME] {
            let total = (f64::from(income) + f64::from(self.read::<f32>(offset))) as f32;
            self.write(offset, total);
        }
        if self.show_id() == 0 {
            zoo_status(game).buy_people_food(income);
            game_mgr(game).add_cash(income);
        }
    }

    /// Gives a guest leaving a return building what it needs to come back: the building's type id and return behaviour
    /// set, a place among the members when the type tracks its users, and a recount of the water capacity.
    fn send_off_with_return(self, guest: u32) {
        let id = self.type_id();
        call1(guest, slots::SET_RETURN_BUILDING_ID, id);
        let name = VanillaString::from_bytes(&self.return_behavior_set());
        call1(guest, slots::SET_RETURN_BUILDING_NAME, name.as_ptr() as u32);
        drop(name);
        if self.type_byte(offset_of!(ZTBuildingType, user_tracker)) {
            self.add_member(guest);
        }
        let capacity = self.calculate_water_capacity();
        self.write(building_offsets::WATER_CAPACITY, capacity);
    }

    /// `ZTBuilding::removeAllUsers` (`0x005ae745`): ejects the users, first to last.
    pub fn remove_all_users(self) {
        let world = globals().ztworldmgr_ptr() as u32;
        while self.users().0 != self.users().1 {
            let entry = self.users().0;
            let unit = world_entity(world, word(entry));
            if unit != 0 {
                self.remove_user(unit, true);
            } else {
                erase_element(self.0 + building_offsets::USERS, entry, 8);
            }
        }
    }

    /// `ZTBuilding::buyItem` (`0x0042d1e2`): a building that sells nothing satisfies the guest's need with what the guest
    /// carries; otherwise the guest buys one of the items at random, unless the price is too high.
    // The negation is vanilla's: an unordered price comparison counts as cheap.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn buy_item(self, guest: u32) -> bool {
        if guest == 0 || self.building_type() == 0 {
            return false;
        }
        let sold = self.entity_type() + TYPE_ITEMS_SOLD;
        let count = vector_len(sold, 4);
        if count == 0 {
            let item = ZTGuest(guest).get_need_item();
            ZTGuest(guest).consume_item(item);
            return true;
        }
        let item = random_item(sold, count);
        if item == 0 {
            return true;
        }
        let game = globals().ztgamemgr_ptr() as u32;
        let default = f64::from(self.type_field::<f32>(offset_of!(ZTBuildingType, default_cost)));
        let price = f64::from(self.price());
        if price >= f64::from(get_from_memory::<f32>(game + DEAR_FACTOR)) * default {
            guest_thought(guest, THOUGHT_DEAR, self.0);
            ZTGuest(guest).do_angry_price_change();
            return true;
        }
        if !(price > f64::from(get_from_memory::<f32>(game + CHEAP_FACTOR)) * default) {
            guest_thought(guest, THOUGHT_CHEAP, self.0);
            ZTGuest(guest).do_happy_price_change();
        }
        // An empty name makes `addItem` file the item under the guest's current need.
        if ZTGuest(guest).add_item(word(item), 0, 0, 0) && get_from_memory::<u8>(item + 0x1c) != 0 {
            ZTGuest(guest).consume_item(item);
        }
        true
    }

    /// `ZTBuilding::removeItems` (`0x0042d181`): takes from the guest each item the type lists. Trash goes into a trash
    /// can until it is full; the guest throws away what does not fit. `false` when an item is unknown or was not carried.
    pub fn remove_items(self, guest: u32) -> bool {
        if guest == 0 {
            return false;
        }
        let building_type = self.building_type();
        if building_type == 0 {
            return false;
        }
        let taken = building_type + TYPE_ITEMS_TAKEN;
        let mut all = true;
        let mut entry = word(taken);
        while entry != word(taken + 4) {
            let item = item_named_at(taken, (entry - word(taken)) / 4);
            entry += 4;
            if item == 0 {
                all = false;
                continue;
            }
            let id: i32 = get_from_memory(item);
            if id == TRASH_ITEM_ID {
                if self.type_state() >= TRASH_CAN_FULL {
                    unsafe { THROW_TRASH.hooked()(guest as *const u32, std::ptr::null()) };
                    continue;
                }
                self.set_type_state(self.type_state().wrapping_add(1));
                self.user_data_changed();
                if self.type_state() == TRASH_CAN_NOTICE {
                    self.send_trash_event();
                }
            }
            let removed = ZTGuest(guest).remove_item(id);
            all &= removed;
        }
        all
    }

    /// Tells the AI, through the building's own `sendEvent`, that this trash can wants emptying.
    fn send_trash_event(self) {
        let tile = unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) } as u32;
        type SendEvent = extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u32);
        let send = unsafe { virtual_method::<SendEvent>(self.0, slots::SEND_EVENT) };
        send(self.0, u32::from(TRASH_EVENT), 0, u32::from(TRASH_EVENT_FLAG), tile, 0, 1);
    }

    /// `ZTBuilding::getFunGiftIfAvailable` (`0x0059cd02`): in a building that gives gifts, hands the guest one of its
    /// items, filed under the gift need. `false` when the building gives no gifts.
    pub fn get_fun_gift_if_available(self, guest: u32) -> bool {
        if !satisfies(self.0, GIFT_NEED_RVA) || guest == 0 || self.building_type() == 0 {
            return false;
        }
        let sold = self.entity_type() + TYPE_ITEMS_SOLD;
        let count = vector_len(sold, 4);
        if count != 0 {
            let item = random_item(sold, count);
            if item != 0 {
                // `addItem` takes over the name and frees it.
                let [begin, end, capacity] = VanillaString::from_bytes(&object_bytes(base() + GIFT_NEED_RVA)).release();
                ZTGuest(guest).add_item(word(item), begin, end, capacity);
            }
        }
        true
    }

    /// `ZTBuilding::userDataChanged` (`0x00412513`): shows how full a trash can is. Vanilla's test order means the third
    /// fill animation is never chosen.
    pub fn user_data_changed(self) {
        if self.entity_type() == 0 || self.type_id() != TRASH_CAN_TYPE_ID || self.building_type() == 0 {
            return;
        }
        let level = usize::from(self.type_state() >= TRASH_CAN_NOTICE);
        let name = word(base() + TRASH_ANIM_NAMES_RVA + level as u32 * 4);
        let anim = unsafe { FIND_ANIM.hooked()(self.building_type() as *const u32, name as *const i8) } as u32;
        if anim == 0 {
            return;
        }
        self.set_base_animation(anim);
    }

    /// `ZTBuilding::emptyGrandstands` (`0x006127b8`): on a grandstand with a show, aborts the show and empties every
    /// grandstand that watches it.
    pub fn empty_grandstands(self) {
        if !satisfies(self.0, GRANDSTAND_NEED_RVA) {
            return;
        }
        let show = self.show_id();
        let info = ZTShowMgr::get_show_info(show);
        if info == 0 {
            return;
        }
        unsafe { ABORT_SHOW.hooked()((info + 4) as *const u32) };
        self.set_type_state(0);
        self.user_data_changed();
        let habitats = globals().zthabitatmgr_ptr() as u32;
        if habitats != 0 {
            unsafe { ref_from_memory::<ZTHabitatMgr>(habitats as *const u32) }.empty_grandstands(show);
        }
    }
}

#[detour_mod]
mod user_detours {
    use super::*;

    #[detour(CAN_ADD_USER)]
    unsafe extern "thiscall" fn can_add_user(this: *const u32, unit: *const i32, slot: *const i32) -> u32 {
        u32::from(ZTBuilding(this as u32).can_add_user(unit as u32, slot as u32))
    }

    #[detour(HAS_OPEN_SLOT)]
    unsafe extern "thiscall" fn has_open_slot(this: *const u32, unit: *const u32) -> u32 {
        u32::from(ZTBuilding(this as u32).has_open_slot(unit as u32))
    }

    #[detour(ADD_USER)]
    unsafe extern "thiscall" fn add_user(this: *const u32, unit: *const i32, slot: *const i32) -> u32 {
        u32::from(ZTBuilding(this as u32).add_user(unit as u32, slot as u32))
    }

    #[detour(REMOVE_USER)]
    unsafe extern "thiscall" fn remove_user(this: *const u32, unit: *const i32, eject: i8) -> u32 {
        u32::from(ZTBuilding(this as u32).remove_user(unit as u32, eject != 0))
    }

    #[detour(REMOVE_USER_IF_PRESENT)]
    unsafe extern "thiscall" fn remove_user_if_present(this: *const u32, unit: *const i32) -> u32 {
        u32::from(ZTBuilding(this as u32).remove_user_if_present(unit as u32))
    }

    #[detour(YANK_USER)]
    unsafe extern "thiscall" fn yank_user(this: *const u32, unit: i32, slot_out: *const i32) -> u32 {
        match ZTBuilding(this as u32).yank_user(unit as u32) {
            Some(slot) => {
                save_to_memory(slot_out as u32, slot);
                1
            }
            None => 0,
        }
    }

    #[detour(YANK_USER_IF_PRESENT)]
    unsafe extern "thiscall" fn yank_user_if_present(this: *const u32, unit: i32) -> u32 {
        u32::from(ZTBuilding(this as u32).yank_user_if_present(unit as u32))
    }

    #[detour(REMOVE_ALL_USERS)]
    unsafe extern "thiscall" fn remove_all_users(this: *const u32) {
        ZTBuilding(this as u32).remove_all_users();
    }

    #[detour(BUY_ITEM)]
    unsafe extern "thiscall" fn buy_item(this: *const u32, guest: *const i32) -> u32 {
        u32::from(ZTBuilding(this as u32).buy_item(guest as u32))
    }

    #[detour(REMOVE_ITEMS)]
    unsafe extern "thiscall" fn remove_items(this: *const u32, guest: i32) -> u8 {
        u8::from(ZTBuilding(this as u32).remove_items(guest as u32))
    }

    #[detour(GET_FUN_GIFT_IF_AVAILABLE)]
    unsafe extern "thiscall" fn get_fun_gift_if_available(this: *const u32, guest: *const c_void) -> u32 {
        u32::from(ZTBuilding(this as u32).get_fun_gift_if_available(guest as u32))
    }

    #[detour(USER_DATA_CHANGED)]
    unsafe extern "thiscall" fn user_data_changed(this: *const u32) {
        ZTBuilding(this as u32).user_data_changed();
    }

    #[detour(EMPTY_GRANDSTANDS)]
    unsafe extern "thiscall" fn empty_grandstands(this: *const u32) {
        ZTBuilding(this as u32).empty_grandstands();
    }

    #[detour(CAN_ADD)]
    unsafe extern "thiscall" fn can_add(this: *const u32, unit_type: *const u32, unit: *const u32) -> bool {
        slot_type_can_add(this as u32, unit_type as u32, unit as u32)
    }

    #[detour(REMOVE_DRAW_USER)]
    unsafe extern "thiscall" fn remove_draw_user(this: *const u32, unit: i32) {
        ZTSlot(this as u32).remove_draw_user(unit as u32);
    }

    #[detour(CLEAR_DRAW_USERS)]
    unsafe extern "thiscall" fn clear_draw_users(this: *const u32) {
        ZTSlot(this as u32).clear_draw_users();
    }

    #[detour(GET_TOP_USER)]
    unsafe extern "thiscall" fn get_top_user(this: *const u32, out: *const u32) -> u32 {
        let (top, drawable) = ZTSlot(this as u32).top_user();
        save_to_memory(out as u32, top);
        u32::from(drawable)
    }

    #[detour(SET_TOP_USER)]
    unsafe extern "thiscall" fn set_top_user(this: *const u32, unit: i32) -> u32 {
        u32::from(ZTSlot(this as u32).set_top_user(unit as u32))
    }

    #[detour(ADD_DRAW_USER)]
    unsafe extern "thiscall" fn add_draw_user(this: *const u32, unit: *const i32, index: i32) {
        ZTSlot(this as u32).add_draw_user(unit as u32, index);
    }

    /// The vanilla bodies, for the live tests.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn can_add_user(this: u32, unit: u32, slot: u32) -> bool {
            unsafe { super::CAN_ADD_USER_DETOUR.call(this as *const u32, unit as *const i32, slot as *const i32) & 0xff != 0 }
        }

        pub(crate) fn has_open_slot(this: u32, unit: u32) -> bool {
            unsafe { super::HAS_OPEN_SLOT_DETOUR.call(this as *const u32, unit as *const u32) & 0xff != 0 }
        }

        pub(crate) fn add_user(this: u32, unit: u32, slot: u32) -> bool {
            unsafe { super::ADD_USER_DETOUR.call(this as *const u32, unit as *const i32, slot as *const i32) & 0xff != 0 }
        }

        pub(crate) fn remove_user(this: u32, unit: u32, eject: bool) -> bool {
            unsafe { super::REMOVE_USER_DETOUR.call(this as *const u32, unit as *const i32, i8::from(eject)) & 0xff != 0 }
        }

        pub(crate) fn remove_user_if_present(this: u32, unit: u32) -> bool {
            unsafe { super::REMOVE_USER_IF_PRESENT_DETOUR.call(this as *const u32, unit as *const i32) & 0xff != 0 }
        }

        pub(crate) fn yank_user(this: u32, unit: u32) -> Option<i32> {
            let mut slot = 0i32;
            let result = unsafe { super::YANK_USER_DETOUR.call(this as *const u32, unit as i32, &mut slot as *mut i32 as *const i32) };
            (result & 0xff != 0).then_some(slot)
        }

        pub(crate) fn yank_user_if_present(this: u32, unit: u32) -> bool {
            unsafe { super::YANK_USER_IF_PRESENT_DETOUR.call(this as *const u32, unit as i32) & 0xff != 0 }
        }

        pub(crate) fn remove_all_users(this: u32) {
            unsafe { super::REMOVE_ALL_USERS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn buy_item(this: u32, guest: u32) -> bool {
            unsafe { super::BUY_ITEM_DETOUR.call(this as *const u32, guest as *const i32) & 0xff != 0 }
        }

        pub(crate) fn remove_items(this: u32, guest: u32) -> bool {
            unsafe { super::REMOVE_ITEMS_DETOUR.call(this as *const u32, guest as i32) != 0 }
        }

        pub(crate) fn get_fun_gift_if_available(this: u32, guest: u32) -> bool {
            unsafe { super::GET_FUN_GIFT_IF_AVAILABLE_DETOUR.call(this as *const u32, guest as *const c_void) & 0xff != 0 }
        }

        pub(crate) fn user_data_changed(this: u32) {
            unsafe { super::USER_DATA_CHANGED_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn empty_grandstands(this: u32) {
            unsafe { super::EMPTY_GRANDSTANDS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn slot_type_can_add(record: u32, unit_type: u32, unit: u32) -> bool {
            unsafe { super::CAN_ADD_DETOUR.call(record as *const u32, unit_type as *const u32, unit as *const u32) }
        }

        pub(crate) fn remove_draw_user(slot: u32, unit: u32) {
            unsafe { super::REMOVE_DRAW_USER_DETOUR.call(slot as *const u32, unit as i32) }
        }

        pub(crate) fn clear_draw_users(slot: u32) {
            unsafe { super::CLEAR_DRAW_USERS_DETOUR.call(slot as *const u32) }
        }

        pub(crate) fn get_top_user(slot: u32) -> (u32, bool) {
            let mut top = 0u32;
            let result = unsafe { super::GET_TOP_USER_DETOUR.call(slot as *const u32, &mut top as *mut u32 as *const u32) };
            (top, result & 0xff != 0)
        }

        pub(crate) fn set_top_user(slot: u32, unit: u32) -> bool {
            unsafe { super::SET_TOP_USER_DETOUR.call(slot as *const u32, unit as i32) & 0xff != 0 }
        }

        pub(crate) fn add_draw_user(slot: u32, unit: u32, index: i32) {
            unsafe { super::ADD_DRAW_USER_DETOUR.call(slot as *const u32, unit as *const i32, index) }
        }
    }
}

/// Installs the user and slot detours.
pub(super) fn init() {
    if let Err(e) = unsafe { user_detours::init_detours() } {
        tracing::error!("Failed to initialise ztbuilding user detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    user_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use user_detours::test_real;
