use std::{
    collections::HashMap,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use regex::Regex;

use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
    opnsense,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn checkmk_header(out: &mut AgentOutput, config: &Config, runner: &CommandRunner) {
    let core = opnsense::read_core_version(Path::new("/usr/local/opnsense/version/core"));
    let hostname = runner
        .run("hostname", std::iter::empty::<&str>())
        .unwrap_or_default()
        .trim()
        .to_owned();
    out.section("check_mk");
    out.line(format!(
        "AgentOS: {}",
        core.product_name.unwrap_or_else(|| "OPNsense".to_owned())
    ));
    out.line(format!("Version: {VERSION}"));
    out.line(format!("Hostname: {hostname}"));
    out.line(format!(
        "AgentDirectory: {}",
        config.paths.config_dir.display()
    ));
    out.line(format!("DataDirectory: {}", config.paths.var.display()));
    out.line(format!("SpoolDirectory: {}", config.paths.spool.display()));
    out.line(format!(
        "PluginsDirectory: {}",
        config.paths.plugins.display()
    ));
    out.line(format!("LocalDirectory: {}", config.paths.local.display()));
}

pub fn labels(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let dmesg = runner
        .run("dmesg", std::iter::empty::<&str>())
        .unwrap_or_default();
    if dmesg.to_lowercase().contains("hypervisor:") {
        out.section("labels:sep(0)");
        out.line(r#"{"cmk/device_type":"vm"}"#);
    }
}

pub fn firmware_local(out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
    let core_path = Path::new("/usr/local/opnsense/version/core");
    if !core_path.exists() {
        return;
    }
    let core = opnsense::read_core_version(core_path);
    let current = core.product_version.unwrap_or_else(|| "unknown".to_owned());
    out.section("local:sep(0)");
    out.local(
        LocalState::Ok,
        "Firmware",
        "update_available=0",
        &format!("Version {current}"),
    );
}

pub fn pkgaudit_local(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner
        .run("pkg", ["audit", "-F", "--raw=json-compact", "-q"])
        .unwrap_or_default();
    out.section("local:sep(0)");
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
        out.local(LocalState::Ok, "Audit", "issues=0", "OK");
        return;
    };
    let vulns = json.get("pkg_count").and_then(|v| v.as_u64()).unwrap_or(0);
    if vulns == 0 {
        out.local(LocalState::Ok, "Audit", "issues=0", "OK");
        return;
    }
    let packages = json
        .get("packages")
        .and_then(|v| v.as_object())
        .map(|packages| packages.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    out.local(
        LocalState::Warn,
        "Audit",
        &format!("issues={vulns}"),
        &format!("Pkg: {packages} vulnerable"),
    );
}

pub fn df(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("df", ["-kTP", "-t", "ufs"]).unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("df");
    out.lines(data.lines().skip(1).map(str::to_owned));
}

pub fn mounts(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("mount", ["-p", "-t", "ufs"]).unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("mounts");
    out.raw_block(data.trim_end());
}

pub fn cpu(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let loadavg = runner
        .run("sysctl", ["-n", "vm.loadavg"])
        .unwrap_or_default();
    if loadavg.trim().is_empty() {
        return;
    }
    let top = runner.run("top", ["-b", "-n", "1"]).unwrap_or_default();
    let proc = top
        .lines()
        .nth(1)
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .and_then(|parts| (parts.len() > 3).then(|| format!("{}/{}", parts[3], parts[0])))
        .unwrap_or_else(|| "0/0".to_owned());
    let lastpid = runner
        .run("sysctl", ["-n", "kern.lastpid"])
        .unwrap_or_default();
    let ncpu = runner.run("sysctl", ["-n", "hw.ncpu"]).unwrap_or_default();
    out.section("cpu");
    out.line(format!(
        "{} {} {} {}",
        loadavg.trim().trim_matches(|c| c == '{' || c == '}'),
        proc,
        lastpid.trim(),
        ncpu.trim()
    ));
}

pub fn mem(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let pagesize = runner
        .run("sysctl", ["-n", "hw.pagesize"])
        .unwrap_or_default()
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    let stats = runner.run("sysctl", ["vm.stats"]).unwrap_or_default();
    if pagesize == 0 || stats.trim().is_empty() {
        return;
    }
    let get = |key: &str| -> u64 {
        stats
            .lines()
            .find_map(|line| {
                line.strip_prefix(key)?
                    .split_once(": ")
                    .map(|(_, value)| value)
            })
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
            * pagesize
    };
    let cache = get("vm.stats.vm.v_cache_count");
    let free = get("vm.stats.vm.v_free_count");
    let inactive = get("vm.stats.vm.v_inactive_count");
    let total = get("vm.stats.vm.v_page_count");
    let used = total.saturating_sub(cache + free + inactive);
    out.section("statgrab_mem");
    out.line(format!("mem.cache {cache}"));
    out.line(format!("mem.free {free}"));
    out.line(format!("mem.total {total}"));
    out.line(format!("mem.used {used}"));
    out.line("swap.free 0");
    out.line("swap.total 0");
    out.line("swap.used 0");
}

pub fn kernel(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let stats = runner.run("sysctl", ["vm.stats"]).unwrap_or_default();
    let cp_time = runner
        .run("sysctl", ["-n", "kern.cp_time"])
        .unwrap_or_default();
    if stats.trim().is_empty() || cp_time.trim().is_empty() {
        return;
    }
    let values = parse_sysctl_colon(&stats);
    let cpus = cp_time.split_whitespace().collect::<Vec<_>>();
    if cpus.len() < 5 {
        return;
    }
    let processes = [
        "vm.stats.vm.v_forks",
        "vm.stats.vm.v_vforks",
        "vm.stats.vm.v_rforks",
        "vm.stats.vm.v_kthreads",
    ]
    .iter()
    .filter_map(|key| values.get(*key))
    .filter_map(|value| value.parse::<u64>().ok())
    .sum::<u64>();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    out.section("kernel");
    out.line(now.to_string());
    out.line(format!(
        "cpu {} {} {} {} {}",
        cpus[0], cpus[1], cpus[2], cpus[4], cpus[3]
    ));
    out.line(format!(
        "ctxt {}",
        values
            .get("vm.stats.sys.v_swtch")
            .cloned()
            .unwrap_or_default()
    ));
    out.line(format!("processes {processes}"));
}

pub fn temperature(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("sysctl", ["dev.cpu"]).unwrap_or_default();
    let temperatures = data
        .lines()
        .filter(|line| line.contains("temperature"))
        .filter_map(|line| line.split_once(": ").map(|(_, value)| value))
        .filter_map(|value| value.trim_end_matches('C').parse::<f64>().ok())
        .collect::<Vec<_>>();
    if temperatures.is_empty() {
        return;
    }
    let max_temp = temperatures
        .into_iter()
        .fold(f64::MIN, f64::max)
        .mul_add(1000.0, 0.0) as u64;
    out.section("lnx_thermal:sep(124)");
    out.line(format!("CPU|enabled|unknown|{max_temp}"));
}

pub fn netctr(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("netstat", ["-inb"]).unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("netctr");
    for line in data.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 12
            || matches!(parts[0], "Name" | "lo" | "plip")
            || !parts.contains(&"Link")
        {
            continue;
        }
        let Some(link_pos) = parts.iter().position(|part| *part == "Link") else {
            continue;
        };
        if parts.len() <= link_pos + 8 {
            continue;
        }
        out.line(format!(
            "{} {} {} {} {} 0 0 0 0 {} {} {} 0 0 0 0 0",
            parts[0],
            parts[link_pos + 5],
            parts[link_pos + 2],
            parts[link_pos + 3],
            parts[link_pos + 4],
            parts[link_pos + 8],
            parts[link_pos + 6],
            parts[link_pos + 7]
        ));
    }
}

pub fn ntp(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("ntpq", ["-np"]).unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("ntp");
    for line in data.lines().skip(2).filter(|line| !line.trim().is_empty()) {
        let marker = line.chars().next().unwrap_or(' ');
        out.line(format!("{} {}", marker, line.get(1..).unwrap_or_default()));
    }
}

pub fn ssh(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("sshd", ["-T"]).unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("sshd_config");
    out.raw_block(data.trim_end());
}

pub fn smartinfo(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    if !Path::new("/usr/local/sbin/smartctl").exists() && !Path::new("/usr/sbin/smartctl").exists()
    {
        return;
    }
    let Ok(dev_regex) = Regex::new(r"^(sd[a-z]+|da[0-9]+|nvme[0-9]+|ada[0-9]+)$") else {
        return;
    };
    let Ok(entries) = fs::read_dir("/dev") else {
        return;
    };
    out.section("disk_smart_info:sep(124)");
    for entry in entries.flatten() {
        let Some(device) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !dev_regex.is_match(&device) {
            continue;
        }
        let smart = runner
            .run(
                "smartctl",
                ["-a", "-n", "standby", &format!("/dev/{device}")],
            )
            .unwrap_or_default();
        emit_smart_summary(out, &device, &smart);
    }
}

pub fn ipmi(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    if !Path::new("/usr/local/bin/ipmitool").exists() && !Path::new("/usr/bin/ipmitool").exists() {
        return;
    }
    let data = runner
        .run("ipmitool", ["sensor", "list"])
        .unwrap_or_default();
    let lines = data
        .lines()
        .filter(|line| !line.contains(" na "))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }
    out.section("ipmi:sep(124)");
    out.lines(lines);
}

