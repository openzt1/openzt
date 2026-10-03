use openzt_detour::{
    generated::{
        bfentity::SET_WORLD_POS as BFENTITY_SET_WORLD_POS,
        bfmap::WORLD_TO_VIRTUAL_0,
        bfsndmgr::ACQUIRE as BFSNDMGR_ACQUIRE,
        msvc_std_listuint::INSERT as MSVC_LIST_UINT_INSERT,
        msvc_std_mapint_float::{
            CLEAR_0 as MSVC_MAP_INT_FLOAT_CLEAR, OPERATOR_INDEX as MSVC_MAP_INT_FLOAT_OPERATOR_INDEX, TREE as MSVC_MAP_INT_FLOAT_TREE,
        },
        msvc_std_mapint_habitatsuitability::{
            OPERATOR_ASSIGN as MSVC_MAP_INT_HABITATSUITABILITY_OPERATOR_ASSIGN,
            OPERATOR_INDEX as MSVC_MAP_INT_HABITATSUITABILITY_OPERATOR_INDEX, TREE as MSVC_MAP_INT_HABITATSUITABILITY_TREE,
            TREE_DTOR as MSVC_MAP_INT_HABITATSUITABILITY_TREE_DTOR,
        },
        poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
        standalone::{OPERATOR_DELETE, OPERATOR_NEW, WRITE_BYTES_TO_FILE},
        zthabitat::GET_SPECIES_RATING,
    },
};
use std::{
    collections::HashMap,
    mem,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

use crate::{
    globals::get_module_base,
    util::{get_from_memory, save_to_memory},
    ztmapview::BFTile,
    ztmegatilemgr::entity_type_matches,
    ztshow::call_entity_vtable_noargs,
    ztworldmgr::{Direction, IVec3, ZTWorldMgr},
};
use super::habitat::ZTHabitat;
use super::mgr::zthabitatmgr::ZTHabitatMgr;

/// `DAT_00639144`'s RVA - a shared "habitat list changed" dirty byte flag, set by
/// `ZTHabitatMgr::addHabitat`/`removeHabitat_0`/`removeHabitat_1`/`nameLoadedHabitats` and cleared by
/// `ZTUI::habitatinfo::update` once it's refreshed the habitat list UI off the back of it (confirmed via
/// `habitatinfo_update.c`). RVA = `0x00639144 - 0x400000`.
pub const HABITAT_LIST_DIRTY_RVA: u32 = 0x0023_9144;

/// Shared "impassable/too expensive" path-cost sentinel (`DAT_00635494`) - read (never written) across
/// this decompile corpus's entire `getPathCost`/`getTerrainCost` family (`BFUnit::getPathCost`,
/// `ZTGuest`/`ZTGuide`/`ZTStaff::getTerrainCost`, `BFPathFinder::cost`), not specific to habitats -
/// [`ZTHabitatMgr::check_enter_habitat`] is just the first port in this codebase to need it, and
/// [`ZTHabitat::get_random_clear_tile_ahead`] the second (its "cheap enough to enter" bound).
/// `pub(crate)` for the directional-tile live tests, which rebuild the port's expected candidate set
/// from the same bound. RVA = `0x00635494 - 0x400000`.
pub const MAX_PATH_COST_RVA: u32 = 0x0023_5494;

/// `isCastClass` type-tag constant for `ZTStaff` (`&CAST_ZTStaff`, `0x00638710`) -
/// `ZTHabitatMgr::clearStaffHabitat`'s per-entity gate before it calls
/// `ZTStaff::removeAssignedHabitat` on an `entity_array` entry. Sits between
/// [`RVA_FENCE_TYPE_CHECK_ARG`] (`0x638660`) and [`RVA_TANK_WALL_TYPE_CHECK_ARG`] (`0x638720`) in the
/// same class-tag table. RVA = `0x00638710 - 0x400000`.
pub const RVA_STAFF_TYPE_CHECK_ARG: u32 = 0x0023_8710;

/// `DAT_00638588`'s RVA - a single byte `ZTHabitatMgr::terrainChanged` reads to gate its entire body
/// (`if (DAT_00638588 == '\0') { ... }`). Per `species-rating-cache-identification-handover.md`, this
/// reads as a general pause/dialog-blocking flag, not specific to the species-rating cache - not
/// independently confirmed against any other reader, since nothing else in this codebase references this
/// address yet. RVA = `0x00638588 - 0x400000`.
pub const RVA_GAME_PAUSED_FLAG: u32 = 0x0023_8588;

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
pub unsafe fn get_species_rating(habitat_ptr: u32, species_key: i32) -> f32 {
    let real_fn: unsafe extern "thiscall" fn(*const u32, i32) -> f32 = unsafe { mem::transmute(GET_SPECIES_RATING.address) };
    unsafe { real_fn(habitat_ptr as *const u32, species_key) }
}

/// One entry of the species-rating cache `ZTHabitatMgr::terrainAboutToBeChanged`/`terrainChanged` share -
/// see [`SPECIES_RATING_CACHE`]'s own doc comment.
pub struct SpeciesRatingCacheEntry {
    pub habitat_ptr: u32,
    pub ratings: HashMap<i32, f32>,
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
pub static SPECIES_RATING_CACHE: LazyLock<Mutex<Vec<SpeciesRatingCacheEntry>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Locks [`SPECIES_RATING_CACHE`], recovering the data from a poisoned lock rather than panicking
/// inside a detour. Callers must not hold the guard across a call into vanilla code.
pub fn lock_species_rating_cache() -> MutexGuard<'static, Vec<SpeciesRatingCacheEntry>> {
    SPECIES_RATING_CACHE.lock().unwrap_or_else(PoisonError::into_inner)
}


/// Base of vanilla's shared small-object freelist bucket array, bucketed by `(byte_capacity - 1) >> 3` -
/// the same `DAT_00638000` family `ambients.rs`'s `RVA_GROUP_ARRAY_FREELIST_BUCKETS` already documents
/// and live-tests (`Ambients_~Ambients.asm`'s `SUB EAX,1; SAR EAX,3; MOV EAX,[EAX*4+0x638000]; ...` per-
/// bucket singly-linked push), confirmed independently here by `ZTHabitat_listen.c`'s own tail (`uVar1 =
/// uVar1 - 1 >> 3; *local_c = DAT_00638000[uVar1]; DAT_00638000[uVar1] = local_c;`) - the identical
/// pattern. RVA = `0x00638000 - 0x400000`.
pub const RVA_EVENT_VECTOR_FREELIST_BUCKETS: u32 = 0x0023_8000;

/// One node of the generic small-object circular doubly-linked list container `ZTHabitat::owned_tiles_ptr`
/// (`+0x40`) points to as a sentinel - the same node/sentinel shape `BFTile`'s own `+0x0` occupant list
/// uses (see `ZTHabitat::reset_unit_ai`). Confirmed directly against
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
pub struct TileListNode {
    pub next: u32,    // 0x0
    pub _prev: u32,   // 0x4
    pub payload: u32, // 0x8 - BFTile*
    pub _unused: u32, // 0xc
}

const _: () = assert!(std::mem::size_of::<TileListNode>() == 0x10);

/// Bucket 1 (16-byte allocations) of the shared small-object freelist array this file's own
/// [`RVA_EVENT_VECTOR_FREELIST_BUCKETS`] documents - [`TileListNode`]s are freed back to this exact
/// bucket. Confirmed directly via `.asm`: `BFTile::cls_0x40143b`'s freelist-reuse fast path pops from
/// `DAT_00638004`, which is `RVA_EVENT_VECTOR_FREELIST_BUCKETS + 4` - matching `(0x10 - 1) >> 3 == 1`,
/// the same bucketing formula `ambients.rs`'s `freelist_bucket_index` documents for this identical
/// bucket-array family.
pub const TILE_LIST_NODE_FREELIST_HEAD_RVA: u32 = RVA_EVENT_VECTOR_FREELIST_BUCKETS + 4;

/// `GLOBAL_ZTApp`'s RVA - see `ztgamemgr.rs`'s own `GLOBAL_ZTAPP_RVA` doc comment for the shared
/// one-level-of-indirection shape (`MOV EAX, GLOBAL_ZTApp` / `MOV EAX, appInitSuccess` resolving to the
/// same live `ZTApp*` value) and why the real body's "if null, lazily assign a bogus function-pointer
/// sentinel" branch is dead in practice and not reproduced here. Re-declared per this repo's own
/// per-file convention (see `ztshowmgr.rs`'s own copy) - used by [`ZTHabitat::reset_unit_ai`].
pub const GLOBAL_ZTAPP_RVA: u32 = 0x00638154 - 0x400000;

/// The shared game RNG state (`DAT_00638060`) [`ZTHabitat::update`] rerolls its two lazy-recalculate
/// timers through - the same dword LCG state `ztsoundscape.rs`'s own `GAME_RNG_RVA`/`lcg_next`
/// document and advance for their own, unrelated purpose (ambient position jitter); duplicated locally
/// per this codebase's existing per-file convention (see e.g. this file's own `GLOBAL_DX8SNDMGR_RVA`).
pub const GAME_RNG_RVA: u32 = 0x00638060 - 0x400000;

/// One MSVC LCG advance over the shared game RNG state: `state = state * 0x343fd + 0x269ec3` with full
/// 32-bit wrap - see `ztsoundscape.rs`'s own `lcg_next` for the identical formula/derivation.
pub fn lcg_next(state: u32) -> u32 {
    state.wrapping_mul(0x343fd).wrapping_add(0x269ec3)
}

/// The per-habitat field rotation [`ZTHabitatMgr::enter_new_month`] applies identically to every
/// `exhibit_array` entry and to [`ZTHabitatMgr::pending_habitat_ptr`]: `current_donations` (`+0xfc`) ->
/// `last_donations` (`+0x100`), `unknown_u32_2` (`+0x114`) -> `unknown_u32_3` (`+0x118`), and
/// `current_upkeep` (`+0x108`) -> `last_upkeep` (`+0x10c`), zeroing each leading field. Operates on a raw
/// address rather than a typed `&ZTHabitat`/`&mut ZTHabitat` since real vanilla's own body never
/// distinguishes `ZTHabitat` from `ZTTankExhibit` here - both share the same base-class field layout this
/// touches.
pub fn rotate_month_fields(habitat_ptr: u32) {
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

/// The raw bytes of a live entity's `name` string (`BFEntity+0x108` begin / `+0x10c` end, the
/// `ZTBufferString` `ztworld/entity.rs` maps). Comparing two of these with Rust's `[u8]` ordering is
/// exactly vanilla's name comparator at `0x004690cd` (`std::string operator<`: unsigned byte-wise over
/// the shorter length, then shorter-first) - see [`ZTHabitat::get_all_animals`].
pub fn entity_name_bytes(entity_ptr: u32) -> Vec<u8> {
    let begin: u32 = get_from_memory(entity_ptr + 0x108);
    let end: u32 = get_from_memory(entity_ptr + 0x10c);
    (begin..end).map(get_from_memory::<u8>).collect()
}

/// Walks a `ZTHabitat::owned_tiles_ptr`-shaped sentinel field's real, live value (`sentinel_addr` - the
/// heap-allocated sentinel node's own address, i.e. the container field's *value*, not its address),
/// yielding each real node's address in order and never the sentinel itself. Matches every corroborating
/// walker's own idiom exactly (`next` lives at the node's own offset `0x0`) - see [`TileListNode`]'s doc
/// comment. An empty list (`sentinel_addr`'s own `next` pointing back to itself) yields nothing.
pub fn walk_tile_list(sentinel_addr: u32) -> impl Iterator<Item = u32> {
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
pub fn walk_neighbor_tree(head_ptr: u32) -> impl Iterator<Item = u32> {
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

/// Size of a `std::set<ZTHabitat*>` node (`color`, `parent`, `left`, `right`, `key`), allocated from
/// `PoolAlloc`'s size-class-2 freelist.
const RB_SET_NODE_SIZE: u32 = 0x14;
const RB_RED: u8 = 0;
const RB_BLACK: u8 = 1;

fn rb_parent(node: u32) -> u32 {
    get_from_memory(node + 0x4)
}
fn rb_left(node: u32) -> u32 {
    get_from_memory(node + 0x8)
}
fn rb_right(node: u32) -> u32 {
    get_from_memory(node + 0xc)
}
fn rb_key(node: u32) -> u32 {
    get_from_memory(node + 0x10)
}
fn rb_color(node: u32) -> u8 {
    get_from_memory(node)
}

/// `_Lrotate` (`0x00403347`): `root_addr` is the address of the head's `_Parent` slot.
fn rb_rotate_left(x: u32, root_addr: u32) {
    let y = rb_right(x);
    save_to_memory(x + 0xc, rb_left(y));
    if rb_left(y) != 0 {
        save_to_memory(rb_left(y) + 0x4, x);
    }
    save_to_memory(y + 0x4, rb_parent(x));
    if x == get_from_memory::<u32>(root_addr) {
        save_to_memory(root_addr, y);
    } else if x == rb_left(rb_parent(x)) {
        save_to_memory(rb_parent(x) + 0x8, y);
    } else {
        save_to_memory(rb_parent(x) + 0xc, y);
    }
    save_to_memory(y + 0x8, x);
    save_to_memory(x + 0x4, y);
}

/// `_Rrotate` (`0x0040338e`), mirror of [`rb_rotate_left`].
fn rb_rotate_right(x: u32, root_addr: u32) {
    let y = rb_left(x);
    save_to_memory(x + 0x8, rb_right(y));
    if rb_right(y) != 0 {
        save_to_memory(rb_right(y) + 0x4, x);
    }
    save_to_memory(y + 0x4, rb_parent(x));
    if x == get_from_memory::<u32>(root_addr) {
        save_to_memory(root_addr, y);
    } else if x == rb_right(rb_parent(x)) {
        save_to_memory(rb_parent(x) + 0xc, y);
    } else {
        save_to_memory(rb_parent(x) + 0x8, y);
    }
    save_to_memory(y + 0xc, x);
    save_to_memory(x + 0x4, y);
}

/// Iterator decrement (`FUN_00403103`), including the `end()` (header) case.
fn rb_decrement(node: u32) -> u32 {
    if rb_color(node) == RB_RED && rb_parent(rb_parent(node)) == node {
        return rb_right(node);
    }
    let left = rb_left(node);
    if left == 0 {
        let mut parent = rb_parent(node);
        if node == rb_left(parent) {
            let mut cursor = parent;
            loop {
                parent = rb_parent(cursor);
                let was_left = cursor == rb_left(parent);
                cursor = parent;
                if !was_left {
                    break;
                }
            }
        }
        parent
    } else {
        let mut result = left;
        let mut next = rb_right(result);
        while next != 0 {
            result = next;
            next = rb_right(next);
        }
        result
    }
}

/// Links a freshly `PoolAlloc`-allocated node holding `key` under `parent` and rebalances - the body of
/// `msvc_std::tree::meth_0x4fbeee` (`_Tree::_Insert` + inlined `_Insert_fixup`) with `force_left == 0`.
fn rb_set_insert_node(container: u32, parent: u32, key: u32) {
    let head: u32 = get_from_memory(container);
    let root_addr = head + 0x4;
    let node = unsafe { POOLALLOC_ALLOCATE.original()(RB_SET_NODE_SIZE) } as u32;
    if node != 0 {
        save_to_memory(node + 0x10, key);
    }

    if parent == head || key < rb_key(parent) {
        save_to_memory(parent + 0x8, node);
        if parent == head {
            save_to_memory(head + 0x4, node);
            save_to_memory(head + 0xc, node);
        } else if parent == rb_left(head) {
            save_to_memory(head + 0x8, node);
        }
    } else {
        save_to_memory(parent + 0xc, node);
        if parent == rb_right(head) {
            save_to_memory(head + 0xc, node);
        }
    }
    save_to_memory(node + 0x4, parent);
    save_to_memory(node + 0x8, 0u32);
    save_to_memory(node + 0xc, 0u32);
    save_to_memory(node, RB_RED);

    let mut x = node;
    while x != get_from_memory::<u32>(root_addr) {
        let p = rb_parent(x);
        if rb_color(p) != RB_RED {
            break;
        }
        let grandparent = rb_parent(p);
        let p_is_left = p == rb_left(grandparent);
        let uncle = if p_is_left { rb_right(grandparent) } else { rb_left(grandparent) };
        if uncle == 0 || rb_color(uncle) != RB_RED {
            let inner = if p_is_left { x == rb_right(p) } else { x == rb_left(p) };
            if inner {
                if p_is_left {
                    rb_rotate_left(p, root_addr);
                } else {
                    rb_rotate_right(p, root_addr);
                }
                x = p;
            }
            save_to_memory(rb_parent(x), RB_BLACK);
            let gp = rb_parent(rb_parent(x));
            save_to_memory(gp, RB_RED);
            if p_is_left {
                rb_rotate_right(gp, root_addr);
            } else {
                rb_rotate_left(gp, root_addr);
            }
        } else {
            save_to_memory(p, RB_BLACK);
            save_to_memory(uncle, RB_BLACK);
            save_to_memory(rb_parent(p), RB_RED);
            x = rb_parent(p);
        }
    }
    save_to_memory(get_from_memory::<u32>(root_addr), RB_BLACK);
    save_to_memory(container + 0x4, get_from_memory::<u32>(container + 0x4) + 1);
}

/// `std::set<ptr>::insert` over a vanilla-layout set whose `{head*, size}` pair lives at `container`
/// (`ZTHabitat+0x8` amphibious neighbours, `+0x14` show neighbours). Returns whether `key` was newly
/// inserted. Matches `addAmphibiousNeighbor`'s inline find-then-`_Insert` (and `0x005b355e` for the show
/// set): descend to the would-be parent, and for a left-going leaf that isn't the leftmost node compare
/// against its in-order predecessor before inserting.
pub fn rb_set_insert(container: u32, key: u32) -> bool {
    let head: u32 = get_from_memory(container);
    let mut parent = head;
    let mut went_left = true;
    let mut node = rb_parent(head);
    while node != 0 {
        parent = node;
        went_left = key < rb_key(node);
        node = if went_left { rb_left(node) } else { rb_right(node) };
    }
    let mut predecessor = parent;
    if went_left {
        if parent == rb_left(head) {
            rb_set_insert_node(container, parent, key);
            return true;
        }
        predecessor = rb_decrement(parent);
    }
    if rb_key(predecessor) < key {
        rb_set_insert_node(container, parent, key);
        true
    } else {
        false
    }
}

/// Frees `node`'s whole subtree: right subtree recursively, then the node, then loops down the left
/// spine - `msvc_std::tree::meth_0x49304a`'s shape. `node_size` is the pool size class the nodes came
/// from (`0x14` for the sets, `0x18` for the show-portal map; both land in the same bucket).
pub fn rb_erase_subtree(mut node: u32, node_size: u32) {
    while node != 0 {
        rb_erase_subtree(rb_right(node), node_size);
        let left = rb_left(node);
        unsafe { POOLALLOC_DEALLOCATE.original()(node as *const u32, node_size) };
        node = left;
    }
}

/// `clear()` of a vanilla-layout tree container at `container` (`{head*, size}`): a no-op when
/// `size == 0`, else frees every node back to `PoolAlloc` and resets the head to the empty shape.
pub fn rb_tree_clear(container: u32, node_size: u32) {
    if get_from_memory::<u32>(container + 0x4) == 0 {
        return;
    }
    let head: u32 = get_from_memory(container);
    rb_erase_subtree(rb_parent(head), node_size);
    save_to_memory(head + 0x8, head);
    save_to_memory(head + 0x4, 0u32);
    save_to_memory(head + 0xc, head);
    save_to_memory(container + 0x4, 0u32);
}

/// Calls vtable slot `+0x100` on every occupant of `tile` (a live `BFTile*`) - the inner walk
/// `ZTHabitat::reset_unit_ai` performs for each owned tile and for the gate-out tile. `tile+0x0`
/// (`BFTile::unit_list_ptr`) is a [`TileListNode`] sentinel of the exact same shape/pool as
/// `ZTHabitat::owned_tiles_ptr`, confirmed directly via `.asm` - see that field's own doc comment.
pub fn reset_unit_ai_for_tile_occupants(tile: u32) {
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
pub const RVA_FENCE_TYPE_CHECK_ARG: u32 = 0x0023_8660;

/// `isCastClass` type-tag constant (`&DAT_00638720`) used alongside tank-specific checks throughout the
/// decompile corpus (`ZTTankExhibit_updateTankInfo.c`, `_setIsShowExhibit.c`, `_addWaterRipples.c`,
/// `_updateAdjustmentCosts.c`) - most likely `ZTTankWallType`'s own tag, distinguishing a tank wall from
/// the broader fence/wall family [`RVA_FENCE_TYPE_CHECK_ARG`] already covers. Not independently confirmed
/// against a named symbol (no vtable/mac-lookup evidence for this specific address), only inferred from
/// convergent call-site context - see [`ZTHabitatMgr::replace_fence_with_gate`]'s own use. RVA =
/// `0x00638720 - 0x400000`.
pub const RVA_TANK_WALL_TYPE_CHECK_ARG: u32 = 0x0023_8720;

/// `isCastClass` type-tag constant for `ZTKeeper` - `ZTHabitat::blockService`'s own leading guard uses it
/// to confirm its `ZTStaff*` argument is really a `ZTKeeper*` before doing anything else (the real
/// decompile calls it `CAST_ZTKeeper`, matching `ZTKeeper::cleansUp`'s own identical self-check). Not
/// independently confirmed against a named symbol - `ZTKeeperType_isClass.c`'s own
/// `DAT_00638764 - CAST_ZTKeeper` byte-range read places it at `0x00638760`, which lands exactly on the
/// same `0x10`-spaced isCastClass tag table [`RVA_STAFF_TYPE_CHECK_ARG`]/[`RVA_TANK_WALL_TYPE_CHECK_ARG`]
/// already document (`...0x638750, 0x638760, 0x638770...`, consistent spacing on both sides) - same
/// caveat as those two. RVA = `0x00638760 - 0x400000`.
pub const RVA_KEEPER_TYPE_CHECK_ARG: u32 = 0x0023_8760;

/// `isCastClass` type-tag constant for `ZTBuilding` (`&CAST_ZTBuilding`, `0x00638680`) - confirmed
/// directly via `ZTHabitat_addToBuildingList.asm`'s own `PUSH CAST_ZTBuilding; CALL [vtable+0x1c]`
/// dispatch on an owned tile's occupant entity type ([`entity_type_matches`]'s exact shape). Sits between
/// [`crate::ztmegatilemgr::RVA_SCENERY_TYPE_CHECK_ARG`] (`0x638670`) and [`RVA_STAFF_TYPE_CHECK_ARG`]
/// (`0x638710`) in the same `0x10`-spaced isCastClass tag table those two document. RVA =
/// `0x00638680 - 0x400000`.
pub const RVA_BUILDING_TYPE_CHECK_ARG: u32 = 0x0023_8680;

/// A `ZTHabitatSuitabilityRecord` (the real vanilla `msvc_std::map<int, ZTHabitatSuitabilityRecord>`'s
/// per-key payload) is `0x70` bytes, confirmed via `msvc_std_mapint_habitatsuitability::TREE`'s own node
/// allocation (`operator_new(0x84)`, tree node header `0x14` bytes, `0x84 - 0x14 = 0x70`). Field offsets
/// are read directly off `ZTHabitatSuitabilityRecord_ZTHabitatSuitabilityRecord.asm`'s own zero-init write
/// sequence - one dword/byte-run write per instruction, in the exact address order written (`0x0, 0x4,
/// 0x8, 0xc, 0x38, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x34, 0x3c, 0x40, 0x44, 0x48,
/// 0x4c, 0x50, 0x54`, then a 22-byte flag pack at `0x58..0x6e`) - **not** assumed from the `.c`
/// decompile's field-declaration order, which does not track true layout for a couple of these fields
/// (its own `unk_0x28`/`unk_0x2c`/`unk_0x3c`/`unk_0x40`/`unk_0x44` names do match their real offsets,
/// confirming this address-order reading is correct, but its `sum_category_tally` name is attached to the
/// field really at `0x38`, not `0x10`).
/// [`crate::zthabitat::habitat::ZTHabitat::additional_scenery_suitability_change`] only touches
/// `+0x10` (running `f32` suitability score accumulator), `+0x14` (`i32` count of owned tiles with at
/// least one matching, flagged scenery occupant) and `+0x1c` (`i32` count of matching scenery occupants
/// whose own `+0x12b` flag byte is set) - read/written directly as raw offsets off
/// [`map_int_habitatsuitability_find_or_insert`]'s returned record address, no dedicated struct needed.
///
/// Finds (or default-inserts) the record for `key` in the real vanilla
/// `msvc_std::map<int, ZTHabitatSuitabilityRecord>` at `map_ptr`, returning the record's own address
/// (`node+0x14`). Delegates the whole find-or-insert to real vanilla's own
/// `msvc_std_mapint_habitatsuitability::OPERATOR_INDEX` (`map::operator[]`) rather than hand-rolling the
/// manual lower-bound-walk-then-`INSERT_WRAPPER`-on-miss dance `ZTHabitat_additionalScenerySuitabilityChange.c`'s
/// own body performs inline: a hand-rolled version of that dance crashed inside vanilla's own generic
/// tree-insert helper (`FUN_00403103`, confirmed live via `crash-capture` - `cmp byte ptr [edx],0` on a
/// null `edx`) on its very first real insert, despite the argument mapping matching the decompile
/// byte-for-byte; `OPERATOR_INDEX` performs the identical `insert_wrapper(this, &out, 0, &{key,record})`
/// sequence internally (confirmed via its own decompile) but is real vanilla's own single entry point for
/// it, used throughout the game for countless other `map<K,V>::operator[]` call sites, so it carries far
/// more real-world exercise than any hand assembly of the same steps.
pub fn map_int_habitatsuitability_find_or_insert(map_ptr: u32, key: i32) -> u32 {
    (unsafe { MSVC_MAP_INT_HABITATSUITABILITY_OPERATOR_INDEX.original()(map_ptr as *const i32, &key as *const i32) }) as u32
}

/// Constructs a real vanilla `msvc_std::map<int, ZTHabitatSuitabilityRecord>` header in place at
/// `header_ptr` - the same function-local scratch tree `ZTHabitat::recalculateCharacteristics` builds as
/// a stack local at the very top of its own body (`msvc_std::map<int_habitatsuitability>::Tree`,
/// `generated.rs`'s `msvc_std_mapint_habitatsuitability::TREE`, live-decompiled to confirm the exact
/// writes below). `header_ptr` needs at least 12 bytes (a `[u32; 4]`/16-byte caller-owned buffer is the
/// established convention - see [`destroy_suitability_scratch_tree`]'s own caller); real vanilla itself
/// keeps this as a stack local for the duration of a single `recalculateCharacteristics` call, so a
/// Rust stack-local array works identically - the header itself is never touched by any other, un-ported
/// vanilla code, only [`map_int_habitatsuitability_find_or_insert`]'s own node allocations (which go
/// through real vanilla's `operator_new`/allocator) need to stay vanilla-shaped.
///
/// The real ctor's own body: allocates one sentinel tree node (`operator_new(0x84)`, matching
/// `map_int_habitatsuitability_find_or_insert`'s own node-size derivation) whose `color`/`parent`/
/// `left`/`right` fields are zeroed/self-referenced (an empty-tree sentinel, same shape as
/// `amphibious_neighbors_head`'s own family), sets `header+0x0` (`_Myhead`) to that sentinel and
/// `header+0x4` (`_Mysize`) to `0`, then copies its own second parameter (the comparator/allocator
/// functor) into `header+0x8` - dead in practice, since `std::less<int>`/its allocator are both
/// stateless; the ctor never reads its *third* parameter at all. Both extra arguments are passed as
/// plain readable stack addresses here for that reason - their contents are never meaningfully used.
pub fn init_suitability_scratch_tree(header_ptr: u32) {
    let dummy_comparator: i32 = 0;
    let dummy_allocator: i8 = 0;
    unsafe {
        MSVC_MAP_INT_HABITATSUITABILITY_TREE.original()(
            header_ptr as *const i32,
            &dummy_comparator as *const i32,
            &dummy_allocator as *const i8,
        );
    }
}

/// Tears down a tree built by [`init_suitability_scratch_tree`] (or found in `ZTHabitat::
/// speciesSuitabilityCache`), freeing the sentinel and every inserted node through real vanilla's own
/// allocator. These nodes were allocated by real vanilla's `operator_new`/`PoolAlloc`, never by `Box` -
/// see `AGENTS.md`'s cross-allocator safety rules for why they must be freed this way, never by a Rust
/// `Box`-walking teardown.
pub fn destroy_suitability_scratch_tree(header_ptr: u32) {
    unsafe { MSVC_MAP_INT_HABITATSUITABILITY_TREE_DTOR.original()(header_ptr as *const i32) };
}

/// Copies the scratch suitability tree at `source_header_ptr` into the tree header at `dest_header_ptr`
/// through real vanilla's own `map::operator=` (`0x00446370`), which clears the destination and
/// allocates the copied nodes with vanilla's allocator - the only safe way to fill a live habitat's own
/// `species_suitability_cache` (`+0x148`), whose nodes un-ported vanilla code later frees.
pub fn assign_suitability_tree(dest_header_ptr: u32, source_header_ptr: u32) {
    unsafe { MSVC_MAP_INT_HABITATSUITABILITY_OPERATOR_ASSIGN.original()(dest_header_ptr as *const i32, source_header_ptr as *const i32) };
}

/// Finds (or default-inserts, `0.0`) the value for `key` in a real vanilla `msvc_std::map<int, float>` at
/// `map_ptr`, returning the value's own address - real vanilla's `OPERATOR_INDEX`
/// (`msvc_std_mapint_float::OPERATOR_INDEX`), the same "don't hand-roll the find/insert dance" precedent
/// [`map_int_habitatsuitability_find_or_insert`]'s own doc comment already established (same shared
/// `FIND`/`ITERATOR_ASSIGN` navigation primitives underneath - `generated.rs`'s `msvc_std_tree` module -
/// confirmed via disassembly to be reused, value-type-agnostic, across both map instantiations).
///
/// `ZTHabitat::recalculateCharacteristics` builds two of these (`local_c30`/`local_bc0`, the plan's own
/// Stage 7 "five distinct passes" breakdown step (3)) but - per that same breakdown's step (5) - they are
/// only ever read/written later, by `constructSurroundingSpeciesList`'s own follow-up walk; step (4)'s
/// `GET_SURROUNDING_ANIMALS` scan (confirmed via disassembly this session) does not touch either despite
/// initially looking like it might.
pub fn map_int_float_find_or_insert(map_ptr: u32, key: i32) -> u32 {
    (unsafe { MSVC_MAP_INT_FLOAT_OPERATOR_INDEX.original()(map_ptr as *const std::ffi::c_void, &key as *const i32) }) as u32
}

/// Constructs a real vanilla `msvc_std::map<int, float>` header in place at `header_ptr` - same
/// stack-local-buffer convention as [`init_suitability_scratch_tree`] (a `[u32; 4]`/16-byte caller-owned
/// buffer), confirmed via disassembly at `0x004452ab`-`0x004452d2` (real vanilla constructs two of these
/// back-to-back, `local_c30` then `local_bc0`, right after step (2)'s own `local_c1c`/`local_c10` arrays).
/// Unlike the habitat-suitability tree's own 3-argument ctor, `msvc_std_mapint_float::TREE` takes only one
/// extra argument (the comparator/allocator functor, `std::less<int>` - stateless, dead in practice, same
/// caveat as `init_suitability_scratch_tree`'s own dummy arguments).
///
/// Tear it down with [`clear_float_scratch_tree`].
pub fn init_float_scratch_tree(header_ptr: u32) {
    // `msvc_std_mapint_float::TREE` (`0x00404b91`) ends in `RET 8`: two stack arguments, both dead. The
    // `generated.rs` entry declares one, so the address is called with its real shape here.
    let dummy_a: i8 = 0;
    let dummy_b: i8 = 0;
    let ctor: unsafe extern "thiscall" fn(*const std::ffi::c_void, *const i8, *const i8) -> *const u32 =
        unsafe { std::mem::transmute(MSVC_MAP_INT_FLOAT_TREE.address) };
    unsafe { ctor(header_ptr as *const std::ffi::c_void, &dummy_a, &dummy_b) };
}

/// Frees every node of a tree built by [`init_float_scratch_tree`] through real vanilla's own
/// `map<int, float>::clear` (`0x0041e7f6`), exactly as `recalculateCharacteristics`'s own teardown does.
/// Like vanilla, this leaves the tree's sentinel node allocated.
pub fn clear_float_scratch_tree(header_ptr: u32) {
    unsafe { MSVC_MAP_INT_FLOAT_CLEAR.original()(header_ptr as *const std::ffi::c_void) };
}

/// `DAT_006393c8`'s RVA - a shared, stateless scratch dword `ZTHabitat::recalculateCharacteristics`
/// stashes the *current* animal's species id into immediately before scanning its own found-species
/// list via `ZTSpecies::isSpecialDummySpecies` (`generated.rs`'s `ztspecies::IS_SPECIAL_DUMMY_SPECIES`).
/// That function's real body is just `*(int*)(param+0x1ec) == DAT_006393c8` - real vanilla repurposes
/// it here as a plain "does this candidate's species id equal the stashed one" equality check, not a
/// genuine dummy-species filter (confirmed via its own decompile: no other logic in the function).
/// Callers must write the current species id here (`save_to_memory`) before calling it for this
/// purpose. RVA = `0x006393c8 - 0x400000`.
pub const RVA_CURRENT_SPECIES_ID_STASH: u32 = 0x0023_93c8;

/// `DAT_006393c0`'s RVA - the shared rate-limit counter `ZTHabitat::checkEscapability` increments on
/// every call, resetting it (and doing real work) once it reaches `0x1e` (30) or `unknown_flag_0x30` is
/// already set. Not per-habitat - every habitat's `checkEscapability` call shares this one global dword,
/// matching real vanilla exactly (`ZTHabitat_checkEscapability.asm`'s own `DAT_006393c0` reference). RVA
/// = `0x006393c0 - 0x400000`.
pub const RVA_CHECK_ESCAPABILITY_COUNTER: u32 = 0x0023_93c0;

/// `isCastClass`-style type-check argument for the `ZTFood` gate in `getNumKeeperFoodTiles`
/// (`&DAT_006386c0` in the decompile; the macOS build names the same call site
/// `BFEntityCastConst<ZTFood>` - attribution the sibling scenery/animal/guest/keeper tags lack). Sits in
/// the same tag-table region as [`RVA_KEEPER_TYPE_CHECK_ARG`]
/// (`0x00638660`/`0x00638670`/`0x006386c0`/`0x00638760`). RVA = `0x006386c0 - 0x400000`.
pub const RVA_ZTFOOD_TYPE_CHECK_ARG: u32 = 0x0023_86c0;

/// `DAT_00639148`'s RVA - a shared "gate placement/conversion in progress" flag, set to `1` for the
/// duration of `ZTHabitatMgr::replaceGateWithFence`/`replaceFenceWithGate` and read as an early-return
/// guard by `ZTHabitatMgr::fencePlaced`/`fenceRemoved` (`ZTHabitatMgr::fence_placed`/`fence_removed`) to suppress their own
/// reaction while a gate conversion is already underway. RVA = `0x00639148 - 0x400000`.
pub const GATE_CONVERSION_IN_PROGRESS_RVA: u32 = 0x0023_9148;

/// `DAT_00638084`'s RVA - a fixed, null-terminated debug/undo-action-description string
/// `ZTHabitatMgr::createDoubleFence` copies (via a real `PoolAlloc::allocate`/`memmove`, matching
/// `ZTHabitatMgr_createDoubleFence.asm`'s own byte-for-byte sequence) into a scratch buffer handed to
/// `ZTMapView::addUndoAction`. Content not read/interpreted by this port - only its length and address
/// matter. RVA = `0x00638084 - 0x400000`.
pub const RVA_CREATE_DOUBLE_FENCE_UNDO_LABEL: u32 = 0x0023_8084;

/// `DAT_00635494`'s RVA - a fixed scalar "unreachable/no candidate" path-cost sentinel, read directly
/// (never behind a `GLOBAL_ZTApp`-style lazy-init check) by pathfinding-adjacent code throughout the
/// decompile corpus (`BFUnit::getTerrainCost`/`getEdgeCost`/`getPathCost`, `BFMap::validate`,
/// `ZTAnimal::getTerrainCost`, `BFAIMgr::fRandomWalk`) - always used as either a returned "max cost" or a
/// `<`/`<=` comparison bound, never dereferenced as a pointer despite `ZTHabitatMgr_placeGate.c`'s own
/// `BFTile *` mistyping of it. [`ZTHabitatMgr::place_gate`] reuses it identically: as the initial "no
/// candidate found yet" sentinel for both its own best-distance tracker and `checkGate`'s own out-distance
/// parameter. RVA = `0x00635494 - 0x400000`.
pub const RVA_INFINITE_PATH_COST: u32 = 0x0023_5494;

/// `GLOBAL_ZTApp`'s RVA (same singleton/address `ztgamemgr.rs::GLOBAL_ZTAPP_RVA`/
/// `ztshowmgr.rs::GLOBAL_ZTAPP_RVA` already document independently - not shared as a single constant
/// since each file's own port predates the others). [`ZTHabitatMgr::place_gate`] reads its own `+0x441`
/// byte field (`ZTUI::gameopts::loadInProgress`'s backing store, confirmed via `gameopts_loadInProgress.c`'s
/// clean `return DAT_00638589;` body and `gameopts_loadFile.c`'s own `DAT_00638589 = 1;` write - one byte
/// past `ztgamemgr.rs`'s already-documented `+0x440` `appInitSuccess` field). RVA = `0x00638154 - 0x400000`.
///
/// **Do not dereference this as a live pointer to read the `+0x440`/`+0x441` fields - use
/// [`RVA_APP_INIT_SUCCESS_BASE`] instead.** See that constant's own doc comment for why: this slot is not a
/// stable object pointer for that purpose, even though it looks like one from the decompile alone.
pub const RVA_GLOBAL_ZTAPP: u32 = 0x0023_8154;

/// The real, fixed base address every `appInitSuccess`(`+0x440`)/`loadInProgress`(`+0x441`) read in this file
/// actually resolves to - confirmed directly from `gameopts_loadInProgress.asm`/`ZTGameMgr_stop.asm`, which
/// both compile to the identical shape `MOV EAX, GLOBAL_ZTApp; TEST EAX,EAX; JZ .init; ...; .init: MOV dword
/// ptr GLOBAL_ZTApp, ZTApp::handleMessages; .common: MOV EAX, appInitSuccess; MOV AL, [EAX+0x440_or_0x441]`.
/// The `TEST EAX,EAX`/`JZ` only gates a *side-effect* write (stashing `ZTApp::handleMessages`'s address into
/// [`RVA_GLOBAL_ZTAPP`]'s slot as a run-once sentinel) - both branches converge on the same `MOV EAX,
/// appInitSuccess` before the actual field read, so the field is always read from this fixed address,
/// **never** through [`RVA_GLOBAL_ZTAPP`]'s own stored value.
///
/// Every call site in this file that instead did `ztapp_ptr = *(RVA_GLOBAL_ZTAPP); flag =
/// *(ztapp_ptr+0x440/0x441)` was reading through a live bug, not a faithful port: once *any* one of these
/// inlined accessors has executed even once, `RVA_GLOBAL_ZTAPP`'s slot permanently holds
/// `ZTApp::handleMessages`'s address (`0x441880`) as that leftover sentinel - not a real `ZTApp*` - so every
/// such read was dereferencing `[0x441880 + 0x440/0x441]`, effectively-random bytes inside the `.text`
/// section near that function. Live-confirmed (via a temporary diagnostic during the
/// [`ZTHabitatMgr::snap_tank_walls_inward`] crash investigation - see its own doc comment) that this made
/// `load_in_progress` read persistently non-zero throughout ordinary interactive play, long after any real
/// loading screen had finished, while the true fixed-address read correctly came back `0`. RVA =
/// `0x00638148 - 0x400000`.
pub const RVA_APP_INIT_SUCCESS_BASE: u32 = 0x0023_8148;

/// `DAT_00639188`'s RVA - the currently-loading save file's own format version (`ZTUI::gameopts::
/// getFileVersion`'s backing store, set once by `gameopts_loadFile.c`'s own `deallocate(&DAT_00639188,...)`
/// read). [`ZTHabitatMgr::place_gate`]'s own load-time fast path only consults the candidate-placement
/// table below once this exceeds `0x14` (20) - real vanilla's own save-format-version gate for whether the
/// table was even populated by whatever earlier load step fills it. RVA = `0x00639188 - 0x400000`.
pub const RVA_SAVE_FILE_VERSION: u32 = 0x0023_9188;

/// `DAT_00639178`/`DAT_0063917c`'s RVAs - the begin/end of a load-time table of already-decided gate
/// placements (each record `0x128` bytes: tile `x`/`y` at `+0x0`/`+0x4`, a validity flag at `+0x14`,
/// content beyond that not read by this port). [`ZTHabitatMgr::place_gate`]'s own fast path walks this
/// table first (only once [`RVA_GLOBAL_ZTAPP`]'s `+0x441` load-in-progress flag is set and
/// [`RVA_SAVE_FILE_VERSION`] is new enough) and returns `true` immediately if a record's tile resolves
/// (via [`ZTHabitatMgr::get_habitat_ptr`]) to the habitat being placed and its validity flag is
/// non-negative - real vanilla treats a gate already recorded this way as already placed, skipping the
/// rest of the function entirely. Purpose/producer of this table not otherwise identified; only its shape
/// matters here. RVAs = `0x00639178 - 0x400000` / `0x0063917c - 0x400000`.
pub const RVA_LOAD_CANDIDATE_GATES_BEGIN: u32 = 0x0023_9178;
pub const RVA_LOAD_CANDIDATE_GATES_END: u32 = 0x0023_917c;

/// `DAT_006351a4`'s RVA - a single byte flag [`ZTHabitatMgr::temp_keeper_for_pathfinding`] forwards
/// verbatim as the "create" argument to the temporary keeper's own entity-type vtable slot `+0x24`
/// (`ZTHabitatMgr_placeGate.asm`'s own `MOV DL, byte ptr DAT_006351a4` right before that call) - meaning
/// not otherwise identified, only its role as a pass-through argument. RVA = `0x006351a4 - 0x400000`.
pub const RVA_TEMP_KEEPER_CREATE_FLAG: u32 = 0x0023_51a4;

/// `GLOBAL_BFUIMgr`'s own fixed address (a plain static object, not a pointer slot - same constant
/// `ztresearch.rs`/`ztshowui.rs`/`ztthoughtmgr.rs`/`zoostatus.rs::raw_globals::GLOBAL_BFUIMGR_RVA` each
/// already document independently). [`ZTHabitatMgr::display_gate_placement_message`] needs its own copy
/// since `zoostatus.rs`'s is `pub(super)`. RVA = `0x00638de0 - 0x400000`.
pub const RVA_GLOBAL_BFUIMGR: u32 = 0x0023_8de0;

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
pub fn is_wall(fence_ptr: u32) -> bool {
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
pub fn is_tank_wall(fence_ptr: u32) -> bool {
    if fence_ptr == 0 {
        return false;
    }
    if !unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
        return false;
    }
    let entity_type_ptr: u32 = get_from_memory(fence_ptr + 0x128);
    get_from_memory::<u8>(entity_type_ptr + 0x193) != 0
}

/// Swaps `fence_a`'s and `fence_b`'s own world position (`BFEntity`'s `pos` field, `+0x114`), rotation
/// (`+0x12c`), and cached virtual position (`+0xb4`/`+0xb8`/`+0xbc`) - each keeps its own identity, but the
/// two fences trade places, `fence_b` moving onto `fence_a`'s original spot and vice versa. Part of
/// [`ZTHabitatMgr::snap_tank_walls_inward`]'s own position-swap branch - confirmed at the `.asm` level
/// (`ZTHabitatMgr_snapTankWallsInward.asm`); the C decompile's own field names for these offsets are
/// fictional.
pub fn swap_fence_positions(world_map_ptr: u32, fence_a: u32, fence_b: u32) {
    unsafe {
        call_vtable_slot_with_u8(fence_b, 0x78, 0); // fence_b->removeFromMap(false)
        call_vtable_slot_with_u8(fence_a, 0x78, 0); // fence_a->removeFromMap(false)
    }

    let fence_a_pos = get_from_memory::<IVec3>(fence_a + 0x114);
    let fence_a_rotation: i32 = get_from_memory(fence_a + 0x12c);
    let fence_b_pos = get_from_memory::<IVec3>(fence_b + 0x114);
    let fence_b_rotation: i32 = get_from_memory(fence_b + 0x12c);

    let mut virtual_pos_for_b = [0i32; 3];
    unsafe {
        WORLD_TO_VIRTUAL_0.original()(world_map_ptr as *const u32, virtual_pos_for_b.as_mut_ptr() as *const i32, &fence_a_pos as *const IVec3 as *const i32)
    };
    save_to_memory(fence_b + 0xb4, virtual_pos_for_b[0]);
    save_to_memory(fence_b + 0xb8, virtual_pos_for_b[1]);
    save_to_memory(fence_b + 0xbc, virtual_pos_for_b[2]);
    unsafe { BFENTITY_SET_WORLD_POS.original()(fence_b as *const u32, &fence_a_pos as *const IVec3 as *const u32) };
    save_to_memory(fence_b + 0x12c, fence_a_rotation);

    let mut virtual_pos_for_a = [0i32; 3];
    unsafe {
        WORLD_TO_VIRTUAL_0.original()(world_map_ptr as *const u32, virtual_pos_for_a.as_mut_ptr() as *const i32, &fence_b_pos as *const IVec3 as *const i32)
    };
    save_to_memory(fence_a + 0xb4, virtual_pos_for_a[0]);
    save_to_memory(fence_a + 0xb8, virtual_pos_for_a[1]);
    save_to_memory(fence_a + 0xbc, virtual_pos_for_a[2]);
    unsafe { BFENTITY_SET_WORLD_POS.original()(fence_a as *const u32, &fence_b_pos as *const IVec3 as *const u32) };
    save_to_memory(fence_a + 0x12c, fence_b_rotation);

    unsafe {
        call_vtable_slot_noargs(fence_b, 0x74); // fence_b->addToMap()
        call_vtable_slot_noargs(fence_a, 0x74); // fence_a->addToMap()
    }
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
pub fn fence_entity_flag_0x6f(fence_ptr: u32) -> bool {
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
pub fn first_owned_tile_extra_height(habitat_ptr: u32) -> i32 {
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
pub fn tank_height_plus_extra(habitat_ptr: u32) -> i32 {
    let tank_height: i32 = get_from_memory(habitat_ptr + 0x184);
    tank_height + first_owned_tile_extra_height(habitat_ptr)
}

/// The `(source_fence, neighbour_fence)` pair [`is_wall`] must both clear before the flood-fill in
/// [`ZTHabitat::add_habitat_tiles`] may step from `tile` to `neighbour` in `direction` - confirmed by
/// cross-referencing which `BFTile` fence field (`north_fence`=`+0x14`/`east_fence`=`+0x18`/
/// `south_fence`=`+0x1c`/`west_fence`=`+0x20`) `ZTHabitat_addSeedsOnStack.c`/`_addContiguousSpan.c` check
/// for each direction constant - the same cross-reference that corrected [`Direction`]'s own variant
/// names. Only the four cardinal directions are ever used by `addHabitatTiles`.
pub fn fence_pair(tile: &BFTile, neighbour: &BFTile, direction: &Direction) -> (u32, u32) {
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
pub fn get_neighbour_ptr(world: &ZTWorldMgr, tile_ptr: u32, direction: Direction) -> u32 {
    let tile = get_from_memory::<BFTile>(tile_ptr);
    match world.get_neighbour(&tile, direction) {
        Some(neighbour) => world.get_ptr_from_bftile(&neighbour),
        None => 0,
    }
}

/// `BFMap::getNeighbor` called with a raw, unvalidated direction rather than a [`Direction`] enum value -
/// see [`ZTWorldMgr::get_neighbour_ptr_raw`]. A direction outside `0..=7` returns `tile_ptr` itself,
/// which [`ZTHabitatMgr::fence_placed`] relies on (its direction argument can be the `0xffffffff` "no
/// direction" sentinel); [`get_neighbour_ptr`]'s [`Direction::from`] would step North instead.
pub fn get_neighbour_raw(world: &ZTWorldMgr, tile_ptr: u32, direction: u32) -> u32 {
    world.get_neighbour_ptr_raw(tile_ptr, direction)
}

/// Rotates a cardinal `BFTile` direction (`0`/`2`/`4`/`6` - `North`/`East`/`South`/`West`) by `steps`
/// 45-degree increments (`±2` = a 90-degree turn) - [`ZTHabitatMgr::fence_placed`]'s own
/// `DAT_00634fd4`/`DAT_00634ff4` per-direction lookup tables, confirmed via a live Ghidra hexdump:
/// `DAT_00634fd4[i] = (i+2)&7` for even `i` (else `-1`); `DAT_00634ff4[i] = (i-2)&7` for even `i` (else
/// `-1`). `None` for a non-cardinal or sentinel (`0xffffffff`) input, matching those tables' own `-1`
/// entries - real vanilla's own array reads for these two tables have no bounds guard at their call
/// sites, an out-of-bounds read this port avoids by returning `None` instead (same "dead in practice,
/// defend anyway" convention this file uses throughout for a real, unguarded vanilla read that no
/// realistic caller actually reaches).
pub fn rotate_cardinal_direction(direction: u32, steps: i32) -> Option<u32> {
    if direction > 6 || !direction.is_multiple_of(2) {
        return None;
    }
    Some(((direction as i32 + steps).rem_euclid(8)) as u32)
}

/// `is_wall`([`ZTHabitat::fence_slot_by_index`]`(tile, direction/2))` - `false` for a null `tile_ptr` or a
/// `None` direction (the non-cardinal/sentinel case [`rotate_cardinal_direction`] already filters), the
/// same "guard rather than index -1" shape [`ZTHabitatMgr::fence_placed`]'s own dozen near-identical
/// fence-passability checks all share.
pub fn fence_wall_at(tile_ptr: u32, direction: Option<u32>) -> bool {
    if tile_ptr == 0 {
        return false;
    }
    let Some(direction) = direction else { return false };
    let tile = get_from_memory::<BFTile>(tile_ptr);
    is_wall(ZTHabitat::fence_slot_by_index(&tile, (direction / 2) as i32))
}

/// The up-to-4 cardinal neighbours of `tile_ptr` that [`ZTHabitatMgr::can_find_path`]'s BFS may step
/// into - a real, in-map neighbour ([`get_neighbour_ptr`]) not blocked by a wall on either side
/// ([`fence_pair`]/[`is_wall`], the same two-sided check [`ZTHabitat::add_habitat_tiles`]'s own
/// flood-fill uses).
pub fn pathfinding_frontier(world: &ZTWorldMgr, tile_ptr: u32) -> impl Iterator<Item = u32> + '_ {
    let tile = get_from_memory::<BFTile>(tile_ptr);
    [Direction::North, Direction::East, Direction::South, Direction::West].into_iter().filter_map(move |direction| {
        let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction);
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
pub fn insert_tile_list_node(sentinel: u32, tile_ptr: u32) {
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
pub fn claim_tile(habitat_mgr: &ZTHabitatMgr, sentinel: u32, target: u32, tile_ptr: u32) {
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
pub fn add_contiguous_span(world: &ZTWorldMgr, habitat_mgr: &ZTHabitatMgr, sentinel: u32, target: u32, seed_tile_ptr: u32) -> (u32, u32) {
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
pub fn enqueue_if_claimable(world: &ZTWorldMgr, habitat_mgr: &ZTHabitatMgr, target: u32, tile_ptr: u32, direction: Direction, worklist: &mut Vec<u32>) {
    let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction);
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
pub const START_SOUND_NAME_RVA: u32 = 0x0063_e49c - 0x400000;
pub const END_SOUND_NAME_RVA: u32 = 0x0063_e4a0 - 0x400000;

/// The showpanel UI's currently-open habitat pointer (`DAT_0063e44c`) - also read by `_addTrick.c`/
/// `_collectAnimalTypes.c`/`_copyListToScript.c`/`_decreaseAdmission.c` for the same purpose.
pub const SHOWPANEL_CURRENT_HABITAT_RVA: u32 = 0x0063_e44c - 0x400000;

/// Sound-device singleton - same address `ztsoundscape.rs`'s/`ambients.rs`'s own `GLOBAL_DX8SNDMGR_RVA`
/// already document, duplicated locally per this codebase's existing per-file convention.
pub const GLOBAL_DX8SNDMGR_RVA: u32 = 0x0063_80a8 - 0x400000;

/// Real vanilla `SNDSound`'s static vtable (`SNDSound__vtable_00630bc0`, confirmed directly in
/// `ZTHabitat_setIsShowExhibit.c`'s `param2->vftptr_0x0 = &SNDSound__vtable_00630bc0` assignment) - a
/// data address, RVA'd like every other non-code address in this codebase.
pub const SNDSOUND_VTABLE_RVA: u32 = 0x0063_0bc0 - 0x400000;

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
pub fn rvo_target() -> Self {
        VanillaEventVector { begin: 0, end: 0, cap_end: 0 }
    }

pub fn as_ptr(&mut self) -> *mut u32 {
        self as *mut VanillaEventVector as *mut u32
    }
}

/// Frees the event-list buffer `ZTHabitat::listen` receives from `getEvents`, back to wherever real
/// vanilla's own allocator would - by **capacity** (`cap_end - begin`), matching `ZTHabitat_listen.c`'s
/// own math and `ambients.rs`'s live-tested `free_group_array_buffer` exactly (same freelist family,
/// same `>0x80` `operator_delete` cutoff).
pub fn free_event_vector_buffer(buf: u32, byte_capacity: u32) {
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

/// Tears down the real vanilla-layout found-species scratch vector `ZTHabitat::recalc_phase_1_2`/
/// `recalc_phase_3` build and extend (real vanilla's own `in_stack_fffff3b8`/`_bc`/`_c0` stack local,
/// `msvc_std::vector_pod<>::_Tidy`'d at the very end of the whole `recalculateCharacteristics` call) -
/// frees the buffer through [`free_event_vector_buffer`]'s same freelist-bucket/`operator_delete` split,
/// matching `ZTHabitat_addFoundSpecies.c`'s own internal growth teardown exactly (this vector is only
/// ever grown through [`vector_push_pool_alloc4`] or real vanilla's own `addFoundSpecies`, both of which
/// share this same allocator convention). No `generated.rs` entry exists for real vanilla's own `_Tidy`
/// (not yet cataloged by the Ghidra pass) - reimplemented directly here rather than guessed/hand-added.
pub fn destroy_found_species_vector(vec: &crate::vanilla_vector::VanillaVector<u32>) {
    let begin = vec.begin as u32;
    let cap_end = vec.cap_end as u32;
    free_event_vector_buffer(begin, cap_end - begin);
}

/// Appends `value` (a raw `u32` pointer) to a real vanilla `std::vector<T*>` (`begin`/`end`/`cap_end`, one
/// word each) living at `vector_ptr` - the shared out-param growth shape `ZTHabitat_getSicklyAnimals.c`/
/// `_getViewingAreasWithGuests.c` both use identically (real vanilla's own `PoolAlloc::allocate` doubling
/// growth, old buffer freed via [`free_event_vector_buffer`] - the same manual freelist-bucket/
/// `operator_delete` split those two decompiles' own tails use, not a `PoolAlloc::deallocate` call).
/// Distinct from [`Self::push_boundary_tile_pair`]-style helpers, which grow a *field* of `self` - this
/// operates on an arbitrary out-param address handed in by the caller (real vanilla's own stack-allocated
/// local vector, in every known call site).
pub fn vector_push_pool_alloc4(vector_ptr: u32, value: u32) {
    vector_push_pool_alloc4_with_dealloc(vector_ptr, value, free_event_vector_buffer);
}

/// Variant used by the biome-tile-filter triplet (`ZTHabitat_addLandTiles.c`/`_addWaterTiles.c`/
/// `_addUnderwaterTiles.c`, all byte-for-byte identical apart from the predicate): same growth shape as
/// [`vector_push_pool_alloc4`], but the old buffer goes back through real vanilla
/// `PoolAlloc::deallocate(buf, byte_capacity)` guarded on non-null (`.asm`-confirmed
/// `TEST %EAX, %EAX`/`JZ` around the call, and `SAR 2`/`SHL 2` capacity math) - a different teardown
/// from the manual freelist split [`free_event_vector_buffer`] reimplements, which those decompiles
/// themselves don't use at this call site.
pub fn vector_push_pool_alloc4_pool_dealloc(vector_ptr: u32, value: u32) {
    vector_push_pool_alloc4_with_dealloc(vector_ptr, value, |buf, byte_capacity| {
        if buf != 0 {
            unsafe { POOLALLOC_DEALLOCATE.original()(buf as *const u32, byte_capacity) };
        }
    });
}

/// [`vector_push_pool_alloc4`]'s own body, parameterized over the old buffer's teardown - the two
/// shapes above are the only variants this corpus's push-back sites use.
fn vector_push_pool_alloc4_with_dealloc(vector_ptr: u32, value: u32, dealloc_old_buffer: fn(u32, u32)) {
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
        dealloc_old_buffer(begin, cap_end - begin);

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
pub fn looks_like_configured_sound_name(addr: u32) -> bool {
pub const MAX_LEN: u32 = 64;
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

pub fn construct_and_acquire_sound(dx8_sndmgr: u32, name_ptr: u32) -> u32 {
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
pub unsafe fn call_vtable_slot_noargs(entity_ptr: u32, slot_offset: u32) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32)>(target) };
    f(entity_ptr);
}

/// Same shape as [`call_vtable_slot_noargs`], but propagating the slot's `bool` result (read from
/// `AL`, matching [`call_vtable_slot_with_ptr_ret_bool`]) - `ZTHabitat`'s own vtable `+0x20` `isTank`
/// dispatch, used by the battery's `ZTHABITAT_IS_TANK_LIVE` (both of that slot's vtable poles - the
/// plain-habitat base and `ZTTankExhibit`'s override - are 2-instruction constant-return stubs that
/// never dereference `this`).
pub unsafe fn call_vtable_slot_noargs_ret_bool(entity_ptr: u32, slot_offset: u32) -> bool {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> bool>(target) };
    f(entity_ptr)
}

/// Calls a 1-arg (`u8`) thiscall vtable slot - `ZTHabitat_setIsNotShowExhibit.c`'s sound teardown's final
/// `(**vtable)(1)` scalar-deleting-destructor call (slot `+0x0`), the same idiom this vtable doc's own
/// "+0x18 slot" correction documents for `ZTHabitat`'s own destructor.
pub unsafe fn call_vtable_slot_with_u8(entity_ptr: u32, slot_offset: u32, arg: u8) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u8)>(target) };
    f(entity_ptr, arg);
}

/// Same shape as [`call_vtable_slot_with_u8`], for a 2-arg (`u8`, `u8`) thiscall vtable slot -
/// `ZTHabitat_removeFoodTargetForAll.c`/`.asm`'s own `BFEntity::setIsRemoved(bool, bool)` teardown call
/// (slot `+0xa8`, confirmed real `BFEntity` base slot - index 42 in `private/docs/vtables/BFEntity.md`/
/// `ZTScenery.md`).
pub unsafe fn call_vtable_slot_with_u8_u8(entity_ptr: u32, slot_offset: u32, arg1: u8, arg2: u8) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u8, u8)>(target) };
    f(entity_ptr, arg1, arg2);
}

/// Calls a 1-arg (raw pointer) thiscall vtable slot - `ZTHabitat_moveGateTo_0.asm`'s own `+0x1c` gate/
/// fence `setName` dispatch (`PUSH <name-buffer-ptr>; CALL [vtable+0x1c]`), confirmed at the `.asm` level
/// since the C decompile's own struct-offset math for this call is garbled (see
/// [`ZTHabitatMgr::replace_fence_with_gate`]'s own doc comment for the same class of decompile-vs-`.asm`
/// mismatch).
pub unsafe fn call_vtable_slot_with_ptr(entity_ptr: u32, slot_offset: u32, arg: u32) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32)>(target) };
    f(entity_ptr, arg);
}

/// Calls a 2-arg (pointer, pointer) thiscall vtable slot - `ZTHabitat_resize.asm`'s own `+0x38`
/// `addHabitatTiles(seed_tile, this)` dispatch (`PUSH this; PUSH tile; CALL [vtable+0x38]`), the twin of
/// the `+0x3c` no-arg `removeHabitatTiles` call right before it. Kept a real dispatch (rather than a
/// fixed-address call-through) because `ZTTankExhibit` overrides both slots - see
/// [`crate::zthabitat::habitat::ZTHabitat::resize`].
pub unsafe fn call_vtable_slot_with_ptr_ptr(entity_ptr: u32, slot_offset: u32, arg1: u32, arg2: u32) {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32)>(target) };
    f(entity_ptr, arg1, arg2);
}

/// Same shape as [`call_vtable_slot_with_ptr`], but for a slot returning a bool result -
/// `ZTHabitat_getNearestDirtPile.asm`'s own `keeper_ptr` vtable `+0x324` dispatch (`this=keeper_ptr`, one
/// pointer stack arg = the candidate entity) - an unidentified per-keeper entity-target filter.
pub unsafe fn call_vtable_slot_with_ptr_ret_bool(entity_ptr: u32, slot_offset: u32, arg: u32) -> bool {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(target) };
    f(entity_ptr, arg)
}

