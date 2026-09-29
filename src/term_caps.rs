//! What the terminal we draw into can do, read from its environment: which
//! emulator it is, whether it takes 24-bit colour, and a frame-rate default
//! that suits its throughput. Explicit settings (flags, config) always win.

/// The emulator, as far as the environment tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terminal {
    Kitty,
    Ghostty,
    WezTerm,
    Alacritty,
    Foot,
    Rio,
    ITerm2,
    AppleTerminal,
    WindowsTerminal,
    /// the classic Windows console host (no Windows Terminal around it)
    Conhost,
    VsCode,
    Konsole,
    /// GNOME Terminal, Tilix, Ptyxis and other VTE-based terminals
    Vte,
    Tmux,
    Screen,
    Unknown,
}

impl Terminal {
    pub fn name(self) -> &'static str {
        match self {
            Terminal::Kitty => "kitty",
            Terminal::Ghostty => "Ghostty",
            Terminal::WezTerm => "WezTerm",
            Terminal::Alacritty => "Alacritty",
            Terminal::Foot => "foot",
            Terminal::Rio => "Rio",
            Terminal::ITerm2 => "iTerm2",
            Terminal::AppleTerminal => "Terminal.app",
            Terminal::WindowsTerminal => "Windows Terminal",
            Terminal::Conhost => "Windows console",
            Terminal::VsCode => "VS Code",
            Terminal::Konsole => "Konsole",
            Terminal::Vte => "VTE terminal",
            Terminal::Tmux => "tmux",
            Terminal::Screen => "screen",
            Terminal::Unknown => "terminal",
        }
    }

    /// GPU-rendered emulators that swallow full-screen colour at high rates.
    fn fast(self) -> bool {
        matches!(
            self,
            Terminal::Kitty
                | Terminal::Ghostty
                | Terminal::WezTerm
                | Terminal::Alacritty
                | Terminal::Foot
                | Terminal::Rio
        )
    }
}

/// What termpaper assumes about the terminal until told otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    pub terminal: Terminal,
    pub truecolor: bool,
    /// frame-rate cap when neither the flags nor the config set one
    pub default_fps: u32,
}

/// Detect from the real environment.
pub fn detect() -> Caps {
    detect_with(
        |k| std::env::var(k).ok().filter(|v| !v.is_empty()),
        cfg!(windows),
        macos_major(),
    )
}

/// Detection with the environment injected (tests, and callers that
/// already hold a snapshot). `macos` is the macOS major version, if any.
pub fn detect_with(get: impl Fn(&str) -> Option<String>, windows: bool, macos: Option<u32>) -> Caps {
    let term = get("TERM").unwrap_or_default();
    let program = get("TERM_PROGRAM").unwrap_or_default();
    let has = |k: &str| get(k).is_some();

    // a multiplexer decides what reaches the screen, whatever runs outside it
    let terminal = if has("TMUX") || program == "tmux" || term.starts_with("tmux") {
        Terminal::Tmux
    } else if has("STY") || term.starts_with("screen") {
        Terminal::Screen
    } else {
        match program.as_str() {
            "iTerm.app" => Terminal::ITerm2,
            "Apple_Terminal" => Terminal::AppleTerminal,
            "WezTerm" => Terminal::WezTerm,
            "ghostty" => Terminal::Ghostty,
            "vscode" => Terminal::VsCode,
            "rio" => Terminal::Rio,
            _ if has("KITTY_WINDOW_ID") || term == "xterm-kitty" => Terminal::Kitty,
            _ if has("GHOSTTY_RESOURCES_DIR") || term == "xterm-ghostty" => Terminal::Ghostty,
            _ if has("WEZTERM_EXECUTABLE") || has("WEZTERM_PANE") => Terminal::WezTerm,
            _ if has("ALACRITTY_WINDOW_ID") || has("ALACRITTY_LOG") || term == "alacritty" => {
                Terminal::Alacritty
            }
            _ if term.starts_with("foot") => Terminal::Foot,
            _ if has("KONSOLE_VERSION") => Terminal::Konsole,
            _ if has("VTE_VERSION") => Terminal::Vte,
            _ if has("WT_SESSION") => Terminal::WindowsTerminal,
            _ if windows => Terminal::Conhost,
            _ => Terminal::Unknown,
        }
    };

    let colorterm = get("COLORTERM").unwrap_or_default().to_ascii_lowercase();
    let advertised = colorterm.contains("truecolor")
        || colorterm.contains("24bit")
        || term.ends_with("-direct");
    let truecolor = advertised
        || match terminal {
            // Terminal.app learned 24-bit colour in macOS 26 but does not
            // say so in the environment; before that it mangles it
            Terminal::AppleTerminal => macos.is_some_and(|v| v >= 26),
            // what a multiplexer passes through depends on its own config,
            // which only COLORTERM reflects
            Terminal::Tmux | Terminal::Screen | Terminal::Unknown => false,
            // everything else named here has done 24-bit colour for years,
            // the Windows 10+ console host included
            _ => true,
        };

    let default_fps = if terminal.fast() {
        120
    } else {
        match terminal {
            Terminal::AppleTerminal | Terminal::Conhost | Terminal::Tmux | Terminal::Screen => 30,
            _ => 60,
        }
    };

    Caps {
        terminal,
        truecolor,
        default_fps,
    }
}

