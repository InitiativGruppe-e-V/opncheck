use anyhow::Result;
use clap::Parser;
use opncheck::{
    cli::{Cli, Command},
    config::Config,
    plugin, setup,
};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = if cli.debug { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    match cli.command.unwrap_or(Command::Plugin) {
        Command::Plugin => {
            let mut config = Config::load(&cli.config)?;
            print!("{}", plugin::plugin_output(&cli.config, &mut config)?);
        }
        Command::Config => {
            let config = Config::load(&cli.config)?;
            println!("{}", toml::to_string_pretty(&config)?);
        }
        Command::Setup(options) => {
            setup::run(&cli.config, options)?;
        }
    }
    Ok(())
}
