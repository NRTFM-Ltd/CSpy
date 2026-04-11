# Icon Text Countdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the 5-hour countdown text directly into the tray icon's RGBA buffer, bypassing the broken macOS 26 `set_title()` API.

**Architecture:** Extend `icon.rs` with a 5x7 bitmap font (13 characters: 0-9, h, m, space) rendered at 2x scale into a variable-width icon. The bar occupies the left 32px (unchanged), text appears to its right. `lib.rs` replaces all `set_title` calls with icon regeneration that includes countdown text. When no active window exists, the icon reverts to the existing 32x32 bar-only format.

**Tech Stack:** Pure Rust, no new crates. Pixel-level RGBA rendering.

---

## File Structure

- **Modify:** `src-tauri/src/icon.rs` — bitmap font data, text rendering, variable-width icon
- **Modify:** `src-tauri/src/lib.rs` — replace `set_title` with icon-based countdown, add `update_tray_icon` helper

---

### Task 1: Add Bitmap Font Glyph Data

**Files:**
- Modify: `src-tauri/src/icon.rs:1-10`

- [ ] **Step 1: Write the failing test**

Add at the bottom of the `mod tests` block in `icon.rs`:

```rust
#[test]
fn glyph_coverage_all_countdown_chars() {
    for ch in "0123456789hm ".chars() {
        assert!(
            glyph_for_char(ch).is_some(),
            "missing glyph for '{ch}'"
        );
    }
}

#[test]
fn glyphs_are_5x7() {
    for ch in "0123456789hm".chars() {
        let glyph = glyph_for_char(ch).unwrap();
        assert_eq!(glyph.len(), 7, "glyph for '{ch}' should have 7 rows");
        for (row_idx, row) in glyph.iter().enumerate() {
            assert!(
                *row < 32,
                "glyph '{ch}' row {row_idx} uses more than 5 bits: {row:#010b}"
            );
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib icon::tests::glyph_coverage -- --nocapture`
Expected: FAIL — `glyph_for_char` not found

- [ ] **Step 3: Write the glyph data and lookup function**

Add after the `ICON_CACHE` static in `icon.rs` (after line 10):

```rust
/// 5x7 bitmap font for countdown text. Each glyph is 7 rows of 5 bits.
/// Bit 4 = leftmost pixel, bit 0 = rightmost pixel.
const GLYPH_0: [u8; 7] = [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110];
const GLYPH_1: [u8; 7] = [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110];
const GLYPH_2: [u8; 7] = [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111];
const GLYPH_3: [u8; 7] = [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110];
const GLYPH_4: [u8; 7] = [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010];
const GLYPH_5: [u8; 7] = [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110];
const GLYPH_6: [u8; 7] = [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110];
const GLYPH_7: [u8; 7] = [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000];
const GLYPH_8: [u8; 7] = [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110];
const GLYPH_9: [u8; 7] = [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110];
const GLYPH_H: [u8; 7] = [0b10000, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001];
const GLYPH_M: [u8; 7] = [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101];

/// Return the 5x7 glyph for a character, or None for space/unknown.
fn glyph_for_char(ch: char) -> Option<&'static [u8; 7]> {
    match ch {
        '0' => Some(&GLYPH_0),
        '1' => Some(&GLYPH_1),
        '2' => Some(&GLYPH_2),
        '3' => Some(&GLYPH_3),
        '4' => Some(&GLYPH_4),
        '5' => Some(&GLYPH_5),
        '6' => Some(&GLYPH_6),
        '7' => Some(&GLYPH_7),
        '8' => Some(&GLYPH_8),
        '9' => Some(&GLYPH_9),
        'h' => Some(&GLYPH_H),
        'm' => Some(&GLYPH_M),
        ' ' => None, // space is a gap, not a glyph
        _ => None,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib icon::tests::glyph -- --nocapture`
