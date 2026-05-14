use std::{
    path::Path,
    time::{Duration, SystemTime},
};

use serde::Deserialize;

use super::Check;
use crate::{
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml::OpnsenseConfig,
    plugin::output::{LocalSection, LocalState},
    skip_check,
};

const STATUS_SOCKET: &str = "/var/run/nginx_status.sock";

pub struct Nginx;

impl Check for Nginx {
    fn name(&self) -> &'static str {
        "nginx"
    }

    fn run(
        &self,
        _config: &Config,
        opnsense_config: &OpnsenseConfig,
        _runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let mut out = LocalSection::new();
        if !opnsense_config.nginx_enabled() || !Path::new(STATUS_SOCKET).exists() {
            skip_check!();
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

        let uptime = response.load_msec.map_or(0.0, nginx_uptime);
        out.row(
            LocalState::Ok,
            "Nginx Uptime",
            "Nginx VTS status socket responding",
        )
        .with_metric("uptime", format!("{uptime:.0}"));
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
        .map_or(start_msec, |duration| duration.as_millis() as f64);
    ((now_msec - start_msec) / 1000.0).max(0.0)
}
