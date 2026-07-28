//! Classic demoscene plasma: layered sine functions mapped to a slowly
//! hue-cycling palette. Dark shaped troughs keep the field disciplined while
//! two roaming focal lights lift local brightness, and a SURGE event runs
//! gather -> flash -> fade: light collapses into a tight point, flashes hot
//! white, then bleeds back into the field. Debris motes drift with the field,
//! gather toward the surge, and scatter on flash; a radial aftermath ring
//! carries lighting bleed through the fade.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, hsv, lerp, Canvas};
use rand::{rngs::StdRng, RngExt};

const GATHER_TIME: f32 = 2.4;
const FLASH_TIME: f32 = 0.45;
const FADE_TIME: f32 = 3.2;

/// Surge rhythm: Idle -> Gather (light tightens) -> Flash (hot payoff) -> Fade.
#[derive(Clone, Copy)]
enum Surge {
    Idle,
    Gather(f32), // elapsed
    Flash(f32),
    Fade(f32),
}

/// Mid/near debris mote that drifts with the field and reacts to surges.
struct Mote {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    near: bool,
    phase: f32,
    bright: f32,
}

pub struct Plasma {
    rng: StdRng,
    detail: Detail,
    t: f32,
    sat: f32,
    hue_span: f32,
    p1: f32,
    p2: f32,
    p3: f32,
    hue_base: f32,
    hue_target: f32,
    next_jump: f32,
    surge: Surge,
    next_surge: f32,
    surge_pos: (f32, f32),
    /// Radial aftermath ring strength after flash (decays through Fade).
    aftermath: f32,
    motes: Vec<Mote>,
    w: usize,
    h: usize,
}

impl Plasma {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (sat, hue_span) = match theme {
            Some("cool") => (0.8, 0.45),
            Some("warm") => (0.85, 0.4),
            Some("acid") => (1.0, 0.9),
            _ => (0.85, 0.7), // rainbow
        };
        let hue_base0 = match theme {
            Some("cool") => 0.55,
            Some("warm") => 0.0,
            _ => 0.0,
        };
        Plasma {
            detail,
            t: 0.0,
            sat,
            hue_span,
            p1: rng.random_range(0.0..std::f32::consts::TAU),
            p2: rng.random_range(0.0..std::f32::consts::TAU),
            p3: rng.random_range(0.0..std::f32::consts::TAU),
            hue_base: hue_base0,
            hue_target: hue_base0,
            next_jump: 18.0,
            surge: Surge::Idle,
            next_surge: 8.0, // first payoff lands early
            surge_pos: (0.5, 0.5),
            aftermath: 0.0,
            motes: Vec::new(),
            w: 0,
            h: 0,
            rng,
        }
    }

    fn spawn_mote(&mut self, w: usize, h: usize) -> Mote {
        let near = self.rng.random::<f32>() < 0.35;
        Mote {
            x: self.rng.random_range(0.0..w as f32),
            y: self.rng.random_range(0.0..h as f32),
            vx: self.rng.random_range(-8.0..8.0),
            vy: self.rng.random_range(-6.0..6.0),
            near,
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            bright: if near {
                self.rng.random_range(0.55..1.0)
            } else {
                self.rng.random_range(0.25..0.55)
            },
        }
    }

    fn ensure_motes(&mut self, w: usize, h: usize) {
        if self.w == w && self.h == h && !self.motes.is_empty() {
            return;
        }
        self.w = w;
        self.h = h;
        let n = ((12.0 * density_for(w, h) * self.detail.factor()) as usize).clamp(6, 48);
        self.motes = (0..n).map(|_| self.spawn_mote(w, h)).collect();
    }
}

impl Scene for Plasma {
    fn name(&self) -> &'static str {
        "plasma"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let t = self.t;
        self.ensure_motes(w, h);

        // occasional palette family jump (cool <-> warm), eased — no pop
        self.next_jump -= dt;
        if self.next_jump <= 0.0 {
            self.hue_target += 0.5;
            self.next_jump = 18.0;
        }
        self.hue_base += (self.hue_target - self.hue_base) * (dt * 0.55).min(1.0);

        // --- surge scheduling (anticipation -> payoff -> decay) ---
        self.next_surge -= dt;
        match self.surge {
            Surge::Idle => {
                if self.next_surge <= 0.0 {
                    self.surge_pos = (
                        self.rng.random_range(0.25..0.75),
                        self.rng.random_range(0.25..0.75),
                    );
                    self.surge = Surge::Gather(0.0);
                }
            }
            Surge::Gather(e) => {
                let e = e + dt;
                self.surge = if e >= GATHER_TIME {
                    Surge::Flash(0.0)
                } else {
                    Surge::Gather(e)
                };
            }
            Surge::Flash(e) => {
                let e = e + dt;
                if e >= FLASH_TIME {
                    self.aftermath = 1.0;
                    self.surge = Surge::Fade(0.0);
                } else {
                    self.surge = Surge::Flash(e);
                }
            }
            Surge::Fade(e) => {
                let e = e + dt;
                if e >= FADE_TIME {
                    self.surge = Surge::Idle;
                    self.next_surge = self.rng.random_range(20.0..32.0);
                } else {
                    self.surge = Surge::Fade(e);
                }
            }
        }
        self.aftermath = (self.aftermath - dt * 0.55).max(0.0);

