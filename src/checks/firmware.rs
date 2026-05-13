use anyhow::anyhow;

use super::Check;
use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

pub struct Firmware;

impl Check for Firmware {
    fn name(&self) -> &'static str {
        "firmware"
    }

    fn run(&self, _config: &Config, runner: &CommandRunner) -> anyhow::Result<AgentOutput> {
        let mut out = AgentOutput::new();
        out.section("local:sep(0)");

        let response = runner.run("configctl", ["firmware", "status"])?;

        let mut version: Option<&str> = None;
        let mut updates: Option<u64> = None;
        for line in response.lines() {
            if let Some(rest) = line.strip_prefix("Currently running OPNsense ") {
                version = rest.split_whitespace().next();
            }
            if let Ok((n, _)) =
                sscanf::sscanf!(line, "Checking for upgrades ({u64} candidates): {str}")
            {
                updates = Some(n);
            }
        }

        let version = version.ok_or_else(|| anyhow!("could not parse current OPNsense version"))?;
        let updates = updates.ok_or_else(|| anyhow!("could not parse upgrade candidate count"))?;

        let state = if updates == 0 {
            LocalState::Ok
        } else {
            LocalState::Warn
        };
        let summary = if updates == 0 {
            format!("Version {version}, up to date")
        } else {
            format!("Version {version}, {updates} update(s) available")
        };

        out.local(
            state,
            "OPNsense Firmware",
            &format!("updates={updates}"),
            &summary,
        );

        Ok(out)
    }
}
