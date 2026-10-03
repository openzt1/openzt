use openzt_detour::generated::{
    bfworldmgr::{GET_TYPE, GET_UNIT},
    zthabitat::PLAY_SHOW_START_SOUND,
    ztshow::{
        CALCULATE_PERCENT_ADJUSTMENT, CHECK_SCRIPT, CLEAR_SHOW_SCRIPT_STATES, DO_CURRENT_ITEM, DO_KEEPER_EVENT, DO_TRICK_EVENT,
        GATHER_UNITS, GET_SHOW_SCRIPT_STATE, REINIT, RESOLVE_NEXT_SCHEDULED_SCRIPT_ID, START, STOP_0, VALIDATE, VALIDATE_ITEM,
    },
    ztshowinfo::{GET_NUM_UNITS, GET_SHOW_UNIT_LIST, IS_STARTED, RECALCULATE_SCHEDULE, SEND_EVENT},
    ztshowmgr::GET_SHOW_INFO,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::globals,
    util::{get_from_memory, save_to_memory},
    ztmegatilemgr::entity_type_matches,
};

use super::info::remove_unit;

pub const RVA_SHOW_TRICK_TYPE_CHECK: u32 = 0x0023_86b0;
pub const RVA_ANIMAL_TYPE_CHECK: u32 = 0x0023_8690;

unsafe fn call_unit_vtable_u16_u16(unit_ptr: u32, slot_offset: u32, arg1: u16, arg2: u16) -> i32 {
    let vtable = get_from_memory::<u32>(unit_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u16, u16) -> i32>(target) };
    f(unit_ptr, arg1, arg2)
}

unsafe fn send_event(show_info: u32, event_id: u16, unused: u32, category: u8, value: u32, value2: u16, flag: u16) {
    unsafe { SEND_EVENT.hooked()(show_info as *const u32, event_id, unused, category, value, value2, flag) };
}

fn find_script_state_node(header: u32, key: u32) -> u32 {
    let mut candidate = header;
    let mut node = get_from_memory::<u32>(header + 0x4);
    while node != 0 {
        if get_from_memory::<u32>(node + 0x10) < key {
            node = get_from_memory::<u32>(node + 0xc);
        } else {
            candidate = node;
            node = get_from_memory::<u32>(node + 0x8);
        }
    }
    candidate
}

pub fn get_show_script_state(ztshow: u32, key: u32) -> u32 {
    let header = get_from_memory::<u32>(ztshow + 0x34);
    let candidate = find_script_state_node(header, key);
    if candidate != header && get_from_memory::<u32>(candidate + 0x10) <= key {
        get_from_memory::<u32>(candidate + 0x14)
    } else {
        0
    }
}

pub fn do_current_item(this: u32, unit_id: u32) -> i32 {
    let state = get_show_script_state(this, unit_id);
    if state == 0 {
        return 5;
    }
    if get_from_memory::<u8>(state + 0xe) != 0 {
        return 0;
    }
    if get_from_memory::<u8>(state + 0x12) != 0 {
        return 0;
    }

    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_ptr = unsafe { GET_UNIT.original()(world, unit_id as i32) };
    if unit_ptr == 0 {
        return -1;
    }
    if !unsafe { entity_type_matches(unit_ptr, RVA_SHOW_TRICK_TYPE_CHECK) } {
        return -1;
    }

    let script_id = get_from_memory::<u16>(this + 0x4);
    let item_count = super::script::script_item_count_by_id(script_id);
    let trick_index = get_from_memory::<u16>(state + 0xc);
    if trick_index as usize >= item_count {
        return -1;
    }
    if trick_index == 0xffff {
        return 0;
    }
    let Some(item) = super::script::item_snapshot_by_id(script_id, trick_index) else {
        return -1;
    };

    let secondary = get_from_memory::<u16>(this + 0x6);
    let result = unsafe { call_unit_vtable_u16_u16(unit_ptr, 0x210, item.id, secondary) };
    if result == 0 {
        save_to_memory(state + 0x12, 1u8);
    }
    result
}

