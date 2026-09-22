use openzt_detour::generated::{bfworldmgr::GET_TYPE, ztshow::ABORT_SHOW};

use crate::{
    globals::globals,
    util::{get_from_memory, save_to_memory},
};

use super::{
    lifecycle::is_started,
    pending_scripts::find_or_insert_pending_script_node,
};

pub(crate) fn scheduled_unit_type_id(this: u32) -> u32 {
    let begin = get_from_memory::<u32>(this + 0x50);
    let end = get_from_memory::<u32>(this + 0x54);
    if begin == end {
        return 0;
    }
    let slot = get_from_memory::<u32>(this + 0xa4);
    get_from_memory::<u32>(begin + slot * 4)
}

pub fn get_scheduled_show_script(this: u32) -> u16 {
    let unit_type_id = scheduled_unit_type_id(this);
    if unit_type_id == 0 {
        return 0;
    }
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    get_from_memory::<u16>(node + 0x1c)
}

pub fn get_scheduled_show_keeper_type(this: u32) -> u32 {
    let unit_type_id = scheduled_unit_type_id(this);
    let world = globals().ztworldmgr_ptr() as *const u32;
    let type_ptr = unsafe { GET_TYPE.original()(world, unit_type_id as i32) } as u32;
    if type_ptr != 0 && unsafe { super::super::show::type_check(type_ptr, super::super::show::RVA_ANIMAL_TYPE_CHECK) } {
        0x2550
    } else {
        0
    }
}

pub fn recalculate_schedule(this: u32, advance_slot: bool) {
    if advance_slot {
        let count = (get_from_memory::<i32>(this + 0x54) - get_from_memory::<i32>(this + 0x50)) >> 2;
        if count > 0 {
            let slot = get_from_memory::<i32>(this + 0xa4).wrapping_add(1);
            save_to_memory(this + 0xa4, slot % count);
        }
    }
    let frequency = get_from_memory::<i32>(this + 0x68);
    if frequency == -1 {
        save_to_memory(this + 0x6c, -1i32);
    } else {
        let ai_mgr = globals().ztaimgr_ptr() as u32;
        save_to_memory(this + 0x6c, get_from_memory::<i32>(ai_mgr + 0xec).wrapping_add(frequency));
    }
}

pub fn set_show_frequency(this: u32, frequency: i32) {
    if get_from_memory::<i32>(this + 0x68) == frequency {
        return;
    }
    save_to_memory(this + 0x68, frequency);
    recalculate_schedule(this, false);
    if get_from_memory::<i32>(this + 0x68) == -1 && !is_started(this) {
        unsafe { ABORT_SHOW.original()((this + 0x4) as *const u32) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_show_info() -> [u8; 0xa8] {
        [0u8; 0xa8]
    }

    #[test]
    fn get_scheduled_show_script_returns_zero_when_schedule_is_empty() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        assert_eq!(get_scheduled_show_script(this), 0);
    }

    #[test]
    fn get_scheduled_show_script_reads_current_field_of_matching_tree_node() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x20];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node);
        save_to_memory(node + 0x10, 42u32);
        save_to_memory(node + 0x1c, 0x1234u16);

        let mut schedule_buf = [0u8; 4];
        let schedule = schedule_buf.as_mut_ptr() as u32;
        save_to_memory(schedule, 42u32);

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0x50, schedule);
        save_to_memory(this + 0x54, schedule + 4);
        save_to_memory(this + 0xa4, 0u32);

        assert_eq!(get_scheduled_show_script(this), 0x1234);
    }

    #[test]
    fn recalculate_schedule_advances_and_wraps_the_slot_cursor() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut schedule_buf = [0u32; 3];
        let schedule = schedule_buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, schedule);
        save_to_memory(this + 0x54, schedule + 12);
        save_to_memory(this + 0x68, -1i32);

        recalculate_schedule(this, true);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 1);
        recalculate_schedule(this, true);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 2);
        recalculate_schedule(this, true);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 0);

        recalculate_schedule(this, false);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 0);
        assert_eq!(get_from_memory::<i32>(this + 0x6c), -1);
    }

    #[test]
    fn recalculate_schedule_skips_the_advance_when_the_schedule_is_empty() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x68, -1i32);

        recalculate_schedule(this, true);

        assert_eq!(get_from_memory::<i32>(this + 0xa4), 0);
        assert_eq!(get_from_memory::<i32>(this + 0x6c), -1);
    }

    #[test]
    fn recalculate_schedule_wraps_the_cursor_like_vanillas_inc_idiv() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut schedule_buf = [0u32; 3];
        let schedule = schedule_buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, schedule);
        save_to_memory(this + 0x54, schedule + 12);
        save_to_memory(this + 0xa4, i32::MAX);
        save_to_memory(this + 0x68, -1i32);

        recalculate_schedule(this, true);

        assert_eq!(get_from_memory::<i32>(this + 0xa4), i32::MIN % 3);
    }

    #[test]
    fn set_show_frequency_short_circuits_when_the_frequency_is_unchanged() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x68, 7i32);
        save_to_memory(this + 0x6c, 0x5au32);

        set_show_frequency(this, 7);

        assert_eq!(get_from_memory::<u32>(this + 0x6c), 0x5a);
    }

    #[test]
    fn set_show_frequency_to_never_while_started_stops_at_the_sentinel() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x23, 1u8);

        set_show_frequency(this, -1);

        assert_eq!(get_from_memory::<i32>(this + 0x68), -1);
        assert_eq!(get_from_memory::<i32>(this + 0x6c), -1);
    }
}
