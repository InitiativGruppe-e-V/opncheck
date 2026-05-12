use crate::{agent::output::AgentOutput, config::Config, exec::CommandRunner};

pub mod dhcp;
pub mod firmware;
pub mod gateway;
pub mod haproxy;
pub mod ipsec;
pub mod nginx;
pub mod pkgaudit;
pub mod services;
pub mod unbound;
pub mod utils;
pub mod wireguard;

pub trait Check {
    fn name(&self) -> &'static str;
    fn run(&self, out: &mut AgentOutput, config: &Config, runner: &CommandRunner);
}

pub fn all_checks() -> &'static [&'static dyn Check] {
    &[
        &firmware::Firmware,
        &pkgaudit::PkgAudit,
        &services::Services,
        &dhcp::Dhcp,
        &gateway::Gateway,
        &unbound::Unbound,
        &haproxy::Haproxy,
        &nginx::Nginx,
        &ipsec::Ipsec,
        &wireguard::Wireguard,
    ]
}

pub fn collect_all(out: &mut AgentOutput, config: &Config, runner: &CommandRunner) {
    for check in all_checks() {
        if config.check_enabled(check.name()) {
            check.run(out, config, runner);
        }
    }
}