/// Calls a 4-arg (3 pointers + `u32`) thiscall vtable slot returning bool - the shared `GLOBAL_ZTAIMgr`
/// vtable `+0x1c` visibility/path-reachability dispatch both `ZTHabitat::getNearestDirtPile` and
/// `ZTHabitat::getNearestSickAnimal` call when their own `check_can_see` parameter is set (`this=ai_mgr`,
/// `from_tile`, `to_tile`, `unit_ptr`, `0` - confirmed identical stack shape at both real call sites via a
/// manual `.asm` trace). Resembles `generated.rs`'s `bfaimgr::CHECK_PATH` in its first three stack args but
/// is not confirmed to be the same function - that entry's own signature carries one fewer stack arg than
/// real vanilla pushes here, so this stays a raw, unidentified dispatch rather than a call to
/// `CHECK_PATH`.
pub unsafe fn call_vtable_slot_ptr_ptr_ptr_u32_ret_bool(this_ptr: u32, slot_offset: u32, arg1: u32, arg2: u32, arg3: u32, arg4: u32) -> bool {
    let vtable = get_from_memory::<u32>(this_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32) -> bool>(target) };
    f(this_ptr, arg1, arg2, arg3, arg4)
}

/// Same shape as [`call_vtable_slot_with_ptr`], for a single-byte-argument slot that returns a `u32`
/// (pointer) result - `BFEntityType`'s own unnamed vtable `+0x24` slot,
/// [`ZTHabitatMgr::temp_keeper_for_pathfinding`]'s own "create an instance of this type" call
/// (`ZTHabitatMgr_placeGate.asm`'s `CALL dword ptr [EAX+0x24]` on a `BFWorldMgr::getType` result, one
/// byte arg pushed, `EAX` returned as the new instance).
pub unsafe fn call_type_vtable_slot_u8_ret_u32(type_ptr: u32, slot_offset: u32, arg: u8) -> u32 {
    let vtable = get_from_memory::<u32>(type_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(target) };
    f(type_ptr, arg as u32)
}

