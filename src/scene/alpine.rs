//! Alpine lake at dusk: parallax ridges under a sinking sun, a mirrored
//! lake, a lone canoe.
//!
//! Composed like the classic "mountain at dusk" parallax paintings: a
//! peach-to-violet sky, a sun disc that squashes as it meets the far ridge,
//! five ridge planes stepping from pale lavender to near-black teal with
//! aerial perspective, a pine line on the nearest ridge, and the whole thing
//! mirrored in still water broken by wind lanes. A 40 s day cycle carries
//! the light: every plate is tinted by the same sky, so sunset touches the
//! ridges, the mist, the lake and the canoe alike.
//!
//! Static planes (ridges, bank) are painted once into plates and composited
//! each frame; only the sky ramp, the mist grid and the lake mirror are
//! per-pixel work.
use super::{noise::{fbm, hash2, vnoise}, Detail, Scene};
use crate::anim::{
    beat::Timeline,
    draw::{blend_f, capsule, ellipse_f, gradient_v, radial_light, stroke_f, Plate},
    light::{fog, mix},
    rig::{ease, Ease, Follow, Key, Track},
    scenery::{pine, ridge_fill, ridge_profile, NoiseGrid, StarField},
    stage::{Camera, ParallaxLayer, Stage},
    Rgb,
};
use crate::canvas::{lerp, scale, Canvas};
use crate::render::Pixels;
use rand::{rngs::StdRng, RngExt};

const LOOP: f32 = 40.0;

struct Theme {
    night: [Rgb; 4],
    dusk: [Rgb; 4],
    day: [Rgb; 4],
    ridge_far: Rgb,
    ridge_mid: Rgb,
    ridge_near: Rgb,
    pine: Rgb,
    lake: Rgb,
    sun: Rgb,
    halo: Rgb,
    cabin: Rgb,
    storm: bool,
    /// mood 0 = night, 1 = dusk, 2 = day
    mood: Track,
    /// sun altitude, fraction of the sky band (negative = set)
    alt: Track,
    /// sun x as a fraction of the width
    sunx: Track,
    mist: f32,
}

const SKY_POS: [f32; 4] = [0.0, 0.45, 0.78, 1.0];

