//! Pendulum wave: a row of pendulums hanging from a thin support line, each
//! swinging at a slightly different frequency so the row weaves through
//! snake/zigzag/split interference patterns and periodically re-syncs.
//! Side view: glowing bobs on very faint strings, smooth trails behind the
//! nearest bobs, a camera that breathes with a slow zoom drift. The natural
//! re-sync moment is the payoff — brightness builds across all bobs as the
//! phases converge, a crisp flash fires at full alignment and a white shimmer
//! races along the support line. Between re-syncs a hand periodically plucks
//! one bob: it swings wide, flares, and settles back on a decaying impulse.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::TAU;

/// Trail ring length per bright bob (bounded allocation).
const TRAIL: usize = 7;

struct Pend {
    /// anchor position along the row, 0..1
    frac: f32,
    /// angular frequency, rad/s — slightly different per pendulum
    omega: f32,
    /// per-pendulum amplitude jitter, ~0.92..1.08
    jitter: f32,
    /// 0 = far end of the row, 1 = nearest the camera
    near: f32,
    trail: [(f32, f32); TRAIL],
    head: usize,
}

struct Dust {
    x: f32,
    y: f32,
    phase: f32,
    rate: f32,
}

pub struct Pendulum {
    rng: StdRng,
    detail: Detail,
    accent: (u8, u8, u8),
    string_c: (u8, u8, u8),
    dust_c: (u8, u8, u8),
    pends: Vec<Pend>,
    dust: Vec<Dust>,
    /// oscillations the fastest pendulum's base completes per sync cycle
    base_osc: u32,
    /// seconds for the whole row to re-sync (the event rhythm, ~26-34s)
    t_sync: f32,
    /// flash envelope 0..1, decays after re-sync
    flash: f32,
    flash_cd: f32,
    /// pluck event: index of the kicked pendulum + decaying impulse envelope
    pluck: Option<(usize, f32)>,
    next_pluck: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Pendulum {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let accent = match theme {
            Some("amber") => (255, 182, 72),
            Some("ice") => (150, 205, 255),
            _ => (205, 213, 226), // chrome
        };
        Pendulum {
            base_osc: rng.random_range(15..=19),
            t_sync: rng.random_range(26.0..34.0),
            rng,
            detail,
            accent,
            string_c: scale(accent, 0.09),
            dust_c: scale(accent, 0.05),
            pends: Vec::new(),
            dust: Vec::new(),
            flash: 0.0,
            flash_cd: 0.0,
            pluck: None,
            next_pluck: 5.0, // first pluck lands early; then 7-12s apart
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.pends.clear();
        self.dust.clear();
        if w == 0 || h == 0 {
            return;
        }
        // 12-20 pendulums at typical sizes, scaled by area and detail
        let n = self
            .detail
            .scale(14.0 * density_for(w, h), 10)
            .clamp(10, 24)
            .min(w.saturating_sub(6))
            .max(2);
        let rest_y = (h as f32 * 0.12).max(1.0)
            + (h as f32 * 0.62).min(h as f32 - (h as f32 * 0.12).max(1.0) - 2.0).max(3.0);
        self.pends = (0..n)
            .map(|i| {
                let frac = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.5 };
                Pend {
                    frac,
                    omega: TAU * (self.base_osc + i as u32) as f32 / self.t_sync,
                    jitter: self.rng.random_range(0.92..1.08),
                    near: frac, // left end far, right end near the camera
                    trail: [(frac * w as f32, rest_y); TRAIL],
                    head: 0,
                }
            })
            .collect();
        // far plane: drifting dust motes, dense enough to read at any size
        let nd = self
            .detail
            .scale(30.0 * density_for(w, h), 12)
            .clamp(12, 96);
        self.dust = (0..nd)
            .map(|_| Dust {
                x: self.rng.random::<f32>(),
                y: self.rng.random::<f32>(),
                phase: self.rng.random_range(0.0..TAU),
                rate: self.rng.random_range(0.4..1.3),
            })
            .collect();
    }
}

impl Scene for Pendulum {
    fn name(&self) -> &'static str {
        "pendulum"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        if w == 0 || h == 0 {
            return;
        }
        // cap dt so fast-forward can't blow up the flash envelope
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let t = self.t;

        // --- re-sync event: anticipation -> payoff -> decay ---
        // order parameter of the swing phases: 1 at full alignment
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for p in &self.pends {
            let ph = p.omega * t;
            re += ph.cos();
            im += ph.sin();
        }
        let r = (((re * re + im * im).sqrt()) / self.pends.len().max(1) as f32).clamp(0.0, 1.0);
        // brightness builds only in the approach window (r > 0.55)
        let build = ease_smooth((r - 0.55) / 0.45);
        self.flash_cd -= dt;
        if build > 0.985 && self.flash_cd <= 0.0 {
            self.flash = 1.0;
            self.flash_cd = self.t_sync * 0.55;
        }
        self.flash *= (-dt * 2.6).exp();
        let flash = self.flash;

