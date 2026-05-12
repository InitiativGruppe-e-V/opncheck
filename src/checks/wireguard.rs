use super::{Check, utils};
use crate::{
    agent::output::{AgentOutput, LocalState},
    config::Config,
    exec::CommandRunner,
};

pub struct Wireguard;

impl Check for Wireguard {
    fn name(&self) -> &'static str {
        "wireguard"
    }

    fn run(&self, out: &mut AgentOutput, config: &Config, runner: &CommandRunner) {
        let Some(config_xml) = utils::read_opnsense_config() else {
            return;
        };
        if !config_xml.wireguard_enabled() {
            return;
        }
        let data = runner
            .run("wg", ["show", "all", "dump"])
            .unwrap_or_default();
        if data.trim().is_empty() {
            return;
        }
        out.section("local:sep(0)");
        let warn_secs = config.checks.wireguard.stale_warn_seconds;
        let crit_secs = config.checks.wireguard.stale_crit_seconds;
        let now = epoch_seconds();
        for line in data.lines() {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 9 {
                continue;
            }
            let iface = fields[0];
            let peer = fields[1];
            let endpoint = fields[3];
            let latest_handshake: i64 = fields[5].parse().unwrap_or(0);
            let received = fields[6];
            let sent = fields[7];
            let (state, age_secs, detail) =
                classify_peer(latest_handshake, now, warn_secs, crit_secs);
            let metrics = format!(
                "if_in_octets={received}|if_out_octets={sent}|latest_handshake_age={age_secs}"
            );
            let summary = format!("{iface}: {endpoint}{detail}");
            let display_name = config_xml.wireguard_peer_name(peer).unwrap_or(peer);
            out.local(
                state,
                &format!("WireGuard: {display_name}"),
                &metrics,
                &summary,
            );
        }
    }
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
