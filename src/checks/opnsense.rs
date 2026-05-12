use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

use regex::Regex;

use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

pub fn net(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner
        .run("netstat", ["-i", "-b", "-d", "-n", "-W", "-f", "link"])
        .unwrap_or_default();
    let ifconfig = runner
        .run("ifconfig", ["-m", "-v", "-f", "inet:cidr,inet6:cidr"])
        .unwrap_or_default();
    if data.trim().is_empty() && ifconfig.trim().is_empty() {
        return;
    }
    out.section("statgrab_net");
    emit_netstat_interfaces(out, &data);
    emit_ifconfig_status(out, &ifconfig);
}

pub fn services_local(out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
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
            (parts.len() == 3).then(|| (parts[1].to_owned(), parts[2] == "1"))
        })
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
            "Services",
            &format!("running_services={}|stopped_service=0", services.len()),
            "All Services running",
        );
    } else {
        out.local(
            LocalState::Crit,
            "Services",
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

pub fn squid(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner
        .run(
            "fetch",
            ["-qo", "-", "http://127.0.0.1:3128/squid-internal-mgr/5min"],
        )
        .unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("squid");
    out.raw_block(data.trim_end());
}

pub fn haproxy(out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
    let Some(data) = unix_socket_command("/var/run/haproxy.socket", b"show stat\n") else {
        return;
    };
    out.section("haproxy:sep(44)");
    out.raw_block(data.trim_end());
}

pub fn nginx_local(out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
    let Some(data) = unix_socket_http(
        "/var/run/nginx_status.sock",
        b"GET / HTTP/1.0\r\nHost: nginx\r\n\r\n",
    ) else {
        return;
    };
    out.section("local:sep(0)");
    if data.contains("loadMsec") || data.contains("serverZones") || data.contains("upstreamZones") {
        out.local(
            LocalState::Ok,
            "Nginx Uptime",
            "uptime=1",
            "Nginx status socket responding",
        );
    } else {
        out.local(
            LocalState::Warn,
            "Nginx Uptime",
            "uptime=0",
            "Nginx status socket returned unexpected data",
        );
    }
}

pub fn ipsec_local(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
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

fn emit_netstat_interfaces(out: &mut AgentOutput, data: &str) {
    let mut lines = data.lines();
    let Some(header) = lines.next() else {
        return;
    };
    let headers = header
        .to_lowercase()
        .replace("pkts", "packets")
        .replace("coll", "collisions")
        .replace("errs", "errors")
        .replace("ibytes", "rx")
        .replace("obytes", "tx");
    let headers = headers.split_whitespace().collect::<Vec<_>>();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let values = line.split_whitespace().collect::<Vec<_>>();
        if values.len() < headers.len() {
            continue;
        }
        let row = headers.iter().zip(values.iter()).collect::<HashMap<_, _>>();
        let Some(name) = row.get(&"name") else {
            continue;
        };
        let sanitized = name.replace('.', "_");
        for key in [
            "mtu",
            "ipackets",
            "ierrors",
            "idrop",
            "rx",
            "opackets",
            "oerrors",
            "tx",
            "collisions",
            "drop",
        ] {
            if let Some(value) = row.get(&key) {
                out.line(format!("{sanitized}.{key} {value}"));
            }
        }
    }
}

fn emit_ifconfig_status(out: &mut AgentOutput, data: &str) {
    let ether_regex = Regex::new(r"(?m)^\s*ether\s+([^\s]+)").ok();
    for (iface, body) in split_ifconfig_blocks(data) {
        let sanitized = iface.replace('.', "_");
        let up = if body.contains("status: active")
            || body.contains("flags=") && body.contains("<UP,")
        {
            "true"
        } else {
            "false"
        };
        out.line(format!("{sanitized}.up {up}"));
        if let Some(mac) = ether_regex
            .as_ref()
            .and_then(|re| re.captures(&body))
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_owned()))
        {
            out.line(format!("{sanitized}.phys_address {mac}"));
        }
    }
}

fn split_ifconfig_blocks(data: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut current_iface: Option<String> = None;
    let mut current_body = String::new();
    for line in data.lines() {
        let is_header = !line.starts_with(char::is_whitespace)
            && line
                .split_once(':')
                .map(|(name, _)| {
                    name.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                })
                .unwrap_or(false);
        if is_header {
            if let Some(iface) = current_iface.take() {
                blocks.push((iface, std::mem::take(&mut current_body)));
            }
            current_iface = line.split_once(':').map(|(name, _)| name.to_owned());
        }
        current_body.push_str(line);
        current_body.push('\n');
    }
    if let Some(iface) = current_iface {
        blocks.push((iface, current_body));
    }
    blocks
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
    stream.write_all(command).ok()?;
    let mut data = String::new();
    stream.read_to_string(&mut data).ok()?;
    Some(data)
}

fn unix_socket_http(path: &str, request: &[u8]) -> Option<String> {
    unix_socket_command(path, request)
}
