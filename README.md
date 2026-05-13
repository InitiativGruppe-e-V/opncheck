# OPNcheck

A CheckMK agent plugin for OPNsense.

`opncheck` extends the standard Checkmk FreeBSD agent with OPNsense-specific monitoring data. It is intended to be executed as a plugin by the `check_mk_agent`.

Supported checks:
- Firmware status and vulnerable packages (pkg audit)
- Service status
- Gateway health and DHCP lease counts
- Unbound and Nginx status
- WireGuard peers and handshake staleness

## Installation

Download the binary for FreeBSD x86_64 and run the interactive setup:

```sh
fetch -o /usr/local/bin/opncheck https://github.com/initiativgruppe-e-v/opncheck/releases/latest/download/opncheck-x86_64-unknown-freebsd
chmod +x /usr/local/bin/opncheck
opncheck setup
```

The `setup` command:
- Installs `check_mk_agent` and required packages (`ipmitool`, `libstatgrab`, etc.)
- Configures the plugin symlink in `/usr/local/lib/check_mk_agent/plugins/`
- Sets up restricted SSH access in `/root/.ssh/authorized_keys2`
- Generates a default configuration in `/usr/local/etc/opncheck.toml`

For unattended setup:
```sh
opncheck setup --yes --enable-updates --checkmk-key 'ssh-ed25519 AAAA... checkmk-site'
```

## Auto-Updates

If enabled, `opncheck` checks for newer GitHub releases during execution. The check is timestamp-gated to run at most once every 6 hours. When an update is found, the binary replaces itself in `/usr/local/bin/opncheck`.

Updates can be checked and applied manually with `opncheck update`.

## Configuration

Configuration is managed in `/usr/local/etc/opncheck.toml`.

```toml
[checks]
skip = []

[checks.services]
ignored = ["iperf"]

[checks.wireguard]
stale_warn_seconds = 300
stale_crit_seconds = 900
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

4. **Service Discovery**: Run a service discovery. Standard FreeBSD services (CPU, Memory, Interfaces) will be combined with OPNsense-specific checks provided by `opncheck`.
