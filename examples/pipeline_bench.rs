//! Full-pipeline bench: scene update + filters + smoothing per frame,
//! mirroring the user's config (bloom + vignette, smooth 0.4, 200x100,
//! Detail::Low). Prints worst-to-best total ms/frame.
use std::time::Instant;
use termpaper::canvas::Canvas;
use termpaper::filter;
use termpaper::scene::{self, Detail, SceneOptions};
use rand::{rngs::StdRng, SeedableRng};

fn main() {
    let filters = vec!["bloom".to_string(), "vignette".to_string()];
    let mut rows: Vec<(String, f32, f32, f32)> = Vec::new();
    for name in scene::names() {
        let opts = SceneOptions {
            theme: None,
            detail: Detail::Low,
            text_scale: None,
            pixels: Default::default(),
        };
        let mut s = scene::create(name, &opts, StdRng::seed_from_u64(1)).unwrap();
        let mut canvas = Canvas::new(200, 100);
        let mut prev = Canvas::new(200, 100);
        s.update(1.0 / 30.0, &mut canvas); // init
        let mut upd = 0.0f32;
        let mut flt = 0.0f32;
        let mut smo = 0.0f32;
        for _ in 0..60 {
            let t = Instant::now();
            s.update(1.0 / 30.0, &mut canvas);
            upd += t.elapsed().as_secs_f32();
            let t = Instant::now();
            filter::apply_all(&filters, &mut canvas, 1.0);
            flt += t.elapsed().as_secs_f32();
            let t = Instant::now();
            canvas.smooth_blend(&prev, 0.6);
            prev = canvas.clone_for_smooth();
            smo += t.elapsed().as_secs_f32();
        }
        rows.push((
            name.to_string(),
            upd / 60.0 * 1000.0,
            flt / 60.0 * 1000.0,
            smo / 60.0 * 1000.0,
        ));
    }
    rows.sort_by(|a, b| (b.1 + b.2 + b.3).partial_cmp(&(a.1 + a.2 + a.3)).unwrap());
    println!("{:<14} {:>8} {:>8} {:>8} {:>8}", "scene", "update", "filters", "smooth", "total");
    for r in &rows {
        println!(
            "{:<14} {:>7.2}ms {:>7.2}ms {:>7.2}ms {:>7.2}ms",
            r.0, r.1, r.2, r.3, r.1 + r.2 + r.3
        );
    }
}
