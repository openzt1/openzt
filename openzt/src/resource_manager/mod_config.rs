use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use indexmap::IndexMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use tracing::info;
use tracing_subscriber::filter::LevelFilter;

static LOGGING_INITIALIZED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Initialize logging with settings from openzt.toml
/// This should be called AFTER config is loaded
pub fn init_logging(config: &LoggingConfig) -> anyhow::Result<()> {
    let mut initialized = LOGGING_INITIALIZED.lock().unwrap();
    if *initialized {
        return Err(anyhow::anyhow!("Logging already initialized"));
    }

    let enable_ansi = enable_ansi_support::enable_ansi_support().is_ok();

    #[cfg(not(feature = "integration-tests"))]
    let level_filter = config.level.to_level_filter();
    #[cfg(feature = "integration-tests")]
    let level_filter = LevelFilter::TRACE; // Force TRACE level for integration tests


    #[cfg(not(feature = "integration-tests"))]
    let log_to_file = config.log_to_file;
    #[cfg(feature = "integration-tests")]
    let log_to_file = true;

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    // Always set up console logging
    let console_layer = tracing_subscriber::fmt::layer()
        .with_ansi(enable_ansi)
        .with_writer(std::io::stdout)
        .with_filter(level_filter);

    // Set up file logging if enabled
    if log_to_file {
        let log_path = crate::util::get_base_path().join("openzt.log");
        match std::fs::File::create(&log_path) {
            Ok(log_file) => {
                // Wrap in non-blocking writer
                let (non_blocking, _guard) = tracing_appender::non_blocking(log_file);

                let file_layer = tracing_subscriber::fmt::layer()
                    .with_ansi(false) // No ANSI codes in file
                    .with_writer(non_blocking)
                    .with_filter(level_filter);

                // Initialize with both console and file layers
                tracing_subscriber::registry()
                    .with(console_layer)
                    .with(file_layer)
                    .init();

                // Store guard to prevent it from being dropped
                // Note: We need to leak this guard to keep file logging active
                std::mem::forget(_guard);

                eprintln!("Logging initialized: level={:?}, file={}", config.level, log_path.display());
            }
            Err(e) => {
                // Fall back to console-only if file creation fails
                tracing_subscriber::registry()
                    .with(console_layer)
                    .init();

                eprintln!("Failed to create openzt.log: {}", e);
                eprintln!("Logging initialized: level={:?}, console only", config.level);
            }
        }
    } else {
        // Console-only logging
        tracing_subscriber::registry()
            .with(console_layer)
            .init();

        eprintln!("Logging initialized: level={:?}, console only", config.level);
    }

    *initialized = true;
    Ok(())
}
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

/// Log level setting for OpenZT logging
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    #[default]
    Warn,
    Error,
}

impl LogLevel {
    /// Convert to tracing's LevelFilter
    pub fn to_level_filter(self) -> LevelFilter {
        match self {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}


/// Logging configuration section
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    /// Enable file logging to openzt.log (default: true)
    #[serde(default = "default_true")]
    pub log_to_file: bool,

    /// Log level (default: Warn)
    #[serde(default)]
    pub level: LogLevel,
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

fn default_true() -> bool {
    true
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
            logging: LoggingConfig {
                log_to_file: true,
                level: LogLevel::Warn,
            },
            resource_cache: ResourceCacheConfig::default(),
            expansions: ExpansionConfig::default(),
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

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            log_to_file: true,
            level: LogLevel::Warn,
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
        ExpansionConfig {
            custom: IndexMap::new(),
        }
    }
}

// Global cached configuration
static CACHED_CONFIG: LazyLock<Mutex<OpenZTConfig>> = LazyLock::new(|| {
    Mutex::new(load_openzt_config_from_disk())
});

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
                        logging.get("log_to_file").is_some() && logging.get("level").is_some()
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

                    // expansions section should always be present (even if empty)
                    // No field-level validation needed for expansions since it's a free-form HashMap

                    // Update needed if sections missing or fields incomplete
                    !has_mod_loading || !has_logging || !has_resource_cache || !has_expansions
                        || !mod_loading_complete || !logging_complete || !resource_cache_complete
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
                    eprintln!("OpenZT will now wait indefinitely to allow you to read this message.");
                    eprintln!();
                    eprintln!("Config file location: {}", config_path.display());

                    // Wait indefinitely so user can see the error
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(3600));
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Could not read openzt.toml: {}", e);
            eprintln!();
            eprintln!("Please fix the file permissions and restart the game.");
            eprintln!("OpenZT will now wait indefinitely to allow you to read this message.");
            eprintln!();
            eprintln!("Config file location: {}", config_path.display());

            // Wait indefinitely so user can see the error
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
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
    let toml_string = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

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

    // Add inline comment to disabled field to indicate it can disable vanilla ZTDs
    // Only when empty (similar to expansions section behavior)
    content = content.replace(
        "disabled = []",
        "disabled = [] # Can also be used to disable vanilla ZTD mods (e.g., \"legacy_expansion.ztd\")"
    );

    // Write to temp file
    std::fs::write(&temp_path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write temp config: {}", e))?;

    // Atomic rename (overwrites existing on Windows)
    std::fs::rename(&temp_path, &config_path)
        .map_err(|e| anyhow::anyhow!("Failed to rename temp config: {}", e))?;

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
        config.mod_loading.order = vec![
            "mod_a".to_string(),
            "mod_b".to_string(),
            "mod_c".to_string(),
        ];
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
        let trace_config: LoggingConfig = toml::from_str("enabled = true\nlevel = \"trace\"").unwrap();
        assert_eq!(trace_config.level, LogLevel::Trace);

        let debug_config: LoggingConfig = toml::from_str("enabled = true\nlevel = \"debug\"").unwrap();
        assert_eq!(debug_config.level, LogLevel::Debug);

        let info_config: LoggingConfig = toml::from_str("enabled = true\nlevel = \"info\"").unwrap();
        assert_eq!(info_config.level, LogLevel::Info);

        let warn_config: LoggingConfig = toml::from_str("enabled = true\nlevel = \"warn\"").unwrap();
        assert_eq!(warn_config.level, LogLevel::Warn);

        let error_config: LoggingConfig = toml::from_str("enabled = true\nlevel = \"error\"").unwrap();
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
