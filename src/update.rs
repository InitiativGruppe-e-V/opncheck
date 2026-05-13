use std::{
    cmp::Ordering,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use jiff::{Timestamp, tz::TimeZone};
use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;

use crate::{config::Config, install};

const REPO: &str = "initiativgruppe-e-v/opncheck";
const INSTALL_PATH: &str = "/usr/local/bin/opncheck";
const TARGET: &str = "x86_64-unknown-freebsd";

#[derive(Debug, Eq, PartialEq)]
pub enum UpdateOutcome {
    Disabled,
    NotDue,
    UpToDate,
    Updated { from: String, to: String },
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
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
    let current = env!("CARGO_PKG_VERSION").to_owned();
    let client = Client::builder()
        .user_agent(concat!("opncheck/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to build HTTP client")?;

    let release: Release = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .send()
        .context("failed to fetch latest release metadata")?
        .error_for_status()
        .context("GitHub latest release request failed")?
        .json()
        .context("failed to parse latest release metadata")?;

    let latest = clean_version(&release.tag_name)?;
    if compare_versions(&current, &latest)? != Ordering::Less {
        return Ok(UpdateOutcome::UpToDate);
    }

    let asset_name = release_asset_name(&latest);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| anyhow!("latest release is missing asset {asset_name}"))?;

    download_and_replace(
        &client,
        &asset.browser_download_url,
        Path::new(INSTALL_PATH),
    )
    .with_context(|| format!("failed to install opncheck {latest}"))?;

    Ok(UpdateOutcome::Updated {
        from: current,
        to: latest,
    })
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

fn download_and_replace(client: &Client, url: &str, destination: &Path) -> Result<()> {
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download request failed for {url}"))?;

    install::replace_with_reader(
        destination,
        &mut response,
        "downloaded update asset was empty",
    )?;

    Ok(())
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

fn release_asset_name(version: &str) -> String {
    format!("opncheck-{version}-{TARGET}")
}

fn compare_versions(left: &str, right: &str) -> Result<Ordering> {
    let left = Version::parse(&clean_version(left)?)
        .with_context(|| format!("invalid current version {left:?}"))?;
    let right = Version::parse(&clean_version(right)?)
        .with_context(|| format!("invalid latest version {right:?}"))?;

    Ok(left.cmp(&right))
}

fn clean_version(version: &str) -> Result<String> {
    let version = version.trim_start_matches('v').to_owned();
    if version.is_empty() {
        anyhow::bail!("empty version");
    }

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_dotted_versions() {
        assert_eq!(compare_versions("0.2.0", "0.2.1").unwrap(), Ordering::Less);
        assert_eq!(
            compare_versions("v0.2.0", "0.2.0").unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            compare_versions("0.10.0", "0.9.9").unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            compare_versions("0.2.0-alpha.1", "0.2.0").unwrap(),
            Ordering::Less
        );
    }

    #[test]
    fn rejects_invalid_versions() {
        assert!(compare_versions("0.2.0", "0.2").is_err());
        assert!(compare_versions("0.2.0", "not-a-version").is_err());
    }

    #[test]
    fn builds_raw_release_asset_name() {
        assert_eq!(
            release_asset_name("0.3.0"),
            "opncheck-0.3.0-x86_64-unknown-freebsd"
        );
    }

    #[test]
    fn disabled_updates_do_not_write_config() {
        let mut config = Config::default();

        assert_eq!(
            check_and_update(Path::new("/no/such/config.toml"), &mut config).unwrap(),
            UpdateOutcome::Disabled
        );
    }

    #[test]
    fn formats_next_update_check_in_utc() {
        let mut config = Config::default();
        config.updates.enabled = true;
        config.updates.interval_seconds = 21_600;
        config.updates.last_checked_unix = Some(0);

        assert_eq!(
            next_check_summary(&config).unwrap(),
            "1970-01-01 06:00:00 UTC"
        );
    }

    #[test]
    fn disabled_updates_have_no_next_check() {
        let config = Config::default();

        assert_eq!(next_check_summary(&config), None);
    }
}
