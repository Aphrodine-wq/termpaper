//! Shared firework shell library: 11 shell types with weighted selection,
//! per-type colors and behavior. Used by `finale`.

use crate::canvas::{hsv, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shell {
    Peony,
    Chrysanthemum,
    Willow,
    Strobe,
    Ring,
    DoubleRing,
    Heart,
    Crossette,
    Crackle,
    ColorChange,
    Waterfall,
}

#[cfg(test)]
pub const SHELL_KINDS: &[Shell] = &[
    Shell::Peony,
    Shell::Chrysanthemum,
    Shell::Willow,
    Shell::Strobe,
    Shell::Ring,
    Shell::DoubleRing,
    Shell::Heart,
    Shell::Crossette,
    Shell::Crackle,
    Shell::ColorChange,
    Shell::Waterfall,
];

/// Weighted random shell pick.
pub fn pick_shell(rng: &mut StdRng) -> Shell {
    match rng.random_range(0..20) {
        0..=3 => Shell::Peony,
        4..=5 => Shell::Chrysanthemum,
        6..=7 => Shell::Willow,
        8 => Shell::Strobe,
        9 => Shell::Ring,
        10 => Shell::DoubleRing,
        11 => Shell::Heart,
        12..=13 => Shell::Crossette,
        14..=15 => Shell::Crackle,
        16 => Shell::ColorChange,
        _ => Shell::Waterfall,
    }
}

#[derive(Clone)]
pub struct ShellParticle {
    pub x: f32,
    pub y: f32,
    pub px: f32,
    pub py: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max_life: f32,
    pub color: (u8, u8, u8),
    pub g: f32,
    pub drag: f32,
    pub trail: bool,
    pub strobe: bool,
    pub hue: f32,
    pub hue_drift: f32, // color-change shells rotate mid-fall
    pub crackle_on_death: bool,
    pub split_at: f32,
}

impl ShellParticle {
    pub(crate) fn basic(x: f32, y: f32, vx: f32, vy: f32, life: f32, color: (u8, u8, u8)) -> Self {
        ShellParticle {
            x,
            y,
            px: x,
            py: y,
            vx,
            vy,
            life,
            max_life: life,
            color,
            g: 16.0,
            drag: 0.55,
            trail: false,
            strobe: false,
            hue: 0.0,
            hue_drift: 0.0,
            crackle_on_death: false,
            split_at: 0.0,
        }
    }
}

const GOLD: f32 = 0.105;

/// Spawn a shell burst of `kind` at (x, y). `hue` is the rocket's hue;
/// several types override it with their own color rules.
pub fn spawn_shell(
    rng: &mut StdRng,
    out: &mut Vec<ShellParticle>,
    x: f32,
    y: f32,
    kind: Shell,
    hue: f32,
    detail: f32,
) {
    let n = |base: usize| ((base as f32 * detail) as usize).max(12);
    match kind {
        Shell::Peony => {
            let base = rng.random_range(16.0..26.0);
            for _ in 0..n(70) {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let s = base * rng.random_range(0.2..1.0);
                let life = rng.random_range(1.4..2.4);
                out.push(ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.92,
                    life,
                    hsv(hue + rng.random_range(-0.03..0.03), 0.85, 1.0),
                ));
            }
        }
        Shell::Chrysanthemum => {
            let base = rng.random_range(16.0..24.0);
            for _ in 0..n(90) {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let s = base * rng.random_range(0.25..1.0);
                let life = rng.random_range(1.8..2.8);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.92,
                    life,
                    hsv(hue, 0.8, 1.0),
                );
                p.trail = true;
                out.push(p);
            }
        }
        Shell::Willow => {
            for _ in 0..n(60) {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let s = rng.random_range(10.0..20.0);
                let life = rng.random_range(2.6..3.8);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.8,
                    life,
                    hsv(GOLD, 0.75, 0.95),
                );
                p.g = 26.0; // heavy sag: drooping gold willow
                p.drag = 0.35;
                p.trail = true;
                out.push(p);
            }
        }
        Shell::Strobe => {
            for _ in 0..n(65) {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let s = rng.random_range(12.0..22.0);
                let life = rng.random_range(1.6..2.6);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.9,
                    life,
                    hsv(hue, 0.4, 1.0),
                );
                p.strobe = true;
                out.push(p);
            }
        }
        Shell::Ring => {
            let npts = n(50);
            let s = rng.random_range(18.0..24.0);
            for i in 0..npts {
                let a = i as f32 / npts as f32 * std::f32::consts::TAU;
                let life = rng.random_range(1.3..1.7);
                out.push(ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.92,
                    life,
                    hsv(hue, 0.85, 1.0),
                ));
            }
        }
        Shell::DoubleRing => {
            let hue2 = hue + 0.33;
            for (ring_hue, s) in [(hue, 22.0f32), (hue2, 13.0)] {
                let npts = n(40);
                for i in 0..npts {
                    let a = i as f32 / npts as f32 * std::f32::consts::TAU;
                    let life = rng.random_range(1.3..1.8);
                    out.push(ShellParticle::basic(
                        x,
                        y,
                        a.cos() * s,
                        a.sin() * s * 0.92,
                        life,
                        hsv(ring_hue, 0.85, 1.0),
                    ));
                }
            }
        }
        Shell::Heart => {
            // parametric heart: x=16sin³t, y=13cos t−5cos2t−2cos3t−cos4t
            let npts = n(56);
            let s = rng.random_range(1.0..1.4);
            for i in 0..npts {
                let t = i as f32 / npts as f32 * std::f32::consts::TAU;
                let hx = 16.0 * t.sin().powi(3) / 16.0;
                let hy = -(13.0 * t.cos() - 5.0 * (2.0 * t).cos()
                    - 2.0 * (3.0 * t).cos()
                    - (4.0 * t).cos())
                    / 16.0;
                let life = rng.random_range(1.5..2.0);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    hx * s * 14.0,
                    hy * s * 14.0,
                    life,
                    hsv(rng.random_range(0.9..1.05), 0.85, 1.0), // reds/pinks
                );
                p.g = 8.0;
                out.push(p);
            }
        }
        Shell::Crossette => {
            let base_a = rng.random_range(0.0..std::f32::consts::TAU);
            for k in 0..4 {
                let a = base_a + k as f32 * std::f32::consts::FRAC_PI_2;
                let life = rng.random_range(0.5..0.7);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    a.cos() * 18.0,
                    a.sin() * 18.0,
                    life,
                    hsv(hue, 0.8, 1.0),
                );
                p.split_at = life * 0.55;
                out.push(p);
            }
        }
        Shell::Crackle => {
            let base = rng.random_range(14.0..22.0);
            for _ in 0..n(55) {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let s = base * rng.random_range(0.3..1.0);
                let life = rng.random_range(0.9..1.4);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.9,
                    life,
                    hsv(hue, 0.85, 1.0),
                );
                p.crackle_on_death = true;
                out.push(p);
            }
        }
        Shell::ColorChange => {
            let base = rng.random_range(15.0..22.0);
            for _ in 0..n(70) {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let s = base * rng.random_range(0.25..1.0);
                let life = rng.random_range(1.8..2.6);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.9,
                    life,
                    hsv(hue, 0.85, 1.0),
                );
                p.hue = hue;
                p.hue_drift = rng.random_range(0.15..0.35);
                out.push(p);
            }
        }
        Shell::Waterfall => {
            // one shell → a long gold curtain drooping straight down
            for _ in 0..n(80) {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let s = rng.random_range(8.0..16.0);
                let life = rng.random_range(3.0..4.5);
                let mut p = ShellParticle::basic(
                    x,
                    y,
                    a.cos() * s,
                    a.sin() * s * 0.6,
                    life,
                    hsv(GOLD, 0.7, 0.9),
                );
                p.g = 20.0;
                p.drag = 0.25;
                p.trail = true;
                out.push(p);
            }
        }
    }
}

