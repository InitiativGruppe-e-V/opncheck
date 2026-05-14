use std::{borrow::Cow, fmt::Display, ops::Deref};

use serde::{Deserialize, Deserializer, de::Error};

pub struct Percentage(f64);

impl Percentage {
    pub const HUNDRED: Percentage = Percentage(1.0);
    pub const ZERO: Percentage = Percentage(0.0);

    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && value >= 0.0 && value <= 1.0 {
            Some(Self(value))
        } else {
            None
        }
    }
}

impl Deref for Percentage {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
