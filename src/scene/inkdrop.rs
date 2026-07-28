//! Inkdrop: ink blooming through still black water. Drops strike the
//! surface and bloom into soft clouds on a half-res density field, advected
//! by a slow fbm flow with swirling vortices around each young bloom —
//! two-tone render: dense core color, wispy brighter rim modulated by
//! tendril noise. Older blooms dim and diffuse as the field fades. Every
//! ~20-30s a DOUBLE DROP: a ripple ring announces it, two blooms land side
//! by side with a shockwave ring rolling out, their vortices intertwine the
//! merging clouds, then diffusion dissolves them back to black.

use super::noise::{fbm, vnoise};
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_out, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::TAU;

/// Cap on live blooms so long fast-forwards can't grow the vec unbounded.
const MAX_BLOOMS: usize = 24;
const MAX_RINGS: usize = 12;
/// Density ceiling: keeps additive injection from saturating the field.
const DENS_MAX: f32 = 2.5;

/// Depth planes: (brightness, radius scale). Far blooms are dim and small,
/// near blooms big and dense — overlapping ages read as layered water.
const LAYERS: [(f32, f32); 3] = [(0.45, 0.7), (0.7, 1.0), (1.0, 1.35)];

struct Bloom {
    x: f32, // grid coords
    y: f32,
    age: f32,
    life: f32,
    max_r: f32,
    swirl: f32, // signed vortex strength
    bright: f32,
}

struct Ring {
    x: f32, // canvas coords
    y: f32,
    age: f32,
    dur: f32,
    max_r: f32,
    bright: f32,
}

struct Flash {
    x: f32, // canvas coords
    y: f32,
    age: f32,
    big: bool,
}

pub struct Inkdrop {
    rng: StdRng,
    detail: Detail,
    core: (u8, u8, u8),
    rim: (u8, u8, u8),
    ring_c: (u8, u8, u8),
    seed: u32,
    dens: Vec<f32>,
    scratch: Vec<f32>,
    gw: usize,
    gh: usize,
    blooms: Vec<Bloom>,
    rings: Vec<Ring>,
    flashes: Vec<Flash>,
    drop_timer: f32,
    next_double: f32,
    /// pending double drop: (grid x, grid y, anticipation countdown)
    pending: Option<(f32, f32, f32)>,
    t: f32,
    w: usize,
    h: usize,
}

