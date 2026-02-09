use std::{ffi::CString, path::Path, str, sync::Mutex};

use getset::CopyGetters;
use openzt_configparser::ini::{Ini, WriteOptions};
use std::sync::LazyLock;
use tracing::{debug, error, info};

use crate::{
    animation::Animation,
    resource_manager::{
        lazyresourcemap::{add_ztfile, get_file},
        ztfile::{ZTFile, ZTFileType},
    },
};

static RESOURCE_HANDLER_ARRAY: LazyLock<Mutex<Vec<Handler>>> = LazyLock::new(|| Mutex::new(Vec::new()));

pub fn add_handler(handler: Handler) {
    let mut data_mutex = RESOURCE_HANDLER_ARRAY.lock().unwrap();
    data_mutex.push(handler);
}

pub fn get_handlers() -> Vec<Handler> {
    RESOURCE_HANDLER_ARRAY.lock().unwrap().clone()
}

///Indicates when the handler should be called
/// BeforeOpenZTMods means they are run on any files found, but before any OpenZT mods are loaded (OpenZT mods can modify or add files)
/// AfterOpenZTMods is the same as above but after OpenZT mods are loaded
/// AfterFiltering is only run on files that are referenced in *.cfg files
#[derive(Clone, PartialEq, Copy)]
pub enum RunStage {
    BeforeOpenZTMods,
    AfterOpenZTMods,
    AfterFiltering,
}

pub type IniHandlerFunction = fn(&str, &str, Ini) -> Option<(String, String, Ini)>;
pub type AnimationHandlerFunction = fn(&str, &str, Animation) -> Option<(String, String, Animation)>;
pub type RawBytesHandlerFunction = fn(&str, &str, Box<[u8]>) -> Option<(String, String, Box<[u8]>)>;

///Each hander must have a stage and either an ini, animation or raw_bytes handler. Other fields are optional
/// This is enforced through a typestate pattern
pub struct HandlerBuilder<const HAS_STAGE: bool, const HAS_HANDLER: bool> {
    matcher_prefix: Option<String>,
    matcher_suffix: Option<String>,
    ini_handler: Option<IniHandlerFunction>,
    animation_handler: Option<AnimationHandlerFunction>,
    raw_bytes_handler: Option<RawBytesHandlerFunction>,
    stage: Option<RunStage>,
}

impl<const HAS_STAGE: bool, const HAS_HANDLER: bool> HandlerBuilder<HAS_STAGE, HAS_HANDLER> {
    pub fn prefix(mut self, prefix: &str) -> HandlerBuilder<HAS_STAGE, HAS_HANDLER> {
        self.matcher_prefix = Some(prefix.to_owned());
        self
    }

    pub fn suffix(mut self, suffix: &str) -> HandlerBuilder<HAS_STAGE, HAS_HANDLER> {
        self.matcher_suffix = Some(suffix.to_owned());
        self
    }

    pub fn ini_handler(self, handler: IniHandlerFunction) -> HandlerBuilder<HAS_STAGE, true> {
        HandlerBuilder {
            matcher_prefix: self.matcher_prefix,
            matcher_suffix: self.matcher_suffix,
            ini_handler: Some(handler),
            animation_handler: self.animation_handler,
            raw_bytes_handler: self.raw_bytes_handler,
            stage: self.stage,
        }
    }

    pub fn animation_handler(self, handler: AnimationHandlerFunction) -> HandlerBuilder<HAS_STAGE, true> {
        HandlerBuilder {
            matcher_prefix: self.matcher_prefix,
            matcher_suffix: self.matcher_suffix,
            ini_handler: self.ini_handler,
            animation_handler: Some(handler),
            raw_bytes_handler: self.raw_bytes_handler,
            stage: self.stage,
        }
    }

    pub fn raw_bytes_handler(self, handler: RawBytesHandlerFunction) -> HandlerBuilder<HAS_STAGE, true> {
        HandlerBuilder {
            matcher_prefix: self.matcher_prefix,
            matcher_suffix: self.matcher_suffix,
            ini_handler: self.ini_handler,
            animation_handler: self.animation_handler,
            raw_bytes_handler: Some(handler),
            stage: self.stage,
        }
    }

    pub fn run_stage(self, stage: RunStage) -> HandlerBuilder<true, HAS_HANDLER> {
        HandlerBuilder {
            matcher_prefix: self.matcher_prefix,
            matcher_suffix: self.matcher_suffix,
            ini_handler: self.ini_handler,
            animation_handler: self.animation_handler,
            raw_bytes_handler: self.raw_bytes_handler,
            stage: Some(stage),
        }
    }
}

impl HandlerBuilder<true, true> {
    pub fn build(self) -> Handler {
        unsafe {
            Handler {
                matcher_prefix: self.matcher_prefix,
                matcher_suffix: self.matcher_suffix,
                ini_handler: self.ini_handler,
                animation_handler: self.animation_handler,
                raw_bytes_handler: self.raw_bytes_handler,
                stage: self.stage.unwrap_unchecked(),
            }
        }
    }
}

