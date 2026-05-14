use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

use self::enabled::EnabledFlag;

mod enabled;
mod wireguard;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpnsenseConfig {
    unbound: Option<LegacyServiceSection>,
    #[serde(rename = "OPNsense")]
    opnsense: Option<OPNsenseSection>,
}

impl OpnsenseConfig {
    pub fn unbound_enabled(&self) -> bool {
        !self
            .unbound
            .as_ref()
            .is_some_and(LegacyServiceSection::is_explicitly_disabled)
            && !self
                .opnsense
                .as_ref()
                .and_then(|opnsense| opnsense.unboundplus.as_ref())
                .is_some_and(MvcServiceSection::is_explicitly_disabled)
    }

    pub fn nginx_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.nginx.as_ref())
            .is_some_and(MvcServiceSection::is_enabled_or_present)
    }

    pub fn wireguard_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.wireguard.as_ref())
            .is_some_and(wireguard::WireguardSection::is_enabled_or_present)
    }

    pub fn wireguard_peer_name(&self, pubkey: &str) -> Option<&str> {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.wireguard.as_ref())
            .and_then(|wg| wg.find_peer_name(pubkey))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OPNsenseSection {
    #[serde(rename = "Nginx")]
    nginx: Option<MvcServiceSection>,
    wireguard: Option<wireguard::WireguardSection>,
    unboundplus: Option<MvcServiceSection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LegacyServiceSection {
    #[serde(default)]
    enable: EnabledFlag,
}

impl LegacyServiceSection {
    fn is_explicitly_disabled(&self) -> bool {
        self.enable.is_explicitly_disabled()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MvcServiceSection {
    general: Option<MvcGeneral>,
}

impl MvcServiceSection {
    fn is_enabled_or_present(&self) -> bool {
        self.general
            .as_ref()
            .is_some_and(MvcGeneral::is_enabled_or_present)
    }

    fn is_explicitly_disabled(&self) -> bool {
        self.general
            .as_ref()
            .is_some_and(MvcGeneral::is_explicitly_disabled)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct MvcGeneral {
    #[serde(default)]
    enabled: EnabledFlag,
}

impl MvcGeneral {
    fn is_enabled_or_present(&self) -> bool {
        self.enabled.is_enabled_or_present()
    }

    fn is_explicitly_disabled(&self) -> bool {
        self.enabled.is_explicitly_disabled()
    }
}

pub fn read_config(path: &Path) -> Result<OpnsenseConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read OPNsense config {}", path.display()))?;
    quick_xml::de::from_str::<OpnsenseConfig>(&raw)
        .with_context(|| format!("failed to parse OPNsense config {}", path.display()))
}
