use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use console::style;

use super::{SetupStep, StepStatus};

pub(in crate::setup) struct PackagesStep;

impl SetupStep for PackagesStep {
    const NAME: &'static str = "install packages";

    fn run(&self) -> Result<StepStatus> {
        let mut child = Command::new("pkg")
            .args([
                "install",
                "-y",
                "ipmitool",
                "libstatgrab",
                "bash",
                "wget",
                "check_mk_agent",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn pkg install")?;

        let stdout = child.stdout.take().context("failed to capture stdout")?;
        let stderr = child.stderr.take().context("failed to capture stderr")?;

        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let mut nothing_to_do = false;

        // Print stdout lines indented and gray
        for line in stdout_reader.lines().map_while(Result::ok) {
            if line.contains("The most recent versions of packages are already installed") {
                nothing_to_do = true;
            }
            println!("    {}", style(line).dim());
        }

        // Print stderr lines indented and gray
        for line in stderr_reader.lines().map_while(Result::ok) {
            eprintln!("    {}", style(line).dim());
        }

        let status = child.wait().context("failed to wait for pkg install")?;

        if !status.success() {
            bail!("pkg install failed with status {status}");
        }

        let status = if nothing_to_do {
            StepStatus::Unchanged
        } else {
            StepStatus::Changed
        };

        Ok(status)
    }
}
