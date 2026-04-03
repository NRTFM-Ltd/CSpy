# Release Pipeline and Auto-Updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship polling bug fixes to deployed users and ensure all future fixes auto-deliver via Tauri updater + GitHub Releases.

**Architecture:** Tauri updater plugin checks GitHub Releases every 30 minutes, downloads updates silently, and restarts during quiet hours (23:00-08:00 local). Release Please automates version bumps and changelogs from conventional commits. A new CI workflow builds and uploads artefacts on every tag.

**Tech Stack:** Tauri 2 updater plugin, tauri-apps/tauri-action, Release Please, GitHub Actions

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src-tauri/Cargo.toml` | Modify | Add `tauri-plugin-updater` dependency |
| `package.json` | Modify | Add `@tauri-apps/plugin-updater` dependency |
| `src-tauri/tauri.conf.json` | Modify | Add updater plugin config, `updater` bundle target |
| `src-tauri/capabilities/default.json` | Modify | Add updater permissions |
| `src-tauri/src/lib.rs` | Modify | Register plugin, add `update_pending` state, update check loop, quiet-hours restart |
| `.release-please-config.json` | Create | Release Please configuration with extra-files for version sync |
| `.release-please-manifest.json` | Create | Version tracking for Release Please |
| `.github/workflows/release-please.yml` | Create | Automated release PR creation |
| `.github/workflows/commitlint.yml` | Create | Conventional commit validation on PRs |
| `.github/workflows/build-and-release.yml` | Create | Build + upload artefacts on tag |
| `commitlint.config.js` | Create | Commitlint configuration (extends conventional) |

---

### Task 1: Generate Tauri Updater Signing Keypair

This is the prerequisite for everything. The updater needs a keypair to verify update bundles are authentic. The public key goes in `tauri.conf.json`, the private key goes in GitHub Secrets.

**Files:**
- Output: `~/.tauri/CSpy.key` (private key — never commit this)
- Output: public key string (goes into tauri.conf.json in Task 3)

- [ ] **Step 1: Generate the keypair**

Run interactively (prompts for password):
```bash
cargo tauri signer generate -w ~/.tauri/CSpy.key
```

This outputs a public key string to stdout. **Copy it** — you need it for Task 3.

- [ ] **Step 2: Verify the key files exist**

Run: `ls -la ~/.tauri/CSpy.key*`
Expected: `CSpy.key` (private) and `CSpy.key.pub` (public)

- [ ] **Step 3: Read the public key for later use**

Run: `cat ~/.tauri/CSpy.key.pub`

Save this value — it goes in `tauri.conf.json` in Task 3.

---

### Task 2: Add Updater Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml:17-27` (dependencies section)
- Modify: `package.json:24-27` (dependencies section)

- [ ] **Step 1: Add Rust dependency**

In `src-tauri/Cargo.toml`, add `tauri-plugin-updater` to the `[dependencies]` section, after `tauri-plugin-notification`:

```toml
tauri-plugin-updater = "2"
```

The full `[dependencies]` section should now be:

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
chrono = { version = "0.4", features = ["serde"] }
tauri = { version = "2.10.3", features = ["macos-private-api", "tray-icon"] }
tauri-plugin-log = "2.0.0"
tauri-plugin-notification = "2.2.1"
tauri-plugin-updater = "2"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1.0", features = ["rt", "time", "sync"] }
```

- [ ] **Step 2: Add JS dependency**

In `package.json`, add to the `"dependencies"` object:

```json
"@tauri-apps/plugin-updater": "^2"
```

The full `"dependencies"` section should now be:

```json
"dependencies": {
    "@tauri-apps/api": "^2.10.1",
    "@tauri-apps/plugin-notification": "^2.2.1",
    "@tauri-apps/plugin-updater": "^2"
}
```

- [ ] **Step 3: Install JS dependencies**

Run: `npm install`
Expected: lockfile updated, no errors.

- [ ] **Step 4: Verify Rust dependency resolves**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: compiles successfully (updater plugin not registered yet, but dep resolves).

---

### Task 3: Configure tauri.conf.json

**Files:**
- Modify: `src-tauri/tauri.conf.json:34-36` (bundle targets)
- Modify: `src-tauri/tauri.conf.json:50` (plugins section)

- [ ] **Step 1: Add `updater` to bundle targets**

In `src-tauri/tauri.conf.json`, change the `targets` array:

From:
```json
"targets": ["dmg", "app"],
```

To:
```json
"targets": ["dmg", "app", "updater"],
```

- [ ] **Step 2: Add updater plugin configuration**

Replace the empty `"plugins": {}` with:

```json
"plugins": {
    "updater": {
        "endpoints": [
            "https://github.com/TheChasman/CSpy/releases/latest/download/latest.json"
        ],
        "pubkey": "PASTE_YOUR_PUBLIC_KEY_FROM_TASK_1_HERE"
    }
}
```

Replace `PASTE_YOUR_PUBLIC_KEY_FROM_TASK_1_HERE` with the actual public key string from Task 1 Step 3.

- [ ] **Step 3: Verify JSON is valid**

Run: `python3 -c "import json; json.load(open('src-tauri/tauri.conf.json'))"`
Expected: no output (valid JSON).

---

### Task 4: Add Updater Capabilities

**Files:**
- Modify: `src-tauri/capabilities/default.json:6-16` (permissions array)

- [ ] **Step 1: Add updater permissions**

In `src-tauri/capabilities/default.json`, add these three entries to the `permissions` array:

```json
"updater:default",
"updater:allow-check",
"updater:allow-download-and-install"
```

The full file should now be:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "CSpy default capabilities",
  "windows": ["popover"],
  "permissions": [
    "core:default",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-set-focus",
    "core:window:allow-set-position",
    "notification:default",
    "notification:allow-notify",
    "notification:allow-is-permission-granted",
    "notification:allow-request-permission",
    "updater:default",
    "updater:allow-check",
    "updater:allow-download-and-install"
  ]
}
```

