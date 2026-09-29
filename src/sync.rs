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

/// Maps this process's monotonic clock onto the shared epoch clock and lays
/// frame deadlines on a grid every pane agrees on: slot `n` of a grid
/// anchored at `origin_ms` with period `p` is epoch time `origin + n·p`,
/// whatever the process's `Instant` base. Panes with the same anchor and
/// fps therefore present the same slots, and a 30 fps pane's slots are
/// every other 60 fps slot.
#[derive(Clone, Copy, Debug)]
pub struct FrameClock {
    base: Instant,
    base_epoch_ms: f64,
}

/// Epoch ms with sub-ms precision.
pub fn epoch_now_ms_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// Realtime drift beyond this re-samples the clock mapping (NTP slew).
const RESYNC_MS: f64 = 2.0;

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameClock {
    pub fn new() -> Self {
        Self::with_base(Instant::now(), epoch_now_ms_f64())
    }

    /// A clock whose `base` instant reads `base_epoch_ms` on the epoch clock.
    pub fn with_base(base: Instant, base_epoch_ms: f64) -> Self {
        FrameClock { base, base_epoch_ms }
    }

    /// Epoch ms at monotonic instant `t`.
    pub fn epoch_at(&self, t: Instant) -> f64 {
        if t >= self.base {
            self.base_epoch_ms + (t - self.base).as_secs_f64() * 1000.0
        } else {
            self.base_epoch_ms - (self.base - t).as_secs_f64() * 1000.0
        }
    }

    /// Monotonic instant at epoch time `ms`.
    pub fn instant_at(&self, ms: f64) -> Instant {
        let d = ms - self.base_epoch_ms;
        if d >= 0.0 {
            self.base + Duration::from_secs_f64(d / 1000.0)
        } else {
            self.base.checked_sub(Duration::from_secs_f64(-d / 1000.0)).unwrap_or(self.base)
        }
    }

    /// Compare the mapping with a fresh realtime reading and re-sample it
    /// when they disagree. Returns how far realtime ran ahead of the
    /// mapping: a few ms of NTP slew, or — on the order of seconds and more
    /// — a system suspend, during which the monotonic clock stood still.
    pub fn resync(&mut self, now: Instant, epoch_now_ms: f64) -> f64 {
        let drift = epoch_now_ms - self.epoch_at(now);
        if drift.abs() > RESYNC_MS {
            self.base = now;
            self.base_epoch_ms = epoch_now_ms;
        }
        drift
    }

    /// The latest slot at or before `now` on the grid `origin + n·period`.
    pub fn slot_at(&self, now: Instant, origin_ms: u64, period_ms: f64) -> i64 {
        ((self.epoch_at(now) - origin_ms as f64) / period_ms + 1e-6).floor() as i64
    }

    /// Epoch ms of slot `n`.
    pub fn slot_epoch(origin_ms: u64, period_ms: f64, n: i64) -> f64 {
        origin_ms as f64 + n as f64 * period_ms
    }

    /// Monotonic deadline of slot `n`.
    pub fn slot_instant(&self, origin_ms: u64, period_ms: f64, n: i64) -> Instant {
        self.instant_at(Self::slot_epoch(origin_ms, period_ms, n))
    }
}

/// Scene clock for slot `n` of a grid anchored at the scene's t0: the first
/// whole ms at or after the slot. Rounding up (never down) makes the 60 Hz
/// Classic tick at slot `n` of a 60 fps grid exactly `n` — and exactly
/// `2n` at 30 fps — so panes at different rates agree on shared slots.
pub fn slot_elapsed_ms(n: i64, period_ms: f64) -> u64 {
    if n <= 0 {
        0
    } else {
        (n as f64 * period_ms - 1e-6).ceil() as u64
    }
}

/// Classic scenes advance on the 60 Hz tick grid: rates that do not divide
/// 60 would show ticks unevenly, and rates above 60 would re-send identical
/// ticks. Snap to the highest divisor of 60 at or below `fps`.
pub fn classic_fps(fps: u32) -> u32 {
    const DIVISORS: [u32; 12] = [60, 30, 20, 15, 12, 10, 6, 5, 4, 3, 2, 1];
    DIVISORS.into_iter().find(|&d| d <= fps.max(1)).unwrap_or(1)
}

/// Most frames a request is made ahead of its display slot.
pub const MAX_LEAD: u32 = 3;
/// Late frames in a row before the lead grows (quick: late frames show).
const RAISE_AFTER: u32 = 3;
/// Early frames in a row before the lead shrinks (slow: avoid flapping).
const LOWER_AFTER: u32 = 90;

