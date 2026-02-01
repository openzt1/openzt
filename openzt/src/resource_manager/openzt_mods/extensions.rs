//! Extension mod storage and retrieval system.

use std::sync::LazyLock;
use std::{collections::HashMap, str::FromStr, sync::Mutex};
use tracing::debug;

use crate::mods::EntityExtension;
use crate::resource_manager::openzt_mods::legacy_attributes::LegacyEntityType;
use crate::util::{get_from_memory, get_string_from_memory};

/// Represents which entity types a tag or attribute can apply to
#[derive(Debug, Clone, PartialEq)]
pub enum EntityScope {
    Single(LegacyEntityType),
    Multiple(Vec<LegacyEntityType>),
    All,
}

impl EntityScope {
    pub fn includes(&self, entity_type: LegacyEntityType) -> bool {
        match self {
            EntityScope::Single(t) => *t == entity_type,
            EntityScope::Multiple(types) => types.contains(&entity_type),
            EntityScope::All => true,
        }
    }

    pub fn single(entity_type: LegacyEntityType) -> Self {
        EntityScope::Single(entity_type)
    }

    pub fn multiple(types: Vec<LegacyEntityType>) -> Self {
        EntityScope::Multiple(types)
    }

    pub fn all() -> Self {
        EntityScope::All
    }
}

#[derive(Debug, Clone)]
pub struct TagDefinition {
    pub name: String,
    pub description: String,
    pub scope: EntityScope,
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct AttributeDefinition {
    pub name: String,
    pub description: String,
    pub scope: EntityScope,
    pub module: String,
    pub value_type: Option<String>,
}

pub struct ExtensionRegistry {
    tags: HashMap<String, TagDefinition>,
    attributes: HashMap<String, AttributeDefinition>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            tags: HashMap::new(),
            attributes: HashMap::new(),
        }
    }

    pub fn register_tag(&mut self, tag: TagDefinition) -> Result<(), String> {
        if self.tags.contains_key(&tag.name) {
            return Err(format!("Tag '{}' already registered", tag.name));
        }
        self.tags.insert(tag.name.clone(), tag);
        Ok(())
    }

    pub fn register_attribute(&mut self, attr: AttributeDefinition) -> Result<(), String> {
        if self.attributes.contains_key(&attr.name) {
            return Err(format!("Attribute '{}' already registered", attr.name));
        }
        self.attributes.insert(attr.name.clone(), attr);
        Ok(())
    }

    pub fn is_tag_valid(&self, tag: &str, entity_type: LegacyEntityType) -> bool {
        self.tags.get(tag).map(|def| def.scope.includes(entity_type)).unwrap_or(false)
    }

    pub fn is_attribute_valid(&self, attr: &str, entity_type: LegacyEntityType) -> bool {
        self.attributes.get(attr).map(|def| def.scope.includes(entity_type)).unwrap_or(false)
    }

    pub fn list_tags(&self) -> Vec<&TagDefinition> {
        self.tags.values().collect()
    }

    pub fn list_attributes(&self) -> Vec<&AttributeDefinition> {
        self.attributes.values().collect()
    }
}

/// Global registry for tag and attribute definitions
pub static EXTENSION_REGISTRY: LazyLock<Mutex<ExtensionRegistry>> = LazyLock::new(|| Mutex::new(ExtensionRegistry::new()));

/// Register a tag for specific entity types
pub fn register_tag(module: &str, name: &str, description: &str, scope: EntityScope) -> Result<(), String> {
    let definition = TagDefinition {
        name: name.to_string(),
        description: description.to_string(),
        scope,
        module: module.to_string(),
    };

    EXTENSION_REGISTRY.lock().unwrap().register_tag(definition)
}

/// Register an attribute for specific entity types
pub fn register_attribute(module: &str, name: &str, description: &str, scope: EntityScope, value_type: Option<&str>) -> Result<(), String> {
    let definition = AttributeDefinition {
        name: name.to_string(),
        description: description.to_string(),
        scope,
        module: module.to_string(),
        value_type: value_type.map(|s| s.to_string()),
    };

    EXTENSION_REGISTRY.lock().unwrap().register_attribute(definition)
}

