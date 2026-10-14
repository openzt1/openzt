pub mod attendance;
pub mod lifecycle;
pub mod pending_scripts;
pub mod schedule;
pub mod units;

pub use attendance::*;
pub use lifecycle::*;
pub use pending_scripts::*;
pub use schedule::*;
pub use units::*;

use openzt_detour::generated::ztshowinfo::{
    ADD_SCRIPT, ADD_SHOW, ADD_UNIT, ADD_UNIT_TO_LIST, CHECK_PENDING_SCRIPTS, CHECK_UNIT, CHECK_UNIT_TYPE, CLEANUP_EVENTS,
    CREATE_DEFAULT_SCRIPT, ENTER_NEW_MONTH, GATHER_UNITS, GET_EVENTS, GET_NUM_UNITS, GET_SCHEDULED_SHOW_KEEPER_TYPE,
    GET_SCHEDULED_SHOW_SCRIPT, GET_SHOW_UNIT_LIST, HAS_KEEPER, INCREMENT_ATTENDANCE, INCREMENT_RECEIPTS, IS_READY, IS_STARTED,
    IS_STOPPED, LISTEN, LOAD, NEEDS_KEEPER, RECALCULATE_SCHEDULE, REMOVE_SHOW, REMOVE_UNIT, SAVE, SEND_EVENT, SET_SHOW_FREQUENCY,
    SET_SHOW_INFO_ID, UPDATE, UPDATE_FROM_LOAD,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_SCHEDULED_SHOW_SCRIPT)]
    unsafe extern "thiscall" fn get_scheduled_show_script_detour(this: *const u32) -> u32 {
        get_scheduled_show_script(this as u32) as u32
    }

    #[detour(IS_READY)]
    unsafe extern "thiscall" fn is_ready_detour(this: *const u32) -> u32 {
        is_ready(this as u32) as u32
    }

    #[detour(IS_STARTED)]
    unsafe extern "thiscall" fn is_started_detour(this: *const u32) -> u32 {
        is_started(this as u32) as u32
    }

    #[detour(IS_STOPPED)]
    unsafe extern "thiscall" fn is_stopped_detour(this: *const u32) -> u32 {
        is_stopped(this as u32) as u32
    }

    #[detour(HAS_KEEPER)]
    unsafe extern "thiscall" fn has_keeper_detour(this: *const u32) -> bool {
        has_keeper(this as u32)
    }

    #[detour(NEEDS_KEEPER)]
    unsafe extern "thiscall" fn needs_keeper_detour(this: *const u32, unit_type_id: u32) -> bool {
        needs_keeper(this as u32, unit_type_id)
    }

    #[detour(GET_SCHEDULED_SHOW_KEEPER_TYPE)]
    unsafe extern "thiscall" fn get_scheduled_show_keeper_type_detour(this: *const u32) -> u32 {
        get_scheduled_show_keeper_type(this as u32)
    }

    #[detour(INCREMENT_ATTENDANCE)]
    unsafe extern "thiscall" fn increment_attendance_detour(this: *const u32, amount: i32) {
        increment_attendance(this as u32, amount);
    }

    #[detour(INCREMENT_RECEIPTS)]
    unsafe extern "thiscall" fn increment_receipts_detour(this: *const u32, amount: f32) {
        increment_receipts(this as u32, amount);
    }

    #[detour(SET_SHOW_FREQUENCY)]
    unsafe extern "thiscall" fn set_show_frequency_detour(this: *const u32, frequency: i32) {
        set_show_frequency(this as u32, frequency);
    }

    #[detour(RECALCULATE_SCHEDULE)]
    unsafe extern "thiscall" fn recalculate_schedule_detour(this: *const u32, advance_slot: i8) {
        recalculate_schedule(this as u32, advance_slot != 0);
    }

    #[detour(ADD_SHOW)]
    unsafe extern "thiscall" fn add_show_detour(this: *const u32, unit_type_id: u32) {
        add_show(this as u32, unit_type_id);
    }

    #[detour(REMOVE_SHOW)]
    unsafe extern "thiscall" fn remove_show_detour(this: *const u32, unit_type_id: u32) {
        remove_show(this as u32, unit_type_id);
    }

    #[detour(CREATE_DEFAULT_SCRIPT)]
    unsafe extern "thiscall" fn create_default_script_detour(this: *const u32, unit_type_id: i32) -> *const u32 {
        create_default_script(this as u32, unit_type_id as u32) as *const u32
    }

    #[detour(GET_EVENTS)]
    unsafe extern "thiscall" fn get_events_detour(this: *const u32, arg: u32) {
        get_events(this as u32, arg);
    }

    #[detour(SEND_EVENT)]
    unsafe extern "thiscall" fn send_event_detour(
        this: *const u32,
        event_id: u16,
        unused: u32,
        category: u8,
        value: u32,
        value2: u16,
        flag: u16,
    ) {
        send_event(this as u32, event_id, unused, category, value, value2, flag);
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn listen_detour(this: *const u32) {
        listen(this as u32);
    }

    #[detour(CLEANUP_EVENTS)]
    unsafe extern "thiscall" fn cleanup_events_detour(this: *const u32) {
        cleanup_events(this as u32);
    }

    #[detour(GET_NUM_UNITS)]
    unsafe extern "thiscall" fn get_num_units_detour(this: *const u32, unit_type_id: u32) -> i32 {
        get_num_units(this as u32, unit_type_id)
    }

    #[detour(GET_SHOW_UNIT_LIST)]
    unsafe extern "thiscall" fn get_show_unit_list_detour(this: *const u32, unit_type_id: u32) -> i32 {
        get_show_unit_list(this as u32, unit_type_id) as i32
    }

    #[detour(CHECK_UNIT)]
    unsafe extern "thiscall" fn check_unit_detour(this: *const u32, unit_id: u32) -> bool {
        check_unit(this as u32, unit_id) & 0xff != 0
    }

    #[detour(REMOVE_UNIT)]
    unsafe extern "thiscall" fn remove_unit_detour(this: *const u32, unit_type_id: u32, unit_id_ptr: *const i32) {
        remove_unit(this as u32, unit_type_id, unit_id_ptr as u32);
    }

    #[detour(ADD_UNIT_TO_LIST)]
    unsafe extern "thiscall" fn add_unit_to_list_detour(this: *const u32, unit_ptr: i32) -> u32 {
        add_unit_to_list(this as u32, unit_ptr as u32) as u32
    }

    #[detour(ADD_UNIT)]
    unsafe extern "thiscall" fn add_unit_detour(this: *const u32, unit_ptr: i32) -> u32 {
        add_unit(this as u32, unit_ptr as u32) as u32
    }

    #[detour(GATHER_UNITS)]
    unsafe extern "thiscall" fn gather_units_detour(this: *const u32, unit_type_id: u32) -> bool {
        gather_units(this as u32, unit_type_id)
    }

    #[detour(ENTER_NEW_MONTH)]
    unsafe extern "thiscall" fn enter_new_month_detour(this: *const u32) {
        enter_new_month(this as u32);
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update_detour(this: *const u32) {
        update(this as u32);
    }

    #[detour(SET_SHOW_INFO_ID)]
    unsafe extern "thiscall" fn set_show_info_id_detour(this: *const u32, id: u16) -> bool {
        set_show_info_id(this as u32, id)
    }

    #[detour(UPDATE_FROM_LOAD)]
    unsafe extern "thiscall" fn update_from_load_detour(this: *const u32, source: *const u32) {
        update_from_load(this as u32, source as u32);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const i8) -> bool {
        show_info_save(this as u32, file)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load_detour(this: *const u32, file: *const u32, version: u32) -> bool {
        show_info_load(this as u32, file, version)
    }

    #[detour(CHECK_UNIT_TYPE)]
    unsafe extern "thiscall" fn check_unit_type_detour(this: *const u32, unit_type: u32) -> u32 {
        check_unit_type(this as u32, unit_type)
    }

    #[detour(CHECK_PENDING_SCRIPTS)]
    unsafe extern "thiscall" fn check_pending_scripts_detour(this: *const u32) {
        check_pending_scripts(this as u32);
    }

    #[detour(ADD_SCRIPT)]
    unsafe extern "thiscall" fn add_script_detour(this: *const u32, unit_type_id: u32, new_script_id: u16) -> bool {
        add_script(this as u32, unit_type_id, new_script_id)
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshowinfo detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }

    const ZTSHOWINFO_VTABLE_RVA: u32 = 0x0023_53cc;

    pub(crate) fn install_vtable_pointer(show_info: u32) {
        crate::util::save_to_memory(show_info, crate::globals::get_module_base("zoo.exe") as u32 + ZTSHOWINFO_VTABLE_RVA);
    }

    use openzt_detour::generated::standalone::{OPERATOR_DELETE, OPERATOR_NEW};
    use openzt_detour::generated::ztshowinfo::{CONSTRUCTOR_0, CONSTRUCTOR_1, DESTRUCTOR_1 as ZTSHOWINFO_DESTRUCTOR};

    const ZTSHOWINFO_SIZE: u32 = 0xa8;

    pub(crate) fn build_standalone_show_info_via_real_ctor() -> u32 {
        let buf = unsafe { OPERATOR_NEW.original()(ZTSHOWINFO_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(buf as *mut u8, 0, ZTSHOWINFO_SIZE as usize) };
        unsafe { CONSTRUCTOR_1.original()(buf as *const u32) };
        buf
    }

    pub(crate) fn build_standalone_show_info_copy_via_real_ctor(source: u32) -> u32 {
        let buf = unsafe { OPERATOR_NEW.original()(ZTSHOWINFO_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(buf as *mut u8, 0, ZTSHOWINFO_SIZE as usize) };
        unsafe { CONSTRUCTOR_0.original()(buf as *const u32, source as i32) };
        buf
    }

    pub(crate) fn destroy_standalone_show_info_via_real_dtor(buf: u32) {
        unsafe { ZTSHOWINFO_DESTRUCTOR.original()(buf as *const u32, 0) };
        unsafe { OPERATOR_DELETE.original()(buf) };
    }
}
