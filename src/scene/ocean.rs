//! Night ocean: layered sine swells breathing at different speeds, moonlit
//! crest rims, a shimmering glint path, sparse stars and meteors, a lanterned
//! sailboat riding the back swell, leaping fish with splash rings, sheet
//! lightning that also lights the water, and a periodic swell surge that
//! rolls through the sea (anticipation -> payoff -> decay).

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const TAU: f32 = std::f32::consts::TAU;

struct Layer {
    ybase: f32,
    amp: f32,
    freq: f32,
    speed: f32,
    color: (u8, u8, u8),
    foam: (u8, u8, u8),
}

/// Hand-tuned per-theme colors. Water keeps its tinted identity but stays
/// under ~25% luminance so the moon, foam and glint pop.
struct Palette {
    sky: ((u8, u8, u8), (u8, u8, u8)), // zenith -> horizon
    layers: [((u8, u8, u8), (u8, u8, u8)); 4], // (body, foam) back -> front
    moon: (u8, u8, u8),
    mare: (u8, u8, u8),
    glint: (u8, u8, u8),
    flash: (u8, u8, u8),
    plankton: (u8, u8, u8),
    moon_dim: f32,
    amp_mul: f32,
    flash_gap: (f32, f32),
}

fn palette_for(theme: Option<&str>) -> Palette {
    match theme {
        Some("storm") => Palette {
            sky: ((3, 5, 10), (14, 20, 27)),
            layers: [
                ((18, 42, 50), (95, 130, 136)),
                ((14, 34, 41), (82, 116, 122)),
                ((11, 27, 33), (68, 100, 106)),
                ((8, 21, 26), (56, 84, 90)),
            ],
            moon: (205, 215, 220),
            mare: (160, 172, 180),
            glint: (165, 185, 195),
            flash: (120, 130, 160),
            plankton: (70, 215, 185),
            moon_dim: 0.5,
            amp_mul: 1.3,
            flash_gap: (9.0, 18.0),
        },
        Some("golden") => Palette {
            sky: ((7, 5, 11), (36, 24, 30)),
            layers: [
                ((30, 28, 66), (150, 118, 96)),
                ((24, 24, 55), (130, 102, 84)),
                ((19, 19, 46), (112, 88, 72)),
                ((14, 14, 37), (95, 74, 62)),
            ],
            moon: (255, 216, 152),
            mare: (214, 168, 118),
            glint: (255, 206, 132),
            flash: (95, 85, 110),
            plankton: (80, 220, 180),
            moon_dim: 1.0,
            amp_mul: 1.0,
            flash_gap: (22.0, 40.0),
        },
        _ => Palette {
            // moonlit
            sky: ((2, 5, 14), (14, 26, 52)),
            layers: [
                ((18, 40, 80), (82, 114, 156)),
                ((14, 32, 66), (70, 100, 142)),
                ((10, 24, 52), (58, 86, 126)),
                ((7, 17, 38), (48, 72, 108)),
            ],
            moon: (230, 236, 248),
            mare: (185, 195, 215),
            glint: (222, 233, 250),
            flash: (70, 80, 110),
            plankton: (60, 220, 190),
            moon_dim: 1.0,
            amp_mul: 1.0,
            flash_gap: (18.0, 35.0),
        },
    }
}

struct Star {
    x: i32,
    y: i32,
    bright: f32,
    phase: f32,
}

struct Boat {
    x: f32,
    vx: f32,
}

struct Leap {
    x: f32,
    y: f32,
    t: f32,
}

struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

