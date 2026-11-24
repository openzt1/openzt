//! `fRandomWalk` and `fPlaySetTerrainF`.

use super::*;

/// The vanilla pieces `fRandomWalk` leans on, so a test can substitute recorders.
pub struct RandomWalkOps {
    /// The game's `rand()` (`0..=0x7fff`).
    pub rand: unsafe fn() -> u32,
    /// The world manager's pointer (the map is at `+8`).
    pub world: unsafe fn() -> u32,
    /// `BFMap::getNeighbors(map, tile)` as a plain list (the vanilla buffer is freed by the implementation).
    pub neighbors: unsafe fn(u32, u32) -> Vec<u32>,
    /// `BFUnit::getPathCost(unit, from_tile, to_tile)`.
    pub path_cost: unsafe fn(u32, u32, u32) -> i32,
    pub path_cost_limit: unsafe fn() -> i32,
    pub no_destination: unsafe fn() -> [i32; 3],
}

pub(super) unsafe fn vcall_walk(unit: u32, tile: u32) -> bool {
    (unsafe { call_virtual_args(unit, ENTITY_VT_CAN_WALK_ON, [tile, 1]) }) as u8 != 0
}

pub(super) unsafe fn vcall_path(this: u32, start: &[i32; 3], goal: &[i32; 3], unit: u32, flag: u32) -> bool {
    (unsafe { call_virtual_args(this, AI_VT_GET_PATH, [start.as_ptr() as u32, goal.as_ptr() as u32, unit, flag]) }) as u8 != 0
}

/// The map's tile at tile coordinates `(x, y)`, or `0` outside the map.
pub(super) fn map_tile(map: u32, x: i32, y: i32) -> u32 {
    let width = get_from_memory::<i32>(map + MAP_WIDTH);
    if x < 0 || y < 0 || x >= width || y >= get_from_memory::<i32>(map + MAP_HEIGHT) {
        return 0;
    }
    get_from_memory::<u32>(map + MAP_TILES).wrapping_add((y.wrapping_mul(width).wrapping_add(x) as u32).wrapping_mul(MAP_TILE_SIZE))
}

pub(super) fn tile_position(tile: u32) -> [i32; 3] {
    [get_from_memory::<i32>(tile + TILE_POSITION_OFFSET), get_from_memory::<i32>(tile + TILE_POSITION_OFFSET + 4), get_from_memory::<i32>(tile + TILE_POSITION_OFFSET + 8)]
}

/// The tile offsets on the border of the box `[-half_x, half_x] x [-half_y, half_y]`, in the order `fRandomWalk`
/// tests them: `dx` ascending, then `dy` ascending (the centre and the interior are skipped).
pub(super) fn border_offsets(half_x: i32, half_y: i32) -> impl Iterator<Item = (i32, i32)> {
    (-half_x..=half_x).flat_map(move |dx| (-half_y..=half_y).map(move |dy| (dx, dy))).filter(move |&(dx, dy)| dx == half_x || dx == -half_x || dy == half_y || dy == -half_y)
}

/// The neighbour indices `fRandomWalk` tries: from the random start index up to the end of the list. The scan
/// does not wrap, so the entries before the start index are never tried.
pub(super) fn neighbor_scan(first: usize, len: usize) -> std::ops::Range<usize> {
    first..len
}

