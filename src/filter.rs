//! Composable post-processing filters applied to the finished canvas
//! before blitting. Ordered, cheap, pure functions (grain takes a seed).
//!
//! Every effect takes a strength ("amount", 0 ..= 2, 1 = the classic look).
//! [`params`] turns a strength into the constants the effect runs with; the
//! GPU passes receive exactly those values, so both sides compute with
//! identical f32s. At strength 1 every constant is the original one, bit
//! for bit (`base + (k - base) * 1.0` is exact for these ranges).

use crate::canvas::{hsv, Canvas};
use rand::{rngs::StdRng, RngExt, SeedableRng};

/// Available filter names, in quick-cycle order.
pub const FILTER_CYCLE: &[&str] = &[
    "scanlines",
    "vignette",
    "grain",
    "warm",
    "cool",
    "hue",
    "crt",
    "bloom",
    "duotone",
    "pixelate",
    "chroma",
    "spectrum",
    "edges",
    "thermal",
    "warp",
    "invert",
    "sepia",
    "posterize",
    "gamma",
    "sharpen",
    "mirror",
    "noir",
    "letterbox",
    "halation",
    "dither",
    "tiltshift",
    "kaleido",
];

/// Effects whose strength does nothing (on or off only).
pub fn has_amount(name: &str) -> bool {
    !matches!(name, "mirror" | "kaleido")
}

/// The constants an effect runs with at strength `a`. Layout per effect:
///
/// | effect | values |
/// |---|---|
/// | scanlines | row factor |
/// | vignette | edge darkening |
/// | grain | noise range (levels) |
/// | warm, cool | r, g, b multipliers |
/// | hue | degrees |
/// | spectrum | degrees per second |
/// | crt | chroma scale, row factor, edge darkening |
/// | duotone, thermal, invert, sepia | mix toward the effect (0..1) |
/// | pixelate | block size |
/// | chroma | fringe scale |
/// | edges, sharpen | gain |
/// | warp | amplitude (px) |
/// | posterize, dither | levels per channel |
/// | gamma | exponent |
/// | noir | contrast |
/// | letterbox | bar height (fraction of the frame, each side) |
/// | tiltshift | strength, sharp band (half height), blur radius |
///
/// Bloom and halation have their own, [`bloom_params`].
pub fn params(name: &str, a: f32) -> [f32; 4] {
    let a = if a.is_finite() { a.clamp(0.0, 2.0) } else { 1.0 };
    let lerp = |base: f32, k: f32| base + (k - base) * a;
    match name {
        "scanlines" => [lerp(1.0, 0.72).max(0.0), 0.0, 0.0, 0.0],
        "vignette" => [0.45 * a, 0.0, 0.0, 0.0],
        "grain" => [(14.0 * a).round(), 0.0, 0.0, 0.0],
        "warm" => [lerp(1.0, 1.10), 1.0, lerp(1.0, 0.88), 0.0],
        "cool" => [lerp(1.0, 0.88), 1.0, lerp(1.0, 1.12), 0.0],
        "hue" => [120.0 * a, 0.0, 0.0, 0.0],
        "spectrum" => [30.0 * a, 0.0, 0.0, 0.0],
        "crt" => [2.0 * a, lerp(1.0, 0.72).max(0.0), 0.45 * a, 0.0],
        "duotone" | "thermal" | "invert" | "sepia" => [a.min(1.0), 0.0, 0.0, 0.0],
        "pixelate" => [(1.0 + 2.0 * a).round().clamp(2.0, 8.0), 0.0, 0.0, 0.0],
        "chroma" => [2.0 * a, 0.0, 0.0, 0.0],
        "edges" | "sharpen" => [a, 0.0, 0.0, 0.0],
        "warp" => [2.0 * a, 0.0, 0.0, 0.0],
        "posterize" => [(9.0 - 4.0 * a).round().clamp(2.0, 16.0), 0.0, 0.0, 0.0],
        "dither" => [(6.0 - 2.0 * a).round().clamp(2.0, 8.0), 0.0, 0.0, 0.0],
        "gamma" => [lerp(1.0, 1.35), 0.0, 0.0, 0.0],
        "noir" => [lerp(1.0, 1.6), 0.0, 0.0, 0.0],
        "letterbox" => [0.12 * a, 0.0, 0.0, 0.0],
        "tiltshift" => [a.min(1.5), 0.18, 3.0, 0.0],
        _ => [1.0, 0.0, 0.0, 0.0],
    }
}

/// Bright-pass threshold (on the `(2r+3g+b)/6` luminance), blur radius and
/// per-channel gains for `bloom` and `halation` (a warm film glow).
pub fn bloom_params(name: &str, a: f32) -> (u32, u32, [f32; 3]) {
    let a = if a.is_finite() { a.clamp(0.0, 2.0) } else { 1.0 };
    match name {
        "halation" => {
            let s = 0.55 * a;
            (150, 3, [s, s * 0.42, s * 0.18])
        }
        _ => {
            let s = 0.4 * a;
            (180, 2, [s, s, s])
        }
    }
}

/// Apply one named filter at its classic strength. `t` is seconds (animated
/// effects); unknown names are ignored.
pub fn apply(name: &str, canvas: &mut Canvas, t: f32) {
    apply_with(name, canvas, t, 1.0);
}

