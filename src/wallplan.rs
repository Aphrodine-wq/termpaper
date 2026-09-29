//! The wall plan: one layout for every pane of a linked group.
//!
//! Before, each pane guessed the wall from peers' window geometry on its own
//! 2 s timer, counting in cells — panes with different fonts disagreed, a
//! portrait monitor was left out, and panes could hold different layouts for
//! seconds. Now one pane (the leader) computes the whole wall from a single
//! compositor snapshot plus the physical desk, and every pane adopts the
//! published plan verbatim.
//!
//! Each pane gets two mappings:
//! - `comp`: its cell grid in Studio composition space (continuous, exact in
//!   millimetres across monitors and fonts);
//! - `classic`: its cell grid in the shared virtual canvas Classic scenes
//!   draw, at one common cell pitch.
use crate::desk::{comp_view, CompView, Desk, DeskConfig, RectMm};
use crate::hypr::{window_for, HyprClient};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// What a pane tells the leader about itself (from its registry entry).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaneGeom {
    pub pid: u32,
    /// pid and ancestors: the compositor knows the terminal's pid, not ours
    pub ancestors: Vec<u32>,
    pub cols: u16,
    pub rows: u16,
    /// measured cell size in logical (layout) px, when the terminal
    /// reports it
    pub cell: Option<(f64, f64)>,
    /// padding between the window edge and the cell grid, logical px
    /// (left, top)
    pub pad: (f64, f64),
    /// the terminal centres the grid in the window (leftover split evenly)
    /// instead of leaving it at the right and bottom
    pub centered: bool,
}

/// Where one pane sits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanePlan {
    pub pid: u32,
    pub monitor: String,
    /// the cell grid on the desk
    pub content: RectMm,
    pub portrait: bool,
    /// composition space of the cell grid: origin at the grid's top-left
    /// corner, `step` per CELL (divide by pixels-per-cell for pixels)
    pub comp: CompView,
    /// Classic canvas: origin in virtual cells, and virtual cells per pane
    /// cell; None when this pane is not part of the Classic wall
    pub classic: Option<([f64; 2], [f64; 2])>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WallPlan {
    /// bumped by the leader on every real layout change
    pub rev: u64,
    pub leader: u32,
    pub computed_ms: u64,
    pub frame: RectMm,
    /// virtual Classic canvas in cells
    pub classic_cells: (usize, usize),
    pub panes: Vec<PanePlan>,
    /// monitor name, desk rect, mm per logical px — for `termpaper desk`
    pub desk: Vec<(String, RectMm, f64)>,
}

/// Classic scenes simulate the whole virtual canvas in every pane: cap it.
pub const CLASSIC_BUDGET_CELLS: usize = 60_000;

impl WallPlan {
    pub fn pane(&self, pid: u32) -> Option<&PanePlan> {
        self.panes.iter().find(|p| p.pid == pid)
    }

    /// Same layout, ignoring bookkeeping (rev, leader, time).
    pub fn same_layout(&self, o: &WallPlan) -> bool {
        self.frame == o.frame && self.panes == o.panes && self.classic_cells == o.classic_cells
    }

    pub fn load(dir: &Path) -> Option<WallPlan> {
        serde_json::from_str(&std::fs::read_to_string(dir.join("wall.json")).ok()?).ok()
    }

    pub fn store(&self, dir: &Path) -> std::io::Result<()> {
        let tmp = dir.join(format!(".wall.json.{}", std::process::id()));
        std::fs::write(&tmp, serde_json::to_vec(self).map_err(std::io::Error::other)?)?;
        std::fs::rename(tmp, dir.join("wall.json"))
    }
}

/// The cell grid of a pane in logical px of the compositor layout.
fn content_logical(win: &HyprClient, g: &PaneGeom) -> [f64; 4] {
    content_rect([win.at[0] as f64, win.at[1] as f64, win.size[0] as f64, win.size[1] as f64], g)
}

/// A pane's cell grid, in logical px, inside its window `[x, y, w, h]`.
pub fn content_rect(win: [f64; 4], g: &PaneGeom) -> [f64; 4] {
    let (wx, wy) = (win[0], win[1]);
    let (ww, wh) = (win[2], win[3]);
    let (cols, rows) = (g.cols.max(1) as f64, g.rows.max(1) as f64);
    let (cw, ch) = match g.cell {
        Some((w, h)) if w > 0.0 && h > 0.0 => (w, h),
        // estimate: the window minus padding, split evenly
        _ => ((ww - 2.0 * g.pad.0).max(1.0) / cols, (wh - 2.0 * g.pad.1).max(1.0) / rows),
    };
    let (gw, gh) = (cw * cols, ch * rows);
    if g.centered {
        [wx + ((ww - gw) / 2.0).max(0.0), wy + ((wh - gh) / 2.0).max(0.0), gw, gh]
    } else {
        [wx + g.pad.0, wy + g.pad.1, gw, gh]
    }
}

