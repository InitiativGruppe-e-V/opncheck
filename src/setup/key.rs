use std::{fs, fs::OpenOptions, path::Path};

use anyhow::{Context, Result, bail};
use dialoguer::Input;

use crate::{
    cli::SetupOptions,
    setup::{AUTHORIZED_KEYS, CHECKMK_AGENT, SSH_DIR},
};

use super::{SetupStep, StepStatus, can_prompt, ensure_mode};
use std::io::Write;

pub(super) struct CheckmkKeyStep<'a> {
    options: &'a SetupOptions,
}

impl<'a> CheckmkKeyStep<'a> {
    pub(super) fn new(options: &'a SetupOptions) -> Self {
        Self { options }
    }
}

impl SetupStep for CheckmkKeyStep<'_> {
    const NAME: &'static str = "checkmk ssh key";

    fn run(&self) -> Result<StepStatus> {
        let mut changed = false;
        fs::create_dir_all(SSH_DIR).with_context(|| format!("failed to create {SSH_DIR}"))?;
        changed |= ensure_mode(Path::new(SSH_DIR), 0o700)
            .with_context(|| format!("failed to set permissions on {SSH_DIR}"))?;

        if !Path::new(AUTHORIZED_KEYS).exists() {
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(AUTHORIZED_KEYS)
                .with_context(|| format!("failed to create {AUTHORIZED_KEYS}"))?;
            changed = true;
        }
        changed |= ensure_mode(Path::new(AUTHORIZED_KEYS), 0o600)
            .with_context(|| format!("failed to set permissions on {AUTHORIZED_KEYS}"))?;

        let Some(key) = checkmk_key(self.options)? else {
            return Ok(if changed {
                StepStatus::Changed
            } else {
                StepStatus::Skipped
            });
        };

        changed |= ensure_authorized_key(Path::new(AUTHORIZED_KEYS), CHECKMK_AGENT, &key)?;

        Ok(if changed {
            StepStatus::Changed
        } else {
            StepStatus::Unchanged
        })
    }
}

fn checkmk_key(options: &SetupOptions) -> Result<Option<String>> {
    if let Some(key) = options.checkmk_key.as_deref() {
        return validate_checkmk_key(key);
    }

    if options.yes || !can_prompt() {
        return Ok(None);
    }

    let key: String = Input::new()
        .with_prompt("Paste the ssh-ed25519 public key of your Checkmk instance")
        .allow_empty(true)
        .interact()
        .context("failed to read setup answer")?;
    validate_checkmk_key(key.trim())
}

fn validate_checkmk_key(key: &str) -> Result<Option<String>> {
    let key = key.trim();
    if key.is_empty() {
        return Ok(None);
    }

    if !key.starts_with("ssh-ed25519 ") {
        bail!("Checkmk key must be an ssh-ed25519 public key");
    }

    Ok(Some(key.to_owned()))
}

fn ensure_authorized_key(path: &Path, command: &str, key: &str) -> Result<bool> {
    let entry = forced_command_entry(command, key);
    let existing = fs::read_to_string(path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(false);
    }

    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file)
            .with_context(|| format!("failed to append newline to {}", path.display()))?;
    }
    writeln!(file, "{entry}")
        .with_context(|| format!("failed to append Checkmk key to {}", path.display()))?;

    Ok(true)
}

fn forced_command_entry(command: &str, key: &str) -> String {
    format!("command=\"{command}\" {key}")
}