/// Calls the unnamed `BFUnit` vtable slot `+0x164` (`BFUnit.md` confirms the slot exists, marked
/// `*unknown*` - no name anywhere in the vtable docs, and no other call site anywhere in the decompile
/// corpus) with a single `BFTile*` argument, returning its `i32` result. [`ZTHabitatMgr::check_enter_habitat`]'s
/// own tank-branch cost comparison, in place of [`BFUNIT_GET_PATH_COST`]'s non-tank equivalent - same
/// "raw pointer-indirection call-through" this codebase already uses for `ZTHabitat::block_service`'s
/// own unnamed `ZTStaff` vtable slots. `pub(crate)` for the directional-tile live tests, which rebuild
/// the port's expected candidate set through the same dispatch.
pub unsafe fn call_bfunit_tile_cost_vtable_slot(unit_ptr: u32, tile_ptr: u32) -> i32 {
    let vtable = get_from_memory::<u32>(unit_ptr);
    let target = get_from_memory::<u32>(vtable + 0x164);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> i32>(target) };
    f(unit_ptr, tile_ptr)
}

/// Calls `BFEntityType`'s `create` vtable slot (`+0x24`, confirmed via `BFOverlayType_create.c` - the
/// only decompiled instantiation, a base default implementation shared across entity-type subclasses
/// rather than a `BFOverlayType`-specific override, matching the same "shared default across slots"
/// idiom `BFEntityType.md`'s own raw vtable dump documents elsewhere in that same table) with a single
/// `bool` argument (real vanilla always passes `false` at every known call site). Returns the newly
/// spawned entity's own pointer, or `0`. [`ZTHabitatMgr::create_double_fence`]'s own entity factory call,
/// and a genuine virtual dispatch (unlike most of this file's raw-pointer helpers), since different
/// `*Type` subclasses could in principle override `create` itself, even though none has been observed
/// doing so in this codebase.
pub unsafe fn call_entity_type_create(entity_type_ptr: u32) -> u32 {
    let vtable = get_from_memory::<u32>(entity_type_ptr);
    let target = get_from_memory::<u32>(vtable + 0x24);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u8) -> u32>(target) };
    f(entity_type_ptr, 0)
}

