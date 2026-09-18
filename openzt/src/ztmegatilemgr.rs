//! Structs and methods for the vanilla `ZTMegatileMgr`/`ZTMegatile` classes: terrain "megatile" (5x5
//! tile block) characteristic recalculation - guest density, a per-tile `stink` scalar, and per-
//! `BFCategory` "esthetic" averages, consumed by `ZTGuest::fCrowdDensityMegatile`/`fStinkyMegatile`/
//! `fEstheticBonusMegatile`, detoured in `ztguest.rs` onto Rust reimplementations that call this
//! file's accessors (see that module's doc comment). The struct layout below must still stay
//! byte-exact rather than merely behaviorally equivalent: those accessors still read vanilla's own
//! live-owned memory directly, not a migrated Rust store.
//!
//! Allocator strategy: the outer `vector<vector<ZTMegatile>>` grid and every embedded
//! `std::map<int,float>` stay vanilla-owned, allocated and mutated through vanilla's own
//! resolved-address STL helpers (see the raw-address consts below); any write that could allocate/free
//! (`vector::resize`, `map::insert`, `map::clear`) is call-through only. The one Rust allocation is
//! [`empty_category_map_sentinel`]'s leaked, read-only sentinel node - handed to vanilla to copy
//! *from* and never freed by either side, so `CLAUDE.md`'s Box-vs-vanilla-freelist warning has no
//! bite here.
//!
//! `update()`/`recalculate_characteristics()` never resize the outer vectors; `init()` is the only
//! vector-resize/allocation path. STL helper signatures are inferred from decompiled call sites rather
//! than confirmed from symbols; the live `reimplementation-tests` battery (see `live_support` below and
//! `reimplementation_tests::ZTMEGATILEMGR_*`) is the verification mechanism for them, not code review.

use std::mem;

use openzt_detour::generated::{
    bfcategory::GET_VALUE,
    ztmegatilemgr::{INIT, RECALCULATE_CHARACTERISTICS, UPDATE},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    util::get_from_memory,
    ztworldmgr::IVec3,
};

/// The zoo's megatile manager - owns a `vector<vector<ZTMegatile>>` grid, one `ZTMegatile` per 5x5 tile
/// block. Confirmed 1-entry (destructor-only) vtable - see `private/docs/vtables/ZTMegatileMgr.md`.
/// Offsets `0x10`..`0x17` are confirmed genuine reserved/padding space, not an undiscovered field: every
/// method on both platforms was checked at the raw-`.asm` level and none touches that range, and
/// `ZTMegatileMgr` has no `save`/`load` at all (`ZTWorldMgr::load` calls `init()` instead - megatile
/// state is fully transient/derived, recomputed from map dimensions on load, never serialized).
#[derive(Debug)]
#[repr(C)]
pub struct ZTMegatileMgr {
    vtable: u32,                 // 0x0
    flag: u8,                    // 0x4 - inherited BFMgr field, not behaviorally relevant
    _pad0: [u8; 3],
    dirty: u8,                   // 0x8 - "needs recalculateCharacteristics" flag
    _pad1: [u8; 3],
    tick_accumulator: u32,       // 0xc - update()'s delta-tick accumulator, threshold 0x1d4b (7499)
    _reserved_0x10: [u8; 8],     // 0x10-0x17 - confirmed genuine unused/reserved space
    row_start: *mut MegatileRow, // 0x18 - outer vector<vector<ZTMegatile>> begin
    row_end: *mut MegatileRow,   // 0x1c - outer vector end
    row_capacity_end: *mut MegatileRow, // 0x20 - outer vector end-of-storage
}

const _: () = assert!(mem::size_of::<ZTMegatileMgr>() == 0x24);

/// One outer-vector element: a `vector<ZTMegatile>` header (begin/end/end-of-storage), indexed by
/// `x_tile / 5` (confirmed via `ZTMegatileMgr_recalculateCharacteristics.c`'s `(iVar12/5)*0xc` outer
/// index, stride `0xc` = 3 pointers). Each column's own inner elements are indexed by `y_tile / 5`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct MegatileRow {
    start: *mut ZTMegatile,
    end: *mut ZTMegatile,
    capacity_end: *mut ZTMegatile,
}

const _: () = assert!(mem::size_of::<MegatileRow>() == 0xc);

impl MegatileRow {
    fn len(&self) -> usize {
        (self.end as usize - self.start as usize) / mem::size_of::<ZTMegatile>()
    }
}

/// One 5x5-tile-block's worth of recalculated characteristics. Size `0x14` confirmed twice
/// independently on Windows (`init.c`'s `/0x14` element-count division,
/// `recalculateCharacteristics.c`'s `*0x14` per-column stride); the macOS build shows a 24-byte
/// (`0x18`) struct with an extra vtable-like field at `+0x14`, but Windows evidence is authoritative
/// for this Windows-only build.
#[derive(Debug)]
#[repr(C)]
pub struct ZTMegatile {
    guest_count: i32,        // 0x0 - zeroed then incremented per resident guest in recalc
    category_map: MapHeader, // 0x4-0xf - std::map<int,float>, see MapHeader/TreeNode below
    stink: f32,              // 0x10 - running per-tile stink accumulator; the "esthetic" data is
                              // `category_map`, read via `category_value()`
}

const _: () = assert!(mem::size_of::<ZTMegatile>() == 0x14);

