pub mod section;

use crate::scripts;
use anyhow::Result;
use std::path::Path;

pub use section::{LocalSection, LocalState, collect_sections};

use crate::{
    checks::{self, Check},
    config::Config,
    platform::Platform,
    platform::*,
    runner::CommandRunner,
    update,
};

pub fn plugin_output(config_path: &Path, config: &mut Config) -> Result<String> {
    #[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
    return plugin_output_for::<OPNSenseX64>(config_path, config, checks::opnsense_checks());
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return plugin_output_for::<LinuxX64>(config_path, config, checks::linux_checks());
}

fn plugin_output_for<P: Platform>(
    config_path: &Path,
    config: &mut Config,
    checks: &'static [&'static dyn Check<P>],
) -> Result<String> {
    let platform_data = P::platform_data()?;
    let runner = CommandRunner::new(config.security.command_timeout_seconds);
    let update_result = update::check_and_update(config_path, config);

    let sections = checks::collect_all::<P>(config, &platform_data, checks, &runner, update_result);
    let script_output = scripts::collect::<P>(config, &runner)?;

    let mut output = collect_sections(sections);
    output.push_str(&script_output);
    Ok(output)
}
