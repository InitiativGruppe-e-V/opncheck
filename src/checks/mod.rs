use std::time::Instant;

use crate::{
    config::Config, exec::CommandRunner, opnsense::config_xml::OpnsenseConfig,
    plugin::output::LocalSection,
};

use self::meta::{status::CheckError, timings::CheckTiming};

pub mod firmware;
pub mod gateway;
pub mod kea;
pub mod meta;
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
    let mut check_timings = Vec::new();
    let mut sections = Vec::new();

    for check in all_checks() {
        if config.check_enabled(check.name()) {
            let started = Instant::now();
            let out = check.run(config, opnsense_config, runner);
            let elapsed = started.elapsed();
            check_timings.push(CheckTiming {
                section: check.name(),
                elapsed,
            });
            match out {
                Ok(out) => sections.push(out),
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

    sections.push(meta::timings::section(&check_timings));
    sections.push(meta::status::section(&check_errors));
    sections
}
