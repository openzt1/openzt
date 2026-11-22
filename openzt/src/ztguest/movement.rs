//! `ZTGuest`'s movement hooks and satisfier search: the pathfinder callbacks (`getTerrainCost`, `fencesOK`,
//! `tileFilter`, `pickRandomDest`), the building-need predicates, the searches that send a guest to a seat, a trash
//! can or a nearby need, and the grandstand helpers. Reads and writes guest memory in place through [`ZTGuest`].
//!
//! Every search ends the same way: the guest walks to the building's tile (virtual `+0x2e4`), takes the building's
//! position as its position record (`+0x214`) and asks `ZTUnit::updateBuildingTiles` to lock the building in.

use std::ffi::c_void;

use glam::i32::IVec3;
use openzt_detour::generated::{
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfmap::GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR,
    bftile::IS_IN_ZOO as BFTILE_IS_IN_ZOO,
    bfunit::{IS_NON_PATH_STEEP, IS_SAME_TILE},
    msvc_std::RAND,
    msvc_std_basic_string::BASIC_STRING_RANGE,
    msvc_std_listuint::LIST_UINT_CTOR,
    msvc_std_vectorint::INSERT_N as VECTOR_INT_INSERT_N,
    poolalloc::DEALLOCATE as POOLALLOC_DEALLOCATE,
    ztbuilding::{EMPTY_GRANDSTANDS, HAS_OPEN_SLOT},
    ztguest::{
        ADD_ITEM, ADD_TRASH, DROP, FENCES_OK, FIND_NEARBY_NEED_SATISFIER, FIND_PLACE_TO_SIT, FIND_TRASH_CAN, F_GUEST_THOUGHT, GET_TERRAIN_COST, GOTO_NEARBY_NEED,
        IN_GRANDSTAND, IS_NEED_SATISFIER, IS_PRESSING_NEED_SATISFIER, PICK_RANDOM_DEST, THROW_TRASH, TILE_FILTER, TRIGGER_GRANDSTAND_ESCAPE_0, TRIGGER_GRANDSTAND_ESCAPE_1,
    },
    zthabitat::GET_GATE_TILE_PASS_OUT,
    ztunit::{FIND_ANY_SATISFIER, FIND_NEAREST_SATISFIER, PICK_RANDOM_DEST as ZTUNIT_PICK_RANDOM_DEST, UPDATE_BUILDING_TILES},
    ztworldmgr::{GET_BUILDING_LIST, REMOVE_FROM_MEMBER_LISTS, SATISFIES},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{checks::need_names, ZTGuest};
use crate::{
    globals::get_module_base,
    util::{get_from_memory, low_byte_bool, save_to_memory},
    zthabitat::MAX_PATH_COST_RVA,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, vtable_slot},
    ztmegatilemgr::entity_type_matches,
};

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// `GLOBAL_ZTWorldMgr`.
const WORLD_MGR_RVA: u32 = 0x0023_8040;
/// `CAST_ZTAnimal`, `CAST_ZTBuilding` and the class tag `getTerrainCost` tests tile entities against, as RVAs.
const ANIMAL_TAG_RVA: u32 = 0x0023_8690;
const BUILDING_TAG_RVA: u32 = 0x0023_8680;
const FENCE_TAG_RVA: u32 = 0x0023_8660;
const TERRAIN_ENTITY_TAG_RVA: u32 = 0x0023_86b0;
/// The grandstand need name (`DAT_0063afb8`).
const GRANDSTAND_NAME_RVA: u32 = 0x0023_afb8;
/// `DAT_00646390`: the name `findNearbyNeedSatisfier` asks the world manager for buildings of.
const NEARBY_BUILDING_NAME_RVA: u32 = 0x0024_6390;
/// `DAT_006390d8`: the value the satisfier searches take as their second range argument.
const SEARCH_GLOBAL_RVA: u32 = 0x0023_90d8;
/// `DAT_0063c538`: the `{begin, end, cap}` vector `findNearbyNeedSatisfier` collects its candidates in.
const CANDIDATE_VECTOR_RVA: u32 = 0x0023_c538;

/// Tile fields.
mod tile_fields {
    /// The tile's entity slots: `+0x4`..`+0x10` hold the building quadrants, `+0x14`.. the edge fences.
    pub const EDGE_SLOTS: u32 = 0x14;
    pub const FLAGS_83: u32 = 0x83;
    pub const FLAGS_85: u32 = 0x85;
    /// Tile position words `findNearbyNeedSatisfier` measures distances in.
    pub const POSITION_X: u32 = 0x34;
    pub const POSITION_Y: u32 = 0x38;
}

/// Guest fields.
mod guest_fields {
    /// The need index the guest is walking towards.
    pub const NEED_INDEX: u32 = 0x24e;
    pub const HELD_TILE_ENTITY: u32 = 0x158;
    pub const GOAL_ID: u32 = 0x170;
    pub const WATER_FLAG: u32 = 0x210;
    pub const DEFAULT_COST: u32 = 0x230;
    pub const TILE_FILTER_MODE: u32 = 0x1bc;
    /// Byte every search sets on failure so the need checks stand down for the tick.
    pub const SEARCH_LOCK: u32 = 0x33e;
    pub const TRASH_FLAG: u32 = 0x33d;
    pub const LEAVING: u32 = 0x339;
}

