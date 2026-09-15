//! Falling-sand automaton: colored grains pour from emitters, obey
//! fall/slide rules, and pile into hue-stratified sandstone dunes.
//! Overfull piles dissolve and restart. Sandstorms run anticipation →
//! scour → settle with wind-skewed fall, lifted grain haze, crest tint
//! bleed, and a denser near-plane of dust motes.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, hsv, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Pour,
    Dissolve,
}

/// Near-camera dust mote drifting past on the wind (foreground plane).
struct Mote {
    phase: f32,
    speed: f32,
    y_frac: f32,
    size: f32,
}

pub struct Sand {
    rng: StdRng,
    detail: Detail,
    hue_lo: f32,
    hue_span: f32,
    sat: f32,
    /// 0 = empty, otherwise hue (0..=255) + 1; 254 = splitter, 255 = gemstone
    grid: Vec<u8>,
    emitters: Vec<usize>,
    splitters: Vec<(usize, usize)>,
    motes: Vec<Mote>,
    hue_t: f32,
    phase: Phase,
    dissolve_t: f32,
    storm: f32,
    storm_dur: f32,
    next_storm: f32,
    /// Wind skew during storm (−1..1), baked at storm start.
    wind: f32,
    step_acc: f32,
    w: usize,
    h: usize,
}

impl Sand {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (hue_lo, hue_span, sat) = match theme {
            Some("mono") => (0.0, 0.0, 0.12),
            Some("ocean") => (0.5, 0.12, 0.6),
            _ => (0.02, 0.11, 0.62), // sandstone
        };
        Sand {
            rng,
            detail,
            hue_lo,
            hue_span,
            sat,
            grid: Vec::new(),
            emitters: Vec::new(),
            splitters: Vec::new(),
            motes: Vec::new(),
            hue_t: 0.0,
            phase: Phase::Pour,
            dissolve_t: 0.0,
            storm: 0.0,
            storm_dur: 5.5,
            next_storm: 12.0, // first storm lands early; then 18-32s apart
            wind: 0.0,
            step_acc: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.grid = vec![0; w * h];
        // splitter obstacles: V-shaped diverters that split the streams
        self.splitters = (1..=2).map(|i| (w * i / 3, h * i / 3)).collect();
        let n = ((w as f32 / 18.0) * density_for(w, h) * self.detail.density())
            .round()
            .clamp(3.0, 22.0) as usize;
        self.motes = (0..n)
            .map(|_| Mote {
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                speed: self.rng.random_range(2.0..6.0),
                y_frac: self.rng.random_range(0.05..0.95),
                size: self.rng.random_range(0.5..1.4),
            })
            .collect();
        self.phase = Phase::Pour;
        self.spawn_emitters();
    }

    fn spawn_emitters(&mut self) {
        let base = self.detail.scale(2.0, 1);
        let n = self.rng.random_range(base..=(base + 2).min(5));
        let w = self.w;
        self.emitters = (0..n)
            .map(|i| {
                if n == 1 {
                    w / 2
                } else {
                    w * (i + 1) / (n + 1) + self.rng.random_range(0..(w / 8).max(1))
                }
            })
            .collect();
    }

    /// Storm envelope: dust builds first (anticipation), erosion surges at
    /// the peak, then the wind dies down (decay). Longer attack than before.
    fn storm_intensity(&self) -> f32 {
        if self.storm <= 0.0 {
            return 0.0;
        }
        let elapsed = self.storm_dur - self.storm;
        ease_smooth(elapsed / 1.8) * ease_smooth(self.storm / 2.2)
    }

    /// Early storm phase for sky darken (anticipation before peak scour).
    fn storm_build(&self) -> f32 {
        if self.storm <= 0.0 {
            return 0.0;
        }
        let elapsed = self.storm_dur - self.storm;
        if elapsed < 1.8 {
            ease_smooth(elapsed / 1.8)
        } else if self.storm > 2.2 {
            1.0
        } else {
            ease_smooth(self.storm / 2.2)
        }
    }

    fn current_hue(&self) -> u8 {
        let h = self.hue_lo + self.hue_span * (0.5 + 0.5 * (self.hue_t * 0.25).sin());
        (h * 255.0) as u8
    }

