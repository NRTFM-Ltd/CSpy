# AGENTS.md

Instructions for AI coding assistants working in this repository.

## Project

CSpy is a macOS menu bar app (Tauri 2 + Rust + SvelteKit + Svelte 5) that monitors Claude AI subscription usage quotas. It reads an OAuth token from a config file (`~/.config/cspy/token`) or the macOS Keychain and polls the Anthropic usage API every 3 minutes.

## Commands

```bash
npm install              # Install frontend dependencies
cargo tauri dev          # Dev mode (hot-reload Svelte + Rust rebuild on change)
cargo tauri build        # Production build
npm run check            # TypeScript type checking
```

No test runner or linter is configured.

## Architecture

- **Rust backend** (`src-tauri/src/`): Token access (file + Keychain), HTTP polling, dynamic tray icon, Tauri commands.
- **Svelte frontend** (`src/`): 290x240 borderless popover with progress bar, burn rate, and countdown timer.
- **Communication**: Rust exposes `get_usage` and `refresh_usage` as Tauri commands (invoked via `invoke()`). The background polling loop pushes `usage-updated` and `usage-error` events that Svelte listens to via `listen()`.

## Dual type definitions

`UsageData` and `UsageBucket` are defined in both Rust (`src-tauri/src/usage.rs`) and TypeScript (`src/lib/types.ts`). When modifying these structs, update both files to keep them in sync.

Colour tier thresholds (green < 70%, amber 70-89%, red >= 90%) are defined in `icon.rs` (Rust) and `types.ts` (TypeScript). Keep these in sync.

## Conventions

- **Svelte 5 runes** — use `$state`, `$effect`, `$derived`. Do not use Svelte 4 stores.
- **Tauri capabilities** — any new window operation or plugin permission must be added to `src-tauri/capabilities/default.json`.
- **Token access** — checks `~/.config/cspy/token` first, then macOS Keychain via `security` CLI. See `keychain.rs`.
- **No Dock icon** — the app sets `ActivationPolicy::Accessory`. Do not add a main application window.
- **No persistence** — stateless by design. No database, no config files beyond the optional token file. Re-reads token source on each launch.
- **Polling interval** — `POLL_SECS = 180` in `lib.rs`. Do not reduce below 60 seconds.
- **Shared HTTP client** — single `reqwest::Client` with timeout, stored in `AppState`. Do not create new clients per request.
- **Redacted Debug** — `OAuthCreds` never prints the token. Do not derive `Debug` on structs containing secrets.

## Spelling

Use UK English (e.g. `utilisation`, `colour`). Field names in Rust structs and TypeScript interfaces already follow this.

## What not to do

- Do not add a Dock icon or main window — this is a menu bar-only app.
- Do not store the OAuth token on disk beyond `~/.config/cspy/token` — Keychain tokens are read at runtime only.
- Do not change the `security` CLI approach to Keychain access without good reason; the `keyring` crate was tried and was less reliable for this use case.
- Do not create new `reqwest::Client` instances per request — use the shared client in `AppState`.
- Do not derive `Debug` on credential structs — use redacting `Debug` impls.
