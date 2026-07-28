//! Harmonograph: glowing spiro curves drawing themselves, then fading.
//! Two superimposed damped pendulums per axis trace a luminous Lissajous
//! figure point-by-point — a bright drawing head with a long smoothly-fading
//! trail, its fresh path lit by a small comet glow. Faint dust motes hang
//! behind as a far background plane. The finished figure dissolves into a
//! dim ghost layer behind the next one while the frequency/phase set
//! ease-morphs to new values. Every ~20-30s a RESONANCE event snaps the
//! frequencies to a low-integer ratio: the curve locks into a crisp
//! symmetric mandala with a brightness bloom
//! (anticipation charge -> payoff bloom -> decay).

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, hsv, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::{FRAC_PI_2, TAU};

const SPEED: f32 = 2.4; // rad/s advance of the drawing head
const MORPH_SLOW: f32 = 2.6; // s, eased glide to a new random figure
const MORPH_FAST: f32 = 1.1; // s, resonance snap
const DISSOLVE_TIME: f32 = 2.2; // s, finished figure fades out
const GHOST_FADE: f32 = 5.5; // s, ghost layer dissolves
const BLOOM_ATTACK: f32 = 0.9;
const BLOOM_HOLD: f32 = 2.6;
const BLOOM_DECAY: f32 = 2.8;
const BLOOM_TOTAL: f32 = BLOOM_ATTACK + BLOOM_HOLD + BLOOM_DECAY;
const HUE_SPAN: f32 = 0.62; // prism: hue drift from tail to head

/// Low-integer frequency pairs for the resonance mandala.
const RATIOS: [(f32, f32); 6] = [(1.0, 2.0), (2.0, 3.0), (3.0, 4.0), (1.0, 3.0), (3.0, 5.0), (2.0, 5.0)];

#[derive(Clone, Copy, PartialEq)]
enum Theme {
    Prism,
    Gold,
    Ice,
}

/// One pendulum parameter set: x = a0*sin(f0*t+p0)*e^-d0*t + a1*sin(...),
/// y likewise with slots 2 and 3. Amplitudes sum to 1 per axis, so the
/// figure stays inside the unit square.
#[derive(Clone, Copy)]
struct Params {
    f: [f32; 4],
    p: [f32; 4],
    d: [f32; 4],
    a: [f32; 4],
}

impl Params {
    fn lerp(from: &Params, to: &Params, t: f32) -> Params {
        let mut out = *from;
        for i in 0..4 {
            out.f[i] = from.f[i] + (to.f[i] - from.f[i]) * t;
            out.p[i] = from.p[i] + (to.p[i] - from.p[i]) * t;
            out.d[i] = from.d[i] + (to.d[i] - from.d[i]) * t;
            out.a[i] = from.a[i] + (to.a[i] - from.a[i]) * t;
        }
        out
    }

    fn eval(&self, tau: f32) -> (f32, f32) {
        let x = self.a[0] * (self.f[0] * tau + self.p[0]).sin() * (-self.d[0] * tau).exp()
            + self.a[1] * (self.f[1] * tau + self.p[1]).sin() * (-self.d[1] * tau).exp();
        let y = self.a[2] * (self.f[2] * tau + self.p[2]).sin() * (-self.d[2] * tau).exp()
            + self.a[3] * (self.f[3] * tau + self.p[3]).sin() * (-self.d[3] * tau).exp();
        (x, y)
    }

    fn mean_damping(&self) -> f32 {
        (self.d[0] + self.d[1] + self.d[2] + self.d[3]) * 0.25
    }
}

fn random_params(rng: &mut StdRng) -> Params {
    let a0 = rng.random_range(0.55..0.85);
    let a2 = rng.random_range(0.55..0.85);
    Params {
        f: [
            rng.random_range(1.0..4.5),
            rng.random_range(1.0..4.5),
            rng.random_range(1.0..4.5),
            rng.random_range(1.0..4.5),
        ],
        p: [
            rng.random_range(0.0..TAU),
            rng.random_range(0.0..TAU),
            rng.random_range(0.0..TAU),
            rng.random_range(0.0..TAU),
        ],
        d: [
            rng.random_range(0.10..0.18),
            rng.random_range(0.10..0.18),
            rng.random_range(0.10..0.18),
            rng.random_range(0.10..0.18),
        ],
        a: [a0, 1.0 - a0, a2, 1.0 - a2],
    }
}

