use openzt_detour::generated::ztgamemgr::{START, STOP};
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

#[detour_mod]
mod hooks {
    use super::*;

    #[detour(START)]
    unsafe extern "fastcall" fn ztgamemgr_start(this: i32) {
        unsafe { START_DETOUR.call(this) };
        crate::ui::set_live_game_active(true);
    }

    #[detour(STOP)]
    unsafe extern "fastcall" fn ztgamemgr_stop(this: i32) {
        crate::ui::set_live_game_active(false);
        unsafe { STOP_DETOUR.call(this) };
    }
}

pub fn init() {
    match unsafe { hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized ZTGameMgr live-game detours"),
        Err(err) => error!("egui overlay: failed to initialize live-game detours: {err}"),
    }
}
