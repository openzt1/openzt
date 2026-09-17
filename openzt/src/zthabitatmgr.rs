use nt_time::{time::UtcDateTime, FileTime};
use openzt_detour::{
    generated::{
        ambients::PLAY as AMBIENTS_PLAY,
        bfentity::GET_TILE as BFENTITY_GET_TILE,
        bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0, WORLD_TO_TILE},
        bfsndmgr::ACQUIRE as BFSNDMGR_ACQUIRE,
        bfunit::GET_PATH_COST as BFUNIT_GET_PATH_COST,
        bftile::{GET_CORNER_ELEVATION as BFTILE_GET_CORNER_ELEVATION, VALIDATE_POSITIONS as BFTILE_VALIDATE_POSITIONS, IS_IN_ZOO as BFTILE_IS_IN_ZOO},
        msvc_std_listuint::INSERT as MSVC_LIST_UINT_INSERT,
        poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4, DEALLOCATE as POOLALLOC_DEALLOCATE},
        standalone::{OPERATOR_DELETE, OPERATOR_NEW, TILE_WITHIN_AVA, IS_ZOO_WALL, IS_ZOO_GATE},
        standalone::WRITE_BYTES_TO_FILE,
        ztfence::{MAKE_FENCE as ZTFENCE_MAKE_FENCE, MAKE_GATE as ZTFENCE_MAKE_GATE, IS_WORTH_FIXING},
        ztanimal::{CAN_SERVICE, IS_SICKLY, IS_HUNGRY_AND_FOODLESS},
        ztkeeper::CLEANS_UP,
        ztunit::GET_HABITAT as ZTUNIT_GET_HABITAT,
        ztshowinfo::{CONSTRUCTOR_1 as ZTSHOWINFO_CONSTRUCTOR, DESTRUCTOR_1 as ZTSHOWINFO_DESTRUCTOR, SAVE as ZTSHOWINFO_SAVE},
        ztshowmgr::{REGISTER_SHOW, UNREGISTER_SHOW},
        zttankwall::SET_IS_COMBINED_CONNECTOR,
        ztui_general::GET_MAPVIEW as ZTUI_GENERAL_GET_MAPVIEW,
        ztui_habitatinfo::ADD_HABITAT as ZTUI_HABITATINFO_ADD_HABITAT,
        ztui_showpanel::SET_EXHIBIT,
        ztvisibilitytesting::TEST_LOS,
        ztviewingarea::{
            UPDATE_AMBIENTS as ZTVIEWINGAREA_UPDATE_AMBIENTS, RECALCULATE_CHARACTERISTICS as ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS,
            ADD_TILE as ZTVIEWINGAREA_ADD_TILE, REMOVE_TILE as ZTVIEWINGAREA_REMOVE_TILE, DESTRUCTOR as ZTVIEWINGAREA_DESTRUCTOR,
            GET_NSEXTENT as ZTVIEWINGAREA_GET_NSEXTENT, GET_EWEXTENT as ZTVIEWINGAREA_GET_EWEXTENT, CONSTRUCTOR as ZTVIEWINGAREA_CONSTRUCTOR,
        },
        ztworldmgr::{
            UPDATE_SHOW_ASSOCIATIONS as ZTWORLDMGR_UPDATE_SHOW_ASSOCIATIONS, PLAY_SMILE_SOUND as ZTWORLDMGR_PLAY_SMILE_SOUND,
            PLAY_FROWN_SOUND as ZTWORLDMGR_PLAY_FROWN_SOUND,
        },
        zthabitat::{
            ADD_AMPHIBIOUS_NEIGHBOR, ADD_HABITAT_TILES, ADD_SHOW_NEIGHBOR, ADD_SHOW_PORTAL, CLEAR_AMPHIBIOUS_NEIGHBORS, CLEAR_SHOW_NEIGHBORS,
            GET_EVENTS, GET_GATE, GET_SHOW_PORTAL, HILITE_AMPHIBIOUS_NEIGHBORS, HILITE_SHOW_NEIGHBORS, RECALCULATE_CHARACTERISTICS,
            REMOVE_HABITAT_TILES, RESET_UNIT_AI, REVISE_SPECIES_LIST, UPDATE_PORTALS, VALIDATE_POSITIONS, SAVE as ZTHABITAT_SAVE,
            CONSTRUCTOR as ZTHABITAT_CONSTRUCTOR, SET_NAME as ZTHABITAT_SET_NAME, RESIZE as ZTHABITAT_RESIZE,
            SET_DIRTY_CHARACTERISTICS as ZTHABITAT_SET_DIRTY_CHARACTERISTICS,
            GET_NUM_ANIMALS, GET_ALL_ANIMALS, GET_SURROUNDING_SPECIES, REMOVE_SPECIES,
            ACCEPT_DONATION, SET_TIME_LAST_SERVICED, TRIGGER_DEATH_ARRIVED,
            GET_OUTERMOST_TANK as ZTHABITAT_GET_OUTERMOST_TANK, SET_DETERIORATION as ZTHABITAT_SET_DETERIORATION,
            UPDATE as ZTHABITAT_UPDATE, NEEDS_SERVICE, SEND_EVENT,
            GET_ANIMALS, GET_AMOUNT_KEEPER_FOOD, GET_FOOD_TO_LEAVE, GET_NUM_KEEPERS, IS_BEING_SERVICED,
            SEND_MAINT_WORKER_CLEANUP_EVENTS, GET_NUM_HUNGRY_FOODLESS_ANIMALS, GET_NUM_SICKLY_ANIMALS,
            GET_SICKLY_ANIMALS, GET_NEAREST_SICK_ANIMAL, GET_VIEWING_AREAS_WITH_GUESTS, HAS_BLDG, REMOVE_VIEWING_AREAS,
            GET_SPECIES_RATING, GENERATE_FACES,
        },
        zthabitatmgr::{
            GET_ZOO_ENTRANCE_TILE, SAVE as ZTHABITATMGR_SAVE, CREATE_HABITAT as ZTHABITATMGR_CREATE_HABITAT,
            ADD_HABITAT as ZTHABITATMGR_ADD_HABITAT, CAN_SEE_HABITAT_FROM_BUILDING, CAN_SEE_SHOW_FROM_BUILDING, CHECK_AMPHIBIOUS_NEIGHBOR,
            CHECK_SHOW_NEIGHBOR, DECREMENT_HABITAT_NUM, FIND_BETTER_GATES_FOR_NEIGHBORS, HABITAT_SEEN_FROM_BUILDING, NAME_HABITAT,
            PLACE_GATE, UPDATE_AMPHIBIOUS_NEIGHBORS_0, UPDATE_AMPHIBIOUS_NEIGHBORS_1, UPDATE_SHOW_NEIGHBORS_0, UPDATE_SHOW_NEIGHBORS_1,
            DO_SHOW_CHECK, SNAP_TANK_WALLS_INWARD, CLEAR_PATHFINDING, CLEAR_STAFF_HABITAT, CAN_FIND_PATH,
            GET_TANK, BREAK_AMPHIBIOUS_CONNECTION, FENCE_REPLACED, RECALCULATE_DETERIORATION, FILL_ZOO_EXTERIOR, MARK_ZOO_EXTERIOR, LEADS_TO,
            UPDATE as ZTHABITATMGR_UPDATE, UPDATE_GATES, CHECK_EXHIBIT_MORPH, AFTER_ENTITY_CHANGE,
        },
        zttankexhibit::{
            CONSTRUCTOR as ZTTANKEXHIBIT_CONSTRUCTOR, UPDATE_TANK_INFO as ZTTANKEXHIBIT_UPDATE_TANK_INFO,
            REMOVE_ILLEGAL_ENTITIES as ZTTANKEXHIBIT_REMOVE_ILLEGAL_ENTITIES, FILL as ZTTANKEXHIBIT_FILL,
        },
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt, mem,
    sync::{LazyLock, Mutex},
};
use tracing::info;

use getset::Getters;

use crate::{
    ambients::Ambients,
    command_console::CommandError,
    globals::{get_module_base, globals},
    lua_fn,
    util::{get_from_memory, low_byte_bool, mut_from_memory, ref_from_memory, save_to_memory, ZTArray, ZTBufferString, ZTString},
    ztmapview::BFTile,
    ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG},
    ztshow::{call_entity_vtable_noargs, call_entity_vtable_u32_noargs, type_check, RVA_ANIMAL_TYPE_CHECK},
    ztshowinfo,
    ztworldmgr::{Direction, ZTWorldMgr},
    zoostatus::ZooStatus,
};

