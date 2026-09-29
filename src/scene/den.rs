//! Den: a cozy Japanese room at night. A chunky CRT on a stand plays REAL
//! other scenes on its screen; through the window, the countryside —
//! terraced paddies, mountains, a cherry tree, drifting clouds, birds.
//! A cat sleeps on a cushion, breathing.

use super::{Detail, Scene, SceneOptions};
use crate::canvas::{density_for, ease_smooth, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt, SeedableRng};

/// Scenes the TV cycles through.
const CHANNELS: &[&str] = &["fire", "starfield", "nexus"];
const CHANNEL_SECS: f32 = 20.0;

pub struct Den {
    rng: StdRng,
    detail: Detail,
    inner: Vec<Box<dyn Scene>>,
    inner_canvas: Canvas,
    channel: usize,
    channel_t: f32,
    static_t: f32,
    clouds: Vec<(f32, f32, f32)>, // x, y, speed
    bird: Option<(f32, f32, f32)>,
    next_bird: f32,
    meteor: Option<(f32, f32, f32, f32, f32)>, // x, y, vx, vy, life left
    next_meteor: f32,
    fireflies: Vec<(f32, f32, f32)>, // base x, base y, phase
    evening: bool,
    rain_outside: bool,
    t: f32,
    w: usize,
    h: usize,
}