fn theme(name: Option<&str>) -> Theme {
    let k = |t: f32, v: f32, e: Ease| Key::new(t, v, e);
    match name {
        Some("dawn") => Theme {
            night: [(4, 6, 20), (10, 14, 38), (24, 22, 52), (40, 30, 60)],
            dusk: [(70, 50, 110), (190, 110, 130), (250, 170, 130), (255, 215, 160)],
            day: [(110, 160, 220), (150, 190, 235), (205, 220, 235), (235, 230, 220)],
            ridge_far: (200, 175, 200),
            ridge_mid: (120, 120, 160),
            ridge_near: (22, 32, 40),
            pine: (10, 16, 18),
            lake: (30, 40, 70),
            sun: (255, 235, 200),
            halo: (255, 180, 110),
            cabin: (255, 190, 110),
            storm: false,
            mood: Track::new(vec![
                k(0.0, 0.0, Ease::Linear),
                k(4.0, 0.0, Ease::Linear),
                k(8.5, 1.0, Ease::InOut),
                k(14.0, 2.0, Ease::InOut),
                k(30.0, 2.0, Ease::Linear),
                k(34.5, 1.0, Ease::InOut),
                k(37.0, 0.0, Ease::InOut),
                k(40.0, 0.0, Ease::Linear),
            ]),
            alt: Track::new(vec![
                k(0.0, 0.25, Ease::Linear),
                k(5.0, 0.25, Ease::Linear),
                k(13.0, 0.85, Ease::Out),
                k(30.0, 0.85, Ease::Linear),
                k(36.0, 0.25, Ease::InOut),
                k(40.0, 0.25, Ease::Linear),
            ]),
            sunx: Track::new(vec![k(0.0, 0.22, Ease::Linear), k(5.0, 0.22, Ease::Linear), k(36.0, 0.8, Ease::Linear), k(40.0, 0.22, Ease::Hold)]),
            mist: 0.55,
        },
        Some("storm") => Theme {
            night: [(18, 20, 30), (30, 34, 46), (44, 48, 60), (58, 62, 74)],
            dusk: [(40, 46, 58), (62, 70, 84), (84, 92, 106), (110, 116, 128)],
            day: [(70, 78, 92), (96, 104, 118), (120, 126, 138), (150, 152, 158)],
            ridge_far: (120, 128, 142),
            ridge_mid: (70, 78, 92),
            ridge_near: (20, 26, 32),
            pine: (10, 14, 18),
            lake: (38, 46, 58),
            sun: (0, 0, 0),
            halo: (0, 0, 0),
            cabin: (255, 200, 120),
            storm: true,
            // clouds breathe between dusk-grey and day-grey
            mood: Track::new(vec![
                k(0.0, 1.0, Ease::Linear),
                k(9.0, 1.6, Ease::InOut),
                k(18.0, 0.8, Ease::InOut),
                k(29.0, 1.4, Ease::InOut),
                k(40.0, 1.0, Ease::InOut),
            ]),
            alt: Track::new(vec![k(0.0, -1.0, Ease::Linear)]),
            sunx: Track::new(vec![k(0.0, 0.5, Ease::Linear)]),
            mist: 0.7,
        },
        _ => Theme {
            night: [(6, 8, 24), (12, 16, 44), (26, 24, 60), (44, 34, 70)],
            dusk: [(40, 35, 90), (120, 70, 140), (225, 110, 120), (255, 180, 120)],
            day: [(120, 170, 225), (160, 195, 235), (215, 220, 225), (235, 215, 185)],
            ridge_far: (196, 170, 210),
            ridge_mid: (95, 105, 150),
            ridge_near: (14, 24, 28),
            pine: (8, 14, 16),
            lake: (20, 30, 50),
            sun: (255, 236, 200),
            halo: (255, 150, 90),
            cabin: (255, 190, 110),
            storm: false,
            mood: Track::new(vec![
                k(0.0, 2.0, Ease::Linear),
                k(10.0, 2.0, Ease::Linear),
                k(19.0, 1.0, Ease::InOut),
                k(26.0, 0.0, Ease::InOut),
                k(35.0, 0.0, Ease::Linear),
                k(37.5, 1.0, Ease::InOut),
                k(40.0, 2.0, Ease::InOut),
            ]),
            alt: Track::new(vec![
                k(0.0, 0.85, Ease::Linear),
                k(10.0, 0.85, Ease::Linear),
                k(24.0, 0.28, Ease::InOut),
                k(35.0, 0.28, Ease::Linear),
                k(40.0, 0.85, Ease::Out),
            ]),
            sunx: Track::new(vec![k(0.0, 0.35, Ease::Linear), k(24.0, 0.85, Ease::Linear), k(35.0, 0.15, Ease::Hold), k(40.0, 0.35, Ease::Linear)]),
            mist: 0.35,
        },
    }
}

struct Ridge {
    layer: ParallaxLayer,
    color: Rgb,
    /// 0 far .. 1 near
    depth: f32,
    profile: Vec<f32>,
    base: f32,
}

struct Canoe {
    x: f32,
    dir: f32,
    stroke_t: f32,
    lean: Follow,
    wake: Vec<(f32, f32, f32)>,
    active: bool,
}

struct Flock {
    x: f32,
    y: f32,
    vx: f32,
    n: usize,
    phase: f32,
    alive: bool,
}

pub struct Alpine {
    rng: StdRng,
    detail: Detail,
    pixels: Pixels,
    theme: Theme,
    seed: u32,
    w: usize,
    h: usize,
    stage: Stage,
    horizon: f32,
    sky_top: f32,
    tl: Timeline,
    cam: Camera,
    t: f32,
    ridges: Vec<Ridge>,
    bank: Plate,
    stars: StarField,
    mist: NoiseGrid,
    canoe: Canoe,
    flocks: Vec<Flock>,
    /// (start time, x0, y0) of this cycle's meteor
    meteor: Option<(f32, f32, f32)>,
    /// storm lightning: flash envelope and time of the next bolt
    flash: f32,
    next_bolt: f32,
    cabin_x: f32,
    scratch: Vec<f32>,
}

impl Alpine {
    pub fn new(mut rng: StdRng, theme_name: Option<&str>, detail: Detail, pixels: Pixels) -> Self {
        let seed: u32 = rng.random();
        let theme = theme(theme_name);
        let next_bolt = rng.random_range(3.0..9.0);
        let mut tl = Timeline::new(LOOP)
            .beat("canoe", 27.0, 13.0)
            .beat("meteor", 33.0, 0.7)
            .beat("cabin", 26.0, LOOP - 26.0);
        if theme_name == Some("dawn") {
            tl.set_at("cabin", 0.0);
            tl.set_at("canoe", 9.0);
            tl.set_at("meteor", 1.5);
        }
        Alpine {
            rng,
            detail,
            pixels,
            theme,
            seed,
            w: 0,
            h: 0,
            stage: Stage::new(1, 1, pixels),
            horizon: 0.0,
            sky_top: 0.0,
            tl,
            cam: Camera::new((3.0, 1.2), (45.0, 71.0), seed as f32 * 0.001),
            t: 0.0,
            ridges: Vec::new(),
            bank: Plate::new(1, 1),
            stars: StarField::new(seed ^ 0x5a5a, 5.0, (235, 235, 250)),
            mist: NoiseGrid::new(6),
            canoe: Canoe { x: 0.0, dir: 1.0, stroke_t: 0.0, lean: Follow::new(0.0, 30.0, 5.0), wake: Vec::new(), active: false },
            flocks: Vec::new(),
            meteor: None,
            flash: 0.0,
            next_bolt,
            cabin_x: 0.0,
            scratch: Vec::new(),
        }
    }

