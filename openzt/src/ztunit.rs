//! `ZTUnit`: the movement/AI base under `ZTAnimal`, `ZTGuest` and `ZTStaff` (`ZTUnit` -> `BFUnit` -> `BFEntity`).
//!
//! Vanilla-layout-compatible (style 1): every ported subclass and the un-ported `BFUnit`/`ZTStaff`/`BFEntity` code read
//! a unit's fields by raw offset, so the object stays vanilla memory and this module reads and writes it in place. A
//! `ZTUnit` is `0x260` bytes: the `BFUnit` part (`0x000`-`0x213`) followed by the fields in [`unit_offsets`]; `ZTStaff`
//! and `ZTAnimal` append their own fields after it. Nothing here owns an allocation.
//!
//! Every method on [`ZTUnit`] is the base behaviour: the subclasses call it, directly or through the vtable, after their
//! own part. A method the vanilla body would only fault on (a unit whose entity type is missing or is not a unit type)
//! answers `0`/`false` instead.
//!
//! This file holds the leaf getters, the animation getters, the lifecycle (constructor, save/load, map add/remove) and
//! the goal and tank-position methods. The submodules hold the rest: `movement` (targets, facing, paths, blocking),
//! `satisfiers` (targets, predators, satisfier searches), `buildings` and `building_list` (the building the unit uses
//! and the vanilla list of buildings), `shows` (shows and tricks) and `messages` (the event messages).

mod building_list;
mod buildings;
mod movement;
mod satisfiers;
mod messages;
mod shows;

use std::ffi::c_void;

use openzt_detour::{
    generated::{
    bfcoggoal::SET as BFCOGGOAL_SET,
    bfentity::{GET_GRID_POS as BFENTITY_GET_GRID_POS, GET_TILE as BFENTITY_GET_TILE, INIT_STATUS_VARS as BFENTITY_INIT_STATUS_VARS, SET_WORLD_POS as BFENTITY_SET_WORLD_POS},
    bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION, GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR},
    bftile::IS_IN_ZOO as BFTILE_IS_IN_ZOO,
    bfapp::LOAD_STRING,
    bfentity::{CREATE_NAME as BFENTITY_CREATE_NAME, SET_BS},
    bfentitytype::GET_UNIT_COUNT,
    bfinternat::GET_NUMBER_TEXT,
    msvc_std_vectorgxmixerlink::DESTRUCTOR_0X411C0F as GXMIXERLINK_VECTOR_DESTRUCTOR,
    gxmixer::{CLEAR_ALL as GXMIXER_CLEAR_ALL, UPDATE as GXMIXER_UPDATE},
    bfunit::{
        ADD_TO_MAP as BFUNIT_ADD_TO_MAP, CLEAR_GOAL as BFUNIT_CLEAR_GOAL, CONSTRUCTOR as BFUNIT_CONSTRUCTOR, DO_AMBIENT_ANIMS as BFUNIT_DO_AMBIENT_ANIMS, GET_IDLE_ANIM as BFUNIT_GET_IDLE_ANIM,
        INIT_AMBIENT_ANIMS as BFUNIT_INIT_AMBIENT_ANIMS, INTO_NEW_TILE as BFUNIT_INTO_NEW_TILE, LOAD as BFUNIT_LOAD, REMOVE_FROM_MAP as BFUNIT_REMOVE_FROM_MAP,
        SAVE as BFUNIT_SAVE, UPDATE_GOALS as BFUNIT_UPDATE_GOALS,
    },
    poolalloc::DEALLOCATE as POOLALLOC_DEALLOCATE,
    ztbuilding::{CHECK_USER as ZTBUILDING_CHECK_USER, REMOVE_USER as ZTBUILDING_REMOVE_USER},
    ztfence::{CLOSE_GATE as ZTFENCE_CLOSE_GATE, OPEN_GATE as ZTFENCE_OPEN_GATE},
    ztmapview::{CHECK_UNDO_BUFFER, PREPARE_TO_DELETE_ENTITY},
    ztthoughtmgr::REMOVE_THOUGHTS_BY_OBJECT,
    ztui_general::GET_MAPVIEW,
    ztworldmgr::GET_NEED_INDEX,
    ztunit::{
        DO_AMBIENT_ANIMS, GET_DEPTH, GET_FAST_ANIM_SPEED, GET_FAST_ANIM_SYMBOL, GET_HABITAT, GET_HABITAT_FROM_GATE, GET_HABITAT_FROM_TILE, GET_ICON_ANIM,
        GET_IDLE_ANIM, GET_IDLE_ANIM_SYMBOL, GET_MEDIUM_ANIM_SPEED, GET_MEDIUM_ANIM_SYMBOL, GET_ONLY_UNDERWATER, GET_SLOW_ANIM_SPEED, GET_SLOW_ANIM_SYMBOL,
        GET_SWIMS, GET_TANK, GET_TANK_MODE, GET_UNDERWATER, INIT_AMBIENT_ANIMS, IS_IN_TANK, IS_ON_PLATFORM, IS_ON_WATER, IS_WATER_TRAVELING, NO_SHADOW_PART,
        NO_UNDER_PART, SET_TANK_MODE,
        ADD_TO_MAP, CREATE_NAME, INIT_AFTER_LOAD, ABORT_GOAL, CLEAR_GOAL, CLEAR_STATE_VARIABLES, CONSTRUCTOR, DO_BUILDING, GET_RANDOM_OK, GOAL_SET, INIT_STATUS_VARS, INTO_NEW_TILE, IS_DOING_SOMETHING, LOAD,
        REMOVE_FROM_MAP, RESET_AI, SAVE, SET_IS_REMOVED, UPDATE_GOALS,
        VALIDATE_TANK_POSITION,
    },
    zthabitat::{ADD_SHOW_UNIT as ZTHABITAT_ADD_SHOW_UNIT, GET_GATE_TILE_IN, GET_GATE_TILE_OUT, REMOVE_SHOW_UNIT as ZTHABITAT_REMOVE_SHOW_UNIT},
    zttankexhibit::{GET_LADDER_BASE_INSIDE_POS, GET_LADDER_BASE_OUTSIDE_POS, GET_LADDER_TOP_INSIDE_POS, GET_LADDER_TOP_OUTSIDE_POS},
    ztunittype::{
        GET_DIVE_ANIM_SYMBOL, GET_RISE_ANIM_SYMBOL, GET_SUB_IDLE_ANIM_SYMBOL, GET_SUB_SWIM_ANIM_SYMBOL, GET_SURF_IDLE_ANIM_SYMBOL, GET_SURF_SWIM_ANIM_SYMBOL,
    },
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use building_list::BuildingList;

use crate::{
    globals::{get_module_base, globals},
    msvc_deque::MsvcDeque,
    vanilla_list::NodeList,
    util::{get_from_memory, low_byte_bool, position_in_tile, ref_from_memory, save_to_memory, world_to_tile},
    bfentity::new_owned_name,
    vanilla_string::{c_string_bytes, VanillaString},
    zthabitatmgr::ZTHabitatMgr,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, call_slot_3, read_field, write_field},
    ztworld::mgr::ztworldmgr::ZTWorldMgr,
    ztmegatilemgr::entity_type_matches,
    ztshow::mgr::ztshowmgr::ZTShowMgr,
};

/// A `ZTUnit` is this many bytes; subclasses append their own fields after it.
pub const UNIT_SIZE: u32 = 0x260;

/// Vtable address of `ZTUnit` (`0x0062fa44`) as an RVA from the `zoo.exe` base.
pub const UNIT_VTABLE_RVA: u32 = 0x0022_fa44;

/// Class tag (`DAT_006386b0`) as an RVA: the entity type's `isCastClass` accepts it for a unit type.
pub const UNIT_TYPE_TAG_RVA: u32 = 0x0023_86b0;

/// `GLOBAL_ZTWorldMgr` and `GLOBAL_ZTHabitatMgr` as RVAs from the `zoo.exe` base.
const WORLD_MGR_RVA: u32 = 0x0023_8040;
const HABITAT_MGR_RVA: u32 = 0x0023_805c;

/// The `BFUnit` fields from the end of `BFEntity` (`0x154`) to the end of `BFUnit` (`0x214`). Fields the unit methods
/// never touch are gaps; the offsets of the named fields are checked against vanilla's below.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BFUnitTail {
    /// Sentinel of the unit's list of path steps (20-byte nodes `{next, previous, x, y, z}`).
    pub path_steps: u32,
    _gap_158: [u8; 4],
    /// Start of the embedded `BFCogGoal`.
    pub cog_goal: [u8; 0x14],
    /// The goal id: `0x6e` on habitat duties, `0x6f` retrieving an escaped animal, `0x66`-`0x6a` in shows and buildings.
    pub goal_id: u32,
    /// The embedded `BFCogGoal`'s state word.
    pub cog_goal_state: u32,
    /// The id of the unit being chased or fled from.
    pub target_unit_id: u32,
    _gap_17c: [u8; 4],
    /// Non-zero once a guest has a state to voice a thought about.
    pub thought_state: u32,
    _gap_184: [u8; 0x24],
    /// A byte of the embedded `BFCogGoal`; the routines only run their goal while it is clear.
    pub cog_goal_flag: u8,
    _gap_1a9: [u8; 0x13],
    /// `updateGoals` tests it against `3` ("in building").
    pub building_state: u32,
    _gap_1c0: [u8; 4],
    /// Object notified (virtual `+0x18`) when the tank mode changed; null when none.
    pub mode_listener: u32,
    _gap_1c8: [u8; 0x24],
    /// The depth mode string, the byte range `[begin, end)`.
    pub depth_mode_begin: u32,
    pub depth_mode_end: u32,
    _gap_1f4: [u8; 4],
    /// Movement speed written by `setSpeed`.
    pub speed: u32,
    _gap_1fc: [u8; 0xc],
    /// `-1` while diving, `1` while rising.
    pub vertical_direction: i32,
    /// Set while the unit plays movement animations.
    pub move_anim_enabled: u8,
    /// Cleared by `clearStateVariables`; an animal's `releasePredator` clears it too.
    pub byte_20d: u8,
    /// `BFCogGoal` validity flag cleared by `updateGoals`' no-habitat tail.
    pub cog_invalid: u8,
    _gap_20f: u8,
    /// Answer of `canIgnoreBlocking` when no blocking building is involved.
    pub ignores_blocking: u8,
    _gap_211: [u8; 3],
}

/// The `ZTUnit` fields from the end of `BFUnit` (`0x214`) to the end of `ZTUnit` (`0x260`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct ZTUnitTail {
    /// Offsets (x, y, z) inside the building tile the unit works at.
    pub building_offset: [i32; 3],
    /// A second default-initialised position.
    pub position_b: [i32; 3],
    /// The id of the predator `updateGoals` records when it switches to the predator goal.
    pub predator: u32,
    /// Answer of `getNonPathCost`; the constructor sets it to `1`.
    pub non_path_cost: u32,
    /// The tile the unit is heading for.
    pub target_tile: u32,
    /// World position of the target; saved, and `load` rebuilds `target_tile` from it.
    pub target_position: [i32; 3],
    /// Tile of the building the unit uses.
    pub building_tile: u32,
    /// The fence gate the unit is queued at.
    pub gate: u32,
    /// Set when something wants a predator check on the next goal update.
    pub predator_check_pending: u8,
    /// The slot of its building the unit occupies (`0xff` for none).
    pub building_slot: u8,
    /// The index of the need the unit is satisfying.
    pub need_index: u8,
    pub show_flag_a: u8,
    pub show_flag_b: u8,
    /// Set while the unit performs in a show.
    pub in_show: u8,
    /// Trick type the unit performs (`0` when none).
    pub trick_type: u16,
    /// The id of the `ZTShowInfo` the unit performs in.
    pub show_info_id: u16,
    _gap_256: [u8; 2],
    /// `2` on land beside a gate, `3` on the surface, `4` underwater.
    pub tank_mode: u32,
    /// Non-zero while the unit is occupied.
    pub busy: u8,
    _gap_25d: [u8; 3],
}

/// Where each tail starts inside a unit.
const BF_UNIT_TAIL_START: u32 = 0x154;
const ZT_UNIT_TAIL_START: u32 = 0x214;

const _: () = assert!(std::mem::size_of::<BFUnitTail>() as u32 == ZT_UNIT_TAIL_START - BF_UNIT_TAIL_START);
const _: () = assert!(std::mem::size_of::<ZTUnitTail>() as u32 == UNIT_SIZE - ZT_UNIT_TAIL_START);

