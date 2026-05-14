use std::time::{Duration, Instant};

use crate::{
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml::OpnsenseConfig,
    plugin::output::{LocalSection, LocalState},
};

pub mod firmware;
pub mod gateway;
pub mod kea;
pub mod nginx;
pub mod pkgaudit;
pub mod services;
pub mod unbound;
pub mod utils;
pub mod wireguard;

pub trait Check {
    fn name(&self) -> &'static str;
    fn run(
        &self,
        config: &Config,
        opnsense_config: &OpnsenseConfig,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection>;
}

pub fn all_checks() -> &'static [&'static dyn Check] {
    &[
        &firmware::Firmware,
        &pkgaudit::PkgAudit,
        &services::Services,
        &kea::Kea,
        &gateway::Gateway,
        &unbound::Unbound,
        &nginx::Nginx,
        &wireguard::Wireguard,
    ]
}

pub fn collect_all(
    config: &Config,
    opnsense_config: &OpnsenseConfig,
    runner: &CommandRunner,
) -> Vec<LocalSection> {
    let mut check_errors = Vec::new();
    let mut sections = Vec::new();

    for check in all_checks() {
        if config.check_enabled(check.name()) {
            let started = Instant::now();
            let out = check.run(config, opnsense_config, runner);
            let elapsed = started.elapsed();
            match out {
                Ok(mut out) => {
                    out.inject("took", format_took(elapsed));
                    sections.push(out);
                }
                Err(e) => {
                    check_errors.push(CheckError {
                        section: check.name(),
                        error: e.to_string(),
                        elapsed,
                    });
                }
            }
        }
    }

    let mut status = LocalSection::new();

    if check_errors.is_empty() {
        status
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
                    "{}: {} (took {})",
                    error.section,
                    error.error,
                    format_took(error.elapsed)
                )
            })
            .collect();

        let errors = errors.join("\n");
        let errors = format!("Errors occurred during some checks: \n{errors}");

        let row = status
            .row(LocalState::Crit, "OPNCheck Status", &errors)
            .with_metric("status", "err");

        for error in &check_errors {
            row.with_metric(
                format!("{}_took", metric_key_suffix(error.section)),
                format_took(error.elapsed),
            );
        }
    }

    sections.push(status);
    sections
}

struct CheckError {
    section: &'static str,
    error: String,
    elapsed: Duration,
}

fn format_took(elapsed: Duration) -> String {
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
