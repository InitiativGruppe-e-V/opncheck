use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};
use super::{utils, Check};

pub struct Ipsec;

impl Check for Ipsec {
    fn name(&self) -> &'static str {
        "ipsec"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
            return;
        };
        if !config_xml.ipsec_enabled() {
            return;
        }
        let data = runner
            .run(
                "/usr/local/opnsense/scripts/ipsec/list_status.py",
                std::iter::empty::<&str>(),
            )
            .unwrap_or_default();
        if data.trim().is_empty() {
            return;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
            return;
        };
        out.section("local:sep(0)");
        let Some(connections) = json.as_object() else {
            return;
        };
        for (name, conn) in connections {
            let sas = conn
                .get("sas")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let established = sas
                .iter()
                .any(|sa| sa.get("state").and_then(|v| v.as_str()) == Some("ESTABLISHED"));
            let state = if established {
                LocalState::Ok
            } else {
                LocalState::Crit
            };
            out.local(
                state,
                &format!("IPsec Tunnel: {name}"),
                "if_in_octets=0|if_out_octets=0|lifetime=0",
                if established {
                    "ESTABLISHED"
                } else {
                    "not connected"
                },
            );
        }
    }
}