impl BFUnitTail {
    pub(crate) const fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl ZTUnitTail {
    pub(crate) const fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// Offsets of the fields a unit's methods read or write, from the start of the unit. Those inside the two tail
/// structs are derived from them; the `BFEntity` ones below `0x154` are written out.
pub mod unit_offsets {
    use super::{BFUnitTail, ZTUnitTail, BF_UNIT_TAIL_START as BF, ZT_UNIT_TAIL_START as ZT};
    use std::mem::offset_of;

    /// `BFEntity` id, the key `ZTShowInfo::removeUnit` and `getEvents` use.
    pub const ENTITY_ID: u32 = 0x124;
    /// The entity's `ZTKeeperType` (or other entity type) pointer.
    pub const ENTITY_TYPE: u32 = 0x128;
    /// `BFEntity` remove flags `setIsRemoved` writes: the flag, and a byte it clears.
    pub const REMOVED_FLAG: u32 = 0x13c;
    pub const REMOVED_FLAG_B: u32 = 0x13d;
    /// `BFEntity` wander-restart countdown `resetAI` re-rolls for the `0x1530` unit type.
    pub const RESET_COUNTDOWN: u32 = 0x14c;
    /// Height above the terrain (`BFEntity`, `i32`).
    pub const HEIGHT: u32 = 0x120;
    /// `BFEntity` position (x, y, z).
    pub const POS_X: u32 = 0x114;
    pub const POS_Y: u32 = 0x118;
    pub const POS_Z: u32 = 0x11c;
    /// A byte `removeFromMap` tests before cancelling the unit's habitat visit.
    pub const MBR_0X142: u32 = 0x142;
    /// `initStatusVars` stores the unit's own address here.
    pub const SELF_POINTER: u32 = 0x64;
    /// `initStatusVars` stores `entity_type + 0x80` here when the type's `+0x84` is non-zero, otherwise null.
    pub const TYPE_BLOCK_POINTER: u32 = 0x68;
    /// Byte set by `intoNewTile` when the unit crosses between water and land.
    pub const MOVE_ANIM_DIRTY: u32 = 0x6c;
    /// The byte every `fRoutine` copies into its `done` flag: set while the unit is not moving.
    pub const STOPPED: u32 = 0xf9;
    pub use crate::ztstaff::staff_offsets::{ASSIGNED_HABITATS, ESCAPED_ANIMAL};

    /// `BFUnit` part.
    pub const PATH_LIST: u32 = BF + offset_of!(BFUnitTail, path_steps) as u32;
    pub const COG_GOAL: u32 = BF + offset_of!(BFUnitTail, cog_goal) as u32;
    pub const GOAL_ID: u32 = BF + offset_of!(BFUnitTail, goal_id) as u32;
    pub const COG_GOAL_STATE: u32 = BF + offset_of!(BFUnitTail, cog_goal_state) as u32;
    pub const TARGET_UNIT_ID: u32 = BF + offset_of!(BFUnitTail, target_unit_id) as u32;
    pub const THOUGHT_STATE: u32 = BF + offset_of!(BFUnitTail, thought_state) as u32;
    pub const COG_GOAL_FLAG: u32 = BF + offset_of!(BFUnitTail, cog_goal_flag) as u32;
    pub const BUILDING_STATE: u32 = BF + offset_of!(BFUnitTail, building_state) as u32;
    pub const MODE_LISTENER: u32 = BF + offset_of!(BFUnitTail, mode_listener) as u32;
    pub const DEPTH_MODE_BEGIN: u32 = BF + offset_of!(BFUnitTail, depth_mode_begin) as u32;
    pub const DEPTH_MODE_END: u32 = BF + offset_of!(BFUnitTail, depth_mode_end) as u32;
    pub const SPEED: u32 = BF + offset_of!(BFUnitTail, speed) as u32;
    pub const VERTICAL_DIRECTION: u32 = BF + offset_of!(BFUnitTail, vertical_direction) as u32;
    pub const MOVE_ANIM_ENABLED: u32 = BF + offset_of!(BFUnitTail, move_anim_enabled) as u32;
    pub const BYTE_20D: u32 = BF + offset_of!(BFUnitTail, byte_20d) as u32;
    pub const COG_INVALID: u32 = BF + offset_of!(BFUnitTail, cog_invalid) as u32;
    pub const IGNORES_BLOCKING: u32 = BF + offset_of!(BFUnitTail, ignores_blocking) as u32;

    /// `ZTUnit` part.
    pub const BUILDING_OFFSET: u32 = ZT + offset_of!(ZTUnitTail, building_offset) as u32;
    pub const POSITION_B: u32 = ZT + offset_of!(ZTUnitTail, position_b) as u32;
    pub const PREDATOR: u32 = ZT + offset_of!(ZTUnitTail, predator) as u32;
    pub const NON_PATH_COST: u32 = ZT + offset_of!(ZTUnitTail, non_path_cost) as u32;
    pub const TARGET_TILE: u32 = ZT + offset_of!(ZTUnitTail, target_tile) as u32;
    pub const TARGET_POSITION: u32 = ZT + offset_of!(ZTUnitTail, target_position) as u32;
    pub const BUILDING_TILE: u32 = ZT + offset_of!(ZTUnitTail, building_tile) as u32;
    pub const GATE: u32 = ZT + offset_of!(ZTUnitTail, gate) as u32;
    pub const PREDATOR_CHECK_PENDING: u32 = ZT + offset_of!(ZTUnitTail, predator_check_pending) as u32;
    pub const BUILDING_SLOT: u32 = ZT + offset_of!(ZTUnitTail, building_slot) as u32;
    pub const NEED_INDEX: u32 = ZT + offset_of!(ZTUnitTail, need_index) as u32;
    pub const SHOW_FLAG_A: u32 = ZT + offset_of!(ZTUnitTail, show_flag_a) as u32;
    pub const SHOW_FLAG_B: u32 = ZT + offset_of!(ZTUnitTail, show_flag_b) as u32;
    pub const IN_SHOW: u32 = ZT + offset_of!(ZTUnitTail, in_show) as u32;
    pub const TRICK_TYPE: u32 = ZT + offset_of!(ZTUnitTail, trick_type) as u32;
    pub const SHOW_INFO_ID: u32 = ZT + offset_of!(ZTUnitTail, show_info_id) as u32;
    pub const TANK_MODE: u32 = ZT + offset_of!(ZTUnitTail, tank_mode) as u32;
    pub const BUSY: u32 = ZT + offset_of!(ZTUnitTail, busy) as u32;
}

/// The addresses vanilla reads and writes, as the decompiles show them: a change to a tail struct that moves a field
/// fails the build here.
const _: () = {
    use unit_offsets::*;
    assert!(PATH_LIST == 0x154 && COG_GOAL == 0x15c && GOAL_ID == 0x170 && COG_GOAL_STATE == 0x174);
    assert!(TARGET_UNIT_ID == 0x178 && THOUGHT_STATE == 0x180 && COG_GOAL_FLAG == 0x1a8 && BUILDING_STATE == 0x1bc);
    assert!(MODE_LISTENER == 0x1c4 && DEPTH_MODE_BEGIN == 0x1ec && DEPTH_MODE_END == 0x1f0 && SPEED == 0x1f8);
    assert!(VERTICAL_DIRECTION == 0x208 && MOVE_ANIM_ENABLED == 0x20c && BYTE_20D == 0x20d && COG_INVALID == 0x20e);
    assert!(IGNORES_BLOCKING == 0x210 && BUILDING_OFFSET == 0x214 && POSITION_B == 0x220 && PREDATOR == 0x22c);
    assert!(NON_PATH_COST == 0x230 && TARGET_TILE == 0x234 && TARGET_POSITION == 0x238 && BUILDING_TILE == 0x244);
    assert!(GATE == 0x248 && PREDATOR_CHECK_PENDING == 0x24c && BUILDING_SLOT == 0x24d && NEED_INDEX == 0x24e);
    assert!(SHOW_FLAG_A == 0x24f && SHOW_FLAG_B == 0x250 && IN_SHOW == 0x251 && TRICK_TYPE == 0x252);
    assert!(SHOW_INFO_ID == 0x254 && TANK_MODE == 0x258 && BUSY == 0x25c);
};

/// `ZTWorldMgr` map offsets (from the world manager): tile count along x and y, and the tile array.
const WORLD_MAP_X: u32 = std::mem::offset_of!(ZTWorldMgr, map_x_size) as u32;
const WORLD_MAP_Y: u32 = std::mem::offset_of!(ZTWorldMgr, map_y_size) as u32;
const WORLD_TILE_ARRAY: u32 = std::mem::offset_of!(ZTWorldMgr, tile_array) as u32;
const TILE_SIZE: u32 = tile_offsets::SIZE;
const _: () = assert!(WORLD_MAP_X == 0x34 && WORLD_MAP_Y == 0x38 && WORLD_TILE_ARRAY == 0x40);

/// RVAs of the game `rand` state (`DAT_00638060`), the "no position" default (`DAT_00639370`, three words), the
/// data `goalSet` passes `BFCogGoal::set` (`DAT_00639360`) and the "building" need name (`DAT_0063afd8`).
const RNG_STATE_RVA: u32 = 0x0023_8060;
const DEFAULT_POSITION_RVA: u32 = 0x0023_9370;
const GOAL_SET_DATA_RVA: u32 = 0x0023_9360;
const BUILDING_NEED_NAME_RVA: u32 = 0x0023_afd8;
/// The `BFApp` object (`appInitSuccess`, `0x00638148`): `loadString`'s `this`, and its byte `+0x441` is set while a save loads.
const APP_RVA: u32 = 0x0023_8148;
const APP_LOADING_FLAG: u32 = 0x441;
/// The text `loadString` answers for a type without a name (`DAT_00638084`), and the separator between a name and its
/// unit count (`DAT_00639110`), as RVAs of NUL-terminated strings.
const NO_NAME_TEXT_RVA: u32 = 0x0023_8084;
const COUNT_SEPARATOR_RVA: u32 = 0x0023_9110;
/// `BFEntityType` byte that makes `createName` append a unit count, and the count's embedded table.
const TYPE_NUMBERED_NAMES: u32 = 0x5b;
const TYPE_UNIT_COUNTS: u32 = 0x98;
/// The animation set a mermaid unit plays when it is added to the map.
const MERMAID_TRANSFORM_SET: &[u8] = b"bMermtrans";
/// The entity type id of the unit type that transforms (`0x1530`).
const MERMAID_TYPE_ID: u32 = 0x1530;
/// `BFEntity` byte `addToMap` and `removeFromMap` tests alongside `+0x142`.
const ENTITY_FLAG_143: u32 = 0x143;
/// The mixer `GXMixer` embedded in the entity.
const ENTITY_MIXER: u32 = 0x74;

/// `isCastClass` tags: `ZTAnimal` (`DAT_00638690`), `ZTGuest` (`DAT_00638700`), `ZTBuilding` (`DAT_00638680`).
const ANIMAL_TAG_RVA: u32 = 0x0023_8690;
const GUEST_TAG_RVA: u32 = 0x0023_8700;
const BUILDING_TAG_RVA: u32 = 0x0023_8680;

/// Goal ids the unit's `getRandomOK`/`isDoingSomething` treat as "doing something": the show goals `0x66`-`0x6a`
/// (`0x66`/`0x67` performer, `0x68`-`0x6a` building use), and the two `0x80`/`0x81` goals.
fn is_performing_goal(goal: u32) -> bool {
    (0x66..=0x6a).contains(&goal)
}

fn is_building_use_goal(goal: u32) -> bool {
    (0x68..=0x6a).contains(&goal)
}

/// Tank-mode values the animation getters compare against.
const TANK_MODE_SURFACE: u32 = 3;
const TANK_MODE_UNDERWATER: u32 = 4;

/// `BFTile` field offsets, taken from the struct: map coordinates, the entity slots, the four edge (fence) slots and
/// the bytes the cliff, water, walkable and ramp tests read.
mod tile_offsets {
    use std::mem::{offset_of, size_of};

    use crate::ztmapview::BFTile;

    pub const SIZE: u32 = size_of::<BFTile>() as u32;
    pub const X: u32 = offset_of!(BFTile, pos) as u32;
    pub const Y: u32 = X + 4;
    /// The four entity slots of a tile, contiguous from here; the last is `BFTile::entity_ptr`.
    pub const ENTITY_SLOTS: u32 = offset_of!(BFTile, quadrant_entities) as u32;
    /// The tile's own first word: the sentinel of the list of entities standing on it.
    pub const ENTITY_LIST: u32 = offset_of!(BFTile, unit_list_ptr) as u32;
    /// The four edge entities (fences), one per direction pair, contiguous from here.
    pub const EDGE_ENTITIES: u32 = offset_of!(BFTile, north_fence) as u32;
    pub const CLIFF_BITS: u32 = offset_of!(BFTile, cliff_bits) as u32;
    pub const TERRAIN_FLAGS: u32 = offset_of!(BFTile, terrain_flags) as u32;
    pub const HABITAT_FLAGS: u32 = offset_of!(BFTile, habitat_flags) as u32;
    /// The slot of the fourth quadrant entity.
    pub const ENTITY_PTR: u32 = offset_of!(BFTile, entity_ptr) as u32;
    /// `TERRAIN_FLAGS` bits: water, walkable. `HABITAT_FLAGS` bit: ramp / tank tile.
    pub const WATER_BITS: u8 = 0x3;
    pub const WALKABLE_BIT: u8 = 0x8;
    pub const TANK_BIT: u8 = 0x20;

    const _: () = assert!(ENTITY_SLOTS + 12 == ENTITY_PTR);
}

/// `ZTHabitat` fields `getHabitatFromGate` reads.
const HABITAT_GATE_TILE_OFFSET: u32 = 0x8c;
const HABITAT_GATE_DIRECTION_OFFSET: u32 = 0x90;
/// `ZTShowInfo` field `getHabitat` answers when it is set.
const SHOW_INFO_HABITAT_OFFSET: u32 = 0xa0;

/// `ZTUnitType` fields the getters read.
mod type_offsets {
    /// Idle animation used while swimming (`getIdleAnim`).
    pub const WATER_IDLE_ANIM: u32 = 0x1b0;
    /// Depth of the unit's own type, and of the default.
    pub const DEPTH: u32 = 0x64;
    pub const DEFAULT_DEPTH: u32 = 0x68;
    /// Walking speeds (`u16`): slow, medium, fast.
    pub const SLOW_SPEED: u32 = 0x10c;
    pub const MEDIUM_SPEED: u32 = 0x10e;
    pub const FAST_SPEED: u32 = 0x110;
    /// Swimming speeds (`u16`): slow, medium, fast.
    pub const WATER_SLOW_SPEED: u32 = 0x154;
    pub const WATER_MEDIUM_SPEED: u32 = 0x156;
    pub const WATER_FAST_SPEED: u32 = 0x158;
    /// Ambient animation lists. `initAmbientAnims` fills them; `doAmbientAnims` picks one by tank mode.
    pub const AMBIENT_LAND: u32 = 0x4;
    pub const AMBIENT_WATER: u32 = 0x188;
    pub const AMBIENT_UNDERWATER: u32 = 0x194;
    pub const AMBIENT_SURFACE: u32 = 0x1a0;
}

/// Vtable slots (byte offsets) on the unit that the methods dispatch through, so a subclass override is honoured.
mod unit_slots {
    pub const GET_IDLE_ANIM: u32 = 0x8c;
    pub const GET_SWIMS: u32 = 0xbc;
    pub const GET_UNDERWATER: u32 = 0x16c;
    pub const GET_HABITAT_FROM_TILE: u32 = 0x248;
    pub const GET_HABITAT: u32 = 0x24c;
    pub const IS_RETURNING: u32 = 0x264;
    pub const IS_IN_TANK: u32 = 0x268;
    pub const GET_TANK_MODE: u32 = 0x2c8;
    /// `BFEntity::isWalkableBy(unit)`, called on the entities standing on a tile.
    pub const IS_WALKABLE_BY: u32 = 0xb8;
}

/// Vtable slots on the unit's `ZTUnitType`.
mod type_slots {
    pub const GET_IDLE_ANIM_SYMBOL: u32 = 0xa4;
    pub const GET_SLOW_ANIM_SYMBOL: u32 = 0xa8;
    pub const GET_MEDIUM_ANIM_SYMBOL: u32 = 0xac;
    pub const GET_FAST_ANIM_SYMBOL: u32 = 0xb0;
    pub const GET_IDLE_ANIM_SYMBOL_WATER: u32 = 0xb4;
    pub const GET_MOVE_ANIM_SYMBOL_WATER: u32 = 0xb8;
    pub const GET_SURFACE: u32 = 0xc0;
    pub const GET_UNDERWATER: u32 = 0xc4;
    pub const GET_ONLY_UNDERWATER: u32 = 0xc8;
    pub const GET_SWIMS: u32 = 0x44;
    /// `ZTHabitat::isTank`, on a habitat.
    pub const HABITAT_IS_TANK: u32 = 0x20;
}

/// Calls one of the `ZTUnitType` symbol helpers; the string it returns is vanilla-owned.
fn symbol(helper: FunctionDef<unsafe extern "stdcall" fn() -> *const u32>) -> u32 {
    unsafe { helper.original()() as u32 }
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// The address of the habitat manager's grid cell for `tile` (`ZTHabitatMgr::get_habitat_cell_addr`: its first word is
/// the habitat, the following eight the neighbour entries), or `None` with no manager or tile, or a tile off the grid.
fn habitat_cell(tile: u32) -> Option<u32> {
    let manager: u32 = get_from_memory(base() + HABITAT_MGR_RVA);
    if manager == 0 || tile == 0 {
        return None;
    }
    let (x, y): (i32, i32) = (get_from_memory(tile + tile_offsets::X), get_from_memory(tile + tile_offsets::Y));
    unsafe { ref_from_memory::<ZTHabitatMgr>(manager) }.get_habitat_cell_addr(x, y)
}

/// The habitat the habitat manager records for `tile`'s column and row, or `0` with no manager or tile.
fn habitat_at_tile(tile: u32) -> u32 {
    habitat_cell(tile).map_or(0, get_from_memory::<u32>)
}

/// A unit in vanilla memory, addressed by its raw pointer. All reads and writes are volatile through the address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTUnit(pub u32);

impl ZTUnit {
    fn read<T>(self, offset: u32) -> T {
        get_from_memory(self.0 + offset)
    }

    fn write<T>(self, offset: u32, value: T) {
        save_to_memory(self.0 + offset, value);
    }

    /// The unit's entity type when it is a unit type (`isCastClass` against [`UNIT_TYPE_TAG_RVA`]), else `0`. Each call
    /// is one `isCastClass` dispatch, matching the cast vanilla inlines at each use.
    fn unit_type(self) -> u32 {
        if unsafe { entity_type_matches(self.0, UNIT_TYPE_TAG_RVA) } {
            self.read(unit_offsets::ENTITY_TYPE)
        } else {
            0
        }
    }

    fn tile(self) -> u32 {
        unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) as u32 }
    }

    fn slot_bool(self, slot: u32) -> bool {
        low_byte_bool(call_slot_0(self.0, slot))
    }

    fn type_bool(unit_type: u32, slot: u32) -> bool {
        unit_type != 0 && low_byte_bool(call_slot_0(unit_type, slot))
    }

    fn type_u16(unit_type: u32, offset: u32) -> u32 {
        if unit_type == 0 {
            0
        } else {
            u32::from(get_from_memory::<u16>(unit_type + offset))
        }
    }

    /// `ZTUnit::getTankMode` (`0x00410919`).
    pub fn get_tank_mode(self) -> u32 {
        self.read(unit_offsets::TANK_MODE)
    }

    /// `ZTUnit::setTankMode` (`0x004930dd`).
    pub fn set_tank_mode(self, mode: u32) {
        self.write(unit_offsets::TANK_MODE, mode);
    }

    /// `ZTUnit::noShadowPart` (`0x00492e0a`): `0` for the surface and underwater modes, `1` otherwise.
    pub fn no_shadow_part(self) -> u32 {
        let mode = self.get_tank_mode();
        u32::from(mode != TANK_MODE_UNDERWATER && mode != TANK_MODE_SURFACE)
    }

    /// `ZTUnit::noUnderPart` (`0x004968ce`): true unless the unit is on the surface.
    pub fn no_under_part(self) -> bool {
        self.get_tank_mode() != TANK_MODE_SURFACE
    }

    /// `ZTUnit::isOnPlatform` (`0x00496888`): the unit's tank mode (through the `getTankMode` slot) is `2`.
    pub fn is_on_platform(self) -> bool {
        call_slot_0(self.0, unit_slots::GET_TANK_MODE) == 2
    }

    /// `ZTUnit::isWaterTraveling` (`0x0060c9ab`): the `isReturning` slot's answer.
    pub fn is_water_traveling(self) -> bool {
        self.slot_bool(unit_slots::IS_RETURNING)
    }

    /// `ZTUnit::isInTank` (`0x004106c4`): the tile under the unit has its tank bit (`+0x85`, `0x20`).
    pub fn is_in_tank(self) -> u32 {
        let tile = self.tile();
        u32::from(tile != 0 && get_from_memory::<u8>(tile + tile_offsets::HABITAT_FLAGS) & tile_offsets::TANK_BIT != 0)
    }

    /// `ZTUnit::isOnWater` (`0x00410fe0`): the tile under the unit has a water bit (`+0x83`, `0x3`).
    pub fn is_on_water(self) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = live_support::on_water_override::intercept(self.0) {
            return answer;
        }
        let tile = self.tile();
        tile != 0 && get_from_memory::<u8>(tile + tile_offsets::TERRAIN_FLAGS) & tile_offsets::WATER_BITS != 0
    }

