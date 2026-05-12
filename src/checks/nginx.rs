use std::{path::Path, time::SystemTime};

use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};
use super::{utils, Check};

pub struct Nginx;

impl Check for Nginx {
    fn name(&self) -> &'static str {
        "nginx"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
            return;
        };
        if !config_xml.nginx_enabled() {
            return;
        }
        if !Path::new("/var/run/nginx_status.sock").exists() {
            return;
        }
        let Some(response) = utils::unix_socket_http(
            "/var/run/nginx_status.sock",
            b"GET /vts HTTP/1.1\r\nHost: nginx\r\nConnection: close\r\n\r\n",
        )
        .or_else(|| {
            utils::unix_socket_http(
                "/var/run/nginx_status.sock",
                b"GET / HTTP/1.1\r\nHost: nginx\r\nConnection: close\r\n\r\n",
            )
        }) else {
            return;
        };

        let (status, body) = utils::split_http_response(&response);
        if !matches!(status, Some(200..=299) | None) {
            return;
        }

        out.section("local:sep(0)");
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(uptime) = nginx_uptime(&json) {
                out.local(
                    LocalState::Ok,
                    "Nginx Uptime",
                    &format!("uptime={uptime:.0}"),
                    "Nginx VTS status socket responding",
                );
                return;
            }
            out.local(
                LocalState::Ok,
                "Nginx Uptime",
                "uptime=0",
                "Nginx status socket responding without loadMsec",
            );
        } else {
            out.local(
                LocalState::Ok,
                "Nginx Uptime",
                "uptime=0",
                "Nginx status socket responding",
            );
        }
    }
}

fn nginx_uptime(json: &serde_json::Value) -> Option<f64> {
    json.get("loadMsec")
        .and_then(|value| value.as_f64())
        .map(|start_msec| {
            let now_msec = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis() as f64)
                .unwrap_or(start_msec);
            ((now_msec - start_msec) / 1000.0).max(0.0)
        })
}
