//! Clockwork: interlocking brass gears turning in the dark. A spanning
//! train of 4-8 gears meshes tooth-to-tooth — angular velocities in exact
//! inverse-radius ratios, directions alternating, tooth phases set so the
//! teeth interleave at every mesh point. Wheels run large and crop at the
//! frame edges like a macro shot of a movement. Two depth layers (far gears
//! are smaller and dimmer), specular arcs that sweep as each wheel turns,
//! glint sparkles on tooth tips, and a wind-up event every ~20-30s: the gears
//! ease to a stop (anticipation), shudder under mainspring tension, then
//! release in a motion-blurred spin-up that decays back to the steady rate.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::{PI, TAU};

/// Teeth per pixel of pitch radius — shared by every gear so meshing
/// ratios stay exact (teeth counts are exactly proportional to radius).
const TEETH_PER_PX: f32 = 1.35;
/// Tooth duty cycle: fraction of the angular pitch a tooth occupies.
const DUTY: f32 = 0.42;
/// Unit-ish key light direction (upper left).
const LX: f32 = -0.63;
const LY: f32 = -0.78;

const EASE_DUR: f32 = 2.0;
const SHUD_DUR: f32 = 1.3;
const REL_DUR: f32 = 3.0;
const MAX_GLINTS: usize = 8;

#[derive(Clone, Copy)]
struct Palette {
    dark: (u8, u8, u8),
    mid: (u8, u8, u8),
    bright: (u8, u8, u8),
    spec: (u8, u8, u8),
}

#[derive(Clone, Copy)]
struct Gear {
    x: f32,
    y: f32,
    r: f32,          // pitch radius
    teeth: u32,
    tooth_h: f32,
    rim_w: f32,
    hub_r: f32,
    spokes: u32,
    spoke_px: f32,   // spoke half-width in arc pixels
    mid_ring: bool,  // large gears get an inner web ring
    rot: f32,        // integrated rotation
    omega: f32,      // base angular velocity (signed)
    phase: f32,      // tooth phase, set so meshing teeth interleave
    spec_off: f32,   // specular arc offset in the gear-local frame
    seed: f32,
    far: bool,
    dim: f32,
}

struct Glint {
    gear: usize,
    tooth: u32,
    age: f32,
    dur: f32,
}

/// Wind-up rhythm: Idle -> Ease (stop) -> Shudder -> Release -> Idle.
#[derive(Clone, Copy)]
enum Wind {
    Idle,
    Ease(f32),
    Shudder(f32),
    Release(f32),
}

/// Shortest angular distance from `a` to a multiple of TAU.
fn ang_dist(a: f32) -> f32 {
    let r = a.rem_euclid(TAU);
    if r > PI {
        TAU - r
    } else {
        r
    }
}

/// Two polished arcs riding the gear (they sweep as it turns).
fn spec_arc(local: f32, off: f32) -> f32 {
    let d = ang_dist(local - off).min(ang_dist(local - off - PI * 0.85));
    let s = (1.0 - d / 0.6).max(0.0);
    s * s
}

impl Gear {
    /// Body color + specular intensity at pixel center (px, py), with the
    /// gear drawn at rotation `rot_draw`. None = outside the gear.
    fn eval(&self, px: f32, py: f32, rot_draw: f32, pal: &Palette) -> Option<((u8, u8, u8), f32)> {
        let dx = px - self.x;
        let dy = py - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let r_out = self.r + self.tooth_h;
        if dist > r_out + 0.4 {
            return None;
        }
        let ang = dy.atan2(dx);
        let local = ang - rot_draw - self.phase;
        // directional key light with a floor of ambient bounce
        let dir_l = if dist > 0.001 {
            (0.35 + 0.65 * (0.5 + 0.5 * (dx * LX + dy * LY) / dist)).clamp(0.0, 1.0)
        } else {
            0.6
        };
        let teeth_f = self.teeth as f32;
        let spokes_f = self.spokes as f32;

        let mut body: Option<(u8, u8, u8)> = None;
        let mut spec = 0.0f32;

        if dist > self.r {
            // tooth band: teeth taper slightly toward the tip
            let tb = ((dist - self.r) / self.tooth_h).clamp(0.0, 1.0);
            let f = (local * teeth_f / TAU).rem_euclid(1.0);
            if f < DUTY * (1.0 - 0.30 * tb) {
                let shade = lerp(pal.dark, pal.mid, dir_l);
                // tips catch the light
                body = Some(lerp(shade, pal.bright, tb * dir_l * 0.85));
                spec = spec_arc(local, self.spec_off) * 0.35 * tb;
            }
        } else if dist > self.r - self.rim_w {
            // rim, outer edge brighter on the lit side
            let k = (dist - (self.r - self.rim_w)) / self.rim_w;
            body = Some(lerp(pal.dark, lerp(pal.mid, pal.bright, k * 0.7), dir_l));
            spec = spec_arc(local, self.spec_off) * (0.4 + 0.6 * k);
            // bolt heads seated in the rim between the spokes
            let bf = (local * spokes_f / TAU + 0.5).rem_euclid(1.0);
            let bd = bf.min(1.0 - bf) * TAU / spokes_f * dist;
            if bd < 0.7 {
                body = Some(lerp(body.unwrap_or(pal.mid), pal.bright, 0.55));
            }
        } else if dist < self.hub_r {
            // hub with a dark axle bore
            body = Some(lerp(pal.mid, pal.bright, 0.25 + 0.45 * dir_l));
            if dist < (self.hub_r * 0.35).max(1.0) {
                body = Some(pal.dark);
            }
            spec = spec_arc(local, self.spec_off) * 0.5;
        } else {
            // web: constant-width spokes plus an inner ring on big wheels
            let sf = (local * spokes_f / TAU).rem_euclid(1.0);
            let sang = sf.min(1.0 - sf) * TAU / spokes_f;
            if sang * dist < self.spoke_px {
                body = Some(lerp(pal.dark, pal.mid, 0.3 + 0.7 * dir_l));
                spec = spec_arc(local, self.spec_off) * 0.6;
            }
            if self.mid_ring && (dist - self.r * 0.58).abs() < 0.9 {
                body = Some(lerp(pal.dark, pal.mid, dir_l));
            }
        }

        body.map(|c| (c, spec))
    }
}

