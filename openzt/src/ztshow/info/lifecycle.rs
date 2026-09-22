use openzt_detour::generated::{
    bfevent::{CONSTRUCTOR as BFEVENT_CONSTRUCTOR, LOAD as BFEVENT_LOAD, SAVE as BFEVENT_SAVE},
    poolalloc::ALLOCATE as ALLOCATE_UNIT_ARRAY,
    standalone::{DEALLOCATE, OPERATOR_NEW, WRITE_BYTES_TO_FILE},
    ztgamemgr::GET_DATE,
    ztshow::{ABORT_SHOW, LOAD as ZTSHOW_LOAD, SAVE as ZTSHOW_SAVE},
    ztshowinfo::CALCULATE_ALL_FROM_TYPES as SET_DEFAULT_SATISFACTION_FIELDS,
    ztworldmgr::GET_GRANDSTANDS_UPKEEP,
};
use windows::Win32::Foundation::FILETIME;

use crate::{
    globals::globals,
    util::{get_from_memory, mut_from_memory, save_to_memory},
};

use super::{
    attendance::{apply_engagement_sample, roll_monthly_totals},
    pending_scripts::{clear_pending_script_tree, collect_pending_script_nodes, find_or_insert_pending_script_node},
    schedule::{get_scheduled_show_keeper_type, get_scheduled_show_script},
    units::{add_show, free_unit_array_buffer, get_num_units, merge_registered_unit_types},
};

pub fn is_ready(this: u32) -> bool {
    get_scheduled_show_script(this);
    get_from_memory::<u8>(this + 0x22) != 0
}

pub fn is_started(this: u32) -> bool {
    let scheduled = get_scheduled_show_script(this);
    if get_from_memory::<u8>(this + 0x23) == 0 {
        return false;
    }
    get_from_memory::<u16>(this + 0x8) == scheduled
}

pub fn is_stopped(this: u32) -> bool {
    !is_ready(this) && !is_started(this)
}

pub fn has_keeper(this: u32) -> bool {
    let keeper_type = get_scheduled_show_keeper_type(this);
    get_num_units(this, keeper_type) > 0
}

pub fn needs_keeper(this: u32, unit_type_id: u32) -> bool {
    let keeper_type = get_scheduled_show_keeper_type(this);
    if unit_type_id != keeper_type {
        return false;
    }
    if get_num_units(this, unit_type_id) >= 1 {
        return false;
    }
    is_ready(this)
}

pub fn get_events(this: u32, arg: u32) {
    let field_0x70 = get_from_memory::<u16>(this + 0x70) as u32;
    let vtable = get_from_memory::<u32>(this);
    let slot = get_from_memory::<u32>(vtable + 0xc);
    let f: unsafe extern "thiscall" fn(u32, u32, u32, u32) = unsafe { std::mem::transmute(slot) };
    unsafe { f(this, arg, field_0x70, 0x53) };
}

pub fn send_event(this: u32, event_id: u16, unused: u32, category: u8, value: u32, value2: u16, flag: u16) {
    let ai_mgr = globals().ztaimgr_ptr() as u32;
    if ai_mgr == 0 {
        return;
    }
    let target = ai_mgr + 0x8;
    let field_0x70 = get_from_memory::<u16>(this + 0x70);
    let vtable = get_from_memory::<u32>(target);
    let slot = get_from_memory::<u32>(vtable + 0x4);
    let f: unsafe extern "thiscall" fn(u32, u16, u32, u8, u16, u32, u32, u16, u16) = unsafe { std::mem::transmute(slot) };
    unsafe { f(target, event_id, unused, category, field_0x70, 0x53, value, value2, flag) };
}