pub struct Ocean {
    rng: StdRng,
    detail: Detail,
    pal: Palette,
    stars: Vec<Star>,
    layers: Vec<Layer>,
    boat: Option<Boat>,
    next_boat: f32,
    far_flash: f32,
    next_far_flash: f32,
    leap: Option<Leap>,
    splash: Option<(f32, f32, f32)>, // x, y, age 0..1
    next_leap: f32,
    meteor: Option<Meteor>,
    next_meteor: f32,
    surge_t: Option<f32>, // elapsed while a surge rolls through
    next_surge: f32,
    surge_k: f32, // eased envelope 0..1 for this frame
    moon_x: f32,
    moon_y: f32,
    moon_r: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Ocean {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let pal = palette_for(theme);
        let geom = [
            (0.44, 1.6, 0.045, 0.5),
            (0.58, 2.4, 0.06, -0.8),
            (0.74, 3.2, 0.075, 1.2),
            (0.90, 4.0, 0.09, -1.7),
        ];
        Ocean {
            rng,
            detail,
            layers: geom
                .iter()
                .zip(pal.layers.iter())
                .map(|(&(ybase, amp, freq, speed), &(color, foam))| Layer {
                    ybase,
                    amp,
                    freq,
                    speed,
                    color,
                    foam,
                })
                .collect(),
            pal,
            stars: Vec::new(),
            moon_x: 0.0,
            moon_y: 0.0,
            moon_r: 0.0,
            boat: None,
            next_boat: 10.0,
            far_flash: 0.0,
            next_far_flash: 20.0,
            leap: None,
            splash: None,
            next_leap: 12.0,
            meteor: None,
            next_meteor: 8.0,
            surge_t: None,
            next_surge: 9.0, // first payoff lands early
            surge_k: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.moon_x = w as f32 * 0.72;
        self.moon_y = h as f32 * 0.14;
        self.moon_r = (h as f32 * 0.07).max(2.0);
        self.stars = (0..self
            .detail
            .scale((w * h / 90) as f32 * density_for(w, h), 12)
            .clamp(12, 260))
            .map(|_| Star {
                x: self.rng.random_range(0..w as i32),
                y: self.rng.random_range(0..(h as f32 * 0.4).max(1.0) as i32),
                bright: self.rng.random_range(0.3..1.0),
                phase: self.rng.random_range(0.0..TAU),
            })
            .collect();
    }

    fn wave_y(&self, li: usize, x: f32) -> f32 {
        let l = &self.layers[li];
        let hf = self.h as f32;
        // slow amplitude breathing + the surge rolling through the back sea
        let surge_w = if li < 2 { 1.0 } else { 0.45 };
        let amp = l.amp
            * self.pal.amp_mul
            * (1.0 + 0.22 * (self.t * 0.23 + li as f32 * 1.7).sin() + 0.5 * self.surge_k * surge_w);
        l.ybase * hf
            + amp * (x * l.freq + self.t * l.speed).sin()
            + amp * 0.4 * (x * l.freq * 2.7 - self.t * l.speed * 1.4).sin()
    }
}

/// Surge rhythm: 2s ease-in, 1.5s hold, 3s release — no popping.
fn surge_env(t: f32) -> f32 {
    if t < 2.0 {
        ease_smooth(t / 2.0)
    } else if t < 3.5 {
        1.0
    } else {
        1.0 - ease_smooth((t - 3.5) / 3.0)
    }
}

impl Scene for Ocean {
    fn name(&self) -> &'static str {
        "ocean"
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

        // --- swell surge scheduling (anticipation -> payoff -> decay) ---
        self.next_surge -= dt;
        if self.next_surge <= 0.0 && self.surge_t.is_none() {
            self.surge_t = Some(0.0);
        }
        if let Some(st) = &mut self.surge_t {
            *st += dt;
            if *st >= 6.5 {
                self.surge_t = None;
                self.next_surge = self.rng.random_range(22.0..38.0);
            }
        }
        self.surge_k = self.surge_t.map(surge_env).unwrap_or(0.0);
        let surge_k = self.surge_k;

