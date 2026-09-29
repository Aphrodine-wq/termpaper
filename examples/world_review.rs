//! GPU catalog validation and PPM capture. Run with --features gpu.
//!
//! By default every scene is rendered the way the worker renders it: the Rust
//! scene on the CPU, then GPU post-processing and cell packing (scenes in
//! `scene::GPU_WORLD_SCENES` use their WGSL arm). Pass `--world` to force the
//! WGSL arm for every scene and review the GPU worlds themselves.
//!
//! Usage: world_review [scene|all] [dir] [--world]
use rand::{rngs::StdRng, SeedableRng};
use std::{io::Write, time::Instant};
use termpaper::{
    canvas::Canvas,
    gpu::{FrameCells, Gpu, Plan},
    render::Pixels,
    scene::{self, SceneOptions},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let force_world = args.iter().any(|a| a == "--world");
    let positional: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    let selected = positional.first().map(|s| s.as_str()).unwrap_or("scroll");
    let dir = positional.get(1).map(|s| s.as_str()).unwrap_or("/tmp");
    let (w, h) = (180, 320);
    let mut gpu = Gpu::new(w, h, w * h / 2).expect("usable Vulkan adapter");
    eprintln!("adapter: {}", gpu.adapter_name());
    let mut canvas = Canvas::new(w, h);
    let opts = SceneOptions {
        pixels: Pixels::Half,
        ..SceneOptions::default()
    };
    let plan = Plan {
        look: &termpaper::look::Look::default(),
        lut: None,
        quick_filter: None,
        t: 8.0,
        dim: 1.0, mask: None,
        smooth: 0.0,
        pixels: Pixels::Half,
        cols: w,
        rows: h / 2,
        crop: (0, 0),
        virt: None,
        hysteresis: 0,
    };
    for name in scene::names() {
        if selected != "all" && selected != name {
            continue;
        }
        let world = name != "bump" && (force_world || scene::gpu_world(name));
        gpu.invalidate();
        let mut cpu = (!world).then(|| scene::create(name, &opts, StdRng::seed_from_u64(42)).unwrap());
        let start = Instant::now();
        for step in 0..120 {
            if world {
                gpu.scene_frame(name, &opts, 42, (step + 1) as f32 / 15.0, 1.0);
            } else {
                cpu.as_mut().unwrap().update(1.0 / 15.0, &mut canvas);
                gpu.canvas_frame();
            }
            gpu.run_blocking(&canvas, &plan).expect("valid frame");
            assert!(!gpu.failed(), "{name}: GPU validation/device error");
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / 120.0;
        let words = gpu.run_blocking(&canvas, &plan).unwrap();
        let cells = FrameCells {
            words: &words,
            cols: w,
            rows: h / 2,
        };
        let mut file = std::fs::File::create(format!("{dir}/{name}.ppm")).unwrap();
        write!(file, "P6\n{w} {h}\n255\n").unwrap();
        let mut bytes = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let (_, a, b) = cells.get(y / 2 * w + x);
                let c = if y % 2 == 0 { a } else { b };
                bytes.extend_from_slice(&[c.0, c.1, c.2]);
            }
        }
        let unique: std::collections::HashSet<_> = bytes.chunks_exact(3).collect();
        let minimum = if matches!(name, "dvd" | "bump") { 1 } else { 8 };
        assert!(
            unique.len() > minimum,
            "{name}: blank or flat output ({} colors)",
            unique.len()
        );
        file.write_all(&bytes).unwrap();
        println!(
            "{name:14} {ms:.3} ms/frame  {}",
            if world { "GPU world" } else { "CPU scene + GPU post" }
        );
    }
}
