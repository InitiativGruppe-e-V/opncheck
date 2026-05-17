use std::fs;

use anyhow::{Context, Result};

use crate::{
    platform::{CurrentPlatform, Platform},
    utils::fs::{ensure_mode, files_identical, paths_are_same_file},
};

use super::{SetupStep, StepStatus};

pub(super) struct BinaryStep;

impl SetupStep for BinaryStep {
    const NAME: &'static str = "install binary";

    fn run(&self) -> Result<StepStatus> {
        let source = std::env::current_exe().context("failed to locate running opncheck binary")?;
        let destination = CurrentPlatform::install_path();

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        if destination.exists()
            && (paths_are_same_file(&source, destination)?
                || files_identical(&source, destination)?)
        {
            let changed = ensure_mode(destination, 0o755).with_context(|| {
                format!("failed to set permissions on {}", destination.display())
            })?;
            return Ok(if changed {
                StepStatus::Changed
            } else {
                StepStatus::Unchanged
            });
        }

        fs::copy(source, destination).context("failed to copy binary to target")?;
        ensure_mode(destination, 0o755)
            .with_context(|| format!("failed to set permissions on {}", destination.display()))?;

        Ok(StepStatus::Changed)
    }
}