/// The macOS major version (26 for Tahoe), read from the system version
/// file rather than spawning `sw_vers`.
#[cfg(target_os = "macos")]
fn macos_major() -> Option<u32> {
    let text = std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist").ok()?;
    parse_product_version(&text)
}

#[cfg(not(target_os = "macos"))]
fn macos_major() -> Option<u32> {
    None
}

/// `ProductVersion` from a SystemVersion.plist, major part only.
pub fn parse_product_version(plist: &str) -> Option<u32> {
    let after = plist.split("<key>ProductVersion</key>").nth(1)?;
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")? + start;
    after[start..end].trim().split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn caps(vars: &[(&str, &str)], windows: bool, macos: Option<u32>) -> Caps {
        let env: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        detect_with(|k| env.get(k).cloned(), windows, macos)
    }

    #[test]
    fn identifies_the_common_emulators() {
        assert_eq!(caps(&[("TERM", "xterm-kitty"), ("KITTY_WINDOW_ID", "1")], false, None).terminal, Terminal::Kitty);
        assert_eq!(caps(&[("TERM_PROGRAM", "ghostty")], false, Some(15)).terminal, Terminal::Ghostty);
        assert_eq!(caps(&[("TERM_PROGRAM", "iTerm.app")], false, Some(15)).terminal, Terminal::ITerm2);
        assert_eq!(caps(&[("TERM_PROGRAM", "WezTerm")], false, None).terminal, Terminal::WezTerm);
        assert_eq!(caps(&[("TERM", "foot")], false, None).terminal, Terminal::Foot);
        assert_eq!(caps(&[("WT_SESSION", "x")], true, None).terminal, Terminal::WindowsTerminal);
        assert_eq!(caps(&[], true, None).terminal, Terminal::Conhost);
        assert_eq!(caps(&[("TERM", "xterm-256color")], false, None).terminal, Terminal::Unknown);
    }

    #[test]
    fn a_multiplexer_wins_over_the_outer_terminal() {
        let c = caps(&[("KITTY_WINDOW_ID", "1"), ("TMUX", "/tmp/tmux-1000/default,1,0")], false, None);
        assert_eq!(c.terminal, Terminal::Tmux);
        assert!(!c.truecolor, "tmux passes 24-bit only when COLORTERM says so");
        assert_eq!(c.default_fps, 30);
        let c = caps(&[("TMUX", "x"), ("COLORTERM", "truecolor")], false, None);
        assert!(c.truecolor);
    }

    #[test]
    fn truecolor_rules() {
        // advertised anywhere
        assert!(caps(&[("COLORTERM", "truecolor")], false, None).truecolor);
        assert!(caps(&[("COLORTERM", "24bit")], false, None).truecolor);
        assert!(caps(&[("TERM", "xterm-direct")], false, None).truecolor);
        // unknown terminal without the hint: stay safe with 256 colours
        assert!(!caps(&[("TERM", "xterm-256color")], false, None).truecolor);
        // Terminal.app only from macOS 26
        assert!(!caps(&[("TERM_PROGRAM", "Apple_Terminal")], false, Some(15)).truecolor);
        assert!(caps(&[("TERM_PROGRAM", "Apple_Terminal")], false, Some(26)).truecolor);
        // Windows: both hosts take 24-bit colour
        assert!(caps(&[("WT_SESSION", "x")], true, None).truecolor);
        assert!(caps(&[], true, None).truecolor);
    }

    #[test]
    fn frame_rate_defaults_follow_throughput() {
        assert_eq!(caps(&[("TERM", "xterm-kitty")], false, None).default_fps, 120);
        assert_eq!(caps(&[("TERM_PROGRAM", "iTerm.app")], false, None).default_fps, 60);
        assert_eq!(caps(&[("WT_SESSION", "x")], true, None).default_fps, 60);
        assert_eq!(caps(&[("TERM_PROGRAM", "Apple_Terminal")], false, Some(26)).default_fps, 30);
        assert_eq!(caps(&[], true, None).default_fps, 30);
        assert_eq!(caps(&[("TERM", "xterm-256color")], false, None).default_fps, 60);
    }

    #[test]
    fn reads_the_macos_product_version() {
        let plist = "<dict>\n\t<key>ProductName</key>\n\t<string>macOS</string>\n\t<key>ProductVersion</key>\n\t<string>26.0.1</string>\n</dict>";
        assert_eq!(parse_product_version(plist), Some(26));
        assert_eq!(parse_product_version("<dict></dict>"), None);
    }
}
