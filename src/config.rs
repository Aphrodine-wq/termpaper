//! Config file: `config.toml` in the config directory (see
//! `platform::config_dir`: ~/.config/termpaper, or %APPDATA%\termpaper).
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
    /// terminal padding, so wall crops line up across window borders:
    /// `7` (px), `"3.5pt"` (points, e.g. kitty's `window_padding_width`),
    /// or `[x, y]` of either
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad: Option<PadSpec>,
    /// where the terminal puts the leftover strip when the window is not a
    /// whole number of cells: `"top-left"` (kitty's default: grid at the
    /// padding, leftover right/bottom) or `"center"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<crate::wall::Placement>,
    /// Studio scenes: cells whose colours move by at most this many levels
    /// (and keep their glyph) are re-sent unchanged, so the terminal diff
    /// skips them. 0 = off; default 3. Classic scenes never use it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hysteresis: Option<u8>,
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
    /// the theme the look came from (a slug in the theme store)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look_theme: Option<String>,
    /// action → key remaps
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub keys: HashMap<String, String>,
    /// the first-run welcome has been seen
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarded: Option<bool>,
    /// the theme gallery to install from and publish to (default: termpaper's
    /// website; `$TERMPAPER_GALLERY` wins)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gallery_url: Option<String>,
    /// a manual wall layout for this machine's panes, `COLSxROWS:INDEX`
    /// (what `--wall` takes), set from the Wall page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wall_grid: Option<String>,
    /// take the group's look when linked (default true); false keeps this
    /// pane's own
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_look: Option<bool>,
    /// The Look beyond what `filters`, `hue_shift`, `saturation` and
    /// `contrast` can say (effect strengths, the rest of the grade, the
    /// palette). Written in full when present; those four keys stay as
    /// mirrors so older binaries keep the basics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<crate::look::Look>,
    #[serde(default, skip_serializing_if = "is_default_playback")]
    pub playback: crate::prefs::PlaybackPrefs,
    #[serde(default, skip_serializing_if = "is_default_display")]
    pub display: crate::prefs::DisplayPrefs,
}

fn is_default_playback(p: &crate::prefs::PlaybackPrefs) -> bool {
    *p == crate::prefs::PlaybackPrefs::default()
}

fn is_default_display(d: &crate::prefs::DisplayPrefs) -> bool {
    *d == crate::prefs::DisplayPrefs::default()
}

/// The display preferences a config describes, taking the older flat keys
/// (`idle_fps`, `gpu_budget_ms`, `shader_fps`) where the `[display]` table
/// leaves them unset.
pub fn display_of(cfg: &Config) -> crate::prefs::DisplayPrefs {
    use crate::prefs::{Unfocused, STUDIO_BUDGET_MS, STUDIO_FPS};
    let mut d = cfg.display.clone();
    if d.unfocused == Unfocused::Keep {
        if let Some(f) = cfg.idle_fps {
            d.unfocused = if f <= 20 { Unfocused::Fps15 } else { Unfocused::Fps30 };
        }
    }
    if (d.studio_budget_ms - STUDIO_BUDGET_MS).abs() < 1e-6 {
        if let Some(b) = cfg.gpu_budget_ms {
            d.studio_budget_ms = b;
        }
    }
    if d.studio_fps == STUDIO_FPS {
        if let Some(f) = cfg.shader_fps {
            d.studio_fps = f;
        }
    }
    d.sanitize();
    d
}

/// The Look a config describes: its `[look]` table when there is one, else
/// the legacy keys.
pub fn look_of(cfg: &Config) -> crate::look::Look {
    let mut look = match &cfg.look {
        Some(l) => l.clone(),
        None => {
            let mut l = crate::look::Look::default();
            l.grade.hue = cfg.hue_shift.unwrap_or(0.0);
            l.grade.saturation = cfg.saturation.unwrap_or(1.0);
            l.grade.contrast = cfg.contrast.unwrap_or(1.0);
            l.effects.stack = cfg.filters.clone();
            l
        }
    };
    look.sanitize();
    look
}

