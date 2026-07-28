//! Frost: true ice-fern dendrites freezing across black glass. Each crystal
//! grows a main spine that sheds regular alternating side-branches at ~60°
//! (which sub-branch once more), with thickness tapering from a broad root
//! to a hairline tip. Every grown cell remembers a facet orientation, and a
//! slow light band sweeping the pane makes those facets shimmer as it passes.
//! A NUCLEATION event fires every ~9-16s: a seed flashes, rings outward, and
//! explodes into a radial star of 6+ arms with an eased speed burst; the
//! expanding shock ring rim-lights the crystals it sweeps over. Warm
//! breath-fog rolls in from the pane edges every ~14-22s, milking over the
//! glass and dimming the crystals beneath before clearing. Three depth
//! planes (dim thin far fronts, bright broad near ferns) plus a faint
//! drifting haze veil underneath. Growth speed scales with pane size so
//! large windows fill as readily as small ones. Growth fills the pane, then
//! a slow eased melt, a quiet rest beat, and fresh seeds — many creeping in
//! from the edges, as real frost does.

use super::{noise::fbm, Detail, Scene};
use crate::canvas::{density_for, ease_out, ease_smooth, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::{PI, TAU};

/// Depth planes: (brightness, growth speed px/s, thickness multiplier).
/// Far fronts are dim, thin and slow; near ferns are bright and broad.
const LAYERS: [(f32, f32, f32); 3] = [
    (0.22, 2.6, 0.45), // far
    (0.50, 4.6, 0.75), // mid
    (0.95, 6.5, 1.00), // near
];

const MAX_ARMS: usize = 640;
const MAX_GLINTS: usize = 24;
const MAX_RINGS: usize = 8;
const MAX_HOT_TIPS: usize = 260;
const MELT_DUR: f32 = 3.0;
/// Growth sub-step length in px (kept under a cell so arms have no gaps).
const STEP: f32 = 0.4;
/// Side-branch angle: ~60°, the classic ice-fern split.
const BRANCH_ANG: f32 = PI / 3.0;

/// Growth lifecycle: Grow -> Melt (fade) -> Rest (quiet beat) -> reseed.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Grow,
    Melt(f32), // elapsed
    Rest(f32), // elapsed (duration in `rest_dur`)
}

struct Arm {
    x: f32,
    y: f32,
    heading: f32,
    bias: f32, // preferred heading; keeps the spine arrow-straight
    layer: usize,
    gen: u8, // 0 = spine, 1 = side branch, 2 = sub-branch (no further splits)
    len: f32,
    max_len: f32,
    next_branch: f32, // spine length at which the next side branch sheds
    branch_step: f32, // regular spacing between side branches
    side: f32,        // alternates branch chirality for the fern look
    width0: f32,      // root thickness; tapers to a hairline at the tip
    facet: f32,       // 0..1 facet family, drives the shimmer
    burst: f32,       // nucleation energy 0..1, eases speed + brightness
    px: i32,          // last painted cell; growth steps are sub-cell, so the
    py: i32,          // collision check must ignore the arm's own fresh paint
}

struct Glint {
    x: i32,
    y: i32,
    age: f32,
    dur: f32,
}

/// Expanding shock ring from a nucleation burst.
struct Ring {
    x: f32,
    y: f32,
    age: f32,
    dur: f32,
}

pub struct Frost {
    rng: StdRng,
    detail: Detail,
    base_c: (u8, u8, u8),
    tip_c: (u8, u8, u8),
    glint_c: (u8, u8, u8),
    fog_c: (u8, u8, u8),
    /// Persistent crystal brightness per cell, 0..~1.2 (tips can exceed 1).
    field: Vec<f32>,
    /// Facet family 0..1 per crystal cell (for the light-play shimmer).
    facet: Vec<f32>,
    /// Indices of cells that hold crystal (for coverage budget + glints).
    filled: Vec<usize>,
    arms: Vec<Arm>,
    /// Growth scratch buffers, reused every frame (no per-frame allocs).
    branches: Vec<Arm>,
    dead: Vec<usize>,
    glints: Vec<Glint>,
    rings: Vec<Ring>,
    /// (x, y, brightness) of glowing growth tips, rebuilt each frame.
    hot_tips: Vec<(i32, i32, f32)>,
    phase: Phase,
    rest_dur: f32,
    next_burst: f32,
    /// Breath-fog event: <0 inactive, else seconds since it rolled in.
    fog_t: f32,
    fog_dur: f32,
    next_fog: f32,
    fog_dir: (f32, f32), // unit vector the fog wave drifts along
    next_glint: f32,
    haze_seed: u32,
    t: f32,
    w: usize,
    h: usize,
}