pub fn listen(this: u32) {
    get_events(this, this + 0x5c);

    let target_id = get_from_memory::<u16>(this + 0x70) as u32;
    let mut cursor = get_from_memory::<u32>(this + 0x5c);
    if cursor == get_from_memory::<u32>(this + 0x60) {
        return;
    }
    loop {
        if get_from_memory::<u32>(cursor + 0xc) == target_id {
            match get_from_memory::<u16>(cursor + 0x16) {
                0x2714 => {
                    let ztshow = this + 0x4;
                    let vtable = get_from_memory::<u32>(ztshow);
                    let slot = get_from_memory::<u32>(vtable + 0x14);
                    let f: unsafe extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(slot) };
                    unsafe { f(ztshow) };
                }
                0x2716 => {
                    unsafe { ABORT_SHOW.original()((this + 0x4) as *const u32) };
                }
                _ => {}
            }
        }
        cursor += 0x1c;
        if cursor == get_from_memory::<u32>(this + 0x60) {
            break;
        }
    }
}

pub fn cleanup_events(this: u32) {
    let begin = get_from_memory::<u32>(this + 0x5c);
    save_to_memory(this + 0x60, begin);
}

pub fn enter_new_month(this: u32) {
    roll_monthly_totals(this);
    save_to_memory(this + 0x8c, get_from_memory::<f32>(this + 0x88));

    let world = globals().ztworldmgr_ptr() as *const u32;
    let target_id = get_from_memory::<u16>(this + 0x70);
    let sample = unsafe { GET_GRANDSTANDS_UPKEEP.original()(world, target_id as i16) };
    apply_engagement_sample(this, sample);
}

pub fn update(this: u32) {
    listen(this);

    let ztshow = this + 0x4;
    let vtable = get_from_memory::<u32>(ztshow);
    let slot = get_from_memory::<u32>(vtable + 0xc);
    let f: unsafe extern "thiscall" fn(u32) = unsafe { std::mem::transmute(slot) };
    unsafe { f(ztshow) };

    cleanup_events(this);

    save_to_memory(this + 0x90, get_from_memory::<f32>(this + 0x90) - get_from_memory::<f32>(this + 0x88));
    let world = globals().ztworldmgr_ptr() as *const u32;
    let target_id = get_from_memory::<u16>(this + 0x70);
    let sample = unsafe { GET_GRANDSTANDS_UPKEEP.original()(world, target_id as i16) };
    apply_engagement_sample(this, sample);
}

pub fn set_show_info_id(this: u32, id: u16) -> bool {
    save_to_memory(this + 0x70, id);
    let ztshow = this + 0x4;
    let back_pointer: u32 = get_from_memory(ztshow + 0x10);
    if back_pointer == 0 || get_from_memory::<u16>(back_pointer + 0x70) != id {
        save_to_memory(ztshow + 0x10, this);
    }
    save_to_memory(ztshow + 0x6, id);
    true
}

fn copy_pending_script_value_fields(dest_node: u32, source_node: u32) {
    save_to_memory(dest_node + 0x1c, get_from_memory::<u16>(source_node + 0x1c));
    save_to_memory(dest_node + 0x1e, get_from_memory::<u16>(source_node + 0x1e));
    save_to_memory(dest_node + 0x24, get_from_memory::<u32>(source_node + 0x24));
    save_to_memory(dest_node + 0x28, get_from_memory::<u32>(source_node + 0x28));
    save_to_memory(dest_node + 0x2c, get_from_memory::<u32>(source_node + 0x2c));
    save_to_memory(dest_node + 0x30, get_from_memory::<u32>(source_node + 0x30));
    save_to_memory(dest_node + 0x34, get_from_memory::<u32>(source_node + 0x34));
    save_to_memory(dest_node + 0x38, get_from_memory::<u32>(source_node + 0x38));
    save_to_memory(dest_node + 0x3c, get_from_memory::<u32>(source_node + 0x3c));
    save_to_memory(dest_node + 0x40, get_from_memory::<u32>(source_node + 0x40));
    save_to_memory(dest_node + 0x44, get_from_memory::<u32>(source_node + 0x44));
}

fn clone_script_state_value(source_value: u32) -> u32 {
    let new_value = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
    if new_value == 0 {
        return 0;
    }
    for offset in (0..0x14u32).step_by(4) {
        save_to_memory(new_value + offset, get_from_memory::<u32>(source_value + offset));
    }
    new_value
}

