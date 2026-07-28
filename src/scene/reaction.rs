//! Reaction: Gray-Scott reaction-diffusion — a coral colony grows from a few
//! seeded spots, branches and splits (mitosis regime) as it colonizes the
//! black. When coverage overruns the canvas the field fades out and a fresh
//! colony sprouts far from the old one. Every ~20-30s a nutrient pulse
//! sweeps the field: edges flare as the wavefront passes and growth briefly
//! accelerates. Wrapped (toroidal) boundaries, so colonies crossing an edge
//! read seamlessly on tiled video walls.

use super::noise::fbm;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_out, ease_smooth, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

// Karl Sims parameters, coral / mitosis regime.
const DU: f32 = 1.0;
const DV: f32 = 0.5;
const FEED: f32 = 0.0545;
const KILL: f32 = 0.062;
const SIM_DT: f32 = 1.0;
/// Sim steps per real second; ~3 substeps per frame at 30fps.
const STEPS_PER_SEC: f32 = 90.0;
/// Cap on substeps per frame so a dt hitch can't spiral.
const MAX_SUBSTEPS: usize = 6;
/// Lit-cell fraction that triggers the fade -> reseed cycle.
const COVERAGE_MAX: f32 = 0.6;
const FADE_DUR: f32 = 2.5;
const PULSE_DUR: f32 = 3.2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Grow,
    Fade,
}

pub struct Reaction {
    rng: StdRng,
    detail: Detail,
    base: (u8, u8, u8),
    rim: (u8, u8, u8),
    tip: (u8, u8, u8),
    u: Vec<f32>,
    v: Vec<f32>,
    u2: Vec<f32>,
    v2: Vec<f32>,
    phase: Phase,
    phase_t: f32,
    colony_center: (f32, f32),
    cov_timer: f32,
    sim_acc: f32,
    next_pulse: f32,
    /// <0 inactive, else seconds since the pulse started
    pulse_t: f32,
    pulse_x: f32,
    pulse_y: f32,
    pulse_max_r: f32,
    /// seconds since the current colony was seeded (grow-in envelope)
    grow_t: f32,
    /// seed for the interior mottle noise
    noise_seed: u32,
    t: f32,
    w: usize,
    h: usize,
}

impl Reaction {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (base, rim, tip) = match theme {
            Some("acid") => ((8, 24, 8), (86, 224, 64), (240, 255, 140)),
            Some("ice") => ((8, 14, 30), (84, 190, 240), (240, 250, 255)),
            _ => ((30, 10, 18), (232, 96, 76), (255, 224, 168)), // coral
        };
        Reaction {
            rng,
            detail,
            base,
            rim,
            tip,
            u: Vec::new(),
            v: Vec::new(),
            u2: Vec::new(),
            v2: Vec::new(),
            phase: Phase::Grow,
            phase_t: 0.0,
            colony_center: (0.0, 0.0),
            cov_timer: 1.0,
            sim_acc: 0.0,
            next_pulse: 12.0,
            pulse_t: -1.0,
            pulse_x: 0.0,
            pulse_y: 0.0,
            pulse_max_r: 1.0,
            grow_t: 0.0,
            noise_seed: 0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.u = vec![1.0; w * h];
        self.v = vec![0.0; w * h];
        self.u2 = vec![1.0; w * h];
        self.v2 = vec![0.0; w * h];
        self.phase = Phase::Grow;
        self.phase_t = 0.0;
        self.sim_acc = 0.0;
        if w == 0 || h == 0 {
            return;
        }
        self.noise_seed = self.rng.random::<u32>();
        self.seed_colony(None);
    }

