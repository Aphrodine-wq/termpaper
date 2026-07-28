//! Orbits: planets tracing luminous orbital trails around a star. Kepler-ish
//! eccentric orbits (Newton-solved anomaly, faster near periapsis), tilted /
//! squashed 2D projections, comet-like decaying trail ribbons per planet, a
//! pulsing star with soft glow, close-approach glow swells that flare the
//! star, sparse fixed background stars, and a TRANSIT event every ~20-30s:
//! planets ease toward a syzygy line, hold a harmonic pulse, then release.
//! The system fades in over the first ~1.5s, and the far side of every orbit
//! is dimmed a plane back so the star reads as a true occluder.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const TAU: f32 = std::f32::consts::TAU;
const EASE_TIME: f32 = 4.0;
const HOLD_TIME: f32 = 2.6;
const RELEASE_TIME: f32 = 5.0;

/// Transit rhythm: Idle -> Ease (toward the line) -> Hold (pulse) -> Release.
#[derive(Clone, Copy, PartialEq)]
enum Transit {
    Idle,
    Ease(f32), // elapsed
    Hold(f32),
    Release(f32),
}

struct Planet {
    a: f32,      // semi-major axis, fraction of the scene radius
    e: f32,      // eccentricity
    squash: f32, // cos(inclination): y-flattening of the projected ellipse
    tilt: f32,   // rotation of the orbit plane on screen
    n: f32,      // mean motion (rad/s), Kepler: inner orbits run faster
    m: f32,      // mean anomaly
    color: (u8, u8, u8),
    size: i32, // glow radius when lit
    trail: Vec<(f32, f32)>,
    trail_cap: usize,
    jitter: f32, // per-planet offset from the syzygy line during transit
}

struct BgStar {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
}

/// Solve Kepler's equation (E - e*sinE = M) and return the orbital-plane
/// position in units of the semi-major axis.
fn kepler_pos(e: f32, m: f32) -> (f32, f32) {
    let mut ea = m;
    for _ in 0..4 {
        ea -= (ea - e * ea.sin() - m) / (1.0 - e * ea.cos());
    }
    (ea.cos() - e, (1.0 - e * e).sqrt() * ea.sin())
}

pub struct Orbits {
    rng: StdRng,
    detail: Detail,
    star_core: (u8, u8, u8),
    star_glow: (u8, u8, u8),
    planet_palette: Vec<(u8, u8, u8)>,
    bg_tints: Vec<(u8, u8, u8)>,
    binary: bool,
    planets: Vec<Planet>,
    bg_stars: Vec<BgStar>,
    swell: Vec<f32>, // per-planet close-approach glow, reused each frame
    transit: Transit,
    next_transit: f32,
    line_ang: f32, // syzygy line angle for the active transit
    flare: f32,    // star flare envelope (close passes + transit pulse)
    t: f32,
    w: usize,
    h: usize,
}

