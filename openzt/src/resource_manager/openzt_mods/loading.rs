use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    fmt,
    path::{Path, PathBuf},
    str,
    sync::Mutex,
};

use anyhow::{anyhow, Context};
use openzt_configparser::ini::{Ini, WriteOptions};
use std::sync::LazyLock;
use tracing::{debug, error, info};

use crate::{
    animation::Animation,
    mods,
    resource_manager::{
        lazyresourcemap::add_ztfile,
        openzt_mods::habitats_locations::add_location_or_habitat,
        ztd::ZtdArchive,
        ztfile::{ZTFile, ZTFileType},
    },
};

/// Used to ensure mod_ids don't clash, a mod will not load if an id is already in this map
static MOD_ID_SET: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Tries to add a new mod id to the set, returns false if the mod_id already exists
pub fn add_new_mod_id(mod_id: &str) -> bool {
    let mut binding = MOD_ID_SET.lock().unwrap();
    binding.insert(mod_id.to_string())
}

pub fn get_num_mod_ids() -> usize {
    let binding = MOD_ID_SET.lock().unwrap();
    binding.len()
}

pub fn get_mod_ids() -> Vec<String> {
    let binding = MOD_ID_SET.lock().unwrap();
    binding.iter().cloned().collect()
}

/// Clear the MOD_ID_SET (for integration tests)
#[cfg(feature = "integration-tests")]
pub fn clear_mod_ids_for_tests() {
    MOD_ID_SET.lock().unwrap().clear();
}

/// Result of discovering mods and legacy archives
///
/// Contains both OpenZT mods (with meta.toml) and pure legacy archives (without meta.toml)
/// Only pure legacy archives found in /mods/ directory are included
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    /// OpenZT mods: mod_id -> (archive_name, Meta)
    pub openzt_mods: HashMap<String, (String, mods::Meta)>,
    /// Pure legacy archives in /mods/: (archive_name, path)
    pub pure_legacy_in_mods: Vec<(String, PathBuf)>,
}

impl DiscoveryResult {
    pub fn new() -> Self {
        Self {
            openzt_mods: HashMap::new(),
            pure_legacy_in_mods: Vec::new(),
        }
    }
}

