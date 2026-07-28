//! Classic texture-mapped tunnel: per-pixel angle + depth into a procedural
//! checker texture, rotating and flying forward, darker toward the center.
//! Light gates spawn at the vanishing point and race outward past the camera
//! (anticipation -> payoff -> a white kiss as they pass), speed surges ease
//! in and out, and debris chunks tumble by with a hot rim.

use super::Scene;
use crate::canvas::{ease_out, ease_smooth, hsv, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

/// Per-pixel polar geometry, rebuilt only when the canvas size changes.
/// The polar transform (angle, 1/depth, shading, core falloff) depends
/// purely on pixel coordinates, so it is hoisted out of the frame loop.
struct PolarTable {
    w: usize,
    h: usize,
    /// angle * 4/PI (texture u before rotation offset)
    ang4: Vec<f32>,
    /// 2.2 / d (texture v before flight offset)
    vbase: Vec<f32>,
    /// (d / (d + 0.28))^1.5 depth shading — mid-rings pulled down so
    /// accents pop against a dark wall
    shade: Vec<f32>,
    /// exp(-d * 2.5) vanishing-point glow base
    core: Vec<f32>,
}

impl PolarTable {
    fn build(w: usize, h: usize) -> Self {
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        // normalize by half-height so cells stay square-ish
        let norm = h as f32 / 2.0;
        let n = w * h;
        let mut t = PolarTable {
            w,
            h,
            ang4: Vec::with_capacity(n),
            vbase: Vec::with_capacity(n),
            shade: Vec::with_capacity(n),
            core: Vec::with_capacity(n),
        };
        for y in 0..h {
            let dy = (y as f32 - cy) / norm;
            for x in 0..w {
                let dx = (x as f32 - cx) / norm;
                let d = (dx * dx + dy * dy).sqrt() + 1e-4;
                t.ang4.push(dy.atan2(dx) * 4.0 / std::f32::consts::PI);
                t.vbase.push(2.2 / d);
                t.shade.push((d / (d + 0.28)).powf(1.5));
                t.core.push((-d * 2.5).exp());
            }
        }
        t
    }
}

/// A light gate: a bright ring fixed at texture-v, carried outward by the
/// flight offset until it sweeps past the camera.
struct Gate {
    v: f32,
    amp: f32, // peak brightness multiplier
}

pub struct Tunnel {
    rng: StdRng,
    t: f32,
    hue0: f32,
    sat: f32,
    mode: usize,
    mode_t: f32,
    flash: f32,
    /// age of the current surge (>= 1.6 when idle)
    surge_e: f32,
    next_surge: f32,
    /// integrated flight distance / rotation angle (surges must not jump)
    dist: f32,
    rot_a: f32,
    gates: Vec<Gate>,
    next_gate: f32,
    debris: Vec<(f32, f32, f32)>, // angle, depth(z), spin
    next_debris: f32,
    table: Option<PolarTable>,
}

impl Tunnel {
    pub fn new(rng: StdRng, theme: Option<&str>) -> Self {
        let (hue0, sat) = match theme {
            Some("inferno") => (0.02, 0.9),
            Some("mono") => (0.0, 0.15),
            _ => (0.0, 0.8),
        };
        Tunnel {
            rng,
            hue0,
            sat,
            t: 0.0,
            mode: 0,
            mode_t: 0.0,
            flash: 0.0,
            surge_e: 10.0,
            next_surge: 6.0,
            dist: 0.0,
            rot_a: 0.0,
            gates: Vec::new(),
            next_gate: 3.5,
            debris: Vec::new(),
            next_debris: 4.0,
            table: None,
        }
    }
}

impl Scene for Tunnel {
    fn name(&self) -> &'static str {
        "tunnel"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let dt = dt.clamp(0.0, 0.1); // survive fast-forward
        self.t += dt;
        let t = self.t;
        // texture rotation every ~12s with a white flash transition
        self.mode_t += dt;
        if self.mode_t > 12.0 {
            self.mode_t = 0.0;
            self.mode = (self.mode + 1) % 3;
            self.flash = 0.25;
        }
        self.flash = (self.flash - dt).max(0.0);
        // occasional speed surge: fast attack, smooth decay — never linear
        self.next_surge -= dt;
        let mut surge_fired = false;
        if self.next_surge <= 0.0 {
            surge_fired = true;
            self.surge_e = 0.0;
            self.next_surge = self.rng.random_range(8.0..13.0);
        }
        let env = if self.surge_e < 1.6 {
            let e = self.surge_e;
            if e < 0.25 {
                ease_out(e / 0.25)
            } else {
                1.0 - ease_smooth((e - 0.25) / 1.35)
            }
        } else {
            0.0
        };
        self.surge_e += dt;
        // integrate distance and rotation so the eased speed never pops
        self.dist += dt * 1.6 * (1.0 + env * 1.8);
        self.rot_a += dt * (0.25 + env * 0.55);
        let vfly = self.dist;
        let rot = self.rot_a + (t * 0.11).sin() * 1.2;

        // light gates: spawn deep, race outward, blaze, fade before the edge
        if surge_fired && self.gates.len() < 4 {
            self.gates.push(Gate {
                v: vfly + 9.0,
                amp: 1.2,
            });
        }
        self.next_gate -= dt;
        if self.next_gate <= 0.0 {
            self.next_gate = self.rng.random_range(4.0..7.0);
            if self.gates.len() < 4 {
                self.gates.push(Gate {
                    v: vfly + 9.0,
                    amp: self.rng.random_range(0.7..1.0),
                });
            }
        }
        let mut passed = false;
        self.gates.retain(|g| {
            if g.v - vfly < 1.45 {
                passed = true; // swept past the camera
                return false;
            }
            true
        });
        if passed {
            // payoff: a brief white kiss as the ring exits the frame
            self.flash = self.flash.max(0.12);
        }

        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // rebuild the polar lookup only on (re)size
        if !matches!(&self.table, Some(tb) if tb.w == w && tb.h == h) {
            self.table = Some(PolarTable::build(w, h));
        }
        let tb = self.table.as_ref().unwrap();
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let norm = h as f32 / 2.0;
        let hue = self.hue0 + t * 0.03; // slow hue drift
        // near-white accent tinted by the theme: gates, core, debris rims
        let accent = hsv(hue + 0.06, self.sat * 0.25, 1.0);
        let ac = (
            accent.0 as f32,
            accent.1 as f32,
            accent.2 as f32,
        );
        // loop-invariant gate envelope: grow in from the core, blaze as it
        // nears the camera, fade just before the edge so it never pops off
        let mut gate_pv = [(0.0f32, 0.0f32); 4];
        let mut n_gates = 0usize;
        for g in &self.gates {
            if n_gates >= 4 {
                break;
            }
            let ahead = g.v - vfly;
            let amp = g.amp
                * ease_smooth((9.0 - ahead) / 2.0)
                * ease_smooth((ahead - 1.45) / 0.9)
                * (1.0 + env * 0.4);
            if amp > 0.01 {
                gate_pv[n_gates] = (g.v, amp);
                n_gates += 1;
            }
        }
        // loop-invariant offsets, hoisted out of the pixel loop
        let core_amp = 0.12 + 0.08 * (t * 0.5).sin();
        let flash_t = (self.flash / 0.25).min(1.0) * 0.8;
        let sat = self.sat;
        let sat2 = (sat + 0.1).min(1.0);
        // per-branch constants for the specialized hsv below
        let p0 = 0.95 * (1.0 - sat);
        let p1 = 0.32 * (1.0 - sat2);
        let mode = self.mode;
        let mut i = 0usize;
        for y in 0..h {
            for x in 0..w {
                // texture coordinates: angular stripes × depth rings
                let u = tb.ang4[i] + rot;
                let v = tb.vbase[i] + vfly;
                let uf = u.floor() as i32;
                let vf = v.floor() as i32;
                let checker = match mode {
                    0 => (uf + vf) & 1, // checker
                    1 => vf & 1,        // rings
                    _ => uf & 1,        // stripes
                };
                // depth shading: dark at the vanishing point
                // |sin(v*PI)| via a parabola on fract(v) — visually identical
                // at the ±0.15 brightness modulation it drives
                let f = v - vf as f32;
                let ring_glow = 0.85 + 0.15 * 4.0 * f * (1.0 - f);
                let hue_v = hue + v * 0.01 + if checker == 0 { 0.0 } else { 0.08 };
                // specialized hsv(hue_v, s, val): hue_v is always positive,
                // and s/val are per-branch constants, so p is hoisted and the
                // segment math needs no rem_euclid
                let (val, sat_b, p) = if checker == 0 {
                    (0.95, sat, p0)
                } else {
                    (0.32, sat2, p1)
                };
                let h6 = (hue_v - hue_v.floor()) * 6.0;
                let seg = h6 as i32; // 0..=5 since fract < 1
                let hf = h6 - seg as f32;
                let q = val * (1.0 - sat_b * hf);
                let tt = val * (1.0 - sat_b * (1.0 - hf));
                let (r, g, b) = match seg {
                    0 => (val, tt, p),
                    1 => (q, val, p),
                    2 => (p, val, tt),
                    3 => (p, q, val),
                    4 => (tt, p, val),
                    _ => (val, p, q),
                };
                let color = (r * 255.0, g * 255.0, b * 255.0);
                // additive light: breathing core glow + gate rings blazing by
                let core = tb.core[i] * core_amp;
                let mut boost = core;
                for &(gv, ga) in gate_pv.iter().take(n_gates) {
                    let dv = (v - gv) / 0.22;
                    let g = (1.0 - dv * dv).max(0.0);
                    boost += ga * g * g * 1.3;
                }
                // flash lerp toward white, depth shade, additive accent —
                // fused per channel:
                //   final = (color + (255-color)*flash_t) * k + accent * boost
                let k = tb.shade[i] * ring_glow * (1.0 - core);
                let fin = |c: f32, a: f32| {
                    let c2 = c + (255.0 - c) * flash_t;
                    (c2 * k + a * boost).clamp(0.0, 255.0) as u8
                };
                canvas.set(
                    x as i32,
                    y as i32,
                    (fin(color.0, ac.0), fin(color.1, ac.1), fin(color.2, ac.2)),
                );
                i += 1;
            }
        }

        // debris tumbling past the camera: additive, hot-rimmed, with a
        // lifecycle envelope so chunks grow in and fade out — no popping
        self.next_debris -= dt;
        if self.next_debris <= 0.0 {
            self.next_debris = self.rng.random_range(3.0..8.0);
            self.debris.push((
                self.rng.random_range(0.0..std::f32::consts::TAU),
                1.0,
                self.rng.random_range(0.0..6.0),
            ));
        }
        let debris_col = lerp(accent, (255, 255, 255), 0.2);
        self.debris.retain_mut(|(ang, z, spin)| {
            *z -= dt * 0.9;
            if *z < 0.06 {
                return false;
            }
            let alpha = ease_smooth((1.0 - *z) / 0.12) * ease_smooth((*z - 0.06) / 0.18);
            *ang += dt * 0.5 * spin.sin(); // lazy spiral drift
            let d = *z;
            let px = cx + ang.cos() * d * norm * 2.2;
            let py = cy + ang.sin() * d * norm * 2.2;
            let size = (1.0 - d).max(0.1);
            let shade2 = 0.3 + 0.7 * (1.0 - d);
            let col = scale(debris_col, shade2 * alpha * 0.7);
            for dy in 0..=(size * 2.0) as i32 {
                for dx in 0..=(size * 2.0) as i32 {
                    canvas.add(
                        (px + dx as f32 + spin.sin()) as i32,
                        (py + dy as f32 + spin.cos()) as i32,
                        col,
                    );
                }
            }
            *spin += dt * 4.0;
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use std::collections::HashSet;

    #[test]
    fn tunnel_is_nonuniform_and_dark_at_center() {
        let mut t = Tunnel::new(StdRng::seed_from_u64(2), None);
        let mut c = Canvas::new(60, 30);
        t.update(1.0 / 30.0, &mut c);
        let colors: HashSet<_> = (0..60)
            .flat_map(|x| (0..30).map(move |y| (x, y)))
            .map(|(x, y)| c.get(x, y).color)
            .collect();
        assert!(colors.len() > 20, "tunnel should have rich shading");
        let sum = |x: i32, y: i32| {
            let (r, g, b) = c.get(x, y).color;
            r as u32 + g as u32 + b as u32
        };
        let center = sum(30, 15);
        let edge = sum(2, 15).max(sum(58, 15));
        assert!(center < edge, "center {center} should be darker than edge {edge}");
    }
}
