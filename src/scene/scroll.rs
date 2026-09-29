//! Scroll: an endless hanging ink-wash landscape, composed for a tall
//! (portrait) canvas. The camera pans slowly *up* a painted mountain range
//! the way you would unroll a kakejiku: ridge after ridge rises from the
//! bottom edge and climbs toward the top, near cliffs painted in dense
//! ink with pines on the ridgeline, distant ranges as pale washes half
//! eaten by drifting mist. Every ridgeline fades downward into paper —
//! the classic sumi-e "mountain dissolving into cloud" — so the next
//! range emerges out of the fade of the one above it.
//!
//! The painting is a static, procedurally endless composition; only the
//! viewing window moves. Its rhythm alternates bands of near, heavily
//! inked ground with bands of misty distance so the pan never turns into
//! a flat stripe pattern. Living details: mist drifting across the ranges,
//! waterfalls threading pale streaks down the taller cliffs, flocks of
//! birds crossing in a loose line, and (on the dark themes) a moon. A
//! vermilion artist's seal sits in the lower corner, fixed to the frame.
//!
//! Sized for the terminal it runs in — it works landscape too, but the
//! composition reads best tall (a 9:16 monitor at a small font size).

use super::noise::{fbm, hash2, vnoise};
use super::{Detail, Scene};
use crate::canvas::{lerp, Canvas, Cell};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::TAU;

/// Base pan speed up the scroll, canvas px per second.
const PAN_SPEED: f32 = 1.3;
/// Longest wash below any ridgeline; also the off-screen cull margin.
const MAX_FADE: f32 = 80.0;
/// World-space period of the near/far composition rhythm (px).
const BAND_PERIOD: f32 = 540.0;
/// Hard cap on flocks in the air at once.
const MAX_FLOCKS: usize = 3;

#[derive(Clone, Copy)]
struct Theme {
    paper: (u8, u8, u8),
    ink: (u8, u8, u8),
    edge: (u8, u8, u8),
    seal: (u8, u8, u8),
    moon: Option<(u8, u8, u8)>,
}

/// One painted mountain range. Everything visual derives from `seed` and
/// the canvas width, so profiles rebuild deterministically on resize.
struct Ridge {
    base: f32, // world y of the baseline (larger = farther up the scroll)
    seed: u32,
    amp: f32,  // peak height above the baseline
    ink: f32,  // wash density 0..1 at the ridgeline
    fade: f32, // how far the wash runs below the ridgeline
    mist: f32, // how much drifting mist eats this range (0..1)
    exp: f32,  // wash falloff exponent (low = solid ground, high = quick dissolve)
    profile: Vec<f32>,
    env: Vec<f32>,          // horizontal extent: 1 inside the painted stretch, 0 dissolved
    trees: Vec<(i32, i32)>, // (column, height)
    fall: Option<(i32, f32)>, // waterfall: (column, length)
}

impl Ridge {
    fn rebuild(&mut self, w: usize, detail: Detail) {
        let wf = w.max(1) as f32;
        // near ranges: a few broad tall peaks; far ranges: finer serration
        let scale = 1.3 + hash2(1, 1, self.seed) * (1.4 + 3.2 * (1.0 - self.ink));
        let off = hash2(2, 2, self.seed) * 100.0;
        self.profile = (0..w)
            .map(|x| {
                let u = x as f32 / wf * scale + off;
                // ridged fbm for peaks, plain fbm for the rolling body
                let ridged = 1.0 - (2.0 * fbm(u, 0.37, 4, self.seed) - 1.0).abs();
                let body = fbm(u * 0.6 + 3.1, 7.7, 3, self.seed ^ 0x51ed);
                self.amp * (0.7 * ridged.powf(2.0) + 0.3 * body)
            })
            .collect();

        // ~40% of ranges only occupy part of the width and dissolve at the ends
        let partial = hash2(3, 3, self.seed) < 0.4;
        let center = 0.15 + hash2(4, 4, self.seed) * 0.7;
        let half = 0.22 + hash2(5, 5, self.seed) * 0.35;
        self.env = (0..w)
            .map(|x| {
                if !partial {
                    1.0
                } else {
                    let d = (x as f32 / wf - center).abs();
                    1.0 - smoothstep(half - 0.16, half + 0.06, d)
                }
            })
            .collect();

        self.trees.clear();
        if self.ink > 0.58 {
            let n = detail.scale(wf / 13.0 * self.ink, 3);
            for i in 0..n {
                let x = (hash2(i as i32, 0, self.seed) * wf) as i32;
                if self.env.get(x as usize).copied().unwrap_or(0.0) < 0.6 {
                    continue;
                }
                let h = 3 + (hash2(i as i32, 1, self.seed) * (2.0 + self.ink * 3.0)) as i32;
                self.trees.push((x, h));
            }
        }

        self.fall = if self.ink > 0.5 && hash2(9, 9, self.seed) < 0.38 && w > 24 {
            let col = (wf * (0.15 + hash2(9, 10, self.seed) * 0.7)) as i32;
            // a waterfall needs painted rock on both sides
            let col = if self.env.get(col as usize).copied().unwrap_or(0.0) < 0.9 {
                (wf * center) as i32
            } else {
                col
            };
            Some((col, self.fade * 0.9))
        } else {
            None
        };
    }