/// `std::map<int,float>` header, reconstructed from the inlined find-or-insert traversal in
/// `ZTMegatileMgr_recalculateCharacteristics.c` (MSVC inlines `map::operator[]`; the macOS build calls
/// a named `_find_or_insert<i,f>` template instead - useful for semantic corroboration but not for
/// Windows byte offsets, since the two platforms use unrelated STL implementations with different map
/// ABIs). `head` is **High** confidence (directly dereferenced in the traversal); `size`/`_reserved`
/// are Medium/Low confidence (inferred from local-variable ordering / elimination only) and are never
/// touched by the read-only `category_value` walk this struct exists to support.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct MapHeader {
    head: *mut TreeNode, // +0x0 - High confidence: directly dereferenced in the traversal
    size: u32,           // +0x4 - Medium confidence: element count, never read by this file
    _reserved: u32,      // +0x8 - Low confidence: comparator/allocator padding, never read by this file
}

const _: () = assert!(mem::size_of::<MapHeader>() == 0xc);

/// One `std::map<int,float>` node (24 bytes, heap-allocated, also reused as the header/sentinel node).
/// `parent`/`left`/`right`/`key`/`value` are all **High** confidence - every one is directly read or
/// compared in the confirmed traversal; `_color_isnil` is Medium-High (never touched by the "reset to
/// empty" sequence, but must exist structurally; corroborated by a freelist-push reusing this word once
/// a node is dead).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct TreeNode {
    _color_isnil: u32,   // +0x0
    parent: *mut TreeNode, // +0x4
    left: *mut TreeNode,   // +0x8
    right: *mut TreeNode,  // +0xc
    key: i32,             // +0x10
    value: f32,           // +0x14
}

const _: () = assert!(mem::size_of::<TreeNode>() == 0x18);

/// `std::pair<int,float>`, the map's `value_type` - used as the insert argument to the confirmed
/// find-or-insert helper (see `category_map_find_or_insert`).
#[repr(C)]
struct CategoryKv {
    key: i32,
    value: f32,
}

impl ZTMegatile {
    pub fn guest_count(&self) -> i32 {
        self.guest_count
    }

    pub fn stink(&self) -> f32 {
        self.stink
    }

    /// Read-only BST lower-bound walk over `category_map`, mirroring
    /// `recalculateCharacteristics`'s own inlined traversal exactly: starting from the header's
    /// `parent` (the tree root), follow `right` while `node.key < category_id`, else record `node` as
    /// the current candidate and follow `left`. `None` if the walk lands on the header sentinel itself
    /// or a non-matching key - matching the decompile's own `(pcStack_40 == head) || (search_key <
    /// candidate.key)` "not found" check.
    ///
    /// Never allocates or frees anything, so this is safe to call against the real, vanilla-owned tree
    /// regardless of how well-confirmed `size`/`_color_isnil`/the allocator/comparator fields are - see
    /// the module doc comment.
    pub fn category_value(&self, category_id: i32) -> Option<f32> {
        let head = self.category_map.head;
        if head.is_null() {
            return None;
        }
        let mut node = unsafe { (*head).parent };
        let mut candidate = head;
        // A real red-black tree of this size is never more than a few dozen nodes deep; this cap only
        // exists to turn a wrong left/right offset guess into a graceful `None` instead of a live hang -
        // see the module's own `_reserved`/`_color_isnil` confidence caveats and the
        // `ZTMEGATILE_CATEGORY_MAP_LAYOUT` live test.
        for _ in 0..256 {
            if node.is_null() {
                break;
            }
            let n = unsafe { &*node };
            if n.key < category_id {
                node = n.right;
            } else {
                candidate = node;
                node = n.left;
            }
        }
        if candidate == head {
            return None;
        }
        let c = unsafe { &*candidate };
        if category_id < c.key {
            None
        } else {
            Some(c.value)
        }
    }
}

impl ZTMegatileMgr {
    pub fn is_dirty(&self) -> bool {
        self.dirty != 0
    }

    pub fn tick_accumulator(&self) -> u32 {
        self.tick_accumulator
    }

    /// Number of outer-vector (x/5) columns currently allocated.
    pub fn megatile_columns(&self) -> usize {
        (self.row_end as usize - self.row_start as usize) / mem::size_of::<MegatileRow>()
    }

    fn row(&self, column: usize) -> Option<&MegatileRow> {
        if column >= self.megatile_columns() {
            return None;
        }
        Some(unsafe { &*self.row_start.add(column) })
    }

    /// Number of inner-vector (y/5) entries allocated for `column`, or `0` if `column` is out of range.
    pub fn megatile_rows_in_column(&self, column: usize) -> usize {
        self.row(column).map(MegatileRow::len).unwrap_or(0)
    }

    /// Bounds-checked read accessor for a single megatile.
    pub fn megatile(&self, column: usize, row: usize) -> Option<&ZTMegatile> {
        let r = self.row(column)?;
        if row >= r.len() {
            return None;
        }
        Some(unsafe { &*r.start.add(row) })
    }

    /// Pure tick/dirty-flag transition logic behind [`update`](Self::update), isolated for unit testing
    /// without touching real memory. Per `ZTMegatileMgr_update.c`: the delta is added to the
    /// accumulator unconditionally; crossing the `0x1d4b` (7499) threshold sets `dirty`; if `dirty` ends
    /// up set (whether from this call or already set beforehand, e.g. by `init`), the accumulator resets
    /// to `0` and a recalculation is due. Returns `(new_tick_accumulator, new_dirty, should_recalculate)`.
    pub(crate) fn compute_update_state(tick_accumulator: u32, dirty: bool, delta_ticks: u32) -> (u32, bool, bool) {
        let accumulated = tick_accumulator.wrapping_add(delta_ticks);
        let dirty = dirty || accumulated > 0x1d4b;
        if dirty {
            (0, true, true)
        } else {
            (accumulated, false, false)
        }
    }

