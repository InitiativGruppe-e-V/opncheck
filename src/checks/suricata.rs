use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::Check;
use crate::{
    config::Config,
    runner::CommandRunner,
    xml::OpnsenseConfig,
    output::{LocalSection, LocalState},
    skip_check,
};

const SERVICE_NAME: &str = "OPNsense Suricata Events";
const ROTATED_SUFFIX: &str = "0";

pub struct Suricata;

impl Check for Suricata {
    fn name(&self) -> &'static str {
        "suricata"
    }

    fn run(
        &self,
        config: &Config,
        _opnsense_config: &OpnsenseConfig,
        _runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let log_path = &config.checks.suricata.log_path;
        if !log_path.exists() {
            skip_check!();
        }

        let mut out = LocalSection::new();
        match collect_events(config) {
            Ok(result) => write_result(&mut out, config, &result),
            Err(err) => {
                out.row(
                    LocalState::Unknown,
                    SERVICE_NAME,
                    format!("Failed: {err:#}"),
                )
                .with_metric("new_events", "0")
                .with_metric("new_alerts", "0")
                .with_metric("new_blocked", "0")
                .with_metric("parse_errors", "0")
                .with_metric("rotation_gap", "0");
            }
        }

        Ok(out)
    }
}

fn collect_events(config: &Config) -> Result<CollectionResult> {
    let log_path = &config.checks.suricata.log_path;
    let current_meta = LogIdentity::from_path(log_path)
        .with_context(|| format!("failed to read metadata for {}", log_path.display()))?;
    let previous_state = read_state(&config.checks.suricata.state_path)?;

    let mut result = CollectionResult::default();
    let next_offset = match previous_state {
        None if config.checks.suricata.initialize_from_end => {
            write_state(&config.checks.suricata.state_path, current_meta.to_state())?;
            return Ok(result);
        }
        None => collect_from_file(log_path, 0, &current_meta, &mut result)?,
        Some(state) if state.matches(&current_meta) => {
            if state.offset <= current_meta.len {
                collect_from_file(log_path, state.offset, &current_meta, &mut result)?
            } else {
                result.rotation_gap = true;
                collect_from_file(log_path, 0, &current_meta, &mut result)?
            }
        }
        Some(state) => {
            let rotated = rotated_log_path(log_path);
            if let Ok(rotated_meta) = LogIdentity::from_path(&rotated) {
                if state.matches(&rotated_meta) {
                    collect_from_file(
                        &rotated,
                        state.offset.min(rotated_meta.len),
                        &rotated_meta,
                        &mut result,
                    )?;
                } else {
                    result.rotation_gap = true;
                }
            } else {
                result.rotation_gap = true;
            }
            collect_from_file(log_path, 0, &current_meta, &mut result)?
        }
    };

    if result.parse_errors == 0 {
        write_state(
            &config.checks.suricata.state_path,
            SuricataState {
                device: current_meta.device,
                inode: current_meta.inode,
                offset: next_offset,
                last_timestamp: result.last_timestamp.clone(),
                last_flow_id: result.last_flow_id,
            },
        )?;
    }

    Ok(result)
}

fn collect_from_file(
    path: &Path,
    start: u64,
    identity: &LogIdentity,
    result: &mut CollectionResult,
) -> Result<u64> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(start))
        .with_context(|| format!("failed to seek {}", path.display()))?;

    let mut offset = start;
    loop {
        let mut line = Vec::new();
        let bytes = reader
            .read_until(b'\n', &mut line)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if bytes == 0 {
            break;
        }

        let complete_line = line.ends_with(b"\n");
        if !complete_line && offset + bytes as u64 >= identity.len {
            break;
        }

        let line_offset = offset;
        offset += bytes as u64;

        let line = String::from_utf8_lossy(&line);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<EveEvent>(line) {
            Ok(event) if event.is_interesting() => result.push_event(event),
            Ok(_) => {}
            Err(_) => {
                result.parse_errors += 1;
                offset = line_offset;
                break;
            }
        }
    }

    Ok(offset)
}

