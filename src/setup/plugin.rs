use std::{fs, io, os::unix::fs::symlink, path::Path};

use anyhow::{Context, Result, bail};

use crate::setup::{INSTALL_PATH, PLUGIN_PATH};

use super::{SetupStep, StepStatus};

pub(super) struct PluginStep;

impl SetupStep for PluginStep {
    const NAME: &'static str = "install plugin link";

    fn run(&self) -> Result<StepStatus> {
        ensure_plugin_symlink(Path::new(PLUGIN_PATH), Path::new(INSTALL_PATH))
    }
}

fn ensure_plugin_symlink(plugin_path: &Path, install_path: &Path) -> Result<StepStatus> {
    let parent = plugin_path
        .parent()
        .context("plugin path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create plugin directory {}", parent.display()))?;

    match fs::symlink_metadata(plugin_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(plugin_path).with_context(|| {
                format!("failed to read plugin symlink {}", plugin_path.display())
            })?;
            if target == install_path {
                return Ok(StepStatus::Unchanged);
            }
            fs::remove_file(plugin_path)
                .with_context(|| format!("failed to replace existing {}", plugin_path.display()))?;
        }
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(plugin_path)
                .with_context(|| format!("failed to replace existing {}", plugin_path.display()))?;
        }
        Ok(_) => bail!(
            "{} exists and is not a file or symlink; remove it before rerunning setup",
            plugin_path.display()
        ),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", plugin_path.display()));
        }
    }

    symlink(install_path, plugin_path)
        .with_context(|| format!("failed to create plugin symlink {}", plugin_path.display()))?;
    Ok(StepStatus::Changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_correct_plugin_symlink_unchanged() {
        let temp_dir = tempfile::tempdir().unwrap();
        let install_path = temp_dir.path().join("bin/opncheck");
        let plugin_path = temp_dir.path().join("plugins/opncheck");
        fs::create_dir_all(install_path.parent().unwrap()).unwrap();
        fs::create_dir_all(plugin_path.parent().unwrap()).unwrap();
        fs::write(&install_path, "bin").unwrap();
        symlink(&install_path, &plugin_path).unwrap();

        let status = ensure_plugin_symlink(&plugin_path, &install_path).unwrap();

        assert_eq!(status, StepStatus::Unchanged);
        assert_eq!(fs::read_link(&plugin_path).unwrap(), install_path);
    }

    #[test]
    fn replaces_wrong_plugin_symlink() {
        let temp_dir = tempfile::tempdir().unwrap();
        let install_path = temp_dir.path().join("bin/opncheck");
        let wrong_path = temp_dir.path().join("bin/wrong");
        let plugin_path = temp_dir.path().join("plugins/opncheck");
        fs::create_dir_all(plugin_path.parent().unwrap()).unwrap();
        symlink(&wrong_path, &plugin_path).unwrap();

        let status = ensure_plugin_symlink(&plugin_path, &install_path).unwrap();

        assert_eq!(status, StepStatus::Changed);
        assert_eq!(fs::read_link(&plugin_path).unwrap(), install_path);
    }

    #[test]
    fn refuses_directory_at_plugin_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let install_path = temp_dir.path().join("bin/opncheck");
        let plugin_path = temp_dir.path().join("plugins/opncheck");
        fs::create_dir_all(&plugin_path).unwrap();

        let err = ensure_plugin_symlink(&plugin_path, &install_path).unwrap_err();

        assert!(err.to_string().contains("is not a file or symlink"));
    }
}
