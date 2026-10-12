use getset::Getters;
use glam::i32::IVec3;
use openzt_detour::generated::{
    bfcategory::GET_VALUE,
    bfmap::{ADJUST_CELL, PAINT_CELL, SET_SELECTION, VALIDATE_TILES},
    bfuimgr::{DISPLAY_MESSAGE_0, DISPLAY_MESSAGE_1},
    bfentity::SET_WORLD_POS,
    bfworldmgr::{ADD_TRANSIENT, FIND_TYPES, GET_ENTITY, GET_ENTITY_REVERSE},
    zttankfilter::{IS_STILL_ATTACHED, REMOVE_BUBBLES},
    zttankwall::SET_WALL_HEIGHT,
    ztanimal::BOX as ZTANIMAL_BOX,
    ztmapview::{PREPARE_TO_DELETE_ENTITY, REMOVE_ENTITY_FROM_MAP},
    ztui_general::GET_MAPVIEW,
    ztworldmgr::{BOX_UP_AND_FIND_APLACE_FOR_ANIMAL, FIND_APLACE_FOR_STAFF_MEMBER},
    gxmixer::UPDATE as GXMIXER_UPDATE,
    msvc_std_basic_string::COMPARE as BASIC_STRING_ASSIGN_RANGE,
    msvc_std_vectorbyte::VECTORBYTE,
    standalone::SET_NEEDS_ORDER_RECALCULATE,
    ztui_tankmodify::FETCH_VALUES_FROM_TANK,
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfmap::GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0,
    bftile::SNAP_TO_EDGE,
    poolalloc::DEALLOCATE as POOLALLOC_DEALLOCATE,
    zttankexhibit::{
        CONFIRM_SELL_OFF, REMOVE_ILLEGAL_ENTITIES, ADD_TANK_WALL, CLEAR_WALL_VECTOR, DO_DEPTH_SUITABILITY, GET_ILLEGAL_ENTITIES, IS_ILLEGAL_ENTITY, LEVEL_ALL_TILES, RECLAIM_TILES, RESTORE_OWNED_TRANSIENTS, UPDATE_TANK_WALL_INFO, VALIDATE_FILTER_POSITION,
        ADD_RANDOM_SPARKLE, ADD_WATER_RIPPLE, ADD_WATER_RIPPLES, REMOVE_ALL_SPARKLES, REMOVE_ALL_SPARKLES_BY_ID, CAN_BE_IN_TANK, CHECK_ENTITIES_FOR_DRAIN, CHECK_ENTITIES_FOR_FILL, CLEAN_WATER_AMOUNT, DRAIN, FILL, GET_ALL_ENTITIES, REMOVE_OWNED_TRANSIENTS, REMOVE_WATER_RIPPLES, SET_BASE_LEVEL, SET_BASE_TERRAIN_TYPE, SET_SALINITY, SET_WATER_CLEAN, GET_COST_TO_CHANGE_HEIGHT, GET_LADDER_BASE_INSIDE_POS, GET_LADDER_BASE_OUTSIDE_POS, GET_LADDER_TOP_INSIDE_POS, GET_LADDER_TOP_OUTSIDE_POS, GET_MIN_DEPTH, GET_PLATFORM_POS, GET_TANK_HEIGHT,
        GET_WATER_LEVEL, GET_WATER_PURITY, IS_EXTREMELY_MURKY_WATER, IS_PURE_WATER, IS_RIGHT_SALINITY, REMOVE_DEAD_SPARKLES, SET_TANK_HEIGHT, SET_WATER_LEVEL, SET_WATER_PURITY, UPDATE, UPDATE_ADJUSTMENT_COSTS,
    },
};
use openzt_detour_macro::detour_mod;
use std::collections::BTreeSet;
use tracing::error;

use super::habitat::ZTHabitat;
use super::support::{call_entity_type_create, call_vtable_slot_noargs, call_vtable_slot_noargs_ret_bool, RVA_STAFF_TYPE_CHECK_ARG, call_vtable_slot_with_u8, call_vtable_slot_with_u8_u8, free_event_vector_buffer, next_game_random, rb_set_insert, walk_tile_list, RVA_APP_INIT_SUCCESS_BASE, RVA_GLOBAL_BFUIMGR, RVA_TANK_WALL_TYPE_CHECK_ARG};
use crate::string_registry::load_string_by_id;
use crate::util::ZTString;
use crate::ztmegatilemgr::entity_type_matches;
use crate::globals::{get_module_base, globals};
use crate::util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory, LiveMemory};
use crate::vanilla_vector::{push_pair, push_word};
use crate::write_live;

/// `ZTTankExhibit`, the single-inheritance subclass of [`ZTHabitat`] real tank/show exhibits are
/// instantiated as (allocated via `operator_new(0x1e8)` by `ZTTankExhibit::cls_0x40183b`, confirmed at
/// the `.asm` level; see `ZTHabitat`'s own doc comment for the allocator bifurcation).
///
/// embeds the base class at offset 0 (matching real C++ single inheritance layout), so every
/// `ZTHabitat` field/method is reachable through it. Only ever construct this from a pointer already
/// confirmed via `ZTHabitat::is_tank()` - reading it from a real, plain `ZTHabitat` (0x178 bytes) would
/// over-read past that object's actual allocation, exactly the bug this struct split fixes.
///
/// `0x18c` onward was previously modeled as opaque padding; `ZTTankExhibit_isRightSalinity.c`/
/// `_update.c`/`_addRandomSparkle.c` (the last confirmed via Ghidra MCP identification of the former
/// `FUN_00496382`) prove almost all of it is real fields (sparkle-effect bookkeeping plus a
/// current/pending water-type pair), named below with the confidence each decompile actually supports -
/// several names are inferred from call shape, not confirmed by any setter/getter method name, and are
/// flagged as such.
#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTTankExhibit {
    pub habitat: ZTHabitat,   // 0x000 - 0x178
    wall_ids_begin: u32,  // 0x178 // Begin of a vanilla `std::vector<i32>` of this tank's wall entity ids (`updateAdjustmentCosts` walks it; `addTankWall`/`clearWallVector` maintain it).
    wall_ids_end: u32,    // 0x17c
    wall_ids_cap_end: u32, // 0x180
    tank_height: u32,     // 0x184 // Actual structural tank height (ZTTankExhibit::getTankHeight/setTankHeight); not the field checkTankPlacement compares against, see water_level.
    water_level: u32,     // 0x188 // Current water level (ZTTankExhibit::getWaterLevel); this is what checkTankPlacement's height comparisons actually use. Also `addRandomSparkle`'s "tank has any water at all" gate (`!= 0`).
    current_water_type: i32, // 0x18c // Inferred, not confirmed by any method name: `isRightSalinity` flips its salinity comparison when this is `0` (read as a fresh/salt discriminant), and `update` overwrites it from `pending_water_type` exactly when the tank finishes draining (`water_level == 0`), immediately following with `updateAdjustmentCosts`+`fill` - i.e. this is the water type the tank is *currently* filled/configured for, applied lazily on drain rather than immediately on `setBaseTerrainType`.
    pending_water_type: i32, // 0x190 // Inferred: the requested water type staged by (the un-ported) `ZTTankExhibit::setBaseTerrainType`, compared against `current_water_type` and swapped in once the tank fully drains - see `current_water_type`'s own note.
    pad_tank2b: [u8; 0x4], // ----------------------- padding: 4 bytes (0x194-0x198, not read by isRightSalinity/update/addRandomSparkle)
    is_filled: bool,      // 0x198 // Set true by ZTTankExhibit::fill(), false by ZTTankExhibit::drain(). Confirmed against ZTTankExhibit_fill.c/ZTTankExhibit_drain.c/ZTMapView_checkTankPlacement.c (field_0x198, checked as `this_00->field_0x198 == '\0'`) - not flipped between platforms.
    fill_suppressed: u8,  // 0x199 // `fill` returns immediately while this is non-zero; nothing in the decompile corpus writes it.
    pad_tank3a: [u8; 2], // ----------------------- padding: 2 bytes (0x19a-0x19c, alignment)
    cost_rate_a: f32,     // 0x19c // Per-height-step wall cost: `updateAdjustmentCosts` sums each wall type's `getPurchaseCost` (vtable `+0xa4`) and divides by `DAT_0063af14`; `getCostToChangeHeight` scales the height delta by it.
    cost_rate_b: f32,     // 0x1a0 // Per-height-step water cost: `updateAdjustmentCosts` sets it to `owned_tile_count * (fresh|salt cost)`; `getCostToChangeHeight` adds `delta * this` only while `is_filled`.
    cost_rate_c: f32,     // 0x1a4 // `updateAdjustmentCosts` sets it to `owned_tile_count * ZTWorldMgr[+0x64]`; read only by `tankmodify::fetchValuesFromTank`, which shows it as a money amount in the tank-modify dialog.
    water_purity: i32,     // 0x1a8 // Inferred: current purity level - `update`'s countdown-expiry branch only fires while `> 0` and calls `setWaterPurity(this, water_purity - 1)` on expiry, matching the already-generated `zttankexhibit::SET_WATER_PURITY` setter's own shape (not yet decompiled to directly confirm it writes this exact offset).
    water_purity_timer: i32, // 0x1ac // Inferred: ticks down by `elapsed` each `update()` call; reloaded from `DAT_0063af04` and triggers the `water_purity` decrement above when it drops to `<= 0`.
    attached_filter_id: i32, // 0x1b0 // `ZTTankFilter` entity id attached to this tank, `-1` when none; `validateFilterPosition` resets it to `-1` when the entity is gone or `ZTTankFilter::isStillAttached` fails.
    ripple_entity_ids_begin: u32, // 0x1b4 // Begin of a vanilla `std::vector<u32>` of the entity ids of this tank's water-ripple transients (`addWaterRipple` appends each ripple's `+0x124` id; `removeWaterRipples` retires them).
    ripple_entity_ids_end: u32,   // 0x1b8
    ripple_entity_ids_cap_end: u32, // 0x1bc
    ripple_entity_type: u32, // 0x1c0 // Cached entity-type pointer for the ripple type (`BFWorldMgr::findTypes`, looked up once; `0` while the lookup has failed, so a failed lookup retries on the next call). `addWaterRipple` spawns through the type's vtable slot `+0x24`.
    sparkle_entities_begin: u32, // 0x1c4 // Inferred: begin pointer of a real vanilla `std::vector<std::pair<i32,BFEntity*>>` - `addRandomSparkle`'s own growth path (`cls_0x43f6a9::meth_0x43f6a9` when `end == cap_end`, else in-place `*end = 0; end[1] = entity`) is the same begin/end/cap_end vector-growth shape used elsewhere in this codebase (e.g. `ZTHabitat::viewing_areas_begin`), tracking each spawned sparkle entity this tank owns.
    sparkle_entities_end: u32,  // 0x1c8
    sparkle_entities_cap_end: u32, // 0x1cc
    sparkle_entity_ids_begin: u32, // 0x1d0 // Inferred: begin pointer of a parallel `std::vector<u32>` (`msvc_std::vector_pod<>::_Insert_n` growth path) storing each spawned sparkle's own `+0x124` id field, one entry per `sparkle_entities_*` entry above.
    sparkle_entity_ids_end: u32,   // 0x1d4
    sparkle_entity_ids_cap_end: u32, // 0x1d8
    salt_water_sparkle_type: u32, // 0x1dc // Cached entity-type pointer for the salt-water sparkle type (`BFWorldMgr::findTypes`, looked up once; `0` while the lookup has failed), spawned through the type's vtable slot `+0x24` when `addRandomSparkle` runs on a non-freshwater tank.
    fresh_water_sparkle_type: u32, // 0x1e0 // Same shape as `salt_water_sparkle_type`, for the freshwater sparkle type `addRandomSparkle` spawns when `current_water_type == 0`.
    sparkle_spawn_timer: i32, // 0x1e4 // Inferred: ticks down by `elapsed` each `update()` call; on reaching `< 0`, `update` calls `addRandomSparkle` and reloads this from `DAT_006390b8 / owned_tile_count`.
    #[getset(skip)]
    pub _live: LiveMemory, // trailing ZST - see LiveMemory's own docs; required by this struct's real methods reading live game memory through &self (is_right_salinity, update).
}

const _: () = assert!(std::mem::size_of::<ZTTankExhibit>() == 0x1e8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, tank_height) == 0x184);
const TANK_HEIGHT_OFFSET: u32 = 0x184;
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_level) == 0x188);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, current_water_type) == 0x18c);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, pending_water_type) == 0x190);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, is_filled) == 0x198);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, fill_suppressed) == 0x199);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, cost_rate_a) == 0x19c);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, cost_rate_b) == 0x1a0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, cost_rate_c) == 0x1a4);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, wall_ids_begin) == 0x178);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, wall_ids_end) == 0x17c);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity) == 0x1a8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, attached_filter_id) == 0x1b0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity_timer) == 0x1ac);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, ripple_entity_ids_begin) == 0x1b4);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, ripple_entity_type) == 0x1c0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entities_begin) == 0x1c4);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entity_ids_begin) == 0x1d0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, salt_water_sparkle_type) == 0x1dc);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, fresh_water_sparkle_type) == 0x1e0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_spawn_timer) == 0x1e4);

impl std::ops::Deref for ZTTankExhibit {
    type Target = ZTHabitat;
    fn deref(&self) -> &ZTHabitat {
        &self.habitat
    }
}

/// `ZTAnimalType`'s embedded water-suitability category map head, at `animal_type + 0x2d8`
/// (`LEA ESI,[EAX+0x2d8]` in `ZTTankExhibit_isRightSalinity.asm`; the macOS build's own copy of the
/// function reads the equivalent `+0x2a8`). Kept a raw offset with a doc comment rather than a named
/// `ZTAnimalType` field, same precedent as the sibling `+0x2cc` site in
/// [`ZTHabitat::additional_scenery_suitability_change`]: it sits inside that struct's `pad11` region
/// (`openzt/src/bfentitytype/units.rs`) and is a *different* category map from `+0x2cc`'s.
const ANIMAL_TYPE_WATER_SUITABILITY_MAP_OFFSET: u32 = 0x2d8;

/// The two water-suitability category ids `isRightSalinity` compares (`PUSH 0xa` then `PUSH 0x9` at the
/// call sites). Raw ids only - no decompile names them, and which of them "means" fresh vs salt water is
/// unconfirmed either way, so don't guess.
const SALINITY_CATEGORY_10: i32 = 10;
const SALINITY_CATEGORY_9: i32 = 9;

/// `DAT_006390ac` - the settings global real vanilla's rise branch adds to `tank_height` to form the
/// rise target (`water_level` rises one level per tick while below `tank_height + this`). Data global,
/// so resolved at runtime as `zoo.exe` base + RVA; read live, never hardcoded (`0` at rest).
const WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA: u32 = 0x0023_90ac; // DAT_006390ac - 0 at rest

/// `DAT_0063af04` - the purity-countdown reload: `update` resets `water_purity_timer` to this each
/// time it drops below 1, just before dropping `water_purity` by one (data global, `1000` at rest).
const WATER_PURITY_TIMER_RELOAD_RVA: u32 = 0x0023_af04; // DAT_0063af04 - 1000 at rest

/// `DAT_006390b8` - the sparkle-spawn interval numerator: `update` reloads `sparkle_spawn_timer` to
/// this divided by the owned-tile count each time it drops below 0 (data global, `40000` at rest).
const SPARKLE_SPAWN_INTERVAL_NUMERATOR_RVA: u32 = 0x0023_90b8; // DAT_006390b8 - 40000 at rest

/// `DAT_006390a4` - `tanks.cfg`'s `murkyWaterPurity`: `isPureWater` is `water_purity >=` this (signed). Data
/// global, resolved at runtime as `zoo.exe` base + RVA.
const MURKY_WATER_PURITY_RVA: u32 = 0x0023_90a4;

