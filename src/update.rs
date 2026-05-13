use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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

pub fn check_and_update(config_path: &Path, config: &mut Config) -> Result<UpdateOutcome> {
    if !config.updates.enabled {
        return Ok(UpdateOutcome::Disabled);
    }

    if !is_check_due(config)? {
        return Ok(UpdateOutcome::NotDue);
    }

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
    let next_check_unix = next_check_unix(config)?;
    let timestamp = i64::try_from(next_check_unix).ok()?;
    let timestamp = Timestamp::from_second(timestamp).ok()?;
    Some(
        timestamp
            .to_zoned(TimeZone::UTC)
            .strftime("%Y-%m-%d %H:%M:%S UTC")
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

fn next_check_unix(config: &Config) -> Option<u64> {
    if !config.updates.enabled {
        return None;
    }

    if config.updates.interval_seconds == 0 {
        return now_unix().ok();
    }

    Some(
        config
            .updates
            .last_checked_unix
            .unwrap_or_else(|| now_unix().unwrap_or(0))
            .saturating_add(config.updates.interval_seconds),
    )
}

fn is_check_due(config: &Config) -> Result<bool> {
    if config.updates.interval_seconds == 0 {
        return Ok(true);
    }

    let Some(last_checked_unix) = config.updates.last_checked_unix else {
        return Ok(true);
    };

    let now = now_unix()?;
    Ok(now.saturating_sub(last_checked_unix) >= config.updates.interval_seconds)
}

fn write_last_checked(config_path: &Path, config: &mut Config) -> Result<()> {
    config.updates.last_checked_unix = Some(now_unix()?);

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(config_path, raw)
        .with_context(|| format!("failed to write config {}", config_path.display()))
}

fn now_unix() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs())
}
