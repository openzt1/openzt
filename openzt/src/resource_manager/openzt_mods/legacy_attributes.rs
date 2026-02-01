//! Legacy Zoo Tycoon entity attribute extraction and storage.
//!
//! This module handles extraction of entity attributes from vanilla Zoo Tycoon .ai files
//! during legacy loading, and provides access to those attributes for patch substitution.

use std::{collections::HashMap, str::FromStr, sync::Mutex};

use openzt_configparser::ini::Ini;
use std::sync::LazyLock;
use tracing::trace;

// Import LegacyCfgType from legacy_loading for conversion
use crate::resource_manager::legacy_loading::LegacyCfgType;

/// Entity types that correspond to Zoo Tycoon .cfg file patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LegacyEntityType {
    Animal,
    Building,
    Fence,
    Food,
    Guest,
    Item,
    Path,
    Scenery,
    Staff,
    Wall,
}

impl LegacyEntityType {
    /// Convert from LegacyCfgType (used in legacy_loading.rs)
    pub fn from_legacy_cfg_type(cfg_type: &LegacyCfgType) -> Option<Self> {
        match cfg_type {
            LegacyCfgType::Animal => Some(Self::Animal),
            LegacyCfgType::Building => Some(Self::Building),
            LegacyCfgType::Fence => Some(Self::Fence),
            LegacyCfgType::Food => Some(Self::Food),
            LegacyCfgType::Guest => Some(Self::Guest),
            LegacyCfgType::Item => Some(Self::Item),
            LegacyCfgType::Path => Some(Self::Path),
            LegacyCfgType::Scenery => Some(Self::Scenery),
            LegacyCfgType::Staff => Some(Self::Staff),
            LegacyCfgType::Wall => Some(Self::Wall),
            _ => None,
        }
    }

    /// Get the string representation for error messages
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Animal => "animals",
            Self::Building => "buildings",
            Self::Fence => "fences",
            Self::Food => "food",
            Self::Guest => "guests",
            Self::Item => "items",
            Self::Path => "paths",
            Self::Scenery => "scenery",
            Self::Staff => "staff",
            Self::Wall => "walls",
        }
    }

    /// Get the INI section name for this entity type in .cfg files
    pub fn section_name(&self) -> &'static str {
        match self {
            Self::Animal => "animals",
            Self::Building => "building",
            Self::Fence => "fences",
            Self::Food => "food",
            Self::Guest => "guests", // guests.cfg uses [guests] (plural)
            Self::Item => "items",
            Self::Path => "paths",
            Self::Scenery => "objects", // Primary section for scenery
            Self::Staff => "staff",
            Self::Wall => "tankwall",
        }
    }

    /// Get the default subtype for this entity type
    pub fn default_subtype(&self) -> Option<&'static str> {
        match self {
            Self::Animal => Some("m"),
            Self::Staff => Some("m"),
            Self::Fence => Some("f"),
            Self::Wall => Some("f"),
            _ => None,
        }
    }

    /// Check if this entity type supports subtypes
    pub fn has_subtypes(&self) -> bool {
        matches!(self, Self::Animal | Self::Staff | Self::Fence | Self::Wall)
    }

    /// Get the section name for attributes in .ai files
    pub fn attribute_section(&self) -> &'static str {
        match self {
            Self::Item => "characteristics", // Items use 'characteristics' (singular, no /Integers)
            _ => "Characteristics/Integers", // Everything else uses this
        }
    }
}

impl FromStr for LegacyEntityType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "animals" => Ok(Self::Animal),
            "buildings" => Ok(Self::Building),
            "fences" => Ok(Self::Fence),
            "food" => Ok(Self::Food),
            "guests" => Ok(Self::Guest),
            "items" => Ok(Self::Item),
            "paths" => Ok(Self::Path),
            "scenery" => Ok(Self::Scenery),
            "staff" => Ok(Self::Staff),
            "walls" => Ok(Self::Wall),
            _ => anyhow::bail!(
                "Invalid legacy entity type: '{}'. Valid types: animals, buildings, fences, food, guests, items, paths, scenery, staff, walls",
                s
            ),
        }
    }
}

