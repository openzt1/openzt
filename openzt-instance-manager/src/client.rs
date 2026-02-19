//! HTTP client for the OpenZT Instance Manager API
//!
//! This module provides a convenient async client for interacting with
//! the instance manager API endpoints.

use crate::instance::{CreateInstanceResponse, InstanceConfig, InstanceDetails, LogsResponse, InstanceStatusResponse};
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use std::path::Path;

/// API client for the OpenZT Instance Manager
#[derive(Clone)]
pub struct InstanceClient {
    base_url: String,
    http_client: Client,
}

impl InstanceClient {
    /// Create a new API client with the given base URL
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http_client: Client::new(),
        }
    }

    /// Get the full URL for an API endpoint
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    /// Check if the API server is healthy
    pub async fn health(&self) -> Result<bool> {
        let response = self
            .http_client
            .get(self.url("/health"))
            .send()
            .await
            .context("Failed to connect to API server")?;

        Ok(response.status().is_success())
    }

    /// Create a new instance with the given DLL file
    pub async fn create_instance(
        &self,
        dll_path: &Path,
        config: Option<InstanceConfig>,
    ) -> Result<CreateInstanceResponse> {
        // Read and encode the DLL file
        let dll_bytes = std::fs::read(dll_path)
            .with_context(|| format!("Failed to read DLL file: {}", dll_path.display()))?;

        let dll_base64 = base64::prelude::BASE64_STANDARD.encode(&dll_bytes);

        let request = serde_json::json!({
            "openzt_dll": dll_base64,
            "config": config,
        });

        let response = self
            .http_client
            .post(self.url("/api/instances"))
            .json(&request)
            .send()
            .await
            .context("Failed to send create instance request")?;

        self.handle_response(response).await
    }

    /// List all instances
    pub async fn list_instances(&self) -> Result<Vec<InstanceDetails>> {
        let response = self
            .http_client
            .get(self.url("/api/instances"))
            .send()
            .await
            .context("Failed to list instances")?;

        self.handle_response(response).await
    }

    /// Get details for a specific instance
    pub async fn get_instance(&self, id: &str) -> Result<InstanceDetails> {
        let response = self
            .http_client
            .get(self.url(&format!("/api/instances/{}", id)))
            .send()
            .await
            .with_context(|| format!("Failed to get instance {}", id))?;

        self.handle_response(response).await
    }

    /// Delete an instance
    pub async fn delete_instance(&self, id: &str) -> Result<()> {
        let response = self
            .http_client
            .delete(self.url(&format!("/api/instances/{}", id)))
            .send()
            .await
            .with_context(|| format!("Failed to delete instance {}", id))?;

        match response.status() {
            StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = self.extract_error(response).await;
                Err(anyhow!("Failed to delete instance: {} - {}", status, error))
            }
        }
    }

    /// Get logs for an instance
    pub async fn get_logs(&self, id: &str) -> Result<String> {
        let response = self
            .http_client
            .get(self.url(&format!("/api/instances/{}/logs", id)))
            .send()
            .await
            .with_context(|| format!("Failed to get logs for instance {}", id))?;

        let logs_response: LogsResponse = self.handle_response(response).await?;
        Ok(logs_response.logs)
    }

    /// Stop a running instance
    pub async fn stop_instance(&self, id: &str) -> Result<InstanceStatusResponse> {
        let response = self
            .http_client
            .post(self.url(&format!("/api/instances/{}/stop", id)))
            .send()
            .await
            .with_context(|| format!("Failed to stop instance {}", id))?;

        self.handle_response(response).await
    }

    /// Start a stopped instance
    pub async fn start_instance(&self, id: &str) -> Result<InstanceStatusResponse> {
        let response = self
            .http_client
            .post(self.url(&format!("/api/instances/{}/start", id)))
            .send()
            .await
            .with_context(|| format!("Failed to start instance {}", id))?;

        self.handle_response(response).await
    }

    /// Restart a running instance
    pub async fn restart_instance(&self, id: &str) -> Result<InstanceStatusResponse> {
        let response = self
            .http_client
            .post(self.url(&format!("/api/instances/{}/restart", id)))
            .send()
            .await
            .with_context(|| format!("Failed to restart instance {}", id))?;

        self.handle_response(response).await
    }

    /// Handle a response, extracting the JSON body or returning an error
    async fn handle_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            response
                .json::<T>()
                .await
                .context("Failed to parse response JSON")
        } else {
            let error = self.extract_error(response).await;
            Err(anyhow!("API error ({}): {}", status.as_u16(), error))
        }
    }

    /// Extract error message from a failed response
    async fn extract_error(&self, response: reqwest::Response) -> String {
        match response.json::<serde_json::Value>().await {
            Ok(json) => {
                if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                    error.to_string()
                } else if let Some(message) = json.get("message").and_then(|m| m.as_str()) {
                    message.to_string()
                } else {
                    format!("Unknown error: {}", json)
                }
            }
            Err(_) => "Unable to parse error response".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = InstanceClient::new("http://localhost:3000");
        assert_eq!(client.base_url, "http://localhost:3000");
        assert_eq!(client.url("/health"), "http://localhost:3000/health");
        assert_eq!(client.url("/api/instances"), "http://localhost:3000/api/instances");
    }

    #[test]
    fn test_url_trimming() {
        let client = InstanceClient::new("http://localhost:3000/");
        assert_eq!(client.url("/health"), "http://localhost:3000/health");
    }
}
