//! Hyprland queries over the compositor's IPC socket.
//!
//! One request/response per call on `$XDG_RUNTIME_DIR/hypr/$SIG/.socket.sock`
//! (the same thing `hyprctl` does, minus spawning a process), falling back to
//! the `hyprctl` binary. Only the fields termpaper needs are decoded; unknown
//! fields are ignored so newer compositors keep working.
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HyprMonitor {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// mode size in device pixels, before rotation
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub refresh_rate: f64,
    /// position in the logical (scaled, rotated) layout
    pub x: i32,
    pub y: i32,
    #[serde(default = "one")]
    pub scale: f64,
    /// wl_output transform: 0 normal, 1 90°, 2 180°, 3 270°, 4-7 flipped
    #[serde(default)]
    pub transform: u8,
    /// EDID size in millimetres, before rotation (0 when unknown)
    #[serde(default)]
    pub physical_width: i32,
    #[serde(default)]
    pub physical_height: i32,
    /// reserved edges in logical px: left, top, right, bottom
    #[serde(default)]
    pub reserved: [i32; 4],
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct HyprClient {
    /// the window's address, `0x…`
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub pid: i64,
    pub at: [i32; 2],
    pub size: [i32; 2],
    #[serde(default)]
    pub monitor: i64,
    #[serde(default)]
    pub class: String,
    #[serde(default = "yes")]
    pub mapped: bool,
    #[serde(default)]
    pub hidden: bool,
    /// 0 windowed, 1 maximised, 2 fullscreen (a bool before Hyprland 0.42)
    #[serde(default, deserialize_with = "fullscreen_level")]
    pub fullscreen: u8,
}

fn yes() -> bool {
    true
}

fn fullscreen_level<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Bool(b) => 2 * b as u8,
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0).min(2) as u8,
        _ => 0,
    })
}

/// The instance signature: from the environment, or the only instance
/// directory under `$XDG_RUNTIME_DIR/hypr` (detached shells lose the env).
fn signature() -> Option<String> {
    if let Ok(s) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if !s.is_empty() {
            return Some(s);
        }
    }
    let rd = std::fs::read_dir(PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("hypr")).ok()?;
    let dirs: Vec<_> = rd.flatten().filter(|e| e.path().is_dir()).collect();
    (dirs.len() == 1).then(|| dirs[0].file_name().to_string_lossy().into_owned())
}

/// Whether a Hyprland session is reachable from here. Cheap (no process
/// spawn), so callers check it before asking anything: on macOS, Windows or
/// another compositor every query would otherwise try to spawn `hyprctl`.
pub fn present() -> bool {
    signature().is_some()
}

#[cfg(unix)]
fn via_socket(request: &str) -> Option<String> {
    use std::io::{Read, Write};
    use std::time::Duration;
    let sig = signature()?;
    let path = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?)
        .join("hypr")
        .join(sig)
        .join(".socket.sock");
    let mut s = std::os::unix::net::UnixStream::connect(path).ok()?;
    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = s.set_write_timeout(Some(Duration::from_millis(500)));
    s.write_all(request.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Background interaction sampling uses only the compositor socket. If IPC
/// is unavailable it skips the sample; it must never spawn `hyprctl`.
pub(crate) fn socket_json(what: &str) -> Option<String> {
    via_socket(&format!("j/{what}"))
}

#[cfg(unix)]
pub struct EventStream {
    stream: std::os::unix::net::UnixStream,
    pending: Vec<u8>,
}

#[cfg(unix)]
impl EventStream {
    pub fn connect() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?)
            .join("hypr").join(signature()?).join(".socket2.sock");
        let stream = std::os::unix::net::UnixStream::connect(path).ok()?;
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(100)));
        Some(Self { stream, pending: Vec::new() })
    }

    /// None means the read timed out; a closed socket reconnects on the next
    /// loop iteration. Unknown event lines can safely be ignored.
    pub fn next_line(&mut self) -> Result<Option<String>, ()> {
        use std::io::Read;
        loop {
            if let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
                let line = self.pending.drain(..=end).collect::<Vec<_>>();
                return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
            }
            let mut buf = [0u8; 1024];
            match self.stream.read(&mut buf) {
                Ok(0) => return Err(()),
                Ok(n) => self.pending.extend_from_slice(&buf[..n]),
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return Ok(None),
                Err(_) => return Err(()),
            }
            if self.pending.len() > 8192 { self.pending.clear(); }
        }
    }
}

#[cfg(not(unix))]
pub struct EventStream;
#[cfg(not(unix))]
impl EventStream {
    pub fn connect() -> Option<Self> { None }
    pub fn next_line(&mut self) -> Result<Option<String>, ()> { Err(()) }
}

