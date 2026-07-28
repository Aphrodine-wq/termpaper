//! Traffic: aerial night traffic, long-exposure style. Two mostly-straight
//! highways cross at one clean overpass — wide asphalt bands with dashed
//! lane markings. White headlights one way, red taillights the other;
//! brake waves read as red intensification; one lane closure with merging.
//! Every so often an emergency run races through with alternating strobes
//! and traffic eases onto the shoulder to let it pass.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

/// A highway as a polyline with per-direction lanes.
struct Road {
    pts: Vec<(f32, f32)>,
    /// closed lane region (merging), as s-range on the + direction
    closed_from: f32,
    closed_to: f32,
}

struct Vehicle {
    road: usize,
    s: f32,
    speed: f32,
    dir: f32,
    /// lane offset: -1..1 within the vehicle's carriageway
    lane: f32,
    target_lane: f32,
    bright: f32,
    phase: f32,
}

/// An emergency vehicle racing one full length of a road, strobes on.
struct Emergency {
    road: usize,
    s: f32,
    /// distance traveled this run (lifecycle envelope + despawn)
    dist: f32,
    speed: f32,
    dir: f32,
}

pub struct Traffic {
    rng: StdRng,
    detail: Detail,
    roads: Vec<Road>,
    vehicles: Vec<Vehicle>,
    lens: Vec<f32>,
    emergency: Option<Emergency>,
    next_emergency: f32,
    slick: bool,
    lamp_c: (u8, u8, u8),
    sky: (u8, u8, u8),
    asphalt: (u8, u8, u8),
    /// asphalt half-width in px, scaled to the canvas height
    road_half: f32,
    /// px per lane offset unit, scaled with the road
    lane_w: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Traffic {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (slick, lamp_c, sky, asphalt) = match theme {
            Some("dusk") => (false, (255, 180, 90), (0, 0, 0), (22, 20, 26)),
            Some("rain-slick") => (true, (255, 170, 80), (0, 0, 0), (18, 20, 26)),
            _ => (false, (255, 175, 70), (0, 0, 0), (18, 18, 22)), // night
        };
        Traffic {
            rng,
            detail,
            roads: Vec::new(),
            vehicles: Vec::new(),
            lens: Vec::new(),
            emergency: None,
            next_emergency: 9.0, // first run lands early; then 22-40s apart
            slick,
            lamp_c,
            sky,
            asphalt,
            road_half: 4.0,
            lane_w: 2.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let (wf, hf) = (w as f32, h as f32);
        // road geometry scales with the pane: 2px bands at 40x12, 7px at 100+
        self.road_half = (hf * 0.09).clamp(2.0, 7.0);
        self.lane_w = self.road_half * 0.45;
        // highway 1: gentle S across the middle
        let mut r1 = Vec::new();
        for i in 0..=60 {
            let u = i as f32 / 60.0;
            r1.push((u * wf, hf * 0.60 + (u * 2.6).sin() * hf * 0.08));
        }
        // highway 2: gentle diagonal crossing it at the overpass (~60% x)
        let mut r2 = Vec::new();
        for i in 0..=60 {
            let u = i as f32 / 60.0;
            r2.push((
                wf * (0.15 + u * 0.72),
                hf * (0.08 + u * 0.84) + (u * 2.0).sin() * hf * 0.04,
            ));
        }
        let len1 = poly_len(&r1);
        let len2 = poly_len(&r2);
        self.roads = vec![
            Road {
                pts: r1,
                closed_from: len1 * 0.35,
                closed_to: len1 * 0.55,
            },
            Road {
                pts: r2,
                closed_from: f32::MAX, // no closure on the crossing
                closed_to: f32::MAX,
            },
        ];
        self.lens = vec![len1, len2];

        let n = self.detail.scale((w * h / 200) as f32 * 2.0 * density_for(w, h), 20);
        self.vehicles = (0..n)
            .map(|_| {
                let road = self.rng.random_range(0..2);
                let dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
                let lane = if self.rng.random::<bool>() { 0.5 } else { -0.5 };
                Vehicle {
                    road,
                    s: self.rng.random_range(0.0..self.lens[road]),
                    speed: self.rng.random_range(14.0..24.0),
                    dir,
                    lane,
                    target_lane: lane,
                    bright: self.rng.random_range(0.75..1.0),
                    phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                }
            })
            .collect();
    }

    fn road_at(&self, road: usize, s: f32) -> ((f32, f32), (f32, f32)) {
        road_at(&self.roads, &self.lens, road, s)
    }
}

fn road_at(roads: &[Road], lens: &[f32], road: usize, s: f32) -> ((f32, f32), (f32, f32)) {
    let r = &roads[road];
    let len = lens[road];
    let s = s.rem_euclid(len);
    let mut acc = 0.0;
    for seg in r.pts.windows(2) {
        let seg_len = ((seg[1].0 - seg[0].0).powi(2) + (seg[1].1 - seg[0].1).powi(2)).sqrt();
        if acc + seg_len >= s && seg_len > 1e-3 {
            let f = (s - acc) / seg_len;
            return (
                (
                    seg[0].0 + (seg[1].0 - seg[0].0) * f,
                    seg[0].1 + (seg[1].1 - seg[0].1) * f,
                ),
                ((seg[1].0 - seg[0].0) / seg_len, (seg[1].1 - seg[0].1) / seg_len),
            );
        }
        acc += seg_len;
    }
    (r.pts[0], (1.0, 0.0))
}

fn poly_len(pts: &[(f32, f32)]) -> f32 {
    pts.windows(2)
        .map(|s| ((s[1].0 - s[0].0).powi(2) + (s[1].1 - s[0].1).powi(2)).sqrt())
        .sum()
}

impl Scene for Traffic {
    fn name(&self) -> &'static str {
        "traffic"
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

