//! Your scene — replace this file and register via marketplace/index.json PR.

use super::{Detail, Scene};
use crate::canvas::{ease_smooth, glow, lerp, Canvas};
use rand::rngs::StdRng;

pub struct YourScene {
    t: f32,
}

impl YourScene {
    pub fn new(_rng: StdRng, _theme: Option<&str>, _detail: Detail) -> Self {
        YourScene { t: 0.0 }
    }
}

impl Scene for YourScene {
    fn name(&self) -> &'static str {
        "your_scene"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        self.t += dt;
        canvas.clear((0, 0, 0));
        let cx = w as f32 * 0.5;
        let cy = h as f32 * 0.5;
        let pulse = 0.5 + 0.5 * (self.t * 1.4).sin();
        let r = pulse * w.min(h) as f32 * 0.35;
        let c = lerp((40, 40, 60), (180, 200, 255), ease_smooth(pulse));
        glow(canvas, cx as i32, cy as i32, r as i32, c, 0.7);
    }
}
