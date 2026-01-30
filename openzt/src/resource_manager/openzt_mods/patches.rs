use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str;

use anyhow::{self, Context};
use openzt_configparser::ini::{Ini, MergeMode as IniMergeMode};
use tracing::{error, info, warn};

use crate::{
    animation::Animation,
    mods::{
        AddSectionPatch,
        AppendValuePatch,
        AppendValuesPatch,
        ClearSectionPatch,
        DeletePatch,
        ErrorHandling,
        MergeMode,
        MergePatch,
        OnExists,
        Patch,
        PatchCondition,
        PatchMeta,
        RemoveKeyPatch,
        RemoveKeysPatch,
        RemoveSectionPatch,
        ReplacePatch,
        SetKeyPatch,
        SetKeysPatch,
        SetPalettePatch,
    },
    resource_manager::{
        lazyresourcemap::{add_ztfile, add_ztfile_from_memory, check_file, get_file, remove_resource},
        openzt_mods::{get_mod_ids, habitats_locations::{get_habitat_id, get_location_id}, legacy_attributes::{get_legacy_attribute_with_subtype, LegacyEntityType}},
        ztfile::{modify_ztfile_as_animation, ZTFile, ZTFileType},
    },
    string_registry::get_string_from_registry,
};

// ============================================================================
// Variable Substitution System
// ============================================================================

/// Variable types that can be substituted in patch values
#[derive(Debug, PartialEq)]
enum VariableType {
    Habitats,
    Locations,
    Strings,
    Legacy,
}

/// Parsed variable reference from {variable} syntax
#[derive(Debug)]
struct ParsedVariable {
    var_type: VariableType,
    mod_id: Option<String>,  // None = current mod
    identifier: String,
    legacy_parts: Option<LegacyVariableParts>,  // NEW: For legacy entity attributes
}

/// Parsed parts for legacy entity variable references
#[derive(Debug)]
struct LegacyVariableParts {
    entity_type: LegacyEntityType,
    entity_name: String,
    subtype: Option<String>,  // NEW: Optional subtype
    attribute: String,
}

/// Context for variable substitution during patch application
pub struct SubstitutionContext {
    pub current_mod_id: String,
}

/// Parse variable syntax: "habitats.moon" or "lunar.habitats.crater" or "string.9500"
///
/// # Arguments
/// * `var_str` - The variable string content (without curly braces)
///
/// # Returns
/// * `Ok(ParsedVariable)` - Successfully parsed variable
/// * `Err` - Invalid syntax
///
/// # Examples
/// * "habitats.swamp" → ParsedVariable { var_type: Habitat, mod_id: None, identifier: "swamp" }
/// * "lunar.locations.moon" → ParsedVariable { var_type: Location, mod_id: Some("lunar"), identifier: "moon" }
/// * "string.9500" → ParsedVariable { var_type: String, mod_id: None, identifier: "9500" }
fn parse_variable(var_str: &str) -> anyhow::Result<ParsedVariable> {
    let parts: Vec<&str> = var_str.split('.').collect();

    match parts.len() {
        2 => {
            // Format: {type.identifier} - current mod
            let var_type = match parts[0] {
                "habitats" => VariableType::Habitats,
                "locations" => VariableType::Locations,
                "strings" => VariableType::Strings,
                _ => anyhow::bail!("Invalid variable type '{}': expected 'habitat', 'location', or 'string'", parts[0]),
            };

            Ok(ParsedVariable {
                var_type,
                mod_id: None,
                identifier: parts[1].to_string(),
                legacy_parts: None,
            })
        }
        3 => {
            // Check for legacy variable syntax: {legacy.type.name} (default subtype)
            if parts[0] == "legacy" {
                let entity_type: LegacyEntityType = parts[1].parse()?;
                Ok(ParsedVariable {
                    var_type: VariableType::Legacy,
                    mod_id: None,
                    identifier: parts[2].to_string(),
                    legacy_parts: Some(LegacyVariableParts {
                        entity_type,
                        entity_name: parts[2].to_string(),
                        subtype: None,  // Will use default
                        attribute: "name_id".to_string(),
                    }),
                })
            } else {
                // Cross-mod reference: {mod.type.identifier}
                let var_type = match parts[1] {
                    "habitats" => VariableType::Habitats,
                    "locations" => VariableType::Locations,
                    "strings" => VariableType::Strings,
                    _ => anyhow::bail!("Invalid variable type '{}': expected 'habitats', 'locations', or 'string'", parts[1]),
                };

                Ok(ParsedVariable {
                    var_type,
                    mod_id: Some(parts[0].to_string()),
                    identifier: parts[2].to_string(),
                    legacy_parts: None,
                })
            }
        }
        4 => {
            // NEW: Format: {legacy.type.name.attribute} - explicit attribute, default subtype
            // OR: {legacy.type.name.subtype} - no attribute, invalid
            if parts[0] == "legacy" {
                let entity_type: LegacyEntityType = parts[1].parse()?;

                // Check if parts[3] is a known attribute or a subtype
                if parts[3] == "name_id" {
                    Ok(ParsedVariable {
                        var_type: VariableType::Legacy,
                        mod_id: None,
                        identifier: parts[2].to_string(),
                        legacy_parts: Some(LegacyVariableParts {
                            entity_type,
                            entity_name: parts[2].to_string(),
                            subtype: None,  // Use default
                            attribute: parts[3].to_string(),
                        }),
                    })
                } else {
                    // Assume it's a subtype - but we need 5 parts for subtype+attribute
                    anyhow::bail!("Invalid variable syntax '{}': expected {{legacy.type.name.subtype.attribute}} for subtype references", var_str)
                }
            } else {
                anyhow::bail!("Invalid variable syntax")
            }
        }
        5 => {
            // NEW: Format: {legacy.type.name.subtype.attribute} - explicit subtype
            if parts[0] == "legacy" {
                let entity_type: LegacyEntityType = parts[1].parse()?;
                Ok(ParsedVariable {
                    var_type: VariableType::Legacy,
                    mod_id: None,
                    identifier: parts[2].to_string(),
                    legacy_parts: Some(LegacyVariableParts {
                        entity_type,
                        entity_name: parts[2].to_string(),
                        subtype: Some(parts[3].to_string()),
                        attribute: parts[4].to_string(),
                    }),
                })
            } else {
                anyhow::bail!("Invalid variable syntax")
            }
        }
        _ => {
            anyhow::bail!("Invalid variable syntax '{}': expected {{type.name}}, {{mod.type.name}}, {{legacy.type.name.attribute}}, or {{legacy.type.name.subtype.attribute}}", var_str)
        }
    }
}

/// Resolve a parsed variable to its string value
///
/// # Arguments
/// * `var` - Parsed variable structure
/// * `context` - Current mod context for relative references
///
/// # Returns
/// * `Ok(String)` - The resolved value (string ID or string content)
/// * `Err` - If variable doesn't exist or mod not loaded
fn resolve_variable(var: &ParsedVariable, context: &SubstitutionContext) -> anyhow::Result<String> {
    match &var.var_type {
        VariableType::Habitats => {
            let mod_id = var.mod_id.as_deref().unwrap_or(&context.current_mod_id);

            match get_habitat_id(mod_id, &var.identifier) {
                Some(string_id) => Ok(string_id.to_string()),
                None => {
                    if var.mod_id.is_some() {
                        anyhow::bail!(
                            "Habitat '{}' not found in mod '{}' (ensure mod is loaded and habitat is defined)",
                            var.identifier, mod_id
                        )
                    } else {
                        anyhow::bail!(
                            "Habitat '{}' not found in current mod",
                            var.identifier
                        )
                    }
                }
            }
        }
        VariableType::Locations => {
            let mod_id = var.mod_id.as_deref().unwrap_or(&context.current_mod_id);

            match get_location_id(mod_id, &var.identifier) {
                Some(string_id) => Ok(string_id.to_string()),
                None => {
                    if var.mod_id.is_some() {
                        anyhow::bail!(
                            "Location '{}' not found in mod '{}' (ensure mod is loaded and location is defined)",
                            var.identifier, mod_id
                        )
                    } else {
                        anyhow::bail!(
                            "Location '{}' not found in current mod",
                            var.identifier
                        )
                    }
                }
            }
        }
        VariableType::Strings => {
            let string_id: u32 = var.identifier.parse()
                .with_context(|| format!("Invalid string ID '{}': must be a number", var.identifier))?;

            get_string_from_registry(string_id)
                .map_err(|_| anyhow::anyhow!("String ID {} not found in registry", string_id))
        }
        VariableType::Legacy => {
            // NEW: Resolve legacy entity attribute
            let parts = var.legacy_parts.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Legacy variable missing parts"))?;

            // Only name_id is supported in initial implementation
            if parts.attribute != "name_id" {
                anyhow::bail!(
                    "Unsupported attribute '{}'. Only 'name_id' is currently supported.",
                    parts.attribute
                );
            }

            // Determine which subtype to use
            let subtype_to_use = if let Some(ref st) = parts.subtype {
                Some(st.as_str())
            } else {
                // Use default subtype for this entity type
                parts.entity_type.default_subtype()
            };

            get_legacy_attribute_with_subtype(
                parts.entity_type,
                &parts.entity_name,
                subtype_to_use,
                &parts.attribute
            ).map_err(|e| anyhow::anyhow!("{} (type: {}, entity: {}, subtype: {:?})",
                e, parts.entity_type.as_str(), parts.entity_name,
                subtype_to_use))
        }
    }
}

