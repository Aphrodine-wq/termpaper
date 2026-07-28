//! Tide: luminous contour ridges morphing like a slow signal. Stacked
//! horizontal ridge lines (Unknown Pleasures energy) driven by fbm noise
//! that slowly morphs over time. Depth runs top->bottom: far ridges are
//! dim and flat, near ridges bright and tall. Ridges are drawn back to
//! front, each clipped against the running silhouette of the ones above
//! it, so the stack occludes cleanly with additive-only rendering.
//! Every ~18-28s a swell rolls through the stack — ridges lean ahead of
//! the front (anticipation), crest with a bright lift (payoff), then
//! settle back into the drift (decay). Sparse glints ride the near-ridge
//! crests; foam flecks and a crest shimmer band lift the near plane
//! during swell payoff.

use super::noise::fbm;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

/// Swell event: a big slow wave traveling the stack top-to-bottom or
/// laterally. `front` sweeps 0..1 across ridge-fraction or x-fraction.
struct Swell {
    active: bool,
    lateral: bool,
    t: f32,
    dur: f32,
    next: f32,
}

impl Swell {
    /// (strength envelope 0..1, front position 0..1).
    fn state(&self) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }
        let attack = ease_smooth(self.t / 1.4);
        let release = 1.0 - ease_smooth((self.t - (self.dur - 1.8)) / 1.8);
        let front = ease_smooth(((self.t - 0.7) / (self.dur - 1.4)).clamp(0.0, 1.0));
        (attack * release, front)
    }

    /// Vertical displacement and brightness lift at stack-position `s`.
    /// The crest is a gaussian bump; a smaller negative lobe just ahead
    /// of the front makes ridges lean in anticipation.
    fn displacement(&self, s: f32, amp: f32) -> (f32, f32) {
        let (e, front) = self.state();
        if e <= 0.0 {
            return (0.0, 0.0);
        }
        let crest = (-((s - front) * 5.0).powi(2)).exp();
        let lean = (-((s - (front + 0.16)) * 8.0).powi(2)).exp();
        (amp * e * (crest * 1.5 - lean * 0.4), e * crest)
    }
}

/// A glint: a brief sparkle riding one ridge's crest line. Eases in,
/// drifts slowly sideways, eases out — no popping.
struct Glint {
    ridge: usize,
    x: f32,
    vx: f32,
    t: f32,
    dur: f32,
}

impl Glint {
    /// Lifecycle envelope 0..1: smooth attack, smooth release.
    fn brightness(&self) -> f32 {
        ease_smooth(self.t / 0.5) * (1.0 - ease_smooth((self.t - (self.dur - 0.9)) / 0.9))
    }
}

/// Near-plane foam fleck / spray mote spawned on swell crest.
struct Foam {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    size: f32,
}

impl Foam {
    fn brightness(&self) -> f32 {
        let k = self.life / self.max_life;
        ease_smooth(1.0 - k) * ease_smooth(k * 2.0).min(1.0)
    }
}

pub struct Tide {
    rng: StdRng,
    detail: Detail,
    base_c: (u8, u8, u8),
    crest_c: (u8, u8, u8),
    seed: u32,
    rows: usize,
    sil: Vec<f32>, // scratch: per-column occlusion silhouette
    glints: Vec<Glint>,
    foam: Vec<Foam>,
    next_glint: f32,
    next_foam: f32,
    swell: Swell,
    t: f32,
    w: usize,
    h: usize,
}

impl Tide {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (base_c, crest_c) = match theme {
            Some("ice") => ((132, 188, 255), (238, 248, 255)),
            Some("ember") => ((232, 118, 42), (255, 222, 172)),
            _ => ((96, 214, 235), (224, 250, 255)), // pulse: cool cyan-white
        };
        Tide {
            seed: rng.random::<u32>(),
            rng,
            detail,
            base_c,
            crest_c,
            rows: 0,
            sil: Vec::new(),
            glints: Vec::new(),
            foam: Vec::new(),
            next_glint: 2.0,
            next_foam: 0.3,
            swell: Swell {
                active: false,
                lateral: false,
                t: 0.0,
                dur: 6.0,
                next: 9.0,
            },
            t: 0.0,
            w: 0,
            h: 0,
        }
    }
}

