use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::platform::{CurrentPlatform, Platform};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub checks: Checks,
    pub scripts: Scripts,
    pub security: Security,
    pub updates: Updates,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Checks {
    pub skip: BTreeSet<String>,
    pub nginx: Nginx,
    pub services: Services,
    pub wireguard: Wireguard,
    pub suricata: Suricata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Services {
    pub ignored: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Nginx {
    pub status_socket: PathBuf,
    pub status_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Wireguard {
    pub stale_warn_seconds: u64,
    pub stale_crit_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Suricata {
    pub log_path: PathBuf,
    pub state_path: PathBuf,
    pub max_summary_events: usize,
    pub include_allowed_in_summary: bool,
    pub initialize_from_end: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scripts {
    pub enabled: BTreeSet<String>,
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

impl Default for Services {
    fn default() -> Self {
        Self {
            ignored: BTreeSet::from(["iperf".to_owned()]),
        }
    }
}

impl Default for Nginx {
    fn default() -> Self {
        Self {
            status_socket: PathBuf::from("/var/run/nginx_status.sock"),
            status_urls: vec![
                "http://127.0.0.1/nginx_status".to_owned(),
                "http://127.0.0.1/status".to_owned(),
                "http://127.0.0.1/vts".to_owned(),
                "http://localhost/nginx_status".to_owned(),
                "http://localhost/status".to_owned(),
                "http://localhost/vts".to_owned(),
            ],
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

impl Default for Suricata {
    fn default() -> Self {
        Self {
            log_path: PathBuf::from("/var/log/suricata/eve.json"),
            state_path: CurrentPlatform::state_dir().join("suricata-state.json"),
            max_summary_events: 5,
            include_allowed_in_summary: true,
            initialize_from_end: true,
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
                .map_or_else(Timestamp::now, |last_checked| last_checked + self.interval),
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
