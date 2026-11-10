//! Vanilla MSVC `std::set` / `std::map` (Dinkumware red-black `_Tree`) layout compatibility.
//!
//! Reads and edits trees that live in vanilla memory, byte-compatible with what the game's own inlined
//! `_Tree::insert` / `erase` / `clear` produce, so vanilla code can keep walking a tree Rust has modified
//! (and Rust can read one vanilla built). The first users are `ZTHabitat`'s neighbour sets and portal map
//! and the `std::set<BFEntity*>` out-params `ZTTankExhibit` fills for vanilla callers.
//!
//! # Layout
//!
//! A tree container is `{head*, size}` (two words; the allocator and comparator are empty members laid out
//! before it, so a container sits at `member offset + 4` of the owning struct). The head is a node of the
//! same size as every other node whose `+0x4` is the root (0 when empty), `+0x8` the leftmost node and
//! `+0xc` the rightmost; an empty tree has both pointing back at the head, and the head is coloured red so
//! an iterator decrement can tell it from a real node. A node is `+0x0` colour (0 red, 1 black), `+0x4`
//! parent, `+0x8` left, `+0xc` right, `+0x10` key, and for a map `+0x14` the mapped value.
//!
//! # Allocation
//!
//! Nodes come from vanilla's `PoolAlloc` (size classes by `(size - 1) >> 3`), exactly like the game's own,
//! because vanilla may free them. Never free a vanilla-allocated node through Rust's allocator or the
//! reverse (see `AGENTS.md`, cross-allocator memory safety). `node_size` arguments name the size class the
//! nodes were allocated with: [`SET_NODE_SIZE`] for a set, [`MAP_NODE_SIZE`] for a map with a `u32` value.
//!
//! Keys are compared as unsigned 32-bit integers (`key < other`), which is vanilla's `std::less<T*>`.
//! Host unit tests allocate nodes from the Rust allocator instead of `PoolAlloc` (`cfg(test)`), which
//! needs a running game.

use crate::util::{get_from_memory, save_to_memory};
#[cfg(not(test))]
use openzt_detour::generated::poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE};

#[cfg(not(test))]
fn alloc_node(node_size: u32) -> u32 {
    unsafe { POOLALLOC_ALLOCATE.original()(node_size) as u32 }
}

#[cfg(not(test))]
fn free_node(node: u32, node_size: u32) {
    unsafe { POOLALLOC_DEALLOCATE.original()(node as *const u32, node_size) };
}

#[cfg(test)]
fn node_layout(node_size: u32) -> std::alloc::Layout {
    std::alloc::Layout::from_size_align(node_size as usize, 4).expect("valid node layout")
}

#[cfg(test)]
fn alloc_node(node_size: u32) -> u32 {
    unsafe { std::alloc::alloc_zeroed(node_layout(node_size)) as u32 }
}

#[cfg(test)]
fn free_node(node: u32, node_size: u32) {
    unsafe { std::alloc::dealloc(node as *mut u8, node_layout(node_size)) };
}

