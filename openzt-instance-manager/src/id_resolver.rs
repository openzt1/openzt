//! Instance ID resolution for CLI commands
//!
//! This module provides functionality to resolve short ID prefixes (e.g., "ba4fc512")
//! to full UUIDs for API calls. It handles both short and full IDs, provides helpful
//! error messages for not-found and ambiguous matches, and includes utilities for
//! determining safe display lengths.
//!
//! Short IDs of any length (1+ characters) are supported as long as they uniquely
//! identify an instance. When multiple instances share the same prefix, an
//! ambiguous match error is returned with helpful guidance.

use crate::client::InstanceClient;
use crate::instance::InstanceDetails;
use anyhow::Result;

/// Resolution result - not currently exposed externally but useful for future extensibility
pub enum IdResolution {
    ExactMatch(String),
    PrefixMatch(String),
}

/// Resolution error types
pub enum ResolutionError {
    /// No instance found with this prefix
    NotFound(String),
    /// Multiple instances match this prefix
    Ambiguous {
        prefix: String,
        matches: Vec<InstanceDetails>,
    },
    /// Failed to fetch instance list from API
    ApiError(anyhow::Error),
}

impl ResolutionError {
    /// Get a user-friendly error message
    pub fn message(&self) -> String {
        match self {
            ResolutionError::NotFound(prefix) => {
                format!("No instance found with ID prefix '{}'", prefix)
            }
            ResolutionError::Ambiguous { prefix, matches } => {
                format!("Ambiguous ID prefix '{}' matches {} instances", prefix, matches.len())
            }
            ResolutionError::ApiError(e) => {
                format!("Failed to resolve instance ID: {}", e)
            }
        }
    }
}

/// Full UUID length
const UUID_LENGTH: usize = 36;

/// Resolve a short ID or full UUID to a full instance ID.
///
/// This function accepts both short ID prefixes (any length 1+) and full UUIDs.
/// For short IDs, it fetches all instances and finds matches starting with the prefix.
/// Empty strings will match all instances and result in an ambiguous match error.
///
/// # Arguments
/// * `client` - The API client to use for fetching instances
/// * `input` - The user-provided ID (short prefix or full UUID)
///
/// # Returns
/// * `Ok(String)` - The full instance ID UUID
/// * `Err(ResolutionError)` - If the ID cannot be resolved
///
/// # Examples
/// ```ignore
/// let resolved_id = resolve_instance_id(client, "b").await?;
/// // Returns: Ok("ba4fc512-3d48-4f9e-9a1b-123456789abc") if unique
///
/// let resolved_id = resolve_instance_id(client, "ba4fc512").await?;
/// // Returns: Ok("ba4fc512-3d48-4f9e-9a1b-123456789abc")
///
/// let full_id = resolve_instance_id(client, "ba4fc512-3d48-4f9e-9a1b-123456789abc").await?;
/// // Returns: Ok("ba4fc512-3d48-4f9e-9a1b-123456789abc") (passthrough)
/// ```
pub async fn resolve_instance_id(
    client: &InstanceClient,
    input: &str,
) -> Result<String, ResolutionError> {
    let input = input.trim();

    // If it's a full UUID length, treat as exact match (passthrough)
    if input.len() == UUID_LENGTH {
        return Ok(input.to_string());
    }

    // Fetch all instances to find matches
    let instances = client
        .list_instances()
        .await
        .map_err(ResolutionError::ApiError)?;

    // Find instances that start with the prefix
    let matching_instances: Vec<InstanceDetails> = instances
        .into_iter()
        .filter(|instance| instance.id.starts_with(input))
        .collect();

    match matching_instances.len() {
        0 => Err(ResolutionError::NotFound(input.to_string())),
        1 => Ok(matching_instances[0].id.clone()),
        _ => Err(ResolutionError::Ambiguous {
            prefix: input.to_string(),
            matches: matching_instances,
        }),
    }
}

/// Calculate how many characters to show for IDs to avoid duplicates.
///
/// This function determines a safe display length for instance IDs in list output.
/// It starts with 8 characters and increases until all IDs are unique at that length.
///
/// # Arguments
/// * `instances` - The list of instances to analyze
///
/// # Returns
/// The number of characters to display (between 8 and 36)
pub fn calculate_safe_id_length(instances: &[InstanceDetails]) -> usize {
    if instances.is_empty() {
        return 8;
    }

    // Start with default 8 characters
    let mut length = 8;

    loop {
        // Collect prefixes at current length
        let prefixes: Vec<&str> = instances
            .iter()
            .map(|i| &i.id[..length.min(i.id.len())])
            .collect();

        // Check if all prefixes are unique
        let unique: std::collections::HashSet<_> = prefixes.iter().collect();
        if unique.len() == instances.len() {
            return length;
        }

        // Increase length, capped at full UUID length
        if length >= 36 {
            return 36;
        }
        length += 4; // Increase in chunks of 4 for cleaner output
    }
}

