use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::{cli::SetupOptions, config::Config};

use super::{SetupStep, StepStatus, can_prompt, ensure_mode, prompt_line};

pub(super) struct ConfigStep<'a> {
    config_path: &'a Path,
    options: &'a SetupOptions,
}

impl<'a> ConfigStep<'a> {
    pub(super) fn new(config_path: &'a Path, options: &'a SetupOptions) -> Self {
        Self {
            config_path,
            options,
        }
    }
}

impl SetupStep for ConfigStep<'_> {
    const NAME: &'static str = "config file";

    fn run(&self) -> Result<StepStatus> {
        ensure_config(self.config_path, self.options)
    }
}

fn ensure_config(config_path: &Path, options: &SetupOptions) -> Result<StepStatus> {
    let mut changed = false;
    let mut config = if config_path.exists() {
        let raw = fs::read_to_string(config_path)
            .with_context(|| format!("failed to read config {}", config_path.display()))?;
        toml::from_str(&raw)
            .with_context(|| format!("failed to parse config {}", config_path.display()))?
    } else {
        changed = true;
        let mut config = Config::default();
        if let Some(enabled) = prompted_update_preference(options)? {
            config.updates.enabled = enabled;
        }
        config
    };

    if options.enable_updates && !config.updates.enabled {
        config.updates.enabled = true;
        changed = true;
    }
    if options.disable_updates && config.updates.enabled {
        config.updates.enabled = false;
        changed = true;
    }

    if !config_path.exists() || changed {
        config.save(config_path)?;
    }

    changed |= ensure_mode(config_path, 0o600)
        .with_context(|| format!("failed to set permissions on {}", config_path.display()))?;

    Ok(if changed {
        StepStatus::Changed
    } else {
        StepStatus::Unchanged
    })
}

fn prompted_update_preference(options: &SetupOptions) -> Result<Option<bool>> {
    if options.enable_updates {
        return Ok(Some(true));
    }
    if options.disable_updates {
        return Ok(Some(false));
    }
    if options.yes || !can_prompt() {
        return Ok(None);
    }

    Ok(Some(prompt_yes_no(
        "Enable opncheck auto-updates during plugin runs? [y/N] ",
    )?))
}

fn prompt_yes_no(prompt: &str) -> Result<bool> {
    let input = prompt_line(prompt)?;
    Ok(matches!(input.trim(), "y" | "Y" | "yes" | "YES"))
}
