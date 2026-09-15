//! Airspace: realistic sky over a rolling landscape — atmospheric gradient,
//! parallax clouds, aircraft at multiple depths with contrails and nav
//! lights, and events (formation pass, sun break, low pass with ground
//! shadow). Eight themes act as presets (day, golden, dusk, coast, storm,
//! night, winter, busy) cycled via Settings → Theme.

use super::noise::fbm;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

#[derive(Clone, Copy, PartialEq)]
enum LandStyle {
    Fields,
    Coast,
    Snow,
    Dark,
}

#[derive(Clone, Copy)]
struct ThemeCfg {
    sky_a: (u8, u8, u8),
    sky_b: (u8, u8, u8),
    haze: (u8, u8, u8),
    land_a: (u8, u8, u8),
    land_b: (u8, u8, u8),
    water: Option<(u8, u8, u8)>,
    cloud_hi: (u8, u8, u8),
    cloud_lo: (u8, u8, u8),
    land_style: LandStyle,
    cloud_density: f32,
    traffic_rate: f32,
    contrail_str: f32,
    stars: bool,
    storm: bool,
    accent: Option<(f32, f32, (u8, u8, u8))>, // fx, fy, color (sun/moon)
}

struct Aircraft {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    layer: u8, // 0 far, 1 mid, 2 near
    phase: f32,
    trail: Vec<(f32, f32, f32)>, // x, y, brightness
    formation: bool,
}
#[derive(Clone, Copy)]
enum Event {
    None,
    SunBreak(f32),
    LowPass(f32), // elapsed
    Formation(f32),
}

pub struct Airspace {
    rng: StdRng,
    detail: Detail,
    cfg: ThemeCfg,
    seed: u32,
    planes: Vec<Aircraft>,
    spawn_acc: f32,
    event: Event,
    next_event: f32,
    sun_break_x: f32,
    flash: f32,
    next_flash: f32,
    flash_x: f32,
    grid: Vec<f32>,
    xoff0: f32,
    xoff1: f32,
    t: f32,
}

fn theme_cfg(theme: Option<&str>) -> ThemeCfg {
    match theme {
        Some("golden") => ThemeCfg {
            sky_a: (18, 28, 58),
            sky_b: (120, 72, 38),
            haze: (180, 130, 80),
            land_a: (28, 38, 18),
            land_b: (58, 48, 22),
            water: None,
            cloud_hi: (255, 236, 210),
            cloud_lo: (160, 130, 100),
            land_style: LandStyle::Fields,
            cloud_density: 0.55,
            traffic_rate: 0.9,
            contrail_str: 0.7,
            stars: false,
            storm: false,
            accent: Some((0.35, 0.55, (255, 190, 90))),
        },
        Some("dusk") => ThemeCfg {
            sky_a: (8, 10, 32),
            sky_b: (80, 38, 52),
            haze: (140, 80, 100),
            land_a: (12, 14, 22),
            land_b: (22, 20, 28),
            water: None,
            cloud_hi: (220, 180, 200),
            cloud_lo: (90, 70, 90),
            land_style: LandStyle::Dark,
            cloud_density: 0.5,
            traffic_rate: 0.85,
            contrail_str: 0.55,
            stars: false,
            storm: false,
            accent: None,
        },
        Some("coast") => ThemeCfg {
            sky_a: (24, 38, 62),
            sky_b: (100, 120, 140),
            haze: (130, 150, 165),
            land_a: (22, 32, 28),
            land_b: (38, 48, 42),
            water: Some((12, 28, 42)),
            cloud_hi: (230, 238, 245),
            cloud_lo: (140, 155, 170),
            land_style: LandStyle::Coast,
            cloud_density: 0.45,
            traffic_rate: 0.6,
            contrail_str: 0.5,
            stars: false,
            storm: false,
            accent: None,
        },
        Some("storm") => ThemeCfg {
            sky_a: (6, 8, 14),
            sky_b: (28, 32, 42),
            haze: (50, 55, 65),
            land_a: (14, 16, 18),
            land_b: (24, 26, 28),
            water: None,
            cloud_hi: (170, 178, 190),
            cloud_lo: (80, 88, 100),
            land_style: LandStyle::Dark,
            cloud_density: 0.85,
            traffic_rate: 0.45,
            contrail_str: 0.35,
            stars: false,
            storm: true,
            accent: None,
        },
        Some("night") => ThemeCfg {
            sky_a: (0, 0, 4),
            sky_b: (4, 6, 14),
            haze: (12, 16, 28),
            land_a: (4, 6, 8),
            land_b: (8, 10, 14),
            water: None,
            cloud_hi: (40, 45, 55),
            cloud_lo: (18, 20, 28),
            land_style: LandStyle::Dark,
            cloud_density: 0.35,
            traffic_rate: 0.5,
            contrail_str: 0.25,
            stars: true,
            storm: false,
            accent: Some((0.78, 0.22, (210, 215, 230))),
        },
        Some("winter") => ThemeCfg {
            sky_a: (140, 155, 175),
            sky_b: (190, 198, 210),
            haze: (200, 205, 215),
            land_a: (170, 178, 188),
            land_b: (200, 205, 212),
            water: None,
            cloud_hi: (240, 244, 248),
            cloud_lo: (190, 196, 204),
            land_style: LandStyle::Snow,
            cloud_density: 0.5,
            traffic_rate: 0.55,
            contrail_str: 0.4,
            stars: false,
            storm: false,
            accent: None,
        },
        Some("busy") => ThemeCfg {
            sky_a: (22, 42, 88),
            sky_b: (120, 160, 200),
            haze: (150, 180, 210),
            land_a: (26, 40, 22),
            land_b: (48, 58, 32),
            water: None,
            cloud_hi: (248, 252, 255),
            cloud_lo: (170, 185, 200),
            land_style: LandStyle::Fields,
            cloud_density: 0.5,
            traffic_rate: 2.0,
            contrail_str: 0.85,
            stars: false,
            storm: false,
            accent: None,
        },
        _ => ThemeCfg {
            // day
            sky_a: (18, 42, 92),
            sky_b: (120, 168, 210),
            haze: (150, 190, 220),
            land_a: (28, 48, 24),
            land_b: (52, 68, 36),
            water: None,
            cloud_hi: (250, 252, 255),
            cloud_lo: (175, 188, 200),
            land_style: LandStyle::Fields,
            cloud_density: 0.5,
            traffic_rate: 1.0,
            contrail_str: 0.65,
            stars: false,
            storm: false,
            accent: None,
        },
    }
}

