# Checkmk FreeBSD Agent Plugin

`opncheck` is a plugin for the stock Checkmk FreeBSD agent. The FreeBSD agent owns the base agent output, SSH transport, plugin execution, local checks, and spool handling. `opncheck` only emits OPNsense-specific local checks and custom sections.

## Install On OPNsense

Install the stock Checkmk FreeBSD agent first. The default plugin directory is:

```text
/usr/local/lib/check_mk_agent/plugins
```

Build and install `opncheck`:

```sh
cargo build --release
install -o root -g wheel -m 0755 target/release/opncheck /usr/local/lib/check_mk_agent/plugins/opncheck
install -o root -g wheel -m 0600 opncheck.example.toml /usr/local/etc/opncheck.toml
```

Test the plugin directly:

```sh
/usr/local/lib/check_mk_agent/plugins/opncheck
```

Test through the FreeBSD agent:

```sh
/usr/local/bin/check_mk_agent
```

The plugin must not emit stock FreeBSD agent sections such as `<<<check_mk>>>`, `<<<df>>>`, `<<<cpu>>>`, `<<<ps>>>`, `<<<zfsget>>>`, or `<<<zpool_status>>>`.

## SSH Transport

Use SSH to run the stock FreeBSD agent, not `opncheck` directly.

Restricted key on OPNsense:

```text
command="/usr/local/bin/check_mk_agent",no-port-forwarding,no-X11-forwarding,no-agent-forwarding,no-pty ssh-ed25519 AAAA... checkmk-site
```

Checkmk rule:

`Setup > Agents > Other integrations > Custom integrations > Individual program call instead of agent access`

Command:

```sh
ssh -i $OMD_ROOT/.ssh/opncheck_ed25519 -C -T root@$HOSTADDRESS$
```

Do not run a plaintext listener on port `6556` when using SSH transport.
