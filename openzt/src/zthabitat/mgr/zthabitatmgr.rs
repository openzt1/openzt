use openzt_detour::{
    generated::{
        bfaimgr::CHECK_PATH as BFAIMGR_CHECK_PATH,
        bfentity::{DIR_TO_SET as BFENTITY_DIR_TO_SET, GET_TILE as BFENTITY_GET_TILE, SET_WORLD_POS as BFENTITY_SET_WORLD_POS
    },bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0, IS_CLOSE_DIRECTION as BFMAP_IS_CLOSE_DIRECTION, WORLD_TO_TILE, WORLD_TO_VIRTUAL_0},
        bftile::{
            ADD_EDGE as BFTILE_ADD_EDGE, GET_CORNER_ELEVATION as BFTILE_GET_CORNER_ELEVATION, IS_GENTLY_SLOPED as BFTILE_IS_GENTLY_SLOPED,
            IS_IN_ZOO as BFTILE_IS_IN_ZOO, REMOVE_EDGE as BFTILE_REMOVE_EDGE, SNAP_TO_EDGE, VALIDATE_POSITIONS as BFTILE_VALIDATE_POSITIONS,
        },
        bfuimgr::DISPLAY_MESSAGE_1 as BFUIMGR_DISPLAY_MESSAGE_1,
        bfunit::GET_PATH_COST as BFUNIT_GET_PATH_COST,
        bfworldmgr::{ADD_ENTITY as BFWORLDMGR_ADD_ENTITY, GET_TYPE as BFWORLDMGR_GET_TYPE, REMOVE_ENTITY as BFWORLDMGR_REMOVE_ENTITY, VERIFY_ENTITY_0 as BFWORLDMGR_VERIFY_ENTITY_0},
        gxmixer::SET_SET as GXMIXER_SET_SET,
        msvc_std_listuint::INSERT as MSVC_LIST_UINT_INSERT,
        msvc_std_vectorbyte::VECTORBYTE,
        poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4},
        standalone::{IS_ZOO_GATE, IS_ZOO_WALL, MEMMOVE, OPERATOR_DELETE, OPERATOR_NEW, TILE_WITHIN_AVA, WRITE_BYTES_TO_FILE},
        ztanimal::{CAN_SERVICE, IS_HUNGRY_AND_FOODLESS, IS_SICKLY},
        ztfence::{IS_WORTH_FIXING, JUMP_TILE_EDGE as ZTFENCE_JUMP_TILE_EDGE, MAKE_FENCE as ZTFENCE_MAKE_FENCE, MAKE_GATE as ZTFENCE_MAKE_GATE},
        zthabitat::{
            ADD_AMPHIBIOUS_NEIGHBOR, ADD_CLEAR_TILES, ADD_HABITAT_TILES, ADD_SHOW_NEIGHBOR, ADD_SHOW_PORTAL, CLEAR_AMPHIBIOUS_NEIGHBORS,
            CLEAR_SHOW_NEIGHBORS, CONSTRUCTOR as ZTHABITAT_CONSTRUCTOR, GENERATE_FACES, GET_ALL_ANIMALS, GET_AMOUNT_KEEPER_FOOD,
            GET_ANIMALS, GET_AVG_ANIMAL_HAPPINESS, GET_EVENTS, GET_FOOD_TO_LEAVE, GET_GATE, GET_GATE_TILE_IN as ZTHABITAT_GET_GATE_TILE_IN,
            GET_NEAREST_SICK_ANIMAL, GET_NUM_ANGRY_ANIMALS, GET_NUM_ANIMALS, GET_NUM_HUNGRY_FOODLESS_ANIMALS, GET_NUM_KEEPERS,
            GET_NUM_SICKLY_ANIMALS, GET_NUM_SICK_ANIMALS, GET_OUTERMOST_TANK as ZTHABITAT_GET_OUTERMOST_TANK, GET_RANDOM_ANIMAL,
            GET_RANDOM_CLEAR_TILE_AHEAD, GET_RANDOM_TILE, GET_RANDOM_TILE_IN_DIRECTION, GET_SHOW_PORTAL, GET_SIZE, GET_SPECIES_ANIMALS,
            GET_SPECIES_RATING, GET_SURROUNDING_SPECIES, GET_VIEWING_AREAS_WITH_GUESTS, HAS_BLDG, HILITE_AMPHIBIOUS_NEIGHBORS,
            HILITE_SHOW_NEIGHBORS, IS_BEING_SERVICED, NEEDS_SERVICE, RECALCULATE_CHARACTERISTICS, REMOVE_HABITAT_TILES, REMOVE_SPECIES,
            REMOVE_VIEWING_AREAS, RESET_UNIT_AI, RESIZE as ZTHABITAT_RESIZE, REVISE_SPECIES_LIST, SAVE as ZTHABITAT_SAVE, SEND_EVENT,
            SEND_MAINT_WORKER_CLEANUP_EVENTS, SET_DETERIORATION as ZTHABITAT_SET_DETERIORATION,
            SET_DIRTY_CHARACTERISTICS as ZTHABITAT_SET_DIRTY_CHARACTERISTICS, SET_NAME as ZTHABITAT_SET_NAME, SET_TIME_LAST_SERVICED,
            TRIGGER_DEATH_ARRIVED, UPDATE_PORTALS, VALIDATE_POSITIONS,
        },
        zthabitatmgr::{
            ADD_HABITAT as ZTHABITATMGR_ADD_HABITAT, AFTER_ENTITY_CHANGE, BEFORE_ENTITY_CHANGE, BREAK_AMPHIBIOUS_CONNECTION, CAN_FIND_PATH,
            CAN_SEE_HABITAT_FROM_BUILDING, CAN_SEE_SHOW_FROM_BUILDING, CHECK_AMPHIBIOUS_NEIGHBOR, CHECK_ENTER_HABITAT, CHECK_EXHIBIT_MORPH,
            CHECK_GATE, CHECK_SHOW_NEIGHBOR, CREATE_DOUBLE_FENCE, CREATE_HABITAT as ZTHABITATMGR_CREATE_HABITAT, DECREMENT_HABITAT_NUM,
            DO_SHOW_CHECK, DO_TANK_CHECK, ENTER_NEW_MONTH, ENTITY_ABOUT_TO_BE_PLACED, ENTITY_ABOUT_TO_BE_REMOVED, ENTITY_PLACED, ENTITY_REMOVED,
            FENCE_PLACED, FENCE_REMOVED, FENCE_REPLACED, FILL_ZOO_EXTERIOR, FIND_BEST_PLACE_FOR_GATE, FIND_BETTER_GATES_FOR_NEIGHBORS,
            FORMAT_HABITAT_MESSAGE, GET_AVERAGE_HABITAT_ATTRACTIVENESS, GET_HABITAT, GET_NEEDY_NESTED_TANK, GET_NEXT_FENCE_PAIR, GET_NUM_FAMILIES,
            GET_NUM_SPECIES, GET_OUTERMOST_TANK, GET_TANK, GET_ZOO_ENTRANCE_TILE, HABITAT_SEEN_FROM_BUILDING, HABITAT_TILE_CHANGED, LEADS_TO,
            MARK_ZOO_EXTERIOR, MERGE_TANKS, MORPH_EXHIBIT, NAME_HABITAT, PATH_PLACED as ZTHABITATMGR_PATH_PLACED,
            PATH_REMOVED as ZTHABITATMGR_PATH_REMOVED, PLACE_GATE, RECALCULATE_DETERIORATION, REMOVE_HABITAT_0, REPLACE_FENCE_WITH_GATE,
            REPLACE_GATE, REPLACE_GATE_WITH_FENCE, SCENERY_ENTITY_CHANGE, SNAP_TANK_WALLS_INWARD, SPLIT_TANK, SPLIT_TANK_INTO_LAND,
            TERRAIN_ABOUT_TO_BE_CHANGED, TERRAIN_CHANGED, TERRAIN_TILE_CHANGED, UNHIGHLIGHT_HABITAT, UPDATE as ZTHABITATMGR_UPDATE,
            UPDATE_AMPHIBIOUS_NEIGHBORS_0, UPDATE_AMPHIBIOUS_NEIGHBORS_1, UPDATE_GATES, UPDATE_SHOW_NEIGHBORS_0, UPDATE_SHOW_NEIGHBORS_1,
        },
        zttankexhibit::{
            ADD_TANK_WALL as ZTTANKEXHIBIT_ADD_TANK_WALL, CLEAR_WALL_VECTOR as ZTTANKEXHIBIT_CLEAR_WALL_VECTOR,
            CONSTRUCTOR as ZTTANKEXHIBIT_CONSTRUCTOR, FILL as ZTTANKEXHIBIT_FILL,
            REMOVE_ILLEGAL_ENTITIES as ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES, UPDATE_TANK_INFO as ZTTANKEXHIBIT_UPDATE_TANK_INFO,
        },
        ztmapview::ADD_UNDO_ACTION,
        zttankwall::SET_IS_COMBINED_CONNECTOR,
        ztui_general::GET_MAPVIEW as ZTUI_GENERAL_GET_MAPVIEW,
        ztui_habitatinfo::ADD_HABITAT as ZTUI_HABITATINFO_ADD_HABITAT,
        ztui_showpanel::SET_EXHIBIT,
        ztviewingarea::{
            ADD_TILE as ZTVIEWINGAREA_ADD_TILE, CONSTRUCTOR as ZTVIEWINGAREA_CONSTRUCTOR, DESTRUCTOR as ZTVIEWINGAREA_DESTRUCTOR,
            GET_EWEXTENT as ZTVIEWINGAREA_GET_EWEXTENT, GET_NSEXTENT as ZTVIEWINGAREA_GET_NSEXTENT,
            RECALCULATE_CHARACTERISTICS as ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS, REMOVE_TILE as ZTVIEWINGAREA_REMOVE_TILE,
            UPDATE_AMBIENTS as ZTVIEWINGAREA_UPDATE_AMBIENTS,
        },
        ztvisibilitytesting::TEST_LOS,
        ztworldmgr::{
            PLAY_FROWN_SOUND as ZTWORLDMGR_PLAY_FROWN_SOUND, PLAY_SMILE_SOUND as ZTWORLDMGR_PLAY_SMILE_SOUND,
            UPDATE_SHOW_ASSOCIATIONS as ZTWORLDMGR_UPDATE_SHOW_ASSOCIATIONS,
        },
    },
    FunctionDef,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use crate::{
    globals::{get_module_base, globals},
    string_registry::load_string_by_id,
    util::{get_from_memory, low_byte_bool, mut_from_memory, ref_from_memory, save_to_memory, ZTArray, ZTString},
    ztmapview::BFTile,
    ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG},
    ztshow::{type_check, RVA_ANIMAL_TYPE_CHECK},
    ztworldmgr::{Direction, IVec3},
};
use super::super::habitat::ZTHabitat;
use super::super::support::*;

#[derive(Debug)]
#[repr(C)]
pub struct ZTHabitatMgr {
    pub vtable: u32,                       // 0x000
    pub pad1: [u8; 0x04],                  // ----------------------- padding: 4 bytes
    pub map_size_x: u32,                   // 0x008
    pub map_size_y: u32,                   // 0x00c
    pub zoo_entrance_x: u32,               // 0x010
    pub zoo_entrance_y: u32,               // 0x014
    pub pending_habitat_ptr: u32,          // 0x018 // `ZTHabitatMgr::addHabitat`'s own "deferred" slot - a habitat whose `unknown_flag_0x2c` is set gets stashed here instead of appended to `exhibit_array` (confirmed directly via `.asm`'s `MOV dword ptr [ESI+0x18], EAX` on that branch). Almost certainly the single, always-present "world" habitat (unclaimed/exterior tiles): `getAverageHabitatAttractiveness` skips every `exhibit_array` entry with the same `unknown_flag_0x2c` flag set, and `enterNewMonth` rotates this field's own donation/upkeep totals unconditionally, with no null check - real vanilla never treats it as merely a rare/optional queue slot. See `ZTHabitatMgr::enter_new_month`.
    pub exhibit_array: ZTArray<ZTHabitat>, // 0x01c (0xc bytes)
    pub other_array_start: u32,            // 0x028 //TODO: Use ZTArray; Seems to be some kind of mapping from BFTile to ZTHabitat or a ZTHabitat index
    pub other_array_end: u32,              // 0x02c
    pub other_array_buffer_end: u32,       // 0x030
    // `ZTHabitatMgr::updateGates` (still real/un-ported, called through by `Self::update` every tick -
    // see that method's own doc comment) is these 8 fields' only confirmed reader
    // (`ZTHabitatMgr_updateGates.c`); `ZTHabitatMgr::fenceRemoved` (`Self::fence_removed`) is their only
    // confirmed writer - a deferred-request queue, not scalar state: `pending_place_gate_*` mirrors
    // `Self::place_gate`'s own 4 arguments exactly (consumed by a real `placeGate` call, then zeroed);
    // `pending_replace_fence_ptr`/`_habitat_ptr`/`_tile_ptr`/`_rotation` are consumed by a real
    // `replaceFenceWithGate` call that, on success, writes `_tile_ptr`/`_rotation` into
    // `_habitat_ptr`'s own `entrance_tile_ptr`/`entrance_rotation` (`+0x8c`/`+0x90`) directly rather than
    // going through `ZTHabitat::moveGateTo`.
    pub pending_place_gate_habitat_ptr: u32,   // 0x034
    pub pending_place_gate_seed_tile_ptr: u32, // 0x038
    pub pending_place_gate_resize_tile_ptr: u32, // 0x03c
    pub pending_place_gate_hint_tile_ptr: u32, // 0x040
    pub pending_replace_fence_ptr: u32,    // 0x044
    pub pending_replace_habitat_ptr: u32,  // 0x048
    pub pending_replace_tile_ptr: u32,     // 0x04c
    pub pending_replace_rotation: u32,     // 0x050 // `0xffffffff` is a real sentinel value here, same as elsewhere in this file.
    pub pending_gate_fence_ptr: u32,       // 0x054 // Stashed by `ZTHabitatMgr::removeHabitat` when the habitat being deleted owns a real gate fence still present at its entrance tile (`ZTHabitatMgr_removeHabitat_0.c`'s own `this->mbr_0x54 = dVar1` branch), cleared and converted back to a plain fence by `ZTHabitatMgr::replaceGate` - see `Self::replace_gate`.
    pub popularity_scale_factor: f32,      // 0x058
    pub pad4: [u8; 0xc],                   // ----------------------- padding: 12 bytes (0x05c-0x068, not yet reverse-engineered)
    pub loaded_marker: u32,                // 0x068 // Set to 1 by both the constructor and the tail of `load` (every branch, success or failure past the header); `save` writes it back out raw. Real meaning beyond "always observed as 1" unconfirmed.
    pub unknown_flag_0x6c: u8,              // 0x06c // Set to 1 unconditionally by the constructor (`ZTHabitatMgr_ZTHabitatMgr.c`'s own `pBVar6->field_0x6c = 1`, confirmed to be `this`-relative despite the decompile's misleading `pBVar6` naming - see `scenery_entity_change_suspended`'s own note); zeroed unconditionally by `ZTHabitatMgr::recalculateDeterioration` (see `Self::recalculate_deterioration`) - no reader confirmed anywhere in the corpus yet, so real meaning still unknown.
    pub scenery_entity_change_suspended: u8, // 0x06d // Zeroed by the constructor; gates the entire body of `ZTHabitatMgr::sceneryEntityChange` when set (see `Self::scenery_entity_change`) - not written anywhere in this pass's own scope, so always clear/no-op for the functions ported here. Not part of `save`'s own persisted byte layout (`ZTHabitatMgr_save.c` stops at `field_0x68`/`loaded_marker`), confirming these are pure runtime fields.
    pub pad5: [u8; 0x2],                    // ----------------------- padding: 2 bytes (0x06e-0x070; the ctor also zeroes 0x06e, purpose/reader unconfirmed - 0x06f unconfirmed)
}


impl ZTHabitatMgr {
    // fn get_tank(tile: &BFTile) -> Option<ZTHabitat> {

    // }

    /// The manager's own `exhibit_array` (every habitat in the loaded zoo) - exposed read-only so callers
    /// outside this module (e.g. live reimplementation-tests that need to scan real habitats for one
    /// matching some predicate, like a tank exhibit with `water_level() == 0`) can enumerate it via
    /// `ZTArray`'s own public `len`/`get`/`get_ptr`.
    pub fn exhibit_array(&self) -> &ZTArray<ZTHabitat> {
        &self.exhibit_array
    }

    pub fn get_habitat_by_tile(&self, tile: &BFTile) -> Option<ZTHabitat> {
        self.get_habitat(tile.pos.x, tile.pos.y)
    }

    /// Address of the tile-ownership grid cell for `(pos_x, pos_y)` - the same cell [`Self::get_habitat_ptr`]
    /// reads - or `None` when out of range. Exposed so [`ZTHabitat::remove_habitat_tiles`] can clear a
    /// specific cell's backpointer in place, matching real vanilla's own `removeHabitatTiles` (which
    /// zeroes the raw grid cell directly, not through any setter).
    ///
    /// Ports `ZTHabitatMgr::getHabitat`'s (`ZTHabitatMgr_getHabitat.c`) own address math faithfully,
    /// including its bounds checks: `other_array_start`/`other_array_end`/`other_array_buffer_end` is the
    /// outer, per-x-column array (`0xc` bytes/entry - a `start_ptr, end_ptr` pair per column); each
    /// column's own buffer is a flat, `0x28`-byte-strided run of slots whose first 4 bytes are the
    /// `ZTHabitat*` occupying that tile (0 when empty). Real vanilla returns 0 for any out-of-range
    /// `pos_x`/`pos_y` rather than reading past either array - this previously read unconditionally,
    /// which could over-read past a short/empty column for tiles outside the loaded map.
    ///
    /// Each `0x28`-byte row holds more than the single occupant pointer at offset `0`:
    /// `ZTHabitatMgr_habitatTileChanged.asm` reads 8 more `u32` slots at offsets `0x4..0x24` (stride 4)
    /// from the same row address this returns, each apparently a cached nearby-habitat pointer (see
    /// `Self::habitat_tile_changed`) - purpose of those 8 slots beyond "something `habitatTileChanged`
    /// marks dirty" not otherwise confirmed. Offset `0x24` itself (the row's 10th and final 4-byte slot's
    /// low byte) is [`Self::pathfinding_flags`]/[`Self::clear_pathfinding`]'s own visited-bit byte, shared
    /// by [`Self::can_find_path`]'s bidirectional search.
    pub fn get_habitat_cell_addr(&self, pos_x: i32, pos_y: i32) -> Option<u32> {
        if pos_x < 0 || pos_y < 0 {
            return None;
        }
        let column_count = (self.other_array_end - self.other_array_start) / 0xc;
        if pos_x as u32 >= column_count {
            return None;
        }
        let column_entry_ptr = self.other_array_start + pos_x as u32 * 0xc;
        let column_start = get_from_memory::<u32>(column_entry_ptr);
        let column_end = get_from_memory::<u32>(column_entry_ptr + 4);
        let row_count = (column_end - column_start) / 0x28;
        if pos_y as u32 >= row_count {
            return None;
        }
        Some(column_start + pos_y as u32 * 0x28)
    }

    /// Raw pointer variant of `get_habitat`, for callers that need the address itself (e.g.
    /// `ZTThought::populate`, which stores it verbatim as `habitat_ptr`) rather than a copy of the
    /// pointed-to `ZTHabitat`. Returns `0` (null) if no habitat occupies the tile, matching vanilla's
    /// own `ZTHabitatMgr::getHabitat` return value directly.
    pub fn get_habitat_ptr(&self, pos_x: i32, pos_y: i32) -> u32 {
        match self.get_habitat_cell_addr(pos_x, pos_y) {
            Some(addr) => get_from_memory::<u32>(addr),
            None => 0,
        }
    }

    // TODO: Should return Option<ZTExhibit> where ZTExhibit is a enum of ZTHabitat or ZTTankExhibit
    pub fn get_habitat(&self, pos_x: i32, pos_y: i32) -> Option<ZTHabitat> {
        let ptr = self.get_habitat_ptr(pos_x, pos_y);

        // TODO: Check vtable ptr and return ZTHabitat or ZTTankExhibit?

        if ptr != 0 {
            return Some(get_from_memory::<ZTHabitat>(ptr));
        }

        None
    }

    /// Ports `ZTHabitatMgr::getZooEntranceTile` (`ZTHabitatMgr_getZooEntranceTile.c`): the entrance
    /// tile's address (`GLOBAL_ZTWorldMgr`'s own `tile_array` indexed by `zoo_entrance_x`/`_y`, via
    /// [`crate::ztworldmgr::ZTWorldMgr::get_tile_ptr`]) if both coordinates are non-negative and within
    /// the live map's own bounds, else `0`. `zoo_entrance_x`/`_y` are stored as `u32` here but real
    /// vanilla treats them as signed `int` (`-1` is the "unset" sentinel) - the `< 0` checks below read
    /// the same bit pattern correctly regardless.
    pub fn get_zoo_entrance_tile_ptr(&self) -> u32 {
        let x = self.zoo_entrance_x as i32;
        let y = self.zoo_entrance_y as i32;
        if x < 0 || y < 0 {
            return 0;
        }
        let world = globals().ztworldmgr();
        if x >= world.map_x_size as i32 || y >= world.map_y_size as i32 {
            return 0;
        }
        world.get_tile_ptr(x as u32, y as u32)
    }

    /// Ports `ZTHabitatMgr::getAverageHabitatAttractiveness` (`ZTHabitatMgr_getAverageHabitatAttractiveness.c`):
    /// the mean [`ZTHabitat::get_attractiveness`] over every `exhibit_array` entry whose own
    /// `unknown_flag_0x2c` is clear (skips the special "world" habitat - see [`Self::pending_habitat_ptr`]'s
    /// own doc comment), truncating toward zero (real vanilla's `iVar3 / iVar2` is a plain signed integer
    /// divide); `0` if there are no such habitats.
    pub fn get_average_habitat_attractiveness(&self) -> i32 {
        let mut sum = 0i32;
        let mut count = 0i32;
        for i in 0..self.exhibit_array.len() {
            let ptr = self.exhibit_array.get_ptr(i);
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
            if habitat.unknown_flag_0x2c == 0 {
                sum += habitat.get_attractiveness();
                count += 1;
            }
        }
        if count != 0 {
            sum / count
        } else {
            0
        }
    }

    /// Shared driver for [`Self::get_num_families`]/[`Self::get_num_species`]: the count of distinct
    /// `u32` catalog-entry ids at `catalog_id_offset` (`0x1e4` for family, `0x1ec` for species; see
    /// `ZTHabitatMgr_getNumFamilies.c`/`_getNumSpecies.c`) across every `ZTHabitat::species_list` entry,
    /// over every habitat in `exhibit_array`. Real vanilla builds this same distinct-id count by
    /// inserting into a real `std::set<int>` (an RB-tree) as it walks; a Rust `HashSet` reaches the same
    /// count without needing to reproduce that container's own node allocation.
    pub(crate) fn distinct_species_catalog_ids(&self, catalog_id_offset: u32) -> usize {
        let mut ids = HashSet::new();
        for i in 0..self.exhibit_array.len() {
            let ptr = self.exhibit_array.get_ptr(i);
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
            for entry_ptr in habitat.species_list() {
                ids.insert(get_from_memory::<u32>(entry_ptr + catalog_id_offset));
            }
        }
        ids.len()
    }

    /// Ports `ZTHabitatMgr::getNumFamilies` - see [`Self::distinct_species_catalog_ids`].
    pub fn get_num_families(&self) -> i32 {
        self.distinct_species_catalog_ids(0x1e4) as i32
    }

    /// Ports `ZTHabitatMgr::getNumSpecies` - see [`Self::distinct_species_catalog_ids`].
    pub fn get_num_species(&self) -> i32 {
        self.distinct_species_catalog_ids(0x1ec) as i32
    }

    /// Ports `ZTHabitatMgr::enterNewMonth` (`ZTHabitatMgr_enterNewMonth.c`): for every `exhibit_array`
    /// entry, and finally for [`Self::pending_habitat_ptr`] itself (real vanilla reads it unconditionally,
    /// no null check - see that field's own doc comment for why this is confidently the always-present
    /// "world" habitat rather than a rarely-populated queue slot), rotates `current_donations` ->
    /// `last_donations`, `unknown_u32_2` -> `unknown_u32_3`, and `current_upkeep` -> `last_upkeep`,
    /// zeroing each `current`/leading field. The `total_*`/`unknown_u32_4` fields are untouched, matching
    /// real vanilla exactly - only the two rolling-window pairs plus `unknown_u32_2`/`_3` reset monthly.
    pub fn enter_new_month(&self) {
        for i in 0..self.exhibit_array.len() {
            rotate_month_fields(self.exhibit_array.get_ptr(i));
        }
        rotate_month_fields(self.pending_habitat_ptr);
    }

    /// Ports `ZTHabitatMgr::save` (`ZTHabitatMgr_save.c`/`.asm`, confirmed identical on both platforms
    /// modulo the macOS decompile's own endian-swap noise): map size (read from the live
    /// `GLOBAL_ZTWorldMgr`, **not** this manager's own `map_size_x`/`map_size_y` cache - real vanilla
    /// never touches those two fields here), the zoo entrance tile's position (via
    /// [`Self::get_zoo_entrance_tile_ptr`] - including its own lack of a null-tile guard before
    /// dereferencing `+0x34`/`+0x38`, "dead in practice" the same way this codebase already treats
    /// several other unguarded vanilla reads), `exhibit_array`'s length, then every exhibit's own `save`
    /// in order, and finally the raw `loaded_marker` dword.
    ///
    /// Unlike `ZTHabitat::save` (which ANDs every field's success together and keeps writing
    /// regardless, matching that function's own flat structure), a single exhibit's `save` failing here
    /// **stops the loop immediately and skips the trailing `loaded_marker` write** - real vanilla's own
    /// nested-if short-circuit, preserved because it changes which bytes actually reach the file, not
    /// just the boolean result.
    ///
    /// Dispatches each exhibit's `save` through its own real vtable slot (`+0x1c`) rather than calling
    /// [`ZTHabitat::save`] directly - transparently reaches our own detour for a plain `ZTHabitat` (once
    /// installed, the vtable slot itself points at it) or any un-detoured `ZTTankExhibit` override,
    /// exactly matching vanilla's own polymorphic dispatch without this port needing to know whether
    /// `ZTTankExhibit` overrides `save` at all.
    pub fn save(&self, file: *const i8) -> bool {
        let world = globals().ztworldmgr();
        let map_x = world.map_x_size as i32;
        let map_y = world.map_y_size as i32;
        let mut ok = write_bytes_to_file(&map_x, file);
        ok &= write_bytes_to_file(&map_y, file);

        let entrance_tile_ptr = self.get_zoo_entrance_tile_ptr();
        let entrance_x: i32 = get_from_memory(entrance_tile_ptr + 0x34);
        let entrance_y: i32 = get_from_memory(entrance_tile_ptr + 0x38);
        ok &= write_bytes_to_file(&entrance_x, file);
        ok &= write_bytes_to_file(&entrance_y, file);

        let count = self.exhibit_array.len() as u32;
        ok &= write_bytes_to_file(&count, file);

        for i in 0..self.exhibit_array.len() {
            let habitat_ptr = self.exhibit_array.get_ptr(i);
            if !unsafe { call_save_vtable_slot(habitat_ptr, file) } {
                return false;
            }
        }

        ok &= write_bytes_to_file(&self.loaded_marker, file);
        ok
    }

