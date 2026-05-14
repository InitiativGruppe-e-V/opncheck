use super::Check;
use sscanf::sscanf;

use crate::{
    config::Config,
    exec::CommandRunner,
    opnsense::config_xml::OpnsenseConfig,
    plugin::output::{LocalSection, LocalState},
    skip_check,
};

pub struct Wireguard;

impl Check for Wireguard {
    fn name(&self) -> &'static str {
        "wireguard"
    }

    fn run(
        &self,
        config: &Config,
        opnsense_config: &OpnsenseConfig,
        runner: &CommandRunner,
    ) -> anyhow::Result<LocalSection> {
        let mut out = LocalSection::new();
        if !opnsense_config.wireguard_enabled() {
            skip_check!();
        }
        let data = runner
            .run("wg", ["show", "all", "dump"])
            .unwrap_or_default();
        if data.trim().is_empty() {
            skip_check!();
        }
        let warn_secs = config.checks.wireguard.stale_warn_seconds;
        let crit_secs = config.checks.wireguard.stale_crit_seconds;
        let now = epoch_seconds();
        for line in data.lines() {
            let Some(peer) = parse_peer(line) else {
                continue;
            };

            let (state, age_secs, detail) =
                classify_peer(peer.latest_handshake, now, warn_secs, crit_secs);
            let summary = format!("{}: {}{detail}", peer.interface, peer.endpoint);
            let display_name = opnsense_config
                .wireguard_peer_name(peer.public_key)
                .unwrap_or(peer.public_key);
            out.row(state, &format!("WireGuard: {display_name}"), &summary)
                .with_metric("if_in_octets", peer.received.to_string())
                .with_metric("if_out_octets", peer.sent.to_string())
                .with_metric("latest_handshake_age", age_secs.to_string());
        }
        Ok(out)
    }
}

struct WireguardPeerDump<'a> {
    interface: &'a str,
    public_key: &'a str,
    endpoint: &'a str,
    latest_handshake: i64,
    received: u64,
    sent: u64,
}

fn parse_peer(line: &str) -> Option<WireguardPeerDump<'_>> {
    let (interface, public_key, _, endpoint, _, latest_handshake, received, sent, _) = sscanf!(
        line,
        "{str}\t{str}\t{str}\t{str}\t{str}\t{i64}\t{u64}\t{u64}\t{str}"
    )?;

    Some(WireguardPeerDump {
        interface,
        public_key,
        endpoint,
        latest_handshake,
        received,
        sent,
    })
}

fn classify_peer(
    latest_handshake: i64,
    now: i64,
    warn_secs: u64,
    crit_secs: u64,
) -> (LocalState, u64, &'static str) {
    if latest_handshake == 0 {
        return (LocalState::Crit, 0, " (never connected)");
    }
    let age_secs = (now - latest_handshake).max(0) as u64;
    if age_secs > crit_secs {
        (LocalState::Crit, age_secs, "")
    } else if age_secs > warn_secs {
        (LocalState::Warn, age_secs, "")
    } else {
        (LocalState::Ok, age_secs, "")
    }
}

fn epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
