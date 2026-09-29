//! Deterministic scene playback, independent of terminal refresh rate.
use crate::{
    canvas::Canvas,
    scene::{self, Scene, SceneOptions},
};
use rand::{rngs::StdRng, SeedableRng};
use std::time::{Duration, Instant};

/// Time one call may spend replaying a freshly built simulation up to the
/// anchor's clock. Nothing is shown meanwhile (the pane is black or mid
/// fade), so the budget is generous; it only bounds how long one request
/// can keep the worker from seeing a newer one.
pub const REPLAY_BUDGET: Duration = Duration::from_millis(12);

/// Live playback: ticks per call before the time budget applies, the budget,
/// and the hard cap. Ordinary 15–60 fps displays repay their live ticks even
/// when a scene takes more than 2 ms; only a hitch's extra replay is budgeted.
const LIVE_FREE_STEPS: u32 = 4;
const LIVE_BUDGET: Duration = Duration::from_millis(2);
const LIVE_MAX_STEPS: u32 = 90;

pub struct Playback {
    scene: Box<dyn Scene>,
    canvas: Canvas,
    ticks: u64,
    seed: u64,
    opts: SceneOptions,
    speed: f32,
    /// caught up with the clock at least once since the last rebuild
    primed: bool,
    /// the replay since the last rebuild: (started, ticks then, busy time)
    replay: Option<(Instant, u64, Duration)>,
}

/// Where one `advance` left the playback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    /// The output canvas shows the simulation at the requested clock (or,
    /// once primed, as close as live playback got). False while a rebuilt
    /// simulation is still replaying: the output is left untouched.
    pub ready: bool,
    /// Ticks still owed.
    pub behind: u64,
    /// While replaying: the estimated time until caught up, measured from
    /// the replay's actual progress (the first estimate assumes the worker
    /// does nothing else, so it can only be optimistic).
    pub eta: Option<Duration>,
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
            primed: false,
            replay: None,
        }
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
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
    ) -> Progress {
        self.advance_cancellable(name, opts, size, elapsed_ms, speed, paused, output, || {
            false
        })
    }

    /// Advance to the tick `elapsed_ms` falls in and copy the result into
    /// `output` — or, while a rebuilt simulation is still replaying up to
    /// that tick, spend up to `REPLAY_BUDGET` on the replay and leave
    /// `output` alone, so nobody sees the simulation fast-forward.
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
    ) -> Progress {
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
            self.primed = false;
            self.replay = None;
        }
        let target = elapsed_ms.saturating_mul(60) / 1000;
        let start = Instant::now();
        if !self.primed && !paused {
            let (began, from, busy) = *self.replay.get_or_insert((start, self.ticks, Duration::ZERO));
            while self.ticks < target {
                if cancelled() {
                    return Progress { ready: false, behind: target - self.ticks, eta: None };
                }
                self.scene.update(speed / 60.0, &mut self.canvas);
                self.ticks += 1;
                if start.elapsed() >= REPLAY_BUDGET {
                    break;
                }
            }
            let busy = busy + start.elapsed();
            if self.ticks < target {
                self.replay = Some((began, from, busy));
                // measured rate, including the time between calls the worker
                // spent waiting for requests
                let wall = began.elapsed().max(busy).as_secs_f64().max(1e-6);
                let rate = (self.ticks - from) as f64 / wall;
                let behind = target - self.ticks;
                let eta = (rate > 0.0).then(|| Duration::from_secs_f64(behind as f64 / rate));
                return Progress { ready: false, behind, eta };
            }
            self.primed = true;
            self.replay = None;
        } else {
            let mut steps = 0;
            while !paused && self.ticks < target && steps < LIVE_MAX_STEPS {
                if cancelled() {
                    return Progress { ready: false, behind: target - self.ticks, eta: None };
                }
                self.scene.update(speed / 60.0, &mut self.canvas);
                self.ticks += 1;
                steps += 1;
                if steps >= LIVE_FREE_STEPS && start.elapsed() >= LIVE_BUDGET {
                    break;
                }
            }
        }
        // Do not update with dt=0 on extra display frames: some scenes consume
        // randomness or evolve cellular state on every call. Keep filters out
        // of the simulation buffer for the same reason.
        self.canvas.snapshot_into(output);
        Progress { ready: true, behind: target.saturating_sub(self.ticks), eta: None }
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

    /// A rebuilt simulation replays out of sight: no frame reaches the
    /// output until it shows the anchor's tick exactly, with an estimate of
    /// the remaining replay meanwhile. Afterwards playback is live again.
    #[test]
    fn replay_is_hidden_until_caught_up() {
        let opts = SceneOptions::default();
        let mut player = Playback::new("life", &opts, 3);
        let mut out = Canvas::new(1, 1);
        out.clear((1, 2, 3));
        // two hours in: far beyond one call's budget
        let p = player.advance("life", &opts, (160, 80), 7_200_000, 1.0, false, &mut out);
        assert!(!p.ready, "a long replay cannot finish in one call");
        assert_eq!((out.width(), out.height()), (1, 1), "the output is untouched while replaying");
        assert!(p.eta.is_some_and(|e| e > Duration::from_secs(1)), "eta {:?}", p.eta);
        assert!(p.behind > 0 && p.behind < 432_000);
        // a short anchor finishes in one call and shows exactly its tick
        let mut fresh = Playback::new("life", &opts, 3);
        let p = fresh.advance("life", &opts, (48, 24), 2_000, 1.0, false, &mut out);
        assert!(p.ready && p.eta.is_none());
        assert_eq!(fresh.ticks(), 120);
        assert_eq!((out.width(), out.height()), (48, 24));
        // live: a stall is repaid with frames, not hidden
        let p = fresh.advance("life", &opts, (48, 24), 2_500, 1.0, false, &mut out);
        assert!(p.ready);
        assert_eq!(fresh.ticks(), 150);
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
