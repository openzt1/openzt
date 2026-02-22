use super::{
    instance::{
        CreateInstanceRequest, CreateInstanceResponse, Instance,
        InstanceDetails, InstanceStatus, LogsResponse, InstanceStatusResponse,
    },
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub fn create_router() -> Router<Arc<RwLock<AppState>>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/instances", post(create_instance).get(list_instances))
        .route(
            "/api/instances/{id}",
            get(get_instance).delete(delete_instance),
        )
        .route("/api/instances/{id}/logs", get(get_instance_logs))
        .route("/api/instances/{id}/logs/stream", get(stream_logs))
        .route("/api/instances/{id}/stop", post(stop_instance))
        .route("/api/instances/{id}/start", post(start_instance))
        .route("/api/instances/{id}/restart", post(restart_instance))
}

async fn health_check() -> &'static str {
    "OK"
}

async fn create_instance(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<CreateInstanceRequest>,
) -> Result<Json<CreateInstanceResponse>, ApiError> {
    let instance_id = Uuid::new_v4().to_string();
    let container_name = format!("{}{}", state.read().await.config.docker.container_prefix, instance_id);

    tracing::info!("Creating instance {}", instance_id);

    // Allocate ports
    let (vnc_port, console_port) = {
        let mut state_guard = state.write().await;
        state_guard
            .port_pool
            .allocate_pair()
            .ok_or(ApiError::PortsExhausted)?
    };

    // Write DLL to temp file
    let dll_path =
        super::docker::write_dll_to_temp(&instance_id, &req.openzt_dll).map_err(|e| {
            tracing::error!("Failed to write DLL: {}", e);
            ApiError::InvalidDll(e.to_string())
        })?;

    // Create instance record
    let instance = Instance {
        id: instance_id.clone(),
        container_id: String::new(),
        vnc_port,
        console_port,
        status: InstanceStatus::Creating,
        created_at: Utc::now(),
        config: req.config.unwrap_or_default(),
    };

    {
        let mut state_guard = state.write().await;
        if state_guard.instances.len() >= state_guard.config.instances.max_instances {
            // Release ports
            state_guard.port_pool.release_pair(vnc_port, console_port);
            return Err(ApiError::MaxInstancesReached);
        }
        state_guard.instances.insert(instance_id.clone(), instance);
    }

    // Create Docker container (background task)
    let state_clone = state.clone();
    let instance_id_clone = instance_id.clone();
    tokio::spawn(async move {
        if let Err(e) = create_container_task(
            state_clone.clone(),
            instance_id_clone.clone(),
            container_name,
            vnc_port,
            console_port,
            dll_path.clone(),
        )
        .await
        {
            tracing::error!("Failed to create container for instance {}: {}", instance_id_clone, e);

            // Clean up temp DLL file
            super::docker::cleanup_dll_temp(&instance_id_clone);

            // Update instance status to error and release ports
            let mut state_guard = state_clone.write().await;
            if let Some(instance) = state_guard.instances.get_mut(&instance_id_clone) {
                instance.status = InstanceStatus::Error(e.to_string());
            }
            state_guard.port_pool.release_pair(vnc_port, console_port);
        }
    });

    Ok(Json(CreateInstanceResponse {
        instance_id,
        vnc_port,
        console_port,
        vnc_url: format!("vnc://localhost:{}", vnc_port),
        status: "creating".to_string(),
    }))
}

async fn create_container_task(
    state: Arc<RwLock<AppState>>,
    instance_id: String,
    container_name: String,
    vnc_port: u16,
    console_port: u16,
    dll_path: String,
) -> anyhow::Result<()> {
    let docker_manager = super::docker::DockerManager::new()?;

    // Ensure image exists
    let image = {
        let state_guard = state.read().await;
        state_guard.config.docker.image.clone()
    };
    docker_manager.ensure_image(&image).await?;

    // Get instance config and apply default cpulimit if not set
    let instance_config = {
        let state_guard = state.read().await;
        let mut config = state_guard.instances.get(&instance_id)
            .map(|inst| inst.config.clone())
            .unwrap_or_default();

        // Apply default cpulimit if not set
        if config.cpulimit.is_none() {
            config.cpulimit = Some(state_guard.config.instances.default_cpulimit);
        }
        config
    };

    // Create container
    let container_id = match docker_manager
        .create_container(&container_name, &image, vnc_port, console_port, &dll_path, &instance_config)
        .await
    {
        Ok(id) => id,
        Err(e) => {
            // Container creation failed - nothing to clean up
            return Err(e.context("Failed to create container"));
        }
    };

    tracing::info!("Created container {} for instance {}", container_id, instance_id);

    // Start container - clean up if this fails
    if let Err(e) = docker_manager.start_container(&container_id).await {
        tracing::error!("Failed to start container {}: {}", container_id, e);

        // Clean up the failed container
        if let Err(cleanup_err) = docker_manager.stop_and_remove_container(&container_id).await {
            tracing::error!("Failed to clean up container {}: {}", container_id, cleanup_err);
        } else {
            tracing::info!("Cleaned up failed container {}", container_id);
        }

        return Err(e.context("Failed to start container"));
    }

    tracing::info!("Started container {} for instance {}", container_id, instance_id);

    // Update instance status
    {
        let mut state_guard = state.write().await;
        if let Some(instance) = state_guard.instances.get_mut(&instance_id) {
            instance.container_id = container_id.clone();
            instance.status = InstanceStatus::Running;
        }
    }

    Ok(())
}

