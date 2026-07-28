//! Nexus: glowing nodes linked into a slowly drifting graph on pure black.
//! Nodes wander with eased spring-damper steering; faint edges connect
//! neighbors; data pulses ride the edges node-to-node, flaring their
//! destination (and its incident edges) on arrival. Every ~12-20s a surge
//! cascade-lights a whole neighborhood: anticipation, payoff, decay.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use crate::physics;
use rand::{rngs::StdRng, RngExt};

/// Surge pre-glow: the epicenter brightens before the wave fires.
const ANTICIPATE: f32 = 0.9;

struct Node {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// wander target the spring-damper steers toward
    tx: f32,
    ty: f32,
    retarget: f32,
    /// 0 far (dim, slow) .. 1 near (bright, blooming)
    depth: f32,
    phase: f32,
    /// arrival / surge energy; decays exponentially
    flare: f32,
}

struct Pulse {
    a: usize,
    b: usize,
    /// progress 0..1 along the edge a→b (eased: fast mid-edge)
    t: f32,
    /// progress units per second (travel speed / edge length)
    speed: f32,
}

/// Very faint static dust: the far background plane behind the graph.
struct Dust {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
}

struct Surge {
    /// (node index, cascade delay) — BFS rings from the epicenter
    wave: Vec<(usize, f32)>,
    epicenter: usize,
    t: f32,
    dur: f32,
}

