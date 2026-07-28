//! Instance linking: file-based discovery and control between termpaper
//! instances in different terminals. Registry dir holds one inst-<pid>.json
//! per instance plus a shared control.json channel. Everything degrades
//! silently to "disabled" when the filesystem says no.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlKind {
    Scene,
    Settings,
}

/// A settings blob broadcast alongside scene switches.
#[derive(Clone, Default)]
pub struct SettingsMsg {
    pub pixels: String,
    pub detail: String,
    pub filters: Vec<String>,
    pub theme: Option<String>,
    pub text_scale: Option<u32>,
    pub speed: f32,
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
}

/// Total ordering for control messages: newer wins lexicographically,
/// ties broken deterministically by pid.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Stamp {
    pub epoch: u64,
    pub seq: u64,
    pub from_pid: u32,
}

pub struct InstanceInfo {
    pub pid: u32,
    pub scene: String,
    pub group: String,
    pub started_at: u64,
    pub cols: usize,
    pub rows: usize,
    pub geo: Option<(i32, i32, i32, i32)>,
    /// terminal padding in px (x, y) — the cell grid is inset by this much
    /// inside the window geometry, so wall crops must account for it
    pub pad: (i32, i32),
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

#[cfg(not(target_os = "linux"))]
fn libc_getuid() -> u32 {
    1000
}

fn pid_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

fn atomic_write(path: &PathBuf, contents: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
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
    pad: (i32, i32),
}

impl Guard {
    pub fn new(scene: &str, group: &str) -> Option<Self> {
        let group = sanitize_group(group);
        let dir = group_dir(&group)?;
        std::fs::create_dir_all(&dir).ok()?;
        reap_stale(&dir);
        let pid = std::process::id();
        let g = Guard {
            dir,
            pid,
            group,
            scene: scene.to_string(),
            started_at: epoch_now_ms() / 1000,
            seq: 0,
            cols: 0,
            rows: 0,
            geo: None,
            pad: (0, 0),
        };
        g.write()?;
        Some(g)
    }

    fn path(&self) -> PathBuf {
        self.dir.join(format!("inst-{}.json", self.pid))
    }

    fn write(&self) -> Option<()> {
        let geo_json = match self.geo {
            Some((x, y, w, h)) => format!(",\"geo\":{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}"),
            None => String::new(),
        };
        atomic_write(
            &self.path(),
            &format!(
                "{{\"pid\":{},\"scene\":\"{}\",\"group\":\"{}\",\"started_at_epoch_secs\":{},\"cols\":{},\"rows\":{},\"px\":{},\"py\":{},\"version\":\"{}\"{}}}",
                self.pid,
                esc(&self.scene),
                esc(&self.group),
                self.started_at,
                self.cols,
                self.rows,
                self.pad.0,
                self.pad.1,
                env!("CARGO_PKG_VERSION"),
                geo_json,
            ),
        )
        .ok()
    }

