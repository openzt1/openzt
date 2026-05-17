use std::sync::{LazyLock, Mutex};

use openzt_configparser::ini::Ini;
use tracing::{info, warn};

use crate::util::get_ini_path;

static ZOO_INI_SETTINGS: LazyLock<Mutex<Ini>> = LazyLock::new(|| Mutex::new(new_zoo_ini()));

fn new_zoo_ini() -> Ini {
    let mut ini = Ini::new();
    ini.set_comment_symbols(&['#']);
    ini
}

fn load_from_disk() -> Result<Ini, String> {
    let path = get_ini_path();
    let mut ini = new_zoo_ini();
    ini.load(&path)?;
    info!("Loaded zoo.ini settings from {}", path.display());
    Ok(ini)
}

pub(crate) fn reload_zoo_ini_settings() -> Result<(), String> {
    let loaded = load_from_disk();
    let mut settings = ZOO_INI_SETTINGS.lock().unwrap();
    match loaded {
        Ok(ini) => {
            *settings = ini;
            Ok(())
        }
        Err(e) => {
            warn!("Failed to load zoo.ini settings: {}", e);
            *settings = new_zoo_ini();
            Err(e)
        }
    }
}

pub(crate) fn get_zoo_setting(section: &str, key: &str) -> Option<String> {
    ZOO_INI_SETTINGS.lock().unwrap().get(section, key)
}

pub(crate) fn get_zoo_setting_vec(section: &str, key: &str) -> Option<Vec<String>> {
    ZOO_INI_SETTINGS.lock().unwrap().get_vec(section, key)
}

pub(crate) fn get_zoo_setting_bool(section: &str, key: &str) -> Result<Option<bool>, String> {
    ZOO_INI_SETTINGS.lock().unwrap().get_bool_coerce(section, key)
}

pub(crate) fn get_zoo_setting_i32(section: &str, key: &str) -> Result<Option<i32>, String> {
    ZOO_INI_SETTINGS.lock().unwrap().get_parse(section, key)
}

pub(crate) fn zoo_setting_sections() -> Vec<String> {
    ZOO_INI_SETTINGS.lock().unwrap().sections()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_test_ini(input: &str) -> Ini {
        let mut ini = new_zoo_ini();
        ini.read(input.to_string()).unwrap();
        ini
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let ini = read_test_ini("[debug]\nlogCutoff=15\n");

        assert_eq!(ini.get("Debug", "logCutoff"), Some("15".to_string()));
        assert_eq!(ini.get("DEBUG", "LOGCUTOFF"), Some("15".to_string()));
    }

    #[test]
    fn reads_string_bool_and_i32_values() {
        let ini = read_test_ini("[user]\nfullscreen=0\nscreenwidth=1024\nlastfile=test.zoo\n");

        assert_eq!(ini.get("user", "lastfile"), Some("test.zoo".to_string()));
        assert_eq!(ini.get_bool_coerce("user", "fullscreen"), Ok(Some(false)));
        assert_eq!(ini.get_parse::<i32>("user", "screenwidth"), Ok(Some(1024)));
    }

    #[test]
    fn missing_values_return_none() {
        let ini = read_test_ini("[user]\nfullscreen=0\n");

        assert_eq!(ini.get("missing", "fullscreen"), None);
        assert_eq!(ini.get("user", "missing"), None);
        assert_eq!(ini.get_bool_coerce("user", "missing"), Ok(None));
        assert_eq!(ini.get_parse::<i32>("user", "missing"), Ok(None));
    }

    #[test]
    fn duplicate_keys_keep_all_values_for_vec_lookup() {
        let ini = read_test_ini("[resource]\npath=first\npath=second\n");

        assert_eq!(ini.get("resource", "path"), Some("second".to_string()));
        assert_eq!(
            ini.get_vec("resource", "path"),
            Some(vec!["first".to_string(), "second".to_string()])
        );
    }
}