/// `DAT_006390b0` - the threshold `isExtremelyMurkyWater` compares against (`water_purity <=` this, signed).
/// `loadProperties` never writes this address (its `extremelyMurkyWaterPurity` read lands in `DAT_0063af0c`
/// together with `veryMurkyWaterPurity`), so it keeps its static initial value.
const EXTREMELY_MURKY_WATER_PURITY_RVA: u32 = 0x0023_90b0;

/// `tanks.cfg` `[cost]` settings `updateAdjustmentCosts` reads (data globals, `zoo.exe` base + RVA):
/// `DAT_0063af10` `saltWater` and `DAT_0063af40` `freshWater` (`f32`, cost per owned tile per height
/// step), `DAT_0063af14` `wallHeightPriceDivisor` (`i32`).
const SALT_WATER_COST_RVA: u32 = 0x0023_af10;
const FRESH_WATER_COST_RVA: u32 = 0x0023_af40;
const WALL_HEIGHT_PRICE_DIVISOR_RVA: u32 = 0x0023_af14;

/// `DAT_0063af20` `initialHeight` / `DAT_0063af1c` `initialHeightShow` (`i32`): the height up to which
/// `getCostToChangeHeight` charges nothing, for a regular / show tank.
const INITIAL_HEIGHT_RVA: u32 = 0x0023_af20;
const INITIAL_HEIGHT_SHOW_RVA: u32 = 0x0023_af1c;

/// `ZTWorldMgr`'s `f32` at `+0x64`, which `updateAdjustmentCosts` multiplies by the owned-tile count into
/// [`ZTTankExhibit::cost_rate_c`]. `ZTWorldMgr` leaves this inside padding, so it is read by offset.
const WORLD_MGR_TILE_COST_OFFSET: u32 = 0x64;

/// Tank wall entity type vtable slot (`+0xa4`) `updateAdjustmentCosts` calls on each wall's type object:
/// `getPurchaseCost() -> f32` (returned in `ST0`).
const WALL_TYPE_GET_PURCHASE_COST_VTABLE_OFFSET: u32 = 0xa4;

/// `BFTile`'s world-grid position (`IVec3`), as every tile pointer in this module reads it.
const TILE_POS_OFFSET: u32 = 0x34;

/// `BFTile`'s flag byte; bit `0x20` marks a tile in a tank's water footprint (the ladder-base-outside
/// position raises its height by the tank's water level when the out tile has it).
const TILE_FLAGS_OFFSET: u32 = 0x85;
const TILE_FLAG_TANK_WATER: u8 = 0x20;

/// World units one water level / one tank-height step adds to a position's `z` (`SHL 4`).
const WORLD_Z_PER_LEVEL: i32 = 16;

/// Tile-local centre point the platform and gate-height lookups convert from.
const TILE_LOCAL_CENTER: IVec3 = IVec3::new(32, 32, 0);

/// Size class of a vanilla `std::set<BFEntity*>` header/node (`0x14` bytes; see `support::rb_set_insert`).
const ENTITY_SET_NODE_SIZE: u32 = 0x14;

/// Entity vtable slot (`+0xe8`) `getMinDepth` calls on each entity: `getDepth(tile)`.
const ENTITY_GET_DEPTH_VTABLE_OFFSET: u32 = 0xe8;

/// Value `getMinDepth`'s running minimum starts from.
const MIN_DEPTH_INITIAL: i32 = 2;

/// `ZTApp`'s `appInitSuccess` byte (`+0x440`) off [`RVA_APP_INIT_SUCCESS_BASE`]: non-zero outside loading,
/// where `fill`/`drain`/`setSalinity` also move the water level.
const APP_INIT_SUCCESS_OFFSET: u32 = 0x440;

/// `DAT_0063af00` - `tanks.cfg`'s `initialWaterPurity`, which `setWaterPurity` also uses as the upper
/// clamp.
const INITIAL_WATER_PURITY_RVA: u32 = 0x0023_af00;

/// Vtable slots `setWaterLevel` dispatches after writing the level: `resetUnitAI` (`+0x34`) and
/// `validatePositions` (`+0x40`).
const RESET_UNIT_AI_VTABLE_OFFSET: u32 = 0x34;
const VALIDATE_POSITIONS_VTABLE_OFFSET: u32 = 0x40;

/// String ids of the toasts: `setWaterPurity` (water became extremely murky), `fill` (an entity blocks
/// it) and `drain` (an entity blocks it; shown by id).
const WATER_MURKY_MESSAGE_ID: u32 = 0x2935;
const FILL_BLOCKED_MESSAGE_ID: u32 = 0x2936;
const DRAIN_BLOCKED_MESSAGE_ID: u32 = 0x2937;

/// `drain`'s `sendEvent` (vtable slot `+0`) event id and category.
const DRAIN_EVENT_ID: u32 = 0x2716;
const DRAIN_EVENT_CATEGORY: u32 = 0x53;

/// `BFEntity`'s entity-type pointer, and the entity-type vtable slots `fill` (`+0x20`, string id of the
/// type's name) and `checkEntitiesForDrain` (`+0xc8`, a bool predicate) call.
const ENTITY_TYPE_OFFSET: u32 = 0x128;
const ENTITY_TYPE_NAME_ID_VTABLE_OFFSET: u32 = 0x20;
const ENTITY_TYPE_DRAIN_BLOCKER_VTABLE_OFFSET: u32 = 0xc8;

/// `BFEntity` fields and vtable slots the sparkle/ripple code touches: the entity id (`+0x124`), rotation
/// (`+0x12c`), world position (`+0x114`), the `GXMixer` (`+0x74`), and slots `setIsRemoved(bool, bool)`
/// (`+0xa8`), destroy (`+0xcc`) and `validatePosition(bool)` (`+0x84`).
const ENTITY_ID_OFFSET: u32 = 0x124;
const ENTITY_ROTATION_OFFSET: u32 = 0x12c;
const ENTITY_WORLD_POS_OFFSET: u32 = 0x114;
const ENTITY_MIXER_OFFSET: u32 = 0x74;
const ENTITY_SET_IS_REMOVED_VTABLE_OFFSET: u32 = 0xa8;
const ENTITY_DESTROY_VTABLE_OFFSET: u32 = 0xcc;
const ENTITY_VALIDATE_POSITION_VTABLE_OFFSET: u32 = 0x84;

/// `DAT_006390b4`: a sparkle is retired once its age (ticks) exceeds this (signed).
const SPARKLE_MAX_AGE_RVA: u32 = 0x0023_90b4;

/// A `BFTile`'s four per-direction wall entity slots start here (indexed by `direction / 2`), and a tank
/// wall entity's own byte at `+0x464` gates whether it gets a ripple.
const TILE_WALL_SLOTS_OFFSET: u32 = 0x14;
const TANK_WALL_RIPPLE_FLAG_OFFSET: u32 = 0x464;

/// Entity-type lookup names: `findTypes` takes the category, subcategory and type name.
const ENTITY_TYPE_CATEGORY: &[u8] = b"objects";
const ENTITY_TYPE_SUBCATEGORY: &[u8] = b"other";
const SALT_SPARKLE_TYPE_NAME: &[u8] = b"twaterwv";
const FRESH_SPARKLE_TYPE_NAME: &[u8] = b"frshwav";
const RIPPLE_TYPE_NAME: &[u8] = b"tankripl";

/// Ripple wave offset range: `addWaterRipple` starts each ripple's `GXMixer` at `rand % this`.
const RIPPLE_ANIMATION_START_RANGE: u32 = 2000;

/// `isCastClass` type tags `checkEntitiesForDrain` tests (`CAST_ZTAnimal`, `0x00638690`, and
/// `DAT_006386b0`).
const RVA_ANIMAL_TYPE_CHECK_ARG: u32 = 0x0023_8690;
const RVA_TYPE_CHECK_ARG_6386B0: u32 = 0x0023_86b0;

/// `BFTile` bytes `setBaseTerrainType`/`setBaseLevel` raise around the map call that edits the tile: the
/// terrain-paint guard (`+0x87`) and the height-adjust guard (`+0x86`).
const TILE_PAINTING_FLAG_OFFSET: u32 = 0x87;
const TILE_ADJUSTING_FLAG_OFFSET: u32 = 0x86;

/// `ZTAdvTerrainMgr`'s terrain-type info vector (`begin` at `+0x10`, `end` at `+0x14`; elements are `0x30`
/// bytes) and the `BFMap` embedded in `ZTWorldMgr` at `+0x8`.
const ADV_TERRAIN_MGR_INFO_BEGIN_OFFSET: u32 = 0x10;
const ADV_TERRAIN_MGR_INFO_END_OFFSET: u32 = 0x14;
const TERRAIN_TYPE_INFO_SIZE: u32 = 0x30;
const WORLD_MGR_BFMAP_OFFSET: u32 = 0x8;

/// `setBaseLevel` clamps the requested level to this signed range.
const BASE_LEVEL_MIN: i32 = -12;
const BASE_LEVEL_MAX: i32 = 12;

/// `ZTApp`'s `loadInProgress` byte (`+0x441`) off [`RVA_APP_INIT_SUCCESS_BASE`]: the terrain setters do
/// nothing while it is set.
const LOAD_IN_PROGRESS_OFFSET: u32 = 0x441;

/// Owned-tile list node: the tile pointer sits at `+0x8`. `setSalinity` sets bit `0x1` of each tile's
/// flag byte so the tile is refreshed.
const OWNED_TILE_NODE_TILE_OFFSET: u32 = 0x8;
const TILE_FLAG_NEEDS_REFRESH: u8 = 0x1;

/// `isCastClass` type tags `canBeInTank` (`CAST_ZTSceneryType`, `0x00638670`) and `getIllegalEntities`
/// (`DAT_00639100`) test.
const RVA_SCENERY_TYPE_CHECK_ARG: u32 = 0x0023_8670;
const RVA_TYPE_CHECK_ARG_639100: u32 = 0x0023_9100;

/// Entity-type vtable slot (`+0xc4`) `canBeInTank` calls on a `DAT_006386b0`-class type: a bool predicate.
const ENTITY_TYPE_CAN_BE_IN_TANK_VTABLE_OFFSET: u32 = 0xc4;

/// Entity-type bytes: a non-zero `+0x6d` excludes the entity from the tile-list scans of `getAllEntities`/
/// `getIllegalEntities`; a zero `+0x6f` makes `isIllegalEntity` mode 2 flag the entity.
const ENTITY_TYPE_SKIP_IN_TILE_SCAN_OFFSET: u32 = 0x6d;
const ENTITY_TYPE_MODE2_ALLOWED_OFFSET: u32 = 0x6f;

/// `ZTSceneryType` bytes `canBeInTank` reads: a scenery type is tank-safe when either is non-zero.
const SCENERY_TYPE_TANK_SAFE_A_OFFSET: u32 = 0x130;
const SCENERY_TYPE_TANK_SAFE_B_OFFSET: u32 = 0x131;

/// `ZTAnimal` byte (`+0x395`) that makes `canBeInTank` reject the animal when non-zero.
const ANIMAL_NOT_IN_TANK_OFFSET: u32 = 0x395;

/// `BFEntity`'s owner link (`+0x158`): `[[[entity+0x158]]]` is the owner's entity id.
const ENTITY_OWNER_LINK_OFFSET: u32 = 0x158;

/// `BFTile`'s entity bookkeeping: the pointer at `+0x0` is the sentinel of the list of entities on the
/// tile (node `+0x8` = entity), `+0x4` starts four direct entity slots, and `TILE_WALL_SLOTS_OFFSET`
/// four wall slots.
const TILE_ENTITY_SLOTS_OFFSET: u32 = 0x4;
const TILE_SLOT_COUNT: u32 = 4;

/// `isIllegalEntity` modes.
const ILLEGAL_MODE_DRAIN_BLOCKER: i32 = 0;
const ILLEGAL_MODE_CANNOT_BE_IN_TANK: i32 = 1;
const ILLEGAL_MODE_NOT_MODE2_ALLOWED: i32 = 2;

/// `removeIllegalEntities`: the tank vtable slot (`+0x20`, `isTank`) it asks before the extra mode-2 pass, the
/// zero-argument `f32` sell-value getters it feeds `removeEntityFromMap` (scenery `+0x120`, animal and staff
/// `+0x2a4`; each is the entity type's purchase cost times a global factor).
const TANK_IS_TANK_VTABLE_OFFSET: u32 = 0x20;
const SCENERY_SELL_VALUE_VTABLE_OFFSET: u32 = 0x120;
const UNIT_SELL_VALUE_VTABLE_OFFSET: u32 = 0x2a4;

/// `BFTile`'s owner back-pointer (`+0x88`; `reclaimTiles` points it at the habitat's owned-tile field)
/// and the flag bit that marks the tile as owned.
const TILE_OWNER_LIST_OFFSET: u32 = 0x88;
const TILE_FLAG_OWNED: u8 = 0x20;

/// Shows `text` through `BFUIMgr::displayMessage` (the `std::string` overload) at `priority`, attached to
/// `entity_ptr` (`0` for none). The message vector is built on vanilla's allocator, as the overload's
/// callers do, and freed after the call.
fn display_message_text(text: &str, priority: i32, entity_ptr: u32) {
    let bytes = text.as_bytes();
    let mut message = [0u32; 3];
    unsafe { VECTORBYTE.original()(message.as_mut_ptr() as *const u8, (bytes.len() + 1) as *const u32) };
    let begin = message[0];
    for (i, &byte) in bytes.iter().enumerate() {
        save_to_memory::<u8>(begin + i as u32, byte);
    }
    save_to_memory::<u8>(begin + bytes.len() as u32, 0);
    message[1] = begin + bytes.len() as u32;

    let bfuimgr = get_module_base("zoo.exe") as u32 + RVA_GLOBAL_BFUIMGR;
    unsafe { DISPLAY_MESSAGE_1.hooked()(bfuimgr as *const u32, message.as_ptr() as *const i32, priority, std::ptr::null(), entity_ptr as *const u32, true, false) };

    if begin != 0 {
        unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, message[2] - begin) };
    }
}

impl ZTTankExhibit {
    /// The in/out gate tile pointers, or `None` when either is missing (real vanilla dereferences them
    /// unconditionally; a habitat with a gate always has both).
    fn gate_tiles(&self) -> Option<(u32, u32)> {
        let gate_in = self.habitat.gate_tile_in_ptr();
        let gate_out = self.habitat.gate_tile_out_ptr();
        (gate_in != 0 && gate_out != 0).then_some((gate_in, gate_out))
    }

    fn tile_world(tile_ptr: u32, local: IVec3) -> IVec3 {
        let tile_pos: IVec3 = get_from_memory(tile_ptr + TILE_POS_OFFSET);
        globals().ztworldmgr().tile_to_world(tile_pos, local)
    }

