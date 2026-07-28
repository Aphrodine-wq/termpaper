//! Night sky: fractal value-noise clouds drifting laterally over a sparse
//! starfield with a glowing moon; two parallax cloud layers with billowy
//! whites and gray undersides. Themes: night (cool moon), sunset (low warm
//! sun over a dark ember horizon), storm (steel clouds, distant lightning).

use super::{noise::fbm, Scene};
use crate::canvas::{ease_smooth, glow, lerp, scale, Canvas};

pub struct Clouds {
    t: f32,
    seed: u32,
    sky_a: (u8, u8, u8),
    sky_b: (u8, u8, u8),
    cloud_hi: (u8, u8, u8),
    cloud_lo: (u8, u8, u8),
    star_tint: (u8, u8, u8),
    stars: bool,
    storm: bool,
    moon: Option<(f32, f32, (u8, u8, u8))>, // fx, fy, color
    xoff0: f32, // accumulated wind offsets (gusted, never a constant slide)
    xoff1: f32,
    birds: Option<(f32, f32, f32, usize)>, // x, y, vx, count
    next_birds: f32,
    sun_break: f32,
    next_break: f32,
    break_x: f32,
    flash: f32, // storm lightning envelope
    next_flash: f32,
    flash_x: f32,
    grid: Vec<f32>, // reused density scratch buffer (no per-frame alloc)
}

/// Bilinear sample of a `gw`-wide grid at integer cell `ix`, row offsets
/// `r0`/`r1` (iy and iy+1 times gw), fractional position `tx`/`ty`.
#[inline]
fn bilerp(g: &[f32], r0: usize, r1: usize, ix: usize, tx: f32, ty: f32) -> f32 {
    let a = g[r0 + ix];
    let b = g[r0 + ix + 1];
    let c = g[r1 + ix];
    let d = g[r1 + ix + 1];
    a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty
}

impl Clouds {
    pub fn new(mut rng: rand::rngs::StdRng, theme: Option<&str>) -> Self {
        use rand::RngExt;
        let (sky_a, sky_b, cloud_hi, cloud_lo, star_tint, stars, storm, moon) = match theme {
            Some("sunset") => (
                (2, 2, 6),
                (26, 10, 14),           // dark ember horizon, still <25% lum
                (252, 236, 214),        // warm-lit tops
                (150, 128, 138),        // mauve undersides
                (255, 226, 200),
                true,
                false,
                Some((0.30, 0.62, (255, 176, 92))),
            ),
            Some("storm") => (
                (3, 4, 8),
                (12, 16, 24),
                (196, 204, 216),
                (120, 128, 144),
                (214, 224, 244),
                false,
                true,
                None,
            ),
            _ => (
                // night
                (0, 0, 0),
                (0, 0, 0),
                (246, 250, 252),
                (168, 176, 192),
                (214, 224, 244),
                true,
                false,
                Some((0.72, 0.18, (228, 234, 244))),
            ),
        };
        Clouds {
            t: 0.0,
            seed: rng.random::<u32>(),
            sky_a,
            sky_b,
            cloud_hi,
            cloud_lo,
            star_tint,
            stars,
            storm,
            moon,
            xoff0: 0.0,
            xoff1: 0.0,
            birds: None,
            next_birds: 12.0,
            sun_break: 0.0,
            next_break: 16.0,
            break_x: 0.5,
            flash: 0.0,
            next_flash: 9.0,
            flash_x: 0.5,
            grid: Vec::new(),
        }
    }

    /// Cloud density at canvas position for a layer, drifting over time.
    fn density(&self, x: f32, y: f32, layer: usize) -> f32 {
        match layer {
            0 => fbm((x + self.xoff0) * 0.014, y * 0.045, 4, self.seed),
            _ => fbm(
                (x + self.xoff1) * 0.024,
                y * 0.06,
                3,
                self.seed.wrapping_add(4242),
            ),
        }
    }
}

