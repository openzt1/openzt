use openzt_detour::generated::{
    bfworldmgr::{GET_ENTITY, GET_TYPE, GET_UNIT},
    poolalloc::{self, ALLOCATE as ALLOCATE_UNIT_ARRAY},
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    ztshowscript::CONSTRUCTOR as ZTSHOW_SCRIPT_CONSTRUCTOR,
    ztui_showpanel::FORCE_UPDATE,
};

use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, save_to_memory},
    ztmegatilemgr::entity_type_matches,
};

use super::pending_scripts::{
    add_script, check_unit_type, collect_pending_script_nodes, find_or_insert_pending_script_node,
    find_pending_script_node,
};

pub fn get_num_units(this: u32, unit_type_id: u32) -> i32 {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    let sentinel = get_from_memory::<u32>(node + 0x18);
    let mut cursor = get_from_memory::<u32>(sentinel);
    let mut count = 0i32;
    while cursor != sentinel {
        count += 1;
        cursor = get_from_memory::<u32>(cursor);
    }
    count
}

pub fn get_show_unit_list(this: u32, unit_type_id: u32) -> u32 {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    node + 0x18
}

pub fn check_unit(this: u32, unit_id: u32) -> u32 {
    if unit_id == 0 {
        return 0;
    }
    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_ptr = unsafe { GET_UNIT.original()(world, unit_id as i32) };
    if unit_ptr == 0 {
        return 0;
    }
    if !unsafe { entity_type_matches(unit_ptr, super::super::show::RVA_SHOW_TRICK_TYPE_CHECK) } {
        return 0;
    }
    let entity_type_ptr = get_from_memory::<u32>(unit_ptr + 0x128);
    let unit_type_id = unsafe { super::super::show::call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };
    check_unit_type(this, unit_type_id)
}

pub fn remove_unit(this: u32, unit_type_id: u32, unit_id: u32) {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    let sentinel = get_from_memory::<u32>(node + 0x18);
    let mut cursor = get_from_memory::<u32>(sentinel);
    while cursor != sentinel {
        if get_from_memory::<u32>(cursor + 0x8) == unit_id {
            let prev = get_from_memory::<u32>(cursor + 0x4);
            let next = get_from_memory::<u32>(cursor);
            save_to_memory(prev, next);
            save_to_memory(next + 0x4, prev);
            free_unit_array_buffer(cursor, 0xc);
            break;
        }
        cursor = get_from_memory::<u32>(cursor);
    }
    unsafe { FORCE_UPDATE.original()() };
}

pub const RVA_UNIT_ARRAY_FREELIST_BUCKETS: u32 = 0x0023_8000;

pub(crate) fn free_unit_array_buffer(buf: u32, byte_capacity: u32) {
    if buf == 0 {
        return;
    }
    if byte_capacity > 0x80 {
        unsafe { OPERATOR_DELETE.original()(buf) };
        return;
    }
    let bucket_head_addr =
        get_module_base("zoo.exe") as u32 + RVA_UNIT_ARRAY_FREELIST_BUCKETS + (((byte_capacity - 1) >> 3) * 4);
    let old_head = get_from_memory::<u32>(bucket_head_addr);
    save_to_memory(buf, old_head);
    save_to_memory(bucket_head_addr, buf);
}

fn pool_allocate(byte_size: u32) -> u32 {
    let bucket_head_addr =
        get_module_base("zoo.exe") as u32 + RVA_UNIT_ARRAY_FREELIST_BUCKETS + (((byte_size - 1) >> 3) * 4);
    let mut head = get_from_memory::<u32>(bucket_head_addr);
    if head == 0 {
        unsafe { poolalloc::REFILL.original()(byte_size as i32) };
        head = get_from_memory::<u32>(bucket_head_addr);
    }
    if head == 0 {
        return 0;
    }
    let next = get_from_memory::<u32>(head);
    save_to_memory(bucket_head_addr, next);
    head
}

fn insert_unit_list_entry(position: u32, value: u32) -> u32 {
    let entry = pool_allocate(0xc);
    if entry == 0 {
        return 0;
    }
    let prev = get_from_memory::<u32>(position + 4);
    save_to_memory(entry, position);
    save_to_memory(entry + 4, prev);
    save_to_memory(entry + 8, value);
    save_to_memory(prev, entry);
    save_to_memory(position + 4, entry);
    entry
}

