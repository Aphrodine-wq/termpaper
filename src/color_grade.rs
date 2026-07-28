//! Global color grading applied after scene render and stackable filters.
//! Hue, saturation, and contrast — works on dark/desaturated pixels too.

use crate::canvas::{hsv, Canvas};

pub fn apply(canvas: &mut Canvas, hue_deg: f32, saturation: f32, contrast: f32) {
    let hue_on = hue_deg >= 0.5;
    let sat_on = (saturation - 1.0).abs() > 0.02;
    let con_on = (contrast - 1.0).abs() > 0.02;
    if !hue_on && !sat_on && !con_on {
        return;
    }

    let rot = hue_deg / 360.0;
    let (w, h) = (canvas.width(), canvas.height());
    for y in 0..h {
        for x in 0..w {
            let c = canvas.get(x as i32, y as i32).color;
            let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let d = max - min;

            let mut hue = if d < 1e-6 {
                0.0
            } else if max == r {
                ((g - b) / d).rem_euclid(6.0) / 6.0
            } else if max == g {
                ((b - r) / d + 2.0) / 6.0
            } else {
                ((r - g) / d + 4.0) / 6.0
            };
            let mut sat = if max < 1e-6 { 0.0 } else { d / max };
            let val = max;

            if hue_on {
                if sat < 0.10 && val > 0.015 {
                    sat = 0.10;
                }
                hue = (hue + rot).fract();
            }
            if sat_on {
                sat = (sat * saturation).clamp(0.0, 1.0);
            }

            let mut out = hsv(hue, sat, val);

            if con_on {
                out = (
                    contrast_channel(out.0, contrast),
                    contrast_channel(out.1, contrast),
                    contrast_channel(out.2, contrast),
                );
            }

            canvas.set(x as i32, y as i32, out);
        }
    }
}

fn contrast_channel(v: u8, contrast: f32) -> u8 {
    let f = v as f32 / 255.0;
    (((f - 0.5) * contrast + 0.5).clamp(0.0, 1.0) * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_shifts_red_toward_green() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, (255, 0, 0));
        apply(&mut c, 120.0, 1.0, 1.0);
        let (r, g, b) = c.get(0, 0).color;
        assert!(g > r && g > 100, "expected green shift, got ({r},{g},{b})");
    }

    #[test]
    fn saturation_boosts_muted_pixel() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, (40, 42, 50));
        apply(&mut c, 0.0, 2.0, 1.0);
        let a = c.get(0, 0).color;
        let spread = (a.0 as i32 - a.1 as i32).abs() + (a.1 as i32 - a.2 as i32).abs();
        assert!(spread > 10, "sat boost should spread channels");
    }
}