/// Attributes for a specific subtype
#[derive(Debug, Clone)]
pub struct SubtypeAttributes {
    /// The subtype identifier (e.g., "m", "f", "man", etc.)
    pub subtype: String,
    /// cNameID from [Characteristics/Integers] section
    pub name_id: Option<u32>,
}

/// Extractable attributes from legacy .ai files (extensible)
///
/// For entities with subtypes, stores attributes per subtype.
/// For entities without subtypes, uses a single default entry.
#[derive(Debug, Clone)]
pub struct LegacyEntityAttributes {
    /// The entity name (e.g., "elephant" from "animals/elephant.ai")
    pub entity_name: String,
    /// Map of subtype -> attributes
    /// For non-subtype entities, all attributes stored under empty string key
    pub subtype_attributes: HashMap<String, SubtypeAttributes>,
}

impl LegacyEntityAttributes {
    /// Create a new attributes struct
    pub fn new(entity_name: String) -> Self {
        Self {
            entity_name,
            subtype_attributes: HashMap::new(),
        }
    }

    /// Get name_id for a specific subtype with fallback logic
    pub fn get_name_id(&self, subtype: Option<&str>) -> Option<u32> {
        // Try specified subtype first
        if let Some(st) = subtype {
            if let Some(attrs) = self.subtype_attributes.get(st) {
                if attrs.name_id.is_some() {
                    return attrs.name_id;
                }
            }
        }

        // Fallback: return the first available name_id (used for all subtypes if only one exists)
        let name_ids: Vec<_> = self.subtype_attributes.values().filter_map(|a| a.name_id).collect();

        if !name_ids.is_empty() {
            return Some(name_ids[0]); // Return first available
        }

        // Otherwise return None
        None
    }

    /// Get all subtypes that have attributes
    pub fn get_subtypes_with_name_id(&self) -> Vec<String> {
        self.subtype_attributes.values().filter(|a| a.name_id.is_some()).map(|a| a.subtype.clone()).collect()
    }

