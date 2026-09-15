//! Candy: saturated sugar-rush orbs on a neon gradient. Three parallax layers
//! read as real depth — far orbs are small, dim and desaturated by atmosphere,
//! near orbs are large, bright and carry a specular highlight from a single
//! fixed light. Hues are not drawn at random: each theme defines two tight
//! anchor families plus a rare complement accent, and orbs are distributed
//! inside those bands by a golden-angle walk so they spread evenly without
//! clumping. Every ~18-26s a sugar rush fires — a wavefront radiates from one
//! orb, and each orb flares in turn as the front sweeps over it.

use super::{Detail, Scene};
use crate::canvas::{disc, ease_out, ease_smooth, glow, hsv, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::TAU;

/// Golden-angle step (1 - 1/phi). Used to place orbs *within* a hue band with
/// low discrepancy — not across the wheel, which is what produced the old
/// rainbow-confetti look.
const GOLDEN: f32 = 0.381_966;

/// Fixed light direction for specular highlights, in units of orb radius.
/// One direction for every orb in the scene is what makes the gloss read as
/// lighting rather than as noise.
const LIGHT: (f32, f32) = (-0.36, -0.36);

const RUSH_IN: f32 = 0.9;
const RUSH_HOLD: f32 = 1.1;
const RUSH_OUT: f32 = 2.2;
const RUSH_TOTAL: f32 = RUSH_IN + RUSH_HOLD + RUSH_OUT;

/// Palette and tonal shape for one theme.
struct Palette {
    /// two analogous anchor hues the orbs cluster around
    hue_a: f32,
    hue_b: f32,
    /// how far an orb may wander from its anchor
    spread: f32,
    /// rare complement accent hue, and how often it is picked
    accent: f32,
    accent_p: f32,
    sat: f32,
    /// background gradient hues + saturation
    bg_a: f32,
    bg_b: f32,
    bg_sat: f32,
    /// hue is inert; depth must read through value and size alone
    mono: bool,
}

fn palette_for(theme: Option<&str>) -> Palette {
    match theme {
        // citrus: lime and lemon, with a magenta sour-candy pop
        Some("sour") => Palette {
            hue_a: 0.24,
            hue_b: 0.15,
            spread: 0.035,
            accent: 0.90,
            accent_p: 0.14,
            sat: 1.0,
            bg_a: 0.30,
            bg_b: 0.16,
            bg_sat: 0.85,
            mono: false,
        },
        // chalky, low-saturation pinks and sky blues
        Some("pastel") => Palette {
            hue_a: 0.90,
            hue_b: 0.56,
            spread: 0.06,
            accent: 0.11,
            accent_p: 0.12,
            sat: 0.42,
            bg_a: 0.74,
            bg_b: 0.55,
            bg_sat: 0.22,
            mono: false,
        },
        // no hue at all: depth is carried entirely by value and size
        Some("mono") => Palette {
            hue_a: 0.0,
            hue_b: 0.0,
            spread: 0.0,
            accent: 0.0,
            accent_p: 0.0,
            sat: 0.0,
            bg_a: 0.0,
            bg_b: 0.0,
            bg_sat: 0.0,
            mono: true,
        },
        // classic neon candy: hot pink and cyan, gold accent
        _ => Palette {
            hue_a: 0.91,
            hue_b: 0.51,
            spread: 0.05,
            accent: 0.13,
            accent_p: 0.13,
            sat: 0.95,
            bg_a: 0.78,
            bg_b: 0.53,
            bg_sat: 0.7,
            mono: false,
        },
    }
}

/// Per-layer depth grading. Index 0 is farthest, 2 is nearest the viewer.
struct Depth {
    radius: f32,
    value: f32,
    /// atmospheric desaturation: far orbs wash toward the background
    sat_k: f32,
    parallax: f32,
    glow_r: i32,
    /// 0 = matte (too far to catch a highlight), 1 = full gloss
    spec: f32,
}

const DEPTHS: [Depth; 3] = [
    Depth { radius: 0.60, value: 0.30, sat_k: 0.55, parallax: 0.45, glow_r: 1, spec: 0.0 },
    Depth { radius: 0.85, value: 0.50, sat_k: 0.80, parallax: 0.75, glow_r: 2, spec: 0.5 },
    Depth { radius: 1.20, value: 0.76, sat_k: 1.00, parallax: 1.10, glow_r: 3, spec: 1.0 },
];

/// Mono has no hue to separate the planes, so it leans much harder on value
/// and size contrast to keep the depth readable.
const MONO_VALUE: [f32; 3] = [0.16, 0.44, 0.95];
const MONO_RADIUS: [f32; 3] = [0.50, 0.85, 1.35];

struct Orb {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    hue: f32,
    layer: usize,
    phase: f32,
}

/// A sugar rush: a wavefront expanding from one orb, flaring each orb it
/// passes. `age` runs 0..RUSH_TOTAL.
struct Rush {
    x: f32,
    y: f32,
    age: f32,
}

pub struct Candy {
    rng: StdRng,
    detail: Detail,
    pal: Palette,
    orbs: Vec<Orb>,
    rush: Option<Rush>,
    next_rush: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Candy {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        Candy {
            rng,
            detail,
            pal: palette_for(theme),
            orbs: Vec::new(),
            rush: None,
            next_rush: 7.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// Hue for orb `i`: alternate between the two anchor families, placing each
    /// orb inside its family's band by a golden-angle walk, with an occasional
    /// complement accent for pop.
    fn hue_for(&self, i: usize) -> f32 {
        let p = &self.pal;
        let g = (i as f32 * GOLDEN).fract();
        if g < p.accent_p {
            return p.accent;
        }
        let anchor = if i.is_multiple_of(2) { p.hue_a } else { p.hue_b };
        (anchor + (g - 0.5) * 2.0 * p.spread).rem_euclid(1.0)
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        // Linear in area, and deliberately *not* multiplied by density_for():
        // that helper already carries a w*h term, so the old
        // `(w*h/900) * density_for(w, h)` was quadratic in area — which floored
        // at the minimum on small canvases (only 8 orbs at 100x30) while
        // saturating the cap on large ones.
        let n = self
            .detail
            .scale((w * h) as f32 / 200.0, 12)
            .clamp(12, 64);
        let mut orbs: Vec<Orb> = (0..n)
            .map(|i| {
                let layer = i % 3;
                let base = self.rng.random_range(1.6..3.4);
                Orb {
                    x: self.rng.random_range(0.0..w as f32),
                    y: self.rng.random_range(0.0..h as f32),
                    vx: self.rng.random_range(-6.0..6.0),
                    vy: self.rng.random_range(-4.0..4.0),
                    r: base,
                    hue: self.hue_for(i),
                    layer,
                    phase: self.rng.random_range(0.0..TAU),
                }
            })
            .collect();
        // painter's order, established once: `layer` never changes, so the old
        // per-frame sort was pure waste
        orbs.sort_by_key(|o| o.layer);
        self.orbs = orbs;
        self.rush = None;
    }

    /// Background: a two-hue diagonal gradient with enough tonal range that
    /// the orb planes have something to sit against.
    fn draw_background(&self, canvas: &mut Canvas) {
        let (w, h) = (self.w, self.h);
        let p = &self.pal;
        for y in 0..h {
            let ty = y as f32 / h.max(1) as f32;
            let wave = (self.t * 0.15 + ty * 2.0).sin() * 0.035;
            let (left, right) = if p.mono {
                // mono keeps a darker floor: value contrast is all it has to
                // separate the depth planes, so the planes need room above it
                let v = 0.05 + ty * 0.10;
                let g = (v * 255.0) as u8;
                let g2 = ((v + 0.05) * 255.0) as u8;
                ((g, g, g), (g2, g2, g2))
            } else {
                // lifted off near-black so the "neon gradient" in the scene's
                // description actually reads, while staying well under the
                // 0.30-0.76 orb value range
                (
                    hsv(
                        (p.bg_a + wave).rem_euclid(1.0),
                        p.bg_sat * 0.7,
                        0.11 + ty * 0.13,
                    ),
                    hsv(
                        (p.bg_b + wave * 0.5).rem_euclid(1.0),
                        p.bg_sat * 0.8,
                        0.14 + (1.0 - ty) * 0.14,
                    ),
                )
            };
            for x in 0..w {
                let tx = x as f32 / w.max(1) as f32;
                canvas.set(x as i32, y as i32, lerp(left, right, tx));
            }
        }
    }

    /// The expanding ring itself, drawn parametrically so cost tracks the
    /// front's circumference rather than the whole canvas.
    fn draw_rush_front(&self, canvas: &mut Canvas, rush: &Rush, front_r: f32, k: f32) {
        if front_r < 1.0 || k <= 0.01 {
            return;
        }
        let tint = if self.pal.mono {
            (255, 255, 255)
        } else {
            hsv(self.pal.accent, self.pal.sat * 0.35, 1.0)
        };
        let steps = ((front_r * 7.0) as usize).clamp(32, 1400);
        for s in 0..steps {
            let a = s as f32 / steps as f32 * TAU;
            // a touch of radial wobble so the ring never reads as a perfect
            // mechanical circle
            let wob = (a * 5.0 + self.t * 1.7).sin() * front_r * 0.018;
            let (ca, sa) = (a.cos(), a.sin());
            for band in 0..3 {
                let rr = front_r + wob + band as f32 * 0.8 - 0.8;
                let f = k * (1.0 - (band as f32 - 1.0).abs() * 0.45) * 0.5;
                canvas.add(
                    (rush.x + ca * rr) as i32,
                    (rush.y + sa * rr) as i32,
                    scale(tint, f),
                );
            }
        }
    }
}

impl Scene for Candy {
    fn name(&self) -> &'static str {
        "candy"
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

        // --- sugar rush: anticipation -> the front sweeps out -> decay ---
        self.next_rush -= dt;
        if self.next_rush <= 0.0 && self.rush.is_none() && !self.orbs.is_empty() {
            let i = self.rng.random_range(0..self.orbs.len());
            self.rush = Some(Rush {
                x: self.orbs[i].x,
                y: self.orbs[i].y,
                age: 0.0,
            });
            self.next_rush = self.rng.random_range(18.0..26.0);
        }
        let mut rush_k = 0.0;
        let mut front_r = 0.0;
        let mut rush_pos = None;
        if let Some(r) = &mut self.rush {
            r.age += dt;
            if r.age >= RUSH_TOTAL {
                self.rush = None;
            } else {
                let a = r.age;
                rush_k = if a < RUSH_IN {
                    ease_smooth(a / RUSH_IN)
                } else if a < RUSH_IN + RUSH_HOLD {
                    1.0
                } else {
                    1.0 - ease_smooth((a - RUSH_IN - RUSH_HOLD) / RUSH_OUT)
                };
                // the front only starts travelling after the anticipation beat
                let travel = ((a - RUSH_IN * 0.55) / (RUSH_TOTAL - RUSH_IN * 0.55)).clamp(0.0, 1.0);
                let reach = ((w * w + h * h) as f32).sqrt() * 0.62;
                front_r = ease_out(travel) * reach;
                rush_pos = Some((r.x, r.y));
            }
        }

        self.draw_background(canvas);

        if let (Some(_), Some(r)) = (rush_pos, self.rush.as_ref()) {
            self.draw_rush_front(canvas, r, front_r, rush_k);
        }

        // --- orbs, far plane first (order fixed at init) ---
        let p_sat = self.pal.sat;
        let mono = self.pal.mono;
        for o in &mut self.orbs {
            let d = &DEPTHS[o.layer];
            o.x += o.vx * dt * d.parallax;
            o.y += o.vy * dt * d.parallax;
            let (wf, hf) = (w as f32, h as f32);
            let pad = o.r * d.radius + 3.0;
            if o.x < -pad {
                o.x = wf + pad;
            } else if o.x > wf + pad {
                o.x = -pad;
            }
            if o.y < -pad {
                o.y = hf + pad;
            } else if o.y > hf + pad {
                o.y = -pad;
            }

            // the front flares each orb as it sweeps past: a narrow gaussian
            // band on distance-from-source, so orbs light up in sequence
            let mut hit = 0.0;
            if let Some((sx, sy)) = rush_pos {
                let dist = ((o.x - sx).powi(2) + (o.y - sy).powi(2)).sqrt();
                let band = (dist - front_r) / (5.0 + front_r * 0.06);
                hit = rush_k * (-band * band).exp();
            }

            let pulse = 0.82 + 0.18 * (self.t * 1.6 + o.phase).sin();
            let radius = o.r * if mono { MONO_RADIUS[o.layer] } else { d.radius }
                * (1.0 + hit * 0.30);
            let value = (if mono { MONO_VALUE[o.layer] } else { d.value })
                * pulse
                * (1.0 + hit * 0.85);
            let col = if mono {
                let g = (value.clamp(0.0, 1.0) * 255.0) as u8;
                (g, g, g)
            } else {
                hsv(o.hue, p_sat * d.sat_k, value.clamp(0.0, 1.0))
            };

            glow(
                canvas,
                o.x as i32,
                o.y as i32,
                d.glow_r + (hit * 2.0) as i32,
                col,
                0.30 + hit * 0.45,
            );
            disc(canvas, o.x, o.y, radius, col);

            // specular: one fixed light for the whole scene, near planes only
            let spec = d.spec * (0.55 + 0.45 * pulse);
            if spec > 0.01 && radius > 1.2 {
                let hr = (radius * 0.30).max(0.65);
                disc(
                    canvas,
                    o.x + LIGHT.0 * radius,
                    o.y + LIGHT.1 * radius,
                    hr,
                    lerp(col, (255, 255, 255), 0.72 * spec + hit * 0.2),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    const THEMES: [Option<&str>; 4] = [None, Some("sour"), Some("pastel"), Some("mono")];

    #[test]
    fn candy_runs_all_themes() {
        for theme in THEMES {
            let mut c = Candy::new(StdRng::seed_from_u64(1), theme, Detail::Medium);
            let mut cv = Canvas::new(80, 40);
            for _ in 0..90 {
                c.update(1.0 / 30.0, &mut cv);
            }
        }
    }

    /// The doctrine point: hues must cluster into the theme's families, not
    /// spread over the whole wheel the way a uniform random draw does.
    #[test]
    fn hues_are_harmonic_not_uniform() {
        for theme in [None, Some("sour"), Some("pastel")] {
            let mut c = Candy::new(StdRng::seed_from_u64(7), theme, Detail::Medium);
            c.init(120, 40);
            let pal = palette_for(theme);
            for o in &c.orbs {
                let near = |anchor: f32| {
                    let d = (o.hue - anchor).abs();
                    d.min(1.0 - d) <= pal.spread + 1e-3
                };
                assert!(
                    near(pal.hue_a) || near(pal.hue_b) || (o.hue - pal.accent).abs() < 1e-6,
                    "theme {theme:?}: hue {} belongs to no family",
                    o.hue
                );
            }
            // and both families are actually used
            let a = c.orbs.iter().filter(|o| o.layer < 3).count();
            assert!(a >= 10, "expected a populated field, got {a}");
        }
    }

    /// Depth must be readable: near orbs strictly brighter and bigger than far.
    #[test]
    fn depth_planes_are_graded() {
        for i in 1..3 {
            assert!(DEPTHS[i].value > DEPTHS[i - 1].value);
            assert!(DEPTHS[i].radius > DEPTHS[i - 1].radius);
            assert!(DEPTHS[i].sat_k >= DEPTHS[i - 1].sat_k);
            assert!(MONO_VALUE[i] > MONO_VALUE[i - 1]);
            assert!(MONO_RADIUS[i] > MONO_RADIUS[i - 1]);
        }
    }

    /// Specular highlights must actually produce bright pixels — the old scene
    /// never exceeded luminance 180 anywhere.
    #[test]
    fn specular_highlights_reach_full_brightness() {
        let mut c = Candy::new(StdRng::seed_from_u64(3), None, Detail::Medium);
        let mut cv = Canvas::new(120, 40);
        for _ in 0..120 {
            c.update(1.0 / 30.0, &mut cv);
        }
        let bright = (0..120)
            .flat_map(|x| (0..40).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let (r, g, b) = cv.get(x, y).color;
                (r as u32 * 2 + g as u32 * 3 + b as u32) / 6 > 180
            })
            .count();
        assert!(bright > 0, "no highlight pixels above luminance 180");
    }

    /// The rush is the scene's event: it must actually change the frame.
    #[test]
    fn sugar_rush_changes_the_frame() {
        let sum = |c: &Canvas| -> u64 {
            (0..120)
                .flat_map(|x| (0..40).map(move |y| (x, y)))
                .map(|(x, y)| {
                    let (r, g, b) = c.get(x, y).color;
                    r as u64 + g as u64 + b as u64
                })
                .sum()
        };
        let mut c = Candy::new(StdRng::seed_from_u64(5), None, Detail::Medium);
        let mut cv = Canvas::new(120, 40);
        // settle, with no rush pending
        c.next_rush = 999.0;
        for _ in 0..60 {
            c.update(1.0 / 30.0, &mut cv);
        }
        let calm = sum(&cv);
        // fire one and sample at the hold beat
        c.next_rush = 0.0;
        for _ in 0..((RUSH_IN + RUSH_HOLD * 0.5) * 30.0) as usize {
            c.update(1.0 / 30.0, &mut cv);
        }
        let during = sum(&cv);
        assert!(
            during > calm,
            "rush should add light: calm {calm} vs during {during}"
        );
    }
}
