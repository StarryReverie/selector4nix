use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "selector4nix", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(long = "config-file", value_name = "PATH")]
    pub config_file: Option<PathBuf>,

    #[arg(long = "log-level", value_name = "LEVEL")]
    pub log_level: Option<LogLevel>,
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
