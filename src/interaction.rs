//! Shared, timestamped desk input for depth scenes. Only the group leader
//! touches Hyprland IPC; followers evaluate exactly the same easing curve at
//! the same shared frame time. No IPC or easing state lives in the render loop.
use crate::desk::{Desk, RectMm};
use crate::{hypr, link, wallplan::WallPlan};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::{Duration, Instant};

const POINTER_EASE_MS: u64 = 2500;
const FOCUS_EASE_MS: u64 = 900;
const FOCUS_PULSE_MS: u64 = 1200;
// Publish a target before it can affect a frame. Followers poll the shared
// file every 100 ms; this gives them time to see the same segment first.
const INPUT_LEAD_MS: u64 = 400;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ease2 {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub at_ms: u64,
}

impl Ease2 {
    pub fn value(&self, frame_ms: u64, duration_ms: u64) -> [f32; 2] {
        let u = (frame_ms.saturating_sub(self.at_ms) as f32 / duration_ms as f32).clamp(0.0, 1.0);
        let s = u * u * (3.0 - 2.0 * u);
        [self.from[0] + (self.to[0] - self.from[0]) * s,
         self.from[1] + (self.to[1] - self.from[1]) * s]
    }

    fn retarget(&mut self, to: [f32; 2], now_ms: u64, duration_ms: u64) -> bool {
        if (self.to[0] - to[0]).abs() < 0.001 && (self.to[1] - to[1]).abs() < 0.001 { return false; }
        self.from = self.value(now_ms, duration_ms);
        self.to = to;
        self.at_ms = now_ms;
        true
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub pointer: Ease2,
    pub pointer_prev: Ease2,
    pub focus: Ease2,
    pub focus_prev: Ease2,
    pub focus_monitor: String,
    pub focus_epoch_ms: u64,
    pub focus_prev_epoch_ms: u64,
    pub updated_ms: u64,
}

impl Snapshot {
    pub fn fresh(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.updated_ms) < 1500
    }

    pub fn pointer_at(&self, frame_ms: u64) -> [f32; 2] {
        let segment = if frame_ms < self.pointer.at_ms { &self.pointer_prev } else { &self.pointer };
        segment.value(frame_ms, POINTER_EASE_MS)
    }

    pub fn focus_at(&self, frame_ms: u64) -> [f32; 2] {
        let segment = if frame_ms < self.focus.at_ms { &self.focus_prev } else { &self.focus };
        segment.value(frame_ms, FOCUS_EASE_MS)
    }

    pub fn focus_pulse_at(&self, frame_ms: u64) -> f32 {
        let epoch = if frame_ms < self.focus_epoch_ms { self.focus_prev_epoch_ms } else { self.focus_epoch_ms };
        if epoch == 0 { return 0.0; }
        let u = (frame_ms.saturating_sub(epoch) as f32 / FOCUS_PULSE_MS as f32).clamp(0.0, 1.0);
        let s = u * u * (3.0 - 2.0 * u);
        1.0 - s
    }
}

fn comp_point(frame: RectMm, x: f64, y: f64) -> [f32; 2] {
    let short = frame.w.min(frame.h).max(1.0);
    [((x - frame.x - frame.w * 0.5) / short).clamp(-2.0, 2.0) as f32,
     ((frame.y + frame.h * 0.5 - y) / short).clamp(-2.0, 2.0) as f32]
}

fn desk_from_ipc() -> Option<Desk> {
    let json = hypr::socket_json("monitors")?;
    let monitors = hypr::parse_monitors(&json)?;
    Some(Desk::from_hypr(&monitors, &crate::desk::load_desk()))
}

fn frame_for(dir: &Path, desk: &Desk) -> Option<RectMm> {
    WallPlan::load(dir).map(|p| p.frame)
        .or_else(|| desk.monitors.iter().map(|m| m.rect).reduce(|a, b| a.union(&b)))
}

fn initial_focus(desk: &Desk) -> Option<String> {
    let json = hypr::socket_json("activewindow")?;
    let client = hypr::parse_client(&json)?;
    desk.by_id(client.monitor).map(|m| m.mon.name.clone())
}

fn focus_point(desk: &Desk, frame: RectMm, monitor: &str) -> Option<[f32; 2]> {
    let rect = desk.get(monitor)?.rect;
    Some(comp_point(frame, rect.x + rect.w * 0.5, rect.y + rect.h * 0.5))
}