    fn ridge_count(&self) -> usize {
        match self.detail {
            Detail::Low => 4,
            Detail::Medium => 5,
            Detail::High => 6,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.stage = Stage::new(w, h, self.pixels);
        let st = self.stage;
        self.horizon = st.horizon(0.6, 0.5).round();
        self.sky_top = 0.0;
        let hero = st.hero(1.0);
        let n = self.ridge_count();
        let pad = 8;
        self.ridges.clear();
        // ridge bases climb from the horizon into the lower half of the sky,
        // leaving the upper sky open for the sun's arc; portrait stacks them
        // taller so the mirror gets a full mountain
        let band = self.horizon * st.pick(0.5, 0.55);
        for i in 0..n {
            let f = i as f32 / (n - 1).max(1) as f32; // 0 far .. 1 near
            let base = self.horizon - band * (1.0 - f) * 0.8 + 1.0;
            let amp = (self.horizon * st.pick(0.26, 0.24) * (1.0 - 0.55 * f)).max(3.0);
            let mut profile = Vec::new();
            let freq = 0.012 * st.pick(1.0, 1.4) * (1.0 + f * 0.6);
            ridge_profile(&mut profile, w + pad as usize * 2, self.seed.wrapping_add(i as u32 * 131), amp, freq, 4);
            let color = if f < 0.5 {
                lerp(self.theme.ridge_far, self.theme.ridge_mid, f * 2.0)
            } else {
                lerp(self.theme.ridge_mid, self.theme.ridge_near, (f - 0.5) * 2.0)
            };
            let mut plate = Plate::new(w + pad as usize * 2, h);
            let snow = f < 0.34 && !self.theme.storm;
            let seed = self.seed ^ (i as u32 * 977);
            let peak = profile.iter().cloned().fold(0.0f32, f32::max).max(1.0);
            let prof = profile.clone();
            ridge_fill(&mut plate, &profile, base, self.horizon + 1.0, 1.0, |u, v| {
                // faint vertical banding reads as brush strokes; far ridges
                // carry snow on their highest crests only
                let band = 0.92 + 0.08 * vnoise(u * 40.0, v * 6.0, seed);
                let c = scale(color, band);
                let xi = ((u * prof.len() as f32) as usize).min(prof.len() - 1);
                let rel = prof[xi] / peak;
                if snow && rel > 0.62 && v < 0.12 {
                    let cap = ((rel - 0.62) / 0.38).clamp(0.0, 1.0) * (1.0 - v / 0.12);
                    lerp(c, (240, 240, 250), cap * 0.7)
                } else {
                    c
                }
            });
            if f >= 0.6 {
                // pines along the crest of the near ridges
                let count = self.detail.scale(w as f32 / 6.0, 6);
                for j in 0..count {
                    let hx = hash2(j as i32, i as i32, self.seed) * (w + pad as usize * 2) as f32;
                    let xi = (hx as usize).min(profile.len() - 1);
                    let top = base - profile[xi];
                    let ph = (hero * 0.018 + hash2(j as i32, 9, seed) * hero * 0.03) * st.sy * 2.0;
                    let pw = ph * 0.55 + 1.0;
                    pine(&mut plate, hx, top + 1.5, ph.max(2.0), pw, self.theme.pine, 0.95);
                }
            }
            self.ridges.push(Ridge {
                layer: ParallaxLayer::new(0.15 + 0.85 * f, plate, pad),
                color,
                depth: 1.0 - f,
                profile,
                base,
            });
        }
        // near bank: a dark shore in one corner with pines and a boulder
        self.bank = Plate::new(w, h);
        let bank_h = st.pick(st.hf * 0.16, st.hf * 0.11);
        let bank_w = st.pick(st.wf * 0.36, st.wf * 0.62);
        let by = st.hf;
        let mut pts = vec![(0.0, by), (0.0, by - bank_h * 0.55)];
        let segs = 12;
        for k in 0..=segs {
            let u = k as f32 / segs as f32;
            let x = u * bank_w;
            let yy = by - bank_h * (0.55 + 0.45 * (1.0 - u) * (1.0 - u)) + fbm(u * 6.0, 1.0, 2, self.seed ^ 0x33) * bank_h * 0.2;
            pts.push((x, yy));
        }
        pts.push((bank_w, by));
        crate::anim::draw::polygon_fill(&mut self.bank, &pts, self.theme.ridge_near, 1.0);
        let boulder_r = bank_h * 0.28;
        ellipse_f(&mut self.bank, bank_w * 0.7, by - bank_h * 0.5, boulder_r * 1.4, boulder_r * st.sy, 0.2, scale(self.theme.ridge_near, 0.8), 1.0);
        let pines = st.pick(3, 4);
        for j in 0..pines {
            let u = 0.1 + 0.8 * j as f32 / pines as f32;
            let x = u * bank_w * 0.9;
            let ph = bank_h * (1.4 + 0.6 * hash2(j, 1, self.seed)) * st.pick(1.0, 1.2);
            pine(&mut self.bank, x, by - bank_h * (0.6 + 0.3 * (1.0 - u)), ph, ph * 0.5, self.theme.pine, 1.0);
        }
        // grass strokes along the shore
        for j in 0..(bank_w / 3.0) as i32 {
            let x = j as f32 * 3.0 + hash2(j, 4, self.seed) * 3.0;
            let u = x / bank_w;
            let gy = by - bank_h * (0.55 + 0.45 * (1.0 - u) * (1.0 - u));
            let gh = (0.8 + hash2(j, 6, self.seed) * 1.6) * st.sy * 2.0;
            let lean = (hash2(j, 8, self.seed) - 0.5) * 1.2;
            stroke_f(&mut self.bank, x, gy + 0.5, x + lean, gy - gh, 0.6, self.theme.pine, 0.8);
        }
        self.cabin_x = st.wf * st.pick(0.78, 0.7);
        self.canoe.active = false;
        self.flocks.clear();
        self.meteor = None;
        self.mist = NoiseGrid::new(match self.detail {
            Detail::Low => 8,
            Detail::Medium => 6,
            Detail::High => 4,
        });
    }

    /// (night, dusk, day) weights from the theme's mood track.
    fn mood(&self) -> (f32, f32, f32) {
        let m = self.theme.mood.at(self.tl.t).clamp(0.0, 2.0);
        let wn = (1.0 - m).clamp(0.0, 1.0);
        let wd = (m - 1.0).clamp(0.0, 1.0);
        (wn, 1.0 - wn - wd, wd)
    }

    fn sky_stop(&self, i: usize, wn: f32, wk: f32, wd: f32) -> Rgb {
        let n = self.theme.night[i];
        let k = self.theme.dusk[i];
        let d = self.theme.day[i];
        let c = lerp(n, k, if wn + wk > 0.0 { wk / (wn + wk) } else { 0.0 });
        lerp(c, d, wd)
    }

    fn spawn_flock(&mut self) {
        let st = self.stage;
        let dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
        let n = self.rng.random_range(3..8usize);
        let y = self.rng.random_range(self.horizon * 0.15..self.horizon * 0.55);
        self.flocks.push(Flock {
            x: if dir > 0.0 { -10.0 } else { st.wf + 10.0 },
            y,
            vx: dir * self.rng.random_range(4.0..7.0) * st.pick(1.0, 0.8),
            n,
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            alive: true,
        });
    }

    fn next_cycle(&mut self) {
        // per-cycle variation: meteor timing, canoe direction, bird schedule
        let mt = self.rng.random_range(0.0..5.0);
        self.tl.set_at("meteor", self.tl.at("meteor") - 2.5 + mt);
        self.canoe.dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
    }
}

impl Scene for Alpine {
    fn name(&self) -> &'static str {
        "alpine"
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
        self.cam.step(self.t);
        let st = self.stage;
        let t = self.t;
        let (wn, wk, wd) = self.mood();
        let storm = self.theme.storm;

