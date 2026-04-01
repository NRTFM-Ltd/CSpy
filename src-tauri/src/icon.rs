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