fn write_result(out: &mut LocalSection, config: &Config, result: &CollectionResult) {
    let state = if result.parse_errors > 0 {
        LocalState::Unknown
    } else if result.blocked > 0 {
        LocalState::Warn
    } else {
        LocalState::Ok
    };

    let summary = if result.parse_errors > 0 {
        format!(
            "Could not parse {} Suricata EVE line(s); state not advanced",
            result.parse_errors
        )
    } else if result.events == 0 {
        "No new Suricata events".to_owned()
    } else {
        let mut summary = format!(
            "{} new Suricata event(s), {} alert(s), {} blocked",
            result.events, result.alerts, result.blocked
        );
        let details = result
            .summaries
            .iter()
            .filter(|summary| config.checks.suricata.include_allowed_in_summary || summary.blocked)
            .take(config.checks.suricata.max_summary_events)
            .map(|summary| summary.text.clone())
            .collect::<Vec<_>>();
        if !details.is_empty() {
            summary.push_str(": ");
            summary.push_str(&details.join("; "));
        }
        summary
    };

    out.row(state, SERVICE_NAME, summary)
        .with_metric("new_events", result.events.to_string())
        .with_metric("new_alerts", result.alerts.to_string())
        .with_metric("new_blocked", result.blocked.to_string())
        .with_metric("parse_errors", result.parse_errors.to_string())
        .with_metric("rotation_gap", usize::from(result.rotation_gap).to_string());
}

fn read_state(path: &Path) -> Result<Option<SuricataState>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))
        .map(Some)
}

fn write_state(path: &Path, state: SuricataState) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state directory {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(&state).context("failed to serialize state")?;
    fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))
}

fn rotated_log_path(path: &Path) -> PathBuf {
    let mut rotated = path.as_os_str().to_owned();
    rotated.push(".");
    rotated.push(ROTATED_SUFFIX);
    PathBuf::from(rotated)
}

#[derive(Debug)]
struct LogIdentity {
    device: u64,
    inode: u64,
    len: u64,
}

