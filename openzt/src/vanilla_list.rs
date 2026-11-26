//! Vanilla MSVC `std::list<T>` node handling for 12-byte nodes (`list<uint>`, `list<T*>`).
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

const NODE_VALUE_OFFSET: u32 = 8;

fn freelist_head() -> u32 {
    get_module_base("zoo.exe") as u32 + FREELIST_12_RVA
}

/// Pushes `node` on vanilla's 12-byte freelist, as every vanilla list erase does. A null node is ignored.
pub fn free_node(node: u32) {
    if node != 0 {
        let head = freelist_head();
        save_to_memory(node, get_from_memory::<u32>(head));
        save_to_memory(head, node);
    }
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
    for node in nodes(sentinel) {
        free_node(node);
    }
    save_to_memory(sentinel, sentinel);
    save_to_memory(sentinel + 4, sentinel);
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
