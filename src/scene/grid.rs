//! Grid: a synthwave perspective plane rolling toward a glowing horizon.
//! Vertical lanes fan out of the vanishing point; horizontal lines compress
//! with distance and scroll toward the viewer on an eased (pow) curve, so
//! they accelerate as they approach. The sky plane is a near-black themed
//! gradient fed by the horizon's glow, with sparse twinkling stars; a
//! scanline-striped sun sliver hangs above a thin bright horizon. Light
//! runners race down random lanes with fading trails, and every ~15-25s a
//! pulse wave brightens the horizon and sky (anticipation), then expands as
//! a luminous ring across the grid toward the camera (payoff), lighting
//! every line it crosses before decaying.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

/// Depth → screen-y curve exponent: >1 compresses lines near the horizon.
const PERSP: f32 = 2.4;
/// Pulse ring half-width in depth space.
const PULSE_BAND: f32 = 0.09;
/// Pulse travel time in seconds (horizon → past the camera).
const PULSE_TIME: f32 = 2.4;

struct Palette {
    near: (u8, u8, u8),    // grid lines close to the camera
    far: (u8, u8, u8),     // grid lines dissolving into the horizon
    horizon: (u8, u8, u8), // the thin horizon line
    sun_hot: (u8, u8, u8),
    sun_cool: (u8, u8, u8),
    runner: (u8, u8, u8),
}

/// A bright streak racing down one lane toward the viewer.
struct Runner {
    lane: usize,
    p: f32,    // eased phase 0 (horizon) .. 1.2 (past camera)
    rate: f32, // phase per second
}

/// Pulse wave: phase < 0 is the anticipation (horizon charging up),
/// 0..1 is the ring expanding toward the camera.
struct Pulse {
    phase: f32,
}

/// A faint fixed star in the sky plane above the horizon.
struct Star {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
    speed: f32,
}

pub struct Grid {
    rng: StdRng,
    detail: Detail,
    pal: Palette,
    lanes: Vec<f32>, // normalized fan offsets, -1..1
    stars: Vec<Star>,
    runners: Vec<Runner>,
    pulse: Option<Pulse>,
    next_pulse: f32,
    scroll: f32, // horizontal-line phase, wraps 0..1
    t: f32,
    h_lines: usize,
    runner_cap: usize,
    w: usize,
    h: usize,
}

impl Grid {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let pal = match theme {
            Some("lime") => Palette {
                near: (150, 255, 70),
                far: (18, 80, 40),
                horizon: (190, 255, 120),
                sun_hot: (240, 255, 170),
                sun_cool: (130, 230, 70),
                runner: (230, 255, 200),
            },
            Some("mono") => Palette {
                near: (225, 225, 235),
                far: (55, 55, 68),
                horizon: (255, 255, 255),
                sun_hot: (250, 250, 255),
                sun_cool: (150, 150, 165),
                runner: (255, 255, 255),
            },
            _ => Palette {
                // vapor: magenta grid over deep purple, cyan horizon
                near: (255, 70, 220),
                far: (70, 35, 150),
                horizon: (80, 225, 255),
                sun_hot: (255, 205, 120),
                sun_cool: (255, 70, 170),
                runner: (190, 240, 255),
            },
        };
        Grid {
            rng,
            detail,
            pal,
            lanes: Vec::new(),
            stars: Vec::new(),
            runners: Vec::new(),
            pulse: None,
            next_pulse: 8.0,
            scroll: 0.0,
            t: 0.0,
            h_lines: 8,
            runner_cap: 3,
            w: 0,
            h: 0,
        }
    }
}

