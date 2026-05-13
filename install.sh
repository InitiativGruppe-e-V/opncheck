#!/bin/sh
set -e

REPO="initiativgruppe-e-v/opncheck"

download_stdout() {
    URL="$1"
    if command -v fetch >/dev/null 2>&1; then
        fetch -q -o - "$URL"
    else
        curl -fsSL "$URL"
    fi
}

download_file() {
    URL="$1"
    DEST="$2"
    if command -v fetch >/dev/null 2>&1; then
        fetch -q -o "$DEST" "$URL"
    else
        curl -fsSL "$URL" -o "$DEST"
    fi
}

ARCH=$(uname -m)
if [ "$ARCH" != "amd64" ]; then
    echo "Unsupported architecture: $ARCH (only amd64 is supported)"
    exit 1
fi

echo "Fetching latest release info ..."
RELEASE_JSON=$(download_stdout "https://api.github.com/repos/$REPO/releases/latest")
LATEST=$(echo "$RELEASE_JSON" | sed -n 's/.*"tag_name": "\([^"]*\)".*/\1/p')
if [ -z "$LATEST" ]; then
    echo "Failed to detect latest release"
    exit 1
fi

TAG="${LATEST#v}"
ASSET="opncheck-${TAG}-x86_64-unknown-freebsd"
URL="https://github.com/$REPO/releases/download/${LATEST}/${ASSET}"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading $URL ..."
download_file "$URL" "$TMPDIR/opncheck"
chmod 0755 "$TMPDIR/opncheck"

"$TMPDIR/opncheck" setup
