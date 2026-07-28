//! Nebula: deep-space clouds drifting in parallax layers. Three strata of
//! domain-warped fbm (far = broad/slow/dim, near = fine/fast/bright) drift in
//! different directions over pure black; density maps through a theme ramp
//! (deep base -> mid glow -> hot cores). A faint far starfield sits behind,
//! sparse sharp glints twinkle in front. Every ~20-30s a KNOT FLARE: the
//! densest mid-layer region condenses (anticipation), ignites into a bright
//! forming-star flash that lights nearby wisps, stars and glints and blows
//! off an easing shockfront ring (payoff), then relaxes (decay).

use super::noise::fbm;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_out, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

/// Per-layer fbm tuning: spatial freq, drift (cells/s), density threshold and
/// softness, brightness gain, domain-warp strength, noise seed, fbm octaves.
/// The far layer drops to 3 octaves: its top octave sat below the density
/// grid's Nyquist limit and 3-octave fbm has nearly identical threshold
/// statistics. Mid/near keep 4 — their texture is the visible one.
struct Layer {
    freq: f32,
    dx: f32,
    dy: f32,
    thresh: f32,
    soft: f32,
    gain: f32,
    warp: f32,
    seed: u32,
    oct: usize,
}

/// Density/warp grid stride in px. Cloud fbm is evaluated on this coarse grid
/// and bilinearly upsampled; the broad cloud forms survive easily.
const STEP: usize = 3;

const LAYERS: [Layer; 3] = [
    // far: broad, slow, faint
    Layer { freq: 0.040, dx: 1.6, dy: 0.5, thresh: 0.60, soft: 0.22, gain: 0.50, warp: 2.0, seed: 11, oct: 3 },
    // mid: carries the knot-flare peak scan
    Layer { freq: 0.070, dx: -2.6, dy: 0.9, thresh: 0.63, soft: 0.20, gain: 0.70, warp: 2.6, seed: 47, oct: 4 },
    // near: fine, fast, brightest
    Layer { freq: 0.115, dx: 4.2, dy: -1.6, thresh: 0.66, soft: 0.18, gain: 0.95, warp: 3.2, seed: 83, oct: 4 },
];

const COMPRESS_T: f32 = 1.6;
const IGNITE_T: f32 = 0.9;
const DECAY_T: f32 = 3.4;

/// Knot-flare rhythm: Idle -> Compress (region pulls in) -> Ignite (flash)
/// -> Decay (afterglow) -> Idle.
#[derive(Clone, Copy, PartialEq)]
enum Flare {
    Idle(f32), // seconds until next flare
    Compress(f32),
    Ignite(f32),
    Decay(f32),
}

struct Star {
    x: f32, // 0..1 normalized, survives resizes
    y: f32,
    phase: f32,
    freq: f32,
    mag: f32,
}

pub struct Nebula {
    rng: StdRng,
    detail: Detail,
    base: (u8, u8, u8),
    mid: (u8, u8, u8),
    core: (u8, u8, u8),
    star_c: (u8, u8, u8),
    glint_c: (u8, u8, u8),
    seed: u32,
    far: Vec<Star>,
    glints: Vec<Star>,
    flare: Flare,
    fx: f32, // flare center (px)
    fy: f32,
    peak: (f32, f32, f32), // densest mid-layer point last frame (x, y, k)
    warp_x: Vec<f32>,      // coarse-grid domain-warp fields
    warp_y: Vec<f32>,
    dens: Vec<f32>,  // coarse-grid density scratch, reused per layer
    pow_lut: Vec<f32>, // k^1.6 gain shaping, 257 entries (k in 0..=1)
    /// Prebaked per-layer k→color ramp (257 entries each), valid when no
    /// flare is active: with `den = thresh + k*soft`, the whole ramp chain
    /// (body lerp, hot-core lerp, gain shaping) collapses to a function of k.
    col_lut: Vec<(u8, u8, u8)>,
    t: f32,
    w: usize,
    h: usize,
}

