use std::sync::atomic::{AtomicBool, Ordering};

use egui::CursorIcon;

static OVERLAY_CURSOR_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn init() {}

pub fn apply_egui_cursor(cursor_icon: CursorIcon, wants_pointer_input: bool) {
    let wants_vanilla_arrow = wants_pointer_input || cursor_icon != CursorIcon::Default;
    let was_active = OVERLAY_CURSOR_ACTIVE.swap(wants_vanilla_arrow, Ordering::Relaxed);

    if wants_vanilla_arrow {
        if !was_active {
            crate::cursors::set_default_cursor();
        }
    } else if was_active {
        crate::cursors::reset_global_element_cursor();
    }
}

pub fn clear_egui_cursor() {
    if OVERLAY_CURSOR_ACTIVE.swap(false, Ordering::Relaxed) {
        crate::cursors::reset_global_element_cursor();
    }
}