/// ZTHabitatMgr struct
#[derive(Debug)]
#[repr(C)]
pub struct ZTHabitatMgr {
    vtable: u32,                       // 0x000
    pad1: [u8; 0x04],                  // ----------------------- padding: 4 bytes
    map_size_x: u32,                   // 0x008
    map_size_y: u32,                   // 0x00c
    zoo_entrance_x: u32,               // 0x010
    zoo_entrance_y: u32,               // 0x014
    pending_habitat_ptr: u32,          // 0x018 // `ZTHabitatMgr::addHabitat`'s own "deferred" slot - a habitat whose `unknown_flag_0x2c` is set gets stashed here instead of appended to `exhibit_array` (confirmed directly via `.asm`'s `MOV dword ptr [ESI+0x18], EAX` on that branch). Almost certainly the single, always-present "world" habitat (unclaimed/exterior tiles): `getAverageHabitatAttractiveness` skips every `exhibit_array` entry with the same `unknown_flag_0x2c` flag set, and `enterNewMonth` rotates this field's own donation/upkeep totals unconditionally, with no null check - real vanilla never treats it as merely a rare/optional queue slot. See `ZTHabitatMgr::enter_new_month`.
    exhibit_array: ZTArray<ZTHabitat>, // 0x01c (0xc bytes)
    other_array_start: u32,            // 0x028 //TODO: Use ZTArray; Seems to be some kind of mapping from BFTile to ZTHabitat or a ZTHabitat index
    other_array_end: u32,              // 0x02c
    other_array_buffer_end: u32,       // 0x030
    pad3: [u8; 0x20],                  // ----------------------- padding: 32 bytes
    pending_gate_fence_ptr: u32,       // 0x054 // Stashed by `ZTHabitatMgr::removeHabitat` when the habitat being deleted owns a real gate fence still present at its entrance tile (`ZTHabitatMgr_removeHabitat_0.c`'s own `this->mbr_0x54 = dVar1` branch), cleared and converted back to a plain fence by `ZTHabitatMgr::replaceGate` - see `Self::replace_gate`.
    popularity_scale_factor: f32,      // 0x058
    pad4: [u8; 0xc],                   // ----------------------- padding: 12 bytes (0x05c-0x068, not yet reverse-engineered)
    loaded_marker: u32,                // 0x068 // Set to 1 by both the constructor and the tail of `load` (every branch, success or failure past the header); `save` writes it back out raw. Real meaning beyond "always observed as 1" unconfirmed.
    unknown_flag_0x6c: u8,              // 0x06c // Set to 1 unconditionally by the constructor (`ZTHabitatMgr_ZTHabitatMgr.c`'s own `pBVar6->field_0x6c = 1`, confirmed to be `this`-relative despite the decompile's misleading `pBVar6` naming - see `scenery_entity_change_suspended`'s own note); zeroed unconditionally by `ZTHabitatMgr::recalculateDeterioration` (see `Self::recalculate_deterioration`) - no reader confirmed anywhere in the corpus yet, so real meaning still unknown.
    scenery_entity_change_suspended: u8, // 0x06d // Zeroed by the constructor; gates the entire body of `ZTHabitatMgr::sceneryEntityChange` when set (see `Self::scenery_entity_change`) - not written anywhere in this pass's own scope, so always clear/no-op for the functions ported here. Not part of `save`'s own persisted byte layout (`ZTHabitatMgr_save.c` stops at `field_0x68`/`loaded_marker`), confirming these are pure runtime fields.
    pad5: [u8; 0x2],                    // ----------------------- padding: 2 bytes (0x06e-0x070; the ctor also zeroes 0x06e, purpose/reader unconfirmed - 0x06f unconfirmed)
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
    fn distinct_species_catalog_ids(&self, catalog_id_offset: u32) -> usize {
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
    /// `doTankCheck`/`decrementHabitatNum`/`nameHabitat`/`ZTHabitat::setName`/`ZTHabitat::resize`/
    /// `ZTHabitat::setDirtyCharacteristics`/`placeGate`/`updateAmphibiousNeighbors`/`doShowCheck`, the
    /// two constructors, and (tank-only) `snapTankWallsInward`/`findBetterGatesForNeighbors`/
    /// `ZTTankExhibit::updateTankInfo`/`removeIllegalEntities`/`fill`. Only [`Self::add_habitat`]
    /// itself is genuinely reimplemented here, matching this pass's own scoped goal (see
    /// `zthabitatmgr-implementation-plan.md`'s step 5) rather than porting the ~15 other functions this
    /// single method touches.
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
            let ztapp_ptr: u32 = get_from_memory(base + GLOBAL_ZTAPP_RVA);
            let gate_flag: u8 = if ztapp_ptr != 0 { get_from_memory(ztapp_ptr + 0x441) } else { 0 };
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

            unsafe { SNAP_TANK_WALLS_INWARD.original()(mgr_ptr, tank_ptr as *const std::ffi::c_void) };
            if !show_active {
                unsafe { FIND_BETTER_GATES_FOR_NEIGHBORS.original()(mgr_ptr, tank_ptr as *const u32) };
            }
            unsafe { DO_SHOW_CHECK.original()(mgr_ptr, tank_ptr as *const i32, 0) };
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
            unsafe { ZTHABITAT_SET_DIRTY_CHARACTERISTICS.original()(resize_target_ptr as *const u32) };
        }
        unsafe {
            PLACE_GATE.original()(
                mgr_ptr,
                habitat_ptr as *const u32,
                seed_tile_ptr as *const u32,
                resize_tile_ptr as *const u32,
                gate_tile_ptr as *const u32,
            )
        };
        unsafe { UPDATE_AMPHIBIOUS_NEIGHBORS_1.original()(mgr_ptr, habitat_ptr as *const u32) };
        unsafe { DO_SHOW_CHECK.original()(mgr_ptr, habitat_ptr as *const i32, 1) };
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
    fn snapshot_boundary_tile_pairs(begin: u32, end: u32) -> Vec<(u32, u32)> {
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
    unsafe fn can_see_habitat_from_building(habitat_ptr: u32, building_ptr: u32) -> u32 {
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
    fn pathfinding_flags(&self, tile_ptr: u32) -> u8 {
        let tile = get_from_memory::<BFTile>(tile_ptr);
        match self.get_habitat_cell_addr(tile.pos.x, tile.pos.y) {
            Some(addr) => get_from_memory(addr + 0x24),
            None => 0,
        }
    }

    fn set_pathfinding_flag(&self, tile_ptr: u32, bit: u8) {
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

    /// Ports `ZTHabitatMgr::breakAmphibiousConnection` (`ZTHabitatMgr_breakAmphibiousConnection.c`/`.asm`)
    /// - despite its `ZTHabitatMgr::` decompile namespace this is a plain `stdcall` free function taking
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
    /// - the same "no cross-allocator risk, pure per-call scratch state" reasoning [`Self::can_find_path`]'s
    /// own `VecDeque` swap-out doc comment already gives for reimplementing a real-vanilla traversal
    /// without following its own literal call/allocation shape.
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
    /// vanilla `updateGates` first - its own decompile touches 8 currently-unlabeled `ZTHabitatMgr` fields
    /// (`0x34`-`0x54`, previously just padding) and, on one branch, the exact same `isCastClass`-shaped
    /// double-pointer-indirection virtual dispatch `ZTHabitat::getGate`'s own deferral already documents
    /// (see this file's "Correction: getSize/getGate are not leaf functions" note) - genuinely not the
    /// "trivial" function the plan's own regeneration-corrections summary described, since that summary
    /// characterized it only from its `.meta` callee list without reading its actual decompile. Calling
    /// through here reproduces real vanilla's own behavior exactly, at zero cost to the tractable part of
    /// this function.
    ///
    /// Then calls the already-ported [`ZTHabitat::update`] vtable slot (`.hooked()`, so it transparently
    /// reaches whichever override is actually installed - our own detour, or `ZTTankExhibit`'s own separate
    /// address) on the manager's own "world" habitat ([`Self::pending_habitat_ptr`]) and on every habitat in
    /// [`Self::exhibit_array`], passing `elapsed` through - matching real vanilla's own `.asm` exactly
    /// (`this->mbr_0x18` first, then the array).
    pub fn update(&self, elapsed: u32) {
        unsafe { UPDATE_GATES.original()(self as *const Self as *const u32) };

        if self.pending_habitat_ptr != 0 {
            unsafe { ZTHABITAT_UPDATE.hooked()(self.pending_habitat_ptr as *const u32, elapsed) };
        }
        for i in 0..self.exhibit_array.len() {
            let habitat_ptr = self.exhibit_array.get_ptr(i);
            if habitat_ptr != 0 {
                unsafe { ZTHABITAT_UPDATE.hooked()(habitat_ptr as *const u32, elapsed) };
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


#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTHabitat {
    vtable: u32,                 // 0x000
    zt_show_info_ptr: u32,       // 0x004
    amphibious_neighbors_head: u32, // 0x008 // MSVC `std::set<ZTHabitat*>` head/sentinel node pointer for the amphibious-neighbor set - confirmed red-black-tree node layout (`+0x0`=color/isnil, `+0x4`=parent, `+0x8`=left, `+0xc`=right, `+0x10`=value) via `ZTHabitat_hiliteAmphibiousNeighbors.c`'s own in-order walk (see `walk_neighbor_tree`) and `addAmphibiousNeighbor`'s own STL insert helper (both left un-ported - see `Self::hilite_amphibious_neighbors`'s own doc comment). `ZTHabitat_getSize.c` independently walks this exact same field with identical node arithmetic - `getSize` itself remains out of this pass's scope, but this corrects `zthabitatmgr-implementation-plan.md`'s own earlier step 6h note (which speculated this was an unrelated nested-sub-habitat tree).
    pad1a_a1: [u8; 0x8],          // ----------------------- padding: 8 bytes
    show_neighbors_head: u32,    // 0x014 // Same shape as `amphibious_neighbors_head`, for the show-neighbor set (`ZTHabitat_hiliteShowNeighbors.c`/`addShowNeighbor`/`clearShowNeighbors`). The C decompiles mislabel this field `zoo_entrance_y` (an OOAnalyzer type-propagation artifact bleeding in a `ZTHabitatMgr`-shaped name) - trust the `.asm`-confirmed `+0x14` offset, named here for what it actually is.
    pad1a_a2: [u8; 0xd],          // ----------------------- padding: 13 bytes
    neighbor_dirty: u8,          // 0x025 // Set to 1 by `ZTHabitatMgr::habitatTileChanged`/`sceneryEntityChange` on every cached neighbor-habitat pointer found in a changed tile's own grid-cell row (see `ZTHabitatMgr::habitat_tile_changed`), and by the still-unported `ZTHabitat::recreateOAs`/`ZTHabitatMgr::pathRemoved`. No reader identified in this pass - real consumer not yet found in the decompile corpus.
    pad1a_b: [u8; 0x6],          // ----------------------- padding: 6 bytes
    unknown_flag_0x2c: u8,       // 0x02c // Gates ZTThought::ZTThought's acceptance of a passed-in habitat pointer (see ztthoughtmgr.rs); ZTHabitat::recalculateCharacteristics also early-returns when this is set. Meaning not otherwise confirmed.
    characteristics_dirty: u8,   // 0x02d // Gates the lazy `recalculateCharacteristics` call in getAttractiveness/hasKeeperAssigned (see ZTHabitat_getAttractiveness.c/ZTHabitat_hasKeeperAssigned.c) - distinct from unknown_flag_0x2c above.
    pad1b_a: [u8; 0x3],          // ----------------------- padding: 3 bytes
    species_list_dirty: u8,      // 0x031 // Gates the lazy `reviseSpeciesList` call in update() once species_list_timer crosses its threshold - same dirty-flag/timer shape as characteristics_dirty/characteristics_timer, cleared by real vanilla reviseSpeciesList's own (still un-ported) body as a side effect.
    pad1b_b: [u8; 0x2],          // ----------------------- padding: 2 bytes
    viewing_areas_begin: u32,    // 0x034 // Begin pointer of the real vanilla std::vector<ZTViewingArea*> update() walks to tick each entry's own ambient state.
    viewing_areas_end: u32,      // 0x038
    viewing_areas_cap_end: u32,  // 0x03c // The vector's own capacity end - written by `ZTHabitat::addViewingArea`'s own growth path (see `Self::add_viewing_area`), previously undifferentiated padding.
    owned_tiles_ptr: u32,        // 0x040 // Pointer to the sentinel node of this habitat's owned-tile list (see TileListNode below), not a BFTile* itself - see getSize/removeHabitatTiles/validatePositions/resetUnitAI/createEdgePairs, all of which walk it identically.
    pad2a1: [u8; 0x4],            // ----------------------- padding: 4 bytes (0x044 - a real field per `ZTHabitat_createViewingAreas.c`'s own use of it as a second tile-list-shaped container, but that function is deferred - see `Self::create_viewing_areas`'s absence - so this stays unidentified padding rather than a guessed name/shape)
    boundary_tile_pairs_begin: u32, // 0x048 // Begin pointer of the real vanilla `std::vector<std::pair<BFTile*,BFTile*>>` `updateAmphibiousNeighbors_1`/`updateShowNeighbors_1`/`doShowCheck` each independently snapshot-and-iterate (see `ZTHabitatMgr::update_amphibious_neighbors`/`update_show_neighbors`/`do_show_check`), and `ZTHabitat::createEdgePairs` (see `Self::create_edge_pairs`) populates - confirmed via all these decompiles reading/writing the identical `field_0x48`/`field_0x4c`/`field_0x50` triple.
    boundary_tile_pairs_end: u32,   // 0x04c
    boundary_tile_pairs_cap_end: u32, // 0x050 // The vector's own capacity end - written by `Self::create_edge_pairs`'s own growth path, previously undifferentiated padding.
    ambients_begin: u32,         // 0x054 // Begin pointer of the real vanilla std::vector<(u32, Ambients*)> update() walks to play each entry's own ambient sound - see viewing_areas_begin's own doc comment for the same only-begin/end-modeled reasoning.
    ambients_end: u32,           // 0x058
    pad2b_a: [u8; 0x4],          // ----------------------- padding: 4 bytes (cap_end of the ambients vector above)
    species_list_begin: u32,     // 0x060 // Begin pointer of the real vanilla std::vector<catalog-entry*> ZTHabitat::getSpeciesList exposes (`&this->field_0x60`) - lazily recalculated via the same characteristics_dirty gate as get_attractiveness/has_keeper_assigned. See Self::species_list.
    species_list_end: u32,       // 0x064
    pad2b_b1: [u8; 0x4],         // ----------------------- padding: 4 bytes (cap_end of the species-list vector above)
    all_animals_begin: u32,      // 0x06c // Begin pointer of the real vanilla std::vector<ZTAnimal*> ZTHabitat::getAllAnimals exposes (`&this->field_0x6c`, `.asm`-confirmed `LEA EAX,[ESI+0x6c]`) - unlike species_list_begin/end this vector is never recalculated here, only optionally sorted in place by Self::get_all_animals.
    all_animals_end: u32,        // 0x070
    all_animals_cap_end: u32,    // 0x074 // The animals vector's own capacity end - never observed written by any function ported so far (`get_all_animals`'s own sort never grows it), carried here for completeness alongside the vector's other two fields.
    building_list_begin: u32,    // 0x078 // Begin pointer of a real vanilla std::vector<BFEntity*> - `ZTHabitat::hasBldg` (`Self::has_bldg`) is this vector's only reader ported so far; its writer, `ZTHabitat::addToBuildingList`, is still un-ported (mac-only, no Windows decompile as of this pass - see `zthabitatmgr-implementation-plan.md`'s "Guest/animal-experience queries" bulk).
    building_list_end: u32,      // 0x07c
    building_list_cap_end: u32,  // 0x080 // The building-list vector's own capacity end - never written by any function ported so far.
    characteristics_timer: u32,  // 0x084 // Elapsed-time accumulator update() advances every tick; past 6999 sets characteristics_dirty and rerolls to a random 0..200 value via the shared game RNG.
    species_list_timer: u32,     // 0x088 // Same shape as characteristics_timer, gating species_list_dirty/reviseSpeciesList at threshold 7999.
    entrance_tile_ptr: u32,      // 0x08c
    entrance_rotation: u32,      // 0x090
    num_animals: i32,            // 0x094 // ZTHabitat::getNumAnimals's own cached direct-occupant count (`.asm`-confirmed `MOV EAX,[EDI+0x94]`) - not itself gated by characteristics_dirty/recalculateCharacteristics (unlike attractiveness/has_keeper_assigned_raw), so presumably kept current some other way (e.g. incremented directly wherever an animal is added/removed) not otherwise traced by this pass.
    pad3a: [u8; 0x10],           // ----------------------- padding: 16 bytes (0x098-0x0a8, not yet reverse-engineered)
    keeper_food_category_amounts: [i32; 16], // 0x0a8 // Per-category cached "amount of keeper food left out" tally (`ZTHabitat_getAmountKeeperFood.c`'s own `&this->field_0xa8 + category * 4`), recomputed by recalculateCharacteristics when characteristics_dirty is set - same `18-factor per-tile suitability` recalculation `zthabitatmgr-implementation-plan.md`'s step 6m documents as this function's own writer. See Self::get_amount_keeper_food.
    pad3b: [u8; 0x4],            // ----------------------- padding: 4 bytes (0x0e8-0x0ec, not yet reverse-engineered)
    time_last_serviced: u32,     // 0x0ec // ZTHabitat::setTimeLastServiced's own written field (`this->mbr_0xec`, `.asm`-confirmed) - a timestamp, presumably compared against ZTScenarioTimer to gate keeper-service scheduling, though no reader was found anywhere in this pass's own scope (see Self::set_time_last_serviced).
    num_keepers: i32,            // 0x0f0 // Cached count of assigned keepers (subtype `0x6e`) tallied by recalculateCharacteristics's own owned-tile census, per `ZTHabitat_getNumKeepers.c`'s `*(int*)&this->field_0xf0`. See Self::get_num_keepers.
    pad4b: [u8; 0x4],            // ----------------------- padding: 4 bytes (0x0f4-0x0f8, not yet reverse-engineered)
    attractiveness: i32,         // 0x0f8 // ZTHabitat::getAttractiveness's cached result, recomputed by recalculateCharacteristics when characteristics_dirty is set.
    current_donations: f32,     // 0xfc
    last_donations: f32,        // 0x100
    total_donations: f32,       // 0x104
    current_upkeep: f32,         // 0x108
    last_upkeep: f32,            // 0x10c
    total_upkeep: f32,           // 0x110
    unknown_u32_2: u32,          // 0x114
    unknown_u32_3: u32,          // 0x118
    unknown_u32_4: u32,          // 0x11c
    created_timestamp: FileTime, // 0x120
    unknown_nt_time: FileTime,   // 0x128
    pad5a: [u8; 0x1],            // ----------------------- padding: 1 byte
    has_keeper_assigned_raw: u8, // 0x131 // ZTHabitat::hasKeeperAssigned's cached result, recomputed by recalculateCharacteristics when characteristics_dirty is set.
    is_being_serviced_raw: u8,   // 0x132 // ZTHabitat::isBeingServiced's cached result (`this->mbr_0x132`), recomputed by recalculateCharacteristics when characteristics_dirty is set - same shape as has_keeper_assigned_raw. See Self::is_being_serviced.
    pad5b1b: [u8; 0x9],          // ----------------------- padding: 9 bytes (0x133-0x13c, not yet reverse-engineered)
    surrounding_species_begin: u32, // 0x13c // Begin pointer of the real vanilla std::vector<catalog-entry*> ZTHabitat::getSurroundingSpecies exposes (`&this->field_0x13c`, `.asm`-confirmed `LEA EAX,[ESI+0x13c]`), same characteristics_dirty-gated lazy-recalculate shape as species_list_begin/end. Populated by the still-un-ported ZTHabitat::constructSurroundingSpeciesList (unions this habitat's own amphibious/show neighbors' species lists - see zthabitatmgr-implementation-plan.md's step 6f notes), called internally by recalculateCharacteristics.
    surrounding_species_end: u32,   // 0x140
    pad5b2: [u8; 0x10],          // ----------------------- padding: 16 bytes (0x144-0x154: cap_end of the vector above at 0x144, then a msvc_std::map<int, ZTHabitatSuitabilityRecord> tree handle (node pointer + count) at 0x148/0x14c - ZTHabitat's own per-species suitability-scoring cache, keyed by ZTAnimalType::species and holding a 112-byte scoring record (elevation range, neighbor-habitat-tile count, per-scenery/building BFCategory scores, tank water-level min/max, ~15 sub-factors), whole-tree-replaced at the end of every recalculateCharacteristics pass (see ZTHabitat::speciesSuitabilityCache_alloc/_clear/_dtor and cls_0x40143b-disambiguation-handover.md). ZTHabitatSuitabilityRecord's own internal field layout is not yet reverse-engineered.)
    exhibit_name: ZTBufferString, // 0x154 // 3-pointer (start/end/buffer_end) buffer string - ZTHabitat::ZTHabitat zero-inits all of field_0x154/0x158/0x15c before allocating, and field_0x160 is a distinct, separately-referenced pointer (ZTHabitat::playShowStartSound etc.) right after it. Was previously mis-typed as the 2-pointer ZTBoundedString, which shifted every field below 4 bytes early.
    start_sound_ptr: u32,        // 0x160 // Real vanilla SNDSound* for the configured `[sounds] startSound`, built/acquired by set_is_show_exhibit and torn down by set_is_not_show_exhibit.
    end_sound_ptr: u32,          // 0x164 // Real vanilla SNDSound* for the configured `[sounds] endSound` - see start_sound_ptr.
    tank_walk_visited_marker: u8, // 0x168 // Scratch cycle-detection flag shared by `getOutermostTank`/`getNeedyNestedTank`'s own gate-chain/boundary-pair walks (`ZTHabitat_getOutermostTank.c`/`.asm`, `ZTHabitat_getNeedyNestedTank.c`/`.asm`): set to `1` on entry, checked before recursing into a candidate neighbour to stop a cycle. Real meaning outside these two calls unconfirmed; part of the ctor's own 0x168-0x178 zero-init range (see pad6's own note).
    pad6: [u8; 0xf],             // ----------------------- padding: 15 bytes (0x169-0x178: the real head is a msvc_std::tree36 sentinel at +0x16c the ctor zero-inits and ~ZTHabitat tears down via tree36::clear/_Erase, not yet individually reverse-engineered)
}

// `ZTHabitatMgr::createHabitat` (`ZTHabitatMgr_createHabitat.c`) allocates a plain `ZTHabitat` with
// `operator_new(0x178)` and a `ZTTankExhibit` (single-inheritance subclass) with a *separate*
// `operator_new(0x1e8)` - i.e. `ZTHabitat` itself is only 0x178 bytes; `tank_height`/`water_level`/
// `is_filled` (used to live directly on this struct) only exist on the derived `ZTTankExhibit`, from
// 0x178 onward - see that struct below. Putting them here made every full-struct copy of a real,
// non-tank habitat (`get_from_memory::<ZTHabitat>`) over-read past its true 0x178-byte allocation.
const _: () = assert!(std::mem::size_of::<ZTHabitat>() == 0x178);

/// `ZTTankExhibit`, the single-inheritance subclass of [`ZTHabitat`] real tank/show exhibits are
/// actually allocated as (`ZTHabitatMgr::createHabitat`'s second `operator_new(0x1e8)` call). `habitat`
/// embeds the base class at offset 0 (matching real C++ single inheritance layout), so every
/// `ZTHabitat` field/method is reachable through it. Only ever construct this from a pointer already
/// confirmed via `ZTHabitat::is_tank()` - reading it from a real, plain `ZTHabitat` (0x178 bytes) would
/// over-read past that object's actual allocation, exactly the bug this struct split fixes.
#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTTankExhibit {
    habitat: ZTHabitat,   // 0x000 - 0x178
    pad_tank1: [u8; 0xc], // ----------------------- padding: 12 bytes (0x178-0x184)
    tank_height: u32,     // 0x184 // Actual structural tank height (ZTTankExhibit::getTankHeight/setTankHeight); not the field checkTankPlacement compares against, see water_level.
    water_level: u32,     // 0x188 // Current water level (ZTTankExhibit::getWaterLevel); this is what checkTankPlacement's height comparisons actually use.
    pad_tank2: [u8; 0xc], // ----------------------- padding: 12 bytes (0x18c-0x198)
    is_filled: bool,      // 0x198 // Set true by ZTTankExhibit::fill(), false by ZTTankExhibit::drain(). Confirmed against ZTTankExhibit_fill.c/ZTTankExhibit_drain.c/ZTMapView_checkTankPlacement.c (field_0x198, checked as `this_00->field_0x198 == '\0'`) - not flipped between platforms.
    pad_tank3: [u8; 0x4f], // ----------------------- padding: 79 bytes (0x199-0x1e8, not yet reverse-engineered)
}

const _: () = assert!(std::mem::size_of::<ZTTankExhibit>() == 0x1e8);

impl std::ops::Deref for ZTTankExhibit {
    type Target = ZTHabitat;
    fn deref(&self) -> &ZTHabitat {
        &self.habitat
    }
}

/// `ZTHabitat::isRightSalinity` (vtable `+0x28`) has no per-function decompile - only `ZTTankExhibit`'s
/// own override (`zttankexhibit::IS_RIGHT_SALINITY`, `0x004936df`) was captured by the Ghidra pass. Per
/// `FunctionDef::new`'s own doc comment, this is defined locally (address confirmed directly against
/// `private/docs/vtables/ZTHabitat.md`'s own raw vtable dump) rather than hand-editing `generated.rs`.
pub(crate) const IS_RIGHT_SALINITY: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32) -> bool> = FunctionDef::new(0x00446995);

/// Unidentified no-argument helper `ZTHabitatMgr::addHabitat`/`removeHabitat_0`/`removeHabitat_1` all
/// call immediately after setting [`HABITAT_LIST_DIRTY_RVA`] - confirmed directly via
/// `ZTHabitatMgr_addHabitat.asm`'s own tail (`CALL habitatinfo::addHabitat; MOV byte ptr DAT_00639144,
/// 0x1; CALL FUN_0044bb5f`). No per-function decompile/`.meta` exists for `0x0044bb5f`.
const FUN_0044BB5F: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef::new(0x0044bb5f);

/// `DAT_00639144`'s RVA - a shared "habitat list changed" dirty byte flag, set by
/// `ZTHabitatMgr::addHabitat`/`removeHabitat_0`/`removeHabitat_1`/`nameLoadedHabitats` and cleared by
/// `ZTUI::habitatinfo::update` once it's refreshed the habitat list UI off the back of it (confirmed via
/// `habitatinfo_update.c`). RVA = `0x00639144 - 0x400000`.
const HABITAT_LIST_DIRTY_RVA: u32 = 0x0023_9144;

/// Shared "impassable/too expensive" path-cost sentinel (`DAT_00635494`) - read (never written) across
/// this decompile corpus's entire `getPathCost`/`getTerrainCost` family (`BFUnit::getPathCost`,
/// `ZTGuest`/`ZTGuide`/`ZTStaff::getTerrainCost`, `BFPathFinder::cost`), not specific to habitats -
/// [`ZTHabitatMgr::check_enter_habitat`] is just the first port in this codebase to need it. RVA =
/// `0x00635494 - 0x400000`.
const MAX_PATH_COST_RVA: u32 = 0x0023_5494;

/// `isCastClass` type-tag constant (`&DAT_00638710`) `ZTHabitatMgr::clearStaffHabitat`'s own per-entity
/// gate uses before handing an `entity_array` entry to [`FUN_0050C884`] - most likely `ZTHabitat`'s own
/// family tag (this function's whole purpose, per its name, is walking every *habitat* to drop a staff
/// member's assignment from it), sitting between the already-named [`RVA_FENCE_TYPE_CHECK_ARG`]
/// (`0x638660`) and [`RVA_TANK_WALL_TYPE_CHECK_ARG`] (`0x638720`) in what looks like the same class-tag
/// table. Not independently confirmed against a named symbol - same caveat as
/// `RVA_TANK_WALL_TYPE_CHECK_ARG`'s own doc comment. RVA = `0x00638710 - 0x400000`.
const RVA_HABITAT_TYPE_CHECK_ARG: u32 = 0x0023_8710;

/// `DAT_00638588`'s RVA - a single byte `ZTHabitatMgr::terrainChanged` reads to gate its entire body
/// (`if (DAT_00638588 == '\0') { ... }`). Per `species-rating-cache-identification-handover.md`, this
/// reads as a general pause/dialog-blocking flag, not specific to the species-rating cache - not
/// independently confirmed against any other reader, since nothing else in this codebase references this
/// address yet. RVA = `0x00638588 - 0x400000`.
const RVA_GAME_PAUSED_FLAG: u32 = 0x0023_8588;

/// `ZTHabitat::getSpeciesRating`'s real signature, confirmed by reading `ZTHabitat_getSpeciesRating.asm`
/// directly rather than trusting the `.c` decompile or `generated.rs`'s own `GET_SPECIES_RATING` entry:
/// the prologue loads the function's *only* stack argument (the species catalog id) into `EDI` right
/// after the leading `isTank` vtable dispatch, then reuses `EDI` across a long chain of `PUSH EDI` calls -
/// which is exactly what the `.c` decompile's spurious `unaff_EDI` third parameter is (Ghidra mistaking
/// this internal register reuse for a second incoming argument). The function ends `POP EDI; POP ESI;
/// RET 0x4` - one 4-byte stack argument, no `FSTP` before the return, so the accumulated `FADD` chain's
/// final value is left on `ST0` as the real return. Real signature:
/// `unsafe extern "thiscall" fn(*const u32, i32) -> f32` - not `generated.rs`'s declared
/// `fn(*const u32, *const f32, i32) -> *const f32`, which is the same x87-return mistyping this codebase's
/// own `ZooStatus::getStatus`/`GET_STATUS` entry already hit (see `zoostatus.rs`), just not yet
/// regenerated for this entry. Per `CLAUDE.md`'s standing rule this is a call-site transmute to the real
/// signature, not a hand-edit of `generated.rs` itself - surface the real signature to whoever next runs
/// the Ghidra/OOAnalyzer pass.
unsafe fn get_species_rating(habitat_ptr: u32, species_key: i32) -> f32 {
    let real_fn: unsafe extern "thiscall" fn(*const u32, i32) -> f32 = unsafe { mem::transmute(GET_SPECIES_RATING.address) };
    unsafe { real_fn(habitat_ptr as *const u32, species_key) }
}

/// One entry of the species-rating cache `ZTHabitatMgr::terrainAboutToBeChanged`/`terrainChanged` share -
/// see [`SPECIES_RATING_CACHE`]'s own doc comment.
struct SpeciesRatingCacheEntry {
    habitat_ptr: u32,
    ratings: HashMap<i32, f32>,
}

/// Ports the per-run species-rating cache `ZTHabitatMgr::terrainAboutToBeChanged`/`terrainChanged` share
/// via real vanilla's own file-scope `DAT_0063b9f0`/`_f4`/`_f8` vector of (real vanilla's own, invented
/// placeholder name) `ZTHabitatSpeciesRatingCacheEntry` records - see
/// `species-rating-cache-identification-handover.md` for the full identification trail.
///
/// Confirmed genuinely per-call scratch: `terrainAboutToBeChanged` clears it at the very top of every
/// run (real vanilla's own `vector::erase(begin(), end())`), and no other function anywhere in the
/// decompile corpus reads or writes it (checked in the handover doc) - `ZTHabitat::recalculateCharacteristics`
/// phases 4/6 reuse the same *tree mechanics* but against their own separate, function-local trees, not
/// this shared vector. So this is modeled as an independent Rust-side store rather than real vanilla
/// memory (style 2 of `CLAUDE.md`'s reimplementation-pattern section) - nothing else needs to stay
/// binary-compatible with it, and real vanilla's own RB-tree-per-habitat/double-copy-into-the-vector
/// machinery (see the handover doc's Follow-ups 2/4) collapses to a plain `HashMap` per entry.
static SPECIES_RATING_CACHE: LazyLock<Mutex<Vec<SpeciesRatingCacheEntry>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Real vanilla's own unidentified per-entity worker (`ZTHabitatMgr_clearStaffHabitat.asm`'s only real
/// payload call: `MOV ECX, entity; CALL FUN_0050c884` with `staff_ptr` pushed as its one stack argument) -
/// called through unmodified rather than reimplemented, matching this file's own [`FUN_0044BB5F`]/
/// [`FUN_005B66D7`] precedent for a genuinely unidentified helper: no decompile/`.meta` exists for
/// `0x0050c884` to identify its real name or body.
const FUN_0050C884: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32)> = FunctionDef::new(0x0050c884);

/// `ZTHabitat::getAllAnimals`'s own real sort comparator (`ZTHabitat_getAllAnimals.c`'s own repeated
/// `FUN_004690cd(a, b)` calls throughout its inlined MSVC introsort) - a genuinely unidentified helper,
/// same precedent as [`FUN_0044BB5F`]/[`FUN_0050C884`]: no decompile/`.meta` exists for `0x004690cd`
/// beyond the address itself, embedded directly in the caller's own decompile. Called through rather
/// than reimplemented so [`ZTHabitat::get_all_animals`]'s own Rust-side sort produces the exact same
/// final ordering as real vanilla's, without needing this comparator's own logic understood.
const GET_ALL_ANIMALS_SORT_COMPARATOR: FunctionDef<unsafe extern "cdecl" fn(u32, u32) -> bool> = FunctionDef::new(0x004690cd);

/// Base of vanilla's shared small-object freelist bucket array, bucketed by `(byte_capacity - 1) >> 3` -
/// the same `DAT_00638000` family `ambients.rs`'s `RVA_GROUP_ARRAY_FREELIST_BUCKETS` already documents
/// and live-tests (`Ambients_~Ambients.asm`'s `SUB EAX,1; SAR EAX,3; MOV EAX,[EAX*4+0x638000]; ...` per-
/// bucket singly-linked push), confirmed independently here by `ZTHabitat_listen.c`'s own tail (`uVar1 =
/// uVar1 - 1 >> 3; *local_c = DAT_00638000[uVar1]; DAT_00638000[uVar1] = local_c;`) - the identical
/// pattern. RVA = `0x00638000 - 0x400000`.
const RVA_EVENT_VECTOR_FREELIST_BUCKETS: u32 = 0x0023_8000;

/// One node of the generic small-object circular doubly-linked list container `ZTHabitat::owned_tiles_ptr`
/// (`+0x40`) points to as a sentinel - the same node/sentinel shape `BFTile`'s own `+0x0` occupant list
/// uses (see `ZTHabitat::reset_unit_ai`'s doc comment, not yet ported). Confirmed directly against
/// `.asm`, not just decompiled pseudocode: the node allocator `BFTile::cls_0x40143b` (`0x0040143b`)
/// only ever self-references offsets `0x0`/`0x4` when carving a fresh, empty sentinel node - identically
/// on its freelist-reuse fast path (`0x401454`-`0x401469`) and its bump-allocate-a-fresh-chunk cold path
/// (`0x004ab942`-`0x004ab9a8`) - and every corroborating walker (`ZTHabitat_getSize.c`/`_resize.c`/
/// `_removeHabitatTiles.c`/`_validatePositions.c`/`_resetUnitAI.c`/`_createEdgePairs.c`) reads `node+0x0`
/// as `next` and `node+0x8` as the payload (`BFTile*`). Node size is `0x10` bytes, confirmed by the
/// bump-allocator's own chunk math (`0x140`-byte chunks / 20 nodes per chunk = `0x10` bytes/node,
/// `edi=0x14=20` in the `.asm`). Offsets `0x4` (self-ref'd alongside `0x0` on an empty sentinel, i.e. a
/// classic doubly-linked list's `prev`) and `0xc` are never dereferenced by any walker seen so far, so
/// are carried here as opaque fields rather than dropped.
#[repr(C)]
struct TileListNode {
    next: u32,    // 0x0
    _prev: u32,   // 0x4
    payload: u32, // 0x8 - BFTile*
    _unused: u32, // 0xc
}

const _: () = assert!(std::mem::size_of::<TileListNode>() == 0x10);

/// Bucket 1 (16-byte allocations) of the shared small-object freelist array this file's own
/// [`RVA_EVENT_VECTOR_FREELIST_BUCKETS`] documents - [`TileListNode`]s are freed back to this exact
/// bucket. Confirmed directly via `.asm`: `BFTile::cls_0x40143b`'s freelist-reuse fast path pops from
/// `DAT_00638004`, which is `RVA_EVENT_VECTOR_FREELIST_BUCKETS + 4` - matching `(0x10 - 1) >> 3 == 1`,
/// the same bucketing formula `ambients.rs`'s `freelist_bucket_index` documents for this identical
/// bucket-array family.
const TILE_LIST_NODE_FREELIST_HEAD_RVA: u32 = RVA_EVENT_VECTOR_FREELIST_BUCKETS + 4;

/// `GLOBAL_ZTApp`'s RVA - see `ztgamemgr.rs`'s own `GLOBAL_ZTAPP_RVA` doc comment for the shared
/// one-level-of-indirection shape (`MOV EAX, GLOBAL_ZTApp` / `MOV EAX, appInitSuccess` resolving to the
/// same live `ZTApp*` value) and why the real body's "if null, lazily assign a bogus function-pointer
/// sentinel" branch is dead in practice and not reproduced here. Re-declared per this repo's own
/// per-file convention (see `ztshowmgr.rs`'s own copy) - used by [`ZTHabitat::reset_unit_ai`].
const GLOBAL_ZTAPP_RVA: u32 = 0x00638154 - 0x400000;

/// The shared game RNG state (`DAT_00638060`) [`ZTHabitat::update`] rerolls its two lazy-recalculate
/// timers through - the same dword LCG state `ztsoundscape.rs`'s own `GAME_RNG_RVA`/`lcg_next`
/// document and advance for their own, unrelated purpose (ambient position jitter); duplicated locally
/// per this codebase's existing per-file convention (see e.g. this file's own `GLOBAL_DX8SNDMGR_RVA`).
const GAME_RNG_RVA: u32 = 0x00638060 - 0x400000;

/// One MSVC LCG advance over the shared game RNG state: `state = state * 0x343fd + 0x269ec3` with full
/// 32-bit wrap - see `ztsoundscape.rs`'s own `lcg_next` for the identical formula/derivation.
fn lcg_next(state: u32) -> u32 {
    state.wrapping_mul(0x343fd).wrapping_add(0x269ec3)
}

/// The per-habitat field rotation [`ZTHabitatMgr::enter_new_month`] applies identically to every
/// `exhibit_array` entry and to [`ZTHabitatMgr::pending_habitat_ptr`]: `current_donations` (`+0xfc`) ->
/// `last_donations` (`+0x100`), `unknown_u32_2` (`+0x114`) -> `unknown_u32_3` (`+0x118`), and
/// `current_upkeep` (`+0x108`) -> `last_upkeep` (`+0x10c`), zeroing each leading field. Operates on a raw
/// address rather than a typed `&ZTHabitat`/`&mut ZTHabitat` since real vanilla's own body never
/// distinguishes `ZTHabitat` from `ZTTankExhibit` here - both share the same base-class field layout this
/// touches.
fn rotate_month_fields(habitat_ptr: u32) {
    let current_donations: f32 = get_from_memory(habitat_ptr + 0xfc);
    save_to_memory::<f32>(habitat_ptr + 0xfc, 0.0);
    save_to_memory(habitat_ptr + 0x100, current_donations);

    let unknown_u32_2: u32 = get_from_memory(habitat_ptr + 0x114);
    save_to_memory::<u32>(habitat_ptr + 0x114, 0);
    save_to_memory(habitat_ptr + 0x118, unknown_u32_2);

    let current_upkeep: f32 = get_from_memory(habitat_ptr + 0x108);
    save_to_memory::<f32>(habitat_ptr + 0x108, 0.0);
    save_to_memory(habitat_ptr + 0x10c, current_upkeep);
}

/// Walks a `ZTHabitat::owned_tiles_ptr`-shaped sentinel field's real, live value (`sentinel_addr` - the
/// heap-allocated sentinel node's own address, i.e. the container field's *value*, not its address),
/// yielding each real node's address in order and never the sentinel itself. Matches every corroborating
/// walker's own idiom exactly (`next` lives at the node's own offset `0x0`) - see [`TileListNode`]'s doc
/// comment. An empty list (`sentinel_addr`'s own `next` pointing back to itself) yields nothing.
pub(crate) fn walk_tile_list(sentinel_addr: u32) -> impl Iterator<Item = u32> {
    let mut current = get_from_memory::<u32>(sentinel_addr);
    std::iter::from_fn(move || {
        if current == sentinel_addr {
            None
        } else {
            let node = current;
            current = get_from_memory::<u32>(node);
            Some(node)
        }
    })
}

/// Walks a real MSVC `std::set<ZTHabitat*>` (`amphibious_neighbors_head`/`show_neighbors_head`) in
/// sorted (in-order) order, yielding each node's own address - never the head/sentinel itself. Node
/// layout confirmed via `ZTHabitat_hiliteAmphibiousNeighbors.c`/`_hiliteShowNeighbors.c` (byte-for-byte
/// identical bodies): `+0x0`=color/isnil, `+0x4`=parent, `+0x8`=left, `+0xc`=right, `+0x10`=value (the
/// `ZTHabitat*` payload, read directly - not a further pointer indirection). This is a direct,
/// line-for-line translation of that decompile's own in-order-successor walk (start at the head's
/// leftmost descendant; each step take the right child's own leftmost descendant if a right child
/// exists, else climb via `parent` while the current node is its parent's *right* child; terminate when
/// back at `head_ptr`) - not a "clean" textbook re-derivation, to avoid an off-by-one divergence from
/// what real vanilla's own insert/erase actually produced.
///
/// The tree itself (insert/erase) is deliberately left un-ported - see
/// [`ZTHabitat::hilite_amphibious_neighbors`]'s own doc comment - so this only ever reads a tree shape
/// real vanilla's own `addAmphibiousNeighbor`/`addShowNeighbor`/`clearAmphibiousNeighbors`/
/// `clearShowNeighbors` produced, never mutates it.
pub(crate) fn walk_neighbor_tree(head_ptr: u32) -> impl Iterator<Item = u32> {
    let mut current = get_from_memory::<u32>(head_ptr + 0x8);
    std::iter::from_fn(move || {
        if current == head_ptr {
            return None;
        }
        let node = current;

        let right_child = get_from_memory::<u32>(node + 0xc);
        if right_child == 0 {
            let mut parent = get_from_memory::<u32>(node + 0x4);
            let mut cursor = node;
            if cursor == get_from_memory::<u32>(parent + 0xc) {
                loop {
                    cursor = parent;
                    parent = get_from_memory::<u32>(cursor + 0x4);
                    if cursor != get_from_memory::<u32>(parent + 0xc) {
                        break;
                    }
                }
            }
            current = if get_from_memory::<u32>(cursor + 0xc) != parent { parent } else { cursor };
        } else {
            let mut right = right_child;
            let mut left_child = get_from_memory::<u32>(right + 0x8);
            let successor;
            loop {
                if left_child == 0 {
                    successor = right;
                    break;
                }
                right = left_child;
                left_child = get_from_memory::<u32>(left_child + 0x8);
            }
            current = successor;
        }
        Some(node)
    })
}

/// Calls vtable slot `+0x100` on every occupant of `tile` (a live `BFTile*`) - the inner walk
/// `ZTHabitat::reset_unit_ai` performs for each owned tile and for the gate-out tile. `tile+0x0`
/// (`BFTile::unit_list_ptr`) is a [`TileListNode`] sentinel of the exact same shape/pool as
/// `ZTHabitat::owned_tiles_ptr`, confirmed directly via `.asm` - see that field's own doc comment.
fn reset_unit_ai_for_tile_occupants(tile: u32) {
    let sentinel = get_from_memory::<u32>(tile);
    for node in walk_tile_list(sentinel) {
        let occupant = get_from_memory::<TileListNode>(node).payload;
        if occupant != 0 {
            unsafe { call_vtable_slot_noargs(occupant, 0x100) };
        }
    }
}

/// `ZTFenceType`'s (and every other fence/wall-family subtype's) shared `isCastClass` type-tag constant
/// (`&DAT_00638660` in the decompile) - confirms an entity's type is *some* member of the fence/wall
/// family. RVA = `0x00638660 - 0x400000`.
const RVA_FENCE_TYPE_CHECK_ARG: u32 = 0x0023_8660;

/// `isCastClass` type-tag constant (`&DAT_00638720`) used alongside tank-specific checks throughout the
/// decompile corpus (`ZTTankExhibit_updateTankInfo.c`, `_setIsShowExhibit.c`, `_addWaterRipples.c`,
/// `_updateAdjustmentCosts.c`) - most likely `ZTTankWallType`'s own tag, distinguishing a tank wall from
/// the broader fence/wall family [`RVA_FENCE_TYPE_CHECK_ARG`] already covers. Not independently confirmed
/// against a named symbol (no vtable/mac-lookup evidence for this specific address), only inferred from
/// convergent call-site context - see [`ZTHabitatMgr::replace_fence_with_gate`]'s own use. RVA =
/// `0x00638720 - 0x400000`.
const RVA_TANK_WALL_TYPE_CHECK_ARG: u32 = 0x0023_8720;

/// `isCastClass` type-tag constant for `ZTKeeper` - `ZTHabitat::blockService`'s own leading guard uses it
/// to confirm its `ZTStaff*` argument is really a `ZTKeeper*` before doing anything else (the real
/// decompile calls it `CAST_ZTKeeper`, matching `ZTKeeper::cleansUp`'s own identical self-check). Not
/// independently confirmed against a named symbol - `ZTKeeperType_isClass.c`'s own
/// `DAT_00638764 - CAST_ZTKeeper` byte-range read places it at `0x00638760`, which lands exactly on the
/// same `0x10`-spaced isCastClass tag table [`RVA_HABITAT_TYPE_CHECK_ARG`]/[`RVA_TANK_WALL_TYPE_CHECK_ARG`]
/// already document (`...0x638750, 0x638760, 0x638770...`, consistent spacing on both sides) - same
/// caveat as those two. RVA = `0x00638760 - 0x400000`.
pub(crate) const RVA_KEEPER_TYPE_CHECK_ARG: u32 = 0x0023_8760;

/// `DAT_00639148`'s RVA - a shared "gate placement/conversion in progress" flag, set to `1` for the
/// duration of `ZTHabitatMgr::replaceGateWithFence`/`replaceFenceWithGate` and read as an early-return
/// guard by `ZTHabitatMgr::fencePlaced`/`fenceRemoved` (both still real/un-ported) to suppress their own
/// reaction while a gate conversion is already underway. RVA = `0x00639148 - 0x400000`.
const GATE_CONVERSION_IN_PROGRESS_RVA: u32 = 0x0023_9148;

/// Unidentified tail-call helper `ZTHabitatMgr::replaceGate`/`removeHabitat` both jump into
/// (`JMP FUN_005b66d7`, not a normal `CALL` - it owns cleaning up its caller's own stack frame, the same
/// implicit-stack-convention idiom `zthabitatmgr-implementation-plan.md` documents for
/// `fillZooExterior`'s `FUN_005947c3`) when a stashed gate-fence pointer isn't found in
/// `GLOBAL_ZTWorldMgr`'s own `entity_array` - an inconsistency-guard branch that real callers never
/// actually reach in practice (a stashed pointer always came from a real, live world entity). No
/// decompile/`.meta` exists for `0x005b66d7` to identify its real name or purpose further; surfaced here
/// rather than hand-edited into `generated.rs` per `CLAUDE.md`.
const FUN_005B66D7: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef::new(0x005b66d7);

/// `ZTHabitat::addContiguousSpan`'s real fence-passability check (`ZTHabitat_addContiguousSpan.asm`,
/// confirmed identical at all five inlined call sites), **not** `standalone::IS_ZOO_WALL`/`isZooWall`
/// despite sharing the same `isCastClass` ([`RVA_FENCE_TYPE_CHECK_ARG`]) type-family gate. Disassembly
/// shows the two read different fields: `isZooWall` (`_isZooWall.asm`) tests `entity_type+0x190`, while
/// `addContiguousSpan` tests `entity_type+0x192` - two bytes apart, a different per-fence-type config
/// flag. `isZooWall` itself is only ever called from `ZTHabitatMgr::fillZooExterior`
/// (`ZTHabitatMgr_fillZooExterior.c`), the outer *zoo perimeter* exterior flood-fill - unrelated to an
/// exhibit's own tile flood-fill - which is why its flag reads true only for the "Zoo Wall" catalog item
/// and false for an ordinary exhibit fence or tank wall. Reusing it here made every real fence read as
/// passable, letting the flood-fill leak across the whole map; see
/// `show-tank-nondeterminism-handover.md`.
fn is_wall(fence_ptr: u32) -> bool {
    if fence_ptr == 0 {
        return false;
    }
    if !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
        return false;
    }
    let entity_type_ptr: u32 = get_from_memory(fence_ptr + 0x128);
    get_from_memory::<u8>(entity_type_ptr + 0x192) != 0
}

/// `ZTHabitat::doTankCheck`'s own fence-family gate + `+0x193` entity-type byte read - one byte past
/// [`is_wall`]'s own `+0x192`, same `entity_type_matches(fence, RVA_FENCE_TYPE_CHECK_ARG)` shape.
/// Meaning of this specific byte beyond gating [`ZTHabitat::do_tank_check`]'s own "counts as a tank
/// wall" decision not otherwise confirmed; not used anywhere else in this codebase.
fn is_tank_wall(fence_ptr: u32) -> bool {
    if fence_ptr == 0 {
        return false;
    }
    if !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
        return false;
    }
    let entity_type_ptr: u32 = get_from_memory(fence_ptr + 0x128);
    get_from_memory::<u8>(entity_type_ptr + 0x193) != 0
}

/// `ZTHabitatMgr::doShowCheck`'s own fence-family gate + `+0x6f` entity-type byte read - same
/// `entity_type_matches(fence, RVA_FENCE_TYPE_CHECK_ARG)` shape as [`is_wall`], but reading a different
/// byte (`+0x6f` rather than `is_wall`'s own `+0x192`). Meaning of this byte not otherwise confirmed
/// beyond gating `doShowCheck`'s own "found a real show neighbor"/"still valid" accumulation - see
/// `ZTHabitatMgr::do_show_check`. The real decompile's own fallback for a failed `isCastClass` check
/// reads a literal near-null address (`byte ptr [0x6f]`, an OOAnalyzer artifact for an uninitialized
/// local, not a real memory reference) - dead in practice since every caller already confirmed the same
/// `isCastClass` check passed once before reaching this read, so not reproduced here (would be a real
/// null-adjacent read in Rust with no behavioral upside).
fn fence_entity_flag_0x6f(fence_ptr: u32) -> bool {
    if fence_ptr == 0 {
        return false;
    }
    if !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
        return false;
    }
    let entity_type_ptr: u32 = get_from_memory(fence_ptr + 0x128);
    get_from_memory::<u8>(entity_type_ptr + 0x6f) != 0
}

