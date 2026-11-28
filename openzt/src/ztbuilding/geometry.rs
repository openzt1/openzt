//! `ZTBuilding`'s slot geometry and exits: the slot records' positions turned into map positions, where a leaving unit
//! may be put down, and the search for the nearest slot a unit can walk to.
//!
//! A slot record's positions are relative to the building and turn with it. The `ZTBuildingType` getters vanilla calls
//! for them (`getSlotLoc`, `getSlotExit`, `getSlotDrawLoc`, `getSlotEntrances`) have no other caller, so they are read
//! here and not detoured.
//!
//! Where vanilla faults, the port does this instead:
//! - `findNearestOpenSlot` for a null unit: nothing is found;
//! - a building without a type: positions are the record `(0, 0, 0)`, there are no entrances, and no exit or slot is
//!   found;
//! - a draw index past the slot's draw positions: the record position `(0, 0, 0)`;
//! - an exit, or the step inside from it, off the map: the exit counts as blocked, so the entrance is tried.

use std::mem::{offset_of, size_of};

use glam::IVec3;
use openzt_detour::{
    generated::{
        bfentity::{GET_GRID_POS, GET_INSIDE_POS},
        bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION, MOVE_ENTITY},
        bftile::{IS_CLEAR_FOR_WALK_BY, IS_IN_ZOO},
        msvc_std::RAND,
        msvc_std_dequegxmixerchain::CLEAR_0X41BF8F as MIXER_CHAIN_DEQUE_CLEAR,
        poolalloc::ALLOCATE as POOLALLOC_ALLOCATE,
        ztbuilding::{
            EXIT_VIA_EXIT, FIND_NEAREST_OPEN_SLOT, GET_SLOT_DRAW_LOC, GET_SLOT_ENTRANCES, GET_SLOT_EXIT, GET_SLOT_FACING,
            GET_SLOT_HAS_DRAW_SLOTS, GET_SLOT_LOC, GET_VALID_ENTRANCE_FOR_EXIT, GET_VALID_EXIT, GX_POS_FROM_XYPOS_0 as GX_POS_FROM_XY_POS, GX_POS_FROM_XYPOS_1, IS_VALID_EXIT,
        },
    },
};
use openzt_detour_macro::detour_mod;

use super::{
    users::{cast, ANIMAL_TAG_RVA, GUEST_TAG_RVA, UNIT_TAG_RVA},
    ZTBuilding,
};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, save_to_memory, world_to_tile},
    ztaimgr::target_list::{TargetList, VanillaPool},
    ztunit::map_tile,
};


/// `BFEntity` position and rotation.
const ENTITY_POS: u32 = 0x114;
const ENTITY_ROTATION: u32 = 0x12c;
/// The byte `BFEntity` count a steep tile refuses an animal at, and the unit's animation queue.
const UNIT_STEEP_LIMIT_FIELD: u32 = 0x150;
const UNIT_STEEP_LIMIT: i32 = 4;
const UNIT_MIXER_CHAINS: u32 = 0x8c;
/// `BFTile`: terrain shape, water bits and world position.
const TILE_SHAPE: u32 = 0x81;
const TILE_WATER_FLAGS: u32 = 0x83;
const TILE_WATER_MASK: u8 = 0x3;
const TILE_WORLD_POS: u32 = 0x34;
/// The non-flat shapes that count as gently sloped.
const GENTLE_SHAPES: [u8; 4] = [0x41, 0x50, 0x14, 0x05];
/// `ZTHabitat` byte a guest must find set in the habitat it is in to be put down.
const HABITAT_GUESTS_ALLOWED: u32 = 0x2c;
/// The building type id an animal may leave into another habitat from.
const ANY_HABITAT_TYPE_ID: u32 = 0x1b93;
/// The path cost from which a step is impassable, as an RVA.
const MAX_PATH_COST_RVA: u32 = 0x0023_5494;
/// The `BFPos` no tile has, as an RVA.
const NO_TILE_RVA: u32 = 0x0023_9370;

