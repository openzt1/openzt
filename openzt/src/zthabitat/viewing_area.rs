//! `ZTViewingArea` (`0x5c` bytes): the guest-facing spot beside a habitat that guests stand at to look in.
//! Layout offsets used here (read from the decompiles, field by field):
//!
//! | Offset | Meaning |
//! |---|---|
//! | `+0x04` / `+0x08` | begin/end of the embedded observable-area tile vector |
//! | `+0x40` / `+0x44` | begin/end of the viewing area's own `BFTile*` vector |
//! | `+0x4c` | characteristics-dirty byte (set by `addTile`/`removeTile`, cleared by `recalculateCharacteristics`) |
//! | `+0x50` | `Ambients*` (null when the seed tile was outside the zoo) |
//! | `+0x54` | guest count |
//!
//! `BFTile` coordinates are the `i32`s at `+0x34` (x) and `+0x38` (y).
//!
//! Only the leaf methods that have no grid side effects are ported here. Everything that mutates the
//! manager's per-tile ownership grid, or that recalculates, stays on vanilla; this module calls
//! `recalculateCharacteristics` through its (un-detoured) address.

use openzt_detour::generated::{
    bftile::IS_IN_ZOO as BFTILE_IS_IN_ZOO,
    ztviewingarea::{
        ADD_TILE, CLEAR_TILE_OF_VA, CONSTRUCTOR, CREATE_OA, DESTRUCTOR, GET_EWEXTENT, GET_NSEXTENT, HAS_OA, RECALCULATE_CHARACTERISTICS, REMOVE_TILE,
        UPDATE_AMBIENTS,
    },
    bfmap::{DISTANCE_CARTESIAN as BFMAP_DISTANCE_CARTESIAN, GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0},
    poolalloc::{
        ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_12 as POOLALLOC_DEALLOCATE_N_12,
        DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4,
    },
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    ztobservablearea::{ADD_TILE as ZTOBSERVABLEAREA_ADD_TILE, RECALCULATE_CHARACTERISTICS as ZTOBSERVABLEAREA_RECALCULATE_OA},
    ztvisibilitytesting::TEST_LOS,
};
use glam::IVec3;
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::support::{call_vtable_slot_with_ptr_ret_bool, free_event_vector_buffer, next_game_random, walk_tile_list};
use super::habitat::ZTHabitat;
use crate::ambients::Ambients;
use crate::globals::{get_module_base, globals};
use crate::util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory};
use crate::vanilla_vector::{erase_element, push_word};

const OFFSET_OA_TILES_BEGIN: u32 = 0x04;
const OFFSET_OA_TILES_END: u32 = 0x08;
const OFFSET_TILES_BEGIN: u32 = 0x40;
const OFFSET_TILES_END: u32 = 0x44;
const OFFSET_HABITAT: u32 = 0x00;
const OFFSET_RECREATE_OA_DIRTY: u32 = 0x25;
const OFFSET_CHARACTERISTICS_DIRTY: u32 = 0x4c;
const OFFSET_AMBIENTS: u32 = 0x50;
const OFFSET_GUEST_COUNT: u32 = 0x54;
const OFFSET_GUIDE_VALUE: u32 = 0x58;
/// Embedded `ZTObservableArea`; its own dirty-characteristics byte is `OA+0x20` = viewing area `+0x24`.
const OFFSET_OBSERVABLE_AREA: u32 = 0x04;
const OFFSET_OA_CHARACTERISTICS_DIRTY: u32 = 0x24;

/// First word of a tile is a pointer to the sentinel of its entity list; a list node's entity is at `+8`.
const LIST_NODE_ENTITY_OFFSET: u32 = 0x08;
/// An entity's type/behaviour object (the one with the `CAST_*` vtable slot) is at `+0x128`.
const ENTITY_TYPE_OBJECT_OFFSET: u32 = 0x128;
/// Vtable slot `+0x1c` on that object: `bool cast(const CastTag*)`, one stack argument.
const VTABLE_CAST_SLOT: u32 = 0x1c;
/// A `ZTGuide`'s guide value, read from its type object.
const GUIDE_VALUE_OFFSET: u32 = 0x1f8;
/// The cast tags (`CAST_ZTGuest` `0x00638700`, `CAST_ZTGuide` `0x00638770`) are static data in `zoo.exe`.
const RVA_CAST_ZTGUEST: u32 = 0x0063_8700 - 0x0040_0000;
const RVA_CAST_ZTGUIDE: u32 = 0x0063_8770 - 0x0040_0000;

const TILE_OFFSET_X: u32 = 0x34;
const TILE_OFFSET_Y: u32 = 0x38;

/// Each grid cell is `0x28` bytes: the habitat pointer at `+0`, then eight `ZTViewingArea*` slots (one per
/// direction) at `+4 + 4 * direction`.
const GRID_SLOT_COUNT: u32 = 8;
const GRID_CELL_FIRST_SLOT: u32 = 4;
/// Bit 0 of the tile byte at `+0x85` is the flag the grid writers set when a tile's slots change.
const TILE_FLAGS_OFFSET: u32 = 0x85;
const TILE_DIRTY_BIT: u8 = 1;
/// `directionArray` (`0x00630d44`): eight `(dx, dy)` signed-byte pairs, read at runtime. `addTile` walks up
/// to [`ADD_TILE_MAX_STEPS`] steps along each looking for a tile owned by the same habitat.
const RVA_DIRECTION_ARRAY: u32 = 0x0063_0d44 - 0x0040_0000;
const ADD_TILE_MAX_STEPS: i32 = 3;

/// Squared tile distance beyond which a habitat tile is never observable from a viewing-area tile.
const OBSERVATION_RANGE_SQUARED: i32 = 100;
const TILE_OFFSET_Z: u32 = 0x3c;
/// Head of vanilla's free list for 12-byte list nodes (`DAT_00638004`).
const RVA_NODE12_FREELIST_HEAD: u32 = 0x0063_8004 - 0x0040_0000;