    fn emit(&mut self) {
        let storm = self.storm_intensity();
        let hue = self.current_hue() + 1;
        let rate = 0.85 + storm * 0.55 * self.detail.density().min(1.5);
        let gem_p = 0.004 + storm * 0.02; // more gemstones during storm payoff
        for &e in &self.emitters {
            if e < self.w && self.grid[e] == 0 && self.rng.random::<f32>() < rate {
                self.grid[e] = if self.rng.random::<f32>() < gem_p {
                    255
                } else {
                    hue
                };
            }
        }
    }

    /// Light wind erosion: crest grains occasionally shift sideways.
    fn erode(&mut self) {
        let (w, h) = (self.w, self.h);
        if w < 3 || h < 3 {
            return;
        }
        let storm = self.storm_intensity();
        let passes = ((w / 6).max(1) as f32 * (1.0 + storm * 2.5 * self.detail.factor())) as usize;
        for _ in 0..passes {
            let x = self.rng.random_range(1..w - 1);
            let y = self.rng.random_range(1..h - 1);
            let i = y * w + x;
            // crest grain: solid with open sky above
            if self.grid[i] != 0 && self.grid[i] < 254 && self.grid[i - w] == 0
                && self.rng.random::<f32>() < 0.4 + storm * 0.35
            {
                let d = if self.wind.abs() > 0.15 {
                    if self.wind > 0.0 { 1 } else { -1 }
                } else if self.rng.random::<bool>() {
                    1
                } else {
                    -1
                };
                let nx = x as i32 + d;
                let ni = (y * w).wrapping_add(nx as usize);
                let below = ni + w;
                if nx >= 0
                    && nx < w as i32
                    && self.grid[ni] == 0
                    && below < w * h
                    && self.grid[below] == 0
                {
                    self.grid.swap(i, ni);
                }
            }
        }
    }

    /// One automaton step. Returns the number of grains that moved.
    pub fn step(&mut self) -> usize {
        let (w, h) = (self.w, self.h);
        if w == 0 || h < 2 {
            return 0;
        }
        let mut moved = 0;
        let storm = self.storm_intensity();
        // wind skew: prefer diagonal fall in wind direction during storm
        let left_first = if storm > 0.2 && self.wind.abs() > 0.1 {
            self.wind < 0.0
        } else {
            self.rng.random::<bool>()
        };
        let forward = self.rng.random::<bool>();
        // bottom-up so grains fall one cell per step
        for y in (0..h - 1).rev() {
            for xi in 0..w {
                // alternate scan direction without a per-row allocation
                let x = if forward { xi } else { w - 1 - xi };
                let i = y * w + x;
                if self.grid[i] == 0 || self.grid[i] == 254 {
                    continue;
                }
                let below = (y + 1) * w + x;
                // during storm peak, sometimes prefer wind-diagonal first
                if storm > 0.45 && self.rng.random::<f32>() < storm * 0.55 {
                    let d = if self.wind >= 0.0 { 1 } else { -1 };
                    let nx = x as i32 + d;
                    if nx >= 0 && nx < w as i32 {
                        let ni = (y + 1) * w + nx as usize;
                        if self.grid[ni] == 0 {
                            self.grid.swap(i, ni);
                            moved += 1;
                            continue;
                        }
                    }
                }
                if self.grid[below] == 0 {
                    self.grid.swap(i, below);
                    moved += 1;
                    continue;
                }
                let dirs: [i32; 2] = if left_first { [-1, 1] } else { [1, -1] };
                for d in dirs {
                    let nx = x as i32 + d;
                    if nx < 0 || nx >= w as i32 {
                        continue;
                    }
                    let ni = (y + 1) * w + nx as usize;
                    if self.grid[ni] == 0 {
                        self.grid.swap(i, ni);
                        moved += 1;
                        break;
                    }
                }
            }
        }
        moved
    }

    #[cfg(test)]
    fn grain_count(&self) -> usize {
        self.grid.iter().filter(|&&v| v != 0 && v != 254).count()
    }
}

impl Scene for Sand {
    fn name(&self) -> &'static str {
        "sand"
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
        self.hue_t += dt;
        let t = self.hue_t;

        // sandstorm: wind builds, erosion surges, dust settles
        self.next_storm -= dt;
        if self.next_storm <= 0.0 {
            self.storm = self.storm_dur;
            self.wind = self.rng.random_range(-1.0..1.0);
            self.next_storm = self.rng.random_range(18.0..32.0);
        }
        self.storm = (self.storm - dt).max(0.0);
        let storm = self.storm_intensity();
        let build = self.storm_build();

