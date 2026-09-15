//! Tree canopy seen from above: branching growth from the center, fork
//! tips bright fresh-green deepening with age, leaves clustered on mature
//! branches, brown at the core. Fills, holds, fades, reseeds. Mid-cycle
//! gusts lean the tips, shed a burst of near-plane leaves, and flash tip
//! highlights; blossom/spring themes get occasional blossom bursts with
//! a soft canopy-wide tint bleed.

use super::{Detail, Scene};
use crate::physics::{self, Forces};
use crate::canvas::{ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

// grid encoding: 0 empty, 1 trunk, 2..=200 branch (depth = v-2), 255 leaf
const TRUNK: u8 = 1;
const LEAF: u8 = 255;
const MAX_DEPTH: u8 = 30;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Growing,
    Hold,
    Autumn,
    Fall,
    Bare,
    Fading,
}

struct LeafP {
    body: physics::Body,
    phase: f32,
    color: (u8, u8, u8),
    /// Near-plane leaves are larger / slower and read above the grid.
    near: bool,
}

struct Tip {
    x: f32,
    y: f32,
    angle: f32,
    depth: u8,
    energy: f32,
}

/// Gust event: lean tips (anticipation) → leaf burst + tip flash (payoff) → settle.
#[derive(Clone, Copy)]
enum Gust {
    Idle,
    Build(f32),
    Burst(f32),
    Settle(f32),
}