// Embedded `ZTObservableArea` layout, relative to the observable area (`viewing area + 4`).
const OA_TILES_BEGIN: u32 = 0x00;
const OA_TILES_END: u32 = 0x04;
const OA_SPECIES_COUNT: u32 = 0x0c;
const OA_ANIMAL_COUNT: u32 = 0x10;
const OA_FLAG_A_COUNT: u32 = 0x14;
const OA_FLAG_B_COUNT: u32 = 0x18;
/// Back-pointer to the owning `ZTViewingArea`.
const OA_VIEWING_AREA: u32 = 0x1c;
const OA_DIRTY_CHARACTERISTICS: u32 = 0x20;
const OA_DIRTY_RECREATE: u32 = 0x21;
/// Species-id `vector<int>` (begin/end/capacity-end).
const OA_SPECIES_IDS: u32 = 0x24;
/// Already-counted animal ids (`entity+0x124`), a scratch `vector<int>` rebuilt on every recalculation.
const OA_SEEN_ANIMALS: u32 = 0x30;

const ENTITY_ID_OFFSET: u32 = 0x124;
/// Vtable slot `+0x20` of an animal's type object: the species id, no arguments.
const VTABLE_SPECIES_ID_SLOT: u32 = 0x20;
const ANIMAL_FLAG_A_OFFSET: u32 = 0x3aa;
const ANIMAL_FLAG_B_OFFSET: u32 = 0x3a7;
/// Per-animal viewing accumulators written by `recalculateOA`.
const ANIMAL_VIEW_TOTAL_OFFSET: u32 = 0x368;
const ANIMAL_VIEW_NS_OFFSET: u32 = 0x36c;
const ANIMAL_VIEW_EW_OFFSET: u32 = 0x370;
/// `CAST_ZTAnimal` (`0x00638690`).
const RVA_CAST_ZTANIMAL: u32 = 0x0063_8690 - 0x0040_0000;
/// Per-tile-of-distance attenuation factor (`double` at `0x00630d68`, 0.9), read at runtime.
const RVA_DISTANCE_ATTENUATION: u32 = 0x0063_0d68 - 0x0040_0000;
/// The `BFMap` inside `ZTWorldMgr`.
const WORLD_MGR_BFMAP_OFFSET: u32 = 0x08;

const OFFSET_TILES_CAPACITY_END: u32 = 0x48;
/// `Ambients` is a `0x18`-byte object (see `ambients.rs`), built from this ini with the seed tile's world position.
const AMBIENTS_SIZE: u32 = 0x18;
const RVA_AMBIENTS_CONFIG_NAME: u32 = 0x0063_b770 - 0x0040_0000;
/// Local offset the constructor passes to `BFMap::tileToWorld` (`{0x20, 0x20, 0}`).
const AMBIENTS_TILE_OFFSET: [i32; 3] = [0x20, 0x20, 0];
const OA_SPECIES_IDS_CAPACITY_END: u32 = OA_SPECIES_IDS + 8;
const OA_SEEN_ANIMALS_CAPACITY_END: u32 = OA_SEEN_ANIMALS + 8;
const OA_TILES_CAPACITY_END: u32 = 0x08;

/// Level passed to `Ambients::play` by `updateAmbients`.
const AMBIENTS_PLAY_LEVEL: i32 = 0x50;

/// Pure min/max fold over the tile coordinates. Matches vanilla's seed-from-first-tile shape.
fn extent_of(coords: impl IntoIterator<Item = i32>) -> Option<(i32, i32)> {
    coords.into_iter().fold(None, |acc, c| match acc {
        None => Some((c, c)),
        Some((lo, hi)) => Some((lo.min(c), hi.max(c))),
    })
}

/// Whether the observable-area tile vector `[begin, end)` is non-empty at pointer granularity
/// (`(end - begin) & !3 != 0`, `hasOA`'s own test).
fn oa_vector_nonempty(begin: u32, end: u32) -> bool {
    end.wrapping_sub(begin) & 0xffff_fffc != 0
}

/// Reads the `BFTile*` vector at `+0x40`/`+0x44` and projects each tile through `offset`.
///
/// Vanilla seeds the result from the first tile unconditionally, so an empty vector makes it read
/// through whatever `*begin` holds. `viewing_area` is always non-empty in practice (the constructor
/// adds the seed tile before any caller can reach these), so an empty vector here is not reachable;
/// the first read below keeps vanilla's behaviour regardless.
fn tile_extent(viewing_area: u32, offset: u32) -> (i32, i32) {
    let begin = get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN);
    let end = get_from_memory::<u32>(viewing_area + OFFSET_TILES_END);
    let first = get_from_memory::<i32>(get_from_memory::<u32>(begin) + offset);
    let rest = (begin..end)
        .step_by(4)
        .map(|slot| get_from_memory::<i32>(get_from_memory::<u32>(slot) + offset));
    extent_of(std::iter::once(first).chain(rest)).unwrap_or((first, first))
}

/// Reimplementation of `ZTViewingArea::getEWExtent` (`0x0044ee46`): min/max tile x over the viewing
/// area's own tile vector.
pub fn get_ew_extent(viewing_area: u32) -> (i32, i32) {
    tile_extent(viewing_area, TILE_OFFSET_X)
}

/// Reimplementation of `ZTViewingArea::getNSExtent` (`0x0044f0f2`): min/max tile y.
pub fn get_ns_extent(viewing_area: u32) -> (i32, i32) {
    tile_extent(viewing_area, TILE_OFFSET_Y)
}

/// Whether `entity`'s type object answers true to the cast with the tag at `tag_rva`.
fn entity_casts_to(entity: u32, tag_rva: u32) -> bool {
    let type_object = get_from_memory::<u32>(entity + ENTITY_TYPE_OBJECT_OFFSET);
    if type_object == 0 {
        return false;
    }
    let tag = get_module_base("zoo.exe") as u32 + tag_rva;
    unsafe { call_vtable_slot_with_ptr_ret_bool(type_object, VTABLE_CAST_SLOT, tag) }
}

/// Whether the squared distance between two tiles is close enough to bother with a line-of-sight test.
fn within_observation_range(distance_squared: i32) -> bool {
    distance_squared <= OBSERVATION_RANGE_SQUARED
}

