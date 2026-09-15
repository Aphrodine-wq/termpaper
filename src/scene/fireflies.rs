//! Fireflies: amber motes breathing over a black meadow. Three parallax
//! depth layers (far = tiny/dim/slow, near = 2px clusters/bright/faster),
//! eased glow envelopes, spring-damper wandering with occasional darts,
//! neighbor glow bleed, low flies washing the grass with warm light, and a
//! swarm gather event every ~18-28s: a faint beacon ember breathes at the
//! chosen spot (anticipation), then converge -> mill about -> scatter.

use super::noise::{hash2, vnoise};
use super::{Detail, Scene};
use crate::canvas::{ease_smooth, glow, lerp, scale, Canvas};
use crate::physics::spring_damper;
use rand::{rngs::StdRng, RngExt};

/// Depth layer tuning: (brightness, max speed, spring stiffness, damping).
const LAYERS: [(f32, f32, f32, f32); 3] = [
    (0.35, 5.0, 0.7, 1.4),  // far
    (0.65, 9.0, 1.1, 1.8),  // mid
    (1.0, 14.0, 1.6, 2.2),  // near
];

const GATHER_TIME: f32 = 3.0;
const MILL_TIME: f32 = 2.6;
const SCATTER_TIME: f32 = 1.2;
/// Seconds before a gather when the beacon ember starts breathing.
const BEACON_TIME: f32 = 2.5;

/// Swarm-gather rhythm: Idle -> Converge -> Mill -> Scatter -> Idle.
#[derive(Clone, Copy, PartialEq)]
enum Swarm {
    Idle,
    Converge(f32), // elapsed
    Mill(f32),
    Scatter(f32),
}

struct Firefly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    tx: f32, // wander target
    ty: f32,
    retarget: f32,
    layer: usize,
    phase: f32, // blink envelope clock
    off: f32,   // envelope segment durations
    up: f32,
    hold: f32,
    down: f32,
    seed: f32, // per-fly variation
    ball_ang: f32, // swarm ball offset
    ball_rad: f32,
    ball_rot: f32,
}

impl Firefly {
    /// Eased glow envelope 0..1: off -> ramp up -> hold (shimmer) -> decay.
    fn envelope(&self) -> f32 {
        let t = self.phase;
        if t < self.off {
            0.0
        } else if t < self.off + self.up {
            ease_smooth((t - self.off) / self.up)
        } else if t < self.off + self.up + self.hold {
            0.85 + 0.15 * (t * 9.0 + self.seed * 20.0).sin()
        } else {
            let k = (t - self.off - self.up - self.hold) / self.down;
            1.0 - ease_smooth(k)
        }
    }

    fn cycle(&self) -> f32 {
        self.off + self.up + self.hold + self.down
    }
}

fn spawn(rng: &mut StdRng, w: usize, h: usize) -> Firefly {
    let r = rng.random::<f32>();
    let layer = if r < 0.5 {
        0 // far
    } else if r < 0.85 {
        1 // mid
    } else {
        2 // near
    };
    let (off, up, hold, down) = if layer == 0 {
        (
            rng.random_range(1.5..4.5),
            rng.random_range(0.6..1.3),
            rng.random_range(0.3..1.0),
            rng.random_range(0.9..1.7),
        )
    } else {
        (
            rng.random_range(0.8..2.8),
            rng.random_range(0.3..0.8),
            rng.random_range(0.4..1.2),
            rng.random_range(0.5..1.1),
        )
    };
    let x = rng.random_range(0.0..w as f32);
    let y = rng.random_range(0.0..h as f32);
    Firefly {
        x,
        y,
        vx: 0.0,
        vy: 0.0,
        tx: x,
        ty: y,
        retarget: rng.random_range(1.0..4.0),
        layer,
        phase: rng.random_range(0.0..off + up + hold + down),
        off,
        up,
        hold,
        down,
        seed: rng.random::<f32>(),
        ball_ang: 0.0,
        ball_rad: 0.0,
        ball_rot: 1.0,
    }
}

pub struct Fireflies {
    rng: StdRng,
    detail: Detail,
    glow_c: (u8, u8, u8),
    core_c: (u8, u8, u8),
    grass_c: (u8, u8, u8),
    flies: Vec<Firefly>,
    bright: Vec<f32>, // scratch: per-fly envelope for the bleed pass
    swarm: Swarm,
    next_gather: f32,
    gx: f32,
    gy: f32,
    beacon: bool, // gather spot chosen, ember breathing (anticipation)
    t: f32,
    w: usize,
    h: usize,
}