/// Compute the plan for the panes the compositor can place.
pub fn plan(desk: &Desk, cfg: &DeskConfig, clients: &[HyprClient], panes: &[PaneGeom], leader: u32) -> Option<WallPlan> {
    struct Placed<'a> {
        g: &'a PaneGeom,
        monitor: String,
        content: RectMm,
        portrait: bool,
    }
    let mut placed = Vec::new();
    for g in panes {
        let Some(win) = window_for(clients, &g.ancestors) else { continue };
        let Some(dm) = desk
            .by_id(win.monitor)
            .or_else(|| desk.at_logical(win.at[0] as f64 + 1.0, win.at[1] as f64 + 1.0))
        else {
            continue;
        };
        let _ = dm;
        let r = content_logical(win, g);
        let Some((content, dm)) = desk.logical_to_mm(r) else { continue };
        placed.push(Placed { g, monitor: dm.mon.name.clone(), content, portrait: dm.mon.portrait() });
    }
    if placed.is_empty() {
        return None;
    }
    placed.sort_by_key(|p| p.g.pid);
    let rects: Vec<(RectMm, bool)> = placed.iter().map(|p| (p.content, p.portrait)).collect();
    let frame = desk.frame(cfg, &rects)?;

    // Classic canvas: which panes join, and at what pitch
    let joins = |p: &Placed| match cfg.portrait.as_str() {
        "separate" => !p.portrait,
        _ => true,
    };
    let mut region: Option<RectMm> = None;
    let mut pitch = (f64::MAX, f64::MAX);
    for p in placed.iter().filter(|p| joins(p)) {
        let mut r = p.content;
        if cfg.portrait == "band" && p.portrait {
            // only the frame's height of a portrait pane
            let top = r.y.max(frame.y);
            let bottom = r.bottom().min(frame.bottom());
            r = RectMm { y: top, h: (bottom - top).max(0.0), ..r };
        }
        region = Some(region.map_or(r, |a| a.union(&r)));
        pitch.0 = pitch.0.min(p.content.w / p.g.cols.max(1) as f64);
        pitch.1 = pitch.1.min(p.content.h / p.g.rows.max(1) as f64);
    }
    let mut classic_cells = (0, 0);
    let mut classic_map = std::collections::HashMap::new();
    if let Some(reg) = region {
        // finest pitch first; coarsen until the canvas fits the budget
        let mut k = 1.0;
        loop {
            let (cw, ch) = (pitch.0 * k, pitch.1 * k);
            let cells = ((reg.w / cw).ceil() as usize, (reg.h / ch).ceil() as usize);
            if cells.0 * cells.1 <= CLASSIC_BUDGET_CELLS || k > 16.0 {
                classic_cells = (cells.0.max(1), cells.1.max(1));
                for p in placed.iter().filter(|p| joins(p)) {
                    let origin = [(p.content.x - reg.x) / cw, (p.content.y - reg.y) / ch];
                    let step = [
                        p.content.w / p.g.cols.max(1) as f64 / cw,
                        p.content.h / p.g.rows.max(1) as f64 / ch,
                    ];
                    classic_map.insert(p.g.pid, (origin, step));
                }
                break;
            }
            k *= 1.15;
        }
    }

    let panes = placed
        .iter()
        .map(|p| PanePlan {
            pid: p.g.pid,
            monitor: p.monitor.clone(),
            content: p.content,
            portrait: p.portrait,
            comp: comp_view(frame, p.content, (p.g.cols.max(1) as usize, p.g.rows.max(1) as usize)),
            classic: classic_map.get(&p.g.pid).copied(),
        })
        .collect();
    Some(WallPlan {
        rev: 0,
        leader,
        computed_ms: 0,
        frame,
        classic_cells,
        panes,
        desk: desk.monitors.iter().map(|d| (d.mon.name.clone(), d.rect, d.mm_per_px)).collect(),
    })
}

