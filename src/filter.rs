//! Composable post-processing filters applied to the finished canvas
//! before blitting. Ordered, cheap, pure functions (grain takes a seed).

use crate::canvas::{hsv, Canvas};
use rand::{rngs::StdRng, RngExt, SeedableRng};

/// Available filter names, in quick-cycle order.
#[allow(dead_code)] // used by quick-cycle keybind
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
];

/// Apply one named filter. `t` is seconds (animated grain); unknown names
/// are ignored.
pub fn apply(name: &str, canvas: &mut Canvas, t: f32) {
    match name {
        "scanlines" => scanlines(canvas),
        "vignette" => vignette(canvas),
        "grain" => grain(canvas, t),
        "warm" => warm(canvas),
        "cool" => cool(canvas),
        "hue" => hue(canvas, 120.0),
        "crt" => crt(canvas),
        "bloom" => bloom(canvas),
        "duotone" => duotone(canvas),
        "pixelate" => pixelate(canvas),
        "chroma" => chroma(canvas),
        "spectrum" => spectrum(canvas, t),
        "edges" => edges(canvas),
        "thermal" => thermal(canvas),
        "warp" => warp(canvas, t),
        "invert" => invert(canvas),
        "sepia" => sepia(canvas),
        "posterize" => posterize(canvas),
        "gamma" => gamma(canvas),
        "sharpen" => sharpen(canvas),
        "mirror" => mirror(canvas),
        "noir" => noir(canvas),
        _ => {}
    }
}

pub fn apply_all(names: &[String], canvas: &mut Canvas, t: f32) {
    for n in names {
        apply(n, canvas, t);
    }
}

/// Apply a Look's effect stack in order.
pub fn apply_stack(effects: &crate::look::Effects, canvas: &mut Canvas, t: f32) {
    for n in &effects.stack {
        apply(n, canvas, t);
    }
}

/// Effects that read neighbouring pixels (blurs, offsets, kernels): on a
/// wall they need the scene rendered a little past the pane's edge.
pub fn reads_neighbours(name: &str) -> bool {
    matches!(name, "bloom" | "crt" | "chroma" | "pixelate" | "edges" | "warp" | "sharpen")
}

fn scale_cell(c: &mut (u8, u8, u8), f: f32) {
    c.0 = (c.0 as f32 * f).clamp(0.0, 255.0) as u8;
    c.1 = (c.1 as f32 * f).clamp(0.0, 255.0) as u8;
    c.2 = (c.2 as f32 * f).clamp(0.0, 255.0) as u8;
}

/// Darken every other canvas pixel row.
pub fn scanlines(canvas: &mut Canvas) {
    let (w, h) = (canvas.width(), canvas.height());
    for y in (1..h).step_by(2) {
        for x in 0..w {
            let mut c = canvas.get(x as i32, y as i32).color;
            scale_cell(&mut c, 0.72);
            canvas.set(x as i32, y as i32, c);
        }
    }
}

/// Radial edge darkening.
pub fn vignette(canvas: &mut Canvas) {
    let (w, h) = (canvas.width(), canvas.height());
    if w == 0 || h == 0 {
        return;
    }
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let max_d = (cx * cx + cy * cy).sqrt();
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 - cx) / cx;
            let dy = (y as f32 - cy) / cy;
            let d = ((dx * dx + dy * dy).sqrt() / std::f32::consts::SQRT_2).min(1.0);
            let f = 1.0 - d * d * 0.45;
            let _ = max_d;
            let mut c = canvas.get(x as i32, y as i32).color;
            scale_cell(&mut c, f);
            canvas.set(x as i32, y as i32, c);
        }
    }
}

/// Animated film grain, deterministic for a given frame index.
pub fn grain(canvas: &mut Canvas, t: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    // seed per frame tick so the grain animates but stays reproducible
    let mut rng = StdRng::seed_from_u64((t * 30.0) as u64);
    for y in 0..h {
        for x in 0..w {
            let n = rng.random_range(-14i32..=14);
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
    let (w, h) = (canvas.width(), canvas.height());
    // chromatic fringe: shift red left, blue right, growing toward edges
    let mut snapshot = vec![(0u8, 0u8, 0u8); w * h];
    for y in 0..h {
        for x in 0..w {
            snapshot[y * w + x] = canvas.get(x as i32, y as i32).color;
        }
    }
    let (cx, _) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        for x in 0..w {
            let off = ((x as f32 - cx) / cx * 2.0) as i32;
            let xr = (x as i32 - off).clamp(0, w as i32 - 1) as usize;
            let xb = (x as i32 + off).clamp(0, w as i32 - 1) as usize;
            let mid = snapshot[y * w + x];
            let c = (
                snapshot[y * w + xr].0,
                mid.1,
                snapshot[y * w + xb].2,
            );
            canvas.set(x as i32, y as i32, c);
        }
    }
    scanlines(canvas);
    vignette(canvas);
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
    const THRESHOLD: u32 = 180;
    const RADIUS: usize = 2;
    const STRENGTH: f32 = 0.4;
    let (w, h) = (canvas.width(), canvas.height());
    if w < RADIUS * 2 + 1 || h < RADIUS * 2 + 1 {
        return;
    }
    let src = snapshot(canvas);
    let lum = |c: (u8, u8, u8)| (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) / 6;
    // bright-pass into a float buffer
    let mut buf = vec![(0f32, 0f32, 0f32); w * h];
    for (i, &c) in src.iter().enumerate() {
        if lum(c) > THRESHOLD {
            buf[i] = (c.0 as f32, c.1 as f32, c.2 as f32);
        }
    }
    // horizontal box blur
    let mut tmp = vec![(0f32, 0f32, 0f32); w * h];
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut n) = (0f32, 0f32, 0f32, 0f32);
            let x0 = x.saturating_sub(RADIUS);
            let x1 = (x + RADIUS).min(w - 1);
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
            let y0 = y.saturating_sub(RADIUS);
            let y1 = (y + RADIUS).min(h - 1);
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
                    (c.0 as f32 + r / n * STRENGTH).min(255.0) as u8,
                    (c.1 as f32 + g / n * STRENGTH).min(255.0) as u8,
                    (c.2 as f32 + b / n * STRENGTH).min(255.0) as u8,
                ),
            );
        }
    }
}