        // night sky: eased vertical gradient + a faint horizon haze
        let horizon = (self.layers[0].ybase * h as f32) as i32;
        for y in 0..h {
            let f = ease_smooth((y as f32 / horizon.max(1) as f32).min(1.0));
            canvas.fill_row(y, lerp(self.pal.sky.0, self.pal.sky.1, f));
        }
        for hy in [horizon - 1, horizon] {
            if hy >= 0 && (hy as usize) < h {
                for x in 0..w {
                    canvas.add(x as i32, hy, scale(self.pal.sky.1, 0.30));
                }
            }
        }

        // stars
        for s in &self.stars {
            let b = s.bright * (0.7 + 0.3 * (t * 1.8 + s.phase).sin());
            canvas.set(s.x, s.y, scale((200, 210, 235), b * 0.75));
        }

        // meteor: a fast streak with a decaying trail
        self.next_meteor -= dt;
        if self.next_meteor <= 0.0 && self.meteor.is_none() {
            let speed = self.rng.random_range(50.0..90.0);
            self.meteor = Some(Meteor {
                x: self.rng.random_range(w as f32 * 0.1..w as f32 * 0.9),
                y: self.rng.random_range(0.0..h as f32 * 0.15),
                vx: if self.rng.random::<bool>() { speed } else { -speed },
                vy: self.rng.random_range(16.0..28.0),
                life: 1.0,
            });
            self.next_meteor = self.rng.random_range(14.0..28.0);
        }
        if let Some(m) = &mut self.meteor {
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            m.life -= dt * 1.6;
            if m.life <= 0.0 || m.y >= horizon as f32 {
                self.meteor = None;
            } else {
                canvas.set_f(m.x, m.y, scale((240, 240, 255), (m.life * 0.9).min(1.0)));
                for k in 1..=4 {
                    let f = (m.life * (1.0 - k as f32 * 0.2) * 0.4).max(0.0);
                    canvas.add(
                        (m.x - m.vx * k as f32 * 0.02) as i32,
                        (m.y - m.vy * k as f32 * 0.02) as i32,
                        scale((180, 190, 230), f),
                    );
                }
            }
        }

        // moon + halo (gently breathing)
        let (mx, my, mr) = (self.moon_x, self.moon_y, self.moon_r);
        let breathe = (0.85 + 0.15 * (t * 0.5).sin()) * self.pal.moon_dim;
        for dy in (-(mr as i32) - 4)..=(mr as i32 + 4) {
            for dx in (-(mr as i32) - 4)..=(mr as i32 + 4) {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d <= mr {
                    // soft-shaded disc with a darker mare patch
                    let shade = (1.0 - 0.15 * (d / mr)) * breathe;
                    let mare = ((dx - 1).pow(2) + (dy + 1).pow(2)) as f32 <= (mr * 0.45).powi(2);
                    let c = if mare { self.pal.mare } else { self.pal.moon };
                    canvas.set(mx as i32 + dx, my as i32 + dy, scale(c, shade));
                } else if d <= mr + 3.5 {
                    let glow_v = (1.0 - (d - mr) / 3.5) * 30.0 * breathe;
                    canvas.add(
                        mx as i32 + dx,
                        my as i32 + dy,
                        scale(self.pal.glint, glow_v / 255.0 * 1.4),
                    );
                }
            }
        }

        // rare silent lightning on the far horizon (also rims the crests below)
        self.next_far_flash -= dt;
        if self.next_far_flash <= 0.0 {
            self.far_flash = 1.0;
            self.next_far_flash = self.rng.random_range(self.pal.flash_gap.0..self.pal.flash_gap.1);
        }
        self.far_flash = (self.far_flash - dt * 3.5).max(0.0);
        let flash_k = self.far_flash * self.far_flash;
        if flash_k > 0.0 {
            for y in 0..horizon {
                let near = 1.0 - y as f32 / horizon.max(1) as f32;
                for x in 0..w {
                    canvas.add(x as i32, y, scale(self.pal.flash, flash_k * near * 0.4));
                }
            }
        }

