//! Reusable scenery, extracted so every scene draws stars, moons, ridges and
//! pines the same way (and so a new scene gets them for free).
use super::draw::{blend, blend_f, capsule, ellipse_f, polygon_fill, Surface};
use super::Rgb;
use crate::canvas::{lerp, scale, Canvas};
use crate::scene::noise::{fbm, hash2, vnoise};

/// A stateless, hash-placed star field: deterministic for a seed and size,
/// so a resize never re-rolls the sky. `density` is stars per 1000 px.
pub struct StarField {
    pub seed: u32,
    pub density: f32,
    pub color: Rgb,
}

impl StarField {
    pub fn new(seed: u32, density: f32, color: Rgb) -> Self {
        StarField { seed, density, color }
    }

    /// Draw stars over rows `0..y_max` with a global `alpha` (dusk fade) and
    /// twinkle driven by `t`.
    pub fn draw(&self, canvas: &mut Canvas, y_max: i32, t: f32, alpha: f32) {
        if alpha <= 0.01 {
            return;
        }
        let (w, h) = (canvas.width() as i32, canvas.height() as i32);
        let y_max = y_max.min(h);
        // lattice of 8x8 cells, each hosting a star with probability from density
        let p = (self.density * 64.0 / 1000.0).clamp(0.0, 1.0);
        for cy in 0..(y_max + 7) / 8 {
            for cx in 0..(w + 7) / 8 {
                let r = hash2(cx, cy, self.seed);
                if r > p {
                    continue;
                }
                let ox = hash2(cx, cy, self.seed ^ 0x51) * 8.0;
                let oy = hash2(cx, cy, self.seed ^ 0x93) * 8.0;
                let (x, y) = (cx as f32 * 8.0 + ox, cy as f32 * 8.0 + oy);
                if y >= y_max as f32 {
                    continue;
                }
                let mag = hash2(cx, cy, self.seed ^ 0xa7);
                let tw = 0.7 + 0.3 * (t * (1.5 + mag * 3.0) + mag * 40.0).sin();
                let bright = (0.25 + mag * mag * 0.75) * tw * alpha;
                blend_f(canvas, x, y, self.color, bright);
                if mag > 0.92 {
                    // a bright star spills into its neighbours
                    blend(canvas, x as i32 + 1, y as i32, self.color, bright * 0.35);
                    blend(canvas, x as i32 - 1, y as i32, self.color, bright * 0.35);
                }
            }
        }
    }
}

/// A limb-darkened moon disc with faint maria. `ry` is the y radius in
/// canvas px (`stage.ry(r)`); `glow` adds a soft halo.
#[allow(clippy::too_many_arguments)]
pub fn moon<S: Surface>(s: &mut S, cx: f32, cy: f32, r: f32, ry: f32, color: Rgb, glow: f32, seed: u32) {
    if glow > 0.0 {
        let (gw, gh) = (r * 2.6, ry * 2.6);
        super::draw::radial_light(s, cx, cy, gw, gh, color, glow * 0.25);
    }
    let dark = scale(color, 0.72);
    ellipse_f(s, cx, cy, r, ry, 0.0, color, 1.0);
    // maria: low-frequency noise patches, limb darkening toward the rim
    let (sw, sh) = s.size();
    let x0 = ((cx - r).floor() as i32).max(0);
    let x1 = ((cx + r).ceil() as i32).min(sw as i32 - 1);
    let y0 = ((cy - ry).floor() as i32).max(0);
    let y1 = ((cy + ry).ceil() as i32).min(sh as i32 - 1);
    for py in y0..=y1 {
        for px in x0..=x1 {
            let u = (px as f32 + 0.5 - cx) / r;
            let v = (py as f32 + 0.5 - cy) / ry;
            let d = (u * u + v * v).sqrt();
            if d >= 0.98 {
                continue;
            }
            let m = vnoise(u * 2.5 + 3.0, v * 2.5 + 7.0, seed);
            let maria = ((m - 0.55) * 4.0).clamp(0.0, 1.0) * 0.35;
            let limb = (d * d) * 0.35;
            s.blend_px(px, py, dark, (maria + limb).min(0.8));
        }
    }
}

/// Deterministic ridge silhouette: `out[x]` is the ridge height (in px
/// above `base`, positive = up) for each column, from ridged fbm plus a
/// body swell. `amp` is the peak amplitude; `freq` scales the terrain.
pub fn ridge_profile(out: &mut Vec<f32>, w: usize, seed: u32, amp: f32, freq: f32, octaves: usize) {
    out.clear();
    out.reserve(w);
    for x in 0..w {
        let u = x as f32 * freq;
        let body = fbm(u * 0.35, 2.7, 2, seed) - 0.5;
        // ridged: fold the noise so peaks are sharp and valleys soft
        let r = 1.0 - (fbm(u, 0.5, octaves, seed ^ 0x77) * 2.0 - 1.0).abs();
        out.push(amp * (0.55 * r * r + 0.45 * (body + 0.5)));
    }
}

