mod binary;
mod config;
mod key;
mod packages;
mod plugin;

use std::{
    io::{self, IsTerminal},
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
