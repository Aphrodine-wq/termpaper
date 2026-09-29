//! Studio shader scenes on a real GPU. Ignored by default (CI has no Vulkan
//! device): `cargo test --release --test gpu_shader_scenes -- --ignored`.
#![cfg(feature = "gpu")]
use std::time::{Duration, Instant};
use termpaper::engine::{Renderer, Request, SimKey, ViewKey, Worker, DEFAULT_CELL_ASPECT, DEFAULT_GPU_BUDGET_MS};
use termpaper::gpu::{self, Gpu};
use termpaper::render::Pixels;
use termpaper::scene::{shader, Detail, SceneOptions};

fn desc(size: (usize, usize), win: (usize, usize), window: (usize, usize), theme: u32, ms: u64) -> gpu::FrameDesc {
    gpu::FrameDesc {
        view: gpu::ShaderView::for_canvas(size, win, 1.0),
        window,
        time: gpu::shader_time(ms, 1.0),
        speed: 1.0,
        seed: 7,
        theme,
        detail: Detail::Medium,
        spp: 1,
        mirror: false,
        kaleido: false,
        exposure: 0.0,
    }
}

fn render(g: &mut Gpu, spec: &shader::ShaderSpec, d: &gpu::FrameDesc) -> Vec<u32> {
    g.render_shader_pixels(spec.name, &shader::compose(spec), &gpu::uniforms(d))
        .unwrap_or_else(|e| panic!("{}: {e}", spec.name))
        .data
}

fn luma(v: u32) -> f32 {
    let (r, g, b) = ((v & 0xff) as f32, ((v >> 8) & 0xff) as f32, ((v >> 16) & 0xff) as f32);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn mean_abs_diff(a: &[u32], b: &[u32]) -> f32 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        for sh in [0, 8, 16] {
            s += (((x >> sh) & 0xff) as f32 - ((y >> sh) & 0xff) as f32).abs();
        }
    }
    s / (a.len() * 3) as f32
}

#[test]
#[ignore]
fn every_scene_renders_animates_and_honours_its_themes() {
    let mut g = Gpu::new(1, 1, 1).expect("Vulkan GPU");
    let size = (192, 96);
    for spec in shader::SHADER_SCENES {
        let base = render(&mut g, spec, &desc(size, (0, 0), size, 0, 10_000));
        let mean = base.iter().map(|&v| luma(v)).sum::<f32>() / base.len() as f32;
        assert!(mean > 4.0, "{}: frame is black (mean luma {mean})", spec.name);
        let mut colors: Vec<u32> = base.clone();
        colors.sort_unstable();
        colors.dedup();
        assert!(colors.len() > 64, "{}: only {} distinct colours", spec.name, colors.len());
        let later = render(&mut g, spec, &desc(size, (0, 0), size, 0, 12_000));
        assert!(mean_abs_diff(&base, &later) > 0.02, "{}: nothing moves in 2 s", spec.name);
        for t in 1..spec.themes.len() as u32 {
            let other = render(&mut g, spec, &desc(size, (0, 0), size, t, 10_000));
            assert!(
                mean_abs_diff(&base, &other) > 1.0,
                "{}: theme '{}' looks like '{}'",
                spec.name,
                spec.themes[t as usize],
                spec.themes[0]
            );
        }
        for shape in [(54, 96), (340, 96)] {
            let px = render(&mut g, spec, &desc(shape, (0, 0), shape, 0, 10_000));
            let m = px.iter().map(|&v| luma(v)).sum::<f32>() / px.len() as f32;
            assert!(m > 4.0, "{}: {}x{} frame is black", spec.name, shape.0, shape.1);
        }
    }
}

/// A wall is panes each rendering a crop of one frame. Two crops side by side
/// must reproduce the full render, or seams would show between monitors.
#[test]
#[ignore]
fn crops_stitch_into_the_full_frame() {
    let mut g = Gpu::new(1, 1, 1).expect("Vulkan GPU");
    let size = (160, 72);
    for spec in shader::SHADER_SCENES {
        let full = render(&mut g, spec, &desc(size, (0, 0), size, 0, 30_000));
        let left = render(&mut g, spec, &desc(size, (0, 0), (80, 72), 0, 30_000));
        let right = render(&mut g, spec, &desc(size, (80, 0), (80, 72), 0, 30_000));
        let mut stitched = vec![0u32; full.len()];
        for y in 0..72 {
            stitched[y * 160..y * 160 + 80].copy_from_slice(&left[y * 80..y * 80 + 80]);
            stitched[y * 160 + 80..y * 160 + 160].copy_from_slice(&right[y * 80..y * 80 + 80]);
        }
        let d = mean_abs_diff(&full, &stitched);
        assert!(d < 0.6, "{}: crops differ from the full frame by {d} levels", spec.name);
    }
}