#[derive(Clone, CopyGetters)]
pub struct Handler {
    matcher_prefix: Option<String>,
    matcher_suffix: Option<String>,
    ini_handler: Option<IniHandlerFunction>,
    animation_handler: Option<AnimationHandlerFunction>,
    raw_bytes_handler: Option<RawBytesHandlerFunction>,
    #[getset(get_copy = "pub")]
    stage: RunStage,
}

impl Handler {
    pub fn builder() -> HandlerBuilder<false, false> {
        HandlerBuilder::<false, false> {
            matcher_prefix: None,
            matcher_suffix: None,
            ini_handler: None,
            animation_handler: None,
            raw_bytes_handler: None,
            stage: None,
        }
    }

    // TODO: Return Result?
    pub fn handle(&self, file_name: &String) {
        if let Some(prefix) = &self.matcher_prefix && !file_name.starts_with(prefix) {
            return;
        }
        if let Some(suffix) = &self.matcher_suffix && !file_name.ends_with(suffix) {
            return;
        }

        let file_type = match ZTFileType::try_from(Path::new(&file_name)) {
            Ok(file_type) => file_type,
            Err(e) => {
                error!("Error getting file type: {} error: {}", file_name, e);
                return;
            }
        };

        let new_file = match file_type {
            ZTFileType::Ini
            | ZTFileType::Ai
            | ZTFileType::Ani
            | ZTFileType::Cfg
            | ZTFileType::Lyt
            | ZTFileType::Scn
            | ZTFileType::Uca
            | ZTFileType::Ucs
            | ZTFileType::Ucb => {
                if let Some(handler) = self.ini_handler {
                    let Some((archive_name, file)) = get_file(file_name) else {
                        error!("Error getting file: {}", file_name);
                        return;
                    };
                    debug!(
                        "Ini Handler {} {} is handling file: {} {}",
                        self.matcher_prefix.as_deref().unwrap_or_default(),
                        self.matcher_suffix.as_deref().unwrap_or_default(),
                        archive_name,
                        file_name
                    );
                    let mut ini = Ini::new_cs();
                    ini.set_comment_symbols(&[';', '#', ':']);

                    let input_string = crate::encoding_utils::decode_game_text(&file);
                    if let Err(e) = ini.read(input_string) {
                        error!("Error reading ini {}: {}", file_name, e);
                        return;
                    }
                    if let Some((new_archive_name, new_file_path, new_ini)) = handler(&archive_name, file_name, ini) {
                        let mut write_options = WriteOptions::default();
                        write_options.space_around_delimiters = true;
                        write_options.blank_lines_between_sections = 1;
                        let new_string = new_ini.pretty_writes(&write_options);
                        let new_string_length = new_string.len() as u32;

                        match CString::new(new_string) {
                            Ok(new_c_string) => Some((new_archive_name, new_file_path, ZTFile::Text(new_c_string, file_type, new_string_length))),
                            Err(_) => {
                                error!("Error converting ini to CString after modifying {}", file_name);
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ZTFileType::Animation => {
                if let Some(handler) = self.animation_handler {
                    info!(
                        "Animation Handler {} {} is handling file: {}",
                        self.matcher_prefix.as_deref().unwrap_or_default(),
                        self.matcher_suffix.as_deref().unwrap_or_default(),
                        file_name
                    );
                    let Some((archive_name, file)) = get_file(file_name) else {
                        error!("Error getting file: {}", file_name);
                        return;
                    };
                    let Ok(animation) = Animation::parse(&file) else {
                        error!("Error parsing animation: {}", file_name);
                        return;
                    };
                    if let Some((new_archive_name, new_file_path, new_animation)) = handler(&archive_name, file_name, animation) {
                        let Ok((new_animation_bytes, animation_size)) = new_animation.write() else {
                            error!("Error writing animation: {}", file_name);
                            return;
                        };
                        Some((
                            new_archive_name,
                            new_file_path,
                            ZTFile::RawBytes(new_animation_bytes.into_boxed_slice(), ZTFileType::Animation, animation_size as u32),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ZTFileType::Bmp | ZTFileType::Lle | ZTFileType::Tga | ZTFileType::Wav | ZTFileType::Palette => {
                if let Some(handler) = self.raw_bytes_handler {
                    info!(
                        "Raw Bytes Handler {} {} is handling file: {}",
                        self.matcher_prefix.as_deref().unwrap_or_default(),
                        self.matcher_suffix.as_deref().unwrap_or_default(),
                        file_name
                    );
                    let Some((archive_name, file)) = get_file(file_name) else {
                        error!("Error getting file: {}", file_name);
                        return;
                    };
                    if let Some((new_archive_name, new_file_path, new_data)) = handler(&archive_name, file_name, file) {
                        let new_data_len = new_data.len() as u32;
                        Some((new_archive_name, new_file_path, ZTFile::RawBytes(new_data, file_type, new_data_len)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            _ => return,
        };

        // TODO: Use "zip::./openzt.ztd" as archive?
        if let Some((new_archive_name, new_file_name, ztfile)) = new_file && let Err(e) = add_ztfile(Path::new(&new_archive_name), new_file_name, ztfile) {
            error!("Error adding file to archive: {}", e);
        }
    }
}
