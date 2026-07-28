//! Conway's Game of Life: random soup on a faint breathing grid, newborns
//! kindle up to full brightness with a small glow bleed, dead cells cool
//! through a desaturating ember trail. Detects stagnation (static or
//! periodic) and reseeds with an eased ring sweep that blooms hue into
//! neighbors; glider injections run anticipation → flash → ember wash.
//! Sparse near-plane spores drift above the grid and brighten on events.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, hsv, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::collections::VecDeque;

const STEP_RATE: f32 = 12.0; // generations per second
const STAGNANT_LIMIT: usize = 20;
const HASH_WINDOW: usize = 16;
const PULSE_TIME: f32 = 1.4; // reseed ring-sweep duration
const INJECT_ANTICIPATE: f32 = 1.2;
const INJECT_WASH: f32 = 1.8;

/// Glider inject rhythm: countdown → darken → flash+drop → ember wash.
#[derive(Clone, Copy)]
enum Inject {
    Idle,
    Anticipate(f32), // elapsed toward flash
    Wash(f32),       // post-flash ember wash
}

/// Out-of-focus spore mote on the near plane.
struct Spore {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
    size: f32,
}

pub struct Life {
    rng: StdRng,
    detail: Detail,
    cells: Vec<bool>,
    next: Vec<bool>,
    fade: Vec<f32>,
    hue_at: Vec<f32>,
    age: Vec<u16>,
    inject_in: f32,
    inject: Inject,
    inject_pos: Option<(i32, i32)>,
    inject_fx: f32,
    hue: f32,
    hue0: f32,
    pulse: f32, // time since the last reseed (ring sweep while < PULSE_TIME)
    /// Extra brightness bloom that tints live cells during reseed.
    reseed_bloom: f32,
    sat: f32,
    t: f32,
    generations: u64,
    stagnant: usize,
    reseeds: usize,
    hashes: VecDeque<u64>,
    step_acc: f32,
    spores: Vec<Spore>,
    w: usize,
    h: usize,
}