/// Reimplementation of `FUN_004f39a2` (macOS `LOS(tile, tile)`), `createOA`'s per-tile predicate: a null
/// tile never qualifies; tiles more than ten tiles apart (squared distance over 100) never qualify;
/// otherwise the result is `ZTVisibilityTesting::testLOS` between the two tiles (low byte only).
fn is_observable(viewing_tile: u32, habitat_tile: u32) -> bool {
    let distance_squared = if viewing_tile == 0 || habitat_tile == 0 {
        i32::MAX
    } else {
        let (ax, ay) = tile_pos(viewing_tile);
        let (bx, by) = tile_pos(habitat_tile);
        let (dx, dy) = (bx.wrapping_sub(ax), by.wrapping_sub(ay));
        dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
    };
    if !within_observation_range(distance_squared) {
        return false;
    }
    let (ax, ay) = tile_pos(viewing_tile);
    let (bx, by) = tile_pos(habitat_tile);
    let (az, bz) = (get_from_memory::<i32>(viewing_tile + TILE_OFFSET_Z), get_from_memory::<i32>(habitat_tile + TILE_OFFSET_Z));
    low_byte_bool(unsafe { TEST_LOS.original()(ax << 1, ay << 1, az * 2 + 2, bx << 1, by << 1, bz * 2 + 2) })
}

/// Reimplementation of `ZTObservableArea::addTile` (`0x004f3b00`, `this` = the embedded observable area):
/// appends `tile` to the observable-area tile vector unless it is already there.
pub fn observable_area_add_tile(observable_area: u32, tile: u32) {
    let begin = get_from_memory::<u32>(observable_area);
    let end = get_from_memory::<u32>(observable_area + 4);
    if (begin..end).step_by(4).any(|slot| get_from_memory::<u32>(slot) == tile) {
        return;
    }
    push_word(observable_area, tile, free_event_vector_buffer);
}

/// Unlinks `node` from its circular list and returns it to vanilla's 12-byte node free list, exactly as
/// vanilla's inlined `list::erase` does.
fn unlink_and_free_node12(node: u32) {
    let next = get_from_memory::<u32>(node);
    let prev = get_from_memory::<u32>(node + 4);
    save_to_memory(prev, next);
    save_to_memory(next + 4, prev);
    let head_addr = get_module_base("zoo.exe") as u32 + RVA_NODE12_FREELIST_HEAD;
    save_to_memory(node, get_from_memory::<u32>(head_addr));
    save_to_memory(head_addr, node);
}

/// Reimplementation of `ZTViewingArea::createOA` (`0x004f380b`): rebuilds the observable-area tile vector.
/// Takes a copy of the habitat's tile list; for each viewing-area tile, every habitat tile still in the copy
/// that passes [`is_observable`] is added to the observable area, then all of those tiles are removed from
/// the copy so each habitat tile is claimed by the first viewing-area tile that sees it.
///
/// The copy's nodes and sentinel are vanilla's own 12-byte list nodes and are released through vanilla's
/// free list. The per-tile "claimed" scratch vector is a plain Rust `Vec` that never reaches vanilla.
pub fn create_oa(viewing_area: u32) {
    let observable_area = viewing_area + OFFSET_OBSERVABLE_AREA;
    save_to_memory(viewing_area + OFFSET_OA_TILES_END, get_from_memory::<u32>(viewing_area + OFFSET_OA_TILES_BEGIN));

    let habitat = get_from_memory::<u32>(viewing_area + OFFSET_HABITAT);
    let mut copy_sentinel: u32 = 0;
    unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_tiles_copy(&mut copy_sentinel as *mut u32 as u32);

    let begin = get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN);
    let end = get_from_memory::<u32>(viewing_area + OFFSET_TILES_END);
    let mut claimed: Vec<u32> = Vec::new();
    for slot in (begin..end).step_by(4) {
        let viewing_tile = get_from_memory::<u32>(slot);
        claimed.clear();
        for node in walk_tile_list(copy_sentinel) {
            let habitat_tile = get_from_memory::<u32>(node + LIST_NODE_ENTITY_OFFSET);
            if is_observable(viewing_tile, habitat_tile) {
                observable_area_add_tile(observable_area, habitat_tile);
                claimed.push(habitat_tile);
            }
        }
        for &tile in &claimed {
            for node in walk_tile_list(copy_sentinel) {
                if get_from_memory::<u32>(node + LIST_NODE_ENTITY_OFFSET) == tile {
                    unlink_and_free_node12(node);
                }
            }
        }
    }
    save_to_memory(viewing_area + OFFSET_OA_CHARACTERISTICS_DIRTY, 1u8);

    let mut node = get_from_memory::<u32>(copy_sentinel);
    while node != copy_sentinel {
        let next = get_from_memory::<u32>(node);
        unsafe { POOLALLOC_DEALLOCATE_N_12.original()(node as *const u32, 1) };
        node = next;
    }
    save_to_memory(copy_sentinel, copy_sentinel);
    save_to_memory(copy_sentinel + 4, copy_sentinel);
    unsafe { POOLALLOC_DEALLOCATE_N_12.original()(copy_sentinel as *const u32, 1) };
}

/// Guest weight an animal receives from one viewing area: the guest count attenuated by `factor` once per
/// tile of `distance` (each step truncated toward zero, as `FISTP` with the truncate rounding mode does),
/// raised to 1 when there were guests but the attenuation reached 0.
fn attenuated_weight(guests: i32, distance: i32, factor: f64) -> i32 {
    let mut weight = guests;
    for _ in 0..distance.max(0) {
        weight = (weight as f64 * factor) as i64 as i32;
    }
    if guests != 0 && weight == 0 {
        1
    } else {
        weight
    }
}

/// Signs `recalculateOA` applies to an animal's `+0x370` and `+0x36c` accumulators for a `BFMap` direction:
/// `+0x370` is added to for directions 0, 1, 7 and subtracted from otherwise; `+0x36c` is added to for
/// directions 1, 2, 3 and subtracted from otherwise.
fn direction_signs(direction: i32) -> (i32, i32) {
    let first = if matches!(direction, 0 | 1 | 7) { 1 } else { -1 };
    let second = if matches!(direction, 1..=3) { 1 } else { -1 };
    (first, second)
}

fn add_word(address: u32, delta: i32) {
    save_to_memory(address, get_from_memory::<i32>(address).wrapping_add(delta));
}

