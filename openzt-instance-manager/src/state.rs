use super::{
    config::Config,
    docker::DockerManager,
    instance::Instance,
    ports::PortPool,
};
use std::collections::HashMap;
use uuid::Uuid;

pub struct AppState {
    pub config: Config,
    pub port_pool: PortPool,
    pub instances: HashMap<String, Instance>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let port_pool = PortPool::new(
            config.ports.rdp_start..config.ports.rdp_end,
            config.ports.console_start..config.ports.console_end,
            config.ports.xpra_start..config.ports.xpra_end,
        );

        Self {
            config,
            port_pool,
            instances: HashMap::new(),
        }
    }

    /// Recover existing containers from Docker on startup
    pub async fn recover_instances(&mut self) -> anyhow::Result<usize> {
        let docker = DockerManager::new()?;
        let prefix = &self.config.docker.container_prefix;

        tracing::info!("Scanning for containers with prefix '{}'", prefix);

        let containers = docker.list_containers_with_prefix(prefix).await?;
        let mut recovered_count = 0;

        for container in containers {
            let name = container.names.as_ref()
                .and_then(|n| n.first())
                .ok_or_else(|| anyhow::anyhow!("Container missing name"))?;

            // Extract instance ID from "/openzt-uuid" format
            let instance_id = name
                .strip_prefix(&format!("/{}", prefix))
                .ok_or_else(|| anyhow::anyhow!("Invalid container name format: {}", name))?;

            // Validate UUID
            if Uuid::parse_str(instance_id).is_err() {
                tracing::warn!("Skipping container with invalid UUID: {}", name);
                continue;
            }

            let container_id = container.id
                .ok_or_else(|| anyhow::anyhow!("Container missing ID"))?;

            match docker.inspect_container_for_recovery(&container_id).await {
                Ok(info) => {
                    // Register ports in pool
                    if let Err(e) = self.port_pool.add_existing_triplet(info.rdp_port, info.console_port, info.xpra_port) {
                        tracing::error!("Failed to register ports for {}: {}, skipping", instance_id, e);
                        continue;
                    }

                    // Capture status for logging before moving
                    let status = info.status.clone();

                    // Reconstruct instance
                    let instance = Instance {
                        id: instance_id.to_string(),
                        container_id: info.container_id,
                        rdp_port: info.rdp_port,
                        console_port: info.console_port,
                        xpra_port: info.xpra_port,
                        status: info.status,
                        created_at: info.created_at,
                        config: info.config,
                    };

                    self.instances.insert(instance_id.to_string(), instance);
                    recovered_count += 1;

                    tracing::info!("Recovered instance {} (RDP: {}, Console: {}, XPRA: {}, Status: {:?})",
                        instance_id, info.rdp_port, info.console_port, info.xpra_port, status);
                }
                Err(e) => {
                    tracing::warn!("Failed to inspect container {}: {}", container_id, e);
                }
            }
        }

        tracing::info!("Recovered {} instances", recovered_count);
        Ok(recovered_count)
    }
}
