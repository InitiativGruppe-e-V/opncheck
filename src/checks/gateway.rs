use std::{collections::BTreeMap, net::IpAddr};

use jiff::SignedDuration;
use serde::Deserialize;
use strum::Display;

use crate::{
    config::Config,
    runner::CommandRunner,
    xml::OpnsenseConfig,
    output::{LocalSection, LocalState},
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

        for gateway in response.into_gateways() {
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
            let address = address
                .as_ref()
                .map_or_else(|| "no address".to_owned(), IpAddr::to_string);

            out.row(
                state,
                format!("Gateway {name}"),
                format!("{status} -> {address}, RTT {delay}ms"),
            )
            .with_metric("rtt", format!("{delay}ms"))
            .with_metric("rttsd", format!("{stddev}ms"))
            .with_metric("loss", loss.to_string());
        }

        Ok(out)
    }
}

/// Output of `configctl interface gateways status`.
///
/// Older OPNsense releases return a list of gateways. Newer releases return an
/// object keyed by gateway name (but still `[]` when there are no gateways).
#[derive(Deserialize)]
#[serde(untagged)]
pub enum GatewayResponse {
    List(Vec<GatewayInfo>),
    Map(BTreeMap<String, GatewayInfo>),
}

impl GatewayResponse {
    fn into_gateways(self) -> Vec<GatewayInfo> {
        match self {
            GatewayResponse::List(list) => list,
            GatewayResponse::Map(map) => map.into_values().collect(),
        }
    }
}

#[derive(Deserialize)]
pub struct GatewayInfo {
    name: String,
    address: CatchMissing<IpAddr>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_map_response() {
        let json = r#"{"WAN_DHCP":{"name":"WAN_DHCP","address":"192.0.2.1","status":"none","status_translated":"Online","loss":"0.0 %","delay":"1.234 ms","stddev":"0.1 ms","monitor":"192.0.2.1"},"WAN_DHCP6":{"name":"WAN_DHCP6","address":"~","status":"none","status_translated":"Online","loss":"~","delay":"~","stddev":"~","monitor":"~"}}"#;
        let gateways = serde_json::from_str::<GatewayResponse>(json)
            .unwrap()
            .into_gateways();
        assert_eq!(gateways.len(), 2);
        assert_eq!(gateways[0].name, "WAN_DHCP");
        assert!(gateways[1].address.is_none());
    }

    #[test]
    fn parses_list_response() {
        let json = r#"[{"name":"WAN_DHCP","address":"192.0.2.1","status":"down","loss":"100.0 %","delay":"~","stddev":"~","monitor":"192.0.2.1"}]"#;
        let gateways = serde_json::from_str::<GatewayResponse>(json)
            .unwrap()
            .into_gateways();
        assert_eq!(gateways.len(), 1);
    }

    #[test]
    fn parses_empty_response() {
        let gateways = serde_json::from_str::<GatewayResponse>("[]")
            .unwrap()
            .into_gateways();
        assert!(gateways.is_empty());
    }
}
