use openzt_detour::generated::standalone::{CLICK_ZOOM_IN, CLICK_ZOOM_OUT};
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

#[detour_mod]
mod zoom_block_hooks {
    use super::*;

    #[detour(CLICK_ZOOM_IN)]
    unsafe extern "stdcall" fn click_zoom_in() {
        if crate::ui::is_vanilla_ui_visible() {
            return;
        }

        unsafe { CLICK_ZOOM_IN_DETOUR.call() };
    }

    #[detour(CLICK_ZOOM_OUT)]
    unsafe extern "stdcall" fn click_zoom_out() {
        if crate::ui::is_vanilla_ui_visible() {
            return;
        }

        unsafe { CLICK_ZOOM_OUT_DETOUR.call() };
    }
}

pub fn init() {
    match unsafe { zoom_block_hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized vanilla zoom suppression detours"),
        Err(err) => error!("egui overlay: failed to initialize vanilla zoom suppression detours: {err}"),
    }
}
