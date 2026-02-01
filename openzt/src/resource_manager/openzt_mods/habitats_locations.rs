use std::{collections::HashMap, ffi::CString, sync::Mutex};

use anyhow::Context;
use std::sync::LazyLock;
use tracing::info;

use crate::string_registry::add_string_to_registry;

/// Map between the id ZT uses to reference locations/habitats and the string ptr of the animation (icon) resource
pub static LOCATIONS_HABITATS_RESOURCE_MAP: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Map between the animation (icon resource) and the id ZT uses to reference location/habitats, this is used to lookup the id needed to add the habitat/location to an animal
pub static LOCATIONS_HABITATS_ID_MAP: LazyLock<Mutex<HashMap<String, u32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Track per-mod habitat registrations: mod_id → (habitat_name → string_id)
/// This enables cross-mod variable references in patches
pub static MOD_HABITATS_MAP: LazyLock<Mutex<HashMap<String, HashMap<String, u32>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Track per-mod location registrations: mod_id → (location_name → string_id)
/// This enables cross-mod variable references in patches
pub static MOD_LOCATIONS_MAP: LazyLock<Mutex<HashMap<String, HashMap<String, u32>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn add_location_or_habitat(mod_id: &str, name: &String, icon_resource_id: &String, is_habitat: bool) -> anyhow::Result<()> {
    let mut resource_binding = LOCATIONS_HABITATS_RESOURCE_MAP.lock().unwrap();

    let mut id_binding = LOCATIONS_HABITATS_ID_MAP.lock().unwrap();

    let string_id = add_string_to_registry(name.clone());

    info!(
        "Adding location/habitat: {} {} -> {} (mod: {}, is_habitat: {})",
        name, icon_resource_id, string_id, mod_id, is_habitat
    );

    let icon_resource_id_cstring = CString::new(icon_resource_id.clone())
        .with_context(|| format!("Failed to create cstring for location/habitat {} with icon_resource_id {}", name, icon_resource_id))?;
    resource_binding.insert(string_id, icon_resource_id_cstring.into_raw() as u32);
    id_binding.insert(name.clone(), string_id);

    // Track per-mod registration for variable substitution
    let mod_map = if is_habitat { &MOD_HABITATS_MAP } else { &MOD_LOCATIONS_MAP };

    let mut mod_map_binding = mod_map.lock().unwrap();
    mod_map_binding.entry(mod_id.to_string()).or_insert_with(HashMap::new).insert(name.clone(), string_id);

    Ok(())
}

pub fn get_location_or_habitat_by_id(id: u32) -> Option<u32> {
    let binding = LOCATIONS_HABITATS_RESOURCE_MAP.lock().unwrap();
    binding.get(&id).cloned()
}

pub fn get_location_or_habitat_by_name(name: &String) -> Option<u32> {
    let binding = LOCATIONS_HABITATS_ID_MAP.lock().unwrap();
    binding.get(name).cloned()
}

pub fn get_location_habitat_ids() -> Vec<u32> {
    let binding = LOCATIONS_HABITATS_RESOURCE_MAP.lock().unwrap();
    binding.keys().cloned().collect()
}

/// Get habitat string ID for a specific mod
/// Used for variable substitution in patches
pub fn get_habitat_id(mod_id: &str, habitat_name: &str) -> Option<u32> {
    let binding = MOD_HABITATS_MAP.lock().unwrap();
    binding.get(mod_id)?.get(habitat_name).cloned()
}

/// Get location string ID for a specific mod
/// Used for variable substitution in patches
pub fn get_location_id(mod_id: &str, location_name: &str) -> Option<u32> {
    let binding = MOD_LOCATIONS_MAP.lock().unwrap();
    binding.get(mod_id)?.get(location_name).cloned()
}
