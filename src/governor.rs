//! Adaptive supersampling for Studio shader scenes.
//!
//! The objective is a GPU-time budget per frame, not an fps target: a
//! wallpaper shares the GPU with the compositor, and a long compute
//! submission delays the desktop's own frame. The governor only ever moves
//! the sample count — which changes anti-aliasing, never structure — so panes
//! of a wall that settle on different counts still line up at the seams.

/// Sample counts the governor steps between.
const LADDER: [u32; 9] = [1, 2, 3, 4, 6, 8, 12, 16, 24];

pub struct Governor {
    idx: usize,
    max_idx: usize,
    budget_ms: f32,
    /// Smoothed GPU cost of one sample per pixel, in ms.
    per_sample: Option<f32>,
    over: u32,
    under: u32,
    backpressure: u32,
    cooldown: u32,
}

fn rung_at_most(spp: u32) -> usize {
    LADDER.iter().rposition(|&s| s <= spp.max(1)).unwrap_or(0)
}

impl Governor {
    pub fn new(budget_ms: f32, max_spp: u32) -> Self {
        let max_idx = rung_at_most(max_spp);
        Self {
            // start in the middle: fast enough to never stall the first frames
            idx: max_idx / 2,
            max_idx,
            budget_ms: budget_ms.max(0.25),
            per_sample: None,
            over: 0,
            under: 0,
            backpressure: 0,
            cooldown: 10,
        }
    }

    /// New scene or quality ceiling: keep the learned cost only if the ceiling
    /// did not change the meaning of the ladder position.
    pub fn reset(&mut self, max_spp: u32) {
        *self = Self::new(self.budget_ms, max_spp);
    }

    pub fn set_budget(&mut self, budget_ms: f32) {
        self.budget_ms = budget_ms.max(0.25);
    }

    pub fn spp(&self) -> u32 {
        LADDER[self.idx.min(self.max_idx)]
    }

    /// A measured scene pass: `ms` of GPU time at `spp` samples per pixel.
    pub fn observe(&mut self, ms: f32, spp: u32) {
        if !ms.is_finite() || ms <= 0.0 {
            return;
        }
        let one = ms / spp.max(1) as f32;
        self.per_sample = Some(match self.per_sample {
            Some(e) => e + (one - e) * 0.15,
            None => one,
        });
        self.step();
    }

    /// The readback ring was full: the GPU (or the terminal) is behind.
    pub fn on_backpressure(&mut self) {
        self.backpressure += 1;
        if self.backpressure >= 2 && self.idx > 0 {
            self.idx -= 1;
            self.backpressure = 0;
            self.cooldown = 20;
            self.over = 0;
            self.under = 0;
        }
    }

    fn step(&mut self) {
        if self.cooldown > 0 {
            self.cooldown -= 1;
            return;
        }
        let Some(e) = self.per_sample else { return };
        let cost = |i: usize| e * LADDER[i] as f32;
        if cost(self.idx) > self.budget_ms * 1.1 {
            self.over += 1;
            self.under = 0;
            if self.over >= 3 && self.idx > 0 {
                // jump straight to the largest rung that fits
                while self.idx > 0 && cost(self.idx) > self.budget_ms {
                    self.idx -= 1;
                }
                self.over = 0;
                self.cooldown = 20;
            }
        } else if self.idx < self.max_idx && cost(self.idx + 1) < self.budget_ms * 0.75 {
            self.under += 1;
            self.over = 0;
            if self.under >= 30 {
                self.idx += 1;
                self.under = 0;
                self.cooldown = 20;
            }
        } else {
            self.over = 0;
            self.under = 0;
        }
        self.backpressure = self.backpressure.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Governor, per_sample_ms: f32, frames: usize) {
        for _ in 0..frames {
            let s = g.spp();
            g.observe(per_sample_ms * s as f32, s);
        }
    }

    #[test]
    fn steps_down_when_over_budget() {
        let mut g = Governor::new(3.0, 16);
        run(&mut g, 2.0, 200);
        assert_eq!(g.spp(), 1, "2 ms per sample only fits one sample in 3 ms");
    }

    #[test]
    fn climbs_to_the_ceiling_when_cheap() {
        let mut g = Governor::new(3.0, 12);
        run(&mut g, 0.01, 2000);
        assert_eq!(g.spp(), 12);
    }

    #[test]
    fn settles_at_the_largest_rung_that_fits() {
        let mut g = Governor::new(3.0, 24);
        run(&mut g, 0.4, 3000);
        // 6 samples = 2.4 ms fits; 8 = 3.2 ms does not
        assert_eq!(g.spp(), 6);
    }

    #[test]
    fn never_exceeds_the_ceiling_and_backs_off_on_backpressure() {
        let mut g = Governor::new(100.0, 4);
        run(&mut g, 0.001, 2000);
        assert_eq!(g.spp(), 4);
        g.on_backpressure();
        g.on_backpressure();
        assert!(g.spp() < 4);
    }
}
