#!/usr/bin/env bash
# What a scene costs the terminal: bytes written per second and CPU used.
# Runs in a private tmux server (213x56 cells, about a 1080p kitty) under an
# isolated link group and scratch XDG dirs, with the Hyprland/kitty handles
# unset, so it never touches a real window or a real termpaper instance.
#
#   scripts/term_cost.sh <binary> <secs> <scene> [extra termpaper args...]
set -euo pipefail
bin=$(realpath "$1"); secs=$2; scene=$3; shift 3
sock=tpbench-$$
scratch=$(mktemp -d -p "${TMPDIR:-/tmp}")
trap 'tmux -L "$sock" kill-server 2>/dev/null || true; rm -rf "$scratch"' EXIT
mkdir -p "$scratch"/{config/termpaper,state,run}
chmod 700 "$scratch/run"
# a config file means no first-run welcome; fullscreen off
printf '[display]\nfullscreen = false\n' > "$scratch/config/termpaper/config.toml"

# TERM_PROGRAM=kitty with TMUX unset: measure what a kitty pane would get,
# not tmux's slower defaults
tmux -L "$sock" -f /dev/null new-session -d -x 213 -y 56 -s b \
  env -u TMUX -u HYPRLAND_INSTANCE_SIGNATURE -u KITTY_WINDOW_ID -u KITTY_LISTEN_ON -u WAYLAND_DISPLAY \
  TERM=xterm-256color COLORTERM=truecolor TERM_PROGRAM=kitty \
  XDG_CONFIG_HOME="$scratch/config" XDG_STATE_HOME="$scratch/state" XDG_RUNTIME_DIR="$scratch/run" \
  "$bin" --group perfbench "$scene" "$@"
tmux -L "$sock" pipe-pane -t b -o "cat > '$scratch/out'"
pid=$(tmux -L "$sock" display -p -t b '#{pane_pid}')
# skip the first two seconds (startup, shader compile, warm-up)
sleep 2
# measure growth: truncating under a running `cat` would leave a hole
b0=$(stat -c %s "$scratch/out")
cpu0=$(awk '{ print $14 + $15 }' "/proc/$pid/stat")
sleep "$secs"
cpu1=$(awk '{ print $14 + $15 }' "/proc/$pid/stat")
bytes=$(( $(stat -c %s "$scratch/out") - b0 ))
hz=$(getconf CLK_TCK)
awk -v b="$bytes" -v s="$secs" -v c=$((cpu1 - cpu0)) -v hz="$hz" -v n="$scene" \
  'BEGIN { printf "%-14s %8.1f KB/s   cpu %5.1f%%\n", n, b / s / 1024, c / hz / s * 100 }'
