//! Centralized logging initialization for OpenZT
//!
//! This module provides a single entry point for initializing console and logging
//! across all contexts: main app, integration tests, and reimplementation tests.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::sync::Mutex;
use tracing_subscriber::filter::LevelFilter;

#[cfg(target_os = "windows")]
use windows::Win32::System::Console::{AllocConsole, FreeConsole};

static LOGGING_INITIALIZED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Initialize console and logging with the given configuration
///
/// This is the centralized function that should be used by all entry points.
/// On Windows, this allocates a new console before initializing logging.
///
/// # Arguments
/// * `config` - Logging configuration specifying log level and file logging preference
///
/// # Returns
/// * `Ok(())` if logging was initialized successfully
/// * `Err(...)` if logging was already initialized or console allocation failed
pub fn init_with_console(config: &LoggingConfig) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    init_console()?;

    init(config)
}

#[cfg(target_os = "windows")]
fn init_console() -> windows::core::Result<()> {
    unsafe { FreeConsole()? };
    unsafe { AllocConsole()? };
    Ok(())
}

/// Initialize logging with settings from the given configuration
///
/// This should be called AFTER config is loaded and console is allocated.
///
/// # Arguments
/// * `config` - Logging configuration specifying log level and file logging preference
///
/// # Returns
/// * `Ok(())` if logging was initialized successfully
/// * `Err(...)` if logging was already initialized
fn init(config: &LoggingConfig) -> anyhow::Result<()> {
    let mut initialized = LOGGING_INITIALIZED.lock().unwrap();
    if *initialized {
        return Err(anyhow::anyhow!("Logging already initialized"));
    }

    let enable_ansi = enable_ansi_support::enable_ansi_support().is_ok();

    #[cfg(not(feature = "integration-tests"))]
    let level_filter = config.level.to_level_filter();
    #[cfg(feature = "integration-tests")]
    let level_filter = LevelFilter::TRACE; // Force TRACE level for integration tests

    #[cfg(not(feature = "integration-tests"))]
    let log_to_file = config.log_to_file;
    #[cfg(feature = "integration-tests")]
    let log_to_file = true;

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    // Set up file logging if enabled
    if log_to_file {
        let log_path = crate::util::get_base_path().join("openzt.log");
        match std::fs::File::create(&log_path) {
            Ok(log_file) => {
                // NOTE: There's some weird trait errors if we try to extract the console layer to be initial
                let console_layer = tracing_subscriber::fmt::layer()
                    .with_ansi(enable_ansi)
                    .with_writer(std::io::stdout)
                    .with_filter(level_filter);
                // Wrap in non-blocking writer
                let (non_blocking, _guard) = tracing_appender::non_blocking(log_file);

                let file_layer = tracing_subscriber::fmt::layer()
                    .with_ansi(false) // No ANSI codes in file
                    .with_writer(non_blocking)
                    .with_filter(level_filter);

                // Initialize with both console and file layers
                tracing_subscriber::registry().with(file_layer).with(console_layer).init();

                // Store guard to prevent it from being dropped
                // Note: We need to leak this guard to keep file logging active
                std::mem::forget(_guard);

                eprintln!("Logging initialized: level={:?}, file={}", config.level, log_path.display());
            }
            Err(e) => {
                let console_layer = tracing_subscriber::fmt::layer()
                    .with_ansi(enable_ansi)
                    .with_writer(std::io::stdout)
                    .with_filter(level_filter);
                // Fall back to console-only if file creation fails
                tracing_subscriber::registry().with(console_layer).init();

                eprintln!("Failed to create openzt.log: {}", e);
                eprintln!("Logging initialized: level={:?}, console only", config.level);
            }
        }
    } else {
        let console_layer = tracing_subscriber::fmt::layer()
            .with_ansi(enable_ansi)
            .with_writer(std::io::stdout)
            .with_filter(level_filter);
        // Console-only logging
        tracing_subscriber::registry().with(console_layer).init();

        eprintln!("Logging initialized: level={:?}, console only", config.level);
    }

    *initialized = true;
    Ok(())
}

/// Logging configuration section
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    /// Enable file logging to openzt.log (default: true)
    #[serde(default = "default_true")]
    pub log_to_file: bool,

    /// Log level (default: Warn)
    #[serde(default)]
    pub level: LogLevel,
}

/// Log level setting for OpenZT logging
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    #[default]
    Warn,
    Error,
}

impl LogLevel {
    /// Convert to tracing's LevelFilter
    pub fn to_level_filter(self) -> LevelFilter {
        match self {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            log_to_file: true,
            level: LogLevel::Warn,
        }
    }
}

fn default_true() -> bool {
    true
}