/// Pack one pane's cells (half mode) through the real post chain, the way the
/// engine does for a wall: window plus apron, wall coordinates in `virt` —
/// or, with `seamless = false`, the old way (no apron, the pane as its own
/// canvas) for comparison.
fn pane_cells(
    g: &mut Gpu,
    spec: &'static shader::ShaderSpec,
    canvas: (usize, usize),
    crop: (usize, usize),
    grid: (usize, usize),
    filters: &[String],
    seamless: bool,
) -> Vec<u32> {
    let apron = if seamless { 8 } else { 0 };
    let window = (grid.0 + 2 * apron, grid.1 * 2 + 2 * apron);
    let origin = (crop.0 as i64 - apron as i64, crop.1 as i64 - apron as i64);
    g.resize(window.0, window.1, grid.0 * grid.1);
    let start = Instant::now();
    while g.shader_status(spec) != gpu::ShaderStatus::Ready {
        assert!(start.elapsed() < Duration::from_secs(60), "{} never compiled", spec.name);
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut d = desc(canvas, (0, 0), window, 0, 30_000);
    d.view = gpu::ShaderView::for_window(canvas, origin, 1.0);
    g.shader_frame(spec, gpu::uniforms(&d));
    let look = termpaper::look::Look::with_effects(filters);
    let plan = gpu::Plan {
        look: &look,
        lut: None,
        quick_filter: None,
        t: 30.0,
        dim: 1.0, mask: None,
        smooth: 0.0,
        pixels: Pixels::Half,
        cols: grid.0,
        rows: grid.1,
        crop: (apron, apron),
        virt: seamless.then_some([origin.0 as i32, origin.1 as i32, canvas.0 as i32, canvas.1 as i32]),
        hysteresis: 0,
    };
    g.run_blocking(&termpaper::canvas::Canvas::new(1, 1), &plan).expect("frame")
}

/// Two panes of a wall, post-processed with filters that depend on position
/// (vignette, scanlines, grain) or read neighbours (bloom, edges, pixelate,
/// warp): stitched, they must match the whole wall processed at once.
#[test]
#[ignore]
fn post_filters_stay_seamless_across_panes() {
    let Some(spec) = shader::SHADER_SCENES.first() else { return };
    let mut g = Gpu::new(1, 1, 1).expect("Vulkan GPU");
    let canvas = (160, 72);
    let (cols, rows) = (160, 36);
    let filters: Vec<String> =
        ["vignette", "scanlines", "grain", "bloom", "pixelate", "warp"].iter().map(|s| s.to_string()).collect();
    let colours = |cells: &[u32]| -> Vec<u32> { cells.chunks(3).flat_map(|c| [c[1], c[2]]).collect() };
    let full = colours(&pane_cells(&mut g, spec, canvas, (0, 0), (cols, rows), &filters, true));
    let stitch = |seamless: bool, g: &mut Gpu| {
        let left = pane_cells(g, spec, canvas, (0, 0), (cols / 2, rows), &filters, seamless);
        let right = pane_cells(g, spec, canvas, (cols / 2, 0), (cols / 2, rows), &filters, seamless);
        let mut cells = Vec::with_capacity(cols * rows * 3);
        for r in 0..rows {
            let w = cols / 2 * 3;
            cells.extend_from_slice(&left[r * w..r * w + w]);
            cells.extend_from_slice(&right[r * w..r * w + w]);
        }
        colours(&cells)
    };
    let seamless = mean_abs_diff(&full, &stitch(true, &mut g));
    let naive = mean_abs_diff(&full, &stitch(false, &mut g));
    eprintln!("stitched panes vs whole wall: {seamless:.3} levels (without apron/virt: {naive:.3})");
    assert!(seamless < 0.6, "panes differ from the whole wall by {seamless} levels");
    assert!(naive > seamless * 4.0 + 1.0, "the comparison should show the old seams: naive {naive} vs {seamless}");
}

/// With hysteresis on, a frame identical to the last re-emits it exactly,
/// and cells that moved by less than the threshold keep their old values.
#[test]
#[ignore]
fn hysteresis_holds_small_changes_and_passes_big_ones() {
    let Some(spec) = shader::SHADER_SCENES.first() else { return };
    let mut g = Gpu::new(1, 1, 1).expect("Vulkan GPU");
    let size = (96, 48);
    let grid = (96, 24);
    g.resize(size.0, size.1, grid.0 * grid.1);
    while g.shader_status(spec) != gpu::ShaderStatus::Ready {
        std::thread::sleep(Duration::from_millis(5));
    }
    let frame = |g: &mut Gpu, ms: u64, dim: f32, hysteresis: u8| {
        g.shader_frame(spec, gpu::uniforms(&desc(size, (0, 0), size, 0, ms)));
        let plan = gpu::Plan {
            look: &termpaper::look::Look::default(),
            lut: None,
            quick_filter: None,
            t: ms as f32 / 1000.0,
            dim,
            mask: None,
            smooth: 0.0,
            pixels: Pixels::Half,
            cols: grid.0,
            rows: grid.1,
            crop: (0, 0),
            virt: None,
            hysteresis,
        };
        g.run_blocking(&termpaper::canvas::Canvas::new(1, 1), &plan).expect("frame")
    };
    let a = frame(&mut g, 10_000, 1.0, 3);
    // 1% dimmer: every channel moves by at most 2-3 levels — held
    let held = frame(&mut g, 10_000, 0.99, 3);
    assert_eq!(a, held, "sub-threshold changes must re-emit the previous cells");
    // half as bright: passes through
    let dark = frame(&mut g, 10_000, 0.5, 3);
    let reference = frame(&mut g, 10_000, 0.5, 0);
    assert_eq!(dark, reference, "big changes are sent as they are");
    // off: identical to a fresh render
    let off = frame(&mut g, 10_000, 0.99, 0);
    assert_ne!(off, a);
}

#[test]
#[ignore]
fn a_broken_scene_does_not_poison_the_device() {
    let mut g = Gpu::new(1, 1, 1).expect("Vulkan GPU");
    let bad = "//! name: bad\n//! title: Bad\n//! category: coast\n//! desc: broken\n//! themes: a, b\n\
               //! fallback: ocean\nfn scene(p: vec2f, ctx: Ctx) -> vec3f { return undefined_thing(p); }\n";
    let (composed, _) = shader::compose_text("bad.wgsl", "bad", bad).unwrap();
    let size = (32, 16);
    let err = g
        .render_shader_pixels("bad", &composed, &gpu::uniforms(&desc(size, (0, 0), size, 0, 0)))
        .expect_err("invalid WGSL must fail to compile");
    assert!(err.contains("bad.wgsl"), "error should point at the scene file: {err}");
    assert!(!g.failed(), "a compile error must not mark the device failed");
    if let Some(spec) = shader::SHADER_SCENES.first() {
        render(&mut g, spec, &desc(size, (0, 0), size, 0, 0));
    }
}

/// The real frame path: engine worker → compile thread → scene pass → post
/// chain → cell readback.
#[test]
#[ignore]
fn the_worker_delivers_cells_for_a_studio_scene() {
    let Some(spec) = shader::SHADER_SCENES.first() else { return };
    let mut worker = Worker::new(Renderer::Auto);
    let start = Instant::now();
    let grid = (80, 24);
    loop {
        let req = Request {
            generation: 0,
            sim: SimKey::new(spec.name, 1, SceneOptions::default(), (grid.0, grid.1 * 2), 1.0),
            view: ViewKey {
                canvas: (grid.0, grid.1 * 2),
                grid,
                crop: (0, 0),
                pixels: Pixels::Half,
                cell_aspect: DEFAULT_CELL_ASPECT,
                rev: 0,
                comp: None,
                classic_map: None,
            },
            elapsed_ms: start.elapsed().as_millis() as u64,
            paused: false,
            look: termpaper::look::Baked::new(termpaper::look::Look::with_effects(&["vignette".to_string()])),
            quick: None,
            dim: 1.0, mask: None,
            smooth: 0.0,
            budget_ms: DEFAULT_GPU_BUDGET_MS,
            prefetch: None,
            hysteresis: 3,
        };
        if let Some(frame) = worker.submit(req) {
            let cells = frame.cells.expect("GPU cells");
            assert_eq!(cells.len(), grid.0 * grid.1 * 3);
            assert!(frame.backend.starts_with("GPU shader"), "{}", frame.backend);
            let lit = cells.chunks(3).filter(|c| c[1] & 0xffffff != 0 || c[2] & 0xffffff != 0).count();
            assert!(lit > grid.0 * grid.1 / 2, "most cells should carry colour");
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(60), "no frame from the worker");
        std::thread::sleep(Duration::from_millis(5));
    }
}
