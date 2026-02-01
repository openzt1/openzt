//! ZTD load order registry for tracking ZTD archive loading order.
//!
//! This module provides a global registry to track the order in which ZTD archives
//! are loaded, enabling the `ztd_loaded` patch condition to check if a ZTD was
//! loaded earlier in the load order.

use std::sync::LazyLock;
use std::{collections::HashMap, sync::Mutex};

/// Load status of a ZTD archive
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZtdLoadStatus {
    /// ZTD is enabled and loaded
    Enabled,
    /// ZTD is disabled via mod_loading.disabled
    Disabled,
}

/// Global registry tracking ZTD load order
/// Maps lowercase ZTD filename -> (load_position, status)
static ZTD_LOAD_ORDER: LazyLock<Mutex<HashMap<String, (usize, ZtdLoadStatus)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Current load position (increments for each loaded ZTD)
static CURRENT_LOAD_POSITION: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// Map mod_id -> ZTD filename (lowercase)
static MOD_TO_ZTD: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register a ZTD archive as loaded with its status
///
/// # Arguments
/// * `ztd_filename` - The ZTD filename (e.g., "basemod.ztd")
/// * `status` - Whether the ZTD is enabled or disabled
pub fn register_ztd(ztd_filename: &str, status: ZtdLoadStatus) {
    let lowercase = ztd_filename.to_lowercase();
    let mut position = CURRENT_LOAD_POSITION.lock().unwrap();
    let mut registry = ZTD_LOAD_ORDER.lock().unwrap();

    registry.insert(lowercase, (*position, status));
    *position += 1;
}

/// Register the ZTD filename for a mod_id
///
/// # Arguments
/// * `mod_id` - The mod identifier (e.g., "mymod")
/// * `ztd_filename` - The ZTD filename associated with this mod
pub fn register_mod_ztd(mod_id: &str, ztd_filename: &str) {
    let lowercase = ztd_filename.to_lowercase();
    let mut map = MOD_TO_ZTD.lock().unwrap();
    map.insert(mod_id.to_string(), lowercase);
}

/// Get the ZTD filename for a mod_id
///
/// # Arguments
/// * `mod_id` - The mod identifier
///
/// # Returns
/// * `Some(filename)` if the mod has a registered ZTD
/// * `None` if the mod has no registered ZTD
pub fn get_mod_ztd(mod_id: &str) -> Option<String> {
    let map = MOD_TO_ZTD.lock().unwrap();
    map.get(mod_id).cloned()
}

/// Get the load position of a ZTD
///
/// # Arguments
/// * `ztd_filename` - The ZTD filename
///
/// # Returns
/// * `Some(position)` if the ZTD is registered
/// * `None` if the ZTD is not registered
pub fn get_ztd_position(ztd_filename: &str) -> Option<usize> {
    let lowercase = ztd_filename.to_lowercase();
    let registry = ZTD_LOAD_ORDER.lock().unwrap();
    registry.get(&lowercase).map(|(pos, _)| *pos)
}

/// Check if a ZTD was loaded (enabled) before a given position
///
/// # Arguments
/// * `ztd_filename` - The ZTD filename to check
/// * `current_position` - The current load position to compare against
///
/// # Returns
/// * `true` if the ZTD was enabled and loaded before the given position
/// * `false` otherwise
pub fn is_ztd_loaded_before(ztd_filename: &str, current_position: usize) -> bool {
    let lowercase = ztd_filename.to_lowercase();
    let registry = ZTD_LOAD_ORDER.lock().unwrap();

    registry
        .get(&lowercase)
        .map(|&(pos, status)| status == ZtdLoadStatus::Enabled && pos < current_position)
        .unwrap_or(false)
}

#[cfg(test)]
pub fn clear_registry() {
    ZTD_LOAD_ORDER.lock().unwrap().clear();
    *CURRENT_LOAD_POSITION.lock().unwrap() = 0;
    MOD_TO_ZTD.lock().unwrap().clear();
}

#[cfg(feature = "integration-tests")]
pub fn clear_registry_for_tests() {
    ZTD_LOAD_ORDER.lock().unwrap().clear();
    *CURRENT_LOAD_POSITION.lock().unwrap() = 0;
    MOD_TO_ZTD.lock().unwrap().clear();
}
