//! Ripple: still black water — raindrop rings, drifting leaves, night breeze.
//! Top-down pond on pure black: raindrops spawn 2-3 concentric expanding
//! rings whose additive rims brighten where they cross; a few leaves drift
//! with eased wind steering and faint V-wakes; every ~15-25s a breeze passes
//! as a diagonal shimmer band with a brief heavier drizzle burst.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_out, ease_smooth, glow, lerp, scale, Canvas};
use crate::physics;
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// Cap on live rings so drizzle bursts can't grow the vec without bound.
const MAX_RINGS: usize = 200;

struct Ring {
    x: f32,
    y: f32,
    age: f32,
    delay: f32,
    max_r: f32,
    dur: f32,
    bright: f32,
}

struct Flash {
    x: f32,
    y: f32,
    age: f32,
}

struct Leaf {
    x: f32,
    y: f32,
    heading: f32,
    turn_vel: f32,
    speed: f32,
    phase: f32,
    turn_a: f32,
    turn_b: f32,
    size: f32,
    tint: f32,
    /// seconds since spawn; drives the grow-in envelope
    age: f32,
}

pub struct Ripple {
    rng: StdRng,
    detail: Detail,
    accent: (u8, u8, u8),
    water_tint: (u8, u8, u8),
    shimmer_c: (u8, u8, u8),
    leaf_c: (u8, u8, u8),
    rings: Vec<Ring>,
    flashes: Vec<Flash>,
    leaves: Vec<Leaf>,
    drop_timer: f32,
    next_breeze: f32,
    /// <0 inactive, else seconds since the breeze started
    breeze_t: f32,
    breeze_dur: f32,
    breeze_dir: (f32, f32),
    t: f32,
    w: usize,
    h: usize,
}

impl Ripple {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (accent, leaf_c) = match theme {
            Some("silver") => ((205, 214, 228), (112, 114, 96)),
            Some("ink") => ((96, 112, 172), (62, 70, 58)),
            _ => ((88, 214, 190), (88, 104, 58)), // teal
        };
        Ripple {
            rng,
            detail,
            accent,
            water_tint: scale(accent, 0.05),
            shimmer_c: lerp(accent, (255, 255, 255), 0.5),
            leaf_c,
            rings: Vec::new(),
            flashes: Vec::new(),
            leaves: Vec::new(),
            drop_timer: 0.5,
            next_breeze: 8.0,
            breeze_t: -1.0,
            breeze_dur: 6.0,
            breeze_dir: (FRAC_PI_4.cos(), FRAC_PI_4.sin()),
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.rings.clear();
        self.flashes.clear();
        self.leaves.clear();
        if w == 0 || h == 0 {
            return;
        }
        // leaves scale with area so big panes don't feel empty
        let n = self
            .detail
            .scale((w * h / 2200) as f32 * density_for(w, h), 2)
            .clamp(2, 12);
        let size_k = (w.min(h) as f32 / 50.0).clamp(0.6, 2.0);
        self.leaves = (0..n)
            .map(|_| Leaf {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                heading: self.rng.random_range(0.0..TAU),
                turn_vel: 0.0,
                speed: self.rng.random_range(2.0..4.5),
                phase: self.rng.random_range(0.0..TAU),
                turn_a: self.rng.random_range(0.3..0.8),
                turn_b: self.rng.random_range(0.15..0.5),
                size: self.rng.random_range(1.5..2.5) * size_k,
                tint: self.rng.random::<f32>(),
                // stagger grow-in so resize doesn't pop a full canopy at once
                age: -self.rng.random_range(0.0..2.5),
            })
            .collect();
    }

