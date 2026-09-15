//! The bouncing DVD logo meme: crisp block-letter pixel art, perfect
//! wall-bounce physics, new bright color on every bounce, near-black bg.
//! Polish: dim dust-mote background plane, a colored comet-trail glow
//! behind the logo, squash-and-stretch on the impact axis, sparkles with
//! drag on every bounce, a tinted edge flash + wall scuff on corner hits,
//! and a 1x letter scale so the scene still reads on tiny terminals.

use super::{Detail, Scene};
use crate::canvas::{dot, ease_smooth, glow_f, lerp, rect_f, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

// 5x7 block letters, scaled 2x at draw time → 10x14 per letter,
// 2-cell gaps → logo is 34x14 canvas pixels.
const D: [&str; 7] = [
    "####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####.",
];
const V: [&str; 7] = [
    "#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#..",
];

pub const LOGO_W: i32 = 34;
pub const LOGO_H: i32 = 14;

fn colors_for(theme: Option<&str>) -> &'static [(u8, u8, u8)] {
    match theme {
        Some("pastel") => &[
            (255, 160, 160),
            (160, 255, 190),
            (150, 190, 255),
            (255, 240, 160),
            (220, 160, 255),
            (160, 240, 240),
        ],
        Some("hot") => &[
            (255, 60, 40),
            (255, 140, 30),
            (255, 200, 40),
            (255, 90, 120),
            (230, 40, 60),
        ],
        _ => COLORS,
    }
}

const COLORS: &[(u8, u8, u8)] = &[
    (255, 60, 60),   // red
    (60, 255, 120),  // green
    (80, 140, 255),  // blue
    (255, 230, 60),  // yellow
    (200, 90, 255),  // purple
    (60, 230, 230),  // cyan
    (255, 150, 40),  // orange
    (255, 90, 180),  // pink
];

pub struct Dvd {
    detail: Detail,
    rng: StdRng,
    colors: &'static [(u8, u8, u8)],
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: usize,
    sparkles: Vec<(f32, f32, f32, f32, f32, (u8, u8, u8))>, // x, y, vx, vy, life, color
    /// fading colored glows left on the wall at each impact
    impacts: Vec<(f32, f32, f32, (u8, u8, u8))>, // x, y, life, color
    /// recent logo-center positions + the color of that era (comet tail)
    trail: Vec<(f32, f32, usize)>,
    trail_t: f32,
    /// dim drifting background motes: x, y, magnitude, phase
    dust: Vec<(f32, f32, f32, f32)>,
    edge_flash: f32,
    color_blend: f32,
    prev_color: usize,
    squash: f32,
    squash_x: bool, // which axis the last impact compressed
    hit_x: bool,
    hit_y: bool,
    t: f32,
    w: usize,
    h: usize,
}

impl Dvd {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let colors = colors_for(theme);
        let color = rng.random_range(0..colors.len());
        Dvd {
            colors,
            x: 10.0,
            y: 6.0,
            vx: 42.0,
            vy: 26.0,
            color,
            sparkles: Vec::new(),
            impacts: Vec::new(),
            trail: Vec::new(),
            trail_t: 0.0,
            detail,
            dust: Vec::new(),
            edge_flash: 0.0,
            color_blend: 1.0,
            prev_color: color,
            squash: 0.0,
            squash_x: false,
            hit_x: false,
            hit_y: false,
            t: 0.0,
            w: 0,
            h: 0,
            rng,
        }
    }

    fn new_color(&mut self) {
        let old = self.color;
        while self.color == old {
            self.color = self.rng.random_range(0..self.colors.len());
        }
    }

    /// Effective logo size: 1x letter scale when the 2x logo can't fit.
    fn dims(&self) -> (i32, i32) {
        if self.w >= LOGO_W as usize + 2 && self.h >= LOGO_H as usize + 2 {
            (LOGO_W, LOGO_H)
        } else {
            (LOGO_W / 2, LOGO_H / 2)
        }
    }

    fn max_x(&self) -> f32 {
        (self.w as i32 - self.dims().0).max(0) as f32
    }

    fn max_y(&self) -> f32 {
        (self.h as i32 - self.dims().1).max(0) as f32
    }

    /// Move and bounce; returns number of wall hits this step.
    /// Picks a new color on any bounce.
    pub fn step_physics(&mut self, dt: f32) -> usize {
        let mut hits = 0;
        self.hit_x = false;
        self.hit_y = false;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        if self.x <= 0.0 {
            self.x = 0.0;
            self.vx = self.vx.abs();
            hits += 1;
            self.hit_x = true;
        } else if self.x >= self.max_x() {
            self.x = self.max_x();
            self.vx = -self.vx.abs();
            hits += 1;
            self.hit_x = true;
        }
        if self.y <= 0.0 {
            self.y = 0.0;
            self.vy = self.vy.abs();
            hits += 1;
            self.hit_y = true;
        } else if self.y >= self.max_y() {
            self.y = self.max_y();
            self.vy = -self.vy.abs();
            hits += 1;
            self.hit_y = true;
        }
        if hits > 0 {
            self.prev_color = self.color;
            self.new_color();
            self.color_blend = 0.0;
            self.squash = 1.0;
            self.squash_x = self.hit_x;
        }
        hits
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_letter(
        canvas: &mut Canvas,
        glyph: &[&str; 7],
        ox: f32,
        oy: f32,
        kx: f32,
        ky: f32,
        cell: i32,
        c: (u8, u8, u8),
    ) {
        // squash/stretch is anchored at each letter's center so the logo
        // deforms in place instead of sliding. Fractional origin + rect_f
        // edges make the logo glide between cells instead of popping.
        let (cx, cy) = (2.5 * cell as f32, 3.5 * cell as f32);
        let cf = cell as f32;
        for (gy, row) in glyph.iter().enumerate() {
            for (gx, ch) in row.chars().enumerate() {
                if ch == '#' {
                    // top-lit gradient keeps the flat blocks from reading dead
                    let shade = 1.05 - (gy as f32 / 6.0) * 0.3;
                    let gc = scale(c, shade);
                    let lx = gx as f32 * cf;
                    let ly = gy as f32 * cf;
                    rect_f(
                        canvas,
                        ox + (lx - cx) * kx + cx,
                        oy + (ly - cy) * ky + cy,
                        cf * kx,
                        cf * ky,
                        gc,
                    );
                }
            }
        }
    }
}

