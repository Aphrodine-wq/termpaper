//! Sonar: phosphor radar sweep lighting up drifting contacts. A scope on
//! pure black: sparse receiver-noise speckle as the background plane, very
//! dim range rings and cross ticks, a rotating sweep arm with an afterglow
//! wedge decaying behind it, contacts that only glow while the phosphor
//! remembers the sweep, an occasional fast mover with a short trail, and
//! every ~15-25s a PING — a charge-up hum at the center, then an expanding
//! pulse that flares every contact it crosses and echoes a faint ring off
//! each one.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_out, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::TAU;

/// Normalized ring radii (fraction of the scope radius).
const RINGS: [f32; 4] = [0.25, 0.5, 0.75, 1.0];
/// Angular length of the phosphor afterglow behind the arm (radians).
const WEDGE_ARC: f32 = 2.4;
/// Ping pulse lifetime in seconds.
const PING_DUR: f32 = 2.4;
/// Cap on live echo rings so flare bursts stay bounded.
const MAX_ECHOES: usize = 96;
/// Charge-up hum at the center before a ping releases (seconds).
const PING_CHARGE: f32 = 0.7;

struct Contact {
    x: f32,
    y: f32,
    px: f32, // previous position (trail memory)
    py: f32,
    heading: f32,
    speed: f32,
    seed: f32,
    lit: f32,    // phosphor persistence 0..1, set by the passing sweep
    flare: f32,  // ping flare, decays fast
    pinged: bool,
    fast_t: f32, // >0 while darting (fast mover)
}

/// A faint ring echoing off a contact the ping just crossed.
struct Echo {
    x: f32,
    y: f32,
    age: f32,
    strength: f32,
}

/// A receiver-noise speckle: the dim static floor behind the scope.
struct NoiseDot {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
}

pub struct Sonar {
    rng: StdRng,
    detail: Detail,
    base: (u8, u8, u8), // phosphor color
    hot: (u8, u8, u8),  // near-white core for flares
    contacts: Vec<Contact>,
    echoes: Vec<Echo>,
    echo_scratch: Vec<Echo>, // reused per-frame spawn buffer (no hot-path alloc)
    noise: Vec<NoiseDot>,
    sweep: f32, // arm angle, radians
    next_ping: f32,
    ping_age: f32, // <0 inactive
    next_fast: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Sonar {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (base, hot) = match theme {
            Some("amber") => ((255, 186, 66), (255, 238, 196)),
            Some("cyan") => ((86, 224, 255), (218, 248, 255)),
            _ => ((88, 255, 140), (216, 255, 226)), // green: classic phosphor
        };
        Sonar {
            rng,
            detail,
            base,
            hot,
            contacts: Vec::new(),
            echoes: Vec::new(),
            echo_scratch: Vec::new(),
            noise: Vec::new(),
            sweep: 0.0,
            next_ping: 7.0,
            ping_age: -1.0,
            next_fast: 5.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.contacts.clear();
        self.echoes.clear();
        self.noise.clear();
        if w == 0 || h == 0 {
            return;
        }
        let n = self
            .detail
            .scale((w * h / 900) as f32 * density_for(w, h), 10)
            .min(160);
        self.contacts = (0..n)
            .map(|_| {
                let x = self.rng.random_range(0.0..w as f32);
                let y = self.rng.random_range(0.0..h as f32);
                Contact {
                    x,
                    y,
                    px: x,
                    py: y,
                    heading: self.rng.random_range(0.0..TAU),
                    speed: self.rng.random_range(0.6..2.0),
                    seed: self.rng.random::<f32>(),
                    lit: 0.0,
                    flare: 0.0,
                    pinged: false,
                    fast_t: 0.0,
                }
            })
            .collect();
        // sparse receiver noise: covers the void outside the scope too
        let nn = self
            .detail
            .scale((w * h / 300) as f32 * density_for(w, h), 12)
            .min(320);
        self.noise = (0..nn)
            .map(|_| NoiseDot {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                mag: self.rng.random_range(0.045..0.12),
                phase: self.rng.random_range(0.0..TAU),
            })
            .collect();
    }
}

impl Scene for Sonar {
    fn name(&self) -> &'static str {
        "sonar"
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

        let cx = w as f32 * 0.5;
        let cy = h as f32 * 0.5;
        let rx = (w as f32 * 0.5).max(1.0);
        let ry = (h as f32 * 0.5).max(1.0);

        // --- sweep arm: steady rotation with a gentle breathing rate ---
        let speed = 0.55 * (1.0 + 0.1 * (t * 0.23).sin());
        self.sweep = (self.sweep + speed * dt).rem_euclid(TAU);
        let sweep = self.sweep;

