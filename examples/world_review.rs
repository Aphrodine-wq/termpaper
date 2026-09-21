//! GPU catalog validation and PPM capture. Run with --features gpu.
use std::{io::Write, time::Instant};
use termpaper::{
    canvas::Canvas,
    gpu::{FrameCells, Gpu, Plan},
    render::Pixels,
    scene::{self, SceneOptions},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let selected = args.get(1).map(String::as_str).unwrap_or("scroll");
    let dir = args.get(2).map(String::as_str).unwrap_or("/tmp");
    let (w, h) = (180, 320);
    let mut gpu = Gpu::new(w, h, w * h / 2).expect("usable Vulkan adapter");
    eprintln!("adapter: {}", gpu.adapter_name());
    let canvas = Canvas::new(w, h);
    let opts = SceneOptions::default();
    let plan = Plan {
        filters: &[],
        quick_filter: None,
        t: 8.0,
        hue_shift: 0.0,
        saturation: 1.0,
        contrast: 1.0,
        dim: 1.0,
        smooth: 0.0,
        pixels: Pixels::Half,
        cols: w,
        rows: h / 2,
        crop: (0, 0),
    };
    for name in scene::names() {
        if selected != "all" && selected != name {
            continue;
        }
        gpu.invalidate();
        let start = Instant::now();
        for step in 0..120 {
            gpu.scene_frame(name, &opts, 42, (step + 1) as f32 / 15.0, 1.0);
            gpu.run_blocking(&canvas, &plan).expect("valid scene frame");
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
        println!("{name:14} {ms:.3} ms/frame");
    }
}
