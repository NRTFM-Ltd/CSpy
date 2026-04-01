# 5-Hour Dynamic Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement dynamic tray icon showing 5-hour usage as a colour-coded filled rectangle, with popover displaying burn rate indicator.

**Architecture:** Refactor icon generation to render hollow rectangles with dynamic fills based on utilisation. Backend calculates burn rate and emits via events. Frontend displays both usage bar and burn rate with colour indicators. Remove all 7-day quota references.

**Tech Stack:** Rust (icon generation via `image` crate), SvelteKit/Svelte 5 (popover + calculations), Tauri 2 (tray icon updates)

---

## File Structure

**Rust Backend:**
- `src-tauri/src/icon.rs` — Refactor to generate usage icons dynamically (replace static Owl loading)
- `src-tauri/src/lib.rs` — Update polling loop to emit burn rate in events, call icon generation

**Svelte Frontend:**
- `src/lib/types.ts` — Add tier functions (usage % + burn rate), burn rate calculator
- `src/routes/+page.svelte` — Simplify to 5-hour only, add burn rate display
- `src/app.css` — Update colour values to new thresholds (70/90)

---

## Task 1: Update TypeScript Types with Burn Rate Functions

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Read current types.ts to understand existing structure**

```bash
cat src/lib/types.ts
```

Expected: Current `tierFor()` uses thresholds ~60% and ~85%. Has interfaces `UsageData` and `UsageBucket`.

- [ ] **Step 2: Update tierFor() to new thresholds**

Replace the `tierFor()` function in `src/lib/types.ts`:

```typescript
/** Colour tier for usage percentage (0.0 to 1.0) */
export function tierFor(utilisation: number): Tier {
	if (utilisation >= 0.90) return 'red';
	if (utilisation >= 0.70) return 'amber';
	return 'green';
}
```

- [ ] **Step 3: Add burnRateTier() function**

Add this function to `src/lib/types.ts` after `tierFor()`:

```typescript
/** Colour tier for burn rate (%/hr) */
export function burnRateTier(burnRatePercent: number): Tier {
	if (burnRatePercent >= 20) return 'red';
	if (burnRatePercent >= 16) return 'amber';
	return 'green';
}
```

- [ ] **Step 4: Add calculateBurnRate() function**

Add this function to `src/lib/types.ts` after `burnRateTier()`:

```typescript
/** Calculate burn rate in percentage per hour */
export function calculateBurnRate(utilisation: number, secondsUntilReset: number): number {
	if (secondsUntilReset <= 0) return 0;
	const hoursRemaining = secondsUntilReset / 3600;
	const usagePercent = utilisation * 100;
	return usagePercent / hoursRemaining;
}
```

- [ ] **Step 5: Run type check**

```bash
npm run check
```

Expected: PASS with no TypeScript errors.

- [ ] **Step 6: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat: add burn rate tier and calculation functions"
```

---

## Task 2: Refactor icon.rs to Generate Dynamic Usage Icon

**Files:**
- Modify: `src-tauri/src/icon.rs` (complete rewrite)

- [ ] **Step 1: Replace icon.rs entirely with usage icon generator**

Replace entire contents of `src-tauri/src/icon.rs`:

```rust
use tauri::image::Image;

