use std::path::Path;

use anyhow::{Context, Result};
use console::{Emoji, style};
use jiff::{Timestamp, tz::TimeZone};
use self_update::{Status, backends::github::Update, update::ReleaseUpdate};

use crate::{
    config::Config,
    platform::{CurrentPlatform, Platform},
};

const REPO_OWNER: &str = "initiativgruppe-e-v";
const REPO_NAME: &str = "opncheck";
const BIN_NAME: &str = "opncheck";

static CHECKMARK: Emoji<'_, '_> = Emoji("✔", "OK");
static SPARKLES: Emoji<'_, '_> = Emoji("✨", "NEW");

#[derive(Debug, Eq, PartialEq)]
pub enum UpdateOutcome {
    Disabled,
    NotDue,
    UpToDate { version: String },
    UpdateAvailable { current: String, latest: String },
    Updated { from: String, to: String },
}

impl UpdateOutcome {
    pub fn summary(&self) -> String {
        match self {
            Self::Disabled => style("Auto-updates are disabled").dim().to_string(),
            Self::NotDue => style("Update check is not due yet").dim().to_string(),
            Self::UpToDate { version } => format!(
                "{} {} (current: {})",
                CHECKMARK,
                style("opncheck is up to date").green(),
                style(version).dim()
            ),
            Self::UpdateAvailable { current, latest } => format!(
                "{} {} ({} -> {})",
                SPARKLES,
                style("A new version of opncheck is available!")
                    .bold()
                    .yellow(),
                style(current).dim(),
                style(latest).bold().green()
            ),
            Self::Updated { from, to } => format!(
                "{} {}",
                SPARKLES,
                style(format!("Updated opncheck from {from} to {to}"))
                    .bold()
                    .green()
            ),
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

fn update_builder() -> Result<Box<dyn ReleaseUpdate>> {
    Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .bin_install_path(CurrentPlatform::install_path())
        .target(CurrentPlatform::release_target())
        .identifier(BIN_NAME)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()
        .context("failed to configure self-update")
}

pub fn check_for_update() -> Result<UpdateOutcome> {
    let current = env!("CARGO_PKG_VERSION");
    let releases = update_builder()?
        .get_latest_release()
        .context("failed to fetch latest release from GitHub")?;

    if releases.version == current {
        Ok(UpdateOutcome::UpToDate {
            version: current.to_owned(),
        })
    } else {
        Ok(UpdateOutcome::UpdateAvailable {
            current: current.to_owned(),
            latest: releases.version,
        })
    }
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
    let status = update_builder()?
        .update()
        .context("failed to update opncheck from GitHub release")?;

    match status {
        Status::UpToDate(_) => Ok(UpdateOutcome::UpToDate {
            version: current.to_owned(),
        }),
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