/// Perform variable substitution on a string value
///
/// Finds all {variable} patterns, resolves them, and replaces with values.
///
/// # Arguments
/// * `input` - The string potentially containing {variable} references
/// * `context` - Current mod context
///
/// # Returns
/// * `Ok(String)` - String with all variables substituted
/// * `Err` - If any variable fails to resolve
///
/// # Examples
/// * Input: "cHabitat={habitats.swamp}" with swamp registered as ID 100005
/// * Output: "cHabitat=100005"
fn substitute_variables(input: &str, context: &SubstitutionContext) -> anyhow::Result<String> {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Find the closing brace
            let mut var_content = String::new();
            let mut found_close = false;

            while let Some(&next_ch) = chars.peek() {
                if next_ch == '}' {
                    chars.next(); // Consume the '}'
                    found_close = true;
                    break;
                }
                var_content.push(chars.next().unwrap());
            }

            if !found_close {
                anyhow::bail!("Unclosed variable brace in: {}", input);
            }

            // Parse and resolve the variable
            let parsed_var = parse_variable(&var_content)
                .with_context(|| format!("Failed to parse variable '{{{}}}'", var_content))?;

            let resolved_value = resolve_variable(&parsed_var, context)
                .with_context(|| format!("Failed to resolve variable '{{{}}}'", var_content))?;

            result.push_str(&resolved_value);
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

// ============================================================================
// Shadow Resources for Rollback Support
// ============================================================================

/// Shadow resource map for transactional patch application
///
/// This struct holds shadow copies of files that patches will modify.
/// Patches are applied to the shadow copies, then committed to the main
/// resource system on success, or discarded on failure (automatic rollback).
pub struct ShadowResources {
    /// Shadow copies of files being modified (path -> shadow ZTFile)
    pub files: HashMap<String, ZTFile>,

    /// Files that will be created (don't exist in main resources yet)
    pub new_files: HashSet<String>,

    /// Files that should be deleted from main resources on commit
    deleted_files: HashSet<String>,

    /// Scope of this shadow (for logging)
    scope: ShadowScope,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadowScope {
    /// Shadow for patch file only
    PatchFile,

    /// Shadow for entire mod
    Mod,
}

impl ShadowResources {
    /// Create shadow by cloning affected files from main resource system
    ///
    /// # Arguments
    /// * `affected_files` - Set of file paths that will be modified
    /// * `scope` - Scope of this shadow (PatchFile or Mod)
    ///
    /// # Returns
    /// * `Ok(ShadowResources)` - Shadow with cloned files
    /// * `Err(_)` if there's an error accessing files
    pub fn new(affected_files: &HashSet<String>, scope: ShadowScope) -> anyhow::Result<Self> {
        let mut files = HashMap::new();
        let mut new_files = HashSet::new();

        for path in affected_files {
            if let Some((_filename, raw_data)) = get_file(path) {
                // File exists - convert to ZTFile and clone into shadow
                let file_type = ZTFileType::try_from(Path::new(path))
                    .map_err(|e| anyhow::anyhow!("Invalid file type for '{}': {}", path, e))?;

                let ztfile = match file_type {
                    ZTFileType::Ini | ZTFileType::Ai | ZTFileType::Ani | ZTFileType::Cfg
                    | ZTFileType::Lyt | ZTFileType::Scn | ZTFileType::Uca | ZTFileType::Ucs
                    | ZTFileType::Ucb | ZTFileType::Txt | ZTFileType::Toml => {
                        let content = crate::encoding_utils::decode_game_text(&raw_data);
                        let content_len = content.len() as u32;
                        let c_string = std::ffi::CString::new(content)?;
                        ZTFile::Text(c_string, file_type, content_len)
                    }
                    _ => {
                        ZTFile::RawBytes(raw_data, file_type, 0)
                    }
                };

                files.insert(path.clone(), ztfile);
            } else {
                // File doesn't exist yet - mark as new
                new_files.insert(path.clone());
            }
        }

        info!("Created {:?} shadow: {} files cloned, {} will be created",
              scope, files.len(), new_files.len());

        Ok(ShadowResources {
            files,
            new_files,
            deleted_files: HashSet::new(),
            scope,
        })
    }

    /// Get a file from shadow (or fall back to main resources if not shadowed)
    ///
    /// # Arguments
    /// * `path` - File path to retrieve
    ///
    /// # Returns
    /// * `Some(ZTFile)` - File from shadow or main resources
    /// * `None` - File not found anywhere
    pub fn get_file(&self, path: &str) -> Option<ZTFile> {
        // Check shadow first
        if let Some(file) = self.files.get(path) {
            return Some(file.clone());
        }

        // Fall back to main resources for non-shadowed files
        if let Some((_, raw_data)) = get_file(path) {
            // Convert to ZTFile
            let file_type = ZTFileType::try_from(Path::new(path)).ok()?;

            let ztfile = match file_type {
                ZTFileType::Ini | ZTFileType::Ai | ZTFileType::Ani | ZTFileType::Cfg
                | ZTFileType::Lyt | ZTFileType::Scn | ZTFileType::Uca | ZTFileType::Ucs
                | ZTFileType::Ucb | ZTFileType::Txt | ZTFileType::Toml => {
                    let content = crate::encoding_utils::decode_game_text(&raw_data);
                    let content_len = content.len() as u32;
                    let c_string = std::ffi::CString::new(content).ok()?;
                    ZTFile::Text(c_string, file_type, content_len)
                }
                _ => {
                    ZTFile::RawBytes(raw_data, file_type, 0)
                }
            };

            Some(ztfile)
        } else {
            None
        }
    }

    /// Update a file in the shadow
    ///
    /// # Arguments
    /// * `path` - File path to update
    /// * `file` - New file content
    pub fn update_file(&mut self, path: &str, file: ZTFile) {
        self.files.insert(path.to_string(), file);

        // If this was marked as new, it's now created
        self.new_files.remove(path);
    }

    /// Delete a file from the shadow
    ///
    /// # Arguments
    /// * `path` - File path to delete
    pub fn delete_file(&mut self, path: &str) {
        self.files.remove(path);
        self.new_files.remove(path);
        // Mark file for deletion from main resources on commit
        self.deleted_files.insert(path.to_string());
    }

    /// Check if a file exists in shadow or main resources
    ///
    /// # Arguments
    /// * `path` - File path to check
    ///
    /// # Returns
    /// * `true` - File exists in shadow or main resources (and not deleted)
    /// * `false` - File not found or marked for deletion
    pub fn file_exists(&self, path: &str) -> bool {
        // If file is marked for deletion, it doesn't exist
        if self.deleted_files.contains(path) {
            return false;
        }

        // Check shadow or main resources
        self.files.contains_key(path) || check_file(path)
    }

    /// Commit shadow to main resource system (success case)
    ///
    /// This writes all shadow files to the main resource system.
    ///
    /// # Returns
    /// * `Ok(())` if all files were committed successfully
    /// * `Err(_)` if there's an error writing files
    pub fn commit(self) -> anyhow::Result<()> {
        info!("Committing {:?} shadow: {} files to write, {} to delete",
              self.scope, self.files.len(), self.deleted_files.len());

        let start = std::time::Instant::now();

        // Write all shadow files to main resource system
        for (path, file) in self.files {
            add_ztfile(Path::new(""), path, file)?;
        }

        // Delete files marked for deletion
        for path in self.deleted_files {
            remove_resource(&path);
        }

        let elapsed = start.elapsed();
        info!("Shadow committed in {:.2?}", elapsed);

        Ok(())
    }

    /// Discard shadow without committing (failure case - automatic rollback)
    ///
    /// This method is called explicitly to log the discard, but rollback
    /// happens automatically when the ShadowResources is dropped.
    pub fn discard(self) {
        info!("Discarding {:?} shadow: {} files dropped, {} deletions cancelled (rollback)",
              self.scope, self.files.len(), self.deleted_files.len());
        // Automatic drop - no action needed
    }
}

// ============================================================================
// Shadow Helper Functions
// ============================================================================

/// Load INI file from shadow (or main resources if not shadowed)
///
/// # Arguments
/// * `path` - File path to load
/// * `shadow` - Shadow resources to check first
///
/// # Returns
/// * `Ok(Ini)` - Parsed INI file
/// * `Err(_)` if file not found or not parseable as INI
fn load_ini_from_shadow(path: &str, shadow: &ShadowResources) -> anyhow::Result<Ini> {
    let file = shadow.get_file(path)
        .ok_or_else(|| anyhow::anyhow!("File '{}' not found", path))?;

    match file {
        ZTFile::Text(content, _, _) => {
            let content_str = content.to_str()?.to_string();
            let mut ini = Ini::new_cs();
            ini.set_comment_symbols(&[';', '#', ':']);
            ini.read(content_str)
                .map_err(|e| anyhow::anyhow!("Failed to parse INI: {}", e))?;
            Ok(ini)
        }
        _ => anyhow::bail!("File '{}' is not a text file", path),
    }
}

/// Save INI file to shadow
///
/// # Arguments
/// * `path` - File path to save to
/// * `ini` - INI content to save
/// * `shadow` - Shadow resources to update
///
/// # Returns
/// * `Ok(())` if saved successfully
/// * `Err(_)` if there's an error converting or saving
fn save_ini_to_shadow(path: &str, ini: &Ini, shadow: &mut ShadowResources) -> anyhow::Result<()> {
    let content = ini.writes();
    let content_len = content.len() as u32;
    let c_string = std::ffi::CString::new(content)?;

    let file_type = ZTFileType::try_from(Path::new(path))
        .map_err(|e| anyhow::anyhow!("Invalid file type: {}", e))?;

    let ztfile = ZTFile::Text(c_string, file_type, content_len);
    shadow.update_file(path, ztfile);

    Ok(())
}

/// Check if file exists in shadow or main resources
///
/// # Arguments
/// * `path` - File path to check
/// * `shadow` - Shadow resources to check
///
/// # Returns
/// * `true` if file exists
/// * `false` if file not found
fn check_file_in_shadow(path: &str, shadow: &ShadowResources) -> bool {
    shadow.file_exists(path)
}

/// Collect all files that will be modified by patches
///
/// # Arguments
/// * `patches` - Map of patches to analyze
///
/// # Returns
/// * `HashSet<String>` - Set of unique file paths that will be affected
fn collect_affected_files(patches: &indexmap::IndexMap<String, Patch>) -> HashSet<String> {
    let mut files = HashSet::new();

    for patch in patches.values() {
        match patch {
            Patch::Replace(p) => { files.insert(p.target.clone()); },
            Patch::Merge(p) => { files.insert(p.target.clone()); },
            Patch::Delete(p) => { files.insert(p.target.clone()); },
            Patch::SetPalette(p) => { files.insert(p.target.clone()); },
            Patch::SetKey(p) => { files.insert(p.target.clone()); },
            Patch::SetKeys(p) => { files.insert(p.target.clone()); },
            Patch::AppendValue(p) => { files.insert(p.target.clone()); },
            Patch::AppendValues(p) => { files.insert(p.target.clone()); },
            Patch::RemoveKey(p) => { files.insert(p.target.clone()); },
            Patch::RemoveKeys(p) => { files.insert(p.target.clone()); },
            Patch::AddSection(p) => { files.insert(p.target.clone()); },
            Patch::ClearSection(p) => { files.insert(p.target.clone()); },
            Patch::RemoveSection(p) => { files.insert(p.target.clone()); },
        }
    }

    files
}

// ============================================================================
// Shadow Patch Operations (for abort/abort_mod modes)
// ============================================================================

/// Apply set_key patch to shadow
fn apply_set_key_patch_shadow(
    patch: &SetKeyPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    context: &SubstitutionContext,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying set_key patch '{}' to shadow: {} [{}] {} = {}",
          patch_name, patch.target, patch.section, patch.key, patch.value);

    validate_ini_file(&patch.target)?;
    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    let resolved_value = substitute_variables(&patch.value, context)?;
    ini.setstr(&patch.section, &patch.key, Some(&resolved_value));

    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied set_key patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply set_keys patch to shadow
fn apply_set_keys_patch_shadow(
    patch: &SetKeysPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    context: &SubstitutionContext,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying set_keys patch '{}' to shadow: {} [{}] ({} keys)",
          patch_name, patch.target, patch.section, patch.keys.len());

    validate_ini_file(&patch.target)?;
    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    for (key, value) in &patch.keys {
        let resolved_value = substitute_variables(value, context)?;
        ini.setstr(&patch.section, key, Some(&resolved_value));
    }

    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied set_keys patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply append_value patch to shadow
fn apply_append_value_patch_shadow(
    patch: &AppendValuePatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    context: &SubstitutionContext,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying append_value patch '{}' to shadow: {} [{}] {} += {}",
          patch_name, patch.target, patch.section, patch.key, patch.value);

    validate_ini_file(&patch.target)?;
    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    let resolved_value = substitute_variables(&patch.value, context)?;
    ini.addstr(&patch.section, &patch.key, &resolved_value);

    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied append_value patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply append_values patch to shadow
fn apply_append_values_patch_shadow(
    patch: &AppendValuesPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    context: &SubstitutionContext,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying append_values patch '{}' to shadow: {} [{}] {} += {} values",
          patch_name, patch.target, patch.section, patch.key, patch.values.len());

    validate_ini_file(&patch.target)?;
    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    for value in &patch.values {
        let resolved_value = substitute_variables(value, context)?;
        ini.addstr(&patch.section, &patch.key, &resolved_value);
    }

    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied append_values patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply remove_key patch to shadow
fn apply_remove_key_patch_shadow(
    patch: &RemoveKeyPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying remove_key patch '{}' to shadow: {} [{}] -{}",
          patch_name, patch.target, patch.section, patch.key);

    validate_ini_file(&patch.target)?;

    if !check_file_in_shadow(&patch.target, shadow) {
        warn!("Remove_key patch '{}': file '{}' not found, skipping",
              patch_name, patch.target);
        return Ok(());
    }

    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    if !ini.has_section(&patch.section) {
        warn!("Remove_key patch '{}': section '{}' not found, skipping",
              patch_name, patch.section);
        return Ok(());
    }

    if ini.remove_key(&patch.section, &patch.key).is_none() {
        warn!("Remove_key patch '{}': key '{}' not found in section '{}', skipping",
              patch_name, patch.key, patch.section);
        return Ok(());
    }

    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied remove_key patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply remove_keys patch to shadow
fn apply_remove_keys_patch_shadow(
    patch: &RemoveKeysPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying remove_keys patch '{}' to shadow: {} [{}] -{} keys",
          patch_name, patch.target, patch.section, patch.keys.len());

    validate_ini_file(&patch.target)?;

    if !check_file_in_shadow(&patch.target, shadow) {
        warn!("Remove_keys patch '{}': file '{}' not found, skipping",
              patch_name, patch.target);
        return Ok(());
    }

    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    if !ini.has_section(&patch.section) {
        warn!("Remove_keys patch '{}': section '{}' not found, skipping",
              patch_name, patch.section);
        return Ok(());
    }

    for key in &patch.keys {
        ini.remove_key(&patch.section, key);
    }

    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied remove_keys patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply add_section patch to shadow
fn apply_add_section_patch_shadow(
    patch: &AddSectionPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    context: &SubstitutionContext,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying add_section patch '{}' to shadow: {} [{}] ({} keys, on_exists: {:?})",
          patch_name, patch.target, patch.section, patch.keys.len(), patch.on_exists);

    validate_ini_file(&patch.target)?;
    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    let section_exists = ini.has_section(&patch.section);

    match (&patch.on_exists, section_exists) {
        (OnExists::Error, true) => {
            anyhow::bail!("Section '{}' already exists in '{}'", patch.section, patch.target);
        }
        (OnExists::Skip, true) => {
            warn!("Add_section patch '{}': section '{}' already exists, skipping",
                  patch_name, patch.section);
            return Ok(());
        }
        (OnExists::Replace, true) => {
            ini.clear_section(&patch.section);
        }
        _ => {}
    }

    for (key, value) in &patch.keys {
        let resolved_value = substitute_variables(value, context)?;
        ini.setstr(&patch.section, key, Some(&resolved_value));
    }

    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied add_section patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply clear_section patch to shadow
fn apply_clear_section_patch_shadow(
    patch: &ClearSectionPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying clear_section patch '{}' to shadow: {} [{}]",
          patch_name, patch.target, patch.section);

    validate_ini_file(&patch.target)?;

    if !check_file_in_shadow(&patch.target, shadow) {
        warn!("Clear_section patch '{}': file '{}' not found, skipping",
              patch_name, patch.target);
        return Ok(());
    }

    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    if !ini.has_section(&patch.section) {
        warn!("Clear_section patch '{}': section '{}' not found, skipping",
              patch_name, patch.section);
        return Ok(());
    }

    ini.clear_section(&patch.section);
    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied clear_section patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply remove_section patch to shadow
fn apply_remove_section_patch_shadow(
    patch: &RemoveSectionPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying remove_section patch '{}' to shadow: {} -[{}]",
          patch_name, patch.target, patch.section);

    validate_ini_file(&patch.target)?;

    if !check_file_in_shadow(&patch.target, shadow) {
        warn!("Remove_section patch '{}': file '{}' not found, skipping",
              patch_name, patch.target);
        return Ok(());
    }

    let mut ini = load_ini_from_shadow(&patch.target, shadow)?;

    if !ini.has_section(&patch.section) {
        warn!("Remove_section patch '{}': section '{}' not found, skipping",
              patch_name, patch.section);
        return Ok(());
    }

    ini.remove_section(&patch.section);
    save_ini_to_shadow(&patch.target, &ini, shadow)?;

    info!("Successfully applied remove_section patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply replace patch to shadow
fn apply_replace_patch_shadow(
    patch: &ReplacePatch,
    file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,  // Not used in shadow mode
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying replace patch '{}' to shadow: {} -> {}",
          patch_name, patch.source, patch.target);

    // Check if target exists (in shadow or main resources)
    if !check_file_in_shadow(&patch.target, shadow) {
        anyhow::bail!("Target file '{}' not found", patch.target);
    }

    // Load source file from archive
    let source_data = resolve_source_file(&patch.source, file_map)?;
    let file_type = ZTFileType::try_from(Path::new(&patch.target))
        .map_err(|e| anyhow::anyhow!("Invalid target file type: {}", e))?;

    let ztfile = match file_type {
        ZTFileType::Ini | ZTFileType::Ai | ZTFileType::Ani | ZTFileType::Cfg
        | ZTFileType::Lyt | ZTFileType::Scn | ZTFileType::Uca | ZTFileType::Ucs
        | ZTFileType::Ucb | ZTFileType::Txt | ZTFileType::Toml => {
            let content = crate::encoding_utils::decode_game_text(&source_data);
            let content_len = content.len() as u32;
            let c_string = std::ffi::CString::new(content)?;
            ZTFile::Text(c_string, file_type, content_len)
        }
        _ => {
            ZTFile::RawBytes(source_data.into_boxed_slice(), file_type, 0)
        }
    };

    shadow.update_file(&patch.target, ztfile);

    info!("Successfully applied replace patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply merge patch to shadow
fn apply_merge_patch_shadow(
    patch: &MergePatch,
    file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,  // Not used in shadow mode
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying merge patch '{}' to shadow: {} + {} (mode: {:?})",
          patch_name, patch.target, patch.source, patch.merge_mode);

    // Validate INI file type
    let target_path = Path::new(&patch.target);
    let target_ext = target_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let valid_extensions = ["ini", "ai", "cfg", "uca", "ucs", "ucb", "scn", "lyt"];
    if !valid_extensions.contains(&target_ext) {
        anyhow::bail!("Target file '{}' is not an INI file. Merge only works with INI files.",
                     patch.target);
    }

    // Check target exists
    if !check_file_in_shadow(&patch.target, shadow) {
        anyhow::bail!("Target file '{}' not found", patch.target);
    }

    // Load target INI from shadow
    let mut target_ini = load_ini_from_shadow(&patch.target, shadow)?;

    // Load source INI from archive
    let source_data = resolve_source_file(&patch.source, file_map)?;
    let source_str = crate::encoding_utils::decode_game_text(&source_data);
    let mut source_ini = Ini::new_cs();
    source_ini.set_comment_symbols(&[';', '#', ':']);
    source_ini.read(source_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse source INI '{}': {}", patch.source, e))?;

    // Merge based on mode
    let mode = match patch.merge_mode {
        MergeMode::PatchPriority => IniMergeMode::PatchPriority,
        MergeMode::BasePriority => IniMergeMode::BasePriority,
    };

    target_ini.merge_with_priority(&source_ini, mode);

    // Save merged result to shadow
    save_ini_to_shadow(&patch.target, &target_ini, shadow)?;

    info!("Successfully applied merge patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply delete patch to shadow
fn apply_delete_patch_shadow(
    patch: &DeletePatch,
    patch_name: &str,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying delete patch '{}' to shadow: -{}", patch_name, patch.target);

    if !check_file_in_shadow(&patch.target, shadow) {
        warn!("Delete patch '{}': file '{}' not found, skipping",
              patch_name, patch.target);
        return Ok(());
    }

    shadow.delete_file(&patch.target);

    info!("Successfully applied delete patch '{}' to shadow", patch_name);
    Ok(())
}

/// Apply set_palette patch to shadow
fn apply_set_palette_patch_shadow(
    patch: &SetPalettePatch,
    patch_name: &str,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    info!("Applying set_palette patch '{}' to shadow: {} palette -> {}",
          patch_name, patch.target, patch.palette);

    // Validate target is animation (no extension)
    if Path::new(&patch.target).extension().is_some() {
        anyhow::bail!("Target '{}' has extension - set_palette only works on animation files (no extension)",
                     patch.target);
    }

    // Validate palette has .pal extension
    let palette_path = Path::new(&patch.palette);
    let palette_ext = palette_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if palette_ext != "pal" {
        anyhow::bail!("Palette '{}' must have .pal extension", patch.palette);
    }

    // Check target exists
    if !check_file_in_shadow(&patch.target, shadow) {
        anyhow::bail!("Target animation file '{}' not found", patch.target);
    }

    // Load animation from shadow
    let animation_file = shadow.get_file(&patch.target)
        .ok_or_else(|| anyhow::anyhow!("Failed to load animation '{}' from shadow", patch.target))?;

    let animation_data = match animation_file {
        ZTFile::RawBytes(data, _, _) => data,
        _ => anyhow::bail!("Animation file '{}' is not raw bytes", patch.target),
    };

    // Parse animation
    let mut animation = Animation::parse(&animation_data)?;

    // Set new palette filename
    animation.set_palette_filename(patch.palette.clone());

    // Write animation back
    let (new_animation_bytes, _length) = animation.write()?;

    // Update shadow with modified animation
    let ztfile = ZTFile::RawBytes(new_animation_bytes.into_boxed_slice(), ZTFileType::Animation, 0);
    shadow.update_file(&patch.target, ztfile);

    info!("Successfully applied set_palette patch '{}' to shadow", patch_name);
    Ok(())
}

// ============================================================================
// Phase 3: Direct Patch Operations (for continue mode - no shadow)
// ============================================================================

/// Apply a replace patch directly to resources: replaces an entire file in the resource system
///
/// This loads the source file from the current mod and replaces the target file
/// in the resource map. The target file must exist, and the source file must
/// exist in the current mod.
///
/// # Arguments
/// * `patch` - The replace patch configuration
/// * `mod_path` - Path to the current mod being loaded (for resolving source files)
/// * `patch_name` - Name of the patch (for logging)
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the target file doesn't exist, source file doesn't exist, or other errors occur
fn apply_replace_patch_direct(
    patch: &ReplacePatch,
    file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    info!("Applying replace patch '{}': {} -> {}", patch_name, patch.source, patch.target);

    // Check if target file exists in resource system
    if !check_file(&patch.target) {
        anyhow::bail!("Target file '{}' not found in resource system", patch.target);
    }

    // Load source file from archive
    let source_data = resolve_source_file(&patch.source, file_map)?;
    let file_type = ZTFileType::try_from(Path::new(&patch.target))
        .map_err(|e| anyhow::anyhow!("Invalid target file type: {}", e))?;

    // Create ZTFile based on file type
    let ztfile = match file_type {
        ZTFileType::Ini | ZTFileType::Ai | ZTFileType::Ani | ZTFileType::Cfg
        | ZTFileType::Lyt | ZTFileType::Scn | ZTFileType::Uca | ZTFileType::Ucs
        | ZTFileType::Ucb | ZTFileType::Txt | ZTFileType::Toml => {
            let content = crate::encoding_utils::decode_game_text(&source_data);
            let content_len = content.len() as u32;
            let c_string = std::ffi::CString::new(content)?;
            ZTFile::Text(c_string, file_type, content_len)
        }
        _ => {
            ZTFile::RawBytes(source_data.into_boxed_slice(), file_type, 0)
        }
    };

    // Update resource (add_ztfile_from_memory automatically replaces if exists)
    add_ztfile_from_memory(current_mod_id, patch.target.clone(), ztfile)?;

    info!("Successfully applied replace patch '{}'", patch_name);
    Ok(())
}

/// Apply a merge patch directly to resources: merges two INI files together
///
/// This loads both the target and source INI files, merges them according to
/// the merge_mode, and replaces the target file with the merged result.
///
/// # Arguments
/// * `patch` - The merge patch configuration
/// * `file_map` - HashMap of files from the .ztd archive
/// * `patch_name` - Name of the patch (for logging)
/// * `current_mod_id` - The ID of the current mod (for tracking resource sources)
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if files don't exist, aren't INI files, or other errors occur
fn apply_merge_patch_direct(
    patch: &MergePatch,
    file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    info!("Applying merge patch '{}': {} + {} (mode: {:?})",
          patch_name, patch.target, patch.source, patch.merge_mode);

    // Check if target file exists
    if !check_file(&patch.target) {
        anyhow::bail!("Target file '{}' not found in resource system", patch.target);
    }

    // Validate that target is an INI-compatible file
    validate_ini_file(&patch.target)?;

    // Load target INI file
    let target_file = get_file(&patch.target)
        .ok_or_else(|| anyhow::anyhow!("Failed to load target file '{}'", patch.target))?;
    let target_str = crate::encoding_utils::decode_game_text(&target_file.1);
    let mut target_ini = Ini::new_cs();
    target_ini.set_comment_symbols(&[';', '#', ':']);
    target_ini.read(target_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse target INI file '{}': {}", patch.target, e))?;

    // Load source INI file from archive
    let source_data = resolve_source_file(&patch.source, file_map)?;
    let source_str = crate::encoding_utils::decode_game_text(&source_data);
    let mut source_ini = Ini::new_cs();
    source_ini.set_comment_symbols(&[';', '#', ':']);
    source_ini.read(source_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse source INI file '{}': {}", patch.source, e))?;

    // Convert MergeMode enum from mods to IniMergeMode from configparser
    let ini_merge_mode = match patch.merge_mode {
        MergeMode::PatchPriority => IniMergeMode::PatchPriority,
        MergeMode::BasePriority => IniMergeMode::BasePriority,
    };

    // Merge source into target
    target_ini.merge_with_priority(&source_ini, ini_merge_mode);

    // Write merged INI to string
    let merged_content = target_ini.writes();

    // Create ZTFile and update resource
    let file_type = ZTFileType::try_from(Path::new(&patch.target))
        .map_err(|e| anyhow::anyhow!("Invalid target file type: {}", e))?;
    let c_string = std::ffi::CString::new(merged_content.clone())?;
    let ztfile = ZTFile::Text(c_string, file_type, merged_content.len() as u32);

    // Update resource (add_ztfile_from_memory automatically replaces if exists)
    add_ztfile_from_memory(current_mod_id, patch.target.clone(), ztfile)?;

    info!("Successfully applied merge patch '{}'", patch_name);
    Ok(())
}

/// Apply a delete patch directly to resources: removes a file from the resource system
///
/// # Arguments
/// * `patch` - The delete patch configuration
/// * `patch_name` - Name of the patch (for logging)
///
/// # Returns
/// * `Ok(())` always (warnings logged if file doesn't exist)
fn apply_delete_patch_direct(patch: &DeletePatch, patch_name: &str) -> anyhow::Result<()> {
    info!("Applying delete patch '{}': {}", patch_name, patch.target);

    if !check_file(&patch.target) {
        warn!("Delete patch '{}': target file '{}' not found (already deleted or never existed)",
              patch_name, patch.target);
        return Ok(());
    }

    let removed = remove_resource(&patch.target);
    if removed {
        info!("Successfully applied delete patch '{}' - removed '{}'", patch_name, patch.target);
    } else {
        warn!("Delete patch '{}': failed to remove '{}' (may have been removed by another operation)",
              patch_name, patch.target);
    }

    Ok(())
}

/// Apply a set_palette patch directly to resources: changes the palette reference in an animation file
///
/// This modifies the palette filename stored inside an animation file's header
/// without changing the animation data itself.
///
/// # Arguments
/// * `patch` - The set_palette patch configuration
/// * `patch_name` - Name of the patch (for logging)
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if validation fails or animation parsing/writing fails
fn apply_set_palette_patch_direct(patch: &SetPalettePatch, patch_name: &str) -> anyhow::Result<()> {
    info!("Applying set_palette patch '{}': {} -> palette: {}",
          patch_name, patch.target, patch.palette);

    // Validate target has no extension (must be animation file)
    let target_path = Path::new(&patch.target);
    if target_path.extension().is_some() {
        anyhow::bail!("Target file '{}' has an extension. Animation files must have no extension.",
                     patch.target);
    }

    // Validate palette has .pal extension
    if !patch.palette.to_lowercase().ends_with(".pal") {
        anyhow::bail!("Palette file '{}' must have .pal extension", patch.palette);
    }

    // Check if target file exists
    if !check_file(&patch.target) {
        anyhow::bail!("Target animation file '{}' not found in resource system", patch.target);
    }

    // Modify the animation file's palette reference
    modify_ztfile_as_animation(&patch.target, |animation: &mut Animation| {
        animation.set_palette_filename(patch.palette.clone());
        Ok(())
    })?;

    info!("Successfully applied set_palette patch '{}' - updated palette reference to '{}'",
          patch_name, patch.palette);
    Ok(())
}

// ============================================================================
// Phase 4: Direct Element-Level Patch Operations (INI Files - for continue mode)
// ============================================================================

/// Valid INI file extensions
const VALID_INI_EXTENSIONS: &[&str] = &["ini", "ai", "cfg", "uca", "ucs", "ucb", "scn", "lyt"];

/// Check if a file extension is valid for INI operations
fn is_valid_ini_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| VALID_INI_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Helper function to validate that a file is an INI-compatible file
fn validate_ini_file(target: &str) -> anyhow::Result<()> {
    let path = Path::new(target);

    if !is_valid_ini_extension(path) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("none");
        anyhow::bail!(
            "Target file '{}' is not an INI file (extension: {}). Valid extensions: {}",
            target,
            ext,
            VALID_INI_EXTENSIONS.join(", ")
        );
    }

    Ok(())
}

/// Helper function to load an INI file from the resource system
fn load_ini_from_resources(target: &str) -> anyhow::Result<Ini> {
    // Check if target file exists
    if !check_file(target) {
        anyhow::bail!("Target file '{}' not found in resource system", target);
    }

    // Validate file type
    validate_ini_file(target)?;

    // Load and parse INI file
    let target_file = get_file(target)
        .ok_or_else(|| anyhow::anyhow!("Failed to load target file '{}'", target))?;
    let target_str = crate::encoding_utils::decode_game_text(&target_file.1);
    let mut ini = Ini::new_cs();
    ini.set_comment_symbols(&[';', '#', ':']);
    ini.read(target_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse INI file '{}': {}", target, e))?;

    Ok(ini)
}

/// Helper function to save a modified INI file back to the resource system
fn save_ini_to_resources(target: &str, ini: &Ini, current_mod_id: &str) -> anyhow::Result<()> {
    // Write INI to string
    let content = ini.writes();

    // Create ZTFile
    let file_type = ZTFileType::try_from(Path::new(target))
        .map_err(|e| anyhow::anyhow!("Invalid target file type: {}", e))?;
    let c_string = std::ffi::CString::new(content.clone())?;
    let ztfile = ZTFile::Text(c_string, file_type, content.len() as u32);

    // Update resource (add_ztfile_from_memory automatically replaces if exists)
    add_ztfile_from_memory(current_mod_id, target.to_string(), ztfile)?;

    Ok(())
}

/// Apply a set_key patch directly to resources: sets a single key-value pair in an INI section
///
/// # Arguments
/// * `patch` - The set_key patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
/// * `context` - Substitution context for variable resolution
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_set_key_patch_direct(
    patch: &SetKeyPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
    context: &SubstitutionContext,
) -> anyhow::Result<()> {
    info!("Applying set_key patch '{}': {} [{}] {} = {}",
          patch_name, patch.target, patch.section, patch.key, patch.value);

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Perform variable substitution on the value
    let resolved_value = substitute_variables(&patch.value, context)?;

    // Set the key (creates section if it doesn't exist)
    ini.setstr(&patch.section, &patch.key, Some(&resolved_value));

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied set_key patch '{}'", patch_name);
    Ok(())
}

/// Apply a set_keys patch directly to resources: sets multiple key-value pairs in an INI section
///
/// # Arguments
/// * `patch` - The set_keys patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
/// * `context` - Substitution context for variable resolution
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_set_keys_patch_direct(
    patch: &SetKeysPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
    context: &SubstitutionContext,
) -> anyhow::Result<()> {
    info!("Applying set_keys patch '{}': {} [{}] ({} keys)",
          patch_name, patch.target, patch.section, patch.keys.len());

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Set all keys with variable substitution
    for (key, value) in &patch.keys {
        let resolved_value = substitute_variables(value, context)?;
        ini.setstr(&patch.section, key, Some(&resolved_value));
    }

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied set_keys patch '{}' - set {} keys", patch_name, patch.keys.len());
    Ok(())
}

/// Apply an append_value patch directly to resources: appends a single value to an array (repeated key)
///
/// # Arguments
/// * `patch` - The append_value patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
/// * `context` - Substitution context for variable resolution
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_append_value_patch_direct(
    patch: &AppendValuePatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
    context: &SubstitutionContext,
) -> anyhow::Result<()> {
    info!("Applying append_value patch '{}': {} [{}] {} += {}",
          patch_name, patch.target, patch.section, patch.key, patch.value);

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Perform variable substitution on the value
    let resolved_value = substitute_variables(&patch.value, context)?;

    // Append the value (creates section if it doesn't exist)
    ini.addstr(&patch.section, &patch.key, &resolved_value);

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied append_value patch '{}'", patch_name);
    Ok(())
}

/// Apply an append_values patch directly to resources: appends multiple values to an array
///
/// # Arguments
/// * `patch` - The append_values patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
/// * `context` - Substitution context for variable resolution
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_append_values_patch_direct(
    patch: &AppendValuesPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
    context: &SubstitutionContext,
) -> anyhow::Result<()> {
    info!("Applying append_values patch '{}': {} [{}] {} += {} values",
          patch_name, patch.target, patch.section, patch.key, patch.values.len());

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Append all values with variable substitution
    for value in &patch.values {
        let resolved_value = substitute_variables(value, context)?;
        ini.addstr(&patch.section, &patch.key, &resolved_value);
    }

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied append_values patch '{}' - appended {} values",
          patch_name, patch.values.len());
    Ok(())
}

/// Apply a remove_key patch directly to resources: removes a single key from an INI section
///
/// # Arguments
/// * `patch` - The remove_key patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully (warnings logged if key doesn't exist)
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_remove_key_patch_direct(
    patch: &RemoveKeyPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    info!("Applying remove_key patch '{}': {} [{}] remove key '{}'",
          patch_name, patch.target, patch.section, patch.key);

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Try to remove the key
    let removed = ini.remove_key(&patch.section, &patch.key);

    if removed.is_none() {
        warn!("Remove_key patch '{}': key '{}' not found in section '{}' of '{}'",
              patch_name, patch.key, patch.section, patch.target);
        return Ok(());
    }

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied remove_key patch '{}'", patch_name);
    Ok(())
}

/// Apply a remove_keys patch directly to resources: removes multiple keys from an INI section
///
/// # Arguments
/// * `patch` - The remove_keys patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully (warnings logged for missing keys)
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_remove_keys_patch_direct(
    patch: &RemoveKeysPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    info!("Applying remove_keys patch '{}': {} [{}] remove {} keys",
          patch_name, patch.target, patch.section, patch.keys.len());

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Remove all keys, tracking successes
    let mut removed_count = 0;
    for key in &patch.keys {
        let removed = ini.remove_key(&patch.section, key);
        if removed.is_some() {
            removed_count += 1;
        } else {
            warn!("Remove_keys patch '{}': key '{}' not found in section '{}'",
                  patch_name, key, patch.section);
        }
    }

    if removed_count == 0 {
        warn!("Remove_keys patch '{}': no keys were removed (all keys not found)", patch_name);
        return Ok(());
    }

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied remove_keys patch '{}' - removed {} of {} keys",
          patch_name, removed_count, patch.keys.len());
    Ok(())
}

/// Apply an add_section patch directly to resources: adds a new section with keys to an INI file
///
/// # Arguments
/// * `patch` - The add_section patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
/// * `context` - Substitution context for variable resolution
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the file doesn't exist, isn't an INI file, section collision occurs with on_exists=error, or other errors
fn apply_add_section_patch_direct(
    patch: &AddSectionPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
    context: &SubstitutionContext,
) -> anyhow::Result<()> {
    info!("Applying add_section patch '{}': {} [{}] with {} keys (on_exists: {:?})",
          patch_name, patch.target, patch.section, patch.keys.len(), patch.on_exists);

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Check if section already exists
    let section_exists = ini.has_section(&patch.section);

    if section_exists {
        match patch.on_exists {
            OnExists::Error => {
                anyhow::bail!(
                    "Add_section patch '{}': section '{}' already exists in '{}' (on_exists=error)",
                    patch_name, patch.section, patch.target
                );
            }
            OnExists::Skip => {
                warn!("Add_section patch '{}': section '{}' already exists, skipping (on_exists=skip)",
                      patch_name, patch.section);
                return Ok(());
            }
            OnExists::Merge => {
                info!("Add_section patch '{}': section '{}' exists, merging keys (on_exists=merge)",
                      patch_name, patch.section);
                // Fall through to add keys (they will merge with existing)
            }
            OnExists::Replace => {
                info!("Add_section patch '{}': section '{}' exists, replacing (on_exists=replace)",
                      patch_name, patch.section);
                // Clear the section first
                ini.clear_section(&patch.section);
            }
        }
    }

    // Add all keys to the section with variable substitution
    for (key, value) in &patch.keys {
        let resolved_value = substitute_variables(value, context)?;
        ini.setstr(&patch.section, key, Some(&resolved_value));
    }

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied add_section patch '{}' - {} keys added/merged",
          patch_name, patch.keys.len());
    Ok(())
}

/// Apply a clear_section patch directly to resources: removes all keys from an INI section (keeps section)
///
/// # Arguments
/// * `patch` - The clear_section patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully (warnings logged if section doesn't exist)
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_clear_section_patch_direct(
    patch: &ClearSectionPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    info!("Applying clear_section patch '{}': {} [{}]",
          patch_name, patch.target, patch.section);

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Check if section exists
    if !ini.has_section(&patch.section) {
        warn!("Clear_section patch '{}': section '{}' not found in '{}'",
              patch_name, patch.section, patch.target);
        return Ok(());
    }

    // Clear the section
    ini.clear_section(&patch.section);

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied clear_section patch '{}'", patch_name);
    Ok(())
}

/// Apply a remove_section patch directly to resources: removes an entire section from an INI file
///
/// # Arguments
/// * `patch` - The remove_section patch configuration
/// * `mod_path` - Path to the current mod being loaded
/// * `patch_name` - Name of the patch (for logging)
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully (warnings logged if section doesn't exist)
/// * `Err(_)` if the file doesn't exist, isn't an INI file, or other errors occur
fn apply_remove_section_patch_direct(
    patch: &RemoveSectionPatch,
    _file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    info!("Applying remove_section patch '{}': {} [{}]",
          patch_name, patch.target, patch.section);

    // Load INI file
    let mut ini = load_ini_from_resources(&patch.target)?;

    // Try to remove the section
    let removed = ini.remove_section(&patch.section);

    if removed.is_none() {
        warn!("Remove_section patch '{}': section '{}' not found in '{}'",
              patch_name, patch.section, patch.target);
        return Ok(());
    }

    // Save back to resources
    save_ini_to_resources(&patch.target, &ini, current_mod_id)?;

    info!("Successfully applied remove_section patch '{}'", patch_name);
    Ok(())
}

/// Resolve a source file for patch operations
///
/// Looks up source files in the file_map (from .ztd archive) with "resources/" prefix.
/// Errors if not found in archive.
///
/// # Arguments
/// * `source` - Relative path to the source file
/// * `file_map` - HashMap of files from the .ztd archive
///
/// # Returns
/// * `Ok(Vec<u8>)` - File contents
/// * `Err(_)` - File not found
fn resolve_source_file(
    source: &str,
    file_map: &HashMap<String, Box<[u8]>>,
) -> anyhow::Result<Vec<u8>> {
    let archive_path = format!("resources/{}", source);
    file_map.get(&archive_path)
        .map(|data| data.to_vec())
        .ok_or_else(|| anyhow::anyhow!(
            "Source file '{}' not found in archive (expected as 'resources/{}')",
            source, source
        ))
}

// ============================================================================
// Phase 5: Patch Dispatchers
// ============================================================================

/// Apply a single patch directly to resources (no shadow)
///
/// # Arguments
/// * `patch` - The patch to apply
/// * `file_map` - HashMap of files from the .ztd archive
/// * `patch_name` - Name of the patch (for logging)
/// * `current_mod_id` - The ID of the current mod (for tracking resource sources)
/// * `context` - Substitution context for variable resolution
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the patch failed
fn apply_single_patch_direct(
    patch: &Patch,
    file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
    context: &SubstitutionContext,
) -> anyhow::Result<()> {
    match patch {
        Patch::Replace(p) => apply_replace_patch_direct(p, file_map, patch_name, current_mod_id),
        Patch::Merge(p) => apply_merge_patch_direct(p, file_map, patch_name, current_mod_id),
        Patch::Delete(p) => apply_delete_patch_direct(p, patch_name),
        Patch::SetPalette(p) => apply_set_palette_patch_direct(p, patch_name),
        Patch::SetKey(p) => apply_set_key_patch_direct(p, file_map, patch_name, current_mod_id, context),
        Patch::SetKeys(p) => apply_set_keys_patch_direct(p, file_map, patch_name, current_mod_id, context),
        Patch::AppendValue(p) => apply_append_value_patch_direct(p, file_map, patch_name, current_mod_id, context),
        Patch::AppendValues(p) => apply_append_values_patch_direct(p, file_map, patch_name, current_mod_id, context),
        Patch::RemoveKey(p) => apply_remove_key_patch_direct(p, file_map, patch_name, current_mod_id),
        Patch::RemoveKeys(p) => apply_remove_keys_patch_direct(p, file_map, patch_name, current_mod_id),
        Patch::AddSection(p) => apply_add_section_patch_direct(p, file_map, patch_name, current_mod_id, context),
        Patch::ClearSection(p) => apply_clear_section_patch_direct(p, file_map, patch_name, current_mod_id),
        Patch::RemoveSection(p) => apply_remove_section_patch_direct(p, file_map, patch_name, current_mod_id),
    }
}

/// Apply a single patch to shadow (for abort/abort_mod modes)
///
/// # Arguments
/// * `patch` - The patch to apply
/// * `file_map` - HashMap of files from the .ztd archive
/// * `patch_name` - Name of the patch (for logging)
/// * `current_mod_id` - The ID of the current mod (for tracking resource sources)
/// * `context` - Substitution context for variable resolution
/// * `shadow` - Shadow resources to apply patches to
///
/// # Returns
/// * `Ok(())` if the patch was applied successfully
/// * `Err(_)` if the patch failed
fn apply_single_patch_shadow(
    patch: &Patch,
    file_map: &HashMap<String, Box<[u8]>>,
    patch_name: &str,
    current_mod_id: &str,
    context: &SubstitutionContext,
    shadow: &mut ShadowResources,
) -> anyhow::Result<()> {
    match patch {
        Patch::Replace(p) => apply_replace_patch_shadow(p, file_map, patch_name, current_mod_id, shadow),
        Patch::Merge(p) => apply_merge_patch_shadow(p, file_map, patch_name, current_mod_id, shadow),
        Patch::Delete(p) => apply_delete_patch_shadow(p, patch_name, shadow),
        Patch::SetPalette(p) => apply_set_palette_patch_shadow(p, patch_name, shadow),
        Patch::SetKey(p) => apply_set_key_patch_shadow(p, file_map, patch_name, context, shadow),
        Patch::SetKeys(p) => apply_set_keys_patch_shadow(p, file_map, patch_name, context, shadow),
        Patch::AppendValue(p) => apply_append_value_patch_shadow(p, file_map, patch_name, context, shadow),
        Patch::AppendValues(p) => apply_append_values_patch_shadow(p, file_map, patch_name, context, shadow),
        Patch::RemoveKey(p) => apply_remove_key_patch_shadow(p, file_map, patch_name, shadow),
        Patch::RemoveKeys(p) => apply_remove_keys_patch_shadow(p, file_map, patch_name, shadow),
        Patch::AddSection(p) => apply_add_section_patch_shadow(p, file_map, patch_name, context, shadow),
        Patch::ClearSection(p) => apply_clear_section_patch_shadow(p, file_map, patch_name, shadow),
        Patch::RemoveSection(p) => apply_remove_section_patch_shadow(p, file_map, patch_name, shadow),
    }
}

// ============================================================================
// Phase 6: Patch Orchestration, Conditional Evaluation, and Error Handling
// ============================================================================

/// Helper function to check if a mod is loaded
///
/// # Arguments
/// * `mod_id` - The mod ID to check
///
/// # Returns
/// * `true` if the mod is loaded, `false` otherwise
fn is_mod_loaded(mod_id: &str) -> bool {
    let loaded_mods = get_mod_ids();
    loaded_mods.iter().any(|id| id == mod_id)
}

/// Check if a ZTD was loaded before the current mod
///
/// # Arguments
/// * `ztd_filename` - The ZTD filename to check
/// * `current_mod_id` - The ID of the current mod
///
/// # Returns
/// * `true` if the ZTD was enabled and loaded before the current mod
/// * `false` otherwise
fn is_ztd_loaded_before_current(ztd_filename: &str, current_mod_id: &str) -> bool {
    use crate::resource_manager::openzt_mods::ztd_registry;

    let current_ztd = match ztd_registry::get_mod_ztd(current_mod_id) {
        Some(ztd) => ztd,
        None => {
            warn!("Cannot check ztd_loaded condition: current mod '{}' has no registered ZTD",
                  current_mod_id);
            return false;
        }
    };

    let current_position = match ztd_registry::get_ztd_position(&current_ztd) {
        Some(pos) => pos,
        None => {
            warn!("Cannot check ztd_loaded condition: current ZTD '{}' not found in registry",
                  current_ztd);
            return false;
        }
    };

    ztd_registry::is_ztd_loaded_before(ztd_filename, current_position)
}

/// Evaluate patch-level conditions that require target file access
///
/// # Arguments
/// * `condition` - The condition to evaluate
/// * `default_target` - Default target file path (used if condition.target is not specified)
/// * `patch_name` - Name of the patch (for logging)
/// * `current_mod_id` - The ID of the current mod (for ztd_loaded condition)
///
/// # Returns
/// * `Ok(true)` if all conditions pass
/// * `Ok(false)` if any condition fails
/// * `Err(_)` if there's an error evaluating conditions
fn evaluate_patch_condition_with_target(
    condition: &Option<PatchCondition>,
    default_target: &str,
    patch_name: &str,
    current_mod_id: &str,
) -> anyhow::Result<bool> {
    let Some(cond) = condition else {
        return Ok(true);
    };

    // Use condition.target if specified, otherwise use default_target
    let target = cond.target.as_deref().unwrap_or(default_target);

    // First check mod_loaded condition (doesn't require target)
    if let Some(required_mod) = &cond.mod_loaded {
        if !is_mod_loaded(required_mod) {
            info!("Patch '{}': skipping - required mod '{}' not loaded", patch_name, required_mod);
            return Ok(false);
        }
    }

    // Check ztd_loaded condition
    if let Some(required_ztd) = &cond.ztd_loaded {
        if !is_ztd_loaded_before_current(required_ztd, current_mod_id) {
            info!("Patch '{}': skipping - required ZTD '{}' not loaded before current mod",
                  patch_name, required_ztd);
            return Ok(false);
        }
    }

    // Check entity_exists condition (legacy entities only)
    if let Some(entity_id) = &cond.entity_exists {
        if !crate::resource_manager::openzt_mods::entity_lookup::entity_exists(entity_id) {
            info!("Patch '{}': skipping - required legacy entity '{}' not loaded",
                  patch_name, entity_id);
            return Ok(false);
        }
    }

    // Check key_exists condition
    if let Some(key_check) = &cond.key_exists {
        // Check if target file exists
        if !check_file(target) {
            warn!("Patch '{}': cannot evaluate key_exists condition - target file '{}' not found",
                  patch_name, target);
            return Ok(false);
        }

        // Try to load as INI file
        match load_ini_from_resources(target) {
            Ok(ini) => {
                if ini.get(&key_check.section, &key_check.key).is_none() {
                    info!("Patch '{}': skipping - key '[{}]{}' does not exist in '{}'",
                          patch_name, key_check.section, key_check.key, target);
                    return Ok(false);
                }
            }
            Err(e) => {
                warn!("Patch '{}': cannot evaluate key_exists condition - failed to load target '{}': {}",
                      patch_name, target, e);
                return Ok(false);
            }
        }
    }

    // Check value_equals condition
    if let Some(value_check) = &cond.value_equals {
        // Check if target file exists
        if !check_file(target) {
            warn!("Patch '{}': cannot evaluate value_equals condition - target file '{}' not found",
                  patch_name, target);
            return Ok(false);
        }

        // Try to load as INI file
        match load_ini_from_resources(target) {
            Ok(ini) => {
                let actual_value = ini.get(&value_check.section, &value_check.key);
                if actual_value.as_deref() != Some(&value_check.value) {
                    info!("Patch '{}': skipping - key '[{}]{}' value does not equal '{}' (actual: {:?})",
                          patch_name, value_check.section, value_check.key, value_check.value, actual_value);
                    return Ok(false);
                }
            }
            Err(e) => {
                warn!("Patch '{}': cannot evaluate value_equals condition - failed to load target '{}': {}",
                      patch_name, target, e);
                return Ok(false);
            }
        }
    }

    Ok(true)
}

/// Get the target file path from a patch (for condition evaluation)
fn get_patch_target(patch: &Patch) -> &str {
    match patch {
        Patch::Replace(p) => &p.target,
        Patch::Merge(p) => &p.target,
        Patch::Delete(p) => &p.target,
        Patch::SetPalette(p) => &p.target,
        Patch::SetKey(p) => &p.target,
        Patch::SetKeys(p) => &p.target,
        Patch::AppendValue(p) => &p.target,
        Patch::AppendValues(p) => &p.target,
        Patch::RemoveKey(p) => &p.target,
        Patch::RemoveKeys(p) => &p.target,
        Patch::AddSection(p) => &p.target,
        Patch::ClearSection(p) => &p.target,
        Patch::RemoveSection(p) => &p.target,
    }
}

/// Get the condition from a patch (for condition evaluation)
fn get_patch_condition(patch: &Patch) -> &Option<PatchCondition> {
    match patch {
        Patch::Replace(p) => &p.condition,
        Patch::Merge(p) => &p.condition,
        Patch::Delete(p) => &p.condition,
        Patch::SetPalette(p) => &p.condition,
        Patch::SetKey(p) => &p.condition,
        Patch::SetKeys(p) => &p.condition,
        Patch::AppendValue(p) => &p.condition,
        Patch::AppendValues(p) => &p.condition,
        Patch::RemoveKey(p) => &p.condition,
        Patch::RemoveKeys(p) => &p.condition,
        Patch::AddSection(p) => &p.condition,
        Patch::ClearSection(p) => &p.condition,
        Patch::RemoveSection(p) => &p.condition,
    }
}

/// Result of applying a single patch
#[derive(Debug, Clone, PartialEq)]
enum PatchResult {
    Success,
    Skipped,           // Condition failed or warning situation
    Error(String),     // Error occurred
}

/// Apply patches directly without shadow (continue mode)
///
/// In this mode, patches are applied directly to the resource system.
/// If a patch fails, it's logged but execution continues with the next patch.
///
/// # Arguments
/// * `patch_meta` - Patch metadata containing error handling and file-level conditions
/// * `patches` - Ordered map of patches to apply (order is preserved via IndexMap)
/// * `file_map` - HashMap of files from the .ztd archive
/// * `current_mod_id` - The ID of the current mod (for variable substitution)
///
/// # Returns
/// * `Ok(())` always (errors are logged but don't stop execution)
fn apply_patches_direct(
    patch_meta: &PatchMeta,
    patches: &indexmap::IndexMap<String, Patch>,
    file_map: &HashMap<String, Box<[u8]>>,
    current_mod_id: &str,
) -> anyhow::Result<()> {

    // Create substitution context for variable resolution
    let context = SubstitutionContext {
        current_mod_id: current_mod_id.to_string(),
    };

    info!("Applying patch file with {} patches (on_error: continue)",
          patches.len());

    // Evaluate top-level conditions
    if let Some(top_level_condition) = &patch_meta.condition {
        // Check mod_loaded at file level
        if let Some(required_mod) = &top_level_condition.mod_loaded {
            if !is_mod_loaded(required_mod) {
                warn!("Patch file skipped - required mod '{}' not loaded", required_mod);
                return Ok(());
            }
        }

        // Check ztd_loaded at file level
        if let Some(required_ztd) = &top_level_condition.ztd_loaded {
            if !is_ztd_loaded_before_current(required_ztd, current_mod_id) {
                warn!("Patch file skipped - required ZTD '{}' not loaded before current mod", required_ztd);
                return Ok(());
            }
        }

        // Check entity_exists at file level
        if let Some(entity_id) = &top_level_condition.entity_exists {
            if !crate::resource_manager::openzt_mods::entity_lookup::entity_exists(entity_id) {
                warn!("Patch file skipped - required legacy entity '{}' not loaded", entity_id);
                return Ok(());
            }
        }

        // Check key_exists and value_equals with target
        if top_level_condition.key_exists.is_some() || top_level_condition.value_equals.is_some() {
            let Some(target) = &top_level_condition.target else {
                return Err(anyhow::anyhow!(
                    "Top-level condition with key_exists/value_equals requires 'target' field"
                ));
            };

            // Use existing evaluation function with target
            if !evaluate_patch_condition_with_target(&Some(top_level_condition.clone()), target, "top-level", current_mod_id)? {
                warn!("Patch file skipped - top-level conditions failed");
                return Ok(());
            }
        }
    }

    // Apply patches in order
    for (patch_name, patch) in patches {
        info!("Processing patch '{}'", patch_name);

        // Evaluate patch-level conditions
        let target = get_patch_target(patch);
        let condition = get_patch_condition(patch);

        match evaluate_patch_condition_with_target(condition, target, patch_name, current_mod_id) {
            Ok(true) => {
                // Condition passed, apply patch
                let result = apply_single_patch_direct(patch, file_map, patch_name, current_mod_id, &context);

                if let Err(e) = result {
                    error!("Patch '{}' failed: {}. Continuing.", patch_name, e);
                }
            }
            Ok(false) => {
                // Condition failed, skip patch
                info!("Patch '{}': skipping (condition failed)", patch_name);
            }
            Err(e) => {
                // Error evaluating condition
                error!("Patch '{}': error evaluating condition: {}. Continuing.", patch_name, e);
            }
        }
    }

    info!("Patch application complete (continue mode)");
    Ok(())
}

/// Apply patches with shadow resources (abort/abort_mod modes)
///
/// In this mode, patches are applied to shadow copies of files.
/// If any patch fails, the shadow is discarded (automatic rollback).
/// If all patches succeed, the shadow is committed to the resource system.
///
/// # Arguments
/// * `patch_meta` - Patch metadata containing error handling and file-level conditions
/// * `patches` - Ordered map of patches to apply (order is preserved via IndexMap)
/// * `file_map` - HashMap of files from the .ztd archive
/// * `current_mod_id` - The ID of the current mod (for variable substitution)
///
/// # Returns
/// * `Ok(())` if all patches succeeded and shadow was committed
/// * `Err(_)` if any patch failed (shadow is automatically discarded)
fn apply_patches_with_shadow(
    patch_meta: &PatchMeta,
    patches: &indexmap::IndexMap<String, Patch>,
    file_map: &HashMap<String, Box<[u8]>>,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    // Create substitution context for variable resolution
    let context = SubstitutionContext {
        current_mod_id: current_mod_id.to_string(),
    };

    info!("Applying patch file with {} patches (on_error: {:?})",
          patches.len(), patch_meta.on_error);

    // Evaluate top-level conditions
    if let Some(top_level_condition) = &patch_meta.condition {
        // Check mod_loaded at file level
        if let Some(required_mod) = &top_level_condition.mod_loaded {
            if !is_mod_loaded(required_mod) {
                warn!("Patch file skipped - required mod '{}' not loaded", required_mod);
                return Ok(());
            }
        }

        // Check ztd_loaded at file level
        if let Some(required_ztd) = &top_level_condition.ztd_loaded {
            if !is_ztd_loaded_before_current(required_ztd, current_mod_id) {
                warn!("Patch file skipped - required ZTD '{}' not loaded before current mod", required_ztd);
                return Ok(());
            }
        }

        // Check entity_exists at file level
        if let Some(entity_id) = &top_level_condition.entity_exists {
            if !crate::resource_manager::openzt_mods::entity_lookup::entity_exists(entity_id) {
                warn!("Patch file skipped - required legacy entity '{}' not loaded", entity_id);
                return Ok(());
            }
        }

        // Check key_exists and value_equals with target
        if top_level_condition.key_exists.is_some() || top_level_condition.value_equals.is_some() {
            let Some(target) = &top_level_condition.target else {
                return Err(anyhow::anyhow!(
                    "Top-level condition with key_exists/value_equals requires 'target' field"
                ));
            };

            // Use existing evaluation function with target
            if !evaluate_patch_condition_with_target(&Some(top_level_condition.clone()), target, "top-level", current_mod_id)? {
                warn!("Patch file skipped - top-level conditions failed");
                return Ok(());
            }
        }
    }

    // Collect affected files and create shadow
    let affected_files = collect_affected_files(patches);

    let scope = match patch_meta.on_error {
        ErrorHandling::Abort => ShadowScope::PatchFile,
        ErrorHandling::AbortMod => ShadowScope::Mod,
        _ => unreachable!("apply_patches_with_shadow called with Continue mode"),
    };

    let mut shadow = ShadowResources::new(&affected_files, scope)?;

    // Apply patches to shadow
    for (patch_name, patch) in patches {
        info!("Processing patch '{}'", patch_name);

        // Evaluate patch-level conditions
        let target = get_patch_target(patch);
        let condition = get_patch_condition(patch);

        match evaluate_patch_condition_with_target(condition, target, patch_name, current_mod_id) {
            Ok(true) => {
                // Condition passed, apply patch to shadow
                let result = apply_single_patch_shadow(patch, file_map, patch_name, current_mod_id, &context, &mut shadow);

                if let Err(e) = result {
                    error!("Patch '{}' failed: {}. Rolling back.", patch_name, e);
                    shadow.discard();
                    return Err(e);
                }
            }
            Ok(false) => {
                // Condition failed, skip patch
                info!("Patch '{}': skipping (condition failed)", patch_name);
            }
            Err(e) => {
                // Error evaluating condition
                error!("Patch '{}': error evaluating condition: {}. Rolling back.", patch_name, e);
                shadow.discard();
                return Err(e);
            }
        }
    }

    // All patches succeeded - commit shadow to main resources
    shadow.commit()?;

    info!("All patches applied successfully and committed");
    Ok(())
}

/// Apply all patches with error handling and conditional evaluation
///
/// This is the main entry point for patch application. It routes to either
/// direct mode (continue) or shadow mode (abort/abort_mod) based on the
/// error handling strategy specified in patch_meta.
///
/// # Arguments
/// * `patch_meta` - Patch metadata containing error handling and file-level conditions
/// * `patches` - Ordered map of patches to apply (order is preserved via IndexMap)
/// * `file_map` - HashMap of files from the .ztd archive
/// * `current_mod_id` - The ID of the current mod (for variable substitution)
///
/// # Returns
/// * `Ok(())` if patches were applied successfully
/// * `Err(_)` if on_error=abort or on_error=abort_mod and an error occurred
pub fn apply_patches(
    patch_meta: &PatchMeta,
    patches: &indexmap::IndexMap<String, Patch>,
    file_map: &HashMap<String, Box<[u8]>>,
    current_mod_id: &str,
) -> anyhow::Result<()> {
    // Route based on error handling mode
    match patch_meta.on_error {
        ErrorHandling::Continue => {
            // Direct mode - no shadow, patches applied directly
            apply_patches_direct(patch_meta, patches, file_map, current_mod_id)
        }
        ErrorHandling::Abort | ErrorHandling::AbortMod => {
            // Shadow mode - patches applied to shadow, committed on success
            apply_patches_with_shadow(patch_meta, patches, file_map, current_mod_id)
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variable_current_mod_habitat() {
        let result = parse_variable("habitats.swamp").unwrap();
        assert_eq!(result.var_type, VariableType::Habitats);
        assert_eq!(result.mod_id, None);
        assert_eq!(result.identifier, "swamp");
    }

    #[test]
    fn test_parse_variable_current_mod_location() {
        let result = parse_variable("locations.moon").unwrap();
        assert_eq!(result.var_type, VariableType::Locations);
        assert_eq!(result.mod_id, None);
        assert_eq!(result.identifier, "moon");
    }

    #[test]
    fn test_parse_variable_current_mod_string() {
        let result = parse_variable("strings.9500").unwrap();
        assert_eq!(result.var_type, VariableType::Strings);
        assert_eq!(result.mod_id, None);
        assert_eq!(result.identifier, "9500");
    }

    #[test]
    fn test_parse_variable_cross_mod_habitat() {
        let result = parse_variable("lunar.habitats.crater").unwrap();
        assert_eq!(result.var_type, VariableType::Habitats);
        assert_eq!(result.mod_id, Some("lunar".to_string()));
        assert_eq!(result.identifier, "crater");
    }

    #[test]
    fn test_parse_variable_cross_mod_location() {
        let result = parse_variable("lunar.locations.moon").unwrap();
        assert_eq!(result.var_type, VariableType::Locations);
        assert_eq!(result.mod_id, Some("lunar".to_string()));
        assert_eq!(result.identifier, "moon");
    }

    #[test]
    fn test_parse_variable_invalid_syntax_too_few_parts() {
        let result = parse_variable("habitat");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid variable syntax"));
    }

    #[test]
    fn test_parse_variable_invalid_syntax_too_many_parts() {
        let result = parse_variable("mod.habitats.name.extra");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid variable syntax"));
    }

    #[test]
    fn test_parse_variable_invalid_type() {
        let result = parse_variable("invalid.swamp");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid variable type"));
    }

    #[test]
    fn test_parse_variable_invalid_type_cross_mod() {
        let result = parse_variable("lunar.invalid.moon");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid variable type"));
    }

    #[test]
    fn test_substitute_variables_no_variables() {
        let context = SubstitutionContext {
            current_mod_id: "test_mod".to_string(),
        };
        let result = substitute_variables("plain text", &context).unwrap();
        assert_eq!(result, "plain text");
    }

    #[test]
    fn test_substitute_variables_single_variable() {
        let context = SubstitutionContext {
            current_mod_id: "test_mod".to_string(),
        };
        // This would fail without registered habitats, but tests the parsing
        let input = "{habitats.swamp}";
        let result = substitute_variables(input, &context);
        // Will fail because habitat not registered, but that's expected
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // Error should mention that it failed to resolve the variable
        assert!(
            error_msg.contains("Failed to resolve variable"),
            "Expected error to mention 'Failed to resolve variable', got: {}",
            error_msg
        );
    }

    #[test]
    fn test_substitute_variables_multiple_variables() {
        let context = SubstitutionContext {
            current_mod_id: "test_mod".to_string(),
        };
        let input = "cHabitat={habitats.swamp}, cLocation={locations.moon}";
        let result = substitute_variables(input, &context);
        // Will fail because habitats not registered, but validates parsing multiple variables
        assert!(result.is_err());
    }

    #[test]
    fn test_substitute_variables_mixed_content() {
        let context = SubstitutionContext {
            current_mod_id: "test_mod".to_string(),
        };
        let input = "prefix {habitats.swamp} middle {locations.moon} suffix";
        let result = substitute_variables(input, &context);
        // Will fail because habitats not registered, but validates mixed content parsing
        assert!(result.is_err());
    }

    #[test]
    fn test_substitute_variables_unclosed_brace() {
        let context = SubstitutionContext {
            current_mod_id: "test_mod".to_string(),
        };
        let input = "text {habitats.swamp";
        let result = substitute_variables(input, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unclosed variable brace"));
    }

    #[test]
    fn test_substitute_variables_empty_braces() {
        let context = SubstitutionContext {
            current_mod_id: "test_mod".to_string(),
        };
        let input = "text {} more";
        let result = substitute_variables(input, &context);
        // Will fail due to invalid syntax
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_affected_files() {
        let mut patches = indexmap::IndexMap::new();

        patches.insert(
            "patch1".to_string(),
            Patch::SetKey(SetKeyPatch {
                target: "file1.ini".to_string(),
                section: "Section".to_string(),
                key: "Key".to_string(),
                value: "Value".to_string(),
                condition: None,
            }),
        );
        patches.insert(
            "patch2".to_string(),
            Patch::SetKey(SetKeyPatch {
                target: "file1.ini".to_string(),
                section: "Section".to_string(),
                key: "Key2".to_string(),
                value: "Value2".to_string(),
                condition: None,
            }),
        );
        patches.insert(
            "patch3".to_string(),
            Patch::Delete(DeletePatch {
                target: "file2.ini".to_string(),
                condition: None,
            }),
        );

        let affected = collect_affected_files(&patches);

        // Should collect unique file paths
        assert_eq!(affected.len(), 2, "Should find 2 unique files");
        assert!(affected.contains("file1.ini"), "Should contain file1.ini");
        assert!(affected.contains("file2.ini"), "Should contain file2.ini");
    }

    #[test]
    fn test_shadow_resources_update_file() {
        // Create shadow
        let mut shadow = ShadowResources::new(&HashSet::new(), ShadowScope::PatchFile).unwrap();

        // Create a test file
        let test_file = "test_update.ini";
        let content = "[Section]\nKey = Value\n";
        let content_len = content.len() as u32;
        let c_string = std::ffi::CString::new(content).unwrap();
        let ztfile = ZTFile::Text(c_string, ZTFileType::Ini, content_len);

        // Update file in shadow
        shadow.update_file(test_file, ztfile);

        // Verify file is in shadow
        let file = shadow.get_file(test_file);
        assert!(file.is_some(), "File should be in shadow");
    }

    // =========================================================================
    // Tests for legacy variable syntax (3, 4, and 5-part)
    // =========================================================================

    #[test]
    fn test_parse_variable_legacy_3_part_default_subtype() {
        // Format: {legacy.type.name} - uses default subtype for entity type
        let result = parse_variable("legacy.animals.elephant").unwrap();
        assert_eq!(result.var_type, VariableType::Legacy);
        assert_eq!(result.mod_id, None);
        assert_eq!(result.identifier, "elephant");

        let legacy_parts = result.legacy_parts.unwrap();
        assert_eq!(legacy_parts.entity_type, LegacyEntityType::Animal);
        assert_eq!(legacy_parts.entity_name, "elephant");
        assert_eq!(legacy_parts.subtype, None); // Will use default subtype
        assert_eq!(legacy_parts.attribute, "name_id");
    }

    #[test]
    fn test_parse_variable_legacy_4_part_explicit_attribute() {
        // Format: {legacy.type.name.attribute} - explicit attribute, default subtype
        let result = parse_variable("legacy.buildings.restroom.name_id").unwrap();
        assert_eq!(result.var_type, VariableType::Legacy);
        assert_eq!(result.mod_id, None);
        assert_eq!(result.identifier, "restroom");

        let legacy_parts = result.legacy_parts.unwrap();
        assert_eq!(legacy_parts.entity_type, LegacyEntityType::Building);
        assert_eq!(legacy_parts.entity_name, "restroom");
        assert_eq!(legacy_parts.subtype, None); // Use default
        assert_eq!(legacy_parts.attribute, "name_id");
    }

    #[test]
    fn test_parse_variable_legacy_5_part_explicit_subtype() {
        // Format: {legacy.type.name.subtype.attribute} - explicit subtype
        let result = parse_variable("legacy.animals.elephant.f.name_id").unwrap();
        assert_eq!(result.var_type, VariableType::Legacy);
        assert_eq!(result.mod_id, None);
        assert_eq!(result.identifier, "elephant");

        let legacy_parts = result.legacy_parts.unwrap();
        assert_eq!(legacy_parts.entity_type, LegacyEntityType::Animal);
        assert_eq!(legacy_parts.entity_name, "elephant");
        assert_eq!(legacy_parts.subtype, Some("f".to_string())); // Explicit female subtype
        assert_eq!(legacy_parts.attribute, "name_id");
    }

    #[test]
    fn test_parse_variable_legacy_5_part_guest_subtype() {
        // Guests have no default, must specify subtype
        let result = parse_variable("legacy.guests.guest.man.name_id").unwrap();
        assert_eq!(result.var_type, VariableType::Legacy);

        let legacy_parts = result.legacy_parts.unwrap();
        assert_eq!(legacy_parts.entity_type, LegacyEntityType::Guest);
        assert_eq!(legacy_parts.entity_name, "guest");
        assert_eq!(legacy_parts.subtype, Some("man".to_string()));
        assert_eq!(legacy_parts.attribute, "name_id");
    }

    #[test]
    fn test_parse_variable_legacy_4_part_invalid_attribute() {
        // 4-part with non-name_id attribute should fail
        let result = parse_variable("legacy.animals.elephant.f");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("expected") || error_msg.contains("invalid"),
            "Expected error about invalid syntax, got: {}",
            error_msg
        );
    }

    #[test]
    fn test_parse_variable_legacy_invalid_entity_type() {
        let result = parse_variable("legacy.invalid_type.entity.name_id");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid legacy entity type") || error_msg.contains("invalid"),
            "Expected error about invalid entity type, got: {}",
            error_msg
        );
    }

    #[test]
    fn test_parse_variable_legacy_too_many_parts() {
        let result = parse_variable("legacy.animals.elephant.f.name_id.extra");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid variable syntax"),
            "Expected error about invalid syntax, got: {}",
            error_msg
        );
    }

    #[test]
    fn test_parse_variable_legacy_too_few_parts() {
        // 2-part legacy input falls into the current_mod case, which doesn't recognize "legacy" as a type
        let result = parse_variable("legacy.animals");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // The error will be about "legacy" not being a valid variable type (habitat/location/string)
        assert!(
            error_msg.contains("Invalid variable type") || error_msg.contains("Invalid variable syntax"),
            "Expected error about invalid variable type or syntax, got: {}",
            error_msg
        );
    }

    #[test]
    fn test_parse_variable_legacy_fence_with_subtype() {
        // Fences use 'f' as default subtype
        let result = parse_variable("legacy.fences.atltank.name_id").unwrap();
        assert_eq!(result.var_type, VariableType::Legacy);

        let legacy_parts = result.legacy_parts.unwrap();
        assert_eq!(legacy_parts.entity_type, LegacyEntityType::Fence);
        assert_eq!(legacy_parts.entity_name, "atltank");
        assert_eq!(legacy_parts.subtype, None); // Will use default 'f'
        assert_eq!(legacy_parts.attribute, "name_id");
    }

    #[test]
    fn test_parse_variable_legacy_fence_explicit_glass_subtype() {
        // Explicit 'g' (glass) subtype for fence
        let result = parse_variable("legacy.fences.atltank.g.name_id").unwrap();
        assert_eq!(result.var_type, VariableType::Legacy);

        let legacy_parts = result.legacy_parts.unwrap();
        assert_eq!(legacy_parts.entity_type, LegacyEntityType::Fence);
        assert_eq!(legacy_parts.entity_name, "atltank");
        assert_eq!(legacy_parts.subtype, Some("g".to_string()));
        assert_eq!(legacy_parts.attribute, "name_id");
    }

    #[test]
    fn test_parse_variable_legacy_item_no_subtype() {
        // Items don't have subtypes
        let result = parse_variable("legacy.items.rock.name_id").unwrap();
        assert_eq!(result.var_type, VariableType::Legacy);

        let legacy_parts = result.legacy_parts.unwrap();
        assert_eq!(legacy_parts.entity_type, LegacyEntityType::Item);
        assert_eq!(legacy_parts.entity_name, "rock");
        assert_eq!(legacy_parts.subtype, None);
        assert_eq!(legacy_parts.attribute, "name_id");
    }

}
