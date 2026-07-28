//! Ribbons: silk ribbons flowing across the dark. 3-5 long ribbons, each a
//! verlet point-chain held together by distance constraints (smooth curves,
//! no sharp angles), pushed by fbm wind and spring-damper head steering.
//! Per-ribbon depth: distant ribbons are dimmer and drawn first, near ones
//! weave over them; near heads carry a soft glow. Brightness tapers from a
//! bright leading edge down the silky body. A sparse plane of dust motes
//! drifts on the same wind, keeping the dark alive. Where two ribbons of
//! different colors cross, a brief additive shimmer blooms. Every ~20-30s a
//! gust front sweeps through: anticipation tremble -> shared billow with a
//! faint light wash racing across the frame and the dust surging downwind
//! -> settle.

use super::noise::fbm;
use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use crate::physics::{self, Body, Forces};
use rand::{rngs::StdRng, RngExt};
use std::f32::consts::TAU;

/// Chain segment length in pixels (shorter = silkier, costlier).
const SEG: f32 = 2.6;
/// Cap on live crossing shimmers.
const MAX_SPARKS: usize = 24;

struct Ribbon {
    pts: Vec<Body>,
    color: (u8, u8, u8),
    /// 0.35 (far) .. 1.0 (near): brightness + draw order
    depth: f32,
    /// noise field offset so each ribbon rides its own wind
    phase: f32,
    seed: u32,
    /// head wander target
    tx: f32,
    ty: f32,
    retarget: f32,
    /// base horizontal flow direction (+1 / -1)
    drift: f32,
}

struct Spark {
    x: f32,
    y: f32,
    age: f32,
    color: (u8, u8, u8),
}

/// Background-plane dust: drifts on the wind, surges with the gust front.
struct Mote {
    x: f32,
    y: f32,
    phase: f32,
    mag: f32,
    drift: f32,
}

pub struct Ribbons {
    rng: StdRng,
    detail: Detail,
    palette: Vec<(u8, u8, u8)>,
    ribbons: Vec<Ribbon>,
    sparks: Vec<Spark>,
    motes: Vec<Mote>,
    /// per-frame crossing samples, reused to avoid hot-path allocs
    samples: Vec<(usize, f32, f32)>,
    next_gust: f32,
    /// <0 inactive, else seconds since the gust front started
    gust_t: f32,
    gust_dur: f32,
    gust_dir: (f32, f32),
    t: f32,
    w: usize,
    h: usize,
}

