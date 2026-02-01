use std::{collections::HashMap, error::Error, fmt, str::FromStr};

use getset::Getters;
use indexmap::IndexMap;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize,
};
use toml::Value;
use tracing::warn;

#[derive(Debug)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    pub fn new(message: String) -> Self {
        ParseError { message }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseError: {}", self.message)
    }
}

impl Error for ParseError {}

impl From<std::num::ParseIntError> for ParseError {
    fn from(err: std::num::ParseIntError) -> Self {
        ParseError {
            message: format!("Failed to parse int: {}", err),
        }
    }
}

#[derive(Deserialize, Debug, Getters, Clone)]
#[serde(deny_unknown_fields)]
#[get = "pub"]
pub struct Meta {
    name: String,
    description: String,
    authors: Vec<String>,
    mod_id: String,
    #[serde(deserialize_with = "deserialize_version")]
    version: Version,
    #[serde(default)]
    ztd_type: ZtdType,
    link: Option<String>,
    #[serde(default = "default_empty_dependencies", deserialize_with = "deserialize_dependencies")]
    dependencies: Vec<Dependencies>,
}

fn default_empty_dependencies() -> Vec<Dependencies> {
    Vec::new()
}

fn deserialize_dependencies<'de, D>(deserializer: D) -> Result<Vec<Dependencies>, D::Error>
where
    D: Deserializer<'de>,
{
    struct DependenciesVisitor;

    impl<'de> Visitor<'de> for DependenciesVisitor {
        type Value = Vec<Dependencies>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an array of dependency objects")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut deps = Vec::new();
            let mut index = 0;

            while let Some(value) = seq.next_element::<Value>()? {
                let value_str = format!("{}", value);
                match value.try_into::<Dependencies>() {
                    Ok(dep) => deps.push(dep),
                    Err(e) => {
                        // Skip invalid dependency with a warning (using eprintln as a fallback
                        // since we don't have access to tracing during deserialization)
                        warn!(
                            "Warning: Skipping invalid dependency at index {}: {}. \
                            Dependency will be ignored. Value: {}",
                            index, e, value_str
                        );
                    }
                }
                index += 1;
            }

            Ok(deps)
        }
    }

    deserializer.deserialize_seq(DependenciesVisitor)
}

#[derive(Deserialize, Default, PartialEq, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ZtdType {
    Legacy,
    #[default]
    Combined,
    Openzt,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Getters)]
#[get = "pub"]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl FromStr for Version {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(ParseError::new(format!("Invalid version string: {} (expected 'x.y.z' e.g '1.0.0')", s)));
        }

        Ok(Version {
            major: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn deserialize_version<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
    D: Deserializer<'de>,
{
    struct VersionVisitor;

    impl Visitor<'_> for VersionVisitor {
        type Value = Version;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a version string in the format 'x.y.z'")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Version::from_str(value).map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_str(VersionVisitor)
}

fn deserialize_version_option<'de, D>(deserializer: D) -> Result<Option<Version>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionVersionVisitor;

    impl Visitor<'_> for OptionVersionVisitor {
        type Value = Option<Version>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a version string in the format 'x.y.z'")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match Version::from_str(value) {
                Ok(v) => Ok(Some(v)),
                Err(err) => Err(de::Error::custom(err)),
            }
        }
    }

    deserializer.deserialize_str(OptionVersionVisitor)
}

fn default_as_false() -> bool {
    false
}

/// Dependency identifier types
///
/// Represents three ways to identify a dependency:
/// - ModId: OpenZT mod ID (e.g., "finn.my_mod")
/// - ZtdName: The .ztd filename (e.g., "my_mod.ztd")
/// - DllName: Zoo Tycoon game DLL (e.g., "langusa.dll")
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyIdentifier {
    ModId(String),
    ZtdName(String),
    DllName(String),
}