    /// Real `BFMap::getDirection(from, to)` then `BFTile::snapToEdge` on it: the tile-local point on the
    /// edge facing that direction.
    fn snapped_edge(from_tile: u32, to_tile: u32) -> IVec3 {
        let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(from_tile as i32, to_tile as i32) };
        let mut snapped = [0i32; 3];
        unsafe { SNAP_TO_EDGE.original()(snapped.as_mut_ptr() as *const u32, direction as u32) };
        IVec3::new(snapped[0], snapped[1], snapped[2])
    }

    /// Ports `ZTTankExhibit::getPlatformPos` (`0x00499cfa`): the in-gate tile's centre in world space,
    /// raised `tank_height * 16`. `None` without a gate-in tile.
    pub fn get_platform_pos(&self) -> Option<IVec3> {
        let gate_in = self.habitat.gate_tile_in_ptr();
        if gate_in == 0 {
            return None;
        }
        let mut pos = Self::tile_world(gate_in, TILE_LOCAL_CENTER);
        pos.z = pos.z.wrapping_add((self.tank_height as i32).wrapping_mul(WORLD_Z_PER_LEVEL));
        Some(pos)
    }

    /// Ports `ZTTankExhibit::getLadderBaseInsidePos` (`0x0049b1cc`): the in-gate tile's world position at
    /// the edge facing the out-gate tile.
    pub fn get_ladder_base_inside_pos(&self) -> Option<IVec3> {
        let (gate_in, gate_out) = self.gate_tiles()?;
        Some(Self::tile_world(gate_in, Self::snapped_edge(gate_in, gate_out)))
    }

    /// Ports `ZTTankExhibit::getLadderTopInsidePos` (`0x0049b231`): [`Self::get_ladder_base_inside_pos`]'s
    /// `x`/`y` at the platform's `z`.
    pub fn get_ladder_top_inside_pos(&self) -> Option<IVec3> {
        let base = self.get_ladder_base_inside_pos()?;
        Some(IVec3::new(base.x, base.y, self.get_platform_pos()?.z))
    }

    /// Ports `ZTTankExhibit::getLadderBaseOutsidePos` (`0x00499e80`): the out-gate tile's world position
    /// at the edge facing the in-gate tile. When the out tile carries the tank-water flag, `z` is raised
    /// by that tile's habitat's water level; `z` is then held between the in-gate tile's own height
    /// (floor) and the platform's (ceiling).
    pub fn get_ladder_base_outside_pos(&self) -> Option<IVec3> {
        let (gate_in, gate_out) = self.gate_tiles()?;
        let mut pos = Self::tile_world(gate_out, Self::snapped_edge(gate_out, gate_in));

        if get_from_memory::<u8>(gate_out + TILE_FLAGS_OFFSET) & TILE_FLAG_TANK_WATER != 0 {
            let out_tile_pos: IVec3 = get_from_memory(gate_out + TILE_POS_OFFSET);
            let out_habitat = globals().zthabitatmgr().get_habitat_ptr(out_tile_pos.x, out_tile_pos.y);
            if out_habitat != 0 {
                let water_level = unsafe { ref_from_memory::<ZTTankExhibit>(out_habitat) }.water_level as i32;
                pos.z = pos.z.wrapping_add(water_level.wrapping_mul(WORLD_Z_PER_LEVEL));
            }
        }

        let gate_in_z = Self::tile_world(gate_in, TILE_LOCAL_CENTER).z;
        pos.z = Self::clamp_outside_z(pos.z, gate_in_z, self.get_platform_pos()?.z);
        Some(pos)
    }

    /// Ports `ZTTankExhibit::getLadderTopOutsidePos` (`0x00499f95`): [`Self::get_ladder_base_outside_pos`]'s
    /// `x`/`y` at the platform's `z`.
    pub fn get_ladder_top_outside_pos(&self) -> Option<IVec3> {
        let base = self.get_ladder_base_outside_pos()?;
        Some(IVec3::new(base.x, base.y, self.get_platform_pos()?.z))
    }

    /// [`Self::get_ladder_base_outside_pos`]'s height clamp: signed `max(z, gate_in_z)`, then signed
    /// `min(.., platform_z)`.
    fn clamp_outside_z(z: i32, gate_in_z: i32, platform_z: i32) -> i32 {
        z.max(gate_in_z).min(platform_z)
    }

    /// Ports `ZTTankExhibit::getMinDepth` (`0x005af226`): despite the name, the *largest* `getDepth(entity,
    /// tile) - DAT_006390ac` over every entity in the tank (the minimum tank height that clears them all;
    /// vanilla keeps whichever of the running value and the new one is greater), starting from `2`. Entities
    /// are visited in [`Self::all_entities`]' (pointer) order.
    pub fn get_min_depth(&self) -> i32 {
        let depth_offset: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);

        let mut min_depth = MIN_DEPTH_INITIAL;
        for entity in self.all_entities() {
            let tile = unsafe { BFENTITY_GET_TILE.original()(entity as *const u32) } as u32;
            let vtable: u32 = get_from_memory(entity);
            let get_depth: unsafe extern "thiscall" fn(u32, u32) -> u32 =
                unsafe { std::mem::transmute(get_from_memory::<u32>(vtable + ENTITY_GET_DEPTH_VTABLE_OFFSET)) };
            let depth = unsafe { get_depth(entity, tile) } as i32;
            min_depth = Self::min_depth_step(min_depth, depth, depth_offset);
        }
        min_depth
    }

    /// One [`Self::get_min_depth`] iteration: signed `max(current, depth - offset)`.
    fn min_depth_step(current: i32, depth: i32, offset: i32) -> i32 {
        current.max(depth.wrapping_sub(offset))
    }

    /// Ports `ZTTankExhibit::getWaterLevel` (`0x004110fd`): the raw `water_level` field.
    pub fn get_water_level(&self) -> u32 {
        self.water_level
    }

    /// Ports `ZTTankExhibit::getTankHeight` (`0x00411111`): the raw `tank_height` field.
    pub fn get_tank_height(&self) -> u32 {
        self.tank_height
    }

    /// Ports `ZTTankExhibit::getWaterPurity` (`0x00493bcb`): the raw `water_purity` field.
    pub fn get_water_purity(&self) -> i32 {
        self.water_purity
    }

    /// Ports `ZTTankExhibit::isPureWater` (`0x004111a9`): `water_purity >= DAT_006390a4`, signed (`SETGE`).
    pub fn is_pure_water(&self) -> bool {
        self.water_purity >= Self::read_purity_threshold(MURKY_WATER_PURITY_RVA)
    }

    /// Ports `ZTTankExhibit::isExtremelyMurkyWater` (`0x00411254`): `water_purity <= DAT_006390b0`, signed
    /// (`SETLE`).
    pub fn is_extremely_murky_water(&self) -> bool {
        self.water_purity <= Self::read_purity_threshold(EXTREMELY_MURKY_WATER_PURITY_RVA)
    }

    fn read_purity_threshold(rva: u32) -> i32 {
        get_from_memory(get_module_base("zoo.exe") as u32 + rva)
    }

    fn read_setting<T: Copy>(rva: u32) -> T {
        get_from_memory(get_module_base("zoo.exe") as u32 + rva)
    }

    /// Ports `ZTTankExhibit::getCostToChangeHeight` (`0x004ef687`, `RET 8`, `f32` in `ST0`): the cost of
    /// moving the tank from height `from_height` to `to_height`. Only the part of the move above the
    /// settings' initial height is charged (`initialHeightShow` for a show tank, else `initialHeight`;
    /// see [`Self::billable_height_delta`]), at `cost_rate_a` per step plus `cost_rate_b` per step while
    /// the tank is filled.
    ///
    /// Vanilla computes on the x87 stack and hands the unrounded sum back in `ST0`; this sums in `f64`
    /// (exact for the two `f32 * i32-as-f32` products) and rounds once, which is what every caller's
    /// `FSTP float` does to the real value.
    pub fn get_cost_to_change_height(&self, from_height: i32, to_height: i32) -> f32 {
        let initial_height_rva = if self.is_show_tank_dispatched() { INITIAL_HEIGHT_SHOW_RVA } else { INITIAL_HEIGHT_RVA };
        let initial_height: i32 = Self::read_setting(initial_height_rva);
        let delta = Self::billable_height_delta(from_height, to_height, initial_height) as f32 as f64;
        let mut cost = self.cost_rate_a as f64 * delta;
        if self.is_filled {
            cost += self.cost_rate_b as f64 * delta;
        }
        cost as f32
    }

    /// [`Self::get_cost_to_change_height`]'s height delta (signed compares throughout, as the `.asm`'s
    /// `JGE`/`JG`/`JNS`):
    /// - `from <= initial`: only the part of the move above `initial`, `max(to - initial, 0)`;
    /// - `from > initial` and `to < initial`: the move down to `initial`, `initial - from` (negative);
    /// - otherwise the whole move, `to - from`.
    fn billable_height_delta(from_height: i32, to_height: i32, initial_height: i32) -> i32 {
        if initial_height >= from_height {
            let above_initial = to_height.wrapping_sub(from_height).wrapping_add(from_height.wrapping_sub(initial_height));
            if above_initial < 0 { 0 } else { above_initial }
        } else if initial_height > to_height {
            initial_height.wrapping_sub(from_height)
        } else {
            to_height.wrapping_sub(from_height)
        }
    }

    /// Ports `ZTTankExhibit::updateAdjustmentCosts` (`0x00459b19`): recomputes the three cost fields.
    ///
    /// - `cost_rate_b` = owned-tile count * the water cost (`saltWater` while `current_water_type > 0`,
    ///   else `freshWater`);
    /// - `cost_rate_c` = owned-tile count * `ZTWorldMgr[+0x64]`;
    /// - `cost_rate_a` = the sum of `getPurchaseCost` over every tank wall in `wall_ids_*`, divided by
    ///   `wallHeightPriceDivisor` (`0` when that is `0`). Ids that no longer resolve to an entity, or
    ///   whose type is not a tank wall, add nothing. Vanilla repeats the wall-type check on the same
    ///   object before the cost call; the repeat cannot differ, so it is not ported.
    ///
    /// The sum is kept in `f32` between walls, like the `FADD`/`FSTP float` pair it mirrors, and the
    /// final division is done in `f64` over the `FILD`ed divisor.
    pub fn update_adjustment_costs(&self) {
        let water_cost_rva = if self.current_water_type > 0 { SALT_WATER_COST_RVA } else { FRESH_WATER_COST_RVA };
        let water_cost: f32 = Self::read_setting(water_cost_rva);
        let tile_count = walk_tile_list(self.owned_tiles_ptr).count() as u32;
        write_live!(self, cost_rate_b, (tile_count as f64 * water_cost as f64) as f32);

        let world_mgr = globals().ztworldmgr_ptr() as u32;
        let tile_cost: f32 = get_from_memory(world_mgr + WORLD_MGR_TILE_COST_OFFSET);
        let tile_count = walk_tile_list(self.owned_tiles_ptr).count() as u32;
        write_live!(self, cost_rate_a, 0.0f32);
        write_live!(self, cost_rate_c, (tile_count as f64 * tile_cost as f64) as f32);

        let mut wall_cost_sum = 0.0f32;
        let mut wall_id_ptr = self.wall_ids_begin;
        while wall_id_ptr != self.wall_ids_end {
            let wall_id: i32 = get_from_memory(wall_id_ptr);
            let wall = globals().ztworldmgr().resolve_entity_by_id(wall_id as u32) as u32;
            if wall != 0 && unsafe { entity_type_matches(wall, RVA_TANK_WALL_TYPE_CHECK_ARG) } {
                let wall_type: u32 = get_from_memory(wall + 0x128);
                let vtable: u32 = get_from_memory(wall_type);
                let get_purchase_cost: extern "thiscall" fn(u32) -> f32 =
                    unsafe { std::mem::transmute(get_from_memory::<u32>(vtable + WALL_TYPE_GET_PURCHASE_COST_VTABLE_OFFSET)) };
                wall_cost_sum += get_purchase_cost(wall_type);
            }
            wall_id_ptr += 4;
        }

        let divisor: i32 = Self::read_setting(WALL_HEIGHT_PRICE_DIVISOR_RVA);
        let wall_cost_rate = if divisor == 0 { 0.0 } else { (wall_cost_sum as f64 / divisor as f64) as f32 };
        write_live!(self, cost_rate_a, wall_cost_rate);
    }

    /// Ports `ZTTankExhibit::cleanWaterAmount` (`0x0049bd60`): raises the purity by `amount` through the
    /// real `setWaterPurity` (it clamps and manages the tank's visual state).
    pub fn clean_water_amount(&self, amount: i32) {
        self.set_water_purity(self.water_purity.wrapping_add(amount));
    }

    /// The live `ZTWorldMgr` pointer, `0` while no world exists (vanilla's `GLOBAL_ZTWorldMgr != 0` gate).
    fn world_ptr() -> u32 {
        globals().ztworldmgr_ptr() as u32
    }

    /// Retires one sparkle/ripple entity the way every removal here does: `setIsRemoved(true, false)`
    /// (vtable `+0xa8`), then, unless `destroy` is `false`, the destroy slot (`+0xcc`).
    unsafe fn retire_entity(entity: u32, destroy: bool) {
        unsafe {
            call_vtable_slot_with_u8_u8(entity, ENTITY_SET_IS_REMOVED_VTABLE_OFFSET, 1, 0);
            if destroy {
                call_vtable_slot_with_u8(entity, ENTITY_DESTROY_VTABLE_OFFSET, 0);
            }
        }
    }

    /// Looks up an entity type through the live world manager's `findTypes` (`0x004aa240`) the way
    /// `addRandomSparkle`/`addWaterRipple` do: three vanilla-layout strings (`objects`, `other`, `name`)
    /// built on vanilla's allocator, one result vector, the first hit's type pointer. `None` when the
    /// result is empty. The strings and the result vector are freed through the same freelist split vanilla's
    /// destructors use.
    fn find_entity_type(name: &[u8]) -> Option<u32> {
        fn build_string(string: &mut [u32; 3], text: &[u8]) {
            let begin = text.as_ptr() as u32;
            unsafe { BASIC_STRING_ASSIGN_RANGE.original()(string.as_mut_ptr() as *const std::ffi::c_void, begin as *const u32, (begin + text.len() as u32) as i32) };
        }
        fn free_string(string: &[u32; 3]) {
            free_event_vector_buffer(string[0], string[2].wrapping_sub(string[0]));
        }

        let world = Self::world_ptr();
        if world == 0 {
            return None;
        }
        let (mut category, mut subcategory, mut type_name) = ([0u32; 3], [0u32; 3], [0u32; 3]);
        build_string(&mut category, ENTITY_TYPE_CATEGORY);
        build_string(&mut subcategory, ENTITY_TYPE_SUBCATEGORY);
        build_string(&mut type_name, name);

        let mut found = [0u32; 3];
        unsafe {
            FIND_TYPES.original()(
                world as *const u32,
                found.as_mut_ptr() as *const u32,
                category.as_ptr(),
                subcategory.as_ptr(),
                type_name.as_ptr() as u32,
            )
        };
        let first = (found[1].wrapping_sub(found[0]) >> 2 != 0).then(|| get_from_memory::<u32>(found[0]));

        free_event_vector_buffer(found[0], found[2].wrapping_sub(found[0]));
        free_string(&type_name);
        free_string(&subcategory);
        free_string(&category);
        first
    }

    /// Adds `entity` to the world as a transient on `tile` and, when accepted, validates its position
    /// (vtable `+0x84(true)`). Returns whether the world accepted it.
    fn add_transient(entity: u32, tile: u32) -> bool {
        let world = Self::world_ptr();
        if world == 0 {
            return false;
        }
        if !low_byte_bool(unsafe { ADD_TRANSIENT.original()(world as *const u32, entity as *const u32, tile as *const u32) }) {
            return false;
        }
        unsafe { call_vtable_slot_with_u8(entity, ENTITY_VALIDATE_POSITION_VTABLE_OFFSET, 1) };
        true
    }

    /// Ports `ZTTankExhibit::removeAllSparkles` (`0x005050e2`): retires every tracked sparkle entity
    /// (`setIsRemoved` only) and empties both sparkle vectors.
    pub fn remove_all_sparkles(&self) {
        if Self::world_ptr() == 0 {
            return;
        }
        let mut element = self.sparkle_entities_begin;
        while element != self.sparkle_entities_end {
            let entity = get_from_memory::<u32>(element + 4);
            if entity != 0 {
                unsafe { Self::retire_entity(entity, false) };
            }
            element += 8;
        }
        write_live!(self, sparkle_entities_end, self.sparkle_entities_begin);
        write_live!(self, sparkle_entity_ids_end, self.sparkle_entity_ids_begin);
    }

    /// Ports `ZTTankExhibit::removeAllSparklesById` (`0x005052bd`): resolves each tracked sparkle id
    /// through the world's `getEntityReverse` (vtable `+0x28`, flag `false`), retires and destroys the
    /// ones still alive, and empties both sparkle vectors.
    pub fn remove_all_sparkles_by_id(&self) {
        let world = Self::world_ptr();
        if world == 0 {
            return;
        }
        let mut element = self.sparkle_entity_ids_begin;
        while element != self.sparkle_entity_ids_end {
            let id = get_from_memory::<u32>(element);
            let entity = unsafe { GET_ENTITY_REVERSE.original()(world as *const u32, id as i32, false) };
            if entity != 0 {
                unsafe { Self::retire_entity(entity, true) };
            }
            element += 4;
        }
        write_live!(self, sparkle_entities_end, self.sparkle_entities_begin);
        write_live!(self, sparkle_entity_ids_end, self.sparkle_entity_ids_begin);
    }

    /// Ports `ZTTankExhibit::removeDeadSparkles` (`0x004962fb`): ages every sparkle by `elapsed`; those
    /// past `DAT_006390b4` (signed) lose their id entry, are retired and destroyed, and are then erased
    /// from the sparkle vector.
    ///
    /// Both erases end by shrinking the vector by one element whether or not the element was found
    /// (vanilla's unconditional `end -= size`), so a missing match drops the last element instead.
    pub fn remove_dead_sparkles(&self, elapsed: u32) {
        if Self::world_ptr() == 0 {
            return;
        }
        let max_age: i32 = Self::read_setting(SPARKLE_MAX_AGE_RVA);
        let mut dead: Vec<(u32, u32)> = Vec::new();
        let mut element = self.sparkle_entities_begin;
        while element != self.sparkle_entities_end {
            let age = get_from_memory::<u32>(element).wrapping_add(elapsed);
            save_to_memory(element, age);
            if max_age < age as i32 {
                let entity = get_from_memory::<u32>(element + 4);
                if entity != 0 {
                    self.erase_sparkle_id(get_from_memory::<u32>(entity + ENTITY_ID_OFFSET));
                    unsafe { Self::retire_entity(entity, true) };
                }
                dead.push((age, entity));
            }
            element += 8;
        }
        for (age, entity) in dead {
            self.erase_sparkle_pair(age, entity);
        }
    }

    /// [`Self::remove_dead_sparkles`]' id-vector erase: first match of `id` (or the end), later ids shift
    /// down, then the vector shrinks by one id regardless.
    fn erase_sparkle_id(&self, id: u32) {
        let end = self.sparkle_entity_ids_end;
        let mut slot = self.sparkle_entity_ids_begin;
        while slot != end && get_from_memory::<u32>(slot) != id {
            slot += 4;
        }
        let mut next = slot + 4;
        let mut count = (end as i32).wrapping_sub(next as i32) >> 2;
        if next != end && count > 0 {
            while count != 0 {
                save_to_memory(slot, get_from_memory::<u32>(next));
                slot += 4;
                next += 4;
                count -= 1;
            }
        }
        write_live!(self, sparkle_entity_ids_end, self.sparkle_entity_ids_end.wrapping_sub(4));
    }

    /// [`Self::remove_dead_sparkles`]' pair-vector erase: first `(age, entity)` match (or the end), later
    /// pairs shift down, then the vector shrinks by one pair regardless.
    fn erase_sparkle_pair(&self, age: u32, entity: u32) {
        let end = self.sparkle_entities_end;
        let mut slot = self.sparkle_entities_begin;
        while slot != end && !(get_from_memory::<u32>(slot) == age && get_from_memory::<u32>(slot + 4) == entity) {
            slot += 8;
        }
        let mut next = slot + 8;
        let mut count = (end as i32).wrapping_sub(next as i32) >> 3;
        if next != end && count > 0 {
            while count != 0 {
                save_to_memory(slot, get_from_memory::<u32>(next));
                save_to_memory(slot + 4, get_from_memory::<u32>(next + 4));
                slot += 8;
                next += 8;
                count -= 1;
            }
        }
        write_live!(self, sparkle_entities_end, self.sparkle_entities_end.wrapping_sub(8));
    }

    /// Ports `ZTTankExhibit::addRandomSparkle` (`0x00496382`, `fastcall`): on a tank with water at or above
    /// the pure threshold, spawns one sparkle entity on a random owned tile.
    ///
    /// The sparkle type is the freshwater one when `current_water_type == 0`, else the salt-water one,
    /// looked up on first use and cached (a failed lookup caches `0` and spawns nothing). Draws one RNG
    /// value for the tile (`% tile count`), creates the entity, draws a second for its rotation (`& 7`),
    /// places it at the tile centre and, when the world accepts it, appends `(0, entity)` and the entity id
    /// to the two sparkle vectors. A tank with no owned tiles spawns nothing (vanilla divides by zero).
    pub fn add_random_sparkle(&self) {
        if !(self.is_pure_water() && self.water_level != 0) {
            return;
        }
        let fresh = self.current_water_type == 0;
        let cached = if fresh { self.fresh_water_sparkle_type } else { self.salt_water_sparkle_type };
        let sparkle_type = if cached != 0 {
            cached
        } else {
            let found = Self::find_entity_type(if fresh { FRESH_SPARKLE_TYPE_NAME } else { SALT_SPARKLE_TYPE_NAME }).unwrap_or(0);
            if fresh {
                write_live!(self, fresh_water_sparkle_type, found);
            } else {
                write_live!(self, salt_water_sparkle_type, found);
            }
            if found == 0 {
                return;
            }
            found
        };

        let tile_count = walk_tile_list(self.owned_tiles_ptr).count() as u32;
        if tile_count == 0 {
            return;
        }
        let pick = (next_game_random() % tile_count) as usize;
        let node = walk_tile_list(self.owned_tiles_ptr).nth(pick).unwrap_or(0);
        let tile = get_from_memory::<u32>(node + OWNED_TILE_NODE_TILE_OFFSET);

        let entity = unsafe { call_entity_type_create(sparkle_type) };
        if entity == 0 {
            return;
        }
        save_to_memory(entity + ENTITY_ROTATION_OFFSET, next_game_random() & 7);
        let position = Self::tile_world(tile, TILE_LOCAL_CENTER);
        unsafe { SET_WORLD_POS.original()(entity as *const u32, &position as *const IVec3) };
        if !Self::add_transient(entity, tile) {
            return;
        }

        let this = self.this_ptr() as u32;
        push_pair(this + std::mem::offset_of!(ZTTankExhibit, sparkle_entities_begin) as u32, 0, entity, free_event_vector_buffer);
        push_word(
            this + std::mem::offset_of!(ZTTankExhibit, sparkle_entity_ids_begin) as u32,
            get_from_memory::<u32>(entity + ENTITY_ID_OFFSET),
            free_event_vector_buffer,
        );
    }

    /// Ports `ZTTankExhibit::removeWaterRipples` (`0x005ae615`): resolves each ripple id through the world's
    /// `getEntity` (vtable `+0x24`, flag `true`), retires and destroys the ones still alive, and empties the
    /// ripple id vector.
    pub fn remove_water_ripples(&self) {
        if Self::world_ptr() == 0 {
            return;
        }
        let mut element = self.ripple_entity_ids_begin;
        while element != self.ripple_entity_ids_end {
            let id = get_from_memory::<u32>(element);
            let entity = globals().ztworldmgr().resolve_entity_by_id(id) as u32;
            if entity != 0 {
                unsafe { Self::retire_entity(entity, true) };
            }
            element += 4;
        }
        write_live!(self, ripple_entity_ids_end, self.ripple_entity_ids_begin);
    }

    /// Ports `ZTTankExhibit::addWaterRipples` (`0x005af78f`): on a tank with any water, replaces the
    /// ripples with one per tank-wall entity on each owned tile's four wall slots (directions `0, 2, 4, 6`)
    /// whose type is a tank wall and whose `+0x464` flag is set.
    pub fn add_water_ripples(&self) {
        if self.water_level == 0 {
            return;
        }
        self.remove_water_ripples();
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<u32>(node + OWNED_TILE_NODE_TILE_OFFSET);
            for direction in [0u32, 2, 4, 6] {
                let wall = get_from_memory::<u32>(tile + TILE_WALL_SLOTS_OFFSET + (direction / 2) * 4);
                if wall != 0
                    && unsafe { entity_type_matches(wall, RVA_TANK_WALL_TYPE_CHECK_ARG) }
                    && get_from_memory::<u8>(wall + TANK_WALL_RIPPLE_FLAG_OFFSET) != 0
                {
                    self.add_water_ripple(tile, direction, wall);
                }
            }
        }
    }

    /// Ports `ZTTankExhibit::addWaterRipple` (`0x005af81a`, `RET 0xc`): spawns one ripple entity at `wall`'s
    /// world position with rotation `direction`, starts its `GXMixer` at a random offset (`% 2000`), and
    /// when the world accepts it on `tile` records its id in the ripple vector. The ripple type is looked
    /// up on first use and cached; a failed lookup caches `0` and spawns nothing.
    pub fn add_water_ripple(&self, tile: u32, direction: u32, wall: u32) {
        let ripple_type = if self.ripple_entity_type != 0 {
            self.ripple_entity_type
        } else {
            let found = Self::find_entity_type(RIPPLE_TYPE_NAME).unwrap_or(0);
            write_live!(self, ripple_entity_type, found);
            if found == 0 {
                return;
            }
            found
        };

        let entity = unsafe { call_entity_type_create(ripple_type) };
        if entity == 0 {
            return;
        }
        save_to_memory(entity + ENTITY_ROTATION_OFFSET, direction);
        unsafe { SET_WORLD_POS.original()(entity as *const u32, (wall + ENTITY_WORLD_POS_OFFSET) as *const IVec3) };
        let animation_start = (next_game_random() % RIPPLE_ANIMATION_START_RANGE) as i32;
        unsafe { GXMIXER_UPDATE.original()((entity + ENTITY_MIXER_OFFSET) as *const u32, animation_start, 0) };
        if !Self::add_transient(entity, tile) {
            return;
        }

        push_word(
            self.this_ptr() as u32 + std::mem::offset_of!(ZTTankExhibit, ripple_entity_ids_begin) as u32,
            get_from_memory::<u32>(entity + ENTITY_ID_OFFSET),
            free_event_vector_buffer,
        );
    }

    fn this_ptr(&self) -> *const u32 {
        self as *const Self as *const u32
    }

    fn app_init_success() -> bool {
        get_from_memory::<u8>(get_module_base("zoo.exe") as u32 + RVA_APP_INIT_SUCCESS_BASE + APP_INIT_SUCCESS_OFFSET) != 0
    }

    /// Ports `ZTTankExhibit::setWaterLevel` (`0x005ae565`): clamps `level` to `[0, tank_height +
    /// DAT_006390ac]` (signed), writes it, and keeps the ripples, owned transients and tank-modify UI in
    /// step.
    ///
    /// - A tank with no water that is getting some, at or above the pure threshold, adds ripples *before*
    ///   the write; the post-write block adds them again for the same transition (vanilla does both, and
    ///   so does this).
    /// - Draining to zero from a non-empty tank removes owned transients before the write; the post-write
    ///   block then cleans the water and removes them again.
    /// - After the write, vtable `+0x34` (`resetUnitAI`) and `+0x40` (`validatePositions`) run on the tank.
    ///
    /// Ends by refreshing the tank-modify UI and setting `NeedsOrderRecalculate`.
    pub fn set_water_level(&self, level: i32) {
        let this = self.this_ptr();
        let old_level = self.water_level as i32;
        let max_level = (self.tank_height as i32).wrapping_add(Self::read_setting::<i32>(WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA));
        let level = Self::clamp_water_level(level, max_level);

        if old_level == 0 {
            if level != 0 && self.is_pure_water() {
                self.add_water_ripples();
            }
        } else if level == 0 {
            self.remove_owned_transients();
        }

        write_live!(self, water_level, level as u32);
        unsafe {
            call_vtable_slot_noargs(this as u32, RESET_UNIT_AI_VTABLE_OFFSET);
            call_vtable_slot_noargs(this as u32, VALIDATE_POSITIONS_VTABLE_OFFSET);
        }

        if (self.water_level as i32) < 1 {
            if old_level > 0 {
                unsafe {
                    SET_WATER_CLEAN.original()(this as *const std::ffi::c_void);
                    self.remove_owned_transients();
                    FETCH_VALUES_FROM_TANK.original()();
                }
            }
        } else if old_level < 1 && self.is_pure_water() {
            self.add_water_ripples();
        }

        unsafe {
            FETCH_VALUES_FROM_TANK.original()();
            SET_NEEDS_ORDER_RECALCULATE.original()(1);
        }
    }

    /// [`Self::set_water_level`]'s clamp: signed, the lower bound `0` first and then the upper bound (so a
    /// negative upper bound wins).
    fn clamp_water_level(level: i32, max_level: i32) -> i32 {
        let level = if level < 0 { 0 } else { level };
        if level > max_level { max_level } else { level }
    }

    /// Ports `ZTTankExhibit::setWaterPurity` (`0x0049340d`): clamps `value` to `[0, DAT_0063af00]`
    /// (signed), writes it, then reacts to crossing either threshold:
    ///
    /// - from pure (`>= murkyWaterPurity`) to murky: removes the ripples; from murky to pure: adds them;
    /// - from not-extremely-murky to extremely murky (`<= DAT_006390b0`): recreates the object
    ///   animations and shows the "water is murky" message (string `0x2935`, `%s` = the exhibit name);
    /// - while already extremely murky and no longer so: recreates the object animations.
    pub fn set_water_purity(&self, value: i32) {
        let was_pure = self.is_pure_water();
        let was_extremely_murky = self.is_extremely_murky_water();

        let max_purity: i32 = Self::read_setting(INITIAL_WATER_PURITY_RVA);
        write_live!(self, water_purity, Self::clamp_water_purity(value, max_purity));
        let murky_threshold = Self::read_purity_threshold(MURKY_WATER_PURITY_RVA);

        if was_pure {
            if self.water_purity < murky_threshold {
                self.remove_water_ripples();
            }
        } else if self.is_pure_water() {
            self.add_water_ripples();
        }

        if was_extremely_murky {
            if !self.is_extremely_murky_water() {
                self.habitat.recreate_oas();
            }
        } else if self.is_extremely_murky_water() {
            self.habitat.recreate_oas();
            let name = self.habitat.exhibit_name.copy_to_string();
            let message = load_string_by_id(WATER_MURKY_MESSAGE_ID).unwrap_or_default();
            display_message_text(&Self::substitute_name(&message, &name), 0, 0);
        }
    }

    /// [`Self::set_water_purity`]'s clamp: signed, the upper bound `DAT_0063af00` first, then `0`.
    fn clamp_water_purity(value: i32, max_purity: i32) -> i32 {
        let value = if max_purity < value { max_purity } else { value };
        if value < 0 { 0 } else { value }
    }

    fn substitute_name(template: &str, name: &str) -> String {
        template.replace("%s", name)
    }

    /// Ports `ZTTankExhibit::setSalinity` (`0x0060644b`): switches the tank to water type `water_type`,
    /// returning whether it did anything.
    ///
    /// A request for the current type is a no-op. Otherwise the tank drains first; if it is still filled
    /// afterwards (blocked by an entity) nothing changes. The new type is staged in
    /// `pending_water_type`; a tank that is already empty applies it at once (costs recomputed, tank-modify
    /// UI refreshed, every owned tile flagged `|= 1`). Outside loading, a tank that held water when called
    /// is filled again.
    pub fn set_salinity(&self, water_type: i32) -> bool {
        let was_empty = (self.water_level as i32) <= 0;
        if water_type == self.current_water_type {
            return false;
        }
        self.drain();
        if self.is_filled {
            return false;
        }
        write_live!(self, pending_water_type, water_type);
        if (self.water_level as i32) <= 0 {
            write_live!(self, current_water_type, water_type);
            self.update_adjustment_costs();
            unsafe { FETCH_VALUES_FROM_TANK.original()() };
            for node in walk_tile_list(self.owned_tiles_ptr) {
                let tile: u32 = get_from_memory(node + OWNED_TILE_NODE_TILE_OFFSET);
                let flags: u8 = get_from_memory(tile + TILE_FLAGS_OFFSET);
                save_to_memory(tile + TILE_FLAGS_OFFSET, flags | TILE_FLAG_NEEDS_REFRESH);
            }
        }
        if Self::app_init_success() && !was_empty {
            self.fill();
        }
        true
    }

    fn load_in_progress() -> bool {
        get_from_memory::<u8>(get_module_base("zoo.exe") as u32 + RVA_APP_INIT_SUCCESS_BASE + LOAD_IN_PROGRESS_OFFSET) != 0
    }

    fn world_bfmap() -> *const u32 {
        (globals().ztworldmgr_ptr() as u32 + WORLD_MGR_BFMAP_OFFSET) as *const u32
    }

    /// Ports `ZTTankExhibit::setBaseTerrainType` (`0x00458bea`): repaints every owned tile with the terrain
    /// type at index `terrain_type` of `ZTAdvTerrainMgr`'s info vector (a null cell, which `paintCell`
    /// takes, when the index is out of range). Each tile is selected on the map, painted, and has its
    /// `+0x87` guard byte raised for the duration. Does nothing while a load is in progress or without a
    /// terrain manager.
    pub fn set_base_terrain_type(&self, terrain_type: i32) {
        if Self::load_in_progress() {
            return;
        }
        let adv_terrain_mgr = globals().ztadvterrainmgr_ptr() as u32;
        if adv_terrain_mgr == 0 {
            return;
        }
        let map = Self::world_bfmap();
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + OWNED_TILE_NODE_TILE_OFFSET);
            save_to_memory(tile + TILE_PAINTING_FLAG_OFFSET, 1u8);
            let tile_pos: IVec3 = get_from_memory(tile + TILE_POS_OFFSET);
            unsafe { SET_SELECTION.original()(map, tile_pos.x, tile_pos.y, 1) };
            let info_begin: u32 = get_from_memory(adv_terrain_mgr + ADV_TERRAIN_MGR_INFO_BEGIN_OFFSET);
            let info_end: u32 = get_from_memory(adv_terrain_mgr + ADV_TERRAIN_MGR_INFO_END_OFFSET);
            let cell = Self::terrain_info_cell(info_begin, info_end, terrain_type as u32);
            unsafe { PAINT_CELL.original()(map, cell as i32, true) };
            save_to_memory(tile + TILE_PAINTING_FLAG_OFFSET, 0u8);
        }
    }

    /// The `index`th `0x30`-byte terrain info cell of `[begin, end)`, or `0` when `index` (unsigned) is
    /// past the end.
    fn terrain_info_cell(begin: u32, end: u32, index: u32) -> u32 {
        if index < end.wrapping_sub(begin) / TERRAIN_TYPE_INFO_SIZE { begin.wrapping_add(index.wrapping_mul(TERRAIN_TYPE_INFO_SIZE)) } else { 0 }
    }

    /// Ports `ZTTankExhibit::setBaseLevel` (`0x00458c6a`): raises or lowers the tank's ground by `level`
    /// (clamped to `[-12, 12]`, signed) through `BFMap::adjustCell` on the first owned tile (selected on the
    /// map, with its `+0x86` guard byte raised), then runs the `resetUnitAI` vtable slot, revalidates the map
    /// tiles, refreshes the show and amphibious neighbours and sets `unknown_flag_0x30`. Does nothing while
    /// a load is in progress.
    pub fn set_base_level(&self, level: i32) {
        if Self::load_in_progress() {
            return;
        }
        let level = level.clamp(BASE_LEVEL_MIN, BASE_LEVEL_MAX);
        let Some(first_node) = walk_tile_list(self.owned_tiles_ptr).next() else {
            return;
        };
        let map = Self::world_bfmap();
        let tile: u32 = get_from_memory(first_node + OWNED_TILE_NODE_TILE_OFFSET);
        save_to_memory(tile + TILE_ADJUSTING_FLAG_OFFSET, 1u8);
        let tile_pos: IVec3 = get_from_memory(tile + TILE_POS_OFFSET);
        unsafe {
            SET_SELECTION.original()(map, tile_pos.x, tile_pos.y, 1);
            ADJUST_CELL.original()(map, level, false);
        }
        save_to_memory(tile + TILE_ADJUSTING_FLAG_OFFSET, 0u8);
        unsafe {
            call_vtable_slot_noargs(self.this_ptr() as u32, RESET_UNIT_AI_VTABLE_OFFSET);
            VALIDATE_TILES.original()(map);
        }
        let habitat_mgr = globals().zthabitatmgr();
        habitat_mgr.update_show_neighbors(self.this_ptr() as u32);
        habitat_mgr.update_amphibious_neighbors(self.this_ptr() as u32);
        write_live!(self.habitat, unknown_flag_0x30, 1u8);
    }

    /// Ports `ZTTankExhibit::checkEntitiesForFill` (`0x00459e09`): the first entity (in the set's pointer
    /// order) inside the tank that [`ZTTankExhibit::canBeInTank`](CAN_BE_IN_TANK) rejects, or `0`.
    pub fn check_entities_for_fill(&self) -> u32 {
        self.first_matching_entity(|entity| !Self::can_be_in_tank(entity))
    }

    /// Ports `ZTTankExhibit::checkEntitiesForDrain` (`0x0060656e`): the first entity inside the tank that
    /// is an animal, or whose type answers true to vtable slot `+0xc8` while passing the
    /// `DAT_006386b0` type check; else `0`. (Vanilla also runs a scenery-type check whose result is
    /// never read.)
    pub fn check_entities_for_drain(&self) -> u32 {
        self.first_matching_entity(|entity| unsafe { entity_type_matches(entity, RVA_ANIMAL_TYPE_CHECK_ARG) } || Self::is_type_drain_blocker(entity))
    }

    /// Walks [`Self::all_entities`] in set order and returns the first entity `matches` accepts, else `0`.
    fn first_matching_entity(&self, matches: impl Fn(u32) -> bool) -> u32 {
        self.all_entities().into_iter().find(|&entity| matches(entity)).unwrap_or(0)
    }

    /// Ports `ZTTankExhibit::canBeInTank` (`0x00459f52`, stdcall, one stack argument): whether `entity`
    /// may stay in a filled tank. A null entity can. Otherwise, in vanilla's order: an entity whose type is
    /// of the `DAT_006386b0` class is rejected when its type's vtable `+0xc4` predicate is false; an animal
    /// is rejected when its byte `+0x395` is set; a scenery entity is rejected unless one of its type's
    /// bytes `+0x130`/`+0x131` is set. (Vanilla asks the animal question through the type pointer without a
    /// null check; an entity always has a type.)
    pub fn can_be_in_tank(entity: u32) -> bool {
        if entity == 0 {
            return true;
        }
        let entity_type: u32 = get_from_memory(entity + ENTITY_TYPE_OFFSET);
        let guarded_class = unsafe { entity_type_matches(entity, RVA_TYPE_CHECK_ARG_6386B0) };
        let scenery_type = unsafe { entity_type_matches(entity, RVA_SCENERY_TYPE_CHECK_ARG) }.then_some(entity_type);
        let is_animal = unsafe { entity_type_matches(entity, RVA_ANIMAL_TYPE_CHECK_ARG) };

        if guarded_class {
            let vtable: u32 = get_from_memory(entity_type);
            let allowed: extern "thiscall" fn(u32) -> bool =
                unsafe { std::mem::transmute(get_from_memory::<u32>(vtable + ENTITY_TYPE_CAN_BE_IN_TANK_VTABLE_OFFSET)) };
            if !allowed(entity_type) {
                return false;
            }
        }
        if is_animal && get_from_memory::<u8>(entity + ANIMAL_NOT_IN_TANK_OFFSET) != 0 {
            return false;
        }
        match scenery_type {
            Some(scenery) => {
                get_from_memory::<u8>(scenery + SCENERY_TYPE_TANK_SAFE_A_OFFSET) != 0 || get_from_memory::<u8>(scenery + SCENERY_TYPE_TANK_SAFE_B_OFFSET) != 0
            }
            None => true,
        }
    }

    /// Ports `ZTTankExhibit::isIllegalEntity` (`0x0045da65`, stdcall `(entity, mode)`): whether `entity`
    /// must go for `mode`. Mode `0`: a non-null entity whose type is of the `DAT_006386b0` class and
    /// whose type vtable `+0xc8` predicate holds (what blocks a drain). Mode `1`: `!canBeInTank`. Mode
    /// `2`: a non-null entity whose type's byte `+0x6f` is zero. Any other mode, and a null entity,
    /// are legal.
    pub fn is_illegal_entity(entity: u32, mode: i32) -> bool {
        match mode {
            ILLEGAL_MODE_DRAIN_BLOCKER => entity != 0 && Self::is_type_drain_blocker(entity),
            ILLEGAL_MODE_CANNOT_BE_IN_TANK => !Self::can_be_in_tank(entity),
            ILLEGAL_MODE_NOT_MODE2_ALLOWED => {
                if entity == 0 {
                    return false;
                }
                let entity_type: u32 = get_from_memory(entity + ENTITY_TYPE_OFFSET);
                entity_type != 0 && get_from_memory::<u8>(entity_type + ENTITY_TYPE_MODE2_ALLOWED_OFFSET) == 0
            }
            _ => false,
        }
    }

    /// The `DAT_006386b0` branch of [`Self::check_entities_for_drain`], shared with
    /// [`Self::is_illegal_entity`] mode `0`.
    fn is_type_drain_blocker(entity: u32) -> bool {
        if !unsafe { entity_type_matches(entity, RVA_TYPE_CHECK_ARG_6386B0) } {
            return false;
        }
        let entity_type: u32 = get_from_memory(entity + ENTITY_TYPE_OFFSET);
        let vtable: u32 = get_from_memory(entity_type);
        let predicate: extern "thiscall" fn(u32) -> bool =
            unsafe { std::mem::transmute(get_from_memory::<u32>(vtable + ENTITY_TYPE_DRAIN_BLOCKER_VTABLE_OFFSET)) };
        predicate(entity_type)
    }

    /// `cls_0x615acd::meth_0x615acd` (`0x00615acd`): the entity `[[[entity+0x158]]]` names through the
    /// world's `getEntity(id, true)`, or `0` when `entity` has no owner link.
    fn owner_entity(entity: u32) -> u32 {
        let link: u32 = get_from_memory(entity + ENTITY_OWNER_LINK_OFFSET);
        if link == 0 {
            return 0;
        }
        let id: u32 = get_from_memory(get_from_memory::<u32>(link));
        Self::entity_by_id(id as i32)
    }

    /// The entities `getAllEntities` (`mode == None`) or `getIllegalEntities` (`Some(mode)`) offer to
    /// their output set, in vanilla's insertion order (duplicates included). Per owned tile: the four
    /// direct entity slots (`getAllEntities` takes every non-null one; `getIllegalEntities` those
    /// [`Self::is_illegal_entity`] flags, swapping in their [`Self::owner_entity`] when the type matches
    /// `DAT_00639100`), then the tile's entity list (entities whose type byte `+0x6d` is zero, and for
    /// `getIllegalEntities` also illegal), then, for `getIllegalEntities` only, the four wall slots (the
    /// illegal ones).
    fn collect_entities(&self, mode: Option<i32>) -> Vec<u32> {
        let mut collected = Vec::new();
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + OWNED_TILE_NODE_TILE_OFFSET);
            for slot in 0..TILE_SLOT_COUNT {
                let entity: u32 = get_from_memory(tile + TILE_ENTITY_SLOTS_OFFSET + slot * 4);
                match mode {
                    None if entity != 0 => collected.push(entity),
                    Some(mode) if Self::is_illegal_entity(entity, mode) => {
                        let resolved = if entity != 0 && unsafe { entity_type_matches(entity, RVA_TYPE_CHECK_ARG_639100) } {
                            Self::owner_entity(entity)
                        } else {
                            entity
                        };
                        collected.push(resolved);
                    }
                    _ => {}
                }
            }

            for list_node in walk_tile_list(get_from_memory(tile)) {
                let entity: u32 = get_from_memory(list_node + OWNED_TILE_NODE_TILE_OFFSET);
                let entity_type: u32 = get_from_memory(entity + ENTITY_TYPE_OFFSET);
                if entity_type != 0
                    && get_from_memory::<u8>(entity_type + ENTITY_TYPE_SKIP_IN_TILE_SCAN_OFFSET) == 0
                    && mode.is_none_or(|mode| Self::is_illegal_entity(entity, mode))
                {
                    collected.push(entity);
                }
            }

            if let Some(mode) = mode {
                for slot in 0..TILE_SLOT_COUNT {
                    let entity: u32 = get_from_memory(tile + TILE_WALL_SLOTS_OFFSET + slot * 4);
                    if entity != 0 && Self::is_illegal_entity(entity, mode) {
                        collected.push(entity);
                    }
                }
            }
        }
        collected
    }

    /// Ports `ZTTankExhibit::getAllEntities` (`0x004599a0`) as a Rust set: every entity standing in or on
    /// the tank, in the pointer order a vanilla `std::set` walks them.
    pub fn all_entities(&self) -> BTreeSet<u32> {
        self.collect_entities(None).into_iter().collect()
    }

    /// Ports `ZTTankExhibit::getAllEntities`' output half: inserts every entity of [`Self::all_entities`]
    /// into the vanilla `std::set<BFEntity*>` whose `{head*, size}` pair sits at `container`.
    pub fn get_all_entities_into(&self, container: u32) {
        for entity in self.collect_entities(None) {
            rb_set_insert(container, entity);
        }
    }

    /// Ports `ZTTankExhibit::getIllegalEntities` (`0x0045da8b`): inserts every entity
    /// [`Self::is_illegal_entity`] flags for `mode` into the vanilla set at `container`.
    pub fn get_illegal_entities_into(&self, container: u32, mode: i32) {
        for entity in self.collect_entities(Some(mode)) {
            rb_set_insert(container, entity);
        }
    }

    /// The entity `id` names (`ZTWorldMgr` vtable `+0x24`, `getEntity(id, true)`), or `0`.
    fn entity_by_id(id: i32) -> u32 {
        unsafe { GET_ENTITY.original()(Self::world_ptr() as *const u32, id, true) as u32 }
    }

    /// Runs `f` on every live `ZTTankWall` entity whose id is in the wall vector (`+0x178`..`+0x17c`; the
    /// end is re-read each step).
    fn for_each_tank_wall(&self, f: impl Fn(u32)) {
        let end_addr = self.this_ptr() as u32 + std::mem::offset_of!(ZTTankExhibit, wall_ids_end) as u32;
        let mut slot = self.wall_ids_begin;
        while slot != get_from_memory::<u32>(end_addr) {
            let wall = Self::entity_by_id(get_from_memory::<i32>(slot));
            if wall != 0 && unsafe { entity_type_matches(wall, RVA_TANK_WALL_TYPE_CHECK_ARG) } {
                f(wall);
            }
            slot += 4;
        }
    }

    /// Ports `ZTTankExhibit::clearWallVector` (`0x0045ca92`): drops every tank wall's height to `0`, then
    /// empties the wall id vector (its buffer stays).
    pub fn clear_wall_vector(&self) {
        self.for_each_tank_wall(|wall| unsafe { SET_WALL_HEIGHT.original()(wall as *const u32, 0) });
        write_live!(self, wall_ids_end, self.wall_ids_begin);
    }

    /// Ports `ZTTankExhibit::updateTankWallInfo` (`0x00459ab2`): sets every tank wall to `tank_height`, then
    /// recomputes the adjustment costs.
    pub fn update_tank_wall_info(&self) {
        let height = self.tank_height as i32;
        self.for_each_tank_wall(|wall| unsafe { SET_WALL_HEIGHT.original()(wall as *const u32, height) });
        self.update_adjustment_costs();
    }

    /// Ports `ZTTankExhibit::addTankWall` (`0x0045ca13`): appends `wall`'s entity id (`+0x124`) to the wall
    /// id vector, growing it the way vanilla's `vector<i32>` does.
    pub fn add_tank_wall(&self, wall: u32) {
        push_word(
            self.this_ptr() as u32 + std::mem::offset_of!(ZTTankExhibit, wall_ids_begin) as u32,
            get_from_memory::<u32>(wall + ENTITY_ID_OFFSET),
            free_event_vector_buffer,
        );
    }

    /// Ports `ZTTankExhibit::removeOwnedTransients` (`0x00505099`): removes the ripples and sparkles and,
    /// when a filter is attached and still exists, its bubbles.
    pub fn remove_owned_transients(&self) {
        self.remove_water_ripples();
        self.remove_all_sparkles();
        if self.attached_filter_id != -1 {
            let filter = Self::entity_by_id(self.attached_filter_id);
            if filter != 0 {
                unsafe { REMOVE_BUBBLES.original()(filter as *const u32) };
            }
        }
    }

    /// Ports `ZTTankExhibit::restoreOwnedTransients` (`0x006066eb`): brings the ripples back for a tank
    /// that holds pure water.
    pub fn restore_owned_transients(&self) {
        if self.get_water_level() != 0 && self.is_pure_water() {
            self.add_water_ripples();
        }
    }

    /// Ports `ZTTankExhibit::validateFilterPosition` (`0x00498026`): forgets the attached filter (`-1`)
    /// when its entity is gone or no longer attached to this tank.
    pub fn validate_filter_position(&self) {
        if self.attached_filter_id == -1 {
            return;
        }
        let filter = Self::entity_by_id(self.attached_filter_id);
        if filter == 0 || !low_byte_bool(unsafe { IS_STILL_ATTACHED.original()(filter as *const u32, self.this_ptr()) } as u32) {
            write_live!(self, attached_filter_id, -1i32);
        }
    }

    /// Ports `ZTTankExhibit::removeIllegalEntities` (`0x00458c2b`, `RET 8`): removes or relocates every
    /// entity [`Self::is_illegal_entity`] flags for `mode`, and returns `false` only when `confirm` is set
    /// and the player declines the sell-off dialog.
    ///
    /// The flagged set is [`Self::collect_entities`]`(Some(mode))`, plus mode `2`'s when `mode == 1` and the
    /// tank is a show tank (vtable `+0x20` and a non-null `zt_show_info_ptr`). Nothing flagged returns
    /// `true`, as does a load in progress. Each flagged entity then, in set order:
    ///
    /// - **animal**: mode `0` boxes it (`ZTAnimal::box`); modes `1` and `2` box it up and look for a place
    ///   near the first boundary pair's second tile (`boxUpAndFindAPlaceForAnimal`), removing it when none
    ///   is found; other modes leave it.
    /// - **scenery type**: always removed.
    /// - **staff**: placed near the gate-out tile (else the first boundary pair's second tile) with
    ///   `findAPlaceForStaffMember`, removed when that fails.
    ///
    /// Removing is `ZTMapView::prepareToDeleteEntity`, then `removeEntityFromMap(entity, sell value, 0, 0,
    /// 0)` where the sell value is the entity's `f32` getter (vtable `+0x120` for scenery, `+0x2a4`
    /// otherwise). Vanilla reads the first boundary pair out of the habitat's own `+0x48` vector, which is
    /// empty here; the pair comes from [`ZTHabitat::boundary_tile_pairs`] and a missing one is tile `0`
    /// (no place found), where vanilla would dereference null.
    pub fn remove_illegal_entities(&self, mode: i32, confirm: bool) -> bool {
        if Self::load_in_progress() {
            return true;
        }
        let this = self.this_ptr() as u32;
        let mut flagged: BTreeSet<u32> = self.collect_entities(Some(mode)).into_iter().collect();
        if mode == ILLEGAL_MODE_CANNOT_BE_IN_TANK && unsafe { call_vtable_slot_noargs_ret_bool(this, TANK_IS_TANK_VTABLE_OFFSET) } && self.habitat.zt_show_info_ptr != 0 {
            flagged.extend(self.collect_entities(Some(ILLEGAL_MODE_NOT_MODE2_ALLOWED)));
        }
        if flagged.is_empty() {
            return true;
        }
        if confirm && !low_byte_bool(unsafe { CONFIRM_SELL_OFF.original()() } as u32) {
            return false;
        }

        let map_view = unsafe { GET_MAPVIEW.original()() } as u32;
        let first_pair_second_tile = || self.habitat.boundary_tile_pairs().first().map_or(0, |pair| pair.1);
        for entity in flagged {
            if entity == 0 {
                continue;
            }
            let is_animal = unsafe { entity_type_matches(entity, RVA_ANIMAL_TYPE_CHECK_ARG) };
            if is_animal {
                if mode == 0 {
                    unsafe { ZTANIMAL_BOX.original()(entity as *const u32) };
                } else if (1..=2).contains(&mode) {
                    let placed = unsafe { BOX_UP_AND_FIND_APLACE_FOR_ANIMAL.original()(entity as *const u32, first_pair_second_tile() as *const u32) };
                    if !low_byte_bool(placed) {
                        Self::remove_entity_from_map(map_view, entity, UNIT_SELL_VALUE_VTABLE_OFFSET);
                    }
                }
            } else if unsafe { entity_type_matches(entity, RVA_SCENERY_TYPE_CHECK_ARG) } {
                Self::remove_entity_from_map(map_view, entity, SCENERY_SELL_VALUE_VTABLE_OFFSET);
            } else if unsafe { entity_type_matches(entity, RVA_STAFF_TYPE_CHECK_ARG) } {
                let gate_out = self.habitat.gate_tile_out_ptr();
                let tile = if gate_out != 0 { gate_out } else { first_pair_second_tile() };
                if !low_byte_bool(unsafe { FIND_APLACE_FOR_STAFF_MEMBER.original()(entity as *const u32, tile as *const std::ffi::c_void) }) {
                    Self::remove_entity_from_map(map_view, entity, UNIT_SELL_VALUE_VTABLE_OFFSET);
                }
            }
        }
        true
    }

    /// [`Self::remove_illegal_entities`]' removal: `prepareToDeleteEntity`, then `removeEntityFromMap` with
    /// the entity's sell value from vtable slot `sell_value_offset`.
    fn remove_entity_from_map(map_view: u32, entity: u32, sell_value_offset: u32) {
        let vtable: u32 = get_from_memory(entity);
        let sell_value: extern "thiscall" fn(u32) -> f32 = unsafe { std::mem::transmute(get_from_memory::<u32>(vtable + sell_value_offset)) };
        unsafe {
            PREPARE_TO_DELETE_ENTITY.original()(map_view as *const u32, entity as *const u32);
            REMOVE_ENTITY_FROM_MAP.original()(map_view as *const u32, entity as *const i32, sell_value(entity), 0, std::ptr::null(), 0);
        }
    }

    /// Ports `ZTTankExhibit::levelAllTiles` (`0x0045bbbb`): selects the highest owned tile (first one on a
    /// tie) and lowers the map cell by `12` there. Does nothing while a load is in progress.
    pub fn level_all_tiles(&self) {
        if Self::load_in_progress() {
            return;
        }
        let mut highest: Option<(u32, i32)> = None;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + OWNED_TILE_NODE_TILE_OFFSET);
            let height = get_from_memory::<IVec3>(tile + TILE_POS_OFFSET).z;
            if highest.is_none_or(|(_, best)| height > best) {
                highest = Some((tile, height));
            }
        }
        let Some((tile, _)) = highest else {
            return;
        };
        let map = Self::world_bfmap();
        let tile_pos: IVec3 = get_from_memory(tile + TILE_POS_OFFSET);
        save_to_memory(tile + TILE_ADJUSTING_FLAG_OFFSET, 1u8);
        unsafe {
            SET_SELECTION.original()(map, tile_pos.x, tile_pos.y, 1);
            ADJUST_CELL.original()(map, BASE_LEVEL_MIN, false);
        }
        save_to_memory(tile + TILE_ADJUSTING_FLAG_OFFSET, 0u8);
    }

    /// Ports `ZTTankExhibit::reclaimTiles` (`0x00487a63`): points the habitat manager's ownership cell of
    /// every owned tile at this tank (flagging the tile `|= 1` when that changed it), then marks the tile
    /// owned (`|= 0x20`), clears its two guard bytes and sets its owner back-pointer to this tank's
    /// owned-tile field.
    pub fn reclaim_tiles(&self) {
        let this = self.this_ptr() as u32;
        let habitat_mgr = globals().zthabitatmgr();
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + OWNED_TILE_NODE_TILE_OFFSET);
            if tile == 0 {
                continue;
            }
            let tile_pos: IVec3 = get_from_memory(tile + TILE_POS_OFFSET);
            let mut flags: u8 = get_from_memory(tile + TILE_FLAGS_OFFSET);
            if let Some(cell) = habitat_mgr.get_habitat_cell_addr(tile_pos.x, tile_pos.y).filter(|&cell| get_from_memory::<u32>(cell) != this) {
                save_to_memory(cell, this);
                flags |= TILE_FLAG_NEEDS_REFRESH;
            }
            save_to_memory(tile + TILE_ADJUSTING_FLAG_OFFSET, 0u8);
            save_to_memory(tile + TILE_PAINTING_FLAG_OFFSET, 0u8);
            save_to_memory(tile + TILE_OWNER_LIST_OFFSET, this + std::mem::offset_of!(ZTHabitat, owned_tiles_ptr) as u32);
            save_to_memory(tile + TILE_FLAGS_OFFSET, flags | TILE_FLAG_OWNED);
        }
    }

    /// Ports `ZTTankExhibit::doDepthSuitability` (`0x005af4b9`, `RET 4`): for a filled tank with a gate-in
    /// tile, outside loading, re-evaluates the habitat's terrain suitability as if the water level were
    /// `tank_height + DAT_006390ac` before and `new_height + DAT_006390ac` after
    /// (`terrainAboutToBeChanged`/`terrainChanged` on the gate tile), restores the real level and marks
    /// the characteristics dirty.
    pub fn do_depth_suitability(&self, new_height: i32) {
        let gate_in = self.habitat.gate_tile_in_ptr();
        if !self.is_filled || gate_in == 0 || Self::load_in_progress() {
            return;
        }
        let offset: i32 = Self::read_setting(WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);
        let tile_pos: IVec3 = get_from_memory(gate_in + TILE_POS_OFFSET);
        let real_level = self.water_level;
        let habitat_mgr = globals().zthabitatmgr();

        write_live!(self, water_level, (self.tank_height as i32).wrapping_add(offset) as u32);
        habitat_mgr.terrain_about_to_be_changed(tile_pos.x, tile_pos.y, 1);
        write_live!(self, water_level, new_height.wrapping_add(offset) as u32);
        habitat_mgr.terrain_changed();
        write_live!(self, water_level, real_level);
        self.habitat.set_dirty_characteristics();
    }

    /// Ports `ZTTankExhibit::fill` (`0x00459d94`): does nothing while `fill_suppressed` is set. A blocking
    /// entity ([`Self::check_entities_for_fill`]) aborts the fill with a toast (string `0x2936`, `%s` =
    /// the blocker's type name) when the tank has a gate tile. Otherwise marks the tank filled, stages
    /// the current water type as pending, raises the water to the tank height (outside loading) and
    /// refreshes the show neighbours.
    pub fn fill(&self) {
        if self.fill_suppressed != 0 {
            return;
        }
        let blocker = self.check_entities_for_fill();
        if blocker == 0 {
            write_live!(self, is_filled, true);
            write_live!(self, pending_water_type, self.current_water_type);
            if Self::app_init_success() {
                self.set_water_level(self.tank_height as i32);
            }
            globals().zthabitatmgr().update_show_neighbors(self.this_ptr() as u32);
        } else if self.habitat.gate_tile_in_ptr() != 0 {
            let entity_type: u32 = get_from_memory(blocker + ENTITY_TYPE_OFFSET);
            let vtable: u32 = get_from_memory(entity_type);
            let get_name_id: extern "thiscall" fn(u32) -> u32 =
                unsafe { std::mem::transmute(get_from_memory::<u32>(vtable + ENTITY_TYPE_NAME_ID_VTABLE_OFFSET)) };
            let type_name = load_string_by_id(get_name_id(entity_type)).unwrap_or_default();
            let message = load_string_by_id(FILL_BLOCKED_MESSAGE_ID).unwrap_or_default();
            display_message_text(&Self::substitute_name(&message, &type_name), 5, blocker);
        }
    }

    /// Ports `ZTTankExhibit::drain` (`0x006064ed`): a blocking entity ([`Self::check_entities_for_drain`])
    /// aborts with the string-id toast `0x2937` when the tank has a gate tile. Otherwise clears
    /// `is_filled`, sends event `0x2716` (category `0x53`, the show-info id) through vtable slot `+0`, drops
    /// the water level to zero outside loading, and refreshes the show neighbours.
    pub fn drain(&self) {
        let this = self.this_ptr();
        if self.check_entities_for_drain() != 0 {
            if self.habitat.gate_tile_in_ptr() != 0 {
                let bfuimgr = get_module_base("zoo.exe") as u32 + RVA_GLOBAL_BFUIMGR;
                unsafe { DISPLAY_MESSAGE_0.hooked()(bfuimgr as *const u32, DRAIN_BLOCKED_MESSAGE_ID, 5, std::ptr::null(), std::ptr::null(), true, false) };
            }
            return;
        }
        write_live!(self, is_filled, false);
        let show_info_id = self.habitat.get_show_info_id() as u32;
        let vtable: u32 = get_from_memory(this as u32);
        let send_event: extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u32) =
            unsafe { std::mem::transmute(get_from_memory::<u32>(vtable)) };
        send_event(this as u32, DRAIN_EVENT_ID, show_info_id, DRAIN_EVENT_CATEGORY, 0, 0, 1);
        if Self::app_init_success() {
            self.set_water_level(0);
        }
        globals().zthabitatmgr().update_show_neighbors(this as u32);
    }

    /// Ports `ZTTankExhibit::isRightSalinity` (vtable `+0x28`, `ZTTankExhibit_isRightSalinity.c`/`.asm`,
    /// `generated.rs`'s `zttankexhibit::IS_RIGHT_SALINITY`) - the tank-specific override of the base
    /// [`ZTHabitat::is_right_salinity`] constant-`true` stub. Looks the animal type's two
    /// water-suitability category values up (real vanilla `BFCategory::getValue` - a pure map walk that
    /// answers `0` on a missing key, detoured nowhere, so `.original()` is a plain vanilla call in every
    /// build profile) and answers `category9 <= category10` (signed - real vanilla's `SETGE`), inverted
    /// when `current_water_type == 0` (the same freshwater discriminant `addRandomSparkle`'s freshwater
    /// branch reads). `animal_type_ptr` is the bare `ZTAnimalType*` (the `+0x128` inner-class pointer
    /// every other entity-type consumer here passes).
    pub fn is_right_salinity(&self, animal_type_ptr: u32) -> bool {
        let category_map = animal_type_ptr + ANIMAL_TYPE_WATER_SUITABILITY_MAP_OFFSET;
        let category_10 = unsafe { GET_VALUE.original()(category_map as *const u32, SALINITY_CATEGORY_10) };
        let category_9 = unsafe { GET_VALUE.original()(category_map as *const u32, SALINITY_CATEGORY_9) };
        Self::salinity_result(category_10, category_9, self.current_water_type)
    }

    /// [`Self::is_right_salinity`]'s comparison tail, split out so the (water type x comparison
    /// direction) combinations stay host-safe unit-testable - the `getValue` lookups themselves are real
    /// vanilla calls and are covered by the live comparison test instead.
    fn salinity_result(category_10: i32, category_9: i32, current_water_type: i32) -> bool {
        let suitable = category_9 <= category_10;
        if current_water_type == 0 { !suitable } else { suitable }
    }

    /// Ports `ZTTankExhibit::update` (the tank vtable's `+0x24` override of the base
    /// [`ZTHabitat::update`] slot, `generated.rs`'s `zttankexhibit::UPDATE` at `0x0049625f`,
    /// `ZTTankExhibit_update.c`/`.asm` - the macOS decompile confirms the clean if/else shape behind
    /// the Windows `.c`'s duplicated `is_filled` test, a cold-block artifact): composes the base port
    /// (real vanilla's own flat `CALL ZTHabitat::update` reaches that same port at runtime, the base
    /// address being detoured), then runs the tank tick:
    ///
    /// - **rise/drain on `is_filled`**: while filled and below `tank_height + DAT_006390ac`, rise one
    ///   level per tick; while drained and above zero, fall one level per tick; falling to zero with a
    ///   different `pending_water_type` staged swaps it into `current_water_type` and rebuilds
    ///   (`updateAdjustmentCosts` + `fill`). A filled tank at its target skips the drain branch
    ///   entirely.
    /// - **purity countdown**: while water is present and `water_purity` is positive, count
    ///   `water_purity_timer` down by `elapsed`; below 1, reload it from `DAT_0063af04` and drop
    ///   `water_purity` by one.
    /// - **sparkle lifecycle**: retire dead sparkles (`removeDeadSparkles`), count `sparkle_spawn_timer`
    ///   down by `elapsed`; below 0, spawn one (`addRandomSparkle`) and reload the timer from
    ///   `DAT_006390b8 / owned_tile_count` (unsigned division, walking the owned-tile sentinel exactly
    ///   like [`walk_tile_list`]).
    ///
    /// Every water-level/purity mutation is a real `setWaterLevel`/`setWaterPurity` call-through, never
    /// a field write - the setters clamp, manage water ripples, `removeOwnedTransients` on drain-to-zero
    /// and can `recreateOAs`/post UI messages. Reads are plain field loads (`getWaterLevel` is a trivial
    /// `[ECX+0x188]` return, and the drain path re-reads the field *after* `setWaterLevel` returns
    /// precisely because the setter clamps - the field is the truth for the fill-transition check).
    ///
    /// [`Self::sparkle_timer_reload`] reloads 0 for a tile-less tank where real vanilla's `DIV` would
    /// raise `#DE` - unreachable live (a habitat with no owned tiles is never ticked) and a Rust panic
    /// can't reproduce a hardware divide fault safely.
    ///
    /// Must only be called on a live `ZTTankExhibit` reference, same precondition as
    /// [`Self::is_right_salinity`] - `self`'s own address is passed straight into every real vanilla
    /// call-through.
    pub fn update(&self, elapsed: u32) {
        self.habitat.update(elapsed);

        if self.is_filled {
            let rise_delta: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);
            if Self::rise_needed(self.water_level as i32, self.tank_height as i32, rise_delta) {
                self.set_water_level(self.water_level as i32 + 1);
            }
        } else if self.water_level as i32 > 0 {
            self.set_water_level(self.water_level as i32 - 1);
            if Self::fill_transition_due(self.water_level as i32, self.current_water_type, self.pending_water_type) {
                write_live!(self, current_water_type, self.pending_water_type);
                self.update_adjustment_costs();
                self.fill();
            }
        }

        if self.water_level as i32 > 0 && self.water_purity > 0 {
            let purity_before = self.water_purity;
            let timer = self.water_purity_timer.wrapping_sub(elapsed as i32);
            write_live!(self, water_purity_timer, timer);
            if Self::purity_timer_expired(timer) {
                let reload: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_PURITY_TIMER_RELOAD_RVA);
                write_live!(self, water_purity_timer, reload);
                self.set_water_purity(purity_before - 1);
            }
        }

        self.remove_dead_sparkles(elapsed);

        let sparkle_timer = self.sparkle_spawn_timer.wrapping_sub(elapsed as i32);
        write_live!(self, sparkle_spawn_timer, sparkle_timer);
        if Self::sparkle_timer_expired(sparkle_timer) {
            self.add_random_sparkle();
            let numerator: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + SPARKLE_SPAWN_INTERVAL_NUMERATOR_RVA);
            let tile_count = walk_tile_list(self.owned_tiles_ptr).count() as u32;
            write_live!(self, sparkle_spawn_timer, Self::sparkle_timer_reload(numerator, tile_count));
        }
    }

    /// [`Self::update`]'s rise-branch decision - real vanilla's signed `CMP/JL` against
    /// `tank_height + DAT_006390ac` (`ZTTankExhibit_update.asm`).
    fn rise_needed(water_level: i32, tank_height: i32, rise_delta: i32) -> bool {
        water_level < tank_height + rise_delta
    }

    /// [`Self::update`]'s purity-expiry decision - real vanilla's signed `JLE` (`< 1`, i.e. `<= 0`).
    fn purity_timer_expired(new_timer: i32) -> bool {
        new_timer < 1
    }

    /// [`Self::update`]'s sparkle-expiry decision - real vanilla's `JS` (strictly negative), one step
    /// stricter than [`Self::purity_timer_expired`]'s `<= 0` boundary.
    fn sparkle_timer_expired(new_timer: i32) -> bool {
        new_timer < 0
    }

    /// [`Self::update`]'s drain-completion transition: fires only at zero water with a pending water
    /// type that differs from the currently applied one.
    fn fill_transition_due(water_level: i32, current_water_type: i32, pending_water_type: i32) -> bool {
        water_level == 0 && current_water_type != pending_water_type
    }

    /// [`Self::update`]'s sparkle-timer reload - real vanilla's unsigned `DIV`
    /// (`DAT_006390b8 / owned_tile_count`, `XOR EDX,EDX; DIV EDI` in the `.asm`). A zero tile count
    /// reloads 0 where real vanilla would raise `#DE` - see [`Self::update`]'s doc comment.
    fn sparkle_timer_reload(numerator: u32, tile_count: u32) -> i32 {
        numerator.checked_div(tile_count).unwrap_or(0) as i32
    }
}

