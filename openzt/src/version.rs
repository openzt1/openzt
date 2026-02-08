use openzt_detour_macro::detour_mod;
use tracing::error;

pub fn init() {
    if unsafe { bf_version_info::init_detours() }.is_err() {
        error!("Failed to initialize bf_version_info detours");
    }
}

#[detour_mod]
mod bf_version_info {
    use crate::util::{get_from_memory, get_string_from_memory, save_string_to_memory, save_to_memory};
    use openzt_detour::generated::bfversioninfo::GET_VERSION_STRING;

    #[detour(GET_VERSION_STRING)]
    unsafe extern "cdecl" fn bf_version_info_get_version_string_hook(param_1: u32, param_2: u32, param_3: u32) -> u32 {
        let return_value = unsafe { GET_VERSION_STRING_DETOUR.call(param_1, param_2, param_3) };
        let version_string = get_string_from_memory(get_from_memory::<u32>(param_2));
        let version_length = version_string.len();
        let full_openzt_version_string = format!(" OpenZT: {}", env!("CARGO_PKG_VERSION"));
        save_string_to_memory(get_from_memory::<u32>(param_2) + version_length as u32, &full_openzt_version_string);
        save_to_memory(param_3, (version_length + full_openzt_version_string.len() + 2) as u32);
        return_value
    }
}
