//! OpenZT Instance Manager CLI
//!
//! Command-line client for managing Zoo Tycoon Docker instances.

use std::path::PathBuf;

// Conditionally include CLI dependencies
#[cfg(feature = "cli")]
use base64::Engine;
#[cfg(feature = "cli")]
use openzt_instance_manager::id_resolver::resolve_instance_id;

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
        Commands::Logs { id, log_type, follow, tail } => {
            cmd_logs(&client, &id, &log_type, follow, tail, output_format).await
        }
        Commands::Stop { id } => cmd_stop(&client, &id, output_format).await,
        Commands::Start { id } => cmd_start(&client, &id, output_format).await,
        Commands::Restart { id } => cmd_restart(&client, &id, output_format).await,
        Commands::Health {} => cmd_health(&client, output_format).await,
        Commands::DetourResults { id } => cmd_detour_results(&client, &id, output_format).await,
        Commands::UploadScript { id, script_path } => {
            cmd_upload_script(&client, &id, &script_path, output_format).await
        }
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
        /// Instance ID (full UUID or short prefix)
        id: String,
    },

    /// Delete an instance
    Delete {
        /// Instance ID (full UUID or short prefix)
        id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        confirm: bool,
    },

    /// Get instance logs
    Logs {
        /// Instance ID (full UUID or short prefix)
        id: String,

        /// Log type to read (docker, openzt, integration-tests)
        #[arg(long, default_value = "openzt")]
        log_type: String,

        /// Follow log output in real-time
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, long, default_value = "100")]
        tail: usize,
    },

    /// Check API health
    Health {},

    /// Stop a running instance
    Stop {
        /// Instance ID (full UUID or short prefix)
        id: String,
    },

    /// Start a stopped instance
    Start {
        /// Instance ID (full UUID or short prefix)
        id: String,
    },

    /// Restart a running instance
    Restart {
        /// Instance ID (full UUID or short prefix)
        id: String,
    },

    /// Show detour validation results for an instance
    DetourResults {
        /// Instance ID (full UUID or short prefix)
        id: String,
    },

    /// Upload a Lua script to a running instance
    UploadScript {
        /// Instance ID (full UUID or short prefix)
        id: String,

        /// Path to the Lua script file
        script_path: PathBuf,
    },
}

#[cfg(feature = "cli")]
#[derive(Args, Clone)]
struct InstanceConfigArgs {
    /// CPU limit in cores (e.g., 0.5 = 50%, 2.0 = 2 cores)
    #[arg(long)]
    cpulimit: Option<f64>,

    /// Comma-separated list of detour names to validate (e.g. bfapp/load_string,bfapp/win_main)
    #[arg(long, value_delimiter = ',')]
    validate_detours: Vec<String>,

    /// Lua script files to upload to the instance
    #[arg(long = "script", value_name = "FILE")]
    scripts: Vec<PathBuf>,
}

