//! The abyss: god-ray shafts swaying from the surface, marine snow,
//! passing fish schools, rare whale/manta silhouettes, and pulsing
//! bioluminescent motes near the bottom. Every ~20-30s a SUN-BREAK
//! surge eases in: the shafts brighten, the surface water lifts, snow
//! drifting through the light sparkles, and silhouettes catch a rim.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const SURGE_ATTACK: f32 = 2.5;
const SURGE_HOLD: f32 = 1.5;
const SURGE_RELEASE: f32 = 4.0;
const SURGE_TOTAL: f32 = SURGE_ATTACK + SURGE_HOLD + SURGE_RELEASE;

/// Sun-break envelope: ease in, hold, ease out (0..1).
fn surge_envelope(e: f32) -> f32 {
    if e < SURGE_ATTACK {
        ease_smooth(e / SURGE_ATTACK)
    } else if e < SURGE_ATTACK + SURGE_HOLD {
        1.0
    } else {
        1.0 - ease_smooth((e - SURGE_ATTACK - SURGE_HOLD) / SURGE_RELEASE)
    }
}

struct Ray {
    x: f32,
    w: f32,
    phase: f32,
    speed: f32,
    sway: f32,
}

struct Snow {
    x: f32,
    y: f32,
    vy: f32,
    phase: f32,
    bright: f32,
}

struct School {
    x: f32,
    y: f32,
    vx: f32,
    fish: Vec<(f32, f32, f32)>, // dx, dy, wiggle phase
    panic: Vec<(f32, f32)>,     // per-fish flee offset (decays)
}

#[derive(Clone, Copy, PartialEq)]
enum PredKind {
    Angler,
    Shark,
}

struct Predator {
    kind: PredKind,
    x: f32,
    y: f32,
    vx: f32,
    phase: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum BigKind {
    Whale,
    Manta,
}

struct Big {
    kind: BigKind,
    x: f32,
    y: f32,
    vx: f32,
    phase: f32,
}

struct Mote {
    x: f32,
    y: f32,
    phase: f32,
    speed: f32,
}

pub struct Abyss {
    rng: StdRng,
    detail: Detail,
    water: ((u8, u8, u8), (u8, u8, u8)),
    mote_c: (u8, u8, u8),
    rays: Vec<Ray>,
    snow: Vec<Snow>,
    school: Option<School>,
    next_school: f32,
    big: Option<Big>,
    next_big: f32,
    motes: Vec<Mote>,
    jelly: Option<(f32, f32, f32)>, // x, y, phase
    next_jelly: f32,
    predator: Option<Predator>,
    next_predator: f32,
    surge_t: Option<f32>, // elapsed in the active sun-break surge
    next_surge: f32,
    ray_cache: Vec<(f32, f32, f32)>, // per-frame ray (x, w, sway) for snow sparkle
    t: f32,
    w: usize,
    h: usize,
}

impl Abyss {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (water, mote_c) = match theme {
            Some("trench") => (((6, 26, 20), (1, 6, 5)), (120, 255, 140)),
            Some("twilight") => (((30, 24, 50), (4, 4, 12)), (200, 140, 255)),
            Some("reef") => (((12, 50, 55), (2, 12, 18)), (255, 180, 120)),
            _ => (((10, 36, 52), (1, 5, 10)), (60, 240, 200)), // deep
        };
        Abyss {
            rng,
            detail,
            water,
            mote_c,
            rays: Vec::new(),
            snow: Vec::new(),
            school: None,
            next_school: 2.0,
            big: None,
            next_big: 8.0,
            motes: Vec::new(),
            jelly: None,
            next_jelly: 12.0,
            predator: None,
            next_predator: 9.0,
            surge_t: None,
            next_surge: 7.0, // first payoff lands early; then 18-30s apart
            ray_cache: Vec::new(),
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let n_rays = (((w / 14) as f32 * density_for(w, h)) as usize).clamp(3, 12);
        self.rays = (0..n_rays)
            .map(|_| Ray {
                x: self.rng.random_range(0.0..w as f32),
                w: self.rng.random_range(2.0..6.0),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                speed: self.rng.random_range(0.15..0.4),
                sway: self.rng.random_range(2.0..5.0),
            })
            .collect();
        self.snow = (0..self.detail.scale((w * h / 150) as f32 * 2.0 * density_for(w, h), 30).min(700))
            .map(|_| Snow {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                vy: self.rng.random_range(1.0..3.5),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                bright: self.rng.random_range(0.2..0.7),
            })
            .collect();
        let n_motes = ((w as f32 / 3.0 * density_for(w, h)) as usize).clamp(10, 80);
        self.motes = (0..n_motes)
            .map(|_| Mote {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(h as f32 * 0.78..h as f32 * 0.98),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                speed: self.rng.random_range(0.5..1.6),
            })
            .collect();
    }

