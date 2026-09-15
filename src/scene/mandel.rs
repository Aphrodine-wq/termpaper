//! Mandelbrot deep zoom into seahorse valley. Progressive per-frame
//! recompute at half resolution keeps it responsive; palette cycles
//! deep blues → gold → white. The zoom breathes (sine-modulated rate),
//! crossfades are eased, and each arrival at a new target blooms light
//! outward from the zoom center. A restrained dust plane parallax-drifts
//! with the dive and flares on arrival. A cached minimap HUD tracks the dive.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const MAX_ITER: u32 = 256;
/// Curated deep-zoom targets: seahorse valley, elephant valley, a spiral.
const TARGETS: &[(f64, f64)] = &[
    (-0.745, 0.113),
    (-0.1011, 0.9563),
    (-0.77568377, 0.13646737),
];
const ZOOM_RATE: f64 = 20.0; // seconds per 2x zoom

#[derive(Clone, Copy, PartialEq)]
enum Dive {
    Zoom,
    FadeOut,
    FadeIn,
}

struct Dust {
    x: f32, // 0..1 normalized
    y: f32,
    z: f32, // parallax depth 0.3..1.2
    bright: f32,
    phase: f32,
}

pub struct Mandel {
    rng: StdRng,
    detail: Detail,
    /// iteration counts at half resolution
    buf: Vec<u32>,
    /// cached full-set thumbnail for the minimap (recomputed on resize)
    map: Vec<u32>,
    map_w: usize,
    map_h: usize,
    theme: Option<String>,
    row_cursor: usize,
    bw: usize,
    bh: usize,
    scale: f64,
    target: usize,
    dive: Dive,
    fade_t: f32,
    /// arrival bloom envelope: 1 on landing, decays over ~2s
    pulse: f32,
    /// brief center flash on arrival (decays faster than pulse)
    flash: f32,
    dust: Vec<Dust>,
    t: f32,
    w: usize,
    h: usize,
}

impl Mandel {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        Mandel {
            detail,
            buf: Vec::new(),
            map: Vec::new(),
            map_w: 0,
            map_h: 0,
            theme: theme.map(|t| t.to_string()),
            row_cursor: 0,
            bw: 0,
            bh: 0,
            scale: 3.0,
            target: 0,
            // eased entrance: fade in to the first vista
            dive: Dive::FadeIn,
            fade_t: 0.0,
            pulse: 0.0,
            flash: 0.0,
            dust: Vec::new(),
            t: 0.0,
            w: 0,
            h: 0,
            rng,
        }
    }

    fn ensure_dust(&mut self, w: usize, h: usize) {
        if self.w == w && self.h == h && !self.dust.is_empty() {
            return;
        }
        self.w = w;
        self.h = h;
        let n = ((18.0 * density_for(w, h) * self.detail.density()) as usize).clamp(8, 56);
        self.dust = (0..n)
            .map(|_| Dust {
                x: self.rng.random_range(0.0..1.0),
                y: self.rng.random_range(0.0..1.0),
                z: self.rng.random_range(0.35..1.15),
                bright: self.rng.random_range(0.25..0.85),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            })
            .collect();
    }

    fn iter_at(cx: f64, cy: f64) -> u32 {
        let (mut zx, mut zy) = (0.0f64, 0.0f64);
        let mut i = 0;
        while i < MAX_ITER && zx * zx + zy * zy <= 4.0 {
            let t = zx * zx - zy * zy + cx;
            zy = 2.0 * zx * zy + cy;
            zx = t;
            i += 1;
        }
        i
    }

    /// Compute a few rows of the half-res buffer at the current viewport.
    fn compute_rows(&mut self, rows: usize) {
        if self.bw == 0 || self.bh == 0 {
            return;
        }
        let aspect = self.bh as f64 / self.bw as f64;
        for _ in 0..rows {
            let y = self.row_cursor;
            let c = TARGETS[self.target];
            let cy = c.1 + (y as f64 / self.bh as f64 - 0.5) * self.scale * aspect;
            for x in 0..self.bw {
                let cx = c.0 + (x as f64 / self.bw as f64 - 0.5) * self.scale;
                self.buf[y * self.bw + x] = Self::iter_at(cx, cy);
            }
            self.row_cursor = (self.row_cursor + 1) % self.bh;
        }
    }

    fn palette(&self, i: u32, shift: f32) -> (u8, u8, u8) {
        if i >= MAX_ITER {
            return (2, 2, 8); // interior: near-black
        }
        // deep blue → gold → white, cycling smoothly
        let t = ((i as f32).sqrt() / 11.0 + shift).fract();
        const BLUE: &[(f32, (u8, u8, u8))] = &[
            (0.00, (4, 8, 40)),
            (0.25, (16, 42, 120)),
            (0.50, (60, 120, 200)),
            (0.65, (240, 200, 90)),
            (0.80, (255, 245, 220)),
            (1.00, (4, 8, 40)),
        ];
        const FIRE: &[(f32, (u8, u8, u8))] = &[
            (0.00, (20, 2, 2)),
            (0.25, (90, 8, 4)),
            (0.50, (200, 60, 10)),
            (0.65, (255, 180, 50)),
            (0.80, (255, 245, 210)),
            (1.00, (20, 2, 2)),
        ];
        const MONO: &[(f32, (u8, u8, u8))] = &[
            (0.00, (5, 5, 8)),
            (0.25, (40, 42, 55)),
            (0.50, (110, 115, 135)),
            (0.65, (200, 205, 220)),
            (0.80, (255, 255, 255)),
            (1.00, (5, 5, 8)),
        ];
        let stops = match self.theme.as_deref() {
            Some("fire") => FIRE,
            Some("mono") => MONO,
            _ => BLUE,
        };
        for w in stops.windows(2) {
            if t <= w[1].0 {
                // smoothstep for buttery interpolation
                let lt = (t - w[0].0) / (w[1].0 - w[0].0);
                let lt = lt * lt * (3.0 - 2.0 * lt);
                return lerp(w[0].1, w[1].1, lt);
            }
        }
        stops[0].1
    }

    /// Warm accent for the arrival bloom and the minimap dive dot.
    fn accent(&self) -> (u8, u8, u8) {
        match self.theme.as_deref() {
            Some("fire") => (255, 200, 120),
            Some("mono") => (220, 225, 235),
            _ => (255, 220, 140),
        }
    }
}

