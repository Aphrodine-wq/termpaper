//! Monitor alignment: a test pattern drawn in desk millimetres.
//!
//! While calibrating, every pane of the wall draws the same pattern at the
//! same desk coordinates: a 10 mm grid, level lines across the whole desk, 30°
//! diagonals, circles centred on each seam and a 100 mm ruler per monitor.
//! Where two monitors meet, a misplaced monitor shows up as a broken line —
//! vertical offset breaks the level lines, a wrong bezel or scale bends the
//! diagonals and flattens the circles — and a physical ruler held to the
//! screen checks the EDID size. The controller nudges the selected monitor's
//! desk offset, bezel and scale until every line runs straight, then saves
//! `desk.toml`.
use crate::canvas::Canvas;
use crate::desk::{Align, DeskConfig, MonitorCfg, RectMm};
use crate::wallplan::PanePlan;
use serde::{Deserialize, Serialize};

/// What the controller is doing, shared through `calib.json` so every pane
/// previews the same working copy.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CalibState {
    pub controller: u32,
    pub rev: u64,
    /// monitor being adjusted
    pub selected: String,
    /// working copy of the desk config
    pub desk: DeskConfig,
}

/// One adjustment. Keys map onto these in the controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nudge {
    Move(f64, f64),
    Bezel(f64),
    Scale(f64),
    CycleAlign,
    Reset,
}

impl CalibState {
    /// Apply an adjustment to the selected monitor.
    pub fn apply(&mut self, n: Nudge) {
        if let Nudge::CycleAlign = n {
            let i = Align::ALL.iter().position(|a| *a == self.desk.align).unwrap_or(0);
            self.desk.align = Align::ALL[(i + 1) % Align::ALL.len()];
            // offsets were measured against the old alignment
            for m in self.desk.monitors.values_mut() {
                m.offset_mm = [0.0, 0.0];
            }
            self.rev += 1;
            return;
        }
        let m = self.desk.monitors.entry(self.selected.clone()).or_default();
        match n {
            Nudge::Move(dx, dy) => {
                m.offset_mm[0] += dx;
                m.offset_mm[1] += dy;
            }
            Nudge::Bezel(d) => {
                let b = m.bezel_mm.unwrap_or(self.desk.bezel_mm);
                m.bezel_mm = Some((b + d).max(0.0));
            }
            Nudge::Scale(f) => {
                m.scale = Some((m.scale.unwrap_or(1.0) * f).clamp(0.5, 2.0));
            }
            Nudge::Reset => *m = MonitorCfg::default(),
            Nudge::CycleAlign => {}
        }
        self.rev += 1;
    }
}

/// Minimum distance (in mm) from `v` to the nearest multiple of `step`.
fn to_grid(v: f64, step: f64) -> f64 {
    let r = v.rem_euclid(step);
    r.min(step - r)
}

/// Draw the pattern for one pane. `seams` are the x positions (mm) where
/// monitors meet; `selected` marks this pane's monitor as the one being
/// adjusted (it gets a coloured frame).
pub fn draw_pattern(canvas: &mut Canvas, pane: &PanePlan, frame: RectMm, seams: &[f64], selected: bool) {
    let (w, h) = (canvas.width(), canvas.height());
    if w == 0 || h == 0 {
        return;
    }
    let c = pane.content;
    let mx = c.w / w as f64; // mm per canvas pixel
    let my = c.h / h as f64;
    let line = mx.max(my) * 0.75; // a line is about one pixel wide
    let fc = (frame.x + frame.w / 2.0, frame.y + frame.h / 2.0);
    let (s30, c30) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
    let near = |d: f64, width: f64| d < width;
    for py in 0..h {
        for px in 0..w {
            let x = c.x + (px as f64 + 0.5) * mx;
            let y = c.y + (py as f64 + 0.5) * my;
            let mut col = (6u8, 8u8, 12u8);
            if near(to_grid(x, 10.0), line) || near(to_grid(y, 10.0), line) {
                col = (34, 38, 46);
            }
            if near(to_grid(x, 50.0), line * 1.6) || near(to_grid(y, 50.0), line * 1.6) {
                col = (70, 76, 90);
            }
            // desk-wide level lines: frame top, middle, bottom
            for ly in [frame.y, fc.1, frame.bottom()] {
                if near((y - ly).abs(), line * 1.8) {
                    col = (240, 200, 60);
                }
            }
            // 30° diagonals through the frame centre
            let (dx, dy) = (x - fc.0, y - fc.1);
            if near((dx * s30 - dy * c30).abs(), line * 1.8) || near((dx * s30 + dy * c30).abs(), line * 1.8) {
                col = (80, 200, 230);
            }
            // a 100 mm circle on every seam, centred on the level line
            for &sx in seams {
                let r = ((x - sx).powi(2) + (y - fc.1).powi(2)).sqrt();
                if near((r - 100.0).abs(), line * 1.8) {
                    col = (230, 90, 200);
                }
            }
            // 100 mm ruler with 10 mm ticks, low in the pane
            let rx0 = c.x + c.w / 2.0 - 50.0;
            let ry = c.bottom() - 40.0;
            if x >= rx0 && x <= rx0 + 100.0 {
                if near((y - ry).abs(), line * 2.0) {
                    col = (240, 240, 240);
                }
                if near(to_grid(x - rx0, 10.0), line * 1.5) && y > ry - 6.0 && y < ry {
                    col = (240, 240, 240);
                }
            }
            if selected && (px < 2 || py < 2 || px + 2 >= w || py + 2 >= h) {
                col = (90, 230, 120);
            }
            canvas.set(px as i32, py as i32, col);
        }
    }
}

