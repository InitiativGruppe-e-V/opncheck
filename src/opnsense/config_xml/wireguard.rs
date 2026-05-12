use serde::Deserialize;

use super::enabled::{EnabledFlag, MvcGeneral, deserialize_enabled_flag};

/// `<OPNsense><wireguard>…</wireguard></OPNsense>` per opnsense/core
/// `src/opnsense/mvc/app/models/OPNsense/Wireguard/{General,Client}.xml`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WireguardSection {
    #[serde(default)]
    pub general: MvcGeneral,
    #[serde(default)]
    pub client: WireguardClientHolder,
}

impl WireguardSection {
    pub fn is_enabled_or_present(&self) -> bool {
        self.general.is_enabled_or_present()
    }

    pub fn find_peer_name(&self, pubkey: &str) -> Option<&str> {
        self.client
            .clients
            .peers
            .iter()
            .filter(|peer| peer.enabled.is_some_and(EnabledFlag::get))
            .find(|peer| peer.pubkey.as_deref() == Some(pubkey))
            .and_then(|peer| peer.name.as_deref())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WireguardClientHolder {
    #[serde(default)]
    pub clients: WireguardPeers,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WireguardPeers {
    // The XML repeats `<client>` inside `<clients>`. The previous model named
    // this field `clients`, which silently produced an empty Vec.
    #[serde(default, rename = "client")]
    pub peers: Vec<WireguardPeer>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WireguardPeer {
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enabled: Option<EnabledFlag>,
    pub name: Option<String>,
    pub pubkey: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::super::OpnsenseConfig;

    const FIXTURE: &str = r#"<?xml version="1.0"?>
<opnsense>
  <OPNsense>
    <wireguard>
      <general>
        <enabled>1</enabled>
      </general>
      <client>
        <clients>
          <client uuid="aaaa-1"><enabled>1</enabled><name>laptop</name><pubkey>ABC=</pubkey></client>
          <client uuid="aaaa-2"><enabled>0</enabled><name>disabled-peer</name><pubkey>XYZ=</pubkey></client>
          <client uuid="aaaa-3"><enabled>1</enabled><name>phone</name><pubkey>DEF=</pubkey></client>
        </clients>
      </client>
    </wireguard>
  </OPNsense>
</opnsense>"#;

    #[test]
    fn wireguard_peer_lookup_resolves_pubkey_to_name() {
        let cfg: OpnsenseConfig = quick_xml::de::from_str(FIXTURE).expect("parse fixture");
        assert!(cfg.wireguard_enabled());
        assert_eq!(cfg.wireguard_peer_name("ABC="), Some("laptop"));
        assert_eq!(cfg.wireguard_peer_name("DEF="), Some("phone"));
        // Disabled peers are skipped, just like the legacy implementation.
        assert_eq!(cfg.wireguard_peer_name("XYZ="), None);
        assert_eq!(cfg.wireguard_peer_name("not-a-key"), None);
    }
}
