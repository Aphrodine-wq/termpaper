//! Meadow: windswept night grass on pure black. Three depth layers of
//! curved 1px blades (far = short/dim/slow, near = tall/bright/responsive)
//! sway on an fBm wind field with visible traveling gust waves; dew glints
//! twinkle on blade tips and flare as gusts pass. A breathing moon hangs
//! over a faint horizon, its haze pooling along the skyline; blades and dew
//! beneath its azimuth catch extra light. Dim twinkling stars and
//! occasional shooting stars sit above. Every ~20-30s a BIG GUST builds
//! from one side (grass leans in anticipation), sweeps the field with a
//! shimmer of glints (payoff), and settles (decay).

use super::noise::fbm;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

/// (brightness, bend response, wind time-scale) per depth layer.
const LAYERS: [(f32, f32, f32); 3] = [
    (0.38, 0.45, 0.55), // far: short, dim, slow
    (0.66, 0.80, 0.85), // mid
    (1.00, 1.20, 1.20), // near: tall, bright, responsive
];
/// Blade length range as a fraction of the grass band height, per layer.
const LAYER_LEN: [(f32, f32); 3] = [(0.10, 0.18), (0.22, 0.38), (0.40, 0.75)];
/// Blades per column of width, per layer.
const LAYER_DENS: [f32; 3] = [0.30, 0.30, 0.36];

const BUILD_TIME: f32 = 2.1;
const SETTLE_TIME: f32 = 2.8;

/// Big-gust rhythm: Idle -> Build (lean) -> Sweep (front + shimmer) ->
/// Settle (decaying oscillation) -> Idle.
#[derive(Clone, Copy, PartialEq)]
enum Gust {
    Idle,
    Build(f32),  // elapsed
    Sweep(f32),  // elapsed
    Settle(f32), // elapsed
}

struct Blade {
    x: f32,
    len: f32,      // blade height in cells
    seed: f32,     // per-blade wind phase variation
    dew: bool,     // carries a glint on its tip
    tw: f32,       // twinkle clock
    tw_speed: f32, // twinkle rate
}

struct Star {
    x: f32,
    y: f32,
    phase: f32,
    speed: f32,
    mag: f32,
}

struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
}

struct Moon {
    x: f32,
    y: f32,
    r: f32,
    /// two dark maria patches, as offsets in units of r
    maria: [(f32, f32, f32); 2],
}

