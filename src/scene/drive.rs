//! Drive: first-person night driving — dashboard silhouette, perspective
//! road with scrolling lane markings, oncoming headlights, and side scenery
//! streaking past at parallax speeds. Events: overtake, tunnel squeeze,
//! rain burst on the windshield.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const PERSP: f32 = 2.2;

struct Palette {
    sky_a: (u8, u8, u8),
    sky_b: (u8, u8, u8),
    horizon: (u8, u8, u8),
    road: (u8, u8, u8),
    road_far: (u8, u8, u8),
    dash: (u8, u8, u8),
    head: (u8, u8, u8),
    tail: (u8, u8, u8),
    dash_glow: (u8, u8, u8),
    rain: bool,
    wet: f32,
}

struct SideObj {
    side: f32, // -1 left, +1 right
    d: f32,    // depth 0 horizon .. 1 camera
    kind: u8,  // 0 pole, 1 tree, 2 sign
    h: f32,
}

struct Oncoming {
    lane: f32,
    d: f32,
}

struct RainStreak {
    x: f32,
    y: f32,
    len: f32,
    speed: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Event {
    None,
    Overtake(f32),
    Tunnel(f32),
    RainBurst(f32),
}

pub struct Drive {
    rng: StdRng,
    detail: Detail,
    pal: Palette,
    scroll: f32,
    speed: f32,
    side: Vec<SideObj>,
    oncoming: Vec<Oncoming>,
    rain: Vec<RainStreak>,
    event: Event,
    next_event: f32,
    tunnel_k: f32,
    mirror_glow: f32,
    t: f32,
    w: usize,
    h: usize,
    side_cap: usize,
    on_cap: usize,
}

impl Drive {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let pal = match theme {
            Some("dusk") => Palette {
                sky_a: (8, 6, 18),
                sky_b: (80, 40, 60),
                horizon: (255, 140, 70),
                road: (16, 14, 20),
                road_far: (8, 7, 12),
                dash: (255, 200, 120),
                head: (255, 230, 180),
                tail: (255, 60, 40),
                dash_glow: (255, 160, 90),
                rain: false,
                wet: 0.0,
            },
            Some("rain") => Palette {
                sky_a: (0, 0, 0),
                sky_b: (12, 14, 22),
                horizon: (40, 45, 55),
                road: (14, 16, 22),
                road_far: (6, 7, 10),
                dash: (200, 210, 230),
                head: (255, 255, 255),
                tail: (255, 40, 30),
                dash_glow: (180, 190, 210),
                rain: true,
                wet: 0.55,
            },
            Some("neon") => Palette {
                sky_a: (0, 0, 8),
                sky_b: (20, 8, 40),
                horizon: (255, 40, 180),
                road: (10, 8, 18),
                road_far: (5, 4, 10),
                dash: (80, 255, 255),
                head: (255, 255, 255),
                tail: (255, 50, 120),
                dash_glow: (255, 80, 220),
                rain: false,
                wet: 0.35,
            },
            _ => Palette {
                sky_a: (0, 0, 0),
                sky_b: (8, 10, 18),
                horizon: (30, 35, 45),
                road: (14, 14, 18),
                road_far: (6, 6, 10),
                dash: (220, 220, 200),
                head: (255, 255, 240),
                tail: (255, 50, 40),
                dash_glow: (180, 180, 160),
                rain: false,
                wet: 0.0,
            },
        };
        Drive {
            rng,
            detail,
            pal,
            scroll: 0.0,
            speed: 1.0,
            side: Vec::new(),
            oncoming: Vec::new(),
            rain: Vec::new(),
            event: Event::None,
            next_event: 12.0,
            tunnel_k: 0.0,
            mirror_glow: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
            side_cap: 8,
            on_cap: 4,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.side_cap = self
            .detail
            .scale(10.0 * density_for(w, h), 4)
            .min(24);
        self.on_cap = self.detail.scale(5.0 * density_for(w, h), 2).min(12);
        self.side.clear();
        self.oncoming.clear();
        let n_rain = if self.pal.rain {
            self.detail.scale((w * h / 180) as f32, 8)
        } else {
            self.detail.scale((w * h / 400) as f32, 0)
        };
        self.rain = (0..n_rain)
            .map(|_| RainStreak {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32 * 0.75),
                len: self.rng.random_range(3.0..8.0),
                speed: self.rng.random_range(80.0..160.0),
            })
            .collect();
    }

