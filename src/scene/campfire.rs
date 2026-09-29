//! Night campfire in the forest: flame, embers, smoke, and a figure poking
//! the fire.
//!
//! The fire is the only light. A point light with screen-correct falloff
//! tints every plane — trunks on their fire-facing side, the ground pool,
//! the tent's lit face, the figure's rim, the smoke's underside — and it
//! flickers with smoothed noise so nothing pops. The flame itself is a small
//! Doom-style heat buffer shaped by a fuel mask and rendered through the
//! theme's ramp; embers are verlet particles with noise turbulence; smoke is
//! a column of expanding discs.
//!
//! 30 s loop: breathing and glances, a reach and a poke that bursts embers
//! and flares the whole scene, an owl's eyes in the dark, a log settling,
//! a gust. Trunks, canopy, ground and the foreground crop are plates.
use super::{noise::{fbm, hash2, vnoise}, Detail, Scene};
use crate::anim::{
    beat::Timeline,
    draw::{add_f, blend_f, capsule, capsule_shaded, ellipse_f, gradient_v, polygon_fill, polygon_fill_shaded, ramp, soft_shadow, stroke_f, Plate, Surface},
    light::PointLight,
    rig::{ik2, Follow},
    scenery::{moon, StarField},
    stage::Stage,
    Rgb,
};
use crate::canvas::{approach, lerp, scale, Canvas};
use crate::render::Pixels;
use rand::{rngs::StdRng, RngExt};

const LOOP: f32 = 30.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Pine,
    Autumn,
    Snow,
}

struct Theme {
    kind: Kind,
    sky: [(f32, Rgb); 3],
    trunk: Rgb,
    trunk_lit: Rgb,
    ground: Rgb,
    ground_lit: Rgb,
    canopy: Rgb,
    tent: Rgb,
    smoke: Rgb,
    fire: [(f32, Rgb); 6],
    light: Rgb,
    figure: Rgb,
}

fn theme(name: Option<&str>) -> Theme {
    let fire_classic = [
        (0.0, (0, 0, 0)),
        (0.15, (70, 12, 4)),
        (0.35, (200, 55, 10)),
        (0.6, (250, 150, 30)),
        (0.85, (255, 230, 120)),
        (1.0, (255, 255, 225)),
    ];
    match name {
        Some("autumn") => Theme {
            kind: Kind::Autumn,
            sky: [(0.0, (8, 6, 18)), (0.6, (16, 12, 30)), (1.0, (28, 18, 34))],
            trunk: (36, 26, 20),
            trunk_lit: (225, 140, 60),
            ground: (62, 40, 18),
            ground_lit: (235, 150, 60),
            canopy: (14, 10, 10),
            tent: (110, 60, 30),
            smoke: (70, 60, 58),
            fire: [
                (0.0, (0, 0, 0)),
                (0.15, (80, 10, 4)),
                (0.35, (215, 45, 10)),
                (0.6, (250, 130, 25)),
                (0.85, (255, 215, 110)),
                (1.0, (255, 250, 220)),
            ],
            light: (255, 150, 60),
            figure: (24, 16, 14),
        },
        Some("snow") => Theme {
            kind: Kind::Snow,
            sky: [(0.0, (6, 10, 26)), (0.6, (12, 18, 40)), (1.0, (24, 30, 52))],
            trunk: (14, 12, 16),
            trunk_lit: (240, 170, 90),
            ground: (150, 168, 200),
            ground_lit: (255, 200, 130),
            canopy: (8, 8, 14),
            tent: (60, 90, 120),
            smoke: (80, 80, 88),
            fire: fire_classic,
            light: (255, 175, 90),
            figure: (16, 14, 20),
        },
        _ => Theme {
            kind: Kind::Pine,
            sky: [(0.0, (3, 6, 18)), (0.6, (8, 14, 32)), (1.0, (14, 24, 40))],
            trunk: (22, 18, 16),
            trunk_lit: (210, 115, 50),
            ground: (18, 14, 11),
            ground_lit: (200, 110, 45),
            canopy: (4, 8, 8),
            tent: (40, 80, 70),
            smoke: (60, 56, 56),
            fire: fire_classic,
            light: (255, 160, 70),
            figure: (16, 12, 10),
        },
    }
}

struct Ember {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    heat: f32,
}

struct Puff {
    x: f32,
    y: f32,
    r: f32,
    age: f32,
    life: f32,
}

struct Mote {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
    size: f32,
}