    fn spawn_school(&mut self) {
        let from_left = self.rng.random::<bool>();
        let n = self.rng.random_range(18..36);
        self.school = Some(School {
            x: if from_left { -20.0 } else { self.w as f32 + 20.0 },
            y: self.rng.random_range(self.h as f32 * 0.3..self.h as f32 * 0.65),
            vx: if from_left {
                self.rng.random_range(12.0..20.0)
            } else {
                -self.rng.random_range(12.0..20.0)
            },
            fish: (0..n)
                .map(|_| {
                    (
                        self.rng.random_range(-14.0..14.0),
                        self.rng.random_range(-4.0..4.0),
                        self.rng.random_range(0.0..std::f32::consts::TAU),
                    )
                })
                .collect(),
            panic: vec![(0.0, 0.0); n],
        });
    }

    fn spawn_big(&mut self) {
        let from_left = self.rng.random::<bool>();
        self.big = Some(Big {
            kind: if self.rng.random::<f32>() < 0.6 {
                BigKind::Whale
            } else {
                BigKind::Manta
            },
            x: if from_left { -40.0 } else { self.w as f32 + 40.0 },
            y: self.rng.random_range(self.h as f32 * 0.35..self.h as f32 * 0.6),
            vx: if from_left { 7.0 } else { -7.0 },
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
        });
    }

}

fn draw_big(canvas: &mut Canvas, b: &Big, t: f32, w: usize, surge: f32) {
    let dark = (3, 8, 14);
    let rim = scale((50, 88, 102), 0.10 + surge * 0.30);
    let dir = b.vx.signum();
    // slow vertical bob so the crossing breathes instead of sliding
    let by = b.y + (t * 0.5 + b.phase).sin() * 1.5;
    match b.kind {
        BigKind::Whale => {
            let len = (w as f32 * 0.28).clamp(16.0, 46.0) as i32;
            let und = (t * 1.2 + b.phase).sin() * 1.2;
            for i in 0..len {
                let u = i as f32 / len as f32; // 0 head .. 1 tail
                let thick = (1.6 + (u * std::f32::consts::PI).sin() * 4.2) as i32;
                let yy = by + und * u;
                let x = b.x as i32 - (i as f32 * dir) as i32;
                for t in -thick / 2..=thick / 2 {
                    canvas.set(x, yy as i32 + t, dark);
                }
                // sun-break rim: the top edge catches the shafts
                canvas.add(x, yy as i32 - thick / 2 - 1, rim);
            }
            // tail fluke
            let tx = b.x as i32 - (len as f32 * dir) as i32;
            let ty = (by + und) as i32;
            for t in -3..=3 {
                canvas.set(tx - (dir as i32), ty + t, dark);
                canvas.set(tx - (dir as i32 * 2), ty + t * 2 / 3, dark);
            }
        }
        BigKind::Manta => {
            let span = (w as f32 * 0.20).clamp(10.0, 30.0) as i32;
            let flap = (t * 2.0 + b.phase).sin();
            for i in -span..=span {
                let u = (i as f32 / span as f32).abs(); // 0 center .. 1 tip
                let wing = ((1.0 - u) * 3.0) as i32;
                let lift = (flap * u * 2.0) as i32;
                for t in -wing / 2..=wing / 2 {
                    canvas.set(b.x as i32 + i, by as i32 + t - lift, dark);
                }
                canvas.add(b.x as i32 + i, by as i32 - wing / 2 - lift - 1, rim);
            }
            // tail
            for i in 1..=span / 2 {
                canvas.set(b.x as i32 - i * dir as i32, by as i32, dark);
            }
        }
    }
}

impl Scene for Abyss {
    fn name(&self) -> &'static str {
        "abyss"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        self.t += dt;
        let t = self.t;