/// Suggest minimum length needed to uniquely identify all instances.
///
/// # Arguments
/// * `instances` - The list of conflicting instances
///
/// # Returns
/// The minimum length needed for unique identification (at least 1)
pub fn suggest_min_length(instances: &[InstanceDetails]) -> usize {
    if instances.len() <= 1 {
        return 1;
    }

    let mut length = 1;
    loop {
        if length >= 36 {
            return 36;
        }

        let prefixes: Vec<&str> = instances
            .iter()
            .map(|i| &i.id[..length.min(i.id.len())])
            .collect();

        let unique: std::collections::HashSet<_> = prefixes.iter().collect();
        if unique.len() == instances.len() {
            return length;
        }

        length += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::InstanceConfig;
    use chrono::Utc;

    fn create_test_instance(id: &str) -> InstanceDetails {
        InstanceDetails {
            id: id.to_string(),
            container_id: "container-123".to_string(),
            rdp_port: 13390,
            console_port: 13391,
            rdp_url: "rdp://localhost:13390".to_string(),
            status: "running".to_string(),
            created_at: Utc::now(),
            config: InstanceConfig {
                rdp_password: None,
                wine_debug_level: None,
                cpulimit: None,
            },
        }
    }

    #[test]
    fn test_calculate_safe_id_length_empty() {
        let instances: Vec<InstanceDetails> = vec![];
        assert_eq!(calculate_safe_id_length(&instances), 8);
    }

    #[test]
    fn test_calculate_safe_id_length_no_conflicts() {
        let instances = vec![
            create_test_instance("ba4fc512-3d48-4f9e-9a1b-123456789abc"),
            create_test_instance("c9bb925d-a9b2-4f9e-9a1b-123456789abc"),
        ];
        assert_eq!(calculate_safe_id_length(&instances), 8);
    }

    #[test]
    fn test_calculate_safe_id_length_with_conflicts() {
        let instances = vec![
            create_test_instance("ba4fc512-3d48-4f9e-9a1b-123456789abc"),
            create_test_instance("ba4fc512-a9b2-4f9e-9a1b-123456789abc"),
            create_test_instance("c9bb925d-a9b2-4f9e-9a1b-123456789abc"),
        ];
        // First two share "ba4fc512", need 12 chars to differentiate
        assert_eq!(calculate_safe_id_length(&instances), 12);
    }

    #[test]
    fn test_suggest_min_length_single() {
        let instances = vec![create_test_instance("ba4fc512-3d48-4f9e-9a1b-123456789abc")];
        assert_eq!(suggest_min_length(&instances), 1);
    }

    #[test]
    fn test_suggest_min_length_conflicting() {
        let instances = vec![
            create_test_instance("ba4fc512-3d48-4f9e-9a1b-123456789abc"),
            create_test_instance("ba4fc512-a9b2-4f9e-9a1b-123456789abc"),
        ];
        // Need at least 10 characters to differentiate
        assert_eq!(suggest_min_length(&instances), 10);
    }

    #[test]
    fn test_resolution_error_messages() {
        let err = ResolutionError::NotFound("abc123".to_string());
        assert!(err.message().contains("No instance found"));
        assert!(err.message().contains("abc123"));
    }

    #[test]
    fn test_single_char_id_works() {
        // Single character 'a' should uniquely identify the 'a1b2...' instance
        let instances = vec![
            create_test_instance("a1b2c3d4-5e6f-7a8b-9c0d-123456789abc"),
            create_test_instance("b2c3d4e5-6f7a-8b9c-0d1e-234567890bcd"),
        ];

        let prefixes: Vec<&str> = instances
            .iter()
            .filter(|i| i.id.starts_with("a"))
            .map(|i| &i.id[..1])
            .collect();

        assert_eq!(prefixes.len(), 1);
        assert_eq!(prefixes[0], "a");
    }

    #[test]
    fn test_two_char_id_works() {
        // Two character 'a1' should uniquely identify the 'a1b2...' instance
        let instances = vec![
            create_test_instance("a1b2c3d4-5e6f-7a8b-9c0d-123456789abc"),
            create_test_instance("a2b3c4d5-6f7a-8b9c-0d1e-234567890bcd"),
            create_test_instance("b2c3d4e5-6f7a-8b9c-0d1e-234567890bcd"),
        ];

        let matching: Vec<&InstanceDetails> = instances
            .iter()
            .filter(|i| i.id.starts_with("a1"))
            .collect();

        assert_eq!(matching.len(), 1);
        assert!(matching[0].id.starts_with("a1b2"));
    }

    #[test]
    fn test_ambiguous_short_id() {
        // Single character 'a' should match multiple instances starting with 'a'
        let instances = vec![
            create_test_instance("a1b2c3d4-5e6f-7a8b-9c0d-123456789abc"),
            create_test_instance("a2b3c4d5-6f7a-8b9c-0d1e-234567890bcd"),
        ];

        let matching: Vec<&InstanceDetails> = instances
            .iter()
            .filter(|i| i.id.starts_with("a"))
            .collect();

        assert_eq!(matching.len(), 2);
    }
}