/// Hyprland only runs on Unix; elsewhere there is no socket to ask.
#[cfg(not(unix))]
fn via_socket(_request: &str) -> Option<String> {
    None
}

fn via_hyprctl(what: &str) -> Option<String> {
    let mut cmd = std::process::Command::new("hyprctl");
    cmd.arg(what).arg("-j");
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        if let Some(sig) = signature() {
            cmd.env("HYPRLAND_INSTANCE_SIGNATURE", sig);
        }
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn query(what: &str) -> Option<String> {
    if !present() {
        return None;
    }
    via_socket(&format!("j/{what}")).or_else(|| via_hyprctl(what))
}

pub fn parse_monitors(json: &str) -> Option<Vec<HyprMonitor>> {
    serde_json::from_str(json).ok()
}

pub fn parse_clients(json: &str) -> Option<Vec<HyprClient>> {
    serde_json::from_str(json).ok()
}

/// Every monitor, or None when not running under Hyprland.
pub fn monitors() -> Option<Vec<HyprMonitor>> {
    parse_monitors(&query("monitors")?)
}

/// Every window, or None when not running under Hyprland.
pub fn clients() -> Option<Vec<HyprClient>> {
    parse_clients(&query("clients")?)
}

pub fn parse_client(json: &str) -> Option<HyprClient> {
    serde_json::from_str(json).ok()
}

/// The focused window, or None when not under Hyprland or nothing is focused.
pub fn active_window() -> Option<HyprClient> {
    parse_client(&query("activewindow")?)
}

/// Pointer in compositor logical coordinates. Queried only by the background
/// interaction watcher, never by the render loop.
pub fn cursorpos() -> Option<[f64; 2]> {
    #[derive(Deserialize)]
    struct Pos { x: f64, y: f64 }
    let p: Pos = serde_json::from_str(&query("cursorpos")?).ok()?;
    (p.x.is_finite() && p.y.is_finite()).then_some([p.x, p.y])
}

pub(crate) fn cursorpos_ipc() -> Option<[f64; 2]> {
    #[derive(Deserialize)]
    struct Pos { x: f64, y: f64 }
    let p: Pos = serde_json::from_str(&socket_json("cursorpos")?).ok()?;
    (p.x.is_finite() && p.y.is_finite()).then_some([p.x, p.y])
}

/// Run a dispatcher: the Lua spelling first (Hyprland 0.55+ with a Lua
/// config takes `hl.dsp.…(...)`), then the classic one. True when Hyprland
/// answered "ok". The socket first, then the `hyprctl` binary.
pub fn dispatch(lua: &str, legacy: &str) -> bool {
    if !present() {
        return false;
    }
    let ok = |reply: &str| reply.trim_start().starts_with("ok");
    let mut socket_seen = false;
    for cmd in [lua, legacy] {
        if let Some(reply) = via_socket(&format!("dispatch {cmd}")) {
            socket_seen = true;
            if ok(&reply) {
                return true;
            }
        }
    }
    if socket_seen {
        return false;
    }
    for cmd in [lua, legacy] {
        let mut c = std::process::Command::new("hyprctl");
        c.arg("dispatch");
        if cmd.starts_with("hl.") {
            c.arg(cmd);
        } else {
            c.args(cmd.split_whitespace());
        }
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
            if let Some(sig) = signature() {
                c.env("HYPRLAND_INSTANCE_SIGNATURE", sig);
            }
        }
        if c.output().ok().is_some_and(|o| o.status.success() && ok(&String::from_utf8_lossy(&o.stdout))) {
            return true;
        }
    }
    false
}

/// Give a window (by `0x…` address) the focus.
pub fn focus_window(address: &str) -> bool {
    dispatch(
        &format!("hl.dsp.focus({{ window = \"address:{address}\" }})"),
        &format!("focuswindow address:{address}"),
    )
}

/// Put a window (by address) into fullscreen, or take it out. The classic
/// spelling works on the focused window, so focus it first when it is not.
pub fn set_fullscreen(address: &str, on: bool) -> bool {
    let (internal, client) = if on { (2, 2) } else { (0, 0) };
    dispatch(
        &format!("hl.dsp.window.fullscreen_state({{ internal = {internal}, client = {client}, window = \"address:{address}\" }})"),
        &format!("fullscreenstate {internal} {client}"),
    )
}

/// Parent pid of `pid` from /proc.
pub fn ppid_of(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(')')?.1;
    after.split_whitespace().nth(1)?.parse().ok()
}