        // surge envelope: strength, falloff tightening, flash brightness
        let (surge_str, surge_tight, flash_k) = match self.surge {
            Surge::Idle => (0.0, 0.0, 0.0),
            Surge::Gather(e) => {
                let k = ease_smooth(e / GATHER_TIME);
                (0.35 * k, k, 0.0)
            }
            Surge::Flash(e) => {
                let k = 1.0 - e / FLASH_TIME;
                (1.1 * k, 1.0, k)
            }
            Surge::Fade(e) => {
                let k = 1.0 - ease_smooth(e / FADE_TIME);
                (0.9 * k, 1.0, 0.0)
            }
        };

        // wandering center for the radial component
        let cx = 0.5 + 0.3 * (t * 0.23 + self.p1).sin();
        let cy = 0.5 + 0.3 * (t * 0.19 + self.p2).cos();

        // two roaming focal lights: depth + lighting interplay with the field
        let l1x = 0.5 + 0.36 * (t * 0.10 + self.p1).sin();
        let l1y = 0.5 + 0.30 * (t * 0.13 + self.p2).cos();
        let l2x = 0.5 + 0.40 * (t * 0.07 + self.p3 + 2.0).cos();
        let l2y = 0.5 + 0.34 * (t * 0.09 + self.p1 + 4.0).sin();
        let breathe1 = 0.85 + 0.15 * (t * 0.5).sin();
        let breathe2 = 0.85 + 0.15 * (t * 0.4 + 1.7).cos();
        let s1 = 0.24 * breathe1;
        let s2 = 0.17 * breathe2;

        // pixel-space falloffs (y doubled for the ~1:2 cell aspect)
        let pw = w as f32;
        let ph = h as f32 * 2.0;
        let f1 = 1.0 / (pw * 0.28 * pw * 0.28 + ph * 0.28 * ph * 0.28);
        let f2 = 1.0 / (pw * 0.32 * pw * 0.32 + ph * 0.32 * ph * 0.32);
        let fs = (1.0 + surge_tight * 2.5)
            / (pw * 0.30 * pw * 0.30 + ph * 0.30 * ph * 0.30);
        let global_breathe = 1.0 + 0.06 * (t * 0.4).sin();

        // aftermath ring: expanding brightness shell after flash
        let ring_r = 0.08 + (1.0 - self.aftermath) * 0.55;
        let ring_w = 0.06 + 0.04 * self.aftermath;

