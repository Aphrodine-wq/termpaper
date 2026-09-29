//! How fast the terminal takes frames. [`Meter`] sits between the frame
//! buffer and stdout and counts the bytes and the time spent blocked on the
//! terminal; [`FpsAdapter`] turns that into a frame-rate cap (Adapt FPS): a
//! preset lower while writing takes most of the time between frames, and
//! back up once it has been easy for a while.

use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A writer that counts what passes through it and how long it blocked,
/// readable through the [`Reading`] it hands out (the writer itself ends
/// up deep inside the terminal backend).
pub struct Meter<W> {
    inner: W,
    reading: Reading,
}

/// Bytes written and nanoseconds blocked, shared with a [`Meter`].
#[derive(Clone, Default)]
pub struct Reading(Arc<(AtomicU64, AtomicU64)>);

impl Reading {
    /// Bytes written and time blocked since the last call.
    pub fn take(&self) -> (u64, Duration) {
        let bytes = self.0 .0.swap(0, Ordering::Relaxed);
        let ns = self.0 .1.swap(0, Ordering::Relaxed);
        (bytes, Duration::from_nanos(ns))
    }

    fn add(&self, bytes: u64, busy: Duration) {
        self.0 .0.fetch_add(bytes, Ordering::Relaxed);
        self.0 .1.fetch_add(busy.as_nanos() as u64, Ordering::Relaxed);
    }
}

impl<W> Meter<W> {
    pub fn new(inner: W) -> (Self, Reading) {
        let reading = Reading::default();
        (Meter { inner, reading: reading.clone() }, reading)
    }
}

impl<W: Write> Write for Meter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let t = Instant::now();
        let n = self.inner.write(buf)?;
        self.reading.add(n as u64, t.elapsed());
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        let t = Instant::now();
        let r = self.inner.flush();
        self.reading.add(0, t.elapsed());
        r
    }
}

/// Share of the time spent writing above which the terminal is falling
/// behind, and below which it has room.
const BEHIND: f32 = 0.5;
const EASY: f32 = 0.15;
/// Easy seconds in a row before the rate steps back up.
const CALM_WINDOWS: u32 = 5;
/// Never adapt below this.
const FLOOR_FPS: u32 = 10;

/// Adapt FPS, fed once a frame.
pub struct FpsAdapter {
    since: Instant,
    busy: Duration,
    calm: u32,
    /// the rate the terminal is keeping up with, when lower than asked
    pub cap: Option<u32>,
}

impl FpsAdapter {
    pub fn new(now: Instant) -> Self {
        FpsAdapter { since: now, busy: Duration::ZERO, calm: 0, cap: None }
    }

    /// Account one frame's write time. Once a second, compare the time
    /// spent writing with the time gone by and move the cap a step through
    /// `presets` (ascending). True when the cap changed.
    pub fn observe(&mut self, now: Instant, busy: Duration, asked_fps: u32, presets: &[u32]) -> bool {
        self.busy += busy;
        let window = now.saturating_duration_since(self.since);
        if window < Duration::from_secs(1) {
            return false;
        }
        let share = self.busy.as_secs_f32() / window.as_secs_f32();
        self.since = now;
        self.busy = Duration::ZERO;
        let current = self.cap.map_or(asked_fps, |c| c.min(asked_fps));
        if share > BEHIND {
            self.calm = 0;
            let lower = presets.iter().rev().copied().find(|&p| p < current).unwrap_or(current).max(FLOOR_FPS);
            if lower < current {
                self.cap = Some(lower);
                return true;
            }
        } else if share < EASY {
            self.calm += 1;
            if let (true, Some(c)) = (self.calm >= CALM_WINDOWS, self.cap) {
                self.calm = 0;
                self.cap = presets.iter().copied().find(|&p| p > c).filter(|&p| p < asked_fps);
                return true;
            }
        } else {
            self.calm = 0;
        }
        false
    }

    /// Forget the cap (Adapt FPS turned off, or the rate changed).
    pub fn reset(&mut self, now: Instant) {
        *self = FpsAdapter::new(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRESETS: &[u32] = &[10, 24, 30, 60, 90, 120, 144, 165, 240];

    #[test]
    fn meter_counts_bytes_and_hands_them_over_once() {
        let (mut m, reading) = Meter::new(Vec::new());
        m.write_all(b"hello").unwrap();
        m.flush().unwrap();
        let (bytes, _) = reading.take();
        assert_eq!(bytes, 5);
        assert_eq!(reading.take().0, 0);
    }

    /// Feed `secs` seconds of frames at `fps`, each taking `share` of its
    /// period to write.
    fn run(a: &mut FpsAdapter, t: &mut Instant, secs: u32, fps: u32, share: f32, asked: u32) -> Vec<Option<u32>> {
        let period = Duration::from_secs_f32(1.0 / fps as f32);
        let mut caps = Vec::new();
        for _ in 0..secs * fps {
            *t += period;
            if a.observe(*t, period.mul_f32(share), asked, PRESETS) {
                caps.push(a.cap);
            }
        }
        caps
    }

    #[test]
    fn steps_down_while_behind_and_back_up_when_easy() {
        let mut t = Instant::now();
        let mut a = FpsAdapter::new(t);
        // a terminal that takes 80% of every frame: down a preset a second
        let caps = run(&mut a, &mut t, 3, 120, 0.8, 120);
        assert_eq!(caps, vec![Some(90), Some(60), Some(30)]);
        // comfortable again: back up one preset per five easy seconds
        let caps = run(&mut a, &mut t, 11, 30, 0.05, 120);
        assert_eq!(caps, vec![Some(60), Some(90)]);
        let caps = run(&mut a, &mut t, 5, 90, 0.05, 120);
        assert_eq!(caps, vec![None], "all the way back: no cap");
        // never below the floor
        let caps = run(&mut a, &mut t, 12, 120, 0.95, 120);
        assert_eq!(caps.last(), Some(&Some(10)));
        assert!(!run(&mut a, &mut t, 2, 10, 0.95, 120).iter().any(|c| c.is_some_and(|c| c < 10)));
    }

    #[test]
    fn a_middling_terminal_is_left_alone() {
        let mut t = Instant::now();
        let mut a = FpsAdapter::new(t);
        assert!(run(&mut a, &mut t, 10, 60, 0.3, 60).is_empty());
        assert_eq!(a.cap, None);
    }
}
