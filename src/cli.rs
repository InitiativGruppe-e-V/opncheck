use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Checkmk agent plugin for OPNsense and Linux hosts")]
pub struct Cli {
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print plugin output for the stock Checkmk agent.
    Plugin,
    /// Print a redacted view of the effective configuration.
    Config,
    /// Check GitHub for an opncheck update immediately.
    Update,
    /// Install opncheck as a Checkmk agent plugin on this host.
    Setup(SetupOptions),
}

#[derive(Debug, Clone, Parser)]
pub struct SetupOptions {
    /// Do not prompt for optional setup choices.
    #[arg(long)]
    pub yes: bool,

    /// Checkmk site's ssh-ed25519 public key to install for agent access.
    #[arg(long)]
    pub checkmk_key: Option<String>,

    /// Enable opncheck auto-updates in the configuration.
    #[arg(long, conflicts_with = "disable_updates")]
    pub enable_updates: bool,

    /// Disable opncheck auto-updates in the configuration.
    #[arg(long, conflicts_with = "enable_updates")]
    pub disable_updates: bool,
}
