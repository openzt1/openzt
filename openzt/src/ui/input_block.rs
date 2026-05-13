use openzt_detour::generated::ztmapview::CHECK_MOUSE_OVER_ENTITY;
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

#[detour_mod]
mod input_block_hooks {
    use super::*;

    #[detour(CHECK_MOUSE_OVER_ENTITY)]
    unsafe extern "thiscall" fn ztmapview_check_mouse_over_entity(this: *const u32) {
        if crate::ui::blocks_pointer_input() {
            return;
        }

        unsafe { CHECK_MOUSE_OVER_ENTITY_DETOUR.call(this) };
    }
}

pub fn init() {
    match unsafe { input_block_hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized input block detours"),
        Err(err) => error!("egui overlay: failed to initialize input block detours: {err}"),
    }
}