    /// Parse attributes from an .ai file's INI content
    /// For entities with subtypes, parses sections like "m/Characteristics/Integers"
    ///
    /// Note: For Guest entities, the entity_name is used to filter sections (e.g., "man" entity
    /// only gets the "man/Characteristics/Integers" section, not all guest type sections).
    pub fn parse_from_ini(entity_name: String, ini: &Ini, entity_type: LegacyEntityType) -> anyhow::Result<Self> {
        let mut attrs = Self::new(entity_name.clone());

        let section_base = entity_type.attribute_section();

        if let Some(map) = ini.get_map() {
            // First, try to find subtype-specific sections
            let mut found_subtypes = false;

            for (section_name, _) in map.iter() {
                // Check if this section matches the pattern "<subtype>/<section_base>"
                if let Some(subtype_section) = section_name.strip_suffix(&format!("/{}", section_base)) {
                    if !subtype_section.is_empty() {
                        let subtype = subtype_section.to_string();

                        // For Guest entities, only include the section matching the entity name
                        // because guest.ai contains sections for all guest types (man, woman, boy, girl)
                        if entity_type == LegacyEntityType::Guest {
                            if subtype != entity_name {
                                continue;
                            }
                            // For Guest entities, convert the matching subtype to a non-subtype entity
                            // by storing it under the empty string key instead of the subtype name
                            if let Some(section) = map.get(section_name) {
                                let mut subtype_attrs = SubtypeAttributes {
                                    subtype: String::new(), // Empty string for non-subtype entities
                                    name_id: None,
                                };

                                for (key, values) in section.iter() {
                                    if let Some(values_vec) = values {
                                        if let Some(value) = values_vec.first() {
                                            match key.as_str() {
                                                "cNameID" | "nameID" => {
                                                    // Support both formats
                                                    subtype_attrs.name_id = value.parse().ok();
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }

                                attrs.subtype_attributes.insert(String::new(), subtype_attrs);
                                found_subtypes = true;
                            }
                            continue;
                        }

                        if let Some(section) = map.get(section_name) {
                            let mut subtype_attrs = SubtypeAttributes {
                                subtype: subtype.clone(),
                                name_id: None,
                            };

                            for (key, values) in section.iter() {
                                if let Some(values_vec) = values {
                                    if let Some(value) = values_vec.first() {
                                        match key.as_str() {
                                            "cNameID" | "nameID" => {
                                                // Support both formats
                                                subtype_attrs.name_id = value.parse().ok();
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }

                            attrs.subtype_attributes.insert(subtype, subtype_attrs);
                            found_subtypes = true;
                        }
                    }
                }
            }

            // If no subtype sections found, try the base section directly
            if !found_subtypes {
                if let Some(section) = map.get(section_base) {
                    let mut subtype_attrs = SubtypeAttributes {
                        subtype: String::new(), // Empty string for non-subtype entities
                        name_id: None,
                    };

                    for (key, values) in section.iter() {
                        if let Some(values_vec) = values {
                            if let Some(value) = values_vec.first() {
                                match key.as_str() {
                                    "cNameID" | "nameID" => {
                                        // Support both formats
                                        subtype_attrs.name_id = value.parse().ok();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    attrs.subtype_attributes.insert(String::new(), subtype_attrs);
                }
            }
        }

        Ok(attrs)
    }

    /// Get a comma-separated list of subtypes for debugging
    pub fn subtype_list(&self) -> String {
        if self.subtype_attributes.is_empty() || (self.subtype_attributes.len() == 1 && self.subtype_attributes.contains_key("")) {
            return String::new(); // Return empty string instead of "(no subtypes)"
        }

        let mut subtypes: Vec<_> = self.subtype_attributes.keys().filter(|k| !k.is_empty()).cloned().collect();
        subtypes.sort();
        subtypes.join(", ")
    }
}

/// Global storage for legacy entity attributes
/// Structure: entity_type -> entity_name -> attributes
pub static LEGACY_ATTRIBUTES_MAP: LazyLock<Mutex<HashMap<LegacyEntityType, HashMap<String, LegacyEntityAttributes>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register a legacy entity's attributes
pub fn add_legacy_entity(entity_type: LegacyEntityType, entity_name: String, attributes: LegacyEntityAttributes) -> anyhow::Result<()> {
    let mut map = LEGACY_ATTRIBUTES_MAP.lock().unwrap();

    let subtype_list = attributes.subtype_list();
    let name_id_display = if let Some(nid) = attributes.get_name_id(None) {
        nid.to_string()
    } else {
        "(none)".to_string()
    };

    trace!(
        "Registering legacy entity: type={:?}, name={}, name_id={}, subtypes=[{}]",
        entity_type,
        entity_name,
        name_id_display,
        subtype_list
    );

    map.entry(entity_type).or_default().insert(entity_name, attributes);

    Ok(())
}

/// Get a specific attribute from a legacy entity
///
/// # Arguments
/// * `entity_type` - The type of entity (animals, buildings, etc.)
/// * `entity_name` - The name of the entity (e.g., "elephant")
/// * `attribute` - The attribute name (currently only "name_id" is supported)
///
/// # Returns
/// * `Ok(String)` - The attribute value as a string
/// * `Err` - If the entity, type, or attribute is not found
pub fn get_legacy_attribute(entity_type: LegacyEntityType, entity_name: &str, attribute: &str) -> anyhow::Result<String> {
    get_legacy_attribute_with_subtype(entity_type, entity_name, None, attribute)
}

/// Get a specific attribute from a legacy entity with optional subtype
///
/// # Arguments
/// * `entity_type` - The type of entity (animals, buildings, etc.)
/// * `entity_name` - The name of the entity (e.g., "elephant")
/// * `subtype` - Optional subtype (e.g., "m", "f", "man")
/// * `attribute` - The attribute name (currently only "name_id" is supported)
///
/// # Returns
/// * `Ok(String)` - The attribute value as a string
/// * `Err` - If the entity, type, or attribute is not found
pub fn get_legacy_attribute_with_subtype(entity_type: LegacyEntityType, entity_name: &str, subtype: Option<&str>, attribute: &str) -> anyhow::Result<String> {
    let map = LEGACY_ATTRIBUTES_MAP.lock().unwrap();

    let entity_map = map
        .get(&entity_type)
        .ok_or_else(|| anyhow::anyhow!("No entities found for type '{}'", entity_type.as_str()))?;

    let attrs = entity_map.get(entity_name).ok_or_else(|| {
        let available: Vec<&str> = entity_map.keys().take(5).map(|s| s.as_str()).collect();
        anyhow::anyhow!(
            "Entity '{}' not found in type '{}'. Available entities: {}",
            entity_name,
            entity_type.as_str(),
            available.join(", ")
        )
    })?;

    match attribute {
        "name_id" => attrs
            .get_name_id(subtype)
            .ok_or_else(|| {
                if let Some(st) = subtype {
                    anyhow::anyhow!("Entity '{}' has no cNameID for subtype '{}'", entity_name, st)
                } else {
                    anyhow::anyhow!("Entity '{}' has no cNameID", entity_name)
                }
            })
            .map(|id| id.to_string()),
        _ => anyhow::bail!("Unsupported attribute '{}'. Only 'name_id' is currently supported.", attribute),
    }
}

/// Check if a legacy entity exists
pub fn legacy_entity_exists(entity_type: LegacyEntityType, entity_name: &str) -> bool {
    let map = LEGACY_ATTRIBUTES_MAP.lock().unwrap();
    map.get(&entity_type).and_then(|m| m.get(entity_name)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_entity_type_from_str() {
        assert_eq!(LegacyEntityType::from_str("animals").unwrap(), LegacyEntityType::Animal);
        assert_eq!(LegacyEntityType::from_str("buildings").unwrap(), LegacyEntityType::Building);
        assert!(LegacyEntityType::from_str("invalid").is_err());
    }

    #[test]
    fn test_legacy_entity_type_as_str() {
        assert_eq!(LegacyEntityType::Animal.as_str(), "animals");
        assert_eq!(LegacyEntityType::Building.as_str(), "buildings");
    }

    #[test]
    fn test_legacy_entity_type_section_name() {
        assert_eq!(LegacyEntityType::Animal.section_name(), "animals");
        assert_eq!(LegacyEntityType::Building.section_name(), "building");
        assert_eq!(LegacyEntityType::Guest.section_name(), "guests");
        assert_eq!(LegacyEntityType::Scenery.section_name(), "objects");
    }

    // =========================================================================
    // Tests for get_name_id() with explicit subtype
    // =========================================================================

    #[test]
    fn test_get_name_id_explicit_subtype_male() {
        let mut attrs = LegacyEntityAttributes::new("elephant".to_string());
        attrs.subtype_attributes.insert(
            "m".to_string(),
            SubtypeAttributes {
                subtype: "m".to_string(),
                name_id: Some(1001),
            },
        );
        attrs.subtype_attributes.insert(
            "f".to_string(),
            SubtypeAttributes {
                subtype: "f".to_string(),
                name_id: Some(1002),
            },
        );

        // Request explicit male subtype
        assert_eq!(attrs.get_name_id(Some("m")), Some(1001));
    }

    #[test]
    fn test_get_name_id_explicit_subtype_female() {
        let mut attrs = LegacyEntityAttributes::new("elephant".to_string());
        attrs.subtype_attributes.insert(
            "m".to_string(),
            SubtypeAttributes {
                subtype: "m".to_string(),
                name_id: Some(1001),
            },
        );
        attrs.subtype_attributes.insert(
            "f".to_string(),
            SubtypeAttributes {
                subtype: "f".to_string(),
                name_id: Some(1002),
            },
        );

        // Request explicit female subtype
        assert_eq!(attrs.get_name_id(Some("f")), Some(1002));
    }

    #[test]
    fn test_get_name_id_explicit_subtype_not_found() {
        let mut attrs = LegacyEntityAttributes::new("elephant".to_string());
        attrs.subtype_attributes.insert(
            "m".to_string(),
            SubtypeAttributes {
                subtype: "m".to_string(),
                name_id: Some(1001),
            },
        );

        // Request non-existent subtype - should return first available
        assert_eq!(attrs.get_name_id(Some("x")), Some(1001));
    }

    // =========================================================================
    // Tests for get_name_id() with default subtype fallback (None)
    // =========================================================================

    #[test]
    fn test_get_name_id_none_returns_first_available() {
        let mut attrs = LegacyEntityAttributes::new("elephant".to_string());
        attrs.subtype_attributes.insert(
            "m".to_string(),
            SubtypeAttributes {
                subtype: "m".to_string(),
                name_id: Some(1001),
            },
        );
        attrs.subtype_attributes.insert(
            "f".to_string(),
            SubtypeAttributes {
                subtype: "f".to_string(),
                name_id: Some(1002),
            },
        );

        // Request no subtype (None) - should return first available
        let result = attrs.get_name_id(None);
        assert!(result == Some(1001) || result == Some(1002));
    }

    #[test]
    fn test_get_name_id_none_single_subtype() {
        let mut attrs = LegacyEntityAttributes::new("restroom".to_string());
        attrs.subtype_attributes.insert(
            "".to_string(),
            SubtypeAttributes {
                subtype: "".to_string(),
                name_id: Some(5000),
            },
        );

        // Request no subtype - should return the single available
        assert_eq!(attrs.get_name_id(None), Some(5000));
    }

    #[test]
    fn test_get_name_id_none_no_subtypes() {
        let attrs = LegacyEntityAttributes::new("empty".to_string());

        // No subtypes available
        assert_eq!(attrs.get_name_id(None), None);
    }

    // =========================================================================
    // Tests for get_name_id() with fallback when only one subtype has name_id
    // =========================================================================

    #[test]
    fn test_get_name_id_fallback_single_name_id() {
        let mut attrs = LegacyEntityAttributes::new("guest".to_string());
        attrs.subtype_attributes.insert(
            "man".to_string(),
            SubtypeAttributes {
                subtype: "man".to_string(),
                name_id: Some(3001),
            },
        );
        attrs.subtype_attributes.insert(
            "woman".to_string(),
            SubtypeAttributes {
                subtype: "woman".to_string(),
                name_id: None, // No name_id
            },
        );

        // Request a subtype with no name_id - should return the first available
        assert_eq!(attrs.get_name_id(Some("woman")), Some(3001));
    }

    #[test]
    fn test_get_name_id_all_subtypes_no_name_id() {
        let mut attrs = LegacyEntityAttributes::new("entity".to_string());
        attrs.subtype_attributes.insert(
            "a".to_string(),
            SubtypeAttributes {
                subtype: "a".to_string(),
                name_id: None,
            },
        );
        attrs.subtype_attributes.insert(
            "b".to_string(),
            SubtypeAttributes {
                subtype: "b".to_string(),
                name_id: None,
            },
        );

        // All subtypes have no name_id
        assert_eq!(attrs.get_name_id(Some("a")), None);
        assert_eq!(attrs.get_name_id(None), None);
    }

    // =========================================================================
    // Tests for subtype_list()
    // =========================================================================

    #[test]
    fn test_subtype_list_no_subtypes() {
        let mut attrs = LegacyEntityAttributes::new("restroom".to_string());
        attrs.subtype_attributes.insert(
            "".to_string(),
            SubtypeAttributes {
                subtype: "".to_string(),
                name_id: Some(5000),
            },
        );

        assert_eq!(attrs.subtype_list(), "");
    }

    #[test]
    fn test_subtype_list_empty() {
        let attrs = LegacyEntityAttributes::new("empty".to_string());
        assert_eq!(attrs.subtype_list(), "");
    }

    #[test]
    fn test_subtype_list_multiple_subtypes() {
        let mut attrs = LegacyEntityAttributes::new("elephant".to_string());
        attrs.subtype_attributes.insert(
            "m".to_string(),
            SubtypeAttributes {
                subtype: "m".to_string(),
                name_id: Some(1001),
            },
        );
        attrs.subtype_attributes.insert(
            "f".to_string(),
            SubtypeAttributes {
                subtype: "f".to_string(),
                name_id: Some(1002),
            },
        );

        // Subtypes should be sorted alphabetically
        assert_eq!(attrs.subtype_list(), "f, m");
    }

    // =========================================================================
    // Tests for default_subtype()
    // =========================================================================

    #[test]
    fn test_default_subtype_animal() {
        assert_eq!(LegacyEntityType::Animal.default_subtype(), Some("m"));
    }

    #[test]
    fn test_default_subtype_staff() {
        assert_eq!(LegacyEntityType::Staff.default_subtype(), Some("m"));
    }

    #[test]
    fn test_default_subtype_fence() {
        assert_eq!(LegacyEntityType::Fence.default_subtype(), Some("f"));
    }

    #[test]
    fn test_default_subtype_wall() {
        assert_eq!(LegacyEntityType::Wall.default_subtype(), Some("f"));
    }

    #[test]
    fn test_default_subtype_guest() {
        assert_eq!(LegacyEntityType::Guest.default_subtype(), None);
    }

    #[test]
    fn test_default_subtype_building() {
        assert_eq!(LegacyEntityType::Building.default_subtype(), None);
    }

    // =========================================================================
    // Tests for has_subtypes()
    // =========================================================================

    #[test]
    fn test_has_subtypes_true() {
        assert!(LegacyEntityType::Animal.has_subtypes());
        assert!(LegacyEntityType::Staff.has_subtypes());
        assert!(LegacyEntityType::Fence.has_subtypes());
        assert!(LegacyEntityType::Wall.has_subtypes());
    }

    #[test]
    fn test_has_subtypes_false() {
        assert!(!LegacyEntityType::Building.has_subtypes());
        assert!(!LegacyEntityType::Food.has_subtypes());
        assert!(!LegacyEntityType::Guest.has_subtypes());
        assert!(!LegacyEntityType::Item.has_subtypes());
        assert!(!LegacyEntityType::Path.has_subtypes());
        assert!(!LegacyEntityType::Scenery.has_subtypes());
    }

    // =========================================================================
    // Tests for attribute_section()
    // =========================================================================

    #[test]
    fn test_attribute_section_item() {
        // Items use 'characteristics' (singular)
        assert_eq!(LegacyEntityType::Item.attribute_section(), "characteristics");
    }

    #[test]
    fn test_attribute_section_others() {
        // Everything else uses 'Characteristics/Integers'
        assert_eq!(LegacyEntityType::Animal.attribute_section(), "Characteristics/Integers");
        assert_eq!(LegacyEntityType::Building.attribute_section(), "Characteristics/Integers");
        assert_eq!(LegacyEntityType::Fence.attribute_section(), "Characteristics/Integers");
    }

    // =========================================================================
    // Tests for get_subtypes_with_name_id()
    // =========================================================================

    #[test]
    fn test_get_subtypes_with_name_id() {
        let mut attrs = LegacyEntityAttributes::new("elephant".to_string());
        attrs.subtype_attributes.insert(
            "m".to_string(),
            SubtypeAttributes {
                subtype: "m".to_string(),
                name_id: Some(1001),
            },
        );
        attrs.subtype_attributes.insert(
            "f".to_string(),
            SubtypeAttributes {
                subtype: "f".to_string(),
                name_id: Some(1002),
            },
        );
        attrs.subtype_attributes.insert(
            "j".to_string(),
            SubtypeAttributes {
                subtype: "j".to_string(),
                name_id: None, // No name_id
            },
        );

        let subtypes = attrs.get_subtypes_with_name_id();
        assert_eq!(subtypes.len(), 2);
        assert!(subtypes.contains(&"m".to_string()));
        assert!(subtypes.contains(&"f".to_string()));
        assert!(!subtypes.contains(&"j".to_string()));
    }
}