/// Tears down one of `ZTHabitat`'s two owned `SNDSound`s (`start_sound_ptr`/`end_sound_ptr`), per
/// `ZTHabitat_setIsNotShowExhibit.c`'s identical teardown shape for both fields: a `+0x50` predicate
/// check gating an optional `+0x60` "stop" call, then an unconditional `+0x0(1)` scalar-deleting-
/// destructor call. No-op for a null pointer (mirrors vanilla's own null guard).
pub fn teardown_sound(sound_ptr: u32) {
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
pub fn write_bytes_to_file<T>(value: &T, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(value as *const T as *const u32, mem::size_of::<T>() as u32, 1, file) == 1 }
}

/// Writes `len` raw bytes starting at `ptr` (not a typed value's own address) - the counterpart
/// [`ZTHabitat::save`] needs for its own variable-length `exhibit_name` buffer, which [`write_bytes_to_file`]
/// can't express since its length isn't known at compile time.
pub fn write_raw_bytes(ptr: u32, len: u32, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(ptr as *const u32, len, 1, file) == 1 }
}

/// Calls vtable slot `+0x1c` (`save`, confirmed via `ZTHabitatMgr_save.asm`'s own `CALL dword ptr
/// [EAX+0x1c]` dispatch) on `entity_ptr` with `file` - the polymorphic dispatch [`ZTHabitatMgr::save`]
/// itself uses to reach whichever `save` override each `exhibit_array` entry's real vtable currently
/// points at (our own detoured [`ZTHabitat::save`] once installed, or any un-detoured `ZTTankExhibit`
/// override), rather than assuming every entry is a plain `ZTHabitat`.
pub unsafe fn call_save_vtable_slot(entity_ptr: u32, file: *const i8) -> bool {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + 0x1c);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, *const i8) -> u8>(target) };
    f(entity_ptr, file) != 0
}

