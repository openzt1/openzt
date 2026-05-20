use anyhow::{anyhow, Context, Result};
use bollard::{
    container::{
        Config as ContainerConfig, CreateContainerOptions, RemoveContainerOptions,
        StartContainerOptions, StopContainerOptions, RestartContainerOptions,
        LogsOptions, ListContainersOptions, InspectContainerOptions, LogOutput,
        UploadToContainerOptions,
    },
    image::CreateImageOptions,
    service::{PortBinding, ContainerSummary, ContainerInspectResponse, Mount, MountTypeEnum},
    Docker,
};
use chrono::{DateTime, Utc};
use futures_util::stream::{Stream, StreamExt};
use std::collections::HashMap;
use std::io::Write;
use std::pin::Pin;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::instance::{AppLogType, InstanceConfig, InstanceStatus};

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
        vnc_port: u16,
        console_port: u16,
        dll_path: &str,
        scripts_dir: Option<&str>,
        instance_config: &InstanceConfig,
    ) -> Result<String> {
        let options = Some(CreateContainerOptions {
            name: name.to_string(),
            platform: Some("linux/amd64".to_string()),
        });

        // Build exposed ports
        let mut exposed_ports = HashMap::new();
        exposed_ports.insert("5901/tcp".to_string(), HashMap::new());
        exposed_ports.insert("8080/tcp".to_string(), HashMap::new());

        // Build port bindings
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "5901/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: None,
                host_port: Some(vnc_port.to_string()),
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
        if let Some(ref detours) = instance_config.validate_detours {
            if !detours.is_empty() {
                labels.insert("openzt.validate_detours".to_string(), detours.join(","));
            }
        }

        // The path is already in the correct format (Windows path on Windows, Linux path on Linux)
        let dll_mount = Mount {
            target: Some("/home/wineuser/res-openzt.dll".to_string()),
            source: Some(dll_path.to_string()),
            typ: Some(MountTypeEnum::BIND),
            read_only: Some(true),
            ..Default::default()
        };
        tracing::debug!("Using mount: source={}, target={}", dll_path, dll_mount.target.as_ref().unwrap());

        // Build mounts array - start with DLL mount
        let mut mounts = vec![dll_mount];

        // Add scripts mount if directory provided
        if let Some(scripts_path) = scripts_dir {
            let scripts_mount = Mount {
                target: Some("/home/wineuser/scripts".to_string()),
                source: Some(scripts_path.to_string()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(true),
                ..Default::default()
            };
            tracing::debug!(
                "Adding scripts mount: {} -> {}",
                scripts_path,
                scripts_mount.target.as_ref().unwrap()
            );
            mounts.push(scripts_mount);
        }

        // Build env vars
        let mut env_vars = vec!["VNC_SERVER=yes".to_string()];
        if let Some(ref detours) = instance_config.validate_detours && !detours.is_empty() {
            env_vars.push(format!("OPENZT_VALIDATE_DETOURS={}", detours.join(",")));
        }

        env_vars.push("LIBGL_ALWAYS_SOFTWARE=1".to_string()); // Force software rendering for better compatibility

        let config = ContainerConfig {
            image: Some(image.to_string()),
            hostname: Some(name.to_string()),
            labels: Some(labels),
            env: Some(env_vars),
            exposed_ports: Some(exposed_ports),
            host_config: Some(bollard::service::HostConfig {
                port_bindings: Some(port_bindings),
                mounts: Some(mounts),
                ipc_mode: Some("host".to_string()),
                // CPU limits (equivalent to --cpus=<value>)
                nano_cpus: instance_config.cpulimit
                    .map(|cores| (cores * 1_000_000_000.0) as i64),
                ..Default::default()
            }),
            ..Default::default()
        };

        let result = self.docker.create_container(options, config).await
            .map_err(|e| anyhow::anyhow!("Docker error: {}", e))?;
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

    /// Stream container logs as an async stream for SSE
    pub fn stream_container_logs(
        &self,
        container_id: &str,
        tail_lines: u32,
    ) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: tail_lines.to_string(),
            follow: true,
            ..Default::default()
        };

        let stream = self.docker.logs(container_id, Some(options));

        Box::pin(stream.map(|result| {
            result.map_err(|e| anyhow!("Docker log error: {}", e)).map(|log| match log {
                LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                    String::from_utf8_lossy(&message).to_string()
                }
                _ => String::new(),
            })
        }))
    }

    /// Read static application logs from container
    pub async fn get_app_logs(
        &self,
        container_id: &str,
        log_type: AppLogType,
        tail_lines: u32,
    ) -> Result<String> {
        let log_path = format!(
            "/home/wineuser/.wine/drive_c/Program Files (x86)/Microsoft Games/Zoo Tycoon/{}",
            log_type.filename()
        );

        // Use docker exec to run tail command inside container
        let exec_options = bollard::exec::CreateExecOptions {
            cmd: Some(vec![
                "tail".to_string(),
                "-n".to_string(),
                tail_lines.to_string(),
                log_path.clone(),
            ]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        // Create exec instance
        let exec = self.docker
            .create_exec(container_id, exec_options)
            .await
            .context("Failed to create exec instance")?;

        // Start exec and capture output
        let exec_result = self.docker
            .start_exec(&exec.id, None)
            .await
            .context("Failed to start exec instance")?;

        let mut result = String::new();

        // Process exec output
        match exec_result {
            bollard::exec::StartExecResults::Attached { mut output, .. } => {
                while let Some(item) = output.next().await {
                    match item {
                        Ok(chunk) => {
                            match chunk {
                                bollard::container::LogOutput::StdOut { message } => {
                                    result.push_str(&String::from_utf8_lossy(&message));
                                }
                                bollard::container::LogOutput::StdErr { message } => {
                                    // Check if tail command failed (file not found)
                                    let err_msg = String::from_utf8_lossy(&message);
                                    if err_msg.contains("No such file or directory") {
                                        return Err(anyhow!(
                                            "Log file not found in container: {} (container may not have been started yet, or the game may not have run)",
                                            log_type.filename()
                                        ));
                                    }
                                    result.push_str(&err_msg);
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            return Err(anyhow!("Error reading exec output: {}", e));
                        }
                    }
                }
            }
            bollard::exec::StartExecResults::Detached => {
                return Err(anyhow!("Exec detached unexpectedly"));
            }
        }

        Ok(result)
    }

    /// Stream application logs using file following
    pub fn stream_app_logs(
        &self,
        container_id: &str,
        log_type: AppLogType,
    ) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>> {
        let log_path = format!(
            "/home/wineuser/.wine/drive_c/Program Files (x86)/Microsoft Games/Zoo Tycoon/{}",
            log_type.filename()
        );

        // Use docker exec with tail -f for streaming
        let exec_options = bollard::exec::CreateExecOptions {
            cmd: Some(vec![
                "tail".to_string(),
                "-f".to_string(),
                log_path,
            ]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let docker = self.docker.clone();
        let container_id = container_id.to_string();

        Box::pin(async_stream::stream! {
            // Create exec instance
            let exec = match docker.create_exec(&container_id, exec_options).await {
                Ok(e) => e,
                Err(e) => {
                    yield Err(anyhow!("Failed to create exec instance: {}", e));
                    return;
                }
            };

            // Start exec and get output stream
            match docker.start_exec(&exec.id, None).await {
                Ok(bollard::exec::StartExecResults::Attached { mut output, .. }) => {
                    while let Some(result) = output.next().await {
                        match result {
                            Ok(chunk) => {
                                match chunk {
                                    bollard::container::LogOutput::StdOut { message } |
                                    bollard::container::LogOutput::StdErr { message } => {
                                        let line = String::from_utf8_lossy(&message).to_string();
                                        if !line.is_empty() {
                                            yield Ok(line);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                yield Err(anyhow!("Stream error: {}", e));
                                break;
                            }
                        }
                    }
                }
                Ok(bollard::exec::StartExecResults::Detached) => {
                    yield Err(anyhow!("Exec detached unexpectedly"));
                }
                Err(e) => {
                    yield Err(anyhow!("Failed to start exec instance: {}", e));
                }
            }
        })
    }
}

/// Write base64-encoded DLL to a temporary file
///
/// On Windows with WSL2, this copies the file into the WSL2 filesystem
/// to ensure proper bind mount behavior.
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

    // Write to temp directory
    let mut temp_path = std::env::temp_dir();
    temp_path.push(format!("openzt-{}.dll", instance_id));

    let mut file = std::fs::File::create(&temp_path)
        .context("Failed to create temp DLL file")?;
    file.write_all(&dll_bytes)
        .context("Failed to write DLL data")?;

    let temp_path_str = temp_path.to_string_lossy().to_string();
    tracing::info!("Wrote DLL to {}", temp_path_str);
    Ok(temp_path_str)
}

/// Clean up temporary DLL file
pub fn cleanup_dll_temp(instance_id: &str) {
    let mut temp_path = std::env::temp_dir();
    temp_path.push(format!("openzt-{}.dll", instance_id));
    if let Err(e) = std::fs::remove_file(&temp_path) {
        tracing::warn!("Failed to remove temp DLL file {}: {}", temp_path.display(), e);
    } else {
        tracing::info!("Removed temp DLL file {}", temp_path.display());
    }
}

/// Get the scripts directory path for an instance
pub fn get_scripts_dir(instance_id: &str) -> std::path::PathBuf {
    let mut temp_path = std::env::temp_dir();
    temp_path.push(format!("openzt-scripts-{}", instance_id));
    temp_path
}

/// Create the scripts directory for an instance
pub fn ensure_scripts_dir(instance_id: &str) -> Result<std::path::PathBuf> {
    let scripts_dir = get_scripts_dir(instance_id);

    if !scripts_dir.exists() {
        std::fs::create_dir(&scripts_dir)
            .context("Failed to create scripts directory")?;
        tracing::info!("Created scripts directory: {}", scripts_dir.display());
    }

    Ok(scripts_dir)
}

/// Write a single script file to host temp directory
pub fn write_script_to_temp(
    instance_id: &str,
    filename: &str,
    script_content_base64: &str,
) -> Result<String> {
    let script_bytes = base64::Engine::decode(
        &base64::prelude::BASE64_STANDARD,
        script_content_base64
    ).context("Failed to decode base64 script content")?;

    // Validate UTF-8
    let _script_content = String::from_utf8(script_bytes.clone())
        .context("Script content is not valid UTF-8")?;

    let scripts_dir = ensure_scripts_dir(instance_id)?;
    let script_path = scripts_dir.join(filename);

    let mut file = std::fs::File::create(&script_path)
        .context("Failed to create script file")?;
    file.write_all(&script_bytes)
        .context("Failed to write script content")?;

    tracing::info!("Wrote script to {}", script_path.display());
    Ok(script_path.to_string_lossy().to_string())
}

/// Clean up all script files for an instance
pub fn cleanup_scripts_temp(instance_id: &str) {
    let scripts_dir = get_scripts_dir(instance_id);

    if let Err(e) = std::fs::remove_dir_all(&scripts_dir) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(
                "Failed to remove scripts directory {}: {}",
                scripts_dir.display(),
                e
            );
        }
    } else {
        tracing::info!("Removed scripts directory {}", scripts_dir.display());
    }
}

/// Holds information extracted from a container during recovery
#[derive(Debug)]
pub struct RecoveredInstanceInfo {
    pub container_id: String,
    pub vnc_port: u16,
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

        let (vnc_port, console_port) = self.extract_ports(&inspect)?;
        let status = self.map_docker_status(&inspect.state.ok_or_else(|| anyhow!("Missing state"))?);
        let created_at = self.parse_created_timestamp(inspect.created.as_deref().ok_or_else(|| anyhow!("Missing created timestamp"))?)?;

        // Extract config fields from labels (stored during creation)
        let labels = inspect.config.as_ref().and_then(|c| c.labels.as_ref());
        let config = InstanceConfig {
            cpulimit: labels
                .and_then(|l| l.get("openzt.cpulimit"))
                .and_then(|s| s.parse::<f64>().ok()),
            validate_detours: labels
                .and_then(|l| l.get("openzt.validate_detours"))
                .map(|s| s.split(',').map(String::from).collect::<Vec<_>>())
                .filter(|v| !v.is_empty()),
            ..Default::default()
        };

        Ok(RecoveredInstanceInfo {
            container_id: container_id.to_string(),
            vnc_port,
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
                    if let (Some(vnc), Some(console)) = (
                        self.try_extract_port(ports, "5901/tcp"),
                        self.try_extract_port(ports, "8080/tcp")
                    ) {
                        return Ok((vnc, console));
                    }
                }
            }
        }

        // Fallback to HostConfig port bindings (for stopped containers)
        let bindings = inspect.host_config
            .as_ref()
            .and_then(|hc| hc.port_bindings.as_ref())
            .ok_or_else(|| anyhow!("No port bindings found in HostConfig"))?;

        let vnc_port = self.extract_port_from_binding(bindings, "5901/tcp")?;
        let console_port = self.extract_port_from_binding(bindings, "8080/tcp")?;

        Ok((vnc_port, console_port))
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

    /// Copy a file from host path into container using docker cp
    pub async fn cp_to_container(
        &self,
        container_id: &str,
        host_path: &str,
        container_path: &str,
    ) -> Result<()> {
        // Read the file content
        let mut file = File::open(host_path).await
            .context("Failed to open file for copying")?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await
            .context("Failed to read file content")?;

        // Use bollard's upload to container (this implements docker cp)
        let options = Some(UploadToContainerOptions {
            path: container_path.to_string(),
            ..Default::default()
        });

        self.docker
            .upload_to_container(container_id, options, buffer.into())
            .await
            .context("Failed to copy file to container")?;

        tracing::info!("Copied {} to container:{}", host_path, container_path);
        Ok(())
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