fn animal_species_id(entity: u32) -> i32 {
    let type_object = get_from_memory::<u32>(entity + ENTITY_TYPE_OBJECT_OFFSET);
    let vtable = get_from_memory::<u32>(type_object);
    let target = get_from_memory::<u32>(vtable + VTABLE_SPECIES_ID_SLOT);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> i32>(target) };
    f(type_object)
}

/// Overwrites the vanilla-owned `vector<int>` at `vector_ptr` with `values`, the way vanilla's inlined
/// `vector::operator=` does: reallocates through `PoolAlloc` only when the capacity is too small (freeing the
/// old buffer through `PoolAlloc::deallocate_n_4`), otherwise copies in place.
fn assign_vanilla_vector(vector_ptr: u32, values: &[i32]) {
    let count = values.len() as u32;
    let begin = get_from_memory::<u32>(vector_ptr);
    let end = get_from_memory::<u32>(vector_ptr + 4);
    let capacity = get_from_memory::<u32>(vector_ptr + 8).wrapping_sub(begin) / 4;
    let write = |at: u32, slice: &[i32]| {
        for (i, v) in slice.iter().enumerate() {
            save_to_memory(at + i as u32 * 4, *v);
        }
    };
    let new_begin = if count > capacity {
        let buffer = unsafe { POOLALLOC_ALLOCATE.original()(count * 4) } as u32;
        write(buffer, values);
        unsafe { POOLALLOC_DEALLOCATE_N_4.original()(begin as *const u32, capacity as i32) };
        save_to_memory(vector_ptr, buffer);
        save_to_memory(vector_ptr + 8, buffer + count * 4);
        buffer
    } else {
        let old_len = end.wrapping_sub(begin) / 4;
        if old_len < count {
            write(begin, &values[..old_len as usize]);
            write(end, &values[old_len as usize..]);
        } else {
            write(begin, values);
        }
        begin
    };
    save_to_memory(vector_ptr + 4, new_begin + count * 4);
}

/// Reimplementation of `ZTObservableArea::recalculateCharacteristics` (`0x00447a00`, `this` = the observable
/// area; `ZTViewingArea::recalculateOA` at `0x004479fd` is a stub falling into it). Rebuilds the recreate/
/// dirty state, then, when dirty, re-counts the animals on the observable tiles and gives each a viewing
/// weight from a randomly chosen tile of the owning viewing area.
///
/// Differences from vanilla, both unreachable in a live game: an entity whose type object is null is not
/// an animal (vanilla faults), and a viewing area with no tiles skips the weight step after advancing the
/// RNG (vanilla divides by zero). Vanilla's own re-entry (`recalculateCharacteristics` calling back in) is
/// kept.
pub fn recalculate_observable_area(observable_area: u32) {
    if get_from_memory::<u8>(observable_area + OA_DIRTY_RECREATE) != 0 {
        create_oa(get_from_memory::<u32>(observable_area + OA_VIEWING_AREA));
        save_to_memory(observable_area + OA_DIRTY_RECREATE, 0u8);
    }
    if get_from_memory::<u8>(observable_area + OA_DIRTY_CHARACTERISTICS) == 0 {
        return;
    }
    for counter in [OA_SPECIES_COUNT, OA_ANIMAL_COUNT, OA_FLAG_A_COUNT, OA_FLAG_B_COUNT] {
        save_to_memory(observable_area + counter, 0u32);
    }
    save_to_memory(observable_area + OA_DIRTY_CHARACTERISTICS, 0u8);
    save_to_memory(observable_area + OA_SEEN_ANIMALS + 4, get_from_memory::<u32>(observable_area + OA_SEEN_ANIMALS));

    let factor = get_from_memory::<f64>(get_module_base("zoo.exe") as u32 + RVA_DISTANCE_ATTENUATION);
    let map = globals().ztworldmgr_ptr() as u32 + WORLD_MGR_BFMAP_OFFSET;
    let mut species_ids: Vec<i32> = Vec::new();

    let mut tile_slot = get_from_memory::<u32>(observable_area + OA_TILES_BEGIN);
    while tile_slot != get_from_memory::<u32>(observable_area + OA_TILES_END) {
        let observed_tile = get_from_memory::<u32>(tile_slot);
        for node in walk_tile_list(get_from_memory::<u32>(observed_tile)) {
            let entity = get_from_memory::<u32>(node + LIST_NODE_ENTITY_OFFSET);
            if entity == 0 || !entity_casts_to(entity, RVA_CAST_ZTANIMAL) {
                continue;
            }
            let animal_id = get_from_memory::<u32>(entity + ENTITY_ID_OFFSET);
            let seen_begin = get_from_memory::<u32>(observable_area + OA_SEEN_ANIMALS);
            let seen_end = get_from_memory::<u32>(observable_area + OA_SEEN_ANIMALS + 4);
            if (seen_begin..seen_end).step_by(4).any(|slot| get_from_memory::<u32>(slot) == animal_id) {
                continue;
            }
            push_word(observable_area + OA_SEEN_ANIMALS, animal_id, free_event_vector_buffer);
            add_word(observable_area + OA_ANIMAL_COUNT, 1);

            let species = animal_species_id(entity);
            if !species_ids.contains(&species) {
                add_word(observable_area + OA_SPECIES_COUNT, 1);
                species_ids.push(species);
            }
            if get_from_memory::<u8>(entity + ANIMAL_FLAG_A_OFFSET) != 0 {
                add_word(observable_area + OA_FLAG_A_COUNT, 1);
            }
            if get_from_memory::<u8>(entity + ANIMAL_FLAG_B_OFFSET) != 0 {
                add_word(observable_area + OA_FLAG_B_COUNT, 1);
            }

            let viewing_area = get_from_memory::<u32>(observable_area + OA_VIEWING_AREA);
            if get_from_memory::<u8>(viewing_area + OFFSET_CHARACTERISTICS_DIRTY) != 0 {
                recalculate_characteristics(viewing_area);
            }
            let guests = get_from_memory::<i32>(viewing_area + OFFSET_GUEST_COUNT);
            let viewing_tiles = get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN);
            let viewing_tile_count = get_from_memory::<u32>(viewing_area + OFFSET_TILES_END).wrapping_sub(viewing_tiles) / 4;
            let draw = next_game_random();
            if viewing_tile_count == 0 {
                continue;
            }
            let target = get_from_memory::<u32>(viewing_tiles + (draw % viewing_tile_count) * 4);
            let distance = unsafe { BFMAP_DISTANCE_CARTESIAN.original()(map, 0, observed_tile as *const i32, target as *const i32) };
            let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(observed_tile as i32, target as i32) };

            let weight = attenuated_weight(guests, distance, factor);
            let (ew_sign, ns_sign) = direction_signs(direction);
            add_word(entity + ANIMAL_VIEW_TOTAL_OFFSET, weight);
            add_word(entity + ANIMAL_VIEW_EW_OFFSET, ew_sign * weight);
            add_word(entity + ANIMAL_VIEW_NS_OFFSET, ns_sign * weight);
        }
        tile_slot += 4;
    }

    assign_vanilla_vector(observable_area + OA_SPECIES_IDS, &species_ids);
}

