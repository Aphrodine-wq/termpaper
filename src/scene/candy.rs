//! Candy: saturated sugar-rush gradients, glossy floating orbs, and
//! sparkle bursts. Three parallax orb layers plus a shimmer wash event.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, hsv, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

struct Orb {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    hue: f32,
    layer: u8,
    phase: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Event {
    None,
    Sparkle(f32),
}

pub struct Candy {
    rng: StdRng,
    detail: Detail,
    bg_a: f32,
    bg_b: f32,
    sat: f32,
    orbs: Vec<Orb>,
    event: Event,
    next_event: f32,
    flash: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Candy {
    pub fn new(mut rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (bg_a, bg_b, sat) = match theme {
            Some("sour") => (0.78, 0.92, 1.0),
            Some("pastel") => (0.55, 0.75, 0.55),
            Some("mono") => (0.0, 0.0, 0.0),
            _ => (0.02, 0.18, 0.95), // classic neon candy
        };
        Candy {
            rng,
            detail,
            bg_a,
            bg_b,
            sat,
            orbs: Vec::new(),
            event: Event::None,
            next_event: 8.0,
            flash: 0.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let n = self
            .detail
            .scale((w * h / 900) as f32 * density_for(w, h), 8)
            .clamp(8, 40);
        self.orbs = (0..n)
            .map(|i| {
                let layer = (i % 3) as u8;
                Orb {
                    x: self.rng.random_range(0.0..w as f32),
                    y: self.rng.random_range(0.0..h as f32),
                    vx: self.rng.random_range(-6.0..6.0),
                    vy: self.rng.random_range(-4.0..4.0),
                    r: self.rng.random_range(1.5..4.0) * (1.0 + layer as f32 * 0.35),
                    hue: self.rng.random_range(0.0..1.0),
                    layer,
                    phase: self.rng.random_range(0.0..6.28),
                }
            })
            .collect();
    }
}

impl Scene for Candy {
    fn name(&self) -> &'static str {
        "candy"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if w != self.w || h != self.h {
            self.init(w, h);
        }
        self.t += dt;

        self.next_event -= dt;
        if self.next_event <= 0.0 && self.event == Event::None {
            self.event = Event::Sparkle(0.0);
            self.next_event = self.rng.random_range(16.0..26.0);
        }
        self.flash = 0.0;
        if let Event::Sparkle(ref mut e) = self.event {
            *e += dt;
            if *e < 0.8 {
                self.flash = ease_smooth(*e / 0.8);
            } else if *e < 2.5 {
                self.flash = 1.0;
            } else if *e < 4.0 {
                self.flash = 1.0 - ease_smooth((*e - 2.5) / 1.5);
            } else {
                self.event = Event::None;
            }
        }

        for y in 0..h {
            let ty = y as f32 / h as f32;
            let wave = (self.t * 0.15 + ty * 2.0).sin() * 0.04;
            let ha = (self.bg_a + wave).rem_euclid(1.0);
            let hb = (self.bg_b + wave * 0.5).rem_euclid(1.0);
            let left = hsv(ha, self.sat * 0.7, 0.12 + ty * 0.08);
            let right = hsv(hb, self.sat * 0.8, 0.15 + (1.0 - ty) * 0.1);
            for x in 0..w {
                let tx = x as f32 / w as f32;
                canvas.set(x as i32, y as i32, lerp(left, right, tx));
            }
        }

        if self.flash > 0.0 {
            for y in 0..h {
                for x in 0..w {
                    canvas.add(
                        x as i32,
                        y as i32,
                        scale((255, 255, 255), self.flash * 0.06),
                    );
                }
            }
        }

        self.orbs.sort_by_key(|o| o.layer);
        for o in &mut self.orbs {
            o.x += o.vx * dt * (0.6 + o.layer as f32 * 0.2);
            o.y += o.vy * dt * (0.6 + o.layer as f32 * 0.2);
            if o.x < -5.0 {
                o.x = w as f32 + 5.0;
            }
            if o.x > w as f32 + 5.0 {
                o.x = -5.0;
            }
            if o.y < -5.0 {
                o.y = h as f32 + 5.0;
            }
            if o.y > h as f32 + 5.0 {
                o.y = -5.0;
            }
            let pulse = 0.75 + 0.25 * (self.t * 2.0 + o.phase).sin();
            let v = (0.45 + o.layer as f32 * 0.18) * pulse * (1.0 + self.flash * 0.35);
            let col = hsv((o.hue + self.t * 0.02).fract(), self.sat, v);
            let rad = o.r as i32;
            glow(canvas, o.x as i32, o.y as i32, rad + 1, col, 0.35);
            for dy in -rad..=rad {
                for dx in -rad..=rad {
                    if dx * dx + dy * dy <= rad * rad {
                        canvas.set(
                            o.x as i32 + dx,
                            o.y as i32 + dy,
                            scale(col, 0.85 + 0.15 * pulse),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn candy_runs_all_themes() {
        for theme in [None, Some("sour"), Some("pastel"), Some("mono")] {
            let mut c = Candy::new(StdRng::seed_from_u64(1), theme, Detail::Medium);
            let mut cv = Canvas::new(80, 40);
            for _ in 0..90 {
                c.update(1.0 / 30.0, &mut cv);
            }
        }
    }
}
