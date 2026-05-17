use std::{
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::eve::{EveEvent, EventSummary, EventType};
use super::state::LogIdentity;

const ROTATED_SUFFIX: &str = "0";

#[derive(Default)]
pub struct CollectionResult {
    pub events: usize,
    pub alerts: usize,
    pub blocked: usize,
    pub parse_errors: usize,
    pub rotation_gap: bool,
    pub summaries: Vec<EventSummary>,
    pub last_timestamp: Option<String>,
    pub last_flow_id: Option<u64>,
}

impl CollectionResult {
    pub fn push_event(&mut self, event: EveEvent) {
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

pub fn collect_from_file(
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

pub fn rotated_log_path(path: &Path) -> PathBuf {
    let mut rotated = path.as_os_str().to_owned();
    rotated.push(".");
    rotated.push(ROTATED_SUFFIX);
    PathBuf::from(rotated)
}