        // --- ping event: anticipation -> payoff -> decay ---
        self.next_ping -= dt;
        if self.next_ping <= 0.0 {
            self.ping_age = 0.0;
            self.next_ping = self.rng.random_range(15.0..25.0);
            for c in &mut self.contacts {
                c.pinged = false;
            }
        }
        let mut ping_r = -1.0f32;
        let mut ping_a = 0.0f32;
        if self.ping_age >= 0.0 {
            self.ping_age += dt;
            if self.ping_age >= PING_DUR {
                self.ping_age = -1.0;
            } else {
                let p = self.ping_age / PING_DUR;
                ping_r = ease_out(p) * 1.12; // radiates past the rim
                ping_a = (1.0 - p).powf(1.5) * 0.55;
            }
        }
        // anticipation: the center hums and swells just before release
        let charge = if self.ping_age < 0.0 && self.next_ping < PING_CHARGE {
            ease_smooth((1.0 - self.next_ping / PING_CHARGE).clamp(0.0, 1.0))
        } else {
            0.0
        };

        // --- occasionally send one contact darting across the scope ---
        self.next_fast -= dt;
        if self.next_fast <= 0.0 && !self.contacts.is_empty() {
            let i = self.rng.random_range(0..self.contacts.len());
            self.contacts[i].fast_t = 1.6;
            self.contacts[i].heading = self.rng.random_range(0.0..TAU);
            self.next_fast = self.rng.random_range(7.0..13.0);
        }

        // --- draw: pure black, then the scope pass (grid + wedge + ping) ---
        // line widths in normalized units, widened on small scopes so rings
        // and the pulse still land on whole cells instead of aliasing away
        let ring_w = (0.55 / rx.max(ry)).max(0.008);
        let ping_w = (0.9 / rx.max(ry)).max(0.02);
        canvas.clear((0, 0, 0));
        for y in 0..h {
            let ny = (y as f32 + 0.5 - cy) / ry;
            for x in 0..w {
                let nx = (x as f32 + 0.5 - cx) / rx;
                let r2 = nx * nx + ny * ny;
                if r2 > 1.0 {
                    continue; // off-scope stays black
                }
                let r = r2.sqrt();
                let behind = (sweep - ny.atan2(nx)).rem_euclid(TAU);

                // phosphor afterglow wedge, decaying behind the arm
                let k = 1.0 - behind / WEDGE_ARC;
                let wedge = if k > 0.0 { k * k } else { 0.0 };

                // range rings + cross ticks at the ring crossings (~8%)
                let ring_d = RINGS
                    .iter()
                    .map(|rr| (r - rr).abs())
                    .fold(1.0f32, f32::min);
                let ring_b = (1.0 - ring_d / ring_w).max(0.0) * 0.7;
                let axis_d = (nx.abs() * rx).min(ny.abs() * ry); // cells off-axis
                let tick_b = if axis_d < 0.7 {
                    (1.0 - ring_d / (ring_w * 1.6)).max(0.0)
                } else {
                    0.0
                };
                let grid = (ring_b + tick_b).min(1.0) * 0.085;

                // wedge glow pools a little toward the center
                let mut f = grid + wedge * (0.05 + 0.26 * (1.0 - r * 0.55));

                // the ping pulse radiating outward
                if ping_r >= 0.0 {
                    let pd = (r - ping_r).abs();
                    f += (1.0 - pd / ping_w).max(0.0) * ping_a;
                }

                canvas.set(x as i32, y as i32, scale(self.base, f.min(1.0)));
            }
        }

        // background plane: receiver-noise speckle, additive so it sits over
        // the scope too; in-scope speckles caught in the wedge brighten a touch
        for nd in &self.noise {
            let nx = (nd.x + 0.5 - cx) / rx;
            let ny = (nd.y + 0.5 - cy) / ry;
            let wedge = if nx * nx + ny * ny <= 1.0 {
                let behind = (sweep - ny.atan2(nx)).rem_euclid(TAU);
                let k = 1.0 - behind / WEDGE_ARC;
                if k > 0.0 { k * k } else { 0.0 }
            } else {
                0.0
            };
            let tw = 0.6 + 0.4 * (t * 0.6 + nd.phase).sin();
            canvas.add(
                nd.x as i32,
                nd.y as i32,
                scale(self.base, nd.mag * tw + wedge * 0.05),
            );
        }