        // ground with distant city glow at the top
        for y in 0..h {
            let glow_k = (1.0 - y as f32 / h as f32 * 3.0).max(0.0);
            canvas.fill_row(y, lerp(self.sky, (45, 32, 42), glow_k * 0.5));
        }

        // highways: wide asphalt bands, edge lines, dashed center markings
        let rh = self.road_half as i32;
        for r in &self.roads {
            let mut dash = 0usize;
            for seg in r.pts.windows(2) {
                let steps = 3;
                for k in 0..steps {
                    let f = k as f32 / steps as f32;
                    let (px, py) = (
                        seg[0].0 + (seg[1].0 - seg[0].0) * f,
                        seg[0].1 + (seg[1].1 - seg[0].1) * f,
                    );
                    let dlen = ((seg[1].0 - seg[0].0).powi(2) + (seg[1].1 - seg[0].1).powi(2))
                        .sqrt()
                        .max(1e-3);
                    let (dxv, dyv) = ((seg[1].0 - seg[0].0) / dlen, (seg[1].1 - seg[0].1) / dlen);
                    let (pxv, pyv) = (-dyv, dxv);
                    for off in -rh..=rh {
                        let c = if off.abs() == rh {
                            scale(self.asphalt, 2.2) // edge line
                        } else {
                            self.asphalt
                        };
                        canvas.set(
                            (px + pxv * off as f32) as i32,
                            (py + pyv * off as f32) as i32,
                            c,
                        );
                    }
                    // dashed center line
                    if dash % 3 != 2 {
                        canvas.set(px as i32, py as i32, (150, 140, 100));
                    }
                    dash += 1;
                }
            }
        }

        // overpass: the crossing road casts a shadow band on the highway
        // below; pillar glows mark the structure itself
        let s0 = self.lens[0] * 0.55;
        let (cx, cdir) = self.road_at(0, s0);
        let (cx2, _) = self.road_at(1, self.lens[1] * 0.55);
        {
            let (pxv, pyv) = (-cdir.1, cdir.0);
            for ds in -5..=5 {
                let (sp, _) = self.road_at(0, s0 + ds as f32);
                for off in -rh..=rh {
                    let (ix, iy) = (
                        (sp.0 + pxv * off as f32) as i32,
                        (sp.1 + pyv * off as f32) as i32,
                    );
                    let cell = canvas.get(ix, iy).color;
                    canvas.set(ix, iy, scale(cell, 0.5));
                }
            }
        }
        glow(canvas, cx.0 as i32, cx.1 as i32, 1, (60, 55, 45), 0.25);
        glow(canvas, cx2.0 as i32, cx2.1 as i32, 2, (90, 80, 60), 0.3);

