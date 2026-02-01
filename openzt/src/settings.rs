use crate::command_console::CommandError;
use crate::lua_fn;
use openzt_detour_macro::detour_mod;
use tracing::error;

mod ai;
mod debug;
mod util;

fn command_get_setting(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() != 2 {
        return Err(format!("Got {} arguments, expected 2", args.len()).into());
    }
    if args[0].is_empty() || args[1].is_empty() {
        return Err("Empty arguments".into());
    }
    ai::get_settings()
        .iter()
        .chain(debug::get_settings().iter())
        .find_map(|setting| if setting.check(args[0], args[1]) { Some(setting.get()) } else { None })
        .ok_or(format!("Setting {} {} not found", args[0], args[1]).into())
}

fn command_set_setting(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() != 3 {
        return Err(format!("Got {} arguments, expected 3", args.len()).into());
    }
    if args[0].is_empty() || args[1].is_empty() || args[2].is_empty() {
        return Err("Empty arguments".into());
    }
    let Some(setting) = ai::get_settings()
        .into_iter()
        .chain(debug::get_settings())
        .find_map(|setting| if setting.check(args[0], args[1]) { Some(setting) } else { None })
    else {
        return Err(format!("Setting {} {} not found", args[0], args[1]).into());
    };

    setting.set(args[2])?;
    Ok("Success".to_string())
}

fn command_list_settings(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() > 1 {
        return Err(format!("Got {} arguments, expected at most 1", args.len()).into());
    }
    // TODO: This function needs to be implemented properly
    // For now, return all setting categories
    let categories = if args.is_empty() {
        "Available setting categories: AI, Debug"
    } else {
        "Settings list not yet implemented"
    };
    Ok(categories.to_string())
}

#[detour_mod]
mod zoo_ini_loading {
    use openzt_detour::gen::standalone::LOAD_INI_DEBUG_SETTINGS;
    use tracing::info;

    #[detour(LOAD_INI_DEBUG_SETTINGS)]
    // unsafe extern "cdecl" fn load_debug_settings_from_ini_detour() -> u32 {
    unsafe extern "stdcall" fn load_debug_settings_from_ini_detour() -> u32 {
        let result = unsafe { LOAD_INI_DEBUG_SETTINGS_DETOUR.call() };
        info!("######################LoadDebugSettingsFromIniHook: {}", result);
        result
    }
}

pub fn init() {
    // get_setting(section, key) - two string args
    lua_fn!("get_setting", "Gets a setting value", "get_setting(section, key)", |section: String, key: String| {
        match command_get_setting(vec![&section, &key]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // set_setting(section, key, value) - three string args
    lua_fn!(
        "set_setting",
        "Sets a setting value",
        "set_setting(section, key, value)",
        |section: String, key: String, value: String| {
            match command_set_setting(vec![&section, &key, &value]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // list_settings([category]) - optional string arg
    lua_fn!(
        "list_settings",
        "Lists available settings, optionally filtered by category",
        "list_settings([category])",
        |category: Option<String>| {
            match category {
                Some(cat) => match command_list_settings(vec![&cat]) {
                    Ok(result) => Ok((Some(result), None::<String>)),
                    Err(e) => Ok((None::<String>, Some(e.to_string()))),
                },
                None => match command_list_settings(vec![]) {
                    Ok(result) => Ok((Some(result), None::<String>)),
                    Err(e) => Ok((None::<String>, Some(e.to_string()))),
                },
            }
        }
    );

    if unsafe { zoo_ini_loading::init_detours() }.is_err() {
        error!("Error initialising load ini detours");
    };
}
