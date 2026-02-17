use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub container_id: String,
    pub rdp_port: u16,
    pub console_port: u16,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub config: InstanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "message")]
pub enum InstanceStatus {
    Creating,
    Running,
    Stopped,
    Error(String),
}

impl InstanceStatus {
    pub fn as_str(&self) -> &str {
        match self {
            InstanceStatus::Creating => "creating",
            InstanceStatus::Running => "running",
            InstanceStatus::Stopped => "stopped",
            InstanceStatus::Error(msg) => &msg,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstanceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rdp_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wine_debug_level: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInstanceRequest {
    pub openzt_dll: String,
    #[serde(default)]
    pub mods: Vec<String>,
    #[serde(default)]
    pub config: Option<InstanceConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInstanceResponse {
    pub instance_id: String,
    pub rdp_port: u16,
    pub console_port: u16,
    pub rdp_url: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceDetails {
    pub id: String,
    pub container_id: String,
    pub rdp_port: u16,
    pub console_port: u16,
    pub rdp_url: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub config: InstanceConfig,
}

impl From<Instance> for InstanceDetails {
    fn from(instance: Instance) -> Self {
        Self {
            id: instance.id,
            container_id: instance.container_id,
            rdp_port: instance.rdp_port,
            console_port: instance.console_port,
            rdp_url: format!("rdp://localhost:{}", instance.rdp_port),
            status: instance.status.as_str().to_string(),
            created_at: instance.created_at,
            config: instance.config,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsResponse {
    pub instance_id: String,
    pub logs: String,
}
