use std::collections::BTreeMap;

use anyhow::{Context, anyhow};
use regex::Regex;
use serde::Deserialize;

use super::Check;
use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

pub struct Kea;

impl Check for Kea {
    fn name(&self) -> &'static str {
        "kea"
    }

    fn run(&self, _config: &Config, runner: &CommandRunner) -> anyhow::Result<AgentOutput> {
        let mut out = AgentOutput::new();

        let response = runner
            .run(
                "curl",
                [
                    "-sS",
                    "--max-time",
                    "2",
                    "-H",
                    "Content-Type: application/json",
                    "-d",
                    r#"{"command":"statistic-get-all","service":["dhcp4"]}"#,
                    "http://127.0.0.1:8000/",
                ],
            )
            .unwrap_or_default();
        if response.trim().is_empty() {
            return Ok(out);
        }

        let response: KeaResponse = serde_json::from_str(&response)
            .context("failed to parse Kea Control Agent statistic response")?;
        let response = response
            .0
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Kea Control Agent returned an empty response"))?;
        if response.result != 0 {
            return Err(anyhow!(
                "Kea Control Agent statistic-get-all failed with result {}: {}",
                response.result,
                response
                    .text
                    .unwrap_or_else(|| "no response text".to_owned())
            ));
        }

        for pool in pool_stats(response.arguments)? {
            let used = pool.assigned;
            let total = pool.total;
            let free = total.saturating_sub(used);
            let usage = if total == 0 {
                0.0
            } else {
                used as f64 / total as f64 * 100.0
            };

            out.local(
                LocalState::Ok,
                &format!("Kea DHCP Pool subnet {} pool {}", pool.subnet, pool.pool),
                &format!("used={used}|total={total}|free={free}|usage={usage:.2}%"),
                &format!("{used}/{total} addresses used ({usage:.2}%)"),
            );
        }

        Ok(out)
    }
}

#[derive(Deserialize)]
struct KeaResponse(Vec<KeaResponseItem>);

#[derive(Deserialize)]
struct KeaResponseItem {
    result: i64,
    text: Option<String>,
    #[serde(default)]
    arguments: BTreeMap<String, Vec<KeaStatisticSample>>,
}

#[derive(Deserialize)]
struct KeaStatisticSample(serde_json::Value, String);

#[derive(Default)]
struct PartialPoolStats {
    assigned: Option<u64>,
    total: Option<u64>,
}

struct PoolStats {
    subnet: u64,
    pool: u64,
    assigned: u64,
    total: u64,
}

fn pool_stats(
    statistics: BTreeMap<String, Vec<KeaStatisticSample>>,
) -> anyhow::Result<Vec<PoolStats>> {
    let regex =
        Regex::new(r"^subnet\[(\d+)\]\.pool\[(\d+)\]\.(assigned-addresses|total-addresses)$")?;
    let mut pools = BTreeMap::<(u64, u64), PartialPoolStats>::new();

    for (name, samples) in statistics {
        let Some(captures) = regex.captures(&name) else {
            continue;
        };
        let subnet = captures[1].parse::<u64>()?;
        let pool = captures[2].parse::<u64>()?;
        let value = samples
            .first()
            .and_then(KeaStatisticSample::as_u64)
            .ok_or_else(|| anyhow!("Kea statistic {name} has no numeric sample"))?;

        let stats = pools.entry((subnet, pool)).or_default();
        match &captures[3] {
            "assigned-addresses" => stats.assigned = Some(value),
            "total-addresses" => stats.total = Some(value),
            _ => unreachable!(),
        }
    }

    Ok(pools
        .into_iter()
        .filter_map(|((subnet, pool), stats)| {
            Some(PoolStats {
                subnet,
                pool,
                assigned: stats.assigned?,
                total: stats.total?,
            })
        })
        .collect())
}

impl KeaStatisticSample {
    fn as_u64(&self) -> Option<u64> {
        self.0
            .as_u64()
            .or_else(|| self.0.as_i64().and_then(|value| u64::try_from(value).ok()))
    }
}
