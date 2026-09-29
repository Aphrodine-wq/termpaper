//! Themes: a named, shareable Look.
//!
//! A theme is a small TOML file: a name and credits, the look (`[grade]`,
//! `[palette]`, `[effects]`, exactly as the colour studio sets them), and
//! optionally a scene it was made for and display hints. It works on every
//! scene, because it only changes how a frame is finished, never what a
//! scene draws.
//!
//! ```toml
//! format = 1
//! name = "Tokyo Night"
//! author = "you"
//! tags = ["terminal", "dark", "blue"]
//!
//! [grade]
//! temperature = -0.15
//! vibrance = 0.25
//!
//! [palette]
//! mode = "tint"
//! strength = 0.65
//! colors = ["#1a1b26", "#414868", "#7aa2f7", "#bb9af7", "#c0caf5"]
//!
//! [effects]
//! stack = ["bloom", "vignette"]
//! bloom = 0.6
//!
//! [scene]
//! name = "tokyo"
//! variant = "rain"
//! ```
//!
//! Themes travel as files, or as share codes: `tp1:` plus the theme as
//! compact JSON, deflated and base64url-encoded, short enough to paste into a
//! chat. Built-in themes live in `src/themes/`, one file each (build.rs
//! embeds them); yours live in `<config dir>/themes/`.

use crate::look::Look;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The theme format this build writes and understands.
pub const FORMAT: u32 = 1;

pub const MAX_NAME: usize = 40;
pub const MAX_AUTHOR: usize = 32;
pub const MAX_DESCRIPTION: usize = 160;
pub const MAX_TAGS: usize = 8;
pub const MAX_TAG: usize = 20;

/// The scene a theme was made for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneHint {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Display settings a theme suggests.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayHint {
    /// brightness, 0.2 ..= 1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub format: u32,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(flatten)]
    pub look: Look,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneHint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<DisplayHint>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            format: FORMAT,
            name: "Untitled".into(),
            author: String::new(),
            description: String::new(),
            tags: Vec::new(),
            look: Look::default(),
            scene: None,
            display: None,
        }
    }
}

/// Keys a theme file may carry, per table ("" is the top level).
const KNOWN: &[(&str, &[&str])] = &[
    ("", &["format", "name", "author", "description", "tags", "grade", "palette", "effects", "scene", "display"]),
    (
        "grade",
        &[
            "hue", "saturation", "contrast", "exposure", "vibrance", "temperature", "tint", "gamma", "fade", "shadows",
            "midtones", "highlights", "balance",
        ],
    ),
    ("palette", &["mode", "colors", "strength", "dither"]),
    ("scene", &["name", "variant"]),
    ("display", &["dim"]),
];

/// Printable text only: no control characters (newlines, escapes).
fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