impl Ribbons {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let palette = match theme {
            Some("ember") => vec![(235, 70, 32), (255, 150, 55), (255, 205, 105), (200, 45, 25)],
            Some("ocean") => vec![(45, 185, 205), (60, 120, 220), (85, 220, 225), (30, 85, 175)],
            // silk: muted pastel multi — rose, lilac, pale gold
            _ => vec![(225, 145, 165), (185, 155, 215), (225, 205, 155), (160, 175, 210)],
        };
        Ribbons {
            rng,
            detail,
            palette,
            ribbons: Vec::new(),
            sparks: Vec::new(),
            motes: Vec::new(),
            samples: Vec::new(),
            next_gust: 14.0,
            gust_t: -1.0,
            gust_dur: 4.5,
            gust_dir: (1.0, 0.0),
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.ribbons.clear();
        self.sparks.clear();
        if w == 0 || h == 0 {
            return;
        }
        let n = self
            .detail
            .scale(3.0 * density_for(w, h), 3)
            .clamp(3, 5);
        let n_pts = self
            .detail
            .scale((w as f32 / SEG) * 0.55, 12)
            .clamp(12, 80);
        // background dust, riding the same wind
        let nm = self
            .detail
            .scale((w * h / 200) as f32 * density_for(w, h), 12)
            .min(380);
        self.motes = (0..nm)
            .map(|_| Mote {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                phase: self.rng.random_range(0.0..TAU),
                mag: self.rng.random_range(0.05..0.13),
                drift: self.rng.random_range(1.5..5.0)
                    * if self.rng.random::<bool>() { 1.0 } else { -1.0 },
            })
            .collect();
        for i in 0..n {
            // spread depths far -> near; small jitter so order isn't fixed
            let depth = (0.4 + 0.6 * i as f32 / (n - 1).max(1) as f32
                + self.rng.random_range(-0.05..0.05))
            .clamp(0.35, 1.0);
            let drift = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
            let hx = self.rng.random_range(0.0..w as f32);
            // stratified heights: ribbons start spread across the frame
            let band = (i as f32 + 0.5) / n as f32;
            let hy = h as f32 * (0.15 + 0.7 * band)
                + self.rng.random_range(-(h as f32) * 0.08..h as f32 * 0.08);
            // lay the chain out trailing behind the head, slightly waved
            let phase = self.rng.random_range(0.0..100.0);
            let pts = (0..n_pts)
                .map(|k| {
                    let wave = (k as f32 * 0.45 + phase).sin() * 1.5;
                    Body::new(hx - drift * k as f32 * SEG, hy + wave)
                })
                .collect();
            self.ribbons.push(Ribbon {
                pts,
                color: self.palette[i % self.palette.len()],
                depth,
                phase,
                seed: self.rng.random::<u32>(),
                tx: hx,
                ty: hy,
                retarget: self.rng.random_range(1.0..4.0),
                drift,
            });
        }
        // far first, near last so near ribbons weave over distant ones
        self.ribbons
            .sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap());
    }

    /// Gust envelope 0..1: a low trembling anticipation, a fast ramp to the
    /// billow payoff, then a long eased decay.
    fn gust_env(&self) -> f32 {
        if self.gust_t < 0.0 {
            return 0.0;
        }
        let bt = self.gust_t;
        let fall = 1.0 - ease_smooth((bt - (self.gust_dur - 1.6)) / 1.6);
        if bt < 0.8 {
            0.14 * ease_smooth(bt / 0.8) * fall
        } else {
            ease_smooth((bt - 0.8) / 1.1) * fall
        }
    }
}

impl Scene for Ribbons {
    fn name(&self) -> &'static str {
        "ribbons"
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
        let (wf, hf) = (w as f32, h as f32);

        // --- gust front: anticipation -> payoff -> decay ---
        self.next_gust -= dt;
        if self.next_gust <= 0.0 && self.gust_t < 0.0 {
            self.gust_t = 0.0;
            self.gust_dur = self.rng.random_range(4.0..5.5);
            // mostly horizontal fronts, slight vertical slant, either way
            let slant = self.rng.random_range(-0.35..0.35);
            let dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
            self.gust_dir = (dir, slant);
            self.next_gust = self.rng.random_range(20.0..30.0);
        }
        if self.gust_t >= 0.0 {
            self.gust_t += dt;
            if self.gust_t >= self.gust_dur {
                self.gust_t = -1.0;
            }
        }
        let gust = self.gust_env();
        let trembling = self.gust_t >= 0.0 && self.gust_t < 0.8;

