//! Instance linking: file-based discovery and control between termpaper
//! instances in different terminals. Everything degrades silently to
//! "disabled" when the filesystem says no.
//!
//! A group's registry dir holds:
//!
//! - `inst-<pid>.json` — one per live instance: terminal size, window
//!   geometry and the cell/padding facts the video wall needs.
//! - `anchor.json` — the group's simulation anchor ([`Anchor`]): scene,
//!   seed, start time, pause state and every setting that changes what the
//!   simulation draws. It persists while any instance lives, so late joiners
//!   adopt it at startup and nobody needs a heartbeat.
//! - `settings.json` — the appearance settings ([`SettingsMsg`]): filters,
//!   grading, dim, fade. Its own file, so it can never hide an anchor.
//! - `control.json` (+ its `scene.json` copy) — the legacy single-message
//!   channel. Still written as a mirror of every publish so older binaries
//!   keep following; new binaries only read it for messages that lack the
//!   `proto` marker, i.e. ones an older binary wrote.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Link protocol version written into every file. 2 = anchor.json,
/// settings.json and the wall's cell/padding facts in inst files.
pub const PROTO: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlKind {
    Scene,
    Settings,
}

/// Appearance settings broadcast to the group: everything that changes how
/// a frame looks but not what the simulation draws. Sim-affecting settings
/// (speed, detail, pixels, text scale, theme) travel in the [`Anchor`].
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsMsg {
    pub filters: Vec<String>,
    pub fps: u32,
    pub smooth: f32,
    pub dim: f32,
    pub fade: f32,
    pub clock: bool,
    /// quick filter preview from the `f` key (None = off)
    pub quick: Option<String>,
    /// global hue rotation degrees (0 = off)
    pub hue_shift: f32,
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for SettingsMsg {
    fn default() -> Self {
        SettingsMsg {
            filters: Vec::new(),
            fps: 60,
            smooth: 0.3,
            dim: 1.0,
            fade: 0.25,
            clock: true,
            quick: None,
            hue_shift: 0.0,
            saturation: 1.0,
            contrast: 1.0,
        }
    }
}

pub struct Control {
    pub kind: ControlKind,
    pub settings: Option<SettingsMsg>,
    pub scene: String,
    pub theme: Option<String>,
    /// milliseconds since unix epoch
    pub epoch: u64,
    /// per-publisher monotonic sequence
    pub seq: u64,
    pub from_pid: u32,
    /// artwork sync: scene seed + start timestamp (ms)
    pub seed: u64,
    pub t0_ms: u64,
    /// link protocol of the writer; 0 = an older binary (no marker)
    pub proto: u32,
}

/// Total ordering for control messages: newer wins lexicographically,
/// ties broken deterministically by pid.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub struct Stamp {
    pub epoch: u64,
    pub seq: u64,
    pub from_pid: u32,
}

impl Stamp {
    fn encode(&self) -> String {
        format!("{}-{}-{}", self.epoch, self.seq, self.from_pid)
    }

    fn decode(s: &str) -> Option<Stamp> {
        let mut it = s.split('-');
        let st = Stamp {
            epoch: it.next()?.parse().ok()?,
            seq: it.next()?.parse().ok()?,
            from_pid: it.next()?.parse().ok()?,
        };
        it.next().is_none().then_some(st)
    }
}

/// The group's simulation anchor: everything two panes must agree on to
/// draw the identical simulation at the identical moment.
///
/// The *sim identity* is every field except `stamp`, `t0_ms`,
/// `paused_at_ms` and `proto`. A new anchor with the same identity is a
/// retime (pause, resume, resume from suspend): receivers keep their
/// simulation and only move the clock. A different identity is a switch,
/// scheduled for the epoch time `t0_ms` so every pane swaps together.
#[derive(Clone, Debug, PartialEq)]
pub struct Anchor {
    pub stamp: Stamp,
    pub scene: String,
    pub theme: Option<String>,
    pub seed: u64,
    /// epoch ms at which the scene's clock reads zero (and the switch to it
    /// happens, for a scheduled switch)
    pub t0_ms: u64,
    /// epoch ms the wall was paused at; the scene clock is frozen there
    pub paused_at_ms: Option<u64>,
    pub speed: f32,
    pub detail: String,
    pub pixels: String,
    pub text_scale: Option<u32>,
    pub proto: u32,
}

/// How an incoming anchor relates to the one a pane is running (or about
/// to run).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnchorChange {
    /// Same identity and timing: nothing to do.
    Same,
    /// Same identity, new clock: move t0/pause, keep the simulation.
    Retime,
    /// New identity: rebuild, at the anchor's t0.
    Switch,
}

pub fn classify(current: &Anchor, incoming: &Anchor) -> AnchorChange {
    if !current.same_sim(incoming) {
        AnchorChange::Switch
    } else if current.t0_ms != incoming.t0_ms || current.paused_at_ms != incoming.paused_at_ms {
        AnchorChange::Retime
    } else {
        AnchorChange::Same
    }
}

fn opt_str_json(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("\"{}\"", esc(s)),
        None => "null".into(),
    }
}

fn opt_num_json<T: std::fmt::Display>(v: Option<T>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "null".into(),
    }
}

impl Anchor {
    /// Same simulation: everything but the clock and the stamp.
    pub fn same_sim(&self, o: &Anchor) -> bool {
        self.scene == o.scene
            && self.theme == o.theme
            && self.seed == o.seed
            && self.speed == o.speed
            && self.detail == o.detail
            && self.pixels == o.pixels
            && self.text_scale == o.text_scale
    }

    /// Scene clock in ms at epoch time `now_ms` (frozen while paused, zero
    /// before t0).
    pub fn elapsed_at(&self, now_ms: u64) -> u64 {
        self.paused_at_ms.unwrap_or(now_ms).saturating_sub(self.t0_ms)
    }

    pub fn paused(&self) -> bool {
        self.paused_at_ms.is_some()
    }

    /// Freeze the clock at `now_ms` (no-op when already paused).
    pub fn pause(&mut self, now_ms: u64) {
        if self.paused_at_ms.is_none() {
            self.paused_at_ms = Some(now_ms.max(self.t0_ms));
        }
    }

