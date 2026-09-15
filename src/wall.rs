//! Location-aware video wall: linked terminals whose window geometry is
//! known (Hyprland via `hyprctl`, or manual `--wall COLSxROWS:INDEX`) act
//! as viewports onto one shared virtual canvas. Same seed + t0 makes the
//! simulation identical everywhere; each instance blits only its crop.

/// Window geometry in screen pixels.
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Geo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Everything needed to render one instance's share of the wall.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WallLayout {
    pub virtual_w: usize,
    pub virtual_h: usize,
    pub crop_x: usize,
    pub crop_y: usize,
    pub too_big: bool,
}

/// Max virtual area before falling back to local rendering (3x perf area).
const MAX_AREA: usize = 3 * 200 * 100;

/// Extract `{at:[x,y], size:[w,h]}` for our pid from `hyprctl clients -j`.
/// Hand-rolled: hyprctl emits flat objects, so we locate `"pid":N` inside
/// its enclosing braces and pull the numbers out of that slice.
pub fn parse_hyprctl_for_pid(text: &str, pid: u32) -> Option<Geo> {
    let pat = format!("\"pid\": {pid}");
    let pos = text.find(&pat).or_else(|| text.find(&format!("\"pid\":{pid}")))?;
    // Enclosing object: brace-count outward from the pid field. hyprctl
    // objects contain NESTED objects (e.g. "workspace": {...}) before the
    // pid, so a naive nearest-'{' grabs the wrong slice.
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut start = None;
    for i in (0..pos).rev() {
        match bytes[i] {
            b'}' => depth += 1,
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let start = start?;
    let mut depth = 0i32;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().skip(pos) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                if depth == 0 {
                    end = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let end = end?;
    let obj = &text[start..=end];

    fn arr2(obj: &str, key: &str) -> Option<(i32, i32)> {
        let pat = format!("\"{key}\": [");
        let at = obj.find(&pat).or_else(|| obj.find(&format!("\"{key}\":[")))?;
        let rest = &obj[at..];
        let lb = rest.find('[')?;
        let rb = rest[lb..].find(']')? + lb;
        let nums: Vec<i32> = rest[lb + 1..rb]
            .split(',')
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        if nums.len() >= 2 {
            Some((nums[0], nums[1]))
        } else {
            None
        }
    }
    let (x, y) = arr2(obj, "at")?;
    let (w, h) = arr2(obj, "size")?;
    Some(Geo { x, y, w, h })
}

/// Query the compositor for our window geometry. None when unavailable.
pub fn hypr_geo(pid: u32) -> Option<Geo> {
    let mut cmd = std::process::Command::new("hyprctl");
    cmd.arg("clients").arg("-j");
    // hyprctl refuses to run without HYPRLAND_INSTANCE_SIGNATURE; when the
    // caller's env lacks it (e.g. spawned from a detached shell), fall back
    // to the lone instance dir under $XDG_RUNTIME_DIR/hypr/.
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        if let Ok(x) = std::env::var("XDG_RUNTIME_DIR") {
            if let Ok(rd) = std::fs::read_dir(PathBuf::from(x).join("hypr")) {
                let dirs: Vec<_> = rd.flatten().filter(|e| e.path().is_dir()).collect();
                if dirs.len() == 1 {
                    cmd.env(
                        "HYPRLAND_INSTANCE_SIGNATURE",
                        dirs[0].file_name().to_string_lossy().as_ref(),
                    );
                }
            }
        }
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_hyprctl_for_pid(&String::from_utf8_lossy(&out.stdout), pid)
}

/// Parent pid of `pid` from /proc, or None.
fn ppid_of(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm may contain spaces/parens — ppid is the field after the last ')'
    let after = stat.rsplit_once(')')?.1;
    after.split_whitespace().nth(1)?.parse().ok()
}

/// Window geometry for this process, walking up the ancestor chain.
/// The compositor tracks the terminal emulator's pid, not ours — when
/// launched via `kitty -e termpaper` our parent (or grandparent via a
/// shell) owns the window. Tries up to 4 hops. Also returns the ancestor
/// pid that owned the window, so callers can skip the walk next time.
fn own_geo_traced() -> Option<(u32, Geo)> {
    let mut pid = std::process::id();
    for _ in 0..4 {
        if let Some(g) = hypr_geo(pid) {
            return Some((pid, g));
        }
        pid = ppid_of(pid)?;
    }
    None
}

/// One-shot geometry lookup (blocking — spawns hyprctl).
pub fn own_geo() -> Option<Geo> {
    own_geo_traced().map(|(_, g)| g)
}

/// Polls the compositor for this window's geometry on a background thread
/// so the frame loop never blocks on a subprocess. The thread is detached
/// and dies with the process; it holds no state that needs cleanup.
pub struct GeoWatcher {
    latest: std::sync::Arc<std::sync::Mutex<Option<Geo>>>,
}

impl GeoWatcher {
    pub fn spawn() -> Self {
        let latest = std::sync::Arc::new(std::sync::Mutex::new(None));
        let slot = std::sync::Arc::clone(&latest);
        let _ = std::thread::Builder::new()
            .name("termpaper-geo".into())
            .spawn(move || {
                // remember which ancestor owns the window: steady state is
                // one hyprctl spawn per tick instead of up to four
                let mut owner: Option<u32> = None;
                loop {
                    let found = owner
                        .and_then(|p| hypr_geo(p).map(|g| (p, g)))
                        .or_else(own_geo_traced);
                    owner = found.map(|(p, _)| p);
                    if let Ok(mut s) = slot.lock() {
                        *s = found.map(|(_, g)| g);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
        GeoWatcher { latest }
    }

    /// Most recent geometry (up to ~2s stale). Never blocks on hyprctl.
    pub fn latest(&self) -> Option<Geo> {
        self.latest.lock().map(|g| *g).unwrap_or(None)
    }
}

/// One wall participant: registry info plus geometry and terminal size.
#[derive(Clone, Copy, Debug)]
pub struct Participant {
    pub pid: u32,
    pub geo: Geo,
    pub cols: usize,
    pub rows: usize,
    /// terminal padding in px — the cell grid is inset this far inside
    /// `geo` on every side; crops are computed on the inset content rect
    /// so art lines up across window borders despite the margins
    pub pad: (i32, i32),
}

/// One axis of the wall: given each pane's pixel span and its cell count on
/// that axis, return the cell offset of every distinct pane edge.
///
/// Cell counts are the source of truth; pixel geometry only *orders* the
/// panes. Measuring in pixels instead — dividing absolute offsets by an
/// averaged px-per-cell — accumulates error, because a terminal's window is
/// rarely an exact multiple of its cell size, so px/cell differs slightly per
/// window (e.g. 1920/272 = 7.059 vs 960/135 = 7.111). Averaging those and
/// scaling up over the width of a wall drifts by a column or more per seam,
/// which shows up as duplicated or dropped strips at every window border.
///
/// The union of spans is cut at every edge; each resulting segment is sized
/// from the panes covering it, so panes that tile exactly get exact offsets.
fn axis_offsets(spans: &[(i32, i32, usize)], tol: i32) -> Vec<(i32, usize)> {
    let mut edges: Vec<i32> = spans.iter().flat_map(|&(a, b, _)| [a, b]).collect();
    edges.sort_unstable();
    edges.dedup();
    // merge edges that differ by less than a cell — window borders and
    // compositor rounding put neighbouring panes a few px apart
    let mut merged: Vec<i32> = Vec::with_capacity(edges.len());
    for e in edges {
        if merged.last().is_none_or(|&m| e - m > tol) {
            merged.push(e);
        }
    }
    if merged.len() < 2 {
        return vec![(merged.first().copied().unwrap_or(0), 0)];
    }
    // size each segment from the panes spanning it, in cells
    let mut offsets = Vec::with_capacity(merged.len());
    let mut acc = 0usize;
    for i in 0..merged.len() - 1 {
        offsets.push((merged[i], acc));
        let (s, e) = (merged[i], merged[i + 1]);
        let seg_px = (e - s).max(1) as f32;
        let mut est = 0.0f32;
        let mut n = 0.0f32;
        for &(a, b, cells) in spans {
            // does this pane cover the segment?
            if a <= s + tol && b >= e - tol {
                let span_px = (b - a).max(1) as f32;
                est += cells as f32 * seg_px / span_px;
                n += 1.0;
            }
        }
        let cells = if n > 0.0 {
            (est / n).round().max(1.0) as usize
        } else {
            1
        };
        acc += cells;
    }
    offsets.push((merged[merged.len() - 1], acc));
    offsets
}

/// Compute the shared virtual canvas + our crop. Deterministic: depends
/// only on registry data, sorted by pid for tie-breaks.
pub fn compute_layout(mut parts: Vec<Participant>, own_pid: u32) -> Option<WallLayout> {
    if parts.len() < 2 || !parts.iter().any(|p| p.pid == own_pid) {
        return None;
    }
    parts.sort_by_key(|p| p.pid);
    // content rect: window geometry inset by the terminal's padding
    let content = |p: &Participant| {
        (
            p.geo.x + p.pad.0,
            p.geo.y + p.pad.1,
            (p.geo.w - 2 * p.pad.0).max(1),
            (p.geo.h - 2 * p.pad.1).max(1),
        )
    };
    // rough px-per-cell, used only as an edge-merging tolerance
    let cell_w: f32 = parts
        .iter()
        .map(|p| content(p).2 as f32 / p.cols.max(1) as f32)
        .sum::<f32>()
        / parts.len() as f32;
    let cell_h: f32 = parts
        .iter()
        .map(|p| content(p).3 as f32 / p.rows.max(1) as f32)
        .sum::<f32>()
        / parts.len() as f32;
    if cell_w < 1.0 || cell_h < 1.0 {
        return None;
    }

    let xs: Vec<(i32, i32, usize)> = parts
        .iter()
        .map(|p| {
            let (cx, _, cw, _) = content(p);
            (cx, cx + cw, p.cols.max(1))
        })
        .collect();
    let ys: Vec<(i32, i32, usize)> = parts
        .iter()
        .map(|p| {
            let (_, cy, _, ch) = content(p);
            (cy, cy + ch, p.rows.max(1))
        })
        .collect();
    let xo = axis_offsets(&xs, (cell_w * 0.5) as i32);
    let yo = axis_offsets(&ys, (cell_h * 0.5) as i32);
    let virtual_w = xo.last().map(|&(_, c)| c).unwrap_or(0);
    let virtual_h = yo.last().map(|&(_, c)| c).unwrap_or(0);
    if virtual_w == 0 || virtual_h == 0 {
        return None;
    }

    let own = parts.iter().find(|p| p.pid == own_pid)?;
    let (ox, oy, _, _) = content(own);
    // our crop is the cell offset of the edge our content rect starts at
    let at = |offs: &[(i32, usize)], v: i32| -> usize {
        offs.iter()
            .min_by_key(|&&(px, _)| (px - v).abs())
            .map(|&(_, c)| c)
            .unwrap_or(0)
    };
    let crop_x = at(&xo, ox);
    let crop_y = at(&yo, oy);
    Some(WallLayout {
        virtual_w,
        virtual_h,
        crop_x: crop_x.min(virtual_w.saturating_sub(1)),
        crop_y: crop_y.min(virtual_h.saturating_sub(1)),
        too_big: virtual_w * virtual_h > MAX_AREA,
    })
}

/// Manual grid mode: `--wall COLSxROWS:INDEX` (row-major index).
pub fn manual_layout(spec: &str, cols: usize, rows: usize) -> Option<WallLayout> {
    let (grid, idx) = spec.split_once(':')?;
    let (gc, gr) = grid.split_once('x')?;
    let (gc, gr, idx) = (
        gc.parse::<usize>().ok()?,
        gr.parse::<usize>().ok()?,
        idx.parse::<usize>().ok()?,
    );
    if gc == 0 || gr == 0 || idx >= gc * gr {
        return None;
    }
    Some(WallLayout {
        virtual_w: cols * gc,
        virtual_h: rows * gr,
        crop_x: (idx % gc) * cols,
        crop_y: (idx / gc) * rows,
        too_big: cols * gc * rows * gr > MAX_AREA,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HYPR: &str = r#"[
    {
        "address": "0x5a4b3c",
        "at": [0, 46],
        "size": [960, 1000],
        "class": "kitty",
        "title": "termpaper",
        "pid": 111
    },
    {
        "address": "0x5a4b3d",
        "at": [960, 46],
        "size": [960, 1000],
        "class": "kitty",
        "title": "termpaper fire",
        "pid": 222
    }
]"#;

    #[test]
    fn hyprctl_nested_object_before_pid() {
        // real hyprctl output nests "workspace" between "size" and "pid" —
        // the extractor must not grab the inner object
        let text = r#"[{
            "address": "0x1",
            "at": [960, 46],
            "size": [960, 1000],
            "workspace": {
                "id": 1,
                "name": "1"
            },
            "floating": false,
            "class": "kitty",
            "pid": 222,
            "xwayland": false
        }]"#;
        let g = parse_hyprctl_for_pid(text, 222).unwrap();
        assert_eq!((g.x, g.y, g.w, g.h), (960, 46, 960, 1000));
    }

    #[test]
    fn hyprctl_extraction() {
        let g = parse_hyprctl_for_pid(HYPR, 222).unwrap();
        assert_eq!(g, Geo { x: 960, y: 46, w: 960, h: 1000 });
        let g = parse_hyprctl_for_pid(HYPR, 111).unwrap();
        assert_eq!(g.x, 0);
        assert!(parse_hyprctl_for_pid(HYPR, 999).is_none());
    }

    fn part(pid: u32, x: i32, y: i32, w: i32, h: i32, cols: usize, rows: usize) -> Participant {
        Participant { pid, geo: Geo { x, y, w, h }, cols, rows, pad: (0, 0) }
    }

    fn part_pad(
        pid: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        cols: usize,
        rows: usize,
        pad: (i32, i32),
    ) -> Participant {
        Participant { pad, ..part(pid, x, y, w, h, cols, rows) }
    }

    #[test]
    fn padding_insets_crops() {
        // two 914x1000 windows side by side, each with 7px padding: the
        // 100-col grid covers the inner 900px (9px cells), and the gap
        // between the two content areas (7+7px of adjacent padding) spans
        // one virtual column that neither window renders — the wallpaper
        // instance behind fills it
        let parts = vec![
            part_pad(1, 0, 0, 914, 1000, 100, 50, (7, 7)),
            part_pad(2, 914, 0, 914, 1000, 100, 50, (7, 7)),
        ];
        let left = compute_layout(parts.clone(), 1).unwrap();
        let right = compute_layout(parts, 2).unwrap();
        assert_eq!(left.virtual_w, 201);
        assert_eq!((left.crop_x, left.crop_y), (0, 0));
        assert_eq!((right.crop_x, right.crop_y), (101, 0));
    }

    #[test]
    fn two_windows_side_by_side() {
        // two equal terminals next to each other, 100x50 cells each
        let layout = compute_layout(
            vec![
                part(1, 0, 0, 900, 1000, 100, 50),
                part(2, 900, 0, 900, 1000, 100, 50),
            ],
            2,
        )
        .unwrap();
        assert_eq!(layout.virtual_w, 200);
        assert_eq!(layout.virtual_h, 50);
        assert_eq!((layout.crop_x, layout.crop_y), (100, 0));
        assert!(!layout.too_big);
        let left = compute_layout(
            vec![
                part(1, 0, 0, 900, 1000, 100, 50),
                part(2, 900, 0, 900, 1000, 100, 50),
            ],
            1,
        )
        .unwrap();
        assert_eq!((left.crop_x, left.crop_y), (0, 0));
    }

    #[test]
    fn disagreeing_sizes_and_stacked() {
        let layout = compute_layout(
            vec![
                part(5, 0, 0, 1000, 500, 100, 25),
                part(2, 0, 500, 500, 500, 50, 25),
                part(9, 500, 500, 500, 500, 50, 25),
            ],
            9,
        )
        .unwrap();
        assert_eq!(layout.virtual_w, 100);
        assert_eq!(layout.virtual_h, 50);
        assert_eq!((layout.crop_x, layout.crop_y), (50, 25));
    }

    #[test]
    fn overlap_is_clamped_and_single_is_none() {
        let layout = compute_layout(
            vec![
                part(1, 0, 0, 500, 500, 50, 25),
                part(2, 250, 250, 500, 500, 50, 25),
            ],
            2,
        )
        .unwrap();
        assert!(layout.crop_x < layout.virtual_w);
        assert!(compute_layout(vec![part(1, 0, 0, 500, 500, 50, 25)], 1).is_none());
    }

    #[test]
    fn manual_grid() {
        let l = manual_layout("2x1:0", 80, 24).unwrap();
        assert_eq!((l.virtual_w, l.virtual_h, l.crop_x, l.crop_y), (160, 24, 0, 0));
        let l = manual_layout("2x1:1", 80, 24).unwrap();
        assert_eq!((l.crop_x, l.crop_y), (80, 0));
        let l = manual_layout("3x2:4", 40, 10).unwrap();
        assert_eq!((l.virtual_w, l.virtual_h, l.crop_x, l.crop_y), (120, 20, 40, 10));
        assert!(manual_layout("2x1:2", 80, 24).is_none());
        assert!(manual_layout("bogus", 80, 24).is_none());
    }
}

#[cfg(test)]
mod tiling_tests {
    use super::*;

    fn p(pid: u32, x: i32, y: i32, w: i32, h: i32, cols: usize, rows: usize) -> Participant {
        Participant {
            pid,
            geo: Geo { x, y, w, h },
            cols,
            rows,
            pad: (0, 0),
        }
    }

    /// Real geometry from three kitty windows in a row on a 3840px display.
    /// The windows are not exact multiples of the cell width (kitty leaves a
    /// few px of slack), so px/cell differs slightly per window: 1920/272 =
    /// 7.059 vs 960/135 = 7.111. Crops must still tile exactly — each pane
    /// starting where the previous one ended, with no overlap or gap.
    #[test]
    fn adjacent_panes_tile_without_overlap() {
        let parts = vec![
            p(1357985, 0, 540, 1920, 540, 272, 33),
            p(1357514, 1920, 540, 960, 540, 135, 33),
            p(1357167, 2880, 540, 960, 540, 135, 33),
        ];
        let of = |pid| compute_layout(parts.clone(), pid).unwrap();
        let (a, b, c) = (of(1357985), of(1357514), of(1357167));

        // the virtual canvas is exactly the sum of the columns in the row
        assert_eq!(a.virtual_w, 272 + 135 + 135, "virtual width must be exact");
        assert_eq!(a.virtual_h, 33);

        // and each pane starts exactly where its left neighbour ended
        assert_eq!(a.crop_x, 0, "leftmost pane starts at 0");
        assert_eq!(b.crop_x, 272, "second pane must start where the first ends");
        assert_eq!(c.crop_x, 272 + 135, "third pane must start where the second ends");
        for l in [a, b, c] {
            assert_eq!(l.crop_y, 0);
        }
    }
}