        // swells back to front
        let foam_thr = 0.72 - 0.20 * surge_k;
        let foam_boost = 1.0 + 0.35 * surge_k;
        for li in 0..self.layers.len() {
            let (color, foam) = (self.layers[li].color, self.layers[li].foam);
            let (lfreq, lspeed) = (self.layers[li].freq, self.layers[li].speed);
            for x in 0..w {
                let xf = x as f32;
                let wy = self.wave_y(li, xf);
                let crest = (xf * lfreq + t * lspeed).sin();
                let yi = wy as i32;
                for y in yi.max(0)..h as i32 {
                    // darker with depth below the surface line + drifting texture
                    let depth = ((y as f32 - wy) / (h as f32 * 0.12)).min(1.0);
                    let tex =
                        1.0 + 0.07 * (xf * 0.13 + y as f32 * 0.29 - t * (0.4 + li as f32 * 0.15)).sin();
                    canvas.set(x as i32, y, scale(color, (1.0 - 0.4 * depth) * tex));
                }
                if yi >= 0 && yi < h as i32 {
                    // foam at crests (threshold drops while a surge rolls through)
                    if crest > foam_thr {
                        let f = ((crest - foam_thr) / (1.0 - foam_thr)).min(1.0);
                        canvas.set(x as i32, yi, lerp(color, scale(foam, foam_boost), f));
                        // whitecap scatter just under strong crests
                        if crest > 0.93 && yi + 1 < h as i32 {
                            canvas.add(x as i32, yi + 1, scale(foam, 0.25 * foam_boost));
                        }
                    }
                    // moonlight rims the crests near the glint column
                    let g = 1.0 - ((xf - mx).abs() / (w as f32 * 0.18)).min(1.0);
                    if g > 0.0 {
                        canvas.add(
                            x as i32,
                            yi,
                            scale(self.pal.glint, g * g * 0.10 * (0.5 + 0.5 * crest) * self.pal.moon_dim),
                        );
                    }
                    // lightning kisses the wave tops
                    if flash_k > 0.0 {
                        canvas.add(
                            x as i32,
                            yi,
                            scale(self.pal.flash, flash_k * 0.35 * (1.0 - li as f32 * 0.2)),
                        );
                    }
                }
            }
        }

        // moon glint path on the water: shimmering column below the moon
        let glint_top = (self.layers[0].ybase * h as f32) as i32;
        let glint_boost = (1.0 + 0.5 * surge_k) * self.pal.moon_dim;
        for y in glint_top.max(0)..h as i32 {
            let fy = (y - glint_top) as f32 / (h as i32 - glint_top).max(1) as f32;
            let half_w = 1.5 + fy * w as f32 * 0.03;
            let center = mx + (t * 0.7 + y as f32 * 0.3).sin() * 1.5;
            for dx in -(half_w as i32)..=(half_w as i32) {
                let x = center as i32 + dx;
                if x < 0 || x >= w as i32 {
                    continue;
                }
                let shimmer = ((x * 911 + y * 3571 + (t * 7.0) as i32 * 131) % 100) as f32 / 100.0;
                if shimmer < 0.45 {
                    let inten = ((1.0 - fy * 0.6) * (0.45 - shimmer) * 0.55 * glint_boost).min(0.6);
                    canvas.add(x, y, scale(self.pal.glint, inten));
                }
            }
        }

