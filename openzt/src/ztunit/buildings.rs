//! `ZTUnit` building-use methods (group D of `plans/ztunit-scoping-plan.md`): the building the unit works at, the
//! goals that send it there, and the behaviour-set names it plays while it is inside.
//!
//! A unit records its building as a tile record (`+0x244`, a `BFTile` whose four entity slots hold the buildings
//! standing on its quadrants) plus an offset `(x, y, z)` inside that tile (`+0x214`); the building is whatever
//! building occupies the quadrant the offset names. Every collaborator on the building side (`ZTBuilding`,
//! `BFUnit::goToDest`, `ZTGuest::fGuestThought`) stays vanilla and is called through its `generated.rs` entry.

use std::ffi::c_void;

use openzt_detour::generated::{
    bfentity::{GET_INSIDE_POS, GET_TILE as BFENTITY_GET_TILE, SET_BS},
    bfmap::GET_DIRECTION_0 as BFMAP_GET_DIRECTION,
    bfunit::{GO_TO_DEST_0 as BFUNIT_GO_TO_DEST_STRING, GO_TO_DEST_1 as BFUNIT_GO_TO_DEST},
    msvc_std_basic_string::BASIC_STRING_RANGE,
    ztbuilding::{ADD_USER, CAN_ADD_USER, FIND_NEAREST_OPEN_SLOT, REMOVE_MEMBER, REMOVE_USER, REMOVE_USER_IF_PRESENT, YANK_USER_IF_PRESENT},
    ztguest::F_GUEST_THOUGHT,
    ztunit::{
        F_ENTER_BUILDING, F_EXIT_BUILDING, F_RETURN_TO_BUILDING, GET_HABITAT_FROM_VA, GET_SHOW_ITEM, HAS_BUILDING_USE_GOAL, REMOVE_FROM_BUILDING_IF_PRESENT, SET_BUILDING_GOAL,
        SET_BUILDING_TILE, SET_BUILDING_USE_BS, SET_EXIT_RETURN_BUILDING_BS, UPDATE_BUILDING_TILES, VALIDATE_BUILDING,
    },
    ztworldmgr::{GET_NEED_FROM_INDEX, GET_NEED_INDEX, SATISFIES},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{
    base, call_slot_2_returning, free_string, game_rand, goal_slots, map_tile, tile_offsets, unit_offsets, ZTUnit, BUILDING_NEED_NAME_RVA, BUILDING_TAG_RVA, GUEST_TAG_RVA, WORLD_MGR_RVA,
};
use crate::{
    bfentity::new_owned_name,
    util::{get_from_memory, low_byte_bool, position_in_tile},
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, call_slot_3},
    vanilla_list::NodeList,
    vanilla_string::{c_string_bytes, object_bytes, range_bytes},
    ztmegatilemgr::entity_type_matches,
};

/// Vtable slots on the unit these methods dispatch through.
mod slots {
    /// `setGridDest(position)`.
    pub const SET_GRID_DEST: u32 = 0x1a0;
    /// `getEdgeCost(tile, direction)`.
    pub const GET_EDGE_COST: u32 = 0x160;
    /// `abortGoal(bool)`.
    pub const ABORT_GOAL: u32 = 0x178;
    /// `isReturning()`.
    pub const IS_RETURNING: u32 = 0x264;
    /// `getReturnBuildingString()`.
    pub const GET_RETURN_BUILDING_STRING: u32 = 0x2a0;
    /// `getReturnBuildingNameID()`.
    pub const GET_RETURN_BUILDING_NAME_ID: u32 = 0x298;
    /// `doBuilding()`.
    pub const DO_BUILDING: u32 = 0x2ec;
}

/// The need `fReturnToBuilding` looks for a satisfier of (`DAT_0063af90`), as an RVA of a `string`.
const RETURN_NEED_NAME_RVA: u32 = 0x0023_af90;
/// `DAT_00639388`: the prefix of the behaviour-set name `setBuildingUseBS` builds, as an RVA of a NUL-terminated string.
pub(super) const USE_NAME_PREFIX_RVA: u32 = 0x0023_9388;
/// `DAT_006393ec`: the cost `updateBuildingTiles` stores as the unit's non-path cost, and `DAT_00635494`: the largest
/// edge cost it accepts for the last step into the building.
const BUILDING_NON_PATH_COST_RVA: u32 = 0x0023_93ec;
const MAX_EDGE_COST_RVA: u32 = 0x0023_5494;

