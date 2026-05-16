use std::{
    fs::{self},
    path::Path,
};

use anyhow::{Context, Result};

use crate::{
    setup::INSTALL_PATH,
    utils::fs::{ensure_mode, files_identical, paths_are_same_file},
};

use super::{SetupStep, StepStatus};

pub(super) struct BinaryStep;

impl SetupStep for BinaryStep {
    const NAME: &'static str = "install binary";

    fn run(&self) -> Result<StepStatus> {
        let source = std::env::current_exe().context("failed to locate running opncheck binary")?;
        let destination = Path::new(INSTALL_PATH);

        if paths_are_same_file(&source, destination)? || files_identical(&source, destination)? {
            let changed = ensure_mode(destination, 0o755)
                .with_context(|| format!("failed to set permissions on {INSTALL_PATH}"))?;
            return Ok(if changed {
                StepStatus::Changed
            } else {
                StepStatus::Unchanged
            });
        }

        fs::copy(source, destination).context("failed to copy binary to target")?;

        Ok(StepStatus::Changed)
    }
}
