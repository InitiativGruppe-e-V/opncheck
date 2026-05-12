use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "OPNsense Checkmk FreeBSD agent plugin")]
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
    /// Print plugin output for the stock Checkmk FreeBSD agent.
    Plugin,
    /// Print a redacted view of the effective configuration.
    Config,
}