impl Airspace {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        Airspace {
            seed: rng.random::<u32>(),
            cfg: theme_cfg(theme),
            rng,
            detail,
            planes: Vec::new(),
            spawn_acc: 0.0,
            event: Event::None,
            next_event: 8.0,
            sun_break_x: 0.5,
            flash: 0.0,
            next_flash: 10.0,
            flash_x: 0.5,
            grid: Vec::new(),
            xoff0: 0.0,
            xoff1: 0.0,
            t: 0.0,
        }
    }

    fn land_top(&self, h: usize) -> f32 {
        h as f32 * 0.68
    }

    fn spawn_plane(&mut self, w: usize, h: usize, layer: u8, formation: bool) {
        let lt = self.land_top(h);
        let sky_h = lt * 0.92;
        let (y, vx, vy) = match layer {
            0 => (
                self.rng.random_range(sky_h * 0.08..sky_h * 0.35),
                self.rng.random_range(12.0..22.0),
                self.rng.random_range(-0.5..0.5),
            ),
            1 => (
                self.rng.random_range(sky_h * 0.2..sky_h * 0.55),
                self.rng.random_range(28.0..48.0),
                self.rng.random_range(-1.5..1.5),
            ),
            _ => (
                self.rng.random_range(sky_h * 0.35..sky_h * 0.75),
                self.rng.random_range(45.0..75.0),
                self.rng.random_range(-2.0..2.0),
            ),
        };
        let from_left = self.rng.random::<bool>();
        self.planes.push(Aircraft {
            x: if from_left {
                -10.0
            } else {
                w as f32 + 10.0
            },
            y,
            vx: if from_left { vx } else { -vx },
            vy,
            layer,
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            trail: Vec::new(),
            formation,
        });
    }

    fn draw_jet(canvas: &mut Canvas, x: f32, y: f32, layer: u8, phase: f32, facing_right: bool) {
        let col = match layer {
            0 => (70, 75, 85),
            1 => (45, 50, 58),
            _ => (30, 32, 38),
        };
        let ix = x as i32;
        let iy = y as i32;
        let dir = if facing_right { 1 } else { -1 };
        let wing = if layer >= 2 { 2 } else { 1 };
        canvas.set(ix, iy, col);
        canvas.set(ix + dir, iy, col);
        if layer >= 1 {
            canvas.set(ix, iy - wing, scale(col, 0.85));
            canvas.set(ix, iy + wing, scale(col, 0.85));
        }
        if layer >= 2 {
            canvas.set(ix + dir * 2, iy, scale(col, 0.9));
        }
        // nav lights on near/mid
        if layer >= 1 {
            let blink = (phase * 3.0).sin() > 0.0;
            if blink {
                canvas.set(ix - dir, iy, (255, 60, 60));
            } else {
                canvas.set(ix + dir * 2, iy, (60, 255, 80));
            }
        }
    }

    fn paint_landscape(&self, canvas: &mut Canvas, w: usize, h: usize) {
        let lt = self.land_top(h) as i32;
        let cfg = &self.cfg;
        for y in lt..h as i32 {
            let t = (y - lt) as f32 / (h as i32 - lt).max(1) as f32;
            for x in 0..w as i32 {
                let mut c = lerp(cfg.land_a, cfg.land_b, t);
                match cfg.land_style {
                    LandStyle::Fields => {
                        let patch = fbm(x as f32 * 0.08, y as f32 * 0.06, 2, self.seed);
                        c = lerp(c, lerp(cfg.land_a, cfg.land_b, 0.5), patch * 0.35);
                    }
                    LandStyle::Coast => {
                        if let Some(wc) = cfg.water {
                            if t > 0.55 {
                                let shimmer =
                                    0.85 + 0.15 * (self.t * 1.2 + x as f32 * 0.05).sin();
                                c = scale(wc, shimmer);
                            }
                        }
                    }
                    LandStyle::Snow => {
                        let drift = fbm(x as f32 * 0.04, y as f32 * 0.03, 2, self.seed + 99);
                        c = lerp(c, (220, 225, 232), drift * 0.4);
                    }
                    LandStyle::Dark => {}
                }
                canvas.set(x, y, c);
            }
        }
        // rolling ridgeline
        for x in 0..w {
            let nx = x as f32 / w as f32;
            let ridge = fbm(nx * 3.0 + self.t * 0.02, 0.0, 3, self.seed + 7);
            let rh = (ridge * 0.08 * (h as f32 - lt as f32)) as i32;
            for dy in 0..=rh {
                let y = lt - dy;
                if y >= 0 {
                    let shade = 1.0 - dy as f32 / (rh as f32 + 1.0) * 0.4;
                    canvas.set(
                        x as i32,
                        y,
                        scale(lerp(cfg.land_a, cfg.land_b, 0.3), shade),
                    );
                }
            }
        }
    }

    fn paint_clouds(&mut self, canvas: &mut Canvas, w: usize, _h: usize, lt: f32, dt: f32) {
        let cfg = &self.cfg;
        self.xoff0 += 4.0 * dt_scale(self.t) * dt;
        self.xoff1 += 9.0 * dt_scale(self.t) * dt;
        const STEP: usize = 4;
        let sky_h = lt as usize;
        for layer in 0..2 {
            let thresh = 0.52 - (1.0 - cfg.cloud_density) * 0.12 + layer as f32 * 0.04;
            let gw = w / STEP + 2;
            let gh = sky_h / STEP + 2;
            let (sx, sy, oct, seed, xoff) = if layer == 0 {
                (0.012, 0.035, 4, self.seed, self.xoff0)
            } else {
                (0.020, 0.05, 3, self.seed.wrapping_add(9001), self.xoff1)
            };
            self.grid.resize(gw * gh, 0.0);
            for gy in 0..gh {
                let ny = (gy * STEP) as f32 * sy;
                let row = gy * gw;
                for gx in 0..gw {
                    self.grid[row + gx] =
                        fbm(((gx * STEP) as f32 + xoff) * sx, ny, oct, seed);
                }
            }
            let grid = &self.grid;
            for y in 0..sky_h {
                let (r0, r1) = ((y >> 2) * gw, ((y >> 2) + 1) * gw);
                let ty = (y & 3) as f32 / 4.0;
                for x in 0..w {
                    let ix = x >> 2;
                    let tx = (x & 3) as f32 / 4.0;
                    let d = bilerp(grid, r0, r1, ix, tx, ty);
                    if d <= thresh {
                        continue;
                    }
                    let cov = ((d - thresh) / 0.2).min(1.0) * cfg.cloud_density;
                    let body = if d > thresh + 0.15 {
                        cfg.cloud_hi
                    } else {
                        cfg.cloud_lo
                    };
                    let sky = canvas.get(x as i32, y as i32).color;
                    canvas.set(x as i32, y as i32, lerp(sky, body, 0.3 + 0.6 * cov));
                }
            }
        }
    }
}