impl Fireflies {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (glow_c, core_c, grass_c) = match theme {
            Some("emerald") => ((80, 230, 140), (195, 255, 215), (8, 24, 12)),
            Some("ice") => ((140, 210, 255), (228, 246, 255), (8, 18, 22)),
            _ => ((255, 176, 66), (255, 228, 150), (9, 22, 10)), // amber
        };
        Fireflies {
            rng,
            detail,
            glow_c,
            core_c,
            grass_c,
            flies: Vec::new(),
            bright: Vec::new(),
            swarm: Swarm::Idle,
            next_gather: 14.0,
            gx: 0.0,
            gy: 0.0,
            beacon: false,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// Swarm glow envelope: ease in while converging, hold in the ball,
    /// decay as they scatter.
    fn swarm_glow(&self) -> f32 {
        match self.swarm {
            Swarm::Idle => 0.0,
            Swarm::Converge(e) => ease_smooth(e / GATHER_TIME),
            Swarm::Mill(_) => 0.9 + 0.1 * (self.t * 6.0).sin(),
            Swarm::Scatter(e) => 1.0 - ease_smooth(e / SCATTER_TIME),
        }
    }
}

impl Scene for Fireflies {
    fn name(&self) -> &'static str {
        "fireflies"
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
            // Linear in area, not `(w*h/K) * density_for(w, h)`: density_for()
            // already carries a w*h term, so that form was quadratic and landed
            // under the count floor at every normal terminal size — which meant
            // the floor *was* the count and --detail did nothing here. Medium is
            // tuned to land near the old effective value (~28 at 160x100).
            let n = self.detail.scale((w * h) as f32 / 560.0, 8).min(320);
            self.flies = (0..n).map(|_| spawn(&mut self.rng, w, h)).collect();
            self.bright = vec![0.0; n];
            self.swarm = Swarm::Idle;
            self.beacon = false;
        }
        self.t += dt;

        // --- swarm gather scheduling (anticipation -> payoff -> decay) ---
        self.next_gather -= dt;
        let ball_r = (w.min(h) as f32 * 0.18).clamp(3.0, 12.0);
        match self.swarm {
            Swarm::Idle => {
                // anticipation: pick the spot early so a faint beacon ember
                // can breathe there before the swarm commits
                if self.next_gather <= BEACON_TIME && !self.beacon {
                    self.gx = self.rng.random_range(w as f32 * 0.25..w as f32 * 0.75);
                    self.gy = self.rng.random_range(h as f32 * 0.2..h as f32 * 0.65);
                    self.beacon = true;
                }
                if self.next_gather <= 0.0 {
                    for f in &mut self.flies {
                        f.ball_ang = self.rng.random_range(0.0..std::f32::consts::TAU);
                        f.ball_rad = self.rng.random_range(0.5..ball_r);
                        f.ball_rot = self.rng.random_range(0.8..1.6)
                            * if self.rng.random::<bool>() { 1.0 } else { -1.0 };
                    }
                    self.swarm = Swarm::Converge(0.0);
                }
            }
            Swarm::Converge(e) => {
                let e = e + dt;
                self.swarm = if e >= GATHER_TIME {
                    Swarm::Mill(0.0)
                } else {
                    Swarm::Converge(e)
                };
            }
            Swarm::Mill(e) => {
                let e = e + dt;
                if e >= MILL_TIME {
                    // scatter payoff: outward impulse from the ball center
                    for f in &mut self.flies {
                        let (dx, dy) = (f.x - self.gx, f.y - self.gy);
                        let d = (dx * dx + dy * dy).sqrt().max(1.0);
                        let kick = LAYERS[f.layer].1 * 1.6;
                        f.vx += dx / d * kick;
                        f.vy += dy / d * kick;
                        f.retarget = 0.0; // pick a fresh wander target now
                    }
                    self.swarm = Swarm::Scatter(0.0);
                } else {
                    self.swarm = Swarm::Mill(e);
                }
            }
            Swarm::Scatter(e) => {
                let e = e + dt;
                if e >= SCATTER_TIME {
                    self.swarm = Swarm::Idle;
                    self.beacon = false;
                    self.next_gather = self.rng.random_range(18.0..28.0);
                } else {
                    self.swarm = Swarm::Scatter(e);
                }
            }
        }
        let sw = self.swarm_glow();
        let gathering = !matches!(self.swarm, Swarm::Idle);

