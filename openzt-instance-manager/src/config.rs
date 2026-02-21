use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub ports: PortsConfig,
    pub docker: DockerConfig,
    pub instances: InstancesConfig,
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_listen_address")]
    pub listen_address: SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortsConfig {
    #[serde(default = "default_rdp_start")]
    pub rdp_start: u16,
    #[serde(default = "default_rdp_end")]
    pub rdp_end: u16,
    #[serde(default = "default_console_start")]
    pub console_start: u16,
    #[serde(default = "default_console_end")]
    pub console_end: u16,
    #[serde(default = "default_xpra_start")]
    pub xpra_start: u16,
    #[serde(default = "default_xpra_end")]
    pub xpra_end: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    #[serde(default = "default_docker_image")]
    pub image: String,
    #[serde(default = "default_container_prefix")]
    pub container_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancesConfig {
    #[serde(default = "default_max_instances")]
    pub max_instances: usize,
    #[serde(default = "default_auto_cleanup_hours")]
    pub auto_cleanup_hours: u64,
    #[serde(default = "default_cpulimit")]
    pub default_cpulimit: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default)]
    pub enable_auth: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            ports: PortsConfig::default(),
            docker: DockerConfig::default(),
            instances: InstancesConfig::default(),
            api: ApiConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_address: default_listen_address(),
        }
    }
}

impl Default for PortsConfig {
    fn default() -> Self {
        Self {
            rdp_start: default_rdp_start(),
            rdp_end: default_rdp_end(),
            console_start: default_console_start(),
            console_end: default_console_end(),
            xpra_start: default_xpra_start(),
            xpra_end: default_xpra_end(),
        }
    }
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: default_docker_image(),
            container_prefix: default_container_prefix(),
        }
    }
}

impl Default for InstancesConfig {
    fn default() -> Self {
        Self {
            max_instances: default_max_instances(),
            auto_cleanup_hours: default_auto_cleanup_hours(),
            default_cpulimit: default_cpulimit(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enable_auth: false,
        }
    }
}

fn default_listen_address() -> SocketAddr {
    "0.0.0.0:3000".parse().unwrap()
}

fn default_rdp_start() -> u16 {
    13390
}

fn default_rdp_end() -> u16 {
    13490
}

fn default_console_start() -> u16 {
    18081
}

fn default_console_end() -> u16 {
    18181
}

fn default_xpra_start() -> u16 {
    14500
}

fn default_xpra_end() -> u16 {
    14600
}

fn default_docker_image() -> String {
    "finn/winezt:latest".to_string()
}

fn default_container_prefix() -> String {
    "openzt-".to_string()
}

fn default_max_instances() -> usize {
    100
}

fn default_auto_cleanup_hours() -> u64 {
    24
}

fn default_cpulimit() -> f64 {
    0.5  // Default: 50% of 1 CPU core
}

pub fn load_config() -> Result<Config> {
    let config_path = "config.toml";

    if std::path::Path::new(config_path).exists() {
        let content = std::fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        // Write default config file
        let default_config = Config::default();
        let toml_string = toml::to_string_pretty(&default_config)?;
        std::fs::write(config_path, toml_string)?;
        tracing::info!("Created default config.toml");
        Ok(default_config)
    }
}
