//! Output formatting for the CLI
//!
//! This module provides utilities for formatting and displaying output
//! in various formats (table, JSON) with colored terminal output.

use crate::instance::{CreateInstanceResponse, InstanceDetails, LogsResponse};
use console::{style, Color};
use tabled::{
    settings::{
        object::Rows,
        Alignment, Modify, Style,
    },
    Table, Tabled,
};

// Import ID resolution support for CLI-only error display
#[cfg(feature = "cli")]
use crate::id_resolver::{calculate_safe_id_length, ResolutionError};

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
}

impl OutputFormat {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "table" => Some(Self::Table),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// Print a success message with green checkmark
pub fn print_success(msg: &str) {
    println!("{} {}", style("✓").fg(Color::Green), msg);
}

/// Print an error message with red X
pub fn print_error(msg: &str) {
    eprintln!("{} {}", style("✗").fg(Color::Red), style(msg).fg(Color::Red));
}

/// Print an info message with blue info icon
pub fn print_info(msg: &str) {
    println!("{} {}", style("ℹ").fg(Color::Cyan), style(msg).fg(Color::Cyan));
}

/// Print a warning message with yellow warning icon
pub fn print_warning(msg: &str) {
    eprintln!("{} {}", style("⚠").fg(Color::Yellow), style(msg).fg(Color::Yellow));
}

/// Print instance details
pub fn print_instance(instance: &InstanceDetails, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(instance) {
                println!("{}", json);
            }
        }
        OutputFormat::Table => {
            print_instance_table(instance);
        }
    }
}

/// Print instance in table format
fn print_instance_table(instance: &InstanceDetails) {
    println!();
    println!("  {} {}", style("ID:").fg(Color::Cyan), &instance.id[..8]);
    println!(
        "  {} {}",
        style("Created:").fg(Color::Cyan),
        instance.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!(
        "  {} {}",
        style("RDP URL:").fg(Color::Cyan),
        style(&instance.rdp_url).fg(Color::Green).bold()
    );
    println!("  {} {}", style("RDP Port:").fg(Color::Cyan), instance.rdp_port);
    println!("  {} {}", style("Console:").fg(Color::Cyan), instance.console_port);
    println!(
        "  {} {}",
        style("Status:").fg(Color::Cyan),
        format_status(&instance.status)
    );
    if let Some(ref password) = instance.config.rdp_password {
        println!("  {} {}", style("RDP Password:").fg(Color::Cyan), password);
    }
    if !instance.container_id.is_empty() {
        println!("  {} {}", style("Container:").fg(Color::Cyan), &instance.container_id[..12]);
    }
    println!();
}

/// Format status with color
fn format_status(status: &str) -> String {
    match status {
        "running" => style(status).fg(Color::Green).bold().to_string(),
        "creating" => style(status).fg(Color::Yellow).bold().to_string(),
        "stopped" => style(status).fg(Color::Black).bold().to_string(),
        s if s.starts_with("error:") || s.starts_with("Error") => style(status).fg(Color::Red).bold().to_string(),
        _ => style(status).fg(Color::Red).bold().to_string(),
    }
}

/// Print a list of instances
pub fn print_instance_list(instances: &[InstanceDetails], format: OutputFormat) {
    if instances.is_empty() {
        print_info("No instances found");
        return;
    }

    match format {
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(instances) {
                println!("{}", json);
            }
        }
        OutputFormat::Table => {
            print_instance_list_table(instances);
        }
    }
}

/// Print instance list in table format
#[cfg(not(feature = "cli"))]
fn print_instance_list_table(instances: &[InstanceDetails]) {
    // Stub for non-CLI builds
    let _ = instances;
}