impl Nebula {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (base, mid, core, star_c, glint_c) = match theme {
            Some("crimson") => (
                (44, 8, 18),
                (158, 34, 62),
                (255, 176, 116),
                (255, 215, 200),
                (255, 236, 224),
            ),
            Some("void") => (
                (16, 8, 34),
                (74, 42, 128),
                (206, 168, 255),
                (190, 180, 225),
                (232, 222, 255),
            ),
            _ => (
                // emission: teal/blue wisps with warm cores
                (10, 26, 52),
                (36, 128, 158),
                (255, 196, 130),
                (200, 220, 255),
                (238, 244, 255),
            ),
        };
        // constant per-layer k→color ramps; built once (see col_lut field)
        let mut col_lut = vec![(0u8, 0u8, 0u8); LAYERS.len() * 257];
        for (li, l) in LAYERS.iter().enumerate() {
            for qi in 0..=256 {
                let k = qi as f32 / 256.0;
                let body = lerp(base, mid, ease_smooth(k));
                let hot = ease_smooth(((k - 0.55) / 0.45).clamp(0.0, 1.0));
                let col = lerp(body, core, hot * 0.9);
                col_lut[li * 257 + qi] = scale(col, l.gain * k.powf(1.6));
            }
        }
        Nebula {
            seed: rng.random::<u32>(),
            rng,
            detail,
            base,
            mid,
            core,
            star_c,
            glint_c,
            far: Vec::new(),
            glints: Vec::new(),
            flare: Flare::Idle(0.0),
            fx: 0.0,
            fy: 0.0,
            peak: (0.0, 0.0, 0.0),
            warp_x: Vec::new(),
            warp_y: Vec::new(),
            dens: Vec::new(),
            // constant gain-shaping curve; built once
            pow_lut: (0..=256).map(|i| (i as f32 / 256.0).powf(1.6)).collect(),
            col_lut,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// Flare brightness envelope 0..1: dim gathering, fast ignition, long decay.
    fn flare_env(&self) -> f32 {
        match self.flare {
            Flare::Idle(_) => 0.0,
            Flare::Compress(e) => 0.30 * ease_smooth(e / COMPRESS_T),
            Flare::Ignite(e) => 0.30 + 0.70 * ease_out(e / 0.22),
            Flare::Decay(e) => 1.0 - ease_smooth(e / DECAY_T),
        }
    }

    /// Density pull toward the knot while it condenses (anticipation).
    fn flare_pull(&self) -> f32 {
        match self.flare {
            Flare::Compress(e) => 0.22 * ease_smooth(e / COMPRESS_T),
            Flare::Ignite(e) => 0.22 * (1.0 - ease_smooth(e / IGNITE_T)),
            _ => 0.0,
        }
    }
}

impl Scene for Nebula {
    fn name(&self) -> &'static str {
        "nebula"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;

        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            let gw = w / STEP + 2;
            let gh = h / STEP + 2;
            let wgw = w / (STEP * 2) + 3;
            let wgh = h / (STEP * 2) + 3;
            self.warp_x = vec![0.0; wgw * wgh];
            self.warp_y = vec![0.0; wgw * wgh];
            self.dens = vec![0.0; gw * gh];
            let mk = |rng: &mut StdRng, bright: f32| Star {
                x: rng.random::<f32>(),
                y: rng.random::<f32>(),
                phase: rng.random_range(0.0..std::f32::consts::TAU),
                freq: rng.random_range(0.4..1.6),
                mag: rng.random_range(bright * 0.5..bright),
            };
            let nf = self
                .detail
                .scale((w * h / 350) as f32 * density_for(w, h), 24)
                .min(500);
            self.far = (0..nf).map(|_| mk(&mut self.rng, 0.45)).collect();
            let ng = self
                .detail
                .scale((w * h / 2200) as f32 * density_for(w, h), 5)
                .min(90);
            self.glints = (0..ng).map(|_| mk(&mut self.rng, 1.0)).collect();
            self.flare = Flare::Idle(self.rng.random_range(10.0..16.0));
        }