/// `ZTTankExhibit::isRightSalinity`'s own fixed address (`0x004936df`) is referenced exactly once in the
/// whole binary - the `+0x28` slot of the tank vtable itself (`0x006312e4`; no direct callers, no other
/// vtable shares it) - so this flat detour intercepts only real tank exhibits. Non-tank dispatch reaches
/// the already-detoured base stub instead (see [`ZTHabitat::is_right_salinity`]).
#[detour_mod]
pub mod detours {
    use super::*;
    use crate::util::ref_from_memory;

    #[detour(GET_WATER_LEVEL)]
    unsafe extern "thiscall" fn get_water_level(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_water_level()
    }

    #[detour(GET_TANK_HEIGHT)]
    unsafe extern "thiscall" fn get_tank_height(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_tank_height()
    }

    #[detour(GET_WATER_PURITY)]
    unsafe extern "thiscall" fn get_water_purity(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_water_purity() as u32
    }

    #[detour(IS_PURE_WATER)]
    unsafe extern "thiscall" fn is_pure_water(this: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.is_pure_water()
    }

    #[detour(IS_EXTREMELY_MURKY_WATER)]
    unsafe extern "thiscall" fn is_extremely_murky_water(this: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.is_extremely_murky_water()
    }

    #[detour(IS_RIGHT_SALINITY)]
    unsafe extern "thiscall" fn is_right_salinity(this: *const u32, animal_type: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.is_right_salinity(animal_type as u32)
    }