impl Den {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail, pixels: crate::render::Pixels) -> Self {
        let (evening, rain_outside) = match theme {
            Some("evening") => (true, false),
            Some("rain-outside") => (false, true),
            _ => (false, false), // night
        };
        let mut rng = rng;
        let inner = CHANNELS
            .iter()
            .map(|name| {
                let seed: u64 = rng.random();
                super::create(
                    name,
                    &SceneOptions {
                        theme: None,
                        detail: Detail::Medium,
                        text_scale: None,
                        pixels,
                    },
                    StdRng::seed_from_u64(seed),
                )
                .expect("channel scene")
            })
            .collect();
        Den {
            rng,
            detail,
            inner,
            inner_canvas: Canvas::new(44, 26),
            channel: 0,
            channel_t: CHANNEL_SECS,
            static_t: 0.0,
            clouds: Vec::new(),
            bird: None,
            next_bird: 8.0,
            meteor: None,
            next_meteor: 7.0,
            fireflies: Vec::new(),
            evening,
            rain_outside,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// TV screen rectangle in canvas pixels (test-visible).
    pub fn tv_rect(&self) -> (i32, i32, i32, i32) {
        let sw = self.inner_canvas.width() as i32;
        let sh = self.inner_canvas.height() as i32;
        let x0 = ((self.w as i32 - sw) / 2).min(self.w as i32 / 4).max(4);
        let y0 = (self.h as i32 * 32 / 100).max(2);
        (x0, y0, sw, sh)
    }

    /// Window rectangle (right side).
    fn win_rect(&self) -> (i32, i32, i32, i32) {
        let (w, h) = (self.w as i32, self.h as i32);
        (w * 3 / 5, h / 8, w - w * 3 / 5 - 4, h * 5 / 8)
    }

    fn init(&mut self) {
        // size the CRT's inner screen to fit the canvas (44x26 at 120x50)
        let sw = (self.w * 44 / 120).clamp(10, 44);
        let sh = (self.h * 26 / 50).clamp(6, 26);
        self.inner_canvas.resize(sw, sh);
        let (_, _, ww, wh) = self.win_rect();
        let n = self
            .detail
            .scale(3.0 * density_for(self.w, self.h), 2)
            .clamp(2, 8);
        self.clouds = (0..n)
            .map(|i| {
                (
                    self.rng.random_range(0.0..ww as f32),
                    (i * 3) as f32 % (wh as f32 * 0.5).max(3.0) + 2.0,
                    self.rng.random_range(1.0..2.5),
                )
            })
            .collect();
        // fireflies drift outside the window at dusk and at night
        self.fireflies = if self.rain_outside {
            Vec::new()
        } else {
            let nf = self.detail.scale(2.0, 1).min(4);
            (0..nf)
                .map(|_| {
                    (
                        self.rng.random_range(0.1..0.9) * ww as f32,
                        self.rng.random_range(0.5..0.85) * wh as f32,
                        self.rng.random_range(0.0..std::f32::consts::TAU),
                    )
                })
                .collect()
        };
    }

    fn draw_room(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        let wall = if self.evening { (46, 30, 24) } else { (30, 22, 18) };
        let floor = if self.evening { (64, 46, 30) } else { (48, 34, 24) };
        let floor_y = h * 3 / 4;
        for y in 0..h {
            canvas.fill_row(y, if y >= floor_y { floor } else { wall });
        }
        // tatami seams
        for y in (floor_y..h).step_by(3) {
            canvas.fill_row(y, scale(floor, 0.8));
        }
        // warm lamp glow, upper left — breathes with a slow mains flicker
        let (lx, ly) = (w as f32 * 0.08, h as f32 * 0.10);
        let glow_c = if self.evening { (255, 190, 110) } else { (255, 170, 90) };
        let flick = 0.90 + 0.10 * (self.t * 2.3).sin() * (self.t * 0.7 + 1.3).cos();
        let gr = h as f32 * 0.45;
        for dy in -(gr as i32)..=(gr as i32) {
            for dx in -(gr as i32)..=(gr as i32) {
                let d = ((dx * dx + dy * dy) as f32).sqrt() / gr;
                if d < 1.0 {
                    canvas.add(
                        lx as i32 + dx,
                        ly as i32 + dy,
                        scale(glow_c, (1.0 - d).powi(2) * 0.30 * flick),
                    );
                }
            }
        }
        // the lamp itself
        canvas.set_f(lx, ly, scale(glow_c, flick));
        canvas.set_f(lx, ly + 1.0, scale(glow_c, 0.5 * flick));
    }

    fn draw_window(&mut self, dt: f32, canvas: &mut Canvas) {
        let (wx, wy, ww, wh) = self.win_rect();
        if ww < 6 || wh < 6 {
            return;
        }
        let t = self.t;

        // slow day-night tint breathing
        let day_k = 0.5 + 0.5 * (t * 0.02).sin();
        let (sky_a, sky_b) = if self.evening {
            (
                lerp((40, 20, 40), (70, 40, 50), day_k),
                lerp((90, 45, 40), (140, 80, 55), day_k),
            )
        } else {
            (
                lerp((6, 8, 20), (14, 18, 38), day_k),
                lerp((20, 26, 50), (40, 50, 80), day_k),
            )
        };
        for y in 0..wh {
            canvas.fill_span(wx, wy + y, ww, lerp(sky_a, sky_b, y as f32 / wh as f32));
        }

        // moon with a soft halo (clear nights only), dimmed behind the peaks
        if !self.evening && !self.rain_outside {
            let (mx, my) = (wx + ww * 3 / 4, wy + (wh / 6).max(1));
            let mr = (wh / 9).clamp(1, 3);
            let halo = 0.9 + 0.1 * (t * 0.5).sin();
            for dy in -(mr * 2)..=(mr * 2) {
                for dx in -(mr * 2)..=(mr * 2) {
                    let d2 = dx * dx + dy * dy;
                    if d2 <= mr * mr {
                        let shade = 1.0 - (d2 as f32 / (mr * mr) as f32) * 0.25;
                        canvas.set(mx + dx, my + dy, scale((218, 224, 234), shade * halo));
                    } else if d2 <= mr * mr * 4 {
                        let fall = 1.0 - (d2 as f32).sqrt() / (mr * 2) as f32;
                        canvas.add(mx + dx, my + dy, scale((140, 155, 185), fall * fall * 0.18 * halo));
                    }
                }
            }
        }

        // layered mountains
        for x in 0..ww {
            let xf = x as f32;
            let r1 = wh as f32 * 0.38 + (xf * 0.10).sin() * wh as f32 * 0.08;
            let r2 = wh as f32 * 0.50 + (xf * 0.06 + 2.0).cos() * wh as f32 * 0.10;
            for y in (r1 as i32).max(0)..wh {
                let c = if (y as f32) < r2 {
                    lerp((40, 50, 70), (20, 26, 40), 1.0 - day_k * 0.5)
                } else {
                    lerp((25, 40, 30), (12, 20, 16), 1.0 - day_k * 0.5)
                };
                canvas.set(wx + x, wy + y, c);
            }
        }

        // Fuji-style snowcap with alpenglow at dusk behind the paddies
        let fuji_x = wx + ww * 2 / 3;
        let fuji_base = wy + wh * 3 / 5;
        let fuji_h = wh / 4;
        for dy in 0..fuji_h {
            let half = (dy as f32 * 1.1) as i32;
            for dx in -half..=half {
                let snow = dy > fuji_h * 2 / 3;
                let c = if snow {
                    lerp((235, 240, 250), (255, 190, 160), day_k * 0.6) // alpenglow
                } else {
                    lerp((35, 45, 60), (50, 40, 45), day_k * 0.5)
                };
                canvas.set(fuji_x + dx, fuji_base - dy, c);
            }
        }

        // terraced rice paddies: horizontal bands with water glints
        for t_i in 0..4 {
            let ty = wy + wh * 3 / 5 + t_i * (wh / 10);
            let inset = t_i * 2;
            for x in inset..ww - inset {
                let band = lerp((40, 90, 45), (30, 70, 38), t_i as f32 / 4.0);
                canvas.set(wx + x, ty, band);
                // water glint line on each terrace edge
                if (x * 7 + t_i * 13) % 9 < 2 {
                    canvas.set(wx + x, ty - 1, scale((160, 200, 220), 0.3 + day_k * 0.3));
                }
            }
        }

        // cherry tree on the left of the window
        let (cx, cy) = (wx + ww / 6, wy + wh * 3 / 5);
        for dy in 0..(wh / 6).max(2) {
            canvas.set(cx, cy - dy, (50, 32, 22));
        }
        let cr = (wh / 8).max(2);
        for dy in -cr..=cr {
            for dx in -cr..=cr {
                let d2 = dx * dx + dy * dy;
                if d2 <= cr * cr && (dx * 31 + dy * 17 + 100) % 7 > 1 {
                    let pink = lerp((230, 150, 170), (250, 190, 205), d2 as f32 / (cr * cr) as f32);
                    canvas.set(cx + dx, cy - wh / 6 - cr / 2 + dy, pink);
                }
            }
        }

        // drifting clouds, gently bobbing on the wind
        for (cx2, cy2, sp) in &mut self.clouds {
            *cx2 += *sp * dt;
            if *cx2 > ww as f32 + 8.0 {
                *cx2 = -8.0;
            }
            let bob = (t * 0.4 + *cy2 * 1.7).sin() * 0.8;
            let cc = lerp((60, 66, 90), (120, 126, 150), day_k);
            for dy in 0..2 {
                for dx in -3..=3 {
                    canvas.set(wx + *cx2 as i32 + dx, wy + (*cy2 + bob) as i32 + dy, cc);
                }
            }
        }

        // occasional bird
        self.next_bird -= dt;
        if self.next_bird <= 0.0 && self.bird.is_none() {
            self.bird = Some((-4.0, wy as f32 + wh as f32 * 0.3, 14.0));
            self.next_bird = self.rng.random_range(10.0..20.0);
        }
        if let Some((bx, by, bvx)) = &mut self.bird {
            *bx += *bvx * dt;
            if *bx > ww as f32 + 4.0 {
                self.bird = None;
            } else {
                let flap = ((t * 8.0).sin() * 1.0) as i32;
                let bob = ((t * 2.2).sin() * 1.2) as i32;
                let dark = (10, 10, 16);
                canvas.set(wx + *bx as i32, *by as i32 + bob, dark);
                canvas.set(wx + *bx as i32 - 1, *by as i32 + bob - flap.abs(), dark);
                canvas.set(wx + *bx as i32 + 1, *by as i32 + bob - flap.abs(), dark);
            }
        }

        // shooting star: a rare streak across the night sky (payoff + decay)
        if !self.evening && !self.rain_outside {
            self.next_meteor -= dt;
            if self.next_meteor <= 0.0 && self.meteor.is_none() {
                let from_left = self.rng.random::<bool>();
                self.meteor = Some((
                    if from_left {
                        self.rng.random_range(0.0..0.4) * ww as f32
                    } else {
                        self.rng.random_range(0.6..1.0) * ww as f32
                    },
                    self.rng.random_range(0.05..0.3) * wh as f32,
                    if from_left { 26.0 } else { -26.0 },
                    9.0,
                    0.55,
                ));
                self.next_meteor = self.rng.random_range(14.0..26.0);
            }
            if let Some((mx, my, mvx, mvy, life)) = &mut self.meteor {
                *mx += *mvx * dt;
                *my += *mvy * dt;
                *life -= dt;
                if *life <= 0.0 || *mx < -2.0 || *mx > ww as f32 + 2.0 {
                    self.meteor = None;
                } else {
                    // ease-out fade: bright head, decaying trail behind
                    let a = ease_smooth((*life / 0.55).clamp(0.0, 1.0));
                    for i in 0..6 {
                        let fi = i as f32;
                        let px = *mx - mvx.signum() * fi;
                        let py = *my - fi * 0.35;
                        let fade = a * (1.0 - fi / 6.0);
                        if i == 0 {
                            canvas.set(wx + px as i32, wy + py as i32, scale((240, 244, 252), fade));
                        } else {
                            canvas.add(
                                wx + px as i32,
                                wy + py as i32,
                                scale((170, 185, 215), fade * 0.5),
                            );
                        }
                    }
                }
            }
        }

        // fireflies drifting over the paddies, blinking in slow envelopes
        for (fx, fy, phase) in &self.fireflies {
            let px = wx as f32 + fx + (t * 0.5 + phase).sin() * 3.0;
            let py = wy as f32 + fy + (t * 0.35 + phase * 2.0).cos() * 1.5;
            let blink = (0.5 + 0.5 * (t * 1.3 + phase * 3.0).sin()).powi(2);
            if blink > 0.15 {
                canvas.set_f(px, py, scale((190, 225, 120), blink));
                canvas.add(px as i32, py as i32 - 1, scale((150, 190, 100), blink * 0.3));
            }
        }

        // rain streaks outside (rain-outside theme)
        if self.rain_outside {
            for i in 0..self.detail.scale(ww as f32 * 2.0, 8) {
                let rx = ((i * 37) as f32 + t * 40.0) % ww as f32;
                let ry = ((i * 53) as f32 + t * 90.0) % wh as f32;
                canvas.set(wx + rx as i32, wy + ry as i32, (70, 85, 110));
            }
        }

        // window frame
        let frame = (52, 36, 26);
        for x in wx - 1..=wx + ww {
            canvas.set(x, wy - 1, frame);
            canvas.set(x, wy + wh, frame);
        }
        for y in wy - 1..=wy + wh {
            canvas.set(wx - 1, y, frame);
            canvas.set(wx + ww, y, frame);
            canvas.set(wx + ww / 2, y, frame); // center mullion
        }
    }

    fn draw_tv(&mut self, dt: f32, canvas: &mut Canvas) {
        let (tx, ty, sw, sh) = self.tv_rect();

        // channel changing with a static burst
        self.channel_t -= dt;
        if self.channel_t <= 0.0 {
            self.channel_t = CHANNEL_SECS;
            self.static_t = 0.5;
            self.channel = (self.channel + 1) % self.inner.len();
        }
        if self.static_t > 0.0 {
            self.static_t -= dt;
            for y in 0..sh {
                for x in 0..sw {
                    let v = self.rng.random_range(0..120) as u8;
                    canvas.set(tx + x, ty + y, (v, v, v));
                }
            }
        } else {
            // real inner scene on the CRT
            self.inner[self.channel].update(dt, &mut self.inner_canvas);
            for y in 0..sh {
                for x in 0..sw {
                    let mut c = self.inner_canvas.get(x, y).color;
                    // scanlines + slight green CRT tint
                    if y % 2 == 1 {
                        c = scale(c, 0.7);
                    }
                    c.1 = c.1.saturating_add(6);
                    canvas.set(tx + x, ty + y, c);
                }
            }
        }
        // glass glare: a soft diagonal sheen
        for y in 0..sh {
            for x in 0..sw {
                let g = ((x + y) as f32 / (sw + sh) as f32 - 0.30).abs();
                if g < 0.05 {
                    canvas.add(tx + x, ty + y, (18, 22, 26));
                }
            }
        }

        // chunky bezel + stand + power LED
        let bezel = (24, 20, 18);
        for x in tx - 3..tx + sw + 3 {
            for dy in -3..=1 {
                canvas.set(x, ty + dy, bezel);
                canvas.set(x, ty + sh + dy, bezel);
            }
        }
        for y in ty - 3..ty + sh + 2 {
            for dx in -3..0 {
                canvas.set(tx + dx, y, bezel);
                canvas.set(tx + sw - dx - 1, y, bezel);
            }
        }
        canvas.set(tx + sw + 1, ty + sh - 2, (255, 60, 40)); // power LED
        // remote blink on channel change
        if self.static_t > 0.0 {
            let blink = ((self.static_t * 20.0) as i32) % 2 == 0;
            if blink {
                canvas.set(tx - 2, ty - 2, (255, 80, 60));
            }
        }
        // stand
        let stand = (30, 24, 20);
        let cxm = tx + sw / 2;
        for dy in 1..=3 {
            for dx in -6..=6 {
                canvas.set(cxm + dx, ty + sh + 1 + dy, stand);
            }
        }

        // TV light spill: the floor in front of the set flickers with it
        let t = self.t;
        let h2 = canvas.height();
        let spill_y0 = ty + sh + 4;
        let spill_y1 = (h2 * 7 / 8) as i32;
        let tv_lum = 0.10 + 0.08 * (t * 7.3).sin() * (t * 2.1).cos();
        for y in spill_y0..spill_y1 {
            let spread = sw as f32 * (0.5 + (y - spill_y0) as f32 * 0.06);
            let cxm2 = tx + sw / 2;
            for dx in -(spread as i32)..=(spread as i32) {
                let d = (dx as f32 / spread).abs();
                canvas.add(
                    cxm2 + dx,
                    y,
                    scale((140, 150, 190), (1.0 - d) * tv_lum),
                );
            }
        }

        // low table silhouette in front
        let (w, h) = (canvas.width(), canvas.height());
        let ty2 = (h * 7 / 8) as i32;
        let table = (26, 17, 12);
        let tw = w as i32 / 3;
        let txm = w as i32 / 2;
        for dx in -tw / 2..=tw / 2 {
            canvas.set(txm + dx, ty2, table);
            canvas.set(txm + dx, ty2 + 1, scale(table, 1.3));
        }
        canvas.set(txm - tw / 2 + 2, ty2 + 2, table);
        canvas.set(txm + tw / 2 - 2, ty2 + 2, table);
    }

    fn draw_cat(&self, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        let (cx, cy) = (w as f32 * 0.82, h as f32 * 0.90);
        // cushion
        let cush = if self.evening { (90, 40, 40) } else { (70, 32, 34) };
        let crx = w as f32 * 0.07;
        let cry = h as f32 * 0.03;
        for dy in -(cry as i32)..=(cry as i32) {
            for dx in -(crx as i32)..=(crx as i32) {
                let d = (dx as f32 / crx).powi(2) + (dy as f32 / cry).powi(2);
                if d <= 1.0 {
                    canvas.set(cx as i32 + dx, cy as i32 + dy, scale(cush, 1.0 - d * 0.3));
                }
            }
        }
        // sleeping cat: a breathing dark loaf with ear bumps and a tail
        let breath = 1.0 + 0.08 * (self.t * 0.9).sin();
        let brx = crx * 0.7;
        let bry = cry * 0.9 * breath;
        let fur = (16, 13, 12);
        for dy in -(bry as i32)..=0 {
            for dx in -(brx as i32)..=(brx as i32) {
                let d = (dx as f32 / brx).powi(2) + (dy as f32 / bry).powi(2);
                if d <= 1.0 {
                    canvas.set(cx as i32 + dx, cy as i32 - 1 + dy, fur);
                }
            }
        }
        // ears
        canvas.set_f(cx - brx * 0.5, cy - bry - 1.0, fur);
        canvas.set_f(cx - brx * 0.2, cy - bry - 1.0, fur);
        // tail curling around
        let tx0 = cx + brx * 0.8;
        for i in 0..5 {
            canvas.set_f(tx0 + i as f32 * 0.8, cy - 1.0 - (i as f32 * 0.4).sin(), fur);
        }
    }
}

impl Scene for Den {
    fn name(&self) -> &'static str {
        "den"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.init();
        }
        if w < 30 || h < 20 {
            canvas.clear((0, 0, 0));
            return;
        }
        self.t += dt;

