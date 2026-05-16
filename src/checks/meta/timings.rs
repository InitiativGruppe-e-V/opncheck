use std::time::Duration;

use crate::output::{LocalSection, LocalState};

const WARN_AFTER: Duration = Duration::from_secs(10);

pub struct CheckTiming {
    pub section: &'static str,
    pub elapsed: Duration,
}

pub fn section(timings: &[CheckTiming]) -> LocalSection {
    let total = timings
        .iter()
        .fold(Duration::ZERO, |total, timing| total + timing.elapsed);

    let mut section = LocalSection::new();
    let state = if total > WARN_AFTER {
        LocalState::Warn
    } else {
        LocalState::Ok
    };
    let row = section.row(
        state,
        "OPNCheck Timings",
        format!("Checks completed in {}", format_latency(total)),
    );

    for timing in timings {
        row.with_metric(
            format!("{}_took", metric_key_suffix(timing.section)),
            format_latency(timing.elapsed),
        );
    }

    section
}

pub fn format_latency(elapsed: Duration) -> String {
    format!("{:.3}ms", elapsed.as_secs_f64() * 1000.0)
}

fn metric_key_suffix(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
