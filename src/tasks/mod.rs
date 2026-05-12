use std::{
    collections::HashMap,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use regex::Regex;

use crate::{
    agent::output::AgentOutput,
    config::Config,
    exec::{CommandRunner, safety},
};

pub fn collect(out: &mut AgentOutput, config: &Config, runner: &CommandRunner) {
    let Ok(entries) = fs::read_dir(&config.paths.tasks) else {
        return;
    };
    let mut task_count = 0_u64;
    let mut task_failed = 0_u64;
    let mut data = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("task") {
            continue;
        }
        if safety::ensure_safe_regular_file(&path, config.security.require_safe_paths).is_err() {
            continue;
        }
        let Some(task) = Task::from_file(&path) else {
            task_failed += 1;
            continue;
        };
        if task.disabled() {
            continue;
        }
        task_count += 1;
        match task.run(runner) {
            Some(output) if !output.trim().is_empty() => data.push(output),
            _ => task_failed += 1,
        }
    }
    if task_count == 0 {
        return;
    }
    let status = if task_failed == 0 { 0 } else { 1 };
    out.line(format!(
        "{status} 'CMK Tasks' tasks={task_count}|tasks_running=0,tasks_failed={task_failed} OK"
    ));
    for block in data {
        out.raw_block(block.trim_end());
    }
}

#[derive(Debug, Clone)]
struct Task {
    options: HashMap<String, String>,
}

impl Task {
    fn from_file(path: &Path) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        let regex = Regex::new(r"(?m)^(service|type|interval|interface|disabled|ipaddress|hostname|domain|port|piggyback|sshoptions|options|tenant):\s*(.*?)(?:\s+#|$)").ok()?;
        let options = regex
            .captures_iter(&raw)
            .map(|caps| (caps[1].to_owned(), caps[2].trim().to_owned()))
            .collect::<HashMap<_, _>>();
        Some(Self { options })
    }

    fn disabled(&self) -> bool {
        matches!(
            self.options.get("disabled").map(String::as_str),
            Some("1" | "true" | "yes")
        )
    }

    fn run(&self, runner: &CommandRunner) -> Option<String> {
        match self.options.get("type").map(String::as_str)? {
            "dummy" => Some(self.dummy()),
            "cmk" => Some(self.cmk()),
            "nmap" => self.nmap(runner),
            "blocklist" => self.blocklist(runner),
            "domain" => self.domain(runner),
            _ => None,
        }
    }

    fn dummy(&self) -> String {
        format!(
            "<<<local:sep(0)>>>\ncached({},{}) 0 Dummy - Test\n<<<>>>",
            epoch_seconds(),
            self.cache_lifetime()
        )
    }

    fn cmk(&self) -> String {
        format!(
            "<<<check_mk:cached({},{})>>>\nAgentOS: Task\nVersion: {}\n<<<>>>",
            epoch_seconds(),
            self.cache_lifetime(),
            env!("CARGO_PKG_VERSION")
        )
    }

    fn nmap(&self, runner: &CommandRunner) -> Option<String> {
        let host = self.options.get("hostname")?;
        let service = self.options.get("service").cloned().unwrap_or_default();
        let mut ports = self.options.get("port").cloned().unwrap_or_default();
        if !ports.is_empty() {
            ports.push(',');
        }
        let scan = format!(
            "{ports}U:53,67,123,111,137,138,161,427,500,623,1645,1646,1812,1813,4500,5060,5353,T:21,22,23,25,53,80,88,135,139,389,443,444,445,465,485,514,593,623,636,902,1433,1720,3128,3129,3268,3269,3389,5060,5900,5988,5989,6556,8000,8006,8010,8080,8084,8300,8443"
        );
        let data = runner
            .run(
                "nmap",
                [
                    "-Pn",
                    "-R",
                    "--disable-arp-ping",
                    "--open",
                    "--noninteractive",
                    "-oX",
                    "-",
                    "-sS",
                    "-sU",
                    "-p",
                    &scan,
                    host,
                ],
            )
            .ok()?;
        let ports = parse_nmap_ports(&data);
        let payload = serde_json::json!({ "host": host, "service": service, "ports": ports });
        Some(format!(
            "<<<nmap:sep(0):cached({},{})>>>\n{}\n<<<>>>",
            epoch_seconds(),
            self.cache_lifetime(),
            payload
        ))
    }

    fn blocklist(&self, runner: &CommandRunner) -> Option<String> {
        let service = self
            .options
            .get("service")
            .or_else(|| self.options.get("hostname"))
            .or_else(|| self.options.get("ipaddress"))?
            .to_owned();
        let ips = self
            .options
            .get("ipaddress")
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if ips.is_empty() {
            return None;
        }
        let mut listed = Vec::new();
        for ip in &ips {
            let reverse = reverse_ipv4(ip)?;
            for blacklist in BLACKLISTS {
                let query = format!("{reverse}.{blacklist}");
                if runner
                    .run("drill", [&query])
                    .unwrap_or_default()
                    .contains("ANSWER SECTION")
                {
                    listed.push(format!("{ip} is on {blacklist}"));
                }
            }
        }
        let status = if listed.is_empty() { 0 } else { 2 };
        let message = if listed.is_empty() {
            format!("{} not blocked", ips.join(" "))
        } else {
            listed.join(",")
        };
        Some(format!(
            "<<<local:sep(0)>>>\ncached({},{}) {status} 'Blocklist {service}' blocklist={}|blocked={} {message}\n<<<>>>",
            epoch_seconds(),
            self.cache_lifetime(),
            BLACKLISTS.len(),
            listed.len()
        ))
    }

    fn domain(&self, runner: &CommandRunner) -> Option<String> {
        let domain = self.options.get("domain")?;
        let ns = runner.run("drill", ["NS", domain]).unwrap_or_default();
        let mx = runner.run("drill", ["MX", domain]).unwrap_or_default();
        let result = serde_json::json!({
            "DOMAIN": domain,
            "DNSSEC": false,
            "SOA": [],
            "NS": parse_drill_records(&ns),
            "MX": parse_drill_records(&mx),
            "TLSA": []
        });
        Some(format!(
            "<<<domaincheck:sep(0):cached({},{})>>>\n{}\n<<<>>>",
            epoch_seconds(),
            self.cache_lifetime(),
            result
        ))
    }

    fn cache_lifetime(&self) -> u64 {
        self.options
            .get("interval")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600)
            * 2
    }
}