/// The on-screen instructions for the controller.
pub fn help_lines(state: &CalibState) -> Vec<String> {
    let m = state.desk.monitors.get(&state.selected).cloned().unwrap_or_default();
    vec![
        format!(
            "Aligning {}  ·  offset {:+.1},{:+.1} mm  ·  bezel {:.1} mm  ·  scale {:.3}  ·  align {}",
            state.selected,
            m.offset_mm[0],
            m.offset_mm[1],
            m.bezel_mm.unwrap_or(state.desk.bezel_mm),
            m.scale.unwrap_or(1.0),
            state.desk.align.name()
        ),
        "1. ↑/↓ until the yellow level lines run straight across the seam".into(),
        "2. [ ] (bezel) until the cyan diagonals meet without a kink; ←/→ nudges sideways".into(),
        "3. - = (scale) if a physical ruler disagrees with the white 100 mm bar".into(),
        "Tab next monitor · Shift 10 mm · Alt 0.2 mm · a alignment · r reset · Enter save · Esc cancel".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk::CompView;

    fn pane(content: RectMm) -> PanePlan {
        PanePlan {
            pid: 1,
            monitor: "DP-3".into(),
            content,
            portrait: false,
            comp: CompView { origin: [0.0; 2], step: [0.0; 2], half: [0.0; 2] },
            classic: None,
        }
    }

    #[test]
    fn nudges_edit_the_selected_monitor() {
        let mut s = CalibState { selected: "DP-1".into(), ..Default::default() };
        s.apply(Nudge::Move(1.0, -10.0));
        s.apply(Nudge::Bezel(0.5));
        s.apply(Nudge::Scale(1.0025));
        let m = &s.desk.monitors["DP-1"];
        assert_eq!(m.offset_mm, [1.0, -10.0]);
        assert_eq!(m.bezel_mm, Some(0.5));
        assert!((m.scale.unwrap() - 1.0025).abs() < 1e-12);
        assert_eq!(s.rev, 3);
        s.apply(Nudge::CycleAlign);
        assert_eq!(s.desk.monitors["DP-1"].offset_mm, [0.0, 0.0], "alignment change clears offsets");
        s.apply(Nudge::Reset);
        assert_eq!(s.desk.monitors["DP-1"], MonitorCfg::default());
    }

    #[test]
    fn level_lines_land_at_the_same_desk_height_on_both_panes() {
        let frame = RectMm { x: 0.0, y: 0.0, w: 1000.0, h: 300.0 };
        // two panes of different pixel pitch meeting at x = 500
        let a = pane(RectMm { x: 0.0, y: 0.0, w: 500.0, h: 300.0 });
        let b = pane(RectMm { x: 500.0, y: 0.0, w: 500.0, h: 300.0 });
        let mut ca = Canvas::new(200, 120);
        let mut cb = Canvas::new(160, 96);
        draw_pattern(&mut ca, &a, frame, &[500.0], false);
        draw_pattern(&mut cb, &b, frame, &[500.0], false);
        let yellow = |c: &Canvas, x: i32| (0..c.height() as i32).find(|&y| c.get(x, y).color == (240, 200, 60) && y > 5);
        // middle level line (150 mm) at the right edge of A and left edge of B
        let ya = yellow(&ca, 199).unwrap() as f64 / 120.0 * 300.0;
        let yb = yellow(&cb, 0).unwrap() as f64 / 96.0 * 300.0;
        assert!((ya - yb).abs() < 4.0, "{ya} vs {yb}");
    }
}