impl Frost {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (base_c, tip_c, fog_c) = match theme {
            Some("aurora") => ((64, 198, 168), (214, 255, 238), (120, 215, 190)),
            Some("mono") => ((168, 176, 190), (246, 249, 255), (185, 192, 205)),
            _ => ((104, 158, 210), (226, 242, 255), (150, 180, 215)), // ice (default)
        };
        let haze_seed = rng.random_range(1..1_000_000);
        Frost {
            rng,
            detail,
            base_c,
            tip_c,
            glint_c: lerp(tip_c, (255, 255, 255), 0.6),
            fog_c,
            field: Vec::new(),
            facet: Vec::new(),
            filled: Vec::new(),
            arms: Vec::new(),
            branches: Vec::new(),
            dead: Vec::new(),
            glints: Vec::new(),
            rings: Vec::new(),
            hot_tips: Vec::new(),
            phase: Phase::Grow,
            rest_dur: 2.0,
            next_burst: 4.0, // first starburst lands early enough to be seen
            fog_t: -1.0,
            fog_dur: 8.0,
            next_fog: 7.0,
            fog_dir: (1.0, 0.0),
            next_glint: 1.5,
            haze_seed,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.field = vec![0.0; w * h];
        self.facet = vec![0.0; w * h];
        self.filled.clear();
        self.arms.clear();
        self.branches.clear();
        self.dead.clear();
        self.glints.clear();
        self.rings.clear();
        self.hot_tips.clear();
        self.phase = Phase::Grow;
        if w > 0 && h > 0 {
            self.spawn_seeds();
        }
    }

    /// Push one dendrite arm (shared by seeds and bursts).
    fn push_arm(&mut self, x: f32, y: f32, heading: f32, layer: usize, max_len: f32, burst: f32) {
        if self.arms.len() >= MAX_ARMS {
            return;
        }
        let branch_step = self.rng.random_range(1.7..2.9);
        self.arms.push(Arm {
            x,
            y,
            heading,
            bias: heading,
            layer,
            gen: 0,
            len: 0.0,
            max_len,
            next_branch: branch_step * self.rng.random_range(0.6..1.1),
            branch_step,
            side: if self.rng.random::<bool>() { 1.0 } else { -1.0 },
            // burst spikes stay slender; seeded spines get broad roots
            width0: self.rng.random_range(0.85..1.15) * if burst > 0.0 { 0.55 } else { 1.0 },
            facet: (heading / TAU).rem_euclid(1.0) + self.rng.random_range(-0.04..0.04),
            burst,
            px: -1,
            py: -1,
        });
    }

    /// Scatter nucleation points. Over half creep in from the pane edges
    /// (fanning inward like real window frost); the rest seed the interior.
    /// Each seed flashes as a tiny glint — lighting interplay at birth.
    fn spawn_seeds(&mut self) {
        let (w, h) = (self.w, self.h);
        let n = self.detail.scale(6.0 * density_for(w, h), 3).clamp(3, 18);
        let min_dim = w.min(h) as f32;
        for _ in 0..n {
            let from_edge = self.rng.random::<f32>() < 0.55;
            let (x, y, base_heading) = if from_edge {
                let edge = self.rng.random_range(0..4);
                let inward = self.rng.random_range(1.0..3.0);
                match edge {
                    0 => (
                        self.rng.random_range(0.0..w as f32),
                        inward,
                        PI / 2.0, // down from the top
                    ),
                    1 => (
                        self.rng.random_range(0.0..w as f32),
                        h as f32 - 1.0 - inward,
                        -PI / 2.0, // up from the bottom
                    ),
                    2 => (
                        inward,
                        self.rng.random_range(0.0..h as f32),
                        0.0, // right from the left edge
                    ),
                    _ => (
                        w as f32 - 1.0 - inward,
                        self.rng.random_range(0.0..h as f32),
                        PI, // left from the right edge
                    ),
                }
            } else {
                (
                    self.rng.random_range(2.0..(w as f32 - 2.0).max(3.0)),
                    self.rng.random_range(2.0..(h as f32 - 2.0).max(3.0)),
                    self.rng.random_range(0.0..TAU),
                )
            };
            let arms = if from_edge {
                self.rng.random_range(3..=5)
            } else {
                self.rng.random_range(5..=7)
            };
            for a in 0..arms {
                let r = self.rng.random::<f32>();
                let layer = if r < 0.45 {
                    0
                } else if r < 0.8 {
                    1
                } else {
                    2
                };
                let heading = if from_edge {
                    // fan around the inward normal
                    base_heading + (a as f32 - (arms - 1) as f32 * 0.5) * 0.55
                        + self.rng.random_range(-0.15..0.15)
                } else {
                    base_heading + a as f32 / arms as f32 * TAU
                        + self.rng.random_range(-0.25..0.25)
                };
                let max_len = min_dim * self.rng.random_range(0.20..0.48);
                self.push_arm(x, y, heading, layer, max_len, 0.0);
            }
            self.glints.push(Glint {
                x: x as i32,
                y: y as i32,
                age: 0.0,
                dur: 0.5,
            });
        }
    }

