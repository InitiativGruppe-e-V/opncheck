mod binary;
mod config;
#[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
mod opnsense;
mod plugin;

use std::{
    io::{self, IsTerminal},
    path::Path,
};

use anyhow::{Context, Result};
use console::{Emoji, Term, style};

use crate::cli::SetupOptions;

static CHECKMARK: Emoji<'_, '_> = Emoji("✔", "OK");

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum StepStatus {
    Changed,
    Unchanged,
    #[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
    Skipped,
}

impl StepStatus {
    fn styled(self) -> String {
        match self {
            Self::Changed => style("changed").green().to_string(),
            Self::Unchanged => style("unchanged").dim().to_string(),
            #[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
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
    #[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
    {
        run_step(&opnsense::PackagesStep)?;
        run_step(&opnsense::CheckmkKeyStep::new(options))?;
    }
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