/// Generate a dynamic usage icon: hollow rectangle with coloured fill based on utilisation
pub fn generate_usage_icon(utilisation: f64) -> Image<'static> {
    const WIDTH: u32 = 16;
    const HEIGHT: u32 = 16;
    const BORDER: u32 = 1; // 1px outline

    // Clamp utilisation to 0.0-1.0
    let util = utilisation.max(0.0).min(1.0);

    // Determine colours based on utilisation
    let fill_color = if util >= 0.90 {
        (248, 113, 113) // Red: #f87171
    } else if util >= 0.70 {
        (251, 191, 36) // Amber: #fbbf24
    } else {
        (74, 222, 128) // Green: #4ade80
    };

    // Outline is always dark (assume light macOS mode for now; can add dark mode detection later)
    let outline_color = (0, 0, 0); // Black

    // Calculate fill width (0-14 pixels, leaving 1px border on each side)
    let inner_width = WIDTH - 2 * BORDER;
    let fill_width = ((inner_width as f64 * util) as u32).min(inner_width);

    // Generate RGBA bytes (16x16 = 256 pixels, 4 bytes each = 1024 bytes)
    let mut rgba = vec![0u8; (WIDTH * HEIGHT * 4) as usize];

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let pixel_idx = ((y * WIDTH + x) * 4) as usize;

            let (r, g, b, a) = if y < BORDER || y >= HEIGHT - BORDER || x < BORDER || x >= WIDTH - BORDER {
                // Outline: black with full alpha
                (outline_color.0, outline_color.1, outline_color.2, 255)
            } else {
                // Interior: determine if this pixel is filled or hollow
                let inner_x = x - BORDER;
                if inner_x < fill_width {
                    // Filled region: use fill colour
                    (fill_color.0, fill_color.1, fill_color.2, 255)
                } else {
                    // Hollow region: transparent white (for clean look)
                    (255, 255, 255, 0)
                }
            };

            rgba[pixel_idx] = r;
            rgba[pixel_idx + 1] = g;
            rgba[pixel_idx + 2] = b;
            rgba[pixel_idx + 3] = a;
        }
    }

    // Box::leak to convert Vec into 'static reference
    let rgba_static: &'static [u8] = Box::leak(rgba.into_boxed_slice());
    Image::new(rgba_static, WIDTH, HEIGHT)
}
```

- [ ] **Step 2: Verify file compiles**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -20
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
cd ../.. && git add src-tauri/src/icon.rs
git commit -m "refactor: replace Owl icon loading with dynamic usage icon generation"
```

---

## Task 3: Update lib.rs to Emit Burn Rate in Events

**Files:**
- Modify: `src-tauri/src/lib.rs:79-103` (update polling loop)

- [ ] **Step 1: Read current polling loop structure**

```bash
sed -n '79,103p' src-tauri/src/lib.rs
```

Expected: `start_polling()` function with success/error branches.

- [ ] **Step 2: Add burn rate calculation to success branch**

In the success branch of `start_polling()` (around line 89), after `fetch_usage()` succeeds, calculate burn rate and emit it:

Replace this section in `src-tauri/src/lib.rs`:

```rust
            match usage::fetch_usage(&token).await {
                Ok(data) => {
                    update_tray_tooltip(&app_handle, &data);
                    *state.cached.write().await = Some(data.clone());
                    let _ = app_handle.emit("usage-updated", &data);
                }
```

With:

```rust
            match usage::fetch_usage(&token).await {
                Ok(data) => {
                    // Regenerate tray icon based on utilisation
                    if let Some(bucket) = &data.five_hour {
                        let new_icon = icon::generate_usage_icon(bucket.utilisation);
                        if let Some(tray) = app_handle.tray_by_id("cspy-tray") {
                            let _ = tray.set_icon(Some(new_icon));
                        }
                    }

                    update_tray_tooltip(&app_handle, &data);
                    *state.cached.write().await = Some(data.clone());
                    let _ = app_handle.emit("usage-updated", &data);
                }
```

- [ ] **Step 3: Update tray icon on initial fetch**

Find the initial fetch block (around line 170), and add icon generation there too:

Replace:

```rust
                    match usage::fetch_usage(&token).await {
                        Ok(data) => {
                            update_tray_tooltip(&h, &data);
                            *s.cached.write().await = Some(data.clone());
                            let _ = h.emit("usage-updated", &data);
                        }
```

With:

```rust
                    match usage::fetch_usage(&token).await {
                        Ok(data) => {
                            // Regenerate tray icon
                            if let Some(bucket) = &data.five_hour {
                                let new_icon = icon::generate_usage_icon(bucket.utilisation);
                                if let Some(tray) = h.tray_by_id("cspy-tray") {
                                    let _ = tray.set_icon(Some(new_icon));
                                }
                            }

                            update_tray_tooltip(&h, &data);
                            *s.cached.write().await = Some(data.clone());
                            let _ = h.emit("usage-updated", &data);
                        }
```

- [ ] **Step 4: Verify compile**

```bash
cd src-tauri && cargo build --lib 2>&1 | grep -E "error|warning" | head -10
```

Expected: No errors (warnings about unused imports are OK for now).

- [ ] **Step 5: Commit**

```bash
cd ../.. && git add src-tauri/src/lib.rs
git commit -m "feat: regenerate tray icon on each poll with dynamic fill"
```

---

## Task 4: Simplify Popover to 5-Hour Only and Add Burn Rate Display