mod slots {
    /// Entities and units.
    pub const REMOVE_FROM_MAP: u32 = 0x78;
    pub const GET_SWIMS: u32 = 0xbc;
    pub const SET_VISIBLE: u32 = 0xcc;
    pub const RESET_IDLE_ANIM: u32 = 0x13c;
    pub const GET_EDGE_COST: u32 = 0x160;
    pub const STOP_EVERYTHING: u32 = 0x198;
    pub const GET_HABITAT_FROM_TILE: u32 = 0x248;
    pub const GET_HABITAT: u32 = 0x24c;
    /// `BFAIMgr::getPath(start, end, unit, flag)`, which writes the path into the unit's target list.
    pub const AI_GET_PATH: u32 = 0x18;
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn position_at(address: u32) -> [i32; 3] {
    std::array::from_fn(|axis| get_from_memory(address + axis as u32 * 4))
}

fn write_position(address: u32, position: [i32; 3]) {
    for (axis, value) in position.into_iter().enumerate() {
        save_to_memory(address + axis as u32 * 4, value);
    }
}

/// The address of the virtual method at `slot` of `object`'s vtable.
fn virtual_address(object: u32, slot: u32) -> u32 {
    get_from_memory(get_from_memory::<u32>(object) + slot)
}

macro_rules! virtual_call {
    ($object:expr, $slot:expr $(, $argument:expr)*) => {{
        let object: u32 = $object;
        let address = virtual_address(object, $slot);
        // Every slot called here takes `this` and dword arguments and returns at most a dword.
        unsafe {
            std::mem::transmute::<u32, extern "thiscall" fn(u32 $(, virtual_call!(@word $argument))*) -> u32>(address)(object $(, $argument)*)
        }
    }};
    (@word $argument:expr) => {
        u32
    };
}

/// A building's offset `local` turned by the building's rotation and moved to `origin`. A rotation that is not a
/// quarter turn (0, 2, 4 or 6 of 8) gives `origin` itself.
pub fn offset_from_building(origin: [i32; 3], rotation: u32, local: [i32; 3]) -> [i32; 3] {
    let [x, y, z] = origin;
    let [dx, dy, dz] = local;
    let (x, y) = match rotation {
        0 => (x.wrapping_sub(dx), y.wrapping_sub(dy)),
        2 => (x.wrapping_add(dy), y.wrapping_sub(dx)),
        4 => (x.wrapping_add(dx), y.wrapping_add(dy)),
        6 => (x.wrapping_sub(dy), y.wrapping_add(dx)),
        _ => return origin,
    };
    [x, y, z.wrapping_add(dz)]
}

/// The tile coordinates of a world position, with `z` rounded down.
fn grid_of(position: [i32; 3]) -> [i32; 3] {
    [world_to_tile(position[0]), world_to_tile(position[1]), position[2].div_euclid(16)]
}

/// Writes `positions` to the empty `vector<BFPos>` header at `out`, in an exactly sized pool block.
fn write_position_vector(out: u32, positions: &[[i32; 3]]) {
    let bytes = positions.len() as u32 * 12;
    let begin = if bytes == 0 { 0 } else { unsafe { POOLALLOC_ALLOCATE.original()(bytes) as u32 } };
    for (index, position) in positions.iter().enumerate() {
        write_position(begin + index as u32 * 12, *position);
    }
    write_position(out, [begin as i32, (begin + bytes) as i32, (begin + bytes) as i32]);
}

/// The positions of the `vector<BFPos>` whose header is at `vector`.
fn positions_of(vector: u32) -> Vec<[i32; 3]> {
    let (begin, end): (u32, u32) = (get_from_memory(vector), get_from_memory(vector + 4));
    let count = (end.wrapping_sub(begin) as i32 / 12).max(0) as u32;
    (0..count).map(|index| position_at(begin + index * 12)).collect()
}

/// The positions in a vector vanilla or [`write_position_vector`] allocated, which is then freed.
#[cfg(feature = "reimplementation-tests")]
pub(super) fn take_position_vector(words: [u32; 3]) -> Vec<[i32; 3]> {
    let positions = positions_of(words.as_ptr() as u32);
    if words[0] != 0 {
        let bytes = words[2].wrapping_sub(words[0]) / 12 * 12;
        unsafe { openzt_detour::generated::poolalloc::DEALLOCATE.original()(words[0] as *const u32, bytes) };
    }
    positions
}

impl ZTBuilding {
    /// `ZTBuilding::gxPosFromXYPos` (`0x0042a10c`): the map position of the building-relative `local`.
    pub fn gx_pos_from_xy_pos(self, local: [i32; 3]) -> [i32; 3] {
        offset_from_building(position_at(self.0 + ENTITY_POS), self.read(ENTITY_ROTATION), local)
    }