/// Adaptive render lead. A request is made for slot `now + lead`; the lead
/// tracks how many slots the renderer actually takes to deliver a frame, so
/// the frame displayed at a slot is the one rendered for that slot instead
/// of one rendered a frame or two earlier. It grows after a few late frames
/// and shrinks only after a long run of early ones.
#[derive(Clone, Debug)]
pub struct Lead {
    lead: u32,
    /// (scene clock requested, slot it was requested at), oldest first
    pending: std::collections::VecDeque<(u64, i64)>,
    last_frame: Option<u64>,
    late: u32,
    early: u32,
}

impl Default for Lead {
    fn default() -> Self {
        Self::new()
    }
}

impl Lead {
    pub fn new() -> Self {
        Lead { lead: 1, pending: Default::default(), last_frame: None, late: 0, early: 0 }
    }

    pub fn lead(&self) -> u32 {
        self.lead
    }

    /// A request for scene clock `elapsed_ms` was made at slot `slot`.
    pub fn on_submit(&mut self, elapsed_ms: u64, slot: i64) {
        if self.pending.back().is_some_and(|&(e, _)| e == elapsed_ms) {
            return; // a paused clock repeats: keep the first request
        }
        self.pending.push_back((elapsed_ms, slot));
        while self.pending.len() > 16 {
            self.pending.pop_front();
        }
    }

    /// A frame showing scene clock `elapsed_ms` is displayed at slot `slot`.
    pub fn on_frame(&mut self, elapsed_ms: u64, slot: i64) {
        if self.last_frame == Some(elapsed_ms) {
            return; // the same frame again
        }
        self.last_frame = Some(elapsed_ms);
        let Some(pos) = self.pending.iter().position(|&(e, _)| e == elapsed_ms) else {
            return;
        };
        let submitted = self.pending[pos].1;
        self.pending.drain(..=pos);
        let latency = (slot - submitted).clamp(0, MAX_LEAD as i64) as u32;
        if latency > self.lead {
            self.early = 0;
            self.late += 1;
            if self.late >= RAISE_AFTER {
                self.lead = latency;
                self.late = 0;
            }
        } else if latency < self.lead {
            self.late = 0;
            self.early += 1;
            if self.early >= LOWER_AFTER {
                self.lead -= 1;
                self.early = 0;
            }
        } else {
            self.late = 0;
            self.early = 0;
        }
    }

    /// The grid moved (new anchor, retime): outstanding requests no longer
    /// map onto it. The lead itself is kept — the renderer did not change.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.last_frame = None;
        self.late = 0;
        self.early = 0;
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    /// Two processes with unrelated monotonic bases — sampled a second
    /// apart, with the realtime reading taken for each — map the same real
    /// moment to the same slot and the same deadline.
    #[test]
    fn clocks_with_different_instant_bases_agree_on_slots() {
        let epoch0 = 1_790_000_000_000.25f64;
        let base_a = Instant::now();
        let base_b = base_a + Duration::from_millis(1234);
        let a = FrameClock::with_base(base_a, epoch0);
        let b = FrameClock::with_base(base_b, epoch0 + 1234.0);
        let origin = 1_789_999_000_000u64;
        for period in [1000.0 / 60.0, 1000.0 / 30.0, 1000.0 / 48.0] {
            for k in 0..500u64 {
                let t = base_a + Duration::from_micros(k * 7_919);
                let (sa, sb) = (a.slot_at(t, origin, period), b.slot_at(t, origin, period));
                assert_eq!(sa, sb, "period {period} at {k}");
                let (da, db) = (a.slot_instant(origin, period, sa + 1), b.slot_instant(origin, period, sb + 1));
                let skew = if da > db { da - db } else { db - da };
                assert!(skew < Duration::from_micros(5), "deadlines {skew:?} apart");
                assert!(da > t, "the next deadline is after now");
            }
        }
    }