/// Apply one named filter at strength `a`.
pub fn apply_with(name: &str, canvas: &mut Canvas, t: f32, a: f32) {
    if a <= 0.0 && has_amount(name) {
        return;
    }
    let p = params(name, a);
    match name {
        "scanlines" => scanlines_with(canvas, p[0]),
        "vignette" => vignette_with(canvas, p[0]),
        "grain" => grain_with(canvas, t, p[0] as i32),
        "warm" | "cool" => shift(canvas, p[0], p[1], p[2]),
        "hue" => hue(canvas, p[0]),
        "crt" => crt_with(canvas, p[0], p[1], p[2]),
        "bloom" | "halation" => {
            let (th, r, gains) = bloom_params(name, a);
            bloom_with(canvas, th, r as usize, gains)
        }
        "duotone" => duotone_with(canvas, p[0]),
        "pixelate" => pixelate_with(canvas, p[0] as usize),
        "chroma" => chroma_with(canvas, p[0]),
        "spectrum" => hue(canvas, (t * p[0]).rem_euclid(360.0)),
        "edges" => edges_with(canvas, p[0]),
        "thermal" => thermal_with(canvas, p[0]),
        "warp" => warp_with(canvas, t, p[0]),
        "invert" => invert_with(canvas, p[0]),
        "sepia" => sepia_with(canvas, p[0]),
        "posterize" => posterize_with(canvas, p[0]),
        "gamma" => gamma_with(canvas, p[0]),
        "sharpen" => sharpen_with(canvas, p[0]),
        "mirror" => mirror(canvas),
        "noir" => noir_with(canvas, p[0]),
        "letterbox" => letterbox(canvas, p[0]),
        "dither" => dither(canvas, p[0]),
        "tiltshift" => tiltshift(canvas, p[0], p[1], p[2] as usize),
        "kaleido" => kaleido(canvas),
        _ => {}
    }
}

pub fn apply_all(names: &[String], canvas: &mut Canvas, t: f32) {
    for n in names {
        apply(n, canvas, t);
    }
}

/// Apply a Look's effect stack in order, each at its strength.
pub fn apply_stack(effects: &crate::look::Effects, canvas: &mut Canvas, t: f32) {
    for n in &effects.stack {
        apply_with(n, canvas, t, effects.amount(n));
    }
}

/// Effects that read neighbouring pixels (blurs, offsets, kernels): on a
/// wall they need the scene rendered a little past the pane's edge.
pub fn reads_neighbours(name: &str) -> bool {
    matches!(
        name,
        "bloom" | "halation" | "crt" | "chroma" | "pixelate" | "edges" | "warp" | "sharpen" | "tiltshift"
    )
}

fn scale_cell(c: &mut (u8, u8, u8), f: f32) {
    c.0 = (c.0 as f32 * f).clamp(0.0, 255.0) as u8;
    c.1 = (c.1 as f32 * f).clamp(0.0, 255.0) as u8;
    c.2 = (c.2 as f32 * f).clamp(0.0, 255.0) as u8;
}

/// `a + (b - a) * t`, per channel, truncated like every u8 store here; at
/// `t >= 1` exactly `b`.
fn mix_to(a: (u8, u8, u8), b: (f32, f32, f32), t: f32) -> (u8, u8, u8) {
    if t >= 1.0 {
        return (b.0 as u8, b.1 as u8, b.2 as u8);
    }
    let m = |x: u8, y: f32| (x as f32 + (y - x as f32) * t).clamp(0.0, 255.0) as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Darken every other canvas pixel row.
pub fn scanlines(canvas: &mut Canvas) {
    scanlines_with(canvas, 0.72);
}

fn scanlines_with(canvas: &mut Canvas, f: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    for y in (1..h).step_by(2) {
        for x in 0..w {
            let mut c = canvas.get(x as i32, y as i32).color;
            scale_cell(&mut c, f);
            canvas.set(x as i32, y as i32, c);
        }
    }
}

/// Radial edge darkening.
pub fn vignette(canvas: &mut Canvas) {
    vignette_with(canvas, 0.45);
}

fn vignette_with(canvas: &mut Canvas, k: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    if w == 0 || h == 0 {
        return;
    }
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 - cx) / cx;
            let dy = (y as f32 - cy) / cy;
            let d = ((dx * dx + dy * dy).sqrt() / std::f32::consts::SQRT_2).min(1.0);
            let f = 1.0 - d * d * k;
            let mut c = canvas.get(x as i32, y as i32).color;
            scale_cell(&mut c, f);
            canvas.set(x as i32, y as i32, c);
        }
    }
}

/// Animated film grain, deterministic for a given frame index.
pub fn grain(canvas: &mut Canvas, t: f32) {
    grain_with(canvas, t, 14);
}

