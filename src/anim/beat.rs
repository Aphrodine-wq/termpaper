//! A looping, deterministic timeline with named beats.
//!
//! Scenes advance it with the same `dt` they simulate with, ask where they
//! are inside a beat (`phase`), take an eased envelope from it (`env`), and
//! draw next-cycle variation from their own RNG when `step` reports a wrap.
use crate::canvas::{ease_in_out, ease_out};

#[derive(Clone, Copy, Debug)]
pub struct Beat {
    pub name: &'static str,
    pub at: f32,
    pub dur: f32,
}

#[derive(Clone, Debug)]
pub struct Timeline {
    pub len: f32,
    pub t: f32,
    pub cycle: u32,
    beats: Vec<Beat>,
}

impl Timeline {
    pub fn new(len: f32) -> Self {
        Timeline {
            len: len.max(0.01),
            t: 0.0,
            cycle: 0,
            beats: Vec::new(),
        }
    }

    /// Builder: add a beat starting at `at` seconds lasting `dur`.
    pub fn beat(mut self, name: &'static str, at: f32, dur: f32) -> Self {
        self.beats.push(Beat { name, at, dur });
        self
    }

    /// Advance; returns `Some(cycle)` on the frame the loop wraps.
    pub fn step(&mut self, dt: f32) -> Option<u32> {
        self.t += dt.max(0.0);
        if self.t >= self.len {
            self.t -= self.len;
            if self.t >= self.len {
                self.t = 0.0;
            }
            self.cycle = self.cycle.wrapping_add(1);
            Some(self.cycle)
        } else {
            None
        }
    }

    fn find(&self, name: &str) -> Option<&Beat> {
        self.beats.iter().find(|b| b.name == name)
    }

    /// Seconds since the beat started this cycle (negative before it).
    pub fn since(&self, name: &str) -> f32 {
        self.find(name).map(|b| self.t - b.at).unwrap_or(f32::MIN)
    }

    /// Progress 0..1 while inside the beat, else `None`.
    pub fn phase(&self, name: &str) -> Option<f32> {
        let b = self.find(name)?;
        let s = self.t - b.at;
        if s >= 0.0 && s < b.dur {
            Some(if b.dur > 0.0 { s / b.dur } else { 1.0 })
        } else {
            None
        }
    }

    /// Whether the beat is active this frame.
    pub fn active(&self, name: &str) -> bool {
        self.phase(name).is_some()
    }

    /// Eased 0..1 envelope: rises over `attack` seconds from the beat start,
    /// holds, then decays over `release` seconds after the beat ends.
    pub fn env(&self, name: &str, attack: f32, release: f32) -> f32 {
        let Some(b) = self.find(name) else {
            return 0.0;
        };
        let s = self.t - b.at;
        if s < 0.0 {
            return 0.0;
        }
        if s < b.dur {
            if attack <= 0.0 {
                1.0
            } else {
                ease_in_out((s / attack).min(1.0))
            }
        } else {
            let r = s - b.dur;
            if release <= 0.0 || r >= release {
                0.0
            } else {
                1.0 - ease_out(r / release)
            }
        }
    }

    /// Move a beat's start by `dt` seconds (per-cycle jitter), clamped inside
    /// the loop.
    pub fn shift(&mut self, name: &str, dt: f32) {
        let len = self.len;
        if let Some(b) = self.beats.iter_mut().find(|b| b.name == name) {
            b.at = (b.at + dt).clamp(0.0, (len - b.dur).max(0.0));
        }
    }

    /// Set a beat's start absolutely.
    pub fn set_at(&mut self, name: &str, at: f32) {
        let len = self.len;
        if let Some(b) = self.beats.iter_mut().find(|b| b.name == name) {
            b.at = at.clamp(0.0, (len - b.dur).max(0.0));
        }
    }

    /// Start of a beat this cycle.
    pub fn at(&self, name: &str) -> f32 {
        self.find(name).map(|b| b.at).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_and_reports_cycles() {
        let mut tl = Timeline::new(2.0).beat("hit", 1.0, 0.5);
        assert_eq!(tl.step(0.5), None);
        assert_eq!(tl.phase("hit"), None);
        tl.step(0.6);
        assert!((tl.phase("hit").unwrap() - 0.2).abs() < 1e-5);
        assert!(tl.active("hit"));
        assert_eq!(tl.step(1.0), Some(1));
        assert!((tl.t - 0.1).abs() < 1e-5);
        assert_eq!(tl.cycle, 1);
    }

    #[test]
    fn envelope_attack_hold_release() {
        let mut tl = Timeline::new(10.0).beat("flash", 2.0, 1.0);
        assert_eq!(tl.env("flash", 0.2, 0.5), 0.0);
        tl.step(2.1);
        let rising = tl.env("flash", 0.2, 0.5);
        assert!(rising > 0.0 && rising < 1.0);
        tl.step(0.5);
        assert_eq!(tl.env("flash", 0.2, 0.5), 1.0);
        tl.step(0.6);
        let falling = tl.env("flash", 0.2, 0.5);
        assert!(falling > 0.0 && falling < 1.0);
        tl.step(1.0);
        assert_eq!(tl.env("flash", 0.2, 0.5), 0.0);
        assert!(tl.since("flash") > 2.0);
    }

    #[test]
    fn shift_keeps_beats_inside_the_loop() {
        let mut tl = Timeline::new(10.0).beat("late", 9.0, 2.0);
        tl.shift("late", 5.0);
        assert_eq!(tl.at("late"), 8.0);
        tl.shift("late", -20.0);
        assert_eq!(tl.at("late"), 0.0);
        tl.set_at("late", 4.0);
        assert_eq!(tl.at("late"), 4.0);
    }
}