/// The "extra height" term `checkAmphibiousNeighbor`/`checkShowNeighbor` both add to a tank habitat's
/// own `tank_height` (`+0x184`, [`ZTTankExhibit::tank_height`]) before comparing two tanks for a
/// connection: the habitat's first owned tile's own `+0x3c` cached-height field, or `0` if the habitat
/// owns no tiles yet - confirmed identical in both decompiles (`ZTHabitatMgr_checkAmphibiousNeighbor.c`'s
/// `iVar9`, `ZTHabitatMgr_checkShowNeighbor.c`'s `iVar4`/`iVar1`). Meaning of the tile's own `+0x3c` field
/// itself not otherwise confirmed.
fn first_owned_tile_extra_height(habitat_ptr: u32) -> i32 {
    let sentinel: u32 = get_from_memory(habitat_ptr + 0x40);
    let head: u32 = get_from_memory(sentinel);
    let tile_ptr: u32 = get_from_memory(head + 8);
    if tile_ptr == 0 {
        0
    } else {
        get_from_memory(tile_ptr + 0x3c)
    }
}

/// `habitat_ptr`'s own `tank_height` (`+0x184`) plus [`first_owned_tile_extra_height`] - the sum
/// `checkShowNeighbor` compares between two tank habitats before connecting them as show neighbors. Reads
/// `+0x184` as a raw offset rather than through [`ZTTankExhibit`] since the caller only reaches this once
/// `is_tank()` has already confirmed the object really is a `ZTTankExhibit`, matching this file's own
/// established convention (see e.g. `ZTHabitatMgr::replace_fence_with_gate`'s own raw vtable-pointer
/// check) of not paying for a full struct copy just to read one already-known-safe field.
fn tank_height_plus_extra(habitat_ptr: u32) -> i32 {
    let tank_height: i32 = get_from_memory(habitat_ptr + 0x184);
    tank_height + first_owned_tile_extra_height(habitat_ptr)
}

/// The `(source_fence, neighbour_fence)` pair [`is_wall`] must both clear before the flood-fill in
/// [`ZTHabitat::add_habitat_tiles`] may step from `tile` to `neighbour` in `direction` - confirmed by
/// cross-referencing which `BFTile` fence field (`north_fence`=`+0x14`/`east_fence`=`+0x18`/
/// `south_fence`=`+0x1c`/`west_fence`=`+0x20`) `ZTHabitat_addSeedsOnStack.c`/`_addContiguousSpan.c` check
/// for each direction constant - the same cross-reference that corrected [`Direction`]'s own variant
/// names. Only the four cardinal directions are ever used by `addHabitatTiles`.
fn fence_pair(tile: &BFTile, neighbour: &BFTile, direction: &Direction) -> (u32, u32) {
    match direction {
        Direction::North => (tile.north_fence, neighbour.south_fence),
        Direction::East => (tile.east_fence, neighbour.west_fence),
        Direction::South => (tile.south_fence, neighbour.north_fence),
        Direction::West => (tile.west_fence, neighbour.east_fence),
        _ => unreachable!("addHabitatTiles only ever steps in a cardinal direction"),
    }
}

/// `BFMap::getNeighbor(0)` (`generated.rs`'s `bfmap::GET_NEIGHBOR_0`, confirmed byte-for-byte against
/// its real direction table) by real address, but called through the already-ported, already-tested
/// `ZTWorldMgr::get_neighbour` rather than the real vanilla function directly - returns the neighbour
/// tile's own address (`0` when out of map bounds), not a copy.
fn get_neighbour_ptr(world: &ZTWorldMgr, tile_ptr: u32, direction: Direction) -> u32 {
    let tile = get_from_memory::<BFTile>(tile_ptr);
    match world.get_neighbour(&tile, direction) {
        Some(neighbour) => world.get_ptr_from_bftile(&neighbour),
        None => 0,
    }
}

/// The up-to-4 cardinal neighbours of `tile_ptr` that [`ZTHabitatMgr::can_find_path`]'s BFS may step
/// into - a real, in-map neighbour ([`get_neighbour_ptr`]) not blocked by a wall on either side
/// ([`fence_pair`]/[`is_wall`], the same two-sided check [`ZTHabitat::add_habitat_tiles`]'s own
/// flood-fill uses).
fn pathfinding_frontier(world: &ZTWorldMgr, tile_ptr: u32) -> impl Iterator<Item = u32> + '_ {
    let tile = get_from_memory::<BFTile>(tile_ptr);
    [Direction::North, Direction::East, Direction::South, Direction::West].into_iter().filter_map(move |direction| {
        let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction.clone());
        if neighbour_ptr == 0 {
            return None;
        }
        let neighbour = get_from_memory::<BFTile>(neighbour_ptr);
        let (fence_near, fence_far) = fence_pair(&tile, &neighbour, &direction);
        if is_wall(fence_near) || is_wall(fence_far) {
            None
        } else {
            Some(neighbour_ptr)
        }
    })
}

/// Splices a new [`TileListNode`] holding `tile_ptr` in immediately before the list's current head
/// (`sentinel`'s own `next`) - real vanilla's own `msvc_std::list<uint>::insert(&out, head, &value)`
/// (`generated.rs`'s `msvc_std_listuint::INSERT`), called through rather than reimplemented: it
/// internally allocates the node from the exact same shared small-object freelist/bump-arena
/// [`TILE_LIST_NODE_FREELIST_HEAD_RVA`]'s own doc comment documents, so calling through keeps allocation
/// byte-identical to vanilla instead of hand-rolling the bump-arena chunk-carving path - see `CLAUDE.md`'s
/// own cross-allocator safety note (this stays consistent with it: nothing here is ever `Box`-allocated,
/// matching [`ZTHabitat::remove_habitat_tiles`]'s own free-side of the same pool).
fn insert_tile_list_node(sentinel: u32, tile_ptr: u32) {
    let head: u32 = get_from_memory(sentinel);
    let mut out_result: i32 = 0;
    let value: i32 = tile_ptr as i32;
    unsafe {
        MSVC_LIST_UINT_INSERT.original()(&mut out_result as *mut i32 as *const i32, head as i32, &value as *const i32);
    }
}

/// Claims `tile_ptr` for `target` if it isn't already: splices it into `sentinel`'s list
/// ([`insert_tile_list_node`]) and updates its ownership-grid cell plus its own `+0x85` bit `0x1` flag -
/// the same flag [`ZTHabitat::remove_habitat_tiles`] sets when releasing a tile, now cleared implicitly
/// by being claimed again (real vanilla's own body never clears it here either; matched as-is).
/// No-op for a null `tile_ptr` - real vanilla's own seed-claim path can, in principle, insert a null
/// payload node for a null seed (guarded only on the *grid-write* half, not the list-insert), but no real
/// caller (`ZTHabitat::resize`, the only known call site) can produce a null seed once
/// [`ZTHabitat::add_habitat_tiles`]'s own null check has passed and every neighbour lookup here already
/// guards `0` before this is reached - so this port simply never presents a null seed and skips a
/// dead-in-practice bogus insert rather than reproducing it.
fn claim_tile(habitat_mgr: &ZTHabitatMgr, sentinel: u32, target: u32, tile_ptr: u32) {
    if tile_ptr == 0 {
        return;
    }
    let x: i32 = get_from_memory(tile_ptr + 0x34);
    let y: i32 = get_from_memory(tile_ptr + 0x38);
    if habitat_mgr.get_habitat_ptr(x, y) == target {
        return;
    }
    insert_tile_list_node(sentinel, tile_ptr);
    if let Some(cell_addr) = habitat_mgr.get_habitat_cell_addr(x, y) {
        save_to_memory(cell_addr, target);
    }
    let flags: u8 = get_from_memory(tile_ptr + 0x85);
    save_to_memory(tile_ptr + 0x85, flags | 1);
}

/// Ports `ZTHabitat::addContiguousSpan` (`ZTHabitat_addContiguousSpan.c`/`.asm`): extends the maximal
/// contiguous run of tiles reachable from `seed_tile_ptr` first West then East, claiming
/// ([`claim_tile`]) every tile along the way, and returns `(low, high)` - the run's inclusive West/East
/// endpoints (both equal to `seed_tile_ptr` if it doesn't extend either way).
///
/// **Only a wall bounds the scan - ownership does not.** Confirmed directly from the real body: the
/// per-step ownership check only gates whether a tile gets (re)claimed, never whether the scan keeps
/// advancing (`pBVar6 = param_2;`/`pBVar7 = param_2;` run unconditionally after the ownership check,
/// inside the same loop iteration that already passed the wall check). This matches how the game
/// actually uses this - exhibits are fully fenced, so the flood-fill's real boundary is the fence the
/// player placed; ownership only prevents redundant re-insertion of tiles already claimed this pass.
fn add_contiguous_span(world: &ZTWorldMgr, habitat_mgr: &ZTHabitatMgr, sentinel: u32, target: u32, seed_tile_ptr: u32) -> (u32, u32) {
    claim_tile(habitat_mgr, sentinel, target, seed_tile_ptr);

    let mut low = seed_tile_ptr;
    loop {
        let next = get_neighbour_ptr(world, low, Direction::West);
        if next == 0 {
            break;
        }
        let tile = get_from_memory::<BFTile>(low);
        let neighbour = get_from_memory::<BFTile>(next);
        if is_wall(tile.west_fence) || is_wall(neighbour.east_fence) {
            break;
        }
        claim_tile(habitat_mgr, sentinel, target, next);
        low = next;
    }

    let mut high = seed_tile_ptr;
    loop {
        let next = get_neighbour_ptr(world, high, Direction::East);
        if next == 0 {
            break;
        }
        let tile = get_from_memory::<BFTile>(high);
        let neighbour = get_from_memory::<BFTile>(next);
        if is_wall(tile.east_fence) || is_wall(neighbour.west_fence) {
            break;
        }
        claim_tile(habitat_mgr, sentinel, target, next);
        high = next;
    }

    (low, high)
}

/// Checks `tile_ptr`'s neighbour in `direction` (North/South only, per
/// `ZTHabitat_addSeedsOnStack.c`) and pushes it onto `worklist` if it's reachable (no wall either side,
/// per [`fence_pair`]) and not already owned by `target` - the perpendicular half of the scanline
/// flood-fill [`ZTHabitat::add_habitat_tiles`] performs for every tile in a claimed span.
fn enqueue_if_claimable(world: &ZTWorldMgr, habitat_mgr: &ZTHabitatMgr, target: u32, tile_ptr: u32, direction: Direction, worklist: &mut Vec<u32>) {
    let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction.clone());
    if neighbour_ptr == 0 {
        return;
    }
    let tile = get_from_memory::<BFTile>(tile_ptr);
    let neighbour = get_from_memory::<BFTile>(neighbour_ptr);
    let (source_fence, neighbour_fence) = fence_pair(&tile, &neighbour, &direction);
    if is_wall(source_fence) || is_wall(neighbour_fence) {
        return;
    }
    let nx: i32 = get_from_memory(neighbour_ptr + 0x34);
    let ny: i32 = get_from_memory(neighbour_ptr + 0x38);
    if habitat_mgr.get_habitat_ptr(nx, ny) != target {
        worklist.push(neighbour_ptr);
    }
}

/// The configured `[sounds] startSound`/`endSound` name buffers, populated once by real vanilla
/// `showpanel_init` (`OOAnalyzer::BFConfigFile::getString(..., "sounds", "startSound"/"endSound", &DAT)`)
/// long before any `ZTHabitat` is constructed - read-only from this module's perspective.
pub(crate) const START_SOUND_NAME_RVA: u32 = 0x0063_e49c - 0x400000;
pub(crate) const END_SOUND_NAME_RVA: u32 = 0x0063_e4a0 - 0x400000;

/// The showpanel UI's currently-open habitat pointer (`DAT_0063e44c`) - also read by `_addTrick.c`/
/// `_collectAnimalTypes.c`/`_copyListToScript.c`/`_decreaseAdmission.c` for the same purpose.
const SHOWPANEL_CURRENT_HABITAT_RVA: u32 = 0x0063_e44c - 0x400000;

/// Sound-device singleton - same address `ztsoundscape.rs`'s/`ambients.rs`'s own `GLOBAL_DX8SNDMGR_RVA`
/// already document, duplicated locally per this codebase's existing per-file convention.
const GLOBAL_DX8SNDMGR_RVA: u32 = 0x0063_80a8 - 0x400000;

/// Real vanilla `SNDSound`'s static vtable (`SNDSound__vtable_00630bc0`, confirmed directly in
/// `ZTHabitat_setIsShowExhibit.c`'s `param2->vftptr_0x0 = &SNDSound__vtable_00630bc0` assignment) - a
/// data address, RVA'd like every other non-code address in this codebase.
const SNDSOUND_VTABLE_RVA: u32 = 0x0063_0bc0 - 0x400000;

/// The 3-word (`begin`, `end`, `cap_end`) vanilla `std::vector`-shaped out-param `ZTHabitat::getEvents`
/// (still real vanilla - not yet ported) RVO-constructs into, mirroring `zoostatus.rs`'s
/// `VanillaFloatVector` for the same MSVC out-param convention.
#[repr(C)]
struct VanillaEventVector {
    begin: u32,
    end: u32,
    cap_end: u32,
}

impl VanillaEventVector {
    fn rvo_target() -> Self {
        VanillaEventVector { begin: 0, end: 0, cap_end: 0 }
    }

    fn as_ptr(&mut self) -> *mut u32 {
        self as *mut VanillaEventVector as *mut u32
    }
}

/// Frees the event-list buffer `ZTHabitat::listen` receives from `getEvents`, back to wherever real
/// vanilla's own allocator would - by **capacity** (`cap_end - begin`), matching `ZTHabitat_listen.c`'s
/// own math and `ambients.rs`'s live-tested `free_group_array_buffer` exactly (same freelist family,
/// same `>0x80` `operator_delete` cutoff).
pub(crate) fn free_event_vector_buffer(buf: u32, byte_capacity: u32) {
    if buf == 0 {
        return;
    }
    if byte_capacity > 0x80 {
        unsafe { OPERATOR_DELETE.original()(buf) };
        return;
    }
    let bucket_head_addr = get_module_base("zoo.exe") as u32 + RVA_EVENT_VECTOR_FREELIST_BUCKETS + ((byte_capacity - 1) >> 3) * 4;
    let old_head = get_from_memory::<u32>(bucket_head_addr);
    save_to_memory(buf, old_head);
    save_to_memory(bucket_head_addr, buf);
}

/// Appends `value` (a raw `u32` pointer) to a real vanilla `std::vector<T*>` (`begin`/`end`/`cap_end`, one
/// word each) living at `vector_ptr` - the shared out-param growth shape `ZTHabitat_getSicklyAnimals.c`/
/// `_getViewingAreasWithGuests.c` both use identically (real vanilla's own `PoolAlloc::allocate` doubling
/// growth, old buffer freed via [`free_event_vector_buffer`] - the same manual freelist-bucket/
/// `operator_delete` split those two decompiles' own tails use, not a `PoolAlloc::deallocate` call).
/// Distinct from [`Self::push_boundary_tile_pair`]-style helpers, which grow a *field* of `self` - this
/// operates on an arbitrary out-param address handed in by the caller (real vanilla's own stack-allocated
/// local vector, in every known call site).
fn vector_push_pool_alloc4(vector_ptr: u32, value: u32) {
    let begin = get_from_memory::<u32>(vector_ptr);
    let end = get_from_memory::<u32>(vector_ptr + 4);
    let cap_end = get_from_memory::<u32>(vector_ptr + 8);

    if end == cap_end {
        let old_len = (end - begin) / 4;
        let new_cap = if old_len == 0 { 1 } else { old_len * 2 };
        let new_buf = unsafe { POOLALLOC_ALLOCATE.original()(new_cap * 4) } as u32;

        for i in 0..old_len {
            let v: u32 = get_from_memory(begin + i * 4);
            if new_buf != 0 {
                save_to_memory(new_buf + i * 4, v);
            }
        }
        if new_buf != 0 {
            save_to_memory(new_buf + old_len * 4, value);
        }
        free_event_vector_buffer(begin, cap_end - begin);

        save_to_memory(vector_ptr, new_buf);
        save_to_memory(vector_ptr + 4, new_buf + (old_len + 1) * 4);
        save_to_memory(vector_ptr + 8, new_buf + new_cap * 4);
    } else {
        save_to_memory(end, value);
        save_to_memory(vector_ptr + 4, end + 4);
    }
}

/// Allocates and constructs a real vanilla `SNDSound` (`operator_new(8)`, `mbr_0x4 = 0`, real vanilla
/// vtable pointer - see [`SNDSOUND_VTABLE_RVA`]'s doc comment for why the second of
/// `ZTHabitat_setIsShowExhibit.c`'s two identical-shaped allocations, decompiled as a
/// `cls_0x405ec9::~cls_0x405ec9` *destructor* call, is treated as the same construction here: both
/// allocations are 8 bytes and both get acquired through the identical `BFSndMgr::acquire` call
/// immediately after, and the first (unambiguous) allocation explicitly writes `SNDSound`'s own vtable -
/// this reads as the same OOAnalyzer ctor/dtor mislabeling this vtable doc's own "+0x18 slot" correction
/// and `ztshowstate.rs`'s `CONSTRUCTOR` bug both document elsewhere in this codebase, not a genuinely
/// different class), then acquires it by name through `dx8_sndmgr`. Returns `0` (matching vanilla's own
/// null-on-allocation-failure behavior) if `operator_new` fails.
/// Whether `[sounds] startSound`/`endSound` (`DAT_0063e49c`/`DAT_0063e4a0`) genuinely holds a
/// real, `BFConfigFile::getString`-populated name rather than uninitialized memory. Real vanilla's own
/// check is a bare `DAT_... != 0` (first 4 bytes nonzero) - safe in actual gameplay, where
/// `ZTUI::showpanel::init` (which alone ever writes these buffers, and only when expansion 2's gate is
/// open) always runs during normal game boot, long before any save can be loaded. This reimplementation-
/// tests harness's own stripped init sequence never calls it, though, and the memory left there wasn't
/// zero either - live-bisecting a real hang (see this module's own history in
/// `zthabitatmgr-implementation-plan.md`) found genuinely non-printable garbage bytes there, which
/// vanilla's bare nonzero check can't distinguish from a real name and which then hung real vanilla
/// `BFSndMgr::acquire` indefinitely. A real config-loaded name is always a short, printable filename;
/// garbage memory essentially never is - scanning for a null terminator within a generous bound and
/// requiring every byte before it be printable ASCII reliably tells the two apart without changing
/// behavior for any real, correctly-booted game (where this always finds a real name or a clean empty
/// string).
fn looks_like_configured_sound_name(addr: u32) -> bool {
    const MAX_LEN: u32 = 64;
    for i in 0..MAX_LEN {
        let byte = get_from_memory::<u8>(addr + i);
        if byte == 0 {
            return i > 0;
        }
        if !(0x20..=0x7e).contains(&byte) {
            return false;
        }
    }
    false
}

fn construct_and_acquire_sound(dx8_sndmgr: u32, name_ptr: u32) -> u32 {
    let sound = unsafe { OPERATOR_NEW.original()(8) } as u32;
    if sound == 0 {
        return 0;
    }
    save_to_memory(sound, 0u32);
    save_to_memory(sound + 4, get_module_base("zoo.exe") as u32 + SNDSOUND_VTABLE_RVA);
    unsafe { BFSNDMGR_ACQUIRE.original()(dx8_sndmgr as *const u32, sound as *const u32, name_ptr as *const i8) };
    sound
}

/// Calls a no-arg thiscall vtable slot with no meaningful return value - the `+0x60` "stop" call in
/// `ZTHabitat_setIsNotShowExhibit.c`'s sound teardown. Same implicit-`this`-via-thiscall shape as
/// `ztshow.rs`'s `call_entity_vtable_noargs`/`call_entity_vtable_u32_noargs`, just void-returning.
unsafe fn call_vtable_slot_noargs(entity_ptr: u32, slot_offset: u32) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32)>(target) };
    f(entity_ptr);
}

/// Calls a 1-arg (`u8`) thiscall vtable slot - `ZTHabitat_setIsNotShowExhibit.c`'s sound teardown's final
/// `(**vtable)(1)` scalar-deleting-destructor call (slot `+0x0`), the same idiom this vtable doc's own
/// "+0x18 slot" correction documents for `ZTHabitat`'s own destructor.
unsafe fn call_vtable_slot_with_u8(entity_ptr: u32, slot_offset: u32, arg: u8) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u8)>(target) };
    f(entity_ptr, arg);
}

/// Calls a 1-arg (raw pointer) thiscall vtable slot - `ZTHabitat_moveGateTo_0.asm`'s own `+0x1c` gate/
/// fence `setName` dispatch (`PUSH <name-buffer-ptr>; CALL [vtable+0x1c]`), confirmed at the `.asm` level
/// since the C decompile's own struct-offset math for this call is garbled (see
/// [`ZTHabitatMgr::replace_fence_with_gate`]'s own doc comment for the same class of decompile-vs-`.asm`
/// mismatch).
unsafe fn call_vtable_slot_with_ptr(entity_ptr: u32, slot_offset: u32, arg: u32) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32)>(target) };
    f(entity_ptr, arg);
}

/// Calls the unnamed `BFUnit` vtable slot `+0x164` (`BFUnit.md` confirms the slot exists, marked
/// `*unknown*` - no name anywhere in the vtable docs, and no other call site anywhere in the decompile
/// corpus) with a single `BFTile*` argument, returning its `i32` result. [`ZTHabitatMgr::check_enter_habitat`]'s
/// own tank-branch cost comparison, in place of [`BFUNIT_GET_PATH_COST`]'s non-tank equivalent - same
/// "raw pointer-indirection call-through" this codebase already uses for `ZTHabitat::block_service`'s
/// own unnamed `ZTStaff` vtable slots.
unsafe fn call_bfunit_tile_cost_vtable_slot(unit_ptr: u32, tile_ptr: u32) -> i32 {
    let vtable = get_from_memory::<u32>(unit_ptr);
    let target = get_from_memory::<u32>(vtable + 0x164);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> i32>(target) };
    f(unit_ptr, tile_ptr)
}

/// Tears down one of `ZTHabitat`'s two owned `SNDSound`s (`start_sound_ptr`/`end_sound_ptr`), per
/// `ZTHabitat_setIsNotShowExhibit.c`'s identical teardown shape for both fields: a `+0x50` predicate
/// check gating an optional `+0x60` "stop" call, then an unconditional `+0x0(1)` scalar-deleting-
/// destructor call. No-op for a null pointer (mirrors vanilla's own null guard).
fn teardown_sound(sound_ptr: u32) {
    if sound_ptr == 0 {
        return;
    }
    if unsafe { call_entity_vtable_noargs(sound_ptr, 0x50) } {
        unsafe { call_vtable_slot_noargs(sound_ptr, 0x60) };
    }
    unsafe { call_vtable_slot_with_u8(sound_ptr, 0x0, 1) };
}

/// Writes `value`'s raw bytes through real vanilla `WriteBytesToFile` (`standalone::WRITE_BYTES_TO_FILE`) -
/// `true` on success. `.hooked()`, not `.original()`: a `reimplementation-tests` build's `io_redirect`
/// module detours this exact address to redirect the write into an in-memory capture buffer when a
/// capture window is active, and `.hooked()` is this codebase's established way to reach whatever real
/// address currently holds (see `zoostatus.rs`'s own identically-named/documented helper, duplicated
/// locally per this codebase's per-file convention).
fn write_bytes_to_file<T>(value: &T, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(value as *const T as *const u32, mem::size_of::<T>() as u32, 1, file) == 1 }
}

/// Writes `len` raw bytes starting at `ptr` (not a typed value's own address) - the counterpart
/// [`ZTHabitat::save`] needs for its own variable-length `exhibit_name` buffer, which [`write_bytes_to_file`]
/// can't express since its length isn't known at compile time.
fn write_raw_bytes(ptr: u32, len: u32, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(ptr as *const u32, len, 1, file) == 1 }
}

/// Calls vtable slot `+0x1c` (`save`, confirmed via `ZTHabitatMgr_save.asm`'s own `CALL dword ptr
/// [EAX+0x1c]` dispatch) on `entity_ptr` with `file` - the polymorphic dispatch [`ZTHabitatMgr::save`]
/// itself uses to reach whichever `save` override each `exhibit_array` entry's real vtable currently
/// points at (our own detoured [`ZTHabitat::save`] once installed, or any un-detoured `ZTTankExhibit`
/// override), rather than assuming every entry is a plain `ZTHabitat`.
unsafe fn call_save_vtable_slot(entity_ptr: u32, file: *const i8) -> bool {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + 0x1c);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, *const i8) -> u8>(target) };
    f(entity_ptr, file) != 0
}

impl ZTHabitat {
    const TANK_VTABLE_PTR: u32 = 0x006312bc;
    pub fn get_gate_tile_in(&self) -> Option<BFTile> {
        if self.entrance_tile_ptr == 0 {
            return None;
        }
        // info!("ZTHabitat: {}", self);
        // info!("Entrance tile ptr: {:#x}", self.entrance_tile_ptr);
        let tile = get_from_memory::<BFTile>(self.entrance_tile_ptr);
        // info!("Entrance tile: {}", tile);

        let zthm = globals().zthabitatmgr();
        if let Some(gate_habitat) = zthm.get_habitat_by_tile(&tile)
            && gate_habitat == *self {
                return Some(tile);
            }
        let ztwm = globals().ztworldmgr();
        ztwm.get_neighbour(&tile, Direction::from(self.entrance_rotation))
    }

    /// Ports `ZTHabitat::getGateTileOut` (`ZTHabitat_getGateTileOut.c`) - the exact mirror of
    /// [`Self::get_gate_tile_in`]'s own two branches: when the entrance tile currently belongs to this
    /// habitat, "out" is the neighbour tile stepped through the gate (vs. "in"'s own tile itself); when
    /// it doesn't, "out" is the entrance tile itself (vs. "in"'s own neighbour).
    pub fn get_gate_tile_out(&self) -> Option<BFTile> {
        if self.entrance_tile_ptr == 0 {
            return None;
        }
        let tile = get_from_memory::<BFTile>(self.entrance_tile_ptr);

        let zthm = globals().zthabitatmgr();
        if let Some(gate_habitat) = zthm.get_habitat_by_tile(&tile)
            && gate_habitat == *self {
                let ztwm = globals().ztworldmgr();
                return ztwm.get_neighbour(&tile, Direction::from(self.entrance_rotation));
            }
        Some(tile)
    }

    /// Ports `ZTHabitat::getOutermostTank` (`ZTHabitat_getOutermostTank.c`/`.asm`) - the C decompile's own
    /// `cls_0x49e12a` return type is just the decompiler failing to resolve a *self*-recursive call
    /// (confirmed: `generated.rs`'s own `GET_OUTERMOST_TANK` address, `0x0049e12a`, is this exact
    /// function), not a real unidentified helper as `zthabitatmgr-implementation-plan.md`'s own scoping
    /// previously worried - fully resolves the plan's "one remaining unidentified helper" note.
    ///
    /// Walks the chain of tanks connected through each tank's own gate-tile neighbour
    /// ([`Self::get_gate_tile_out`] -> [`ZTHabitatMgr::get_habitat_ptr`] on the gate tile's own position),
    /// marking each visited tank's own [`Self::tank_walk_visited_marker`] along the way, stopping and
    /// returning the last tank found once either the neighbour isn't itself a tank ([`Self::is_tank`]) or
    /// has already been visited this walk (a cycle).
    ///
    /// Real vanilla's own `.asm` unconditionally dereferences the neighbour's vtable pointer even when
    /// `getGateTileOut` finds no tile (`ESI` zeroed at `.15`/`.1466e`) - a genuine null-deref in real
    /// vanilla itself. Reproduced here as "stop and return the current tank" instead of crashing: a real
    /// habitat reachable through this call always owns an entrance gate by the time anything walks its
    /// tank chain, the same "dead in practice" reasoning already used elsewhere in this file (e.g.
    /// [`ZTHabitat::save`]'s own unguarded entrance-tile read) for a real-vanilla-crashing edge case with
    /// no live-observed trigger.
    pub fn get_outermost_tank(&self) -> u32 {
        let mgr = globals().zthabitatmgr();
        let mut current_ptr = self as *const Self as u32;
        loop {
            unsafe { mut_from_memory::<Self>(current_ptr) }.tank_walk_visited_marker = 1;
            let gate_tile = unsafe { ref_from_memory::<Self>(current_ptr) }.get_gate_tile_out();
            let next_ptr = match gate_tile {
                Some(tile) => mgr.get_habitat_ptr(tile.pos.x, tile.pos.y),
                None => 0,
            };
            if next_ptr == 0 {
                return current_ptr;
            }
            let next = unsafe { ref_from_memory::<Self>(next_ptr) };
            if !next.is_tank() {
                return current_ptr;
            }
            if next.tank_walk_visited_marker != 0 {
                return current_ptr;
            }
            current_ptr = next_ptr;
        }
    }

