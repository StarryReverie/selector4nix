use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "selector4nix", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to the configuration file
    #[arg(long = "config-file", value_name = "PATH")]
    pub config_file: Option<PathBuf>,

    /// Path to the credential file
    #[arg(long = "credential-file", value_name = "PATH")]
    pub credential_file: Option<PathBuf>,

    /// Path to the cache directory
    #[arg(long = "cache-dir", value_name = "PATH")]
    pub cache_dir: Option<PathBuf>,

    /// Path to the log file to which log output gets appended
    #[arg(long = "log-file", value_name = "PATH")]
    pub log_file: Option<PathBuf>,

    /// Log verbosity level
    #[arg(long = "log-level", value_name = "LEVEL")]
    pub log_level: Option<LogLevel>,

    /// Whether to hide timestamps from log output
    #[arg(long = "no-log-timestamp", default_value_t = false)]
    pub no_log_timestamp: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start HTTP server (default)
    Serve,
    /// Validate configuration file
    Check,
}

#[derive(Clone, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let s = match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        };
        f.write_str(s)
    }
}