/// Leaked-block fixtures shared by the `ZTHabitat` and `support` unit tests: raw addresses stay valid for
/// the port's volatile reads because nothing is ever freed.
#[cfg(test)]
pub(super) mod test_fixtures {
    use crate::util::save_to_memory;

    /// Leaks a zeroed 20-byte MSVC `_Tree_node`-shaped block and writes `parent`/`left`/`right`/
    /// `value` into the `+0x4`/`+0x8`/`+0xc`/`+0x10` slots [`super::walk_neighbor_tree`] reads,
    /// returning the block's address.
    pub fn leak_tree_node(parent: u32, left: u32, right: u32, value: u32) -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; 0x14].into_boxed_slice());
        let node_ptr = block.as_ptr() as u32;
        save_to_memory(node_ptr + 0x4, parent);
        save_to_memory(node_ptr + 0x8, left);
        save_to_memory(node_ptr + 0xc, right);
        save_to_memory(node_ptr + 0x10, value);
        node_ptr
    }

    /// Leaks a zeroed 16-byte [`TileListNode`](super::TileListNode)-shaped block with `next`/`payload`
    /// written at the `+0x0`/`+0x8` slots [`super::walk_tile_list`] reads, returning its address.
    /// `prev` (`+0x4`) stays zeroed - no walker reads it.
    pub fn leak_tile_node(next: u32, payload: u32) -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; 0x10].into_boxed_slice());
        let node_ptr = block.as_ptr() as u32;
        save_to_memory(node_ptr, next);
        save_to_memory(node_ptr + 0x8, payload);
        node_ptr
    }

    /// Leaks a `TileListNode` sentinel plus one node per `tiles` entry, returning the sentinel's
    /// address - the value `owned_tiles_ptr` must hold for [`super::walk_tile_list`] to walk `tiles`.
    /// Nodes are spliced at the front, so the walk yields them in reverse of `tiles`' order.
    pub fn leak_tile_list(tiles: &[u32]) -> u32 {
        let sentinel = leak_tile_node(0, 0);
        save_to_memory(sentinel, sentinel); // empty list: the sentinel's own next points back at it
        save_to_memory(sentinel + 0x4, sentinel);
        let mut first = sentinel;
        for &tile in tiles {
            let node = leak_tile_node(first, tile);
            save_to_memory(node + 0x4, sentinel);
            first = node;
        }
        save_to_memory(sentinel, first);
        sentinel
    }

    /// Leaks an MSVC `std::set` head over `payloads`, returning the head's address - the value
    /// `amphibious_neighbors_head` must hold for [`super::walk_neighbor_tree`] to visit exactly
    /// `payloads`. Nodes chain in a right-leaning vine (`payloads[n]` the right child of
    /// `payloads[n-1]`). Every head slot follows MSVC's own real construction - `+0x4`/`+0x8`/`+0xc` hold
    /// the root/leftmost/rightmost node (the head itself when empty) - which matters for the successor
    /// walk's end-of-tree climb: a null `+0x8` would send it reading address `0xc`, and a null `+0x4`
    /// would strand that climb on a single-node set.
    pub fn leak_neighbor_set(payloads: &[u32]) -> u32 {
        let head = leak_tree_node(0, 0, 0, 0);
        save_to_memory(head, 1u8); // the head's own isnil flag; real nodes are all 0 (zeroed)
        let mut prev = head; // newest node = rightmost so far
        let mut root = head; // first node added = tree root = leftmost in a right vine
        for &payload in payloads {
            let node = leak_tree_node(prev, 0, 0, payload);
            save_to_memory(prev + 0xc, node);
            if root == head {
                root = node;
            }
            prev = node;
        }
        save_to_memory(head + 0x4, root); // head._Parent = root (the head itself when empty)
        save_to_memory(head + 0x8, root); // head._Left = leftmost = root in a right vine
        save_to_memory(head + 0xc, prev); // head._Right = rightmost (the head itself when empty)
        head
    }
}

