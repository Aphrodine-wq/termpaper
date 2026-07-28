//! Lanterns: a river-festival night. Paper lanterns rise from a far shore
//! into a pure black sky, in three parallax layers — far ones are dim
//! drifting specks, near ones show banded paper, a hot inner flame that
//! leans against the wind, and a swaying tassel. Below the horizon, dark
//! water doubles every lantern as a wobbling reflection streak, and crowd
//! dots twinkle along the far shore. Event rhythm: a RELEASE (ground-glow
//! anticipation on the shore, staggered formation climb, spread and decay),
//! a lantern BARGE drifting across the water, small gusts, a rare wind
//! SHEAR that bends the whole sky (stronger aloft), and an occasional
//! shooting star. Bright events light their surroundings: the release glow
//! flares the shore, the horizon, and every reflection.

use super::noise::vnoise;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use crate::physics::spring_damper;
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::TAU;

/// Hard cap on live lanterns so bursts can't grow the vec without bound.
const MAX_LANTERNS: usize = 160;
/// Ground-glow anticipation time before a release launches.
const GLOW_DUR: f32 = 2.2;
/// How long a burst keeps its formation cohesion before spreading.
const FORMATION_TIME: f32 = 6.0;
/// Hard cap on barges crossing the water at once.
const MAX_BARGES: usize = 2;

/// Per-layer tuning: (brightness, rise speed cells/s, glow radius).
const LAYERS: [(f32, f32, i32); 3] = [
    (0.46, 1.1, 0), // far
    (0.68, 2.0, 1), // mid
    (1.0, 3.1, 2),  // near
];

struct Lantern {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    anchor: f32,   // horizontal anchor it springs toward
    retarget: f32, // seconds until the anchor wanders
    layer: usize,
    phase: f32, // sway phase (shared within a burst -> group tilt)
    seed: f32,  // per-lantern flicker offset
    age: f32,
    rise: f32, // target rise speed
    wob: f32,  // sway amplitude
    delay: f32,  // >0: still waiting on the shore (staggered launch)
    delay0: f32, // initial delay, for the ember build-up
    burst: bool, // member of a release burst (stronger gust coupling)
}

struct Star {
    x: i32,
    y: i32,
    bright: f32,
    phase: f32,
}

/// A distant festival-goer light on the far shore.
struct Crowd {
    x: i32,
    row: i32, // offset above the horizon (0 = on the shore line)
    bright: f32,
    phase: f32,
}

/// A drifting boat carrying a small cluster of lanterns.
struct Barge {
    x: f32,
    dir: f32,
    speed: f32,
    lights: Vec<(f32, i32, f32)>, // (dx from bow, rows above deck, seed)
}

/// Release rhythm: Idle -> Glow (shore-glow builds) -> launched (residual
/// glow decays while the formation climbs).
#[derive(Clone, Copy, PartialEq)]
enum Release {
    Idle,
    Glow(f32), // elapsed
}

/// Spawn a solo drifter. `anywhere` scatters the initial population across
/// the whole sky so the scene opens already alive; respawns enter at the
/// far shore and climb.
fn spawn_solo(rng: &mut StdRng, w: usize, horizon: f32, anywhere: bool) -> Lantern {
    let r = rng.random::<f32>();
    let layer = if r < 0.38 {
        0
    } else if r < 0.78 {
        1
    } else {
        2
    };
    let (_, rise_base, _) = LAYERS[layer];
    let x = rng.random_range(0.0..w as f32);
    Lantern {
        x,
        y: if anywhere {
            rng.random_range(0.0..horizon)
        } else {
            horizon + rng.random_range(0.0..1.5)
        },
        vx: 0.0,
        vy: 0.0,
        anchor: x,
        retarget: rng.random_range(2.0..6.0),
        layer,
        phase: rng.random_range(0.0..TAU),
        seed: rng.random::<f32>(),
        age: if anywhere {
            rng.random_range(0.0..4.0)
        } else {
            0.0
        },
        rise: rise_base * rng.random_range(0.8..1.25),
        wob: rng.random_range(0.8..2.2),
        delay: 0.0,
        delay0: 0.0,
        burst: false,
    }
}