/// One padding length: a number of px, or a string with a unit (`"3.5pt"`,
/// `"7px"`, `"7"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PadLen {
    Px(f64),
    Text(String),
}

/// Terminal padding: one length for both axes, or `[x, y]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PadSpec {
    One(PadLen),
    Two([PadLen; 2]),
}

/// CSS px per pt: terminals size points at 96 dpi logical.
const PX_PER_PT: f64 = 96.0 / 72.0;

impl PadLen {
    /// Length in the compositor's layout px. `scale` is layout px per
    /// logical px: 1 on Hyprland, whose window geometry is already logical
    /// (a terminal's points are logical too: 96 dpi × monitor scale in
    /// physical px). None for an unparsable string.
    pub fn px(&self, scale: f64) -> Option<f64> {
        let v = match self {
            PadLen::Px(v) => *v,
            PadLen::Text(s) => {
                let s = s.trim();
                if let Some(pt) = s.strip_suffix("pt") {
                    pt.trim().parse::<f64>().ok()? * PX_PER_PT * scale
                } else {
                    s.strip_suffix("px").unwrap_or(s).trim().parse::<f64>().ok()?
                }
            }
        };
        (v.is_finite() && v >= 0.0).then_some(v)
    }
}

impl PadSpec {
    /// (x, y) padding in layout px; unparsable parts count as 0.
    pub fn to_px(&self, scale: f64) -> (f32, f32) {
        let px = |l: &PadLen| l.px(scale).unwrap_or(0.0) as f32;
        match self {
            PadSpec::One(l) => (px(l), px(l)),
            PadSpec::Two([x, y]) => (px(x), px(y)),
        }
    }

    /// The `--pad` flag: `7`, `3.5pt`, or `x,y` of either.
    pub fn parse(s: &str) -> Option<PadSpec> {
        let len = |t: &str| {
            let l = PadLen::Text(t.trim().to_string());
            l.px(1.0).map(|_| l)
        };
        match s.split_once(',') {
            Some((x, y)) => Some(PadSpec::Two([len(x)?, len(y)?])),
            None => Some(PadSpec::One(len(s)?)),
        }
    }
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
    /// GPU scenes only
    Studio,
    Classic,
    /// the scene stays; its variants take turns
    Variants,
}