pub struct Campfire {
    rng: StdRng,
    detail: Detail,
    pixels: Pixels,
    theme: Theme,
    seed: u32,
    w: usize,
    h: usize,
    stage: Stage,
    tl: Timeline,
    t: f32,
    // composition
    ground_y: f32,
    fire_x: f32,
    fire_y: f32,
    hero: f32,
    // plates
    canopy: Plate,
    trunks: Plate,
    ground: Plate,
    fg: Plate,
    stars: StarField,
    // fire
    fw: usize,
    fh: usize,
    heat: Vec<f32>,
    fuel: Vec<f32>,
    base_heat: f32,
    flicker: f32,
    flare: f32,
    embers: Vec<Ember>,
    puffs: Vec<Puff>,
    puff_acc: f32,
    motes: Vec<Mote>,
    // figure
    lean: Follow,
    hood: Follow,
    stick: Follow,
    head_turn: Follow,
    breath_t: f32,
    facing: f32,
    // events
    owl_side: f32,
    log_tilt: f32,
    lut: Vec<f32>,
}

impl Campfire {
    pub fn new(mut rng: StdRng, theme_name: Option<&str>, detail: Detail, pixels: Pixels) -> Self {
        let seed: u32 = rng.random();
        let tl = Timeline::new(LOOP)
            .beat("reach", 7.0, 0.8)
            .beat("poke", 8.5, 0.5)
            .beat("burst", 9.0, 0.3)
            .beat("back", 9.4, 0.9)
            .beat("owl", 14.0, 2.2)
            .beat("settle", 18.0, 0.3)
            .beat("lantern", 20.0, LOOP - 20.0)
            .beat("gust", 24.0, 2.5);
        Campfire {
            rng,
            detail,
            pixels,
            theme: theme(theme_name),
            seed,
            w: 0,
            h: 0,
            stage: Stage::new(1, 1, pixels),
            tl,
            t: 0.0,
            ground_y: 0.0,
            fire_x: 0.0,
            fire_y: 0.0,
            hero: 1.0,
            canopy: Plate::new(1, 1),
            trunks: Plate::new(1, 1),
            ground: Plate::new(1, 1),
            fg: Plate::new(1, 1),
            stars: StarField::new(seed ^ 0x77, 4.0, (220, 225, 240)),
            fw: 0,
            fh: 0,
            heat: Vec::new(),
            fuel: Vec::new(),
            base_heat: 1.0,
            flicker: 1.0,
            flare: 0.0,
            embers: Vec::new(),
            puffs: Vec::new(),
            puff_acc: 0.0,
            motes: Vec::new(),
            lean: Follow::new(0.0, 25.0, 6.0),
            hood: Follow::new(0.0, 60.0, 7.0),
            stick: Follow::new(0.0, 40.0, 6.0),
            head_turn: Follow::new(0.0, 20.0, 5.0),
            breath_t: 0.0,
            facing: 1.0,
            owl_side: 1.0,
            log_tilt: 0.0,
            lut: Vec::new(),
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.stage = Stage::new(w, h, self.pixels);
        let st = self.stage;
        self.hero = st.hero(1.0);
        let hero = self.hero;
        self.ground_y = st.horizon(0.58, 0.62).round();
        self.fire_x = (st.wf * st.pick(0.47, 0.5)).round();
        self.fire_y = self.ground_y + (st.hf - self.ground_y) * st.pick(0.32, 0.26);
        let seed = self.seed;

        // canopy: a black mass along the top with fbm gaps for stars
        self.canopy = Plate::new(w, h);
        let can_h = st.hf * st.pick(0.26, 0.2);
        for x in 0..w {
            let u = x as f32 / st.wf;
            let edge = can_h * (0.45 + 0.55 * fbm(u * 5.0, 0.3, 3, seed ^ 0x11));
            let gap = fbm(u * 3.0 + 9.0, 0.7, 2, seed ^ 0x12);
            let edge = if gap > 0.62 { edge * (1.0 - (gap - 0.62) * 2.2).max(0.15) } else { edge };
            for y in 0..edge as i32 {
                let v = y as f32 / edge.max(1.0);
                // ragged lower edge
                let a = if v > 0.8 { 1.0 - (v - 0.8) / 0.2 * hash2(x as i32, y, seed) } else { 1.0 };
                self.canopy.blend_px(x as i32, y, self.theme.canopy, a);
            }
        }

        // trunks: back to front, wider and darker toward the camera; each is
        // painted with a horizontal gradient brighter on its fire-facing side
        self.trunks = Plate::new(w, h);
        let count = match self.detail {
            Detail::Low => 5,
            Detail::Medium => 8,
            Detail::High => 11,
        };
        let mut placed: Vec<(f32, f32, f32)> = Vec::new(); // x, width, depth
        for i in 0..count {
            let depth = i as f32 / (count - 1).max(1) as f32; // 0 far .. 1 near
            let mut x = hash2(i as i32, 3, seed) * st.wf;
            // keep the fire's own column clear so the flame reads
            if (x - self.fire_x).abs() < hero * 0.12 {
                x += hero * 0.2 * (x - self.fire_x).signum();
            }
            let wd = hero * (0.012 + 0.04 * depth * depth) + 1.0;
            placed.push((x, wd, depth));
        }
        placed.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
        for &(x, wd, depth) in &placed {
            let base = self.ground_y + (st.hf - self.ground_y) * (0.05 + 0.5 * depth);
            let top = -2.0;
            let col = lerp(scale(self.theme.trunk, 0.5), self.theme.trunk, depth);
            let facing = (self.fire_x - x).signum();
            let seedt = seed ^ ((x as u32) * 31);
            let x0 = x - wd * 0.5;
            let pts = [(x0, top), (x0 + wd, top), (x0 + wd * 1.25, base), (x0 - wd * 0.25, base)];
            polygon_fill_shaded(&mut self.trunks, &pts, 1.0, |u, v| {
                // u 0..1 across the trunk: lit side toward the fire
                let side = if facing > 0.0 { u } else { 1.0 - u };
                let bark = 0.85 + 0.15 * vnoise(v * 60.0, u * 4.0, seedt);
                let c = scale(col, bark);
                // encode the lit side as a brighter authored color; the
                // per-frame tint scales it by the fire's reach
                lerp(c, self.theme.trunk_lit, side * side * 0.55)
            });
            if self.theme.kind == Kind::Snow {
                // snow caps on the fire-facing branch stubs
                for k in 0..3 {
                    let y = base - (0.2 + 0.25 * k as f32) * (base - top) * 0.5;
                    stroke_f(&mut self.trunks, x - facing * wd * 0.4, y, x - facing * (wd * 0.5 + hero * 0.04), y - 1.0, 1.0, (200, 210, 230), 0.9);
                }
            }
        }

        // ground: needles / leaves / snow texture band
        self.ground = Plate::new(w, h);
        let gseed = seed ^ 0x55;
        for y in self.ground_y as i32..h as i32 {
            let v = (y as f32 - self.ground_y) / (st.hf - self.ground_y).max(1.0);
            for x in 0..w as i32 {
                let n = vnoise(x as f32 * 0.35, y as f32 * 0.5, gseed);
                let c = match self.theme.kind {
                    Kind::Snow => lerp(self.theme.ground, (200, 215, 240), (n - 0.5) * 0.3 + v * 0.2),
                    Kind::Autumn => {
                        let leaf = hash2(x, y, gseed) > 0.9;
                        if leaf {
                            lerp(self.theme.ground, (150, 70, 30), 0.6)
                        } else {
                            scale(self.theme.ground, 0.8 + 0.4 * n)
                        }
                    }
                    Kind::Pine => scale(self.theme.ground, 0.7 + 0.6 * n),
                };
                self.ground.blend_px(x, y, c, 1.0);
            }
        }
        // the horizon of the ground is softened by a dark band
        for x in 0..w as i32 {
            for k in 0..3 {
                self.ground.blend_px(x, self.ground_y as i32 + k, self.theme.canopy, 0.4 - k as f32 * 0.12);
            }
        }

        // foreground crop: out-of-focus log and rocks along the bottom edge
        self.fg = Plate::new(w, h);
        let fg_col = scale(self.theme.canopy, 0.9);
        let ly = st.hf - 1.0;
        let lr = hero * 0.045 * st.sy * 2.0 + 1.0;
        capsule(&mut self.fg, st.wf * 0.05, ly, lr, st.wf * 0.55, ly + lr * 0.4, lr * 0.85, fg_col, 1.0);
        ellipse_f(&mut self.fg, st.wf * 0.82, ly, hero * 0.09, hero * 0.05 * st.sy, 0.0, fg_col, 1.0);
        ellipse_f(&mut self.fg, st.wf * 0.93, ly + 1.0, hero * 0.06, hero * 0.035 * st.sy, 0.3, fg_col, 1.0);

        // fire buffer in fire cells: ~2 canvas px per cell
        let fire_w = hero * 0.22;
        let fire_h = hero * 0.34 * st.sy * 2.0;
        let cell = match self.detail {
            Detail::Low => 3.0,
            Detail::Medium => 2.0,
            Detail::High => 1.5,
        };
        self.fw = ((fire_w / cell) as usize).clamp(6, 60);
        self.fh = ((fire_h / cell) as usize).clamp(8, 90);
        self.heat = vec![0.0; self.fw * self.fh];
        self.fuel = (0..self.fw)
            .map(|x| {
                let u = (x as f32 + 0.5) / self.fw as f32;
                let bell = (1.0 - ((u - 0.5) * 2.4).powi(2)).max(0.0);
                bell * (0.75 + 0.25 * hash2(x as i32, 1, seed))
            })
            .collect();
        self.embers.clear();
        self.puffs.clear();
        self.motes.clear();
        let n_motes = self.detail.scale(st.pick(40.0, 55.0), 12);
        for i in 0..n_motes {
            self.motes.push(Mote {
                x: hash2(i as i32, 1, seed) * st.wf,
                y: hash2(i as i32, 2, seed) * st.hf,
                vx: 0.0,
                vy: 0.0,
                phase: hash2(i as i32, 3, seed) * 6.28,
                size: 0.6 + hash2(i as i32, 4, seed) * 0.8,
            });
        }
        self.facing = st.pick(1.0, 1.0);
    }

    fn step_fire(&mut self, dt: f32) {
        let (fw, fh) = (self.fw, self.fh);
        if fw == 0 || fh == 0 {
            return;
        }
        // seed the bottom row from the fuel mask
        let base = self.base_heat * (0.85 + 0.15 * self.flicker);
        for x in 0..fw {
            let jitter = 0.8 + 0.2 * self.rng.random::<f32>();
            self.heat[(fh - 1) * fw + x] = (self.fuel[x] * base * jitter).min(1.0);
        }
        // propagate upward with cooling and sideways drift; the cooling
        // rate keeps the flame height near half the buffer
        let cool = 1.0 / (fh as f32 * 0.55);
        let steps = ((dt * 45.0).round() as usize).clamp(1, 3);
        for _ in 0..steps {
            for y in 0..fh - 1 {
                for x in 0..fw {
                    let r = self.rng.random::<f32>();
                    let src_x = (x as i32 + (r * 3.0) as i32 - 1).clamp(0, fw as i32 - 1) as usize;
                    let below = self.heat[(y + 1) * fw + src_x];
                    let v = (below - cool * (0.5 + r)).max(0.0);
                    self.heat[y * fw + x] = v;
                }
            }
        }
    }

    fn heat_at(&self, u: f32, v: f32) -> f32 {
        // bilinear sample of the heat buffer, u/v in 0..1 (v = 0 top)
        let fx = (u * self.fw as f32 - 0.5).max(0.0);
        let fy = (v * self.fh as f32 - 0.5).max(0.0);
        let x0 = (fx.floor() as usize).min(self.fw - 1);
        let y0 = (fy.floor() as usize).min(self.fh - 1);
        let x1 = (x0 + 1).min(self.fw - 1);
        let y1 = (y0 + 1).min(self.fh - 1);
        let (tx, ty) = ((fx - x0 as f32).min(1.0), (fy - y0 as f32).min(1.0));
        let a = self.heat[y0 * self.fw + x0];
        let b = self.heat[y0 * self.fw + x1];
        let c = self.heat[y1 * self.fw + x0];
        let d = self.heat[y1 * self.fw + x1];
        a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty
    }

    fn spawn_embers(&mut self, n: usize, burst: f32) {
        let st = self.stage;
        let fire_w = self.hero * 0.22;
        for _ in 0..n {
            let u = self.rng.random_range(0.2..0.8);
            let x = self.fire_x + (u - 0.5) * fire_w;
            let y = self.fire_y - self.rng.random_range(0.0..self.hero * 0.08 * st.sy * 2.0);
            let up = self.hero * (0.12 + 0.25 * burst) * st.sy * 2.0;
            self.embers.push(Ember {
                x,
                y,
                vx: self.rng.random_range(-1.0..1.0) * self.hero * 0.04 * (1.0 + burst * 2.0),
                vy: -up * (0.6 + self.rng.random::<f32>() * 0.8),
                age: 0.0,
                life: self.rng.random_range(1.2..3.0) * (1.0 + burst),
                heat: 1.0,
            });
        }
    }

    fn next_cycle(&mut self) {
        let j = self.rng.random_range(-1.5..1.5);
        for b in ["reach", "poke", "burst", "back"] {
            self.tl.shift(b, j);
        }
        self.owl_side = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
        self.tl.shift("gust", self.rng.random_range(-2.0..2.0));
    }
}

impl Scene for Campfire {
    fn name(&self) -> &'static str {
        "campfire"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        if self.tl.step(dt).is_some() {
            self.next_cycle();
        }
        let st = self.stage;
        let t = self.t;
        let hero = self.hero;
        let sy = st.sy;
        let (fx, fy) = (self.fire_x, self.fire_y);