/// Reimplementation of `ZTObservableArea::ZTObservableArea` (`0x0044ef63`, `this` = the embedded observable
/// area): empties the tile vector, clears both dirty bytes and empties the species-id and seen-animal
/// vectors. The counters and the back-pointer are left as they were, exactly as vanilla does.
fn construct_observable_area(observable_area: u32) {
    for offset in [OA_TILES_BEGIN, OA_TILES_END, OA_TILES_CAPACITY_END] {
        save_to_memory(observable_area + offset, 0u32);
    }
    save_to_memory(observable_area + OA_DIRTY_CHARACTERISTICS, 0u8);
    save_to_memory(observable_area + OA_DIRTY_RECREATE, 0u8);
    for offset in (OA_SPECIES_IDS..OA_SEEN_ANIMALS + 12).step_by(4) {
        save_to_memory(observable_area + offset, 0u32);
    }
}

/// Reimplementation of `ZTObservableArea::~ZTObservableArea` (`0x00459113`): returns the seen-animal vector,
/// the species-id vector and the tile vector to vanilla's allocator by capacity.
fn destruct_observable_area(observable_area: u32) {
    let seen = get_from_memory::<u32>(observable_area + OA_SEEN_ANIMALS);
    if seen != 0 {
        let bytes = get_from_memory::<u32>(observable_area + OA_SEEN_ANIMALS_CAPACITY_END).wrapping_sub(seen) / 4 * 4;
        unsafe { POOLALLOC_DEALLOCATE.original()(seen as *const u32, bytes) };
    }
    for (begin_offset, cap_offset) in [(OA_SPECIES_IDS, OA_SPECIES_IDS_CAPACITY_END), (OA_TILES_BEGIN, OA_TILES_CAPACITY_END)] {
        let begin = get_from_memory::<u32>(observable_area + begin_offset);
        if begin != 0 {
            let bytes = get_from_memory::<u32>(observable_area + cap_offset).wrapping_sub(begin) / 4 * 4;
            free_event_vector_buffer(begin, bytes);
        }
    }
}

/// Reimplementation of `ZTViewingArea::ZTViewingArea` (`0x0044ee8e`): a viewing area over `habitat`, seeded
/// with `seed_tile`. When the tile is in the zoo it is added (see [`add_tile`]), an `Ambients` object for
/// `guests/va.ini` is built at the tile's world position (allocated with vanilla's `operator_new`, as the
/// destructor frees it with `operator_delete`), and the observable area is created.
pub fn construct(viewing_area: u32, habitat: u32, seed_tile: u32) {
    save_to_memory(viewing_area + OFFSET_HABITAT, habitat);
    construct_observable_area(viewing_area + OFFSET_OBSERVABLE_AREA);
    for offset in [OFFSET_TILES_BEGIN, OFFSET_TILES_END, OFFSET_TILES_CAPACITY_END] {
        save_to_memory(viewing_area + offset, 0u32);
    }
    save_to_memory(viewing_area + OFFSET_CHARACTERISTICS_DIRTY, 1u8);
    if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.original()(seed_tile as *const u32, 1) }) {
        return;
    }
    add_tile(viewing_area, seed_tile);

    let ambients = unsafe { OPERATOR_NEW.original()(AMBIENTS_SIZE) } as u32;
    if ambients != 0 {
        let tile_pos = get_from_memory::<IVec3>(seed_tile + TILE_OFFSET_X);
        let world_position = globals().ztworldmgr().tile_to_world(tile_pos, IVec3::from_array(AMBIENTS_TILE_OFFSET));
        let position = [world_position.x, world_position.y, world_position.z];
        let name = (get_module_base("zoo.exe") as u32 + RVA_AMBIENTS_CONFIG_NAME) as *const u8;
        unsafe { ref_from_memory::<Ambients>(ambients) }.construct(name, position.as_ptr());
    }
    save_to_memory(viewing_area + OFFSET_AMBIENTS, ambients);
    save_to_memory(viewing_area + OFFSET_OA_CHARACTERISTICS_DIRTY, 1u8);
    save_to_memory(viewing_area + OFFSET_OBSERVABLE_AREA + OA_VIEWING_AREA, viewing_area);
    create_oa(viewing_area);
}

/// Reimplementation of `ZTViewingArea::~ZTViewingArea` (`0x00459196`; `this` is the viewing area itself, per
/// its first instructions): destroys and frees the `Ambients`, clears this viewing area out of every tile's
/// grid slots, frees the tile vector and tears down the observable area. The object itself is freed by the
/// caller.
pub fn destruct(viewing_area: u32) {
    let ambients = get_from_memory::<u32>(viewing_area + OFFSET_AMBIENTS);
    if ambients != 0 {
        unsafe { ref_from_memory::<Ambients>(ambients) }.destruct();
        unsafe { OPERATOR_DELETE.original()(ambients) };
    }
    let mut slot = get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN);
    while slot != get_from_memory::<u32>(viewing_area + OFFSET_TILES_END) {
        clear_tile_of_va(viewing_area, get_from_memory::<u32>(slot));
        slot += 4;
    }
    save_to_memory(viewing_area + OFFSET_TILES_END, get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN));
    let begin = get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN);
    if begin != 0 {
        let bytes = get_from_memory::<u32>(viewing_area + OFFSET_TILES_CAPACITY_END).wrapping_sub(begin) / 4 * 4;
        free_event_vector_buffer(begin, bytes);
    }
    destruct_observable_area(viewing_area + OFFSET_OBSERVABLE_AREA);
}