        // ---- storm lightning schedule ----
        if storm {
            self.flash = (self.flash - dt * 2.5).max(0.0);
            if self.tl.t >= self.next_bolt && self.tl.t < self.next_bolt + dt * 1.5 {
                self.flash = 1.0;
                // restrike half the time, otherwise wait 8-20 s
                let gap = if self.rng.random::<f32>() < 0.5 { self.rng.random_range(0.15..0.4) } else { self.rng.random_range(8.0..20.0) };
                self.next_bolt = (self.tl.t + gap) % LOOP;
            }
        }
        let flash = self.flash * self.flash;

        // ---- sky ----
        let stops = [
            (SKY_POS[0], self.sky_stop(0, wn, wk, wd)),
            (SKY_POS[1], self.sky_stop(1, wn, wk, wd)),
            (SKY_POS[2], self.sky_stop(2, wn, wk, wd)),
            (SKY_POS[3], self.sky_stop(3, wn, wk, wd)),
        ];
        let stops_lit: Vec<(f32, Rgb)> = if flash > 0.0 {
            stops.iter().map(|&(p, c)| (p, lerp(c, (215, 220, 240), flash * 0.7))).collect()
        } else {
            stops.to_vec()
        };
        let horizon_i = self.horizon as i32;
        gradient_v(canvas, 0, horizon_i, &stops_lit);
        let horizon_color = stops_lit[3].1;
        // light level that every plane shares
        let lum = (0.24 + 0.76 * wd + 0.4 * wk + flash * 0.6).min(1.0);