    /// Reimplementation of `OOAnalyzer::ZTMegatileMgr::update`, per `ZTMegatileMgr_update.c`.
    pub fn update(&mut self, delta_ticks: u32) {
        let (new_accumulator, new_dirty, should_recalculate) = Self::compute_update_state(self.tick_accumulator, self.dirty != 0, delta_ticks);
        self.tick_accumulator = new_accumulator;
        self.dirty = new_dirty as u8;
        if should_recalculate {
            self.recalculate_characteristics();
        }
    }

    /// Reimplementation of `OOAnalyzer::ZTMegatileMgr::recalculateCharacteristics`, per
    /// `ZTMegatileMgr_recalculateCharacteristics.c`. Two passes over the live map:
    ///
    /// 1. Reset every currently-allocated megatile: `guest_count = 0`, `category_map.clear()` (via
    ///    [`category_map_clear`]), `stink = 0.0`.
    /// 2. For every real map tile `(x, y)`, walk its guest-occupant list (an intrusive circular list
    ///    whose sentinel pointer lives at the tile's own `+0x0`, node shape `{next, prev, entity_ptr}`)
    ///    counting residents whose entity type passes the `ZTGuestType` check
    ///    (`DAT_00638700`/`RVA_GUEST_TYPE_CHECK_ARG`, via the same vtable-slot-`0x1c` "isKindOf" pattern
    ///    as `ztthoughtmgr::resolve_object_own_habitat_ptr`), then for each of the tile's 4 "corner"
    ///    entity pointers (`+0x4`/`+0x8`/`+0xc`/`+0x10`) whose type passes the `ZTSceneryType`-ish check
    ///    (`DAT_00638670`), accumulates that entity type's per-category `BFCategory::getValue`
    ///    (`bfcategory::GET_VALUE`) divided by the entity's own footprint divisor (`+0x150`) into the
    ///    owning megatile's `category_map`, plus its `stink`-source field (`+0x11c`) into `stink` the
    ///    same way.
    ///
    /// Only the not-found branch of the per-category accumulation calls through to vanilla (the
    /// find-or-insert helper, [`accumulate_category_value`]) - once a node exists, writing its `value`
    /// field is a plain memory write, safe regardless of allocator (see the module doc comment).
    pub fn recalculate_characteristics(&mut self) {
        for column in 0..self.megatile_columns() {
            let row = unsafe { &*self.row_start.add(column) };
            for i in 0..row.len() {
                let megatile = unsafe { &mut *row.start.add(i) };
                megatile.guest_count = 0;
                unsafe { category_map_clear(&mut megatile.category_map) };
                megatile.stink = 0.0;
            }
        }

        let world = globals().ztworldmgr();
        let map_x_size = world.map_x_size;
        let map_y_size = world.map_y_size;

        for y in 0..map_y_size {
            for x in 0..map_x_size {
                let Some(tile) = world.get_tile_from_pos(IVec3::new(x as i32, y as i32, 0)) else {
                    continue;
                };
                let tile_addr = world.get_ptr_from_bftile(&tile);
                let Some(megatile) = self.megatile_mut((x / 5) as usize, (y / 5) as usize) else {
                    continue;
                };

                let sentinel = get_from_memory::<u32>(tile_addr);
                if sentinel != 0 {
                    let mut node = get_from_memory::<u32>(sentinel);
                    while node != sentinel {
                        let entity_ptr = get_from_memory::<u32>(node + 0x8);
                        if entity_ptr != 0 && unsafe { entity_type_matches(entity_ptr, RVA_GUEST_TYPE_CHECK_ARG) } {
                            megatile.guest_count += 1;
                        }
                        node = get_from_memory::<u32>(node);
                    }
                }

                for corner_offset in [0x4u32, 0x8, 0xc, 0x10] {
                    let corner_entity_ptr = get_from_memory::<u32>(tile_addr + corner_offset);
                    if corner_entity_ptr == 0 {
                        continue;
                    }
                    let entity_type_ptr = get_from_memory::<u32>(corner_entity_ptr + 0x128);
                    if entity_type_ptr == 0 || !unsafe { entity_type_matches(corner_entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) } {
                        continue;
                    }
                    let divisor = get_from_memory::<i32>(corner_entity_ptr + 0x150) as f32;

                    for category_id in 0x251fi32..0x2523 {
                        // `this` is `entity_type_ptr + 0x154` (not `entity_type_ptr` itself), and the
                        // category id is passed by value - both confirmed directly from
                        // `ZTMegatileMgr_recalculateCharacteristics.asm` (`LEA EBP,[EBX+0x154]` then
                        // `MOV ECX,EBP` / `MOV EAX,[ESP+0x10]` then `PUSH EAX` at the `getValue` call
                        // site), not from the (less reliable) decompiled C's `piVar11+0x55` rendering.
                        let raw_value = unsafe { GET_VALUE.original()((entity_type_ptr + 0x154) as *const u32, category_id) };
                        let delta = raw_value as f32 / divisor;
                        unsafe { accumulate_category_value(&mut megatile.category_map, category_id, delta) };
                    }
                    megatile.stink += get_from_memory::<i32>(entity_type_ptr + 0x11c) as f32 / divisor;
                }
            }
        }

        self.dirty = 0;
    }

