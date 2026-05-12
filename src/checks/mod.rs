use crate::{agent::output::AgentOutput, config::Config, exec::CommandRunner};

pub mod opnsense;
pub mod system;

type Collector = fn(&mut AgentOutput, &Config, &CommandRunner);

const COLLECTORS: &[(&str, Collector)] = &[
    ("checkmk", system::checkmk_header),
    ("label", system::labels),
    ("df", system::df),
    ("mounts", system::mounts),
    ("cpu", system::cpu),
    ("mem", system::mem),
    ("uptime", system::uptime),
    ("tcp", system::tcp),
    ("ps", system::ps),
    ("zpool", system::zpool),
    ("zfs", system::zfs),
    ("firmware", system::firmware_local),
    ("kernel", system::kernel),
    ("temperature", system::temperature),
    ("netctr", system::netctr),
    ("ntp", system::ntp),
    ("ssh", system::ssh),
    ("smartinfo", system::smartinfo),
    ("ipmi", system::ipmi),
    ("apcupsd", system::apcupsd),
    ("pkgaudit", system::pkgaudit_local),
    ("net", opnsense::net),
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