/// Vtable slots the movement code dispatches.
mod movement_slots {
    pub const CLEAR_GOAL: u32 = 0x174;
    pub const GOAL_SET: u32 = 0x1d8;
    pub const GET_HABITAT_FROM_TILE: u32 = 0x248;
    pub const GET_HABITAT: u32 = 0x24c;
    pub const IS_IN_TANK: u32 = 0x268;
    pub const IS_PRESSING_NEED_SATISFIER: u32 = 0x27c;
    pub const FIND_NEARBY_NEED_SATISFIER: u32 = 0x280;
    pub const HAS_PRESSING_NEED: u32 = 0x28c;
    pub const GOTO_NEARBY_NEED: u32 = 0x290;
    pub const SET_RETURN_BUILDING_NAME_ID: u32 = 0x294;
    pub const SET_RETURN_BUILDING_STRING: u32 = 0x29c;
    pub const GO_TO_TILE: u32 = 0x2e4;
    pub const VALIDATE_BUILDING: u32 = 0x2e8;
    pub const CHECK_PREDATOR: u32 = 0x304;
    /// Slots of other classes' vtables.
    pub const WORLD_GET_UNIT: u32 = 0x2c;
    pub const HABITAT_IS_EXHIBIT: u32 = 0x20;
    pub const ENTITY_BLOCKS: u32 = 0xb4;
}

const GOAL_STANDING_DOWN: u32 = 0x71;
const GOAL_GRANDSTAND: u32 = 0x75;
const GOAL_LEAVING: u32 = 0x83;
const SEARCH_UNBOUNDED: i32 = -1;
/// The search radius of `gotoNearbyNeed`.
const NEARBY_NEED_RANGE: i32 = 10;
/// Carried-item id `addTrash` adds.
const TRASH_ITEM_ID: u32 = 0x18a6;
const THOUGHT_NO_SEAT: u32 = 0x276d;
const THOUGHT_CANNOT_LEAVE_WATER: u32 = 0x2948;
/// Entity-type byte on an animal that flags a grandstand escape (`+0x3b0`).
const ANIMAL_ESCAPE_FLAG_OFFSET: u32 = 0x3b0;
/// `+0x19c` of a trash-can target below this keeps a guest's trash search from throwing.
const TRASH_TARGET_RANGE_OFFSET: u32 = 0x19c;
const TRASH_TARGET_RANGE: i32 = 0x18;

/// The need names `isNeedSatisfier` accepts, in the order vanilla tests them.
const SATISFIED_NAMES: [u32; 8] = [
    need_names::HUNGER_A,
    need_names::THIRST,
    need_names::BATHROOM,
    need_names::SOUVENIR,
    need_names::TRASH,
    need_names::ENERGY,
    need_names::HUNGER_B,
    GRANDSTAND_NAME_RVA,
];

/// The needs `isPressingNeedSatisfier` reports, in the order vanilla tests them.
const PRESSING_NAMES: [u32; 4] = [need_names::THIRST, need_names::HUNGER_A, need_names::BATHROOM, need_names::ENERGY];

fn position_xy(value: i32) -> i32 {
    value.wrapping_rem(64)
}

fn position_z(value: i32) -> i32 {
    let remainder = value.wrapping_rem(16);
    if value < 0 {
        remainder + 16
    } else {
        remainder
    }
}

/// A virtual call on `this` taking two arguments and returning `EAX`.
fn call_slot_2_returning(this: u32, offset: u32, first: u32, second: u32) -> u32 {
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(vtable_slot(this, offset)) };
    function(this, first, second)
}

/// A virtual call on `this` taking three arguments and returning `EAX`.
fn call_slot_3_returning(this: u32, offset: u32, first: u32, second: u32, third: u32) -> u32 {
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32) -> u32>(vtable_slot(this, offset)) };
    function(this, first, second, third)
}

/// A virtual call on `this` taking four arguments and returning `EAX`.
fn call_slot_4_returning(this: u32, offset: u32, first: u32, second: u32, third: u32, fourth: u32) -> u32 {
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32>(vtable_slot(this, offset)) };
    function(this, first, second, third, fourth)
}

/// The edge fence of a tile on edge `direction`, or `0` when the slot is empty or holds something other than a fence.
fn edge_fence(tile: u32, direction: i32) -> u32 {
    let slot = tile.wrapping_add(tile_fields::EDGE_SLOTS).wrapping_add(((direction / 2) as u32).wrapping_mul(4));
    let fence: u32 = get_from_memory(slot);
    if fence != 0 && unsafe { entity_type_matches(fence, FENCE_TAG_RVA) } {
        fence
    } else {
        0
    }
}

/// Whether a fence's type name starts with `'g'`: a gate.
fn is_gate(fence: u32) -> bool {
    let entity_type: u32 = get_from_memory(fence + 0x128);
    let name: u32 = get_from_memory(entity_type + 0xa4);
    name != 0 && get_from_memory::<u8>(name) == b'g'
}

/// The building entity on `tile`'s building grid at `position` (`x`, `y` in `0..64`), or `0` outside the grid, on an
/// empty cell or on an entity that is not a `ZTBuilding`.
fn grid_building(tile: u32, position: u32) -> u32 {
    if tile == 0 {
        return 0;
    }
    let x: i32 = get_from_memory(position);
    let y: i32 = get_from_memory(position + 4);
    if !(0..0x40).contains(&x) || !(0..0x40).contains(&y) {
        return 0;
    }
    let cell = (y / 32 + (x / 32) * 2) as u32;
    let building: u32 = get_from_memory(tile + 4 + cell * 4);
    if building != 0 && unsafe { entity_type_matches(building, BUILDING_TAG_RVA) } {
        building
    } else {
        0
    }
}