        match self.phase {
            Phase::Pour => {
                let rate = 55.0 + storm * 35.0; // faster pour during storm payoff
                self.step_acc += dt * rate;
                while self.step_acc >= 1.0 {
                    self.step_acc -= 1.0;
                    self.emit();
                    self.step();
                    self.erode();
                    if storm > 0.35 {
                        // peak wind scours the crests
                        self.erode();
                        self.erode();
                    }
                }
                // dissolve when the pile fills ~45% of the canvas (measuring
                // total fill, not the falling stream near the emitters)
                let filled = self.grid.iter().filter(|&&v| v != 0).count();
                if filled > w * h * 45 / 100 {
                    self.phase = Phase::Dissolve;
                    self.dissolve_t = 0.0;
                }
            }
            Phase::Dissolve => {
                self.dissolve_t += dt;
                if self.dissolve_t > 1.2 {
                    self.grid.fill(0);
                    self.phase = Phase::Pour;
                    self.spawn_emitters(); // fresh streams for the new cycle
                }
            }
        }

        // sky darkens during storm build / peak
        let sky = scale((18, 14, 10), 1.0 - build * 0.55);
        canvas.clear(sky);

        // draw splitters (grains treat them as solid: block via occupancy)
        for &(sx, sy) in &self.splitters {
            for arm in 0..4i32 {
                canvas.set(sx as i32 - arm, sy as i32 + arm, (70, 60, 50));
                canvas.set(sx as i32 + arm, sy as i32 + arm, (70, 60, 50));
                // block the cells so grains slide off the V
                let a = arm as usize;
                if sx >= a && sy + a < self.h {
                    self.grid[(sy + a) * self.w + sx - a] = 254;
                }
                if sx + a < self.w && sy + a < self.h {
                    self.grid[(sy + a) * self.w + sx + a] = 254;
                }
            }
        }
        let fade = if self.phase == Phase::Dissolve {
            // sparkling dissolve: brightness drops with per-grain jitter
            1.0 - (self.dissolve_t / 1.2).clamp(0.0, 1.0)
        } else {
            1.0
        };
        // column-major so burial depth accrues down each contiguous run
        for x in 0..w {
            let mut buried = 0u32;
            for y in 0..h {
                let i = y * w + x;
                let v = self.grid[i];
                if v == 0 {
                    buried = 0;
                    continue;
                }
                if v == 254 {
                    // splitter: keep the dim stone draw
                    buried += 1;
                    continue;
                }
                if v == 255 {
                    // gemstone: cold sparkle — brighter during storm payoff
                    let tw = 0.5 + 0.5 * (t * 6.0 + (x * 13 + y * 7) as f32).sin();
                    let spark = 0.5 + tw * 0.7 + storm * 0.35;
                    let c = scale((190, 230, 255), spark * fade);
                    canvas.set(x as i32, y as i32, c);
                    if tw > 0.88 - storm * 0.15 {
                        glow(canvas, x as i32, y as i32, 2, (200, 240, 255), 0.25 + storm * 0.35);
                        canvas.add(x as i32 + 1, y as i32, scale(c, 0.5));
                        canvas.add(x as i32, y as i32 + 1, scale(c, 0.5));
                    }
                    buried += 1;
                    continue;
                }
                // depth shading: exposed crests catch the light, buried
                // grains sink into shadow — the dunes gain their relief
                let exposed = y == 0
                    || self.grid[i - w] == 0
                    || x == 0
                    || self.grid[i - 1] == 0
                    || x + 1 == w
                    || self.grid[i + 1] == 0
                    || y + 1 == h
                    || self.grid[i + w] == 0;
                let shade = if exposed {
                    1.12
                } else {
                    (0.98 - buried as f32 * 0.05).max(0.45)
                };
                buried += 1;
                let hue = (v - 1) as f32 / 255.0;
                // slight per-grain value variation for texture
                let jitter = 0.85 + (((x * 733 + y * 911) % 100) as f32 / 100.0) * 0.3;
                let mut c = hsv(hue, self.sat, 0.8 * jitter * shade);
                if exposed {
                    // warm rim light on the crests; storms wash them brighter
                    c = lerp(c, (255, 236, 200), 0.14 + storm * 0.22);
                    // storm tint bleed into dune tops
                    if storm > 0.1 {
                        c = lerp(c, (220, 170, 100), storm * 0.18);
                    }
                }
                canvas.set(x as i32, y as i32, scale(c, fade));
            }
        }
        // storm dust haze / lifted grain cloud, drifting with the wind
        if storm > 0.01 {
            let k = storm * (0.10 + 0.08 * self.detail.factor().min(1.5));
            let drift = ((t * 9.0) as i32 + (self.wind.signum() as i32) * (t * 14.0) as i32) as usize;
            let cloud_h = (h as f32 * (0.55 + storm * 0.25)) as usize;
            for y in 0..cloud_h {
                for x in 0..w {
                    if ((x + drift) * 31 + y * 17) % 7 < 2 + (storm * 2.0) as usize {
                        canvas.add(x as i32, y as i32, scale((190, 160, 110), k));
                    }
                }
            }
        }

