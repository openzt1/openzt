use super::util::{Address, GettableSettable, Setting};

pub const EXE_LOCATION_ADDRESS: u32 = 0x0064BEDC;
pub const EXE_LOCATION_ADDRESS_2: u32 = 0x0064BED8;
pub const EXE_LOCATION_ADDRESS_3: u32 = 0x0064A800;

pub fn get_settings() -> Vec<Box<dyn GettableSettable>> {
    vec![
        Box::new(LOG_CUTTOFF),
        Box::new(SEND_DEBUGGER),
        Box::new(SEND_LOG_FILE),
        Box::new(SEND_MESSAGE_BOX),
        Box::new(DELTA_LOG_0),
        Box::new(DELTA_LOG_1),
    ]
}

const LOG_CUTTOFF: Setting<i32> = Setting {
    header: "Debug",
    key: "logCutoff",
    address: Address::Global(0x0063804c),
    default: 9,
};
const SEND_DEBUGGER: Setting<bool> = Setting {
    header: "Debug",
    key: "sendDebugger",
    address: Address::Global(0x00643e44),
    default: false,
};
const SEND_LOG_FILE: Setting<bool> = Setting {
    header: "Debug",
    key: "sendLogFile",
    address: Address::Global(0x00643e48),
    default: false,
};
const SEND_MESSAGE_BOX: Setting<bool> = Setting {
    header: "Debug",
    key: "sendMessageBox",
    address: Address::Global(0x00643e4a),
    default: false,
};
const DELTA_LOG_0: Setting<bool> = Setting {
    header: "Debug",
    key: "deltaLog0",
    address: Address::Global(0x00638054),
    default: false,
};
const DELTA_LOG_1: Setting<bool> = Setting {
    header: "Debug",
    key: "deltaLog1",
    address: Address::Global(0x0064bd7c),
    default: false,
};

//TODO: Find these debug settings? Technically they are in ZTApp
// drawfps=1           // ZTApp + 0x509  // 004bc707
// drawfpsx=720        // ZTApp + 0x50c  // 004bc7b0
// drawfpsy=20         // ZTApp + 0x510  // 004bc854

// #[derive(Debug)]
// pub struct DebugSettings {
//     pub log_cutoff: i32,
//     pub send_log_file: i32,
//     pub send_message_box: i32,
//     pub send_debugger: i32,
//     pub delta_log_1: i32,
//     pub delta_log_0: i32,
// }

// pub fn load_debug_settings(ini_path: &Path) -> DebugSettings {
//     let debug_settings = DebugSettings {
//         log_cutoff: 9,
//         send_log_file: 0,
//         send_message_box: 0,
//         send_debugger: 0,
//         delta_log_0: 0,
//         delta_log_1: 0,
//     };
//     load_debug_settings_from_ini(debug_settings, ini_path)
// }

// fn load_debug_settings_from_ini(mut debug_settings: DebugSettings, ini_path: &Path) -> DebugSettings {
//     let mut zoo_ini = Ini::new();

//     zoo_ini.load(ini_path).unwrap();

//     let debug_header = "Debug";

//     debug_settings.log_cutoff = value_or_default(zoo_ini.get_parse(debug_header, "LogCutoff"), debug_settings.log_cutoff);

//     debug_settings.send_log_file = value_or_default(zoo_ini.get_parse(debug_header, "SendLogfile"), debug_settings.send_log_file);

//     debug_settings.send_message_box = value_or_default(zoo_ini.get_parse(debug_header, "sendMessageBox"), debug_settings.send_message_box);

//     debug_settings.send_debugger = value_or_default(zoo_ini.get_parse(debug_header, "sendDebugger"), debug_settings.send_debugger);

//     debug_settings.delta_log_0 = value_or_default(zoo_ini.get_parse(debug_header, "deltaLog0"), debug_settings.delta_log_0);

//     debug_settings.delta_log_1 = value_or_default(zoo_ini.get_parse(debug_header, "deltaLog1"), debug_settings.delta_log_1);

//     debug_settings
// }

// fn load_debug_settings_from_zoo_ini() {
//     util::debug_logger("load_debug_settings_from_ini");
//     util::log_exe_location_memory_value();
//     util::log_debug_ini_memory_values();
//     let mut base_path = util::get_base_path();
//     base_path.push("zoo.ini");
//     let debug_settings = load_ini::load_debug_settings(base_path.as_path());
//     util::debug_logger("Saving debug ini settings");
//     util::save_debug_settings(debug_settings);
//     util::log_debug_ini_memory_values();
// }