---

### Task 5: Implement Update Check Loop

**Files:**
- Modify: `src-tauri/src/lib.rs`

This is the core implementation. We need to:
1. Add `update_pending` to `AppState`
2. Register the updater plugin
3. Add the update check loop
4. Add a quiet-hours restart check to the existing poll loop

- [ ] **Step 1: Add update_pending to AppState**

In `src-tauri/src/lib.rs`, add a new field to the `AppState` struct:

From:
```rust
pub struct AppState {
    pub token: RwLock<Option<String>>,
    /// Token expiry as millisecond Unix timestamp (None = unknown / token-file source).
    pub token_expires_at_ms: RwLock<Option<i64>>,
    pub cached: RwLock<Option<UsageData>>,
    pub client: reqwest::Client,
}
```

To:
```rust
pub struct AppState {
    pub token: RwLock<Option<String>>,
    /// Token expiry as millisecond Unix timestamp (None = unknown / token-file source).
    pub token_expires_at_ms: RwLock<Option<i64>>,
    pub cached: RwLock<Option<UsageData>>,
    pub client: reqwest::Client,
    /// Set to true when an update has been downloaded and is ready to install on restart.
    pub update_pending: RwLock<bool>,
}
```

- [ ] **Step 2: Add update check interval constant**

After the existing `MAX_BACKOFF_SECS` constant, add:

```rust
/// Update check interval in seconds (30 minutes).
const UPDATE_CHECK_SECS: u64 = 1800;
```

- [ ] **Step 3: Add the update check loop function**

Add this function after `start_polling`, before `update_tray_tooltip`:

```rust
fn start_update_checker(app_handle: tauri::AppHandle, state: Arc<AppState>) {
    use tauri_plugin_updater::UpdaterExt;

    tauri::async_runtime::spawn(async move {
        // Wait 60s before first check — let the app settle after launch
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        loop {
            log::debug!("Checking for updates...");

            match app_handle.updater_builder().build() {
                Ok(updater) => match updater.check().await {
                    Ok(Some(update)) => {
                        log::info!("Update available: v{}", update.version);
                        match update.download_and_install(|_, _| {}, || {}).await {
                            Ok(()) => {
                                log::info!("Update v{} downloaded and staged", update.version);
                                *state.update_pending.write().await = true;

                                // Notify user via macOS notification (Emitter already imported at file top)
                                let msg = format!(
                                    "CSpy v{} downloaded — will update tonight",
                                    update.version
                                );
                                let _ = tauri_plugin_notification::NotificationExt::notification(
                                    &app_handle,
                                )
                                .builder()
                                .title("CSpy Update Ready")
                                .body(&msg)
                                .show();
                                log::info!("Notification sent: {msg}");

                                // If already in quiet hours, restart immediately
                                if is_quiet_hours() {
                                    log::info!(
                                        "Already in quiet hours — restarting to apply update"
                                    );
                                    app_handle.restart();
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to download/install update: {e}");
                            }
                        }
                    }
                    Ok(None) => {
                        log::debug!("No updates available");
                    }
                    Err(e) => {
                        log::warn!("Update check failed: {e}");
                    }
                },
                Err(e) => {
                    log::error!("Failed to build updater: {e}");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(UPDATE_CHECK_SECS)).await;
        }
    });
}
```

- [ ] **Step 4: Add quiet-hours restart check to the existing poll loop**

In `start_polling`, replace the quiet hours block:

From:
```rust
            if is_quiet_hours() {
                log::info!("Quiet hours (23:00–08:00) — skipping poll");
            } else {
```

