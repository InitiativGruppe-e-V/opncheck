use serde::Deserialize;

use super::{MvcGeneral, enabled::EnabledFlag};

/// `<OPNsense><wireguard>…</wireguard></OPNsense>` per opnsense/core
/// `src/opnsense/mvc/app/models/OPNsense/Wireguard/{General,Client}.xml`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct WireguardSection {
    #[serde(default)]
    general: MvcGeneral,
    #[serde(default)]
    client: WireguardClientHolder,
}

impl WireguardSection {
    pub(super) fn is_enabled_or_present(&self) -> bool {
        self.general.is_enabled_or_present()
    }

    pub(super) fn find_peer_name(&self, pubkey: &str) -> Option<&str> {
        self.client
            .clients
            .peers
            .iter()
            .filter(|peer| peer.enabled.is_enabled())
            .find(|peer| peer.pubkey.as_deref() == Some(pubkey))
            .and_then(|peer| peer.name.as_deref())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WireguardClientHolder {
    #[serde(default)]
    clients: WireguardPeers,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WireguardPeers {
    // The XML repeats `<client>` inside `<clients>`. The previous model named
    // this field `clients`, which silently produced an empty Vec.
    #[serde(default, rename = "client")]
    peers: Vec<WireguardPeer>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WireguardPeer {
    #[serde(default)]
    enabled: EnabledFlag,
    name: Option<String>,
    pubkey: Option<String>,
}