        // --- sun-break surge scheduling (anticipation -> payoff -> decay) ---
        self.next_surge -= dt;
        match self.surge_t {
            None => {
                if self.next_surge <= 0.0 {
                    self.surge_t = Some(0.0);
                }
            }
            Some(e) => {
                let e = e + dt;
                if e >= SURGE_TOTAL {
                    self.surge_t = None;
                    self.next_surge = self.rng.random_range(18.0..30.0);
                } else {
                    self.surge_t = Some(e);
                }
            }
        }
        let surge = self.surge_t.map(surge_envelope).unwrap_or(0.0);

        // water column: teal-blue above fading to black below; the surge
        // lifts the surface band toward sunlit
        let sunlit = lerp(self.water.0, (58, 112, 128), surge * 0.55);
        for y in 0..h {
            let ty = y as f32 / h.max(1) as f32;
            let top = lerp(self.water.0, sunlit, (1.0 - ty * 2.2).max(0.0));
            canvas.fill_row(y, lerp(top, self.water.1, ty.powf(0.8)));
        }

        // god rays: swaying shafts fading with depth, breathing, flaring
        // with the surge
        self.ray_cache.clear();
        let depth_max = h as f32 * 0.75;
        for r in &self.rays {
            let rx = r.x + (t * r.speed + r.phase).sin() * r.sway;
            let breathe = 0.72 + 0.28 * (t * 0.45 + r.phase * 1.7).sin();
            let boost = breathe * (1.0 + surge * 1.15);
            for y in 0..depth_max as i32 {
                let fy = y as f32 / depth_max;
                let fade = (1.0 - fy).powi(2) * 0.5 * boost;
                // shaft widens slightly with depth
                let hw = r.w * (0.7 + fy * 0.8);
                for dx in -(hw as i32)..=(hw as i32) {
                    let d = (dx as f32 / hw).abs();
                    let b = (-d * d * 2.5).exp() * fade;
                    let x = (rx + fy * r.sway * 0.6) as i32 + dx;
                    canvas.add(x, y, scale((50, 110, 120), b));
                }
            }
            self.ray_cache.push((rx, r.w, r.sway));
        }

        // marine snow: sparkles when drifting through a lit shaft
        for s in &mut self.snow {
            s.y += s.vy * dt;
            s.x += (t * 0.8 + s.phase).sin() * 0.4 * dt;
            if s.y > h as f32 {
                s.y = -1.0;
                s.x = self.rng.random_range(0.0..w as f32);
            }
            let mut shaft = 0.0f32;
            if s.y < depth_max {
                let fy = s.y / depth_max;
                for &(rx, rw, sway) in &self.ray_cache {
                    let hw = rw * (0.7 + fy * 0.8);
                    let d = (s.x - (rx + fy * sway * 0.6)).abs();
                    if d < hw {
                        shaft = shaft.max(1.0 - d / hw);
                    }
                }
            }
            let b = (s.bright * (1.0 + shaft * (0.8 + surge * 1.3))).min(1.3);
            canvas.set_f(s.x, s.y, scale((170, 190, 210), b));
        }

