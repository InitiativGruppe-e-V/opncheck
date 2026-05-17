use serde::Deserialize;

#[derive(Debug, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Alert,
    Drop,
    #[serde(other)]
    #[default]
    Other,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Drop => "drop",
            Self::Other => "event",
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EveAction {
    Allowed,
    Blocked,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VerdictAction {
    Alert,
    Pass,
    Drop,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct EveAlert {
    #[serde(default)]
    pub action: Option<EveAction>,
    #[serde(default)]
    pub signature_id: Option<u64>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub severity: Option<u8>,
}

impl EveAlert {
    pub fn signature_summary(&self) -> String {
        let signature = self.signature.as_deref().unwrap_or("unknown signature");
        let mut summary = match self.signature_id {
            Some(signature_id) => format!("sid {signature_id} {signature}"),
            None => signature.to_owned(),
        };
        if let Some(category) = &self.category {
            summary.push_str(" / ");
            summary.push_str(category);
        }
        if let Some(severity) = self.severity {
            summary.push_str(&format!(" sev {severity}"));
        }
        summary
    }
}

#[derive(Debug, Deserialize)]
pub struct EveVerdict {
    #[serde(default)]
    pub action: Option<VerdictAction>,
}

#[derive(Debug, Deserialize)]
pub struct EveEvent {
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub flow_id: Option<u64>,
    pub event_type: EventType,
    #[serde(default)]
    pub in_iface: Option<String>,
    #[serde(default)]
    pub src_ip: Option<String>,
    #[serde(default)]
    pub src_port: Option<u16>,
    #[serde(default)]
    pub dest_ip: Option<String>,
    #[serde(default)]
    pub dest_port: Option<u16>,
    #[serde(default)]
    pub proto: Option<String>,
    #[serde(default)]
    pub alert: Option<EveAlert>,
    #[serde(default)]
    pub verdict: Option<EveVerdict>,
}

impl EveEvent {
    pub fn is_interesting(&self) -> bool {
        self.event_type == EventType::Alert || self.event_type == EventType::Drop
    }

    pub fn is_blocked(&self) -> bool {
        self.event_type == EventType::Drop
            || self
                .alert
                .as_ref()
                .is_some_and(|alert| alert.action == Some(EveAction::Blocked))
            || self
                .verdict
                .as_ref()
                .is_some_and(|verdict| verdict.action == Some(VerdictAction::Drop))
    }

    pub fn summary(&self) -> String {
        let action = if self.is_blocked() {
            "blocked"
        } else {
            "allowed"
        };
        let signature = self
            .alert
            .as_ref()
            .map(EveAlert::signature_summary)
            .unwrap_or_else(|| self.event_type.as_str().to_owned());
        let endpoint = self.endpoint_summary();
        match (&self.timestamp, &self.in_iface) {
            (Some(timestamp), Some(in_iface)) => {
                format!("{timestamp} {action} {signature} on {in_iface} {endpoint}")
            }
            (Some(timestamp), None) => {
                format!("{timestamp} {action} {signature} {endpoint}")
            }
            (None, Some(in_iface)) => format!("{action} {signature} on {in_iface} {endpoint}"),
            (None, None) => format!("{action} {signature} {endpoint}"),
        }
    }

    fn endpoint_summary(&self) -> String {
        let source = format_endpoint(self.src_ip.as_deref(), self.src_port);
        let destination = format_endpoint(self.dest_ip.as_deref(), self.dest_port);
        match (source, destination, self.proto.as_deref()) {
            (Some(source), Some(destination), Some(proto)) => {
                format!("{source} -> {destination} {proto}")
            }
            (Some(source), Some(destination), None) => format!("{source} -> {destination}"),
            _ => String::new(),
        }
    }
}

pub struct EventSummary {
    pub blocked: bool,
    pub text: String,
}

fn format_endpoint(ip: Option<&str>, port: Option<u16>) -> Option<String> {
    ip.map(|ip| match port {
        Some(port) => format!("{ip}:{port}"),
        None => ip.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_allowed_alert() {
        let event: EveEvent = serde_json::from_str(
            r#"{"event_type":"alert","alert":{"action":"allowed","signature_id":1}}"#,
        )
        .unwrap();

        assert!(event.is_interesting());
        assert!(!event.is_blocked());
    }

    #[test]
    fn detects_blocked_alert() {
        let event: EveEvent = serde_json::from_str(
            r#"{"event_type":"alert","alert":{"action":"blocked","signature_id":1}}"#,
        )
        .unwrap();

        assert!(event.is_blocked());
    }

    #[test]
    fn detects_drop_event() {
        let event: EveEvent = serde_json::from_str(r#"{"event_type":"drop"}"#).unwrap();

        assert!(event.is_interesting());
        assert!(event.is_blocked());
    }

    #[test]
    fn detects_drop_verdict() {
        let event: EveEvent = serde_json::from_str(
            r#"{"event_type":"alert","alert":{"action":"allowed"},"verdict":{"action":"drop"}}"#,
        )
        .unwrap();

        assert!(event.is_blocked());
    }
}
