use std::time::Duration;

use crate::output::{LocalSection, LocalState};

use super::timings::format_latency;

pub struct CheckError {
    pub section: &'static str,
    pub error: String,
    pub elapsed: Duration,
}

pub fn section(check_errors: &[CheckError]) -> LocalSection {
    let mut section = LocalSection::new();

    if check_errors.is_empty() {
        section
            .row(
                LocalState::Ok,
                "OPNCheck Status",
                "All checks completed successfully",
            )
            .with_metric("status", "ok");
    } else {
        let errors: Vec<String> = check_errors
            .iter()
            .map(|error| {
                format!(
                    "{}: {} (after {})",
                    error.section,
                    error.error,
                    format_latency(error.elapsed)
                )
            })
            .collect();

        let errors = errors.join("\n");
        let errors = format!("Errors occurred during some checks: \n{errors}");

        section
            .row(LocalState::Crit, "OPNCheck Status", &errors)
            .with_metric("status", "err");
    }

    section
}
