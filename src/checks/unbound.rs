use std::{collections::HashMap, path::Path};

use super::Check;
use crate::{
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml::OpnsenseConfig,
    plugin::output::{LocalSection, LocalState},
};

pub struct Unbound;

impl Check for Unbound {
    fn name(&self) -> &'static str {
        "unbound"
    }

    fn run(
        &self,
        _config: &Config,
        opnsense_config: &OpnsenseConfig,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let mut out = LocalSection::new();
        if !opnsense_config.unbound_enabled() {
            crate::skip_check!();
        }
        if !Path::new("/var/unbound/unbound.conf").exists() {
            crate::skip_check!();
        }
        let data = runner
            .run(
                "/usr/local/sbin/unbound-control",
                ["-c", "/var/unbound/unbound.conf", "stats_noreset"],
            )
            .unwrap_or_default();
        if data.trim().is_empty() {
            out.add(
                LocalState::Crit,
                "Unbound DNS",
                "dns_successes=0|dns_recursion=0|dns_cachehits=0|dns_cachemiss=0|avg_response_time=0",
                "Unbound not running",
            );
            return Ok(out);
        }
        let stats = data
            .lines()
            .filter_map(|line| line.strip_prefix("total.")?.split_once('='))
            .map(|(key, value)| (key.replace('.', "_"), value.to_owned()))
            .collect::<HashMap<_, _>>();
        out.add(
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
        Ok(out)
    }
}
