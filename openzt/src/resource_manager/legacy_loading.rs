use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str,
    sync::{Arc, Mutex},
};

use openzt_configparser::ini::Ini;
use std::sync::LazyLock;
use regex::Regex;
use tracing::{debug, error, info, trace, warn};
use walkdir::WalkDir;

use super::ztd::ZtdArchive;
use crate::{
    encoding_utils::decode_game_text,
    mods,
    resource_manager::{
        handlers::{get_handlers, RunStage},
        lazyresourcemap::{add_lazy, check_file_loaded, create_empty_resource, get_file, get_file_names, get_num_resources, mark_disabled_ztd_file},
        openzt_mods::{get_num_mod_ids, legacy_attributes::{add_legacy_entity, LegacyEntityAttributes, LegacyEntityType, SubtypeAttributes}, load_open_zt_mod, ztd_registry::ZtdLoadStatus},
        ztfile::ZTFileType,
    },
};

pub const OPENZT_DIR0: &str = "openzt_resource";

// Note: We are excluding ztat* files until we need to override anything inside them, as they have a rediculous amount of files
fn get_ztd_resources(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut resources = Vec::new();
    if !dir.is_dir() {
        return resources;
    }
    let walker = WalkDir::new(dir).follow_links(true).max_depth(if recursive { 0 } else { 1 });
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                error!("Error walking directory: {}", e);
                continue;
            }
        };
        let Some(filename) = entry.file_name().to_str() else {
            error!("Error getting filename: {:?}", entry);
            continue;
        };
        if filename.to_lowercase().ends_with(".ztd") && !filename.starts_with("ztat") {
            resources.push(entry.path().to_path_buf());
        }
    }
    resources
}