        // ---- stars ----
        let star_a = if storm { 0.0 } else { (wn * 0.95 - wk * 0.3).clamp(0.0, 1.0) };
        self.stars.draw(canvas, horizon_i, t, star_a);

        // ---- moon: rises as the sun sets, opposite side of the sky ----
        let moon_a = if storm { 0.0 } else { (wn * 1.2 - 0.1).clamp(0.0, 1.0) };
        if moon_a > 0.02 {
            let mx = st.wf * (1.0 - self.theme.sunx.at(0.0)).clamp(0.2, 0.8);
            let my = self.horizon * (0.16 + 0.1 * (1.0 - moon_a));
            let mr = st.hero(0.03).max(1.5);
            let mut plate = Plate::new(w, h);
            crate::anim::scenery::moon(&mut plate, mx, my, mr, mr * st.sy, (225, 228, 240), 0.6, self.seed ^ 0x99);
            plate.composite(canvas, 0, 0, |_, _, c| c);
            let _ = moon_a;
            // dim by blending back toward the sky at low alpha
            if moon_a < 0.98 {
                let sky = stops_lit[0].1;
                let (mx0, my0) = ((mx - mr * 3.0).max(0.0) as i32, (my - mr * 3.0 * st.sy).max(0.0) as i32);
                for py in my0..((my + mr * 3.0 * st.sy) as i32).min(horizon_i) {
                    for px in mx0..((mx + mr * 3.0) as i32).min(w as i32) {
                        let a = plate.alpha_at(px, py);
                        if a > 0.0 {
                            canvas.blend(px, py, sky, (1.0 - moon_a) * a);
                        }
                    }
                }
            }
        }

        // ---- meteor ----
        if let Some(p) = self.tl.phase("meteor") {
            if star_a > 0.3 {
                let (x0, y0) = (st.wf * 0.65, self.horizon * 0.12);
                let len = st.wf * 0.18;
                let (dx, dy) = (-len, len * 0.35 * st.sy);
                let head = (x0 + dx * p, y0 + dy * p);
                let tail = (x0 + dx * (p - 0.35).max(0.0), y0 + dy * (p - 0.35).max(0.0));
                stroke_f(canvas, tail.0, tail.1, head.0, head.1, 1.0, (255, 250, 230), 0.8 * (1.0 - p));
            }
        }

        // ---- sun ----
        let alt = self.theme.alt.at(self.tl.t);
        let sun_x = self.theme.sunx.at(self.tl.t) * st.wf;
        let sky_band = self.horizon * 0.95;
        let sun_y = self.horizon - alt * sky_band;
        let sun_r = st.hero(0.045).max(1.5);
        // the far ridge crests sit around alt 0.6; below that the sun is
        // behind the mountains and only its glow remains
        let sun_visible = !storm && alt > 0.3;
        if sun_visible {
            // the disc squashes as it sinks into the haze above the ridges
            let sq = (0.6 + 0.4 * ((alt - 0.5) * 5.0).clamp(0.0, 1.0)) * st.sy;
            let halo_i = 0.25 + 0.75 * wk;
            let halo_col = lerp(self.theme.halo, self.theme.sun, wd * 0.6);
            radial_light(canvas, sun_x, sun_y, sun_r * 7.0, sun_r * 7.0 * st.sy, halo_col, halo_i * 0.3);
            ellipse_f(canvas, sun_x, sun_y, sun_r, sun_r * sq, 0.0, lerp(self.theme.sun, self.theme.halo, wk * 0.5), 1.0);
        }