/// `BFAIMgr::fRandomWalk(unit)`: for an idle unit (no queued targets), tries in turn to resume a standing
/// destination (when it is not the unit's own tile), to walk to a random walkable tile on the border of a
/// random-sized box around it, to walk to a random walkable neighbour whose path cost is under the limit, and
/// finally to "walk" to its own position. The unit's `+0x20f` guard is set for the duration. The result is
/// whether a path was started.
///
/// `None` (defer to vanilla) where vanilla divides by zero (a zero random extent or an empty neighbour list)
/// or hands a null tile to `getNeighbors` (a unit off the map).
pub unsafe fn f_random_walk(this: u32, unit: u32, ops: &RandomWalkOps) -> Option<bool> {
    if unit == 0 || !TargetList::of_unit(unit).is_empty() {
        return Some(false);
    }
    let map = unsafe { (ops.world)() }.wrapping_add(OFFSET_WORLD_MAP);
    if map == 0 {
        return Some(false);
    }
    save_to_memory(unit + UNIT_WALKING_GUARD, 1u8);

    let div64 = |v: i32| v.wrapping_add((v >> 31) & 0x3f) >> 6;
    let mut height = get_from_memory::<i32>(unit + UNIT_POS_Z);
    if height < 0 {
        height = height.wrapping_sub(15);
    }
    let start = [div64(get_from_memory::<i32>(unit + OFFSET_UNIT_POS_X)), div64(get_from_memory::<i32>(unit + OFFSET_UNIT_POS_Y)), height.wrapping_add((height >> 31) & 0xf) >> 4];
    let start_tile = map_tile(map, start[0], start[1]);

    let mut goal = unsafe { (ops.no_destination)() };
    let mut has_destination = unsafe { call_virtual_args(unit, ENTITY_VT_GET_DESTINATION, [goal.as_mut_ptr() as u32]) } as u8 != 0;
    let queued = !TargetList::of_unit(unit).is_empty();
    if has_destination && goal == start {
        has_destination = false;
    }
    let set_destination = |goal: &[i32; 3]| {
        unsafe { call_virtual_args(unit, ENTITY_VT_SET_DESTINATION, [goal.as_ptr() as u32]) };
    };

    let mut result = queued;
    if !queued && has_destination {
        result = unsafe { vcall_path(this, &start, &goal, unit, 1) };
    }

    if !result && start_tile != 0 {
        let range_x = unsafe { vcall1(this, AI_VT_RANDOM_X, unit) } as i32;
        let range_y = unsafe { vcall1(this, AI_VT_RANDOM_Y, unit) } as i32;
        if range_x == 0 || range_y == 0 {
            return None;
        }
        let half_x = (unsafe { (ops.rand)() } as i32).wrapping_rem(range_x).wrapping_add(1).wrapping_add(get_from_memory::<i32>(this + AI_RANDOM_EXTRA_X));
        let half_y = (unsafe { (ops.rand)() } as i32).wrapping_rem(range_y).wrapping_add(1).wrapping_add(get_from_memory::<i32>(this + AI_RANDOM_EXTRA_Y));
        let mut candidates: Vec<u32> = Vec::new();
        for (dx, dy) in border_offsets(half_x, half_y) {
            let tile = map_tile(map, start[0].wrapping_add(dx), start[1].wrapping_add(dy));
            if tile != 0 && unsafe { vcall_walk(unit, tile) } {
                candidates.push(tile);
            }
        }
        if !candidates.is_empty() {
            let pick = (unsafe { (ops.rand)() } as i32).wrapping_rem(candidates.len() as i32) as usize;
            let tile = candidates[pick];
            if tile != 0 {
                goal = tile_position(tile);
                set_destination(&goal);
                result = unsafe { vcall_path(this, &start, &goal, unit, 0) };
            }
        }
    }

    if !result {
        if start_tile == 0 {
            return None;
        }
        let neighbors = unsafe { (ops.neighbors)(map, start_tile) };
        if neighbors.is_empty() {
            return None;
        }
        let first = (unsafe { (ops.rand)() } as i32).wrapping_rem(neighbors.len() as i32) as usize;
        for index in neighbor_scan(first, neighbors.len()) {
            let tile = neighbors[index];
            if unsafe { vcall_walk(unit, tile) } && unsafe { (ops.path_cost)(unit, start_tile, tile) } < unsafe { (ops.path_cost_limit)() } {
                goal = tile_position(tile);
                set_destination(&goal);
                result = unsafe { vcall_path(this, &start, &goal, unit, 0) };
            }
            if result {
                break;
            }
        }
    }
    if !result {
        result = unsafe { vcall_path(this, &start, &start, unit, 0) };
    }
    save_to_memory(unit + UNIT_WALKING_GUARD, 0u8);
    Some(result)
}

pub(super) unsafe fn random_vanilla() -> u32 {
    unsafe { RAND.original()() }
}

pub(super) unsafe fn world_vanilla() -> u32 {
    read_global_pointer(RVA_GLOBAL_WORLD_MGR)
}

pub(super) unsafe fn neighbors_vanilla(map: u32, tile: u32) -> Vec<u32> {
    let mut header = [0u32; 3];
    unsafe { BFMAP_GET_NEIGHBORS.original()(map as *const u32, header.as_mut_ptr() as *const u32, tile as i32) };
    let (begin, end, cap) = (header[0], header[1], header[2]);
    let list = (begin..end).step_by(4).map(get_from_memory::<u32>).collect();
    if begin != 0 {
        unsafe { POOLALLOC_DEALLOCATE_N_4.original()(begin as *const u32, ((cap - begin) >> 2) as i32) };
    }
    list
}

pub(super) unsafe fn path_cost_vanilla(unit: u32, from: u32, to: u32) -> i32 {
    (unsafe { BFUNIT_GET_PATH_COST.original()(unit as *const u32, from as i32, to as i32) }) as i32
}

