#!/bin/bash
# Development script with hot-swap for launchd-managed app
# Kills launchd agent on Rust changes, rebuilds, and restarts automatically

set -e

LABEL="com.nrtfm.cspy"

echo "CSpy hot-swap dev mode — launchd auto-restart on Rust changes"
echo ""

# Kill any existing launchd agent
echo "Stopping launchd agent ($LABEL)..."
launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
sleep 1

# Start Vite frontend dev server in background
echo "Starting Vite (frontend)..."
npm run dev &
VITE_PID=$!

# Give Vite a moment to start
sleep 2

# Function to restart launchd agent
restart_launchd() {
    echo ""
    echo "🔄 Restarting launchd agent..."
    launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
    sleep 1
    PLIST="${HOME}/Library/LaunchAgents/${LABEL}.plist"
    if [[ -f "$PLIST" ]]; then
        launchctl bootstrap "gui/$(id -u)" "$PLIST"
        echo "✅ Agent restarted"
    else
        echo "⚠️ Plist not found at $PLIST"
    fi
}

# Trap to clean up both processes and restart launchd on exit
trap 'kill $VITE_PID 2>/dev/null; restart_launchd; exit' EXIT INT TERM

# Start cargo-watch for Rust backend
# On each rebuild, kill launchd and restart it
cd src-tauri
cargo watch \
  -x 'build --lib' \
  -i '../src/**' \
  -i '../.svelte-kit/**' \
  -w src \
  -s "bash -c 'sleep 1 && launchctl bootout \"gui/\$(id -u)/$LABEL\" 2>/dev/null || true; sleep 2; PLIST=\"\${HOME}/Library/LaunchAgents/$LABEL.plist\"; [[ -f \"\$PLIST\" ]] && launchctl bootstrap \"gui/\$(id -u)\" \"\$PLIST\"; echo \"✅ Hot-swap complete\"'"

# Note: This keeps the script running. Vite runs in background.
# Press Ctrl+C to stop everything and clean up.
