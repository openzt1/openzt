//! OpenZT Instance Manager
//!
//! This library provides the core types and API structures for managing
//! Zoo Tycoon Docker instances, both for the API server and CLI client.

pub mod config;
pub mod docker;
pub mod instance;
pub mod ports;
pub mod routes;
pub mod state;

// CLI-only modules (conditional compilation)
#[cfg(feature = "cli")]
pub mod client;
#[cfg(feature = "cli")]
pub mod client_config;
#[cfg(feature = "cli")]
pub mod id_resolver;
#[cfg(feature = "cli")]
pub mod output;

// Re-export commonly used types for external consumers
pub use instance::{
    CreateInstanceRequest, CreateInstanceResponse, Instance, InstanceConfig, InstanceDetails,
    InstanceStatus, LogsResponse,
};
pub use state::AppState;

// CLI-only re-exports
#[cfg(feature = "cli")]
pub use client::InstanceClient;
#[cfg(feature = "cli")]
pub use output::OutputFormat;