    fn spawn_side(&mut self) {
        if self.side.len() >= self.side_cap {
            return;
        }
        self.side.push(SideObj {
            side: if self.rng.random::<bool>() { -1.0 } else { 1.0 },
            d: 0.02,
            kind: self.rng.random_range(0..3),
            h: self.rng.random_range(0.4..1.2),
        });
    }

    fn spawn_oncoming(&mut self) {
        if self.oncoming.len() >= self.on_cap {
            return;
        }
        self.oncoming.push(Oncoming {
            lane: self.rng.random_range(-0.35..0.35),
            d: 0.05,
        });
    }
}

impl Scene for Drive {
    fn name(&self) -> &'static str {
        "drive"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        self.t += dt;

        // events
        self.next_event -= dt;
        if self.next_event <= 0.0 && self.event == Event::None {
            self.event = match self.rng.random_range(0..3) {
                0 => Event::Overtake(0.0),
                1 => Event::Tunnel(0.0),
                _ => Event::RainBurst(0.0),
            };
            self.next_event = self.rng.random_range(18.0..28.0);
        }

        let mut speed_mul = 1.0 + 0.08 * (self.t * 0.4).sin();
        let mut rain_boost = 0.0;
        let mut tunnel_walls = 0.0;

        match self.event {
            Event::Overtake(ref mut e) => {
                *e += dt;
                if *e < 1.2 {
                    speed_mul *= 1.0 + ease_smooth(*e / 1.2) * 0.15;
                } else if *e < 3.5 {
                    self.mirror_glow = ease_smooth((*e - 2.0).abs().min(1.0));
                } else if *e >= 5.0 {
                    self.event = Event::None;
                    self.mirror_glow = 0.0;
                }
            }
            Event::Tunnel(ref mut e) => {
                *e += dt;
                if *e < 1.5 {
                    tunnel_walls = ease_smooth(*e / 1.5);
                } else if *e < 4.0 {
                    tunnel_walls = 1.0;
                    speed_mul *= 1.12;
                } else if *e < 5.5 {
                    tunnel_walls = 1.0 - ease_smooth((*e - 4.0) / 1.5);
                } else {
                    self.event = Event::None;
                }
            }
            Event::RainBurst(ref mut e) => {
                *e += dt;
                if *e < 0.8 {
                    rain_boost = ease_smooth(*e / 0.8);
                } else if *e < 3.0 {
                    rain_boost = 1.0;
                } else if *e < 4.5 {
                    rain_boost = 1.0 - ease_smooth((*e - 3.0) / 1.5);
                } else {
                    self.event = Event::None;
                }
            }
            Event::None => {}
        }
        self.tunnel_k = tunnel_walls;

        self.speed = speed_mul;
        self.scroll = (self.scroll + dt * 0.35 * self.speed).fract();

        let hy = h as f32 * 0.38;
        let gh = (h as f32 - hy).max(1.0);
        let dash_top = h as f32 * 0.82;
        let cx = w as f32 / 2.0 + (self.t * 0.07).sin() * w as f32 * 0.008;
        let road_half = w as f32 * 0.42;

        canvas.clear((0, 0, 0));

        // sky
        for y in 0..hy as i32 {
            let t = (y as f32 / hy).powi(2);
            canvas.fill_row(y as usize, lerp(self.pal.sky_a, self.pal.sky_b, t));
        }
        // horizon glow
        let hg = 0.35 + rain_boost * 0.2;
        for x in 0..w {
            canvas.add(x as i32, hy as i32, scale(self.pal.horizon, hg));
            canvas.add(x as i32, hy as i32 - 1, scale(self.pal.horizon, hg * 0.25));
        }