    /// `ZTUnit::getHabitatFromTile` (`0x0041060e`): the habitat manager's cell for `tile`.
    pub fn get_habitat_from_tile(self, tile: u32) -> u32 {
        habitat_at_tile(tile)
    }

    /// `ZTUnit::getHabitat` (`0x00410642`): the habitat of the show the unit performs in when that show has one,
    /// otherwise (for a unit standing on a tile) the `getHabitatFromTile` slot's answer.
    pub fn get_habitat(self) -> u32 {
        let tile = self.tile();
        if self.read::<u8>(unit_offsets::IN_SHOW) != 0 {
            let show_info = ZTShowMgr::get_show_info(self.read::<u16>(unit_offsets::SHOW_INFO_ID));
            if show_info != 0 {
                let habitat: u32 = get_from_memory(show_info + SHOW_INFO_HABITAT_OFFSET);
                if habitat != 0 {
                    return habitat;
                }
            }
        }
        if tile == 0 {
            return 0;
        }
        call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, tile)
    }

    /// `ZTUnit::getTank` (`0x004106dd`): unless `flag`, the `getHabitat` slot's habitat when it is a tank.
    pub fn get_tank(self, flag: bool) -> u32 {
        if !flag {
            let habitat = call_slot_0(self.0, unit_slots::GET_HABITAT);
            if habitat != 0 && low_byte_bool(call_slot_0(habitat, type_slots::HABITAT_IS_TANK)) {
                return habitat;
            }
        }
        0
    }

    /// `ZTUnit::getHabitatFromGate` (`0x006130df`): the habitat whose gate tile is `tile`, or the one on the other
    /// side of the gate (the neighbour of `tile` in the habitat's gate direction).
    pub fn get_habitat_from_gate(self, tile: u32) -> u32 {
        if get_from_memory::<u32>(base() + HABITAT_MGR_RVA) == 0 {
            return 0;
        }
        let habitat = habitat_at_tile(tile);
        if habitat == 0 || get_from_memory::<u32>(habitat + HABITAT_GATE_TILE_OFFSET) == tile {
            return habitat;
        }
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let direction: u32 = get_from_memory(habitat + HABITAT_GATE_DIRECTION_OFFSET);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, direction) };
        if neighbour == 0 {
            0
        } else {
            habitat_at_tile(neighbour)
        }
    }

    /// `ZTUnit::getSwims` (`0x00410764`): the unit type's `getSwims` slot.
    pub fn get_swims(self) -> bool {
        Self::type_bool(self.unit_type(), type_slots::GET_SWIMS)
    }

    /// `ZTUnit::getUnderwater` (`0x00410794`): the unit type's `getUnderwater` slot.
    pub fn get_underwater(self) -> bool {
        Self::type_bool(self.unit_type(), type_slots::GET_UNDERWATER)
    }

    /// `ZTUnit::getOnlyUnderwater` (`0x00410695`): the unit type's `getOnlyUnderwater` slot.
    pub fn get_only_underwater(self) -> bool {
        Self::type_bool(self.unit_type(), type_slots::GET_ONLY_UNDERWATER)
    }

    /// `ZTUnit::getDepth` (`0x005af19b`): the deepest of the unit type's default depth and the depth of every entity on
    /// `tile` that is not walkable by the unit.
    pub fn get_depth(self, tile: u32) -> i32 {
        let unit_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if unit_type == 0 {
            return 0;
        }
        let mut depth: i32 = get_from_memory(unit_type + type_offsets::DEFAULT_DEPTH);
        if tile == 0 {
            return depth;
        }
        for slot in 0..4 {
            let entity: u32 = get_from_memory(tile + tile_offsets::ENTITY_SLOTS + slot * 4);
            if entity == 0 {
                continue;
            }
            if !low_byte_bool(call_slot_1(entity, unit_slots::IS_WALKABLE_BY, self.0)) {
                let entity_type: u32 = get_from_memory(entity + unit_offsets::ENTITY_TYPE);
                depth = depth.max(get_from_memory::<i32>(entity_type + type_offsets::DEPTH));
            }
        }
        depth
    }

    /// `ZTUnit::getIdleAnim` (`0x0041338a`): a unit that is on water and swims uses its type's water idle animation
    /// unless `force_unit`; everything else is `BFUnit::getIdleAnim`.
    pub fn get_idle_anim(self, force_unit: bool) -> i32 {
        let unit_type = self.unit_type();
        if !force_unit && self.is_on_water() && self.slot_bool(unit_slots::GET_SWIMS) && unit_type != 0 {
            return get_from_memory(unit_type + type_offsets::WATER_IDLE_ANIM);
        }
        unsafe { BFUNIT_GET_IDLE_ANIM.hooked()(self.0 as *const u32, force_unit) }
    }

    /// `ZTUnit::getIconAnim` (`0x004ee499`): the `getIdleAnim` slot with `force_unit` set.
    pub fn get_icon_anim(self) -> i32 {
        call_slot_1(self.0, unit_slots::GET_IDLE_ANIM, 1) as i32
    }

    /// Whether the unit is underwater (`getUnderwater` slot and tank mode `4`).
    fn is_submerged(self) -> bool {
        self.slot_bool(unit_slots::GET_UNDERWATER) && self.get_tank_mode() == TANK_MODE_UNDERWATER
    }

    /// Whether the unit is on the surface (`getUnderwater` slot and tank mode `3`).
    fn is_on_surface(self) -> bool {
        self.slot_bool(unit_slots::GET_UNDERWATER) && self.get_tank_mode() == TANK_MODE_SURFACE
    }

    /// Whether the unit swims and is on water.
    fn is_swimming_on_water(self) -> bool {
        self.slot_bool(unit_slots::GET_SWIMS) && self.is_on_water()
    }

    /// The submerged swim symbol: diving, rising or level.
    fn submerged_symbol(self) -> u32 {
        match self.read::<i32>(unit_offsets::VERTICAL_DIRECTION) {
            -1 => symbol(GET_DIVE_ANIM_SYMBOL),
            1 => symbol(GET_RISE_ANIM_SYMBOL),
            _ => symbol(GET_SUB_SWIM_ANIM_SYMBOL),
        }
    }

    /// The shared shape of the three move-symbol getters: the submerged and surface swim symbols, the type's water
    /// symbol while swimming on water, and the type's own symbol slot otherwise.
    fn move_anim_symbol(self, unit_type: u32, own_slot: u32) -> u32 {
        if self.is_submerged() {
            return self.submerged_symbol();
        }
        if self.is_on_surface() {
            return symbol(GET_SURF_SWIM_ANIM_SYMBOL);
        }
        if self.is_swimming_on_water() {
            return if unit_type == 0 { 0 } else { call_slot_0(unit_type, type_slots::GET_MOVE_ANIM_SYMBOL_WATER) };
        }
        let unit_type = self.unit_type();
        if unit_type == 0 {
            0
        } else {
            call_slot_0(unit_type, own_slot)
        }
    }

    /// `ZTUnit::getIdleAnimSymbol` (`0x00413491`).
    pub fn get_idle_anim_symbol(self) -> u32 {
        let unit_type = self.unit_type();
        if self.is_submerged() {
            return symbol(GET_SUB_IDLE_ANIM_SYMBOL);
        }
        if self.is_on_surface() {
            return symbol(GET_SURF_IDLE_ANIM_SYMBOL);
        }
        if self.is_swimming_on_water() {
            return if unit_type == 0 { 0 } else { call_slot_0(unit_type, type_slots::GET_IDLE_ANIM_SYMBOL_WATER) };
        }
        let unit_type = self.unit_type();
        if unit_type == 0 {
            0
        } else {
            call_slot_0(unit_type, type_slots::GET_IDLE_ANIM_SYMBOL)
        }
    }

    /// `ZTUnit::getSlowAnimSymbol` (`0x004135fd`).
    pub fn get_slow_anim_symbol(self) -> u32 {
        let unit_type = self.unit_type();
        self.move_anim_symbol(unit_type, type_slots::GET_SLOW_ANIM_SYMBOL)
    }

    /// `ZTUnit::getMediumAnimSymbol` (`0x004a30da`).
    pub fn get_medium_anim_symbol(self) -> u32 {
        let unit_type = self.unit_type();
        self.move_anim_symbol(unit_type, type_slots::GET_MEDIUM_ANIM_SYMBOL)
    }

    /// `ZTUnit::getFastAnimSymbol` (`0x004a5105`).
    pub fn get_fast_anim_symbol(self) -> u32 {
        let unit_type = self.unit_type();
        self.move_anim_symbol(unit_type, type_slots::GET_FAST_ANIM_SYMBOL)
    }

    /// `ZTUnit::getSlowAnimSpeed` (`0x004136bf`): the type's water speed in either swim tank mode or on water, else
    /// its land speed.
    pub fn get_slow_anim_speed(self) -> u32 {
        let unit_type = self.unit_type();
        let mode = self.get_tank_mode();
        if mode == TANK_MODE_UNDERWATER || mode == TANK_MODE_SURFACE || self.is_on_water() {
            return Self::type_u16(unit_type, type_offsets::WATER_SLOW_SPEED);
        }
        Self::type_u16(self.unit_type(), type_offsets::SLOW_SPEED)
    }

    /// `ZTUnit::getMediumAnimSpeed` (`0x004a319c`): the type's water speed while in a tank or on water, else its land
    /// speed.
    pub fn get_medium_anim_speed(self) -> u32 {
        let unit_type = self.unit_type();
        if self.slot_bool(unit_slots::IS_IN_TANK) || self.is_on_water() {
            return Self::type_u16(unit_type, type_offsets::WATER_MEDIUM_SPEED);
        }
        Self::type_u16(self.unit_type(), type_offsets::MEDIUM_SPEED)
    }

    /// `ZTUnit::getFastAnimSpeed` (`0x004a5194`): as [`Self::get_medium_anim_speed`] for the fast speeds.
    pub fn get_fast_anim_speed(self) -> u32 {
        let unit_type = self.unit_type();
        if self.slot_bool(unit_slots::IS_IN_TANK) || self.is_on_water() {
            return Self::type_u16(unit_type, type_offsets::WATER_FAST_SPEED);
        }
        Self::type_u16(self.unit_type(), type_offsets::FAST_SPEED)
    }

    /// `ZTUnit::doAmbientAnims` (`0x0043ca83`): nothing while the unit is returning; otherwise the ambient list of the
    /// unit's tank mode, falling back to `ambient` (the caller's list).
    pub fn do_ambient_anims(self, ambient: u32) {
        if self.slot_bool(unit_slots::IS_RETURNING) {
            return;
        }
        let unit_type = self.unit_type();
        let in_type = |offset: u32| (unit_type != 0).then_some(unit_type + offset);
        let list = if self.is_submerged() {
            in_type(type_offsets::AMBIENT_UNDERWATER)
        } else if self.is_on_surface() {
            in_type(type_offsets::AMBIENT_SURFACE)
        } else if self.is_swimming_on_water() {
            in_type(type_offsets::AMBIENT_WATER)
        } else {
            Some(ambient)
        };
        if let Some(list) = list {
            unsafe { BFUNIT_DO_AMBIENT_ANIMS.hooked()(self.0 as *const u32, list as *const u32) };
        }
    }

    /// `ZTUnit::initAmbientAnims` (`0x004f53bd`): runs `BFUnit::initAmbientAnims` over `list` when given, then over the
    /// type's land, water, underwater and surface lists as the type's capabilities allow. A unit without a unit type
    /// only runs the `list` pass (vanilla faults).
    pub fn init_ambient_anims(self, list: u32, name: *const i8) {
        let unit_type = self.unit_type();
        let run = |vector: u32, key: *const i8| unsafe { BFUNIT_INIT_AMBIENT_ANIMS.hooked()(self.0 as *const u32, vector as *const u32, key) };
        if list != 0 {
            run(list, name);
        }
        if unit_type == 0 {
            return;
        }
        if !low_byte_bool(call_slot_0(unit_type, type_slots::GET_ONLY_UNDERWATER)) {
            run(unit_type + type_offsets::AMBIENT_LAND, name);
        }
        if low_byte_bool(call_slot_0(unit_type, type_slots::GET_SWIMS)) {
            run(unit_type + type_offsets::AMBIENT_WATER, c"AmbientAnimsWater".as_ptr());
        }
        if low_byte_bool(call_slot_0(unit_type, type_slots::GET_UNDERWATER)) {
            if !low_byte_bool(call_slot_0(unit_type, type_slots::GET_SURFACE)) {
                run(unit_type + type_offsets::AMBIENT_UNDERWATER, c"AmbientAnimsUnderwater".as_ptr());
            }
            run(unit_type + type_offsets::AMBIENT_SURFACE, c"AmbientAnimsSurface".as_ptr());
        }
    }
}

