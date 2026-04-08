# Stale Build Prevention Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent WSOD from stale frontend builds by fixing the broken test file, adding a build.rs guard, and catching frontend build failures in CI.

**Architecture:** Three independent changes: rename the test file to unblock SvelteKit builds, enhance build.rs to auto-run `npm run build` when `build/` is missing, and add a frontend build step to the CI test workflow.

**Tech Stack:** Rust (build.rs), SvelteKit/Vite, GitHub Actions

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/routes/+page.test.ts` | Delete | Removed (renamed) |
| `src/routes/page.test.ts` | Create | Colocated component test (same content, no `+` prefix) |
| `src-tauri/build.rs` | Modify | Add frontend existence check and auto-build |
| `.github/workflows/test.yml` | Modify | Add `npm run build` verification step |

---

### Task 1: Rename the test file

**Files:**
- Delete: `src/routes/+page.test.ts`
- Create: `src/routes/page.test.ts`

- [ ] **Step 1: Rename the file**

```bash
git mv src/routes/+page.test.ts src/routes/page.test.ts
```

- [ ] **Step 2: Verify the import path is still correct**

The test file imports `./+page.svelte` — this is the component under test, not the test's own name. Open `src/routes/page.test.ts` and confirm line 22 reads:

```typescript
import Page from './+page.svelte';
```

No change needed — the import references the component, not the test file.

- [ ] **Step 3: Verify SvelteKit build succeeds**

Run: `npm run build`
Expected: Clean build, output in `build/` directory. No "Files prefixed with + are reserved" error.

- [ ] **Step 4: Verify tests still pass**

Run: `npx vitest run`
Expected: All tests in `src/routes/page.test.ts` pass. The vitest include pattern `src/**/*.test.ts` matches the renamed file.

- [ ] **Step 5: Commit**

```bash
git add src/routes/page.test.ts
git rm src/routes/+page.test.ts 2>/dev/null || true
git commit -m "fix: rename +page.test.ts to unblock SvelteKit builds

SvelteKit reserves +-prefixed files in src/routes/. The test file
broke npm run build, causing the frontend build/ directory to go
stale and producing a WSOD in the popover."
```

---

### Task 2: Add frontend auto-build to build.rs

**Files:**
- Modify: `src-tauri/build.rs:1-3`

- [ ] **Step 1: Replace build.rs with the guarded version**

Replace the entire contents of `src-tauri/build.rs` with:

```rust
use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_root = Path::new(&manifest_dir).parent().unwrap();
    let index_html = project_root.join("build").join("index.html");

    // Rerun this script if build/index.html disappears
    println!("cargo:rerun-if-changed={}", index_html.display());

    if !index_html.exists() {
        println!("cargo:warning=Frontend build missing — running `npm run build`");
        let status = Command::new("npm")
            .args(["run", "build"])
            .current_dir(project_root)
            .status()
            .expect("Failed to execute npm run build — is Node.js installed?");
        assert!(status.success(), "npm run build failed — check frontend for errors");
    }

    tauri_build::build()
}
```

- [ ] **Step 2: Verify it works when build/ exists (no-op path)**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -i "frontend"`
Expected: No output — the check passes silently because `build/index.html` exists.

- [ ] **Step 3: Verify it works when build/ is missing (auto-build path)**

```bash
mv build build.bak
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -i "frontend"
```

Expected: Output includes `warning: Frontend build missing — running 'npm run build'` and `build/index.html` is recreated.

```bash
ls build/index.html
rm -rf build.bak
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/build.rs
git commit -m "fix: auto-build frontend in build.rs when build/ is missing

Prevents WSOD when running the binary directly (outside cargo tauri
dev/build) by checking for build/index.html and running npm run build
if absent."
```

---

### Task 3: Add frontend build step to CI

**Files:**
- Modify: `.github/workflows/test.yml:33-34`

- [ ] **Step 1: Add the npm run build step**

In `.github/workflows/test.yml`, insert a new step after `- run: npm ci` (line 33) and before `- name: Clippy` (line 35):

```yaml
      - name: Frontend build
        run: npm run build
```

The full step sequence should read:

```yaml
      - run: npm ci

      - name: Frontend build
        run: npm run build

      - name: Clippy
        run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

- [ ] **Step 2: Verify workflow syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/test.yml'))" && echo "Valid YAML"`
Expected: `Valid YAML`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/test.yml
git commit -m "ci: add frontend build step to test workflow

Catches SvelteKit build failures (like reserved +-prefixed files in
routes) on every PR before merge."
```

---

### Task 4: Final verification

- [ ] **Step 1: Run full local check**

```bash
npm run build && npx vitest run && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: All four commands succeed.