/// Resonance set: near-integer ratios with axis-aligned phases and light
/// damping, so the curve locks into a slowly precessing symmetric mandala.
fn resonant_params(rng: &mut StdRng) -> Params {
    let (n1, n2) = RATIOS[rng.random_range(0..RATIOS.len())];
    let (m1, m2) = RATIOS[rng.random_range(0..RATIOS.len())];
    let det = rng.random_range(0.002..0.008) * if rng.random::<bool>() { 1.0 } else { -1.0 };
    let a0 = rng.random_range(0.6..0.8);
    let a2 = rng.random_range(0.6..0.8);
    Params {
        f: [n1 + det, n2 - det, m1 - det, m2 + det],
        p: [
            (rng.random_range(0..4) as f32) * FRAC_PI_2,
            (rng.random_range(0..4) as f32) * FRAC_PI_2,
            (rng.random_range(0..4) as f32) * FRAC_PI_2,
            (rng.random_range(0..4) as f32) * FRAC_PI_2,
        ],
        d: [
            rng.random_range(0.055..0.085),
            rng.random_range(0.055..0.085),
            rng.random_range(0.055..0.085),
            rng.random_range(0.055..0.085),
        ],
        a: [a0, 1.0 - a0, a2, 1.0 - a2],
    }
}

/// A plotted point of the current figure; `frac` is its position along the
/// whole trace (0 = tail, 1 = head) so brightness and hue can gradient.
#[derive(Clone, Copy)]
struct Pt {
    x: f32,
    y: f32,
    frac: f32,
}

/// A far-background dust mote: fixed anchor, slow drift, gentle twinkle.
#[derive(Clone, Copy)]
struct Dust {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Draw,
    Dissolve(f32), // elapsed
}

pub struct Harmonograph {
    rng: StdRng,
    detail: Detail,
    theme: Theme,
    cur: Params,
    from: Params,
    tgt: Params,
    morph: f32, // 0..1 progress from -> tgt
    morph_time: f32,
    tau: f32,
    tau_end: f32,
    emit: f32, // emission accumulator for fixed-spacing points
    spacing: f32,
    max_pts: usize,
    points: Vec<Pt>,
    base_hue: f32,
    ghost: Vec<Pt>,
    ghost_hue: f32,
    ghost_fade: f32, // 1 -> 0
    dust: Vec<Dust>,
    stage: Stage,
    next_resonance: f32,
    bloom_e: f32, // <0 = inactive
    t: f32,
    w: usize,
    h: usize,
}