        // ---- fire state and light ----
        let burst = self.tl.env("burst", 0.05, 2.0);
        let settle = self.tl.env("settle", 0.05, 1.0);
        self.base_heat = 1.0 + 0.4 * burst + 0.15 * settle;
        let target_flicker = 0.82 + 0.36 * vnoise(t * 5.0, 0.3, self.seed) + 0.12 * vnoise(t * 17.0, 2.0, self.seed ^ 3);
        self.flicker = approach(self.flicker, target_flicker, 12.0, dt);
        self.flare = approach(self.flare, burst, 6.0, dt);
        let light = PointLight {
            x: fx,
            y: fy - hero * 0.04 * sy,
            r: hero * st.pick(0.36, 0.42),
            sy,
            color: self.theme.light,
            intensity: 1.35 * self.flicker * (1.0 + 0.9 * self.flare),
        };
        self.step_fire(dt);
        // steady ember trickle from the hottest columns, plus the burst
        let trickle = self.detail.scale(9.0, 3) as f32;
        if self.rng.random::<f32>() < dt * trickle {
            self.spawn_embers(1, 0.0);
        }
        if let Some(p) = self.tl.phase("burst") {
            if p < 0.4 {
                let n = self.detail.scale(14.0, 5);
                self.spawn_embers(n, 1.0);
            }
        }

