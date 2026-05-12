use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    agent::output::AgentOutput,
    config::Config,
    exec::{CommandRunner, safety},
};

pub fn collect_plugins(out: &mut AgentOutput, config: &Config, runner: &CommandRunner) {
    collect_executables(out, &config.paths.plugins, config, runner, false);
}

pub fn collect_local(out: &mut AgentOutput, config: &Config, runner: &CommandRunner) {
    out.section("local:sep(0)");
    collect_executables(out, &config.paths.local, config, runner, true);
}

pub fn collect_spool(out: &mut AgentOutput, config: &Config) {
    let dir = &config.paths.spool;
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let now = epoch_seconds();
    for entry in entries.flatten() {
        let path = entry.path();
        if safety::ensure_safe_regular_file(&path, config.security.require_safe_paths).is_err() {
            continue;
        }
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if metadata.size() > config.security.max_spool_file_bytes {
            continue;
        }
        if spool_file_expired(&path, &metadata, now) {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&path) {
            out.raw_block(content.trim_end());
        }
    }
}

fn collect_executables(
    out: &mut AgentOutput,
    root: &Path,
    config: &Config,
    runner: &CommandRunner,
    is_local: bool,
) {
    let mut files = Vec::new();
    collect_files(root, &mut files, 0);
    files.sort();

    for path in files {
        if safety::ensure_safe_executable(&path, config.security.require_safe_paths).is_err() {
            continue;
        }
        let cachetime = cachetime_from_parent(&path);
        let data = runner
            .run_path(&path, config.security.plugin_timeout_seconds)
            .unwrap_or_default();
        if data.trim().is_empty() {
            continue;
        }
        if cachetime > 0 {
            let created = epoch_seconds();
            if is_local {
                for line in data.lines().filter(|line| !line.trim().is_empty()) {
                    out.line(format!("cached({created},{cachetime}) {line}"));
                }
            } else {
                append_cached_plugin_output(out, &data, created, cachetime);
            }
        } else {
            out.raw_block(data.trim_end());
        }
    }
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_files(&path, files, depth + 1);
        } else if metadata.is_file() {
            files.push(path);
        }
    }
}

fn append_cached_plugin_output(out: &mut AgentOutput, data: &str, created: u64, cachetime: u64) {
    for line in data.lines() {
        if let Some(section) = line
            .strip_prefix("<<<")
            .and_then(|line| line.strip_suffix(">>>"))
        {
            out.section_cached(section, created as i64, cachetime);
        } else {
            out.line(line.to_owned());
        }
    }
}

fn cachetime_from_parent(path: &Path) -> u64 {
    path.parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .and_then(|name| name.parse::<u64>().ok())
        .unwrap_or(0)
}

fn spool_file_expired(path: &Path, metadata: &fs::Metadata, now: u64) -> bool {
    let Some(max_age) = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.split_once('_'))
        .and_then(|(prefix, _)| prefix.parse::<u64>().ok())
    else {
        return false;
    };
    let modified = metadata
        .modified()
        .ok()
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    now.saturating_sub(modified) > max_age
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_directory_number_sets_cachetime() {
        assert_eq!(cachetime_from_parent(Path::new("/x/local/600/check")), 600);
        assert_eq!(cachetime_from_parent(Path::new("/x/local/check")), 0);
    }
}
