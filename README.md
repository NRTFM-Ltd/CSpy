# CSpy

A macOS menu bar app that monitors your Claude AI subscription usage in real time.

CSpy sits in your menu bar and shows how much of your Claude subscription quota you've used. Click the icon to see a popover with your 5-hour quota, burn rate, and countdown timer. The tray icon itself is a mini progress bar that changes colour as usage climbs.

| Usage   | Colour |
|---------|--------|
| < 70%   | Green  |
| 70-89%  | Amber  |
| >= 90%  | Red    |

## Who is this for?

Claude **subscribers** (Pro, Max, Team, Enterprise) who want to keep an eye on their usage limits without switching to claude.ai.

CSpy is **not** an API cost tracker. It reads the same subscriber usage data that claude.ai displays.

## How it works

```
Token source --> OAuth token --> GET /api/oauth/usage --> tray icon + popover
```

CSpy reads your Claude OAuth token from one of two sources (see below), then polls the Anthropic usage endpoint every 3 minutes. A borderless popover shows your 5-hour quota with a progress bar, burn rate indicator, and countdown until reset.

## Getting your token

CSpy needs a Claude OAuth token. Two options:

### Option A: Claude Code (automatic)

If you have [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview) installed and logged in, CSpy reads the token from macOS Keychain automatically. No configuration needed.

### Option B: Token file (manual)

If you don't use Claude Code, save your OAuth token to a file:

```bash
mkdir -p ~/.config/cspy
echo "YOUR_TOKEN_HERE" > ~/.config/cspy/token
chmod 600 ~/.config/cspy/token
```

CSpy checks the token file first, then falls back to Keychain.

> **How to get a token without Claude Code:** The OAuth token (format `sk-ant-oat01-...`) is obtained through Anthropic's OAuth flow. If you have Claude Code installed on another machine, you can copy the token from there. Run `security find-generic-password -s "Claude Code-credentials" -w` on that machine to extract it.

## Prerequisites

- **macOS 14+** (Sonoma or later)
- **Node.js** v20+
- **Rust** stable toolchain + Tauri CLI (`cargo install tauri-cli`)
- A Claude subscription with an OAuth token (see above)

## Building from source

```bash
git clone https://github.com/TheChasman/CSpy.git
cd CSpy
npm install
cargo tauri build
```

The bundled `.app` and `.dmg` appear in `src-tauri/target/release/bundle/`.

## Installing

After building, run the install script to copy the app to `/Applications` and register it as a login item via launchd:

```bash
./install.sh
```

This will:
- Copy `CSpy.app` to `/Applications`
- Strip macOS quarantine attributes
- Register a launchd agent so CSpy starts on login
- Logs go to `~/Library/Logs/cspy.log`

To uninstall:

```bash
launchctl bootout gui/$(id -u)/com.nrtfm.cspy
rm -rf /Applications/CSpy.app
rm ~/Library/LaunchAgents/com.nrtfm.cspy.plist
```

## Development

```bash
npm install
cargo tauri dev
```

Hot-reloads the Svelte frontend and recompiles the Rust backend on file changes.

## Stack

| Layer    | Technology           |
|----------|----------------------|
| Backend  | Rust + Tauri 2       |
| Frontend | SvelteKit + Svelte 5 |
| HTTP     | reqwest              |
| Token    | macOS Keychain / file |

## Licence

MIT