    fn top(&self, x: usize, base_screen: f32) -> f32 {
        base_screen - self.profile.get(x).copied().unwrap_or(0.0)
    }
}

/// A line of birds crossing the sky.
struct Flock {
    x: f32,
    y: f32,
    dir: f32,
    speed: f32,
    n: usize,
    gap: f32,
    phase: f32,
}

pub struct Scroll {
    rng: StdRng,
    detail: Detail,
    theme: Theme,
    seed: u32,
    band_phase: f32,
    t: f32,
    cam: f32,
    w: usize,
    ridges: Vec<Ridge>, // ascending `base`
    next_base: f32,     // world y where the next range up the scroll goes
    flocks: Vec<Flock>,
    next_flock: f32,
    mist: Vec<f32>,
}

impl Scroll {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let theme = match theme {
            Some("night") => Theme {
                paper: (7, 7, 9),
                ink: (168, 174, 184),
                edge: (224, 228, 234),
                seal: (150, 34, 30),
                moon: Some((236, 232, 214)),
            },
            Some("indigo") => Theme {
                paper: (12, 18, 38),
                ink: (146, 168, 208),
                edge: (216, 228, 246),
                seal: (196, 58, 50),
                moon: Some((240, 236, 220)),
            },
            _ => Theme {
                // sumi: ink on warm, lamp-lit paper
                paper: (214, 202, 176),
                ink: (40, 36, 34),
                edge: (22, 20, 20),
                seal: (176, 42, 34),
                moon: None,
            },
        };
        let seed = rng.random::<u32>();
        let band_phase = rng.random_range(0.0..TAU);
        Scroll {
            rng,
            detail,
            theme,
            seed,
            band_phase,
            t: 0.0,
            cam: 0.0,
            w: 0,
            ridges: Vec::new(),
            next_base: 0.0,
            flocks: Vec::new(),
            next_flock: 6.0,
            mist: Vec::new(),
        }
    }

    /// Composition rhythm at a world height: 1 = near, dense ground; 0 = far
    /// misty distance. A slow sine plus a touch of noise so bands vary.
    fn band(&self, y: f32) -> f32 {
        let s = 0.5 + 0.5 * (y / BAND_PERIOD * TAU + self.band_phase).sin();
        (s * 0.8 + 0.2 * vnoise(y * 0.01, 0.5, self.seed)).clamp(0.0, 1.0)
    }

    fn spawn_ridge(&mut self, base: f32, w: usize) -> f32 {
        let b = self.band(base);
        let seed = self.rng.random::<u32>();
        let jit = |r: &mut StdRng, lo: f32, hi: f32| r.random_range(lo..hi);
        let ink = (0.16 + 0.78 * b.powf(1.3)) * jit(&mut self.rng, 0.85, 1.12);
        let mut ridge = Ridge {
            base,
            seed,
            amp: (6.0 + 46.0 * b) * jit(&mut self.rng, 0.6, 1.4),
            ink: ink.clamp(0.1, 1.0),
            fade: ((18.0 + 50.0 * b) * jit(&mut self.rng, 0.8, 1.2)).min(MAX_FADE),
            mist: 0.85 - 0.6 * b,
            exp: 1.0 + 1.3 * (1.0 - b),
            profile: Vec::new(),
            env: Vec::new(),
            trees: Vec::new(),
            fall: None,
        };
        ridge.rebuild(w, self.detail);
        self.ridges.push(ridge);
        // gap to the next range up the scroll
        (14.0 + 50.0 * b) * jit(&mut self.rng, 0.7, 1.4)
    }

    fn maintain(&mut self, w: usize, h: usize) {
        let hf = h as f32;
        if self.ridges.is_empty() {
            self.next_base = self.cam - MAX_FADE;
        }
        // cull ranges whose highest peak has scrolled off the bottom
        let cam = self.cam;
        self.ridges.retain(|r| r.base + r.amp >= cam - 4.0);
        // extend above the top
        while self.next_base < self.cam + hf + 40.0 {
            let b = self.next_base;
            let gap = self.spawn_ridge(b, w);
            self.next_base = b + gap;
        }
        if self.w != w {
            for r in &mut self.ridges {
                r.rebuild(w, self.detail);
            }
            self.w = w;
        }
    }

    fn spawn_flock(&mut self, w: usize, h: usize) {
        if self.flocks.len() >= MAX_FLOCKS {
            return;
        }
        let dir = if self.rng.random::<f32>() < 0.5 { 1.0 } else { -1.0 };
        let n = self.detail.scale(self.rng.random_range(4.0..9.0), 3);
        let gap = self.rng.random_range(2.5..4.5);
        let span = n as f32 * gap;
        self.flocks.push(Flock {
            x: if dir > 0.0 { -span } else { w as f32 + span },
            y: self.rng.random_range(h as f32 * 0.08..h as f32 * 0.6),
            dir,
            speed: self.rng.random_range(5.0..9.0),
            n,
            gap,
            phase: self.rng.random_range(0.0..TAU),
        });
    }
}

