use serde::{Deserialize, de::IgnoredAny};

use super::Check;
use crate::{
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml::OpnsenseConfig,
    plugin::output::{LocalSection, LocalState},
};

pub struct Firmware;

impl Check for Firmware {
    fn name(&self) -> &'static str {
        "firmware"
    }

    fn run(
        &self,
        _config: &Config,
        _opnsense_config: &OpnsenseConfig,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let mut out = LocalSection::new();

        let response = runner.run("configctl", ["firmware", "product"])?;

        let (state, summary, updates_metric) =
            if let Ok(product) = serde_json::from_str::<Product>(&response) {
                let version = product.product_version;
                let updates = product.product_check.upgrade_packages.len();

                let state = if updates == 0 {
                    LocalState::Ok
                } else {
                    LocalState::Warn
                };
                let summary = if updates == 0 {
                    format!("Version {version}, up to date")
                } else {
                    format!("Version {version}, {updates} update(s) available")
                };
                (state, summary, Some(updates.to_string()))
            } else {
                let version_response = runner.run("opnsense-version", ["-v"])?;
                let version = version_response.trim();
                (
                    LocalState::Unknown,
                    format!("Version {version}, no update information available"),
                    None,
                )
            };

        let row = out.row(state, "OPNsense Firmware", &summary);
        if let Some(updates) = updates_metric {
            row.with_metric("updates", updates);
        }

        Ok(out)
    }
}

#[derive(Deserialize)]
struct Product {
    product_version: String,
    product_check: ProductCheck,
}

#[derive(Deserialize)]
struct ProductCheck {
    upgrade_packages: Vec<IgnoredAny>,
}