Expected: PASS — both `glyph_coverage_all_countdown_chars` and `glyphs_are_5x7`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/icon.rs
git commit -m "feat(icon): add 5x7 bitmap font glyphs for countdown text"
```

---

### Task 2: Text Width Measurement

**Files:**
- Modify: `src-tauri/src/icon.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn text_width_single_digit() {
    // "5m" = glyph(10) + gap(1) + glyph(10) = 21
    assert_eq!(text_pixel_width("5m"), 21);
}

#[test]
fn text_width_hours_and_mins() {
    // "3h 42m" = g(10)+gap(1)+g(10)+space(6)+g(10)+gap(1)+g(10)+gap(1)+g(10) = 59
    assert_eq!(text_pixel_width("3h 42m"), 59);
}

#[test]
fn text_width_empty() {
    assert_eq!(text_pixel_width(""), 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib icon::tests::text_width -- --nocapture`
Expected: FAIL — `text_pixel_width` not found

- [ ] **Step 3: Write the implementation**

Add after `glyph_for_char` in `icon.rs`:

```rust
/// Glyph render width at 2x scale.
const GLYPH_RENDER_W: u32 = 10; // 5px * 2
/// Glyph render height at 2x scale.
const GLYPH_RENDER_H: u32 = 14; // 7px * 2
/// Pixels between glyphs.
const CHAR_GAP: u32 = 1;
/// Pixels for a space character.
const SPACE_WIDTH: u32 = 6;

/// Calculate the total pixel width of rendered text.
fn text_pixel_width(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let mut width: u32 = 0;
    let mut first = true;
    for ch in text.chars() {
        if !first && ch != ' ' {
            width += CHAR_GAP;
        }
        first = false;
        if ch == ' ' {
            width += SPACE_WIDTH;
        } else {
            width += GLYPH_RENDER_W;
        }
    }
    width
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib icon::tests::text_width -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/icon.rs
git commit -m "feat(icon): add text width measurement for countdown rendering"
```

---

### Task 3: Text Rendering into RGBA Buffer

**Files:**
- Modify: `src-tauri/src/icon.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn render_text_produces_nonzero_pixels() {
    let width: u32 = 40;
    let height: u32 = 32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    render_text_into(&mut rgba, width, 0, "5m", (220, 220, 220, 255));
    // At least some pixels should be non-transparent
    let has_visible = rgba.chunks(4).any(|px| px[3] > 0);
    assert!(has_visible, "render_text_into should produce visible pixels");
}

#[test]
fn render_text_respects_x_offset() {
    let width: u32 = 80;
    let height: u32 = 32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    render_text_into(&mut rgba, width, 40, "1m", (220, 220, 220, 255));
    // No visible pixels before x=40
    for y in 0..height {
        for x in 0..40u32 {
            let idx = ((y * width + x) * 4 + 3) as usize;
            assert_eq!(rgba[idx], 0, "pixel ({x},{y}) before offset should be transparent");
        }
    }
    // Some visible pixels at x>=40
    let has_visible_after = (0..height).any(|y| {
        (40..width).any(|x| rgba[((y * width + x) * 4 + 3) as usize] > 0)
    });
    assert!(has_visible_after, "should have visible pixels after x=40");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib icon::tests::render_text -- --nocapture`
Expected: FAIL — `render_text_into` not found

- [ ] **Step 3: Write the implementation**

Add after `text_pixel_width` in `icon.rs`:

```rust
/// Render countdown text into an RGBA buffer at the given x offset.
/// Glyphs are drawn at 2x scale, vertically centred in ICON_HEIGHT.
fn render_text_into(
    rgba: &mut [u8],
    buf_width: u32,
    x_start: u32,
    text: &str,
    colour: (u8, u8, u8, u8),
) {
    let text_h = GLYPH_RENDER_H;
    let y_offset = (ICON_HEIGHT - text_h) / 2; // vertically centre

    let mut cursor_x = x_start;
    let mut first = true;

    for ch in text.chars() {
        if ch == ' ' {
            cursor_x += SPACE_WIDTH;
            first = false;
            continue;
        }
        if !first {
            cursor_x += CHAR_GAP;
        }
        first = false;

        if let Some(glyph) = glyph_for_char(ch) {
            for glyph_row in 0..7u32 {
                let row_bits = glyph[glyph_row as usize];
                for glyph_col in 0..5u32 {
                    if (row_bits >> (4 - glyph_col)) & 1 == 1 {
                        // Scale 2x: each glyph pixel becomes a 2x2 block
                        for dy in 0..2u32 {
                            for dx in 0..2u32 {
                                let px = cursor_x + glyph_col * 2 + dx;
                                let py = y_offset + glyph_row * 2 + dy;
                                if px < buf_width && py < ICON_HEIGHT {
                                    let idx = ((py * buf_width + px) * 4) as usize;
                                    rgba[idx] = colour.0;
                                    rgba[idx + 1] = colour.1;
                                    rgba[idx + 2] = colour.2;
                                    rgba[idx + 3] = colour.3;
                                }
                            }
                        }
                    }
                }
            }
            cursor_x += GLYPH_RENDER_W;
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib icon::tests::render_text -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/icon.rs
git commit -m "feat(icon): add text rendering into RGBA buffer"
```

---

### Task 4: Variable-Width Icon Rendering

**Files:**
- Modify: `src-tauri/src/icon.rs:5-63` (rename constant, change signature, extend rendering)

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn icon_with_text_is_wider_than_bar() {
    let rgba = render_icon_rgba(0.5, Some("3h 42m"));
    let expected_width = BAR_WIDTH + TEXT_GAP + text_pixel_width("3h 42m") + TRAIL_PAD;
    assert_eq!(
        rgba.len(),
        (expected_width * ICON_HEIGHT * 4) as usize,
        "icon with text should be {expected_width}px wide"
    );
}

#[test]
fn icon_without_text_is_bar_width() {
    let rgba = render_icon_rgba(0.5, None);
    assert_eq!(
        rgba.len(),
        (BAR_WIDTH * ICON_HEIGHT * 4) as usize,
        "icon without text should be {BAR_WIDTH}px wide"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib icon::tests::icon_with -- --nocapture`
Expected: FAIL — wrong number of arguments to `render_icon_rgba`

- [ ] **Step 3: Rename ICON_WIDTH and update render_icon_rgba signature**

Rename `ICON_WIDTH` to `BAR_WIDTH` at line 5:

```rust
pub(crate) const BAR_WIDTH: u32 = 32;
```

Add layout constants after `SPACE_WIDTH`:

```rust
/// Gap between bar and text in pixels.
const TEXT_GAP: u32 = 4;
/// Trailing padding after text in pixels.
const TRAIL_PAD: u32 = 2;
/// Text colour: light grey, fully opaque.
const TEXT_COLOUR: (u8, u8, u8, u8) = (220, 220, 220, 255);
```

Change `render_icon_rgba` signature and body:

```rust
/// Render raw RGBA bytes for a usage icon at the given utilisation level.
/// Width is 32px (bar only) when `countdown` is `None`, or wider when text is present.
/// Pure function — no caching, no tauri dependency.
pub(crate) fn render_icon_rgba(quantised_util: f64, countdown: Option<&str>) -> Vec<u8> {
    const BORDER: u32 = 2;
    const PADDING: u32 = 4;

    let tw = countdown.map(|t| text_pixel_width(t)).unwrap_or(0);
    let total_width = if tw > 0 {
        BAR_WIDTH + TEXT_GAP + tw + TRAIL_PAD
    } else {
        BAR_WIDTH
    };

    let fill_color: (u8, u8, u8) = if quantised_util >= 0.90 {
        (248, 113, 113)
    } else if quantised_util >= 0.70 {
        (251, 191, 36)
    } else {
        (74, 222, 128)
    };

    let outline_color: (u8, u8, u8) = (60, 60, 60);

    let inner_left = BORDER;
    let inner_right = BAR_WIDTH - BORDER;
    let inner_top = PADDING;
    let inner_bottom = ICON_HEIGHT - PADDING;
    let inner_width = inner_right - inner_left - 2 * BORDER;
    let fill_width = ((inner_width as f64 * quantised_util) as u32).min(inner_width);

    let mut rgba = vec![0u8; (total_width * ICON_HEIGHT * 4) as usize];

    // Render bar in leftmost BAR_WIDTH columns
    for y in 0..ICON_HEIGHT {
        for x in 0..BAR_WIDTH {
            let pixel_idx = ((y * total_width + x) * 4) as usize;

            let (r, g, b, a) = if y < inner_top || y >= inner_bottom {
                (0, 0, 0, 0)
            } else if x < inner_left + BORDER || x >= inner_right - BORDER
                || y < inner_top + BORDER || y >= inner_bottom - BORDER {
                (outline_color.0, outline_color.1, outline_color.2, 255)
            } else {
                let inner_x = x - inner_left - BORDER;
                if inner_x < fill_width {
                    (fill_color.0, fill_color.1, fill_color.2, 255)
                } else {
                    (180, 180, 180, 80)
                }
            };

            rgba[pixel_idx] = r;
            rgba[pixel_idx + 1] = g;
            rgba[pixel_idx + 2] = b;
            rgba[pixel_idx + 3] = a;
        }
    }

    // Render countdown text to the right of the bar
    if let Some(text) = countdown {
        render_text_into(&mut rgba, total_width, BAR_WIDTH + TEXT_GAP, text, TEXT_COLOUR);
    }

    rgba
}
```

- [ ] **Step 4: Update existing test helpers**

The `pixel_at` helper in tests uses `ICON_WIDTH` which is now `BAR_WIDTH`. Update it to accept a width parameter, and update `count_interior_pixels_with_rgb` similarly:

```rust
fn pixel_at(rgba: &[u8], x: u32, y: u32, row_width: u32) -> (u8, u8, u8, u8) {
    let idx = ((y * row_width + x) * 4) as usize;
    (rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3])
}

fn count_interior_pixels_with_rgb(rgba: &[u8], rgb: (u8, u8, u8), row_width: u32) -> u32 {
    let mut count = 0;
    for y in 6..26 {
        for x in 4..28 {
            let (r, g, b, _) = pixel_at(rgba, x, y, row_width);
            if (r, g, b) == rgb {
                count += 1;
            }
        }
    }
    count
}
```

Update all existing tests to pass `None` as the countdown and `BAR_WIDTH` as the row width. For example:

```rust
#[test]
fn dimensions_are_32x32() {
    let rgba = render_icon_rgba(0.5, None);
    assert_eq!(rgba.len(), (32 * 32 * 4) as usize);
}

#[test]
fn zero_percent_has_no_fill_pixels() {
    let rgba = render_icon_rgba(0.0, None);
    let green = count_interior_pixels_with_rgb(&rgba, (74, 222, 128), BAR_WIDTH);
    assert_eq!(green, 0, "0% should have no green fill pixels");
}

#[test]
fn fifty_percent_uses_green() {
    let rgba = render_icon_rgba(0.5, None);
    let green = count_interior_pixels_with_rgb(&rgba, (74, 222, 128), BAR_WIDTH);
    let grey = count_interior_pixels_with_rgb(&rgba, (180, 180, 180), BAR_WIDTH);
    assert!(green > 0, "50% should have green fill pixels");
    assert!(grey > 0, "50% should have empty grey pixels too");
}

#[test]
fn seventy_percent_uses_amber() {
    let rgba = render_icon_rgba(0.70, None);
    let amber = count_interior_pixels_with_rgb(&rgba, (251, 191, 36), BAR_WIDTH);
    assert!(amber > 0, "70% should use amber fill");
}

#[test]
fn ninety_percent_uses_red() {
    let rgba = render_icon_rgba(0.90, None);
    let red = count_interior_pixels_with_rgb(&rgba, (248, 113, 113), BAR_WIDTH);
    assert!(red > 0, "90% should use red fill");
}

#[test]
fn hundred_percent_fills_entire_interior() {
    let rgba = render_icon_rgba(1.0, None);
    let grey = count_interior_pixels_with_rgb(&rgba, (180, 180, 180), BAR_WIDTH);
    assert_eq!(grey, 0, "100% should have no empty grey pixels in interior");
}

#[test]
fn padding_rows_are_transparent() {
    let rgba = render_icon_rgba(0.5, None);
    for y in 0..4 {
        for x in 0..BAR_WIDTH {
            let (_, _, _, a) = pixel_at(&rgba, x, y, BAR_WIDTH);
            assert_eq!(a, 0, "pixel ({x},{y}) in top padding should be transparent");
        }
    }
    for y in 28..ICON_HEIGHT {
        for x in 0..BAR_WIDTH {
            let (_, _, _, a) = pixel_at(&rgba, x, y, BAR_WIDTH);
            assert_eq!(a, 0, "pixel ({x},{y}) in bottom padding should be transparent");
        }
    }
}
```

- [ ] **Step 5: Run all tests**

Run: `cd src-tauri && cargo test --lib icon::tests -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/icon.rs
git commit -m "feat(icon): variable-width icon rendering with countdown text"
```

---

### Task 5: Update generate_usage_icon and Cache

**Files:**
- Modify: `src-tauri/src/icon.rs:70-92` (the `generate_usage_icon` function)

- [ ] **Step 1: Update the function signature and cache logic**

Bar-only icons (no text) use the existing cache, bounded to 21 entries. Text icons are rendered fresh each call and leaked. The leak rate is ~12 KB/min during an active 5-hour window; the app restarts during quiet hours, bounding total growth to ~3.6 MB per window.

```rust
/// Generate a dynamic usage icon with optional countdown text.
/// Bar-only icons (countdown=None) are cached by quantised utilisation (21 entries max).
/// Text icons are rendered fresh each call — the leaked buffers are bounded by the
/// 5-hour window duration (~3.6 MB max) and reclaimed on app restart.
pub fn generate_usage_icon(utilisation: f64, countdown: Option<&str>) -> Image<'static> {
    let util = utilisation.clamp(0.0, 1.0);
    let key = (util * 20.0).round() as u8;

    // Bar-only: use cache
    if countdown.is_none() {
        let mut guard = ICON_CACHE.lock().unwrap();
        let cache = guard.get_or_insert_with(HashMap::new);
        if let Some(rgba_ref) = cache.get(&key) {
            return Image::new(rgba_ref, BAR_WIDTH, ICON_HEIGHT);
        }

        let quantised_util = key as f64 / 20.0;
        let rgba = render_icon_rgba(quantised_util, None);
        let rgba_static: &'static [u8] = Box::leak(rgba.into_boxed_slice());
        cache.insert(key, rgba_static);
        return Image::new(rgba_static, BAR_WIDTH, ICON_HEIGHT);
    }

    // Text icon: render fresh, leak
    let quantised_util = key as f64 / 20.0;
    let text = countdown.unwrap();
    let tw = text_pixel_width(text);
    let total_width = BAR_WIDTH + TEXT_GAP + tw + TRAIL_PAD;
    let rgba = render_icon_rgba(quantised_util, countdown);
    let rgba_static: &'static [u8] = Box::leak(rgba.into_boxed_slice());
    Image::new(rgba_static, total_width, ICON_HEIGHT)
}
```

- [ ] **Step 2: Run all icon tests**

Run: `cd src-tauri && cargo test --lib icon -- --nocapture`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/icon.rs
git commit -m "feat(icon): update generate_usage_icon for text icons"
```

---

### Task 6: Wire Up lib.rs — Replace set_title With Icon Rendering

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add update_tray_icon helper**

Add this function after `is_window_expired` (around line 534), replacing `update_tray_title`:

```rust
/// Regenerate the tray icon with the current utilisation and countdown text baked in.
/// Replaces the former `update_tray_title` + separate icon update.
fn update_tray_icon(app: &tauri::AppHandle, data: &UsageData) {
    let (util, cd_string) = match &data.five_hour {
        Some(bucket) if !is_window_expired(bucket) => {
            let cd = bucket.resets_at.as_deref()
                .map(format_countdown)
                .filter(|s| s != "\u{2014}"); // filter out em dash (expired)
            (bucket.utilisation, cd)
        }
        _ => (0.0, None),
    };
    let new_icon = icon::generate_usage_icon(util, cd_string.as_deref());
    if let Some(tray) = app.tray_by_id("cspy-tray") {
        let _ = tray.set_icon(Some(new_icon));
    }
}
```

- [ ] **Step 2: Delete update_tray_title**

Remove the entire `update_tray_title` function (lines 554-564 approximately — the function with the `set_title` call and the diagnostic log line).

- [ ] **Step 3: Update start_countdown_ticker**

Replace the body of `start_countdown_ticker` with:

```rust
fn start_countdown_ticker(app_handle: tauri::AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if let Some(data) = state.cached.read().await.as_ref() {
                update_tray_icon(&app_handle, data);
            }
        }
    });
}
```

This replaces both the old `update_tray_title` call and the separate `is_window_expired` icon reset — `update_tray_icon` handles both cases.

- [ ] **Step 4: Update the polling loop success path**

In `start_polling`, replace the icon + title update block (around lines 264-273) with:

```rust
Ok(data) => {
    if consecutive_errors > 0 {
        log::info!("Poll succeeded after {} consecutive error(s) — backoff reset",
            consecutive_errors);
    }
    consecutive_errors = 0;

    update_tray_icon(&app_handle, &data);
    update_tray_tooltip(&app_handle, &data);
    *state.cached.write().await = Some(data.clone());
    let _ = app_handle.emit("usage-updated", &data);
}
```

- [ ] **Step 5: Update the initial fetch in setup**

In the setup closure's immediate-fetch block (around lines 660-674), replace the icon + title update with:

```rust
Ok(data) => {
    update_tray_icon(&h, &data);
    update_tray_tooltip(&h, &data);
    *s.cached.write().await = Some(data.clone());
    let _ = h.emit("usage-updated", &data);
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: No errors. Warnings about unused `format_countdown` parameter are OK at this stage (it is still used by `update_tray_icon` via the new helper).

- [ ] **Step 7: Run all tests**

Run: `cd src-tauri && cargo test -- --nocapture`
Expected: ALL PASS (lib.rs tests for `format_countdown`, `is_window_expired`, quiet hours, backoff, heartbeat all unchanged)

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: replace set_title with icon-rendered countdown text

Bypasses broken macOS 26 set_title() by baking countdown text
directly into the tray icon pixels via update_tray_icon helper."
```

---

### Task 7: Build and Verify

**Files:**
- None (manual verification)

- [ ] **Step 1: Run cargo tauri dev**

Run: `cargo tauri dev`

Expected: app builds, tray icon appears in menu bar with both the coloured bar AND countdown text (e.g. "3h 42m") rendered as part of the icon itself.

- [ ] **Step 2: Verify countdown updates**

Wait 60 seconds. The countdown text in the icon should tick down by 1 minute.

- [ ] **Step 3: Verify expired state**

If the window is expired (or you can temporarily hack `format_countdown` to return "---"), the icon should revert to the narrow 32px bar-only format with no text.

- [ ] **Step 4: Commit final**

```bash
git add -A
git commit -m "chore: verified icon countdown rendering on macOS 26"
```
