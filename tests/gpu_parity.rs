#![cfg(feature = "gpu")]
//! The GPU post chain against its CPU reference, pixel for pixel.
//!
//! Half mode maps every canvas pixel verbatim onto a cell's foreground or
//! background, so reading cells back in half mode is a lossless view of the
//! pixels. Every look must land within one 8-bit step: the CPU truncates to
//! `u8` between stages and the shader stays in `f32` (and may fuse
//! multiply-adds), so exact equality is not the bar, one step is.
//!
//! Needs a GPU: `cargo test --test gpu_parity -- --ignored`.
use rand::{rngs::StdRng, SeedableRng};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use termpaper::canvas::Canvas;
use termpaper::gpu::{FrameCells, Gpu, Plan};
use termpaper::look::{Baked, Look, PaletteMode, Rgb, Tone};
use termpaper::render::{self, Pixels};
use termpaper::scene::{self, Detail, SceneOptions};

/// Every effect the GPU implements. `grain` is absent by design: the CPU
/// draws it from a seeded RNG in raster order, the shader from a positional
/// hash. Same range and determinism, different values.
const EFFECTS: &[&str] = &[
    "scanlines", "vignette", "warm", "cool", "hue", "crt", "bloom", "duotone", "pixelate", "chroma", "spectrum",
    "edges", "thermal", "warp", "invert", "sepia", "posterize", "gamma", "sharpen", "mirror", "noir",
];

fn canvas(w: usize, h: usize) -> Canvas {
    let opts = SceneOptions { theme: None, detail: Detail::Low, text_scale: None, pixels: Default::default() };
    let mut s = scene::create("koi", &opts, StdRng::seed_from_u64(7)).unwrap();
    let mut c = Canvas::new(w, h);
    for _ in 0..40 {
        s.update(1.0 / 60.0, &mut c);
    }
    c
}

fn rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

/// Worst per-channel difference between the CPU and GPU pixels, and the
/// share of pixels that differ at all.
fn compare(gpu: &mut Gpu, canvas: &Canvas, look: &Look) -> (i32, f32) {
    let (cols, rows) = (canvas.width(), canvas.height() / 2);
    let area = Rect::new(0, 0, cols as u16, rows as u16);
    let baked = Baked::new(look.clone());
    let t = 1.5;
    // CPU, exactly as the engine does it
    let mut c = canvas.clone_for_smooth();
    termpaper::engine::finish_look(&mut c, &baked, t);
    let mut buf = Buffer::empty(area);
    render::draw(&c, area, &mut buf, true, Pixels::Half);
    // GPU
    let plan = Plan {
        look: baked.get(),
        lut: baked.lut.as_deref(),
        quick_filter: None,
        t,
        dim: 1.0,
        smooth: 0.0,
        pixels: Pixels::Half,
        cols,
        rows,
        crop: (0, 0),
        virt: None,
        hysteresis: 0,
    };
    gpu.drain();
    let words = gpu.run_blocking(canvas, &plan).expect("GPU frame");
    let cells = FrameCells { words: &words, cols, rows };
    let (mut worst, mut differing) = (0i32, 0usize);
    for row in 0..rows {
        for col in 0..cols {
            let cell = &buf[(col as u16, row as u16)];
            let (_, gf, gb) = cells.get(row * cols + col);
            for (a, b) in [(rgb(cell.fg), gf), (rgb(cell.bg), gb)] {
                let d = (a.0 as i32 - b.0 as i32)
                    .abs()
                    .max((a.1 as i32 - b.1 as i32).abs())
                    .max((a.2 as i32 - b.2 as i32).abs());
                worst = worst.max(d);
                differing += (d > 0) as usize;
            }
        }
    }
    (worst, differing as f32 / (cols * rows * 2) as f32)
}

fn looks() -> Vec<(String, Look, i32)> {
    let mut v: Vec<(String, Look, i32)> = EFFECTS
        .iter()
        .map(|e| (format!("effect {e}"), Look::with_effects(&[e.to_string()]), 1))
        .collect();
    let with = |name: &str, f: &dyn Fn(&mut Look)| {
        let mut l = Look::default();
        f(&mut l);
        (name.to_string(), l, 1)
    };
    // Stages that amplify: a one-step difference going into contrast (or a
    // brightening table) comes out as up to ceil(gain) steps, which is the
    // same slack `color_grade`'s own HSV tests allow.
    let amplified = |(n, l, _): (String, Look, i32)| (n, l, 2);
    v.push(amplified(with("grade hue+sat+contrast", &|l| {
        l.grade.hue = 45.0;
        l.grade.saturation = 1.6;
        l.grade.contrast = 1.3;
    })));
    v.push(with("lut exposure", &|l| l.grade.exposure = 0.8));
    v.push(with("lut temperature+tint", &|l| {
        l.grade.temperature = -0.7;
        l.grade.tint = 0.4;
    }));
    v.push(with("lut vibrance+gamma+fade", &|l| {
        l.grade.vibrance = 0.6;
        l.grade.gamma = 1.4;
        l.grade.fade = 0.12;
    }));
    v.push(with("lut tone wheels", &|l| {
        l.grade.shadows = Tone { hue: 210.0, amount: 0.8 };
        l.grade.midtones = Tone { hue: 120.0, amount: 0.4 };
        l.grade.highlights = Tone { hue: 35.0, amount: 0.7 };
        l.grade.balance = 0.3;
    }));
    let pal = vec![Rgb(0x1a, 0x1b, 0x26), Rgb(0x41, 0x48, 0x68), Rgb(0x7a, 0xa2, 0xf7), Rgb(0xbb, 0x9a, 0xf7), Rgb(0xc0, 0xca, 0xf5)];
    for mode in [PaletteMode::Map, PaletteMode::Tint, PaletteMode::Snap] {
        let p = pal.clone();
        v.push(with(&format!("palette {}", mode.name()), &move |l| {
            l.palette.mode = mode;
            l.palette.colors = p.clone();
            l.palette.strength = 0.8;
        }));
    }
    let p = pal.clone();
    v.push(with("palette snap dithered", &move |l| {
        l.palette.mode = PaletteMode::Snap;
        l.palette.colors = p.clone();
        l.palette.dither = true;
    }));
    // everything at once, in pipeline order
    let p = pal;
    v.push(amplified(with("full stack", &move |l| {
        l.effects.stack = vec!["bloom".into(), "vignette".into()];
        l.grade.saturation = 1.2;
        l.grade.exposure = 0.3;
        l.grade.shadows = Tone { hue: 230.0, amount: 0.5 };
        l.palette.mode = PaletteMode::Map;
        l.palette.colors = p.clone();
        l.palette.strength = 0.5;
    })));
    v
}

#[test]
#[ignore = "requires a GPU; run with -- --ignored"]
fn every_look_matches_the_cpu_within_one_step() {
    let (cols, rows) = (272usize, 33usize);
    let c = canvas(cols, rows * 2);
    let mut gpu = Gpu::new(cols, rows * 2, cols * rows).expect("a GPU adapter");
    let mut failures = Vec::new();
    for (name, look, allowed) in looks() {
        let (worst, share) = compare(&mut gpu, &c, &look);
        eprintln!("{name:<28} worst Δ {worst:>3}   differing {:>6.2}%", share * 100.0);
        if worst > allowed {
            failures.push(format!("{name}: worst Δ {worst} > {allowed}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