fn grain_with(canvas: &mut Canvas, t: f32, range: i32) {
    if range <= 0 {
        return;
    }
    let (w, h) = (canvas.width(), canvas.height());
    // seed per frame tick so the grain animates but stays reproducible
    let mut rng = StdRng::seed_from_u64((t * 30.0) as u64);
    for y in 0..h {
        for x in 0..w {
            let n = rng.random_range(-range..=range);
            let c = canvas.get(x as i32, y as i32).color;
            canvas.set(
                x as i32,
                y as i32,
                (
                    (c.0 as i32 + n).clamp(0, 255) as u8,
                    (c.1 as i32 + n).clamp(0, 255) as u8,
                    (c.2 as i32 + n).clamp(0, 255) as u8,
                ),
            );
        }
    }
}

/// Warm color shift.
pub fn warm(canvas: &mut Canvas) {
    shift(canvas, 1.10, 1.0, 0.88);
}

/// Cool color shift.
pub fn cool(canvas: &mut Canvas) {
    shift(canvas, 0.88, 1.0, 1.12);
}

fn shift(canvas: &mut Canvas, r: f32, g: f32, b: f32) {
    canvas.map_colors(|c| {
        (
            (c.0 as f32 * r).clamp(0.0, 255.0) as u8,
            (c.1 as f32 * g).clamp(0.0, 255.0) as u8,
            (c.2 as f32 * b).clamp(0.0, 255.0) as u8,
        )
    });
}

/// Rotate every pixel's hue by `deg` degrees.
pub fn hue(canvas: &mut Canvas, deg: f32) {
    let rot = deg / 360.0;
    canvas.map_colors(|c| hue_rotate(c, rot));
}

fn hue_rotate(c: (u8, u8, u8), rot: f32) -> (u8, u8, u8) {
    // cheap rgb→hsv→rgb round trip
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    if d < 1e-5 {
        return c; // gray: nothing to rotate
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    let s = if max < 1e-5 { 0.0 } else { d / max };
    hsv(h + rot, s, max)
}

/// CRT combo: scanlines + vignette + chromatic fringe at the edges.
pub fn crt(canvas: &mut Canvas) {
    crt_with(canvas, 2.0, 0.72, 0.45);
}

fn crt_with(canvas: &mut Canvas, fringe: f32, rows: f32, edge: f32) {
    chroma_with(canvas, fringe);
    scanlines_with(canvas, rows);
    vignette_with(canvas, edge);
}

/// Snapshot the canvas into a flat pixel vec.
fn snapshot(canvas: &Canvas) -> Vec<(u8, u8, u8)> {
    let (w, h) = (canvas.width(), canvas.height());
    let mut out = vec![(0u8, 0u8, 0u8); w * h];
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = canvas.get(x as i32, y as i32).color;
        }
    }
    out
}

/// Bloom: bright-pass (luminance > 180), separable box blur, added back at
/// 40%. Makes hot cores and lightning bleed light into their surroundings.
pub fn bloom(canvas: &mut Canvas) {
    let (th, r, gains) = bloom_params("bloom", 1.0);
    bloom_with(canvas, th, r as usize, gains);
}

/// The bloom family: bright pixels (luminance above `threshold`) blurred
/// over `radius` and added back with per-channel `gains` (halation is a warm
/// bloom). The GPU packs the horizontal leg to u8 between passes; so does
/// this, so the two agree.
fn bloom_with(canvas: &mut Canvas, threshold: u32, radius: usize, gains: [f32; 3]) {
    let (w, h) = (canvas.width(), canvas.height());
    if w < radius * 2 + 1 || h < radius * 2 + 1 {
        return;
    }
    let src = snapshot(canvas);
    let lum = |c: (u8, u8, u8)| (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) / 6;
    // bright-pass into a float buffer
    let mut buf = vec![(0f32, 0f32, 0f32); w * h];
    for (i, &c) in src.iter().enumerate() {
        if lum(c) > threshold {
            buf[i] = (c.0 as f32, c.1 as f32, c.2 as f32);
        }
    }
    // horizontal box blur
    let mut tmp = vec![(0f32, 0f32, 0f32); w * h];
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut n) = (0f32, 0f32, 0f32, 0f32);
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius).min(w - 1);
            for xx in x0..=x1 {
                let c = buf[y * w + xx];
                r += c.0;
                g += c.1;
                b += c.2;
                n += 1.0;
            }
            tmp[y * w + x] = (r / n, g / n, b / n);
        }
    }
    // vertical box blur + add
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut n) = (0f32, 0f32, 0f32, 0f32);
            let y0 = y.saturating_sub(radius);
            let y1 = (y + radius).min(h - 1);
            for yy in y0..=y1 {
                let c = tmp[yy * w + x];
                r += c.0;
                g += c.1;
                b += c.2;
                n += 1.0;
            }
            let c = canvas.get(x as i32, y as i32).color;
            canvas.set(
                x as i32,
                y as i32,
                (
                    (c.0 as f32 + r / n * gains[0]).min(255.0) as u8,
                    (c.1 as f32 + g / n * gains[1]).min(255.0) as u8,
                    (c.2 as f32 + b / n * gains[2]).min(255.0) as u8,
                ),
            );
        }
    }
}

/// Duotone: luminance mapped onto a black → accent gradient.
pub fn duotone(canvas: &mut Canvas) {
    duotone_with(canvas, 1.0);
}