        // --- knot-flare state machine (anticipation -> payoff -> decay) ---
        self.flare = match self.flare {
            Flare::Idle(rem) => {
                let rem = rem - dt;
                if rem <= 0.0 {
                    // flare where the mid layer is densest right now
                    let (px, py, pk) = self.peak;
                    if pk > 0.25 {
                        self.fx = px;
                        self.fy = py;
                    } else {
                        self.fx = self.rng.random_range(w as f32 * 0.25..w as f32 * 0.75);
                        self.fy = self.rng.random_range(h as f32 * 0.25..h as f32 * 0.75);
                    }
                    Flare::Compress(0.0)
                } else {
                    Flare::Idle(rem)
                }
            }
            Flare::Compress(e) => {
                let e = e + dt;
                if e >= COMPRESS_T {
                    Flare::Ignite(0.0)
                } else {
                    Flare::Compress(e)
                }
            }
            Flare::Ignite(e) => {
                let e = e + dt;
                if e >= IGNITE_T {
                    Flare::Decay(0.0)
                } else {
                    Flare::Ignite(e)
                }
            }
            Flare::Decay(e) => {
                let e = e + dt;
                if e >= DECAY_T {
                    Flare::Idle(self.rng.random_range(20.0..30.0))
                } else {
                    Flare::Decay(e)
                }
            }
        };
        let env = self.flare_env();
        let pull = self.flare_pull();
        // illumination radius for the flare's lighting interplay
        let r_ill = (w.min(h) as f32 * 0.30).clamp(6.0, 24.0);
        let r2_ill = r_ill * r_ill;

        canvas.clear((0, 0, 0));

        // --- faint far starfield behind the clouds, gentle twinkle ---
        // Stars near the knot catch the flash: proximity-boosted while the
        // flare burns, so the event touches the deepest plane too.
        for s in &self.far {
            let (sx, sy) = (s.x * w as f32, s.y * h as f32);
            let tw = 0.6 + 0.4 * (self.t * s.freq + s.phase).sin();
            let mut b = s.mag * tw * 0.30;
            if env > 0.01 {
                let (ddx, ddy) = (sx - self.fx, sy - self.fy);
                b *= 1.0 + env * 1.6 * (-(ddx * ddx + ddy * ddy) / r2_ill).exp();
            }
            canvas.set(sx as i32, sy as i32, scale(self.star_c, b.min(1.0)));
        }

        // --- domain-warp fields, shared by all layers ---
        // The displacement field is smooth (base wavelength ~20px), so it is
        // evaluated on a 2*STEP grid and bilinearly sampled at density-grid
        // points (which land on whole/half warp cells). Same 0.05/px spatial
        // scale as before; two octaves suffice for a displacement field.
        let gw = w / STEP + 2;
        let gh = h / STEP + 2;
        let wgw = w / (STEP * 2) + 3;
        let wgh = h / (STEP * 2) + 3;
        let wscale = (STEP * 2) as f32 * 0.05;
        for gy in 0..wgh {
            let wy = gy as f32 * wscale;
            for gx in 0..wgw {
                let i = gy * wgw + gx;
                let wx = gx as f32 * wscale + self.t * 0.06;
                self.warp_x[i] = fbm(wx, wy, 2, self.seed.wrapping_add(911)) - 0.5;
                self.warp_y[i] = fbm(wx, wy, 2, self.seed.wrapping_add(313)) - 0.5;
            }
        }

