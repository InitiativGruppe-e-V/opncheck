use anyhow::Result;

use crate::{
    config::Config,
    output::{LocalSection, LocalState},
    update::{self, UpdateOutcome},
};

pub fn section(config: &Config, update_result: Result<UpdateOutcome>) -> LocalSection {
    let mut section = LocalSection::new();
    let version = env!("CARGO_PKG_VERSION");

    match update_result {
        Ok(UpdateOutcome::Disabled) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    format!("Up to date ({version}), {}", next_check_summary(config)),
                )
                .with_metric("status", "disabled");
        }
        Ok(UpdateOutcome::NotDue | UpdateOutcome::UpToDate { .. }) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    format!("Up to date ({version}), {}", next_check_summary(config)),
                )
                .with_metric("status", "ok");
        }
        Ok(UpdateOutcome::UpdateAvailable { current: _, latest }) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    format!("Update available: {latest} (current: {version})"),
                )
                .with_metric("status", "update_available");
        }
        Ok(UpdateOutcome::Updated { from: _, to }) => {
            section
                .row(
                    LocalState::Ok,
                    "OPNCheck Version",
                    format!("Up to date ({to}), {}", next_check_summary(config)),
                )
                .with_metric("status", "updated");
        }
        Err(err) => {
            section
                .row(
                    LocalState::Crit,
                    "OPNCheck Version",
                    format!("Auto-update failed: {err:#}"),
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
