/// Colour palette generation for Mandelbrot visualisation

use serde::{Deserialize, Serialize};

/// Available colour palettes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Palette {
    #[default]
    Fire,
    Ocean,
    Electric,
    Monochrome,
    Rainbow,
    Twilight,
    Forest,
    Lava,
}

impl Palette {
    pub fn generate(&self, num_colours: usize) -> Vec<(u8, u8, u8)> {
        match self {
            Palette::Fire => generate_fire_palette(num_colours),
            Palette::Ocean => generate_ocean_palette(num_colours),
            Palette::Electric => generate_electric_palette(num_colours),
            Palette::Monochrome => generate_monochrome_palette(num_colours),
            Palette::Rainbow => generate_rainbow_palette(num_colours),
            Palette::Twilight => generate_twilight_palette(num_colours),
            Palette::Forest => generate_forest_palette(num_colours),
            Palette::Lava => generate_lava_palette(num_colours),
        }
    }

    pub fn all() -> &'static [Palette] {
        &[
            Palette::Fire,
            Palette::Ocean,
            Palette::Electric,
            Palette::Monochrome,
            Palette::Rainbow,
            Palette::Twilight,
            Palette::Forest,
            Palette::Lava,
        ]
    }
}

/// Classic fire palette - reds, oranges, yellows
fn generate_fire_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        let r = (0.5 + 0.5 * (3.0 + t * 6.28318 + 0.0).sin()) * 255.0;
        let g = (0.5 + 0.5 * (3.0 + t * 6.28318 + 2.094).sin()) * 255.0;
        let b = (0.5 + 0.5 * (3.0 + t * 6.28318 + 4.188).sin()) * 255.0;

        palette.push((r as u8, g as u8, b as u8));
    }

    palette
}

/// Deep sea palette - blues and cyans
fn generate_ocean_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        let r = (0.1 + 0.2 * (t * 6.28318 + 4.0).sin()) * 255.0;
        let g = (0.3 + 0.4 * (t * 6.28318 + 2.0).sin()) * 255.0;
        let b = (0.5 + 0.5 * (t * 6.28318).sin()) * 255.0;

        palette.push((
            r.clamp(0.0, 255.0) as u8,
            g.clamp(0.0, 255.0) as u8,
            b.clamp(0.0, 255.0) as u8,
        ));
    }

    palette
}

/// High contrast electric palette
fn generate_electric_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        let phase = t * 6.0;
        let segment = phase.floor() as i32 % 6;
        let frac = phase.fract();

        let (r, g, b) = match segment {
            0 => (255.0, frac * 255.0, 0.0),
            1 => ((1.0 - frac) * 255.0, 255.0, 0.0),
            2 => (0.0, 255.0, frac * 255.0),
            3 => (0.0, (1.0 - frac) * 255.0, 255.0),
            4 => (frac * 255.0, 0.0, 255.0),
            _ => (255.0, 0.0, (1.0 - frac) * 255.0),
        };

        palette.push((r as u8, g as u8, b as u8));
    }

    palette
}

/// Monochrome grayscale palette
fn generate_monochrome_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;
        // Use sine wave for smooth cycling
        let v = ((t * std::f64::consts::PI * 4.0).sin() * 0.5 + 0.5) * 255.0;
        let v = v as u8;
        palette.push((v, v, v));
    }

    palette
}

/// Smooth rainbow palette using HSV
fn generate_rainbow_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let hue = (i as f64 / num_colours as f64) * 360.0;
        let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
        palette.push((r, g, b));
    }

    palette
}

/// Twilight palette - purples, pinks, and dark blues
fn generate_twilight_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        let r = (0.4 + 0.4 * (t * 6.28318 * 2.0).sin()) * 255.0;
        let g = (0.1 + 0.15 * (t * 6.28318 * 3.0 + 1.0).sin()) * 255.0;
        let b = (0.5 + 0.5 * (t * 6.28318 + 0.5).sin()) * 255.0;

        palette.push((
            r.clamp(0.0, 255.0) as u8,
            g.clamp(0.0, 255.0) as u8,
            b.clamp(0.0, 255.0) as u8,
        ));
    }

    palette
}

/// Forest palette - greens and browns
fn generate_forest_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        let r = (0.3 + 0.25 * (t * 6.28318 * 2.0 + 2.0).sin()) * 255.0;
        let g = (0.4 + 0.4 * (t * 6.28318).sin()) * 255.0;
        let b = (0.15 + 0.15 * (t * 6.28318 * 1.5 + 1.0).sin()) * 255.0;

        palette.push((
            r.clamp(0.0, 255.0) as u8,
            g.clamp(0.0, 255.0) as u8,
            b.clamp(0.0, 255.0) as u8,
        ));
    }

    palette
}

/// Lava palette - deep reds, oranges, and black
fn generate_lava_palette(num_colours: usize) -> Vec<(u8, u8, u8)> {
    let mut palette = Vec::with_capacity(num_colours);

    for i in 0..num_colours {
        let t = i as f64 / num_colours as f64;

        // Emphasise reds and oranges with dark bands
        let intensity = (t * 6.28318 * 3.0).sin().powi(2);
        let r = (intensity * 255.0) as u8;
        let g = ((intensity * 0.5).powf(1.5) * 255.0) as u8;
        let b = ((intensity * 0.2).powf(2.0) * 255.0) as u8;

        palette.push((r, g, b));
    }

    palette
}

/// Convert HSV to RGB
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Colour the interior of the Mandelbrot set based on final orbit position
pub fn colour_interior(final_x: f64, final_y: f64, palette: &[(u8, u8, u8)]) -> (u8, u8, u8) {
    // Use angle of final position for colouring
    let angle = final_y.atan2(final_x);
    let normalised = (angle + std::f64::consts::PI) / (2.0 * std::f64::consts::PI);

    // Add magnitude influence for more variation
    let mag = (final_x * final_x + final_y * final_y).sqrt().min(2.0) / 2.0;
    let combined = (normalised + mag * 0.5) % 1.0;

    let idx = (combined * palette.len() as f64) as usize % palette.len();
    palette[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_generation() {
        for palette_type in Palette::all() {
            let palette = palette_type.generate(256);
            assert_eq!(palette.len(), 256);

            for (r, g, b) in &palette {
                assert!(*r <= 255);
                assert!(*g <= 255);
                assert!(*b <= 255);
            }
        }
    }
}
