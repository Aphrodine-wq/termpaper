//! Window fullscreen: ask the terminal window, or the compositor around it,
//! to fill the screen while termpaper runs, and put it back on quit.
//!
//! A terminal program cannot resize its own window portably, so this is a
//! chain of ways that each work somewhere: Hyprland's IPC socket, kitty's
//! remote control, sway's, `wmctrl` on X11, the xterm window-ops escape
//! (`CSI 10;2 t`), a synthesised keystroke on macOS and Windows. The first
//! that fits the environment is tried; when it fails the escape is sent
//! anyway, which terminals that do not know it ignore. Inside tmux or screen
//! nothing is attempted: the multiplexer owns the screen.
//!
//! Whatever put the window into fullscreen is remembered, so `leave` undoes
//! exactly that and a window that was already fullscreen is left alone.

use crate::term_caps::Terminal;
use std::io::Write;

/// The way to reach the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Hyprland,
    Kitty,
    Sway,
    Wmctrl,
    MacOs,
    Windows,
    /// xterm window ops: `ESC [ 10 ; 2 t` toggles fullscreen
    Xtwinops,
    /// a multiplexer, or no terminal at all
    None,
}

impl Method {
    pub fn name(self) -> &'static str {
        match self {
            Method::Hyprland => "Hyprland",
            Method::Kitty => "kitty",
            Method::Sway => "sway",
            Method::Wmctrl => "wmctrl",
            Method::MacOs => "macOS",
            Method::Windows => "Windows",
            Method::Xtwinops => "the terminal",
            Method::None => "not available here",
        }
    }
}

/// The operating system, for the pure chooser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
}

pub fn this_os() -> Os {
    if cfg!(target_os = "macos") {
        Os::MacOs
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Linux
    }
}

/// Pick the method from what the environment says. Pure, so the table is
/// tested: `env` reads a variable, `has` says whether a command is on PATH.
pub fn choose(terminal: Terminal, os: Os, env: impl Fn(&str) -> Option<String>, has: impl Fn(&str) -> bool) -> Method {
    let set = |k: &str| env(k).is_some_and(|v| !v.is_empty());
    if matches!(terminal, Terminal::Tmux | Terminal::Screen) {
        return Method::None;
    }
    if set("HYPRLAND_INSTANCE_SIGNATURE") {
        return Method::Hyprland;
    }
    if terminal == Terminal::Kitty && set("KITTY_LISTEN_ON") && (has("kitten") || has("kitty")) {
        return Method::Kitty;
    }
    if set("SWAYSOCK") && has("swaymsg") {
        return Method::Sway;
    }
    if set("DISPLAY") && !set("WAYLAND_DISPLAY") && has("wmctrl") {
        return Method::Wmctrl;
    }
    match os {
        Os::MacOs if terminal != Terminal::Kitty => Method::MacOs,
        Os::Windows => Method::Windows,
        _ => Method::Xtwinops,
    }
}