    /// NUCLEATION burst: a flash, a shock ring, and a radial star of 6+
    /// arms erupting with an eased speed burst. The hero event.
    fn nucleate(&mut self) {
        let (w, h) = (self.w as f32, self.h as f32);
        let m = 4.0;
        let x = self.rng.random_range(m..(w - m).max(m + 1.0));
        let y = self.rng.random_range(m..(h - m).max(m + 1.0));
        let arms = self.rng.random_range(6..=9);
        let rot = self.rng.random_range(0.0..TAU);
        let min_dim = self.w.min(self.h) as f32;
        for a in 0..arms {
            // hero event lives in the near/mid planes
            let layer = if self.rng.random::<f32>() < 0.6 { 2 } else { 1 };
            let heading = rot + a as f32 / arms as f32 * TAU + self.rng.random_range(-0.12..0.12);
            let max_len = min_dim * self.rng.random_range(0.16..0.30);
            self.push_arm(x, y, heading, layer, max_len, 1.0);
        }
        if self.glints.len() < MAX_GLINTS {
            self.glints.push(Glint {
                x: x as i32,
                y: y as i32,
                age: 0.0,
                dur: 0.9,
            });
        }
        if self.rings.len() < MAX_RINGS {
            self.rings.push(Ring {
                x,
                y,
                age: 0.0,
                dur: 0.9,
            });
        }
    }