impl Theme {
    /// Read a theme from TOML. Unknown keys and out-of-range values come
    /// back as warnings (the theme still loads, cleaned up); a broken file or
    /// a newer format is an error.
    pub fn from_toml(text: &str) -> Result<(Theme, Vec<String>), String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| format!("not a theme file: {e}"))?;
        let mut warnings = Vec::new();
        for (key, value) in &table {
            match KNOWN.iter().find(|(t, _)| t.is_empty()).map(|(_, k)| k.contains(&key.as_str())) {
                Some(false) => warnings.push(format!("unknown key `{key}` (ignored)")),
                _ => {
                    if let (Some(sub), Some(allowed)) = (value.as_table(), KNOWN.iter().find(|(t, _)| *t == key)) {
                        for k in sub.keys() {
                            if !allowed.1.contains(&k.as_str()) {
                                warnings.push(format!("unknown key `{key}.{k}` (ignored)"));
                            }
                        }
                    }
                }
            }
        }
        let theme: Theme = toml::from_str(text).map_err(|e| format!("not a theme file: {e}"))?;
        let mut theme = theme;
        warnings.extend(theme.validate()?);
        Ok((theme, warnings))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// Clean the theme up for use, returning what had to change. Errors
    /// are what cannot be fixed: a newer format, no name.
    pub fn validate(&mut self) -> Result<Vec<String>, String> {
        let mut w = Vec::new();
        if self.format > FORMAT {
            return Err(format!(
                "made for theme format {} (this termpaper reads {FORMAT}); update termpaper",
                self.format
            ));
        }
        self.format = FORMAT;
        let name = clean(&self.name, MAX_NAME);
        if name.is_empty() {
            return Err("a theme needs a name".into());
        }
        if name.chars().count() < self.name.trim().chars().count() {
            w.push(format!("name shortened to {MAX_NAME} characters"));
        }
        self.name = name;
        self.author = clean(&self.author, MAX_AUTHOR);
        let desc = clean(&self.description, MAX_DESCRIPTION);
        if desc.chars().count() < self.description.trim().chars().count() {
            w.push(format!("description shortened to {MAX_DESCRIPTION} characters"));
        }
        self.description = desc;
        let tags: Vec<String> = self
            .tags
            .iter()
            .map(|t| clean(&t.to_lowercase(), MAX_TAG))
            .filter(|t| !t.is_empty())
            .take(MAX_TAGS)
            .collect();
        if tags.len() < self.tags.len() {
            w.push(format!("kept {} tags", tags.len()));
        }
        self.tags = tags;
        // effects this build does not have are dropped (a newer theme)
        let before = self.look.effects.stack.len();
        self.look.effects.stack.retain(|e| crate::filter::FILTER_CYCLE.contains(&e.as_str()));
        if self.look.effects.stack.len() < before {
            w.push("dropped effects this termpaper does not have".into());
        }
        let raw = self.look.clone();
        self.look.sanitize();
        if self.look != raw {
            w.push("some values were out of range and were clamped".into());
        }
        if let Some(s) = &mut self.scene {
            match crate::scene::lookup(&s.name) {
                None => {
                    w.push(format!("scene `{}` is not in this termpaper", s.name));
                    self.scene = None;
                }
                Some(e) => {
                    if let Some(v) = &s.variant {
                        if !e.themes().contains(&v.as_str()) {
                            w.push(format!("scene `{}` has no variant `{v}`", s.name));
                            s.variant = None;
                        }
                    }
                }
            }
        }
        if let Some(d) = &mut self.display {
            if let Some(dim) = d.dim {
                d.dim = Some(if dim.is_finite() { dim.clamp(0.2, 1.0) } else { 1.0 });
            }
        }
        Ok(w)
    }

    /// The theme as a share code: `tp1:` + base64url(deflate(JSON)).
    pub fn to_code(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let packed = miniz_oxide::deflate::compress_to_vec(json.as_bytes(), 9);
        format!("tp1:{}", base64url_encode(&packed))
    }

    /// Read a share code (surrounding whitespace and line breaks are fine).
    pub fn from_code(code: &str) -> Result<(Theme, Vec<String>), String> {
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let body = code
            .strip_prefix("tp1:")
            .ok_or("not a termpaper theme code (they start with tp1:)")?;
        let packed = base64url_decode(body).ok_or("the code is damaged (not base64url)")?;
        let json = miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, 64 * 1024)
            .map_err(|_| "the code is damaged (does not unpack)")?;
        let mut theme: Theme =
            serde_json::from_slice(&json).map_err(|e| format!("the code does not hold a theme: {e}"))?;
        let w = theme.validate()?;
        Ok((theme, w))
    }

    /// The theme's category shelf: its first tag, when that is one of the
    /// built-in categories.
    pub fn category(&self) -> Option<&str> {
        self.tags.first().map(String::as_str).filter(|t| CATEGORIES.iter().any(|(c, _)| c == t))
    }
}

/// Built-in theme categories: (tag, shelf heading).
pub const CATEGORIES: &[(&str, &str)] = &[
    ("cinematic", "Cinematic"),
    ("terminal", "Terminal palettes"),
    ("retro", "Retro"),
    ("mood", "Mood"),
];

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding.
pub fn base64url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

pub fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let val = |c: u8| B64.iter().position(|&b| b == c).map(|p| p as u32);
    let bytes = text.trim_end_matches('=').as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * i);
        }
        for i in 0..chunk.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

/// A file-name-safe id from a theme name: "Rosé Pine" → "rose-pine".
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars().flat_map(char::to_lowercase) {
        let c = match ch {
            'a'..='z' | '0'..='9' => ch,
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            '&' => {
                if !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
                out.push_str("and");
                continue;
            }
            _ => '-',
        };
        if c == '-' && (out.is_empty() || out.ends_with('-')) {
            continue;
        }
        out.push(c);
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "theme".into()
    } else {
        out.chars().take(48).collect()
    }
}

// ── the store ───────────────────────────────────────────────────────────

