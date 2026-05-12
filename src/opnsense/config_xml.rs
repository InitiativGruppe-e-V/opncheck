use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpnsenseConfig {
    #[serde(default)]
    pub system: System,
    #[serde(default)]
    pub interfaces: BTreeMap<String, Interface>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct System {
    pub hostname: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Interface {
    #[serde(rename = "if")]
    pub device: Option<String>,
    pub descr: Option<String>,
    pub enable: Option<String>,
}

pub fn read_config(path: &Path) -> Option<OpnsenseConfig> {
    let raw = fs::read_to_string(path).ok()?;
    quick_xml::de::from_str::<OpnsenseConfig>(&raw).ok()
}
