use serde::Deserialize;

use super::Check;
use crate::{
    config::Config,
    output::{LocalSection, LocalState},
    platform::{LinuxX64, OPNSensePlatformData, OPNSenseX64},
    runner::CommandRunner,
    skip_check,
};

const SERVICE_NAME: &str = "OPNsense Services";

pub struct Services;

impl Check<OPNSenseX64> for Services {
    fn name(&self) -> &'static str {
        "services"
    }

    fn run(
        &self,
        config: &Config,
        _platform_data: &OPNSensePlatformData,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        run_opnsense_services(config, runner)
    }
}

impl Check<LinuxX64> for Services {
    fn name(&self) -> &'static str {
        "services"
    }

    fn run(
        &self,
        config: &Config,
        _platform_data: &(),
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        run_linux_services(config, runner)
    }
}

fn run_opnsense_services(config: &Config, runner: &CommandRunner) -> anyhow::Result<LocalSection> {
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

fn run_linux_services(config: &Config, runner: &CommandRunner) -> anyhow::Result<LocalSection> {
    let mut out = LocalSection::new();
    let output = runner.run(
        "systemctl",
        [
            "list-units",
            "--type=service",
            "--all",
            "--no-legend",
            "--no-pager",
        ],
    )?;
    if output.trim().is_empty() {
        skip_check!();
    }

    let mut ignored_services = 0;
    let services = output
        .lines()
        .filter_map(parse_systemd_service)
        .filter(|service| {
            if is_ignored(config, service.unit) || is_ignored(config, &service.description) {
                ignored_services += 1;
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>();

    let failed = services
        .iter()
        .filter(|service| service.active == "failed")
        .collect::<Vec<_>>();
    let changing = services
        .iter()
        .filter(|service| matches!(service.active, "activating" | "deactivating"))
        .collect::<Vec<_>>();

    if !failed.is_empty() {
        out.row(
            LocalState::Crit,
            "Linux Services",
            format!("Failed services: {}", linux_service_summary(&failed)),
        )
        .with_metric("failed_services", failed.len().to_string())
        .with_metric("changing_services", changing.len().to_string())
        .with_metric("ignored_services", ignored_services.to_string());
    } else if !changing.is_empty() {
        out.row(
            LocalState::Warn,
            "Linux Services",
            format!("Changing services: {}", linux_service_summary(&changing)),
        )
        .with_metric("failed_services", "0")
        .with_metric("changing_services", changing.len().to_string())
        .with_metric("ignored_services", ignored_services.to_string());
    } else {
        out.row(LocalState::Ok, "Linux Services", "No failed services")
            .with_metric("failed_services", "0")
            .with_metric("changing_services", "0")
            .with_metric("ignored_services", ignored_services.to_string());
    }

    Ok(out)
}

fn linux_service_summary(services: &[&SystemdService<'_>]) -> String {
    const MAX_SUMMARY_SERVICES: usize = 8;
    let mut names = services
        .iter()
        .take(MAX_SUMMARY_SERVICES)
        .map(|service| service.unit)
        .collect::<Vec<_>>();
    let remaining = services.len().saturating_sub(MAX_SUMMARY_SERVICES);
    if remaining > 0 {
        names.push("...");
    }
    names.join(", ")
}

fn parse_systemd_service(line: &str) -> Option<SystemdService<'_>> {
    let line = line.trim_start_matches([' ', '●']);
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let unit = fields.first().copied()?;
    let load = fields.get(1).copied()?;
    let active = fields.get(2).copied()?;
    let description = fields.get(4..).unwrap_or_default().join(" ");

    if load != "loaded" || !unit.ends_with(".service") {
        return None;
    }

    Some(SystemdService {
        unit,
        active,
        description,
    })
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

struct SystemdService<'a> {
    unit: &'a str,
    active: &'a str,
    description: String,
}

impl Service {
    fn is_running(&self) -> bool {
        self.status.contains(" is running")
    }
}