        // --- steering + integration ---
        for f in &mut self.flies {
            f.retarget -= dt;
            if f.retarget <= 0.0 {
                f.tx = self.rng.random_range(0.0..w as f32);
                f.ty = self.rng.random_range(0.0..h as f32);
                f.retarget = self.rng.random_range(2.0..5.0);
            }
            let (_, max_v, stiff, damp) = LAYERS[f.layer];
            // spring-damper steering toward the (swarm or wander) target
            let (tx, ty, k) = if gathering {
                let ang = f.ball_ang
                    + if matches!(self.swarm, Swarm::Mill(_)) {
                        self.t * f.ball_rot
                    } else {
                        0.0
                    };
                (
                    self.gx + ang.cos() * f.ball_rad,
                    self.gy + ang.sin() * f.ball_rad * 0.6,
                    stiff * 4.0,
                )
            } else {
                (f.tx, f.ty, stiff)
            };
            let ax = spring_damper(f.x, f.vx, tx, k, damp);
            let ay = spring_damper(f.y, f.vy, ty, k, damp);
            f.vx += ax * dt;
            f.vy += ay * dt;
            // occasional dart: a sudden impulse in a random direction
            if self.rng.random::<f32>() < 0.12 * dt {
                let a = self.rng.random_range(0.0..std::f32::consts::TAU);
                let j = max_v * self.rng.random_range(0.8..1.6);
                f.vx += a.cos() * j;
                f.vy += a.sin() * j;
            }
            // speed limit + gentle bob
            let s = (f.vx * f.vx + f.vy * f.vy).sqrt();
            let cap = max_v * if gathering { 2.2 } else { 1.0 };
            if s > cap {
                f.vx *= cap / s;
                f.vy *= cap / s;
            }
            f.x = (f.x + f.vx * dt).rem_euclid(w as f32);
            f.y = (f.y + f.vy * dt + (self.t * 1.3 + f.seed * 9.0).sin() * 0.6 * dt)
                .rem_euclid(h as f32);
            // blink envelope clock
            f.phase += dt;
            if f.phase >= f.cycle() {
                f.phase -= f.cycle();
            }
        }

        // --- lighting interplay: envelope pass, then neighbor bleed ---
        for (i, f) in self.flies.iter().enumerate() {
            self.bright[i] = f.envelope() * LAYERS[f.layer].0;
        }
        let n = self.flies.len();
        for i in 0..n {
            let mut bleed = 0.0f32;
            for j in 0..n {
                if i == j || self.bright[j] < 0.05 {
                    continue;
                }
                let dx = self.flies[j].x - self.flies[i].x;
                let dy = self.flies[j].y - self.flies[i].y;
                let d2 = dx * dx + dy * dy;
                if d2 < 36.0 {
                    bleed += self.bright[j] * (1.0 - d2 / 36.0) * 0.18;
                }
            }
            self.bright[i] = (self.bright[i] + bleed).min(1.3);
        }

        // --- draw ---
        canvas.clear((0, 0, 0));

        // sparse near-black grass along the bottom rows, swaying faintly
        if h >= 4 {
            for x in 0..w {
                let r = hash2(x as i32, 0, 11);
                let sway = vnoise(x as f32 * 0.3, self.t * 0.5, 5);
                if r > 0.55 {
                    // near blade: 1-2 cells tall, leans with the breeze
                    let tall = r > 0.85;
                    let lean = ((sway - 0.5) * 1.6).round() as i32;
                    canvas.set(
                        x as i32,
                        h as i32 - 1,
                        scale(self.grass_c, 0.5 + sway * 0.25),
                    );
                    if tall {
                        canvas.set(
                            x as i32 + lean,
                            h as i32 - 2,
                            scale(self.grass_c, 0.32 + sway * 0.18),
                        );
                    }
                } else if r > 0.32 {
                    // far depth cue: dimmer tufts one row higher
                    canvas.set(x as i32, h as i32 - 3, scale(self.grass_c, 0.28));
                }
            }
        }

        // beacon ember: a faint warm breath where the swarm will gather —
        // the visible anticipation half of the event rhythm
        if self.beacon && matches!(self.swarm, Swarm::Idle) {
            let k = 1.0 - (self.next_gather / BEACON_TIME).clamp(0.0, 1.0);
            let breathe = 0.7 + 0.3 * (self.t * 4.5).sin();
            glow(canvas, self.gx as i32, self.gy as i32, 2, self.glow_c, 0.09 * k * breathe);
        }