/// Vtable slots (byte offsets) the lifecycle and goal methods dispatch through.
mod goal_slots {
    pub const UPDATE_SUB_GOALS: u32 = 0x3c;
    pub const RESET_MOVE_ANIM: u32 = 0x138;
    pub const CLEAR_GOAL: u32 = 0x174;
    pub const STOP_EVERYTHING: u32 = 0x198;
    pub const GOAL_SET: u32 = 0x1d8;
    pub const IS_SHOW_UNIT: u32 = 0x228;
    pub const IS_DOING_SHOW: u32 = 0x22c;
    pub const GET_HABITAT: u32 = 0x24c;
    pub const GET_TANK: u32 = 0x250;
    pub const END_TRICK: u32 = 0x2bc;
    pub const ABORT_TRICK: u32 = 0x2c0;
    pub const FORCE_UI_UPDATE: u32 = 0x2d0;
    pub const SET_TARGET_TILE: u32 = 0x2d8;
    pub const SET_BUILDING_TILE: u32 = 0x2e4;
    /// `BFCogGoal::update`, on the embedded goal.
    pub const COG_GOAL_UPDATE: u32 = 0x24;
    pub const GET_ONLY_UNDERWATER: u32 = 0x170;
    pub const SET_NAME: u32 = 0x1c;
    pub const SET_VISIBLE: u32 = 0xcc;
    pub const VALIDATE_TANK_POSITION: u32 = 0x2d4;
    pub const VALIDATE_BUILDING: u32 = 0x2e8;
    pub const RUN_BEHAVIOR_SET: u32 = 0x34;
    pub const VALIDATE_POSITION: u32 = 0x84;
    pub const RESET_IDLE_ANIM: u32 = 0x13c;
    pub const INVALIDATE_TARGET: u32 = 0x17c;
    pub const SET_HEIGHT_ABOVE_TERRAIN: u32 = 0x1cc;
    pub const SET_TANK_MODE: u32 = 0x2cc;
    /// `BFEntityType` slot answering the type id (`0x1530` for the unit type `resetAI` special-cases).
    pub const TYPE_ID: u32 = 0x20;
}

/// `ZTTankExhibit` fields `validateTankPosition` reads: the ladder levels (in sixteenths of a tile).
const TANK_LADDER_A: u32 = 0x184;
const TANK_LADDER_B: u32 = 0x188;
/// The fence type's name pointer (`+0xa4`); a gate's name starts with `g`.
const FENCE_TYPE_NAME: u32 = 0xa4;
/// `isCastClass` tag for `ZTFence` (`DAT_00638660`).
const FENCE_TAG_RVA: u32 = 0x0023_8660;

/// `BFMap::getCollision` (`0x00414852`, `this` is the map at `ZTWorldMgr + 8`): whether `unit` collides with `building`
/// standing at `position` on `tile`. Identified from the macOS `BFMap::getCollision`; not yet in `generated.rs`.
const BFMAP_GET_COLLISION: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32, *const u32, *const i32, *const u32) -> u32> = FunctionDef::new(0x0041_4852);

/// `ZTUnit::getGate` (`0x00436954`), vanilla. Identified from the macOS `ZTUnit::getGate`; not yet in `generated.rs`.
/// Only the live tests call it: [`ZTUnit::get_gate`] is the port.
#[cfg(feature = "reimplementation-tests")]
pub(crate) const ZTUNIT_GET_GATE: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32, u32) -> u32> = FunctionDef::new(0x0043_6954);

fn call_slot_2_returning(this: u32, offset: u32, first: u32, second: u32) -> u32 {
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(this) + offset)) };
    function(this, first, second)
}

/// The mixer queue `initAfterLoad` empties: a `std::deque<vector<GXMixerLink>>` whose `start` iterator sits at `0x8c`
/// (`{cur, first, last, node}`, the `finish` iterator following at `0x9c`). Each block holds `0x1f8` bytes of
/// `vector<GXMixerLink>` elements (`0xc` bytes each).
mod mixer_queue {
    pub const DEQUE_START: u32 = 0x8c;
    pub const BLOCK_BYTES: u32 = 0x1f8;
    pub const ELEMENT_BYTES: u32 = 0xc;
}

/// Advances the game's `rand` state and returns the `rand()` result (`0..=0x7fff`).
fn game_rand() -> i32 {
    let state_address = base() + RNG_STATE_RVA;
    let state: u32 = get_from_memory(state_address);
    let next = state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
    save_to_memory(state_address, next);
    ((next >> 16) & 0x7fff) as i32
}

fn default_position() -> [u32; 3] {
    let at = base() + DEFAULT_POSITION_RVA;
    [get_from_memory(at), get_from_memory(at + 4), get_from_memory(at + 8)]
}

/// The tile of the zoo map at `(x, y)`, or `0` outside the map.
fn map_tile(x: i32, y: i32) -> u32 {
    let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
    let width: i32 = get_from_memory(world + WORLD_MAP_X);
    let height: i32 = get_from_memory(world + WORLD_MAP_Y);
    if x < 0 || y < 0 || x >= width || y >= height {
        return 0;
    }
    let array: u32 = get_from_memory(world + WORLD_TILE_ARRAY);
    array.wrapping_add(((width.wrapping_mul(y) + x) as u32).wrapping_mul(TILE_SIZE))
}

/// Frees a vanilla `{begin, end, cap}` string the way its destructor does.
fn free_string(string: &[u32; 3]) {
    if string[0] != 0 {
        unsafe { POOLALLOC_DEALLOCATE.original()(string[0] as *const u32, string[2].wrapping_sub(string[0])) };
    }
}

impl ZTUnit {
    fn write_default_position(self, offset: u32) {
        let [x, y, z] = default_position();
        self.write(offset, x);
        self.write(offset + 4, y);
        self.write(offset + 8, z);
    }

    fn is_guest(self) -> bool {
        unsafe { entity_type_matches(self.0, GUEST_TAG_RVA) }
    }

    fn is_animal(self) -> bool {
        unsafe { entity_type_matches(self.0, ANIMAL_TAG_RVA) }
    }

    /// `ZTUnit::ZTUnit` (`0x004f578d`): `BFUnit`'s constructor, then the unit's own fields. Returns `this`.
    pub fn construct(self, entity_type: u32) -> u32 {
        unsafe { BFUNIT_CONSTRUCTOR.hooked()(self.0 as *const u32, entity_type) };
        for offset in (unit_offsets::BUILDING_OFFSET..unit_offsets::PREDATOR).step_by(4) {
            self.write(offset, 0u32);
        }
        self.write(unit_offsets::TARGET_TILE, 0u32);
        self.write_default_position(unit_offsets::TARGET_POSITION);
        self.write(unit_offsets::BUILDING_TILE, 0u32);
        self.write(unit_offsets::GATE, 0u32);
        self.write(unit_offsets::TRICK_TYPE, 0u16);
        self.write(unit_offsets::SHOW_INFO_ID, 0u16);
        self.write(unit_offsets::TANK_MODE, 0u32);
        self.write(unit_offsets::SHOW_FLAG_A, 0u8);
        self.write(unit_offsets::SHOW_FLAG_B, 0u8);
        self.write(unit_offsets::IN_SHOW, 0u8);
        self.write(unit_offsets::NEED_INDEX - 1, 0xffu8);
        self.write(unit_offsets::NEED_INDEX, 0xffu8);
        self.write(unit_offsets::NON_PATH_COST, 1u32);
        self.write(0u32, base() + UNIT_VTABLE_RVA);
        self.0
    }

    /// `ZTUnit::initStatusVars` (`0x004f5c58`).
    pub fn init_status_vars(self) {
        unsafe { BFENTITY_INIT_STATUS_VARS.hooked()(self.0 as *const u32) };
        self.write(unit_offsets::PREDATOR_CHECK_PENDING, 0u8);
        self.write(unit_offsets::PREDATOR, 0u32);
        self.write(unit_offsets::BUSY, 0u8);
    }

