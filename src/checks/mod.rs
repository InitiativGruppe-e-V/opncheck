use crate::{agent::output::AgentOutput, config::Config, exec::CommandRunner};

pub mod opnsense;

type Collector = fn(&mut AgentOutput, &Config, &CommandRunner);

const COLLECTORS: &[(&str, Collector)] = &[
    ("firmware", opnsense::firmware_local),
    ("pkgaudit", opnsense::pkgaudit_local),
    ("services", opnsense::services_local),
    ("dhcp", opnsense::dhcp),
    ("gateway", opnsense::gateway_local),
    ("unbound", opnsense::unbound_local),
    ("squid", opnsense::squid),
    ("haproxy", opnsense::haproxy),
    ("nginx", opnsense::nginx_local),
    ("ipsec", opnsense::ipsec_local),
    ("wireguard", opnsense::wireguard_local),
];

pub fn collect_all(out: &mut AgentOutput, config: &Config, runner: &CommandRunner) {
    for (name, collector) in COLLECTORS {
        if config.check_enabled(name) {
            collector(out, config, runner);
        }
    }
}
