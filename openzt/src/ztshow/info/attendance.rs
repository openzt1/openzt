use crate::util::{get_from_memory, save_to_memory};

use super::pending_scripts::{collect_pending_script_nodes, find_or_insert_pending_script_node};

pub fn increment_attendance(this: u32, amount: i32) {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, get_from_memory::<u32>(this + 0xc));
    save_to_memory(node + 0x34, get_from_memory::<i32>(node + 0x34).wrapping_add(amount));
    save_to_memory(node + 0x3c, get_from_memory::<i32>(node + 0x3c).wrapping_add(amount));
    save_to_memory(this + 0x7c, get_from_memory::<i32>(this + 0x7c).wrapping_add(amount));
    save_to_memory(this + 0x84, get_from_memory::<i32>(this + 0x84).wrapping_add(amount));
}

pub fn increment_receipts(this: u32, amount: f32) {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, get_from_memory::<u32>(this + 0xc));
    save_to_memory(node + 0x28, get_from_memory::<f32>(node + 0x28) + amount);
    save_to_memory(node + 0x30, get_from_memory::<f32>(node + 0x30) + amount);
    save_to_memory(this + 0x94, get_from_memory::<f32>(this + 0x94) + amount);
    save_to_memory(this + 0x9c, get_from_memory::<f32>(this + 0x9c) + amount);
}

pub(crate) fn apply_engagement_sample(this: u32, sample: f32) {
    save_to_memory(this + 0x88, sample);
    save_to_memory(this + 0x90, sample + get_from_memory::<f32>(this + 0x90));
}

pub(crate) fn roll_monthly_totals(this: u32) {
    save_to_memory(this + 0x98, get_from_memory::<f32>(this + 0x94));
    save_to_memory(this + 0x80, get_from_memory::<i32>(this + 0x7c));
    save_to_memory(this + 0x7c, 0i32);
    save_to_memory(this + 0x94, 0f32);

    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);
    for node in nodes {
        let attendance = get_from_memory::<i32>(node + 0x34);
        save_to_memory(node + 0x34, 0i32);
        save_to_memory(node + 0x38, attendance);
        let receipts = get_from_memory::<f32>(node + 0x28);
        save_to_memory(node + 0x2c, receipts);
        save_to_memory(node + 0x28, 0f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_show_info() -> [u8; 0xa8] {
        [0u8; 0xa8]
    }

    #[test]
    fn increment_attendance_accumulates_into_node_counters_and_instance_totals() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x48];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node);
        save_to_memory(node + 0x10, 42u32);

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0xc, 42u32);
        save_to_memory(node + 0x34, 100i32);
        save_to_memory(node + 0x3c, 200i32);
        save_to_memory(this + 0x7c, 300i32);
        save_to_memory(this + 0x84, 400i32);

        increment_attendance(this, 5);
        increment_attendance(this, 3);
        increment_attendance(this, -2);

        assert_eq!(get_from_memory::<i32>(node + 0x34), 106);
        assert_eq!(get_from_memory::<i32>(node + 0x3c), 206);
        assert_eq!(get_from_memory::<i32>(this + 0x7c), 306);
        assert_eq!(get_from_memory::<i32>(this + 0x84), 406);
        assert_eq!(get_from_memory::<u32>(this + 0x44), header);
        assert_eq!(get_from_memory::<u32>(this + 0x48), 0);
    }

    #[test]
    fn increment_attendance_wraps_like_vanillas_raw_dword_add() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x48];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node);
        save_to_memory(node + 0x10, 7u32);

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0xc, 7u32);
        save_to_memory(this + 0x7c, i32::MAX);

        increment_attendance(this, 1);

        assert_eq!(get_from_memory::<i32>(this + 0x7c), i32::MIN);
    }

    #[test]
    fn increment_receipts_accumulates_float_fields() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x48];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node);
        save_to_memory(node + 0x10, 42u32);

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0xc, 42u32);
        save_to_memory(node + 0x28, 10.5f32);
        save_to_memory(node + 0x30, 20.25f32);
        save_to_memory(this + 0x94, 30.5f32);
        save_to_memory(this + 0x9c, 40.75f32);

        increment_receipts(this, 2.5);
        increment_receipts(this, 0.75);

        assert_eq!(get_from_memory::<f32>(node + 0x28), 13.75);
        assert_eq!(get_from_memory::<f32>(node + 0x30), 23.5);
        assert_eq!(get_from_memory::<f32>(this + 0x94), 33.75);
        assert_eq!(get_from_memory::<f32>(this + 0x9c), 44.0);
        assert_eq!(get_from_memory::<u32>(this + 0x44), header);
    }
}
