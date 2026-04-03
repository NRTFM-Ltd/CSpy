# Release Pipeline and Auto-Updater — Design Spec

**Date:** 2026-04-03
**Version target:** 0.2.0
**Status:** Approved

## Context

CSpy is a public macOS menu bar app (TheChasman/CSpy). Three serious bugs in the polling loop (no token expiry detection, no exponential backoff, Retry-After header ignored) have been fixed locally but cannot reach deployed installations because:

1. No GitHub Releases exist
2. No automated release workflow runs on `main`
3. No auto-update mechanism is built into the app

Users who built from source and installed via `install.sh` are stuck on the broken version. This spec covers shipping the fix and ensuring future fixes reach users automatically.

## Goals

1. **Ship the fix** — commit, tag, build, release v0.2.0 with all polling fixes + the updater itself
2. **Automate future releases** — conventional commits → Release Please → GitHub Release → CI build → artefacts
3. **Auto-update deployed installations** — Tauri updater plugin checks GitHub Releases, downloads updates silently, applies during quiet hours

## Non-Goals

- Cross-platform support (macOS only for now)
- Staged rollouts or feature flags
- Custom update server (GitHub Releases is the endpoint)
- Apple code signing and notarisation (pending Developer ID application; pipeline will have placeholders)

---

## Architecture

### Release Flow (Commit to User)

```
1. fix: commit lands on main
2. Release Please opens/updates a release PR (bumps version, writes CHANGELOG)
3. Maintainer merges the release PR when ready to ship
4. Release Please creates GitHub Release + git tag (e.g. v0.2.0)
5. build-and-release.yml triggers on new tag → builds on macOS runner
6. CI uploads .tar.gz (update bundle) + .dmg (fresh install) + latest.json to the GitHub Release
7. User's CSpy checks GitHub Releases every 30 minutes
8. Downloads update silently → macOS notification: "CSpy v0.2.0 ready — will update tonight"
9. During quiet hours (23:00–08:00 local) → tauri::process::restart() → updated app runs
```

### CI Workflows (3 files)

| Workflow | Trigger | Purpose | Status |
|---|---|---|---|
| `release-please.yml` | push to `main` | Creates/updates release PR; on merge creates tag + GitHub Release | Exists on branch, needs merge |
| `build-and-release.yml` | new tag (`v*`) | Builds .app/.dmg on macOS, uploads artefacts to GitHub Release | New — must create |
| `preview-build.yml` | PR to `main` | Builds preview DMG for testing | Exists — no changes |

### Update Lifecycle

The core problem: CSpy runs via launchd (`RunAtLoad: true`, `KeepAlive: false`). It starts at login and stays running indefinitely. Sleep/wake does not kill or restart the process. "Apply on next launch" would mean "apply on next login", which for users who just close/open their lid could be weeks or never.

**Solution: quiet-hours restart.**

```
14:30  Update check finds v0.2.0 → downloads .tar.gz silently
14:30  macOS notification: "CSpy v0.2.0 downloaded — will update tonight"
14:30–23:00  Normal operation. update_pending = true in AppState.
23:00  Poll loop enters quiet hours, sees update_pending → restart()
       Process exits → launchd relaunches → new binary runs
```

**Edge cases:**

| Scenario | Behaviour |
|---|---|
| Mac awake at 23:00 | Restart on next poll tick (~3 min into quiet hours) |
| Mac asleep at 23:00, wakes at 02:00 | Poll loop resumes, still quiet hours → restart immediately |
| Mac asleep at 23:00, wakes at 09:00 | Missed window → waits for tonight's quiet hours |
| Update downloaded during quiet hours | Restart immediately (user is asleep) |

Quiet hours use `chrono::Local::now()` — always the user's local timezone.

---

## Code Changes

### Rust Dependencies

Add to `src-tauri/Cargo.toml`:
```toml
tauri-plugin-updater = "2"
```

Add to `package.json`:
```json
"@tauri-apps/plugin-updater": "^2"
```

### tauri.conf.json

Add updater plugin configuration:
```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/TheChasman/CSpy/releases/latest/download/latest.json"
      ],
      "pubkey": "<UPDATER_PUBKEY>"
    }
  }
}
```

The `pubkey` is a Tauri updater signing key (not Apple code signing). Generated once via `cargo tauri signer generate`, public key goes in config, private key goes in GitHub Secrets (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). This ensures update bundles are authentic.

### Capabilities (default.json)

Add updater permissions:
```json
"updater:default",
"updater:allow-check",
"updater:allow-download-and-install"
```

### AppState

Add one field:
```rust
pub update_pending: RwLock<bool>,
```

### Update Check Loop (new, in lib.rs)

A separate `tokio::spawn` loop, independent of usage polling:

```
loop {
    sleep(30 minutes)
    check for update via tauri_plugin_updater
    if update available:
        download silently
        send macOS notification via tauri-plugin-notification (already installed): "CSpy vX.Y.Z downloaded — will update tonight"
        set update_pending = true
    if update_pending && is_quiet_hours():
        tauri::process::restart()
}
```

