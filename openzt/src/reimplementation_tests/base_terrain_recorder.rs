//! Redirects `zttankexhibit::SET_BASE_TERRAIN_TYPE` (`ZTTankExhibit::setBaseTerrainType`, `0x00458bea`) into an
//! in-memory record of `(tank_ptr, terrain_type)` calls instead of painting the map, so
//! `ZTHABITATMGR_DO_SHOW_CHECK_DISPATCHES_TANK_OVERRIDES_LIVE` can observe whether `doShowCheck` reached the
//! tank's `setIsShowExhibit`/`setIsNotShowExhibit` overrides (the only callers of this function) without
//! repainting a live tank. Same thread-local capture-window shape as `portal_dispatch_recorder.rs`. Retire
//! this once `setBaseTerrainType` itself is detoured by production code (two detours cannot share an address).

use std::cell::{Cell, RefCell};

use openzt_detour::generated::zttankexhibit::SET_BASE_TERRAIN_TYPE;
use openzt_detour_macro::detour_mod;

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

#[detour_mod]
mod detours {
    use super::*;

    #[detour(SET_BASE_TERRAIN_TYPE)]
    unsafe extern "thiscall" fn set_base_terrain_type(this: *const u32, terrain_type: i32) {
        if REDIRECT_ACTIVE.with(|a| a.get()) {
            RECORDED.with(|r| r.borrow_mut().push((this as u32, terrain_type)));
            return;
        }
        unsafe { SET_BASE_TERRAIN_TYPE_DETOUR.call(this, terrain_type) };
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        tracing::error!("Failed to initialise base_terrain_recorder detours: {e:?}");
    }
}
