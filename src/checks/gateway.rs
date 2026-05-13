use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

use super::{Check, utils};

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
        let status = read_gateway_status(runner);
        if status.trim().is_empty() {
            return;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&status) {
            if gateway_json_is_empty(&json) {
                return;
            }
            out.section("local:sep(0)");
            emit_gateway_json(out, &json);
        }
    }
}

fn read_gateway_status(runner: &CommandRunner) -> String {
    runner
        .run("configctl", ["interface", "gateways", "status"])
        .unwrap_or_default()
}

fn gateway_json_is_empty(json: &serde_json::Value) -> bool {
    json.as_array().is_some_and(Vec::is_empty)
}

fn emit_gateway_json(out: &mut AgentOutput, json: &serde_json::Value) {
    if let Some(items) = json.as_array() {
        for item in items {
            emit_gateway_item(out, item);
        }
    }
}

fn emit_gateway_item(out: &mut AgentOutput, value: &serde_json::Value) {
    let Some(name) = value.get("name").and_then(serde_json::Value::as_str) else {
        return;
    };
    let delay = gateway_metric(value, "delay").unwrap_or(0.0);
    let stddev = gateway_metric(value, "stddev").unwrap_or(0.0);
    let loss = gateway_metric(value, "loss").unwrap_or(0.0);
    let state = gateway_state(value, delay, loss);
    let summary = value
        .get("status_translated")
        .or_else(|| value.get("status"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Gateway status");

    out.local(
        state,
        &format!("Gateway {name}"),
        &format!("rtt={delay}|rttsd={stddev}|loss={loss}"),
        summary,
    );
}

fn gateway_state(value: &serde_json::Value, delay: f64, loss: f64) -> LocalState {
    match value
        .get("status")
        .and_then(serde_json::Value::as_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("down" | "force_down") => LocalState::Crit,
        Some("delay" | "loss" | "delay+loss") => LocalState::Warn,
        Some("none" | "online" | "up") => LocalState::Ok,
        Some(_) => LocalState::Unknown,
        None if loss > 90.0 => LocalState::Crit,
        None if loss > 0.0 || delay > 100.0 => LocalState::Warn,
        None => LocalState::Ok,
    }
}

fn gateway_metric(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(metric_value)
}

fn metric_value(value: &serde_json::Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    let raw = value.as_str()?.trim();
    let start = raw.find(|c: char| c.is_ascii_digit() || c == '-' || c == '.')?;
    let number = raw[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.')
        .collect::<String>();
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_current_gateway_status_array() {
        let json = serde_json::json!([
            {
                "name": "WAN_DHCP",
                "status": "none",
                "status_translated": "Online",
                "loss": "0.0 %",
                "delay": "12.5 ms",
                "stddev": "0.4 ms"
            },
            {
                "name": "LTE",
                "status": "down",
                "status_translated": "Offline",
                "loss": "100.0 %",
                "delay": "~",
                "stddev": "~"
            }
        ]);
        let mut out = AgentOutput::new();

        emit_gateway_json(&mut out, &json);

        assert_eq!(
            out.finish(),
            "0 \"Gateway WAN_DHCP\" rtt=12.5|rttsd=0.4|loss=0 Online\n\
2 \"Gateway LTE\" rtt=0|rttsd=0|loss=100 Offline\n"
        );
    }
}