#[cfg(test)]
mod tests {
    use super::test_fixtures::{leak_neighbor_set, leak_tile_list, leak_tile_node, leak_tree_node};
    use super::{lcg_next, rotate_cardinal_direction, walk_neighbor_tree, walk_tile_list};
    use crate::util::get_from_memory;

    #[test]
    fn lcg_next_known_sequence() {
        assert_eq!(lcg_next(0), 0x269ec3);
        assert_eq!(lcg_next(0x269ec3), 0x269ec3u32.wrapping_mul(0x343fd).wrapping_add(0x269ec3));
        assert_eq!(lcg_next(1), 0x343fd + 0x269ec3);
    }

    #[test]
    fn lcg_next_wraps_at_u32_max() {
        // u32::MAX * 0x343fd == -0x343fd (mod 2^32), so the result is 0x269ec3 - 0x343fd.
        assert_eq!(lcg_next(u32::MAX), 0x269ec3u32.wrapping_sub(0x343fd));
    }

    #[test]
    fn walk_tile_list_empty_yields_nothing() {
        let sentinel = leak_tile_list(&[]);
        assert_eq!(walk_tile_list(sentinel).count(), 0);
    }

    #[test]
    fn walk_tile_list_yields_nodes_never_sentinel() {
        let sentinel = leak_tile_list(&[0xa, 0xb, 0xc]);
        let nodes: Vec<u32> = walk_tile_list(sentinel).collect();
        assert_eq!(nodes.len(), 3);
        assert!(!nodes.contains(&sentinel));
        // Front-spliced: the walk yields the last-added node first.
        let payloads: Vec<u32> = nodes.iter().map(|&n| get_from_memory::<u32>(n + 0x8)).collect();
        assert_eq!(payloads, vec![0xc, 0xb, 0xa]);
    }