    /// Unfreeze: t0 moves forward by the paused duration, so the clock
    /// resumes exactly where it stopped.
    pub fn resume(&mut self, now_ms: u64) {
        if let Some(at) = self.paused_at_ms.take() {
            self.t0_ms += now_ms.saturating_sub(at);
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"anchor\",\"proto\":{},\"epoch\":{},\"seq\":{},\"from_pid\":{},\"scene\":\"{}\",\"theme\":{},\"seed\":{},\"t0_ms\":{},\"paused_at_ms\":{},\"speed\":{},\"detail\":\"{}\",\"pixels\":\"{}\",\"text_scale\":{}}}",
            self.proto,
            self.stamp.epoch,
            self.stamp.seq,
            self.stamp.from_pid,
            esc(&self.scene),
            opt_str_json(&self.theme),
            self.seed,
            self.t0_ms,
            opt_num_json(self.paused_at_ms),
            self.speed,
            esc(&self.detail),
            esc(&self.pixels),
            opt_num_json(self.text_scale),
        )
    }

    pub fn parse(text: &str) -> Option<Anchor> {
        if json_get(text, "kind") != Some("anchor") {
            return None;
        }
        Some(Anchor {
            stamp: Stamp {
                epoch: json_get(text, "epoch")?.parse().ok()?,
                seq: json_get(text, "seq")?.parse().ok()?,
                from_pid: json_get(text, "from_pid")?.parse().ok()?,
            },
            scene: json_get(text, "scene")?.to_string(),
            theme: json_get(text, "theme").map(|s| s.to_string()),
            seed: json_get(text, "seed")?.parse().ok()?,
            t0_ms: json_get(text, "t0_ms")?.parse().ok()?,
            paused_at_ms: json_get(text, "paused_at_ms").and_then(|v| v.parse().ok()),
            speed: json_get(text, "speed")?.parse().ok()?,
            detail: json_get(text, "detail")?.to_string(),
            pixels: json_get(text, "pixels")?.to_string(),
            text_scale: json_get(text, "text_scale").and_then(|v| v.parse().ok()),
            proto: json_get(text, "proto").and_then(|v| v.parse().ok()).unwrap_or(PROTO),
        })
    }

    /// An older binary's scene message, as an anchor: it only carries the
    /// scene, theme, seed and t0, so the sim settings come from `sim`.
    pub fn from_legacy(c: &Control, sim: &Anchor) -> Anchor {
        Anchor {
            stamp: Stamp { epoch: c.epoch, seq: c.seq, from_pid: c.from_pid },
            scene: c.scene.clone(),
            theme: c.theme.clone(),
            seed: c.seed,
            t0_ms: c.t0_ms,
            paused_at_ms: None,
            proto: c.proto,
            ..sim.clone()
        }
    }

    /// The legacy `control.json` scene message mirroring this anchor.
    fn legacy_json(&self) -> String {
        format!(
            "{{\"kind\":\"scene\",\"proto\":{},\"scene\":\"{}\",\"theme\":{},\"epoch\":{},\"seq\":{},\"from_pid\":{},\"seed\":{},\"t0_ms\":{}}}",
            PROTO,
            esc(&self.scene),
            opt_str_json(&self.theme),
            self.stamp.epoch,
            self.stamp.seq,
            self.stamp.from_pid,
            self.seed,
            self.t0_ms,
        )
    }
}

impl SettingsMsg {
    fn fields_json(&self) -> String {
        let filters = self
            .filters
            .iter()
            .map(|f| format!("\"{}\"", esc(f)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "\"filters\":[{}],\"fps\":{},\"smooth\":{},\"dim\":{},\"fade\":{},\"clock\":{},\"quick\":{},\"hue_shift\":{},\"saturation\":{},\"contrast\":{}",
            filters,
            self.fps,
            self.smooth,
            self.dim,
            self.fade,
            self.clock,
            opt_str_json(&self.quick),
            self.hue_shift,
            self.saturation,
            self.contrast,
        )
    }

    fn parse_fields(text: &str) -> SettingsMsg {
        let d = SettingsMsg::default();
        let num = |k: &str, dflt: f32| json_get(text, k).and_then(|v| v.parse().ok()).unwrap_or(dflt);
        SettingsMsg {
            filters: parse_str_array(text, "filters"),
            fps: json_get(text, "fps").and_then(|v| v.parse().ok()).unwrap_or(d.fps),
            smooth: num("smooth", d.smooth),
            dim: num("dim", d.dim),
            fade: num("fade", d.fade),
            clock: json_get(text, "clock").map(|v| v == "true").unwrap_or(d.clock),
            quick: json_get(text, "quick").map(|s| s.to_string()),
            hue_shift: num("hue_shift", d.hue_shift),
            saturation: num("saturation", d.saturation),
            contrast: num("contrast", d.contrast),
        }
    }
}

pub struct InstanceInfo {
    pub pid: u32,
    pub scene: String,
    pub group: String,
    pub started_at: u64,
    pub cols: usize,
    pub rows: usize,
    pub geo: Option<(i32, i32, i32, i32)>,
    /// terminal padding in layout px (x, y) — the cell grid is inset by
    /// this much inside the window geometry, so wall crops must account for it
    pub pad: (f32, f32),
    /// measured cell size in layout px (proto 2; None when unmeasured)
    pub cell: Option<(f32, f32)>,
    /// where the terminal puts its leftover strip
    pub placement: crate::wall::Placement,
    /// running the group's shared anchor — false after a local `--cycle`
    /// switch; such a peer must not lead the group
    pub synced: bool,
    /// this pane cannot catch up with the anchor with this stamp in time
    /// and asks the leader for a fresh one
    pub reanchor: Option<Stamp>,
    /// link protocol of the instance (0 = older binary)
    pub proto: u32,
}

/// Whether a process is still running (a live peer, a calibration
/// controller).
pub fn process_alive(pid: u32) -> bool {
    pid_alive(pid)
}

pub fn epoch_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Preset link groups cycled in Settings. Custom names work via config/CLI too.
pub const GROUP_PRESETS: &[&str] = &["default", "wallpaper", "desk", "art"];

/// Sanitize a group name for filesystem use.
pub fn sanitize_group(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return "default".into();
    }
    let clean: String = t
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if clean.is_empty() {
        "default".into()
    } else {
        clean
    }
}

/// $XDG_RUNTIME_DIR/termpaper, else /tmp/termpaper-$UID.
pub fn registry_base() -> Option<PathBuf> {
    if let Ok(x) = std::env::var("XDG_RUNTIME_DIR") {
        if !x.is_empty() {
            return Some(PathBuf::from(x).join("termpaper"));
        }
    }
    let uid = libc_getuid();
    Some(PathBuf::from(format!("/tmp/termpaper-{uid}")))
}

