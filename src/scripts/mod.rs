use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{config::Config, platform::Platform, runner::CommandRunner};

const SCRIPT_DIR: &str = "embedded-scripts";

const BUNDLED_SCRIPTS: &[EmbeddedScript] = &[
    EmbeddedScript::bash("mk_apt", include_str!("bash/mk_apt.sh")),
    EmbeddedScript::python(
        "mk_docker",
        include_str!("python/mk_docker.py"),
        &["docker"],
    ),
];

pub enum EmbeddedScript {
    Bash(BashScript),
    Python(PythonScript),
}

pub struct BashScript {
    pub name: &'static str,
    pub source: &'static str,
}

pub struct PythonScript {
    pub name: &'static str,
    pub source: &'static str,
    pub packages: &'static [&'static str],
}

impl EmbeddedScript {
    pub const fn bash(name: &'static str, source: &'static str) -> Self {
        Self::Bash(BashScript { name, source })
    }

    pub const fn python(
        name: &'static str,
        source: &'static str,
        packages: &'static [&'static str],
    ) -> Self {
        Self::Python(PythonScript {
            name,
            source,
            packages,
        })
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Bash(script) => script.name,
            Self::Python(script) => script.name,
        }
    }
}

pub fn collect<P: Platform>(config: &Config, runner: &CommandRunner) -> Result<String> {
    let mut output = String::new();
    let base_dir = P::state_dir().join(SCRIPT_DIR);

    for script in bundled_scripts() {
        if !config.scripts.enabled.contains(script.name()) {
            continue;
        }

        output.push_str(&run_script(&base_dir, script, runner)?);
    }

    Ok(output)
}

fn bundled_scripts() -> &'static [EmbeddedScript] {
    BUNDLED_SCRIPTS
}

fn run_script(base_dir: &Path, script: &EmbeddedScript, runner: &CommandRunner) -> Result<String> {
    match script {
        EmbeddedScript::Bash(script) => run_bash(base_dir, script, runner),
        EmbeddedScript::Python(script) => run_python(base_dir, script, runner),
    }
}

fn run_bash(base_dir: &Path, script: &BashScript, runner: &CommandRunner) -> Result<String> {
    let path = materialize_script(
        &base_dir.join("bash"),
        script.name,
        "sh",
        script.source,
        0o700,
    )?;

    let output = runner.run_output("bash", [path.as_os_str()])?;
    if !output.success() {
        bail!(
            "embedded bash script {} failed: {}",
            script.name,
            output.stderr().trim()
        );
    }
    Ok(output.stdout().to_owned())
}

fn run_python(base_dir: &Path, script: &PythonScript, runner: &CommandRunner) -> Result<String> {
    let script_path = materialize_script(
        &base_dir.join("python"),
        script.name,
        "py",
        script.source,
        0o600,
    )?;
    let python = ensure_venv(base_dir, script, runner)?;

    let output = runner.run_output(&python.to_string_lossy(), [script_path.as_os_str()])?;
    if !output.success() {
        bail!(
            "embedded python script {} failed: {}",
            script.name,
            output.stderr().trim()
        );
    }
    Ok(output.stdout().to_owned())
}

fn materialize_script(
    dir: &Path,
    name: &str,
    extension: &str,
    source: &str,
    mode: u32,
) -> Result<PathBuf> {
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create script directory {}", dir.display()))?;
    let path = dir.join(format!("{name}.{extension}"));

    if fs::read_to_string(&path).ok().as_deref() != Some(source) {
        fs::write(&path, source)
            .with_context(|| format!("failed to write embedded script {}", path.display()))?;
    }

    fs::set_permissions(&path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    Ok(path)
}

fn ensure_venv(base_dir: &Path, script: &PythonScript, runner: &CommandRunner) -> Result<PathBuf> {
    let venv = base_dir.join("venvs").join(script.name);
    let python = venv.join("bin").join("python");
    let pip = venv.join("bin").join("pip");

    if !python.exists() {
        fs::create_dir_all(
            venv.parent()
                .context("python venv path has no parent directory")?,
        )
        .with_context(|| format!("failed to create venv directory {}", venv.display()))?;
        let output =
            runner.run_output("python3", ["-m", "venv", venv.to_string_lossy().as_ref()])?;
        if !output.success() {
            bail!(
                "failed to create python venv for {}: {}",
                script.name,
                output.stderr().trim()
            );
        }
    }

    let package_state = script.packages.join("\n");
    let package_state_path = venv.join(".opncheck-packages");
    let packages_current =
        fs::read_to_string(&package_state_path).ok().as_deref() == Some(package_state.as_str());

    if !script.packages.is_empty() && !packages_current {
        let mut args = vec!["install"];
        args.extend(script.packages.iter().copied());
        let output = runner.run_output(&pip.to_string_lossy(), args)?;
        if !output.success() {
            bail!(
                "failed to install python packages for {}: {}",
                script.name,
                output.stderr().trim()
            );
        }
        fs::write(&package_state_path, package_state).with_context(|| {
            format!(
                "failed to write python package state {}",
                package_state_path.display()
            )
        })?;
    }

    Ok(python)
}