/// Fill a ridge silhouette: everything from the profile line down to `bottom`
/// gets `color`, with a `depth`-scaled aerial tint handled by the caller's
/// plate tint. `shade(u, v)` receives 0..1 across the width and 0..1 down
/// from the ridge line to `bottom` for banding/snow-cap effects.
pub fn ridge_fill<S: Surface, F: Fn(f32, f32) -> Rgb>(s: &mut S, profile: &[f32], base: f32, bottom: f32, a: f32, shade: F) {
    let (sw, sh) = s.size();
    let w = profile.len().min(sw);
    let bottom_i = (bottom.ceil() as i32).min(sh as i32);
    for x in 0..w {
        let top = base - profile[x];
        let ti = top.floor() as i32;
        // anti-aliased ridge line: partial coverage on the top pixel
        let frac = 1.0 - (top - ti as f32);
        if ti >= 0 && ti < bottom_i {
            s.blend_px(x as i32, ti, shade(x as f32 / w as f32, 0.0), a * frac);
        }
        let span = (bottom - top).max(1.0);
        for y in (ti + 1).max(0)..bottom_i {
            let v = (y as f32 - top) / span;
            s.blend_px(x as i32, y, shade(x as f32 / w as f32, v.min(1.0)), a);
        }
    }
}

/// A conifer silhouette: mast plus symmetric layered branches, tapering to
/// the top. `height` in canvas px, `width` at the base.
pub fn pine<S: Surface>(s: &mut S, x: f32, base_y: f32, height: f32, width: f32, color: Rgb, a: f32) {
    if height < 2.0 {
        blend_f(s, x, base_y - 1.0, color, a);
        return;
    }
    let top = base_y - height;
    // trunk peeking out under the foliage
    capsule(s, x, base_y, width * 0.08 + 0.3, x, base_y - height * 0.2, 0.3, color, a);
    // a solid stepped triangle: tiers notch the outline so it reads as a
    // conifer rather than a plain wedge, and it stays solid at 3 px tall
    let tiers = (height / 2.2).clamp(2.0, 9.0) as i32;
    let mut right: Vec<(f32, f32)> = Vec::with_capacity(tiers as usize * 2 + 2);
    let mut left: Vec<(f32, f32)> = Vec::with_capacity(tiers as usize * 2 + 2);
    for i in 0..tiers {
        let f0 = i as f32 / tiers as f32;
        let f1 = (i + 1) as f32 / tiers as f32;
        let y1 = top + f1 * height * 0.9;
        let half1 = width * 0.5 * (0.12 + 0.88 * f1);
        let half0 = width * 0.5 * (0.12 + 0.88 * f0);
        // tier bottom edge, then notch back in for the next tier
        right.push((x + half1, y1));
        right.push((x + half0.max(half1 * 0.55), y1 + 0.2));
        left.push((x - half1, y1));
        left.push((x - half0.max(half1 * 0.55), y1 + 0.2));
    }
    let mut pts = vec![(x, top)];
    pts.extend(right.iter().copied());
    pts.push((x, base_y - height * 0.1));
    pts.extend(left.iter().rev().copied());
    polygon_fill(s, &pts, color, a);
}

/// A rounded deciduous tree: trunk and a lumpy canopy built from a few
/// overlapping ellipses.
#[allow(clippy::too_many_arguments)]
pub fn tree<S: Surface>(s: &mut S, x: f32, base_y: f32, height: f32, width: f32, sy: f32, trunk: Rgb, leaf: Rgb, a: f32, seed: u32) {
    let top = base_y - height;
    capsule(s, x, base_y, width * 0.07 + 0.4, x, top + height * 0.45, 0.5, trunk, a);
    let cy = top + height * 0.35;
    for i in 0..5 {
        let h = hash2(i, 3, seed);
        let ox = (hash2(i, 5, seed) - 0.5) * width * 0.8;
        let oy = (h - 0.5) * height * 0.35;
        let r = width * (0.25 + 0.2 * hash2(i, 7, seed));
        ellipse_f(s, x + ox, cy + oy, r, r * sy, 0.0, lerp(leaf, trunk, 0.15 * h), a);
    }
}