        // road surface
        for y in (hy as i32 + 1)..dash_top as i32 {
            let d = ((y as f32 - hy) / (dash_top - hy)).powf(1.0 / PERSP);
            let wet = self.pal.wet + rain_boost * 0.3;
            let base = lerp(self.pal.road_far, self.pal.road, d);
            for x in 0..w {
                let spec = if wet > 0.0 {
                    let stripe =
                        ((x_wave(x as f32, self.t) + d * 4.0).sin() * 0.5 + 0.5) * wet * d;
                    lerp(base, self.pal.dash_glow, stripe * 0.15)
                } else {
                    base
                };
                let rx = (x as f32 - cx).abs() / road_half;
                if rx <= 1.0 + d * 0.2 {
                    let edge = 1.0 - (rx - 0.85).max(0.0) / 0.35;
                    canvas.set(x as i32, y, scale(spec, edge.max(0.15)));
                }
            }
        }

        // lane dashes
        let n_dash = self.detail.scale(14.0, 8);
        for k in 0..n_dash {
            let p = (k as f32 / n_dash as f32 + self.scroll).fract();
            let d = p.powf(PERSP);
            if d < 0.03 {
                continue;
            }
            let y = (hy + (dash_top - hy) * d) as i32;
            let bright = 0.25 + 0.75 * d;
            let c = scale(self.pal.dash, bright);
            canvas.add(cx as i32, y, c);
            canvas.add((cx + road_half * 0.35 * d) as i32, y, scale(c, 0.7));
            canvas.add((cx - road_half * 0.35 * d) as i32, y, scale(c, 0.7));
        }

        // tunnel walls
        if self.tunnel_k > 0.0 {
            for y in hy as i32..dash_top as i32 {
                let d = ((y as f32 - hy) / (dash_top - hy)).powf(0.5);
                let inset = (1.0 - d) * w as f32 * 0.35 * self.tunnel_k;
                for x in 0..inset as i32 {
                    let c = scale((18, 18, 22), 0.4 + d * 0.5);
                    canvas.set(x, y, c);
                    canvas.set(w as i32 - 1 - x, y, c);
                }
                if (y + (self.t * 80.0) as i32) % 8 < 2 {
                    let lc = scale((255, 230, 180), (0.3 + d * 0.5) * self.tunnel_k);
                    canvas.add((inset * 0.5) as i32, y, lc);
                    canvas.add(w as i32 - 1 - (inset * 0.5) as i32, y, lc);
                }
            }
        }

        // oncoming headlights
        if self.rng.random::<f32>() < 2.5 * dt {
            self.spawn_oncoming();
        }
        self.oncoming.retain_mut(|o| {
            o.d += dt * 0.55 * self.speed;
            if o.d > 1.1 {
                return false;
            }
            let d = o.d.powf(PERSP);
            let y = hy + (dash_top - hy) * d;
            let x = cx + o.lane * road_half * d * 0.8;
            let b = (0.2 + 0.8 * d) * (1.0 + rain_boost * 0.4);
            glow(canvas, x as i32, y as i32, (1.0 + d * 3.0) as i32, self.pal.head, b * 0.35);
            canvas.add((x - 2.0 * d) as i32, y as i32, scale(self.pal.head, b));
            canvas.add((x + 2.0 * d) as i32, y as i32, scale(self.pal.head, b));
            true
        });

        // overtake car ahead
        if let Event::Overtake(e) = self.event {
            if e > 0.5 && e < 4.5 {
                let p = ((e - 0.5) / 3.5).clamp(0.0, 1.0);
                let d = (1.0 - p).powf(1.5) * 0.7 + 0.05;
                let y = hy + (dash_top - hy) * d;
                let x = cx + road_half * 0.12 * (1.0 - p);
                let tb = (0.3 + 0.7 * (1.0 - p)) * 0.9;
                glow(canvas, x as i32, y as i32, 2, self.pal.tail, tb * 0.4);
                canvas.add(x as i32, y as i32, scale(self.pal.tail, tb));
                canvas.add((x + 3.0) as i32, y as i32, scale(self.pal.tail, tb));
            }
        }

