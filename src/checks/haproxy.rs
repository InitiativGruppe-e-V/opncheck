use std::path::Path;

use crate::{agent::output::AgentOutput, config::Config, exec::CommandRunner};
use super::{utils, Check};

pub struct Haproxy;

impl Check for Haproxy {
    fn name(&self) -> &'static str {
        "haproxy"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
            return;
        };
        if !config_xml.haproxy_enabled() {
            return;
        }
        if !Path::new("/var/run/haproxy.socket").exists() {
            return;
        }
        let Some(data) = utils::unix_socket_command("/var/run/haproxy.socket", b"show stat\n") else {
            return;
        };
        out.section("haproxy:sep(44)");
        out.raw_block(data.trim_end());
    }
}