    /// The address of the type's record for slot `slot`.
    fn slot_record(self, slot: u32) -> Option<u32> {
        let building_type = self.entity_type();
        if building_type == 0 {
            return None;
        }
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        let (begin, end): (u32, u32) = (get_from_memory(records), get_from_memory(records + 4));
        let size = size_of::<ZTBuildingSlotType>() as u32;
        let count = (end.wrapping_sub(begin) as i32 / size as i32) as u32;
        (slot < count).then(|| begin + slot * size)
    }

    /// The record position at `field` of slot `slot`'s record, `(0, 0, 0)` for a slot the type does not have.
    fn record_position(self, slot: u32, field: usize) -> [i32; 3] {
        self.slot_record(slot).map_or([0; 3], |record| position_at(record + field as u32))
    }

    /// `ZTBuilding::getSlotLoc` (`0x005ad347`).
    pub fn slot_loc(self, slot: u32) -> [i32; 3] {
        self.gx_pos_from_xy_pos(self.record_position(slot, offset_of!(ZTBuildingSlotType, position)))
    }

    /// `ZTBuilding::getSlotExit` (`0x004a705e`).
    pub fn slot_exit(self, slot: u32) -> [i32; 3] {
        self.gx_pos_from_xy_pos(self.record_position(slot, offset_of!(ZTBuildingSlotType, exit_position)))
    }

    /// `ZTBuilding::getSlotDrawLoc` (`0x005a954d`): where the slot's `index`th drawn user stands.
    pub fn slot_draw_loc(self, slot: u32, index: i32) -> [i32; 3] {
        let local = self.slot_record(slot).map_or([0; 3], |record| {
            let positions = record + offset_of!(ZTBuildingSlotType, draw_positions) as u32;
            let (begin, end): (u32, u32) = (get_from_memory(positions), get_from_memory(positions + 4));
            if index >= 0 && index < end.wrapping_sub(begin) as i32 / 12 {
                position_at(begin + index as u32 * 12)
            } else {
                [0; 3]
            }
        });
        self.gx_pos_from_xy_pos(local)
    }

    /// `ZTBuilding::getSlotFacing` (`0x005ad288`): the direction a user of the slot faces, the record's facing turned
    /// by the building's rotation.
    pub fn slot_facing(self, slot: u32) -> u32 {
        let facing: i32 = self.slot_record(slot).map_or(0, |record| get_from_memory(record + offset_of!(ZTBuildingSlotType, facing) as u32));
        (self.read::<i32>(ENTITY_ROTATION).wrapping_add(facing) % 8) as u32
    }

    /// `ZTBuilding::getSlotHasDrawSlots` (`0x005ad441`): the record's byte.
    pub fn slot_has_draw_slots(self, slot: u32) -> u8 {
        self.slot_record(slot).map_or(0, |record| get_from_memory(record + offset_of!(ZTBuildingSlotType, has_draw_slots) as u32))
    }

    /// `ZTBuilding::getSlotEntrances` (`0x0042b101`): the map positions of the slot's entrances.
    pub fn slot_entrances(self, slot: u32) -> Vec<[i32; 3]> {
        let Some(record) = self.slot_record(slot) else {
            return Vec::new();
        };
        let entrances = positions_of(record + offset_of!(ZTBuildingSlotType, entrances) as u32);
        entrances.into_iter().map(|local| self.gx_pos_from_xy_pos(local)).collect()
    }

