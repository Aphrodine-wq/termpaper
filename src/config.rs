//! Config file: ~/.config/termpaper/config.toml
//! Read on startup; the settings menu writes changes back atomically
//! (temp file + rename). Unknown keys warn, never fail.

use crate::engine::Renderer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    pub renderer: Option<Renderer>,
    /// GPU milliseconds per frame a Studio scene may spend (default 3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_budget_ms: Option<f32>,
    /// fps cap while a Studio scene is showing (default 60)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader_fps: Option<u32>,
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
    /// video-wall cropping when linked (default true); false renders the
    /// local canvas and hides this window's geometry from peers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wall: Option<bool>,
    /// which scenes `cycle` rotates through: all, category or favorites
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_scope: Option<String>,
    /// scenes starred in the menu browser (`f`), in the order starred
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub favorites: Vec<String>,
    /// scenes last switched to from the menu, newest first
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recents: Vec<String>,
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
pub const DEFAULT_SCENE: &str = "rain";

/// Which scenes the auto-cycle rotates through.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CycleScope {
    #[default]
    All,
    /// the current scene's browser category
    Category,
    Favorites,
}

impl CycleScope {
    pub const ALL: [CycleScope; 3] = [CycleScope::All, CycleScope::Category, CycleScope::Favorites];

    pub fn name(self) -> &'static str {
        match self {
            CycleScope::All => "all",
            CycleScope::Category => "category",
            CycleScope::Favorites => "favorites",
        }
    }

    /// Lenient: an unknown value falls back to `All` rather than failing the
    /// whole config file.
    pub fn parse(s: &str) -> Self {
        match s {
            "category" => CycleScope::Category,
            "favorites" | "favourites" => CycleScope::Favorites,
            _ => CycleScope::All,
        }
    }
}

/// Reset stored config to defaults, keeping custom keybinds, the active
/// scene, and the user's favourites and recents (content, not settings).
pub fn reset_stored_defaults(cfg: &mut Config, scene: &str) {
    let keys = std::mem::take(&mut cfg.keys);
    let favorites = std::mem::take(&mut cfg.favorites);
    let recents = std::mem::take(&mut cfg.recents);
    *cfg = Config::default();
    cfg.keys = keys;
    cfg.favorites = favorites;
    cfg.recents = recents;
    cfg.scene = Some(scene.to_string());
}

/// Round to two decimals, so repeated ±0.15 steps store `1.45`, not
/// `1.4499998`.
pub fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// The live settings worth remembering, in their runtime types. [`store`]
/// turns this into config entries.
pub struct Live<'a> {
    pub scene: &'a str,
    pub theme: Option<&'a str>,
    pub pixels: &'a str,
    /// platform default pixel mode (macOS differs); not stored when equal
    pub default_pixels: &'a str,
    pub detail: &'a str,
    pub default_detail: &'a str,
    pub filters: &'a [String],
    pub text_scale: Option<u32>,
    pub fps: u32,
    pub speed: f32,
    pub smooth: f32,
    pub dim: f32,
    pub fade: f32,
    pub clock: bool,
    pub cycle: Option<f64>,
    pub cycle_scope: CycleScope,
    pub hue_shift: f32,
    pub saturation: f32,
    pub contrast: f32,
    pub renderer: Renderer,
    pub link: bool,
    pub group: &'a str,
    pub wall: bool,
}