fn pointer_point(desk: &Desk, frame: RectMm) -> Option<[f32; 2]> {
    let pos = hypr::cursorpos_ipc()?;
    let (rect, _) = desk.logical_to_mm([pos[0], pos[1], 1.0, 1.0])?;
    Some(comp_point(frame, rect.x, rect.y))
}

#[derive(Debug, PartialEq)]
enum Event {
    Focus(String),
    Geometry,
    Other,
}

fn parse_event(line: &str) -> Event {
    let Some((name, data)) = line.trim().split_once(">>") else { return Event::Other; };
    match name {
        "focusedmon" | "focusedmonv2" => Event::Focus(data.split(',').next().unwrap_or("").to_string()),
        "monitoradded" | "monitoraddedv2" | "monitorremoved" | "monitorremovedv2"
        | "monitorupdated" | "monitorupdatedv2" | "configreloaded" => Event::Geometry,
        _ => Event::Other,
    }
}

fn store(dir: &Path, state: &Snapshot) {
    let path = dir.join("interaction.json");
    let tmp = dir.join(format!(".interaction.{}", std::process::id()));
    if let Ok(data) = serde_json::to_vec(state) {
        if std::fs::write(&tmp, data).is_ok() { let _ = std::fs::rename(&tmp, path); }
    }
}

fn load(dir: &Path) -> Option<Snapshot> {
    serde_json::from_slice(&std::fs::read(dir.join("interaction.json")).ok()?).ok()
}

