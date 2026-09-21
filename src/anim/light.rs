//! Light and color: palettes with named roles, aerial perspective, rim
//! light, painterly tone bands and point lights whose falloff respects the
//! on-screen pixel aspect.
use super::Rgb;
use crate::canvas::lerp;

/// One theme's colors, by role. Keep a theme to seven hues: a sky ramp, one
/// key light, one rim, a fog color the far planes sink into, a shadow color
/// that is never pure black on a lit theme, near/far ground and one
/// saturated accent (the screen, the flame, the aurora, the sun).
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// sky gradient stops, top → horizon
    pub sky: &'static [(f32, Rgb)],
    pub key: Rgb,
    /// unit vector in screen space (x right, y down) pointing *from* the light
    pub key_dir: (f32, f32),
    pub rim: Rgb,
    pub fog: Rgb,
    pub shadow: Rgb,
    /// (near, far) ground colors
    pub ground: (Rgb, Rgb),
    pub accent: Rgb,
}

/// Aerial perspective: `depth` 0 = near, 1 = far. Colors sink into `fog_color`
/// with an exponential falloff of strength `k` (2..4 reads as haze, 6+ as fog).
pub fn fog(color: Rgb, fog_color: Rgb, depth: f32, k: f32) -> Rgb {
    let t = 1.0 - (-k * depth.clamp(0.0, 1.0)).exp();
    lerp(color, fog_color, t)
}

/// Rim light: `base` plus `rim_color` scaled by `max(0, ndotl)^power`.
pub fn rim(base: Rgb, rim_color: Rgb, ndotl: f32, power: f32) -> Rgb {
    let f = ndotl.max(0.0).powf(power.max(0.01));
    if f <= 0.0 {
        return base;
    }
    (
        (base.0 as f32 + rim_color.0 as f32 * f).min(255.0) as u8,
        (base.1 as f32 + rim_color.1 as f32 * f).min(255.0) as u8,
        (base.2 as f32 + rim_color.2 as f32 * f).min(255.0) as u8,
    )
}

/// Painterly banding without dither: quantize `t` (0..1) into `bands` steps
/// with a short smooth edge so bands read as brush tones, not staircases.
pub fn tone_bands(t: f32, bands: u32) -> f32 {
    let n = bands.max(1) as f32;
    let x = t.clamp(0.0, 1.0) * n;
    let i = x.floor();
    let f = x - i;
    // smooth the last 20% of each band into the next
    let edge = ((f - 0.8) / 0.2).clamp(0.0, 1.0);
    let e = edge * edge * (3.0 - 2.0 * edge);
    ((i + e) / n).min(1.0)
}

/// Mix `a` toward `b` by `t`, in place-friendly form.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    lerp(a, b, t.clamp(0.0, 1.0))
}

/// Multiply a color by per-channel factors (tinting by a light color).
pub fn tint(c: Rgb, light: Rgb, amount: f32) -> Rgb {
    let f = |v: u8, l: u8| {
        let lit = v as f32 * (l as f32 / 255.0);
        (v as f32 + (lit - v as f32) * amount).clamp(0.0, 255.0) as u8
    };
    (f(c.0, light.0), f(c.1, light.1), f(c.2, light.2))
}

/// A point light with screen-aspect-correct falloff:
/// `f = intensity / (1 + (dx² + (dy/sy)²) / r²)`.
#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub x: f32,
    pub y: f32,
    /// radius in x-pixels where the light has fallen to half
    pub r: f32,
    /// pixel aspect squash (`Stage::sy`) so the pool is round on screen
    pub sy: f32,
    pub color: Rgb,
    pub intensity: f32,
}

impl PointLight {
    /// Light factor at a canvas position.
    #[inline]
    pub fn at(&self, x: f32, y: f32) -> f32 {
        let dx = x - self.x;
        let dy = (y - self.y) / self.sy.max(1e-3);
        self.intensity / (1.0 + (dx * dx + dy * dy) / (self.r * self.r).max(1e-3))
    }

    /// `base * (ambient + f)`, with the lit part pulled toward the light color.
    pub fn lit(&self, base: Rgb, x: f32, y: f32, ambient: f32) -> Rgb {
        let f = self.at(x, y);
        let lit = tint(base, self.color, 0.6);
        let c = lerp(base, lit, (f / (1.0 + f)).clamp(0.0, 1.0));
        let g = (ambient + f).clamp(0.0, 2.0);
        (
            (c.0 as f32 * g).min(255.0) as u8,
            (c.1 as f32 * g).min(255.0) as u8,
            (c.2 as f32 * g).min(255.0) as u8,
        )
    }

    /// Per-column light factor at a reference row, for plates lit by column
    /// (tree trunks): `out[x] = at(x, y_ref)`.
    pub fn column_lut(&self, w: usize, y_ref: f32, out: &mut Vec<f32>) {
        out.clear();
        out.extend((0..w).map(|x| self.at(x as f32 + 0.5, y_ref)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_sinks_far_colors() {
        let c = (200, 40, 40);
        let f = (120, 130, 160);
        assert_eq!(fog(c, f, 0.0, 3.0), c);
        let far = fog(c, f, 1.0, 3.0);
        assert!(far.0 < 130 && far.2 > 140);
        let mid = fog(c, f, 0.5, 3.0);
        assert!(mid.0 < c.0 && mid.0 > far.0);
    }

    #[test]
    fn rim_only_lights_facing_sides() {
        let base = (20, 20, 20);
        assert_eq!(rim(base, (200, 100, 0), -0.5, 2.0), base);
        let lit = rim(base, (200, 100, 0), 1.0, 2.0);
        assert_eq!(lit, (220, 120, 20));
    }

    #[test]
    fn tone_bands_are_monotonic_and_bounded() {
        let mut prev = -1.0;
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let b = tone_bands(t, 4);
            assert!(b >= prev - 1e-6 && (0.0..=1.0).contains(&b));
            prev = b;
        }
        assert_eq!(tone_bands(0.1, 4), 0.0);
        assert_eq!(tone_bands(1.0, 4), 1.0);
    }

    #[test]
    fn point_light_respects_pixel_aspect() {
        let l = PointLight { x: 10.0, y: 10.0, r: 4.0, sy: 0.5, color: (255, 200, 150), intensity: 1.0 };
        assert!((l.at(10.0, 10.0) - 1.0).abs() < 1e-5);
        // 4 px right and 2 px down are the same screen distance under sy=0.5
        assert!((l.at(14.0, 10.0) - l.at(10.0, 12.0)).abs() < 1e-5);
        assert!(l.at(14.0, 10.0) < l.at(12.0, 10.0));
        let mut lut = Vec::new();
        l.column_lut(20, 10.0, &mut lut);
        assert_eq!(lut.len(), 20);
        assert!(lut[10] > lut[0]);
        let lit = l.lit((100, 100, 100), 10.0, 10.0, 0.3);
        assert!(lit.0 > 100 && lit.0 >= lit.2);
    }
}
