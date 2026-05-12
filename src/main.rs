use anyhow::Result;
use clap::Parser;
use opncheck::{
    agent,
    cli::{Cli, Command},
    config::Config,
};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = if cli.debug { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let config = Config::load(&cli.config)?;
    match cli.command.unwrap_or(Command::Plugin) {
        Command::Plugin => {
            print!("{}", agent::plugin_output(&config)?);
        }
        Command::Config => {
            println!("{}", toml::to_string_pretty(&config)?);
        }
    }
    Ok(())
}
