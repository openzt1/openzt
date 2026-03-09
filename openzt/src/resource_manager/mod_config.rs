use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;
use tracing::info;

// Re-export LoggingConfig from the logging module
pub use crate::logging::LoggingConfig;

// Re-export TuiConfig from the tui_console module
#[cfg(feature = "tui")]
pub use crate::tui_console::TuiConfig;
/// OpenZT configuration file structure (openzt.toml)
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct OpenZTConfig {
    #[serde(default)]
    pub mod_loading: ModLoadingConfig,

    #[serde(default)]
    pub logging: LoggingConfig,

    #[serde(default)]
    pub resource_cache: ResourceCacheConfig,

    #[serde(default)]
    pub expansions: ExpansionConfig,

    #[serde(default)]
    pub dev: DevConfig,

    #[cfg(feature = "tui")]
    #[serde(default)]
    pub tui: TuiConfig,
}

/// Mod loading configuration section
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ModLoadingConfig {
    /// Explicit load order - mods are loaded in this sequence
    #[serde(default)]
    pub order: Vec<String>,

    /// Disabled mods and ZTD files
    /// - OpenZT mod IDs (e.g., "com.example.mod")
    /// - ZTD filenames ending in .ztd (e.g., "mymod.ztd")
    ///
    /// OpenZT mods in this list are skipped during loading.
    /// ZTD files in this list:
    /// - Files already loaded by other mods: skipped (not overwritten)
    /// - .cfg/.uca/.ucb/.ucs files: empty resources added (prevents vanilla loading)
    /// - Other file types: errors logged, vanilla will load them (configuration error)
    #[serde(default)]
    pub disabled: Vec<String>,

    /// Auto-resolve new mods (default: true)
    #[serde(default = "default_true")]
    pub auto_resolve_new_mods: bool,

    /// Warn on conflicts (default: true)
    #[serde(default = "default_true")]
    pub warn_on_conflicts: bool,
}

/// Resource cache configuration section
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ResourceCacheConfig {
    /// Maximum memory usage before unloading begins (in MB)
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u32,

    /// Target memory usage after unloading (in MB)
    #[serde(default = "default_target_memory_mb")]
    pub target_memory_mb: u32,

    /// Unload resources not accessed within this time (in seconds)
    #[serde(default = "default_stale_timeout_seconds")]
    pub stale_timeout_seconds: u64,
}

/// Custom expansions configuration section
///
/// Defines custom expansions for menu filtering in openzt.toml.
/// Each key is an expansion name, and the value is an array of entity names and/or .ztd archives.
///
/// Example:
/// ```toml
/// [expansions]
/// x = ["1", "2", "3", "1.ztd", "2.ztd"]
/// y = ["2", "3"]
/// z = ["4", "5"]
/// ```
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ExpansionConfig {
    /// Custom expansion definitions (flattened into [expansions] table)
    /// Key: expansion name, Value: array of entity names and/or .ztd archives
    #[serde(default, flatten)]
    pub custom: IndexMap<String, Vec<String>>,
}

/// Development tools configuration section
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DevConfig {
    /// Console listening address (default: "127.0.0.1:8080")
    #[serde(default = "default_console_listen")]
    pub console_listen: String,
}

fn default_true() -> bool {
    true
}

fn default_console_listen() -> String {
    "127.0.0.1:8080".to_string()
}

fn default_max_memory_mb() -> u32 {
    2048 // 2GB
}

fn default_target_memory_mb() -> u32 {
    1536 // 1.5GB
}

fn default_stale_timeout_seconds() -> u64 {
    300 // 5 minutes
}

impl Default for OpenZTConfig {
    fn default() -> Self {
        OpenZTConfig {
            mod_loading: ModLoadingConfig {
                order: Vec::new(),
                disabled: Vec::new(),
                auto_resolve_new_mods: true,
                warn_on_conflicts: true,
            },
            logging: LoggingConfig::default(),
            resource_cache: ResourceCacheConfig::default(),
            expansions: ExpansionConfig::default(),
            dev: DevConfig::default(),
            #[cfg(feature = "tui")]
            tui: TuiConfig::default(),
        }
    }
}