        // ---- ridges, far to near, with aerial perspective and sun rim ----
        let sun_dir = if sun_visible { (sun_x - st.wf * 0.5).signum() } else { 0.0 };
        let rim_col = lerp(self.theme.halo, (255, 255, 255), wd * 0.5);
        let rim_amt = if sun_visible { (0.35 * wk + 0.12 * wd) * (1.0 - ((alt - 0.5) * 2.5).clamp(0.0, 1.0) * 0.6) } else { 0.0 };
        let n_r = self.ridges.len();
        for (i, r) in self.ridges.iter().enumerate() {
            let fogged = fog(r.color, horizon_color, r.depth, 1.5);
            let lit = scale(fogged, lum.max(0.12));
            let lit = if flash > 0.0 { lerp(lit, (200, 210, 230), flash * r.depth * 0.8) } else { lit };
            let base_col = r.color;
            let off = r.layer.offset(&self.cam, 1.0);
            let profile = &r.profile;
            let base = r.base;
            let rim = rim_amt * (0.3 + 0.7 * (1.0 - r.depth));
            let mist_alpha = if i + 1 < n_r { 0.0 } else { 0.0 };
            let _ = mist_alpha;
            r.layer.composite(canvas, &self.cam, 1.0, |px, py, c| {
                // c is the authored plate color; re-tint by how far the
                // authored color sits from the ridge's base color (snow,
                // pines stay lighter/darker relative to the fogged base)
                let rel = (c.0 as f32 - base_col.0 as f32, c.1 as f32 - base_col.1 as f32, c.2 as f32 - base_col.2 as f32);
                let mut out = (
                    (lit.0 as f32 + rel.0 * lum).clamp(0.0, 255.0) as u8,
                    (lit.1 as f32 + rel.1 * lum).clamp(0.0, 255.0) as u8,
                    (lit.2 as f32 + rel.2 * lum).clamp(0.0, 255.0) as u8,
                );
                if rim > 0.0 {
                    let xi = px as usize;
                    if xi >= 1 && xi + 1 < profile.len() {
                        let top = base - profile[xi];
                        let d = py as f32 - top;
                        if d < 3.0 {
                            let slope = profile[xi + 1] - profile[xi - 1];
                            let facing = (slope * sun_dir).clamp(0.0, 2.0) * 0.5;
                            if facing > 0.0 {
                                out = lerp(out, rim_col, rim * facing * (1.0 - d / 3.0));
                            }
                        }
                    }
                }
                let _ = off;
                out
            });
        }

        // ---- mist between the ridges ----
        let mist_a = self.theme.mist * (0.5 + 0.5 * wn + 0.3 * wk) * (0.75 + 0.25 * vnoise(t * 0.05, 1.0, self.seed));
        if mist_a > 0.02 && n_r >= 2 {
            let band_top = (self.ridges[n_r / 2].base - st.hf * 0.05).max(0.0);
            let band_h = (self.horizon - band_top).max(1.0);
            let seed = self.seed ^ 0x1234;
            let drift = t * 0.6;
            self.mist.fill(w, band_h as usize + 1, |x, y| fbm((x + drift) * 0.025, (y + band_top) * 0.05, 2, seed));
            let mist_col = lerp(horizon_color, (235, 235, 245), 0.2 * wd);
            for y in band_top as i32..horizon_i {
                let v = (y as f32 - band_top) / band_h;
                let fade = (v * 3.0).clamp(0.0, 1.0) * (1.0 - v * 0.3);
                for x in 0..w as i32 {
                    let m = self.mist.at(x as f32, y as f32 - band_top);
                    let a = ((m - 0.42) * 2.2).clamp(0.0, 1.0) * mist_a * fade;
                    if a > 0.01 {
                        canvas.blend(x, y, mist_col, a);
                    }
                }
            }
        }

        // ---- cabin light on the far shore ----
        let cabin = self.tl.env("cabin", 2.0, 1.5) * (0.4 + 0.6 * wn) * if storm { 0.7 } else { 1.0 };
        if cabin > 0.02 {
            let (cx, cy) = (self.cabin_x, self.horizon - 2.0);
            let flick = 0.85 + 0.15 * vnoise(t * 3.0, 0.0, self.seed ^ 7);
            blend_f(canvas, cx, cy, self.theme.cabin, cabin * flick);
            blend_f(canvas, cx + 1.0, cy, self.theme.cabin, cabin * flick * 0.6);
            radial_light(canvas, cx + 0.5, cy, 4.0, 3.0 * st.sy, self.theme.cabin, cabin * 0.25);
        }