fn satisfies(building: u32, name_rva: u32) -> bool {
    low_byte_bool(unsafe { SATISFIES.hooked()(building as i32, (base() + name_rva) as *const i8, std::ptr::null()) })
}

/// Squared distance between two tiles' position words, `i32::MAX` when either is missing.
fn tile_distance_squared(from: u32, to: u32) -> i32 {
    if from == 0 || to == 0 {
        return i32::MAX;
    }
    let dx = get_from_memory::<i32>(to + tile_fields::POSITION_X).wrapping_sub(get_from_memory::<i32>(from + tile_fields::POSITION_X));
    let dy = get_from_memory::<i32>(to + tile_fields::POSITION_Y).wrapping_sub(get_from_memory::<i32>(from + tile_fields::POSITION_Y));
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
}

/// Overrides the live tests install on the three functions `ztguest/checks.rs`'s live test scripts. Each takes the
/// call's arguments and answers `Some` when it has handled the call.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod overrides {
    use std::sync::Mutex;

    pub type FindPlace = fn(this: u32, arg: u32) -> Option<bool>;
    pub type FindTrash = fn(this: u32, arg: u32) -> Option<bool>;
    pub type Escape = fn(this: u32, arg: i32) -> bool;
    pub type InGrandstand = fn(this: u32) -> Option<bool>;

    pub static FIND_PLACE: Mutex<Option<FindPlace>> = Mutex::new(None);
    pub static FIND_TRASH: Mutex<Option<FindTrash>> = Mutex::new(None);
    pub static ESCAPE: Mutex<Option<Escape>> = Mutex::new(None);
    pub static IN_GRANDSTAND: Mutex<Option<InGrandstand>> = Mutex::new(None);
}