/// Reimplementation of `ZTViewingArea::recalculateCharacteristics` (`0x00447957`): clears the dirty byte,
/// then counts the guests standing on the viewing area's tiles and takes the guide value of the first
/// guide that has one, marks the observable area dirty and recalculates it.
///
/// Vanilla casts a non-guest entity to `ZTGuide` twice (once to decide, once to read the value); the cast
/// is a pure predicate, so it is done once here. Vanilla dereferences a null type object in the guide
/// branch; here a null type object counts as neither guest nor guide.
pub fn recalculate_characteristics(viewing_area: u32) {
    save_to_memory(viewing_area + OFFSET_CHARACTERISTICS_DIRTY, 0u8);
    save_to_memory(viewing_area + OFFSET_GUEST_COUNT, 0u32);
    save_to_memory(viewing_area + OFFSET_GUIDE_VALUE, 0u32);

    let begin = get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN);
    let end = get_from_memory::<u32>(viewing_area + OFFSET_TILES_END);
    for slot in (begin..end).step_by(4) {
        let tile = get_from_memory::<u32>(slot);
        for node in walk_tile_list(get_from_memory::<u32>(tile)) {
            let entity = get_from_memory::<u32>(node + LIST_NODE_ENTITY_OFFSET);
            if entity_casts_to(entity, RVA_CAST_ZTGUEST) {
                save_to_memory(viewing_area + OFFSET_GUEST_COUNT, get_from_memory::<u32>(viewing_area + OFFSET_GUEST_COUNT).wrapping_add(1));
            } else if get_from_memory::<u32>(viewing_area + OFFSET_GUIDE_VALUE) == 0 && entity != 0 && entity_casts_to(entity, RVA_CAST_ZTGUIDE) {
                let type_object = get_from_memory::<u32>(entity + ENTITY_TYPE_OBJECT_OFFSET);
                let value = get_from_memory::<u32>(type_object + GUIDE_VALUE_OFFSET);
                save_to_memory(viewing_area + OFFSET_GUIDE_VALUE, get_from_memory::<u32>(viewing_area + OFFSET_GUIDE_VALUE).wrapping_add(value));
            }
        }
    }

    save_to_memory(viewing_area + OFFSET_OA_CHARACTERISTICS_DIRTY, 1u8);
    recalculate_observable_area(viewing_area + OFFSET_OBSERVABLE_AREA);
}

/// Reimplementation of `ZTViewingArea::hasOA` (`0x0043aac6`): recalculates if dirty, then reports whether
/// the observable-area tile vector holds at least one entry.
pub fn has_oa(viewing_area: u32) -> bool {
    if get_from_memory::<u8>(viewing_area + OFFSET_CHARACTERISTICS_DIRTY) != 0 {
        recalculate_characteristics(viewing_area);
    }
    oa_vector_nonempty(
        get_from_memory::<u32>(viewing_area + OFFSET_OA_TILES_BEGIN),
        get_from_memory::<u32>(viewing_area + OFFSET_OA_TILES_END),
    )
}

/// Reimplementation of `ZTViewingArea::updateAmbients` (`0x004358f2`): when an `Ambients` object exists,
/// recalculates if dirty and plays it while guests are watching.
pub fn update_ambients(viewing_area: u32, delta: i32) {
    let ambients = get_from_memory::<u32>(viewing_area + OFFSET_AMBIENTS);
    if ambients == 0 {
        return;
    }
    if get_from_memory::<u8>(viewing_area + OFFSET_CHARACTERISTICS_DIRTY) != 0 {
        recalculate_characteristics(viewing_area);
    }
    if get_from_memory::<i32>(viewing_area + OFFSET_GUEST_COUNT) > 0 {
        unsafe { ref_from_memory::<Ambients>(ambients) }.play(delta, AMBIENTS_PLAY_LEVEL);
    }
}

fn mark_tile_dirty(tile: u32) {
    save_to_memory(tile + TILE_FLAGS_OFFSET, get_from_memory::<u8>(tile + TILE_FLAGS_OFFSET) | TILE_DIRTY_BIT);
}

fn tile_pos(tile: u32) -> (i32, i32) {
    (get_from_memory::<i32>(tile + TILE_OFFSET_X), get_from_memory::<i32>(tile + TILE_OFFSET_Y))
}

/// `(dx, dy)` of grid direction `index`, read from the game's own table.
fn direction_step(index: u32) -> (i32, i32) {
    let base = get_module_base("zoo.exe") as u32 + RVA_DIRECTION_ARRAY + index * 2;
    (get_from_memory::<i8>(base) as i32, get_from_memory::<i8>(base + 1) as i32)
}

/// Reimplementation of `ZTViewingArea::clearTileOfVA` (`0x004590c7`): clears every direction slot of the
/// tile's grid cell that holds this viewing area, marking the tile dirty when one changes.
///
/// Vanilla indexes the grid without a bounds check; a tile outside the grid is skipped here instead of
/// reading out of bounds.
pub fn clear_tile_of_va(viewing_area: u32, tile: u32) {
    let (x, y) = tile_pos(tile);
    let Some(cell) = globals().zthabitatmgr().get_habitat_cell_addr(x, y) else {
        return;
    };
    for direction in 0..GRID_SLOT_COUNT {
        let slot = cell + GRID_CELL_FIRST_SLOT + direction * 4;
        if get_from_memory::<u32>(slot) == viewing_area {
            save_to_memory(slot, 0u32);
            mark_tile_dirty(tile);
        }
    }
}