/// Print instance list in table format
#[cfg(feature = "cli")]
fn print_instance_list_table(instances: &[InstanceDetails]) {
    #[derive(Tabled)]
    #[tabled(rename_all = "PASCAL")]
    struct InstanceRow {
        #[tabled(rename = "ID")]
        id: String,
        #[tabled(rename = "Created")]
        created_at: String,
        #[tabled(rename = "RDP Port")]
        rdp_port: u16,
        #[tabled(rename = "Console")]
        console_port: u16,
        #[tabled(rename = "Status")]
        status: String,
        #[tabled(rename = "RDP URL")]
        rdp_url: String,
    }

    // Calculate safe ID length to avoid duplicates
    let id_length = calculate_safe_id_length(instances);

    let rows: Vec<InstanceRow> = instances
        .iter()
        .map(|i| InstanceRow {
            id: i.id[..id_length.min(i.id.len())].to_string(),
            created_at: i.created_at.format("%Y-%m-%d %H:%M").to_string(),
            rdp_port: i.rdp_port,
            console_port: i.console_port,
            status: i.status.clone(),
            rdp_url: i.rdp_url.clone(),
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::modern());
    table.with(Modify::new(Rows::new(1..)).with(Alignment::left()));

    println!();
    println!("{}", table);
    println!();
}

/// Print logs output
pub fn print_logs(logs_response: &LogsResponse, output_json: bool) {
    if output_json {
        if let Ok(json) = serde_json::to_string_pretty(logs_response) {
            println!("{}", json);
        }
    } else {
        println!("Logs for instance {}:", style(&logs_response.instance_id[..8]).fg(Color::Cyan));
        println!();
        if logs_response.logs.is_empty() {
            print_info("(no logs available)");
        } else {
            println!("{}", logs_response.logs);
        }
    }
}

/// Print the result of creating an instance
pub fn print_create_result(response: &CreateInstanceResponse, output_json: bool) {
    if output_json {
        if let Ok(json) = serde_json::to_string_pretty(response) {
            println!("{}", json);
        }
    } else {
        println!();
        print_success(&format!("Created instance: {}", response.instance_id));
        println!("  {} {}", style("RDP URL:").fg(Color::Cyan), style(&response.rdp_url).fg(Color::Green));
        println!(
            "  {} {}",
            style("Console:").fg(Color::Cyan),
            response.console_port
        );
        println!(
            "  {} {}",
            style("Status:").fg(Color::Cyan),
            format_status(&response.status)
        );
        println!();
    }
}

/// Print health check result
pub fn print_health(healthy: bool, output_json: bool) {
    if output_json {
        println!("{}", serde_json::json!({ "healthy": healthy }));
    } else if healthy {
        print_success("API server is healthy");
    } else {
        print_error("API server is not responding");
    }
}

/// Print confirmation prompt and return true if user confirms
pub fn confirm_action(action: &str, target: &str) -> bool {
    print_warning(&format!("About to {}: {}", action, target));
    print!("Continue? [y/N] ");
    use std::io::Write;
    std::io::stdout().flush().ok();

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .ok();

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Print an ID resolution error with helpful context
#[cfg(feature = "cli")]
pub fn print_resolution_error(error: &ResolutionError) {
    match error {
        ResolutionError::NotFound(_prefix) => {
            print_error(&error.message());
            print_info("Use 'openzt list' to see available instances");
        }
        ResolutionError::Ambiguous { prefix: _, matches } => {
            print_error(&error.message());
            print_info("Matching instances:");
            print_ambiguous_matches(matches);
            let min_len = crate::id_resolver::suggest_min_length(matches);
            print_info(&format!("Use at least {} characters to uniquely identify", min_len));
        }
        ResolutionError::ApiError(_e) => {
            print_error(&error.message());
        }
    }
}

/// Print abbreviated table of ambiguous matches
#[cfg(feature = "cli")]
fn print_ambiguous_matches(matches: &[InstanceDetails]) {
    #[derive(Tabled)]
    #[tabled(rename_all = "PASCAL")]
    struct AmbiguousRow {
        #[tabled(rename = "ID")]
        id: String,
        #[tabled(rename = "Created")]
        created_at: String,
        #[tabled(rename = "Status")]
        status: String,
    }

    let rows: Vec<AmbiguousRow> = matches
        .iter()
        .map(|i| AmbiguousRow {
            id: truncate_id(&i.id, 12),
            created_at: i.created_at.format("%Y-%m-%d %H:%M").to_string(),
            status: i.status.clone(),
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::modern());
    table.with(Modify::new(Rows::new(1..)).with(Alignment::left()));

    println!("{}", table);
}

/// Truncate an ID to a specified length for display
#[cfg(feature = "cli")]
fn truncate_id(id: &str, length: usize) -> String {
    let safe_len = length.min(id.len());
    let truncated = &id[..safe_len];
    if safe_len < id.len() {
        format!("{}...", truncated)
    } else {
        truncated.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::from_str("table"), Some(OutputFormat::Table));
        assert_eq!(OutputFormat::from_str("TABLE"), Some(OutputFormat::Table));
        assert_eq!(OutputFormat::from_str("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("JSON"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("invalid"), None);
    }
}