    /// Ports `ZTHabitatMgr::addHabitat` (`ZTHabitatMgr_addHabitat.c`/`.asm`, confirmed identical
    /// control flow on both platforms): appends `habitat_ptr` to `exhibit_array`, doubling its
    /// backing buffer (minimum `1`) through real vanilla's own `PoolAlloc::allocate`/
    /// `PoolAlloc::deallocate_n_4` when full - called through rather than reimplemented, since real
    /// vanilla itself calls these as plain function calls here (unlike most other `PoolAlloc`-backed
    /// containers in this codebase, which inline the bucket-freelist fast path at every call site -
    /// see `ztshowinfo.rs`'s own `pool_allocate` doc comment for that contrast), so calling through
    /// keeps this allocation byte-identical to vanilla for free.
    ///
    /// If `habitat_ptr`'s own `unknown_flag_0x2c` is set, the habitat is instead stashed in
    /// [`Self::pending_habitat_ptr`] and never appended to the array at all - matches vanilla's own
    /// early-return branch (`ZTHabitatMgr_addHabitat.asm`'s `.7bcc6` block) exactly.
    ///
    /// On the append path, also calls through to real vanilla `ZTUI::habitatinfo::addHabitat`, sets
    /// the shared "habitat list changed" dirty flag ([`HABITAT_LIST_DIRTY_RVA`]), and calls the
    /// unidentified no-arg helper [`FUN_0044BB5F`] - all three exactly as vanilla's own tail does.
    pub fn add_habitat(&self, habitat_ptr: u32) {
        let deferred: u8 = get_from_memory(habitat_ptr + 0x2c);
        if deferred != 0 {
            let self_addr = self as *const Self as u32;
            save_to_memory(self_addr + 0x18, habitat_ptr);
            return;
        }

        let (start, end, buffer_end) = self.exhibit_array.raw_parts();
        if end == buffer_end {
            let old_len = self.exhibit_array.len() as u32;
            let new_cap = if old_len == 0 { 1 } else { old_len * 2 };
            let new_buf = unsafe { POOLALLOC_ALLOCATE.original()(new_cap * 4) } as u32;

            for i in 0..old_len {
                let value = get_from_memory::<u32>(start + i * 4);
                if new_buf != 0 {
                    save_to_memory(new_buf + i * 4, value);
                }
            }
            if new_buf != 0 {
                save_to_memory(new_buf + old_len * 4, habitat_ptr);
            }
            // Real vanilla calls `PoolAlloc::deallocate_n_4` unconditionally here, even for
            // old_len==0 (freeing a null pointer with count 0) - matched as-is rather than skipped,
            // since PoolAlloc's real deallocate is expected to no-op on an empty range and this keeps
            // the call-through byte-for-byte faithful to `ZTHabitatMgr_addHabitat.asm`.
            unsafe { POOLALLOC_DEALLOCATE_N_4.original()(start as *const u32, old_len as i32) };
            self.exhibit_array.set_raw_parts(new_buf, new_buf + (old_len + 1) * 4, new_buf + new_cap * 4);
        } else {
            save_to_memory(end, habitat_ptr);
            self.exhibit_array.set_raw_parts(start, end + 4, buffer_end);
        }

        unsafe { ZTUI_HABITATINFO_ADD_HABITAT.original()(habitat_ptr as *const i32) };
        let base = get_module_base("zoo.exe") as u32;
        save_to_memory::<u8>(base + HABITAT_LIST_DIRTY_RVA, 1);
        unsafe { FUN_0044BB5F.original()() };
    }

    /// Ports `ZTHabitatMgr::createHabitat` (`ZTHabitatMgr_createHabitat.c`/`.asm`) as an orchestrator:
    /// constructs the new exhibit and reproduces the real branching exactly, but calls through to real
    /// vanilla (`.original()`) for every step this pass doesn't separately reimplement -
    /// `doTankCheck`/`decrementHabitatNum`/`nameHabitat`/`ZTHabitat::setName`/`ZTHabitat::resize`, the two
    /// constructors, and (tank-only) `findBetterGatesForNeighbors`/`ZTTankExhibit::updateTankInfo`/
    /// `removeIllegalEntities`/`fill`. [`Self::add_habitat`], (tank-only) [`Self::snap_tank_walls_inward`],
    /// and the four steps that already have their own genuine ports elsewhere in this file
    /// ([`ZTHabitat::set_dirty_characteristics`], [`Self::place_gate`], [`Self::update_amphibious_neighbors`],
    /// [`Self::do_show_check`]) are called directly rather than through `.original()`: each of those four
    /// addresses is separately detoured (`#[detour(...)]`) elsewhere in this file, and `FunctionDef::original()`
    /// re-enters that *same* detour in a release build (it's only a raw address cast there - see
    /// `CLAUDE.md`'s own `generated.rs` section) - meaning `.original()` here would have silently called
    /// this port's own reimplementation anyway, just via a needlessly indirect, profile-dependent path
    /// (debug builds would have reached genuine real vanilla instead, a hidden debug/release behavioral
    /// split). Found live via `debug-play` after a crash inside `call_pathfinder` surfaced the general
    /// pattern - see that function's own doc comment for the `place_gate` instance of this bug.
    ///
    /// `name_ptr` may be null, matching real call sites (`fencePlaced`/`morphExhibit` always pass a null
    /// name, taking the `nameHabitat` modal-dialog branch). `seed_tile_ptr` is always real.
    /// `resize_tile_ptr`/`gate_tile_ptr`, despite the raw decompile's signature allowing null, are
    /// **never** actually null at any real call site - every caller (`fencePlaced`, `morphExhibit`,
    /// and `placeGate`'s other direct callers `splitTank`/`splitTankIntoLand`) passes three real BFTile
    /// pointers pairwise related to `seed_tile_ptr` (confirmed by reading every `createHabitat`/
    /// `placeGate` call site in the decompile corpus) - a genuinely null resize/gate tile is not a
    /// realistic input, just something the raw signature happens to permit. Confirmed live
    /// (crash-capture) that a null `resize_tile_ptr` crashes two different ways depending on where it's
    /// read: real vanilla `resize` does not tolerate `this == 0` (a null-pointer dereference a few calls
    /// deep, not an early return) when `resize_target_ptr` resolves to null, and separately,
    /// `resize_tile_ptr` itself flows straight into `placeGate`'s own pathfinding
    /// (`BFPathFinder::findPath`/`BFUnit::getPathCost`, which dereferences both tile arguments
    /// unconditionally at `+0x3c`/`+0x40`) regardless of `resize_target_ptr`. This method still guards
    /// the `resize`/`setDirtyCharacteristics` calls on `resize_target_ptr != 0` (skipping them rather
    /// than reproducing the raw decompile's unconditional null-receiver call) since a real, unknown
    /// call site with no resize target is plausible even though none is attested in the decompile
    /// corpus - but `resize_tile_ptr` itself must still be a real tile pointer for `placeGate` to run
    /// safely; there is no equivalent guard for a null `resize_tile_ptr` reaching `placeGate`.
    ///
    /// The `local_4`/`bVar1` mapview-visibility flags (`ZTUI::general::getMapview()`'s own `+0x378`
    /// byte, `ZTMapView`'s own real meaning not otherwise reverse-engineered in this codebase - see
    /// `zthabitatmgr-implementation-plan.md`) are read as flat offsets off the returned pointer,
    /// confirmed directly via `.asm` (`MOV AL, [ECX+0x378]`) rather than the C decompile's fictional
    /// `cls_0x6314c8.cls_0x6313e4` nested-base naming.
    pub fn create_habitat(&self, seed_tile_ptr: u32, resize_tile_ptr: u32, gate_tile_ptr: u32, name_ptr: u32) {
        let mgr_ptr = self as *const Self as *const u32;

        let mut habitat_ptr = unsafe { OPERATOR_NEW.original()(0x178) } as u32;
        if habitat_ptr != 0 {
            habitat_ptr = unsafe { ZTHABITAT_CONSTRUCTOR.original()(habitat_ptr as *const u32, seed_tile_ptr as *const u32, false) } as u32;
        }

        let is_tank = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.do_tank_check();
        if is_tank {
            unsafe { DECREMENT_HABITAT_NUM.original()(mgr_ptr) };
            if habitat_ptr != 0 {
                unsafe { call_vtable_slot_with_u8(habitat_ptr, 0x18, 1) };
            }

            let base = get_module_base("zoo.exe") as u32;
            let gate_flag: u8 = get_from_memory(base + RVA_APP_INIT_SUCCESS_BASE + 0x441);
            let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() } as u32;
            let mapview_active = mapview_ptr != 0 && get_from_memory::<u8>(mapview_ptr + 0x378) != 0;
            let start_filled = gate_flag == 0 && !mapview_active;

            let mut tank_ptr = unsafe { OPERATOR_NEW.original()(0x1e8) } as u32;
            if tank_ptr != 0 {
                tank_ptr = unsafe { ZTTANKEXHIBIT_CONSTRUCTOR.original()(tank_ptr as *const u32, seed_tile_ptr as *const u32, false, start_filled) } as u32;
            }
            habitat_ptr = tank_ptr;

            let mapview_ptr2 = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() } as u32;
            let show_active = mapview_ptr2 != 0 && get_from_memory::<u8>(mapview_ptr2 + 0x378) != 0;

