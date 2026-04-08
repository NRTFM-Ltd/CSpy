use std::collections::HashMap;
use std::sync::Mutex;
use tauri::image::Image;

pub(crate) const ICON_WIDTH: u32 = 32;
pub(crate) const ICON_HEIGHT: u32 = 32;

/// Cache of rendered icon buffers, keyed by quantised utilisation (0–20 = 5% steps).
/// Maximum 21 entries × 4 KiB = 84 KiB total — bounded, no unbounded leak.
static ICON_CACHE: Mutex<Option<HashMap<u8, &'static [u8]>>> = Mutex::new(None);

/// Render raw RGBA bytes for a 32×32 usage icon at the given utilisation level.
/// Pure function — no caching, no tauri dependency. Used directly by tests.
pub(crate) fn render_icon_rgba(quantised_util: f64) -> Vec<u8> {
    const BORDER: u32 = 2;
    const PADDING: u32 = 4;

    let fill_color: (u8, u8, u8) = if quantised_util >= 0.90 {
        (248, 113, 113) // Red: #f87171
    } else if quantised_util >= 0.70 {
        (251, 191, 36)  // Amber: #fbbf24
    } else {
        (74, 222, 128)  // Green: #4ade80
    };

    let outline_color: (u8, u8, u8) = (60, 60, 60);

    let inner_left = BORDER;
    let inner_right = ICON_WIDTH - BORDER;
    let inner_top = PADDING;
    let inner_bottom = ICON_HEIGHT - PADDING;
    let inner_width = inner_right - inner_left - 2 * BORDER;
    let fill_width = ((inner_width as f64 * quantised_util) as u32).min(inner_width);

    let mut rgba = vec![0u8; (ICON_WIDTH * ICON_HEIGHT * 4) as usize];

    for y in 0..ICON_HEIGHT {
        for x in 0..ICON_WIDTH {
            let pixel_idx = ((y * ICON_WIDTH + x) * 4) as usize;

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

    rgba
}

/// Generate a dynamic usage icon: hollow rectangle with coloured fill based on utilisation.
/// Renders at 32×32 for Retina crispness. macOS menu bar expects @2x icons.
///
/// Icons are cached by quantised utilisation (5% steps) so each unique level
/// is only rendered once. The leaked buffers are bounded to ~84 KiB total.
pub fn generate_usage_icon(utilisation: f64) -> Image<'static> {
    let util = utilisation.max(0.0).min(1.0);
    let key = (util * 20.0).round() as u8;

    {
        let mut guard = ICON_CACHE.lock().unwrap();
        let cache = guard.get_or_insert_with(HashMap::new);
        if let Some(rgba_ref) = cache.get(&key) {
            return Image::new(rgba_ref, ICON_WIDTH, ICON_HEIGHT);
        }
    }

    let quantised_util = key as f64 / 20.0;
    let rgba = render_icon_rgba(quantised_util);

    let rgba_static: &'static [u8] = Box::leak(rgba.into_boxed_slice());

    let mut guard = ICON_CACHE.lock().unwrap();
    let cache = guard.get_or_insert_with(HashMap::new);
    cache.insert(key, rgba_static);

    Image::new(rgba_static, ICON_WIDTH, ICON_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel_at(rgba: &[u8], x: u32, y: u32) -> (u8, u8, u8, u8) {
        let idx = ((y * ICON_WIDTH + x) * 4) as usize;
        (rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3])
    }

    fn count_interior_pixels_with_rgb(rgba: &[u8], rgb: (u8, u8, u8)) -> u32 {
        let mut count = 0;
        for y in 6..26 {
            for x in 4..28 {
                let (r, g, b, _) = pixel_at(rgba, x, y);
                if (r, g, b) == rgb {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn dimensions_are_32x32() {
        let rgba = render_icon_rgba(0.5);
        assert_eq!(rgba.len(), (32 * 32 * 4) as usize);
    }

    #[test]
    fn zero_percent_has_no_fill_pixels() {
        let rgba = render_icon_rgba(0.0);
        let green = count_interior_pixels_with_rgb(&rgba, (74, 222, 128));
        assert_eq!(green, 0, "0% should have no green fill pixels");
    }

    #[test]
    fn fifty_percent_uses_green() {
        let rgba = render_icon_rgba(0.5);
        let green = count_interior_pixels_with_rgb(&rgba, (74, 222, 128));
        let grey = count_interior_pixels_with_rgb(&rgba, (180, 180, 180));
        assert!(green > 0, "50% should have green fill pixels");
        assert!(grey > 0, "50% should have empty grey pixels too");
    }

    #[test]
    fn seventy_percent_uses_amber() {
        let rgba = render_icon_rgba(0.70);
        let amber = count_interior_pixels_with_rgb(&rgba, (251, 191, 36));
        assert!(amber > 0, "70% should use amber fill");
    }

    #[test]
    fn ninety_percent_uses_red() {
        let rgba = render_icon_rgba(0.90);
        let red = count_interior_pixels_with_rgb(&rgba, (248, 113, 113));
        assert!(red > 0, "90% should use red fill");
    }

    #[test]
    fn hundred_percent_fills_entire_interior() {
        let rgba = render_icon_rgba(1.0);
        let grey = count_interior_pixels_with_rgb(&rgba, (180, 180, 180));
        assert_eq!(grey, 0, "100% should have no empty grey pixels in interior");
    }

    #[test]
    fn padding_rows_are_transparent() {
        let rgba = render_icon_rgba(0.5);
        for y in 0..4 {
            for x in 0..ICON_WIDTH {
                let (_, _, _, a) = pixel_at(&rgba, x, y);
                assert_eq!(a, 0, "pixel ({x},{y}) in top padding should be transparent");
            }
        }
        for y in 28..ICON_HEIGHT {
            for x in 0..ICON_WIDTH {
                let (_, _, _, a) = pixel_at(&rgba, x, y);
                assert_eq!(a, 0, "pixel ({x},{y}) in bottom padding should be transparent");
            }
        }
    }
}