fn duotone_with(canvas: &mut Canvas, t: f32) {
    const ACCENT: (u8, u8, u8) = (120, 180, 255);
    canvas.map_colors(|c| {
        let l = (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) as f32 / (6.0 * 255.0);
        let d = (
            (ACCENT.0 as f32 * l).floor(),
            (ACCENT.1 as f32 * l).floor(),
            (ACCENT.2 as f32 * l).floor(),
        );
        mix_to(c, d, t)
    });
}

/// Pixelate: 3x3 mosaic, each block becomes its average color.
pub fn pixelate(canvas: &mut Canvas) {
    pixelate_with(canvas, 3);
}

fn pixelate_with(canvas: &mut Canvas, block: usize) {
    let block = block.max(1);
    let (w, h) = (canvas.width(), canvas.height());
    if w < block || h < block {
        return;
    }
    let src = snapshot(canvas);
    for by in (0..h).step_by(block) {
        for bx in (0..w).step_by(block) {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for y in by..(by + block).min(h) {
                for x in bx..(bx + block).min(w) {
                    let c = src[y * w + x];
                    r += c.0 as u32;
                    g += c.1 as u32;
                    b += c.2 as u32;
                    n += 1;
                }
            }
            let avg = ((r / n) as u8, (g / n) as u8, (b / n) as u8);
            for y in by..(by + block).min(h) {
                for x in bx..(bx + block).min(w) {
                    canvas.set(x as i32, y as i32, avg);
                }
            }
        }
    }
}

/// Chroma: chromatic aberration only (the crt fringe, without the scanlines
/// and vignette) — red shifts left, blue right, growing toward the edges.
pub fn chroma(canvas: &mut Canvas) {
    chroma_with(canvas, 2.0);
}

fn chroma_with(canvas: &mut Canvas, scale: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    if w < 4 {
        return;
    }
    let src = snapshot(canvas);
    let cx = w as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let off = ((x as f32 - cx) / cx * scale) as i32;
            let xr = (x as i32 - off).clamp(0, w as i32 - 1) as usize;
            let xb = (x as i32 + off).clamp(0, w as i32 - 1) as usize;
            let mid = src[y * w + x];
            canvas.set(
                x as i32,
                y as i32,
                (src[y * w + xr].0, mid.1, src[y * w + xb].2),
            );
        }
    }
}

/// Spectrum: animated hue rotation, one full 360° cycle every ~12 seconds.
pub fn spectrum(canvas: &mut Canvas, t: f32) {
    hue(canvas, (t * 30.0).rem_euclid(360.0));
}

/// Edges: 3x3 Sobel-ish luminance gradient magnitude, drawn as a pale neon
/// glow on black — the input canvas goes dark and only edges shine.
pub fn edges(canvas: &mut Canvas) {
    edges_with(canvas, 1.0);
}

fn edges_with(canvas: &mut Canvas, gain: f32) {
    const NEON: (u8, u8, u8) = (170, 255, 225);
    let (w, h) = (canvas.width(), canvas.height());
    if w < 3 || h < 3 {
        return;
    }
    let src = snapshot(canvas);
    let lum = |c: (u8, u8, u8)| (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) as f32 / 6.0;
    // flat luminance grid so each pixel is read once
    let mut grid = vec![0f32; w * h];
    for (i, &c) in src.iter().enumerate() {
        grid[i] = lum(c);
    }
    for y in 0..h {
        let yu = y.saturating_sub(1);
        let yd = (y + 1).min(h - 1);
        for x in 0..w {
            let xl = x.saturating_sub(1);
            let xr = (x + 1).min(w - 1);
            let gx = (grid[yu * w + xr] + 2.0 * grid[y * w + xr] + grid[yd * w + xr])
                - (grid[yu * w + xl] + 2.0 * grid[y * w + xl] + grid[yd * w + xl]);
            let gy = (grid[yd * w + xl] + 2.0 * grid[yd * w + x] + grid[yd * w + xr])
                - (grid[yu * w + xl] + 2.0 * grid[yu * w + x] + grid[yu * w + xr]);
            let m = ((gx * gx + gy * gy).sqrt() / (255.0 * 4.0) * gain).clamp(0.0, 1.0);
            canvas.set(
                x as i32,
                y as i32,
                (
                    (NEON.0 as f32 * m) as u8,
                    (NEON.1 as f32 * m) as u8,
                    (NEON.2 as f32 * m) as u8,
                ),
            );
        }
    }
}

/// Thermal: false-color heat map — luminance through the ramp
/// black → deep red → orange → yellow → white, lerped between stops.
pub fn thermal(canvas: &mut Canvas) {
    thermal_with(canvas, 1.0);
}

fn thermal_with(canvas: &mut Canvas, t: f32) {
    const RAMP: &[(u8, u8, u8)] = &[
        (0, 0, 0),
        (90, 8, 4),
        (255, 105, 0),
        (255, 200, 40),
        (255, 255, 255),
    ];
    let (w, h) = (canvas.width(), canvas.height());
    for y in 0..h {
        for x in 0..w {
            let c = canvas.get(x as i32, y as i32).color;
            let l = (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) as f32 / (6.0 * 255.0);
            let seg = l * (RAMP.len() - 1) as f32;
            let i = (seg as usize).min(RAMP.len() - 2);
            let f = seg - i as f32;
            let (a, b) = (RAMP[i], RAMP[i + 1]);
            let heat = (
                (a.0 as f32 + (b.0 as f32 - a.0 as f32) * f).floor(),
                (a.1 as f32 + (b.1 as f32 - a.1 as f32) * f).floor(),
                (a.2 as f32 + (b.2 as f32 - a.2 as f32) * f).floor(),
            );
            canvas.set(x as i32, y as i32, mix_to(c, heat, t));
        }
    }
}