impl Inkdrop {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (core, rim) = match theme {
            Some("crimson") => ((150, 22, 44), (255, 110, 125)),
            Some("sepia") => ((128, 88, 40), (235, 192, 130)),
            _ => ((62, 44, 168), (158, 128, 255)), // indigo
        };
        Inkdrop {
            seed: rng.random::<u32>(),
            rng,
            detail,
            core,
            rim,
            ring_c: lerp(rim, (255, 255, 255), 0.45),
            dens: Vec::new(),
            scratch: Vec::new(),
            gw: 0,
            gh: 0,
            blooms: Vec::new(),
            rings: Vec::new(),
            flashes: Vec::new(),
            drop_timer: 1.0,
            next_double: 12.0,
            pending: None,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.gw = (w / 2).max(1);
        self.gh = (h / 2).max(1);
        self.dens = vec![0.0; self.gw * self.gh];
        self.scratch = vec![0.0; self.gw * self.gh];
        self.blooms.clear();
        self.rings.clear();
        self.flashes.clear();
        self.pending = None;
        // pre-seed a few part-grown blooms so the scene reads from frame one
        let (gw, gh) = (self.gw, self.gh);
        let n = (gw * gh / 400).clamp(1, 3);
        for _ in 0..n {
            let x = self.rng.random_range(gw as f32 * 0.15..gw as f32 * 0.85);
            let y = self.rng.random_range(gh as f32 * 0.15..gh as f32 * 0.85);
            let age = self.rng.random_range(0.6..2.6);
            self.spawn_bloom(x, y, false);
            let b = self.blooms.last_mut().unwrap();
            b.age = age;
            // backfill the ink it would have injected by now (minus fade)
            let grow = ease_smooth((age / 1.6).clamp(0.0, 1.0));
            let rad = b.max_r * (0.2 + 0.8 * grow);
            let ri = rad.ceil() as i32;
            let (bx, by) = (b.x as i32, b.y as i32);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / rad.max(0.5);
                    if d < 1.0 {
                        let px = bx + dx;
                        let py = by + dy;
                        if px >= 0 && py >= 0 && (px as usize) < gw && (py as usize) < gh {
                            let prof = ease_smooth(1.0 - d);
                            let idx = py as usize * gw + px as usize;
                            self.dens[idx] =
                                (self.dens[idx] + 4.4 * b.bright * prof * age).min(DENS_MAX);
                        }
                    }
                }
            }
        }
        self.flashes.clear(); // no impact flashes for blooms that landed "off-screen"
    }

    /// Inject a new bloom (grid coords) plus its impact flash.
    fn spawn_bloom(&mut self, x: f32, y: f32, big: bool) {
        let r = self.rng.random::<f32>();
        let depth = if r < 0.4 {
            0 // far
        } else if r < 0.8 {
            1 // mid
        } else {
            2 // near
        };
        let (bright_l, rad_l) = LAYERS[depth];
        let gk = (self.gw.min(self.gh) as f32 / 34.0).clamp(0.45, 2.0);
        let big_k = if big { 1.3 } else { 1.0 };
        let max_r = self.rng.random_range(2.8..5.5) * gk * rad_l * big_k;
        self.blooms.push(Bloom {
            x,
            y,
            age: 0.0,
            life: self.rng.random_range(10.0..18.0),
            max_r,
            swirl: self.rng.random_range(0.7..1.3) * if self.rng.random::<bool>() { 1.0 } else { -1.0 },
            bright: bright_l * self.rng.random_range(0.85..1.1) * if big { 1.15 } else { 1.0 },
        });
        while self.blooms.len() > MAX_BLOOMS {
            self.blooms.remove(0);
        }
        self.flashes.push(Flash {
            x: x * 2.0,
            y: y * 2.0,
            age: 0.0,
            big,
        });
    }

    /// Steady single drop somewhere on the surface.
    fn spawn_single(&mut self) {
        let (gw, gh) = (self.gw as f32, self.gh as f32);
        let x = self.rng.random_range(gw * 0.12..gw * 0.88);
        let y = self.rng.random_range(gh * 0.12..gh * 0.88);
        self.spawn_bloom(x, y, false);
    }
}

impl Scene for Inkdrop {
    fn name(&self) -> &'static str {
        "inkdrop"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        if w == 0 || h == 0 {
            return;
        }
        // cap dt so fast-forward can't blow up advection or ring growth
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let t = self.t;
        let (gw, gh) = (self.gw, self.gh);

        // --- event rhythm: steady drops, double drop every ~20-30s ---
        self.drop_timer -= dt;
        if self.drop_timer <= 0.0 {
            self.spawn_single();
            let interval = self.rng.random_range(3.2..6.0)
                / (density_for(w, h) * self.detail.factor());
            self.drop_timer = interval.clamp(1.0, 7.0);
        }

        self.next_double -= dt;
        if self.next_double <= 0.0 && self.pending.is_none() {
            // anticipation: a ripple ring spreads from the future impact point
            let cx = self.rng.random_range(gw as f32 * 0.25..gw as f32 * 0.75);
            let cy = self.rng.random_range(gh as f32 * 0.25..gh as f32 * 0.75);
            self.pending = Some((cx, cy, 1.15));
            let gk = (gw.min(gh) as f32 / 34.0).clamp(0.45, 2.0);
            self.rings.push(Ring {
                x: cx * 2.0,
                y: cy * 2.0,
                age: 0.0,
                dur: 1.15,
                max_r: 7.0 * gk * 2.0,
                bright: 0.8,
            });
            while self.rings.len() > MAX_RINGS {
                self.rings.remove(0);
            }
            self.next_double = self.rng.random_range(20.0..30.0);
        }
        if let Some((cx, cy, cd)) = &mut self.pending {
            *cd -= dt;
            if *cd <= 0.0 {
                // payoff: twin drops land side by side, vortices counter-rotate
                let ang = self.rng.random_range(0.0..TAU);
                let gap = self.rng.random_range(2.2..3.8)
                    * (gw.min(gh) as f32 / 34.0).clamp(0.45, 2.0);
                let (ox, oy) = (ang.cos() * gap * 0.5, ang.sin() * gap * 0.5);
                let (cx, cy) = (*cx, *cy);
                self.spawn_bloom(
                    (cx - ox).clamp(2.0, gw as f32 - 3.0),
                    (cy - oy).clamp(2.0, gh as f32 - 3.0),
                    true,
                );
                self.spawn_bloom(
                    (cx + ox).clamp(2.0, gw as f32 - 3.0),
                    (cy + oy).clamp(2.0, gh as f32 - 3.0),
                    true,
                );
                // decay: a slow shockwave ring rolls out from the twin impact
                let gk = (gw.min(gh) as f32 / 34.0).clamp(0.45, 2.0);
                self.rings.push(Ring {
                    x: cx * 2.0,
                    y: cy * 2.0,
                    age: 0.0,
                    dur: 1.8,
                    max_r: 11.0 * gk * 2.0,
                    bright: 0.55,
                });
                while self.rings.len() > MAX_RINGS {
                    self.rings.remove(0);
                }
                self.pending = None;
            }
        }

