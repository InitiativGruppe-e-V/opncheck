use serde::Deserialize;

use super::enabled::{EnabledFlag, deserialize_enabled_flag};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IpsecSection {
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enable: Option<EnabledFlag>,
}

impl IpsecSection {
    pub fn is_enabled_or_present(&self) -> bool {
        self.enable.map(EnabledFlag::get).unwrap_or(true)
    }
}