impl Orbits {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (star_core, star_glow, planet_palette, bg_tints, binary) = match theme {
            Some("binary") => (
                (215, 230, 255),
                (90, 130, 220),
                vec![
                    (140, 180, 240),
                    (110, 210, 220),
                    (170, 150, 230),
                    (120, 160, 200),
                    (190, 220, 250),
                    (100, 190, 180),
                ],
                vec![(200, 215, 240), (170, 190, 230)],
                true,
            ),
            Some("ice") => (
                (235, 246, 255),
                (120, 170, 235),
                vec![
                    (160, 210, 250),
                    (120, 235, 230),
                    (200, 230, 255),
                    (150, 170, 240),
                    (230, 240, 255),
                    (110, 200, 250),
                ],
                vec![(220, 235, 255), (180, 205, 245)],
                false,
            ),
            _ => (
                // solar: warm star, varied planet accents
                (255, 232, 180),
                (235, 150, 60),
                vec![
                    (255, 150, 80),  // ember
                    (120, 175, 255), // blue
                    (140, 230, 160), // jade
                    (250, 210, 110), // gold
                    (215, 130, 230), // violet
                    (110, 225, 225), // cyan
                    (235, 110, 100), // red
                    (235, 235, 240), // pale
                ],
                vec![(255, 250, 235), (205, 215, 245), (255, 225, 190)],
                false,
            ),
        };
        Orbits {
            rng,
            detail,
            star_core,
            star_glow,
            planet_palette,
            bg_tints,
            binary,
            planets: Vec::new(),
            bg_stars: Vec::new(),
            swell: Vec::new(),
            transit: Transit::Idle,
            next_transit: 13.0, // first payoff lands early; then 20-30s apart
            line_ang: 0.0,
            flare: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// Scene center: the star sits slightly off-center.
    fn center(&self) -> (f32, f32) {
        (self.w as f32 * 0.53, self.h as f32 * 0.46)
    }

    /// Base orbit radius in pixels; y is halved for the ~2:1 cell aspect.
    fn scene_r(&self) -> f32 {
        (self.w as f32 * 0.44).min(self.h as f32 * 0.9)
    }

    /// Projected screen position of an orbit point (pre-transit-blend), plus
    /// a depth sign (>0 = passes in front of the star). Static so it can be
    /// called while planet state is mutably borrowed.
    fn project(
        cx: f32,
        cy: f32,
        r: f32,
        a: f32,
        e: f32,
        squash: f32,
        tilt: f32,
        m: f32,
    ) -> (f32, f32, f32) {
        let (ox, oy) = kepler_pos(e, m);
        let (ox, oy) = (ox * a, oy * a);
        let (ct, st) = (tilt.cos(), tilt.sin());
        let rx = ox * ct - oy * squash * st;
        let ry = ox * st + oy * squash * ct;
        let depth = ox * st + oy * ct; // unsquashed: which side of the star
        (cx + rx * r, cy + ry * r * 0.5, depth)
    }

    fn planet_pos(&self, p: &Planet) -> (f32, f32, f32) {
        let (cx, cy) = self.center();
        Self::project(cx, cy, self.scene_r(), p.a, p.e, p.squash, p.tilt, p.m)
    }

    /// Eased alignment factor 0..1 for the current transit phase.
    fn align_k(&self) -> f32 {
        match self.transit {
            Transit::Idle => 0.0,
            Transit::Ease(e) => ease_smooth(e / EASE_TIME),
            Transit::Hold(_) => 1.0,
            Transit::Release(e) => 1.0 - ease_smooth(e / RELEASE_TIME),
        }
    }
}

impl Scene for Orbits {
    fn name(&self) -> &'static str {
        "orbits"
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
            // 4-8 planets; trail length scales with detail and orbit size
            // (larger canvases need longer ribbons to cover the same arc)
            let n = self.rng.random_range(4..=8);
            let r = self.scene_r();
            let trail_cap = (self.detail.scale(70.0, 28).min(150) as f32
                * (r / 40.0).clamp(0.6, 2.0)) as usize;
            let palette = self.planet_palette.clone();
            self.planets = (0..n)
                .map(|i| {
                    let a = 0.16 + 0.78 * (i as f32 + self.rng.random_range(0.25..0.9))
                        / n as f32;
                    // keep apoapsis on screen: a*(1+e) <= ~1.05 of the radius
                    let e: f32 = self.rng.random_range(0.05..0.42);
                    let e = e.min((1.05 / a - 1.0).max(0.02));
                    Planet {
                        a,
                        e,
                        squash: self.rng.random_range(0.35..0.85),
                        tilt: self.rng.random_range(0.0..TAU),
                        // Kepler's third law: inner orbits swing faster
                        n: self.rng.random_range(0.35..0.6) * a.powf(-1.5),
                        m: self.rng.random_range(0.0..TAU),
                        color: palette[i % palette.len()],
                        size: if self.rng.random::<f32>() < 0.3 { 2 } else { 1 },
                        trail: Vec::with_capacity(trail_cap),
                        trail_cap,
                        jitter: 0.0,
                    }
                })
                .collect();
            let nb = self
                .detail
                .scale((w * h / 220) as f32 * density_for(w, h), 18)
                .min(400);
            self.bg_stars = (0..nb)
                .map(|_| BgStar {
                    x: self.rng.random_range(0.0..w as f32),
                    y: self.rng.random_range(0.0..h as f32),
                    mag: self.rng.random_range(0.10..0.32),
                    phase: self.rng.random_range(0.0..TAU),
                })
                .collect();
            self.swell.clear();
            self.swell.resize(self.planets.len(), 0.0);
            self.transit = Transit::Idle;
        }
        self.t += dt;
        // intro fade: the system grows in instead of popping into view
        let intro = ease_smooth((self.t / 1.6).min(1.0));