/// Whether `cmd` is on PATH.
pub fn on_path(cmd: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|d| {
        let p = d.join(cmd);
        p.is_file() || (cfg!(windows) && d.join(format!("{cmd}.exe")).is_file())
    })
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd).args(args).stdin(std::process::Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The escape that asks an xterm-compatible terminal to toggle fullscreen.
pub const XTWINOPS_TOGGLE: &[u8] = b"\x1b[10;2t";

pub struct Fullscreen {
    method: Method,
    /// termpaper put the window into fullscreen, so it takes it out again
    active: bool,
    /// Hyprland: the window's address, to find it again on quit
    address: Option<String>,
    /// what the last attempt found, for the help overlay
    note: &'static str,
}

impl Fullscreen {
    pub fn new(method: Method) -> Self {
        Fullscreen { method, active: false, address: None, note: "" }
    }

    /// From the real environment.
    pub fn detect(terminal: Terminal) -> Self {
        let method = choose(terminal, this_os(), |k| std::env::var(k).ok(), on_path);
        Self::new(method)
    }

    pub fn method(&self) -> Method {
        self.method
    }

    /// Whether termpaper holds the window in fullscreen right now.
    pub fn active(&self) -> bool {
        self.active
    }

    /// One line for the help overlay: `on · Hyprland`, `off · kitty`,
    /// `not available here`, `off · window not focused`.
    pub fn status(&self) -> String {
        if self.method == Method::None {
            return Method::None.name().to_string();
        }
        let state = if self.active { "on" } else { "off" };
        if self.note.is_empty() {
            format!("{state} · {}", self.method.name())
        } else {
            format!("{state} · {}", self.note)
        }
    }

    /// Go fullscreen. True when the window is fullscreen afterwards (ours
    /// or already so). Writes only the escape, and only to `out`.
    pub fn enter(&mut self, out: &mut dyn Write) -> bool {
        if self.active {
            return true;
        }
        self.note = "";
        let ok = match self.method {
            Method::None => false,
            Method::Hyprland => self.enter_hyprland(),
            Method::Kitty => Self::kitty_toggle(),
            Method::Sway => self.sway(true),
            Method::Wmctrl => run("wmctrl", &["-r", ":ACTIVE:", "-b", "add,fullscreen"]).is_some(),
            Method::MacOs => Self::macos_toggle(),
            Method::Windows => Self::windows_toggle(),
            Method::Xtwinops => Self::xtwinops(out),
        };
        if ok {
            self.active = self.note.is_empty();
            return true;
        }
        // the escape as a last resort: ignored by terminals that lack it
        if self.method != Method::None && self.method != Method::Xtwinops && self.note.is_empty() {
            if Self::xtwinops(out) {
                self.note = "the terminal";
                self.active = true;
                return true;
            }
        }
        false
    }

    /// Undo what `enter` did, if anything.
    pub fn leave(&mut self, out: &mut dyn Write) {
        if !self.active {
            return;
        }
        match self.method {
            Method::None => {}
            Method::Hyprland if self.note.is_empty() => self.leave_hyprland(),
            Method::Kitty if self.note.is_empty() => {
                Self::kitty_toggle();
            }
            Method::Sway if self.note.is_empty() => {
                self.sway(false);
            }
            Method::Wmctrl if self.note.is_empty() => {
                run("wmctrl", &["-r", ":ACTIVE:", "-b", "remove,fullscreen"]);
            }
            Method::MacOs if self.note.is_empty() => {
                Self::macos_toggle();
            }
            Method::Windows if self.note.is_empty() => {
                Self::windows_toggle();
            }
            // the escape put it there (directly, or as the fallback)
            _ => {
                Self::xtwinops(out);
            }
        }
        self.active = false;
        self.address = None;
    }

    /// `F`: in when out, out when in. Returns whether it is on afterwards.
    pub fn toggle(&mut self, out: &mut dyn Write) -> bool {
        if self.active {
            self.leave(out);
            false
        } else {
            self.enter(out)
        }
    }

    fn enter_hyprland(&mut self) -> bool {
        // right after launch our window may not be mapped yet: wait a little
        let chain = crate::hypr::ancestors(std::process::id());
        let mut mine = None;
        for _ in 0..10 {
            let clients = crate::hypr::clients().unwrap_or_default();
            if let Some(w) = crate::hypr::window_for(&clients, &chain) {
                mine = Some(w.clone());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let Some(win) = mine else {
            self.note = "Hyprland: window not found";
            return false;
        };
        if win.fullscreen != 0 {
            // already fullscreen (the user's doing): fine, and not ours to undo
            self.note = "Hyprland, already";
            return true;
        }
        // the user just started termpaper here: this is the window they mean
        let active = crate::hypr::active_window().is_some_and(|a| a.address == win.address);
        if !active {
            crate::hypr::focus_window(&win.address);
        }
        if crate::hypr::set_fullscreen(&win.address, true) {
            self.address = Some(win.address);
            return true;
        }
        self.note = "Hyprland refused";
        false
    }

    fn leave_hyprland(&mut self) {
        let Some(addr) = self.address.take() else { return };
        let still = crate::hypr::clients()
            .unwrap_or_default()
            .into_iter()
            .find(|c| c.address == addr)
            .is_some_and(|c| c.fullscreen != 0);
        if still {
            let active = crate::hypr::active_window().is_some_and(|w| w.address == addr);
            if !active {
                crate::hypr::focus_window(&addr);
            }
            crate::hypr::set_fullscreen(&addr, false);
        }
    }

    fn kitty_toggle() -> bool {
        let args = ["@", "resize-os-window", "--action", "toggle-fullscreen", "--match", "state:focused"];
        run("kitten", &args).is_some() || run("kitty", &args).is_some()
    }

    fn sway(&mut self, on: bool) -> bool {
        let state = if on { "enable" } else { "disable" };
        for pid in crate::hypr::ancestors(std::process::id()) {
            let crit = format!("[pid={pid}] fullscreen {state}");
            if run("swaymsg", &[&crit]).is_some_and(|o| o.contains("\"success\": true")) {
                return true;
            }
        }
        self.note = "sway found no window";
        false
    }

    fn macos_toggle() -> bool {
        run(
            "osascript",
            &["-e", "tell application \"System Events\" to keystroke \"f\" using {command down, control down}"],
        )
        .is_some()
    }

    #[cfg(windows)]
    fn windows_toggle() -> bool {
        // Alt+Enter toggles fullscreen in Windows Terminal and the console
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_MENU, VK_RETURN,
        };
        let key = |vk: u16, up: bool| {
            let mut i: INPUT = unsafe { std::mem::zeroed() };
            i.r#type = INPUT_KEYBOARD;
            i.Anonymous.ki = KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 };
            i
        };
        let inputs = [key(VK_MENU, false), key(VK_RETURN, false), key(VK_RETURN, true), key(VK_MENU, true)];
        let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
        sent == inputs.len() as u32
    }

    #[cfg(not(windows))]
    fn windows_toggle() -> bool {
        false
    }

    fn xtwinops(out: &mut dyn Write) -> bool {
        out.write_all(XTWINOPS_TOGGLE).is_ok() && out.flush().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn pick(terminal: Terminal, os: Os, vars: &[(&str, &str)], cmds: &[&str]) -> Method {
        let env: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        choose(terminal, os, |k| env.get(k).cloned(), |c| cmds.contains(&c))
    }

    #[test]
    fn the_compositor_wins_then_the_terminal_then_the_escape() {
        assert_eq!(pick(Terminal::Kitty, Os::Linux, &[("HYPRLAND_INSTANCE_SIGNATURE", "abc"), ("KITTY_LISTEN_ON", "unix:/x")], &["kitten"]), Method::Hyprland);
        assert_eq!(pick(Terminal::Kitty, Os::Linux, &[("KITTY_LISTEN_ON", "unix:/x")], &["kitten"]), Method::Kitty);
        assert_eq!(pick(Terminal::Kitty, Os::Linux, &[("KITTY_LISTEN_ON", "unix:/x")], &[]), Method::Xtwinops, "no kitten on PATH");
        assert_eq!(pick(Terminal::Kitty, Os::Linux, &[], &["kitten"]), Method::Xtwinops, "no remote control socket");
        assert_eq!(pick(Terminal::Foot, Os::Linux, &[("SWAYSOCK", "/run/sway")], &["swaymsg"]), Method::Sway);
        assert_eq!(pick(Terminal::Alacritty, Os::Linux, &[("DISPLAY", ":0")], &["wmctrl"]), Method::Wmctrl);
        assert_eq!(pick(Terminal::Alacritty, Os::Linux, &[("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "wayland-1")], &["wmctrl"]), Method::Xtwinops, "Xwayland: wmctrl cannot see the window");
        assert_eq!(pick(Terminal::Unknown, Os::Linux, &[], &[]), Method::Xtwinops);
        assert_eq!(pick(Terminal::ITerm2, Os::MacOs, &[], &[]), Method::MacOs);
        assert_eq!(pick(Terminal::Kitty, Os::MacOs, &[], &[]), Method::Xtwinops, "kitty on macOS without remote control");
        assert_eq!(pick(Terminal::WindowsTerminal, Os::Windows, &[], &[]), Method::Windows);
    }

    #[test]
    fn multiplexers_get_nothing() {
        assert_eq!(pick(Terminal::Tmux, Os::Linux, &[("HYPRLAND_INSTANCE_SIGNATURE", "abc")], &[]), Method::None);
        assert_eq!(pick(Terminal::Screen, Os::MacOs, &[], &[]), Method::None);
        let mut f = Fullscreen::new(Method::None);
        let mut out = Vec::new();
        assert!(!f.enter(&mut out));
        assert!(out.is_empty(), "nothing is written when there is no way");
        assert_eq!(f.status(), "not available here");
    }

    #[test]
    fn the_escape_toggles_and_is_undone_once() {
        let mut f = Fullscreen::new(Method::Xtwinops);
        let mut out = Vec::new();
        assert!(f.enter(&mut out));
        assert_eq!(out, XTWINOPS_TOGGLE);
        assert!(f.active());
        assert_eq!(f.status(), "on · the terminal");
        assert!(f.enter(&mut out), "entering twice sends nothing more");
        assert_eq!(out.len(), XTWINOPS_TOGGLE.len());
        f.leave(&mut out);
        assert_eq!(out.len(), 2 * XTWINOPS_TOGGLE.len());
        assert!(!f.active());
        f.leave(&mut out);
        assert_eq!(out.len(), 2 * XTWINOPS_TOGGLE.len(), "leaving twice sends nothing more");
        assert!(f.toggle(&mut out));
        assert!(!f.toggle(&mut out));
        assert_eq!(out.len(), 4 * XTWINOPS_TOGGLE.len());
    }
}