    /// `ZTTankExhibit::update`'s own fixed address (`0x0049625f`) is referenced exactly once in the
    /// whole binary - the `+0x24` slot of the tank vtable itself (`0x006312e0`; no direct callers, no
    /// other vtable shares it) - so this detour intercepts every real tank's per-tick tick through
    /// `ZTHabitatMgr::update`'s vtable dispatch, while non-tank habitats still reach the base
    /// `ZTHabitat::update` detour.
    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, elapsed: u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.update(elapsed);
    }

    #[detour(GET_PLATFORM_POS)]
    unsafe extern "thiscall" fn get_platform_pos(this: *const u32, out: *const i32) -> *const i32 {
        write_position(out, unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_platform_pos())
    }

    #[detour(GET_LADDER_BASE_OUTSIDE_POS)]
    unsafe extern "thiscall" fn get_ladder_base_outside_pos(this: *const u32, out: *const i32) -> *const i32 {
        write_position(out, unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_ladder_base_outside_pos())
    }

    #[detour(GET_LADDER_TOP_OUTSIDE_POS)]
    unsafe extern "thiscall" fn get_ladder_top_outside_pos(this: *const u32, out: *const i32) -> *const i32 {
        write_position(out, unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_ladder_top_outside_pos())
    }

    #[detour(GET_LADDER_BASE_INSIDE_POS)]
    unsafe extern "thiscall" fn get_ladder_base_inside_pos(this: *const u32, out: *const i32) -> *const i32 {
        write_position(out, unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_ladder_base_inside_pos())
    }

    #[detour(GET_LADDER_TOP_INSIDE_POS)]
    unsafe extern "thiscall" fn get_ladder_top_inside_pos(this: *const u32, out: *const i32) -> *const i32 {
        write_position(out, unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_ladder_top_inside_pos())
    }

    #[detour(GET_COST_TO_CHANGE_HEIGHT)]
    unsafe extern "thiscall" fn get_cost_to_change_height(this: *const u32, from_height: i32, to_height: i32) -> f32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_cost_to_change_height(from_height, to_height)
    }

    #[detour(UPDATE_ADJUSTMENT_COSTS)]
    unsafe extern "thiscall" fn update_adjustment_costs(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.update_adjustment_costs();
    }

    #[detour(CLEAN_WATER_AMOUNT)]
    unsafe extern "thiscall" fn clean_water_amount(this: *const u32, amount: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.clean_water_amount(amount);
    }

    #[detour(SET_WATER_LEVEL)]
    unsafe extern "thiscall" fn set_water_level(this: *const u32, level: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.set_water_level(level);
    }

    #[detour(SET_WATER_PURITY)]
    unsafe extern "thiscall" fn set_water_purity(this: *const u32, value: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.set_water_purity(value);
    }

    #[detour(SET_SALINITY)]
    unsafe extern "thiscall" fn set_salinity(this: *const u32, water_type: i32) -> u32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.set_salinity(water_type) as u32
    }

    #[detour(FILL)]
    unsafe extern "thiscall" fn fill(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.fill();
    }

    #[detour(DRAIN)]
    unsafe extern "thiscall" fn drain(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.drain();
    }

    #[detour(CHECK_ENTITIES_FOR_FILL)]
    unsafe extern "thiscall" fn check_entities_for_fill(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.check_entities_for_fill()
    }

    #[detour(CHECK_ENTITIES_FOR_DRAIN)]
    unsafe extern "thiscall" fn check_entities_for_drain(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.check_entities_for_drain() as i32
    }

    #[detour(REMOVE_ILLEGAL_ENTITIES)]
    unsafe extern "thiscall" fn remove_illegal_entities(this: *const u32, mode: i32, confirm: bool) -> bool {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.remove_illegal_entities(mode, confirm)
    }

    #[detour(CAN_BE_IN_TANK)]
    unsafe extern "stdcall" fn can_be_in_tank(entity: u32) -> u8 {
        ZTTankExhibit::can_be_in_tank(entity) as u8
    }

    #[detour(IS_ILLEGAL_ENTITY)]
    unsafe extern "stdcall" fn is_illegal_entity(entity: *const u32, mode: i32) -> u32 {
        ZTTankExhibit::is_illegal_entity(entity as u32, mode) as u32
    }

    #[detour(GET_ALL_ENTITIES)]
    unsafe extern "thiscall" fn get_all_entities(this: *const u32, container: *const std::ffi::c_void) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_all_entities_into(container as u32)
    }

    #[detour(GET_ILLEGAL_ENTITIES)]
    unsafe extern "thiscall" fn get_illegal_entities(this: *const u32, container: *const std::ffi::c_void, mode: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_illegal_entities_into(container as u32, mode)
    }

    #[detour(CLEAR_WALL_VECTOR)]
    unsafe extern "fastcall" fn clear_wall_vector(this: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this as *const u32) }.clear_wall_vector()
    }

    #[detour(UPDATE_TANK_WALL_INFO)]
    unsafe extern "thiscall" fn update_tank_wall_info(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.update_tank_wall_info()
    }

    #[detour(ADD_TANK_WALL)]
    unsafe extern "thiscall" fn add_tank_wall(this: *const u32, wall: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.add_tank_wall(wall as u32)
    }

    #[detour(REMOVE_OWNED_TRANSIENTS)]
    unsafe extern "thiscall" fn remove_owned_transients(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.remove_owned_transients()
    }

    #[detour(RESTORE_OWNED_TRANSIENTS)]
    unsafe extern "thiscall" fn restore_owned_transients(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.restore_owned_transients()
    }

    #[detour(VALIDATE_FILTER_POSITION)]
    unsafe extern "thiscall" fn validate_filter_position(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.validate_filter_position()
    }

    #[detour(LEVEL_ALL_TILES)]
    unsafe extern "thiscall" fn level_all_tiles(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.level_all_tiles()
    }

    #[detour(RECLAIM_TILES)]
    unsafe extern "thiscall" fn reclaim_tiles(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.reclaim_tiles()
    }

    #[detour(DO_DEPTH_SUITABILITY)]
    unsafe extern "thiscall" fn do_depth_suitability(this: *const u32, new_height: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.do_depth_suitability(new_height)
    }

    #[detour(SET_BASE_TERRAIN_TYPE)]
    unsafe extern "thiscall" fn set_base_terrain_type(this: *const u32, terrain_type: i32) {
        #[cfg(feature = "reimplementation-tests")]
        if crate::reimplementation_tests::base_terrain_recorder::record(this as u32, terrain_type) {
            return;
        }
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.set_base_terrain_type(terrain_type);
    }

    #[detour(SET_BASE_LEVEL)]
    unsafe extern "thiscall" fn set_base_level(this: *const u32, level: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.set_base_level(level);
    }

    #[detour(REMOVE_DEAD_SPARKLES)]
    unsafe extern "thiscall" fn remove_dead_sparkles(this: *const u32, elapsed: u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.remove_dead_sparkles(elapsed);
    }

    #[detour(ADD_RANDOM_SPARKLE)]
    unsafe extern "fastcall" fn add_random_sparkle(this: i32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this as *const u32) }.add_random_sparkle();
    }

    #[detour(REMOVE_ALL_SPARKLES)]
    unsafe extern "thiscall" fn remove_all_sparkles(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.remove_all_sparkles();
    }

    #[detour(REMOVE_ALL_SPARKLES_BY_ID)]
    unsafe extern "thiscall" fn remove_all_sparkles_by_id(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.remove_all_sparkles_by_id();
    }

    #[detour(REMOVE_WATER_RIPPLES)]
    unsafe extern "thiscall" fn remove_water_ripples(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.remove_water_ripples();
    }

    #[detour(ADD_WATER_RIPPLES)]
    unsafe extern "thiscall" fn add_water_ripples(this: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.add_water_ripples();
    }

    #[detour(ADD_WATER_RIPPLE)]
    unsafe extern "thiscall" fn add_water_ripple(this: *const u32, tile: *const u32, direction: u32, wall: *const u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.add_water_ripple(tile as u32, direction, wall as u32);
    }

    #[detour(GET_MIN_DEPTH)]
    unsafe extern "thiscall" fn get_min_depth(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.get_min_depth()
    }

    /// Writes `position` through the caller's out-pointer and returns that pointer in `EAX`, which every
    /// vanilla caller reads (`MOV reg,[EAX]`) straight after the call. A `None` (no gate tile; vanilla
    /// would fault) leaves the buffer untouched.
    fn write_position(out: *const i32, position: Option<IVec3>) -> *const i32 {
        if let Some(position) = position {
            save_to_memory(out as u32, position.x);
            save_to_memory(out as u32 + 0x4, position.y);
            save_to_memory(out as u32 + 0x8, position.z);
        }
        out
    }

    /// `ZTTankExhibit::setTankHeight` ends its height-changed branch with a loop that marks every
    /// viewing area of the habitat's own `+0x34` vector for recreation (`+0x25 = 1`). That vector is empty
    /// in vanilla memory (the viewing areas live in [`super::viewing_areas`]'s store), so vanilla's loop is
    /// a no-op; this partial override runs the real function and then applies the same marking through the
    /// store exactly when the real function changed the height - its changed-branch condition.
    #[detour(SET_TANK_HEIGHT)]
    unsafe extern "thiscall" fn set_tank_height(this: *const u32, height: i32) {
        let height_before = get_from_memory::<u32>(this as u32 + TANK_HEIGHT_OFFSET);
        unsafe { SET_TANK_HEIGHT_DETOUR.call(this, height) };
        if get_from_memory::<u32>(this as u32 + TANK_HEIGHT_OFFSET) != height_before {
            unsafe { ref_from_memory::<ZTHabitat>(this) }.recreate_oas();
        }
    }

    /// Release-safe paths back to real vanilla for the Stage 1 leaf-getter comparison test - see
    /// [`is_right_salinity_real`] for why `.original()` cannot be used here.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_water_level_real(this: *const u32) -> u32 {
        unsafe { GET_WATER_LEVEL_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_tank_height_real(this: *const u32) -> u32 {
        unsafe { GET_TANK_HEIGHT_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_water_purity_real(this: *const u32) -> u32 {
        unsafe { GET_WATER_PURITY_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn is_pure_water_real(this: *const u32) -> bool {
        unsafe { IS_PURE_WATER_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn is_extremely_murky_water_real(this: *const u32) -> bool {
        unsafe { IS_EXTREMELY_MURKY_WATER_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_platform_pos_real(this: *const u32, out: *const i32) -> *const i32 {
        unsafe { GET_PLATFORM_POS_DETOUR.call(this, out) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_ladder_base_outside_pos_real(this: *const u32, out: *const i32) -> *const i32 {
        unsafe { GET_LADDER_BASE_OUTSIDE_POS_DETOUR.call(this, out) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_ladder_top_outside_pos_real(this: *const u32, out: *const i32) -> *const i32 {
        unsafe { GET_LADDER_TOP_OUTSIDE_POS_DETOUR.call(this, out) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_ladder_base_inside_pos_real(this: *const u32, out: *const i32) -> *const i32 {
        unsafe { GET_LADDER_BASE_INSIDE_POS_DETOUR.call(this, out) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_ladder_top_inside_pos_real(this: *const u32, out: *const i32) -> *const i32 {
        unsafe { GET_LADDER_TOP_INSIDE_POS_DETOUR.call(this, out) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn remove_dead_sparkles_real(this: *const u32, elapsed: u32) {
        unsafe { REMOVE_DEAD_SPARKLES_DETOUR.call(this, elapsed) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn add_random_sparkle_real(this: i32) {
        unsafe { ADD_RANDOM_SPARKLE_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn remove_all_sparkles_real(this: *const u32) {
        unsafe { REMOVE_ALL_SPARKLES_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn remove_all_sparkles_by_id_real(this: *const u32) {
        unsafe { REMOVE_ALL_SPARKLES_BY_ID_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn remove_water_ripples_real(this: *const u32) {
        unsafe { REMOVE_WATER_RIPPLES_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn add_water_ripples_real(this: *const u32) {
        unsafe { ADD_WATER_RIPPLES_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn add_water_ripple_real(this: *const u32, tile: *const u32, direction: u32, wall: *const u32) {
        unsafe { ADD_WATER_RIPPLE_DETOUR.call(this, tile, direction, wall) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_min_depth_real(this: *const u32) -> i32 {
        unsafe { GET_MIN_DEPTH_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_cost_to_change_height_real(this: *const u32, from_height: i32, to_height: i32) -> f32 {
        unsafe { GET_COST_TO_CHANGE_HEIGHT_DETOUR.call(this, from_height, to_height) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn update_adjustment_costs_real(this: *const u32) {
        unsafe { UPDATE_ADJUSTMENT_COSTS_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn clean_water_amount_real(this: *const u32, amount: i32) {
        unsafe { CLEAN_WATER_AMOUNT_DETOUR.call(this, amount) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_water_level_real(this: *const u32, level: i32) {
        unsafe { SET_WATER_LEVEL_DETOUR.call(this, level) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_water_purity_real(this: *const u32, value: i32) {
        unsafe { SET_WATER_PURITY_DETOUR.call(this, value) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_salinity_real(this: *const u32, water_type: i32) -> u32 {
        unsafe { SET_SALINITY_DETOUR.call(this, water_type) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_base_terrain_type_real(this: *const u32, terrain_type: i32) {
        unsafe { SET_BASE_TERRAIN_TYPE_DETOUR.call(this, terrain_type) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_base_level_real(this: *const u32, level: i32) {
        unsafe { SET_BASE_LEVEL_DETOUR.call(this, level) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn fill_real(this: *const u32) {
        unsafe { FILL_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn drain_real(this: *const u32) {
        unsafe { DRAIN_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn check_entities_for_fill_real(this: *const u32) -> u32 {
        unsafe { CHECK_ENTITIES_FOR_FILL_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn check_entities_for_drain_real(this: *const u32) -> i32 {
        unsafe { CHECK_ENTITIES_FOR_DRAIN_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn remove_illegal_entities_real(this: *const u32, mode: i32, confirm: bool) -> bool {
        unsafe { REMOVE_ILLEGAL_ENTITIES_DETOUR.call(this, mode, confirm) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn can_be_in_tank_real(entity: u32) -> u8 {
        unsafe { CAN_BE_IN_TANK_DETOUR.call(entity) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn is_illegal_entity_real(entity: *const u32, mode: i32) -> u32 {
        unsafe { IS_ILLEGAL_ENTITY_DETOUR.call(entity, mode) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_all_entities_real(this: *const u32, container: *const std::ffi::c_void) {
        unsafe { GET_ALL_ENTITIES_DETOUR.call(this, container) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn get_illegal_entities_real(this: *const u32, container: *const std::ffi::c_void, mode: i32) {
        unsafe { GET_ILLEGAL_ENTITIES_DETOUR.call(this, container, mode) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn clear_wall_vector_real(this: i32) {
        unsafe { CLEAR_WALL_VECTOR_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn update_tank_wall_info_real(this: *const u32) {
        unsafe { UPDATE_TANK_WALL_INFO_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn add_tank_wall_real(this: *const u32, wall: *const u32) {
        unsafe { ADD_TANK_WALL_DETOUR.call(this, wall) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn remove_owned_transients_real(this: *const u32) {
        unsafe { REMOVE_OWNED_TRANSIENTS_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn restore_owned_transients_real(this: *const u32) {
        unsafe { RESTORE_OWNED_TRANSIENTS_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn validate_filter_position_real(this: *const u32) {
        unsafe { VALIDATE_FILTER_POSITION_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn level_all_tiles_real(this: *const u32) {
        unsafe { LEVEL_ALL_TILES_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn reclaim_tiles_real(this: *const u32) {
        unsafe { RECLAIM_TILES_DETOUR.call(this) }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn do_depth_suitability_real(this: *const u32, new_height: i32) {
        unsafe { DO_DEPTH_SUITABILITY_DETOUR.call(this, new_height) }
    }

    /// Release-safe path back to real vanilla for the live comparison test - see [`is_right_salinity_real`]
    /// for why `.original()` cannot be used here.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_tank_height_real(this: *const u32, height: i32) {
        unsafe { SET_TANK_HEIGHT_DETOUR.call(this, height) }
    }

    /// Release-safe path back to real vanilla for the live comparison test - see `ztawardmgr`'s
    /// `call_real` doc comments for why `.original()` cannot be used here.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn is_right_salinity_real(this: *const u32, animal_type: *const u32) -> bool {
        unsafe { IS_RIGHT_SALINITY_DETOUR.call(this, animal_type) }
    }

    /// Release-safe path back to real vanilla for the live comparison test - see
    /// [`is_right_salinity_real`] above for why `.original()` cannot be used here.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn update_real(this: *const u32, elapsed: u32) {
        unsafe { UPDATE_DETOUR.call(this, elapsed) }
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ZTTankExhibit detours: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_z_is_floored_at_gate_in_then_capped_at_platform() {
        assert_eq!(ZTTankExhibit::clamp_outside_z(5, 10, 100), 10);
        assert_eq!(ZTTankExhibit::clamp_outside_z(50, 10, 100), 50);
        assert_eq!(ZTTankExhibit::clamp_outside_z(500, 10, 100), 100);
        // The cap wins over the floor when the platform sits below the gate-in height.
        assert_eq!(ZTTankExhibit::clamp_outside_z(5, 10, 8), 8);
    }

    #[test]
    fn min_depth_step_keeps_the_signed_maximum_after_subtracting_the_offset() {
        assert_eq!(ZTTankExhibit::min_depth_step(2, 10, 3), 7);
        assert_eq!(ZTTankExhibit::min_depth_step(2, 4, 3), 2);
        assert_eq!(ZTTankExhibit::min_depth_step(-9, -5, 0), -5);
    }

    #[test]
    fn height_delta_below_initial_charges_only_the_part_above_it() {
        // initial = 4: a move that stays at or below it is free, one that crosses it pays the excess.
        assert_eq!(ZTTankExhibit::billable_height_delta(2, 3, 4), 0);
        assert_eq!(ZTTankExhibit::billable_height_delta(4, 5, 4), 1);
        assert_eq!(ZTTankExhibit::billable_height_delta(2, 7, 4), 3);
        assert_eq!(ZTTankExhibit::billable_height_delta(2, 1, 4), 0);
    }

    #[test]
    fn height_delta_above_initial_credits_only_down_to_it() {
        // from 9 down to 2 with initial 4: the credit stops at the initial height.
        assert_eq!(ZTTankExhibit::billable_height_delta(9, 2, 4), -5);
        // from 9 down to 4 (not below initial): the plain delta, which is the same number.
        assert_eq!(ZTTankExhibit::billable_height_delta(9, 4, 4), -5);
        assert_eq!(ZTTankExhibit::billable_height_delta(9, 6, 4), -3);
        assert_eq!(ZTTankExhibit::billable_height_delta(9, 12, 4), 3);
    }

    #[test]
    fn height_delta_comparisons_are_signed() {
        assert_eq!(ZTTankExhibit::billable_height_delta(-5, 3, 0), 3);
        assert_eq!(ZTTankExhibit::billable_height_delta(3, -5, 0), -3);
    }

    #[test]
    fn terrain_info_cell_is_indexed_in_0x30_steps_and_null_past_the_end() {
        assert_eq!(ZTTankExhibit::terrain_info_cell(0x1000, 0x1000 + 3 * 0x30, 0), 0x1000);
        assert_eq!(ZTTankExhibit::terrain_info_cell(0x1000, 0x1000 + 3 * 0x30, 2), 0x1000 + 2 * 0x30);
        assert_eq!(ZTTankExhibit::terrain_info_cell(0x1000, 0x1000 + 3 * 0x30, 3), 0);
        // Unsigned: a negative index is far past the end.
        assert_eq!(ZTTankExhibit::terrain_info_cell(0x1000, 0x1000 + 3 * 0x30, -1i32 as u32), 0);
        assert_eq!(ZTTankExhibit::terrain_info_cell(0, 0, 0), 0);
    }

    #[test]
    fn water_level_clamps_low_then_high_signed() {
        assert_eq!(ZTTankExhibit::clamp_water_level(-4, 10), 0);
        assert_eq!(ZTTankExhibit::clamp_water_level(5, 10), 5);
        assert_eq!(ZTTankExhibit::clamp_water_level(11, 10), 10);
        // The upper bound is applied last, so a negative bound wins over the floor of zero.
        assert_eq!(ZTTankExhibit::clamp_water_level(3, -2), -2);
    }

    #[test]
    fn water_purity_clamps_high_then_low_signed() {
        assert_eq!(ZTTankExhibit::clamp_water_purity(500, 100), 100);
        assert_eq!(ZTTankExhibit::clamp_water_purity(-3, 100), 0);
        assert_eq!(ZTTankExhibit::clamp_water_purity(40, 100), 40);
        // The floor is applied last, so a negative maximum still yields zero.
        assert_eq!(ZTTankExhibit::clamp_water_purity(5, -9), 0);
    }

    #[test]
    fn message_template_substitutes_the_name() {
        assert_eq!(ZTTankExhibit::substitute_name("The water in %s is murky.", "Tank 1"), "The water in Tank 1 is murky.");
        assert_eq!(ZTTankExhibit::substitute_name("no placeholder", "x"), "no placeholder");
    }

    #[test]
    fn nonfresh_water_passes_when_category9_within_category10() {
        assert!(ZTTankExhibit::salinity_result(7, 5, 1));
    }

    #[test]
    fn nonfresh_water_fails_when_category9_exceeds_category10() {
        assert!(!ZTTankExhibit::salinity_result(5, 7, 1));
    }

    #[test]
    fn fresh_water_inverts_a_passing_comparison() {
        assert!(!ZTTankExhibit::salinity_result(7, 5, 0));
    }

    #[test]
    fn fresh_water_inverts_a_failing_comparison() {
        assert!(ZTTankExhibit::salinity_result(5, 7, 0));
    }

    #[test]
    fn equal_values_count_as_within() {
        assert!(ZTTankExhibit::salinity_result(6, 6, 1));
        assert!(!ZTTankExhibit::salinity_result(6, 6, 0));
    }

    #[test]
    fn comparison_is_signed_not_unsigned() {
        // Real vanilla compares with `SETGE`: category_10 = -1 loses to category_9 = 1 signed, but
        // would win a raw u32 comparison.
        assert!(!ZTTankExhibit::salinity_result(-1, 1, 1));
    }

    #[test]
    fn any_nonzero_water_type_skips_the_inversion() {
        assert!(ZTTankExhibit::salinity_result(7, 5, -3));
        assert!(ZTTankExhibit::salinity_result(7, 5, 2));
    }

    #[test]
    fn rise_comparison_is_signed_not_unsigned() {
        // Real vanilla compares with `JL`: a negative water level is below any target and must rise; a
        // raw u32 compare would read it as huge and skip the rise.
        assert!(ZTTankExhibit::rise_needed(-1, 5, 0));
        assert!(ZTTankExhibit::rise_needed(4, 5, 0));
        assert!(!ZTTankExhibit::rise_needed(5, 5, 0));
        assert!(!ZTTankExhibit::rise_needed(6, 5, 0));
    }

    #[test]
    fn rise_target_includes_the_settings_delta() {
        assert!(ZTTankExhibit::rise_needed(7, 5, 3));
        assert!(!ZTTankExhibit::rise_needed(8, 5, 3));
    }

    #[test]
    fn purity_timer_expires_at_one_not_zero() {
        // Real vanilla's `JLE`: `<= 0` counts as expired, `1` does not.
        assert!(ZTTankExhibit::purity_timer_expired(0));
        assert!(ZTTankExhibit::purity_timer_expired(-100));
        assert!(!ZTTankExhibit::purity_timer_expired(1));
    }

    #[test]
    fn sparkle_timer_expires_only_strictly_negative() {
        // Real vanilla's `JS` - one step stricter than the purity timer's boundary above, which is why
        // the two helpers are separate.
        assert!(!ZTTankExhibit::sparkle_timer_expired(0));
        assert!(!ZTTankExhibit::sparkle_timer_expired(1));
        assert!(ZTTankExhibit::sparkle_timer_expired(-1));
        assert!(ZTTankExhibit::sparkle_timer_expired(-100));
    }

    #[test]
    fn fill_transition_needs_zero_water_and_a_different_pending_type() {
        assert!(ZTTankExhibit::fill_transition_due(0, 1, 2));
        assert!(!ZTTankExhibit::fill_transition_due(0, 2, 2));
        assert!(!ZTTankExhibit::fill_transition_due(1, 1, 2));
        assert!(!ZTTankExhibit::fill_transition_due(-1, 1, 2));
    }

    #[test]
    fn sparkle_reload_is_unsigned_division() {
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(40_000, 4), 10_000);
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(7, 2), 3); // integer division truncates
        // Unsigned: a numerator above i32::MAX stays positive, and a count of 1 keeps it as-is (the
        // u32 result lands in the i32 field bit-for-bit).
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(0xffff_ffff, 2), 0x7fff_ffff);
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(0xffff_ffff, 1), -1);
    }

    #[test]
    fn sparkle_reload_guards_the_zero_tile_count() {
        // Documented deviation: vanilla's DIV by zero raises #DE; unreachable live, guarded to 0 here.
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(40_000, 0), 0);
    }
}
