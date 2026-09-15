//! Koi pond from above: dark teal water with expanding ripple rings,
//! faint lily pads, and koi gliding in smooth wandering paths with
//! orange/white/spotted bodies and a subtle tail wiggle.

use super::{Detail, Scene};
use crate::physics;
use crate::canvas::{density_for, ease_smooth, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

struct Koi {
    x: f32,
    y: f32,
    heading: f32,
    turn_vel: f32,
    speed: f32,
    /// eased 0..1 feed-dart envelope (no instant speed pop)
    dart: f32,
    /// feeding target (x, y, time left) when darting up to feed
    feeding: Option<(f32, f32, f32)>,
    turn_a: f32,
    turn_b: f32,
    phase: f32,
    pattern: usize, // 0 orange, 1 white-orange, 2 spotted
    len: f32,
    wake_t: f32,
}

struct Ripple {
    x: f32,
    y: f32,
    r: f32,
    max_r: f32,
}

/// A food pellet floating on the surface: the anticipation beat before a
/// koi darts up and gulps it.
struct Pellet {
    x: f32,
    y: f32,
    age: f32,
    /// set once a koi has been dispatched, so only one is sent
    sent: bool,
}

/// Seconds a pellet sits on the surface before a koi notices it.
const PELLET_WAIT: f32 = 1.1;
/// A pellet nobody reaches dissolves after this long.
const PELLET_LIFE: f32 = 9.0;

struct Pad {
    x: f32,
    y: f32,
    r: f32,
}

struct Bubble {
    x: f32,
    y: f32,
    vy: f32,
}

pub struct KoiPond {
    rng: StdRng,
    detail: Detail,
    water: (u8, u8, u8),
    koi: Vec<Koi>,
    ripples: Vec<Ripple>,
    bubbles: Vec<Bubble>,
    pads: Vec<Pad>,
    pellet: Option<Pellet>,
    /// scratch buffer for gulp positions, reused across frames
    feed_events: Vec<(f32, f32)>,
    flower_pad: usize,
    next_feed: f32,
    dragonfly: Option<(f32, f32, f32)>,
    next_dragonfly: f32,
    turtle: Option<(f32, f32, f32)>,
    next_turtle: f32,
    next_ripple: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl KoiPond {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let water = match theme {
            Some("ink") => (8, 10, 16),
            Some("garden") => (10, 40, 30),
            Some("pond-blue") => (10, 35, 55),
            Some("midnight") => (4, 6, 14),
            _ => (6, 30, 34), // teal
        };
        KoiPond {
            rng,
            detail,
            water,
            koi: Vec::new(),
            ripples: Vec::new(),
            bubbles: Vec::new(),
            pads: Vec::new(),
            pellet: None,
            feed_events: Vec::new(),
            flower_pad: 0,
            next_feed: 6.0,
            dragonfly: None,
            next_dragonfly: 14.0,
            turtle: None,
            next_turtle: 12.0,
            next_ripple: 1.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        // size factor: keeps koi and pads readable at 40x12, fuller at 400x200
        let sf = (w.min(h) as f32 / 60.0).clamp(0.45, 2.0);
        let n = self.detail.scale((w * h / 1500) as f32 * 2.0 * density_for(w, h), 3).clamp(3, 18);
        self.koi = (0..n)
            .map(|_| Koi {
                x: self.rng.random_range(w as f32 * 0.15..w as f32 * 0.85),
                y: self.rng.random_range(h as f32 * 0.15..h as f32 * 0.85),
                heading: self.rng.random_range(0.0..std::f32::consts::TAU),
                turn_vel: 0.0,
                speed: self.rng.random_range(5.0..10.0),
                dart: 0.0,
                feeding: None,
                turn_a: self.rng.random_range(0.4..1.1),
                turn_b: self.rng.random_range(0.2..0.7),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                pattern: self.rng.random_range(0..4),
                len: self.rng.random_range(9.0..15.0) * sf,
                wake_t: self.rng.random_range(0.2..1.0),
            })
            .collect();
        self.pads = (0..(w * h / 2500).clamp(2, 5))
            .map(|_| Pad {
                x: self.rng.random_range(w as f32 * 0.1..w as f32 * 0.9),
                y: self.rng.random_range(h as f32 * 0.1..h as f32 * 0.9),
                r: self.rng.random_range(2.0..4.5) * sf,
            })
            .collect();
        self.flower_pad = self.rng.random_range(0..self.pads.len().max(1));
    }

    /// Body color at body-coordinate (along 0=head..len=tail, lateral) per
    /// breed: kohaku (red-on-white), sanke (red+black-on-white), showa
    /// (black-based), ogon (solid gold).
    fn body_color(pattern: usize, along: i32, lat: i32, seed: usize) -> (u8, u8, u8) {
        let red = (215, 70, 30);
        let white = (238, 233, 218);
        let black = (28, 24, 22);
        let gold = (230, 160, 40);
        // patch fields: two independent hash waves per fish
        let p1 = ((along * 5 + lat * 3 + seed as i32 * 11) % 13).unsigned_abs() as i32;
        let p2 = ((along * 3 - lat * 5 + seed as i32 * 7) % 11).unsigned_abs() as i32;
        match pattern {
            1 => {
                // kohaku: white with red patches
                if p1 < 5 { red } else { white }
            }
            2 => {
                // sanke: white with red patches AND black speckles
                if p2 < 2 {
                    black
                } else if p1 < 5 {
                    red
                } else {
                    white
                }
            }
            3 => {
                // showa: black base with red and white patches
                if p1 < 3 {
                    red
                } else if p2 < 3 {
                    white
                } else {
                    black
                }
            }
            _ => {
                // ogon: solid metallic gold with scale shimmer
                let shimmer = 0.9 + (along + lat + seed as i32).rem_euclid(3) as f32 * 0.05;
                scale(gold, shimmer)
            }
        }
    }
}

impl Scene for KoiPond {
    fn name(&self) -> &'static str {
        "koi"
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

        // water: dark teal with slow caustic light dapple
        for y in 0..h {
            for x in 0..w {
                let s = (x as f32 * 0.05 + t * 0.3).sin() * (y as f32 * 0.07 - t * 0.2).cos();
                let dap = ((x as f32 * 0.11 + t * 0.5).sin()
                    * (y as f32 * 0.13 - t * 0.35).cos())
                .max(0.0);
                canvas.set(
                    x as i32,
                    y as i32,
                    lerp(
                        scale(self.water, 1.0 + s * 0.12),
                        scale(self.water, 1.6),
                        dap * dap * 0.35,
                    ),
                );
            }
        }

        // lily pads with a notch
        for p in &self.pads {
            let r = p.r as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d2 = dx * dx + dy * dy;
                    if d2 as f32 <= p.r * p.r {
                        // notch wedge
                        if dx > 0 && dy.abs() as f32 <= dx as f32 * 0.4 {
                            continue;
                        }
                        let shade = 1.0 - (d2 as f32).sqrt() / (p.r * 2.2);
                        canvas.set(
                            p.x as i32 + dx,
                            p.y as i32 + dy,
                            scale((24, 68, 40), shade),
                        );
                    }
                }
            }
        }

        // ambient ripples
        self.next_ripple -= dt;
        if self.next_ripple <= 0.0 {
            self.ripples.push(Ripple {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                r: 0.5,
                max_r: self.rng.random_range(4.0..9.0),
            });
            self.next_ripple = self.rng.random_range(0.6..1.8);
        }

        // feeding, in three beats: a pellet lands (anticipation), the nearest
        // koi notices and darts up (payoff), the gulp bursts ripples/bubbles.
        self.next_feed -= dt;
        if self.next_feed <= 0.0 && self.pellet.is_none() && !self.koi.is_empty() {
            self.next_feed = self.rng.random_range(6.0..12.0);
            let (fx, fy) = (
                self.rng.random_range(w as f32 * 0.2..w as f32 * 0.8),
                self.rng.random_range(h as f32 * 0.15..h as f32 * 0.5),
            );
            // the landing itself makes a small ring — the cue that something hit
            self.ripples.push(Ripple {
                x: fx,
                y: fy,
                r: 0.5,
                max_r: 3.5,
            });
            self.pellet = Some(Pellet {
                x: fx,
                y: fy,
                age: 0.0,
                sent: false,
            });
        }
        // after the anticipation beat, the *nearest* koi is dispatched: a fish
        // noticing reads far better than a random one being teleported a target
        if let Some(p) = &mut self.pellet {
            p.age += dt;
            if !p.sent && p.age >= PELLET_WAIT && !self.koi.is_empty() {
                p.sent = true;
                let (px, py) = (p.x, p.y);
                let ki = self
                    .koi
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let da = (a.x - px).powi(2) + (a.y - py).powi(2);
                        let db = (b.x - px).powi(2) + (b.y - py).powi(2);
                        da.total_cmp(&db)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                self.koi[ki].feeding = Some((px, py, 3.0));
            }
            if p.age > PELLET_LIFE {
                self.pellet = None;
            }
        }

        // reuse the scratch buffer across frames instead of allocating a fresh
        // Vec every update, which is what the field was always for
        let mut feed_events = std::mem::take(&mut self.feed_events);
        feed_events.clear();
        // koi movement + wakes
        for k in self.koi.iter_mut() {
            // smooth wandering with inertia: target turn rate, damped
            let target_turn =
                ((t * k.turn_a + k.phase).sin() + 0.6 * (t * k.turn_b).cos()) * 0.9;
            k.turn_vel += physics::spring_damper(k.turn_vel, k.turn_vel, target_turn, 6.0, 4.0) * dt;
            k.heading += k.turn_vel * dt;
            // steer back toward center near edges
            let margin = 6.0;
            let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
            if k.x < margin || k.x > w as f32 - margin || k.y < margin || k.y > h as f32 - margin {
                let to_center = (cy - k.y).atan2(cx - k.x);
                let mut diff = to_center - k.heading;
                while diff > std::f32::consts::PI {
                    diff -= std::f32::consts::TAU;
                }
                while diff < -std::f32::consts::PI {
                    diff += std::f32::consts::TAU;
                }
                k.heading += diff * 2.5 * dt;
            }
            let mut speed = k.speed;
            if let Some((fx, fy, left)) = &mut k.feeding {
                *left -= dt;
                let want = (*fy - k.y).atan2(*fx - k.x);
                let mut diff = want - k.heading;
                while diff > std::f32::consts::PI {
                    diff -= std::f32::consts::TAU;
                }
                while diff < -std::f32::consts::PI {
                    diff += std::f32::consts::TAU;
                }
                k.heading += diff * 5.0 * dt;
                // ramp into the dart rather than snapping to 2.4x
                k.dart = (k.dart + dt * 4.0).min(1.0);
                let dist = ((*fx - k.x).powi(2) + (*fy - k.y).powi(2)).sqrt();
                if dist < 2.0 || *left <= 0.0 {
                    k.feeding = None;
                    feed_events.push((k.x, k.y));
                }
            } else {
                // and coast back down afterwards, so the gulp has a settle
                k.dart = (k.dart - dt * 1.6).max(0.0);
            }
            speed *= 1.0 + 1.4 * ease_smooth(k.dart);
            k.x = (k.x + k.heading.cos() * speed * dt).clamp(1.0, w as f32 - 1.0);
            k.y = (k.y + k.heading.sin() * speed * dt).clamp(1.0, h as f32 - 1.0);
            k.wake_t -= dt;
            if k.wake_t <= 0.0 {
                self.ripples.push(Ripple {
                    x: k.x,
                    y: k.y,
                    r: 0.5,
                    max_r: 3.0,
                });
                k.wake_t = self.rng.random_range(0.5..1.2);
            }
        }

        if !feed_events.is_empty() {
            // the pellet is gone the moment a mouth reaches it
            self.pellet = None;
        }
        for (fx, fy) in feed_events.drain(..) {
            // mouth-break: pale ring where the mouth breaks the surface
            for i in 0..10 {
                let a = i as f32 / 10.0 * std::f32::consts::TAU;
                canvas.set_f(fx + a.cos() * 2.0, fy + a.sin() * 1.2, (200, 215, 210));
            }
            for rr in 0..3 {
                self.ripples.push(Ripple {
                    x: fx,
                    y: fy,
                    r: 0.5 + rr as f32 * 1.5,
                    max_r: 6.0 + rr as f32 * 2.0,
                });
            }
            for i in 0..6 {
                let a = i as f32 / 6.0 * std::f32::consts::TAU;
                self.bubbles.push(Bubble {
                    x: fx + a.cos() * 1.5,
                    y: fy + a.sin() * 1.5,
                    vy: self.rng.random_range(3.0..6.0),
                });
            }
        }
        // bubbles rising from feeding bursts
        self.bubbles.retain_mut(|b| {
            b.y -= b.vy * dt;
            if b.y < 1.0 {
                return false;
            }
            canvas.set_f(b.x, b.y, (150, 190, 200));
            true
        });

        // lily flower drifting gently on one pad
        if let Some(p) = self.pads.get(self.flower_pad) {
            let fx = p.x + (t * 0.3).sin() * 1.2;
            let fy = p.y + (t * 0.23).cos() * 0.8;
            canvas.set_f(fx, fy, (245, 190, 205));
            canvas.set_f(fx + 1.0, fy, (240, 175, 195));
            canvas.set_f(fx, fy - 1.0, (255, 230, 130));
        }

        // a turtle cruises through slowly; nearby koi give way
        self.next_turtle -= dt;
        if self.next_turtle <= 0.0 && self.turtle.is_none() {
            let from_left = self.rng.random::<bool>();
            self.turtle = Some((
                if from_left { -10.0 } else { w as f32 + 10.0 },
                self.rng.random_range(h as f32 * 0.3..h as f32 * 0.7),
                if from_left { 4.0 } else { -4.0 },
            ));
            self.next_turtle = self.rng.random_range(25.0..45.0);
        }
        if let Some((tx, ty, tvx)) = &mut self.turtle {
            *tx += *tvx * dt;
            if *tx < -12.0 || *tx > w as f32 + 12.0 {
                self.turtle = None;
            } else {
                // koi yield: nudge away from the turtle's path
                for k in self.koi.iter_mut() {
                    let d = ((k.x - *tx).powi(2) + (k.y - *ty).powi(2)).sqrt();
                    if d < 8.0 {
                        let away = (k.y - *ty).signum();
                        k.y += away * dt * 6.0;
                    }
                }
                // shell dome + head + flippers
                let shell = (40, 60, 35);
                for dy in -2..=1 {
                    for dx in -4..=4 {
                        if dx * dx + dy * dy * 3 <= 16 {
                            canvas.set(*tx as i32 + dx, *ty as i32 + dy, shell);
                        }
                    }
                }
                let dir = tvx.signum() as i32;
                canvas.set(*tx as i32 + dir * 5, *ty as i32, (55, 80, 45)); // head
                let paddle = ((t * 2.5).sin() * 1.0) as i32;
                canvas.set(*tx as i32 - 2, *ty as i32 - 3 - paddle, shell);
                canvas.set(*tx as i32 + 2, *ty as i32 + 2 + paddle, shell);
            }
        }

        // dragonfly: a fast dart across the surface with a ripple touch
        self.next_dragonfly -= dt;
        if self.next_dragonfly <= 0.0 && self.dragonfly.is_none() {
            self.dragonfly = Some((
                -4.0,
                self.rng.random_range(h as f32 * 0.2..h as f32 * 0.5),
                self.rng.random_range(35.0..55.0),
            ));
            self.next_dragonfly = self.rng.random_range(16.0..30.0);
        }
        if let Some((dx, dy, dvx)) = &mut self.dragonfly {
            *dx += *dvx * dt;
            if *dx > w as f32 + 4.0 {
                // touch the water mid-flight: one ring
                self.ripples.push(Ripple {
                    x: *dx - *dvx * 2.0,
                    y: *dy,
                    r: 0.5,
                    max_r: 3.5,
                });
                self.dragonfly = None;
            } else {
                let bob = (t * 14.0).sin() * 0.5;
                canvas.set_f(*dx, *dy + bob, (40, 60, 70));
                canvas.set_f(*dx - 1.0, *dy + bob - 0.5, (60, 90, 100));
            }
        }

        // ripples (drawn under koi)
        self.ripples.retain_mut(|r| {
            r.r += 9.0 * dt;
            if r.r >= r.max_r {
                return false;
            }
            let alpha = 1.0 - r.r / r.max_r;
            // ring outline via midpoint-ish sampling
            let steps = (r.r * 6.0) as i32 + 8;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                let px = r.x + a.cos() * r.r;
                let py = r.y + a.sin() * r.r * 0.6; // squashed: perspective
                canvas.add(px as i32, py as i32, scale((50, 90, 90), alpha * 0.5));
            }
            true
        });

        // hand the scratch buffer back with its capacity intact
        self.feed_events = feed_events;

        // the pellet itself: a pale speck bobbing on the surface, fading in on
        // landing and dimming as it goes stale
        if let Some(p) = &self.pellet {
            let fade = ease_smooth(p.age / 0.35)
                * (1.0 - ease_smooth((p.age - (PELLET_LIFE - 2.0)) / 2.0));
            let bob = (t * 2.3).sin() * 0.4;
            canvas.set_f(p.x, p.y + bob, scale((214, 198, 150), fade));
            canvas.add(
                p.x as i32,
                (p.y + bob) as i32 - 1,
                scale((90, 84, 62), fade * 0.5),
            );
        }

        // koi bodies: shadow first, then anatomy
        for (ki, k) in self.koi.iter().enumerate() {
            let (hx, hy) = (k.heading.cos(), k.heading.sin());
            let (px, py) = (-hy, hx); // perpendicular
            let wiggle = (t * 5.0 + k.phase).sin();
            let half = (k.len / 2.0) as i32;
            // soft shadow offset below-right by "depth"
            let shadow = scale(self.water, 0.55);
            for along in -half..=half {
                let u = (along as f32 + k.len / 2.0) / k.len;
                let width = (k.len * 0.16 * (u * std::f32::consts::PI * 0.85).sin()).max(0.0);
                for lat in -(width as i32)..=(width as i32) {
                    canvas.set_f(
                        k.x + hx * along as f32 + px * lat as f32 + 1.5,
                        k.y + hy * along as f32 + py * lat as f32 + 2.0,
                        shadow,
                    );
                }
            }
            // wake: fading dimples behind the tail
            for wk in 1..=3 {
                let wx = k.x - hx * (k.len / 2.0 + wk as f32 * 2.0);
                let wy = k.y - hy * (k.len / 2.0 + wk as f32 * 2.0);
                canvas.add(wx as i32, wy as i32, scale((80, 110, 110), 0.12 / wk as f32));
            }
            for along in -half..=half {
                let u = (along as f32 + k.len / 2.0) / k.len; // 0 tail → 1 head
                // anatomy: tapered tail, broad mid-shoulder, rounded head
                let width = (k.len * 0.16 * (u * std::f32::consts::PI * 0.85).sin()).max(0.0);
                // tail undulation runs down the rear third
                let sway = if u < 0.35 {
                    wiggle * (0.35 - u) * 3.0
                } else {
                    0.0
                };
                for lat in -(width as i32)..=(width as i32) {
                    let bx = k.x + hx * along as f32 + px * (lat as f32 + sway);
                    let by = k.y + hy * along as f32 + py * (lat as f32 + sway);
                    let c = Self::body_color(k.pattern, along + half, lat, ki);
                    canvas.set_f(bx, by, scale(c, 0.92));
                }
                // side fins flapping at the shoulder
                if (0.68..0.78).contains(&u) {
                    let flap = ((t * 3.0 + k.phase).sin() * 1.5) as i32;
                    for side in [-1i32, 1] {
                        let fx = k.x + hx * along as f32 + px * (width + 1.0) * side as f32;
                        let fy = k.y + hy * along as f32 + py * (width + 1.0) * side as f32;
                        canvas.set_f(
                            fx,
                            fy + flap as f32 * side as f32 * 0.5,
                            scale(Self::body_color(k.pattern, along + half, 0, ki), 0.7),
                        );
                    }
                }
            }
            // eyes on the head
            let eye_u = k.len / 2.0 - 1.5;
            for side in [-1i32, 1] {
                canvas.set_f(
                    k.x + hx * eye_u + px * side as f32 * 1.5,
                    k.y + hy * eye_u + py * side as f32 * 1.5,
                    (20, 16, 14),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn koi_stay_in_bounds() {
        let mut p = KoiPond::new(StdRng::seed_from_u64(61), None, Detail::Medium);
        let mut c = Canvas::new(80, 40);
        for _ in 0..1000 {
            p.update(1.0 / 30.0, &mut c);
        }
        assert!(!p.koi.is_empty());
        for k in &p.koi {
            assert!(k.x >= 0.0 && k.x <= 80.0, "koi x {} out of bounds", k.x);
            assert!(k.y >= 0.0 && k.y <= 40.0, "koi y {} out of bounds", k.y);
        }
    }
}

#[cfg(test)]
mod breed_tests {
    use super::*;

    #[test]
    fn breeds_have_distinct_pattern_maps() {
        // each breed produces a different color signature over the body
        let sig = |pattern: usize| {
            let mut counts = std::collections::HashMap::new();
            for along in 0..20 {
                for lat in -4..=4 {
                    *counts
                        .entry(KoiPond::body_color(pattern, along, lat, 0))
                        .or_insert(0usize) += 1;
                }
            }
            counts
        };
        let (kohaku, sanke, showa, ogon) = (sig(1), sig(2), sig(3), sig(0));
        assert_ne!(kohaku, sanke);
        assert_ne!(sanke, showa);
        assert_ne!(showa, ogon);
        // ogon is essentially solid gold (small shimmer set, all golden)
        assert!(ogon.len() <= 3, "ogon should be near-solid gold");
        assert!(ogon.keys().all(|&(r, g, b)| r > g && g > b));
        // kohaku is exactly two colors (red + white)
        assert_eq!(kohaku.len(), 2);
        // sanke has all three
        assert_eq!(sanke.len(), 3);
    }
}