/// Registry directory for a link group. `default` uses the legacy flat dir.
pub fn group_dir(group: &str) -> Option<PathBuf> {
    let base = registry_base()?;
    let g = sanitize_group(group);
    if g == "default" {
        Some(base)
    } else {
        Some(base.join("groups").join(g))
    }
}

/// Back-compat alias.
pub fn registry_dir() -> Option<PathBuf> {
    group_dir("default")
}

#[cfg(target_os = "linux")]
fn libc_getuid() -> u32 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1).map(|v| v.to_string()))
        })
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000)
}

#[cfg(target_os = "macos")]
fn libc_getuid() -> u32 {
    unsafe { libc::getuid() }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn libc_getuid() -> u32 {
    1000
}

#[cfg(target_os = "macos")]
fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0): 0 = alive & ours, EPERM = alive but not ours, ESRCH = dead
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(target_os = "macos"))]
fn pid_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

fn atomic_write(path: &PathBuf, contents: &str) -> std::io::Result<()> {
    // pid-suffixed temp name: concurrent publishers to the same target
    // (control.json) must never share a temp file, or interleaved writes
    // can rename torn JSON into place
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let tmp = path.with_file_name(format!(".{}.{}.tmp", name, std::process::id()));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    // Keep the scene anchor even when a settings message replaces control.
    if name == "control.json"
        && parse_control(contents).is_some_and(|c| c.kind == ControlKind::Scene)
    {
        atomic_write(&path.with_file_name("scene.json"), contents)?;
    }
    Ok(())
}

// Minimal hand-rolled JSON for our flat records (no new dependencies).
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn json_get<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\":");
    let start = text.find(&pat)? + pat.len();
    let rest = &text[start..];
    if rest.starts_with("null") {
        return None;
    }
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(&stripped[..end])
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim())
    }
}

/// (inode, mtime) of a channel file at the last poll that read it — lets
/// per-frame polling stop at a stat when nothing changed. `atomic_write`
/// renames a fresh inode into place on every publish.
type FileSig = (u64, SystemTime);

fn file_sig(path: &Path) -> Option<FileSig> {
    let meta = std::fs::metadata(path).ok()?;
    Some((file_ino(&meta), meta.modified().ok()?))
}

/// The live instance's registry presence; Drop cleans up.
pub struct Guard {
    dir: PathBuf,
    pub pid: u32,
    pub group: String,
    scene: String,
    started_at: u64,
    seq: u64,
    cols: usize,
    rows: usize,
    geo: Option<(i32, i32, i32, i32)>,
    pad: (f32, f32),
    cell: Option<(f32, f32)>,
    placement: crate::wall::Placement,
    synced: bool,
    reanchor: Option<Stamp>,
    ctrl_seen: Option<FileSig>,
    anchor_seen: Option<FileSig>,
    /// stamp of the anchor last published or returned by a poll
    anchor_last: Option<Stamp>,
    /// the group's latest anchor as far as this instance knows; fills the
    /// legacy fields of settings mirrors for older binaries
    anchor: Option<Anchor>,
    settings_seen: Option<FileSig>,
    settings_last: Option<Stamp>,
}

#[cfg(unix)]
fn file_ino(m: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    m.ino()
}

#[cfg(not(unix))]
fn file_ino(_m: &std::fs::Metadata) -> u64 {
    0
}

impl Guard {
    /// The legacy scene anchor (`scene.json`, else `control.json`) — what a
    /// group run by older binaries has instead of `anchor.json`.
    pub fn latest_scene(&self) -> Option<Control> {
        let text = std::fs::read_to_string(self.dir.join("scene.json"))
            .or_else(|_| std::fs::read_to_string(self.dir.join("control.json"))).ok()?;
        parse_control(&text).filter(|c| c.kind == ControlKind::Scene)
    }

    /// The group's current anchor, for a pane joining it: `anchor.json`, or
    /// an older binary's scene message converted with `sim` supplying the
    /// sim settings it lacks. Marks it seen, so polls only report newer ones.
    pub fn latest_anchor(&mut self, sim: &Anchor) -> Option<Anchor> {
        let path = self.dir.join("anchor.json");
        let sig = file_sig(&path);
        let found = std::fs::read_to_string(&path).ok().and_then(|t| Anchor::parse(&t));
        if let Some(a) = found {
            self.anchor_seen = sig;
            self.anchor_last = Some(a.stamp);
            self.anchor = Some(a.clone());
            return Some(a);
        }
        let legacy = self.latest_scene().filter(|c| c.proto < PROTO)?;
        let a = Anchor::from_legacy(&legacy, sim);
        self.anchor = Some(a.clone());
        Some(a)
    }

    pub fn new(scene: &str, group: &str) -> Option<Self> {
        let group = sanitize_group(group);
        let dir = group_dir(&group)?;
        Self::new_in(dir, scene, &group)
    }

    /// A guard in an explicit registry directory (tests, tools).
    pub fn new_in(dir: PathBuf, scene: &str, group: &str) -> Option<Self> {
        std::fs::create_dir_all(&dir).ok()?;
        reap_stale(&dir);
        let pid = std::process::id();
        let g = Guard {
            dir,
            pid,
            group: sanitize_group(group),
            scene: scene.to_string(),
            started_at: epoch_now_ms() / 1000,
            seq: 0,
            cols: 0,
            rows: 0,
            geo: None,
            pad: (0.0, 0.0),
            cell: None,
            placement: crate::wall::Placement::TopLeft,
            synced: true,
            reanchor: None,
            ctrl_seen: None,
            anchor_seen: None,
            anchor_last: None,
            anchor: None,
            settings_seen: None,
            settings_last: None,
        };
        g.write()?;
        Some(g)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self) -> PathBuf {
        self.dir.join(format!("inst-{}.json", self.pid))
    }

    fn write(&self) -> Option<()> {
        let geo_json = match self.geo {
            Some((x, y, w, h)) => format!(",\"geo\":{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}"),
            None => String::new(),
        };
        let reanchor_json = match self.reanchor {
            Some(s) => format!(",\"reanchor\":\"{}\"", s.encode()),
            None => String::new(),
        };
        let cell_json = match self.cell {
            Some((w, h)) => format!(",\"cw\":{w},\"ch\":{h}"),
            None => String::new(),
        };
        // px/py stay whole numbers for older binaries; padx/pady are exact
        atomic_write(
            &self.path(),
            &format!(
                "{{\"pid\":{},\"proto\":{},\"scene\":\"{}\",\"group\":\"{}\",\"started_at_epoch_secs\":{},\"cols\":{},\"rows\":{},\"px\":{},\"py\":{},\"padx\":{},\"pady\":{},\"place\":\"{}\",\"synced\":{},\"version\":\"{}\"{}{}{}}}",
                self.pid,
                PROTO,
                esc(&self.scene),
                esc(&self.group),
                self.started_at,
                self.cols,
                self.rows,
                self.pad.0.round() as i32,
                self.pad.1.round() as i32,
                self.pad.0,
                self.pad.1,
                self.placement.name(),
                self.synced,
                env!("CARGO_PKG_VERSION"),
                cell_json,
                reanchor_json,
                geo_json,
            ),
        )
        .ok()
    }

