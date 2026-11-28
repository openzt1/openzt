//! `ZTUnit` movement and target methods (group F of `plans/ztunit-scoping-plan.md`): the target setters, the facing
//! helpers and the tile-blocking predicates. Each is the base behaviour a subclass reaches directly or through the
//! vtable.

use openzt_detour::generated::{
    bfaimgr::F_RANDOM_WALK,
    bfentity::GET_GRID_POS,
    bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION, GET_DIRECTION_1 as BFMAP_GET_DIRECTION_OF_OFFSET, GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR, TILE_TO_WORLD},
    bftile::GET_CORNER_ELEVATION,
    bfunit::{
        CHECK_ELEVATION as BFUNIT_CHECK_ELEVATION, CLIFFS_OK as BFUNIT_CLIFFS_OK, FACE_TOWARD_ENTITY as BFUNIT_FACE_TOWARD_ENTITY, FACE_TOWARD_TILE as BFUNIT_FACE_TOWARD_TILE,
        GET_PATH_0 as BFUNIT_GET_PATH, GO_TO_DEST_1 as BFUNIT_GO_TO_DEST, VALIDATE_POSITION as BFUNIT_VALIDATE_POSITION,
    },
    ztunit::{
        CAN_IGNORE_BLOCKING, CHECK_ELEVATION, CLIFFS_OK, DO_RANDOM_WALK, FACE_TOWARD_TARGET, FACE_TOWARD_TARGET_UNIT, GET_NON_PATH_COST, GET_PATH_TO_TARGET, GET_PATH_AWAY_FROM_TARGET, PICK_RANDOM_DEST, PICK_ZDEST,
        SET_TARGET_GRID_POS, SET_TARGET_POS, SET_TARGET_TILE, VALIDATE_POSITION,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{
    base, call_slot_2_returning, default_position, game_rand, goal_slots, map_tile, tile_offsets, unit_offsets, unit_slots, ZTUnit, APP_LOADING_FLAG, APP_RVA, BUILDING_TAG_RVA, TANK_LADDER_A, TANK_LADDER_B, UNIT_TYPE_TAG_RVA,
    WORLD_MGR_RVA,
};
use crate::{
    util::{get_from_memory, low_byte_bool, world_to_tile},
    vanilla_string::range_bytes,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2},
    vanilla_list::NodeList,
    ztmegatilemgr::entity_type_matches,
};

/// Vtable slots on the unit these methods dispatch through.
mod slots {
    /// `setTargetTile(tile, keepPosition)`.
    pub const SET_TARGET_TILE: u32 = 0x2d8;
    /// `setTargetPos(position)`.
    pub const SET_TARGET_POS: u32 = 0x2dc;
    /// `validateBuilding(out, flag)`: the building the unit is working at, or `0`.
    pub const VALIDATE_BUILDING: u32 = 0x2e8;
    /// `isTank()` on a habitat.
    pub const HABITAT_IS_TANK: u32 = 0x20;
    /// The predicate `doRandomWalk` starts with.
    pub const CAN_RANDOM_WALK: u32 = 0x184;
    /// The slot `doRandomWalk` calls with `0` when the walk could not start.
    pub const ABORT_WALK: u32 = 0x178;
}
/// `DAT_00639528`: how many path steps ahead a unit aims at, and `DAT_00635820`: the distance a unit keeps (`f64`).
const PATH_LOOK_AHEAD_RVA: u32 = 0x0023_9528;
const PATH_STANDOFF_RVA: u32 = 0x0023_5820;
/// `ZTAIMgr` slots (`getRandomX(unit)`, `getRandomY(unit)`) and fields `pickRandomDest` reads: the search half-widths and
/// their minimum extents.
const AIMGR_WIDTH_SLOT: u32 = 0x20;
const AIMGR_HEIGHT_SLOT: u32 = 0x24;
const AIMGR_MIN_X: u32 = 0xd4;
const AIMGR_MIN_Y: u32 = 0xd8;
/// Unit slots: `canWalkTo(tile, flag)` and `setStopAtEnd(position)`.
const CAN_WALK_TO_SLOT: u32 = 0x168;
const SET_DESTINATION_SLOT: u32 = 0x1f8;
/// `GLOBAL_ZTAIMgr` as an RVA.
const AIMGR_RVA: u32 = 0x0023_8098;
/// Goal id and argument `doRandomWalk` sets before it asks the AI manager for a destination.
const RANDOM_WALK_GOAL: u32 = 1;
const RANDOM_WALK_GOAL_ARGUMENT: u32 = 4;