    /// `ZTBuilding::getValidEntranceForExit` (`0x0042b350`): writes to `out` the first entrance of the slot `unit` can be
    /// put down on.
    pub fn valid_entrance_for_exit(self, out: &mut [i32; 3], unit: u32, slot: u32) -> bool {
        if unit == 0 || slot == i32::MIN as u32 {
            return false;
        }
        let entrance = self.slot_entrances(slot).into_iter().find(|&entrance| self.is_valid_exit(unit, entrance, -1));
        if let Some(entrance) = entrance {
            *out = entrance;
        }
        entrance.is_some()
    }

    /// `ZTBuilding::getValidExit` (`0x0042aad5`): where `unit` leaves slot `slot` to. A type whose users use the exit
    /// writes the slot's exit to `out` whether or not it is valid; any other takes a valid entrance.
    pub fn valid_exit(self, out: &mut [i32; 3], unit: u32, slot: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::valid_exit(self.0, unit, slot) {
            return answer;
        }
        if unit == 0 || slot == i32::MIN as u32 || self.entity_type() == 0 {
            return false;
        }
        if self.type_field::<u8>(offset_of!(ZTBuildingType, user_uses_exit)) == 0 {
            return self.valid_entrance_for_exit(out, unit, slot);
        }
        *out = self.slot_exit(slot);
        self.is_valid_exit(unit, *out, slot as i32)
    }

    /// `ZTBuilding::isValidExit` (`0x0042ab29`): whether `unit` may be put down at `position`. A slot's own exit
    /// (`slot >= 0`) must also allow the water or land it is on; an entrance is checked with `-1`.
    pub fn is_valid_exit(self, unit: u32, position: [i32; 3], slot: i32) -> bool {
        let tile = map_tile(world_to_tile(position[0]), world_to_tile(position[1]));
        if tile == 0
            || !low_byte_bool(unsafe { IS_IN_ZOO.original()(tile as *const u32, 0) })
            || !unsafe { IS_CLEAR_FOR_WALK_BY.original()(tile as *const u32, unit as *const u32) }
        {
            return false;
        }
        let water = get_from_memory::<u8>(tile + TILE_WATER_FLAGS) & TILE_WATER_MASK != 0;
        if slot >= 0 {
            let field = if water { offset_of!(ZTBuildingSlotType, exit_on_water) } else { offset_of!(ZTBuildingSlotType, exit_on_land) };
            let record = if self.is_building_type() { self.slot_record(slot as u32) } else { None };
            if !record.is_some_and(|record| get_from_memory::<u8>(record + field as u32) != 0) {
                return false;
            }
        }
        if unit == 0 {
            return true;
        }
        let mut valid = true;
        if cast(unit, GUEST_TAG_RVA) != 0 {
            let habitat = virtual_call!(unit, slots::GET_HABITAT);
            valid = habitat != 0 && get_from_memory::<u8>(habitat + HABITAT_GUESTS_ALLOWED) != 0;
        }
        if cast(unit, ANIMAL_TAG_RVA) == 0 {
            return valid;
        }
        let any_habitat = self.entity_type() != 0 && self.type_id() == ANY_HABITAT_TYPE_ID;
        let own = virtual_call!(unit, slots::GET_HABITAT);
        let there = virtual_call!(unit, slots::GET_HABITAT_FROM_TILE, tile);
        if own != there && !any_habitat {
            return false;
        }
        let shape: u8 = get_from_memory(tile + TILE_SHAPE);
        if shape != 0 && !GENTLE_SHAPES.contains(&shape) && get_from_memory::<i32>(unit + UNIT_STEEP_LIMIT_FIELD) >= UNIT_STEEP_LIMIT {
            return false;
        }
        if low_byte_bool(virtual_call!(unit, slots::GET_SWIMS)) {
            return valid;
        }
        valid && !water
    }

