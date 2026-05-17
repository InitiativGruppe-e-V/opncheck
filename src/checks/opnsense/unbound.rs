use std::{collections::HashMap, path::Path};

use super::Check;
use crate::{
    config::Config,
    output::{LocalSection, LocalState},
    platform::{OPNSensePlatformData, OPNSenseX64},
    runner::CommandRunner,
    skip_check,
};

pub struct Unbound;

impl Check<OPNSenseX64> for Unbound {
    fn name(&self) -> &'static str {
        "unbound"
    }

    fn run(
        &self,
        _config: &Config,
        platform_data: &OPNSensePlatformData,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let mut out = LocalSection::new();
        let opnsense_config = &platform_data.opnsense_config;
        if !opnsense_config.unbound_enabled() {
            skip_check!();
        }
        if !Path::new("/var/unbound/unbound.conf").exists() {
            skip_check!();
        }
        let data = runner
            .run(
                "/usr/local/sbin/unbound-control",
                ["-c", "/var/unbound/unbound.conf", "stats_noreset"],
            )
            .unwrap_or_default();
        if data.trim().is_empty() {
            out.row(LocalState::Crit, "Unbound DNS", "Unbound not running")
                .with_metric("dns_successes", "0")
                .with_metric("dns_recursion", "0")
                .with_metric("dns_cachehits", "0")
                .with_metric("dns_cachemiss", "0")
                .with_metric("avg_response_time", "0");
            return Ok(out);
        }
        let stats = data
            .lines()
            .filter_map(|line| line.strip_prefix("total.")?.split_once('='))
            .map(|(key, value)| (key.replace('.', "_"), value.to_owned()))
            .collect::<HashMap<_, _>>();
        out.row(LocalState::Ok, "Unbound DNS", "Unbound running")
            .with_metric(
                "dns_successes",
                stats.get("num_queries").map_or("0", String::as_str),
            )
            .with_metric(
                "dns_recursion",
                stats
                    .get("num_recursivereplies")
                    .map_or("0", String::as_str),
            )
            .with_metric(
                "dns_cachehits",
                stats.get("num_cachehits").map_or("0", String::as_str),
            )
            .with_metric(
                "dns_cachemiss",
                stats.get("num_cachemiss").map_or("0", String::as_str),
            )
            .with_metric(
                "avg_response_time",
                stats.get("recursion_time_avg").map_or("0", String::as_str),
            );
        Ok(out)
    }
}