pub struct Clockwork {
    rng: StdRng,
    detail: Detail,
    pal: Palette,
    gears: Vec<Gear>,
    glints: Vec<Glint>,
    glint_timer: f32,
    wind: Wind,
    next_wind: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Clockwork {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let pal = match theme {
            Some("steel") => Palette {
                dark: (44, 50, 60),
                mid: (148, 158, 172),
                bright: (206, 216, 228),
                spec: (244, 250, 255),
            },
            Some("verdigris") => Palette {
                dark: (20, 52, 44),
                mid: (78, 140, 116),
                bright: (150, 196, 164),
                spec: (214, 244, 226),
            },
            _ => Palette {
                dark: (70, 50, 20),
                mid: (186, 136, 56),
                bright: (240, 192, 102),
                spec: (255, 240, 196),
            },
        };
        Clockwork {
            rng,
            detail,
            pal,
            gears: Vec::new(),
            glints: Vec::new(),
            glint_timer: 2.0,
            wind: Wind::Idle,
            next_wind: 14.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn make_gear(
        rng: &mut StdRng,
        x: f32,
        y: f32,
        teeth: u32,
        omega: f32,
        phase: f32,
        far: bool,
    ) -> Gear {
        let r = teeth as f32 / TEETH_PER_PX;
        Gear {
            x,
            y,
            r,
            teeth,
            tooth_h: (r * 0.16 + 0.9).min(3.2),
            rim_w: (r * 0.16).clamp(0.9, 2.4),
            hub_r: r * 0.22 + 0.8,
            spokes: rng.random_range(3..=5),
            spoke_px: (r * 0.11).clamp(0.7, 1.7),
            mid_ring: r > 9.0,
            rot: 0.0,
            omega,
            phase,
            spec_off: rng.random_range(0.0..TAU),
            seed: rng.random_range(0.0..TAU),
            far,
            dim: if far { 0.45 } else { 1.0 },
        }
    }

    /// (Re)build the gear train for the current canvas size: a root wheel
    /// near center, each later wheel meshed onto a random earlier one.
    fn build(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.gears.clear();
        self.glints.clear();
        let n = self
            .detail
            .scale(4.5 + 2.0 * density_for(w, h), 4)
            .clamp(4, 9);
        let min_dim = w.min(h) as f32;

        // root gear
        let r0 = (min_dim * self.rng.random_range(0.16..0.24))
            .max(3.0)
            .min(26.0);
        let teeth0 = ((r0 * TEETH_PER_PX).round() as u32).max(4);
        let omega0 = self.rng.random_range(0.35..0.55)
            * if self.rng.random::<bool>() { 1.0 } else { -1.0 };
        let rx = w as f32 * 0.5 + self.rng.random_range(-0.12..0.12) * w as f32;
        let ry = h as f32 * 0.5 + self.rng.random_range(-0.12..0.12) * h as f32;
        let rphase = self.rng.random_range(0.0..TAU);
        let root = Self::make_gear(&mut self.rng, rx, ry, teeth0, omega0, rphase, false);
        self.gears.push(root);

        for _ in 1..n {
            // mostly chain off the last wheel: elongated trains that cross
            // the frame instead of a tight central cluster
            let pi = if self.rng.random::<f32>() < 0.55 {
                self.gears.len() - 1
            } else {
                self.rng.random_range(0..self.gears.len())
            };
            let parent = self.gears[pi];
            let far = self.rng.random::<f32>() < 0.35;
            let mut r = min_dim * self.rng.random_range(0.08..0.26);
            r = r.max(2.5).min(26.0);
            if far {
                r *= 0.75;
            }
            let teeth = ((r * TEETH_PER_PX).round() as u32).max(4);
            let r = teeth as f32 / TEETH_PER_PX;
            let d = parent.r + r - 0.5; // slight overlap: teeth interleave
            // grow the train outward: of 8 candidate mesh angles prefer the
            // one farthest from the train's centroid, as long as the wheel
            // stays mostly on canvas (centers may crop 0.35r past the edge,
            // like a close-up of a movement)
            let (cx, cy) = {
                let (mut sx, mut sy) = (0.0f32, 0.0f32);
                for g in &self.gears {
                    sx += g.x;
                    sy += g.y;
                }
                (sx / self.gears.len() as f32, sy / self.gears.len() as f32)
            };
            let m = r * 0.35;
            let mut best_a = 0.0f32;
            let mut best_score = f32::MAX;
            for _ in 0..8 {
                let a = self.rng.random_range(0.0..TAU);
                let x = parent.x + a.cos() * d;
                let y = parent.y + a.sin() * d;
                let off = (x - (w as f32 + m)).max(0.0)
                    + (-m - x).max(0.0)
                    + (y - (h as f32 + m)).max(0.0)
                    + (-m - y).max(0.0);
                let spread = (x - cx).hypot(y - cy);
                let score = off * 50.0 - spread;
                if score < best_score {
                    best_score = score;
                    best_a = a;
                }
            }
            let a = best_a;
            let x = parent.x + a.cos() * d;
            let y = parent.y + a.sin() * d;
            // exact inverse-radius ratio, alternating direction
            let omega = -parent.omega * parent.teeth as f32 / teeth as f32;
            // tooth phase so a tooth of one wheel meets a gap of the other:
            // p_i(a) + p_j(a+PI) == 0.5 + DUTY (constant over time, since
            // both patterns sweep the contact point at the same rate)
            let p_i =
                ((a - parent.rot - parent.phase) * parent.teeth as f32 / TAU).rem_euclid(1.0);
            let phase = (a + PI - (0.5 + DUTY - p_i) * TAU / teeth as f32).rem_euclid(TAU);
            let g = Self::make_gear(&mut self.rng, x, y, teeth, omega, phase, far);
            self.gears.push(g);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_gear(
        canvas: &mut Canvas,
        g: &Gear,
        pal: &Palette,
        rot_draw: f32,
        omega_draw: f32,
        bright: f32,
        blur: f32,
    ) {
        let r_out = g.r + g.tooth_h + 1.0;
        let x0 = (g.x - r_out).floor() as i32;
        let x1 = (g.x + r_out).ceil() as i32;
        let y0 = (g.y - r_out).floor() as i32;
        let y1 = (g.y + r_out).ceil() as i32;
        let dim = g.dim;

        // motion-blur streaks trailing the spin direction (release payoff)
        if blur > 0.04 {
            for &(back, fb) in &[(0.10f32, 0.30f32), (0.22, 0.14)] {
                let rb = rot_draw - omega_draw * back;
                let inten = bright * dim * blur * fb;
                for py in y0..=y1 {
                    for px in x0..=x1 {
                        if let Some((c, sp)) = g.eval(px as f32 + 0.5, py as f32 + 0.5, rb, pal) {
                            canvas.add(px, py, scale(c, inten));
                            if sp > 0.02 {
                                canvas.add(px, py, scale(pal.spec, sp * inten * 0.5));
                            }
                        }
                    }
                }
            }
        }

        for py in y0..=y1 {
            for px in x0..=x1 {
                if let Some((c, sp)) = g.eval(px as f32 + 0.5, py as f32 + 0.5, rot_draw, pal) {
                    canvas.set(px, py, scale(c, bright * dim));
                    if sp > 0.02 {
                        canvas.add(px, py, scale(pal.spec, sp * 0.55 * bright * dim));
                    }
                }
            }
        }
    }
}

impl Scene for Clockwork {
    fn name(&self) -> &'static str {
        "clockwork"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);

        if w != self.w || h != self.h {
            self.build(w, h);
        }
        self.t += dt;

        // --- wind-up event: anticipation -> shudder -> release -> decay ---
        let mut speed = 1.0f32;
        let mut bright = 1.0f32;
        let mut shudder = 0.0f32;
        match self.wind {
            Wind::Idle => {
                self.next_wind -= dt;
                if self.next_wind <= 0.0 {
                    self.wind = Wind::Ease(0.0);
                }
            }
            Wind::Ease(e) => {
                let e = e + dt;
                let k = (e / EASE_DUR).min(1.0);
                speed = 1.0 - ease_smooth(k);
                bright = 1.0 - 0.28 * ease_smooth(k);
                self.wind = if e >= EASE_DUR {
                    Wind::Shudder(0.0)
                } else {
                    Wind::Ease(e)
                };
            }
            Wind::Shudder(e) => {
                let e = e + dt;
                speed = 0.0;
                shudder = ease_smooth(e / SHUD_DUR);
                bright = 0.72 + 0.05 * (self.t * 31.0).sin() * shudder;
                self.wind = if e >= SHUD_DUR {
                    Wind::Release(0.0)
                } else {
                    Wind::Shudder(e)
                };
            }
            Wind::Release(e) => {
                let e = e + dt;
                let decay = (-2.6 * e).exp();
                speed = 1.0 + 3.2 * decay; // mainspring snap, then settle
                bright = 0.85 + 0.45 * decay;
                self.wind = if e >= REL_DUR {
                    self.next_wind = self.rng.random_range(20.0..30.0);
                    Wind::Idle
                } else {
                    Wind::Release(e)
                };
            }
        }
        let blur = ((speed - 1.0) / 3.0).clamp(0.0, 1.0);
        let releasing = matches!(self.wind, Wind::Release(_));

        // --- integrate rotations (ratios preserved by the common speed) ---
        for g in &mut self.gears {
            g.rot = (g.rot + g.omega * speed * dt).rem_euclid(TAU);
        }

        // --- glint sparkles on tooth tips ---
        self.glint_timer -= dt;
        if self.glint_timer <= 0.0 {
            if self.glints.len() < MAX_GLINTS && !self.gears.is_empty() {
                let gi = self.rng.random_range(0..self.gears.len());
                let tooth = self.rng.random_range(0..self.gears[gi].teeth);
                self.glints.push(Glint {
                    gear: gi,
                    tooth,
                    age: 0.0,
                    dur: self.rng.random_range(0.35..0.6),
                });
            }
            self.glint_timer = if releasing {
                self.rng.random_range(0.4..1.0)
            } else {
                self.rng.random_range(1.6..4.2)
            };
        }
        for gl in &mut self.glints {
            gl.age += dt;
        }
        self.glints.retain(|gl| gl.age < gl.dur);

        // --- draw ---
        canvas.clear((0, 0, 0));

        // faint ambient halo behind near gears; swells with the release
        // payoff so the spin-up bleeds light onto its surroundings
        for g in &self.gears {
            if !g.far {
                let hr = ((g.r * 0.9).clamp(3.0, 9.0) + blur * 3.0) as i32;
                let inten = (0.05 * bright + 0.12 * blur).min(0.22);
                glow(canvas, g.x as i32, g.y as i32, hr, self.pal.mid, inten);
            }
        }

        // far layer first, then near (occlusion reads as depth)
        for pass in 0..2 {
            for i in 0..self.gears.len() {
                let g = self.gears[i];
                if (pass == 0) != g.far {
                    continue;
                }
                let tremble = (self.t * 38.0 + g.seed * 9.0).sin() * 0.03 * shudder;
                let rot_draw = g.rot + tremble;
                Self::draw_gear(
                    canvas,
                    &g,
                    &self.pal,
                    rot_draw,
                    g.omega * speed,
                    bright,
                    blur,
                );
            }
        }

        for gl in &self.glints {
            let g = self.gears[gl.gear];
            // far-layer glints recede with their gear
            let env = (gl.age / gl.dur * PI).sin() * g.dim; // eased hump
            if env <= 0.01 {
                continue;
            }
            let ang = g.rot + g.phase + (gl.tooth as f32 + DUTY * 0.5) * TAU / g.teeth as f32;
            let rr = g.r + g.tooth_h * 0.95;
            let x = (g.x + ang.cos() * rr) as i32;
            let y = (g.y + ang.sin() * rr) as i32;
            canvas.set(x, y, lerp(self.pal.spec, (255, 255, 255), 0.5 * env));
            canvas.add(x + 1, y, scale(self.pal.spec, 0.55 * env));
            canvas.add(x - 1, y, scale(self.pal.spec, 0.55 * env));
            canvas.add(x, y + 1, scale(self.pal.spec, 0.55 * env));
            canvas.add(x, y - 1, scale(self.pal.spec, 0.55 * env));
            glow(canvas, x, y, 2, self.pal.spec, 0.30 * env * bright);
        }
    }
}