impl Scene for Clouds {
    fn name(&self) -> &'static str {
        "clouds"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }

        // wind gusts: the drift itself breathes on a slow sine
        self.xoff0 += 5.0 * (0.75 + 0.35 * (self.t * 0.11).sin()) * dt;
        self.xoff1 += 13.0 * (0.75 + 0.35 * (self.t * 0.17 + 2.0).sin()) * dt;

        // sky gradient (near-black; themes differ at the horizon)
        for y in 0..h {
            let ty = y as f32 / h.max(1) as f32;
            canvas.fill_row(y, lerp(self.sky_a, self.sky_b, ty.powf(1.4)));
        }

        // sparse background stars with a slow twinkle (hash-placed, no state)
        if self.stars {
            let n = ((w * h / 200).max(3)).min(240);
            for i in 0..n as u32 {
                let hsh = (self.seed ^ i.wrapping_mul(2654435761)).wrapping_mul(2246822519);
                let fx = (hsh & 0xffff) as f32 / 65536.0;
                let fy = ((hsh >> 16) & 0xffff) as f32 / 65536.0 * 0.8;
                let phase =
                    (hsh.rotate_left(9) & 0xff) as f32 / 255.0 * std::f32::consts::TAU;
                let mag = 0.08 + (hsh.rotate_left(17) & 0xff) as f32 / 255.0 * 0.18;
                let tw = 0.7 + 0.3 * (self.t * 0.6 + phase).sin();
                canvas.set_f(
                    fx * w as f32,
                    fy * h as f32,
                    scale(self.star_tint, mag * tw),
                );
            }
        }

        // moon (night) / low sun (sunset): the accent anchor, cloud-occluded
        if let Some((fx, fy, color)) = self.moon {
            let (mx, my) = ((fx * w as f32) as i32, (fy * h as f32) as i32);
            let r = ((w.min(h) as f32 * 0.07) as i32).clamp(2, 5);
            glow(canvas, mx, my, r * 3, color, 0.22);
            for dy in -r..=r {
                for dx in -r..=r {
                    let d2 = dx * dx + dy * dy;
                    if d2 <= r * r {
                        // limb darkening toward the rim
                        let shade = 1.0 - 0.25 * (d2 as f32 / (r * r) as f32);
                        canvas.set(mx + dx, my + dy, scale(color, shade));
                    }
                }
            }
        }

        // sun-break: light floods through a passing gap, eased in and out
        self.next_break -= dt;
        if self.next_break <= 0.0 {
            self.sun_break = 5.0;
            self.break_x = 0.2 + ((self.seed as f32 * 0.37 + self.t * 0.01) % 0.6);
            self.next_break = 16.0 + ((self.seed % 13) as f32) + (self.t % 7.0);
        }
        self.sun_break = (self.sun_break - dt).max(0.0);
        let break_k = if self.sun_break > 0.0 {
            crate::canvas::ease_smooth(1.0 - (self.sun_break / 5.0 - 0.5).abs() * 2.0)
        } else {
            0.0
        };
        if break_k > 0.0 {
            let bx = w as f32 * self.break_x;
            let k = break_k * 0.15;
            for y in 0..h {
                let spread = w as f32 * (0.10 + y as f32 / h as f32 * 0.25);
                // d < 1.0 only inside bx ± spread — skip the rest of the row
                let x0 = ((bx - spread).ceil() as i32).max(0);
                let x1 = ((bx + spread).floor() as i32).min(w as i32 - 1);
                for x in x0..=x1 {
                    let d = ((x as f32 - bx) / spread).abs();
                    canvas.add(x, y as i32, scale((255, 240, 200), (1.0 - d) * k));
                }
            }
        }

        // storm-only: distant lightning backlights the deck with a
        // double-strike flicker that dies out (payoff -> decay)
        let mut flash_env = 0.0;
        if self.storm {
            self.next_flash -= dt;
            if self.next_flash <= 0.0 {
                self.flash = 0.9;
                self.flash_x = 0.15 + ((self.seed as f32 * 0.61 + self.t * 0.13) % 0.7);
                self.next_flash = 14.0 + ((self.seed % 11) as f32) + (self.t % 8.0);
            }
            self.flash = (self.flash - dt).max(0.0);
            if self.flash > 0.0 {
                let e = 1.0 - self.flash / 0.9;
                flash_env = (e * 18.0).sin().abs() * (1.0 - e).powi(2);
                let fx = self.flash_x * w as f32;
                let spread = w as f32 * 0.45;
                let x0 = ((fx - spread).ceil() as i32).max(0);
                let x1 = ((fx + spread).floor() as i32).min(w as i32 - 1);
                for y in 0..h {
                    let k = flash_env * (0.10 + 0.06 * (1.0 - y as f32 / h as f32));
                    for x in x0..=x1 {
                        let d = ((x as f32 - fx) / spread).abs();
                        canvas.add(x, y as i32, scale((170, 180, 220), (1.0 - d) * k));
                    }
                }
            }
        }

        // crepuscular god-rays from gaps in the upper cloud deck
        let mut best: [(f32, f32); 2] = [(0.0, f32::MAX); 2];
        for i in 0..16 {
            let x = (i as f32 + 0.5) / 16.0 * w as f32;
            let d = self.density(x, h as f32 * 0.12, 0);
            if d < best[0].1 {
                best[1] = best[0];
                best[0] = (x, d);
            } else if d < best[1].1 {
                best[1] = (x, d);
            }
        }
        for &(gx, d) in &best {
            if d < 0.5 {
                for y in 0..(h as f32 * 0.7) as i32 {
                    let fy = y as f32 / h as f32;
                    let sx = gx + fy * 10.0; // slant
                    let width = 1.0 + fy * 4.0;
                    for dx in -(width as i32)..=(width as i32) {
                        let b = (1.0 - fy) * (-((dx as f32 / width).powi(2))).exp() * 14.0;
                        canvas.add(sx as i32 + dx, y, (b as u8, (b * 0.97) as u8, (b * 0.85) as u8));
                    }
                }
            }
        }

        // virga: faint rain streaks falling from the darkest clouds
        for x in (0..w).step_by(3) {
            let d = self.density(x as f32, h as f32 * 0.25, 1);
            if d > 0.62 {
                let len = (h as f32 * 0.3 * (d - 0.6)) as i32;
                let drift = ((x as f32 * 0.1 + self.t * 2.0).sin() * 2.0) as i32;
                for dy in 0..len {
                    canvas.add(
                        x as i32 + drift,
                        (h as f32 * 0.35) as i32 + dy,
                        (30, 40, 55),
                    );
                }
            }
        }

        // distant bird V-formation crossing
        self.next_birds -= dt;
        if self.next_birds <= 0.0 && self.birds.is_none() {
            self.birds = Some((
                -12.0,
                h as f32 * (0.15 + 0.2 * ((self.seed % 7) as f32 / 7.0)),
                14.0,
                5 + (self.seed % 4) as usize,
            ));
            self.next_birds = 18.0 + (self.seed % 9) as f32;
        }
        if let Some((bx, by, bvx, n)) = &mut self.birds {
            *bx += *bvx * dt;
            if *bx > w as f32 + 20.0 {
                self.birds = None;
            } else {
                for i in 0..*n as i32 {
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let rank = (i + 1) as f32 / 2.0;
                    let bob = (self.t * 5.0 + i as f32).sin() * 0.4;
                    canvas.set_f(
                        *bx - rank * 3.0,
                        *by + side * rank * 1.6 + bob,
                        (40, 48, 62),
                    );
                }
            }
        }

        // two parallax layers, sampled on a quarter-res grid and upsampled
        const STEP: usize = 4;
        for layer in 0..2 {
            let (thresh, soft) = if layer == 0 { (0.52, 0.22) } else { (0.55, 0.18) };
            let gw = w / STEP + 2;
            let gh = h / STEP + 2;
            let (sx, sy, oct, seed, drift) = if layer == 0 {
                (0.014, 0.045, 4, self.seed, 5.0)
            } else {
                (0.024, 0.06, 3, self.seed.wrapping_add(4242), 13.0)
            };
            let xoff = self.t * drift;
            self.grid.resize(gw * gh, 0.0);
            for gy in 0..gh {
                let ny = (gy * STEP) as f32 * sy;
                let row = gy * gw;
                for gx in 0..gw {
                    self.grid[row + gx] = fbm(((gx * STEP) as f32 + xoff) * sx, ny, oct, seed);
                }
            }
            let grid = &self.grid;
            // STEP is a power of two and coords are integers, so the bilinear
            // cell/fraction come from shifts — bit-exact, no float floor/fract.
            const FRACT: [f32; STEP] = [0.0, 0.25, 0.5, 0.75];
            for y in 0..h {
                // hoisted row lookups for the density sample at (x, y) …
                let (r0, r1) = ((y >> 2) * gw, ((y >> 2) + 1) * gw);
                let ty = FRACT[y & 3];
                // … and for the underside probe at (x, y + 2)
                let iy2 = ((y + 2) >> 2).min(gh - 2);
                let (b0, b1) = (iy2 * gw, (iy2 + 1) * gw);
                let ty2 = FRACT[(y + 2) & 3];
                for x in 0..w {
                    let ix = x >> 2;
                    let tx = FRACT[x & 3];
                    let d = bilerp(grid, r0, r1, ix, tx, ty);
                    if d <= thresh {
                        continue;
                    }
                    let cov = ((d - thresh) / soft).min(1.0);
                    let body = if bilerp(grid, b0, b1, ix, tx, ty2) <= thresh {
                        (176, 182, 196) // gray underside
                    } else {
                        (248, 250, 252) // billowy white
                    };
                    let sky = canvas.get(x as i32, y as i32).color;
                    canvas.set(x as i32, y as i32, lerp(sky, body, 0.35 + 0.65 * cov));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_deterministic_and_ranged() {
        let a = fbm(3.7, 9.2, 3, 42);
        let b = fbm(3.7, 9.2, 3, 42);
        assert_eq!(a, b);
        assert!((0.0..=1.0).contains(&a));
        let c = fbm(100.3, 9.2, 3, 42);
        assert!((a - c).abs() > 1e-6, "noise should vary spatially");
    }
}