        // --- pluck event: a hand taps one bob, it swings wide and settles ---
        self.next_pluck -= dt;
        if self.next_pluck <= 0.0 && !self.pends.is_empty() {
            self.pluck = Some((self.rng.random_range(0..self.pends.len()), 1.0));
            self.next_pluck = self.rng.random_range(7.0..12.0);
        }
        if let Some((_, env)) = &mut self.pluck {
            *env *= (-dt * 1.5).exp();
            if *env < 0.02 {
                self.pluck = None;
            }
        }
        let (pluck_idx, pluck_env) = match self.pluck {
            Some((i, e)) => (i as i32, e),
            None => (-1, 0.0),
        };

        // camera breath: slow zoom drift ±5% around the row center
        let zoom = 1.0 + 0.05 * (t * 0.12).sin();
        let (wf, hf) = (w as f32, h as f32);
        let cx = wf * 0.5;
        let support_y = (hf * 0.12).max(1.0);
        let margin = (wf * 0.06).max(2.0);
        let length = (hf * 0.62 * zoom).min(hf - support_y - 2.0).max(3.0);
        let amp = 0.55f32; // max swing angle, rad

        canvas.clear((0, 0, 0));

        // far plane: dust motes drifting slowly, twinkling up as the event builds
        let dust_k = 0.35 + build * 0.4 + flash * 0.6;
        for d in &self.dust {
            let tw = (0.5 + 0.5 * (t * d.rate + d.phase).sin()) * dust_k;
            let dx = (d.x + t * 0.006 * d.rate).rem_euclid(1.0) * wf;
            let dy = (d.y + (t * 0.15 + d.phase).sin() * 0.008) * hf;
            canvas.set_f(dx, dy, scale(self.dust_c, tw));
        }

        // support line, brightening with the event
        let sy = support_y as i32;
        let line_k = (0.5 + build * 0.5 + flash * 2.2).min(2.6);
        for x in 0..w {
            canvas.set(x as i32, sy, scale(self.string_c, line_k));
        }

        let n = self.pends.len();
        let spacing = if n > 1 {
            (wf - margin * 2.0) / (n - 1) as f32
        } else {
            wf
        };
        // swing width, capped so neighbours interleave without dissolving
        let x_amp = (length * amp.sin() * 1.8).min(spacing * 1.7).max(1.5);
        let str_k = (0.55 + build * 0.5 + flash * 1.8).min(2.4);

        for (pi, p) in self.pends.iter_mut().enumerate() {
            // pluck impulse: the tapped bob swings wider and flares
            let kick = if pi as i32 == pluck_idx { pluck_env } else { 0.0 };
            let anchor = margin + p.frac * (wf - margin * 2.0);
            let ax = cx + (anchor - cx) * zoom;
            let s = ((p.omega * t).cos() * p.jitter * (1.0 + 0.5 * kick)).clamp(-1.0, 1.0);
            let theta = amp * s;
            let bx = ax + x_amp * theta.sin() / amp.sin();
            let by = support_y + length * theta.cos();

            // faint string from anchor to bob, rim-lit toward the bob's glow
            let steps = length as i32;
            for k in 0..=steps {
                let u = k as f32 / steps.max(1) as f32;
                let rim = str_k * (0.55 + 0.8 * u * u) + kick * 0.5 * u;
                canvas.set_f(
                    ax + (bx - ax) * u,
                    support_y + (by - support_y) * u,
                    scale(self.string_c, rim),
                );
            }
            // anchor tick on the support line; the plucked one pulses
            canvas.set(
                ax as i32,
                sy,
                scale(self.accent, 0.22 + flash * 0.55 + kick * 0.5),
            );

            // trail ring: only the near (brightest) bobs leave trails
            p.trail[p.head] = (bx, by);
            p.head = (p.head + 1) % TRAIL;
            if p.near > 0.45 {
                for k in 1..TRAIL {
                    let idx = (p.head + TRAIL - 1 - k) % TRAIL;
                    let (tx, ty) = p.trail[idx];
                    let f = 1.0 - k as f32 / TRAIL as f32;
                    canvas.add(
                        tx as i32,
                        ty as i32,
                        scale(self.accent, 0.20 * f * f * (0.4 + 0.6 * p.near)),
                    );
                }
            }

            // bob: depth-graded brightness, flaring with the events
            let bright = (0.5 + 0.5 * p.near) * (1.0 + build * 0.45 + kick * 0.7)
                + flash * 1.1
                + kick * 0.5;
            let core = lerp(
                scale(self.accent, bright.min(1.3)),
                (255, 255, 255),
                (0.30 + build * 0.15 + flash * 0.55 + kick * 0.3).min(0.9),
            );
            let (bxi, byi) = (bx as i32, by as i32);
            canvas.set(bxi, byi, core);
            let gr = (1.0 + p.near * 1.5 + flash * 1.5 + kick * 1.2)
                .round()
                .clamp(1.0, 4.0) as i32;
            glow(
                canvas,
                bxi,
                byi,
                gr,
                self.accent,
                (0.26 * bright + flash * 0.5).min(1.2),
            );
        }

        // crisp flash: a white shimmer racing left-to-right along the
        // support line as the flash decays, over a faint uniform wash
        if flash > 0.02 {
            let sweep = wf * (1.0 - flash);
            let width = (wf * 0.10).max(3.0);
            for x in 0..w {
                let d = (x as f32 - sweep) / width;
                let k = flash * (0.08 + 0.30 * (-d * d).exp());
                canvas.add(x as i32, sy, scale((255, 255, 255), k));
            }
        }
    }
}