pub fn apcupsd(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner
        .run("apcaccess", std::iter::empty::<&str>())
        .unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("apcaccess:sep(58)");
    out.line("[[apcupsd.conf]]");
    out.raw_block(data.trim_end());
}

pub fn uptime(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let boottime = runner
        .run("sysctl", ["-n", "kern.boottime"])
        .unwrap_or_default();
    let Some(epoch) = Regex::new(r"sec = (\d+)")
        .ok()
        .and_then(|re| re.captures(&boottime))
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
    else {
        return;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let uptime = now.saturating_sub(epoch);
    out.section("uptime");
    out.line(format!("{uptime} 0"));
}

pub fn tcp(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("netstat", ["-na"]).unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    let established = data.matches("ESTABLISHED").count();
    let listen = data.matches("LISTEN").count();
    out.section("tcp_conn_stats");
    out.line(format!("ESTABLISHED {established}"));
    out.line(format!("LISTEN {listen}"));
}

pub fn ps(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner
        .run("ps", ["ax", "-o", "state,user,vsz,rss,pcpu,command"])
        .unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("ps");
    for line in data.lines().skip(1) {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 6 {
            continue;
        }
        let command = parts[5..].join(" ");
        out.line(format!(
            "({},{},{},{}) {}",
            parts[1], parts[2], parts[3], parts[4], command
        ));
    }
}

pub fn zpool(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let data = runner.run("zpool", ["status", "-x"]).unwrap_or_default();
    if data.trim().is_empty() {
        return;
    }
    out.section("zpool_status");
    out.lines(
        data.lines()
            .filter(|line| !line.contains("errors: No known data errors"))
            .map(str::to_owned),
    );
}

pub fn zfs(out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
    let zfs_get = runner
        .run(
            "zfs",
            [
                "get",
                "-t",
                "filesystem,volume",
                "-Hp",
                "name,quota,used,avail,mountpoint,type",
            ],
        )
        .unwrap_or_default();
    if zfs_get.trim().is_empty() {
        return;
    }
    out.section("zfsget");
    out.raw_block(zfs_get.trim_end());
    out.line("[df]");
    if let Ok(df) = runner.run("df", ["-kP", "-t", "zfs"]) {
        out.raw_block(df.trim_end());
    }
    if let Ok(arc) = runner.run("sysctl", ["-q", "kstat.zfs.misc.arcstats"])
        && !arc.trim().is_empty()
    {
        out.section("zfs_arc_cache");
        out.raw_block(
            &arc.replace("kstat.zfs.misc.arcstats.", "")
                .replace(": ", " = "),
        );
    }
}

#[allow(dead_code)]
fn file_mtime_age_seconds(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now()
        .duration_since(modified)
        .ok()
        .map(|d| d.as_secs())
}

fn parse_sysctl_colon(data: &str) -> HashMap<String, String> {
    data.lines()
        .filter_map(|line| line.split_once(": "))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn emit_smart_summary(out: &mut AgentOutput, device: &str, smart: &str) {
    if smart.trim().is_empty() {
        return;
    }
    const KEYS: &[(&str, &str)] = &[
        ("Device Model", "model_type"),
        ("Model Number", "model_family"),
        ("Model Family", "model_family"),
        ("Serial Number", "serial_number"),
        ("Firmware Version", "firmware_version"),
        (
            "SMART overall-health self-assessment test result",
            "smart_status",
        ),
        ("SMART Health Status", "smart_status"),
        ("Power_On_Hours", "poweronhours"),
        ("Power On Hours", "poweronhours"),
        ("Temperature_Celsius", "temperature"),
        ("Temperature", "temperature"),
        ("Reallocated_Sector_Ct", "reallocate"),
        ("Current_Pending_Sector", "pendingsector"),
        ("Offline_Uncorrectable", "uncorrectable"),
        ("UDMA_CRC_Error_Count", "udma_error"),
    ];
    for line in smart.lines() {
        for (needle, key) in KEYS {
            if let Some(value) = smart_value(line, needle) {
                out.line(format!("{device}|{key}|{value}"));
            }
        }
    }
}

fn smart_value(line: &str, needle: &str) -> Option<String> {
    if let Some((key, value)) = line.split_once(':')
        && key.trim() == needle
    {
        return Some(value.trim().to_owned());
    }
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.len() >= 10 && parts.get(1).copied() == Some(needle) {
        return parts.last().map(|value| (*value).to_owned());
    }
    None
}
