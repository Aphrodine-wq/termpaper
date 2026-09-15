//! Boids: flocking with separation/alignment/cohesion, wrap-around edges,
//! bright heading-oriented bodies with velocity-fading trails. Distant
//! near-black specks form a background plane; a red predator telegraphs at
//! the edge, sweeps through scattering the flock and rim-lighting nearby
//! boids; periodic murmuration swirls brighten the whole flock.

use super::{Detail, Scene};
use crate::canvas::{density_for, dot, ease_smooth, glow_f, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const MAX_SPEED: f32 = 34.0;
const MIN_SPEED: f32 = 14.0;
const PERCEPTION: f32 = 11.0;
const SEP_DIST: f32 = 4.5;

#[derive(Clone)]
struct Boid {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    px: f32,
    py: f32,
    tint: f32,  // 0..1 per-boid color variation
    phase: f32, // wander steering phase
    leader: bool,
}

struct Speck {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
    drift: f32,
}

pub struct Boids {
    rng: StdRng,
    detail: Detail,
    c_a: (u8, u8, u8),
    c_b: (u8, u8, u8),
    boids: Vec<Boid>,
    prev: Vec<Boid>, // scratch for the neighbor pass (no per-frame alloc)
    specks: Vec<Speck>,
    predator: Option<(f32, f32, f32, f32)>,          // x, y, vx, vy
    pred_pending: Option<(bool, f32, f32)>,          // from_left, y, countdown
    next_predator: f32,
    swirl: f32,
    next_swirl: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Boids {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (c_a, c_b) = match theme {
            Some("sunset") => ((255, 170, 90), (255, 100, 80)),
            Some("mono") => ((200, 210, 220), (130, 140, 155)),
            _ => ((140, 200, 255), (255, 205, 130)), // ice
        };
        Boids {
            detail,
            c_a,
            c_b,
            boids: Vec::new(),
            prev: Vec::new(),
            specks: Vec::new(),
            predator: None,
            pred_pending: None,
            next_predator: 10.0,
            swirl: 0.0,
            next_swirl: 16.0,
            t: 0.0,
            w: 0,
            h: 0,
            rng: {
                let _ = rng.random::<u64>();
                rng
            },
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let n = self.detail.scale((w * h / 220) as f32 * 2.0 * density_for(w, h), 30).clamp(30, 400);
        // spawn in a few loose clusters so streams interleave instead of
        // collapsing into a single ball
        let clusters: Vec<(f32, f32)> = (0..3)
            .map(|_| {
                (
                    self.rng.random_range(w as f32 * 0.2..w as f32 * 0.8),
                    self.rng.random_range(h as f32 * 0.2..h as f32 * 0.8),
                )
            })
            .collect();
        self.boids = (0..n)
            .map(|i| {
                let a = self.rng.random_range(0.0..std::f32::consts::TAU);
                let s = self.rng.random_range(MIN_SPEED..MAX_SPEED);
                let (cx, cy) = clusters[i % clusters.len()];
                Boid {
                    x: (cx + self.rng.random_range(-10.0..10.0)).rem_euclid(w as f32),
                    y: (cy + self.rng.random_range(-6.0..6.0)).rem_euclid(h as f32),
                    vx: a.cos() * s,
                    vy: a.sin() * s,
                    px: 0.0,
                    py: 0.0,
                    tint: self.rng.random::<f32>(),
                    phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                    leader: self.rng.random::<f32>() < 0.08,
                }
            })
            .collect();
        let ns = self
            .detail
            .scale((w * h / 300) as f32 * density_for(w, h), 12)
            .min(300);
        self.specks = (0..ns)
            .map(|_| Speck {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                mag: self.rng.random_range(0.05..0.14),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                drift: self.rng.random_range(0.2..0.9),
            })
            .collect();
    }

    fn step(&mut self, dt: f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let mut prev = std::mem::take(&mut self.prev);
        prev.clone_from(&self.boids);
        let predator = self.predator;
        // flock centroid, once per step (murmuration pivot)
        let (mut mx, mut my) = (0.0f32, 0.0f32);
        for o in prev.iter() {
            mx += o.x;
            my += o.y;
        }
        mx /= prev.len().max(1) as f32;
        my /= prev.len().max(1) as f32;
        let swirl_k = ease_smooth((self.swirl / 3.5).min(1.0)) * 60.0;
        for (i, b) in self.boids.iter_mut().enumerate() {
            let mut sep = (0.0f32, 0.0f32);
            let mut ali = (0.0f32, 0.0f32);
            let mut coh = (0.0f32, 0.0f32);
            let mut n_flock = 0usize;
            let mut n_sep = 0usize;
            for (j, o) in prev.iter().enumerate() {
                if i == j {
                    continue;
                }
                // shortest distance considering wrap-around
                let mut dx = o.x - b.x;
                let mut dy = o.y - b.y;
                if dx.abs() > w / 2.0 {
                    dx -= w * dx.signum();
                }
                if dy.abs() > h / 2.0 {
                    dy -= h * dy.signum();
                }
                let d2 = dx * dx + dy * dy;
                if d2 < PERCEPTION * PERCEPTION {
                    ali.0 += o.vx;
                    ali.1 += o.vy;
                    coh.0 += o.x + if (o.x - b.x).abs() > w / 2.0 { -w * (o.x - b.x).signum() } else { 0.0 };
                    coh.1 += o.y + if (o.y - b.y).abs() > h / 2.0 { -h * (o.y - b.y).signum() } else { 0.0 };
                    n_flock += 1;
                    if d2 < SEP_DIST * SEP_DIST && d2 > 1e-6 {
                        sep.0 -= dx / d2;
                        sep.1 -= dy / d2;
                        n_sep += 1;
                    }
                }
            }
            let mut ax = 0.0;
            let mut ay = 0.0;
            if n_sep > 0 {
                ax += sep.0 * 320.0;
                ay += sep.1 * 320.0;
            }
            if n_flock > 0 {
                let nf = n_flock as f32;
                // alignment: steer toward average velocity
                ax += (ali.0 / nf - b.vx) * 1.7;
                ay += (ali.1 / nf - b.vy) * 1.7;
                // cohesion: gentle capped pull toward the local center —
                // enough to keep streams coherent, not enough to ball up
                let (mut cx_, mut cy_) = ((coh.0 / nf - b.x) * 0.45, (coh.1 / nf - b.y) * 0.45);
                let cm = (cx_ * cx_ + cy_ * cy_).sqrt();
                if cm > 14.0 {
                    cx_ *= 14.0 / cm;
                    cy_ *= 14.0 / cm;
                }
                ax += cx_;
                ay += cy_;
            }
            // per-boid wander: slow sine steering keeps streams flowing
            ax += (self.t * 0.7 + b.phase).cos() * 16.0;
            ay += (self.t * 0.9 + b.phase * 1.7).sin() * 16.0;
            // murmuration swirl: the flock briefly orbits its centroid
            if swirl_k > 0.0 {
                let (dx, dy) = (mx - b.x, my - b.y);
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                // tangential + gentle inward pull, eased by swirl envelope
                ax += -dy / d * swirl_k + dx / d * swirl_k * 0.4;
                ay += dx / d * swirl_k + dy / d * swirl_k * 0.4;
            }
            // predator avoidance: scatter!
            if let Some((px_, py_, _, _)) = predator {
                let dx = b.x - px_;
                let dy = b.y - py_;
                let d2 = dx * dx + dy * dy;
                if d2 < 144.0 && d2 > 1e-3 {
                    let d = d2.sqrt();
                    ax += dx / d * 420.0;
                    ay += dy / d * 420.0;
                }
            }
            b.px = b.x;
            b.py = b.y;
            b.vx += ax * dt;
            b.vy += ay * dt;
            // speed limits
            let s = (b.vx * b.vx + b.vy * b.vy).sqrt();
            if s > MAX_SPEED {
                b.vx *= MAX_SPEED / s;
                b.vy *= MAX_SPEED / s;
            } else if s < MIN_SPEED && s > 1e-3 {
                b.vx *= MIN_SPEED / s;
                b.vy *= MIN_SPEED / s;
            }
            b.x = (b.x + b.vx * dt).rem_euclid(w);
            b.y = (b.y + b.vy * dt).rem_euclid(h);
        }
        self.prev = prev;
    }

}

impl Scene for Boids {
    fn name(&self) -> &'static str {
        "boids"
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
        let t = self.t;
        self.step(dt);

        // murmuration event scheduling
        self.next_swirl -= dt;
        if self.next_swirl <= 0.0 {
            self.swirl = 3.5;
            self.next_swirl = self.rng.random_range(18.0..32.0);
        }
        self.swirl = (self.swirl - dt).max(0.0);
        // flock-wide shimmer while the swirl winds up
        let shimmer = 1.0 + ease_smooth((self.swirl / 3.5).min(1.0)) * 0.3;

        // predator: telegraph at the edge, then sweep through
        self.next_predator -= dt;
        if self.next_predator <= 0.0 && self.predator.is_none() && self.pred_pending.is_none() {
            self.pred_pending = Some((
                self.rng.random::<bool>(),
                self.rng.random_range(0.0..h as f32),
                1.4,
            ));
            self.next_predator = self.rng.random_range(10.0..18.0);
        }
        if let Some((from_left, py, left)) = &mut self.pred_pending {
            *left -= dt;
            if *left <= 0.0 {
                let from_left = *from_left;
                self.predator = Some((
                    if from_left { -10.0 } else { w as f32 + 10.0 },
                    *py,
                    if from_left { 55.0 } else { -55.0 },
                    self.rng.random_range(-8.0..8.0),
                ));
                self.pred_pending = None;
            }
        }

        canvas.clear((0, 0, 0));

        // background plane: distant specks drifting and breathing, near-black
        for (i, s) in self.specks.iter().enumerate() {
            let tw = 0.7 + 0.3 * (t * 0.6 + s.phase).sin();
            let sx = (s.x + t * s.drift).rem_euclid(w as f32);
            let tint = if i % 2 == 0 { self.c_a } else { self.c_b };
            canvas.set_f(sx, s.y, scale(tint, s.mag * tw));
        }

        // midground: the flock
        for b in &self.boids {
            let mut base = lerp(self.c_a, self.c_b, b.tint);
            let mut boost = 1.0f32;
            // red rim light when the predator is close
            if let Some((px_, py_, _, _)) = self.predator {
                let dx = b.x - px_;
                let dy = b.y - py_;
                let d2 = dx * dx + dy * dy;
                if d2 < 100.0 {
                    let f = 1.0 - d2.sqrt() / 10.0;
                    base = lerp(base, (255, 70, 60), f * 0.6);
                    boost += f * 0.5;
                }
            }
            // velocity-fading trail: longer line from previous position,
            // additive so crossing streams accumulate light
            if (b.x - b.px).abs() < w as f32 / 2.0 && (b.y - b.py).abs() < h as f32 / 2.0 {
                for i in 1..=4 {
                    let tf = i as f32 / 5.0;
                    let tx = b.px + (b.x - b.px) * tf;
                    let ty = b.py + (b.y - b.py) * tf;
                    dot(canvas, tx, ty, base, 0.35 * (1.0 - tf) * shimmer * boost);
                }
            }
            // heading-oriented body: nose + two wing pixels
            let s = (b.vx * b.vx + b.vy * b.vy).sqrt().max(1e-3);
            let (dx, dy) = (b.vx / s, b.vy / s);
            let (pxv, pyv) = (-dy, dx); // perpendicular
            let core = lerp(base, (255, 255, 255), 0.25);
            dot(canvas, b.x, b.y, core, shimmer * boost.min(1.2));
            dot(
                canvas,
                b.x - dx * 1.4 + pxv,
                b.y - dy * 1.4 + pyv,
                base,
                0.7 * shimmer * boost.min(1.4),
            );
            dot(
                canvas,
                b.x - dx * 1.4 - pxv,
                b.y - dy * 1.4 - pyv,
                base,
                0.7 * shimmer * boost.min(1.4),
            );
            // foreground accents: a few leaders carry a soft glow
            if b.leader {
                glow_f(canvas, b.x, b.y, 1.0, base, 0.18 * shimmer);
            }
        }

        // anticipation: a dim red flicker at the entry edge before the sweep
        if let Some((from_left, py, left)) = &self.pred_pending {
            let ex = if *from_left { 0 } else { w as i32 - 1 };
            let blink = 0.5 + 0.5 * (t * 9.0).sin();
            let c = scale((255, 55, 45), (0.12 + 0.2 * blink) * (1.4 - *left).min(1.0));
            let iy = *py as i32;
            canvas.add(ex, iy, c);
            canvas.add(ex, iy - 1, scale(c, 0.6));
            canvas.add(ex, iy + 1, scale(c, 0.6));
        }

        // the predator: a red presence with a soft glow
        if let Some((px, py, vx, vy)) = &mut self.predator {
            *px += *vx * dt;
            *py += *vy * dt + (*px * 0.05).sin() * 20.0 * dt;
            if *px < -15.0 || *px > w as f32 + 15.0 {
                self.predator = None;
            } else {
                glow_f(canvas, *px, *py, 2.0, (255, 60, 50), 0.35);
                dot(canvas, *px, *py, (255, 55, 45), 1.0);
                dot(canvas, *px + 1.0, *py, (90, 20, 15), 1.0);
                dot(canvas, *px - 1.0, *py, (90, 20, 15), 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn boids_stay_in_bounds_over_500_frames() {
        let mut b = Boids::new(StdRng::seed_from_u64(11), None, Detail::Medium);
        let mut c = Canvas::new(80, 40);
        for _ in 0..500 {
            b.update(1.0 / 30.0, &mut c);
        }
        assert!(!b.boids.is_empty());
        for bd in &b.boids {
            assert!(bd.x >= 0.0 && bd.x < 80.0, "x out of bounds: {}", bd.x);
            assert!(bd.y >= 0.0 && bd.y < 40.0, "y out of bounds: {}", bd.y);
            let s = (bd.vx * bd.vx + bd.vy * bd.vy).sqrt();
            assert!(s <= MAX_SPEED + 1e-3, "speed {s} exceeds limit");
        }
    }
}
