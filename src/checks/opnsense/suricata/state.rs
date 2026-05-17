use std::{fs, os::unix::fs::MetadataExt, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct LogIdentity {
    pub device: u64,
    pub inode: u64,
    pub len: u64,
}

impl LogIdentity {
    pub fn from_path(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path)?;
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            len: metadata.len(),
        })
    }

    pub fn to_state(&self) -> SuricataState {
        SuricataState {
            device: self.device,
            inode: self.inode,
            offset: self.len,
            last_timestamp: None,
            last_flow_id: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuricataState {
    pub device: u64,
    pub inode: u64,
    pub offset: u64,
    #[serde(default)]
    pub last_timestamp: Option<String>,
    #[serde(default)]
    pub last_flow_id: Option<u64>,
}

impl SuricataState {
    pub fn matches(&self, identity: &LogIdentity) -> bool {
        self.device == identity.device && self.inode == identity.inode
    }
}

pub fn read_state(path: &Path) -> Result<Option<SuricataState>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))
        .map(Some)
}

pub fn write_state(path: &Path, state: SuricataState) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state directory {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(&state).context("failed to serialize state")?;
    fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))
}
