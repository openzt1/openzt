use std::sync::atomic::{AtomicI32, Ordering};

use openzt_detour::generated::ztui_main::{SET_ANIMAL_RATING, SET_GUEST_RATING, SET_ZOO_RATING};
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

const DEFAULT_RATING: i32 = 75;

static ANIMAL_RATING: AtomicI32 = AtomicI32::new(DEFAULT_RATING);
static GUEST_RATING: AtomicI32 = AtomicI32::new(DEFAULT_RATING);
static ZOO_RATING: AtomicI32 = AtomicI32::new(DEFAULT_RATING);

#[detour_mod]
mod hooks {
    use super::*;

    #[detour(SET_ANIMAL_RATING)]
    unsafe extern "cdecl" fn ztui_main_set_animal_rating(value: i32) {
        unsafe { SET_ANIMAL_RATING_DETOUR.call(value) };
        set_animal_rating(value);
    }

    #[detour(SET_GUEST_RATING)]
    unsafe extern "cdecl" fn ztui_main_set_guest_rating(value: i32) {
        unsafe { SET_GUEST_RATING_DETOUR.call(value) };
        set_guest_rating(value);
    }

    #[detour(SET_ZOO_RATING)]
    unsafe extern "cdecl" fn ztui_main_set_zoo_rating(value: i32) {
        unsafe { SET_ZOO_RATING_DETOUR.call(value) };
        set_zoo_rating(value);
    }
}

pub fn init() {
    match unsafe { hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized ZTUI::main rating detours"),
        Err(err) => error!("egui overlay: failed to initialize rating detours: {err}"),
    }
}

pub fn set_animal_rating(value: i32) {
    ANIMAL_RATING.store(value, Ordering::Release);
}

pub fn set_guest_rating(value: i32) {
    GUEST_RATING.store(value, Ordering::Release);
}

pub fn set_zoo_rating(value: i32) {
    ZOO_RATING.store(value, Ordering::Release);
}

pub fn animal_rating() -> u8 {
    rating_for_display(ANIMAL_RATING.load(Ordering::Acquire))
}

pub fn guest_rating() -> u8 {
    rating_for_display(GUEST_RATING.load(Ordering::Acquire))
}

pub fn zoo_rating() -> u8 {
    rating_for_display(ZOO_RATING.load(Ordering::Acquire))
}

fn rating_for_display(value: i32) -> u8 {
    value.clamp(0, 100) as u8
}