pub struct Nexus {
    rng: StdRng,
    detail: Detail,
    node_c: (u8, u8, u8),
    edge_c: (u8, u8, u8),
    pulse_c: (u8, u8, u8),
    nodes: Vec<Node>,
    pulses: Vec<Pulse>,
    dust: Vec<Dust>,
    /// adjacency scratch, rebuilt each frame (no per-frame alloc)
    edges: Vec<(usize, usize, f32)>,
    surge: Option<Surge>,
    next_surge: f32,
    next_pulse: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Nexus {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        // (node, edge, pulse); edges stay very dim so the accents pop
        let (node_c, edge_c, pulse_c) = match theme {
            Some("amber") => ((235, 175, 85), (75, 52, 22), (255, 235, 195)),
            Some("violet") => ((185, 125, 235), (56, 38, 80), (240, 220, 255)),
            Some("mono") => ((195, 200, 210), (52, 55, 62), (245, 248, 255)),
            _ => ((110, 215, 235), (26, 64, 80), (215, 245, 255)), // cyan
        };
        Nexus {
            rng,
            detail,
            node_c,
            edge_c,
            pulse_c,
            nodes: Vec::new(),
            pulses: Vec::new(),
            dust: Vec::new(),
            edges: Vec::new(),
            surge: None,
            next_surge: 8.0,
            next_pulse: 0.5,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn spawn_node(&mut self, w: usize, h: usize) -> Node {
        let (wf, hf) = (w as f32, h as f32);
        Node {
            x: self.rng.random_range(wf * 0.05..wf * 0.95),
            y: self.rng.random_range(hf * 0.05..hf * 0.95),
            vx: 0.0,
            vy: 0.0,
            tx: self.rng.random_range(wf * 0.05..wf * 0.95),
            ty: self.rng.random_range(hf * 0.05..hf * 0.95),
            retarget: self.rng.random_range(4.0..10.0),
            depth: self.rng.random::<f32>(),
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            flare: 0.0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let n = self
            .detail
            .scale((w * h) as f32 / 450.0 * density_for(w, h), 10)
            .clamp(10, 150);
        self.nodes = (0..n).map(|_| self.spawn_node(w, h)).collect();
        self.pulses.clear();
        self.surge = None;
        // far dust: sparse, theme-tinted, barely above black
        let nd = self
            .detail
            .scale((w * h) as f32 / 260.0 * density_for(w, h), 8)
            .clamp(8, 220);
        self.dust = (0..nd)
            .map(|_| Dust {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                mag: self.rng.random_range(0.04..0.11),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            })
            .collect();
    }
}

impl Scene for Nexus {
    fn name(&self) -> &'static str {
        "nexus"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        // cap dt: fast-forward steps must not explode the springs
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let (wf, hf) = (w as f32, h as f32);
        let n = self.nodes.len();

        // --- node drift: eased spring-damper steering toward wander targets
        for nd in &mut self.nodes {
            nd.retarget -= dt;
            let close = (nd.tx - nd.x).powi(2) + (nd.ty - nd.y).powi(2) < 4.0;
            if nd.retarget <= 0.0 || close {
                nd.tx = self.rng.random_range(wf * 0.05..wf * 0.95);
                nd.ty = self.rng.random_range(hf * 0.05..hf * 0.95);
                nd.retarget = self.rng.random_range(5.0..11.0);
            }
            // near nodes steer a touch more crisply
            let stiff = 1.6 + nd.depth;
            let ax = physics::spring_damper(nd.x, nd.vx, nd.tx, stiff, 2.4);
            let ay = physics::spring_damper(nd.y, nd.vy, nd.ty, stiff, 2.4);
            nd.vx += ax * dt;
            nd.vy += ay * dt;
            let vmax = 3.0 + nd.depth * 5.0;
            let s = (nd.vx * nd.vx + nd.vy * nd.vy).sqrt();
            if s > vmax {
                nd.vx *= vmax / s;
                nd.vy *= vmax / s;
            }
            nd.x = (nd.x + nd.vx * dt).clamp(0.5, wf - 0.5);
            nd.y = (nd.y + nd.vy * dt).clamp(0.5, hf - 0.5);
            nd.flare *= (-2.6 * dt).exp();
        }

        // --- adjacency: edges between nodes within a distance threshold
        // threshold keeps ~constant degree at any canvas size, capped so
        // edges stay local
        let thr = (((w * h) as f32 / n as f32).sqrt() * 1.35).clamp(7.0, 26.0);
        let thr2 = thr * thr;
        // reuse the adjacency buffer: take it, refill, put it back below
        let mut edges = std::mem::take(&mut self.edges);
        edges.clear();
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = self.nodes[j].x - self.nodes[i].x;
                let dy = self.nodes[j].y - self.nodes[i].y;
                let d2 = dx * dx + dy * dy;
                if d2 < thr2 {
                    edges.push((i, j, d2.sqrt()));
                }
            }
        }

        // --- surge event: a neighborhood cascade every ~12-20s
        self.next_surge -= dt;
        if self.next_surge <= 0.0 && self.surge.is_none() && n > 0 {
            let center = self.rng.random_range(0..n);
            // BFS rings over the current edges: ring 1 direct, ring 2 2-hop
            let mut ring = vec![usize::MAX; n];
            ring[center] = 0;
            for &(a, b, _) in &edges {
                if a == center {
                    ring[b] = 1;
                } else if b == center {
                    ring[a] = 1;
                }
            }
            for &(a, b, _) in &edges {
                if ring[a] == 1 && ring[b] == usize::MAX {
                    ring[b] = 2;
                }
                if ring[b] == 1 && ring[a] == usize::MAX {
                    ring[a] = 2;
                }
            }
            let mut wave = vec![(center, ANTICIPATE)];
            for (i, &r) in ring.iter().enumerate() {
                if r == 1 {
                    wave.push((i, ANTICIPATE + 0.25));
                } else if r == 2 {
                    wave.push((i, ANTICIPATE + 0.5));
                }
            }
            self.surge = Some(Surge {
                wave,
                epicenter: center,
                t: 0.0,
                dur: ANTICIPATE + 0.5 + 0.8,
            });
            self.next_surge = self.rng.random_range(12.0..20.0);
        }
        if let Some(mut s) = self.surge.take() {
            s.t += dt;
            // anticipation: epicenter pre-glow ramps up before the wave
            if s.t < ANTICIPATE {
                let e = s.epicenter;
                self.nodes[e].flare = self.nodes[e]
                    .flare
                    .max(ease_smooth(s.t / ANTICIPATE) * 0.55);
            }
            // payoff: each ring flares as its delay is crossed
            for &(idx, delay) in &s.wave {
                if s.t >= delay && s.t - dt < delay {
                    self.nodes[idx].flare = (self.nodes[idx].flare + 1.25).min(1.6);
                }
            }
            if s.t < s.dur {
                self.surge = Some(s);
            } else {
                // decay tail: pulses radiate out of the lit neighborhood
                let lit: Vec<usize> = s.wave.iter().map(|&(i, _)| i).collect();
                for _ in 0..self.rng.random_range(2..=4) {
                    let cand: Vec<(usize, usize, f32)> = edges
                        .iter()
                        .filter(|&&(a, b, _)| lit.contains(&a) && lit.contains(&b))
                        .copied()
                        .collect();
                    if !cand.is_empty() {
                        let (a, b, d) = cand[self.rng.random_range(0..cand.len())];
                        self.pulses.push(Pulse {
                            a,
                            b,
                            t: 0.0,
                            speed: self.rng.random_range(12.0..20.0) / d.max(1.0),
                        });
                    }
                }
            }
        }

        // --- pulses spawn on random edges
        self.next_pulse -= dt;
        if self.next_pulse <= 0.0 {
            let cap = (n / 5).max(3);
            if !edges.is_empty() && self.pulses.len() < cap {
                let &(a, b, d) = &edges[self.rng.random_range(0..edges.len())];
                self.pulses.push(Pulse {
                    a,
                    b,
                    t: 0.0,
                    speed: self.rng.random_range(10.0..18.0) / d.max(1.0),
                });
            }
            self.next_pulse = self.rng.random_range(0.25..0.7);
        }

        // --- pulses travel; arrivals flare the destination and maybe chain
        let mut pulses = std::mem::take(&mut self.pulses);
        for mut p in pulses.drain(..) {
            // eased travel: launch soft, run fast mid-edge, ease into arrival
            let ease = 0.45 + 1.1 * (p.t * std::f32::consts::PI).sin();
            p.t += p.speed * ease * dt;
            if p.t < 1.0 {
                self.pulses.push(p);
                continue;
            }
            let arrived = p.b;
            self.nodes[arrived].flare = (self.nodes[arrived].flare + 1.1).min(1.6);
            // chain onward to a random neighbor (not back where it came from)
            if self.rng.random::<f32>() < 0.65 {
                let opts: Vec<(usize, f32)> = edges
                    .iter()
                    .filter_map(|&(a, b, d)| {
                        if a == p.b && b != p.a {
                            Some((b, d))
                        } else if b == p.b && a != p.a {
                            Some((a, d))
                        } else {
                            None
                        }
                    })
                    .collect();
                if !opts.is_empty() {
                    let (nb, d) = opts[self.rng.random_range(0..opts.len())];
                    p.a = arrived;
                    p.b = nb;
                    p.t = 0.0;
                    p.speed = self.rng.random_range(10.0..18.0) / d.max(1.0);
                    self.pulses.push(p);
                }
            }
        }

        // ================= draw =================
        canvas.clear((0, 0, 0));

        // far plane: static dust, barely above black, slow twinkle
        for ds in &self.dust {
            let tw = 0.7 + 0.3 * (self.t * 0.5 + ds.phase).sin();
            canvas.set_f(ds.x, ds.y, scale(self.node_c, ds.mag * tw));
        }

        // edges: faint web; a flared node brightens its incident edges
        for &(a, b, d) in &edges {
            let (na, nb) = (&self.nodes[a], &self.nodes[b]);
            let depth = (na.depth + nb.depth) * 0.5;
            let flare = na.flare + nb.flare;
            let bright = (0.16 + 0.5 * (1.0 - d / thr))
                * (0.35 + 0.65 * depth)
                * (1.0 + flare * 2.2).min(4.0);
            let col = scale(self.edge_c, bright);
            let steps = (d as i32).max(1);
            for i in 0..=steps {
                let k = i as f32 / steps as f32;
                canvas.add(
                    (na.x + (nb.x - na.x) * k) as i32,
                    (na.y + (nb.y - na.y) * k) as i32,
                    col,
                );
            }
        }

        // pulses: bright head + short fading trail riding the edge
        for p in &self.pulses {
            let (na, nb) = (&self.nodes[p.a], &self.nodes[p.b]);
            let d = ((nb.x - na.x).powi(2) + (nb.y - na.y).powi(2)).sqrt().max(1.0);
            // lifecycle envelope: grow in after (re)spawn, no pop
            let env = ease_smooth((p.t / 0.15).min(1.0));
            for i in (1..=4).rev() {
                let tt = p.t - i as f32 * 1.2 / d;
                if tt < 0.0 {
                    continue;
                }
                let k = 0.55 * (1.0 - i as f32 / 5.0) * env;
                canvas.add(
                    (na.x + (nb.x - na.x) * tt) as i32,
                    (na.y + (nb.y - na.y) * tt) as i32,
                    scale(self.pulse_c, k),
                );
            }
            let px = (na.x + (nb.x - na.x) * p.t) as i32;
            let py = (na.y + (nb.y - na.y) * p.t) as i32;
            canvas.set(px, py, scale(self.pulse_c, 0.35 + 0.65 * env));
            glow(canvas, px, py, 2, self.pulse_c, 0.35 * env);
        }

        // nodes: depth-scaled cores, sine breathing, flare bloom
        for nd in &self.nodes {
            let breathe = 0.8 + 0.2 * (self.t * (0.6 + nd.depth) + nd.phase).sin();
            let bright = (0.4 + 0.6 * nd.depth) * breathe * (1.0 + nd.flare * 1.8);
            let core = scale(self.node_c, bright.min(1.6));
            let (x, y) = (nd.x as i32, nd.y as i32);
            canvas.set(x, y, lerp(core, (255, 255, 255), (nd.flare * 0.35).min(0.5)));
            if nd.depth > 0.66 {
                // near plane: cross bloom into the neighbors
                let halo = scale(core, 0.5);
                canvas.add(x + 1, y, halo);
                canvas.add(x - 1, y, halo);
                canvas.add(x, y + 1, halo);
                canvas.add(x, y - 1, halo);
            }
            if nd.flare > 0.12 {
                // flare bleeds light into the surroundings
                let r = (1.0 + nd.flare * 2.0) as i32;
                glow(canvas, x, y, r, self.pulse_c, (nd.flare * 0.45).min(0.7));
            }
        }

        // return the adjacency scratch buffer for the next frame
        self.edges = edges;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn deterministic_same_seed_same_canvas() {
        let run = || {
            let mut s = Nexus::new(StdRng::seed_from_u64(7), None, Detail::Medium);
            let mut c = Canvas::new(80, 40);
            for _ in 0..120 {
                s.update(1.0 / 60.0, &mut c);
            }
            let mut v = Vec::new();
            for y in 0..40 {
                for x in 0..80 {
                    v.push(c.get(x, y).color);
                }
            }
            v
        };
        assert_eq!(run(), run(), "same seed must produce identical frames");
    }

    #[test]
    fn resize_reinits_without_panic() {
        let mut s = Nexus::new(StdRng::seed_from_u64(3), Some("amber"), Detail::High);
        let mut c = Canvas::new(80, 40);
        for _ in 0..30 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(40, 12);
        for _ in 0..30 {
            s.update(1.0 / 30.0, &mut c);
        }
        c.resize(160, 60);
        for _ in 0..30 {
            s.update(1.0 / 30.0, &mut c);
        }
        assert!(s.nodes.iter().all(|nd| {
            nd.x >= 0.0 && nd.x <= 160.0 && nd.y >= 0.0 && nd.y <= 60.0 && nd.x.is_finite()
        }));
    }

    #[test]
    fn themes_resolve_and_paint() {
        for theme in [None, Some("cyan"), Some("amber"), Some("violet"), Some("mono")] {
            let mut s = Nexus::new(StdRng::seed_from_u64(9), theme, Detail::Medium);
            assert_eq!(s.name(), "nexus");
            let mut c = Canvas::new(60, 30);
            for _ in 0..60 {
                s.update(1.0 / 30.0, &mut c);
            }
            let lit = (0..60)
                .flat_map(|x| (0..30).map(move |y| (x, y)))
                .filter(|&(x, y)| c.get(x, y).color != (0, 0, 0))
                .count();
            assert!(lit > 30, "theme {theme:?} should paint nodes and edges");
        }
    }

    #[test]
    fn large_dt_steps_stay_stable() {
        let mut s = Nexus::new(StdRng::seed_from_u64(21), None, Detail::Medium);
        let mut c = Canvas::new(100, 50);
        for _ in 0..200 {
            s.update(0.5, &mut c); // fast-forward sized steps
        }
        for nd in &s.nodes {
            assert!(nd.x.is_finite() && nd.y.is_finite());
            assert!(nd.x >= 0.0 && nd.x <= 100.0);
            assert!(nd.y >= 0.0 && nd.y <= 50.0);
            assert!(nd.flare >= 0.0 && nd.flare <= 1.7);
        }
    }
}