        // amber lamps along one shoulder of highway 1, pools of light below
        for i in 0..8 {
            let s = i as f32 / 8.0 * self.lens[0];
            let (pos, dirv) = self.road_at(0, s);
            let (pxv, pyv) = (-dirv.1, dirv.0);
            let blink = 0.8 + 0.2 * (self.t * 0.8 + i as f32).sin();
            let (lx, ly) = (
                (pos.0 + pxv * (self.road_half + 1.0)) as i32,
                (pos.1 + pyv * (self.road_half + 1.0)) as i32,
            );
            canvas.set(lx, ly, scale(self.lamp_c, blink));
            // warm lamplight spilling onto the shoulder
            glow(canvas, lx, ly, 2, self.lamp_c, 0.12 * blink);
        }

        // emergency run: strobes race one full length, traffic pulls over
        self.next_emergency -= dt;
        if self.next_emergency <= 0.0 && self.emergency.is_none() {
            let road = self.rng.random_range(0..self.roads.len());
            let dir = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
            self.emergency = Some(Emergency {
                road,
                s: 0.0,
                dist: 0.0,
                speed: self.rng.random_range(30.0..38.0),
                dir,
            });
        }

        // vehicles: brighter, larger, with speed-scaled streaks
        let roads = &self.roads;
        let lens = &self.lens;
        let slick = self.slick;
        for v in &mut self.vehicles {
            // lane closure: merge left before the closed region
            let road = &roads[v.road];
            if v.dir > 0.0 && v.s > road.closed_from - 15.0 && v.s < road.closed_to {
                v.target_lane = -0.5;
            } else {
                v.target_lane = v.lane;
            }
            // yield: ease onto the shoulder as the emergency run closes in
            if let Some(e) = &self.emergency {
                if e.road == v.road && e.dir == v.dir {
                    let gap = ((v.s - e.s) * e.dir).rem_euclid(lens[v.road]);
                    if gap < 22.0 {
                        v.target_lane = if v.lane >= 0.0 { 0.8 } else { -0.8 };
                    }
                }
            }
            // brake wave: speed eases off approaching the closure's tail,
            // instead of stepping down at one fixed boundary
            let brake_k = if v.dir > 0.0 && v.s > road.closed_from - 18.0 && v.s < road.closed_to
            {
                let span = road.closed_to - road.closed_from + 18.0;
                ease_smooth(((v.s - (road.closed_from - 18.0)) / span).clamp(0.0, 1.0))
            } else {
                0.0
            };
            // gentle per-vehicle breathing so streams don't run metronomic
            let base = v.speed * (1.0 + 0.08 * (self.t * 0.7 + v.phase).sin());
            let speed = base * (1.0 - 0.65 * brake_k);
            v.s = (v.s + speed * dt * v.dir).rem_euclid(lens[v.road]);
            // ease lateral toward target lane
            let cur_lat = v.lane + (v.target_lane - v.lane) * (dt * 2.0).min(1.0);
            v.lane = cur_lat;

            let (pos, dirv) = road_at(roads, lens, v.road, v.s);
            let (pxv, pyv) = (-dirv.1, dirv.0);
            let lat = v.lane * self.lane_w + v.dir * 1.2; // directional carriageway
            let (x, y) = (pos.0 + pxv * lat, pos.1 + pyv * lat);

            let headlight = v.dir > 0.0;
            let braking = brake_k > 0.4;
            let c = if headlight {
                if braking {
                    (255, 200, 120) // brake glow warms the whites
                } else {
                    (255, 248, 230)
                }
            } else if braking {
                (255, 20, 10) // red intensification
            } else {
                (255, 45, 35)
            };
            // 2x2 bright core + speed-scaled streak behind (additive: the
            // long-exposure streams build up where traffic bunches)
            canvas.set_f(x, y, scale(c, v.bright));
            canvas.set_f(x + pxv, y + pyv, scale(c, v.bright * 0.8));
            let streak = (speed * 0.25) as i32 + 2;
            for k in 1..=streak {
                let f = 1.0 - k as f32 / (streak + 1) as f32;
                canvas.add(
                    (x - dirv.0 * v.dir * k as f32 * 1.2) as i32,
                    (y - dirv.1 * v.dir * k as f32 * 1.2) as i32,
                    scale(c, f * f * 0.5 * v.bright),
                );
            }
            // light bleeds onto the asphalt: headlight pools, brake flares
            if headlight {
                if v.bright > 0.88 {
                    glow(canvas, x as i32, y as i32, 1, c, 0.22 * v.bright);
                }
            } else if braking {
                glow(canvas, x as i32, y as i32, 2, c, 0.25);
            }
            if slick {
                canvas.set_f(x, y + 2.0, scale(c, 0.25));
            }
        }

