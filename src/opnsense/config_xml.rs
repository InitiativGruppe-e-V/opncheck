use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpnsenseConfig {
    #[serde(default)]
    pub system: System,
    #[serde(default)]
    pub interfaces: BTreeMap<String, Interface>,
    pub dhcpd: Option<EnableSection>,
    pub gateways: Option<Gateways>,
    pub ipsec: Option<EnableSection>,
    pub unbound: Option<EnableSection>,
    #[serde(rename = "OPNsense")]
    pub opnsense: Option<OPNsenseSection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OPNsenseSection {
    #[serde(rename = "Gateways")]
    pub gateways: Option<Gateways>,
    #[serde(rename = "HAProxy")]
    pub haproxy: Option<PluginSection>,
    #[serde(rename = "Nginx")]
    pub nginx: Option<PluginSection>,
    #[serde(rename = "OpenVPN")]
    pub openvpn: Option<EnableSection>,
    #[serde(rename = "wireguard")]
    pub wireguard: Option<WireguardConfig>,
    pub unboundplus: Option<PluginSection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WireguardConfig {
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enabled: Option<EnabledFlag>,
    #[serde(default)]
    pub clients: WireguardClients,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WireguardClients {
    #[serde(default)]
    pub client: Vec<WireguardClient>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WireguardClient {
    pub name: Option<String>,
    pub pubkey: Option<String>,
}

impl WireguardConfig {
    pub fn is_enabled_or_present(&self) -> bool {
        self.enabled.map(EnabledFlag::get).unwrap_or(true)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginSection {
    pub general: Option<EnableSection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Gateways {
    #[serde(default)]
    pub gateway_item: Vec<GatewayItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EnableSection {
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enabled: Option<EnabledFlag>,
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enable: Option<EnabledFlag>,
    #[serde(rename = "Enabled")]
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enabled_upper: Option<EnabledFlag>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnabledFlag(bool);

impl EnabledFlag {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "yes" | "true" | "enabled" => Some(Self(true)),
            "0" | "no" | "false" | "disabled" => Some(Self(false)),
            _ => None,
        }
    }

    fn get(self) -> bool {
        self.0
    }
}

impl EnableSection {
    pub fn is_enabled_or_present(&self) -> bool {
        self.enabled_value().unwrap_or(true)
    }

    pub fn is_explicitly_disabled(&self) -> bool {
        self.enabled_value()
            .map(|enabled| !enabled)
            .unwrap_or(false)
    }

    fn enabled_value(&self) -> Option<bool> {
        self.enabled
            .or(self.enable)
            .or(self.enabled_upper)
            .map(EnabledFlag::get)
    }
}

impl OpnsenseConfig {
    pub fn has_dhcp(&self) -> bool {
        self.dhcpd.is_some()
    }

    pub fn has_gateways(&self) -> bool {
        self.gateways.is_some()
            || self
                .opnsense
                .as_ref()
                .and_then(|opnsense| opnsense.gateways.as_ref())
                .is_some()
    }

    pub fn unbound_enabled(&self) -> bool {
        !self
            .unbound
            .as_ref()
            .is_some_and(EnableSection::is_explicitly_disabled)
            && !self
                .opnsense
                .as_ref()
                .and_then(|opnsense| opnsense.unboundplus.as_ref())
                .and_then(|unboundplus| unboundplus.general.as_ref())
                .is_some_and(EnableSection::is_explicitly_disabled)
    }

    pub fn haproxy_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.haproxy.as_ref())
            .and_then(|haproxy| haproxy.general.as_ref())
            .is_some_and(EnableSection::is_enabled_or_present)
    }

    pub fn nginx_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.nginx.as_ref())
            .and_then(|nginx| nginx.general.as_ref())
            .is_some_and(EnableSection::is_enabled_or_present)
    }

    pub fn ipsec_enabled(&self) -> bool {
        self.ipsec
            .as_ref()
            .is_some_and(EnableSection::is_enabled_or_present)
    }

    pub fn wireguard_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.wireguard.as_ref())
            .is_some_and(WireguardConfig::is_enabled_or_present)
    }

    pub fn wireguard_peer_name(&self, pubkey: &str) -> Option<&str> {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.wireguard.as_ref())
            .and_then(|wg| {
                wg.clients
                    .client
                    .iter()
                    .find(|c| c.pubkey.as_deref() == Some(pubkey))
                    .and_then(|c| c.name.as_deref())
            })
    }
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
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enable: Option<EnabledFlag>,
}

pub fn read_config(path: &Path) -> Option<OpnsenseConfig> {
    let raw = fs::read_to_string(path).ok()?;
    quick_xml::de::from_str::<OpnsenseConfig>(&raw).ok()
}

fn deserialize_enabled_flag<'de, D>(deserializer: D) -> Result<Option<EnabledFlag>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.and_then(|value| EnabledFlag::parse(&value)))
}