impl Default for ModLoadingConfig {
    fn default() -> Self {
        ModLoadingConfig {
            order: Vec::new(),
            disabled: Vec::new(),
            auto_resolve_new_mods: true,
            warn_on_conflicts: true,
        }
    }
}

impl Default for ResourceCacheConfig {
    fn default() -> Self {
        ResourceCacheConfig {
            max_memory_mb: 2048,
            target_memory_mb: 1536,
            stale_timeout_seconds: 300,
        }
    }
}

impl Default for ExpansionConfig {
    fn default() -> Self {
        ExpansionConfig { custom: IndexMap::new() }
    }
}

impl Default for DevConfig {
    fn default() -> Self {
        DevConfig {
            console_listen: "127.0.0.1:8080".to_string(),
        }
    }
}

// Global cached configuration
static CACHED_CONFIG: LazyLock<Mutex<OpenZTConfig>> = LazyLock::new(|| Mutex::new(load_openzt_config_from_disk()));

/// Load OpenZT configuration from openzt.toml
///
/// Location: <Zoo Tycoon Install>/openzt.toml
///
/// If file doesn't exist, creates it with default values and returns default config.
/// If file exists but is missing sections, adds missing sections with defaults.
/// If file fails to parse, returns default config without overwriting the file.
fn load_openzt_config_from_disk() -> OpenZTConfig {
    let config_path = get_config_path();

    if !config_path.exists() {
        eprintln!("No openzt.toml found, creating with default configuration");
        let default_config = OpenZTConfig::default();

        // Save default config to file (skip cache update - we're initializing it)
        if let Err(e) = save_openzt_config(&default_config, true) {
            eprintln!("Failed to create openzt.toml: {}", e);
        }

        return default_config;
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            // Check which sections and fields are present in the file
            let needs_update = match toml::from_str::<toml::Value>(&content) {
                Ok(toml_value) => {
                    // Check if sections exist
                    let has_mod_loading = toml_value.get("mod_loading").is_some();
                    let has_logging = toml_value.get("logging").is_some();
                    let has_resource_cache = toml_value.get("resource_cache").is_some();
                    let has_expansions = toml_value.get("expansions").is_some();
                    let has_dev = toml_value.get("dev").is_some();
                    #[cfg(feature = "tui")]
                    let has_tui = toml_value.get("tui").is_some();
                    #[cfg(not(feature = "tui"))]
                    let has_tui = false;

                    // Check if all fields exist within sections
                    let mod_loading_complete = if let Some(mod_loading) = toml_value.get("mod_loading") {
                        mod_loading.get("order").is_some()
                            && mod_loading.get("disabled").is_some()
                            && mod_loading.get("auto_resolve_new_mods").is_some()
                            && mod_loading.get("warn_on_conflicts").is_some()
                    } else {
                        false
                    };

                    let logging_complete = if let Some(logging) = toml_value.get("logging") {
                        logging.get("log_to_file").is_some()
                            && logging.get("level").is_some()
                            && logging.get("log_command_output").is_some()
                    } else {
                        false
                    };

                    let resource_cache_complete = if let Some(resource_cache) = toml_value.get("resource_cache") {
                        resource_cache.get("max_memory_mb").is_some()
                            && resource_cache.get("target_memory_mb").is_some()
                            && resource_cache.get("stale_timeout_seconds").is_some()
                    } else {
                        false
                    };

                    let dev_complete = if let Some(dev) = toml_value.get("dev") {
                        dev.get("console_listen").is_some()
                    } else {
                        false
                    };

                    #[cfg(feature = "tui")]
                    let tui_complete = if let Some(tui) = toml_value.get("tui") {
                        tui.get("enabled").is_some()
                            && tui.get("show_logs").is_some()
                            && tui.get("log_level").is_some()
                    } else {
                        false
                    };
                    #[cfg(not(feature = "tui"))]
                    let tui_complete = true;

                    // expansions section should always be present (even if empty)
                    // No field-level validation needed for expansions since it's a free-form HashMap

                    // Update needed if sections missing or fields incomplete
                    !has_mod_loading
                        || !has_logging
                        || !has_resource_cache
                        || !has_expansions
                        || !has_dev
                        || !mod_loading_complete
                        || !logging_complete
                        || !resource_cache_complete
                        || !dev_complete
                        || !has_tui
                        || !tui_complete
                }
                Err(_) => false, // If we can't parse as Value, the full parse will fail below
            };

            match toml::from_str::<OpenZTConfig>(&content) {
                Ok(config) => {
                    eprintln!("Loaded OpenZT configuration from openzt.toml");

                    // If sections or fields were missing, save the complete config with defaults
                    if needs_update {
                        eprintln!("Adding missing configuration sections/fields to openzt.toml");
                        if let Err(e) = save_openzt_config(&config, true) {
                            eprintln!("Failed to update openzt.toml with missing entries: {}", e);
                        }
                    }

                    config
                }
                Err(e) => {
                    eprintln!("Failed to parse openzt.toml: {}", e);
                    eprintln!();
                    eprintln!("Please fix the configuration file and restart the game.");
                    eprintln!();
                    eprintln!("Config file location: {}", config_path.display());

                    // Wait to allow user to read the error, then exit
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                    eprintln!("OpenZT will now exit.");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Could not read openzt.toml: {}", e);
            eprintln!();
            eprintln!("Please fix the file permissions and restart the game.");
            eprintln!();
            eprintln!("Config file location: {}", config_path.display());

            // Wait to allow user to read the error, then exit
            std::thread::sleep(std::time::Duration::from_secs(3600));
            eprintln!("OpenZT will now exit.");
            std::process::exit(1);
        }
    }
}

/// Get the cached OpenZT configuration
///
/// Returns a clone of the cached config. The config is loaded once on startup
/// and cached in memory for fast access.
pub fn get_openzt_config() -> OpenZTConfig {
    let config = CACHED_CONFIG.lock().unwrap();
    config.clone()
}

/// Save OpenZT configuration to openzt.toml
///
/// Uses atomic write (temp file + rename) to prevent corruption
///
/// # Arguments
/// * `config` - The configuration to save
/// * `skip_cache_update` - If true, skip updating the in-memory cache (use during initialization)
pub fn save_openzt_config(config: &OpenZTConfig, skip_cache_update: bool) -> anyhow::Result<()> {
    let config_path = get_config_path();
    let temp_path = get_temp_config_path();

    // Serialize with pretty formatting
    let toml_string = toml::to_string_pretty(config).map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

    // Build the content with header comment
    let mut content = format!(
        "# OpenZT Configuration File\n\
         # This file controls mod loading order, logging, and other behavior\n\
         # Generated automatically - edit with caution\n\n{}",
        toml_string
    );

    // Add expansions section with comments if no custom expansions are defined yet
    if config.expansions.custom.is_empty() {
        // Replace the empty [expansions] section with the commented version
        if content.contains("[expansions]") {
            // Remove the empty section (it may be just "[expansions]" or have whitespace)
            content = content.replace("\n[expansions]\n", "");
            content = content.replace("[expansions]\n", "");
        }
        // Append the expansions section with comments
        let expansions_comment = "\n\
# Custom Expansions\n\
# Define custom expansions for menu filtering in the game\n\
# Format: expansion_name = [\"entity1\", \"entity2\", \"archive.ztd\"]\n\
# - Archives (files ending in .ztd): All entities from that archive are included\n\
# - Entity names: Specific entities to include (e.g., \"elephant\", \"lion\")\n\
# - For uca/ucb/ucs files use the filename without the extension (e.g., \"a2f21026\" for \"a2f21026.uca\")\n\
#\n\
# Example:\n\
# \"My animals\" = [\"elephant\", \"lion\", \"my_mod.ztd\"]\n\
# \"More Stuff\" = [\"zebra\", \"giraffe\"]\n\
[expansions]\n";
        content.push_str(expansions_comment);
    }

    // Write to temp file
    std::fs::write(&temp_path, content).map_err(|e| anyhow::anyhow!("Failed to write temp config: {}", e))?;

    // Atomic rename (overwrites existing on Windows)
    std::fs::rename(&temp_path, &config_path).map_err(|e| anyhow::anyhow!("Failed to rename temp config: {}", e))?;

    info!("Updated openzt.toml with new configuration");

    // Update the cached config (only if not during initialization)
    if !skip_cache_update {
        let mut cached = CACHED_CONFIG.lock().unwrap();
        *cached = config.clone();
    }

    Ok(())
}

/// Get path to openzt.toml
fn get_config_path() -> PathBuf {
    crate::util::get_base_path().join("openzt.toml")
}

/// Get path to temporary config file for atomic writes
fn get_temp_config_path() -> PathBuf {
    crate::util::get_base_path().join("openzt.toml.tmp")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::LogLevel;

    #[test]
    fn test_default_config() {
        let config = OpenZTConfig::default();
        assert!(config.mod_loading.order.is_empty());
        assert!(config.mod_loading.disabled.is_empty());
        assert!(config.mod_loading.auto_resolve_new_mods);
        assert!(config.mod_loading.warn_on_conflicts);
        assert!(config.logging.log_to_file);
        assert_eq!(config.logging.level, LogLevel::Warn);
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut config = OpenZTConfig::default();
        config.mod_loading.order = vec!["mod_a".to_string(), "mod_b".to_string(), "mod_c".to_string()];
        config.mod_loading.disabled = vec!["disabled_mod".to_string()];

        let toml_str = toml::to_string(&config).unwrap();
        let parsed: OpenZTConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.mod_loading.order, config.mod_loading.order);
        assert_eq!(parsed.mod_loading.disabled, config.mod_loading.disabled);
        assert_eq!(parsed.mod_loading.auto_resolve_new_mods, config.mod_loading.auto_resolve_new_mods);
        assert_eq!(parsed.logging.log_to_file, config.logging.log_to_file);
        assert_eq!(parsed.logging.level, config.logging.level);
    }

    #[test]
    fn test_log_level_serialization() {
        // Test that log levels serialize to lowercase strings within a config
        let mut config = LoggingConfig {
            log_to_file: true,
            level: LogLevel::Trace,
        };

        assert!(toml::to_string(&config).unwrap().contains("level = \"trace\""));

        config.level = LogLevel::Debug;
        assert!(toml::to_string(&config).unwrap().contains("level = \"debug\""));

        config.level = LogLevel::Info;
        assert!(toml::to_string(&config).unwrap().contains("level = \"info\""));

        config.level = LogLevel::Warn;
        assert!(toml::to_string(&config).unwrap().contains("level = \"warn\""));

        config.level = LogLevel::Error;
        assert!(toml::to_string(&config).unwrap().contains("level = \"error\""));
    }

    #[test]
    fn test_log_level_deserialization() {
        // Test that lowercase strings deserialize to LogLevel
        let trace_config: LoggingConfig = toml::from_str("log_to_file = true\nlevel = \"trace\"").unwrap();
        assert_eq!(trace_config.level, LogLevel::Trace);

        let debug_config: LoggingConfig = toml::from_str("log_to_file = true\nlevel = \"debug\"").unwrap();
        assert_eq!(debug_config.level, LogLevel::Debug);

        let info_config: LoggingConfig = toml::from_str("log_to_file = true\nlevel = \"info\"").unwrap();
        assert_eq!(info_config.level, LogLevel::Info);

        let warn_config: LoggingConfig = toml::from_str("log_to_file = true\nlevel = \"warn\"").unwrap();
        assert_eq!(warn_config.level, LogLevel::Warn);

        let error_config: LoggingConfig = toml::from_str("log_to_file = true\nlevel = \"error\"").unwrap();
        assert_eq!(error_config.level, LogLevel::Error);
    }

    #[test]
    fn test_log_level_to_level_filter() {
        // Test that LogLevel correctly converts to LevelFilter
        assert_eq!(LogLevel::Trace.to_level_filter(), LevelFilter::TRACE);
        assert_eq!(LogLevel::Debug.to_level_filter(), LevelFilter::DEBUG);
        assert_eq!(LogLevel::Info.to_level_filter(), LevelFilter::INFO);
        assert_eq!(LogLevel::Warn.to_level_filter(), LevelFilter::WARN);
        assert_eq!(LogLevel::Error.to_level_filter(), LevelFilter::ERROR);
    }

    #[test]
    fn test_logging_config_with_different_levels() {
        let config_str = "[logging]\nlog_to_file = false\nlevel = \"debug\"";

        let parsed: OpenZTConfig = toml::from_str(config_str).unwrap();
        assert!(!parsed.logging.log_to_file);
        assert_eq!(parsed.logging.level, LogLevel::Debug);
    }

    #[test]
    fn test_missing_logging_section_uses_defaults() {
        // Config file with only mod_loading section (older format)
        let config_str = r#"
[mod_loading]
order = ["mod_a", "mod_b"]
disabled = []
auto_resolve_new_mods = true
warn_on_conflicts = true
"#;

        let parsed: OpenZTConfig = toml::from_str(config_str).unwrap();

        // mod_loading should be loaded from file
        assert_eq!(parsed.mod_loading.order, vec!["mod_a", "mod_b"]);

        // logging should use defaults (serde's #[serde(default)])
        assert!(parsed.logging.log_to_file);
        assert_eq!(parsed.logging.level, LogLevel::Warn);
    }

    #[test]
    fn test_missing_mod_loading_section_uses_defaults() {
        // Config file with only logging section
        let config_str = r#"
[logging]
log_to_file = false
level = "error"
"#;

        let parsed: OpenZTConfig = toml::from_str(config_str).unwrap();

        // logging should be loaded from file
        assert!(!parsed.logging.log_to_file);
        assert_eq!(parsed.logging.level, LogLevel::Error);

        // mod_loading should use defaults
        assert!(parsed.mod_loading.order.is_empty());
        assert!(parsed.mod_loading.disabled.is_empty());
        assert!(parsed.mod_loading.auto_resolve_new_mods);
        assert!(parsed.mod_loading.warn_on_conflicts);
    }

    #[test]
    fn test_empty_config_uses_all_defaults() {
        // Empty config file
        let config_str = "";

        let parsed: OpenZTConfig = toml::from_str(config_str).unwrap();

        // Everything should use defaults
        assert!(parsed.mod_loading.order.is_empty());
        assert!(parsed.mod_loading.disabled.is_empty());
        assert!(parsed.mod_loading.auto_resolve_new_mods);
        assert!(parsed.mod_loading.warn_on_conflicts);
        assert!(parsed.logging.log_to_file);
        assert_eq!(parsed.logging.level, LogLevel::Warn);
    }

    #[test]
    fn test_missing_fields_within_logging_section() {
        // Logging section with only 'log_to_file' field (missing 'level')
        let config_str = r#"
[logging]
log_to_file = false
"#;

        let parsed: OpenZTConfig = toml::from_str(config_str).unwrap();

        // log_to_file should be from file
        assert!(!parsed.logging.log_to_file);

        // level should use default
        assert_eq!(parsed.logging.level, LogLevel::Warn);
    }

    #[test]
    fn test_missing_fields_within_mod_loading_section() {
        // mod_loading section with only 'order' field (missing other fields)
        let config_str = r#"
[mod_loading]
order = ["mod_a"]
"#;

        let parsed: OpenZTConfig = toml::from_str(config_str).unwrap();

        // order should be from file
        assert_eq!(parsed.mod_loading.order, vec!["mod_a"]);

        // Other fields should use defaults
        assert!(parsed.mod_loading.disabled.is_empty());
        assert!(parsed.mod_loading.auto_resolve_new_mods);
        assert!(parsed.mod_loading.warn_on_conflicts);
    }

    #[test]
    fn test_partial_fields_in_both_sections() {
        // Both sections present but each missing some fields
        let config_str = r#"
[mod_loading]
order = ["mod_a", "mod_b"]
warn_on_conflicts = false

[logging]
level = "debug"
"#;

        let parsed: OpenZTConfig = toml::from_str(config_str).unwrap();

        // mod_loading: order and warn_on_conflicts from file, rest defaults
        assert_eq!(parsed.mod_loading.order, vec!["mod_a", "mod_b"]);
        assert!(!parsed.mod_loading.warn_on_conflicts);
        assert!(parsed.mod_loading.disabled.is_empty()); // default
        assert!(parsed.mod_loading.auto_resolve_new_mods); // default

        // logging: level from file, log_to_file is default
        assert_eq!(parsed.logging.level, LogLevel::Debug);
        assert!(parsed.logging.log_to_file); // default
    }
}