#[cfg(feature = "cli")]
async fn cmd_create(
    client: &openzt_instance_manager::client::InstanceClient,
    dll_path: &PathBuf,
    config_args: InstanceConfigArgs,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::instance::{InstanceConfig, ScriptFile};
    use openzt_instance_manager::output::{print_create_result, print_error};

    // Check if DLL file exists
    if !dll_path.exists() {
        print_error(&format!("DLL file not found: {}", dll_path.display()));
        std::process::exit(1);
    }

    // Load and encode scripts if provided
    let scripts: Vec<ScriptFile> = config_args
        .scripts
        .iter()
        .map(|path| {
            let content = std::fs::read(path)
                .map_err(|e| miette!("Failed to read script {}: {}", path.display(), e))?;
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| miette!("Invalid script filename"))?
                .to_string();
            let content_base64 = base64::prelude::BASE64_STANDARD.encode(&content);
            Ok::<_, miette::ErrReport>(ScriptFile {
                filename,
                content: content_base64,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|e| {
            print_error(&format!("Failed to load scripts: {}", e));
            std::process::exit(1);
        });

    // Build instance config
    let validate_detours = if config_args.validate_detours.is_empty() {
        None
    } else {
        Some(config_args.validate_detours.clone())
    };
    let instance_config = if config_args.cpulimit.is_some() || validate_detours.is_some() {
        Some(InstanceConfig {
            wine_debug_level: None,
            cpulimit: config_args.cpulimit,
            validate_detours,
        })
    } else {
        None
    };

    // Call the API
    let response = client
        .create_instance(dll_path, instance_config, scripts)
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
    use openzt_instance_manager::output::{print_error, print_instance, print_resolution_error};

    // Resolve ID (handles both short and full UUIDs)
    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    match client.get_instance(&resolved_id).await {
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
    use openzt_instance_manager::output::{confirm_action, print_error, print_resolution_error, print_success};

    // Resolve ID (handles both short and full UUIDs)
    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    // Confirm unless --confirm flag was provided
    if !confirm {
        if !confirm_action("delete instance", &format!("ID: {}", &resolved_id[..8])) {
            openzt_instance_manager::output::print_info("Delete cancelled");
            return Ok(());
        }
    }

    match client.delete_instance(&resolved_id).await {
        Ok(()) => {
            if output_format != openzt_instance_manager::output::OutputFormat::Json {
                print_success(&format!("Deleted instance: {}", &resolved_id[..8]));
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
    log_type: &str,
    follow: bool,
    tail: usize,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use futures_util::StreamExt;
    use openzt_instance_manager::instance::LogsResponse;
    use openzt_instance_manager::output::{print_error, print_info, print_logs, print_resolution_error};

    // Validate log type
    if !matches!(log_type, "docker" | "openzt" | "integration-tests") {
        print_error(&format!(
            "Invalid log type: '{}'. Valid types are: docker, openzt, integration-tests",
            log_type
        ));
        std::process::exit(1);
    }

    // Resolve ID (handles both short and full UUIDs)
    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    if follow {
        let mut stream = client.stream_logs(&resolved_id, Some(log_type)).await
            .map_err(|e| miette!(e))?;

        print_info(&format!(
            "Streaming {} logs for instance {} (Ctrl+C to stop)...",
            log_type,
            &resolved_id[..8]
        ));
        println!();

        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => {
                    // Parse SSE format from the chunk
                    // SSE format: "data: <line>\n\n" or ": keep-alive\n\n"
                    for line in chunk.lines() {
                        if let Some(log_line) = line.strip_prefix("data: ") {
                            if !log_line.is_empty() {
                                println!("{}", log_line);
                            }
                        }
                        // Ignore ": keep-alive" and other comment lines
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
        Ok(())
    } else {
        match client.get_logs(&resolved_id, Some(log_type), Some(tail as u32)).await {
            Ok(logs) => {
                let response = LogsResponse {
                    instance_id: resolved_id,
                    log_type: log_type.to_string(),
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

#[cfg(feature = "cli")]
async fn cmd_detour_results(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::{print_detour_results, print_error, print_resolution_error};

    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    match client.get_detour_results(&resolved_id).await {
        Ok(results) => {
            let output_json = output_format == openzt_instance_manager::output::OutputFormat::Json;
            print_detour_results(&results, output_json);
            if !results.passed {
                std::process::exit(1);
            }
        }
        Err(e) => {
            print_error(&format!("Failed to get detour results: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_upload_script(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    script_path: &PathBuf,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::{print_error, print_resolution_error, print_success};

    // Check if script file exists
    if !script_path.exists() {
        print_error(&format!("Script file not found: {}", script_path.display()));
        std::process::exit(1);
    }

    // Resolve ID (handles both short and full UUIDs)
    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    match client.upload_script(&resolved_id, script_path).await {
        Ok(response) => {
            if output_format != openzt_instance_manager::output::OutputFormat::Json {
                print_success(&format!(
                    "Uploaded script '{}' to instance {}",
                    response.filename,
                    &response.instance_id[..8]
                ));
                println!("  Path: {}", response.path);
            } else {
                println!("{}", serde_json::to_string_pretty(&response).unwrap());
            }
        }
        Err(e) => {
            print_error(&format!("Failed to upload script: {}", e));
            std::process::exit(1);
        }
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

#[cfg(feature = "cli")]
async fn cmd_stop(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::{print_error, print_resolution_error, print_success};

    // Resolve ID (handles both short and full UUIDs)
    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    match client.stop_instance(&resolved_id).await {
        Ok(response) => {
            if output_format != openzt_instance_manager::output::OutputFormat::Json {
                print_success(&format!("Stopped instance: {}", &response.id[..8]));
            } else {
                println!("{}", serde_json::to_string_pretty(&response).unwrap());
            }
        }
        Err(e) => {
            print_error(&format!("Failed to stop instance: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_start(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::{print_error, print_resolution_error, print_success};

    // Resolve ID (handles both short and full UUIDs)
    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    match client.start_instance(&resolved_id).await {
        Ok(response) => {
            if output_format != openzt_instance_manager::output::OutputFormat::Json {
                print_success(&format!("Started instance: {}", &response.id[..8]));
            } else {
                println!("{}", serde_json::to_string_pretty(&response).unwrap());
            }
        }
        Err(e) => {
            print_error(&format!("Failed to start instance: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn cmd_restart(
    client: &openzt_instance_manager::client::InstanceClient,
    id: &str,
    output_format: openzt_instance_manager::output::OutputFormat,
) -> Result<()> {
    use openzt_instance_manager::output::{print_error, print_resolution_error, print_success};

    // Resolve ID (handles both short and full UUIDs)
    let resolved_id = match resolve_instance_id(client, id).await {
        Ok(resolved) => resolved,
        Err(e) => {
            print_resolution_error(&e);
            std::process::exit(1);
        }
    };

    match client.restart_instance(&resolved_id).await {
        Ok(response) => {
            if output_format != openzt_instance_manager::output::OutputFormat::Json {
                print_success(&format!("Restarted instance: {}", &response.id[..8]));
            } else {
                println!("{}", serde_json::to_string_pretty(&response).unwrap());
            }
        }
        Err(e) => {
            print_error(&format!("Failed to restart instance: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}