impl Life {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (hue0, sat) = match theme {
            Some("ember") => (0.05, 0.85),
            Some("ice") => (0.55, 0.6),
            _ => (0.35, 0.65),
        };
        Life {
            rng,
            detail,
            cells: Vec::new(),
            next: Vec::new(),
            fade: Vec::new(),
            hue_at: Vec::new(),
            age: Vec::new(),
            inject_in: 12.0,
            inject: Inject::Idle,
            inject_pos: None,
            inject_fx: 0.0,
            hue: hue0,
            hue0,
            pulse: PULSE_TIME, // no sweep until the first reseed
            reseed_bloom: 0.0,
            sat,
            t: 0.0,
            generations: 0,
            stagnant: 0,
            reseeds: 0,
            hashes: VecDeque::new(),
            step_acc: 0.0,
            spores: Vec::new(),
            w: 0,
            h: 0,
        }
    }

    fn soup_density(&self) -> f32 {
        0.18 + 0.08 * self.detail.factor().min(1.5)
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.cells = vec![false; w * h];
        self.next = vec![false; w * h];
        self.fade = vec![0.0; w * h];
        self.hue_at = vec![0.0; w * h];
        self.age = vec![0; w * h];
        self.hashes.clear();
        self.stagnant = 0;
        let n = ((6.0 * density_for(w, h) * self.detail.factor()) as usize).clamp(3, 24);
        self.spores = (0..n)
            .map(|_| Spore {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                vx: self.rng.random_range(-4.0..4.0),
                vy: self.rng.random_range(-3.0..3.0),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                size: self.rng.random_range(0.6..1.4),
            })
            .collect();
        self.reseed();
    }

    fn reseed(&mut self) {
        self.pulse = 0.0; // starts the ring sweep
        self.reseed_bloom = 1.0;
        let dens = self.soup_density();
        for c in &mut self.cells {
            *c = self.rng.random::<f32>() < dens;
        }
        self.age.fill(0);
        self.fade.fill(0.0);
        self.hashes.clear();
        self.stagnant = 0;
        self.reseeds += 1;
    }

    /// FNV-1a over the cell bits, for periodic-repeat detection.
    fn hash(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for &c in &self.cells {
            h ^= c as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// One generation. Returns (births, deaths).
    pub fn step(&mut self) -> (usize, usize) {
        let (w, h) = (self.w, self.h);
        if w < 3 || h < 3 {
            return (0, 0);
        }
        let mut births = 0;
        let mut deaths = 0;
        for y in 0..h {
            for x in 0..w {
                let mut n = 0;
                for dy in [-1i32, 0, 1] {
                    for dx in [-1i32, 0, 1] {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                            n += self.cells[ny as usize * w + nx as usize] as usize;
                        }
                    }
                }
                let alive = self.cells[y * w + x];
                let lives = n == 3 || (alive && n == 2);
                self.next[y * w + x] = lives;
                match (alive, lives) {
                    (false, true) => births += 1,
                    (true, false) => deaths += 1,
                    _ => {}
                }
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        self.generations += 1;
        // hue drifts slowly with the generations
        self.hue = self.hue0 + self.generations as f32 * 0.004;
        (births, deaths)
    }

    /// Stagnation / periodicity bookkeeping; reseeds when stuck.
    fn check_stagnation(&mut self, births: usize, deaths: usize) {
        if births + deaths == 0 {
            self.stagnant += 1;
        } else {
            self.stagnant = 0;
        }
        // periodic repeat detection: only record states that changed since
        // the previous generation (still configurations are already covered
        // by the stagnant counter above)
        if births + deaths > 0 {
            let hash = self.hash();
            if self.hashes.contains(&hash) {
                self.reseed();
                return;
            }
            self.hashes.push_back(hash);
            while self.hashes.len() > HASH_WINDOW {
                self.hashes.pop_front();
            }
        }
        if self.stagnant > STAGNANT_LIMIT {
            self.reseed();
        }
    }

    fn drop_glider(&mut self, gy: i32) {
        let (w, h) = (self.w, self.h);
        let glider = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];
        for (dx, dy) in glider {
            let (x, y) = (1 + dx, gy + dy);
            if (y as usize) < h && (x as usize) < w {
                self.cells[y as usize * w + x as usize] = true;
                let i = y as usize * w + x as usize;
                self.fade[i] = 1.0;
                self.hue_at[i] = self.hue;
            }
        }
        self.inject_pos = Some((3, gy + 1));
        self.inject_fx = 1.0;
    }
}

impl Scene for Life {
    fn name(&self) -> &'static str {
        "life"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        self.t += dt;
        self.reseed_bloom = (self.reseed_bloom - dt * 0.7).max(0.0);

        self.step_acc += dt * STEP_RATE;
        while self.step_acc >= 1.0 {
            self.step_acc -= 1.0;
            let (births, deaths) = self.step();
            self.check_stagnation(births, deaths);
        }

        // --- inject event: anticipation → flash+drop → ember wash ---
        match self.inject {
            Inject::Idle => {
                self.inject_in -= dt;
                if self.inject_in <= 0.0 {
                    let base = 18.0 / self.detail.factor().max(0.5);
                    self.inject_in = self.rng.random_range(base..(base + 12.0));
                    let gy = self.rng.random_range(2..(h as i32 - 4).max(3));
                    self.inject_pos = Some((3, gy + 1));
                    self.inject = Inject::Anticipate(0.0);
                }
            }
            Inject::Anticipate(e) => {
                let e = e + dt;
                if e >= INJECT_ANTICIPATE {
                    if let Some((_, gy)) = self.inject_pos {
                        self.drop_glider(gy - 1);
                    }
                    self.inject = Inject::Wash(0.0);
                } else {
                    self.inject = Inject::Anticipate(e);
                }
            }
            Inject::Wash(e) => {
                let e = e + dt;
                self.inject = if e >= INJECT_WASH {
                    Inject::Idle
                } else {
                    Inject::Wash(e)
                };
            }
        }

        // reseed event: a brief hue-tinted flash, then an eased ring sweeps
        // the field and lights everything it crosses
        self.pulse += dt;
        let flash = (1.0 - self.pulse / 0.4).max(0.0);
        canvas.clear(scale(hsv(self.hue, 0.55, 0.10), flash));
        let ring = if self.pulse < PULSE_TIME {
            let k = self.pulse / PULSE_TIME;
            let max_r = ((w * w) as f32 * 0.25 + (h * h) as f32).sqrt();
            Some((ease_smooth(k) * max_r, 1.0 - k))
        } else {
            None
        };

        // inject anticipation: local darkening around drop site
        let (anti_k, wash_k) = match self.inject {
            Inject::Anticipate(e) => (ease_smooth(e / INJECT_ANTICIPATE), 0.0),
            Inject::Wash(e) => (0.0, 1.0 - ease_smooth(e / INJECT_WASH)),
            Inject::Idle => (0.0, 0.0),
        };

        let glow_r = self.detail.scale(1.5, 1) as i32;
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if self.cells[i] {
                    self.age[i] = self.age[i].saturating_add(1);
                    // kindle: newborns ease up to full brightness, no popping
                    self.fade[i] = (self.fade[i] + dt * 6.0).min(1.0);
                    self.hue_at[i] = self.hue;
                } else {
                    self.age[i] = 0;
                    // dead: cool through a fading trail
                    self.fade[i] *= (-2.6 * dt).exp();
                }
                let f = self.fade[i];
                if f > 0.04 {
                    let age_k = (self.age[i] as f32 / 40.0).min(1.0);
                    let newborn = self.cells[i] && self.age[i] <= 3 && f > 0.4;
                    let bloom = self.reseed_bloom * 0.25;
                    let col = if self.cells[i] {
                        // young cells bright, old cells deep and saturated
                        let base = hsv(
                            self.hue_at[i] + age_k * 0.06 + bloom * 0.08,
                            (self.sat + age_k * 0.3).min(1.0),
                            (0.25 + 0.75 * f * (1.0 - age_k * 0.35) + bloom).min(1.0),
                        );
                        if newborn {
                            // white-hot core that cools into the colony hue
                            lerp(base, (255, 255, 255), 0.45 * (1.0 - age_k * 12.0).max(0.0))
                        } else {
                            base
                        }
                    } else {
                        // cooling trail: darker, desaturating, hue drifting —
                        // stays out of the muddy mid-tones
                        hsv(
                            self.hue_at[i] + (1.0 - f) * 0.10,
                            self.sat * (0.35 + 0.65 * f),
                            0.05 + 0.75 * f * f,
                        )
                    };
                    canvas.set(x as i32, y as i32, col);
                    // newborn glow bleed: fresh births light their neighbors
                    if newborn {
                        glow(canvas, x as i32, y as i32, glow_r, col, 0.25);
                    }
                    // reseed bloom: live cells tint neighbors
                    if self.cells[i] && self.reseed_bloom > 0.05 {
                        glow(
                            canvas,
                            x as i32,
                            y as i32,
                            1,
                            hsv(self.hue, self.sat, 1.0),
                            self.reseed_bloom * 0.12,
                        );
                    }
                } else if x % 4 == 0 && y % 4 == 0 {
                    // faint background grid, breathing with the drifting hue
                    let breathe =
                        0.5 + 0.5 * (self.t * 0.6 + x as f32 * 0.11 + y as f32 * 0.17).sin();
                    canvas.set(
                        x as i32,
                        y as i32,
                        hsv(self.hue, 0.5, 0.025 + 0.03 * breathe),
                    );
                }
                // reseed ring: additive accent band crossing the field
                if let Some((rr, k)) = ring {
                    let dx = x as f32 - cx;
                    let dy = (y as f32 - cy) * 2.0; // cell aspect
                    let dd = (dx * dx + dy * dy).sqrt() - rr;
                    if dd.abs() < 2.8 {
                        let fall = 1.0 - dd.abs() / 2.8;
                        canvas.add(
                            x as i32,
                            y as i32,
                            scale(hsv(self.hue, self.sat, 1.0), fall * fall * 0.55 * k),
                        );
                        // lighting bleed: ring tints cells it crosses
                        if self.cells[i] {
                            canvas.add(
                                x as i32,
                                y as i32,
                                scale(hsv(self.hue + 0.05, 0.4, 1.0), fall * 0.35 * k),
                            );
                        }
                    }
                }

                // inject anticipation darkening / wash brightening
                if let Some((ix, iy)) = self.inject_pos {
                    let dx = (x as i32 - ix) as f32;
                    let dy = ((y as i32 - iy) as f32) * 2.0;
                    let d2 = dx * dx + dy * dy;
                    if anti_k > 0.0 && d2 < 220.0 {
                        let fall = (1.0 - d2 / 220.0) * anti_k * 0.55;
                        let cur = canvas.get(x as i32, y as i32).color;
                        canvas.set(x as i32, y as i32, scale(cur, 1.0 - fall));
                    }
                    if wash_k > 0.0 && d2 < 280.0 {
                        let fall = (1.0 - d2 / 280.0) * wash_k;
                        canvas.add(
                            x as i32,
                            y as i32,
                            scale(hsv(self.hue, self.sat * 0.7, 1.0), fall * 0.22),
                        );
                    }
                }
            }
        }

        // glider arrival flash
        if self.inject_fx > 0.0 {
            self.inject_fx = (self.inject_fx - dt * 1.5).max(0.0);
            if let Some((ix, iy)) = self.inject_pos {
                glow(
                    canvas,
                    ix,
                    iy,
                    3 + glow_r,
                    hsv(self.hue, self.sat, 1.0),
                    self.inject_fx * 0.65,
                );
            }
        }

        // near-plane spores
        let event_boost = (self.reseed_bloom * 0.5
            + anti_k * 0.3
            + wash_k * 0.45
            + self.inject_fx * 0.4)
            .min(1.0);
        for s in &mut self.spores {
            s.phase += dt * 1.8;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            if s.x < 0.0 {
                s.x += w as f32;
            } else if s.x >= w as f32 {
                s.x -= w as f32;
            }
            if s.y < 0.0 {
                s.y += h as f32;
            } else if s.y >= h as f32 {
                s.y -= h as f32;
            }
            let b = 0.2 + 0.25 * (s.phase.sin() * 0.5 + 0.5) + event_boost * 0.55;
            let col = hsv(self.hue + 0.04, self.sat * 0.5, b);
            glow(
                canvas,
                s.x as i32,
                s.y as i32,
                if s.size > 1.0 { 2 } else { 1 },
                col,
                0.25 + 0.35 * event_boost,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    /// A 2x2 block is a still life: no births, no deaths — must trigger
    /// the stagnation reseed.
    #[test]
    fn stagnation_triggers_reseed() {
        let mut l = Life::new(StdRng::seed_from_u64(5), None, Detail::Medium);
        l.init(10, 10);
        // wipe to a lone still-life block
        l.cells.fill(false);
        for (x, y) in [(4, 4), (5, 4), (4, 5), (5, 5)] {
            l.cells[y * 10 + x] = true;
        }
        l.hashes.clear();
        l.stagnant = 0;
        let before = l.reseeds;
        for _ in 0..STAGNANT_LIMIT + 5 {
            let (b, d) = l.step();
            l.check_stagnation(b, d);
        }
        assert!(l.reseeds > before, "still life should be reseeded");
        // after reseed the soup is much denser than 4 cells
        let pop = l.cells.iter().filter(|&&c| c).count();
        assert!(pop > 4, "reseed should produce a fresh soup, got {pop}");
    }

    #[test]
    fn block_is_stable_before_limit() {
        let mut l = Life::new(StdRng::seed_from_u64(6), None, Detail::Medium);
        l.init(10, 10);
        l.cells.fill(false);
        for (x, y) in [(4, 4), (5, 4), (4, 5), (5, 5)] {
            l.cells[y * 10 + x] = true;
        }
        l.hashes.clear();
        for _ in 0..3 {
            let (b, d) = l.step();
            assert_eq!((b, d), (0, 0), "2x2 block must be a still life");
            l.check_stagnation(b, d);
        }
        assert_eq!(l.reseeds, 1); // only the init reseed
    }

    #[test]
    fn periodic_oscillator_triggers_reseed() {
        // a blinker is period-2: its states must be caught by hash repetition
        let mut l = Life::new(StdRng::seed_from_u64(9), None, Detail::Medium);
        l.init(10, 10);
        l.cells.fill(false);
        for x in 3..6 {
            l.cells[5 * 10 + x] = true;
        }
        l.hashes.clear();
        l.stagnant = 0;
        let before = l.reseeds;
        for _ in 0..6 {
            let (b, d) = l.step();
            l.check_stagnation(b, d);
        }
        assert!(l.reseeds > before, "period-2 blinker should be reseeded");
    }

    #[test]
    fn runs_on_canvas_without_panic() {
        let mut l = Life::new(StdRng::seed_from_u64(7), None, Detail::Medium);
        let mut c = Canvas::new(30, 20);
        for _ in 0..120 {
            l.update(1.0 / 30.0, &mut c);
        }
    }
}
