//! Aquarium: side-view tank with caustic light on gravel, drifting plants,
//! fish at varied depths, rising bubbles, and events — feed drop, filter
//! surge, passing shadow.

use super::noise::fbm;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use crate::physics::spring_damper;
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::{PI, TAU};

struct Palette {
    water_a: (u8, u8, u8),
    water_b: (u8, u8, u8),
    sand: (u8, u8, u8),
    plant: (u8, u8, u8),
    fish: [(u8, u8, u8); 3],
    bubble: (u8, u8, u8),
}

struct Fish {
    x: f32,
    y: f32,
    heading: f32,
    turn_vel: f32,
    tx: f32,
    ty: f32,
    retarget: f32,
    depth: f32,
    pattern: usize,
    dart: f32,
}

struct Bubble {
    x: f32,
    y: f32,
    vy: f32,
    wobble: f32,
}

struct Plant {
    x: f32,
    h: f32,
    phase: f32,
    near: bool,
}

struct Pellet {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Event {
    None,
    Feed(f32),
    Filter(f32),
    Shadow(f32),
}

pub struct Aquarium {
    rng: StdRng,
    detail: Detail,
    pal: Palette,
    fish: Vec<Fish>,
    bubbles: Vec<Bubble>,
    plants: Vec<Plant>,
    pellet: Option<Pellet>,
    event: Event,
    next_event: f32,
    filter_k: f32,
    shadow_x: f32,
    caustic_t: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Aquarium {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let pal = match theme {
            Some("freshwater") => Palette {
                water_a: (12, 28, 18),
                water_b: (28, 48, 32),
                sand: (48, 42, 28),
                plant: (40, 70, 35),
                fish: [(140, 150, 160), (100, 110, 120), (180, 170, 150)],
                bubble: (180, 200, 190),
            },
            Some("moonlit") => Palette {
                water_a: (4, 8, 28),
                water_b: (12, 20, 48),
                sand: (20, 24, 40),
                plant: (15, 30, 45),
                fish: [(180, 190, 210), (140, 160, 200), (200, 210, 230)],
                bubble: (160, 180, 220),
            },
            Some("mono") => Palette {
                water_a: (8, 12, 18),
                water_b: (20, 28, 36),
                sand: (32, 36, 42),
                plant: (28, 32, 38),
                fish: [(80, 90, 100), (60, 70, 80), (100, 110, 120)],
                bubble: (140, 150, 160),
            },
            _ => Palette {
                water_a: (8, 28, 38),
                water_b: (18, 55, 62),
                sand: (55, 48, 38),
                plant: (30, 90, 50),
                fish: [(255, 120, 40), (60, 140, 255), (255, 200, 80)],
                bubble: (200, 230, 240),
            },
        };
        Aquarium {
            rng,
            detail,
            pal,
            fish: Vec::new(),
            bubbles: Vec::new(),
            plants: Vec::new(),
            pellet: None,
            event: Event::None,
            next_event: 10.0,
            filter_k: 0.0,
            shadow_x: 0.5,
            caustic_t: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let n_fish = self
            .detail
            .scale((w * h / 1200) as f32 * density_for(w, h), 3)
            .clamp(3, 18);
        self.fish = (0..n_fish)
            .map(|i| {
                let x = self.rng.random_range(w as f32 * 0.15..w as f32 * 0.85);
                let y = self.rng.random_range(h as f32 * 0.25..h as f32 * 0.75);
                Fish {
                    x,
                    y,
                    heading: self.rng.random_range(0.0..TAU),
                    turn_vel: 0.0,
                    tx: x,
                    ty: y,
                    retarget: self.rng.random_range(1.0..3.0),
                    depth: self.rng.random_range(0.3..1.0),
                    pattern: i % 3,
                    dart: 0.0,
                }
            })
            .collect();
        let n_bub = self.detail.scale((w * h / 800) as f32, 4).clamp(4, 30);
        self.bubbles = (0..n_bub)
            .map(|_| Bubble {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(h as f32 * 0.3..h as f32 * 0.9),
                vy: self.rng.random_range(8.0..18.0),
                wobble: self.rng.random_range(0.0..TAU),
            })
            .collect();
        let n_plants = self.detail.scale(w as f32 / 12.0, 3).clamp(3, 12);
        self.plants = (0..n_plants)
            .map(|i| Plant {
                x: self.rng.random_range(0.0..w as f32),
                h: self.rng.random_range(h as f32 * 0.12..h as f32 * 0.35),
                phase: self.rng.random_range(0.0..TAU),
                near: i % 4 == 0,
            })
            .collect();
    }
}

impl Scene for Aquarium {
    fn name(&self) -> &'static str {
        "aquarium"
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
        self.caustic_t += dt * 0.8;