        // side scenery
        if self.rng.random::<f32>() < 3.0 * dt * self.detail.density() {
            self.spawn_side();
        }
        self.side.retain_mut(|o| {
            o.d += dt * 0.45 * self.speed * (1.0 + o.d * 0.8);
            if o.d > 1.05 {
                return false;
            }
            let d = o.d.powf(PERSP * 0.9);
            let y = hy + (dash_top - hy) * d * 0.95;
            let x_off = road_half * (1.05 + d * 0.15) * o.side;
            let x = cx + x_off;
            let col = scale((30, 35, 28), 0.2 + 0.8 * d);
            let hh = (o.h * d * gh * 0.12).max(1.0) as i32;
            match o.kind {
                0 => {
                    for dy in 0..hh {
                        canvas.set(x as i32, (y - dy as f32) as i32, col);
                    }
                }
                1 => {
                    for dy in 0..hh {
                        let wob = (dy as f32 * 0.3).sin() * d;
                        canvas.set((x + wob) as i32, (y - dy as f32) as i32, col);
                        if dy > hh / 2 {
                            canvas.set((x + wob + 1.0) as i32, (y - dy as f32) as i32, scale(col, 0.7));
                        }
                    }
                }
                _ => {
                    let wsign = (o.side * 20.0) as i32;
                    for dx in 0..4 {
                        canvas.set(
                            x as i32 + dx * wsign.signum(),
                            (y - hh as f32 / 2.0) as i32,
                            scale((200, 180, 60), d * 0.5),
                        );
                    }
                }
            }
            true
        });

        // rain on windshield
        let rain_amt = self.pal.rain as u8 as f32 * 0.6 + rain_boost;
        if rain_amt > 0.05 {
            for s in &mut self.rain {
                s.y += s.speed * dt * rain_amt;
                s.x += 12.0 * dt;
                if s.y > dash_top {
                    s.y = self.rng.random_range(0.0..hy * 0.5);
                    s.x = self.rng.random_range(0.0..w as f32);
                }
                let alpha = 0.15 * rain_amt;
                for i in 0..s.len as i32 {
                    canvas.add(
                        s.x as i32,
                        (s.y + i as f32) as i32,
                        scale((180, 190, 210), alpha),
                    );
                }
            }
        }

        // dashboard / windshield frame (near plane)
        let dash_y = (h as f32 * 0.82) as i32;
        for y in dash_y..h as i32 {
            let t = (y - dash_y) as f32 / (h as i32 - dash_y).max(1) as f32;
            let c = scale((4, 4, 6), 0.85 + t * 0.15);
            for x in 0..w {
                canvas.set(x as i32, y, c);
            }
        }
        // windshield pillars
        let pillar_w = (w as f32 * 0.06).max(2.0) as i32;
        for y in 0..dash_y {
            for px in 0..pillar_w {
                canvas.set(px, y, scale((0, 0, 0), 0.7));
                canvas.set(w as i32 - 1 - px, y, scale((0, 0, 0), 0.7));
            }
        }
        // steering wheel arc
        let sw_y = h as i32 - (h as f32 * 0.08) as i32;
        for dx in -8..=8 {
            let dy = (dx * dx) as f32 / 20.0;
            canvas.set(cx as i32 + dx, sw_y - dy as i32, scale((20, 20, 24), 0.9));
        }
        // mirror glow after overtake
        if self.mirror_glow > 0.0 {
            glow(
                canvas,
                (w as f32 * 0.12) as i32,
                (h as f32 * 0.15) as i32,
                2,
                self.pal.head,
                self.mirror_glow * 0.25,
            );
        }
    }
}

#[inline]
fn x_wave(x: f32, t: f32) -> f32 {
    x * 0.08 + t * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn drive_runs_all_themes() {
        for theme in [None, Some("dusk"), Some("rain"), Some("neon")] {
            let mut d = Drive::new(StdRng::seed_from_u64(1), theme, Detail::Medium);
            let mut c = Canvas::new(80, 40);
            for _ in 0..120 {
                d.update(1.0 / 30.0, &mut c);
            }
        }
    }
}