    /// `ZTUnit::clearStateVariables` (`0x0046ad2c`).
    pub fn clear_state_variables(self) {
        self.write_default_position(unit_offsets::BUILDING_OFFSET);
        self.write_default_position(unit_offsets::POSITION_B);
        self.write(unit_offsets::PREDATOR, 0u32);
        self.write(unit_offsets::NON_PATH_COST, 1u32);
        self.write(unit_offsets::TARGET_TILE, 0u32);
        self.write_default_position(unit_offsets::TARGET_POSITION);
        self.write(unit_offsets::BUILDING_TILE, 0u32);
        self.write(unit_offsets::GATE, 0u32);
        self.write(unit_offsets::PREDATOR_CHECK_PENDING, 0u8);
        self.write(unit_offsets::BUSY, 0u8);
        self.write(unit_offsets::NEED_INDEX - 1, 0xffu8);
        self.write(unit_offsets::NEED_INDEX, 0xffu8);
        self.write(unit_offsets::TANK_MODE, 0u32);
        self.write(unit_offsets::BYTE_20D, 0u8);
    }

    /// `ZTUnit::setIsRemoved` (`0x004fa78e`): lets the map view prepare to drop the entity, then sets the removed flag.
    pub fn set_is_removed(self, flag: u8) {
        let map_view = unsafe { GET_MAPVIEW.hooked()() } as u32;
        if map_view != 0 {
            unsafe {
                CHECK_UNDO_BUFFER.hooked()(map_view as *const u32, self.0 as *const i32);
                PREPARE_TO_DELETE_ENTITY.hooked()(map_view as *const u32, self.0 as *const u32);
            }
        }
        self.write(unit_offsets::REMOVED_FLAG_B, 0u8);
        self.write(unit_offsets::REMOVED_FLAG, flag);
    }

    /// `ZTUnit::save` (`0x00478501`).
    pub fn save(self, file: u32) -> bool {
        let file = file as *const u32;
        let write = |address: u32, size: u32| write_field(address, size, file);
        let at = |offset: u32| self.0 + offset;
        let mut ok = write(at(unit_offsets::PREDATOR_CHECK_PENDING), 1);
        ok &= write(at(unit_offsets::PREDATOR), 4);
        ok &= write(at(unit_offsets::BUSY), 1);
        for offset in [unit_offsets::TARGET_POSITION, unit_offsets::TARGET_POSITION + 4, unit_offsets::TARGET_POSITION + 8] {
            ok &= write(at(offset), 4);
        }
        let building_tile: u32 = self.read(unit_offsets::BUILDING_TILE);
        let [default_x, default_y, _] = default_position();
        let (x, y): (u32, u32) = if building_tile != 0 {
            (get_from_memory(building_tile + tile_offsets::X), get_from_memory(building_tile + tile_offsets::Y))
        } else {
            (default_x, default_y)
        };
        ok &= write(&x as *const u32 as u32, 4);
        ok &= write(&y as *const u32 as u32, 4);
        for offset in [unit_offsets::BUILDING_OFFSET, unit_offsets::BUILDING_OFFSET + 4, unit_offsets::POSITION_B, unit_offsets::POSITION_B + 4, unit_offsets::POSITION_B + 8] {
            ok &= write(at(offset), 4);
        }
        ok &= write(at(unit_offsets::NEED_INDEX - 1), 1);
        ok &= write(at(unit_offsets::NEED_INDEX), 1);
        ok &= write(at(unit_offsets::NON_PATH_COST), 4);
        if !self.is_guest() {
            for offset in [unit_offsets::TRICK_TYPE, unit_offsets::SHOW_INFO_ID] {
                let value: u16 = self.read(offset);
                let present = u32::from(value != 0);
                ok &= write(&present as *const u32 as u32, 1);
                if present == 1 {
                    ok &= write(at(offset), 2);
                }
            }
        }
        let flag_a = write(at(unit_offsets::SHOW_FLAG_A), 1);
        let flag_b = write(at(unit_offsets::SHOW_FLAG_B), 1);
        let base_ok = unsafe { BFUNIT_SAVE.hooked()(self.0 as *const u32, file) };
        base_ok & ok & flag_a & flag_b
    }

    /// `ZTUnit::load` (`0x00454488`). Saves older than version `0x5b` carry the positions in tile units and are read by
    /// the vanilla body; every later version is read here.
    pub fn load(self, file: u32, version: u32) -> bool {
        if version < 0x5b {
            return unit_detours::real_load(self.0, file, version);
        }
        let file = file as *const u32;
        let read = |offset: u32, size: u32| read_field(self.0 + offset, size, file);
        let mut ok = true;
        if version > 0x15 {
            ok &= read(unit_offsets::PREDATOR_CHECK_PENDING, 1);
        }
        if version > 0x22 {
            ok &= read(unit_offsets::PREDATOR, 4);
            ok &= read(unit_offsets::BUSY, 1);
            for offset in [unit_offsets::TARGET_POSITION, unit_offsets::TARGET_POSITION + 4, unit_offsets::TARGET_POSITION + 8] {
                ok &= read(offset, 4);
            }
            let position: [i32; 3] = [self.read(unit_offsets::TARGET_POSITION), self.read(unit_offsets::TARGET_POSITION + 4), self.read(unit_offsets::TARGET_POSITION + 8)];
            if position.map(|word| word as u32) == default_position() {
                self.write(unit_offsets::TARGET_TILE, 0u32);
            } else {
                self.write(unit_offsets::TARGET_TILE, map_tile(world_to_tile(position[0]), world_to_tile(position[1])));
            }
            let mut local = default_position();
            ok &= read_field(&mut local[0] as *mut u32 as u32, 4, file);
            ok &= read_field(&mut local[1] as *mut u32 as u32, 4, file);
            if ok && local != default_position() {
                self.write(unit_offsets::BUILDING_TILE, map_tile(local[0] as i32, local[1] as i32));
            }
            ok &= read(unit_offsets::BUILDING_OFFSET, 4);
            ok &= read(unit_offsets::BUILDING_OFFSET + 4, 4);
            for offset in [unit_offsets::POSITION_B, unit_offsets::POSITION_B + 4, unit_offsets::POSITION_B + 8] {
                ok &= read(offset, 4);
            }
            ok &= read(unit_offsets::NEED_INDEX - 1, 1);
            ok &= read(unit_offsets::NEED_INDEX, 1);
            ok &= read(unit_offsets::NON_PATH_COST, 4);
        }
        if version > 0x60 && !self.is_guest() {
            for offset in [unit_offsets::TRICK_TYPE, unit_offsets::SHOW_INFO_ID] {
                let mut present: u32 = 0;
                ok &= read_field(&mut present as *mut u32 as u32, 1, file);
                if present & 0xff == 1 {
                    ok &= read(offset, 2);
                }
            }
        }
        if version > 0x61 {
            ok &= read(unit_offsets::SHOW_FLAG_A, 1);
            ok &= read(unit_offsets::SHOW_FLAG_B, 1);
        }
        if !ok {
            return false;
        }
        unsafe { BFUNIT_LOAD.hooked()(self.0 as *const u32, file, version) }
    }

    /// `ZTUnit::addToMap` (`0x00412c70`): `BFUnit::addToMap`, then registers a show unit with its habitat, settles the unit in a
    /// tank or restarts its animation, and starts the mermaid transform on a freshly added `0x1530` unit.
    pub fn add_to_map(self) -> bool {
        let added = unsafe { BFUNIT_ADD_TO_MAP.hooked()(self.0 as *const u32) };
        if !added {
            return added;
        }
        let locked = || self.read::<u8>(unit_offsets::MBR_0X142) != 0;
        if self.slot_bool(goal_slots::IS_SHOW_UNIT) && !locked() {
            let habitat = call_slot_0(self.0, goal_slots::GET_HABITAT);
            if habitat != 0 {
                unsafe { ZTHABITAT_ADD_SHOW_UNIT.hooked()(habitat as *const u32, self.0) };
            }
        }
        let tank = call_slot_1(self.0, goal_slots::GET_TANK, 0);
        if tank == 0 {
            if !locked() {
                if self.read::<u8>(unit_offsets::MOVE_ANIM_ENABLED) != 0 {
                    call_slot_0(self.0, goal_slots::RESET_MOVE_ANIM);
                } else {
                    call_slot_0(self.0, goal_slots::RESET_IDLE_ANIM);
                }
            }
            return added;
        }
        if !locked() {
            call_slot_1(self.0, goal_slots::VALIDATE_POSITION, 1);
        }
        let loading = get_from_memory::<u8>(base() + APP_RVA + APP_LOADING_FLAG) != 0;
        if !loading && self.read::<u8>(ENTITY_FLAG_143) == 0 && !locked() {
            let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
            if entity_type != 0 && call_slot_0(entity_type, goal_slots::TYPE_ID) == MERMAID_TYPE_ID {
                self.start_mermaid_transform();
            }
        }
        added
    }

    /// `addToMap`'s tail for the `0x1530` type: clears the mixer, queues `bMermtrans` and refreshes the mixer.
    fn start_mermaid_transform(self) {
        let mixer = (self.0 + ENTITY_MIXER) as *const u32;
        unsafe { GXMIXER_CLEAR_ALL.hooked()(mixer) };
        let name = unsafe { new_owned_name(MERMAID_TRANSFORM_SET) };
        unsafe { SET_BS.hooked()(self.0 as *const u32, name.begin as *const u32, name.end as *const u32, name.capacity as *const u32, 0, 1, 2) };
        call_slot_0(self.0, goal_slots::RUN_BEHAVIOR_SET);
        unsafe { GXMIXER_UPDATE.hooked()(mixer, 0, 0) };
    }

    /// `ZTUnit::createName` (`0x004e1b58`): the type's display name, followed (for a numbered type) by the separator and the
    /// type's next unit count, handed to the virtual `setName`; a type without a name gets `BFEntity::createName`.
    pub fn create_name(self) {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if entity_type == 0 {
            return;
        }
        let string_id = call_slot_0(entity_type, goal_slots::TYPE_ID);
        let mut buffer = [0u8; 520];
        unsafe { LOAD_STRING.hooked()((base() + APP_RVA) as *const u32, string_id as *const u32, buffer.as_mut_ptr()) };
        let length = buffer.iter().position(|&byte| byte == 0).unwrap_or(buffer.len());
        let mut name = buffer[..length].to_vec();
        if c_string_bytes(base() + NO_NAME_TEXT_RVA) == name {
            unsafe { BFENTITY_CREATE_NAME.hooked()(self.0 as *const u32) };
            return;
        }
        if get_from_memory::<u8>(entity_type + TYPE_NUMBERED_NAMES) != 0 {
            let count = unsafe { GET_UNIT_COUNT.hooked()((entity_type + TYPE_UNIT_COUNTS) as *const u8, std::ptr::dangling::<i8>()) };
            name.extend(c_string_bytes(base() + COUNT_SEPARATOR_RVA));
            let number = VanillaString::rvo_target();
            unsafe { GET_NUMBER_TEXT.hooked()(number.as_ptr(), count as u32, 0) };
            name.extend(number.bytes());
        }
        let name = VanillaString::from_bytes(&name);
        call_slot_1(self.0, goal_slots::SET_NAME, name.as_ptr() as u32);
    }

    /// Runs the `vector<GXMixerLink>` destructor over each element in `[begin, end)`.
    fn destroy_mixer_links(begin: u32, end: u32) {
        let mut element = begin;
        while element != end {
            unsafe { GXMIXERLINK_VECTOR_DESTRUCTOR.hooked()(element as *const u32) };
            element = element.wrapping_add(mixer_queue::ELEMENT_BYTES);
        }
    }

    /// `ZTUnit::initAfterLoad` (`0x005951b3`): a unit holding a building-use goal (`0x68`-`0x6a`) that the virtual
    /// `validateBuilding` does not confirm is shown and has its mixer queue emptied (every block freed through vanilla's
    /// `operator_delete`) and stops everything; then the tank position is validated and a move animation restarted.
    /// Always succeeds.
    pub fn init_after_load(self, version: u32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = live_support::intercept::init_after_load(self.0, version) {
            return answer;
        }
        let _ = version;
        if is_building_use_goal(self.read(unit_offsets::GOAL_ID)) {
            let mut confirmed: u8 = 0;
            if call_slot_2_returning(self.0, goal_slots::VALIDATE_BUILDING, &mut confirmed as *mut u8 as u32, 0) == 0 {
                self.empty_mixer_queue();
            }
        }
        call_slot_0(self.0, goal_slots::VALIDATE_TANK_POSITION);
        if self.read::<u8>(unit_offsets::MOVE_ANIM_ENABLED) != 0 {
            call_slot_0(self.0, goal_slots::RESET_MOVE_ANIM);
        }
        true
    }

    fn empty_mixer_queue(self) {
        call_slot_1(self.0, goal_slots::SET_VISIBLE, 1);
        MsvcDeque::new(self.0 + mixer_queue::DEQUE_START, mixer_queue::BLOCK_BYTES).clear(Self::destroy_mixer_links);
        call_slot_3(self.0, goal_slots::STOP_EVERYTHING, 1, 1, 1);
    }

