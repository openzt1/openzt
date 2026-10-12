//! Redirects `ZTTankExhibit::setBaseTerrainType` (`0x00458bea`) into an in-memory record of
//! `(tank_ptr, terrain_type)` calls instead of painting the map, so
//! `ZTHABITATMGR_DO_SHOW_CHECK_DISPATCHES_TANK_OVERRIDES_LIVE` can observe whether `doShowCheck` reached the
//! tank's `setIsShowExhibit`/`setIsNotShowExhibit` overrides (the only callers of this function) without
//! repainting a live tank. Same thread-local capture-window shape as `portal_dispatch_recorder.rs`, but the
//! detour on that address belongs to `tank_exhibit::detours`, which calls [`record`] first.

use std::cell::{Cell, RefCell};

thread_local! {
    static REDIRECT_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static RECORDED: RefCell<Vec<(u32, i32)>> = const { RefCell::new(Vec::new()) };
}

pub fn begin_capture() {
    RECORDED.with(|r| r.borrow_mut().clear());
    REDIRECT_ACTIVE.with(|a| a.set(true));
}

pub fn end_capture() -> Vec<(u32, i32)> {
    REDIRECT_ACTIVE.with(|a| a.set(false));
    RECORDED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// Records the call and returns `true` while a capture window is open (the caller then skips the paint).
pub fn record(tank_ptr: u32, terrain_type: i32) -> bool {
    if !REDIRECT_ACTIVE.with(|a| a.get()) {
        return false;
    }
    RECORDED.with(|r| r.borrow_mut().push((tank_ptr, terrain_type)));
    true
}