        // --- parallax cloud layers, sampled on the coarse grid, upsampled ---
        self.peak = (0.0, 0.0, 0.0);
        for (li, l) in LAYERS.iter().enumerate() {
            let lseed = self.seed.wrapping_add(l.seed);
            let fr = l.freq * STEP as f32;
            // slow current sway: the drift breathes instead of running a
            // perfectly uniform conveyor (amplitude ~ a quarter feature)
            let sway = (self.t * 0.07 + li as f32 * 2.1).sin() * 0.25;
            let sway2 = (self.t * 0.05 + li as f32 * 1.3).cos() * 0.20;
            let tdx = self.t * l.dx * l.freq + sway;
            let tdy = self.t * l.dy * l.freq + sway2;
            for gy in 0..gh {
                // this grid row's position in warp-grid coords (STEP/2 per
                // cell, so the fraction is always 0.0 or 0.5; +1 stays in
                // bounds because wgw/wgh carry a spare cell)
                let wyf = gy as f32 * (STEP as f32 / (STEP * 2) as f32);
                let wyi = wyf as usize;
                let wty = wyf - wyi as f32;
                let wr0 = wyi * wgw;
                let wr1 = wr0 + wgw;
                let ny0 = gy as f32 * fr + tdy;
                for gx in 0..gw {
                    let i = gy * gw + gx;
                    let wxf = gx as f32 * (STEP as f32 / (STEP * 2) as f32);
                    let wxi = wxf as usize;
                    let wtx = wxf - wxi as f32;
                    let j0 = wr0 + wxi;
                    let j1 = wr1 + wxi;
                    let wxv = {
                        let (a, b) = (self.warp_x[j0], self.warp_x[j0 + 1]);
                        let (c, d) = (self.warp_x[j1], self.warp_x[j1 + 1]);
                        a + (b - a) * wtx + (c - a) * wty + (a - b - c + d) * wtx * wty
                    };
                    let wyv = {
                        let (a, b) = (self.warp_y[j0], self.warp_y[j0 + 1]);
                        let (c, d) = (self.warp_y[j1], self.warp_y[j1 + 1]);
                        a + (b - a) * wtx + (c - a) * wty + (a - b - c + d) * wtx * wty
                    };
                    let nx = gx as f32 * fr + tdx + wxv * l.warp;
                    let ny = ny0 + wyv * l.warp;
                    self.dens[i] = fbm(nx, ny, l.oct, lseed);
                }
            }
            let hot0 = l.thresh + l.soft * 0.55;
            let hot_soft = l.soft * 0.45;
            let inv_soft = 1.0 / l.soft;
            let flare_on = env > 0.0 || pull > 0.0;
            // Visit the canvas in STEP×STEP quads aligned to the density
            // grid. Bilinear interpolation never exceeds the four corner
            // values, so a quad whose corners all sit below the threshold
            // (and no flare is adding light) cannot contain a lit pixel —
            // skip it whole. Bounds: ix+1 <= (w-1)/STEP+1 <= gw-1, same in y.
            let mut qy = 0;
            while qy < h {
                let iy = qy / STEP;
                let r0 = iy * gw;
                let r1 = r0 + gw;
                let y_end = (qy + STEP).min(h);
                let mut qx = 0;
                while qx < w {
                    let ix = qx / STEP;
                    let a = self.dens[r0 + ix];
                    let b = self.dens[r0 + ix + 1];
                    let c = self.dens[r1 + ix];
                    let d = self.dens[r1 + ix + 1];
                    let x0 = qx;
                    qx += STEP;
                    if !flare_on && a.max(b).max(c).max(d) <= l.thresh {
                        continue;
                    }
                    for y in qy..y_end {
                        let ty = (y - iy * STEP) as f32 * (1.0 / STEP as f32);
                        let va = a + (c - a) * ty;
                        let vb = b + (d - b) * ty;
                        let fy = y as f32;
                        for x in x0..qx.min(w) {
                            let tx = (x - ix * STEP) as f32 * (1.0 / STEP as f32);
                            let den = va + (vb - va) * tx;

                            if !flare_on {
                                // fast path: whole ramp prebaked in col_lut
                                let kf = (den - l.thresh) * inv_soft;
                                if kf <= 0.0 {
                                    continue;
                                }
                                let k = kf.min(1.0);
                                if li == 1 && k > self.peak.2 {
                                    self.peak = (x as f32, fy, k);
                                }
                                let col = self.col_lut[li * 257 + (k * 256.0) as usize];
                                canvas.add(x as i32, y as i32, col);
                                continue;
                            }

                            // knot-flare interplay: condense + illuminate
                            let ddx = x as f32 - self.fx;
                            let ddy = fy - self.fy;
                            let fall = (-(ddx * ddx + ddy * ddy) / r2_ill).exp();
                            let den = den + pull * fall;
                            let ill = env * fall;

                            let k = ((den - l.thresh) / l.soft).clamp(0.0, 1.0);
                            if k <= 0.0 {
                                continue;
                            }
                            if li == 1 && k > self.peak.2 {
                                self.peak = (x as f32, fy, k);
                            }
                            let body = lerp(self.base, self.mid, ease_smooth(k));
                            let hot = ease_smooth(((den - hot0) / hot_soft).clamp(0.0, 1.0));
                            let mut col = lerp(body, self.core, hot * 0.9);
                            // power-shaped gain: wisp edges fade to black,
                            // only dense knots accumulate real light
                            let g =
                                l.gain * self.pow_lut[(k * 256.0) as usize] * (1.0 + ill * 1.2);
                            col = scale(col, g);
                            canvas.add(x as i32, y as i32, col);
                            if ill > 0.02 {
                                // the flash warms everything it touches
                                canvas.add(x as i32, y as i32, scale(self.core, ill * 0.15 * k));
                            }
                        }
                    }
                }
                qy += STEP;
            }
        }

