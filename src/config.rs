use std::{collections::BTreeSet, fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub paths: Paths,
    pub checks: Checks,
    pub security: Security,
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
    pub require_safe_paths: bool,
    pub max_spool_file_bytes: u64,
    pub plugin_timeout_seconds: u64,
    pub command_timeout_seconds: u64,
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
            require_safe_paths: true,
            max_spool_file_bytes: 1024 * 1024,
            plugin_timeout_seconds: 60,
            command_timeout_seconds: 30,
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("failed to parse config {}", path.display()))
    }

    pub fn check_enabled(&self, name: &str) -> bool {
        !self.checks.skip.contains(name)
    }
}
