//! The unit's target list (`unit+0x154`): a circular doubly linked list of `{next, prev, position}` nodes around a
//! sentinel, with an empty list being a sentinel that points at itself. Nodes are 20-byte `PoolAlloc` blocks (the 17-to-24-byte size class), so
//! the list shares vanilla's size-class freelist (`DAT_00638008`) with everything else of that size; vanilla
//! (`BFUnit::stop`, `~BFUnit`, the path searches, `BFPathMove::doProgress`) fills and empties it with inlined
//! freelist pushes and pops. This type reads and edits the list in place and takes nodes from, and returns them
//! to, the same pool, so a list edited here stays valid for vanilla and the other way round.

#![allow(dead_code)] // `clear`, `push_back` and `positions` are for the stop/setMoving ports (E2); the live test exercises them now.

use openzt_detour::generated::poolalloc::{ALLOCATE as POOL_ALLOCATE, DEALLOCATE as POOL_DEALLOCATE};

use crate::util::{get_from_memory, save_to_memory};

/// Entity field holding the pointer to the list's sentinel node.
pub(crate) const UNIT_TARGET_LIST: u32 = 0x154;
/// Bytes in a node: `next`, `prev` and a tile position (three `i32`s).
pub(crate) const NODE_SIZE: u32 = 20;
const NODE_NEXT: u32 = 0;
const NODE_PREV: u32 = 4;
const NODE_POSITION: u32 = 8;

/// Where list nodes come from and go back to.
pub(crate) trait NodePool {
    /// A `NODE_SIZE`-byte block.
    unsafe fn alloc(&mut self) -> u32;
    unsafe fn free(&mut self, node: u32);
}

/// Vanilla's pool: `PoolAlloc::allocate`/`deallocate` for `NODE_SIZE`-byte blocks.
pub(crate) struct VanillaPool;

impl NodePool for VanillaPool {
    unsafe fn alloc(&mut self) -> u32 {
        unsafe { POOL_ALLOCATE.original()(NODE_SIZE) as u32 }
    }

    unsafe fn free(&mut self, node: u32) {
        unsafe { POOL_DEALLOCATE.original()(node as *const u32, NODE_SIZE) };
    }
}

/// A view of one unit's list.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TargetList {
    sentinel: u32,
}

impl TargetList {
    pub(crate) fn of_unit(unit: u32) -> Self {
        Self { sentinel: get_from_memory::<u32>(unit + UNIT_TARGET_LIST) }
    }

    #[cfg(any(test, feature = "reimplementation-tests"))]
    pub(crate) fn from_sentinel(sentinel: u32) -> Self {
        Self { sentinel }
    }

    fn next(node: u32) -> u32 {
        get_from_memory::<u32>(node + NODE_NEXT)
    }

    fn position(node: u32) -> [i32; 3] {
        [get_from_memory::<i32>(node + NODE_POSITION), get_from_memory::<i32>(node + NODE_POSITION + 4), get_from_memory::<i32>(node + NODE_POSITION + 8)]
    }

    pub(crate) fn is_empty(&self) -> bool {
        Self::next(self.sentinel) == self.sentinel
    }

    /// Number of nodes between the sentinel's `next` and the sentinel.
    pub(crate) fn len(&self) -> u32 {
        let mut count = 0;
        let mut node = Self::next(self.sentinel);
        while node != self.sentinel {
            count += 1;
            node = Self::next(node);
        }
        count
    }

    /// The position of every node, front to back.
    pub(crate) fn positions(&self) -> Vec<[i32; 3]> {
        let mut values = Vec::new();
        let mut node = Self::next(self.sentinel);
        while node != self.sentinel {
            values.push(Self::position(node));
            node = Self::next(node);
        }
        values
    }

    /// Appends a node holding `position`.
    pub(crate) unsafe fn push_back(&self, pool: &mut impl NodePool, position: [i32; 3]) {
        let node = unsafe { pool.alloc() };
        let last = get_from_memory::<u32>(self.sentinel + NODE_PREV);
        save_to_memory(node + NODE_NEXT, self.sentinel);
        save_to_memory(node + NODE_PREV, last);
        for (index, word) in position.iter().enumerate() {
            save_to_memory(node + NODE_POSITION + 4 * index as u32, *word);
        }
        save_to_memory(last + NODE_NEXT, node);
        save_to_memory(self.sentinel + NODE_PREV, node);
    }