pub fn load_resources(
    paths: Vec<String>,
    mod_order: &[String],
    discovered_mods: &HashMap<String, (String, mods::Meta)>,
    disabled_mods: &[String],
    disabled_ztds: &[String],
) {
    use std::time::Instant;
    use std::collections::HashMap;

    let now = Instant::now();
    let mut resource_count = 0;

    // Build a mapping from mod_id to .ztd file path for ordered loading
    // Also categorize mods into legacy vs OpenZT (including mixed/legacy ztd_type)
    let mut mod_to_path: HashMap<String, PathBuf> = HashMap::new();
    let mut legacy_resources: Vec<PathBuf> = Vec::new();

    paths.iter().rev().for_each(|path| {
        let resources = get_ztd_resources(Path::new(path), false);
        resources.iter().for_each(|resource| {
            let file_name = resource.file_name().and_then(|n| n.to_str()).unwrap_or_default();

            // Check if this is an OpenZT mod by looking up in discovered_mods
            let is_openzt_mod = discovered_mods.values().any(|(archive_name, _)| {
                archive_name == file_name
            });

            if is_openzt_mod {
                // OpenZT mod - find the mod_id and check ztd_type
                for (mod_id, (archive_name, meta)) in discovered_mods.iter() {
                    if archive_name == file_name {
                        // Skip disabled mods entirely
                        if disabled_mods.contains(mod_id) {
                            info!("Skipping disabled OpenZT mod archive: {} (mod_id: {})", file_name, mod_id);
                            break;
                        }

                        let ztd_type = meta.ztd_type();
                        match ztd_type {
                            mods::ZtdType::Combined => {
                                // Combined mods: handle_ztd() processes both OpenZT AND legacy in a single call
                                // Only add to mod_to_path for ordered loading
                                mod_to_path.entry(mod_id.clone()).or_insert_with(|| resource.clone());
                            }
                            mods::ZtdType::Legacy => {
                                // Legacy-only ztd_type - treat as pure legacy
                                legacy_resources.push(resource.clone());
                            }
                            mods::ZtdType::Openzt => {
                                // Pure OpenZT mod - add to ordered loading only
                                mod_to_path.entry(mod_id.clone()).or_insert_with(|| resource.clone());
                            }
                        }
                        break;
                    }
                }
            } else {
                // True legacy mod (no meta.toml)
                legacy_resources.push(resource.clone());
            }
        });
    });

    // Load legacy mods FIRST (before OpenZT mods)
    // This allows OpenZT mods to patch and modify legacy mod behavior
    for resource in legacy_resources {
        trace!("Loading legacy resource: {}", resource.display());
        let file_name = resource.to_str().unwrap_or_default().to_lowercase();
        match handle_ztd(&resource, disabled_ztds) {
            Ok(count) => resource_count += count,
            Err(err) => {
                error!("Error loading ztd: {}:\n{:#}", file_name, err);
            }
        }
    }

    // Then load OpenZT mods in the specified dependency-resolved order
    for mod_id in mod_order {
        if let Some(resource) = mod_to_path.get(mod_id) {
            info!("Loading ordered mod '{}' from: {}", mod_id, resource.display());
            let file_name = resource.to_str().unwrap_or_default().to_lowercase();
            match handle_ztd(resource, disabled_ztds) {
                Ok(count) => resource_count += count,
                Err(err) => {
                    error!("Error loading ztd: {}:\n{:#}", file_name, err);
                }
            }
        } else {
            warn!("Mod '{}' in load order but not found in resource paths", mod_id);
        }
    }

    let elapsed = now.elapsed();
    info!(
        "Loaded {} mods and {} ({}) resources in: {:.2?}",
        get_num_mod_ids(),
        get_num_resources(),
        resource_count,
        elapsed
    );

    let now = Instant::now();

    info!("Running BeforeOpenZTMods handlers");
    for handler in get_handlers().iter() {
        if handler.stage() == RunStage::BeforeOpenZTMods {
            get_file_names().into_iter().for_each(|file| {
                handler.handle(&file);
            });
        }
    }

    // Patches are now applied per-mod in handle_ztd()
    // See handle_ztd() for patch orchestration

    info!("Running AfterOpenZTMods handlers");
    for handler in get_handlers().iter() {
        if handler.stage() == RunStage::AfterOpenZTMods {
            get_file_names().into_iter().for_each(|file| {
                handler.handle(&file);
            });
        }
    }

    let mut filtered_files = Vec::new();

    get_file_names().into_iter().for_each(|file| {
        let extension = Path::new(&file).extension().unwrap_or_default().to_ascii_lowercase();
        match extension.to_str().unwrap_or_default() {
            "uca" | "ucs" | "ucb" => filtered_files.push(file),
            "cfg" => {
                let inner_filtered = parse_cfg(&file);
                filtered_files.extend(inner_filtered);
            }
            _ => {}
        }
    });

    info!("Loaded {} filtered files", filtered_files.len());

    info!("Running AfterFiltering handlers");
    for handler in get_handlers().iter() {
        if handler.stage() == RunStage::AfterFiltering {
            filtered_files.clone().into_iter().for_each(|file| {
                handler.handle(&file);
            });
        }
    }

    let elapsed = now.elapsed();
    info!("Extra handling took an extra: {:.2?}", elapsed);
}