    /// Advance all arms one frame: spine-straight growth, regular ~60° side
    /// branches (one level of sub-branching), tapering thickness.
    fn grow(&mut self, dt: f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let (fw, fh) = (self.w, self.h);
        // growth scales with pane size: frost fills a big window in roughly
        // the same wall-clock time as a small one
        let size_k = (self.w.min(self.h) as f32 / 45.0).clamp(0.8, 1.7);
        self.branches.clear();
        self.dead.clear();
        self.hot_tips.clear();
        let rng = &mut self.rng;
        let arm_count = self.arms.len();
        for (ai, arm) in self.arms.iter_mut().enumerate() {
            let (_, speed0, thick0) = LAYERS[arm.layer];
            // nucleation burst: fast bright eruption, easing back to normal
            arm.burst = (arm.burst - dt / 1.8).max(0.0);
            let burst_k = ease_out(arm.burst);
            let speed = speed0 * size_k * (1.0 + 2.4 * burst_k);
            let mut dist = speed * dt;
            let mut alive = true;
            while dist > 0.0 {
                dist -= STEP;
                arm.len += STEP;
                // spines stay arrow-straight; branches may wander a little
                let (jitter, pull) = if arm.gen == 0 { (0.05, 0.10) } else { (0.11, 0.05) };
                arm.heading +=
                    rng.random_range(-jitter..jitter) + (arm.bias - arm.heading) * pull;
                arm.x += arm.heading.cos() * STEP;
                arm.y += arm.heading.sin() * STEP;
                // die at the pane edge, at full length, or against crystal
                // (but never against the pixel we just painted ourselves —
                // growth steps are sub-cell, so the same cell recurs)
                let out = arm.x < 0.0 || arm.y < 0.0 || arm.x >= w || arm.y >= h;
                if out || arm.len >= arm.max_len {
                    self.dead.push(ai);
                    alive = false;
                    break;
                }
                let (cxi, cyi) = (arm.x as i32, arm.y as i32);
                if (cxi, cyi) != (arm.px, arm.py) {
                    arm.px = cxi;
                    arm.py = cyi;
                    if self.field[cyi as usize * fw + cxi as usize] > 0.10
                        && rng.random::<f32>() < 0.5
                    {
                        self.dead.push(ai);
                        alive = false;
                        break;
                    }
                }
                // regular alternating ~60° side branches, one level of subs
                if arm.gen < 2 && arm.len >= arm.next_branch {
                    arm.side = -arm.side; // alternate chirality => fern fronds
                    if arm_count + self.branches.len() < MAX_ARMS {
                        let off = (BRANCH_ANG + rng.random_range(-0.12..0.12)) * arm.side;
                        let heading = arm.heading + off;
                        let taper_here = 1.0 - arm.len / arm.max_len;
                        let width0 = arm.width0 * taper_here * 0.72;
                        let branch_step = arm.branch_step * rng.random_range(0.65..0.95);
                        self.branches.push(Arm {
                            x: arm.x,
                            y: arm.y,
                            heading,
                            bias: heading,
                            layer: arm.layer,
                            gen: arm.gen + 1,
                            len: 0.0,
                            max_len: (arm.max_len - arm.len)
                                * rng.random_range(0.40..0.62)
                                / (arm.gen as f32 + 1.0),
                            next_branch: branch_step * rng.random_range(0.6..1.1),
                            branch_step,
                            side: -arm.side,
                            width0,
                            facet: (heading / TAU).rem_euclid(1.0)
                                + rng.random_range(-0.04..0.04),
                            burst: 0.0,
                            px: -1,
                            py: -1,
                        });
                    }
                    arm.next_branch += arm.branch_step * rng.random_range(0.85..1.25);
                }
                // thickness tapers from broad root to hairline tip
                let frac = (1.0 - arm.len / arm.max_len).max(0.0);
                let thick = (0.15 + 0.85 * frac.powf(1.6)) * arm.width0 * thick0;
                let v = LAYERS[arm.layer].0
                    * (0.45 + 0.55 * thick)
                    * (1.0 + 0.6 * burst_k);
                paint_cell(
                    &mut self.field,
                    &mut self.facet,
                    &mut self.filled,
                    fw,
                    fh,
                    arm.x,
                    arm.y,
                    v,
                    arm.facet,
                );
                // broad near-plane roots paint a second pixel row, giving
                // spines visible body that thins out along the arm
                if thick > 0.80 {
                    let (px, py) = (-arm.heading.sin(), arm.heading.cos());
                    paint_cell(
                        &mut self.field,
                        &mut self.facet,
                        &mut self.filled,
                        fw,
                        fh,
                        arm.x + px,
                        arm.y + py,
                        v * 0.5,
                        arm.facet,
                    );
                    paint_cell(
                        &mut self.field,
                        &mut self.facet,
                        &mut self.filled,
                        fw,
                        fh,
                        arm.x - px,
                        arm.y - py,
                        v * 0.5,
                        arm.facet,
                    );
                }
            }
            // glowing growth tips: near plane always, bursting arms brightest
            if alive
                && self.hot_tips.len() < MAX_HOT_TIPS
                && (arm.layer == 2 || burst_k > 0.05)
            {
                let b = 0.30 + 0.55 * burst_k;
                self.hot_tips.push((arm.x as i32, arm.y as i32, b));
            }
        }
        for ai in self.dead.drain(..).rev() {
            self.arms.swap_remove(ai);
        }
        self.arms.append(&mut self.branches);
        self.arms.truncate(MAX_ARMS);
    }
}

/// Paint one growth pixel into the field, tracking coverage and facet.
/// Free function so `grow` can hold disjoint borrows while iterating arms.
fn paint_cell(
    field: &mut [f32],
    facet: &mut [f32],
    filled: &mut Vec<usize>,
    w: usize,
    h: usize,
    x: f32,
    y: f32,
    v: f32,
    f: f32,
) {
    let (xi, yi) = (x as i32, y as i32);
    if xi < 0 || yi < 0 || xi >= w as i32 || yi >= h as i32 {
        return;
    }
    let i = yi as usize * w + xi as usize;
    if field[i] < 0.02 {
        filled.push(i);
    }
    if v > field[i] {
        field[i] = v;
        facet[i] = f;
    }
}

impl Scene for Frost {
    fn name(&self) -> &'static str {
        "frost"
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

