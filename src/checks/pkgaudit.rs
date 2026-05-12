use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};
use super::Check;

pub struct PkgAudit;

impl Check for PkgAudit {
    fn name(&self) -> &'static str {
        "pkgaudit"
    }

    fn run(&self, out: &mut AgentOutput, _config: &Config, runner: &CommandRunner) {
        let data = runner
            .run("pkg", ["audit", "-F", "--raw=json-compact", "-q"])
            .unwrap_or_default();
        out.section("local:sep(0)");
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
            out.local(LocalState::Ok, "OPNsense Package Audit", "issues=0", "OK");
            return;
        };
        let vulns = json.get("pkg_count").and_then(|v| v.as_u64()).unwrap_or(0);
        if vulns == 0 {
            out.local(LocalState::Ok, "OPNsense Package Audit", "issues=0", "OK");
            return;
        }
        let packages = json
            .get("packages")
            .and_then(|v| v.as_object())
            .map(|packages| packages.keys().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        out.local(
            LocalState::Warn,
            "OPNsense Package Audit",
            &format!("issues={vulns}"),
            &format!("Pkg: {packages} vulnerable"),
        );
    }
}