        // emitters breathe a warm glow, swelling with the storm
        if self.phase == Phase::Pour {
            let pulse = 0.7 + 0.3 * (t * 3.0).sin();
            for &e in &self.emitters {
                glow(
                    canvas,
                    e as i32,
                    0,
                    2,
                    (200, 150, 80),
                    0.10 * pulse + storm * 0.12,
                );
                canvas.add(e as i32, 0, scale((60, 45, 25), pulse));
            }
        }

        // foreground: near-camera dust motes — denser / brighter in storm
        let span = w as f32 + 8.0;
        let mote_boost = 1.0 + storm * 2.8;
        for m in &self.motes {
            let wx = m.speed * mote_boost * (1.0 + self.wind.abs() * 0.5);
            let mx = (t * wx + m.phase * span) % span - 4.0;
            let my = m.y_frac * h as f32 + (t * 0.6 + m.phase).sin() * (1.5 + storm * 2.0);
            let bri = (0.05 + storm * 0.18) * m.size;
            canvas.add(mx as i32, my as i32, scale((215, 185, 140), bri));
            if storm > 0.3 && m.size > 0.9 {
                glow(
                    canvas,
                    mx as i32,
                    my as i32,
                    1,
                    (220, 190, 140),
                    storm * 0.2 * m.size,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn grains_are_conserved() {
        let mut s = Sand::new(StdRng::seed_from_u64(71), None, Detail::Medium);
        s.init(20, 20);
        s.emitters.clear(); // no new grains during the test
        // scatter some grains by hand
        for i in 0..40 {
            s.grid[i * 3 % 400] = 100;
        }
        let before = s.grain_count();
        for _ in 0..300 {
            s.step();
        }
        assert_eq!(s.grain_count(), before, "grains must be conserved");
    }

    #[test]
    fn piles_settle_to_rest() {
        let mut s = Sand::new(StdRng::seed_from_u64(72), None, Detail::Medium);
        s.init(15, 15);
        s.emitters.clear();
        // a 1-wide column falls straight down and rests on the floor
        for y in 0..8 {
            s.grid[y * 15 + 7] = 100;
        }
        for _ in 0..100 {
            s.step();
        }
        assert_eq!(s.step(), 0, "a settled pile should not move");
        // all grains rest in the bottom 3 rows (column spreads into a pyramid)
        for x in 0..15 {
            for y in 0..12 {
                assert_eq!(s.grid[y * 15 + x], 0, "grain floating at ({x},{y})");
            }
        }
    }

    #[test]
    fn dunes_form_under_emitter() {
        let mut s = Sand::new(StdRng::seed_from_u64(73), None, Detail::Medium);
        let mut c = Canvas::new(40, 30);
        // mid-run: grains accumulate
        for _ in 0..200 {
            s.update(1.0 / 30.0, &mut c);
        }
        let grains = s.grid.iter().filter(|&&v| v != 0).count();
        assert!(grains > 80, "sand should have accumulated, got {grains}");
        // long-run: pile eventually grows high enough to trigger dissolve
        let mut dissolved = false;
        for _ in 0..3000 {
            s.update(1.0 / 30.0, &mut c);
            if s.phase == Phase::Dissolve {
                dissolved = true;
                break;
            }
        }
        assert!(dissolved, "overfull pile should dissolve");
    }
}
