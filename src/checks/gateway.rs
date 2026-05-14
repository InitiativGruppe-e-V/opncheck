use std::net::IpAddr;

use jiff::SignedDuration;
use serde::Deserialize;
use strum::Display;

use crate::{
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml::OpnsenseConfig,
    plugin::output::{LocalSection, LocalState},
    utils::{catch_missing::CatchMissing, percentage::Percentage},
};

use super::Check;

pub struct Gateway;

impl Check for Gateway {
    fn name(&self) -> &'static str {
        "gateway"
    }

    fn run(
        &self,
        _config: &Config,
        _opnsense_config: &OpnsenseConfig,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let mut out = LocalSection::new();

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

            let delay = delay.unwrap_or_default().as_millis_f64();
            let stddev = stddev.unwrap_or_default().as_millis_f64();
            let loss = loss.as_ref().unwrap_or(&Percentage::HUNDRED);

            let state = LocalState::from(status);

            out.row(
                state,
                &format!("Gateway {name}"),
                &format!("{status} -> {address}, RTT {delay}ms"),
            )
            .with_metric("rtt", format!("{delay}ms"))
            .with_metric("rttsd", format!("{stddev}ms"))
            .with_metric("loss", loss.to_string());
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
    loss: CatchMissing<Percentage>,
    delay: CatchMissing<SignedDuration>,
    stddev: CatchMissing<SignedDuration>,
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
