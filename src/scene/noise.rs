//! Shared value-noise / fBm helpers for procedural scenes.
#![allow(dead_code)]

/// Deterministic lattice hash → 0..1.
pub fn hash2(ix: i32, iy: i32, seed: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x8da6b343)
        ^ (iy as u32).wrapping_mul(0xd8163841)
        ^ seed.wrapping_mul(0xcb1ab31f);
    h = h.wrapping_mul(0x9e3779b1);
    h ^= h >> 15;
    (h % 10000) as f32 / 10000.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise at (x, y), bilinear with smoothstep.
pub fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (smooth(x.fract()), smooth(y.fract()));
    let a = hash2(ix, iy, seed);
    let b = hash2(ix.wrapping_add(1), iy, seed);
    let c = hash2(ix, iy.wrapping_add(1), seed);
    let d = hash2(ix.wrapping_add(1), iy.wrapping_add(1), seed);
    a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy
}

/// Fractal brownian motion, 0..~1.
pub fn fbm(x: f32, y: f32, octaves: usize, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += amp * vnoise(x * freq, y * freq, seed.wrapping_add(o as u32 * 7919));
        norm += amp;
        amp *= 0.5;
        freq *= 2.1;
    }
    sum / norm
}

