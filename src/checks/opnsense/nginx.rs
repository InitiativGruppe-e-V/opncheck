use std::time::{Duration, SystemTime};

use serde::Deserialize;

use super::Check;
use crate::{
    config::Config,
    output::{LocalSection, LocalState},
    platform::{OPNSensePlatformData, OPNSenseX64},
    runner::CommandRunner,
    skip_check,
};

pub struct Nginx;

impl Check<OPNSenseX64> for Nginx {
    fn name(&self) -> &'static str {
        "nginx"
    }

    fn run(
        &self,
        config: &Config,
        platform_data: &OPNSensePlatformData,
        _runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        run_opnsense_nginx(config, platform_data)
    }
}

fn run_opnsense_nginx(
    config: &Config,
    platform_data: &OPNSensePlatformData,
) -> anyhow::Result<LocalSection> {
    let mut out = LocalSection::new();
    let opnsense_config = &platform_data.opnsense_config;
    if !opnsense_config.nginx_enabled() || !config.checks.nginx.status_socket.exists() {
        skip_check!();
    }

    let client = reqwest::blocking::Client::builder()
        .unix_socket(config.checks.nginx.status_socket.as_path())
        .timeout(Duration::from_secs(2))
        .build()?;

    let response = client
        .get("http://localhost/vts")
        .send()?
        .error_for_status()?
        .json::<VtsStatus>()?;

    let uptime = response.load_msec.map_or(0.0, nginx_uptime);
    write_vts_result(&mut out, uptime, "Nginx VTS status socket responding");
    Ok(out)
}

fn write_vts_result(out: &mut LocalSection, uptime: f64, summary: impl AsRef<str>) {
    out.row(LocalState::Ok, "Nginx Uptime", summary.as_ref())
        .with_metric("uptime", format!("{uptime:.0}"));
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