impl LogIdentity {
    fn from_path(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path)?;
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            len: metadata.len(),
        })
    }

    fn to_state(&self) -> SuricataState {
        SuricataState {
            device: self.device,
            inode: self.inode,
            offset: self.len,
            last_timestamp: None,
            last_flow_id: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SuricataState {
    device: u64,
    inode: u64,
    offset: u64,
    #[serde(default)]
    last_timestamp: Option<String>,
    #[serde(default)]
    last_flow_id: Option<u64>,
}

impl SuricataState {
    fn matches(&self, identity: &LogIdentity) -> bool {
        self.device == identity.device && self.inode == identity.inode
    }
}

#[derive(Default)]
struct CollectionResult {
    events: usize,
    alerts: usize,
    blocked: usize,
    parse_errors: usize,
    rotation_gap: bool,
    summaries: Vec<EventSummary>,
    last_timestamp: Option<String>,
    last_flow_id: Option<u64>,
}

impl CollectionResult {
    fn push_event(&mut self, event: EveEvent) {
        self.events += 1;
        if event.event_type == EventType::Alert {
            self.alerts += 1;
        }
        if event.is_blocked() {
            self.blocked += 1;
        }
        self.summaries.push(EventSummary {
            blocked: event.is_blocked(),
            text: event.summary(),
        });
        if event.timestamp.is_some() {
            self.last_timestamp = event.timestamp;
        }
        if event.flow_id.is_some() {
            self.last_flow_id = event.flow_id;
        }
    }
}

struct EventSummary {
    blocked: bool,
    text: String,
}

#[derive(Debug, Deserialize)]
struct EveEvent {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    flow_id: Option<u64>,
    event_type: EventType,
    #[serde(default)]
    in_iface: Option<String>,
    #[serde(default)]
    src_ip: Option<String>,
    #[serde(default)]
    src_port: Option<u16>,
    #[serde(default)]
    dest_ip: Option<String>,
    #[serde(default)]
    dest_port: Option<u16>,
    #[serde(default)]
    proto: Option<String>,
    #[serde(default)]
    alert: Option<EveAlert>,
    #[serde(default)]
    verdict: Option<EveVerdict>,
}

impl EveEvent {
    fn is_interesting(&self) -> bool {
        self.event_type == EventType::Alert || self.event_type == EventType::Drop
    }

    fn is_blocked(&self) -> bool {
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

    fn summary(&self) -> String {
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

fn format_endpoint(ip: Option<&str>, port: Option<u16>) -> Option<String> {
    ip.map(|ip| match port {
        Some(port) => format!("{ip}:{port}"),
        None => ip.to_owned(),
    })
}

#[derive(Debug, Deserialize)]
struct EveAlert {
    #[serde(default)]
    action: Option<EveAction>,
    #[serde(default)]
    signature_id: Option<u64>,
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    severity: Option<u8>,
}

impl EveAlert {
    fn signature_summary(&self) -> String {
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
struct EveVerdict {
    #[serde(default)]
    action: Option<VerdictAction>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum EventType {
    Alert,
    Drop,
    #[serde(other)]
    Other,
}

impl Default for EventType {
    fn default() -> Self {
        Self::Other
    }
}

impl EventType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Drop => "drop",
            Self::Other => "event",
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum EveAction {
    Allowed,
    Blocked,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum VerdictAction {
    Alert,
    Pass,
    Drop,
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        path::{Path, PathBuf},
    };

    use super::*;
    use crate::config::Config;

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

    #[test]
    fn first_run_starts_at_end_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("eve.json");
        let state_path = tmp.path().join("suricata-state.json");
        fs::write(
            &log_path,
            r#"{"event_type":"alert","alert":{"action":"blocked"}}"#.to_owned() + "\n",
        )
        .unwrap();

        let config = test_config(log_path.clone(), state_path.clone(), true);
        let result = collect_events(&config).unwrap();

        assert_eq!(result.events, 0);
        let state = read_state(&state_path).unwrap().unwrap();
        assert_eq!(state.offset, fs::metadata(&log_path).unwrap().len());
    }

    #[test]
    fn reads_only_appended_events_after_initial_cursor() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("eve.json");
        let state_path = tmp.path().join("suricata-state.json");
        fs::write(
            &log_path,
            r#"{"event_type":"alert","alert":{"action":"allowed"}}"#.to_owned() + "\n",
        )
        .unwrap();

        let config = test_config(log_path.clone(), state_path, true);
        assert_eq!(collect_events(&config).unwrap().events, 0);

        append_line(
            &log_path,
            r#"{"event_type":"alert","alert":{"action":"allowed"}}"#,
        );
        append_line(
            &log_path,
            r#"{"event_type":"alert","alert":{"action":"blocked"}}"#,
        );

        let result = collect_events(&config).unwrap();

        assert_eq!(result.events, 2);
        assert_eq!(result.alerts, 2);
        assert_eq!(result.blocked, 1);
        assert_eq!(collect_events(&config).unwrap().events, 0);
    }

    #[test]
    fn reads_rotated_file_when_identity_matches_previous_state() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("eve.json");
        let rotated_path = tmp.path().join("eve.json.0");
        let state_path = tmp.path().join("suricata-state.json");
        fs::write(
            &log_path,
            r#"{"event_type":"alert","alert":{"action":"allowed"}}"#.to_owned() + "\n",
        )
        .unwrap();

        let config = test_config(log_path.clone(), state_path.clone(), true);
        assert_eq!(collect_events(&config).unwrap().events, 0);

        append_line(
            &log_path,
            r#"{"event_type":"alert","timestamp":"2026-05-15T10:00:00.000000+0200","flow_id":42,"alert":{"action":"allowed"}}"#,
        );
        fs::rename(&log_path, &rotated_path).unwrap();
        fs::write(
            &log_path,
            r#"{"event_type":"alert","alert":{"action":"blocked"}}"#.to_owned() + "\n",
        )
        .unwrap();

        let result = collect_events(&config).unwrap();
        let state = read_state(&state_path).unwrap().unwrap();

        assert_eq!(result.events, 2);
        assert_eq!(result.blocked, 1);
        assert!(!result.rotation_gap);
        assert_eq!(
            state.last_timestamp.as_deref(),
            Some("2026-05-15T10:00:00.000000+0200")
        );
        assert_eq!(state.last_flow_id, Some(42));
    }

    #[test]
    fn malformed_line_keeps_state_behind_error() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("eve.json");
        let state_path = tmp.path().join("suricata-state.json");
        fs::write(
            &log_path,
            r#"{"event_type":"alert","alert":{"action":"allowed"}}"#.to_owned()
                + "\n"
                + r#"{"event_type":"alert","alert":{"action":"blocked"}"#
                + "\n",
        )
        .unwrap();

        let config = test_config(log_path, state_path.clone(), false);
        let result = collect_events(&config).unwrap();

        assert_eq!(result.events, 1);
        assert_eq!(result.parse_errors, 1);
        assert!(read_state(&state_path).unwrap().is_none());
    }

    fn test_config(log_path: PathBuf, state_path: PathBuf, initialize_from_end: bool) -> Config {
        let mut config = Config::default();
        config.checks.suricata.log_path = log_path;
        config.checks.suricata.state_path = state_path;
        config.checks.suricata.initialize_from_end = initialize_from_end;
        config
    }

    fn append_line(path: &Path, line: &str) {
        let mut file = OpenOptions::new().append(true).open(path).unwrap();
        writeln!(file, "{line}").unwrap();
    }
}