/// Step + draw all shell particles. Handles strobe blinking, hue drift,
/// crossette splits and crackle-on-death.
pub fn step_particles(
    rng: &mut StdRng,
    particles: &mut Vec<ShellParticle>,
    canvas: &mut Canvas,
    dt: f32,
) {
    let mut spawned: Vec<ShellParticle> = Vec::new();
    particles.retain_mut(|p| {
        p.life -= dt;
        if p.life <= 0.0 {
            if p.crackle_on_death {
                for _ in 0..rng.random_range(4..8) {
                    let a = rng.random_range(0.0..std::f32::consts::TAU);
                    let s = rng.random_range(2.0..9.0);
                    let life = rng.random_range(0.15..0.45);
                    let mut c = ShellParticle::basic(
                        p.x,
                        p.y,
                        p.vx * 0.3 + a.cos() * s,
                        p.vy * 0.3 + a.sin() * s,
                        life,
                        (255, 245, 220),
                    );
                    c.g = 6.0;
                    c.drag = 0.8;
                    spawned.push(c);
                }
            }
            return false;
        }
        if p.split_at > 0.0 && p.life < p.split_at {
            p.split_at = 0.0;
            let base_a = rng.random_range(0.0..std::f32::consts::TAU);
            for k in 0..4 {
                let a = base_a + k as f32 * std::f32::consts::FRAC_PI_2;
                let life = rng.random_range(0.8..1.3);
                let mut c = ShellParticle::basic(
                    p.x,
                    p.y,
                    a.cos() * 12.0,
                    a.sin() * 12.0,
                    life,
                    p.color,
                );
                c.g = 12.0;
                c.drag = 0.4;
                c.trail = true;
                spawned.push(c);
            }
            return false;
        }
        p.px = p.x;
        p.py = p.y;
        p.vy += p.g * dt;
        let d = 1.0 - p.drag * dt;
        p.vx *= d;
        p.vy *= d;
        p.x += p.vx * dt;
        p.y += p.vy * dt;

        let f = p.life / p.max_life;
        if p.hue_drift > 0.0 {
            p.hue += p.hue_drift * dt;
            p.color = hsv(p.hue, 0.85, (0.4 + 0.6 * f).min(1.0));
        }
        if p.strobe {
            // hard blink with 2-frame soft edge
            let phase = (p.life * 14.0).fract();
            if phase < 0.45 && phase > 0.05 {
                return true; // dark part of the blink
            }
        }
        if p.trail {
            canvas.set_f(
                p.px + (p.x - p.px) * 0.5,
                p.py + (p.y - p.py) * 0.5,
                scale(p.color, f * f * 0.5),
            );
        }
        if f < 0.25 && rng.random::<f32>() < 0.3 {
            return true; // ember twinkle-out
        }
        canvas.set_f(p.x, p.y, scale(p.color, f * f));
        true
    });
    particles.append(&mut spawned);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn registry_has_at_least_nine_types() {
        assert!(SHELL_KINDS.len() >= 9);
        // every kind spawns particles and they step cleanly
        let mut rng = StdRng::seed_from_u64(5);
        for &kind in SHELL_KINDS {
            let mut ps = Vec::new();
            spawn_shell(&mut rng, &mut ps, 50.0, 20.0, kind, 0.3, 1.0);
            assert!(!ps.is_empty(), "{kind:?} spawned nothing");
            let mut c = Canvas::new(100, 50);
            for _ in 0..30 {
                step_particles(&mut rng, &mut ps, &mut c, 1.0 / 30.0);
            }
            for p in &ps {
                assert!(p.x.is_finite() && p.y.is_finite(), "{kind:?} produced NaN");
            }
        }
    }

    #[test]
    fn heart_shape_bounds() {
        let mut rng = StdRng::seed_from_u64(6);
        let mut ps = Vec::new();
        spawn_shell(&mut rng, &mut ps, 50.0, 20.0, Shell::Heart, 0.0, 1.0);
        for p in &ps {
            assert!(p.vx.abs() < 25.0 && p.vy.abs() < 25.0, "heart curve extents");
        }
    }
}