/// Goals the building routines set.
const GOAL_WALK_TO_BUILDING: u32 = 0x66;
const GOAL_WALK_TO_EXHIBIT_BUILDING: u32 = 0x67;
const GOAL_USE_BUILDING: u32 = 0x6a;
const GOAL_LEAVE_BUILDING: u32 = 0x83;
/// `ZTGuest::fGuestThought` id a guest voices when a full building turns it away.
const THOUGHT_BUILDING_FULL: u32 = 0x2803;

/// `ZTBuildingType` fields: the byte that picks the exhibit-style goal, the byte `updateBuildingTiles` removes a member on
/// leaving, and the occupancy a full building reaches (`+0x16c`, compared with the building's `+0x18c`).
const TYPE_EXHIBIT_FLAG: u32 = 0x1a4;
const TYPE_REMOVES_MEMBER: u32 = 0x19b;
const TYPE_FULL_COUNT: u32 = 0x16c;
/// `ZTBuilding` fields: the user records (`0x24` bytes each, `{begin, end}` at `+0x170`/`+0x174`), the occupancy
/// (`+0x18c`).
const BUILDING_USERS_BEGIN: u32 = 0x170;
const BUILDING_USERS_END: u32 = 0x174;
const BUILDING_OCCUPANCY: u32 = 0x18c;
const USER_RECORD_SIZE: i32 = 0x24;
/// `BFEntity` fields: position (x, y, z), and the building type's name range (`+0xa4`/`+0xa8` of its entity type).
const ENTITY_POS: u32 = 0x114;
const TYPE_NAME_BEGIN: u32 = 0xa4;
const TYPE_NAME_END: u32 = 0xa8;
/// `ZTGuest` byte `updateBuildingTiles` sets when a guest that is not returning gives up on the building.
const GUEST_GAVE_UP: u32 = 0x33e;

/// `ZTBuilding`/`BFUnit` collaborators, all called through their vanilla entries. A live test reaches them by
/// detouring the entries; nothing here is intercepted.
fn building_need_name() -> u32 {
    base() + BUILDING_NEED_NAME_RVA
}