/// `pickZDest`: the tank's water level (`ZTTankExhibit::getWaterLevel`) and the unit type's minimum depth.
const TANK_WATER_LEVEL: u32 = 0x188;
const TYPE_MINIMUM_DEPTH: u32 = 0x68;

/// `getPathAwayFromTarget`: the extra ring radius a unit type adds (`DAT_006394f4`).
const FLEE_RADIUS_RVA: u32 = 0x0023_94f4;


impl ZTUnit {
    /// `ZTUnit::validatePosition` (`0x004102b3`): revalidates the tank mode first when asked to, unless a save is loading.
    pub fn validate_position(self, flag: bool) {
        if flag && get_from_memory::<u8>(base() + APP_RVA + APP_LOADING_FLAG) == 0 {
            call_slot_0(self.0, goal_slots::VALIDATE_TANK_POSITION);
        }
        unsafe { BFUNIT_VALIDATE_POSITION.hooked()(self.0 as *const u32, flag) };
    }

    /// `ZTUnit::setTargetTile` (`0x00413342`): records the target tile and, unless told to keep the position, the world
    /// position of the tile's centre (or the "no position" default for no tile).
    pub fn set_target_tile(self, tile: u32, update_position: bool) {
        self.write(unit_offsets::TARGET_TILE, tile);
        if !update_position {
            return;
        }
        if tile == 0 {
            self.write_default_position(unit_offsets::TARGET_POSITION);
            return;
        }
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let offset = [0x20i32, 0x20, 0];
        let mut position = [0i32; 3];
        let result = unsafe { TILE_TO_WORLD.hooked()((world + 8) as *const u32, position.as_mut_ptr(), (tile + tile_offsets::X) as *const i32, offset.as_ptr()) };
        let result = result as u32;
        for index in 0..3u32 {
            let value: i32 = get_from_memory(result + index * 4);
            self.write(unit_offsets::TARGET_POSITION + index * 4, value);
        }
    }

    /// `ZTUnit::setTargetGridPos` (`0x004e7289`): the grid position as a world position (tile centre, `z` in sixteenths),
    /// through the virtual `setTargetPos`.
    pub fn set_target_grid_pos(self, grid: *const [i32; 3]) {
        let grid = unsafe { *grid };
        let position = [grid[0].wrapping_shl(6).wrapping_add(0x20), grid[1].wrapping_shl(6).wrapping_add(0x20), grid[2].wrapping_shl(4)];
        call_slot_1(self.0, slots::SET_TARGET_POS, position.as_ptr() as u32);
    }

    /// `ZTUnit::setTargetPos` (`0x004e72cc`): records the world position and hands the tile it lies in (none for the
    /// default position or one off the map) to the virtual `setTargetTile`, keeping the position.
    pub fn set_target_pos(self, position: *const [i32; 3]) {
        let position = unsafe { *position };
        for (index, value) in position.iter().enumerate() {
            self.write(unit_offsets::TARGET_POSITION + index as u32 * 4, *value);
        }
        let default = default_position();
        let tile = if position.iter().zip(default).all(|(a, b)| *a as u32 == b) {
            0
        } else {
            map_tile(world_to_tile(position[0]), world_to_tile(position[1]))
        };
        call_slot_2(self.0, slots::SET_TARGET_TILE, tile, 0);
    }

    /// `ZTUnit::getNonPathCost` (`0x0060c9a4`).
    pub fn get_non_path_cost(self) -> u32 {
        self.read(unit_offsets::NON_PATH_COST)
    }

    /// `ZTUnit::faceTowardTarget` (`0x00426f21`): turns towards the target tile when there is one.
    pub fn face_toward_target(self) {
        let tile: u32 = self.read(unit_offsets::TARGET_TILE);
        if tile != 0 {
            unsafe { BFUNIT_FACE_TOWARD_TILE.hooked()(self.0 as *const u32, tile as i32) };
        }
    }

    /// `ZTUnit::faceTowardTargetUnit` (`0x0042fa2c`): turns towards the target unit when there is one.
    pub fn face_toward_target_unit(self) {
        let target = self.get_target_unit();
        if target != 0 {
            unsafe { BFUNIT_FACE_TOWARD_ENTITY.hooked()(self.0 as *const u32, target as i32) };
        }
    }