/// Warp: animated horizontal displacement — each row shifts by
/// `round(2.0 * sin(y*0.35 + t*1.8))` pixels, sampled with wraparound.
pub fn warp(canvas: &mut Canvas, t: f32) {
    warp_with(canvas, t, 2.0);
}

fn warp_with(canvas: &mut Canvas, t: f32, amp: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    if w == 0 || h == 0 {
        return;
    }
    let src = snapshot(canvas);
    for y in 0..h {
        let shift = (amp * (y as f32 * 0.35 + t * 1.8).sin()).round() as i32;
        for x in 0..w {
            let xs = (x as i32 - shift).rem_euclid(w as i32) as usize;
            canvas.set(x as i32, y as i32, src[y * w + xs]);
        }
    }
}

pub fn invert(canvas: &mut Canvas) {
    invert_with(canvas, 1.0);
}

fn invert_with(canvas: &mut Canvas, t: f32) {
    canvas.map_colors(|c| {
        mix_to(c, ((255 - c.0) as f32, (255 - c.1) as f32, (255 - c.2) as f32), t)
    });
}

pub fn sepia(canvas: &mut Canvas) {
    sepia_with(canvas, 1.0);
}

fn sepia_with(canvas: &mut Canvas, t: f32) {
    canvas.map_colors(|c| {
        let (rf, gf, bf) = (c.0 as f32, c.1 as f32, c.2 as f32);
        let s = (
            (rf * 0.393 + gf * 0.769 + bf * 0.189).min(255.0).floor(),
            (rf * 0.349 + gf * 0.686 + bf * 0.168).min(255.0).floor(),
            (rf * 0.272 + gf * 0.534 + bf * 0.131).min(255.0).floor(),
        );
        mix_to(c, s, t)
    });
}

/// Build a 256-entry channel lookup table from a per-value function.
///
/// Every per-channel filter here maps a `u8` to a `u8` independently of
/// position, so the whole mapping is 256 values wide. Computing it once per
/// frame instead of per channel per pixel removes the float math (and, for
/// `gamma`, a `powf`) from the hot loop entirely.
fn channel_lut(f: impl Fn(u8) -> u8) -> [u8; 256] {
    let mut lut = [0u8; 256];
    for (v, out) in lut.iter_mut().enumerate() {
        *out = f(v as u8);
    }
    lut
}

pub fn posterize(canvas: &mut Canvas) {
    posterize_with(canvas, 5.0);
}

fn posterize_with(canvas: &mut Canvas, levels: f32) {
    let lut = channel_lut(|v| quantize(v, levels));
    canvas.map_colors(|c| {
        (
            lut[c.0 as usize],
            lut[c.1 as usize],
            lut[c.2 as usize],
        )
    });
}

fn quantize(v: u8, levels: f32) -> u8 {
    let q = (v as f32 / 255.0 * levels).round() / levels * 255.0;
    q.clamp(0.0, 255.0) as u8
}

pub fn gamma(canvas: &mut Canvas) {
    gamma_with(canvas, 1.35);
}

fn gamma_with(canvas: &mut Canvas, g: f32) {
    // 256 powf calls per frame instead of three per pixel
    let lut = channel_lut(|v| gamma_ch(v, g));
    canvas.map_colors(|c| {
        (
            lut[c.0 as usize],
            lut[c.1 as usize],
            lut[c.2 as usize],
        )
    });
}

fn gamma_ch(v: u8, g: f32) -> u8 {
    ((v as f32 / 255.0).powf(g) * 255.0) as u8
}

pub fn sharpen(canvas: &mut Canvas) {
    sharpen_with(canvas, 1.0);
}

fn sharpen_with(canvas: &mut Canvas, gain: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    if w < 3 || h < 3 {
        return;
    }
    let src = snapshot(canvas);
    let lum = |c: (u8, u8, u8)| (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) as f32 / 6.0;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let c = lum(src[y * w + x]);
            let blur = (lum(src[y * w + x - 1])
                + lum(src[y * w + x + 1])
                + lum(src[(y - 1) * w + x])
                + lum(src[(y + 1) * w + x]))
                * 0.25;
            let edge = (c - blur).clamp(-80.0, 80.0) * gain;
            let base = src[y * w + x];
            canvas.set(
                x as i32,
                y as i32,
                (
                    (base.0 as f32 + edge).clamp(0.0, 255.0) as u8,
                    (base.1 as f32 + edge).clamp(0.0, 255.0) as u8,
                    (base.2 as f32 + edge).clamp(0.0, 255.0) as u8,
                ),
            );
        }
    }
}

