/// Core Mandelbrot set computation
///
/// Uses escape-time algorithm with smooth colouring

use crate::colour::colour_interior;

/// Result of computing a single Mandelbrot point
pub struct MandelbrotResult {
    /// Smooth iteration count (>= max_iterations means in set)
    pub smooth_iter: f64,
    /// Final x position of orbit (for interior colouring)
    pub final_x: f64,
    /// Final y position of orbit (for interior colouring)
    pub final_y: f64,
    /// Whether the point is in the set
    pub in_set: bool,
}

/// Compute the Mandelbrot iteration count for a single point
/// Returns smooth iteration count and final orbit position
#[inline]
pub fn mandelbrot_point(cx: f64, cy: f64, max_iterations: u32) -> MandelbrotResult {
    let mut x = 0.0_f64;
    let mut y = 0.0_f64;
    let mut x2 = 0.0_f64;
    let mut y2 = 0.0_f64;

    let mut iteration = 0u32;

    // Escape radius squared (using 256 for smooth colouring)
    const ESCAPE_RADIUS_SQ: f64 = 65536.0; // 256^2

    while x2 + y2 <= ESCAPE_RADIUS_SQ && iteration < max_iterations {
        y = 2.0 * x * y + cy;
        x = x2 - y2 + cx;
        x2 = x * x;
        y2 = y * y;
        iteration += 1;
    }

    if iteration >= max_iterations {
        // Point is in the set
        return MandelbrotResult {
            smooth_iter: max_iterations as f64,
            final_x: x,
            final_y: y,
            in_set: true,
        };
    }

    // Smooth colouring using normalised iteration count
    let log_zn = (x2 + y2).ln() / 2.0;
    let nu = (log_zn / std::f64::consts::LN_2).ln() / std::f64::consts::LN_2;

    MandelbrotResult {
        smooth_iter: iteration as f64 + 1.0 - nu,
        final_x: x,
        final_y: y,
        in_set: false,
    }
}

/// Render a horizontal strip of the Mandelbrot set
///
/// Returns RGB pixel data as a Vec<u8> (3 bytes per pixel)
pub fn render_strip(
    width: u32,
    y_start: u32,
    y_end: u32,
    total_height: u32,
    center_x: f64,
    center_y: f64,
    zoom: f64,
    max_iterations: u32,
    palette: &[(u8, u8, u8)],
    colour_interior_enabled: bool,
) -> Vec<u8> {
    let height = y_end - y_start;
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);

    // Calculate the view bounds
    // Aspect ratio preserved, width determines scale
    let aspect = total_height as f64 / width as f64;
    let view_width = 4.0 / zoom;
    let view_height = view_width * aspect;

    let x_min = center_x - view_width / 2.0;
    let y_min = center_y - view_height / 2.0;

    let x_scale = view_width / width as f64;
    let y_scale = view_height / total_height as f64;

    for py in y_start..y_end {
        for px in 0..width {
            let cx = x_min + px as f64 * x_scale;
            let cy = y_min + py as f64 * y_scale;

            let result = mandelbrot_point(cx, cy, max_iterations);

            let (r, g, b) = if result.in_set {
                if colour_interior_enabled {
                    colour_interior(result.final_x, result.final_y, palette)
                } else {
                    (0, 0, 0)
                }
            } else {
                smooth_colour(result.smooth_iter, palette)
            };

            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        }
    }

    pixels
}

/// Get a smoothly interpolated colour from the palette
fn smooth_colour(smooth_iter: f64, palette: &[(u8, u8, u8)]) -> (u8, u8, u8) {
    let palette_len = palette.len();

    // Scale and wrap the iteration count to palette indices
    let scaled = smooth_iter * 0.1; // Adjust this for colour density
    let idx1 = (scaled.floor() as usize) % palette_len;
    let idx2 = (idx1 + 1) % palette_len;
    let frac = scaled.fract();

    let (r1, g1, b1) = palette[idx1];
    let (r2, g2, b2) = palette[idx2];

    // Linear interpolation
    let r = lerp(r1, r2, frac);
    let g = lerp(g1, g2, frac);
    let b = lerp(b1, b2, frac);

    (r, g, b)
}

#[inline]
fn lerp(a: u8, b: u8, t: f64) -> u8 {
    let result = a as f64 * (1.0 - t) + b as f64 * t;
    result.clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mandelbrot_in_set() {
        // Origin is in the Mandelbrot set
        let result = mandelbrot_point(0.0, 0.0, 100);
        assert!(result.in_set);
        assert_eq!(result.smooth_iter, 100.0);
    }

    #[test]
    fn test_mandelbrot_escapes() {
        // Point well outside the set
        let result = mandelbrot_point(2.0, 2.0, 100);
        assert!(!result.in_set);
        assert!(result.smooth_iter < 10.0);
    }
}