    /// Record terminal padding in px (set once at startup from --pad/config).
    pub fn set_pad(&mut self, pad: (i32, i32)) {
        if self.pad != pad {
            self.pad = pad;
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

    /// Check control.json for a message newer than `last` (and not ours).
    pub fn poll_control(&self, last: Stamp) -> Option<Control> {
        let path = self.dir.join("control.json");
        let text = std::fs::read_to_string(path).ok()?;
        parse_control(&text).filter(|c| {
            c.from_pid != self.pid
                && Stamp {
                    epoch: c.epoch,
                    seq: c.seq,
                    from_pid: c.from_pid,
                } > last
        })
    }

    /// Publish a scene switch to all other instances, with artwork-sync
    /// parameters: the scene's rng seed and its start timestamp.
    pub fn publish(&mut self, scene: &str, theme: Option<&str>, seed: u64, t0_ms: u64) {
        self.seq += 1;
        let theme_json = match theme {
            Some(t) => format!("\"{}\"", esc(t)),
            None => "null".to_string(),
        };
        let _ = atomic_write(
            &self.dir.join("control.json"),
            &format!(
                "{{\"kind\":\"scene\",\"scene\":\"{}\",\"theme\":{},\"epoch\":{},\"seq\":{},\"from_pid\":{},\"seed\":{},\"t0_ms\":{}}}",
                esc(scene),
                theme_json,
                epoch_now_ms(),
                self.seq,
                self.pid,
                seed,
                t0_ms,
            ),
        );
    }

    /// Broadcast a settings blob to all other instances.
    pub fn publish_settings(&mut self, m: &SettingsMsg) {
        self.seq += 1;
        let theme_json = match &m.theme {
            Some(t) => format!("\"{}\"", esc(t)),
            None => "null".to_string(),
        };
        let filters_json = m
            .filters
            .iter()
            .map(|f| format!("\"{}\"", esc(f)))
            .collect::<Vec<_>>()
            .join(",");
        let ts_json = match m.text_scale {
            Some(t) => t.to_string(),
            None => "null".to_string(),
        };
        let quick_json = match &m.quick {
            Some(q) => format!("\"{}\"", esc(q)),
            None => "null".to_string(),
        };
        let _ = atomic_write(
            &self.dir.join("control.json"),
            &format!(
                "{{\"kind\":\"settings\",\"pixels\":\"{}\",\"detail\":\"{}\",\"filters\":[{}],\"theme\":{},\"text_scale\":{},\"speed\":{},\"fps\":{},\"smooth\":{},\"dim\":{},\"fade\":{},\"clock\":{},\"quick\":{},\"hue_shift\":{},\"saturation\":{},\"contrast\":{},\"epoch\":{},\"seq\":{},\"from_pid\":{},\"seed\":0,\"t0_ms\":0}}",
                esc(&m.pixels),
                esc(&m.detail),
                filters_json,
                theme_json,
                ts_json,
                m.speed,
                m.fps,
                m.smooth,
                m.dim,
                m.fade,
                m.clock,
                quick_json,
                m.hue_shift,
                m.saturation,
                m.contrast,
                epoch_now_ms(),
                self.seq,
                self.pid,
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
    let settings = if kind == ControlKind::Settings {
        Some(SettingsMsg {
            pixels: json_get(text, "pixels").unwrap_or("quad").to_string(),
            detail: json_get(text, "detail").unwrap_or("medium").to_string(),
            filters: parse_str_array(text, "filters"),
            theme: json_get(text, "theme").map(|s| s.to_string()),
            text_scale: json_get(text, "text_scale").and_then(|v| v.parse().ok()),
            speed: json_get(text, "speed")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            fps: json_get(text, "fps")
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            smooth: json_get(text, "smooth")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3),
            dim: json_get(text, "dim")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            fade: json_get(text, "fade")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.25),
            clock: json_get(text, "clock")
                .map(|v| v == "true")
                .unwrap_or(true),
            quick: json_get(text, "quick").map(|s| s.to_string()),
            hue_shift: json_get(text, "hue_shift")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            saturation: json_get(text, "saturation")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            contrast: json_get(text, "contrast")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
        })
    } else {
        None
    };
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
        pad: (
            json_get(text, "px").and_then(|v| v.parse().ok()).unwrap_or(0),
            json_get(text, "py").and_then(|v| v.parse().ok()).unwrap_or(0),
        ),
    })
}

/// Remove registry files whose pids are dead.
pub fn reap_stale(dir: &PathBuf) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(rest) = name.strip_prefix("inst-") {
            if let Some(pid_s) = rest.strip_suffix(".json") {
                if let Ok(pid) = pid_s.parse::<u32>() {
                    if pid != std::process::id() && !pid_alive(pid) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
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

/// Publish a scene switch to one link group (the `--switch` remote).
pub fn publish_remote(scene: &str, seed: u64, t0_ms: u64, group: &str) -> std::io::Result<()> {
    let Some(dir) = group_dir(group) else {
        return Ok(());
    };
    std::fs::create_dir_all(&dir)?;
    atomic_write(
        &dir.join("control.json"),
        &format!(
            "{{\"scene\":\"{}\",\"theme\":null,\"epoch\":{},\"seq\":0,\"from_pid\":{},\"seed\":{},\"t0_ms\":{}}}",
            esc(scene),
            epoch_now_ms(),
            std::process::id(),
            seed,
            t0_ms,
        ),
    )
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

pub fn publish_remote_all_groups(scene: &str, seed: u64, t0_ms: u64) -> std::io::Result<()> {
    for g in all_group_names() {
        publish_remote(scene, seed, t0_ms, &g)?;
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
            let c = g.poll_control(Stamp::default()).expect("should see it");
            assert_eq!(c.scene, "fire");
            assert_eq!(c.theme.as_deref(), Some("frost"));
            assert_eq!(c.seed, 42);
            assert_eq!(c.t0_ms, 90);
            // older-or-equal stamp ignored
            let applied = Stamp { epoch: 100, seq: 1, from_pid: 12345 };
            assert!(g.poll_control(applied).is_none());
            assert!(g.poll_control(Stamp { from_pid: 99999, ..applied }).is_none());
            // own messages ignored
            g.publish("rain", None, 1, 2);
            assert!(g.poll_control(Stamp::default()).is_none(), "own pid must be ignored");
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
        assert!(parse_instance("garbage").is_none());
    }
}

#[cfg(test)]
mod settings_sync_tests {
    use super::*;

    #[test]
    fn settings_message_round_trip() {
        let _g = settings_sync_tests_lock();
        let m = SettingsMsg {
            pixels: "braille".into(),
            detail: "high".into(),
            filters: vec!["scanlines".into(), "vignette".into()],
            theme: Some("amber".into()),
            text_scale: Some(3),
            speed: 1.5,
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
        // serialize the same way publish_settings does
        let filters_json = m
            .filters
            .iter()
            .map(|f| format!("\"{f}\""))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!(
            "{{\"kind\":\"settings\",\"pixels\":\"{}\",\"detail\":\"{}\",\"filters\":[{}],\"theme\":\"amber\",\"text_scale\":3,\"speed\":1.5,\"fps\":48,\"smooth\":0.45,\"dim\":0.8,\"fade\":0.5,\"clock\":false,\"quick\":\"vignette\",\"hue_shift\":45,\"saturation\":1.4,\"contrast\":1.2,\"epoch\":7,\"seq\":2,\"from_pid\":5}}",
            m.pixels, m.detail, filters_json
        );
        let c = parse_control(&text).unwrap();
        assert_eq!(c.kind, ControlKind::Settings);
        let back = c.settings.unwrap();
        assert_eq!(back.pixels, "braille");
        assert_eq!(back.detail, "high");
        assert_eq!(back.filters, vec!["scanlines", "vignette"]);
        assert_eq!(back.theme.as_deref(), Some("amber"));
        assert_eq!(back.text_scale, Some(3));
        assert_eq!(back.fps, 48);
        assert!((back.smooth - 0.45).abs() < 1e-6);
        assert!((back.dim - 0.8).abs() < 1e-6);
        assert!((back.fade - 0.5).abs() < 1e-6);
        assert!(!back.clock);
        assert_eq!(back.quick.as_deref(), Some("vignette"));
        assert!((back.hue_shift - 45.0).abs() < 1e-6);
        assert!((back.saturation - 1.4).abs() < 1e-6);
        assert!((back.contrast - 1.2).abs() < 1e-6);
        assert!((back.speed - 1.5).abs() < 1e-6);
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