impl Scene for Dvd {
    fn name(&self) -> &'static str {
        "dvd"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        let (w, h) = (canvas.width(), canvas.height());
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            // respawn at a random valid spot on resize
            self.x = self.rng.random_range(0.0..self.max_x().max(1.0));
            self.y = self.rng.random_range(0.0..self.max_y().max(1.0));
            self.trail.clear();
            // the dust plane is the one density knob this scene has:
            // the logo itself is a fixed-size sprite
            let nd = self
                .detail
                .scale(((w * h) / 350) as f32, 6)
                .clamp(6, 110);
            self.dust = (0..nd)
                .map(|_| {
                    (
                        self.rng.random_range(0.0..w as f32),
                        self.rng.random_range(0.0..h as f32),
                        self.rng.random_range(0.04..0.11),
                        self.rng.random_range(0.0..std::f32::consts::TAU),
                    )
                })
                .collect();
        }
        let (lw, lh) = self.dims();
        if w < (LOGO_W / 2) as usize + 2 || h < (LOGO_H / 2) as usize + 2 {
            canvas.clear((0, 0, 0));
            return; // terminal too small even for the 1x logo
        }

        let hits = self.step_physics(dt);

        canvas.clear((0, 0, 0));

        // background plane: sparse dust motes, drifting and twinkling
        for &(dx, dy, mag, phase) in &self.dust {
            let mx = dx + (self.t * 0.11 + phase).sin() * 1.5;
            let my = dy + (self.t * 0.07 + phase * 2.0).cos() * 0.8;
            let tw = 0.6 + 0.4 * (self.t * 0.8 + phase).sin();
            dot(canvas, mx, my, (110, 140, 180), mag * tw);
        }

        // impact payoff: scuff glow + sparkles in the NEW color; corner
        // hits get the big burst and the screen-edge flash
        if hits > 0 {
            let c_new = self.colors[self.color];
            let ix = if self.hit_x {
                if self.vx > 0.0 {
                    self.x
                } else {
                    self.x + lw as f32
                }
            } else {
                self.x + lw as f32 / 2.0
            };
            let iy = if self.hit_y {
                if self.vy > 0.0 {
                    self.y
                } else {
                    self.y + lh as f32
                }
            } else {
                self.y + lh as f32 / 2.0
            };
            self.impacts.push((ix, iy, 1.0, c_new));
            let n = if hits >= 2 { 14 } else { 5 };
            for i in 0..n {
                let a = self.rng.random_range(0.0..std::f32::consts::TAU);
                let sp = self.rng.random_range(10.0..40.0);
                self.sparkles.push((
                    ix,
                    iy,
                    a.cos() * sp,
                    a.sin() * sp,
                    self.rng.random_range(0.3..0.7),
                    if i % 2 == 0 { (255, 255, 255) } else { c_new },
                ));
            }
            if hits >= 2 {
                self.edge_flash = 0.5;
            }
        }
        self.edge_flash = (self.edge_flash - dt).max(0.0);
        if self.edge_flash > 0.0 {
            let f = self.edge_flash / 0.5;
            let fc = scale(self.colors[self.color], f * 0.35);
            for x in 0..w as i32 {
                canvas.add(x, 0, fc);
                canvas.add(x, h as i32 - 1, fc);
            }
            for y in 0..h as i32 {
                canvas.add(0, y, fc);
                canvas.add(w as i32 - 1, y, fc);
            }
        }
        self.impacts.retain_mut(|(ix, iy, life, c)| {
            *life -= dt * 1.6;
            if *life <= 0.0 {
                return false;
            }
            let r = 2.0 + (1.0 - *life) * 3.0;
            glow_f(canvas, *ix, *iy, r, *c, *life * 0.5);
            true
        });
        self.sparkles.retain_mut(|(sx, sy, svx, svy, life, c)| {
            *life -= dt;
            // drag: sparks shoot out fast, then hang and fade
            let drag = (1.0 - 3.0 * dt).max(0.0);
            *svx *= drag;
            *svy *= drag;
            *sx += *svx * dt;
            *sy += *svy * dt;
            if *life <= 0.0 {
                return false;
            }
            dot(canvas, *sx, *sy, *c, *life * 1.8);
            true
        });

        // comet tail: dim colored glows at recent logo-center positions
        self.trail_t -= dt;
        if self.trail_t <= 0.0 {
            self.trail_t = 0.07;
            self.trail
                .push((self.x + lw as f32 / 2.0, self.y + lh as f32 / 2.0, self.color));
            if self.trail.len() > 12 {
                self.trail.remove(0);
            }
        }
        let tlen = self.trail.len();
        for (i, &(tx, ty, ci)) in self.trail.iter().enumerate() {
            let age = (i + 1) as f32 / tlen as f32; // 1 = newest
            glow_f(canvas, tx, ty, 2.0, self.colors[ci], 0.15 * age * age);
        }

        // color eases to the new one over ~0.25s; logo squashes on impact
        self.color_blend = (self.color_blend + dt * 4.0).min(1.0);
        self.squash = (self.squash - dt * 5.0).max(0.0);
        let c = lerp(
            self.colors[self.prev_color],
            self.colors[self.color],
            ease_smooth(self.color_blend),
        );
        let s = ease_smooth(self.squash);
        let (kx, ky) = if self.squash_x {
            (1.0 - 0.22 * s, 1.0 + 0.12 * s)
        } else {
            (1.0 + 0.12 * s, 1.0 - 0.22 * s)
        };
        let cell = if (lw, lh) == (LOGO_W, LOGO_H) { 2 } else { 1 };
        let adv = (6 * cell) as f32;
        // soft halo so the logo lights the dark around it
        glow_f(
            canvas,
            self.x + lw as f32 / 2.0,
            self.y + lh as f32 / 2.0,
            (cell + 1) as f32,
            c,
            0.08,
        );
        Self::draw_letter(canvas, &D, self.x, self.y, kx, ky, cell, c);
        Self::draw_letter(canvas, &V, self.x + adv, self.y, kx, ky, cell, c);
        Self::draw_letter(canvas, &D, self.x + adv * 2.0, self.y, kx, ky, cell, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn dvd_at(x: f32, y: f32, vx: f32, vy: f32) -> Dvd {
        let mut d = Dvd::new(StdRng::seed_from_u64(8), None, Detail::Medium);
        d.w = 100;
        d.h = 50;
        d.x = x;
        d.y = y;
        d.vx = vx;
        d.vy = vy;
        d
    }

    #[test]
    fn bounces_reverse_velocity_at_walls() {
        // right wall: step until the first hit, then vx must be reversed
        let mut d = dvd_at(65.0, 10.0, 42.0, 26.0);
        let mut hit = false;
        for _ in 0..60 {
            if d.step_physics(1.0 / 30.0) > 0 {
                hit = true;
                break;
            }
        }
        assert!(hit, "should have hit the right wall");
        assert!(d.vx < 0.0, "vx must reverse after right-wall hit");
        assert!(d.x <= d.max_x() && d.x >= 0.0);

        // top wall
        let mut d = dvd_at(10.0, 1.0, 42.0, -26.0);
        d.step_physics(1.0 / 10.0);
        assert!(d.vy > 0.0, "vy must reverse after top-wall hit");
        assert!(d.y >= 0.0);
    }

    #[test]
    fn corner_hit_changes_color_and_stays_in_bounds() {
        let mut d = dvd_at(65.9, 35.9, 60.0, 40.0);
        let before = d.color;
        let mut hits = 0;
        for _ in 0..30 {
            hits += d.step_physics(1.0 / 30.0);
        }
        assert!(hits >= 2, "corner region should produce multiple hits");
        assert_ne!(d.color, before, "bounce must change the color");
        for _ in 0..300 {
            d.step_physics(1.0 / 30.0);
            assert!(d.x >= 0.0 && d.x <= d.max_x());
            assert!(d.y >= 0.0 && d.y <= d.max_y());
        }
    }

    #[test]
    fn logo_dimensions_match_bitmap() {
        // 3 letters * 10px + 2 gaps * 2px = 34 wide, 14 tall
        assert_eq!(LOGO_W, 34);
        assert_eq!(LOGO_H, 14);
        assert!(D.iter().all(|r| r.chars().count() == 5));
        assert!(V.iter().all(|r| r.chars().count() == 5));
    }
}