fn handle_ztd(resource: &Path, disabled_ztds: &[String]) -> anyhow::Result<i32> {
    let mut zip = ZtdArchive::new(resource)?;
    let ztd_filename = resource
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();

    // Check if this ZTD is disabled
    let is_disabled = disabled_ztds
        .iter()
        .any(|d| d.to_lowercase() == ztd_filename);

    // Register this ZTD in the load order BEFORE loading
    let status = if is_disabled {
        ZtdLoadStatus::Disabled
    } else {
        ZtdLoadStatus::Enabled
    };
    crate::resource_manager::openzt_mods::ztd_registry::register_ztd(&ztd_filename, status);

    let ztd_type = load_open_zt_mod(&mut zip, resource)?;

    if ztd_type == mods::ZtdType::Openzt {
        return Ok(0);
    }

    // Add span for legacy loading - provides archive context for any errors
    let span = tracing::info_span!("handle_ztd", archive_name = %ztd_filename, disabled = is_disabled);
    let _guard = span.enter();

    let archive = Arc::new(Mutex::new(zip));

    if is_disabled {
        info!("Processing DISABLED ZTD '{}'", ztd_filename);
        let mut added_count = 0;
        let mut skipped_count = 0;

        archive
            .lock()
            .unwrap()
            .file_names()
            .filter(|s| !s.ends_with("/"))
            .for_each(|file_name| {
                let lowercase_name = file_name.to_lowercase();

                // Check if already loaded
                if check_file_loaded(&lowercase_name) {
                    debug!(
                        "File '{}' already loaded, skipping from disabled ZTD",
                        lowercase_name
                    );
                    skipped_count += 1;
                    return;
                }

                // Check file extension
                let path = Path::new(&file_name);
                let extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();

                match extension.as_str() {
                    "cfg" | "uca" | "ucb" | "ucs" => {
                        // Add empty resource for these types
                        if let Ok(file_type) = ZTFileType::try_from(path) {
                            if let Err(e) =
                                create_empty_resource(file_name.to_string(), file_type)
                            {
                                error!(
                                    "Failed to create empty resource for '{}': {}",
                                    file_name, e
                                );
                            } else {
                                // Mark as disabled ZTD file for potential error logging later
                                mark_disabled_ztd_file(&lowercase_name);
                                debug!(
                                    "Added empty resource for '{}' from disabled ZTD",
                                    file_name
                                );
                                added_count += 1;
                            }
                        }
                    }
                    _ => {
                        // Track unsupported files from disabled ZTDs
                        // Error will only be logged if vanilla actually tries to load them
                        mark_disabled_ztd_file(&lowercase_name);
                        debug!(
                            "File '{}' from disabled ZTD has unsupported type - will log error if vanilla loads it",
                            file_name
                        );
                        added_count += 1; // Count as "added" for tracking purposes
                    }
                }
            });

        info!(
            "Disabled ZTD '{}': added {} empty resources, skipped {} already loaded",
            ztd_filename, added_count, skipped_count
        );
        Ok(added_count)
    } else {
        // Normal loading for enabled ZTDs
        let mut load_count = 0;
        archive
            .lock()
            .unwrap()
            .file_names()
            .filter(|s| !s.ends_with("/"))
            .for_each(|file_name| {
                add_lazy(file_name.to_string(), archive.clone());
                load_count += 1;
            });
        Ok(load_count)
    }
}