pub struct Meadow {
    rng: StdRng,
    detail: Detail,
    grass_c: (u8, u8, u8),
    tip_c: (u8, u8, u8),
    glint_c: (u8, u8, u8),
    horizon_c: (u8, u8, u8),
    star_c: (u8, u8, u8),
    meteor_c: (u8, u8, u8),
    moon_c: (u8, u8, u8),
    layers: [Vec<Blade>; 3],
    stars: Vec<Star>,
    meteor: Option<Meteor>,
    moon: Option<Moon>,
    next_meteor: f32,
    gust: Gust,
    gust_dir: f32,   // +1: front travels left->right
    gust_speed: f32, // front px/s
    next_gust: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Meadow {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (grass_c, tip_c, glint_c, horizon_c, star_c, meteor_c, moon_c) = match theme {
            Some("amber") => (
                (112, 84, 30),
                (206, 162, 74),
                (255, 224, 160),
                (26, 19, 7),
                (220, 190, 150),
                (255, 240, 210),
                (255, 226, 168),
            ),
            Some("jade") => (
                (26, 110, 72),
                (92, 204, 142),
                (200, 255, 225),
                (7, 24, 16),
                (170, 220, 195),
                (225, 255, 240),
                (210, 248, 226),
            ),
            // moonlit: pale blue-green blades, silver glints
            _ => (
                (52, 104, 96),
                (126, 196, 182),
                (215, 230, 235),
                (12, 26, 25),
                (160, 190, 205),
                (235, 245, 250),
                (224, 234, 238),
            ),
        };
        Meadow {
            rng,
            detail,
            grass_c,
            tip_c,
            glint_c,
            horizon_c,
            star_c,
            meteor_c,
            moon_c,
            layers: [Vec::new(), Vec::new(), Vec::new()],
            stars: Vec::new(),
            meteor: None,
            moon: None,
            next_meteor: 6.0,
            gust: Gust::Idle,
            gust_dir: 1.0,
            gust_speed: 80.0,
            next_gust: 10.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn horizon(&self) -> i32 {
        (self.h as f32 * 0.45) as i32
    }
}

impl Scene for Meadow {
    fn name(&self) -> &'static str {
        "meadow"
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
            let horizon = self.horizon();
            let span = (h as i32 - 1 - horizon).max(2) as f32;
            let dens = density_for(w, h).min(1.5);
            let rng = &mut self.rng;
            for (li, layer) in self.layers.iter_mut().enumerate() {
                let n = self
                    .detail
                    .scale(w as f32 * LAYER_DENS[li] * dens, 8)
                    .min(500);
                let (lo, hi) = LAYER_LEN[li];
                *layer = (0..n)
                    .map(|_| Blade {
                        x: rng.random_range(0.0..w as f32),
                        len: span * rng.random_range(lo..hi),
                        seed: rng.random::<f32>(),
                        dew: rng.random::<f32>() < 0.28,
                        tw: rng.random::<f32>(),
                        tw_speed: rng.random_range(0.6..1.8),
                    })
                    .collect();
            }
            let star_band = (horizon - 2).max(1) as f32;
            let sn = self
                .detail
                .scale((w * h / 900) as f32 * density_for(w, h), 6)
                .min(200);
            self.stars = (0..sn)
                .map(|_| Star {
                    x: rng.random_range(0.0..w as f32),
                    y: rng.random_range(0.0..star_band),
                    phase: rng.random_range(0.0..std::f32::consts::TAU),
                    speed: rng.random_range(0.4..1.6),
                    mag: rng.random_range(0.4..1.0),
                })
                .collect();
            // the moon: parked in the upper sky band, off-centre so the
            // azimuth lighting on the grass below is asymmetric and readable
            let r = ((h as f32 * 0.075).max(1.6)).min(w as f32 * 0.06).max(1.6);
            self.moon = if horizon > 4 {
                Some(Moon {
                    x: rng.random_range(w as f32 * 0.18..w as f32 * 0.82),
                    y: rng.random_range(r + 1.0..(horizon as f32 * 0.5).max(r + 1.5)),
                    r,
                    maria: [
                        (
                            rng.random_range(-0.35..0.15),
                            rng.random_range(-0.30..0.10),
                            rng.random_range(0.22..0.38),
                        ),
                        (
                            rng.random_range(0.05..0.40),
                            rng.random_range(0.05..0.40),
                            rng.random_range(0.16..0.30),
                        ),
                    ],
                })
            } else {
                None // no sky worth speaking of at this size
            };
            self.gust = Gust::Idle;
            self.meteor = None;
        }
        self.t += dt;

        // --- big gust scheduling (anticipation -> payoff -> decay) ---
        self.next_gust -= dt;
        match self.gust {
            Gust::Idle => {
                if self.next_gust <= 0.0 {
                    self.gust_dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
                    self.gust_speed = self.rng.random_range(60.0..110.0);
                    self.gust = Gust::Build(0.0);
                }
            }
            Gust::Build(e) => {
                let e = e + dt;
                self.gust = if e >= BUILD_TIME {
                    Gust::Sweep(0.0)
                } else {
                    Gust::Build(e)
                };
            }
            Gust::Sweep(e) => {
                let e = e + dt;
                let span = w as f32 + 60.0;
                let dur = (span / self.gust_speed).max(0.5);
                self.gust = if e >= dur { Gust::Settle(0.0) } else { Gust::Sweep(e) };
            }
            Gust::Settle(e) => {
                let e = e + dt;
                if e >= SETTLE_TIME {
                    self.gust = Gust::Idle;
                    self.next_gust = self.rng.random_range(20.0..30.0);
                } else {
                    self.gust = Gust::Settle(e);
                }
            }
        }

        // per-frame gust geometry
        let build_lean = match self.gust {
            Gust::Build(e) => -self.gust_dir * ease_smooth(e / BUILD_TIME) * 0.5,
            _ => 0.0,
        };
        let settle_k = match self.gust {
            Gust::Settle(e) => (1.0 - ease_smooth(e / SETTLE_TIME)) * self.gust_dir,
            _ => 0.0,
        };
        let settle_e = match self.gust {
            Gust::Settle(e) => e,
            _ => 0.0,
        };
        let (front_x, sweep_env) = match self.gust {
            Gust::Sweep(e) => {
                let span = w as f32 + 60.0;
                let dur = (span / self.gust_speed).max(0.5);
                let p = (e / dur).clamp(0.0, 1.0);
                let fx = if self.gust_dir > 0.0 {
                    -30.0 + span * p
                } else {
                    w as f32 + 30.0 - span * p
                };
                (fx, (p * std::f32::consts::PI).sin())
            }
            _ => (0.0, 0.0),
        };
        let sig = (w as f32 / 10.0).clamp(4.0, 16.0);

        // --- shooting star scheduling ---
        self.next_meteor -= dt;
        if self.next_meteor <= 0.0 && self.meteor.is_none() {
            let horizon = self.horizon();
            let dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
            self.meteor = Some(Meteor {
                x: self.rng.random_range(w as f32 * 0.1..w as f32 * 0.9),
                y: self.rng.random_range(0.0..(horizon as f32 * 0.4).max(1.0)),
                vx: dir * self.rng.random_range(35.0..75.0),
                vy: self.rng.random_range(10.0..24.0),
                age: 0.0,
                life: self.rng.random_range(0.7..1.2),
            });
            self.next_meteor = self.rng.random_range(7.0..18.0);
        }
        if let Some(m) = &mut self.meteor {
            m.age += dt;
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            if m.age >= m.life || m.x < -12.0 || m.x > w as f32 + 12.0 || m.y as i32 >= self.horizon()
            {
                self.meteor = None;
            }
        }

        // --- draw ---
        canvas.clear((0, 0, 0));

        // dim stars, slow individual twinkle
        for s in &self.stars {
            let tw = 0.30 + 0.70 * (0.5 + 0.5 * (self.t * s.speed + s.phase).sin()).powi(2);
            canvas.set_f(s.x, s.y, scale(self.star_c, s.mag * tw * 0.55));
        }

        // breathing moon: a slow luminance swell, limb-darkened, with two dark
        // maria. Drawn in one pass rather than by stacking discs — overdrawing
        // a dark disc on a bright one leaves a bright antialiased ring around
        // each patch, which is exactly the artifact this avoids.
        let (moon_x, moon_light) = match &self.moon {
            Some(m) => {
                let breathe = 0.88 + 0.12 * (self.t * 0.21).sin();
                // halo: two nested falloffs so the glow has no single hard ring
                let (mxi, myi) = (m.x as i32, m.y as i32);
                glow(canvas, mxi, myi, (m.r * 3.2) as i32, self.moon_c, 0.09 * breathe);
                glow(canvas, mxi, myi, (m.r * 1.7) as i32, self.moon_c, 0.15 * breathe);
                let x0 = (m.x - m.r - 1.0).floor() as i32;
                let x1 = (m.x + m.r + 1.0).ceil() as i32;
                let y0 = (m.y - m.r - 1.0).floor() as i32;
                let y1 = (m.y + m.r + 1.0).ceil() as i32;
                for y in y0..=y1 {
                    let dy = y as f32 - m.y;
                    for x in x0..=x1 {
                        let dx = x as f32 - m.x;
                        let d = (dx * dx + dy * dy).sqrt();
                        let cov = (m.r + 0.5 - d).clamp(0.0, 1.0);
                        if cov <= 0.0 {
                            continue;
                        }
                        // limb darkening toward the edge
                        let mut v = breathe * (1.0 - 0.20 * (d / m.r).min(1.0).powi(2));
                        for &(mx, my, mr) in &m.maria {
                            let (ax, ay) = (dx - mx * m.r, dy - my * m.r);
                            let md = (ax * ax + ay * ay).sqrt() / (mr * m.r).max(0.001);
                            if md < 1.0 {
                                v *= 1.0 - 0.26 * (1.0 - md * md);
                            }
                        }
                        let c = scale(self.moon_c, v);
                        if cov >= 0.999 {
                            canvas.set(x, y, c);
                        } else {
                            canvas.add(x, y, scale(c, cov));
                        }
                    }
                }
                (m.x, breathe)
            }
            None => (0.0, 0.0),
        };

        // shooting star: eased envelope, fading additive trail
        if let Some(m) = &self.meteor {
            let p = (m.age / m.life).clamp(0.0, 1.0);
            let env = if p < 0.15 {
                ease_smooth(p / 0.15)
            } else {
                1.0 - ease_smooth((p - 0.15) / 0.85)
            };
            let trail = 8;
            for i in 1..=trail {
                let k = i as f32 / trail as f32;
                let tx = m.x - m.vx * k * 0.09;
                let ty = m.y - m.vy * k * 0.09;
                canvas.add(
                    tx as i32,
                    ty as i32,
                    scale(self.meteor_c, env * (1.0 - k) * (1.0 - k) * 0.8),
                );
            }
            canvas.set_f(m.x, m.y, scale(self.meteor_c, env));
            if env > 0.55 {
                glow(canvas, m.x as i32, m.y as i32, 1, self.meteor_c, env * 0.35);
            }
        }

        // faint horizon line with a little texture
        let horizon = self.horizon();
        for x in 0..w {
            let tex = 0.65 + 0.7 * fbm(x as f32 * 0.12, 3.3, 2, 77);
            canvas.set(x as i32, horizon, scale(self.horizon_c, tex));
        }

        // moon haze pooling along the skyline: brightest directly under the
        // moon's azimuth, spilling a couple of rows either side of the horizon
        if moon_light > 0.0 {
            let reach = (w as f32 * 0.42).max(6.0);
            for dy in -2i32..=1 {
                let row = horizon + dy;
                if row < 0 || row >= h as i32 {
                    continue;
                }
                // the pool is densest right at the skyline
                let vk = 1.0 - (dy as f32 + 0.5).abs() / 2.6;
                if vk <= 0.0 {
                    continue;
                }
                for x in 0..w as i32 {
                    let d = ((x as f32 - moon_x).abs() / reach).min(1.0);
                    let k = (1.0 - d * d) * vk * moon_light * 0.16;
                    if k > 0.004 {
                        canvas.add(x, row, scale(self.moon_c, k));
                    }
                }
            }
        }

        // grass, far to near so near blades occlude far ones
        let bases = [
            horizon,
            horizon + (h as i32 - 1 - horizon) / 2,
            h as i32 - 1,
        ];
        for (li, layer) in self.layers.iter_mut().enumerate() {
            let (bright, amp, ts) = LAYERS[li];
            let base = bases[li];
            for b in layer.iter_mut() {
                // ambient fBm wind + a traveling gust wave (phase offset by x)
                let wind = fbm(b.x * 0.055 + b.seed * 7.0, self.t * 0.5 * ts, 3, 101);
                let wave = fbm(b.x * 0.03 - self.t * 0.85 * ts, 13.7 + b.seed * 3.0, 2, 202);
                let mut bend = ((wind - 0.5) * 1.1 + (wave - 0.5) * 1.3) * amp;
                bend += build_lean * amp;
                bend += settle_k * (settle_e * 6.5 + b.seed * 3.0).sin() * 0.7 * amp;
                // big-gust front: strong localized bend + light catch
                let mut inf = 0.0;
                if sweep_env > 0.0 {
                    let d = b.x - front_x;
                    inf = (-(d * d) / (2.0 * sig * sig)).exp() * sweep_env;
                    bend += self.gust_dir * inf * 2.1 * amp;
                }

                // blades beneath the moon's azimuth catch extra light
                let az = if moon_light > 0.0 {
                    let d = ((b.x - moon_x).abs() / (w as f32 * 0.35)).min(1.0);
                    1.0 + 0.26 * moon_light * (1.0 - d * d)
                } else {
                    1.0
                };

                let max_lean = b.len * 0.85;
                let lean = (bend * b.len * 0.35).clamp(-max_lean, max_lean);
                let segs = b.len.ceil().max(1.0) as i32;
                let shade = (1.0 + bend * 0.10 + inf * 0.4) * az;
                let mut tip = (b.x as i32, base);
                for i in 0..segs {
                    let t = i as f32 / segs as f32;
                    let xi = (b.x + lean * t * t) as i32;
                    let yi = base - i;
                    tip = (xi, yi);
                    let c = scale(self.grass_c, bright * (0.30 + 0.55 * t));
                    let c = if t > 0.7 {
                        lerp(c, scale(self.tip_c, bright), (t - 0.7) / 0.3 * 0.55)
                    } else {
                        c
                    };
                    canvas.set(xi, yi, scale(c, shade));
                }

                // dew glint on the tip: sparse twinkle, flares in gusts
                if b.dew {
                    b.tw += dt * b.tw_speed;
                    let tw = 0.5 + 0.5 * (b.tw * std::f32::consts::TAU + b.seed * 9.0).sin();
                    let sparkle = tw * tw * tw;
                    let mut gb = sparkle * 0.28 * az + inf * 1.1;
                    gb += (bend.abs() - 0.35).max(0.0) * 0.5;
                    if gb > 0.06 {
                        let gb = gb.min(1.2);
                        canvas.add(tip.0, tip.1, scale(self.glint_c, gb));
                        if gb > 0.75 && li == 2 {
                            glow(canvas, tip.0, tip.1, 1, self.glint_c, gb * 0.35);
                        }
                    }
                }
            }
        }
    }
}