pub struct Canopy {
    rng: StdRng,
    detail: Detail,
    leaf_a: (u8, u8, u8),
    leaf_b: (u8, u8, u8),
    blossom_theme: bool,
    spring_theme: bool,
    grid: Vec<u8>,
    tips: Vec<Tip>,
    phase: Phase,
    timer: f32,
    grow_acc: f32,
    falling: Vec<LeafP>,
    generation: usize,
    lean: f32,
    gust: Gust,
    next_gust: f32,
    tip_flash: f32,
    blossom_tint: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Canopy {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (leaf_a, leaf_b) = match theme {
            Some("deep-green") => ((40, 120, 40), (90, 170, 60)),
            Some("mono") => ((120, 130, 120), (190, 200, 190)),
            Some("blossom") => ((150, 190, 90), (250, 190, 205)),
            _ => ((96, 185, 70), (150, 225, 95)), // spring
        };
        let blossom_theme = matches!(theme, Some("blossom"));
        let spring_theme = theme.is_none() || matches!(theme, Some("spring"));
        Canopy {
            rng,
            detail,
            leaf_a,
            leaf_b,
            blossom_theme,
            spring_theme,
            grid: Vec::new(),
            tips: Vec::new(),
            phase: Phase::Growing,
            timer: 0.0,
            grow_acc: 0.0,
            falling: Vec::new(),
            generation: 0,
            lean: 0.0,
            gust: Gust::Idle,
            next_gust: 10.0,
            tip_flash: 0.0,
            blossom_tint: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn spawn_falling(&mut self, x: f32, y: f32, color: (u8, u8, u8), near: bool) {
        let mut body = physics::Body::new(x, y);
        let (vx, vy) = if near {
            (
                self.rng.random_range(-2.5..2.5),
                self.rng.random_range(1.5..4.0),
            )
        } else {
            (
                self.rng.random_range(-1.0..1.0),
                self.rng.random_range(4.0..9.0),
            )
        };
        body.set_velocity(vx, vy, 1.0 / 30.0);
        self.falling.push(LeafP {
            body,
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            color,
            near,
        });
    }

    fn shed_burst(&mut self, count: usize, blossom: bool) {
        let (w, _h) = (self.w, self.h);
        let mut shed = 0usize;
        let want = (count as f32 * self.detail.density()) as usize;
        for i in 0..self.grid.len() {
            if shed >= want {
                break;
            }
            if self.grid[i] == LEAF && self.rng.random::<f32>() < 0.35 {
                let (x, y) = (i % w, i / w);
                if !blossom {
                    self.grid[i] = 0;
                }
                let color = if blossom {
                    lerp(
                        (250, 200, 215),
                        (255, 240, 245),
                        self.rng.random::<f32>(),
                    )
                } else {
                    self.autumn_color(x, y)
                };
                let near = self.rng.random::<f32>() < 0.35;
                self.spawn_falling(x as f32, y as f32, color, near);
                shed += 1;
            }
        }
        // tip flash for payoff lighting
        self.tip_flash = 1.0;
        if blossom {
            self.blossom_tint = 1.0;
        }
    }

    fn reseed(&mut self) {
        self.grid.fill(0);
        self.tips.clear();
        self.phase = Phase::Growing;
        self.timer = 0.0;
        self.generation += 1;
        self.lean = self.rng.random_range(0.0..std::f32::consts::TAU);
        let (cx, cy) = (self.w as f32 / 2.0, self.h as f32 / 2.0);
        let reach = (self.w.min(self.h) as f32) * 0.48;
        // 7-10 primary limbs fanning out from the core, biased toward `lean`
        let limbs = self.detail.scale(self.rng.random_range(7..=10) as f32, 4);
        for i in 0..limbs {
            let base = std::f32::consts::TAU * i as f32 / limbs as f32;
            let angle = base + self.lean * 0.15 + self.rng.random_range(-0.3..0.3);
            self.tips.push(Tip {
                x: cx,
                y: cy,
                angle,
                depth: 0,
                energy: reach * self.rng.random_range(0.9..1.25),
            });
        }
        // trunk core
        for dy in -2..=2 {
            for dx in -2..=2 {
                if dx * dx + dy * dy <= 5 {
                    self.set(cx as i32 + dx, cy as i32 + dy, TRUNK);
                }
            }
        }
    }

    fn set(&mut self, x: i32, y: i32, v: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.grid[y as usize * self.w + x as usize] = v;
        }
    }

    /// One growth tick across all tips.
    fn grow_tick(&mut self) {
        let (w, h) = (self.w as f32, self.h as f32);
        let mut new_tips: Vec<Tip> = Vec::new();
        let rng = &mut self.rng;
        let grid_set = |grid: &mut Vec<u8>, gw: usize, gh: usize, x: i32, y: i32, v: u8| {
            if x >= 0 && y >= 0 && (x as usize) < gw && (y as usize) < gh {
                grid[y as usize * gw + x as usize] = v;
            }
        };
        let (gw, gh) = (self.w, self.h);
        let grid = &mut self.grid;
        self.tips.retain_mut(|tip| {
            // meander
            tip.angle += rng.random_range(-0.25..0.25);
            tip.x += tip.angle.cos();
            tip.y += tip.angle.sin();
            tip.energy -= 1.0;

            if tip.x < 0.0 || tip.y < 0.0 || tip.x >= w || tip.y >= h || tip.energy <= 0.0 {
                // branch ends: sprout a leaf cluster at the tip
                sprout_leaves(rng, grid, gw, gh, tip.x as i32, tip.y as i32);
                return false;
            }

            let (tx, ty) = (tip.x as i32, tip.y as i32);
            grid_set(grid, gw, gh, tx, ty, 2 + tip.depth.min(198));
            // occasional thickness on young growth
            if tip.depth < 4 && rng.random::<f32>() < 0.5 {
                grid_set(grid, gw, gh, tx + 1, ty, 2 + tip.depth.min(198));
            }
            // leaves sprout along mature branches
            if tip.depth >= 3 && rng.random::<f32>() < 0.10 {
                sprout_leaves(rng, grid, gw, gh, tx, ty);
            }
            // fork: denser tertiary branching in the mid-depths
            let fork_p = match tip.depth {
                0..=1 => 0.05,
                2..=5 => 0.11,
                _ => 0.06,
            };
            if tip.depth < MAX_DEPTH
                && new_tips.len() + 1 < 900
                && rng.random::<f32>() < fork_p
            {
                let spread = rng.random_range(0.35..0.8);
                new_tips.push(Tip {
                    x: tip.x,
                    y: tip.y,
                    angle: tip.angle + spread * if rng.random::<bool>() { 1.0 } else { -1.0 },
                    depth: tip.depth + 1,
                    energy: tip.energy * rng.random_range(0.55..0.8),
                });
                tip.depth += 1;
                tip.energy *= 0.9;
            }
            true
        });
        self.tips.append(&mut new_tips);
    }

    /// Interior leaf fill: branch cells sprout leaves into adjacent gaps,
    /// turning the skeleton into a dense mass with holes.
    fn interior_fill(&mut self) {
        let (w, h) = (self.w, self.h);
        let mut fills = Vec::new();
        for i in 0..self.grid.len() {
            let v = self.grid[i];
            if (2..=200).contains(&v) {
                let (x, y) = (i % w, i / w);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        let j = ny as usize * w + nx as usize;
                        if self.grid[j] == 0 && self.rng.random::<f32>() < 0.06 {
                            fills.push(j);
                        }
                    }
                }
            }
        }
        for j in fills {
            self.grid[j] = LEAF;
        }
    }

    fn advance(&mut self, dt: f32) {
        match self.phase {
            Phase::Growing => {
                self.grow_acc += dt * 34.0; // growth ticks per second
                while self.grow_acc >= 1.0 {
                    self.grow_acc -= 1.0;
                    self.grow_tick();
                }
                if self.tips.is_empty() {
                    self.phase = Phase::Hold;
                    self.timer = 0.0;
                    // one dense fill pass as the canopy matures
                    for _ in 0..3 {
                        self.interior_fill();
                    }
                }
            }
            Phase::Hold => {
                self.timer += dt;
                if self.timer > 4.0 {
                    self.phase = Phase::Autumn;
                    self.timer = 0.0;
                }
            }
            Phase::Autumn => {
                self.timer += dt;
                if self.timer > 6.0 {
                    self.phase = Phase::Fall;
                    self.timer = 0.0;
                }
            }
            Phase::Fall => {
                self.timer += dt;
                // leaves detach and drift down over ~5s
                let detach = (self.grid.len() as f32 * dt / 5.0) as usize + 1;
                let mut detached = 0;
                for i in 0..self.grid.len() {
                    if detached >= detach {
                        break;
                    }
                    if self.grid[i] == LEAF && self.rng.random::<f32>() < 0.5 {
                        self.grid[i] = 0;
                        let (x, y) = (i % self.w, i / self.w);
                        let mut body = physics::Body::new(x as f32, y as f32);
                        body.set_velocity(
                            self.rng.random_range(-1.0..1.0),
                            self.rng.random_range(4.0..9.0),
                            1.0 / 30.0,
                        );
                        self.falling.push(LeafP {
                            body,
                            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                            color: self.autumn_color(x, y),
                            near: false,
                        });
                        detached += 1;
                    }
                }
                let leaves_left = self.grid.contains(&LEAF);
                if !leaves_left && self.timer > 5.0 {
                    self.phase = Phase::Bare;
                    self.timer = 0.0;
                }
            }
            Phase::Bare => {
                self.timer += dt;
                if self.timer > 3.0 {
                    self.phase = Phase::Fading;
                    self.timer = 0.0;
                }
            }
            Phase::Fading => {
                self.timer += dt;
                if self.timer > 2.0 {
                    self.reseed();
                }
            }
        }
    }

    /// Leaf color by season: summer green → amber → red.
    fn autumn_color(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let n = ((x * 7349 + y * 15139) % 100) as f32 / 100.0;
        let summer = lerp(self.leaf_a, self.leaf_b, n);
        match self.phase {
            Phase::Autumn => {
                // patchy turn: each leaf flips on its own schedule, so the
                // color wave reads as spots spreading across the canopy
                let k = (self.timer / 6.0 * 1.6 - n * 0.6).clamp(0.0, 1.0);
                let amber = lerp(summer, (225, 160, 45), (k * 2.0).min(1.0));
                lerp(amber, (200, 60, 30), (k * 2.0 - 1.0).max(0.0))
            }
            Phase::Fall | Phase::Bare | Phase::Fading => lerp((225, 160, 45), (200, 60, 30), n),
            _ => summer,
        }
    }

    /// Fraction of the canvas covered (0..1).
    #[cfg(test)]
    fn coverage(&self) -> f32 {
        let filled = self.grid.iter().filter(|&&v| v != 0).count();
        filled as f32 / (self.w * self.h).max(1) as f32
    }
}