    /// Ask the group's leader for a fresh anchor because this pane cannot
    /// catch up with the one stamped `stamp` (None withdraws the request).
    pub fn set_reanchor(&mut self, stamp: Option<Stamp>) {
        if self.reanchor != stamp {
            self.reanchor = stamp;
            let _ = self.write();
        }
    }

    /// Record the facts peers need to place this pane's cell grid exactly:
    /// terminal padding (layout px), leftover placement, measured cell size.
    pub fn set_layout(&mut self, pad: (f32, f32), placement: crate::wall::Placement, cell: Option<(f32, f32)>) {
        if self.pad != pad || self.placement != placement || self.cell != cell {
            self.pad = pad;
            self.placement = placement;
            self.cell = cell;
            let _ = self.write();
        }
    }

    /// Record whether this instance runs the group's shared anchor.
    pub fn set_synced(&mut self, synced: bool) {
        if self.synced != synced {
            self.synced = synced;
            let _ = self.write();
        }
    }

    /// Record a scene change (rewrites the file with original start time).
    pub fn set_scene(&mut self, scene: &str) {
        self.scene = scene.to_string();
        let _ = self.write();
    }

    /// Refresh terminal size / window geometry (called periodically).
    pub fn set_geometry(&mut self, cols: usize, rows: usize, geo: Option<(i32, i32, i32, i32)>) {
        if self.cols != cols || self.rows != rows || self.geo != geo {
            self.cols = cols;
            self.rows = rows;
            self.geo = geo;
            let _ = self.write();
        }
    }

    /// Check control.json for a legacy message newer than `last` (and not
    /// ours). Messages carrying the `proto` marker are mirrors of an anchor
    /// or settings publish that arrive through their own files, so they are
    /// skipped here: only older binaries speak through control.json.
    ///
    /// Cheap when idle: a stat per call, and the file is only read (and
    /// parsed) when its (inode, mtime) differ from the last read.
    pub fn poll_control(&mut self, last: Stamp) -> Option<Control> {
        let path = self.dir.join("control.json");
        let sig = file_sig(&path)?;
        if self.ctrl_seen == Some(sig) {
            return None;
        }
        self.ctrl_seen = Some(sig);
        let text = std::fs::read_to_string(path).ok()?;
        parse_control(&text).filter(|c| {
            c.from_pid != self.pid
                && c.proto < PROTO
                && Stamp {
                    epoch: c.epoch,
                    seq: c.seq,
                    from_pid: c.from_pid,
                } > last
        })
    }

    /// Check anchor.json: returns the group's anchor when it changed since
    /// the last poll and is not the one this instance published or already
    /// applied. The file is the source of truth — last writer wins, so every
    /// pane converges on its content even when two publishes race.
    pub fn poll_anchor(&mut self) -> Option<Anchor> {
        let path = self.dir.join("anchor.json");
        let sig = file_sig(&path)?;
        if self.anchor_seen == Some(sig) {
            return None;
        }
        self.anchor_seen = Some(sig);
        let a = Anchor::parse(&std::fs::read_to_string(path).ok()?)?;
        if self.anchor_last == Some(a.stamp) {
            return None;
        }
        self.anchor_last = Some(a.stamp);
        self.anchor = Some(a.clone());
        Some(a)
    }

    /// Publish an anchor to the group: stamps it, writes anchor.json, and
    /// mirrors it as a legacy scene message for older binaries.
    pub fn publish_anchor(&mut self, a: &mut Anchor) {
        self.seq += 1;
        a.stamp = Stamp { epoch: epoch_now_ms(), seq: self.seq, from_pid: self.pid };
        a.proto = PROTO;
        let _ = atomic_write(&self.dir.join("anchor.json"), &a.to_json());
        self.anchor_last = Some(a.stamp);
        self.anchor = Some(a.clone());
        let _ = atomic_write(&self.dir.join("control.json"), &a.legacy_json());
    }

    /// Check settings.json for appearance settings another instance
    /// published since the last poll.
    pub fn poll_settings(&mut self) -> Option<SettingsMsg> {
        let path = self.dir.join("settings.json");
        let sig = file_sig(&path)?;
        if self.settings_seen == Some(sig) {
            return None;
        }
        self.settings_seen = Some(sig);
        let text = std::fs::read_to_string(path).ok()?;
        let stamp = Stamp {
            epoch: json_get(&text, "epoch")?.parse().ok()?,
            seq: json_get(&text, "seq")?.parse().ok()?,
            from_pid: json_get(&text, "from_pid")?.parse().ok()?,
        };
        if stamp.from_pid == self.pid || self.settings_last == Some(stamp) {
            return None;
        }
        self.settings_last = Some(stamp);
        Some(SettingsMsg::parse_fields(&text))
    }

    /// Broadcast the appearance settings: settings.json, plus a legacy
    /// control.json mirror whose sim fields come from the group's anchor so
    /// older binaries don't reset them to defaults.
    pub fn publish_settings(&mut self, m: &SettingsMsg) {
        self.seq += 1;
        let stamp = Stamp { epoch: epoch_now_ms(), seq: self.seq, from_pid: self.pid };
        self.settings_last = Some(stamp);
        let fields = m.fields_json();
        let _ = atomic_write(
            &self.dir.join("settings.json"),
            &format!(
                "{{\"kind\":\"settings\",\"proto\":{},\"epoch\":{},\"seq\":{},\"from_pid\":{},{}}}",
                PROTO, stamp.epoch, stamp.seq, stamp.from_pid, fields
            ),
        );
        let legacy = match &self.anchor {
            Some(a) => format!(
                "\"pixels\":\"{}\",\"detail\":\"{}\",\"theme\":{},\"text_scale\":{},\"speed\":{},",
                esc(&a.pixels),
                esc(&a.detail),
                opt_str_json(&a.theme),
                opt_num_json(a.text_scale),
                a.speed
            ),
            None => String::new(),
        };
        let _ = atomic_write(
            &self.dir.join("control.json"),
            &format!(
                "{{\"kind\":\"settings\",\"proto\":{},{}{},\"epoch\":{},\"seq\":{},\"from_pid\":{},\"seed\":0,\"t0_ms\":0}}",
                PROTO, legacy, fields, stamp.epoch, stamp.seq, stamp.from_pid
            ),
        );
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path());
    }
}