    /// `ZTUnit::removeFromMap` (`0x004277c9`).
    pub fn remove_from_map(self, keep: bool) {
        unsafe { REMOVE_THOUGHTS_BY_OBJECT.hooked()(globals().ztthoughtmgr() as *const _ as *const u32, self.0 as *const u32) };
        if (self.is_animal() || self.is_guest()) && self.read::<u8>(unit_offsets::MBR_0X142) == 0 && !keep {
            self.leave_building();
        }
        if self.slot_bool(goal_slots::IS_SHOW_UNIT) && self.read::<u8>(unit_offsets::MBR_0X142) == 0 {
            let habitat = call_slot_0(self.0, goal_slots::GET_HABITAT);
            if habitat != 0 {
                unsafe { ZTHABITAT_REMOVE_SHOW_UNIT.hooked()(habitat as *const u32, self.0 as *const u32) };
            }
        }
        if call_slot_1(self.0, goal_slots::GET_TANK, u32::from(keep)) != 0 && self.read::<u8>(unit_offsets::MBR_0X142) == 0 {
            self.write(unit_offsets::TANK_MODE, 0u32);
        }
        unsafe { BFUNIT_REMOVE_FROM_MAP.hooked()(self.0 as *const u32, keep) };
    }

    /// Removes the unit from the first building that lists it as a user (`removeFromMap`'s building scan).
    fn leave_building(self) {
        let buildings = BuildingList::named_text(b"building");
        for building in buildings.buildings() {
            if low_byte_bool(unsafe { ZTBUILDING_CHECK_USER.hooked()(building as *const c_void, self.0 as i32) }) {
                unsafe { ZTBUILDING_REMOVE_USER.hooked()(building as *const u32, self.0 as *const i32, 0) };
                break;
            }
        }
    }

    /// `ZTUnit::isDoingSomething` (`0x00420f65`).
    pub fn is_doing_something(self) -> bool {
        let goal: u32 = self.read(unit_offsets::GOAL_ID);
        self.read::<u8>(unit_offsets::BUSY) != 0 || is_performing_goal(goal) || self.slot_bool(goal_slots::IS_DOING_SHOW) || goal == 0x80 || goal == 0x81
    }

    /// `ZTUnit::getRandomOK` (`0x0043eb52`): free to wander when it is neither performing a show goal nor busy.
    pub fn get_random_ok(self) -> bool {
        !(is_performing_goal(self.read(unit_offsets::GOAL_ID)) || self.read::<u8>(unit_offsets::BUSY) != 0)
    }

    /// `ZTUnit::doBuilding` (`0x0042aa21`): ends the trick (virtual) when the unit's need is the building need.
    pub fn do_building(self) {
        #[cfg(feature = "reimplementation-tests")]
        if live_support::intercept::do_building(self.0) {
            return;
        }
        let building_need = unsafe { GET_NEED_INDEX.hooked()((base() + BUILDING_NEED_NAME_RVA) as *const i8) } as u8;
        if self.read::<u8>(unit_offsets::NEED_INDEX) == building_need {
            call_slot_0(self.0, goal_slots::END_TRICK);
        }
    }

    /// `ZTUnit::goalSet` (`0x0043e8d3`).
    pub fn goal_set(self, goal: i32, argument: u32) {
        unsafe { BFCOGGOAL_SET.hooked()((self.0 + unit_offsets::COG_GOAL) as *const u32, goal, argument as i32, 0, (base() + GOAL_SET_DATA_RVA) as *const i32, 0) };
    }

    /// `ZTUnit::abortGoal` (`0x005ad7ff`): aborts a running trick, then clears the goal (virtually).
    pub fn abort_goal(self) {
        if self.slot_bool(goal_slots::IS_DOING_SHOW) {
            call_slot_0(self.0, goal_slots::ABORT_TRICK);
        }
        call_slot_0(self.0, goal_slots::CLEAR_GOAL);
    }

    /// `ZTUnit::clearGoal` (`0x0043e6ff`).
    pub fn clear_goal(self) {
        unsafe { BFUNIT_CLEAR_GOAL.hooked()(self.0 as *const u32) };
        call_slot_2(self.0, goal_slots::SET_TARGET_TILE, 0, 1);
        call_slot_1(self.0, goal_slots::SET_BUILDING_TILE, 0);
        self.write_default_position(unit_offsets::BUILDING_OFFSET);
        self.write_default_position(unit_offsets::POSITION_B);
    }

    /// `ZTUnit::resetAI` (`0x005ae68b`): returns what vanilla leaves in `EAX` (the wander countdown it kept, or the
    /// quotient of the re-roll).
    pub fn reset_ai(self) -> u32 {
        self.remove_from_building_if_present();
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if entity_type != 0 && call_slot_0(entity_type, goal_slots::TYPE_ID) == 0x1530 {
            let countdown: u32 = self.read(unit_offsets::RESET_COUNTDOWN);
            if countdown != 0 {
                return countdown;
            }
        }
        call_slot_3(self.0, goal_slots::STOP_EVERYTHING, 1, 0, 1);
        let roll = game_rand();
        self.write(unit_offsets::RESET_COUNTDOWN, (roll % 4000 + 1000) as u32);
        (roll / 4000) as u32
    }

    /// `ZTUnit::updateGoals` (`0x00436109`).
    pub fn update_goals(self) {
        #[cfg(feature = "reimplementation-tests")]
        if live_support::intercept::update_goals(self.0) {
            return;
        }
        if game_rand() % 15 == 0 {
            let tile = self.tile();
            if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.hooked()(tile as *const u32, 1) }) {
                self.set_is_removed(1);
                unsafe { BFUNIT_UPDATE_GOALS.hooked()(self.0 as *const u32) };
                return;
            }
            self.enter_tile_building(tile);
        }
        call_slot_0(self.0 + unit_offsets::COG_GOAL, goal_slots::COG_GOAL_UPDATE);
        call_slot_0(self.0, goal_slots::UPDATE_SUB_GOALS);
    }

    /// `updateGoals`' once-in-fifteen check: a unit standing in a building's footprint that is not already using it
    /// takes the building-use goal and remembers where inside the tile it is.
    fn enter_tile_building(self, tile: u32) {
        let building: u32 = get_from_memory(tile + tile_offsets::ENTITY_SLOTS + 3 * 4);
        if building == 0 || !unsafe { entity_type_matches(building, BUILDING_TAG_RVA) } || self.read::<u32>(unit_offsets::BUILDING_STATE) == 3 {
            return;
        }
        let mut grid = [0i32; 3];
        unsafe { BFENTITY_GET_GRID_POS.hooked()(self.0 as *const u32, grid.as_mut_ptr()) };
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let size: i32 = get_from_memory(building + 0x150);
        let collides = low_byte_bool(unsafe { BFMAP_GET_COLLISION.hooked()((world + 8) as *const u32, self.0 as *const u32, building as *const u32, grid.as_ptr(), tile as *const u32) });
        if !collides || size < 4 || is_building_use_goal(self.read(unit_offsets::GOAL_ID)) {
            return;
        }
        call_slot_0(self.0, goal_slots::CLEAR_GOAL);
        call_slot_2(self.0, goal_slots::GOAL_SET, 0x69, 1);
        let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
        call_slot_1(self.0, goal_slots::SET_BUILDING_TILE, building_tile);
        let (x, y, z): (i32, i32, i32) = (get_from_memory(building + unit_offsets::POS_X), get_from_memory(building + unit_offsets::POS_Y), get_from_memory(building + unit_offsets::POS_Z));
        for (index, value) in position_in_tile(x, y, z).into_iter().enumerate() {
            self.write(unit_offsets::BUILDING_OFFSET + index as u32 * 4, value);
        }
    }

    /// `ZTUnit::validateTankPosition` (`0x00410857`): works out the tank mode (`0` land, `1`/`2` at a gate, `3` on the
    /// surface, `4` underwater) for the tank the unit is in or heading for, moves a unit on a ladder back between the
    /// ladder's base and top, and applies the mode through the virtual setters when it changed.
    pub fn validate_tank_position(self) {
        let mut mode: u32 = 0;
        let mut tank: u32 = 0;
        if self.slot_bool(unit_slots::IS_IN_TANK) {
            tank = call_slot_1(self.0, goal_slots::GET_TANK, 0);
        }
        let goal_state: u32 = self.read(unit_offsets::COG_GOAL_STATE);
        if goal_state == 0x14 || goal_state == 0x15 {
            let target: u32 = self.read(unit_offsets::TARGET_TILE);
            let habitat = call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, target);
            if habitat != 0 && low_byte_bool(call_slot_0(habitat, type_slots::HABITAT_IS_TANK)) {
                tank = habitat;
            }
        }
        if tank == 0 {
            self.write(unit_offsets::BUILDING_STATE, 2u32);
            if self.slot_bool(goal_slots::GET_ONLY_UNDERWATER) {
                mode = 4;
            }
        } else {
            mode = self.tank_mode_in(tank);
        }
        let current: u32 = self.read(unit_offsets::TANK_MODE);
        if mode != current {
            self.apply_tank_mode(mode);
        }
    }

    /// The tank-mode branch of `validateTankPosition` for a unit with a tank: the mode, after the height and ladder
    /// side effects.
    fn tank_mode_in(self, tank: u32) -> u32 {
        let tile = self.tile();
        let gate_in = unsafe { GET_GATE_TILE_IN.hooked()(tank as *const u32) } as u32;
        let gate_out = unsafe { GET_GATE_TILE_OUT.hooked()(tank as *const u32) } as u32;
        let ladder_a = get_from_memory::<i32>(tank + TANK_LADDER_A).wrapping_shl(4);
        let ladder_b = get_from_memory::<i32>(tank + TANK_LADDER_B).wrapping_shl(4);
        let goal_state: u32 = self.read(unit_offsets::COG_GOAL_STATE);
        let mut mode: u32 = 0;
        if tile == gate_in {
            if goal_state == 0x1c || goal_state == 0x1d {
                mode = 1;
            } else if self.read::<u32>(unit_offsets::TANK_MODE) == 2 || (!self.is_animal() && self.read::<i32>(unit_offsets::HEIGHT) == ladder_a) {
                mode = 2;
            }
        } else if tile == gate_out {
            mode = 1;
        }
        if mode != 1 {
            self.write(unit_offsets::BUILDING_STATE, 3u32);
        }
        match mode {
            2 => {
                call_slot_1(self.0, goal_slots::SET_HEIGHT_ABOVE_TERRAIN, ladder_a as u32);
                2
            }
            1 => {
                self.keep_on_ladder(tank, tile, gate_out);
                1
            }
            _ if ladder_b <= 0 => {
                call_slot_1(self.0, goal_slots::SET_HEIGHT_ABOVE_TERRAIN, 0);
                if self.slot_bool(goal_slots::GET_ONLY_UNDERWATER) {
                    4
                } else {
                    0
                }
            }
            _ => {
                if self.read::<i32>(unit_offsets::HEIGHT) >= ladder_b || self.type_surfaces() {
                    call_slot_1(self.0, goal_slots::SET_HEIGHT_ABOVE_TERRAIN, ladder_b as u32);
                    3
                } else {
                    4
                }
            }
        }
    }

    /// Whether the unit type's `getSurface` slot says the unit may come up; a unit without a unit type cannot.
    fn type_surfaces(self) -> bool {
        Self::type_bool(self.unit_type(), type_slots::GET_SURFACE)
    }

    /// Clamps the unit's height to the tank ladder it stands at, moving it (and dropping its target) when it was off it.
    fn keep_on_ladder(self, tank: u32, tile: u32, gate_out: u32) {
        let mut position: [i32; 3] = [self.read(unit_offsets::POS_X), self.read(unit_offsets::POS_Y), self.read(unit_offsets::POS_Z)];
        let mut base = [0i32; 3];
        let mut top = [0i32; 3];
        unsafe {
            let (base_getter, top_getter) = if tile == gate_out {
                (GET_LADDER_BASE_OUTSIDE_POS, GET_LADDER_TOP_OUTSIDE_POS)
            } else {
                (GET_LADDER_BASE_INSIDE_POS, GET_LADDER_TOP_INSIDE_POS)
            };
            let base_ptr = base_getter.hooked()(tank as *const u32, base.as_mut_ptr());
            base = [get_from_memory(base_ptr as u32), get_from_memory(base_ptr as u32 + 4), get_from_memory(base_ptr as u32 + 8)];
            let top_ptr = top_getter.hooked()(tank as *const u32, top.as_mut_ptr());
            top = [get_from_memory(top_ptr as u32), get_from_memory(top_ptr as u32 + 4), get_from_memory(top_ptr as u32 + 8)];
        }
        if position[2] > top[2] {
            position[2] = top[2];
        } else if position[2] < base[2] {
            position[2] = base[2];
        } else {
            return;
        }
        let position = glam::i32::IVec3::from_array(position);
        unsafe { BFENTITY_SET_WORLD_POS.hooked()(self.0 as *const u32, &position as *const glam::i32::IVec3) };
        call_slot_0(self.0, goal_slots::INVALIDATE_TARGET);
    }

    /// `validateTankPosition`'s tail: sets the mode virtually and restarts the move or idle animation.
    fn apply_tank_mode(self, mode: u32) {
        call_slot_1(self.0, goal_slots::SET_TANK_MODE, mode);
        if self.read::<u8>(unit_offsets::MOVE_ANIM_ENABLED) != 0 {
            if self.read::<u32>(unit_offsets::VERTICAL_DIRECTION) != 0 {
                self.write(unit_offsets::VERTICAL_DIRECTION, 0u32);
                call_slot_0(self.0, goal_slots::RESET_MOVE_ANIM);
            }
            let listener: u32 = self.read(unit_offsets::MODE_LISTENER);
            if listener != 0 {
                call_slot_0(listener, 0x18);
            }
            call_slot_0(self.0, goal_slots::RESET_MOVE_ANIM);
        } else {
            call_slot_0(self.0, goal_slots::RESET_IDLE_ANIM);
        }
    }

    /// `ZTUnit::getGate` (`0x00436954`): the gate fence on `tile`'s `direction` edge, or on the neighbouring tile's
    /// matching edge, or `0`. Vanilla reads the neighbour unchecked and faults on a map-edge tile; this answers `0`.
    pub fn get_gate(tile: u32, direction: u32) -> u32 {
        let gate_on_edge = |tile: u32, edge: u32| -> u32 {
            let fence: u32 = get_from_memory(tile + tile_offsets::EDGE_ENTITIES + (edge as i32 / 2) as u32 * 4);
            if fence == 0 || !unsafe { entity_type_matches(fence, FENCE_TAG_RVA) } {
                return 0;
            }
            let fence_type: u32 = get_from_memory(fence + unit_offsets::ENTITY_TYPE);
            let name: u32 = get_from_memory(fence_type + FENCE_TYPE_NAME);
            if name != 0 && get_from_memory::<u8>(name) == b'g' {
                fence
            } else {
                0
            }
        };
        if direction == u32::MAX {
            return 0;
        }
        let on_tile = gate_on_edge(tile, direction);
        if on_tile != 0 {
            return on_tile;
        }
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, direction) };
        if neighbour == 0 {
            return 0;
        }
        gate_on_edge(neighbour, direction.wrapping_sub(4) & 7)
    }

    fn gate(self, tile: u32, direction: u32) -> u32 {
        let _ = self;
        Self::get_gate(tile, direction)
    }

    /// `ZTUnit::intoNewTile` (`0x00436840`).
    pub fn into_new_tile(self, direction: u32) {
        #[cfg(feature = "reimplementation-tests")]
        if live_support::intercept::into_new_tile(self.0, direction) {
            return;
        }
        let tile = self.tile();
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, direction) };
        if self.slot_bool(unit_slots::GET_SWIMS) {
            let crosses_water = neighbour == 0
                || (get_from_memory::<u8>(neighbour + tile_offsets::TERRAIN_FLAGS) & tile_offsets::WATER_BITS != 0) != (get_from_memory::<u8>(tile + tile_offsets::TERRAIN_FLAGS) & tile_offsets::WATER_BITS != 0);
            if crosses_water {
                call_slot_0(self.0, goal_slots::RESET_MOVE_ANIM);
                call_slot_0(self.0, goal_slots::FORCE_UI_UPDATE);
                self.write(unit_offsets::MOVE_ANIM_DIRTY, 1u8);
            }
        }
        if let Some(second) = NodeList::path_steps(self.read(unit_offsets::PATH_LIST)).nodes().nth(1) {
            let next_tile = map_tile(get_from_memory(second + 8), get_from_memory(second + 0xc));
            let towards = unsafe { BFMAP_GET_DIRECTION.hooked()(tile as i32, next_tile as i32) } as u32;
            let gate = self.gate(tile, towards);
            if gate != 0 && !self.is_animal() {
                unsafe { ZTFENCE_OPEN_GATE.hooked()(gate as *const c_void) };
                self.write(unit_offsets::GATE, gate);
            }
        }
        let gate = self.gate(tile, direction);
        if gate != 0 {
            unsafe { ZTFENCE_CLOSE_GATE.hooked()(gate as *const c_void) };
            if gate == self.read::<u32>(unit_offsets::GATE) {
                self.write(unit_offsets::GATE, 0u32);
            }
        }
        unsafe { BFUNIT_INTO_NEW_TILE.hooked()(self.0 as *const u32, direction) };
    }
}