/// Merge live settings into `cfg`, keeping only values that differ from the
/// built-in defaults (a default removes the entry) and rounding floats to two
/// decimals. Keybinds, favourites, recents and other scenes' themes are left
/// as they are.
pub fn store(cfg: &mut Config, live: &Live) {
    fn keep<T: PartialEq>(v: T, default: T) -> Option<T> {
        (v != default).then_some(v)
    }
    cfg.scene = keep(live.scene, DEFAULT_SCENE).map(str::to_string);
    cfg.pixels = keep(live.pixels, live.default_pixels).map(str::to_string);
    cfg.detail = keep(live.detail, live.default_detail).map(str::to_string);
    cfg.filters = live.filters.to_vec();
    cfg.text_scale = live.text_scale;
    cfg.fps = keep(live.fps, DEFAULT_FPS);
    cfg.speed = keep(round2(live.speed), DEFAULT_SPEED);
    cfg.smooth = keep(round2(live.smooth), DEFAULT_SMOOTH);
    cfg.dim = keep(round2(live.dim), DEFAULT_DIM);
    cfg.fade = keep(round2(live.fade), DEFAULT_FADE);
    cfg.clock = keep(live.clock, DEFAULT_CLOCK);
    cfg.cycle = live.cycle.map(|c| (c * 100.0).round() / 100.0);
    cfg.cycle_scope = keep(live.cycle_scope, CycleScope::All).map(|s| s.name().to_string());
    cfg.hue_shift = (live.hue_shift >= 0.5).then(|| round2(live.hue_shift));
    cfg.saturation = keep(round2(live.saturation), 1.0);
    cfg.contrast = keep(round2(live.contrast), 1.0);
    // `renderer` supersedes the legacy `gpu` switch it was derived from
    cfg.renderer = keep(live.renderer, Renderer::Auto);
    cfg.gpu = None;
    cfg.link = keep(live.link, DEFAULT_LINK);
    cfg.group = keep(live.group, DEFAULT_GROUP).map(str::to_string);
    cfg.wall = keep(live.wall, true);
    if let Some(t) = live.theme {
        // a scene's first theme is its default — but with a global `theme`
        // set, an explicit per-scene entry still matters
        let default = crate::scene::themes(live.scene).first().copied();
        if cfg.theme.is_none() && default == Some(t) {
            cfg.themes.remove(live.scene);
        } else {
            cfg.themes.insert(live.scene.to_string(), t.to_string());
        }
    }
}

/// Coalesces config writes: a change marks the config dirty and it is
/// written once the window has passed, so holding an arrow key in the menu
/// writes at most twice a second instead of once per press.
#[derive(Default)]
pub struct SaveTimer {
    dirty_since: Option<Instant>,
}

impl SaveTimer {
    pub const WINDOW: Duration = Duration::from_millis(500);

    pub fn mark(&mut self, now: Instant) {
        self.dirty_since.get_or_insert(now);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_since.is_some()
    }

    /// True once the window since the first unsaved change has passed;
    /// clears the mark.
    pub fn due(&mut self, now: Instant) -> bool {
        match self.dirty_since {
            Some(t) if now.saturating_duration_since(t) >= Self::WINDOW => {
                self.dirty_since = None;
                true
            }
            _ => false,
        }
    }

