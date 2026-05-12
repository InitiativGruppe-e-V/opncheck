use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "OPNsense Checkmk agent")]
pub struct Cli {
    #[arg(short, long, default_value = "/usr/local/etc/opncheck.toml")]
    pub config: PathBuf,

    #[arg(long)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print Checkmk agent output to stdout. Recommended for SSH transport.
    Dump,
    /// Print a redacted view of the effective configuration.
    Config,
}