    /// Ports `ZTHabitat::getNeedyNestedTank` (`ZTHabitat_getNeedyNestedTank.c`/`.asm`) - depth-first
    /// search over the amphibious-connected tank chain (via [`Self::boundary_tile_pairs_begin`]/`_end`,
    /// not the `amphibious_neighbors_head` tree despite this file's own earlier "recurses the
    /// amphibious-neighbor tree" description) for a tank whose own `needsService(keeper_ptr, true)` is
    /// true and whose keeper isn't already stationed there ([`ztunit::GET_HABITAT`] on `keeper_ptr` !=
    /// the candidate).
    ///
    /// `generated.rs`'s own `zthabitat::GET_NEEDY_NESTED_TANK` entry previously declared 2 stack args
    /// instead of the real `.asm`'s own `RET 0x4` (1 stack arg) - fixed upstream, unblocking this port.
    /// The real remaining complication this file's earlier scoping flagged - "an unidentified `ZTKeeper`
    /// vtable slot at a very large offset (`+0x24c`)" - dissolves once checked against the vtable docs
    /// already in `private/docs/vtables/`: `ZTKeeper.md`/`ZTUnit.md` both list `+0x24c` (index 147) as a
    /// real, ordinary base-`ZTUnit` slot (not a second/thunk-adjusted vtable - the C decompile's own
    /// `(param_2->cls_0x62d4b4).vftptr_0x0[1]` notation just *looks* like one, same false alarm as
    /// `checkEnterHabitat`'s own correction above), and its address (`0x00410642`) already has a named
    /// decompile - `ZTUnit::getHabitat` - with an existing `generated.rs` entry (`ztunit::GET_HABITAT`).
    ///
    /// Real vanilla calls `getGateTileOut` on the neighbour resolved from each boundary pair's own tile
    /// position unconditionally, with no null guard, for two independent reasons a pair can resolve to no
    /// habitat: the pair's own second tile pointer can itself be null, or [`ZTHabitatMgr::get_habitat_ptr`]
    /// can legitimately return `0` for an unowned tile. Both guarded here (skip that pair) rather than
    /// reproduced - same "dead in practice" reasoning as this file's other unguarded-real-vanilla-read
    /// deferrals, and consistent with [`Self::get_outermost_tank`]'s own null-deref workaround just above.
    ///
    /// A recursive match returns the *immediate* child candidate (matches real vanilla's own `.13c319`
    /// exit, which reloads `ESI` - the child just recursed into - rather than propagating the deeper
    /// call's own return value), not necessarily the actual deepest tank found - faithfully reproduced
    /// rather than "corrected", since callers only need *some* genuinely needy nested tank.
    pub fn get_needy_nested_tank(&self, keeper_ptr: u32) -> u32 {
        let self_ptr = self as *const Self as u32;
        if self.tank_walk_visited_marker != 0 {
            return 0;
        }
        unsafe { mut_from_memory::<Self>(self_ptr) }.tank_walk_visited_marker = 1;

        if !self.is_tank() {
            return 0;
        }

        if low_byte_bool(unsafe { NEEDS_SERVICE.original()(self_ptr as *const u32, keeper_ptr as *const u32, true) }) {
            let keepers_habitat = unsafe { ZTUNIT_GET_HABITAT.original()(keeper_ptr as *const u32) } as u32;
            if keepers_habitat != self_ptr {
                return self_ptr;
            }
        }

        let mgr = globals().zthabitatmgr();
        let ztwm = globals().ztworldmgr();
        let pairs = ZTHabitatMgr::snapshot_boundary_tile_pairs(self.boundary_tile_pairs_begin, self.boundary_tile_pairs_end);
        for (first_ptr, second_ptr) in pairs {
            if first_ptr == 0 || second_ptr == 0 {
                continue;
            }
            let second_tile = get_from_memory::<BFTile>(second_ptr);
            let candidate_ptr = mgr.get_habitat_ptr(second_tile.pos.x, second_tile.pos.y);
            if candidate_ptr == 0 {
                continue;
            }
            let candidate = unsafe { ref_from_memory::<Self>(candidate_ptr) };
            let gate_tile_out_ptr = match candidate.get_gate_tile_out() {
                Some(tile) => ztwm.get_ptr_from_bftile(&tile),
                None => 0,
            };
            if gate_tile_out_ptr != first_ptr {
                continue;
            }
            if candidate.get_needy_nested_tank(keeper_ptr) != 0 {
                return candidate_ptr;
            }
        }
        0
    }

    /// The fence occupying `tile`'s own slot in `direction` (`&tile->field_0x14 + (direction/2)*4` in
    /// the decompile - the same idiom `BFTile`'s own `north_fence`/`east_fence`/`south_fence`/
    /// `west_fence` fields already model, see [`fence_pair`]), or `0` for a non-cardinal direction (real
    /// callers - fence/gate placement - never pass one in practice, but this stays defensive rather than
    /// panicking on an unexpected value from a live, external caller).
    fn tile_fence_in_direction(tile: &BFTile, direction_raw: u32) -> u32 {
        match Direction::from(direction_raw) {
            Direction::North => tile.north_fence,
            Direction::East => tile.east_fence,
            Direction::South => tile.south_fence,
            Direction::West => tile.west_fence,
            _ => 0,
        }
    }

    /// Ports `ZTHabitat::moveGateTo`'s internal 2-arg helper (`ZTHabitat_moveGateTo_0.c`/`.asm`,
    /// `generated.rs`'s `MOVE_GATE_TO_0` at `0x0046616c` - the C decompile's own struct-offset math is
    /// garbled, e.g. the final `setName` call's real target is confirmed via `.asm` as vtable `+0x1c`, not
    /// the decompile's own nested-base guess). Demotes this habitat's current gate ([`GET_GATE`], still
    /// real/un-ported) back into a plain fence and promotes `candidate_fence_ptr` into the new gate -
    /// updating [`Self::entrance_tile_ptr`]/[`Self::entrance_rotation`] and giving the new gate this
    /// habitat's own name - but only when `candidate_fence_ptr` genuinely separates two different
    /// habitats (its own tile and the neighbour stepped through by its own rotation belong to different
    /// [`ZTHabitatMgr::get_habitat_ptr`] owners). No-op (`false`) for a null candidate or one that doesn't
    /// satisfy that check.
    ///
    /// **Must only be called on a live `ZTHabitat` reference** - `self`'s address is written into
    /// directly (`entrance_tile_ptr`/`entrance_rotation`) and passed to real vanilla `getGate`/`setName`,
    /// same precondition [`Self::get_attractiveness`] documents.
    ///
    /// **Not detoured - deliberately unverified, do not wire up without further investigation.**
    /// Live-tested (calling this directly, bypassing any detour) against a real habitat's own current
    /// gate and crash-captured twice, both times inside real, unmodified vanilla `ZTFence::makeGate` ->
    /// `setHealthy` -> `dirtyHabitatEscapability`, which dereferences `+0x2c` on whatever
    /// `ZTHabitatMgr::get_habitat_ptr` returns for `candidate_fence_ptr`'s own tile with **no null guard
    /// at all** (`mov al, byte ptr [esi+0x2c]` with `esi=0`). The first run crashed because the picked
    /// tile had no habitat owner at all; adding a precondition check (only proceed when
    /// `get_habitat_ptr` on the fence's own tile is non-null) still crashed the *second* time at the
    /// identical instruction, meaning `Self::tile_fence_in_direction`'s candidate-discovery lookup (used
    /// by [`Self::move_gate_to`]) does not reliably find the same fence real vanilla `getGate`'s own more
    /// involved logic would - this file's own "Correction: getSize/getGate are not leaf functions" note
    /// already flags `getGate`'s real body as depending on a fence/tile-array layout subtlety
    /// (`isCastClass`-shaped double indirection) this codebase doesn't yet model, and this crash is
    /// concrete live evidence that `moveGateTo` inherits the same gap rather than being the
    /// self-contained leaf this doc's own earlier scoping assumed. Needs that same fence/tile-array
    /// question resolved before this can be trusted enough to detour or live-test again.
    fn move_gate_to_inner(&self, candidate_fence_ptr: u32) -> bool {
        if candidate_fence_ptr == 0 {
            return false;
        }
        let zthm = globals().zthabitatmgr();
        let world = globals().ztworldmgr();

        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(candidate_fence_ptr as *const u32) } as u32;
        let rotation: u32 = get_from_memory(candidate_fence_ptr + 0x12c);
        let neighbour_ptr = if tile_ptr != 0 { get_neighbour_ptr(world, tile_ptr, Direction::from(rotation)) } else { 0 };

        let owner = if tile_ptr != 0 {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            zthm.get_habitat_ptr(tile.pos.x, tile.pos.y)
        } else {
            0
        };
        let neighbour_owner = if neighbour_ptr != 0 {
            let neighbour = get_from_memory::<BFTile>(neighbour_ptr);
            zthm.get_habitat_ptr(neighbour.pos.x, neighbour.pos.y)
        } else {
            0
        };
        if owner == neighbour_owner {
            return false;
        }

        let old_gate_ptr = unsafe { GET_GATE.original()(self as *const Self as *const u32) } as u32;
        if old_gate_ptr != 0 {
            unsafe {
                ZTFENCE_MAKE_FENCE.original()(old_gate_ptr as *const u32);
                call_vtable_slot_noargs(old_gate_ptr, 0x20); // createName
                call_vtable_slot_with_u8(old_gate_ptr, 0x84, 1); // validatePosition(true)
            }
        }

