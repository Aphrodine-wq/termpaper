//! Deterministic scene playback, independent of terminal refresh rate.
use crate::{
    canvas::Canvas,
    scene::{self, Scene, SceneOptions},
};
use rand::{rngs::StdRng, SeedableRng};
use std::time::{Duration, Instant};

pub struct Playback {
    scene: Box<dyn Scene>,
    canvas: Canvas,
    ticks: u64,
    seed: u64,
    opts: SceneOptions,
    speed: f32,
}

impl Playback {
    pub fn new(name: &str, opts: &SceneOptions, seed: u64) -> Self {
        Self {
            scene: scene::create(name, opts, StdRng::seed_from_u64(seed)).expect("registry"),
            canvas: Canvas::new(0, 0),
            ticks: 0,
            seed,
            opts: opts.clone(),
            speed: 1.0,
        }
    }

    /// Replay at the final wall size. Resizing reconstructs the RNG as well
    /// as the scene, so different monitor discovery histories converge.
    pub fn advance(
        &mut self,
        name: &str,
        opts: &SceneOptions,
        size: (usize, usize),
        elapsed_ms: u64,
        speed: f32,
        paused: bool,
        output: &mut Canvas,
    ) {
        self.advance_cancellable(name, opts, size, elapsed_ms, speed, paused, output, || {
            false
        });
    }

    pub fn advance_cancellable(
        &mut self,
        name: &str,
        opts: &SceneOptions,
        size: (usize, usize),
        elapsed_ms: u64,
        speed: f32,
        paused: bool,
        output: &mut Canvas,
        cancelled: impl Fn() -> bool,
    ) {
        if (self.canvas.width(), self.canvas.height()) != size
            || self.opts != *opts
            || self.speed != speed
        {
            self.scene =
                scene::create(name, opts, StdRng::seed_from_u64(self.seed)).expect("registry");
            self.canvas.resize(size.0, size.1);
            self.ticks = 0;
            self.opts = opts.clone();
            self.speed = speed;
        }
        let target = elapsed_ms.saturating_mul(60) / 1000;
        let start = Instant::now();
        let mut steps = 0;
        while !paused && self.ticks < target && steps < 90 {
            if cancelled() {
                return;
            }
            self.scene.update(speed / 60.0, &mut self.canvas);
            self.ticks += 1;
            steps += 1;
            // Allow ordinary 15–60fps displays to repay their live ticks even
            // when a scene takes more than 2ms; budget only the extra replay.
            if steps >= 4 && start.elapsed() >= Duration::from_millis(2) {
                break;
            }
        }
        // Do not update with dt=0 on extra display frames: some scenes consume
        // randomness or evolve cellular state on every call. Keep filters out
        // of the simulation buffer for the same reason.
        self.canvas.snapshot_into(output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_anchor_is_not_truncated_and_filters_cannot_change_state() {
        let opts = SceneOptions::default();
        let mut player = Playback::new("fire", &opts, 7);
        let mut output = Canvas::new(1, 1);
        while player.ticks < 1860 {
            player.advance("fire", &opts, (24, 12), 31_000, 1.0, false, &mut output);
        }
        let expected = output.get(12, 6);
        output.clear((255, 0, 255));
        player.advance("fire", &opts, (24, 12), 31_000, 1.0, false, &mut output);
        assert_eq!(output.get(12, 6), expected);
        assert_eq!(player.ticks, 1860);
    }

    #[test]
    fn refresh_rates_late_join_and_resize_converge() {
        let opts = SceneOptions::default();
        for name in scene::names() {
            let mut live = Playback::new(name, &opts, 42);
            let mut late = Playback::new(name, &opts, 42);
            let mut a = Canvas::new(1, 1);
            let mut b = Canvas::new(1, 1);
            for ms in (0..=1000).step_by(8) {
                live.advance(name, &opts, (48, 24), ms, 1.0, false, &mut a);
            }
            late.advance(name, &opts, (20, 10), 100, 1.0, false, &mut b);
            while live.ticks < 60 {
                live.advance(name, &opts, (48, 24), 1000, 1.0, false, &mut a);
            }
            // Resize even if the initial small-canvas replay hit its budget.
            late.advance(name, &opts, (48, 24), 1000, 1.0, false, &mut b);
            while late.ticks < 60 {
                late.advance(name, &opts, (48, 24), 1000, 1.0, false, &mut b);
            }
            for y in 0..24 {
                for x in 0..48 {
                    assert_eq!(a.get(x, y), b.get(x, y), "{name} at {x},{y}");
                }
            }
            live.advance(name, &opts, (48, 24), 1000, 1.0, false, &mut a);
            assert_eq!(live.ticks, 60);
        }
    }
}