        // ---- sky, stars, moon, canopy ----
        let gy = self.ground_y as i32;
        gradient_v(canvas, 0, gy, &self.theme.sky);
        self.stars.draw(canvas, gy, t, 0.85);
        let mx = st.wf * st.pick(0.8, 0.72);
        let my = st.hf * st.pick(0.09, 0.06);
        moon(canvas, mx, my, hero * 0.025, hero * 0.025 * sy, (205, 210, 225), 0.4, self.seed);
        let canopy_lit = self.theme.trunk_lit;
        self.canopy.composite(canvas, 0, 0, |px, py, c| {
            let f = light.at(px as f32, py as f32) * 0.35;
            lerp(c, canopy_lit, (f / (1.0 + f)).min(0.6))
        });

        // ---- trunks lit on the fire side ----
        let trunk_col = self.theme.trunk;
        let ambient = 0.18;
        self.trunks.composite(canvas, 0, 0, |px, py, c| {
            let f = light.at(px as f32, py as f32);
            // authored plate holds base bark (dark) → trunk_lit blend on the
            // facing side; scale that blend by the light reaching it
            let lit_amt = (f / (1.0 + f)).clamp(0.0, 1.0);
            let base = lerp(c, trunk_col, 0.5);
            let bright = (ambient + f).min(1.6);
            let cc = lerp(scale(base, bright.min(1.0)), c, lit_amt);
            scale(cc, (ambient + f * 1.2).clamp(0.2, 1.4))
        });

