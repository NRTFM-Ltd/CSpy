# CSpy Test Suite & Linting — Design Spec

**Date:** 2026-04-08
**Status:** Approved

---

## Problem

No tests or linters are configured. Small regressions (e.g. stale tray icon after window reset, "0m" countdown on expired window) are only caught by eye after release.

---

## Scope

- Rust unit tests (inline `#[cfg(test)]`)
- Rust integration tests (`src-tauri/tests/`)
- Svelte/TypeScript unit + component tests (Vitest)
- Linting: Clippy (Rust) + ESLint (Svelte/TS)
- CI integration

---

## Approach

**B — Inline unit tests + separate integration tests**

Pure-function tests live as `#[cfg(test)]` blocks inside each Rust module (collocated, access to private helpers). HTTP integration tests live in `src-tauri/tests/` as a separate Cargo integration test crate with a `wiremock` server lifecycle. Svelte tests are co-located `*.test.ts` files.

---

## Rust Unit Tests

### `src-tauri/src/lib.rs`

| Function | Cases |
|---|---|
| `format_countdown` | future with hours+mins; future mins-only; past timestamp → `"—"`; unparseable string → `"—"` |
| `is_window_expired` | past `resets_at` → `true`; future → `false`; `resets_at: None` → `false` |
| `backoff_sleep` | 0–1 errors → `POLL_SECS`; 2 → 360s; 3 → 720s; 5+ → capped at 1800s |
| `is_quiet_hours` | Refactor to accept `hour: u32` param. Test boundary values: 22 → false; 23 → true; 0 → true; 7 → true; 8 → false |

### `src-tauri/src/icon.rs`

| Case | Assertion |
|---|---|
| 0% utilisation | No filled interior pixels (all interior is semi-transparent grey `(180, 180, 180, 80)`) |
| 50% utilisation | ~50% of interior pixels are green `(74, 222, 128, 255)` |
| 90% utilisation | Fill pixels are red `(248, 113, 113, 255)` |
| 70% utilisation | Fill pixels are amber `(251, 191, 36, 255)` |
| All levels | Output dimensions are always 32×32 |
| Cache | Same quantised level (5% steps) returns identical buffer |

### `src-tauri/src/keychain.rs`

`read_token_file` is refactored to accept a `home: &Path` parameter (instead of reading `$HOME`) to allow tempdir-based testing.

| Case | Assertion |
|---|---|
| Valid token file | Returns `Some("sk-ant-…")` with whitespace trimmed |
| Empty token file | Returns `None` |
| Missing token file | Returns `None` |
| `ClaudeCredentials` JSON deserialization | Valid JSON → correct fields populated |
| Missing `claudeAiOauth` field | `claude_ai_oauth` is `None` |
| Empty `accessToken` | Detected and rejected by `get_oauth_token` |

### `src-tauri/src/usage.rs`

| Case | Assertion |
|---|---|
| `utilization: 50` | Normalised to `utilisation: 0.5` |
| `utilization: 0` | Normalised to `utilisation: 0.0` |
| `utilization: 100` | Normalised to `utilisation: 1.0` |
| Missing `five_hour` key | `five_hour: None` |
| `resets_at: null` | `resets_at: None` in `UsageBucket` |

---

## Rust Integration Tests

**Location:** `src-tauri/tests/fetch_usage.rs`

**New dev-dependency:** `wiremock = "0.6"`

Each test spins up a `MockServer`, overrides the `USAGE_URL` constant (extracted to a function or passed as parameter), and asserts on the returned `Result`.

| Scenario | Expected result |
|---|---|
| 200 with valid JSON | `Ok(UsageData)` with correct normalised values |
| 401 response | `Err("token_expired")` |
| 429 with `Retry-After: 60` | `Err("rate_limited:60")` |
| 429 without `Retry-After` | `Err("rate_limited:0")` |
| 500 with body | `Err` containing the status code |
| Malformed JSON body | `Err` containing "parse" |

To make the URL injectable, `fetch_usage` gains a `url: &str` parameter — the full endpoint URL (e.g. `"https://api.anthropic.com/api/oauth/usage"`). Production callers pass the `USAGE_URL` constant. Integration tests pass `format!("{}/api/oauth/usage", mock_server.uri())`.

---

## Svelte / TypeScript Tests

**Framework:** Vitest + `@testing-library/svelte`

**New dev-dependencies:** `vitest`, `@testing-library/svelte`, `@vitest/coverage-v8` (optional)

### `src/lib/types.test.ts`

Pure functions — no mocking needed.

| Function | Cases |
|---|---|
| `tierFor` | 0.0 → `green`; 0.69 → `green`; 0.70 → `amber`; 0.89 → `amber`; 0.90 → `red`; 1.0 → `red` |
| `burnRateTier` | 0 → `green`; 15.9 → `green`; 16 → `amber`; 19.9 → `amber`; 20 → `red` |
| `calculateBurnRate` | elapsed < 60s → 0; 50% used over 2.5h → 20%/hr; elapsed = full window (18000s) → correct rate |
| `formatCountdown` | `null` → `"—"`; past ISO → `"resetting…"`; 90 mins future → `"1h 30m"`; 45 mins future → `"45m"` |

### `src/routes/+page.test.ts`

Tauri APIs mocked via `vi.mock('@tauri-apps/api/core')` and `vi.mock('@tauri-apps/api/event')`.

| Scenario | Assertion |
|---|---|
| Initial render | Shows "Reading Keychain…" loading state |
| `usage-updated` event fires | Renders percentage value, progress bar, countdown |
| `usage-error` event (no prior data) | Renders error box with message |
| `usage-error` event (with stale data) | Renders stale warning, NOT the error box |
| Refresh button clicked | `invoke('refresh_usage')` is called |
| Refresh button while loading | Button is disabled |

Note: `formatCountdown` returns `"resetting…"` in the frontend for expired windows (user-facing text). The Rust tray uses `"—"` (compact). Both are correct for their contexts.

---

## Linting

### Rust — Clippy

```bash
cargo clippy -- -D warnings
```

No custom project-level `#[allow(...)]` pragmas. If clippy flags it, fix it.

### Svelte/TS — ESLint

Flat config (`eslint.config.js`) with:
- `eslint-plugin-svelte` v3+ (required for Svelte 5 runes support)
- `@typescript-eslint/eslint-plugin`

New script in `package.json`:
```json
"lint": "eslint src"
```

---

## CI Integration

New job order in the existing workflow:

```
lint (clippy + eslint)  ┐
                         ├─→  build
test (cargo test + vitest) ┘
```

Lint and test run in parallel; build is gated on both passing.

```yaml
# In CI workflow
- name: Clippy
  run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

- name: Cargo test
  run: cargo test --manifest-path src-tauri/Cargo.toml

- name: ESLint
  run: npm run lint

- name: Vitest
  run: npm run test -- --run
```

---

## Refactors Required

These are minimal, targeted changes needed to make code testable — not cleanup:

1. **`is_quiet_hours`** — extract `hour` param: `fn is_quiet_hours_at(hour: u32) -> bool`. Keep `fn is_quiet_hours() -> bool` as a one-liner calling `is_quiet_hours_at(Local::now().hour())`.
2. **`read_token_file`** — accept `home: &Path` instead of reading `$HOME` env var.
3. **`fetch_usage`** — accept `base_url: &str` parameter; production callers pass `USAGE_URL`, integration tests pass the mock server URI.
