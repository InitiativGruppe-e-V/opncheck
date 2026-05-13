use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufRead, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};

use crate::{config::Config, install};

const INSTALL_PATH: &str = "/usr/local/bin/opncheck";
const PLUGIN_PATH: &str = "/usr/local/lib/check_mk_agent/plugins/opncheck";
const SSH_DIR: &str = "/root/.ssh";
const AUTHORIZED_KEYS: &str = "/root/.ssh/authorized_keys2";
const CHECKMK_AGENT: &str = "/usr/local/bin/check_mk_agent";

pub fn run(config_path: &Path) -> Result<()> {
    let first_install = !Path::new(INSTALL_PATH).exists();

    install_binary()?;
    install_plugin_symlink()?;

    if first_install {
        install_packages()?;
        install_checkmk_key()?;
        let enable_updates =
            prompt_yes_no("Enable opncheck auto-updates during plugin runs? [y/N] ")?;
        install_config(config_path, enable_updates)?;
    } else {
        install_config(config_path, false)?;
    }

    println!("opncheck setup completed");
    Ok(())
}

fn install_binary() -> Result<()> {
    let source = std::env::current_exe().context("failed to locate running opncheck binary")?;
    let destination = Path::new(INSTALL_PATH);

    if paths_are_same_file(&source, destination)? {
        fs::set_permissions(destination, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("failed to set executable mode on {INSTALL_PATH}"))?;
        return Ok(());
    }

    let source_file =
        File::open(&source).with_context(|| format!("failed to open {}", source.display()))?;
    install::replace_with_reader(
        destination,
        source_file,
        "running opncheck binary was empty",
    )
    .with_context(|| format!("failed to install {INSTALL_PATH}"))?;

    Ok(())
}

fn install_plugin_symlink() -> Result<()> {
    let plugin_path = Path::new(PLUGIN_PATH);
    let parent = plugin_path
        .parent()
        .context("plugin path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create plugin directory {}", parent.display()))?;

    match fs::symlink_metadata(plugin_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(plugin_path)
                .with_context(|| format!("failed to replace existing {PLUGIN_PATH}"))?;
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).with_context(|| format!("failed to inspect {PLUGIN_PATH}")),
    }

    std::os::unix::fs::symlink(INSTALL_PATH, plugin_path)
        .with_context(|| format!("failed to create plugin symlink {PLUGIN_PATH}"))
}

fn install_packages() -> Result<()> {
    println!("Installing check_mk_agent and dependencies ...");
    let status = Command::new("pkg")
        .args([
            "install",
            "-y",
            "ipmitool",
            "libstatgrab",
            "bash",
            "wget",
            "check_mk_agent",
        ])
        .status()
        .context("failed to run pkg install")?;

    if !status.success() {
        anyhow::bail!("pkg install failed with status {status}");
    }

    Ok(())
}

fn install_checkmk_key() -> Result<()> {
    fs::create_dir_all(SSH_DIR).with_context(|| format!("failed to create {SSH_DIR}"))?;
    fs::set_permissions(SSH_DIR, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to set permissions on {SSH_DIR}"))?;

    if !Path::new(AUTHORIZED_KEYS).exists() {
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(AUTHORIZED_KEYS)
            .with_context(|| format!("failed to create {AUTHORIZED_KEYS}"))?;
    }
    fs::set_permissions(AUTHORIZED_KEYS, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set permissions on {AUTHORIZED_KEYS}"))?;

    let Some(key) = prompt_checkmk_key()? else {
        return Ok(());
    };

    let existing = fs::read_to_string(AUTHORIZED_KEYS).unwrap_or_default();
    if existing.contains(&key) {
        println!("Key already present in {AUTHORIZED_KEYS}; not appending.");
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .append(true)
        .open(AUTHORIZED_KEYS)
        .with_context(|| format!("failed to open {AUTHORIZED_KEYS}"))?;
    writeln!(file, "command=\"{CHECKMK_AGENT}\" {key}")
        .with_context(|| format!("failed to append Checkmk key to {AUTHORIZED_KEYS}"))?;
    println!("Appended Checkmk key to {AUTHORIZED_KEYS}");

    Ok(())
}

fn prompt_checkmk_key() -> Result<Option<String>> {
    let key = prompt_line("Paste the ssh-ed25519 public key of your Checkmk instance: ")?
        .trim()
        .to_owned();

    if key.is_empty() {
        return Ok(None);
    }

    if !key.starts_with("ssh-ed25519 ") {
        println!("Input does not look like an ssh-ed25519 public key; skipping key install.");
        return Ok(None);
    }

    Ok(Some(key))
}

fn prompt_yes_no(prompt: &str) -> Result<bool> {
    let input = prompt_line(prompt)?;
    Ok(matches!(input.trim(), "y" | "Y" | "yes" | "YES"))
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().ok();

    let mut input = String::new();
    match OpenOptions::new().read(true).open("/dev/tty") {
        Ok(tty) => {
            let mut tty = io::BufReader::new(tty);
            tty.read_line(&mut input)
                .context("failed to read setup answer from /dev/tty")?;
        }
        Err(_) => {
            io::stdin()
                .read_line(&mut input)
                .context("failed to read setup answer")?;
        }
    }

    Ok(input)
}

fn install_config(config_path: &Path, enable_updates: bool) -> Result<()> {
    if config_path.exists() {
        return Ok(());
    }

    let parent = config_path
        .parent()
        .context("config path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;

    let mut config = Config::default();
    config.updates.enabled = enable_updates;
    let config = toml::to_string_pretty(&config).context("failed to serialize config")?;

    fs::write(config_path, config)
        .with_context(|| format!("failed to write config {}", config_path.display()))?;
    fs::set_permissions(config_path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set permissions on {}", config_path.display()))?;

    Ok(())
}

fn paths_are_same_file(left: &Path, right: &Path) -> Result<bool> {
    let left = canonicalize_if_exists(left)?;
    let right = canonicalize_if_exists(right)?;
    Ok(matches!((left, right), (Some(left), Some(right)) if left == right))
}

fn canonicalize_if_exists(path: &Path) -> Result<Option<PathBuf>> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(Some(path)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}
