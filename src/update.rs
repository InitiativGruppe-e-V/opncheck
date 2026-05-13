use std::{
    cmp::Ordering,
    fs::{self, File},
    io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::config::Config;

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

    let latest = release.tag_name.trim_start_matches('v').to_owned();
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

fn download_and_replace(client: &Client, url: &str, destination: &Path) -> Result<()> {
    let destination_dir = destination
        .parent()
        .ok_or_else(|| anyhow!("install destination has no parent directory"))?;
    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "failed to create install directory {}",
            destination_dir.display()
        )
    })?;

    let temp_path = temp_update_path(destination);
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download request failed for {url}"))?;

    let mut temp_file = File::create(&temp_path)
        .with_context(|| format!("failed to create {}", temp_path.display()))?;
    let bytes = io::copy(&mut response, &mut temp_file)
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    drop(temp_file);
    if bytes == 0 {
        let _ = fs::remove_file(&temp_path);
        bail!("downloaded update asset was empty");
    }

    fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to set executable mode on {}", temp_path.display()))?;

    match fs::rename(&temp_path, destination) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(&temp_path);
            Err(err).with_context(|| {
                format!(
                    "failed to replace {} with {}",
                    destination.display(),
                    temp_path.display()
                )
            })
        }
    }
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

fn temp_update_path(destination: &Path) -> PathBuf {
    let pid = std::process::id();
    destination.with_file_name(format!(".opncheck.{pid}.new"))
}

fn release_asset_name(version: &str) -> String {
    format!("opncheck-{version}-{TARGET}")
}

fn compare_versions(left: &str, right: &str) -> Result<Ordering> {
    let left = parse_version(left)?;
    let right = parse_version(right)?;
    let len = left.len().max(right.len());

    for idx in 0..len {
        let left_part = left.get(idx).copied().unwrap_or(0);
        let right_part = right.get(idx).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return Ok(ordering),
        }
    }

    Ok(Ordering::Equal)
}

fn parse_version(version: &str) -> Result<Vec<u64>> {
    let version = version.trim_start_matches('v');
    if version.is_empty() {
        bail!("empty version");
    }

    version
        .split('.')
        .map(|part| {
            part.parse::<u64>()
                .with_context(|| format!("invalid numeric version segment {part:?} in {version:?}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_dotted_versions() {
        assert_eq!(compare_versions("0.2.0", "0.2.1").unwrap(), Ordering::Less);
        assert_eq!(compare_versions("v0.2.0", "0.2").unwrap(), Ordering::Equal);
        assert_eq!(
            compare_versions("0.10.0", "0.9.9").unwrap(),
            Ordering::Greater
        );
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
}
