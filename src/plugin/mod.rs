pub mod output;

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::{
    checks, config::Config, exec::CommandRunner, opnsense::config_xml,
    plugin::output::LocalSection, update,
};

pub fn plugin_output(config_path: &PathBuf, config: &mut Config) -> Result<String> {
    let runner = CommandRunner::new(config.security.command_timeout_seconds);
    let update_warning = update::check_and_update(config_path, config)
        .err()
        .map(|err| format!("auto-update failed: {err:#}"));
    let opnsense_config = config_xml::read_config(Path::new("/conf/config.xml"))?;

    let sections =
        checks::collect_all(config, &opnsense_config, &runner, update_warning.as_deref());

    Ok(LocalSection::finalize(sections))
}
