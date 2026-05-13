use std::{borrow::Cow, fmt::Display};

use serde::{Deserialize, Deserializer, de::Error};

pub struct Percentage(pub f64);

impl<'de> Deserialize<'de> for Percentage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string: Cow<str> = Cow::deserialize(deserializer)?;
        let string = string.trim();
        let pct = if let Some(numeric) = string.strip_suffix('%').map(|s| s.trim()) {
            let v: f64 = numeric.parse().map_err(Error::custom)?;
            v / 100.0
        } else {
            let v: f64 = string.parse().map_err(Error::custom)?;
            if v >= 1.0 { v / 100.0 } else { v }
        };
        Ok(Percentage(pct))
    }
}

impl Display for Percentage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.0 * 100.0)
    }
}