    #[test]
    fn walk_tile_list_hand_built_chain_order() {
        let sentinel = leak_tile_node(0, 0);
        let second = leak_tile_node(sentinel, 2);
        let first = leak_tile_node(second, 1);
        crate::util::save_to_memory(sentinel, first);
        assert_eq!(walk_tile_list(sentinel).collect::<Vec<_>>(), vec![first, second]);
    }

    fn payloads_of(nodes: &[u32]) -> Vec<u32> {
        nodes.iter().map(|&n| get_from_memory::<u32>(n + 0x10)).collect()
    }

    #[test]
    fn walk_neighbor_tree_empty() {
        let head = leak_neighbor_set(&[]);
        assert_eq!(walk_neighbor_tree(head).count(), 0);
    }

    #[test]
    fn walk_neighbor_tree_single_node() {
        let head = leak_neighbor_set(&[0x1111]);
        let nodes: Vec<u32> = walk_neighbor_tree(head).collect();
        assert_eq!(nodes.len(), 1);
        assert_ne!(nodes[0], head);
        assert_eq!(payloads_of(&nodes), vec![0x1111]);
    }

    #[test]
    fn walk_neighbor_tree_right_vine_in_order() {
        let head = leak_neighbor_set(&[0x1, 0x2, 0x3, 0x4]);
        let nodes: Vec<u32> = walk_neighbor_tree(head).collect();
        assert_eq!(nodes.len(), 4);
        assert!(!nodes.contains(&head));
        assert_eq!(payloads_of(&nodes), vec![0x1, 0x2, 0x3, 0x4]);
    }

