use serde::Deserialize;

use super::enabled::MvcGeneral;

/// MVC `<OPNsense><Nginx><general><enabled>…</enabled></general></Nginx>`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NginxSection {
    pub general: Option<MvcGeneral>,
}
