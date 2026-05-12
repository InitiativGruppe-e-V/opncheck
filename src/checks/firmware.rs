use std::{fs, path::Path, time::SystemTime};

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

    fn run(&self, out: &mut AgentOutput, _config: &Config, _runner: &CommandRunner) {
        let core_path = Path::new("/usr/local/opnsense/version/core");
        if !core_path.exists() {
            return;
        }
        let core = opnsense_data::read_core_version(core_path);
        let current = core.product_version.unwrap_or_else(|| "unknown".to_owned());
        let age = fs::metadata("/conf/config.xml")
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        out.section("local:sep(0)");
        out.local(
            LocalState::Ok,
            "OPNsense Firmware",
            &format!("update_available=0|apply_finish_time={age}"),
            &format!("Version {current}"),
        );
    }
}
