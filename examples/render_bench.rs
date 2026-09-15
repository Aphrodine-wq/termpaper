//! Render-path bench: canvas → ratatui buffer, per pixel mode, plus the
//! per-frame post-processing stages. Sizes mirror a real wide pane (272x33
//! cells) so the canvas pixel counts are realistic.
use rand::{rngs::StdRng, SeedableRng};
use ratatui::{buffer::Buffer, layout::Rect};
use std::time::Instant;
use termpaper::canvas::Canvas;
use termpaper::render::Pixels;
use termpaper::scene::{self, Detail, SceneOptions};
use termpaper::{color_grade, filter, render};

fn bench(label: &str, iters: usize, mut f: impl FnMut()) -> f32 {
    f(); // warm
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    let ms = t.elapsed().as_secs_f32() * 1000.0 / iters as f32;
    println!("  {label:<34} {ms:>7.3}ms");
    ms
}

/// The user's real config: fps 240, half pixels, detail low, koi,
/// filters noir+gamma+posterize, saturation 2.5, contrast 2.5, smooth 0.6.
/// Budget at 240fps is 4.17ms; the wall case is the 542-col virtual canvas.
fn user_config_bench() {
    let filters: Vec<String> = ["noir", "gamma", "posterize"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for (label, cols, rows) in [("single pane", 272u16, 33u16), ("wall virtual", 542, 33)] {
        let area = Rect::new(0, 0, cols, rows);
        let (pw, ph) = Pixels::Half.cell_size();
        let (w, h) = (cols as usize * pw, rows as usize * ph);
        println!("\n=== USER CONFIG · {label} · {cols}x{rows} cells → {w}x{h} px ===");
        let opts = SceneOptions {
            theme: None,
            detail: Detail::Low,
            text_scale: None,
        };
        let mut canvas = Canvas::new(w, h);
        let mut scene = scene::create("koi", &opts, StdRng::seed_from_u64(1)).unwrap();
        for _ in 0..30 {
            scene.update(1.0 / 60.0, &mut canvas);
        }
        let mut buf = Buffer::empty(area);
        let mut prev = canvas.clone_for_smooth();

        let a = bench("scene::update (koi, low)", 300, || {
            scene.update(4.0 / 240.0, &mut canvas);
        });
        let b = bench("filters noir+gamma+posterize", 300, || {
            filter::apply_all(&filters, &mut canvas, 1.0);
        });
        let c = bench("color_grade sat2.5 con2.5", 300, || {
            color_grade::apply(&mut canvas, 0.0, 2.5, 2.5);
        });
        let d = bench("smooth_blend + clone_for_smooth", 300, || {
            canvas.smooth_blend(&prev, 0.4);
            prev = canvas.clone_for_smooth();
        });
        let e = bench("render::draw half truecolor", 300, || {
            render::draw(&canvas, area, &mut buf, true, Pixels::Half);
        });
        let total = a + b + c + d + e;
        println!(
            "  {:<34} {total:>7.3}ms  → {:.0} fps ceiling  (240fps needs <4.17ms)",
            "TOTAL",
            1000.0 / total
        );
    }
}

fn main() {
    user_config_bench();
    let (cols, rows) = (272u16, 33u16);
    let area = Rect::new(0, 0, cols, rows);
    let opts = SceneOptions {
        theme: None,
        detail: Detail::Medium,
        text_scale: None,
    };

    for mode in [Pixels::Half, Pixels::Quad, Pixels::Braille] {
        let (pw, ph) = mode.cell_size();
        let (w, h) = (cols as usize * pw, rows as usize * ph);
        println!(
            "\n=== {:?}  {cols}x{rows} cells → {w}x{h} px ({} px) ===",
            mode,
            w * h
        );
        let mut canvas = Canvas::new(w, h);
        let mut scene = scene::create("plasma", &opts, StdRng::seed_from_u64(1)).unwrap();
        for _ in 0..10 {
            scene.update(1.0 / 60.0, &mut canvas);
        }
        let mut buf = Buffer::empty(area);

        let upd = bench("scene::update (plasma)", 200, || {
            scene.update(1.0 / 60.0, &mut canvas);
        });
        let dr = bench("render::draw truecolor", 200, || {
            render::draw(&canvas, area, &mut buf, true, mode);
        });
        let dr256 = bench("render::draw 256-color", 200, || {
            render::draw(&canvas, area, &mut buf, false, mode);
        });
        let grade = bench("color_grade::apply (hue+sat+con)", 200, || {
            color_grade::apply(&mut canvas, 20.0, 1.2, 1.1);
        });
        let filt = bench("filter bloom+vignette", 200, || {
            filter::apply_all(
                &["bloom".to_string(), "vignette".to_string()],
                &mut canvas,
                1.0,
            );
        });
        let prev = canvas.clone_for_smooth();
        let sm = bench("smooth_blend", 200, || {
            canvas.smooth_blend(&prev, 0.7);
        });
        println!(
            "  {:<34} {:>7.3}ms  → {:.0} fps ceiling",
            "TOTAL (update+draw+grade+filter+smooth)",
            upd + dr + grade + filt + sm,
            1000.0 / (upd + dr + grade + filt + sm)
        );
        println!("  (256-color draw would be {:+.3}ms vs truecolor)", dr256 - dr);
    }
}