#[detour_mod]
mod unit_detours {
    use super::*;

    #[detour(GET_TANK_MODE)]
    unsafe extern "thiscall" fn get_tank_mode(this: *const u32) -> u32 {
        ZTUnit(this as u32).get_tank_mode()
    }

    #[detour(SET_TANK_MODE)]
    unsafe extern "thiscall" fn set_tank_mode(this: *const u32, mode: u32) {
        ZTUnit(this as u32).set_tank_mode(mode);
    }

    #[detour(NO_SHADOW_PART)]
    unsafe extern "thiscall" fn no_shadow_part(this: *const u32) -> u32 {
        ZTUnit(this as u32).no_shadow_part()
    }

    #[detour(NO_UNDER_PART)]
    unsafe extern "thiscall" fn no_under_part(this: *const u32) -> bool {
        ZTUnit(this as u32).no_under_part()
    }

    #[detour(IS_ON_PLATFORM)]
    unsafe extern "thiscall" fn is_on_platform(this: *const u32) -> bool {
        ZTUnit(this as u32).is_on_platform()
    }

    #[detour(IS_WATER_TRAVELING)]
    unsafe extern "thiscall" fn is_water_traveling(this: *const u32) -> bool {
        ZTUnit(this as u32).is_water_traveling()
    }

    #[detour(IS_IN_TANK)]
    unsafe extern "thiscall" fn is_in_tank(this: *const u32) -> u32 {
        ZTUnit(this as u32).is_in_tank()
    }

    #[detour(IS_ON_WATER)]
    unsafe extern "fastcall" fn is_on_water(this: *const u32) -> bool {
        ZTUnit(this as u32).is_on_water()
    }

    #[detour(GET_HABITAT_FROM_TILE)]
    unsafe extern "thiscall" fn get_habitat_from_tile(this: *const u32, tile: *const u32) -> u32 {
        ZTUnit(this as u32).get_habitat_from_tile(tile as u32)
    }

    #[detour(GET_HABITAT)]
    unsafe extern "thiscall" fn get_habitat(this: *const u32) -> i32 {
        ZTUnit(this as u32).get_habitat() as i32
    }

    #[detour(GET_TANK)]
    unsafe extern "thiscall" fn get_tank(this: *const u32, flag: bool) -> *const i32 {
        ZTUnit(this as u32).get_tank(flag) as *const i32
    }

    #[detour(GET_HABITAT_FROM_GATE)]
    unsafe extern "thiscall" fn get_habitat_from_gate(this: *const u32, tile: *const u32) -> i32 {
        ZTUnit(this as u32).get_habitat_from_gate(tile as u32) as i32
    }

    #[detour(GET_SWIMS)]
    unsafe extern "thiscall" fn get_swims(this: *const u32) -> bool {
        ZTUnit(this as u32).get_swims()
    }

    #[detour(GET_UNDERWATER)]
    unsafe extern "thiscall" fn get_underwater(this: *const u32) -> bool {
        ZTUnit(this as u32).get_underwater()
    }

    #[detour(GET_ONLY_UNDERWATER)]
    unsafe extern "thiscall" fn get_only_underwater(this: *const u32) -> bool {
        ZTUnit(this as u32).get_only_underwater()
    }

    #[detour(GET_DEPTH)]
    unsafe extern "thiscall" fn get_depth(this: *const u32, tile: *const u32) -> i32 {
        ZTUnit(this as u32).get_depth(tile as u32)
    }

    #[detour(GET_IDLE_ANIM)]
    unsafe extern "thiscall" fn get_idle_anim(this: *const u32, force_unit: bool) -> i32 {
        ZTUnit(this as u32).get_idle_anim(force_unit)
    }

    #[detour(GET_ICON_ANIM)]
    unsafe extern "thiscall" fn get_icon_anim(this: *const u32) -> i32 {
        ZTUnit(this as u32).get_icon_anim()
    }

    #[detour(GET_IDLE_ANIM_SYMBOL)]
    unsafe extern "thiscall" fn get_idle_anim_symbol(this: *const u32) -> *const u32 {
        ZTUnit(this as u32).get_idle_anim_symbol() as *const u32
    }

    #[detour(GET_SLOW_ANIM_SYMBOL)]
    unsafe extern "thiscall" fn get_slow_anim_symbol(this: *const u32) -> *const u32 {
        ZTUnit(this as u32).get_slow_anim_symbol() as *const u32
    }

    #[detour(GET_MEDIUM_ANIM_SYMBOL)]
    unsafe extern "thiscall" fn get_medium_anim_symbol(this: *const u32) -> *const u32 {
        ZTUnit(this as u32).get_medium_anim_symbol() as *const u32
    }

    #[detour(GET_FAST_ANIM_SYMBOL)]
    unsafe extern "thiscall" fn get_fast_anim_symbol(this: *const u32) -> *const u32 {
        ZTUnit(this as u32).get_fast_anim_symbol() as *const u32
    }

    #[detour(GET_SLOW_ANIM_SPEED)]
    unsafe extern "thiscall" fn get_slow_anim_speed(this: *const u32) -> u32 {
        ZTUnit(this as u32).get_slow_anim_speed()
    }

    #[detour(GET_MEDIUM_ANIM_SPEED)]
    unsafe extern "thiscall" fn get_medium_anim_speed(this: *const u32) -> u32 {
        ZTUnit(this as u32).get_medium_anim_speed()
    }

    #[detour(GET_FAST_ANIM_SPEED)]
    unsafe extern "thiscall" fn get_fast_anim_speed(this: *const u32) -> u32 {
        ZTUnit(this as u32).get_fast_anim_speed()
    }

    #[detour(DO_AMBIENT_ANIMS)]
    unsafe extern "thiscall" fn do_ambient_anims(this: *const u32, ambient: *const u32) {
        ZTUnit(this as u32).do_ambient_anims(ambient as u32);
    }

    #[detour(INIT_AMBIENT_ANIMS)]
    unsafe extern "thiscall" fn init_ambient_anims(this: *const u32, list: *const u32, name: *const i8) {
        ZTUnit(this as u32).init_ambient_anims(list as u32, name);
    }

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor(this: *const u32, entity_type: u32) -> *const u32 {
        ZTUnit(this as u32).construct(entity_type) as *const u32
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const u32) {
        ZTUnit(this as u32).init_status_vars();
    }

    #[detour(CLEAR_STATE_VARIABLES)]
    unsafe extern "thiscall" fn clear_state_variables(this: *const u32) {
        ZTUnit(this as u32).clear_state_variables();
    }

    #[detour(SET_IS_REMOVED)]
    unsafe extern "thiscall" fn set_is_removed(this: *const u32, flag: u8) {
        ZTUnit(this as u32).set_is_removed(flag);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        ZTUnit(this as u32).save(file as u32)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        ZTUnit(this as u32).load(file as u32, version)
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, keep: bool) {
        ZTUnit(this as u32).remove_from_map(keep);
    }

