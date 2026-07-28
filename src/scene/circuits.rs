//! Circuit board: near-black solder mask with a faint via grid; copper/teal
//! traces crawl orthogonally from pads with glowing tips, vias at turns, and
//! bright data pulses zip along completed traces before they fade and regrow
//! elsewhere. Power surges charge up at a pad, then sweep a trace in a
//! glowing wave that flashes any chip it passes.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const TRACE_COLORS: &[(u8, u8, u8)] = &[
    (205, 125, 55),  // copper
    (45, 195, 170),  // teal
    (190, 100, 60),  // dark copper
    (70, 170, 200),  // blue-teal
];

#[derive(Clone)]
struct Trace {
    path: Vec<(i32, i32)>,
    vias: Vec<(i32, i32)>,
    dir: (i32, i32),
    color: (u8, u8, u8),
    max_len: usize,
    done: bool,
    fade: f32,
    pulses_left: usize,
    pulse_timer: f32,
}

struct Pulse {
    trace: usize,
    pos: f32,
    speed: f32,
}

struct Chip {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    leds: Vec<(i32, i32, f32)>, // dx, dy, phase
    flash: f32,                 // light envelope from a passing surge
}

pub struct Circuits {
    rng: StdRng,
    detail: Detail,
    grid_c: (u8, u8, u8),
    traces: Vec<Trace>,
    pulses: Vec<Pulse>,
    chips: Vec<Chip>,
    surge_trace: i32,
    surge_pos: f32,
    surge_charge: f32,
    next_surge: f32,
    owner: Vec<usize>,
    bg_buf: Vec<(u8, u8, u8)>,
    step_acc: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Circuits {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let grid_c = match theme {
            Some("blueprint") => (18, 36, 80),
            Some("dark") => (14, 18, 20),
            _ => (12, 38, 24), // pcb
        };
        Circuits {
            rng,
            detail,
            grid_c,
            traces: Vec::new(),
            pulses: Vec::new(),
            chips: Vec::new(),
            surge_trace: -1,
            surge_pos: 0.0,
            surge_charge: 0.0,
            next_surge: 14.0,
            owner: Vec::new(),
            bg_buf: Vec::new(),
            step_acc: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn spawn_trace(rng: &mut StdRng, w: usize, h: usize) -> Trace {
        let dir = match rng.random_range(0..4) {
            0 => (1, 0),
            1 => (-1, 0),
            2 => (0, 1),
            _ => (0, -1),
        };
        let (x, y) = (
            rng.random_range(2..(w as i32 - 2).max(3)),
            rng.random_range(2..(h as i32 - 2).max(3)),
        );
        Trace {
            path: vec![(x, y)],
            vias: Vec::new(),
            dir,
            color: TRACE_COLORS[rng.random_range(0..TRACE_COLORS.len())],
            max_len: rng.random_range(20..90),
            done: false,
            fade: 1.0,
            pulses_left: rng.random_range(2..=4),
            pulse_timer: 0.2,
        }
    }

    fn free(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.owner[y as usize * self.w + x as usize] == usize::MAX
    }

    fn step_trace(&mut self, i: usize) {
        let (w, h) = (self.w, self.h);
        let mut tr = std::mem::replace(
            &mut self.traces[i],
            Trace {
                path: Vec::new(),
                vias: Vec::new(),
                dir: (1, 0),
                color: (0, 0, 0),
                max_len: 0,
                done: true,
                fade: 0.0,
                pulses_left: 0,
                pulse_timer: 0.0,
            },
        );

        if !tr.done {
            let (hx, hy) = *tr.path.last().unwrap();
            let blocked = !self.free(hx + tr.dir.0, hy + tr.dir.1);
            if blocked || self.rng.random::<f32>() < 0.11 {
                // orthogonal turns only
                let left = self.rng.random::<bool>();
                let d1 = if left { (tr.dir.1, -tr.dir.0) } else { (-tr.dir.1, tr.dir.0) };
                let d2 = (-d1.0, -d1.1);
                if self.free(hx + d1.0, hy + d1.1) {
                    tr.dir = d1;
                    tr.vias.push((hx, hy));
                } else if self.free(hx + d2.0, hy + d2.1) {
                    tr.dir = d2;
                    tr.vias.push((hx, hy));
                } else if blocked {
                    tr.done = true;
                }
            }
            if !tr.done {
                let (nx, ny) = (hx + tr.dir.0, hy + tr.dir.1);
                tr.path.push((nx, ny));
                self.owner[ny as usize * w + nx as usize] = i;
                if tr.path.len() >= tr.max_len {
                    tr.done = true;
                }
            }
        } else if !tr.path.is_empty() {
            // schedule pulses along the finished trace
            tr.pulse_timer -= 0.025;
            if tr.pulses_left > 0 && tr.pulse_timer <= 0.0 {
                self.pulses.push(Pulse {
                    trace: i,
                    pos: 0.0,
                    speed: self.rng.random_range(45.0..75.0),
                });
                tr.pulses_left -= 1;
                tr.pulse_timer = self.rng.random_range(0.15..0.4);
            } else if tr.pulses_left == 0 {
                tr.fade -= 0.02;
                if tr.fade <= 0.0 {
                    for &(cx, cy) in &tr.path {
                        self.owner[cy as usize * w + cx as usize] = usize::MAX;
                    }
                    tr = Self::spawn_trace(&mut self.rng, w, h);
                }
            }
        }
        self.traces[i] = tr;
    }
}

impl Scene for Circuits {
    fn name(&self) -> &'static str {
        "circuits"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.owner = vec![usize::MAX; w * h];
            // chip packages that traces must route around (capped so small
            // boards still have room to route)
            self.chips = (0..(w / 50).clamp(1, 6))
                .map(|_| {
                    let cw = self.rng.random_range(8..16).min((w as i32 / 3).max(5));
                    let ch = self.rng.random_range(5..9).min((h as i32 / 2).max(3));
                    let cx = self.rng.random_range(2..(w as i32 - cw - 2).max(3));
                    let cy = self.rng.random_range(2..(h as i32 - ch - 2).max(3));
                    for dy in -1..=ch {
                        for dx in -1..=cw {
                            let (ox, oy) = (cx + dx, cy + dy);
                            if ox >= 0 && oy >= 0 && (ox as usize) < w && (oy as usize) < h {
                                self.owner[oy as usize * w + ox as usize] = usize::MAX - 1;
                            }
                        }
                    }
                    Chip {
                        x: cx,
                        y: cy,
                        w: cw,
                        h: ch,
                        leds: (0..self.rng.random_range(1..=2))
                            .map(|_| {
                                (
                                    self.rng.random_range(0..cw),
                                    self.rng.random_range(0..ch),
                                    self.rng.random_range(0.0..6.0),
                                )
                            })
                            .collect(),
                        flash: 0.0,
                    }
                })
                .collect();
            self.traces = (0..self.detail.scale((w / 7) as f32 * density_for(w, h), 2).clamp(2, 36))
                .map(|_| Self::spawn_trace(&mut self.rng, w, h))
                .collect();
            self.pulses.clear();
            self.surge_trace = -1;
            self.surge_charge = 0.0;
            self.step_acc = 0.0;
            // precompute the pcb surface: near-black mottled wash + via grid
            self.bg_buf = if w == 0 || h == 0 {
                Vec::new()
            } else {
                (0..w * h)
                    .map(|i| {
                        let (x, y) = (i % w, i / w);
                        if x % 4 == 0 && y % 4 == 0 {
                            self.grid_c
                        } else {
                            let m = ((x as f32 * 0.23).sin() * (y as f32 * 0.31).cos()).abs();
                            scale(self.grid_c, 0.16 + m * 0.12)
                        }
                    })
                    .collect()
            };
        }
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);