/// `pid` and up to four ancestors: the compositor tracks the terminal
/// emulator, which is our parent or grandparent.
pub fn ancestors(pid: u32) -> Vec<u32> {
    let mut v = vec![pid];
    let mut p = pid;
    for _ in 0..4 {
        match ppid_of(p) {
            Some(q) if q > 1 => {
                v.push(q);
                p = q;
            }
            _ => break,
        }
    }
    v
}

/// The window owned by any pid in `chain` (nearest first).
pub fn window_for<'a>(clients: &'a [HyprClient], chain: &[u32]) -> Option<&'a HyprClient> {
    chain
        .iter()
        .find_map(|p| clients.iter().find(|c| c.pid == *p as i64 && c.mapped && !c.hidden))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITORS: &str = r#"[
      {"id":0,"name":"DP-1","description":"Samsung LS27AG30x","width":1920,"height":1080,
       "refreshRate":143.981,"x":3840,"y":0,"scale":1,"transform":1,"physicalWidth":600,
       "physicalHeight":340,"reserved":[0,30,0,0],"activeWorkspace":{"id":3,"name":"3"},
       "availableModes":["1920x1080@143.98Hz"]},
      {"id":2,"name":"HDMI-A-1","width":1920,"height":1080,"x":1920,"y":0,"scale":1.0,
       "transform":0,"physicalWidth":480,"physicalHeight":270,"reserved":[0,30,0,0]}
    ]"#;

    #[test]
    fn parses_real_monitor_json_and_ignores_unknown_fields() {
        let m = parse_monitors(MONITORS).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].name, "DP-1");
        assert_eq!(m[0].transform, 1);
        assert_eq!(m[0].reserved, [0, 30, 0, 0]);
        assert_eq!((m[1].physical_width, m[1].physical_height), (480, 270));
        assert_eq!(m[1].description, "");
    }

    #[test]
    fn reads_the_active_window_and_its_fullscreen_state() {
        let w = parse_client(
            r#"{"address":"0x564902694cc0","pid":2658391,"at":[0,0],"size":[1920,1080],"class":"kitty","title":"termpaper","fullscreen":2,"fullscreenClient":0}"#,
        )
        .unwrap();
        assert_eq!((w.address.as_str(), w.pid, w.fullscreen), ("0x564902694cc0", 2658391, 2));
        // older Hyprland says true/false
        let old = parse_client(r#"{"pid":1,"at":[0,0],"size":[1,1],"fullscreen":true}"#).unwrap();
        assert_eq!(old.fullscreen, 2);
        let none = parse_client(r#"{"pid":1,"at":[0,0],"size":[1,1]}"#).unwrap();
        assert_eq!((none.fullscreen, none.address.as_str()), (0, ""));
        assert!(parse_client("Invalid").is_none(), "no active window");
    }

    #[test]
    fn finds_the_window_through_the_ancestor_chain() {
        let c = parse_clients(
            r#"[{"pid":10,"at":[0,0],"size":[100,50],"class":"a"},
                {"pid":42,"at":[1920,0],"size":[1920,1080],"class":"kitty","workspace":{"id":1}}]"#,
        )
        .unwrap();
        assert_eq!(window_for(&c, &[99, 42]).unwrap().at, [1920, 0]);
        assert!(window_for(&c, &[7]).is_none());
    }
}

/// Poll coverage off the frame loop: compositor IPC can take hundreds of ms.
pub struct FullscreenWatcher(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl FullscreenWatcher {
    pub fn spawn() -> Self {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        let state = Arc::new(AtomicBool::new(false));
        let slot = Arc::clone(&state);
        let chain = ancestors(std::process::id());
        let _ = std::thread::Builder::new().name("termpaper-cover".into()).spawn(move || loop {
            let covered = clients().and_then(|list| {
                let own = window_for(&list, &chain)?;
                let active = active_window()?;
                Some(covers_window(own, &active))
            }).unwrap_or(false);
            slot.store(covered, Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_secs(2));
        });
        Self(state)
    }

    pub fn covered(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn covers_window(own: &HyprClient, active: &HyprClient) -> bool {
    active.fullscreen == 2 && active.mapped && !active.hidden
        && active.pid != own.pid && active.monitor == own.monitor
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    #[test]
    fn only_another_fullscreen_window_on_our_monitor_covers_us() {
        let own: HyprClient = serde_json::from_str(r#"{"pid":1,"at":[0,0],"size":[100,100],"monitor":0}"#).unwrap();
        let mut other = own.clone();
        other.pid = 2;
        other.fullscreen = 2;
        assert!(covers_window(&own, &other));
        other.monitor = 1;
        assert!(!covers_window(&own, &other));
        other.monitor = 0;
        other.fullscreen = 1;
        assert!(!covers_window(&own, &other));
        other.fullscreen = 2;
        other.pid = own.pid;
        assert!(!covers_window(&own, &other));
    }
}