    /// `ZTUnit::canIgnoreBlocking` (`0x004141f3`): whether the unit may walk onto `target` although it is occupied. A
    /// building on the unit's own tile that is `target` itself never blocks; otherwise a unit flagged to ignore blocking
    /// may, unless `target` holds a building other than the one the unit works at.
    pub fn can_ignore_blocking(self, target: u32) -> bool {
        let tile = self.tile();
        let occupant: u32 = get_from_memory(tile + tile_offsets::ENTITY_PTR);
        if occupant != 0 && unsafe { entity_type_matches(occupant, BUILDING_TAG_RVA) } && target != 0 && target == tile {
            return true;
        }
        let mut allowed = self.read::<u8>(unit_offsets::IGNORES_BLOCKING) != 0;
        if allowed && target != 0 {
            let mut scratch = [0u8; 4];
            let working_at = call_slot_2_returning(self.0, slots::VALIDATE_BUILDING, scratch.as_mut_ptr() as u32, 0);
            if working_at != 0 {
                let on_target: u32 = get_from_memory(target + tile_offsets::ENTITY_PTR);
                if on_target == 0 || !unsafe { entity_type_matches(on_target, BUILDING_TAG_RVA) } || on_target == working_at {
                    allowed = true;
                } else {
                    return false;
                }
            }
        }
        allowed
    }
}

/// The base level of a tank (`+0x40` -> first record -> its `+8` record's `+0x3c`), `0` when any link is missing.
fn tank_base_level(tank: u32) -> i32 {
    let walls: u32 = get_from_memory(tank + 0x40);
    if walls == 0 {
        return 0;
    }
    let record: u32 = get_from_memory(walls);
    if record == 0 {
        return 0;
    }
    let level_source: u32 = get_from_memory(record + 8);
    if level_source == 0 {
        0
    } else {
        get_from_memory(level_source + 0x3c)
    }
}

