//! Adaptive supersampling for Studio shader scenes.
//!
//! The objective is a GPU-time budget per frame, not an fps target: a
//! wallpaper shares the GPU with the compositor, and a long compute
//! submission delays the desktop's own frame. The governor only ever moves
//! the sample count — which changes anti-aliasing, never structure — so panes
//! of a wall that settle on different counts still line up at the seams.
//!
//! Kitty pixel modes add a second, coarser axis: when one sample per pixel
//! is still over budget, the frame is rendered at a coarser rung (4x8 → 2x4
//! → 1x2 px per cell) and kitty scales it over the same cells. Resolution
//! comes back before sample counts do: a sharp frame beats a smooth edge.

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
    /// Coarser resolution rungs taken (kitty modes only), and how many exist.
    rung: u32,
    max_rung: u32,
    /// The smoothed one-sample cost last seen at each rung: a finer rung is
    /// retried when it is known to fit, or probed once in a while.
    rung_cost: [Option<f32>; 3],
    /// Frames to wait before probing a finer rung whose cost is unknown or
    /// was over budget.
    probe_wait: u32,
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
            rung: 0,
            max_rung: 0,
            rung_cost: [None; 3],
            probe_wait: 0,
        }
    }

    /// New scene or quality ceiling: keep the learned cost only if the ceiling
    /// did not change the meaning of the ladder position.
    pub fn reset(&mut self, max_spp: u32) {
        let (rung, max_rung) = (self.rung, self.max_rung);
        *self = Self::new(self.budget_ms, max_spp);
        self.rung = rung.min(max_rung);
        self.max_rung = max_rung;
    }

    /// How many coarser rungs the output mode allows (0 for cell modes).
    pub fn set_max_rung(&mut self, max_rung: u32) {
        if self.max_rung != max_rung {
            self.max_rung = max_rung;
            self.rung = self.rung.min(max_rung);
            self.per_sample = None;
        }
    }

    /// The resolution rung to render at: 0 = the mode's own pixels.
    pub fn rung(&self) -> u32 {
        self.rung
    }

    fn coarser(&mut self) {
        if let Some(e) = self.per_sample {
            self.rung_cost[self.rung as usize] = Some(e);
        }
        // the finer rung failed: leave it alone for a while (~5 s)
        self.probe_wait = 300;
        self.rung += 1;
        self.idx = 0;
        // a quarter of the pixels: relearn the cost
        self.per_sample = None;
        self.over = 0;
        self.under = 0;
        self.backpressure = 0;
        self.cooldown = 20;
    }

    fn finer(&mut self) {
        if let Some(e) = self.per_sample {
            self.rung_cost[self.rung as usize] = Some(e);
        }
        self.rung -= 1;
        self.idx = 0;
        self.per_sample = None;
        self.over = 0;
        self.under = 0;
        self.cooldown = 30;
    }

    pub fn set_budget(&mut self, budget_ms: f32) {
        self.budget_ms = budget_ms.max(0.25);
    }

    pub fn spp(&self) -> u32 {
        // at a coarser rung the headroom goes to resolution, not samples
        if self.rung > 0 {
            return 1;
        }
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
        if self.backpressure >= 2 && self.idx == 0 && self.rung < self.max_rung {
            self.coarser();
            return;
        }
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
        if self.rung > 0 {
            // one sample at this rung: over budget steps coarser still;
            // comfortably under (a finer rung costs ~4x) steps back
            if e > self.budget_ms * 1.1 {
                self.over += 1;
                self.under = 0;
                if self.over >= 3 && self.rung < self.max_rung {
                    self.coarser();
                }
            } else if e < self.budget_ms * 0.9 {
                // under budget here: go finer when the finer rung is known to
                // fit (a fixed per-frame cost means it is not simply 4x), or
                // probe it once the wait is over
                self.under += 1;
                self.over = 0;
                self.probe_wait = self.probe_wait.saturating_sub(1);
                let known = self.rung_cost[(self.rung - 1) as usize];
                let fits = match known {
                    Some(c) => c < self.budget_ms * 0.9,
                    None => e * 4.0 < self.budget_ms * 0.9,
                };
                if self.under >= 30 && (fits || self.probe_wait == 0) {
                    self.finer();
                }
            } else {
                self.over = 0;
                self.under = 0;
            }
            return;
        }
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
            } else if self.over >= 3 && self.rung < self.max_rung {
                // one sample is still too much: fewer pixels
                self.coarser();
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
    fn kitty_modes_trade_resolution_before_giving_up() {
        let mut g = Governor::new(3.0, 4);
        g.set_max_rung(2);
        // 10 ms a sample at full resolution: one rung down (a quarter of
        // the pixels, 2.5 ms) fits the 3 ms budget
        for _ in 0..400 {
            let s = g.spp();
            let r = g.rung();
            g.observe(10.0 / f32::powi(4.0, r as i32) * s as f32, s);
        }
        assert_eq!(g.rung(), 1);
        assert_eq!(g.spp(), 1);
        // 40 ms a sample: the coarsest rung, still at one sample
        for _ in 0..400 {
            let s = g.spp();
            let r = g.rung();
            g.observe(40.0 / f32::powi(4.0, r as i32) * s as f32, s);
        }
        assert_eq!(g.rung(), 2);
        assert_eq!(g.spp(), 1);
        // the scene gets cheap again: resolution comes back, then samples
        for _ in 0..600 {
            let s = g.spp();
            let r = g.rung();
            g.observe(0.05 / f32::powi(4.0, r as i32) * s as f32, s);
        }
        assert_eq!(g.rung(), 0);
        assert!(g.spp() > 1);
    }

    #[test]
    fn a_fixed_frame_cost_does_not_pin_the_coarsest_rung() {
        // 1 ms of fixed work plus 0.1 ms per thousand pixels: at full
        // resolution (16 units) 2.6 ms fits a 3 ms budget; the naive 4x rule
        // from the rung below would say it does not
        let mut g = Governor::new(3.0, 4);
        g.set_max_rung(2);
        // a heavy spell first drives it coarse
        for _ in 0..400 {
            let s = g.spp();
            let px = 16.0 / f32::powi(4.0, g.rung() as i32);
            g.observe((1.0 + 1.0 * px) * s as f32, s);
        }
        assert!(g.rung() > 0);
        // then the probes find their way back up
        for _ in 0..3000 {
            let s = g.spp();
            let px = 16.0 / f32::powi(4.0, g.rung() as i32);
            g.observe((1.0 + 0.1 * px) * s as f32, s);
        }
        assert_eq!(g.rung(), 0);
    }

    #[test]
    fn cell_modes_never_change_resolution() {
        let mut g = Governor::new(3.0, 4);
        run(&mut g, 10.0, 200);
        assert_eq!(g.rung(), 0);
        assert_eq!(g.spp(), 1);
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
