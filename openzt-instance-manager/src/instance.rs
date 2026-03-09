use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub container_id: String,
    pub vnc_port: u16,
    pub console_port: u16,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub config: InstanceConfig,
    pub script_files: Vec<String>,  // Track script filenames for cleanup
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
    pub wine_debug_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpulimit: Option<f64>,  // CPU cores (e.g., 0.5 = 50%, 2.0 = 2 cores)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate_detours: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetourResult {
    pub name: String,
    pub called: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetourTestResults {
    pub instance_id: String,
    pub results: Vec<DetourResult>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptFile {
    pub filename: String,
    pub content: String,  // base64-encoded Lua file content
}

#[derive(Debug, Deserialize)]
pub struct CreateInstanceRequest {
    pub openzt_dll: String,
    #[serde(default)]
    pub config: Option<InstanceConfig>,
    #[serde(default)]
    pub scripts: Vec<ScriptFile>,  // Scripts to upload at creation time
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInstanceResponse {
    pub instance_id: String,
    pub vnc_port: u16,
    pub console_port: u16,
    pub vnc_url: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceDetails {
    pub id: String,
    pub container_id: String,
    pub vnc_port: u16,
    pub console_port: u16,
    pub vnc_url: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub config: InstanceConfig,
}

impl From<Instance> for InstanceDetails {
    fn from(instance: Instance) -> Self {
        Self {
            id: instance.id,
            container_id: instance.container_id,
            vnc_port: instance.vnc_port,
            console_port: instance.console_port,
            vnc_url: format!("vnc://localhost:{}", instance.vnc_port),
            status: instance.status.as_str().to_string(),
            created_at: instance.created_at,
            config: instance.config,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppLogType {
    #[serde(rename = "openzt")]
    Openzt,
    #[serde(rename = "integration-tests")]
    IntegrationTests,
}

impl AppLogType {
    pub fn filename(&self) -> &str {
        match self {
            AppLogType::Openzt => "openzt.log",
            AppLogType::IntegrationTests => "openzt_integration_tests.log",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsResponse {
    pub instance_id: String,
    pub log_type: String,
    pub logs: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceStatusResponse {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadScriptRequest {
    pub script_content: String,  // base64-encoded Lua file content
    pub filename: String,         // script filename (e.g., "detour.lua")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadScriptResponse {
    pub instance_id: String,
    pub filename: String,
    pub path: String,  // Full path where script was uploaded
}
