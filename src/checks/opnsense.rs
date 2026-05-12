use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, SystemTime},
};

use regex::Regex;

use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
    opnsense as opnsense_data,
};

pub fn firmware_local(out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
    let core_path = Path::new("/usr/local/opnsense/version/core");
    if !core_path.exists() {
        return;
    }
    let core = opnsense_data::read_core_version(core_path);
    let current = core.product_version.unwrap_or_else(|| "unknown".to_owned());
    let age = fs::metadata("/conf/config.xml")
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    out.section("local:sep(0)");
    out.local(
        LocalState::Ok,
        "OPNsense Firmware",
        &format!("update_available=0|apply_finish_time={age}"),
        &format!("Version {current}"),
    );
}

pub fn pkgaudit_local(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner
        .run("pkg", ["audit", "-F", "--raw=json-compact", "-q"])
        .unwrap_or_default();
    out.section("local:sep(0)");
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
        out.local(LocalState::Ok, "OPNsense Package Audit", "issues=0", "OK");
        return;
    };
    let vulns = json.get("pkg_count").and_then(|v| v.as_u64()).unwrap_or(0);
    if vulns == 0 {
        out.local(LocalState::Ok, "OPNsense Package Audit", "issues=0", "OK");
        return;
    }
    let packages = json
        .get("packages")
        .and_then(|v| v.as_object())
        .map(|packages| packages.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    out.local(
        LocalState::Warn,
        "OPNsense Package Audit",
        &format!("issues={vulns}"),
        &format!("Pkg: {packages} vulnerable"),
    );
}

pub fn services_local(out: &mut AgentOutput, config: &Config, _runner: &CommandRunner) {
    let php = r#"<?php require_once("config.inc");require_once("system.inc");require_once("plugins.inc");require_once("util.inc"); foreach(plugins_services() as $_service) { printf("%s;%s;%s\n",$_service["name"],$_service["description"],service_status($_service));} ?>"#;
    let Ok(output) = std::process::Command::new("php")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(php.as_bytes())?;
            }
            child.wait_with_output()
        })
    else {
        return;
    };
    let data = String::from_utf8_lossy(&output.stdout);
    let services = data
        .lines()
        .filter_map(|line| {
            let parts = line.splitn(3, ';').collect::<Vec<_>>();
            if parts.len() != 3 {
                return None;
            }
            let name = parts[0].to_owned();
            let description = parts[1].to_owned();
            let running = parts[2] == "1";
            Some((name, description, running))
        })
        .filter(|(name, _, _)| {
            let name_lower = name.to_lowercase();
            !config
                .checks
                .services_ignored
                .iter()
                .any(|ignored| name_lower.contains(&ignored.to_lowercase()))
        })
        .map(|(_, description, running)| (description, running))
        .collect::<Vec<_>>();
    if services.is_empty() {
        return;
    }
    let stopped = services
        .iter()
        .filter(|(_, running)| !running)
        .map(|(description, _)| description.clone())
        .collect::<Vec<_>>();
    out.section("local:sep(0)");
    if stopped.is_empty() {
        out.local(
            LocalState::Ok,
            "OPNsense Services",
            &format!("running_services={}|stopped_service=0", services.len()),
            "All Services running",
        );
    } else {
        out.local(
            LocalState::Crit,
            "OPNsense Services",
            &format!(
                "running_services={}|stopped_service={}",
                services.len() - stopped.len(),
                stopped.len()
            ),
            &format!("Services: {} not running", stopped.join(", ")),
        );
    }
}

pub fn dhcp(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let Some(config_xml) = read_opnsense_config() else {
        return;
    };
    if !config_xml.has_dhcp() {
        return;
    }
    let lease_path = Path::new("/var/dhcpd/var/db/dhcpd.leases");
    let Ok(leases) = fs::read_to_string(lease_path) else {
        return;
    };
    let pid = pidof(runner, "dhcpd").unwrap_or(-1);
    out.section("isc_dhcpd");
    out.line(format!("[general]\nPID: {pid}"));
    out.line("[leases]");
    let Ok(regex) = Regex::new(r"lease\s+(?P<ip>[0-9.]+)\s+\{(?s:.*?)binding state active") else {
        return;
    };
    let mut ips = regex
        .captures_iter(&leases)
        .map(|caps| caps["ip"].to_owned())
        .collect::<Vec<_>>();
    ips.sort();
    ips.dedup();
    out.lines(ips);
}

