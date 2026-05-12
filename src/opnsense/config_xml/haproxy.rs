use serde::Deserialize;

use super::enabled::MvcGeneral;

/// MVC `<OPNsense><HAProxy><general><enabled>…</enabled></general></HAProxy>`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HaproxySection {
    pub general: Option<MvcGeneral>,
}