        unsafe { ZTFENCE_MAKE_GATE.original()(candidate_fence_ptr as *const u32) };
        let new_gate_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(candidate_fence_ptr as *const u32) } as u32;
        let new_rotation: u32 = get_from_memory(candidate_fence_ptr + 0x12c);
        let self_addr = self as *const Self as u32;
        save_to_memory(self_addr + 0x8c, new_gate_tile_ptr);
        save_to_memory(self_addr + 0x90, new_rotation);
        unsafe { call_vtable_slot_with_ptr(candidate_fence_ptr, 0x1c, self_addr + 0x154) }; // setName(&exhibit_name)
        true
    }

    /// Ports `ZTHabitat::moveGateTo` (`ZTHabitat_moveGateTo.c`, `generated.rs`'s `MOVE_GATE_TO_1`):
    /// resolves whatever fence currently occupies `tile_ptr`'s own slot in `direction_raw` (only when
    /// it's genuinely a member of the fence/wall family, per [`entity_type_matches`]/
    /// [`RVA_FENCE_TYPE_CHECK_ARG`] - discarded (treated as no candidate) otherwise, matching real
    /// vanilla's own guard) and hands it to [`Self::move_gate_to_inner`]. `direction_raw == 0xffffffff`
    /// (real vanilla's own "no direction" `EDirection` sentinel, outside `Direction`'s own enum domain -
    /// never fed through [`Direction::from`], which would silently default it to `North`) always skips
    /// straight to a null candidate.
    ///
    /// Always returns `true` once past the null-tile guard - real vanilla's own return value is
    /// `CONCAT31(garbage, 1)` (see `CLAUDE.md`'s note on this decompile shape), i.e. the low byte is
    /// hardcoded regardless of `move_gate_to_inner`'s own result. Same live-reference precondition as
    /// [`Self::move_gate_to_inner`].
    pub fn move_gate_to(&self, tile_ptr: u32, direction_raw: u32) -> bool {
        if tile_ptr == 0 {
            return false;
        }
        let candidate_ptr = if direction_raw != 0xffff_ffff {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            let fence_ptr = Self::tile_fence_in_direction(&tile, direction_raw);
            if fence_ptr != 0 && unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
                fence_ptr
            } else {
                0
            }
        } else {
            0
        };
        self.move_gate_to_inner(candidate_ptr);
        true
    }

    /// Ports `ZTHabitat::getAttractiveness` (`ZTHabitat_getAttractiveness.c`/macOS
    /// `ZTHabitat_getAttractiveness.c`, same shape on both platforms): lazily recomputes via the real,
    /// still-un-ported `recalculateCharacteristics` when `characteristics_dirty` is set, then returns the
    /// cached field.
    ///
    /// **Must only be called on a live `ZTHabitat` reference** (one obtained via `ref_from_memory`/a real
    /// detour's `this` pointer, e.g. `exhibit_array`'s own entries while iterated in place) - `self`'s
    /// address is passed straight into vanilla's `recalculateCharacteristics`, so calling this on a stack
    /// copy (e.g. one returned by `ZTArray::get`) would hand vanilla a bogus pointer. Same precondition
    /// [`Self::has_keeper_assigned`] carries, and the same one `zoostatus.rs`'s own `newguest_checks`
    /// documents for its `F_CREATE_GUEST` call-through.
    pub fn get_attractiveness(&self) -> i32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        self.attractiveness
    }

    /// Ports `ZTHabitat::hasKeeperAssigned` (`ZTHabitat_hasKeeperAssigned.c`) - same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_attractiveness`]; see that
    /// method's doc comment for the live-reference precondition this one shares.
    pub fn has_keeper_assigned(&self) -> bool {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        self.has_keeper_assigned_raw != 0
    }

    /// Ports `ZTHabitat::getSpeciesList` (`ZTHabitat_getSpeciesList.c`): same `characteristics_dirty`-gated
    /// lazy-recalculate shape as [`Self::get_attractiveness`]/[`Self::has_keeper_assigned`], then yields
    /// every raw catalog-entry pointer in the real vanilla `std::vector<T*>` at `species_list_begin`/
    /// `species_list_end` (`&this->field_0x60` in the decompile) - callers read whichever offset off each
    /// entry they need (e.g. [`ZTHabitatMgr::distinct_species_catalog_ids`]'s `0x1e4`/`0x1ec` family/
    /// species ids) rather than this method modeling the catalog-entry struct itself.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - the lazy recalculate call passes `self`'s own address to real
    /// vanilla, and the vector fields are re-read from live memory after it runs.
    pub fn species_list(&self) -> impl Iterator<Item = u32> {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let begin = self.species_list_begin;
        let end = self.species_list_end;
        (begin..end).step_by(4).map(get_from_memory::<u32>)
    }

    /// Ports `ZTHabitat::getSurroundingSpecies` (`ZTHabitat_getSurroundingSpecies.c`/`.asm`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::species_list`], yielding raw
    /// catalog-entry pointers from `surrounding_species_begin`/`_end` (`&this->field_0x13c`,
    /// `.asm`-confirmed `LEA EAX,[ESI+0x13c]`) instead. Populated by the still-un-ported
    /// `ZTHabitat::constructSurroundingSpeciesList` (unions this habitat's own amphibious- and
    /// show-neighbor sets' [`Self::species_list`]s - see `zthabitatmgr-implementation-plan.md`'s step 6f
    /// notes for why that union-builder itself is left un-ported this pass), reached transitively through
    /// the same `.original()` `recalculateCharacteristics` call-through below - no separate call-through
    /// is needed here for that reason.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::species_list`].
    pub fn surrounding_species(&self) -> impl Iterator<Item = u32> {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let begin = self.surrounding_species_begin;
        let end = self.surrounding_species_end;
        (begin..end).step_by(4).map(get_from_memory::<u32>)
    }

    /// Ports `ZTHabitat::getNumAnimals` (`ZTHabitat_getNumAnimals.c`/`.asm`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::species_list`], then returns the
    /// cached direct-occupant count ([`Self::num_animals`]) alone when `include_neighbors` is `false`. When
    /// `true`, additionally walks the amphibious-neighbor set ([`walk_neighbor_tree`] over
    /// `amphibious_neighbors_head`) - `.asm`-confirmed identical `+0x8` node arithmetic to
    /// [`Self::hilite_amphibious_neighbors`]'s own walk, per the correction
    /// `zthabitatmgr-implementation-plan.md`'s step 6d recorded for this same field - recursively summing
    /// each neighbor's own `get_num_animals(false)` (never recursing sub-neighbors of sub-neighbors,
    /// matching real vanilla's own `getNumAnimals(neighbor, false)` call exactly).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::species_list`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry, which reads
    /// real `ZTHabitat*` pointers directly out of the tree).
    pub fn get_num_animals(&self, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let mut total = self.num_animals;
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_animals(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getAllAnimals` (`ZTHabitat_getAllAnimals.c`/`.asm`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::species_list`], then, when `sort`
    /// is set, sorts the real vanilla `std::vector<ZTAnimal*>` at `all_animals_begin`/`_end`
    /// (`.asm`-confirmed `LEA EAX,[ESI+0x6c]`) in place before yielding it - real vanilla always returns
    /// the same vector pointer regardless of `sort`, only conditionally reordering its contents first.
    ///
    /// Real vanilla's own sort is an inlined MSVC introsort; reimplemented here as a plain Rust
    /// `sort_by`, calling through to the real, unidentified comparator (`FUN_004690cd`, address known
    /// directly from the decompile's own call site, `bool __cdecl(ZTAnimal*, ZTAnimal*)` - no
    /// `generated.rs` entry exists since Ghidra's OOAnalyzer pass never named it) rather than
    /// reimplementing its own comparison logic. This is a pure in-place reorder of an already-allocated
    /// real vanilla array - no allocation on either side - so there is no cross-allocator risk (per
    /// `CLAUDE.md`'s own `PoolAlloc` caveat) in using Rust's own sort algorithm instead of vanilla's; the
    /// two are not guaranteed to visit equal-order ties identically (`sort_by` is not given vanilla's own
    /// tie-breaking rule, only its `<` predicate), but both converge on the same final ordering for any
    /// genuinely distinct keys.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::species_list`].
    pub fn get_all_animals(&self, sort: bool) -> impl Iterator<Item = u32> {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let begin = self.all_animals_begin;
        let end = self.all_animals_end;
        if sort && end > begin {
            let mut animals: Vec<u32> = (begin..end).step_by(4).map(get_from_memory::<u32>).collect();
            animals.sort_by(|&a, &b| {
                if unsafe { GET_ALL_ANIMALS_SORT_COMPARATOR.original()(a, b) } {
                    std::cmp::Ordering::Less
                } else if unsafe { GET_ALL_ANIMALS_SORT_COMPARATOR.original()(b, a) } {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });
            for (i, &animal_ptr) in animals.iter().enumerate() {
                save_to_memory(begin + (i as u32) * 4, animal_ptr);
            }
        }
        (begin..end).step_by(4).map(get_from_memory::<u32>)
    }

    /// Ports `ZTHabitat::getAnimals` (`ZTHabitat_getAnimals.c`, `generated.rs`'s `GET_ANIMALS`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_all_animals`], but returns the
    /// raw address of the vector header itself (`&this->field_0x6c`, i.e. `&self.all_animals_begin`)
    /// rather than yielding individual animal pointers - real vanilla's own callers treat the return value
    /// as a `std::vector<ZTAnimal*>&`. Distinct from [`Self::get_all_animals`], which is the more useful
    /// entry point for anything actually iterating the animals (and additionally supports sorting); this
    /// exists for parity with real vanilla's own signature/callers (e.g. `ZTHabitat::blockService`'s own
    /// `cls_0x498958::meth_0x498958` callee, per `zthabitatmgr-implementation-plan.md`'s step 6l notes).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_animals(&self) -> u32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        self as *const Self as u32 + 0x6c
    }

    /// Ports `ZTHabitat::getAmountKeeperFood` (`ZTHabitat_getAmountKeeperFood.c`, `generated.rs`'s
    /// `GET_AMOUNT_KEEPER_FOOD`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then reads this habitat's own cached per-category tally
    /// ([`Self::keeper_food_category_amounts`]`[category]`). When `include_neighbors` is set, additionally
    /// sums every amphibious neighbor's own cached tally for the same category ([`walk_neighbor_tree`] over
    /// `amphibious_neighbors_head`) - real vanilla's own recursive call always passes `include_neighbors =
    /// false` for each neighbor, so despite being coded as recursion in the decompile this is only ever one
    /// level deep, matching [`Self::get_num_animals`]'s own established shape for the identical pattern.
    ///
    /// `category` is an opaque index into the 16-entry array, not otherwise validated here - matching real
    /// vanilla, which performs no bounds check either.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_amount_keeper_food(&self, category: u32, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let mut total = self.keeper_food_category_amounts[category as usize];
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_amount_keeper_food(category, false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getFoodToLeave` (`ZTHabitat_getFoodToLeave.c`, `generated.rs`'s
    /// `GET_FOOD_TO_LEAVE`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`]. Sums [`crate::bfentitytype::ZTAnimalType::needed_food`] (`+0x220`, real
    /// vanilla's own `piVar6[0x88]`) over every direct occupant animal ([`entity_type_matches`]-gated,
    /// [`RVA_ANIMAL_TYPE_CHECK`]) whose own [`crate::bfentitytype::ZTAnimalType::keeper_food_type`]
    /// (`+0x3c8`, `piVar6[0xf2]`) equals `category`. When `include_neighbors` is set, additionally sums the
    /// same total over every amphibious neighbor (recursing with `include_neighbors = false`, same
    /// single-level-only shape as [`Self::get_amount_keeper_food`]) and subtracts
    /// [`Self::get_amount_keeper_food`]`(category, include_neighbors)` (i.e. food already left out, over
    /// the same self+neighbors scope) - matching real vanilla's own final `iVar5 - iVar3` exactly. Clamps
    /// the result to `0` (never negative), matching real vanilla's own trailing `-1 < iVar5` check.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    pub fn get_food_to_leave(&self, category: i32, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let mut total = 0i32;
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr == 0 || !unsafe { entity_type_matches(animal_ptr, RVA_ANIMAL_TYPE_CHECK) } {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let keeper_food_type: i32 = get_from_memory(entity_type_ptr + 0x3c8);
            if keeper_food_type == category {
                total += get_from_memory::<i32>(entity_type_ptr + 0x220);
            }
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_food_to_leave(category, false);
            }
            total -= self.get_amount_keeper_food(category as u32, include_neighbors);
        }
        total.max(0)
    }

    /// Ports `ZTHabitat::getNumKeepers` (`ZTHabitat_getNumKeepers.c`, `generated.rs`'s `GET_NUM_KEEPERS`):
    /// same `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_all_animals`], then returns
    /// the cached count ([`Self::num_keepers`]) recalculateCharacteristics's own owned-tile census tallies.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_num_keepers(&self) -> i32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        self.num_keepers
    }

    /// Ports `ZTHabitat::isBeingServiced` (`ZTHabitat_isBeingServiced.c`, `generated.rs`'s
    /// `IS_BEING_SERVICED`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then returns the cached flag ([`Self::is_being_serviced_raw`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn is_being_serviced(&self) -> bool {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        self.is_being_serviced_raw != 0
    }

    /// Ports `ZTHabitat::sendMaintWorkerCleanupEvents` (`ZTHabitat_sendMaintWorkerCleanupEvents.c`,
    /// `generated.rs`'s `SEND_MAINT_WORKER_CLEANUP_EVENTS`): a no-op on a tank habitat ([`Self::is_tank`]);
    /// otherwise walks every owned tile ([`walk_tile_list`] over [`Self::owned_tiles_ptr`]) and, for each
    /// tile with a real occupant ([`crate::ztmapview::BFTile::entity_ptr`]) whose own entity type
    /// ([`entity_type_matches`]-gated, [`RVA_SCENERY_TYPE_CHECK_ARG`]) has either of two unconfirmed
    /// scenery-type flag bytes set (`entity_type+0x11c != 0` or `entity_type+0x12c != 0` - real vanilla's
    /// own `piVar1[0x47]`/`(char)piVar1[0x4b]`, no corroborating name found elsewhere in the corpus this
    /// pass), calls this habitat's own `sendEvent` (vtable `+0x0`) - real vanilla always calls this address
    /// directly rather than through the vtable pointer, since [`SEND_EVENT`] is inherited unchanged by
    /// every known subclass including `ZTTankExhibit` (confirmed via `private/docs/vtables/ZTTankExhibit.md`),
    /// so a raw call-through is exactly equivalent to a real virtual dispatch here.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address is passed directly into
    /// [`SEND_EVENT`].
    ///
    /// **No live test exercises this directly** - live-bisected (a temporary bounded diagnostic walk of
    /// just the owned-tile list, with no `SEND_EVENT` call, completed cleanly in well under 100 nodes for
    /// every real habitat), the real, un-ported `SEND_EVENT` call-through itself hangs/crashes the whole
    /// battery silently (no exception logged, matching the `openzt.log` "went quiet" signature `CLAUDE.md`
    /// documents for this class of failure) the moment it's genuinely invoked in this stripped
    /// reimplementation-tests harness - presumably real vanilla's own `sendEvent` reaches UI/event-queue
    /// infrastructure this harness never initializes. Detoured anyway (byte-for-byte reproducing real
    /// vanilla's own call graph adds no new risk over baseline) and covered by `DETOURS_ENABLED` only -
    /// same "no known safe way to exercise this live" reasoning as [`Self::trigger_death_arrived`]/
    /// `ZTHabitatMgr::fence_replaced`.
    pub fn send_maint_worker_cleanup_events(&self) {
        if self.is_tank() {
            return;
        }
        let self_addr = self as *const Self as u32;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
            if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) } {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
            let flag_a: u32 = get_from_memory(entity_type_ptr + 0x11c);
            let flag_b: u8 = get_from_memory(entity_type_ptr + 0x12c);
            if flag_a != 0 || flag_b != 0 {
                unsafe { SEND_EVENT.original()(self_addr as *const u32) };
            }
        }
    }

    /// Ports `ZTHabitat::getNumHungryFoodlessAnimals` (`ZTHabitat_getNumHungryFoodlessAnimals.c`,
    /// `generated.rs`'s `GET_NUM_HUNGRY_FOODLESS_ANIMALS`): same `characteristics_dirty`-gated
    /// lazy-recalculate shape as [`Self::get_all_animals`], then counts direct-occupant animals for which
    /// real vanilla `ZTAnimal::isHungryAndFoodless` (masked via [`low_byte_bool`] - its own decompile shows
    /// the standard `CONCAT31` undefined-upper-bytes-bool shape `CLAUDE.md` documents) returns true. When
    /// `include_neighbors` is set, additionally sums every amphibious neighbor's own count (recursing with
    /// `false`, same single-level-only shape as [`Self::get_num_animals`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    pub fn get_num_hungry_foodless_animals(&self, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let mut total = 0i32;
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if low_byte_bool(unsafe { IS_HUNGRY_AND_FOODLESS.original()(animal_ptr as *const u32) }) {
                total += 1;
            }
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_hungry_foodless_animals(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getNumSicklyAnimals` (`ZTHabitat_getNumSicklyAnimals.c`, `generated.rs`'s
    /// `GET_NUM_SICKLY_ANIMALS`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then counts direct-occupant animals for which all of the following hold:
    /// real vanilla `ZTAnimal::canService(animal, keeper_ptr)` (masked via [`low_byte_bool`]);
    /// [`Self::keeper_assigned_to_animal`] (`keeper_ptr`'s own assigned-species id list contains the
    /// animal's id); the animal's own tile doesn't have an unconfirmed flag set (`tile+0x85 & 4`, real
    /// vanilla's own `(*(byte*)(tile+0x85) & 4) == 0` check - no corroborating name found elsewhere in the
    /// corpus this pass, plausibly "escaped"/"not visible" given its use alongside a sickness check); real
    /// vanilla `ZTAnimal::isSickly` (masked via [`low_byte_bool`]); and a second, distinct unconfirmed tile
    /// flag check (`tile+0x83 & 3`, real vanilla's own `(*(byte*)(tile+0x83) & 3) == 0`). When
    /// `include_neighbors` is set, additionally sums every amphibious neighbor's own count (recursing with
    /// `false`, same single-level-only shape as [`Self::get_num_animals`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    pub fn get_num_sickly_animals(&self, keeper_ptr: u32, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let mut total = 0i32;
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr == 0 {
                continue;
            }
            if !low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) {
                continue;
            }
            if !Self::keeper_assigned_to_animal(keeper_ptr, animal_ptr) {
                continue;
            }
            let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(animal_ptr as *const u32) } as u32;
            if tile_ptr == 0 || get_from_memory::<u8>(tile_ptr + 0x85) & 4 != 0 {
                continue;
            }
            if !low_byte_bool(unsafe { IS_SICKLY.original()(animal_ptr as *const u32) }) {
                continue;
            }
            if get_from_memory::<u8>(tile_ptr + 0x83) & 3 != 0 {
                continue;
            }
            total += 1;
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_sickly_animals(keeper_ptr, false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getSicklyAnimals` (`ZTHabitat_getSicklyAnimals.c`, `generated.rs`'s
    /// `GET_SICKLY_ANIMALS`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then appends every direct-occupant animal for which real vanilla
    /// `ZTAnimal::isSickly` (masked via [`low_byte_bool`]) returns true onto the real vanilla
    /// `std::vector<ZTAnimal*>` out-param at `out_vector_ptr` ([`vector_push_pool_alloc4`] - the same
    /// `PoolAlloc::allocate`-doubling-growth/manual-freelist-teardown shape this decompile's own tail uses).
    /// `out_vector_ptr` is never read as a pre-existing vector on entry beyond its current
    /// `begin`/`end`/`cap_end` state - matching real vanilla, which only ever appends.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_sickly_animals(&self, out_vector_ptr: u32) {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if low_byte_bool(unsafe { IS_SICKLY.original()(animal_ptr as *const u32) }) {
                vector_push_pool_alloc4(out_vector_ptr, animal_ptr);
            }
        }
    }

    /// Ports `ZTHabitat::getNearestSickAnimal` (`ZTHabitat_getNearestSickAnimal.c`/`.asm`, `generated.rs`'s
    /// `GET_NEAREST_SICK_ANIMAL`): returns `0` immediately if this habitat [`Self::is_tank`] and has a
    /// `ZTShowInfo` attached (real vanilla's own `isTank(this) && this->zt_show_info_ptr != 0` check -
    /// `.asm`-confirmed `MOV EAX, [ESI+0x4]`, the decompile's `this->mbr_0x4` correctly names the raw byte
    /// offset per this repo's own `mbr_0xADDR` convention, not a misread of some other field - a show tank
    /// is simply never a valid maintenance-request target). Also bails if `keeper_ptr` (real vanilla's own
    /// `param_1`, actually a `ZTStaff*`) is null, if `GLOBAL_ZTAIMgr` is null, or if `keeper_ptr`'s own
    /// tile can't be resolved. Real vanilla's own separate `GLOBAL_ZTWorldMgr == 0xfffffff8` ("world not
    /// loaded") sentinel check is not reproduced as its own branch here - [`BFENTITY_GET_TILE`]'s real
    /// body already returns null in that exact state (confirmed via `BFEntity_getTile.c`'s own identical
    /// sentinel check), so the tile-resolution null-check below already covers it with the same observable
    /// result.
    ///
    /// Otherwise builds a scratch `std::vector<ZTAnimal*>` via [`Self::get_sickly_animals`] (real vanilla's
    /// own stack-local out-param, zero-initialized here identically), then scans it for the closest animal
    /// for which all of the following hold: the animal is not itself a `ZTKeeper` (vtable `+0x110`-style
    /// slot the decompile mislabels `virt_meth_0x401115_272` - the `_N` suffix is the decompiler's own
    /// resolved byte offset per `private/docs/vtables/README.md`, not independently identified further this
    /// pass, kept as a raw vtable dispatch); its own tile doesn't have the unconfirmed `+0x85 & 4` flag set
    /// (same check [`Self::get_num_sickly_animals`] uses); [`Self::keeper_assigned_to_animal`]; and real
    /// vanilla `ZTAnimal::canService(animal, keeper_ptr)` (masked via [`low_byte_bool`]). When
    /// `check_can_see` is set, additionally requires a real, un-ported `ZTAIMgr` vtable `+0x1c`-style
    /// visibility check (`virt_meth_0x601e1c_28` in the decompile - called through raw, not independently
    /// identified) to pass. Distance is squared Euclidean over each candidate's own tile `x`/`y` (`+0x34`/`+0x38`,
    /// [`crate::ztmapview::BFTile::pos`]) against `keeper_ptr`'s own tile.
    ///
    /// Frees the scratch vector's buffer via [`vector_push_pool_alloc4`]'s own counterpart,
    /// [`free_event_vector_buffer`], by **capacity** - matching real vanilla's own tail exactly (the same
    /// manual freelist-bucket/`operator_delete` split, not a `PoolAlloc::deallocate` call).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_nearest_sick_animal(&self, keeper_ptr: u32, check_can_see: bool) -> u32 {
        if self.is_tank() && self.zt_show_info_ptr != 0 {
            return 0;
        }
        let ai_mgr = globals().ztaimgr_ptr() as u32;
        if keeper_ptr == 0 || ai_mgr == 0 {
            return 0;
        }
        let keeper_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(keeper_ptr as *const u32) } as u32;
        if keeper_tile_ptr == 0 {
            return 0;
        }

        let mut scratch_vector = [0u32; 3];
        let scratch_ptr = scratch_vector.as_mut_ptr() as u32;
        self.get_sickly_animals(scratch_ptr);

        let mut best_animal = 0u32;
        let mut best_dist = i32::MAX;
        for addr in (scratch_vector[0]..scratch_vector[1]).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            let is_keeper = unsafe { call_entity_vtable_noargs(animal_ptr, 0x110) };
            if is_keeper {
                continue;
            }
            let animal_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(animal_ptr as *const u32) } as u32;
            if animal_tile_ptr == 0 || get_from_memory::<u8>(animal_tile_ptr + 0x85) & 4 != 0 {
                continue;
            }
            if !Self::keeper_assigned_to_animal(keeper_ptr, animal_ptr) {
                continue;
            }
            if !low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) {
                continue;
            }
            if check_can_see && !unsafe { call_entity_vtable_noargs(ai_mgr, 0x1c) } {
                continue;
            }
            let dx: i32 = get_from_memory::<i32>(keeper_tile_ptr + 0x34) - get_from_memory::<i32>(animal_tile_ptr + 0x34);
            let dy: i32 = get_from_memory::<i32>(keeper_tile_ptr + 0x38) - get_from_memory::<i32>(animal_tile_ptr + 0x38);
            let dist = dx * dx + dy * dy;
            if best_animal == 0 || dist < best_dist {
                best_animal = animal_ptr;
                best_dist = dist;
            }
        }

        free_event_vector_buffer(scratch_vector[0], scratch_vector[2] - scratch_vector[0]);
        best_animal
    }

    /// Ports `ZTHabitat::getViewingAreasWithGuests` (`ZTHabitat_getViewingAreasWithGuests.c`,
    /// `generated.rs`'s `GET_VIEWING_AREAS_WITH_GUESTS`): for every non-null entry in
    /// [`Self::viewing_areas_begin`]/`_end`, recalculates it (real, un-ported vanilla
    /// `ZTViewingArea::recalculateCharacteristics`, called through - gated on the viewing area's own dirty
    /// flag at `+0x4c`, not otherwise modeled in this codebase) then, if its own guest count (`+0x54`) is
    /// positive, appends it onto the real vanilla `std::vector<ZTViewingArea*>` out-param at
    /// `out_vector_ptr` ([`vector_push_pool_alloc4`] - same growth shape as [`Self::get_sickly_animals`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds the vector fields
    /// read directly off it.
    pub fn get_viewing_areas_with_guests(&self, out_vector_ptr: u32) {
        for cursor in (self.viewing_areas_begin..self.viewing_areas_end).step_by(4) {
            let va_ptr: u32 = get_from_memory(cursor);
            if va_ptr == 0 {
                continue;
            }
            if get_from_memory::<u32>(va_ptr + 0x4c) != 0 {
                unsafe { ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS.original()(va_ptr as *const u32) };
            }
            if get_from_memory::<i32>(va_ptr + 0x54) > 0 {
                vector_push_pool_alloc4(out_vector_ptr, va_ptr);
            }
        }
    }

    /// Ports `ZTHabitat::hasBldg` (`ZTHabitat_hasBldg.c`, `generated.rs`'s `HAS_BLDG`): membership test
    /// over [`Self::building_list_begin`]/`_end` (a real vanilla `std::vector<BFEntity*>` this pass newly
    /// names - previously undifferentiated padding, see that field's own doc comment) for `entity_ptr`.
    pub fn has_bldg(&self, entity_ptr: u32) -> bool {
        (self.building_list_begin..self.building_list_end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == entity_ptr)
    }

    /// Ports `ZTHabitat::removeViewingAreas` (`ZTHabitat_removeViewingAreas.c`, `generated.rs`'s
    /// `REMOVE_VIEWING_AREAS`): a no-op if `unknown_flag_0x2c` is set (the same guard
    /// `recalculateCharacteristics`'s own early-return uses). Otherwise destroys and frees every entry in
    /// [`Self::viewing_areas_begin`]/`_end` (call-through to real vanilla `ZTViewingArea::~ZTViewingArea` +
    /// `operator_delete`, same as [`Self::remove_viewing_area`]), then resets `_end` back to `_begin` -
    /// keeps the backing buffer rather than freeing it, unlike a full vector destructor.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds the vector field
    /// write at the end.
    pub fn remove_viewing_areas(&mut self) {
        if self.unknown_flag_0x2c != 0 {
            return;
        }
        let self_addr = self as *const Self as u32;
        let begin = self.viewing_areas_begin;
        let end = self.viewing_areas_end;
        let mut cursor = begin;
        while cursor != end {
            let va_ptr: u32 = get_from_memory(cursor);
            if va_ptr != 0 {
                unsafe {
                    ZTVIEWINGAREA_DESTRUCTOR.original()(va_ptr as *const u32);
                    OPERATOR_DELETE.original()(va_ptr);
                }
            }
            cursor += 4;
        }
        save_to_memory(self_addr + 0x38, begin);
    }

    /// Ports `ZTHabitat::removeSpecies` (`ZTHabitat_removeSpecies.c`/`.asm`): scans the real vanilla
    /// `std::vector<(u32, Ambients*)>` at `ambients_begin`/`ambients_end` (the same vector
    /// [`Self::update`] already walks to play each entry's own ambient sound) for an 8-byte pair whose
    /// key matches `species_key`, no-oping if none is found. On a match, tears down the pair's own
    /// `Ambients*` (if non-null) via [`Ambients::destruct`] (a faithful field-level reimplementation of
    /// real vanilla `Ambients::~Ambients` - safe to call on an object either pole allocated, since both
    /// interpret the same real memory layout and free through the same real vanilla allocator) followed
    /// by `OPERATOR_DELETE` on the outer block, then shifts every later pair down by one slot
    /// (`.asm`-confirmed plain `memmove`-shaped copy, no reallocation) and decrements `ambients_end` by
    /// `8` - matches vanilla's own vector-erase exactly, no PoolAlloc/freelist involvement since this
    /// vector's own growth (real vanilla `ZTHabitat::addSpecies`, still un-ported - see
    /// `zthabitatmgr-implementation-plan.md`'s step 6f notes) never shrinks capacity on erase either.
    ///
    /// `species_key` is treated as an opaque equality key throughout (matching real vanilla, which never
    /// dereferences it here) - real callers pass the same catalog-entry pointer value `addSpecies` itself
    /// stored as each pair's key.
    pub fn remove_species(&mut self, species_key: u32) {
        let mut p = self.ambients_begin;
        while p != self.ambients_end {
            if get_from_memory::<u32>(p) == species_key {
                let ambients_ptr = get_from_memory::<u32>(p + 0x4);
                if ambients_ptr != 0 {
                    unsafe { (*(ambients_ptr as *mut Ambients)).destruct() };
                    unsafe { OPERATOR_DELETE.original()(ambients_ptr) };
                }

                let mut src = p + 0x8;
                let mut dst = p;
                while src != self.ambients_end {
                    save_to_memory(dst, get_from_memory::<u32>(src));
                    save_to_memory(dst + 0x4, get_from_memory::<u32>(src + 0x4));
                    src += 0x8;
                    dst += 0x8;
                }
                self.ambients_end -= 0x8;
                return;
            }
            p += 0x8;
        }
    }

    /// Ports `ZTHabitat::setDirtyCharacteristics` (`ZTHabitat_setDirtyCharacteristics.c`/`.asm`): sets
    /// `characteristics_dirty`, guarded so it never re-enters an already-dirty habitat (the same guard
    /// real vanilla's own recursive walk depends on to terminate over a neighbor graph that may contain
    /// cycles), then recursively marks every amphibious- and show-neighbor
    /// ([`walk_neighbor_tree`] over [`Self::amphibious_neighbors_head`]/[`Self::show_neighbors_head`],
    /// the same two sets step 6d already resolved) dirty in turn - both trees are walked (unlike
    /// [`Self::set_time_last_serviced`], which only walks the amphibious set), confirmed directly against
    /// the decompile's own two, near-identical tree-walk blocks.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds directly into
    /// the tree walk, and each neighbor visited must also be live (true for every `walk_neighbor_tree`
    /// entry, which reads real `ZTHabitat*` pointers directly out of the tree).
    pub fn set_dirty_characteristics(&mut self) {
        if self.characteristics_dirty != 0 {
            return;
        }
        self.characteristics_dirty = 1;
        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) }.set_dirty_characteristics();
        }
        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) }.set_dirty_characteristics();
        }
    }

    /// Ports `ZTHabitat::acceptDonation` (`ZTHabitat_acceptDonation.c`): adds `amount` to both
    /// `current_donations`/`total_donations`, then calls the already-ported
    /// [`crate::zoostatus::ZooStatus::increase_donations`]/[`crate::ztgamemgr::ZTGameMgr::add_cash`]
    /// directly on the real global `ZTGameMgr`/its embedded `ZooStatus` sub-object (`this + 0x10`) - the
    /// same `&GLOBAL_ZTGameMgr->field_0x10` access `zoostatus.rs`'s own `f_grant_donation` already
    /// establishes for this exact call pair, reused here rather than a real-vanilla call-through since
    /// both real methods are already faithful, live-tested Rust ports.
    pub fn accept_donation(&mut self, amount: f32) {
        self.current_donations += amount;
        self.total_donations += amount;
        let ztgamemgr_ptr = globals().ztgamemgr_ptr();
        let zoostatus_ptr = (ztgamemgr_ptr as u32 + 0x10) as *mut ZooStatus;
        unsafe { (*zoostatus_ptr).increase_donations(amount) };
        unsafe { (*ztgamemgr_ptr).add_cash(amount) };
    }

    /// Ports `ZTHabitat::setTimeLastServiced` (`ZTHabitat_setTimeLastServiced.c`/`.asm`): sets
    /// [`Self::time_last_serviced`] (`+0xec`), then - when `propagate` is set - recursively calls itself
    /// with `propagate = false` on every amphibious neighbor ([`walk_neighbor_tree`] over
    /// [`Self::amphibious_neighbors_head`] only, not `show_neighbors_head` too - confirmed directly
    /// against the decompile, which reads only `field_0x8`). Real vanilla's own decompile has no guard
    /// against visiting the same neighbor twice (unlike [`Self::set_dirty_characteristics`]'s own dirty-
    /// flag guard), but since the recursive call always passes `propagate = false`, the walk is only ever
    /// one level deep regardless - no risk of infinite recursion either way.
    pub fn set_time_last_serviced(&mut self, time: u32, propagate: bool) {
        self.time_last_serviced = time;
        if propagate {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) }.set_time_last_serviced(time, false);
            }
        }
    }

    /// Ports `ZTHabitat::triggerDeathArrived` (`ZTHabitat_triggerDeathArrived.c`): recalculates via the
    /// still-deferred real vanilla `recalculateCharacteristics` when `characteristics_dirty` is set (the
    /// same call-through shape every other dirty-gated getter here already uses), then walks the real
    /// vanilla `all_animals_begin`/`_end` vector, setting each matching animal's own `+0x39d` byte (real
    /// vanilla's own "death arrived" flag - name not otherwise confirmed, no reader identified in this
    /// pass's own scope) to `1`. `species_key == 0` matches every animal in the vector unconditionally;
    /// otherwise only animals whose own catalog-entry pointer (`+0x128`) passes
    /// [`entity_type_matches`]([`RVA_ANIMAL_TYPE_CHECK`]) and whose species catalog id (`+0x1ec`, same
    /// offset [`ZTHabitatMgr::distinct_species_catalog_ids`] already uses) equals `species_key`.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - `self`'s own address is passed straight into the
    /// `recalculateCharacteristics` call-through above.
    pub fn trigger_death_arrived(&self, species_key: i32) {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr == 0 {
                continue;
            }
            if species_key == 0 || Self::animal_species_matches(animal_ptr, species_key) {
                save_to_memory(animal_ptr + 0x39d, 1u8);
            }
        }
    }

    fn animal_species_matches(animal_ptr: u32, species_key: i32) -> bool {
        let type_matches = unsafe { entity_type_matches(animal_ptr, RVA_ANIMAL_TYPE_CHECK) };
        type_matches && get_from_memory::<i32>(get_from_memory::<u32>(animal_ptr + 0x128) + 0x1ec) == species_key
    }

    /// Checks whether `keeper_ptr`'s own assigned-species id list (`keeper_ptr+0x270`/`+0x274`, a real
    /// vanilla `std::vector<u32>` of catalog ids not otherwise modeled in this codebase) contains
    /// `animal_ptr`'s own numeric id (`+0x124`, `BFEntity::id` - see `ztworldmgr.rs`) - the shared "is this
    /// keeper actually assigned to service this specific animal" membership check
    /// `ZTHabitat_getNearestSickAnimal.c`/`_getNumSicklyAnimals.c` both inline identically.
    fn keeper_assigned_to_animal(keeper_ptr: u32, animal_ptr: u32) -> bool {
        let animal_id: u32 = get_from_memory(animal_ptr + 0x124);
        let begin: u32 = get_from_memory(keeper_ptr + 0x270);
        let end: u32 = get_from_memory(keeper_ptr + 0x274);
        (begin..end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == animal_id)
    }

    /// Ports `ZTHabitat::blockService` (`ZTHabitat_blockService.c`/`.asm`) - decides whether `keeper_ptr`
    /// (real vanilla only ever calls this with a genuine `ZTKeeper*`, hence the leading
    /// [`RVA_KEEPER_TYPE_CHECK_ARG`] guard) should currently be blocked from servicing this habitat.
    /// Returns real vanilla's own 3-state result: `0` once some assignable match this keeper is
    /// compatible with was found (or the trailing show-tank special case matches) - not blocked; `1` if
    /// this habitat owns fewer than 2 tiles, an early-out real vanilla takes before doing anything else;
    /// `2` the default fallback once every other check is exhausted.
    ///
    /// `check_tank_and_neighbors`/`skip_tank_depth_check` are real vanilla's own 3rd/2nd parameters -
    /// names inferred from their own usage (`check_tank_and_neighbors` gates the habitat/keeper-capability
    /// equivalence check below, the tank-depth check, and the amphibious-neighbor walk all at once;
    /// `skip_tank_depth_check` only matters when `check_tank_and_neighbors` is set, and additionally skips
    /// just the tank-depth check within it) - not confirmed against any real caller.
    ///
    /// Two vtable slots are called via raw pointer indirection with a documented-but-unconfirmed
    /// interpretation - see `zthabitatmgr-implementation-plan.md`'s step 6l write-up for the full
    /// identification trail:
    /// - `keeper_ptr`'s own vtable `+0x16c` (inherited unchanged from `BFUnit`, real name unconfirmed -
    ///   the one decompile referencing this address, `ZTGuest::tileFilter`, is a misattribution) - the
    ///   equivalence check below (`is_tank() == this predicate`) reads as "block on a habitat-type/
    ///   staff-capability mismatch", consistent with convergent evidence from several `ZTAnimal`/`BFUnit`
    ///   callers all gating water/movement-capability logic through the identical slot, but not confirmed
    ///   by name.
    /// - `BFEntityType`'s own vtable `+0x20` (`generated.rs::bfentitytype::NULLSUB_29`'s slot - see
    ///   `BFEntityType::isUserType`/`isUserTypeID`) - a per-subclass type/catalog-ID getter, confirmed via
    ///   `ZTKeeper_cleansUp.asm`'s own use of it (the returned ID is linear-searched against
    ///   `ZTKeeperType`'s own `std::vector<int>` at `+0x20c`/`+0x210`, right where `bfentitytype.rs`'s
    ///   already-modeled `ZTKeeperType` fields stop at `+0x208`). The trailing show-tank special case below
    ///   compares its raw return value against `0x2550`, a magic catalog-ID constant faithfully ported
    ///   without full identification (same treatment `ztshow.rs`'s own `STOP_0` already gives this exact
    ///   constant).
    ///
    /// A tank-specific field at `+0x1a8` (only read once [`Self::is_tank`] is confirmed, so always safely
    /// within `ZTTankExhibit`'s own `0x1e8`-byte allocation) is compared against
    /// `ZTKeeperType::clean_tank_threshold` (`+0x208`, already named in `bfentitytype.rs`) - real meaning
    /// of `+0x1a8` itself unconfirmed, a depth/size metric by inference only.
    pub fn block_service(&self, keeper_ptr: u32, skip_tank_depth_check: bool, check_tank_and_neighbors: bool) -> u32 {
        if keeper_ptr == 0 || !unsafe { entity_type_matches(keeper_ptr, RVA_KEEPER_TYPE_CHECK_ARG) } {
            return 0;
        }
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        if walk_tile_list(self.owned_tiles_ptr).count() < 2 {
            return 1;
        }

        if check_tank_and_neighbors {
            let keeper_predicate = unsafe { call_entity_vtable_noargs(keeper_ptr, 0x16c) };
            if self.is_tank() != keeper_predicate {
                return 2;
            }
        }

        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr != 0 && low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) {
                return 0;
            }
        }

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            if tile_ptr == 0 {
                continue;
            }
            let occupant_ptr: u32 = get_from_memory(tile_ptr + 0x10); // BFTile::entity_ptr
            if occupant_ptr != 0 && low_byte_bool(unsafe { CLEANS_UP.original()(keeper_ptr as *const u32, occupant_ptr as *const u32) }) {
                return 0;
            }
        }

        if check_tank_and_neighbors {
            if !skip_tank_depth_check && self.is_tank() {
                let keeper_entity_type: u32 = get_from_memory(keeper_ptr + 0x128);
                if keeper_entity_type != 0 {
                    let self_addr = self as *const Self as u32;
                    let tank_metric: i32 = get_from_memory(self_addr + 0x1a8);
                    let clean_tank_threshold: i32 = get_from_memory(keeper_entity_type + 0x208);
                    if tank_metric < clean_tank_threshold {
                        return 0;
                    }
                }
            }

            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                let neighbor = unsafe { ref_from_memory::<Self>(neighbor_ptr) };
                for addr in (neighbor.all_animals_begin..neighbor.all_animals_end).step_by(4) {
                    let animal_ptr: u32 = get_from_memory(addr);
                    if animal_ptr == 0 || !unsafe { entity_type_matches(animal_ptr, RVA_ANIMAL_TYPE_CHECK) } {
                        continue;
                    }
                    let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
                    if unsafe { call_entity_vtable_noargs(entity_type_ptr, 0xcc) }
                        && low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) })
                    {
                        return 0;
                    }
                }
            }
        }

        if self.is_tank() && self.is_show_tank() {
            let keeper_entity_type: u32 = get_from_memory(keeper_ptr + 0x128);
            if keeper_entity_type != 0 && unsafe { call_entity_vtable_u32_noargs(keeper_entity_type, 0x20) } == 0x2550 {
                return 0;
            }
        }

        2
    }

    /// Ports `ZTHabitatMgr::highlightHabitat` (`ZTHabitatMgr_highlightHabitat.c`) - despite the
    /// `ZTHabitatMgr::` decompile namespace, the real function is a free `stdcall` helper taking a
    /// `ZTHabitat*` directly (no `this`), which is why this lives on `ZTHabitat` rather than
    /// `ZTHabitatMgr`. Walks the owned-tile list ([`walk_tile_list`]) setting bit `0x80` at each tile's
    /// own `+0x83` when `hilite` is set, or bit `0x10` at `+0x85` when it isn't - two different flags/
    /// bytes, not a single toggle, confirmed against the real decompile's own two distinct branches (see
    /// [`Self::unhighlight`] for the separate, differently-shaped "clear" function real vanilla exposes
    /// alongside this one).
    pub fn highlight(&self, hilite: bool) {
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            if hilite {
                let flags: u8 = get_from_memory(tile + 0x83);
                save_to_memory(tile + 0x83, flags | 0x80);
            } else {
                let flags: u8 = get_from_memory(tile + 0x85);
                save_to_memory(tile + 0x85, flags | 0x10);
            }
        }
    }

    /// Ports `ZTHabitatMgr::unhighlightHabitat` (`ZTHabitatMgr_unhighlightHabitat.c`) - see
    /// [`Self::highlight`]'s own doc comment for why this lives on `ZTHabitat`. Clears both `+0x83` bit
    /// `0x80` and `+0x85` bit `0x10` on every owned tile - a genuinely different, not merely inverse,
    /// operation from `highlight(false)` (which only ever touches `+0x85`), confirmed directly against
    /// the real decompile.
    pub fn unhighlight(&self) {
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            let flags_83: u8 = get_from_memory(tile + 0x83);
            save_to_memory(tile + 0x83, flags_83 & 0x7f);
            let flags_85: u8 = get_from_memory(tile + 0x85);
            save_to_memory(tile + 0x85, flags_85 & 0xef);
        }
    }

    /// Ports `ZTHabitat::getShowInfoID` (`ZTHabitat_getShowInfoID.c`): the real body's upper 16 bits are
    /// leftover from the `ZTShowInfo` pointer's own high half (a partial-register decompiler artifact,
    /// not real dataflow - every real caller (`_setShowTimes.c` et al.) immediately truncates the result
    /// to `ushort` before use), so only the low 16 bits - `ZTShowInfo`'s own `field_0x70` id, per
    /// `ztshowinfo.rs`'s `set_show_info_id` - are reproduced here, zero-extended.
    pub fn get_show_info_id(&self) -> u16 {
        if self.zt_show_info_ptr == 0 {
            return 0;
        }
        get_from_memory::<u16>(self.zt_show_info_ptr + 0x70)
    }

    /// Ports `ZTHabitat::isShowStopped` (`ZTHabitat_isShowStopped.c`): `false` with no `ZTShowInfo`,
    /// otherwise delegates to the already-ported `ztshowinfo::is_stopped`.
    pub fn is_show_stopped(&self) -> bool {
        self.zt_show_info_ptr != 0 && ztshowinfo::is_stopped(self.zt_show_info_ptr)
    }

    /// Ports `ZTHabitat::getPopularity` (`ZTHabitat_getPopularity.c`/`.asm`, confirmed against the
    /// macOS decompile's same shape): `views / ticks_since_creation * popularity_scale_factor`, clamped
    /// to `[0, 100]` and truncated toward zero - `views` is `unknown_nt_time` (a running popularity-view
    /// accumulator, not really a timestamp despite its current field type; see
    /// `command_get_zt_habitats`'s own prior speculative use of this same field), `ticks_since_creation`
    /// is `ZTGameMgr::timeAgo(created_timestamp)`. The two float constants the real body clamps against
    /// (`DAT_00630d64`/`DAT_00630d5c`) aren't directly readable from the decompile, but the `.asm` itself
    /// pins both: `local_10[1] = 100.0` and `_DAT_00630d64` are the same underlying constant (one
    /// literal, one memory reference to it), and `local_8.dwLowDateTime = 0` reinterprets the same 4
    /// zeroed bytes as `0.0f` for the other clamp.
    ///
    /// **Truncated, not rounded**: the decompile's own `ROUND(*pFVar4)` pseudocode is misleading - the
    /// real `.asm` (`FLDCW` with `AH |= 0xc` before `FISTP`) temporarily forces the FPU's rounding-control
    /// bits to round-toward-zero before converting to int, i.e. a plain `(int)` truncation, not
    /// round-to-nearest. A prior version of this port used `.round()` here and failed
    /// `ZTHABITAT_GET_POPULARITY_LIVE` against a real, loaded zoo (`real=15, reimpl=16` - `.round()`
    /// rounding a `15.9x` result up while real vanilla truncates it down) until corrected to a plain `as
    /// i32` cast, which truncates the same way.
    ///
    /// The division and multiplication are done in `f64`, not `f32`: the real body computes both in one
    /// x87 FPU expression (`FDIVR` then `FMUL` over 80-bit extended-precision registers) with a single
    /// store to `float` only afterward - closer to `f64` intermediate precision than doing the division
    /// and multiplication as two independently-rounded `f32` operations.
    pub fn get_popularity(&self) -> i32 {
        let views = self.unknown_nt_time.to_raw() as f64;
        let ticks_since_creation = globals().ztgamemgr().time_ago(self.created_timestamp.to_raw());
        let scale = globals().zthabitatmgr().popularity_scale_factor;
        let raw = (views / ticks_since_creation as f64) * scale as f64;
        raw.clamp(0.0, 100.0) as i32
    }

    pub fn is_tank(&self) -> bool {
        self.vtable == Self::TANK_VTABLE_PTR
    }

    pub fn is_show_tank(&self) -> bool {
        self.zt_show_info_ptr != 0
    }

    /// Ports `ZTHabitatMgr::doTankCheck` (`ZTHabitatMgr_doTankCheck.c`/`.asm`) - despite its
    /// `ZTHabitatMgr::` decompile namespace it's a real free `stdcall` helper taking only a bare
    /// `ZTHabitat*` and touching no manager state at all (confirmed directly against `generated.rs`'s
    /// own `DO_TANK_CHECK` signature, no `this`), the same misattribution pattern this file already
    /// documents for `highlightHabitat`/`replaceGate`/`clearStaffHabitat` - ported as a `ZTHabitat`
    /// method instead.
    ///
    /// For every entry in this habitat's own [`Self::boundary_tile_pairs_begin`]/`_end` (a fresh
    /// snapshot via [`ZTHabitatMgr::snapshot_boundary_tile_pairs`], matching real vanilla's own
    /// copy-before-iterate shape), resolves the fence connecting the pair in each direction
    /// ([`Self::tile_fence_in_direction`] + [`BFMAP_GET_DIRECTION_0`], only kept when a genuine
    /// fence-family member - [`RVA_FENCE_TYPE_CHECK_ARG`]), and requires **at least one side** to be a
    /// fence whose own `entity_type+0x193` byte is set ([`is_tank_wall`], preferring the "A to B" side
    /// when it qualifies, matching the decompile's own `if (fence_a != 0) ... else ...` branch order).
    /// The moment any pair fails that, this returns `false` immediately (matching real vanilla's own
    /// early-`break`); an empty pairs vector (e.g. a freshly-constructed single-tile habitat before its
    /// first `resize`) returns `true` by default, matching real vanilla's own `local_11 = true`
    /// initializer that the loop never gets a chance to flip - see [`ZTHabitatMgr::create_habitat`]'s
    /// own call site, which relies on exactly this default for a brand-new seed-tile habitat.
    pub fn do_tank_check(&self) -> bool {
        let pairs = ZTHabitatMgr::snapshot_boundary_tile_pairs(self.boundary_tile_pairs_begin, self.boundary_tile_pairs_end);
        for (tile_a_ptr, tile_b_ptr) in pairs {
            let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
            let tile_b = get_from_memory::<BFTile>(tile_b_ptr);

            let dir_ab = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
            let fence_a = if dir_ab != -1 {
                let f = Self::tile_fence_in_direction(&tile_a, dir_ab as u32);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            let dir_ba = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_b_ptr as i32, tile_a_ptr as i32) };
            let fence_b = if dir_ba != -1 {
                let f = Self::tile_fence_in_direction(&tile_b, dir_ba as u32);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            let ok = if fence_a != 0 { is_tank_wall(fence_a) } else { fence_b != 0 && is_tank_wall(fence_b) };
            if !ok {
                return false;
            }
        }
        true
    }

    /// Ports `ZTHabitat::isRightSalinity` (vtable `+0x28`) base default. A plain `ZTHabitat` has no
    /// water/salinity concept at all (only `ZTTankExhibit` does - see that class's own override,
    /// `zttankexhibit::IS_RIGHT_SALINITY`, left un-ported); the base implementation is a constant `true`.
    pub fn is_right_salinity(&self, _animal_type: *const u32) -> bool {
        true
    }

    /// Ports `ZTHabitat::validatePositions` (vtable slot, `ZTHabitat_validatePositions.c`/`.asm`): walks
    /// the owned-tile list ([`TileListNode`]/[`walk_tile_list`]), calling real vanilla
    /// `BFTile::validatePositions(tile, tree, false)` for each tile, then does the same for the
    /// gate-out tile if any. `tree` is `GLOBAL_ZTWorldMgr + 0x8` - real vanilla's own guard on it
    /// (`&GLOBAL_ZTWorldMgr->field_0x8 != 0`) is dead in practice (would require
    /// `GLOBAL_ZTWorldMgr == -8`), so this checks `GLOBAL_ZTWorldMgr` itself instead: equivalent for
    /// every real value, and additionally guards the one case real vanilla's own check cannot.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn validate_positions(&self) {
        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 {
            return;
        }
        let tree = (world + 0x8) as *const u32;

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            unsafe { BFTILE_VALIDATE_POSITIONS.original()(tile as *const u32, tree, false) };
        }

        if let Some(gate_tile) = self.get_gate_tile_out() {
            let gate_tile_ptr = globals().ztworldmgr().get_ptr_from_bftile(&gate_tile);
            unsafe { BFTILE_VALIDATE_POSITIONS.original()(gate_tile_ptr as *const u32, tree, false) };
        }
    }

    /// Ports `ZTHabitat::removeHabitatTiles` (vtable slot, `ZTHabitat_removeHabitatTiles.c`/`.asm`): for
    /// every owned tile whose ownership-grid cell still points back at `self`, clears that cell and sets
    /// the tile's own `+0x85` bit `0x1` flag; then splices every list node onto the shared small-object
    /// freelist ([`TILE_LIST_NODE_FREELIST_HEAD_RVA`]) and resets the sentinel back to its empty
    /// (self-referencing) state - the exact same two-pass shape as real vanilla's own body. Frees only
    /// the `0x10`-byte [`TileListNode`]s themselves, never the `BFTile`s they point at (those are
    /// permanent map tiles owned by `GLOBAL_ZTWorldMgr`, not by this list) - so there is no cross-
    /// allocator hazard here (see `CLAUDE.md`'s own warning on this): every address this function frees
    /// was itself carved from this exact freelist bucket by real vanilla's own `cls_0x40143b`/bump
    /// allocator, and nothing here is ever `Box`-allocated.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - reads/writes the real ownership grid and mutates real vanilla's
    /// shared node pool in place.
    pub fn remove_habitat_tiles(&mut self) {
        let habitat_mgr = globals().zthabitatmgr();
        let self_addr = self as *const Self as u32;
        let sentinel = self.owned_tiles_ptr;

        for node in walk_tile_list(sentinel) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            if tile == 0 {
                continue;
            }
            let x: i32 = get_from_memory(tile + 0x34);
            let y: i32 = get_from_memory(tile + 0x38);
            if let Some(cell_addr) = habitat_mgr.get_habitat_cell_addr(x, y) {
                let owner: u32 = get_from_memory(cell_addr);
                if owner == self_addr {
                    save_to_memory(cell_addr, 0u32);
                    let flags: u8 = get_from_memory(tile + 0x85);
                    save_to_memory(tile + 0x85, flags | 1);
                }
            }
        }

        let freelist_head_addr = get_module_base("zoo.exe") as u32 + TILE_LIST_NODE_FREELIST_HEAD_RVA;
        for node in walk_tile_list(sentinel) {
            let old_head: u32 = get_from_memory(freelist_head_addr);
            save_to_memory(node, old_head);
            save_to_memory(freelist_head_addr, node);
        }

        save_to_memory(sentinel, sentinel);
        save_to_memory(sentinel + 4, sentinel);
    }

    /// Ports `ZTHabitat::resetUnitAI` (vtable slot, `ZTHabitat_resetUnitAI.c`/`.asm`): for every owned
    /// tile, walks that tile's own occupant list (`BFTile::unit_list_ptr`, `+0x0` - the exact same
    /// [`TileListNode`] shape/pool as `owned_tiles_ptr`, one level more nested, confirmed directly
    /// against `.asm` not just the decompile - see that field's own doc comment), calling vtable slot
    /// `+0x100` on each occupant; then the same for the gate-out tile's own occupants, if any.
    ///
    /// Gated on the same live `GLOBAL_ZTApp` singleton [`GLOBAL_ZTAPP_RVA`] resolves, one byte further
    /// into the object than `ZTGameMgr::stop`'s own `appInitSuccess` read (`+0x441` rather than `+0x440`)
    /// - a distinct flag, semantics otherwise unconfirmed beyond gating this entire method when set. A
    /// null `GLOBAL_ZTApp` is treated as "not ready" and skips the whole body, same reasoning as
    /// `ZTGameMgr::stop`'s own doc comment gives for not reproducing the real "lazily assign a bogus
    /// sentinel" branch.
    ///
    /// The real body's `GLOBAL_ZTWorldMgr != -8` guard is, like [`Self::validate_positions`]'s own
    /// identical guard, dead in practice - checked here as `GLOBAL_ZTWorldMgr != 0` instead (equivalent
    /// for every real value, and additionally guards the one case the real check cannot).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn reset_unit_ai(&self) {
        let base = get_module_base("zoo.exe") as u32;
        let ztapp_ptr: u32 = get_from_memory(base + GLOBAL_ZTAPP_RVA);
        if ztapp_ptr == 0 {
            return;
        }
        let gate_flag: u8 = get_from_memory(ztapp_ptr + 0x441);
        if gate_flag != 0 {
            return;
        }

        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 {
            return;
        }

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            if tile != 0 {
                reset_unit_ai_for_tile_occupants(tile);
            }
        }

        if let Some(gate_tile) = self.get_gate_tile_out() {
            let gate_tile_ptr = globals().ztworldmgr().get_ptr_from_bftile(&gate_tile);
            reset_unit_ai_for_tile_occupants(gate_tile_ptr);
        }
    }

    /// Ports `ZTHabitat::addHabitatTiles` (vtable slot) plus its two real workers,
    /// `addSeedsOnStack`/`addContiguousSpan` (`ZTHabitat_addSeedsOnStack.c`/`_addContiguousSpan.c`/
    /// `.asm`): a scanline flood-fill from `seed_tile_ptr` that claims ([`claim_tile`]) every tile
    /// reachable without crossing a wall ([`is_wall`]) - [`add_contiguous_span`] extends a horizontal
    /// (West/East) run from the seed, then for every tile in that run this checks the North/South
    /// neighbours ([`enqueue_if_claimable`]) and queues any newly-reachable tile for the same treatment,
    /// repeating until nothing's left to visit.
    ///
    /// The real function's third parameter is nominally "the other habitat", but the only real call site
    /// (`ZTHabitat::resize`: `(*this->vftptr_0x0->addHabitatTiles)(this, seed_tile, this)`) always passes
    /// `this` again - so this port only takes the seed tile and always targets `self`, rather than
    /// reproducing a parameter that's never actually different from `self` in practice. The detour below
    /// still matches the real 3-argument vtable signature and simply ignores the third.
    ///
    /// Uses a plain `Vec<u32>` as the flood-fill's scratch worklist rather than touching real vanilla's
    /// own scratch `std::deque` global (`DAT_0063b710` family) - that deque is purely transient within
    /// this call and visit order doesn't affect the final claimed set, so there's nothing to keep in sync
    /// with vanilla's own copy.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - mutates the real ownership grid and vanilla's own shared node pool
    /// in place (via [`insert_tile_list_node`]'s call-through to real vanilla's own list-insert, matching
    /// [`Self::remove_habitat_tiles`]'s own allocator-consistency reasoning).
    pub fn add_habitat_tiles(&mut self, seed_tile_ptr: u32) {
        if seed_tile_ptr == 0 {
            return;
        }
        let target = self as *const Self as u32;
        let habitat_mgr = globals().zthabitatmgr();
        let world = globals().ztworldmgr();
        let sentinel = self.owned_tiles_ptr;

        let mut worklist: Vec<u32> = vec![seed_tile_ptr];
        while let Some(tile_ptr) = worklist.pop() {
            let x: i32 = get_from_memory(tile_ptr + 0x34);
            let y: i32 = get_from_memory(tile_ptr + 0x38);
            if habitat_mgr.get_habitat_ptr(x, y) == target {
                continue;
            }

            let (low, high) = add_contiguous_span(world, habitat_mgr, sentinel, target, tile_ptr);

            let mut cursor = low;
            loop {
                enqueue_if_claimable(world, habitat_mgr, target, cursor, Direction::North, &mut worklist);
                enqueue_if_claimable(world, habitat_mgr, target, cursor, Direction::South, &mut worklist);
                if cursor == high {
                    break;
                }
                let next = get_neighbour_ptr(world, cursor, Direction::East);
                if next == 0 {
                    break;
                }
                cursor = next;
            }
        }
    }

    /// Ports `ZTHabitat::listen` (vtable `+0x10`, `ZTHabitat_listen.c`): drains the event list built by
    /// `getEvents` (vtable `+0x8`, still real vanilla - not yet ported) and releases its buffer back to
    /// wherever real vanilla's own allocator would - see [`free_event_vector_buffer`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn listen(&self) {
        let mut events = VanillaEventVector::rvo_target();
        unsafe { GET_EVENTS.original()(self as *const Self as *const u32, events.as_ptr()) };
        if events.begin != 0 {
            free_event_vector_buffer(events.begin, events.cap_end - events.begin);
        }
    }

    /// Ports `ZTHabitat::setIsShowExhibit` (vtable `+0x2c`, `ZTHabitat_setIsShowExhibit.c`): allocates
    /// and constructs a real `ZTShowInfo` (`ztshowinfo::CONSTRUCTOR_1`), registers it with
    /// `GLOBAL_ZTShowMgr`. `REGISTER_SHOW`/`UNREGISTER_SHOW` are already detoured by `ztshowmgr.rs`
    /// (Stage 9), so this calls `.hooked()` rather than `.original()` - the correct call-through for an
    /// external caller of an address it doesn't itself own (see `ztshow.rs`'s own `GET_NUM_UNITS.hooked()`
    /// precedent). On registration failure, tears the just-built `ZTShowInfo` back down via its own
    /// scalar-deleting-destructor slot and leaves `zt_show_info_ptr` null, matching vanilla exactly.
    ///
    /// Also constructs the two configured ambient sounds (`[sounds] startSound`/`endSound`) and acquires
    /// each through `GLOBAL_DX8SndMgr` - see [`construct_and_acquire_sound`]'s doc comment for why both
    /// are treated as `SNDSound`. Gated on [`looks_like_configured_sound_name`] rather than real
    /// vanilla's own bare `DAT_... != 0` check - see that function's own doc comment for why.
    ///
    /// Must only be called on a live `ZTHabitat` reference (own address passed to real vanilla calls),
    /// same precondition as [`Self::get_attractiveness`].
    pub fn set_is_show_exhibit(&mut self) {
        if self.zt_show_info_ptr != 0 {
            return;
        }

        let alloc = unsafe { OPERATOR_NEW.original()(0xa8) } as u32;
        let show_info = if alloc == 0 { 0 } else { unsafe { ZTSHOWINFO_CONSTRUCTOR.original()(alloc as *const u32) as u32 } };
        self.zt_show_info_ptr = show_info;

        let zt_show_mgr = globals().ztshowmgr_ptr() as *const u32;
        if zt_show_mgr.is_null() || show_info == 0 {
            return;
        }

        let registered = unsafe { REGISTER_SHOW.hooked()(zt_show_mgr, show_info as *const u32, true) };
        if !registered {
            unsafe { ZTSHOWINFO_DESTRUCTOR.original()(show_info as *const u32, 1) };
            self.zt_show_info_ptr = 0;
            return;
        }
        save_to_memory(show_info + 0xa0, self as *const Self as u32);

        let base = get_module_base("zoo.exe") as u32;
        let dx8_sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        let start_sound_name = base + START_SOUND_NAME_RVA;
        let end_sound_name = base + END_SOUND_NAME_RVA;

        if looks_like_configured_sound_name(start_sound_name) {
            self.start_sound_ptr = construct_and_acquire_sound(dx8_sndmgr, start_sound_name);
        }
        if looks_like_configured_sound_name(end_sound_name) {
            self.end_sound_ptr = construct_and_acquire_sound(dx8_sndmgr, end_sound_name);
        }
    }

    /// Ports `ZTHabitat::setIsNotShowExhibit` (vtable `+0x30`, `ZTHabitat_setIsNotShowExhibit.c`):
    /// unregisters and tears down the real `ZTShowInfo` `set_is_show_exhibit` built, clearing the
    /// showpanel UI's own selection first if it currently points at this habitat, then tears down both
    /// owned sounds via [`teardown_sound`].
    ///
    /// The `unregisterShow` call carries no `GLOBAL_ZTShowMgr` null check in the real decompile either -
    /// mirrored exactly rather than adding a guard vanilla itself doesn't have (same reasoning
    /// `ztshow.rs`'s own `RESOLVE_NEXT_SCHEDULED_SCRIPT_ID` doc comment gives for an identical unchecked
    /// vanilla read).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn set_is_not_show_exhibit(&mut self) {
        if self.zt_show_info_ptr == 0 {
            return;
        }

        let base = get_module_base("zoo.exe") as u32;
        let currently_open_habitat: u32 = get_from_memory(base + SHOWPANEL_CURRENT_HABITAT_RVA);
        if currently_open_habitat == self as *const Self as u32 {
            unsafe { SET_EXHIBIT.original()(0) };
        }

        let show_id: u16 = get_from_memory(self.zt_show_info_ptr + 0x70);
        let zt_show_mgr = globals().ztshowmgr_ptr() as *const u32;
        unsafe { UNREGISTER_SHOW.hooked()(zt_show_mgr, show_id, std::ptr::null(), true) };

        unsafe { ZTSHOWINFO_DESTRUCTOR.original()(self.zt_show_info_ptr as *const u32, 1) };
        self.zt_show_info_ptr = 0;

        teardown_sound(self.start_sound_ptr);
        self.start_sound_ptr = 0;
        teardown_sound(self.end_sound_ptr);
        self.end_sound_ptr = 0;
    }

    /// Ports `ZTHabitat::update` (vtable `+0x24`, `ZTHabitat_update.c`/`.asm`, confirmed against the
    /// macOS decompile's identical shape): plays every queued ambient sound
    /// (`ambients_begin`/`ambients_end`), advances the species-list/characteristics lazy-recalculate
    /// timers - rerolling each via the shared game RNG ([`lcg_next`]) and calling through to the
    /// still-un-ported real vanilla `reviseSpeciesList`/`recalculateCharacteristics` once its own
    /// threshold trips, the same dirty-flag/timer shape [`Self::get_attractiveness`] already relies on
    /// for `characteristics_dirty` - ticks every viewing area's own ambient state
    /// (`viewing_areas_begin`/`viewing_areas_end`), then calls through to the still-un-ported real
    /// vanilla `updatePortals` and finally this object's own, already-ported [`Self::listen`].
    ///
    /// `updatePortals` is a plain, non-virtual helper (not one of `ZTHabitat`'s 17 vtable slots) -
    /// deliberately left un-ported and called via `.original()`: its own body is an `isTank`-gated
    /// portal-list check this port doesn't need to understand to reproduce `update` itself.
    ///
    /// `ZTTankExhibit` overrides this vtable slot with its own, separate address (`0x0049625f`, out of
    /// scope for this pass per `zthabitatmgr-implementation-plan.md`) - detouring only the base
    /// `ZTHabitat::update` address never intercepts a real tank's own tick, the same base-only-override
    /// pattern [`Self::is_right_salinity`] already relies on.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - `self`'s own address is passed straight into every real vanilla
    /// call-through above.
    pub fn update(&mut self, elapsed: u32) {
        let mut ambient_entry = self.ambients_begin;
        while ambient_entry != self.ambients_end {
            let ambient: u32 = get_from_memory(ambient_entry + 4);
            unsafe { AMBIENTS_PLAY.original()(ambient as *const u32, elapsed as i32, 0x50) };
            ambient_entry += 8;
        }

        let species_list_timer = self.species_list_timer.wrapping_add(elapsed);
        self.species_list_timer = species_list_timer;
        self.characteristics_timer = self.characteristics_timer.wrapping_add(elapsed);

        if species_list_timer > 7999 {
            self.species_list_dirty = 1;
        }
        if self.species_list_dirty != 0 {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            self.species_list_timer = (rng >> 0x10 & 0x7fff) % 200;
            unsafe { REVISE_SPECIES_LIST.original()(self as *const Self as *const u32) };
        }

        if self.characteristics_timer > 6999 {
            self.characteristics_dirty = 1;
        }
        if self.characteristics_dirty != 0 {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            self.characteristics_timer = (rng >> 0x10 & 0x7fff) % 200;
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }

        let mut viewing_area_entry = self.viewing_areas_begin;
        while viewing_area_entry != self.viewing_areas_end {
            let viewing_area: u32 = get_from_memory(viewing_area_entry);
            unsafe { ZTVIEWINGAREA_UPDATE_AMBIENTS.original()(viewing_area as *const u32, elapsed as i32) };
            viewing_area_entry += 4;
        }

        unsafe { UPDATE_PORTALS.original()(self as *const Self as *const u32) };
        self.listen();
    }

    /// Ports `ZTHabitat::save` (vtable `+0x1c`, `ZTHabitat_save.c`/`.asm`, confirmed identical on both
    /// platforms modulo the macOS decompile's own endian-swap noise): the "seed tile" position (the
    /// first owned tile - `owned_tiles_ptr`'s sentinel own `next`, then that node's `payload`, read
    /// exactly as vanilla's own double-pointer-chase does, including its lack of a guard for an empty
    /// tile list - "dead in practice" the same way `ZTHabitatMgr::save`'s own unguarded entrance-tile
    /// read is, since a real habitat is never saved without at least one owned tile), `exhibit_name`'s
    /// length-then-bytes (skipped entirely, not just zero-length, if the name is implausibly long -
    /// `>= 0x1000` bytes - matching vanilla's own dead branch rather than adding a guard it lacks), the
    /// entrance tile's position (or `(-1, -1)` with no entrance), `entrance_rotation`, the six
    /// donation/upkeep running totals, three unknown dwords, `created_timestamp`/`unknown_nt_time` (8
    /// bytes each), `time_last_serviced`, then `is_tank()`/`is_show_tank()` as trailing bytes - delegating to
    /// [`Self::is_tank`] rather than the real vtable `+0x20` slot it actually calls, per this file's
    /// existing precedent (see that method's own doc comment). Finally, if `is_show_tank()`, calls
    /// through to real vanilla `ZTShowInfo::save` (not yet ported).
    ///
    /// Every field's success is ANDed together and every write happens regardless of an earlier one
    /// failing - matches the real body's own flat structure (no early-exit branches besides the
    /// oversized-name case), unlike [`ZTHabitatMgr::save`]'s own per-exhibit loop.
    pub fn save(&self, file: *const i8) -> bool {
        let head_node: u32 = get_from_memory(self.owned_tiles_ptr);
        let seed_tile_ptr: u32 = get_from_memory(head_node + 8);
        let seed_x: i32 = get_from_memory(seed_tile_ptr + 0x34);
        let seed_y: i32 = get_from_memory(seed_tile_ptr + 0x38);
        let mut ok = write_bytes_to_file(&seed_x, file);
        ok &= write_bytes_to_file(&seed_y, file);

        let (name_start, name_end, _) = self.exhibit_name.raw_parts();
        let name_len = name_end.wrapping_sub(name_start);
        if name_len < 0x1000 {
            ok &= write_bytes_to_file(&name_len, file);
            if name_len != 0 {
                ok &= write_raw_bytes(name_start, name_len, file);
            }
        } else {
            ok = false;
        }

        let (entrance_x, entrance_y): (i32, i32) = if self.entrance_tile_ptr == 0 {
            (-1, -1)
        } else {
            (get_from_memory(self.entrance_tile_ptr + 0x34), get_from_memory(self.entrance_tile_ptr + 0x38))
        };
        ok &= write_bytes_to_file(&entrance_x, file);
        ok &= write_bytes_to_file(&entrance_y, file);
        ok &= write_bytes_to_file(&self.entrance_rotation, file);
        ok &= write_bytes_to_file(&self.current_donations, file);
        ok &= write_bytes_to_file(&self.last_donations, file);
        ok &= write_bytes_to_file(&self.total_donations, file);
        ok &= write_bytes_to_file(&self.current_upkeep, file);
        ok &= write_bytes_to_file(&self.last_upkeep, file);
        ok &= write_bytes_to_file(&self.total_upkeep, file);
        ok &= write_bytes_to_file(&self.unknown_u32_2, file);
        ok &= write_bytes_to_file(&self.unknown_u32_3, file);
        ok &= write_bytes_to_file(&self.unknown_u32_4, file);
        ok &= write_bytes_to_file(&self.created_timestamp, file);
        ok &= write_bytes_to_file(&self.unknown_nt_time, file);
        ok &= write_bytes_to_file(&self.time_last_serviced, file);

        ok &= write_bytes_to_file(&(self.is_tank() as u8), file);
        let is_show_tank = self.is_show_tank();
        ok &= write_bytes_to_file(&(is_show_tank as u8), file);
        if is_show_tank {
            ok &= unsafe { ZTSHOWINFO_SAVE.original()(self.zt_show_info_ptr as *const u32, file) };
        }

        ok
    }

    /// Ports `ZTHabitat::hiliteAmphibiousNeighbors` (`ZTHabitat_hiliteAmphibiousNeighbors.c`): walks
    /// [`Self::amphibious_neighbors_head`] via [`walk_neighbor_tree`] and calls the already-ported
    /// [`Self::highlight`]/[`Self::unhighlight`] on each neighbor - `unhighlight()` when `hilite` is
    /// `false`, `highlight(true)` otherwise, matching the real decompile's own two branches exactly (not
    /// a symmetric `highlight(hilite)`/`highlight(!hilite)` pair - see those methods' own doc comments for
    /// why they're genuinely different operations, not each other's inverse). Purely a read of the tree
    /// real vanilla's own `addAmphibiousNeighbor`/`clearAmphibiousNeighbors` (left un-ported) produced -
    /// no allocation, no mutation of the tree itself.
    pub fn hilite_amphibious_neighbors(&self, hilite: bool) {
        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
            if hilite {
                neighbor.highlight(true);
            } else {
                neighbor.unhighlight();
            }
        }
    }

    /// Ports `ZTHabitat::hiliteShowNeighbors` (`ZTHabitat_hiliteShowNeighbors.c`) - identical shape to
    /// [`Self::hilite_amphibious_neighbors`], walking [`Self::show_neighbors_head`] instead.
    pub fn hilite_show_neighbors(&self, hilite: bool) {
        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
            if hilite {
                neighbor.highlight(true);
            } else {
                neighbor.unhighlight();
            }
        }
    }

    /// Push-back helper for [`Self::create_edge_pairs`]'s own [`Self::boundary_tile_pairs_begin`]/`_end`/
    /// `_cap_end` vector - real vanilla's own `PoolAlloc::allocate`/`PoolAlloc::deallocate` doubling growth
    /// (`ZTHabitat_createEdgePairs.c`), called through rather than reimplemented, matching
    /// [`ZTHabitatMgr::add_habitat`]'s own established precedent for the identical allocator pair. The
    /// decompile's own general "insert in the middle" shift helper (`FUN_00411192`) never actually executes
    /// for this call site (insertion is always at the vector's own current end - the loop that would call it
    /// is unconditionally empty here), so it's sidestepped entirely rather than needing to be identified.
    fn push_boundary_tile_pair(&self, tile_a: u32, tile_b: u32) {
        let self_addr = self as *const Self as u32;
        let begin = self.boundary_tile_pairs_begin;
        let end = self.boundary_tile_pairs_end;
        let cap_end = self.boundary_tile_pairs_cap_end;

        if end == cap_end {
            let old_len = (end - begin) / 8;
            let new_cap = if old_len == 0 { 1 } else { old_len * 2 };
            let new_buf = unsafe { POOLALLOC_ALLOCATE.original()(new_cap * 8) } as u32;

            for i in 0..old_len {
                let a: u32 = get_from_memory(begin + i * 8);
                let b: u32 = get_from_memory(begin + i * 8 + 4);
                if new_buf != 0 {
                    save_to_memory(new_buf + i * 8, a);
                    save_to_memory(new_buf + i * 8 + 4, b);
                }
            }
            if new_buf != 0 {
                save_to_memory(new_buf + old_len * 8, tile_a);
                save_to_memory(new_buf + old_len * 8 + 4, tile_b);
            }
            // Real vanilla calls `PoolAlloc::deallocate` unconditionally here, even for `begin==0` (a
            // null-pointer, zero-length free) - matched as-is, same reasoning as `ZTHabitatMgr::add_habitat`'s
            // own identical unconditional-deallocate precedent.
            unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, cap_end - begin) };

            save_to_memory(self_addr + 0x48, new_buf);
            save_to_memory(self_addr + 0x4c, new_buf + (old_len + 1) * 8);
            save_to_memory(self_addr + 0x50, new_buf + new_cap * 8);
        } else {
            save_to_memory(end, tile_a);
            save_to_memory(end + 4, tile_b);
            save_to_memory(self_addr + 0x4c, end + 8);
        }
    }

    /// Ports `ZTHabitat::createEdgePairs` (`ZTHabitat_createEdgePairs.c`/`.asm`, `generated.rs`'s
    /// `CREATE_EDGE_PAIRS`): rebuilds [`Self::boundary_tile_pairs_begin`]/`_end` from scratch (real
    /// vanilla's own `end = begin` reset, keeping the existing buffer rather than deallocating it), then
    /// walks the owned-tile list ([`walk_tile_list`]) and, for every owned tile and each of its 4 cardinal
    /// neighbours ([`get_neighbour_ptr`]), pushes `(owned_tile, neighbour_tile)` ([`Self::push_boundary_tile_pair`])
    /// whenever the neighbour isn't owned by `self` - including when it doesn't exist (real vanilla's own
    /// address, `0` when out of map bounds).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    ///
    /// Verified correct against real vanilla via a direct call
    /// (`ZTHABITAT_CREATE_EDGE_PAIRS_MATCHES_REAL_LIVE`), but its own `#[detour]` is deliberately **not**
    /// installed - see `hooks_zthabitatmgr::create_edge_pairs`'s own doc comment for the live-battery hang
    /// this caused when hooked, bisected directly against the other 8 stage-6h detours.
    pub fn create_edge_pairs(&mut self) {
        let self_addr = self as *const Self as u32;
        save_to_memory(self_addr + 0x4c, self.boundary_tile_pairs_begin);

        let world = globals().ztworldmgr();
        let habitat_mgr = globals().zthabitatmgr();
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            for direction in [Direction::North, Direction::East, Direction::South, Direction::West] {
                let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction);
                let owner_ptr = if neighbour_ptr == 0 {
                    0
                } else {
                    let neighbour = get_from_memory::<BFTile>(neighbour_ptr);
                    habitat_mgr.get_habitat_ptr(neighbour.pos.x, neighbour.pos.y)
                };
                if owner_ptr != self_addr {
                    self.push_boundary_tile_pair(tile_ptr, neighbour_ptr);
                }
            }
        }
    }

    /// Ports `ZTHabitat::recalculateViewingAreas` (`ZTHabitat_recalculateViewingAreas.c`, `generated.rs`'s
    /// `RECALCULATE_VIEWING_AREAS`): calls real vanilla `ZTViewingArea::recalculateCharacteristics`
    /// (call-through - `ZTViewingArea` itself isn't reimplemented anywhere in this codebase) on every
    /// [`Self::viewing_areas_begin`]/`_end` entry.
    pub fn recalculate_viewing_areas(&self) {
        let mut cursor = self.viewing_areas_begin;
        while cursor != self.viewing_areas_end {
            let va_ptr: u32 = get_from_memory(cursor);
            unsafe { ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS.original()(va_ptr as *const u32) };
            cursor += 4;
        }
    }

    /// Ports `ZTHabitat::addViewingArea` (`ZTHabitat_addViewingArea.c`/`.asm`, `generated.rs`'s
    /// `ADD_VIEWING_AREA`): appends `va_ptr` to [`Self::viewing_areas_begin`]/`_end`, doubling the backing
    /// buffer (minimum `1`) through real vanilla's own `PoolAlloc::allocate` when full, freeing the old
    /// buffer via [`free_event_vector_buffer`] (the same small-object freelist-bucket/`operator_delete`
    /// split `ZTHabitat::listen`'s own event-vector teardown already uses - confirmed identical via this
    /// decompile's own `(&DAT_00638000)[...]`/`operator_delete` split). Always sets
    /// [`Self::characteristics_dirty`] afterward, matching real vanilla's own unconditional
    /// `this->field_0x2d = 1` on both the grow and no-grow paths.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn add_viewing_area(&mut self, va_ptr: u32) {
        let self_addr = self as *const Self as u32;
        let begin = self.viewing_areas_begin;
        let end = self.viewing_areas_end;
        let cap_end = self.viewing_areas_cap_end;

        if end == cap_end {
            let old_len = (end - begin) / 4;
            let new_cap = if old_len == 0 { 1 } else { old_len * 2 };
            let new_buf = unsafe { POOLALLOC_ALLOCATE.original()(new_cap * 4) } as u32;

            for i in 0..old_len {
                let value: u32 = get_from_memory(begin + i * 4);
                if new_buf != 0 {
                    save_to_memory(new_buf + i * 4, value);
                }
            }
            if new_buf != 0 {
                save_to_memory(new_buf + old_len * 4, va_ptr);
            }
            free_event_vector_buffer(begin, cap_end - begin);

            save_to_memory(self_addr + 0x34, new_buf);
            save_to_memory(self_addr + 0x38, new_buf + (old_len + 1) * 4);
            save_to_memory(self_addr + 0x3c, new_buf + new_cap * 4);
        } else {
            save_to_memory(end, va_ptr);
            save_to_memory(self_addr + 0x38, end + 4);
        }
        save_to_memory::<u8>(self_addr + 0x2d, 1);
    }

    /// Ports `ZTHabitat::removeViewingArea` (`ZTHabitat_removeViewingArea.c`/`.asm`, `generated.rs`'s
    /// `REMOVE_VIEWING_AREA`): finds `va_ptr` in [`Self::viewing_areas_begin`]/`_end` (no-op if absent),
    /// destroys and frees it (call-through to real vanilla `ZTViewingArea::~ZTViewingArea` +
    /// `operator_delete` - `ZTViewingArea` itself isn't reimplemented anywhere in this codebase), shifts
    /// every later entry down by one slot, shrinks the vector's own `_end` by one pointer, and sets
    /// [`Self::characteristics_dirty`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn remove_viewing_area(&mut self, va_ptr: u32) {
        let self_addr = self as *const Self as u32;
        let begin = self.viewing_areas_begin;
        let end = self.viewing_areas_end;

        let mut cursor = begin;
        while cursor != end && get_from_memory::<u32>(cursor) != va_ptr {
            cursor += 4;
        }
        if cursor == end {
            return;
        }

        if va_ptr != 0 {
            unsafe {
                ZTVIEWINGAREA_DESTRUCTOR.original()(va_ptr as *const u32);
                OPERATOR_DELETE.original()(va_ptr);
            }
        }

        let mut write = cursor;
        let mut read = cursor + 4;
        while read != end {
            let value: u32 = get_from_memory(read);
            save_to_memory(write, value);
            write += 4;
            read += 4;
        }
        save_to_memory(self_addr + 0x38, end - 4);
        save_to_memory::<u8>(self_addr + 0x2d, 1);
    }

    /// Ports `ZTHabitat::removeFromAllVAs` (`ZTHabitat_removeFromAllVAs.c`/`.asm`, `generated.rs`'s
    /// `REMOVE_FROM_ALL_VAS`): for every [`Self::viewing_areas_begin`]/`_end` entry whose own tile-vector
    /// (`+0x40`/`+0x44`) contains `tile_ptr`, removes it (real vanilla `ZTViewingArea::removeTile`, called
    /// through - `ZTViewingArea` itself isn't reimplemented anywhere in this codebase); if that empties the
    /// viewing area's own tile vector, removes the viewing area itself ([`Self::remove_viewing_area`]) and
    /// restarts the scan from the beginning (real vanilla's own `goto`-driven restart, since the vector was
    /// just mutated out from under the walk).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn remove_from_all_vas(&mut self, tile_ptr: u32) {
        'restart: loop {
            let mut cursor = self.viewing_areas_begin;
            let end = self.viewing_areas_end;
            while cursor != end {
                let va_ptr: u32 = get_from_memory(cursor);
                let vec_begin: u32 = get_from_memory(va_ptr + 0x40);
                let vec_end: u32 = get_from_memory(va_ptr + 0x44);
                let contains = (vec_begin..vec_end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == tile_ptr);
                if contains {
                    unsafe { ZTVIEWINGAREA_REMOVE_TILE.original()(va_ptr as *const u32, tile_ptr as i32) };
                    let new_begin: u32 = get_from_memory(va_ptr + 0x40);
                    let new_end: u32 = get_from_memory(va_ptr + 0x44);
                    if new_begin == new_end {
                        self.remove_viewing_area(va_ptr);
                        continue 'restart;
                    }
                }
                cursor += 4;
            }
            break;
        }
    }

    /// Ports `ZTHabitat::recreateOAs` (`ZTHabitat_recreateOAs.c`, `generated.rs`'s `RECREATE_OAS`): sets
    /// byte `+0x25` (a `ZTViewingArea`-level "needs recreate" flag - unrelated to [`Self::neighbor_dirty`]'s
    /// own `+0x25`, on a different struct, despite the coincidental shared offset) on every
    /// [`Self::viewing_areas_begin`]/`_end` entry.
    pub fn recreate_oas(&self) {
        let mut cursor = self.viewing_areas_begin;
        while cursor != self.viewing_areas_end {
            let va_ptr: u32 = get_from_memory(cursor);
            save_to_memory::<u8>(va_ptr + 0x25, 1);
            cursor += 4;
        }
    }

    /// Ports `ZTHabitat::pathPlaced` (`ZTHabitat_pathPlaced.c`/`.asm`, `generated.rs`'s `PATH_PLACED`):
    /// called when `tile_ptr` becomes a path tile (real vanilla `BFTile` `+0x83` bit `0x8`) - extends an
    /// existing, adjacent `self`-owned `ZTViewingArea` to cover it if one is short enough (span `< 4` tiles
    /// along its own axis), else creates a brand-new one.
    ///
    /// Early-returns if `tile_ptr` isn't actually in the zoo (real vanilla `BFTile::isInZoo`, called
    /// through). Otherwise gathers up to 4 candidate cardinal neighbours ([`get_neighbour_ptr`]) that are
    /// themselves path tiles and within an existing viewing area (`generated.rs`'s `standalone::TILE_WITHIN_AVA`,
    /// called through) - as a plain `Vec<(Direction, u32)>` local scratch (real vanilla backs the equivalent
    /// local list with its own small-object allocator; this is pure per-call scratch nothing else reads, so
    /// a plain Rust `Vec` carries no cross-allocator risk, same reasoning as [`ZTHabitatMgr::can_find_path`]'s
    /// own BFS frontier).
    ///
    /// For each candidate in turn, scans the neighbour tile's own cached-neighbour-habitat cell-row slots
    /// (the same 8 `+0x4..0x24` slots [`ZTHabitatMgr::habitat_tile_changed`] marks dirty) for a
    /// `ZTViewingArea` owned by `self`: if found, reads its current NS/EW extent (axis chosen by the
    /// candidate's own direction - North/South use `generated.rs`'s `ztviewingarea::GET_NSEXTENT`,
    /// East/West `GET_EWEXTENT`, both called through), clamps it to include the neighbour tile's own
    /// coordinate on that axis, and - if the resulting span would stay under 4 tiles - adds `tile_ptr` to
    /// that viewing area (`ztviewingarea::ADD_TILE`, called through) and returns immediately, matching real
    /// vanilla's own `goto`-past-the-new-VA-creation-code exactly.
    ///
    /// If every candidate's matching viewing area (if any) rejected the tile as too large, constructs a
    /// brand-new one (`operator_new(0x5c)` + real vanilla `ZTViewingArea::ZTViewingArea` constructor, both
    /// called through) and appends it via [`Self::add_viewing_area`] - but **not** when there were
    /// candidates and *none* of them ever found a matching viewing area at all (real vanilla's own
    /// `if (!bVar4) goto cleanup` skips the new-VA creation code entirely in that case, matched here as-is
    /// rather than "fixed" - a genuine, if surprising, real vanilla edge case, not a bug this port
    /// introduces). A new VA is still created, as usual, when there were no candidates at all.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn path_placed(&mut self, tile_ptr: u32) {
        if unsafe { BFTILE_IS_IN_ZOO.original()(tile_ptr as *const u32, 1) } == 0 {
            return;
        }

        let world = globals().ztworldmgr();
        let habitat_mgr = globals().zthabitatmgr();
        let self_addr = self as *const Self as u32;

        let mut candidates: Vec<(Direction, u32)> = Vec::new();
        for direction in [Direction::North, Direction::East, Direction::South, Direction::West] {
            let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction.clone());
            if neighbour_ptr == 0 {
                continue;
            }
            let flags: u8 = get_from_memory(neighbour_ptr + 0x83);
            if flags & 8 == 0 {
                continue;
            }
            if unsafe { TILE_WITHIN_AVA.original()(neighbour_ptr as i32, self_addr as i32) } == 0 {
                continue;
            }
            candidates.push((direction, neighbour_ptr));
        }

        if !candidates.is_empty() {
            let mut any_rejected = false;
            for (direction, neighbour_ptr) in &candidates {
                let neighbour = get_from_memory::<BFTile>(*neighbour_ptr);
                let Some(cell_addr) = habitat_mgr.get_habitat_cell_addr(neighbour.pos.x, neighbour.pos.y) else {
                    continue;
                };
                for slot in 0..8u32 {
                    let va_ptr: u32 = get_from_memory(cell_addr + 0x4 + slot * 4);
                    if va_ptr == 0 {
                        continue;
                    }
                    let owner: u32 = get_from_memory(va_ptr);
                    if owner != self_addr {
                        continue;
                    }
                    let is_ns = matches!(direction, Direction::North | Direction::South);
                    let mut low: i32 = 0;
                    let mut high: i32 = 0;
                    if is_ns {
                        unsafe { ZTVIEWINGAREA_GET_NSEXTENT.original()(va_ptr as *const u32, &mut low, &mut high) };
                    } else {
                        unsafe { ZTVIEWINGAREA_GET_EWEXTENT.original()(va_ptr as *const u32, &mut low, &mut high) };
                    }
                    let coord = if is_ns { neighbour.pos.y } else { neighbour.pos.x };
                    if coord < low {
                        low = coord;
                    }
                    if high < coord {
                        high = coord;
                    }
                    if (high - low) + 1 < 4 {
                        unsafe { ZTVIEWINGAREA_ADD_TILE.original()(va_ptr as *const u32, tile_ptr as *const std::ffi::c_void) };
                        return;
                    }
                    any_rejected = true;
                    break;
                }
            }
            if !any_rejected {
                return;
            }
        }

        let new_va = unsafe { OPERATOR_NEW.original()(0x5c) } as u32;
        if new_va != 0 {
            unsafe { ZTVIEWINGAREA_CONSTRUCTOR.original()(new_va as *const u32, self_addr as *const std::ffi::c_void, tile_ptr as *const std::ffi::c_void) };
            self.add_viewing_area(new_va);
        }
    }
}

