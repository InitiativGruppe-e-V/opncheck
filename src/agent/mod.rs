pub mod output;

use anyhow::Result;

use crate::{checks, config::Config, exec::CommandRunner};

pub fn plugin_output(config: &Config) -> Result<String> {
    let runner = CommandRunner::new(config.security.command_timeout_seconds);

    let out = checks::collect_all(config, &runner);

    Ok(out.finish())
}
