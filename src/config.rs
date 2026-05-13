use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{Context, Result};
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub paths: Paths,
    pub checks: Checks,
    pub security: Security,
    pub updates: Updates,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Paths {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Checks {
    pub skip: BTreeSet<String>,
    pub services_ignored: BTreeSet<String>,
    pub inventory_interval_seconds: u64,
    pub wireguard: Wireguard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Wireguard {
    pub stale_warn_seconds: u64,
    pub stale_crit_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Security {
    pub plugin_timeout_seconds: u64,
    pub command_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Updates {
    pub enabled: bool,
    #[serde(serialize_with = "jiff::fmt::serde::duration::friendly::compact::required")]
    pub interval: SignedDuration,
    pub last_checked: Option<Timestamp>,
}

impl Default for Checks {
    fn default() -> Self {
        Self {
            skip: BTreeSet::new(),
            services_ignored: BTreeSet::from(["iperf".to_owned()]),
            inventory_interval_seconds: 14_400,
            wireguard: Wireguard::default(),
        }
    }
}

impl Default for Wireguard {
    fn default() -> Self {
        Self {
            stale_warn_seconds: 300,
            stale_crit_seconds: 900,
        }
    }
}

impl Default for Security {
    fn default() -> Self {
        Self {
            plugin_timeout_seconds: 60,
            command_timeout_seconds: 30,
        }
    }
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: SignedDuration::from_hours(6),
            last_checked: None,
        }
    }
}

impl Updates {
    pub fn is_due(&self) -> bool {
        self.next_check()
            .is_some_and(|next_check| Timestamp::now() >= next_check)
    }

    pub fn next_check(&self) -> Option<Timestamp> {
        if !self.enabled {
            return None;
        }

        Some(
            self.last_checked
                .map(|last_checked| last_checked + self.interval)
                .unwrap_or_else(Timestamp::now),
        )
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("failed to parse config {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }

        let raw = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(path, raw).with_context(|| format!("failed to write config {}", path.display()))
    }

    pub fn check_enabled(&self, name: &str) -> bool {
        !self.checks.skip.contains(name)
    }
}
