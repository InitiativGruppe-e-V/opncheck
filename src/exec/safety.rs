use std::{fs, os::unix::fs::MetadataExt, path::Path};

use anyhow::{Context, Result, bail};

pub fn ensure_safe_executable(path: &Path, require_safe_paths: bool) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    if !metadata.file_type().is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if metadata.file_type().is_symlink() {
        bail!("{} is a symlink", path.display());
    }
    if metadata.mode() & 0o111 == 0 {
        bail!("{} is not executable", path.display());
    }
    if require_safe_paths {
        ensure_root_owned_not_writable_by_group_or_other(path, &metadata)?;
        if let Some(parent) = path.parent() {
            let parent_metadata = fs::symlink_metadata(parent)
                .with_context(|| format!("failed to stat {}", parent.display()))?;
            ensure_root_owned_not_writable_by_group_or_other(parent, &parent_metadata)?;
        }
    }
    Ok(())
}

pub fn ensure_safe_regular_file(path: &Path, require_safe_paths: bool) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    if !metadata.file_type().is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if metadata.file_type().is_symlink() {
        bail!("{} is a symlink", path.display());
    }
    if require_safe_paths {
        ensure_root_owned_not_writable_by_group_or_other(path, &metadata)?;
    }
    Ok(())
}

fn ensure_root_owned_not_writable_by_group_or_other(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    if metadata.uid() != 0 {
        bail!("{} is not root-owned", path.display());
    }
    if metadata.mode() & 0o022 != 0 {
        bail!("{} is group/world writable", path.display());
    }
    Ok(())
}
