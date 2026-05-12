#!/bin/sh
set -e

REPO="initiativgruppe-e-v/opncheck"
INSTALL_DIR="/usr/local/bin"
PLUGIN_DIR="/usr/local/lib/check_mk_agent/plugins"
CONFIG_DIR="/usr/local/etc"

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

echo "opncheck $TAG installed successfully"