fn parse_str_array(text: &str, key: &str) -> Vec<String> {
    let pat = format!("\"{key}\":[");
    let Some(start) = text.find(&pat) else {
        return Vec::new();
    };
    let rest = &text[start + pat.len()..];
    let Some(end) = rest.find(']') else {
        return Vec::new();
    };
    rest[..end]
        .split(',')
        .filter_map(|p| p.trim().strip_prefix('"').and_then(|p| p.strip_suffix('"')))
        .map(|p| p.to_string())
        .collect()
}

pub fn parse_control(text: &str) -> Option<Control> {
    let kind = match json_get(text, "kind") {
        Some("settings") => ControlKind::Settings,
        _ => ControlKind::Scene,
    };
    // an older binary's settings message also carries pixels/detail/speed/
    // theme/text_scale; those are sim settings now and only travel in the
    // anchor, so a legacy message contributes its appearance fields only
    let settings = (kind == ControlKind::Settings).then(|| SettingsMsg::parse_fields(text));
    Some(Control {
        kind,
        settings,
        scene: json_get(text, "scene").unwrap_or("").to_string(),
        theme: json_get(text, "theme").map(|s| s.to_string()),
        epoch: json_get(text, "epoch")?.parse().ok()?,
        seq: json_get(text, "seq")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        from_pid: json_get(text, "from_pid")?.parse().ok()?,
        seed: json_get(text, "seed")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        t0_ms: json_get(text, "t0_ms")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        proto: json_get(text, "proto").and_then(|v| v.parse().ok()).unwrap_or(0),
    })
}

pub fn parse_instance(text: &str) -> Option<InstanceInfo> {
    let geo = if text.contains("\"geo\"") {
        Some((
            json_get(text, "x")?.parse().ok()?,
            json_get(text, "y")?.parse().ok()?,
            json_get(text, "w")?.parse().ok()?,
            json_get(text, "h")?.parse().ok()?,
        ))
    } else {
        None
    };
    Some(InstanceInfo {
        pid: json_get(text, "pid")?.parse().ok()?,
        scene: json_get(text, "scene")?.to_string(),
        group: json_get(text, "group")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "default".into()),
        started_at: json_get(text, "started_at_epoch_secs")?.parse().ok()?,
        cols: json_get(text, "cols")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        rows: json_get(text, "rows")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        geo,
        pad: {
            let f = |exact: &str, whole: &str| {
                json_get(text, exact)
                    .or_else(|| json_get(text, whole))
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .unwrap_or(0.0)
            };
            (f("padx", "px"), f("pady", "py"))
        },
        cell: json_get(text, "cw")
            .zip(json_get(text, "ch"))
            .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
            .filter(|(w, h)| w.is_finite() && h.is_finite() && *w >= 1.0 && *h >= 1.0),
        placement: json_get(text, "place").and_then(crate::wall::Placement::parse).unwrap_or_default(),
        // older instance files predate the flag: treat them as anchored
        synced: json_get(text, "synced").map(|v| v != "false").unwrap_or(true),
        reanchor: json_get(text, "reanchor").and_then(Stamp::decode),
        proto: json_get(text, "proto").and_then(|v| v.parse().ok()).unwrap_or(0),
    })
}

/// Remove registry files whose pids are dead, temp-file orphans from
/// crashed publishers, and a control.json left behind by a dead group.
pub fn reap_stale(dir: &PathBuf) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut any_live_inst = false;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(rest) = name.strip_prefix("inst-") {
            if let Some(pid_s) = rest.strip_suffix(".json") {
                if let Ok(pid) = pid_s.parse::<u32>() {
                    if pid != std::process::id() && !pid_alive(pid) {
                        let _ = std::fs::remove_file(e.path());
                    } else {
                        any_live_inst = true;
                    }
                }
            }
        } else if name.ends_with(".tmp") {
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age.as_secs() > 60);
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    // a control file with no living publisher and no live peers is a relic
    // of a dead session; drop it so it can't confuse the next launch. The
    // anchor and settings persist exactly as long as the group has a member.
    if !any_live_inst {
        let _ = std::fs::remove_file(dir.join("scene.json"));
        for relic in ["anchor.json", "settings.json"] {
            let path = dir.join(relic);
            let publisher_dead = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| json_get(&t, "from_pid").and_then(|v| v.parse::<u32>().ok()))
                .is_none_or(|pid| !pid_alive(pid));
            if publisher_dead {
                let _ = std::fs::remove_file(path);
            }
        }
        let ctrl = dir.join("control.json");
        if let Ok(text) = std::fs::read_to_string(&ctrl) {
            let publisher_dead = json_get(&text, "from_pid")
                .and_then(|v| v.parse::<u32>().ok())
                .is_none_or(|pid| !pid_alive(pid));
            if publisher_dead {
                let _ = std::fs::remove_file(ctrl);
            }
        }
    }
}

fn scan_group_dir(dir: &PathBuf, group: &str) -> Vec<InstanceInfo> {
    reap_stale(dir);
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("inst-") && name.ends_with(".json") {
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                if let Some(mut info) = parse_instance(&text) {
                    if info.group.is_empty() {
                        info.group = group.to_string();
                    }
                    if pid_alive(info.pid) {
                        out.push(info);
                    }
                }
            }
        }
    }
    out
}

/// Live instances in one link group.
pub fn list_instances_in_group(group: &str) -> Vec<InstanceInfo> {
    let Some(dir) = group_dir(group) else {
        return Vec::new();
    };
    let mut out = scan_group_dir(&dir, group);
    out.sort_by_key(|i| i.pid);
    out
}

/// List live instances in the default group.
pub fn list_instances() -> Vec<InstanceInfo> {
    list_instances_in_group("default")
}

/// Every live instance across all link groups.
pub fn list_all_instances() -> Vec<InstanceInfo> {
    let mut out = list_instances_in_group("default");
    if let Some(base) = registry_base() {
        let groups_root = base.join("groups");
        if let Ok(entries) = std::fs::read_dir(groups_root) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let name = e.file_name().to_string_lossy().to_string();
                    out.extend(scan_group_dir(&e.path(), &name));
                }
            }
        }
    }
    out.sort_by_key(|i| i.pid);
    out
}