    fn megatile_mut(&mut self, column: usize, row: usize) -> Option<&mut ZTMegatile> {
        if column >= self.megatile_columns() {
            return None;
        }
        let r = unsafe { &*self.row_start.add(column) };
        if row >= r.len() {
            return None;
        }
        Some(unsafe { &mut *r.start.add(row) })
    }

    /// Reimplementation of `OOAnalyzer::ZTMegatileMgr::init`, per `ZTMegatileMgr_init.c`. Resizes the
    /// outer `vector<vector<ZTMegatile>>` to `tile_y_count` columns, then each column's inner
    /// `vector<ZTMegatile>` to `tile_x_count` entries - transposed relative to
    /// `recalculate_characteristics`'s own x/5-outer, y/5-inner indexing (see that method's doc
    /// comment), but preserved exactly as the decompile has it: a faithful translation of vanilla's own
    /// behavior rather than something this reimplementation tries to "fix".
    ///
    /// The riskiest code in this file: the outer/inner vector resize helpers
    /// ([`outer_vector_erase`]/[`outer_vector_insert_n`]/[`inner_vector_erase_tail`]/
    /// [`inner_vector_insert_n`]) have calling conventions reconstructed from a decompile that reuses
    /// the same stack slots for multiple, logically-unrelated purposes across the function - see each
    /// helper's own doc comment for specifics.
    pub fn init(&mut self, tile_x_count: i32, tile_y_count: i32) {
        let outer_target = tile_y_count.max(0) as usize;
        let current_outer = self.megatile_columns();
        if outer_target < current_outer {
            unsafe {
                outer_vector_erase(self, self.row_start.add(outer_target), self.row_end);
            }
        } else if outer_target > current_outer {
            let fill = MegatileRow { start: std::ptr::null_mut(), end: std::ptr::null_mut(), capacity_end: std::ptr::null_mut() };
            unsafe {
                outer_vector_insert_n(self, self.row_end, (outer_target - current_outer) as u32, &fill);
            }
        }

        // Guards on `tile_y_count` (`param_2`, the just-applied outer/column target), not
        // `tile_x_count` - confirmed in `ZTMegatileMgr_init.asm` (`CMP EBP,EBX; JLE ...` where `EBP`
        // holds `param_2` and `EBX` is zero). A `tile_x_count < 1` check here would skip the per-column
        // inner-vector resize even when `tile_y_count >= 1`, leaving stale inner vectors behind -
        // vanilla only skips it when there are no columns to resize in the first place.
        if tile_y_count < 1 {
            self.dirty = 1;
            return;
        }

        let inner_target = tile_x_count as usize;
        for column in 0..self.megatile_columns() {
            let row = unsafe { &mut *self.row_start.add(column) };
            let current_inner = row.len();
            if inner_target < current_inner {
                unsafe {
                    inner_vector_erase_tail(row, inner_target);
                }
            } else if inner_target > current_inner {
                // A null value pointer here makes the callee skip per-element construction entirely,
                // leaving new slots as raw uninitialized allocator memory - confirmed by disassembling
                // the fast-path helper (`0x4c8962`) the insert dispatcher calls into: it does
                // `test esi,esi; je <skip>` on the value pointer before the per-element
                // construct-from-value call. A `category_map.head: null` within that value isn't safe
                // either - see [`empty_category_map_sentinel`]'s own doc comment. The fill value needs a
                // real empty-tree sentinel for `head` (`parent: null`, `left`/`right` self-referential).
                let fill = ZTMegatile { guest_count: 0, category_map: MapHeader { head: empty_category_map_sentinel(), size: 0, _reserved: 0 }, stink: 0.0 };
                unsafe {
                    inner_vector_insert_n(row, (inner_target - current_inner) as u32, &fill);
                }
            }
        }

        self.dirty = 1;
    }
}

/// Address of `BFCategory::getValue`'s target-type check argument for the guest-occupant list walk in
/// `recalculateCharacteristics` (`&DAT_00638700` in the decompile) - an opaque RTTI-style type-check
/// argument, same mechanism as `ztthoughtmgr::resolve_object_own_habitat_ptr`'s own `DAT_00638690`.
/// RVA = `0x00638700 - 0x400000`.
const RVA_GUEST_TYPE_CHECK_ARG: u32 = 0x0023_8700;

/// Same mechanism as [`RVA_GUEST_TYPE_CHECK_ARG`], for the "corner entity" scenery-type check
/// (`&DAT_00638670` in the decompile). RVA = `0x00638670 - 0x400000`.
pub(crate) const RVA_SCENERY_TYPE_CHECK_ARG: u32 = 0x0023_8670;

