# Checkmk SSH Transport

`opncheck` is built to be queried over SSH instead of exposing TCP port `6556`.

## On OPNsense

Install the compiled binary as root, for example:

```sh
install -o root -g wheel -m 0755 opncheck /usr/local/bin/opncheck
install -o root -g wheel -m 0600 opncheck.example.toml /usr/local/etc/opncheck.toml
```

Create or edit `/root/.ssh/authorized_keys` and restrict the Checkmk public key to agent execution only:

```text
command="/usr/local/bin/opncheck dump",no-port-forwarding,no-X11-forwarding,no-agent-forwarding,no-pty ssh-ed25519 AAAA... checkmk-site
```

Recommended permissions:

```sh
chmod 700 /root/.ssh
chmod 600 /root/.ssh/authorized_keys
```

Do not run a legacy plaintext listener on port `6556` when using SSH transport.

## On The Checkmk Site

Create an SSH key as the site user:

```sh
ssh-keygen -t ed25519 -f ~/.ssh/opncheck_ed25519 -N ''
```

Configure the host with:

`Setup > Agents > Other integrations > Custom integrations > Individual program call instead of agent access`

Command using the host address:

```sh
ssh -i $OMD_ROOT/.ssh/opncheck_ed25519 -C -T root@$HOSTADDRESS$
```

Command using the host name:

```sh
ssh -i $OMD_ROOT/.ssh/opncheck_ed25519 -C -T root@$HOSTNAME$
```

Test from the Checkmk site user:

```sh
ssh -i ~/.ssh/opncheck_ed25519 -C -T root@<opnsense-address>
```

Expected output starts with:

```text
<<<check_mk>>>
AgentOS: OPNsense
Version: ...
```
