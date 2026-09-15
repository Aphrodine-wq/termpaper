//! Meteor shower: starry sky with a faint milky-way band, meteors streaking
//! diagonally with bright heads and fading ion trails, periodic shower
//! surges, and rare bolides that flash the whole sky.

use super::{Detail, Scene};
use crate::canvas::{density_for, dot, ease_smooth, glow_f, lerp, line_f, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

struct Star {
    x: f32,
    y: f32,
    bright: f32,
    phase: f32,
    tint: (u8, u8, u8),
}

struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    age: f32,
    trail: usize,
    bright: f32,
    bolide: bool,
    color: (u8, u8, u8),
}

struct Ember {
    x: f32,
    y: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    phase: f32,
}

type Constellation = (Vec<(f32, f32)>, Vec<(usize, usize)>);
type ShapeDef = (&'static [(f32, f32)], &'static [(usize, usize)]);

pub struct Meteors {
    rng: StdRng,
    detail: Detail,
    sky: ((u8, u8, u8), (u8, u8, u8)),
    stars: Vec<Star>,
    /// constellation stick figures: (points, edges) in canvas coords
    constellations: Vec<Constellation>,
    satellite: Option<(f32, f32, f32)>,
    t: f32,
    meteors: Vec<Meteor>,
    embers: Vec<Ember>,
    next_spawn: f32,
    surge_in: f32,
    surge_left: f32,
    bolide_in: f32,
    flash: f32,
    w: usize,
    h: usize,
}

impl Meteors {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let sky = match theme {
            // deep night gradients: near-black up top, a whisper of color
            // hugging the horizon (all well under 25% luminance)
            Some("warm") => ((4, 2, 8), (18, 8, 10)),
            Some("cold") => ((1, 5, 10), (6, 14, 20)),
            _ => ((2, 4, 12), (10, 8, 20)), // night
        };
        Meteors {
            rng,
            detail,
            sky,
            stars: Vec::new(),
            constellations: Vec::new(),
            satellite: None,
            t: 0.0,
            meteors: Vec::new(),
            embers: Vec::new(),
            next_spawn: 0.5,
            surge_in: 9.0, // first shower peak lands early
            surge_left: 0.0,
            bolide_in: 12.0,
            flash: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.stars.clear();
        // background stars
        for _ in 0..self.detail.scale((w * h / 160) as f32 * 2.0 * density_for(w, h), 20).min(900) {
            self.stars.push(Star {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                bright: self.rng.random_range(0.15..0.7),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                tint: (210, 215, 235),
            });
        }
        // milky-way band: dense dim stars along a diagonal with spread
        let band_n = ((w * h / 60) as f32 * 2.0 * density_for(w, h)) as usize;
        for _ in 0..band_n {
            let t = self.rng.random::<f32>();
            // diagonal band from upper-left to lower-right
            let bx = t * w as f32;
            let by = t * h as f32 * 0.8 + h as f32 * 0.05;
            let spread: f32 = self.rng.random_range(-1.0..1.0);
            let spread = spread * spread * spread.signum(); // gaussian-ish
            self.stars.push(Star {
                x: bx + spread * w as f32 * 0.10,
                y: by + spread * h as f32 * 0.28,
                bright: self.rng.random_range(0.08..0.45),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                tint: if self.rng.random::<f32>() < 0.3 {
                    (190, 200, 240) // blue-ish band stars
                } else {
                    (225, 220, 205)
                },
            });
        }
        self.init_constellations();
    }