/// Discover all OpenZT mods and pure legacy archives from .ztd archives without loading them
///
/// Returns a DiscoveryResult containing:
/// - OpenZT mods (with meta.toml) from all resource paths
/// - Pure legacy archives (no meta.toml) ONLY from /mods/ directory
///
/// This is used for dependency resolution and load order generation before actual mod loading
pub fn discover_mods(paths: &[String]) -> DiscoveryResult {
    let mut result = DiscoveryResult::new();
    let mods_path = PathBuf::from("./mods");

    // Iterate through resource paths to find .ztd files
    for path_str in paths.iter().rev() {
        let path = PathBuf::from(path_str);

        if !path.exists() {
            continue;
        }

        // Check if this is the /mods/ directory
        let is_mods_dir = path == mods_path
            || path
                .canonicalize()
                .ok()
                .map(|p| p == mods_path.canonicalize().unwrap_or(mods_path.clone()))
                .unwrap_or(false);

        // Read directory entries
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };

        for entry in entries.flatten() {
            let file_path = entry.path();

            // Only process .ztd files (case-insensitive)
            if !file_path.extension().is_some_and(|s| s.eq_ignore_ascii_case("ztd")) {
                continue;
            }

            let archive_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();

            // Try to read meta.toml from the archive
            match read_meta_from_archive(&file_path) {
                Ok(Some(meta)) => {
                    let mod_id = meta.mod_id().to_string();

                    // Skip if we already found this mod (earlier paths take precedence)
                    result.openzt_mods.entry(mod_id.clone()).or_insert_with(|| {
                        let span = tracing::error_span!(
                            "discover_mod",
                            archive_path = %file_path.display().to_string(),
                            mod_id = %mod_id,
                            mod_name = %meta.name()
                        );
                        let _guard = span.enter();

                        info!("Discovered mod: {} ({})", meta.name(), mod_id);
                        (archive_name, meta)
                    });
                }
                Ok(None) => {
                    // Legacy mod (no meta.toml)
                    // Only add to pure_legacy_in_mods if in /mods/ directory
                    if is_mods_dir {
                        // Check if already discovered (earlier paths take precedence)
                        let already_discovered = result.pure_legacy_in_mods.iter().any(|(name, _)| name.to_lowercase() == archive_name.to_lowercase());

                        if !already_discovered {
                            debug!("Discovered pure legacy archive in /mods/: {}", archive_name);
                            result.pure_legacy_in_mods.push((archive_name, file_path));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read meta from  {:?}: Failed to parse meta.toml\n{:#}", file_path, e);
                }
            }
        }
    }

    result
}

/// Read and parse meta.toml from a .ztd archive
///
/// Returns None if no meta.toml exists (legacy mod)
fn read_meta_from_archive(archive_path: &Path) -> anyhow::Result<Option<mods::Meta>> {
    use crate::resource_manager::ztd::ZtdArchive;

    let archive_path_str = archive_path.display().to_string();
    let span = tracing::error_span!("read_meta_from_archive", archive_path = %archive_path_str);
    let _guard = span.enter();

    let mut archive = ZtdArchive::new(archive_path).with_context(|| format!("Failed to open archive: {:?}", archive_path))?;

    // Check if meta.toml exists
    let Ok(meta_file) = archive.by_name("meta.toml") else {
        // No meta.toml = legacy mod
        return Ok(None);
    };

    // Parse meta.toml
    let meta_str = String::try_from(meta_file).with_context(|| format!("Failed to read meta.toml from {:?}", archive_path))?;

    let meta = toml::from_str::<mods::Meta>(&meta_str).with_context(|| format!("Failed to parse meta.toml from {:?}", archive_path))?;

    // Record mod_id in the span for downstream error context
    let mod_id = meta.mod_id().to_string();
    span.record("mod_id", &mod_id);

    Ok(Some(meta))
}

// === Load Order Tracking (for integration tests) ===
#[cfg(feature = "integration-tests")]
#[derive(Debug, Clone)]
pub struct LoadEvent {
    pub mod_id: String,
    pub filename: String,
    pub category: DefFileCategory,
    pub timestamp: std::time::Instant,
}

#[cfg(feature = "integration-tests")]
pub static LOAD_ORDER_TRACKER: LazyLock<Mutex<Vec<LoadEvent>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[cfg(feature = "integration-tests")]
pub fn clear_load_tracker() {
    LOAD_ORDER_TRACKER.lock().unwrap().clear();
}

#[cfg(feature = "integration-tests")]
pub fn get_load_events() -> Vec<LoadEvent> {
    LOAD_ORDER_TRACKER.lock().unwrap().clone()
}

/// Category of definition file based on its contents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefFileCategory {
    NoPatch,   // Habitats/locations only, no patches
    Mixed,     // Both habitats/locations AND patches
    PatchOnly, // Patches only, no habitats/locations
}

/// Classify a definition file based on its contents
fn classify_def_file(mod_def: &mods::ModDefinition) -> DefFileCategory {
    let has_patches = mod_def.patches().as_ref().map(|p| !p.is_empty()).unwrap_or(false);
    let has_other = mod_def.habitats().is_some() || mod_def.locations().is_some();

    match (has_patches, has_other) {
        (false, _) => DefFileCategory::NoPatch,
        (true, true) => DefFileCategory::Mixed,
        (true, false) => DefFileCategory::PatchOnly,
    }
}

// TODO: We should use '/' as separator instead of '.' to match other resource ids
pub fn openzt_base_resource_id(mod_id: &str, resource_type: ResourceType, resource_name: &str) -> String {
    let resource_type_name = resource_type.to_string();
    format!("openzt.mods.{}.{}.{}", mod_id, resource_type_name, resource_name)
}

// TODO: We should use '/' as separator instead of '.' to match other resource ids
pub fn openzt_full_resource_id_path(base_resource_id: &str, file_type: ZTFileType) -> String {
    format!("{}.{}", base_resource_id, file_type)
}

/// Load an OpenZT mod from a file map (shared implementation)
fn load_open_zt_mod_internal(file_map: HashMap<String, Box<[u8]>>, archive_name: &str, _resource: &Path) -> anyhow::Result<mods::ZtdType> {
    let meta_file = file_map.get("meta.toml").ok_or_else(|| anyhow!("meta.toml not found in {}", archive_name))?;

    let meta_str = String::from_utf8_lossy(meta_file.as_ref());
    let meta = toml::from_str::<mods::Meta>(&meta_str).with_context(|| format!("Failed to parse meta.toml in {}", archive_name))?;

    if meta.ztd_type() == &mods::ZtdType::Legacy {
        return Ok(mods::ZtdType::Legacy);
    }

    let mod_id = meta.mod_id().to_string();

    if !add_new_mod_id(&mod_id) {
        return Err(anyhow!("Mod already loaded: {}", mod_id));
    }

    // Register the mod_id to ZTD mapping for ztd_loaded condition
    crate::resource_manager::openzt_mods::ztd_registry::register_mod_ztd(&mod_id, archive_name);

    // Create span for the entire loading process
    let mod_name = meta.name().to_string();
    let span = tracing::error_span!(
        "load_openzt_mod",
        mod_id = %mod_id,
        mod_name = %mod_name,
        archive_name = %archive_name
    );
    let _guard = span.enter();

    info!("Loading OpenZT mod: {} {}", meta.name(), meta.mod_id());

    // Collect all defs/ files and sort alphabetically (case-insensitive)
    let mut def_files: Vec<String> = file_map.keys().filter(|name| name.starts_with("defs/")).cloned().collect();

    // Sort case-insensitively, then by original case for stability
    def_files.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));

    // Helper struct to store parsed file info
    struct DefFileInfo {
        filename: String,
        mod_def: mods::ModDefinition,
        category: DefFileCategory,
    }

    // Pre-parse all files to classify them
    let mut file_infos: Vec<DefFileInfo> = Vec::new();
    for file_name in def_files {
        let mod_def = parse_def(&mod_id, &file_name, &file_map)?;
        let category = classify_def_file(&mod_def);
        file_infos.push(DefFileInfo {
            filename: file_name,
            mod_def,
            category,
        });
    }

    // Sort by category (NoPatch -> Mixed -> PatchOnly), then alphabetically within category
    file_infos.sort_by(|a, b| {
        use DefFileCategory::*;
        let category_order = |cat: &DefFileCategory| match cat {
            NoPatch => 0,
            Mixed => 1,
            PatchOnly => 2,
        };

        category_order(&a.category)
            .cmp(&category_order(&b.category))
            .then_with(|| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()))
            .then_with(|| a.filename.cmp(&b.filename))
    });

    // Process files in sorted order
    for file_info in file_infos {
        info!("Loading {} (category: {:?})", file_info.filename, file_info.category);

        // Track loading order for integration tests
        #[cfg(feature = "integration-tests")]
        {
            let event = LoadEvent {
                mod_id: mod_id.clone(),
                filename: file_info.filename.clone(),
                category: file_info.category,
                timestamp: std::time::Instant::now(),
            };
            LOAD_ORDER_TRACKER.lock().unwrap().push(event);
        }

        // Load habitats/locations first (before patches)
        load_habitats_locations(&mod_id, &file_info.mod_def, &file_map)?;

        // Load extensions
        load_extensions(&mod_id, &file_info.mod_def)?;

        // Then apply patches if present
        if let Some(patches) = file_info.mod_def.patches() {
            let patch_meta = file_info.mod_def.patch_meta().as_ref().cloned().unwrap_or_default();
            info!("Found {} patches in {}", patches.len(), file_info.filename);
            if let Err(e) = super::patches::apply_patches(&patch_meta, patches, &file_map, &mod_id) {
                error!("Failed to apply patches from {}: {}", file_info.filename, e);
                return Err(e);
            }
        }
    }

    Ok(meta.ztd_type().clone())
}