        // --- flare core: tight hot glow plus a wide faint halo ---
        if env > 0.01 {
            let (ix, iy) = (self.fx as i32, self.fy as i32);
            let rc = (w.min(h) as f32 * 0.14).clamp(2.0, 8.0) as i32;
            glow(canvas, ix, iy, rc, self.core, env * 1.3);
            glow(canvas, ix, iy, (rc * 2).min(12), self.mid, env * 0.30);
            canvas.set(ix, iy, lerp(self.core, (255, 255, 255), env * 0.7));
        }

        // --- ignition shockfront: a faint ring blowing off the flash ---
        // Expands with ease_out from ignition through mid-decay, y-squashed
        // for the ~2:1 cell aspect, fading quadratically as it travels.
        let wave_p = match self.flare {
            Flare::Ignite(e) => Some(e / IGNITE_T * 0.5),
            Flare::Decay(e) => Some((0.5 + e / DECAY_T * 0.5).min(1.0)),
            _ => None,
        };
        if let Some(p) = wave_p {
            let rr = ease_out(p) * r_ill * 1.7;
            let a = (1.0 - p) * (1.0 - p) * 0.45;
            if rr > 1.5 && a > 0.02 {
                let ring_c = lerp(self.mid, self.core, 0.4);
                let steps = (rr * 5.0) as i32 + 12;
                for i in 0..steps {
                    let ang = i as f32 / steps as f32 * std::f32::consts::TAU;
                    canvas.add(
                        (self.fx + ang.cos() * rr) as i32,
                        (self.fy + ang.sin() * rr * 0.5) as i32,
                        scale(ring_c, a),
                    );
                }
            }
        }

        // --- sparse sharp glints in front, squared-sine twinkle ---
        // Glints near the knot flare up with it (foreground rim light).
        for s in &self.glints {
            let sn = (self.t * s.freq + s.phase).sin();
            let mut b = s.mag * sn * sn; // eased: bright peaks, long dim troughs
            let (ix, iy) = ((s.x * w as f32) as i32, (s.y * h as f32) as i32);
            if env > 0.01 {
                let (ddx, ddy) = (ix as f32 - self.fx, iy as f32 - self.fy);
                b *= 1.0 + env * 1.2 * (-(ddx * ddx + ddy * ddy) / r2_ill).exp();
            }
            if b < 0.06 {
                continue;
            }
            let c = scale(self.glint_c, b.min(1.0));
            canvas.set(ix, iy, c);
            if b > 0.72 {
                // cross sparkle at the twinkle peak
                canvas.add(ix + 1, iy, scale(c, 0.45));
                canvas.add(ix - 1, iy, scale(c, 0.45));
                canvas.add(ix, iy + 1, scale(c, 0.35));
                canvas.add(ix, iy - 1, scale(c, 0.35));
            }
        }
    }
}