        // ---- lake: mirror the composited sky with wind lanes ----
        let chop = if storm { 3.0 } else { 1.0 } * (0.6 + 0.4 * wd + 0.3 * wk);
        let lake_tint = lerp(self.theme.lake, horizon_color, 0.15);
        let glint_on = sun_visible && alt < 0.8;
        let glint_str = ((0.8 - alt) / 0.25).clamp(0.0, 1.0) * ((alt - 0.3) / 0.15).clamp(0.0, 1.0) * 0.7;
        let seed = self.seed ^ 0x77;
        for y in horizon_i..h as i32 {
            let d = (y - horizon_i) as f32;
            let depth = d / (h as f32 - self.horizon).max(1.0);
            // mirrored source row, slightly compressed so the reflection
            // stays inside the frame
            let src_y = self.horizon - 1.0 - d * 0.6;
            let lane = vnoise(d * 0.08 + 3.0, t * 0.15, seed) - 0.5;
            let amp = chop * (0.6 + depth * 3.0) * (0.5 + lane.abs());
            let phase = t * 1.4 + d * 0.5;
            for x in 0..w as i32 {
                let dx = amp * ((x as f32 * 0.11 + phase).sin() + 0.5 * (x as f32 * 0.037 - phase * 0.7 + lane * 6.0).sin());
                let sx = (x as f32 + dx).round() as i32;
                let src = canvas.get(sx.clamp(0, w as i32 - 1), src_y.max(0.0) as i32).color;
                let mut c = lerp(scale(src, 0.62), lake_tint, 0.3 + depth * 0.3);
                // wind lanes darken in streaks
                let streak = (lane * 2.0).clamp(-0.5, 0.5);
                c = scale(c, 1.0 + streak * 0.25);
                if glint_on {
                    let gw = sun_r * (0.7 + depth * 3.5);
                    let dxs = (x as f32 - sun_x).abs();
                    if dxs < gw {
                        let sp = vnoise(x as f32 * 0.7, d * 1.1 + t * 6.0, seed ^ 9);
                        if sp > 0.66 {
                            let g = (1.0 - dxs / gw) * (sp - 0.66) * 3.0 * (0.5 + wk) * glint_str;
                            c = lerp(c, self.theme.sun, g.min(0.85));
                        }
                    }
                }
                if flash > 0.0 {
                    c = lerp(c, (180, 190, 210), flash * 0.25);
                }
                canvas.set(x, y, c);
            }
        }

        // ---- storm rain sheets over the far planes ----
        if storm {
            let n = self.detail.scale(40.0, 12);
            let rain_col = lerp(horizon_color, (220, 225, 235), 0.4);
            for k in 0..n {
                let phase = (t * 1.3 + k as f32 * 0.37) % 1.0;
                let x = ((hash2(k as i32, 1, self.seed) + t * 0.05) % 1.0) * st.wf;
                let y0 = self.horizon * (0.1 + 0.7 * hash2(k as i32, 2, self.seed));
                let len = self.horizon * 0.12;
                let y = y0 + phase * len;
                stroke_f(canvas, x, y, x - len * 0.25, y + len * 0.5 * st.sy, 0.6, rain_col, 0.07);
            }
        }

