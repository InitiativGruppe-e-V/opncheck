use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::xml::{self, OpnsenseConfig};

#[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
pub type CurrentPlatform = OPNSenseX64;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub type CurrentPlatform = LinuxX64;

#[cfg(not(any(
    all(target_os = "freebsd", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64")
)))]
compile_error!("opncheck only supports x86_64 Linux and x86_64 OPNsense targets");

pub trait Platform: Sized + Copy + 'static {
    type PlatformData;

    fn name() -> &'static str;
    fn release_target() -> &'static str;
    fn install_path() -> &'static Path;
    fn plugin_path() -> &'static Path;
    fn config_path() -> PathBuf;
    fn state_dir() -> &'static Path;
    fn checkmk_agent_path() -> &'static Path;
    fn platform_data() -> Result<Self::PlatformData>;
}

#[derive(Debug, Clone, Copy)]
pub struct LinuxX64;

#[derive(Debug, Clone, Copy)]
pub struct OPNSenseX64;

pub struct OPNSensePlatformData {
    pub opnsense_config: OpnsenseConfig,
}

impl Platform for LinuxX64 {
    type PlatformData = ();

    fn name() -> &'static str {
        "linux-x86_64"
    }

    fn release_target() -> &'static str {
        "x86_64-unknown-linux-gnu"
    }

    fn install_path() -> &'static Path {
        Path::new("/usr/local/bin/opncheck")
    }

    fn plugin_path() -> &'static Path {
        Path::new("/usr/lib/check_mk_agent/plugins/opncheck")
    }

    fn config_path() -> PathBuf {
        PathBuf::from("/etc/opncheck.toml")
    }

    fn state_dir() -> &'static Path {
        Path::new("/var/lib/opncheck")
    }

    fn checkmk_agent_path() -> &'static Path {
        Path::new("/usr/bin/check_mk_agent")
    }

    fn platform_data() -> Result<Self::PlatformData> {
        Ok(())
    }
}

impl Platform for OPNSenseX64 {
    type PlatformData = OPNSensePlatformData;

    fn name() -> &'static str {
        "opnsense-x86_64"
    }

    fn release_target() -> &'static str {
        "x86_64-unknown-freebsd"
    }

    fn install_path() -> &'static Path {
        Path::new("/usr/local/bin/opncheck")
    }

    fn plugin_path() -> &'static Path {
        Path::new("/usr/local/lib/check_mk_agent/plugins/opncheck")
    }

    fn config_path() -> PathBuf {
        PathBuf::from("/usr/local/etc/opncheck.toml")
    }

    fn state_dir() -> &'static Path {
        Path::new("/var/db/opncheck")
    }

    fn checkmk_agent_path() -> &'static Path {
        Path::new("/usr/local/bin/check_mk_agent")
    }

    fn platform_data() -> Result<Self::PlatformData> {
        Ok(OPNSensePlatformData {
            opnsense_config: xml::read_config(Path::new("/conf/config.xml"))?,
        })
    }
}
