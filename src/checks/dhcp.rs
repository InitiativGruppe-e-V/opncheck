use std::{fs, path::Path};

use regex::Regex;

use crate::{agent::output::AgentOutput, config::Config, exec::CommandRunner};
use super::{utils, Check};

pub struct Dhcp;

impl Check for Dhcp {
    fn name(&self) -> &'static str {
        "dhcp"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
            return;
        };
        if !config_xml.has_dhcp() {
            return;
        }
        let lease_path = Path::new("/var/dhcpd/var/db/dhcpd.leases");
        let Ok(leases) = fs::read_to_string(lease_path) else {
            return;
        };
        let pid = utils::pidof(runner, "dhcpd").unwrap_or(-1);
        out.section("isc_dhcpd");
        out.line(format!("[general]\nPID: {pid}"));
        out.line("[leases]");
        let Ok(regex) = Regex::new(r"lease\s+(?P<ip>[0-9.]+)\s+\{(?s:.*?)binding state active") else {
            return;
        };
        let mut ips = regex
            .captures_iter(&leases)
            .map(|caps| caps["ip"].to_owned())
            .collect::<Vec<_>>();
        ips.sort();
        ips.dedup();
        out.lines(ips);
    }
}
