use crate::globals::get_module_base;
use crate::util::mut_from_memory;
use openzt_detour::generated::ztshowmgr::{
    ENTER_NEW_MONTH, GET_SCRIPT_ID, GET_SHOW_INFO, INIT_SHOW_PARAMS, IS_DOING_SHOW, IS_SHOW_SCRIPT_DONE, LOAD, REGISTER_SHOW, SAVE,
    UNREGISTER_SHOW, UPDATE,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

pub mod ztshowmgr;
pub use ztshowmgr::*;

#[detour_mod]
mod detours {
    use super::*;

    #[detour(INIT_SHOW_PARAMS)]
    unsafe extern "thiscall" fn init_show_params_detour(this: *const u32) -> u32 {
        unsafe { mut_from_memory::<ZTShowMgr>(this).init_show_params() }
    }

    #[detour(REGISTER_SHOW)]
    unsafe extern "thiscall" fn register_show_detour(this: *const u32, show: *const u32, force: bool) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).register_show(show, force) != 0 }
    }

    #[detour(UNREGISTER_SHOW)]
    unsafe extern "thiscall" fn unregister_show_detour(this: *const u32, id: u16, show: *const u32, clear: bool) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).unregister_show(id, show, clear) != 0 }
    }

    #[detour(GET_SHOW_INFO)]
    unsafe extern "thiscall" fn get_show_info_detour(_this: *const u32, id: u16) -> *const u32 {
        ZTShowMgr::get_show_info(id) as *const u32
    }

    #[detour(GET_SCRIPT_ID)]
    unsafe extern "thiscall" fn get_script_id_detour(_this: *const u32, id: u16) -> u16 {
        ZTShowMgr::get_script_id(id) as u16
    }

    #[detour(ENTER_NEW_MONTH)]
    unsafe extern "thiscall" fn enter_new_month_detour(_this: *const u32) {
        ZTShowMgr::enter_new_month()
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update_detour(_this: *const u32) {
        ZTShowMgr::update()
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const i8) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).save(file) != 0 }
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load_detour(this: *const u32, file: *const u32, version: u32) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).load(file, version) != 0 }
    }

    #[detour(IS_DOING_SHOW)]
    unsafe extern "thiscall" fn is_doing_show_detour(_this: *const u32, unit_id: u32, show_id: u16) -> bool {
        ZTShowMgr::is_doing_show(unit_id, show_id) != 0
    }

    #[detour(IS_SHOW_SCRIPT_DONE)]
    unsafe extern "thiscall" fn is_show_script_done_detour(_this: *const u32, script_id: u32, show_id: u16) -> u8 {
        ZTShowMgr::is_show_script_done(script_id, show_id) as u8
    }

    pub(super) fn call_real(this: *const u32) -> u32 {
        unsafe { INIT_SHOW_PARAMS_DETOUR.call(this) }
    }

    pub(super) fn call_real_register_show(this: *const u32, show: *const u32, force: bool) -> bool {
        unsafe { REGISTER_SHOW_DETOUR.call(this, show, force) }
    }

    pub(super) fn call_real_unregister_show(this: *const u32, id: u16, show: *const u32, clear: bool) -> bool {
        unsafe { UNREGISTER_SHOW_DETOUR.call(this, id, show, clear) }
    }

    pub(super) fn call_real_get_show_info(this: *const u32, id: u16) -> *const u32 {
        unsafe { GET_SHOW_INFO_DETOUR.call(this, id) }
    }

    pub(super) fn call_real_get_script_id(this: *const u32, id: u16) -> u16 {
        unsafe { GET_SCRIPT_ID_DETOUR.call(this, id) }
    }

    pub(super) fn call_real_enter_new_month(this: *const u32) {
        unsafe { ENTER_NEW_MONTH_DETOUR.call(this) }
    }

    pub(super) fn call_real_update(this: *const u32) {
        unsafe { UPDATE_DETOUR.call(this) }
    }

    pub(super) fn call_real_save(this: *const u32, file: *const i8) -> bool {
        unsafe { SAVE_DETOUR.call(this, file) }
    }

    pub(super) fn call_real_load(this: *const u32, file: *const u32, version: u32) -> bool {
        unsafe { LOAD_DETOUR.call(this, file, version) }
    }

    pub(super) fn call_real_is_doing_show(this: *const u32, unit_id: u32, show_id: u16) -> bool {
        unsafe { IS_DOING_SHOW_DETOUR.call(this, unit_id, show_id) }
    }

    pub(super) fn call_real_is_show_script_done(this: *const u32, script_id: u32, show_id: u16) -> u8 {
        unsafe { IS_SHOW_SCRIPT_DONE_DETOUR.call(this, script_id, show_id) }
    }
}

pub fn init() {
    let counter_addr = (get_module_base("zoo.exe") as u32 + SHOW_ID_COUNTER_RVA) as *const u16;
    SHOW_STORE.lock().unwrap().show_id_counter = unsafe { *counter_addr };
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshowmgr detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;
    use openzt_detour::generated::standalone::OPERATOR_NEW;

    pub(crate) fn allocate_uninitialized() -> *mut ZTShowMgr {
        unsafe { OPERATOR_NEW.original()(0x44) as *mut ZTShowMgr }
    }

    pub(crate) fn call_real_init_show_params(this: *const u32) -> u32 {
        detours::call_real(this)
    }

    pub(crate) fn call_real_get_show_info(this: *const u32, id: u16) -> *const u32 {
        detours::call_real_get_show_info(this, id)
    }

    pub(crate) fn call_real_get_script_id(this: *const u32, id: u16) -> u16 {
        detours::call_real_get_script_id(this, id)
    }

    pub(crate) fn call_real_enter_new_month(this: *const u32) {
        detours::call_real_enter_new_month(this)
    }

    pub(crate) fn call_real_update(this: *const u32) {
        detours::call_real_update(this)
    }

    pub(crate) fn show_id_counter_addr() -> u32 {
        get_module_base("zoo.exe") as u32 + SHOW_ID_COUNTER_RVA
    }

    pub(crate) fn show_id_counter() -> u16 {
        SHOW_STORE.lock().unwrap().show_id_counter
    }

    pub(crate) fn set_show_id_counter(value: u16) {
        SHOW_STORE.lock().unwrap().show_id_counter = value;
    }

    pub(crate) fn call_real_save(this: *const u32, file: *const i8) -> bool {
        detours::call_real_save(this, file)
    }

    pub(crate) fn call_real_load(this: *const u32, file: *const u32, version: u32) -> bool {
        detours::call_real_load(this, file, version)
    }

    pub(crate) fn call_real_is_doing_show(this: *const u32, unit_id: u32, show_id: u16) -> bool {
        detours::call_real_is_doing_show(this, unit_id, show_id)
    }

    pub(crate) fn call_real_is_show_script_done(this: *const u32, script_id: u32, show_id: u16) -> u8 {
        detours::call_real_is_show_script_done(this, script_id, show_id)
    }

    pub(crate) fn call_real_register_show(this: *const u32, show: *const u32, force: bool) -> bool {
        detours::call_real_register_show(this, show, force)
    }

    pub(crate) fn call_real_unregister_show(this: *const u32, id: u16, show: *const u32, clear: bool) -> bool {
        detours::call_real_unregister_show(this, id, show, clear)
    }
}
