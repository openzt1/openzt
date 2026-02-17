use anyhow::{anyhow, Context, Result};
use bollard::{
    container::{
        Config as ContainerConfig, CreateContainerOptions, RemoveContainerOptions,
        StartContainerOptions, LogsOptions,
    },
    image::CreateImageOptions,
    service::PortBinding,
    Docker,
};
use std::io::Write;
use std::collections::HashMap;
use futures_util::stream::StreamExt;

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

        let config = ContainerConfig {
            image: Some(image.to_string()),
            hostname: Some(name.to_string()),
            env: Some(vec![
                "RDP_SERVER=yes".to_string(),
            ]),
            exposed_ports: Some(exposed_ports),
            host_config: Some(bollard::service::HostConfig {
                port_bindings: Some(port_bindings),
                binds: Some(vec![
                    format!("{}:/home/wineuser/.wine/drive_c/Program Files (x86)/Microsoft Games/Zoo Tycoon/res-openzt.dll:ro", dll_path),
                ]),
                ipc_mode: Some("host".to_string()),
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