    /// Whether the step from the tile inside the building onto the tile of `exit` is one `unit` can take.
    fn exit_is_reachable(self, unit: u32, exit: [i32; 3]) -> bool {
        let exit_tile = map_tile(world_to_tile(exit[0]), world_to_tile(exit[1]));
        if exit_tile == 0 {
            return false;
        }
        let tile_position = position_at(exit_tile + TILE_WORLD_POS);
        let mut inside = [0i32; 3];
        let at = unsafe { GET_INSIDE_POS.hooked()(self.0 as *const u32, inside.as_mut_ptr() as *const u32, tile_position.as_ptr()) } as u32;
        let inside_tile = map_tile(get_from_memory(at), get_from_memory(at + 4));
        if inside_tile == 0 {
            return false;
        }
        let direction = unsafe { BFMAP_GET_DIRECTION.hooked()(inside_tile as i32, exit_tile as i32) } as u32;
        let cost = virtual_call!(unit, slots::GET_EDGE_COST, inside_tile, direction) as i32;
        cost < get_from_memory::<i32>(base() + MAX_PATH_COST_RVA)
    }

    /// `ZTBuilding::exitViaExit` (`0x004a75bb`): stops `unit` and puts it down outside the slot, or takes it off the map
    /// when there is nowhere to put it. A type that may use the entrance as an emergency exit does so when its exit
    /// cannot be reached from inside. `false` only for something that is not a unit.
    pub fn exit_via_exit(self, unit: u32, slot: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::exit_via_exit(self.0, unit, slot) {
            return answer;
        }
        if cast(unit, UNIT_TAG_RVA) == 0 || self.entity_type() == 0 {
            return false;
        }
        unsafe { MIXER_CHAIN_DEQUE_CLEAR.hooked()((unit + UNIT_MIXER_CHAINS) as *const u32) };
        virtual_call!(unit, slots::STOP_EVERYTHING, 1, 1, 1);
        let mut exit = [0i32; 3];
        let valid = self.valid_exit(&mut exit, unit, slot);
        let placeable = if self.type_field::<u8>(offset_of!(ZTBuildingType, user_uses_entrance_as_emergency_exit)) == 0 {
            valid
        } else {
            self.exit_is_reachable(unit, exit) || self.valid_entrance_for_exit(&mut exit, unit, slot)
        };
        if !placeable {
            virtual_call!(unit, slots::REMOVE_FROM_MAP, 0);
            return true;
        }
        let position = IVec3::from_array(exit);
        unsafe { MOVE_ENTITY.hooked()(unit as *const u32, &position) };
        if cast(unit, UNIT_TAG_RVA) != 0 {
            virtual_call!(unit, slots::RESET_IDLE_ANIM);
        }
        if self.type_field::<u8>(offset_of!(ZTBuildingType, hide_user)) != 0 {
            virtual_call!(unit, slots::SET_VISIBLE, 1);
        }
        true
    }

