//! GPU backend: parity against the CPU pipeline, then a head-to-head bench.
//!
//! Parity runs in two stages, because the two things that can go wrong are
//! different and the second one masks the first.
//!
//! **Stage 1 — pixel parity.** Half mode maps each canvas pixel verbatim onto a
//! cell's foreground or background, so reading cells back in half mode is a
//! lossless view of the pixels themselves. Every filter must land within one
//! 8-bit step there. That is the real correctness gate: the CPU truncates to
//! `u8` between stages while the shader stays in `f32` until the final pack, and
//! RADV contracts multiply-adds, so exact equality is not achievable — but one
//! step is, and anything worse is a genuine bug.
//!
//! **Stage 2 — glyph agreement.** Quad and braille reduce a 2x2 or 2x4 block to
//! one glyph by thresholding each pixel's luminance against the block average. A
//! pixel sitting within a hair of that average can fall either side given the
//! one-step slack from stage 1, which changes the glyph and both colour means.
//! Those flips are expected and invisible — the two pixels being reordered were
//! near-identical. What must not happen is a flip of a pixel that was nowhere
//! near the threshold, so stage 2 reconstructs both masks and, for every bit
//! that differs, checks how far that pixel actually sat from the average.
//!
//! An earlier version of this test compared cell colours directly and reported
//! quad/braille "failures" for six filters. That was the metric's fault, not the
//! shader's: cell colours in those modes are weighted means over a block, so one
//! flipped mask bit moves them by far more than one step.
//!
//! Run with:  cargo run --release --features gpu --example gpu_bench

use rand::{rngs::StdRng, SeedableRng};
use ratatui::{buffer::Buffer, layout::Rect};
use std::time::Instant;
use termpaper::canvas::Canvas;
use termpaper::gpu::{FrameCells, Gpu, Plan};
use termpaper::render::{self, Pixels};
use termpaper::look::{Baked, Look};
use termpaper::scene::{self, Detail, SceneOptions};
use termpaper::{color_grade, filter};

/// Every filter the GPU implements. `grain` is absent by design: the CPU seeds
/// an RNG per frame and pulls samples in raster order, a sequential dependency
/// the shader replaces with a positional hash. Same range, same per-frame
/// determinism, different values — so there is nothing to compare.
const FILTERS: &[&str] = &[
    "scanlines",
    "vignette",
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

/// A one-step pixel difference moves a luminance `3r + 6g + b` by at most 10,
/// and moves the block average by at most 10 as well. A mask bit can therefore
/// only flip if its pixel sat within 20 of the average. Beyond that the shader
/// and the CPU genuinely disagree about the split.
const MAX_FLIP_MARGIN: u32 = 20;

fn make_canvas(w: usize, h: usize) -> Canvas {
    let opts = SceneOptions {
        theme: None,
        detail: Detail::Low,
        text_scale: None,
        pixels: Default::default(),
    };
    let mut s = scene::create("koi", &opts, StdRng::seed_from_u64(7)).unwrap();
    let mut c = Canvas::new(w, h);
    for _ in 0..40 {
        s.update(1.0 / 60.0, &mut c);
    }
    c
}

fn rgb_of(c: ratatui::style::Color) -> (u8, u8, u8) {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

fn base_plan<'a>(look: &'a Look, pixels: Pixels, cols: usize, rows: usize) -> Plan<'a> {
    Plan {
        look,
        lut: None,
        quick_filter: None,
        t: 1.5,
        dim: 1.0,
        smooth: 0.0,
        pixels,
        cols,
        rows,
        crop: (0, 0),
        virt: None,
        hysteresis: 0,
    }
}

/// Run the CPU pipeline exactly as `main.rs` does, and hand back both the
/// terminal cells and the post-processed canvas behind them.
fn cpu_run(canvas: &Canvas, plan: &Plan, area: Rect) -> (Buffer, Canvas) {
    let mut c = canvas.clone_for_smooth();
    termpaper::engine::finish_look(&mut c, &Baked::new(plan.look.clone()), plan.t);
    if let Some(q) = plan.quick_filter {
        filter::apply(q, &mut c, plan.t);
    }
    termpaper::canvas::dim(&mut c, plan.dim);
    let mut buf = Buffer::empty(area);
    render::draw(&c, area, &mut buf, true, plan.pixels);
    (buf, c)
}

// ------------------------------------------------------------------- stage 1