impl ZTUnit {
    /// `ZTUnit::hasBuildingUseGoal` (`0x004108f7`): the goal is one of the three building-use goals.
    pub fn has_building_use_goal(self) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::has_building_use_goal(self.0) {
            return answer;
        }
        super::is_building_use_goal(self.read(unit_offsets::GOAL_ID))
    }

    /// `ZTUnit::setBuildingTile` (`0x0043e6b3`).
    pub fn set_building_tile(self, tile: u32) {
        self.write(unit_offsets::BUILDING_TILE, tile);
    }

    /// The building standing on the quadrant the unit's building offset names, or `0`.
    fn building_at_offset(self) -> u32 {
        let record: u32 = self.read(unit_offsets::BUILDING_TILE);
        if record == 0 {
            return 0;
        }
        let (x, y): (i32, i32) = (self.read(unit_offsets::BUILDING_OFFSET), self.read(unit_offsets::BUILDING_OFFSET + 4));
        if !(0..0x40).contains(&x) || !(0..0x40).contains(&y) {
            return 0;
        }
        let candidate: u32 = get_from_memory(record + tile_offsets::ENTITY_SLOTS + (((y >> 5) + (x >> 5) * 2) as u32) * 4);
        if candidate != 0 && unsafe { entity_type_matches(candidate, BUILDING_TAG_RVA) } {
            candidate
        } else {
            0
        }
    }

    /// `ZTUnit::validateBuilding` (`0x0042a5af`): the building the unit works at. With `check` set it must also satisfy the
    /// unit's need (`satisfies`) and the unit needs a slot (`+0x24d`); `*out` then says whether it can take another user.
    pub fn validate_building(self, out: *mut bool, check: bool) -> u32 {
        let mut building = self.building_at_offset();
        if !check {
            return building;
        }
        let slot: u8 = self.read(unit_offsets::BUILDING_SLOT);
        if slot == 0xff || building == 0 {
            return 0;
        }
        let mut need = [0u32; 3];
        unsafe { GET_NEED_FROM_INDEX.hooked()(need.as_mut_ptr(), self.read::<u8>(unit_offsets::NEED_INDEX)) };
        if low_byte_bool(unsafe { SATISFIES.hooked()(building as i32, need.as_ptr() as *const i8, std::ptr::null()) }) {
            let can_add = unsafe { CAN_ADD_USER.hooked()(building as *const u32, self.0 as *const i32, u32::from(slot) as *const i32) };
            unsafe { out.write(low_byte_bool(can_add)) };
        } else {
            building = 0;
        }
        free_string(&need);
        building
    }

    /// `ZTUnit::fEnterBuilding` (`0x005ad3a6`): joins the building as a user. When `addUser` refuses, the unit stops
    /// everything and still answers `true`; with no usable building it ends a building trick, stops everything and answers
    /// `false`.
    pub fn f_enter_building(self) -> bool {
        let mut can_add = false;
        let building = self.validate_building_slot(&mut can_add, true);
        if building != 0 && can_add {
            let slot = u32::from(self.read::<u8>(unit_offsets::BUILDING_SLOT));
            if !low_byte_bool(unsafe { ADD_USER.hooked()(building as *const u32, self.0 as *const i32, slot as *const i32) }) {
                call_slot_3(self.0, goal_slots::STOP_EVERYTHING, 1, 1, 0);
            }
            return true;
        }
        self.end_trick_when_building_need();
        call_slot_3(self.0, goal_slots::STOP_EVERYTHING, 1, 1, 0);
        false
    }

    /// `ZTUnit::fExitBuilding` (`0x004a7705`): runs the unit's building step, then leaves the building. `false` when the unit
    /// has no building.
    pub fn f_exit_building(self) -> bool {
        let mut can_add = false;
        let building = self.validate_building_slot(&mut can_add, false);
        if building == 0 {
            return false;
        }
        call_slot_0(self.0, slots::DO_BUILDING);
        unsafe { REMOVE_USER.hooked()(building as *const u32, self.0 as *const i32, 0) };
        true
    }

    /// Dispatches `validateBuilding` through the vtable, so a subclass override is honoured.
    fn validate_building_slot(self, out: &mut bool, check: bool) -> u32 {
        let mut flag: u8 = 0;
        let building = call_slot_2_returning(self.0, super::goal_slots::VALIDATE_BUILDING, &mut flag as *mut u8 as u32, u32::from(check));
        *out = flag != 0;
        building
    }

    /// Ends the running trick (virtual) when the unit's need is the building need.
    fn end_trick_when_building_need(self) {
        let building_need = unsafe { GET_NEED_INDEX.hooked()(building_need_name() as *const i8) } as u8;
        if self.read::<u8>(unit_offsets::NEED_INDEX) == building_need {
            call_slot_0(self.0, goal_slots::ABORT_TRICK);
        }
    }

    /// `ZTUnit::removeFromBuildingIfPresent` (`0x005ae65f`): while the unit has a building-use goal, drops it from its
    /// building's users, yanking it when it was not a regular user.
    pub fn remove_from_building_if_present(self) {
        if !self.has_building_use_goal() {
            return;
        }
        let mut can_add = false;
        let building = self.validate_building_slot(&mut can_add, false);
        if building != 0 && !low_byte_bool(unsafe { REMOVE_USER_IF_PRESENT.hooked()(building as *const u32, self.0 as *const i32) }) {
            unsafe { YANK_USER_IF_PRESENT.hooked()(building as *const u32, self.0 as i32) };
        }
    }

    /// `ZTUnit::setBuildingGoal` (`0x00482c90`): drops everything, takes the building-use goal at `tile` with `slot` and the
    /// offset `(x, y, z)` inside it.
    pub fn set_building_goal(self, tile: u32, slot: u8, offset: [i32; 3]) {
        call_slot_3(self.0, goal_slots::STOP_EVERYTHING, 1, 1, 1);
        call_slot_2(self.0, goal_slots::GOAL_SET, GOAL_USE_BUILDING, 1);
        call_slot_1(self.0, goal_slots::SET_BUILDING_TILE, tile);
        for (index, value) in offset.into_iter().enumerate() {
            self.write(unit_offsets::BUILDING_OFFSET + index as u32 * 4, value);
        }
        self.write(unit_offsets::BUILDING_SLOT, slot);
        self.write(unit_offsets::COG_GOAL_FLAG, 1u8);
    }

    /// `ZTUnit::fReturnToBuilding` (`0x005a05e7`): picks any building that satisfies the return need within range and
    /// points the unit at it (the offset is the building's position inside its tile).
    pub fn f_return_to_building(self) -> bool {
        let mut building: u32 = 0;
        let filter = call_slot_0(self.0, slots::GET_RETURN_BUILDING_NAME_ID) as i32;
        let found = self.find_any_satisfier(self.tile(), base() + RETURN_NEED_NAME_RVA, &mut building as *mut u32 as u32, 0xb504, filter);
        if !found {
            return false;
        }
        let need = unsafe { GET_NEED_INDEX.hooked()((base() + RETURN_NEED_NAME_RVA) as *const i8) } as u8;
        self.write(unit_offsets::NEED_INDEX, need);
        let tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
        call_slot_1(self.0, goal_slots::SET_BUILDING_TILE, tile);
        let (x, y, z): (i32, i32, i32) = (get_from_memory(building + ENTITY_POS), get_from_memory(building + ENTITY_POS + 4), get_from_memory(building + ENTITY_POS + 8));
        for (index, value) in position_in_tile(x, y, z).into_iter().enumerate() {
            self.write(unit_offsets::BUILDING_OFFSET + index as u32 * 4, value);
        }
        true
    }

    /// `ZTUnit::setExitReturnBuildingBS` (`0x006133a0`): queues the behaviour set named `name` (a `string` object, which
    /// stays the caller's).
    pub fn set_exit_return_building_bs(self, name: *const u32) -> bool {
        let text = object_bytes(name as u32);
        self.queue_behaviour_set(&text)
    }

    /// `BFEntity::setBS(BFBSCall(name, 0), force = 1, mode = 2)`: one owned name handed to `setBS`.
    pub(super) fn queue_behaviour_set(self, text: &[u8]) -> bool {
        let name = unsafe { new_owned_name(text) };
        low_byte_bool(unsafe { SET_BS.hooked()(self.0 as *const u32, name.begin as *const u32, name.end as *const u32, name.capacity as *const u32, 0, 1, 2) } as u32)
    }

    /// `ZTUnit::setBuildingUseBS` (`0x005ad535`): queues the behaviour set `prefix + building type name + need name + slot`.
    /// While the unit performs a trick for the "building" need, the need name is the trick's own item name instead.
    /// Vanilla dereferences a type-less building; the port leaves that part of the name empty.
    pub fn set_building_use_bs(self, building: u32) -> bool {
        let slot = self.read::<u8>(unit_offsets::BUILDING_SLOT);
        let mut need = [0u32; 3];
        unsafe { GET_NEED_FROM_INDEX.hooked()(need.as_mut_ptr(), self.read::<u8>(unit_offsets::NEED_INDEX)) };
        let mut need_text = range_bytes(need[0], need[1]);
        if need_text == object_bytes(building_need_name()) {
            let item = unsafe { GET_SHOW_ITEM.hooked()(self.0 as *const u32, self.read::<u16>(unit_offsets::TRICK_TYPE), self.read::<u16>(unit_offsets::SHOW_INFO_ID)) } as u32;
            if item != 0 {
                need_text = object_bytes(item + 0x10);
            }
        }
        free_string(&need);
        let building_type: u32 = if building == 0 { 0 } else { get_from_memory(building + unit_offsets::ENTITY_TYPE) };
        let mut name = c_string_bytes(base() + USE_NAME_PREFIX_RVA);
        if building_type != 0 {
            name.extend(object_bytes(building_type + TYPE_NAME_BEGIN));
        }
        name.extend(need_text);
        name.extend(u32::from(slot).to_string().into_bytes());
        self.queue_behaviour_set(&name)
    }

    /// `ZTUnit::getHabitatFromVA` (`0x00613072`): the first of the unit tile's eight neighbour-habitat entries
    /// (starting at a random one) that is set, or `0`.
    pub fn get_habitat_from_va(self) -> u32 {
        let Some(cell) = super::habitat_cell(self.tile()) else {
            return 0;
        };
        let start = game_rand() & 7;
        for step in 0..8 {
            let record: u32 = get_from_memory(cell + 4 + ((start + step) & 7) as u32 * 4);
            if record != 0 {
                return get_from_memory(record);
            }
        }
        0
    }

    /// The position of the unit's second-to-last path step (`BFPathFinder::getPenultimatePos`, `0x0042bb16`): drops the
    /// last step when more than one remains, then answers the step that is now last (the default position when there is none).
    fn penultimate_path_pos(self) -> [i32; 3] {
        let steps = NodeList::path_steps(self.read(unit_offsets::PATH_LIST));
        let count = steps.len();
        if count == 0 {
            let default = super::default_position();
            return [default[0] as i32, default[1] as i32, default[2] as i32];
        }
        if count > 1 {
            steps.pop_back();
        }
        let last = steps.last();
        [get_from_memory(last + 8), get_from_memory(last + 12), get_from_memory(last + 16)]
    }

    /// The map tile at the grid position of the unit's last path step, and that position.
    fn path_destination(self) -> (u32, [i32; 3]) {
        let sentinel: u32 = self.read(unit_offsets::PATH_LIST);
        let last: u32 = get_from_memory(sentinel + 4);
        let position: [i32; 3] = [get_from_memory(last + 8), get_from_memory(last + 12), get_from_memory(last + 16)];
        (map_tile(position[0], position[1]), position)
    }

    /// `ZTUnit::updateBuildingTiles` (`0x0043b2f6`): points the unit at the building its building offset names. When the
    /// building has no open slot, a guest may voice a thought about it. Every outcome but a successful start runs the shared
    /// tail ([`Self::update_building_tiles_tail`]). The result is whether the unit set off for the building.
    pub fn update_building_tiles(self, abort: bool) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::update_building_tiles(self.0, abort) {
            return answer;
        }
        let offset: [i32; 3] = [self.read(unit_offsets::BUILDING_OFFSET), self.read(unit_offsets::BUILDING_OFFSET + 4), self.read(unit_offsets::BUILDING_OFFSET + 8)];
        if !self.is_need_or_fun_satisfier(self.read(unit_offsets::BUILDING_TILE), offset.as_ptr()) {
            return false;
        }
        if self.has_building_use_goal() {
            return false;
        }
        self.write(unit_offsets::NON_PATH_COST, get_from_memory::<u32>(base() + BUILDING_NON_PATH_COST_RVA));
        let building = self.building_at_offset();
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        if building == 0 || world == 0 {
            return self.update_building_tiles_tail(abort, false, building);
        }
        let building_pos: [i32; 3] = [get_from_memory(building + ENTITY_POS), get_from_memory(building + ENTITY_POS + 4), get_from_memory(building + ENTITY_POS + 8)];
        let mut slot: u8 = 0;
        let found = low_byte_bool(unsafe { FIND_NEAREST_OPEN_SLOT.hooked()(building as *const u32, self.0 as *const i32, &mut slot as *mut u8 as *const u32, building_pos.as_ptr() as u32) });
        if !found {
            let users = (get_from_memory::<i32>(building + BUILDING_USERS_END)).wrapping_sub(get_from_memory::<i32>(building + BUILDING_USERS_BEGIN)) / USER_RECORD_SIZE;
            let building_type: u32 = get_from_memory(building + unit_offsets::ENTITY_TYPE);
            if users > 0 && building_type != 0 && get_from_memory::<i32>(building + BUILDING_OCCUPANCY) >= get_from_memory::<i32>(building_type + TYPE_FULL_COUNT) {
                if unsafe { entity_type_matches(self.0, GUEST_TAG_RVA) } && self.read::<u32>(unit_offsets::THOUGHT_STATE) != 0 {
                    unsafe { F_GUEST_THOUGHT.hooked()(self.0 as *const u32, THOUGHT_BUILDING_FULL, building as *const i32, std::ptr::null()) };
                }
            }
            return self.update_building_tiles_tail(abort, false, building);
        }
        self.write(unit_offsets::BUILDING_SLOT, slot);
        let (destination_tile, destination) = self.path_destination();
        if destination_tile == 0 {
            return self.update_building_tiles_tail(abort, false, building);
        }
        let mut inside = [0i32; 3];
        let inside_position = unsafe { GET_INSIDE_POS.hooked()(building as *const u32, inside.as_mut_ptr() as *const u32, destination.as_ptr()) } as u32;
        let entrance_tile = map_tile(get_from_memory(inside_position), get_from_memory(inside_position + 4));
        if entrance_tile != 0 {
            let direction = unsafe { BFMAP_GET_DIRECTION.hooked()(destination_tile as i32, entrance_tile as i32) } as u32;
            let cost = call_slot_2_returning(self.0, slots::GET_EDGE_COST, destination_tile, direction) as i32;
            if cost >= get_from_memory::<i32>(base() + MAX_EDGE_COST_RVA) {
                return self.update_building_tiles_tail(abort, false, building);
            }
        }
        let building_type: u32 = get_from_memory(building + unit_offsets::ENTITY_TYPE);
        let goal: u32 = self.read(unit_offsets::GOAL_ID);
        if building_type != 0 && get_from_memory::<u8>(building_type + TYPE_EXHIBIT_FLAG) != 0 {
            if goal != GOAL_WALK_TO_EXHIBIT_BUILDING {
                call_slot_2(self.0, goal_slots::GOAL_SET, GOAL_WALK_TO_EXHIBIT_BUILDING, 1);
            }
        } else {
            if goal != GOAL_WALK_TO_BUILDING {
                call_slot_2(self.0, goal_slots::GOAL_SET, GOAL_WALK_TO_BUILDING, 1);
            }
            let step = self.penultimate_path_pos();
            call_slot_1(self.0, slots::SET_GRID_DEST, step.as_ptr() as u32);
        }
        let started = if call_slot_0(self.0, slots::IS_RETURNING) & 0xff == 0 {
            low_byte_bool(unsafe { BFUNIT_GO_TO_DEST.hooked()(self.0 as *const u32, false, 1) } as u32)
        } else {
            let source = call_slot_0(self.0, slots::GET_RETURN_BUILDING_STRING);
            let mut string = [0u32; 3];
            unsafe { BASIC_STRING_RANGE.hooked()(string.as_mut_ptr() as *const c_void, get_from_memory::<u32>(source) as *const u32, get_from_memory::<u32>(source + 4) as i32) };
            let result = unsafe { BFUNIT_GO_TO_DEST_STRING.hooked()(self.0 as *const u32, string.as_ptr(), 0, 1) };
            free_string(&string);
            result & 0xff != 0
        };
        if !started {
            return self.update_building_tiles_tail(abort, false, building);
        }
        for (index, value) in building_pos.into_iter().enumerate() {
            self.write(unit_offsets::POSITION_B + index as u32 * 4, value);
        }
        true
    }

    /// The shared tail of `updateBuildingTiles` (`0x005adc8a`): ends a building trick, then either aborts the goal (when asked),
    /// clears it, or, for a returning unit, leaves its building. `started` is the byte `updateBuildingTiles` carries.
    fn update_building_tiles_tail(self, abort: bool, started: bool, building: u32) -> bool {
        self.end_trick_when_building_need();
        if abort {
            call_slot_1(self.0, slots::ABORT_GOAL, 1);
            return started;
        }
        if call_slot_0(self.0, slots::IS_RETURNING) & 0xff == 0 {
            call_slot_0(self.0, goal_slots::CLEAR_GOAL);
            if unsafe { entity_type_matches(self.0, GUEST_TAG_RVA) } {
                self.write(GUEST_GAVE_UP, 1u8);
            }
            return started;
        }
        if building != 0 && unsafe { entity_type_matches(building, BUILDING_TAG_RVA) } {
            let building_type: u32 = get_from_memory(building + unit_offsets::ENTITY_TYPE);
            if get_from_memory::<u8>(building_type + TYPE_REMOVES_MEMBER) != 0 {
                unsafe { REMOVE_MEMBER.hooked()(building as *const u32, self.0 as i32) };
            }
        }
        call_slot_2(self.0, goal_slots::GOAL_SET, GOAL_LEAVE_BUILDING, 1);
        true
    }
}

