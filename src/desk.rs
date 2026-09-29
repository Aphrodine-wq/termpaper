//! The physical desk: every monitor placed in millimetres.
//!
//! Hyprland lays monitors out in logical pixels, but a pixel is not the same
//! size on every monitor (0.25 mm on one, 0.31 mm on the next), a rotated
//! monitor reports its size unrotated, and the compositor knows nothing about
//! bezels or how the screens actually sit on the desk. Art that should flow
//! across screens has to be placed in real units. `Desk` does that: monitors
//! left to right in their Hyprland order, sized from EDID (or config), aligned
//! by a preset, nudged by per-monitor offsets from calibration.
//!
//! The desk is machine-level state (`~/.config/termpaper/desk.toml`), not part
//! of the per-profile `config.toml`: every termpaper on the machine must agree
//! on it, whichever config profile it runs with.
use crate::hypr::HyprMonitor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A rectangle on the desk, millimetres, y down (screen convention).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RectMm {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl RectMm {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn union(&self, o: &RectMm) -> RectMm {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        RectMm { x, y, w: self.right().max(o.right()) - x, h: self.bottom().max(o.bottom()) - y }
    }
}

/// How monitors of different heights line up vertically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Top,
    /// centres on one line — how monitors on stands or arms usually sit
    #[default]
    Center,
    Bottom,
    /// Hyprland's own vertical layout, scaled to millimetres
    Hypr,
}

impl Align {
    pub const ALL: [Align; 4] = [Align::Center, Align::Top, Align::Bottom, Align::Hypr];
    pub fn name(self) -> &'static str {
        match self {
            Align::Top => "top",
            Align::Center => "center",
            Align::Bottom => "bottom",
            Align::Hypr => "hypr",
        }
    }
}

/// Per-monitor calibration, keyed by output name in desk.toml.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorCfg {
    /// Nudge from the default spot beside the left neighbour, mm. Moving a
    /// monitor right also moves every monitor to its right.
    pub offset_mm: [f64; 2],
    /// Gap to the left neighbour (art hidden behind the bezels), mm;
    /// overrides the desk-wide `bezel_mm`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bezel_mm: Option<f64>,
    /// Visible size in the monitor's on-screen orientation, mm; overrides a
    /// missing or wrong EDID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_mm: Option<[f64; 2]>,
    /// Fine size correction (1.0 = as measured).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeskConfig {
    pub align: Align,
    /// Desk-wide gap between neighbouring screens, mm (0 = art continuous at
    /// the screen edges).
    pub bezel_mm: f64,
    /// The composition frame: "row" (the landscape monitors, so a portrait
    /// screen extends above and below the picture), "all", or
    /// "monitor:<NAME>".
    pub frame: String,
    /// Classic (CPU) scenes on a portrait monitor: "extend" (one continuous
    /// wall), "band" (only the frame's height) or "separate".
    pub portrait: String,
    pub monitors: BTreeMap<String, MonitorCfg>,
}

impl Default for DeskConfig {
    fn default() -> Self {
        Self {
            align: Align::Center,
            bezel_mm: 0.0,
            frame: "row".into(),
            portrait: "extend".into(),
            monitors: BTreeMap::new(),
        }
    }
}

/// A monitor in its on-screen orientation, logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub desc: String,
    /// x, y, w, h in the compositor's logical layout (scaled and rotated)
    pub logical: [f64; 4],
    pub scale: f64,
    pub transform: u8,
    pub refresh_hz: f64,
    /// EDID size rotated to the on-screen orientation, when plausible
    pub edid_mm: Option<(f64, f64)>,
    pub reserved: [i32; 4],
}

/// Logical pixels at 96 dpi, for monitors that report no usable size.
const FALLBACK_MM_PER_PX: f64 = 25.4 / 96.0;