        // advance + draw the emergency run on top of everything
        let mut run_done = false;
        if let Some(e) = &mut self.emergency {
            e.s += e.speed * dt * e.dir;
            e.dist += e.speed * dt;
            run_done = e.dist >= lens[e.road];
        }
        if run_done {
            self.emergency = None;
            self.next_emergency = self.rng.random_range(22.0..40.0);
        }
        if let Some(e) = &self.emergency {
            let len = lens[e.road];
            // lifecycle envelope: fades in and out along the run — no popping
            let env = ease_smooth((e.dist / 6.0).min(1.0))
                * ease_smooth(((len - e.dist) / 10.0).min(1.0));
            let (pos, dirv) = road_at(roads, lens, e.road, e.s);
            let (pxv, pyv) = (-dirv.1, dirv.0);
            let (x, y) = (pos.0 + pxv * e.dir * 1.2, pos.1 + pyv * e.dir * 1.2);
            // alternating strobes + a long-exposure red/blue trail
            let red_on = (self.t * 9.0).sin() > 0.0;
            let strobe = if red_on { (255, 40, 40) } else { (80, 130, 255) };
            glow(canvas, x as i32, y as i32, 3, strobe, 0.5 * env);
            canvas.set_f(x, y, scale((255, 255, 255), env));
            for k in 1..=5 {
                let kc = if (k + red_on as i32) % 2 == 0 {
                    (255, 40, 40)
                } else {
                    (80, 130, 255)
                };
                let f = 1.0 - k as f32 / 6.0;
                canvas.add(
                    (x - dirv.0 * e.dir * k as f32 * 1.6) as i32,
                    (y - dirv.1 * e.dir * k as f32 * 1.6) as i32,
                    scale(kc, f * f * 0.55 * env),
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
    fn vehicles_stay_on_lanes() {
        let mut t = Traffic::new(StdRng::seed_from_u64(21), None, Detail::Medium);
        let mut c = Canvas::new(120, 60);
        t.update(1.0 / 30.0, &mut c);
        for _ in 0..300 {
            t.update(1.0 / 30.0, &mut c);
        }
        for v in &t.vehicles {
            let (pos, _) = t.road_at(v.road, v.s);
            let (_, dirv) = t.road_at(v.road, v.s);
            let (pxv, pyv) = (-dirv.1, dirv.0);
            let lat = v.lane * t.lane_w + v.dir * 1.2;
            let (x, y) = (pos.0 + pxv * lat, pos.1 + pyv * lat);
            let dev = ((x - pos.0).powi(2) + (y - pos.1).powi(2)).sqrt();
            assert!(dev <= t.lane_w + 2.0, "vehicle drifted off lane: {dev}");
            assert!(v.s >= 0.0 && v.s < t.lens[v.road]);
        }
    }

    #[test]
    fn merging_moves_vehicles_to_open_lane() {
        let mut t = Traffic::new(StdRng::seed_from_u64(22), None, Detail::Medium);
        let mut c = Canvas::new(120, 60);
        t.update(1.0 / 30.0, &mut c);
        // park a +dir vehicle just before the closure and let it merge
        let r0_len = t.lens[0];
        for v in &mut t.vehicles {
            if v.road == 0 && v.dir > 0.0 {
                v.s = r0_len * 0.35 - 10.0;
                v.lane = 0.5;
                v.target_lane = 0.5;
                break;
            }
        }
        let before = t
            .vehicles
            .iter()
            .find(|v| v.road == 0 && v.dir > 0.0)
            .map(|v| v.lane);
        for _ in 0..200 {
            t.update(1.0 / 30.0, &mut c);
        }
        let after = t
            .vehicles
            .iter()
            .find(|v| v.road == 0 && v.dir > 0.0)
            .map(|v| v.lane);
        if let (Some(b), Some(a)) = (before, after) {
            assert!(a < b, "vehicle should merge left: {b} → {a}");
        }
    }
}
