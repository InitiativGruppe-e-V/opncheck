# OPNcheck

A CheckMK agent plugin for OPNsense and Linux hosts.

`opncheck` extends the standard Checkmk agent with local monitoring data. On
OPNsense/FreeBSD it provides the original firewall-specific checks. On Linux it
currently provides platform-neutral checks only.

Supported checks:
- OPNsense x86_64: firmware status, vulnerable packages (`pkg audit`),
  services, gateway health, DHCP lease counts, Unbound, Nginx, WireGuard, and
  Suricata IDS/IPS events
- Linux x86_64: meta/version checks, systemd service status, and Nginx status

## Installation

Download the binary for the current platform and run setup.

```sh
fetch -o /usr/local/bin/opncheck https://github.com/initiativgruppe-e-v/opncheck/releases/latest/download/opncheck-x86_64-unknown-freebsd
chmod +x /usr/local/bin/opncheck
opncheck setup
```

Linux x86_64:

```sh
curl -L -o /usr/local/bin/opncheck https://github.com/initiativgruppe-e-v/opncheck/releases/latest/download/opncheck-x86_64-unknown-linux-gnu
chmod +x /usr/local/bin/opncheck
opncheck setup --yes
```

The `setup` command:
- On OPNsense, installs `check_mk_agent` and required packages (`ipmitool`,
  `libstatgrab`, etc.)
- Configures the plugin symlink in the platform-specific Checkmk plugin path
- On OPNsense, sets up restricted SSH access in `/root/.ssh/authorized_keys2`
- Generates a default configuration in the platform-specific config path
- Optionally configures auto-updates of the binary in regular intervals

Platform paths:

| Platform | Config | Plugin link | State |
| --- | --- | --- | --- |
| OPNsense x86_64 | `/usr/local/etc/opncheck.toml` | `/usr/local/lib/check_mk_agent/plugins/opncheck` | `/var/db/opncheck` |
| Linux x86_64 | `/etc/opncheck.toml` | `/usr/lib/check_mk_agent/plugins/opncheck` | `/var/lib/opncheck` |

For unattended setup:
```sh
opncheck setup --yes --enable-updates --checkmk-key 'ssh-ed25519 AAAA... checkmk-site'
```

## Auto-Updates

If enabled, `opncheck` checks for newer GitHub releases during execution. The check is timestamp-gated to run at most once every 6 hours. When an update is found, the binary for the current supported OS/architecture replaces itself in `/usr/local/bin/opncheck`.

Updates can be checked and applied manually with `opncheck update`.

## Configuration

Configuration is managed in `/usr/local/etc/opncheck.toml` on OPNsense and
`/etc/opncheck.toml` on Linux.

```toml
[checks]
skip = []

[checks.services]
ignored = ["iperf"]

[checks.nginx]
status_socket = "/var/run/nginx_status.sock"
status_urls = [
  "http://127.0.0.1/nginx_status",
  "http://127.0.0.1/status",
  "http://127.0.0.1/vts",
]

[checks.wireguard]
stale_warn_seconds = 300
stale_crit_seconds = 900

[checks.suricata]
log_path = "/var/log/suricata/eve.json"
state_path = "/var/db/opncheck/suricata-state.json"
max_summary_events = 5
include_allowed_in_summary = true
initialize_from_end = true
```

The effective configuration can be inspected with `opncheck config`.

## Checkmk Setup

1. **SSH Access**: Provide your Checkmk site's public SSH key during `opncheck setup`. The installer adds the key to `/root/.ssh/authorized_keys2` with a `command="/usr/local/bin/check_mk_agent"` restriction. This ensures the key can only trigger the agent and not gain a general shell.

2. **Host Configuration**: In Checkmk, configure the OPNsense host with the following settings:
   - **Hostname**: IP Address or Hostname of your OPNSense host.
   - **Checkmk Agent**: "Configured API integrations and Checkmk agent".

3. **Individual Program Call**: Create a rule under `Setup > Agents > Other integrations > Individual program call instead of agent access`:
   - **Command line**: `ssh -T root@$HOSTADDRESS$`
   - **Conditions**: Apply to host or to an entire folder if you prefer.

4. **SSH Host Key Verification**: Make a connection attempt at least once from your CheckMK environment to the SSH host: `ssh root@YOUR_OPNSENSE_IP` and confirm the host key. 

4. **Service Discovery**: Run a service discovery. Standard agent services (CPU, Memory, Interfaces) will be combined with OPNsense-specific checks provided by `opncheck`.
