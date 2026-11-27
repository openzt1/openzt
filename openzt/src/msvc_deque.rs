//! Vanilla MSVC `std::deque<T>` layout: the two iterators it keeps over its block table.
//!
//! The deque object holds a `start` and a `finish` iterator, each `{cur, first, last, node}`: `cur` is the element
//! position inside the block `[first, last)`, and `node` points at that block's slot in the table of block pointers.
//! Every block is a fixed number of bytes from `operator_new`, so a block goes back through `operator_delete`.

use openzt_detour::generated::standalone::OPERATOR_DELETE;

use crate::util::{get_from_memory, save_to_memory};

/// Offsets inside one iterator.
mod iterator {
    pub const CUR: u32 = 0;
    pub const FIRST: u32 = 4;
    pub const LAST: u32 = 8;
    pub const NODE: u32 = 12;
    pub const SIZE: u32 = 16;
}

/// A deque in vanilla memory, addressed by its `start` iterator (the `finish` iterator follows it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MsvcDeque {
    start: u32,
    block_bytes: u32,
}

impl MsvcDeque {
    /// `start` is the address of the `start` iterator; blocks hold `block_bytes` bytes of elements.
    pub fn new(start: u32, block_bytes: u32) -> Self {
        Self { start, block_bytes }
    }

    fn finish(self) -> u32 {
        self.start + iterator::SIZE
    }

    fn start_word(self, offset: u32) -> u32 {
        get_from_memory(self.start + offset)
    }

    fn finish_word(self, offset: u32) -> u32 {
        get_from_memory(self.finish() + offset)
    }

    /// Empties the deque as `clear()` does: `destroy_range(begin, end)` runs over every live element range, the blocks
    /// past the first go back to `operator_delete`, and `finish` is set equal to `start`. The first block stays.
    pub fn clear(self, destroy_range: impl Fn(u32, u32)) {
        let mut slot = self.start_word(iterator::NODE).wrapping_add(4);
        let finish_node = self.finish_word(iterator::NODE);
        while slot < finish_node {
            let block: u32 = get_from_memory(slot);
            destroy_range(block, block.wrapping_add(self.block_bytes));
            if block != 0 {
                unsafe { OPERATOR_DELETE.original()(block) };
            }
            slot = slot.wrapping_add(4);
        }
        if self.start_word(iterator::NODE) != finish_node {
            destroy_range(self.start_word(iterator::CUR), self.start_word(iterator::LAST));
            destroy_range(self.finish_word(iterator::FIRST), self.finish_word(iterator::CUR));
            let block = self.finish_word(iterator::FIRST);
            if block != 0 {
                unsafe { OPERATOR_DELETE.original()(block) };
            }
        } else {
            destroy_range(self.start_word(iterator::CUR), self.finish_word(iterator::CUR));
        }
        for offset in [iterator::CUR, iterator::FIRST, iterator::LAST, iterator::NODE] {
            save_to_memory(self.finish() + offset, self.start_word(offset));
        }
    }
}
