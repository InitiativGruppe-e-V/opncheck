pub mod output;

use anyhow::Result;
use std::path::Path;

use crate::{
    checks,
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml,
    plugin::output::{LocalSection, LocalState, collect_sections},
    update::{self, UpdateOutcome},
};

pub fn plugin_output(config_path: &Path, config: &mut Config) -> Result<String> {
    let runner = CommandRunner::new(config.security.command_timeout_seconds);
    let update_result = update::check_and_update(config_path, config);
    let opnsense_config = config_xml::read_config(Path::new("/conf/config.xml"))?;

    let mut sections = checks::collect_all(config, &opnsense_config, &runner);
    sections.push(version_section(config, update_result));

    Ok(collect_sections(sections))
}

fn version_section(config: &Config, update_result: Result<UpdateOutcome>) -> LocalSection {
    let mut section = LocalSection::new();
    let version = env!("CARGO_PKG_VERSION");

    match update_result {
        Ok(UpdateOutcome::Disabled) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    &format!("Up to date ({version}), {}", next_check_summary(config)),
                )
                .with_metric("status", "disabled");
        }
        Ok(UpdateOutcome::NotDue | UpdateOutcome::UpToDate { .. }) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    &format!("Up to date ({version}), {}", next_check_summary(config)),
                )
                .with_metric("status", "ok");
        }
        Ok(UpdateOutcome::UpdateAvailable { current: _, latest }) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    &format!("Update available: {latest} (current: {version})"),
                )
                .with_metric("status", "update_available");
        }
        Ok(UpdateOutcome::Updated { from: _, to }) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    &format!("Up to date ({to}), {}", next_check_summary(config)),
                )
                .with_metric("status", "updated");
        }
        Err(err) => {
            section
                .row(
                    LocalState::Crit,
                    "OPNCheck Version",
                    &format!("Auto-update failed: {err:#}"),
                )
                .with_metric("status", "err");
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