    #[test]
    fn walk_neighbor_tree_with_left_child_climbs_to_parent() {
        // root(2) with left child (1) and right child (3): in-order 1, 2, 3. The step off the left
        // leaf takes the "right child == 0, climb via parent" branch.
        let head = leak_tree_node(0, 0, 0, 0);
        crate::util::save_to_memory(head, 1u8);
        let left = leak_tree_node(0, 0, 0, 1);
        let right = leak_tree_node(0, 0, 0, 3);
        let root = leak_tree_node(head, left, right, 2);
        crate::util::save_to_memory(left + 0x4, root);
        crate::util::save_to_memory(right + 0x4, root);
        crate::util::save_to_memory(head + 0x4, root);
        crate::util::save_to_memory(head + 0x8, left);
        crate::util::save_to_memory(head + 0xc, right);

        let nodes: Vec<u32> = walk_neighbor_tree(head).collect();
        assert_eq!(nodes, vec![left, root, right]);
        assert_eq!(payloads_of(&nodes), vec![1, 2, 3]);
    }

    #[test]
    fn rotate_cardinal_direction_wraps_and_rejects() {
        assert_eq!(rotate_cardinal_direction(0, 2), Some(2));
        assert_eq!(rotate_cardinal_direction(6, 2), Some(0));
        assert_eq!(rotate_cardinal_direction(0, -2), Some(6));
        assert_eq!(rotate_cardinal_direction(2, 0), Some(2));
        assert_eq!(rotate_cardinal_direction(1, 2), None);
        assert_eq!(rotate_cardinal_direction(7, 2), None);
        assert_eq!(rotate_cardinal_direction(8, 2), None);
        assert_eq!(rotate_cardinal_direction(0xffff_ffff, 2), None);
    }
}

/// Per-habitat boundary tile-pair lists (`(owned tile, neighbouring tile)` addresses), keyed by the
/// `ZTHabitat`'s own address. Vanilla's `+0x48`/`+0x4c`/`+0x50` vector triple is never written once the
/// constructor has zeroed it, so vanilla code sees an empty vector. Entries are created by
/// [`push_boundary_tile_pair`] and removed by [`take_boundary_tile_pairs`] in `ZTHabitat::destruct`.
static BOUNDARY_TILE_PAIRS: LazyLock<Mutex<HashMap<u32, Vec<(u32, u32)>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn boundary_tile_pairs_store() -> MutexGuard<'static, HashMap<u32, Vec<(u32, u32)>>> {
    BOUNDARY_TILE_PAIRS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Copy of `habitat_ptr`'s boundary tile-pairs; empty when none have been built.
pub(crate) fn boundary_tile_pairs(habitat_ptr: u32) -> Vec<(u32, u32)> {
    boundary_tile_pairs_store().get(&habitat_ptr).cloned().unwrap_or_default()
}

pub(crate) fn push_boundary_tile_pair(habitat_ptr: u32, tile_a: u32, tile_b: u32) {
    boundary_tile_pairs_store().entry(habitat_ptr).or_default().push((tile_a, tile_b));
}

pub(crate) fn clear_boundary_tile_pairs(habitat_ptr: u32) {
    if let Some(pairs) = boundary_tile_pairs_store().get_mut(&habitat_ptr) {
        pairs.clear();
    }
}

/// Removes and returns `habitat_ptr`'s boundary tile-pairs.
pub(crate) fn take_boundary_tile_pairs(habitat_ptr: u32) -> Vec<(u32, u32)> {
    boundary_tile_pairs_store().remove(&habitat_ptr).unwrap_or_default()
}