impl<'de> Deserialize<'de> for DependencyIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            ModId,
            ZtdName,
            DllName,
        }

        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = DependencyIdentifier;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("one of: mod_id, ztd_name, or dll_name")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut mod_id = None;
                let mut ztd_name = None;
                let mut dll_name = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::ModId => {
                            if mod_id.is_some() {
                                return Err(de::Error::duplicate_field("mod_id"));
                            }
                            mod_id = Some(map.next_value()?);
                        }
                        Field::ZtdName => {
                            if ztd_name.is_some() {
                                return Err(de::Error::duplicate_field("ztd_name"));
                            }
                            ztd_name = Some(map.next_value()?);
                        }
                        Field::DllName => {
                            if dll_name.is_some() {
                                return Err(de::Error::duplicate_field("dll_name"));
                            }
                            dll_name = Some(map.next_value()?);
                        }
                    }
                }

                // Priority: mod_id > ztd_name > dll_name
                if let Some(id) = mod_id {
                    Ok(DependencyIdentifier::ModId(id))
                } else if let Some(name) = ztd_name {
                    Ok(DependencyIdentifier::ZtdName(name))
                } else if let Some(name) = dll_name {
                    Ok(DependencyIdentifier::DllName(name))
                } else {
                    Err(de::Error::missing_field("mod_id, ztd_name, or dll_name"))
                }
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

#[derive(Deserialize, Clone, Debug, Getters)]
#[get = "pub"]
pub struct Dependencies {
    #[serde(flatten)]
    identifier: DependencyIdentifier,
    name: String,
    #[serde(default, deserialize_with = "deserialize_version_option")]
    min_version: Option<Version>,
    #[serde(default = "default_as_false")]
    optional: bool,
    #[serde(default)]
    ordering: Ordering,
}

#[derive(Deserialize, Default, PartialEq, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Ordering {
    Before,
    After,
    #[default]
    None,
}

#[derive(Deserialize, Debug, Getters)]
#[serde(deny_unknown_fields)]
#[get = "pub"]
pub struct ModDefinition {
    habitats: Option<HashMap<String, IconDefinition>>,
    locations: Option<HashMap<String, IconDefinition>>,

    // Entity type-specific extension fields
    #[serde(default)]
    scenery: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    animals: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    buildings: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    fences: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    walls: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    paths: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    food: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    staff: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    guests: Option<HashMap<String, EntityExtension>>,
    #[serde(default)]
    items: Option<HashMap<String, EntityExtension>>,

    // Patch system - split into metadata and patches
    patch_meta: Option<PatchMeta>,
    patches: Option<IndexMap<String, Patch>>, // MUST use IndexMap for order preservation
}

