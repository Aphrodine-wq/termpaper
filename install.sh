#!/usr/bin/env bash
# Install termpaper to ~/.local/bin (and optionally ~/.cargo/bin).
#
# From a clone:
#   ./install.sh
#
# Prebuilt binary (no Rust required):
#   ./install.sh --binary
#
# From GitHub (once published):
#   curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash
#
# Options:
#   TERMPAPER_REPO=https://github.com/you/termpaper.git  override clone URL
#   TERMPAPER_LOCAL_BIN=1                                symlink into ~/.local/bin (default)
#   TERMPAPER_USE_RELEASE=1                              same as --binary
#   TERMPAPER_RELEASE_REPO=Aphrodine-wq/termpaper           GitHub owner/repo for binaries

set -euo pipefail

CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
CARGO_BIN="$CARGO_HOME/bin"
LOCAL_BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
REPO="${TERMPAPER_REPO:-https://github.com/Aphrodine-wq/termpaper.git}"
RELEASE_REPO="${TERMPAPER_RELEASE_REPO:-Aphrodine-wq/termpaper}"
LOCAL_BIN_LINK="${TERMPAPER_LOCAL_BIN:-1}"
USE_RELEASE="${TERMPAPER_USE_RELEASE:-0}"

info() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

for arg in "$@"; do
    case "$arg" in
        --binary | --release) USE_RELEASE=1 ;;
        -h | --help)
            sed -n '2,20p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
    esac
done

need_cargo() {
    if command -v cargo >/dev/null 2>&1; then
        return 0
    fi
    return 1
}

machine_target() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64 | amd64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64 | arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) return 1 ;;
    esac
}

path_hint() {
    local missing=0
    if [[ ":${PATH}:" != *":${LOCAL_BIN}:"* ]]; then
        warn "~/.local/bin is not on your PATH."
        missing=1
    fi
    if [[ -x "$CARGO_BIN/termpaper" && ":${PATH}:" != *":${CARGO_BIN}:"* ]]; then
        warn "~/.cargo/bin is not on your PATH."
        missing=1
    fi
    if [[ "$missing" == 1 ]]; then
        cat >&2 <<EOF
Add one of these to ~/.bashrc, ~/.zshrc, or ~/.profile:

  export PATH="\$HOME/.local/bin:\$PATH"
  export PATH="\$HOME/.cargo/bin:\$PATH"

Then restart your shell and run:  termpaper rain
EOF
    fi
}

install_from_dir() {
    local dir="$1"
    [[ -f "$dir/Cargo.toml" ]] || die "no Cargo.toml in $dir"
    grep -q '^name = "termpaper"' "$dir/Cargo.toml" || die "not the termpaper crate: $dir"
    info "building and installing from $dir"
    (cd "$dir" && cargo install --path . --locked --force)
}


install_from_git() {
    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    info "cloning $REPO"
    git clone --depth 1 "$REPO" "$tmp/repo"
    install_from_dir "$tmp/repo"
}

install_binary() {
    local target url tmp
    target="$(machine_target)" || die "no prebuilt Linux binary for $(uname -m); install Rust from https://rustup.rs and re-run without --binary"
    url="https://github.com/${RELEASE_REPO}/releases/latest/download/termpaper-${target}.tar.gz"
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    info "downloading prebuilt binary ($target)"
    if ! curl -fsSL "$url" -o "$tmp/pkg.tar.gz"; then
        die "could not download $url — publish a GitHub release first, or install without --binary (needs Rust/cargo)"
    fi
    tar -xzf "$tmp/pkg.tar.gz" -C "$tmp"
    [[ -x "$tmp/termpaper" ]] || die "archive did not contain a termpaper binary"
    mkdir -p "$LOCAL_BIN" "$CARGO_BIN"
    install -m755 "$tmp/termpaper" "$LOCAL_BIN/termpaper"
    ln -sf "$LOCAL_BIN/termpaper" "$CARGO_BIN/termpaper"
    info "installed $LOCAL_BIN/termpaper (prebuilt)"
}

link_local_bin() {
    [[ "$LOCAL_BIN_LINK" == "1" ]] || return 0
    [[ -x "$CARGO_BIN/termpaper" ]] || return 0
    mkdir -p "$LOCAL_BIN"
    ln -sf "$CARGO_BIN/termpaper" "$LOCAL_BIN/termpaper"
    info "symlinked $LOCAL_BIN/termpaper -> $CARGO_BIN/termpaper"
}

truecolor_hint() {
    local ct="${COLORTERM:-}"
    if [[ "$ct" != *truecolor* && "$ct" != *24bit* ]]; then
        warn "COLORTERM is '${ct:-unset}' — termpaper works best in a truecolor terminal (kitty, ghostty, alacritty, foot, wezterm)."
        warn "256-color fallback still works; use --no-truecolor to force it."
    fi
}

success_msg() {
    cat <<EOF

termpaper is live. Your terminal just got interesting.

  termpaper rain --fps 120   high-refresh rain on glass
  termpaper --list            the full catalog
  termpaper                   press ? — mixing desk

Tune live:  [ ] fps   , . speed   c color   f filter

Build your own: CONTRIBUTING.md

EOF
}

main() {
    if [[ "$USE_RELEASE" == "1" ]]; then
        install_binary
        truecolor_hint
        path_hint
        success_msg
        return 0
    fi

    if ! need_cargo; then
        warn "cargo not found — trying prebuilt binary instead"
        if install_binary; then
            truecolor_hint
            path_hint
            success_msg
            return 0
        fi
        die "install Rust from https://rustup.rs or use ./install.sh --binary after a GitHub release exists"
    fi

    # shellcheck source=/dev/null
    [[ -f "$CARGO_HOME/env" ]] && source "$CARGO_HOME/env"

    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "$script_dir/Cargo.toml" ]] && grep -q '^name = "termpaper"' "$script_dir/Cargo.toml"; then
        install_from_dir "$script_dir"
    elif command -v git >/dev/null 2>&1; then
        install_from_git
    else
        die "run this script from a termpaper clone, or install git to clone from $REPO"
    fi

    link_local_bin
    truecolor_hint
    path_hint
    success_msg
}

main "$@"