    /// A 30 fps pane shows every other slot of a 60 fps pane, and both
    /// render the same Classic tick there.
    #[test]
    fn thirty_and_sixty_fps_land_on_the_same_ticks() {
        let (p60, p30) = (1000.0 / 60.0, 1000.0 / 30.0);
        for n in 0..10_000i64 {
            let e60 = slot_elapsed_ms(2 * n, p60);
            let e30 = slot_elapsed_ms(n, p30);
            assert_eq!(e60 * 60 / 1000, (2 * n) as u64, "60 fps slot {} tick", 2 * n);
            assert_eq!(e30 * 60 / 1000, e60 * 60 / 1000, "30 fps slot {n} tick");
            let odd = slot_elapsed_ms(2 * n + 1, p60);
            assert_eq!(odd * 60 / 1000, (2 * n + 1) as u64);
        }
        for (fps, want) in [(240, 60), (120, 60), (60, 60), (59, 30), (48, 30), (25, 20), (16, 15), (11, 10), (7, 6), (1, 1), (0, 1)] {
            assert_eq!(classic_fps(fps), want, "{fps}");
        }
        // and a 30 fps pane's deadlines are 60 fps deadlines
        let c = FrameClock::with_base(Instant::now(), 5_000_000.0);
        for n in 0..100 {
            let (a, b) = (c.slot_instant(4_000_000, p30, n), c.slot_instant(4_000_000, p60, 2 * n));
            let skew = if a > b { a - b } else { b - a };
            assert!(skew < Duration::from_micros(1), "slot {n}: {skew:?}");
        }
    }

    #[test]
    fn resync_reports_a_suspend_and_follows_it() {
        let base = Instant::now();
        let mut c = FrameClock::with_base(base, 1_000_000.0);
        let t = base + Duration::from_secs(1);
        assert!(c.resync(t, 1_001_000.5).abs() < RESYNC_MS, "no drift, no resample");
        // eight hours of realtime pass during one monotonic second
        let jump = c.resync(t + Duration::from_secs(1), 1_002_000.0 + 8.0 * 3_600_000.0);
        assert!((jump - 8.0 * 3_600_000.0).abs() < 1.0);
        assert!((c.epoch_at(t + Duration::from_secs(1)) - (1_002_000.0 + 8.0 * 3_600_000.0)).abs() < 1e-3);
    }

    fn run_lead(lead: &mut Lead, latency: i64, frames: i64, slot0: i64) -> i64 {
        let mut slot = slot0;
        for _ in 0..frames {
            // request for the slot `lead` ahead; its frame lands `latency` later
            let e = slot_elapsed_ms(slot + lead.lead() as i64, 1000.0 / 60.0);
            lead.on_submit(e, slot);
            lead.on_frame(e, slot + latency);
            slot += 1;
        }
        slot
    }

    #[test]
    fn lead_adapts_to_the_renderer_latency() {
        let mut lead = Lead::new();
        assert_eq!(lead.lead(), 1);
        // a pipelined GPU delivers two slots after the request
        let slot = run_lead(&mut lead, 2, 10, 0);
        assert_eq!(lead.lead(), 2, "grows after a few late frames");
        // one stray late frame does not move it
        let slot = run_lead(&mut lead, 3, 1, slot);
        let slot = run_lead(&mut lead, 2, 5, slot);
        assert_eq!(lead.lead(), 2);
        // the renderer gets faster: shrinks, but only after a long run
        let slot = run_lead(&mut lead, 1, 30, slot);
        assert_eq!(lead.lead(), 2, "hysteresis: no flapping on a short run");
        run_lead(&mut lead, 1, 200, slot);
        assert_eq!(lead.lead(), 1);
        // capped
        let mut slow = Lead::new();
        run_lead(&mut slow, 9, 20, 0);
        assert_eq!(slow.lead(), MAX_LEAD);
    }

    #[test]
    fn lead_ignores_repeats_and_unknown_frames() {
        let mut lead = Lead::new();
        for s in 0..20 {
            lead.on_submit(500, s); // paused: one clock value over and over
            lead.on_frame(500, s + 3);
        }
        assert_eq!(lead.lead(), 1, "a repeated frame counts once");
        lead.reset();
        for s in 0..20 {
            lead.on_frame(10_000 + s as u64, s); // never requested
        }
        assert_eq!(lead.lead(), 1);
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
        // a short anchor finishes within a few calls (one on an idle
        // machine) and then shows exactly its tick
        let mut fresh = Playback::new("life", &opts, 3);
        let mut p = fresh.advance("life", &opts, (48, 24), 2_000, 1.0, false, &mut out);
        for _ in 0..20 {
            if p.ready {
                break;
            }
            assert_eq!((out.width(), out.height()), (1, 1), "hidden until caught up");
            p = fresh.advance("life", &opts, (48, 24), 2_000, 1.0, false, &mut out);
        }
        assert!(p.ready && p.eta.is_none());
        assert_eq!(fresh.ticks(), 120);
        assert_eq!((out.width(), out.height()), (48, 24));
        // live: a stall is repaid with frames, not hidden (every call
        // returns a frame, however many calls the repayment takes)
        for _ in 0..40 {
            let p = fresh.advance("life", &opts, (48, 24), 2_500, 1.0, false, &mut out);
            assert!(p.ready);
            if fresh.ticks() == 150 {
                break;
            }
        }
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