impl ModDefinition {
    /// Get all extensions as a single HashMap
    pub fn extensions(&self) -> HashMap<String, EntityExtension> {
        let mut all = HashMap::new();

        // Collect from each entity type field
        if let Some(ref ext) = self.scenery {
            for (k, v) in ext {
                all.insert(format!("scenery.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.animals {
            for (k, v) in ext {
                all.insert(format!("animals.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.buildings {
            for (k, v) in ext {
                all.insert(format!("buildings.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.fences {
            for (k, v) in ext {
                all.insert(format!("fences.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.walls {
            for (k, v) in ext {
                all.insert(format!("walls.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.paths {
            for (k, v) in ext {
                all.insert(format!("paths.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.food {
            for (k, v) in ext {
                all.insert(format!("food.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.staff {
            for (k, v) in ext {
                all.insert(format!("staff.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.guests {
            for (k, v) in ext {
                all.insert(format!("guests.{}", k), v.clone());
            }
        }
        if let Some(ref ext) = self.items {
            for (k, v) in ext {
                all.insert(format!("items.{}", k), v.clone());
            }
        }

        all
    }

    /// Check if any extensions exist
    pub fn has_extensions(&self) -> bool {
        self.scenery.is_some()
            || self.animals.is_some()
            || self.buildings.is_some()
            || self.fences.is_some()
            || self.walls.is_some()
            || self.paths.is_some()
            || self.food.is_some()
            || self.staff.is_some()
            || self.guests.is_some()
            || self.items.is_some()
    }

    pub fn len(&self) -> usize {
        let mut len = 0;
        if let Some(habitats) = &self.habitats {
            len += habitats.len();
        }
        if let Some(locations) = &self.locations {
            len += locations.len();
        }
        // Count all entity type extensions
        if let Some(ref ext) = self.scenery {
            len += ext.len();
        }
        if let Some(ref ext) = self.animals {
            len += ext.len();
        }
        if let Some(ref ext) = self.buildings {
            len += ext.len();
        }
        if let Some(ref ext) = self.fences {
            len += ext.len();
        }
        if let Some(ref ext) = self.walls {
            len += ext.len();
        }
        if let Some(ref ext) = self.paths {
            len += ext.len();
        }
        if let Some(ref ext) = self.food {
            len += ext.len();
        }
        if let Some(ref ext) = self.staff {
            len += ext.len();
        }
        if let Some(ref ext) = self.guests {
            len += ext.len();
        }
        if let Some(ref ext) = self.items {
            len += ext.len();
        }
        len
    }
}

#[derive(Deserialize, Debug, Getters)]
#[get = "pub"]
pub struct IconDefinition {
    name: String,
    icon_path: String,
    icon_palette_path: String,
}

// ============================================================================
// Extension System Data Structures
// ============================================================================

/// Extension data for a single legacy entity
#[derive(Deserialize, Debug, Clone, Getters)]
#[get = "pub"]
pub struct EntityExtension {
    /// Base entity to extend (e.g., "legacy.animals.elephant")
    base: String,

    #[serde(default)]
    tags: Vec<String>,

    #[serde(default)]
    attributes: HashMap<String, String>,
}

impl EntityExtension {
    #[cfg(feature = "integration-tests")]
    pub fn new_test(base: String, tags: Vec<String>, attributes: HashMap<String, String>) -> Self {
        EntityExtension { base, tags, attributes }
    }
}

// ============================================================================
// Patch System Data Structures
// ============================================================================

/// Metadata for patch configuration
///
/// Contains file-level error handling and conditional evaluation settings
/// that apply to all patches in a mod definition.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PatchMeta {
    /// File-level on_error directive for error handling
    #[serde(default = "default_on_error")]
    pub on_error: ErrorHandling,

    /// File-level conditions - if these fail, all patches are skipped
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

impl Default for PatchMeta {
    fn default() -> Self {
        PatchMeta {
            on_error: ErrorHandling::Continue,
            condition: None,
        }
    }
}

fn default_on_error() -> ErrorHandling {
    ErrorHandling::Continue
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorHandling {
    Continue,
    Abort,
    AbortMod,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Patch {
    Replace(ReplacePatch),
    Merge(MergePatch),
    Delete(DeletePatch),
    SetPalette(SetPalettePatch),
    SetKey(SetKeyPatch),
    SetKeys(SetKeysPatch),
    AppendValue(AppendValuePatch),
    AppendValues(AppendValuesPatch),
    RemoveKey(RemoveKeyPatch),
    RemoveKeys(RemoveKeysPatch),
    AddSection(AddSectionPatch),
    ClearSection(ClearSectionPatch),
    RemoveSection(RemoveSectionPatch),
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    PatchPriority,
    BasePriority,
}

fn default_merge_mode() -> MergeMode {
    MergeMode::PatchPriority
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OnExists {
    Error,
    Merge,
    Skip,
    Replace,
}

fn default_on_exists() -> OnExists {
    OnExists::Error
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PatchCondition {
    /// Target file for key_exists/value_equals conditions
    /// Required at top-level if using key_exists or value_equals
    /// Optional at patch-level (defaults to the patch's own target)
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub mod_loaded: Option<String>,
    #[serde(default)]
    pub key_exists: Option<KeyCheck>,
    #[serde(default)]
    pub value_equals: Option<ValueCheck>,
    /// Check if a ZTD archive was loaded earlier in the load order
    #[serde(default)]
    pub ztd_loaded: Option<String>,
    /// Check if a legacy entity exists (format: "legacy.{type}.{name}")
    #[serde(default)]
    pub entity_exists: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct KeyCheck {
    pub section: String,
    pub key: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ValueCheck {
    pub section: String,
    pub key: String,
    pub value: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ReplacePatch {
    pub target: String,
    pub source: String,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MergePatch {
    pub target: String,
    pub source: String,
    #[serde(default = "default_merge_mode")]
    pub merge_mode: MergeMode,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeletePatch {
    pub target: String,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

/// Patch operation to change the palette file reference in an animation file
///
/// This operation modifies the palette filename stored inside an animation file's
/// header without changing the animation data itself. This allows mods to swap
/// color palettes for animals, buildings, or other animated sprites.
///
/// # Fields
/// * `target` - Path to the animation file (must have no extension, e.g., "animals/elephant/n")
/// * `palette` - Path to the palette file (.pal) to reference
/// * `condition` - Optional conditions that must be met for the patch to apply
///
/// # Validation
/// - Target file must exist in the resource system
/// - Target file must be an animation file (no extension)
/// - Palette path must end with .pal extension
/// - Palette path must not be empty
///
/// # Example TOML
/// ```toml
/// [patches.update_elephant_colors]
/// operation = "set_palette"
/// target = "animals/elephant/adult/male/n"
/// palette = "animals/elephant/new_colors.pal"
/// ```
///
/// # Errors
/// - Target file not found
/// - Target has extension (not an animation file)
/// - Palette path empty or has wrong extension
/// - Animation file parsing/writing fails
#[derive(Deserialize, Debug, Clone)]
pub struct SetPalettePatch {
    /// Target animation file path (must have no extension)
    pub target: String,
    /// Path to the palette file (.pal) to reference
    pub palette: String,
    /// Optional conditions for applying this patch
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SetKeyPatch {
    pub target: String,
    pub section: String,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SetKeysPatch {
    pub target: String,
    pub section: String,
    pub keys: HashMap<String, String>,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppendValuePatch {
    pub target: String,
    pub section: String,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppendValuesPatch {
    pub target: String,
    pub section: String,
    pub key: String,
    pub values: Vec<String>,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RemoveKeyPatch {
    pub target: String,
    pub section: String,
    pub key: String,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RemoveKeysPatch {
    pub target: String,
    pub section: String,
    pub keys: Vec<String>,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AddSectionPatch {
    pub target: String,
    pub section: String,
    #[serde(default)]
    pub keys: HashMap<String, String>,
    #[serde(default = "default_on_exists")]
    pub on_exists: OnExists,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ClearSectionPatch {
    pub target: String,
    pub section: String,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RemoveSectionPatch {
    pub target: String,
    pub section: String,
    #[serde(default)]
    pub condition: Option<PatchCondition>,
}

// Test helpers for creating test instances
#[cfg(test)]
impl IconDefinition {
    pub fn new_test(name: String, icon_path: String, icon_palette_path: String) -> Self {
        IconDefinition {
            name,
            icon_path,
            icon_palette_path,
        }
    }
}

#[cfg(test)]
impl ModDefinition {
    pub fn new_test(
        habitats: Option<std::collections::HashMap<String, IconDefinition>>,
        locations: Option<std::collections::HashMap<String, IconDefinition>>,
        scenery: Option<std::collections::HashMap<String, EntityExtension>>,
        animals: Option<std::collections::HashMap<String, EntityExtension>>,
        patch_meta: Option<PatchMeta>,
        patches: Option<indexmap::IndexMap<String, Patch>>,
    ) -> Self {
        ModDefinition {
            habitats,
            locations,
            scenery,
            animals,
            buildings: None,
            fences: None,
            walls: None,
            paths: None,
            food: None,
            staff: None,
            guests: None,
            items: None,
            patch_meta,
            patches,
        }
    }
}
#[cfg(test)]
mod mod_loading_tests {
    use crate::mods::{DependencyIdentifier, Version};

    #[test]
    fn test_parse_meta() {
        let meta: super::Meta = toml::from_str(include_str!("../resources/test/meta.toml")).unwrap();
        assert_eq!(meta.name, "my fun mod");
        assert_eq!(meta.description, "a mod full of fun");
        assert_eq!(meta.authors, vec!["Finn".to_string()]);
        assert_eq!(meta.mod_id, "finn.my_fun_mod");
        assert_eq!(meta.version, Version { major: 1, minor: 0, patch: 0 });
        assert_eq!(meta.version.minor, 0);
        assert_eq!(meta.version.patch, 0);
        assert_eq!(meta.link, Some("https://mywebsite.com/myfunmod".to_string()));
        assert_eq!(meta.dependencies.len(), 1);
        assert_eq!(meta.ztd_type, super::ZtdType::Combined);
        let dep = meta.dependencies[0].clone();
        assert_eq!(dep.identifier(), &DependencyIdentifier::ModId("finn.my_other_mod".to_string()));
        assert_eq!(dep.name(), "my other mod");
        assert_eq!(dep.min_version().as_ref().unwrap(), &Version { major: 1, minor: 1, patch: 2 });
        assert!(*dep.optional());
        assert_eq!(dep.ordering(), &super::Ordering::Before);
    }

    #[test]
    fn test_parse_meta_zb() {
        // Test that empty dependency objects are skipped with a warning
        let meta: super::Meta = toml::from_str(include_str!("../resources/test/meta_zb.toml")).unwrap();
        assert_eq!(meta.dependencies.len(), 0);
    }

    #[test]
    fn test_lenient_dependency_parsing() {
        // Test that the parser handles mixed valid/invalid dependencies gracefully
        // This uses inline table syntax where we can have an empty table {}
        let toml_str = r#"
name = "test mod"
description = "test"
authors = ["test"]
mod_id = "test.mod"
version = "1.0.0"
dependencies = [
    {},
    { mod_id = "valid.mod", name = "Valid Mod" }
]
"#;
        let meta: super::Meta = toml::from_str(toml_str).unwrap();
        // Empty dependency should be skipped with a warning, only valid one should remain
        assert_eq!(meta.dependencies.len(), 1);
        assert_eq!(meta.dependencies[0].identifier(), &DependencyIdentifier::ModId("valid.mod".to_string()));
    }

    #[test]
    fn test_parse_meta_legacy() {
        let meta: super::Meta = toml::from_str(include_str!("../resources/test/meta-legacy.toml")).unwrap();
        assert_eq!(meta.name, "my fun mod");
        assert_eq!(meta.description, "a mod full of fun");
        assert_eq!(meta.authors, vec!["Finn".to_string()]);
        assert_eq!(meta.mod_id, "finn.my_fun_mod");
        assert_eq!(meta.version, Version { major: 1, minor: 0, patch: 0 });
        assert_eq!(meta.version.minor, 0);
        assert_eq!(meta.version.patch, 0);
        assert_eq!(meta.link, Some("https://mywebsite.com/myfunmod".to_string()));
        assert_eq!(meta.ztd_type, super::ZtdType::Legacy);
    }

    fn check_moon_location(location: &super::IconDefinition) {
        assert_eq!(location.name, "Moon");
        assert_eq!(location.icon_path, "resources/moon/N");
        assert_eq!(location.icon_palette_path, "resources/moon/moon.pal");
    }

    fn check_swamp_habitat(habitat: &super::IconDefinition) {
        assert_eq!(habitat.name, "Swamp");
        assert_eq!(habitat.icon_path, "resources/swamp/N");
        assert_eq!(habitat.icon_palette_path, "resources/swamp/swamp.pal");
    }

    #[test]
    fn test_parse_location() {
        let mods: super::ModDefinition = toml::from_str(include_str!("../resources/test/example-location.toml")).unwrap();
        let locations = mods.locations.unwrap();
        let location = locations.get("moon").unwrap();
        check_moon_location(location);
    }

    #[test]
    fn test_parse_habitat() {
        let mods: super::ModDefinition = toml::from_str(include_str!("../resources/test/example-habitat.toml")).unwrap();
        let habitats = mods.habitats.unwrap();
        let habitat = habitats.get("swamp").unwrap();
        check_swamp_habitat(habitat);
    }

    #[test]
    fn test_parse_combined() {
        let mods: super::ModDefinition = toml::from_str(include_str!("../resources/test/example-combined.toml")).unwrap();
        let locations = mods.locations.unwrap();
        let habitats = mods.habitats.unwrap();
        let location = locations.get("moon").unwrap();
        check_moon_location(location);
        let location_2 = locations.get("moon2").unwrap();
        check_moon_location(location_2);
        let habitat = habitats.get("swamp").unwrap();
        check_swamp_habitat(habitat);
    }

    #[test]
    fn test_parse_patches() {
        let mod_def: super::ModDefinition = toml::from_str(include_str!("../resources/test/patch.toml")).unwrap();
        let patch_meta = mod_def.patch_meta.expect("patch_meta should be present");
        let patches = mod_def.patches.expect("patches should be present");

        assert_eq!(patches.len(), 10);

        // Test file-level config
        assert_eq!(patch_meta.on_error, super::ErrorHandling::Continue);
        assert!(patch_meta.condition.is_none());

        // Test merge patch (defaults to patch_priority)
        let merge_patch = patches.get("merge_blackbuck_ai").expect("merge_blackbuck_ai patch not found");
        match merge_patch {
            super::Patch::Merge(patch) => {
                assert_eq!(patch.target, "animals/blckbuck.ai");
                assert_eq!(patch.source, "resources/patches/blckbuck.ai");
                assert_eq!(patch.merge_mode, super::MergeMode::PatchPriority);
                assert!(patch.condition.is_none());
            }
            _ => panic!("Expected Merge patch"),
        }

        // Test replace patch
        let replace_patch = patches.get("replace_blackbuck_ai").expect("replace_blackbuck_ai patch not found");
        match replace_patch {
            super::Patch::Replace(patch) => {
                assert_eq!(patch.target, "animals/blckbuck.ai");
                assert_eq!(patch.source, "resources/patches/blckbuck.ai");
                assert!(patch.condition.is_none());
            }
            _ => panic!("Expected Replace patch"),
        }

        // Test delete patch
        let delete_patch = patches.get("remove_old_animal").expect("remove_old_animal patch not found");
        match delete_patch {
            super::Patch::Delete(patch) => {
                assert_eq!(patch.target, "animals/oldanimal.ai");
                assert!(patch.condition.is_none());
            }
            _ => panic!("Expected Delete patch"),
        }

        // Test set_key patch
        let set_key_patch = patches.get("update_resolution").expect("update_resolution patch not found");
        match set_key_patch {
            super::Patch::SetKey(patch) => {
                assert_eq!(patch.target, "config/settings.ini");
                assert_eq!(patch.section, "Graphics");
                assert_eq!(patch.key, "Resolution");
                assert_eq!(patch.value, "1920x1080");
                assert!(patch.condition.is_none());
            }
            _ => panic!("Expected SetKey patch"),
        }

        // Test conditional patch with mod_loaded
        let conditional_patch = patches.get("conditional_mod_loaded").expect("conditional_mod_loaded patch not found");
        match conditional_patch {
            super::Patch::SetKey(patch) => {
                assert_eq!(patch.target, "config/settings.ini");
                assert_eq!(patch.section, "Features");
                assert_eq!(patch.key, "AdvancedAI");
                assert_eq!(patch.value, "true");
                let condition = patch.condition.as_ref().expect("Expected condition");
                assert_eq!(condition.mod_loaded, Some("AdvancedAI".to_string()));
                assert!(condition.key_exists.is_none());
                assert!(condition.value_equals.is_none());
            }
            _ => panic!("Expected SetKey patch"),
        }

        // Test conditional patch with key_exists
        let conditional_patch = patches.get("conditional_key_exists").expect("conditional_key_exists patch not found");
        match conditional_patch {
            super::Patch::SetKey(patch) => {
                assert_eq!(patch.target, "config/settings.ini");
                let condition = patch.condition.as_ref().expect("Expected condition");
                assert!(condition.mod_loaded.is_none());
                let key_check = condition.key_exists.as_ref().expect("Expected key_exists");
                assert_eq!(key_check.section, "Graphics");
                assert_eq!(key_check.key, "AntiAliasing");
                assert!(condition.value_equals.is_none());
            }
            _ => panic!("Expected SetKey patch"),
        }

        // Test conditional patch with value_equals
        let conditional_patch = patches.get("conditional_value_equals").expect("conditional_value_equals patch not found");
        match conditional_patch {
            super::Patch::SetKey(patch) => {
                assert_eq!(patch.target, "config/settings.ini");
                let condition = patch.condition.as_ref().expect("Expected condition");
                assert!(condition.mod_loaded.is_none());
                assert!(condition.key_exists.is_none());
                let value_check = condition.value_equals.as_ref().expect("Expected value_equals");
                assert_eq!(value_check.section, "Graphics");
                assert_eq!(value_check.key, "TextureQuality");
                assert_eq!(value_check.value, "medium");
            }
            _ => panic!("Expected SetKey patch"),
        }

        // Test add_section patch with on_exists
        let add_section_patch = patches.get("add_section_with_on_exists").expect("add_section_with_on_exists patch not found");
        match add_section_patch {
            super::Patch::AddSection(patch) => {
                assert_eq!(patch.target, "config/settings.ini");
                assert_eq!(patch.section, "NewFeature");
                assert_eq!(patch.on_exists, super::OnExists::Merge);
                assert_eq!(patch.keys.get("Enabled"), Some(&"true".to_string()));
                assert_eq!(patch.keys.get("Value"), Some(&"100".to_string()));
                assert!(patch.condition.is_none());
            }
            _ => panic!("Expected AddSection patch"),
        }

        // Test set_palette patch
        let set_palette_patch = patches.get("set_elephant_palette").expect("set_elephant_palette patch not found");
        match set_palette_patch {
            super::Patch::SetPalette(patch) => {
                assert_eq!(patch.target, "animals/elephant/adult/male/n");
                assert_eq!(patch.palette, "animals/elephant/new_palette.pal");
                assert!(patch.condition.is_none());
            }
            _ => panic!("Expected SetPalette patch"),
        }

        // Test set_palette patch with condition
        let conditional_palette = patches.get("conditional_palette_swap").expect("conditional_palette_swap patch not found");
        match conditional_palette {
            super::Patch::SetPalette(patch) => {
                assert_eq!(patch.target, "animals/tiger/adult/n");
                assert_eq!(patch.palette, "resources/tiger_hd.pal");
                let condition = patch.condition.as_ref().expect("Expected condition");
                assert_eq!(condition.mod_loaded, Some("HDTexturesMod".to_string()));
            }
            _ => panic!("Expected SetPalette patch"),
        }

        // Test that patches are ordered correctly (IndexMap preserves insertion order)
        let patch_names: Vec<_> = patches.keys().collect();
        assert_eq!(patch_names[0], "merge_blackbuck_ai");
        assert_eq!(patch_names[1], "replace_blackbuck_ai");
        assert_eq!(patch_names[2], "remove_old_animal");
        assert_eq!(patch_names[3], "update_resolution");
        assert_eq!(patch_names[4], "conditional_mod_loaded");
        assert_eq!(patch_names[5], "conditional_key_exists");
        assert_eq!(patch_names[6], "conditional_value_equals");
        assert_eq!(patch_names[7], "add_section_with_on_exists");
        assert_eq!(patch_names[8], "set_elephant_palette");
        assert_eq!(patch_names[9], "conditional_palette_swap");
    }

    #[test]
    fn test_parse_extensions_nested_tables() {
        let mod_def: super::ModDefinition = toml::from_str(include_str!("../resources/test/extensions.toml")).unwrap();

        // Check scenery extensions
        let scenery = mod_def.scenery.as_ref().expect("scenery should be present");
        assert_eq!(scenery.len(), 2);

        let roof = scenery.get("vondel_greenhouse_roof").expect("vondel_greenhouse_roof not found");
        assert_eq!(roof.base(), "legacy.scenery.vogrhrf1");
        assert_eq!(roof.tags(), &vec!["roof".to_string()]);

        let statue = scenery.get("statue").expect("statue not found");
        assert_eq!(statue.base(), "legacy.scenery.statue");
        assert_eq!(statue.tags(), &vec!["roof".to_string(), "decoration".to_string()]);

        // Check animals extensions
        let animals = mod_def.animals.as_ref().expect("animals should be present");
        assert_eq!(animals.len(), 2);

        let elephant = animals.get("elephant").expect("elephant not found");
        assert_eq!(elephant.base(), "legacy.animals.elephant");
        assert_eq!(elephant.tags(), &vec!["big".to_string()]);

        let lion = animals.get("lion").expect("lion not found");
        assert_eq!(lion.base(), "legacy.animals.lion");
        assert_eq!(lion.tags(), &vec!["big".to_string(), "predator".to_string()]);

        // Check fences extensions
        let fences = mod_def.fences.as_ref().expect("fences should be present");
        assert_eq!(fences.len(), 1);

        let wood = fences.get("wood").expect("wood not found");
        assert_eq!(wood.base(), "legacy.fences.wood");
        assert_eq!(wood.tags(), &vec!["natural".to_string()]);

        // Check buildings extensions
        let buildings = mod_def.buildings.as_ref().expect("buildings should be present");
        assert_eq!(buildings.len(), 1);

        let restaurant = buildings.get("restaurant").expect("restaurant not found");
        assert_eq!(restaurant.base(), "legacy.buildings.restaurant");
        assert_eq!(restaurant.tags(), &vec!["food".to_string()]);

        // Verify extensions() method collects all correctly
        let all_extensions = mod_def.extensions();
        assert_eq!(all_extensions.len(), 6);
        assert!(all_extensions.contains_key("scenery.vondel_greenhouse_roof"));
        assert!(all_extensions.contains_key("scenery.statue"));
        assert!(all_extensions.contains_key("animals.elephant"));
        assert!(all_extensions.contains_key("animals.lion"));
        assert!(all_extensions.contains_key("fences.wood"));
        assert!(all_extensions.contains_key("buildings.restaurant"));
    }
}
