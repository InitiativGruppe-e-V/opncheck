use serde::Deserialize;

use super::enabled::{EnabledFlag, deserialize_enabled_flag};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Interface {
    #[serde(rename = "if")]
    pub device: Option<String>,
    pub descr: Option<String>,
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enable: Option<EnabledFlag>,
}