        // ---- ground pool ----
        let ground_lit = self.theme.ground_lit;
        self.ground.composite(canvas, 0, 0, |px, py, c| {
            let f = light.at(px as f32, py as f32) * 0.9;
            let lit = lerp(c, ground_lit, (f / (1.0 + f)).min(0.85));
            scale(lit, (0.22 + f).clamp(0.15, 1.3))
        });
        // shadow of the figure and the logs, cast away from the fire
        let fig_x = fx - self.facing * hero * st.pick(0.26, 0.24);
        let fig_y = fy + hero * 0.02 * sy;
        soft_shadow(canvas, fig_x - self.facing * hero * 0.08, fig_y + hero * 0.02 * sy, hero * 0.16, hero * 0.05 * sy, 0.55 * (0.7 + 0.3 * self.flicker));

        // ---- tent on the far side ----
        if !st.tiny() {
            let tx = fx + self.facing * hero * st.pick(0.34, 0.3);
            let ty = self.ground_y + (st.hf - self.ground_y) * 0.12;
            let tw = hero * 0.2;
            let th = hero * 0.17 * sy * 2.0;
            let lantern = self.tl.env("lantern", 2.0, 0.0) * (0.75 + 0.25 * (t * 1.3).sin());
            let tent = self.theme.tent;
            let apex = (tx, ty - th);
            // near face (toward the fire) and far face
            let near = [apex, (tx - self.facing * tw * 0.55, ty), (tx + self.facing * tw * 0.1, ty + 1.0)];
            let far = [apex, (tx + self.facing * tw * 0.1, ty + 1.0), (tx + self.facing * tw * 0.5, ty - th * 0.08)];
            let lit_col = lerp(tent, self.theme.light, 0.35);
            polygon_fill_shaded(canvas, &near, 1.0, |u, v| {
                let px = tx - self.facing * tw * 0.3;
                let f = light.at(px, ty - th * (1.0 - v) * 0.5) * 0.9;
                let c = lerp(tent, lit_col, (f / (1.0 + f)).min(0.8));
                let c = scale(c, (0.22 + f).clamp(0.15, 1.2));
                let _ = u;
                lerp(c, (255, 220, 150), lantern * 0.35 * (1.0 - v * 0.5))
            });
            polygon_fill_shaded(canvas, &far, 1.0, |_, v| {
                let c = scale(tent, 0.28 + 0.1 * (1.0 - v));
                lerp(c, (255, 220, 150), lantern * 0.25)
            });
            // door glow when the lantern is on
            if lantern > 0.02 {
                let dx = tx - self.facing * tw * 0.22;
                capsule(canvas, dx, ty - 1.0, tw * 0.05, dx, ty - th * 0.35, tw * 0.04, (255, 215, 140), lantern * 0.7);
            }
        }

