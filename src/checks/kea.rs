use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::anyhow;
use serde::{Deserialize, de::IgnoredAny};
use serde_json::json;

use super::Check;
use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

const KEA_URL: &str = "http://127.0.0.1:8000/";

pub struct Kea;

impl Check for Kea {
    fn name(&self) -> &'static str {
        "kea"
    }

    fn run(&self, _config: &Config, _runner: &CommandRunner) -> anyhow::Result<AgentOutput> {
        let mut out = AgentOutput::new();

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        let body = json!({ "command": "statistic-get-all", "service": ["dhcp4"] });

        // Kea may be disabled; treat connection errors as a no-op check.
        let Ok(response) = client.post(KEA_URL).json(&body).send() else {
            return Ok(out);
        };

        let response: Vec<KeaResponseItem> = response.json()?;
        let response = response
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("empty Kea response"))?;
        if response.result != 0 {
            return Err(anyhow!(
                "Kea statistic-get-all failed: {}",
                response.text.unwrap_or_default()
            ));
        }

        for pool in collect_pools(response.arguments)? {
            let PoolStats {
                subnet,
                pool,
                assigned,
                total,
            } = pool;
            let free = total.saturating_sub(assigned);
            let usage = if total == 0 {
                0.0
            } else {
                assigned as f64 / total as f64 * 100.0
            };
            let state = if usage > 90.0 {
                LocalState::Crit
            } else if usage > 70.0 {
                LocalState::Warn
            } else {
                LocalState::Ok
            };

            out.local(
                state,
                &format!("Kea DHCP Pool subnet {subnet} pool {pool}"),
                &format!("used={assigned}|total={total}|free={free}|usage={usage:.2}%"),
                &format!("{assigned}/{total} addresses used ({usage:.2}%)"),
            );
        }

        Ok(out)
    }
}

#[derive(Deserialize)]
struct KeaResponseItem {
    result: i64,
    text: Option<String>,
    #[serde(default)]
    arguments: BTreeMap<String, Vec<KeaSample>>,
}

#[derive(Deserialize)]
struct KeaSample(serde_json::Value, IgnoredAny);

struct PoolStats {
    subnet: u64,
    pool: u64,
    assigned: u64,
    total: u64,
}

#[derive(Default)]
struct PartialPool {
    assigned: Option<u64>,
    total: Option<u64>,
}

fn collect_pools(stats: BTreeMap<String, Vec<KeaSample>>) -> anyhow::Result<Vec<PoolStats>> {
    let mut pools = BTreeMap::<(u64, u64), PartialPool>::new();
    for (name, samples) in stats {
        let Some((subnet, pool, field)) =
            sscanf::sscanf!(&name, "subnet[{u64}].pool[{u64}].{str}").ok()
        else {
            continue;
        };
        let entry = pools.entry((subnet, pool)).or_default();
        let target = match field {
            "assigned-addresses" => &mut entry.assigned,
            "total-addresses" => &mut entry.total,
            _ => continue,
        };
        let value = samples
            .first()
            .and_then(|s| s.0.as_u64())
            .ok_or_else(|| anyhow!("Kea statistic {name} has no numeric sample"))?;
        *target = Some(value);
    }
    pools
        .into_iter()
        .map(|((subnet, pool), p)| {
            Ok(PoolStats {
                subnet,
                pool,
                assigned: p.assigned.ok_or_else(|| {
                    anyhow!("Kea pool subnet[{subnet}].pool[{pool}] missing assigned-addresses")
                })?,
                total: p.total.ok_or_else(|| {
                    anyhow!("Kea pool subnet[{subnet}].pool[{pool}] missing total-addresses")
                })?,
            })
        })
        .collect()
}