    /// A raindrop lands: brief impact flash plus 2-3 staggered rings.
    fn spawn_drop(&mut self, w: usize, h: usize) {
        let x = self.rng.random_range(2.0..(w as f32 - 2.0).max(3.0));
        let y = self.rng.random_range(2.0..(h as f32 - 2.0).max(3.0));
        self.flashes.push(Flash { x, y, age: 0.0 });
        let size_k = (w.min(h) as f32 / 50.0).clamp(0.6, 2.0);
        let n = self.rng.random_range(2..=3);
        for i in 0..n {
            let max_r = self.rng.random_range(3.5..8.0) * size_k + i as f32 * 2.5;
            self.rings.push(Ring {
                x,
                y,
                age: 0.0,
                delay: i as f32 * self.rng.random_range(0.12..0.3),
                max_r,
                dur: 0.7 + max_r * 0.14,
                bright: self.rng.random_range(0.6..1.0),
            });
        }
        if self.rings.len() > MAX_RINGS {
            let excess = self.rings.len() - MAX_RINGS;
            self.rings.drain(..excess);
        }
    }
}

impl Scene for Ripple {
    fn name(&self) -> &'static str {
        "ripple"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        if w == 0 || h == 0 {
            return;
        }
        // cap dt so fast-forward can't blow up steering or ring growth
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let t = self.t;

        // --- breeze event: anticipation -> sweep -> decay ---
        self.next_breeze -= dt;
        if self.next_breeze <= 0.0 && self.breeze_t < 0.0 {
            self.breeze_t = 0.0;
            self.breeze_dur = self.rng.random_range(5.0..7.0);
            let base = self.rng.random_range(0..4) as f32 * FRAC_PI_2 + FRAC_PI_4;
            let ang = base + self.rng.random_range(-0.25..0.25);
            self.breeze_dir = (ang.cos(), ang.sin());
            self.next_breeze = self.rng.random_range(15.0..25.0);
        }
        let mut breeze_env = 0.0;
        let mut breeze_p = 0.0;
        if self.breeze_t >= 0.0 {
            self.breeze_t += dt;
            let bt = self.breeze_t;
            if bt >= self.breeze_dur {
                self.breeze_t = -1.0;
            } else {
                let ramp = ease_smooth(bt / 1.2);
                let fall = 1.0 - ease_smooth((bt - (self.breeze_dur - 1.6)) / 1.6);
                breeze_env = ramp * fall;
                let sweep =
                    ease_smooth(((bt - 0.8) / (self.breeze_dur - 2.0)).clamp(0.0, 1.0));
                let (dx, dy) = self.breeze_dir;
                let (wf, hf) = (w as f32, h as f32);
                let s_min = (if dx < 0.0 { wf * dx } else { 0.0 })
                    + (if dy < 0.0 { hf * dy } else { 0.0 });
                let s_max = (if dx > 0.0 { wf * dx } else { 0.0 })
                    + (if dy > 0.0 { hf * dy } else { 0.0 });
                breeze_p = s_min - 8.0 + (s_max - s_min + 16.0) * sweep;
            }
        }

        // --- raindrops: steady patter, heavier during the breeze ---
        self.drop_timer -= dt;
        if self.drop_timer <= 0.0 {
            self.spawn_drop(w, h);
            self.drop_timer = if breeze_env > 0.25 {
                self.rng.random_range(0.05..0.16)
            } else {
                self.rng.random_range(0.45..1.1) / density_for(w, h)
            };
        }

        // --- draw: pure black water with a whisper of moving dapple ---
        canvas.clear((0, 0, 0));
        for y in 0..h {
            for x in 0..w {
                let dap = ((x as f32 * 0.09 + t * 0.35).sin()
                    * (y as f32 * 0.11 - t * 0.28).cos())
                .max(0.0);
                let mut c = scale(self.water_tint, dap * dap);
                if breeze_env > 0.0 {
                    // diagonal shimmer band sweeping the surface
                    let s = x as f32 * self.breeze_dir.0 + y as f32 * self.breeze_dir.1;
                    let g = ((s - breeze_p) / 5.0).powi(2);
                    let shim = scale(self.shimmer_c, (-g).exp() * breeze_env * 0.22);
                    c = (
                        c.0.saturating_add(shim.0),
                        c.1.saturating_add(shim.1),
                        c.2.saturating_add(shim.2),
                    );
                }
                canvas.set(x as i32, y as i32, c);
            }
        }