        // ---- logs: three capsules with glowing ends ----
        let log_r = (hero * 0.028 * sy * 2.0).max(0.8);
        let log_len = hero * 0.14;
        let log_col = scale(self.theme.trunk, 0.8);
        self.log_tilt = approach(self.log_tilt, if self.tl.since("settle") > 0.0 { 1.0 } else { 0.0 }, 8.0, dt);
        let heat_col = ramp(&self.theme.fire, 0.55);
        for (i, ang) in [0.18f32, -0.22, 0.05].iter().enumerate() {
            let a = ang + if i == 1 { self.log_tilt * 0.15 } else { 0.0 };
            let (cx, cy) = (fx + (i as f32 - 1.0) * log_len * 0.25, fy + log_r * (0.6 + i as f32 * 0.5));
            let (dx, dy) = (a.cos() * log_len * 0.5, a.sin() * log_len * 0.5 * sy);
            capsule_shaded(canvas, cx - dx, cy - dy, log_r, cx + dx, cy + dy, log_r * 0.9, 1.0, |u, v| {
                let end_glow = (1.0 - (u - 0.5).abs() * 2.0).powi(3) * 0.5;
                let top = (1.0 - v).max(0.0) * 0.5;
                let c = lerp(log_col, heat_col, (end_glow * (0.6 + 0.4 * self.flicker) + top * 0.2).min(1.0));
                scale(c, 0.6 + 0.5 * self.flicker)
            });
        }

        // ---- figure: seated in profile, facing the fire ----
        if !st.tiny() {
            let fig = self.theme.figure;
            let rim_col = self.theme.light;
            let size = hero * 0.26;
            let fac = self.facing;
            let reach = self.tl.env("reach", 0.7, 0.0) + self.tl.env("poke", 0.0, 0.0);
            let back = self.tl.env("back", 0.6, 0.0);
            let lean_target = (reach.min(1.0) * 0.5 - back * 0.25) * fac;
            let lean = self.lean.step(lean_target, dt);
            let hood = self.hood.step(lean, dt);
            self.breath_t += dt;
            let breath = (self.breath_t * 1.5).sin() * 0.5 + 0.5;
            let head_t = if self.tl.active("owl") { -0.5 * fac } else { 0.0 } + if (t * 0.23).sin() > 0.93 { 0.2 } else { 0.0 };
            let head = self.head_turn.step(head_t, dt);
            let hips = (fig_x, fig_y);
            let torso_len = size * 0.42 * sy * 2.0;
            let shoulder = (hips.0 + lean * torso_len * 0.6, hips.1 - torso_len * (1.0 - lean.abs() * 0.25) - breath * 0.6 * sy);
            let r_body = size * 0.09;
            let shade = |c: Rgb, lit_side: f32| move |_u: f32, v: f32| {
                let f = (v * lit_side * fac).max(0.0);
                lerp(c, rim_col, f * f * 0.35 * (0.7 + 0.3 * lit_side))
            };
            // thighs toward the fire, shins down (sitting on the ground)
            let knee = (hips.0 + fac * size * 0.28, hips.1 - size * 0.06 * sy);
            capsule_shaded(canvas, hips.0, hips.1, r_body * 0.9, knee.0, knee.1, r_body * 0.8, 1.0, shade(fig, 1.0));
            capsule_shaded(canvas, knee.0, knee.1, r_body * 0.75, knee.0 + fac * size * 0.05, hips.1 + size * 0.12 * sy, r_body * 0.6, 1.0, shade(fig, 1.0));
            // torso
            capsule_shaded(canvas, hips.0, hips.1, r_body, shoulder.0, shoulder.1, r_body * 0.9, 1.0, shade(fig, 1.0));
            // hood / head
            let head_r = size * 0.11;
            let head_c = (shoulder.0 + fac * head_r * (0.5 + head * 0.4) + hood * head_r * 0.3, shoulder.1 - head_r * 1.3 * sy);
            ellipse_f(canvas, head_c.0, head_c.1, head_r, head_r * sy * 1.1, 0.0, fig, 1.0);
            // face rim toward the fire
            let rim_a = (0.3 + 0.2 * self.flicker) * (1.0 - head.abs() * 0.8).max(0.0);
            capsule(canvas, head_c.0 + fac * head_r * 0.55, head_c.1 - head_r * 0.5 * sy, 0.5, head_c.0 + fac * head_r * 0.7, head_c.1 + head_r * 0.4 * sy, 0.6, rim_col, rim_a);
            // arm: two bones from the shoulder, IK to the stick grip
            let stick_ext = self.stick.step(reach.min(1.0), dt);
            let grip_rest = (shoulder.0 + fac * size * 0.22, shoulder.1 + size * 0.2 * sy);
            let grip_reach = (shoulder.0 + fac * size * 0.42, shoulder.1 + size * 0.12 * sy);
            let grip = (grip_rest.0 + (grip_reach.0 - grip_rest.0) * stick_ext, grip_rest.1 + (grip_reach.1 - grip_rest.1) * stick_ext);
            let l1 = size * 0.22;
            let l2 = size * 0.22;
            // solve in screen units, then squash y
            let sh_s = (shoulder.0, shoulder.1 / sy);
            let gr_s = (grip.0, grip.1 / sy);
            let (elbow_s, _, _) = ik2(sh_s, gr_s, l1, l2, -fac);
            let elbow = (elbow_s.0, elbow_s.1 * sy);
            capsule_shaded(canvas, shoulder.0, shoulder.1, r_body * 0.7, elbow.0, elbow.1, r_body * 0.6, 1.0, shade(fig, 1.0));
            capsule_shaded(canvas, elbow.0, elbow.1, r_body * 0.6, grip.0, grip.1, r_body * 0.5, 1.0, shade(fig, 1.0));
            // the stick: from the grip toward the fire base, tip glowing when in the coals
            let tip_rest = (fx - fac * hero * 0.06, fy - hero * 0.06 * sy);
            let tip_in = (fx - fac * hero * 0.01, fy + hero * 0.02 * sy);
            let poke = self.tl.env("poke", 0.25, 0.4);
            let tip = (tip_rest.0 + (tip_in.0 - tip_rest.0) * poke, tip_rest.1 + (tip_in.1 - tip_rest.1) * poke);
            let stick_col = scale(self.theme.trunk, 1.4);
            stroke_f(canvas, grip.0, grip.1, tip.0, tip.1, 1.1, stick_col, 0.95);
            if poke > 0.1 {
                blend_f(canvas, tip.0, tip.1, ramp(&self.theme.fire, 0.8), poke * 0.9);
            }
        }

