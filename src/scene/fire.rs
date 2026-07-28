//! Doom-style fire: heat map with cooling, upward propagation and wobble,
//! mapped through a black → dark red → red → orange → yellow → white palette.
//! Cooling increases with altitude so the flames taper into a night sky with
//! faint stars; glowing embers and smoke wisps drift through the dark. A
//! stoke event runs on an anticipation → payoff → decay envelope: the base
//! swells, a burst of embers kicks off, and the log rims answer the light.

use super::{noise::fbm, Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

/// Fire palette families.
fn stops_for(theme: Option<&str>) -> &'static [(f32, (u8, u8, u8))] {
    match theme {
        Some("inferno") => &[
            (0.00, (0, 0, 0)),
            (0.15, (60, 0, 0)),
            (0.35, (180, 20, 0)),
            (0.55, (255, 90, 0)),
            (0.75, (255, 190, 40)),
            (0.90, (255, 240, 150)),
            (1.00, (255, 255, 255)),
        ],
        Some("emerald") => &[
            (0.00, (0, 0, 0)),
            (0.12, (0, 26, 8)),
            (0.28, (6, 90, 22)),
            (0.45, (16, 180, 40)),
            (0.62, (60, 235, 80)),
            (0.78, (140, 255, 130)),
            (0.90, (200, 255, 180)),
            (1.00, (240, 255, 240)),
        ],
        Some("frost") => &[
            (0.00, (0, 0, 0)),
            (0.12, (4, 8, 32)),
            (0.28, (10, 30, 110)),
            (0.45, (20, 80, 200)),
            (0.62, (50, 150, 255)),
            (0.78, (120, 210, 255)),
            (0.90, (190, 240, 255)),
            (1.00, (245, 255, 255)),
        ],
        _ => &[
            (0.00, (0, 0, 0)),
            (0.12, (32, 4, 0)),
            (0.28, (110, 12, 0)),
            (0.45, (200, 32, 0)),
            (0.62, (255, 80, 0)),
            (0.78, (255, 160, 20)),
            (0.90, (255, 220, 90)),
            (1.00, (255, 255, 220)),
        ],
    }
}

/// Fire palette: piecewise gradient tuned by eye.
#[cfg(test)]
pub fn palette(t: f32) -> (u8, u8, u8) {
    palette_themed(t, None)
}

pub fn palette_themed(t: f32, theme: Option<&str>) -> (u8, u8, u8) {
    let stops = stops_for(theme);
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        if t <= w[1].0 {
            let local = (t - w[0].0) / (w[1].0 - w[0].0);
            return lerp(w[0].1, w[1].1, local);
        }
    }
    stops.last().unwrap().1
}

struct Ember {
    x: f32,
    y: f32,
    vy: f32,
    life: f32,
    age: f32,
}

struct Wisp {
    x: f32,
    y: f32,
    life: f32,
    life0: f32,
    age: f32,
}

/// Faint background star in the night sky above the flames.
struct Star {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
}

pub struct Fire {
    rng: StdRng,
    detail: Detail,
    theme: Option<String>,
    heat: Vec<u8>,
    embers: Vec<Ember>,
    wisps: Vec<Wisp>,
    stars: Vec<Star>,
    logs: Vec<(i32, i32, i32, i32)>, // x, y, w, h
    /// stoke event: elapsed / duration; idle when flare_t >= flare_dur
    flare_t: f32,
    flare_dur: f32,
    /// eased flare envelope 0..1, recomputed each frame
    flare_k: f32,
    next_flare: f32,
    shimmer_t: f32,
    seed: u32,
    w: usize,
    h: usize,
}

