/// Colour palette generation for Mandelbrot visualisation

/// Generate a smooth colour palette using HSV to RGB conversion
pub fn generate_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        // Create a visually pleasing gradient using multiple sine waves
        // This produces a classic "fire" palette that looks great on Mandelbrot
        let r = (0.5 + 0.5 * (3.0 + t * 6.28318 + 0.0).sin()) * 255.0;
        let g = (0.5 + 0.5 * (3.0 + t * 6.28318 + 2.094).sin()) * 255.0;
        let b = (0.5 + 0.5 * (3.0 + t * 6.28318 + 4.188).sin()) * 255.0;

        palette.push((r as u8, g as u8, b as u8));
    }

    palette
}

/// Alternative "deep sea" palette
#[allow(dead_code)]
pub fn generate_ocean_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        // Blues and cyans with hints of purple
        let r = (0.2 + 0.3 * (t * 6.28318 + 4.0).sin()) * 255.0;
        let g = (0.3 + 0.4 * (t * 6.28318 + 2.0).sin()) * 255.0;
        let b = (0.5 + 0.5 * (t * 6.28318).sin()) * 255.0;

        palette.push((r.clamp(0.0, 255.0) as u8, g.clamp(0.0, 255.0) as u8, b.clamp(0.0, 255.0) as u8));
    }

    palette
}

/// High contrast "electric" palette
#[allow(dead_code)]
pub fn generate_electric_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        // Sharp transitions with bright colours
        let phase = t * 6.0;
        let segment = phase.floor() as i32 % 6;
        let frac = phase.fract();

        let (r, g, b) = match segment {
            0 => (255.0, frac * 255.0, 0.0),                    // Red to Yellow
            1 => ((1.0 - frac) * 255.0, 255.0, 0.0),           // Yellow to Green
            2 => (0.0, 255.0, frac * 255.0),                    // Green to Cyan
            3 => (0.0, (1.0 - frac) * 255.0, 255.0),           // Cyan to Blue
            4 => (frac * 255.0, 0.0, 255.0),                    // Blue to Magenta
            _ => (255.0, 0.0, (1.0 - frac) * 255.0),           // Magenta to Red
        };

        palette.push((r as u8, g as u8, b as u8));
    }

    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_generation() {
        let palette = generate_palette(256);
        assert_eq!(palette.len(), 256);

        // Check all values are valid RGB
        for (r, g, b) in &palette {
            assert!(*r <= 255);
            assert!(*g <= 255);
            assert!(*b <= 255);
        }
    }
}