impl Monitor {
    pub fn from_hypr(m: &HyprMonitor) -> Self {
        let rotated = m.transform % 2 == 1;
        let (pw, ph) = if rotated { (m.height, m.width) } else { (m.width, m.height) };
        let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
        let (mw, mh) = if rotated {
            (m.physical_height, m.physical_width)
        } else {
            (m.physical_width, m.physical_height)
        };
        let w = pw as f64 / scale;
        let h = ph as f64 / scale;
        // EDIDs often carry 0x0, or an aspect ratio (16x9, 160x90) instead
        // of a size: accept only sizes that give a sane pixel pitch and
        // roughly match the mode's aspect
        const ASPECT_ONLY: [(i32, i32); 6] = [(160, 90), (160, 100), (90, 160), (100, 160), (64, 27), (43, 18)];
        let edid_mm = (mw > 50 && mh > 50 && !ASPECT_ONLY.contains(&(mw, mh)))
            .then_some((mw as f64, mh as f64))
            .filter(|(a, b)| {
                let pitch = (a * a + b * b).sqrt() / (w * w + h * h).sqrt();
                let aspect_err = ((a / b) / (w / h) - 1.0).abs();
                (0.1..=1.5).contains(&pitch) && aspect_err < 0.2
            });
        Self {
            id: m.id,
            name: m.name.clone(),
            desc: m.description.clone(),
            logical: [m.x as f64, m.y as f64, w, h],
            scale,
            transform: m.transform,
            refresh_hz: m.refresh_rate,
            edid_mm,
            reserved: m.reserved,
        }
    }

    pub fn portrait(&self) -> bool {
        self.logical[3] > self.logical[2]
    }