#[inline]
fn dt_scale(t: f32) -> f32 {
    0.75 + 0.25 * (t * 0.11).sin()
}

#[inline]
fn bilerp(g: &[f32], r0: usize, r1: usize, ix: usize, tx: f32, ty: f32) -> f32 {
    let a = g[r0 + ix];
    let b = g[r0 + ix + 1];
    let c = g[r1 + ix];
    let d = g[r1 + ix + 1];
    a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty
}

impl Scene for Airspace {
    fn name(&self) -> &'static str {
        "airspace"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let cfg = self.cfg;
        let lt = self.land_top(h);

        // --- events ---
        self.next_event -= dt;
        if self.next_event <= 0.0 && matches!(self.event, Event::None) {
            self.event = match self.rng.random_range(0..3) {
                0 => {
                    self.sun_break_x = self.rng.random_range(0.2..0.8);
                    Event::SunBreak(5.0)
                }
                1 => Event::LowPass(0.0),
                _ => Event::Formation(0.0),
            };
            self.next_event = self.rng.random_range(18.0..28.0);
        }
        match self.event {
            Event::SunBreak(ref mut t) => {
                *t -= dt;
                if *t <= 0.0 {
                    self.event = Event::None;
                }
            }
            Event::LowPass(ref mut e) => {
                *e += dt;
                if *e < 0.35 && self.planes.iter().all(|p| p.layer != 2) {
                    self.spawn_plane(w, h, 2, false);
                } else if *e >= 4.0 {
                    self.event = Event::None;
                }
            }
            Event::Formation(ref mut e) => {
                *e += dt;
                if *e < 0.1 {
                    for i in 0..5 {
                        self.spawn_plane(w, h, 1, true);
                        let last = self.planes.len() - 1;
                        self.planes[last].y += (i as f32 - 2.0) * 8.0;
                        self.planes[last].x -= i as f32 * 12.0;
                    }
                } else if *e >= 6.0 {
                    self.event = Event::None;
                }
            }
            Event::None => {}
        }