impl Scene for Mandel {
    fn name(&self) -> &'static str {
        "mandel"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (bw, bh) = (canvas.width() / 2, canvas.height() / 2);
        if bw == 0 || bh == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        let (w, h) = (canvas.width(), canvas.height());
        self.ensure_dust(w, h);
        if bw != self.bw || bh != self.bh {
            self.bw = bw;
            self.bh = bh;
            self.buf = vec![0; bw * bh];
            self.row_cursor = 0;
            // minimap thumbnail: the full set never changes, compute once
            let (mw, mh) = (w / 6 + 4, h / 6 + 3);
            if mw < w && mh < h {
                self.map_w = mw;
                self.map_h = mh;
                self.map = vec![0; mw * mh];
                for my in 0..mh {
                    for mx in 0..mw {
                        let cx = -2.2 + mx as f64 / mw as f64 * 2.8;
                        let cy = -1.2 + my as f64 / mh as f64 * 2.4;
                        self.map[my * mw + mx] = Self::iter_at(cx, cy);
                    }
                }
            } else {
                self.map_w = 0;
                self.map_h = 0;
                self.map = Vec::new();
            }
        }
        self.t += dt;
        // continuous zoom, crossfade to a new target at the precision floor
        match self.dive {
            Dive::Zoom => {
                // breathing rate: the dive surges and lingers, never metronomic
                let breathe = 1.0 + 0.35 * (self.t as f64 * 0.21).sin();
                self.scale *=
                    (-dt as f64 * std::f64::consts::LN_2 / ZOOM_RATE * breathe).exp();
                if self.scale < 1e-11 {
                    self.dive = Dive::FadeOut;
                    self.fade_t = 0.0;
                }
            }
            Dive::FadeOut => {
                self.fade_t += dt;
                if self.fade_t > 1.2 {
                    self.target = (self.target + 1) % TARGETS.len();
                    self.scale = 3.0;
                    // clear stale rows: the new vista emerges from darkness
                    self.buf.fill(0);
                    self.row_cursor = 0;
                    self.dive = Dive::FadeIn;
                    self.fade_t = 0.0;
                }
            }
            Dive::FadeIn => {
                self.fade_t += dt;
                if self.fade_t > 1.2 {
                    self.dive = Dive::Zoom;
                    // payoff: light blooms out from the zoom center
                    self.pulse = 1.0;
                    self.flash = 1.0;
                }
            }
        }
        self.pulse = (self.pulse - dt * 0.45).max(0.0);
        self.flash = (self.flash - dt * 1.4).max(0.0);
        let dim = match self.dive {
            Dive::FadeOut => 1.0 - ease_smooth(self.fade_t / 1.2),
            Dive::FadeIn => ease_smooth(self.fade_t / 1.2),
            _ => 1.0,
        };
        // Detail scales progressive row budget (not iteration identity)
        let rows = ((bh / 6) as f32
            * self.detail.factor()
            * (1.0 + self.pulse * 2.0))
            .max(1.0) as usize;
        self.compute_rows(rows);

