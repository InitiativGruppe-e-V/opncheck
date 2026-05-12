pub mod extensions;
pub mod output;

use anyhow::Result;

use crate::{checks, config::Config, exec::CommandRunner, tasks};

pub fn dump(config: &Config) -> Result<String> {
    let runner = CommandRunner::new(config.security.command_timeout_seconds);
    let mut out = output::AgentOutput::new();

    checks::collect_all(&mut out, config, &runner);
    extensions::collect_plugins(&mut out, config, &runner);
    extensions::collect_local(&mut out, config, &runner);
    extensions::collect_spool(&mut out, config);
    tasks::collect(&mut out, config, &runner);

    Ok(out.finish())
}
