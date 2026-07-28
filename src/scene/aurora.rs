//! Aurora borealis: translucent sine-warped curtains of green/teal/violet
//! with sharp vertical ray structure over a dark starry sky, a periodic
//! intensity swell that bleeds onto the horizon, occasional shooting stars,
//! and a wobble-distorted frozen-lake reflection along the bottom.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const TAU: f32 = std::f32::consts::TAU;

struct Star {
    x: i32,
    y: i32,
    bright: f32,
    phase: f32,
    twinkle: bool,
    hero: bool, // rare bright star with a soft glow halo
    warm: bool, // tint: cool blue-white or warm white
}

struct Curtain {
    color: (u8, u8, u8),
    alpha: f32,
    ybase: f32,
    amp: f32,
    freq: f32,
    speed: f32,
    phase: f32,
    len: f32,
    rays: f32,      // vertical ray count across the width
    ray_drift: f32, // how fast the rays slide along the curtain
}

struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

pub struct Aurora {
    rng: StdRng,
    stars: Vec<Star>,
    curtains: Vec<Curtain>,
    meteor: Option<Meteor>,
    next_meteor: f32,
    swell: f32,
    next_swell: f32,
    detail: Detail,
    t: f32,
    w: usize,
    h: usize,
}

impl Aurora {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let curtain_colors: Vec<((u8, u8, u8), f32)> = match theme {
            Some("crimson") => vec![
                ((255, 60, 90), 0.45),
                ((220, 40, 60), 0.55),
                ((255, 120, 90), 0.7),
            ],
            Some("arctic") => vec![
                ((120, 160, 255), 0.45),
                ((90, 220, 235), 0.55),
                ((190, 245, 255), 0.7),
            ],
            _ => vec![
                ((140, 70, 255), 0.45),
                ((30, 215, 190), 0.55),
                ((70, 255, 145), 0.7),
            ],
        };
        let curtains = vec![
            // back: tall, faint, fine rays
            Curtain {
                color: curtain_colors[0].0,
                alpha: curtain_colors[0].1,
                ybase: 0.08,
                amp: 0.10,
                freq: 0.9,
                speed: 0.35,
                phase: rng.random_range(0.0..TAU),
                len: 0.50,
                rays: 10.0,
                ray_drift: 0.55,
            },
            // middle
            Curtain {
                color: curtain_colors[1].0,
                alpha: curtain_colors[1].1,
                ybase: 0.14,
                amp: 0.13,
                freq: 1.3,
                speed: 0.5,
                phase: rng.random_range(0.0..TAU),
                len: 0.42,
                rays: 7.0,
                ray_drift: 0.4,
            },
            // front: shortest, brightest, broad rays, reaches deepest
            Curtain {
                color: curtain_colors[2].0,
                alpha: curtain_colors[2].1,
                ybase: 0.20,
                amp: 0.16,
                freq: 1.7,
                speed: 0.65,
                phase: rng.random_range(0.0..TAU),
                len: 0.36,
                rays: 5.0,
                ray_drift: 0.28,
            },
        ];
        Aurora {
            rng,
            stars: Vec::new(),
            curtains,
            swell: 0.0,
            next_swell: 20.0,
            meteor: None,
            next_meteor: 5.0,
            t: 0.0,
            detail,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let n = self
            .detail
            .scale((w * h / 140) as f32 * density_for(w, h), 20)
            .min(500);
        self.stars = (0..n)
            .map(|_| Star {
                x: self.rng.random_range(0..w.max(1) as i32),
                y: self.rng.random_range(0..(h as f32 * 0.7).max(1.0) as i32),
                bright: self.rng.random_range(0.25..1.0),
                phase: self.rng.random_range(0.0..TAU),
                twinkle: self.rng.random::<f32>() < 0.2,
                hero: self.rng.random::<f32>() < 0.05,
                warm: self.rng.random::<f32>() < 0.3,
            })
            .collect();
    }
}

impl Scene for Aurora {
    fn name(&self) -> &'static str {
        "aurora"
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
        let hf = h as f32;

        // pure black night sky
        canvas.clear((0, 0, 0));

        // stars, some twinkling; a few hero stars carry a soft halo
        for s in &self.stars {
            let b = if s.twinkle {
                let tw = 0.5 + 0.5 * (t * 2.5 + s.phase).sin();
                s.bright * (0.35 + 0.65 * tw * tw)
            } else {
                s.bright
            };
            let tint = if s.warm {
                (235, 225, 200)
            } else {
                (190, 200, 230)
            };
            canvas.set(s.x, s.y, scale(tint, b * 0.8));
            if s.hero {
                glow(canvas, s.x, s.y, 1, tint, b * 0.35);
            }
        }