pub fn mirror(canvas: &mut Canvas) {
    let (w, h) = (canvas.width(), canvas.height());
    if w < 2 {
        return;
    }
    let src = snapshot(canvas);
    for y in 0..h {
        for x in 0..w {
            canvas.set(x as i32, y as i32, src[y * w + (w - 1 - x)]);
        }
    }
}

pub fn noir(canvas: &mut Canvas) {
    noir_with(canvas, 1.6);
}

fn noir_with(canvas: &mut Canvas, k: f32) {
    // luminance depends on all three channels, so this is not a per-channel
    // LUT; the win is dropping the per-pixel bounds-checked get/set round trip
    canvas.map_colors(|c| {
        let l = (c.0 as f32 * 0.299 + c.1 as f32 * 0.587 + c.2 as f32 * 0.114) / 255.0;
        let t = ((l - 0.5) * k + 0.5).clamp(0.0, 1.0);
        let v = (t * 255.0) as u8;
        (v, v, v)
    });
}

/// Letterbox: black bars top and bottom, `frac` of the frame each (0.12 is
/// about 2.35:1 on a 16:9 screen).
pub fn letterbox(canvas: &mut Canvas, frac: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    let bars = (h as f32 * frac).round() as usize;
    if bars == 0 {
        return;
    }
    for y in (0..bars.min(h)).chain(h.saturating_sub(bars)..h) {
        for x in 0..w {
            canvas.set(x as i32, y as i32, (0, 0, 0));
        }
    }
}