/// Publish a scene switch to one link group (the `--switch` remote): a new
/// anchor for `scene`, keeping the group's current sim settings (or
/// `defaults` when the group has no anchor yet), plus the legacy mirror.
pub fn publish_remote(scene: &str, seed: u64, t0_ms: u64, group: &str, defaults: &Anchor) -> std::io::Result<()> {
    let Some(dir) = group_dir(group) else {
        return Ok(());
    };
    publish_remote_in(&dir, scene, seed, t0_ms, defaults)
}

pub fn publish_remote_in(dir: &Path, scene: &str, seed: u64, t0_ms: u64, defaults: &Anchor) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let base = std::fs::read_to_string(dir.join("anchor.json"))
        .ok()
        .and_then(|t| Anchor::parse(&t))
        .unwrap_or_else(|| defaults.clone());
    let a = Anchor {
        stamp: Stamp { epoch: epoch_now_ms(), seq: 0, from_pid: std::process::id() },
        scene: scene.to_string(),
        // the scene's own default theme: the remote knows no per-scene memory
        theme: None,
        seed,
        t0_ms,
        paused_at_ms: None,
        proto: PROTO,
        ..base
    };
    atomic_write(&dir.join("anchor.json"), &a.to_json())?;
    atomic_write(&dir.join("control.json"), &a.legacy_json())
}

