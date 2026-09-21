//! TEMPORARY (deleted after use): frost-only bench at 200x100 Detail::High.
use std::time::Instant;
use termpaper::canvas::Canvas;
use termpaper::scene::{self, Detail, SceneOptions};
use rand::{rngs::StdRng, SeedableRng};

fn main() {
    for detail in [Detail::Low, Detail::Medium, Detail::High] {
        let opts = SceneOptions {
            theme: None,
            detail,
            text_scale: None,
            pixels: Default::default(),
        };
        let mut s = scene::create("frost", &opts, StdRng::seed_from_u64(1)).unwrap();
        let mut canvas = Canvas::new(200, 100);
        s.update(1.0 / 30.0, &mut canvas); // init
        // warm up into heavy growth (~20s in) so arms/branches are active
        for _ in 0..600 {
            s.update(1.0 / 30.0, &mut canvas);
        }
        let mut worst = 0.0f32;
        let mut total = 0.0f32;
        for _ in 0..120 {
            let t = Instant::now();
            s.update(1.0 / 30.0, &mut canvas);
            let e = t.elapsed().as_secs_f32() * 1000.0;
            worst = worst.max(e);
            total += e;
        }
        println!(
            "frost {:?}: avg {:.2}ms worst {:.2}ms",
            detail,
            total / 120.0,
            worst
        );
    }
}