To:
```rust
            if is_quiet_hours() {
                // If an update was downloaded, restart now (user is asleep)
                if *state.update_pending.read().await {
                    log::info!("Quiet hours + update pending — restarting to apply update");
                    app_handle.restart();
                }
                log::info!("Quiet hours (23:00–08:00) — skipping poll");
            } else {
```

- [ ] **Step 5: Register the updater plugin in the builder chain**

In the `run()` function, add the updater plugin registration after the notification plugin:

From:
```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
```

To:
```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
```

- [ ] **Step 6: Initialise update_pending in AppState constructor**

In the `run()` function, update the `AppState` constructor:

From:
```rust
    let state = Arc::new(AppState {
        token: RwLock::new(None),
        token_expires_at_ms: RwLock::new(None),
        cached: RwLock::new(None),
        client,
    });
```

To:
```rust
    let state = Arc::new(AppState {
        token: RwLock::new(None),
        token_expires_at_ms: RwLock::new(None),
        cached: RwLock::new(None),
        client,
        update_pending: RwLock::new(false),
    });
```

- [ ] **Step 7: Start the update checker in setup**

In the `setup` closure, after `start_polling(handle, state.clone());`, add:

```rust
            // Start background update checker
            start_update_checker(app.handle().clone(), state.clone());
```

The setup section should now have:
```rust
            // Start background polling
            start_polling(handle, state.clone());

            // Start background update checker
            start_update_checker(app.handle().clone(), state.clone());
```

---

### Task 6: Verify Build

**Files:** None (verification only)

- [ ] **Step 1: Check compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: compiles with no errors. Warnings are OK.

- [ ] **Step 2: Full build with updater bundle target**

Run: `cargo tauri build 2>&1 | tail -20`
Expected: build succeeds, outputs to `src-tauri/target/release/bundle/`. Look for the `.tar.gz` updater bundle alongside the `.dmg`.

- [ ] **Step 3: Verify updater bundle was produced**

Run: `ls -la src-tauri/target/release/bundle/macos/*.tar.gz* 2>/dev/null || echo "No .tar.gz — check build output"`
Expected: a `.app.tar.gz` file exists (the updater bundle).

Note: without the signing key available at build time, the `.tar.gz.sig` signature file won't be produced. That's expected — CI will have the signing key via GitHub Secrets.

- [ ] **Step 4: Commit the code changes**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock package.json package-lock.json \
  src-tauri/tauri.conf.json src-tauri/capabilities/default.json \
  src-tauri/src/lib.rs src-tauri/src/usage.rs src-tauri/src/keychain.rs \
  scripts/switch_prod
git commit -m "feat: add auto-updater with quiet-hours restart

- Tauri updater plugin checks GitHub Releases every 30 minutes
- Downloads updates silently, notifies user via macOS notification
- Restarts during quiet hours (23:00-08:00 local) to apply update
- Also includes polling fixes: token expiry detection, exponential
  backoff on consecutive errors, Retry-After header respect

BREAKING: requires Tauri updater signing key in CI secrets"
```

---

### Task 7: Add Release Please Configuration

**Files:**
- Create: `.release-please-config.json`
- Create: `.release-please-manifest.json`

- [ ] **Step 1: Create .release-please-config.json**

```json
{
  "$schema": "https://raw.githubusercontent.com/googleapis/release-please/main/schemas/config.json",
  "release-type": "rust",
  "include-component-in-tag": false,
  "packages": {
    ".": {
      "changelog-path": "CHANGELOG.md",
      "bump-minor-pre-major": true,
      "bump-patch-for-minor-pre-major": true,
      "extra-files": [
        {
          "type": "json",
          "path": "package.json",
          "jsonpath": "$.version"
        },
        {
          "type": "json",
          "path": "src-tauri/tauri.conf.json",
          "jsonpath": "$.version"
        }
      ]
    }
  }
}
```

Note: the `extra-files` block ensures Release Please bumps the version in `package.json` and `tauri.conf.json` alongside `Cargo.toml`, keeping all three in sync.

- [ ] **Step 2: Create .release-please-manifest.json**

```json
{
  ".": "0.1.0"
}
```

This tells Release Please the current version is 0.1.0. Since the commit history contains `feat:` commits, it will propose bumping to 0.2.0.

---

### Task 8: Add CI Workflows

**Files:**
- Create: `.github/workflows/release-please.yml`
- Create: `.github/workflows/commitlint.yml`
- Create: `.github/workflows/build-and-release.yml`

- [ ] **Step 1: Create release-please.yml**

```yaml
name: Release Please

on:
  push:
    branches:
      - main

permissions:
  contents: write
  pull-requests: write

jobs:
  release-please:
    runs-on: ubuntu-latest
    steps:
      - uses: googleapis/release-please-action@v4
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 2: Create commitlint.yml**

