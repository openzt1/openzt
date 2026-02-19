use anyhow::{anyhow, Context, Result};
use bollard::{
    container::{
        Config as ContainerConfig, CreateContainerOptions, RemoveContainerOptions,
        StartContainerOptions, StopContainerOptions, RestartContainerOptions,
        LogsOptions, ListContainersOptions, InspectContainerOptions,
    },
    image::CreateImageOptions,
    service::{PortBinding, ContainerSummary, ContainerInspectResponse},
    Docker,
};
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::io::Write;

use crate::instance::{InstanceConfig, InstanceStatus};

pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon")?;
        Ok(Self { docker })
    }

    pub async fn ensure_image(&self, image: &str) -> Result<()> {
        // Check if image exists locally
        let images = self.docker.list_images::<String>(None).await?;
        let image_exists = images.iter().any(|img| {
            img.repo_tags.iter().any(|tag| tag == image)
        });

        if image_exists {
            tracing::info!("Image {} already exists locally", image);
            return Ok(());
        }

        tracing::info!("Pulling image {}...", image);
        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = stream.next().await {
            match result {
                Ok(progress) => {
                    if let Some(id) = progress.id {
                        tracing::debug!("Pulling {}: {}", id, progress.status.unwrap_or_default());
                    }
                }
                Err(e) => return Err(anyhow!("Failed to pull image: {}", e)),
            }
        }

        tracing::info!("Image {} pulled successfully", image);
        Ok(())
    }

    pub async fn create_container(
        &self,
        name: &str,
        image: &str,
        rdp_port: u16,
        console_port: u16,
        dll_path: &str,
        instance_config: &InstanceConfig,
    ) -> Result<String> {
        let options = Some(CreateContainerOptions {
            name: name.to_string(),
            platform: Some("linux/amd64".to_string()),
        });

        // Build exposed ports
        let mut exposed_ports = HashMap::new();
        exposed_ports.insert("3389/tcp".to_string(), HashMap::new());
        exposed_ports.insert("8080/tcp".to_string(), HashMap::new());

        // Build port bindings
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "3389/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: None,
                host_port: Some(rdp_port.to_string()),
            }]),
        );
        port_bindings.insert(
            "8080/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: None,
                host_port: Some(console_port.to_string()),
            }]),
        );

        // Build labels for persistence
        let mut labels = HashMap::new();
        labels.insert("openzt.managed".to_string(), "true".to_string());
        if let Some(cpulimit) = instance_config.cpulimit {
            labels.insert("openzt.cpulimit".to_string(), cpulimit.to_string());
        }

        let config = ContainerConfig {
            image: Some(image.to_string()),
            hostname: Some(name.to_string()),
            labels: Some(labels),
            env: Some(vec![
                "RDP_SERVER=yes".to_string(),
                "XPRA_HTML_PORT=3389".to_string(),
                "USE_XPRA=yes".to_string(),
                //"USE_XVFB=yes".to_string(),
                //"XVFB_RESOLUTION=1024x768x8".to_string(),
                //"XVFB_SCREEN=0".to_string(),
                //"XVFB_SERVER=:95".to_string(),
            ]),
            exposed_ports: Some(exposed_ports),
            host_config: Some(bollard::service::HostConfig {
                port_bindings: Some(port_bindings),
                binds: Some(vec![
                    format!("{}:/home/wineuser/.wine/drive_c/Program Files (x86)/Microsoft Games/Zoo Tycoon/res-openzt.dll:ro", dll_path),
                ]),
                ipc_mode: Some("host".to_string()),
                // CPU limits (equivalent to --cpus=<value>)
                nano_cpus: instance_config.cpulimit
                    .map(|cores| (cores * 1_000_000_000.0) as i64),
                ..Default::default()
            }),
            ..Default::default()
        };

        let result = self.docker.create_container(options, config).await?;
        Ok(result.id)
    }

    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;
        Ok(())
    }

    /// Stop a running container without removing it
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        let options = Some(StopContainerOptions {
            t: 10, // Wait up to 10 seconds for graceful shutdown
        });

        self.docker
            .stop_container(container_id, options)
            .await
            .context("Failed to stop container")?;
        Ok(())
    }

    /// Restart a running container
    pub async fn restart_container(&self, container_id: &str) -> Result<()> {
        let options = Some(RestartContainerOptions {
            t: 10, // Wait up to 10 seconds before forcefully restarting
        });

        self.docker
            .restart_container(container_id, options)
            .await
            .context("Failed to restart container")?;
        Ok(())
    }

    pub async fn stop_and_remove_container(&self, container_id: &str) -> Result<()> {
        let options = RemoveContainerOptions {
            force: true,
            v: true,
            ..Default::default()
        };

        self.docker
            .remove_container(container_id, Some(options))
            .await
            .context("Failed to remove container")?;
        Ok(())
    }

    pub async fn get_container_logs(
        &self,
        container_id: &str,
        tail_lines: u32,
    ) -> Result<String> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: tail_lines.to_string(),
            ..Default::default()
        };

        let mut stream = self.docker.logs(container_id, Some(options));
        let mut output = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(log) => {
                    match log {
                        bollard::container::LogOutput::StdOut { message } => {
                            output.push_str(&String::from_utf8_lossy(&message));
                        }
                        bollard::container::LogOutput::StdErr { message } => {
                            output.push_str(&String::from_utf8_lossy(&message));
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading log: {}", e);
                }
            }
        }

        Ok(output)
    }
}

