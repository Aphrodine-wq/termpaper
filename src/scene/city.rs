//! Rainy neon metropolis at night: three skyline depth planes with slowly
//! toggling windows, neon signs, headlight/taillight streams, rain
//! streaks, and silent lightning that silhouettes the skyline.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

struct Win {
    dx: i32,
    dy: i32,
    on: bool,
    warm: bool,
    next_toggle: f32,
    /// eased brightness 0..1 (anti-flicker)
    level: f32,
}

struct Building {
    x: i32,
    w: i32,
    h: i32,
    windows: Vec<Win>,
}

struct Neon {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: (u8, u8, u8),
    dark_until: f32,
    phase: f32,
}

struct Car {
    x: f32,
    y: i32,
    vx: f32,
    tail: bool,
}

struct Streak {
    x: f32,
    y: f32,
    vy: f32,
}

pub struct City {
    rng: StdRng,
    detail: Detail,
    sky: ((u8, u8, u8), (u8, u8, u8)),
    neon_density: f32,
    layers: [Vec<Building>; 3],
    neons: Vec<Neon>,
    cars: Vec<Car>,
    rain: Vec<Streak>,
    lightning: f32,
    next_lightning: f32,
    plane: f32,
    train_x: f32,
    t: f32,
    w: usize,
    h: usize,
}

const NEON_COLORS: &[(u8, u8, u8)] = &[
    (255, 60, 180), // pink
    (40, 230, 255), // cyan
    (120, 255, 90), // lime
    (255, 150, 40), // amber
    (190, 90, 255), // violet
];

impl City {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (sky, neon_density) = match theme {
            Some("noir") => (((2, 2, 5), (12, 11, 16)), 0.3),
            Some("dusk") => (((5, 4, 13), (48, 24, 22)), 0.7),
            Some("realistic") => (((3, 5, 10), (17, 21, 31)), 0.5),
            _ => (((4, 3, 12), (22, 11, 36)), 1.0), // neon
        };
        City {
            rng,
            detail,
            sky,
            neon_density,
            layers: [Vec::new(), Vec::new(), Vec::new()],
            neons: Vec::new(),
            cars: Vec::new(),
            rain: Vec::new(),
            lightning: 0.0,
            next_lightning: 8.0,
            plane: -20.0,
            train_x: -60.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let street = (h as f32 * 0.90) as i32;
        // three depth planes: far tall & dim, near short & detailed
        for (li, layer) in self.layers.iter_mut().enumerate() {
            layer.clear();
            let mut x = -self.rng.random_range(0..4);
            while x < w as i32 + 4 {
                let bw = self.rng.random_range(5..13);
                let hmin = [0.34, 0.24, 0.14][li];
                let hvar = [0.30, 0.24, 0.18][li];
                let bh = (h as f32 * (hmin + self.rng.random::<f32>() * hvar)) as i32;
                let mut windows = Vec::new();
                // windows on every plane; the far plane is sparse + dim
                let win_p = if li == 0 { 0.16 } else { 0.55 };
                for wy in (2..bh - 2).step_by(3) {
                    for wx in (1..bw - 1).step_by(2) {
                        if self.rng.random::<f32>() < win_p {
                            let on = self.rng.random::<f32>() < 0.5;
                            windows.push(Win {
                                dx: wx,
                                dy: wy,
                                on,
                                warm: self.rng.random::<f32>() < 0.7,
                                next_toggle: self.rng.random_range(2.0..30.0),
                                level: if on { 1.0 } else { 0.0 },
                            });
                        }
                    }
                }
                layer.push(Building {
                    x,
                    w: bw,
                    h: bh,
                    windows,
                });
                x += bw + self.rng.random_range(1..4);
            }
        }
        // neon signs on the front plane
        self.neons = (0..(((w / 40).clamp(2, 6) as f32 * self.neon_density) as usize).max(1))
            .map(|_| {
                let b = &self.layers[2][self.rng.random_range(0..self.layers[2].len().max(1))];
                Neon {
                    x: b.x + 1,
                    y: street - b.h + self.rng.random_range(2..(b.h / 2).max(3)),
                    w: self.rng.random_range(3..7).min(b.w - 2).max(2),
                    h: self.rng.random_range(2..4),
                    color: NEON_COLORS[self.rng.random_range(0..NEON_COLORS.len())],
                    dark_until: 0.0,
                    phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                }
            })
            .collect();
        self.rain = (0..self.detail.scale((w * 4) as f32 * density_for(w, h), 40).min(1000))
            .map(|_| Streak {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                vy: self.rng.random_range(60.0..110.0),
            })
            .collect();
    }

    #[cfg(test)]
    fn lit_windows(&self) -> usize {
        self.layers
            .iter()
            .flat_map(|l| l.iter())
            .flat_map(|b| b.windows.iter())
            .filter(|w| w.on)
            .count()
    }
}

