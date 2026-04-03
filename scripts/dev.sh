#!/usr/bin/env bash
# dev.sh — prepare environment for Tauri dev mode
# Called by Tauri as beforeDevCommand from the project root.
# cargo tauri dev handles Rust watching and binary restarts itself.
set -euo pipefail

LABEL="com.nrtfm.cspy"

# Kill any running CSpy instances to avoid duplicate tray icons
pkill -x "CSpy" 2>/dev/null || true

# Stop the production launchd agent
echo "Stopping launchd agent (${LABEL})..."
launchctl bootout "gui/$(id -u)/${LABEL}" 2>/dev/null || true
sleep 1

# Release port 1420 if held by a leftover Vite instance
if lsof -ti:1420 &>/dev/null 2>&1; then
    echo "Releasing port 1420..."
    kill "$(lsof -ti:1420)" 2>/dev/null || true
    sleep 1
fi