        // --- growth lifecycle: grow -> melt -> rest -> reseed ---
        let budget = ((w * h) / 2).max(1); // heavy = ~40% of the budget
        match self.phase {
            Phase::Grow => {
                self.grow(dt);
                if self.filled.len() as f32 > budget as f32 * 0.40 {
                    self.phase = Phase::Melt(0.0);
                    self.arms.clear();
                }
            }
            Phase::Melt(e) => {
                let e = e + dt;
                if e >= MELT_DUR {
                    self.field.fill(0.0);
                    self.filled.clear();
                    self.glints.clear();
                    self.rings.clear();
                    self.hot_tips.clear();
                    self.rest_dur = self.rng.random_range(1.2..2.6);
                    self.phase = Phase::Rest(0.0);
                } else {
                    self.phase = Phase::Melt(e);
                }
            }
            Phase::Rest(e) => {
                let e = e + dt;
                if e >= self.rest_dur {
                    self.spawn_seeds();
                    self.phase = Phase::Grow;
                } else {
                    self.phase = Phase::Rest(e);
                }
            }
        }
        let growing = matches!(self.phase, Phase::Grow);

        // melt fade envelope: slow, eased, not a hard cut
        let fade = match self.phase {
            Phase::Melt(e) => 1.0 - ease_smooth(e / MELT_DUR),
            Phase::Rest(_) => 0.0,
            Phase::Grow => 1.0,
        };

        // --- nucleation bursts (anticipation -> flash/star -> settle) ---
        self.next_burst -= dt;
        if self.next_burst <= 0.0 {
            self.next_burst = self.rng.random_range(9.0..16.0);
            if growing {
                self.nucleate();
            }
        }

        // --- breath-fog: a warm exhale fogs the pane edges, then clears ---
        self.next_fog -= dt;
        if self.next_fog <= 0.0 && self.fog_t < 0.0 {
            self.fog_t = 0.0;
            self.fog_dur = self.rng.random_range(7.0..10.0);
            self.next_fog = self.rng.random_range(14.0..22.0);
            let a = self.rng.random_range(0.0..TAU);
            self.fog_dir = (a.cos(), a.sin());
        }
        let mut fog_env = 0.0;
        if self.fog_t >= 0.0 {
            self.fog_t += dt;
            if self.fog_t >= self.fog_dur {
                self.fog_t = -1.0;
            } else {
                // ease in over 2.5s, hold, clear over the last 3s
                fog_env = ease_smooth(self.fog_t / 2.5)
                    * (1.0 - ease_smooth((self.fog_t - (self.fog_dur - 3.0)) / 3.0));
            }
        }

        // --- glints: occasional sparkles on random crystal pixels ---
        self.next_glint -= dt;
        if self.next_glint <= 0.0 {
            self.next_glint = self.rng.random_range(0.10..0.35) / density_for(w, h);
            if growing && !self.filled.is_empty() && self.glints.len() < MAX_GLINTS {
                let i = self.filled[self.rng.random_range(0..self.filled.len())];
                if self.field[i] > 0.30 {
                    self.glints.push(Glint {
                        x: (i % w) as i32,
                        y: (i / w) as i32,
                        age: 0.0,
                        dur: self.rng.random_range(0.3..0.7),
                    });
                }
            }
        }

        // --- draw: pure black glass ---
        canvas.clear((0, 0, 0));

        // slow light band sweeping the pane; facets flash as it crosses them.
        // the band runs along fx*0.7+fy*0.7, whose range is 0..span*0.7 —
        // sweep within that so the band is only briefly off-pane
        let span = (w + h) as f32;
        let band_max = span * 0.7;
        let light_p = band_max * (0.5 + 0.6 * (t * 0.21).sin());
        let band_w = span * 0.16 + 6.0;
        let shimmer_phase = t * 1.4;

        // active shock rings rim-light the crystals they sweep over:
        // (x, y, radius, strength) per ring, read-only during the pixel loop
        let mut ring_fx = [(0.0f32, 0.0f32, 0.0f32, 0.0f32); MAX_RINGS];
        let mut ring_n = 0;
        for r in &self.rings {
            let p = (r.age / r.dur).min(1.0);
            ring_fx[ring_n] = (r.x, r.y, ease_out(p) * 6.0, (1.0 - p).powf(1.5));
            ring_n += 1;
        }
        let ring_fx = &ring_fx[..ring_n];