/// Shared "does this entity's type pass vanilla's isKindOf-style check" helper, used for both the
/// guest-occupant check and the corner-entity scenery check in `recalculateCharacteristics` - both call
/// sites in the decompile are byte-for-byte identical apart from the type-check argument, and structurally
/// identical to `ztthoughtmgr::resolve_object_own_habitat_ptr`'s own vtable-slot-`0x1c` call. `entity_ptr`
/// is a raw `BFEntity*`; this resolves its `inner_class_ptr` (`+0x128`) itself before dispatching.
pub(crate) unsafe fn entity_type_matches(entity_ptr: u32, type_check_arg_rva: u32) -> bool {
    let entity_type_ptr = get_from_memory::<u32>(entity_ptr + 0x128);
    if entity_type_ptr == 0 {
        return false;
    }
    let entity_type_vtable = get_from_memory::<u32>(entity_type_ptr);
    let type_check_fn = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(entity_type_vtable + 0x1c)) };
    let arg = get_module_base("zoo.exe") as u32 + type_check_arg_rva;
    type_check_fn(entity_type_ptr, arg)
}

/// Vanilla `std::map<int,float>::clear()`, confirmed via `BFTile::meth_0x41e7f6` in
/// `ZTMegatileMgr_recalculateCharacteristics.c` (address `0x0041e7f6`; cross-checked against the macOS
/// build's clean, named `_clear__...__tree<...>` call at the equivalent call site). RVA =
/// `0x0041e7f6 - 0x400000`.
const RVA_CATEGORY_MAP_CLEAR: u32 = 0x0001_e7f6;

unsafe fn category_map_clear(map: &mut MapHeader) {
    let clear_fn = unsafe { mem::transmute::<u32, extern "thiscall" fn(*mut MapHeader)>(get_module_base("zoo.exe") as u32 + RVA_CATEGORY_MAP_CLEAR) };
    clear_fn(map as *mut MapHeader);
}

/// A `head: null` `MapHeader` is *not* a safe "empty map" to hand to vanilla's own map/tree code as a
/// copy-construction source: the `category_map` copy-constructor (`zoo.exe` RVA `0xdd35f`, called from
/// the "sufficient capacity" insert path at `0x5cccb7`) does `mov edx,[ecx]; cmp [edx+4],edi`,
/// unconditionally dereferencing `head` (`edx`) to read its `parent` field before ever checking `size`,
/// unlike [`category_map_clear`]. That `parent == NULL` comparison is the copy-ctor's actual "is the
/// source subtree empty" check, so `parent` must be genuinely null (not self-referential) for the
/// empty-tree fast path to be taken; a self-referential `parent` gets walked as if it were a real data
/// node instead. `left`/`right` are left self-referential to match a real empty-tree header's usual
/// shape, though this copy-ctor's own check never inspects them.
///
/// Leaked (`'static`, so it outlives every call that might reference it - see `CLAUDE.md`'s
/// cross-allocator warning: this is read-only data handed to vanilla to copy *from*, never freed by
/// either side, so there is no Box-vs-vanilla-freelist hazard), and reused for every call.
fn empty_category_map_sentinel() -> *mut TreeNode {
    use std::sync::OnceLock;
    static SENTINEL: OnceLock<usize> = OnceLock::new();
    *SENTINEL.get_or_init(|| {
        let node = Box::leak(Box::new(TreeNode { _color_isnil: 1, parent: std::ptr::null_mut(), left: std::ptr::null_mut(), right: std::ptr::null_mut(), key: 0, value: 0.0 }));
        let ptr = node as *mut TreeNode;
        node.left = ptr;
        node.right = ptr;
        ptr as usize
    }) as *mut TreeNode
}

/// Vanilla `std::map<int,float>`'s hinted find-or-insert, confirmed via `BFTile::meth_0x40a01d`
/// (address `0x0040a01d`) - both its call site in `ZTMegatileMgr_recalculateCharacteristics.c` and the
/// smaller, clearer `BFCategory_setFromIntList.c` (`OOAnalyzer::BFTile::meth_0x40a01d((BFTile*)this,
/// &local_c, local_1c, &local_8)`) show the same 4-arg shape: `(map, sret_out, hint_node,
/// &key_value_pair)`, matching MSVC's hidden-sret-as-first-explicit-arg convention for a thiscall member
/// returning `pair<iterator,bool>` by value - the sret pointer is also returned in EAX, so callers that
/// only need the found/inserted node (as this file does) can read it directly out of the buffer they
/// passed rather than trust the return value. RVA = `0x0040a01d - 0x400000`.
const RVA_CATEGORY_MAP_FIND_OR_INSERT: u32 = 0x0000_a01d;

/// Performs `recalculate_characteristics`'s find-or-insert-then-accumulate step for one category id:
/// re-runs the same lower-bound walk [`ZTMegatile::category_value`] uses (natively, no allocation risk)
/// to find an existing node or the insertion hint; only calls through to vanilla
/// ([`RVA_CATEGORY_MAP_FIND_OR_INSERT`]) when the key doesn't already exist. Writing an existing node's
/// `value` field is a plain memory write regardless of which branch found it - safe against either
/// allocator, per the module doc comment.
unsafe fn accumulate_category_value(map: &mut MapHeader, category_id: i32, delta: f32) {
    let head = map.head;
    if head.is_null() {
        return;
    }
    let mut node = unsafe { (*head).parent };
    let mut candidate = head;
    // See ZTMegatile::category_value's own doc comment for why this loop is capped.
    for _ in 0..256 {
        if node.is_null() {
            break;
        }
        let n = unsafe { &*node };
        if n.key < category_id {
            node = n.right;
        } else {
            candidate = node;
            node = n.left;
        }
    }

    let target = if candidate == head || category_id < unsafe { (*candidate).key } {
        let kv = CategoryKv { key: category_id, value: 0.0 };
        let mut sret_buf = [0u32; 2];
        let find_or_insert_fn = unsafe {
            mem::transmute::<u32, extern "thiscall" fn(*mut MapHeader, *mut u32, *mut TreeNode, *const CategoryKv) -> *mut u32>(
                get_module_base("zoo.exe") as u32 + RVA_CATEGORY_MAP_FIND_OR_INSERT,
            )
        };
        find_or_insert_fn(map as *mut MapHeader, sret_buf.as_mut_ptr(), candidate, &kv as *const CategoryKv);
        sret_buf[0] as *mut TreeNode
    } else {
        candidate
    };

    if !target.is_null() {
        unsafe { (*target).value += delta };
    }
}