    #[detour(IS_DOING_SOMETHING)]
    unsafe extern "thiscall" fn is_doing_something(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).is_doing_something())
    }

    #[detour(GET_RANDOM_OK)]
    unsafe extern "thiscall" fn get_random_ok(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).get_random_ok())
    }

    #[detour(DO_BUILDING)]
    unsafe extern "thiscall" fn do_building(this: *const u32) {
        ZTUnit(this as u32).do_building();
    }

    #[detour(GOAL_SET)]
    unsafe extern "thiscall" fn goal_set(this: *const u32, goal: i32, argument: u32) {
        ZTUnit(this as u32).goal_set(goal, argument);
    }

    #[detour(ABORT_GOAL)]
    unsafe extern "thiscall" fn abort_goal(this: *const u32, _flag: u32) {
        ZTUnit(this as u32).abort_goal();
    }

    #[detour(CLEAR_GOAL)]
    unsafe extern "thiscall" fn clear_goal(this: *const u32) {
        ZTUnit(this as u32).clear_goal();
    }

    #[detour(RESET_AI)]
    unsafe extern "thiscall" fn reset_ai(this: *const u32) -> u32 {
        ZTUnit(this as u32).reset_ai()
    }

    #[detour(UPDATE_GOALS)]
    unsafe extern "thiscall" fn update_goals(this: *const u32) {
        ZTUnit(this as u32).update_goals();
    }

    #[detour(ADD_TO_MAP)]
    unsafe extern "thiscall" fn add_to_map(this: *const u32) -> bool {
        ZTUnit(this as u32).add_to_map()
    }

    #[detour(CREATE_NAME)]
    unsafe extern "thiscall" fn create_name(this: *const u32) {
        ZTUnit(this as u32).create_name();
    }

    #[detour(INIT_AFTER_LOAD)]
    unsafe extern "thiscall" fn init_after_load(this: *const u32, version: u32) -> bool {
        ZTUnit(this as u32).init_after_load(version)
    }

    #[detour(INTO_NEW_TILE)]
    unsafe extern "thiscall" fn into_new_tile(this: *const u32, direction: u32) {
        ZTUnit(this as u32).into_new_tile(direction);
    }

    #[detour(VALIDATE_TANK_POSITION)]
    unsafe extern "thiscall" fn validate_tank_position(this: *const u32) {
        ZTUnit(this as u32).validate_tank_position();
    }

    /// The vanilla `load`, for the saves `ZTUnit::load` leaves to it.
    pub(super) fn real_load(this: u32, file: u32, version: u32) -> bool {
        unsafe { LOAD_DETOUR.call(this as *const u32, file as *const u32, version) }
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn get_tank_mode(this: u32) -> u32 {
            unsafe { super::GET_TANK_MODE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_tank_mode(this: u32, mode: u32) {
            unsafe { super::SET_TANK_MODE_DETOUR.call(this as *const u32, mode) }
        }

        pub(crate) fn no_shadow_part(this: u32) -> u32 {
            unsafe { super::NO_SHADOW_PART_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn no_under_part(this: u32) -> bool {
            unsafe { super::NO_UNDER_PART_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_on_platform(this: u32) -> bool {
            unsafe { super::IS_ON_PLATFORM_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_water_traveling(this: u32) -> bool {
            unsafe { super::IS_WATER_TRAVELING_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_in_tank(this: u32) -> u32 {
            unsafe { super::IS_IN_TANK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_on_water(this: u32) -> bool {
            unsafe { super::IS_ON_WATER_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_habitat_from_tile(this: u32, tile: u32) -> u32 {
            unsafe { super::GET_HABITAT_FROM_TILE_DETOUR.call(this as *const u32, tile as *const u32) }
        }

        pub(crate) fn get_habitat(this: u32) -> u32 {
            unsafe { super::GET_HABITAT_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_tank(this: u32, flag: bool) -> u32 {
            unsafe { super::GET_TANK_DETOUR.call(this as *const u32, flag) as u32 }
        }

        pub(crate) fn get_habitat_from_gate(this: u32, tile: u32) -> u32 {
            unsafe { super::GET_HABITAT_FROM_GATE_DETOUR.call(this as *const u32, tile as *const u32) as u32 }
        }

        pub(crate) fn get_swims(this: u32) -> bool {
            unsafe { super::GET_SWIMS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_underwater(this: u32) -> bool {
            unsafe { super::GET_UNDERWATER_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_only_underwater(this: u32) -> bool {
            unsafe { super::GET_ONLY_UNDERWATER_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_depth(this: u32, tile: u32) -> i32 {
            unsafe { super::GET_DEPTH_DETOUR.call(this as *const u32, tile as *const u32) }
        }

        pub(crate) fn get_idle_anim(this: u32, force_unit: bool) -> i32 {
            unsafe { super::GET_IDLE_ANIM_DETOUR.call(this as *const u32, force_unit) }
        }

        pub(crate) fn get_icon_anim(this: u32) -> i32 {
            unsafe { super::GET_ICON_ANIM_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_idle_anim_symbol(this: u32) -> u32 {
            unsafe { super::GET_IDLE_ANIM_SYMBOL_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_slow_anim_symbol(this: u32) -> u32 {
            unsafe { super::GET_SLOW_ANIM_SYMBOL_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_medium_anim_symbol(this: u32) -> u32 {
            unsafe { super::GET_MEDIUM_ANIM_SYMBOL_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_fast_anim_symbol(this: u32) -> u32 {
            unsafe { super::GET_FAST_ANIM_SYMBOL_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_slow_anim_speed(this: u32) -> u32 {
            unsafe { super::GET_SLOW_ANIM_SPEED_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_medium_anim_speed(this: u32) -> u32 {
            unsafe { super::GET_MEDIUM_ANIM_SPEED_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_fast_anim_speed(this: u32) -> u32 {
            unsafe { super::GET_FAST_ANIM_SPEED_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_ambient_anims(this: u32, ambient: u32) {
            unsafe { super::DO_AMBIENT_ANIMS_DETOUR.call(this as *const u32, ambient as *const u32) }
        }

        pub(crate) fn init_ambient_anims(this: u32, list: u32, name: *const i8) {
            unsafe { super::INIT_AMBIENT_ANIMS_DETOUR.call(this as *const u32, list as *const u32, name) }
        }

        pub(crate) fn constructor(this: u32, entity_type: u32) -> u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this as *const u32, entity_type) as u32 }
        }

        pub(crate) fn init_status_vars(this: u32) {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn clear_state_variables(this: u32) {
            unsafe { super::CLEAR_STATE_VARIABLES_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_is_removed(this: u32, flag: u8) {
            unsafe { super::SET_IS_REMOVED_DETOUR.call(this as *const u32, flag) }
        }

        pub(crate) fn save(this: u32, file: u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file as *const u32) }
        }

        pub(crate) fn load(this: u32, file: u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file as *const u32, version) }
        }

        pub(crate) fn remove_from_map(this: u32, keep: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const u32, keep) }
        }

        pub(crate) fn is_doing_something(this: u32) -> u32 {
            unsafe { super::IS_DOING_SOMETHING_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_random_ok(this: u32) -> u32 {
            unsafe { super::GET_RANDOM_OK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_building(this: u32) {
            unsafe { super::DO_BUILDING_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn goal_set(this: u32, goal: i32, argument: u32) {
            unsafe { super::GOAL_SET_DETOUR.call(this as *const u32, goal, argument) }
        }

        pub(crate) fn abort_goal(this: u32) {
            unsafe { super::ABORT_GOAL_DETOUR.call(this as *const u32, 0) }
        }

        pub(crate) fn clear_goal(this: u32) {
            unsafe { super::CLEAR_GOAL_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn reset_ai(this: u32) -> u32 {
            unsafe { super::RESET_AI_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn update_goals(this: u32) {
            unsafe { super::UPDATE_GOALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn add_to_map(this: u32) -> bool {
            unsafe { super::ADD_TO_MAP_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn create_name(this: u32) {
            unsafe { super::CREATE_NAME_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn init_after_load(this: u32, version: u32) -> bool {
            unsafe { super::INIT_AFTER_LOAD_DETOUR.call(this as *const u32, version) }
        }

        pub(crate) fn into_new_tile(this: u32, direction: u32) {
            unsafe { super::INTO_NEW_TILE_DETOUR.call(this as *const u32, direction) }
        }

        pub(crate) fn validate_tank_position(this: u32) {
            unsafe { super::VALIDATE_TANK_POSITION_DETOUR.call(this as *const u32) }
        }
    }
}

/// Installs `ZTUnit`'s detours. Called once at DLL load (via `lib.rs`) and before the `ZTUNIT_*` tests in
/// `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { unit_detours::init_detours() } {
        error!("Failed to initialise ztunit detours: {e:?}");
    }
    movement::init();
    satisfiers::init();
    buildings::init();
    shows::init();
    messages::init();
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::{
        standalone::OPERATOR_NEW,
        ztunit::DESTRUCTOR_1 as ZTUNIT_DESTRUCTOR_1,
    };

    use super::{ZTUnit, UNIT_SIZE};

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        let mut status = super::unit_detours::status();
        status.extend(super::movement::status());
        status.extend(super::satisfiers::status());
        status.extend(super::buildings::status());
        status.extend(super::shows::status());
        status.extend(super::messages::status());
        status
    }

    pub(crate) use super::unit_detours::test_real as real;
    pub(crate) use super::movement::test_real as real_movement;
    pub(crate) use super::satisfiers::test_real as real_satisfiers;
    pub(crate) use super::buildings::test_real as real_buildings;
    pub(crate) use super::shows::test_real as real_shows;
    pub(crate) use super::messages::test_real as real_messages;

    /// Test hook: while set, `ZTUnit::is_on_water` answers from it (when it returns `Some`) instead of reading the tile,
    /// so callers that reach it through the Rust method or the hooked address see the same scripted answer.
    pub(crate) mod on_water_override {
        use std::sync::Mutex;

        pub(crate) static HOOK: Mutex<Option<fn(u32) -> Option<bool>>> = Mutex::new(None);

        pub(in crate::ztunit) fn intercept(unit: u32) -> Option<bool> {
            let hook = *HOOK.lock().unwrap();
            hook.and_then(|hook| hook(unit))
        }
    }

    /// Test hooks that stand in for a unit method a live test wants recorded instead of run: each returns `true` when
    /// it handled the call. A hook installed once stays set; it decides per call whether to intercept.
    pub(crate) mod intercept {
        use std::sync::Mutex;

        pub(crate) static UPDATE_GOALS: Mutex<Option<fn(u32) -> bool>> = Mutex::new(None);
        pub(crate) static DO_BUILDING: Mutex<Option<fn(u32) -> bool>> = Mutex::new(None);
        pub(crate) static INTO_NEW_TILE: Mutex<Option<fn(u32, u32) -> bool>> = Mutex::new(None);
        pub(crate) static UPDATE_BUILDING_TILES: Mutex<Option<fn(u32, bool) -> Option<bool>>> = Mutex::new(None);
        pub(crate) static HAS_BUILDING_USE_GOAL: Mutex<Option<fn(u32) -> Option<bool>>> = Mutex::new(None);
        pub(crate) static INIT_AFTER_LOAD: Mutex<Option<fn(u32, u32) -> Option<bool>>> = Mutex::new(None);

        pub(crate) static IS_NEED_OR_FUN_SATISFIER: Mutex<Option<fn(u32, u32, *const i32) -> Option<bool>>> = Mutex::new(None);
        pub(crate) static IS_FUN_SATISFIER: Mutex<Option<fn(u32, *const i32) -> Option<bool>>> = Mutex::new(None);
        pub(crate) static IN_FRIENDLY_CHASE: Mutex<Option<fn(u32) -> Option<bool>>> = Mutex::new(None);
        pub(crate) static FIND_PREDATOR: Mutex<Option<fn(u32, bool) -> Option<i32>>> = Mutex::new(None);
        /// `(unit, tile, name, out, range, filter)`.
        pub(crate) static FIND_ANY_SATISFIER: Mutex<Option<fn(u32, u32, u32, u32, i32, i32) -> Option<bool>>> = Mutex::new(None);
        pub(crate) static FIND_NEAREST_SATISFIER: Mutex<Option<fn(u32, u32, u32, u32, i32, i32) -> Option<bool>>> = Mutex::new(None);
        /// Stands in for `doRandomWalk`'s two collaborators: `false` is `BFAIMgr::fRandomWalk`, `true` `BFUnit::goToDest`.
        pub(crate) static RANDOM_WALK: Mutex<Option<fn(u32, bool) -> Option<bool>>> = Mutex::new(None);

        pub(in crate::ztunit) fn update_building_tiles(unit: u32, abort: bool) -> Option<bool> {
            let hook = *UPDATE_BUILDING_TILES.lock().unwrap();
            hook.and_then(|hook| hook(unit, abort))
        }

        pub(in crate::ztunit) fn has_building_use_goal(unit: u32) -> Option<bool> {
            let hook = *HAS_BUILDING_USE_GOAL.lock().unwrap();
            hook.and_then(|hook| hook(unit))
        }

        pub(in crate::ztunit) fn random_walk(unit: u32, go_to_dest: bool) -> Option<bool> {
            let hook = *RANDOM_WALK.lock().unwrap();
            hook.and_then(|hook| hook(unit, go_to_dest))
        }

        pub(in crate::ztunit) fn is_need_or_fun_satisfier(unit: u32, quadrants: u32, position: *const i32) -> Option<bool> {
            let hook = *IS_NEED_OR_FUN_SATISFIER.lock().unwrap();
            hook.and_then(|hook| hook(unit, quadrants, position))
        }

        pub(in crate::ztunit) fn is_fun_satisfier(quadrants: u32, position: *const i32) -> Option<bool> {
            let hook = *IS_FUN_SATISFIER.lock().unwrap();
            hook.and_then(|hook| hook(quadrants, position))
        }

        pub(in crate::ztunit) fn find_any_satisfier(unit: u32, tile: u32, name: u32, out: u32, range: i32, filter: i32) -> Option<bool> {
            let hook = *FIND_ANY_SATISFIER.lock().unwrap();
            hook.and_then(|hook| hook(unit, tile, name, out, range, filter))
        }

        pub(in crate::ztunit) fn find_nearest_satisfier(unit: u32, tile: u32, name: u32, out: u32, range: i32, filter: i32) -> Option<bool> {
            let hook = *FIND_NEAREST_SATISFIER.lock().unwrap();
            hook.and_then(|hook| hook(unit, tile, name, out, range, filter))
        }

        pub(in crate::ztunit) fn find_predator(unit: u32, flag: bool) -> Option<i32> {
            let hook = *FIND_PREDATOR.lock().unwrap();
            hook.and_then(|hook| hook(unit, flag))
        }

        pub(in crate::ztunit) fn in_friendly_chase(unit: u32) -> Option<bool> {
            let hook = *IN_FRIENDLY_CHASE.lock().unwrap();
            hook.and_then(|hook| hook(unit))
        }

        pub(in crate::ztunit) fn init_after_load(unit: u32, version: u32) -> Option<bool> {
            let hook = *INIT_AFTER_LOAD.lock().unwrap();
            hook.and_then(|hook| hook(unit, version))
        }

        pub(in crate::ztunit) fn update_goals(unit: u32) -> bool {
            let hook = *UPDATE_GOALS.lock().unwrap();
            hook.is_some_and(|hook| hook(unit))
        }

        pub(in crate::ztunit) fn do_building(unit: u32) -> bool {
            let hook = *DO_BUILDING.lock().unwrap();
            hook.is_some_and(|hook| hook(unit))
        }

        pub(in crate::ztunit) fn into_new_tile(unit: u32, direction: u32) -> bool {
            let hook = *INTO_NEW_TILE.lock().unwrap();
            hook.is_some_and(|hook| hook(unit, direction))
        }
    }

    /// A zeroed allocation for a unit built with the port's constructor. It is guest-sized rather than unit-sized: a unit
    /// over a guest type makes the base methods write guest fields (`updateBuildingTiles` sets a byte at `+0x33e`), which
    /// would run past a `0x260`-byte block.
    pub fn allocate_unit_memory() -> u32 {
        let size = UNIT_SIZE.max(crate::ztguest::GUEST_SIZE);
        let memory = unsafe { OPERATOR_NEW.original()(size) } as u32;
        unsafe { std::ptr::write_bytes(memory as *mut u8, 0, size as usize) };
        memory
    }

    /// A `ZTUnit` built by the real vanilla constructor over `unit_type` (any `ZTUnitType` pointer). Its vtable is the
    /// base `ZTUnit` one.
    pub fn build_standalone_unit(unit_type: u32) -> ZTUnit {
        let memory = allocate_unit_memory();
        real::constructor(memory, unit_type);
        ZTUnit(memory)
    }

    /// Tears a unit down through the real scalar-deleting destructor, which also frees the memory.
    pub fn destroy_standalone_unit(unit: ZTUnit) {
        unsafe { ZTUNIT_DESTRUCTOR_1.original()(unit.0 as *const u32, 1) };
    }
}

#[cfg(test)]
mod tests {
    use super::{unit_offsets::*, UNIT_SIZE};

    #[test]
    fn own_fields_fit_before_the_staff_part() {
        assert!(TANK_MODE + 4 <= UNIT_SIZE);
        assert!(SHOW_INFO_ID + 2 <= TANK_MODE);
        assert!(IN_SHOW < SHOW_INFO_ID);
    }
}
