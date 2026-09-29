//! Hyprland queries over the compositor's IPC socket.
//!
//! One request/response per call on `$XDG_RUNTIME_DIR/hypr/$SIG/.socket.sock`
//! (the same thing `hyprctl` does, minus spawning a process), falling back to
//! the `hyprctl` binary. Only the fields termpaper needs are decoded; unknown
//! fields are ignored so newer compositors keep working.
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

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
}

fn yes() -> bool {
    true
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

fn via_socket(request: &str) -> Option<String> {
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
