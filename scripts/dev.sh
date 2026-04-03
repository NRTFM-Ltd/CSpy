#!/usr/bin/env bash
# dev.sh — hot-swap dev mode for launchd-managed Tauri apps
# Called by Tauri's beforeDevCommand from the project root.
set -euo pipefail

LABEL="com.nrtfm.cspy"
PLIST="${HOME}/Library/LaunchAgents/${LABEL}.plist"

# Ensure cargo-watch is installed (mandatory for FSEvents-based watching)
if ! cargo watch --version &>/dev/null 2>&1; then
    echo "cargo-watch not found — installing (one-time)..."
    cargo install cargo-watch
fi

# Kill any process already holding port 1420 (leftover Vite instances)
if lsof -ti:1420 &>/dev/null 2>&1; then
    echo "Killing stale process on port 1420..."
    kill "$(lsof -ti:1420)" 2>/dev/null || true
    sleep 1
fi

# Stop the production launchd agent
echo "Stopping launchd agent (${LABEL})..."
launchctl bootout "gui/$(id -u)/${LABEL}" 2>/dev/null || true
sleep 1

# Restore production agent on exit/interrupt
restore_prod() {
    echo "Restoring production agent..."
    launchctl bootout "gui/$(id -u)/${LABEL}" 2>/dev/null || true
    sleep 1
    [[ -f "${PLIST}" ]] && launchctl bootstrap "gui/$(id -u)" "${PLIST}" && echo "Production agent restored."
}
trap 'kill "${VITE_PID}" 2>/dev/null; restore_prod; exit' EXIT INT TERM

# Start Vite dev server in background
echo "Starting Vite..."
npm run dev &
VITE_PID=$!
sleep 2

# cargo-watch: rebuild lib on Rust source changes, then hot-swap launchd
echo "Watching for Rust changes (cargo-watch)..."
cd src-tauri
cargo watch \
    -x 'build --lib' \
    -i '../src/**' \
    -i '../.svelte-kit/**' \
    -w src \
    -s "bash -c '
        sleep 1
        launchctl bootout \"gui/\$(id -u)/${LABEL}\" 2>/dev/null || true
        sleep 2
        [[ -f \"${PLIST}\" ]] && launchctl bootstrap \"gui/\$(id -u)\" \"${PLIST}\" && echo \"Hot-swap complete\"
    '"