        // impact flashes: bright pinprick that dies in a third of a second,
        // bleeding a soft glow onto the water so impacts light their patch
        self.flashes.retain_mut(|f| {
            f.age += dt;
            let life = 0.35;
            if f.age >= life {
                return false;
            }
            let a = 1.0 - f.age / life;
            let fc = lerp(self.accent, (255, 255, 255), 0.5);
            glow(canvas, f.x as i32, f.y as i32, 3, fc, a * 0.35);
            canvas.add(f.x as i32, f.y as i32, scale(fc, a * 0.8));
            canvas.add(f.x as i32 + 1, f.y as i32, scale(fc, a * 0.3));
            canvas.add(f.x as i32 - 1, f.y as i32, scale(fc, a * 0.3));
            canvas.add(f.x as i32, f.y as i32 + 1, scale(fc, a * 0.3));
            canvas.add(f.x as i32, f.y as i32 - 1, scale(fc, a * 0.3));
            true
        });

        // rings: eased expansion, fading rims, additive so crossings glint
        let accent = self.accent;
        self.rings.retain_mut(|r| {
            r.age += dt;
            if r.age < r.delay {
                return true;
            }
            let p = ((r.age - r.delay) / r.dur).clamp(0.0, 1.0);
            if p >= 1.0 {
                return false;
            }
            let rad = r.max_r * ease_out(p);
            let alpha = (1.0 - p).powf(1.5) * r.bright;
            // young rings glint toward white, old ones sink into the accent
            let rim = lerp(accent, (255, 255, 255), 0.35 * (1.0 - p));
            let steps = (rad * 7.0) as i32 + 12;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * TAU;
                let px = r.x + a.cos() * rad;
                let py = r.y + a.sin() * rad * 0.55; // squash for cell aspect
                canvas.add(px as i32, py as i32, scale(rim, alpha * 0.45));
            }
            true
        });

        // --- leaves: eased wandering, wind steering, faint V-wakes ---
        for l in self.leaves.iter_mut() {
            l.age += dt;
            let wander =
                ((t * l.turn_a + l.phase).sin() + 0.5 * (t * l.turn_b).cos()) * 0.7;
            let mut target_turn = wander;
            if breeze_env > 0.0 {
                // align downwind as the gust passes through
                let wind_ang = self.breeze_dir.1.atan2(self.breeze_dir.0);
                let mut diff = wind_ang - l.heading;
                while diff > PI {
                    diff -= TAU;
                }
                while diff < -PI {
                    diff += TAU;
                }
                target_turn += diff * 1.6 * breeze_env;
            }
            l.turn_vel +=
                physics::spring_damper(l.turn_vel, l.turn_vel, target_turn, 5.0, 3.5) * dt;
            l.heading += l.turn_vel * dt;
            let speed = l.speed * (1.0 + breeze_env * 2.2);
            l.x = (l.x + l.heading.cos() * speed * dt).rem_euclid(w as f32);
            l.y = (l.y + l.heading.sin() * speed * dt).rem_euclid(h as f32);

            // grow-in envelope: leaves surface gently instead of popping in
            let grow = ease_smooth(l.age / 1.2);
            if grow <= 0.0 {
                continue;
            }

            // V-wake: two fading arms trailing behind the leaf
            let back = l.heading + PI;
            let wake_len = 6;
            for k in 1..=wake_len {
                let f = 1.0 - k as f32 / (wake_len + 1) as f32;
                for side in [-1.0f32, 1.0] {
                    let a = back + side * (0.35 + k as f32 * 0.05);
                    let wx = l.x + a.cos() * k as f32 * 1.3;
                    let wy = l.y + a.sin() * k as f32 * 1.3;
                    canvas.add(
                        wx as i32,
                        wy as i32,
                        scale(accent, 0.07 * f * (0.5 + breeze_env) * grow),
                    );
                }
            }

            // body: small oriented oval with a soft sheen along its length
            let (hx, hy) = (l.heading.cos(), l.heading.sin());
            let (px, py) = (-hy, hx);
            let half = l.size as i32;
            let bob = (t * 1.7 + l.phase).sin() * 0.15;
            for along in -half..=half {
                let u = (along as f32 + l.size) / (l.size * 2.0);
                let width = (l.size * 0.5 * (u * PI).sin()).max(0.0) as i32;
                let shade =
                    (0.7 + 0.3 * (u + bob).clamp(0.0, 1.0)) * (0.85 + l.tint * 0.3) * grow;
                for lat in -width..=width {
                    canvas.set_f(
                        l.x + hx * along as f32 + px * lat as f32,
                        l.y + hy * along as f32 + py * lat as f32,
                        scale(self.leaf_c, shade),
                    );
                }
            }
            // pale tip catching the light
            canvas.add(
                (l.x + hx * l.size) as i32,
                (l.y + hy * l.size) as i32,
                scale(accent, (0.25 + breeze_env * 0.3) * grow),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn checksum(s: &mut Ripple, steps: usize) -> u64 {
        let mut c = Canvas::new(80, 40);
        let mut h = 0xcbf29ce484222325u64;
        for _ in 0..steps {
            s.update(1.0 / 60.0, &mut c);
        }
        for y in 0..40 {
            for x in 0..80 {
                let (r, g, b) = c.get(x, y).color;
                h ^= ((r as u64) << 16) ^ ((g as u64) << 8) ^ b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
        h
    }

    #[test]
    fn same_seed_is_deterministic() {
        let mut a = Ripple::new(StdRng::seed_from_u64(777), None, Detail::Medium);
        let mut b = Ripple::new(StdRng::seed_from_u64(777), None, Detail::Medium);
        assert_eq!(checksum(&mut a, 90), checksum(&mut b, 90));
    }

    #[test]
    fn themes_resolve_and_unknown_falls_back_to_teal() {
        let teal = Ripple::new(StdRng::seed_from_u64(1), None, Detail::Medium).accent;
        let silver = Ripple::new(StdRng::seed_from_u64(1), Some("silver"), Detail::Medium).accent;
        let ink = Ripple::new(StdRng::seed_from_u64(1), Some("ink"), Detail::Medium).accent;
        let bogus = Ripple::new(StdRng::seed_from_u64(1), Some("bogus"), Detail::Medium).accent;
        assert_ne!(teal, silver);
        assert_ne!(teal, ink);
        assert_ne!(silver, ink);
        assert_eq!(teal, bogus);
    }

    #[test]
    fn resize_and_zero_size_do_not_panic() {
        let mut s = Ripple::new(StdRng::seed_from_u64(9), None, Detail::High);
        let mut c = Canvas::new(40, 12);
        for _ in 0..10 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(400, 200);
        for _ in 0..10 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(0, 0);
        s.update(1.0 / 30.0, &mut c);
        assert_eq!(s.leaves.len(), 0);
    }

    #[test]
    fn rings_spawn_and_stay_bounded_under_fast_forward() {
        let mut s = Ripple::new(StdRng::seed_from_u64(31), None, Detail::High);
        let mut c = Canvas::new(120, 60);
        for _ in 0..300 {
            s.update(1.0 / 30.0, &mut c);
        }
        assert!(!s.rings.is_empty(), "raindrops should leave rings");
        // huge dt steps must not explode state
        for _ in 0..200 {
            s.update(5.0, &mut c);
        }
        assert!(s.rings.len() <= MAX_RINGS);
        assert!(s.rings.iter().all(|r| r.x.is_finite() && r.max_r.is_finite()));
        // something is still being painted
        let lit = (0..120)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| c.get(x, y).color != (0, 0, 0))
            .count();
        assert!(lit > 100);
    }
}