        // hydrothermal vent: dark smoker chimney with a shimmering plume
        let vent_x = w as f32 * 0.78;
        for dy in 0..(h as f32 * 0.08) as i32 {
            for dx in -2..=2 {
                canvas.set(vent_x as i32 + dx, h as i32 - 1 - dy, (10, 10, 12));
            }
        }
        for i in 0..14 {
            let fi = i as f32;
            let rise = ((t * 6.0 + fi * 4.0) % (h as f32 * 0.3)) as i32;
            let spread = rise as f32 * 0.15;
            let wob = (t * 1.5 + fi).sin() * 2.0;
            canvas.add(
                vent_x as i32 + (wob + (fi - 7.0) * spread * 0.3) as i32,
                h as i32 - (h as f32 * 0.08) as i32 - rise,
                scale((60, 70, 80), 0.35),
            );
        }

        // bioluminescent motes pulsing near the bottom: snappy peaks with
        // a hot core and a faint bleed at full flash
        for m in &self.motes {
            let p = 0.5 + 0.5 * (t * m.speed + m.phase).sin();
            if p > 0.55 {
                let b = ((p - 0.55) / 0.45).powf(1.4);
                let core = lerp(self.mote_c, (235, 255, 250), (b - 0.75).max(0.0) * 1.6);
                canvas.add(m.x as i32, m.y as i32, scale(core, b));
                if b > 0.8 {
                    glow(canvas, m.x as i32, m.y as i32, 1, self.mote_c, b * 0.5);
                }
            }
        }

        // fish schools
        self.next_school -= dt;
        if self.next_school <= 0.0 && self.school.is_none() {
            self.spawn_school();
            self.next_school = self.rng.random_range(7.0..14.0);
        }
        if let Some(s) = &mut self.school {
            // speed breathes; the whole school bobs gently as it crosses
            let spd = s.vx * (0.85 + 0.15 * (t * 0.9).sin());
            s.x += spd * dt;
            let bob = (t * 0.7).sin() * 1.2;
            let gone = (s.vx > 0.0 && s.x > w as f32 + 25.0) || (s.vx < 0.0 && s.x < -25.0);
            if gone {
                self.school = None;
            } else {
                let sil = (8, 18, 26);
                for (i, &(dx, dy, ph)) in s.fish.iter().enumerate() {
                    let wig = (t * 6.0 + ph).sin() * 0.8;
                    let fx = s.x + dx + wig * 0.4 + s.panic[i].0;
                    let fy = s.y + dy + wig * 0.3 + s.panic[i].1 + bob;
                    // small elongated silhouette with tail flick
                    canvas.set_f(fx, fy, sil);
                    canvas.set_f(fx - s.vx.signum(), fy + wig * 0.5, sil);
                }
            }
        }

        // a jellyfish drifts up, pulsing
        self.next_jelly -= dt;
        if self.next_jelly <= 0.0 && self.jelly.is_none() {
            self.jelly = Some((
                self.rng.random_range(w as f32 * 0.2..w as f32 * 0.8),
                h as f32 + 6.0,
                self.rng.random_range(0.0..6.0),
            ));
            self.next_jelly = self.rng.random_range(18.0..32.0);
        }
        if let Some((jx, jy, jph)) = &mut self.jelly {
            let pulse = 0.5 + 0.5 * (t * 1.8 + *jph).sin();
            // swims by contracting: rises in pulse-driven surges
            *jy -= dt * (0.8 + 3.0 * pulse);
            if *jy < -10.0 {
                self.jelly = None;
            } else {
                let jr = 3.0 + pulse * 1.5;
                let glow_c = (120, 180, 220);
                // translucent dome
                for dy in 0..=(jr as i32) {
                    let half = ((jr * jr - dy as f32 * dy as f32).sqrt()) as i32;
                    for dx in -half..=half {
                        canvas.add(
                            *jx as i32 + dx,
                            *jy as i32 - dy,
                            scale(glow_c, 0.25 + 0.35 * pulse),
                        );
                    }
                }
                // tentacles
                for k in -2..=2 {
                    let sway = (t * 2.0 + k as f32).sin();
                    for dy in 1..=(jr as i32 + 3) {
                        canvas.add(
                            *jx as i32 + k * 2 + (sway * dy as f32 * 0.3) as i32,
                            *jy as i32 + dy,
                            scale(glow_c, 0.18),
                        );
                    }
                }
            }
        }

