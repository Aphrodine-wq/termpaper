//! Lava lamp: metaballs wandering and merging (spring-damped steering, no
//! hard bounces), inverse-square fields thresholded through a deep red →
//! orange → yellow palette with a dim ember halo bleeding off each blob.
//! A burp event swells one blob, erupts risers and surface bubbles at its
//! peak, and flashes heat onto its surroundings.

use super::{noise::fbm, Detail, Scene};
use crate::canvas::{density_for, glow, lerp, Canvas};
use crate::physics;
use rand::{rngs::StdRng, RngExt};

struct Blob {
    x: f32,
    y: f32,
    r: f32,
    heading: f32,
    turn_vel: f32,
    turn_a: f32,
    turn_b: f32,
    speed: f32,
    phase: f32,
    wobble: f32,
}

/// Field value → lava palette stops. Under 0.55 is pure black, 0.55–0.9 a
/// dim ember halo; hotter = yellower. Selected once per frame so the pixel
/// loop is a table lookup.
fn stops_table(theme: Option<&str>) -> &'static [(f32, (u8, u8, u8))] {
    match theme {
        Some("basalt") => &[
            (0.00, (0, 0, 0)),
            (0.55, (0, 0, 0)),     // halo floor: pure black
            (0.90, (14, 14, 17)),  // dim ember halo bleeding off the blobs
            (1.05, (50, 50, 58)),
            (1.30, (110, 112, 120)),
            (1.70, (180, 184, 192)),
            (2.30, (225, 228, 235)),
            (3.20, (255, 255, 255)),
        ],
        Some("toxic") => &[
            (0.00, (0, 0, 0)),
            (0.55, (0, 0, 0)),
            (0.90, (4, 22, 4)),
            (1.05, (12, 70, 10)),
            (1.30, (25, 150, 20)),
            (1.70, (80, 230, 40)),
            (2.30, (160, 255, 60)),
            (3.20, (220, 255, 160)),
        ],
        _ => &[
            (0.00, (0, 0, 0)),
            (0.55, (0, 0, 0)),      // halo floor: pure black
            (0.90, (40, 5, 3)),     // dim ember halo bleeding off the blobs
            (1.05, (90, 8, 4)),     // deep red rim
            (1.30, (190, 30, 0)),   // red
            (1.70, (255, 105, 0)),  // orange
            (2.30, (255, 185, 30)),
            (3.20, (255, 235, 120)), // yellow-hot cores
        ],
    }
}