impl Scene for City {
    fn name(&self) -> &'static str {
        "city"
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
        let street = (h as f32 * 0.90) as i32;

        // lightning scheduling + decay; half the strikes double-flicker
        self.next_lightning -= dt;
        if self.next_lightning <= 0.0 {
            self.lightning = 1.0;
            self.next_lightning = if self.rng.random::<bool>() {
                self.rng.random_range(0.12..0.3) // quick restrike
            } else {
                self.rng.random_range(10.0..24.0)
            };
        }
        self.lightning = (self.lightning - dt * 2.2).max(0.0);
        let flash = self.lightning.powi(2);

        // sky: theme gradient with a light-pollution bloom at the horizon
        for y in 0..h {
            let ty = y as f32 / h.max(1) as f32;
            let horizon = ease_smooth(((ty - 0.68) / 0.32).clamp(0.0, 1.0));
            let base = lerp(
                lerp(self.sky.0, self.sky.1, ty),
                scale(self.sky.1, 1.5),
                horizon * 0.35,
            );
            canvas.fill_row(y, lerp(base, (150, 160, 200), flash * 0.8));
        }

        // skyline layers back to front
        let layer_colors = [(13, 15, 26), (9, 11, 20), (5, 6, 12)];
        for (li, layer) in self.layers.iter().enumerate() {
            let bc = layer_colors[li];
            for b in layer {
                let top = street - b.h;
                for y in top.max(0)..street.min(h as i32) {
                    for x in b.x..b.x + b.w {
                        if x >= 0 && x < w as i32 {
                            canvas.set(x, y, bc);
                        }
                    }
                }
                // rooftop edge; lightning rims the skyline
                let edge = lerp(scale(bc, 1.4), (215, 225, 250), flash * 0.65);
                for x in b.x..b.x + b.w {
                    canvas.set(x, top, edge);
                }
            }
        }

        // windows (toggle slowly); far-plane windows are dimmer
        for (li, layer) in self.layers.iter_mut().enumerate() {
            let dim = if li == 0 { 0.45 } else { 1.0 };
            for b in layer {
                for win in &mut b.windows {
                    if t >= win.next_toggle {
                        win.on = !win.on;
                        win.next_toggle = t + self.rng.random_range(3.0..25.0);
                    }
                    // ease toward target over ~5 frames
                    let target = if win.on { 1.0 } else { 0.0 };
                    win.level += (target - win.level) * dt * 10.0;
                    if win.level > 0.02 {
                        let c = if win.warm { (255, 190, 95) } else { (165, 195, 240) };
                        let flick = 0.85 + 0.15 * ((win.dx * 31 + win.dy * 17) as f32 + t * 2.0).sin();
                        canvas.set(
                            b.x + win.dx,
                            street - b.h + win.dy,
                            scale(c, flick * win.level * dim),
                        );
                    }
                }
            }
        }

        // neon signs with glow, breathing brightness + flicker
        for n in &mut self.neons {
            if n.dark_until > t {
                continue;
            }
            if self.rng.random::<f32>() < dt * 0.4 {
                n.dark_until = t + self.rng.random_range(0.06..0.25);
                continue;
            }
            let breathe = 0.82 + 0.18 * (t * 2.1 + n.phase).sin();
            let c = scale(n.color, breathe);
            for dy in 0..n.h {
                for dx in 0..n.w {
                    canvas.set(n.x + dx, n.y + dy, c);
                }
            }
            for dx in -1..=n.w {
                canvas.add(n.x + dx, n.y - 1, scale(c, 0.25));
                canvas.add(n.x + dx, n.y + n.h, scale(c, 0.25));
                canvas.add(n.x + dx, n.y - 2, scale(c, 0.1));
                canvas.add(n.x + dx, n.y + n.h + 1, scale(c, 0.1));
            }
            canvas.add(n.x - 1, n.y + n.h / 2, scale(c, 0.18));
            canvas.add(n.x + n.w, n.y + n.h / 2, scale(c, 0.18));
        }

        // street band + car light streams
        for y in street.max(0)..h as i32 {
            for x in 0..w {
                canvas.set(x as i32, y, (10, 10, 14));
            }
        }
        // wet asphalt smears the neon down the street
        for n in &self.neons {
            if n.dark_until > t {
                continue;
            }
            let breathe = 0.82 + 0.18 * (t * 2.1 + n.phase).sin();
            for dy in 1..=4 {
                let falloff = (0.14 - dy as f32 * 0.03).max(0.0) * breathe;
                for dx in 0..n.w {
                    canvas.add(n.x + dx, street + dy, scale(n.color, falloff));
                }
            }
        }
        if self.rng.random::<f32>() < dt * 4.0 {
            self.cars.push(Car {
                x: -2.0,
                y: street + 1,
                vx: self.rng.random_range(25.0..45.0),
                tail: false,
            });
        }
        if self.rng.random::<f32>() < dt * 3.2 {
            self.cars.push(Car {
                x: w as f32 + 2.0,
                y: street + 3,
                vx: -self.rng.random_range(22.0..40.0),
                tail: true,
            });
        }
        self.cars.retain_mut(|c| {
            c.x += c.vx * dt;
            if c.x < -4.0 || c.x > w as f32 + 4.0 {
                return false;
            }
            let col = if c.tail { (255, 55, 45) } else { (255, 240, 200) };
            canvas.set_f(c.x, c.y as f32, col);
            canvas.set_f(c.x + if c.tail { 1.0 } else { -1.0 }, c.y as f32, scale(col, 0.7));
            // wet-road reflection
            canvas.set_f(c.x, c.y as f32 + 2.0, scale(col, 0.25));
            true
        });