include!(concat!(env!("OUT_DIR"), "/builtin_themes.rs"));

/// Where a theme came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// ships with termpaper
    Builtin,
    /// a file in your themes folder (made here, imported or installed)
    User,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub slug: String,
    pub theme: Theme,
    pub source: Source,
    /// the file, for your own themes
    pub path: Option<PathBuf>,
}

/// Every theme: built-ins first (by category), then yours.
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub entries: Vec<Entry>,
}

/// Your themes folder: `<config dir>/themes`.
pub fn user_dir() -> Option<PathBuf> {
    crate::platform::config_dir().map(|d| d.join("themes"))
}

/// The built-in themes, parsed.
pub fn builtin() -> Vec<Entry> {
    let mut v: Vec<Entry> = BUILTIN
        .iter()
        .filter_map(|(slug, text)| {
            let (theme, _) = Theme::from_toml(text).ok()?;
            Some(Entry { slug: slug.to_string(), theme, source: Source::Builtin, path: None })
        })
        .collect();
    let rank = |e: &Entry| {
        e.theme
            .category()
            .and_then(|c| CATEGORIES.iter().position(|(t, _)| *t == c))
            .unwrap_or(CATEGORIES.len())
    };
    // "clean" (the neutral look) first, then by category in file order
    v.sort_by_key(|e| (e.slug != "clean", rank(e)));
    v
}