        // ---- flame ----
        let fire_w = hero * 0.22;
        let fire_h = hero * 0.34 * sy * 2.0;
        let (x0, y0) = (fx - fire_w * 0.5, fy + hero * 0.02 * sy - fire_h);
        let core = ramp(&self.theme.fire, 1.0);
        for py in y0.floor() as i32..(y0 + fire_h).ceil() as i32 {
            let v = (py as f32 + 0.5 - y0) / fire_h;
            for px in x0.floor() as i32..(x0 + fire_w).ceil() as i32 {
                let u = (px as f32 + 0.5 - x0) / fire_w;
                if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                    continue;
                }
                let hval = self.heat_at(u, v);
                if hval < 0.06 {
                    continue;
                }
                let c = ramp(&self.theme.fire, hval);
                let a = ((hval - 0.06) * 4.0).clamp(0.0, 1.0);
                if hval > 0.35 {
                    canvas.blend(px, py, c, a);
                } else {
                    canvas.add_scaled(px, py, c, a * 0.9);
                }
            }
        }
        let _ = core;

        // ---- embers ----
        let wind = (vnoise(t * 0.3, 5.0, self.seed ^ 0x21) - 0.5) * hero * 0.05 + self.tl.env("gust", 0.5, 1.0) * hero * 0.08;
        for e in self.embers.iter_mut() {
            e.age += dt;
            let turb = (vnoise(e.y * 0.08, t * 2.5 + e.x * 0.05, self.seed ^ 0x31) - 0.5) * hero * 0.25;
            e.vx += (turb + wind - e.vx * 0.8) * dt;
            e.vy += (-hero * 0.05 * sy * 2.0 - e.vy * 0.4) * dt;
            e.x += e.vx * dt;
            e.y += e.vy * dt;
            e.heat = (1.0 - e.age / e.life).max(0.0);
            let c = ramp(&self.theme.fire, 0.45 + 0.55 * e.heat);
            add_f(canvas, e.x, e.y, c, e.heat * 0.9);
        }
        self.embers.retain(|e| e.age < e.life && e.y > -4.0);

        // ---- smoke column ----
        self.puff_acc += dt * (5.0 + 8.0 * self.flare);
        while self.puff_acc >= 1.0 {
            self.puff_acc -= 1.0;
            self.puffs.push(Puff {
                x: fx + self.rng.random_range(-0.15..0.15) * fire_w,
                y: y0 + fire_h * 0.25,
                r: hero * 0.02,
                age: 0.0,
                life: self.rng.random_range(3.5..5.5),
            });
        }
        let smoke = self.theme.smoke;
        let smoke_lit = lerp(smoke, self.theme.light, 0.5);
        for p in self.puffs.iter_mut() {
            p.age += dt;
            let k = p.age / p.life;
            p.y -= hero * 0.06 * sy * 2.0 * dt * (1.2 - k * 0.6);
            p.x += (vnoise(p.y * 0.05, t * 0.4, self.seed ^ 0x41) - 0.5) * hero * 0.12 * dt + wind * dt;
            p.r = hero * (0.02 + 0.09 * k);
            let a = 0.28 * (1.0 - k) * (0.4 + 0.6 * (k * 4.0).min(1.0));
            let f = light.at(p.x, p.y) * 0.6;
            let c = lerp(smoke, smoke_lit, (f / (1.0 + f)).min(0.8));
            ellipse_f(canvas, p.x, p.y, p.r, p.r * sy, 0.0, c, a);
        }
        self.puffs.retain(|p| p.age < p.life);