pub fn update_from_load(this: u32, source: u32) {
    let mgr_ptr = globals().ztshowmgr_ptr();
    if mgr_ptr.is_null() {
        return;
    }

    save_to_memory(this + 0x68, get_from_memory::<u32>(source + 0x68));
    save_to_memory(this + 0xa4, get_from_memory::<u32>(source + 0xa4));

    merge_registered_unit_types(this, source);

    let source_header = get_from_memory::<u32>(source + 0x44);
    let source_root = get_from_memory::<u32>(source_header + 4);
    let mut source_nodes = Vec::new();
    collect_pending_script_nodes(source_root, &mut source_nodes);
    for source_node in source_nodes {
        let key = get_from_memory::<u32>(source_node + 0x10);
        let (dest_node, _was_inserted) = find_or_insert_pending_script_node(this, key);
        copy_pending_script_value_fields(dest_node, source_node);
    }

    let old_id = get_from_memory::<u16>(this + 0x70);
    save_to_memory(this + 0x6c, get_from_memory::<u32>(source + 0x6c));
    let new_id = get_from_memory::<u16>(source + 0x70);
    save_to_memory(this + 0x70, new_id);
    save_to_memory(this + 0x88, get_from_memory::<u32>(source + 0x88));
    save_to_memory(this + 0x8c, get_from_memory::<u32>(source + 0x8c));
    save_to_memory(this + 0x90, get_from_memory::<u32>(source + 0x90));
    save_to_memory(this + 0x94, get_from_memory::<u32>(source + 0x94));
    save_to_memory(this + 0x98, get_from_memory::<u32>(source + 0x98));
    save_to_memory(this + 0x9c, get_from_memory::<u32>(source + 0x9c));
    save_to_memory(this + 0x7c, get_from_memory::<u32>(source + 0x7c));
    save_to_memory(this + 0x80, get_from_memory::<u32>(source + 0x80));
    save_to_memory(this + 0x84, get_from_memory::<u32>(source + 0x84));

    let dest_ztshow = this + 0x4;
    let source_ztshow = source + 0x4;
    save_to_memory(dest_ztshow + 0x4, get_from_memory::<u16>(source_ztshow + 0x4));
    save_to_memory(dest_ztshow + 0x6, get_from_memory::<u16>(source_ztshow + 0x6));
    save_to_memory(dest_ztshow + 0x8, get_from_memory::<u32>(source_ztshow + 0x8));
    save_to_memory(dest_ztshow + 0xc, get_from_memory::<u32>(source_ztshow + 0xc));
    save_to_memory(dest_ztshow + 0x10, get_from_memory::<u32>(source_ztshow + 0x10));
    save_to_memory(dest_ztshow + 0x14, get_from_memory::<u32>(source_ztshow + 0x14));

    let dest_state = dest_ztshow + 0x18;
    let source_state = source_ztshow + 0x18;
    save_to_memory(dest_state + 0x4, get_from_memory::<u16>(source_state + 0x4));
    save_to_memory(dest_state + 0x6, get_from_memory::<u8>(source_state + 0x6));
    save_to_memory(dest_state + 0x7, get_from_memory::<u8>(source_state + 0x7));
    save_to_memory(dest_state + 0x8, get_from_memory::<u8>(source_state + 0x8));
    save_to_memory(dest_state + 0xc, get_from_memory::<u32>(source_state + 0xc));
    save_to_memory(dest_state + 0x10, get_from_memory::<u32>(source_state + 0x10));
    save_to_memory(dest_state + 0x14, get_from_memory::<u32>(source_state + 0x14));
    save_to_memory(dest_state + 0x18, get_from_memory::<u32>(source_state + 0x18));
    save_to_memory(dest_state + 0x24, get_from_memory::<u8>(source_state + 0x24));

    super::super::state::show_state_clear(dest_state);
    let source_state_header = get_from_memory::<u32>(source_state + 0x1c);
    let source_state_root = get_from_memory::<u32>(source_state_header + 0x4);
    let mut source_state_nodes = Vec::new();
    super::super::state::collect_tree_nodes(source_state_root, &mut source_state_nodes);
    let dest_state_header = get_from_memory::<u32>(dest_state + 0x1c);
    for source_state_node in source_state_nodes {
        let key = get_from_memory::<u32>(source_state_node + 0x10);
        let dest_state_node = super::super::state::find_or_insert_state_node(dest_state_header, key);
        let source_value = get_from_memory::<u32>(source_state_node + 0x14);
        let new_value = clone_script_state_value(source_value);
        save_to_memory(dest_state_node + 0x14, new_value);
    }
    save_to_memory(dest_state + 0x20, get_from_memory::<u32>(source_state + 0x20));

    set_show_info_id(this, new_id);
    let mgr = unsafe { mut_from_memory::<super::super::mgr::ZTShowMgr>(mgr_ptr as u32) };
    mgr.register_show(this as *const u32, false);
    if old_id != new_id {
        let old_show = super::super::mgr::ZTShowMgr::get_show_info(old_id);
        if old_show != 0 {
            mgr.unregister_show(old_id, old_show as *const u32, false);
        }
    }
}

