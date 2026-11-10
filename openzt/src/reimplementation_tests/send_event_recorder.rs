//! Redirects `ZTHabitat::sendEvent` (`0x004ffe80`, via [`intercept`] from the port's own detour) into an in-memory record of
//! `(this, event_id, category, tile_ptr)` calls instead of letting it run for real, so
//! `ZTHABITAT_SEND_MAINT_WORKER_CLEANUP_EVENTS_MATCHES_REAL_LIVE` can call real vanilla
//! `sendMaintWorkerCleanupEvents` and observe exactly which tiles it would have notified the AI about,
//! without delivering any event. Same thread-local capture-window shape as `portal_dispatch_recorder.rs`.

use std::cell::{Cell, RefCell};

thread_local! {
    static REDIRECT_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static RECORDED: RefCell<Vec<(u32, u16, u8, u32)>> = const { RefCell::new(Vec::new()) };
}

pub fn begin_capture() {
    RECORDED.with(|r| r.borrow_mut().clear());
    REDIRECT_ACTIVE.with(|a| a.set(true));
}

pub fn end_capture() -> Vec<(u32, u16, u8, u32)> {
    REDIRECT_ACTIVE.with(|a| a.set(false));
    RECORDED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// Called first by [`crate::zthabitat::ZTHabitat::send_event`] (the `sendEvent` port behind the production
/// detour, since two detours cannot share an address): while a capture window is open, records the call
/// and reports it consumed instead of letting it reach `GLOBAL_ZTAIMgr`.
pub fn intercept(this: u32, event_id: u16, category: u8, tile_ptr: u32) -> bool {
    if !REDIRECT_ACTIVE.with(|a| a.get()) {
        return false;
    }
    RECORDED.with(|r| r.borrow_mut().push((this, event_id, category, tile_ptr)));
    true
}