            self.snap_tank_walls_inward(tank_ptr);
            if !show_active {
                self.find_better_gates_for_neighbors(tank_ptr);
            }
            self.do_show_check(tank_ptr, false);
            unsafe { ZTTANKEXHIBIT_UPDATE_TANK_INFO.original()(tank_ptr as *const u32) };
            let has_illegal_entities = unsafe { ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES.original()(tank_ptr as *const u32, 1, !show_active) };
            if !has_illegal_entities {
                self.add_habitat(tank_ptr);
                let mapview_ptr3 = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() } as u32;
                if mapview_ptr3 != 0 {
                    save_to_memory::<u8>(mapview_ptr3 + 0x455, 1);
                }
                unsafe { DECREMENT_HABITAT_NUM.original()(mgr_ptr) };
                return;
            }
            unsafe { ZTTANKEXHIBIT_FILL.original()(tank_ptr as *const u32) };
        }

        if name_ptr == 0 {
            unsafe { NAME_HABITAT.original()(mgr_ptr, habitat_ptr as *const u32) };
        } else {
            unsafe { ZTHABITAT_SET_NAME.original()(habitat_ptr as *const u32, name_ptr as *const u32) };
            unsafe { DECREMENT_HABITAT_NUM.original()(mgr_ptr) };
        }
        self.add_habitat(habitat_ptr);

        let resize_target_ptr = if resize_tile_ptr == 0 {
            0
        } else {
            let x: i32 = get_from_memory(resize_tile_ptr + 0x34);
            let y: i32 = get_from_memory(resize_tile_ptr + 0x38);
            self.get_habitat_ptr(x, y)
        };
        // Guarded on a non-null receiver despite the raw decompile showing an unconditional call -
        // confirmed live (crash-capture) that real vanilla `resize` does NOT tolerate `this == 0`: it
        // reaches a null-pointer dereference at `+0x40` a few calls deep rather than early-returning,
        // contradicting this method's own earlier assumption (now corrected) that vanilla runs this
        // path constantly with a null receiver. Skipping the call when there's no resize target is
        // also the semantically sensible behavior (nothing to resize).
        if resize_target_ptr != 0 {
            unsafe { ZTHABITAT_RESIZE.original()(resize_target_ptr as *const u32, resize_tile_ptr as i32) };
            unsafe { mut_from_memory::<ZTHabitat>(resize_target_ptr) }.set_dirty_characteristics();
        }
        self.place_gate(habitat_ptr, seed_tile_ptr, resize_tile_ptr, gate_tile_ptr);
        self.update_amphibious_neighbors(habitat_ptr);
        self.do_show_check(habitat_ptr, true);
    }

    /// Ports `ZTHabitatMgr::replaceGateWithFence` (`ZTHabitatMgr_replaceGateWithFence.c`/`.asm`) - a
    /// plain free `stdcall` helper (`generated.rs`'s own signature has no `this`) despite living in the
    /// `ZTHabitatMgr::` decompile namespace, the same "misnamed as instance method" pattern
    /// `Self::highlight_habitat`/`ZTHabitat::highlight` already document elsewhere in this file. Demotes
    /// `fence_ptr` (real vanilla's own current gate) back into a plain fence via `ZTFence::makeFence` and
    /// its own vtable `+0x84` `validatePosition(true)` (confirmed via `.asm`; see
    /// [`Self::replace_fence_with_gate`]'s own doc comment for the same call), gated on
    /// [`RVA_FENCE_TYPE_CHECK_ARG`]/`fence_ptr` being non-null. Sets/clears
    /// [`GATE_CONVERSION_IN_PROGRESS_RVA`] around the conversion, matching real vanilla's own
    /// `fencePlaced`/`fenceRemoved` re-entrancy guard.
    pub fn replace_gate_with_fence(fence_ptr: u32) -> bool {
        let base = get_module_base("zoo.exe") as u32;
        save_to_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA, 1);
        if fence_ptr == 0 || !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
            return false;
        }
        unsafe {
            ZTFENCE_MAKE_FENCE.original()(fence_ptr as *const u32);
            call_vtable_slot_with_u8(fence_ptr, 0x84, 1);
        }
        save_to_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA, 0);
        true
    }

    /// Ports `ZTHabitatMgr::replaceFenceWithGate` (`ZTHabitatMgr_replaceFenceWithGate.c`/`.asm` -
    /// followed at the `.asm` level throughout: the C decompile's own struct-offset math for `param_1`'s
    /// fields is garbled, e.g. `param_1[3].field_0x69` is really the flat byte `fence_ptr+0x465`).
    /// Promotes `fence_ptr` into a gate (`ZTFence::makeGate`, its own vtable `+0x84`
    /// `validatePosition(true)`), clears a tank-wall-specific flag at `+0x465` when the fence is
    /// specifically a tank wall ([`RVA_TANK_WALL_TYPE_CHECK_ARG`]), then - if the fence's own tile is
    /// occupied by a real tank exhibit - calls through to real vanilla `ZTTankExhibit::updateTankInfo` to
    /// refresh it. Gated the same way as [`Self::replace_gate_with_fence`] on `fence_ptr` being non-null
    /// and a genuine member of the fence/wall family.
    pub fn replace_fence_with_gate(&self, fence_ptr: u32) -> bool {
        let base = get_module_base("zoo.exe") as u32;
        save_to_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA, 1);
        if fence_ptr == 0 || !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
            return false;
        }
        unsafe {
            ZTFENCE_MAKE_GATE.original()(fence_ptr as *const u32);
            call_vtable_slot_with_u8(fence_ptr, 0x84, 1);
        }
        if unsafe { entity_type_matches(fence_ptr, RVA_TANK_WALL_TYPE_CHECK_ARG) } {
            save_to_memory::<u8>(fence_ptr + 0x465, 0);
        }
        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(fence_ptr as *const u32) } as u32;
        if tile_ptr != 0 {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            let habitat_ptr = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
            if habitat_ptr != 0 && get_from_memory::<u32>(habitat_ptr) == ZTHabitat::TANK_VTABLE_PTR {
                unsafe { ZTTANKEXHIBIT_UPDATE_TANK_INFO.original()(habitat_ptr as *const u32) };
            }
        }
        save_to_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA, 0);
        true
    }

    /// Ports `ZTHabitatMgr::placeGate` (`ZTHabitatMgr_placeGate.c`/`.asm`,
    /// `zthabitatmgr-implementation-plan.md`'s longest-standing "corrupted, needs `.asm`-level work"
    /// blocker - 28 `unaff_*`/`extraout_*` register artifacts around the core candidate-gate scan loop,
    /// per `zthabitatmgr-corrupted-decompiles-handover.md`). The Windows decompile's own SSA analysis
    /// loses track of the loop's carried registers across a compiler-shared "retry via
    /// `getNextFencePair`" code block placed far outside this function's own address range (real code,
    /// not garbage - `.asm`'s own far `JMP 0x00487e1c`/`0x00488048`/`0x0048802b` targets, ~230KB away,
    /// consistent with MSVC folding an identical cold-path tail shared by several functions). Resolved
    /// here by cross-referencing macOS's own clean, uncorrupted decompile of the *same* function
    /// (`private/resources/macos-decompiles/ZTHabitatMgr_placeGate.c`) - same algorithm shape confirmed
    /// step-for-step against the Windows `.asm`'s own real register moves (see the loop body below for
    /// where each platform's naming diverges but the logic matches exactly).
    ///
    /// Real body, in order:
    /// 1. **Load-time fast path**: if a save is currently loading ([`RVA_APP_INIT_SUCCESS_BASE`]'s `+0x441` byte)
    ///    at format version > `0x14` ([`RVA_SAVE_FILE_VERSION`]), walks the load-time candidate-gate table
    ///    ([`RVA_LOAD_CANDIDATE_GATES_BEGIN`]/`_END`) for a record whose tile resolves to `habitat_ptr`
    ///    with a non-negative validity flag - returns `true` immediately if found, skipping everything
    ///    else (real vanilla treats an already-recorded gate as already placed).
    /// 2. **Undo-replay fast path**: if the live `ZTMapView` (`ZTUI_GENERAL_GET_MAPVIEW`) is currently
    ///    undoing (`+0x378` byte - confirmed via macOS's own `ZTMapView::isUndoing` call at the identical
    ///    call site), returns `true` immediately.
    /// 3. Obtains a temporary keeper unit purely to drive the pathfinding checks below
    ///    ([`Self::temp_keeper_for_pathfinding`]) - `false` if any step of that fails.
    /// 4. Seeds two candidate tiles from `seed_tile_ptr`/`resize_tile_ptr` and scans candidate fence pairs
    ///    (`checkGate` + `getNextFencePair`, both still real/un-ported) looking for a fence pair the
    ///    keeper can actually path to from the zoo entrance (a raw function-pointer call through
    ///    `GLOBAL_ZTAIMgr+0x10`, see [`call_pathfinder`]) whose *own* tile is "gently sloped" (or flat) and
    ///    whose neighbouring habitat (if any) has its own `+0x2c` flag set. The loop keeps a cheap
    ///    "closest so far" fallback while nothing better has been found; once a keeper-reachable candidate
    ///    is found (even one that fails the slope/neighbour check), that becomes the new fallback and the
    ///    cheap distance-tracking stops. Ends either by finding a fully-qualifying candidate, by
    ///    `getNextFencePair` running out of pairs, or by cycling back to the starting pair.
    /// 5. Destroys the temporary keeper, resolves the direction between the winning pair
    ///    ([`BFMAP_GET_DIRECTION_0`]) and each side's own fence slot in that direction/its opposite
    ///    ([`ZTHabitat::fence_slot_by_index`]), then picks which fence to actually convert and which tile
    ///    to hand `moveGateTo` - preferring the hint tile (`gate_hint_tile_ptr`) when both sides have a
    ///    fence and the habitat isn't a tank.
    /// 6. Converts the chosen fence into a gate ([`Self::replace_fence_with_gate`]) and moves it to the
    ///    chosen tile ([`ZTHabitat::move_gate_to`], called on `habitat_ptr` itself, not either candidate
    ///    tile - confirmed via `.asm`, the C decompile's own "`this`" for this call is mislabeled). Returns
    ///    `false` (without a message) if the conversion itself fails.
    /// 7. For select non-ideal-but-still-successful outcomes (result codes `1`/`3`/`4`/`6`), shows a toast
    ///    via [`Self::display_gate_placement_message`]; every other outcome returns silently.
    ///
    /// **Live-testing note**: like [`ZTHabitat::move_gate_to`]/[`Self::create_double_fence`], this mutates
    /// real habitat/fence/gate state (and briefly creates/destroys a live keeper unit) with no known
    /// synthetic-safe input - detoured for manual/interactive live verification, not covered by an
    /// automated live test.
    pub fn place_gate(&self, habitat_ptr: u32, seed_tile_ptr: u32, resize_tile_ptr: u32, gate_hint_tile_ptr: u32) -> bool {
        let base = get_module_base("zoo.exe") as u32;

        let load_in_progress: u8 = get_from_memory(base + RVA_APP_INIT_SUCCESS_BASE + 0x441);
        let file_version: u32 = get_from_memory(base + RVA_SAVE_FILE_VERSION);
        if load_in_progress != 0 && file_version > 0x14 {
            let begin: u32 = get_from_memory(base + RVA_LOAD_CANDIDATE_GATES_BEGIN);
            let end: u32 = get_from_memory(base + RVA_LOAD_CANDIDATE_GATES_END);
            let mut record = begin;
            while record != end {
                let x: i32 = get_from_memory(record);
                let y: i32 = get_from_memory(record + 4);
                let valid_flag: i32 = get_from_memory(record + 0x14);
                if valid_flag >= 0 && self.get_habitat_ptr(x, y) == habitat_ptr {
                    return true;
                }
                record += 0x128;
            }
        }

        let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() } as u32;
        if mapview_ptr != 0 && get_from_memory::<u8>(mapview_ptr + 0x378) != 0 {
            return true;
        }

        let infinite_cost: i32 = get_from_memory(base + RVA_INFINITE_PATH_COST);
        let zoo_entrance_tile_ptr = self.get_zoo_entrance_tile_ptr();

        let is_tank = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.is_tank();
        let Some(keeper_ptr) = self.temp_keeper_for_pathfinding(is_tank) else {
            return false;
        };

        let world_ptr = globals().ztworldmgr_ptr() as u32;
        let ai_mgr_ptr = globals().ztaimgr_ptr() as u32;
        if world_ptr != 0 && ai_mgr_ptr != 0 {
            let world = globals().ztworldmgr();
            save_to_memory::<u32>(ai_mgr_ptr + 0x18, world_ptr + 0x8);
            save_to_memory::<u32>(ai_mgr_ptr + 0x1c, world.map_y_size);
            save_to_memory::<u32>(ai_mgr_ptr + 0x20, world.map_x_size * world.map_y_size);
        }
        let pathfinder_table_addr = ai_mgr_ptr + 0x10;

        let mut cand_a = seed_tile_ptr;
        let mut cand_b = resize_tile_ptr;
        let mut best_distance = infinite_cost;
        let mut found_reachable = false;
        let mut result_code: i32 = 0;
        // Cross-paired vs. `cand_a`/`cand_b`'s own inits - confirmed via `.asm`, not a transcription slip:
        // real vanilla seeds its "winner" slots from the *other* original tile argument.
        let mut winner_a = resize_tile_ptr;
        let mut winner_b = seed_tile_ptr;

        loop {
            let mut out_distance = infinite_cost;
            let check_result = self.check_gate(
                cand_a,
                cand_b,
                keeper_ptr,
                &mut out_distance,
                true,
            );

            let mut stop = false;
            if check_result == 0 {
                if !found_reachable && out_distance < best_distance {
                    result_code = 6;
                    winner_a = cand_a;
                    winner_b = cand_b;
                    best_distance = out_distance;
                }

                let path_ok = unsafe { Self::call_pathfinder(pathfinder_table_addr, zoo_entrance_tile_ptr, cand_b, keeper_ptr) };
                if !path_ok {
                    if !found_reachable && result_code == 0 {
                        result_code = 6;
                    }
                    if !self.advance_fence_pair(habitat_ptr, &mut cand_a, &mut cand_b) {
                        return false;
                    }
                } else {
                    let tile_a = get_from_memory::<BFTile>(cand_a);
                    let tile_b = get_from_memory::<BFTile>(cand_b);
                    let slope_ok = (tile_a.unknown_byte_2 == 0 || unsafe { low_byte_bool(BFTILE_IS_GENTLY_SLOPED.original()(cand_a as *const u32)) })
                        && (tile_b.unknown_byte_2 == 0 || unsafe { low_byte_bool(BFTILE_IS_GENTLY_SLOPED.original()(cand_b as *const u32)) });
                    let neighbor_ok = slope_ok && {
                        let neighbor_habitat_ptr = self.get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
                        neighbor_habitat_ptr != 0 && get_from_memory::<u8>(neighbor_habitat_ptr + 0x2c) != 0
                    };

                    if neighbor_ok {
                        winner_a = cand_a;
                        winner_b = cand_b;
                        result_code = 0;
                        stop = true;
                    } else {
                        winner_a = cand_a;
                        winner_b = cand_b;
                        result_code = 5;
                        best_distance = out_distance;
                        found_reachable = true;
                        if !self.advance_fence_pair(habitat_ptr, &mut cand_a, &mut cand_b) {
                            result_code = 0;
                            stop = true;
                        }
                    }
                }
            } else {
                if !found_reachable && result_code == 0 {
                    result_code = check_result;
                }
                if !self.advance_fence_pair(habitat_ptr, &mut cand_a, &mut cand_b) {
                    return false;
                }
            }

            if stop || (cand_a == seed_tile_ptr && cand_b == resize_tile_ptr) {
                break;
            }
        }

        Self::destroy_temp_keeper(keeper_ptr);

        let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(winner_a as i32, winner_b as i32) };
        let (fence_a, fence_b) = if direction == -1 {
            (0u32, 0u32)
        } else {
            let opposite = (direction - 4) & 7;
            let tile_a = get_from_memory::<BFTile>(winner_a);
            let fa = ZTHabitat::fence_slot_by_index(&tile_a, direction / 2);
            let fb = if opposite == -1 {
                0
            } else {
                let tile_b = get_from_memory::<BFTile>(winner_b);
                ZTHabitat::fence_slot_by_index(&tile_b, opposite / 2)
            };
            (fa, fb)
        };

        let (chosen_fence, chosen_tile) = if fence_a == 0 {
            (fence_b, winner_b)
        } else if fence_b == 0 {
            (fence_a, winner_a)
        } else if is_tank {
            (fence_b, winner_b)
        } else if gate_hint_tile_ptr == winner_a {
            (fence_a, gate_hint_tile_ptr)
        } else if gate_hint_tile_ptr == winner_b {
            (fence_b, gate_hint_tile_ptr)
        } else {
            (0, gate_hint_tile_ptr)
        };

        let mut result_flag = true;
        if chosen_fence != 0 {
            if self.replace_fence_with_gate(chosen_fence) {
                let rotation: u32 = get_from_memory(chosen_fence + 0x12c);
                unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.move_gate_to(chosen_tile, rotation);
            } else {
                result_code = 7;
                result_flag = false;
            }
        }

        if matches!(result_code, 1 | 3 | 4 | 6) {
            let string_id: u32 = if matches!(result_code, 1 | 3) { 0x2af9 } else { 11000 };
            self.display_gate_placement_message(habitat_ptr, string_id);
        }

        result_flag
    }

    /// Ports `ZTHabitatMgr::fencePlaced` (`ZTHabitatMgr_fencePlaced.c`/`.asm`, cross-checked against
    /// macOS's own independent decompile of the same function - both agree step-for-step) as an
    /// orchestrator, in the same style as [`Self::create_habitat`]: reimplements the real logic directly
    /// but calls through to real vanilla (`.original()`) for every step this pass doesn't separately
    /// port (`splitTank`/`splitTankIntoLand`/`ZTHabitat::resize`/`setName`/`checkExhibitMorph`, the
    /// `ZTHabitat` constructor, `ZTTankExhibit::updateTankInfo`) - [`Self::snap_tank_walls_inward`] itself
    /// is genuinely reimplemented.
    ///
    /// Real body, in order:
    /// 1. Bails immediately while [`GATE_CONVERSION_IN_PROGRESS_RVA`] is set (a gate conversion this same
    ///    fence is already part of is in progress - [`Self::replace_gate_with_fence`]/
    ///    [`Self::replace_fence_with_gate`]'s own re-entrancy guard).
    /// 2. Resolves `tile_ptr`'s neighbour in `direction` ([`get_neighbour_raw`] - **not**
    ///    [`get_neighbour_ptr`]'s [`Direction::from`]-based North fallback: real vanilla's own
    ///    `BFMap::getNeighbor` treats any out-of-range direction, including the `0xffffffff` "no
    ///    direction" sentinel, as a `(0,0)` offset, i.e. the tile itself, confirmed directly against
    ///    `BFMap_getNeighbor_0.c`). Bails if there's no neighbour (map edge).
    /// 3. Reads the fence sitting on the neighbour's own slot facing back at `tile_ptr` (the *opposite*
    ///    direction). If it's a real fence/wall ([`is_wall`]), takes the **short branch**: when `direction`
    ///    is a real cardinal direction and `tile_ptr`'s own fence in that direction is specifically a
    ///    `ZTTankWall` ([`RVA_TANK_WALL_TYPE_CHECK_ARG`]) *and* a save is currently loading
    ///    (`ZTUI::gameopts::loadInProgress`'s backing store - [`RVA_APP_INIT_SUCCESS_BASE`]'s own `+0x441`
    ///    byte, same fixed address [`Self::place_gate`] and `ZTHabitat::reset_unit_ai` already read), snaps
    ///    the tank occupying `tile_ptr` ([`Self::get_tank`]) back inward
    ///    ([`SNAP_TANK_WALLS_INWARD`]/[`Self::do_show_check`]/`ZTTankExhibit::updateTankInfo`). Either
    ///    way, calls through to still-un-ported `checkExhibitMorph` and re-derives both neighbour kinds
    ///    ([`Self::update_amphibious_neighbors_from_tile`]/[`Self::update_show_neighbors_from_tile`]), then
    ///    returns.
    /// 4. Otherwise (no wall directly between the two tiles) takes the **long branch**: computes 4 more
    ///    tiles (`tile_ptr`/the neighbour, each stepped ±90° from `direction` ([`rotate_cardinal_direction`]))
    ///    plus 14 fence-passability checks ([`fence_wall_at`]) spread across them, split into two
    ///    7-term sets (one per side of the wall). Bails unless **both** sets have at least one true term -
    ///    real vanilla's own way of asking "does this new fence actually close off a boundary on both
    ///    sides," not just a single mid-wall segment with open ends. One term per side (`local_24`) is a
    ///    genuine `AND` of two sibling checks already covered separately by another term in the same set
    ///    (`local_28`) - confirmed via `.asm` register/stack-slot reuse (an init-from-sibling-then-
    ///    conditionally-clear pattern), making it logically redundant in the final `OR`, but reproduced
    ///    faithfully rather than simplified away, matching real vanilla exactly.
    /// 5. If `tile_ptr` and the neighbour can already reach each other ([`Self::can_find_path`]), nothing
    ///    to do - returns. Otherwise checks whether either side can reach the zoo entrance
    ///    ([`Self::get_zoo_entrance_tile_ptr`]); whichever can becomes the seed for a brand new habitat via
    ///    [`Self::create_habitat`] (resized/gated using the other tile), and the function returns.
    /// 6. If neither side can reach the entrance, the habitat currently occupying `tile_ptr` needs
    ///    splitting instead. A tank ([`ZTHabitat::is_tank`]) goes through `splitTank`, falling back to
    ///    `splitTankIntoLand` + a `checkExhibitMorph` call-through if the split itself fails. A plain
    ///    habitat is split by construction: allocates a second `ZTHabitat` seeded at the neighbour tile,
    ///    names it (or the original, whichever isn't a tank - `decrementHabitatNum` if both are),
    ///    [`Self::add_habitat`]s it, resizes both around whichever side still owns the original's own
    ///    entrance tile, places a gate between them ([`Self::place_gate`]), preserves the original's own
    ///    name on whichever new habitat doesn't already carry it, re-derives both neighbour kinds and
    ///    `doShowCheck`, and finally calls through to `checkExhibitMorph`. The name-preservation buffer is
    ///    built/torn down via the same real-`PoolAlloc`-backed `vector<byte>` idiom already established by
    ///    [`Self::display_gate_placement_message`]'s own `final_buf` ([`VECTORBYTE`]/[`free_event_vector_buffer`]).
    ///
    /// A defensive `tile_ptr`/habitat-owner null guard is added ahead of the split/construct step - real
    /// vanilla dereferences the habitat occupying `tile_ptr` unconditionally there with no null check, a
    /// genuinely dead-in-practice input this file guards rather than reproduces elsewhere too (e.g.
    /// [`Self::fence_replaced`]).
    ///
    /// **Live-testing note**: like [`Self::place_gate`]/[`Self::create_habitat`], this mutates real
    /// habitat/fence/gate state (and can destroy/recreate a habitat) with no known synthetic-safe input -
    /// detoured for manual/interactive live verification, not covered by an automated live test.
    pub fn fence_placed(&self, tile_ptr: u32, direction: u32) {
        let mgr_ptr = self as *const Self as *const u32;
        let base = get_module_base("zoo.exe") as u32;
        if get_from_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA) != 0 {
            return;
        }

        let world = globals().ztworldmgr();
        let neighbour_ptr = get_neighbour_raw(world, tile_ptr, direction);
        if neighbour_ptr == 0 {
            return;
        }
        let opposite = rotate_cardinal_direction(direction, 4);
        let central_is_wall = fence_wall_at(neighbour_ptr, opposite);

        if central_is_wall {
            if direction != 0xffff_ffff {
                let tile = get_from_memory::<BFTile>(tile_ptr);
                let fence_here = ZTHabitat::fence_slot_by_index(&tile, (direction / 2) as i32);
                if fence_here != 0 && unsafe { entity_type_matches(fence_here, RVA_TANK_WALL_TYPE_CHECK_ARG) } {
                    let load_in_progress = get_from_memory::<u8>(base + RVA_APP_INIT_SUCCESS_BASE + 0x441) != 0;
                    if load_in_progress {
                        let tank_ptr = self.get_tank(tile_ptr);
                        if tank_ptr != 0 {
                            self.snap_tank_walls_inward(tank_ptr);
                            self.do_show_check(tank_ptr, false);
                            unsafe { ZTTANKEXHIBIT_UPDATE_TANK_INFO.original()(tank_ptr as *const u32) };
                        }
                    }
                }
            }
            unsafe { CHECK_EXHIBIT_MORPH.original()(mgr_ptr, tile_ptr as *const u32, direction) };
            self.update_amphibious_neighbors_from_tile(tile_ptr, direction);
            self.update_show_neighbors_from_tile(tile_ptr, direction);
            return;
        }

        let r1 = rotate_cardinal_direction(direction, 2);
        let r2 = rotate_cardinal_direction(direction, -2);
        let d_opt = if direction == 0xffff_ffff { None } else { Some(direction) };

        let n_r1_ptr = r1.map(|d| get_neighbour_raw(world, neighbour_ptr, d)).unwrap_or(0);
        let n_r2_ptr = r2.map(|d| get_neighbour_raw(world, neighbour_ptr, d)).unwrap_or(0);
        let t_r1_ptr = r1.map(|d| get_neighbour_raw(world, tile_ptr, d)).unwrap_or(0);
        let t_r2_ptr = r2.map(|d| get_neighbour_raw(world, tile_ptr, d)).unwrap_or(0);

        let side_a_any = (t_r2_ptr == 0 && n_r2_ptr == 0)
            || fence_wall_at(t_r2_ptr, r1)
            || fence_wall_at(t_r2_ptr, d_opt)
            || fence_wall_at(n_r2_ptr, opposite)
            || fence_wall_at(n_r2_ptr, r1)
            || fence_wall_at(neighbour_ptr, r2)
            || fence_wall_at(tile_ptr, r2);
        if !side_a_any {
            return;
        }

        let local_28 = fence_wall_at(n_r1_ptr, r2);
        let side_b_any = (t_r1_ptr == 0 && n_r1_ptr == 0)
            || fence_wall_at(t_r1_ptr, r2)
            || fence_wall_at(t_r1_ptr, d_opt)
            || (fence_wall_at(n_r1_ptr, opposite) && local_28)
            || local_28
            || fence_wall_at(neighbour_ptr, r1)
            || fence_wall_at(tile_ptr, r1);
        if !side_b_any {
            return;
        }

        self.clear_pathfinding();
        if self.can_find_path(tile_ptr, neighbour_ptr) {
            return;
        }
        let zoo_entrance_ptr = self.get_zoo_entrance_tile_ptr();
        self.clear_pathfinding();
        if self.can_find_path(tile_ptr, zoo_entrance_ptr) {
            self.create_habitat(neighbour_ptr, tile_ptr, tile_ptr, 0);
            return;
        }
        self.clear_pathfinding();
        if self.can_find_path(neighbour_ptr, zoo_entrance_ptr) {
            self.create_habitat(tile_ptr, neighbour_ptr, tile_ptr, 0);
            return;
        }

        let old_habitat_ptr = if tile_ptr == 0 {
            0
        } else {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            self.get_habitat_ptr(tile.pos.x, tile.pos.y)
        };
        if old_habitat_ptr == 0 {
            return;
        }
        let old_habitat = unsafe { ref_from_memory::<ZTHabitat>(old_habitat_ptr) };

        if old_habitat.is_tank() {
            let split_ok = low_byte_bool(unsafe {
                SPLIT_TANK.original()(mgr_ptr, old_habitat_ptr as *const i32, tile_ptr as *const u32, neighbour_ptr as *const u32)
            });
            if !split_ok {
                unsafe {
                    SPLIT_TANK_INTO_LAND.original()(mgr_ptr, old_habitat_ptr as *const u32, tile_ptr as *const u32, neighbour_ptr as *const u32, tile_ptr as *const u32)
                };
                unsafe { CHECK_EXHIBIT_MORPH.original()(mgr_ptr, tile_ptr as *const u32, direction) };
            }
            return;
        }

        // Entrance-tile bookkeeping, captured before `resize` below may change tile ownership.
        let entrance_tile_ptr_raw = old_habitat.entrance_tile_ptr;
        let entrance_rotation = old_habitat.entrance_rotation;
        let mut entrance_ref_tile = entrance_tile_ptr_raw;
        if entrance_tile_ptr_raw != 0 {
            let entrance_tile = get_from_memory::<BFTile>(entrance_tile_ptr_raw);
            if self.get_habitat_ptr(entrance_tile.pos.x, entrance_tile.pos.y) != old_habitat_ptr {
                entrance_ref_tile = get_neighbour_raw(world, entrance_tile_ptr_raw, entrance_rotation);
            }
        }
        let (undo_rotation, undo_x, undo_y): (i32, i32, i32) = if entrance_tile_ptr_raw != 0 {
            let entrance_tile = get_from_memory::<BFTile>(entrance_tile_ptr_raw);
            (entrance_rotation as i32, entrance_tile.pos.x, entrance_tile.pos.y)
        } else {
            (-1, 0, 0)
        };

        // Copy the original habitat's own name into a real, `PoolAlloc`-backed scratch buffer before
        // `resize` below - matching [`Self::display_gate_placement_message`]'s own `final_buf` idiom.
        let (name_begin, name_end, _) = old_habitat.exhibit_name.raw_parts();
        let name_len = name_end.saturating_sub(name_begin);
        let mut name_buf = [0u32; 3];
        unsafe { VECTORBYTE.original()(name_buf.as_mut_ptr() as *const u8, (name_len + 1) as *const u32) };
        let name_buf_begin = name_buf[0];
        for i in 0..name_len {
            save_to_memory::<u8>(name_buf_begin + i, get_from_memory::<u8>(name_begin + i));
        }
        save_to_memory::<u8>(name_buf_begin + name_len, 0);
        name_buf[1] = name_buf_begin + name_len;
        name_buf[2] = name_buf_begin + name_len + 1;

        unsafe { ZTHABITAT_RESIZE.original()(old_habitat_ptr as *const u32, tile_ptr as i32) };
        unsafe { mut_from_memory::<ZTHabitat>(old_habitat_ptr) }.set_dirty_characteristics();

        let tile = get_from_memory::<BFTile>(tile_ptr);
        let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() };
        unsafe {
            ADD_UNDO_ACTION.original()(
                mapview_ptr,
                0xb,
                0,
                0,
                std::ptr::null::<u32>(),
                undo_rotation as u32,
                0,
                tile.pos.x as u32,
                tile.pos.y as u32,
                undo_x as u32,
                undo_y as *const i32,
                name_buf[0] as *const u32,
                name_buf[1] as i32,
                name_buf[2] as i32,
            )
        };

        let mut new_habitat_ptr = unsafe { OPERATOR_NEW.original()(0x178) } as u32;
        if new_habitat_ptr != 0 {
            new_habitat_ptr = unsafe { ZTHABITAT_CONSTRUCTOR.original()(new_habitat_ptr as *const u32, neighbour_ptr as *const u32, false) } as u32;
        }

        let new_is_tank = new_habitat_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(new_habitat_ptr) }.do_tank_check();
        let old_is_tank = unsafe { ref_from_memory::<ZTHabitat>(old_habitat_ptr) }.do_tank_check();
        if new_is_tank || old_is_tank {
            unsafe { DECREMENT_HABITAT_NUM.original()(mgr_ptr) };
        } else {
            unsafe { NAME_HABITAT.original()(mgr_ptr, new_habitat_ptr as *const u32) };
        }
        self.add_habitat(new_habitat_ptr);

        let entrance_still_owned = entrance_ref_tile == 0 || {
            let entrance_tile = get_from_memory::<BFTile>(entrance_ref_tile);
            self.get_habitat_ptr(entrance_tile.pos.x, entrance_tile.pos.y) == old_habitat_ptr
        };
        let (primary_ptr, secondary_ptr) = if entrance_still_owned {
            self.place_gate(new_habitat_ptr, neighbour_ptr, tile_ptr, tile_ptr);
            (old_habitat_ptr, new_habitat_ptr)
        } else {
            unsafe { ZTHABITAT_RESIZE.original()(old_habitat_ptr as *const u32, neighbour_ptr as i32) };
            unsafe { ZTHABITAT_RESIZE.original()(new_habitat_ptr as *const u32, tile_ptr as i32) };
            self.place_gate(new_habitat_ptr, tile_ptr, neighbour_ptr, tile_ptr);
            (new_habitat_ptr, old_habitat_ptr)
        };
        if new_is_tank {
            unsafe { ZTHABITAT_SET_NAME.original()(primary_ptr as *const u32, name_buf.as_ptr()) };
        } else if old_is_tank {
            unsafe { ZTHABITAT_SET_NAME.original()(secondary_ptr as *const u32, name_buf.as_ptr()) };
        }

        self.do_show_check(new_habitat_ptr, false);
        self.do_show_check(old_habitat_ptr, false);
        self.update_amphibious_neighbors(new_habitat_ptr);
        self.update_amphibious_neighbors(old_habitat_ptr);
        unsafe { CHECK_EXHIBIT_MORPH.original()(mgr_ptr, tile_ptr as *const u32, direction) };

        free_event_vector_buffer(name_buf[0], name_buf[2] - name_buf[0]);
    }

    /// Ports `ZTHabitatMgr::fenceRemoved` (`ZTHabitatMgr_fenceRemoved.c`/`.asm`, cross-checked against
    /// macOS's own independent decompile - both agree step-for-step), the mirror image of
    /// [`Self::fence_placed`]: where placing a fence asks "did this close off a boundary," removing one
    /// asks "did this open one up."
    ///
    /// Real body, in order:
    /// 1. Bails while [`GATE_CONVERSION_IN_PROGRESS_RVA`] is set, same guard [`Self::fence_placed`] uses.
    /// 2. Resolves the neighbour in `direction` ([`get_neighbour_raw`], same real-vanilla `(0,0)`-offset
    ///    fallback for an out-of-range/sentinel direction as `fence_placed`). Bails if there's none.
    /// 3. Resolves the habitat occupying `tile_ptr` and the habitat occupying the neighbour. For whichever
    ///    side isn't the "world" habitat ([`ZTHabitat::unknown_flag_0x2c`] clear), sets a third,
    ///    still-unconfirmed dirty-style flag ([`ZTHabitat::unknown_flag_0x30`]). Bails if both sides are
    ///    already the same habitat (nothing to do).
    /// 4. Reads the fence sitting on the neighbour's own slot facing back at `tile_ptr` (the *opposite*
    ///    direction - identical computation to `fence_placed`'s own `central_fence`). If it's **still** a
    ///    real fence/wall ([`is_wall`]), the two habitats remain separated - takes the **defer path**
    ///    (step 5). If it's gone, the fence just removed was the last thing keeping the two habitats
    ///    apart - takes the **merge path** (step 6).
    /// 5. **Defer path**: rather than calling `placeGate`/`replaceFenceWithGate` directly mid-removal,
    ///    stashes a deferred request into [`Self::pending_place_gate_habitat_ptr`] (and its 3 sibling
    ///    fields) or [`Self::pending_replace_fence_ptr`] (and its 3 siblings) - see those fields' own doc
    ///    comment for the full shape, and `ZTHabitatMgr::updateGates` (still real/un-ported, called
    ///    through by [`Self::update`] every tick) for the consumer. Only one side is ever queued: whichever
    ///    of the two habitats' own `entrance_tile_ptr`/`entrance_rotation` exactly matches
    ///    `(tile_ptr, direction)` (i.e. its own gate sat right where the fence was removed) - the *other*
    ///    side, unless it's the "world" habitat, in which case a fence-replace is queued instead of a
    ///    gate-placement. Neither queue is touched if neither side's entrance matches. Either way, then -
    ///    gated on `ZTApp`'s own `appInitSuccess` byte ([`RVA_APP_INIT_SUCCESS_BASE`]`+0x440`, one byte before
    ///    `fence_placed`'s own `+0x441` `loadInProgress` read) - applies the queue immediately via
    ///    `updateGates` rather than waiting for the next tick, then calls through to `checkExhibitMorph`
    ///    and re-derives both neighbour kinds.
    /// 6. **Merge path**: picks a survivor (the habitat that absorbs the other) - the "world" habitat
    ///    always wins; otherwise whichever side's own entrance *isn't* sitting exactly on the removed
    ///    fence survives, with `leadsTo` connectivity and then habitat size as tie-breakers when neither
    ///    (or both) entrances match. If survivor and absorbed are **both** tanks, calls through to
    ///    `mergeTanks` and returns - tank merging (wall geometry) is out of this pass's own scope, same as
    ///    `ZTHabitatMgr::splitTank`/`splitTankIntoLand` in `fence_placed`. If the *survivor* turns out to
    ///    be a tank but the absorbed side isn't, swaps them (captures the tank's own current gate first via
    ///    [`ZTHabitat::get_gate`], to relocate onto the real survivor below) - real vanilla prefers keeping
    ///    a plain habitat's own identity over a tank's. Then: removes illegal entities from a tank about to
    ///    be absorbed, [`REMOVE_HABITAT_0`]s it, resizes/dirties the survivor around whichever tile it now
    ///    owns, relocates the captured gate onto the survivor if one was captured
    ///    ([`Self::replace_gate`] + [`ZTHabitat::move_gate_to_inner`] - the same internal helper
    ///    `Self::place_gate` uses, already fixed against the two live-crash bugs its own doc comment
    ///    documents), re-derives amphibious neighbours, and calls [`Self::morph_exhibit`].
    ///
    /// **Live-testing note**: like [`Self::fence_placed`], this mutates real habitat/fence state and can
    /// destroy/merge habitats with no known synthetic-safe input - detoured for manual/interactive live
    /// verification, not covered by an automated live test.
    pub fn fence_removed(&self, tile_ptr: u32, direction: u32) {
        let mgr_ptr = self as *const Self as *const u32;
        let self_addr = self as *const Self as u32;
        let base = get_module_base("zoo.exe") as u32;
        if get_from_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA) != 0 {
            return;
        }

        let world = globals().ztworldmgr();
        let neighbour_ptr = get_neighbour_raw(world, tile_ptr, direction);
        if neighbour_ptr == 0 {
            return;
        }

        let habitat_a_ptr = if tile_ptr == 0 {
            0
        } else {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            self.get_habitat_ptr(tile.pos.x, tile.pos.y)
        };
        let neighbour_tile = get_from_memory::<BFTile>(neighbour_ptr);
        let habitat_b_ptr = self.get_habitat_ptr(neighbour_tile.pos.x, neighbour_tile.pos.y);

        if habitat_a_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) }.unknown_flag_0x2c == 0 {
            save_to_memory::<u8>(habitat_a_ptr + 0x30, 1);
        }
        if habitat_b_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) }.unknown_flag_0x2c == 0 {
            save_to_memory::<u8>(habitat_b_ptr + 0x30, 1);
        }
        if habitat_a_ptr == habitat_b_ptr {
            return;
        }

        let opposite = rotate_cardinal_direction(direction, 4);
        let central_fence = opposite.map(|od| ZTHabitat::fence_slot_by_index(&neighbour_tile, (od / 2) as i32)).unwrap_or(0);

        if !is_wall(central_fence) {
            // ===== Merge path =====
            if habitat_a_ptr == 0 || habitat_b_ptr == 0 {
                return;
            }
            let a_is_world = unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) }.unknown_flag_0x2c != 0;
            let keep_a = if a_is_world {
                true
            } else {
                let b_is_world = unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) }.unknown_flag_0x2c != 0;
                if b_is_world {
                    false
                } else {
                    let a = unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) };
                    let a_entrance_here = a.entrance_tile_ptr == tile_ptr && a.entrance_rotation == direction;
                    if a_entrance_here {
                        false
                    } else {
                        let b = unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) };
                        let b_entrance_here = b.entrance_tile_ptr == tile_ptr && b.entrance_rotation == direction;
                        if b_entrance_here {
                            true
                        } else {
                            let leads_a_to_b = self.leads_to(habitat_a_ptr, habitat_b_ptr);
                            let leads_b_to_a = self.leads_to(habitat_b_ptr, habitat_a_ptr);
                            if !leads_a_to_b && leads_b_to_a {
                                true
                            } else if leads_a_to_b && !leads_b_to_a {
                                false
                            } else {
                                let size_b = unsafe { GET_SIZE.original()(habitat_b_ptr as *const u32, false) };
                                let size_a = unsafe { GET_SIZE.original()(habitat_a_ptr as *const u32, false) };
                                size_b < size_a
                            }
                        }
                    }
                }
            };

            let (mut survivor_ptr, mut absorbed_ptr, mut resize_tile_ptr) =
                if keep_a { (habitat_a_ptr, habitat_b_ptr, tile_ptr) } else { (habitat_b_ptr, habitat_a_ptr, neighbour_ptr) };

            let survivor_is_tank = unsafe { ref_from_memory::<ZTHabitat>(survivor_ptr) }.is_tank();
            let absorbed_is_tank = unsafe { ref_from_memory::<ZTHabitat>(absorbed_ptr) }.is_tank();
            if survivor_is_tank && absorbed_is_tank {
                unsafe { MERGE_TANKS.original()(mgr_ptr, survivor_ptr as *const u32, absorbed_ptr as *const u32, resize_tile_ptr as *const u32) };
                return;
            }

            let mut gate_fence_to_move: u32 = 0;
            if survivor_is_tank && !absorbed_is_tank {
                gate_fence_to_move = unsafe { ref_from_memory::<ZTHabitat>(survivor_ptr) }.get_gate();
                (survivor_ptr, absorbed_ptr) = (absorbed_ptr, survivor_ptr);
                resize_tile_ptr = if resize_tile_ptr != tile_ptr { tile_ptr } else { neighbour_ptr };
            }

            if unsafe { ref_from_memory::<ZTHabitat>(absorbed_ptr) }.is_tank() {
                unsafe { ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES.original()(absorbed_ptr as *const u32, 0, false) };
            }
            unsafe { REMOVE_HABITAT_0.original()(mgr_ptr, absorbed_ptr as *const i32) };
            unsafe { ZTHABITAT_RESIZE.original()(survivor_ptr as *const u32, resize_tile_ptr as i32) };
            unsafe { mut_from_memory::<ZTHabitat>(survivor_ptr) }.set_dirty_characteristics();
            if gate_fence_to_move != 0 {
                self.replace_gate();
                unsafe { ref_from_memory::<ZTHabitat>(survivor_ptr) }.move_gate_to_inner(gate_fence_to_move);
            }
            self.update_amphibious_neighbors(survivor_ptr);
            self.update_show_neighbors(survivor_ptr);
            let survivor = unsafe { ref_from_memory::<ZTHabitat>(survivor_ptr) };
            let out_ptr = survivor.get_gate_tile_out().map(|t| world.get_ptr_from_bftile(&t)).unwrap_or(0);
            let in_ptr = survivor.get_gate_tile_in().map(|t| world.get_ptr_from_bftile(&t)).unwrap_or(0);
            self.morph_exhibit(survivor_ptr, in_ptr, out_ptr);
            return;
        }

        // ===== Defer path =====
        let a_entrance_here = habitat_a_ptr != 0 && {
            let a = unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) };
            a.entrance_tile_ptr == tile_ptr && a.entrance_rotation == direction
        };
        let mut queue_replace = false;
        if a_entrance_here {
            let b_is_world = habitat_b_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) }.unknown_flag_0x2c != 0;
            if !b_is_world {
                save_to_memory::<u32>(self_addr + 0x34, habitat_a_ptr);
                save_to_memory::<u32>(self_addr + 0x38, tile_ptr);
                save_to_memory::<u32>(self_addr + 0x3c, neighbour_ptr);
                save_to_memory::<u32>(self_addr + 0x40, neighbour_ptr);
            } else {
                save_to_memory::<u32>(self_addr + 0x48, habitat_a_ptr);
                save_to_memory::<u32>(self_addr + 0x4c, neighbour_ptr);
                queue_replace = true;
            }
        } else {
            let b_entrance_here = habitat_b_ptr != 0 && {
                let b = unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) };
                b.entrance_tile_ptr == tile_ptr && b.entrance_rotation == direction
            };
            if b_entrance_here {
                let a_is_world = habitat_a_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) }.unknown_flag_0x2c != 0;
                if !a_is_world {
                    save_to_memory::<u32>(self_addr + 0x34, habitat_b_ptr);
                    save_to_memory::<u32>(self_addr + 0x38, neighbour_ptr);
                    save_to_memory::<u32>(self_addr + 0x3c, tile_ptr);
                    save_to_memory::<u32>(self_addr + 0x40, neighbour_ptr);
                } else {
                    save_to_memory::<u32>(self_addr + 0x48, habitat_b_ptr);
                    save_to_memory::<u32>(self_addr + 0x4c, neighbour_ptr);
                    queue_replace = true;
                }
            }
        }
        if queue_replace {
            save_to_memory::<u32>(self_addr + 0x44, central_fence);
            save_to_memory::<u32>(self_addr + 0x50, opposite.unwrap_or(0xffff_ffff));
        }

        let app_init_success = get_from_memory::<u8>(base + RVA_APP_INIT_SUCCESS_BASE + 0x440) != 0;
        if app_init_success {
            self.update_gates();
        }
        unsafe { CHECK_EXHIBIT_MORPH.original()(mgr_ptr, tile_ptr as *const u32, direction) };
        self.update_amphibious_neighbors_from_tile(tile_ptr, direction);
        self.update_show_neighbors_from_tile(tile_ptr, direction);
    }

    /// Ports `ZTHabitatMgr::morphExhibit` (`ZTHabitatMgr_morphExhibit.c`/`.asm`, cross-checked
    /// step-for-step against the macOS decompile). Destroys and recreates `habitat_ptr` as the opposite
    /// tank/land kind whenever its "should be a tank" boundary check ([`ZTHabitat::do_tank_check`])
    /// disagrees with what it currently is ([`ZTHabitat::is_tank`]) - a no-op otherwise.
    ///
    /// Real body, in order:
    /// 1. Builds the real vanilla morph undo action ([`ADD_UNDO_ACTION`], action id `0xb`) up front,
    ///    while `habitat_ptr`'s own fields are still valid: its entrance rotation/position if it
    ///    currently has a gate ([`ZTHabitat::entrance_tile_ptr`], else `-1`/`(0,0)`), `tile_2_ptr`'s own
    ///    position, and a real, `PoolAlloc`-backed copy of the habitat's own name (matching
    ///    [`Self::fence_placed`]'s own name-buffer idiom - real vanilla's own second, identically-shaped
    ///    buffer built later in this function is read nowhere and immediately freed again, a pure
    ///    compiler RAII artifact with no observable effect; skipped here).
    /// 2. Picks the two boundary tiles the gate-search loop starts from: `tile_2_ptr`/`tile_3_ptr`
    ///    themselves, unless they resolve (via [`Self::get_habitat_ptr`]) to the *same* owning habitat,
    ///    in which case it starts instead from `habitat_ptr`'s own first boundary tile-pair
    ///    ([`ZTHabitat::boundary_tile_pairs_begin`]). Then repeatedly calls real vanilla, still-un-ported
    ///    `getNextFencePair` ([`GET_NEXT_FENCE_PAIR`], single-step mode - the `false` flag, confirmed
    ///    against `.asm`: this loop is real vanilla's own outer loop, not `getNextFencePair`'s internal
    ///    one, unlike [`Self::advance_fence_pair`]'s own `true`-flag call in `place_gate`) until the
    ///    second candidate is back in the zoo ([`BFTILE_IS_IN_ZOO`]).
    /// 3. If `habitat_ptr` currently has a gate, captures it ([`ZTHabitat::get_gate`]) to relocate onto
    ///    the new habitat afterward.
    /// 4. If the *original* habitat was a tank, removes its illegal entities
    ///    ([`ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES`]) before destroying it.
    /// 5. [`Self::replace_gate`], captures the current gate-tile-in/out pair, [`REMOVE_HABITAT_0`]s the
    ///    habitat, then [`Self::replace_gate`] again (matching real vanilla's own two calls either side
    ///    of the removal) - the same "capture then call twice" shape [`Self::fence_removed`]'s own merge
    ///    path already uses around `move_gate_to_inner`.
    /// 6. If a gate was captured in step 3, converts it back into a plain fence
    ///    ([`ZTFENCE_MAKE_FENCE`]).
    /// 7. Recreates the habitat ([`Self::create_habitat`]) seeded from the captured gate-tile-in/out
    ///    pair, or from the post-loop boundary-tile candidates if the old habitat had no gate tile in.
    ///
    /// **Live-testing note**: like [`Self::create_double_fence`]/[`ZTHabitat::move_gate_to`], this both
    /// creates a real undo action and destroys/recreates real habitat state with no known synthetic-safe
    /// input - detoured for manual/interactive live verification, not covered by an automated live test.
    pub fn morph_exhibit(&self, habitat_ptr: u32, tile_2_ptr: u32, tile_3_ptr: u32) {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let was_tank = habitat.is_tank();
        if habitat.do_tank_check() == was_tank {
            return;
        }

        let entrance_tile_ptr = habitat.entrance_tile_ptr;
        let (undo_rotation, undo_x, undo_y): (i32, i32, i32) = if entrance_tile_ptr != 0 {
            let entrance_tile = get_from_memory::<BFTile>(entrance_tile_ptr);
            (habitat.entrance_rotation as i32, entrance_tile.pos.x, entrance_tile.pos.y)
        } else {
            (-1, 0, 0)
        };
        let tile_2 = get_from_memory::<BFTile>(tile_2_ptr);

        let (name_begin, name_end, _) = habitat.exhibit_name.raw_parts();
        let name_len = name_end.saturating_sub(name_begin);
        let mut name_buf = [0u32; 3];
        unsafe { VECTORBYTE.original()(name_buf.as_mut_ptr() as *const u8, (name_len + 1) as *const u32) };
        let name_buf_begin = name_buf[0];
        for i in 0..name_len {
            save_to_memory::<u8>(name_buf_begin + i, get_from_memory::<u8>(name_begin + i));
        }
        save_to_memory::<u8>(name_buf_begin + name_len, 0);
        name_buf[1] = name_buf_begin + name_len;
        name_buf[2] = name_buf_begin + name_len + 1;

        let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() };
        unsafe {
            ADD_UNDO_ACTION.original()(
                mapview_ptr,
                0xb,
                0,
                0,
                std::ptr::null::<u32>(),
                undo_rotation as u32,
                0,
                tile_2.pos.x as u32,
                tile_2.pos.y as u32,
                undo_x as u32,
                undo_y as *const i32,
                name_buf[0] as *const u32,
                name_buf[1] as i32,
                name_buf[2] as i32,
            )
        };

        let owner_2 = self.get_habitat_ptr(tile_2.pos.x, tile_2.pos.y);
        let owner_3 = if tile_3_ptr == 0 {
            0
        } else {
            let tile_3 = get_from_memory::<BFTile>(tile_3_ptr);
            self.get_habitat_ptr(tile_3.pos.x, tile_3.pos.y)
        };
        let (mut cand_a, mut cand_b) = if owner_2 == owner_3 {
            (
                get_from_memory::<u32>(habitat.boundary_tile_pairs_begin),
                get_from_memory::<u32>(habitat.boundary_tile_pairs_begin + 4),
            )
        } else {
            (tile_2_ptr, tile_3_ptr)
        };
        while !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.original()(cand_b as *const u32, 1) }) {
            self.get_next_fence_pair(habitat_ptr, &mut cand_a, &mut cand_b, false);
        }

        let gate_fence_ptr = if entrance_tile_ptr != 0 { habitat.get_gate() } else { 0 };

        if was_tank {
            unsafe { ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES.original()(habitat_ptr as *const u32, 0, false) };
        }
        self.replace_gate();
        let world = globals().ztworldmgr();
        let gate_tile_in_ptr = habitat.get_gate_tile_in().map(|t| world.get_ptr_from_bftile(&t)).unwrap_or(0);
        let gate_tile_out_ptr = habitat.get_gate_tile_out().map(|t| world.get_ptr_from_bftile(&t)).unwrap_or(0);
        unsafe { REMOVE_HABITAT_0.original()(self as *const Self as *const u32, habitat_ptr as *const i32) };
        self.replace_gate();

        if gate_fence_ptr != 0 {
            unsafe { ZTFENCE_MAKE_FENCE.original()(gate_fence_ptr as *const u32) };
        }

        let (seed_ptr, resize_ptr) = if gate_tile_in_ptr == 0 { (cand_a, cand_b) } else { (gate_tile_in_ptr, gate_tile_out_ptr) };
        self.create_habitat(seed_ptr, resize_ptr, seed_ptr, 0);
    }

    /// Creates a temporary `ZTKeeper`/tank-keeper-type unit purely to drive [`Self::place_gate`]'s own
    /// pathfinding checks - `BFWorldMgr::getType` for the keeper catalog id (`0x2550` for a tank habitat,
    /// `0x251c` otherwise), confirming it's really keeper-family ([`type_check`]/
    /// [`RVA_KEEPER_TYPE_CHECK_ARG`]) before instantiating it ([`call_type_vtable_slot_u8_ret_u32`],
    /// `+0x24`), then re-confirming the *created instance* is keeper-family too
    /// ([`entity_type_matches`]/[`RVA_KEEPER_TYPE_CHECK_ARG`]) and setting its own `+0x2a8` flag (real
    /// vanilla's own `.asm`, purpose not otherwise identified). `None` if any step fails.
    pub(crate) fn temp_keeper_for_pathfinding(&self, is_tank: bool) -> Option<u32> {
        let world_ptr = globals().ztworldmgr_ptr() as u32;
        let type_id: i32 = if is_tank { 0x2550 } else { 0x251c };
        let type_ptr = unsafe { BFWORLDMGR_GET_TYPE.original()(world_ptr as *const u32, type_id) } as u32;
        if type_ptr == 0 || !unsafe { type_check(type_ptr, RVA_KEEPER_TYPE_CHECK_ARG) } {
            return None;
        }

        let base = get_module_base("zoo.exe") as u32;
        let create_flag: u8 = get_from_memory(base + RVA_TEMP_KEEPER_CREATE_FLAG);
        let keeper_ptr = unsafe { call_type_vtable_slot_u8_ret_u32(type_ptr, 0x24, create_flag) };
        if keeper_ptr == 0 || !unsafe { entity_type_matches(keeper_ptr, RVA_KEEPER_TYPE_CHECK_ARG) } {
            return None;
        }

        save_to_memory::<u8>(keeper_ptr + 0x2a8, 1);
        Some(keeper_ptr)
    }

    /// Destroys a keeper obtained from [`Self::temp_keeper_for_pathfinding`] - real vanilla's own vtable
    /// `+0x18` (a `BFEntity`-family destructor overload, `arg=1`), confirmed via `.asm`'s own `CALL dword
    /// ptr [EDX+0x18]` with a single pushed `1`.
    ///
    /// **Not called on every exit path** - matching real vanilla exactly: the two `getNextFencePair`
    /// exhaustion points inside [`Self::place_gate`]'s own scan loop return `false` *before* real vanilla's
    /// own keeper-destroy call (confirmed via `.asm` - both early returns are a direct `RET`, no
    /// destructor call beforehand). A genuine resource leak in real vanilla itself on total exhaustion,
    /// not a bug in this port - reproduced faithfully rather than "fixed", since fixing it would diverge
    /// from real vanilla's own observable behavior with no supporting evidence it's ever hit live.
    pub(crate) fn destroy_temp_keeper(keeper_ptr: u32) {
        unsafe { call_vtable_slot_with_u8(keeper_ptr, 0x18, 1) };
    }

    /// Calls real vanilla's own gate-candidate path-reachability check (`BFPathFinder::findPath`, matching
    /// macOS's own decompile naming and `generated.rs`'s `bfpathfinder::FIND_PATH`) via a raw
    /// function-pointer table stored at `GLOBAL_ZTAIMgr+0x10` - the function itself is [`Self::place_gate`]'s
    /// only use of `ZTAIMgr`'s internals, itself out of scope for reimplementation per `globals.rs`'s own
    /// `ZTAIMgr` doc comment. `from_tile_ptr`/`to_tile_ptr` are passed as `tile_ptr + 0x34` (each `BFTile`'s
    /// own `x_pos` field address, i.e. `&tile.pos`).
    ///
    /// **`thiscall` with `this = table_addr`, not a plain 4-arg `stdcall`** - confirmed by reading
    /// `ZTHabitatMgr_placeGate.asm` instruction-by-instruction around the call site: `ECX` is loaded with
    /// `&GLOBAL_ZTAIMgr->field_0x10` (this same `table_addr`) several instructions earlier and is never
    /// overwritten before `CALL dword ptr [EAX]` - real vanilla's own compiled code leaves it sitting in
    /// `ECX` as the call's implicit receiver. Real vanilla never crashes on this because `table_addr` is
    /// always a valid, mapped address (a real field inside `GLOBAL_ZTAIMgr`'s own struct); the previous
    /// `extern "stdcall"` version here left `ECX` as whatever this port's own compiled code happened to
    /// leave behind (observed as `0` in a live crash) - `findPath`'s own body immediately reads `this+0x2c`,
    /// so a null/garbage `ECX` is a guaranteed null-pointer deref inside real vanilla itself. Confirmed live
    /// via `debug-play`: `zoo+0x14d66` (`BFPathFinder::findPath+0x27`, `mov eax,[ecx+0x2c]` with `ecx=0`),
    /// called directly from this function inside [`Self::place_gate`]'s pathfinding scan loop.
    ///
    /// `findPath`'s own return is `u32` (`generated.rs`), and macOS's own decompile checks it with
    /// `(uVar8 & 0xff) == 0` - the same `CONCAT31`-style undefined-upper-bytes shape this file already
    /// guards elsewhere ([`low_byte_bool`]), not a genuine 32-bit boolean.
    pub(crate) unsafe fn call_pathfinder(table_addr: u32, from_tile_ptr: u32, to_tile_ptr: u32, unit_ptr: u32) -> bool {
        let vtable_ptr: u32 = get_from_memory(table_addr);
        if vtable_ptr == 0 {
            return false;
        }
        let func_ptr: u32 = get_from_memory(vtable_ptr);
        if func_ptr == 0 {
            return false;
        }
        let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, i8) -> u32>(func_ptr) };
        low_byte_bool(f(table_addr, from_tile_ptr + 0x34, to_tile_ptr + 0x34, unit_ptr, 0))
    }

    /// Thin wrapper around real vanilla `getNextFencePair` (still un-ported) for
    /// [`Self::place_gate`]'s own scan loop - advances `cand_a`/`cand_b` in place, masked via
    /// [`low_byte_bool`] per this file's own established `CONCAT31` caution (real callers elsewhere all
    /// Ports `ZTHabitatMgr::updateGates` (`ZTHabitatMgr_updateGates.c`/`.asm`, `UPDATE_GATES` at `0x00435a0d`).
    /// Drains the manager's deferred replacement and deferred gate-placement queues (`pending_replace_*`
    /// and `pending_place_gate_*`).
    pub fn update_gates(&self) {
        let self_addr = self as *const Self as u32;
        let replace_fence = self.pending_replace_fence_ptr;
        if replace_fence != 0 {
            let world_ptr = globals().ztworldmgr_ptr() as *const u32;
            let valid = unsafe { BFWORLDMGR_VERIFY_ENTITY_0.original()(world_ptr, replace_fence as *const u32) } != 0;
            if valid {
                let replaced = self.replace_fence_with_gate(replace_fence);
                let habitat_ptr = self.pending_replace_habitat_ptr;
                if habitat_ptr != 0 {
                    if !replaced {
                        save_to_memory::<u32>(habitat_ptr + 0x8c, 0);
                        save_to_memory::<u32>(habitat_ptr + 0x90, 0xffff_ffff);
                    } else {
                        let tile_ptr = self.pending_replace_tile_ptr;
                        let rotation = self.pending_replace_rotation;
                        save_to_memory::<u32>(habitat_ptr + 0x8c, tile_ptr);
                        save_to_memory::<u32>(habitat_ptr + 0x90, rotation);
                        if rotation != 0xffff_ffff && tile_ptr != 0 {
                            let tile = get_from_memory::<BFTile>(tile_ptr);
                            let fence_ptr = ZTHabitat::fence_slot_by_index(&tile, (rotation as i32) / 2);
                            if fence_ptr != 0 {
                                unsafe { call_vtable_slot_with_ptr(fence_ptr, 0x1c, habitat_ptr + 0x154) };
                            }
                        }
                    }
                }
            }
        }
        save_to_memory::<u32>(self_addr + 0x44, 0);
        save_to_memory::<u32>(self_addr + 0x48, 0);
        save_to_memory::<u32>(self_addr + 0x4c, 0);
        save_to_memory::<u32>(self_addr + 0x50, 0xffff_ffff);

        let place_habitat = self.pending_place_gate_habitat_ptr;
        if place_habitat != 0 {
            self.place_gate(
                place_habitat,
                self.pending_place_gate_seed_tile_ptr,
                self.pending_place_gate_resize_tile_ptr,
                self.pending_place_gate_hint_tile_ptr,
            );
        }
        save_to_memory::<u32>(self_addr + 0x34, 0);
        save_to_memory::<u32>(self_addr + 0x38, 0);
        save_to_memory::<u32>(self_addr + 0x3c, 0);
        save_to_memory::<u32>(self_addr + 0x40, 0);
    }

    /// Ports `ZTHabitatMgr::formatHabitatMessage` (`ZTHabitatMgr_formatHabitatMessage.c`/`.asm`,
    /// `FORMAT_HABITAT_MESSAGE` at `0x004e877d`). Formats a localized string containing the habitat's name
    /// into a vanilla-heap `vector<byte>`.
    pub fn format_habitat_message(out_vec: *mut u32, string_id: u32, habitat_ptr: u32) -> *mut u32 {
        let habitat_name = if habitat_ptr != 0 {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
            habitat.exhibit_name.copy_to_string()
        } else {
            String::new()
        };
        let template = load_string_by_id(string_id & 0xffff).unwrap_or_default();
        let formatted = template.replace("%s", &habitat_name);
        let bytes = formatted.as_bytes();
        let total_len = (bytes.len() + 1) as u32;
        unsafe {
            VECTORBYTE.original()(out_vec as *const u8, total_len as *const u32);
            let begin = *out_vec;
            if begin != 0 {
                for (i, &b) in bytes.iter().enumerate() {
                    save_to_memory::<u8>(begin + i as u32, b);
                }
                save_to_memory::<u8>(begin + bytes.len() as u32, 0);
                *out_vec.add(1) = begin + bytes.len() as u32;
            }
        }
        out_vec
    }

    /// Formats and displays [`Self::place_gate`]'s own "gate placement wasn't ideal" toast -
    /// [`Self::format_habitat_message`] followed by `BFUIMgr::displayMessage` (`bfuimgr::DISPLAY_MESSAGE_1`).
    pub(crate) fn display_gate_placement_message(&self, habitat_ptr: u32, string_id: u32) {
        let mut format_out = [0u32; 3];
        Self::format_habitat_message(format_out.as_mut_ptr(), string_id, habitat_ptr);
        let (msg_begin, msg_end, msg_cap_end) = (format_out[0], format_out[1], format_out[2]);
        let msg_len = msg_end.saturating_sub(msg_begin);

        let mut final_buf = [0u32; 3];
        unsafe { VECTORBYTE.original()(final_buf.as_mut_ptr() as *const u8, (msg_len + 1) as *const u32) };
        let final_begin = final_buf[0];
        for i in 0..msg_len {
            save_to_memory::<u8>(final_begin + i, get_from_memory::<u8>(msg_begin + i));
        }
        save_to_memory::<u8>(final_begin + msg_len, 0);
        final_buf[1] = final_begin + msg_len;
        final_buf[2] = final_begin + msg_len + 1;

        if msg_begin != 0 {
            unsafe { POOLALLOC_DEALLOCATE.original()(msg_begin as *const u32, msg_cap_end - msg_begin) };
        }

        let gate_tile_ptr = unsafe { ZTHABITAT_GET_GATE_TILE_IN.original()(habitat_ptr as *const u32) } as u32;
        let bfuimgr_ptr = get_module_base("zoo.exe") as u32 + RVA_GLOBAL_BFUIMGR;
        unsafe {
            BFUIMGR_DISPLAY_MESSAGE_1.original()(
                bfuimgr_ptr as *const u32,
                final_buf.as_ptr() as *const i32,
                5,
                gate_tile_ptr as *const u32,
                std::ptr::null(),
                true,
                false,
            )
        };

        free_event_vector_buffer(final_begin, final_buf[2] - final_begin);
    }

    /// Ports `ZTHabitatMgr::findBestPlaceForGate` (`ZTHabitatMgr_findBestPlaceForGate.c`/`.asm`,
    /// `FIND_BEST_PLACE_FOR_GATE` at `0x0060477c`).
    pub fn find_best_place_for_gate(&self, habitat_ptr: u32) -> bool {
        if habitat_ptr == 0 {
            return false;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let pairs_begin = habitat.boundary_tile_pairs_begin;
        let pairs_end = habitat.boundary_tile_pairs_end;
        if pairs_begin == 0 || pairs_end <= pairs_begin {
            return false;
        }
        let num_pairs = (pairs_end - pairs_begin) / 8;
        for i in 0..num_pairs {
            let pair_addr = pairs_begin + i * 8;
            let tile_a_ptr = get_from_memory::<u32>(pair_addr);
            let tile_b_ptr = get_from_memory::<u32>(pair_addr + 4);
            if tile_b_ptr == 0 {
                continue;
            }
            if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.original()(tile_b_ptr as *const u32, 1) }) {
                continue;
            }
            let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
            let neighbor_habitat_ptr = self.get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
            if neighbor_habitat_ptr == 0 {
                continue;
            }
            let neighbor_habitat = unsafe { ref_from_memory::<ZTHabitat>(neighbor_habitat_ptr) };
            if neighbor_habitat.is_tank() || self.leads_to(neighbor_habitat_ptr, habitat_ptr) {
                continue;
            }
            let dir_a = if tile_a_ptr != 0 {
                unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) }
            } else {
                -1
            };
            let fence_a = if dir_a != -1 && tile_a_ptr != 0 {
                let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
                let f = ZTHabitat::fence_slot_by_index(&tile_a, dir_a / 2);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            let dir_b = if tile_a_ptr != 0 {
                unsafe { BFMAP_GET_DIRECTION_0.original()(tile_b_ptr as i32, tile_a_ptr as i32) }
            } else {
                -1
            };
            let fence_b = if dir_b != -1 {
                let f = ZTHabitat::fence_slot_by_index(&tile_b, dir_b / 2);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            // Single candidate check: if both exist, skip (internal/double boundary)
            let candidate = if fence_a != 0 && fence_b != 0 {
                continue;
            } else if fence_b != 0 {
                fence_b
            } else {
                fence_a
            };

            if candidate == 0 {
                habitat.move_gate_to_fence(0);
                return true;
            }
            let entity_type = get_from_memory::<u32>(candidate + 0x128);
            if entity_type != 0 && unsafe { entity_type_matches(candidate, RVA_FENCE_TYPE_CHECK_ARG) } {
                let impassable = get_from_memory::<u8>(entity_type + 0x190);
                if impassable == 0 {
                    habitat.move_gate_to_fence(candidate);
                    return true;
                }
            }
        }
        false
    }

    /// Ports `ZTHabitatMgr::findBetterGatesForNeighbors` (`ZTHabitatMgr_findBetterGatesForNeighbors.c`/`.asm`,
    /// `FIND_BETTER_GATES_FOR_NEIGHBORS` at `0x0045c7c2`).
    ///
    /// **Crash Fix**: Vanilla dereferences `[fence + 0x128]` even when `fence == 0` (e.g. open boundary or
    /// invalid direction), causing an immediate null-pointer access violation at `0x00467117`
    /// (`debug_play_tank_crash_findgates.txt`). We guard against null and non-fence candidates before
    /// inspecting the entity type prefix name.
    pub fn find_better_gates_for_neighbors(&self, tank_ptr: u32) {
        let base = get_module_base("zoo.exe") as u32;
        let load_in_progress = get_from_memory::<u8>(base + RVA_APP_INIT_SUCCESS_BASE + 0x441) != 0;
        if load_in_progress || tank_ptr == 0 {
            return;
        }
        let tank = unsafe { ref_from_memory::<ZTHabitat>(tank_ptr) };
        let pairs_begin = tank.boundary_tile_pairs_begin;
        let pairs_end = tank.boundary_tile_pairs_end;
        if pairs_begin == 0 || pairs_end <= pairs_begin {
            return;
        }
        let num_pairs = (pairs_end - pairs_begin) / 8;
        for i in 0..num_pairs {
            let pair_addr = pairs_begin + i * 8;
            let tile_a_ptr = get_from_memory::<u32>(pair_addr);
            let tile_b_ptr = get_from_memory::<u32>(pair_addr + 4);
            if tile_a_ptr == 0 || tile_b_ptr == 0 {
                continue;
            }
            let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
            if dir == -1 {
                continue;
            }
            let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
            let fence_ptr = ZTHabitat::fence_slot_by_index(&tile_a, dir / 2);
            // CRITICAL BUG FIX: guard against null or non-fence candidate before inspecting entity type!
            if fence_ptr == 0 || !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
                continue;
            }
            let entity_type_ptr = get_from_memory::<u32>(fence_ptr + 0x128);
            if entity_type_ptr == 0 {
                continue;
            }
            let prefix_str_ptr = get_from_memory::<u32>(entity_type_ptr + 0xa4);
            if prefix_str_ptr == 0 || get_from_memory::<u8>(prefix_str_ptr) != b'g' {
                continue;
            }
            let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
            let neighbor_habitat_ptr = self.get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
            if neighbor_habitat_ptr != 0 {
                let neighbor_habitat = unsafe { ref_from_memory::<ZTHabitat>(neighbor_habitat_ptr) };
                if neighbor_habitat.get_gate() == fence_ptr {
                    self.find_best_place_for_gate(neighbor_habitat_ptr);
                }
            }
        }
    }

    /// Ports `ZTHabitatMgr::checkGate` (`ZTHabitatMgr_checkGate.c`/`.asm`, `CHECK_GATE` at `0x0046676a`).
    pub fn check_gate(
        &self,
        tile_a_ptr: u32,
        tile_b_ptr: u32,
        keeper_ptr: u32,
        out_cost: *mut i32,
        flag: bool,
    ) -> i32 {
        let base = get_module_base("zoo.exe") as u32;
        let infinite_cost: i32 = get_from_memory(base + RVA_INFINITE_PATH_COST);
        if !out_cost.is_null() {
            unsafe { *out_cost = infinite_cost };
        }
        if tile_a_ptr == 0 || tile_b_ptr == 0 {
            return 4;
        }
        let dir_ba = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_b_ptr as i32, tile_a_ptr as i32) };
        let (opp_dir, fence_b) = if dir_ba != -1 {
            let opp = ((dir_ba as u32).wrapping_sub(4)) & 7;
            let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
            let f = ZTHabitat::fence_slot_by_index(&tile_b, dir_ba / 2);
            let valid_f = if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 };
            (opp as i32, valid_f)
        } else {
            (-1, 0)
        };

        let fence_a = if opp_dir != -1 {
            let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
            let f = ZTHabitat::fence_slot_by_index(&tile_a, opp_dir / 2);
            if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
        } else {
            0
        };

        let is_gate_b = if fence_b != 0 {
            let et = get_from_memory::<u32>(fence_b + 0x128);
            if et != 0 {
                let pfx = get_from_memory::<u32>(et + 0xa4);
                pfx != 0 && get_from_memory::<u8>(pfx) == b'g'
            } else {
                false
            }
        } else {
            false
        };

        let is_gate_a = if fence_a != 0 {
            let et = get_from_memory::<u32>(fence_a + 0x128);
            if et != 0 {
                let pfx = get_from_memory::<u32>(et + 0xa4);
                pfx != 0 && get_from_memory::<u8>(pfx) == b'g'
            } else {
                false
            }
        } else {
            false
        };

        let mut status = 0i32;
        if fence_b != 0 && !is_gate_b && fence_a != 0 && !is_gate_a {
            status = 1;
        }
        if flag && (is_gate_b || is_gate_a) {
            status = 2;
        }

        // Wall solidity check: if either fence is a solid wall (entity_type+0x190 != 0), status = 4
        if fence_b != 0 {
            let et = get_from_memory::<u32>(fence_b + 0x128);
            if et != 0 && get_from_memory::<u8>(et + 0x190) != 0 {
                return 4;
            }
        }
        if fence_a != 0 {
            let et = get_from_memory::<u32>(fence_a + 0x128);
            if et != 0 && get_from_memory::<u8>(et + 0x190) != 0 {
                return 4;
            }
        }

        if status == 0 {
            if fence_b != 0 && dir_ba != -1 {
                unsafe { BFTILE_REMOVE_EDGE.original()(tile_b_ptr as *const u32, dir_ba as *const u32) };
            }
            if fence_a != 0 && opp_dir != -1 {
                unsafe { BFTILE_REMOVE_EDGE.original()(tile_a_ptr as *const u32, opp_dir as *const u32) };
            }

            let cost_forward = if keeper_ptr != 0 {
                (unsafe { BFUNIT_GET_PATH_COST.original()(keeper_ptr as *const u32, tile_a_ptr as i32, tile_b_ptr as i32) }) as i32
            } else {
                infinite_cost
            };
            let cost_backward = if keeper_ptr != 0 {
                (unsafe { BFUNIT_GET_PATH_COST.original()(keeper_ptr as *const u32, tile_b_ptr as i32, tile_a_ptr as i32) }) as i32
            } else {
                infinite_cost
            };
            let max_cost = std::cmp::max(cost_forward, cost_backward);
            if !out_cost.is_null() {
                unsafe { *out_cost = max_cost };
            }

            if fence_b != 0 && dir_ba != -1 {
                unsafe { BFTILE_ADD_EDGE.original()(tile_b_ptr as *const u32, fence_b as *const u32, dir_ba as u32) };
            }
            if fence_a != 0 && opp_dir != -1 {
                unsafe { BFTILE_ADD_EDGE.original()(tile_a_ptr as *const u32, fence_a as *const u32, opp_dir as u32) };
            }

            if max_cost < infinite_cost - 1 {
                let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
                let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
                if tile_a.entity_ptr != 0 || tile_b.entity_ptr != 0 {
                    status = 3;
                }
                let hab_a_ptr = self.get_habitat_ptr(tile_a.pos.x, tile_a.pos.y);
                if hab_a_ptr != 0 {
                    let hab_a = unsafe { ref_from_memory::<ZTHabitat>(hab_a_ptr) };
                    if hab_a.is_tank() {
                        let o0 = get_from_memory::<u32>(tile_a_ptr + 0x4);
                        let o1 = get_from_memory::<u32>(tile_a_ptr + 0x8);
                        let o2 = get_from_memory::<u32>(tile_a_ptr + 0xc);
                        let o3 = get_from_memory::<u32>(tile_a_ptr + 0x10);
                        if o0 != 0 || o1 != 0 || o2 != 0 || o3 != 0 {
                            status = 3;
                            if !out_cost.is_null() {
                                unsafe { *out_cost = infinite_cost };
                            }
                        }
                    }
                }
            } else {
                status = 4;
            }
        }
        status
    }

    /// Ports `ZTHabitatMgr::getNextFencePair` (`ZTHabitatMgr_getNextFencePair.c`, `GET_NEXT_FENCE_PAIR` at `0x00487f34`).
    pub fn get_next_fence_pair(
        &self,
        habitat_ptr: u32,
        cand_a: &mut u32,
        cand_b: &mut u32,
        check_in_zoo: bool,
    ) -> bool {
        let world = globals().ztworldmgr();
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 512 {
                return false;
            }
            if *cand_a == 0 || *cand_b == 0 {
                return false;
            }
            let mut found_flag = false;
            let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(*cand_b as i32, *cand_a as i32) };
            let mut rot_dir = match dir {
                0 => 6,
                2 => 0,
                4 => 2,
                6 => 4,
                _ => return false,
            };
            let mut curr_a = get_neighbour_raw(world, *cand_a, rot_dir);
            let mut curr_b = get_neighbour_raw(world, *cand_b, rot_dir);

            // Handle corners where outer neighbour is null
            let mut corner_iters = 0;
            while curr_b == 0 {
                corner_iters += 1;
                if corner_iters > 16 {
                    return false;
                }
                let mut search_node = *cand_a;
                rot_dir = (rot_dir + 2) & 7;
                loop {
                    search_node = get_neighbour_raw(world, search_node, rot_dir);
                    curr_a = search_node;
                    if search_node == 0 {
                        break;
                    }
                    let sn_tile = get_from_memory::<BFTile>(search_node);
                    let sn_hab = self.get_habitat_ptr(sn_tile.pos.x, sn_tile.pos.y);
                    if sn_hab == habitat_ptr {
                        *cand_a = search_node;
                    } else {
                        break;
                    }
                }
                if search_node != 0 {
                    let sn_tile = get_from_memory::<BFTile>(search_node);
                    let sn_hab = self.get_habitat_ptr(sn_tile.pos.x, sn_tile.pos.y);
                    if sn_hab != habitat_ptr {
                        curr_a = *cand_a;
                        curr_b = search_node;
                    }
                }
            }

            if curr_a != 0 && curr_b != 0 {
                let tile_a_obj = get_from_memory::<BFTile>(curr_a);
                let tile_b_obj = get_from_memory::<BFTile>(curr_b);
                let hab_a = self.get_habitat_ptr(tile_a_obj.pos.x, tile_a_obj.pos.y);
                let hab_b = self.get_habitat_ptr(tile_b_obj.pos.x, tile_b_obj.pos.y);

                if hab_a == habitat_ptr {
                    if hab_b != habitat_ptr {
                        *cand_a = curr_a;
                        *cand_b = curr_b;
                    } else {
                        *cand_b = curr_b;
                    }
                } else if hab_b == habitat_ptr {
                    if hab_a != habitat_ptr {
                        // skip
                    } else {
                        *cand_b = curr_b;
                    }
                } else {
                    *cand_b = curr_a;
                }
                found_flag = true;
            }

            if !check_in_zoo || low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.original()(*cand_b as *const u32, 0) }) {
                return found_flag;
            }
        }
    }

    /// Thin wrapper around [`Self::get_next_fence_pair`] for [`Self::place_gate`]'s own scan loop.
    pub(crate) fn advance_fence_pair(&self, habitat_ptr: u32, cand_a: &mut u32, cand_b: &mut u32) -> bool {
        self.get_next_fence_pair(habitat_ptr, cand_a, cand_b, true)
    }

    /// Ports `ZTHabitatMgr::snapTankWallsInward` (`ZTHabitatMgr_snapTankWallsInward.c`/`.asm`) - read at
    /// the `.asm` level throughout, since the C decompile's own struct-offset math for the fence-family/
    /// tank-wall type checks is garbled the same way [`Self::check_amphibious_neighbor`]'s own doc comment
    /// documents; [`is_wall`]'s/[`is_tank_wall`]'s existing `+0x192`/`+0x193` byte offsets on a fence's own
    /// `entity_type` are reused here, confirmed identical at the `.asm` level.
    ///
    /// Clears `tank_ptr`'s own wall vector ([`ZTTANKEXHIBIT_CLEAR_WALL_VECTOR`]), then snapshots its
    /// [`ZTHabitat::boundary_tile_pairs_begin`]/`_end` vector (the same
    /// [snapshot][Self::snapshot_boundary_tile_pairs]/iterate shape [`Self::update_amphibious_neighbors`]/
    /// [`Self::update_show_neighbors`]/[`Self::do_show_check`] already use - real vanilla builds this same
    /// defensive copy up front too) and, for each `(tile_a, tile_b)` boundary pair:
    ///
    /// 1. Resolves `fence_a` (the fence on `tile_a`'s own slot facing `tile_b`, via
    ///    [`BFMAP_GET_DIRECTION_0`] + [`ZTHabitat::fence_slot_by_index`], gated on
    ///    [`RVA_FENCE_TYPE_CHECK_ARG`]) and `fence_b` (the mirror, on `tile_b` facing `tile_a`) the same
    ///    way.
    /// 2. If `fence_a` is present and **not** [`is_tank_wall`], and `fence_b` **is** [`is_tank_wall`]:
    ///    resolves the tank occupying `tile_b` ([`Self::get_tank`]). If none, [swaps][swap_fence_positions]
    ///    `fence_a`'s and `fence_b`'s own world position, cached virtual position, and rotation in place -
    ///    each keeps its own identity, but the two fences trade places. If a tank *is* found, instead
    ///    removes `fence_a` from the world ([`BFWORLDMGR_REMOVE_ENTITY`]) and replaces it with
    ///    [`Self::create_double_fence`]`(fence_b)`.
    /// 3. Else, if `fence_a` is absent and `fence_b` is present and [`is_tank_wall`]: resolves the habitat
    ///    occupying `tile_b`. While the game isn't mid-load ([`ZTUI::gameopts::loadInProgress`],
    ///    [`RVA_APP_INIT_SUCCESS_BASE`]`+0x441`) - if that habitat is missing or not a real tank
    ///    ([`ZTHabitat::do_tank_check`]), snaps `fence_b` back onto its own tile edge
    ///    ([`ZTFENCE_JUMP_TILE_EDGE`]) and posts a real map-editor undo action (action id `10`, the same
    ///    fixed debug label [`RVA_CREATE_DOUBLE_FENCE_UNDO_LABEL`] [`Self::create_double_fence`] uses) -
    ///    live UI, same class as that function's own undo action. If a real tank *is* found, replaces
    ///    `fence_a` with [`Self::create_double_fence`]`(fence_b)` instead.
    /// 4. Either way, both `fence_a`/`fence_b` (whichever ended up non-null) are
    ///    [validated][call_vtable_slot_with_u8] (`+0x84`, `true`).
    /// 5. Regardless of the above: re-resolves the fence on `tile_a`'s slot facing `tile_b` one more time
    ///    (a fresh lookup - real vanilla re-reads live tile memory here rather than reusing `fence_a`) and,
    ///    if it's both [`is_tank_wall`] and specifically tank-wall-typed
    ///    ([`RVA_TANK_WALL_TYPE_CHECK_ARG`]), registers it via [`ZTTANKEXHIBIT_ADD_TANK_WALL`].
    ///
    /// **Live-testing note**: like `fence_placed`/`fence_removed`/`place_gate`, this mutates real fence/
    /// tank state (including the position swap and the live undo-action UI in step 3) with no known
    /// synthetic-safe input - detoured for manual/interactive live verification, not covered by an
    /// automated live test.
    pub fn snap_tank_walls_inward(&self, tank_ptr: u32) {
        let base = get_module_base("zoo.exe") as u32;
        // Real vanilla sets this for the entire body (ZTHabitatMgr_snapTankWallsInward.c:44/213) - it's
        // the same fencePlaced/fenceRemoved reentrancy guard [`Self::replace_gate_with_fence`]/
        // [`Self::replace_fence_with_gate`] already set around their own conversions. Without it,
        // create_double_fence's/jump_tile_edge's own BFWorldMgr::addEntity call recurses straight back
        // into fence_placed while this loop is still mid-flight, and each such reentrant call can itself
        // conclude "disconnected from the zoo entrance" and spawn an extra, permanently-degenerate
        // habitat (empty boundary pairs, no resize/placeGate) - the exact "several duplicate tank
        // exhibits with no water/height controls" bug this fixes.
        save_to_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA, 1);
        unsafe { ZTTANKEXHIBIT_CLEAR_WALL_VECTOR.original()(tank_ptr as i32) };

        let habitat = unsafe { ref_from_memory::<ZTHabitat>(tank_ptr) };
        let pairs = Self::snapshot_boundary_tile_pairs(habitat.boundary_tile_pairs_begin, habitat.boundary_tile_pairs_end);

        let world_map_ptr = globals().ztworldmgr_ptr() as u32;
        let load_in_progress = get_from_memory::<u8>(base + RVA_APP_INIT_SUCCESS_BASE + 0x441) != 0;

        for (tile_a_ptr, tile_b_ptr) in pairs {
            // A boundary-tile-pair entry can hold a null tile pointer (e.g. mid-bulldoze on a shared tank
            // wall, or a freshly-constructed exhibit's not-yet-fully-populated pairs) - skipped
            // defensively, same guard `ZTHabitat::do_tank_check` already applies to this exact vector.
            if tile_a_ptr == 0 || tile_b_ptr == 0 {
                continue;
            }
            let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
            let tile_b = get_from_memory::<BFTile>(tile_b_ptr);

            let dir_ab = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
            let mut fence_a = if dir_ab != -1 {
                let f = ZTHabitat::fence_slot_by_index(&tile_a, dir_ab / 2);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            let dir_ba = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_b_ptr as i32, tile_a_ptr as i32) };
            let fence_b = if dir_ba != -1 {
                let f = ZTHabitat::fence_slot_by_index(&tile_b, dir_ba / 2);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            if fence_a != 0 {
                if !is_tank_wall(fence_a) && fence_b != 0 && is_tank_wall(fence_b) {
                    if self.get_tank(tile_b_ptr) == 0 {
                        swap_fence_positions(world_map_ptr, fence_a, fence_b);
                    } else {
                        unsafe { BFWORLDMGR_REMOVE_ENTITY.original()(world_map_ptr as *const u32, fence_a as *const u32, true) };
                        fence_a = ZTHabitatMgr::create_double_fence(fence_b);
                    }
                }
            } else if fence_b != 0 && is_tank_wall(fence_b) {
                let habitat_b_ptr = self.get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
                let is_real_tank = habitat_b_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) }.do_tank_check();
                if !load_in_progress {
                    if !is_real_tank {
                        unsafe { ZTFENCE_JUMP_TILE_EDGE.original()(fence_b as *const u32) };
                        let rotation: u32 = get_from_memory(fence_b + 0x12c);

                        let label_ptr = base + RVA_CREATE_DOUBLE_FENCE_UNDO_LABEL;
                        let mut len: u32 = 0;
                        while get_from_memory::<u8>(label_ptr + len) != 0 {
                            len += 1;
                        }
                        let buf_len = len + 1;
                        let buffer_ptr = unsafe { POOLALLOC_ALLOCATE.original()(buf_len) } as u32;
                        unsafe { MEMMOVE.original()(buffer_ptr as *const u32, label_ptr as *const u32, len) };
                        save_to_memory::<u8>(buffer_ptr + len, 0);

                        let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() };
                        unsafe {
                            ADD_UNDO_ACTION.original()(
                                mapview_ptr,
                                0xa,
                                0,
                                0,
                                std::ptr::null::<u32>(),
                                rotation,
                                0,
                                tile_a.pos.x as u32,
                                tile_a.pos.y as u32,
                                0,
                                std::ptr::null::<i32>(),
                                buffer_ptr as *const u32,
                                (buffer_ptr + len) as i32,
                                (buffer_ptr + buf_len) as i32,
                            )
                        };
                    } else {
                        fence_a = ZTHabitatMgr::create_double_fence(fence_b);
                    }
                }
            }

            if fence_a != 0 {
                unsafe { call_vtable_slot_with_u8(fence_a, 0x84, 1) };
            }
            if fence_b != 0 {
                unsafe { call_vtable_slot_with_u8(fence_b, 0x84, 1) };
            }

            let dir_ab2 = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
            if dir_ab2 != -1 {
                let tile_a_fresh = get_from_memory::<BFTile>(tile_a_ptr);
                let wall = ZTHabitat::fence_slot_by_index(&tile_a_fresh, dir_ab2 / 2);
                if wall != 0 && is_tank_wall(wall) && unsafe { entity_type_matches(wall, RVA_TANK_WALL_TYPE_CHECK_ARG) } {
                    unsafe { ZTTANKEXHIBIT_ADD_TANK_WALL.original()(tank_ptr as *const u32, wall as *const u32) };
                }
            }
        }
        save_to_memory::<u8>(base + GATE_CONVERSION_IN_PROGRESS_RVA, 0);
    }

    /// Ports `ZTHabitatMgr::createDoubleFence` (`ZTHabitatMgr_createDoubleFence.c`/`.asm`,
    /// `generated.rs`'s `CREATE_DOUBLE_FENCE`) - despite its decompile namespace, a plain free `stdcall`
    /// helper taking a single `ZTFence*` (real `.asm`: `RET 0x4`, no `this`-via-ECX at all), the same
    /// misnaming pattern already documented for [`Self::replace_gate_with_fence`]/
    /// [`Self::replace_fence_with_gate`]. The C decompile's own dozen `extraout_EAX`/`unaff_EBX`/
    /// `unaff_retaddr` artifacts are all the standard "register value read right after an untyped call"
    /// idiom already validated elsewhere in this file (e.g. `ZTHabitat::recalculate_characteristics`'s own
    /// note) - not real corruption, confirmed by reading the raw `.asm` line-by-line; the 2 "unidentified
    /// helpers" the roadmap doc previously flagged (`FUN_005fd10e`/`FUN_00629f1a`) are the same game-wide
    /// fatal-assert/exit pair already encountered and deliberately not reproduced in `zoostatus.rs`'s
    /// `calculate_sums` - dead code guarding an assert on a compile-time-constant non-empty debug string
    /// that can never actually fire.
    ///
    /// Given an existing fence (`fence_ptr`), builds a second fence on the *opposite* side of the tile
    /// boundary it sits on - used when a habitat boundary needs a fence visible from both sides. Steps
    /// through `fence_ptr`'s own rotation to find the neighbouring tile; if the rotation is real (not the
    /// `0xffffffff` "no direction" sentinel), computes the opposite direction and bails (`0`) if the
    /// neighbour already has a fence facing back ([`ZTHabitat::fence_slot_by_index`] at that slot).
    /// Otherwise snaps a position onto that edge ([`SNAP_TO_EDGE`]), converts it to a world position via
    /// the already-ported [`ZTWorldMgr::tile_to_world`], spawns a new fence of the same catalog type via
    /// the entity type's own `create` factory ([`call_entity_type_create`] on `fence_ptr`'s own
    /// `entity_type_ptr` at `+0x128`), gives it the standard visual direction-set treatment (a real
    /// `std::vector<ph_GXLLEAnimSet>`-style bounds-checked index read at `+0x74`/`+0x78`/`+0xcc`, feeding
    /// [`BFENTITY_DIR_TO_SET`] then [`GXMIXER_SET_SET`]), positions it in world space
    /// ([`WORLD_TO_VIRTUAL_0`] + [`BFENTITY_SET_WORLD_POS`]), adds it to the world
    /// ([`BFWORLDMGR_ADD_ENTITY`]), and registers an undo action for the map editor
    /// ([`ADD_UNDO_ACTION`]) - this last part makes this function, independent of
    /// [`Self::snap_tank_walls_inward`] (its only known caller), a live-UI function in its own right: same
    /// "unsafe for automated testing" class as `nameHabitat`/`morphExhibit`. **Deliberately not detoured**
    /// for that reason - see the standalone comment near `MOVE_GATE_TO_1` in the detour module below.
    ///
    /// **Two real-vanilla quirks reproduced faithfully rather than "fixed", with one deliberate
    /// deviation:** (1) real vanilla calls `ZTFence::makeFence` unconditionally even when the factory call
    /// fails or returns a non-fence type (leaving its own `this_00` null) - genuinely dead in practice
    /// since the factory always succeeds for a real fence's own catalog type, but this port guards it
    /// (`return 0` instead) rather than handing a null pointer to a real vanilla function that itself does
    /// no null check, matching this file's own established "dead in practice, defend anyway" convention
    /// (see [`ZTHabitat::get_outermost_tank`]); (2) the undo-action description buffer is built from a
    /// fixed global debug string ([`RVA_CREATE_DOUBLE_FENCE_UNDO_LABEL`]) via a real
    /// `PoolAlloc::allocate`/`memmove`, handed directly to `ZTMapView::addUndoAction` - a real, transient
    /// allocator use never touched again by this function, so safe as a `PoolAlloc`-backed buffer per this
    /// file's own convention (see [`construct_and_acquire_sound`] for the same "real allocator, immediately
    /// handed to vanilla" pattern) - reproduced faithfully, unguarded, since it can't meaningfully fail.
    pub fn create_double_fence(fence_ptr: u32) -> u32 {
        if fence_ptr == 0 {
            return 0;
        }
        let world = globals().ztworldmgr();
        let world_map_ptr = globals().ztworldmgr_ptr() as u32;

        let rotation: u32 = get_from_memory(fence_ptr + 0x12c);
        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(fence_ptr as *const u32) } as u32;
        let neighbour_ptr = if tile_ptr != 0 { get_neighbour_ptr(world, tile_ptr, Direction::from(rotation)) } else { 0 };
        if neighbour_ptr == 0 {
            // Real vanilla reads the neighbour tile's own position/fence-slot fields unconditionally a
            // few lines further down (both to check for an existing fence facing back and to compute the
            // new fence's own world position), with no null guard anywhere - dead in practice (a fence
            // eligible for doubling always has a real neighbour tile), guarded defensively rather than
            // reproduced, same convention as (1) above.
            return 0;
        }
        let neighbour_tile = get_from_memory::<BFTile>(neighbour_ptr);

        let direction = if rotation == 0xffff_ffff {
            0xffff_ffff
        } else {
            let opposite = rotation.wrapping_sub(4) & 7;
            if ZTHabitat::fence_slot_by_index(&neighbour_tile, (opposite as i32) / 2) != 0 {
                return 0;
            }
            opposite
        };

        let mut snapped = [0i32; 3];
        unsafe { SNAP_TO_EDGE.original()(snapped.as_mut_ptr() as *const u32, direction) };
        let world_pos = world.tile_to_world(neighbour_tile.pos, IVec3::new(snapped[0], snapped[1], snapped[2]));

        let entity_type_ptr: u32 = get_from_memory(fence_ptr + 0x128);
        let new_fence_ptr = unsafe { call_entity_type_create(entity_type_ptr) };
        if new_fence_ptr == 0 || !unsafe { entity_type_matches(new_fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
            // Real vanilla calls `ZTFence::makeFence` unconditionally here even on a null/wrong-type
            // `this_00` - see this method's own doc comment, deviation (1).
            return 0;
        }
        unsafe { ZTFENCE_MAKE_FENCE.original()(new_fence_ptr as *const u32) };

        let gxmixer_ptr = new_fence_ptr + 0x74;
        let anim_index: u32 = get_from_memory(new_fence_ptr + 0xcc);
        let vec_begin: u32 = get_from_memory(new_fence_ptr + 0x74);
        let vec_end: u32 = get_from_memory(new_fence_ptr + 0x78);
        let anim_set_ptr: u32 = if vec_end > vec_begin && anim_index < (vec_end - vec_begin) / 0xc {
            get_from_memory(vec_begin + anim_index * 0xc)
        } else {
            0
        };
        let mut flip: bool = false;
        let set_index = unsafe {
            BFENTITY_DIR_TO_SET.original()(
                new_fence_ptr as *const u32,
                direction,
                &mut flip as *mut bool as *const bool,
                anim_set_ptr as *const u32,
                std::ptr::null::<u32>(),
            )
        };
        unsafe { GXMIXER_SET_SET.original()(gxmixer_ptr as *const u32, set_index, flip as u8) };

        let mut virtual_pos = [0i32; 3];
        unsafe {
            WORLD_TO_VIRTUAL_0.original()(world_map_ptr as *const u32, virtual_pos.as_mut_ptr() as *const i32, &world_pos as *const IVec3 as *const i32)
        };
        save_to_memory(new_fence_ptr + 0xb4, virtual_pos[0]);
        save_to_memory(new_fence_ptr + 0xb8, virtual_pos[1]);
        save_to_memory(new_fence_ptr + 0xbc, virtual_pos[2]);
        unsafe { BFENTITY_SET_WORLD_POS.original()(new_fence_ptr as *const u32, &world_pos as *const IVec3 as *const u32) };
        save_to_memory(new_fence_ptr + 0x12c, direction);

        unsafe { BFWORLDMGR_ADD_ENTITY.original()(world_map_ptr as *const u32, new_fence_ptr as *const u32) };

        let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() };
        let base = get_module_base("zoo.exe") as u32;
        let label_ptr = base + RVA_CREATE_DOUBLE_FENCE_UNDO_LABEL;
        let mut len: u32 = 0;
        while get_from_memory::<u8>(label_ptr + len) != 0 {
            len += 1;
        }
        let buf_len = len + 1;
        let buffer_ptr = unsafe { POOLALLOC_ALLOCATE.original()(buf_len) } as u32;
        unsafe { MEMMOVE.original()(buffer_ptr as *const u32, label_ptr as *const u32, len) };
        save_to_memory::<u8>(buffer_ptr + len, 0);
        unsafe {
            ADD_UNDO_ACTION.original()(
                mapview_ptr,
                1,
                new_fence_ptr as i32,
                0,
                std::ptr::null::<u32>(),
                0xffff_ffff,
                0,
                0,
                0,
                0,
                std::ptr::null::<i32>(),
                buffer_ptr as *const u32,
                (buffer_ptr + len) as i32,
                (buffer_ptr + buf_len) as i32,
            )
        };

        new_fence_ptr
    }

    /// Ports `ZTHabitatMgr::replaceGate` (`ZTHabitatMgr_replaceGate.c`/`.asm`): converts
    /// [`Self::pending_gate_fence_ptr`] back into a plain fence via [`Self::replace_gate_with_fence`] -
    /// but only after confirming it's still a live entity in `GLOBAL_ZTWorldMgr`'s own `entity_array`
    /// (real vanilla's own defensive re-check; see [`crate::ztworldmgr::ZTWorldMgr::entity_array`]) -
    /// then clears the field. If the field is set but genuinely isn't found among a *non-empty*
    /// `entity_array` (real vanilla's own inconsistency-guard branch, never attested by any real call
    /// site), calls through the unidentified [`FUN_005B66D7`] and returns **without** clearing the field,
    /// matching real vanilla's own early return exactly.
    pub fn replace_gate(&self) {
        let self_addr = self as *const Self as u32;
        let gate_ptr = self.pending_gate_fence_ptr;
        if gate_ptr != 0 {
            let world = globals().ztworldmgr();
            let mut entities = world.entity_array().peekable();
            if entities.peek().is_some() {
                if entities.any(|e| e == gate_ptr) {
                    Self::replace_gate_with_fence(gate_ptr);
                } else {
                    unsafe { FUN_005B66D7.original()() };
                    return;
                }
            }
        }
        save_to_memory::<u32>(self_addr + 0x54, 0);
    }

    /// Ports `ZTHabitatMgr::habitatTileChanged` (`ZTHabitatMgr_habitatTileChanged.c`/`.asm`, both
    /// confirmed identical - no CONCAT/garbled-offset surprises). For every tile within a 7x7 square
    /// centred on `tile_ptr` (`±3` in both axes, matching real vanilla's own inclusive bounds), reads
    /// that tile's own grid-cell row (see [`Self::get_habitat_cell_addr`]'s own updated doc comment for
    /// the row's real 10-slot shape) and sets [`ZTHabitat::neighbor_dirty`] on every non-null pointer
    /// found in the row's 8 "extra" slots (offsets `0x4..0x24`, skipping the primary occupant at offset
    /// `0` and the row's unread final slot at `0x24`). Out-of-range neighbor tiles are silently skipped
    /// via [`Self::get_habitat_cell_addr`]'s own `None` - real vanilla's separate `map_x_size`/
    /// `map_y_size`/`tile_array != 0` guard collapses to the same effect once a zoo is loaded (the only
    /// state this is ever called in), so is not reproduced as a second, redundant check here.
    pub fn habitat_tile_changed(&self, tile_ptr: u32) {
        let tile = get_from_memory::<BFTile>(tile_ptr);
        for x in (tile.pos.x - 3)..=(tile.pos.x + 3) {
            for y in (tile.pos.y - 3)..=(tile.pos.y + 3) {
                let Some(row_addr) = self.get_habitat_cell_addr(x, y) else {
                    continue;
                };
                for slot in 0..8u32 {
                    let neighbor_ptr: u32 = get_from_memory(row_addr + 0x4 + slot * 4);
                    if neighbor_ptr != 0 {
                        save_to_memory::<u8>(neighbor_ptr + 0x25, 1);
                    }
                }
            }
        }
    }

    /// Ports `ZTHabitatMgr::terrainTileChanged` (`ZTHabitatMgr_terrainTileChanged.c`): a thin gate in
    /// front of [`Self::habitat_tile_changed`] - only calls through when `tile_ptr`'s own occupying
    /// habitat exists and has its [`ZTHabitat::unknown_flag_0x2c`] set. `tile_ptr == 0` is real vanilla's
    /// own explicit no-op branch (not merely a defensive addition here).
    pub fn terrain_tile_changed(&self, tile_ptr: u32) {
        if tile_ptr == 0 {
            return;
        }
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let habitat_ptr = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.unknown_flag_0x2c != 0 {
            self.habitat_tile_changed(tile_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::sceneryEntityChange` (`ZTHabitatMgr_sceneryEntityChange.c`): gated on
    /// [`Self::scenery_entity_change_suspended`] (never set anywhere in this pass's own scope, so always
    /// a no-op gate for now). When `tile_ptr`'s own occupying habitat has [`ZTHabitat::unknown_flag_0x2c`]
    /// set, delegates entirely to [`Self::habitat_tile_changed`] (same as [`Self::terrain_tile_changed`]);
    /// otherwise walks the habitat's own `viewing_areas_begin..viewing_areas_end` vector and sets the same
    /// [`ZTHabitat::neighbor_dirty`] byte on every entry - real vanilla's own `*(undefined1*)(*piVar2 +
    /// 0x25) = 1` loop, same shape as [`ZTHabitat::update`]'s own viewing-area walk.
    pub fn scenery_entity_change(&self, tile_ptr: u32) {
        if self.scenery_entity_change_suspended != 0 || tile_ptr == 0 {
            return;
        }
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let habitat_ptr = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr == 0 {
            return;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        if habitat.unknown_flag_0x2c != 0 {
            self.habitat_tile_changed(tile_ptr);
            return;
        }
        let mut viewing_area_entry = habitat.viewing_areas_begin;
        while viewing_area_entry != habitat.viewing_areas_end {
            let viewing_area_ptr: u32 = get_from_memory(viewing_area_entry);
            save_to_memory::<u8>(viewing_area_ptr + 0x25, 1);
            viewing_area_entry += 4;
        }
    }

    /// Ports `ZTHabitatMgr::entityAboutToBeRemoved` (`ZTHabitatMgr_entityAboutToBeRemoved.c`/`.asm`):
    /// resolves the entity's occupying tile/habitat, marks it dirty, and calls
    /// [`BEFORE_ENTITY_CHANGE`] on it. For a tank habitat containing a scenery or animal entity, also
    /// calls [`BEFORE_ENTITY_CHANGE`] on every amphibious neighbor - collected into a `Vec` up front
    /// (rather than porting vanilla's own defensive PoolAlloc tree copy) since `BEFORE_ENTITY_CHANGE` is
    /// an un-ported vanilla function that could itself mutate the live tree. `BEFORE_ENTITY_CHANGE`'s own
    /// real body (tied to an un-ported species-rating cache) immediately clears `characteristics_dirty`/
    /// `species_list_dirty` back to `0` as part of its own work (confirmed live - the dirty write here is
    /// real, but does not outlive the call), so no test can observe the dirty flags as still set once this
    /// method returns.
    pub fn entity_about_to_be_removed(&self, entity_ptr: u32) {
        if entity_ptr == 0 {
            return;
        }
        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(entity_ptr as *const u32) } as u32;
        if tile_ptr == 0 {
            return;
        }
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let habitat_ptr = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr == 0 {
            // Real vanilla null-derefs here (unreachable in practice); guarded instead, matching this
            // file's own established convention (e.g. `scenery_entity_change`'s identical guard).
            return;
        }
        let habitat = unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) };
        habitat.set_dirty_characteristics();
        habitat.species_list_dirty = 1;
        Self::before_entity_change(habitat_ptr);
        if !habitat.is_tank() {
            return;
        }
        let is_scenery_or_animal =
            unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || entity_type_matches(entity_ptr, RVA_ANIMAL_TYPE_CHECK) };
        if !is_scenery_or_animal {
            return;
        }
        let neighbors: Vec<u32> = walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for neighbor_ptr in neighbors {
            Self::before_entity_change(neighbor_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::entityRemoved` (`ZTHabitatMgr_entityRemoved.c`/`.asm`): marks the tile's
    /// habitat dirty, delegates to [`Self::scenery_entity_change`], then calls [`AFTER_ENTITY_CHANGE`] on
    /// it. For a tank habitat containing a scenery or animal entity, also re-marks and calls
    /// [`AFTER_ENTITY_CHANGE`] on every amphibious neighbor. Takes the already-resolved
    /// `BFEntityType*` directly (via [`type_check`]) rather than an entity pointer - confirmed via both
    /// `.c` and `.asm` that this is what the real caller hands in, unlike the `*AboutToBe*` siblings. Same
    /// caveat as [`Self::entity_about_to_be_removed`]'s own doc comment: `AFTER_ENTITY_CHANGE`'s own real
    /// body clears the dirty flags this method just set, so their post-call state is not itself testable.
    pub fn entity_removed(&self, tile_ptr: u32, entity_type_ptr: u32) {
        if tile_ptr == 0 {
            return;
        }
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let habitat_ptr = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr == 0 {
            return;
        }
        let habitat = unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) };
        habitat.set_dirty_characteristics();
        habitat.species_list_dirty = 1;
        self.scenery_entity_change(tile_ptr);
        unsafe {
            AFTER_ENTITY_CHANGE.original()(habitat_ptr as *const u32, entity_type_ptr as *const u32, tile_ptr as *const u32, true, 0);
        }
        if !habitat.is_tank() {
            return;
        }
        let is_scenery_or_animal =
            entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) };
        if !is_scenery_or_animal {
            return;
        }
        let neighbors: Vec<u32> = walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for neighbor_ptr in neighbors {
            let neighbor = unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) };
            neighbor.set_dirty_characteristics();
            neighbor.species_list_dirty = 1;
            unsafe {
                AFTER_ENTITY_CHANGE.original()(neighbor_ptr as *const u32, entity_type_ptr as *const u32, tile_ptr as *const u32, true, 1);
            }
        }
    }

    /// Ports `ZTHabitatMgr::entityAboutToBePlaced` (`ZTHabitatMgr_entityAboutToBePlaced.c`/`.asm`): same
    /// shape as [`Self::entity_about_to_be_removed`], but resolves the tile via [`WORLD_TO_TILE`] against
    /// the entity's own `pos` field (`entity_ptr + 0x114`, [`crate::ztworldmgr::BFEntity::pos`]) rather
    /// than via [`BFENTITY_GET_TILE`] - the entity has no tile assigned yet at this point.
    pub fn entity_about_to_be_placed(&self, entity_ptr: u32) {
        let mut tile_xyz = [0i32; 3];
        unsafe { WORLD_TO_TILE.original()(tile_xyz.as_mut_ptr() as *const i32, (entity_ptr + 0x114) as *const i32) };
        let habitat_ptr = self.get_habitat_ptr(tile_xyz[0], tile_xyz[1]);
        if habitat_ptr == 0 {
            return;
        }
        let habitat = unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) };
        habitat.set_dirty_characteristics();
        habitat.species_list_dirty = 1;
        Self::before_entity_change(habitat_ptr);
        if !habitat.is_tank() {
            return;
        }
        let is_scenery_or_animal =
            unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || entity_type_matches(entity_ptr, RVA_ANIMAL_TYPE_CHECK) };
        if !is_scenery_or_animal {
            return;
        }
        let neighbors: Vec<u32> = walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for neighbor_ptr in neighbors {
            Self::before_entity_change(neighbor_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::entityPlaced` (`ZTHabitatMgr_entityPlaced.c`/`.asm`): same shape as
    /// [`Self::entity_removed`], but resolves the tile via [`WORLD_TO_TILE`] (bounds-checked against
    /// [`crate::ztworldmgr::ZTWorldMgr::map_x_size`]/`map_y_size`, matching vanilla's own check - a
    /// failure here fully skips, unlike [`Self::entity_about_to_be_placed`]'s null-deref-shaped guard)
    /// and passes `null`/`false` (not the real tile/`true`) as [`AFTER_ENTITY_CHANGE`]'s 3rd/4th
    /// arguments - re-derived from this call site's own `.asm` push order independently of
    /// `entity_removed`'s, not assumed symmetric.
    pub fn entity_placed(&self, entity_ptr: u32) {
        let mut tile_xyz = [0i32; 3];
        unsafe { WORLD_TO_TILE.original()(tile_xyz.as_mut_ptr() as *const i32, (entity_ptr + 0x114) as *const i32) };
        let (tile_x, tile_y) = (tile_xyz[0], tile_xyz[1]);
        let world_mgr = globals().ztworldmgr();
        if tile_x < 0 || tile_y < 0 || tile_x as u32 >= world_mgr.map_x_size || tile_y as u32 >= world_mgr.map_y_size {
            return;
        }
        let tile_ptr = world_mgr.get_tile_ptr(tile_x as u32, tile_y as u32);
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let habitat_ptr = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr == 0 {
            return;
        }
        let habitat = unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) };
        habitat.set_dirty_characteristics();
        habitat.species_list_dirty = 1;
        self.scenery_entity_change(tile_ptr);
        let entity_type_ptr = get_from_memory::<u32>(entity_ptr + 0x128);
        unsafe {
            AFTER_ENTITY_CHANGE.original()(habitat_ptr as *const u32, entity_type_ptr as *const u32, std::ptr::null(), false, 0);
        }
        if !habitat.is_tank() {
            return;
        }
        let is_scenery_or_animal =
            unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) || entity_type_matches(entity_ptr, RVA_ANIMAL_TYPE_CHECK) };
        if !is_scenery_or_animal {
            return;
        }
        let neighbors: Vec<u32> = walk_neighbor_tree(habitat.amphibious_neighbors_head).map(|node| get_from_memory::<u32>(node + 0x10)).collect();
        for neighbor_ptr in neighbors {
            let neighbor = unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) };
            neighbor.set_dirty_characteristics();
            neighbor.species_list_dirty = 1;
            unsafe {
                AFTER_ENTITY_CHANGE.original()(neighbor_ptr as *const u32, entity_type_ptr as *const u32, std::ptr::null(), false, 1);
            }
        }
    }

    /// Ports `ZTHabitatMgr::beforeEntityChange` (`ZTHabitatMgr_beforeEntityChange.c`/`.asm`): real
    /// vanilla is a free `stdcall` helper taking a habitat pointer directly, not a `ZTHabitatMgr`
    /// instance method (`generated.rs`'s own `BEFORE_ENTITY_CHANGE` has no `this`) - called as an
    /// associated function here, matching [`Self::replace_gate_with_fence`]'s own precedent for the same
    /// shape. Clears real vanilla's own single, separate, file-scope `map<int,float>`
    /// (`DAT_0063b998`/`_99c`) at the top of every call - architecturally distinct from
    /// [`SPECIES_RATING_CACHE`] above, not a second producer/consumer of it, per
    /// `species-rating-cache-identification-handover.md`. Unless `habitat_ptr` is the "world" habitat
    /// ([`ZTHabitat::unknown_flag_0x2c`] set), calls [`get_species_rating`] for every one of its current
    /// [`ZTHabitat::surrounding_species`]. Real vanilla stores each result back into that just-cleared
    /// map, but nothing else anywhere in the decompile corpus ever reads it afterward (confirmed in the
    /// handover doc) - the whole function's only observable effect is the `getSpeciesRating` calls
    /// themselves, so this port performs the same calls in the same order without modeling the
    /// write-only map.
    pub fn before_entity_change(habitat_ptr: u32) {
        if habitat_ptr == 0 {
            return;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        if habitat.unknown_flag_0x2c != 0 {
            return;
        }
        for species_ptr in habitat.surrounding_species() {
            let species_key = get_from_memory::<i32>(species_ptr + 0x1ec);
            unsafe { get_species_rating(habitat_ptr, species_key) };
        }
    }

    /// Ports `ZTHabitatMgr::terrainAboutToBeChanged` (`ZTHabitatMgr_terrainAboutToBeChanged.c`/`.asm`):
    /// the producer half of the species-rating-cache pair [`Self::terrain_changed`] consumes - see
    /// `species-rating-cache-identification-handover.md`. Omits real vanilla's own bracketing
    /// `GetTickCount()` calls (perf-timing only, no functional effect).
    ///
    /// Clears [`SPECIES_RATING_CACHE`], then scans the `[x, x+size)` x `[y, y+size)` tile square (via
    /// [`Self::get_habitat_ptr`]) collecting each distinct, non-"world"
    /// ([`ZTHabitat::unknown_flag_0x2c`] clear) habitat found. On first encountering each one, marks it
    /// dirty ([`ZTHabitat::set_dirty_characteristics`], `species_list_dirty = 1`) and, if it's a tank
    /// ([`ZTHabitat::is_tank`]), calls [`ZTHabitat::reset_unit_ai`] - matching real vanilla's own
    /// `isTank` (`+0x20`) / `resetUnitAI` (`+0x34`) vtable-slot pair, called directly here since both are
    /// already fully ported.
    ///
    /// For each distinct habitat found, builds a fresh `species_id -> rating` map from every one of its
    /// current [`ZTHabitat::surrounding_species`] (via [`get_species_rating`], overwriting on a repeated
    /// species key - matches real vanilla's own unconditional overwrite-after-lookup, not a
    /// skip-if-present) and pushes it onto [`SPECIES_RATING_CACHE`]. Real vanilla builds this as a real
    /// RB-tree and deep-copies it twice (a pass-by-value idiom, see the handover doc's Follow-up 2) into
    /// the persisted vector before freeing both scratch copies; functionally equivalent to building the
    /// map once here, since nothing else in the decompile corpus ever touches this cache (confirmed in
    /// the handover doc) - see [`SPECIES_RATING_CACHE`]'s own doc comment for why that makes an
    /// independent Rust-side store safe.
    pub fn terrain_about_to_be_changed(&self, x: i32, y: i32, size: i32) {
        let mut cache = SPECIES_RATING_CACHE.lock().unwrap();
        cache.clear();

        let mut habitat_ptrs: Vec<u32> = Vec::new();
        for tile_x in x..x + size {
            for tile_y in y..y + size {
                let habitat_ptr = self.get_habitat_ptr(tile_x, tile_y);
                if habitat_ptr == 0 {
                    continue;
                }
                if unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.unknown_flag_0x2c != 0 {
                    continue;
                }
                if habitat_ptrs.contains(&habitat_ptr) {
                    continue;
                }
                habitat_ptrs.push(habitat_ptr);

                let habitat = unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) };
                habitat.set_dirty_characteristics();
                habitat.species_list_dirty = 1;
                if habitat.is_tank() {
                    habitat.reset_unit_ai();
                }
            }
        }

        for habitat_ptr in habitat_ptrs {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
            let mut ratings = HashMap::new();
            for species_ptr in habitat.surrounding_species() {
                let species_key = get_from_memory::<i32>(species_ptr + 0x1ec);
                let rating = unsafe { get_species_rating(habitat_ptr, species_key) };
                ratings.insert(species_key, rating);
            }
            cache.push(SpeciesRatingCacheEntry { habitat_ptr, ratings });
        }
    }

    /// Ports `ZTHabitatMgr::terrainChanged` (`ZTHabitatMgr_terrainChanged.c`/`.asm`): the consumer half
    /// of the species-rating-cache pair [`Self::terrain_about_to_be_changed`] populates. Gated on
    /// [`RVA_GAME_PAUSED_FLAG`] (a general pause/dialog-blocking flag, not cache-specific - see the
    /// handover doc). Omits real vanilla's own dead-in-practice `GLOBAL_ZTApp` lazy-init guard (would
    /// only fire before the app singleton is ever set, unreachable once any terrain-changed event can
    /// occur - same class of dead guard this file already documents for other functions, e.g.
    /// [`Self::reset_unit_ai`]'s own doc comment) and its bracketing `GetTickCount()` perf-timing calls.
    ///
    /// For each entry in [`SPECIES_RATING_CACHE`] (populated by the last
    /// [`Self::terrain_about_to_be_changed`] run - **not** refreshed here; a habitat/species pair whose
    /// real rating has drifted from its cached snapshot keeps re-triggering every tick until the next
    /// terrain change resets the cache, matching real vanilla's own behavior, quirks included): marks
    /// the habitat dirty ([`ZTHabitat::set_dirty_characteristics`], `species_list_dirty = 1`), then for
    /// every one of its current [`ZTHabitat::surrounding_species`] compares a freshly computed
    /// [`get_species_rating`] against the cached value (a first-seen species - not part of the last
    /// snapshot - is inserted into the cache at `0.0` and compared against that, matching real vanilla's
    /// own insert-if-absent shape), calling real vanilla `ZTHabitat::generateFaces` (smile on an
    /// increase, frown on a decrease) and OR-ing its own return into this tick's overall smile/frown
    /// flag. Real vanilla never writes the fresh value back into the cache after comparing - ported
    /// faithfully, not "fixed".
    ///
    /// After each habitat's species loop, calls the already-ported
    /// [`Self::update_amphibious_neighbors`]/[`Self::update_show_neighbors`] and sets the
    /// still-unidentified `field_0x30` byte (`this_00->field_0x30 = 1` in the decompile - distinct from
    /// the already-named `species_list_dirty` at `+0x31`; no reader found anywhere in this pass's own
    /// scope). Finally plays `ZTWorldMgr::playSmileSound`/`playFrownSound` if any habitat's loop set the
    /// corresponding flag.
    pub fn terrain_changed(&self) {
        if get_from_memory::<u8>(get_module_base("zoo.exe") as u32 + RVA_GAME_PAUSED_FLAG) != 0 {
            return;
        }

        let mut smile = false;
        let mut frown = false;
        let mut habitat_ptrs: Vec<u32> = Vec::new();

        {
            // Scoped so `SPECIES_RATING_CACHE`'s lock is released before the neighbor-update calls
            // below, which reach into arbitrary un-ported real vanilla code - `std::sync::Mutex` is not
            // reentrant, so holding the lock across a call that (directly or indirectly) re-enters
            // `terrain_changed`/`terrain_about_to_be_changed` on the same thread would self-deadlock
            // silently (no exception, looks like a hang) rather than panic.
            let mut cache = SPECIES_RATING_CACHE.lock().unwrap();
            for entry in cache.iter_mut() {
                if entry.habitat_ptr == 0 {
                    continue;
                }
                habitat_ptrs.push(entry.habitat_ptr);
                let habitat = unsafe { mut_from_memory::<ZTHabitat>(entry.habitat_ptr) };
                habitat.set_dirty_characteristics();
                habitat.species_list_dirty = 1;

                for species_ptr in habitat.surrounding_species() {
                    let species_key = get_from_memory::<i32>(species_ptr + 0x1ec);
                    let fresh_rating = unsafe { get_species_rating(entry.habitat_ptr, species_key) };
                    let cached_rating = *entry.ratings.entry(species_key).or_insert(0.0);
                    if cached_rating < fresh_rating {
                        let generated =
                            unsafe { GENERATE_FACES.original()(entry.habitat_ptr as *const u32, species_ptr as *const u32, true, std::ptr::null()) };
                        smile |= generated;
                    } else if fresh_rating < cached_rating {
                        let generated =
                            unsafe { GENERATE_FACES.original()(entry.habitat_ptr as *const u32, species_ptr as *const u32, false, std::ptr::null()) };
                        frown |= generated;
                    }
                }
            }
        }

        for habitat_ptr in habitat_ptrs {
            self.update_amphibious_neighbors(habitat_ptr);
            self.update_show_neighbors(habitat_ptr);
            save_to_memory::<u8>(habitat_ptr + 0x30, 1);
        }

        let world_ptr = globals().ztworldmgr_ptr() as u32;
        if smile {
            unsafe { ZTWORLDMGR_PLAY_SMILE_SOUND.original()(world_ptr as *const u32) };
        }
        if frown {
            unsafe { ZTWORLDMGR_PLAY_FROWN_SOUND.original()(world_ptr as *const u32) };
        }
    }

    /// Ports `ZTHabitatMgr::checkAmphibiousNeighbor` (`ZTHabitatMgr_checkAmphibiousNeighbor.c`/`.asm`) -
    /// read at the `.asm` level throughout since the C decompile's own struct-offset math for the
    /// "blocked by name" branch is garbled (a `setName` vtable call passed a type-tag address as if it
    /// were a name string): that whole block turned out, cross-referenced against the `.asm`, to be the
    /// exact same fence-family + `+0x192` passability check [`is_wall`] already implements, just called
    /// on `fence_a`/`fence_b` - see below.
    ///
    /// Resolves `habitat_b` from `tile_b_ptr`'s own position, bails if it's null or the "world" habitat
    /// ([`ZTHabitat::unknown_flag_0x2c`] set), and requires `habitat_a`/`habitat_b` to differ on
    /// [`ZTHabitat::is_tank`] (exactly one of the two must be a tank). Resolves the fence occupying each
    /// tile's own slot in the direction connecting them (via `ZTHabitat::tile_fence_in_direction` +
    /// [`BFMAP_GET_DIRECTION_0`], only when a genuine fence-family member -
    /// [`RVA_FENCE_TYPE_CHECK_ARG`]), then picks the "combined connector" wall: `fence_a` if it's a tank
    /// wall ([`RVA_TANK_WALL_TYPE_CHECK_ARG`]) with its own `+0x464` flag set, else `fence_b` under the
    /// same tank-wall check - bailing if neither qualifies, or if that wall's own `+0x464` flag is clear.
    /// Clears the wall's combined-connector flag, then bails if `fence_a`/`fence_b` are **both** real
    /// walls per [`is_wall`] (both sides physically block the connection). Bails if either habitat is
    /// itself a show tank ([`ZTHabitat::is_tank`] `&&` `zt_show_info_ptr != 0`- those go through
    /// [`Self::check_show_neighbor`] instead). Resolves whichever of `tile_a_ptr`/`tile_b_ptr` has its own
    /// `+0x85` bit `0x20` set to the "far" tile, requires a real *tank* habitat to occupy the *other*
    /// tile's position, and requires both corner elevations adjacent to the connecting direction
    /// ([`BFTILE_GET_CORNER_ELEVATION`]) to equal that tank's own `tank_height` (`+0x184`) plus its first
    /// owned tile's own `+0x3c` field ([`first_owned_tile_extra_height`]). On a full match, calls real
    /// vanilla `ZTHabitat::addAmphibiousNeighbor` both directions and sets the wall's combined-connector
    /// flag, returning `true`.
    ///
    /// `addAmphibiousNeighbor`/`clearAmphibiousNeighbors` themselves are deliberately left un-ported and
    /// called only via `.original()` - see `zthabitatmgr-implementation-plan.md`'s own top-level note on
    /// why (real STL red-black-tree insert/erase through an unidentified internal helper - cross-allocator
    /// risk for no behavioral gain, since every consumer here only needs to *call* add, never reimplement
    /// it).
    pub fn check_amphibious_neighbor(&self, habitat_a_ptr: u32, tile_a_ptr: u32, tile_b_ptr: u32) -> bool {
        if tile_b_ptr == 0 {
            return false;
        }
        let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
        let habitat_b_ptr = self.get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
        if habitat_b_ptr == 0 {
            return false;
        }
        let habitat_b = unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) };
        if habitat_b.unknown_flag_0x2c != 0 {
            return false;
        }
        let habitat_a = unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) };
        if habitat_a.is_tank() == habitat_b.is_tank() {
            return false;
        }

        let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
        let dir_ab = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
        let fence_a = if dir_ab != -1 {
            let f = ZTHabitat::tile_fence_in_direction(&tile_a, dir_ab as u32);
            if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
        } else {
            0
        };

        let dir_ba = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_b_ptr as i32, tile_a_ptr as i32) };
        let fence_b = if dir_ba != -1 {
            let f = ZTHabitat::tile_fence_in_direction(&tile_b, dir_ba as u32);
            if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
        } else {
            0
        };

        let mut connector = 0u32;
        if fence_a != 0 && unsafe { entity_type_matches(fence_a, RVA_TANK_WALL_TYPE_CHECK_ARG) } && get_from_memory::<u8>(fence_a + 0x464) != 0 {
            connector = fence_a;
        } else if fence_b != 0 && unsafe { entity_type_matches(fence_b, RVA_TANK_WALL_TYPE_CHECK_ARG) } {
            connector = fence_b;
        }
        if connector == 0 || get_from_memory::<u8>(connector + 0x464) == 0 {
            return false;
        }

        unsafe { SET_IS_COMBINED_CONNECTOR.original()(connector as *const u32, false) };

        if fence_a != 0 && fence_b != 0 && is_wall(fence_a) && is_wall(fence_b) {
            return false;
        }

        if (habitat_a.is_tank() && habitat_a.zt_show_info_ptr != 0) || (habitat_b.is_tank() && habitat_b.zt_show_info_ptr != 0) {
            return false;
        }

        let (far_tile_ptr, near_tile_ptr) = if get_from_memory::<u8>(tile_a_ptr + 0x85) & 0x20 != 0 {
            (tile_b_ptr, tile_a_ptr)
        } else {
            (tile_a_ptr, tile_b_ptr)
        };

        let near_tile = get_from_memory::<BFTile>(near_tile_ptr);
        let candidate_ptr = self.get_habitat_ptr(near_tile.pos.x, near_tile.pos.y);
        if candidate_ptr == 0 || !unsafe { ref_from_memory::<ZTHabitat>(candidate_ptr) }.is_tank() {
            return false;
        }

        let extra_height = first_owned_tile_extra_height(candidate_ptr);
        let tank_height: i32 = get_from_memory(candidate_ptr + 0x184);
        let target_elevation = tank_height + extra_height;

        let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(far_tile_ptr as i32, near_tile_ptr as i32) };
        let corner_a = if direction == -1 { -1 } else { (direction + 1) & 7 };
        let elevation_a = unsafe { BFTILE_GET_CORNER_ELEVATION.original()(far_tile_ptr as *const u32, corner_a) };
        if target_elevation != elevation_a {
            return false;
        }
        let corner_b = if direction == -1 { -1 } else { (direction - 1) & 7 };
        let elevation_b = unsafe { BFTILE_GET_CORNER_ELEVATION.original()(far_tile_ptr as *const u32, corner_b) };
        if target_elevation != elevation_b {
            return false;
        }

        unsafe {
            ADD_AMPHIBIOUS_NEIGHBOR.original()(habitat_b_ptr as *const u32, habitat_a_ptr as *const u32);
            ADD_AMPHIBIOUS_NEIGHBOR.original()(habitat_a_ptr as *const u32, habitat_b_ptr as *const u32);
            SET_IS_COMBINED_CONNECTOR.original()(connector as *const u32, true);
        }
        true
    }

    /// Snapshots `habitat_ptr`'s own [`ZTHabitat::boundary_tile_pairs_begin`]/`_end` vector into a plain
    /// `Vec<(u32,u32)>` - shared by [`Self::update_amphibious_neighbors`]/[`Self::update_show_neighbors`]/
    /// [`Self::do_show_check`], all three of which iterate real vanilla `checkAmphibiousNeighbor`/
    /// `checkShowNeighbor`/show-exhibit calls that can themselves mutate habitat state, matching real
    /// vanilla's own defensive snapshot-before-iterate shape (a fresh `std::vector` copy) rather than
    /// walking the live vector in place.
    pub(crate) fn snapshot_boundary_tile_pairs(begin: u32, end: u32) -> Vec<(u32, u32)> {
        let mut pairs = Vec::new();
        let mut entry = begin;
        while entry != end {
            pairs.push((get_from_memory(entry), get_from_memory(entry + 4)));
            entry += 8;
        }
        pairs
    }

    /// Ports `ZTHabitatMgr::updateAmphibiousNeighbors` (`_1`, `ZTHabitatMgr_updateAmphibiousNeighbors_1.c`):
    /// gated on `habitat_ptr` not being the "world" habitat ([`ZTHabitat::unknown_flag_0x2c`] clear),
    /// calls through to real vanilla `ZTHabitat::clearAmphibiousNeighbors` (left un-ported - see
    /// [`Self::check_amphibious_neighbor`]'s own doc comment), then re-derives every amphibious
    /// connection from scratch via [`Self::check_amphibious_neighbor`] over a
    /// [snapshot][Self::snapshot_boundary_tile_pairs] of the habitat's own boundary tile-pairs.
    pub fn update_amphibious_neighbors(&self, habitat_ptr: u32) {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        if habitat.unknown_flag_0x2c != 0 {
            return;
        }
        unsafe { CLEAR_AMPHIBIOUS_NEIGHBORS.original()(habitat_ptr as *const u32) };
        let pairs = Self::snapshot_boundary_tile_pairs(habitat.boundary_tile_pairs_begin, habitat.boundary_tile_pairs_end);
        for (tile_a_ptr, tile_b_ptr) in pairs {
            self.check_amphibious_neighbor(habitat_ptr, tile_a_ptr, tile_b_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::updateAmphibiousNeighbors` (`_0`, `ZTHabitatMgr_updateAmphibiousNeighbors_0.c`):
    /// a thin wrapper resolving the habitats occupying `tile_ptr` and its neighbour in `direction` (via
    /// [`ZTWorldMgr::get_neighbour`], the already-ported/tested `BFMap::getNeighbor(0)` equivalent), and -
    /// only when they differ and neither is the "world" habitat - calling through to
    /// [`Self::update_amphibious_neighbors`] for the first. Real vanilla's own body dereferences both
    /// resolved habitat pointers' `+0x2c` unconditionally with no null guard; this port adds one (skip
    /// rather than dereference null) since a real, unguarded null pointer read here would just crash Rust
    /// for no behavioral gain - real vanilla's own call sites never seem to hit this case in practice
    /// either, since map edges (where a neighbour tile can be absent) are exactly where this would occur.
    pub fn update_amphibious_neighbors_from_tile(&self, tile_ptr: u32, direction: u32) {
        let world = globals().ztworldmgr();
        let habitat_a_ptr = if tile_ptr != 0 {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            self.get_habitat_ptr(tile.pos.x, tile.pos.y)
        } else {
            0
        };
        let neighbour_ptr = if tile_ptr != 0 { get_neighbour_ptr(world, tile_ptr, Direction::from(direction)) } else { 0 };
        let habitat_b_ptr = if neighbour_ptr != 0 {
            let tile = get_from_memory::<BFTile>(neighbour_ptr);
            self.get_habitat_ptr(tile.pos.x, tile.pos.y)
        } else {
            0
        };
        if habitat_a_ptr == habitat_b_ptr || habitat_a_ptr == 0 || habitat_b_ptr == 0 {
            return;
        }
        let habitat_a = unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) };
        let habitat_b = unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) };
        if habitat_a.unknown_flag_0x2c == 0 && habitat_b.unknown_flag_0x2c == 0 {
            self.update_amphibious_neighbors(habitat_a_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::checkShowNeighbor` (`ZTHabitatMgr_checkShowNeighbor.c`/`.asm`). Resolves
    /// `habitat_b` from `tile_b_ptr`'s own position, bailing if null or the "world" habitat
    /// ([`ZTHabitat::unknown_flag_0x2c`] set). Requires exactly one of `habitat_a`/`habitat_b` to be a
    /// "showable" tank ([`ZTHabitat::is_tank`] `&&` `zt_show_info_ptr != 0`) - the other must not be.
    ///
    /// If `habitat_a`/`habitat_b` agree on [`ZTHabitat::is_tank`] (both tanks, or both non-tanks) and
    /// `habitat_a` is *not* a tank, connects directly. If both are tanks, connects only when their
    /// [`tank_height_plus_extra`] sums match, both `tank_height` (`+0x184`) exceed `3`, and both
    /// `is_filled` (`+0x198`). If `is_tank()` disagrees between the two, bails.
    ///
    /// On connect: calls real vanilla `ZTHabitat::addShowNeighbor` both directions, resolves the fence on
    /// each tile's own connecting-direction slot (own direction for `tile_a`, the opposite direction -
    /// `(dir - 4) & 7` - for `tile_b`, confirmed via `.asm` rather than a second `BFMap::getDirection`
    /// call) gated on [`RVA_FENCE_TYPE_CHECK_ARG`], calls each found fence's own vtable `+0x138` slot with
    /// arg `0`, then registers a show portal via `ZTHabitat::getShowPortal`/`addShowPortal` if one doesn't
    /// already exist. Always returns `true` once a connection was made, regardless of the portal calls'
    /// own results - matching real vanilla's own unconditional `return true` tail.
    ///
    /// `addShowNeighbor`/`clearShowNeighbors` themselves are deliberately left un-ported - same reasoning
    /// as [`Self::check_amphibious_neighbor`]'s own doc comment gives for `addAmphibiousNeighbor`.
    pub fn check_show_neighbor(&self, habitat_a_ptr: u32, tile_a_ptr: u32, tile_b_ptr: u32) -> bool {
        if tile_b_ptr == 0 {
            return false;
        }
        let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
        let habitat_b_ptr = self.get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
        if habitat_b_ptr == 0 {
            return false;
        }
        let habitat_b = unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) };
        if habitat_b.unknown_flag_0x2c != 0 {
            return false;
        }
        let habitat_a = unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) };

        let a_showable = habitat_a.is_tank() && habitat_a.zt_show_info_ptr != 0;
        let b_showable = habitat_b.is_tank() && habitat_b.zt_show_info_ptr != 0;
        if a_showable == b_showable {
            return false;
        }

        if habitat_a.is_tank() != habitat_b.is_tank() {
            return false;
        }
        if habitat_a.is_tank() {
            let sum_a = tank_height_plus_extra(habitat_a_ptr);
            let sum_b = tank_height_plus_extra(habitat_b_ptr);
            let tank_height_a: i32 = get_from_memory(habitat_a_ptr + 0x184);
            let tank_height_b: i32 = get_from_memory(habitat_b_ptr + 0x184);
            let filled_a = get_from_memory::<u8>(habitat_a_ptr + 0x198) != 0;
            let filled_b = get_from_memory::<u8>(habitat_b_ptr + 0x198) != 0;
            if !(sum_a == sum_b && tank_height_a > 3 && tank_height_b > 3 && filled_a && filled_b) {
                return false;
            }
        }

        unsafe {
            ADD_SHOW_NEIGHBOR.original()(habitat_a_ptr as *const u32, habitat_b_ptr as *const u32);
            ADD_SHOW_NEIGHBOR.original()(habitat_b_ptr as *const u32, habitat_a_ptr as *const u32);
        }

        let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
        let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
        let fence_a = if direction != -1 {
            let f = ZTHabitat::tile_fence_in_direction(&tile_a, direction as u32);
            if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
        } else {
            0
        };
        let opposite = if direction == -1 { -1 } else { (direction - 4) & 7 };
        let fence_b = if opposite != -1 {
            let f = ZTHabitat::tile_fence_in_direction(&tile_b, opposite as u32);
            if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
        } else {
            0
        };
        if fence_a != 0 {
            unsafe { call_vtable_slot_with_ptr(fence_a, 0x138, 0) };
        }
        if fence_b != 0 {
            unsafe { call_vtable_slot_with_ptr(fence_b, 0x138, 0) };
        }

        let existing_portal = unsafe { GET_SHOW_PORTAL.original()(habitat_a_ptr as *const u32, habitat_b_ptr as *const u32) };
        if existing_portal == 0 {
            let added = unsafe { ADD_SHOW_PORTAL.original()(habitat_a_ptr as *const u32, tile_a_ptr as *const u32, tile_b_ptr as *const u32) };
            if added != 0 {
                unsafe { ADD_SHOW_PORTAL.original()(habitat_b_ptr as *const u32, tile_b_ptr as *const u32, tile_a_ptr as *const u32) };
            }
        }
        true
    }

    /// Ports `ZTHabitatMgr::updateShowNeighbors` (`_1`, `ZTHabitatMgr_updateShowNeighbors_1.c`) - the
    /// recursive worker. Gated on `habitat_ptr` not being the "world" habitat
    /// ([`ZTHabitat::unknown_flag_0x2c`] clear).
    ///
    /// If `habitat_ptr` is itself "showable" ([`ZTHabitat::is_tank`] `&&` `zt_show_info_ptr != 0`): calls
    /// through to real vanilla `ZTHabitat::clearShowNeighbors` (left un-ported - see
    /// [`Self::check_show_neighbor`]'s own doc comment), then re-derives every show connection via
    /// [`Self::check_show_neighbor`] over a [snapshot][Self::snapshot_boundary_tile_pairs] of the
    /// boundary tile-pairs.
    ///
    /// Otherwise: for each boundary pair's own *second* tile (matching the real decompile's own
    /// `puVar7[1]` - only the second tile of each pair is examined, not the first), resolves the habitat
    /// occupying it; if that habitat is showable and hasn't already been visited during this call (a
    /// plain linear-scan `Vec<u32>`, matching the real decompile's own hand-rolled growable visited-set
    /// exactly - no need to model its own `PoolAlloc`-backed storage, this is purely local scratch state),
    /// records it and recurses into `update_show_neighbors` for it.
    pub fn update_show_neighbors(&self, habitat_ptr: u32) {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        if habitat.unknown_flag_0x2c != 0 {
            return;
        }
        let showable = habitat.is_tank() && habitat.zt_show_info_ptr != 0;
        let pairs = Self::snapshot_boundary_tile_pairs(habitat.boundary_tile_pairs_begin, habitat.boundary_tile_pairs_end);

        if showable {
            unsafe { CLEAR_SHOW_NEIGHBORS.original()(habitat_ptr as *const u32) };
            for (tile_a_ptr, tile_b_ptr) in pairs {
                self.check_show_neighbor(habitat_ptr, tile_a_ptr, tile_b_ptr);
            }
            return;
        }

        let mut visited: Vec<u32> = Vec::new();
        for (_, tile_b_ptr) in pairs {
            if tile_b_ptr == 0 {
                continue;
            }
            let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
            let neighbor_ptr = self.get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
            if neighbor_ptr == 0 {
                continue;
            }
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
            let neighbor_showable = neighbor.is_tank() && neighbor.zt_show_info_ptr != 0;
            if neighbor_showable && !visited.contains(&neighbor_ptr) {
                visited.push(neighbor_ptr);
                self.update_show_neighbors(neighbor_ptr);
            }
        }
    }

    /// Ports `ZTHabitatMgr::updateShowNeighbors` (`_0`, `ZTHabitatMgr_updateShowNeighbors_0.c`) - same
    /// thin-wrapper shape as [`Self::update_amphibious_neighbors_from_tile`], calling through to
    /// [`Self::update_show_neighbors`] for the tile's own habitat.
    pub fn update_show_neighbors_from_tile(&self, tile_ptr: u32, direction: u32) {
        let world = globals().ztworldmgr();
        let habitat_a_ptr = if tile_ptr != 0 {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            self.get_habitat_ptr(tile.pos.x, tile.pos.y)
        } else {
            0
        };
        let neighbour_ptr = if tile_ptr != 0 { get_neighbour_ptr(world, tile_ptr, Direction::from(direction)) } else { 0 };
        let habitat_b_ptr = if neighbour_ptr != 0 {
            let tile = get_from_memory::<BFTile>(neighbour_ptr);
            self.get_habitat_ptr(tile.pos.x, tile.pos.y)
        } else {
            0
        };
        if habitat_a_ptr == habitat_b_ptr || habitat_a_ptr == 0 || habitat_b_ptr == 0 {
            return;
        }
        let habitat_a = unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) };
        let habitat_b = unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) };
        if habitat_a.unknown_flag_0x2c == 0 && habitat_b.unknown_flag_0x2c == 0 {
            self.update_show_neighbors(habitat_a_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::doShowCheck` (`ZTHabitatMgr_doShowCheck.c`/`.asm`, read at the `.asm` level
    /// throughout for the same reason as [`Self::check_amphibious_neighbor`]). Over a
    /// [snapshot][Self::snapshot_boundary_tile_pairs] of `habitat_ptr`'s own boundary tile-pairs (an empty
    /// snapshot immediately fails the whole check, matching real vanilla exactly), for each pair resolves
    /// the fence on each tile's own connecting-direction slot (own/reverse direction, both gated on
    /// [`RVA_FENCE_TYPE_CHECK_ARG`] like [`Self::check_show_neighbor`]) and:
    /// - if `fence_a` is absent but `fence_b` is present: [`fence_entity_flag_0x6f`] on `fence_b` sets
    ///   "found a show neighbor" if true, else requires a real *tank* habitat at `fence_b`'s own tile
    ///   (`BFEntity::getTile` + [`Self::get_habitat_ptr`]) or the whole check fails immediately.
    /// - if `fence_a` is present: [`fence_entity_flag_0x6f`] on `fence_a` sets "found a show neighbor" if
    ///   true; additionally, when `fence_b` is absent or `ZTUI::general::getMapview()`'s own `+0x378` flag
    ///   is set, the same flag must be true or the whole check fails immediately.
    ///
    /// A pair with neither fence present, or one that doesn't trip either failure branch, simply moves on
    /// to the next pair. If the check never fails but also never finds a show neighbor, it's forced to
    /// fail (matching real vanilla's own `if (!bVar2) goto <fail>` tail).
    ///
    /// On success: calls the already-ported [`ZTHabitat::set_is_show_exhibit`], and - when `remove_illegal`
    /// and [`ZTHabitat::is_tank`] - calls through to real vanilla `ZTTankExhibit::removeIllegalEntities`.
    /// On failure: calls the already-ported [`ZTHabitat::set_is_not_show_exhibit`]. Either way, finally
    /// calls [`Self::update_show_neighbors`] (this port's own Rust version, not `.original()`) and real
    /// vanilla `ZTWorldMgr::updateShowAssociations`, then returns the success/failure result.
    pub fn do_show_check(&self, habitat_ptr: u32, remove_illegal: bool) -> bool {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let pairs = Self::snapshot_boundary_tile_pairs(habitat.boundary_tile_pairs_begin, habitat.boundary_tile_pairs_end);

        let mut still_valid = !pairs.is_empty();
        let mut found_show_neighbor = false;

        if still_valid {
            for (tile_a_ptr, tile_b_ptr) in &pairs {
                if !still_valid {
                    break;
                }
                let (tile_a_ptr, tile_b_ptr) = (*tile_a_ptr, *tile_b_ptr);
                let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
                let tile_b = get_from_memory::<BFTile>(tile_b_ptr);

                let dir_ab = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
                let fence_a = if dir_ab != -1 {
                    let f = ZTHabitat::tile_fence_in_direction(&tile_a, dir_ab as u32);
                    if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
                } else {
                    0
                };
                let dir_ba = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_b_ptr as i32, tile_a_ptr as i32) };
                let fence_b = if dir_ba != -1 {
                    let f = ZTHabitat::tile_fence_in_direction(&tile_b, dir_ba as u32);
                    if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
                } else {
                    0
                };

                if fence_a == 0 {
                    if fence_b != 0 {
                        let flag = fence_entity_flag_0x6f(fence_b);
                        if flag {
                            found_show_neighbor = true;
                        } else {
                            let entity_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(fence_b as *const u32) } as u32;
                            let candidate_ptr = if entity_tile_ptr != 0 {
                                let t = get_from_memory::<BFTile>(entity_tile_ptr);
                                self.get_habitat_ptr(t.pos.x, t.pos.y)
                            } else {
                                0
                            };
                            let valid = candidate_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(candidate_ptr) }.is_tank();
                            if !valid {
                                still_valid = false;
                            }
                        }
                    }
                } else {
                    let flag = fence_entity_flag_0x6f(fence_a);
                    if flag {
                        found_show_neighbor = true;
                    }
                    let mapview_ptr = unsafe { ZTUI_GENERAL_GET_MAPVIEW.original()() } as u32;
                    let mapview_flag = mapview_ptr != 0 && get_from_memory::<u8>(mapview_ptr + 0x378) != 0;
                    if (fence_b == 0 || mapview_flag) && !flag {
                        still_valid = false;
                    }
                }
            }
            if !found_show_neighbor {
                still_valid = false;
            }
        }

        if still_valid {
            unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) }.set_is_show_exhibit();
            if remove_illegal && habitat.is_tank() {
                unsafe { ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES.original()(habitat_ptr as *const u32, 2, false) };
            }
        } else {
            unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) }.set_is_not_show_exhibit();
        }

        self.update_show_neighbors(habitat_ptr);
        unsafe { ZTWORLDMGR_UPDATE_SHOW_ASSOCIATIONS.original()() };

        still_valid
    }

    /// Ports `ZTHabitatMgr::canSeeShowFromBuilding` (`ZTHabitatMgr_canSeeShowFromBuilding.c`): the show
    /// id ([`ZTHabitat::get_show_info_id`]) of the first `exhibit_array` entry with a real `ZTShowInfo`
    /// that [`Self::can_see_habitat_from_building`] (kept un-ported - see that method's own doc comment)
    /// reports visible from `building_ptr`, or `0` if none. Real vanilla's own return value packs
    /// undefined upper 16 bits around the real `u16` show id (same `CONCAT22` shape [`ZTHabitat::get_show_info_id`]'s
    /// own doc comment documents) - masked here via a plain zero-extending cast, matching that method.
    pub fn can_see_show_from_building(&self, building_ptr: u32) -> u32 {
        for i in 0..self.exhibit_array.len() {
            let habitat_ptr = self.exhibit_array.get_ptr(i);
            let show_id = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.get_show_info_id();
            if show_id != 0 && low_byte_bool(unsafe { Self::can_see_habitat_from_building(habitat_ptr, building_ptr) }) {
                return show_id as u32;
            }
        }
        0
    }

    /// Ports `ZTHabitatMgr::habitatSeenFromBuilding` (`ZTHabitatMgr_habitatSeenFromBuilding.c`): the
    /// pointer of the first `exhibit_array` entry that isn't the "world" habitat
    /// ([`ZTHabitat::unknown_flag_0x2c`] clear), is a "showable" tank ([`ZTHabitat::is_tank`] `&&`
    /// `zt_show_info_ptr != 0`), and that [`Self::can_see_habitat_from_building`] reports visible from
    /// `building_ptr` - or `0` (null) if none.
    pub fn habitat_seen_from_building(&self, building_ptr: u32) -> u32 {
        for i in 0..self.exhibit_array.len() {
            let habitat_ptr = self.exhibit_array.get_ptr(i);
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
            if habitat.unknown_flag_0x2c != 0 {
                continue;
            }
            if habitat.is_tank() && habitat.zt_show_info_ptr != 0 && low_byte_bool(unsafe { Self::can_see_habitat_from_building(habitat_ptr, building_ptr) }) {
                return habitat_ptr;
            }
        }
        0
    }

    /// Ports `ZTHabitatMgr::canSeeHabitatFromBuilding` (`ZTHabitatMgr_canSeeHabitatFromBuilding.c`/`.asm`,
    /// `generated.rs`'s `CAN_SEE_HABITAT_FROM_BUILDING`, `cdecl`) - the previous scoping pass's "genuine
    /// identification blocker" (`FUN_004f2d44`) resolves to `ZTVisibilityTesting::testLOS`
    /// (`ztvisibilitytesting::TEST_LOS`), confirming that pass's own guess ("likely a line-of-sight
    /// raycast/rect-intersection helper") - see `zthabitatmgr-implementation-plan.md`'s "Regeneration
    /// corrections".
    ///
    /// Resolves the tile 4 steps in front of `building_ptr`, in its own facing direction (`+0x12c` -
    /// `0`/`2`/`4`/`6` for N/E/S/W, the same shared per-placed-entity "facing/rotation" field
    /// [`ZTHabitat::tile_fence_in_direction`]'s own callers already read off a fence at this file's other
    /// `+0x12c` call sites), and requires it to belong to `habitat_ptr`. Then line-of-sight-tests between
    /// the tile 3 steps out and the tile 5 steps out (each tile's `x`/`y` doubled, elevation doubled plus
    /// `2` - matching real vanilla's own coordinate-to-visibility-space scaling byte-for-byte) via
    /// `ZTVisibilityTesting::testLOS`.
    ///
    /// Real vanilla's own `.asm` reads each resolved tile's `pos`/elevation fields unconditionally even
    /// when the computed position falls outside the live map (`iVar7`/`iVar5` left `0`, a null-page read) -
    /// guarded here as `(0, 0, 0)` instead of crashing, matching this file's established "dead in
    /// practice" convention for a case that shouldn't occur for a building actually placed on the map.
    pub(crate) unsafe fn can_see_habitat_from_building(habitat_ptr: u32, building_ptr: u32) -> u32 {
        let habitat_mgr = globals().zthabitatmgr();
        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(building_ptr as *const u32) } as u32;
        if tile_ptr == 0 {
            return 0;
        }
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let facing: u32 = get_from_memory(building_ptr + 0x12c);
        let (dx, dy) = match facing {
            0 => (0i32, -1i32),
            2 => (1, 0),
            4 => (0, 1),
            6 => (-1, 0),
            _ => (0, 0),
        };

        if habitat_mgr.get_habitat_ptr(tile.pos.x + dx * 4, tile.pos.y + dy * 4) != habitat_ptr {
            return 0;
        }

        let world = globals().ztworldmgr();
        let resolve = |steps: i32| -> (i32, i32, i32) {
            let x = tile.pos.x + dx * steps;
            let y = tile.pos.y + dy * steps;
            if x < 0 || y < 0 || x as u32 >= world.map_x_size || y as u32 >= world.map_y_size {
                (0, 0, 0)
            } else {
                let t = get_from_memory::<BFTile>(world.get_tile_ptr(x as u32, y as u32));
                (t.pos.x, t.pos.y, t.pos.z)
            }
        };

        let (near_x, near_y, near_z) = resolve(3);
        let (far_x, far_y, far_z) = resolve(5);

        unsafe { TEST_LOS.original()(near_x << 1, near_y << 1, near_z * 2 + 2, far_x << 1, far_y << 1, far_z * 2 + 2) }
    }

    /// Real vanilla's own combined "reached by [`Self::can_find_path`]'s search-A"/"search-B" bit pair
    /// (`0x1`/`0x2`) living at `+0x24` on the same grid-cell row [`Self::get_habitat_cell_addr`] returns -
    /// the byte [`Self::clear_pathfinding`] resets to `0` between calls. `tile_ptr` must be a valid,
    /// in-map `BFTile*`; an out-of-range position (shouldn't occur for a tile [`Self::can_find_path`]
    /// itself discovered via a real map neighbour lookup) reads as `0`, matching this file's own
    /// established defensive-bounds-check convention (see [`Self::get_habitat_ptr`]) rather than real
    /// vanilla's own unguarded pointer arithmetic here.
    pub(crate) fn pathfinding_flags(&self, tile_ptr: u32) -> u8 {
        let tile = get_from_memory::<BFTile>(tile_ptr);
        match self.get_habitat_cell_addr(tile.pos.x, tile.pos.y) {
            Some(addr) => get_from_memory(addr + 0x24),
            None => 0,
        }
    }

    pub(crate) fn set_pathfinding_flag(&self, tile_ptr: u32, bit: u8) {
        let tile = get_from_memory::<BFTile>(tile_ptr);
        if let Some(addr) = self.get_habitat_cell_addr(tile.pos.x, tile.pos.y) {
            let existing: u8 = get_from_memory(addr + 0x24);
            save_to_memory(addr + 0x24, existing | bit);
        }
    }

    /// Ports `ZTHabitatMgr::canFindPath` (`ZTHabitatMgr_canFindPath.c`/`.asm`, `generated.rs`'s
    /// `CAN_FIND_PATH`).
    ///
    /// A real bidirectional BFS over the live grid's own [`Self::pathfinding_flags`] byte, checking
    /// whether `tile_a_ptr`/`tile_b_ptr` are connected without crossing a wall. Real vanilla backs each
    /// of its two search frontiers with a `PoolAlloc`-node `std::list<BFTile*>`; reimplemented here with
    /// a plain `VecDeque<u32>` instead - both lists are pure per-call scratch state (nothing else in the
    /// decompile corpus reads or writes them), so there is no cross-allocator risk (per `CLAUDE.md`'s own
    /// `PoolAlloc` caveat) in leaving vanilla's own allocator out of the traversal entirely; the *grid
    /// cells* the search marks are still real, shared vanilla memory, read/written in place exactly as
    /// before. Passability between a tile and a cardinal neighbour reuses [`fence_pair`]/[`is_wall`] - the
    /// same two-sided fence check [`ZTHabitat::add_habitat_tiles`]'s own flood-fill already implements
    /// (checking both the source tile's own fence field and the neighbour's own fence field in the
    /// opposite direction), confirmed identical against the `.asm`'s own dual fence-offset reads.
    ///
    /// Each loop iteration pops one tile off both frontiers, returns `true` immediately if either tile's
    /// cell already carries the *other* search's own visited bit (the two searches met), otherwise marks
    /// its own bit and pushes its unvisited, passable cardinal neighbours - matching
    /// `ZTHabitatMgr_canFindPath.c`'s own pop-check-mark-expand order exactly. Returns `false` once either
    /// frontier empties without the searches meeting. Real vanilla has no null guard on its own two `BFTile*`
    /// parameters; guarded here since at least one real call site (`ZTHabitatMgr::fencePlaced`, still
    /// un-ported) can pass a null "zoo entrance tile" pointer when none is set yet.
    pub fn can_find_path(&self, tile_a_ptr: u32, tile_b_ptr: u32) -> bool {
        if tile_a_ptr == 0 || tile_b_ptr == 0 {
            return false;
        }
        let world = globals().ztworldmgr();
        let mut queue_a = VecDeque::from([tile_a_ptr]);
        let mut queue_b = VecDeque::from([tile_b_ptr]);

        while let (Some(cur_a), Some(cur_b)) = (queue_a.pop_front(), queue_b.pop_front()) {
            if self.pathfinding_flags(cur_a) & 0x2 != 0 || self.pathfinding_flags(cur_b) & 0x1 != 0 {
                return true;
            }

            if self.pathfinding_flags(cur_a) & 0x1 == 0 {
                self.set_pathfinding_flag(cur_a, 0x1);
                queue_a.extend(pathfinding_frontier(world, cur_a));
            }
            if self.pathfinding_flags(cur_b) & 0x2 == 0 {
                self.set_pathfinding_flag(cur_b, 0x2);
                queue_b.extend(pathfinding_frontier(world, cur_b));
            }
        }
        false
    }

    /// Ports `ZTHabitatMgr::clearPathfinding` (`ZTHabitatMgr_clearPathfinding.c`): clears both
    /// [`Self::can_find_path`] visited bits (`0x1`/`0x2`) on every grid cell in the live map - the same
    /// `+0x24` byte [`Self::get_habitat_cell_addr`]'s own doc comment previously described as unread by
    /// anything this pass ported (no longer true now that this and `canFindPath` both use it). Real
    /// vanilla's own loop nests oddly - the outer loop is bounded by `map_y_size` but steps through
    /// `other_array`'s per-*column* (`0xc`-stride, i.e. X-indexed) entries, and the inner loop is bounded
    /// by `map_x_size` but steps through each column's own *row* (`0x28`-stride, Y-indexed) entries -
    /// which still clears every real cell only because Zoo Tycoon's own maps are always square
    /// (`map_x_size == map_y_size`). Ported here as a straightforward `x in 0..map_x_size, y in
    /// 0..map_y_size` double loop via [`Self::get_habitat_cell_addr`] instead of reproducing that
    /// axis-swapped quirk, reaching the same real-world result without depending on it.
    pub fn clear_pathfinding(&self) {
        let world = globals().ztworldmgr();
        for x in 0..world.map_x_size as i32 {
            for y in 0..world.map_y_size as i32 {
                if let Some(addr) = self.get_habitat_cell_addr(x, y) {
                    let existing: u8 = get_from_memory(addr + 0x24);
                    save_to_memory(addr + 0x24, existing & 0xfc);
                }
            }
        }
    }

    /// Ports `ZTHabitatMgr::clearStaffHabitat` (`ZTHabitatMgr_clearStaffHabitat.c`/`.asm`) as an
    /// orchestrator: despite its `ZTHabitatMgr::` decompile namespace this is a plain free `stdcall`
    /// helper - the `.asm`'s own `RET 0x4` pops only the one stack argument and nothing in the body reads
    /// `ECX`/`this` at all, the same misnaming pattern already documented for
    /// [`Self::replace_gate_with_fence`]/[`ZTHabitat::highlight`]. Walks the live `GLOBAL_ZTWorldMgr`'s
    /// own [`ZTWorldMgr::entity_array`], and for every non-null entry whose own
    /// [`RVA_HABITAT_TYPE_CHECK_ARG`] `isCastClass` check passes, calls through to the real, unidentified
    /// per-entry worker [`FUN_0050C884`] with `staff_ptr`.
    ///
    /// Deliberately not exercised by an active live test with a synthesized `staff_ptr` - same reasoning
    /// as `ZTHabitat::move_gate_to`: [`FUN_0050C884`]'s own body is unidentified and may dereference its
    /// argument, and this harness has no existing helper for finding a real `ZTStaff*` to pass instead.
    /// The detour is still installed (byte-for-byte reproducing real vanilla's own call graph adds no new
    /// risk over baseline) and covered by the `DETOURS_ENABLED` wiring check.
    pub fn clear_staff_habitat(staff_ptr: u32) {
        let world = globals().ztworldmgr();
        for entity_ptr in world.entity_array() {
            if entity_ptr != 0 && unsafe { entity_type_matches(entity_ptr, RVA_HABITAT_TYPE_CHECK_ARG) } {
                unsafe { FUN_0050C884.original()(entity_ptr as *const u32, staff_ptr as *const u32) };
            }
        }
    }

    /// Ports `ZTHabitatMgr::getTank` (`ZTHabitatMgr_getTank.c`/`.asm`): the habitat occupying `tile_ptr`
    /// if it's a tank ([`ZTHabitat::is_tank`]), else `0` (null) - including when `tile_ptr` itself is null
    /// or no habitat occupies the tile.
    pub fn get_tank(&self, tile_ptr: u32) -> u32 {
        if tile_ptr == 0 {
            return 0;
        }
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let habitat_ptr = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
        if habitat_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.is_tank() {
            habitat_ptr
        } else {
            0
        }
    }

    /// Ports `ZTHabitatMgr::getOutermostTank` (`ZTHabitatMgr_getOutermostTank.c`/`.asm`): clears every
    /// real habitat's own [`ZTHabitat::tank_walk_visited_marker`] (a fresh walk each call), then returns
    /// [`ZTHabitat::get_outermost_tank`] for `habitat_ptr`.
    ///
    /// Was deliberately left un-detoured: `generated.rs`'s own `zthabitatmgr::GET_OUTERMOST_TANK` entry
    /// declared no return value (`fn(*const u32, *const u32)`) despite `.asm` proving real vanilla leaves
    /// the inner `ZTHabitat::getOutermostTank` call's own `EAX` result in place through a bare `RET 0x4` -
    /// real callers do receive a pointer back. Per `CLAUDE.md`'s own note on `generated.rs` errors this was
    /// surfaced rather than hand-edited; the entry has since been corrected upstream
    /// (`-> *const u32` added) and the detour is now installed.
    pub fn get_outermost_tank(&self, habitat_ptr: u32) -> u32 {
        self.clear_tank_walk_markers();
        unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.get_outermost_tank()
    }

    /// Ports `ZTHabitatMgr::getNeedyNestedTank` (`ZTHabitatMgr_getNeedyNestedTank.c`/`.asm`) - a thin
    /// wrapper: clears every real habitat's own [`ZTHabitat::tank_walk_visited_marker`] (same reset
    /// [`Self::get_outermost_tank`] performs), then delegates to [`ZTHabitat::get_needy_nested_tank`].
    /// See that method's own doc comment for the full algorithm and the `generated.rs` signature bug this
    /// was blocked on.
    pub fn get_needy_nested_tank(&self, habitat_ptr: u32, keeper_ptr: u32) -> u32 {
        self.clear_tank_walk_markers();
        unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.get_needy_nested_tank(keeper_ptr)
    }

    /// Clears every real habitat's own [`ZTHabitat::tank_walk_visited_marker`] - the same reset
    /// [`Self::get_outermost_tank`] performs before every walk. Exposed `pub(crate)` so live tests
    /// comparing [`ZTHabitat::get_outermost_tank`] directly (real vs. reimplemented) can reset the
    /// shared scratch flag between habitats themselves, the same way production code does.
    pub(crate) fn clear_tank_walk_markers(&self) {
        for i in 0..self.exhibit_array.len() {
            let ptr = self.exhibit_array.get_ptr(i);
            if ptr != 0 {
                unsafe { mut_from_memory::<ZTHabitat>(ptr) }.tank_walk_visited_marker = 0;
            }
        }
    }

    /// Ports `ZTHabitatMgr::leadsTo` (`ZTHabitatMgr_leadsTo.c`/`.asm`): `true` if walking the gate-tile-out
    /// chain from `habitat_a_ptr` (via [`ZTHabitat::get_gate_tile_out`] + [`Self::get_habitat_ptr`] on the
    /// resulting tile's own position, the same lookup [`Self::get_outermost_tank`] uses) reaches
    /// `habitat_b_ptr` or loops back to `habitat_a_ptr`, before the walk hits a tile with no further gate,
    /// an already-visited habitat, or a "world" habitat ([`ZTHabitat::unknown_flag_0x2c`] set). Requires
    /// both `habitat_a_ptr`/`habitat_b_ptr` to be non-null and non-"world" up front, else returns `false`
    /// immediately without touching any walk state.
    ///
    /// Shares [`ZTHabitat::tank_walk_visited_marker`] (`+0x168`) with [`Self::get_outermost_tank`]/
    /// [`Self::get_needy_nested_tank`]'s own tank-chain walks - resets it across every `exhibit_array`
    /// entry via [`Self::clear_tank_walk_markers`] before walking, the same shared-scratch-flag convention
    /// those two already establish, rather than duplicating the reset loop real vanilla's own decompile
    /// inlines. The decompile's own `this_00 == param_1` "looped back to the start" check is reproduced
    /// faithfully even though it's unreachable in practice - `habitat_a_ptr` is already marked visited
    /// before the walk begins, so the earlier already-visited check always catches a direct loop-back
    /// first; kept for exact parity with real vanilla's own control flow rather than "corrected" away.
    pub fn leads_to(&self, habitat_a_ptr: u32, habitat_b_ptr: u32) -> bool {
        if habitat_a_ptr == 0 {
            return false;
        }
        if unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) }.unknown_flag_0x2c != 0 || habitat_b_ptr == 0 {
            return false;
        }
        if unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) }.unknown_flag_0x2c != 0 {
            return false;
        }

        self.clear_tank_walk_markers();
        unsafe { mut_from_memory::<ZTHabitat>(habitat_a_ptr) }.tank_walk_visited_marker = 1;

        let mut current_ptr = habitat_a_ptr;
        loop {
            let gate_tile = unsafe { ref_from_memory::<ZTHabitat>(current_ptr) }.get_gate_tile_out();
            let next_ptr = match gate_tile {
                Some(tile) => self.get_habitat_ptr(tile.pos.x, tile.pos.y),
                None => 0,
            };
            if next_ptr == 0 {
                return false;
            }
            if unsafe { ref_from_memory::<ZTHabitat>(next_ptr) }.tank_walk_visited_marker != 0 {
                return false;
            }
            unsafe { mut_from_memory::<ZTHabitat>(next_ptr) }.tank_walk_visited_marker = 1;
            if next_ptr == habitat_b_ptr || next_ptr == habitat_a_ptr {
                return true;
            }
            if unsafe { ref_from_memory::<ZTHabitat>(next_ptr) }.unknown_flag_0x2c != 0 {
                return false;
            }
            current_ptr = next_ptr;
        }
    }

    /// Ports `ZTHabitatMgr::breakAmphibiousConnection` (`ZTHabitatMgr_breakAmphibiousConnection.c`/`.asm`).
    /// Despite its `ZTHabitatMgr::` decompile namespace this is a plain `stdcall` free function taking
    /// two tile pointers (matches `generated.rs`'s own `stdcall fn(*const u32, i32)` signature), the same
    /// misnaming pattern already documented for [`Self::clear_staff_habitat`]/
    /// [`Self::replace_gate_with_fence`].
    ///
    /// The inverse of [`Self::check_amphibious_neighbor`]'s own connector-selection logic: resolves the
    /// fence occupying each tile's own slot in the direction connecting them (only when a genuine tank
    /// wall - [`RVA_TANK_WALL_TYPE_CHECK_ARG`], matching real vanilla's own `entity_type`-based
    /// `isCastClass` check byte-for-byte), and clears each one's own combined-connector flag via
    /// `ZTTankWall::setIsCombinedConnector`.
    pub fn break_amphibious_connection(tile_a_ptr: u32, tile_b_ptr: u32) {
        let tank_wall_in_direction = |from_ptr: u32, to_ptr: u32| -> u32 {
            let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(from_ptr as i32, to_ptr as i32) };
            if dir == -1 {
                return 0;
            }
            let from_tile = get_from_memory::<BFTile>(from_ptr);
            let fence = ZTHabitat::tile_fence_in_direction(&from_tile, dir as u32);
            if fence != 0 && unsafe { entity_type_matches(fence, RVA_TANK_WALL_TYPE_CHECK_ARG) } {
                fence
            } else {
                0
            }
        };

        let wall_a = tank_wall_in_direction(tile_a_ptr, tile_b_ptr);
        let wall_b = tank_wall_in_direction(tile_b_ptr, tile_a_ptr);
        if wall_a != 0 {
            unsafe { SET_IS_COMBINED_CONNECTOR.original()(wall_a as *const u32, false) };
        }
        if wall_b != 0 {
            unsafe { SET_IS_COMBINED_CONNECTOR.original()(wall_b as *const u32, false) };
        }
    }

    /// Ports `ZTHabitatMgr::checkEnterHabitat` (`ZTHabitatMgr_checkEnterHabitat.c`/`.asm`) - despite its
    /// `ZTHabitatMgr::` decompile namespace this is a plain `stdcall` free function (matches
    /// `generated.rs`'s own `stdcall fn(*const u32, *const u32) -> u32`, and the `.asm` never reads
    /// `ECX`), the same misnaming pattern already documented for [`Self::clear_staff_habitat`]/
    /// [`Self::break_amphibious_connection`].
    ///
    /// This file's own plan doc previously deferred this as needing a "thunk-adjusted secondary vtable
    /// dispatch this codebase has no model for" - reading the real `.asm` instead of the C decompile's own
    /// confusing `(param_2->cls_0x62d4b4).vftptr_0x0[1]` multiple-inheritance sugar shows that claim was
    /// wrong: both virtual calls this function makes (`isTank` at `ZTHabitat`'s own vtable `+0x20`, and an
    /// unnamed `BFUnit` vtable slot at `+0x164` - `BFUnit.md` confirms the slot exists, marked `*unknown*`,
    /// zero other call-site evidence in the corpus) are plain, single-inheritance dispatches through each
    /// object's own primary vtable at offset `0`. The `isCastClass`-shaped fence-family checks
    /// (`(**(fence->entity_type->vtable+0x1c))(&CAST_ZTFence)`) are likewise not a new blocker - they're
    /// the exact mechanism [`entity_type_matches`] already ports faithfully, used throughout this file
    /// (e.g. [`Self::check_show_neighbor`]/[`is_wall`]).
    ///
    /// Resolves the gate-tile-in/gate-tile-out pair and the direction between them; if each side's own
    /// fence slot holds a real fence, returns `1` (blocked - a closed gate on both ends). Otherwise
    /// compares `unit_ptr`'s own cost to traverse the gate in each direction - via
    /// [`BFUNIT_GET_PATH_COST`]'s real body for a non-tank habitat, or [`call_bfunit_tile_cost_vtable_slot`]
    /// for a tank (a `ZTTankExhibit`'s entrance is presumably water, needing a different cost metric
    /// `getPathCost` doesn't cover) - against the shared "impassable" cost threshold
    /// ([`MAX_PATH_COST_RVA`]). Returns `4` if even the cheaper direction is unreachable, `3` if either
    /// gate tile is already occupied ([`BFTile::entity_ptr`]), else `0` (clear to enter).
    ///
    /// Real vanilla dereferences both gate tiles unconditionally with no null guard; guarded here instead
    /// (returns `4`, "can't enter") for the same "dead in practice" reasoning this file already documents
    /// elsewhere - a habitat reachable through this call always owns a real entrance gate.
    pub fn check_enter_habitat(habitat_ptr: u32, unit_ptr: u32) -> u32 {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let (Some(gti_tile), Some(gto_tile)) = (habitat.get_gate_tile_in(), habitat.get_gate_tile_out()) else {
            return 4;
        };
        let ztwm = globals().ztworldmgr();
        let gti_ptr = ztwm.get_ptr_from_bftile(&gti_tile);
        let gto_ptr = ztwm.get_ptr_from_bftile(&gto_tile);

        let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(gto_ptr as i32, gti_ptr as i32) };
        let (fence_a, fence_b) = if dir == -1 {
            (0, 0)
        } else {
            let opposite = ((dir - 4) & 7) as u32;
            let fa = ZTHabitat::tile_fence_in_direction(&gto_tile, dir as u32);
            let fa = if fa != 0 && unsafe { entity_type_matches(fa, RVA_FENCE_TYPE_CHECK_ARG) } { fa } else { 0 };
            let fb = ZTHabitat::tile_fence_in_direction(&gti_tile, opposite);
            let fb = if fb != 0 && unsafe { entity_type_matches(fb, RVA_FENCE_TYPE_CHECK_ARG) } { fb } else { 0 };
            (fa, fb)
        };
        if fence_a != 0 && fence_b != 0 {
            return 1;
        }

        let max_cost: i32 = if habitat.is_tank() {
            let cost_out = unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, gto_ptr) };
            let cost_in = unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, gti_ptr) };
            cost_out.max(cost_in)
        } else {
            let cost_a = unsafe { BFUNIT_GET_PATH_COST.original()(unit_ptr as *const u32, gti_ptr as i32, gto_ptr as i32) } as i32;
            let cost_b = unsafe { BFUNIT_GET_PATH_COST.original()(unit_ptr as *const u32, gto_ptr as i32, gti_ptr as i32) } as i32;
            cost_a.max(cost_b)
        };

        let threshold = get_from_memory::<i32>(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA) - 1;
        if max_cost >= threshold {
            return 4;
        }
        if gti_tile.entity_ptr != 0 || gto_tile.entity_ptr != 0 {
            3
        } else {
            0
        }
    }

    /// Ports `ZTHabitatMgr::fenceReplaced` (`ZTHabitatMgr_fenceReplaced.c`/`.asm`) - not a duplicate of
    /// `replaceFenceWithGate`/`replaceGateWithFence` as earlier scoping guessed (see
    /// `zthabitatmgr-implementation-plan.md`'s "Regeneration corrections"): checks whether `tile_ptr` and
    /// its own neighbour in `direction` sit in different habitats, calls [`Self::do_show_check`] on
    /// whichever side isn't the "world" habitat ([`ZTHabitat::unknown_flag_0x2c`] clear), then calls
    /// through to the still-un-ported real vanilla `checkExhibitMorph` unconditionally (see that method's
    /// own deferral in the "Fence/gate placement" table - `morphExhibit`'s own heavy UI/undo-action
    /// orchestration and permanent habitat destroy/recreate make it unsafe to reimplement or exercise
    /// synthetically, but calling through here reproduces exactly what real, un-ported vanilla already
    /// does on every fence replacement today - no new risk over baseline).
    ///
    /// Real vanilla's own `.asm` reads each habitat's `+0x2c` byte unconditionally once the two occupants
    /// differ, with no null guard - guarded here instead of crashing on a tile with no habitat occupant at
    /// all (shouldn't occur for any tile that has ever held a fence, matching this file's other "dead in
    /// practice" unguarded-vanilla-read deferrals).
    pub fn fence_replaced(&self, tile_ptr: u32, direction: u32) {
        let habitat_a_ptr = if tile_ptr == 0 {
            0
        } else {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            self.get_habitat_ptr(tile.pos.x, tile.pos.y)
        };

        let habitat_b_ptr = if tile_ptr == 0 {
            0
        } else {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            match globals().ztworldmgr().get_neighbour(&tile, Direction::from(direction)) {
                Some(neighbour) => self.get_habitat_ptr(neighbour.pos.x, neighbour.pos.y),
                None => 0,
            }
        };

        if habitat_a_ptr != habitat_b_ptr {
            if habitat_a_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_a_ptr) }.unknown_flag_0x2c == 0 {
                self.do_show_check(habitat_a_ptr, true);
            }
            if habitat_b_ptr != 0 && unsafe { ref_from_memory::<ZTHabitat>(habitat_b_ptr) }.unknown_flag_0x2c == 0 {
                self.do_show_check(habitat_b_ptr, true);
            }
        }

        unsafe { CHECK_EXHIBIT_MORPH.original()(self as *const Self as *const u32, tile_ptr as *const u32, direction) };
    }

    /// Ports `ZTHabitatMgr::recalculateDeterioration` (`ZTHabitatMgr_recalculateDeterioration.c`/`.asm`):
    /// resets [`Self::unknown_flag_0x6c`] and every real habitat's own deterioration to `0`
    /// (`ZTHabitat::setDeterioration.original()`), then for every fence in the live `GLOBAL_ZTWorldMgr`'s
    /// own entity array that's a genuine fence-family member ([`RVA_FENCE_TYPE_CHECK_ARG`]) and
    /// [`ZTFence::isWorthFixing`], recomputes a deterioration level (`2` if the fence's own `+0x164` field
    /// is set, else `1`) and applies it to the habitat owning the fence's own tile (when it isn't the
    /// "world" habitat) and to the habitat on the far side of the fence's own facing neighbour (via the
    /// same reverse-direction fence-slot check [`Self::recalculate_deterioration`]'s sibling functions use
    /// elsewhere in this file - blocked only when a fence occupies that reverse slot).
    pub fn recalculate_deterioration(&mut self) {
        self.unknown_flag_0x6c = 0;
        for i in 0..self.exhibit_array.len() {
            let habitat_ptr = self.exhibit_array.get_ptr(i);
            if habitat_ptr != 0 {
                unsafe { ZTHABITAT_SET_DETERIORATION.original()(habitat_ptr as *const u32, 0) };
            }
        }

        let world = globals().ztworldmgr();
        for fence_ptr in world.entity_array() {
            if fence_ptr == 0 || !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
                continue;
            }
            if !low_byte_bool(unsafe { IS_WORTH_FIXING.original()(fence_ptr as *const u32) }) {
                continue;
            }

            let needs_major_repair: u8 = get_from_memory(fence_ptr + 0x164);
            let level: u32 = if needs_major_repair != 0 { 2 } else { 1 };

            let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(fence_ptr as *const u32) } as u32;

            let mut near_habitat_ptr = 0;
            if tile_ptr != 0 {
                let tile = get_from_memory::<BFTile>(tile_ptr);
                let candidate = self.get_habitat_ptr(tile.pos.x, tile.pos.y);
                if candidate != 0 && unsafe { ref_from_memory::<ZTHabitat>(candidate) }.unknown_flag_0x2c == 0 {
                    near_habitat_ptr = candidate;
                }
            }

            let mut far_habitat_ptr = 0;
            if tile_ptr != 0 {
                let rotation: u32 = get_from_memory(fence_ptr + 0x12c);
                let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, Direction::from(rotation));
                if neighbour_ptr != 0 {
                    let opposite = rotation.wrapping_sub(4) & 7;
                    let neighbour_tile = get_from_memory::<BFTile>(neighbour_ptr);
                    if ZTHabitat::tile_fence_in_direction(&neighbour_tile, opposite) == 0 {
                        let candidate = self.get_habitat_ptr(neighbour_tile.pos.x, neighbour_tile.pos.y);
                        if candidate != 0 && unsafe { ref_from_memory::<ZTHabitat>(candidate) }.unknown_flag_0x2c == 0 {
                            far_habitat_ptr = candidate;
                        }
                    }
                }
            }

            if near_habitat_ptr != 0 {
                unsafe { ZTHABITAT_SET_DETERIORATION.original()(near_habitat_ptr as *const u32, level) };
            }
            if far_habitat_ptr != 0 {
                unsafe { ZTHABITAT_SET_DETERIORATION.original()(far_habitat_ptr as *const u32, level) };
            }
        }
    }

    /// Ports `ZTHabitatMgr::fillZooExterior` (`ZTHabitatMgr_fillZooExterior.c`/`.asm`) - real vanilla's own
    /// flood-fill is genuinely recursive; reimplemented here with an explicit work-stack instead, visiting
    /// the exact same tiles recursion would without risking a Rust stack overflow on a large open exterior
    /// (the same "no cross-allocator risk, pure per-call scratch state" reasoning [`Self::can_find_path`]'s
    /// own `VecDeque` swap-out doc comment already gives for reimplementing a real-vanilla traversal
    /// without following its own literal call/allocation shape).
    ///
    /// For each tile still marked "in zoo" (`BFTile::isInZoo`), clears its own `0x40` bit at `+0x83` (real
    /// meaning beyond "gates this exact flood-fill" unconfirmed - this is also what makes the walk
    /// self-terminating: a tile visited once never re-passes the `isInZoo` check), then visits every
    /// cardinal neighbour (`0`/`2`/`4`/`6`) that isn't a zoo gate (`standalone::IS_ZOO_GATE`) and isn't
    /// blocked by a zoo perimeter wall on either the near tile's own fence slot or the neighbour's own
    /// reverse-direction slot (`standalone::IS_ZOO_WALL` - **not** [`is_wall`]/
    /// [`RVA_TANK_WALL_TYPE_CHECK_ARG`]'s own `+0x192` "exhibit-fence-passable" flag: `isZooWall` reads the
    /// different, perimeter-specific `+0x190` flag instead, per [`is_wall`]'s own doc comment).
    pub fn fill_zoo_exterior(&self, start_tile_ptr: u32) {
        let world = globals().ztworldmgr();
        let mut stack = vec![start_tile_ptr];
        while let Some(tile_ptr) = stack.pop() {
            if tile_ptr == 0 {
                continue;
            }
            if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.original()(tile_ptr as *const u32, 0) }) {
                continue;
            }
            let existing: u8 = get_from_memory(tile_ptr + 0x83);
            save_to_memory(tile_ptr + 0x83, existing & 0xbf);

            for dir_raw in [0u32, 2, 4, 6] {
                let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, Direction::from(dir_raw));
                if neighbour_ptr == 0 {
                    continue;
                }
                let neighbour_entity: u32 = get_from_memory(neighbour_ptr + 0x10);
                if unsafe { IS_ZOO_GATE.original()(neighbour_entity as *const u32) } {
                    continue;
                }
                let near_tile = get_from_memory::<BFTile>(tile_ptr);
                let near_fence = ZTHabitat::tile_fence_in_direction(&near_tile, dir_raw);
                if unsafe { IS_ZOO_WALL.original()(near_fence as *const u32) } {
                    continue;
                }
                let opposite = dir_raw.wrapping_sub(4) & 7;
                let far_tile = get_from_memory::<BFTile>(neighbour_ptr);
                let far_fence = ZTHabitat::tile_fence_in_direction(&far_tile, opposite);
                if unsafe { IS_ZOO_WALL.original()(far_fence as *const u32) } {
                    continue;
                }
                stack.push(neighbour_ptr);
            }
        }
    }

    /// Ports `ZTHabitatMgr::markZooExterior` (`ZTHabitatMgr_markZooExterior.c`): starting at the zoo
    /// entrance tile ([`Self::get_zoo_entrance_tile_ptr`]), steps forward through the entrance gate's own
    /// facing direction (`+0x12c` on the gate entity) until reaching a tile whose own entity differs from
    /// the gate (i.e. stepped past the gate structure itself, which may span more than one tile sharing the
    /// same entity pointer, or off the map entirely), then flood-fills from there via
    /// [`Self::fill_zoo_exterior`].
    pub fn mark_zoo_exterior(&self) {
        let entrance_tile_ptr = self.get_zoo_entrance_tile_ptr();
        if entrance_tile_ptr == 0 {
            return;
        }
        let gate_entity: u32 = get_from_memory(entrance_tile_ptr + 0x10);
        if gate_entity == 0 {
            return;
        }
        let facing: u32 = get_from_memory(gate_entity + 0x12c);
        let world = globals().ztworldmgr();

        let mut current_ptr = entrance_tile_ptr;
        loop {
            current_ptr = get_neighbour_ptr(world, current_ptr, Direction::from(facing));
            let entity_here: u32 = if current_ptr != 0 { get_from_memory(current_ptr + 0x10) } else { 0 };
            if entity_here != gate_entity {
                break;
            }
        }
        if current_ptr != 0 {
            self.fill_zoo_exterior(current_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::update` (`ZTHabitatMgr_update.c`/`.asm`): calls through to real, un-ported
    /// vanilla `updateGates` first - its own decompile drains the `pending_place_gate_*`/
    /// `pending_replace_*` deferred-request fields [`Self::fence_removed`] writes (see those fields' own
    /// doc comment) and, on one branch, the exact same `isCastClass`-shaped
    /// double-pointer-indirection virtual dispatch `ZTHabitat::getGate`'s own deferral already documents
    /// (see this file's "Correction: getSize/getGate are not leaf functions" note) - genuinely not the
    /// "trivial" function the plan's own regeneration-corrections summary described, since that summary
    /// characterized it only from its `.meta` callee list without reading its actual decompile. Calling
    /// through here reproduces real vanilla's own behavior exactly, at zero cost to the tractable part of
    /// this function.
    ///
    /// Then calls the `update` vtable slot (`+0x24`, [`call_vtable_slot_with_ptr`] - a genuine virtual
    /// dispatch through each habitat's own vtable pointer, exactly matching
    /// `ZTHabitatMgr_update.c`'s `(**(code **)(*(int *)*puVar1 + 0x24))(param_1)`) on the manager's own
    /// "world" habitat ([`Self::pending_habitat_ptr`]) and on every habitat in [`Self::exhibit_array`],
    /// passing `elapsed` through - matching real vanilla's own `.asm` exactly (`this->mbr_0x18` first,
    /// then the array).
    ///
    /// Must be a real virtual dispatch, not a flat call through the base `ZTHabitat::update`'s own fixed
    /// address (`.hooked()`/`.original()` alike): `ZTTankExhibit` overrides this slot with its own,
    /// separate address (`0x0049625f`) that runs the tank's water-level fill/drain tick
    /// (`ZTTankExhibit_update.c`) after delegating back into the base implementation - calling the base
    /// address directly for every habitat, tank or not, skips that override entirely and silently starves
    /// every tank's water level of its per-tick rise/fall (confirmed live regression: newly created and
    /// resized tanks never filled with water).
    pub fn update(&self, elapsed: u32) {
        self.update_gates();

        if self.pending_habitat_ptr != 0 {
            unsafe { call_vtable_slot_with_ptr(self.pending_habitat_ptr, 0x24, elapsed) };
        }
        for i in 0..self.exhibit_array.len() {
            let habitat_ptr = self.exhibit_array.get_ptr(i);
            if habitat_ptr != 0 {
                unsafe { call_vtable_slot_with_ptr(habitat_ptr, 0x24, elapsed) };
            }
        }
    }

    /// Ports `ZTHabitatMgr::pathPlaced` (`ZTHabitatMgr_pathPlaced.c`, `generated.rs`'s `PATH_PLACED`):
    /// scans the 5x5 tile square centred on `tile_ptr` (`±2` in both axes, matching real vanilla's own
    /// inclusive bounds), collecting each distinct, non-"world" ([`ZTHabitat::unknown_flag_0x2c`] clear)
    /// habitat occupying a tile in that square - as a plain `Vec<u32>` local scratch (same "pure per-call
    /// scratch, no cross-allocator risk" reasoning as [`ZTHabitat::path_placed`]'s own candidate list) -
    /// then calls [`ZTHabitat::path_placed`] once per distinct habitat found. Real vanilla's own
    /// `tile_array != 0` guard is dead in practice for a loaded map (always true) and isn't reproduced,
    /// same established convention as [`Self::habitat_tile_changed`]'s own doc comment.
    pub fn path_placed(&self, tile_ptr: u32) {
        let world = globals().ztworldmgr();
        let tile = get_from_memory::<BFTile>(tile_ptr);
        let mut seen: Vec<u32> = Vec::new();

        for x in (tile.pos.x - 2)..=(tile.pos.x + 2) {
            for y in (tile.pos.y - 2)..=(tile.pos.y + 2) {
                if x < 0 || y < 0 || x >= world.map_x_size as i32 || y >= world.map_y_size as i32 {
                    continue;
                }
                let habitat_ptr = self.get_habitat_ptr(x, y);
                if habitat_ptr == 0 || seen.contains(&habitat_ptr) {
                    continue;
                }
                let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
                if habitat.unknown_flag_0x2c != 0 {
                    continue;
                }
                seen.push(habitat_ptr);
            }
        }

        for habitat_ptr in seen {
            unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) }.path_placed(tile_ptr);
        }
    }

    /// Ports `ZTHabitatMgr::pathRemoved` (`ZTHabitatMgr_pathRemoved.c`, `generated.rs`'s `PATH_REMOVED`):
    /// for each of the 8 cached-neighbour-habitat cell-row slots on `tile_ptr`'s own grid cell (the same
    /// slots [`Self::habitat_tile_changed`] marks dirty and [`ZTHabitat::path_placed`] scans), if non-null,
    /// removes `tile_ptr` from that `ZTViewingArea`'s own tile vector (real vanilla
    /// `ZTViewingArea::removeTile`, called through - `ZTViewingArea` itself isn't reimplemented anywhere in
    /// this codebase); if that empties it, removes the viewing area from its owning habitat
    /// ([`ZTHabitat::remove_viewing_area`] - `+0x0` is the viewing area's own owner `ZTHabitat*`), else
    /// marks its own `+0x4c` and `+0x25` ("recreate"/dirty, matching [`ZTHabitat::recreate_oas`]'s own
    /// `+0x25` write) bytes. Finally calls [`ZTHabitat::remove_from_all_vas`] on every habitat in
    /// `exhibit_array` (a tile can be referenced by a neighbouring habitat's own viewing area too, not just
    /// the 8 cached slots above).
    pub fn path_removed(&self, tile_ptr: u32) {
        let tile = get_from_memory::<BFTile>(tile_ptr);
        if let Some(cell_addr) = self.get_habitat_cell_addr(tile.pos.x, tile.pos.y) {
            for slot in 0..8u32 {
                let va_ptr: u32 = get_from_memory(cell_addr + 0x4 + slot * 4);
                if va_ptr == 0 {
                    continue;
                }
                unsafe { ZTVIEWINGAREA_REMOVE_TILE.original()(va_ptr as *const u32, tile_ptr as i32) };
                let vec_begin: u32 = get_from_memory(va_ptr + 0x40);
                let vec_end: u32 = get_from_memory(va_ptr + 0x44);
                if vec_begin == vec_end {
                    let owner: u32 = get_from_memory(va_ptr);
                    unsafe { mut_from_memory::<ZTHabitat>(owner) }.remove_viewing_area(va_ptr);
                } else {
                    save_to_memory::<u8>(va_ptr + 0x4c, 1);
                    save_to_memory::<u8>(va_ptr + 0x25, 1);
                }
            }
        }

        for i in 0..self.exhibit_array.len() {
            let habitat_ptr = self.exhibit_array.get_ptr(i);
            if habitat_ptr != 0 {
                unsafe { mut_from_memory::<ZTHabitat>(habitat_ptr) }.remove_from_all_vas(tile_ptr);
            }
        }
    }
}


impl fmt::Display for ZTHabitatMgr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTHabitatMgr ({:#x}) {{", self.vtable)?;
        writeln!(f, "  map_size_x: {},", self.map_size_x)?;
        writeln!(f, "  map_size_y: {},", self.map_size_y)?;
        writeln!(f, "  zoo_entrance_x: {},", self.zoo_entrance_x)?;
        writeln!(f, "  zoo_entrance_y: {},", self.zoo_entrance_y)?;
        writeln!(f, "  exhibit_array length: {},", self.exhibit_array.len())?;
        writeln!(f, "  other_array_start: {:#x},", self.other_array_start)?;
        writeln!(
            f,
            "  other_array_end: {:#x} ({}),",
            self.other_array_end,
            (self.other_array_end - self.other_array_start) / 12
        )?;
        writeln!(
            f,
            "  other_array_buffer_end: {:#x} ({}),",
            self.other_array_buffer_end,
            (self.other_array_buffer_end - self.other_array_start) / 12
        )?;
        writeln!(f, "  popularity_scale_factor: {},", self.popularity_scale_factor)?;
        write!(f, "}}")
    }
}