fn sprout_leaves(rng: &mut StdRng, grid: &mut [u8], w: usize, h: usize, cx: i32, cy: i32) {
    for dy in -2..=2 {
        for dx in -2..=2 {
            let d2 = dx * dx + dy * dy;
            if d2 <= 5 && rng.random::<f32>() < if d2 == 0 { 0.9 } else { 0.55 } {
                let (x, y) = (cx + dx, cy + dy);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                    let i = y as usize * w + x as usize;
                    if grid[i] == 0 {
                        grid[i] = LEAF;
                    }
                }
            }
        }
    }
}

impl Scene for Canopy {
    fn name(&self) -> &'static str {
        "canopy"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.grid = vec![0; w * h];
            self.reseed();
        }
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let t = self.t;
        self.advance(dt);
        self.tip_flash = (self.tip_flash - dt * 1.2).max(0.0);
        self.blossom_tint = (self.blossom_tint - dt * 0.55).max(0.0);

        // --- gust / blossom event (during Growing/Hold/Autumn) ---
        let can_gust = matches!(
            self.phase,
            Phase::Growing | Phase::Hold | Phase::Autumn
        );
        match self.gust {
            Gust::Idle if can_gust => {
                self.next_gust -= dt;
                if self.next_gust <= 0.0 {
                    self.gust = Gust::Build(0.0);
                }
            }
            Gust::Build(e) => {
                let e = e + dt;
                // lean tips toward wind during anticipation
                for tip in &mut self.tips {
                    tip.angle += 0.35 * dt * (1.0 + e);
                }
                if e >= 1.6 {
                    let blossom = self.blossom_theme
                        || (self.spring_theme && self.rng.random::<f32>() < 0.45);
                    self.shed_burst(self.detail.scale(18.0, 8), blossom);
                    self.gust = Gust::Burst(0.0);
                } else {
                    self.gust = Gust::Build(e);
                }
            }
            Gust::Burst(e) => {
                let e = e + dt;
                if e >= 0.8 {
                    self.gust = Gust::Settle(0.0);
                } else {
                    self.gust = Gust::Burst(e);
                }
            }
            Gust::Settle(e) => {
                let e = e + dt;
                if e >= 2.2 {
                    self.gust = Gust::Idle;
                    self.next_gust = self.rng.random_range(18.0..28.0);
                } else {
                    self.gust = Gust::Settle(e);
                }
            }
            _ => {}
        }
        let gust_k = match self.gust {
            Gust::Build(e) => ease_smooth(e / 1.6) * 0.5,
            Gust::Burst(e) => 1.0 - e / 0.8 * 0.3,
            Gust::Settle(e) => (1.0 - ease_smooth(e / 2.2)) * 0.55,
            Gust::Idle => 0.0,
        };

        let fade = if self.phase == Phase::Fading {
            1.0 - (self.timer / 2.0).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // canopy body first — birds, pollen and falling leaves composite on top
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        for y in 0..h {
            for x in 0..w {
                let v = self.grid[y * w + x];
                let mut color = match v {
                    0 => (5, 9, 5),
                    TRUNK => (96, 62, 34),
                    LEAF => {
                        // spring blossom speckle before full leaf-out
                        if self.phase == Phase::Growing
                            && ((x * 733 + y * 1513) % 100) < 8
                        {
                            lerp((250, 200, 215), (245, 175, 200), ((x * 31 + y * 17) % 100) as f32 / 100.0)
                        } else {
                            let c = self.autumn_color(x, y);
                            // sun dapple: leaves shimmer while the canopy is green
                            if matches!(self.phase, Phase::Growing | Phase::Hold) {
                                let n = ((x * 7349 + y * 15139) % 100) as f32 / 100.0;
                                scale(c, 1.0 + 0.16 * (t * 1.3 + n * std::f32::consts::TAU).sin())
                            } else {
                                c
                            }
                        }
                    }
                    b => {
                        let depth = (b - 2) as f32 / MAX_DEPTH as f32;
                        // brown near the core, deepening green outward
                        let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                        if dist < 4.0 {
                            lerp((96, 62, 34), (60, 90, 40), dist / 4.0)
                        } else {
                            lerp((150, 232, 92), (16, 66, 22), depth)
                        }
                    }
                };
                // blossom burst tint bleed across the canopy
                if self.blossom_tint > 0.02 && v != 0 {
                    color = lerp(color, (255, 210, 230), self.blossom_tint * 0.22);
                }
                // gust tip flash: brighten outer leaves
                if self.tip_flash > 0.02 && v == LEAF {
                    let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                    let outer = (dist / (w.min(h) as f32 * 0.35)).clamp(0.0, 1.0);
                    color = lerp(color, (255, 255, 200), self.tip_flash * 0.35 * outer);
                }
                canvas.set(x as i32, y as i32, scale(color, fade));
            }
        }

        // tip glow during gust payoff
        if self.tip_flash > 0.05 {
            for tip in &self.tips {
                glow(
                    canvas,
                    tip.x as i32,
                    tip.y as i32,
                    2,
                    lerp(self.leaf_b, (255, 255, 220), 0.5),
                    self.tip_flash * 0.45,
                );
            }
        }

        // drifting fallen leaves: gravity + flutter lift, settling out at the ground
        let hf = h as f32;
        self.falling.retain_mut(|l| {
            let flutter = (t * 2.0 + l.phase).sin() * (14.0 + gust_k * 18.0);
            let grav = if l.near { 3.5 } else { 7.0 };
            physics::integrate(
                &mut l.body,
                &Forces {
                    gravity: grav,
                    drag: if l.near { 2.4 } else { 1.8 },
                    wind_x: flutter + gust_k * 22.0 * self.lean.cos(),
                    ay: if l.near { -5.0 } else { -3.0 },
                    ..Default::default()
                },
                dt,
            );
            if l.body.y >= hf - 1.0 {
                return false;
            }
            let ground = ((hf - l.body.y) / 3.0).clamp(0.0, 1.0);
            if l.near {
                glow(
                    canvas,
                    l.body.x as i32,
                    l.body.y as i32,
                    2,
                    l.color,
                    0.35 * ground * fade,
                );
                canvas.set_f(l.body.x, l.body.y, scale(l.color, ground * fade));
            } else {
                canvas.set_f(l.body.x, l.body.y, scale(l.color, ground * fade));
            }
            true
        });

        // ambient: pollen motes drifting through the canopy in spring
        if matches!(self.phase, Phase::Growing | Phase::Hold) {
            for i in 0..8 {
                let fi = i as f32;
                let mx = ((t * 3.0 + fi * 47.0) % (w as f32 + 20.0)) - 10.0;
                let my = (fi * 23.0 + (t * 0.7 + fi).sin() * 8.0 + h as f32 * 0.4)
                    .rem_euclid(h as f32);
                let tw = 0.5 + 0.5 * (t * 2.0 + fi * 1.7).sin();
                canvas.add(mx as i32, my as i32, scale((220, 230, 180), 0.25 * tw * fade));
            }
        }

        // birds: perch on the canopy silhouette, startle off on a slow cycle
        for i in 0..3 {
            let fi = i as f32;
            let bx = w as f32 * (0.3 + fi * 0.2);
            let cycle = (t + fi * 4.0) % 12.0;
            if cycle < 8.0 {
                // perch just above the topmost canopy cell in this column
                let col = (bx as usize).min(w - 1);
                let mut perch = h as f32 * 0.35;
                for y in 0..h {
                    if self.grid[y * w + col] != 0 {
                        perch = y as f32 - 1.0;
                        break;
                    }
                }
                let bob = (t * 2.2 + fi * 1.9).sin() * 0.8;
                canvas.set_f(bx, perch + bob, scale((70, 58, 44), fade));
                canvas.set_f(bx + 1.0, perch + bob - 1.0, scale((70, 58, 44), fade * 0.7));
            } else {
                // startled: eased climb out, then glide away
                let ft = cycle - 8.0;
                let climb = 1.0 - (1.0 - (ft / 1.2).min(1.0)).powi(3);
                let fx = bx + ft * 14.0;
                let fy = h as f32 * 0.35 - climb * 10.0 - (ft * 1.5).min(3.0);
                canvas.set_f(fx, fy, scale((55, 46, 36), fade));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn canopy_grows_and_reseeds() {
        let mut c = Canopy::new(StdRng::seed_from_u64(13), None, Detail::Medium);
        let mut canvas = Canvas::new(50, 50);
        c.update(1.0 / 30.0, &mut canvas);
        let gen0 = c.generation;
        // run until a reseed happens (generous cap)
        let mut frames = 0;
        while c.generation == gen0 && frames < 60_000 {
            c.update(1.0 / 30.0, &mut canvas);
            frames += 1;
        }
        assert!(c.generation > gen0, "canopy should complete and reseed");
        assert!(frames < 60_000, "reseed took too long");
    }

    #[test]
    fn growth_stays_in_bounds_and_covers() {
        let mut c = Canopy::new(StdRng::seed_from_u64(14), None, Detail::Medium);
        let mut canvas = Canvas::new(60, 40);
        for _ in 0..3000 {
            c.update(1.0 / 30.0, &mut canvas);
            if c.phase == Phase::Hold {
                break;
            }
        }
        // internal grid is exactly w*h so all writes are in bounds by construction
        assert_eq!(c.grid.len(), 60 * 40);
        let cov = c.coverage();
        assert!(cov > 0.12, "canopy should fill a good share, got {cov:.2}");
    }
}
