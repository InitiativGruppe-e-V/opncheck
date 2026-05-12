use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct System {
    pub hostname: Option<String>,
    pub domain: Option<String>,
}