    /// `ZTBuilding::findNearestOpenSlot` (`0x0042c02b`): among the slots that would take `unit`, the nearest entrance it
    /// has a path to. Writes the slot to `slot_out` and the entrance to `position_out`, and leaves that path in the
    /// unit's target list. A type with random slots tries one slot, picked with `rand`.
    pub fn find_nearest_open_slot(self, unit: u32, slot_out: u32, position_out: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::find_nearest_open_slot(self.0, unit, slot_out, position_out) {
            return answer;
        }
        let ai = globals().ztaimgr_ptr() as u32;
        if ai == 0 || unit == 0 {
            return false;
        }
        let mut grid = [0i32; 3];
        let at = unsafe { GET_GRID_POS.hooked()(unit as *const u32, grid.as_mut_ptr()) } as u32;
        let grid = position_at(at);
        if cast(unit, UNIT_TAG_RVA) == 0 || self.entity_type() == 0 {
            return false;
        }
        let random = self.type_field::<u8>(offset_of!(ZTBuildingType, use_random_slot)) != 0;
        let none = position_at(base() + NO_TILE_RVA);
        // The last tile a path was found to, and the last one it was not: neither is tried again.
        let (mut reached, mut unreached) = (none, none);
        let path = TargetList::of_unit(unit);
        let mut nearest = i32::MAX;
        let mut found = false;
        let count = self.slot_count() as u32;
        let mut slot = 0;
        while slot < count {
            if random {
                slot = unsafe { RAND.hooked()() } % count;
            }
            if self.can_add_user(unit, slot) {
                for entrance in self.slot_entrances(slot) {
                    let dx = get_from_memory::<i32>(unit + ENTITY_POS).wrapping_sub(entrance[0]);
                    let dy = get_from_memory::<i32>(unit + ENTITY_POS + 4).wrapping_sub(entrance[1]);
                    let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
                    if distance >= nearest {
                        continue;
                    }
                    let target = grid_of(entrance);
                    if target == reached || target == unreached {
                        continue;
                    }
                    let previous_path = path.positions();
                    let has_path = low_byte_bool(virtual_call!(ai, slots::AI_GET_PATH, grid.as_ptr() as u32, target.as_ptr() as u32, unit, 0));
                    if has_path {
                        reached = target;
                        found = true;
                        nearest = distance;
                        write_position(position_out, entrance);
                        save_to_memory(slot_out, slot);
                    } else {
                        unsafe { path.assign(&mut VanillaPool, &previous_path) };
                        unreached = target;
                    }
                }
            }
            if random {
                break;
            }
            slot += 1;
        }
        found
    }
}

#[detour_mod]
mod geometry_detours {
    use super::*;

    #[detour(GX_POS_FROM_XY_POS)]
    unsafe extern "thiscall" fn gx_pos_from_xy_pos(this: *const u32, out: *const i32, local: *const i32) -> *const i32 {
        let position = ZTBuilding(this as u32).gx_pos_from_xy_pos(position_at(local as u32));
        write_position(out as u32, position);
        out
    }

    #[detour(GX_POS_FROM_XYPOS_1)]
    unsafe extern "thiscall" fn gx_positions_from_xy_positions(this: *const u32, out: u32, local: *const u32) -> *const u32 {
        let building = ZTBuilding(this as u32);
        let positions: Vec<[i32; 3]> = positions_of(local as u32).into_iter().map(|local| building.gx_pos_from_xy_pos(local)).collect();
        write_position_vector(out, &positions);
        out as *const u32
    }

    #[detour(GET_SLOT_LOC)]
    unsafe extern "thiscall" fn get_slot_loc(this: *const u32, out: *const u32, slot: u32) -> *const u32 {
        write_position(out as u32, ZTBuilding(this as u32).slot_loc(slot));
        out
    }

    #[detour(GET_SLOT_EXIT)]
    unsafe extern "thiscall" fn get_slot_exit(this: *const u32, out: *const u32, slot: u32) -> *const u32 {
        write_position(out as u32, ZTBuilding(this as u32).slot_exit(slot));
        out
    }

    #[detour(GET_SLOT_DRAW_LOC)]
    unsafe extern "thiscall" fn get_slot_draw_loc(this: *const u32, out: *const u32, slot: u32, index: i32) -> *const u32 {
        write_position(out as u32, ZTBuilding(this as u32).slot_draw_loc(slot, index));
        out
    }

    #[detour(GET_SLOT_FACING)]
    unsafe extern "thiscall" fn get_slot_facing(this: *const u32, slot: u32) -> u32 {
        ZTBuilding(this as u32).slot_facing(slot)
    }

    #[detour(GET_SLOT_HAS_DRAW_SLOTS)]
    unsafe extern "thiscall" fn get_slot_has_draw_slots(this: *const u32, slot: u32) -> u32 {
        u32::from(ZTBuilding(this as u32).slot_has_draw_slots(slot))
    }

    #[detour(GET_SLOT_ENTRANCES)]
    unsafe extern "thiscall" fn get_slot_entrances(this: *const u32, out: *const u32, slot: u32) -> *const u32 {
        write_position_vector(out as u32, &ZTBuilding(this as u32).slot_entrances(slot));
        out
    }

