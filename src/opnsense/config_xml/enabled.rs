use serde::{Deserialize, Deserializer};

pub fn deserialize_optional_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.as_deref().and_then(parse_bool))
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "yes" | "true" | "enabled" => Some(true),
        "0" | "no" | "false" | "disabled" => Some(false),
        _ => None,
    }
}