        // intensity swell: anticipation -> payoff -> decay over ~14s
        self.next_swell -= dt;
        if self.next_swell <= 0.0 {
            self.swell = 14.0;
            self.next_swell = self.rng.random_range(25.0..40.0);
        }
        self.swell = (self.swell - dt).max(0.0);
        let swell_env = if self.swell > 0.0 {
            ease_smooth(1.0 - (self.swell / 14.0 - 0.5).abs() * 2.0)
        } else {
            0.0
        };
        let swell_k = 1.0 + 0.8 * swell_env;

        // swell bleeds onto the horizon: a faint band of the front color
        // (drawn before the lake so the reflection picks it up too)
        if swell_env > 0.01 {
            let front = self.curtains[2].color;
            let gy = (hf * 0.62) as i32;
            for dy in 0..3 {
                let row_k = 1.0 - dy as f32 * 0.35;
                for x in 0..w {
                    let xf = x as f32 / w as f32;
                    let band = swell_env * 0.10 * row_k * (0.7 + 0.3 * (xf * 5.0 + t * 0.3).sin());
                    canvas.add(x as i32, gy + dy, scale(front, band));
                }
            }
        }

        // curtains back to front; light is additive so layers blend
        for c in &self.curtains {
            for x in 0..w {
                let xf = x as f32 / w as f32;
                // swaying, rippling top edge plus a slow large-scale roll
                let edge = (c.ybase
                    + c.amp
                        * (xf * 6.0 * c.freq + t * c.speed + c.phase).sin()
                        * (0.6 + 0.4 * (xf * 2.3 * c.freq - t * c.speed * 0.6).sin())
                    + 0.04 * (xf * 2.5 + t * 0.12 + c.phase).sin())
                    * hf;
                // distinct vertical rays drifting along the curtain, leaning
                // slightly with a low-frequency warp
                let ray = 0.5
                    + 0.5
                        * (xf * c.rays * TAU
                            + t * c.ray_drift
                            + c.phase
                            + 0.6 * (xf * 2.0 + t * 0.05).sin())
                        .sin();
                let streak = ray * ray * ray;
                let len_px = c.len * hf;
                let y0 = edge.max(0.0) as i32;
                let y1 = ((edge + len_px) as i32).min(h as i32);
                for y in y0..y1 {
                    let d = (y as f32 - edge) / len_px;
                    if d < 0.0 {
                        continue;
                    }
                    // brightest just below the edge, exponential falloff;
                    // dim gaps between rays keep the ribbon structure readable
                    let fall = (-d * 3.2_f32).exp();
                    let b = c.alpha
                        * fall
                        * (0.25 + 1.1 * streak)
                        * (0.75 + 0.25 * (t * 0.4 + c.phase).sin())
                        * swell_k;
                    canvas.add(x as i32, y, scale(c.color, b));
                }
            }
        }

        // occasional shooting star: eased life envelope + glowing head
        self.next_meteor -= dt;
        if self.next_meteor <= 0.0 && self.meteor.is_none() {
            self.meteor = Some(Meteor {
                x: self.rng.random_range(w as f32 * 0.3..w as f32),
                y: self.rng.random_range(0.0..h as f32 * 0.2),
                vx: -self.rng.random_range(60.0..100.0),
                vy: self.rng.random_range(15.0..30.0),
                life: 0.7,
            });
            self.next_meteor = self.rng.random_range(6.0..14.0);
        }
        if let Some(m) = &mut self.meteor {
            m.life -= dt;
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            if m.life <= 0.0 {
                self.meteor = None;
            } else {
                let prog = 1.0 - m.life / 0.7; // 0 at spawn → 1 at burnout
                let env = ease_smooth((prog / 0.18).min(1.0))
                    * (1.0 - ease_smooth((prog - 0.55).max(0.0) / 0.45));
                for i in 0..6 {
                    let f = 1.0 - i as f32 / 6.0;
                    canvas.set_f(
                        m.x - m.vx * 0.02 * i as f32,
                        m.y - m.vy * 0.02 * i as f32,
                        scale((240, 245, 255), f * env),
                    );
                }
                glow(
                    canvas,
                    m.x as i32,
                    m.y as i32,
                    1,
                    (240, 245, 255),
                    env * 0.5,
                );
            }
        }

        // frozen-lake reflection: bottom 15%, wobble-distorted and dimmer,
        // brightening with the swell so events touch the ground too
        let refl_top = (h as f32 * 0.85) as i32;
        for y in refl_top..h as i32 {
            let depth = y - refl_top;
            let src_y = refl_top - depth;
            let wob = ((y as f32 * 0.5 + t * 1.4 + depth as f32 * 0.4).sin() * 2.0) as i32;
            for x in 0..w as i32 {
                let sx = x + wob;
                let src = canvas.get(sx, src_y).color;
                let dim = (0.36 + 0.15 * swell_env)
                    * (1.0 - depth as f32 / (h - refl_top as usize) as f32 * 0.5);
                canvas.set(x, y, scale(src, dim));
            }
        }
    }
}