impl Scene for Grid {
    fn name(&self) -> &'static str {
        "grid"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);

        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            let n_lanes = self
                .detail
                .scale(w as f32 / 8.0 * density_for(w, h), 8)
                .min(96);
            let denom = (n_lanes.max(2) - 1) as f32;
            self.lanes = (0..n_lanes)
                .map(|i| -1.0 + 2.0 * i as f32 / denom)
                .collect();
            let ground = (h as f32 * 0.6).max(1.0);
            self.h_lines = self
                .detail
                .scale(ground / 2.2 * density_for(w, h), 6)
                .min(64);
            self.runner_cap = self.detail.scale(6.0 * density_for(w, h), 3).min(20);
            // sky plane: sparse fixed stars above the horizon
            let sky = (h as f32 * 0.4).max(1.0);
            let n_stars = self
                .detail
                .scale(w as f32 * sky / 240.0 * density_for(w, h), 5)
                .min(140);
            self.stars = (0..n_stars)
                .map(|_| Star {
                    x: self.rng.random_range(0.0..w as f32),
                    y: self.rng.random_range(0.0..sky * 0.92),
                    mag: self.rng.random_range(0.08..0.30),
                    phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                    speed: self.rng.random_range(0.4..1.4),
                })
                .collect();
            self.runners.clear();
            self.pulse = None;
            self.next_pulse = self.rng.random_range(5.0..9.0);
        }
        self.t += dt;

        let hy = (h as f32 * 0.4).floor();
        let gh = (h as f32 - hy).max(1.0);
        // the whole scene sways gently around the vanishing point
        let cx = w as f32 / 2.0 + (self.t * 0.09).sin() * w as f32 * 0.02;
        let spread = w as f32 * 0.62;

        // --- pulse wave scheduling (anticipation -> payoff -> decay) ---
        self.next_pulse -= dt;
        if self.next_pulse <= 0.0 && self.pulse.is_none() {
            self.pulse = Some(Pulse { phase: -0.8 });
        }
        if let Some(p) = &mut self.pulse {
            p.phase += dt / PULSE_TIME;
            if p.phase >= 1.0 {
                self.pulse = None;
                self.next_pulse = self.rng.random_range(15.0..25.0);
            }
        }
        let (ring_d, ring_amp, anticipation) = match &self.pulse {
            Some(p) if p.phase < 0.0 => (-1.0, 0.0, ease_smooth((p.phase + 0.8) / 0.8)),
            Some(p) => (
                // eased expansion: slow off the horizon, sweeping in fast
                1.35 * p.phase.powf(1.7),
                // decay as the ring spends its energy
                (1.0 - p.phase * 0.55) * 1.1,
                1.0,
            ),
            None => (-1.0, 0.0, 0.0),
        };
        // brightness boost for anything at depth d inside the ring
        let boost = |d: f32| -> f32 {
            if ring_amp <= 0.0 {
                0.0
            } else {
                let x = (d - ring_d) / PULSE_BAND;
                ring_amp * (-x * x).exp()
            }
        };

        // eased scroll: constant phase speed + pow depth curve = lines
        // accelerate as they roll toward the viewer
        self.scroll = (self.scroll + dt * 0.2).fract();

        // --- draw ---
        canvas.clear((0, 0, 0));

        // sky plane: near-black themed gradient rising from the horizon's
        // glow (brightens while the pulse charges), then sparse stars
        let sky_glow = 0.10 + anticipation * 0.14;
        for y in 0..hy as i32 {
            let t = (y as f32 / hy).powi(2);
            canvas.fill_row(
                y as usize,
                lerp(
                    scale(self.pal.far, 0.08),
                    scale(self.pal.horizon, sky_glow),
                    t,
                ),
            );
        }
        let star_tint = lerp((255, 255, 255), self.pal.horizon, 0.45);
        for s in &self.stars {
            let tw = 0.6 + 0.4 * (self.t * s.speed + s.phase).sin();
            // stars drown in the horizon glow near the skyline
            let drown = 1.0 - (s.y / hy.max(1.0)).powi(3) * 0.5;
            canvas.set_f(s.x, s.y, scale(star_tint, s.mag * tw * drown));
        }

        // sun sliver above the horizon, slowly pulsing; flares with the pulse
        let sr = (h as f32 * 0.11).clamp(2.0, 12.0);
        let sun_pulse = (0.78 + 0.22 * (self.t * 0.9).sin()) * (1.0 + anticipation * 0.5);
        let sun_cy = hy - sr * 0.55;
        let r = sr * (1.0 + 0.05 * (self.t * 0.7 + 1.3).sin());
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            // synthwave scanline slits, widening toward the sun's base and
            // drifting slowly downward
            if dy > 0 && r >= 3.5 {
                let v = dy as f32 / r; // 0..1 down the lower half
                let period = 2.0 + v * 2.0;
                let slit = (dy as f32 + self.t * 0.7).rem_euclid(period);
                if slit < 0.5 + v * 1.3 {
                    continue;
                }
            }
            for dx in -ri..=ri {
                let dd = ((dx * dx + dy * dy) as f32).sqrt() / r;
                if dd <= 1.0 {
                    let y = sun_cy as i32 + dy;
                    if (y as f32) < hy {
                        let c = lerp(self.pal.sun_cool, self.pal.sun_hot, (1.0 - dd).powf(1.5));
                        canvas.add(
                            cx as i32 + dx,
                            y,
                            scale(c, sun_pulse * (1.0 - dd * 0.55) * 0.9),
                        );
                    }
                }
            }
        }
        glow(
            canvas,
            cx as i32,
            sun_cy as i32,
            ri + 2,
            self.pal.sun_cool,
            0.08 * sun_pulse,
        );

        // thin bright horizon line; flares during pulse anticipation
        let h_bright = (0.5 + 0.2 * sun_pulse + anticipation * 0.9 + boost(0.02)).min(1.6);
        for x in 0..w {
            canvas.add(x as i32, hy as i32, scale(self.pal.horizon, h_bright));
            canvas.add(x as i32, hy as i32 - 1, scale(self.pal.horizon, h_bright * 0.22));
        }

        // vertical lanes fanning from the vanishing point
        let inv = 1.0 / PERSP;
        for y in (hy as i32 + 1)..h as i32 {
            let d = ((y as f32 - hy) / gh).powf(inv);
            if d < 0.04 {
                continue; // lanes collapse into the vanishing point
            }
            let bo = boost(d);
            let b = ((0.16 + 0.84 * d) * (1.0 + bo)).min(1.4);
            let mut lc = lerp(self.pal.far, self.pal.near, d);
            if bo > 0.05 {
                lc = lerp(lc, (255, 255, 255), (bo * 0.35).min(0.6));
            }
            let c = scale(lc, b);
            for &u in &self.lanes {
                canvas.add((cx + u * spread * d) as i32, y, c);
            }
        }

        // horizontal lines scrolling toward the viewer
        for k in 0..self.h_lines {
            let p = (k as f32 / self.h_lines as f32 + self.scroll).fract();
            let d = p.powf(PERSP);
            if d < 0.02 {
                continue;
            }
            let y = (hy + gh * d) as i32;
            if y >= h as i32 {
                continue;
            }
            let bo = boost(d);
            let b = ((0.22 + 0.78 * d) * (1.0 + bo)).min(1.5);
            let mut lc = lerp(self.pal.far, self.pal.near, d);
            if bo > 0.05 {
                lc = lerp(lc, (255, 255, 255), (bo * 0.4).min(0.7));
            }
            let c = scale(lc, b);
            for x in 0..w {
                canvas.add(x as i32, y, c);
            }
        }

        // --- light runners racing down the lanes ---
        if self.runners.len() < self.runner_cap && self.rng.random::<f32>() < 1.4 * dt {
            let lane = self.rng.random_range(0..self.lanes.len());
            self.runners.push(Runner {
                lane,
                p: 0.0,
                rate: self.rng.random_range(0.30..0.55),
            });
        }
        let mut i = 0;
        while i < self.runners.len() {
            let r = &mut self.runners[i];
            r.p += r.rate * dt;
            if r.p > 1.2 {
                self.runners.swap_remove(i);
                continue;
            }
            let u = self.lanes[r.lane];
            // head plus a fading trail of earlier (eased) positions
            for j in 0..=6usize {
                let tp = r.p - j as f32 * 0.028;
                if tp <= 0.0 {
                    break;
                }
                let d = tp.powf(1.8);
                let fall = if j == 0 {
                    1.0
                } else {
                    let k = 1.0 - j as f32 / 7.0;
                    k * k * 0.55
                };
                let b = (fall * (0.5 + 0.5 * d) * (1.0 + boost(d))).min(1.5);
                let c = scale(lerp(self.pal.runner, (255, 255, 255), 0.4), b);
                canvas.add((cx + u * spread * d) as i32, (hy + gh * d) as i32, c);
            }
            // soft glow around the head, growing as it nears the camera
            let d = r.p.min(1.0).powf(1.8);
            glow(
                canvas,
                (cx + u * spread * d) as i32,
                (hy + gh * d) as i32,
                2,
                self.pal.runner,
                0.5 * (0.4 + 0.6 * d),
            );
            i += 1;
        }
    }
}
