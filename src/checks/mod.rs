use std::collections::HashMap;

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
    let mut check_errors = HashMap::new();
    let mut sections = Vec::new();

    for check in all_checks() {
        if config.check_enabled(check.name()) {
            let out = check.run(config, opnsense_config, runner);
            match out {
                Ok(out) => {
                    sections.push(out);
                }
                Err(e) => {
                    check_errors.insert(check.name(), e.to_string());
                }
            }
        }
    }

    let mut status = LocalSection::new();

    if check_errors.is_empty() {
        status.add(
            LocalState::Ok,
            "OPNCheck Status",
            "status=ok",
            "All checks completed successfully",
        );
    } else {
        let errors: Vec<String> = check_errors
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        let errors = errors.join("\n");
        let err_string = format!("Errors occurred during some checks: \n{errors}");
        status.add(
            LocalState::Crit,
            "OPNCheck Status",
            "status=err",
            &err_string,
        );
    }

    sections.push(status);
    sections
}