        self.draw_room(canvas);
        self.draw_window(dt, canvas);
        self.draw_tv(dt, canvas);
        self.draw_cat(canvas);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn tv_plays_real_scenes() {
        let mut d = Den::new(StdRng::seed_from_u64(7), None, Detail::Medium, Default::default());
        let mut c = Canvas::new(120, 50);
        for _ in 0..90 {
            d.update(1.0 / 30.0, &mut c);
        }
        let (tx, ty, sw, sh) = d.tv_rect();
        // TV region: non-uniform, and different from the wall behind
        let mut distinct = std::collections::HashSet::new();
        let mut lit = 0;
        for y in ty..ty + sh {
            for x in tx..tx + sw {
                let col = c.get(x, y).color;
                distinct.insert(col);
                if col != (30, 22, 18) {
                    lit += 1;
                }
            }
        }
        assert!(lit > (sw * sh) / 2, "TV screen should be lit, {lit}");
        assert!(distinct.len() > 8, "inner scene should be colorful");
    }

    #[test]
    fn channel_changes_after_interval() {
        let mut d = Den::new(StdRng::seed_from_u64(8), None, Detail::Medium, Default::default());
        let mut c = Canvas::new(120, 50);
        d.update(1.0 / 30.0, &mut c);
        assert_eq!(d.channel, 0);
        d.channel_t = 0.01;
        d.update(1.0 / 30.0, &mut c);
        assert_eq!(d.channel, 1, "channel should advance");
        assert!(d.static_t > 0.0, "static burst on channel change");
    }
}
