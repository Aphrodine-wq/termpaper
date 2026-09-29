//! Scene transition state machine: fade the outgoing scene to black over
//! ~0.25s, swap, fade the new one in. Cheap — a single luminance scale.
//!
//! Two clocks drive it. Local requests (`request` + `tick(dt)`) fade on the
//! pane's own time, from whenever the request arrived. Linked switches are
//! scheduled on the shared epoch clock (`schedule` + `tick_at`): the swap
//! happens at `swap_at_ms` on every pane, the fade-out ends there and the
//! fade-in starts there, so a pane that learns of the switch late simply
//! joins the curve where the others already are.

use crate::canvas::ease_smooth;

pub const FADE_SECS: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    In,
    Steady,
    Out,
}

/// A switch pinned to the epoch clock.
#[derive(Clone, Copy, Debug)]
struct Scheduled {
    scene: usize,
    swap_at_ms: u64,
    swapped: bool,
}

pub struct Transition {
    phase: Phase,
    t: f32,
    pending: Option<usize>,
    fade_secs: f32,
    sched: Option<Scheduled>,
    /// A scheduled switch that swapped and is still fading in when a newer
    /// one arrived: the new fade-out starts from its level, not from 1.
    fading_in_since: Option<u64>,
    /// Epoch time of the last `tick_at`, for the local (dt) phases.
    last_ms: Option<u64>,
}

impl Default for Transition {
    fn default() -> Self {
        Self::new()
    }
}

impl Transition {
    pub fn new() -> Self {
        Transition {
            phase: Phase::In,
            t: 0.0,
            pending: None,
            fade_secs: FADE_SECS,
            sched: None,
            fading_in_since: None,
            last_ms: None,
        }
    }

    /// Fade duration in whole ms (the epoch clock's unit).
    pub fn fade_ms(&self) -> u64 {
        (self.fade_secs * 1000.0).round() as u64
    }

    /// Schedule a switch to `scene` at epoch time `swap_at_ms`: the fade-out
    /// runs over the `fade` before it and the fade-in over the `fade` after,
    /// identically on every pane that schedules the same switch. Supersedes
    /// any local request.
    pub fn schedule(&mut self, scene: usize, swap_at_ms: u64) {
        if let Some(s) = self.sched {
            if s.swapped {
                self.fading_in_since = Some(s.swap_at_ms);
            }
        }
        self.pending = None;
        self.sched = Some(Scheduled { scene, swap_at_ms, swapped: false });
    }

    /// Fade-in level `ms` after a swap (1 once complete).
    fn fade_in_at(&self, since_ms: u64, now_ms: u64) -> f32 {
        let fade = self.fade_ms().max(1) as f32;
        ease_smooth((now_ms.saturating_sub(since_ms) as f32 / fade).min(1.0))
    }

    /// Advance to epoch time `now_ms`. Returns (fade factor, scene index to
    /// swap to now, if any). Scheduled switches depend only on `now_ms`, so
    /// two panes asked about the same moment answer the same; local phases
    /// (the startup fade-in, `request`) advance by the time since the last
    /// call.
    pub fn tick_at(&mut self, now_ms: u64) -> (f32, Option<usize>) {
        let dt = self
            .last_ms
            .map(|l| now_ms.saturating_sub(l) as f32 / 1000.0)
            .unwrap_or(0.0);
        self.last_ms = Some(now_ms.max(self.last_ms.unwrap_or(0)));
        let Some(mut s) = self.sched else {
            return self.tick(dt);
        };
        let fade = self.fade_ms().max(1) as f32;
        if !s.swapped && now_ms >= s.swap_at_ms {
            s.swapped = true;
            self.sched = Some(s);
            self.fading_in_since = None;
            self.pending = None;
            self.phase = Phase::In;
            let f = self.fade_in_at(s.swap_at_ms, now_ms);
            if f >= 1.0 {
                self.sched = None;
                self.phase = Phase::Steady;
            }
            return (f, Some(s.scene));
        }
        if s.swapped {
            let f = self.fade_in_at(s.swap_at_ms, now_ms);
            if f >= 1.0 {
                self.sched = None;
                self.t = 0.0;
                // a local request that arrived meanwhile starts fading out
                self.phase = if self.pending.is_some() { Phase::Out } else { Phase::Steady };
            }
            return (f, None);
        }
        // before the swap: fading out over the last `fade` ms
        let until = (s.swap_at_ms - now_ms) as f32 / fade;
        let mut f = ease_smooth(until.min(1.0));
        if let Some(since) = self.fading_in_since {
            f = f.min(self.fade_in_at(since, now_ms));
        }
        // the local startup fade-in still runs underneath
        if self.phase == Phase::In {
            self.t += dt;
            if self.t >= self.fade_secs {
                self.phase = Phase::Steady;
            } else {
                f = f.min(ease_smooth(self.t / self.fade_secs));
            }
        }
        (f, None)
    }