async fn list_instances(
    State(state): State<Arc<RwLock<AppState>>>,
) -> Result<Json<Vec<InstanceDetails>>, ApiError> {
    // Collect instance IDs and container IDs first (drop read lock before acquiring write lock)
    let instance_ids: Vec<(String, String)> = {
        let state_guard = state.read().await;
        state_guard.instances.iter()
            .map(|(id, inst)| (id.clone(), inst.container_id.clone()))
            .collect()
    };

    // Try to refresh instance statuses
    if let Ok(docker_manager) = super::docker::DockerManager::new() {
        let mut state_guard = state.write().await;
        let mut deleted_count = 0;

        for (id, container_id) in &instance_ids {
            match docker_manager.refresh_instance_status(container_id).await {
                Ok(Some(status)) => {
                    if let Some(inst) = state_guard.instances.get_mut(id) {
                        inst.status = status;
                    }
                }
                Ok(None) => {
                    // Container was deleted externally
                    if let Some(inst) = state_guard.instances.get_mut(id) {
                        inst.status = InstanceStatus::Error("Container deleted externally".to_string());
                    }
                    deleted_count += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to refresh status for {}: {}. Using cached.", id, e);
                }
            }
        }

        if deleted_count > 0 {
            tracing::info!("Status refresh: {} containers deleted externally", deleted_count);
        }
    } else {
        tracing::warn!("Failed to connect to Docker. Using cached status.");
    }

    // Return (possibly refreshed) list
    let state_guard = state.read().await;
    let instances: Vec<InstanceDetails> = state_guard
        .instances
        .values()
        .cloned()
        .map(Into::into)
        .collect();
    Ok(Json(instances))
}

async fn get_instance(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(id): Path<String>,
) -> Result<Json<InstanceDetails>, ApiError> {
    // First check if instance exists and get container_id
    let container_id = {
        let state_guard = state.read().await;
        state_guard.instances.get(&id)
            .map(|inst| inst.container_id.clone())
            .ok_or(ApiError::NotFound)?
    };

    // Refresh this instance's status
    if let Ok(docker_manager) = super::docker::DockerManager::new() {
        match docker_manager.refresh_instance_status(&container_id).await {
            Ok(Some(status)) => {
                let mut state_guard = state.write().await;
                if let Some(inst) = state_guard.instances.get_mut(&id) {
                    inst.status = status;
                }
            }
            Ok(None) => {
                // Container was deleted externally
                let mut state_guard = state.write().await;
                if let Some(inst) = state_guard.instances.get_mut(&id) {
                    inst.status = InstanceStatus::Error("Container deleted externally".to_string());
                }
            }
            Err(e) => {
                tracing::warn!("Failed to refresh status for {}: {}. Using cached.", id, e);
            }
        }
    }

    // Return (possibly refreshed) instance
    let state_guard = state.read().await;
    state_guard
        .instances
        .get(&id)
        .cloned()
        .map(Into::into)
        .ok_or(ApiError::NotFound)
        .map(Json)
}