/// Names of all link groups (presets plus any on disk).
pub fn all_group_names() -> Vec<String> {
    let mut names: Vec<String> = GROUP_PRESETS.iter().map(|s| (*s).to_string()).collect();
    if let Some(base) = registry_base() {
        let groups_root = base.join("groups");
        if let Ok(entries) = std::fs::read_dir(groups_root) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    names.push(e.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

pub fn publish_remote_all_groups(scene: &str, seed: u64, t0_ms: u64, defaults: &Anchor) -> std::io::Result<()> {
    for g in all_group_names() {
        publish_remote(scene, seed, t0_ms, &g, defaults)?;
    }
    Ok(())
}

pub fn uptime_secs(started_at: u64) -> u64 {
    (epoch_now_ms() / 1000).saturating_sub(started_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // env vars are process-global: serialize these tests
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn anchor(scene: &str, seed: u64, t0: u64) -> Anchor {
        Anchor {
            stamp: Stamp::default(),
            scene: scene.into(),
            theme: Some("mono".into()),
            seed,
            t0_ms: t0,
            paused_at_ms: None,
            speed: 1.5,
            detail: "high".into(),
            pixels: "braille".into(),
            text_scale: Some(2),
            proto: PROTO,
        }
    }

    /// A private registry dir: link tests never touch $XDG_RUNTIME_DIR.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termpaper-link-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_settings_publish_cannot_hide_an_anchor() {
        let dir = temp_dir("hide");
        let mut guard = Guard::new_in(dir.clone(), "rain", "default").unwrap();
        let mut a = anchor("rain", 123, 456);
        guard.publish_anchor(&mut a);
        // settings published in the same frame, both ways round
        guard.publish_settings(&SettingsMsg::default());
        let mut peer = Guard::new_in(dir.clone(), "fire", "default").unwrap();
        peer.pid = 1; // a different instance sharing the dir
        let got = peer.latest_anchor(&anchor("x", 0, 0)).expect("anchor survives");
        assert_eq!(got, a);
        guard.publish_settings(&SettingsMsg { dim: 0.5, ..Default::default() });
        guard.publish_anchor(&mut anchor("fire", 9, 10));
        guard.publish_settings(&SettingsMsg { dim: 0.25, ..Default::default() });
        let polled = peer.poll_anchor().expect("the newer anchor");
        assert_eq!((polled.scene.as_str(), polled.seed), ("fire", 9));
        assert_eq!(peer.poll_settings().map(|m| m.dim), Some(0.25));
        assert!(peer.poll_anchor().is_none(), "an unchanged anchor is reported once");
        // legacy readers still find the scene through scene.json
        let legacy = guard.latest_scene().unwrap();
        assert_eq!((legacy.scene.as_str(), legacy.seed, legacy.t0_ms), ("fire", 9, 10));
        drop((peer, guard));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn anchor_round_trip() {
        let mut a = anchor("lanterns", u64::MAX - 7, 1_700_000_000_123);
        a.stamp = Stamp { epoch: 1_700_000_000_000, seq: 42, from_pid: 777 };
        a.paused_at_ms = Some(1_700_000_004_000);
        let back = Anchor::parse(&a.to_json()).expect("parses");
        assert_eq!(back, a);
        let mut plain = anchor("rain", 1, 2);
        plain.theme = None;
        plain.text_scale = None;
        plain.speed = 0.1 + 0.2; // not a short decimal: must still survive exactly
        assert_eq!(Anchor::parse(&plain.to_json()).unwrap(), plain);
        assert!(Anchor::parse("{\"kind\":\"scene\",\"epoch\":1}").is_none());
    }

    #[test]
    fn classify_retime_and_switch() {
        let a = anchor("rain", 5, 1000);
        let mut paused = a.clone();
        paused.pause(4000);
        assert_eq!(classify(&a, &a), AnchorChange::Same);
        assert_eq!(classify(&a, &paused), AnchorChange::Retime);
        let mut resumed = paused.clone();
        resumed.resume(9000);
        assert_eq!(resumed.t0_ms, 6000, "resume shifts t0 by the paused time");
        assert_eq!(resumed.elapsed_at(9000), paused.elapsed_at(123_456), "the clock continues where it stopped");
        assert_eq!(classify(&paused, &resumed), AnchorChange::Retime);
        let changes: [fn(&mut Anchor); 7] = [
            |x| x.speed = 2.0,
            |x| x.detail = "low".into(),
            |x| x.pixels = "half".into(),
            |x| x.text_scale = None,
            |x| x.theme = None,
            |x| x.seed = 6,
            |x| x.scene = "fire".into(),
        ];
        for change in changes {
            let mut b = a.clone();
            change(&mut b);
            assert_eq!(classify(&a, &b), AnchorChange::Switch);
        }
    }

    #[test]
    fn remote_switch_keeps_the_group_sim_settings() {
        let dir = temp_dir("remote");
        let defaults = anchor("x", 0, 0);
        publish_remote_in(&dir, "fire", 3, 4, &defaults).unwrap();
        let first = Anchor::parse(&std::fs::read_to_string(dir.join("anchor.json")).unwrap()).unwrap();
        assert_eq!((first.scene.as_str(), first.seed, first.speed), ("fire", 3, 1.5));
        // a group anchor exists now: its settings win over the defaults
        let mut g = Guard::new_in(dir.clone(), "fire", "default").unwrap();
        let mut slow = anchor("fire", 3, 4);
        slow.speed = 0.5;
        g.publish_anchor(&mut slow);
        publish_remote_in(&dir, "rain", 8, 9, &defaults).unwrap();
        let mut peer = Guard::new_in(dir.clone(), "x", "default").unwrap();
        peer.pid = 2;
        let got = peer.latest_anchor(&defaults).unwrap();
        assert_eq!((got.scene.as_str(), got.seed, got.t0_ms, got.speed), ("rain", 8, 9, 0.5));
        drop((g, peer));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_scene_messages_become_anchors() {
        let dir = temp_dir("legacy");
        let mut g = Guard::new_in(dir.clone(), "rain", "default").unwrap();
        // an older binary's message: no proto marker
        std::fs::write(
            dir.join("control.json"),
            "{\"kind\":\"scene\",\"scene\":\"fire\",\"theme\":null,\"epoch\":100,\"seq\":1,\"from_pid\":12345,\"seed\":42,\"t0_ms\":90}",
        )
        .unwrap();
        let sim = anchor("rain", 1, 1);
        let a = g.latest_anchor(&sim).unwrap();
        assert_eq!((a.scene.as_str(), a.seed, a.t0_ms), ("fire", 42, 90));
        assert_eq!((a.speed, a.pixels.as_str()), (1.5, "braille"), "sim settings come from the local anchor");
        let c = g.poll_control(Stamp::default()).expect("a legacy message is live");
        assert_eq!(c.proto, 0);
        // a mirror written by a new binary carries the marker and is skipped
        g.pid = 777;
        g.publish_anchor(&mut anchor("rain", 5, 6));
        g.pid = std::process::id();
        assert!(g.poll_control(Stamp::default()).is_none(), "mirrors are not legacy messages");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reanchor_requests_round_trip_through_the_inst_file() {
        let dir = temp_dir("reanchor");
        let mut g = Guard::new_in(dir.clone(), "rain", "default").unwrap();
        let s = Stamp { epoch: 1_700_000_000_000, seq: 3, from_pid: 99 };
        g.set_reanchor(Some(s));
        let inst = dir.join(format!("inst-{}.json", g.pid));
        let info = parse_instance(&std::fs::read_to_string(&inst).unwrap()).unwrap();
        assert_eq!(info.reanchor, Some(s));
        assert_eq!(info.proto, PROTO);
        g.set_reanchor(None);
        assert_eq!(parse_instance(&std::fs::read_to_string(&inst).unwrap()).unwrap().reanchor, None);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inst_files_carry_exact_padding_cells_and_placement() {
        let dir = temp_dir("layout");
        let mut g = Guard::new_in(dir.clone(), "rain", "default").unwrap();
        g.set_layout((4.6666665, 2.0), crate::wall::Placement::Center, Some((9.0, 20.0)));
        let text = std::fs::read_to_string(dir.join(format!("inst-{}.json", g.pid))).unwrap();
        let info = parse_instance(&text).unwrap();
        assert_eq!(info.pad, (4.6666665, 2.0));
        assert_eq!(info.cell, Some((9.0, 20.0)));
        assert_eq!(info.placement, crate::wall::Placement::Center);
        // older binaries read whole px from px/py
        assert!(text.contains("\"px\":5,") && text.contains("\"py\":2,"), "{text}");
        // and an older binary's file (whole px, no cells) still parses
        let old = parse_instance("{\"pid\":7,\"scene\":\"fire\",\"started_at_epoch_secs\":1,\"px\":7,\"py\":3}").unwrap();
        assert_eq!((old.pad, old.cell, old.placement), ((7.0, 3.0), None, crate::wall::Placement::TopLeft));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn with_registry<F: FnOnce(PathBuf)>(f: F) {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("termpaper-link-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
        f(dir.join("termpaper"));
        std::env::remove_var("XDG_RUNTIME_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_write_list_reap_round_trip() {
        with_registry(|dir| {
            let g = Guard::new("rain", "default").expect("guard");
            assert!(dir.join(format!("inst-{}.json", g.pid)).exists());

            // a stale entry from a dead pid gets reaped on next list
            let stale = dir.join("inst-99999999.json");
            std::fs::write(
                &stale,
                "{\"pid\":99999999,\"scene\":\"fire\",\"started_at_epoch_secs\":1}",
            )
            .unwrap();
            let list = list_instances();
            assert!(!stale.exists(), "stale entry should be reaped");
            assert!(list.iter().any(|i| i.pid == g.pid));
            assert!(list.iter().all(|i| i.pid != 99999999));
            let ours = list.iter().find(|i| i.pid == g.pid).unwrap();
            assert_eq!(ours.scene, "rain");
            let pid = g.pid;
            drop(g);
            assert!(!dir.join(format!("inst-{pid}.json")).exists());
        });
    }

    #[test]
    fn pid_alive_detects_self_and_rejects_bogus() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(0x7FFF_FFFF));
    }

    #[test]
    fn stamp_ordering() {
        // same-ms different-seq: higher seq wins
        let a = Stamp { epoch: 100, seq: 1, from_pid: 1 };
        let b = Stamp { epoch: 100, seq: 2, from_pid: 1 };
        assert!(b > a);
        // same epoch, same seq: tie broken by pid
        let c = Stamp { epoch: 100, seq: 2, from_pid: 2 };
        assert!(c > b);
        // newer epoch beats everything
        let d = Stamp { epoch: 101, seq: 0, from_pid: 1 };
        assert!(d > c);
        assert!(Stamp::default() < a);
    }

    #[test]
    fn control_epoch_ordering_and_own_pid() {
        let _ = epoch_now_ms();
        with_registry(|dir| {
            let mut g = Guard::new("rain", "default").expect("guard");
            // a message from a different (fake) pid
            std::fs::write(
                dir.join("control.json"),
                "{\"scene\":\"fire\",\"theme\":\"frost\",\"epoch\":100,\"seq\":1,\"from_pid\":12345,\"seed\":42,\"t0_ms\":90}",
            )
            .unwrap();
            let msg = "{\"scene\":\"fire\",\"theme\":\"frost\",\"epoch\":100,\"seq\":1,\"from_pid\":12345,\"seed\":42,\"t0_ms\":90}";
            let c = g.poll_control(Stamp::default()).expect("should see it");
            assert_eq!(c.scene, "fire");
            assert_eq!(c.theme.as_deref(), Some("frost"));
            assert_eq!(c.seed, 42);
            assert_eq!(c.t0_ms, 90);
            // unchanged file: the (ino, mtime) gate skips the re-read
            assert!(g.poll_control(Stamp::default()).is_none(), "unchanged file must not re-apply");
            // older-or-equal stamp ignored (rewrite so the gate re-reads)
            let applied = Stamp { epoch: 100, seq: 1, from_pid: 12345 };
            touch_control(&dir, msg);
            assert!(g.poll_control(applied).is_none());
            touch_control(&dir, msg);
            assert!(g.poll_control(Stamp { from_pid: 99999, ..applied }).is_none());
            // a stamp taken at launch time filters out any pre-launch relic
            let launch = Stamp { epoch: epoch_now_ms(), seq: 0, from_pid: 0 };
            touch_control(&dir, msg);
            assert!(g.poll_control(launch).is_none(), "pre-launch message must be ignored");
            // own messages ignored
            let mut own = anchor("rain", 1, 2);
            g.publish_anchor(&mut own);
            assert!(g.poll_control(Stamp::default()).is_none(), "own pid must be ignored");
        });
    }

    /// Rewrite control.json ensuring its (ino, mtime) signature changes even
    /// on filesystems with coarse timestamps.
    fn touch_control(dir: &PathBuf, msg: &str) {
        let path = dir.join("control.json");
        let before = std::fs::metadata(&path).ok().map(|m| (file_ino(&m), m.modified().ok()));
        for _ in 0..1000 {
            let _ = std::fs::remove_file(&path);
            std::fs::write(&path, msg).unwrap();
            let now = std::fs::metadata(&path).ok().map(|m| (file_ino(&m), m.modified().ok()));
            if now != before {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("could not produce a distinct control.json signature");
    }

    #[test]
    fn atomic_write_tmp_is_pid_scoped_and_cleaned() {
        with_registry(|dir| {
            std::fs::create_dir_all(&dir).unwrap();
            let target = dir.join("control.json");
            atomic_write(&target, "{\"x\":1}").unwrap();
            assert_eq!(std::fs::read_to_string(&target).unwrap(), "{\"x\":1}");
            // no temp files remain, and the naming is pid-scoped
            let leftovers: Vec<String> = std::fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".tmp"))
                .collect();
            assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");
        });
    }

    #[test]
    fn reap_removes_orphaned_control_of_dead_group() {
        with_registry(|dir| {
            std::fs::create_dir_all(&dir).unwrap();
            // control.json from a dead publisher, no live instances
            std::fs::write(
                dir.join("control.json"),
                "{\"scene\":\"fire\",\"epoch\":1,\"seq\":1,\"from_pid\":99999999,\"seed\":0,\"t0_ms\":0}",
            )
            .unwrap();
            reap_stale(&dir);
            assert!(!dir.join("control.json").exists(), "orphaned control.json should be reaped");

            // but with a live instance present it must survive
            let _g = Guard::new("rain", "default").expect("guard");
            std::fs::write(
                dir.join("control.json"),
                "{\"scene\":\"fire\",\"epoch\":1,\"seq\":1,\"from_pid\":99999999,\"seed\":0,\"t0_ms\":0}",
            )
            .unwrap();
            reap_stale(&dir);
            assert!(dir.join("control.json").exists(), "control.json with live peers must survive");
        });
    }

    #[test]
    fn instance_file_parsing() {
        let info = parse_instance(
            "{\"pid\":4242,\"scene\":\"fire\",\"started_at_epoch_secs\":1700000000}",
        )
        .unwrap();
        assert_eq!(info.pid, 4242);
        assert_eq!(info.scene, "fire");
        assert!(uptime_secs(info.started_at) > 0);
        assert!(info.synced, "missing synced flag defaults to anchored");
        assert!(parse_instance("garbage").is_none());
        let cycled = parse_instance(
            "{\"pid\":1,\"scene\":\"fire\",\"started_at_epoch_secs\":1700000000,\"synced\":false}",
        )
        .unwrap();
        assert!(!cycled.synced);
    }
}

#[cfg(test)]
mod settings_sync_tests {
    use super::*;

    #[test]
    fn settings_message_round_trip() {
        let _g = settings_sync_tests_lock();
        let m = SettingsMsg {
            filters: vec!["scanlines".into(), "vignette".into()],
            fps: 48,
            smooth: 0.45,
            dim: 0.8,
            fade: 0.5,
            clock: false,
            quick: Some("vignette".into()),
            hue_shift: 45.0,
            saturation: 1.4,
            contrast: 1.2,
        };
        let text = format!("{{\"kind\":\"settings\",{},\"epoch\":7,\"seq\":2,\"from_pid\":5}}", m.fields_json());
        let c = parse_control(&text).unwrap();
        assert_eq!(c.kind, ControlKind::Settings);
        assert_eq!(c.settings.unwrap(), m);
        // an older binary's settings message: only its appearance fields count
        let old = parse_control(
            "{\"kind\":\"settings\",\"pixels\":\"braille\",\"detail\":\"high\",\"filters\":[\"crt\"],\"theme\":\"amber\",\"text_scale\":3,\"speed\":1.5,\"fps\":48,\"smooth\":0.45,\"dim\":0.8,\"fade\":0.5,\"clock\":false,\"quick\":null,\"hue_shift\":45,\"saturation\":1.4,\"contrast\":1.2,\"epoch\":7,\"seq\":2,\"from_pid\":5}",
        )
        .unwrap();
        assert_eq!(old.proto, 0);
        let back = old.settings.unwrap();
        assert_eq!(back.filters, vec!["crt"]);
        assert_eq!(back.fps, 48);
        assert!((back.hue_shift - 45.0).abs() < 1e-6);
        // scene messages still parse with default kind
        let sc = parse_control(
            "{\"kind\":\"scene\",\"scene\":\"fire\",\"theme\":null,\"epoch\":1,\"seq\":1,\"from_pid\":2,\"seed\":9,\"t0_ms\":8}",
        )
        .unwrap();
        assert_eq!(sc.kind, ControlKind::Scene);
        assert!(sc.settings.is_none());
    }

    fn settings_sync_tests_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
        L.lock().unwrap_or_else(|e| e.into_inner())
    }
}