        self.next_event -= dt;
        if self.next_event <= 0.0 && self.event == Event::None {
            self.event = match self.rng.random_range(0..3) {
                0 => {
                    self.pellet = Some(Pellet {
                        x: self.rng.random_range(w as f32 * 0.3..w as f32 * 0.7),
                        y: h as f32 * 0.08,
                    });
                    Event::Feed(0.0)
                }
                1 => Event::Filter(0.0),
                _ => {
                    self.shadow_x = self.rng.random_range(0.0..1.0);
                    Event::Shadow(0.0)
                }
            };
            self.next_event = self.rng.random_range(18.0..28.0);
        }

        self.filter_k = 0.0;
        let mut shadow_k = 0.0;

        match self.event {
            Event::Feed(ref mut e) => {
                *e += dt;
                if let Some(ref mut p) = self.pellet {
                    p.y += 25.0 * dt;
                }
                if *e >= 5.0 {
                    self.event = Event::None;
                    self.pellet = None;
                    for f in &mut self.fish {
                        f.dart = 0.0;
                    }
                }
            }
            Event::Filter(ref mut e) => {
                *e += dt;
                if *e < 1.0 {
                    self.filter_k = ease_smooth(*e / 1.0);
                } else if *e < 3.5 {
                    self.filter_k = 1.0;
                } else if *e < 5.0 {
                    self.filter_k = 1.0 - ease_smooth((*e - 3.5) / 1.5);
                } else {
                    self.event = Event::None;
                }
            }
            Event::Shadow(ref mut e) => {
                *e += dt;
                if *e > 0.8 && *e < 3.2 {
                    shadow_k = 0.5;
                } else if *e >= 4.0 {
                    self.event = Event::None;
                }
            }
            Event::None => {}
        }

        let sand_top = h as f32 * 0.78;
        let seed = 42u32;

        for y in 0..h {
            let t = y as f32 / h as f32;
            let base = lerp(self.pal.water_a, self.pal.water_b, t.powf(0.7));
            for x in 0..w {
                canvas.set(x as i32, y as i32, base);
            }
        }

        for p in self.plants.iter().filter(|p| !p.near) {
            let base_y = sand_top as i32;
            let sway = (self.t * 0.6 + p.phase).sin() * 2.0;
            let hh = p.h as i32;
            for dy in 0..hh {
                let taper = 1.0 - dy as f32 / hh as f32;
                let px = (p.x + sway * taper) as i32;
                canvas.set(px, base_y - dy, scale(self.pal.plant, 0.25 + taper * 0.35));
            }
        }

        for y in sand_top as i32..h as i32 {
            let t = (y as f32 - sand_top) / (h as f32 - sand_top).max(1.0);
            for x in 0..w {
                let sand = lerp(self.pal.sand, scale(self.pal.sand, 0.7), t);
                let caust = fbm(
                    x as f32 * 0.06 + self.caustic_t,
                    y as f32 * 0.05 - self.caustic_t * 0.7,
                    3,
                    seed,
                );
                let fk = (caust - 0.45).max(0.0) * (1.0 + self.filter_k * 0.5);
                let c = lerp(sand, scale((255, 255, 240), 0.3), fk * 0.35);
                canvas.set(x as i32, y as i32, c);
            }
        }

        if let Some(ref pellet) = self.pellet {
            for f in &mut self.fish {
                let dx = pellet.x - f.x;
                let dy = pellet.y - f.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 80.0 && pellet.y < sand_top {
                    f.tx = pellet.x;
                    f.ty = pellet.y;
                    f.dart = (f.dart + dt * 2.0).min(1.0);
                }
            }
        }