impl ZTUnit {
    /// The habitat `tile` belongs to for this unit (virtual `getHabitatFromTile`).
    fn habitat_for_tile(self, tile: u32) -> u32 {
        call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, tile)
    }

    fn habitat_is_tank(habitat: u32) -> bool {
        habitat != 0 && low_byte_bool(call_slot_0(habitat, slots::HABITAT_IS_TANK))
    }

    /// `ZTUnit::checkElevation` (`0x004935aa`): when `to` is a ramp tile and its habitat is a tank, whether `height` fits
    /// the ladder level; every other case is `BFUnit`'s.
    pub fn check_elevation(self, from: u32, from_height: i32, to: u32, height: i32) -> bool {
        let fallback = || unsafe { BFUNIT_CHECK_ELEVATION.hooked()(self.0 as *const u32, from as *const u32, from_height, to as *const u32, height) };
        if get_from_memory::<u8>(to + tile_offsets::HABITAT_FLAGS) & tile_offsets::TANK_BIT == 0 {
            return fallback();
        }
        let from_habitat = self.habitat_for_tile(from);
        let to_habitat = self.habitat_for_tile(to);
        if !Self::habitat_is_tank(to_habitat) {
            return fallback();
        }
        let base_level = tank_base_level(to_habitat);
        let level = get_from_memory::<i32>(to_habitat + TANK_LADDER_B).wrapping_add(base_level);
        if from_habitat != to_habitat && Self::habitat_is_tank(from_habitat) {
            return height == level && height == from_height && level >= base_level;
        }
        if self.type_surfaces() {
            height == level && height >= base_level
        } else {
            height <= level && height >= base_level
        }
    }

    /// `ZTUnit::cliffsOK` (`0x00414728`): whether the unit may step from `tile` towards `direction`. A ramp tile on one
    /// side compares the tank's ladder level with the flat tile's corner elevations; two tiles of different tanks compare
    /// their levels; everything else is `BFUnit`'s (or the plain edge-cliff bits).
    pub fn cliffs_ok(self, tile: u32, direction: u32) -> bool {
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let map = (world + 8) as *const u32;
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()(map, tile as *const u32, direction) };
        let fallback = || unsafe { BFUNIT_CLIFFS_OK.hooked()(self.0 as *const u32, tile as *const u32, direction) };
        let is_ramp = |tile: u32| tile != 0 && get_from_memory::<u8>(tile + tile_offsets::HABITAT_FLAGS) & tile_offsets::TANK_BIT != 0;
        let (tile_ramp, neighbour_ramp) = (is_ramp(tile), is_ramp(neighbour));
        if neighbour == 0 || (!tile_ramp && !neighbour_ramp) {
            let cliff: u8 = get_from_memory(tile + tile_offsets::CLIFF_BITS);
            let shifted = if direction & 0x1f >= 8 { 0 } else { cliff >> (direction & 0x1f) };
            return shifted & 3 <= 1;
        }
        if tile_ramp != neighbour_ramp {
            // `getDirection` is asked (ramp-side tile second when the ramp is on this tile), and the corner elevations
            // are read on the flat tile.
            let (ramp, flat, first, second) = if tile_ramp { (tile, neighbour, neighbour, tile) } else { (neighbour, tile, tile, neighbour) };
            let habitat = self.habitat_for_tile(ramp);
            if !Self::habitat_is_tank(habitat) {
                return fallback();
            }
            let level = get_from_memory::<i32>(habitat + TANK_LADDER_A).wrapping_add(tank_base_level(habitat));
            let towards = unsafe { BFMAP_GET_DIRECTION.hooked()(first as i32, second as i32) };
            let corner = |offset: i32| {
                let index = if towards == -1 { -1 } else { towards.wrapping_add(offset) & 7 };
                unsafe { GET_CORNER_ELEVATION.hooked()(flat as *const u32, index) }
            };
            return level == corner(1) && level == corner(-1);
        }
        let from_habitat = self.habitat_for_tile(tile);
        let to_habitat = self.habitat_for_tile(neighbour);
        if from_habitat == to_habitat {
            return true;
        }
        if !Self::habitat_is_tank(from_habitat) || !Self::habitat_is_tank(to_habitat) {
            return fallback();
        }
        let level = |habitat: u32| get_from_memory::<i32>(habitat + TANK_LADDER_A).wrapping_add(tank_base_level(habitat));
        level(from_habitat) == level(to_habitat)
    }

    /// `ZTUnit::pickZDest` (`0x0043dc11`): when the tile under `position` belongs to a tank, picks the height the unit swims
    /// to and adds it to `position`'s `z`. The depth the unit already holds (its grid `z` less the destination's, never
    /// negative) is clamped between the unit type's minimum and the water level, then the unit's depth mode
    /// (`+0x1ec`/`+0x1f0`, a byte range) chooses: `surface` (or a type that surfaces) the water level, `dive` a random
    /// depth between the minimum and the clamped one, `rise` a random height between the clamped one and the water
    /// level, `level` the clamped one, `floor` the minimum, anything else a random height from the minimum to the water
    /// level. A unit without an entity type is left alone (vanilla dereferences null).
    pub fn pick_z_dest(self, position: *mut [i32; 3]) {
        let (x, y, z): (i32, i32, i32) = (get_from_memory(position as u32), get_from_memory(position as u32 + 4), get_from_memory(position as u32 + 8));
        let habitat = call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, map_tile(x, y));
        if habitat == 0 || !low_byte_bool(call_slot_0(habitat, slots::HABITAT_IS_TANK)) {
            return;
        }
        let water: i32 = get_from_memory(habitat + TANK_WATER_LEVEL);
        let depth = Self::grid_pos(self.0)[2].wrapping_sub(z);
        let depth = if depth < 0 { 0 } else { depth };
        let unit_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if unit_type == 0 {
            return;
        }
        let minimum: i32 = get_from_memory(unit_type + TYPE_MINIMUM_DEPTH);
        let clamped = if depth > water {
            water
        } else if depth < minimum {
            minimum
        } else {
            depth
        };
        let add = |value: i32| crate::util::save_to_memory(position as u32 + 8, z.wrapping_add(value));
        if self.depth_mode_is(b"surface") || self.type_surfaces() {
            add(water);
        } else if self.depth_mode_is(b"dive") {
            add(if minimum < clamped { minimum + game_rand() % (clamped - minimum) } else { clamped });
        } else if self.depth_mode_is(b"rise") {
            add(if clamped < water { water - game_rand() % (water - clamped) } else { clamped });
        } else if self.depth_mode_is(b"level") {
            add(clamped);
        } else if self.depth_mode_is(b"floor") {
            add(minimum);
        } else if minimum.wrapping_add(1) < water {
            add(minimum + game_rand() % (water - minimum + 1));
        } else {
            add(clamped);
        }
    }

    /// Whether the depth mode, the byte range `[+0x1ec, +0x1f0)`, holds exactly `text`.
    fn depth_mode_is(self, text: &[u8]) -> bool {
        range_bytes(self.read(unit_offsets::DEPTH_MODE_BEGIN), self.read(unit_offsets::DEPTH_MODE_END)) == text
    }

    /// The grid position of an entity.
    fn grid_pos(entity: u32) -> [i32; 3] {
        let mut out = [0i32; 3];
        let result = unsafe { GET_GRID_POS.hooked()(entity as *const u32, out.as_mut_ptr()) };
        let result = result as u32;
        [get_from_memory(result), get_from_memory(result + 4), get_from_memory(result + 8)]
    }

    /// `ZTUnit::getPathToTarget` (`0x004a5b4d`): paths to the target, or to one of the steps ahead on the target's own path
    /// when that is nearer than the target and within the look-ahead.
    pub fn get_path_to_target(self, flag: bool) -> u32 {
        let target_id: u32 = self.read(unit_offsets::TARGET_UNIT_ID);
        if target_id == 0 {
            return 0;
        }
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let target = call_slot_2_returning(world, 0x24, target_id, 1);
        if target == 0 {
            return 0;
        }
        let own = Self::grid_pos(self.0);
        let at = Self::grid_pos(target);
        let target_unit = if unsafe { entity_type_matches(target, UNIT_TYPE_TAG_RVA) } { target } else { 0 };
        let steps: u32 = if target_unit == 0 { 0 } else { get_from_memory(target_unit + unit_offsets::PATH_LIST) };
        let step_count = if steps == 0 { 0 } else { NodeList::path_steps(steps).len() as i32 };
        let mut destination = at;
        if target_unit != 0 && step_count >= 1 {
            let mut reach: i32 = if self.unit_type() != 0 { get_from_memory(base() + PATH_LOOK_AHEAD_RVA) } else { 1 };
            if reach.wrapping_add(1) > step_count {
                reach = step_count - 1;
            }
            let squared = |a: [i32; 3], b: [i32; 3]| (0..3).fold(0i32, |sum, i| sum.wrapping_add(a[i].wrapping_sub(b[i]).wrapping_mul(a[i].wrapping_sub(b[i]))));
            let distance = squared(at, own);
            if reach.wrapping_mul(reach) > distance {
                let standoff: f64 = get_from_memory(base() + PATH_STANDOFF_RVA);
                let remaining = crate::util::ftol_low32_f64((f64::from(distance)).sqrt() - standoff);
                reach = remaining.max(0);
            }
            let pick = game_rand().checked_rem(reach + 1).unwrap_or(0);
            let node = NodeList::path_steps(steps).nth(pick.max(0) as usize);
            let step: [i32; 3] = [get_from_memory(node + 8), get_from_memory(node + 12), get_from_memory(node + 16)];
            if distance >= squared(step, own) {
                destination = step;
            }
        }
        unsafe { BFUNIT_GET_PATH.hooked()(self.0 as *const u32, destination.as_ptr(), u32::from(flag)) }
    }

    /// `ZTUnit::pickRandomDest` (`0x00428cc9`): a random walkable tile on the border of a box around the unit, sized by the
    /// AI manager. Writes the tile's position to `out` and hands it to the virtual destination setter; `false` when no tile
    /// qualified. With no walkable tile the last border tile examined is used, walkable or not (and none if that one was
    /// off the map), as vanilla does.
    pub fn pick_random_dest(self, out: *mut [i32; 3]) -> bool {
        let tile = self.tile();
        let aimgr: u32 = get_from_memory(base() + AIMGR_RVA);
        let mut chosen = 0u32;
        if tile != 0 && aimgr != 0 {
            let (cx, cy): (i32, i32) = (get_from_memory(tile + tile_offsets::X), get_from_memory(tile + tile_offsets::Y));
            let (width, height) = (call_slot_1(aimgr, AIMGR_WIDTH_SLOT, self.0) as i32, call_slot_1(aimgr, AIMGR_HEIGHT_SLOT, self.0) as i32);
            if width == 0 || height == 0 {
                return false;
            }
            let reach_x = game_rand() % width + 1 + get_from_memory::<i32>(aimgr + AIMGR_MIN_X);
            let reach_y = game_rand() % height + 1 + get_from_memory::<i32>(aimgr + AIMGR_MIN_Y);
            let mut candidates: Vec<u32> = Vec::new();
            for dx in -reach_x..=reach_x {
                for dy in -reach_y..=reach_y {
                    if !(dx == reach_x || dx == -reach_x || dy == reach_y || dy == -reach_y) {
                        continue;
                    }
                    // Vanilla keeps the last border tile it looked at in the same stack slot as the result (`0` for a tile
                    // off the map), so with no walkable candidate that tile is the answer.
                    let candidate = map_tile(cx.wrapping_add(dx), cy.wrapping_add(dy));
                    chosen = candidate;
                    if candidate != 0 && get_from_memory::<u8>(candidate + tile_offsets::TERRAIN_FLAGS) & tile_offsets::WALKABLE_BIT != 0 && low_byte_bool(call_slot_2_returning(self.0, CAN_WALK_TO_SLOT, candidate, 0)) {
                        candidates.push(candidate);
                    }
                }
            }
            if !candidates.is_empty() {
                chosen = candidates[(game_rand() as usize) % candidates.len()];
            }
        }
        if chosen == 0 {
            return false;
        }
        for index in 0..3u32 {
            let value: i32 = get_from_memory(chosen + tile_offsets::X + index * 4);
            crate::util::save_to_memory(out as u32 + index * 4, value);
        }
        call_slot_1(self.0, SET_DESTINATION_SLOT, out as u32);
        true
    }

    /// `ZTUnit::getPathAwayFromTarget` (`0x004a639e`): looks for a reachable tile on a ring around the unit, away from
    /// the unit it is fleeing, and starts a path to the first one `getPath` accepts. The ring starts three tiles out (a unit
    /// type adds `DAT_006394f4 + 2` instead) and closes in by one tile per round; its walkable border tiles are tried
    /// facing-away ones first (those whose direction differs from the direction away from the target by at most one step,
    /// or by seven), the rest after. With no path at any radius the unit's path steps are dropped and it random-walks.
    /// Returns whether a path was started (or the random walk did); without a target or an AI manager, `false`.
    pub fn get_path_away_from_target(self) -> bool {
        let target_id: u32 = self.read(unit_offsets::TARGET_UNIT_ID);
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let target = if target_id == 0 { 0 } else { call_slot_2_returning(world, 0x24, target_id, 1) };
        let aimgr: u32 = get_from_memory(base() + AIMGR_RVA);
        if target == 0 || aimgr == 0 {
            return false;
        }
        let own = Self::grid_pos(self.0);
        let at = Self::grid_pos(target);
        let direction_of = |offset: [i32; 3]| unsafe { BFMAP_GET_DIRECTION_OF_OFFSET.hooked()(offset.as_ptr() as *const glam::IVec3) };
        let heading = direction_of([own[0].wrapping_sub(at[0]), own[1].wrapping_sub(at[1]), own[2].wrapping_sub(at[2])]);
        let mut radius = if self.unit_type() != 0 { get_from_memory::<i32>(base() + FLEE_RADIUS_RVA).wrapping_add(2) } else { 3 };
        loop {
            let mut ring: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if !(dx == radius || dx == -radius || dy == radius || dy == -radius) {
                        continue;
                    }
                    let (x, y) = (own[0].wrapping_add(dx), own[1].wrapping_add(dy));
                    let tile = map_tile(x, y);
                    if tile == 0 || !low_byte_bool(call_slot_2_returning(self.0, CAN_WALK_TO_SLOT, tile, 0)) {
                        continue;
                    }
                    let away_by = heading.wrapping_sub(direction_of([x.wrapping_sub(own[0]), y.wrapping_sub(own[1]), 0])).wrapping_abs();
                    if away_by < 2 || away_by == 7 {
                        ring.push_front([x, y, own[2]]);
                    } else {
                        ring.push_back([x, y, own[2]]);
                    }
                }
            }
            if ring.iter().any(|candidate| low_byte_bool(unsafe { BFUNIT_GET_PATH.hooked()(self.0 as *const u32, candidate.as_ptr(), 0) })) {
                return true;
            }
            radius -= 1;
            if radius < 1 {
                break;
            }
        }
        self.drop_path_steps();
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::random_walk(self.0, false) {
            return answer;
        }
        low_byte_bool(unsafe { F_RANDOM_WALK.hooked()(aimgr as *const u32, self.0 as *const i32) })
    }

    /// Empties the unit's path-step list (`+0x154`), returning each node to vanilla's 20-byte freelist.
    fn drop_path_steps(self) {
        NodeList::path_steps(self.read(unit_offsets::PATH_LIST)).clear();
    }

    /// `ZTUnit::doRandomWalk` (`0x0049dafe`): starts a random walk through the AI manager and the unit's path-follower.
    pub fn do_random_walk(self) -> bool {
        let aimgr: u32 = get_from_memory(base() + AIMGR_RVA);
        if !low_byte_bool(call_slot_0(self.0, slots::CAN_RANDOM_WALK)) {
            return false;
        }
        call_slot_0(self.0, goal_slots::CLEAR_GOAL);
        call_slot_2(self.0, goal_slots::GOAL_SET, RANDOM_WALK_GOAL, RANDOM_WALK_GOAL_ARGUMENT);
        self.write(unit_offsets::COG_INVALID, 0u8);
        let step = |go_to_dest: bool| {
            #[cfg(feature = "reimplementation-tests")]
            if let Some(answer) = super::live_support::intercept::random_walk(self.0, go_to_dest) {
                return answer;
            }
            if go_to_dest {
                unsafe { BFUNIT_GO_TO_DEST.hooked()(self.0 as *const u32, false, 0) }
            } else {
                low_byte_bool(unsafe { F_RANDOM_WALK.hooked()(aimgr as *const u32, self.0 as *const i32) })
            }
        };
        if step(false) && step(true) {
            return true;
        }
        call_slot_1(self.0, slots::ABORT_WALK, 0);
        false
    }
}