/// Outer `vector<MegatileRow>::erase(first, last)`, called by `init()`'s shrink branch. Thiscall member
/// signature `(this, first, last)` from `ZTMegatileMgr_init.c`'s `FUN_0047cea0(this_00, erase_start,
/// old_end)` call - confirmed directly in `ZTMegatileMgr_init.asm`: `this` (`ECX`) is set from
/// `LEA ESI,[ECX_orig+0x18]` (i.e. `&mgr->row_start`, the address of the embedded
/// `vector<MegatileRow>` header at offset `0x18`), **not** the `ZTMegatileMgr*` itself - passing the
/// manager pointer directly makes the callee read/write `vtable`/`flag`/`dirty`/`tick_accumulator` as if
/// they were `begin`/`end`/`capacity_end`, corrupting the manager without touching the real vector
/// header. Address `0x0047cea0`, RVA `0x0007cea0`.
unsafe fn outer_vector_erase(mgr: &mut ZTMegatileMgr, first: *mut MegatileRow, last: *mut MegatileRow) {
    let erase_fn = unsafe {
        mem::transmute::<u32, extern "thiscall" fn(*mut *mut MegatileRow, *mut MegatileRow, *mut MegatileRow)>(get_module_base("zoo.exe") as u32 + 0x0007_cea0)
    };
    erase_fn(&mut mgr.row_start as *mut *mut MegatileRow, first, last);
}

/// Outer `vector<MegatileRow>::insert(pos, n, value)`, called by `init()`'s grow branch. Thiscall member
/// signature `(this, pos, n, &value)` from `ZTMegatileMgr_init.c`'s `FUN_0058e9a0(this_00, old_end,
/// count, &fill)` call - same `this = &mgr->row_start` (offset `0x18`) correction as
/// [`outer_vector_erase`], confirmed in the same `.asm` block (`MOV ECX,ESI` immediately before the
/// call, `ESI` still holding `ECX_orig+0x18` from the shared prologue). Address `0x0058e9a0`, RVA
/// `0x0018e9a0`.
unsafe fn outer_vector_insert_n(mgr: &mut ZTMegatileMgr, pos: *mut MegatileRow, n: u32, value: &MegatileRow) {
    let insert_fn = unsafe {
        mem::transmute::<u32, extern "thiscall" fn(*mut *mut MegatileRow, *mut MegatileRow, u32, *const MegatileRow)>(
            get_module_base("zoo.exe") as u32 + 0x0018_e9a0,
        )
    };
    insert_fn(&mut mgr.row_start as *mut *mut MegatileRow, pos, n, value as *const MegatileRow);
}

/// Inner `vector<ZTMegatile>` tail-erase (shrink to `new_len`), reassembled from
/// `ZTMegatileMgr_init.c`'s manually-inlined shrink branch: `FUN_0047cdfe(old_end, old_end, new_end)`
/// (a `std::copy_backward`-shaped free function - confirmed reasonably well, since for a tail erase the
/// shift is a no-op and the result equals `new_end`, matching the decompile's own use of its return
/// value as the destroy-loop start), then destroys each trailing element via `FUN_0047cda7(elem, 0)`
/// (confirmed live at `0x14`-byte/one-`ZTMegatile` stride in the same loop), then updates `row.end`
/// directly. Addresses `0x0047cdfe`/`0x0047cda7`, RVAs `0x0007cdfe`/`0x0007cda7`.
unsafe fn inner_vector_erase_tail(row: &mut MegatileRow, new_len: usize) {
    let copy_backward_fn = unsafe {
        mem::transmute::<u32, extern "cdecl" fn(*mut ZTMegatile, *mut ZTMegatile, *mut ZTMegatile) -> *mut ZTMegatile>(
            get_module_base("zoo.exe") as u32 + 0x0007_cdfe,
        )
    };
    let destroy_fn = unsafe { mem::transmute::<u32, extern "thiscall" fn(*mut ZTMegatile, i32)>(get_module_base("zoo.exe") as u32 + 0x0007_cda7) };

    let new_end = unsafe { row.start.add(new_len) };
    let erase_start = copy_backward_fn(row.end, row.end, new_end);
    let mut elem = erase_start;
    while elem != row.end {
        destroy_fn(elem, 0);
        elem = unsafe { elem.add(1) };
    }
    row.end = erase_start;
}