/// Duotone: luminance mapped onto a black → accent gradient.
pub fn duotone(canvas: &mut Canvas) {
    const ACCENT: (u8, u8, u8) = (120, 180, 255);
    canvas.map_colors(|c| {
        let l = (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) as f32 / (6.0 * 255.0);
        (
            (ACCENT.0 as f32 * l) as u8,
            (ACCENT.1 as f32 * l) as u8,
            (ACCENT.2 as f32 * l) as u8,
        )
    });
}

/// Pixelate: 3x3 mosaic, each block becomes its average color.
pub fn pixelate(canvas: &mut Canvas) {
    const BLOCK: usize = 3;
    let (w, h) = (canvas.width(), canvas.height());
    if w < BLOCK || h < BLOCK {
        return;
    }
    let src = snapshot(canvas);
    for by in (0..h).step_by(BLOCK) {
        for bx in (0..w).step_by(BLOCK) {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for y in by..(by + BLOCK).min(h) {
                for x in bx..(bx + BLOCK).min(w) {
                    let c = src[y * w + x];
                    r += c.0 as u32;
                    g += c.1 as u32;
                    b += c.2 as u32;
                    n += 1;
                }
            }
            let avg = ((r / n) as u8, (g / n) as u8, (b / n) as u8);
            for y in by..(by + BLOCK).min(h) {
                for x in bx..(bx + BLOCK).min(w) {
                    canvas.set(x as i32, y as i32, avg);
                }
            }
        }
    }
}

/// Chroma: chromatic aberration only (the crt fringe, without the scanlines
/// and vignette) — red shifts left, blue right, growing toward the edges.
pub fn chroma(canvas: &mut Canvas) {
    let (w, h) = (canvas.width(), canvas.height());
    if w < 4 {
        return;
    }
    let src = snapshot(canvas);
    let cx = w as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let off = ((x as f32 - cx) / cx * 2.0) as i32;
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
            let m = ((gx * gx + gy * gy).sqrt() / (255.0 * 4.0)).clamp(0.0, 1.0);
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
            canvas.set(
                x as i32,
                y as i32,
                (
                    (a.0 as f32 + (b.0 as f32 - a.0 as f32) * f) as u8,
                    (a.1 as f32 + (b.1 as f32 - a.1 as f32) * f) as u8,
                    (a.2 as f32 + (b.2 as f32 - a.2 as f32) * f) as u8,
                ),
            );
        }
    }
}

/// Warp: animated horizontal displacement — each row shifts by
/// `round(2.0 * sin(y*0.35 + t*1.8))` pixels, sampled with wraparound.
pub fn warp(canvas: &mut Canvas, t: f32) {
    let (w, h) = (canvas.width(), canvas.height());
    if w == 0 || h == 0 {
        return;
    }
    let src = snapshot(canvas);
    for y in 0..h {
        let shift = (2.0 * (y as f32 * 0.35 + t * 1.8).sin()).round() as i32;
        for x in 0..w {
            let xs = (x as i32 - shift).rem_euclid(w as i32) as usize;
            canvas.set(x as i32, y as i32, src[y * w + xs]);
        }
    }
}

pub fn invert(canvas: &mut Canvas) {
    canvas.map_colors(|c| (255 - c.0, 255 - c.1, 255 - c.2));
}

pub fn sepia(canvas: &mut Canvas) {
    canvas.map_colors(|(r, g, b)| {
        let (rf, gf, bf) = (r as f32, g as f32, b as f32);
        (
            (rf * 0.393 + gf * 0.769 + bf * 0.189).min(255.0) as u8,
            (rf * 0.349 + gf * 0.686 + bf * 0.168).min(255.0) as u8,
            (rf * 0.272 + gf * 0.534 + bf * 0.131).min(255.0) as u8,
        )
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
    const LEVELS: f32 = 5.0;
    let lut = channel_lut(|v| quantize(v, LEVELS));
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
    const G: f32 = 1.35;
    // 256 powf calls per frame instead of three per pixel
    let lut = channel_lut(|v| gamma_ch(v, G));
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
            let edge = (c - blur).clamp(-80.0, 80.0);
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
    // luminance depends on all three channels, so this is not a per-channel
    // LUT; the win is dropping the per-pixel bounds-checked get/set round trip
    canvas.map_colors(|c| {
        let l = (c.0 as f32 * 0.299 + c.1 as f32 * 0.587 + c.2 as f32 * 0.114) / 255.0;
        let t = ((l - 0.5) * 1.6 + 0.5).clamp(0.0, 1.0);
        let v = (t * 255.0) as u8;
        (v, v, v)
    });
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
}
