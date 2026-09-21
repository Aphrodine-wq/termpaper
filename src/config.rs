//! Config file: ~/.config/termpaper/config.toml
//! Read on startup; the settings menu writes changes back atomically
//! (temp file + rename). Unknown keys warn, never fail.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
    /// fps cap while the terminal is unfocused (unset = no throttle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_fps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixels: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_scale: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dim: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock: Option<bool>,
    /// run post-processing on the GPU (needs a build with `--features gpu`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<crate::engine::Renderer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad: Option<i32>,
    /// global hue rotation in degrees (0 = off)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hue_shift: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f32>,
    /// instance linking (default true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<bool>,
    /// link group name — instances in the same group sync; different groups stay independent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// per-scene remembered theme
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub themes: HashMap<String, String>,
    /// action → key remaps
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub keys: HashMap<String, String>,
}

/// Built-in runtime defaults (empty config file, no CLI overrides).
pub const DEFAULT_FPS: u32 = 120;
pub const DEFAULT_SPEED: f32 = 1.0;
pub const DEFAULT_SMOOTH: f32 = 0.3;
pub const DEFAULT_DIM: f32 = 1.0;
pub const DEFAULT_FADE: f32 = 0.25;
pub const DEFAULT_CLOCK: bool = true;
pub const DEFAULT_LINK: bool = true;
pub const DEFAULT_GROUP: &str = "default";

/// Reset stored config to defaults, keeping custom keybinds and the active scene.
pub fn reset_stored_defaults(cfg: &mut Config, scene: &str) {
    let keys = std::mem::take(&mut cfg.keys);
    *cfg = Config::default();
    cfg.keys = keys;
    cfg.scene = Some(scene.to_string());
}

pub fn config_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("termpaper/config.toml"));
        }
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".config/termpaper/config.toml"))
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Config::default();
    };
    match toml::from_str(&text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("termpaper: ignoring invalid config {}: {e}", path.display());
            Config::default()
        }
    }
}

/// Persist the config atomically: write a temp file next to the target,
/// then rename over it. Creates ~/.config/termpaper if needed (explicit
/// user action via the menu — not done silently on startup).
#[allow(dead_code)] // used by the settings menu
pub fn save(cfg: &Config) -> std::io::Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, toml::to_string_pretty(cfg).map_err(std::io::Error::other)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Rebindable actions.
pub const ACTIONS: &[&str] = &[
    "quit",
    "menu",
    "next",
    "prev",
    "filter_next",
    "detail_next",
    "pause",
    "color",
    "reset",
    "fps_up",
    "fps_down",
    "speed_up",
    "speed_down",
];

pub fn default_key(action: &str) -> &'static str {
    match action {
        "quit" => "q",
        "menu" => "?",
        "next" => "right",
        "prev" => "left",
        "filter_next" => "f",
        "detail_next" => "d",
        "pause" => "space",
        "color" => "c",
        "reset" => "0",
        "fps_up" => "]",
        "fps_down" => "[",
        "speed_up" => ".",
        "speed_down" => ",",
        _ => "",
    }
}

/// A parsed key: single char or a named key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(char),
    Esc,
    Tab,
    Enter,
    Space,
    Left,
    Right,
    Up,
    Down,
}

pub fn parse_key(s: &str) -> Option<Key> {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "esc" | "escape" => Some(Key::Esc),
        "tab" => Some(Key::Tab),
        "enter" | "return" => Some(Key::Enter),
        "space" => Some(Key::Space),
        "left" => Some(Key::Left),
        "right" => Some(Key::Right),
        "up" => Some(Key::Up),
        "down" => Some(Key::Down),
        _ if s.chars().count() == 1 => Some(Key::Char(s.chars().next().unwrap())),
        _ => None,
    }
}

/// Action → key map with defaults, overlaid by `[keys]` from config.
pub struct KeyMap {
    pub map: HashMap<&'static str, Key>,
}