/// Coarse-grid noise field sampled with bilinear interpolation: the
/// cheap way to get a full-band mist/caustic pass. `step` px between samples.
pub struct NoiseGrid {
    pub step: usize,
    gw: usize,
    gh: usize,
    vals: Vec<f32>,
}

impl NoiseGrid {
    pub fn new(step: usize) -> Self {
        NoiseGrid {
            step: step.max(1),
            gw: 0,
            gh: 0,
            vals: Vec::new(),
        }
    }

    /// Recompute the grid for a `w`x`h` region with `f(x, y)` sampled at
    /// grid points (in region pixel coordinates).
    pub fn fill<F: Fn(f32, f32) -> f32>(&mut self, w: usize, h: usize, f: F) {
        self.gw = w / self.step + 2;
        self.gh = h / self.step + 2;
        self.vals.clear();
        self.vals.reserve(self.gw * self.gh);
        for gy in 0..self.gh {
            for gx in 0..self.gw {
                self.vals
                    .push(f((gx * self.step) as f32, (gy * self.step) as f32));
            }
        }
    }

    /// Bilinear sample at region pixel `(x, y)`.
    #[inline]
    pub fn at(&self, x: f32, y: f32) -> f32 {
        if self.gw < 2 || self.gh < 2 {
            return 0.0;
        }
        let fx = (x / self.step as f32).max(0.0);
        let fy = (y / self.step as f32).max(0.0);
        let gx = (fx.floor() as usize).min(self.gw - 2);
        let gy = (fy.floor() as usize).min(self.gh - 2);
        let (tx, ty) = ((fx - gx as f32).min(1.0), (fy - gy as f32).min(1.0));
        let i = gy * self.gw + gx;
        let a = self.vals[i];
        let b = self.vals[i + 1];
        let c = self.vals[i + self.gw];
        let d = self.vals[i + self.gw + 1];
        a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::draw::Plate;

    #[test]
    fn star_field_is_deterministic_and_scales() {
        let sf = StarField::new(7, 6.0, (255, 255, 255));
        let mut a = Canvas::new(120, 60);
        let mut b = Canvas::new(120, 60);
        a.clear((0, 0, 0));
        b.clear((0, 0, 0));
        sf.draw(&mut a, 60, 1.0, 1.0);
        sf.draw(&mut b, 60, 1.0, 1.0);
        assert_eq!(a.cells_raw().iter().map(|c| c.color).collect::<Vec<_>>(), b.cells_raw().iter().map(|c| c.color).collect::<Vec<_>>());
        let lit = a.cells_raw().iter().filter(|c| c.color.0 > 30).count();
        assert!(lit > 10 && lit < 800, "{lit}");
        let mut none = Canvas::new(120, 60);
        none.clear((0, 0, 0));
        sf.draw(&mut none, 60, 1.0, 0.0);
        assert!(none.cells_raw().iter().all(|c| c.color == (0, 0, 0)));
    }

    #[test]
    fn ridge_profile_is_bounded_and_seeded() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        ridge_profile(&mut a, 200, 1, 30.0, 0.02, 4);
        ridge_profile(&mut b, 200, 2, 30.0, 0.02, 4);
        assert_eq!(a.len(), 200);
        assert!(a.iter().all(|&v| (0.0..=30.0).contains(&v)));
        assert!(a != b);
        let mut p = Plate::new(200, 80);
        ridge_fill(&mut p, &a, 50.0, 80.0, 1.0, |_, _| (10, 20, 30));
        assert!(p.alpha_at(100, 79) > 0.99);
        assert_eq!(p.alpha_at(100, 5), 0.0);
    }

    #[test]
    fn pine_and_moon_paint_something() {
        let mut p = Plate::new(40, 40);
        pine(&mut p, 20.0, 38.0, 20.0, 10.0, (5, 10, 8), 1.0);
        assert!(p.alpha_at(20, 30) > 0.5, "trunk");
        assert!(p.alpha_at(16, 34) > 0.2, "low branch");
        assert_eq!(p.alpha_at(16, 20), 0.0, "narrow near the top");
        let mut m = Plate::new(40, 40);
        moon(&mut m, 20.0, 20.0, 6.0, 6.0, (230, 230, 240), 0.5, 3);
        assert!(m.alpha_at(20, 20) > 0.99);
        assert!(m.alpha_at(20, 8) > 0.0, "halo");
        let mut g = NoiseGrid::new(4);
        g.fill(40, 40, |x, y| (x + y) / 80.0);
        assert!((g.at(20.0, 20.0) - 0.5).abs() < 0.05);
        assert!(g.at(0.0, 0.0) < g.at(39.0, 39.0));
    }
}
