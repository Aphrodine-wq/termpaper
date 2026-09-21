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
        filters: &[],
        quick_filter: None,
        t: 1.0,
        hue_shift: 0.0,
        saturation: 1.0,
        contrast: 1.0,
        dim: 1.0,
        smooth: 0.0,
        pixels: Pixels::Half,
        cols: 48,
        rows: 48,
        crop: (0, 0),
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