pub struct Lanterns {
    rng: StdRng,
    detail: Detail,
    core_c: (u8, u8, u8),   // bright paper near the flame
    glow_c: (u8, u8, u8),   // mid paper / glow bleed / reflections
    rim_c: (u8, u8, u8),    // caps, ribs, tassel, hull
    flame_c: (u8, u8, u8),  // near-white flame core
    star_c: (u8, u8, u8),
    lanterns: Vec<Lantern>,
    stars: Vec<Star>,
    crowd: Vec<Crowd>,
    barges: Vec<Barge>,
    base: usize, // solo population to maintain; bursts exceed it temporarily
    release: Release,
    release_x: f32,
    glow_resid: f32, // decaying shore glow right after launch
    next_release: f32,
    gust_t: f32, // <0 inactive, else elapsed
    gust_dur: f32,
    gust_dir: f32,
    gust_str: f32,
    next_gust: f32,
    shear_t: f32, // <0 inactive: rare sky-bending wind
    shear_dur: f32,
    shear_dir: f32,
    shear_str: f32,
    next_shear: f32,
    next_barge: f32,
    meteor: Option<(f32, f32, f32, f32, f32)>, // x, y, vx, vy, age
    next_meteor: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Lanterns {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (core_c, glow_c, rim_c) = match theme {
            Some("jade") => ((170, 255, 200), (90, 230, 150), (45, 140, 90)),
            Some("violet") => ((225, 185, 255), (180, 120, 250), (110, 65, 160)),
            _ => ((255, 200, 110), (255, 160, 60), (190, 110, 45)), // warm
        };
        Lanterns {
            rng,
            detail,
            core_c,
            glow_c,
            rim_c,
            flame_c: lerp(core_c, (255, 255, 255), 0.5),
            star_c: (150, 160, 190),
            lanterns: Vec::new(),
            stars: Vec::new(),
            crowd: Vec::new(),
            barges: Vec::new(),
            base: 0,
            release: Release::Idle,
            release_x: 0.0,
            glow_resid: 0.0,
            next_release: 8.0,
            gust_t: -1.0,
            gust_dur: 2.5,
            gust_dir: 1.0,
            gust_str: 6.0,
            next_gust: 5.0,
            shear_t: -1.0,
            shear_dur: 5.0,
            shear_dir: 1.0,
            shear_str: 4.0,
            next_shear: 18.0,
            next_barge: 11.0,
            meteor: None,
            next_meteor: 14.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// Far-shore line: lanterns rise from here, water lies below.
    fn horizon(&self) -> i32 {
        ((self.h as f32 * 0.78) as i32).min(self.h as i32 - 1).max(0)
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.lanterns.clear();
        self.stars.clear();
        self.crowd.clear();
        self.barges.clear();
        self.release = Release::Idle;
        self.meteor = None;
        if w == 0 || h == 0 {
            self.base = 0;
            return;
        }
        let hz = self.horizon() as f32;
        self.base = self
            .detail
            .scale((w * h / 200) as f32, 10)
            .min(110);
        for _ in 0..self.base {
            let l = spawn_solo(&mut self.rng, w, hz, true);
            self.lanterns.push(l);
        }
        let n_stars = self
            .detail
            .scale((w * h / 400) as f32 * density_for(w, h), 10)
            .min(180);
        for _ in 0..n_stars {
            self.stars.push(Star {
                x: self.rng.random_range(0..w as i32),
                y: self.rng.random_range(0..(hz as i32 * 3 / 4).max(1)),
                bright: self.rng.random_range(0.10..0.30),
                phase: self.rng.random_range(0.0..TAU),
            });
        }
        // festival crowd along the far shore: a ragged band of tiny lights
        let n_crowd = self.detail.scale(w as f32 / 4.5, 8).min(140);
        for _ in 0..n_crowd {
            self.crowd.push(Crowd {
                x: self.rng.random_range(0..w as i32),
                row: if self.rng.random::<f32>() < 0.3 { 1 } else { 0 },
                bright: self.rng.random_range(0.10..0.32),
                phase: self.rng.random_range(0.0..TAU),
            });
        }
    }

    /// Launch a staggered burst from the release point on the shore.
    fn launch_burst(&mut self, w: usize) {
        let hz = self.horizon() as f32;
        let n = self
            .detail
            .scale(self.rng.random_range(6..=10) as f32, 5)
            .min(14);
        let group_phase = self.rng.random_range(0.0..TAU); // shared: tilts together
        let spread = (w as f32 * 0.08).max(2.0);
        for i in 0..n {
            let r = self.rng.random::<f32>();
            let layer = if r < 0.2 {
                0
            } else if r < 0.62 {
                1
            } else {
                2
            };
            let (_, rise_base, _) = LAYERS[layer];
            let x = (self.release_x + self.rng.random_range(-spread..spread))
                .clamp(1.0, (w as f32 - 1.0).max(1.0));
            let delay = i as f32 * self.rng.random_range(0.14..0.24);
            self.lanterns.push(Lantern {
                x,
                y: hz,
                vx: 0.0,
                vy: 0.0,
                anchor: x,
                retarget: self.rng.random_range(3.0..7.0),
                layer,
                phase: group_phase + self.rng.random_range(-0.3..0.3),
                seed: self.rng.random::<f32>(),
                age: 0.0,
                rise: rise_base * self.rng.random_range(1.15..1.45), // formation climbs faster
                wob: self.rng.random_range(0.6..1.4),
                delay,
                delay0: delay,
                burst: true,
            });
        }
        while self.lanterns.len() > MAX_LANTERNS {
            self.lanterns.remove(0);
        }
    }
}

impl Scene for Lanterns {
    fn name(&self) -> &'static str {
        "lanterns"
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
        self.t += dt;
        let t = self.t;
        let (wf, hf) = (w as f32, h as f32);
        let hz = self.horizon();
        let hzf = hz as f32;
        let water_rows = (h as i32 - hz - 1).max(0);

        // --- release scheduling (anticipation -> payoff -> decay) ---
        self.next_release -= dt;
        match self.release {
            Release::Idle => {
                if self.next_release <= 0.0 {
                    self.release_x = self.rng.random_range(wf * 0.2..wf * 0.8);
                    self.release = Release::Glow(0.0);
                }
            }
            Release::Glow(e) => {
                let e = e + dt;
                if e >= GLOW_DUR {
                    self.launch_burst(w);
                    self.glow_resid = 1.0;
                    self.release = Release::Idle;
                    self.next_release = self.rng.random_range(20.0..30.0);
                } else {
                    self.release = Release::Glow(e);
                }
            }
        }
        self.glow_resid = (self.glow_resid - dt * 0.4).max(0.0);
        // anticipation envelope: eased build-up, tiny breathing on top
        let glow_env = match self.release {
            Release::Glow(e) => ease_smooth(e / GLOW_DUR) * (0.9 + 0.1 * (t * 5.0).sin()),
            Release::Idle => 0.0,
        } + self.glow_resid * 0.7;

        // --- gusts: eased wind that tilts whole groups sideways ---
        self.next_gust -= dt;
        if self.next_gust <= 0.0 && self.gust_t < 0.0 {
            self.gust_t = 0.0;
            self.gust_dur = self.rng.random_range(2.0..3.5);
            self.gust_dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
            self.gust_str = self.rng.random_range(4.0..9.0);
            self.next_gust = self.rng.random_range(7.0..14.0);
        }
        let mut gust_acc = 0.0;
        let mut gust_env = 0.0;
        if self.gust_t >= 0.0 {
            self.gust_t += dt;
            if self.gust_t >= self.gust_dur {
                self.gust_t = -1.0;
            } else {
                let ramp = ease_smooth(self.gust_t / 0.8);
                let fall = 1.0 - ease_smooth((self.gust_t - (self.gust_dur - 0.9)) / 0.9);
                gust_env = ramp * fall;
                gust_acc = self.gust_dir * self.gust_str * gust_env;
            }
        }

        // --- wind shear: rare, long, bends the sky harder the higher up ---
        self.next_shear -= dt;
        if self.next_shear <= 0.0 && self.shear_t < 0.0 {
            self.shear_t = 0.0;
            self.shear_dur = self.rng.random_range(4.5..7.0);
            self.shear_dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
            self.shear_str = (wf * 0.05).clamp(2.5, 9.0) * self.rng.random_range(0.8..1.2);
            self.next_shear = self.rng.random_range(26.0..42.0);
        }
        let mut shear_env = 0.0;
        if self.shear_t >= 0.0 {
            self.shear_t += dt;
            if self.shear_t >= self.shear_dur {
                self.shear_t = -1.0;
            } else {
                let ramp = ease_smooth(self.shear_t / 1.6);
                let fall = 1.0 - ease_smooth((self.shear_t - (self.shear_dur - 1.8)) / 1.8);
                shear_env = ramp * fall;
            }
        }
        // horizontal bend offset for something at height y (0 at the shore)
        let bend_at = |y: f32| -> f32 {
            if shear_env <= 0.0 || hzf <= 0.0 {
                return 0.0;
            }
            let alt = ((hzf - y) / hzf).clamp(0.0, 1.0);
            self.shear_dir * self.shear_str * shear_env * alt.powf(1.6)
        };

        // --- rare shooting star in the far background ---
        self.next_meteor -= dt;
        if self.next_meteor <= 0.0 && self.meteor.is_none() {
            let from_left = self.rng.random::<bool>();
            self.meteor = Some((
                if from_left { -4.0 } else { wf + 4.0 },
                self.rng.random_range(1.0..hzf * 0.35).max(1.0),
                if from_left {
                    self.rng.random_range(45.0..70.0)
                } else {
                    -self.rng.random_range(45.0..70.0)
                },
                self.rng.random_range(6.0..12.0),
                0.0,
            ));
            self.next_meteor = self.rng.random_range(22.0..45.0);
        }

        // --- barges: a slow lantern-lit boat crosses the water ---
        self.next_barge -= dt;
        if self.next_barge <= 0.0 && self.barges.len() < MAX_BARGES {
            let from_left = self.rng.random::<bool>();
            let n_lights = self.rng.random_range(2..=4);
            let mut lights = Vec::with_capacity(n_lights);
            for i in 0..n_lights {
                lights.push((
                    i as f32 * self.rng.random_range(1.6..2.6),
                    if self.rng.random::<f32>() < 0.4 { 1 } else { 0 },
                    self.rng.random::<f32>(),
                ));
            }
            self.barges.push(Barge {
                x: if from_left { -8.0 } else { wf + 8.0 },
                dir: if from_left { 1.0 } else { -1.0 },
                speed: (wf * self.rng.random_range(0.006..0.014)).max(0.8),
                lights,
            });
            self.next_barge = self.rng.random_range(16.0..28.0);
        }
        for b in &mut self.barges {
            b.x += b.dir * b.speed * dt;
        }
        self.barges.retain(|b| b.x > -14.0 && b.x < wf + 14.0);

        // --- lantern steering + integration ---
        let base = self.base;
        let mut live = self.lanterns.len();
        let mut trim = 0usize; // burst members past the top, removed below
        for l in &mut self.lanterns {
            if l.delay > 0.0 {
                l.delay -= dt; // waiting on the shore for its stagger cue
                continue;
            }
            l.age += dt;
            l.retarget -= dt;
            if l.retarget <= 0.0 {
                l.anchor = self.rng.random_range(0.0..wf);
                l.retarget = self.rng.random_range(3.0..7.0);
            }
            // sine sway + sky bend riding on spring steering toward the anchor
            let sway = (t * 0.6 + l.phase).sin() * l.wob * (1.0 + gust_env * 1.5);
            let ax = spring_damper(l.x, l.vx, l.anchor + sway + bend_at(l.y), 1.4, 2.0);
            l.vx += ax * dt;
            // gusts push everyone, burst groups lean harder (shared phase)
            l.vx += gust_acc * if l.burst { 1.6 } else { 1.0 } * dt;
            // rise: ease toward the target climb rate, never linear
            let ay = spring_damper(l.vy, 0.0, -l.rise, 2.0, 1.6);
            l.vy += ay * dt;
            l.x = (l.x + l.vx * dt).rem_euclid(wf);
            l.y += l.vy * dt;
            // formation decays: after the climb, resume solo drift
            if l.burst && l.age > FORMATION_TIME {
                l.burst = false;
                l.phase = self.rng.random_range(0.0..TAU);
                l.wob = self.rng.random_range(0.8..2.2);
                l.retarget = 0.0;
            }
            if l.y < -6.0 {
                if live > base && l.burst {
                    trim += 1; // excess burst member: retire instead of respawn
                    live -= 1;
                    l.delay = f32::INFINITY; // parked; swept by retain below
                    l.y = -100.0;
                } else {
                    *l = spawn_solo(&mut self.rng, w, hzf, false);
                }
            }
        }
        if trim > 0 {
            self.lanterns.retain(|l| l.y > -50.0);
        }

        // per-lantern brightness, shared by the body and its reflection
        let bright_of = |l: &Lantern| -> f32 {
            let flick = 0.78 + 0.3 * vnoise(l.seed * 60.0, t * 1.8, 13);
            let fade_in = ease_smooth(l.age / 1.2);
            let zone = (hzf * 0.18).max(2.0);
            let fade_top = ease_smooth(((l.y + 2.0) / zone).clamp(0.0, 1.0));
            LAYERS[l.layer].0 * flick * fade_in * fade_top
        };

        // ===================== draw =====================
        canvas.clear((0, 0, 0));

        // far background: sparse stars with a gentle eased twinkle
        for s in &self.stars {
            let tw = 0.72 + 0.28 * (t * 0.8 + s.phase).sin();
            canvas.set(s.x, s.y, scale(self.star_c, s.bright * tw));
        }

        // shooting star: bright head, short additive trail
        if let Some((mx, my, mvx, mvy, mage)) = &mut self.meteor {
            *mage += dt;
            *mx += *mvx * dt;
            *my += *mvy * dt;
            let fade = 1.0 - (*mage / 1.5).clamp(0.0, 1.0);
            if fade <= 0.0 || *mx < -8.0 || *mx > wf + 8.0 || *my > hzf * 0.6 {
                self.meteor = None;
            } else {
                let head = (230, 235, 255);
                canvas.set_f(*mx, *my, scale(head, fade));
                let spd = (*mvx * *mvx + *mvy * *mvy).sqrt().max(1.0);
                for k in 1..=7 {
                    let f = 1.0 - k as f32 / 8.0;
                    canvas.add(
                        (*mx - *mvx / spd * k as f32 * 1.4) as i32,
                        (*my - *mvy / spd * k as f32 * 1.4) as i32,
                        scale(head, fade * f * f * 0.4),
                    );
                }
            }
        }

        // the far shore: a whisper of a horizon line, warmed by releases
        if water_rows > 0 {
            canvas.fill_row(hz as usize, scale(self.glow_c, 0.06 + glow_env * 0.05));
        }

        // crowd dots: tiny twinkling lights; the release glow flares the
        // ones near the launch point (bright events touch their neighbours)
        for c in &self.crowd {
            let tw = 0.66 + 0.34 * (t * 1.1 + c.phase).sin();
            let near = if glow_env > 0.01 {
                let d = (c.x as f32 - self.release_x).abs();
                glow_env * (1.0 - d / (wf * 0.18).max(6.0)).max(0.0)
            } else {
                0.0
            };
            let b = (c.bright * tw + near * 0.35).min(0.8);
            canvas.add(c.x, hz - c.row, scale(self.glow_c, b));
        }

        // release shore-glow: a warm pool of light at the launch point
        if glow_env > 0.01 {
            let r = ((wf.min(hf) * 0.12).clamp(3.0, 7.0)) as i32;
            let gx = self.release_x as i32;
            glow(canvas, gx, hz, r, self.glow_c, glow_env * 0.22);
            // hot spot right at the source, and its shimmer in the water
            glow(canvas, gx, hz, 1, self.core_c, glow_env * 0.3);
            if water_rows > 0 {
                for j in 1..=water_rows.min(3) {
                    let f = 1.0 - j as f32 / 4.0;
                    canvas.add(gx, hz + j, scale(self.glow_c, glow_env * 0.12 * f));
                }
            }
        }

        // ambient water sheen: a whisper of drifting dapple so the lower
        // plane reads as water, not void; lifts with the release glow
        if water_rows > 0 {
            let sheen = 0.030 + glow_env * 0.05;
            for j in 1..=water_rows {
                let row_fade = 1.0 / (1.0 + j as f32 * 0.30);
                for x in 0..w as i32 {
                    let n = vnoise(x as f32 * 0.09, t * 0.45 + j as f32 * 13.7, 31);
                    if n > 0.72 {
                        let f = (n - 0.72) / 0.28;
                        canvas.add(x, hz + j, scale(self.glow_c, sheen * row_fade * f));
                    }
                }
            }
        }

        // water reflections: every lantern doubled below the horizon as a
        // wobbling streak — near layers streak longer, releases brighten all
        if water_rows > 0 {
            let refl_boost = 1.0 + glow_env * 0.6;
            for l in &self.lanterns {
                if l.delay > 0.0 || l.y >= hzf {
                    continue;
                }
                let b = bright_of(l);
                if b < 0.03 {
                    continue;
                }
                let depth = hzf - l.y;
                let streak = (1 + l.layer as i32 + (depth * 0.10) as i32)
                    .min(water_rows)
                    .max(1);
                let bend = bend_at(l.y) * 0.6; // the bent sky bends the water too
                for j in 1..=streak {
                    let wob = (vnoise(l.seed * 40.0 + j as f32 * 0.9, t * 1.3, 7) - 0.5)
                        * (1.2 + j as f32 * 0.6);
                    let f = 1.0 - j as f32 / (streak + 1) as f32;
                    canvas.add(
                        (l.x + bend + wob) as i32,
                        hz + j,
                        scale(self.glow_c, b * 0.22 * f * f * refl_boost),
                    );
                }
            }
        }

        // barges on the water: a dim hull line, mast lanterns, reflections
        for b in &self.barges {
            // eased fade in/out at the screen edges — never a pop
            let span = b.lights.last().map(|l| l.0).unwrap_or(0.0);
            let head = if b.dir > 0.0 { b.x + span } else { b.x };
            let tail = if b.dir > 0.0 { b.x } else { b.x + span };
            let edge = (head.min(wf - 1.0 - tail.min(wf)) + 8.0) / 8.0;
            let env = ease_smooth(edge.clamp(0.0, 1.0));
            if env <= 0.01 {
                continue;
            }
            let deck = hz; // distant water, right at the shore line
            for &(dx, _, _) in &b.lights {
                canvas.add((b.x + dx * b.dir) as i32, deck, scale(self.rim_c, 0.10 * env));
            }
            for &(dx, rows_up, seed) in &b.lights {
                let lx = (b.x + dx * b.dir) as i32;
                let flick = 0.75 + 0.3 * vnoise(seed * 50.0, t * 1.6, 17);
                let lb = flick * env * 0.7;
                canvas.add(lx, deck - rows_up, scale(self.core_c, lb));
                glow(canvas, lx, deck - rows_up, 1, self.glow_c, lb * 0.25);
                for j in 1..=water_rows.min(2) {
                    let wob = (vnoise(seed * 30.0 + j as f32, t * 1.2, 9) - 0.5) * 1.4;
                    let f = 1.0 - j as f32 / 3.0;
                    canvas.add(
                        (lx as f32 + wob) as i32,
                        hz + j,
                        scale(self.glow_c, lb * 0.20 * f),
                    );
                }
            }
        }

        // lanterns, far to near so closer bodies paint over farther glow
        let big = w >= 120 && h >= 40;
        for layer in 0..3 {
            let (_, _, gr) = LAYERS[layer];
            for l in &self.lanterns {
                if l.layer != layer {
                    continue;
                }
                if l.delay > 0.0 {
                    // grounded, awaiting launch: an ember brightening toward its cue
                    if l.delay0 > 0.0 {
                        let e = 1.0 - l.delay / l.delay0;
                        let c = scale(
                            self.glow_c,
                            0.35 * e * (0.7 + 0.3 * (t * 7.0 + l.seed * 9.0).sin()),
                        );
                        canvas.add(l.x as i32, hz, c);
                    }
                    continue;
                }
                let b = bright_of(l);
                if b < 0.015 {
                    continue;
                }
                // wind lean: the flame lags the sway, the shear bends it too
                let lean = (-l.vx * 0.10 + bend_at(l.y) * 0.12).clamp(-1.5, 1.5);
                let (ix, iy) = (l.x as i32, l.y as i32);
                let (bw, bh) = match layer {
                    0 => (1, 1),
                    1 => (2, 3),
                    _ => {
                        if big {
                            (5, 6)
                        } else {
                            (3, 4)
                        }
                    }
                };
                if layer == 0 {
                    // far: a single breathing ember
                    canvas.add(ix, iy, scale(self.glow_c, b));
                    continue;
                }
                // flame: hot core with its own faster flicker, offset by lean
                let flame_f = 0.85 + 0.35 * vnoise(l.seed * 90.0, t * 3.1, 29);
                let max_off = (bw / 2 - 1).max(0);
                let flame_off = lean.round().clamp(-max_off as f32, max_off as f32) as i32;
                let flame_row = bh - 2; // the flame sits low in the body
                // banded paper body: shaved corner caps, ribbed rows, rim
                // darker at the edges, warmest around the flame
                for dy in 0..bh {
                    let cap = dy == 0 || dy == bh - 1;
                    let shrink = if bw >= 3 && cap { 1 } else { 0 };
                    for dx in shrink..bw - shrink {
                        let px = ix + dx - bw / 2;
                        let py = iy + dy - bh / 2;
                        if cap {
                            canvas.set(px, py, scale(self.rim_c, b * 0.75));
                            continue;
                        }
                        if dy == flame_row && dx as i32 - bw as i32 / 2 == flame_off {
                            canvas.set(px, py, scale(self.flame_c, (b * flame_f).min(1.3)));
                            continue;
                        }
                        let rib = if dy % 2 == 0 { 0.80 } else { 1.0 }; // paper ribs
                        let edge = dx == 0 || dx == bw - 1;
                        // paper glows from within: brighter toward the flame
                        let fd = ((dx as i32 - bw as i32 / 2 - flame_off).abs()
                            + (dy as i32 - flame_row as i32).abs()) as f32;
                        let inner = (1.0 - fd / (bw + bh) as f32 * 1.6).max(0.35);
                        let paper = if edge {
                            lerp(self.rim_c, self.glow_c, 0.45)
                        } else {
                            lerp(self.glow_c, self.core_c, inner * 0.7)
                        };
                        canvas.set(px, py, scale(paper, b * rib * inner));
                    }
                }
                // tassel: a dark thread below the body, lagging the sway
                let tas_swing =
                    ((t * 0.6 + l.phase - 0.9).sin() * l.wob * 0.3 + lean).round() as i32;
                let tas_len = if big { 2 } else { 1 };
                for k in 1..=tas_len {
                    let f = 1.0 - k as f32 * 0.3;
                    canvas.set(
                        ix + tas_swing,
                        iy + bh / 2 + k,
                        scale(self.rim_c, b * 0.55 * f),
                    );
                }
                // glow bleed: the flame touches the night around it
                if gr > 0 && b > 0.1 {
                    glow(canvas, ix, iy, gr, self.glow_c, b * 0.30);
                }
            }
        }
    }
}
