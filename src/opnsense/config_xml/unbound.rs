use serde::Deserialize;

use super::enabled::{LegacyEnable, MvcGeneral};

/// Legacy `<unbound><enable>1</enable></unbound>`.
pub type UnboundSection = LegacyEnable;

/// MVC `<OPNsense><unboundplus><general><enabled>…</enabled></general></unboundplus>`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UnboundPlusSection {
    pub general: Option<MvcGeneral>,
}
