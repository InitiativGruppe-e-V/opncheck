use serde::Deserialize;

/// Legacy ISC dhcpd section. The real XML wraps per-interface entries
/// (`<dhcpd><lan><enable/>…</lan></dhcpd>`) but the only consumer here is a
/// presence check, so the inner shape is intentionally not modelled.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DhcpdSection {}
