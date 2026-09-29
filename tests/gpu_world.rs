#![cfg(feature = "gpu")]
use termpaper::{
    canvas::Canvas,
    gpu::{Gpu, Plan},
    render::Pixels,
    scene::SceneOptions,
};

#[test]
#[ignore = "requires a real Vulkan device; run with --features gpu -- --ignored"]
fn gpu_replay_resize_and_backpressure() {
    let mut gpu = Gpu::new(48, 96, 48 * 48).expect("Vulkan device required");
    let canvas = Canvas::new(48, 96);
    let opts = SceneOptions::default();
    let plan = Plan {
        look: &termpaper::look::Look::default(),
        lut: None,
        quick_filter: None,
        t: 1.0,
        dim: 1.0,
        smooth: 0.0,
        pixels: Pixels::Half,
        cols: 48,
        rows: 48,
        crop: (0, 0),
        virt: None,
        hysteresis: 0,
    };
    for name in ["fire", "life", "boids", "sand", "reaction"] {
        gpu.invalidate();
        for tick in 1..=60 {
            gpu.scene_frame(name, &opts, 42, tick as f32 / 60.0, 1.0);
            gpu.run_blocking(&canvas, &plan).unwrap();
        }
        let expected = gpu.run_blocking(&canvas, &plan).unwrap();
        // Recreate the same world following a different resize history and
        // arrive late. Four replay ticks per request must converge exactly.
        gpu.resize(16, 32, 256);
        gpu.resize(48, 96, 48 * 48);
        gpu.scene_frame(name, &opts, 42, 1.0, 1.0);
        for _ in 0..15 {
            gpu.run_blocking(&canvas, &plan).unwrap();
        }
        let actual = gpu.run_blocking(&canvas, &plan).unwrap();
        assert_eq!(expected, actual, "{name}: replay/resize diverged");
        assert!(!gpu.failed(), "{name}: device/validation failure");
    }
    gpu.invalidate();
    for _ in 0..3 {
        assert!(gpu.submit(&canvas, &plan));
    }
    assert!(
        !gpu.submit(&canvas, &plan),
        "full ring must reject, never wait"
    );
    gpu.invalidate();
    assert!(
        gpu.poll_cells(48, 48).is_none(),
        "stale scene frame escaped invalidation"
    );
    gpu.scene_frame("scroll", &opts, 17, 1.0, 1.0);
    assert!(gpu.run_blocking(&canvas, &plan).is_some());
    assert!(!gpu.failed());
}

#[test]
#[ignore = "requires a real Vulkan device; run with --features gpu -- --ignored"]
fn hybrid_canvas_frame_packs_cpu_scene() {
    use rand::{rngs::StdRng, SeedableRng};
    use termpaper::gpu::FrameCells;
    let (w, h) = (48usize, 96usize);
    let mut gpu = Gpu::new(w, h, w * h / 2).expect("Vulkan device required");
    let mut canvas = Canvas::new(w, h);
    let opts = SceneOptions {
        pixels: Pixels::Half,
        ..SceneOptions::default()
    };
    let mut scene = termpaper::scene::create("scroll", &opts, StdRng::seed_from_u64(42)).unwrap();
    for _ in 0..30 {
        scene.update(1.0 / 30.0, &mut canvas);
    }
    let plan = Plan {
        look: &termpaper::look::Look::default(),
        lut: None,
        quick_filter: None,
        t: 1.0,
        dim: 1.0,
        smooth: 0.0,
        pixels: Pixels::Half,
        cols: w,
        rows: h / 2,
        crop: (0, 0),
        virt: None,
        hysteresis: 0,
    };
    gpu.canvas_frame();
    let words = gpu.run_blocking(&canvas, &plan).unwrap();
    assert!(!gpu.failed());
    let cells = FrameCells { words: &words, cols: w, rows: h / 2 };
    let mut unique = std::collections::HashSet::new();
    for i in 0..w * h / 2 {
        let (_, a, b) = cells.get(i);
        unique.insert(a);
        unique.insert(b);
    }
    assert!(unique.len() > 8, "flat output ({} colors)", unique.len());
    // half mode packs the two source pixels of a cell verbatim
    for (x, row) in [(0usize, 0usize), (10, 20), (47, 47)] {
        let (_, a, b) = cells.get(row * w + x);
        assert_eq!(a, canvas.get(x as i32, (row * 2) as i32).color, "top pixel at {x},{row}");
        assert_eq!(b, canvas.get(x as i32, (row * 2 + 1) as i32).color, "bottom pixel at {x},{row}");
    }
}
