use serde::Deserialize;

use super::enabled::{EnabledFlag, deserialize_enabled_flag};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Gateways {
    #[serde(default)]
    pub items: GatewayItems,
}

impl Gateways {
    pub fn has_items(&self) -> bool {
        !self.items.gateway_item.is_empty()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GatewayItems {
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

#[cfg(test)]
mod tests {
    use super::super::OpnsenseConfig;

    #[test]
    fn parses_mvc_gateways_items_wrapper() {
        let cfg: OpnsenseConfig = quick_xml::de::from_str(
            r#"<opnsense>
  <OPNsense>
    <Gateways version="1.0.0">
      <items>
        <gateway_item uuid="11111111-1111-1111-1111-111111111111">
          <disabled>0</disabled>
          <interface>wan</interface>
          <gateway>198.51.100.1</gateway>
          <name>WAN_DHCP</name>
        </gateway_item>
      </items>
    </Gateways>
  </OPNsense>
</opnsense>"#,
        )
        .expect("parse mvc gateway fixture");

        assert!(cfg.has_gateways());
        assert_eq!(
            cfg.opnsense
                .as_ref()
                .and_then(|opnsense| opnsense.gateways.as_ref())
                .and_then(|gateways| gateways.items.gateway_item.first())
                .and_then(|gateway| gateway.name.as_deref()),
            Some("WAN_DHCP")
        );
    }
}
