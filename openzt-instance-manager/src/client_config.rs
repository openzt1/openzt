//! Client configuration for the OpenZT CLI
//!
//! This module handles loading and managing configuration from
//! ~/.config/openzt-client/config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Default API URL
const DEFAULT_API_URL: &str = "http://localhost:3000";

/// Default output format
const DEFAULT_OUTPUT_FORMAT: &str = "table";

/// Client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// API configuration section
    #[serde(default)]
    pub api: ApiConfig,
    /// Output configuration section
    #[serde(default)]
    pub output: OutputConfig,
    /// Instance creation defaults
    #[serde(default)]
    pub create: CreateConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            output: OutputConfig::default(),
            create: CreateConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Default API base URL
    #[serde(default = "default_api_url")]
    pub base_url: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: default_api_url(),
        }
    }
}

fn default_api_url() -> String {
    DEFAULT_API_URL.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Default output format (table or json)
    #[serde(default = "default_output_format")]
    pub format: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_output_format(),
        }
    }
}

fn default_output_format() -> String {
    DEFAULT_OUTPUT_FORMAT.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConfig {
    /// Default RDP password for new instances
    #[serde(default)]
    pub rdp_password: Option<String>,
}

impl Default for CreateConfig {
    fn default() -> Self {
        Self { rdp_password: None }
    }
}

impl ClientConfig {
    /// Get the config directory path
    pub fn config_dir() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("com.openzt", "OpenZT", "openzt-client")
            .context("Failed to determine config directory")?;

        Ok(dirs.config_dir().to_path_buf())
    }

    /// Get the config file path
    pub fn config_file() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load configuration from file, or return defaults if not found
    pub fn load() -> Self {
        match Self::config_file() {
            Ok(path) => Self::load_from_path(path).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(anyhow::anyhow!("Config file not found: {}", path.display()));
        }

        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: ClientConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_file()?;

        // Create config directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Get the output format as an enum
    pub fn output_format(&self) -> Option<super::output::OutputFormat> {
        super::output::OutputFormat::from_str(&self.output.format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClientConfig::default();
        assert_eq!(config.api.base_url, DEFAULT_API_URL);
        assert_eq!(config.output.format, DEFAULT_OUTPUT_FORMAT);
        assert!(config.create.rdp_password.is_none());
    }

    #[test]
    fn test_config_dir() {
        let dir = ClientConfig::config_dir();
        assert!(dir.is_ok());
        let dir_path = dir.unwrap();
        assert!(dir_path.ends_with("openzt-client"));
    }

    #[test]
    fn test_config_parsing() {
        let toml_content = r#"
            [api]
            base_url = "http://example.com:8080"

            [output]
            format = "json"

            [create]
            rdp_password = "secret123"
        "#;

        let config: ClientConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.api.base_url, "http://example.com:8080");
        assert_eq!(config.output.format, "json");
        assert_eq!(config.create.rdp_password, Some("secret123".to_string()));
    }
}
