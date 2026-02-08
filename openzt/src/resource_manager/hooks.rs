use openzt_detour_macro::detour_mod;
use tracing::error;

pub fn init_hooks() {
    if unsafe { zoo_resource_mgr::init_detours() }.is_err() {
        error!("Error initialising custom expansion detours");
    };
}

#[detour_mod]
mod zoo_resource_mgr {
    use std::collections::HashMap;
    use std::ffi::CString;
    use std::time::Instant;

    use tracing::{debug, error, info, warn};

    use openzt_configparser::ini::Ini;
    use openzt_detour::generated::bfresource::{ATTEMPT, PREPARE};
    use openzt_detour::generated::bfresourcemgr::{ADD_PATH, CONSTRUCTOR};
    use openzt_detour::generated::bfresourceptr::{
        DELREF_0, DELREF_1, DELREF_10, DELREF_11, DELREF_12, DELREF_13, DELREF_14, DELREF_15, DELREF_16, DELREF_17, DELREF_18, DELREF_19, DELREF_2, DELREF_20,
        DELREF_21, DELREF_22, DELREF_23, DELREF_24, DELREF_25, DELREF_26, DELREF_27, DELREF_28, DELREF_29, DELREF_3, DELREF_4, DELREF_5, DELREF_6, DELREF_7, DELREF_8,
        DELREF_9,
    };
    use openzt_detour::generated::ztui_general::GET_INFO_IMAGE_NAME;

    use crate::{
        mods,
        resource_manager::{
            bfresourcemgr::BFResourcePtr,
            dependency_resolver::DependencyResolver,
            lazyresourcemap::{check_file, deref_resource, get_file_ptr, is_disabled_ztd_file},
            legacy_loading::{load_resources, OPENZT_DIR0},
            mod_config::{get_openzt_config, save_openzt_config},
            openzt_mods::{discover_mods, get_location_or_habitat_by_id},
            validation::{log_validation_result, validate_load_order},
        },
        util::{get_ini_path, get_string_from_memory, save_to_memory},
    };

    ///When Zoo Tycoon tries to load a resource we check if it's a resource we've already loaded and return that instead
    #[detour(ATTEMPT)]
    unsafe extern "thiscall" fn zoo_bf_resource_attempt(this_ptr: u32, file_name: u32) -> bool {
        if bf_resource_inner(this_ptr, file_name) {
            return true;
        }
        unsafe { ATTEMPT_DETOUR.call(this_ptr, file_name) }
    }

    ///When Zoo Tycoon tries to load a resource we check if it's a resource we've already loaded and return that instead
    #[detour(PREPARE)]
    unsafe extern "thiscall" fn zoo_bf_resource_prepare(this_ptr: u32, file_name: u32) -> bool {
        if bf_resource_inner(this_ptr, file_name) {
            return true;
        }

        unsafe { PREPARE_DETOUR.call(this_ptr, file_name) }
    }

    fn bf_resource_inner(this_ptr: u32, file_name: u32) -> bool {
        let mut file_name_string = get_string_from_memory(file_name).to_lowercase();
        if file_name_string.starts_with(OPENZT_DIR0) {
            match parse_openzt_resource_string(file_name_string.clone()) {
                Ok(resource_name) => {
                    file_name_string = resource_name;
                }
                Err(e) => {
                    info!("Failed to parse openzt resource string: {} {}", file_name_string, e);
                    return false;
                }
            }
        }
        if !check_file(&file_name_string) {
            // Check if this is a file from a disabled ZTD
            if is_disabled_ztd_file(&file_name_string) {
                error!(
                    "Vanilla game is loading file '{}' from a disabled ZTD! \
                     The file has an unsupported type (not .cfg/.uca/.ucb/.ucs) \
                     and could not be disabled. This indicates a configuration mistake.",
                    file_name_string
                );
            }
            return false;
        }

        if let Some(ptr) = get_file_ptr(&file_name_string) {
            let mut bfrp = unsafe { Box::from_raw(ptr as *mut BFResourcePtr) };

            if bfrp.num_refs < 100 {
                debug!("Resource '{}' refs have been lost {}", file_name_string, 100 - bfrp.num_refs);
                bfrp.num_refs = 100;
            }

            let ptr = Box::into_raw(bfrp) as u32;

            save_to_memory(this_ptr, ptr);
            true
        } else {
            false
        }
    }

