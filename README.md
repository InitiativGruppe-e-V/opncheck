# opncheck

`opncheck` adds OPNsense-specific monitoring data to the stock Checkmk
FreeBSD agent.

It is installed as a Checkmk agent plugin on the firewall. The normal FreeBSD
agent still owns the base agent output, SSH transport, plugin execution, local
checks, and spool handling. `opncheck` only emits the OPNsense checks and
sections that Checkmk should consume in addition to the standard FreeBSD data.

Current checks cover firmware information, vulnerable packages, OPNsense
services, gateways, DHCP leases, Unbound, Nginx, and
WireGuard.

## Installation

Run the installer as `root` on the OPNsense host:

```sh
fetch -o install.sh https://raw.githubusercontent.com/initiativgruppe-e-v/opncheck/main/install.sh
sh install.sh
```

The shell installer is intentionally small. It downloads the latest FreeBSD
binary from GitHub and runs `opncheck setup`. The setup command is safe to rerun.
It:

- installs the stock `check_mk_agent` package and its dependencies
- installs or repairs `/usr/local/bin/opncheck`
- installs or repairs the Checkmk plugin symlink
- creates `/root/.ssh/authorized_keys2` if needed and fixes its permissions
- asks for the Checkmk site's `ssh-ed25519` public key
- adds that key with a forced `/usr/local/bin/check_mk_agent` command if missing
- creates a default `/usr/local/etc/opncheck.toml` if needed
- asks whether to enable `opncheck` auto-updates when creating the config

Existing config values are preserved unless an explicit setup flag changes them.
For unattended setup, pass the key and update choice directly:

```sh
opncheck setup --yes --enable-updates --checkmk-key 'ssh-ed25519 AAAA... checkmk-site'
```

Use `--disable-updates` instead of `--enable-updates` to force updates off.

If auto-updates are enabled during first install, plugin execution checks the
latest GitHub release at the configured interval, compares it with
`opncheck --version`, and replaces the binary only when a newer release exists.
The default interval is 6 hours. Update state and failures are reported in the
`OPNCheck Version` local check while normal monitoring output continues.

To test the plugin directly:

```sh
/usr/local/lib/check_mk_agent/plugins/opncheck
```

To test the full Checkmk agent output:

```sh
/usr/local/bin/check_mk_agent
```

## Configuration

The default configuration path is:

```text
/usr/local/etc/opncheck.toml
```

Most installations can start with the generated file. Checks can be disabled
with `checks.skip`, service names can be ignored with `services_ignored`, and
WireGuard stale-handshake thresholds can be adjusted:

```toml
[checks]
skip = []
services_ignored = ["iperf"]
inventory_interval_seconds = 14400

[checks.wireguard]
stale_warn_seconds = 300
stale_crit_seconds = 900

[updates]
enabled = false
interval_seconds = 21600
```

The effective configuration can be inspected with:

```sh
opncheck config
```

## Checkmk Setup

Use SSH to run the stock FreeBSD agent. Do not call `opncheck` directly from
Checkmk.

Create or choose an SSH key inside the Checkmk site and paste its public key
when the installer asks for it. The installer adds it to OPNsense with a forced
command similar to:

```text
command="/usr/local/bin/check_mk_agent" ssh-ed25519 AAAA... checkmk-site
```

In Checkmk, configure the host with:

`Setup > Agents > Other integrations > Custom integrations > Individual program call instead of agent access`

Use a command like:

```sh
ssh -i $OMD_ROOT/.ssh/opncheck_ed25519 -C -T root@$HOSTADDRESS$
```

After saving the rule, run service discovery for the OPNsense host. The normal
FreeBSD services should appear together with the OPNsense-specific local checks
and sections emitted by `opncheck`.