        // --- transit scheduling (anticipation -> payoff -> decay) ---
        self.next_transit -= dt;
        match self.transit {
            Transit::Idle => {
                if self.next_transit <= 0.0 {
                    self.line_ang = self.rng.random_range(0.0..TAU);
                    for p in &mut self.planets {
                        p.jitter = self.rng.random_range(-0.05..0.05);
                    }
                    self.transit = Transit::Ease(0.0);
                }
            }
            Transit::Ease(e) => {
                let e = e + dt;
                self.transit = if e >= EASE_TIME {
                    Transit::Hold(0.0)
                } else {
                    Transit::Ease(e)
                };
            }
            Transit::Hold(e) => {
                let e = e + dt;
                self.transit = if e >= HOLD_TIME {
                    Transit::Release(0.0)
                } else {
                    Transit::Hold(e)
                };
            }
            Transit::Release(e) => {
                let e = e + dt;
                if e >= RELEASE_TIME {
                    self.transit = Transit::Idle;
                    self.next_transit = self.rng.random_range(20.0..30.0);
                } else {
                    self.transit = Transit::Release(e);
                }
            }
        }
        let k = self.align_k();
        // harmonic pulse while the syzygy holds
        let pulse = if matches!(self.transit, Transit::Hold(_)) {
            0.5 + 0.5 * (self.t * 7.0).sin()
        } else {
            0.0
        };
        self.flare = (self.flare - dt * 1.2).max(pulse * 0.9).max(0.0);

        // --- draw ---
        canvas.clear((0, 0, 0));

        // background plane: sparse fixed stars, gently breathing
        for (i, s) in self.bg_stars.iter().enumerate() {
            let tw = 0.75 + 0.25 * (self.t * 0.7 + s.phase).sin();
            let tint = self.bg_tints[i % self.bg_tints.len()];
            canvas.set_f(s.x, s.y, scale(tint, s.mag * tw * intro));
        }

        // star(s): binary = two small stars circling a common barycenter
        let (cx, cy) = self.center();
        let r = self.scene_r();
        let star_positions: [(f32, f32); 2] = if self.binary {
            let ang = self.t * 0.45;
            let sep = r * 0.14;
            [
                (cx + ang.cos() * sep, cy + ang.sin() * sep * 0.5),
                (cx - ang.cos() * sep, cy - ang.sin() * sep * 0.5),
            ]
        } else {
            [(cx, cy), (cx, cy)]
        };
        let star_pts: &[(f32, f32)] = if self.binary {
            &star_positions
        } else {
            &star_positions[..1]
        };

        // faint orbit guides: the ellipse each planet rides, near-black;
        // flares from close passes / transit wash a little light into them
        let guide_lvl = (0.07 + self.flare * 0.06) * intro;
        for p in &self.planets {
            let guide = scale(p.color, guide_lvl);
            for step in 0..48 {
                let m = step as f32 / 48.0 * TAU;
                let (gx, gy, _) = Self::project(cx, cy, r, p.a, p.e, p.squash, p.tilt, m);
                canvas.set_f(gx, gy, guide);
            }
        }

