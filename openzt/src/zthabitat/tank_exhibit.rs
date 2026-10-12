use getset::Getters;
use glam::i32::IVec3;
use openzt_detour::generated::{
    bfcategory::GET_VALUE,
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfmap::GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0,
    bftile::SNAP_TO_EDGE,
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
    zttankexhibit::{
        ADD_RANDOM_SPARKLE, CLEAN_WATER_AMOUNT, FILL, GET_ALL_ENTITIES, GET_COST_TO_CHANGE_HEIGHT, GET_LADDER_BASE_INSIDE_POS, GET_LADDER_BASE_OUTSIDE_POS, GET_LADDER_TOP_INSIDE_POS, GET_LADDER_TOP_OUTSIDE_POS, GET_MIN_DEPTH, GET_PLATFORM_POS, GET_TANK_HEIGHT,
        GET_WATER_LEVEL, GET_WATER_PURITY, IS_EXTREMELY_MURKY_WATER, IS_PURE_WATER, IS_RIGHT_SALINITY, REMOVE_DEAD_SPARKLES, SET_TANK_HEIGHT, SET_WATER_LEVEL, SET_WATER_PURITY, UPDATE, UPDATE_ADJUSTMENT_COSTS,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::habitat::ZTHabitat;
use super::support::{rb_tree_clear, walk_neighbor_tree, walk_tile_list, RVA_TANK_WALL_TYPE_CHECK_ARG};
use crate::ztmegatilemgr::entity_type_matches;
use crate::globals::{get_module_base, globals};
use crate::util::{get_from_memory, ref_from_memory, save_to_memory, LiveMemory};
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
    pad_tank3a: [u8; 3], // ----------------------- padding: 3 bytes (0x199-0x19c, alignment after `is_filled`)
    cost_rate_a: f32,     // 0x19c // Per-height-step wall cost: `updateAdjustmentCosts` sums each wall type's `getPurchaseCost` (vtable `+0xa4`) and divides by `DAT_0063af14`; `getCostToChangeHeight` scales the height delta by it.
    cost_rate_b: f32,     // 0x1a0 // Per-height-step water cost: `updateAdjustmentCosts` sets it to `owned_tile_count * (fresh|salt cost)`; `getCostToChangeHeight` adds `delta * this` only while `is_filled`.
    cost_rate_c: f32,     // 0x1a4 // `updateAdjustmentCosts` sets it to `owned_tile_count * ZTWorldMgr[+0x64]`; read only by `tankmodify::fetchValuesFromTank`, which shows it as a money amount in the tank-modify dialog.
    water_purity: i32,     // 0x1a8 // Inferred: current purity level - `update`'s countdown-expiry branch only fires while `> 0` and calls `setWaterPurity(this, water_purity - 1)` on expiry, matching the already-generated `zttankexhibit::SET_WATER_PURITY` setter's own shape (not yet decompiled to directly confirm it writes this exact offset).
    water_purity_timer: i32, // 0x1ac // Inferred: ticks down by `elapsed` each `update()` call; reloaded from `DAT_0063af04` and triggers the `water_purity` decrement above when it drops to `<= 0`.
    attached_filter_id: i32, // 0x1b0 // `ZTTankFilter` entity id attached to this tank, `-1` when none; `validateFilterPosition` resets it to `-1` when the entity is gone or `ZTTankFilter::isStillAttached` fails.
    pad_tank3b: [u8; 0x10], // ----------------------- padding: 16 bytes (0x1b4-0x1c4, not yet reverse-engineered)
    sparkle_entities_begin: u32, // 0x1c4 // Inferred: begin pointer of a real vanilla `std::vector<std::pair<i32,BFEntity*>>` - `addRandomSparkle`'s own growth path (`cls_0x43f6a9::meth_0x43f6a9` when `end == cap_end`, else in-place `*end = 0; end[1] = entity`) is the same begin/end/cap_end vector-growth shape used elsewhere in this codebase (e.g. `ZTHabitat::viewing_areas_begin`), tracking each spawned sparkle entity this tank owns.
    sparkle_entities_end: u32,  // 0x1c8
    sparkle_entities_cap_end: u32, // 0x1cc
    sparkle_entity_ids_begin: u32, // 0x1d0 // Inferred: begin pointer of a parallel `std::vector<u32>` (`msvc_std::vector_pod<>::_Insert_n` growth path) storing each spawned sparkle's own `+0x124` id field, one entry per `sparkle_entities_*` entry above.
    sparkle_entity_ids_end: u32,   // 0x1d4
    sparkle_entity_ids_cap_end: u32, // 0x1d8
    salt_water_sparkle_vtable_cache: u32, // 0x1dc // Inferred: cached `BFEntity` vtable pointer for the "twaterwv"-named entity type (`BFWorldMgr::findTypes` result, looked up once and reused), constructed via its own vtable slot `+0x24` (a virtual-constructor idiom) when `addRandomSparkle` spawns a non-freshwater sparkle.
    fresh_water_sparkle_vtable_cache: u32, // 0x1e0 // Inferred: same shape as `salt_water_sparkle_vtable_cache`, for the "frshwav"-named entity type used when `addRandomSparkle`'s freshwater branch (`current_water_type == 0`) spawns instead.
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
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, cost_rate_a) == 0x19c);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, cost_rate_b) == 0x1a0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, cost_rate_c) == 0x1a4);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, wall_ids_begin) == 0x178);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, wall_ids_end) == 0x17c);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity) == 0x1a8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, attached_filter_id) == 0x1b0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity_timer) == 0x1ac);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entities_begin) == 0x1c4);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entity_ids_begin) == 0x1d0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, salt_water_sparkle_vtable_cache) == 0x1dc);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, fresh_water_sparkle_vtable_cache) == 0x1e0);
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
    /// vanilla keeps whichever of the running value and the new one is greater), starting from `2`. The entity set is a real vanilla
    /// `std::set` (header from `PoolAlloc`, filled by the un-ported vanilla `getAllEntities`), walked in
    /// order and then freed back through `PoolAlloc`.
    pub fn get_min_depth(&self) -> i32 {
        let this = self as *const Self as *const u32;
        let depth_offset: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);

        let head = unsafe { POOLALLOC_ALLOCATE.original()(ENTITY_SET_NODE_SIZE) } as u32;
        save_to_memory(head, 0u32);
        save_to_memory(head + 0x4, 0u32);
        save_to_memory(head + 0x8, head);
        save_to_memory(head + 0xc, head);
        let mut container = [head, 0u32, 0u32];
        unsafe { GET_ALL_ENTITIES.original()(this, container.as_mut_ptr() as *const std::ffi::c_void) };

        let mut min_depth = MIN_DEPTH_INITIAL;
        for node in walk_neighbor_tree(head) {
            let entity: u32 = get_from_memory(node + 0x10);
            let tile = unsafe { BFENTITY_GET_TILE.original()(entity as *const u32) } as u32;
            let vtable: u32 = get_from_memory(entity);
            let get_depth: unsafe extern "thiscall" fn(u32, u32) -> u32 =
                unsafe { std::mem::transmute(get_from_memory::<u32>(vtable + ENTITY_GET_DEPTH_VTABLE_OFFSET)) };
            let depth = unsafe { get_depth(entity, tile) } as i32;
            min_depth = Self::min_depth_step(min_depth, depth, depth_offset);
        }

        rb_tree_clear(container.as_ptr() as u32, ENTITY_SET_NODE_SIZE);
        unsafe { POOLALLOC_DEALLOCATE.original()(head as *const u32, ENTITY_SET_NODE_SIZE) };
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
        let this = self as *const Self as *const u32;
        unsafe { SET_WATER_PURITY.original()(this, self.water_purity.wrapping_add(amount)) };
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
        let this = self as *const Self as *const u32;

        if self.is_filled {
            let rise_delta: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);
            if Self::rise_needed(self.water_level as i32, self.tank_height as i32, rise_delta) {
                unsafe { SET_WATER_LEVEL.original()(this, self.water_level as i32 + 1) };
            }
        } else if self.water_level as i32 > 0 {
            unsafe { SET_WATER_LEVEL.original()(this, self.water_level as i32 - 1) };
            if Self::fill_transition_due(self.water_level as i32, self.current_water_type, self.pending_water_type) {
                write_live!(self, current_water_type, self.pending_water_type);
                self.update_adjustment_costs();
                unsafe { FILL.original()(this) };
            }
        }

        if self.water_level as i32 > 0 && self.water_purity > 0 {
            let purity_before = self.water_purity;
            let timer = self.water_purity_timer.wrapping_sub(elapsed as i32);
            write_live!(self, water_purity_timer, timer);
            if Self::purity_timer_expired(timer) {
                let reload: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_PURITY_TIMER_RELOAD_RVA);
                write_live!(self, water_purity_timer, reload);
                unsafe { SET_WATER_PURITY.original()(this, purity_before - 1) };
            }
        }

        unsafe { REMOVE_DEAD_SPARKLES.original()(this, elapsed) };

        let sparkle_timer = self.sparkle_spawn_timer.wrapping_sub(elapsed as i32);
        write_live!(self, sparkle_spawn_timer, sparkle_timer);
        if Self::sparkle_timer_expired(sparkle_timer) {
            unsafe { ADD_RANDOM_SPARKLE.original()(this as i32) };
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