        // ---- theme particles: fireflies / leaves / snow ----
        let gust = self.tl.env("gust", 0.5, 1.0);
        for m in self.motes.iter_mut() {
            m.phase += dt;
            match self.theme.kind {
                Kind::Pine => {
                    // fireflies drift lazily and blink
                    m.vx = (vnoise(m.phase * 0.3, m.y * 0.02, self.seed ^ 7) - 0.5) * hero * 0.06;
                    m.vy = (vnoise(m.x * 0.02, m.phase * 0.3 + 5.0, self.seed ^ 8) - 0.5) * hero * 0.04 * sy;
                    m.x += (m.vx + gust * hero * 0.05) * dt;
                    m.y += m.vy * dt;
                    let blink = ((m.phase * 1.7).sin() * 0.5 + 0.5).powi(6);
                    if m.y > self.ground_y * 0.4 {
                        add_f(canvas, m.x, m.y, (190, 225, 120), blink * 0.9);
                    }
                }
                Kind::Autumn => {
                    m.vy = hero * 0.04 * sy * 2.0 * (0.6 + 0.4 * m.size);
                    m.vx = (m.phase * 2.0).sin() * hero * 0.06 + gust * hero * 0.12;
                    m.x += m.vx * dt;
                    m.y += m.vy * dt;
                    let rot = m.phase * 3.0;
                    let leaf = lerp((170, 80, 30), (210, 130, 40), (m.phase * 0.5).sin() * 0.5 + 0.5);
                    let f = light.at(m.x, m.y);
                    let c = scale(leaf, (0.25 + f).min(1.2));
                    ellipse_f(canvas, m.x, m.y, m.size * 1.3, m.size * 0.7 * sy, rot, c, 0.9);
                }
                Kind::Snow => {
                    m.vy = hero * 0.05 * sy * 2.0 * (0.5 + 0.5 * m.size);
                    m.vx = (vnoise(m.phase, m.y * 0.03, self.seed ^ 9) - 0.5) * hero * 0.08 + gust * hero * 0.2;
                    m.x += m.vx * dt;
                    m.y += m.vy * dt;
                    let f = light.at(m.x, m.y);
                    let c = lerp((200, 210, 230), self.theme.light, (f / (1.0 + f)).min(0.6));
                    blend_f(canvas, m.x, m.y, c, (0.5 + 0.5 * m.size) * (0.35 + f.min(1.0)));
                }
            }
            if m.y > st.hf + 2.0 {
                m.y = -2.0;
                m.x = self.rng.random_range(0.0..st.wf);
            }
            if m.x < -3.0 {
                m.x += st.wf + 6.0;
            }
            if m.x > st.wf + 3.0 {
                m.x -= st.wf + 6.0;
            }
            if m.y < -3.0 {
                m.y = st.hf * 0.5;
            }
        }
        // breath puffs on the snow theme
        if self.theme.kind == Kind::Snow && !st.tiny() {
            let bp = (t * 0.25).fract();
            if bp < 0.4 {
                let k = bp / 0.4;
                let bx = fig_x + self.facing * hero * 0.12 + k * hero * 0.06 * self.facing;
                let by = fig_y - hero * 0.3 * sy - k * hero * 0.03 * sy;
                ellipse_f(canvas, bx, by, hero * 0.012 + k * hero * 0.03, (hero * 0.01 + k * hero * 0.02) * sy, 0.0, (200, 205, 220), 0.25 * (1.0 - k));
            }
        }

        // ---- owl eyes in a far trunk gap ----
        if let Some(p) = self.tl.phase("owl") {
            let blink = ((p * 9.0).sin() > -0.7) as i32 as f32;
            let ox = fx + self.owl_side * hero * 0.55;
            let oy = self.ground_y * 0.55;
            if ox > 2.0 && ox < st.wf - 2.0 {
                let a = (p * 4.0).min(1.0) * ((1.0 - p) * 4.0).min(1.0) * blink;
                blend_f(canvas, ox - 1.2, oy, (255, 200, 60), a);
                blend_f(canvas, ox + 1.2, oy, (255, 200, 60), a);
            }
        }

        // ---- foreground crop, barely lit ----
        self.fg.composite(canvas, 0, 0, |px, py, c| {
            let f = light.at(px as f32, py as f32) * 0.5;
            scale(lerp(c, ground_lit, (f / (1.0 + f)).min(0.5)), (0.3 + f).min(1.0))
        });
        let _ = polygon_fill::<Canvas>;
        let _ = &mut self.lut;
    }
}