        // a plane crosses the high sky, cold blinking light
        self.plane += dt * 9.0;
        if self.plane > w as f32 + 30.0 {
            self.plane = -20.0 - self.rng.random_range(0.0..60.0);
        }
        if self.plane > -10.0 && self.plane < w as f32 + 10.0 {
            let py = h as f32 * 0.06 + (self.plane * 0.02).sin() * 2.0;
            let blink = (self.t * 2.5).sin() > 0.3;
            canvas.set_f(self.plane, py, (110, 120, 140));
            if blink {
                canvas.set_f(self.plane, py - 1.0, (255, 80, 80));
            }
        }

        // elevated train: a lit caterpillar crossing behind the front layer
        self.train_x += dt * 22.0;
        if self.train_x > w as f32 + 80.0 {
            self.train_x = -80.0 - self.rng.random_range(0.0..120.0);
        }
        let train_y = street - (h as f32 * 0.30) as i32;
        for i in 0..12 {
            let cx = self.train_x as i32 - i * 5;
            // body
            for dy in 0..2 {
                canvas.set(cx, train_y + dy, (14, 16, 24));
            }
            // lit windows
            if i % 2 == 0 {
                canvas.set(cx, train_y, (255, 220, 130));
            }
            // red marker at the tail
            if i == 11 {
                canvas.set(cx, train_y + 1, (255, 60, 50));
            }
        }

        // rain streaks over everything; wind gusts, flashes catch the drops
        let wind = (t * 0.35).sin() * 7.0;
        let drop = lerp((140, 160, 190), (235, 242, 255), flash * 0.8);
        let a0 = 0.35 + flash * 0.3;
        let a1 = 0.18 + flash * 0.2;
        for r in &mut self.rain {
            r.y += r.vy * dt;
            r.x += wind * dt;
            if r.y > h as f32 {
                r.y = -2.0;
                r.x = self.rng.random_range(0.0..w as f32);
            }
            if r.x < 0.0 {
                r.x += w as f32;
            } else if r.x >= w as f32 {
                r.x -= w as f32;
            }
            let c = canvas.get(r.x as i32, r.y as i32).color;
            canvas.set(r.x as i32, r.y as i32, lerp(c, drop, a0));
            canvas.set(
                r.x as i32,
                r.y as i32 - 1,
                lerp(canvas.get(r.x as i32, r.y as i32 - 1).color, drop, a1),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn car_debug() {
        use rand::SeedableRng;
        let mut c = City::new(StdRng::seed_from_u64(81), None, Detail::Medium);
        let mut canvas = Canvas::new(120, 50);
        for _ in 0..200 {
            c.update(1.0 / 30.0, &mut canvas);
        }
        eprintln!("cars alive: {}", c.cars.len());
        let hits = (0..120)
            .flat_map(|x| (0..50).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let col = canvas.get(x, y).color;
                col == (255, 240, 200) || col == (255, 55, 45)
            })
            .count();
        eprintln!("car pixels on canvas: {hits}");
        assert!(!c.cars.is_empty() || hits > 0, "cars should exist");
    }

    use super::*;
    use rand::SeedableRng;

    #[test]
    fn windows_toggle_over_time() {
        let mut c = City::new(StdRng::seed_from_u64(81), None, Detail::Medium);
        let mut canvas = Canvas::new(120, 50);
        c.update(1.0 / 30.0, &mut canvas);
        let states: Vec<bool> = c
            .layers
            .iter()
            .flat_map(|l| l.iter())
            .flat_map(|b| b.windows.iter().map(|w| w.on))
            .collect();
        assert!(states.len() > 50, "city should have many windows");
        // simulate 20 seconds
        for _ in 0..600 {
            c.update(1.0 / 30.0, &mut canvas);
        }
        let states2: Vec<bool> = c
            .layers
            .iter()
            .flat_map(|l| l.iter())
            .flat_map(|b| b.windows.iter().map(|w| w.on))
            .collect();
        let changed = states.iter().zip(&states2).filter(|(a, b)| a != b).count();
        assert!(changed > 5, "windows should toggle, {changed} changed");
        assert!(c.lit_windows() > 10);
    }
}
