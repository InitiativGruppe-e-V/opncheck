pub mod checks;
pub mod cli;
pub mod config;
pub mod output;
pub mod platform;
pub mod runner;
pub mod scripts;
pub mod setup;
pub mod update;
pub mod utils;
pub mod xml;

use crate::{
    cli::{Cli, Command},
    config::Config,
    platform::{CurrentPlatform, Platform},
};

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(CurrentPlatform::config_path);
    let filter = if cli.debug { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    match cli.command.unwrap_or(Command::Plugin) {
        Command::Plugin => {
            let mut config = Config::load(&config_path)?;
            print!("{}", output::plugin_output(&config_path, &mut config)?);
        }
        Command::Config => {
            let config = Config::load(&config_path)?;
            println!("{}", toml::to_string_pretty(&config)?);
        }
        Command::Update => {
            let mut config = Config::load(&config_path)?;
            let outcome = update::check_for_update()?;
            println!("{}", outcome.summary());

            if let update::UpdateOutcome::UpdateAvailable { .. } = outcome {
                let confirmed = dialoguer::Confirm::new()
                    .with_prompt("Do you want to update opncheck now?")
                    .default(true)
                    .interact()?;

                if confirmed {
                    let outcome = update::update_now(&config_path, &mut config)?;
                    println!("{}", outcome.summary());
                }
            }
        }
        Command::Setup(options) => {
            setup::run(&config_path, &options)?;
        }
    }
    Ok(())
}
