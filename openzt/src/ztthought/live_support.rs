use crate::globals::get_module_base;
use super::thought::ZTThought;
use super::mgr::ztthoughtmgr::{ZTThoughtMgr, THOUGHT_STORES};

/// A persistent-list node: 8 bytes of intrusive links followed by the `ZTThought` payload at `+0x8`.
/// The sentinel node vanilla allocates at startup shares this same link layout (its `data` is never
/// read). Test-only: no production code walks this layout.
#[repr(C)]
pub struct ThoughtNode {
    next: *mut ThoughtNode,
    prev: *mut ThoughtNode,
    data: ZTThought,
}

/// Builds a `ZTThought` with every field directly settable - unlike `ZTThought::new`, which
/// dereferences `thinker_ptr`/`object_ptr`/`habitat_arg` when non-null to resolve
/// `thinker_id`/`object_id`/the habitat-flag gate. Several live comparisons need fields set
/// directly without running that resolution logic.
///
/// `vtable` is set to the same real `ZTThought` vtable address `ZTThought::new` itself uses, not
/// `0`: `ZTThoughtMgr::save` dispatches through each node's own `data.vtable` slot 0 rather than
/// calling `ZTThought::save` directly, so every node reachable from a real vanilla call needs a
/// genuinely valid vtable, not just correct data fields.
// The args are `ZTThought`'s own flat field list in declaration order (minus `vtable`, which this
// sets itself) - a params struct would just wrap the same eight values.
#[allow(clippy::too_many_arguments)]
pub fn new_thought(
    string_id: u32,
    thinker_id: u32,
    object_id: u32,
    tile_x: i32,
    tile_y: i32,
    thinker_ptr: u32,
    object_ptr: u32,
    habitat_ptr: u32,
) -> ZTThought {
    let vtable = get_module_base("zoo.exe") as u32 + 0x0023_5400;
    ZTThought { vtable, string_id, thinker_id, object_id, tile_x, tile_y, thinker_ptr, object_ptr, habitat_ptr }
}

/// Builds a standalone `ZTThoughtMgr` with a freshly heap-allocated, self-referencing sentinel node,
/// never spliced into the real singleton. Heap-allocates the `ZTThoughtMgr` itself too (returned as
/// a raw pointer, for passing to real vanilla `.original()()` calls). The sentinel is a genuine,
/// self-referencing `ThoughtNode` (not just an opaque placeholder): any real, undetoured vanilla call
/// against this instance (`ADD_THOUGHT.original()`, `GET_THOUGHTS_BY_THINKER.original()`, ...) reads
/// `sentinel_ptr` as a real intrusive-list pointer and needs it to be one. Reimplemented-side methods
/// never dereference it, only using its value as a [`THOUGHT_STORES`] key, so the exact same
/// construction is safe and sufficient for both real and reimplemented standalone instances.
pub fn build_standalone_mgr(max_thoughts: u32) -> *mut ZTThoughtMgr {
    let sentinel = Box::into_raw(Box::new(ThoughtNode {
        next: std::ptr::null_mut(),
        prev: std::ptr::null_mut(),
        data: new_thought(0, 0, 0, -1, -1, 0, 0, 0),
    }));
    unsafe {
        (*sentinel).next = sentinel;
        (*sentinel).prev = sentinel;
    }
    Box::into_raw(Box::new(ZTThoughtMgr { vtable: 0, flag: 0, _pad: [0; 3], sentinel_ptr: sentinel as u32, max_thoughts }))
}

/// Splices `thought` in as a new `Box`-owned node at the front of `mgr`'s *raw* `sentinel_ptr`
/// chain, bypassing [`THOUGHT_STORES`] entirely - use this to seed the "real" side of a live
/// comparison that will drive `mgr` through a genuine, undetoured `.original()` call (which reads
/// `sentinel_ptr` directly and knows nothing about our Rust-side store).
pub fn seed_raw_chain(mgr: &ZTThoughtMgr, thought: ZTThought) {
    let sentinel = mgr.sentinel_ptr as *mut ThoughtNode;
    let old_front = unsafe { (*sentinel).next };
    let node = Box::into_raw(Box::new(ThoughtNode { next: old_front, prev: sentinel, data: thought }));
    unsafe {
        (*old_front).prev = node;
        (*sentinel).next = node;
    }
}

/// Walks a raw, sentinel-terminated `ThoughtNode` chain starting from `sentinel_ptr`, front-to-back,
/// returning owned copies. Used both for `mgr`'s own persistent-list chain (via [`read_raw_chain`])
/// and for a vanilla-allocated temporary output list a `getThoughtsBy*` `.original()` call wrote its
/// sentinel into (the two share the same node layout, only the allocator differs). Never mutates or
/// frees anything - safe regardless of which allocator produced the chain.
pub fn read_raw_chain_from_sentinel(sentinel_ptr: u32) -> Vec<ZTThought> {
    let sentinel = sentinel_ptr as *const ThoughtNode;
    let mut result = Vec::new();
    let mut current = unsafe { (*sentinel).next as *const ThoughtNode };
    while current != sentinel {
        result.push(unsafe { (*current).data });
        current = unsafe { (*current).next };
    }
    result
}