pub fn add_unit_to_list(this: u32, unit_ptr: u32) -> bool {
    if unit_ptr == 0 {
        return false;
    }

    let entity_type_ptr = get_from_memory::<u32>(unit_ptr + 0x128);
    let unit_type_id = unsafe { super::super::show::call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };
    let unit_id = get_from_memory::<u32>(unit_ptr + 0x124);

    let (node, was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    if was_inserted {
        let script_ptr = create_default_script(this, unit_type_id);
        if script_ptr != 0 {
            let script_id = get_from_memory::<u16>(script_ptr + 4);
            add_script(this, unit_type_id, script_id);
        }
    }

    let sentinel = get_from_memory::<u32>(node + 0x18);
    let mut cursor = get_from_memory::<u32>(sentinel);
    let mut already_present = false;
    while cursor != sentinel {
        if get_from_memory::<u32>(cursor + 0x8) == unit_id {
            already_present = true;
            break;
        }
        cursor = get_from_memory::<u32>(cursor);
    }
    if !already_present {
        insert_unit_list_entry(sentinel, unit_id);
    }

    unsafe { FORCE_UPDATE.original()() };
    true
}

pub fn add_unit(this: u32, unit_ptr: u32) -> bool {
    if unit_ptr == 0 {
        return false;
    }
    add_unit_to_list(this, unit_ptr)
}

pub fn gather_units(this: u32, unit_type_id: u32) -> bool {
    let header = get_from_memory::<u32>(this + 0x44);
    let Some(node) = find_pending_script_node(header, unit_type_id) else {
        return false;
    };

    let world = globals().ztworldmgr_ptr() as *const u32;
    let sentinel = get_from_memory::<u32>(node + 0x18);

    let mut found_eligible = false;
    let mut cursor = get_from_memory::<u32>(sentinel);
    while cursor != sentinel {
        let next = get_from_memory::<u32>(cursor);
        let unit_id = get_from_memory::<u32>(cursor + 0x8);
        let entity_ptr = unsafe { GET_ENTITY.original()(world, unit_id as i32, true) } as u32;
        if entity_ptr != 0 && unsafe { entity_type_matches(entity_ptr, super::super::show::RVA_SHOW_TRICK_TYPE_CHECK) } {
            found_eligible = true;
        }
        cursor = next;
    }

    found_eligible
}

pub fn add_show(this: u32, unit_type_id: u32) {
    if unit_type_id == 0 {
        return;
    }

    let begin = get_from_memory::<u32>(this + 0x50);
    let end = get_from_memory::<u32>(this + 0x54);
    let mut cursor = begin;
    while cursor != end {
        if get_from_memory::<u32>(cursor) == unit_type_id {
            return;
        }
        cursor += 4;
    }

    let cap_end = get_from_memory::<u32>(this + 0x58);
    if end != cap_end {
        if end != 0 {
            save_to_memory(end, unit_type_id);
        }
        save_to_memory(this + 0x54, end + 4);
        return;
    }

    let count = (end - begin) >> 2;
    let new_count = if count == 0 { 1 } else { count * 2 };
    let new_buf = unsafe { ALLOCATE_UNIT_ARRAY.original()(new_count * 4) as u32 };

    let mut src = begin;
    let mut dst = new_buf;
    for _ in 0..count {
        if dst != 0 {
            save_to_memory(dst, get_from_memory::<u32>(src));
            dst += 4;
        }
        src += 4;
    }
    if dst != 0 {
        save_to_memory(dst, unit_type_id);
        dst += 4;
    }

    free_unit_array_buffer(begin, cap_end - begin);

    save_to_memory(this + 0x50, new_buf);
    save_to_memory(this + 0x54, dst);
    save_to_memory(this + 0x58, new_buf + new_count * 4);
}

pub fn remove_show(this: u32, unit_type_id: u32) {
    let begin = get_from_memory::<u32>(this + 0x50);
    let mut end = get_from_memory::<u32>(this + 0x54);
    let mut cursor = begin;
    while cursor != end {
        if get_from_memory::<u32>(cursor) == unit_type_id {
            let mut dst = cursor;
            let mut src = cursor + 4;
            while src != end {
                save_to_memory(dst, get_from_memory::<u32>(src));
                dst += 4;
                src += 4;
            }
            end -= 4;
            save_to_memory(this + 0x54, end);
            break;
        }
        cursor += 4;
    }

    let count = ((end as i32) - (begin as i32)) >> 2;
    if get_from_memory::<i32>(this + 0xa4) >= count {
        save_to_memory(this + 0xa4, 0i32);
    }
}

pub const COMPLEXITY_BUDGET_RVA: u32 = 0x0023_e4ac;
pub const SENTINEL_TRICK_ID: u16 = 0x2c23;

fn dat(rva: u32) -> u32 {
    get_module_base("zoo.exe") as u32 + rva
}

pub fn create_default_script(this: u32, unit_type_id: u32) -> u32 {
    let world = globals().ztworldmgr_ptr() as *const u32;
    let type_ptr = unsafe { GET_TYPE.original()(world, unit_type_id as i32) } as u32;
    if type_ptr == 0 || !unsafe { super::super::show::type_check(type_ptr, super::super::show::RVA_ANIMAL_TYPE_CHECK) } {
        return 0;
    }

    let alloc = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
    if alloc == 0 {
        return 0;
    }
    let script_ptr = unsafe { ZTSHOW_SCRIPT_CONSTRUCTOR.original()(alloc as *const u32, unit_type_id, true) } as u32;
    if script_ptr == 0 {
        return 0;
    }

    let budget = get_from_memory::<i32>(dat(COMPLEXITY_BUDGET_RVA)) as i64;
    let mut complexity_accum: i64 = 0;
    for item_ptr in super::super::ui::walk_trick_list(type_ptr) {
        if !super::super::ui::validate_trick(this, item_ptr) {
            continue;
        }
        let raw = unsafe { &*(item_ptr as *const super::super::script::ZTShowScriptItemRaw) };
        super::super::script::add_item(script_ptr, raw);

        complexity_accum += get_from_memory::<u32>(item_ptr + 0x44) as i64;
        if complexity_accum >= budget {
            if let Some(sentinel_ptr) = super::super::ui::find_trick_by_id(type_ptr, SENTINEL_TRICK_ID) {
                let sentinel_raw = unsafe { &*(sentinel_ptr as *const super::super::script::ZTShowScriptItemRaw) };
                super::super::script::add_item(script_ptr, sentinel_raw);
            }
            complexity_accum = 0;
        }
    }

    script_ptr
}

pub(crate) fn scheduled_species_ids(this: u32) -> Vec<u32> {
    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);

    nodes
        .into_iter()
        .map(|node| get_from_memory::<u32>(node + 0x10))
        .filter(|&key| key != 0 && key != 0x2550)
        .collect()
}