        // --- simulate the chains ---
        for r in &mut self.ribbons {
            r.retarget -= dt;
            if r.retarget <= 0.0 {
                // wander biased downwind of the ribbon's own drift, targets
                // kept inside the frame so the bright head stays in view
                r.tx = self.rng.random_range(wf * 0.08..wf * 0.92);
                if self.rng.random::<f32>() < 0.6 {
                    r.tx = if r.drift > 0.0 { wf * 0.72 } else { wf * 0.28 };
                }
                r.ty = self.rng.random_range(hf * 0.12..hf * 0.88);
                r.retarget = self.rng.random_range(3.0..6.0);
            }
            // a head pushed out of frame steers back hard: silk may slide
            // off-screen and re-enter, but never camps out of view
            let head_off = r.pts[0].x < 0.0
                || r.pts[0].x > wf
                || r.pts[0].y < 0.0
                || r.pts[0].y > hf;

            let n = r.pts.len();
            for i in 0..n {
                let (px, py) = (r.pts[i].x, r.pts[i].y);
                // fbm wind: direction and strength wander over the field
                let na = fbm(px * 0.03 + r.phase, t * 0.25, 2, r.seed);
                let nm = fbm(py * 0.04, t * 0.2 + r.phase, 2, r.seed ^ 0x9e37);
                let ang = na * TAU * 1.6 + r.phase;
                let mag = (30.0 + 60.0 * nm) * (1.0 + gust * 1.5);
                let mut fx = ang.cos() * mag + r.drift * 26.0;
                let mut fy = ang.sin() * mag * 0.7;
                // gust payoff: shared shove downwind plus a lift
                fx += self.gust_dir.0 * gust * 320.0;
                fy += self.gust_dir.1 * gust * 220.0 - gust * 110.0;
                if trembling {
                    // anticipation shiver running down the silk
                    fy += (t * 24.0 + r.phase * 7.0 + i as f32 * 0.6).sin() * 55.0;
                }
                if i == 0 {
                    // head steers toward its wander target, spring-damped
                    let (vx, vy) = r.pts[0].velocity();
                    let (vx, vy) = (vx / dt.max(1e-3), vy / dt.max(1e-3));
                    let (k, c) = if head_off { (8.0, 4.5) } else { (2.2, 3.0) };
                    fx += physics::spring_damper(px, vx, r.tx, k, c);
                    fy += physics::spring_damper(py, vy, r.ty, k, c);
                }
                let f = Forces {
                    drag: 1.6,
                    ax: fx,
                    ay: fy,
                    ..Default::default()
                };
                physics::integrate(&mut r.pts[i], &f, dt);
            }

            // distance constraints keep the chain a smooth silk curve
            for _ in 0..3 {
                for i in 1..n {
                    let (dx, dy) = (r.pts[i].x - r.pts[i - 1].x, r.pts[i].y - r.pts[i - 1].y);
                    let d = (dx * dx + dy * dy).sqrt().max(1e-4);
                    let diff = (d - SEG) / d;
                    let (cx, cy) = (dx * diff, dy * diff);
                    r.pts[i - 1].x += cx * 0.4;
                    r.pts[i - 1].y += cy * 0.4;
                    r.pts[i].x -= cx * 0.6;
                    r.pts[i].y -= cy * 0.6;
                }
            }
            // soft padded bounds: ribbons may slide off-screen and re-enter
            for p in &mut r.pts {
                p.x = p.x.clamp(-12.0, wf + 12.0);
                p.y = p.y.clamp(-12.0, hf + 12.0);
                p.px = p.px.clamp(-12.0, wf + 12.0);
                p.py = p.py.clamp(-12.0, hf + 12.0);
            }
        }

        // --- crossing shimmer: different colors meeting bloom additively ---
        // sample every 4th point of each ribbon; near-coincident samples of
        // differently-colored ribbons occasionally seed a spark
        self.samples.clear();
        for (ri, r) in self.ribbons.iter().enumerate() {
            for p in r.pts.iter().step_by(4) {
                self.samples.push((ri, p.x, p.y));
            }
        }
        for a in 0..self.samples.len() {
            for b in (a + 1)..self.samples.len() {
                let (ra, xa, ya) = self.samples[a];
                let (rb, xb, yb) = self.samples[b];
                if ra == rb || self.ribbons[ra].color == self.ribbons[rb].color {
                    continue;
                }
                let (dx, dy) = (xb - xa, yb - ya);
                if dx * dx + dy * dy < 6.25
                    && self.sparks.len() < MAX_SPARKS
                    && self.rng.random::<f32>() < 0.15
                {
                    let mix = lerp(self.ribbons[ra].color, self.ribbons[rb].color, 0.5);
                    self.sparks.push(Spark {
                        x: (xa + xb) * 0.5,
                        y: (ya + yb) * 0.5,
                        age: 0.0,
                        color: lerp(mix, (255, 255, 255), 0.45),
                    });
                }
            }
        }

        // --- draw ---
        canvas.clear((0, 0, 0));