impl PartialEq for ZTHabitat {
    fn eq(&self, other: &Self) -> bool {
        self.owned_tiles_ptr == other.owned_tiles_ptr
            && self.entrance_rotation == other.entrance_rotation
            && self.entrance_tile_ptr == other.entrance_tile_ptr
            && self.exhibit_name.copy_to_string() == other.exhibit_name.copy_to_string()
    }
}

impl fmt::Display for ZTHabitat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTHabitat {{",)?;
        writeln!(f, "  vtable: {:#x},", self.vtable)?;
        writeln!(f, "  zt_show_info_ptr: {:#x},", self.zt_show_info_ptr)?;
        writeln!(f, "  owned_tiles_ptr: {:#x},", self.owned_tiles_ptr)?;
        writeln!(f, "  entrance_tile_ptr: {:#x},", self.entrance_tile_ptr)?;
        writeln!(f, "  entrance_rotation: {:#x},", self.entrance_rotation)?;
        writeln!(f, "  characteristics_dirty: {},", self.characteristics_dirty)?;
        writeln!(f, "  time_last_serviced: {:#x},", self.time_last_serviced)?;
        writeln!(f, "  attractiveness: {},", self.attractiveness)?;
        writeln!(f, "  has_keeper_assigned_raw: {},", self.has_keeper_assigned_raw)?;
        writeln!(f, "  current_donations: {},", self.current_donations)?;
        writeln!(f, "  last_donations: {},", self.last_donations)?;
        writeln!(f, "  total_donations: {},", self.total_donations)?;
        writeln!(f, "  current_upkeep: {},", self.current_upkeep)?;
        writeln!(f, "  last_upkeep: {},", self.last_upkeep)?;
        writeln!(f, "  total_upkeep: {},", self.total_upkeep)?;
        writeln!(f, "  unknown_u32_2: {:#x},", self.unknown_u32_2)?;
        writeln!(f, "  unknown_u32_3: {:#x},", self.unknown_u32_3)?;
        writeln!(f, "  unknown_u32_4: {:#x},", self.unknown_u32_4)?;
        writeln!(f, "  created_timestamp: {},", UtcDateTime::try_from(self.created_timestamp).unwrap())?;
        writeln!(
            f,
            "  unknown_nt_time: {} ({}, {}, {}),",
            UtcDateTime::try_from(self.unknown_nt_time).unwrap(),
            self.unknown_nt_time.to_raw() as f64,
            self.unknown_nt_time.to_raw() as u32,
            (self.unknown_nt_time.to_raw() >> 32) as u32
        )?;
        writeln!(f, "  exhibit_name: {},", self.exhibit_name.copy_to_string())?;