        // integrate planets, then split by depth around the star
        for s in self.swell.iter_mut() {
            *s = 0.0;
        }
        for i in 0..self.planets.len() {
            let p = &mut self.planets[i];
            p.m = (p.m + p.n * dt * (1.0 - k * 0.75)).rem_euclid(TAU);
            let (sx, sy, _) = Self::project(cx, cy, r, p.a, p.e, p.squash, p.tilt, p.m);
            // close-approach swell: nearest passes bloom and flare the star
            let d = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
            if d < 8.0 {
                self.swell[i] = (1.0 - d / 8.0).powi(2);
            }
        }
        let max_swell = self.swell.iter().cloned().fold(0.0f32, f32::max);
        self.flare = self.flare.max(max_swell * 0.45);

        // dd: depth dim — the far side of the system sits a plane back
        let draw_planet = |canvas: &mut Canvas, p: &Planet, sw: f32, k: f32, pulse: f32, dd: f32| {
            let (sx, sy, _) = self.planet_pos(p);
            // ease toward the syzygy line (blend positions — no teleporting)
            let (sx, sy) = if k > 0.001 {
                let (cx, cy) = self.center();
                let r = self.scene_r();
                let ru = ((sx - cx) / r).hypot((sy - cy) / (r * 0.5));
                let ang = self.line_ang + p.jitter;
                let tx = cx + ang.cos() * ru * r;
                let ty = cy + ang.sin() * ru * r * 0.5;
                (sx + (tx - sx) * k, sy + (ty - sy) * k)
            } else {
                (sx, sy)
            };
            let lit = dd * intro;
            // trail ribbon: decaying brightness with age — the star of the show
            let boost = (1.0 + sw * 1.2 + pulse * 0.8 + k * 0.3) * lit;
            let len = p.trail.len();
            for (i, &(tx, ty)) in p.trail.iter().enumerate() {
                let age = (i + 1) as f32 / len as f32; // 1 = newest
                let b = age * age * 0.5 * boost;
                canvas.add(tx as i32, ty as i32, scale(p.color, b));
            }
            let bright = (0.75 + sw * 0.6 + pulse * 0.5).min(1.6) * lit;
            let core = lerp(p.color, (255, 255, 255), 0.35 + pulse * 0.2);
            let (ix, iy) = (sx as i32, sy as i32);
            canvas.set(ix, iy, scale(core, bright.min(1.0)));
            let gr = p.size + if sw > 0.25 || pulse > 0.4 { 1 } else { 0 };
            glow(canvas, ix, iy, gr, p.color, (0.30 * bright).min(0.7));
        };

        // behind the star first, dimmed a plane back
        for (i, p) in self.planets.iter().enumerate() {
            if self.planet_pos(p).2 <= 0.0 {
                draw_planet(canvas, p, self.swell[i], k, pulse, 0.78);
            }
        }

        // the star: soft glow, gentle pulse, flares on close passes / transit
        let breathe = 0.82 + 0.18 * (self.t * 0.9).sin();
        for &(sx, sy) in star_pts {
            let (ix, iy) = (sx as i32, sy as i32);
            let inten = (0.5 * breathe + self.flare * 0.6).min(1.2) * intro;
            let rad = (3.0 + self.flare * 3.0 + pulse * 2.0) as i32;
            glow(canvas, ix, iy, rad.clamp(2, 8), self.star_glow, inten);
            let core = lerp(self.star_core, (255, 255, 255), (self.flare * 0.5).min(0.5));
            canvas.set(ix, iy, scale(core, intro));
            // second-row bleed so the core fills the full cell pixel
            canvas.add(ix, iy + 1, scale(core, 0.55 * intro));
            if self.binary {
                canvas.add(ix + 1, iy, scale(core, 0.5 * intro));
            }
        }

        // in front of the star, full brightness
        for (i, p) in self.planets.iter().enumerate() {
            if self.planet_pos(p).2 > 0.0 {
                draw_planet(canvas, p, self.swell[i], k, pulse, 1.0);
            }
        }