impl CycleScope {
    pub const ALL: [CycleScope; 6] = [
        CycleScope::All,
        CycleScope::Category,
        CycleScope::Favorites,
        CycleScope::Studio,
        CycleScope::Classic,
        CycleScope::Variants,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CycleScope::All => "all",
            CycleScope::Category => "category",
            CycleScope::Favorites => "favorites",
            CycleScope::Studio => "studio",
            CycleScope::Classic => "classic",
            CycleScope::Variants => "variants",
        }
    }

    /// Lenient: an unknown value falls back to `All` rather than failing the
    /// whole config file.
    pub fn parse(s: &str) -> Self {
        match s {
            "category" => CycleScope::Category,
            "favorites" | "favourites" => CycleScope::Favorites,
            "studio" => CycleScope::Studio,
            "classic" => CycleScope::Classic,
            "variants" => CycleScope::Variants,
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
    pub look: &'a crate::look::Look,
    /// the theme the look came from
    pub look_theme: Option<&'a str>,
    pub text_scale: Option<u32>,
    pub fps: u32,
    /// the terminal's frame-rate default; not stored when equal
    pub default_fps: u32,
    pub speed: f32,
    pub smooth: f32,
    pub dim: f32,
    pub fade: f32,
    pub clock: bool,
    pub cycle: Option<f64>,
    pub cycle_scope: CycleScope,
    pub renderer: Renderer,
    pub link: bool,
    pub group: &'a str,
    pub wall: bool,
    pub playback: &'a crate::prefs::PlaybackPrefs,
    pub display: &'a crate::prefs::DisplayPrefs,
}

/// Store a look (and the theme it came from): the four keys every binary
/// reads, then the rest under `[look]` when there is more.
pub fn set_look(cfg: &mut Config, look: &crate::look::Look, theme: Option<&str>) {
    cfg.filters = look.effects.stack.clone();
    cfg.hue_shift = (look.grade.hue >= 0.5).then(|| round2(look.grade.hue));
    cfg.saturation = (round2(look.grade.saturation) != 1.0).then(|| round2(look.grade.saturation));
    cfg.contrast = (round2(look.grade.contrast) != 1.0).then(|| round2(look.grade.contrast));
    cfg.look = look.has_extras().then(|| look.rounded());
    cfg.look_theme = theme.map(str::to_string);
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
    set_look(cfg, live.look, live.look_theme);
    cfg.text_scale = live.text_scale;
    cfg.fps = keep(live.fps, live.default_fps);
    cfg.speed = keep(round2(live.speed), DEFAULT_SPEED);
    cfg.smooth = keep(round2(live.smooth), DEFAULT_SMOOTH);
    cfg.dim = keep(round2(live.dim), DEFAULT_DIM);
    cfg.fade = keep(round2(live.fade), DEFAULT_FADE);
    cfg.clock = keep(live.clock, DEFAULT_CLOCK);
    cfg.cycle = live.cycle.map(|c| (c * 100.0).round() / 100.0);
    cfg.cycle_scope = keep(live.cycle_scope, CycleScope::All).map(|s| s.name().to_string());
    // `renderer` supersedes the legacy `gpu` switch it was derived from
    cfg.renderer = keep(live.renderer, Renderer::Auto);
    cfg.gpu = None;
    cfg.link = keep(live.link, DEFAULT_LINK);
    cfg.group = keep(live.group, DEFAULT_GROUP).map(str::to_string);
    cfg.wall = keep(live.wall, true);
    cfg.playback = live.playback.clone();
    cfg.display = live.display.clone();
    // now said by the [display] table
    cfg.idle_fps = None;
    cfg.gpu_budget_ms = None;
    cfg.shader_fps = None;
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
    crate::platform::config_dir().map(|d| d.join("config.toml"))
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
/// then rename over it. Creates the config directory if needed (explicit
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
            look: Box::leak(Box::new(crate::look::Look::default())),
            look_theme: None,
            text_scale: None,
            fps: DEFAULT_FPS,
            default_fps: DEFAULT_FPS,
            speed: DEFAULT_SPEED,
            smooth: DEFAULT_SMOOTH,
            dim: DEFAULT_DIM,
            fade: DEFAULT_FADE,
            clock: DEFAULT_CLOCK,
            cycle: None,
            cycle_scope: CycleScope::All,
            renderer: Renderer::Auto,
            link: DEFAULT_LINK,
            group: DEFAULT_GROUP,
            wall: true,
            playback: Box::leak(Box::default()),
            display: Box::leak(Box::default()),
        }
    }

    #[test]
    fn prefs_tables_and_their_older_keys() {
        use crate::prefs::{DisplayPrefs, TransitionStyle, Unfocused};
        // the flat keys older builds wrote still count
        let cfg: Config = toml::from_str("idle_fps = 15\ngpu_budget_ms = 5.0\nshader_fps = 30").unwrap();
        let d = display_of(&cfg);
        assert_eq!((d.unfocused, d.studio_budget_ms, d.studio_fps), (Unfocused::Fps15, 5.0, 30));
        // written back as the [display] table, the flat keys gone
        let mut cfg = cfg;
        let mut playback = crate::prefs::PlaybackPrefs::default();
        playback.transition = TransitionStyle::Blinds;
        store(&mut cfg, &Live { display: &d, playback: &playback, ..live_defaults() });
        let text = toml::to_string_pretty(&cfg).unwrap();
        assert!(!text.contains("idle_fps") && !text.contains("gpu_budget_ms") && !text.contains("shader_fps"), "{text}");
        assert!(text.contains("[display]") && text.contains("unfocused = \"15\"") && text.contains("studio_fps = 30"), "{text}");
        assert!(text.contains("[playback]\ntransition = \"blinds\""), "{text}");
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(display_of(&back), d);
        assert_eq!(back.playback, playback);
        // the table wins over a stale flat key
        let both: Config = toml::from_str("idle_fps = 30\n[display]\nunfocused = \"pause\"").unwrap();
        assert_eq!(display_of(&both).unfocused, Unfocused::Pause);
        assert_eq!(display_of(&Config::default()), DisplayPrefs::default());
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
        let mut look = crate::look::Look::default();
        look.grade.contrast = contrast;
        look.grade.saturation = 0.7000001;
        look.grade.hue = 43.199997;
        look.grade.temperature = 0.15000001;
        let live = Live {
            look: &look,
            dim: 0.70000005,
            fade: 0.35000002,
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
            // the rest of the look rides in [look], rounded the same way
            "temperature = 0.15",
        ] {
            assert!(
                text.lines().any(|l| l == want),
                "missing `{want}` in:\n{text}"
            );
        }
    }

    #[test]
    fn look_round_trips_and_legacy_keys_still_load() {
        use crate::look::{Look, PaletteMode, Rgb};
        // a basic look lives entirely in the legacy keys, no [look] table
        let mut basic = Look::default();
        basic.grade.saturation = 1.3;
        basic.effects.stack = vec!["bloom".into()];
        let mut cfg = Config::default();
        store(&mut cfg, &Live { look: &basic, ..live_defaults() });
        assert!(cfg.look.is_none());
        assert_eq!(cfg.filters, vec!["bloom"]);
        assert_eq!(look_of(&cfg), basic);
        // anything more goes to [look], with the legacy keys kept as mirrors
        let mut rich = basic.clone();
        rich.grade.vibrance = 0.4;
        rich.palette.mode = PaletteMode::Map;
        rich.palette.colors = vec![Rgb(0, 0, 0), Rgb(255, 200, 100)];
        rich.effects.set_amount("bloom", 0.5);
        store(&mut cfg, &Live { look: &rich, ..live_defaults() });
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(look_of(&back), rich, "{text}");
        assert_eq!(back.saturation, Some(1.3), "mirror for older binaries");
        // an old config with only legacy keys
        let old: Config = toml::from_str("filters = [\"crt\"]\nhue_shift = 90.0\n").unwrap();
        let l = look_of(&old);
        assert_eq!(l.effects.stack, vec!["crt"]);
        assert_eq!(l.grade.hue, 90.0);
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
    fn pad_accepts_px_points_and_pairs() {
        let parse = |t: &str| toml::from_str::<Config>(t).unwrap().pad.unwrap().to_px(1.0);
        assert_eq!(parse("pad = 7"), (7.0, 7.0));
        assert_eq!(parse("pad = 4.5"), (4.5, 4.5));
        let (x, y) = parse("pad = \"3.5pt\"");
        assert!((x - 4.6667).abs() < 1e-3 && x == y, "3.5pt is {x}px");
        assert_eq!(parse("pad = \"7px\""), (7.0, 7.0));
        let (x, y) = parse("pad = [6, \"3pt\"]");
        assert_eq!((x, y), (6.0, 4.0));
        // at 2x (layout px per logical px) points double
        let two = toml::from_str::<Config>("pad = \"3pt\"").unwrap().pad.unwrap().to_px(2.0);
        assert_eq!(two, (8.0, 8.0));
        // the --pad flag
        assert_eq!(PadSpec::parse("7").unwrap().to_px(1.0), (7.0, 7.0));
        assert_eq!(PadSpec::parse("3pt,2").unwrap().to_px(1.0), (4.0, 2.0));
        assert!(PadSpec::parse("wide").is_none());
        assert!(PadSpec::parse("-3").is_none());
        // round-trips through the saved config in the user's own form
        let cfg: Config = toml::from_str("pad = \"3.5pt\"\nplacement = \"center\"\nhysteresis = 2").unwrap();
        let back: Config = toml::from_str(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!(back.pad, Some(PadSpec::One(PadLen::Text("3.5pt".into()))));
        assert_eq!(back.placement, Some(crate::wall::Placement::Center));
        assert_eq!(back.hysteresis, Some(2));
        let tl: Config = toml::from_str("placement = \"top-left\"").unwrap();
        assert_eq!(tl.placement, Some(crate::wall::Placement::TopLeft));
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