    /// Reset the field and plant a small colony; when `away` is set, pick a
    /// spot far from the old colony so the reseed reads as a fresh start.
    fn seed_colony(&mut self, away: Option<(f32, f32)>) {
        let (w, h) = (self.w, self.h);
        self.u.fill(1.0);
        self.v.fill(0.0);
        self.grow_t = 0.0;
        let (wf, hf) = (w as f32, h as f32);
        let mut cx = self.rng.random_range(wf * 0.25..wf * 0.75);
        let mut cy = self.rng.random_range(hf * 0.25..hf * 0.75);
        if let Some((ox, oy)) = away {
            let mut best_d = -1.0;
            for _ in 0..6 {
                let px = self.rng.random_range(wf * 0.15..wf * 0.85);
                let py = self.rng.random_range(hf * 0.15..hf * 0.85);
                let d = (px - ox).powi(2) + (py - oy).powi(2);
                if d > best_d {
                    best_d = d;
                    cx = px;
                    cy = py;
                }
            }
        }
        let n = self.detail.scale(1.6 * density_for(w, h), 1).min(4);
        let spread = (wf.min(hf) * 0.18).max(4.0);
        let r = ((w.min(h) as f32) / 22.0).clamp(2.0, 4.0) as i32;
        for _ in 0..n {
            let sx = (cx + self.rng.random_range(-spread..spread)) as i32;
            let sy = (cy + self.rng.random_range(-spread..spread)) as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        let xi = (sx + dx).rem_euclid(w as i32) as usize;
                        let yi = (sy + dy).rem_euclid(h as i32) as usize;
                        self.u[yi * w + xi] = 0.5;
                        self.v[yi * w + xi] = 0.5;
                    }
                }
            }
        }
    }

    /// One Gray-Scott step: 3x3 Laplacian (0.2 axial / 0.05 diagonal),
    /// toroidal wrap, double-buffered.
    fn sim_step(&mut self, feed: f32) {
        let (w, h) = (self.w, self.h);
        for y in 0..h {
            let ym = (if y == 0 { h - 1 } else { y - 1 }) * w;
            let yp = (if y + 1 == h { 0 } else { y + 1 }) * w;
            let yc = y * w;
            for x in 0..w {
                let xm = if x == 0 { w - 1 } else { x - 1 };
                let xp = if x + 1 == w { 0 } else { x + 1 };
                let i = yc + x;
                let u = self.u[i];
                let v = self.v[i];
                let lap_u = 0.2
                    * (self.u[yc + xm] + self.u[yc + xp] + self.u[ym + x] + self.u[yp + x])
                    + 0.05
                        * (self.u[ym + xm]
                            + self.u[ym + xp]
                            + self.u[yp + xm]
                            + self.u[yp + xp])
                    - u;
                let lap_v = 0.2
                    * (self.v[yc + xm] + self.v[yc + xp] + self.v[ym + x] + self.v[yp + x])
                    + 0.05
                        * (self.v[ym + xm]
                            + self.v[ym + xp]
                            + self.v[yp + xm]
                            + self.v[yp + xp])
                    - v;
                let uvv = u * v * v;
                self.u2[i] =
                    (u + (DU * lap_u - uvv + feed * (1.0 - u)) * SIM_DT).clamp(0.0, 1.0);
                self.v2[i] =
                    (v + (DV * lap_v + uvv - (feed + KILL) * v) * SIM_DT).clamp(0.0, 1.0);
            }
        }
        std::mem::swap(&mut self.u, &mut self.u2);
        std::mem::swap(&mut self.v, &mut self.v2);
    }

    /// Pulse envelope (0..1) and current wavefront radius.
    /// Anticipation shimmer -> expanding wave payoff -> decay.
    fn pulse_env(&self) -> (f32, f32) {
        if self.pulse_t < 0.0 {
            return (0.0, 0.0);
        }
        let pt = self.pulse_t;
        let env = if pt < 0.8 {
            ease_smooth(pt / 0.8) * 0.35
        } else if pt < 2.5 {
            0.35 + 0.65 * ease_smooth(((pt - 0.8) / 0.4).min(1.0))
        } else {
            1.0 - ease_smooth((pt - 2.5) / (PULSE_DUR - 2.5))
        };
        let sweep = ease_out(((pt - 0.8) / 1.7).clamp(0.0, 1.0));
        (env, sweep * self.pulse_max_r * 1.15)
    }
}

impl Scene for Reaction {
    fn name(&self) -> &'static str {
        "reaction"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;

        // --- nutrient pulse scheduling ---
        self.next_pulse -= dt;
        if self.next_pulse <= 0.0 && self.pulse_t < 0.0 {
            self.pulse_t = 0.0;
            let (wf, hf) = (w as f32, h as f32);
            self.pulse_x = self.rng.random_range(wf * 0.2..wf * 0.8);
            self.pulse_y = self.rng.random_range(hf * 0.2..hf * 0.8);
            // farthest corner (y doubled for cell aspect) sets the sweep range
            self.pulse_max_r = [(0.0, 0.0), (wf, 0.0), (0.0, hf), (wf, hf)]
                .iter()
                .map(|&(cx, cy)| {
                    ((self.pulse_x - cx).powi(2) + ((self.pulse_y - cy) * 2.0).powi(2)).sqrt()
                })
                .fold(0.0f32, f32::max);
            self.next_pulse = self.rng.random_range(20.0..30.0);
        }
        if self.pulse_t >= 0.0 {
            self.pulse_t += dt;
            if self.pulse_t >= PULSE_DUR {
                self.pulse_t = -1.0;
            }
        }
        let (pulse_env, wave_r) = self.pulse_env();

        // --- sim: pulse briefly accelerates growth and shifts the regime ---
        let rate = STEPS_PER_SEC * (1.0 + pulse_env * 1.2);
        let feed = FEED + pulse_env * 0.003;
        self.sim_acc += dt * rate;
        let steps = (self.sim_acc as usize).min(MAX_SUBSTEPS);
        self.sim_acc -= steps as f32;
        for _ in 0..steps {
            self.sim_step(feed);
        }

