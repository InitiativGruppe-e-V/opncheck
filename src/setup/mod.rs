mod binary;
mod config;
mod key;
mod packages;
mod plugin;

use std::{
    fs::{self, File},
    io::{self, BufReader, IsTerminal, Read},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

use anyhow::{Context, Result};
use console::{Emoji, Term, style};

use crate::cli::SetupOptions;

const INSTALL_PATH: &str = "/usr/local/bin/opncheck";
const PLUGIN_PATH: &str = "/usr/local/lib/check_mk_agent/plugins/opncheck";
const SSH_DIR: &str = "/root/.ssh";
const AUTHORIZED_KEYS: &str = "/root/.ssh/authorized_keys2";
const CHECKMK_AGENT: &str = "/usr/local/bin/check_mk_agent";

static CHECKMARK: Emoji<'_, '_> = Emoji("✔", "OK");

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum StepStatus {
    Changed,
    Unchanged,
    Skipped,
}

impl StepStatus {
    fn styled(self) -> String {
        match self {
            Self::Changed => style("changed").green().to_string(),
            Self::Unchanged => style("unchanged").dim().to_string(),
            Self::Skipped => style("skipped").yellow().to_string(),
        }
    }
}

pub(super) trait SetupStep {
    const NAME: &'static str;

    fn run(&self) -> Result<StepStatus>;
}

pub fn run(config_path: &Path, options: &SetupOptions) -> Result<()> {
    println!("{}", style("opncheck setup").bold().underlined());
    println!();

    run_step(&binary::BinaryStep)?;
    run_step(&plugin::PluginStep)?;
    run_step(&packages::PackagesStep)?;
    run_step(&key::CheckmkKeyStep::new(options))?;
    run_step(&config::ConfigStep::new(config_path, options))?;

    println!("\n{}", style("Setup completed.").bold().green());
    Ok(())
}

fn run_step<S: SetupStep>(step: &S) -> Result<()> {
    let status = step
        .run()
        .with_context(|| format!("setup step failed: {}", S::NAME))?;

    println!(
        "{} {:<22} {}",
        CHECKMARK,
        style(S::NAME).cyan(),
        status.styled()
    );
    Ok(())
}

pub(super) fn can_prompt() -> bool {
    Term::stdout().is_term() && io::stdin().is_terminal()
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

pub(super) fn paths_are_same_file(left: &Path, right: &Path) -> Result<bool> {
    let left = File::open(left)?;
    let right = File::open(right)?;
    let lmeta = left.metadata()?;
    let rmeta = right.metadata()?;
    let ino_eq = lmeta.ino() == rmeta.ino();
    let dev_eq = lmeta.dev() == rmeta.dev();
    Ok(ino_eq && dev_eq)
}

pub(super) fn files_identical(left: &Path, right: &Path) -> Result<bool> {
    let fa = File::open(left)?;
    let fb = File::open(right)?;

    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }

    let mut ra = BufReader::new(fa);
    let mut rb = BufReader::new(fb);

    let mut ba = [0u8; 8 * 1024];
    let mut bb = [0u8; 8 * 1024];

    loop {
        let na = ra.read(&mut ba)?;
        let nb = rb.read(&mut bb)?;

        if na != nb {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
        if ba[..na] != bb[..nb] {
            return Ok(false);
        }
    }
}
