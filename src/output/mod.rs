pub mod section;

use anyhow::Result;
use std::path::Path;

pub use section::{LocalSection, LocalState, collect_sections};

use crate::{checks, config::Config, runner::CommandRunner, update, xml};

pub fn plugin_output(config_path: &Path, config: &mut Config) -> Result<String> {
    let runner = CommandRunner::new(config.security.command_timeout_seconds);
    let update_result = update::check_and_update(config_path, config);
    let opnsense_config = xml::read_config(Path::new("/conf/config.xml"))?;

    let sections = checks::collect_all(config, &opnsense_config, &runner, update_result);

    Ok(collect_sections(sections))
}
