//! Redirects `zthabitat::SEND_EVENT` (`ZTHabitat::sendEvent`, `0x004ffe80`) into an in-memory record of
//! `(this, event_id, category, tile_ptr)` calls instead of letting it run for real, so
//! `ZTHABITAT_SEND_MAINT_WORKER_CLEANUP_EVENTS_MATCHES_REAL_LIVE` can call real vanilla
//! `sendMaintWorkerCleanupEvents` and observe exactly which tiles it would have notified the AI about,
//! without delivering any event. Same thread-local capture-window shape as `portal_dispatch_recorder.rs`.

use std::cell::{Cell, RefCell};

use openzt_detour::generated::zthabitat::SEND_EVENT;
use openzt_detour_macro::detour_mod;

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

#[detour_mod]
mod detours {
    use super::*;

    #[detour(SEND_EVENT)]
    unsafe extern "thiscall" fn send_event(this: *const u32, event_id: u16, unused_a: u32, category: u8, tile_ptr: u32, unused_b: u16, flag: u16) {
        if REDIRECT_ACTIVE.with(|a| a.get()) {
            RECORDED.with(|r| r.borrow_mut().push((this as u32, event_id, category, tile_ptr)));
            return;
        }
        unsafe { SEND_EVENT_DETOUR.call(this, event_id, unused_a, category, tile_ptr, unused_b, flag) };
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        tracing::error!("Failed to initialise send_event_recorder detours: {e:?}");
    }
}
