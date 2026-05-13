use std::collections::BTreeMap;

use anyhow::{Context, bail};
use serde::Deserialize;

use super::Check;
use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

const SERVICE_NAME: &str = "OPNsense Package Audit";
const MAX_SUMMARY_PACKAGES: usize = 5;
const MAX_CVES_PER_PACKAGE: usize = 3;

pub struct PkgAudit;

impl Check for PkgAudit {
    fn name(&self) -> &'static str {
        "pkgaudit"
    }

    fn run(&self, _config: &Config, runner: &CommandRunner) -> anyhow::Result<AgentOutput> {
        let mut out = AgentOutput::new();
        out.section("local:sep(0)");

        let output = runner.run_output("pkg", ["audit", "-F", "--raw=json-compact", "-q"])?;
        let stdout = output.stdout().trim();

        if stdout.is_empty() {
            if output.success() {
                write_healthy_result(&mut out);
                return Ok(out);
            }

            bail!(
                "pkg audit failed without JSON output: {}",
                output.stderr().trim()
            );
        }

        let audit = serde_json::from_str::<PkgAuditResponse>(stdout)
            .context("failed to parse pkg audit JSON output")?;
        write_audit_result(&mut out, &audit);

        Ok(out)
    }
}

fn write_healthy_result(out: &mut AgentOutput) {
    out.local(LocalState::Ok, SERVICE_NAME, "packages=0|issues=0", "OK");
}

fn write_audit_result(out: &mut AgentOutput, audit: &PkgAuditResponse) {
    let package_count = audit.package_count();
    let issue_count = audit.issue_count();

    if package_count == 0 && issue_count == 0 {
        write_healthy_result(out);
        return;
    }

    out.local(
        LocalState::Warn,
        SERVICE_NAME,
        &format!("packages={package_count}|issues={issue_count}"),
        &format!("Vulnerable packages: {}", audit.summary()),
    );
}

#[derive(Deserialize)]
struct PkgAuditResponse {
    #[serde(default)]
    pkg_count: usize,
    #[serde(default)]
    packages: BTreeMap<String, PkgAuditPackage>,
}

impl PkgAuditResponse {
    fn package_count(&self) -> usize {
        self.pkg_count.max(self.packages.len())
    }

    fn issue_count(&self) -> usize {
        self.packages
            .values()
            .map(PkgAuditPackage::issue_count)
            .sum()
    }

    fn summary(&self) -> String {
        let mut package_summaries = self
            .packages
            .iter()
            .take(MAX_SUMMARY_PACKAGES)
            .map(|(name, package)| package.summary(name))
            .collect::<Vec<_>>();

        let remaining = self.packages.len().saturating_sub(MAX_SUMMARY_PACKAGES);
        if remaining > 0 {
            package_summaries.push(format!("and {remaining} more"));
        }

        if package_summaries.is_empty() {
            format!("{} package(s)", self.package_count())
        } else {
            package_summaries.join(", ")
        }
    }
}

#[derive(Deserialize)]
struct PkgAuditPackage {
    version: String,
    #[serde(default)]
    issue_count: usize,
    #[serde(default)]
    issues: Vec<PkgAuditIssue>,
}

impl PkgAuditPackage {
    fn issue_count(&self) -> usize {
        self.issue_count.max(self.issues.len())
    }

    fn summary(&self, name: &str) -> String {
        let cves = self
            .issues
            .iter()
            .flat_map(|issue| issue.cve.iter())
            .take(MAX_CVES_PER_PACKAGE)
            .cloned()
            .collect::<Vec<_>>();

        let remaining = self
            .issues
            .iter()
            .map(|issue| issue.cve.len())
            .sum::<usize>()
            .saturating_sub(MAX_CVES_PER_PACKAGE);

        let cves = match (cves.is_empty(), remaining) {
            (true, _) => self
                .issues
                .first()
                .and_then(|issue| issue.description.as_deref())
                .unwrap_or("unknown issue")
                .to_owned(),
            (false, 0) => cves.join(", "),
            (false, remaining) => format!("{} +{remaining}", cves.join(", ")),
        };

        format!("{name} {} ({cves})", self.version)
    }
}

#[derive(Deserialize)]
struct PkgAuditIssue {
    #[serde(default)]
    cve: Vec<String>,
    #[serde(default)]
    description: Option<String>,
}
