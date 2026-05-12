pub mod safety;

use std::{ffi::OsStr, process::Command, time::Duration};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct CommandRunner {
    default_timeout: Duration,
}

impl CommandRunner {
    pub fn new(default_timeout_seconds: u64) -> Self {
        Self {
            default_timeout: Duration::from_secs(default_timeout_seconds),
        }
    }

    pub fn run<I, S>(&self, program: &str, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_timeout(program, args, self.default_timeout)
    }

    pub fn run_timeout<I, S>(&self, program: &str, args: I, _timeout: Duration) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        // The MVP uses std::process to avoid pulling an async runtime into SSH dump mode.
        // The timeout is kept in the API so the server/task scheduler can switch to tokio later.
        let output = Command::new(program)
            .args(args)
            .env(
                "PATH",
                "/sbin:/bin:/usr/sbin:/usr/bin:/usr/local/sbin:/usr/local/bin",
            )
            .output()
            .with_context(|| format!("failed to execute {program}"))?;
        if !output.status.success() {
            return Ok(String::new());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub fn run_path(&self, program: &std::path::Path, timeout_seconds: u64) -> Result<String> {
        self.run_timeout(
            &program.to_string_lossy(),
            std::iter::empty::<&str>(),
            Duration::from_secs(timeout_seconds),
        )
    }
}
