//! Rain on glass: layered droplet trails with wobble and bottom splashes,
//! clinging statics that swell when near drops pass, wind gusts that skew
//! trails and spike splash rate, and lightning that backlights the skyline
//! with theme-aware refraction tint (neon bleed on the neon theme). Soft
//! painted window-edge vignette keeps the pane reading as glass.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

struct Drop {
    x: f32,
    y: f32,
    speed: f32,
    trail: usize,
    bright: f32,
    phase: f32,
    wobble: f32,
    near: bool,
}

struct Splash {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

/// Cling static: (x, y, brightness, radius swell).
struct Cling {
    x: f32,
    y: f32,
    bright: f32,
    radius: f32,
}

/// Spawn a drop. `near` selects the depth layer: far drops are dimmer and
/// faster, near drops brighter and slower.
fn spawn(rng: &mut StdRng, w: usize, near: bool) -> Drop {
    let (speed, trail, bright) = if near {
        (
            rng.random_range(28.0..60.0),
            rng.random_range(7..15),
            rng.random_range(0.75..1.0),
        )
    } else {
        (
            rng.random_range(70.0..150.0),
            rng.random_range(3..7),
            rng.random_range(0.25..0.5),
        )
    };
    Drop {
        x: rng.random_range(0.0..w as f32),
        y: rng.random_range(-(w.max(20) as f32)..0.0),
        speed,
        trail,
        bright,
        phase: rng.random_range(0.0..std::f32::consts::TAU),
        wobble: rng.random_range(0.4..1.6),
        near,
    }
}

pub struct Rain {
    rng: StdRng,
    detail: Detail,
    neon: bool,
    bg_top: (u8, u8, u8),
    bg_bot: (u8, u8, u8),
    near_c: (u8, u8, u8),
    far_c: (u8, u8, u8),
    far: Vec<Drop>,
    near: Vec<Drop>,
    splashes: Vec<Splash>,
    /// static drops clinging to the glass; running drops absorb them
    statics: Vec<Cling>,
    static_timer: f32,
    skyline: Vec<i32>,
    gust: f32,
    gust_t: f32,
    next_gust: f32,
    /// Peak splash multiplier during gust payoff.
    gust_splash: f32,
    lightning: f32,
    next_lightning: f32,
    /// countdown to the follow-up stroke of a lightning flash (0 = none)
    stroke2: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Rain {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let neon = matches!(theme, Some("neon"));
        let (bg_top, bg_bot, near_c, far_c) = match theme {
            Some("storm") => ((3, 4, 8), (12, 15, 24), (170, 190, 225), (85, 105, 140)),
            Some("neon") => ((8, 4, 12), (22, 10, 28), (230, 120, 220), (120, 50, 130)),
            Some("window-day") => ((12, 14, 20), (30, 34, 46), (205, 220, 240), (140, 155, 175)),
            _ => ((4, 6, 11), (14, 18, 28), (150, 175, 215), (70, 90, 125)),
        };
        Rain {
            rng,
            detail,
            neon,
            bg_top,
            bg_bot,
            near_c,
            far_c,
            far: Vec::new(),
            near: Vec::new(),
            splashes: Vec::new(),
            statics: Vec::new(),
            static_timer: 0.0,
            skyline: Vec::new(),
            gust: 0.0,
            gust_t: 0.0,
            next_gust: 6.0,
            gust_splash: 0.0,
            lightning: 0.0,
            next_lightning: 14.0,
            stroke2: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_layer(
        drops: &mut Vec<Drop>,
        rng: &mut StdRng,
        splashes: &mut Vec<Splash>,
        statics: &mut Vec<Cling>,
        gust: f32,
        gust_splash: f32,
        colors: ((u8, u8, u8), (u8, u8, u8)),
        canvas: &mut Canvas,
        dt: f32,
        t: f32,
        flash: f32,
    ) {
        let (w, h) = (canvas.width(), canvas.height());
        drops.retain_mut(|d| {
            // near drops creep down the glass with a stutter; far rain falls steady
            let stut = if d.near {
                0.8 + 0.35 * (t * 1.7 + d.phase * 3.0).sin()
            } else {
                1.0
            };
            d.y += d.speed * stut * dt * (1.0 + gust.abs() * 0.15);
            if d.y - d.trail as f32 > h as f32 {
                if d.near && rng.random::<f32>() < 0.7 + gust_splash * 0.25 {
                    // impact flash on the gutter where the drop lands
                    glow(canvas, d.x as i32, h as i32 - 1, 1, colors.0, 0.30);
                    let n = rng.random_range(2..5) + (gust_splash * 3.0) as usize;
                    for _ in 0..n {
                        splashes.push(Splash {
                            x: d.x,
                            y: h as f32 - 1.0,
                            vx: rng.random_range(-24.0..24.0) + gust * 18.0,
                            vy: rng.random_range(-70.0..-25.0),
                            life: rng.random_range(0.4..0.9),
                        });
                    }
                }
                *d = spawn(rng, w, d.near);
            }
            // absorb / swell static drops the head passes
            if d.near {
                let hx = d.x + (d.y * 0.35 + d.phase).sin() * d.wobble + gust * d.y * 0.05;
                let mut i = 0;
                while i < statics.len() {
                    let s = &mut statics[i];
                    let dist = ((s.x - hx).powi(2) + (s.y - d.y).powi(2)).sqrt();
                    if dist < 2.5 {
                        // swell then absorb when very close
                        s.radius = (s.radius + dt * 2.5).min(2.2);
                        s.bright = (s.bright + dt * 0.8).min(1.0);
                        if dist < 1.2 {
                            statics.remove(i);
                            d.trail = (d.trail + 1).min(20);
                            d.bright = (d.bright + 0.08).min(1.0);
                            d.speed *= 1.04;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
            let xw = d.x + (d.y * 0.35 + d.phase).sin() * d.wobble + gust * d.y * 0.05;
            let base = if d.near { colors.0 } else { colors.1 };
            for i in 0..d.trail {
                let yy = d.y - i as f32;
                if yy < 0.0 {
                    break;
                }
                let f = (1.0 - i as f32 / d.trail as f32).powi(2) * d.bright;
                // head of a near drop gets a bright glint; lightning backlights it
                let c = if i == 0 && d.near {
                    lerp(scale(base, f), (235, 245, 255), 0.55 + flash * 0.35)
                } else {
                    scale(base, f * (1.0 + flash * 0.5))
                };
                canvas.set(xw as i32, yy as i32, c);
            }
            // soft halo around near-drop heads so they read as beads on glass
            if d.near && d.y >= 0.0 && d.y < h as f32 {
                glow(
                    canvas,
                    xw as i32,
                    d.y as i32,
                    1,
                    base,
                    0.20 * d.bright + flash * 0.25,
                );
            }
            true
        });
    }
}

impl Scene for Rain {
    fn name(&self) -> &'static str {
        "rain"
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
            let f = self.detail.factor();
            let dens = density_for(w, h);
            self.far = (0..((w / 2 + 12) as f32 * f * dens) as usize + 6)
                .map(|_| spawn(&mut self.rng, w, false))
                .collect();
            self.near = (0..((w / 4 + 8) as f32 * f * dens) as usize + 4)
                .map(|_| spawn(&mut self.rng, w, true))
                .collect();
            self.statics = (0..((w / 2 + 14) as f32 * f * dens) as usize + 10)
                .map(|_| Cling {
                    x: self.rng.random_range(0.0..w as f32),
                    y: self.rng.random_range(0.0..h as f32),
                    bright: self.rng.random_range(0.2..0.5),
                    radius: self.rng.random_range(0.6..1.1),
                })
                .collect();
            // distant skyline silhouette along the bottom
            let mut sky = Vec::with_capacity(w);
            let mut x = 0;
            while x < w {
                let bw = self.rng.random_range(3..9);
                let bh = self.rng.random_range(2..(h / 8).max(3) as i32);
                for _ in 0..bw {
                    sky.push(bh);
                }
                x += bw;
            }
            sky.truncate(w);
            self.skyline = sky;
        }
        self.t += dt;

        // wind gusts: build → peak slant + splash spike → decay
        self.next_gust -= dt;
        if self.next_gust <= 0.0 {
            self.gust_t = self.rng.random_range(2.5..4.5);
            self.next_gust = self.rng.random_range(8.0..16.0);
        }
        if self.gust_t > 0.0 {
            self.gust_t -= dt;
            let peak = ease_smooth((2.5 - self.gust_t).max(0.0) / 1.0)
                * ease_smooth(self.gust_t / 1.2);
            self.gust_splash = peak;
            let target = if (self.t / 9.0) as i32 % 2 == 0 {
                1.6
            } else {
                -1.6
            };
            self.gust += (target * (0.4 + peak) - self.gust) * dt * 1.8;
        } else {
            self.gust *= 1.0 - dt.min(1.0);
            self.gust_splash = (self.gust_splash - dt * 1.5).max(0.0);
        }

        // rare distant lightning: a flash, often chased by a weaker second stroke
        self.next_lightning -= dt;
        if self.next_lightning <= 0.0 {
            self.lightning = 1.0;
            self.stroke2 = if self.rng.random::<f32>() < 0.6 {
                self.rng.random_range(0.10..0.22)
            } else {
                0.0
            };
            self.next_lightning = self.rng.random_range(12.0..26.0);
        }
        if self.stroke2 > 0.0 {
            self.stroke2 -= dt;
            if self.stroke2 <= 0.0 {
                self.lightning = self.lightning.max(0.75);
            }
        }
        self.lightning = (self.lightning - dt * 3.0).max(0.0);

        // static drops slowly re-form on the glass
        let static_cap =
            ((w / 2 + 14) as f32 * self.detail.factor() * density_for(w, h)) as usize + 12;
        self.static_timer += dt;
        if self.static_timer > 0.35 && self.statics.len() < static_cap {
            self.static_timer = 0.0;
            self.statics.push(Cling {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                bright: self.rng.random_range(0.2..0.5),
                radius: self.rng.random_range(0.6..1.1),
            });
        }
        // slow radius settle when idle
        for s in &mut self.statics {
            s.radius += (1.0 - s.radius) * dt * 0.4;
        }

        // background: dark sky gradient, lighter near the horizon, with slow
        // cloud shading drifting across; lightning washes it (neon bleed on neon)
        let fl = self.lightning * self.lightning;
        let flash_tint = if self.neon {
            (180, 90, 220)
        } else {
            (120, 140, 180)
        };
        for y in 0..h {
            let base = lerp(self.bg_top, self.bg_bot, y as f32 / h.max(1) as f32);
            for x in 0..w {
                let cloud = ((x as f32 * 0.06 + self.t * 0.25).sin()
                    * (y as f32 * 0.11 - self.t * 0.1).cos())
                    * 0.5
                    + 0.5;
                let mut c = lerp(scale(base, 0.7 + cloud * 0.5), flash_tint, fl * 0.55);
                // soft window-edge vignette (painted, not empty)
                let nx = x as f32 / w as f32;
                let ny = y as f32 / h as f32;
                let edge = ((nx.min(1.0 - nx) * 8.0).min(1.0)
                    * (ny.min(1.0 - ny) * 6.0).min(1.0))
                    .clamp(0.0, 1.0);
                let vig = 0.72 + 0.28 * ease_smooth(edge);
                // sill hint along the bottom edge
                let sill = if ny > 0.92 {
                    1.0 - (ny - 0.92) / 0.08
                } else {
                    1.0
                };
                c = scale(c, vig * (0.85 + 0.15 * sill));
                canvas.set(x as i32, y as i32, c);
            }
        }

        // distant skyline silhouette with a few lit windows; lightning rims it
        // gust payoff brightens the silhouette
        let sky_boost = fl * 0.8 + self.gust_splash * 0.35;
        for (x, &bh) in self.skyline.iter().enumerate() {
            let top = (h as i32 - bh).max(0);
            for y in top..h as i32 {
                canvas.set(x as i32, y, (10, 13, 20));
            }
            if sky_boost > 0.02 {
                canvas.add(x as i32, top, scale(flash_tint, sky_boost));
            }
            // sparse dim windows flickering slowly, deterministic per column
            if bh > 3 && (x * 37 % 11) < 2 {
                let on = ((x * 37) as f32 + self.t * 0.3).sin() > -0.6;
                if on {
                    let warm = lerp((96, 72, 30), flash_tint, fl * 0.6);
                    canvas.set(x as i32, h as i32 - bh + 1, warm);
                }
            }
        }

        // static clinging drops (with swell radius)
        for s in &self.statics {
            let col = scale(self.near_c, s.bright * 0.55);
            canvas.set(s.x as i32, s.y as i32, col);
            if s.radius > 1.15 {
                glow(
                    canvas,
                    s.x as i32,
                    s.y as i32,
                    1,
                    self.near_c,
                    (s.radius - 1.0) * 0.35 * s.bright,
                );
            }
        }

        // gutter line at the very bottom: slow drips + expanding rings
        for i in 0..6 {
            let fi = i as f32;
            let rx = ((self.t * 3.0 + fi * 47.0) % (w as f32 + 20.0)) - 10.0;
            let ring_r = ((self.t * 2.0 + fi * 1.3) % 3.0) as i32;
            let alpha = 0.25 * (1.0 - ring_r as f32 / 3.0);
            for a in 0..12 {
                let ang = a as f32 / 12.0 * std::f32::consts::TAU;
                canvas.add(
                    (rx + ang.cos() * ring_r as f32 * 1.6) as i32,
                    h as i32 - 2 + (ang.sin() * ring_r as f32 * 0.4) as i32,
                    scale((100, 130, 170), alpha),
                );
            }
        }

        // Splashes: small additive marks that bounce once off the glass.
        self.splashes.retain_mut(|s| {
            s.life -= dt * 2.4;
            s.vy += 220.0 * dt;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            // bounce arc off the bottom with energy loss
            if s.y >= h as f32 - 1.0 && s.vy > 0.0 {
                s.y = h as f32 - 1.0;
                s.vy = -s.vy * 0.35;
                s.vx *= 0.6;
            }
            if s.life > 0.0 && s.y < h as f32 {
                let b = (s.life * 90.0) as u8;
                canvas.add(s.x as i32, s.y as i32, (b / 2, b / 2 + b / 4, b));
                true
            } else {
                false
            }
        });

        // lightning refraction tint on glass (additive cool / neon wash)
        if fl > 0.02 {
            let tint = if self.neon {
                scale((255, 80, 220), fl * 0.12)
            } else {
                scale((180, 210, 255), fl * 0.10)
            };
            for y in 0..h {
                for x in (0..w).step_by(2) {
                    canvas.add(x as i32, y as i32, tint);
                }
            }
        }

        let mut splashes = std::mem::take(&mut self.splashes);
        let mut statics = std::mem::take(&mut self.statics);
        let gust = self.gust;
        let gust_splash = self.gust_splash;
        let (nc, fc) = (self.near_c, self.far_c);
        let flash = fl;
        let t = self.t;
        Self::draw_layer(
            &mut self.far,
            &mut self.rng,
            &mut splashes,
            &mut statics,
            gust,
            gust_splash,
            (nc, fc),
            canvas,
            dt,
            t,
            flash,
        );
        Self::draw_layer(
            &mut self.near,
            &mut self.rng,
            &mut splashes,
            &mut statics,
            gust,
            gust_splash,
            (nc, fc),
            canvas,
            dt,
            t,
            flash,
        );
        self.splashes = splashes;
        self.statics = statics;
    }
}