        // ---- canoe crossing during the quiet hours ----
        let canoe_on = self.tl.active("canoe") && !st.tiny();
        if canoe_on && !self.canoe.active {
            self.canoe.active = true;
            self.canoe.x = if self.canoe.dir > 0.0 { -6.0 } else { st.wf + 6.0 };
            self.canoe.wake.clear();
            self.canoe.stroke_t = 0.0;
        }
        if !canoe_on {
            self.canoe.active = false;
        }
        if self.canoe.active {
            let len = st.hero(0.11).max(4.0);
            let speed = (st.wf + 12.0) / 13.0;
            self.canoe.x += self.canoe.dir * speed * dt;
            self.canoe.stroke_t += dt;
            let stroke = (self.canoe.stroke_t * 1.6).fract();
            let lean = self.canoe.lean.step(if stroke < 0.3 { 0.35 } else { 0.0 }, dt);
            let cy = self.horizon + (st.hf - self.horizon) * 0.32;
            let ink = scale(self.theme.ridge_near, 0.7 + 0.3 * lum);
            let ink = lerp(ink, (0, 0, 0), 0.5);
            if stroke < 0.05 && self.canoe.wake.len() < 60 {
                self.canoe.wake.push((self.canoe.x - self.canoe.dir * len * 0.5, cy, 0.0));
            }
            for wpt in self.canoe.wake.iter_mut() {
                wpt.2 += dt;
            }
            self.canoe.wake.retain(|p| p.2 < 4.0);
            for &(wx, wy, age) in &self.canoe.wake {
                let a = (1.0 - age / 4.0) * 0.35;
                let spread = age * 3.0;
                let wcol = lerp(lake_tint, horizon_color, 0.5);
                blend_f(canvas, wx - self.canoe.dir * spread * 0.6, wy - spread * 0.25 * st.sy, wcol, a);
                blend_f(canvas, wx - self.canoe.dir * spread * 0.6, wy + spread * 0.25 * st.sy, wcol, a);
            }
            let hx = self.canoe.x;
            // hull: a shallow crescent
            let hull_h = (len * 0.18 * st.sy).max(1.0);
            capsule(canvas, hx - len * 0.5, cy - hull_h * 0.3, 0.4, hx + len * 0.5, cy - hull_h * 0.3, 0.4, ink, 1.0);
            capsule(canvas, hx - len * 0.38, cy, hull_h * 0.55, hx + len * 0.38, cy, hull_h * 0.55, ink, 1.0);
            // paddler: torso capsule leaning with the stroke, head disc, paddle stroke
            let px = hx + self.canoe.dir * len * 0.05;
            let torso_h = len * 0.32 * st.sy;
            let lean_x = self.canoe.dir * lean * torso_h * 0.5;
            capsule(canvas, px, cy - hull_h * 0.4, hull_h * 0.45, px + lean_x, cy - hull_h * 0.4 - torso_h, hull_h * 0.32, ink, 1.0);
            ellipse_f(canvas, px + lean_x, cy - hull_h * 0.4 - torso_h - hull_h * 0.35, hull_h * 0.36, hull_h * 0.36 * 1.1, 0.0, ink, 1.0);
            let pa = (stroke * std::f32::consts::TAU).sin();
            let paddle_x = px + lean_x + self.canoe.dir * len * (0.12 + 0.12 * pa);
            capsule(canvas, px + lean_x, cy - hull_h * 0.4 - torso_h * 0.6, 0.3, paddle_x, cy + hull_h * 0.6 + (1.0 - stroke) * hull_h * 0.4, 0.3, ink, 0.9);
            // reflection: the same silhouette mirrored and wobbled
            let ref_a = 0.35 + 0.15 * wn;
            let wob = (t * 2.0 + hx * 0.3).sin() * 0.6;
            capsule(canvas, hx - len * 0.38 + wob, cy + hull_h * 1.2, hull_h * 0.5, hx + len * 0.38 + wob, cy + hull_h * 1.2, hull_h * 0.5, ink, ref_a);
            capsule(canvas, px + wob, cy + hull_h * 1.6, hull_h * 0.4, px + lean_x + wob, cy + hull_h * 1.6 + torso_h, hull_h * 0.3, ink, ref_a * 0.8);
            if self.canoe.x < -12.0 || self.canoe.x > st.wf + 12.0 {
                self.canoe.active = false;
            }
        }

        // ---- birds ----
        if !st.tiny() && (wd > 0.3 || wk > 0.5) && !storm && self.flocks.len() < 2 && self.rng.random::<f32>() < dt * 0.06 {
            self.spawn_flock();
        }
        for fl in self.flocks.iter_mut() {
            fl.x += fl.vx * dt;
            fl.phase += dt * 9.0;
            let dark = lerp(self.theme.ridge_near, (0, 0, 0), 0.4);
            for k in 0..fl.n {
                let kk = k as f32;
                let bx = fl.x - fl.vx.signum() * kk * 3.2;
                let byy = fl.y + kk * 1.1 * st.sy + (t * 0.7 + kk).sin() * 0.4;
                let flap = (fl.phase + kk * 0.8).sin() * 1.2 * st.sy;
                stroke_f(canvas, bx - 1.4, byy - flap, bx, byy, 0.6, dark, 0.85);
                stroke_f(canvas, bx, byy, bx + 1.4, byy - flap, 0.6, dark, 0.85);
            }
            if fl.x < -30.0 || fl.x > st.wf + 30.0 {
                fl.alive = false;
            }
        }
        self.flocks.retain(|f| f.alive);

        // ---- near bank, unlit by the sky (it is the frame's dark edge) ----
        let bank_lum = 0.55 + 0.45 * lum;
        let bank_fog = horizon_color;
        self.bank.composite(canvas, 0, 0, |_, _, c| {
            let c = scale(c, bank_lum);
            if flash > 0.0 { lerp(c, bank_fog, flash * 0.3) } else { c }
        });
        let _ = &mut self.scratch;
        let _ = ease(Ease::Linear, 0.0);
        let _ = mix((0, 0, 0), (0, 0, 0), 0.0);
    }
}