pub fn load_open_zt_mod(archive: &mut ZtdArchive, resource: &Path) -> anyhow::Result<mods::ZtdType> {
    let archive_name = archive.name().to_string();

    // Early exit: check if meta.toml exists in the archive
    let Ok(meta_file) = archive.by_name("meta.toml") else {
        return Ok(mods::ZtdType::Legacy);
    };

    // Build file map from archive
    let mut file_map: HashMap<String, Box<[u8]>> = HashMap::new();

    // Add meta.toml to file map (we already read it for the early check)
    let meta_bytes = String::try_from(meta_file)
        .with_context(|| format!("error reading meta.toml from {}", archive_name))?
        .into_bytes()
        .into_boxed_slice();
    file_map.insert("meta.toml".to_string(), meta_bytes);

    // Read remaining files from archive
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("Error reading zip file at index {} from file {}", i, archive_name))?;

        if file.is_dir() {
            continue;
        }

        let file_name = file.name().to_string();

        // Skip meta.toml since we already added it
        if file_name == "meta.toml" {
            continue;
        }

        let mut file_buffer = vec![0; file.size() as usize].into_boxed_slice();
        file.read_exact(&mut file_buffer).with_context(|| format!("Error reading file: {}", file_name))?;

        file_map.insert(file_name, file_buffer);
    }

    // Call shared implementation
    load_open_zt_mod_internal(file_map, &archive_name, resource)
}