pub(super) unsafe fn path_cost_limit_vanilla() -> i32 {
    get_from_memory::<i32>(get_module_base("zoo.exe") as u32 + RVA_PATH_COST_LIMIT)
}

pub(super) unsafe fn no_destination_vanilla() -> [i32; 3] {
    let base = get_module_base("zoo.exe") as u32 + RVA_NO_DESTINATION;
    [get_from_memory::<i32>(base), get_from_memory::<i32>(base + 4), get_from_memory::<i32>(base + 8)]
}

pub(super) const RANDOM_WALK_VANILLA: RandomWalkOps = RandomWalkOps {
    rand: random_vanilla,
    world: world_vanilla,
    neighbors: neighbors_vanilla,
    path_cost: path_cost_vanilla,
    path_cost_limit: path_cost_limit_vanilla,
    no_destination: no_destination_vanilla,
};

pub(super) unsafe fn animal_target_building_vanilla(unit: u32) -> bool {
    unsafe { ZTANIMAL_TARGET_BUILDING.hooked()(unit as *const u32, true) }
}

pub(super) unsafe fn f_move_vanilla(this: u32, none: u32, unit: u32, done: u32, name: u32, speed: u32, empty: u32) -> bool {
    unsafe {
        F_MOVE.hooked()(
            this as *const u32,
            none as i32,
            0,
            unit as *const i32,
            done as *const u8,
            name as i32,
            speed,
            empty as *const i8,
            TARGET_BUILDING_MOVE_FLAG,
            TARGET_BUILDING_MOVE_LIMIT,
        )
    }
}

pub(super) const TARGET_BUILDING_VANILLA: TargetBuildingOps = TargetBuildingOps { target_building: animal_target_building_vanilla, f_move: f_move_vanilla };

/// `BFTile` byte whose low two bits mark a water tile.
pub(super) const TILE_WATER_FLAGS_OFFSET: u32 = 0x83;

/// Terrain flag byte of the unit's tile, through the vanilla `BFEntity::getTile`.
pub(super) unsafe fn unit_tile_flags_vanilla(unit: u32) -> u8 {
    let tile = unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) } as u32;
    get_from_memory::<u8>(tile + TILE_WATER_FLAGS_OFFSET)
}

/// `ZTAIMgr::fPlaySetTerrainF(call, unit)` for the four-parameter form: clears the call's result flag, picks a
/// set name by the unit's `+0x258` selector (`4` the fourth parameter, `3` the third, otherwise the first on a
/// land tile and the second on a water tile) and plays it with `play_set(name, unit)`, returning its low byte.
/// The name is a fresh vanilla string holding a copy of the parameter (vanilla starts from an empty string and
/// assigns the parameter into it, so the contents match).
///
/// `None` (defer to vanilla) for any other parameter count, where vanilla forwards to `BFAIMgr`'s handler, and
/// for a unit that is null or not a `ZTUnitType`, where vanilla reads the selector through null.
pub unsafe fn f_play_set_terrain_f(call: u32, unit: u32, tile_flags: unsafe fn(u32) -> u8, play_set: unsafe fn(u32, u32) -> u32) -> Option<bool> {
    if unsafe { call_param_count(call) } != 4 || !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        return None;
    }
    save_to_memory(call + CALL_DONE_FLAG_OFFSET, 0u8);
    let index = match get_from_memory::<u32>(unit + UNIT_TANK_SELECTOR_OFFSET) {
        4 => 3,
        3 => 2,
        _ if unsafe { tile_flags(unit) } & 3 == 0 => 0,
        _ => 1,
    };
    let name = unsafe { call_params_begin(call) } + index * CALL_PARAM_SIZE;
    Some(low_byte_bool(unsafe { play_set(name, unit) }))
}

/// `BFAIMgr` vtable slot `convertConstantParam(param, unit)`.
pub(super) const AI_VTABLE_CONVERT_CONSTANT_PARAM: u32 = 0x34;
/// Entity vtable slot `setStopAtEnd(bool)`.
pub(super) const ENTITY_VTABLE_SET_STOP_AT_END: u32 = 0xdc;
/// Offset of the call's second flag byte, which `fPlayTimeF` forwards to `fPlayTime`.
pub(super) const CALL_FLAG_B_OFFSET: u32 = 0x19;
/// Milliseconds per second: `fPlayTimeF` converts its duration parameter to milliseconds.
pub(super) const MILLIS_PER_SECOND: u32 = 1000;