#[detour_mod]
mod movement_detours {
    use super::*;

    #[detour(VALIDATE_POSITION)]
    unsafe extern "thiscall" fn validate_position(this: *const u32, flag: bool) {
        ZTUnit(this as u32).validate_position(flag);
    }

    #[detour(SET_TARGET_TILE)]
    unsafe extern "thiscall" fn set_target_tile(this: *const u32, tile: *const u32, update_position: bool) {
        ZTUnit(this as u32).set_target_tile(tile as u32, update_position);
    }

    #[detour(SET_TARGET_GRID_POS)]
    unsafe extern "thiscall" fn set_target_grid_pos(this: *const u32, grid: *const glam::IVec3) {
        ZTUnit(this as u32).set_target_grid_pos(grid as *const [i32; 3]);
    }

    #[detour(SET_TARGET_POS)]
    unsafe extern "thiscall" fn set_target_pos(this: *const u32, position: *const glam::IVec3) {
        ZTUnit(this as u32).set_target_pos(position as *const [i32; 3]);
    }

    #[detour(GET_NON_PATH_COST)]
    unsafe extern "thiscall" fn get_non_path_cost(this: *const u32) -> u32 {
        ZTUnit(this as u32).get_non_path_cost()
    }

    #[detour(FACE_TOWARD_TARGET)]
    unsafe extern "thiscall" fn face_toward_target(this: *const u32) {
        ZTUnit(this as u32).face_toward_target();
    }