pub(crate) fn merge_registered_unit_types(this: u32, source: u32) {
    let source_begin = get_from_memory::<u32>(source + 0x50);
    let source_end = get_from_memory::<u32>(source + 0x54);
    let count = (source_end - source_begin) >> 2;

    let dest_begin = get_from_memory::<u32>(this + 0x50);
    let dest_cap_end = get_from_memory::<u32>(this + 0x58);
    free_unit_array_buffer(dest_begin, dest_cap_end - dest_begin);

    if count == 0 {
        save_to_memory(this + 0x50, 0u32);
        save_to_memory(this + 0x54, 0u32);
        save_to_memory(this + 0x58, 0u32);
        return;
    }

    let new_buf = unsafe { ALLOCATE_UNIT_ARRAY.original()(count * 4) as u32 };
    let mut src = source_begin;
    let mut dst = new_buf;
    for _ in 0..count {
        if dst != 0 {
            save_to_memory(dst, get_from_memory::<u32>(src));
            dst += 4;
        }
        src += 4;
    }
    save_to_memory(this + 0x50, new_buf);
    save_to_memory(this + 0x54, dst);
    save_to_memory(this + 0x58, new_buf + count * 4);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_show_info() -> [u8; 0xa8] {
        [0u8; 0xa8]
    }

    #[test]
    fn add_show_appends_within_existing_capacity() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [0u32; 4];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr);
        save_to_memory(this + 0x58, array_addr + 16);

        add_show(this, 42);
        assert_eq!(get_from_memory::<u32>(array_addr), 42);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 4);

        add_show(this, 99);
        assert_eq!(get_from_memory::<u32>(array_addr + 4), 99);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 8);
    }

    #[test]
    fn add_show_skips_existing_duplicate_and_zero_id() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [7u32, 0, 0, 0];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr + 4);
        save_to_memory(this + 0x58, array_addr + 16);

        add_show(this, 7);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 4);

        add_show(this, 0);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 4);
    }

    #[test]
    fn remove_show_erases_match_and_shifts_later_elements() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [1u32, 2, 3, 4];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr + 16);
        save_to_memory(this + 0x58, array_addr + 16);
        save_to_memory(this + 0xa4, 0i32);

        remove_show(this, 2);

        assert_eq!(get_from_memory::<u32>(array_addr), 1);
        assert_eq!(get_from_memory::<u32>(array_addr + 4), 3);
        assert_eq!(get_from_memory::<u32>(array_addr + 8), 4);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 12);
    }
}