/// Walks a vanilla tree in sorted (in-order) order, yielding each node's own address - never the head
/// itself. `head_ptr` is the head node (`*container`). This is a direct, line-for-line translation of the
/// in-order-successor walk in `ZTHabitat_hiliteAmphibiousNeighbors.c`/`_hiliteShowNeighbors.c` (start at
/// the head's leftmost descendant; each step takes the right child's own leftmost descendant if a right
/// child exists, else climbs via `parent` while the current node is its parent's *right* child; terminate
/// when back at `head_ptr`), not a textbook re-derivation, to avoid an off-by-one divergence from the
/// shapes vanilla's own insert and erase produce.
///
/// Reads node memory lazily, one step per `next()`, so the tree must not be modified while iterating.
pub fn walk_nodes(head_ptr: u32) -> impl Iterator<Item = u32> {
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

/// Keys (node `+0x10`, read as a `u32`) of the `std::set` or `std::map` whose head node is `head_ptr`, in
/// [`walk_nodes`]' order. Reads node memory lazily, one step per `next()`.
pub fn set_members(head_ptr: u32) -> impl Iterator<Item = u32> {
    walk_nodes(head_ptr).map(|node| get_from_memory::<u32>(node + 0x10))
}

/// Size of a `std::set<ZTHabitat*>` node (`color`, `parent`, `left`, `right`, `key`), allocated from
/// `PoolAlloc`'s size-class-2 freelist.
pub const SET_NODE_SIZE: u32 = 0x14;
pub const MAP_NODE_SIZE: u32 = 0x18;
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
///
/// `node_size` is `SET_NODE_SIZE` for a set and `MAP_NODE_SIZE` for the show-portal map, whose
/// nodes carry a zero-initialised value word at `+0x14`. Returns the new node.
fn rb_set_insert_node(container: u32, parent: u32, key: u32, node_size: u32) -> u32 {
    let head: u32 = get_from_memory(container);
    let root_addr = head + 0x4;
    let node = alloc_node(node_size);
    if node != 0 {
        save_to_memory(node + 0x10, key);
        if node_size == MAP_NODE_SIZE {
            save_to_memory(node + 0x14, 0u32);
        }
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
    node
}

/// The node holding `key` in a vanilla-layout tree container at `container`, if any (`lower_bound`
/// followed by an equality check, as `get_show_portal`'s own inline descent does).
pub fn rb_find(container: u32, key: u32) -> Option<u32> {
    let head: u32 = get_from_memory(container);
    let mut candidate = head;
    let mut node = rb_parent(head);
    while node != 0 {
        if rb_key(node) < key {
            node = rb_right(node);
        } else {
            candidate = node;
            node = rb_left(node);
        }
    }
    (candidate != head && rb_key(candidate) <= key).then_some(candidate)
}

/// `std::map<ZTHabitat*, ZTFence*>::operator[]`'s insert half over the show-portal map at `container`
/// (`ZTHabitat+0x20`, `0x18`-byte nodes): returns the node for `key`, inserting a zero-valued one
/// when absent. The new node's position is the plain leaf insert of a unique key, so its shape matches
/// vanilla's `tree24::insert`.
pub fn rb_map_find_or_insert(container: u32, key: u32) -> u32 {
    if let Some(node) = rb_find(container, key) {
        return node;
    }
    let head: u32 = get_from_memory(container);
    let mut parent = head;
    let mut node = rb_parent(head);
    while node != 0 {
        parent = node;
        node = if key < rb_key(node) { rb_left(node) } else { rb_right(node) };
    }
    rb_set_insert_node(container, parent, key, MAP_NODE_SIZE)
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
            rb_set_insert_node(container, parent, key, SET_NODE_SIZE);
            return true;
        }
        predecessor = rb_decrement(parent);
    }
    if rb_key(predecessor) < key {
        rb_set_insert_node(container, parent, key, SET_NODE_SIZE);
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
        free_node(node, node_size);
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

/// Iterator increment (`msvc_std::tree::meth_0x427950`), including the climb off the rightmost node
/// onto the header.
fn rb_increment(node: u32) -> u32 {
    let right = rb_right(node);
    if right != 0 {
        let mut result = right;
        while rb_left(result) != 0 {
            result = rb_left(result);
        }
        return result;
    }
    let mut cursor = node;
    let mut parent = rb_parent(node);
    if cursor == rb_right(parent) {
        loop {
            cursor = parent;
            parent = rb_parent(cursor);
            if cursor != rb_right(parent) {
                break;
            }
        }
    }
    if rb_right(cursor) != parent {
        cursor = parent;
    }
    cursor
}

fn rb_set_color(node: u32, color: u8) {
    save_to_memory(node, color);
}

/// A null child counts as black, as in the vanilla rebalance's `x == 0 || *x == 1` tests.
fn rb_is_black(node: u32) -> bool {
    node == 0 || rb_color(node) != RB_RED
}

/// Unlinks `z` from the tree under `head` and rebalances - `FUN_0040a3da` (`_Rebalance_for_erase`),
/// with `head+4`/`+8`/`+0xc` as its root/leftmost/rightmost slots. Returns the node to free (always
/// `z`); the caller owns the free and the size decrement.
fn rb_rebalance_for_erase(z: u32, head: u32) -> u32 {
    let root_addr = head + 0x4;
    let min_addr = head + 0x8;
    let max_addr = head + 0xc;
    let root = || get_from_memory::<u32>(root_addr);

    let mut y = z;
    let mut x: u32;
    let mut x_parent: u32;
    let z_left = rb_left(z);

    if z_left == 0 {
        x = rb_right(z);
    } else {
        let z_right = rb_right(z);
        x = z_left;
        if z_right != 0 {
            y = z_right;
            while rb_left(y) != 0 {
                y = rb_left(y);
            }
            x = rb_right(y);
            if y != z {
                save_to_memory(z_left + 0x4, y);
                save_to_memory(y + 0x8, rb_left(z));
                x_parent = y;
                if y != z_right {
                    x_parent = rb_parent(y);
                    if x != 0 {
                        save_to_memory(x + 0x4, x_parent);
                    }
                    save_to_memory(rb_parent(y) + 0x8, x);
                    save_to_memory(y + 0xc, rb_right(z));
                    save_to_memory(rb_right(z) + 0x4, y);
                }
                if root() == z {
                    save_to_memory(root_addr, y);
                } else {
                    let parent = rb_parent(z);
                    if rb_left(parent) == z {
                        save_to_memory(parent + 0x8, y);
                    } else {
                        save_to_memory(parent + 0xc, y);
                    }
                }
                save_to_memory(y + 0x4, rb_parent(z));
                let (y_color, z_color) = (rb_color(y), rb_color(z));
                rb_set_color(y, z_color);
                rb_set_color(z, y_color);
                return rb_erase_fixup(z, x, x_parent, head);
            }
        }
    }

    x_parent = rb_parent(y);
    if x != 0 {
        save_to_memory(x + 0x4, x_parent);
    }
    if root() == z {
        save_to_memory(root_addr, x);
    } else {
        let parent = rb_parent(z);
        if rb_left(parent) == z {
            save_to_memory(parent + 0x8, x);
        } else {
            save_to_memory(parent + 0xc, x);
        }
    }
    if get_from_memory::<u32>(min_addr) == z {
        if rb_right(z) == 0 {
            save_to_memory(min_addr, rb_parent(z));
        } else {
            let mut leftmost = x;
            while rb_left(leftmost) != 0 {
                leftmost = rb_left(leftmost);
            }
            save_to_memory(min_addr, leftmost);
        }
    }
    if get_from_memory::<u32>(max_addr) == z {
        if rb_left(z) == 0 {
            save_to_memory(max_addr, rb_parent(z));
        } else {
            let mut rightmost = x;
            while rb_right(rightmost) != 0 {
                rightmost = rb_right(rightmost);
            }
            save_to_memory(max_addr, rightmost);
        }
    }
    rb_erase_fixup(y, x, x_parent, head)
}

/// Tail of [`rb_rebalance_for_erase`] (`LAB_0040a428`): restores the black-height invariant when the
/// physically removed node `y` was black. `x` is the node that took its place (possibly null) under
/// `x_parent`.
fn rb_erase_fixup(y: u32, mut x: u32, mut x_parent: u32, head: u32) -> u32 {
    let root_addr = head + 0x4;
    let root = || get_from_memory::<u32>(root_addr);

    if rb_color(y) != RB_RED {
        while x != root() {
            if x != 0 && rb_color(x) != RB_BLACK {
                break;
            }
            let parent = x_parent;
            if x == rb_left(parent) {
                let mut sibling = rb_right(parent);
                if rb_color(sibling) == RB_RED {
                    rb_set_color(sibling, RB_BLACK);
                    rb_set_color(parent, RB_RED);
                    rb_rotate_left(parent, root_addr);
                    sibling = rb_right(parent);
                }
                if !rb_is_black(rb_left(sibling)) || !rb_is_black(rb_right(sibling)) {
                    if rb_is_black(rb_right(sibling)) {
                        if rb_left(sibling) != 0 {
                            rb_set_color(rb_left(sibling), RB_BLACK);
                        }
                        rb_set_color(sibling, RB_RED);
                        rb_rotate_right(sibling, root_addr);
                        sibling = rb_right(parent);
                    }
                    rb_set_color(sibling, rb_color(parent));
                    rb_set_color(parent, RB_BLACK);
                    if rb_right(sibling) != 0 {
                        rb_set_color(rb_right(sibling), RB_BLACK);
                    }
                    rb_rotate_left(parent, root_addr);
                    break;
                }
                rb_set_color(sibling, RB_RED);
            } else {
                let mut sibling = rb_left(parent);
                if rb_color(sibling) == RB_RED {
                    rb_set_color(sibling, RB_BLACK);
                    rb_set_color(parent, RB_RED);
                    rb_rotate_right(parent, root_addr);
                    sibling = rb_left(parent);
                }
                if !rb_is_black(rb_right(sibling)) || !rb_is_black(rb_left(sibling)) {
                    if rb_is_black(rb_left(sibling)) {
                        if rb_right(sibling) != 0 {
                            rb_set_color(rb_right(sibling), RB_BLACK);
                        }
                        rb_set_color(sibling, RB_RED);
                        rb_rotate_left(sibling, root_addr);
                        sibling = rb_left(parent);
                    }
                    rb_set_color(sibling, rb_color(parent));
                    rb_set_color(parent, RB_BLACK);
                    if rb_left(sibling) != 0 {
                        rb_set_color(rb_left(sibling), RB_BLACK);
                    }
                    rb_rotate_right(parent, root_addr);
                    break;
                }
                rb_set_color(sibling, RB_RED);
            }
            x = parent;
            x_parent = rb_parent(parent);
            if parent == root() {
                break;
            }
        }
        if x != 0 {
            rb_set_color(x, RB_BLACK);
        }
    }
    y
}

/// `std::set<ptr>::erase(key)` over a vanilla-layout set whose `{head*, size}` pair lives at
/// `container`: `removeAmphibiousNeighbor`'s `equal_range` + `erase(first, last)` (`0x59f5d5`). Erasing
/// the whole tree takes vanilla's clear path ([`rb_tree_clear`]); otherwise each node in the range is
/// unlinked with [`rb_rebalance_for_erase`], freed to `PoolAlloc`, and the size decremented. An absent
/// key is a no-op.
pub fn rb_set_erase(container: u32, key: u32) {
    rb_erase_key(container, key, SET_NODE_SIZE);
}

/// `std::map<ZTHabitat*, ZTFence*>::erase(key)` over the show-portal map at `container` - the same
/// erase as [`rb_set_erase`] over `0x18`-byte nodes.
pub fn rb_map_erase(container: u32, key: u32) {
    rb_erase_key(container, key, MAP_NODE_SIZE);
}

fn rb_erase_key(container: u32, key: u32, node_size: u32) {
    let head: u32 = get_from_memory(container);

    let mut upper = head;
    let mut node = rb_parent(head);
    while node != 0 {
        if key < rb_key(node) {
            upper = node;
            node = rb_left(node);
        } else {
            node = rb_right(node);
        }
    }
    let mut lower = head;
    node = rb_parent(head);
    while node != 0 {
        if rb_key(node) >= key {
            lower = node;
            node = rb_left(node);
        } else {
            node = rb_right(node);
        }
    }

    if lower == rb_left(head) && upper == head {
        rb_tree_clear(container, node_size);
        return;
    }
    while lower != upper {
        let erased = lower;
        lower = rb_increment(lower);
        let freed = rb_rebalance_for_erase(erased, head);
        free_node(freed, node_size);
        save_to_memory(container + 0x4, get_from_memory::<u32>(container + 0x4) - 1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    /// Allocates a zeroed `{head*, size}` container plus an empty head node, the shape vanilla's
    /// constructor leaves: red head, no root, leftmost and rightmost pointing back at the head.
    fn new_tree() -> u32 {
        let container = alloc_node(8);
        let head = alloc_node(MAP_NODE_SIZE);
        save_to_memory(container, head);
        save_to_memory(container + 4, 0u32);
        save_to_memory(head, RB_RED);
        save_to_memory(head + 0x4, 0u32);
        save_to_memory(head + 0x8, head);
        save_to_memory(head + 0xc, head);
        container
    }

    fn head_of(container: u32) -> u32 {
        get_from_memory(container)
    }

    /// Black node count down every path from `node` to a leaf, panicking on any red-black violation:
    /// wrong parent pointer, red node with a red child, or unequal black heights.
    fn black_height(node: u32, parent: u32) -> u32 {
        if node == 0 {
            return 1;
        }
        assert_eq!(rb_parent(node), parent, "parent pointer of node {node:#x}");
        if rb_color(node) == RB_RED {
            for child in [rb_left(node), rb_right(node)] {
                assert!(child == 0 || rb_color(child) != RB_RED, "red node {node:#x} has a red child");
            }
        }
        let left = black_height(rb_left(node), node);
        let right = black_height(rb_right(node), node);
        assert_eq!(left, right, "unequal black height under {node:#x}");
        left + u32::from(rb_color(node) != RB_RED)
    }

    /// Asserts the tree at `container` holds exactly `expected` (sorted keys), with a valid red-black shape,
    /// head bookkeeping and size word.
    fn check_tree(container: u32, expected: &[u32]) {
        let head = head_of(container);
        assert_eq!(get_from_memory::<u32>(container + 4) as usize, expected.len(), "size word");
        assert_eq!(set_members(head).collect::<Vec<_>>(), expected, "in-order keys");
        let root = rb_parent(head);
        if expected.is_empty() {
            assert_eq!(root, 0, "empty tree has a root");
            assert_eq!(rb_left(head), head, "empty tree leftmost");
            assert_eq!(rb_right(head), head, "empty tree rightmost");
            return;
        }
        assert_eq!(rb_parent(root), head, "root's parent is the head");
        assert_eq!(rb_color(root), RB_BLACK, "root is black");
        let nodes: Vec<u32> = walk_nodes(head).collect();
        assert_eq!(rb_left(head), nodes[0], "leftmost pointer");
        assert_eq!(rb_right(head), *nodes.last().unwrap(), "rightmost pointer");
        black_height(root, head);
    }

    struct Rng(u32);
    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            self.0
        }
    }

    fn key_for(raw: u32, spread: u32) -> u32 {
        0x1000 + (raw % spread) * 4
    }

    #[test]
    fn empty_tree_walks_nothing_and_finds_nothing() {
        let container = new_tree();
        check_tree(container, &[]);
        assert_eq!(rb_find(container, 0x1000), None);
        rb_tree_clear(container, SET_NODE_SIZE);
        rb_set_erase(container, 0x1000);
        check_tree(container, &[]);
    }

    #[test]
    fn set_insert_reports_new_vs_duplicate_and_keeps_order() {
        let container = new_tree();
        for key in [0x30, 0x10, 0x20, 0x10, 0x30, 0x40] {
            let expected_new = !set_members(head_of(container)).any(|k| k == key);
            assert_eq!(rb_set_insert(container, key), expected_new, "insert {key:#x}");
        }
        check_tree(container, &[0x10, 0x20, 0x30, 0x40]);
        rb_tree_clear(container, SET_NODE_SIZE);
    }

    #[test]
    fn ascending_and_descending_inserts_stay_balanced() {
        for descending in [false, true] {
            let container = new_tree();
            let keys: Vec<u32> = (0..200u32).map(|i| 0x1000 + i * 4).collect();
            let order: Vec<u32> = if descending { keys.iter().rev().copied().collect() } else { keys.clone() };
            for (i, &key) in order.iter().enumerate() {
                assert!(rb_set_insert(container, key));
                if i % 17 == 0 {
                    let mut so_far: Vec<u32> = order[..=i].to_vec();
                    so_far.sort_unstable();
                    check_tree(container, &so_far);
                }
            }
            check_tree(container, &keys);
            for (i, &key) in keys.iter().enumerate() {
                rb_set_erase(container, key);
                check_tree(container, &keys[i + 1..]);
            }
        }
    }

    #[test]
    fn set_random_insert_erase_matches_btreeset() {
        for seed in [1u32, 0xdead_beef, 0x1234_5678, 0x0bad_f00d] {
            let mut rng = Rng(seed);
            let container = new_tree();
            let mut reference: BTreeSet<u32> = BTreeSet::new();
            for step in 0..3000 {
                let key = key_for(rng.next(), 97);
                match rng.next() % 8 {
                    0..=3 => {
                        assert_eq!(rb_set_insert(container, key), reference.insert(key), "insert {key:#x} step {step}");
                    }
                    4..=6 => {
                        rb_set_erase(container, key);
                        reference.remove(&key);
                    }
                    _ => {
                        if rng.next() % 16 == 0 {
                            rb_tree_clear(container, SET_NODE_SIZE);
                            reference.clear();
                        }
                    }
                }
                if step % 7 == 0 || reference.len() < 3 {
                    check_tree(container, &reference.iter().copied().collect::<Vec<_>>());
                }
            }
            check_tree(container, &reference.iter().copied().collect::<Vec<_>>());
            rb_tree_clear(container, SET_NODE_SIZE);
            check_tree(container, &[]);
        }
    }

    #[test]
    fn erase_absent_key_changes_nothing() {
        let container = new_tree();
        for key in [0x20, 0x10, 0x30] {
            rb_set_insert(container, key);
        }
        rb_set_erase(container, 0x18);
        rb_set_erase(container, 0x99);
        check_tree(container, &[0x10, 0x20, 0x30]);
        rb_tree_clear(container, SET_NODE_SIZE);
    }

    #[test]
    fn erasing_the_only_key_empties_the_tree() {
        let container = new_tree();
        rb_set_insert(container, 0x1000);
        rb_set_erase(container, 0x1000);
        check_tree(container, &[]);
        assert!(rb_set_insert(container, 0x1000));
        check_tree(container, &[0x1000]);
        rb_tree_clear(container, SET_NODE_SIZE);
    }

    #[test]
    fn rb_find_is_exact_match_only() {
        let container = new_tree();
        for key in [0x10, 0x30, 0x50] {
            rb_set_insert(container, key);
        }
        for (key, present) in [(0x10, true), (0x30, true), (0x50, true), (0x0, false), (0x20, false), (0x40, false), (0x60, false)] {
            let found = rb_find(container, key);
            assert_eq!(found.is_some(), present, "find {key:#x}");
            if let Some(node) = found {
                assert_eq!(rb_key(node), key);
            }
        }
        rb_tree_clear(container, SET_NODE_SIZE);
    }

    #[test]
    fn map_find_or_insert_reuses_nodes_and_keeps_values() {
        let container = new_tree();
        let node = rb_map_find_or_insert(container, 0x2000);
        assert_eq!(get_from_memory::<u32>(node + 0x14), 0, "new value is zero");
        save_to_memory(node + 0x14, 0xabcd_u32);
        assert_eq!(rb_map_find_or_insert(container, 0x2000), node, "same node for the same key");
        let other = rb_map_find_or_insert(container, 0x1000);
        save_to_memory(other + 0x14, 0x1111_u32);
        assert_eq!(get_from_memory::<u32>(rb_find(container, 0x2000).unwrap() + 0x14), 0xabcd);
        assert_eq!(get_from_memory::<u32>(rb_find(container, 0x1000).unwrap() + 0x14), 0x1111);
        check_tree(container, &[0x1000, 0x2000]);
        rb_map_erase(container, 0x2000);
        check_tree(container, &[0x1000]);
        assert_eq!(rb_find(container, 0x2000), None);
        rb_tree_clear(container, MAP_NODE_SIZE);
    }

    #[test]
    fn map_random_operations_match_btreemap() {
        for seed in [7u32, 0xcafe_f00d] {
            let mut rng = Rng(seed);
            let container = new_tree();
            let mut reference: BTreeMap<u32, u32> = BTreeMap::new();
            for step in 0..2000 {
                let key = key_for(rng.next(), 61);
                if rng.next() % 3 == 0 {
                    rb_map_erase(container, key);
                    reference.remove(&key);
                } else {
                    let value = rng.next();
                    let node = rb_map_find_or_insert(container, key);
                    assert_eq!(get_from_memory::<u32>(node + 0x14), reference.get(&key).copied().unwrap_or(0), "step {step}");
                    save_to_memory(node + 0x14, value);
                    reference.insert(key, value);
                }
                if step % 9 == 0 {
                    check_tree(container, &reference.keys().copied().collect::<Vec<_>>());
                }
            }
            check_tree(container, &reference.keys().copied().collect::<Vec<_>>());
            for (&key, &value) in &reference {
                assert_eq!(get_from_memory::<u32>(rb_find(container, key).unwrap() + 0x14), value);
            }
            rb_tree_clear(container, MAP_NODE_SIZE);
            check_tree(container, &[]);
        }
    }

    #[test]
    fn clear_frees_every_node_and_tree_is_reusable() {
        let container = new_tree();
        for i in 0..50 {
            rb_set_insert(container, 0x1000 + i * 4);
        }
        rb_tree_clear(container, SET_NODE_SIZE);
        check_tree(container, &[]);
        for key in [0x5, 0x3, 0x9] {
            rb_set_insert(container, key);
        }
        check_tree(container, &[0x3, 0x5, 0x9]);
        rb_tree_clear(container, SET_NODE_SIZE);
    }
}