        // gust payoff: a faint light wash races across the frame along the
        // front, brightest at the billow and gone by the settle
        if gust > 0.01 {
            let (gdx, gdy) = self.gust_dir;
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for &(cx, cy) in &[(0.0, 0.0), (wf, 0.0), (0.0, hf), (wf, hf)] {
                let p = cx * gdx + cy * gdy;
                lo = lo.min(p);
                hi = hi.max(p);
            }
            let front = lo + (hi - lo) * ease_smooth(self.gust_t / self.gust_dur);
            let wash = lerp(self.palette[0], (255, 255, 255), 0.5);
            for y in 0..h {
                let shim = 0.75 + 0.25 * (y as f32 * 0.4 - t * 3.0).sin();
                for x in 0..w {
                    let d = (x as f32 * gdx + y as f32 * gdy - front) / 7.0;
                    let band = (-d * d).exp();
                    if band > 0.03 {
                        canvas.add(x as i32, y as i32, scale(wash, band * gust * 0.10 * shim));
                    }
                }
            }
        }

        // background plane: dust motes drifting on the wind; the gust front
        // shoves them downwind so the whole frame feels the event
        let mote_tint = scale(self.palette[0], 0.55);
        for m in &mut self.motes {
            let sway = (t * 0.4 + m.phase).sin();
            m.x += (m.drift + self.gust_dir.0 * gust * 45.0) * dt;
            m.y += (sway * 1.2 + self.gust_dir.1 * gust * 30.0 - gust * 8.0) * dt;
            if m.x < -2.0 {
                m.x = wf + 1.0;
            } else if m.x > wf + 2.0 {
                m.x = -1.0;
            }
            if m.y < -2.0 {
                m.y = hf + 1.0;
            } else if m.y > hf + 2.0 {
                m.y = -1.0;
            }
            let tw = 0.7 + 0.3 * (t * 0.9 + m.phase * 3.0).sin();
            canvas.add(
                m.x as i32,
                m.y as i32,
                scale(mote_tint, m.mag * tw * (1.0 + gust * 0.8)),
            );
        }

        for r in &self.ribbons {
            let base = 0.22 + 0.78 * r.depth;
            let near = r.depth > 0.72;
            let n = r.pts.len();
            for i in 0..n {
                // bright leading edge, silky taper down the body
                let taper = 1.0 - ease_smooth(i as f32 / n as f32);
                let b = base * (0.18 + 0.82 * taper) * (1.0 + gust * 0.25);
                let mut c = scale(r.color, b);
                if i < 3 {
                    // leading edge catches the light
                    c = lerp(c, (255, 255, 255), 0.45 * (1.0 - i as f32 / 3.0));
                }
                let (px, py) = (r.pts[i].x as i32, r.pts[i].y as i32);
                canvas.set(px, py, c);
                if near {
                    // soft band: sheen bleeding off the core pixel
                    canvas.add(px, py - 1, scale(c, 0.35));
                    canvas.add(px, py + 1, scale(c, 0.35));
                    canvas.add(px - 1, py, scale(c, 0.2));
                    canvas.add(px + 1, py, scale(c, 0.2));
                }
                // midpoint fill keeps the band continuous between segments
                if i + 1 < n {
                    let mx = ((r.pts[i].x + r.pts[i + 1].x) * 0.5) as i32;
                    let my = ((r.pts[i].y + r.pts[i + 1].y) * 0.5) as i32;
                    canvas.set(mx, my, scale(c, 0.85));
                    if near {
                        canvas.add(mx, my + 1, scale(c, 0.25));
                    }
                }
            }
            // leading edge glows, bleeding light into the dark around it
            let gi = (0.16 + 0.30 * gust) * r.depth;
            glow(
                canvas,
                r.pts[0].x as i32,
                r.pts[0].y as i32,
                if near { 2 } else { 1 },
                r.color,
                gi,
            );
        }