#[detour_mod]
mod building_detours {
    use super::*;

    #[detour(HAS_BUILDING_USE_GOAL)]
    unsafe extern "thiscall" fn has_building_use_goal(this: *const u32) -> bool {
        ZTUnit(this as u32).has_building_use_goal()
    }

    #[detour(VALIDATE_BUILDING)]
    unsafe extern "thiscall" fn validate_building(this: *const u32, out: *const bool, check: bool) -> *const u32 {
        ZTUnit(this as u32).validate_building(out as *mut bool, check) as *const u32
    }

    #[detour(UPDATE_BUILDING_TILES)]
    unsafe extern "thiscall" fn update_building_tiles(this: *const u32, abort: bool) -> bool {
        ZTUnit(this as u32).update_building_tiles(abort)
    }

    #[detour(SET_BUILDING_TILE)]
    unsafe extern "thiscall" fn set_building_tile(this: *const u32, tile: *const u32) {
        ZTUnit(this as u32).set_building_tile(tile as u32);
    }

    #[detour(SET_BUILDING_GOAL)]
    unsafe extern "thiscall" fn set_building_goal(this: *const u32, tile: *const u32, slot: i32, offset: glam::IVec3) {
        ZTUnit(this as u32).set_building_goal(tile as u32, slot as u8, offset.to_array());
    }

