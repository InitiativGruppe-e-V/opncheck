use std::{ffi::OsStr, process::Command, time::Duration};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct CommandRunner {
    default_timeout: Duration,
}

#[derive(Debug)]
pub struct CommandOutput {
    stdout: String,
    stderr: String,
    success: bool,
}

impl CommandOutput {
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub fn success(&self) -> bool {
        self.success
    }
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

    pub fn run_timeout<I, S>(&self, program: &str, args: I, timeout: Duration) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.run_timeout_output(program, args, timeout)?;
        if !output.success() {
            return Ok(String::new());
        }
        Ok(output.stdout)
    }

    pub fn run_output<I, S>(&self, program: &str, args: I) -> Result<CommandOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_timeout_output(program, args, self.default_timeout)
    }

    pub fn run_timeout_output<I, S>(
        &self,
        program: &str,
        args: I,
        _timeout: Duration,
    ) -> Result<CommandOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        // The plugin path uses std::process to avoid pulling an async runtime into agent execution.
        // The timeout is kept in the API so the server/task scheduler can switch to tokio later.
        let output = Command::new(program)
            .args(args)
            .env(
                "PATH",
                "/sbin:/bin:/usr/sbin:/usr/bin:/usr/local/sbin:/usr/local/bin",
            )
            .output()
            .with_context(|| format!("failed to execute {program}"))?;
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            success: output.status.success(),
        })
    }

    pub fn run_path(&self, program: &std::path::Path, timeout_seconds: u64) -> Result<String> {
        self.run_timeout(
            &program.to_string_lossy(),
            std::iter::empty::<&str>(),
            Duration::from_secs(timeout_seconds),
        )
    }
}