fn write_field(addr: u32, size: u32, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(addr as *const u32, size, 1, file) == 1 }
}

fn read_field(addr: u32, size: u32, file: *const u32) -> bool {
    unsafe { DEALLOCATE.hooked()(addr as *const u32, size, 1, file as *const u8) == 1 }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn show_info_save(this: u32, file: *const i8) -> bool {
    let mut ok = write_field(this + 0x68, 4, file);
    ok &= write_field(this + 0xa4, 4, file);

    let begin = get_from_memory::<u32>(this + 0x50);
    let end = get_from_memory::<u32>(this + 0x54);
    let show_count = (end - begin) >> 2;
    ok &= write_field(&show_count as *const u32 as u32, 4, file);
    let mut cursor = begin;
    while cursor != end {
        ok &= write_field(cursor, 4, file);
        cursor += 4;
    }

    let node_count = get_from_memory::<u32>(this + 0x48);
    ok &= write_field(&node_count as *const u32 as u32, 4, file);

    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);
    for node in nodes {
        ok &= write_field(node + 0x10, 4, file);
        ok &= write_field(node + 0x1c, 2, file);
        ok &= write_field(node + 0x1e, 2, file);
        ok &= write_field(node + 0x24, 4, file);
        ok &= write_field(node + 0x28, 4, file);
        ok &= write_field(node + 0x2c, 4, file);
        ok &= write_field(node + 0x30, 4, file);
        ok &= write_field(node + 0x34, 4, file);
        ok &= write_field(node + 0x38, 4, file);
        ok &= write_field(node + 0x3c, 4, file);
        ok &= write_field(node + 0x40, 8, file);
    }

    let ev_begin = get_from_memory::<u32>(this + 0x5c);
    let ev_end = get_from_memory::<u32>(this + 0x60);
    let ev_count = (ev_end - ev_begin) / 0x1c;
    ok &= write_field(&ev_count as *const u32 as u32, 4, file);
    let mut ev_cursor = ev_begin;
    while ev_cursor != ev_end {
        let saved = unsafe { BFEVENT_SAVE.original()(ev_cursor as *const u32, file) };
        ok &= saved;
        if !ok {
            return false;
        }
        ev_cursor += 0x1c;
    }

    ok &= write_field(this + 0x6c, 4, file);
    ok &= write_field(this + 0x70, 2, file);
    ok &= write_field(this + 0x88, 4, file);
    ok &= write_field(this + 0x8c, 4, file);
    ok &= write_field(this + 0x90, 4, file);
    ok &= write_field(this + 0x94, 4, file);
    ok &= write_field(this + 0x98, 4, file);
    ok &= write_field(this + 0x9c, 4, file);
    ok &= write_field(this + 0x7c, 4, file);
    ok &= write_field(this + 0x80, 4, file);
    ok &= write_field(this + 0x84, 4, file);

    let show_ok = unsafe { ZTSHOW_SAVE.original()((this + 0x4) as *const u32, file as *const u32) };
    ok && show_ok
}

