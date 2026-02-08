use openzt_detour_macro::detour_mod;
use tracing::info;

enum ZTLogLevel {
    Trace,
    Info,
    Note,
    Error,
    Fatal,
    BFLog,
}

impl ZTLogLevel {
    fn from_i32(value: i32) -> ZTLogLevel {
        match value {
            3 => ZTLogLevel::Trace,
            6 => ZTLogLevel::Info,
            9 => ZTLogLevel::Note,
            12 => ZTLogLevel::Error,
            15 => ZTLogLevel::Fatal,
            _ => ZTLogLevel::BFLog,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            ZTLogLevel::Trace => "TRACE",
            ZTLogLevel::Info => "INFO",
            ZTLogLevel::Note => "NOTE",
            ZTLogLevel::Error => "ERROR",
            ZTLogLevel::Fatal => "FATAL",
            ZTLogLevel::BFLog => "BFLOG",
        }
    }
}

pub fn log_from_zt(source_file: &String, line_number: i32, level: i32, message: &String) {
    let level = ZTLogLevel::from_i32(level);
    info!("{}({}) : {} : {}", source_file, line_number, level.as_str(), message);
}

#[detour_mod]
mod zoo_logging {
    use crate::{capture_ztlog::log_from_zt, util::get_string_from_memory};
    use openzt_detour::generated::bflog::LOG_MESSAGE;

    #[detour(LOG_MESSAGE)]
    unsafe extern "cdecl" fn zoo_log_func(source_file: u32, line_number: i32, level: i32, _param_4: i8, _param_5: i32, _param_6: i32, log_message: u32) {
        let source_file_string = get_string_from_memory(source_file);
        let log_message_string = get_string_from_memory(log_message);
        log_from_zt(&source_file_string, line_number, level, &log_message_string);
    }
}

pub fn init() {
    if let Err(e) = unsafe { zoo_logging::init_detours() } {
        info!("Error initialising zt logging detours: {}", e);
    };
}