pub fn do_trick_event(this: u32, state_ptr: u32) {
    if state_ptr == 0 {
        return;
    }
    let script_id = get_from_memory::<u16>(this + 0x4);
    let trick_index = get_from_memory::<u16>(state_ptr + 0xc);
    let Some(item) = super::script::item_snapshot_by_id(script_id, trick_index) else {
        return;
    };
    if item.item_type == 3 {
        return;
    }

    let satisfaction_sum = get_from_memory::<i32>(this + 0x2c);
    save_to_memory(this + 0x2c, satisfaction_sum.wrapping_add(item.satisfaction as i32));

    let show_info = get_from_memory::<u32>(this + 0x10);
    let skip_scoring = get_from_memory::<u8>(state_ptr + 0xf) != 0;

    if skip_scoring {
        unsafe { send_event(show_info, 0x272a, 0, 0x57, 0, 0, 1) };
    } else {
        let count = get_from_memory::<i32>(this + 0x28);
        save_to_memory(this + 0x28, count.wrapping_add(1));
        let mirror = item.satisfaction_mirror as i32;
        let mirror_sum = get_from_memory::<i32>(this + 0x30);
        save_to_memory(this + 0x30, mirror_sum.wrapping_add(mirror));

        let mgr_ptr = globals().ztshowmgr_ptr();
        if !mgr_ptr.is_null() {
            let mgr = unsafe { &*mgr_ptr };
            if mirror <= mgr.threshold_a as i32 {
                unsafe {
                    send_event(show_info, 0x272a, 0, 0x57, mirror as u32, (mgr.threshold_a as i32 - mirror) as u16, 1);
                    DO_KEEPER_EVENT.original()(this as *const u32, 0x271f, state_ptr as *const u32);
                }
                return;
            }
            if mirror < mgr.threshold_c as i32 {
                unsafe {
                    if mgr.threshold_b as i32 <= mirror {
                        send_event(show_info, 0x272c, 0, 0x57, mirror as u32, (mirror - mgr.threshold_b as i32) as u16, 1);
                    } else {
                        send_event(show_info, 0x272b, 0, 0x57, mirror as u32, 0, 1);
                    }
                    send_event(show_info, 0x271f, 0, 0x4b, 0, trick_index, 1);
                }
                return;
            }
            unsafe {
                send_event(show_info, 0x272d, 0, 0x57, mirror as u32, (mirror - mgr.threshold_c as i32) as u16, 1);
                DO_KEEPER_EVENT.original()(this as *const u32, 0x271f, state_ptr as *const u32);
            }
            return;
        }
    }
    unsafe { DO_KEEPER_EVENT.original()(this as *const u32, 0x271f, state_ptr as *const u32) };
}

pub fn validate_item(this: u32, index: u16) -> i32 {
    if index == 0xffff {
        return 0;
    }
    let show_info = get_from_memory::<u32>(this + 0x10);
    let unit_type_id = get_from_memory::<u32>(this + 0x8);
    let list_ptr = unsafe { GET_SHOW_UNIT_LIST.hooked()(show_info as *const u32, unit_type_id) } as u32;
    let sentinel = get_from_memory::<u32>(list_ptr);
    let first_node = get_from_memory::<u32>(sentinel);
    let unit_numeric_id = get_from_memory::<u32>(first_node + 0x8);

    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_ptr = unsafe { GET_UNIT.original()(world, unit_numeric_id as i32) };
    if unit_ptr == 0 || !unsafe { entity_type_matches(unit_ptr, RVA_SHOW_TRICK_TYPE_CHECK) } {
        return 0;
    }

    let script_id = get_from_memory::<u16>(this + 0x4);
    let Some(item) = super::script::item_snapshot_by_id(script_id, index) else {
        return -1;
    };
    let arg2 = get_from_memory::<u16>(show_info + 0x70);
    unsafe { call_unit_vtable_u16_u16(unit_ptr, 0x218, item.id, arg2) }
}

pub(crate) unsafe fn type_check(type_ptr: u32, type_check_arg_rva: u32) -> bool {
    let vtable = get_from_memory::<u32>(type_ptr);
    let check_fn = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
    let arg = crate::globals::get_module_base("zoo.exe") as u32 + type_check_arg_rva;
    check_fn(type_ptr, arg)
}

pub(crate) unsafe fn call_entity_vtable_noargs(entity_ptr: u32, slot_offset: u32) -> bool {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> bool>(target) };
    f(entity_ptr)
}

pub(crate) unsafe fn call_entity_vtable_u32_noargs(entity_ptr: u32, slot_offset: u32) -> u32 {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target) };
    f(entity_ptr)
}

