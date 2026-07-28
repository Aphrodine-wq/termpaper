//! Windows 95 pipes homage: colorful pipe heads crawl orthogonally over a
//! dim riveted backplane, turning with elbow joints, fading out and
//! respawning when stuck or finished. Fresh pipes ease in, trails shade
//! darker with age, and a light pulse periodically flushes along a pipe.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

pub type Dir = (i32, i32);

/// Rotate a direction 90°. `left` picks the side.
pub fn perpendicular(d: Dir, left: bool) -> Dir {
    if left {
        (d.1, -d.0)
    } else {
        (-d.1, d.0)
    }
}

fn color_set(theme: Option<&str>) -> &'static [(u8, u8, u8)] {
    match theme {
        Some("pastel") => &[
            (240, 150, 150),
            (150, 230, 160),
            (150, 180, 240),
            (240, 225, 140),
            (220, 160, 235),
            (150, 230, 230),
        ],
        Some("mono") => &[
            (60, 200, 90),
            (40, 160, 70),
            (90, 220, 120),
            (120, 200, 120),
            (70, 180, 80),
        ],
        Some("hotmetal") => &[
            (255, 90, 40),
            (255, 140, 30),
            (230, 60, 30),
            (255, 190, 60),
            (200, 40, 40),
        ],
        _ => &[
            (232, 60, 60),  // red
            (70, 200, 70),  // green
            (70, 110, 235), // blue
            (235, 205, 55), // yellow
            (185, 80, 220), // purple
            (60, 205, 215), // cyan
            (240, 140, 45), // orange
        ],
    }
}

#[derive(Clone)]
struct Pipe {
    x: i32,
    y: i32,
    dir: Dir,
    color: (u8, u8, u8),
    segments: usize,
    cells: Vec<(i32, i32)>,
    joints: Vec<(i32, i32)>,
    fade: f32,
    /// grow-in envelope 0→1 for freshly spawned pipes
    fade_in: f32,
    alive: bool,
    speed: f32,
    acc: f32,
    /// end-cap position once the pipe terminates
    cap: Option<(i32, i32)>,
    t: f32,
    /// flush pulse: index along `cells` the light front has reached
    pulse: Option<f32>,
    next_pulse: f32,
}

impl Pipe {
    fn dummy() -> Self {
        Pipe {
            x: 0,
            y: 0,
            dir: (1, 0),
            color: (0, 0, 0),
            segments: 0,
            cells: Vec::new(),
            joints: Vec::new(),
            fade: 0.0,
            fade_in: 0.0,
            alive: false,
            speed: 1.0,
            acc: 0.0,
            cap: None,
            t: 0.0,
            pulse: None,
            next_pulse: 0.0,
        }
    }
}

pub struct Pipes {
    rng: StdRng,
    detail: Detail,
    colors: &'static [(u8, u8, u8)],
    pipes: Vec<Pipe>,
    /// occupancy: which pipe owns a cell (usize::MAX = free)
    owner: Vec<usize>,
    step_acc: f32,
    w: usize,
    h: usize,
}

impl Pipes {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        Pipes {
            rng,
            detail,
            colors: color_set(theme),
            pipes: Vec::new(),
            owner: Vec::new(),
            step_acc: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn spawn_pipe(rng: &mut StdRng, colors: &[(u8, u8, u8)], w: usize, h: usize) -> Pipe {
        let dir = match rng.random_range(0..4) {
            0 => (1, 0),
            1 => (-1, 0),
            2 => (0, 1),
            _ => (0, -1),
        };
        Pipe {
            x: rng.random_range(2..(w as i32 - 2).max(3)),
            y: rng.random_range(2..(h as i32 - 2).max(3)),
            dir,
            color: colors[rng.random_range(0..colors.len())],
            segments: rng.random_range(100..300),
            cells: Vec::new(),
            joints: Vec::new(),
            fade: 1.0,
            fade_in: 0.0,
            alive: true,
            speed: rng.random_range(0.7..1.3),
            acc: 0.0,
            cap: None,
            t: 0.0,
            pulse: None,
            next_pulse: rng.random_range(4.0..12.0),
        }
    }

    fn free(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.owner[y as usize * self.w + x as usize] == usize::MAX
    }

