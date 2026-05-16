use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use dialoguer::Input;

use crate::{
    cli::SetupOptions,
    setup::{AUTHORIZED_KEYS, CHECKMK_AGENT, SSH_DIR},
    utils::fs::ensure_mode,
};

use super::{SetupStep, StepStatus, can_prompt};

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
        let auth_keys_path = Path::new(AUTHORIZED_KEYS);
        let mut lines = self.read_authorized_keys(auth_keys_path)?;

        let target_idx = lines
            .iter()
            .position(|l| l.contains(CHECKMK_AGENT) && l.contains("ssh-ed25519"));

        if let Some(idx) = target_idx {
            self.handle_existing_key(auth_keys_path, &mut lines, idx)
        } else {
            self.handle_missing_key(auth_keys_path, &mut lines)
        }
    }
}

impl CheckmkKeyStep<'_> {
    fn read_authorized_keys(&self, path: &Path) -> Result<Vec<String>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(content.lines().map(String::from).collect())
    }

    fn format_key_line(&self, key: &str) -> String {
        format!(
            "command=\"{}\",no-pty,no-port-forwarding,no-X11-forwarding,no-agent-forwarding {}",
            CHECKMK_AGENT,
            key.trim()
        )
    }

    fn handle_existing_key(
        &self,
        path: &Path,
        lines: &mut [String],
        idx: usize,
    ) -> Result<StepStatus> {
        let Some(cli_key) = &self.options.checkmk_key else {
            return Ok(StepStatus::Unchanged);
        };

        let new_line = self.format_key_line(cli_key);
        if lines[idx] == new_line {
            let mode_changed = ensure_mode(path, 0o600)?;
            Ok(if mode_changed {
                StepStatus::Changed
            } else {
                StepStatus::Unchanged
            })
        } else {
            lines[idx] = new_line;
            self.write_authorized_keys(path, lines)?;
            Ok(StepStatus::Changed)
        }
    }

    fn handle_missing_key(&self, path: &Path, lines: &mut Vec<String>) -> Result<StepStatus> {
        let Some(key) = get_checkmk_key(self.options)? else {
            return Ok(StepStatus::Skipped);
        };

        let ssh_dir = Path::new(SSH_DIR);
        if !ssh_dir.exists() {
            fs::create_dir_all(ssh_dir)
                .with_context(|| format!("failed to create {}", ssh_dir.display()))?;
            ensure_mode(ssh_dir, 0o700)?;
        }

        lines.push(self.format_key_line(&key));
        self.write_authorized_keys(path, lines)?;
        Ok(StepStatus::Changed)
    }

    fn write_authorized_keys(&self, path: &Path, lines: &[String]) -> Result<()> {
        let mut content = lines.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }
        fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
        ensure_mode(path, 0o600)?;
        Ok(())
    }
}

fn get_checkmk_key(options: &SetupOptions) -> Result<Option<String>> {
    let raw: &str = if let Some(k) = options.checkmk_key.as_deref() {
        k
    } else if options.yes || !can_prompt() {
        return Ok(None);
    } else {
        &Input::<String>::new()
            .with_prompt("Enter the ssh-ed25519 public key of your CheckMK instance")
            .allow_empty(true)
            .interact()
            .context("failed to read setup answer")?
    };

    let key = raw.trim();
    if key.is_empty() {
        return Ok(None);
    }
    if !key.starts_with("ssh-ed25519 ") {
        bail!("Checkmk key must be an ssh-ed25519 public key");
    }
    Ok(Some(key.to_owned()))
}
