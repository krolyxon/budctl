#!/bin/sh
# Install budctl and budctl-gui on Linux, or remove them.
#
#   curl -fsSL https://github.com/krolyxon/budctl/releases/latest/download/install.sh | sh
#   ./install.sh              in an unpacked release: installs the binaries next to it
#   ./install.sh --source     in a checkout: builds with cargo, then installs
#   ./install.sh --uninstall
#
# Installs into ~/.local by default (bin/ and share/applications/), so no root is needed.

set -eu

REPO=krolyxon/budctl
PREFIX=$HOME/.local
VERSION=latest
ACTION=install
FROM=auto

usage() {
    cat <<'EOF'
Install budctl (command line) and budctl-gui (desktop app), or remove them.

Usage:
  install.sh [--prefix DIR] [--version X.Y.Z] [--source]
  install.sh --uninstall [--prefix DIR]

Options:
  --prefix DIR     Install into DIR/bin and DIR/share/applications (default: ~/.local)
  --version X.Y.Z  Download this release instead of the latest
  --source         Build from this checkout with cargo instead of downloading
  --uninstall      Remove what install.sh installed. Your config (~/.config/budctl)
                   and log (~/.local/state/budctl) are left alone.
  -h, --help       Show this help
EOF
}

fail() {
    printf 'install.sh: %s\n' "$1" >&2
    exit 1
}

say() {
    printf '%s\n' "$1"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix)
            [ "$#" -ge 2 ] && [ -n "$2" ] || fail "--prefix needs a directory"
            PREFIX=$2
            shift 2
            ;;
        --version)
            [ "$#" -ge 2 ] && [ -n "$2" ] || fail "--version needs a version, like 0.1.1"
            VERSION=${2#v}
            shift 2
            ;;
        --source)
            FROM=source
            shift
            ;;
        --uninstall)
            ACTION=uninstall
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1 (see --help)"
            ;;
    esac
done

[ "$(uname -s)" = Linux ] || fail "budctl runs on Linux only"

BIN_DIR=$PREFIX/bin
APP_DIR=$PREFIX/share/applications

# Use sudo only when the prefix cannot be written as this user (such as /usr/local).
SUDO=
writable_dir=$PREFIX
while [ ! -d "$writable_dir" ]; do
    writable_dir=$(dirname "$writable_dir")
done
if [ ! -w "$writable_dir" ] && [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || fail "$PREFIX is not writable and sudo was not found"
    SUDO=sudo
fi

if [ "$ACTION" = uninstall ]; then
    for file in "$BIN_DIR/budctl" "$BIN_DIR/budctl-gui" "$APP_DIR/budctl.desktop"; do
        if [ -e "$file" ]; then
            $SUDO rm -f "$file"
            say "Removed $file"
        fi
    done
    say "Your config (~/.config/budctl) and log (~/.local/state/budctl) were left alone."
    exit 0
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT INT TERM

# Where the files to install come from: a release unpacked next to this script, this checkout
# built with cargo, or a release downloaded from GitHub.
SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
if [ "$FROM" = auto ]; then
    if [ -f "$SCRIPT_DIR/install.sh" ] && [ -x "$SCRIPT_DIR/budctl" ] && [ -x "$SCRIPT_DIR/budctl-gui" ]; then
        FROM=bundle
    else
        FROM=download
    fi
fi

case "$FROM" in
    bundle)
        FILES=$SCRIPT_DIR
        DESKTOP=$SCRIPT_DIR/budctl.desktop
        ;;
    source)
        [ -f "$SCRIPT_DIR/Cargo.toml" ] || fail "--source needs a budctl checkout next to install.sh"
        command -v cargo >/dev/null 2>&1 || fail "cargo was not found; install Rust from https://rustup.rs/"
        say "Building budctl in $SCRIPT_DIR"
        cargo build --release --locked --manifest-path "$SCRIPT_DIR/Cargo.toml" -p budctl -p budctl-gui
        FILES=$SCRIPT_DIR/target/release
        DESKTOP=$SCRIPT_DIR/contrib/budctl.desktop
        ;;
    download)
        case "$(uname -m)" in
            x86_64 | amd64) TARGET=x86_64-unknown-linux-gnu ;;
            aarch64 | arm64) TARGET=aarch64-unknown-linux-gnu ;;
            *) fail "no release build for $(uname -m); build from a checkout with ./install.sh --source" ;;
        esac
        ARCHIVE=budctl-$TARGET.tar.gz
        if [ "$VERSION" = latest ]; then
            URL=https://github.com/$REPO/releases/latest/download
        else
            URL=https://github.com/$REPO/releases/download/v$VERSION
        fi

        if command -v curl >/dev/null 2>&1; then
            fetch() { curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
        elif command -v wget >/dev/null 2>&1; then
            fetch() { wget -q -O "$2" "$1"; }
        else
            fail "curl or wget is needed to download budctl"
        fi

        say "Downloading $URL/$ARCHIVE"
        fetch "$URL/$ARCHIVE" "$WORK/$ARCHIVE" || fail "download failed: $URL/$ARCHIVE"
        fetch "$URL/$ARCHIVE.sha256" "$WORK/$ARCHIVE.sha256" || fail "download failed: $URL/$ARCHIVE.sha256"
        (cd "$WORK" && sha256sum -c --quiet "$ARCHIVE.sha256") || fail "checksum mismatch for $ARCHIVE"
        tar -xzf "$WORK/$ARCHIVE" -C "$WORK"
        FILES=$WORK/budctl-$TARGET
        DESKTOP=$FILES/budctl.desktop
        ;;
esac

for name in budctl budctl-gui; do
    [ -x "$FILES/$name" ] || fail "$name was not found in $FILES"
done
[ -f "$DESKTOP" ] || fail "budctl.desktop was not found at $DESKTOP"

# The launcher entry runs the binary by its full path, so it works even when the launcher's PATH
# lacks the install directory.
sed "s|^Exec=budctl-gui|Exec=$BIN_DIR/budctl-gui|" "$DESKTOP" >"$WORK/budctl.desktop"

$SUDO install -d "$BIN_DIR" "$APP_DIR"
$SUDO install -m 0755 "$FILES/budctl" "$FILES/budctl-gui" "$BIN_DIR/"
$SUDO install -m 0644 "$WORK/budctl.desktop" "$APP_DIR/budctl.desktop"

say "Installed $("$BIN_DIR/budctl" --version) to $BIN_DIR (budctl, budctl-gui)"
say "Installed the launcher entry to $APP_DIR/budctl.desktop"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say "Note: $BIN_DIR is not on your PATH. Add it to run budctl from a terminal." ;;
esac