/// Largest per-channel difference between the CPU and GPU cells over a half
/// mode render — which is to say, over the pixels themselves.
fn pixel_parity(gpu: &mut Gpu, canvas: &Canvas, filters: &[String]) -> i32 {
    let (cols, rows) = (canvas.width(), canvas.height() / 2);
    let area = Rect::new(0, 0, cols as u16, rows as u16);
    let look = Look::with_effects(filters);
    let plan = base_plan(&look, Pixels::Half, cols, rows);
    let (buf, _) = cpu_run(canvas, &plan, area);
    gpu.drain();
    let Some(words) = gpu.run_blocking(canvas, &plan) else {
        return i32::MAX;
    };
    let cells = FrameCells {
        words: &words,
        cols,
        rows,
    };
    let mut worst = 0i32;
    for row in 0..rows {
        for col in 0..cols {
            let cell = &buf[(col as u16, row as u16)];
            let (cf, cb) = (rgb_of(cell.fg), rgb_of(cell.bg));
            let (_, gf, gb) = cells.get(row * cols + col);
            for (a, b) in [
                (cf.0, gf.0),
                (cf.1, gf.1),
                (cf.2, gf.2),
                (cb.0, gb.0),
                (cb.1, gb.1),
                (cb.2, gb.2),
            ] {
                worst = worst.max((a as i32 - b as i32).abs());
            }
        }
    }
    worst
}

// ------------------------------------------------------------------- stage 2

/// Invert `render::quad_glyph` / `render::braille_glyph` so a rendered cell can
/// be turned back into the mask that produced it.
fn mask_of_glyph(cp: u32, pixels: Pixels) -> Option<u8> {
    let ch = char::from_u32(cp)?;
    match pixels {
        Pixels::Quad => (0u8..16).find(|&m| render::quad_glyph(m) == ch),
        Pixels::Braille => (0u8..=255).find(|&m| render::braille_glyph(m) == ch),
        Pixels::Half => None,
    }
}

/// The block's per-pixel luminances and their average, in `render::split`'s own
/// integer arithmetic and its own bit order.
fn block_lums(c: &Canvas, ox: usize, oy: usize, pixels: Pixels) -> (Vec<u32>, u32) {
    let lum = |p: (u8, u8, u8)| p.0 as u32 * 3 + p.1 as u32 * 6 + p.2 as u32;
    let mut lums = Vec::with_capacity(8);
    match pixels {
        // bit order TL, TR, BL, BR
        Pixels::Quad => {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                lums.push(lum(c.get((ox + dx) as i32, (oy + dy) as i32).color));
            }
        }
        // left column top-to-bottom, then right column
        Pixels::Braille => {
            for dx in 0..2 {
                for dy in 0..4 {
                    lums.push(lum(c.get((ox + dx) as i32, (oy + dy) as i32).color));
                }
            }
        }
        Pixels::Half => {}
    }
    let avg = lums.iter().sum::<u32>() / lums.len().max(1) as u32;
    (lums, avg)
}

struct GlyphReport {
    flips: usize,
    cells: usize,
    /// How far from the block average the *flipped* pixels actually sat.
    worst_flip_margin: u32,
    unexplained: usize,
}

fn glyph_parity(gpu: &mut Gpu, canvas: &Canvas, plan: &Plan) -> GlyphReport {
    let area = Rect::new(0, 0, plan.cols as u16, plan.rows as u16);
    let (buf, cpu_canvas) = cpu_run(canvas, plan, area);
    gpu.drain();
    let words = gpu.run_blocking(canvas, plan).unwrap();
    let cells = FrameCells {
        words: &words,
        cols: plan.cols,
        rows: plan.rows,
    };
    let (pw, ph) = plan.pixels.cell_size();
    let mut r = GlyphReport {
        flips: 0,
        cells: plan.cols * plan.rows,
        worst_flip_margin: 0,
        unexplained: 0,
    };
    for row in 0..plan.rows {
        for col in 0..plan.cols {
            let cpu_cp = buf[(col as u16, row as u16)]
                .symbol()
                .chars()
                .next()
                .map(u32::from)
                .unwrap_or(32);
            let (gpu_cp, _, _) = cells.get(row * plan.cols + col);
            if gpu_cp == cpu_cp {
                continue;
            }
            r.flips += 1;
            let (Some(cm), Some(gm)) = (
                mask_of_glyph(cpu_cp, plan.pixels),
                mask_of_glyph(gpu_cp, plan.pixels),
            ) else {
                r.unexplained += 1;
                continue;
            };
            let (lums, avg) = block_lums(&cpu_canvas, col * pw, row * ph, plan.pixels);
            let mut explained = true;
            for (i, l) in lums.iter().enumerate() {
                if (cm ^ gm) & (1 << i) == 0 {
                    continue;
                }
                let margin = l.abs_diff(avg);
                r.worst_flip_margin = r.worst_flip_margin.max(margin);
                if margin > MAX_FLIP_MARGIN {
                    explained = false;
                }
            }
            if !explained {
                r.unexplained += 1;
            }
        }
    }
    r
}

fn bench(label: &str, iters: usize, mut f: impl FnMut()) -> f32 {
    f();
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    let ms = t.elapsed().as_secs_f32() * 1000.0 / iters as f32;
    println!("  {label:<40} {ms:>7.3}ms");
    ms
}