        // fog geometry: strongest at the edges, waving slowly inward
        let margin = (w.min(h) as f32 * 0.30).max(3.0);
        let (fdx, fdy) = self.fog_dir;

        for y in 0..h {
            let ey = (y.min(h - 1 - y) as f32 / margin).min(1.0);
            for x in 0..w {
                let i = y * w + x;
                let (fx, fy) = (x as f32, y as f32);

                // breath-fog strength at this cell
                let mut fog = 0.0f32;
                if fog_env > 0.0 {
                    let ex = (x.min(w - 1 - x) as f32 / margin).min(1.0);
                    let edge = 1.0 - ease_smooth(ex.min(ey));
                    let wave = 0.5
                        + 0.5 * ((fx * fdx + fy * fdy) * 0.10 - t * 0.9).sin();
                    fog = fog_env * (0.25 + 0.75 * edge) * (0.55 + 0.45 * wave);
                }

                // crystal with facet shimmer and fog dimming
                let v = self.field[i];
                if v > 0.004 {
                    let band = fx * 0.7 + fy * 0.7; // band runs diagonally
                    let d = (band - light_p) / band_w;
                    let lb = (-d * d).exp();
                    let sh = (0.5 + 0.5 * (TAU * self.facet[i] * 3.0 - shimmer_phase).cos())
                        .powi(3);
                    let shim = sh * (0.25 + 0.75 * lb) * (v.min(1.0));
                    // shock-ring passage flashes the crystal it crosses
                    let mut rim = 0.0f32;
                    for &(rx, ry, rad, alpha) in ring_fx {
                        let dx = fx - rx;
                        let dy = (fy - ry) * 1.8; // matches the squashed ring
                        let dd = (dx * dx + dy * dy).sqrt() - rad;
                        rim += alpha * (-dd * dd * 0.8).exp();
                    }
                    let lum = 1.0 - 0.62 * fog; // fog dims the crystal beneath
                    let vv = (v * fade * lum * (1.0 + 0.50 * shim + rim * 0.9)).clamp(0.0, 1.15);
                    let mut shade =
                        lerp(self.base_c, self.tip_c, (vv * 0.7).clamp(0.0, 1.0));
                    // shimmer and ring light whiten the lit facets
                    shade = lerp(shade, self.glint_c, (shim * 0.4 + rim * 0.35).min(0.5));
                    canvas.set(x as i32, y as i32, scale(shade, vv.min(1.0)));
                }

                // far haze plane + milky fog veil (kept very dark)
                let hz = fbm(fx * 0.045 + t * 0.02, fy * 0.05, 2, self.haze_seed);
                let amb = hz * 0.045 + fog * 0.075;
                if amb > 0.002 {
                    canvas.add(x as i32, y as i32, scale(self.fog_c, amb));
                }
            }
        }

        // glowing growth tips sit on top of the crystal
        for &(tx, ty, b) in &self.hot_tips {
            canvas.add(tx, ty, scale(self.tip_c, b * fade));
        }

        // nucleation shock rings
        let base_c = self.base_c;
        self.rings.retain_mut(|r| {
            r.age += dt;
            if r.age >= r.dur {
                return false;
            }
            let p = r.age / r.dur;
            let rad = ease_out(p) * 6.0;
            let alpha = (1.0 - p).powf(1.5) * 0.5;
            let steps = (rad * 6.0) as i32 + 8;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * TAU;
                let px = r.x + a.cos() * rad;
                let py = r.y + a.sin() * rad * 0.55; // squash for cell aspect
                canvas.add(px as i32, py as i32, scale(base_c, alpha));
            }
            true
        });

        // glints: brief star sparkles with a faint cross bleed
        let glint_c = self.glint_c;
        self.glints.retain_mut(|g| {
            g.age += dt;
            if g.age >= g.dur {
                return false;
            }
            let a = (PI * g.age / g.dur).sin() * fade;
            canvas.add(g.x, g.y, scale(glint_c, a * 0.9));
            canvas.add(g.x + 1, g.y, scale(glint_c, a * 0.3));
            canvas.add(g.x - 1, g.y, scale(glint_c, a * 0.3));
            canvas.add(g.x, g.y + 1, scale(glint_c, a * 0.3));
            canvas.add(g.x, g.y - 1, scale(glint_c, a * 0.3));
            true
        });
    }
}
