use serde::Deserialize;

use super::gateways::Gateways;
use super::haproxy::HaproxySection;
use super::nginx::NginxSection;
use super::unbound::UnboundPlusSection;
use super::wireguard::WireguardSection;

/// Wrapper for MVC-style subsystems that live under `<OPNsense>` in `config.xml`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OPNsenseSection {
    #[serde(rename = "Gateways")]
    pub gateways: Option<Gateways>,
    #[serde(rename = "HAProxy")]
    pub haproxy: Option<HaproxySection>,
    #[serde(rename = "Nginx")]
    pub nginx: Option<NginxSection>,
    pub wireguard: Option<WireguardSection>,
    pub unboundplus: Option<UnboundPlusSection>,
}