impl Harmonograph {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let theme = match theme {
            Some("gold") => Theme::Gold,
            Some("ice") => Theme::Ice,
            _ => Theme::Prism,
        };
        let mut rng = rng;
        let p = random_params(&mut rng);
        Harmonograph {
            base_hue: rng.random::<f32>(),
            next_resonance: rng.random_range(12.0..20.0),
            rng,
            detail,
            theme,
            cur: p,
            from: p,
            tgt: p,
            morph: 1.0,
            morph_time: MORPH_SLOW,
            tau: 0.0,
            tau_end: 12.0,
            emit: 0.0,
            spacing: 0.01,
            max_pts: 1200,
            points: Vec::new(),
            ghost: Vec::new(),
            ghost_hue: 0.0,
            ghost_fade: 0.0,
            dust: Vec::new(),
            stage: Stage::Draw,
            bloom_e: -1.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// Begin a new figure: glide current params toward `tgt` over
    /// `morph_time` seconds and rewind the trace clock.
    fn start_figure(&mut self, tgt: Params, morph_time: f32) {
        self.from = self.cur;
        self.tgt = tgt;
        self.morph = 0.0;
        self.morph_time = morph_time;
        self.tau = 0.0;
        self.emit = 0.0;
        self.tau_end = 2.3 / tgt.mean_damping();
        self.spacing = self.tau_end / (self.max_pts as f32 * 0.95);
        self.base_hue = self.rng.random::<f32>();
        self.stage = Stage::Draw;
    }

    /// Retire the current figure into the dim ghost layer behind.
    fn ghost_current(&mut self) {
        self.ghost = std::mem::take(&mut self.points);
        self.ghost_hue = self.base_hue;
        self.ghost_fade = 1.0;
    }

    fn bloom(&self) -> f32 {
        if self.bloom_e < 0.0 {
            return 0.0;
        }
        let e = self.bloom_e;
        if e < BLOOM_ATTACK {
            ease_smooth(e / BLOOM_ATTACK)
        } else if e < BLOOM_ATTACK + BLOOM_HOLD {
            1.0
        } else {
            1.0 - ease_smooth((e - BLOOM_ATTACK - BLOOM_HOLD) / BLOOM_DECAY)
        }
    }

    fn color(&self, base_hue: f32, frac: f32, b: f32) -> (u8, u8, u8) {
        let c = match self.theme {
            Theme::Gold => hsv(0.095 + 0.05 * frac, 0.82, 1.0),
            Theme::Ice => hsv(0.55 + 0.05 * frac, 0.55, 1.0),
            Theme::Prism => hsv(base_hue + frac * HUE_SPAN + self.t * 0.02, 0.72, 1.0),
        };
        scale(c, b)
    }
}

impl Scene for Harmonograph {
    fn name(&self) -> &'static str {
        "harmonograph"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);

        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            // trace length scales with area and detail; capped for perf
            self.max_pts = self
                .detail
                .scale(1500.0 * density_for(w, h), 500)
                .min(3600);
            self.spacing = self.tau_end / (self.max_pts as f32 * 0.95);
            // far background: sparse dust motes, dim enough to stay off the accents
            let nd = self
                .detail
                .scale((w * h / 350) as f32 * density_for(w, h), 6)
                .min(180);
            self.dust = (0..nd)
                .map(|_| Dust {
                    x: self.rng.random_range(0.0..w as f32),
                    y: self.rng.random_range(0.0..h as f32),
                    mag: self.rng.random_range(0.04..0.12),
                    phase: self.rng.random_range(0.0..TAU),
                })
                .collect();
        }
        self.t += dt;

        // --- resonance scheduling (charge -> snap -> bloom -> decay) ---
        self.next_resonance -= dt;
        let charge = if self.next_resonance > 0.0 && self.next_resonance < 1.6 {
            ease_smooth((1.6 - self.next_resonance) / 1.6) // anticipation
        } else {
            0.0
        };
        if self.next_resonance <= 0.0 {
            self.ghost_current();
            let p = resonant_params(&mut self.rng);
            self.start_figure(p, MORPH_FAST);
            self.bloom_e = 0.0;
            self.next_resonance = self.rng.random_range(20.0..30.0);
        }
        if self.bloom_e >= 0.0 {
            self.bloom_e += dt;
            if self.bloom_e > BLOOM_TOTAL {
                self.bloom_e = -1.0;
            }
        }
        let bloom = self.bloom();

        // --- eased parameter morph ---
        if self.morph < 1.0 {
            self.morph = (self.morph + dt / self.morph_time).min(1.0);
        }
        self.cur = Params::lerp(&self.from, &self.tgt, ease_smooth(self.morph));

        // --- figure lifecycle ---
        match self.stage {
            Stage::Draw => {
                self.emit += SPEED * dt;
                while self.emit >= self.spacing {
                    self.emit -= self.spacing;
                    self.tau += self.spacing;
                    let (x, y) = self.cur.eval(self.tau);
                    let frac = (self.tau / self.tau_end).min(1.0);
                    self.points.push(Pt { x, y, frac });
                }
                if self.tau >= self.tau_end || self.points.len() >= self.max_pts {
                    self.stage = Stage::Dissolve(0.0);
                }
            }
            Stage::Dissolve(e) => {
                let e = e + dt;
                if e >= DISSOLVE_TIME {
                    self.ghost_current();
                    let p = random_params(&mut self.rng);
                    self.start_figure(p, MORPH_SLOW);
                } else {
                    self.stage = Stage::Dissolve(e);
                }
            }
        }
        self.ghost_fade = (self.ghost_fade - dt / GHOST_FADE).max(0.0);

        // --- draw ---
        canvas.clear((0, 0, 0));
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let ry = h as f32 * 0.44;
        // cells are ~twice as tall as wide: keep it round, but claim spare
        // horizontal room on wide canvases instead of leaving dead margins
        let rx = (w as f32 * 0.46).min(ry * 2.0);
        let breathe = 0.9 + 0.1 * (self.t * 0.6).sin();

        // depth layer 0: dust motes hanging far behind the figure
        for m in &self.dust {
            let tw = 0.7 + 0.3 * (self.t * 0.5 + m.phase).sin();
            let dx = m.x + (self.t * 0.05 + m.phase).sin() * 1.2;
            let dy = m.y + (self.t * 0.04 + m.phase * 1.7).cos() * 0.7;
            let c = self.color(self.base_hue, m.phase / TAU, m.mag * tw * (1.0 + bloom * 0.4));
            canvas.set(dx as i32, dy as i32, c);
        }

        // soft center ember, flaring with the resonance bloom
        let ember_r = ((h as f32 * 0.18) as i32).clamp(2, 8);
        let ember = self.color(self.base_hue, 0.5, 1.0);
        glow(canvas, cx as i32, cy as i32, ember_r, ember, 0.04 + bloom * 0.10);

        // depth layer 1: ghost of the previous figure dissolving behind
        if self.ghost_fade > 0.0 && !self.ghost.is_empty() {
            let n = self.ghost.len();
            for (i, pt) in self.ghost.iter().enumerate() {
                let frac = (i + 1) as f32 / n as f32;
                let b = 0.24 * self.ghost_fade * (0.25 + 0.75 * frac) * (1.0 + bloom * 0.5);
                let c = self.color(self.ghost_hue, pt.frac, b);
                canvas.add(
                    (cx + pt.x * rx) as i32,
                    (cy + pt.y * ry) as i32,
                    c,
                );
            }
        }

        // depth layer 2: current figure, readable weave with a bright head —
        // the outer loops (drawn first) keep enough floor to show the pattern
        let fade = match self.stage {
            Stage::Draw => 1.0,
            Stage::Dissolve(e) => 1.0 - ease_smooth(e / DISSOLVE_TIME),
        };
        let n = self.points.len();
        for (i, pt) in self.points.iter().enumerate() {
            let frac = (i + 1) as f32 / n.max(1) as f32;
            let b = (0.30 + 0.70 * frac.powf(1.5)) * fade * breathe * (1.0 + bloom * 0.8);
            let c = self.color(self.base_hue, pt.frac, b);
            canvas.add(
                (cx + pt.x * rx) as i32,
                (cy + pt.y * ry) as i32,
                c,
            );
        }

        // the drawing head: hot core, a bloom-reactive glow, and a short
        // comet glow bleeding back along the freshest trail points
        if matches!(self.stage, Stage::Draw) && !self.points.is_empty() {
            let head = self.points[n - 1];
            let (hx, hy) = ((cx + head.x * rx) as i32, (cy + head.y * ry) as i32);
            let pulse = 1.0 + 0.5 * charge * (self.t * 10.0).sin().abs();
            let base = self.color(self.base_hue, head.frac, 1.0);
            let core = lerp(base, (255, 255, 255), 0.65);
            canvas.add(hx, hy, core);
            let r = (2.0 + bloom * 2.5 + charge * 1.0) as i32;
            glow(canvas, hx, hy, r.max(2), base, (0.5 + 0.5 * bloom) * pulse * fade);
            for k in 1..=3 {
                let idx = n as i32 - 1 - k * 4;
                if idx < 0 {
                    break;
                }
                let pt = self.points[idx as usize];
                let trail_c = self.color(self.base_hue, pt.frac, 1.0);
                let inten = 0.14 * (4 - k) as f32 / 3.0 * (1.0 + bloom) * fade;
                glow(
                    canvas,
                    (cx + pt.x * rx) as i32,
                    (cy + pt.y * ry) as i32,
                    if bloom > 0.3 { 2 } else { 1 },
                    trail_c,
                    inten,
                );
            }
        }
    }
}
