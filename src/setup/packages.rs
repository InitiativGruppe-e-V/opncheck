use std::process::Command;

use anyhow::{Context, Result, bail};

use super::{SetupStep, StepStatus};

pub(super) struct PackagesStep;

impl SetupStep for PackagesStep {
    const NAME: &'static str = "install packages";

    fn run(&self) -> Result<StepStatus> {
        let status = Command::new("pkg")
            .args([
                "install",
                "-y",
                "ipmitool",
                "libstatgrab",
                "bash",
                "wget",
                "check_mk_agent",
            ])
            .status()
            .context("failed to run pkg install")?;

        if !status.success() {
            bail!("pkg install failed with status {status}");
        }

        Ok(StepStatus::Changed)
    }
}