pub fn gateway_local(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let Some(config_xml) = read_opnsense_config() else {
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

pub fn unbound_local(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let Some(config_xml) = read_opnsense_config() else {
        return;
    };
    if !config_xml.unbound_enabled() {
        return;
    }
    if !Path::new("/var/unbound/unbound.conf").exists() {
        return;
    }
    let data = runner
        .run(
            "/usr/local/sbin/unbound-control",
            ["-c", "/var/unbound/unbound.conf", "stats_noreset"],
        )
        .unwrap_or_default();
    out.section("local:sep(0)");
    if data.trim().is_empty() {
        out.local(
            LocalState::Crit,
            "Unbound DNS",
            "dns_successes=0|dns_recursion=0|dns_cachehits=0|dns_cachemiss=0|avg_response_time=0",
            "Unbound not running",
        );
        return;
    }
    let stats = data
        .lines()
        .filter_map(|line| line.strip_prefix("total.")?.split_once('='))
        .map(|(key, value)| (key.replace('.', "_"), value.to_owned()))
        .collect::<HashMap<_, _>>();
    out.local(
        LocalState::Ok,
        "Unbound DNS",
        &format!(
            "dns_successes={}|dns_recursion={}|dns_cachehits={}|dns_cachemiss={}|avg_response_time={}",
            stats.get("num_queries").map(String::as_str).unwrap_or("0"),
            stats.get("num_recursivereplies").map(String::as_str).unwrap_or("0"),
            stats.get("num_cachehits").map(String::as_str).unwrap_or("0"),
            stats.get("num_cachemiss").map(String::as_str).unwrap_or("0"),
            stats.get("recursion_time_avg").map(String::as_str).unwrap_or("0"),
        ),
        "Unbound running",
    );
}

pub fn haproxy(out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
    let Some(config_xml) = read_opnsense_config() else {
        return;
    };
    if !config_xml.haproxy_enabled() {
        return;
    }
    if !Path::new("/var/run/haproxy.socket").exists() {
        return;
    }
    let Some(data) = unix_socket_command("/var/run/haproxy.socket", b"show stat\n") else {
        return;
    };
    out.section("haproxy:sep(44)");
    out.raw_block(data.trim_end());
}

pub fn nginx_local(out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
    let Some(config_xml) = read_opnsense_config() else {
        return;
    };
    if !config_xml.nginx_enabled() {
        return;
    }
    if !Path::new("/var/run/nginx_status.sock").exists() {
        return;
    }
    let Some(response) = unix_socket_http(
        "/var/run/nginx_status.sock",
        b"GET /vts HTTP/1.1\r\nHost: nginx\r\nConnection: close\r\n\r\n",
    )
    .or_else(|| {
        unix_socket_http(
            "/var/run/nginx_status.sock",
            b"GET / HTTP/1.1\r\nHost: nginx\r\nConnection: close\r\n\r\n",
        )
    }) else {
        return;
    };

    let (status, body) = split_http_response(&response);
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

pub fn ipsec_local(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let Some(config_xml) = read_opnsense_config() else {
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

pub fn wireguard_local(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let Some(config_xml) = read_opnsense_config() else {
        return;
    };
    if !config_xml.wireguard_enabled() {
        return;
    }
    let data = runner
        .run("wg", ["show", "all", "dump"])
        .unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("local:sep(0)");
    for line in data.lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 9 {
            continue;
        }
        let iface = fields[0];
        let peer = fields[1];
        let endpoint = fields[3];
        let received = fields[6];
        let sent = fields[7];
        out.local(
            LocalState::Ok,
            &format!("WireGuard Client: {peer}"),
            &format!("if_in_octets={received}|if_out_octets={sent}"),
            &format!("{iface}: {endpoint}"),
        );
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

fn read_opnsense_config() -> Option<opnsense_data::config_xml::OpnsenseConfig> {
    opnsense_data::config_xml::read_config(Path::new("/conf/config.xml"))
}

fn pidof(runner: &CommandRunner, process_name: &str) -> Option<i64> {
    let data = runner.run("ps", ["ax", "-c", "-o", "command,pid"]).ok()?;
    data.lines().find_map(|line| {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        (parts.len() == 2 && parts[0] == process_name)
            .then(|| parts[1].parse::<i64>().ok())
            .flatten()
    })
}

fn unix_socket_command(path: &str, command: &[u8]) -> Option<String> {
    let mut stream = UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;
    stream.write_all(command).ok()?;
    let mut data = String::new();
    stream.read_to_string(&mut data).ok()?;
    Some(data)
}

fn unix_socket_http(path: &str, request: &[u8]) -> Option<String> {
    unix_socket_command(path, request)
}

fn split_http_response(response: &str) -> (Option<u16>, &str) {
    let status = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok());
    let body = response
        .split_once("\r\n\r\n")
        .or_else(|| response.split_once("\n\n"))
        .map(|(_, body)| body)
        .unwrap_or(response);
    (status, body)
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
