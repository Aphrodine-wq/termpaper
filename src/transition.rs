//! Scene transition state machine: fade the outgoing scene to black over
//! ~0.25s, swap, fade the new one in. Cheap — a single luminance scale.

use crate::canvas::ease_smooth;

pub const FADE_SECS: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    In,
    Steady,
    Out,
}

pub struct Transition {
    phase: Phase,
    t: f32,
    pending: Option<usize>,
    fade_secs: f32,
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
        }
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
        self.pending
    }

    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Steady && self.pending.is_none()
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