/// Inner `vector<ZTMegatile>::insert(end(), n, value)`. `ZTMegatileMgr_init.c` renders the value
/// argument as `&local_14` (`undefined1 local_14 [4]`, unconditionally zeroed just before the call),
/// not the unrelated `local_10`/`cls_0x4012a6` tree scratch object a few lines above. The insert
/// dispatcher's fast-path helper (`0x4c8962`) does `test esi,esi; je <skip-construct>` on the value
/// pointer (null skips per-element construction, leaving raw allocator memory), and any non-null
/// pointer - including `&local_14` - reaches a real per-element construct-from-value call reading a
/// full `ZTMegatile`-shaped source. Vanilla's own `local_14` therefore over-reads into whatever's
/// adjacent to it on the stack for the trailing bytes; this reimplementation instead passes a genuinely
/// valid, correctly-sized, zeroed `ZTMegatile` (`category_map` `head: null, size: 0`) so the
/// copy-construct path sees a well-defined empty source. Address `0x004c8489`, RVA `0x000c8489`.
unsafe fn inner_vector_insert_n(row: &mut MegatileRow, n: u32, value: &ZTMegatile) {
    let insert_fn = unsafe {
        mem::transmute::<u32, extern "thiscall" fn(*mut MegatileRow, *mut ZTMegatile, u32, *const ZTMegatile)>(get_module_base("zoo.exe") as u32 + 0x000c_8489)
    };
    insert_fn(row as *mut MegatileRow, row.end, n, value as *const ZTMegatile);
}

#[detour_mod]
mod megatilemgr_detours {
    use super::*;
    use crate::util::mut_from_memory;

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, delta_ticks: i32) {
        unsafe { mut_from_memory::<ZTMegatileMgr>(this) }.update(delta_ticks as u32);
    }

    #[detour(RECALCULATE_CHARACTERISTICS)]
    unsafe extern "thiscall" fn recalculate_characteristics(this: *const u32) {
        unsafe { mut_from_memory::<ZTMegatileMgr>(this) }.recalculate_characteristics();
    }

    #[detour(INIT)]
    unsafe extern "thiscall" fn init(this: *const u32, tile_x_count: u32, tile_y_count: u32) {
        unsafe { mut_from_memory::<ZTMegatileMgr>(this) }.init(tile_x_count as i32, tile_y_count as i32);
    }
}