/// See [`read_raw_chain_from_sentinel`]. Convenience wrapper for reading `mgr`'s own persistent-list
/// chain directly (as opposed to a separate temporary output list's sentinel).
pub fn read_raw_chain(mgr: &ZTThoughtMgr) -> Vec<ZTThought> {
    read_raw_chain_from_sentinel(mgr.sentinel_ptr)
}

/// Frees every `Box`-owned node currently in `mgr`'s raw chain, without touching the sentinel. Safe
/// only when every node presently linked is genuinely `Box`-owned - i.e. `mgr` was seeded via
/// [`seed_raw_chain`], and any `.original()` call made against it since only *removed* nodes (which
/// vanilla frees via its own freelist push, already gone from the chain by the time this walks it) or
/// mutated fields in place, never *allocated* new ones. Never call this after a call that could have
/// allocated (`ADD_THOUGHT`/`LOAD`) - see [`destroy_standalone_mgr_leaking_nodes`] for that case.
fn free_raw_chain_nodes(mgr: &ZTThoughtMgr) {
    let sentinel = mgr.sentinel_ptr as *mut ThoughtNode;
    let mut current = unsafe { (*sentinel).next };
    while current != sentinel {
        let next = unsafe { (*current).next };
        drop(unsafe { Box::from_raw(current) });
        current = next;
    }
}

/// Tears down a standalone instance whose raw chain holds only `Box`-owned nodes (see
/// [`free_raw_chain_nodes`]) and which was never registered in [`THOUGHT_STORES`] - i.e. a "real"
/// comparison instance seeded via [`seed_raw_chain`] and driven only through `.original()` calls that
/// remove/mutate but never allocate.
pub fn free_raw_chain_mgr(ptr: *mut ZTThoughtMgr) {
    if ptr.is_null() {
        return;
    }
    let mgr = unsafe { &*ptr };
    free_raw_chain_nodes(mgr);
    drop(unsafe { Box::from_raw(mgr.sentinel_ptr as *mut ThoughtNode) });
    drop(unsafe { Box::from_raw(ptr) });
}

/// Tears down a standalone instance seeded on *both* sides at once (see `seed_thoughts_both` in the
/// live-comparison suite) - i.e. its raw chain holds `Box`-owned nodes (safe to free per
/// [`free_raw_chain_nodes`]) *and* it has a [`THOUGHT_STORES`] entry from also being driven through a
/// reimplemented-method call. Frees both representations, then the sentinel and the struct itself.
pub fn destroy_standalone_mgr_both(ptr: *mut ZTThoughtMgr) {
    if ptr.is_null() {
        return;
    }
    let mgr = unsafe { &*ptr };
    free_raw_chain_nodes(mgr);
    THOUGHT_STORES.lock().unwrap().remove(&mgr.store_key());
    drop(unsafe { Box::from_raw(mgr.sentinel_ptr as *mut ThoughtNode) });
    drop(unsafe { Box::from_raw(ptr) });
}

/// Tears down a standalone instance driven only through reimplemented methods (its data, if any,
/// lives entirely in [`THOUGHT_STORES`] - its raw chain was never linked into and stays a bare,
/// self-referencing sentinel). Removes the store entry, then frees the sentinel and the struct.
pub fn destroy_standalone_mgr(ptr: *mut ZTThoughtMgr) {
    if ptr.is_null() {
        return;
    }
    let mgr = unsafe { &*ptr };
    THOUGHT_STORES.lock().unwrap().remove(&mgr.store_key());
    drop(unsafe { Box::from_raw(mgr.sentinel_ptr as *mut ThoughtNode) });
    drop(unsafe { Box::from_raw(ptr) });
}

/// Frees only the sentinel node and the `ZTThoughtMgr` allocation itself, without walking/freeing
/// any raw-chain nodes - use this instead of `free_raw_chain_mgr` whenever real vanilla code (the
/// real, undetoured `ADD_THOUGHT`/`LOAD`) may have linked nodes it allocated through vanilla's own
/// small-object freelist into this manager's list: those nodes must never be freed through `Box` (a
/// cross-allocator free is undefined behavior / heap corruption), so this deliberately leaks them - a
/// one-time, per-proptest-case leak, reclaimed at process exit. The sentinel node itself is still
/// safe to free normally here: `ADD_THOUGHT`/`LOAD` only ever relink its `next`/`prev` fields to
/// point at newly inserted nodes, never reallocate or hand its own address to the freelist.
pub fn destroy_standalone_mgr_leaking_nodes(ptr: *mut ZTThoughtMgr) {
    if ptr.is_null() {
        return;
    }
    let mgr = unsafe { &*ptr };
    drop(unsafe { Box::from_raw(mgr.sentinel_ptr as *mut ThoughtNode) });
    drop(unsafe { Box::from_raw(ptr) });
}