fn main() {
    let (cols, rows) = (272usize, 33usize);
    let Some(mut gpu) = Gpu::new(1, 1, 1) else {
        eprintln!("no Vulkan adapter — nothing to test");
        std::process::exit(1);
    };
    println!("adapter: {}\n", gpu.adapter_name());
    let mut failures = 0usize;

    let modes = [Pixels::Half, Pixels::Quad, Pixels::Braille];
    let sizes: Vec<(usize, usize)> = modes
        .iter()
        .map(|p| {
            let (pw, ph) = p.cell_size();
            (cols * pw, rows * ph)
        })
        .collect();

    // -------------------------------------------------- stage 1: pixel parity
    println!("=== stage 1 · pixel parity · worst per-channel Δ (must be <= 1) ===");
    println!(
        "  {:<12} {:>9} {:>9} {:>9}",
        "filter", "272x66", "544x66", "544x132"
    );
    let canvases: Vec<Canvas> = sizes.iter().map(|&(w, h)| make_canvas(w, h)).collect();
    for name in FILTERS {
        let filters = vec![name.to_string()];
        let mut worst = Vec::new();
        for (canvas, &(w, h)) in canvases.iter().zip(&sizes) {
            gpu.resize(w, h, w * (h / 2));
            worst.push(pixel_parity(&mut gpu, canvas, &filters));
        }
        let ok = worst.iter().all(|&d| d <= 1);
        if !ok {
            failures += 1;
        }
        println!(
            "  {:<12} {:>9} {:>9} {:>9}   {}",
            name,
            worst[0],
            worst[1],
            worst[2],
            if ok { "" } else { "<- FAIL" }
        );
    }
    println!();

    // ------------------------------------------------ stage 2: glyph agreement
    println!("=== stage 2 · glyph agreement · flips must be threshold ties ===");
    for (pixels, (canvas, &(w, h))) in modes[1..].iter().zip(canvases[1..].iter().zip(&sizes[1..]))
    {
        println!("  --- {:?} · {w}x{h}px ---", pixels);
        gpu.resize(w, h, cols * rows);
        for name in FILTERS {
            let look = Look::with_effects(&[name.to_string()]);
            let plan = base_plan(&look, *pixels, cols, rows);
            let r = glyph_parity(&mut gpu, canvas, &plan);
            let ok = r.unexplained == 0;
            if !ok {
                failures += 1;
            }
            println!(
                "  {:<12} flips {:>5} ({:>5.2}%)  worst flip margin {:>3}/{}  unexplained {:>4} {}",
                name,
                r.flips,
                r.flips as f32 / r.cells as f32 * 100.0,
                r.worst_flip_margin,
                MAX_FLIP_MARGIN,
                r.unexplained,
                if ok { "" } else { "<- FAIL" }
            );
        }
        println!();
    }

    // ------------------------------------------------------------- benchmark
    let filters: Vec<String> = ["noir", "gamma", "posterize"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for (pixels, (canvas, &(w, h))) in modes.iter().zip(canvases.iter().zip(&sizes)) {
        println!("=== bench · {:?} · {w}x{h}px ({} px) ===", pixels, w * h);
        gpu.resize(w, h, cols * rows);
        gpu.drain();
        let area = Rect::new(0, 0, cols as u16, rows as u16);
        let mut look = Look::with_effects(&filters);
        look.grade.saturation = 2.5;
        look.grade.contrast = 2.5;
        let mut plan = base_plan(&look, *pixels, cols, rows);
        plan.smooth = 0.6;

        let mut work = canvas.clone_for_smooth();
        let mut prev = canvas.clone_for_smooth();
        let mut buf = Buffer::empty(area);
        let cpu_ms = bench("cpu: filters+grade+smooth+draw", 200, || {
            canvas.snapshot_into(&mut work);
            filter::apply_all(&filters, &mut work, 1.5);
            color_grade::apply(&mut work, 0.0, 2.5, 2.5);
            work.smooth_blend(&prev, 0.4);
            work.snapshot_into(&mut prev);
            render::draw(&work, area, &mut buf, true, *pixels);
        });

        let gpu_block_ms = bench("gpu: same, blocking readback", 200, || {
            gpu.run_blocking(canvas, &plan).unwrap();
        });

        // The pipelined path: submit and poll without ever fencing. This is what
        // the frame loop does, and the number that matters.
        gpu.drain();
        let mut landed = 0usize;
        let gpu_pipe_ms = bench("gpu: pipelined submit+poll", 200, || {
            gpu.submit(canvas, &plan);
            if gpu.poll_cells(cols, rows).is_some() {
                landed += 1;
            }
        });
        gpu.drain();

        println!(
            "  → blocking {:.2}x, pipelined {:.2}x vs CPU   ({landed}/200 frames landed)\n",
            cpu_ms / gpu_block_ms,
            cpu_ms / gpu_pipe_ms,
        );
    }

    if failures > 0 {
        eprintln!("{failures} parity check(s) failed");
        std::process::exit(1);
    }
    println!("all parity checks passed");
}
