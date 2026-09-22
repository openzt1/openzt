use openzt_detour::generated::{
    standalone::OPERATOR_NEW,
    ztgamemgr::GET_DATE,
    ztshowinfo::{ADD_SHOW, IS_STARTED, REMOVE_SHOW},
};
use windows::Win32::Foundation::FILETIME;

use crate::{
    globals::globals,
    util::{get_from_memory, save_to_memory},
};

use super::units::free_unit_array_buffer;

pub const PENDING_SCRIPT_NODE_SIZE: u32 = 0x48;

pub fn check_unit_type(this: u32, unit_type: u32) -> u32 {
    let script_id = get_from_memory::<u16>(this + 0x8);
    match super::super::script::script_type_by_id(script_id) {
        Some(script_type) if script_type != unit_type => unit_type & 0xffff_ff00,
        Some(_) => (unit_type & 0xffff_ff00) | 1,
        None => 1,
    }
}

pub(crate) fn collect_pending_script_nodes(node: u32, out: &mut Vec<u32>) {
    if node == 0 {
        return;
    }
    collect_pending_script_nodes(get_from_memory::<u32>(node + 0x8), out);
    out.push(node);
    collect_pending_script_nodes(get_from_memory::<u32>(node + 0xc), out);
}

pub(crate) fn pending_script_node_count(show_info: u32) -> usize {
    let header = get_from_memory::<u32>(show_info + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);
    nodes.len()
}

pub fn check_pending_scripts(show_info: u32) {
    let header = get_from_memory::<u32>(show_info + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);

    for node in nodes {
        let pending_id = get_from_memory::<u16>(node + 0x1e);
        if pending_id == 0xffff {
            continue;
        }
        let unit_type_id = get_from_memory::<u32>(node + 0x10);
        let current_id = get_from_memory::<u16>(node + 0x1c);
        if current_id != pending_id {
            save_to_memory(node + 0x1c, pending_id);
            if super::super::script::script_exists_by_id(current_id) {
                super::super::script::unregister_script_by_id(current_id);
            }
            let has_items = super::super::script::script_exists_by_id(pending_id)
                && super::super::script::script_item_count_by_id(pending_id) > 0;
            unsafe {
                if has_items {
                    ADD_SHOW.hooked()(show_info as *const u32, unit_type_id);
                } else {
                    REMOVE_SHOW.hooked()(show_info as *const u32, unit_type_id);
                }
            }
        }
        save_to_memory(node + 0x1e, 0xffffu16);
    }
}

