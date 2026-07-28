//! Pulse — concentric rings breathing on black. Marketplace demo scene.

use super::{Detail, Scene};
use crate::canvas::{ease_smooth, glow, lerp, Canvas};
use rand::rngs::StdRng;

pub struct Pulse {
    t: f32,
    theme: Theme,
}

#[derive(Clone, Copy)]
enum Theme {
    Classic,
    Ember,
    Ice,
    Mono,
}

impl Pulse {
    pub fn new(_rng: StdRng, theme: Option<&str>, _detail: Detail) -> Self {
        let theme = match theme {
            Some("ember") => Theme::Ember,
            Some("ice") => Theme::Ice,
            Some("mono") => Theme::Mono,
            _ => Theme::Classic,
        };
        Pulse { t: 0.0, theme }
    }

    fn palette(&self, k: f32) -> (u8, u8, u8) {
        let k = k.clamp(0.0, 1.0);
        match self.theme {
            Theme::Classic => lerp((20, 30, 50), (120, 180, 255), k),
            Theme::Ember => lerp((30, 10, 5), (255, 120, 40), k),
            Theme::Ice => lerp((10, 20, 35), (180, 230, 255), k),
            Theme::Mono => {
                let v = (k * 220.0) as u8;
                (v, v, v)
            }
        }
    }
}

fn ring(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, color: (u8, u8, u8)) {
    let steps = (r * 5.0) as usize + 12;
    for i in 0..steps {
        let a = i as f32 / steps as f32 * std::f32::consts::TAU;
        canvas.set_f(cx + a.cos() * r, cy + a.sin() * r, color);
    }
}

impl Scene for Pulse {
    fn name(&self) -> &'static str {
        "pulse"
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
        let max_r = w.min(h) as f32 * 0.48;
        let breath = 0.5 + 0.5 * (self.t * 1.1).sin();
        let rings = 6;

        for i in 0..rings {
            let phase = self.t * 0.9 + i as f32 * 0.55;
            let wave = 0.5 + 0.5 * phase.sin();
            let r = max_r * (0.15 + 0.85 * wave * breath);
            let k = ease_smooth(wave * breath);
            let col = self.palette(k);
            let alpha = (0.15 + 0.55 * k) * (1.0 - i as f32 / rings as f32 * 0.35);
            glow(
                canvas,
                cx as i32,
                cy as i32,
                r as i32,
                col,
                alpha,
            );
            ring(canvas, cx, cy, r, self.palette(k * 0.85));
        }

        let core = max_r * 0.08 * (0.7 + 0.3 * breath);
        glow(
            canvas,
            cx as i32,
            cy as i32,
            (core * 1.5) as i32,
            self.palette(0.95),
            0.85,
        );
    }
}