        // predator patrol: anglerfish with pulsing lure, or shark sweep
        self.next_predator -= dt;
        if self.next_predator <= 0.0 && self.predator.is_none() {
            let from_left = self.rng.random::<bool>();
            self.predator = Some(Predator {
                kind: if self.rng.random::<f32>() < 0.5 {
                    PredKind::Angler
                } else {
                    PredKind::Shark
                },
                x: if from_left { -15.0 } else { w as f32 + 15.0 },
                y: self.rng.random_range(h as f32 * 0.3..h as f32 * 0.55),
                vx: if from_left { 10.0 } else { -10.0 },
                phase: self.rng.random_range(0.0..6.0),
            });
            self.next_predator = self.rng.random_range(16.0..28.0);
        }
        let mut pred_pos: Option<(f32, f32)> = None;
        if let Some(pr) = &mut self.predator {
            pr.x += pr.vx * dt;
            let gone = (pr.vx > 0.0 && pr.x > w as f32 + 20.0)
                || (pr.vx < 0.0 && pr.x < -20.0);
            if gone {
                self.predator = None;
            } else {
                // swims with a slow sine undulation instead of a straight rail
                let py = pr.y + (t * 1.2 + pr.phase).sin() * 1.4;
                pred_pos = Some((pr.x, py));
                let dark = (2, 5, 9);
                match pr.kind {
                    PredKind::Angler => {
                        // rounded dark body
                        let br = 3;
                        for dy in -br..=br {
                            for dx in -br * 2..=br * 2 {
                                if (dx * dx + dy * dy * 2) as f32 <= (br * br * 2) as f32 {
                                    canvas.set(pr.x as i32 + dx, py as i32 + dy, dark);
                                }
                            }
                        }
                        // fang hints
                        let dir = pr.vx.signum() as i32;
                        canvas.set(pr.x as i32 + dir * 5, py as i32 + 1, (150, 160, 170));
                        canvas.set(pr.x as i32 + dir * 4, py as i32 + 2, (130, 140, 150));
                        // glowing lure on a stalk, pulsing; the glow bleeds
                        // into the surrounding water
                        let pulse = 0.5 + 0.5 * (t * 2.5 + pr.phase).sin();
                        let (lx, ly) = (pr.x as i32 + dir * 3, py as i32 - 5);
                        canvas.set(pr.x as i32 + dir * 2, py as i32 - 4, dark);
                        glow(canvas, lx, ly, 3, (150, 230, 255), pulse * 0.8);
                        canvas.set(lx, ly, scale((200, 245, 255), 0.4 + 0.6 * pulse));
                    }
                    PredKind::Shark => {
                        // dorsal-fin profile sweep
                        let len = 12;
                        for i in 0..len {
                            let u = i as f32 / len as f32;
                            let thick = (1.0 + (u * std::f32::consts::PI).sin() * 2.2) as i32;
                            for ty in -thick / 2..=thick / 2 {
                                canvas.set(
                                    pr.x as i32 - (i as f32 * pr.vx.signum()) as i32,
                                    py as i32 + ty,
                                    dark,
                                );
                            }
                        }
                        // dorsal fin
                        for fdx in 0..4 {
                            canvas.set(
                                pr.x as i32 - (fdx as f32 * pr.vx.signum() * 0.6) as i32,
                                py as i32 - 3 - fdx / 2,
                                dark,
                            );
                        }
                    }
                }
            }
        }

