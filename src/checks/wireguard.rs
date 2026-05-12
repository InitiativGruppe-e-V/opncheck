use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};
use super::{utils, Check};

pub struct Wireguard;

impl Check for Wireguard {
    fn name(&self) -> &'static str {
        "wireguard"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
            return;
        };
        if !config_xml.wireguard_enabled() {
            return;
        }
        let data = runner
            .run("wg", ["show", "all", "dump"])
            .unwrap_or_default();
        if data.trim().is_empty() {
            return;
        }
        out.section("local:sep(0)");
        for line in data.lines() {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 9 {
                continue;
            }
            let iface = fields[0];
            let peer = fields[1];
            let endpoint = fields[3];
            let received = fields[6];
            let sent = fields[7];
            out.local(
                LocalState::Ok,
                &format!("WireGuard Client: {peer}"),
                &format!("if_in_octets={received}|if_out_octets={sent}"),
                &format!("{iface}: {endpoint}"),
            );
        }
    }
}