fn stops(f: f32, table: &[(f32, (u8, u8, u8))]) -> (u8, u8, u8) {
    if f <= table[0].0 {
        return table[0].1;
    }
    for w in table.windows(2) {
        if f <= w[1].0 {
            return lerp(w[0].1, w[1].1, (f - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    table.last().unwrap().1
}

struct Bubble {
    x: f32,
    y: f32,
    r: f32,
    max_r: f32,
}

pub struct Lava {
    rng: StdRng,
    detail: Detail,
    theme: Option<String>,
    blobs: Vec<Blob>,
    bubbles: Vec<Bubble>,
    /// rising gas bubbles: (x, y, speed, sway phase)
    risers: Vec<(f32, f32, f32, f32)>,
    next_bubble: f32,
    burp: f32,
    next_burp: f32,
    /// blob the current burp is swelling; eruption burst fired at its peak
    burp_idx: usize,
    burp_burst: bool,
    crust_seed: u32,
    t: f32,
    w: usize,
    h: usize,
    /// half-resolution field buffer, bilinearly upsampled per pixel
    field: Vec<f32>,
    /// half-resolution crust-noise buffer, upsampled alongside `field`
    crust: Vec<f32>,
    /// per-blob squared radii, reused each frame
    radii: Vec<f32>,
}

impl Lava {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let crust_seed = rng.random::<u32>();
        Lava {
            rng,
            detail,
            theme: theme.map(|t| t.to_string()),
            blobs: Vec::new(),
            bubbles: Vec::new(),
            risers: Vec::new(),
            next_bubble: 1.5,
            burp: 0.0,
            next_burp: 8.0,
            burp_idx: 0,
            burp_burst: false,
            crust_seed,
            t: 0.0,
            w: 0,
            h: 0,
            field: Vec::new(),
            crust: Vec::new(),
            radii: Vec::new(),
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        // count grows with area, but the radius cap below keeps coverage
        // fraction roughly constant — large terminals stay molten, not soup
        let n = self.detail.scale((w * h / 2500) as f32 * 2.0 * density_for(w, h), 5).clamp(5, 26);
        let unit = ((w.min(h) as f32) * 0.16).min(10.0);
        self.blobs = (0..n)
            .map(|_| Blob {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                r: (unit * self.rng.random_range(0.7..1.4)).max(2.4),
                heading: self.rng.random_range(0.0..std::f32::consts::TAU),
                turn_vel: 0.0,
                turn_a: self.rng.random_range(0.2..0.6),
                turn_b: self.rng.random_range(0.1..0.35),
                speed: self.rng.random_range(4.0..9.0),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                wobble: self.rng.random_range(0.6..1.4),
            })
            .collect();
        self.radii = vec![0.0; n];
    }
}

impl Scene for Lava {
    fn name(&self) -> &'static str {
        "lava"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let t = self.t;

        // wander: spring-damped turning with sine targets, speed breathing;
        // steer softly back to the interior inside the bleed margin so blobs
        // never pop off a hard bounce. Margin scales with the canvas so
        // small terminals don't lose their blobs offscreen.
        let m = (w.min(h) as f32 * 0.5).clamp(4.0, 20.0);
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        for b in &mut self.blobs {
            let target_turn =
                ((t * b.turn_a + b.phase).sin() + 0.6 * (t * b.turn_b).cos()) * 0.7;
            b.turn_vel +=
                physics::spring_damper(b.turn_vel, b.turn_vel, target_turn, 5.0, 3.5) * dt;
            b.heading += b.turn_vel * dt;
            let speed = b.speed * (0.7 + 0.3 * (t * 0.4 + b.phase * 2.0).sin());
            b.x += b.heading.cos() * speed * dt;
            b.y += b.heading.sin() * speed * dt * 0.7;
            if b.x < -m * 0.4 || b.x > w as f32 + m * 0.4 || b.y < -m * 0.4 || b.y > h as f32 + m * 0.4
            {
                let to_center = (cy - b.y).atan2(cx - b.x);
                let mut diff = to_center - b.heading;
                while diff > std::f32::consts::PI {
                    diff -= std::f32::consts::TAU;
                }
                while diff < -std::f32::consts::PI {
                    diff += std::f32::consts::TAU;
                }
                b.heading += diff * 1.5 * dt;
            }
            b.x = b.x.clamp(-m, w as f32 + m);
            b.y = b.y.clamp(-m, h as f32 + m);
        }

        // burp event: one blob swells (anticipation), erupts at the peak
        // (payoff: a burst of risers and surface bubbles), then relaxes
        self.next_burp -= dt;
        if self.next_burp <= 0.0 && !self.blobs.is_empty() {
            self.burp = 4.0;
            self.burp_idx = self.rng.random_range(0..self.blobs.len());
            self.burp_burst = false;
            self.next_burp = self.rng.random_range(14.0..26.0);
        }
        self.burp = (self.burp - dt).max(0.0);
        // envelope 0→1→0, eased, peaking halfway through the burp
        let burp_e = if self.burp > 0.0 {
            crate::canvas::ease_smooth(1.0 - (self.burp / 4.0 - 0.5).abs() * 2.0)
        } else {
            0.0
        };
        if burp_e > 0.9 && !self.burp_burst && self.burp_idx < self.blobs.len() {
            self.burp_burst = true;
            let (bx, by, br) = {
                let b = &self.blobs[self.burp_idx];
                (b.x, b.y, b.r)
            };
            for _ in 0..6 {
                self.risers.push((
                    bx + self.rng.random_range(-br * 0.5..br * 0.5),
                    by + self.rng.random_range(-br * 0.3..br * 0.5),
                    self.rng.random_range(5.0..9.0),
                    self.rng.random_range(0.0..std::f32::consts::TAU),
                ));
            }
            for _ in 0..2 {
                self.bubbles.push(Bubble {
                    x: bx + self.rng.random_range(-br * 0.4..br * 0.4),
                    y: by + self.rng.random_range(-br * 0.4..br * 0.4),
                    r: 0.5,
                    max_r: self.rng.random_range(2.5..5.0),
                });
            }
        }

        // evaluate the summed inverse-square field and the crust noise on
        // a half-resolution grid, then bilinearly upsample per pixel: both
        // are smooth at 2px sampling, so this is visually identical to
        // full-res at a quarter of the cost
        self.radii.clear();
        for i in 0..self.blobs.len() {
            let b = &self.blobs[i];
            let wob = 1.0 + 0.12 * (t * b.wobble + b.phase).sin();
            let swell = if i == self.burp_idx && burp_e > 0.0 {
                1.0 + 0.45 * burp_e
            } else {
                1.0
            };
            let r = b.r * wob * swell;
            self.radii.push(r * r);
        }
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);
        self.field.clear();
        self.field.resize(cw * ch, 0.0);
        self.crust.clear();
        self.crust.resize(cw * ch, 0.0);
        {
            let blobs = &self.blobs;
            let radii = &self.radii;
            let field = &mut self.field;
            for cy in 0..ch {
                let y = (cy * 2) as f32;
                let row = &mut field[cy * cw..(cy + 1) * cw];
                for (b, r2) in blobs.iter().zip(radii) {
                    // softening epsilon scales with r² so small blobs still
                    // develop a hot core and large ones don't wash out
                    let eps = r2 * 0.2;
                    let dy2 = (y - b.y) * (y - b.y) + eps;
                    let mut dx = -b.x;
                    for fx in row.iter_mut() {
                        *fx += r2 / (dx * dx + dy2);
                        dx += 2.0;
                    }
                }
            }
            // cooling crust noise on the same coarse grid: it drifts at
            // 5px/s with a ~4.5px finest wavelength, so 2px sampling plus
            // bilinear upsampling is visually identical to full-res
            let crust = &mut self.crust;
            for cy in 0..ch {
                let ny = (cy * 2) as f32 * 0.05;
                for cx in 0..cw {
                    crust[cy * cw + cx] =
                        fbm((cx * 2) as f32 * 0.05 + t * 0.25, ny, 3, self.crust_seed);
                }
            }
        }

        let table = stops_table(self.theme.as_deref());
        for y in 0..h {
            // vertical bilinear weights into the coarse grids
            let fy = y as f32 * 0.5;
            let y0 = fy as usize;
            let y1 = (y0 + 1).min(ch - 1);
            let ty = fy - y0 as f32;
            let frow0 = &self.field[y0 * cw..y0 * cw + cw];
            let frow1 = &self.field[y1 * cw..y1 * cw + cw];
            let crow0 = &self.crust[y0 * cw..y0 * cw + cw];
            let crow1 = &self.crust[y1 * cw..y1 * cw + cw];
            for x in 0..w {
                let fx = x as f32 * 0.5;
                let x0 = fx as usize;
                let x1 = (x0 + 1).min(cw - 1);
                let tx = fx - x0 as f32;
                let top = frow0[x0] + (frow0[x1] - frow0[x0]) * tx;
                let bot = frow1[x0] + (frow1[x1] - frow1[x0]) * tx;
                let f = top + (bot - top) * ty;
                let top = crow0[x0] + (crow0[x1] - crow0[x0]) * tx;
                let bot = crow1[x0] + (crow1[x1] - crow1[x0]) * tx;
                let crust = top + (bot - top) * ty;
                // cooling crust drifting over the cooler regions, hot cracks
                // glowing at its ragged edges
                if crust > 0.63 && f < 1.7 {
                    canvas.set(x as i32, y as i32, (26, 17, 13));
                } else if crust > 0.60 && f > 1.05 {
                    canvas.set(
                        x as i32,
                        y as i32,
                        lerp(stops(f, table), (255, 130, 25), 0.5),
                    );
                } else {
                    canvas.set(x as i32, y as i32, stops(f, table));
                }
            }
        }

        // heat flash: the burping blob lights its surroundings as it swells
        if burp_e > 0.01 && self.burp_idx < self.blobs.len() {
            let b = &self.blobs[self.burp_idx];
            let rad = (b.r * 0.6 * burp_e) as i32 + 2;
            glow(
                canvas,
                b.x as i32,
                b.y as i32,
                rad.clamp(2, 9),
                (255, 150, 40),
                0.35 * burp_e,
            );
        }

        // gas bubbles rising THROUGH the molten blobs: buoyant, swaying
        if self.rng.random::<f32>() < dt * 4.0 && !self.blobs.is_empty() {
            let bi = self.rng.random_range(0..self.blobs.len());
            let bb = &self.blobs[bi];
            self.risers.push((
                bb.x,
                bb.y + bb.r * 0.5,
                self.rng.random_range(5.0..9.0),
                self.rng.random_range(0.0..std::f32::consts::TAU),
            ));
        }
        self.risers.retain_mut(|(rx, ry, rv, phase)| {
            *rv = (*rv + 7.0 * dt).min(16.0); // buoyancy: accelerate upward
            *ry -= *rv * dt;
            *rx += (t * 3.0 + *phase).sin() * 2.5 * dt;
            if *ry < 2.0 {
                return false;
            }
            canvas.add(*rx as i32, *ry as i32, (200, 160, 60));
            canvas.add(*rx as i32, *ry as i32 + 1, (120, 90, 30));
            true
        });

        // surface bubbles form and pop over the hot blobs
        self.next_bubble -= dt;
        if self.next_bubble <= 0.0 {
            self.next_bubble = self.rng.random_range(0.4..1.4);
            if !self.blobs.is_empty() {
                let bi = self.rng.random_range(0..self.blobs.len());
                let bb = &self.blobs[bi];
                self.bubbles.push(Bubble {
                    x: bb.x + self.rng.random_range(-bb.r * 0.4..bb.r * 0.4),
                    y: bb.y + self.rng.random_range(-bb.r * 0.4..bb.r * 0.4),
                    r: 0.5,
                    max_r: self.rng.random_range(2.5..5.0),
                });
            }
        }
        self.bubbles.retain_mut(|b| {
            // ease-out growth: fast at birth, settling as it nears the pop
            b.r += 6.0 * dt * (1.0 - 0.55 * b.r / b.max_r);
            if b.r >= b.max_r {
                // pop: brief warm flash bleeding onto the blob
                glow(canvas, b.x as i32, b.y as i32, 2, (255, 170, 60), 0.55);
                return false;
            }
            let alpha = 1.0 - b.r / b.max_r;
            let steps = (b.r * 5.0) as i32 + 6;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                canvas.add(
                    (b.x + a.cos() * b.r) as i32,
                    (b.y + a.sin() * b.r) as i32,
                    crate::canvas::scale((255, 190, 90), alpha * 0.5),
                );
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use std::collections::HashSet;

    #[test]
    fn lava_produces_blobs() {
        let mut l = Lava::new(StdRng::seed_from_u64(3), None, Detail::Medium);
        let mut c = Canvas::new(60, 30);
        for _ in 0..30 {
            l.update(1.0 / 30.0, &mut c);
        }
        let colors: HashSet<_> = (0..60)
            .flat_map(|x| (0..30).map(move |y| (x, y)))
            .map(|(x, y)| c.get(x, y).color)
            .collect();
        assert!(colors.len() > 20, "expected smooth lava gradients");
        // some pixel should be hot (orange or brighter)
        let hot = colors.iter().any(|&(r, g, _)| r > 200 && g > 60);
        assert!(hot, "expected hot lava colors somewhere");
    }
}
