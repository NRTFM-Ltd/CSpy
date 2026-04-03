#!/usr/bin/env bash
# dev.sh — prepare environment for Tauri dev mode
# Called by Tauri as beforeDevCommand from the project root.
# Starts Vite in the background; cargo tauri dev handles Rust watching.
set -euo pipefail

LABEL="com.nrtfm.cspy"

# Kill any running CSpy instances to avoid duplicate tray icons
pkill -f "Contents/MacOS/CSpy" 2>/dev/null || true

# Stop the production launchd agent
echo "Stopping launchd agent (${LABEL})..."
launchctl bootout "gui/$(id -u)/${LABEL}" 2>/dev/null || true

# Release port 1420 if held by a leftover Vite instance
if lsof -ti:1420 &>/dev/null 2>&1; then
    echo "Releasing port 1420..."
    kill "$(lsof -ti:1420)" 2>/dev/null || true
    sleep 1
fi

# Start Vite dev server in background — cargo tauri dev waits for localhost:1420
echo "Starting Vite..."
npm run dev &