/// Reimplementation of `ZTViewingArea::addTile` (`0x0044ebe6`): appends an in-zoo tile to the viewing area's
/// tile vector, then for each of the eight directions walks up to three steps from the tile looking for a
/// tile owned by this viewing area's habitat and records this viewing area in the tile's grid cell slot.
pub fn add_tile(viewing_area: u32, tile: u32) {
    if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.original()(tile as *const u32, 1) }) {
        return;
    }
    push_word(viewing_area + OFFSET_TILES_BEGIN, tile, free_event_vector_buffer);

    let world = globals().ztworldmgr();
    let habitat_mgr = globals().zthabitatmgr();
    let habitat = get_from_memory::<u32>(viewing_area + OFFSET_HABITAT);
    let (tile_x, tile_y) = tile_pos(tile);

    for direction in 0..GRID_SLOT_COUNT {
        let (dx, dy) = direction_step(direction);
        let (mut x, mut y) = (tile_x, tile_y);
        for _ in 0..ADD_TILE_MAX_STEPS {
            y += dy;
            x += dx;
            if x < 0 || y < 0 || x >= world.map_x_size as i32 || y >= world.map_y_size as i32 {
                break;
            }
            if habitat_mgr.get_habitat_ptr(x, y) == habitat {
                if let Some(cell) = habitat_mgr.get_habitat_cell_addr(tile_x, tile_y) {
                    let slot = cell + GRID_CELL_FIRST_SLOT + direction * 4;
                    if get_from_memory::<u32>(slot) != viewing_area {
                        save_to_memory(slot, viewing_area);
                        mark_tile_dirty(tile);
                    }
                }
                break;
            }
        }
    }
    save_to_memory(viewing_area + OFFSET_RECREATE_OA_DIRTY, 1u8);
    save_to_memory(viewing_area + OFFSET_CHARACTERISTICS_DIRTY, 1u8);
}

/// Reimplementation of `ZTViewingArea::removeTile` (`0x0050bc1a`): removes the first matching tile from the
/// tile vector (shifting the rest down), clears its grid slots and marks the viewing area dirty. A tile that
/// is not in the vector changes nothing.
pub fn remove_tile(viewing_area: u32, tile: u32) {
    let begin = get_from_memory::<u32>(viewing_area + OFFSET_TILES_BEGIN);
    let end = get_from_memory::<u32>(viewing_area + OFFSET_TILES_END);
    let Some(found) = (begin..end).step_by(4).find(|&slot| get_from_memory::<u32>(slot) == tile) else {
        return;
    };
    erase_element(viewing_area + OFFSET_TILES_BEGIN, found, 4);
    clear_tile_of_va(viewing_area, tile);
    save_to_memory(viewing_area + OFFSET_RECREATE_OA_DIRTY, 1u8);
    save_to_memory(viewing_area + OFFSET_CHARACTERISTICS_DIRTY, 1u8);
}

#[detour_mod]
mod viewing_area_detours {
    use super::*;

    #[detour(GET_EWEXTENT)]
    unsafe extern "thiscall" fn get_ew_extent_detour(this: *const u32, low: *const i32, high: *const i32) {
        let (lo, hi) = get_ew_extent(this as u32);
        unsafe {
            (low as *mut i32).write(lo);
            (high as *mut i32).write(hi);
        }
    }

    #[detour(GET_NSEXTENT)]
    unsafe extern "thiscall" fn get_ns_extent_detour(this: *const u32, low: *const i32, high: *const i32) {
        let (lo, hi) = get_ns_extent(this as u32);
        unsafe {
            (low as *mut i32).write(lo);
            (high as *mut i32).write(hi);
        }
    }

    #[detour(RECALCULATE_CHARACTERISTICS)]
    unsafe extern "thiscall" fn recalculate_characteristics_detour(this: *const u32) {
        recalculate_characteristics(this as u32);
    }

    #[detour(ZTOBSERVABLEAREA_ADD_TILE)]
    unsafe extern "thiscall" fn observable_area_add_tile_detour(this: *const u32, tile: i32) {
        observable_area_add_tile(this as u32, tile as u32);
    }

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor_detour(this: *const u32, habitat: *const std::ffi::c_void, seed_tile: *const std::ffi::c_void) -> *const u32 {
        construct(this as u32, habitat as u32, seed_tile as u32);
        this
    }

    #[detour(DESTRUCTOR)]
    unsafe extern "thiscall" fn destructor_detour(this: *const u32) {
        destruct(this as u32);
    }

    #[detour(ZTOBSERVABLEAREA_RECALCULATE_OA)]
    unsafe extern "fastcall" fn observable_area_recalculate_detour(this: *const i32) {
        recalculate_observable_area(this as u32);
    }

    #[detour(CREATE_OA)]
    unsafe extern "thiscall" fn create_oa_detour(this: *const u32) {
        create_oa(this as u32);
    }

    #[detour(HAS_OA)]
    unsafe extern "thiscall" fn has_oa_detour(this: *const u32) -> bool {
        has_oa(this as u32)
    }

    #[detour(UPDATE_AMBIENTS)]
    unsafe extern "thiscall" fn update_ambients_detour(this: *const u32, delta: i32) {
        update_ambients(this as u32, delta);
    }

    #[detour(CLEAR_TILE_OF_VA)]
    unsafe extern "thiscall" fn clear_tile_of_va_detour(this: *const u32, tile: i32) {
        clear_tile_of_va(this as u32, tile as u32);
    }

    #[detour(ADD_TILE)]
    unsafe extern "thiscall" fn add_tile_detour(this: *const u32, tile: *const std::ffi::c_void) {
        add_tile(this as u32, tile as u32);
    }

    #[detour(REMOVE_TILE)]
    unsafe extern "thiscall" fn remove_tile_detour(this: *const u32, tile: i32) {
        remove_tile(this as u32, tile as u32);
    }

    /// Real vanilla bodies for live comparison tests (`*_DETOUR.call` stays correct in release, where
    /// `.original()` on a hooked address would re-enter the detour).
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn get_ew_extent(this: u32) -> (i32, i32) {
            let (mut lo, mut hi) = (0i32, 0i32);
            unsafe { super::GET_EWEXTENT_DETOUR.call(this as *const u32, &mut lo, &mut hi) };
            (lo, hi)
        }

        pub(crate) fn get_ns_extent(this: u32) -> (i32, i32) {
            let (mut lo, mut hi) = (0i32, 0i32);
            unsafe { super::GET_NSEXTENT_DETOUR.call(this as *const u32, &mut lo, &mut hi) };
            (lo, hi)
        }

