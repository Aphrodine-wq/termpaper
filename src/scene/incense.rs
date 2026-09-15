//! Incense: a thin smoke ribbon rising from a glowing ember in a dark
//! room. Laminar and nearly straight just above the stick, the ribbon
//! picks up sway with altitude and shears into curling vortices near the
//! top before dissipating. Drawn entirely with subcell primitives (`dot`,
//! `line_f`, `glow_f`) so the advection reads as continuous drift, never
//! cell-snapped. The ember breathes on an eased envelope and occasionally
//! flares, thickening the ribbon for a moment; spent ash drops now and
//! then as the stick burns down.

use super::noise::vnoise;
use super::{Detail, Scene};
use crate::canvas::{dot, ease_out, ease_smooth, glow_f, lerp, line_f, scale, Canvas};
use crate::physics::{integrate, Body, Forces};
use rand::{rngs::StdRng, RngExt};

/// Seconds of sim time between ribbon particle emissions.
const EMIT_EVERY: f32 = 0.055;
/// Ribbon particle lifetime bounds (seconds).
const LIFE: (f32, f32) = (5.0, 8.0);

/// One tracer along the smoke ribbon. Consecutive ids are joined into a
/// polyline, so the ribbon stays a connected thread as it deforms.
struct Puff {
    b: Body,
    age: f32,
    life: f32,
    /// per-puff phase so the sway decorrelates slowly along the ribbon
    seed: f32,
    /// emission counter; a gap in ids (dropped puff) breaks the polyline
    id: u64,
}

struct Ash {
    b: Body,
    life: f32,
}

pub struct Incense {
    rng: StdRng,
    detail: Detail,
    // palette
    ember_c: (u8, u8, u8),
    ember_hot: (u8, u8, u8),
    smoke_lo: (u8, u8, u8),
    smoke_hi: (u8, u8, u8),
    stick_c: (u8, u8, u8),
    bg_top: (u8, u8, u8),
    bg_bot: (u8, u8, u8),
    puffs: Vec<Puff>,
    ash: Vec<Ash>,
    emit_t: f32,
    emit_id: u64,
    /// slow breathing clock for the ember envelope
    t: f32,
    /// >0 while the ember flares (decays to 0)
    flare: f32,
    next_flare: f32,
    next_ash: f32,
    w: usize,
    h: usize,
    tip_x: f32,
    tip_y: f32,
}

impl Incense {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (ember_c, ember_hot, smoke_lo, smoke_hi, stick_c, bg_top, bg_bot) = match theme {
            // golden hall: warm smoke over deep brown dusk
            Some("temple") => (
                (255, 170, 60),
                (255, 235, 170),
                (150, 130, 105),
                (235, 215, 180),
                (95, 70, 45),
                (16, 10, 6),
                (30, 20, 12),
            ),
            // cool night: violet-gray ribbon in blue dark
            Some("midnight") => (
                (255, 120, 70),
                (255, 220, 160),
                (110, 105, 150),
                (200, 195, 235),
                (70, 60, 70),
                (5, 6, 14),
                (12, 12, 26),
            ),
            // ink on paper: near-mono grays
            Some("zen") => (
                (235, 120, 60),
                (255, 220, 170),
                (105, 105, 110),
                (215, 215, 220),
                (60, 58, 55),
                (7, 7, 8),
                (16, 16, 18),
            ),
            // sandalwood (default): amber ember, blue-gray smoke
            _ => (
                (255, 140, 60),
                (255, 225, 160),
                (115, 120, 140),
                (210, 218, 235),
                (80, 62, 48),
                (8, 7, 10),
                (20, 16, 18),
            ),
        };
        Incense {
            rng,
            detail,
            ember_c,
            ember_hot,
            smoke_lo,
            smoke_hi,
            stick_c,
            bg_top,
            bg_bot,
            puffs: Vec::new(),
            ash: Vec::new(),
            emit_t: 0.0,
            emit_id: 0,
            t: 0.0,
            flare: 0.0,
            next_flare: 7.0,
            next_ash: 11.0,
            w: 0,
            h: 0,
            tip_x: 0.0,
            tip_y: 0.0,
        }
    }

    /// Ember brightness envelope: slow breathing plus the flare payoff.
    fn ember_env(&self) -> f32 {
        let breathe = 0.55 + 0.25 * (self.t * 0.9).sin() + 0.08 * (self.t * 3.7).sin();
        (breathe + ease_out(self.flare) * 0.8).clamp(0.0, 1.6)
    }
}

