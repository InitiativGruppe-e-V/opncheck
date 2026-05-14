use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Default)]
pub(super) struct EnabledFlag(Option<bool>);

impl EnabledFlag {
    pub(super) fn is_enabled(&self) -> bool {
        self.0 == Some(true)
    }

    pub(super) fn is_enabled_or_present(&self) -> bool {
        self.0.unwrap_or(true)
    }

    pub(super) fn is_explicitly_disabled(&self) -> bool {
        self.0 == Some(false)
    }
}

impl<'de> Deserialize<'de> for EnabledFlag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        Ok(Self(value.as_deref().and_then(parse_bool)))
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "yes" | "true" | "enabled" => Some(true),
        "0" | "no" | "false" | "disabled" => Some(false),
        _ => None,
    }
}