fn allocate_pending_script_node(unit_type_id: u32) -> u32 {
    let node = unsafe { OPERATOR_NEW.original()(PENDING_SCRIPT_NODE_SIZE) } as u32;
    unsafe { std::ptr::write_bytes(node as *mut u8, 0, PENDING_SCRIPT_NODE_SIZE as usize) };
    save_to_memory(node + 0x10, unit_type_id);

    let sentinel = unsafe { OPERATOR_NEW.original()(0xc) } as u32;
    save_to_memory(sentinel, sentinel);
    save_to_memory(sentinel + 4, sentinel);
    save_to_memory(sentinel + 8, 0u32);
    save_to_memory(node + 0x18, sentinel);

    node
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PendingNodeInsertPlan {
    Found(u32),
    NewRoot,
    InsertLeft { parent: u32, parent_is_leftmost: bool },
    InsertRight { parent: u32 },
}

pub(crate) fn plan_pending_node_insert(header: u32, unit_type_id: u32) -> PendingNodeInsertPlan {
    let root = get_from_memory::<u32>(header + 4);
    if root == 0 {
        return PendingNodeInsertPlan::NewRoot;
    }

    let mut node = root;
    let mut candidate = header;
    let (parent, went_left) = loop {
        let parent = node;
        if get_from_memory::<u32>(node + 0x10) < unit_type_id {
            let right = get_from_memory::<u32>(node + 0xc);
            if right == 0 {
                break (parent, false);
            }
            node = right;
        } else {
            candidate = node;
            let left = get_from_memory::<u32>(node + 0x8);
            if left == 0 {
                break (parent, true);
            }
            node = left;
        }
    };

    if candidate != header && get_from_memory::<u32>(candidate + 0x10) <= unit_type_id {
        return PendingNodeInsertPlan::Found(candidate);
    }

    if went_left {
        let leftmost = get_from_memory::<u32>(header + 8);
        PendingNodeInsertPlan::InsertLeft { parent, parent_is_leftmost: parent == leftmost }
    } else {
        PendingNodeInsertPlan::InsertRight { parent }
    }
}

fn increment_pending_script_node_count(show_info: u32) {
    let count = get_from_memory::<u32>(show_info + 0x48);
    save_to_memory(show_info + 0x48, count + 1);
}

pub fn find_or_insert_pending_script_node(show_info: u32, unit_type_id: u32) -> (u32, bool) {
    let header = get_from_memory::<u32>(show_info + 0x44);
    let result = match plan_pending_node_insert(header, unit_type_id) {
        PendingNodeInsertPlan::Found(node) => (node, false),
        PendingNodeInsertPlan::NewRoot => {
            let new_node = allocate_pending_script_node(unit_type_id);
            save_to_memory(new_node + 4, header);
            save_to_memory(header + 4, new_node);
            save_to_memory(header + 8, new_node);
            (new_node, true)
        }
        PendingNodeInsertPlan::InsertLeft { parent, parent_is_leftmost } => {
            let new_node = allocate_pending_script_node(unit_type_id);
            save_to_memory(new_node + 4, parent);
            save_to_memory(parent + 8, new_node);
            if parent_is_leftmost {
                save_to_memory(header + 8, new_node);
            }
            (new_node, true)
        }
        PendingNodeInsertPlan::InsertRight { parent } => {
            let new_node = allocate_pending_script_node(unit_type_id);
            save_to_memory(new_node + 4, parent);
            save_to_memory(parent + 0xc, new_node);
            (new_node, true)
        }
    };
    if result.1 {
        increment_pending_script_node_count(show_info);
    }
    result
}

pub(crate) fn find_pending_script_node(header: u32, unit_type_id: u32) -> Option<u32> {
    let mut candidate = header;
    let mut cursor = get_from_memory::<u32>(header + 4);
    while cursor != 0 {
        if get_from_memory::<u32>(cursor + 0x10) < unit_type_id {
            cursor = get_from_memory::<u32>(cursor + 0xc);
        } else {
            candidate = cursor;
            cursor = get_from_memory::<u32>(cursor + 0x8);
        }
    }
    if candidate == header || unit_type_id < get_from_memory::<u32>(candidate + 0x10) {
        None
    } else {
        Some(candidate)
    }
}

pub fn add_script(show_info: u32, unit_type_id: u32, new_script_id: u16) -> bool {
    if new_script_id == 0 || new_script_id == 0xffff || unit_type_id == 0 {
        return false;
    }

    let (node, was_inserted) = find_or_insert_pending_script_node(show_info, unit_type_id);
    if was_inserted {
        let mut date = FILETIME::default();
        unsafe { GET_DATE.original()(globals().ztgamemgr_ptr() as *const u32, &mut date as *const FILETIME) };
        let date_ticks = ((date.dwHighDateTime as u64) << 32) | date.dwLowDateTime as u64;
        save_to_memory(node + 0x40, date_ticks as i64);
    }

    save_to_memory(node + 0x1e, new_script_id);

    let started = unsafe { IS_STARTED.hooked()(show_info as *const u32) } != 0;
    if !started {
        let old_current = get_from_memory::<u16>(node + 0x1c);
        save_to_memory(node + 0x1c, new_script_id);
        save_to_memory(node + 0x1e, 0xffffu16);
        if super::super::script::script_exists_by_id(old_current) {
            super::super::script::unregister_script_by_id(old_current);
        }
    }

    let current_id = get_from_memory::<u16>(node + 0x1c);
    let has_items = super::super::script::script_exists_by_id(current_id)
        && super::super::script::script_item_count_by_id(current_id) > 0;
    unsafe {
        if has_items {
            ADD_SHOW.hooked()(show_info as *const u32, unit_type_id);
        } else {
            REMOVE_SHOW.hooked()(show_info as *const u32, unit_type_id);
        }
    }
    true
}

pub(crate) fn clear_pending_script_tree(this: u32) {
    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);

    for node in nodes {
        let sentinel = get_from_memory::<u32>(node + 0x18);
        if sentinel != 0 {
            let mut cursor = get_from_memory::<u32>(sentinel);
            while cursor != sentinel {
                let next = get_from_memory::<u32>(cursor);
                free_unit_array_buffer(cursor, 0xc);
                cursor = next;
            }
            free_unit_array_buffer(sentinel, 0xc);
        }
        free_unit_array_buffer(node, 0x48);
    }

    save_to_memory(header + 4, 0u32);
    save_to_memory(header + 8, header);
    save_to_memory(this + 0x48, 0u32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_unit_type_returns_true_with_low_byte_set_when_script_not_found() {
        let mut fake_show_info = [0u8; 0x10];
        fake_show_info[8..10].copy_from_slice(&0xfffeu16.to_le_bytes());
        let this = fake_show_info.as_ptr() as u32;
        assert_eq!(check_unit_type(this, 42) & 1, 1);
    }
}

#[cfg(test)]
mod pending_node_plan_tests {
    use super::*;

    #[repr(C)]
    struct FakeNode {
        _unused: [u8; 8],
        left: u32,
        right: u32,
        key: u32,
    }

    struct Arena {
        header: Box<[u8; 0x10]>,
        nodes: Vec<Box<FakeNode>>,
    }

    impl Arena {
        fn new() -> Self {
            let arena = Arena { header: Box::new([0u8; 0x10]), nodes: Vec::new() };
            let header = arena.header_addr();
            save_to_memory(header + 8, header);
            arena
        }

        fn header_addr(&self) -> u32 {
            self.header.as_ptr() as u32
        }

        fn push_node(&mut self, key: u32, left: u32, right: u32) -> u32 {
            let node = Box::new(FakeNode { _unused: [0; 8], left, right, key });
            let addr = node.as_ref() as *const FakeNode as u32;
            self.nodes.push(node);
            addr
        }

        fn set_root(&mut self, root: u32) {
            save_to_memory(self.header_addr() + 4, root);
        }

        fn set_leftmost(&mut self, leftmost: u32) {
            save_to_memory(self.header_addr() + 8, leftmost);
        }
    }

    #[test]
    fn empty_tree_plans_new_root() {
        let arena = Arena::new();
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 5), PendingNodeInsertPlan::NewRoot);
    }

    #[test]
    fn single_node_smaller_key_plans_insert_right() {
        let mut arena = Arena::new();
        let root = arena.push_node(5, 0, 0);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 10), PendingNodeInsertPlan::InsertRight { parent: root });
    }

    #[test]
    fn single_node_larger_key_plans_insert_left_as_new_leftmost() {
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(
            plan_pending_node_insert(arena.header_addr(), 5),
            PendingNodeInsertPlan::InsertLeft { parent: root, parent_is_leftmost: true }
        );
    }

    #[test]
    fn exact_key_match_is_found() {
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 10), PendingNodeInsertPlan::Found(root));
    }

    #[test]
    fn insert_left_when_parent_is_not_the_current_leftmost() {
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        let right_child = arena.push_node(15, 0, 0);
        save_to_memory(root + 0xc, right_child);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(
            plan_pending_node_insert(arena.header_addr(), 12),
            PendingNodeInsertPlan::InsertLeft { parent: right_child, parent_is_leftmost: false }
        );
    }

    #[test]
    fn insert_right_after_descending_left_then_right() {
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        let left_child = arena.push_node(3, 0, 0);
        save_to_memory(root + 8, left_child);
        arena.set_root(root);
        arena.set_leftmost(left_child);
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 7), PendingNodeInsertPlan::InsertRight { parent: left_child });
    }

    fn insert(arena: &mut Arena, key: u32) -> u32 {
        let header = arena.header_addr();
        match plan_pending_node_insert(header, key) {
            PendingNodeInsertPlan::Found(node) => node,
            PendingNodeInsertPlan::NewRoot => {
                let node = arena.push_node(key, 0, 0);
                save_to_memory(node + 4, header);
                arena.set_root(node);
                arena.set_leftmost(node);
                node
            }
            PendingNodeInsertPlan::InsertLeft { parent, parent_is_leftmost } => {
                let node = arena.push_node(key, 0, 0);
                save_to_memory(node + 4, parent);
                save_to_memory(parent + 8, node);
                if parent_is_leftmost {
                    arena.set_leftmost(node);
                }
                node
            }
            PendingNodeInsertPlan::InsertRight { parent } => {
                let node = arena.push_node(key, 0, 0);
                save_to_memory(node + 4, parent);
                save_to_memory(parent + 0xc, node);
                node
            }
        }
    }

    fn vanilla_inorder_walk(header: u32) -> Vec<u32> {
        let mut visited = Vec::new();
        let mut node = get_from_memory::<u32>(header + 8);
        if node == header {
            return visited;
        }
        loop {
            visited.push(node);
            let right = get_from_memory::<u32>(node + 0xc);
            if right != 0 {
                node = right;
                loop {
                    let left = get_from_memory::<u32>(node + 8);
                    if left == 0 {
                        break;
                    }
                    node = left;
                }
            } else {
                let mut parent = get_from_memory::<u32>(node + 4);
                if node == get_from_memory::<u32>(parent + 0xc) {
                    loop {
                        node = parent;
                        parent = get_from_memory::<u32>(node + 4);
                        if node != get_from_memory::<u32>(parent + 0xc) {
                            break;
                        }
                    }
                }
                if get_from_memory::<u32>(node + 0xc) != parent {
                    node = parent;
                }
            }
            if node == header {
                break;
            }
        }
        visited
    }

    fn keys_of(_arena: &Arena, nodes: &[u32]) -> Vec<u32> {
        nodes.iter().map(|&n| get_from_memory::<u32>(n + 0x10)).collect()
    }

    #[test]
    fn vanilla_walk_visits_every_node_ascending_insert() {
        let mut arena = Arena::new();
        for key in [1, 2, 3, 4, 5, 6, 7, 8] {
            insert(&mut arena, key);
        }
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn vanilla_walk_visits_every_node_descending_insert() {
        let mut arena = Arena::new();
        for key in [8, 7, 6, 5, 4, 3, 2, 1] {
            insert(&mut arena, key);
        }
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn vanilla_walk_visits_every_node_mixed_insert_order() {
        let mut arena = Arena::new();
        for key in [50, 25, 75, 10, 30, 60, 90, 5, 15, 27, 40, 55, 65, 80, 95] {
            insert(&mut arena, key);
        }
        let mut expected: Vec<u32> = vec![50, 25, 75, 10, 30, 60, 90, 5, 15, 27, 40, 55, 65, 80, 95];
        expected.sort();
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), expected);
    }

    #[test]
    fn vanilla_walk_after_new_node_becomes_new_overall_rightmost() {
        let mut arena = Arena::new();
        for key in [10, 20, 30, 40] {
            insert(&mut arena, key);
        }
        insert(&mut arena, 999);
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![10, 20, 30, 40, 999]);
    }

    #[test]
    fn vanilla_walk_single_node_tree() {
        let mut arena = Arena::new();
        insert(&mut arena, 42);
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![42]);
    }

    #[test]
    fn vanilla_walk_empty_tree() {
        let arena = Arena::new();
        assert_eq!(vanilla_inorder_walk(arena.header_addr()), Vec::<u32>::new());
    }
}
