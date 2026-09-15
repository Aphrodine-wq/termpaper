//! Warp starfield: stars fly outward from center with perspective and trails.
//! Nebulas breathe behind the stars; a tumbling asteroid drifts past now and
//! then. Event: a hyperspace SURGE on an anticipation -> payoff -> decay
//! rhythm — the warp point glows as it charges, stars stretch into long
//! streaks and the nebulas flare, then everything eases back to cruise.

use super::{Detail, Scene};
use crate::canvas::{density_for, dot, ease_smooth, glow_f, lerp, line_f, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const TAU: f32 = std::f32::consts::TAU;
const CHARGE_TIME: f32 = 0.9;
const BOOST_TIME: f32 = 1.4;
const DECAY_TIME: f32 = 2.2;

/// Surge rhythm: Idle -> Charge (warp point glows) -> Boost (full stretch)
/// -> Decay (ease back to cruise speed).
#[derive(Clone, Copy, PartialEq)]
enum Surge {
    Idle,
    Charge(f32), // elapsed
    Boost(f32),
    Decay(f32),
}

struct Star {
    x: f32,
    y: f32,
    z: f32,
    prev: Option<(f32, f32)>,
    tint: (u8, u8, u8),
    mag: f32,
    age: f32,   // fade-in envelope so respawns never pop
    phase: f32, // twinkle offset
}

/// Perspective projection: returns unit-space screen coords centered on 0.
/// As `z` shrinks, the projected point moves outward from the center.
pub fn project(x: f32, y: f32, z: f32) -> (f32, f32) {
    (x / z, y / z)
}

fn spawn(rng: &mut StdRng, z_max: f32, tints: &[(u8, u8, u8)]) -> Star {
    let tint = tints[rng.random_range(0..tints.len())];
    Star {
        x: rng.random_range(-1.0..1.0),
        y: rng.random_range(-1.0..1.0),
        z: rng.random_range(0.2..z_max),
        prev: None,
        tint,
        mag: if rng.random::<f32>() < 0.08 {
            rng.random_range(1.3..1.8) // occasional bright star
        } else {
            rng.random_range(0.5..1.0)
        },
        age: 0.0,
        phase: rng.random_range(0.0..TAU),
    }
}

struct Nebula {
    x: f32,
    y: f32,
    r: f32,
    color: (u8, u8, u8),
}

pub struct Starfield {
    rng: StdRng,
    detail: Detail,
    tints: Vec<(u8, u8, u8)>,
    neb_colors: Vec<(u8, u8, u8)>,
    bg: (u8, u8, u8),
    stars: Vec<Star>,
    nebulas: Vec<Nebula>,
    surge: Surge,
    next_surge: f32,
    asteroid: Option<(f32, f32, f32, f32)>, // x, y, vx, tumble
    next_asteroid: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Starfield {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (tints, neb_colors, bg) = match theme {
            Some("ice") => (
                vec![(235, 245, 255), (170, 200, 255), (200, 230, 255), (255, 255, 255)],
                vec![(40, 80, 150), (60, 120, 170)],
                (0, 0, 0),
            ),
            Some("warm") => (
                vec![(255, 240, 215), (255, 210, 160), (255, 180, 140), (235, 235, 245)],
                vec![(150, 60, 50), (150, 90, 40)],
                (0, 0, 0),
            ),
            Some("void") => (
                vec![(200, 200, 210), (170, 175, 200)],
                vec![(40, 30, 60)],
                (0, 0, 0),
            ),
            _ => (
                vec![(255, 255, 255), (255, 255, 255), (170, 195, 255), (215, 175, 255), (255, 240, 220)],
                vec![(90, 40, 140), (30, 80, 130), (140, 60, 60)],
                (0, 0, 0),
            ),
        };
        Starfield {
            rng,
            detail,
            tints,
            neb_colors,
            bg,
            stars: Vec::new(),
            nebulas: Vec::new(),
            surge: Surge::Idle,
            next_surge: 18.0,
            asteroid: None,
            next_asteroid: 14.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// Eased surge envelope 0..1 for the current phase.
    fn surge_k(&self) -> f32 {
        match self.surge {
            Surge::Idle => 0.0,
            Surge::Charge(e) => ease_smooth(e / CHARGE_TIME) * 0.18,
            Surge::Boost(e) => 0.18 + ease_smooth(e / BOOST_TIME) * 0.82,
            Surge::Decay(e) => 1.0 - ease_smooth(e / DECAY_TIME),
        }
    }
}

impl Scene for Starfield {
    fn name(&self) -> &'static str {
        "starfield"
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
            let n = self.detail.scale((w * h / 60) as f32 * 2.0 * density_for(w, h), 40).min(2400);
            let tints = self.tints.clone();
            self.stars = (0..n).map(|_| spawn(&mut self.rng, 1.0, &tints)).collect();
            let nc = self.neb_colors.clone();
            self.nebulas = (0..self.rng.random_range(3..=5))
                .map(|_| Nebula {
                    x: self.rng.random_range(w as f32 * 0.1..w as f32 * 0.9),
                    y: self.rng.random_range(h as f32 * 0.1..h as f32 * 0.9),
                    r: self.rng.random_range(h as f32 * 0.15..h as f32 * 0.35),
                    color: nc[self.rng.random_range(0..nc.len())],
                })
                .collect();
        }
        self.t += dt;

        // hyperspace surge scheduling: anticipation -> payoff -> decay
        self.next_surge -= dt;
        self.surge = match self.surge {
            Surge::Idle => {
                if self.next_surge <= 0.0 {
                    Surge::Charge(0.0)
                } else {
                    Surge::Idle
                }
            }
            Surge::Charge(e) => {
                let e = e + dt;
                if e >= CHARGE_TIME {
                    Surge::Boost(0.0)
                } else {
                    Surge::Charge(e)
                }
            }
            Surge::Boost(e) => {
                let e = e + dt;
                if e >= BOOST_TIME {
                    Surge::Decay(0.0)
                } else {
                    Surge::Boost(e)
                }
            }
            Surge::Decay(e) => {
                let e = e + dt;
                if e >= DECAY_TIME {
                    self.next_surge = self.rng.random_range(22.0..35.0);
                    Surge::Idle
                } else {
                    Surge::Decay(e)
                }
            }
        };
        let surge_k = self.surge_k();

        canvas.clear(self.bg);

        // faint nebula patches behind the stars, breathing slowly; the surge
        // flares them as the warp field energizes
        for (ni, nb) in self.nebulas.iter().enumerate() {
            let breathe = (0.13 + 0.05 * (self.t * 0.3 + ni as f32 * 2.1).sin())
                * (1.0 + surge_k * 1.6);
            let ri = nb.r as i32;
            let r2 = nb.r * nb.r;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d2 = (dx * dx + dy * dy) as f32;
                    if d2 < r2 {
                        let fall = 1.0 - d2 / r2;
                        canvas.add(nb.x as i32 + dx, nb.y as i32 + dy, scale(nb.color, fall * fall * breathe));
                    }
                }
            }
        }

        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        // vertical scale accounts for cells being ~twice as tall as wide
        let scale_x = w as f32 * 0.5;
        let scale_y = h as f32 * 0.9;

        // the warp point glows as the surge charges and burns white at full boost
        if surge_k > 0.02 {
            let gc = lerp(self.tints[0], (255, 255, 255), surge_k * 0.5);
            // fractional radius: the charge-up swells smoothly instead of
            // stepping through integer radii
            glow_f(canvas, cx, cy, 2.0 + surge_k * 4.0, gc, surge_k * 0.55);
        }

        // a tumbling asteroid drifts past now and then
        self.next_asteroid -= dt;
        if self.next_asteroid <= 0.0 && self.asteroid.is_none() {
            let from_left = self.rng.random::<bool>();
            self.asteroid = Some((
                if from_left { -8.0 } else { w as f32 + 8.0 },
                self.rng.random_range(h as f32 * 0.2..h as f32 * 0.7),
                if from_left { 6.0 } else { -6.0 },
                self.rng.random_range(0.0..6.0),
            ));
            self.next_asteroid = self.rng.random_range(18.0..35.0);
        }
        if let Some((ax, ay, avx, tumble)) = &mut self.asteroid {
            *ax += *avx * dt;
            if *ax < -10.0 || *ax > w as f32 + 10.0 {
                self.asteroid = None;
            } else {
                // lumpy dark rock with one lit edge, slowly rotating; the
                // surge rim-lights it as the warp field sweeps past
                let rot = self.t * 0.8 + *tumble;
                let lit_c = lerp((70, 60, 55), (215, 195, 170), surge_k * 0.6);
                for dy in -2..=2 {
                    for dx in -3..=3 {
                        let d2 = dx * dx + dy * dy * 2;
                        if d2 <= 6 {
                            let lit = (dx as f32 * rot.cos() - dy as f32 * rot.sin()) > 0.5;
                            canvas.set(
                                *ax as i32 + dx,
                                *ay as i32 + dy,
                                if lit { lit_c } else { (22, 20, 18) },
                            );
                        }
                    }
                }
            }
        }

        let t = self.t;
        let rng = &mut self.rng;
        let tints = &self.tints;
        for s in &mut self.stars {
            let (px, py) = project(s.x, s.y, s.z);
            let sx = cx + px * scale_x;
            let sy = cy + py * scale_y;

            // depth-based brightness: quadratic falloff keeps far stars faint
            // so near ones punch through; twinkle + fade-in keep it alive
            let depth = (1.0 - s.z).clamp(0.0, 1.0);
            let twinkle = 0.85 + 0.15 * (t * 2.2 + s.phase).sin();
            let fade = ease_smooth(s.age / 0.5);
            let bright =
                (0.10 + 0.90 * depth * depth) * s.mag * twinkle * fade * (1.0 + surge_k * 0.5);

            // fading trail from previous position to current; long during
            // surge. An AA segment slides subcell with the star, so warp
            // streaks stay unbroken lines instead of stippled columns.
            if let Some((ox, oy)) = s.prev {
                let dist = ((sx - ox).powi(2) + (sy - oy).powi(2)).sqrt();
                let reach = ((dist.min(6.0) + surge_k * 18.0) / dist.max(1e-3)).min(1.0);
                if dist > 0.05 {
                    // two segments fake the fade toward the tail
                    let (tx, ty) = (sx + (ox - sx) * reach, sy + (oy - sy) * reach);
                    let (mx, my) = ((tx + sx) * 0.5, (ty + sy) * 0.5);
                    line_f(canvas, tx, ty, mx, my, s.tint, bright * 0.16);
                    line_f(canvas, mx, my, sx, sy, s.tint, bright * 0.42);
                }
            }

            let color = lerp(scale(s.tint, bright), (255, 255, 255), depth * 0.4);
            dot(canvas, sx, sy, color, 1.0);
            if s.mag > 1.2 {
                // bright stars bleed into neighbors slightly; near-camera
                // whooshes flare as they pass
                let flare = if s.z < 0.2 { 1.6 } else { 1.0 };
                dot(canvas, sx + 1.0, sy, color, 0.35 * flare);
                dot(canvas, sx, sy + 1.0, color, 0.35 * flare);
                if s.z < 0.12 {
                    dot(canvas, sx - 1.0, sy, color, 0.5);
                    dot(canvas, sx, sy - 1.0, color, 0.5);
                    // the closest pass throws light onto its surroundings
                    glow_f(canvas, sx, sy, 2.0, color, 0.35);
                }
            }

            s.prev = Some((sx, sy));
            s.age += dt;
            s.z -= dt * (0.45 + surge_k * 2.4);

            let off = sx < -2.0 || sx > w as f32 + 2.0 || sy < -2.0 || sy > h as f32 + 2.0;
            if s.z < 0.06 || off {
                *s = spawn(rng, 1.0, tints);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_moves_outward_as_z_shrinks() {
        // same star, two depths: the nearer projection is farther from center
        let (x1, y1) = project(0.4, -0.3, 1.0);
        let (x2, y2) = project(0.4, -0.3, 0.5);
        assert!(x2.abs() > x1.abs());
        assert!(y2.abs() > y1.abs());
        // direction preserved
        assert!(x1.signum() == x2.signum() && y1.signum() == y2.signum());
    }

    #[test]
    fn scene_runs_and_stays_in_bounds() {
        use rand::SeedableRng;
        let mut s = Starfield::new(StdRng::seed_from_u64(42), None, Detail::Medium);
        let mut c = Canvas::new(40, 20);
        for _ in 0..60 {
            s.update(1.0 / 30.0, &mut c);
        }
        // some star light should have been drawn
        let lit = (0..40)
            .flat_map(|x| (0..20).map(move |y| (x, y)))
            .filter(|&(x, y)| c.get(x, y).color != (2, 2, 7))
            .count();
        assert!(lit > 10);
    }
}