impl Scene for Incense {
    fn name(&self) -> &'static str {
        "incense"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            // stick tip sits low, a little right of center
            self.tip_x = w as f32 * 0.56;
            self.tip_y = h as f32 * 0.82;
            self.puffs.clear();
            self.ash.clear();
        }
        let (wf, hf) = (w as f32, h as f32);

        // ember rhythm: an occasional slow flare, and ash dropping after
        self.next_flare -= dt;
        if self.next_flare <= 0.0 {
            self.flare = 1.0;
            self.next_flare = self.rng.random_range(9.0..18.0);
        }
        self.flare = (self.flare - dt * 0.6).max(0.0);
        self.next_ash -= dt;
        if self.next_ash <= 0.0 {
            self.next_ash = self.rng.random_range(13.0..24.0);
            let mut b = Body::new(self.tip_x, self.tip_y);
            b.set_velocity(
                self.rng.random_range(-1.5..1.5),
                self.rng.random_range(0.5..2.0),
                dt.max(1e-3),
            );
            self.ash.push(Ash { b, life: 3.0 });
        }

        let env = self.ember_env();

        // emit ribbon tracers on a steady clock; detail densifies the
        // tracer spacing (smoother thread) and the flare thickens the
        // stream by emitting a touch faster and hotter
        let cap = self.detail.scale(140.0, 60).min(400);
        let interval = EMIT_EVERY / self.detail.density().max(0.4);
        self.emit_t -= dt;
        while self.emit_t <= 0.0 {
            self.emit_t += interval / (1.0 + self.flare * 0.6);
            if self.puffs.len() < cap {
                let mut b = Body::new(
                    self.tip_x + self.rng.random_range(-0.2..0.2),
                    self.tip_y - 0.5,
                );
                b.set_velocity(
                    self.rng.random_range(-0.6..0.6),
                    -self.rng.random_range(5.5..7.5) * (0.8 + env * 0.3),
                    dt.max(1e-3),
                );
                self.puffs.push(Puff {
                    b,
                    age: 0.0,
                    life: self.rng.random_range(LIFE.0..LIFE.1),
                    seed: self.rng.random_range(0.0..std::f32::consts::TAU),
                    id: self.emit_id,
                });
                self.emit_id += 1;
            }
        }

        // advect the ribbon: buoyant lift, drag, sway that grows with
        // altitude, and a curl field that shears the top into vortices
        let t = self.t;
        let tip_y = self.tip_y;
        self.puffs.retain_mut(|p| {
            p.age += dt;
            if p.age >= p.life {
                return false;
            }
            let alt = (tip_y - p.b.y).max(0.0);
            let rise = ease_smooth(alt / (hf * 0.25));
            // laminar low: barely any side force near the stick. Higher up
            // the column sways, then vortex curl takes over near the top.
            let sway = (t * 0.55 + alt * 0.09 + p.seed * 0.15).sin() * 3.0 * rise;
            let curl =
                (vnoise(p.b.x * 0.09, p.b.y * 0.07 - t * 0.18, 7) - 0.5) * 26.0 * rise * rise;
            let f = Forces {
                gravity: -9.0 * (1.0 - (p.age / p.life) * 0.55), // old smoke loses lift
                drag: 1.1,
                wind_x: 0.0,
                wind_y: 0.0,
                ax: sway + curl,
                ay: 0.0,
            };
            integrate(&mut p.b, &f, dt);
            p.b.y > -3.0
        });

        // falling ash: a brief gray fleck with a dying ember core
        self.ash.retain_mut(|a| {
            a.life -= dt;
            let f = Forces {
                gravity: 26.0,
                drag: 1.6,
                ..Default::default()
            };
            integrate(&mut a.b, &f, dt);
            a.life > 0.0 && a.b.y < hf + 2.0
        });

        // ---- draw ----

        // room: vertical gradient, darkest at the top
        for y in 0..h {
            let c = lerp(self.bg_top, self.bg_bot, y as f32 / hf.max(1.0));
            for x in 0..w {
                canvas.set(x as i32, y as i32, c);
            }
        }

        // warm pool of light around the ember, breathing with it
        glow_f(
            canvas,
            self.tip_x,
            self.tip_y,
            5.0 + env * 2.0,
            self.ember_c,
            0.10 + env * 0.08,
        );

        // the stick: a slim diagonal from the lower edge up to the tip,
        // with a pale ash cap just below the ember
        let base_x = self.tip_x + wf * 0.045;
        let base_y = (hf - 1.0).min(self.tip_y + hf * 0.16);
        line_f(
            canvas,
            base_x,
            base_y,
            self.tip_x + (base_x - self.tip_x) * 0.22,
            self.tip_y + (base_y - self.tip_y) * 0.22,
            self.stick_c,
            0.9,
        );
        line_f(
            canvas,
            self.tip_x + (base_x - self.tip_x) * 0.22,
            self.tip_y + (base_y - self.tip_y) * 0.22,
            self.tip_x,
            self.tip_y,
            (150, 145, 140),
            0.55,
        );

        // ribbon: join consecutive tracers into a polyline. Brightness
        // eases in just above the ember, dissipates with age, and the
        // color cools from ember-warmed gray to pale as it climbs.
        for pair in self.puffs.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if b.id != a.id + 1 {
                continue; // capped emission broke the thread here
            }
            let fade_a = ease_smooth(a.age / 0.4) * ease_smooth((a.life - a.age) / 1.6);
            let fade_b = ease_smooth(b.age / 0.4) * ease_smooth((b.life - b.age) / 1.6);
            let fade = fade_a.min(fade_b);
            if fade <= 0.01 {
                continue;
            }
            let alt = ((tip_y - a.b.y).max(0.0) / hf.max(1.0)).min(1.0);
            let c = lerp(
                lerp(self.smoke_lo, self.ember_c, (0.35 - alt).max(0.0)),
                self.smoke_hi,
                alt * 0.8,
            );
            let alpha = fade * (0.30 + self.flare * 0.10);
            line_f(canvas, a.b.x, a.b.y, b.b.x, b.b.y, c, alpha);
            // a faint wider breath around the mid ribbon reads as volume
            if a.age > 1.2 && fade > 0.25 {
                dot(canvas, a.b.x + 0.7, a.b.y, scale(c, 0.5), alpha * 0.35);
                dot(canvas, a.b.x - 0.7, a.b.y, scale(c, 0.5), alpha * 0.35);
            }
        }

        // ash flecks
        for a in &self.ash {
            let k = (a.life / 3.0).clamp(0.0, 1.0);
            dot(canvas, a.b.x, a.b.y, lerp((90, 88, 86), self.ember_c, k * 0.4), 0.7 * k + 0.1);
        }

        // the ember itself: hot core over a soft corona, all fractional so
        // the breathing swells smoothly
        glow_f(
            canvas,
            self.tip_x,
            self.tip_y,
            1.6 + env * 0.9,
            self.ember_c,
            0.5 + env * 0.4,
        );
        dot(
            canvas,
            self.tip_x,
            self.tip_y,
            lerp(self.ember_c, self.ember_hot, (env - 0.3).clamp(0.0, 1.0)),
            (0.6 + env * 0.5).min(1.0),
        );
    }
}
