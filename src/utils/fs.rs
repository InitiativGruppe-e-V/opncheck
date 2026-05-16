use std::{
    fs::{self, File},
    io::{BufReader, Read},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

use anyhow::{Context, Result};

pub fn ensure_mode(path: &Path, mode: u32) -> Result<bool> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.permissions().mode() & 0o777 == mode {
        return Ok(false);
    }

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    Ok(true)
}

pub fn paths_are_same_file(left: &Path, right: &Path) -> Result<bool> {
    let left = File::open(left)?;
    let right = File::open(right)?;
    let lmeta = left.metadata()?;
    let rmeta = right.metadata()?;
    let ino_eq = lmeta.ino() == rmeta.ino();
    let dev_eq = lmeta.dev() == rmeta.dev();
    Ok(ino_eq && dev_eq)
}

pub fn files_identical(left: &Path, right: &Path) -> Result<bool> {
    let fa = File::open(left)?;
    let fb = File::open(right)?;

    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }

    let mut ra = BufReader::new(fa);
    let mut rb = BufReader::new(fb);

    let mut ba = [0u8; 8 * 1024];
    let mut bb = [0u8; 8 * 1024];

    loop {
        let na = ra.read(&mut ba)?;
        let nb = rb.read(&mut bb)?;

        if na != nb {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
        if ba[..na] != bb[..nb] {
            return Ok(false);
        }
    }
}
