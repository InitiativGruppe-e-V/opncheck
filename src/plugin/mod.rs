pub mod output;

use anyhow::Result;
use std::path::Path;

use crate::{
    checks,
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml,
    plugin::output::{LocalSection, LocalState},
    update::{self, UpdateOutcome},
};

pub fn plugin_output(config_path: &Path, config: &mut Config) -> Result<String> {
    let runner = CommandRunner::new(config.security.command_timeout_seconds);
    let update_result = update::check_and_update(config_path, config);
    let opnsense_config = config_xml::read_config(Path::new("/conf/config.xml"))?;

    let mut sections = checks::collect_all(config, &opnsense_config, &runner);
    sections.push(version_section(config, update_result));

    Ok(LocalSection::finalize(sections))
}

fn version_section(config: &Config, update_result: Result<UpdateOutcome>) -> LocalSection {
    let mut section = LocalSection::new();
    let version = env!("CARGO_PKG_VERSION");

    match update_result {
        Ok(UpdateOutcome::Disabled) => {
            section.add(
                LocalState::Ok,
                "OPNCheck Version",
                "status=disabled",
                &format!("Up to date ({version}), {}", next_check_summary(config)),
            );
        }
        Ok(UpdateOutcome::NotDue | UpdateOutcome::UpToDate { .. }) => {
            section.add(
                LocalState::Ok,
                "OPNCheck Version",
                "status=ok",
                &format!("Up to date ({version}), {}", next_check_summary(config)),
            );
        }
        Ok(UpdateOutcome::UpdateAvailable { current: _, latest }) => {
            section.add(
                LocalState::Ok,
                "OPNCheck Version",
                "status=update_available",
                &format!("Update available: {latest} (current: {version})"),
            );
        }
        Ok(UpdateOutcome::Updated { from: _, to }) => {
            section.add(
                LocalState::Ok,
                "OPNCheck Version",
                "status=updated",
                &format!("Up to date ({to}), {}", next_check_summary(config)),
            );
        }
        Err(err) => {
            section.add(
                LocalState::Crit,
                "OPNCheck Version",
                "status=err",
                &format!("Auto-update failed: {err:#}"),
            );
        }
    }

    section
}

fn next_check_summary(config: &Config) -> String {
    match update::next_check_summary(config) {
        Some(next_check) => format!("Next check {next_check}"),
        None => "Next check disabled".to_owned(),
    }
}
