#!/bin/sh
set -e

REPO="initiativgruppe-e-v/opncheck"
INSTALL_DIR="/usr/local/bin"
PLUGIN_DIR="/usr/local/lib/check_mk_agent/plugins"
CONFIG_DIR="/usr/local/etc"
PERIODIC_DIR="/usr/local/etc/periodic/daily"
UPDATE_SCRIPT="$PERIODIC_DIR/810.opncheck-update"
SSH_DIR="/root/.ssh"
AUTHORIZED_KEYS="$SSH_DIR/authorized_keys2"
ENABLE_AUTO_UPDATE="no"

install_auto_update() {
    mkdir -p "$PERIODIC_DIR"
    cat > "$UPDATE_SCRIPT" <<'EOF'
#!/bin/sh
set -e

REPO="initiativgruppe-e-v/opncheck"
INSTALL_DIR="/usr/local/bin"
PLUGIN_DIR="/usr/local/lib/check_mk_agent/plugins"

ARCH=$(uname -m)
if [ "$ARCH" != "amd64" ]; then
    echo "Unsupported architecture: $ARCH (only amd64 is supported)"
    exit 1
fi

if [ ! -x "$INSTALL_DIR/opncheck" ]; then
    echo "$INSTALL_DIR/opncheck is not installed"
    exit 1
fi

VERSION_OUTPUT=$("$INSTALL_DIR/opncheck" --version)
set -- $VERSION_OUTPUT
CURRENT="$2"
if [ -z "$CURRENT" ]; then
    echo "Failed to detect installed opncheck version"
    exit 1
fi

RELEASE_JSON=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest")
LATEST=$(echo "$RELEASE_JSON" | sed -n 's/.*"tag_name": "\([^"]*\)".*/\1/p')
if [ -z "$LATEST" ]; then
    echo "Failed to detect latest release"
    exit 1
fi

TAG="${LATEST#v}"
COMPARE=$(pkg version -t "$CURRENT" "$TAG")
if [ "$COMPARE" != "<" ]; then
    exit 0
fi

ASSET="opncheck-${TAG}-x86_64-unknown-freebsd.tar.gz"
URL="https://github.com/$REPO/releases/download/${LATEST}/${ASSET}"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Updating opncheck from $CURRENT to $TAG ..."
curl -fsSL "$URL" -o "$TMPDIR/$ASSET"
tar -xzf "$TMPDIR/$ASSET" -C "$TMPDIR"

cp "$TMPDIR/opncheck" "$INSTALL_DIR/opncheck"
chmod 0755 "$INSTALL_DIR/opncheck"

mkdir -p "$PLUGIN_DIR" || true
ln -sf "$INSTALL_DIR/opncheck" "$PLUGIN_DIR/opncheck"

echo "opncheck $TAG installed successfully"
EOF
    chmod 0755 "$UPDATE_SCRIPT"
    echo "Installed daily auto-update script to $UPDATE_SCRIPT"
}

ARCH=$(uname -m)
if [ "$ARCH" != "amd64" ]; then
    echo "Unsupported architecture: $ARCH (only amd64 is supported)"
    exit 1
fi

echo "Fetching latest release info ..."
RELEASE_JSON=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest")

LATEST=$(echo "$RELEASE_JSON" | sed -n 's/.*"tag_name": "\([^"]*\)".*/\1/p')
if [ -z "$LATEST" ]; then
    echo "Failed to detect latest release"
    exit 1
fi

TAG="${LATEST#v}"
ASSET="opncheck-${TAG}-x86_64-unknown-freebsd.tar.gz"
URL="https://github.com/$REPO/releases/download/${LATEST}/${ASSET}"

if [ ! -f "$INSTALL_DIR/opncheck" ]; then
    echo "First-time install detected; performing one-time setup ..."

    echo "Installing check_mk_agent and dependencies ..."
    pkg install -y ipmitool libstatgrab bash wget check_mk_agent

    if [ ! -d "$SSH_DIR" ]; then
        mkdir -p "$SSH_DIR"
        chmod 700 "$SSH_DIR"
    fi
    if [ ! -f "$AUTHORIZED_KEYS" ]; then
        : > "$AUTHORIZED_KEYS"
        chmod 600 "$AUTHORIZED_KEYS"
    fi

    if [ -r /dev/tty ]; then
        KEY_INPUT_FD=/dev/tty
    else
        KEY_INPUT_FD=/dev/stdin
    fi
    printf "Paste the ssh-ed25519 public key of your Checkmk instance: " > /dev/tty 2>/dev/null || \
        printf "Paste the ssh-ed25519 public key of your Checkmk instance: "
    IFS= read -r CMK_PUBKEY < "$KEY_INPUT_FD" || CMK_PUBKEY=""

    case "$CMK_PUBKEY" in
        "ssh-ed25519 "*) ;;
        *)
            echo "Input does not look like an ssh-ed25519 public key; skipping key install."
            CMK_PUBKEY=""
            ;;
    esac

    if [ -n "$CMK_PUBKEY" ]; then
        if grep -qF "$CMK_PUBKEY" "$AUTHORIZED_KEYS" 2>/dev/null; then
            echo "Key already present in $AUTHORIZED_KEYS; not appending."
        else
            printf 'command="/usr/local/bin/check_mk_agent" %s\n' "$CMK_PUBKEY" >> "$AUTHORIZED_KEYS"
            echo "Appended Checkmk key to $AUTHORIZED_KEYS"
        fi
    fi

    printf "Enable daily opncheck auto-updates? [y/N] " > /dev/tty 2>/dev/null || \
        printf "Enable daily opncheck auto-updates? [y/N] "
    IFS= read -r ENABLE_AUTO_UPDATE < "$KEY_INPUT_FD" || ENABLE_AUTO_UPDATE=""
    case "$ENABLE_AUTO_UPDATE" in
        y|Y|yes|YES)
            ENABLE_AUTO_UPDATE="yes"
            ;;
        *)
            ENABLE_AUTO_UPDATE="no"
            echo "Daily opncheck auto-updates disabled."
            ;;
    esac
fi

mkdir -p "$PLUGIN_DIR" || true

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading $URL ..."
curl -fsSL "$URL" -o "$TMPDIR/$ASSET"

echo "Extracting ..."
tar -xzf "$TMPDIR/$ASSET" -C "$TMPDIR"

echo "Installing binary to $INSTALL_DIR ..."
cp "$TMPDIR/opncheck" "$INSTALL_DIR/opncheck"
chmod 0755 "$INSTALL_DIR/opncheck"

echo "Creating plugin symlink ..."
ln -sf "$INSTALL_DIR/opncheck" "$PLUGIN_DIR/opncheck"

if [ ! -f "$CONFIG_DIR/opncheck.toml" ]; then
    echo "Installing example configuration ..."
    cp "$TMPDIR/opncheck.example.toml" "$CONFIG_DIR/opncheck.toml"
    chmod 0600 "$CONFIG_DIR/opncheck.toml"
fi

if [ "$ENABLE_AUTO_UPDATE" = "yes" ]; then
    install_auto_update
fi

echo "opncheck $TAG installed successfully"
