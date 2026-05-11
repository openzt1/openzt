use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use openzt_detour::generated::gxvideomanager::{FLIP, FLIP_TO_GDI};
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

static RENDERING: AtomicBool = AtomicBool::new(false);
static FLIP_CALLS: AtomicU32 = AtomicU32::new(0);
static FLIP_TO_GDI_CALLS: AtomicU32 = AtomicU32::new(0);

#[detour_mod]
mod hooks {
    use super::*;

    #[detour(FLIP)]
    unsafe extern "fastcall" fn gxvideomanager_flip(this: *const i32) -> bool {
        let result = unsafe { FLIP_DETOUR.call(this) };
        render_overlay("flip", &FLIP_CALLS);
        result
    }

    #[detour(FLIP_TO_GDI)]
    unsafe extern "cdecl" fn gxvideomanager_flip_to_gdi(param: i8) {
        unsafe { FLIP_TO_GDI_DETOUR.call(param) };
        render_overlay("flip_to_gdi", &FLIP_TO_GDI_CALLS);
    }
}

fn render_overlay(hook_name: &str, counter: &AtomicU32) {
    let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
    if count == 1 || count % 300 == 0 {
        info!("egui overlay: {hook_name} render hook called {count} times");
    }

    if RENDERING.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return;
    }

    if let Some(hwnd) = crate::ui::captured_hwnd() {
        crate::ui::render_and_blit(hwnd);
    }

    RENDERING.store(false, Ordering::Release);
}

pub fn init() {
    match unsafe { hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized GXVideoManager::flip detour"),
        Err(err) => error!("egui overlay: failed to initialize render detour: {err}"),
    }
}