pub fn stop_with_id(this: u32, new_script_id: u16) {
    let current_id = get_from_memory::<u16>(this + 0x4);
    if current_id != new_script_id {
        if current_id != 0 {
            unsafe { REINIT.original()(this as *const u32) };
        }
        if let Some(script_type) = super::script::script_type_by_id(new_script_id) {
            save_to_memory(this + 0x4, new_script_id);
            save_to_memory(this + 0x8, script_type);
            let world = globals().ztworldmgr_ptr() as *const u32;
            let unit_type_ptr = unsafe { GET_TYPE.original()(world, script_type as i32) } as u32;
            if unit_type_ptr != 0 && unsafe { type_check(unit_type_ptr, RVA_ANIMAL_TYPE_CHECK) } {
                save_to_memory(this + 0xc, 0x2550u32);
            }
        }
        let show_info = get_from_memory::<u32>(this + 0x10);
        if show_info != 0 {
            unsafe { RECALCULATE_SCHEDULE.hooked()(show_info as *const u32, 0) };
        }
    }
}

pub(crate) fn check_owning_habitat(show_info: u32) -> bool {
    let habitat = get_from_memory::<u32>(show_info + 0xa0);
    habitat != 0 && unsafe { call_entity_vtable_noargs(habitat, 0x20) } && get_from_memory::<u32>(habitat + 0x188) == 0
}

pub fn validate(this: u32, check_units: bool) -> i32 {
    if check_units {
        let show_info = get_from_memory::<u32>(this + 0x10);
        let unit_type_id = get_from_memory::<u32>(this + 0x8);
        let list_ptr = unsafe { GET_SHOW_UNIT_LIST.hooked()(show_info as *const u32, unit_type_id) } as u32;
        let sentinel = get_from_memory::<u32>(list_ptr);

        let mut unit_count = 0;
        let mut node = get_from_memory::<u32>(sentinel);
        while node != sentinel {
            unit_count += 1;
            node = get_from_memory::<u32>(node);
        }
        if unit_count == 0 {
            return 1;
        }

        node = get_from_memory::<u32>(sentinel);
        while node != sentinel {
            let unit_id = get_from_memory::<u32>(node + 0x8);
            if !unsafe { openzt_detour::generated::ztshowinfo::CHECK_UNIT.hooked()(show_info as *const u32, unit_id) } {
                return 4;
            }
            node = get_from_memory::<u32>(node);
        }
    }

    let script_id = get_from_memory::<u16>(this + 0x4);
    let handle = super::script::get_script(script_id);
    if handle != 0 {
        let count = super::script::size(handle);
        let result = run_validate_loop(
            count,
            |index| validate_item(this, index),
            |index| {
                super::script::remove_item(handle, index);
            },
        );
        if super::script::size(handle) != 0 {
            return result;
        }
    }
    6
}

fn run_validate_loop(item_count: i32, mut is_valid: impl FnMut(u16) -> i32, mut remove: impl FnMut(u16)) -> i32 {
    let mut count = item_count;
    let mut index: i32 = 0;
    while index < count {
        if is_valid(index as u16) != 0 {
            remove(index as u16);
            count -= 1;
            index -= 1;
        }
        index += 1;
    }
    0
}

pub fn check_script(this: u32, param_1: u16) -> bool {
    let id = if param_1 != 0 { param_1 } else { get_from_memory::<u16>(this + 0x4) };
    id != 0 && super::script::script_item_count_by_id(id) > 0
}

pub fn calculate_percent_adjustment(this: u32) -> i32 {
    let script_id = get_from_memory::<u16>(this + 0x4);
    let item_count = super::script::script_item_count_by_id(script_id) as i32;
    let threshold = get_from_memory::<i32>(this + 0x28);
    if item_count > 0 && threshold <= item_count {
        let mgr_ptr = globals().ztshowmgr_ptr() as u32;
        let mgr_lower = get_from_memory::<i32>(mgr_ptr + 0x20);
        let mgr_upper = get_from_memory::<i32>(mgr_ptr + 0x24);
        if threshold > mgr_upper {
            return threshold - mgr_upper;
        }
        if threshold < mgr_lower {
            return mgr_lower - threshold;
        }
    }
    0
}

