use std::path::Path;

use anyhow::{Context, Result};
use jiff::{Timestamp, tz::TimeZone};
use self_update::{Status, backends::github::Update};

use crate::config::Config;

const REPO_OWNER: &str = "initiativgruppe-e-v";
const REPO_NAME: &str = "opncheck";
const BIN_NAME: &str = "opncheck";
const INSTALL_PATH: &str = "/usr/local/bin/opncheck";
const TARGET: &str = "x86_64-unknown-freebsd";

#[derive(Debug, Eq, PartialEq)]
pub enum UpdateOutcome {
    Disabled,
    NotDue,
    UpToDate,
    Updated { from: String, to: String },
}

impl UpdateOutcome {
    pub fn summary(&self) -> String {
        match self {
            Self::Disabled => "Auto-updates are disabled".to_owned(),
            Self::NotDue => "Update check is not due yet".to_owned(),
            Self::UpToDate => "opncheck is already up to date".to_owned(),
            Self::Updated { from, to } => format!("Updated opncheck from {from} to {to}"),
        }
    }
}

pub fn check_and_update(config_path: &Path, config: &mut Config) -> Result<UpdateOutcome> {
    if !config.updates.enabled {
        return Ok(UpdateOutcome::Disabled);
    }

    if !config.updates.is_due() {
        return Ok(UpdateOutcome::NotDue);
    }

    attempt_update(config_path, config)
}

pub fn update_now(config_path: &Path, config: &mut Config) -> Result<UpdateOutcome> {
    attempt_update(config_path, config)
}

fn attempt_update(config_path: &Path, config: &mut Config) -> Result<UpdateOutcome> {
    let update_result = perform_update();
    let state_result = write_last_checked(config_path, config);

    match (update_result, state_result) {
        (Ok(outcome), Ok(())) => Ok(outcome),
        (Ok(_), Err(err)) => Err(err.context("failed to write update check timestamp")),
        (Err(err), Ok(())) => Err(err),
        (Err(update_err), Err(state_err)) => Err(update_err.context(format!(
            "also failed to write update check timestamp: {state_err}"
        ))),
    }
}

pub fn next_check_summary(config: &Config) -> Option<String> {
    Some(
        config
            .updates
            .next_check()?
            .to_zoned(TimeZone::system())
            .strftime("%d.%m.%Y %H:%M:%S")
            .to_string(),
    )
}

fn perform_update() -> Result<UpdateOutcome> {
    let current = env!("CARGO_PKG_VERSION");
    let status = Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .bin_install_path(INSTALL_PATH)
        .target(TARGET)
        .identifier(BIN_NAME)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .current_version(current)
        .build()
        .context("failed to configure self-update")?
        .update()
        .context("failed to update opncheck from GitHub release")?;

    match status {
        Status::UpToDate(_) => Ok(UpdateOutcome::UpToDate),
        Status::Updated(version) => Ok(UpdateOutcome::Updated {
            from: current.to_owned(),
            to: version,
        }),
    }
}

fn write_last_checked(config_path: &Path, config: &mut Config) -> Result<()> {
    config.updates.last_checked = Some(Timestamp::now());
    config.save(config_path)
}
