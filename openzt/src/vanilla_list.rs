//! Vanilla MSVC `std::list<T>` node handling over vanilla's small-block freelists: 12-byte nodes (`list<uint>`,
//! `list<T*>`) and 20-byte nodes (a unit's path steps, `{next, previous, x, y, z}`).
//!
//! A list object is a pointer to its sentinel node. Every node, the sentinel included, is `{next +0x0, previous +0x4,
//! value +0x8}`, and the list is circular: an empty list's sentinel points at itself both ways. Nodes come from
//! vanilla's 12-byte small-block freelist (`DAT_00638004`) and go back onto it when erased, so nothing here touches
//! Rust's allocator. Insertion stays with vanilla's `list<uint>::insert` (`msvc_std_listuint::INSERT`), which pops
//! that same freelist.

use openzt_detour::generated::msvc_std_listuint::INSERT as LIST_UINT_INSERT;

use crate::{
    globals::get_module_base,
    util::{get_from_memory, save_to_memory},
};

/// Head of vanilla's 12-byte small-block freelist (`DAT_00638004`) as an RVA from the `zoo.exe` base.
pub const FREELIST_12_RVA: u32 = 0x0023_8004;

/// Head of vanilla's 20-byte small-block freelist (`DAT_00638008`) as an RVA from the `zoo.exe` base.
pub const FREELIST_20_RVA: u32 = 0x0023_8008;

const NODE_VALUE_OFFSET: u32 = 8;

/// Pushes `node` on vanilla's 12-byte freelist, as every vanilla list erase does. A null node is ignored.
pub fn free_node(node: u32) {
    NodeList::words(0).free_node(node);
}

/// The nodes of the list whose sentinel is `sentinel`, in order, as raw addresses. The next link of each node is read
/// before the node is yielded, so a caller may unlink and free the node it was just given.
pub fn nodes(sentinel: u32) -> impl Iterator<Item = u32> {
    let mut node: u32 = get_from_memory(sentinel);
    std::iter::from_fn(move || {
        if node == sentinel {
            return None;
        }
        let current = node;
        node = get_from_memory(current);
        Some(current)
    })
}

/// The value word of `node`.
pub fn value(node: u32) -> u32 {
    get_from_memory(node + NODE_VALUE_OFFSET)
}

/// Unlinks `node` from its neighbours and pushes it on the freelist.
pub fn unlink_and_free(node: u32) {
    let next: u32 = get_from_memory(node);
    let previous: u32 = get_from_memory(node + 4);
    save_to_memory(previous, next);
    save_to_memory(next + 4, previous);
    free_node(node);
}

/// Unlinks and frees every node whose value is `target`, front to back.
pub fn remove_matching(sentinel: u32, target: u32) {
    for node in nodes(sentinel) {
        if value(node) == target {
            unlink_and_free(node);
        }
    }
}

/// Pushes every node onto the freelist front to back (so the last node ends up at the head) and leaves the list
/// empty and self-linked.
pub fn clear(sentinel: u32) {
    NodeList::words(sentinel).clear();
}

/// Inserts `value` at the front through vanilla's `list<uint>::insert`, before the first node.
pub fn push_front(sentinel: u32, value: u32) {
    let first: u32 = get_from_memory(sentinel);
    let mut result: u32 = 0;
    unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, first as i32, &value as *const u32 as *const i32) };
}

/// Appends `value` through vanilla's `list<uint>::insert` at the end position.
pub fn push_back(sentinel: u32, value: u32) {
    let mut result: u32 = 0;
    unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, &value as *const u32 as *const i32) };
}

/// A vanilla list over any node size, identified by its sentinel and the freelist its nodes come from. Nothing here
/// allocates: erased nodes go back onto `freelist_rva`'s head exactly as vanilla's erase leaves them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeList {
    sentinel: u32,
    freelist_rva: u32,
}

impl NodeList {
    /// A list of 12-byte nodes.
    pub fn words(sentinel: u32) -> Self {
        Self { sentinel, freelist_rva: FREELIST_12_RVA }
    }

    /// A list of 20-byte nodes (`{next, previous, x, y, z}` path steps).
    pub fn path_steps(sentinel: u32) -> Self {
        Self { sentinel, freelist_rva: FREELIST_20_RVA }
    }

    pub fn sentinel(self) -> u32 {
        self.sentinel
    }

    /// The nodes in order; the next link is read before a node is yielded, so the node may be unlinked and freed.
    pub fn nodes(self) -> impl Iterator<Item = u32> {
        nodes(self.sentinel)
    }

    /// How many nodes the list holds.
    pub fn len(self) -> usize {
        self.nodes().count()
    }

    pub fn is_empty(self) -> bool {
        get_from_memory::<u32>(self.sentinel) == self.sentinel
    }

    /// The node reached by following `n` next links from the first node (`0` is the first). Like vanilla's unchecked
    /// walk it does not stop at the end: an `n` past the last node lands on the sentinel.
    pub fn nth(self, n: usize) -> u32 {
        let mut node: u32 = get_from_memory(self.sentinel);
        for _ in 0..n {
            node = get_from_memory(node);
        }
        node
    }

    /// The last node, or the sentinel when the list is empty.
    pub fn last(self) -> u32 {
        get_from_memory(self.sentinel + 4)
    }

    /// Pushes `node` on this list's freelist.
    pub fn free_node(self, node: u32) {
        if node != 0 {
            let head = get_module_base("zoo.exe") as u32 + self.freelist_rva;
            save_to_memory(node, get_from_memory::<u32>(head));
            save_to_memory(head, node);
        }
    }

    /// Unlinks the last node and frees it. Does nothing on an empty list.
    pub fn pop_back(self) {
        if self.is_empty() {
            return;
        }
        let last = self.last();
        let (previous, next): (u32, u32) = (get_from_memory(last + 4), get_from_memory(last));
        save_to_memory(previous, next);
        save_to_memory(next + 4, previous);
        self.free_node(last);
    }

    /// Frees every node front to back and leaves the list empty and self-linked.
    pub fn clear(self) {
        for node in self.nodes() {
            self.free_node(node);
        }
        save_to_memory(self.sentinel, self.sentinel);
        save_to_memory(self.sentinel + 4, self.sentinel);
    }
}