    /// Write now regardless of the window (menu closed, quitting): true if
    /// anything was pending; clears the mark.
    pub fn take(&mut self) -> bool {
        self.dirty_since.take().is_some()
    }
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
            wall: Some(false),
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
        assert_eq!(back.wall, Some(false));
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
        cfg.favorites = vec!["koi".into()];
        cfg.recents = vec!["fire".into()];
        reset_stored_defaults(&mut cfg, "rain");
        assert_eq!(cfg.scene.as_deref(), Some("rain"));
        assert_eq!(cfg.fps, None);
        assert!(cfg.filters.is_empty());
        assert_eq!(cfg.hue_shift, None);
        assert_eq!(cfg.group, None);
        assert!(cfg.themes.is_empty());
        assert_eq!(cfg.keys.get("quit").map(|s| s.as_str()), Some("x"));
        // favourites and recents are the user's content, not settings
        assert_eq!(cfg.favorites, vec!["koi"]);
        assert_eq!(cfg.recents, vec!["fire"]);
    }

    /// Everything at its built-in default.
    fn live_defaults() -> Live<'static> {
        Live {
            scene: DEFAULT_SCENE,
            theme: None,
            pixels: "half",
            default_pixels: "half",
            detail: "medium",
            default_detail: "medium",
            filters: &[],
            text_scale: None,
            fps: DEFAULT_FPS,
            speed: DEFAULT_SPEED,
            smooth: DEFAULT_SMOOTH,
            dim: DEFAULT_DIM,
            fade: DEFAULT_FADE,
            clock: DEFAULT_CLOCK,
            cycle: None,
            cycle_scope: CycleScope::All,
            hue_shift: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            renderer: Renderer::Auto,
            link: DEFAULT_LINK,
            group: DEFAULT_GROUP,
            wall: true,
        }
    }

    #[test]
    fn store_writes_no_defaults() {
        // a config full of stale values: storing defaults must clear them all
        let mut cfg = Config {
            fps: Some(60),
            speed: Some(2.0),
            dim: Some(0.5),
            clock: Some(false),
            gpu: Some(true),
            renderer: Some(Renderer::Cpu),
            group: Some("art".into()),
            cycle_scope: Some("favorites".into()),
            ..Default::default()
        };
        cfg.keys.insert("quit".into(), "x".into());
        store(&mut cfg, &live_defaults());
        let text = toml::to_string_pretty(&cfg).unwrap();
        // only the keybind table survives
        assert_eq!(
            text.trim(),
            "[keys]\nquit = \"x\"",
            "defaults leaked into:\n{text}"
        );
    }

    #[test]
    fn store_rounds_floats_to_two_decimals() {
        let mut cfg = Config::default();
        // float noise from repeated steps: 1.0 + 3 * 0.15 in f32
        let contrast = 1.0f32 + 0.15 + 0.15 + 0.15;
        assert_ne!(contrast, 1.45, "precondition: the sum carries noise");
        let live = Live {
            contrast,
            saturation: 0.7000001,
            dim: 0.70000005,
            fade: 0.35000002,
            hue_shift: 43.199997,
            speed: 1.2500001,
            ..live_defaults()
        };
        store(&mut cfg, &live);
        let text = toml::to_string_pretty(&cfg).unwrap();
        for want in [
            "contrast = 1.45",
            "saturation = 0.7",
            "dim = 0.7",
            "fade = 0.35",
            "hue_shift = 43.2",
            "speed = 1.25",
        ] {
            assert!(
                text.lines().any(|l| l == want),
                "missing `{want}` in:\n{text}"
            );
        }
    }

    #[test]
    fn store_theme_default_removes_entry() {
        let mut cfg = Config::default();
        cfg.themes.insert("fire".into(), "inferno".into());
        // fire's first theme is its default: stored as no entry
        store(
            &mut cfg,
            &Live {
                scene: "fire",
                theme: Some("classic"),
                ..live_defaults()
            },
        );
        assert!(!cfg.themes.contains_key("fire"));
        store(
            &mut cfg,
            &Live {
                scene: "fire",
                theme: Some("frost"),
                ..live_defaults()
            },
        );
        assert_eq!(cfg.themes.get("fire").map(String::as_str), Some("frost"));
        assert_eq!(cfg.scene.as_deref(), Some("fire"));
    }

    #[test]
    fn favorites_recents_and_scope_round_trip() {
        let cfg = Config {
            favorites: vec!["bigsur".into(), "koi".into()],
            recents: vec!["fire".into(), "rain".into()],
            cycle_scope: Some("favorites".into()),
            ..Default::default()
        };
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.favorites, vec!["bigsur", "koi"]);
        assert_eq!(back.recents, vec!["fire", "rain"]);
        assert_eq!(
            back.cycle_scope.as_deref().map(CycleScope::parse),
            Some(CycleScope::Favorites)
        );
        // absent lists load as empty, and an old config still parses
        let old: Config = toml::from_str("fps = 60\n").unwrap();
        assert!(old.favorites.is_empty() && old.recents.is_empty());
        assert_eq!(CycleScope::parse("nonsense"), CycleScope::All);
    }

    #[test]
    fn save_timer_coalesces_writes() {
        let t0 = Instant::now();
        let mut s = SaveTimer::default();
        assert!(!s.due(t0), "nothing marked, nothing due");
        s.mark(t0);
        // later marks inside the window do not push the write back
        s.mark(t0 + Duration::from_millis(300));
        assert!(!s.due(t0 + Duration::from_millis(400)));
        assert!(s.due(t0 + Duration::from_millis(500)));
        assert!(!s.is_dirty(), "due clears the mark");
        s.mark(t0 + Duration::from_millis(600));
        assert!(s.take(), "take flushes immediately");
        assert!(!s.take());
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