    fn init_constellations(&mut self) {
        // a few classic stick-figure shapes as (points, edges)
        let shapes: &[ShapeDef] = &[
            // big dipper-ish
            (
                &[(0.0, 0.0), (1.0, 0.3), (2.0, 0.5), (3.0, 0.4), (3.4, 1.2), (4.4, 1.5), (4.0, 0.6), (3.0, 0.4)],
                &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7)],
            ),
            // cassiopeia-ish W
            (
                &[(0.0, 0.5), (0.8, 1.2), (1.6, 0.4), (2.4, 1.1), (3.2, 0.3)],
                &[(0, 1), (1, 2), (2, 3), (3, 4)],
            ),
            // lyra-ish triangle + tail
            (
                &[(0.0, 0.0), (0.8, 0.9), (-0.6, 1.0), (0.0, 0.0), (1.4, 0.2)],
                &[(0, 1), (1, 2), (2, 3), (0, 4)],
            ),
        ];
        let (w, h) = (self.w as f32, self.h as f32);
        self.constellations = shapes
            .iter()
            .map(|(pts, edges)| {
                let ox = self.rng.random_range(w * 0.05..w * 0.75);
                let oy = self.rng.random_range(h * 0.05..h * 0.45);
                let sc = self.rng.random_range(w * 0.04..w * 0.07);
                let aspect = 0.55;
                (
                    pts.iter()
                        .map(|&(px, py)| (ox + px * sc, oy + py * sc * aspect))
                        .collect(),
                    edges.to_vec(),
                )
            })
            .collect();
    }

    fn spawn_meteor(&mut self, bolide: bool) {
        let (w, h) = (self.w as f32, self.h as f32);
        // scale speeds and trails so small/large panes read the same
        let sf = (w.min(h) / 50.0).clamp(0.6, 1.8);
        let from_left = self.rng.random::<bool>();
        let speed = if bolide {
            self.rng.random_range(70.0..100.0)
        } else {
            self.rng.random_range(50.0..110.0)
        } * sf;
        // diagonal, mostly downward
        let angle: f32 = self.rng.random_range(0.5..1.0);
        // ion glow tint: mostly green-white, some sodium-orange, some blue
        let roll = self.rng.random::<f32>();
        let color = if roll < 0.6 {
            (130, 255, 175)
        } else if roll < 0.85 {
            (255, 190, 110)
        } else {
            (150, 190, 255)
        };
        let trail = if bolide {
            22
        } else {
            self.rng.random_range(8..15)
        };
        self.meteors.push(Meteor {
            x: self.rng.random_range(w * 0.1..w * 0.9),
            y: self.rng.random_range(-5.0..h * 0.25),
            vx: angle.cos() * speed * if from_left { 1.0 } else { -1.0 },
            vy: angle.sin().abs() * speed * 0.7,
            life: self.rng.random_range(0.6..1.6),
            age: 0.0,
            trail: ((trail as f32 * sf) as usize).max(4),
            bright: if bolide { 1.6 } else { self.rng.random_range(0.7..1.0) },
            bolide,
            color: if bolide { (255, 200, 130) } else { color },
        });
    }
}

impl Scene for Meteors {
    fn name(&self) -> &'static str {
        "meteors"
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
        // sky flash from bolides
        self.flash = (self.flash - dt * 2.5).max(0.0);
        for y in 0..h {
            let base = lerp(self.sky.0, self.sky.1, y as f32 / h.max(1) as f32);
            canvas.fill_row(y, lerp(base, (90, 95, 120), self.flash * 0.5));
        }

        // campfire glow bottom-left, flickering warm
        let fire_x = w as f32 * 0.12;
        let flick = 0.6 + 0.4 * (self.t * 7.0).sin() * (self.t * 3.1).cos();
        for dy in 0..6 {
            let r = (6 - dy) as f32 * w as f32 * 0.02;
            for dx in -(r as i32)..=(r as i32) {
                canvas.add(
                    fire_x as i32 + dx,
                    h as i32 - 1 - dy,
                    scale((255, 140, 50), (1.0 - dy as f32 / 6.0) * 0.35 * flick),
                );
            }
        }

        // embers rising off the fire: foreground accents with a sine wiggle
        if self.embers.len() < 4 && self.rng.random::<f32>() < dt * 1.5 {
            let max_life = self.rng.random_range(1.2..2.4);
            self.embers.push(Ember {
                x: fire_x + self.rng.random_range(-2.0..2.0),
                y: h as f32 - 2.0,
                vy: self.rng.random_range(2.5..5.0),
                life: max_life,
                max_life,
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            });
        }
        self.embers.retain_mut(|e| {
            e.life -= dt;
            e.y -= e.vy * dt;
            if e.life <= 0.0 || e.y < 0.0 {
                return false;
            }
            e.x += (self.t * 3.0 + e.phase).sin() * 1.2 * dt;
            let age = 1.0 - e.life / e.max_life;
            let env = ease_smooth(age / 0.2) * ease_smooth(e.life / e.max_life / 0.7);
            canvas.set_f(e.x, e.y, scale((255, 150, 60), 0.65 * env));
            true
        });

        // faint city glow hugging the horizon
        for x in 0..w {
            let d = x as f32 / w as f32;
            let haze = ((d * 9.0).sin() * 0.5 + 0.5) * 0.5 + 0.5;
            for dy in 0..3 {
                canvas.add(
                    x as i32,
                    h as i32 - 1 - dy,
                    scale((90, 60, 30), haze * 0.16 * (3 - dy) as f32 / 3.0),
                );
            }
        }

        // stars: gentle twinkle, lifted while a bolide flash washes the sky
        for s in &self.stars {
            let tw = 0.7 + 0.3 * (self.t * 1.4 + s.phase).sin();
            canvas.set_f(
                s.x,
                s.y,
                scale(s.tint, (s.bright * tw + self.flash * 0.2).min(1.0)),
            );
        }

