use std::io::Write;

use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};
use super::Check;

pub struct Services;

impl Check for Services {
    fn name(&self) -> &'static str {
        "services"
    }

    fn run(&self, out: &mut AgentOutput, config: &Config, _runner: &CommandRunner) {
        let php = r#"<?php require_once("config.inc");require_once("system.inc");require_once("plugins.inc");require_once("util.inc"); foreach(plugins_services() as $_service) { printf("%s;%s;%s\n",$_service["name"],$_service["description"],service_status($_service));} ?>"#;
        let Ok(output) = std::process::Command::new("php")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(php.as_bytes())?;
                }
                child.wait_with_output()
            })
        else {
            return;
        };
        let data = String::from_utf8_lossy(&output.stdout);
        let services = data
            .lines()
            .filter_map(|line| {
                let parts = line.splitn(3, ';').collect::<Vec<_>>();
                if parts.len() != 3 {
                    return None;
                }
                let name = parts[0].to_owned();
                let description = parts[1].to_owned();
                let running = parts[2] == "1";
                Some((name, description, running))
            })
            .filter(|(name, _, _)| {
                let name_lower = name.to_lowercase();
                !config
                    .checks
                    .services_ignored
                    .iter()
                    .any(|ignored| name_lower.contains(&ignored.to_lowercase()))
            })
            .map(|(_, description, running)| (description, running))
            .collect::<Vec<_>>();
        if services.is_empty() {
            return;
        }
        let stopped = services
            .iter()
            .filter(|(_, running)| !running)
            .map(|(description, _)| description.clone())
            .collect::<Vec<_>>();
        out.section("local:sep(0)");
        if stopped.is_empty() {
            out.local(
                LocalState::Ok,
                "OPNsense Services",
                &format!("running_services={}|stopped_service=0", services.len()),
                "All Services running",
            );
        } else {
            out.local(
                LocalState::Crit,
                "OPNsense Services",
                &format!(
                    "running_services={}|stopped_service={}",
                    services.len() - stopped.len(),
                    stopped.len()
                ),
                &format!("Services: {} not running", stopped.join(", ")),
            );
        }
    }
}
