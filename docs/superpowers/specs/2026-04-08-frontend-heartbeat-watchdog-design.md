# Frontend Heartbeat Watchdog Design

**Date:** 2026-04-08
**Status:** Approved

## Problem

The CSpy popover shows a White Screen of Death (WSOD) when the frontend fails to load — regardless of environment. In dev, Vite may not be running. In preview/production, the WebView may hang or fail to load embedded assets. There is currently no detection or recovery mechanism.

## Goal

Add a universal frontend health watchdog that detects an unresponsive frontend and takes environment-appropriate recovery action automatically.

## Design

### Section 1: Heartbeat mechanism

The Svelte app emits a `heartbeat` event to Rust every 30 seconds via `emit()` in a `setInterval`. Rust tracks `last_heartbeat: RwLock<Option<Instant>>` in `AppState`. A dedicated watchdog task checks every 60 seconds.

**Startup grace period:** 15 seconds from app launch. The watchdog does not act during this window (gives WebView time to load and send its first heartbeat).

**Thresholds:**
- **Healthy:** Heartbeat received within the last 90 seconds
- **Unhealthy:** No heartbeat for 90+ seconds after the grace period

One missed beat (30s heartbeat, 90s threshold = up to 3 misses before triggering) avoids false positives from a briefly busy JS thread.

### Section 2: Recovery actions

**Dev** (`cfg!(debug_assertions)`):
1. Log warning
2. TCP probe port 1420
3. If Vite is down: spawn `npm run dev` from project root (resolved at compile time via `env!("CARGO_MANIFEST_DIR")`), poll until port 1420 responds (up to 15s)
4. Reload WebView via `webview.eval("window.location.reload()")`
5. Reset heartbeat and grace period

**Preview/Production** (release builds):
1. Log error
2. Reload WebView
3. If still unhealthy after a second reload (no heartbeat within another 90s), emit a native notification: "CSpy frontend unresponsive — restart required"

**Vite child process (dev only):**
- Store `Child` handle in `AppState` as `vite_child: RwLock<Option<std::process::Child>>`
- Kill on `RunEvent::Exit`
- Watchdog respawns Vite on each unhealthy cycle — the 90s detection window is a natural rate limiter

### Section 3: Architecture and files

**`src/routes/+page.svelte`:**
- Add `setInterval(() => emit('heartbeat', null), 30_000)` in `onMount`
- Clean up in `onDestroy`

**`src-tauri/src/lib.rs`:**
- `AppState`: add `last_heartbeat: RwLock<Option<Instant>>`, dev-only `vite_child: RwLock<Option<std::process::Child>>`
- New Tauri command `heartbeat()`: writes `Instant::now()` to `last_heartbeat`
- New task `start_watchdog(...)`: 60s tick, checks threshold, calls recovery
- Dev-only helper `ensure_vite_running(...)`: TCP probe + spawn + poll
- `RunEvent::Exit` handler: kill `vite_child` if present
- Register `heartbeat` in `invoke_handler`

No new files required.

### Section 4: Testing

Unit tests in `lib.rs` test pure threshold logic:

| Test | Scenario | Expected |
|------|----------|----------|
| `heartbeat_healthy_within_threshold` | Last beat 60s ago, threshold 90s | Healthy |
| `heartbeat_unhealthy_beyond_threshold` | Last beat 100s ago | Unhealthy |
| `heartbeat_healthy_during_grace_period` | No beat, 10s since startup | Healthy |
| `heartbeat_none_after_grace_period` | No beat, 20s since startup | Unhealthy |

I/O paths (TCP probe, Vite spawn, WebView eval) are not unit tested — covered by manual verification.

## Files Modified

| File | Change |
|------|--------|
| `src/routes/+page.svelte` | Add heartbeat emitter in onMount/onDestroy |
| `src-tauri/src/lib.rs` | AppState fields, heartbeat command, watchdog task, Vite helper, exit handler |
