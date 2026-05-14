use serde::Deserialize;

use super::Check;
use crate::{
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml::OpnsenseConfig,
    plugin::output::{LocalSection, LocalState},
    skip_check,
};

const SERVICE_NAME: &str = "OPNsense Services";

pub struct Services;

impl Check for Services {
    fn name(&self) -> &'static str {
        "services"
    }

    fn run(
        &self,
        config: &Config,
        _opnsense_config: &OpnsenseConfig,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let mut out = LocalSection::new();

        let response = runner.run("configctl", ["service", "list"])?;
        let services = serde_json::from_str::<Vec<Service>>(&response)?;
        let ignored_services = services
            .iter()
            .filter(|service| is_ignored(config, service.name.as_str()))
            .count();
        let services = services
            .into_iter()
            .filter(|service| !is_ignored(config, service.name.as_str()))
            .collect::<Vec<_>>();

        if services.is_empty() {
            skip_check!();
        }
        write_services_result(&mut out, &services, ignored_services);

        Ok(out)
    }
}

fn write_services_result(out: &mut LocalSection, services: &[Service], ignored_services: usize) {
    let stopped = services
        .iter()
        .filter(|service| !service.is_running())
        .map(|service| service.description.as_str())
        .collect::<Vec<_>>();

    if stopped.is_empty() {
        out.row(LocalState::Ok, SERVICE_NAME, "All Services running")
            .with_metric("running_services", services.len().to_string())
            .with_metric("stopped_service", "0")
            .with_metric("ignored_services", ignored_services.to_string());
    } else {
        out.row(
            LocalState::Crit,
            SERVICE_NAME,
            format!("Services: {} not running", stopped.join(", ")),
        )
        .with_metric(
            "running_services",
            (services.len() - stopped.len()).to_string(),
        )
        .with_metric("stopped_service", stopped.len().to_string())
        .with_metric("ignored_services", ignored_services.to_string());
    }
}

fn is_ignored(config: &Config, service_name: &str) -> bool {
    let service_name = service_name.to_lowercase();
    config
        .checks
        .services
        .ignored
        .iter()
        .any(|ignored| service_name.contains(&ignored.to_lowercase()))
}

#[derive(Deserialize)]
struct Service {
    name: String,
    description: String,
    status: String,
}

impl Service {
    fn is_running(&self) -> bool {
        self.status.contains(" is running")
    }
}