/// Load an OpenZT mod from an in-memory file map (for testing)
#[cfg(feature = "integration-tests")]
pub fn load_open_zt_mod_from_memory(file_map: HashMap<String, Box<[u8]>>, mod_name: &str, resource: &Path) -> anyhow::Result<mods::ZtdType> {
    load_open_zt_mod_internal(file_map, mod_name, resource)
}

pub enum ResourceType {
    Location,
    Habitat,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResourceType::Location => write!(f, "location"),
            ResourceType::Habitat => write!(f, "habitat"),
        }
    }
}

/// Parse a definition file from TOML without side effects
pub fn parse_def(mod_id: &str, file_name: &str, file_map: &HashMap<String, Box<[u8]>>) -> anyhow::Result<mods::ModDefinition> {
    let span = tracing::error_span!("parse_def", mod_id = %mod_id, file_name = %file_name);
    let _guard = span.enter();

    info!("Parsing defs {} from {}", file_name, mod_id);

    let file = file_map
        .get(file_name)
        .with_context(|| format!("Error finding file {} in resource map for mod {}", file_name, mod_id))?;

    let intermediate_string = crate::encoding_utils::decode_game_text(file);

    let defs = toml::from_str::<mods::ModDefinition>(&intermediate_string).with_context(|| format!("Error parsing defs from OpenZT mod: {}", file_name))?;

    info!("Parsed defs: {}", defs.len());

    Ok(defs)
}

/// Load habitats and locations from a ModDefinition into the resource system
pub fn load_habitats_locations(mod_id: &str, mod_def: &mods::ModDefinition, file_map: &HashMap<String, Box<[u8]>>) -> anyhow::Result<()> {
    // Habitats
    if let Some(habitats) = mod_def.habitats() {
        for (habitat_name, habitat_def) in habitats.iter() {
            let base_resource_id = openzt_base_resource_id(mod_id, ResourceType::Habitat, habitat_name);
            load_icon_definition(
                &base_resource_id,
                habitat_def,
                file_map,
                mod_id,
                include_str!("../../../resources/include/infoimg-habitat.ani").to_string(),
            )?;
            add_location_or_habitat(mod_id, habitat_name, &base_resource_id, true)?;
        }
    }

    // Locations
    if let Some(locations) = mod_def.locations() {
        for (location_name, location_def) in locations.iter() {
            let base_resource_id = openzt_base_resource_id(mod_id, ResourceType::Location, location_name);
            load_icon_definition(
                &base_resource_id,
                location_def,
                file_map,
                mod_id,
                include_str!("../../../resources/include/infoimg-location.ani").to_string(),
            )?;
            add_location_or_habitat(mod_id, location_name, &base_resource_id, false)?;
        }
    }

    Ok(())
}