const BLACKLISTS: &[&str] = &[
    "all.s5h.net",
    "aspews.ext.sorbs.net",
    "b.barracudacentral.org",
    "bl.nordspam.com",
    "blackholes.five-ten-sg.com",
    "blacklist.woody.ch",
    "bogons.cymru.com",
    "cbl.abuseat.org",
    "combined.abuse.ch",
    "dnsbl.sorbs.net",
    "ix.dnsbl.manitu.net",
    "zen.spamhaus.org",
];

fn parse_nmap_ports(data: &str) -> Vec<serde_json::Value> {
    let Ok(regex) = Regex::new(
        r##"<port protocol="(?P<proto>tcp|udp)"\sportid="(?P<port>\d+)"(?s:.*?)state="(?P<state>[\w|]+)"\sreason="(?P<reason>[\w-]+)"(?s:.*?)(?:name="(?P<name>[\w-]+)")?"##,
    ) else {
        return Vec::new();
    };
    regex
        .captures_iter(data)
        .map(|caps| {
            serde_json::json!({
                "proto": caps.name("proto").map(|v| v.as_str()).unwrap_or(""),
                "port": caps.name("port").map(|v| v.as_str()).unwrap_or(""),
                "state": caps.name("state").map(|v| v.as_str()).unwrap_or(""),
                "reason": caps.name("reason").map(|v| v.as_str()).unwrap_or(""),
                "protoname": caps.name("name").map(|v| v.as_str()).unwrap_or(""),
            })
        })
        .collect()
}

fn parse_drill_records(data: &str) -> Vec<String> {
    data.lines()
        .filter(|line| !line.starts_with(';'))
        .filter_map(|line| line.split_whitespace().last())
        .map(|value| value.trim_end_matches('.').to_owned())
        .collect()
}

fn reverse_ipv4(ip: &str) -> Option<String> {
    let parts = ip.split('.').collect::<Vec<_>>();
    (parts.len() == 4).then(|| format!("{}.{}.{}.{}", parts[3], parts[2], parts[1], parts[0]))
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
