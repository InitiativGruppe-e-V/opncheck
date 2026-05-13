use std::collections::HashMap;

use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
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
    fn run(&self, config: &Config, runner: &CommandRunner) -> anyhow::Result<AgentOutput>;
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

pub fn collect_all(config: &Config, runner: &CommandRunner) -> AgentOutput {
    let mut check_errors = HashMap::new();
    let mut collect = AgentOutput::new();
    for check in all_checks() {
        if config.check_enabled(check.name()) {
            let out = check.run(config, runner);
            match out {
                Ok(out) => {
                    collect += out;
                }
                Err(e) => {
                    check_errors.insert(check.name(), e.to_string());
                }
            }
        }
    }

    collect.section("local:sep(0)");

    if check_errors.is_empty() {
        collect.local(
            LocalState::Ok,
            "OPNCheck Status",
            "status=ok",
            "All checks completed succesfully",
        );
    } else {
        let errors: Vec<String> = check_errors
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        let errors = errors.join("\n");
        let err_string = format!("Errors occurred during the following checks: \n{errors}");
        collect.local(
            LocalState::Crit,
            "OPNCheck Status",
            "status=err",
            &err_string,
        );
    }
    collect
}
