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
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WallLayout {
    pub virtual_w: usize,
    pub virtual_h: usize,
    pub crop_x: usize,
    pub crop_y: usize,
    pub too_big: bool,
    /// Terminal cell height / width shared by every pane of the wall, when
    /// all of them measured their cells (else the renderer's default).
    pub cell_aspect: Option<f32>,
}

/// Where a terminal puts the strip left over when its window is not a whole
/// number of cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// The grid starts at the padding; the leftover goes right and bottom
    /// (kitty's `placement_strategy top-left`, and most terminals).
    #[default]
    TopLeft,
    /// The leftover is split evenly around the grid.
    Center,
}

impl Placement {
    pub fn name(self) -> &'static str {
        match self {
            Placement::TopLeft => "top-left",
            Placement::Center => "center",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "top-left" => Some(Placement::TopLeft),
            "center" => Some(Placement::Center),
            _ => None,
        }
    }
}

/// Max virtual area before falling back to local rendering (3x perf area).
const MAX_AREA: usize = 3 * 200 * 100;

/// Extract `{at:[x,y], size:[w,h]}` for our pid from `hyprctl clients -j`.
/// Hand-rolled: hyprctl emits flat objects, so we locate `"pid":N` inside
/// its enclosing braces and pull the numbers out of that slice.
pub fn parse_hyprctl_for_pid(text: &str, pid: u32) -> Option<Geo> {
    let obj = client_object(text, pid)?;
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

/// A top-level number field (`"key": 12` or `"key":1.5`) of a JSON object
/// slice, skipping nested objects.
fn top_level_num(obj: &str, key: &str) -> Option<f64> {
    let bytes = obj.as_bytes();
    let pat = format!("\"{key}\"");
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b'"' if depth == 1
                && obj[i..].starts_with(&pat)
                && obj[i + pat.len()..].trim_start().starts_with(':') =>
            {
                let rest = obj[i + pat.len()..].trim_start()[1..].trim_start();
                let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
                return rest[..end].parse().ok();
            }
            b'"' => {
                // skip the string
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The monitor id of our window's client object.
pub fn parse_hyprctl_monitor_for_pid(text: &str, pid: u32) -> Option<i64> {
    top_level_num(client_object(text, pid)?, "monitor").map(|m| m as i64)
}

/// A monitor's scale from `hyprctl monitors -j`.
pub fn parse_monitor_scale(text: &str, id: i64) -> Option<f32> {
    top_level_objects(text)
        .into_iter()
        .find(|o| top_level_num(o, "id") == Some(id as f64))
        .and_then(|o| top_level_num(o, "scale"))
        .map(|s| s as f32)
        .filter(|s| *s > 0.1)
}

/// The objects of a top-level JSON array.
fn top_level_objects(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let (mut depth, mut start, mut in_str, mut i) = (0i32, 0usize, false, 0usize);
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if b == b'\\' {
                i += 1;
            } else if b == b'"' {
                in_str = false;
            }
        } else {
            match b {
                b'"' => in_str = true,
                b'{' => {
                    if depth == 1 {
                        start = i;
                    }
                    depth += 1;
                }
                b'[' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 1 {
                        out.push(&text[start..=i]);
                    }
                }
                b']' => depth -= 1,
                _ => {}
            }
        }
        i += 1;
    }
    out
}

/// The client object whose `"pid"` is `pid`, as a slice of `hyprctl clients
/// -j` output.
fn client_object(text: &str, pid: u32) -> Option<&str> {
    let pat = format!("\"pid\": {pid}");
    let pos = [pat, format!("\"pid\":{pid}")].iter().find_map(|pat| {
        text.match_indices(pat).find_map(|(pos, matched)| {
            (!text.as_bytes().get(pos + matched.len()).is_some_and(u8::is_ascii_digit))
                .then_some(pos)
        })
    })?;
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
    Some(&text[start..=end])
}

/// Query the compositor for our window geometry. None when unavailable.
pub fn hypr_geo(pid: u32) -> Option<Geo> {
    parse_hyprctl_for_pid(&hyprctl(&["clients", "-j"])?, pid)
}

/// Run `hyprctl <args>` and return its stdout.
fn hyprctl(args: &[&str]) -> Option<String> {
    if !crate::hypr::present() {
        return None;
    }
    let mut cmd = std::process::Command::new("hyprctl");
    cmd.args(args);
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
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
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
    /// window geometry, and the scale of the monitor it is on
    latest: std::sync::Arc<std::sync::Mutex<(Option<Geo>, Option<f32>)>>,
}