        pub(crate) fn has_oa(this: u32) -> bool {
            unsafe { super::HAS_OA_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn update_ambients(this: u32, delta: i32) {
            unsafe { super::UPDATE_AMBIENTS_DETOUR.call(this as *const u32, delta) }
        }

        pub(crate) fn recalculate_characteristics(this: u32) {
            unsafe { super::RECALCULATE_CHARACTERISTICS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn observable_area_add_tile(this: u32, tile: u32) {
            unsafe { super::ZTOBSERVABLEAREA_ADD_TILE_DETOUR.call(this as *const u32, tile as i32) }
        }

        pub(crate) fn construct(this: u32, habitat: u32, seed_tile: u32) {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this as *const u32, habitat as *const std::ffi::c_void, seed_tile as *const std::ffi::c_void) };
        }

        pub(crate) fn destruct(this: u32) {
            unsafe { super::DESTRUCTOR_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn recalculate_observable_area(this: u32) {
            unsafe { super::ZTOBSERVABLEAREA_RECALCULATE_OA_DETOUR.call(this as *const i32) }
        }

        pub(crate) fn create_oa(this: u32) {
            unsafe { super::CREATE_OA_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn clear_tile_of_va(this: u32, tile: u32) {
            unsafe { super::CLEAR_TILE_OF_VA_DETOUR.call(this as *const u32, tile as i32) }
        }

        pub(crate) fn add_tile(this: u32, tile: u32) {
            unsafe { super::ADD_TILE_DETOUR.call(this as *const u32, tile as *const std::ffi::c_void) }
        }

        pub(crate) fn remove_tile(this: u32, tile: u32) {
            unsafe { super::REMOVE_TILE_DETOUR.call(this as *const u32, tile as i32) }
        }
    }
}

pub fn init() {
    if let Err(e) = unsafe { viewing_area_detours::init_detours() } {
        error!("Failed to initialise ZTViewingArea detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        viewing_area_detours::status()
    }

    pub(crate) use viewing_area_detours::test_real;

    /// Heap-backed fake `ZTViewingArea` + tiles for read-only comparisons. Never handed to code that
    /// writes or frees through vanilla's allocator, so Rust owns the memory throughout.
    #[allow(dead_code)]
    pub(crate) struct FakeViewingArea {
        block: Box<[u32; 0x5c / 4]>,
        tiles: Vec<Box<[u32; 0x90 / 4]>>,
        tile_ptrs: Vec<u32>,
        oa_tiles: Vec<u32>,
    }

    impl FakeViewingArea {
        pub(crate) fn new(tile_coords: &[(i32, i32)], oa_tile_count: usize, dirty: u8, guests: i32, with_ambients: bool) -> Self {
            let mut tiles: Vec<Box<[u32; 0x90 / 4]>> = tile_coords
                .iter()
                .map(|&(x, y)| {
                    let mut t = Box::new([0u32; 0x90 / 4]);
                    t[TILE_OFFSET_X as usize / 4] = x as u32;
                    t[TILE_OFFSET_Y as usize / 4] = y as u32;
                    t
                })
                .collect();
            let tile_ptrs: Vec<u32> = tiles.iter_mut().map(|t| t.as_mut_ptr() as u32).collect();
            let oa_tiles = vec![0u32; oa_tile_count];
            let mut block = Box::new([0u32; 0x5c / 4]);
            if !tile_ptrs.is_empty() {
                block[OFFSET_TILES_BEGIN as usize / 4] = tile_ptrs.as_ptr() as u32;
                block[OFFSET_TILES_END as usize / 4] = tile_ptrs.as_ptr() as u32 + 4 * tile_ptrs.len() as u32;
            }
            if oa_tile_count != 0 {
                block[OFFSET_OA_TILES_BEGIN as usize / 4] = oa_tiles.as_ptr() as u32;
                block[OFFSET_OA_TILES_END as usize / 4] = oa_tiles.as_ptr() as u32 + 4 * oa_tile_count as u32;
            }
            block[OFFSET_CHARACTERISTICS_DIRTY as usize / 4] = dirty as u32;
            block[OFFSET_GUEST_COUNT as usize / 4] = guests as u32;
            if with_ambients {
                // Zeroed `Ambients`: empty group vector, so `play` is a no-op.
                let ambients: &'static mut [u32; 6] = Box::leak(Box::new([0u32; 6]));
                block[OFFSET_AMBIENTS as usize / 4] = ambients.as_ptr() as u32;
            }
            Self { block, tiles, tile_ptrs, oa_tiles }
        }

        pub(crate) fn ptr(&self) -> u32 {
            self.block.as_ptr() as u32
        }

        pub(crate) fn snapshot(&self) -> Vec<u32> {
            self.block.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attenuation_truncates_each_step_and_keeps_a_minimum_of_one() {
        assert_eq!(attenuated_weight(10, 0, 0.9), 10);
        assert_eq!(attenuated_weight(10, 1, 0.9), 9);
        assert_eq!(attenuated_weight(10, 2, 0.9), 8);
        assert_eq!(attenuated_weight(3, 1, 0.9), 2);
        assert_eq!(attenuated_weight(3, 3, 0.9), 1);
        assert_eq!(attenuated_weight(3, 40, 0.9), 1);
        assert_eq!(attenuated_weight(0, 40, 0.9), 0);
        assert_eq!(attenuated_weight(5, -2, 0.9), 5);
    }

    #[test]
    fn direction_signs_follow_the_two_direction_groups() {
        assert_eq!(direction_signs(0), (1, -1));
        assert_eq!(direction_signs(1), (1, 1));
        assert_eq!(direction_signs(2), (-1, 1));
        assert_eq!(direction_signs(3), (-1, 1));
        assert_eq!(direction_signs(4), (-1, -1));
        assert_eq!(direction_signs(7), (1, -1));
    }

    #[test]
    fn observation_range_is_ten_tiles() {
        assert!(within_observation_range(100));
        assert!(!within_observation_range(101));
        assert!(!within_observation_range(i32::MAX));
    }

    #[test]
    fn extent_folds_min_and_max() {
        assert_eq!(extent_of([5, 2, 9, 2]), Some((2, 9)));
        assert_eq!(extent_of([-3]), Some((-3, -3)));
        assert_eq!(extent_of([]), None);
    }

    #[test]
    fn oa_vector_nonempty_checks_whole_entries() {
        assert!(!oa_vector_nonempty(0x1000, 0x1000));
        assert!(oa_vector_nonempty(0x1000, 0x1004));
        assert!(!oa_vector_nonempty(0x1000, 0x1003));
    }
}
