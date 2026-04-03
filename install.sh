#!/bin/bash
set -euo pipefail

# CSpy launchd installer
# Generates plist via heredoc to avoid com.apple.provenance issues

LABEL="com.nrtfm.cspy"
APP_NAME="CSpy.app"
BUNDLE_DIR="src-tauri/target/release/bundle/macos"
INSTALL_DIR="/Applications"
PLIST_DIR="${HOME}/Library/LaunchAgents"
PLIST_PATH="${PLIST_DIR}/${LABEL}.plist"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_SRC="${SCRIPT_DIR}/${BUNDLE_DIR}/${APP_NAME}"
APP_DST="${INSTALL_DIR}/${APP_NAME}"

# ── Pre-flight checks ────────────────────────────────────────

if [[ ! -d "${APP_SRC}" ]]; then
    echo "ERROR: ${APP_SRC} not found. Run 'cargo tauri build' first."
    exit 1
fi

# ── Install app bundle ────────────────────────────────────────

echo "Installing ${APP_NAME} to ${INSTALL_DIR}..."
rm -rf "${APP_DST}"
cp -R "${APP_SRC}" "${APP_DST}"

# Strip provenance from all files in the bundle
xattr -cr "${APP_DST}" 2>/dev/null
echo "Stripped extended attributes from ${APP_DST}"

# ── Generate launchd plist (heredoc — no provenance) ──────────

echo "Generating launchd plist at ${PLIST_PATH}..."
mkdir -p "${PLIST_DIR}"

cat > "${PLIST_PATH}" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${LABEL}</string>
    <key>Program</key>
    <string>${APP_DST}/Contents/MacOS/CSpy</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>StandardOutPath</key>
    <string>${HOME}/Library/Logs/cspy.log</string>
    <key>StandardErrorPath</key>
    <string>${HOME}/Library/Logs/cspy.log</string>
</dict>
</plist>
PLIST

echo "Plist written."

# ── Unload old agent if running ───────────────────────────────

launchctl bootout "gui/$(id -u)/${LABEL}" 2>/dev/null
sleep 1

# ── Load new agent ────────────────────────────────────────────

echo "Loading launchd agent..."
launchctl bootstrap "gui/$(id -u)" "${PLIST_PATH}"

if [[ $? -eq 0 ]]; then
    echo "CSpy installed and running."
    echo "  App:   ${APP_DST}"
    echo "  Plist: ${PLIST_PATH}"
    echo "  Log:   ~/Library/Logs/cspy.log"
else
    echo "ERROR: launchctl bootstrap failed."
    echo "Try: launchctl bootstrap gui/$(id -u) ${PLIST_PATH}"
    exit 1
fi
