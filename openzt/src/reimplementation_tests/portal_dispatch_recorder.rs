//! Redirects `zttankwall::SET_IS_OPEN_PORTAL` (`ZTTankWall::setIsOpenPortal`, `0x0059ea94`) into an
//! in-memory record of `(fence_ptr, is_open_portal, play_sound)` calls instead of letting it run for
//! real, so `ZTHABITAT_UPDATE_PORTALS_MATCHES_REAL_LIVE` can call real vanilla `updatePortals`
//! (via `hooks_zthabitatmgr::update_portals_real`) and observe exactly which fences it would have
//! dispatched to, without ever flipping a real fence's open/close state or firing its sound. Same
//! thread-local capture-window shape as `io_redirect.rs`.

use std::cell::{Cell, RefCell};

use openzt_detour::generated::zttankwall::SET_IS_OPEN_PORTAL;
use openzt_detour_macro::detour_mod;

thread_local! {
    static REDIRECT_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static RECORDED: RefCell<Vec<(u32, bool, bool)>> = const { RefCell::new(Vec::new()) };
}

pub fn begin_capture() {
    RECORDED.with(|r| r.borrow_mut().clear());
    REDIRECT_ACTIVE.with(|a| a.set(true));
}

pub fn end_capture() -> Vec<(u32, bool, bool)> {
    REDIRECT_ACTIVE.with(|a| a.set(false));
    RECORDED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(SET_IS_OPEN_PORTAL)]
    unsafe extern "thiscall" fn set_is_open_portal(this: *const u32, is_open_portal: bool, play_sound: bool) {
        if REDIRECT_ACTIVE.with(|a| a.get()) {
            RECORDED.with(|r| r.borrow_mut().push((this as u32, is_open_portal, play_sound)));
            return;
        }
        unsafe { SET_IS_OPEN_PORTAL_DETOUR.call(this, is_open_portal, play_sound) };
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        tracing::error!("Failed to initialise portal_dispatch_recorder detours: {e:?}");
    }
}