        // swarm heart: a soft shared glow while the ball is lit
        if gathering && sw > 0.02 {
            let r = (ball_r * 0.7).clamp(2.0, 8.0) as i32;
            glow(canvas, self.gx as i32, self.gy as i32, r, self.glow_c, sw * 0.16);
        }

        for (i, f) in self.flies.iter().enumerate() {
            let mut b = self.bright[i];
            if gathering {
                b = b.max(sw * 0.85 * LAYERS[f.layer].0);
            }
            if b < 0.02 {
                continue;
            }
            let b = b.min(1.0);
            let core = lerp(self.glow_c, self.core_c, b * 0.55);
            let c = scale(core, b);
            let (ix, iy) = (f.x as i32, f.y as i32);
            canvas.set(ix, iy, c);
            // ground light: a low, lit fly washes the grass beneath it
            let ground_d = h as f32 - f.y;
            if f.layer > 0 && b > 0.35 && ground_d < 8.0 {
                let k = (1.0 - ground_d / 8.0) * b;
                glow(canvas, ix, h as i32 - 1, 2, self.glow_c, k * 0.12);
            }
            if f.layer == 2 {
                // near layer: 2px cluster trailing the drift direction
                canvas.set_f(f.x - f.vx * 0.07, f.y - f.vy * 0.07, scale(c, 0.55));
                if b > 0.45 {
                    glow(canvas, ix, iy, 2, self.glow_c, b * 0.35);
                }
            } else if f.layer == 1 && b > 0.7 {
                glow(canvas, ix, iy, 1, self.glow_c, b * 0.3);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn deterministic_from_seed() {
        let run = || {
            let mut s = Fireflies::new(StdRng::seed_from_u64(123), None, Detail::Medium);
            let mut c = Canvas::new(80, 40);
            for _ in 0..150 {
                s.update(1.0 / 60.0, &mut c);
            }
            let mut sum = 0u64;
            for y in 0..40 {
                for x in 0..80 {
                    let (r, g, b) = c.get(x, y).color;
                    sum = sum
                        .wrapping_mul(31)
                        .wrapping_add(((r as u64) << 16) ^ ((g as u64) << 8) ^ b as u64);
                }
            }
            sum
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn resize_reinits_without_panic() {
        let mut s = Fireflies::new(StdRng::seed_from_u64(7), Some("ice"), Detail::High);
        let mut c = Canvas::new(120, 60);
        for _ in 0..30 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(40, 12);
        for _ in 0..30 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(0, 0);
        s.update(1.0 / 30.0, &mut c); // degenerate canvas: early return
        // large dt steps must not explode
        c.resize(60, 20);
        for _ in 0..10 {
            s.update(5.0, &mut c);
        }
    }

    #[test]
    fn themes_resolve_and_background_is_black() {
        for theme in [None, Some("amber"), Some("emerald"), Some("ice"), Some("bogus")] {
            let mut s = Fireflies::new(StdRng::seed_from_u64(3), theme, Detail::Medium);
            assert_eq!(s.name(), "fireflies");
            let mut c = Canvas::new(60, 24);
            s.update(1.0 / 60.0, &mut c);
            // first frame: background stays near black (a lit firefly is an
            // accent, not the background) — almost all cells are dark
            let mut dark = 0usize;
            for y in 0..24 {
                for x in 0..60 {
                    let (r, g, b) = c.get(x, y).color;
                    if r as u32 + g as u32 + b as u32 <= 60 {
                        dark += 1;
                    }
                }
            }
            assert!(
                dark as f32 / (60 * 24) as f32 > 0.9,
                "first frame should be mostly dark"
            );
        }
    }

    #[test]
    fn fireflies_light_up_over_time() {
        let mut s = Fireflies::new(StdRng::seed_from_u64(42), None, Detail::Medium);
        let mut c = Canvas::new(100, 50);
        let mut lit_peak = 0usize;
        for _ in 0..240 {
            s.update(1.0 / 30.0, &mut c);
            let lit = (0..100)
                .flat_map(|x| (0..50).map(move |y| (x, y)))
                .filter(|&(x, y)| {
                    let (r, g, b) = c.get(x, y).color;
                    r as u32 + g as u32 + b as u32 > 90
                })
                .count();
            lit_peak = lit_peak.max(lit);
        }
        assert!(lit_peak > 10, "fireflies should glow, peak lit {lit_peak}");
    }
}