    #[detour(FACE_TOWARD_TARGET_UNIT)]
    unsafe extern "thiscall" fn face_toward_target_unit(this: *const u32) {
        ZTUnit(this as u32).face_toward_target_unit();
    }

    #[detour(CHECK_ELEVATION)]
    unsafe extern "thiscall" fn check_elevation(this: *const u32, from: *const u32, from_height: i32, to: *const u32, height: i32) -> bool {
        ZTUnit(this as u32).check_elevation(from as u32, from_height, to as u32, height)
    }

    #[detour(CLIFFS_OK)]
    unsafe extern "thiscall" fn cliffs_ok(this: *const u32, tile: *const u32, direction: u32) -> bool {
        ZTUnit(this as u32).cliffs_ok(tile as u32, direction)
    }

    #[detour(DO_RANDOM_WALK)]
    unsafe extern "thiscall" fn do_random_walk(this: *const u32) -> i8 {
        i8::from(ZTUnit(this as u32).do_random_walk())
    }

    #[detour(GET_PATH_TO_TARGET)]
    unsafe extern "thiscall" fn get_path_to_target(this: *const u32, flag: bool) -> u32 {
        ZTUnit(this as u32).get_path_to_target(flag)
    }

    #[detour(PICK_RANDOM_DEST)]
    unsafe extern "thiscall" fn pick_random_dest(this: *const u32, out: *const glam::IVec3) -> u32 {
        u32::from(ZTUnit(this as u32).pick_random_dest(out as *mut [i32; 3]))
    }