        // --- coverage check: overgrown fields fade and reseed elsewhere ---
        self.cov_timer -= dt;
        if self.cov_timer <= 0.0 {
            self.cov_timer = 0.5;
            let mut lit = 0usize;
            let (mut sx, mut sy) = (0.0f32, 0.0f32);
            for y in 0..h {
                for x in 0..w {
                    if self.v[y * w + x] > 0.15 {
                        lit += 1;
                        sx += x as f32;
                        sy += y as f32;
                    }
                }
            }
            if lit > 0 {
                self.colony_center = (sx / lit as f32, sy / lit as f32);
            }
            if self.phase == Phase::Grow && lit as f32 / (w * h) as f32 > COVERAGE_MAX {
                self.phase = Phase::Fade;
                self.phase_t = 0.0;
            }
        }
        let mut fade_k = 1.0;
        if self.phase == Phase::Grow {
            self.grow_t += dt;
        }
        if self.phase == Phase::Fade {
            self.phase_t += dt;
            fade_k = 1.0 - ease_smooth(self.phase_t / FADE_DUR);
            let decay = (1.0 - 2.5 * dt).max(0.0);
            for v in self.v.iter_mut() {
                *v *= decay;
            }
            if self.phase_t >= FADE_DUR {
                let old = self.colony_center;
                self.seed_colony(Some(old));
                self.phase = Phase::Grow;
                self.phase_t = 0.0;
            }
        }

        // --- render: rim halo in the void -> deep mottled body -> accent
        // rim at edges -> hot tips at peaks. The grow-in envelope keeps a
        // reseeded colony from popping in at full brightness.
        let env_k = fade_k * ease_smooth(self.grow_t / 1.6);
        canvas.clear((0, 0, 0));
        for y in 0..h {
            let ym = (if y == 0 { h - 1 } else { y - 1 }) * w;
            let yp = (if y + 1 == h { 0 } else { y + 1 }) * w;
            let yc = y * w;
            for x in 0..w {
                let i = yc + x;
                let v = self.v[i];
                let xm = if x == 0 { w - 1 } else { x - 1 };
                let xp = if x + 1 == w { 0 } else { x + 1 };
                if v < 0.02 {
                    // lighting interplay: void cells bordering the colony
                    // catch a faint rim halo; the pulse wavefront flares it
                    let nv = self.v[yc + xm]
                        .max(self.v[yc + xp])
                        .max(self.v[ym + x])
                        .max(self.v[yp + x]);
                    if nv > 0.10 {
                        let mut halo = (nv - 0.10) * 0.85;
                        if pulse_env > 0.0 && self.pulse_t >= 0.8 {
                            let dx = x as f32 - self.pulse_x;
                            let dy = (y as f32 - self.pulse_y) * 2.0;
                            let d = (dx * dx + dy * dy).sqrt();
                            let band = (-((d - wave_r) / 5.0).powi(2)).exp();
                            halo += pulse_env * band * 0.5;
                        }
                        canvas.set(x as i32, y as i32, scale(self.rim, halo * env_k));
                    }
                    continue; // untouched black
                }
                // gradient magnitude marks pattern edges (the growing rim)
                let edge = (((self.v[yc + xp] - self.v[yc + xm]).abs()
                    + (self.v[yp + x] - self.v[ym + x]).abs())
                    * 6.0)
                    .clamp(0.0, 1.0);
                // v tops out near 0.42 in this regime. The body sits deep on
                // the base->rim ramp so rims and tips carry the accents; a
                // slow fbm mottle keeps large interiors from going flat.
                let mottle = 0.82
                    + 0.36
                        * fbm(
                            x as f32 * 0.09,
                            y as f32 * 0.09 + self.t * 0.05,
                            2,
                            self.noise_seed,
                        );
                let body = (v * 1.15 - 0.10).clamp(0.0, 1.0) * mottle;
                let t = (body * 0.5 + edge * 0.55).clamp(0.0, 1.0);
                let mut col = if t < 0.5 {
                    lerp(self.base, self.rim, ease_smooth(t * 2.0))
                } else {
                    lerp(self.rim, self.tip, ease_smooth((t - 0.5) * 2.0))
                };
                if pulse_env > 0.0 {
                    // anticipation: active edges pre-glow
                    let pre = scale(self.rim, pulse_env * 0.25 * edge);
                    col = (
                        col.0.saturating_add(pre.0),
                        col.1.saturating_add(pre.1),
                        col.2.saturating_add(pre.2),
                    );
                    if self.pulse_t >= 0.8 {
                        // payoff: the wavefront flares edges it sweeps over
                        let dx = x as f32 - self.pulse_x;
                        let dy = (y as f32 - self.pulse_y) * 2.0;
                        let d = (dx * dx + dy * dy).sqrt();
                        let band = (-((d - wave_r) / 5.0).powi(2)).exp();
                        let fl = scale(self.tip, pulse_env * band * (0.25 + 0.75 * edge));
                        col = (
                            col.0.saturating_add(fl.0),
                            col.1.saturating_add(fl.1),
                            col.2.saturating_add(fl.2),
                        );
                    }
                }
                canvas.set(x as i32, y as i32, scale(col, env_k));
            }
        }
    }
}