        // writeln!(f, "  entrance_x: {},", self.entrance_x)?;
        // writeln!(f, "  entrance_y: {},", self.entrance_y)?;
        // writeln!(f, "  entrance_rotation: {},", self.entrance_rotation)?;
        // writeln!(f, "  unknown_ptr: {:#x},", self.unknown_ptr)?;
        // writeln!(f, "  unknown_ptr2: {:#x},", self.unknown_ptr2)?;
        // writeln!(f, "  unknown_ptr3: {:#x},", self.unknown_ptr3)?;
        // writeln!(f, "  current_donations: {},", self.current_donations)?;
        // writeln!(f, "  last_donations: {},", self.last_donations)?;
        // writeln!(f, "  total_donations: {},", self.total_donations)?;
        // writeln!(f, "  current_upkeep: {},", self.current_upkeep)?;
        // writeln!(f, "  last_upkeep: {},", self.last_upkeep)?;
        // writeln!(f, "  total_upkeep: {},", self.total_upkeep)?;
        // writeln!(f, " unknown_ptr4: {:#x},", self.unknown_ptr4)?;
        // writeln!(f, " unknown_ptr5: {:#x},", self.unknown_ptr5)?;
        // writeln!(f, " unknown_ptr6: {:#x},", self.unknown_ptr6)?;
        // writeln!(f, " created_timestamp: {:#x},", self.created_timestamp)?;
        writeln!(f, "}}")
    }
}

fn command_get_zt_habitat_mgr(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_habitat_mgr = globals().zthabitatmgr();
    Ok(format!("{}", zt_habitat_mgr))
}

fn command_get_zt_habitats(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_habitat_mgr = globals().zthabitatmgr();
    let mut result_string = String::new();
    for i in 0..zt_habitat_mgr.exhibit_array.len() {
        let habitat = zt_habitat_mgr.exhibit_array.get(i);
        let habitat_location = zt_habitat_mgr.exhibit_array.get_ptr(i);
        result_string.push_str(&format!("Habitat {} ({:#x}): ", i, habitat_location));
        result_string.push_str(&format!("  popularity: {},\n", habitat.get_popularity()));
        result_string.push_str(&format!("{}\n", habitat));
    }
    Ok(result_string)
}

#[detour_mod]
pub mod hooks_zthabitatmgr {
    use super::*;
    use openzt_detour::generated::{
        zthabitat::{
            GET_ATTRACTIVENESS, GET_GATE_TILE_IN, GET_GATE_TILE_OUT, GET_POPULARITY, GET_SHOW_INFO_ID, HAS_KEEPER_ASSIGNED, IS_SHOW_STOPPED, LISTEN,
            SET_IS_NOT_SHOW_EXHIBIT, SET_IS_SHOW_EXHIBIT, UPDATE, BLOCK_SERVICE,
            RECALCULATE_VIEWING_AREAS, ADD_VIEWING_AREA, REMOVE_VIEWING_AREA, REMOVE_FROM_ALL_VAS, RECREATE_OAS,
            PATH_PLACED as ZTHABITAT_PATH_PLACED,
            GET_NEEDY_NESTED_TANK as ZTHABITAT_GET_NEEDY_NESTED_TANK,
        },
        zthabitatmgr::{
            DO_TANK_CHECK, ENTER_NEW_MONTH, GET_AVERAGE_HABITAT_ATTRACTIVENESS, GET_HABITAT, GET_NUM_FAMILIES, GET_NUM_SPECIES, HABITAT_TILE_CHANGED,
            HIGHLIGHT_HABITAT, REPLACE_FENCE_WITH_GATE, REPLACE_GATE, REPLACE_GATE_WITH_FENCE, SCENERY_ENTITY_CHANGE, TERRAIN_TILE_CHANGED,
            UNHIGHLIGHT_HABITAT, PATH_PLACED as ZTHABITATMGR_PATH_PLACED, PATH_REMOVED as ZTHABITATMGR_PATH_REMOVED, CHECK_ENTER_HABITAT,
            GET_OUTERMOST_TANK, GET_NEEDY_NESTED_TANK, ENTITY_ABOUT_TO_BE_PLACED, ENTITY_ABOUT_TO_BE_REMOVED, ENTITY_PLACED, ENTITY_REMOVED,
            BEFORE_ENTITY_CHANGE, TERRAIN_ABOUT_TO_BE_CHANGED, TERRAIN_CHANGED,
        },
    };