        for y in 0..h {
            let ny = y as f32 / h as f32;
            for x in 0..w {
                let nx = x as f32 / w as f32;
                let v = (nx * 9.0 + t * 0.9 + self.p1).sin()
                    + (ny * 7.0 - t * 1.15 + self.p2).sin()
                    + ((nx + ny) * 6.0 + t * 0.6 + self.p3).sin()
                    + (((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt() * 11.0 - t * 1.4).sin();
                // second interfering system: sharp vein-like filaments
                let vein = (1.0 - (nx * 7.0 - ny * 5.0 + t * 0.8 + self.p2).sin().abs()).powi(3);
                let u = ((v + 4.0) / 8.0 + vein * 0.4).clamp(0.0, 1.0);
                // contrast shape: troughs fall dark, crests run hot
                let shaped = ease_smooth(u);
                // slow shadow field multiplies troughs down further
                let shade =
                    0.68 + 0.32 * ((nx * 3.1 - ny * 2.3 + t * 0.21 + self.p3).sin() * 0.5 + 0.5);
                // focal lights lift their surroundings (aspect-corrected)
                let dx1 = (nx - l1x) * pw;
                let dy1 = (ny - l1y) * ph;
                let light1 = s1 / (1.0 + (dx1 * dx1 + dy1 * dy1) * f1);
                let dx2 = (nx - l2x) * pw;
                let dy2 = (ny - l2y) * ph;
                let light2 = s2 / (1.0 + (dx2 * dx2 + dy2 * dy2) * f2);
                let dxs = (nx - self.surge_pos.0) * pw;
                let dys = (ny - self.surge_pos.1) * ph;
                let surge_light = surge_str / (1.0 + (dxs * dxs + dys * dys) * fs);

                // residual hotspot + expanding ring through fade
                let dist = ((nx - self.surge_pos.0).powi(2)
                    + ((ny - self.surge_pos.1) * 2.0).powi(2))
                    .sqrt();
                let hotspot = self.aftermath * 0.35 / (1.0 + dist * dist * 28.0);
                let ring = self.aftermath
                    * (-((dist - ring_r) / ring_w).powi(2)).exp()
                    * 0.55;

                let val = (0.20 + 0.62 * shaped) * shade * global_breathe
                    + light1
                    + light2
                    + surge_light
                    + flash_k * 0.10
                    + hotspot
                    + ring;
                let hue = self.hue_base + t * 0.015 + u * self.hue_span;
                let base = hsv(hue, self.sat, val.min(1.0));
                // hot crests whiten; the flash pushes the core toward white
                let hot = ((val - 0.85) * 2.2 + flash_k * 0.3).clamp(0.0, 0.75);
                canvas.set(x as i32, y as i32, lerp(base, (255, 255, 255), hot));
            }
        }

        // --- debris motes: gather during Gather, scatter on Flash ---
        let sx = self.surge_pos.0 * w as f32;
        let sy = self.surge_pos.1 * h as f32;
        let gather_k = match self.surge {
            Surge::Gather(e) => ease_smooth(e / GATHER_TIME),
            Surge::Flash(_) => 0.0,
            _ => 0.0,
        };
        let scatter = matches!(self.surge, Surge::Flash(_));

        for m in &mut self.motes {
            m.phase += dt * (2.0 + m.bright);
            // field drift
            let fx = (m.y * 0.08 + t * 0.7 + self.p1).sin() * 12.0;
            let fy = (m.x * 0.06 - t * 0.55 + self.p2).cos() * 9.0;
            m.vx += fx * dt;
            m.vy += fy * dt;

            if gather_k > 0.0 {
                let dx = sx - m.x;
                let dy = sy - m.y;
                m.vx += dx * gather_k * 1.8 * dt;
                m.vy += dy * gather_k * 1.8 * dt;
            }
            if scatter {
                let dx = m.x - sx;
                let dy = m.y - sy;
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                m.vx += (dx / len) * 90.0 * dt;
                m.vy += (dy / len) * 90.0 * dt;
            }

            // soft clamp velocity
            let spd = (m.vx * m.vx + m.vy * m.vy).sqrt();
            let max_spd = if m.near { 55.0 } else { 35.0 };
            if spd > max_spd {
                m.vx *= max_spd / spd;
                m.vy *= max_spd / spd;
            }
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            // wrap
            if m.x < 0.0 {
                m.x += w as f32;
            } else if m.x >= w as f32 {
                m.x -= w as f32;
            }
            if m.y < 0.0 {
                m.y += h as f32;
            } else if m.y >= h as f32 {
                m.y -= h as f32;
            }

            let breathe = 0.7 + 0.3 * (m.phase).sin();
            let surge_boost = match self.surge {
                Surge::Gather(_) => 0.15 + 0.35 * gather_k,
                Surge::Flash(e) => 0.6 * (1.0 - e / FLASH_TIME),
                Surge::Fade(e) => 0.25 * (1.0 - ease_smooth(e / FADE_TIME)),
                Surge::Idle => 0.0,
            };
            let b = (m.bright * breathe + surge_boost).clamp(0.0, 1.0);
            let hue = self.hue_base + t * 0.015 + 0.15 * m.bright;
            let col = hsv(hue, self.sat * 0.85, b);
            let rad = if m.near { 2 } else { 1 };
            glow(
                canvas,
                m.x as i32,
                m.y as i32,
                rad,
                col,
                0.45 + 0.45 * b,
            );
            if m.near {
                canvas.set(
                    m.x as i32,
                    m.y as i32,
                    lerp(col, (255, 255, 255), 0.25 * b),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use std::collections::HashSet;

    #[test]
    fn plasma_produces_rich_nonuniform_output() {
        let mut p = Plasma::new(StdRng::seed_from_u64(1), None, Detail::Medium);
        let mut c = Canvas::new(40, 20);
        p.update(1.0 / 30.0, &mut c);
        let colors: HashSet<_> = (0..40)
            .flat_map(|x| (0..20).map(move |y| (x, y)))
            .map(|(x, y)| c.get(x, y).color)
            .collect();
        assert!(colors.len() > 50, "plasma should be colorful, got {}", colors.len());
    }

    #[test]
    fn plasma_evolves_over_time() {
        let mut p = Plasma::new(StdRng::seed_from_u64(1), None, Detail::Medium);
        let mut a = Canvas::new(20, 10);
        let mut b = Canvas::new(20, 10);
        p.update(0.05, &mut a);
        p.update(5.0, &mut b);
        let diff = (0..20)
            .flat_map(|x| (0..10).map(move |y| (x, y)))
            .filter(|&(x, y)| a.get(x, y).color != b.get(x, y).color)
            .count();
        assert!(diff > 100, "plasma should animate");
    }
}