fn parse_cfg(file_name: &String) -> Vec<String> {
    if let Some(legacy_cfg) = get_legacy_cfg_type(file_name) {
        trace!("Legacy cfg: {} {:?}", file_name, legacy_cfg.cfg_type);

        let Some((_archive_name, file)) = get_file(file_name) else {
            error!("Error getting file: {}", file_name);
            return Vec::new();
        };
        let mut ini = Ini::new_cs();
        ini.set_comment_symbols(&[';', '#', ':']);
        let input_string = crate::encoding_utils::decode_game_text(&file);
        if let Err(e) = ini.read(input_string) {
            error!("Error reading ini {}: {}", file_name, e);
            return Vec::new();
        }

        // Extract entity attributes from .ai files for supported types
        if let Some(entity_type) = LegacyEntityType::from_legacy_cfg_type(&legacy_cfg.cfg_type) {
            extract_legacy_entities(&ini, entity_type);
        }

        match legacy_cfg.cfg_type {
            LegacyCfgType::Ambient => parse_simple_cfg(&ini, "ambient"),
            LegacyCfgType::Animal => parse_simple_cfg(&ini, "animals"), //parse_subtypes_cfg(&ini, "animals"),
            LegacyCfgType::Building => parse_simple_cfg(&ini, "building"),
            LegacyCfgType::Fence => parse_simple_cfg(&ini, "fences"),  //parse_subtypes_cfg(&ini, "fences"),
            LegacyCfgType::Filter => parse_simple_cfg(&ini, "filter"), //parse_subtypes_cfg(&ini, "filter"),
            LegacyCfgType::Food => parse_simple_cfg(&ini, "food"),
            LegacyCfgType::Freeform => parse_simple_cfg(&ini, "freeform"),
            // LegacyCfgType::Fringe => Vec::new(),
            LegacyCfgType::Guest => parse_simple_cfg(&ini, "guest"),
            // LegacyCfgType::Help => Vec::new(),
            LegacyCfgType::Item => parse_simple_cfg(&ini, "items"),
            LegacyCfgType::Path => parse_simple_cfg(&ini, "paths"),
            LegacyCfgType::Rubble => parse_simple_cfg(&ini, "other"),
            // LegacyCfgType::Scenario => Vec::new(),
            LegacyCfgType::Scenery => {
                let mut results = parse_simple_cfg(&ini, "objects");
                results.append(&mut parse_simple_cfg(&ini, "foliage"));
                results.append(&mut parse_simple_cfg(&ini, "other"));
                results
            }
            LegacyCfgType::Staff => parse_simple_cfg(&ini, "staff"), //parse_subtypes_cfg(&ini, "staff"),
            LegacyCfgType::Tile => Vec::new(),
            LegacyCfgType::Wall => parse_simple_cfg(&ini, "tankwall"), //parse_subtypes_cfg(&ini, "tankwall"),
            // LegacyCfgType::Expansion => Vec::new(),
            // LegacyCfgType::Show => Vec::new(),
            // LegacyCfgType::Tank => Vec::new(),
            // LegacyCfgType::UIInfoImage => Vec::new(),
            // LegacyCfgType::Economy => Vec::new(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    }
}

fn parse_simple_cfg(file: &Ini, section_name: &str) -> Vec<String> {
    let mut results = Vec::new();
    if let Some(section) = file.get_map().unwrap_or_default().get(section_name) {
        for (_, value) in section.iter() {
            if let Some(value) = value {
                // If there are multiple values, we take the last one, this occurs only once in vanilla ZT and the values are equal.
                //  Unclear what happens in vanilla ZT if the values are different.
                match value.len().cmp(&1) {
                    std::cmp::Ordering::Equal => results.push(value[0].clone()),
                    std::cmp::Ordering::Greater => results.push(value[value.len() - 1].clone()),
                    _ => {}
                }
            };
        }
    }
    results
}

/// Extract entity attributes by loading each .ai file listed in the .cfg
/// Also extracts subtype information from the .cfg file itself
fn extract_legacy_entities(cfg: &Ini, entity_type: LegacyEntityType) {
    let section_name = entity_type.section_name();

    // Get the INI map to avoid temporary value issues
    let Some(map) = cfg.get_map() else {
        return;
    };

    // Get the section from the INI file
    let Some(_section) = map.get(section_name) else {
        return;
    };

    // For scenery, we need to check multiple sections
    let mut sections_to_check = vec![section_name];
    if entity_type == LegacyEntityType::Scenery {
        sections_to_check.push("foliage");
        sections_to_check.push("other");
    }

    for section_name in sections_to_check {
        let Some(section) = map.get(section_name) else {
            continue;
        };

        // First, scan for any subtype definitions in the .cfg
        // Format: [entityname/subtypes]
        let mut entity_subtypes: HashMap<String, Vec<String>> = HashMap::new();

        for (cfg_section_name, _) in map.iter() {
            if let Some(entity_name) = cfg_section_name.strip_suffix("/subtypes") {
                trace!("Found subtype section for entity: {}", entity_name);
                if let Some(subtype_section) = map.get(cfg_section_name) {
                    let subtypes = subtype_section.keys().cloned().collect::<Vec<String>>();
                    if !subtypes.is_empty() {
                        entity_subtypes.insert(entity_name.to_string(), subtypes);
                    }
                }
            }
        }

        // Now process the main section entries
        for (entity_name, ai_file_paths) in section.iter() {
            if let Some(ai_paths_vec) = ai_file_paths {
                if let Some(ai_path) = ai_paths_vec.first() {
                    let ai_path = ai_path.trim().trim_matches('"');

                    // Load and parse the .ai file
                    if let Some((_archive, ai_file)) = get_file(ai_path) {
                        let mut ai_ini = Ini::new_cs();
                        ai_ini.set_comment_symbols(&[';', '#', ':']);
                        let ai_content = decode_game_text(&ai_file);

                        if ai_ini.read(ai_content).is_ok() {
                            match LegacyEntityAttributes::parse_from_ini(
                                entity_name.clone(), &ai_ini, entity_type
                            ) {
                                Ok(mut attrs) => {
                                    // If we have subtype information from the .cfg, validate/merge it
                                    if let Some(subtypes) = entity_subtypes.get(entity_name) {
                                        // Ensure all declared subtypes exist in the attributes
                                        for subtype in subtypes {
                                            if !attrs.subtype_attributes.contains_key(subtype) {
                                                // Add empty entry for this subtype
                                                attrs.subtype_attributes.insert(
                                                    subtype.clone(),
                                                    SubtypeAttributes {
                                                        subtype: subtype.clone(),
                                                        name_id: None,
                                                    }
                                                );
                                            }
                                        }
                                    }

                                    if let Err(e) = add_legacy_entity(entity_type, entity_name.clone(), attrs) {
                                        warn!(
                                            "Failed to register legacy entity '{}': {}",
                                            entity_name, e
                                        );
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to parse attributes from '{}': {}",
                                        ai_path, e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum LegacyCfgType {
    Ambient,
    Animal,
    Building,
    Fence,
    Filter,
    Food,
    Freeform,
    Fringe,
    Guest,
    Help,
    Item,
    Path,
    Rubble,
    Scenario,
    Scenery,
    Staff,
    Tile,
    Wall,
    Expansion,
    Show,
    Tank,
    UIInfoImage,
    Economy,
}

#[derive(Debug)]
struct LegacyCfg {
    cfg_type: LegacyCfgType,
    file_name: String,
}

fn map_legacy_cfg_type(file_type_str: &str, file_name: String) -> Result<LegacyCfg, String> {
    match file_type_str {
        "ambient" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Ambient,
            file_name,
        }),
        "animal" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Animal,
            file_name,
        }),
        "bldg" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Building,
            file_name,
        }),
        "fences" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Fence,
            file_name,
        }),
        "filter" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Filter,
            file_name,
        }),
        "food" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Food,
            file_name,
        }),
        "free" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Freeform,
            file_name,
        }),
        "fringe" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Fringe,
            file_name,
        }),
        "guests" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Guest,
            file_name,
        }),
        "help" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Help,
            file_name,
        }),
        "items" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Item,
            file_name,
        }),
        "paths" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Path,
            file_name,
        }),
        "rubble" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Rubble,
            file_name,
        }),
        "scenar" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Scenario,
            file_name,
        }),
        "scener" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Scenery,
            file_name,
        }),
        "staff" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Staff,
            file_name,
        }),
        "tile" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Tile,
            file_name,
        }),
        "twall" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Wall,
            file_name,
        }),
        "xpac" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Expansion,
            file_name,
        }),
        "shows" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Show,
            file_name,
        }),
        "tanks" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Tank,
            file_name,
        }),
        "ui/infoimg" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::UIInfoImage,
            file_name,
        }),
        "economy" => Ok(LegacyCfg {
            cfg_type: LegacyCfgType::Economy,
            file_name,
        }),
        _ => Err(format!("Unknown legacy cfg type: {}", file_type_str)),
    }
}

