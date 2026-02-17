mod config;
mod docker;
mod instance;
mod ports;
mod routes;
mod state;

use anyhow::Result;
use axum::{http::Method, Router};
use axum::extract::DefaultBodyLimit;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openzt_instance_manager=debug,axum=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = config::load_config()?;
    tracing::info!("Loaded configuration: {:?}", config.server);

    // Create application state
    let mut app_state = state::AppState::new(config.clone());

    // Recover existing containers from Docker
    match app_state.recover_instances().await {
        Ok(count) => {
            tracing::info!("Successfully recovered {} instances on startup", count);
        }
        Err(e) => {
            tracing::warn!("Failed to recover instances: {}. Starting with empty state.", e);
            // Don't fail startup - continue with empty state
        }
    }

    let state = Arc::new(RwLock::new(app_state));

    // Build router with CORS support and increased body limit
    let app = Router::new()
        .merge(routes::create_router())
        .with_state(state)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50 MB limit
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
                .allow_headers(Any),
        );

    // Start server
    let listener = tokio::net::TcpListener::bind(config.server.listen_address)
        .await
        .unwrap();

    tracing::info!(
        "OpenZT Instance Manager API listening on {}",
        config.server.listen_address
    );

    axum::serve(listener, app).await?;

    Ok(())
}