/// Write base64-encoded DLL to a temporary file
pub fn write_dll_to_temp(instance_id: &str, dll_base64: &str) -> Result<String> {
    let dll_bytes = base64::Engine::decode(&base64::prelude::BASE64_STANDARD, dll_base64)
        .context("Failed to decode base64 DLL")?;

    // Validate PE header (basic check for Windows DLL)
    if dll_bytes.len() < 2 {
        return Err(anyhow!("DLL data is too short"));
    }
    if &dll_bytes[0..2] != b"MZ" {
        return Err(anyhow!("Invalid DLL format: missing MZ header"));
    }

    let temp_path = format!("/tmp/openzt-{}.dll", instance_id);

    let mut file = std::fs::File::create(&temp_path)
        .context("Failed to create temp DLL file")?;
    file.write_all(&dll_bytes)
        .context("Failed to write DLL data")?;

    tracing::info!("Wrote DLL to {}", temp_path);
    Ok(temp_path)
}

/// Clean up temporary DLL file
pub fn cleanup_dll_temp(instance_id: &str) {
    let temp_path = format!("/tmp/openzt-{}.dll", instance_id);
    if let Err(e) = std::fs::remove_file(&temp_path) {
        tracing::warn!("Failed to remove temp DLL file {}: {}", temp_path, e);
    } else {
        tracing::info!("Removed temp DLL file {}", temp_path);
    }
}

/// Holds information extracted from a container during recovery
#[derive(Debug)]
pub struct RecoveredInstanceInfo {
    pub container_id: String,
    pub rdp_port: u16,
    pub console_port: u16,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub config: InstanceConfig,
}