```yaml
name: Commit Lint

on:
  pull_request:
    branches:
      - main

jobs:
  commitlint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: actions/setup-node@v4
        with:
          node-version: 20

      - run: npm install @commitlint/cli @commitlint/config-conventional

      - run: npx commitlint --from ${{ github.event.pull_request.base.sha }} --to ${{ github.event.pull_request.head.sha }}
```

- [ ] **Step 3: Create commitlint.config.js**

In the project root:

```js
export default { extends: ['@commitlint/config-conventional'] };
```

Without this file, commitlint won't apply conventional commit rules even though the package is installed.

- [ ] **Step 4: Create build-and-release.yml**

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
          # Apple signing — uncomment when Developer ID arrives:
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

`tauri-apps/tauri-action` detects the existing GitHub Release (created by Release Please for this tag) and uploads artefacts (`.dmg`, `.app.tar.gz`, `.app.tar.gz.sig`, `latest.json`) to it. The `releaseBody` is a fallback only used if no release exists yet.

---

### Task 9: Commit and Push CI/Infra Changes

**Files:** None (git operations only)

- [ ] **Step 1: Commit Release Please and CI files**

```bash
git add .release-please-config.json .release-please-manifest.json \
  .github/workflows/release-please.yml \
  .github/workflows/commitlint.yml \
  .github/workflows/build-and-release.yml \
  commitlint.config.js \
  docs/ .gitignore
git commit -m "chore: add release pipeline and CI workflows

- Release Please for automated version bumps and changelogs
- Commitlint to enforce conventional commits on PRs
- Build-and-release workflow: builds on tag, uploads artefacts to GitHub Release
- Extra-files config keeps version in sync across Cargo.toml, package.json, tauri.conf.json"
```

- [ ] **Step 2: Push to main**

```bash
git push origin main
```

Expected: push succeeds. Within a few minutes, the `release-please` workflow runs and opens a PR titled something like "chore(main): release 0.2.0".

---

### Task 10: Add GitHub Secrets for Updater Signing

This must be done via the GitHub web UI or `gh` CLI.

**Files:** None (GitHub settings)

- [ ] **Step 1: Read the private key**

```bash
cat ~/.tauri/CSpy.key
```

Copy the full contents (including the `-----BEGIN` and `-----END` lines).

- [ ] **Step 2: Add TAURI_SIGNING_PRIVATE_KEY secret**

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/CSpy.key
```

Or: GitHub repo → Settings → Secrets and variables → Actions → New repository secret → Name: `TAURI_SIGNING_PRIVATE_KEY`, Value: paste the private key.

- [ ] **Step 3: Add TAURI_SIGNING_PRIVATE_KEY_PASSWORD secret**

```bash
echo -n "YOUR_PASSWORD_FROM_TASK_1" | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

Use the password you entered when generating the keypair in Task 1.

- [ ] **Step 4: Verify secrets are set**

```bash
gh secret list
```

Expected: both `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` appear in the list.

---

### Task 11: Merge Release Please PR and Verify

**Files:** None (GitHub operations)

- [ ] **Step 1: Wait for and review the Release Please PR**

```bash
gh pr list
```

Expected: a PR titled "chore(main): release 0.2.0" (or similar). It will contain:
- Version bumps in `Cargo.toml`, `package.json`, `tauri.conf.json`
- A new `CHANGELOG.md` with all changes since the repo started

- [ ] **Step 2: Review the PR diff**

```bash
gh pr view <PR_NUMBER> --web
```

Check that:
- All three version files are bumped to 0.2.0
- CHANGELOG.md looks reasonable
- No unexpected file changes

- [ ] **Step 3: Merge the PR**

```bash
gh pr merge <PR_NUMBER> --merge
```

Expected: PR merges. Release Please creates a `v0.2.0` tag and GitHub Release. The tag triggers `build-and-release.yml`.

- [ ] **Step 4: Monitor the build workflow**

```bash
gh run list --workflow=build-and-release.yml
```

Wait for it to appear, then watch it:

```bash
gh run watch <RUN_ID>
```

Expected: workflow completes successfully. Artefacts uploaded to the GitHub Release.

- [ ] **Step 5: Verify the release has all artefacts**

```bash
gh release view v0.2.0
```

Expected artefacts:
- `CSpy.app.tar.gz` — updater bundle
- `CSpy.app.tar.gz.sig` — updater signature
- `CSpy_0.2.0_aarch64.dmg` or similar — fresh install DMG
- `latest.json` — updater manifest

- [ ] **Step 6: Verify latest.json is fetchable**

```bash
curl -sL https://github.com/TheChasman/CSpy/releases/latest/download/latest.json | python3 -m json.tool
```

Expected: JSON with `version`, `notes`, `pub_date`, and platform-specific download URLs.