/// 4x4 Bayer thresholds, 0..16.
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// One channel through an ordered dither to `levels` steps.
pub fn dither_channel(v: u8, levels: f32, x: i32, y: i32) -> u8 {
    let steps = (levels - 1.0).max(1.0);
    let th = (BAYER4[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f32 + 0.5) / 16.0;
    let q = (v as f32 * steps / 255.0 + th).floor().clamp(0.0, steps);
    (q * 255.0 / steps).round() as u8
}

/// Dither: an ordered (Bayer) dither to a few levels per channel, the look
/// of early 8-bit graphics.
pub fn dither(canvas: &mut Canvas, levels: f32) {
    let w = canvas.width();
    for (i, cell) in canvas.cells_raw_mut().iter_mut().enumerate() {
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        let c = cell.color;
        cell.color = (
            dither_channel(c.0, levels, x, y),
            dither_channel(c.1, levels, x, y),
            dither_channel(c.2, levels, x, y),
        );
        cell.ch = None;
    }
}

/// How blurred a row is in tilt-shift, 0..1: sharp in a band around the
/// middle, blending to fully blurred toward the top and bottom.
pub fn tilt_weight(y: f32, height: f32, strength: f32, band: f32) -> f32 {
    let yn = if height > 1.0 { y / (height - 1.0) } else { 0.5 };
    let d = ((yn - 0.5).abs() - band) / (0.5 - band).max(1e-3);
    let s = d.clamp(0.0, 1.0);
    (s * s * (3.0 - 2.0 * s) * strength).clamp(0.0, 1.0)
}

/// Tilt-shift: a sharp band across the middle, everything above and below
/// blurred (a box blur of `radius`), so the scene reads as a miniature. The
/// blur legs are truncated to u8 between passes, like the GPU's.
pub fn tiltshift(canvas: &mut Canvas, strength: f32, band: f32, radius: usize) {
    let (w, h) = (canvas.width(), canvas.height());
    if w == 0 || h == 0 || strength <= 0.0 {
        return;
    }
    let src = snapshot(canvas);
    let box_blur = |data: &[(u8, u8, u8)], horizontal: bool| {
        let mut out = vec![(0u8, 0u8, 0u8); w * h];
        for y in 0..h {
            for x in 0..w {
                let (mut r, mut g, mut b, mut n) = (0f32, 0f32, 0f32, 0f32);
                let (lo, hi, at) = if horizontal {
                    (x.saturating_sub(radius), (x + radius).min(w - 1), y)
                } else {
                    (y.saturating_sub(radius), (y + radius).min(h - 1), x)
                };
                for k in lo..=hi {
                    let c = if horizontal { data[at * w + k] } else { data[k * w + at] };
                    r += c.0 as f32;
                    g += c.1 as f32;
                    b += c.2 as f32;
                    n += 1.0;
                }
                out[y * w + x] = ((r / n) as u8, (g / n) as u8, (b / n) as u8);
            }
        }
        out
    };
    let blurred = box_blur(&box_blur(&src, true), false);
    for y in 0..h {
        let t = tilt_weight(y as f32, h as f32, strength, band);
        for x in 0..w {
            let c = src[y * w + x];
            let b = blurred[y * w + x];
            let m = |a: u8, z: u8| (a as f32 + (z as f32 - a as f32) * t).floor().clamp(0.0, 255.0) as u8;
            canvas.set(x as i32, y as i32, (m(c.0, b.0), m(c.1, b.1), m(c.2, b.2)));
        }
    }
}

/// Kaleido: the top-left quarter mirrored into the other three, a
/// four-fold symmetric picture.
pub fn kaleido(canvas: &mut Canvas) {
    let (w, h) = (canvas.width(), canvas.height());
    if w < 2 || h < 2 {
        return;
    }
    let src = snapshot(canvas);
    for y in 0..h {
        let sy = if y < h / 2 { y } else { h - 1 - y };
        for x in 0..w {
            let sx = if x < w / 2 { x } else { w - 1 - x };
            canvas.set(x as i32, y as i32, src[sy * w + sx]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, c: (u8, u8, u8)) -> Canvas {
        let mut cv = Canvas::new(w, h);
        cv.clear(c);
        cv
    }

    #[test]
    fn scanlines_darkens_alternate_rows() {
        let mut c = solid(4, 4, (100, 100, 100));
        scanlines(&mut c);
        assert_eq!(c.get(0, 0).color, (100, 100, 100)); // row 0 untouched
        assert_eq!(c.get(0, 1).color, (72, 72, 72)); // row 1 darkened
        assert_eq!(c.get(0, 2).color, (100, 100, 100));
    }

    #[test]
    fn vignette_darkens_corners_not_center() {
        let mut c = solid(20, 20, (200, 200, 200));
        vignette(&mut c);
        assert_eq!(c.get(10, 10).color, (200, 200, 200));
        let corner = c.get(0, 0).color;
        assert!(corner.0 < 200 - 60, "corner should darken: {corner:?}");
    }

    #[test]
    fn grain_is_deterministic_per_frame() {
        let mut a = solid(10, 10, (128, 128, 128));
        let mut b = solid(10, 10, (128, 128, 128));
        grain(&mut a, 1.0);
        grain(&mut b, 1.0);
        for y in 0..10 {
            for x in 0..10 {
                assert_eq!(a.get(x, y).color, b.get(x, y).color);
            }
        }
        // different frame → different grain (almost surely)
        let mut c2 = solid(10, 10, (128, 128, 128));
        grain(&mut c2, 2.0);
        let differs = (0..10).any(|x| a.get(x, 0).color != c2.get(x, 0).color);
        assert!(differs);
    }

    #[test]
    fn warm_cool_shift_channels() {
        let mut c = solid(2, 2, (100, 100, 100));
        warm(&mut c);
        assert!(c.get(0, 0).color.0 > 100 && c.get(0, 0).color.2 < 100);
        let mut c = solid(2, 2, (100, 100, 100));
        cool(&mut c);
        assert!(c.get(0, 0).color.2 > 100 && c.get(0, 0).color.0 < 100);
    }

    #[test]
    fn hue_rotates_red_toward_green() {
        let mut c = solid(1, 1, (255, 0, 0));
        hue(&mut c, 120.0);
        let (r, g, b) = c.get(0, 0).color;
        assert!(g > 200 && r < 60 && b < 60, "expected green, got ({r},{g},{b})");
        // grays are untouched
        let mut c = solid(1, 1, (100, 100, 100));
        hue(&mut c, 120.0);
        assert_eq!(c.get(0, 0).color, (100, 100, 100));
    }

    #[test]
    fn crt_fringes_edges_and_scans() {
        let mut c = solid(40, 4, (100, 100, 100));
        crt(&mut c);
        // center keeps channels aligned, edges pull channels from neighbors
        assert_eq!(c.get(20, 0).color.0, c.get(20, 0).color.2);
        // alternate rows darkened by the scanline pass
        assert!(c.get(20, 1).color.1 < 100);
    }

    #[test]
    fn bloom_brightens_around_hot_pixels_not_dark_ones() {
        let mut c = solid(20, 20, (10, 10, 10));
        c.set(10, 10, (255, 255, 255)); // hot core
        bloom(&mut c);
        // neighbor of the hot pixel picked up spill (255/25 blur * 0.4 ≈ 4)
        assert!(c.get(11, 10).color.0 > 10 + 2, "spill: {:?}", c.get(11, 10).color);
        // far corner stays dark
        assert!(c.get(0, 0).color.0 < 30, "corner: {:?}", c.get(0, 0).color);
        // pure dark canvas is untouched
        let mut d = solid(20, 20, (10, 10, 10));
        bloom(&mut d);
        assert_eq!(d.get(10, 10).color, (10, 10, 10));
    }

    #[test]
    fn duotone_maps_luminance_to_accent() {
        let mut c = solid(2, 1, (0, 0, 0));
        c.set(1, 0, (255, 255, 255));
        duotone(&mut c);
        assert_eq!(c.get(0, 0).color, (0, 0, 0)); // black stays black
        assert_eq!(c.get(1, 0).color, (120, 180, 255)); // white becomes accent
    }

    #[test]
    fn pixelate_averages_blocks() {
        let mut c = solid(3, 3, (0, 0, 0));
        c.set(0, 0, (90, 90, 90)); // one bright pixel in the 3x3 block
        pixelate(&mut c);
        // whole block becomes the average (90/9 = 10)
        assert_eq!(c.get(2, 2).color, (10, 10, 10));
        assert_eq!(c.get(1, 1).color, (10, 10, 10));
    }

    #[test]
    fn chroma_splits_channels_at_edges_only() {
        let mut c = solid(40, 4, (100, 100, 100));
        c.set(2, 0, (200, 100, 100));
        chroma(&mut c);
        assert_eq!(c.get(20, 0).color, (100, 100, 100)); // center untouched
        // left edge pulls its red channel from 2px to the right
        assert_eq!(c.get(0, 0).color.0, 200);
    }

    #[test]
    fn strength_one_is_the_classic_look_bit_for_bit() {
        // every effect at amount 1 must equal its classic function exactly
        let base = {
            let mut c = Canvas::new(40, 30);
            for y in 0..30 {
                for x in 0..40 {
                    c.set(x, y, ((x * 6) as u8, (y * 8) as u8, ((x + y) * 3) as u8));
                }
            }
            c
        };
        let classic: &[(&str, fn(&mut Canvas))] = &[
            ("scanlines", scanlines),
            ("vignette", vignette),
            ("warm", warm),
            ("cool", cool),
            ("crt", crt),
            ("bloom", bloom),
            ("duotone", duotone),
            ("pixelate", pixelate),
            ("chroma", chroma),
            ("edges", edges),
            ("thermal", thermal),
            ("invert", invert),
            ("sepia", sepia),
            ("posterize", posterize),
            ("gamma", gamma),
            ("sharpen", sharpen),
            ("noir", noir),
        ];
        for (name, f) in classic {
            let mut a = base.clone_for_smooth();
            let mut b = base.clone_for_smooth();
            f(&mut a);
            apply_with(name, &mut b, 1.5, 1.0);
            for y in 0..30 {
                for x in 0..40 {
                    assert_eq!(a.get(x, y).color, b.get(x, y).color, "{name} at ({x},{y})");
                }
            }
        }
    }

    #[test]
    fn strength_scales_each_effect() {
        let lit = |name: &str, a: f32| {
            let mut c = solid(20, 20, (200, 120, 60));
            apply_with(name, &mut c, 1.0, a);
            c.get(0, 1).color
        };
        // scanlines: darker rows at higher strength, untouched at 0
        assert!(lit("scanlines", 2.0).0 < lit("scanlines", 1.0).0);
        assert_eq!(lit("scanlines", 0.0), (200, 120, 60));
        // warm pushes red further at higher strength
        assert!(lit("warm", 2.0).0 >= lit("warm", 1.0).0);
        assert!(lit("warm", 2.0).2 < lit("warm", 1.0).2);
        // mixes: half-way sits between the picture and the effect
        let half = lit("invert", 0.5);
        assert!(half.0 > 55 && half.0 < 200, "{half:?}");
        // posterize: fewer levels at higher strength
        assert!(params("posterize", 2.0)[0] < params("posterize", 1.0)[0]);
        assert_eq!(params("posterize", 1.0)[0], 5.0);
        assert_eq!(params("pixelate", 1.0)[0], 3.0);
    }

    #[test]
    fn letterbox_blacks_out_bars() {
        let mut c = solid(10, 50, (200, 200, 200));
        letterbox(&mut c, 0.12);
        assert_eq!(c.get(5, 0).color, (0, 0, 0));
        assert_eq!(c.get(5, 5).color, (0, 0, 0));
        assert_eq!(c.get(5, 6).color, (200, 200, 200));
        assert_eq!(c.get(5, 49).color, (0, 0, 0));
    }

    #[test]
    fn dither_uses_few_levels_and_both_neighbours() {
        let mut c = solid(8, 8, (128, 128, 128));
        dither(&mut c, 4.0);
        let mut seen = std::collections::BTreeSet::new();
        for y in 0..8 {
            for x in 0..8 {
                seen.insert(c.get(x, y).color.0);
            }
        }
        // mid grey between two of the four levels: both appear
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert!(seen.iter().all(|v| [0, 85, 170, 255].contains(v)), "{seen:?}");
    }

    #[test]
    fn tiltshift_keeps_the_middle_sharp_and_blurs_the_edges() {
        // vertical stripes: blurring grays them out
        let mut c = Canvas::new(20, 40);
        for y in 0..40 {
            for x in 0..20 {
                let v = if x % 2 == 0 { 0 } else { 255 };
                c.set(x, y, (v, v, v));
            }
        }
        tiltshift(&mut c, 1.0, 0.18, 3);
        let contrast = |y: i32| (c.get(10, y).color.0 as i32 - c.get(11, y).color.0 as i32).abs();
        assert!(contrast(20) > 200, "middle stays sharp");
        assert!(contrast(0) < 100, "top is blurred");
        assert!(contrast(39) < 100, "bottom is blurred");
    }

    #[test]
    fn kaleido_is_symmetric() {
        let mut c = Canvas::new(10, 8);
        for y in 0..8 {
            for x in 0..10 {
                c.set(x, y, ((x * 20) as u8, (y * 30) as u8, 7));
            }
        }
        kaleido(&mut c);
        for y in 0..8 {
            for x in 0..10 {
                assert_eq!(c.get(x, y).color, c.get(9 - x, y).color);
                assert_eq!(c.get(x, y).color, c.get(x, 7 - y).color);
            }
        }
    }

    #[test]
    fn halation_glows_warm() {
        let mut c = solid(20, 20, (10, 10, 10));
        for y in 9..12 {
            for x in 9..12 {
                c.set(x, y, (255, 255, 255));
            }
        }
        apply_with("halation", &mut c, 0.0, 1.0);
        let n = c.get(13, 10).color;
        assert!(n.0 > n.2 + 2, "warm spill: {n:?}");
    }
}