    #[detour(F_EXIT_BUILDING)]
    unsafe extern "thiscall" fn f_exit_building(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).f_exit_building())
    }

    #[detour(F_RETURN_TO_BUILDING)]
    unsafe extern "thiscall" fn f_return_to_building(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).f_return_to_building())
    }

    #[detour(F_ENTER_BUILDING)]
    unsafe extern "thiscall" fn f_enter_building(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).f_enter_building())
    }

    #[detour(SET_BUILDING_USE_BS)]
    unsafe extern "thiscall" fn set_building_use_bs(this: *const u32, building: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).set_building_use_bs(building as u32))
    }

    #[detour(REMOVE_FROM_BUILDING_IF_PRESENT)]
    unsafe extern "thiscall" fn remove_from_building_if_present(this: *const u32) {
        ZTUnit(this as u32).remove_from_building_if_present();
    }

    #[detour(GET_HABITAT_FROM_VA)]
    unsafe extern "thiscall" fn get_habitat_from_va(this: *const u32) -> u32 {
        ZTUnit(this as u32).get_habitat_from_va()
    }

    #[detour(SET_EXIT_RETURN_BUILDING_BS)]
    unsafe extern "thiscall" fn set_exit_return_building_bs(this: *const u32, name: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).set_exit_return_building_bs(name))
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn has_building_use_goal(this: u32) -> bool {
            unsafe { super::HAS_BUILDING_USE_GOAL_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn validate_building(this: u32, out: *mut bool, check: bool) -> u32 {
            unsafe { super::VALIDATE_BUILDING_DETOUR.call(this as *const u32, out as *const bool, check) as u32 }
        }

        pub(crate) fn update_building_tiles(this: u32, abort: bool) -> bool {
            unsafe { super::UPDATE_BUILDING_TILES_DETOUR.call(this as *const u32, abort) }
        }

        pub(crate) fn set_building_tile(this: u32, tile: u32) {
            unsafe { super::SET_BUILDING_TILE_DETOUR.call(this as *const u32, tile as *const u32) }
        }

        pub(crate) fn set_building_goal(this: u32, tile: u32, slot: u8, offset: [i32; 3]) {
            unsafe { super::SET_BUILDING_GOAL_DETOUR.call(this as *const u32, tile as *const u32, i32::from(slot), glam::IVec3::from_array(offset)) }
        }

        pub(crate) fn f_exit_building(this: u32) -> bool {
            super::low_byte_bool(unsafe { super::F_EXIT_BUILDING_DETOUR.call(this as *const u32) })
        }

        pub(crate) fn f_return_to_building(this: u32) -> bool {
            super::low_byte_bool(unsafe { super::F_RETURN_TO_BUILDING_DETOUR.call(this as *const u32) })
        }

        pub(crate) fn f_enter_building(this: u32) -> bool {
            super::low_byte_bool(unsafe { super::F_ENTER_BUILDING_DETOUR.call(this as *const u32) })
        }

        pub(crate) fn set_building_use_bs(this: u32, building: u32) -> bool {
            super::low_byte_bool(unsafe { super::SET_BUILDING_USE_BS_DETOUR.call(this as *const u32, building as *const u32) })
        }

        pub(crate) fn remove_from_building_if_present(this: u32) {
            unsafe { super::REMOVE_FROM_BUILDING_IF_PRESENT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_habitat_from_va(this: u32) -> u32 {
            unsafe { super::GET_HABITAT_FROM_VA_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_exit_return_building_bs(this: u32, name: u32) -> bool {
            super::low_byte_bool(unsafe { super::SET_EXIT_RETURN_BUILDING_BS_DETOUR.call(this as *const u32, name as *const u32) })
        }
    }
}

/// Installs the building detours.
pub(super) fn init() {
    if let Err(e) = unsafe { building_detours::init_detours() } {
        error!("Failed to initialise ztunit building detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    building_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use building_detours::test_real;