        for f in &mut self.fish {
            f.retarget -= dt;
            if f.retarget <= 0.0 {
                f.tx = self.rng.random_range(w as f32 * 0.1..w as f32 * 0.9);
                f.ty = self
                    .rng
                    .random_range(h as f32 * 0.2..sand_top - h as f32 * 0.05);
                f.retarget = self.rng.random_range(2.0..5.0);
            }
            let target_h = (f.ty - f.y).atan2(f.tx - f.x);
            let diff = (target_h - f.heading + PI).rem_euclid(TAU) - PI;
            f.turn_vel += spring_damper(f.heading, f.turn_vel, f.heading + diff, 1.2, 2.0) * dt;
            f.heading += f.turn_vel * dt;
            let spd = (12.0 + f.depth * 18.0) * (1.0 + f.dart * 1.5);
            f.x += f.heading.cos() * spd * dt;
            f.y += f.heading.sin() * spd * dt;
            f.x = f.x.clamp(4.0, w as f32 - 4.0);
            f.y = f.y.clamp(h as f32 * 0.12, sand_top - 2.0);

            let col = self.pal.fish[f.pattern];
            let b = 0.35 + 0.65 * f.depth;
            let len = (3.0 + f.depth * 4.0) as i32;
            let dir = if f.heading.cos() >= 0.0 { 1 } else { -1 };
            for i in 0..len {
                let t = i as f32 / len as f32;
                let fx = f.x + dir as f32 * i as f32;
                let fy = f.y + f.heading.sin() * i as f32 * 0.15;
                canvas.set(fx as i32, fy as i32, scale(col, b * (1.0 - t * 0.3)));
            }
            let tw = (self.t * 8.0 + f.x * 0.1).sin() * f.depth;
            canvas.set(
                (f.x - dir as f32 * len as f32) as i32,
                (f.y + tw) as i32,
                scale(col, b * 0.6),
            );
        }

        if let Some(ref p) = self.pellet {
            if p.y < sand_top {
                glow(canvas, p.x as i32, p.y as i32, 1, (180, 120, 60), 0.4);
                canvas.set(p.x as i32, p.y as i32, (160, 100, 50));
            }
        }

        let extra_bub = (self.filter_k * 8.0) as usize;
        for (i, b) in self.bubbles.iter_mut().enumerate() {
            let boost = if i < extra_bub { 2.5 } else { 1.0 };
            b.y -= b.vy * boost * dt;
            b.x += (self.t * 2.0 + b.wobble).sin() * 8.0 * dt;
            if b.y < h as f32 * 0.05 {
                b.y = sand_top - 2.0;
                b.x = self.rng.random_range(w as f32 * 0.05..w as f32 * 0.95);
            }
            let bb = 0.2 + 0.5 * (1.0 - b.y / h as f32);
            canvas.set(b.x as i32, b.y as i32, scale(self.pal.bubble, bb));
            canvas.set((b.x + 1.0) as i32, b.y as i32, scale(self.pal.bubble, bb * 0.5));
        }
        if self.filter_k > 0.5 {
            let fx = w as f32 * 0.08;
            glow(
                canvas,
                fx as i32,
                sand_top as i32 - 4,
                2,
                self.pal.bubble,
                self.filter_k * 0.3,
            );
        }

        if shadow_k > 0.0 {
            let sx = self.shadow_x * w as f32;
            for y in 0..h {
                for x in 0..w {
                    let d = ((x as f32 - sx) / (w as f32 * 0.25)).abs();
                    if d < 1.0 {
                        let cur = canvas.get(x as i32, y as i32).color;
                        canvas.set(
                            x as i32,
                            y as i32,
                            scale(cur, 1.0 - shadow_k * (1.0 - d) * 0.5),
                        );
                    }
                }
            }
        }

        for p in self.plants.iter().filter(|p| p.near) {
            let base_y = sand_top as i32;
            let sway = (self.t * 0.9 + p.phase).sin() * 3.0;
            let hh = (p.h * 0.6) as i32;
            for dy in 0..hh {
                let px = (p.x + sway * (1.0 - dy as f32 / hh as f32)) as i32;
                canvas.set(
                    px,
                    base_y - dy,
                    scale(
                        self.pal.plant,
                        0.45 + (1.0 - dy as f32 / hh as f32) * 0.3,
                    ),
                );
            }
        }
        for y in 0..(h as f32 * 0.12) as i32 {
            for x in 0..w {
                let glare = ((x as f32 / w as f32 - 0.3).abs() < 0.15) as u8 as f32 * 0.08;
                canvas.add(x as i32, y, scale((255, 255, 255), glare));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn aquarium_runs_all_themes() {
        for theme in [None, Some("freshwater"), Some("moonlit"), Some("mono")] {
            let mut a = Aquarium::new(StdRng::seed_from_u64(3), theme, Detail::Medium);
            let mut c = Canvas::new(80, 40);
            for _ in 0..120 {
                a.update(1.0 / 30.0, &mut c);
            }
        }
    }
}
