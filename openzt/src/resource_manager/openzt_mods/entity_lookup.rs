//! Entity lookup for checking if legacy entities exist.
//!
//! This module provides functions to check if legacy entities have been loaded
//! from the LEGACY_ATTRIBUTES_MAP, enabling the `entity_exists` patch condition.

use crate::resource_manager::openzt_mods::legacy_attributes::{LEGACY_ATTRIBUTES_MAP, LegacyEntityType};

/// Check if a legacy entity exists
///
/// # Arguments
/// * `entity_identifier` - Entity identifier in format "legacy.{type}.{name}"
///   Examples: "legacy.animals.elephant", "legacy.buildings.restroom"
///
/// # Returns
/// * `true` if the legacy entity exists
/// * `false` otherwise
pub fn entity_exists(entity_identifier: &str) -> bool {
    let lowercase = entity_identifier.to_lowercase();
    legacy_entity_exists(&lowercase)
}

/// Check if a legacy entity exists
///
/// Parses "legacy.{type}.{name}" format
///
/// # Arguments
/// * `identifier` - Entity identifier (lowercase)
///
/// # Returns
/// * `true` if the entity exists
/// * `false` otherwise
fn legacy_entity_exists(identifier: &str) -> bool {
    let parts: Vec<&str> = identifier.split(&['.', '/'][..]).collect();

    if parts.len() < 3 || parts[0] != "legacy" {
        return false;
    }

    let entity_type_str = parts[1];
    let entity_name = parts[2];

    let entity_type = match parse_legacy_entity_type(entity_type_str) {
        Some(et) => et,
        None => return false,
    };

    let map = LEGACY_ATTRIBUTES_MAP.lock().unwrap();
    map.get(&entity_type)
        .and_then(|entities| entities.get(entity_name))
        .is_some()
}

/// Parse entity type string to LegacyEntityType enum
///
/// # Arguments
/// * `type_str` - The entity type string (e.g., "animals", "buildings")
///
/// # Returns
/// * `Some(LegacyEntityType)` if the type is valid
/// * `None` if the type is invalid
fn parse_legacy_entity_type(type_str: &str) -> Option<LegacyEntityType> {
    match type_str {
        "animals" => Some(LegacyEntityType::Animal),
        "buildings" => Some(LegacyEntityType::Building),
        "fences" => Some(LegacyEntityType::Fence),
        "food" => Some(LegacyEntityType::Food),
        "guests" => Some(LegacyEntityType::Guest),
        "items" => Some(LegacyEntityType::Item),
        "paths" => Some(LegacyEntityType::Path),
        "scenery" => Some(LegacyEntityType::Scenery),
        "staff" => Some(LegacyEntityType::Staff),
        "walls" => Some(LegacyEntityType::Wall),
        _ => None,
    }
}