    /// Millimetres per logical pixel, isotropic. From the diagonal: EDID
    /// widths and heights are rounded separately, the diagonal averages the
    /// error out.
    pub fn mm_per_px(&self, cfg: Option<&MonitorCfg>) -> f64 {
        let (w, h) = (self.logical[2], self.logical[3]);
        let diag_px = (w * w + h * h).sqrt().max(1.0);
        let base = match cfg.and_then(|c| c.size_mm).map(|s| (s[0], s[1])).or(self.edid_mm) {
            Some((a, b)) => (a * a + b * b).sqrt() / diag_px,
            None => FALLBACK_MM_PER_PX * self.scale,
        };
        base * cfg.and_then(|c| c.scale).unwrap_or(1.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeskMonitor {
    pub mon: Monitor,
    pub rect: RectMm,
    pub mm_per_px: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Desk {
    pub monitors: Vec<DeskMonitor>,
}

impl Desk {
    /// Place monitors left to right in Hyprland's x order.
    pub fn build(mut mons: Vec<Monitor>, cfg: &DeskConfig) -> Desk {
        mons.sort_by(|a, b| {
            (a.logical[0], a.logical[1])
                .partial_cmp(&(b.logical[0], b.logical[1]))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut out: Vec<DeskMonitor> = Vec::with_capacity(mons.len());
        let mut cursor = 0.0;
        for (i, m) in mons.into_iter().enumerate() {
            let mc = cfg.monitors.get(&m.name);
            let mpp = m.mm_per_px(mc);
            let w = m.logical[2] * mpp;
            let h = m.logical[3] * mpp;
            let gap = if i == 0 { 0.0 } else { mc.and_then(|c| c.bezel_mm).unwrap_or(cfg.bezel_mm) };
            let off = mc.map(|c| c.offset_mm).unwrap_or([0.0, 0.0]);
            let x = cursor + gap + off[0];
            let y = match cfg.align {
                Align::Top => 0.0,
                Align::Center => -h / 2.0,
                Align::Bottom => -h,
                Align::Hypr => m.logical[1] * mpp,
            } + off[1];
            cursor = x + w;
            out.push(DeskMonitor { mon: m, rect: RectMm { x, y, w, h }, mm_per_px: mpp });
        }
        // put the desk's top-left at the origin: numbers stay readable
        if let Some(bb) = out.iter().map(|d| d.rect).reduce(|a, b| a.union(&b)) {
            for d in &mut out {
                d.rect.x -= bb.x;
                d.rect.y -= bb.y;
            }
        }
        Desk { monitors: out }
    }

    pub fn from_hypr(mons: &[HyprMonitor], cfg: &DeskConfig) -> Desk {
        Desk::build(mons.iter().map(Monitor::from_hypr).collect(), cfg)
    }

    pub fn get(&self, name: &str) -> Option<&DeskMonitor> {
        self.monitors.iter().find(|d| d.mon.name == name)
    }

    pub fn by_id(&self, id: i64) -> Option<&DeskMonitor> {
        self.monitors.iter().find(|d| d.mon.id == id)
    }

    /// The monitor containing a point of the logical layout.
    pub fn at_logical(&self, x: f64, y: f64) -> Option<&DeskMonitor> {
        self.monitors.iter().find(|d| {
            let [mx, my, mw, mh] = d.mon.logical;
            x >= mx && x < mx + mw && y >= my && y < my + mh
        })
    }

    /// A logical-layout rectangle (e.g. a window) → desk millimetres, via
    /// the monitor that contains its top-left corner.
    pub fn logical_to_mm(&self, r: [f64; 4]) -> Option<(RectMm, &DeskMonitor)> {
        let d = self.at_logical(r[0] + 0.5, r[1] + 0.5)?;
        let [mx, my, _, _] = d.mon.logical;
        Some((
            RectMm {
                x: d.rect.x + (r[0] - mx) * d.mm_per_px,
                y: d.rect.y + (r[1] - my) * d.mm_per_px,
                w: r[2] * d.mm_per_px,
                h: r[3] * d.mm_per_px,
            },
            d,
        ))
    }

    /// The composition frame for a set of pane rectangles (see
    /// `DeskConfig::frame`).
    pub fn frame(&self, cfg: &DeskConfig, panes: &[(RectMm, bool)]) -> Option<RectMm> {
        if let Some(name) = cfg.frame.strip_prefix("monitor:") {
            if let Some(d) = self.get(name) {
                return Some(d.rect);
            }
        }
        let all = panes.iter().map(|p| p.0).reduce(|a, b| a.union(&b))?;
        if cfg.frame == "all" {
            return Some(all);
        }
        // "row": the landscape panes, when there are any
        panes
            .iter()
            .filter(|p| !p.1)
            .map(|p| p.0)
            .reduce(|a, b| a.union(&b))
            .or(Some(all))
    }
}

/// Where a pane's pixels land in Studio composition space: the frame's short
/// side spans [-0.5, 0.5], y up, centred on the frame. `origin` is the point
/// at the top-left corner of the pane's first pixel; `step` is p per pixel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompView {
    pub origin: [f64; 2],
    pub step: [f64; 2],
    pub half: [f64; 2],
}

/// Map a pane's content (the cell grid, in desk mm) into composition space.
/// `px` is the canvas resolution of the pane (cells × pixels per cell).
pub fn comp_view(frame: RectMm, content: RectMm, px: (usize, usize)) -> CompView {
    let short = frame.w.min(frame.h).max(1e-6);
    let cx = frame.x + frame.w / 2.0;
    let cy = frame.y + frame.h / 2.0;
    CompView {
        origin: [(content.x - cx) / short, (cy - content.y) / short],
        step: [
            content.w / px.0.max(1) as f64 / short,
            content.h / px.1.max(1) as f64 / short,
        ],
        half: [frame.w / 2.0 / short, frame.h / 2.0 / short],
    }
}

pub fn desk_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TERMPAPER_DESK") {
        return Some(PathBuf::from(p));
    }
    crate::platform::config_dir().map(|d| d.join("desk.toml"))
}

/// The desk config, or defaults when there is none (or it does not parse).
pub fn load_desk() -> DeskConfig {
    desk_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_desk(cfg: &DeskConfig) -> std::io::Result<()> {
    let path = desk_path().ok_or_else(|| std::io::Error::other("no config directory"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

/// Human-readable table for `termpaper desk`.
pub fn describe(desk: &Desk) -> String {
    let mut s = String::from("monitor     logical        rot  scale  mm/px   size mm        desk rect mm\n");
    for d in &desk.monitors {
        let m = &d.mon;
        s.push_str(&format!(
            "{:<11} {:>5}x{:<5}@{:<4} {:>3}  {:<5.2}  {:.4}  {:>5.0}x{:<5.0}  x {:>6.1} y {:>6.1} w {:>5.1} h {:>5.1}{}\n",
            m.name,
            m.logical[2] as i64,
            m.logical[3] as i64,
            m.logical[0] as i64,
            m.transform as u32 * 90 % 360,
            m.scale,
            d.mm_per_px,
            d.rect.w,
            d.rect.h,
            d.rect.x,
            d.rect.y,
            d.rect.w,
            d.rect.h,
            if m.edid_mm.is_none() { "  (no EDID size)" } else { "" },
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hm(name: &str, x: i32, transform: u8, mm: (i32, i32)) -> HyprMonitor {
        HyprMonitor {
            id: x as i64,
            name: name.into(),
            description: String::new(),
            width: 1920,
            height: 1080,
            refresh_rate: 60.0,
            x,
            y: 0,
            scale: 1.0,
            transform,
            physical_width: mm.0,
            physical_height: mm.1,
            reserved: [0, 30, 0, 0],
        }
    }

    /// The machine this was written on: two landscape 1080p screens and a
    /// rotated one with bigger pixels.
    fn real() -> Vec<HyprMonitor> {
        vec![hm("DP-3", 0, 0, (540, 300)), hm("HDMI-A-1", 1920, 0, (480, 270)), hm("DP-1", 3840, 1, (600, 340))]
    }

    #[test]
    fn rotated_monitors_swap_size_and_millimetres() {
        let m = Monitor::from_hypr(&hm("DP-1", 3840, 1, (600, 340)));
        assert_eq!(m.logical, [3840.0, 0.0, 1080.0, 1920.0]);
        assert_eq!(m.edid_mm, Some((340.0, 600.0)));
        assert!(m.portrait());
    }

    #[test]
    fn pixel_pitch_comes_from_the_diagonal() {
        let d = Desk::from_hypr(&real(), &DeskConfig::default());
        let pitch = |n: &str| d.get(n).unwrap().mm_per_px;
        assert!((pitch("DP-3") - 0.2804).abs() < 0.001, "{}", pitch("DP-3"));
        assert!((pitch("HDMI-A-1") - 0.2500).abs() < 0.001);
        assert!((pitch("DP-1") - 0.3131).abs() < 0.001);
    }

    #[test]
    fn bogus_edid_sizes_fall_back() {
        // aspect-ratio-only EDID (16x9 cm) and zeros
        let m = Monitor::from_hypr(&hm("X", 0, 0, (160, 90)));
        assert!(m.edid_mm.is_none(), "160x90 mm is an aspect ratio, not a 1080p screen");
        let z = Monitor::from_hypr(&hm("Y", 0, 0, (0, 0)));
        assert!((z.mm_per_px(None) - FALLBACK_MM_PER_PX).abs() < 1e-9);
        let fixed = MonitorCfg { size_mm: Some([527.0, 296.0]), ..Default::default() };
        assert!((z.mm_per_px(Some(&fixed)) - 0.2745).abs() < 0.001);
    }

    #[test]
    fn center_alignment_hangs_the_portrait_screen_around_the_row() {
        let d = Desk::from_hypr(&real(), &DeskConfig::default());
        let (a, b, c) = (d.get("DP-3").unwrap().rect, d.get("HDMI-A-1").unwrap().rect, d.get("DP-1").unwrap().rect);
        // edges touch, left to right
        assert!((a.right() - b.x).abs() < 1e-9 && (b.right() - c.x).abs() < 1e-9);
        // all share one centre line
        let mid = |r: RectMm| r.y + r.h / 2.0;
        assert!((mid(a) - mid(b)).abs() < 1e-9 && (mid(a) - mid(c)).abs() < 1e-9);
        // the portrait screen extends ~150 mm above and below the tallest landscape one
        assert!((a.y - c.y - 149.0).abs() < 2.0, "{} vs {}", a.y, c.y);
    }

    #[test]
    fn offsets_and_bezels_carry_to_the_right() {
        let mut cfg = DeskConfig::default();
        cfg.monitors.insert("HDMI-A-1".into(), MonitorCfg { offset_mm: [5.0, -3.0], ..Default::default() });
        cfg.bezel_mm = 10.0;
        let base = Desk::from_hypr(&real(), &DeskConfig::default());
        let d = Desk::from_hypr(&real(), &cfg);
        let dx = |n: &str| d.get(n).unwrap().rect.x - base.get(n).unwrap().rect.x;
        assert!((dx("HDMI-A-1") - 15.0).abs() < 1e-6);
        assert!((dx("DP-1") - 25.0).abs() < 1e-6, "DP-1 moves with its left neighbour plus its own bezel");
    }

    #[test]
    fn composition_space_is_continuous_across_a_seam() {
        let d = Desk::from_hypr(&real(), &DeskConfig::default());
        // two fullscreen panes on the landscape monitors, different fonts
        let (a, _) = d.logical_to_mm([0.0, 0.0, 1920.0, 1080.0]).unwrap();
        let (b, _) = d.logical_to_mm([1920.0, 0.0, 1920.0, 1080.0]).unwrap();
        let frame = d.frame(&DeskConfig::default(), &[(a, false), (b, false)]).unwrap();
        let va = comp_view(frame, a, (213, 112));
        let vb = comp_view(frame, b, (240, 126));
        // A's right edge is B's left edge
        let a_right = va.origin[0] + 213.0 * va.step[0];
        assert!((a_right - vb.origin[0]).abs() < 1e-9);
        // the frame's short side spans one unit
        assert!((va.half[1] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn desk_config_round_trips_through_toml() {
        let mut cfg = DeskConfig::default();
        cfg.monitors.insert("DP-1".into(), MonitorCfg { offset_mm: [0.0, -12.0], bezel_mm: Some(17.5), ..Default::default() });
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: DeskConfig = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
        // an empty file is all defaults
        assert_eq!(toml::from_str::<DeskConfig>("").unwrap(), DeskConfig::default());
    }
}
