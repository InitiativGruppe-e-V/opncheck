use std::{collections::HashMap, path::Path};

use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};
use super::{utils, Check};

pub struct Unbound;

impl Check for Unbound {
    fn name(&self) -> &'static str {
        "unbound"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
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
}