impl Store {
    /// Built-ins plus every readable file in `dir` (your themes folder).
    pub fn load_from(dir: Option<&Path>) -> Store {
        let mut entries = builtin();
        if let Some(dir) = dir {
            let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
                .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect())
                .unwrap_or_default();
            files.sort();
            for path in files {
                let Some(slug) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                    continue;
                };
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if let Ok((theme, _)) = Theme::from_toml(&text) {
                    entries.push(Entry { slug, theme, source: Source::User, path: Some(path) });
                }
            }
        }
        Store { entries }
    }

    pub fn load() -> Store {
        Self::load_from(user_dir().as_deref())
    }

    /// Your theme first when a slug is both yours and built in.
    pub fn get(&self, slug: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.slug == slug && e.source == Source::User)
            .or_else(|| self.entries.iter().find(|e| e.slug == slug))
    }

    /// The theme with this name (case-insensitive) or slug.
    pub fn find(&self, name_or_slug: &str) -> Option<&Entry> {
        self.get(name_or_slug)
            .or_else(|| self.get(&slugify(name_or_slug)))
            .or_else(|| self.entries.iter().find(|e| e.theme.name.eq_ignore_ascii_case(name_or_slug)))
    }

    fn unique_slug(&self, base: &str, dir: &Path) -> String {
        let taken = |s: &str| self.entries.iter().any(|e| e.slug == s) || dir.join(format!("{s}.toml")).exists();
        if !taken(base) {
            return base.to_string();
        }
        (2..).map(|n| format!("{base}-{n}")).find(|s| !taken(s)).unwrap()
    }

    /// Save a theme into `dir` under a new, unused slug. Returns the slug.
    pub fn save_new_in(&mut self, dir: &Path, theme: &Theme) -> std::io::Result<String> {
        std::fs::create_dir_all(dir)?;
        let slug = self.unique_slug(&slugify(&theme.name), dir);
        let path = dir.join(format!("{slug}.toml"));
        write_atomic(&path, &theme.to_toml())?;
        self.entries.push(Entry { slug: slug.clone(), theme: theme.clone(), source: Source::User, path: Some(path) });
        Ok(slug)
    }

    pub fn save_new(&mut self, theme: &Theme) -> std::io::Result<String> {
        let dir = user_dir().ok_or_else(|| std::io::Error::other("no config directory"))?;
        self.save_new_in(&dir, theme)
    }

    /// Replace one of your themes' look (and anything else) in place.
    pub fn update(&mut self, slug: &str, theme: &Theme) -> std::io::Result<()> {
        let e = self
            .entries
            .iter_mut()
            .rev()
            .find(|e| e.slug == slug && e.source == Source::User)
            .ok_or_else(|| std::io::Error::other("only your own themes can be updated"))?;
        let path = e.path.clone().ok_or_else(|| std::io::Error::other("theme has no file"))?;
        write_atomic(&path, &theme.to_toml())?;
        e.theme = theme.clone();
        Ok(())
    }

    /// Rename one of your themes: new name, new file. Returns the new slug.
    pub fn rename(&mut self, slug: &str, name: &str) -> std::io::Result<String> {
        let i = self
            .entries
            .iter()
            .rposition(|e| e.slug == slug && e.source == Source::User)
            .ok_or_else(|| std::io::Error::other("only your own themes can be renamed"))?;
        let old = self.entries.remove(i);
        let mut theme = old.theme.clone();
        theme.name = clean(name, MAX_NAME);
        if theme.name.is_empty() {
            self.entries.insert(i, old);
            return Err(std::io::Error::other("a theme needs a name"));
        }
        let dir = old.path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf).or_else(user_dir);
        let dir = dir.ok_or_else(|| std::io::Error::other("no config directory"))?;
        match self.save_new_in(&dir, &theme) {
            Ok(slug) => {
                if let Some(p) = &old.path {
                    let _ = std::fs::remove_file(p);
                }
                Ok(slug)
            }
            Err(e) => {
                self.entries.insert(i, old);
                Err(e)
            }
        }
    }

    /// Delete one of your themes (built-ins cannot be deleted).
    pub fn delete(&mut self, slug: &str) -> std::io::Result<()> {
        let i = self
            .entries
            .iter()
            .rposition(|e| e.slug == slug && e.source == Source::User)
            .ok_or_else(|| std::io::Error::other("only your own themes can be deleted"))?;
        if let Some(p) = &self.entries[i].path {
            std::fs::remove_file(p)?;
        }
        self.entries.remove(i);
        Ok(())
    }
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{PaletteMode, Rgb};

    fn sample() -> Theme {
        let mut t = Theme {
            name: "Tokyo Night".into(),
            author: "you".into(),
            description: "Any scene in the Tokyo Night palette.".into(),
            tags: vec!["terminal".into(), "dark".into()],
            scene: Some(SceneHint { name: "tokyo".into(), variant: Some("rain".into()) }),
            ..Theme::default()
        };
        t.look.grade.temperature = -0.15;
        t.look.grade.vibrance = 0.25;
        t.look.palette.mode = PaletteMode::Tint;
        t.look.palette.strength = 0.65;
        t.look.palette.colors = vec![Rgb(0x1a, 0x1b, 0x26), Rgb(0x7a, 0xa2, 0xf7), Rgb(0xc0, 0xca, 0xf5)];
        t.look.effects.stack = vec!["bloom".into(), "vignette".into()];
        t.look.effects.set_amount("bloom", 0.6);
        t
    }

    #[test]
    fn toml_reads_the_documented_format() {
        let text = r##"
format = 1
name = "Tokyo Night"
author = "you"
tags = ["terminal", "dark", "blue"]

[grade]
temperature = -0.15
vibrance = 0.25

[palette]
mode = "tint"
strength = 0.65
colors = ["#1a1b26", "#414868", "#7aa2f7", "#bb9af7", "#c0caf5"]

[effects]
stack = ["bloom", "vignette"]
bloom = 0.6

[scene]
name = "tokyo"
variant = "rain"
"##;
        let (t, w) = Theme::from_toml(text).unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(t.name, "Tokyo Night");
        assert_eq!(t.look.grade.temperature, -0.15);
        assert_eq!(t.look.palette.mode, PaletteMode::Tint);
        assert_eq!(t.look.palette.colors.len(), 5);
        assert_eq!(t.look.effects.stack, vec!["bloom", "vignette"]);
        assert_eq!(t.look.effects.amount("bloom"), 0.6);
        assert_eq!(t.look.effects.amount("vignette"), 1.0);
        assert_eq!(t.scene.as_ref().unwrap().variant.as_deref(), Some("rain"));
    }

    #[test]
    fn toml_round_trip() {
        let t = sample();
        let text = t.to_toml();
        let (back, w) = Theme::from_toml(&text).unwrap();
        assert!(w.is_empty(), "{w:?}\n{text}");
        assert_eq!(back, t, "{text}");
        // defaults stay out of the file
        assert!(!text.contains("saturation"), "{text}");
    }

    #[test]
    fn share_codes_round_trip_and_stay_short() {
        let t = sample();
        let code = t.to_code();
        assert!(code.starts_with("tp1:"));
        assert!(code.len() < 400, "{} chars: {code}", code.len());
        let (back, w) = Theme::from_code(&code).unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(back, t);
        // pasted with line breaks and spaces
        let wrapped: String = code.chars().enumerate().flat_map(|(i, c)| if i % 20 == 19 { vec![c, '\n', ' '] } else { vec![c] }).collect();
        assert_eq!(Theme::from_code(&wrapped).unwrap().0, t);
        // damage is reported, not a panic
        assert!(Theme::from_code("tp1:!!!!").is_err());
        assert!(Theme::from_code("hello").is_err());
        assert!(Theme::from_code(&code[..code.len() - 6]).is_err());
    }

    #[test]
    fn validation_cleans_and_warns() {
        let text = r##"
name = "  A\u0007 theme  "
description = "x"
wobble = 3
tags = ["A", "b"]
[grade]
exposure = 9.0
sparkle = 1
[effects]
stack = ["bloom", "unobtainium"]
[scene]
name = "no-such-scene"
"##;
        let (t, w) = Theme::from_toml(text).unwrap();
        assert_eq!(t.name, "A theme");
        assert_eq!(t.look.grade.exposure, 2.0);
        assert_eq!(t.look.effects.stack, vec!["bloom"]);
        assert!(t.scene.is_none());
        assert_eq!(t.tags, vec!["a", "b"]);
        let all = w.join(" | ");
        for want in ["wobble", "grade.sparkle", "clamped", "unobtainium", "no-such-scene"] {
            let hit = all.contains(want) || (want == "unobtainium" && all.contains("dropped effects"));
            assert!(hit, "missing warning about {want}: {all}");
        }
        // a newer format and a missing name are errors
        assert!(Theme::from_toml("format = 9\nname = \"x\"").is_err());
        assert!(Theme::from_toml("name = \"   \"").is_err());
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Rosé Pine"), "rose-pine");
        assert_eq!(slugify("Teal & Orange"), "teal-and-orange");
        assert_eq!(slugify("  --Hello,   World!!  "), "hello-world");
        assert_eq!(slugify("日本"), "theme");
    }

    #[test]
    fn base64url_round_trip() {
        for len in 0..40 {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(base64url_decode(&base64url_encode(&data)).unwrap(), data);
        }
        assert!(base64url_decode("a").is_none());
    }

    #[test]
    fn every_builtin_theme_loads_cleanly() {
        assert!(!BUILTIN.is_empty());
        for (slug, text) in BUILTIN {
            let (t, w) = Theme::from_toml(text).unwrap_or_else(|e| panic!("{slug}: {e}"));
            assert!(w.is_empty(), "{slug}: {w:?}");
            assert_eq!(slugify(&t.name), *slug, "{slug}: file name should be the name's slug");
            assert!(!t.description.is_empty(), "{slug} needs a description");
            if *slug != "clean" {
                assert!(t.category().is_some(), "{slug}: first tag must be a category");
                assert!(!t.look.is_neutral(), "{slug} changes nothing");
            }
        }
        // names are unique
        let mut names: Vec<_> = builtin().into_iter().map(|e| e.theme.name).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n);
    }

    #[test]
    fn store_saves_renames_updates_and_deletes_user_themes() {
        let dir = std::env::temp_dir().join(format!("termpaper-themes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = Store::load_from(Some(&dir));
        let builtins = store.entries.len();
        let slug = store.save_new_in(&dir, &sample()).unwrap();
        assert_eq!(slug, "tokyo-night-2", "the built-in owns tokyo-night");
        // a second save of the same name gets its own slug
        let slug2 = store.save_new_in(&dir, &sample()).unwrap();
        assert_ne!(slug, slug2);
        // reload from disk: both come back as yours
        let store2 = Store::load_from(Some(&dir));
        assert_eq!(store2.entries.len(), builtins + 2);
        assert_eq!(store2.get(&slug).unwrap().source, Source::User);
        // update, rename, delete
        let mut t = sample();
        t.look.grade.exposure = 1.0;
        store.update(&slug, &t).unwrap();
        assert_eq!(Store::load_from(Some(&dir)).get(&slug).unwrap().theme.look.grade.exposure, 1.0);
        let renamed = store.rename(&slug, "Night Drive").unwrap();
        assert_eq!(renamed, "night-drive");
        assert!(store.get(&slug).is_none());
        store.delete(&renamed).unwrap();
        store.delete(&slug2).unwrap();
        assert_eq!(Store::load_from(Some(&dir)).entries.len(), builtins);
        // built-ins are read-only
        assert!(store.delete("teal-and-orange").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