#[inline]
fn blend(cells: &mut [Cell], w: usize, h: usize, x: i32, y: i32, color: (u8, u8, u8), a: f32) {
    if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
        return;
    }
    let c = &mut cells[y as usize * w + x as usize];
    c.color = lerp(c.color, color, a.clamp(0.0, 1.0));
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Scene for Scroll {
    fn name(&self) -> &'static str {
        "scroll"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.min(0.1);
        self.t += dt;
        let t = self.t;
        // breathe the pan a little so it never feels mechanical
        self.cam += PAN_SPEED * dt * (0.8 + 0.2 * (t * 0.07).sin());
        let cam = self.cam;
        let (wf, hf) = (w as f32, h as f32);

        self.maintain(w, h);

        // ---- paper with a faint tooth that scrolls with the painting ----
        let th = self.theme;
        canvas.clear(th.paper);
        let seed = self.seed;
        {
            let cells = canvas.cells_raw_mut();
            for y in 0..h {
                let wy = (hf - y as f32 + cam) as i32;
                for x in 0..w {
                    let g = 0.955 + 0.045 * hash2(x as i32, wy, seed ^ 0x9a9);
                    let c = &mut cells[y * w + x];
                    c.color = (
                        (c.color.0 as f32 * g) as u8,
                        (c.color.1 as f32 * g) as u8,
                        (c.color.2 as f32 * g) as u8,
                    );
                }
            }
        }

        // ---- drifting mist field (world-anchored vertically) ----
        self.mist.resize(w * h, 0.0);
        for y in 0..h {
            let wy = hf - y as f32 + cam;
            for x in 0..w {
                let m = fbm(
                    x as f32 * 0.032 + t * 0.028,
                    wy * 0.055 + t * 0.006,
                    3,
                    seed ^ 0x3a7,
                );
                self.mist[y * w + x] = smoothstep(0.34, 0.74, m);
            }
        }

        // ---- moon (dark themes), fixed to the frame ----
        if let Some(moon) = th.moon {
            let (mx, my, r) = (wf * 0.72, 13.0_f32, (wf * 0.045).clamp(3.0, 7.0));
            let cells = canvas.cells_raw_mut();
            let halo = r * 2.6;
            let (x0, x1) = ((mx - halo) as i32, (mx + halo) as i32 + 1);
            let (y0, y1) = ((my - halo) as i32, (my + halo) as i32 + 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let d = ((x as f32 - mx).powi(2) + (y as f32 - my).powi(2)).sqrt();
                    let a = if d <= r {
                        0.92 - 0.1 * hash2(x, y, seed)
                    } else {
                        0.18 * (1.0 - (d - r) / (halo - r)).clamp(0.0, 1.0).powi(2)
                    };
                    blend(cells, w, h, x, y, moon, a);
                }
            }
        }

        // ---- ranges, far (top) to near (bottom) ----
        let cells = canvas.cells_raw_mut();
        let (ink, edge, paper) = (th.ink, th.edge, th.paper);
        for r in self.ridges.iter().rev() {
            let base_s = hf - (r.base - cam);
            if base_s - r.amp > hf + 1.0 || base_s + r.fade < -1.0 {
                continue;
            }
            let inv_fade = 1.0 / r.fade;
            let hatch_w = 0.28 * r.ink;
            for x in 0..w {
                let env = r.env[x];
                if env <= 0.01 {
                    continue;
                }
                let top = r.top(x, base_s);
                let y0 = top.ceil().max(0.0) as i32;
                let y1 = ((top + r.fade) as i32).min(h as i32 - 1);
                let wy0 = (hf - top + cam) as i32;
                for y in y0..=y1 {
                    let dy = y as f32 - top;
                    let mm = 1.0 - r.mist * self.mist[y as usize * w + x];
                    let grain = 0.8 + 0.2 * hash2(x as i32, wy0 - y, r.seed);
                    if dy < 1.0 {
                        // crisp ridgeline stroke
                        let a = (r.ink * 0.9 + 0.1) * mm * (0.7 + 0.3 * grain) * env;
                        blend(cells, w, h, x as i32, y, edge, a);
                    } else {
                        // brush hatching: slanted strokes following the slope
                        let hatch = 1.0 - hatch_w
                            + hatch_w * vnoise(x as f32 * 0.42 + dy * 0.18, dy * 0.09, r.seed ^ 0x77);
                        let a = r.ink * (1.0 - dy * inv_fade).powf(r.exp) * mm * grain * hatch * env;
                        blend(cells, w, h, x as i32, y, ink, a);
                    }
                }
            }

            // waterfall: pale streaks threading down the wash, ink strokes at the sides
            if let Some((col, len)) = r.fall {
                if (col as usize) < w {
                    let top = r.top(col as usize, base_s);
                    let mm = 1.0 - r.mist * 0.5;
                    for k in -3i32..=3 {
                        let x = col + k;
                        for dy in 0..len as i32 {
                            let y = top as i32 + dy;
                            let f = 1.0 - dy as f32 / len;
                            if k.abs() == 3 {
                                let a = 0.22 * r.ink * f * mm;
                                blend(cells, w, h, x, y, edge, a);
                            } else {
                                let streak = vnoise((dy as f32 - t * 42.0) * 0.45, k as f32 * 3.7, r.seed);
                                let lat = match k.abs() {
                                    0 => 1.0,
                                    1 => 0.75,
                                    _ => 0.35,
                                };
                                let a = (0.3 + 0.55 * streak) * f.sqrt() * lat * mm;
                                blend(cells, w, h, x, y, paper, a);
                            }
                        }
                    }
                    // spray at the foot
                    let foot = top + len;
                    for i in 0..7 {
                        let p = hash2(i, (t * 6.0) as i32, r.seed);
                        let q = hash2(i, (t * 6.0) as i32 + 1, r.seed);
                        let x = col + ((p - 0.5) * 9.0) as i32;
                        let y = (foot + q * 3.0) as i32;
                        blend(cells, w, h, x, y, paper, 0.45 * mm);
                    }
                }
            }

            // pines on the ridgeline
            for &(tx, th_) in &r.trees {
                if (tx as usize) >= w {
                    continue;
                }
                let top = r.top(tx as usize, base_s) as i32;
                if top < -8 || top > h as i32 + 1 {
                    continue;
                }
                let mm = (1.0 - r.mist * self.mist[(top.clamp(0, h as i32 - 1) as usize) * w + tx as usize])
                    * r.env[tx as usize];
                for i in 0..th_ {
                    let y = top - i;
                    blend(cells, w, h, tx, y, edge, 0.85 * r.ink * mm);
                    if i % 2 == 1 && i < th_ - 1 {
                        let span = ((th_ - i) / 2).clamp(1, 3);
                        for k in 1..=span {
                            let a = 0.6 * r.ink * mm * (1.0 - k as f32 / (span as f32 + 1.0));
                            blend(cells, w, h, tx - k, y, edge, a);
                            blend(cells, w, h, tx + k, y, edge, a);
                        }
                    }
                }
            }
        }

        // ---- birds ----
        self.next_flock -= dt;
        if self.next_flock <= 0.0 {
            self.spawn_flock(w, h);
            self.next_flock = self.rng.random_range(16.0..40.0);
        }
        for f in &mut self.flocks {
            f.x += f.dir * f.speed * dt;
            f.y += (t * 0.5 + f.phase).sin() * 0.4 * dt;
        }
        {
            let cells = canvas.cells_raw_mut();
            for f in &self.flocks {
                for i in 0..f.n {
                    let bx = f.x - f.dir * i as f32 * f.gap;
                    let by = f.y + ((i as f32 * 1.7 + f.phase).sin()) * 1.5;
                    let up = (t * 7.0 + f.phase + i as f32 * 0.9).sin() > 0.0;
                    let (x, y) = (bx.round() as i32, by.round() as i32);
                    let a = 0.75;
                    if up {
                        blend(cells, w, h, x - 1, y - 1, ink, a);
                        blend(cells, w, h, x, y, ink, a);
                        blend(cells, w, h, x + 1, y - 1, ink, a);
                    } else {
                        blend(cells, w, h, x - 1, y, ink, a);
                        blend(cells, w, h, x, y, ink, a * 0.6);
                        blend(cells, w, h, x + 1, y, ink, a);
                    }
                }
            }
        }
        self.flocks
            .retain(|f| f.x > -(f.n as f32 * f.gap + 4.0) && f.x < wf + f.n as f32 * f.gap + 4.0);

        // ---- artist's seal, fixed to the frame ----
        if w > 20 && h > 30 {
            let cells = canvas.cells_raw_mut();
            let (sx, sy, s) = (w as i32 - 10, h as i32 - 13, 7);
            for j in 0..s {
                for i in 0..s {
                    let border = i == 0 || j == 0 || i == s - 1 || j == s - 1;
                    let cut = hash2(i, j, seed ^ 0x5ea1) > 0.6;
                    let a = if border {
                        0.88
                    } else if cut {
                        0.22
                    } else {
                        0.8
                    };
                    blend(cells, w, h, sx + i, sy + j, th.seal, a);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn run(theme: Option<&str>, w: usize, h: usize, frames: usize) -> Canvas {
        let mut s = Scroll::new(StdRng::seed_from_u64(7), theme, Detail::Medium);
        let mut c = Canvas::new(w, h);
        for _ in 0..frames {
            s.update(1.0 / 30.0, &mut c);
        }
        c
    }

    #[test]
    fn paints_a_populated_portrait_canvas() {
        let c = run(None, 120, 240, 90);
        let paper = (214u8, 202u8, 176u8);
        let inked = (0..120)
            .flat_map(|x| (0..240).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let (r, g, b) = c.get(x, y).color;
                (r as i32 - paper.0 as i32).abs() > 30
                    || (g as i32 - paper.1 as i32).abs() > 30
                    || (b as i32 - paper.2 as i32).abs() > 30
            })
            .count();
        assert!(inked > 2000, "expected ink on the paper, got {inked} px");
    }

    #[test]
    fn keeps_ridges_bounded_over_a_long_pan() {
        let mut s = Scroll::new(StdRng::seed_from_u64(3), Some("night"), Detail::High);
        let mut c = Canvas::new(100, 200);
        for _ in 0..30 * 60 * 4 {
            s.update(1.0 / 30.0, &mut c);
        }
        assert!(s.ridges.len() < 64, "ridge list grew to {}", s.ridges.len());
        assert!(s.flocks.len() <= MAX_FLOCKS);
        // still sorted ascending so painter's order holds
        assert!(s.ridges.windows(2).all(|p| p[0].base <= p[1].base));
    }

    #[test]
    fn survives_resize_and_tiny_canvases() {
        let mut s = Scroll::new(StdRng::seed_from_u64(1), Some("indigo"), Detail::Low);
        let mut c = Canvas::new(60, 90);
        for _ in 0..10 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(140, 260);
        for _ in 0..10 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(8, 6);
        for _ in 0..10 {
            s.update(1.0 / 30.0, &mut c);
        }
    }
}
