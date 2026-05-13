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
        write_config(config_path, &config)?;
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

fn write_config(config_path: &Path, config: &Config) -> Result<()> {
    let parent = config_path
        .parent()
        .context("config path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;

    let config = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(config_path, config)
        .with_context(|| format!("failed to write config {}", config_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn setup_options() -> SetupOptions {
        SetupOptions {
            yes: true,
            checkmk_key: None,
            enable_updates: false,
            disable_updates: false,
        }
    }

    #[test]
    fn creates_config_with_selected_updates() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("opncheck.toml");
        let options = SetupOptions {
            enable_updates: true,
            ..setup_options()
        };

        let status = ensure_config(&config_path, &options).unwrap();

        assert_eq!(status, StepStatus::Changed);
        let config = Config::load(&config_path).unwrap();
        assert!(config.updates.enabled);
    }

    #[test]
    fn preserves_existing_config_without_explicit_update_choice() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("opncheck.toml");
        let mut config = Config::default();
        config.updates.enabled = true;
        fs::write(&config_path, toml::to_string_pretty(&config).unwrap()).unwrap();
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).unwrap();

        let status = ensure_config(&config_path, &setup_options()).unwrap();

        assert_eq!(status, StepStatus::Unchanged);
        let config = Config::load(&config_path).unwrap();
        assert!(config.updates.enabled);
    }
}