/// Extension record with metadata
#[derive(Debug, Clone)]
pub struct ExtensionRecord {
    pub mod_id: String,
    pub base: String,          // e.g., "legacy.animals.elephant"
    pub extension_key: String, // e.g., "animals.elephant"
    pub extension: EntityExtension,
}

/// Global storage: extension_key -> ExtensionRecord
pub static EXTENSION_STORAGE: LazyLock<Mutex<HashMap<String, ExtensionRecord>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Also store by base for lookups: base -> extension_key
pub static EXTENSION_BY_BASE: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn add_extension(mod_id: String, extension_key: String, extension: EntityExtension) -> anyhow::Result<()> {
    let base = extension.base().clone();

    // Parse entity type from base (e.g., "legacy.animals.elephant" -> Animal)
    let entity_type = extract_entity_type_from_base(&base)?;

    let registry = EXTENSION_REGISTRY.lock().unwrap();

    // Validate all attributes against registry
    for attr_key in extension.attributes().keys() {
        if !registry.is_attribute_valid(attr_key, entity_type) {
            return Err(anyhow::anyhow!("Unsupported attribute '{}' for entity type '{}'", attr_key, entity_type.as_str()));
        }
    }

    // Validate all tags against registry
    for tag in extension.tags().iter() {
        if !registry.is_tag_valid(tag, entity_type) {
            return Err(anyhow::anyhow!("Unsupported tag '{}' for entity type '{}'", tag, entity_type.as_str()));
        }
    }

    drop(registry);

    {
        let mut by_base = EXTENSION_BY_BASE.lock().unwrap();
        by_base.insert(base.clone(), extension_key.clone());
    }

    let mut storage = EXTENSION_STORAGE.lock().unwrap();
    debug!(
        "Registering extension: key={}, base={}, mod={}, tags={}, attributes={}",
        extension_key,
        base,
        mod_id,
        extension.tags().len(),
        extension.attributes().len()
    );

    storage.insert(
        extension_key.clone(),
        ExtensionRecord {
            mod_id,
            base,
            extension_key,
            extension,
        },
    );

    Ok(())
}

/// Helper to extract entity type from base string
fn extract_entity_type_from_base(base: &str) -> anyhow::Result<LegacyEntityType> {
    // Base format: "legacy.{type}.{name}" or "legacy.{type}/{subtype}/{name}"
    let parts: Vec<&str> = base.split(&['.', '/'][..]).collect();

    if parts.len() < 2 || parts[0] != "legacy" {
        anyhow::bail!("Invalid base format: '{}'", base);
    }

    let type_str = parts[1];
    LegacyEntityType::from_str(type_str).map_err(|_| anyhow::anyhow!("Unknown entity type '{}' in base '{}'", type_str, base))
}

pub fn get_extension(extension_key: &str) -> Option<ExtensionRecord> {
    let storage = EXTENSION_STORAGE.lock().unwrap();
    storage.get(extension_key).cloned()
}

pub fn get_extension_by_base(base: &str) -> Option<ExtensionRecord> {
    let by_base = EXTENSION_BY_BASE.lock().unwrap();
    let extension_key = by_base.get(base)?.clone();
    drop(by_base);

    get_extension(&extension_key)
}

pub fn get_entity_tags(extension_key: &str) -> anyhow::Result<Vec<String>> {
    match get_extension(extension_key) {
        Some(record) => Ok(record.extension.tags().clone()),
        None => Ok(Vec::new()),
    }
}

pub fn get_entity_attribute(extension_key: &str, attribute_key: &str) -> anyhow::Result<Option<String>> {
    match get_extension(extension_key) {
        Some(record) => Ok(record.extension.attributes().get(attribute_key).cloned()),
        None => Ok(None),
    }
}

pub fn entity_has_tag(extension_key: &str, tag: &str) -> anyhow::Result<bool> {
    match get_extension(extension_key) {
        Some(record) => Ok(record.extension.tags().contains(&tag.to_string())),
        None => Ok(false),
    }
}

pub fn list_extensions_with_tag(tag: &str) -> Vec<String> {
    let storage = EXTENSION_STORAGE.lock().unwrap();
    storage
        .iter()
        .filter(|(_, record)| record.extension.tags().contains(&tag.to_string()))
        .map(|(key, _)| key.clone())
        .collect()
}