    #[detour(PICK_ZDEST)]
    unsafe extern "thiscall" fn pick_z_dest(this: *const u32, position: *const glam::IVec3) {
        ZTUnit(this as u32).pick_z_dest(position as *mut [i32; 3]);
    }

    #[detour(GET_PATH_AWAY_FROM_TARGET)]
    unsafe extern "thiscall" fn get_path_away_from_target(this: *const u32) -> bool {
        ZTUnit(this as u32).get_path_away_from_target()
    }

    #[detour(CAN_IGNORE_BLOCKING)]
    unsafe extern "thiscall" fn can_ignore_blocking(this: *const u32, target: u32) -> i8 {
        i8::from(ZTUnit(this as u32).can_ignore_blocking(target))
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn validate_position(this: u32, flag: bool) {
            unsafe { super::VALIDATE_POSITION_DETOUR.call(this as *const u32, flag) }
        }

        pub(crate) fn set_target_tile(this: u32, tile: u32, update_position: bool) {
            unsafe { super::SET_TARGET_TILE_DETOUR.call(this as *const u32, tile as *const u32, update_position) }
        }

        pub(crate) fn set_target_grid_pos(this: u32, grid: *const [i32; 3]) {
            unsafe { super::SET_TARGET_GRID_POS_DETOUR.call(this as *const u32, grid as *const glam::IVec3) }
        }

        pub(crate) fn set_target_pos(this: u32, position: *const [i32; 3]) {
            unsafe { super::SET_TARGET_POS_DETOUR.call(this as *const u32, position as *const glam::IVec3) }
        }

        pub(crate) fn get_non_path_cost(this: u32) -> u32 {
            unsafe { super::GET_NON_PATH_COST_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn face_toward_target(this: u32) {
            unsafe { super::FACE_TOWARD_TARGET_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn face_toward_target_unit(this: u32) {
            unsafe { super::FACE_TOWARD_TARGET_UNIT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn check_elevation(this: u32, from: u32, from_height: i32, to: u32, height: i32) -> bool {
            unsafe { super::CHECK_ELEVATION_DETOUR.call(this as *const u32, from as *const u32, from_height, to as *const u32, height) }
        }

        pub(crate) fn cliffs_ok(this: u32, tile: u32, direction: u32) -> bool {
            unsafe { super::CLIFFS_OK_DETOUR.call(this as *const u32, tile as *const u32, direction) }
        }

        pub(crate) fn do_random_walk(this: u32) -> bool {
            super::low_byte_bool(unsafe { super::DO_RANDOM_WALK_DETOUR.call(this as *const u32) } as u8 as u32)
        }

        pub(crate) fn get_path_to_target(this: u32, flag: bool) -> u32 {
            unsafe { super::GET_PATH_TO_TARGET_DETOUR.call(this as *const u32, flag) }
        }

        pub(crate) fn get_path_away_from_target(this: u32) -> bool {
            unsafe { super::GET_PATH_AWAY_FROM_TARGET_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn pick_z_dest(this: u32, position: *mut [i32; 3]) {
            unsafe { super::PICK_ZDEST_DETOUR.call(this as *const u32, position as *const glam::IVec3) }
        }

        pub(crate) fn pick_random_dest(this: u32, out: *mut [i32; 3]) -> bool {
            super::low_byte_bool(unsafe { super::PICK_RANDOM_DEST_DETOUR.call(this as *const u32, out as *const glam::IVec3) })
        }

        pub(crate) fn can_ignore_blocking(this: u32, target: u32) -> bool {
            super::low_byte_bool(unsafe { super::CAN_IGNORE_BLOCKING_DETOUR.call(this as *const u32, target) } as u8 as u32)
        }
    }
}

/// Installs the movement detours.
pub(super) fn init() {
    if let Err(e) = unsafe { movement_detours::init_detours() } {
        error!("Failed to initialise ztunit movement detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    movement_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use movement_detours::test_real;