        self.t += dt;
        self.step_acc += dt * 40.0;
        while self.step_acc >= 1.0 {
            self.step_acc -= 1.0;
            for i in 0..self.traces.len() {
                self.step_trace(i);
            }
        }

        // pcb surface: blit the precomputed dark wash + via grid
        for y in 0..h {
            for x in 0..w {
                canvas.set(x as i32, y as i32, self.bg_buf[y * w + x]);
            }
        }

        // traces: eased fade; the body sits back so pulses/surges pop
        for tr in &self.traces {
            let f = ease_smooth(tr.fade.max(0.0));
            for &(cx, cy) in &tr.path {
                canvas.set(cx, cy, scale(tr.color, f * 0.85));
            }
            // vias brighter
            for &(vx, vy) in &tr.vias {
                canvas.set(vx, vy, scale(lerp(tr.color, (255, 220, 140), 0.5), f));
            }
            // pad at the trace origin
            if let Some(&(px, py)) = tr.path.first() {
                canvas.set(px, py, scale((220, 175, 90), f));
            }
            // bright tip with a soft glow while the trace is still growing
            if !tr.done {
                if let Some(&(hx, hy)) = tr.path.last() {
                    canvas.set(hx, hy, lerp(tr.color, (255, 255, 255), 0.55));
                    glow(canvas, hx, hy, 1, tr.color, 0.45);
                }
            }
        }