    #[detour(GET_VALID_ENTRANCE_FOR_EXIT)]
    unsafe extern "thiscall" fn get_valid_entrance_for_exit(this: *const u32, out: *const u32, unit: *const i32, slot: *const i32) -> u32 {
        let mut position = position_at(out as u32);
        let found = ZTBuilding(this as u32).valid_entrance_for_exit(&mut position, unit as u32, slot as u32);
        if found {
            write_position(out as u32, position);
        }
        u32::from(found)
    }

    #[detour(GET_VALID_EXIT)]
    unsafe extern "thiscall" fn get_valid_exit(this: *const u32, out: *const i32, unit: *const i32, slot: *const i32) -> u32 {
        let mut position = position_at(out as u32);
        let found = ZTBuilding(this as u32).valid_exit(&mut position, unit as u32, slot as u32);
        write_position(out as u32, position);
        u32::from(found)
    }

    #[detour(IS_VALID_EXIT)]
    unsafe extern "thiscall" fn is_valid_exit(this: *const u32, unit: *const u32, position: *const IVec3, slot: i32) -> u32 {
        u32::from(ZTBuilding(this as u32).is_valid_exit(unit as u32, position_at(position as u32), slot))
    }

    #[detour(EXIT_VIA_EXIT)]
    unsafe extern "thiscall" fn exit_via_exit(this: *const u32, unit: *const i32, slot: u32) -> u8 {
        u8::from(ZTBuilding(this as u32).exit_via_exit(unit as u32, slot))
    }

    #[detour(FIND_NEAREST_OPEN_SLOT)]
    unsafe extern "thiscall" fn find_nearest_open_slot(this: *const u32, unit: *const i32, slot_out: *const u32, position_out: u32) -> u32 {
        u32::from(ZTBuilding(this as u32).find_nearest_open_slot(unit as u32, slot_out as u32, position_out))
    }

    /// The vanilla bodies, for the live tests.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use glam::IVec3;

        use super::super::{position_at, take_position_vector};

        pub(crate) fn gx_pos_from_xy_pos(this: u32, local: [i32; 3]) -> [i32; 3] {
            let mut out = [0i32; 3];
            let at = unsafe { super::GX_POS_FROM_XY_POS_DETOUR.call(this as *const u32, out.as_mut_ptr(), local.as_ptr()) };
            position_at(at as u32)
        }

        pub(crate) fn gx_positions(this: u32, local: &[[i32; 3]]) -> Vec<[i32; 3]> {
            let begin = local.as_ptr() as u32;
            let end = begin + local.len() as u32 * 12;
            let input = [begin, end, end];
            let mut out = [0u32; 3];
            unsafe { super::GX_POS_FROM_XYPOS_1_DETOUR.call(this as *const u32, out.as_mut_ptr() as u32, input.as_ptr()) };
            take_position_vector(get_words(&out))
        }

        /// Reads an out-parameter vanilla filled.
        fn get_words(words: &[u32; 3]) -> [u32; 3] {
            crate::util::get_from_memory(words.as_ptr() as u32)
        }

        pub(crate) fn slot_loc(this: u32, slot: u32) -> [i32; 3] {
            let mut out = [0u32; 3];
            let at = unsafe { super::GET_SLOT_LOC_DETOUR.call(this as *const u32, out.as_mut_ptr(), slot) };
            position_at(at as u32)
        }

        pub(crate) fn slot_exit(this: u32, slot: u32) -> [i32; 3] {
            let mut out = [0u32; 3];
            let at = unsafe { super::GET_SLOT_EXIT_DETOUR.call(this as *const u32, out.as_mut_ptr(), slot) };
            position_at(at as u32)
        }

        pub(crate) fn slot_draw_loc(this: u32, slot: u32, index: i32) -> [i32; 3] {
            let mut out = [0u32; 3];
            let at = unsafe { super::GET_SLOT_DRAW_LOC_DETOUR.call(this as *const u32, out.as_mut_ptr(), slot, index) };
            position_at(at as u32)
        }