    /// Set the fade duration in seconds (settings menu / config).
    pub fn set_fade_secs(&mut self, secs: f32) {
        self.fade_secs = secs.clamp(0.05, 2.0);
    }

    /// Ask for a scene switch. During Steady this starts fading out;
    /// queued requests just replace the pending target.
    pub fn request(&mut self, scene: usize) {
        self.pending = Some(scene);
        if self.phase == Phase::Steady {
            self.phase = Phase::Out;
            self.t = 0.0;
        }
    }

    /// Advance. Returns (fade factor, scene index to swap to now, if any).
    pub fn tick(&mut self, dt: f32) -> (f32, Option<usize>) {
        let fade = self.fade_secs;
        match self.phase {
            Phase::In => {
                self.t += dt;
                if self.t >= fade {
                    self.phase = Phase::Steady;
                    // a switch requested during fade-in starts fading out
                    if self.pending.is_some() {
                        self.phase = Phase::Out;
                        self.t = 0.0;
                    }
                    (1.0, None)
                } else {
                    (ease_smooth(self.t / fade), None)
                }
            }
            Phase::Steady => (1.0, None),
            Phase::Out => {
                self.t += dt;
                if self.t >= fade {
                    self.phase = Phase::In;
                    self.t = 0.0;
                    (0.0, self.pending.take())
                } else {
                    (1.0 - ease_smooth(self.t / fade), None)
                }
            }
        }
    }

    /// The scene a queued or in-progress switch will land on — lets the
    /// renderer start compiling it while the old scene fades out.
    pub fn pending(&self) -> Option<usize> {
        match self.sched {
            Some(s) if !s.swapped => Some(s.scene),
            _ => self.pending,
        }
    }

    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Steady && self.pending.is_none() && self.sched.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_transition_cycle() {
        let mut tr = Transition::new();
        // fade in
        let (f, swap) = tr.tick(0.1);
        assert!(f > 0.0 && f < 1.0 && swap.is_none());
        let (f, _) = tr.tick(0.2);
        assert_eq!(f, 1.0);
        assert!(tr.is_idle());
        // request switch: fades out, then swaps
        tr.request(7);
        assert!(!tr.is_idle());
        let (f, swap) = tr.tick(0.1);
        assert!(f < 1.0 && swap.is_none());
        let (f, swap) = tr.tick(0.2);
        assert_eq!(swap, Some(7));
        assert_eq!(f, 0.0);
        // fades back in
        let (f, _) = tr.tick(0.3);
        assert_eq!(f, 1.0);
        assert!(tr.is_idle());
    }

    #[test]
    fn request_during_fade_queues_target() {
        let mut tr = Transition::new();
        tr.request(3);
        let mut last_swap = None;
        for _ in 0..20 {
            let (_, swap) = tr.tick(0.05);
            if swap.is_some() {
                last_swap = swap;
            }
        }
        assert_eq!(last_swap, Some(3));
        assert!(tr.is_idle());
    }
}

#[cfg(test)]
mod epoch_tests {
    use super::*;

    fn settled(start_ms: u64) -> Transition {
        let mut tr = Transition::new();
        tr.tick_at(start_ms);
        tr.tick_at(start_ms + 1000); // startup fade-in done
        assert!(tr.is_idle());
        tr
    }

