use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};
use super::{utils, Check};

pub struct Gateway;

impl Check for Gateway {
    fn name(&self) -> &'static str {
        "gateway"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
            return;
        };
        if !config_xml.has_gateways() {
            return;
        }
        let status = runner
            .run(
                "/usr/local/opnsense/scripts/routes/gateway_status.py",
                std::iter::empty::<&str>(),
            )
            .or_else(|_| runner.run("configctl", ["interface", "list", "status"]))
            .unwrap_or_default();
        if status.trim().is_empty() {
            return;
        }
        out.section("local:sep(0)");
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&status) {
            emit_gateway_json(out, &json);
        }
    }
}

fn emit_gateway_json(out: &mut AgentOutput, json: &serde_json::Value) {
    let Some(obj) = json.as_object() else {
        return;
    };
    for (name, value) in obj {
        let delay = value
            .get("delay")
            .or_else(|| value.get("rtt"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let loss = value.get("loss").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let status = if loss > 90.0 {
            LocalState::Crit
        } else if loss > 0.0 || delay > 100.0 {
            LocalState::Warn
        } else {
            LocalState::Ok
        };
        out.local(
            status,
            &format!("Gateway {name}"),
            &format!("rtt={delay}|rttsd=0|loss={loss}"),
            "Gateway status",
        );
    }
}