        // storm lightning
        if cfg.storm {
            self.next_flash -= dt;
            if self.next_flash <= 0.0 {
                self.flash = 0.85;
                self.flash_x = self.rng.random_range(0.15..0.85);
                self.next_flash = self.rng.random_range(12.0..22.0);
            }
            self.flash = (self.flash - dt).max(0.0);
        }

        // sky gradient + haze
        let haze_start = lt - h as f32 * 0.08;
        for y in 0..h {
            let ty = y as f32 / h as f32;
            let sky_t = (ty / (lt / h as f32)).min(1.0);
            let mut c = lerp(cfg.sky_a, cfg.sky_b, sky_t.powf(1.1));
            if y as f32 > haze_start {
                let haze_k = ((y as f32 - haze_start) / (h as f32 * 0.08)).min(1.0);
                c = lerp(c, cfg.haze, haze_k * 0.55);
            }
            for x in 0..w {
                canvas.set(x as i32, y as i32, c);
            }
        }

        // stars
        if cfg.stars {
            let n = self.detail.scale((w * h / 280) as f32, 20);
            for i in 0..n {
                let hsh = (self.seed ^ (i as u32).wrapping_mul(2654435761)).wrapping_mul(2246822519);
                let fx = (hsh & 0xffff) as f32 / 65536.0;
                let fy = ((hsh >> 16) & 0xffff) as f32 / 65536.0 * (lt / h as f32) * 0.85;
                let tw = 0.6 + 0.4 * (self.t * 0.5 + i as f32).sin();
                canvas.set_f(
                    fx * w as f32,
                    fy * h as f32,
                    scale((200, 210, 230), tw * 0.5),
                );
            }
        }