impl Scene for Tide {
    fn name(&self) -> &'static str {
        "tide"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // cap dt so fast-forward can't blow up the swell clock
        let dt = dt.clamp(0.0, 0.1);

        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            // 20-40 rows at typical sizes; fewer in tiny panes, capped
            let base = (h as f32 * 1.1).clamp(12.0, 36.0);
            self.rows = self
                .detail
                .scale(base * density_for(w, h).min(1.5), 8)
                .clamp(2, 40);
            self.sil = vec![0.0; w];
        }
        self.t += dt;
        let t = self.t;

        // --- swell scheduling: anticipation -> crest -> settle ---
        self.swell.next -= dt;
        if self.swell.next <= 0.0 && !self.swell.active {
            self.swell.active = true;
            self.swell.lateral = self.rng.random::<bool>();
            self.swell.t = 0.0;
            self.swell.dur = self.rng.random_range(5.0..7.0);
            self.swell.next = self.rng.random_range(18.0..28.0);
        }
        if self.swell.active {
            self.swell.t += dt;
            if self.swell.t >= self.swell.dur {
                self.swell.active = false;
            }
        }

        // --- glints: sparse sparkles riding the near-ridge crests ---
        self.next_glint -= dt;
        let glint_cap = self
            .detail
            .scale(6.0 * density_for(w, h).min(1.5), 2)
            .clamp(2, 14);
        if self.next_glint <= 0.0 && self.glints.len() < glint_cap && self.rows > 4 {
            self.next_glint = self.rng.random_range(0.5..1.4);
            self.glints.push(Glint {
                ridge: self.rng.random_range(self.rows / 2..self.rows),
                x: self.rng.random_range(w as f32 * 0.05..w as f32 * 0.95),
                vx: self.rng.random_range(-1.5..1.5),
                t: 0.0,
                dur: self.rng.random_range(1.6..3.4),
            });
        }
        for g in self.glints.iter_mut() {
            g.t += dt;
            g.x += g.vx * dt;
        }
        self.glints.retain(|g| g.t < g.dur && g.ridge < self.rows);

        canvas.clear((0, 0, 0));
        self.sil.fill(0.0);

        // far haze: a whisper of depth behind the topmost ridges
        let haze_rows = (h as f32 * 0.18).max(1.0) as usize;
        for y in 0..haze_rows {
            let f = 1.0 - y as f32 / haze_rows as f32;
            canvas.fill_row(y, scale(self.base_c, 0.05 * f * f));
        }

        let rows = self.rows;
        let top = h as f32 * 0.08;
        let span = h as f32 * 0.86;
        let spacing = span / (rows - 1) as f32;
        let (swell_e, swell_front) = self.swell.state();

        // crest shimmer band: additive lift across near ridges at swell peak
        let shimmer = if swell_e > 0.45 {
            ((swell_e - 0.45) / 0.55).clamp(0.0, 1.0) * swell_e
        } else {
            0.0
        };

        // track near-ridge crest y for foam spawn
        let mut near_crest_samples: Vec<(f32, f32)> = Vec::new();

        for r in 0..rows {
            let df = r as f32 / (rows - 1) as f32; // 0 far .. 1 near
            // near ridges taller and brighter; far ridges flat and dim
            let amp = spacing * (1.1 + 2.8 * df * df);
            let depth_bright = 0.3 + 0.7 * df;
            let base_y = top + r as f32 * spacing;
            // per-ridge noise lane, drifting slowly in time and sideways
            let lane = r as f32 * 0.42 + t * 0.07;
            let drift = t * 0.03;
            let swell_amp = spacing * (3.0 + 5.0 * df);
            let seed = self.seed.wrapping_add(r as u32 * 131);

            for x in 0..w {
                let xf = x as f32 / w as f32;
                let f = fbm(xf * 3.0 + drift, lane, 3, seed);
                // swell displacement: vertical sweeps the ridge stack,
                // lateral sweeps across the width
                let (dy, boost) = if swell_e > 0.0 {
                    let s = if self.swell.lateral { xf } else { df };
                    self.swell.displacement(s, swell_amp)
                } else {
                    (0.0, 0.0)
                };
                // bumps push downward; brightness follows ridge height
                let y = base_y + (f - 0.35) * amp + dy;
                if y <= self.sil[x] {
                    continue; // occluded by a ridge closer to the front
                }
                self.sil[x] = y;

                let hn = ((f - 0.2) / 0.55).clamp(0.0, 1.0);
                let b = (depth_bright * (0.25 + 0.75 * hn) * (1.0 + boost * 1.1))
                    .min(1.25);
                let mut c = lerp(self.base_c, self.crest_c, (hn * 0.4 + boost * 0.4).min(1.0));
                // hot tips: near-ridge peaks burn toward white so accents pop
                let tip = ease_smooth((hn - 0.72) / 0.28) * df * df;
                c = lerp(c, (255, 255, 255), tip * 0.8);
                // crest shimmer band lighting bleed on near ridges at swell peak
                if shimmer > 0.0 && df > 0.55 {
                    let band = (-((df - swell_front).abs() * 6.0).powi(2)).exp();
                    c = lerp(c, (255, 255, 255), shimmer * band * 0.35);
                }
                let c = scale(c, (b + tip * 0.35 + shimmer * df * 0.15).min(1.7));

                // 1px luminous line with vertical sub-pixel AA
                let y0 = y.floor() as i32;
                let frac = y - y0 as f32;
                canvas.add(x as i32, y0, scale(c, 1.0 - frac));
                canvas.add(x as i32, y0 + 1, scale(c, frac));
                // crest glow: the traveling wave lights the band it crosses,
                // bleeding above and below the lifted line
                if boost > 0.35 && df > 0.35 {
                    let gb = (boost - 0.35) * 0.8;
                    canvas.add(x as i32, y0 - 1, scale(c, 0.22 * gb));
                    canvas.add(x as i32, y0 + 2, scale(c, 0.35 * gb));
                    canvas.add(x as i32, y0 + 3, scale(c, 0.18 * gb));
                }
                // sample near crests for foam (every 8th column)
                if df > 0.7 && boost > 0.4 && x % 8 == 0 {
                    near_crest_samples.push((x as f32, y));
                }
                // glints sparkle on the visible crest line
                for g in &self.glints {
                    if g.ridge == r && (g.x - x as f32).abs() < 0.5 {
                        let gb = g.brightness();
                        if gb > 0.01 {
                            glow(canvas, x as i32, y0, 1, self.crest_c, gb * 0.9);
                            canvas.add(x as i32, y0, scale((255, 255, 255), gb * 0.7));
                        }
                    }
                }
            }
        }

        // --- foam flecks on near ridges during swell crest ---
        self.next_foam -= dt;
        let foam_cap = self
            .detail
            .scale(14.0 * density_for(w, h).min(1.5), 4)
            .clamp(4, 36);
        if swell_e > 0.4
            && self.next_foam <= 0.0
            && self.foam.len() < foam_cap
            && !near_crest_samples.is_empty()
        {
            self.next_foam = self.rng.random_range(0.04..0.14);
            let n = self.detail.scale(2.0 + swell_e * 3.0, 1);
            for _ in 0..n {
                let &(cx, cy) = &near_crest_samples
                    [self.rng.random_range(0..near_crest_samples.len())];
                let life = self.rng.random_range(0.5..1.4);
                self.foam.push(Foam {
                    x: cx + self.rng.random_range(-3.0..3.0),
                    y: cy + self.rng.random_range(-1.5..1.0),
                    vx: self.rng.random_range(-12.0..12.0),
                    vy: self.rng.random_range(-8.0..-1.0),
                    life,
                    max_life: life,
                    size: self.rng.random_range(0.6..1.5),
                });
            }
        } else if swell_e <= 0.2 {
            self.next_foam = 0.2;
        }

        for f in self.foam.iter_mut() {
            f.life -= dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.vy += 6.0 * dt; // soft settle
            f.vx *= 1.0 - dt * 0.8;
        }
        self.foam.retain(|f| f.life > 0.0);

        for f in &self.foam {
            let b = f.brightness();
            if b < 0.02 {
                continue;
            }
            let col = lerp(self.crest_c, (255, 255, 255), 0.55);
            glow(
                canvas,
                f.x as i32,
                f.y as i32,
                if f.size > 1.1 { 2 } else { 1 },
                col,
                b * 0.45 * f.size,
            );
            canvas.add(f.x as i32, f.y as i32, scale(col, b * 0.7));
        }
    }
}