impl GeoWatcher {
    pub fn spawn() -> Self {
        let latest = std::sync::Arc::new(std::sync::Mutex::new((None, None)));
        let slot = std::sync::Arc::clone(&latest);
        let _ = std::thread::Builder::new()
            .name("termpaper-geo".into())
            .spawn(move || {
                // remember which ancestor owns the window: steady state is
                // one hyprctl spawn per tick instead of up to four
                let mut owner: Option<u32> = None;
                let mut monitor: Option<(i64, Option<f32>)> = None;
                let mut ticks = 0u32;
                loop {
                    let clients = hyprctl(&["clients", "-j"]);
                    let found = clients.as_deref().and_then(|text| {
                        let hit = |pid: u32| {
                            parse_hyprctl_for_pid(text, pid).map(|g| (pid, g, parse_hyprctl_monitor_for_pid(text, pid)))
                        };
                        owner.and_then(hit).or_else(|| {
                            let mut pid = std::process::id();
                            for _ in 0..4 {
                                if let Some(f) = hit(pid) {
                                    return Some(f);
                                }
                                pid = ppid_of(pid)?;
                            }
                            None
                        })
                    });
                    owner = found.map(|(p, _, _)| p);
                    // the scale only changes with the monitor (or a config
                    // reload): re-read it on a move and every ~20 s
                    let mon = found.and_then(|(_, _, m)| m);
                    let stale = monitor.map(|(id, _)| id) != mon || ticks.is_multiple_of(10);
                    if let Some(id) = mon.filter(|_| stale) {
                        let scale = hyprctl(&["monitors", "-j"]).and_then(|t| parse_monitor_scale(&t, id));
                        monitor = Some((id, scale));
                    }
                    if let Ok(mut s) = slot.lock() {
                        *s = (found.map(|(_, g, _)| g), monitor.and_then(|(_, sc)| sc));
                    }
                    ticks = ticks.wrapping_add(1);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
        GeoWatcher { latest }
    }

    /// Most recent geometry (up to ~2s stale). Never blocks on hyprctl.
    pub fn latest(&self) -> Option<Geo> {
        self.latest.lock().map(|g| g.0).unwrap_or(None)
    }

    /// Scale of the monitor the window is on (physical px per layout px).
    pub fn latest_scale(&self) -> Option<f32> {
        self.latest.lock().map(|g| g.1).unwrap_or(None)
    }
}

/// The terminal's cell size in layout px, from its reported window pixel
/// size — when that is trustworthy: an exact multiple of the grid (some
/// terminals report 0, or the whole window including padding) that fits
/// inside the window's geometry. Physical px are divided by the monitor's
/// `scale`, since compositor geometry is in layout px.
pub fn measure_cell(
    cols: usize,
    rows: usize,
    width_px: u16,
    height_px: u16,
    scale: Option<f32>,
    geo: Option<Geo>,
) -> Option<(f32, f32)> {
    let (w, h) = (width_px as usize, height_px as usize);
    if cols == 0 || rows == 0 || w == 0 || h == 0 || w % cols != 0 || h % rows != 0 {
        return None;
    }
    let s = scale.unwrap_or(1.0).max(0.1);
    let (cw, ch) = ((w / cols) as f32 / s, (h / rows) as f32 / s);
    if let Some(g) = geo {
        if cols as f32 * cw > g.w as f32 + 0.5 || rows as f32 * ch > g.h as f32 + 0.5 {
            return None;
        }
    }
    Some((cw, ch))
}

/// One wall participant: registry info plus geometry and terminal size.
#[derive(Clone, Copy, Debug)]
pub struct Participant {
    pub pid: u32,
    pub geo: Geo,
    pub cols: usize,
    pub rows: usize,
    /// terminal padding in layout px (x, y) — the cell grid is inset this
    /// far inside `geo`; crops are computed on the inset content rect so art
    /// lines up across window borders despite the margins
    pub pad: (f32, f32),
    /// measured cell size in layout px, when the terminal reports it
    pub cell: Option<(f32, f32)>,
    pub placement: Placement,
}

impl Participant {
    /// The exact rectangle the cell grid covers, in layout px: needs the
    /// measured cell size. The grid sits at the padding, plus half the
    /// leftover strip when the terminal centres it.
    pub fn content_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let (cw, ch) = self.cell?;
        let (w, h) = (self.cols as f32 * cw, self.rows as f32 * ch);
        let (mut x, mut y) = (self.geo.x as f32 + self.pad.0, self.geo.y as f32 + self.pad.1);
        if self.placement == Placement::Center {
            x += ((self.geo.w as f32 - 2.0 * self.pad.0 - w) / 2.0).max(0.0);
            y += ((self.geo.h as f32 - 2.0 * self.pad.1 - h) / 2.0).max(0.0);
        }
        Some((x, y, w, h))
    }
}

/// Cell sizes further apart than this can't share a cell grid.
const CELL_AGREEMENT: f32 = 0.03;

/// The wall on the participants' measured cell grids: every pane's content
/// rect is exact, so each crop is its offset from the wall's corner in
/// (shared) cells — gaps between windows included, however wide.
fn exact_layout(parts: &[Participant], own_pid: u32) -> Option<WallLayout> {
    let rects: Vec<_> = parts.iter().map(|p| p.content_rect()).collect::<Option<_>>()?;
    let n = parts.len() as f32;
    let cw = parts.iter().filter_map(|p| p.cell).map(|c| c.0).sum::<f32>() / n;
    let ch = parts.iter().filter_map(|p| p.cell).map(|c| c.1).sum::<f32>() / n;
    let agree = parts.iter().filter_map(|p| p.cell).all(|(w, h)| {
        (w - cw).abs() <= cw * CELL_AGREEMENT && (h - ch).abs() <= ch * CELL_AGREEMENT
    });
    if !agree || cw < 1.0 || ch < 1.0 {
        return None;
    }
    let x0 = rects.iter().map(|r| r.0).fold(f32::INFINITY, f32::min);
    let y0 = rects.iter().map(|r| r.1).fold(f32::INFINITY, f32::min);
    let crop = |r: &(f32, f32, f32, f32)| {
        (((r.0 - x0) / cw).round() as usize, ((r.1 - y0) / ch).round() as usize)
    };
    let mut vw = 0;
    let mut vh = 0;
    let mut own = None;
    for (p, r) in parts.iter().zip(&rects) {
        let (cx, cy) = crop(r);
        vw = vw.max(cx + p.cols);
        vh = vh.max(cy + p.rows);
        if p.pid == own_pid {
            own = Some((cx, cy));
        }
    }
    let (crop_x, crop_y) = own?;
    Some(WallLayout {
        virtual_w: vw,
        virtual_h: vh,
        crop_x,
        crop_y,
        too_big: vw * vh > MAX_AREA,
        cell_aspect: Some(ch / cw),
    })
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

/// Portrait and landscape panes never share a wall: a rotated monitor's
/// content rect would stretch the virtual canvas to ~2x height, and every
/// landscape pane would then render a crop of a picture composed for
/// neither orientation. Square counts as landscape.
fn landscape(content: (i32, i32, i32, i32)) -> bool {
    content.2 >= content.3
}

/// Compute the shared virtual canvas + our crop. Deterministic: depends
/// only on registry data, sorted by pid for tie-breaks.
pub fn compute_layout(mut parts: Vec<Participant>, own_pid: u32) -> Option<WallLayout> {
    // content rect: window geometry inset by the terminal's padding (whole
    // px: this is also the orientation test and the fallback's input)
    let content = |p: &Participant| {
        let (px, py) = (p.pad.0.round() as i32, p.pad.1.round() as i32);
        (
            p.geo.x + px,
            p.geo.y + py,
            (p.geo.w - 2 * px).max(1),
            (p.geo.h - 2 * py).max(1),
        )
    };
    let own_landscape = landscape(content(parts.iter().find(|p| p.pid == own_pid)?));
    parts.retain(|p| landscape(content(p)) == own_landscape);
    if parts.len() < 2 {
        return None;
    }
    parts.sort_by_key(|p| p.pid);
    // every pane measured its cells: place them exactly
    if let Some(l) = exact_layout(&parts, own_pid) {
        return Some(l);
    }
    // otherwise cell counts are the truth and geometry only orders panes
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
        cell_aspect: None,
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
        cell_aspect: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_lookup_does_not_match_a_longer_pid() {
        let text = r#"[{"pid":1112,"at":[1,2],"size":[30,40]},
            {"pid":111,"at":[50,60],"size":[70,80]}]"#;
        assert_eq!(parse_hyprctl_for_pid(text, 111), Some(Geo { x: 50, y: 60, w: 70, h: 80 }));
        assert_eq!(parse_hyprctl_for_pid(text, 11), None);
    }

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
    fn monitor_scale_for_our_window() {
        let clients = r#"[{
            "at": [0, 30], "size": [1920, 1050],
            "workspace": {"id": 2, "name": "2"},
            "monitor": 1, "class": "monitor", "pid": 42
        }]"#;
        assert_eq!(parse_hyprctl_monitor_for_pid(clients, 42), Some(1));
        // nested workspace ids must not be mistaken for the monitor's id
        let monitors = r#"[{
            "id": 0, "name": "DP-1", "activeWorkspace": {"id": 1, "name": "1"},
            "scale": 1, "transform": 1
        },{
            "id": 1, "name": "DP-2", "activeWorkspace": {"id": 0, "name": "x"},
            "scale": 1.50, "transform": 0
        }]"#;
        assert_eq!(parse_monitor_scale(monitors, 1), Some(1.5));
        assert_eq!(parse_monitor_scale(monitors, 0), Some(1.0));
        assert_eq!(parse_monitor_scale(monitors, 7), None);
    }

    #[test]
    fn cell_measurement_is_accepted_only_when_it_fits() {
        let geo = Some(Geo { x: 0, y: 0, w: 940, h: 1000 });
        assert_eq!(measure_cell(103, 49, 927, 980, None, geo), Some((9.0, 20.0)));
        // physical px on a 1.5x monitor
        let (cw, ch) = measure_cell(103, 49, 1854, 1960, Some(1.5), Some(Geo { x: 0, y: 0, w: 1400, h: 1400 })).unwrap();
        assert!((cw - 12.0).abs() < 1e-4 && (ch - 40.0 / 1.5).abs() < 1e-4);
        assert_eq!(measure_cell(103, 49, 0, 0, None, geo), None, "not reported");
        assert_eq!(measure_cell(103, 49, 940, 1000, None, geo), None, "the whole window, not the grid");
        assert_eq!(measure_cell(103, 49, 1854, 1960, None, geo), None, "does not fit the window");
        assert_eq!(measure_cell(103, 49, 927, 980, None, None), Some((9.0, 20.0)), "no geometry to check against");
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
        Participant {
            pid,
            geo: Geo { x, y, w, h },
            cols,
            rows,
            pad: (0.0, 0.0),
            cell: None,
            placement: Placement::TopLeft,
        }
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
        Participant { pad: (pad.0 as f32, pad.1 as f32), ..part(pid, x, y, w, h, cols, rows) }
    }

    /// kitty with `window_padding_width 3.5` (pt = 4.667 px) and 9x20 cells,
    /// windows 20 px apart: the grid's cell count and the terminal's
    /// leftover strip come from the geometry exactly as kitty computes them.
    fn kitty(pid: u32, x: i32, y: i32, w: i32, h: i32, placement: Placement) -> Participant {
        let pad = crate::config::PadSpec::parse("3.5pt").unwrap().to_px(1.0);
        let (cw, ch) = (9.0f32, 20.0f32);
        let cols = ((w as f32 - 2.0 * pad.0) / cw).floor() as usize;
        let rows = ((h as f32 - 2.0 * pad.1) / ch).floor() as usize;
        Participant { pid, geo: Geo { x, y, w, h }, cols, rows, pad, cell: Some((cw, ch)), placement }
    }

    #[test]
    fn exact_content_rects_top_left() {
        // two 940x1000 windows, 20 px apart
        let a = kitty(1, 10, 40, 940, 1000, Placement::TopLeft);
        let b = kitty(2, 970, 40, 940, 1000, Placement::TopLeft);
        assert_eq!((a.cols, a.rows), (103, 49));
        // top-left: the grid starts right at the padding
        let r = a.content_rect().unwrap();
        assert!((r.0 - 14.667).abs() < 1e-3 && (r.1 - 44.667).abs() < 1e-3, "{r:?}");
        assert_eq!((r.2, r.3), (927.0, 980.0));
        let left = compute_layout(vec![a, b], 1).unwrap();
        let right = compute_layout(vec![a, b], 2).unwrap();
        // b's grid starts 960 px (106.67 cells) right of a's: 107, so the
        // 33 px between the grids (a's 3.7 px leftover, both paddings and
        // the gap) is 4 columns nobody draws, not 1
        assert_eq!((left.crop_x, left.crop_y), (0, 0));
        assert_eq!((right.crop_x, right.crop_y), (107, 0));
        assert_eq!((left.virtual_w, left.virtual_h), (107 + 103, 49));
        assert_eq!(left.cell_aspect, Some(20.0 / 9.0));
        // stacked, with b's window 11 px shorter: rows are exact too
        let top = kitty(3, 0, 0, 940, 520, Placement::TopLeft);
        let bottom = kitty(4, 0, 540, 940, 509, Placement::TopLeft);
        let l = compute_layout(vec![top, bottom], 4).unwrap();
        assert_eq!(l.crop_y, 27, "540 px down = 27 rows exactly");
        assert_eq!(l.virtual_h, 27 + bottom.rows);
    }

    #[test]
    fn exact_content_rects_center() {
        let a = kitty(1, 10, 40, 940, 1000, Placement::Center);
        let b = kitty(2, 970, 40, 945, 1000, Placement::Center);
        // centred: half of each leftover strip sits before the grid
        let (ra, rb) = (a.content_rect().unwrap(), b.content_rect().unwrap());
        let lead_a = (940.0 - 2.0 * a.pad.0 - 927.0) / 2.0;
        assert!((ra.0 - (10.0 + a.pad.0 + lead_a)).abs() < 1e-3, "{ra:?}");
        assert!((ra.1 - (40.0 + a.pad.1 + (1000.0 - 2.0 * a.pad.1 - 980.0) / 2.0)).abs() < 1e-3);
        let right = compute_layout(vec![a, b], 2).unwrap();
        assert_eq!(right.crop_x, ((rb.0 - ra.0) / 9.0).round() as usize);
        assert_eq!(right.crop_x, 107);
        // a pane that didn't measure its cells drops the wall back to the
        // cell-count method, which knows nothing of placement
        let mut blind = b;
        blind.cell = None;
        let fallback = compute_layout(vec![a, blind], 2).unwrap();
        assert_eq!(fallback.cell_aspect, None);
        // cells that disagree (different fonts) can't share a grid either
        let mut big = b;
        big.cell = Some((12.0, 26.0));
        assert_eq!(compute_layout(vec![a, big], 2).unwrap().cell_aspect, None);
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
    fn portrait_pane_does_not_stretch_a_landscape_wall() {
        // two 1920x1080 landscape monitors (kitty windows under a 30px bar)
        // plus a rotated 1080x1920 monitor at x=3840 running its own art
        let parts = vec![
            part(1, 0, 30, 1920, 1050, 272, 33),
            part(2, 1920, 30, 1920, 1050, 272, 33),
            part(3, 3840, 0, 1080, 1920, 270, 213),
        ];
        let left = compute_layout(parts.clone(), 1).unwrap();
        let right = compute_layout(parts.clone(), 2).unwrap();
        assert_eq!((left.virtual_w, left.virtual_h), (544, 33));
        assert_eq!((right.virtual_w, right.virtual_h), (544, 33));
        assert_eq!((left.crop_x, left.crop_y), (0, 0));
        assert_eq!((right.crop_x, right.crop_y), (272, 0));
        // the lone portrait pane renders locally
        assert!(compute_layout(parts, 3).is_none());
    }

    #[test]
    fn portrait_pair_forms_its_own_wall() {
        let parts = vec![
            part(1, 0, 0, 1920, 1080, 272, 33),
            part(2, 1920, 0, 1080, 1920, 135, 100),
            part(3, 3000, 0, 1080, 1920, 135, 100),
        ];
        let a = compute_layout(parts.clone(), 2).unwrap();
        let b = compute_layout(parts.clone(), 3).unwrap();
        assert_eq!((a.virtual_w, a.virtual_h), (270, 100));
        assert_eq!((a.crop_x, b.crop_x), (0, 135));
        assert!(compute_layout(parts, 1).is_none());
    }

    #[test]
    fn square_pane_counts_as_landscape() {
        assert!(landscape((0, 0, 500, 500)));
        assert!(landscape((0, 0, 501, 500)));
        assert!(!landscape((0, 0, 500, 501)));
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
            pad: (0.0, 0.0),
            cell: None,
            placement: Placement::TopLeft,
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
