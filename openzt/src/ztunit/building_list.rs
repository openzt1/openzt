//! The `list<ZTBuilding*>` that `ZTWorldMgr::getBuildingList` fills, owned for the length of a scope.
//!
//! Vanilla builds a fresh list on its stack with `list<uint>`'s constructor, lets `getBuildingList` append the
//! buildings of one name, walks it and destroys it, which hands every node and the sentinel back to the 12-byte
//! freelist. [`BuildingList`] does the same: the constructor runs on creation and the destructor's freelist pushes run
//! on drop, so an early return cannot leak the nodes.

use std::ffi::c_void;

use openzt_detour::generated::{msvc_std_basic_string::BASIC_STRING_RANGE, msvc_std_listuint::LIST_UINT_CTOR, ztworldmgr::GET_BUILDING_LIST};

use super::{base, free_string, WORLD_MGR_RVA};
use crate::{
    util::get_from_memory,
    vanilla_list::{self, NodeList},
};

/// A vanilla `list<ZTBuilding*>` filled by `getBuildingList`.
pub(super) struct BuildingList {
    /// The list object: a pointer to its sentinel node.
    sentinel: u32,
}

impl BuildingList {
    /// The buildings of the name held by the vanilla string at `name` (a `{begin, end, cap}` object).
    pub(super) fn named(name: *const i32) -> Self {
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let allocator_dummy: u8 = 0;
        let mut list: u32 = 0;
        unsafe {
            LIST_UINT_CTOR.hooked()(&mut list as *mut u32 as *const c_void, &allocator_dummy as *const u8 as u32);
            GET_BUILDING_LIST.hooked()(world as *const u32, name, &mut list as *mut u32 as *const i32);
        }
        Self { sentinel: list }
    }

    /// The buildings of `text`, spelled in a temporary vanilla string that is freed before returning.
    pub(super) fn named_text(text: &[u8]) -> Self {
        let mut name = [0u32; 3];
        unsafe { BASIC_STRING_RANGE.hooked()(name.as_mut_ptr() as *const c_void, text.as_ptr() as *const u32, text.as_ptr().add(text.len()) as i32) };
        let list = Self::named(name.as_ptr() as *const i32);
        free_string(&name);
        list
    }

    /// The building pointers in list order. The next link is read before a node is yielded.
    pub(super) fn buildings(&self) -> impl Iterator<Item = u32> {
        NodeList::words(self.sentinel).nodes().map(vanilla_list::value)
    }
}

impl Drop for BuildingList {
    fn drop(&mut self) {
        let nodes = NodeList::words(self.sentinel);
        nodes.clear();
        nodes.free_node(self.sentinel);
    }
}
