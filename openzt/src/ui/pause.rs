use std::sync::atomic::{AtomicBool, Ordering};

use openzt_detour::generated::ztui_main::{PAUSE_GAME, UNPAUSE_GAME};
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

static GAME_PAUSED: AtomicBool = AtomicBool::new(false);

#[detour_mod]
mod hooks {
    use super::*;

    #[detour(PAUSE_GAME)]
    unsafe extern "stdcall" fn ztui_main_pause_game() {
        unsafe { PAUSE_GAME_DETOUR.call() };
        set_paused(true);
    }

    #[detour(UNPAUSE_GAME)]
    unsafe extern "stdcall" fn ztui_main_unpause_game() {
        unsafe { UNPAUSE_GAME_DETOUR.call() };
        set_paused(false);
    }
}

pub fn init() {
    match unsafe { hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized ZTUI::main pause detours"),
        Err(err) => error!("egui overlay: failed to initialize pause detours: {err}"),
    }
}

pub fn is_paused() -> bool {
    GAME_PAUSED.load(Ordering::Acquire)
}

pub fn set_paused(paused: bool) {
    GAME_PAUSED.store(paused, Ordering::Release);
}

pub fn click_toggle_pause() {
    unsafe {
        if is_paused() {
            UNPAUSE_GAME.original()();
        } else {
            PAUSE_GAME.original()();
        }
    }
}
