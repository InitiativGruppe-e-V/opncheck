use serde::{Deserialize, Deserializer};

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

    pub fn get(self) -> bool {
        self.0
    }
}

pub fn deserialize_enabled_flag<'de, D>(deserializer: D) -> Result<Option<EnabledFlag>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.and_then(|value| EnabledFlag::parse(&value)))
}

/// Legacy OPNsense core convention: `<foo><enable>1</enable></foo>`.
/// Used by `<unbound>`, `<ipsec>`, per-interface entries, etc.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LegacyEnable {
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enable: Option<EnabledFlag>,
}

impl LegacyEnable {
    pub fn is_enabled_or_present(&self) -> bool {
        self.enable.map(EnabledFlag::get).unwrap_or(true)
    }

    pub fn is_explicitly_disabled(&self) -> bool {
        self.enable.map(|flag| !flag.get()).unwrap_or(false)
    }
}

/// MVC plugin convention: `<Foo><general><enabled>1</enabled></general></Foo>`.
/// Used by `<OPNsense><HAProxy>`, `<OPNsense><Nginx>`, `<OPNsense><wireguard>`, etc.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MvcGeneral {
    #[serde(default, deserialize_with = "deserialize_enabled_flag")]
    pub enabled: Option<EnabledFlag>,
}

impl MvcGeneral {
    pub fn is_enabled_or_present(&self) -> bool {
        self.enabled.map(EnabledFlag::get).unwrap_or(true)
    }

    pub fn is_explicitly_disabled(&self) -> bool {
        self.enabled.map(|flag| !flag.get()).unwrap_or(false)
    }
}
