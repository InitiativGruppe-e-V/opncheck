use serde::Deserialize;

use super::enabled::{EnabledFlag, deserialize_enabled_flag};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Gateways {
    #[serde(default)]
    pub gateway_item: Vec<GatewayItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GatewayItem {
    pub name: Option<String>,
    pub interface: Option<String>,
    pub gateway: Option<String>,
    pub monitor: Option<String>,
    pub descr: Option<String>,
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub disabled: Option<EnabledFlag>,
}