/// Registers this module's live detours. Deliberately does **not** detour the destructor
/// (`ZTMEGATILE_MGR_1` in `generated.rs`) - unlike `ztthoughtmgr`'s dtor detour (needed because that
/// class allocates `Box`-owned nodes), this module never allocates Rust-owned memory anywhere (see the
/// module doc comment), so there is nothing for a Rust dtor hook to free.
pub fn init() {
    if let Err(e) = unsafe { megatilemgr_detours::init_detours() } {
        error!("Failed to initialise ztmegatilemgr detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`. Mirrors `ztthoughtmgr::live_support`'s
/// shape/gating - unlike that module, there is nothing to `Box`-allocate or leak here at all (see the
/// module doc comment), so these helpers only ever snapshot/compare the real singleton's own
/// vanilla-owned memory.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// Restores the live singleton's `dirty`/`tick_accumulator` scalars - used by
    /// `ZTMEGATILEMGR_UPDATE` to reset state between the real and reimplemented calls under test.
    pub(crate) fn restore_scalars(mgr: &mut ZTMegatileMgr, dirty: bool, tick_accumulator: u32) {
        mgr.dirty = dirty as u8;
        mgr.tick_accumulator = tick_accumulator;
    }

    /// A snapshot of every currently-allocated megatile's `guest_count`/`stink`, plus the grid's own
    /// shape (columns, rows-per-column) - the diff mechanism `ZTMEGATILEMGR_RECALCULATE_CHARACTERISTICS`
    /// uses to compare the real vanilla call against the reimplementation, since `category_map`'s raw
    /// tree contents aren't directly comparable field-by-field without the same lower-bound walk
    /// `category_value` already performs (used instead - see `snapshot_categories`).
    #[derive(Debug, PartialEq)]
    pub(crate) struct GridSnapshot {
        pub(crate) columns: Vec<Vec<(i32, f32)>>,
    }

    pub(crate) fn snapshot_grid(mgr: &ZTMegatileMgr) -> GridSnapshot {
        let columns = (0..mgr.megatile_columns())
            .map(|column| (0..mgr.megatile_rows_in_column(column)).map(|row| {
                let mt = mgr.megatile(column, row).expect("row within bounds");
                (mt.guest_count(), mt.stink())
            }).collect())
            .collect();
        GridSnapshot { columns }
    }

    /// Snapshot of `category_value(key)` for every `key` in `keys`, for every currently-allocated
    /// megatile - the `ZTMEGATILE_CATEGORY_MAP_LAYOUT` test's actual comparison mechanism.
    pub(crate) fn snapshot_categories(mgr: &ZTMegatileMgr, keys: &[i32]) -> Vec<Vec<Option<f32>>> {
        (0..mgr.megatile_columns())
            .flat_map(|column| (0..mgr.megatile_rows_in_column(column)).map(move |row| (column, row)))
            .map(|(column, row)| {
                let mt = mgr.megatile(column, row).expect("row within bounds");
                keys.iter().map(|&k| mt.category_value(k)).collect()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_state_accumulates_below_threshold() {
        let (acc, dirty, recalc) = ZTMegatileMgr::compute_update_state(0, false, 100);
        assert_eq!(acc, 100);
        assert!(!dirty);
        assert!(!recalc);
    }

    #[test]
    fn update_state_crosses_threshold() {
        let (acc, dirty, recalc) = ZTMegatileMgr::compute_update_state(0x1d00, false, 0x100);
        assert_eq!(acc, 0);
        assert!(dirty);
        assert!(recalc);
    }

    #[test]
    fn update_state_exact_threshold_does_not_trigger() {
        // Decompile uses a strict `<` comparison (`0x1d4b < uVar1`), so landing exactly on the
        // threshold does not set dirty.
        let (acc, dirty, recalc) = ZTMegatileMgr::compute_update_state(0, false, 0x1d4b);
        assert_eq!(acc, 0x1d4b);
        assert!(!dirty);
        assert!(!recalc);
    }

    #[test]
    fn update_state_one_past_threshold_triggers() {
        let (acc, dirty, recalc) = ZTMegatileMgr::compute_update_state(0, false, 0x1d4c);
        assert_eq!(acc, 0);
        assert!(dirty);
        assert!(recalc);
    }

    #[test]
    fn update_state_already_dirty_recalculates_even_below_threshold() {
        let (acc, dirty, recalc) = ZTMegatileMgr::compute_update_state(0, true, 1);
        assert_eq!(acc, 0);
        assert!(dirty);
        assert!(recalc);
    }

    fn empty_mgr() -> ZTMegatileMgr {
        ZTMegatileMgr {
            vtable: 0,
            flag: 0,
            _pad0: [0; 3],
            dirty: 0,
            _pad1: [0; 3],
            tick_accumulator: 0,
            _reserved_0x10: [0; 8],
            row_start: std::ptr::null_mut(),
            row_end: std::ptr::null_mut(),
            row_capacity_end: std::ptr::null_mut(),
        }
    }

    #[test]
    fn empty_grid_has_no_columns() {
        let mgr = empty_mgr();
        assert_eq!(mgr.megatile_columns(), 0);
        assert_eq!(mgr.megatile_rows_in_column(0), 0);
        assert!(mgr.megatile(0, 0).is_none());
    }

    /// Builds a small fixture grid (1 column x 2 rows) out of leaked boxes - mirrors
    /// `zthabitatmgr`/`ztworldmgr`'s own `new_for_test` fixture convention (acceptable one-time leak for
    /// short-lived unit tests).
    fn fixture_mgr() -> ZTMegatileMgr {
        let megatiles: &'static mut [ZTMegatile] = Box::leak(Box::new([
            ZTMegatile { guest_count: 3, category_map: MapHeader { head: std::ptr::null_mut(), size: 0, _reserved: 0 }, stink: 1.5 },
            ZTMegatile { guest_count: 7, category_map: MapHeader { head: std::ptr::null_mut(), size: 0, _reserved: 0 }, stink: 2.5 },
        ]));
        let start = megatiles.as_mut_ptr();
        let end = unsafe { start.add(megatiles.len()) };
        let rows: &'static mut [MegatileRow] = Box::leak(Box::new([MegatileRow { start, end, capacity_end: end }]));
        let row_start = rows.as_mut_ptr();
        let row_end = unsafe { row_start.add(rows.len()) };
        let mut mgr = empty_mgr();
        mgr.row_start = row_start;
        mgr.row_end = row_end;
        mgr.row_capacity_end = row_end;
        mgr
    }

    #[test]
    fn accessors_read_fixture_grid() {
        let mgr = fixture_mgr();
        assert_eq!(mgr.megatile_columns(), 1);
        assert_eq!(mgr.megatile_rows_in_column(0), 2);
        assert_eq!(mgr.megatile(0, 0).unwrap().guest_count(), 3);
        assert_eq!(mgr.megatile(0, 1).unwrap().guest_count(), 7);
        assert!(mgr.megatile(0, 2).is_none());
        assert!(mgr.megatile(1, 0).is_none());
    }

    #[test]
    fn category_value_on_empty_map_is_none() {
        let mt = ZTMegatile { guest_count: 0, category_map: MapHeader { head: std::ptr::null_mut(), size: 0, _reserved: 0 }, stink: 0.0 };
        assert_eq!(mt.category_value(9503), None);
    }

    #[test]
    fn category_value_walks_a_hand_built_tree() {
        // Small hand-built tree: root=9504(val=2.0), left=9503(val=1.0), right=9505(val=3.0).
        let mut root = TreeNode { _color_isnil: 0, parent: std::ptr::null_mut(), left: std::ptr::null_mut(), right: std::ptr::null_mut(), key: 9504, value: 2.0 };
        let mut left = TreeNode { _color_isnil: 0, parent: std::ptr::null_mut(), left: std::ptr::null_mut(), right: std::ptr::null_mut(), key: 9503, value: 1.0 };
        let mut right = TreeNode { _color_isnil: 0, parent: std::ptr::null_mut(), left: std::ptr::null_mut(), right: std::ptr::null_mut(), key: 9505, value: 3.0 };
        root.left = &mut left as *mut TreeNode;
        root.right = &mut right as *mut TreeNode;
        let mut header = TreeNode { _color_isnil: 0, parent: &mut root as *mut TreeNode, left: std::ptr::null_mut(), right: std::ptr::null_mut(), key: 0, value: 0.0 };
        let map = MapHeader { head: &mut header as *mut TreeNode, size: 3, _reserved: 0 };
        let mt = ZTMegatile { guest_count: 0, category_map: map, stink: 0.0 };

        assert_eq!(mt.category_value(9503), Some(1.0));
        assert_eq!(mt.category_value(9504), Some(2.0));
        assert_eq!(mt.category_value(9505), Some(3.0));
        assert_eq!(mt.category_value(9502), None);
        assert_eq!(mt.category_value(9506), None);
    }
}
