mod binary;
mod config;
mod key;
mod packages;
mod plugin;

use std::{
    fs,
    io::{self, BufRead, IsTerminal, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::cli::SetupOptions;

const INSTALL_PATH: &str = "/usr/local/bin/opncheck";
const PLUGIN_PATH: &str = "/usr/local/lib/check_mk_agent/plugins/opncheck";
const SSH_DIR: &str = "/root/.ssh";
const AUTHORIZED_KEYS: &str = "/root/.ssh/authorized_keys2";
const CHECKMK_AGENT: &str = "/usr/local/bin/check_mk_agent";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum StepStatus {
    Changed,
    Unchanged,
    Skipped,
}

impl StepStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::Skipped => "skipped",
        }
    }
}

pub(super) trait SetupStep {
    const NAME: &'static str;

    fn run(&self) -> Result<StepStatus>;
}

pub fn run(config_path: &Path, options: SetupOptions) -> Result<()> {
    println!("opncheck setup\n");

    run_step(binary::BinaryStep)?;
    run_step(plugin::PluginStep)?;
    run_step(packages::PackagesStep)?;
    run_step(key::CheckmkKeyStep::new(&options))?;
    run_step(config::ConfigStep::new(config_path, &options))?;

    println!("\nSetup completed.");
    Ok(())
}

fn run_step<S: SetupStep>(step: S) -> Result<()> {
    let status = step
        .run()
        .with_context(|| format!("setup step failed: {}", S::NAME))?;
    println!("  {:<22} {}", S::NAME, status.as_str());
    Ok(())
}

pub(super) fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().ok();

    let mut input = String::new();
    match std::fs::OpenOptions::new().read(true).open("/dev/tty") {
        Ok(tty) => {
            let mut tty = io::BufReader::new(tty);
            tty.read_line(&mut input)
                .context("failed to read setup answer from /dev/tty")?;
        }
        Err(_) => {
            io::stdin()
                .read_line(&mut input)
                .context("failed to read setup answer")?;
        }
    }

    Ok(input)
}

pub(super) fn can_prompt() -> bool {
    Path::new("/dev/tty").exists() || io::stdin().is_terminal()
}

pub(super) fn ensure_mode(path: &Path, mode: u32) -> Result<bool> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.permissions().mode() & 0o777 == mode {
        return Ok(false);
    }

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    Ok(true)
}

pub(super) fn files_have_same_contents(left: &Path, right: &Path) -> Result<bool> {
    if !right.exists() {
        return Ok(false);
    }

    let left = fs::read(left).with_context(|| format!("failed to read {}", left.display()))?;
    let right = fs::read(right).with_context(|| format!("failed to read {}", right.display()))?;
    Ok(left == right)
}

pub(super) fn paths_are_same_file(left: &Path, right: &Path) -> Result<bool> {
    let left = canonicalize_if_exists(left)?;
    let right = canonicalize_if_exists(right)?;
    Ok(matches!((left, right), (Some(left), Some(right)) if left == right))
}

fn canonicalize_if_exists(path: &Path) -> Result<Option<PathBuf>> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(Some(path)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}