        // the bloom kicks the palette cycle forward briefly
        let shift = self.t * 0.03 + self.pulse * 0.25;

        // main plane: fractal with vignette + radial arrival bloom + center flash
        for y in 0..h {
            let dy = y as f32 / h as f32 - 0.5;
            for x in 0..w {
                let dx = x as f32 / w as f32 - 0.5;
                let d2n = (dx * dx + dy * dy) / 0.5; // 0 center, 1 corners
                let vignette = 1.0 - 0.30 * d2n + self.pulse * 0.12 * (1.0 - d2n).max(0.0);
                let bloom = 1.0 + self.pulse * 1.05 * (1.0 - d2n).max(0.0);
                let center_flash = self.flash * 0.55 * (1.0 - d2n * 2.5).max(0.0);
                let i = self.buf[(y / 2).min(bh - 1) * bw + (x / 2).min(bw - 1)];
                let mut c = scale(self.palette(i, shift), dim * vignette * bloom);
                if center_flash > 0.01 {
                    c = lerp(c, self.accent(), center_flash * dim);
                }
                canvas.set(x as i32, y as i32, c);
            }
        }

        // dust / star plane: parallax with zoom, flares on arrival
        let zoom_drift = ((3.0f64 / self.scale.max(1e-12)).ln() as f32 * 0.015).min(2.0);
        let accent = self.accent();
        for d in &mut self.dust {
            d.phase += dt * (1.2 + d.z);
            // slow parallax drift tied to dive depth
            d.x = (d.x + dt * 0.008 * d.z + zoom_drift * 0.002 * d.z).fract();
            d.y = (d.y + dt * 0.004 * d.z).fract();
            let px = d.x * w as f32;
            let py = d.y * h as f32;
            let flare = self.pulse * 0.7 + self.flash * 0.5;
            let b = d.bright * (0.35 + 0.25 * d.phase.sin().abs() + flare);
            let col = scale(accent, b * dim * 0.55);
            let rad = if d.z > 0.85 { 2 } else { 1 };
            glow(canvas, px as i32, py as i32, rad, col, 0.2 + flare * 0.35);
            if d.z > 0.9 {
                canvas.add(px as i32, py as i32, scale(accent, b * dim * 0.4));
            }
        }

        // foreground HUD: cached minimap with a breathing dive dot
        let (mw, mh) = (self.map_w, self.map_h);
        if mw > 0 {
            for my in 0..mh {
                for mx in 0..mw {
                    let i = self.map[my * mw + mx];
                    let c = if i >= MAX_ITER {
                        (8, 8, 12)
                    } else {
                        (46, 52, 70)
                    };
                    canvas.set(w as i32 - mw as i32 + mx as i32, my as i32, scale(c, dim * 0.9));
                }
            }
            let c = TARGETS[self.target];
            let dot_x = (w - mw) as f32 + ((c.0 + 2.2) / 2.8 * mw as f64) as f32;
            let dot_y = ((c.1 + 1.2) / 2.4 * mh as f64) as f32;
            let breathe = 0.7 + 0.3 * (self.t * 3.0).sin();
            canvas.set_f(dot_x, dot_y, scale(accent, dim * breathe));
            glow(canvas, dot_x as i32, dot_y as i32, 2, accent, 0.3 * dim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use std::collections::HashSet;

    #[test]
    fn mandel_zooms_and_stays_rich() {
        let mut m = Mandel::new(StdRng::seed_from_u64(51), None, Detail::Medium);
        let mut c = Canvas::new(64, 32);
        let s0 = m.scale;
        for _ in 0..60 {
            m.update(1.0 / 30.0, &mut c);
        }
        assert!(m.scale < s0 || m.dive != Dive::Zoom, "should be zooming in");
        let colors: HashSet<_> = (0..64)
            .flat_map(|x| (0..32).map(move |y| (x, y)))
            .map(|(x, y)| c.get(x, y).color)
            .collect();
        assert!(colors.len() > 10, "mandelbrot should be non-uniform");
    }

    #[test]
    fn known_points_iter_counts() {
        assert_eq!(Mandel::iter_at(0.0, 0.0), MAX_ITER); // interior
        assert!(Mandel::iter_at(2.0, 2.0) < 10); // far outside escapes fast
    }
}