        // chip packages: dark body, pin stubs, blinking status LEDs; a
        // passing surge rims the package and whites out its LEDs
        for c in &mut self.chips {
            c.flash = (c.flash - dt * 1.4).max(0.0);
            let flash = ease_smooth(c.flash);
            for dy in 0..c.h {
                for dx in 0..c.w {
                    canvas.set(c.x + dx, c.y + dy, (10, 12, 14));
                }
            }
            // pin stubs along both long edges
            for dx in (1..c.w).step_by(2) {
                canvas.set(c.x + dx, c.y - 1, (120, 122, 130));
                canvas.set(c.x + dx, c.y + c.h, (120, 122, 130));
            }
            if flash > 0.02 {
                // rim light bleeding off the package as the wave passes
                let rim = scale((110, 220, 180), flash * 0.4);
                for dx in -1..=c.w {
                    canvas.add(c.x + dx, c.y - 1, rim);
                    canvas.add(c.x + dx, c.y + c.h, rim);
                }
                for dy in 0..c.h {
                    canvas.add(c.x - 1, c.y + dy, rim);
                    canvas.add(c.x + c.w, c.y + dy, rim);
                }
            }
            for &(lx, ly, ph) in &c.leds {
                // smooth pulse: red breathes in and out, green fills the gaps
                let k = 0.5 + 0.5 * (self.t * 1.5 + ph).sin();
                let col = lerp(lerp((90, 220, 90), (255, 70, 60), k), (255, 255, 255), flash * 0.6);
                canvas.set(c.x + lx, c.y + ly, col);
                if k > 0.7 {
                    canvas.add(c.x + lx + 1, c.y + ly, (60, 15, 12));
                }
                if flash > 0.1 {
                    glow(canvas, c.x + lx, c.y + ly, 1, col, flash * 0.5);
                }
            }
        }

        // tiny sparks where traces cross close to each other
        if self.rng.random::<f32>() < dt * 6.0 && !self.traces.is_empty() {
            let ti = self.rng.random_range(0..self.traces.len());
            if let Some(&(cx, cy)) = self.traces[ti].path.get(self.traces[ti].path.len() / 2) {
                crate::canvas::glow(canvas, cx, cy, 1, (200, 230, 255), 0.5);
            }
        }

        // power surge: charge wells up at the origin pad (anticipation),
        // then a bright wave sweeps the trace leaving a decaying charged
        // trail and flashing any chip it passes
        self.next_surge -= dt;
        if self.next_surge <= 0.0 {
            if let Some(i) = self
                .traces
                .iter()
                .position(|t| t.done && t.path.len() > 8)
            {
                self.surge_trace = i as i32;
                self.surge_charge = 0.7;
                self.surge_pos = -1.0;
            }
            self.next_surge = self.rng.random_range(12.0..24.0);
        }
        if self.surge_trace >= 0 {
            let i = self.surge_trace as usize;
            if i < self.traces.len() && !self.traces[i].path.is_empty() {
                if self.surge_charge > 0.0 {
                    self.surge_charge -= dt;
                    let k = ease_smooth(1.0 - (self.surge_charge / 0.7).clamp(0.0, 1.0));
                    let (px, py) = self.traces[i].path[0];
                    glow(canvas, px, py, 2, (160, 255, 210), 0.25 + 0.55 * k);
                    if self.surge_charge <= 0.0 {
                        self.surge_pos = 0.0;
                    }
                } else {
                    self.surge_pos += 70.0 * dt;
                    let path_len = self.traces[i].path.len() as f32;
                    if self.surge_pos >= path_len + 10.0 {
                        self.surge_trace = -1;
                    } else {
                        let pi = self.surge_pos.clamp(0.0, path_len - 1.0) as usize;
                        let (sx, sy) = self.traces[i].path[pi];
                        glow(canvas, sx, sy, 3, (180, 255, 220), 0.8);
                        canvas.set(sx, sy, (245, 255, 248));
                        // charged trail decaying behind the wave head
                        for back in 1..=8usize {
                            if pi >= back {
                                let (bx, by) = self.traces[i].path[pi - back];
                                let fall = 1.0 - back as f32 / 9.0;
                                canvas.add(bx, by, scale((120, 255, 190), fall * 0.3));
                            }
                        }
                        // light up chips the wave passes
                        for c in &mut self.chips {
                            let nx = sx.clamp(c.x, c.x + c.w - 1);
                            let ny = sy.clamp(c.y, c.y + c.h - 1);
                            if (sx - nx).abs() + (sy - ny).abs() < 5 {
                                c.flash = 1.0;
                            }
                        }
                    }
                }
            } else {
                self.surge_trace = -1;
            }
        }

        // pulses: bright dots with a soft tinted glow, moving along paths
        self.pulses.retain_mut(|p| {
            p.pos += p.speed * dt;
            let tr = &self.traces[p.trace];
            if tr.path.is_empty() || p.pos >= tr.path.len() as f32 {
                return false;
            }
            let (x, y) = tr.path[p.pos as usize];
            canvas.set(x, y, (235, 255, 240));
            glow(canvas, x, y, 1, lerp(tr.color, (255, 255, 255), 0.4), 0.55);
            true
        });
    }
}
