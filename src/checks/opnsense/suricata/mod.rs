use anyhow::{Context, Result};

use super::Check;
use crate::{
    config::Config,
    output::{LocalSection, LocalState},
    platform::{OPNSensePlatformData, OPNSenseX64},
    runner::CommandRunner,
    skip_check,
};

mod eve;
mod reader;
mod state;

use self::reader::{CollectionResult, collect_from_file, rotated_log_path};
use self::state::{LogIdentity, SuricataState, read_state, write_state};

const SERVICE_NAME: &str = "OPNsense Suricata Events";

pub struct Suricata;

impl Check<OPNSenseX64> for Suricata {
    fn name(&self) -> &'static str {
        "suricata"
    }

    fn run(
        &self,
        config: &Config,
        _platform_data: &OPNSensePlatformData,
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