#[cfg(feature = "integration-tests")]
pub fn clear_extensions() {
    EXTENSION_STORAGE.lock().unwrap().clear();
    EXTENSION_BY_BASE.lock().unwrap().clear();
    EXTENSION_REGISTRY.lock().unwrap().clear();
}

impl ExtensionRegistry {
    #[cfg(feature = "integration-tests")]
    pub fn clear(&mut self) {
        self.tags.clear();
        self.attributes.clear();
    }
}

/// Vtable addresses for entity types (from bfentitytype.rs)
/// Maps vtable addresses to entity types for runtime entity type detection
static VTABLE_TO_ENTITY_TYPE: &[(&str, LegacyEntityType)] = &[
    ("0x6303f4", LegacyEntityType::Scenery),  // Scenery
    ("0x6307e4", LegacyEntityType::Building), // Building
    ("0x630268", LegacyEntityType::Animal),   // Animal
    ("0x62e330", LegacyEntityType::Guest),    // Guest
    ("0x63034c", LegacyEntityType::Fence),    // Fence
    ("0x630544", LegacyEntityType::Food),     // Food
    ("0x63049c", LegacyEntityType::Path),     // Path
    ("0x6305ec", LegacyEntityType::Wall),     // TankWall
    ("0x630694", LegacyEntityType::Wall),     // TankFilter
    ("0x62e7d8", LegacyEntityType::Staff),    // Keeper
    ("0x62e704", LegacyEntityType::Staff),    // MaintenanceWorker
    ("0x62e980", LegacyEntityType::Staff),    // DRT
    ("0x62e8ac", LegacyEntityType::Staff),    // TourGuide
    ("0x62e1e8", LegacyEntityType::Scenery),  // Ambient
    ("0x63073c", LegacyEntityType::Scenery),  // Rubble
];

/// Guess entity type from vtable address
fn guess_entity_type_from_vtable(vtable: u32) -> Option<LegacyEntityType> {
    let vtable_str = format!("{:#x}", vtable);
    VTABLE_TO_ENTITY_TYPE.iter().find(|(vt, _)| *vt == vtable_str).map(|(_, et)| *et)
}

/// Get the base string (e.g., "legacy.scenery.statue") for an in-game entity
///
/// This function reads the entity type from memory and constructs the base string
/// that matches the format used in extension definitions.
///
/// # Arguments
/// * `entity_ptr` - Pointer to the BFEntity in memory
///
/// # Returns
/// * `Some(String)` - The base string (e.g., "legacy.scenery.statue")
/// * `None` - If the entity type cannot be determined
///
/// # Example
/// ```rust
/// if let Some(base) = get_entity_base(entity_ptr) {
///     println!("Entity base: {}", base); // "legacy.scenery.statue"
/// }
/// ```
pub fn get_entity_base(entity_ptr: u32) -> Option<String> {
    // BFEntity.inner_class_ptr at offset 0x128
    let entity_type_ptr = get_from_memory::<u32>(entity_ptr + 0x128);
    if entity_type_ptr == 0 {
        return None;
    }

    // Read zt_type from offset 0x98
    let zt_type_ptr = get_from_memory::<u32>(entity_type_ptr + 0x98);
    if zt_type_ptr == 0 {
        return None;
    }
    let zt_type = get_string_from_memory(zt_type_ptr);

    // Read zt_sub_type from offset 0xa4
    let zt_sub_type_ptr = get_from_memory::<u32>(entity_type_ptr + 0xa4);
    let zt_sub_type = if zt_sub_type_ptr != 0 {
        get_string_from_memory(zt_sub_type_ptr)
    } else {
        String::new()
    };

    // Get entity class from vtable
    let vtable = get_from_memory::<u32>(entity_type_ptr);
    let entity_type = guess_entity_type_from_vtable(vtable)?;

    // For subtype entities (animals, staff, fences, walls), the entity name is in zt_type
    // For non-subtype entities (scenery, buildings, etc.), the entity name is in zt_sub_type
    let type_str = if entity_type.has_subtypes() {
        &zt_type
    } else if !zt_sub_type.is_empty() {
        &zt_sub_type
    } else {
        &zt_type
    };

    // Construct base string: "legacy.{type}.{name}"
    Some(format!("legacy.{}.{}", entity_type.as_str(), type_str))
}