impl KeyMap {
    pub fn new(cfg: &Config) -> Self {
        let mut map = HashMap::new();
        for &action in ACTIONS {
            let key = match cfg.keys.get(action) {
                Some(s) => match parse_key(s) {
                    Some(k) => k,
                    None => {
                        eprintln!("termpaper: unknown key '{s}' for action '{action}', using default");
                        parse_key(default_key(action)).unwrap()
                    }
                },
                None => parse_key(default_key(action)).unwrap(),
            };
            map.insert(action, key);
        }
        // warn on unknown actions
        for k in cfg.keys.keys() {
            if !ACTIONS.contains(&k.as_str()) {
                eprintln!("termpaper: unknown keybind action '{k}' (ignored)");
            }
        }
        KeyMap { map }
    }

    pub fn get(&self, action: &str) -> Option<Key> {
        self.map.get(action).copied()
    }

    /// Does this crossterm key event match the bound key for `action`?
    pub fn matches(&self, action: &str, code: crossterm::event::KeyCode) -> bool {
        let Some(k) = self.get(action) else {
            return false;
        };
        use crossterm::event::KeyCode as C;
        match k {
            Key::Char(ch) => code == C::Char(ch),
            Key::Esc => code == C::Esc,
            Key::Tab => code == C::Tab,
            Key::Enter => code == C::Enter,
            Key::Space => code == C::Char(' '),
            Key::Left => code == C::Left,
            Key::Right => code == C::Right,
            Key::Up => code == C::Up,
            Key::Down => code == C::Down,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let mut cfg = Config {
            scene: Some("fire".into()),
            fps: Some(60),
            detail: Some("high".into()),
            pixels: Some("quad".into()),
            filters: vec!["scanlines".into(), "vignette".into()],
            text_scale: Some(3),
            ..Default::default()
        };
        cfg.themes.insert("airspace".into(), "golden".into());
        cfg.keys.insert("quit".into(), "x".into());
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.scene.as_deref(), Some("fire"));
        assert_eq!(back.fps, Some(60));
        assert_eq!(back.detail.as_deref(), Some("high"));
        assert_eq!(back.pixels.as_deref(), Some("quad"));
        assert_eq!(back.filters, vec!["scanlines", "vignette"]);
        assert_eq!(back.text_scale, Some(3));
        assert_eq!(back.themes.get("airspace").map(|s| s.as_str()), Some("golden"));
        assert_eq!(back.keys.get("quit").map(|s| s.as_str()), Some("x"));
    }

    #[test]
    fn save_and_reload_via_disk() {
        let dir = std::env::temp_dir().join(format!("termpaper-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let cfg = Config {
            detail: Some("high".into()),
            ..Default::default()
        };
        save(&cfg).unwrap();
        let loaded = load();
        assert_eq!(loaded.detail.as_deref(), Some("high"));
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reset_stored_defaults_keeps_keys_and_scene() {
        let mut cfg = Config {
            scene: Some("fire".into()),
            fps: Some(30),
            filters: vec!["crt".into()],
            hue_shift: Some(90.0),
            group: Some("wallpaper".into()),
            ..Default::default()
        };
        cfg.keys.insert("quit".into(), "x".into());
        cfg.themes.insert("fire".into(), "inferno".into());
        reset_stored_defaults(&mut cfg, "rain");
        assert_eq!(cfg.scene.as_deref(), Some("rain"));
        assert_eq!(cfg.fps, None);
        assert!(cfg.filters.is_empty());
        assert_eq!(cfg.hue_shift, None);
        assert_eq!(cfg.group, None);
        assert!(cfg.themes.is_empty());
        assert_eq!(cfg.keys.get("quit").map(|s| s.as_str()), Some("x"));
    }

    #[test]
    fn keybind_parsing_and_matching() {
        assert_eq!(parse_key("q"), Some(Key::Char('q')));
        assert_eq!(parse_key("?"), Some(Key::Char('?')));
        assert_eq!(parse_key("space"), Some(Key::Space));
        assert_eq!(parse_key("right"), Some(Key::Right));
        assert_eq!(parse_key("ESC"), Some(Key::Esc));
        assert_eq!(parse_key("boguskey"), None);

        let mut cfg = Config::default();
        cfg.keys.insert("quit".into(), "x".into());
        let km = KeyMap::new(&cfg);
        assert!(km.matches("quit", crossterm::event::KeyCode::Char('x')));
        assert!(!km.matches("quit", crossterm::event::KeyCode::Char('q')));
        assert!(km.matches("next", crossterm::event::KeyCode::Right));
        assert!(km.matches("pause", crossterm::event::KeyCode::Char(' ')));
    }
}
