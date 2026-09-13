use nt_time::{time::UtcDateTime, FileTime};
use openzt_detour::{
    generated::{
        bfsndmgr::ACQUIRE as BFSNDMGR_ACQUIRE,
        bftile::VALIDATE_POSITIONS as BFTILE_VALIDATE_POSITIONS,
        msvc_std_listuint::INSERT as MSVC_LIST_UINT_INSERT,
        standalone::{OPERATOR_DELETE, OPERATOR_NEW},
        ztshowinfo::{CONSTRUCTOR_1 as ZTSHOWINFO_CONSTRUCTOR, DESTRUCTOR_1 as ZTSHOWINFO_DESTRUCTOR},
        ztshowmgr::{REGISTER_SHOW, UNREGISTER_SHOW},
        ztui_showpanel::SET_EXHIBIT,
        zthabitat::{ADD_HABITAT_TILES, GET_EVENTS, RECALCULATE_CHARACTERISTICS, REMOVE_HABITAT_TILES, RESET_UNIT_AI, VALIDATE_POSITIONS},
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;
use std::fmt;
use tracing::info;

use getset::Getters;

use crate::{
    command_console::CommandError,
    globals::{get_module_base, globals},
    lua_fn,
    util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory, ZTArray, ZTBufferString, ZTString},
    ztmapview::BFTile,
    ztmegatilemgr::entity_type_matches,
    ztshow::call_entity_vtable_noargs,
    ztshowinfo,
    ztworldmgr::{Direction, ZTWorldMgr},
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
    pad2: [u8; 0x04],                  // ----------------------- padding: 4 bytes
    exhibit_array: ZTArray<ZTHabitat>, // 0x01c (0xc bytes)
    other_array_start: u32,            // 0x028 //TODO: Use ZTArray; Seems to be some kind of mapping from BFTile to ZTHabitat or a ZTHabitat index
    other_array_end: u32,              // 0x02c
    other_array_buffer_end: u32,       // 0x030
    pad3: [u8; 0x24],                  // ----------------------- padding: 36 bytes
    popularity_scale_factor: f32,
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
    pad1a: [u8; 0x24],           // ----------------------- padding: 36 bytes
    unknown_flag_0x2c: u8,       // 0x02c // Gates ZTThought::ZTThought's acceptance of a passed-in habitat pointer (see ztthoughtmgr.rs); ZTHabitat::recalculateCharacteristics also early-returns when this is set. Meaning not otherwise confirmed.
    characteristics_dirty: u8,   // 0x02d // Gates the lazy `recalculateCharacteristics` call in getAttractiveness/hasKeeperAssigned (see ZTHabitat_getAttractiveness.c/ZTHabitat_hasKeeperAssigned.c) - distinct from unknown_flag_0x2c above.
    pad1b: [u8; 0x12],           // ----------------------- padding: 18 bytes
    owned_tiles_ptr: u32,        // 0x040 // Pointer to the sentinel node of this habitat's owned-tile list (see TileListNode below), not a BFTile* itself - see getSize/removeHabitatTiles/validatePositions/resetUnitAI/createEdgePairs, all of which walk it identically.
    pad2: [u8; 0x48],            // ----------------------- padding: 72 bytes
    entrance_tile_ptr: u32,      // 0x08c
    entrance_rotation: u32,      // 0x090
    pad3: [u8; 0x58],            // ----------------------- padding: 88 bytes
    unknown_u32: u32,            // 0x0ec
    pad4: [u8; 0x8],             // ----------------------- padding: 8 bytes
    attractiveness: i32,         // 0x0f8 // ZTHabitat::getAttractiveness's cached result, recomputed by recalculateCharacteristics when characteristics_dirty is set.
    current_donactions: f32,     // 0xfc
    last_donactions: f32,        // 0x100
    total_donactions: f32,       // 0x104
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
    pad5b: [u8; 0x22],           // ----------------------- padding: 34 bytes
    exhibit_name: ZTBufferString, // 0x154 // 3-pointer (start/end/buffer_end) buffer string - ZTHabitat::ZTHabitat zero-inits all of field_0x154/0x158/0x15c before allocating, and field_0x160 is a distinct, separately-referenced pointer (ZTHabitat::playShowStartSound etc.) right after it. Was previously mis-typed as the 2-pointer ZTBoundedString, which shifted every field below 4 bytes early.
    start_sound_ptr: u32,        // 0x160 // Real vanilla SNDSound* for the configured `[sounds] startSound`, built/acquired by set_is_show_exhibit and torn down by set_is_not_show_exhibit.
    end_sound_ptr: u32,          // 0x164 // Real vanilla SNDSound* for the configured `[sounds] endSound` - see start_sound_ptr.
    pad6: [u8; 0x10],            // ----------------------- padding: 16 bytes (0x168-0x178: an intrusive-list sentinel the ctor zero-inits, not yet individually reverse-engineered)
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
fn free_event_vector_buffer(buf: u32, byte_capacity: u32) {
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
        if registered == 0 {
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
        writeln!(f, "  unknown_u32: {:#x},", self.unknown_u32)?;
        writeln!(f, "  attractiveness: {},", self.attractiveness)?;
        writeln!(f, "  has_keeper_assigned_raw: {},", self.has_keeper_assigned_raw)?;
        writeln!(f, "  current_donactions: {},", self.current_donactions)?;
        writeln!(f, "  last_donactions: {},", self.last_donactions)?;
        writeln!(f, "  total_donactions: {},", self.total_donactions)?;
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
            SET_IS_NOT_SHOW_EXHIBIT, SET_IS_SHOW_EXHIBIT,
        },
        zthabitatmgr::GET_HABITAT,
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

    /// `(name, is_enabled)` per detour - lets the live battery's `ZTHABITATMGR_DETOURS_ENABLED` test
    /// catch a silently-failed `init_detours()` (error logged, game continues on vanilla) rather than
    /// looking green while every hooked production path still runs real vanilla.
    pub fn detour_status() -> [(&'static str, bool); 16] {
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