        // --- simulate: fade + advect (fbm flow + bloom vortices) + diffuse ---
        let fade = (-0.12 * dt).exp(); // older ink sinks back into black slowly, leaving ghost residue
        let diff = (2.0 * dt).min(0.22);
        let seed = self.seed;
        {
            let dens = &self.dens;
            let scratch = &mut self.scratch;
            let blooms = &self.blooms;
            for gy in 0..gh {
                for gx in 0..gw {
                    let i = gy * gw + gx;
                    // slow drifting flow field, anisotropic like still water
                    let ang = fbm(
                        gx as f32 * 0.07 + t * 0.03,
                        gy as f32 * 0.07 - t * 0.02,
                        3,
                        seed,
                    ) * TAU
                        * 1.6;
                    let mut vx = ang.cos() * 1.7;
                    let mut vy = ang.sin() * 1.1;
                    // swirling tendrils: each young bloom stirs the ink around it
                    for b in blooms {
                        let dx = gx as f32 - b.x;
                        let dy = gy as f32 - b.y;
                        let infl = b.max_r * 2.0;
                        let r2 = dx * dx + dy * dy;
                        if r2 < infl * infl {
                            let r = r2.sqrt();
                            let fall = 1.0 - r / infl;
                            let vigor = 1.0 - ease_smooth(b.age / (b.life * 0.6));
                            let s = b.swirl * fall * fall * vigor * 16.0;
                            let inv = 1.0 / (r + 0.8);
                            vx += -dy * inv * s;
                            vy += dx * inv * s;
                        }
                    }
                    // semi-Lagrangian backtrace into the previous field
                    let sx = (gx as f32 - vx * dt).clamp(0.0, gw as f32 - 1.001);
                    let sy = (gy as f32 - vy * dt).clamp(0.0, gh as f32 - 1.001);
                    let (ix, iy) = (sx as usize, sy as usize);
                    let (fx, fy) = (sx - ix as f32, sy - iy as f32);
                    let i00 = iy * gw + ix;
                    let i10 = i00 + 1;
                    let i01 = i00 + gw;
                    let i11 = i01 + 1;
                    let val = dens[i00] * (1.0 - fx) * (1.0 - fy)
                        + dens[i10] * fx * (1.0 - fy)
                        + dens[i01] * (1.0 - fx) * fy
                        + dens[i11] * fx * fy;
                    // gentle 4-neighbor diffusion: blooms soften as they age
                    let n = dens[iy * gw + ix.saturating_sub(1).max(0)];
                    let e = dens[iy * gw + (ix + 1).min(gw - 1)];
                    let s2 = dens[iy.saturating_sub(1) * gw + ix];
                    let w2 = dens[(iy + 1).min(gh - 1) * gw + ix];
                    let avg = (n + e + s2 + w2) * 0.25;
                    scratch[i] = (val * (1.0 - diff) + avg * diff) * fade;
                }
            }
        }
        std::mem::swap(&mut self.dens, &mut self.scratch);