impl Fire {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let seed = rng.random::<u32>();
        Fire {
            rng,
            detail,
            theme: theme.map(|t| t.to_string()),
            heat: Vec::new(),
            embers: Vec::new(),
            wisps: Vec::new(),
            stars: Vec::new(),
            logs: Vec::new(),
            flare_t: 1.0,
            flare_dur: 1.0, // idle
            flare_k: 0.0,
            next_flare: 6.0,
            shimmer_t: 0.0,
            seed,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.heat = vec![0; w * h];
        // dark log silhouettes at the base
        self.logs = (0..self.rng.random_range(2..=4))
            .map(|_| {
                let lw = self.rng.random_range(w / 6..w / 3).max(4) as i32;
                (
                    self.rng.random_range(0..(w as i32 - lw).max(1)),
                    h as i32 - self.rng.random_range(2..=4),
                    lw,
                    self.rng.random_range(2..=3),
                )
            })
            .collect();
        // sparse dim stars in the upper sky (background plane)
        let n = self
            .detail
            .scale((w * h / 260) as f32 * density_for(w, h), 5)
            .min(120);
        self.stars = (0..n)
            .map(|_| Star {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32 * 0.45),
                mag: self.rng.random_range(0.05..0.17),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            })
            .collect();
    }

    /// Eased stoke envelope: fast attack, brief hold, long decay.
    fn flare_envelope(&self) -> f32 {
        if self.flare_t >= self.flare_dur {
            return 0.0;
        }
        let u = self.flare_t / self.flare_dur;
        ease_smooth((u / 0.2).min(1.0)) * (1.0 - ease_smooth(((u - 0.45) / 0.55).max(0.0)))
    }

    /// One propagation step. Exposed for tests.
    pub fn step(&mut self) {
        let (w, h) = (self.w, self.h);
        if w == 0 || h == 0 {
            return;
        }
        // source row: strong heat with flicker and occasional gaps; a slow
        // fbm "fuel" field gives the flame front structure, and a stoke
        // floors the heat while it burns
        let k = self.flare_k;
        let t = self.shimmer_t;
        for x in 0..w {
            let fuel = 0.72 + 0.28 * fbm(x as f32 * 0.07, t * 0.45, 2, self.seed);
            let base = if self.rng.random::<f32>() < 0.96 + 0.035 * k {
                self.rng.random_range(200..=255)
            } else {
                self.rng.random_range(60..=140)
            };
            let mut v = base as f32 * fuel;
            if k > 0.0 {
                v = v.max((200.0 + 55.0 * k) * fuel);
            }
            self.heat[(h - 1) * w + x] = v.min(255.0) as u8;
        }
        // propagate upward with random cooling and horizontal wobble;
        // cooling grows quadratically with altitude so flames taper into sky
        let rows = (h - 1).max(1) as f32;
        for y in 0..h - 1 {
            let low = 1.0 - y as f32 / rows; // 1 at the base → 0 at the top
            let extra = ((1.0 - low) * (1.0 - low) * 6.0) as u8;
            for x in 0..w {
                let below = self.heat[(y + 1) * w + x];
                let decay = self.rng.random_range(0u8..=3).saturating_add(extra);
                let drift: i32 = self.rng.random_range(-1..=1);
                let dst_x = (x as i32 + drift).rem_euclid(w as i32) as usize;
                self.heat[y * w + dst_x] = below.saturating_sub(decay);
            }
        }
    }

    /// Mean heat of a row band (test helper).
    #[cfg(test)]
    fn band_mean(&self, y0: usize, y1: usize) -> f32 {
        let (w, h) = (self.w, self.h);
        let (y0, y1) = (y0.min(h - 1), y1.min(h));
        let mut sum = 0u64;
        let mut n = 0u64;
        for y in y0..y1 {
            for x in 0..w {
                sum += self.heat[y * w + x] as u64;
                n += 1;
            }
        }
        sum as f32 / n.max(1) as f32
    }
}

impl Scene for Fire {
    fn name(&self) -> &'static str {
        "fire"
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

        // stoke scheduling: anticipation (base swells in) → payoff (ember
        // burst) → decay (long eased tail the logs keep answering)
        self.next_flare -= dt;
        if self.flare_t < self.flare_dur {
            self.flare_t += dt;
        } else if self.next_flare <= 0.0 {
            self.flare_t = 0.0;
            self.flare_dur = self.rng.random_range(1.6..2.6);
            self.next_flare = self.rng.random_range(9.0..16.0);
            // payoff: kick a burst of embers off the hottest cells
            let burst = self
                .detail
                .scale((w as f32 / 6.0) * density_for(w, h), 4)
                .min(40);
            for _ in 0..burst {
                self.embers.push(Ember {
                    x: self.rng.random_range(0.0..w as f32),
                    y: h as f32 - 2.0,
                    vy: -self.rng.random_range(18.0..36.0),
                    life: self.rng.random_range(1.0..2.4),
                    age: 0.0,
                });
            }
        }
        self.flare_k = self.flare_envelope();
        let flare_k = self.flare_k;

        // run the sim at a fixed 60 Hz regardless of render fps
        let steps = ((dt * 60.0).round() as usize).clamp(1, 6);
        for _ in 0..steps {
            self.step();
        }
        self.shimmer_t += dt;
        let t = self.shimmer_t;

        // flame body; burnt-out cells fall through to pure black sky
        for y in 0..h {
            for x in 0..w {
                let heat = self.heat[y * w + x];
                if heat == 0 {
                    canvas.set(x as i32, y as i32, (0, 0, 0));
                } else {
                    let t = heat as f32 / 255.0;
                    canvas.set(x as i32, y as i32, palette_themed(t, self.theme.as_deref()));
                }
            }
        }

        // background plane: faint stars where the flames don't reach
        for s in &self.stars {
            let (ix, iy) = (s.x as usize, s.y as usize);
            if ix < w && iy < h && self.heat[iy * w + ix] < 10 {
                let tw = 0.7 + 0.3 * (t * 0.8 + s.phase).sin();
                canvas.set(s.x as i32, s.y as i32, scale((215, 208, 195), s.mag * tw));
            }
        }

