//! Studio shader scenes on a real GPU. Ignored by default (CI has no Vulkan
//! device): `cargo test --release --test gpu_shader_scenes -- --ignored`.
#![cfg(feature = "gpu")]
use std::time::{Duration, Instant};
use termpaper::engine::{Renderer, Request, SceneKey, Worker, DEFAULT_GPU_BUDGET_MS};
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
            key: SceneKey {
                name: spec.name.into(),
                seed: 1,
                opts: SceneOptions::default(),
                size: (grid.0, grid.1 * 2),
                grid,
                crop: (0, 0),
                pixels: Pixels::Half,
            },
            elapsed_ms: start.elapsed().as_millis() as u64,
            speed: 1.0,
            paused: false,
            filters: vec!["vignette".into()],
            quick: None,
            hue: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            dim: 1.0,
            smooth: 0.0,
            budget_ms: DEFAULT_GPU_BUDGET_MS,
            prefetch: None,
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
