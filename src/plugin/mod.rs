pub mod output;

use anyhow::Result;
use std::path::Path;

use crate::{
    checks, config::Config, exec::CommandRunner, opnsense::config_xml, plugin::output::LocalSection,
};

pub fn plugin_output(config: &Config) -> Result<String> {
    let runner = CommandRunner::new(config.security.command_timeout_seconds);
    let opnsense_config = config_xml::read_config(Path::new("/conf/config.xml"))?;

    let sections = checks::collect_all(config, &opnsense_config, &runner);

    Ok(LocalSection::finalize(sections))
}
