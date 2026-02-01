use crate::{
    command_console::CommandError,
    lua_fn,
    resource_manager::{
        bfresourcemgr::{read_bf_resource_dir_contents_from_memory, read_bf_resource_mgr_from_memory},
        lazyresourcemap::{decrement_ref, get_cache_stats, get_file_names, get_ref_count, increment_ref, unload_all_resources, UnloadResult},
        openzt_mods::{get_location_habitat_ids, get_mod_ids},
    },
    string_registry::get_string_from_registry,
    util::ZTString,
};

pub fn init_commands() {
    // list_resources() - no args
    lua_fn!("list_resources", "Lists all BF resource directories and files", "list_resources()", || {
        match command_list_resources(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // get_bfresourcemgr() - no args
    lua_fn!("get_bfresourcemgr", "Returns BF resource manager details", "get_bfresourcemgr()", || {
        match command_get_bf_resource_mgr(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // list_resource_strings([prefix]) - optional string arg
    lua_fn!(
        "list_resource_strings",
        "Lists resource strings, optionally filtered by prefix",
        "list_resource_strings([prefix])",
        |prefix: Option<String>| {
            match prefix {
                Some(p) => match command_list_resource_strings(vec![&p]) {
                    Ok(result) => Ok((Some(result), None::<String>)),
                    Err(e) => Ok((None::<String>, Some(e.to_string()))),
                },
                None => match command_list_resource_strings(vec![]) {
                    Ok(result) => Ok((Some(result), None::<String>)),
                    Err(e) => Ok((None::<String>, Some(e.to_string()))),
                },
            }
        }
    );

    // list_openzt_resource_strings() - no args
    lua_fn!(
        "list_openzt_resource_strings",
        "Lists all OpenZT resource strings",
        "list_openzt_resource_strings()",
        || {
            match command_list_openzt_resource_strings(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // list_openzt_mods() - no args
    lua_fn!("list_openzt_mods", "Lists all OpenZT mod IDs", "list_openzt_mods()", || {
        match command_list_openzt_mod_ids(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // list_openzt_locations_habitats() - no args
    lua_fn!(
        "list_openzt_locations_habitats",
        "Lists all OpenZT location and habitat IDs",
        "list_openzt_locations_habitats()",
        || {
            match command_list_openzt_locations_habitats(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // unload_resources() - no args
    lua_fn!("unload_resources", "Unload all loaded resources to free memory", "unload_resources()", || {
        let UnloadResult { count, total_size } = unload_all_resources();
        Ok((
            Some(format!(
                "Unloaded {} resources (freed {} bytes, {} MB)",
                count,
                total_size,
                total_size / (1024 * 1024)
            )),
            None::<String>,
        ))
    });

    // cache_stats() - no args
    lua_fn!("cache_stats", "Show resource cache statistics", "cache_stats()", || {
        let stats = get_cache_stats();
        Ok((
            Some(format!(
                "Loaded: {} resources\nMemory: {} MB ({} bytes)",
                stats.loaded_resources, stats.total_memory_mb, stats.total_memory_bytes
            )),
            None::<String>,
        ))
    });

    // increment_ref(file_name) - string arg
    lua_fn!(
        "increment_ref",
        "Increment reference count for a resource",
        "increment_ref(file_name)",
        |file_name: String| {
            if increment_ref(&file_name) {
                Ok((Some(format!("Incremented ref count for: {}", file_name)), None::<String>))
            } else {
                Ok((None::<String>, Some(format!("Resource not found: {}", file_name))))
            }
        }
    );

    // decrement_ref(file_name) - string arg
    lua_fn!(
        "decrement_ref",
        "Decrement reference count for a resource",
        "decrement_ref(file_name)",
        |file_name: String| {
            match decrement_ref(&file_name) {
                Some(new_count) => Ok((Some(format!("Decremented ref count for: {} (new count: {})", file_name, new_count)), None::<String>)),
                None => Ok((None::<String>, Some(format!("Resource not found: {}", file_name)))),
            }
        }
    );

    // get_ref_count(file_name) - string arg
    lua_fn!(
        "get_ref_count",
        "Get reference count for a resource",
        "get_ref_count(file_name)",
        |file_name: String| {
            match get_ref_count(&file_name) {
                Some(count) => Ok((Some(format!("Ref count for {}: {}", file_name, count)), None::<String>)),
                None => Ok((None::<String>, Some(format!("Resource not found: {}", file_name)))),
            }
        }
    );
}

fn command_list_resource_strings(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() > 1 {
        return Err(CommandError::new("Too many arguments".to_string()));
    }
    let mut result_string = String::new();
    for resource_string in get_file_names() {
        if args.len() == 1 && !resource_string.starts_with(args[0]) {
            continue;
        }
        result_string.push_str(&format!("{}\n", resource_string));
    }
    Ok(result_string)
}

fn command_list_openzt_resource_strings(_args: Vec<&str>) -> Result<String, CommandError> {
    let mut result_string = String::new();
    for resource_string in get_file_names() {
        if resource_string.starts_with("openzt") {
            result_string.push_str(&format!("{}\n", resource_string));
        }
    }
    Ok(result_string)
}

fn command_list_resources(_args: Vec<&str>) -> Result<String, CommandError> {
    let mut result_string = String::new();
    let bf_resource_dir_contents = read_bf_resource_dir_contents_from_memory();
    for bf_resource_dir_content in bf_resource_dir_contents {
        let bf_resource_dir = bf_resource_dir_content.dir;
        result_string.push_str(&format!("{} ({})\n", bf_resource_dir.dir_name.copy_to_string(), bf_resource_dir.num_child_files,));
        let bf_resource_zips = bf_resource_dir_content.zips;
        for bf_resource_zip in bf_resource_zips {
            result_string.push_str(&format!("{}\n", bf_resource_zip.zip_name.copy_to_string()));
        }
    }
    Ok(result_string)
}

fn command_get_bf_resource_mgr(_args: Vec<&str>) -> Result<String, CommandError> {
    let bf_resource_mgr = read_bf_resource_mgr_from_memory();
    Ok(format!("{}", bf_resource_mgr))
}

fn command_list_openzt_mod_ids(_args: Vec<&str>) -> Result<String, CommandError> {
    let mut result_string = String::new();
    for mod_id in get_mod_ids() {
        result_string.push_str(&format!("{}\n", mod_id));
    }
    Ok(result_string)
}

fn command_list_openzt_locations_habitats(_args: Vec<&str>) -> Result<String, CommandError> {
    let mut result_string = String::new();
    for id in get_location_habitat_ids() {
        let name = get_string_from_registry(id).unwrap_or("<error>".to_string());
        result_string.push_str(&format!("{} {}\n", id, name));
    }
    Ok(result_string)
}
