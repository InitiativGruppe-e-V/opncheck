use std::time::Instant;

use crate::{
    config::Config,
    output::LocalSection,
    platform::{LinuxX64, OPNSenseX64, Platform},
    runner::CommandRunner,
    update::UpdateOutcome,
};

use self::meta::{status::CheckError, timings::CheckTiming};

pub mod meta;
pub mod opnsense;
pub mod services;
pub mod utils;

pub trait Check<P: Platform>: Sync {
    fn name(&self) -> &'static str;

    fn run(
        &self,
        config: &Config,
        platform_data: &P::PlatformData,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection>;
}

pub fn opnsense_checks() -> &'static [&'static dyn Check<OPNSenseX64>] {
    &[
        &opnsense::firmware::Firmware,
        &opnsense::pkgaudit::PkgAudit,
        &services::Services,
        &opnsense::kea::Kea,
        &opnsense::gateway::Gateway,
        &opnsense::unbound::Unbound,
        &opnsense::nginx::Nginx,
        &opnsense::wireguard::Wireguard,
        &opnsense::suricata::Suricata,
    ]
}

pub fn linux_checks() -> &'static [&'static dyn Check<LinuxX64>] {
    &[&services::Services]
}

pub fn collect_all<P: Platform>(
    config: &Config,
    platform_data: &P::PlatformData,
    checks: &'static [&'static dyn Check<P>],
    runner: &CommandRunner,
    update_result: anyhow::Result<UpdateOutcome>,
) -> Vec<LocalSection> {
    let mut check_errors = Vec::new();
    let mut check_timings = Vec::new();
    let mut sections = Vec::new();

    for check in checks {
        if config.check_enabled(check.name()) {
            let started = Instant::now();
            let out = check.run(config, platform_data, runner);
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
    sections.push(meta::version::section(config, update_result));
    sections
}