impl ZTGuest {
    fn own_tile(self) -> u32 {
        unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) as u32 }
    }

    fn habitat_from_tile(self, tile: u32) -> u32 {
        call_slot_1(self.0, movement_slots::GET_HABITAT_FROM_TILE, tile)
    }

    fn clear_goal_virtual(self) {
        call_slot_0(self.0, movement_slots::CLEAR_GOAL);
    }

    fn search_global() -> i32 {
        get_from_memory(base() + SEARCH_GLOBAL_RVA)
    }

    /// Runs `findNearestSatisfier` (`nearest`) or `findAnySatisfier` for `name_rva`, returning whether one was found
    /// and the building.
    fn run_satisfier_search(self, nearest: bool, name_rva: u32) -> (bool, u32) {
        let mut target: u32 = 0;
        let tile = self.own_tile() as i32;
        let name = (base() + name_rva) as *const i32;
        let found = if nearest {
            unsafe {
                FIND_NEAREST_SATISFIER.hooked()(self.0 as *const u32, tile, name, &mut target as *mut u32 as *const u32, Self::search_global(), SEARCH_UNBOUNDED)
            }
        } else {
            unsafe { FIND_ANY_SATISFIER.hooked()(self.0 as *const u32, tile, name, &mut target as *mut u32 as *const i32, Self::search_global(), SEARCH_UNBOUNDED) }
        };
        (found, target)
    }

    /// Walks towards `building` and adopts its position; returns whether `updateBuildingTiles` accepted it.
    fn commit_to_building(self, building: u32, flag: bool) -> bool {
        let tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
        call_slot_1(self.0, movement_slots::GO_TO_TILE, tile);
        self.take_building_position(building);
        unsafe { UPDATE_BUILDING_TILES.hooked()(self.0 as *const u32, flag) }
    }

    fn throw_trash(self) {
        unsafe { THROW_TRASH.hooked()(self.0 as *const u32, std::ptr::null()) };
    }

    /// `ZTGuest::getTerrainCost` (`0x0043a8fb`): the pathfinder's cost of stepping onto `tile`. The guest's own tile
    /// costs `1`; a tile outside the zoo costs the path-cost sentinel; so does a water tile for a guest not allowed
    /// onto water (not recovering, not in the grandstand goal) and a dry one for a guest travelling on water. A tile
    /// flagged `0x8` costs `1` when the guest holds a flagged tile too (a guest in goal `0x71`/`0x75` otherwise pays
    /// the guest's default cost); a steep tile costs at least the sentinel minus 4; a tile with the predator flag
    /// (`+0x85` bit `0x8`) costs that too when an animal on it frightens the guest, and loses the flag when none does.
    pub fn get_terrain_cost(self, tile: u32) -> i32 {
        let this = self.0 as *const u32;
        let max_cost: i32 = get_from_memory(base() + MAX_PATH_COST_RVA);
        if unsafe { IS_SAME_TILE.hooked()(this, tile as *const u32) } != 0 {
            return 1;
        }
        if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.hooked()(tile as *const u32, 0) }) {
            return max_cost;
        }
        let own = self.own_tile();
        let flags = |tile: u32, offset: u32| if tile == 0 { 0 } else { get_from_memory::<u8>(tile + offset) };
        let target_flags = flags(tile, tile_fields::FLAGS_83);
        let own_flags = flags(own, tile_fields::FLAGS_83);
        let target_wet = target_flags & 3 != 0;
        let goal = self.read::<u32>(guest_fields::GOAL_ID);
        if own_flags & 3 == 0 && target_wet && !self.is_water_recovering() && goal != GOAL_GRANDSTAND {
            return max_cost;
        }
        if self.is_water_traveling() && !target_wet && self.read::<u8>(guest_fields::WATER_FLAG) == 0 && !self.is_water_recovering() {
            return max_cost;
        }
        let target_raised = target_flags >> 3 & 1 != 0;
        let own_raised = own != 0 && own_flags & 8 != 0;
        let held: u32 = self.read(guest_fields::HELD_TILE_ENTITY);
        let held_raised = held != 0 && get_from_memory::<u8>(held + tile_fields::FLAGS_83) & 8 != 0;
        let unit_cost = if held_raised && own_raised {
            if target_raised {
                true
            } else if goal != GOAL_STANDING_DOWN && goal != GOAL_GRANDSTAND {
                return max_cost;
            } else {
                false
            }
        } else {
            target_raised
        };
        let mut cost: i32 = if unit_cost || flags(self.own_tile(), tile_fields::FLAGS_85) & 0x20 != 0 || target_wet {
            1
        } else {
            self.read(guest_fields::DEFAULT_COST)
        };
        let steep_floor = max_cost.wrapping_sub(4);
        if low_byte_bool(unsafe { IS_NON_PATH_STEEP.hooked()(tile as i32) }) {
            cost = cost.max(steep_floor);
        }
        if get_from_memory::<u8>(tile + tile_fields::FLAGS_85) & 8 == 0 {
            return cost;
        }
        let mut node: u32 = get_from_memory(get_from_memory::<u32>(tile));
        while node != get_from_memory::<u32>(tile) {
            let entity: u32 = get_from_memory(node + 8);
            let animal = if entity != 0 && unsafe { entity_type_matches(entity, TERRAIN_ENTITY_TAG_RVA) } { entity } else { 0 };
            if low_byte_bool(call_slot_2_returning(self.0, movement_slots::CHECK_PREDATOR, animal, 0)) {
                return cost.max(steep_floor);
            }
            node = get_from_memory(node);
        }
        save_to_memory(tile + tile_fields::FLAGS_85, get_from_memory::<u8>(tile + tile_fields::FLAGS_85) & 0xf7);
        cost
    }

    /// `ZTGuest::fencesOK` (`0x0043a85a`): whether the step from `tile` in `direction` crosses no fence a guest cannot
    /// pass. Clears both bytes at `out_a`/`out_b` first. A missing tile or neighbour is `false`; direction `-1` is
    /// `true`. With a fence on the tile's edge or on the neighbour's far edge (`(direction - 4) & 7`) both must be
    /// gates (type name starting with `'g'`), and a step out of an exhibit habitat's tile into one outside it is
    /// refused.
    pub fn fences_ok(self, tile: u32, direction: i32, out_a: u32, out_b: u32) -> bool {
        save_to_memory(out_a, 0u8);
        if out_b != 0 {
            save_to_memory(out_b, 0u8);
        }
        if tile == 0 {
            return false;
        }
        if direction == -1 {
            return true;
        }
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, direction as u32) };
        if neighbour == 0 {
            return false;
        }
        let near = edge_fence(tile, direction);
        let far = edge_fence(neighbour, direction.wrapping_sub(4) & 7);
        if near == 0 && far == 0 {
            return true;
        }
        let exhibit = |tile: u32| {
            let habitat = self.habitat_from_tile(tile);
            habitat != 0 && get_from_memory::<u8>(habitat + 0x2c) != 0
        };
        if !exhibit(neighbour) && exhibit(tile) {
            return false;
        }
        (near == 0 || is_gate(near)) && (far == 0 || is_gate(far))
    }

    /// Whether one of a tile's four entity slots is empty or holds an entity whose virtual `+0xb4` answers true.
    fn tile_slot_open(tile: u32) -> bool {
        (1..=4u32).any(|slot| {
            let entity: u32 = get_from_memory(tile + slot * 4);
            entity == 0 || low_byte_bool(call_slot_0(entity, movement_slots::ENTITY_BLOCKS))
        })
    }

    /// `ZTGuest::tileFilter` (`0x00428f76`): whether `tile` is in the zoo, has an open entity slot, is walkable for
    /// this guest (the guest's own mode is `3`, or `flag` is set with the guest's own tile unflagged, or the tile is
    /// flagged `0x8`) and the habitat rules allow the move: not into a tile whose habitat reports the exhibit
    /// virtual `+0x20`, and into the guest's own habitat or an exhibit one.
    pub fn tile_filter(self, tile: u32, flag: bool) -> bool {
        let own = self.own_tile();
        if own == 0 || tile == 0 {
            return false;
        }
        let own_unflagged = get_from_memory::<u8>(own + tile_fields::FLAGS_83) & 8 == 0;
        let target_habitat = self.habitat_from_tile(tile);
        let own_habitat = self.habitat_from_tile(own);
        let (mut target_exhibit, mut own_open, mut own_blocks) = (false, false, false);
        if target_habitat != 0 && own_habitat != 0 {
            target_exhibit = get_from_memory::<u8>(target_habitat + 0x2c) != 0;
            own_open = get_from_memory::<u8>(own_habitat + 0x2c) == 0;
            own_blocks = low_byte_bool(call_slot_0(own_habitat, movement_slots::HABITAT_IS_EXHIBIT));
        }
        if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.hooked()(tile as *const u32, 0) }) || !Self::tile_slot_open(tile) {
            return false;
        }
        let walkable = self.read::<i32>(guest_fields::TILE_FILTER_MODE) == 3
            || (flag && own_unflagged)
            || get_from_memory::<u8>(tile + tile_fields::FLAGS_83) & 8 != 0;
        if !walkable || own_blocks {
            return false;
        }
        own_open || target_exhibit
    }

    /// `ZTGuest::pickRandomDest` (`0x0042909f`): a guest in a habitat that is not an exhibit (`+0x2c` clear) is sent
    /// through the habitat's pass-out gate tile; any other guest defers to `ZTUnit::pickRandomDest`. Without a gate
    /// tile there is no destination.
    pub fn pick_random_dest(self, position: *const IVec3) -> bool {
        let habitat = call_slot_0(self.0, movement_slots::GET_HABITAT);
        if habitat == 0 || get_from_memory::<u8>(habitat + 0x2c) != 0 {
            return low_byte_bool(unsafe { ZTUNIT_PICK_RANDOM_DEST.hooked()(self.0 as *const u32, position) });
        }
        let gate = unsafe { GET_GATE_TILE_PASS_OUT.hooked()(habitat as *const u32, self.0 as *const u32) } as u32;
        if gate == 0 {
            return false;
        }
        for word in 0..3u32 {
            save_to_memory(position as u32 + word * 4, get_from_memory::<u32>(gate + 0x34 + word * 4));
        }
        true
    }

    /// `ZTGuest::isNeedSatisfier` (`0x0043a819`): whether the building on `tile`'s grid at `position` provides one of
    /// the eight needs the guest knows.
    pub fn is_need_satisfier(self, tile: u32, position: u32) -> bool {
        let building = grid_building(tile, position);
        building != 0 && SATISFIED_NAMES.iter().any(|&name| satisfies(building, name))
    }

    /// `ZTGuest::isPressingNeedSatisfier` (`0x00618f29`, virtual `+0x27c`): like [`Self::is_need_satisfier`] for the
    /// four pressing needs (thirst, hunger, bathroom, energy); a match stores the need's index at `out`.
    pub fn is_pressing_need_satisfier(self, out: u32, tile: u32, position: u32) -> bool {
        let building = grid_building(tile, position);
        if building == 0 {
            return false;
        }
        for name in PRESSING_NAMES {
            if satisfies(building, name) {
                save_to_memory(out, Self::need_index(name));
                return true;
            }
        }
        false
    }

    /// The building the guest's `validateBuilding` slot (`+0x2e8`) reports, `0` for none.
    fn validated_building(self) -> u32 {
        let mut flag: u8 = 0;
        call_slot_2_returning(self.0, movement_slots::VALIDATE_BUILDING, &mut flag as *mut u8 as u32, 0)
    }

    /// `ZTGuest::inGrandstand` (`0x0050d152`): whether the guest's validated building provides the grandstand need.
    pub fn in_grandstand(self) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = overrides::IN_GRANDSTAND.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        let building = self.validated_building();
        building != 0 && satisfies(building, GRANDSTAND_NAME_RVA)
    }

    /// `ZTGuest::triggerGrandstandEscape` (`0x0048e054`): a guest in a grandstand empties it.
    pub fn trigger_grandstand_escape(self) {
        if !self.in_grandstand() {
            return;
        }
        let building = self.validated_building();
        if building != 0 {
            unsafe { EMPTY_GRANDSTANDS.hooked()(building as *const u32) };
        }
    }

    /// `ZTGuest::triggerGrandstandEscape(id)` (`0x0050d264`): when `id` names an animal with the escape flag
    /// (`+0x3b0`), empties the guest's grandstand.
    pub fn trigger_grandstand_escape_for(self, id: i32) {
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let unit = call_slot_1(world, movement_slots::WORLD_GET_UNIT, id as u32);
        if unit != 0 && unsafe { entity_type_matches(unit, ANIMAL_TAG_RVA) } && get_from_memory::<u8>(unit + ANIMAL_ESCAPE_FLAG_OFFSET) != 0 {
            self.trigger_grandstand_escape();
        }
    }

    /// Fails a search the way every search fails: the guest forgets its goal and the need checks stand down.
    fn abandon_search(self) {
        self.clear_goal_virtual();
        self.write(guest_fields::SEARCH_LOCK, 1u8);
    }

    /// `ZTGuest::gotoNearbyNeed` (`0x0046b7d6`, virtual `+0x290`): asks the nearby-need slot (`+0x280`, range `10`) for
    /// a building and a need index, and walks to it when both exist.
    pub fn goto_nearby_need(self) -> bool {
        let mut need: u8 = 0xff;
        let mut building: u32 = 0;
        let tile = self.own_tile();
        let found = call_slot_4_returning(
            self.0,
            movement_slots::FIND_NEARBY_NEED_SATISFIER,
            tile,
            &mut need as *mut u8 as u32,
            &mut building as *mut u32 as u32,
            NEARBY_NEED_RANGE as u32,
        );
        if !low_byte_bool(found) || building == 0 || need == 0xff {
            return false;
        }
        self.write(guest_fields::NEED_INDEX, need);
        let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
        call_slot_1(self.0, movement_slots::GO_TO_TILE, building_tile);
        self.take_building_position(building);
        true
    }

    /// `ZTGuest::findPlaceToSit` (`0x005adfac`): finds an energy building (the nearest one when the low byte of `arg`
    /// is set, any otherwise) and commits to it. Failing, the guest drops its goal, stands down for the tick and
    /// thinks `0x276d`.
    pub fn find_place_to_sit(self, arg: u32) -> bool {
        let nearest = arg as u8 != 0;
        let (found, target) = self.run_satisfier_search(nearest, need_names::ENERGY);
        if found {
            if target == 0 {
                return true;
            }
            if self.commit_to_building(target, nearest) {
                let index = Self::need_index(need_names::ENERGY);
                if index != 0xff {
                    self.write(guest_fields::NEED_INDEX, index);
                }
                return true;
            }
            self.clear_goal_virtual();
        } else {
            self.clear_goal_virtual();
        }
        self.write(guest_fields::SEARCH_LOCK, 1u8);
        self.thought(THOUGHT_NO_SEAT, 0);
        false
    }

    /// `ZTGuest::findTrashCan` (`0x0059d5d3`): like [`Self::find_place_to_sit`] for the trash need. A guest that
    /// already holds the trash flag (`+0x33d`) and searches for any can keeps going when the can's `+0x19c` is below
    /// `0x18`; a failed search (or a rejected can) throws the trash away unless the nearest can was asked for.
    pub fn find_trash_can(self, arg: u32) -> bool {
        let nearest = arg as u8 != 0;
        let (found, target) = self.run_satisfier_search(nearest, need_names::TRASH);
        if !found {
            if !nearest {
                self.throw_trash();
            }
            self.abandon_search();
            return false;
        }
        if self.read::<u8>(guest_fields::TRASH_FLAG) != 0 && !nearest && target != 0 {
            if get_from_memory::<i32>(target + TRASH_TARGET_RANGE_OFFSET) < TRASH_TARGET_RANGE {
                return true;
            }
            self.throw_trash();
            self.abandon_search();
            return false;
        }
        if target == 0 {
            return true;
        }
        if !self.commit_to_building(target, nearest) {
            if !nearest {
                self.throw_trash();
            }
            self.abandon_search();
            return false;
        }
        let index = Self::need_index(need_names::TRASH);
        if index != 0xff {
            self.write(guest_fields::NEED_INDEX, index);
        }
        true
    }

    /// `ZTGuest::addTrash` (`0x0059d2c9`): adds the item `0x18a6` named `"trash"`. `addItem` frees the name.
    pub fn add_trash(self) -> bool {
        const NAME: &[u8; 6] = b"trash\0";
        let mut name = [0u32; 3];
        let start = NAME.as_ptr();
        unsafe {
            BASIC_STRING_RANGE.hooked()(name.as_mut_ptr() as *const c_void, start as *const u32, start.add(NAME.len() - 1) as i32);
            ADD_ITEM.hooked()(self.0 as *const u32, TRASH_ITEM_ID as *const u8, name[0] as *const u32, name[1] as *const u32, name[2] as i32)
        }
    }

    /// `ZTGuest::drop` (`0x0046b6a3`): removes the guest from the world manager's member lists, clears its return
    /// building (id `0`, empty string) and either sends a guest on water-flagged ground into the leaving goal or
    /// looks for a pressing need's building; a guest with neither stands down.
    pub fn drop_guest(self) {
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        unsafe { REMOVE_FROM_MEMBER_LISTS.hooked()(world as *const u32, self.0 as i32) };
        call_slot_1(self.0, movement_slots::SET_RETURN_BUILDING_NAME_ID, 0);
        let mut empty = [0u32; 3];
        let start = b"\0".as_ptr();
        unsafe {
            BASIC_STRING_RANGE.hooked()(empty.as_mut_ptr() as *const c_void, start as *const u32, start as i32);
            call_slot_1(self.0, movement_slots::SET_RETURN_BUILDING_STRING, empty.as_ptr() as u32);
            if empty[0] != 0 {
                POOLALLOC_DEALLOCATE.original()(empty[0] as *const u32, empty[2].wrapping_sub(empty[0]));
            }
        }
        let in_tank = low_byte_bool(call_slot_0(self.0, movement_slots::IS_IN_TANK));
        let own = self.own_tile();
        if !in_tank && own != 0 && get_from_memory::<u8>(own + tile_fields::FLAGS_83) & 3 != 0 {
            self.write(guest_fields::LEAVING, 0u8);
            call_slot_2(self.0, movement_slots::GOAL_SET, GOAL_LEAVING, 1);
            unsafe { F_GUEST_THOUGHT.hooked()(self.0 as *const u32, THOUGHT_CANNOT_LEAVE_WATER, std::ptr::null(), std::ptr::null()) };
            return;
        }
        if low_byte_bool(call_slot_0(self.0, movement_slots::HAS_PRESSING_NEED)) && low_byte_bool(call_slot_0(self.0, movement_slots::GOTO_NEARBY_NEED)) {
            return;
        }
        self.write(guest_fields::SEARCH_LOCK, 1u8);
    }

    /// `ZTGuest::findNearbyNeedSatisfier` (`0x0046b818`, virtual `+0x280`): among the buildings of the nearby-building
    /// list that are in the same habitat as `tile`, have an open slot for the guest and satisfy a pressing need
    /// (virtual `+0x27c`, which also stores the need's index at `out_need`), collects those at the smallest squared
    /// distance (within `range`) in the shared candidate vector and stores a random one at `out_building`.
    pub fn find_nearby_need_satisfier(self, tile: u32, out_need: u32, out_building: u32, range: i32) -> bool {
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let candidates = base() + CANDIDATE_VECTOR_RVA;
        let allocator_dummy: u8 = 0;
        let mut list: u32 = 0;
        unsafe {
            LIST_UINT_CTOR.hooked()(&mut list as *mut u32 as *const c_void, &allocator_dummy as *const u8 as u32);
            GET_BUILDING_LIST.hooked()(world as *const u32, (base() + NEARBY_BUILDING_NAME_RVA) as *const i32, &mut list as *mut u32 as *const i32);
        }
        let mut best = range.wrapping_mul(range);
        let mut found = false;
        let head = list;
        let mut node: u32 = get_from_memory(head);
        while node != head {
            let building: u32 = get_from_memory(node + 8);
            if building != 0
                && let Some(distance) = self.nearby_candidate(building, tile, out_need, best)
            {
                found = true;
                if distance < best {
                    save_to_memory(candidates + 4, get_from_memory::<u32>(candidates));
                }
                Self::push_candidate(candidates, building);
                best = distance;
            }
            node = get_from_memory(node);
        }
        if found {
            let begin: u32 = get_from_memory(candidates);
            let end: u32 = get_from_memory(candidates + 4);
            let count = (end.wrapping_sub(begin) as i32) >> 2;
            let roll = unsafe { RAND.hooked()() } as i32;
            let pick: u32 = get_from_memory(begin + (roll.wrapping_rem(count) as u32).wrapping_mul(4));
            save_to_memory(out_building, pick);
        }
        let mut node: u32 = get_from_memory(head);
        while node != head {
            let next: u32 = get_from_memory(node);
            if node != 0 {
                unsafe { POOLALLOC_DEALLOCATE.original()(node as *const u32, 0xc) };
            }
            node = next;
        }
        save_to_memory(head, head);
        save_to_memory(head + 4, head);
        unsafe { POOLALLOC_DEALLOCATE.original()(head as *const u32, 0xc) };
        found
    }

    /// The squared distance of `building` when it is a candidate for `findNearbyNeedSatisfier`: same habitat as
    /// `tile`, an open slot, a pressing-need match and no farther than `best`.
    fn nearby_candidate(self, building: u32, tile: u32, out_need: u32, best: i32) -> Option<i32> {
        let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
        if self.habitat_from_tile(building_tile) != self.habitat_from_tile(tile) {
            return None;
        }
        if !low_byte_bool(unsafe { HAS_OPEN_SLOT.hooked()(building as *const u32, self.0 as *const u32) }) {
            return None;
        }
        let distance = tile_distance_squared(tile, building_tile);
        let position = [
            position_xy(get_from_memory(building + 0x114)),
            position_xy(get_from_memory(building + 0x118)),
            position_z(get_from_memory(building + 0x11c)),
        ];
        let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
        if !low_byte_bool(call_slot_3_returning(self.0, movement_slots::IS_PRESSING_NEED_SATISFIER, out_need, building_tile, position.as_ptr() as u32)) {
            return None;
        }
        (distance <= best).then_some(distance)
    }

    /// Appends `building` to the `{begin, end, cap}` word vector at `vector`, growing through vanilla's `insert_n`.
    fn push_candidate(vector: u32, building: u32) {
        let end: u32 = get_from_memory(vector + 4);
        let cap: u32 = get_from_memory(vector + 8);
        if end == cap {
            let value = building;
            unsafe { VECTOR_INT_INSERT_N.hooked()(vector as *const i32, end as *const u32, &value as *const u32, 1) };
        } else {
            save_to_memory(end, building);
            save_to_memory(vector + 4, end + 4);
        }
    }
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_TERRAIN_COST)]
    unsafe extern "thiscall" fn get_terrain_cost(this: *const u32, tile: *const u32) -> i32 {
        ZTGuest(this as u32).get_terrain_cost(tile as u32)
    }

    #[detour(FENCES_OK)]
    unsafe extern "thiscall" fn fences_ok(this: *const u32, tile: *const u32, direction: u32, out_a: *const bool, out_b: *const bool) -> bool {
        ZTGuest(this as u32).fences_ok(tile as u32, direction as i32, out_a as u32, out_b as u32)
    }

    #[detour(TILE_FILTER)]
    unsafe extern "thiscall" fn tile_filter(this: *const u32, tile: *const u32, flag: bool) -> bool {
        ZTGuest(this as u32).tile_filter(tile as u32, flag)
    }

    #[detour(PICK_RANDOM_DEST)]
    unsafe extern "thiscall" fn pick_random_dest(this: *const u32, position: *const IVec3) -> bool {
        ZTGuest(this as u32).pick_random_dest(position)
    }

    #[detour(IS_NEED_SATISFIER)]
    unsafe extern "thiscall" fn is_need_satisfier(this: *const u32, tile: *const u32, position: *const IVec3) -> bool {
        ZTGuest(this as u32).is_need_satisfier(tile as u32, position as u32)
    }

    #[detour(IS_PRESSING_NEED_SATISFIER)]
    unsafe extern "thiscall" fn is_pressing_need_satisfier(this: *const u32, out: *const u8, tile: *const u32, position: *const IVec3) -> bool {
        ZTGuest(this as u32).is_pressing_need_satisfier(out as u32, tile as u32, position as u32)
    }

    #[detour(FIND_NEARBY_NEED_SATISFIER)]
    unsafe extern "thiscall" fn find_nearby_need_satisfier(this: *const u32, tile: *const u32, out_need: *const u8, out_building: *const u32, range: i32) -> bool {
        ZTGuest(this as u32).find_nearby_need_satisfier(tile as u32, out_need as u32, out_building as u32, range)
    }

    #[detour(GOTO_NEARBY_NEED)]
    unsafe extern "thiscall" fn goto_nearby_need(this: *const u32) -> bool {
        ZTGuest(this as u32).goto_nearby_need()
    }

    #[detour(FIND_PLACE_TO_SIT)]
    unsafe extern "thiscall" fn find_place_to_sit(this: *const u32, arg: *const u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(handled) = super::overrides::FIND_PLACE.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(this as u32, arg as u32)) {
                return handled;
            }
        }
        ZTGuest(this as u32).find_place_to_sit(arg as u32)
    }

    #[detour(FIND_TRASH_CAN)]
    unsafe extern "thiscall" fn find_trash_can(this: *const u32, arg: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(handled) = super::overrides::FIND_TRASH.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(this as u32, arg)) {
                return handled;
            }
        }
        ZTGuest(this as u32).find_trash_can(arg)
    }

    #[detour(ADD_TRASH)]
    unsafe extern "thiscall" fn add_trash(this: *const u32) -> bool {
        ZTGuest(this as u32).add_trash()
    }

    #[detour(DROP)]
    unsafe extern "thiscall" fn drop_guest(this: *const u32) {
        ZTGuest(this as u32).drop_guest();
    }

    #[detour(IN_GRANDSTAND)]
    unsafe extern "thiscall" fn in_grandstand(this: *const u32) -> bool {
        ZTGuest(this as u32).in_grandstand()
    }

    #[detour(TRIGGER_GRANDSTAND_ESCAPE_0)]
    unsafe extern "thiscall" fn trigger_grandstand_escape_0(this: *const u32) {
        ZTGuest(this as u32).trigger_grandstand_escape();
    }

    #[detour(TRIGGER_GRANDSTAND_ESCAPE_1)]
    unsafe extern "thiscall" fn trigger_grandstand_escape_1(this: *const u32, id: i32) {
        #[cfg(feature = "reimplementation-tests")]
        {
            if super::overrides::ESCAPE.lock().ok().and_then(|hook| *hook).is_some_and(|hook| hook(this as u32, id)) {
                return;
            }
        }
        ZTGuest(this as u32).trigger_grandstand_escape_for(id);
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use glam::i32::IVec3;

        pub(crate) fn get_terrain_cost(this: u32, tile: u32) -> i32 {
            unsafe { super::GET_TERRAIN_COST_DETOUR.call(this as *const u32, tile as *const u32) }
        }
        pub(crate) fn fences_ok(this: u32, tile: u32, direction: u32, out_a: u32, out_b: u32) -> bool {
            unsafe { super::FENCES_OK_DETOUR.call(this as *const u32, tile as *const u32, direction, out_a as *const bool, out_b as *const bool) }
        }
        pub(crate) fn tile_filter(this: u32, tile: u32, flag: bool) -> bool {
            unsafe { super::TILE_FILTER_DETOUR.call(this as *const u32, tile as *const u32, flag) }
        }
        pub(crate) fn pick_random_dest(this: u32, position: *const IVec3) -> bool {
            unsafe { super::PICK_RANDOM_DEST_DETOUR.call(this as *const u32, position) }
        }
        pub(crate) fn is_need_satisfier(this: u32, tile: u32, position: *const IVec3) -> bool {
            unsafe { super::IS_NEED_SATISFIER_DETOUR.call(this as *const u32, tile as *const u32, position) }
        }
        pub(crate) fn is_pressing_need_satisfier(this: u32, out: u32, tile: u32, position: *const IVec3) -> bool {
            unsafe { super::IS_PRESSING_NEED_SATISFIER_DETOUR.call(this as *const u32, out as *const u8, tile as *const u32, position) }
        }
        pub(crate) fn find_nearby_need_satisfier(this: u32, tile: u32, out_need: u32, out_building: u32, range: i32) -> bool {
            unsafe { super::FIND_NEARBY_NEED_SATISFIER_DETOUR.call(this as *const u32, tile as *const u32, out_need as *const u8, out_building as *const u32, range) }
        }
        pub(crate) fn goto_nearby_need(this: u32) -> bool {
            unsafe { super::GOTO_NEARBY_NEED_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn find_place_to_sit(this: u32, arg: u32) -> bool {
            unsafe { super::FIND_PLACE_TO_SIT_DETOUR.call(this as *const u32, arg as *const u32) }
        }
        pub(crate) fn find_trash_can(this: u32, arg: u32) -> bool {
            unsafe { super::FIND_TRASH_CAN_DETOUR.call(this as *const u32, arg) }
        }
        pub(crate) fn add_trash(this: u32) -> bool {
            unsafe { super::ADD_TRASH_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn drop_guest(this: u32) {
            unsafe { super::DROP_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn in_grandstand(this: u32) -> bool {
            unsafe { super::IN_GRANDSTAND_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn trigger_grandstand_escape_0(this: u32) {
            unsafe { super::TRIGGER_GRANDSTAND_ESCAPE_0_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn trigger_grandstand_escape_1(this: u32, id: i32) {
            unsafe { super::TRIGGER_GRANDSTAND_ESCAPE_1_DETOUR.call(this as *const u32, id) }
        }
    }
}

/// Registers the movement detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest movement detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub(crate) use super::detours::test_real as real;

    /// Whether vanilla's `fencesOK` would dereference a null habitat for this step: a fenced edge whose neighbour
    /// lies outside every habitat, or (with the neighbour not an exhibit) whose own tile does. The port treats a
    /// missing habitat as "not an exhibit".
    pub fn real_fences_ok_would_fault(guest: super::ZTGuest, tile: u32, direction: i32) -> bool {
        use super::*;
        if tile == 0 || direction == -1 {
            return false;
        }
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, direction as u32) };
        if neighbour == 0 || (edge_fence(tile, direction) == 0 && edge_fence(neighbour, direction.wrapping_sub(4) & 7) == 0) {
            return false;
        }
        let neighbour_habitat = guest.habitat_from_tile(neighbour);
        neighbour_habitat == 0 || (get_from_memory::<u8>(neighbour_habitat + 0x2c) == 0 && guest.habitat_from_tile(tile) == 0)
    }

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }
}
