//! Grand-finale fireworks: near-constant sky over a sparse twinkling
//! starfield, all 11 shell types, rapid salvos, and periodic walls of
//! simultaneous bursts with a sky flash. Every detonation feeds an ambient
//! light envelope that rims the foreground crowd.

use super::{crowd::Crowd, shells, Detail, Scene};
use crate::canvas::{density_for, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

struct BgStar {
    x: f32,
    y: f32,
    mag: f32,
    phase: f32,
}

struct Rocket {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    target_y: f32,
    hue: f32,
    shell: shells::Shell,
}

pub struct Finale {
    rng: StdRng,
    detail: Detail,
    hue_mode: u8,
    crowd: Option<Crowd>,
    rockets: Vec<Rocket>,
    particles: Vec<shells::ShellParticle>,
    flashes: Vec<(f32, f32, f32, f32)>, // x, y, life, radius
    bg_stars: Vec<BgStar>,
    bursts: Vec<(f32, f32, f32, shells::Shell)>, // reused spawn queue
    sky_flash: f32,
    burst_light: f32, // ambient light from detonations: rims the crowd
    dens: f32,
    next_launch: f32,
    next_wall: f32,
    t: f32,
    w: usize,
    h: usize,
}

impl Finale {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let hue_mode = match theme {
            Some("royal") => 1,
            Some("ember") => 2,
            Some("mono-gold") => 3,
            _ => 0,
        };
        Finale {
            rng,
            detail,
            hue_mode,
            crowd: None,
            rockets: Vec::new(),
            particles: Vec::new(),
            flashes: Vec::new(),
            bg_stars: Vec::new(),
            bursts: Vec::new(),
            sky_flash: 0.0,
            burst_light: 0.0,
            dens: 1.0,
            next_launch: 0.3,
            next_wall: 14.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    fn launch(&mut self, n: usize) {
        let (w, h) = (self.w.max(10), self.h.max(10));
        for _ in 0..n {
            let angled = self.rng.random::<f32>() < 0.2;
            self.rockets.push(Rocket {
                x: self.rng.random_range(w as f32 * 0.06..w as f32 * 0.94),
                y: h as f32 - 1.0,
                vx: if angled {
                    self.rng.random_range(-12.0..12.0)
                } else {
                    0.0
                },
                vy: -self.rng.random_range(h as f32 * 1.2..h as f32 * 1.6),
                target_y: self.rng.random_range(h as f32 * 0.10..h as f32 * 0.40),
                hue: match self.hue_mode {
                    1 => self.rng.random_range(0.6..0.8),
                    2 => self.rng.random_range(0.0..0.1),
                    3 => self.rng.random_range(0.08..0.14),
                    _ => self.rng.random::<f32>(),
                },
                shell: shells::pick_shell(&mut self.rng),
            });
        }
    }
}

impl Scene for Finale {
    fn name(&self) -> &'static str {
        "finale"
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
            self.dens = density_for(w, h);
            self.rockets.clear();
            self.particles.clear();
            self.flashes.clear();
            self.next_launch = 0.2;
            self.next_wall = self.rng.random_range(10.0..16.0);
            self.crowd = Some(Crowd::new(&mut self.rng, w, h, 0.45));
            // background plane: sparse fixed stars above the hill, breathing
            let nb = self
                .detail
                .scale((w * h / 260) as f32 * self.dens, 12)
                .min(350);
            self.bg_stars = (0..nb)
                .map(|_| BgStar {
                    x: self.rng.random_range(0.0..w as f32),
                    y: self.rng.random_range(0.0..h as f32 * 0.88),
                    mag: self.rng.random_range(0.08..0.26),
                    phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                })
                .collect();
        }
        self.t += dt;

        // launch pacing: rapid salvos, plus periodic wall-of-bursts
        self.next_launch -= dt;
        if self.next_launch <= 0.0 {
            let n = self
                .detail
                .scale(self.rng.random_range(3..=6) as f32 * self.dens, 3);
            self.launch(n);
            self.next_launch = self.rng.random_range(0.5..1.0);
        }
        self.next_wall -= dt;
        if self.next_wall <= 0.0 {
            let n = self
                .detail
                .scale(self.rng.random_range(7..=10) as f32 * self.dens, 6);
            self.launch(n);
            self.sky_flash = 0.35;
            self.next_wall = self.rng.random_range(14.0..22.0);
        }

        // sky: uniform black with a decaying wall-flash tint
        self.sky_flash = (self.sky_flash - dt).max(0.0);
        let flash = (self.sky_flash / 0.35).powi(2);
        canvas.clear(lerp((0, 0, 0), (70, 70, 90), flash * 0.6));

        // stars twinkle through the smoke
        const STAR_TINTS: [(u8, u8, u8); 3] =
            [(255, 250, 235), (205, 215, 245), (255, 225, 190)];
        for (i, s) in self.bg_stars.iter().enumerate() {
            let tw = 0.7 + 0.3 * (self.t * 0.8 + s.phase).sin();
            canvas.set_f(s.x, s.y, scale(STAR_TINTS[i % STAR_TINTS.len()], s.mag * tw));
        }

        // ambient detonation light: exponential decay, bumped per burst
        self.burst_light *= (1.0 - 2.5 * dt).max(0.0);

        // detonation flashes
        self.flashes.retain_mut(|(fx, fy, life, rad)| {
            *life -= dt;
            if *life <= 0.0 {
                return false;
            }
            let b = *life / 0.16;
            let r = *rad as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d2 = (dx * dx + dy * dy) as f32;
                    if d2 <= (*rad) * (*rad) {
                        let s = (b * (1.0 - d2 / ((*rad) * (*rad) + 1.0)) * 130.0) as u8;
                        canvas.add(*fx as i32 + dx, *fy as i32 + dy, (s, s, (s as f32 * 0.9) as u8));
                    }
                }
            }
            true
        });

        // rockets
        let rng = &mut self.rng;
        let particles = &mut self.particles;
        let mut bursts = std::mem::take(&mut self.bursts);
        self.rockets.retain_mut(|r| {
            r.vy += 8.0 * dt;
            r.x += r.vx * dt;
            r.y += r.vy * dt;
            for _ in 0..2 {
                let life = rng.random_range(0.1..0.35);
                let mut p = shells::ShellParticle::basic(
                    r.x + rng.random_range(-0.5..0.5),
                    r.y + rng.random_range(0.0..1.0),
                    rng.random_range(-3.0..3.0),
                    rng.random_range(2.0..8.0),
                    life,
                    (255, 210, 130),
                );
                p.g = 8.0;
                p.drag = 1.0;
                particles.push(p);
            }
            canvas.add(r.x as i32, r.y as i32, (255, 235, 180));
            if r.y <= r.target_y || r.vy > -10.0 {
                bursts.push((r.x, r.y, r.hue, r.shell));
                false
            } else {
                true
            }
        });
        for (x, y, hue, shell) in bursts.drain(..) {
            self.flashes.push((x, y, 0.16, 3.0));
            self.burst_light = (self.burst_light + 0.3).min(1.0);
            shells::spawn_shell(
                &mut self.rng,
                &mut self.particles,
                x,
                y,
                shell,
                hue,
                self.detail.factor(),
            );
        }
        self.bursts = bursts;

        // ground fountains: brief mines spraying between shell launches
        if self.rng.random::<f32>() < dt * 0.5 {
            let fx = self.rng.random_range(w as f32 * 0.1..w as f32 * 0.9);
            let hue = self.rng.random::<f32>();
            for _ in 0..self.detail.scale(10.0, 4) {
                let a = -std::f32::consts::FRAC_PI_2 + self.rng.random_range(-0.35..0.35);
                let sp = self.rng.random_range(20.0..38.0);
                let life = self.rng.random_range(0.8..1.4);
                let mut sp_p = shells::ShellParticle::basic(
                    fx,
                    h as f32 - 2.0,
                    a.cos() * sp,
                    a.sin() * sp,
                    life,
                    crate::canvas::hsv(hue, 0.8, 1.0),
                );
                sp_p.g = 30.0;
                sp_p.trail = true;
                self.particles.push(sp_p);
            }
        }

        shells::step_particles(&mut self.rng, &mut self.particles, canvas, dt);

        // foreground crowd: everyone films the finale; every burst rims them
        if let Some(c) = &self.crowd {
            c.draw(canvas, flash.min(1.0).max(self.burst_light));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn finale_keeps_the_sky_busy() {
        let mut fin = Finale::new(StdRng::seed_from_u64(31), None, Detail::Medium);
        let mut c1 = Canvas::new(100, 50);
        let mut lit = 0usize;
        for _ in 0..400 {
            fin.update(1.0 / 30.0, &mut c1);
            // count only the sky region (bottom rows are the crowd)
            lit += (0..100)
                .flat_map(|x| (0..40).map(move |y| (x, y)))
                .filter(|&(x, y)| {
                    let (r, g, b) = c1.get(x, y).color;
                    r as u32 + g as u32 + b as u32 > 40
                })
                .count();
        }
        // ~11 shell types on a near-constant cadence should average well
        // over a hundred lit sky pixels per frame
        assert!(lit / 400 > 100, "sky too quiet: {} lit/frame avg", lit / 400);
    }
}
