//! Foreground crowd for fireworks scenes: a dark hill silhouette with
//! individual onlookers (standing, sitting, kids on shoulders), backlit
//! rims during bright bursts, and phone screens that glow when recording.

use crate::canvas::{lerp, Canvas};
use rand::{rngs::StdRng, RngExt};

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Standing,
    Sitting,
    KidOnShoulders,
}

struct Person {
    x: i32,
    shape: Shape,
    phone: bool,
}

pub struct Crowd {
    /// hill top y per column
    hill_top: Vec<i32>,
    people: Vec<Person>,
}

/// Person bitmaps, drawn bottom-up. (width, rows top→bottom of '#'/'.')
fn shape_rows(s: Shape) -> (i32, &'static [&'static str]) {
    match s {
        Shape::Standing => (
            3,
            &[".#.", "###", ".#.", "###", "#.#"],
        ),
        Shape::Sitting => (
            4,
            &[".##.", "####", "####"],
        ),
        Shape::KidOnShoulders => (
            3,
            &[".#.", ".#.", "###", ".#.", "###", "#.#"],
        ),
    }
}

impl Crowd {
    pub fn new(rng: &mut StdRng, w: usize, h: usize, phone_density: f32) -> Self {
        // rolling hill along the bottom, 8-14% of canvas height
        let hill_top: Vec<i32> = (0..w)
            .map(|x| {
                let xf = x as f32;
                let base = h as f32 * 0.10
                    + (xf * 0.05).sin() * h as f32 * 0.02
                    + (xf * 0.013 + 2.0).sin() * h as f32 * 0.025;
                (h as f32 - base.max(h as f32 * 0.06)) as i32
            })
            .collect();

        let mut people = Vec::new();
        let mut x = 2;
        while x < w as i32 - 4 {
            if rng.random::<f32>() < 0.75 {
                let shape = match rng.random_range(0..10) {
                    0..=4 => Shape::Standing,
                    5..=7 => Shape::Sitting,
                    _ => Shape::KidOnShoulders,
                };
                people.push(Person {
                    x,
                    shape,
                    phone: rng.random::<f32>() < phone_density,
                });
            }
            x += rng.random_range(3..6);
        }
        Crowd { hill_top, people }
    }

    /// Draw over the scene. `flash` in 0..1 is the current sky brightness.
    pub fn draw(&self, canvas: &mut Canvas, flash: f32) {
        let (w, h) = (canvas.width(), canvas.height());
        let hill = lerp((9, 9, 16), (70, 75, 100), flash * 0.5);
        let crest = lerp((14, 14, 24), (120, 128, 160), flash * 0.7);
        let body = lerp((2, 2, 5), (30, 32, 48), flash * 0.4);
        let rim = lerp((8, 8, 14), (215, 222, 245), flash * 0.8);

        // hill body
        for x in 0..w.min(self.hill_top.len()) {
            let top = self.hill_top[x];
            for y in top.max(0)..h as i32 {
                canvas.set(x as i32, y, if y == top { crest } else { hill });
            }
        }

        // people
        for p in &self.people {
            let (pw, rows) = shape_rows(p.shape);
            let top = *self.hill_top.get(p.x as usize).unwrap_or(&(h as i32)) - rows.len() as i32;
            for (ri, row) in rows.iter().enumerate() {
                for (ci, ch) in row.chars().enumerate() {
                    if ch != '#' {
                        continue;
                    }
                    let (px, py) = (p.x + ci as i32, top + ri as i32);
                    // head row catches the rim light
                    canvas.set(px, py, if ri == 0 { rim } else { body });
                }
            }
            // phone: tiny rectangle held up, glowing harder while recording
            if p.phone {
                let glow = 0.35 + 0.65 * flash;
                let screen = lerp((70, 80, 110), (200, 215, 255), glow);
                let phx = p.x + pw; // held to the side
                let phy = top + 1;
                canvas.set(phx, phy, screen);
                canvas.set(phx, phy + 1, screen);
            }
        }
    }

    #[cfg(test)]
    pub fn person_count(&self) -> usize {
        self.people.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas;
    use rand::SeedableRng;

    #[test]
    fn crowd_renders_and_backlight_responds_to_flash() {
        let mut rng = StdRng::seed_from_u64(77);
        let crowd = Crowd::new(&mut rng, 100, 50, 0.4);
        assert!(crowd.person_count() > 5, "should have a crowd");

        let mut dark = Canvas::new(100, 50);
        dark.clear((1, 1, 6));
        crowd.draw(&mut dark, 0.0);
        let mut lit = Canvas::new(100, 50);
        lit.clear((1, 1, 6));
        crowd.draw(&mut lit, 1.0);

        let mut dark_sum = 0u64;
        let mut lit_sum = 0u64;
        for y in 0..50 {
            for x in 0..100 {
                let d = dark.get(x, y).color;
                let l = lit.get(x, y).color;
                dark_sum += d.0 as u64 + d.1 as u64 + d.2 as u64;
                lit_sum += l.0 as u64 + l.1 as u64 + l.2 as u64;
            }
        }
        assert!(
            lit_sum > dark_sum + 500,
            "flash should brighten rims/hill: dark {dark_sum} lit {lit_sum}"
        );
    }
}