/// Load extensions from a ModDefinition into the extension storage system
pub fn load_extensions(mod_id: &str, mod_def: &mods::ModDefinition) -> anyhow::Result<()> {
    use crate::resource_manager::openzt_mods::extensions;

    let extensions = mod_def.extensions();
    if extensions.is_empty() {
        return Ok(());
    }

    info!("Loading extensions for mod {}", mod_id);

    for (extension_key, entity_extension) in extensions.iter() {
        extensions::add_extension(mod_id.to_string(), extension_key.clone(), entity_extension.clone())?;
    }

    Ok(())
}

/// Legacy function that combines parsing and loading - kept for backwards compatibility
pub fn load_def(mod_id: &str, file_name: &str, file_map: &HashMap<String, Box<[u8]>>) -> anyhow::Result<mods::ModDefinition> {
    let defs = parse_def(mod_id, file_name, file_map)?;
    load_habitats_locations(mod_id, &defs, file_map)?;
    Ok(defs)
}

fn load_icon_definition(
    base_resource_id: &str,
    icon_definition: &mods::IconDefinition,
    file_map: &HashMap<String, Box<[u8]>>,
    mod_id: &str,
    base_config: String,
) -> anyhow::Result<()> {
    let icon_file = file_map.get(icon_definition.icon_path()).with_context(|| {
        format!(
            "Error loading openzt mod {}, cannot find file {} for icon_def {}",
            mod_id,
            icon_definition.icon_path(),
            icon_definition.name()
        )
    })?;

    let icon_file_palette = file_map.get(icon_definition.icon_palette_path()).with_context(|| {
        format!(
            "Error loading openzt mod {}, cannot find file {} for icon_def {}",
            mod_id,
            icon_definition.icon_palette_path(),
            icon_definition.name()
        )
    })?;

    let palette_file_name = openzt_full_resource_id_path(base_resource_id, ZTFileType::Palette);
    let palette_ztfile = ZTFile::builder()
        .file_name(palette_file_name.clone())
        .file_size(icon_file_palette.len() as u32)
        .type_(ZTFileType::Palette)
        .raw_data(icon_file_palette.clone())
        .build();
    add_ztfile(Path::new("zip::./openzt.ztd"), palette_file_name.clone(), palette_ztfile)?;

    let mut animation = Animation::parse(icon_file)?;
    animation.set_palette_filename(palette_file_name.clone());
    let (new_animation_bytes, icon_size) = animation.write()?;

    let new_icon_file = new_animation_bytes.into_boxed_slice();

    let mut ani_cfg = Ini::new_cs();
    ani_cfg.set_comment_symbols(&[';', '#', ':']);
    ani_cfg.read(base_config).map_err(|s| anyhow!("Error reading ini: {}", s))?;

    if ani_cfg
        .set("animation", "dir1", Some(openzt_full_resource_id_path(base_resource_id, ZTFileType::Animation)))
        .is_none()
    {
        return Err(anyhow!("Error setting dir1 for ani"));
    }

    let mut write_options = WriteOptions::default();
    write_options.space_around_delimiters = true;
    write_options.blank_lines_between_sections = 1;
    let new_string = ani_cfg.pretty_writes(&write_options);
    info!("New ani: \n{}", new_string);
    let file_size = new_string.len() as u32;
    let file_name = openzt_full_resource_id_path(base_resource_id, ZTFileType::Ani);

    let Ok(new_c_string) = CString::new(new_string) else {
        return Err(anyhow!(
            "Error loading openzt mod {} when converting .ani to CString after modifying {}",
            mod_id,
            file_name
        ));
    };

    let ztfile = ZTFile::builder()
        .file_name(file_name.clone())
        .file_size(file_size)
        .type_(ZTFileType::Ani)
        .cstring_data(new_c_string)
        .build();

    add_ztfile(Path::new("zip::./openzt.ztd"), file_name, ztfile)?;

    let animation_file_name = openzt_full_resource_id_path(base_resource_id, ZTFileType::Animation);
    let animation_ztfile = ZTFile::builder()
        .file_name(animation_file_name.clone())
        .file_size(icon_size as u32)
        .type_(ZTFileType::Animation)
        .raw_data(new_icon_file)
        .build();

    add_ztfile(Path::new("zip::./openzt.ztd"), animation_file_name.clone(), animation_ztfile)?;

    let palette_file_name = openzt_full_resource_id_path(base_resource_id, ZTFileType::Palette);
    let palette_ztfile = ZTFile::builder()
        .file_name(palette_file_name.clone())
        .file_size(icon_file_palette.len() as u32)
        .type_(ZTFileType::Palette)
        .raw_data(icon_file_palette.clone())
        .build();
    add_ztfile(Path::new("zip::./openzt.ztd"), palette_file_name, palette_ztfile)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    /// Helper function to create a minimal IconDefinition for testing
    fn create_test_icon_def() -> mods::IconDefinition {
        mods::IconDefinition::new_test("test".to_string(), "test/icon".to_string(), "test/icon.pal".to_string())
    }

    /// Helper function to create a minimal Patch for testing
    fn create_test_patch() -> mods::Patch {
        mods::Patch::Delete(mods::DeletePatch {
            target: "test.ai".to_string(),
            condition: None,
        })
    }

    #[test]
    fn test_classify_nopatch_file() {
        let mut habitats = HashMap::new();
        habitats.insert("savanna".to_string(), create_test_icon_def());

        let mod_def = mods::ModDefinition::new_test(Some(habitats), None, None, None, None, None);

        assert_eq!(classify_def_file(&mod_def), DefFileCategory::NoPatch);
    }

    #[test]
    fn test_classify_mixed_file() {
        let mut habitats = HashMap::new();
        habitats.insert("savanna".to_string(), create_test_icon_def());

        let mut patches = IndexMap::new();
        patches.insert("patch1".to_string(), create_test_patch());

        let mod_def = mods::ModDefinition::new_test(Some(habitats), None, None, None, None, Some(patches));

        assert_eq!(classify_def_file(&mod_def), DefFileCategory::Mixed);
    }

    #[test]
    fn test_classify_patchonly_file() {
        let mut patches = IndexMap::new();
        patches.insert("patch1".to_string(), create_test_patch());

        let mod_def = mods::ModDefinition::new_test(None, None, None, None, None, Some(patches));

        assert_eq!(classify_def_file(&mod_def), DefFileCategory::PatchOnly);
    }

    #[test]
    fn test_classify_empty_file() {
        let mod_def = mods::ModDefinition::new_test(None, None, None, None, None, None);

        // Empty file is treated as NoPatch
        assert_eq!(classify_def_file(&mod_def), DefFileCategory::NoPatch);
    }

    #[test]
    fn test_classify_empty_patches_collection() {
        let patches = IndexMap::new(); // Empty but present

        let mod_def = mods::ModDefinition::new_test(None, None, None, None, None, Some(patches));

        // Empty patches collection should not count as "has patches"
        assert_eq!(classify_def_file(&mod_def), DefFileCategory::NoPatch);
    }

    #[test]
    fn test_case_insensitive_sorting() {
        let mut files = vec![
            "defs/ZEBRA.toml".to_string(),
            "defs/animal.toml".to_string(),
            "defs/Elephant.toml".to_string(),
            "defs/bear.toml".to_string(),
        ];

        files.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));

        assert_eq!(files, vec!["defs/animal.toml", "defs/bear.toml", "defs/Elephant.toml", "defs/ZEBRA.toml",]);
    }
}