**Files:**
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Read current popover to understand structure**

```bash
head -70 src/routes/+page.svelte | tail -30
```

Expected: Two sections (5-hour and 7-day) with progress bars.

- [ ] **Step 2: Import burn rate functions in script**

In the `<script>` block at the top, update imports:

Replace:

```typescript
import { type UsageData, type Tier, tierFor, formatCountdown } from '$lib/types';
```

With:

```typescript
import { type UsageData, type Tier, tierFor, burnRateTier, calculateBurnRate, formatCountdown } from '$lib/types';
```

- [ ] **Step 3: Add burn rate state to script**

In the `<script>` block, after the existing state declarations, add:

```typescript
let burnRate = $state(0); // %/hr
```

- [ ] **Step 4: Calculate burn rate in onMount**

In the `onMount` function, after the `usage = await invoke<UsageData>('get_usage')` line, add:

```typescript
if (usage?.five_hour?.resets_at) {
    const resetTime = new Date(usage.five_hour.resets_at).getTime();
    const secondsUntilReset = Math.max(0, (resetTime - Date.now()) / 1000);
    burnRate = calculateBurnRate(usage.five_hour?.utilisation ?? 0, secondsUntilReset);
}
```

- [ ] **Step 5: Recalculate burn rate on countdown tick**

In the `ticker = setInterval(...)` callback, update it to also recalculate burn rate:

Replace:

```typescript
ticker = setInterval(() => { countdownKey++; }, 30_000);
```

With:

```typescript
ticker = setInterval(() => {
    countdownKey++;
    // Recalculate burn rate as time passes
    if (usage?.five_hour?.resets_at) {
        const resetTime = new Date(usage.five_hour.resets_at).getTime();
        const secondsUntilReset = Math.max(0, (resetTime - Date.now()) / 1000);
        burnRate = calculateBurnRate(usage.five_hour?.utilisation ?? 0, secondsUntilReset);
    }
}, 30_000);
```

- [ ] **Step 6: Update event listener for burn rate**

In the `unlisten = await listen<UsageData>('usage-updated', ...)` callback, recalculate burn rate:

Replace:

```typescript
unlisten = await listen<UsageData>('usage-updated', (event) => {
    usage = event.payload;
    error = null;
    loading = false;
});
```

With:

```typescript
unlisten = await listen<UsageData>('usage-updated', (event) => {
    usage = event.payload;
    error = null;
    loading = false;
    // Recalculate burn rate from new data
    if (usage?.five_hour?.resets_at) {
        const resetTime = new Date(usage.five_hour.resets_at).getTime();
        const secondsUntilReset = Math.max(0, (resetTime - Date.now()) / 1000);
        burnRate = calculateBurnRate(usage.five_hour?.utilisation ?? 0, secondsUntilReset);
    }
});
```

- [ ] **Step 7: Replace popover markup to remove 7-day section**

In the HTML markup, find and remove the entire 7-day section (looks for `<!-- 7-day window -->`). Keep only the 5-hour section plus add burn rate display.

Replace the entire content block between `{#if error && !usage}` and the footer with:

```svelte
{#if error && !usage}
    <div class="error-box">
        <span class="error-icon">⚠</span>
        <span>{error}</span>
    </div>
{:else if usage}
    {#key countdownKey}
        <!-- 5-hour window only -->
        <section class="bucket">
            <div class="bucket-header">
                <span class="bucket-label">5-hour quota</span>
                <span class="mono {tier(usage.five_hour?.utilisation ?? 0)}">
                    {pct(usage.five_hour?.utilisation ?? 0)}
                </span>
            </div>
            <div class="bar-track">
                <div
                    class="bar-fill {tier(usage.five_hour?.utilisation ?? 0)}"
                    style="width: {pct(usage.five_hour?.utilisation ?? 0)}"
                ></div>
            </div>
            <div class="bucket-footer dim mono">
                Resets in {formatCountdown(usage.five_hour?.resets_at ?? null)}
            </div>
        </section>

        <!-- Burn rate indicator -->
        <section class="burn-rate">
            <div class="burn-rate-label">Burn rate</div>
            <div class="burn-rate-display">
                <span class="burn-rate-value">{burnRate.toFixed(1)}%/hr</span>
                <span class="burn-rate-dot {burnRateTier(burnRate)}"></span>
            </div>
        </section>
    {/key}

    {#if error}
        <div class="stale-warning dim mono">⚠ Last refresh failed — showing cached data</div>
    {/if}
{:else}
    <div class="loading">Reading Keychain…</div>
{/if}
```