pub struct Watcher {
    latest: Arc<Mutex<Snapshot>>,
    leader: Arc<AtomicBool>,
    pointer_enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl Watcher {
    pub fn spawn(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let latest = Arc::new(Mutex::new(Snapshot::default()));
        let leader = Arc::new(AtomicBool::new(false));
        let pointer_enabled = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (out, lead, pointer, halt) = (latest.clone(), leader.clone(), pointer_enabled.clone(), stop.clone());
        let _ = std::thread::Builder::new().name("termpaper-interaction".into()).spawn(move || {
            let mut events: Option<hypr::EventStream> = None;
            let mut desk: Option<Desk> = None;
            let mut frame: Option<RectMm> = None;
            let mut focus_name = String::new();
            let mut last_pointer = Instant::now() - Duration::from_secs(2);
            let mut last_frame = Instant::now() - Duration::from_secs(2);
            let mut last_publish = Instant::now() - Duration::from_secs(2);
            let mut state = Snapshot::default();
            while !halt.load(Ordering::Relaxed) {
                if !lead.load(Ordering::Relaxed) {
                    events = None;
                    desk = None;
                    frame = None;
                    if let Some(next) = load(&dir).filter(|s| s.fresh(link::epoch_now_ms())) {
                        state = next.clone();
                        if let Ok(mut slot) = out.lock() { *slot = next; }
                    }
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                if events.is_none() { events = hypr::EventStream::connect(); }
                if desk.is_none() {
                    desk = desk_from_ipc();
                    frame = desk.as_ref().and_then(|d| frame_for(&dir, d));
                    if let Some(d) = &desk { focus_name = initial_focus(d).unwrap_or_default(); }
                }
                let mut changed = false;
                if let Some(stream) = &mut events {
                    match stream.next_line() {
                        Ok(Some(line)) => match parse_event(&line) {
                            Event::Focus(name) => focus_name = name,
                            Event::Geometry => { desk = None; frame = None; },
                            Event::Other => {},
                        },
                        Ok(None) => {},
                        Err(()) => events = None,
                    }
                } else { std::thread::sleep(Duration::from_millis(100)); }
                if let Some(d) = &desk {
                    if last_frame.elapsed() >= Duration::from_secs(1) {
                        frame = frame_for(&dir, d);
                        last_frame = Instant::now();
                    }
                    if let Some(f) = frame {
                        let now = link::epoch_now_ms();
                        if focus_name != state.focus_monitor && now >= state.focus.at_ms {
                            if let Some(pos) = focus_point(d, f, &focus_name) {
                                state.focus_prev = state.focus;
                                state.focus.retarget(pos, now + INPUT_LEAD_MS, FOCUS_EASE_MS);
                                state.focus_prev_epoch_ms = state.focus_epoch_ms;
                                if !state.focus_monitor.is_empty() { state.focus_epoch_ms = now + INPUT_LEAD_MS; }
                                state.focus_monitor = focus_name.clone();
                                changed = true;
                            }
                        }
                        if pointer.load(Ordering::Relaxed) && last_pointer.elapsed() >= Duration::from_millis(500) {
                            if let Some(pos) = pointer_point(d, f) {
                                let mut next = state.pointer;
                                if next.retarget(pos, now + INPUT_LEAD_MS, POINTER_EASE_MS) {
                                    state.pointer_prev = state.pointer;
                                    state.pointer = next;
                                    changed = true;
                                }
                            }
                            last_pointer = Instant::now();
                        }
                    }
                }
                if changed || last_publish.elapsed() >= Duration::from_millis(500) {
                    state.updated_ms = link::epoch_now_ms();
                    store(&dir, &state);
                    if let Ok(mut slot) = out.lock() { *slot = state.clone(); }
                    last_publish = Instant::now();
                }
            }
        });
        Self { latest, leader, pointer_enabled, stop }
    }

    pub fn set_leader(&self, on: bool) { self.leader.store(on, Ordering::Relaxed); }
    pub fn set_pointer_enabled(&self, on: bool) { self.pointer_enabled.store(on, Ordering::Relaxed); }
    pub fn latest(&self) -> Snapshot { self.latest.lock().map(|s| s.clone()).unwrap_or_default() }
}

impl Drop for Watcher {
    fn drop(&mut self) { self.stop.store(true, Ordering::Relaxed); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_point_maps_to_composition_space() {
        let f = RectMm { x: 10.0, y: 20.0, w: 400.0, h: 200.0 };
        assert_eq!(comp_point(f, 210.0, 120.0), [0.0, 0.0]);
        assert_eq!(comp_point(f, 410.0, 20.0), [1.0, 0.5]);
    }

    #[test]
    fn interpolation_is_shared_and_stops_exactly() {
        let mut e = Ease2::default();
        assert!(e.retarget([0.8, -0.4], 1000, POINTER_EASE_MS));
        assert_eq!(e.value(1000, POINTER_EASE_MS), [0.0, 0.0]);
        let a = e.value(2000, POINTER_EASE_MS);
        let serialized = serde_json::to_string(&e).unwrap();
        let follower: Ease2 = serde_json::from_str(&serialized).unwrap();
        assert_eq!(a, follower.value(2000, POINTER_EASE_MS));
        assert_eq!(e.value(3500, POINTER_EASE_MS), [0.8, -0.4]);
        assert_eq!(e.value(5000, POINTER_EASE_MS), [0.8, -0.4]);
        assert!(e.retarget([0.4, 0.6], 2000, POINTER_EASE_MS));
        assert_eq!(e.from, a);
    }

    #[test]
    fn newly_published_input_preserves_the_previous_curve_until_effective() {
        let mut old = Snapshot::default();
        old.pointer.retarget([0.8, -0.4], 1000, POINTER_EASE_MS);
        old.focus.retarget([0.5, 0.0], 1000, FOCUS_EASE_MS);
        old.focus_epoch_ms = 1000;
        let mut new = old.clone();
        new.pointer_prev = old.pointer;
        new.pointer.retarget([-0.2, 0.7], 2000 + INPUT_LEAD_MS, POINTER_EASE_MS);
        new.focus_prev = old.focus;
        new.focus.retarget([-0.5, 0.0], 2000 + INPUT_LEAD_MS, FOCUS_EASE_MS);
        new.focus_prev_epoch_ms = old.focus_epoch_ms;
        new.focus_epoch_ms = 2000 + INPUT_LEAD_MS;
        let follower: Snapshot = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
        for ms in [2000, 2100, 2399] {
            assert_eq!(old.pointer_at(ms), follower.pointer_at(ms));
            assert_eq!(old.focus_at(ms), follower.focus_at(ms));
            assert_eq!(old.focus_pulse_at(ms), follower.focus_pulse_at(ms));
        }
        assert_eq!(new.pointer_at(2400), old.pointer_at(2400));
        assert_eq!(new.focus_at(2400), old.focus_at(2400));
    }

    #[test]
    fn focus_events_are_parsed_without_title_or_workspace() {
        assert_eq!(parse_event("focusedmon>>DP-1,4\n"), Event::Focus("DP-1".into()));
        assert_eq!(parse_event("focusedmonv2>>DP-1,4\n"), Event::Focus("DP-1".into()));
        assert_eq!(parse_event("monitoraddedv2>>1,HDMI-A-1,desc\n"), Event::Geometry);
    }
}