pub fn start(this: u32) {
    let script_id = unsafe { RESOLVE_NEXT_SCHEDULED_SCRIPT_ID.original()(this as *const u32) as u16 };
    let Some(script_type) = super::script::script_type_by_id(script_id) else { return };
    if script_type == 0 {
        return;
    }

    let show_info = get_from_memory::<u32>(this + 0x10);
    let unit_count = unsafe { GET_NUM_UNITS.hooked()(show_info as *const u32, script_type) };
    if unit_count < 1 {
        return;
    }

    if check_owning_habitat(show_info) {
        return;
    }

    save_to_memory(this + 0x8, script_type);
    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_type_ptr = unsafe { GET_TYPE.original()(world, script_type as i32) } as u32;
    if unit_type_ptr != 0 && unsafe { type_check(unit_type_ptr, RVA_ANIMAL_TYPE_CHECK) } {
        save_to_memory(this + 0xc, 0x2550u32);
    }

    stop_with_id(this, script_id);

    if validate(this, true) != 0 {
        return;
    }
    unsafe { CLEAR_SHOW_SCRIPT_STATES.original()(this as *const u32) };

    let unit_type_for_list = get_from_memory::<u32>(this + 0x8);
    let list_ptr = unsafe { GET_SHOW_UNIT_LIST.hooked()(show_info as *const u32, unit_type_for_list) } as u32;
    let sentinel = get_from_memory::<u32>(list_ptr);
    let mut node = get_from_memory::<u32>(sentinel);
    while node != sentinel {
        let next_node = get_from_memory::<u32>(node);
        let unit_id = get_from_memory::<u32>(node + 0x8);
        let unit_ptr = globals().ztworldmgr().resolve_entity_by_id(unit_id) as u32;
        let eligible = unit_ptr != 0 && unsafe { entity_type_matches(unit_ptr, RVA_SHOW_TRICK_TYPE_CHECK) };
        if !eligible {
            remove_unit(show_info, unit_type_for_list, unit_id);
        } else {
            let assigned_show_id = get_from_memory::<u16>(unit_ptr + 0x254);
            let owning_show_info = unsafe { GET_SHOW_INFO.hooked()(globals().ztshowmgr_ptr() as *const u32, assigned_show_id) };
            let needs_state = (!owning_show_info.is_null() && unsafe { IS_STARTED.hooked()(owning_show_info) } == 0)
                || !unsafe { call_entity_vtable_noargs(unit_ptr, 0x22c) };
            if needs_state {
                let show_id = get_from_memory::<u16>(this + 0x6);
                let result = crate::ztshowscriptstate::create_show_script_state(this, unit_id);
                if result != 0 {
                    return;
                }
                save_to_memory(unit_ptr + 0x254, show_id);
            }
        }
        node = next_node;
    }

    let gather_result = unsafe { GATHER_UNITS.original()(this as *const u32) };
    if !gather_result {
        save_to_memory(this + 0x1e, 0u8);
        save_to_memory(this + 0x1f, 1u8);
        save_to_memory(this + 0x20, 0u8);
        let show_info = get_from_memory::<u32>(this + 0x10);
        unsafe { send_event(show_info, 0x2713, 0, 0x57, 0, 0, 1) };
        let habitat = get_from_memory::<u32>(show_info + 0xa0);
        unsafe { PLAY_SHOW_START_SOUND.original()(habitat as *const u32) };
    }
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_SHOW_SCRIPT_STATE)]
    unsafe extern "thiscall" fn get_show_script_state_detour(this: *const u32, key: u32) -> u32 {
        get_show_script_state(this as u32, key)
    }

    #[detour(DO_CURRENT_ITEM)]
    unsafe extern "thiscall" fn do_current_item_detour(this: *const u32, unit_id: u32) -> i32 {
        do_current_item(this as u32, unit_id)
    }

    #[detour(DO_TRICK_EVENT)]
    unsafe extern "thiscall" fn do_trick_event_detour(this: *const u32, state_ptr: *const u32) {
        do_trick_event(this as u32, state_ptr as u32);
    }

    #[detour(VALIDATE_ITEM)]
    unsafe extern "thiscall" fn validate_item_detour(this: *const u32, index: u16) -> u32 {
        validate_item(this as u32, index) as u32
    }

    #[detour(VALIDATE)]
    unsafe extern "thiscall" fn validate_detour(this: *const u32, check_units: bool) -> i32 {
        validate(this as u32, check_units)
    }

    #[detour(STOP_0)]
    unsafe extern "thiscall" fn stop_0_detour(this: *const u32, new_script_id: u32) {
        stop_with_id(this as u32, new_script_id as u16);
    }

    #[detour(START)]
    unsafe extern "thiscall" fn start_detour(this: *const u32) {
        start(this as u32);
    }

    #[detour(CHECK_SCRIPT)]
    unsafe extern "thiscall" fn check_script_detour(this: *const u32, param_1: u16) -> u32 {
        check_script(this as u32, param_1) as u32
    }

    #[detour(CALCULATE_PERCENT_ADJUSTMENT)]
    unsafe extern "fastcall" fn calculate_percent_adjustment_detour(this: *const u32) -> i32 {
        calculate_percent_adjustment(this as u32)
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshow detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::standalone::OPERATOR_NEW;
    use super::*;

    pub(crate) fn build_standalone_show_info() -> u32 {
        let show_info = unsafe { OPERATOR_NEW.original()(0xa8) } as u32;
        unsafe { std::ptr::write_bytes(show_info as *mut u8, 0, 0xa8) };

        let header = unsafe { OPERATOR_NEW.original()(0xc) } as u32;
        unsafe { std::ptr::write_bytes(header as *mut u8, 0, 0xc) };
        save_to_memory(header, header);
        save_to_memory(show_info + 0x44, header);

        show_info
    }

    pub(crate) fn collect_pending_script_nodes(show_info: u32) -> Vec<u32> {
        let header = get_from_memory::<u32>(show_info + 0x44);
        let root = get_from_memory::<u32>(header + 4);
        let mut nodes = Vec::new();
        super::super::info::collect_pending_script_nodes(root, &mut nodes);
        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_script_falls_back_to_own_field_and_rejects_missing_id() {
        let _guard = super::super::script::SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        super::super::script::reset_store_for_test();
        const CTOR_PTR: u32 = 0x9000;
        let id = super::super::script::register_script(CTOR_PTR, 1).unwrap();
        super::super::script::add_item(CTOR_PTR, &super::super::script::test_item(1, 5));

        let mut fake_show = [0u8; 0x8];
        fake_show[4..6].copy_from_slice(&id.to_le_bytes());
        let this = fake_show.as_ptr() as u32;

        assert!(check_script(this, 0));
        assert!(!check_script(this, 0xffff));
    }

    #[test]
    fn run_validate_loop_removes_invalid_items_and_revisits_shifted_position() {
        let items = std::cell::RefCell::new(vec![10, 11, 12, 13, 14]);
        let removed = std::cell::RefCell::new(Vec::new());
        let count = items.borrow().len() as i32;

        let result = run_validate_loop(
            count,
            |index| if items.borrow()[index as usize] % 2 != 0 { 1 } else { 0 },
            |index| {
                let id = items.borrow_mut().remove(index as usize);
                removed.borrow_mut().push(id);
            },
        );

        assert_eq!(result, 0);
        assert_eq!(*removed.borrow(), vec![11, 13]);
        assert_eq!(*items.borrow(), vec![10, 12, 14]);
    }

    #[repr(C)]
    struct FakeStateNode {
        _unused: [u8; 8],
        left: u32,
        right: u32,
        key: u32,
        value: u32,
    }

    #[test]
    fn get_show_script_state_returns_zero_for_empty_tree() {
        let mut show = [0u8; 0x38];
        let header = [0u8; 0x18];
        let header_addr = header.as_ptr() as u32;
        save_to_memory(show.as_mut_ptr() as u32 + 0x34, header_addr);
        let this = show.as_ptr() as u32;
        assert_eq!(get_show_script_state(this, 0), 0);
        assert_eq!(get_show_script_state(this, 0xffff_ffff), 0);
    }

    #[test]
    fn get_show_script_state_finds_single_node_and_misses_other_keys() {
        let mut show = [0u8; 0x38];
        let mut header = [0u8; 0x18];
        let node = FakeStateNode { _unused: [0; 8], left: 0, right: 0, key: 7, value: 0xdead_beef };
        let node_addr = &node as *const FakeStateNode as u32;
        save_to_memory(header.as_mut_ptr() as u32 + 4, node_addr);
        let header_addr = header.as_ptr() as u32;
        save_to_memory(show.as_mut_ptr() as u32 + 0x34, header_addr);
        let this = show.as_ptr() as u32;

        assert_eq!(get_show_script_state(this, 7), 0xdead_beef);
        assert_eq!(get_show_script_state(this, 6), 0);
        assert_eq!(get_show_script_state(this, 8), 0);
    }
}