        // occasional sailboat silhouette riding the back swell, warm lantern
        self.next_boat -= dt;
        if self.next_boat <= 0.0 && self.boat.is_none() {
            let from_left = self.rng.random::<bool>();
            self.boat = Some(Boat {
                x: if from_left { -10.0 } else { w as f32 + 10.0 },
                vx: if from_left { 4.5 } else { -4.5 },
            });
            self.next_boat = self.rng.random_range(20.0..35.0);
        }
        let mut boat_gone = false;
        if let Some(b) = &mut self.boat {
            b.x += b.vx * dt;
            boat_gone = b.x < -12.0 || b.x > w as f32 + 12.0;
        }
        if boat_gone {
            self.boat = None;
        }
        if let Some(b) = &self.boat {
            let bx = b.x as i32;
            // rides the swell: eased vertical motion instead of a fixed line
            let by = (self.wave_y(0, b.x) - 1.0) as i32;
            let dark = (3, 4, 9);
            // hull
            for dx in -3..=3 {
                canvas.set(bx + dx, by, dark);
            }
            canvas.set(bx - 2, by + 1, dark);
            canvas.set(bx + 2, by + 1, dark);
            // mast + triangle sail
            let dir = -b.vx.signum() as i32;
            for dy in 1..=5 {
                canvas.set(bx, by - dy, dark);
                canvas.set(bx + dir * (dy - 1).min(3), by - dy, dark);
            }
            // lantern glow at the stern + shimmering reflection streak
            let lx = bx - dir * 3;
            let flick = 0.8 + 0.2 * (t * 9.0 + bx as f32).sin();
            canvas.set(lx, by - 1, scale((255, 190, 90), flick));
            glow(canvas, lx, by - 1, 2, (255, 170, 70), 0.35 * flick);
            for i in 1..=3 {
                let shim = ((bx * 733 + i * 131 + (t * 6.0) as i32 * 57) % 100) as f32 / 100.0;
                if shim < 0.6 {
                    canvas.add(lx, by + 1 + i, scale((255, 170, 70), 0.16 / i as f32));
                }
            }
            // wake
            for i in 1..=5 {
                canvas.add(bx - (b.vx.signum() as i32) * (3 + i), by + 1, (14, 22, 40));
            }
        }

        // rare fish leap near the glint path; landing leaves a splash ring
        self.next_leap -= dt;
        if self.next_leap <= 0.0 && self.leap.is_none() {
            self.leap = Some(Leap {
                x: self.moon_x + self.rng.random_range(-6.0..6.0),
                y: self.wave_y(3, self.moon_x) - 1.0,
                t: 0.0,
            });
            self.next_leap = self.rng.random_range(15.0..30.0);
        }
        let mut landed: Option<(f32, f32)> = None;
        if let Some(l) = &mut self.leap {
            l.t += dt * 1.4;
            if l.t >= 1.0 {
                landed = Some((l.x, l.y));
            } else {
                // parabolic arc of bright points
                for k in 0..5 {
                    let kt = (l.t - k as f32 * 0.04).clamp(0.0, 1.0);
                    let px = l.x + (kt - 0.5) * 8.0;
                    let py = l.y - (1.0 - (kt * 2.0 - 1.0).powi(2)) * 3.5;
                    canvas.set_f(px, py, scale((200, 220, 240), 1.0 - k as f32 * 0.18));
                }
            }
        }
        if let Some((sx, sy)) = landed {
            self.leap = None;
            self.splash = Some((sx, sy, 0.0));
        }
        if let Some((sx, sy, age)) = &mut self.splash {
            *age += dt * 1.6;
            if *age >= 1.0 {
                self.splash = None;
            } else {
                let r = 0.5 + *age * 4.0;
                let a = (1.0 - *age) * 0.5;
                let steps = (r * 5.0) as i32 + 6;
                for i in 0..steps {
                    let ang = i as f32 / steps as f32 * TAU;
                    canvas.add(
                        (*sx + ang.cos() * r) as i32,
                        (*sy + ang.sin() * r * 0.4) as i32,
                        scale(self.layers[3].foam, a),
                    );
                }
            }
        }

        // bioluminescent plankton sparkles in the wave troughs
        let plankton_n = self.detail.scale((w / 6) as f32, 6).min(60);
        for _ in 0..plankton_n {
            let px = self.rng.random_range(0..w as i32);
            let wy = self.wave_y(3, px as f32) as i32;
            if wy > 0 && wy < h as i32 && self.rng.random::<f32>() < 0.4 {
                let tw = 0.5 + 0.5 * (t * 3.0 + px as f32).sin();
                canvas.add(px, wy + 1, scale(self.pal.plankton, tw * 0.5));
            }
        }
    }
}