    /// Frees every node front to back (so the last node freed is the last one in the list, as `BFUnit::stop`
    /// does) and leaves the sentinel pointing at itself.
    pub(crate) unsafe fn clear(&self, pool: &mut impl NodePool) {
        let mut node = Self::next(self.sentinel);
        while node != self.sentinel {
            let next = Self::next(node);
            if node != 0 {
                unsafe { pool.free(node) };
            }
            node = next;
        }
        save_to_memory(self.sentinel + NODE_NEXT, self.sentinel);
        save_to_memory(self.sentinel + NODE_PREV, self.sentinel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pool of leaked 20-byte blocks with its own LIFO freelist, like vanilla's size class.
    #[derive(Default)]
    struct FakePool {
        freelist: Vec<u32>,
        allocated: Vec<u32>,
        freed_in_order: Vec<u32>,
    }

    impl NodePool for FakePool {
        unsafe fn alloc(&mut self) -> u32 {
            let node = self.freelist.pop().unwrap_or_else(|| Box::leak(Box::new([0u32; 5])).as_ptr() as u32);
            self.allocated.push(node);
            node
        }

        unsafe fn free(&mut self, node: u32) {
            assert!(self.allocated.contains(&node), "freed a block the pool did not hand out");
            assert!(!self.freelist.contains(&node), "double free");
            self.freed_in_order.push(node);
            self.freelist.push(node);
        }
    }

    fn new_list() -> (TargetList, Box<[u32; 3]>) {
        let sentinel = Box::new([0u32; 3]);
        let address = sentinel.as_ptr() as u32;
        let list = TargetList::from_sentinel(address);
        save_to_memory(address, address);
        save_to_memory(address + 4, address);
        (list, sentinel)
    }

    #[test]
    fn empty_list_has_no_nodes() {
        let (list, _keep) = new_list();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(list.positions().is_empty());
    }

    #[test]
    fn push_back_appends_in_order() {
        let (list, _keep) = new_list();
        let mut pool = FakePool::default();
        for value in [7, 8, 9] {
            unsafe { list.push_back(&mut pool, [value, -value, value * 10]) };
        }
        assert!(!list.is_empty());
        assert_eq!(list.len(), 3);
        assert_eq!(list.positions(), [[7, -7, 70], [8, -8, 80], [9, -9, 90]]);
        // The back links are consistent: walking prev from the sentinel visits the nodes in reverse.
        let mut node = get_from_memory::<u32>(list.sentinel + NODE_PREV);
        let mut reversed = Vec::new();
        while node != list.sentinel {
            reversed.push(get_from_memory::<i32>(node + NODE_POSITION));
            node = get_from_memory::<u32>(node + NODE_PREV);
        }
        assert_eq!(reversed, [9, 8, 7]);
    }

    #[test]
    fn clear_returns_every_node_front_to_back_and_relinks_the_sentinel() {
        let (list, _keep) = new_list();
        let mut pool = FakePool::default();
        for value in [1, 2, 3] {
            unsafe { list.push_back(&mut pool, [value, 0, 0]) };
        }
        let nodes = pool.allocated.clone();
        unsafe { list.clear(&mut pool) };
        assert!(list.is_empty());
        assert_eq!(get_from_memory::<u32>(list.sentinel + NODE_PREV), list.sentinel);
        assert_eq!(pool.freed_in_order, nodes);
        // The pool is LIFO, so the next push reuses the last node freed.
        unsafe { list.push_back(&mut pool, [4, 0, 0]) };
        assert_eq!(*pool.allocated.last().unwrap(), nodes[2]);
        assert_eq!(list.positions(), [[4, 0, 0]]);
    }

    #[test]
    fn clearing_an_empty_list_frees_nothing() {
        let (list, _keep) = new_list();
        let mut pool = FakePool::default();
        unsafe { list.clear(&mut pool) };
        assert!(pool.freed_in_order.is_empty());
        assert!(list.is_empty());
    }
}