impl DockerManager {
    /// List all containers (including stopped) with the given prefix
    pub async fn list_containers_with_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<ContainerSummary>> {
        let options = Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        });

        let containers = self.docker.list_containers(options).await?;

        let filtered = containers
            .into_iter()
            .filter(|c| {
                c.names.as_ref()
                    .and_then(|names| names.first())
                    .map(|name| name.starts_with(&format!("/{}", prefix)))
                    .unwrap_or(false)
            })
            .collect();

        Ok(filtered)
    }

    /// Extract instance information from container for recovery
    pub async fn inspect_container_for_recovery(
        &self,
        container_id: &str,
    ) -> Result<RecoveredInstanceInfo> {
        let inspect = self.docker
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await?;

        let (rdp_port, console_port) = self.extract_ports(&inspect)?;
        let status = self.map_docker_status(&inspect.state.ok_or_else(|| anyhow!("Missing state"))?);
        let created_at = self.parse_created_timestamp(inspect.created.as_deref().ok_or_else(|| anyhow!("Missing created timestamp"))?)?;

        // Extract cpulimit from labels (stored during creation)
        let config = InstanceConfig {
            cpulimit: inspect.config.as_ref()
                .and_then(|c| c.labels.as_ref())
                .and_then(|labels| labels.get("openzt.cpulimit"))
                .and_then(|s| s.parse::<f64>().ok()),
            ..Default::default()
        };

        Ok(RecoveredInstanceInfo {
            container_id: container_id.to_string(),
            rdp_port,
            console_port,
            status,
            created_at,
            config,
        })
    }

    fn extract_ports(&self, inspect: &ContainerInspectResponse) -> Result<(u16, u16)> {
        // Try NetworkSettings first (for running containers)
        if let Some(network_settings) = &inspect.network_settings {
            if let Some(ports) = &network_settings.ports {
                if !ports.is_empty() {
                    if let (Some(rdp), Some(console)) = (
                        self.try_extract_port(ports, "3389/tcp"),
                        self.try_extract_port(ports, "8080/tcp")
                    ) {
                        return Ok((rdp, console));
                    }
                }
            }
        }

        // Fallback to HostConfig port bindings (for stopped containers)
        let bindings = inspect.host_config
            .as_ref()
            .and_then(|hc| hc.port_bindings.as_ref())
            .ok_or_else(|| anyhow!("No port bindings found in HostConfig"))?;

        let rdp_port = self.extract_port_from_binding(bindings, "3389/tcp")?;
        let console_port = self.extract_port_from_binding(bindings, "8080/tcp")?;

        Ok((rdp_port, console_port))
    }

    /// Try to extract a port from bindings, returning None if not found
    fn try_extract_port(
        &self,
        bindings: &HashMap<String, Option<Vec<PortBinding>>>,
        key: &str,
    ) -> Option<u16> {
        bindings
            .get(key)
            .and_then(|b| b.as_ref())
            .and_then(|b| b.first())
            .and_then(|b| b.host_port.as_ref())
            .and_then(|p| p.parse::<u16>().ok())
    }

    fn extract_port_from_binding(
        &self,
        bindings: &HashMap<String, Option<Vec<PortBinding>>>,
        key: &str,
    ) -> Result<u16> {
        bindings
            .get(key)
            .and_then(|b| b.as_ref())
            .and_then(|b| b.first())
            .and_then(|b| b.host_port.as_ref())
            .and_then(|p| p.parse::<u16>().ok())
            .ok_or_else(|| anyhow!("Port binding for {} not found", key))
    }

    fn map_docker_status(&self, state: &bollard::service::ContainerState) -> InstanceStatus {
        match state.running {
            Some(true) => InstanceStatus::Running,
            Some(false) => {
                match &state.status {
                    Some(status) => match status.as_ref() {
                        "exited" | "paused" => InstanceStatus::Stopped,
                        "created" => InstanceStatus::Creating,
                        s => InstanceStatus::Error(format!("Container state: {}", s)),
                    },
                    None => InstanceStatus::Stopped,
                }
            },
            None => InstanceStatus::Stopped,
        }
    }

    fn parse_created_timestamp(&self, created: &str) -> Result<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(created)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| anyhow!("Invalid timestamp: {}", e))
    }

    /// Refresh the status of a single instance by inspecting its container.
    /// Returns Ok(Some(status)) if the container exists, Ok(None) if the container
    /// was not found (deleted externally), or Err if Docker communication failed.
    pub async fn refresh_instance_status(
        &self,
        container_id: &str,
    ) -> Result<Option<InstanceStatus>> {
        // Handle empty container_id (container not yet created)
        if container_id.is_empty() {
            return Ok(Some(InstanceStatus::Creating));
        }

        // Attempt to inspect the container
        match self.docker.inspect_container(
            container_id,
            None::<InspectContainerOptions>
        ).await {
            Ok(inspect) => {
                let state = inspect.state
                    .ok_or_else(|| anyhow!("Missing state in inspect response"))?;
                Ok(Some(self.map_docker_status(&state)))
            }
            Err(e) => {
                // Check if this is a 404 (container not found)
                if e.to_string().contains("404") || e.to_string().contains("no such container") {
                    tracing::warn!(
                        "Container {} not found (likely deleted externally)",
                        container_id
                    );
                    Ok(None)
                } else {
                    Err(anyhow!("Failed to inspect container: {}", e))
                }
            }
        }
    }
}