        // rising embers born from the hottest cells (more during a stoke)
        let fy = h.saturating_sub(2);
        for _ in 0..self.detail.scale(3.0 + 6.0 * flare_k, 1) {
            let x = self.rng.random_range(0..w.max(1));
            if self.heat[fy * w + x] > 180 && self.rng.random::<f32>() < 0.4 {
                self.embers.push(Ember {
                    x: x as f32,
                    y: fy as f32,
                    vy: -self.rng.random_range(14.0..30.0),
                    life: self.rng.random_range(0.8..2.0),
                    age: 0.0,
                });
            }
        }
        let ember_color = palette_themed(0.85, self.theme.as_deref());
        self.embers.retain_mut(|e| {
            e.age += dt;
            e.life -= dt;
            e.vy *= 1.0 - 0.35 * dt; // drag: the rise eases off
            e.y += e.vy * dt;
            e.x += (e.y * 0.5).sin() * 6.0 * dt;
            if e.life <= 0.0 || e.y < 0.0 {
                return false;
            }
            // lifecycle envelope: quick fade-in, soft fade-out, flicker
            let env = ease_smooth((e.age * 6.0).min(1.0))
                * ease_smooth((e.life / 0.7).min(1.0));
            let flick = 0.7 + 0.3 * (e.age * 25.0).sin();
            canvas.set_f(e.x, e.y, scale(ember_color, (flick * env).min(1.0)));
            glow(canvas, e.x as i32, e.y as i32, 1, ember_color, 0.22 * env);
            true
        });

        // heat-shimmer: a wavy refraction band in the upper third
        for y in 0..(h / 3) as i32 {
            for x in 0..w as i32 {
                let heat_k = self.heat[y as usize * w + x as usize] as f32 / 255.0;
                if heat_k > 0.15 {
                    let wob = (x as f32 * 0.25 + y as f32 * 0.7 + t * 3.0).sin() * 1.5 * heat_k;
                    let src = canvas.get(x + wob as i32, y).color;
                    canvas.set(x, y, lerp(canvas.get(x, y).color, src, 0.6));
                }
            }
        }

        // smoke wisps drifting up from the flame tops
        if self.rng.random::<f32>() < dt * 6.0 {
            let life = self.rng.random_range(1.5..3.0);
            self.wisps.push(Wisp {
                x: self.rng.random_range(0.0..w as f32),
                y: h as f32 * 0.55,
                life,
                life0: life,
                age: 0.0,
            });
        }
        self.wisps.retain_mut(|ws| {
            ws.age += dt;
            ws.life -= dt;
            // rise eases off as the wisp cools and spreads
            ws.y -= dt * (5.0 + 4.0 * (1.0 - ws.age / ws.life0).max(0.0));
            ws.x += (ws.y * 0.3).sin() * 4.0 * dt;
            if ws.life <= 0.0 || ws.y < 0.0 {
                return false;
            }
            let a = ((ws.age / ws.life0).min(1.0) * std::f32::consts::PI).sin() * 0.5;
            canvas.add(ws.x as i32, ws.y as i32, scale((56, 52, 58), a * 0.3));
            true
        });

        // log silhouettes occlude the base of the flames; their rims answer
        // the heat above them and brighten while a stoke burns
        for &(lx, ly, lw, lh) in &self.logs {
            for dy in 0..lh {
                for dx in 0..lw {
                    canvas.set(lx + dx, ly + dy, (18, 9, 5));
                }
            }
            for dx in 0..lw {
                let above = if ly > 0 {
                    self.heat[(ly - 1) as usize * w + (lx + dx).clamp(0, w as i32 - 1) as usize]
                        as f32
                        / 255.0
                } else {
                    0.0
                };
                let flick = 0.85 + 0.15 * (t * 9.0 + dx as f32 * 1.7).sin();
                let rim = (0.25 + 0.75 * above) * (0.55 + 0.45 * flare_k) * flick;
                canvas.set(lx + dx, ly, scale((255, 120, 30), rim.min(1.0)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn heat_stays_in_bounds_and_decreases_upward() {
        let mut f = Fire::new(StdRng::seed_from_u64(7), None, Detail::Medium);
        f.init(32, 16);
        for _ in 0..120 {
            f.step();
        }
        // heat is u8 so bounds are type-guaranteed; assert the fire is
        // actually burning (nonzero) and cool at the top instead
        assert!(f.heat.iter().any(|&v| v > 0));
        let bottom = f.band_mean(14, 16);
        let top = f.band_mean(0, 2);
        assert!(
            top < bottom,
            "top mean {top} should be below bottom mean {bottom}"
        );
    }

    #[test]
    fn palette_endpoints() {
        assert_eq!(palette(0.0), (0, 0, 0));
        let hi = palette(1.0);
        assert!(hi.0 == 255 && hi.1 == 255 && hi.2 > 200);
        // mid is orange-red
        let mid = palette(0.6);
        assert!(mid.0 > 200 && mid.1 < 120);
    }
}