        // the arm itself: a crisp bright line from center to rim
        let arm_steps = (rx.max(ry) * 1.05) as i32;
        for i in 0..arm_steps {
            let u = i as f32 / arm_steps as f32;
            let ax = cx + sweep.cos() * u * rx;
            let ay = cy + sweep.sin() * u * ry;
            canvas.add(ax as i32, ay as i32, scale(self.base, 0.65 * (1.0 - u * 0.6)));
        }
        glow(canvas, cx as i32, cy as i32, 2, self.base, 0.5);
        glow(
            canvas,
            (cx + sweep.cos() * rx * 0.98) as i32,
            (cy + sweep.sin() * ry * 0.98) as i32,
            1,
            self.base,
            0.5,
        );
        // ping charge-up: a hot swell gathers at the center before release
        if charge > 0.0 {
            let ci = cx as i32;
            let cj = cy as i32;
            glow(canvas, ci, cj, 1 + (charge * 2.0) as i32, self.base, charge * 0.45);
            canvas.add(ci, cj, scale(lerp(self.base, self.hot, charge), charge * 0.6));
        }

        // --- contacts: drift, get lit by the sweep, flare on the ping ---
        let base = self.base;
        let hot = self.hot;
        // reused spawn buffer: no per-frame alloc while contacts iterate
        let mut new_echoes = std::mem::take(&mut self.echo_scratch);
        new_echoes.clear();
        let echo_room = MAX_ECHOES.saturating_sub(self.echoes.len());
        for c in &mut self.contacts {
            // slow drift with a sinusoidal wander; fast movers dart
            c.heading += (t * 0.35 + c.seed * 9.0).sin() * 0.35 * dt;
            let fast_k = if c.fast_t > 0.0 {
                c.fast_t -= dt;
                ease_smooth((c.fast_t / 1.6).min(1.0)) * 6.0
            } else {
                0.0
            };
            let spd = c.speed * (1.0 + fast_k);
            c.px = c.x;
            c.py = c.y;
            c.x = (c.x + c.heading.cos() * spd * dt).rem_euclid(w as f32);
            c.y = (c.y + c.heading.sin() * spd * dt).rem_euclid(h as f32);

            // scope-space position
            let cnx = (c.x - cx) / rx;
            let cny = (c.y - cy) / ry;
            let cr2 = cnx * cnx + cny * cny;
            if cr2 > 1.0 {
                c.lit = (c.lit - dt / 2.0).max(0.0); // off-scope fades quickly
                continue;
            }
            let cr = cr2.sqrt();
            let behind = (sweep - cny.atan2(cnx)).rem_euclid(TAU);
            // the arm just passed this contact: recharge its phosphor
            if behind < speed * dt * 1.5 {
                c.lit = 1.0;
            }
            // persistence-of-vision decay over several seconds
            c.lit = (c.lit - dt / (4.0 + c.seed * 2.5)).max(0.0);
            c.flare = (c.flare - dt * 1.6).max(0.0);

            // ping crossing: flare bright and echo a faint ring
            if ping_r >= 0.0 && !c.pinged && cr <= ping_r {
                c.pinged = true;
                c.flare = 1.0;
                c.lit = 1.0;
                if new_echoes.len() < echo_room {
                    new_echoes.push(Echo {
                        x: c.x,
                        y: c.y,
                        age: 0.0,
                        strength: 0.6 + c.seed * 0.4,
                    });
                }
            }

            // brightness: persistence modulated by the local wedge glow
            let k = 1.0 - behind / WEDGE_ARC;
            let wedge = if k > 0.0 { k * k } else { 0.0 };
            let b = (c.lit * (0.3 + 0.7 * wedge) + c.flare).min(1.3);
            if b < 0.03 {
                continue;
            }
            let b = b.min(1.0);
            let col = lerp(scale(base, b), hot, (c.flare * 0.55).min(0.6));

            // trail: long and bright while darting, a short stub otherwise
            let steps = if c.fast_t > 0.0 { 7 } else { 2 };
            for i in 1..=steps {
                let u = i as f32 / (steps + 1) as f32;
                canvas.add(
                    (c.x + (c.px - c.x) * u) as i32,
                    (c.y + (c.py - c.y) * u) as i32,
                    scale(col, (1.0 - u) * 0.45),
                );
            }
            canvas.set(c.x as i32, c.y as i32, col);
            if c.flare > 0.4 {
                glow(canvas, c.x as i32, c.y as i32, 2, base, c.flare * 0.5);
            }
        }
        self.echoes.append(&mut new_echoes);
        self.echo_scratch = new_echoes; // keep the capacity for next frame

        // --- ping echoes: small rings ringing off each flared contact ---
        self.echoes.retain_mut(|e| {
            e.age += dt;
            let dur = 1.1;
            if e.age >= dur {
                return false;
            }
            let p = e.age / dur;
            let rad = ease_out(p) * 4.5;
            let alpha = (1.0 - p).powf(1.5) * 0.4 * e.strength;
            let steps = (rad * 6.0) as i32 + 8;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * TAU;
                let px = e.x + a.cos() * rad;
                let py = e.y + a.sin() * rad * 0.55; // squash for cell aspect
                canvas.add(px as i32, py as i32, scale(base, alpha * 0.5));
            }
            true
        });
    }
}