        // fear: school scatters from the predator, then regroups
        if let Some(s) = &mut self.school {
            for (i, f) in s.fish.iter_mut().enumerate() {
                if let Some((px, py)) = pred_pos {
                    let fx = s.x + f.0;
                    let fy = s.y + f.1;
                    let (dx, dy) = (fx - px, fy - py);
                    let d2 = dx * dx + dy * dy;
                    const FEAR: f32 = 14.0;
                    if d2 < FEAR * FEAR {
                        let d = d2.sqrt().max(0.5);
                        let burst = (1.0 - d / FEAR) * 26.0;
                        s.panic[i].0 = dx / d * burst;
                        s.panic[i].1 = dy / d * burst;
                    }
                }
                // panic decays: fish drift back to formation
                s.panic[i].0 *= 1.0 - dt * 1.4;
                s.panic[i].1 *= 1.0 - dt * 1.4;
            }
        }

        // big silhouette crossing with presence
        self.next_big -= dt;
        if self.next_big <= 0.0 && self.big.is_none() {
            self.spawn_big();
            self.next_big = self.rng.random_range(25.0..45.0);
        }
        if let Some(b) = &mut self.big {
            b.x += b.vx * dt;
            let gone = (b.vx > 0.0 && b.x > w as f32 + 60.0) || (b.vx < 0.0 && b.x < -60.0);
            if gone {
                self.big = None;
            } else {
                draw_big(canvas, b, t, w, surge);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn school_fish_stay_in_bounds_and_despawn() {
        let mut a = Abyss::new(StdRng::seed_from_u64(95), None, Detail::Medium);
        let mut c = Canvas::new(100, 50);
        a.update(1.0 / 30.0, &mut c);
        a.spawn_school();
        let mut saw_school = false;
        let mut saw_despawn = false;
        for _ in 0..600 {
            a.update(1.0 / 30.0, &mut c);
            if let Some(s) = &a.school {
                saw_school = true;
                for &(dx, dy, _) in &s.fish {
                    let fx = s.x + dx;
                    let fy = s.y + dy;
                    assert!(fx > -40.0 && fx < 140.0, "fish x {fx}");
                    assert!(fy > -10.0 && fy < 60.0, "fish y {fy}");
                }
            } else if saw_school {
                saw_despawn = true; // first school crossed and despawned
                break;
            }
        }
        assert!(saw_school);
        assert!(saw_despawn, "school should cross and despawn");
    }

    #[test]
    fn big_creature_crosses_without_panic() {
        let mut a = Abyss::new(StdRng::seed_from_u64(96), None, Detail::Medium);
        let mut c = Canvas::new(100, 50);
        a.update(1.0 / 30.0, &mut c);
        for kind in [BigKind::Whale, BigKind::Manta] {
            a.big = Some(Big {
                kind,
                x: 50.0,
                y: 25.0,
                vx: 7.0,
                phase: 0.0,
            });
            for _ in 0..30 {
                a.update(1.0 / 30.0, &mut c);
            }
        }
    }
}

#[cfg(test)]
mod fear_tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn school_scatters_from_predator() {
        let mut a = Abyss::new(StdRng::seed_from_u64(95), None, Detail::Medium);
        let mut c = Canvas::new(100, 50);
        a.update(1.0 / 30.0, &mut c);
        a.spawn_school();
        // center the school, park an angler right next to it
        if let Some(s) = &mut a.school {
            s.x = 50.0;
            s.y = 25.0;
            s.vx = 0.0;
        }
        a.predator = Some(Predator {
            kind: PredKind::Angler,
            x: 55.0,
            y: 25.0,
            vx: 0.0,
            phase: 0.0,
        });
        for _ in 0..30 {
            a.update(1.0 / 30.0, &mut c);
        }
        let s = a.school.as_ref().unwrap();
        let panic_mag: f32 = s.panic.iter().map(|(x, y)| x * x + y * y).sum::<f32>().sqrt();
        assert!(panic_mag > 1.0, "school should panic near a predator");
        // panic should point AWAY from the predator (negative x here)
        let mean_px: f32 = s.panic.iter().map(|p| p.0).sum::<f32>() / s.panic.len() as f32;
        assert!(mean_px < 0.0, "fish should flee away from predator, mean {mean_px}");
    }
}
