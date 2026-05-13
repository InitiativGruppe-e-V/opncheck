use std::{
    path::Path,
    time::{Duration, SystemTime},
};

use serde::Deserialize;

use super::{Check, utils};
use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

const STATUS_SOCKET: &str = "/var/run/nginx_status.sock";

pub struct Nginx;

impl Check for Nginx {
    fn name(&self) -> &'static str {
        "nginx"
    }

    fn run(&self, _config: &Config, _runner: &CommandRunner) -> anyhow::Result<AgentOutput> {
        let mut out = AgentOutput::new();
        let Some(config_xml) = utils::read_opnsense_config() else {
            return Ok(out);
        };
        if !config_xml.nginx_enabled() || !Path::new(STATUS_SOCKET).exists() {
            return Ok(out);
        }

        let client = reqwest::blocking::Client::builder()
            .unix_socket(STATUS_SOCKET)
            .timeout(Duration::from_secs(2))
            .build()?;

        let response = client
            .get("http://localhost/vts")
            .send()?
            .error_for_status()?
            .json::<VtsStatus>()?;

        let uptime = response.load_msec.map(nginx_uptime).unwrap_or(0.0);

        out.section("local:sep(0)");
        out.local(
            LocalState::Ok,
            "Nginx Uptime",
            &format!("uptime={uptime:.0}"),
            "Nginx VTS status socket responding",
        );
        Ok(out)
    }
}

#[derive(Deserialize)]
struct VtsStatus {
    #[serde(rename = "loadMsec")]
    load_msec: Option<f64>,
}

fn nginx_uptime(start_msec: f64) -> f64 {
    let now_msec = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as f64)
        .unwrap_or(start_msec);
    ((now_msec - start_msec) / 1000.0).max(0.0)
}
