use std::net::IpAddr;

use jiff::SignedDuration;
use serde::Deserialize;
use strum::Display;

use crate::{
    agent::output::{AgentOutput, LocalState},
    checks::utils::Percentage,
    config::Config,
    exec::CommandRunner,
};

use super::Check;

pub struct Gateway;

impl Check for Gateway {
    fn name(&self) -> &'static str {
        "gateway"
    }

    fn run(&self, _config: &Config, runner: &CommandRunner) -> anyhow::Result<AgentOutput> {
        let mut out = AgentOutput::new();

        let status = runner.run("configctl", ["interface", "gateways", "status"])?;
        let response: GatewayResponse = serde_json::from_str(&status)?;

        for gateway in response.0 {
            let GatewayInfo {
                name,
                address,
                status,
                loss,
                delay,
                stddev,
            } = gateway;

            let delay = delay.as_millis_f64();
            let stddev = stddev.as_millis_f64();

            let state = LocalState::from(status);

            out.local(
                state,
                &format!("Gateway {name}"),
                &format!("addr={address}|rtt={delay}ms|rttsd={stddev}ms|loss={loss}"),
                &status.to_string(),
            );
        }

        Ok(out)
    }
}

#[derive(Deserialize)]
pub struct GatewayResponse(Vec<GatewayInfo>);

#[derive(Deserialize)]
pub struct GatewayInfo {
    name: String,
    address: IpAddr,
    status: GatewayStatus,
    loss: Percentage,
    delay: SignedDuration,
    stddev: SignedDuration,
}

#[derive(Deserialize, Display, Clone, Copy)]
pub enum GatewayStatus {
    #[serde(rename = "none")]
    Online,
    #[serde(rename = "force_down")]
    OfflineForced,
    #[serde(rename = "down")]
    Offline,
    #[serde(rename = "delay")]
    Latency,
    #[serde(rename = "loss")]
    PacketLoss,
    #[serde(rename = "delay+loss")]
    LatencyAndPacketLoss,
    #[serde(other)]
    Pending,
}

impl From<GatewayStatus> for LocalState {
    fn from(value: GatewayStatus) -> Self {
        match value {
            GatewayStatus::Online => LocalState::Ok,
            GatewayStatus::OfflineForced | GatewayStatus::Latency | GatewayStatus::PacketLoss => {
                LocalState::Warn
            }
            GatewayStatus::Offline | GatewayStatus::LatencyAndPacketLoss => LocalState::Crit,
            GatewayStatus::Pending => LocalState::Unknown,
        }
    }
}
