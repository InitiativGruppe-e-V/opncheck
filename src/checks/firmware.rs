use std::path::Path;

use anyhow::bail;

use super::Check;
use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
    opnsense as opnsense_data,
};

pub struct Firmware;

impl Check for Firmware {
    fn name(&self) -> &'static str {
        "firmware"
    }

    fn run(&self, _config: &Config, _runner: &CommandRunner) -> anyhow::Result<AgentOutput> {
        let core_path = Path::new("/usr/local/opnsense/version/core");
        if !core_path.exists() {
            bail!("Did not find version check path");
        }
        let mut out = AgentOutput::new();
        let core = opnsense_data::read_core_version(core_path);
        let current = core.product_version.unwrap_or_else(|| "unknown".to_owned());

        out.section("local:sep(0)");
        out.local(
            LocalState::Ok,
            "OPNsense Firmware",
            &format!("update_available=0"),
            &format!("Version {current}"),
        );
        Ok(out)
    }
}