static LEGACY_CFG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^((ambient|animal|bldg|fences|filter|food|free|fringe|guests|help|items|paths|rubble|scenar|scener|staff|tile|twall|xpac)[\w\-. ]*?\.cfg)|((shows|tanks|ui\/infoimg|economy)\.cfg)$")
        .unwrap()
});

fn get_legacy_cfg_type(file_name: &str) -> Option<LegacyCfg> {
    let capture = LEGACY_CFG_REGEX.captures(file_name)?;
    match capture.iter().collect::<Vec<_>>().as_slice() {
        [_, Some(file_name), Some(file_type), None, None] => map_legacy_cfg_type(file_type.as_str(), file_name.as_str().to_string()).ok(),
        [_, None, None, Some(file_name), Some(file_type)] => map_legacy_cfg_type(file_type.as_str(), file_name.as_str().to_string()).ok(),
        _ => None,
    }
}

/// Load legacy entities from the game's installed .cfg files for integration testing.
///
/// This function reads legacy .cfg files directly from the game directory and extracts
/// entity attributes. It's designed to be called from integration tests to ensure
/// legacy entity data is available for testing.
///
/// # Returns
/// * `Ok(count)` if legacy loading succeeded, where count is the number of .cfg files processed
/// * `Err(_)` if there was an error reading or parsing the .cfg files, or if no .cfg files were found
#[cfg(feature = "integration-tests")]
pub fn load_legacy_entities_for_tests() -> anyhow::Result<usize> {
    use std::fs;

    // Get the Zoo Tycoon installation directory
    let zt_dir = std::env::var("ZOO Tycoon_DIR").unwrap_or_else(|_| {
        // Default installation path
        "C:\\Program Files (x86)\\Microsoft Games\\Zoo Tycoon".to_string()
    });

    info!("Loading legacy entities from Zoo Tycoon directory: {}", zt_dir);

    // List of .cfg files to process for legacy entity extraction
    let cfg_files = vec![
        "animal.cfg",
        "bldg.cfg",
        "fences.cfg",
        "food.cfg",
        "guests.cfg",
        "items.cfg",
        "paths.cfg",
        "scener.cfg",
        "staff.cfg",
        "twall.cfg",
    ];

    let mut loaded_count = 0;

    for cfg_file in cfg_files {
        let cfg_path = std::path::Path::new(&zt_dir).join(cfg_file);

        if !cfg_path.exists() {
            continue; // Skip silently
        }

        info!("Processing {}", cfg_file);

        // Read the .cfg file
        let cfg_content = fs::read_to_string(&cfg_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", cfg_file, e))?;

        // Parse the INI content
        let mut ini = Ini::new_cs();
        ini.set_comment_symbols(&[';', '#', ':']);
        if ini.read(cfg_content).is_err() {
            warn!("Failed to parse {}", cfg_file);
            continue;
        }

        // Determine the legacy entity type from the filename
        if let Some(legacy_cfg) = get_legacy_cfg_type(cfg_file) {
            if let Some(entity_type) = LegacyEntityType::from_legacy_cfg_type(&legacy_cfg.cfg_type) {
                // Extract legacy entities from this .cfg file
                extract_legacy_entities(&ini, entity_type);
                loaded_count += 1;
            }
        }
    }

    // Return error if no .cfg files were found (so test attributes will be added)
    if loaded_count == 0 {
        return Err(anyhow::anyhow!("No legacy .cfg files found in {}", zt_dir));
    }

    info!("Loaded legacy entities from {} .cfg files", loaded_count);
    Ok(loaded_count)
}

/// Load legacy entities from test .cfg files for integration testing.
///
/// This function is called after test .cfg and .ai files have been added to the resource
/// system. It triggers the actual legacy loading code path to parse those files.
#[cfg(feature = "integration-tests")]
pub fn load_legacy_entities_from_test_files() -> anyhow::Result<()> {
    info!("Loading legacy entities from test .cfg files...");

    // Get file names from resource system that match legacy .cfg pattern
    let cfg_files = vec![
        "animal.cfg",
        "bldg.cfg",
        "fences.cfg",
        "guests.cfg",
        "items.cfg",
        "staff.cfg",
        "twall.cfg",
    ];

    let mut loaded_count = 0;

    for cfg_file in cfg_files {
        // Check if file exists in resource system
        if let Some((filename, data)) = crate::resource_manager::lazyresourcemap::get_file(cfg_file) {
            // Parse the .cfg file
            let input_string = crate::encoding_utils::decode_game_text(&data);
            let mut ini = Ini::new_cs();
            ini.set_comment_symbols(&[';', '#', ':']);

            if ini.read(input_string).is_err() {
                warn!("Failed to parse test file: {}", cfg_file);
                continue;
            }

            // Determine the legacy entity type from the filename
            if let Some(legacy_cfg) = get_legacy_cfg_type(cfg_file) {
                if let Some(entity_type) = LegacyEntityType::from_legacy_cfg_type(&legacy_cfg.cfg_type) {
                    // Extract legacy entities from this .cfg file
                    extract_legacy_entities(&ini, entity_type);
                    loaded_count += 1;
                }
            }
        }
    }

    info!("Loaded legacy entities from {} test .cfg files", loaded_count);
    Ok(())
}
