# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What CSpy Is

A macOS menu bar app that monitors Claude AI subscription usage (5-hour and 7-day quotas) in real time. Built with Tauri 2 + Rust + SvelteKit + Svelte 5.

**Not** an API developer cost tracker. This reads the **subscriber** usage data that Claude.ai itself uses to show the usage bar.

## Commands

```bash
npm install              # Install JS deps (required after clone or lockfile change)
cargo tauri dev          # Dev mode — hot-reload Svelte + Rust rebuilds on change
cargo tauri build        # Production build → src-tauri/target/release/bundle/
npm run check            # SvelteKit sync + TypeScript type checking
cargo tauri icon <img>   # Generate all icon sizes from a source image
```

No test runner is configured yet. No linter is configured.

## Architecture

```
Token source (file or Keychain)
    │
    ▼  reads OAuth token
┌──────────────────────┐
│  Rust backend         │
│  ├─ keychain.rs       │  Token file (~/.config/cspy/token) → Keychain fallback
│  ├─ usage.rs          │  GET api.anthropic.com/api/oauth/usage
│  ├─ icon.rs           │  Dynamic 32×32 tray icon with cached rendering
│  └─ lib.rs            │  Tray icon, polling loop, Tauri commands
└──────┬───────────────┘
       │  events (usage-updated, usage-error) + invoke (get_usage, refresh_usage)
┌──────▼───────────────┐
│  Svelte popover       │  290×240 borderless window
│  └─ +page.svelte      │  Progress bar, burn rate, countdown, refresh button
└──────────────────────┘
```

### Token sources (keychain.rs)

Checked in order:
1. **Token file** at `~/.config/cspy/token` — for users without Claude Code
2. **macOS Keychain** — reads "Claude Code-credentials" via the `security` CLI

### Rust ↔ Svelte communication

Two mechanisms:

1. **Tauri Commands (RPC)** — Svelte calls `invoke('get_usage')` or `invoke('refresh_usage')`. Defined in `lib.rs` with `#[tauri::command]`.
2. **Tauri Events (push)** — Rust emits `usage-updated` (with `UsageData` payload) and `usage-error` (with error string) from the background polling loop. Svelte listens via `listen()` in `onMount`.

### Polling loop

- Runs in a `tokio::spawn` task in `lib.rs`.
- Interval: `POLL_SECS = 180` (3 minutes).
- Quiet hours: 23:00–08:00 local time — skips polling.
- On success: caches `UsageData` in `AppState`, updates tray icon and tooltip, emits `usage-updated`.
- On 429: clears cached token (forces Keychain re-read next poll), logs, continues.
- On error: logs, emits `usage-error`, continues polling.

### HTTP client

A single `reqwest::Client` with a 15-second timeout is built once at startup and stored in `AppState`. All API calls share this client for connection pooling.

### Shared state

`AppState` holds `RwLock<Option<String>>` (cached OAuth token), `RwLock<Option<UsageData>>` (last fetched data), and `reqwest::Client`. Passed as Tauri managed state.

### Tray icon (icon.rs)

Dynamic 32×32 RGBA icon rendered at the pixel level. Hollow rectangle with colour-coded fill:
- Green (<70%), Amber (70–89%), Red (>=90%)
- Icons are cached by quantised utilisation (5% steps, max 21 entries) to bound memory usage.

## Key Data Flow

1. On startup, Rust reads the OAuth token from `~/.config/cspy/token` (if it exists) or "Claude Code-credentials" from macOS Keychain via the `security` CLI
2. Extracts `claudeAiOauth.accessToken` (sk-ant-oat01-...) from Keychain, or reads the token file directly
3. Polls `GET https://api.anthropic.com/api/oauth/usage` with header `anthropic-beta: oauth-2025-04-20`
4. API returns `{ five_hour: { utilization, resets_at }, seven_day: { ... } }`
5. Rust normalises utilization from 0-100 to 0.0-1.0, updates tray icon, emits event to frontend
6. Tray left-click toggles the popover, positioned centred below the icon

## Design Decisions

1. **No Dock icon** — `ActivationPolicy::Accessory`; menu bar only.
2. **Keychain via `security` CLI** — More reliable than `keyring` crate for generic-password items with unconventional account fields. See `keychain.rs`.
3. **Token file fallback** — `~/.config/cspy/token` for users without Claude Code installed.
4. **3-minute poll interval** — Balances freshness with being a good API citizen.
5. **Colour tiers** — green (<70%), amber (70–89%), red (>=90%). Defined in `icon.rs` (Rust) and `types.ts` (TypeScript). Keep these in sync.
6. **Popover, not window** — Borderless, always-on-top, `skipTaskbar`. Positioned at `(trayX - width/2, trayY + 4)`.
7. **No persistence** — Stateless; re-reads token source on each launch. No database.
8. **Countdown refresh** — Frontend re-renders reset countdowns every 30s via `setInterval`.
9. **Quiet hours** — 23:00–08:00 local time, no polling to conserve rate limit.
10. **Shared HTTP client** — Single `reqwest::Client` with 15s timeout, stored in `AppState`.
11. **Redacted Debug** — `OAuthCreds` and `ClaudeCredentials` have custom `Debug` impls that never print the token.

## Conventions

- Rust structs use `snake_case` fields; Svelte types mirror them exactly (`five_hour`, `resets_at`).
- `UsageData` and `UsageBucket` are defined in both `usage.rs` (Rust) and `types.ts` (TypeScript) — changes must be kept in sync.
- Tauri capabilities in `src-tauri/capabilities/default.json` — any new window or plugin permission must be added there.
- Frontend uses Svelte 5 runes (`$state`, `$effect`, `$derived`) — not Svelte 4 stores.
- UK English throughout (`utilisation`, `colour`, `licence`).

## Prerequisites

- Claude Code installed and logged in, OR a token file at `~/.config/cspy/token`
- Node.js (v20+)
- Rust stable toolchain + `cargo-tauri` CLI
- macOS 14+ (Sonoma)

## Toolchain Note

Rust toolchain is currently `stable-x86_64-apple-darwin` (Rosetta on M1).
For native ARM builds: `rustup target add aarch64-apple-darwin`
