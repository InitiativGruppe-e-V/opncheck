pub mod config_xml;

use std::{fs, path::Path};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreVersion {
    pub product_name: Option<String>,
    pub product_version: Option<String>,
    pub product_series: Option<String>,
}

pub fn read_core_version(path: &Path) -> CoreVersion {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}
