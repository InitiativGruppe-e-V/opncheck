use std::{fs::File, path::Path};

use anyhow::{Context, Result};

use crate::{install, setup::INSTALL_PATH};

use super::{SetupStep, StepStatus, ensure_mode, files_have_same_contents, paths_are_same_file};

pub(super) struct BinaryStep;

impl SetupStep for BinaryStep {
    const NAME: &'static str = "install binary";

    fn run(&self) -> Result<StepStatus> {
        let source = std::env::current_exe().context("failed to locate running opncheck binary")?;
        let destination = Path::new(INSTALL_PATH);

        if paths_are_same_file(&source, destination)?
            || files_have_same_contents(&source, destination)?
        {
            let changed = ensure_mode(destination, 0o755)
                .with_context(|| format!("failed to set executable mode on {INSTALL_PATH}"))?;
            return Ok(if changed {
                StepStatus::Changed
            } else {
                StepStatus::Unchanged
            });
        }

        let source_file =
            File::open(&source).with_context(|| format!("failed to open {}", source.display()))?;
        install::replace_with_reader(
            destination,
            source_file,
            "running opncheck binary was empty",
        )
        .with_context(|| format!("failed to install {INSTALL_PATH}"))?;

        Ok(StepStatus::Changed)
    }
}