        // --- inject: blooms feed ink while expanding (eased growth) ---
        let bloom_t = 1.6; // expansion phase in seconds
        for b in self.blooms.iter_mut() {
            b.age += dt;
            let grow = ease_smooth((b.age / bloom_t).clamp(0.0, 1.0));
            let rad = b.max_r * (0.2 + 0.8 * grow);
            if b.age < bloom_t * 2.2 {
                let ri = rad.ceil() as i32;
                let (bx, by) = (b.x as i32, b.y as i32);
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        let d = ((dx * dx + dy * dy) as f32).sqrt() / rad.max(0.5);
                        if d < 1.0 {
                            let px = bx + dx;
                            let py = by + dy;
                            if px >= 0 && py >= 0 && (px as usize) < gw && (py as usize) < gh {
                                let prof = ease_smooth(1.0 - d);
                                let idx = py as usize * gw + px as usize;
                                self.dens[idx] =
                                    (self.dens[idx] + 5.5 * b.bright * prof * dt).min(DENS_MAX);
                            }
                        }
                    }
                }
            }
        }
        self.blooms.retain(|b| b.age < b.life);

        // --- draw: pure black water, ink upscaled from the density field ---
        canvas.clear((0, 0, 0));
        let (core, rim) = (self.core, self.rim);
        for y in 0..h {
            for x in 0..w {
                // bilinear upscale from half-res grid
                let gx = (x as f32 * 0.5).min(gw as f32 - 1.001);
                let gy = (y as f32 * 0.5).min(gh as f32 - 1.001);
                let (ix, iy) = (gx as usize, gy as usize);
                let (fx, fy) = (gx - ix as f32, gy - iy as f32);
                let i00 = iy * gw + ix;
                let d = self.dens[i00] * (1.0 - fx) * (1.0 - fy)
                    + self.dens[i00 + 1] * fx * (1.0 - fy)
                    + self.dens[i00 + gw] * (1.0 - fx) * fy
                    + self.dens[i00 + gw + 1] * fx * fy;
                if d < 0.012 {
                    continue; // still black water
                }
                // dense core: smooth saturation curve
                let core_a = ease_smooth((d * 1.5).clamp(0.0, 1.0));
                // wispy rim: peaks at moderate density, torn by tendril noise
                let band = ease_smooth(((d - 0.05) / 0.4).clamp(0.0, 1.0))
                    * (1.0 - ease_smooth(((d - 0.6) / 0.9).clamp(0.0, 1.0)));
                let n = vnoise(x as f32 * 0.55 + t * 0.1, y as f32 * 0.55, seed.wrapping_add(7));
                // hot fringe: only the rim's sharpest crests whiten into pops
                let rim_a = band * (0.22 + 0.55 * n);
                let (cr, cg, cb) = scale(core, core_a);
                let crest = band * band * n;
                let (rr, rg, rb) = scale(lerp(rim, (255, 255, 255), crest * 0.45), rim_a);
                canvas.set(
                    x as i32,
                    y as i32,
                    (cr.saturating_add(rr), cg.saturating_add(rg), cb.saturating_add(rb)),
                );
            }
        }

        // impact flashes: bright pinprick that lifts the ink around it
        self.flashes.retain_mut(|f| {
            f.age += dt;
            let life = if f.big { 0.9 } else { 0.45 };
            if f.age >= life {
                return false;
            }
            let a = 1.0 - ease_smooth(f.age / life);
            let fc = lerp(rim, (255, 255, 255), 0.55);
            let rad = if f.big { 8 } else { 4 };
            glow(canvas, f.x as i32, f.y as i32, rad, fc, a * 0.85);
            // white-hot center while the flash is young
            if f.age < life * 0.4 {
                canvas.set(f.x as i32, f.y as i32, lerp(fc, (255, 255, 255), 0.6));
            }
            true
        });

        // ripple rings: eased expansion, additive so they brighten ink below
        let ring_c = self.ring_c;
        self.rings.retain_mut(|r| {
            r.age += dt;
            let p = (r.age / r.dur).clamp(0.0, 1.0);
            if p >= 1.0 {
                return false;
            }
            let rad = r.max_r * ease_out(p);
            let alpha = (1.0 - p).powf(1.5) * r.bright;
            let ring = lerp(ring_c, (255, 255, 255), 0.3 * (1.0 - p));
            let steps = (rad * 7.0) as i32 + 12;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * TAU;
                let px = r.x + a.cos() * rad;
                let py = r.y + a.sin() * rad * 0.55; // squash for cell aspect
                canvas.add(px as i32, py as i32, scale(ring, alpha * 0.4));
            }
            true
        });
    }
}