30-minute interval — light touch, won't compete with usage polling or hit GitHub API limits.

### Usage Poll Loop (modification)

Add one check at the top of the quiet hours branch:

```rust
if is_quiet_hours() {
    if *state.update_pending.read().await {
        log::info!("Quiet hours + update pending — restarting to apply update");
        app_handle.restart();
    }
    // existing skip-poll logic
}
```

This gives two chances to catch the restart: the update loop and the usage loop. Whichever ticks first during quiet hours triggers it.

### Bundle Targets

Add `updater` to bundle targets in `tauri.conf.json`:
```json
"targets": ["dmg", "app", "updater"]
```

This produces the `.tar.gz` update bundle alongside the `.dmg`.

---

## CI: build-and-release.yml

```yaml
name: Build and Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  build:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@stable
      - name: Cache Rust artefacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            src-tauri/target
          key: ${{ runner.os }}-cargo-${{ hashFiles('src-tauri/Cargo.lock') }}
          restore-keys: ${{ runner.os }}-cargo-
      - run: npm ci
      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          # Apple signing (uncomment when Developer ID arrives):
          # APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          # APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          # APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          # APPLE_ID: ${{ secrets.APPLE_ID }}
          # APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          # APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: 'CSpy ${{ github.ref_name }}'
          releaseBody: 'See CHANGELOG.md for details.'
          releaseDraft: false
          prerelease: false
          updaterJsonKeepUniversal: true
```

`tauri-apps/tauri-action` handles building, bundling, generating `latest.json`, and uploading all artefacts (`.dmg`, `.app.tar.gz`, `.app.tar.gz.sig`, `latest.json`) to the GitHub Release.

**Interaction with Release Please:** Release Please creates the GitHub Release (with CHANGELOG body) and git tag when the release PR is merged. The tag triggers this workflow. `tauri-apps/tauri-action` detects the existing release for the tag and uploads artefacts to it rather than creating a duplicate. The `releaseBody` in this workflow is a fallback — it's only used if the release doesn't already exist.

---

## CI: Release Please and Commitlint

Cherry-pick from `origin/chore/trigger-commitlint-check`:

- `.github/workflows/release-please.yml` — runs on push to `main`, creates release PRs, creates tags on merge
- `.github/workflows/commitlint.yml` — validates conventional commit format on PRs
- `.release-please-config.json` — release type `rust`, changelog at root
- `.release-please-manifest.json` — tracks current version

These files exist on the branch and are ready to use as-is. The only change: `.release-please-manifest.json` version must be updated to `0.2.0` to match the target.

---

## Signing Strategy

### Tauri Updater Signing (required now)

Generated via `cargo tauri signer generate`. This is a Tauri-specific keypair for verifying update bundles — not Apple code signing. The public key goes in `tauri.conf.json`, the private key goes in GitHub Secrets.

### Apple Code Signing (when Developer ID arrives)

Add these GitHub Secrets:
- `APPLE_CERTIFICATE` — base64-encoded .p12
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY` — "Developer ID Application: Name (TeamID)"
- `APPLE_ID` — Apple account email
- `APPLE_PASSWORD` — app-specific password
- `APPLE_TEAM_ID`

Uncomment the env vars in `build-and-release.yml`. `tauri-apps/tauri-action` handles signing and notarisation automatically.

**Until then:** builds are unsigned. Users need right-click → Open on first install. Auto-updates work but trigger Gatekeeper on each update.

---

## Version Bump

All three version locations must be updated to `0.2.0`:
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

Release Please will handle subsequent version bumps automatically after merging.

---

## Files Changed Summary

| File | Action |
|---|---|
| `src-tauri/Cargo.toml` | Add `tauri-plugin-updater` dep, bump to 0.2.0 |
| `package.json` | Add `@tauri-apps/plugin-updater` dep, bump to 0.2.0 |
| `src-tauri/tauri.conf.json` | Add updater plugin config, add `updater` bundle target, bump to 0.2.0 |
| `src-tauri/capabilities/default.json` | Add updater permissions |
| `src-tauri/src/lib.rs` | Add `update_pending` to AppState, add update-check loop, add quiet-hours restart check |
| `.github/workflows/build-and-release.yml` | New — build + upload on tag |
| `.github/workflows/release-please.yml` | From branch — creates release PRs |
| `.github/workflows/commitlint.yml` | From branch — enforces conventional commits |
| `.release-please-config.json` | From branch |
| `.release-please-manifest.json` | From branch, updated to 0.2.0 |

---

## What This Spec Does NOT Cover

- The polling fixes themselves (token expiry detection, exponential backoff, Retry-After handling) — these are already implemented in the working tree
- Frontend changes — no UI changes needed; notifications use macOS native notifications via existing `tauri-plugin-notification`
- `install.sh` changes — existing installs won't auto-update to the first release; users must rebuild once. After that, auto-updates handle everything.
