//! OpenZT Instance Manager CLI
//!
//! Command-line client for managing Zoo Tycoon Docker instances.

use std::path::PathBuf;

// Conditionally include CLI dependencies
#[cfg(feature = "cli")]
use clap::{Parser, Subcommand, Args};
#[cfg(feature = "cli")]
use miette::{miette, Result};

// Conditionally compile the CLI
#[cfg(feature = "cli")]
#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = openzt_instance_manager::client_config::ClientConfig::load();

    let cli = Cli::parse();

    // Determine API URL: CLI flag > config file > default
    let api_url = cli
        .global
        .api_url
        .unwrap_or_else(|| config.api.base_url.clone());

    // Determine output format: CLI flag > config file > table default
    let output_format = cli
        .global
        .output
        .and_then(|s| openzt_instance_manager::output::OutputFormat::from_str(&s))
        .or_else(|| config.output_format())
        .unwrap_or(openzt_instance_manager::output::OutputFormat::Table);

    // Create HTTP client
    let client = openzt_instance_manager::client::InstanceClient::new(api_url);

    // Execute the appropriate subcommand
    match cli.command {
        Commands::Create { dll_path, config: instance_config } => {
            cmd_create(&client, &dll_path, instance_config, output_format).await
        }
        Commands::List {} => cmd_list(&client, output_format).await,
        Commands::Get { id } => cmd_get(&client, &id, output_format).await,
        Commands::Delete { id, confirm } => cmd_delete(&client, &id, confirm, output_format).await,
        Commands::Logs { id, follow, tail } => {
            cmd_logs(&client, &id, follow, tail, output_format).await
        }
        Commands::Health {} => cmd_health(&client, output_format).await,
    }
}

#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(name = "openzt")]
#[command(about = "OpenZT Instance Manager CLI", long_about = None)]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    global: GlobalArgs,
}

#[cfg(feature = "cli")]
#[derive(Args)]
#[group(multiple = false)]
struct GlobalArgs {
    /// API URL
    #[arg(long, global = true)]
    api_url: Option<String>,

    /// Output format (table or json)
    #[arg(long, global = true, value_name = "FORMAT")]
    output: Option<String>,
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum Commands {
    /// Create a new instance
    Create {
        /// Path to the openzt.dll file
        dll_path: PathBuf,

        #[command(flatten)]
        config: InstanceConfigArgs,
    },

    /// List all instances
    List {},

    /// Get instance details
    Get {
        /// Instance ID
        id: String,
    },

    /// Delete an instance
    Delete {
        /// Instance ID
        id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        confirm: bool,
    },

    /// Get instance logs
    Logs {
        /// Instance ID
        id: String,

        /// Follow log output (not yet implemented)
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, long, default_value = "100")]
        tail: usize,
    },

    /// Check API health
    Health {},
}

#[cfg(feature = "cli")]
#[derive(Args, Clone)]
struct InstanceConfigArgs {
    /// Optional RDP password
    #[arg(long)]
    rdp_password: Option<String>,
}

#[cfg(feature = "cli")]
async fn cmd_create(
    client: &openzt_instance_manager::client::InstanceClient,
    dll_path: &PathBuf,
    config_args: InstanceConfigArgs,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::instance::InstanceConfig;
    use openzt_instance_manager::output::{print_create_result, print_error};

    // Check if DLL file exists
    if !dll_path.exists() {
        print_error(&format!("DLL file not found: {}", dll_path.display()));
        std::process::exit(1);
    }

    // Build instance config
    let instance_config = if config_args.rdp_password.is_some() {
        Some(InstanceConfig {
            rdp_password: config_args.rdp_password,
            wine_debug_level: None,
        })
    } else {
        None
    };

    // Call the API
    let response = client
        .create_instance(dll_path, instance_config)
        .await
        .map_err(|e| miette!(e))?;

    // Print result
    let output_json = output_format == openzt_instance_manager::output::OutputFormat::Json;
    print_create_result(&response, output_json);

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_list(
    client: &openzt_instance_manager::client::InstanceClient,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::print_instance_list;

    let instances = client
        .list_instances()
        .await
        .map_err(|e| miette!(e))?;

    print_instance_list(&instances, output_format);

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_get(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::{print_error, print_instance};

    match client.get_instance(id).await {
        Ok(instance) => print_instance(&instance, output_format),
        Err(e) => {
            print_error(&format!("Failed to get instance: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_delete(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    confirm: bool,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::{confirm_action, print_error, print_success};

    // Confirm unless --confirm flag was provided
    if !confirm {
        if !confirm_action("delete instance", &format!("ID: {}", id)) {
            openzt_instance_manager::output::print_info("Delete cancelled");
            return Ok(());
        }
    }

    match client.delete_instance(id).await {
        Ok(()) => {
            if output_format != openzt_instance_manager::output::OutputFormat::Json {
                print_success(&format!("Deleted instance: {}", id));
            }
        }
        Err(e) => {
            print_error(&format!("Failed to delete instance: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_logs(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    follow: bool,
    _tail: usize,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::instance::LogsResponse;
    use openzt_instance_manager::output::{print_error, print_logs};

    if follow {
        openzt_instance_manager::output::print_warning("Log streaming not yet implemented");
    }

    match client.get_logs(id).await {
        Ok(logs) => {
            let response = LogsResponse {
                instance_id: id.to_string(),
                logs,
            };
            let output_json = output_format == openzt_instance_manager::output::OutputFormat::Json;
            print_logs(&response, output_json);
        }
        Err(e) => {
            print_error(&format!("Failed to get logs: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_health(
    client: &openzt_instance_manager::client::InstanceClient,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::print_health;

    let healthy = client.health().await.map_err(|e| miette!(e))?;

    let output_json = output_format == openzt_instance_manager::output::OutputFormat::Json;
    print_health(healthy, output_json);

    // Exit with error code if unhealthy (unless JSON output)
    if !healthy && !output_json {
        std::process::exit(1);
    }

    Ok(())
}

// Stub for when CLI feature is not enabled
#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("Error: The 'openzt' CLI binary requires the 'cli' feature to be enabled.");
    eprintln!("Please build with: cargo build --bin openzt --features cli");
    std::process::exit(1);
}