- [ ] **Step 8: Verify popover structure compiles**

```bash
npm run check
```

Expected: PASS with no TypeScript errors.

- [ ] **Step 9: Commit**

```bash
git add src/routes/+page.svelte
git commit -m "feat: simplify popover to 5-hour only, add burn rate display"
```

---

## Task 5: Update CSS for New Colour Thresholds and Burn Rate Styles

**Files:**
- Modify: `src/app.css`

- [ ] **Step 1: Update colour variable thresholds if they exist**

Check if `app.css` has explicit colour definitions and update them to match the new thresholds (70/90). Find the section with bar fill colours:

Find this block:

```css
.bar-fill.green  { background: var(--green); }
.bar-fill.amber  { background: var(--amber); }
.bar-fill.red    { background: var(--red); }
```

It should already be there (unchanged). Add burn rate dot styles after it:

```css
.burn-rate-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
}

.burn-rate-dot.green { background: var(--green); }
.burn-rate-dot.amber { background: var(--amber); }
.burn-rate-dot.red { background: var(--red); }
```

- [ ] **Step 2: Add burn rate section styles**

Add this to `app.css` (at the end or in a logical section):

```css
.burn-rate {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 0;
    font-size: 12px;
    border-top: 1px solid var(--bar-bg);
}

.burn-rate-label {
    color: var(--text-dim);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.burn-rate-display {
    display: flex;
    gap: 6px;
    align-items: center;
}

.burn-rate-value {
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
}
```

- [ ] **Step 3: Check CSS compiles**

```bash
npm run check
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/app.css
git commit -m "style: add burn rate indicator styles, update colour thresholds"
```

---

## Task 6: End-to-End Integration Test

**Files:**
- Test: Manual smoke test (no automated tests yet)

- [ ] **Step 1: Kill any running dev instances**

```bash
pkill -9 cspy; pkill -9 vite; sleep 2
```

- [ ] **Step 2: Start dev server**

```bash
cargo tauri dev 2>&1 &
sleep 30
```

Expected: Build succeeds, Vite server starts on port 1420, Rust binary runs.

- [ ] **Step 3: Verify tray icon is visible and updating**

Look at the menu bar. You should see the Owl icon (or an icon) with a visual fill.

Expected: Icon has a hollow rectangle with a coloured fill (green if usage <70%).

- [ ] **Step 4: Click the icon and check popover**

Click the Owl in the menu bar.

Expected: A popover appears showing:
- "5-hour quota" label
- Progress bar matching the icon fill colour
- "XX% used · Resets in Xh Ym"
- "Burn rate: XX.X%/hr [●]" with a coloured dot

- [ ] **Step 5: Verify 7-day section is gone**

Popover should only show 5-hour info, no 7-day section.

Expected: Only one progress bar, one countdown, one burn rate indicator.

- [ ] **Step 6: Wait 30 seconds and verify countdown updates**

The countdown should decrease.

Expected: "Resets in Xh Ym" updates to a lower value.

- [ ] **Step 7: Check colour changes (manual)**

If usage is at different levels, icon fill and bar should be:
- Green if < 70%
- Amber if 70–89%
- Red if ≥ 90%

Expected: Colours match thresholds.

- [ ] **Step 8: Final commit (cleanup)**

```bash
pkill -9 cspy; pkill -9 vite
git status
```

Expected: Clean working tree (no uncommitted changes).

---

## Summary

This plan implements:

1. ✅ **Dynamic tray icon** — hollow rectangle with colour-coded fill (green/amber/red)
2. ✅ **Popover display** — 5-hour only, showing usage bar + percentage + reset time
3. ✅ **Burn rate indicator** — calculates %/hr, displays with colour warning
4. ✅ **Removed 7-day quota** — entirely removed from UI
5. ✅ **Updated thresholds** — 70% (amber), 90% (red) for usage; 16%, 20% for burn rate

**Total commits:** 5 (types, icon refactor, polling loop, popover, CSS)

**Files modified:** 5 (types.ts, icon.rs, lib.rs, +page.svelte, app.css)

**Testing:** Manual smoke test (icon visible, popover shows correct data, colours update)
