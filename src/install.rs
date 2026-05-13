use std::{
    fs,
    io::{self, Read},
    os::unix::fs::PermissionsExt,
    path::Path,
};

use anyhow::{Context, Result, bail};
use tempfile::NamedTempFile;

pub fn replace_with_reader<R>(
    destination: &Path,
    reader: R,
    empty_asset_error: &'static str,
) -> Result<u64>
where
    R: Read,
{
    let destination_dir = destination
        .parent()
        .context("install destination has no parent directory")?;
    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "failed to create install directory {}",
            destination_dir.display()
        )
    })?;

    let mut reader = reader;
    let mut temp_file =
        NamedTempFile::with_prefix_in(".opncheck.", destination_dir).with_context(|| {
            format!(
                "failed to create temporary file in {}",
                destination_dir.display()
            )
        })?;
    let bytes = io::copy(&mut reader, temp_file.as_file_mut())
        .with_context(|| format!("failed to write {}", temp_file.path().display()))?;
    if bytes == 0 {
        bail!("{empty_asset_error}");
    }

    fs::set_permissions(temp_file.path(), fs::Permissions::from_mode(0o755)).with_context(
        || {
            format!(
                "failed to set executable mode on {}",
                temp_file.path().display()
            )
        },
    )?;
    temp_file
        .as_file_mut()
        .sync_all()
        .with_context(|| format!("failed to sync {}", temp_file.path().display()))?;
    temp_file
        .persist(destination)
        .map_err(|err| err.error)
        .with_context(|| {
            format!(
                "failed to replace {} with temporary file",
                destination.display()
            )
        })?;

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_destination_with_reader_contents() {
        let temp_dir = tempfile::tempdir().unwrap();
        let destination = temp_dir.path().join("opncheck");
        fs::write(&destination, "old").unwrap();

        let bytes = replace_with_reader(&destination, "new".as_bytes(), "empty").unwrap();

        assert_eq!(bytes, 3);
        assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
    }

    #[test]
    fn rejects_empty_reader() {
        let temp_dir = tempfile::tempdir().unwrap();
        let destination = temp_dir.path().join("opncheck");

        let err = replace_with_reader(&destination, io::empty(), "empty input").unwrap_err();

        assert!(err.to_string().contains("empty input"));
        assert!(!destination.exists());
    }
}