        // push trail samples after drawing (newest = current position)
        for p in self.planets.iter_mut() {
            let (sx, sy, _) = Self::project(cx, cy, r, p.a, p.e, p.squash, p.tilt, p.m);
            if p.trail.len() >= p.trail_cap {
                p.trail.remove(0);
            }
            p.trail.push((sx, sy));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn checksum(s: &mut Orbits, w: usize, h: usize, steps: usize) -> u64 {
        let mut c = Canvas::new(w, h);
        for _ in 0..steps {
            s.update(1.0 / 60.0, &mut c);
        }
        let mut sum = 0u64;
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (r, g, b) = c.get(x, y).color;
                sum = sum
                    .wrapping_mul(31)
                    .wrapping_add(((r as u64) << 16) ^ ((g as u64) << 8) ^ b as u64);
            }
        }
        sum
    }

    #[test]
    fn deterministic_from_seed() {
        let run = || {
            let mut s = Orbits::new(StdRng::seed_from_u64(123), None, Detail::Medium);
            checksum(&mut s, 80, 40, 300)
        };
        assert_eq!(run(), run());
        // different seed diverges
        let mut a = Orbits::new(StdRng::seed_from_u64(1), None, Detail::Medium);
        let mut b = Orbits::new(StdRng::seed_from_u64(2), None, Detail::Medium);
        assert_ne!(
            checksum(&mut a, 80, 40, 120),
            checksum(&mut b, 80, 40, 120)
        );
    }

    #[test]
    fn resize_reinits_without_panic() {
        let mut s = Orbits::new(StdRng::seed_from_u64(7), Some("binary"), Detail::High);
        let mut c = Canvas::new(120, 60);
        for _ in 0..30 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(40, 12);
        for _ in 0..30 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(0, 0);
        s.update(1.0 / 30.0, &mut c); // degenerate canvas: early return
        // large dt steps must not explode
        c.resize(60, 20);
        for _ in 0..10 {
            s.update(5.0, &mut c);
        }
    }

    #[test]
    fn themes_resolve_and_background_stays_dark() {
        for theme in [None, Some("solar"), Some("binary"), Some("ice"), Some("bogus")] {
            let mut s = Orbits::new(StdRng::seed_from_u64(3), theme, Detail::Medium);
            assert_eq!(s.name(), "orbits");
            let mut c = Canvas::new(60, 24);
            s.update(1.0 / 60.0, &mut c);
            let mut dark = 0usize;
            for y in 0..24 {
                for x in 0..60 {
                    let (r, g, b) = c.get(x, y).color;
                    if r as u32 + g as u32 + b as u32 <= 120 {
                        dark += 1;
                    }
                }
            }
            assert!(
                dark as f32 / (60 * 24) as f32 > 0.85,
                "theme {theme:?}: scene should stay mostly dark"
            );
        }
    }

    #[test]
    fn transit_event_eases_holds_and_releases() {
        let mut s = Orbits::new(StdRng::seed_from_u64(11), None, Detail::Medium);
        let mut c = Canvas::new(100, 50);
        let mut saw_ease = false;
        let mut saw_hold = false;
        let mut max_align = 0.0f32;
        // 60 simulated seconds: first transit fires at ~13s + 4s ease
        for _ in 0..1800 {
            s.update(1.0 / 30.0, &mut c);
            saw_ease |= matches!(s.transit, Transit::Ease(_));
            saw_hold |= matches!(s.transit, Transit::Hold(_));
            max_align = max_align.max(s.align_k());
        }
        assert!(saw_ease, "transit should ease in");
        assert!(saw_hold, "transit should hold the syzygy");
        assert!(max_align > 0.99, "alignment should fully engage");
        // and the scene returns to idle for the next cycle
        assert!(
            matches!(s.transit, Transit::Idle) || s.next_transit > 0.0,
            "transit releases back to free orbits"
        );
    }
}
