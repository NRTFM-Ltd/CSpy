use std::collections::HashMap;
use std::sync::Mutex;
use tauri::image::Image;

pub(crate) const BAR_WIDTH: u32 = 32;
pub(crate) const ICON_HEIGHT: u32 = 40; // 20pt at 2x Retina

/// Cache of rendered icon buffers, keyed by quantised utilisation (0–20 = 5% steps).
/// Maximum 21 entries × 4 KiB = 84 KiB total — bounded, no unbounded leak.
static ICON_CACHE: Mutex<Option<HashMap<u8, &'static [u8]>>> = Mutex::new(None);

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
const GLYPH_COLON: [u8; 7] = [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000];

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
        ':' => Some(&GLYPH_COLON),
        ' ' => None,
        _ => None,
    }
}

/// Glyph render width at 3x scale.
const GLYPH_RENDER_W: u32 = 15;
/// Glyph render height at 3x scale.
const GLYPH_RENDER_H: u32 = 21;
/// Pixels between glyphs.
const CHAR_GAP: u32 = 1;
/// Pixels for a space character.
const SPACE_WIDTH: u32 = 6;
/// Gap between bar and text in pixels.
const TEXT_GAP: u32 = 4;
/// Trailing padding after text in pixels.
const TRAIL_PAD: u32 = 2;
/// Text colour: light grey, fully opaque.
const TEXT_COLOUR: (u8, u8, u8, u8) = (220, 220, 220, 255);

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
    let y_offset = (ICON_HEIGHT - text_h) / 2;

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
                        for dy in 0..3u32 {
                            for dx in 0..3u32 {
                                let px = cursor_x + glyph_col * 3 + dx;
                                let py = y_offset + glyph_row * 3 + dy;
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

/// Render raw RGBA bytes for a usage icon at the given utilisation level.
/// Width is `BAR_WIDTH` when no countdown text is given, or wider to fit text.
/// Pure function — no caching, no tauri dependency. Used directly by tests.
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
        (248, 113, 113) // Red: #f87171
    } else if quantised_util >= 0.70 {
        (251, 191, 36)  // Amber: #fbbf24
    } else {
        (74, 222, 128)  // Green: #4ade80
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn dimensions_are_32x40() {
        let rgba = render_icon_rgba(0.5, None);
        assert_eq!(rgba.len(), (32 * 40 * 4) as usize);
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
        for y in 36..ICON_HEIGHT {
            for x in 0..BAR_WIDTH {
                let (_, _, _, a) = pixel_at(&rgba, x, y, BAR_WIDTH);
                assert_eq!(a, 0, "pixel ({x},{y}) in bottom padding should be transparent");
            }
        }
    }

    #[test]
    fn icon_with_text_is_wider_than_bar() {
        let rgba = render_icon_rgba(0.5, Some("1:37"));
        let expected_width = BAR_WIDTH + TEXT_GAP + text_pixel_width("1:37") + TRAIL_PAD;
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

    #[test]
    fn glyph_coverage_all_countdown_chars() {
        for ch in "0123456789: ".chars() {
            assert!(
                glyph_for_char(ch).is_some() || ch == ' ',
                "missing glyph for '{ch}'"
            );
        }
    }

    #[test]
    fn glyphs_are_5x7() {
        for ch in "0123456789:".chars() {
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

    #[test]
    fn text_width_single_digit() {
        assert_eq!(text_pixel_width("42"), 31);
    }

    #[test]
    fn text_width_hours_and_mins() {
        assert_eq!(text_pixel_width("1:37"), 63);
    }

    #[test]
    fn text_width_empty() {
        assert_eq!(text_pixel_width(""), 0);
    }

    #[test]
    fn render_text_produces_nonzero_pixels() {
        let width: u32 = 40;
        let height: u32 = 40;
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        render_text_into(&mut rgba, width, 0, "42", (220, 220, 220, 255));
        let has_visible = rgba.chunks(4).any(|px| px[3] > 0);
        assert!(has_visible, "render_text_into should produce visible pixels");
    }

    #[test]
    fn render_text_respects_x_offset() {
        let width: u32 = 80;
        let height: u32 = 40;
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        render_text_into(&mut rgba, width, 40, "1:37", (220, 220, 220, 255));
        for y in 0..height {
            for x in 0..40u32 {
                let idx = ((y * width + x) * 4 + 3) as usize;
                assert_eq!(rgba[idx], 0, "pixel ({x},{y}) before offset should be transparent");
            }
        }
        let has_visible_after = (0..height).any(|y| {
            (40..width).any(|x| rgba[((y * width + x) * 4 + 3) as usize] > 0)
        });
        assert!(has_visible_after, "should have visible pixels after x=40");
    }
}