    // 00410349 BFTile * __thiscall OOAnalyzer::ZTHabitat::getGateTileIn(ZTHabitat *this)
    #[detour(GET_GATE_TILE_IN)]
    unsafe extern "thiscall" fn get_gate_tile_in(_this: *const u32) -> *const u32 {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(_this) };
        match habitat.get_gate_tile_in() {
            Some(tile) => globals().ztworldmgr().get_ptr_from_bftile(&tile) as *const u32,
            None => std::ptr::null(),
        }
    }

    #[detour(GET_GATE_TILE_OUT)]
    unsafe extern "thiscall" fn get_gate_tile_out(_this: *const u32) -> i32 {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(_this) };
        match habitat.get_gate_tile_out() {
            Some(tile) => globals().ztworldmgr().get_ptr_from_bftile(&tile) as i32,
            None => 0,
        }
    }

    #[detour(GET_ATTRACTIVENESS)]
    unsafe extern "thiscall" fn get_attractiveness(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_attractiveness()
    }

    #[detour(HAS_KEEPER_ASSIGNED)]
    unsafe extern "thiscall" fn has_keeper_assigned(this: *const u32) -> u8 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.has_keeper_assigned() as u8
    }

    #[detour(GET_SHOW_INFO_ID)]
    unsafe extern "thiscall" fn get_show_info_id(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_show_info_id() as u32
    }

    #[detour(DO_TANK_CHECK)]
    unsafe extern "stdcall" fn do_tank_check(habitat_ptr: i32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr as u32) }.do_tank_check()
    }

    #[detour(IS_SHOW_STOPPED)]
    unsafe extern "fastcall" fn is_show_stopped(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.is_show_stopped() as u32
    }

    #[detour(GET_POPULARITY)]
    unsafe extern "thiscall" fn get_popularity(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_popularity()
    }

    #[detour(GET_HABITAT)]
    unsafe extern "thiscall" fn get_habitat(this: *const u32, pos_x: i32, pos_y: i32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_habitat_ptr(pos_x, pos_y)
    }

    #[detour(IS_RIGHT_SALINITY)]
    unsafe extern "thiscall" fn is_right_salinity(this: *const u32, animal_type: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.is_right_salinity(animal_type)
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn listen(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.listen()
    }

    #[detour(SET_IS_SHOW_EXHIBIT)]
    unsafe extern "thiscall" fn set_is_show_exhibit(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_is_show_exhibit()
    }

    #[detour(SET_IS_NOT_SHOW_EXHIBIT)]
    unsafe extern "thiscall" fn set_is_not_show_exhibit(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_is_not_show_exhibit()
    }

    #[detour(VALIDATE_POSITIONS)]
    unsafe extern "thiscall" fn validate_positions(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.validate_positions()
    }

    #[detour(REMOVE_HABITAT_TILES)]
    unsafe extern "thiscall" fn remove_habitat_tiles(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_habitat_tiles()
    }

    #[detour(RESET_UNIT_AI)]
    unsafe extern "thiscall" fn reset_unit_ai(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.reset_unit_ai()
    }

    /// `other_habitat` is unused - see [`ZTHabitat::add_habitat_tiles`]'s own doc comment on why the
    /// real third parameter is always `this` in practice.
    #[detour(ADD_HABITAT_TILES)]
    unsafe extern "thiscall" fn add_habitat_tiles(this: *const u32, seed_tile: *const u32, _other_habitat: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.add_habitat_tiles(seed_tile as u32)
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, elapsed: u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.update(elapsed)
    }

    #[detour(BLOCK_SERVICE)]
    unsafe extern "thiscall" fn block_service(this: *const u32, keeper: *const u32, skip_tank_depth_check: bool, check_tank_and_neighbors: bool) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.block_service(keeper as u32, skip_tank_depth_check, check_tank_and_neighbors)
    }

    /// `generated.rs`'s own entry types `include_neighbors` as plain `bool` - matches real vanilla's own
    /// `TEST AL,AL` byte-only read (`ZTHabitat_getNumAnimals.asm`), no `low_byte_bool` masking needed.
    #[detour(GET_NUM_ANIMALS)]
    unsafe extern "thiscall" fn get_num_animals(this: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_animals(include_neighbors)
    }

    #[detour(GET_ANIMALS)]
    unsafe extern "thiscall" fn get_animals(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_animals() as i32
    }

    #[detour(GET_AMOUNT_KEEPER_FOOD)]
    unsafe extern "thiscall" fn get_amount_keeper_food(this: *const u32, category: u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_amount_keeper_food(category, include_neighbors)
    }

    #[detour(GET_FOOD_TO_LEAVE)]
    unsafe extern "thiscall" fn get_food_to_leave(this: *const u32, category: u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_food_to_leave(category as i32, include_neighbors)
    }

    #[detour(GET_NUM_KEEPERS)]
    unsafe extern "thiscall" fn get_num_keepers(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_keepers()
    }

    #[detour(IS_BEING_SERVICED)]
    unsafe extern "thiscall" fn is_being_serviced(this: *const u32) -> u8 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.is_being_serviced() as u8
    }

    #[detour(SEND_MAINT_WORKER_CLEANUP_EVENTS)]
    unsafe extern "thiscall" fn send_maint_worker_cleanup_events(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.send_maint_worker_cleanup_events()
    }

    #[detour(GET_NUM_HUNGRY_FOODLESS_ANIMALS)]
    unsafe extern "thiscall" fn get_num_hungry_foodless_animals(this: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_hungry_foodless_animals(include_neighbors)
    }

    #[detour(GET_NUM_SICKLY_ANIMALS)]
    unsafe extern "thiscall" fn get_num_sickly_animals(this: *const u32, keeper: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_sickly_animals(keeper as u32, include_neighbors)
    }

    #[detour(GET_SICKLY_ANIMALS)]
    unsafe extern "thiscall" fn get_sickly_animals(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_sickly_animals(out_vector as u32)
    }

    #[detour(GET_NEAREST_SICK_ANIMAL)]
    unsafe extern "thiscall" fn get_nearest_sick_animal(this: *const u32, keeper: i32, check_can_see: i8) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_nearest_sick_animal(keeper as u32, check_can_see != 0) as *const i32
    }

    #[detour(GET_VIEWING_AREAS_WITH_GUESTS)]
    unsafe extern "thiscall" fn get_viewing_areas_with_guests(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_viewing_areas_with_guests(out_vector as u32)
    }

    #[detour(HAS_BLDG)]
    unsafe extern "thiscall" fn has_bldg(this: *const u32, entity: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.has_bldg(entity as u32)
    }

    #[detour(REMOVE_VIEWING_AREAS)]
    unsafe extern "thiscall" fn remove_viewing_areas(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_viewing_areas()
    }

    /// Real vanilla returns `&this->field_0x6c` itself (`.asm`-confirmed `LEA EAX,[ESI+0x6c]`) rather
    /// than the vector's own begin pointer - real callers dereference the result once more to reach the
    /// actual `ZTAnimal**` (see [`ZTHabitat::get_all_animals`]'s own doc comment). Reproduced here via
    /// [`Self`]'s own field offset rather than a stored constant, since it must track the struct layout.
    #[detour(GET_ALL_ANIMALS)]
    unsafe extern "thiscall" fn get_all_animals(this: *const u32, sort: i8) -> *const i32 {
        let _ = unsafe { ref_from_memory::<ZTHabitat>(this) }.get_all_animals(sort != 0);
        (this as u32 + 0x6c) as *const i32
    }

    #[detour(GET_SURROUNDING_SPECIES)]
    unsafe extern "thiscall" fn get_surrounding_species(this: *const u32) -> *const i32 {
        let _ = unsafe { ref_from_memory::<ZTHabitat>(this) }.surrounding_species();
        (this as u32 + 0x13c) as *const i32
    }

    #[detour(REMOVE_SPECIES)]
    unsafe extern "thiscall" fn remove_species(this: *const u32, species_key: i32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_species(species_key as u32)
    }

    #[detour(ZTHABITAT_SET_DIRTY_CHARACTERISTICS)]
    unsafe extern "thiscall" fn set_dirty_characteristics(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_dirty_characteristics()
    }

    #[detour(ACCEPT_DONATION)]
    unsafe extern "thiscall" fn accept_donation(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.accept_donation(amount)
    }

    #[detour(SET_TIME_LAST_SERVICED)]
    unsafe extern "thiscall" fn set_time_last_serviced(this: *const u32, time: u32, propagate: bool) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_time_last_serviced(time, propagate)
    }

    /// Detoured (byte-for-byte reproducing real vanilla's own call graph adds no new risk over baseline),
    /// but deliberately **not** exercised by an active live test - see
    /// [`ZTHabitat::trigger_death_arrived`]'s own doc comment: its `species_key == 0` branch marks every
    /// animal in the habitat with an unidentified "death arrived" flag, and this pass found no reader for
    /// that flag anywhere in the decompile corpus, so the real consequence of setting it on a live zoo's
    /// real animals is unconfirmed - same "no known safe way to exercise this live" reasoning already
    /// applied to [`ZTHabitatMgr::clear_staff_habitat`]/[`ZTHabitatMgr::can_see_habitat_from_building`].
    #[detour(TRIGGER_DEATH_ARRIVED)]
    unsafe extern "thiscall" fn trigger_death_arrived(this: *const u32, species_key: i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.trigger_death_arrived(species_key)
    }

    /// `generated.rs`'s own entry types the `file` parameter as `*const u32` rather than `*const i8` -
    /// an ABI-identical wart (both are 32-bit pointers under `thiscall`), cast at the call site rather
    /// than treated as a real signature difference (per `CLAUDE.md`'s own note on this generator
    /// quirk - never hand-edit `generated.rs` itself to "fix" it).
    #[detour(ZTHABITAT_SAVE)]
    unsafe extern "thiscall" fn zthabitat_save(this: *const u32, file: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.save(file as *const i8)
    }

    #[detour(ZTHABITATMGR_SAVE)]
    unsafe extern "thiscall" fn zthabitatmgr_save(this: *const u32, file: *const i8) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.save(file)
    }

    #[detour(ZTHABITATMGR_ADD_HABITAT)]
    unsafe extern "thiscall" fn zthabitatmgr_add_habitat(this: *const u32, habitat_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.add_habitat(habitat_ptr as u32)
    }

    #[detour(ZTHABITATMGR_CREATE_HABITAT)]
    unsafe extern "thiscall" fn zthabitatmgr_create_habitat(this: *const u32, seed_tile: *const u8, resize_tile: *const u32, gate_tile: u32, name_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.create_habitat(seed_tile as u32, resize_tile as u32, gate_tile, name_ptr as u32)
    }

    #[detour(GET_ZOO_ENTRANCE_TILE)]
    unsafe extern "thiscall" fn get_zoo_entrance_tile(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_zoo_entrance_tile_ptr() as i32
    }

    #[detour(GET_AVERAGE_HABITAT_ATTRACTIVENESS)]
    unsafe extern "fastcall" fn get_average_habitat_attractiveness(this: i32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this as u32) }.get_average_habitat_attractiveness()
    }

    #[detour(GET_NUM_FAMILIES)]
    unsafe extern "fastcall" fn get_num_families(this: i32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this as u32) }.get_num_families()
    }

    #[detour(GET_NUM_SPECIES)]
    unsafe extern "fastcall" fn get_num_species(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_num_species()
    }

    /// Real vanilla is a free `stdcall` helper taking a `ZTHabitat*` directly, not a `ZTHabitatMgr`
    /// method - see `ZTHabitat::highlight`'s own doc comment. Null-checked here rather than inside
    /// `highlight` itself, matching real vanilla's own `if (param_1 != NULL)` guard at the call boundary.
    #[detour(HIGHLIGHT_HABITAT)]
    unsafe extern "stdcall" fn highlight_habitat(habitat: *const u32, hilite: bool) {
        if habitat.is_null() {
            return;
        }
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.highlight(hilite)
    }

    /// See [`highlight_habitat`]'s own doc comment - same free-function/null-guard shape, real vanilla's
    /// `ZTHabitatMgr::unhighlightHabitat`.
    #[detour(UNHIGHLIGHT_HABITAT)]
    unsafe extern "stdcall" fn unhighlight_habitat(habitat: *const u32) {
        if habitat.is_null() {
            return;
        }
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.unhighlight()
    }

    #[detour(ENTER_NEW_MONTH)]
    unsafe extern "thiscall" fn enter_new_month(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.enter_new_month()
    }

    // `MOVE_GATE_TO_1` is deliberately NOT detoured yet - see `ZTHabitat::move_gate_to`'s own doc
    // comment for why (a live-tested crash traced back to the same "entrance tile's own fence-slot
    // array" uncertainty this file's "Correction: getSize/getGate are not leaf functions" note already
    // flags for `getGate`).

    /// Real vanilla is a plain free `stdcall` helper (`generated.rs`'s own signature has no `this`) -
    /// see [`crate::zthabitatmgr::ZTHabitatMgr::replace_gate_with_fence`]'s own doc comment.
    #[detour(REPLACE_GATE_WITH_FENCE)]
    unsafe extern "stdcall" fn replace_gate_with_fence(fence: *const i32) -> bool {
        ZTHabitatMgr::replace_gate_with_fence(fence as u32)
    }

    #[detour(REPLACE_FENCE_WITH_GATE)]
    unsafe extern "thiscall" fn replace_fence_with_gate(this: *const u32, fence: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.replace_fence_with_gate(fence as u32) as u32
    }

    #[detour(REPLACE_GATE)]
    unsafe extern "thiscall" fn replace_gate(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.replace_gate()
    }

    #[detour(HABITAT_TILE_CHANGED)]
    unsafe extern "thiscall" fn habitat_tile_changed(this: *const u32, tile: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.habitat_tile_changed(tile as u32)
    }

    #[detour(TERRAIN_TILE_CHANGED)]
    unsafe extern "thiscall" fn terrain_tile_changed(this: *const u32, tile: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.terrain_tile_changed(tile as u32)
    }

    #[detour(SCENERY_ENTITY_CHANGE)]
    unsafe extern "thiscall" fn scenery_entity_change(this: *const u32, tile: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.scenery_entity_change(tile as u32)
    }

    #[detour(ENTITY_ABOUT_TO_BE_PLACED)]
    unsafe extern "thiscall" fn entity_about_to_be_placed(this: *const u32, entity: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_about_to_be_placed(entity as u32)
    }

    #[detour(ENTITY_PLACED)]
    unsafe extern "thiscall" fn entity_placed(this: *const u32, entity: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_placed(entity as u32)
    }

    #[detour(ENTITY_ABOUT_TO_BE_REMOVED)]
    unsafe extern "thiscall" fn entity_about_to_be_removed(this: *const u32, entity: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_about_to_be_removed(entity as u32)
    }

    #[detour(ENTITY_REMOVED)]
    unsafe extern "thiscall" fn entity_removed(this: *const u32, tile: *const u32, entity_type: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_removed(tile as u32, entity_type as u32)
    }

    /// Real vanilla is a plain free `stdcall` helper (`generated.rs`'s own signature has no `this`) -
    /// see [`crate::zthabitatmgr::ZTHabitatMgr::before_entity_change`]'s own doc comment.
    #[detour(BEFORE_ENTITY_CHANGE)]
    unsafe extern "stdcall" fn before_entity_change(habitat_ptr: *const i32) {
        ZTHabitatMgr::before_entity_change(habitat_ptr as u32)
    }

    #[detour(TERRAIN_ABOUT_TO_BE_CHANGED)]
    unsafe extern "thiscall" fn terrain_about_to_be_changed(this: *const u32, x: i32, y: i32, size: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.terrain_about_to_be_changed(x, y, size)
    }

    #[detour(TERRAIN_CHANGED)]
    unsafe extern "thiscall" fn terrain_changed(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.terrain_changed()
    }

    #[detour(HILITE_AMPHIBIOUS_NEIGHBORS)]
    unsafe extern "thiscall" fn hilite_amphibious_neighbors(this: *const u32, hilite: i8) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.hilite_amphibious_neighbors(hilite != 0)
    }

    #[detour(HILITE_SHOW_NEIGHBORS)]
    unsafe extern "thiscall" fn hilite_show_neighbors(this: *const u32, hilite: i8) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.hilite_show_neighbors(hilite != 0)
    }

    #[detour(CHECK_AMPHIBIOUS_NEIGHBOR)]
    unsafe extern "thiscall" fn check_amphibious_neighbor(this: *const u32, habitat_a: *const u32, tile_a: *const u32, tile_b: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.check_amphibious_neighbor(habitat_a as u32, tile_a as u32, tile_b as u32) as u32
    }

    #[detour(UPDATE_AMPHIBIOUS_NEIGHBORS_1)]
    unsafe extern "thiscall" fn update_amphibious_neighbors_1(this: *const u32, habitat: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_amphibious_neighbors(habitat as u32)
    }

    #[detour(UPDATE_AMPHIBIOUS_NEIGHBORS_0)]
    unsafe extern "thiscall" fn update_amphibious_neighbors_0(this: *const u32, tile: i32, direction: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_amphibious_neighbors_from_tile(tile as u32, direction)
    }

    #[detour(CHECK_SHOW_NEIGHBOR)]
    unsafe extern "thiscall" fn check_show_neighbor(this: *const u32, habitat_a: *const u32, tile_a: *const u32, tile_b: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.check_show_neighbor(habitat_a as u32, tile_a as u32, tile_b as u32) as u32
    }

    #[detour(UPDATE_SHOW_NEIGHBORS_1)]
    unsafe extern "thiscall" fn update_show_neighbors_1(this: *const u32, habitat: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_show_neighbors(habitat as u32)
    }

    #[detour(UPDATE_SHOW_NEIGHBORS_0)]
    unsafe extern "thiscall" fn update_show_neighbors_0(this: *const u32, tile: i32, direction: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_show_neighbors_from_tile(tile as u32, direction)
    }

    #[detour(DO_SHOW_CHECK)]
    unsafe extern "thiscall" fn do_show_check(this: *const u32, habitat: *const i32, remove_illegal: i8) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.do_show_check(habitat as u32, remove_illegal != 0)
    }

    #[detour(CAN_SEE_SHOW_FROM_BUILDING)]
    unsafe extern "thiscall" fn can_see_show_from_building(this: *const u32, building: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.can_see_show_from_building(building as u32)
    }

    #[detour(HABITAT_SEEN_FROM_BUILDING)]
    unsafe extern "thiscall" fn habitat_seen_from_building(this: *const u32, building: i32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.habitat_seen_from_building(building as u32) as *const i32
    }

    #[detour(CAN_FIND_PATH)]
    unsafe extern "thiscall" fn can_find_path(this: *const u32, tile_a: *const u32, tile_b: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.can_find_path(tile_a as u32, tile_b as u32)
    }

    #[detour(CLEAR_PATHFINDING)]
    unsafe extern "thiscall" fn clear_pathfinding(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.clear_pathfinding()
    }

    /// Real vanilla is a plain free `stdcall` helper (no `this`) - see
    /// [`ZTHabitatMgr::clear_staff_habitat`]'s own doc comment.
    #[detour(CLEAR_STAFF_HABITAT)]
    unsafe extern "stdcall" fn clear_staff_habitat(staff: *const u32) {
        ZTHabitatMgr::clear_staff_habitat(staff as u32)
    }

    #[detour(GET_TANK)]
    unsafe extern "thiscall" fn get_tank(this: *const u32, tile: *const u32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_tank(tile as u32) as *const i32
    }

    #[detour(ZTHABITAT_GET_OUTERMOST_TANK)]
    unsafe extern "thiscall" fn get_outermost_tank(this: *const u32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_outermost_tank() as *const u32
    }

    #[detour(GET_OUTERMOST_TANK)]
    unsafe extern "thiscall" fn zthabitatmgr_get_outermost_tank(this: *const u32, habitat: *const u32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_outermost_tank(habitat as u32) as *const u32
    }

    #[detour(ZTHABITAT_GET_NEEDY_NESTED_TANK)]
    unsafe extern "thiscall" fn get_needy_nested_tank(this: *const u32, keeper: *const u32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_needy_nested_tank(keeper as u32) as *const i32
    }

    #[detour(GET_NEEDY_NESTED_TANK)]
    unsafe extern "thiscall" fn zthabitatmgr_get_needy_nested_tank(this: *const u32, habitat: *const std::ffi::c_void, keeper: *const i32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_needy_nested_tank(habitat as u32, keeper as u32) as *const u32
    }

    #[detour(LEADS_TO)]
    unsafe extern "thiscall" fn leads_to(this: *const u32, habitat_a: *const u32, habitat_b: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.leads_to(habitat_a as u32, habitat_b as u32) as u32
    }

    /// Real vanilla is a plain free `stdcall` helper (no `this`) - see
    /// [`ZTHabitatMgr::break_amphibious_connection`]'s own doc comment.
    #[detour(BREAK_AMPHIBIOUS_CONNECTION)]
    unsafe extern "stdcall" fn break_amphibious_connection(tile_a: *const u32, tile_b: i32) {
        ZTHabitatMgr::break_amphibious_connection(tile_a as u32, tile_b as u32)
    }

    #[detour(FENCE_REPLACED)]
    unsafe extern "thiscall" fn fence_replaced(this: *const u32, tile: *const u32, direction: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.fence_replaced(tile as u32, direction)
    }

    /// Real vanilla is a plain free `stdcall` helper (no `this`) - see
    /// [`ZTHabitatMgr::check_enter_habitat`]'s own doc comment.
    #[detour(CHECK_ENTER_HABITAT)]
    unsafe extern "stdcall" fn check_enter_habitat(habitat: *const u32, unit: *const u32) -> u32 {
        ZTHabitatMgr::check_enter_habitat(habitat as u32, unit as u32)
    }

    #[detour(RECALCULATE_DETERIORATION)]
    unsafe extern "thiscall" fn recalculate_deterioration(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitatMgr>(this) }.recalculate_deterioration()
    }

    #[detour(FILL_ZOO_EXTERIOR)]
    unsafe extern "thiscall" fn fill_zoo_exterior(this: *const u32, tile: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.fill_zoo_exterior(tile as u32)
    }

    #[detour(MARK_ZOO_EXTERIOR)]
    unsafe extern "thiscall" fn mark_zoo_exterior(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.mark_zoo_exterior()
    }

    #[detour(ZTHABITATMGR_UPDATE)]
    unsafe extern "thiscall" fn zthabitatmgr_update(this: *const u32, elapsed: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update(elapsed)
    }

    #[detour(CAN_SEE_HABITAT_FROM_BUILDING)]
    unsafe extern "cdecl" fn can_see_habitat_from_building(habitat_ptr: u32, building_ptr: i32) -> u32 {
        unsafe { ZTHabitatMgr::can_see_habitat_from_building(habitat_ptr, building_ptr as u32) }
    }

    /// Implemented and confirmed correct against real vanilla via a direct call
    /// (`ZTHABITAT_CREATE_EDGE_PAIRS_MATCHES_REAL_LIVE`), but **deliberately left un-hooked** - same
    /// "implemented but detour disabled" precedent this file's own `ZTHabitatMgr::add_habitat`/
    /// `create_habitat` already established. Bisected directly: with every other stage-6h detour enabled,
    /// hooking this one specifically hangs `run_load_live_zoo` dead before it can even log (real, un-ported
    /// `ZTHabitat::resize` calls through to this address during real vanilla's own habitat reconstruction
    /// path); disabling only this one detour (all 8 others enabled) lets the full battery pass
    /// (205/205). Not yet root-caused *why* the hook alone misbehaves when the direct call doesn't -
    /// candidates worth checking first: whether the real vanilla-reconstructed habitat this gets called on
    /// mid-load has an already-consistent `boundary_tile_pairs_begin`/`_end`/`_cap_end` triple at that
    /// exact point, or whether `PoolAlloc::allocate`/`deallocate`'s own real behavior during that specific
    /// load phase differs from a live, already-loaded zoo's steady state.
    #[allow(dead_code)]
    unsafe extern "thiscall" fn create_edge_pairs(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.create_edge_pairs()
    }

    #[detour(RECALCULATE_VIEWING_AREAS)]
    unsafe extern "thiscall" fn recalculate_viewing_areas(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.recalculate_viewing_areas()
    }

    #[detour(ADD_VIEWING_AREA)]
    unsafe extern "thiscall" fn add_viewing_area(this: *const u32, va_ptr: u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.add_viewing_area(va_ptr)
    }

    #[detour(REMOVE_VIEWING_AREA)]
    unsafe extern "thiscall" fn remove_viewing_area(this: *const u32, va_ptr: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_viewing_area(va_ptr as u32)
    }

    #[detour(REMOVE_FROM_ALL_VAS)]
    unsafe extern "thiscall" fn remove_from_all_vas(this: *const u32, tile_ptr: i32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_from_all_vas(tile_ptr as u32)
    }

    #[detour(RECREATE_OAS)]
    unsafe extern "thiscall" fn recreate_oas(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.recreate_oas()
    }

    #[detour(ZTHABITAT_PATH_PLACED)]
    unsafe extern "thiscall" fn zthabitat_path_placed(this: *const u32, tile_ptr: *const std::ffi::c_void) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.path_placed(tile_ptr as u32)
    }

    #[detour(ZTHABITATMGR_PATH_PLACED)]
    unsafe extern "thiscall" fn zthabitatmgr_path_placed(this: *const u32, tile_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.path_placed(tile_ptr as u32)
    }

    #[detour(ZTHABITATMGR_PATH_REMOVED)]
    unsafe extern "thiscall" fn zthabitatmgr_path_removed(this: *const u32, tile_ptr: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.path_removed(tile_ptr as u32)
    }

    /// `(name, is_enabled)` per detour - lets the live battery's `ZTHABITATMGR_DETOURS_ENABLED` test
    /// catch a silently-failed `init_detours()` (error logged, game continues on vanilla) rather than
    /// looking green while every hooked production path still runs real vanilla.
    pub fn detour_status() -> [(&'static str, bool); 98] {
        [
            ("GET_GATE_TILE_IN", GET_GATE_TILE_IN_DETOUR.is_enabled()),
            ("GET_GATE_TILE_OUT", GET_GATE_TILE_OUT_DETOUR.is_enabled()),
            ("GET_ATTRACTIVENESS", GET_ATTRACTIVENESS_DETOUR.is_enabled()),
            ("HAS_KEEPER_ASSIGNED", HAS_KEEPER_ASSIGNED_DETOUR.is_enabled()),
            ("GET_SHOW_INFO_ID", GET_SHOW_INFO_ID_DETOUR.is_enabled()),
            ("IS_SHOW_STOPPED", IS_SHOW_STOPPED_DETOUR.is_enabled()),
            ("GET_POPULARITY", GET_POPULARITY_DETOUR.is_enabled()),
            ("GET_HABITAT", GET_HABITAT_DETOUR.is_enabled()),
            ("IS_RIGHT_SALINITY", IS_RIGHT_SALINITY_DETOUR.is_enabled()),
            ("LISTEN", LISTEN_DETOUR.is_enabled()),
            ("SET_IS_SHOW_EXHIBIT", SET_IS_SHOW_EXHIBIT_DETOUR.is_enabled()),
            ("SET_IS_NOT_SHOW_EXHIBIT", SET_IS_NOT_SHOW_EXHIBIT_DETOUR.is_enabled()),
            ("VALIDATE_POSITIONS", VALIDATE_POSITIONS_DETOUR.is_enabled()),
            ("REMOVE_HABITAT_TILES", REMOVE_HABITAT_TILES_DETOUR.is_enabled()),
            ("RESET_UNIT_AI", RESET_UNIT_AI_DETOUR.is_enabled()),
            ("ADD_HABITAT_TILES", ADD_HABITAT_TILES_DETOUR.is_enabled()),
            ("UPDATE", UPDATE_DETOUR.is_enabled()),
            ("ZTHABITAT_SAVE", ZTHABITAT_SAVE_DETOUR.is_enabled()),
            ("ZTHABITATMGR_SAVE", ZTHABITATMGR_SAVE_DETOUR.is_enabled()),
            ("ZTHABITATMGR_ADD_HABITAT", ZTHABITATMGR_ADD_HABITAT_DETOUR.is_enabled()),
            ("ZTHABITATMGR_CREATE_HABITAT", ZTHABITATMGR_CREATE_HABITAT_DETOUR.is_enabled()),
            ("GET_ZOO_ENTRANCE_TILE", GET_ZOO_ENTRANCE_TILE_DETOUR.is_enabled()),
            ("GET_AVERAGE_HABITAT_ATTRACTIVENESS", GET_AVERAGE_HABITAT_ATTRACTIVENESS_DETOUR.is_enabled()),
            ("GET_NUM_FAMILIES", GET_NUM_FAMILIES_DETOUR.is_enabled()),
            ("GET_NUM_SPECIES", GET_NUM_SPECIES_DETOUR.is_enabled()),
            ("HIGHLIGHT_HABITAT", HIGHLIGHT_HABITAT_DETOUR.is_enabled()),
            ("UNHIGHLIGHT_HABITAT", UNHIGHLIGHT_HABITAT_DETOUR.is_enabled()),
            ("ENTER_NEW_MONTH", ENTER_NEW_MONTH_DETOUR.is_enabled()),
            ("REPLACE_GATE_WITH_FENCE", REPLACE_GATE_WITH_FENCE_DETOUR.is_enabled()),
            ("REPLACE_FENCE_WITH_GATE", REPLACE_FENCE_WITH_GATE_DETOUR.is_enabled()),
            ("REPLACE_GATE", REPLACE_GATE_DETOUR.is_enabled()),
            ("HABITAT_TILE_CHANGED", HABITAT_TILE_CHANGED_DETOUR.is_enabled()),
            ("TERRAIN_TILE_CHANGED", TERRAIN_TILE_CHANGED_DETOUR.is_enabled()),
            ("SCENERY_ENTITY_CHANGE", SCENERY_ENTITY_CHANGE_DETOUR.is_enabled()),
            ("ENTITY_ABOUT_TO_BE_PLACED", ENTITY_ABOUT_TO_BE_PLACED_DETOUR.is_enabled()),
            ("ENTITY_PLACED", ENTITY_PLACED_DETOUR.is_enabled()),
            ("ENTITY_ABOUT_TO_BE_REMOVED", ENTITY_ABOUT_TO_BE_REMOVED_DETOUR.is_enabled()),
            ("ENTITY_REMOVED", ENTITY_REMOVED_DETOUR.is_enabled()),
            ("BEFORE_ENTITY_CHANGE", BEFORE_ENTITY_CHANGE_DETOUR.is_enabled()),
            ("TERRAIN_ABOUT_TO_BE_CHANGED", TERRAIN_ABOUT_TO_BE_CHANGED_DETOUR.is_enabled()),
            ("TERRAIN_CHANGED", TERRAIN_CHANGED_DETOUR.is_enabled()),
            ("HILITE_AMPHIBIOUS_NEIGHBORS", HILITE_AMPHIBIOUS_NEIGHBORS_DETOUR.is_enabled()),
            ("HILITE_SHOW_NEIGHBORS", HILITE_SHOW_NEIGHBORS_DETOUR.is_enabled()),
            ("CHECK_AMPHIBIOUS_NEIGHBOR", CHECK_AMPHIBIOUS_NEIGHBOR_DETOUR.is_enabled()),
            ("UPDATE_AMPHIBIOUS_NEIGHBORS_1", UPDATE_AMPHIBIOUS_NEIGHBORS_1_DETOUR.is_enabled()),
            ("UPDATE_AMPHIBIOUS_NEIGHBORS_0", UPDATE_AMPHIBIOUS_NEIGHBORS_0_DETOUR.is_enabled()),
            ("CHECK_SHOW_NEIGHBOR", CHECK_SHOW_NEIGHBOR_DETOUR.is_enabled()),
            ("UPDATE_SHOW_NEIGHBORS_1", UPDATE_SHOW_NEIGHBORS_1_DETOUR.is_enabled()),
            ("UPDATE_SHOW_NEIGHBORS_0", UPDATE_SHOW_NEIGHBORS_0_DETOUR.is_enabled()),
            ("DO_SHOW_CHECK", DO_SHOW_CHECK_DETOUR.is_enabled()),
            ("CAN_SEE_SHOW_FROM_BUILDING", CAN_SEE_SHOW_FROM_BUILDING_DETOUR.is_enabled()),
            ("HABITAT_SEEN_FROM_BUILDING", HABITAT_SEEN_FROM_BUILDING_DETOUR.is_enabled()),
            ("CAN_FIND_PATH", CAN_FIND_PATH_DETOUR.is_enabled()),
            ("CLEAR_PATHFINDING", CLEAR_PATHFINDING_DETOUR.is_enabled()),
            ("CLEAR_STAFF_HABITAT", CLEAR_STAFF_HABITAT_DETOUR.is_enabled()),
            ("GET_NUM_ANIMALS", GET_NUM_ANIMALS_DETOUR.is_enabled()),
            ("GET_ANIMALS", GET_ANIMALS_DETOUR.is_enabled()),
            ("GET_AMOUNT_KEEPER_FOOD", GET_AMOUNT_KEEPER_FOOD_DETOUR.is_enabled()),
            ("GET_FOOD_TO_LEAVE", GET_FOOD_TO_LEAVE_DETOUR.is_enabled()),
            ("GET_NUM_KEEPERS", GET_NUM_KEEPERS_DETOUR.is_enabled()),
            ("IS_BEING_SERVICED", IS_BEING_SERVICED_DETOUR.is_enabled()),
            ("SEND_MAINT_WORKER_CLEANUP_EVENTS", SEND_MAINT_WORKER_CLEANUP_EVENTS_DETOUR.is_enabled()),
            ("GET_NUM_HUNGRY_FOODLESS_ANIMALS", GET_NUM_HUNGRY_FOODLESS_ANIMALS_DETOUR.is_enabled()),
            ("GET_NUM_SICKLY_ANIMALS", GET_NUM_SICKLY_ANIMALS_DETOUR.is_enabled()),
            ("GET_SICKLY_ANIMALS", GET_SICKLY_ANIMALS_DETOUR.is_enabled()),
            ("GET_NEAREST_SICK_ANIMAL", GET_NEAREST_SICK_ANIMAL_DETOUR.is_enabled()),
            ("GET_VIEWING_AREAS_WITH_GUESTS", GET_VIEWING_AREAS_WITH_GUESTS_DETOUR.is_enabled()),
            ("HAS_BLDG", HAS_BLDG_DETOUR.is_enabled()),
            ("REMOVE_VIEWING_AREAS", REMOVE_VIEWING_AREAS_DETOUR.is_enabled()),
            ("GET_ALL_ANIMALS", GET_ALL_ANIMALS_DETOUR.is_enabled()),
            ("GET_SURROUNDING_SPECIES", GET_SURROUNDING_SPECIES_DETOUR.is_enabled()),
            ("REMOVE_SPECIES", REMOVE_SPECIES_DETOUR.is_enabled()),
            ("ZTHABITAT_SET_DIRTY_CHARACTERISTICS", ZTHABITAT_SET_DIRTY_CHARACTERISTICS_DETOUR.is_enabled()),
            ("ACCEPT_DONATION", ACCEPT_DONATION_DETOUR.is_enabled()),
            ("SET_TIME_LAST_SERVICED", SET_TIME_LAST_SERVICED_DETOUR.is_enabled()),
            ("TRIGGER_DEATH_ARRIVED", TRIGGER_DEATH_ARRIVED_DETOUR.is_enabled()),
            ("RECALCULATE_VIEWING_AREAS", RECALCULATE_VIEWING_AREAS_DETOUR.is_enabled()),
            ("ADD_VIEWING_AREA", ADD_VIEWING_AREA_DETOUR.is_enabled()),
            ("REMOVE_VIEWING_AREA", REMOVE_VIEWING_AREA_DETOUR.is_enabled()),
            ("REMOVE_FROM_ALL_VAS", REMOVE_FROM_ALL_VAS_DETOUR.is_enabled()),
            ("RECREATE_OAS", RECREATE_OAS_DETOUR.is_enabled()),
            ("ZTHABITAT_PATH_PLACED", ZTHABITAT_PATH_PLACED_DETOUR.is_enabled()),
            ("ZTHABITATMGR_PATH_PLACED", ZTHABITATMGR_PATH_PLACED_DETOUR.is_enabled()),
            ("ZTHABITATMGR_PATH_REMOVED", ZTHABITATMGR_PATH_REMOVED_DETOUR.is_enabled()),
            ("GET_TANK", GET_TANK_DETOUR.is_enabled()),
            ("ZTHABITAT_GET_OUTERMOST_TANK", ZTHABITAT_GET_OUTERMOST_TANK_DETOUR.is_enabled()),
            ("GET_OUTERMOST_TANK", GET_OUTERMOST_TANK_DETOUR.is_enabled()),
            ("ZTHABITAT_GET_NEEDY_NESTED_TANK", ZTHABITAT_GET_NEEDY_NESTED_TANK_DETOUR.is_enabled()),
            ("GET_NEEDY_NESTED_TANK", GET_NEEDY_NESTED_TANK_DETOUR.is_enabled()),
            ("LEADS_TO", LEADS_TO_DETOUR.is_enabled()),
            ("BREAK_AMPHIBIOUS_CONNECTION", BREAK_AMPHIBIOUS_CONNECTION_DETOUR.is_enabled()),
            ("FENCE_REPLACED", FENCE_REPLACED_DETOUR.is_enabled()),
            ("RECALCULATE_DETERIORATION", RECALCULATE_DETERIORATION_DETOUR.is_enabled()),
            ("FILL_ZOO_EXTERIOR", FILL_ZOO_EXTERIOR_DETOUR.is_enabled()),
            ("MARK_ZOO_EXTERIOR", MARK_ZOO_EXTERIOR_DETOUR.is_enabled()),
            ("ZTHABITATMGR_UPDATE", ZTHABITATMGR_UPDATE_DETOUR.is_enabled()),
            ("CAN_SEE_HABITAT_FROM_BUILDING", CAN_SEE_HABITAT_FROM_BUILDING_DETOUR.is_enabled()),
            ("CHECK_ENTER_HABITAT", CHECK_ENTER_HABITAT_DETOUR.is_enabled()),
        ]
    }
}

pub fn init() {
    // get_zthabitatmgr() - no args
    lua_fn!("get_zthabitatmgr", "Returns ZTHabitatMgr debug info", "get_zthabitatmgr()", || {
        match command_get_zt_habitat_mgr(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // list_exhibits() - no args
    lua_fn!("list_exhibits", "Lists all zoo exhibits/habitats", "list_exhibits()", || {
        match command_get_zt_habitats(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    if let Err(e) = unsafe { hooks_zthabitatmgr::init_detours() } {
        info!("Error initialising zthabitatmgr detours: {}", e);
    }
}