        // shimmers: brief additive bloom, white-hot heart cooling fast
        self.sparks.retain_mut(|s| {
            s.age += dt;
            let life = 0.55;
            if s.age >= life {
                return false;
            }
            let a = 1.0 - ease_smooth(s.age / life);
            let (ix, iy) = (s.x as i32, s.y as i32);
            canvas.add(ix, iy, scale(s.color, a * 0.85));
            canvas.add(ix + 1, iy, scale(s.color, a * 0.35));
            canvas.add(ix - 1, iy, scale(s.color, a * 0.35));
            canvas.add(ix, iy + 1, scale(s.color, a * 0.35));
            canvas.add(ix, iy - 1, scale(s.color, a * 0.35));
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn checksum(s: &mut Ribbons, steps: usize) -> u64 {
        let mut c = Canvas::new(80, 40);
        let mut h = 0xcbf29ce484222325u64;
        for _ in 0..steps {
            s.update(1.0 / 60.0, &mut c);
        }
        for y in 0..40 {
            for x in 0..80 {
                let (r, g, b) = c.get(x, y).color;
                h ^= ((r as u64) << 16) ^ ((g as u64) << 8) ^ b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
        h
    }

    #[test]
    fn same_seed_is_deterministic() {
        let mut a = Ribbons::new(StdRng::seed_from_u64(777), None, Detail::Medium);
        let mut b = Ribbons::new(StdRng::seed_from_u64(777), None, Detail::Medium);
        assert_eq!(checksum(&mut a, 90), checksum(&mut b, 90));
    }

    #[test]
    fn themes_resolve_and_unknown_falls_back_to_silk() {
        let silk = Ribbons::new(StdRng::seed_from_u64(1), None, Detail::Medium).palette;
        let ember = Ribbons::new(StdRng::seed_from_u64(1), Some("ember"), Detail::Medium).palette;
        let ocean = Ribbons::new(StdRng::seed_from_u64(1), Some("ocean"), Detail::Medium).palette;
        let bogus = Ribbons::new(StdRng::seed_from_u64(1), Some("bogus"), Detail::Medium).palette;
        assert_ne!(silk, ember);
        assert_ne!(silk, ocean);
        assert_ne!(ember, ocean);
        assert_eq!(silk, bogus);
        let s = Ribbons::new(StdRng::seed_from_u64(1), Some("ember"), Detail::Medium);
        assert_eq!(s.name(), "ribbons");
    }

    #[test]
    fn resize_and_zero_size_do_not_panic() {
        let mut s = Ribbons::new(StdRng::seed_from_u64(9), None, Detail::High);
        let mut c = Canvas::new(40, 12);
        for _ in 0..10 {
            s.update(1.0 / 30.0, &mut c);
        }
        assert!(!s.ribbons.is_empty());
        c.resize(400, 200);
        for _ in 0..10 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(0, 0);
        s.update(1.0 / 30.0, &mut c);
        assert!(s.ribbons.is_empty());
    }

    #[test]
    fn stays_bounded_under_fast_forward_and_gusts_fire() {
        let mut s = Ribbons::new(StdRng::seed_from_u64(31), None, Detail::High);
        let mut c = Canvas::new(120, 60);
        // 90s of animation in big steps: gusts must fire and nothing explodes
        let mut saw_gust = false;
        for _ in 0..900 {
            s.update(5.0, &mut c);
            if s.gust_env() > 0.5 {
                saw_gust = true;
            }
        }
        assert!(saw_gust, "a gust front should sweep through within 90s");
        for r in &s.ribbons {
            assert!(r.pts.len() >= 12);
            for p in &r.pts {
                assert!(p.x.is_finite() && p.y.is_finite(), "chain point blew up");
                assert!(p.x >= -12.0 && p.x <= 132.0 && p.y >= -12.0 && p.y <= 72.0);
            }
        }
        assert!(s.sparks.len() <= MAX_SPARKS);
        // ribbons are still painting light
        let lit = (0..120)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| c.get(x, y).color != (0, 0, 0))
            .count();
        assert!(lit > 50, "ribbons should be visible, {lit} px lit");
    }
}