        // constellation stick figures: slightly brighter stars + faint lines
        for (pts, edges) in &self.constellations {
            for &(px, py) in pts {
                canvas.set_f(px, py, (200, 205, 230));
            }
            for &(a, b) in edges {
                let (x0, y0) = pts[a];
                let (x1, y1) = pts[b];
                let steps = ((x1 - x0).abs() + (y1 - y0).abs()) as i32 + 1;
                for i in 0..=steps {
                    let f = i as f32 / steps as f32;
                    canvas.set_f(x0 + (x1 - x0) * f, y0 + (y1 - y0) * f, (26, 30, 46));
                }
            }
        }

        // a satellite crossing steadily
        if self.satellite.is_none() && self.rng.random::<f32>() < dt * 0.06 {
            let from_left = self.rng.random::<bool>();
            self.satellite = Some((
                if from_left { -2.0 } else { w as f32 + 2.0 },
                self.rng.random_range(h as f32 * 0.05..h as f32 * 0.4),
                if from_left { 12.0 } else { -12.0 },
            ));
        }
        if let Some((sx, sy, svx)) = &mut self.satellite {
            *sx += *svx * dt;
            if *sx < -4.0 || *sx > w as f32 + 4.0 {
                self.satellite = None;
            } else {
                // steady cold blink
                let b = 0.7 + 0.3 * (self.t * 2.0).sin();
                canvas.set_f(*sx, *sy, scale((230, 235, 255), b));
            }
        }

        // shower surge: a peak of activity every so often (anticipation is
        // the quiet before; payoff the burst; decay the return to pacing)
        self.surge_in -= dt;
        if self.surge_in <= 0.0 {
            self.surge_left = self.rng.random_range(2.5..4.5);
            self.surge_in = self.rng.random_range(24.0..40.0);
        }
        let surging = self.surge_left > 0.0;
        if surging {
            self.surge_left -= dt;
        }

        // spawn pacing
        self.next_spawn -= dt;
        if self.next_spawn <= 0.0 {
            self.spawn_meteor(false);
            self.next_spawn = if surging {
                self.rng.random_range(0.08..0.22)
            } else {
                self.rng.random_range(0.4..1.8)
            };
        }
        self.bolide_in -= dt;
        if self.bolide_in <= 0.0 {
            self.spawn_meteor(true);
            self.flash = 1.0;
            self.bolide_in = self.rng.random_range(10.0..20.0);
        }

        let mut new_bolides = Vec::new();
        self.meteors.retain_mut(|m| {
            m.life -= dt;
            m.age += dt;
            // slight gravity curves the streak downward as it falls
            m.vy += 14.0 * dt;
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            if m.life <= 0.0 || m.y > h as f32 + 4.0 || m.x < -20.0 || m.x > w as f32 + 20.0 {
                if m.bolide && m.life <= 0.0 {
                    new_bolides.push(());
                }
                return false;
            }
            // lifecycle envelope: ease in, ease out — no popping
            let env = ease_smooth(m.age / 0.15) * ease_smooth(m.life / 0.4);
            let b = m.bright * env;
            // ion trail: fading streak behind the head, additively bleeding
            // over the starfield
            let sp = (m.vx * m.vx + m.vy * m.vy).sqrt().max(1.0);
            let (ux, uy) = (m.vx / sp, m.vy / sp);
            // brightness and tint vary along the streak, so draw it as
            // short AA segments rather than one line — each segment slides
            // subcell with the head instead of snapping per sample
            for i in 1..=m.trail {
                let f = 1.0 - i as f32 / m.trail as f32;
                let jitter = ((m.x * 13.0 + m.y * 7.0 + i as f32 * 31.0) as i32 % 3) as f32 - 1.0;
                let tx = m.x - ux * i as f32 * 1.2 + jitter * 0.3;
                let ty = m.y - uy * i as f32 * 1.2;
                let c = lerp(m.color, (245, 250, 255), f);
                line_f(
                    canvas,
                    tx,
                    ty,
                    tx - ux * 1.2,
                    ty - uy * 1.2,
                    c,
                    f * f * b * 0.5,
                );
            }
            // head: hot white core with a tinted glow bleed
            dot(canvas, m.x, m.y, (255, 255, 255), b.min(1.0));
            if m.bolide {
                glow_f(canvas, m.x, m.y, 3.0, (255, 190, 120), 0.55 * env);
            } else {
                glow_f(canvas, m.x, m.y, 2.0, m.color, 0.3 * env);
            }
            true
        });
        for _ in new_bolides {
            self.flash = 1.0;
        }
    }
}