pub struct PendingScriptRecord {
    pub current: u16,
    pub pending: u16,
    pub field_0x24: u32,
    pub receipts_current: u32,
    pub field_0x2c: u32,
    pub receipts_total: u32,
    pub attendance_current: u32,
    pub field_0x38: u32,
    pub attendance_total: u32,
    pub filetime_low: u32,
    pub filetime_high: u32,
}

impl PendingScriptRecord {
    pub fn defaulted(current: u16, filetime_low: u32, filetime_high: u32) -> Self {
        PendingScriptRecord {
            current,
            pending: 0,
            field_0x24: 0,
            receipts_current: 0,
            field_0x2c: 0,
            receipts_total: 0,
            attendance_current: 0,
            field_0x38: 0,
            attendance_total: 0,
            filetime_low,
            filetime_high,
        }
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn show_info_load(this: u32, file: *const u32, version: u32) -> bool {
    let mut ok = read_field(this + 0x68, 4, file);
    ok &= read_field(this + 0xa4, 4, file);

    let mut show_count: u32 = 0;
    ok &= read_field(&mut show_count as *mut u32 as u32, 4, file);

    let old_begin = get_from_memory::<u32>(this + 0x50);
    if old_begin != 0 {
        let old_cap_end = get_from_memory::<u32>(this + 0x58);
        free_unit_array_buffer(old_begin, old_cap_end - old_begin);
    }
    save_to_memory(this + 0x50, 0u32);
    save_to_memory(this + 0x54, 0u32);
    save_to_memory(this + 0x58, 0u32);
    for _ in 0..show_count {
        let mut id: u32 = 0;
        ok &= read_field(&mut id as *mut u32 as u32, 4, file);
        add_show(this, id);
    }

    if version < 99 {
        let mut legacy_count: u32 = 0;
        ok &= read_field(&mut legacy_count as *mut u32 as u32, 4, file);
        for _ in 0..legacy_count {
            let mut discard_a: u32 = 0;
            let mut discard_b: u32 = 0;
            ok &= read_field(&mut discard_a as *mut u32 as u32, 4, file);
            ok &= read_field(&mut discard_b as *mut u32 as u32, 4, file);
        }
    }

    let mut node_count: u32 = 0;
    ok &= read_field(&mut node_count as *mut u32 as u32, 4, file);

    if get_from_memory::<u32>(this + 0x48) != 0 {
        clear_pending_script_tree(this);
    }

    for _ in 0..node_count {
        let mut unit_type_id: u32 = 0;
        ok &= read_field(&mut unit_type_id as *mut u32 as u32, 4, file);

        let mut current: u16 = 0;
        ok &= read_field(&mut current as *mut u16 as u32, 2, file);

        let record = if version < 99 {
            let mut date = FILETIME::default();
            unsafe { GET_DATE.original()(globals().ztgamemgr_ptr() as *const u32, &mut date as *const FILETIME) };
            PendingScriptRecord::defaulted(current, date.dwLowDateTime, date.dwHighDateTime)
        } else {
            let mut pending: u16 = 0;
            ok &= read_field(&mut pending as *mut u16 as u32, 2, file);
            let mut field_0x24: u32 = 0;
            ok &= read_field(&mut field_0x24 as *mut u32 as u32, 4, file);
            let mut receipts_current: u32 = 0;
            ok &= read_field(&mut receipts_current as *mut u32 as u32, 4, file);
            let mut field_0x2c: u32 = 0;
            ok &= read_field(&mut field_0x2c as *mut u32 as u32, 4, file);
            let mut receipts_total: u32 = 0;
            ok &= read_field(&mut receipts_total as *mut u32 as u32, 4, file);
            let mut attendance_current: u32 = 0;
            ok &= read_field(&mut attendance_current as *mut u32 as u32, 4, file);
            let mut field_0x38: u32 = 0;
            ok &= read_field(&mut field_0x38 as *mut u32 as u32, 4, file);
            let mut attendance_total: u32 = 0;
            ok &= read_field(&mut attendance_total as *mut u32 as u32, 4, file);
            let mut filetime_low: u32 = 0;
            ok &= read_field(&mut filetime_low as *mut u32 as u32, 4, file);
            let mut filetime_high: u32 = 0;
            ok &= read_field(&mut filetime_high as *mut u32 as u32, 4, file);
            PendingScriptRecord {
                current,
                pending,
                field_0x24,
                receipts_current,
                field_0x2c,
                receipts_total,
                attendance_current,
                field_0x38,
                attendance_total,
                filetime_low,
                filetime_high,
            }
        };

        let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
        save_to_memory(node + 0x1c, record.current);
        save_to_memory(node + 0x1e, record.pending);
        save_to_memory(node + 0x24, record.field_0x24);
        save_to_memory(node + 0x28, record.receipts_current);
        save_to_memory(node + 0x2c, record.field_0x2c);
        save_to_memory(node + 0x30, record.receipts_total);
        save_to_memory(node + 0x34, record.attendance_current);
        save_to_memory(node + 0x38, record.field_0x38);
        save_to_memory(node + 0x3c, record.attendance_total);
        save_to_memory(node + 0x40, record.filetime_low);
        save_to_memory(node + 0x44, record.filetime_high);
    }

    if version > 0x60 {
        let ev_begin = get_from_memory::<u32>(this + 0x5c);
        save_to_memory(this + 0x60, ev_begin);

        let mut ev_count: u32 = 0;
        ok &= read_field(&mut ev_count as *mut u32 as u32, 4, file);

        if ev_count != 0 {
            let buf = unsafe { ALLOCATE_UNIT_ARRAY.original()(ev_count * 0x1c) as u32 };
            for i in 0..ev_count {
                let slot = buf + i * 0x1c;
                unsafe { BFEVENT_CONSTRUCTOR.original()(slot as *const u32) };
                let loaded = unsafe { BFEVENT_LOAD.original()(slot as *const u32, file, version) };
                ok &= loaded;
                if !ok {
                    return false;
                }
            }
            save_to_memory(this + 0x5c, buf);
            save_to_memory(this + 0x60, buf + ev_count * 0x1c);
            save_to_memory(this + 0x64, buf + ev_count * 0x1c);
        }

        ok &= read_field(this + 0x6c, 4, file);
        ok &= read_field(this + 0x70, 2, file);
    }

    if version > 0x68 {
        ok &= read_field(this + 0x88, 4, file);
        ok &= read_field(this + 0x8c, 4, file);
        ok &= read_field(this + 0x90, 4, file);
    }

    if version < 0x6a {
        unsafe { SET_DEFAULT_SATISFACTION_FIELDS.original()(this as *const u32) };
    } else {
        ok &= read_field(this + 0x94, 4, file);
        ok &= read_field(this + 0x98, 4, file);
        ok &= read_field(this + 0x9c, 4, file);
        ok &= read_field(this + 0x7c, 4, file);
        ok &= read_field(this + 0x80, 4, file);
        ok &= read_field(this + 0x84, 4, file);
    }

    let show_ok = unsafe { ZTSHOW_LOAD.original()((this + 0x4) as *const u32, file, version) };
    ok && show_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_show_info() -> [u8; 0xa8] {
        [0u8; 0xa8]
    }

    #[test]
    fn is_ready_reads_field_0x22_when_schedule_is_empty() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        assert!(!is_ready(this));
        save_to_memory(this + 0x22, 1u8);
        assert!(is_ready(this));
    }

    #[test]
    fn is_started_requires_flag_and_matching_current_script_id() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        assert!(!is_started(this));

        save_to_memory(this + 0x23, 1u8);
        save_to_memory(this + 0x8, 5u16);
        assert!(!is_started(this));

        save_to_memory(this + 0x8, 0u16);
        assert!(is_started(this));
    }

    #[test]
    fn is_stopped_is_true_only_when_neither_ready_nor_started() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        assert!(is_stopped(this));

        save_to_memory(this + 0x22, 1u8);
        assert!(!is_stopped(this));

        save_to_memory(this + 0x22, 0u8);
        save_to_memory(this + 0x23, 1u8);
        save_to_memory(this + 0x8, 0u16);
        assert!(!is_stopped(this));
    }
}
