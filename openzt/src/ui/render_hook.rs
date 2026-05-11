use openzt_detour::generated::gxvideomanager::FLIP;
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

#[detour_mod]
mod hooks {
    use super::*;

    #[detour(FLIP)]
    unsafe extern "fastcall" fn gxvideomanager_flip(this: *const i32) -> bool {
        let result = unsafe { FLIP_DETOUR.call(this) };
        if let Some(hwnd) = crate::ui::captured_hwnd() {
            crate::ui::render_and_blit(hwnd);
        }
        result
    }
}

pub fn init() {
    match unsafe { hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized GXVideoManager::flip detour"),
        Err(err) => error!("egui overlay: failed to initialize render detour: {err}"),
    }
}