/// Leader side: recompute the plan from a fresh compositor snapshot twice a
/// second and publish it when the layout really changed (after seeing the
/// same result twice, so a window mid-drag does not thrash every pane).
pub struct Planner {
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Planner {
    /// `panes` returns the group's current pane geometry (from the registry).
    pub fn spawn<F>(dir: std::path::PathBuf, leader: u32, panes: F) -> Self
    where
        F: Fn() -> Vec<PaneGeom> + Send + 'static,
    {
        use std::sync::atomic::Ordering;
        let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = active.clone();
        let quit = stop.clone();
        let _ = std::thread::Builder::new().name("termpaper-planner".into()).spawn(move || {
            let mut candidate: Option<WallPlan> = None;
            let mut published: Option<WallPlan> = WallPlan::load(&dir);
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if quit.load(Ordering::Acquire) {
                    return;
                }
                if !flag.load(Ordering::Acquire) {
                    candidate = None;
                    continue;
                }
                let (Some(mons), Some(clients)) = (crate::hypr::monitors(), crate::hypr::clients()) else {
                    continue;
                };
                let cfg = crate::desk::load_desk();
                let desk = Desk::from_hypr(&mons, &cfg);
                let Some(next) = plan(&desk, &cfg, &clients, &panes(), leader) else { continue };
                let stable = candidate.as_ref().is_some_and(|c| c.same_layout(&next));
                // compare with what is actually on disk: another leader may
                // have published while this one was idle
                let on_disk = WallPlan::load(&dir);
                let changed = !on_disk.as_ref().is_some_and(|p| p.same_layout(&next));
                if stable && changed {
                    let disk_rev = on_disk.as_ref().map(|p| p.rev).unwrap_or(0);
                    let mut out = next.clone();
                    out.rev = disk_rev.max(published.as_ref().map(|p| p.rev).unwrap_or(0)) + 1;
                    out.computed_ms = crate::link::epoch_now_ms();
                    if out.store(&dir).is_ok() {
                        published = Some(out);
                    }
                }
                candidate = Some(next);
            }
        });
        Planner { active, stop }
    }

    /// Only the leader plans; followers keep the thread idle.
    pub fn set_active(&self, on: bool) {
        self.active.store(on, std::sync::atomic::Ordering::Release);
    }
}

impl Drop for Planner {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
    }
}

/// Follower side: pick up a newly published plan with one `stat` per call.
pub struct PlanWatcher {
    dir: std::path::PathBuf,
    stamp: Option<std::time::SystemTime>,
    rev: u64,
}

impl PlanWatcher {
    pub fn new(dir: std::path::PathBuf) -> Self {
        Self { dir, stamp: None, rev: 0 }
    }

