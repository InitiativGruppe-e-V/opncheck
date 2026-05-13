use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;

mod dhcpd;
mod enabled;
mod gateways;
mod haproxy;
mod interfaces;
mod ipsec;
mod nginx;
mod opnsense_section;
mod system;
mod unbound;
mod wireguard;

pub use dhcpd::DhcpdSection;
pub use enabled::{EnabledFlag, LegacyEnable, MvcGeneral};
pub use gateways::{GatewayItem, Gateways};
pub use haproxy::HaproxySection;
pub use interfaces::Interface;
pub use ipsec::IpsecSection;
pub use nginx::NginxSection;
pub use opnsense_section::OPNsenseSection;
pub use system::System;
pub use unbound::{UnboundPlusSection, UnboundSection};
pub use wireguard::{WireguardClientHolder, WireguardPeer, WireguardPeers, WireguardSection};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpnsenseConfig {
    #[serde(default)]
    pub system: System,
    #[serde(default)]
    pub interfaces: BTreeMap<String, Interface>,
    pub dhcpd: Option<DhcpdSection>,
    pub ipsec: Option<IpsecSection>,
    pub unbound: Option<UnboundSection>,
    #[serde(rename = "OPNsense")]
    pub opnsense: Option<OPNsenseSection>,
}

impl OpnsenseConfig {
    pub fn has_dhcp(&self) -> bool {
        self.dhcpd.is_some()
    }

    pub fn has_gateways(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.gateways.as_ref())
            .is_some_and(Gateways::has_items)
    }

    pub fn unbound_enabled(&self) -> bool {
        !self
            .unbound
            .as_ref()
            .is_some_and(UnboundSection::is_explicitly_disabled)
            && !self
                .opnsense
                .as_ref()
                .and_then(|opnsense| opnsense.unboundplus.as_ref())
                .and_then(|plus| plus.general.as_ref())
                .is_some_and(MvcGeneral::is_explicitly_disabled)
    }

    pub fn haproxy_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.haproxy.as_ref())
            .and_then(|haproxy| haproxy.general.as_ref())
            .is_some_and(MvcGeneral::is_enabled_or_present)
    }

    pub fn nginx_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.nginx.as_ref())
            .and_then(|nginx| nginx.general.as_ref())
            .is_some_and(MvcGeneral::is_enabled_or_present)
    }

    pub fn ipsec_enabled(&self) -> bool {
        self.ipsec
            .as_ref()
            .is_some_and(IpsecSection::is_enabled_or_present)
    }

    pub fn wireguard_enabled(&self) -> bool {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.wireguard.as_ref())
            .is_some_and(WireguardSection::is_enabled_or_present)
    }

    pub fn wireguard_peer_name(&self, pubkey: &str) -> Option<&str> {
        self.opnsense
            .as_ref()
            .and_then(|opnsense| opnsense.wireguard.as_ref())
            .and_then(|wg| wg.find_peer_name(pubkey))
    }
}

pub fn read_config(path: &Path) -> Option<OpnsenseConfig> {
    let raw = fs::read_to_string(path).ok()?;
    quick_xml::de::from_str::<OpnsenseConfig>(&raw).ok()
}
