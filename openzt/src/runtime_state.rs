//! Global runtime state store for OpenZT
//!
//! This module provides a centralized way to store runtime state (booleans, strings)
//! that can be shared across modules. It uses two separate hashmaps for type safety.
//!
//! # Example
//!
//! ```rust
//! use openzt::runtime_state;
//!
//! // Store and retrieve boolean values
//! runtime_state::set_bool("my_feature_enabled", true);
//! let enabled = runtime_state::get_bool("my_feature_enabled"); // true
//!
//! // Toggle a boolean value
//! let new_state = runtime_state::toggle_bool("my_feature_enabled"); // false
//!
//! // Store and retrieve string values
//! runtime_state::set_string("last_file", "config.txt");
//! let file = runtime_state::get_string("last_file"); // "config.txt"
//!
//! // Check if a key exists
//! if runtime_state::has_bool("my_feature_enabled") {
//!     println!("Feature state is stored");
//! }
//! ```

use std::{collections::HashMap, sync::LazyLock};

/// Boolean state storage
static BOOL_STATE: LazyLock<std::sync::Mutex<HashMap<String, bool>>> = LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

/// String state storage
static STRING_STATE: LazyLock<std::sync::Mutex<HashMap<String, String>>> = LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

/// Get a boolean value, returns false if not found (clones value)
///
/// # Arguments
///
/// * `key` - The key to look up
///
/// # Returns
///
/// The boolean value if found, or false if the key doesn't exist
pub fn get_bool(key: &str) -> bool {
    BOOL_STATE.lock().unwrap().get(key).copied().unwrap_or(false)
}

/// Set a boolean value
///
/// # Arguments
///
/// * `key` - The key to store the value under
/// * `value` - The boolean value to store
pub fn set_bool(key: &str, value: bool) {
    BOOL_STATE.lock().unwrap().insert(key.to_string(), value);
}

/// Toggle a boolean value, returns new value
///
/// If the key doesn't exist, it will be created with value `true` (toggling from `false`).
///
/// # Arguments
///
/// * `key` - The key to toggle
///
/// # Returns
///
/// The new boolean value after toggling
pub fn toggle_bool(key: &str) -> bool {
    let mut state = BOOL_STATE.lock().unwrap();
    let current = state.get(key).copied().unwrap_or(false);
    let new_value = !current;
    state.insert(key.to_string(), new_value);
    new_value
}

/// Get a string value, returns empty string if not found (clones value)
///
/// # Arguments
///
/// * `key` - The key to look up
///
/// # Returns
///
/// The string value if found, or an empty string if the key doesn't exist
pub fn get_string(key: &str) -> String {
    STRING_STATE.lock().unwrap().get(key).cloned().unwrap_or_default()
}

/// Set a string value
///
/// # Arguments
///
/// * `key` - The key to store the value under
/// * `value` - The string value to store
pub fn set_string(key: &str, value: String) {
    STRING_STATE.lock().unwrap().insert(key.to_string(), value);
}

/// Check if a key exists in boolean storage
///
/// # Arguments
///
/// * `key` - The key to check
///
/// # Returns
///
/// `true` if the key exists, `false` otherwise
pub fn has_bool(key: &str) -> bool {
    BOOL_STATE.lock().unwrap().contains_key(key)
}

/// Check if a key exists in string storage
///
/// # Arguments
///
/// * `key` - The key to check
///
/// # Returns
///
/// `true` if the key exists, `false` otherwise
pub fn has_string(key: &str) -> bool {
    STRING_STATE.lock().unwrap().contains_key(key)
}