    /// A plan newer than the last one returned, if the file changed.
    pub fn poll(&mut self) -> Option<WallPlan> {
        let m = std::fs::metadata(self.dir.join("wall.json")).ok()?.modified().ok()?;
        if self.stamp == Some(m) {
            return None;
        }
        self.stamp = Some(m);
        let p = WallPlan::load(&self.dir)?;
        if p.rev <= self.rev {
            return None;
        }
        self.rev = p.rev;
        Some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypr::HyprMonitor;

    fn monitors() -> Vec<HyprMonitor> {
        let m = |id: i64, name: &str, x: i32, t: u8, mm: (i32, i32)| HyprMonitor {
            id,
            name: name.into(),
            description: String::new(),
            width: 1920,
            height: 1080,
            refresh_rate: 60.0,
            x,
            y: 0,
            scale: 1.0,
            transform: t,
            physical_width: mm.0,
            physical_height: mm.1,
            reserved: [0, 30, 0, 0],
        };
        vec![m(1, "DP-3", 0, 0, (540, 300)), m(2, "HDMI-A-1", 1920, 0, (480, 270)), m(0, "DP-1", 3840, 1, (600, 340))]
    }

    fn client(pid: i64, x: i32, w: i32, h: i32, monitor: i64) -> HyprClient {
        HyprClient { pid, at: [x, 0], size: [w, h], monitor, class: "kitty".into(), mapped: true, hidden: false }
    }

    fn pane(pid: u32, cols: u16, rows: u16, cell: (u16, u16)) -> PaneGeom {
        PaneGeom {
            pid,
            ancestors: vec![pid],
            cols,
            rows,
            cell: Some((cell.0 as f64, cell.1 as f64)),
            pad: (0.0, 0.0),
            centered: false,
        }
    }

    fn setup() -> (Desk, DeskConfig, Vec<HyprClient>, Vec<PaneGeom>) {
        let cfg = DeskConfig::default();
        let desk = Desk::from_hypr(&monitors(), &cfg);
        let clients = vec![
            client(10, 0, 1920, 1080, 1),
            client(20, 1920, 1920, 1080, 2),
            client(30, 3840, 1080, 1920, 0),
        ];
        // matched fonts give different cell counts on each monitor
        let panes = vec![pane(10, 213, 56, (9, 19)), pane(20, 192, 50, (10, 21)), pane(30, 135, 91, (8, 21))];
        (desk, cfg, clients, panes)
    }

    #[test]
    fn every_pane_is_placed_and_the_frame_is_the_landscape_row() {
        let (desk, cfg, clients, panes) = setup();
        let p = plan(&desk, &cfg, &clients, &panes, 10).unwrap();
        assert_eq!(p.panes.len(), 3);
        let portrait = p.pane(30).unwrap();
        assert!(portrait.portrait);
        // the frame spans the two landscape panes, not the portrait one
        assert!((p.frame.w - (538.4 + 480.0)).abs() < 5.0, "{:?}", p.frame);
        // the portrait pane extends above and below the frame
        assert!(portrait.content.y < p.frame.y && portrait.content.bottom() > p.frame.bottom());
    }

    #[test]
    fn studio_space_joins_at_the_seams() {
        let (desk, cfg, clients, panes) = setup();
        let p = plan(&desk, &cfg, &clients, &panes, 10).unwrap();
        let (a, b, c) = (p.pane(10).unwrap(), p.pane(20).unwrap(), p.pane(30).unwrap());
        // a pane's cell grid ends where the next begins, less the real strip
        // the terminal leaves unused at its right edge
        let short = p.frame.w.min(p.frame.h);
        let right = |pp: &PanePlan, cols: f64| pp.comp.origin[0] + cols * pp.comp.step[0];
        let gap = |l: &PanePlan, r: &PanePlan| (r.content.x - l.content.right()) / short;
        assert!((right(a, 213.0) + gap(a, b) - b.comp.origin[0]).abs() < 1e-9);
        assert!((right(b, 192.0) + gap(b, c) - c.comp.origin[0]).abs() < 1e-9);
        assert!(gap(a, b) >= 0.0 && gap(a, b) < 0.003, "only the unused strip, under one cell");
        // a horizontal line at the frame centre is at p.y = 0 on every monitor
        let mid = p.frame.y + p.frame.h / 2.0;
        for pp in [a, b, c] {
            let y = pp.comp.origin[1] - (mid - pp.content.y) / short;
            assert!(y.abs() < 1e-9, "{}: {y}", pp.monitor);
        }
    }

    #[test]
    fn classic_canvas_fits_the_budget_and_covers_every_pane() {
        let (desk, cfg, clients, panes) = setup();
        let p = plan(&desk, &cfg, &clients, &panes, 10).unwrap();
        let (w, h) = p.classic_cells;
        assert!(w * h <= CLASSIC_BUDGET_CELLS, "{w}x{h}");
        for pp in &p.panes {
            let (o, s) = pp.classic.expect("extend policy: every pane joins");
            let g = panes.iter().find(|g| g.pid == pp.pid).unwrap();
            assert!(o[0] >= -1e-6 && o[1] >= -1e-6);
            assert!(o[0] + s[0] * g.cols as f64 <= w as f64 + 1e-6);
            assert!(o[1] + s[1] * g.rows as f64 <= h as f64 + 1e-6);
        }
    }

    #[test]
    fn separate_policy_leaves_portrait_out_of_the_classic_wall() {
        let (desk, mut cfg, clients, panes) = setup();
        cfg.portrait = "separate".into();
        let p = plan(&desk, &cfg, &clients, &panes, 10).unwrap();
        assert!(p.pane(30).unwrap().classic.is_none());
        assert!(p.pane(10).unwrap().classic.is_some());
    }

    #[test]
    fn watcher_returns_each_revision_once() {
        let (desk, cfg, clients, panes) = setup();
        let dir = std::env::temp_dir().join(format!("termpaper-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = PlanWatcher::new(dir.clone());
        assert!(w.poll().is_none());
        let mut p = plan(&desk, &cfg, &clients, &panes, 10).unwrap();
        p.rev = 1;
        p.store(&dir).unwrap();
        assert_eq!(w.poll().map(|p| p.rev), Some(1));
        assert!(w.poll().is_none(), "unchanged file");
        std::thread::sleep(std::time::Duration::from_millis(20));
        p.rev = 2;
        p.store(&dir).unwrap();
        assert_eq!(w.poll().map(|p| p.rev), Some(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn plan_round_trips_and_compares_by_layout() {
        let (desk, cfg, clients, panes) = setup();
        let mut p = plan(&desk, &cfg, &clients, &panes, 10).unwrap();
        let dir = std::env::temp_dir().join(format!("termpaper-wallplan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        p.rev = 7;
        p.store(&dir).unwrap();
        let back = WallPlan::load(&dir).unwrap();
        assert_eq!(back.rev, 7);
        assert!(back.same_layout(&p));
        let _ = std::fs::remove_dir_all(dir);
    }
}