async fn delete_instance(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    tracing::info!("Deleting instance {}", id);

    // Get instance details for cleanup
    let (container_id, vnc_port, console_port) = {
        let state_guard = state.read().await;
        let instance = state_guard.instances.get(&id).ok_or(ApiError::NotFound)?;
        (instance.container_id.clone(), instance.vnc_port, instance.console_port)
    };

    // Stop and remove container
    if !container_id.is_empty() {
        let docker_manager = super::docker::DockerManager::new()?;
        if let Err(e) = docker_manager.stop_and_remove_container(&container_id).await {
            tracing::warn!("Failed to remove container {}: {}", container_id, e);
        }
    }

    // Clean up temp DLL file
    super::docker::cleanup_dll_temp(&id);

    // Remove instance and release ports
    {
        let mut state_guard = state.write().await;
        state_guard.instances.remove(&id);
        state_guard.port_pool.release_pair(vnc_port, console_port);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_instance_logs(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(id): Path<String>,
) -> Result<Json<LogsResponse>, ApiError> {
    let state_guard = state.read().await;
    let instance = state_guard.instances.get(&id).ok_or(ApiError::NotFound)?;
    let container_id = &instance.container_id;

    if container_id.is_empty() {
        return Ok(Json(LogsResponse {
            instance_id: id,
            logs: "Container not yet created".to_string(),
        }));
    }

    let docker_manager = super::docker::DockerManager::new()?;
    let logs = docker_manager.get_container_logs(container_id, 100).await?;

    Ok(Json(LogsResponse { instance_id: id, logs }))
}

async fn stream_logs(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let state_guard = state.read().await;
    let instance = state_guard.instances.get(&id).ok_or(ApiError::NotFound)?;

    if instance.container_id.is_empty() {
        return Err(ApiError::NotFound);
    }

    // SSE streaming requires more complex async stream handling
    // For now, return a message indicating this is not yet implemented
    Ok(Json(serde_json::json!({
        "message": "Log streaming not yet implemented",
        "instance_id": id,
        "note": "Use /api/instances/:id/logs for recent logs"
    }))
    .into_response())
}

async fn stop_instance(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(id): Path<String>,
) -> Result<Json<InstanceStatusResponse>, ApiError> {
    tracing::info!("Stopping instance {}", id);

    // Get container_id
    let container_id = {
        let state_guard = state.read().await;
        let instance = state_guard.instances.get(&id).ok_or(ApiError::NotFound)?;

        // Check if already stopped
        if matches!(instance.status, InstanceStatus::Stopped) {
            return Ok(Json(InstanceStatusResponse {
                id: id.clone(),
                status: instance.status.as_str().to_string(),
            }));
        }

        // Check if container exists
        if instance.container_id.is_empty() {
            return Err(ApiError::Internal("Container not yet created".to_string()));
        }

        instance.container_id.clone()
    };

    // Stop the container
    let docker_manager = super::docker::DockerManager::new()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    docker_manager.stop_container(&container_id).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Update instance status
    {
        let mut state_guard = state.write().await;
        if let Some(instance) = state_guard.instances.get_mut(&id) {
            instance.status = InstanceStatus::Stopped;
        }
    }

    Ok(Json(InstanceStatusResponse {
        id,
        status: "stopped".to_string(),
    }))
}

async fn start_instance(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(id): Path<String>,
) -> Result<Json<InstanceStatusResponse>, ApiError> {
    tracing::info!("Starting instance {}", id);

    // Get container_id
    let container_id = {
        let state_guard = state.read().await;
        let instance = state_guard.instances.get(&id).ok_or(ApiError::NotFound)?;

        // Check if already running
        if matches!(instance.status, InstanceStatus::Running) {
            return Ok(Json(InstanceStatusResponse {
                id: id.clone(),
                status: instance.status.as_str().to_string(),
            }));
        }

        // Check if container exists
        if instance.container_id.is_empty() {
            return Err(ApiError::Internal("Container not yet created".to_string()));
        }

        instance.container_id.clone()
    };

    // Start the container
    let docker_manager = super::docker::DockerManager::new()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    docker_manager.start_container(&container_id).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Update instance status
    {
        let mut state_guard = state.write().await;
        if let Some(instance) = state_guard.instances.get_mut(&id) {
            instance.status = InstanceStatus::Running;
        }
    }

    Ok(Json(InstanceStatusResponse {
        id,
        status: "running".to_string(),
    }))
}

async fn restart_instance(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(id): Path<String>,
) -> Result<Json<InstanceStatusResponse>, ApiError> {
    tracing::info!("Restarting instance {}", id);

    // Get container_id
    let container_id = {
        let state_guard = state.read().await;
        let instance = state_guard.instances.get(&id).ok_or(ApiError::NotFound)?;

        // Check if container exists
        if instance.container_id.is_empty() {
            return Err(ApiError::Internal("Container not yet created".to_string()));
        }

        instance.container_id.clone()
    };

    // Restart the container
    let docker_manager = super::docker::DockerManager::new()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    docker_manager.restart_container(&container_id).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Update instance status to running (restart ensures container is running)
    {
        let mut state_guard = state.write().await;
        if let Some(instance) = state_guard.instances.get_mut(&id) {
            instance.status = InstanceStatus::Running;
        }
    }

    Ok(Json(InstanceStatusResponse {
        id,
        status: "running".to_string(),
    }))
}

#[derive(Debug)]
pub enum ApiError {
    NotFound,
    PortsExhausted,
    MaxInstancesReached,
    InvalidDll(String),
    Internal(String),
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message): (StatusCode, String) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "Instance not found".to_string()),
            ApiError::PortsExhausted => (StatusCode::SERVICE_UNAVAILABLE, "No ports available".to_string()),
            ApiError::MaxInstancesReached => {
                (StatusCode::SERVICE_UNAVAILABLE, "Maximum instances reached".to_string())
            }
            ApiError::InvalidDll(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