        // sun / moon accent
        if let Some((fx, fy, col)) = cfg.accent {
            let (mx, my) = ((fx * w as f32) as i32, (fy * h as f32) as i32);
            glow(canvas, mx, my, 3, col, 0.2);
            canvas.set(mx, my, col);
        }

        // sun break
        if let Event::SunBreak(t) = self.event {
            let k = ease_smooth(1.0 - (t / 5.0 - 0.5).abs() * 2.0);
            if k > 0.0 {
                let bx = w as f32 * self.sun_break_x;
                for y in 0..lt as usize {
                    let spread = w as f32 * (0.08 + y as f32 / lt * 0.2);
                    for x in 0..w {
                        let d = ((x as f32 - bx) / spread).abs();
                        if d < 1.0 {
                            canvas.add(
                                x as i32,
                                y as i32,
                                scale((255, 240, 210), (1.0 - d) * k * 0.18),
                            );
                        }
                    }
                }
            }
        }

        // lightning flash
        if cfg.storm && self.flash > 0.0 {
            let e = 1.0 - self.flash / 0.85;
            let env = (e * 14.0).sin().abs() * (1.0 - e).powi(2);
            let fx = self.flash_x * w as f32;
            for y in 0..lt as usize {
                let k = env * 0.12;
                for x in 0..w {
                    let d = ((x as f32 - fx) / (w as f32 * 0.4)).abs();
                    canvas.add(x as i32, y as i32, scale((180, 190, 220), (1.0 - d) * k));
                }
            }
        }

        self.paint_clouds(canvas, w, h, lt, dt);
        self.paint_landscape(canvas, w, h);

        // spawn traffic
        let max_trail = self.detail.scale(40.0, 12);
        let rate = cfg.traffic_rate * self.detail.density() * density_for(w, h);
        self.spawn_acc += dt * rate * 0.35;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let layer = if self.rng.random::<f32>() < 0.5 {
                1
            } else if self.rng.random::<f32>() < 0.7 {
                0
            } else {
                2
            };
            if self.planes.len() < self.detail.scale(12.0, 3) {
                self.spawn_plane(w, h, layer, false);
            }
        }

        // low-pass shadow on ground
        let low_pass_k = if let Event::LowPass(e) = self.event {
            if e > 0.5 && e < 2.5 {
                ease_smooth((e - 0.5) / 0.8) * (1.0 - ease_smooth((e - 1.8) / 0.8))
            } else {
                0.0
            }
        } else {
            0.0
        };

        // update aircraft
        self.planes.retain_mut(|p| {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.phase += dt * 4.0;
            if p.x < -30.0 || p.x > w as f32 + 30.0 {
                return false;
            }
            if p.layer >= 1 && cfg.contrail_str > 0.0 {
                let boost = if p.formation { 1.4 } else { 1.0 };
                p.trail.push((p.x, p.y, boost));
                if p.trail.len() > max_trail {
                    p.trail.remove(0);
                }
                for tp in &mut p.trail {
                    tp.2 *= 0.96;
                }
            }
            true
        });

        // draw aircraft back to front: far first
        for layer in 0..3 {
            for p in &self.planes {
                if p.layer != layer {
                    continue;
                }
                if p.layer >= 1 {
                    for &(tx, ty, b) in &p.trail {
                        if b > 0.05 {
                            canvas.add(
                                tx as i32,
                                ty as i32,
                                scale((200, 210, 225), b * cfg.contrail_str * 0.35),
                            );
                        }
                    }
                }
                let facing_right = p.vx > 0.0;
                Self::draw_jet(canvas, p.x, p.y, p.layer, p.phase, facing_right);
                if p.layer == 2 && low_pass_k > 0.0 {
                    let sy = lt + 4.0;
                    for dx in -3..=3 {
                        canvas.add(
                            (p.x + dx as f32 * 2.0) as i32,
                            sy as i32,
                            scale((0, 0, 0), low_pass_k * 0.25),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn airspace_runs_all_themes() {
        for theme in [
            None,
            Some("golden"),
            Some("dusk"),
            Some("coast"),
            Some("storm"),
            Some("night"),
            Some("winter"),
            Some("busy"),
        ] {
            let mut a = Airspace::new(StdRng::seed_from_u64(1), theme, Detail::Medium);
            let mut c = Canvas::new(80, 40);
            for _ in 0..90 {
                a.update(1.0 / 30.0, &mut c);
            }
        }
    }
}