        pub(crate) fn slot_facing(this: u32, slot: u32) -> u32 {
            unsafe { super::GET_SLOT_FACING_DETOUR.call(this as *const u32, slot) }
        }

        pub(crate) fn slot_has_draw_slots(this: u32, slot: u32) -> u8 {
            (unsafe { super::GET_SLOT_HAS_DRAW_SLOTS_DETOUR.call(this as *const u32, slot) }) as u8
        }

        pub(crate) fn slot_entrances(this: u32, slot: u32) -> Vec<[i32; 3]> {
            let mut out = [0u32; 3];
            unsafe { super::GET_SLOT_ENTRANCES_DETOUR.call(this as *const u32, out.as_mut_ptr(), slot) };
            take_position_vector(get_words(&out))
        }

        pub(crate) fn valid_entrance_for_exit(this: u32, out: &mut [i32; 3], unit: u32, slot: u32) -> bool {
            let result = unsafe {
                super::GET_VALID_ENTRANCE_FOR_EXIT_DETOUR.call(this as *const u32, out.as_mut_ptr() as *const u32, unit as *const i32, slot as *const i32)
            };
            result & 0xff != 0
        }

        pub(crate) fn valid_exit(this: u32, out: &mut [i32; 3], unit: u32, slot: u32) -> bool {
            let result = unsafe { super::GET_VALID_EXIT_DETOUR.call(this as *const u32, out.as_mut_ptr(), unit as *const i32, slot as *const i32) };
            result & 0xff != 0
        }

        pub(crate) fn is_valid_exit(this: u32, unit: u32, position: [i32; 3], slot: i32) -> bool {
            let position = IVec3::from_array(position);
            let result = unsafe { super::IS_VALID_EXIT_DETOUR.call(this as *const u32, unit as *const u32, &position, slot) };
            result & 0xff != 0
        }

        pub(crate) fn exit_via_exit(this: u32, unit: u32, slot: u32) -> bool {
            unsafe { super::EXIT_VIA_EXIT_DETOUR.call(this as *const u32, unit as *const i32, slot) != 0 }
        }

        pub(crate) fn find_nearest_open_slot(this: u32, unit: u32, slot_out: &mut u32, position_out: &mut [i32; 3]) -> bool {
            let result = unsafe {
                super::FIND_NEAREST_OPEN_SLOT_DETOUR.call(this as *const u32, unit as *const i32, slot_out, position_out.as_mut_ptr() as u32)
            };
            result & 0xff != 0
        }
    }
}

/// Installs the geometry and exit detours.
pub(super) fn init() {
    if let Err(e) = unsafe { geometry_detours::init_detours() } {
        tracing::error!("Failed to initialise ztbuilding geometry detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    geometry_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use geometry_detours::test_real;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_turn_with_the_building() {
        let origin = [1000, 2000, 30];
        let local = [10, 3, 5];
        assert_eq!(offset_from_building(origin, 0, local), [990, 1997, 35]);
        assert_eq!(offset_from_building(origin, 2, local), [1003, 1990, 35]);
        assert_eq!(offset_from_building(origin, 4, local), [1010, 2003, 35]);
        assert_eq!(offset_from_building(origin, 6, local), [997, 2010, 35]);
    }

    #[test]
    fn other_rotations_keep_the_building_position() {
        for rotation in [1, 3, 5, 7, 8, u32::MAX] {
            assert_eq!(offset_from_building([1, 2, 3], rotation, [10, 20, 30]), [1, 2, 3]);
        }
    }

    #[test]
    fn grid_rounds_z_down_and_xy_toward_zero() {
        assert_eq!(grid_of([130, -1, 33]), [2, 0, 2]);
        assert_eq!(grid_of([-65, 64, -1]), [-1, 1, -1]);
        assert_eq!(grid_of([0, 0, -16]), [0, 0, -1]);
    }
}