    fn parse_openzt_resource_string(file_name: String) -> Result<String, &'static str> {
        if file_name.starts_with(OPENZT_DIR0) {
            let split = file_name.split('/').collect::<Vec<&str>>();
            if split.len() == 2 || split.len() == 3 {
                return Ok(split[1].to_owned());
            }
        }
        Err("Invalid openzt resource string")
    }

    /// Parse disabled entries into mod IDs and ZTD filenames
    ///
    /// # Arguments
    /// * `disabled` - Array of disabled entries from config
    ///
    /// # Returns
    /// * `(mod_ids, ztd_files)` - Tuple of vectors containing mod IDs and ZTD filenames respectively
    fn parse_disabled_entries(disabled: &[String]) -> (Vec<String>, Vec<String>) {
        let mut mod_ids = Vec::new();
        let mut ztd_files = Vec::new();

        for entry in disabled {
            if entry.to_lowercase().ends_with(".ztd") {
                ztd_files.push(entry.clone());
            } else {
                mod_ids.push(entry.clone());
            }
        }

        (mod_ids, ztd_files)
    }

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn zoo_bf_resource_mgr_constructor(this_ptr: u32) -> u32 {
        info!("BFResourceMgr::constructor({:X})", this_ptr);

        let now = Instant::now();

        let return_value = unsafe { CONSTRUCTOR_DETOUR.call(this_ptr) };

        let elapsed = now.elapsed();
        info!("Vanilla loading took {:.2?}", elapsed);

        let ini_path = get_ini_path();
        let mut zoo_ini = Ini::new();
        zoo_ini.set_comment_symbols(&['#']);
        if let Err(e) = zoo_ini.load(ini_path) {
            info!("Failed to load zoo.ini: {}", e);
            return return_value;
        };

        if let Some(paths) = zoo_ini.get("resource", "path") {
            let mut paths: Vec<String> = paths.split(';').map(|s| s.to_owned()).collect();

            if !paths.iter().any(|s| s.trim() == "./mods") {
                info!("Adding mods directory to BFResourceMgr");

                if let Ok(mods_path) = CString::new("./mods") {
                    unsafe { ADD_PATH.original()(this_ptr, mods_path.as_ptr() as u32); }
                }

                paths.insert(0, "./mods".to_owned());
            }

            info!("Loading resources from: {:?}", paths);

            // Load OpenZT configuration
            let mut config = get_openzt_config();

            // Discover all mods and pure legacy archives
            debug!("Discovering mods...");
            let discovery_result = discover_mods(&paths);
            debug!("Discovered {} OpenZT mod(s)", discovery_result.openzt_mods.len());
            debug!("Discovered {} pure legacy archive(s) in /mods/", discovery_result.pure_legacy_in_mods.len());

            // Parse disabled entries into mod IDs and ZTD filenames
            let (disabled_mods, disabled_ztds) = parse_disabled_entries(&config.mod_loading.disabled);

            if !disabled_ztds.is_empty() {
                info!("Disabled ZTD files: {:?}", disabled_ztds);
            }

            // Resolve dependencies and determine load order
            // Extract just the Meta structs for the resolver (convert from tuple)
            let resolver_mods: HashMap<String, mods::Meta> = discovery_result.openzt_mods.iter().map(|(id, (_, meta))| (id.clone(), meta.clone())).collect();
            let resolver = DependencyResolver::new(resolver_mods.clone(), &discovery_result.openzt_mods);
            let resolution_result = resolver.resolve_order(&config.mod_loading.order, &disabled_mods, &discovery_result.pure_legacy_in_mods);

            // Log any dependency resolution warnings
            for warning in &resolution_result.warnings {
                use crate::resource_manager::dependency_resolver::ResolutionWarning;
                match warning {
                    ResolutionWarning::CircularDependency { cycle } => {
                        warn!("Circular dependency detected (with optional deps): {:?}", cycle);
                    }
                    ResolutionWarning::TrulyCyclicDependency { cycle } => {
                        error!("Truly cyclic dependency detected (required deps only): {:?}", cycle);
                        error!("  These mods will be loaded at the end of the order");
                    }
                    ResolutionWarning::FormerlyCyclicDependency { mod_id, reason } => {
                        info!("Mod '{}' had cycle resolved: {}", mod_id, reason);
                    }
                    ResolutionWarning::MissingOptionalDependency { mod_id, missing } => {
                        debug!("Mod '{}' has optional dependency '{}' which is not present", mod_id, missing);
                    }
                    ResolutionWarning::MissingRequiredDependency { mod_id, missing } => {
                        warn!("Mod '{}' requires '{}' which is not present", mod_id, missing);
                    }
                    ResolutionWarning::ConflictingConstraints { mod_id, details } => {
                        warn!("Mod '{}' has conflicting constraints: {}", mod_id, details);
                    }
                }
            }

            // Validate load order if configured
            if config.mod_loading.warn_on_conflicts {
                let validation_result = validate_load_order(&resolution_result.order, &resolver_mods, &discovery_result.pure_legacy_in_mods);
                log_validation_result(&validation_result);
            }

            // Check if we need to update openzt.toml
            let needs_update = resolution_result.order != config.mod_loading.order;
            if needs_update {
                info!("Load order changed, updating openzt.toml");
                config.mod_loading.order = resolution_result.order.clone();
                if let Err(e) = save_openzt_config(&config, false) {
                    info!("WARNING: Failed to save openzt.toml: {}", e);
                }
            }

            // Filter out disabled mods for actual loading
            // (they remain in openzt.toml order but are not loaded)
            let disabled_set: std::collections::HashSet<_> = disabled_mods.iter().collect();
            let enabled_order: Vec<String> = resolution_result.order.iter().filter(|mod_id| !disabled_set.contains(mod_id)).cloned().collect();

            if !disabled_mods.is_empty() {
                info!("Disabled OpenZT mods (not loading): {:?}", disabled_mods);
            }

            // Load resources in resolved order (excluding disabled mods, with disabled ZTD info)
            load_resources(
                paths,
                &enabled_order,
                &discovery_result.openzt_mods,
                &disabled_mods,
                &disabled_ztds,
                &discovery_result.pure_legacy_in_mods,
            );
            info!("Resources loaded");
        }
        return_value
    }

    #[detour(GET_INFO_IMAGE_NAME)]
    unsafe extern "cdecl" fn zoo_ui_general_get_info_image_name(id: i32) -> u32 {
        match get_location_or_habitat_by_id(id as u32) {
            Some(resource_ptr) => resource_ptr,
            None => unsafe { GET_INFO_IMAGE_NAME_DETOUR.call(id) },
        }
    }

    /// Helper function to handle BFResourcePtr delref operations
    /// Extracts the filename from the BFResourcePtr and calls deref_resource
    fn handle_delref(this_ptr: u32) {
        use crate::util::{get_from_memory, ZTString};

        // Read the BFResourcePtr from memory
        let bf_resource_ptr = get_from_memory::<BFResourcePtr>(this_ptr);

        // Get the filename from the resource
        let filename = bf_resource_ptr.bf_resource_name.copy_to_string();

        // Call deref_resource to decrement our ref count
        deref_resource(&filename);
    }

    // BFResourcePtr::delref detours (30 different call sites in the game)
    #[detour(DELREF_0)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_0(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_1)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_1(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_2)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_2(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_3)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_3(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_4)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_4(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_5)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_5(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_6)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_6(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_7)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_7(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_8)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_8(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_9)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_9(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_10)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_10(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_11)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_11(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_12)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_12(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_13)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_13(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_14)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_14(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_15)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_15(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_16)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_16(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_17)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_17(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_18)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_18(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_19)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_19(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_20)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_20(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_21)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_21(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_22)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_22(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_23)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_23(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_24)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_24(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_25)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_25(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_26)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_26(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_27)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_27(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_28)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_28(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    #[detour(DELREF_29)]
    unsafe extern "thiscall" fn zoo_bf_resource_ptr_delref_29(this_ptr: u32) {
        handle_delref(this_ptr);
    }

    // #[detour(DIR_SEARCH)]
    // unsafe extern "cdecl" fn zoo_standalone_dir_search(dir_name: u32) -> u32 {
    //     let result = unsafe { DIR_SEARCH_DETOUR.call(dir_name) };
    //     info!("Standalone::dir_search({}) -> {:X}", get_string_from_memory(get_from_memory(dir_name)), result);
    //     info!("{:x} {:x} {:x}", get_from_memory::<u32>(result), get_from_memory::<u32>(result + 4), get_from_memory::<u32>(result + 8));
    //     info!("{}", get_string_from_memory(get_from_memory(result)));
    //     let mut start = get_from_memory::<u32>(result);
    //     let end = get_from_memory::<u32>(result + 4);
    //     while start < end {
    //         let dir_string = get_string_from_memory(start);
    //         info!("  Found dir: {}", dir_string);
    //         start += 0x108;
    //     }

    //     result
    // }

    // struct SparseFileInfo {
    //     file_name_ptr: u32,
    //     padding: [u8; 0x104],
    // }
}