    /// One grid step for pipe `i`.
    fn step_pipe(&mut self, i: usize) {
        let (w, h) = (self.w, self.h);
        // take the pipe out so we can borrow self freely
        let mut p = std::mem::replace(&mut self.pipes[i], Pipe::dummy());

        if !p.alive {
            p.fade -= 0.03;
            if p.fade <= 0.0 {
                for &(cx, cy) in &p.cells {
                    self.owner[cy as usize * w + cx as usize] = usize::MAX;
                }
                p = Self::spawn_pipe(&mut self.rng, self.colors, w, h);
            }
            self.pipes[i] = p;
            return;
        }

        // decide direction: occasional voluntary turn, forced turn when blocked
        let blocked = !self.free(p.x + p.dir.0, p.y + p.dir.1);
        if blocked || self.rng.random::<f32>() < 0.07 {
            let left_first = self.rng.random::<bool>();
            let d1 = perpendicular(p.dir, left_first);
            let d2 = perpendicular(p.dir, !left_first);
            let new_dir = if self.free(p.x + d1.0, p.y + d1.1) {
                Some(d1)
            } else if self.free(p.x + d2.0, p.y + d2.1) {
                Some(d2)
            } else {
                None
            };
            match new_dir {
                Some(d) => {
                    // elbow joint at the corner cell
                    p.joints.push((p.x, p.y));
                    p.dir = d;
                }
                None => {
                    if blocked {
                        // dead end: glowing cap, then fade out
                        p.alive = false;
                        p.cap = Some((p.x, p.y));
                        self.pipes[i] = p;
                        return;
                    }
                    // voluntary turn failed: keep going straight
                }
            }
        }

        p.x += p.dir.0;
        p.y += p.dir.1;
        p.cells.push((p.x, p.y));
        self.owner[p.y as usize * w + p.x as usize] = i;
        p.segments -= 1;
        if p.segments == 0 {
            p.alive = false;
            p.cap = Some((p.x, p.y));
        }
        self.pipes[i] = p;
    }
}

impl Scene for Pipes {
    fn name(&self) -> &'static str {
        "pipes"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);

        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.owner = vec![usize::MAX; w * h];
            let cs = self.colors;
            self.pipes = (0..self.detail.scale(5.0 * density_for(w, h), 2))
                .map(|_| Self::spawn_pipe(&mut self.rng, cs, w, h))
                .collect();
            self.step_acc = 0.0;
        }

        // crawl: ~45 cells/s, each pipe with its own pace
        for i in 0..self.pipes.len() {
            let speed = self.pipes[i].speed;
            self.pipes[i].t += dt;
            // fresh pipes ease in rather than popping onto the screen
            self.pipes[i].fade_in = (self.pipes[i].fade_in + dt * 1.5).min(1.0);
            self.pipes[i].acc += dt * 45.0 * speed;
            while self.pipes[i].acc >= 1.0 {
                self.pipes[i].acc -= 1.0;
                self.step_pipe(i);
            }
            // flush pulse: anticipation (wait) → payoff (light front races
            // along the run) → decay (front passes the head and dies out)
            if self.pipes[i].alive {
                self.pipes[i].next_pulse -= dt;
                if self.pipes[i].next_pulse <= 0.0 && self.pipes[i].cells.len() > 8 {
                    self.pipes[i].pulse = Some(0.0);
                    self.pipes[i].next_pulse = self.rng.random_range(7.0..16.0);
                }
                if let Some(pos) = self.pipes[i].pulse {
                    let pos = pos + dt * 110.0 * speed;
                    self.pipes[i].pulse = if pos < self.pipes[i].cells.len() as f32 + 3.0 {
                        Some(pos)
                    } else {
                        None
                    };
                }
            } else {
                self.pipes[i].pulse = None;
            }
        }

        // background plane: a dim riveted backplane, far under accent level
        canvas.clear((0, 0, 0));
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                if x % 7 == 3 && y % 7 == 3 {
                    canvas.set(x, y, (12, 14, 18));
                }
            }
        }

        for p in &self.pipes {
            let f = p.fade.max(0.0) * ease_smooth(p.fade_in);
            // trail shading: older segments sit a step deeper
            let n = p.cells.len().max(1) as f32;
            for (ci, &(cx, cy)) in p.cells.iter().enumerate() {
                let age = (ci + 1) as f32 / n; // 1 = newest
                canvas.set(cx, cy, scale(p.color, f * (0.75 + 0.25 * age)));
            }
            // elbow joints: bright rim band
            for &(jx, jy) in &p.joints {
                canvas.set(jx, jy, scale(lerp(p.color, (255, 255, 255), 0.65), f));
            }
            // flush pulse: bright band around the front, bleeding into joints
            if let Some(pos) = p.pulse {
                let pi = pos as i32;
                for (ci, &(cx, cy)) in p.cells.iter().enumerate() {
                    let d = (ci as i32 - pi).abs();
                    if d <= 2 {
                        let k = 0.5 * (1.0 - d as f32 / 3.0) * f;
                        canvas.add(cx, cy, scale(lerp(p.color, (255, 255, 255), 0.4), k));
                    }
                }
            }
            // terminated pipes leave a pulsing glowing cap
            if let Some((cx2, cy2)) = p.cap {
                let pulse = 0.6 + 0.4 * (p.t * 6.0).sin();
                canvas.set(cx2, cy2, scale(lerp(p.color, (255, 255, 255), 0.5), f * pulse));
            }
            // head glows and bleeds light around it, breathing gently
            if p.alive {
                canvas.set(p.x, p.y, lerp(p.color, (255, 255, 255), 0.55));
                let breathe = 0.22 + 0.07 * (p.t * 3.0).sin();
                crate::canvas::glow(canvas, p.x, p.y, 2, p.color, breathe);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn turns_are_perpendicular() {
        let dirs: [Dir; 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for d in dirs {
            for left in [true, false] {
                let n = perpendicular(d, left);
                assert_eq!(d.0 * n.0 + d.1 * n.1, 0, "dot product must be 0");
                assert!(dirs.contains(&n), "must stay a unit axis direction");
            }
        }
        // left and right turns are opposite
        let l = perpendicular((1, 0), true);
        let r = perpendicular((1, 0), false);
        assert_eq!(l, (-r.0, -r.1));
    }

    #[test]
    fn scene_steps_without_panic_and_stays_in_bounds() {
        let mut p = Pipes::new(StdRng::seed_from_u64(99), None, Detail::Medium);
        let mut c = Canvas::new(50, 30);
        for _ in 0..200 {
            p.update(1.0 / 30.0, &mut c);
        }
        for pipe in &p.pipes {
            for &(x, y) in &pipe.cells {
                assert!(x >= 0 && y >= 0 && (x as usize) < 50 && (y as usize) < 30);
            }
        }
    }
}