    /// Two panes learn of the same switch at different times — one well
    /// before the fade-out starts, one mid-fade-out, one after the swap —
    /// yet at every epoch moment all show the same fade and swap together.
    #[test]
    fn scheduled_fade_depends_only_on_epoch_time() {
        let swap = 100_000u64;
        let mut early = settled(90_000);
        early.schedule(4, swap);
        // times from before the fade-out to after the fade-in, 10ms apart
        let times: Vec<u64> = (swap - 400..=swap + 400).step_by(10).collect();
        let mut reference = Vec::new();
        for &t in &times {
            reference.push(early.tick_at(t));
        }
        assert_eq!(reference.first().unwrap().0, 1.0, "no fade 400ms before the swap");
        assert!(reference.iter().any(|&(f, _)| f > 0.0 && f < 1.0));
        let swaps: Vec<u64> = times.iter().zip(&reference).filter(|(_, r)| r.1.is_some()).map(|(t, _)| *t).collect();
        assert_eq!(swaps, vec![swap], "exactly one swap, at swap_at");
        for receipt in [swap - 300, swap - 120, swap + 90] {
            let mut late = settled(receipt - 2000);
            late.schedule(4, swap);
            let mut swapped = false;
            for (&t, &(f, s)) in times.iter().zip(&reference) {
                if t < receipt {
                    continue;
                }
                let (lf, ls) = late.tick_at(t);
                assert!((lf - f).abs() < 1e-6, "receipt {receipt}: fade {lf} vs {f} at {t}");
                if ls.is_some() {
                    swapped = true;
                    assert_eq!(ls, Some(4));
                }
                if s.is_some() {
                    assert!(swapped, "the late pane must swap by the reference swap");
                }
            }
            assert!(swapped || receipt > swap, "receipt {receipt} never swapped");
            if receipt > swap {
                // a pane that learns after the swap swaps on its first tick
                let mut after = settled(receipt - 2000);
                after.schedule(4, swap);
                assert_eq!(after.tick_at(receipt).1, Some(4));
            }
        }
    }

    #[test]
    fn a_receiver_after_the_fade_swaps_straight_to_steady() {
        let mut tr = settled(1_000);
        tr.schedule(2, 5_000);
        assert_eq!(tr.pending(), Some(2), "the pending scene prefetches");
        assert_eq!(tr.tick_at(60_000), (1.0, Some(2)));
        assert!(tr.is_idle());
    }

    #[test]
    fn a_newer_schedule_fades_out_from_the_current_level() {
        let mut tr = settled(1_000);
        let fade = tr.fade_ms();
        tr.schedule(1, 10_000);
        assert_eq!(tr.tick_at(10_000).1, Some(1));
        let mid = tr.tick_at(10_000 + fade / 2).0;
        assert!(mid > 0.0 && mid < 1.0);
        // a switch scheduled far ahead mid-fade-in must not pop to full
        tr.schedule(2, 20_000);
        let (f, s) = tr.tick_at(10_000 + fade / 2 + 5);
        assert!(s.is_none());
        assert!(f <= mid + 0.1, "fade jumped from {mid} to {f}");
        assert_eq!(tr.tick_at(20_000).1, Some(2));
    }
}

#[cfg(test)]
mod rapid_tests {
    use super::*;

    #[test]
    fn five_rapid_requests_land_on_five() {
        let mut tr = Transition::new();
        // five requests in rapid succession, mid-fade-in
        for i in 1..=5 {
            tr.tick(0.02);
            tr.request(i);
        }
        let mut last_swap = None;
        for _ in 0..200 {
            let (_, swap) = tr.tick(0.02);
            if swap.is_some() {
                last_swap = swap;
            }
        }
        assert_eq!(last_swap, Some(5), "rapid requests must end on the last one");
        assert!(tr.is_idle());
    }

    #[test]
    fn request_during_fadeout_redirects() {
        let mut tr = Transition::new();
        for _ in 0..20 {
            tr.tick(0.02);
        }
        tr.request(2);
        for _ in 0..5 {
            tr.tick(0.02);
        }
        tr.request(9); // redirect mid-fade-out
        let mut last_swap = None;
        for _ in 0..200 {
            let (_, swap) = tr.tick(0.02);
            if swap.is_some() {
                last_swap = swap;
            }
        }
        assert_eq!(last_swap, Some(9));
    }
}
