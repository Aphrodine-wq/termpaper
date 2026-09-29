//! `termpaper theme …`: themes from the command line. Each command is a
//! function from the store (and its inputs) to the text it prints, so the
//! binary only does the I/O around it.

use crate::look::{Look, PaletteMode};
use crate::theme::{Entry, Source, Store, Theme};
use std::path::Path;

/// A path as people write it: `~/…` under the home directory.
pub fn tidy_path(p: &Path) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    if let Some(rest) = home.and_then(|h| p.strip_prefix(h).ok().map(Path::to_path_buf)) {
        return format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display());
    }
    p.display().to_string()
}

/// Six truecolour blocks that show what a look does ("" without colour).
fn swatch_line(look: &Look, color: bool) -> String {
    if !color {
        return String::new();
    }
    crate::menu::themes::swatches(look)
        .iter()
        .map(|(r, g, b)| format!("\x1b[38;2;{r};{g};{b}m██\x1b[0m"))
        .collect()
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// `theme list`: every theme under its shelf heading, with swatches when
/// `color`; `user_dir` is shown beside your own. Descriptions are cut to
/// fit `width` columns when there is one (a terminal, not a pipe).
pub fn list(store: &Store, color: bool, user_dir: Option<&Path>, width: Option<usize>) -> String {
    let rows = crate::menu::themes::rows(store);
    let mut out = String::new();
    let mut shelf = "";
    for r in &rows {
        if r.shelf != shelf {
            shelf = r.shelf;
            out.push('\n');
            out.push_str(shelf);
            if shelf == "Yours" {
                if let Some(d) = user_dir {
                    out.push_str(&format!("  ({})", tidy_path(d)));
                }
            }
            out.push('\n');
        }
        let e = store.get(&r.slug).expect("rows come from the store");
        let sw = swatch_line(&e.theme.look, color);
        let sw_cols = if sw.is_empty() { 0 } else { 14 };
        let sw = if sw.is_empty() { String::new() } else { format!("{sw}  ") };
        let desc = match width {
            Some(w) => cut(&r.description, w.saturating_sub(2 + sw_cols + 21).max(20)),
            None => r.description.clone(),
        };
        out.push_str(&format!("  {sw}{:<20} {desc}\n", r.slug));
    }
    if !rows.iter().any(|r| r.yours) {
        out.push_str("\nYours\n  none yet: save one from the Themes page (n), or `termpaper theme new NAME`\n");
    }
    out.push_str("\nUse one:  termpaper --theme NAME   ·   running panes: termpaper theme apply NAME\n");
    out
}

/// `theme browse`: themes shared in the gallery, id first (what `theme
/// install` takes), with swatches when the terminal shows colour.
pub fn gallery_list(list: &[crate::gallery::Listed], color: bool, width: Option<usize>, host: &str, more: bool) -> String {
    if list.is_empty() {
        return format!("Nothing shared at {host} matches that yet.\n");
    }
    let mut out = format!("Shared at {host}\n\n");
    for l in list {
        let sw = l.theme().map(|t| swatch_line(&t.look, color)).unwrap_or_default();
        let pad = if sw.is_empty() { String::new() } else { " ".repeat(14) };
        let sw = if sw.is_empty() { String::new() } else { format!("{sw}  ") };
        let by = match (l.builtin, l.author.trim()) {
            (true, _) => "built in".to_string(),
            (false, "") => "by someone".to_string(),
            (false, a) => format!("by {a}"),
        };
        let stats = if l.builtin { String::new() } else { format!(" · {} installs", l.installs) };
        out.push_str(&format!("  {sw}{:<10} {} ({by}{stats})\n", l.id, l.name));
        if !l.description.is_empty() {
            let desc = match width {
                Some(w) => cut(&l.description, w.saturating_sub(2 + pad.len() + 11).max(20)),
                None => l.description.clone(),
            };
            out.push_str(&format!("  {pad}{:<10} {desc}\n", ""));
        }
    }
    if more {
        out.push_str("  … and more: add words to narrow it down, or browse the gallery online\n");
    }
    out.push_str("\nInstall one:  termpaper theme install ID\n");
    out
}

/// `theme list --json`: one object per theme, with its share code.
pub fn list_json(store: &Store) -> String {
    // serialized straight from the structs, not through serde_json::Value,
    // which would widen every f32 (0.7 → 0.699999988079071)
    #[derive(serde::Serialize)]
    struct Item<'a> {
        slug: &'a str,
        shelf: &'a str,
        yours: bool,
        code: String,
        theme: &'a Theme,
    }
    let rows = crate::menu::themes::rows(store);
    let items: Vec<Item> = rows
        .iter()
        .filter_map(|r| {
            let e = store.get(&r.slug)?;
            Some(Item { slug: &e.slug, shelf: r.shelf, yours: r.yours, code: e.theme.to_code(), theme: &e.theme })
        })
        .collect();
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".into())
}

fn find<'a>(store: &'a Store, name: &str) -> Result<&'a Entry, String> {
    store.find(name).ok_or_else(|| {
        let mut close: Vec<&str> = store
            .entries
            .iter()
            .filter(|e| e.slug.contains(&name.to_lowercase()) || e.theme.name.to_lowercase().contains(&name.to_lowercase()))
            .map(|e| e.slug.as_str())
            .collect();
        close.truncate(5);
        if close.is_empty() {
            format!("no theme called '{name}' (see `termpaper theme list`)")
        } else {
            format!("no theme called '{name}'; did you mean {}?", close.join(", "))
        }
    })
}

fn signed_pct(v: f32) -> String {
    format!("{:+.0}", v * 100.0)
}

/// The grade, palette and effects of a look in words, one line each.
pub fn describe_look(look: &Look) -> Vec<(&'static str, String)> {
    let g = &look.grade;
    let d = crate::look::Grade::default();
    let mut colour = Vec::new();
    if g.exposure.abs() >= 0.05 {
        colour.push(format!("exposure {:+.1} EV", g.exposure));
    }
    if (g.contrast - d.contrast).abs() >= 0.005 {
        colour.push(format!("contrast {:.2}", g.contrast));
    }
    if (g.saturation - d.saturation).abs() >= 0.005 {
        colour.push(format!("saturation {:.0}%", g.saturation * 100.0));
    }
    if g.vibrance.abs() >= 0.005 {
        colour.push(format!("vibrance {}", signed_pct(g.vibrance)));
    }
    if g.temperature.abs() >= 0.005 {
        colour.push(format!("temperature {}", signed_pct(g.temperature)));
    }
    if g.tint.abs() >= 0.005 {
        colour.push(format!("tint {}", signed_pct(g.tint)));
    }
    if g.hue >= 0.5 {
        colour.push(format!("hue {:.0}°", g.hue));
    }
    if (g.gamma - d.gamma).abs() >= 0.005 {
        colour.push(format!("gamma {:.2}", g.gamma));
    }
    if g.fade >= 0.005 {
        colour.push(format!("matte {:.0}%", g.fade * 100.0));
    }
    let mut tones = Vec::new();
    for (name, t) in [("shadows", g.shadows), ("midtones", g.midtones), ("highlights", g.highlights)] {
        if t.amount >= 0.005 {
            tones.push(format!("{name} {:.0}° {:.0}%", t.hue, t.amount * 100.0));
        }
    }
    if g.balance.abs() >= 0.005 && !tones.is_empty() {
        tones.push(format!("balance {}", signed_pct(g.balance)));
    }
    let p = &look.palette;
    let palette = match p.mode {
        PaletteMode::Off => "off".to_string(),
        m => {
            let colours: Vec<String> = p.colors.iter().map(|c| format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)).collect();
            let name = crate::look::palette_name(&p.colors).map(|n| format!("{n}: ")).unwrap_or_default();
            let strength = match m {
                PaletteMode::Snap if p.dither => " · dithered".to_string(),
                PaletteMode::Snap => String::new(),
                _ => format!(" · {:.0}%", p.strength * 100.0),
            };
            format!("{} {name}{}{strength}", m.name(), colours.join(" "))
        }
    };
    let effects = if look.effects.stack.is_empty() {
        "none".to_string()
    } else {
        look.effects
            .stack
            .iter()
            .map(|f| {
                if crate::filter::has_amount(f) {
                    format!("{f} {:.0}%", look.effects.amount(f) * 100.0)
                } else {
                    f.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let none = |v: Vec<String>| if v.is_empty() { "neutral".to_string() } else { v.join(" · ") };
    let mut out = vec![("colour", none(colour))];
    if !tones.is_empty() {
        out.push(("tones", tones.join(" · ")));
    }
    out.push(("palette", palette));
    out.push(("effects", effects));
    out
}

/// `theme show NAME`: what the theme does, where it lives, its share code.
pub fn show(store: &Store, name: &str, color: bool) -> Result<String, String> {
    let e = find(store, name)?;
    let t = &e.theme;
    let kind = match (&e.source, t.category()) {
        (Source::User, _) => "yours".to_string(),
        (Source::Builtin, Some(c)) => format!("built in · {c}"),
        (Source::Builtin, None) => "built in".to_string(),
    };
    let mut out = format!("{}  ({kind})\n", t.name);
    if !t.author.is_empty() {
        out.push_str(&format!("by {}\n", t.author));
    }
    if !t.description.is_empty() {
        out.push_str(&format!("{}\n", t.description));
    }
    out.push('\n');
    let sw = swatch_line(&t.look, color);
    if !sw.is_empty() {
        out.push_str(&format!("  {:<10}{sw}\n", "looks"));
    }
    for (k, v) in describe_look(&t.look) {
        out.push_str(&format!("  {k:<10}{v}\n"));
    }
    if let Some(s) = &t.scene {
        let v = s.variant.as_deref().map(|v| format!(" --variant {v}")).unwrap_or_default();
        out.push_str(&format!("  {:<10}{}  (termpaper {}{v} --theme {})\n", "made for", s.name, s.name, e.slug));
    }
    if !t.tags.is_empty() {
        out.push_str(&format!("  {:<10}{}\n", "tags", t.tags.join(", ")));
    }
    match &e.path {
        Some(p) => out.push_str(&format!("  {:<10}{}\n", "file", tidy_path(p))),
        None => out.push_str(&format!("  {:<10}built in: `termpaper theme export {}` prints it\n", "file", e.slug)),
    }
    out.push_str(&format!("  {:<10}{}\n", "share", t.to_code()));
    Ok(out)
}

/// `theme export NAME [--code]`: the theme file, or its share code.
pub fn export(store: &Store, name: &str, code: bool) -> Result<String, String> {
    let e = find(store, name)?;
    Ok(if code { format!("{}\n", e.theme.to_code()) } else { e.theme.to_toml() })
}

/// Read a theme from a share code, a file, or `-` (the text itself, read
/// by the caller from stdin).
pub fn parse_source(src: &str, stdin: impl FnOnce() -> std::io::Result<String>) -> Result<(Theme, Vec<String>), String> {
    let src = src.trim();
    if src.starts_with("tp1:") {
        return Theme::from_code(src);
    }
    let text = if src == "-" {
        stdin().map_err(|e| format!("stdin: {e}"))?
    } else {
        std::fs::read_to_string(src).map_err(|e| format!("{src}: {e}"))?
    };
    let text = text.trim();
    if text.starts_with("tp1:") {
        Theme::from_code(text)
    } else {
        Theme::from_toml(text)
    }
}

/// `theme import`: save a parsed theme as one of yours, optionally renamed.
pub fn import(store: &mut Store, dir: &Path, theme: Theme, warnings: &[String], name: Option<&str>) -> Result<String, String> {
    let mut theme = theme;
    if let Some(n) = name {
        theme.name = n.to_string();
        theme.validate()?;
    }
    let slug = store.save_new_in(dir, &theme).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{slug}.toml"));
    let mut out = format!("Imported “{}” as {slug}: {}\n", theme.name, tidy_path(&path));
    for w in warnings {
        out.push_str(&format!("  note: {w}\n"));
    }
    out.push_str(&format!("Use it:  termpaper --theme {slug}\n"));
    Ok(out)
}

/// `theme check FILE`: a report for theme authors. Err on a file that
/// will not load.
pub fn check(text: &str) -> Result<String, String> {
    let (t, warnings) = if text.trim_start().starts_with("tp1:") { Theme::from_code(text.trim())? } else { Theme::from_toml(text)? };
    let code = t.to_code();
    let mut out = format!("ok: “{}”\n", t.name);
    for w in &warnings {
        out.push_str(&format!("  warning: {w}\n"));
    }
    for (k, v) in describe_look(&t.look) {
        out.push_str(&format!("  {k:<10}{v}\n"));
    }
    if let Some(s) = &t.scene {
        let v = s.variant.as_deref().map(|v| format!(" ({v})")).unwrap_or_default();
        out.push_str(&format!("  {:<10}{}{v}\n", "made for", s.name));
    }
    out.push_str(&format!("  share code: {} characters\n", code.len()));
    Ok(out)
}

/// `theme new NAME`: a theme file of yours from `look` (the look you have,
/// or another theme's), ready to edit by hand. Returns its slug.
pub fn new_theme(
    store: &mut Store,
    dir: &Path,
    name: &str,
    author: &str,
    description: &str,
    look: Look,
    scene: Option<crate::theme::SceneHint>,
) -> Result<String, String> {
    let mut t = Theme {
        name: name.to_string(),
        author: author.to_string(),
        description: description.to_string(),
        tags: vec!["custom".into()],
        look,
        scene,
        ..Theme::default()
    };
    t.validate()?;
    store.save_new_in(dir, &t).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("termpaper-theme-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn list_shows_every_theme_and_the_way_in() {
        let store = Store::load_from(None);
        let text = list(&store, false, None, None);
        for e in &store.entries {
            assert!(text.contains(&e.slug), "{}", e.slug);
        }
        assert!(text.contains("Terminal palettes") && text.contains("none yet"));
        assert!(!text.contains('\x1b'), "no colour when asked for none");
        assert!(list(&store, true, None, Some(100)).contains("\x1b[38;2;"));
        // cut to the terminal, whole when piped
        let narrow = list(&store, false, None, Some(60));
        assert!(narrow.lines().all(|l| l.chars().count() <= 80), "{narrow}");
        assert!(text.contains("the scene exactly as it is drawn."));
    }

    #[test]
    fn json_lists_round_trip_through_their_codes() {
        let store = Store::load_from(None);
        let v: serde_json::Value = serde_json::from_str(&list_json(&store)).unwrap();
        let items = v.as_array().unwrap();
        assert_eq!(items.len(), store.entries.len());
        for it in items {
            let (t, _) = Theme::from_code(it["code"].as_str().unwrap()).unwrap();
            let e = store.get(it["slug"].as_str().unwrap()).unwrap();
            assert_eq!(t.look, e.theme.look, "{}", e.slug);
        }
    }

    #[test]
    fn show_describes_and_suggests() {
        let store = Store::load_from(None);
        let s = show(&store, "Teal & Orange", false).unwrap();
        assert!(s.contains("built in · cinematic"));
        assert!(s.contains("contrast 1.12"));
        assert!(s.contains("shadows 190° 55%"));
        assert!(s.contains("vignette 50%"));
        assert!(s.contains("termpaper hongkong --variant night --theme teal-and-orange"));
        assert!(s.contains("tp1:"));
        let gb = show(&store, "game-boy", false).unwrap();
        assert!(gb.contains("snap"), "{gb}");
        let err = show(&store, "tokyo", false).unwrap_err();
        assert!(err.contains("tokyo-night"), "{err}");
        assert!(show(&store, "zzzz", false).unwrap_err().contains("theme list"));
    }

    #[test]
    fn export_import_and_check_round_trip() {
        let dir = tmp("import");
        let mut store = Store::load_from(Some(&dir));
        let toml = export(&store, "nord", false).unwrap();
        let code = export(&store, "nord", true).unwrap();
        assert!(check(&toml).unwrap().starts_with("ok: “Nord”"));
        assert!(check(code.trim()).unwrap().contains("share code"));
        // from a code, renamed
        let (t, w) = parse_source(code.trim(), || unreachable!()).unwrap();
        let out = import(&mut store, &dir, t, &w, Some("My Nord")).unwrap();
        assert!(out.contains("as my-nord"), "{out}");
        let back = Store::load_from(Some(&dir));
        let mine = back.get("my-nord").unwrap();
        assert_eq!(mine.source, Source::User);
        assert_eq!(mine.theme.look, store.get("nord").unwrap().theme.look);
        // from stdin
        let (t, _) = parse_source("-", || Ok(toml.clone())).unwrap();
        assert_eq!(t.name, "Nord");
        // a file that is not a theme
        assert!(check("name = 3").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_flags_unknown_scenes_and_variants() {
        let text = "format = 1\nname = \"X\"\n[scene]\nname = \"nowhere\"\n";
        assert!(check(text).unwrap().contains("warning: scene `nowhere` is not in this termpaper"));
        let text = "format = 1\nname = \"X\"\n[scene]\nname = \"hongkong\"\nvariant = \"noon\"\n";
        let report = check(text).unwrap();
        assert!(report.contains("warning: scene `hongkong` has no variant `noon`"), "{report}");
        assert!(report.contains("made for  hongkong\n"), "the scene itself still counts: {report}");
    }

    #[test]
    fn new_theme_writes_an_editable_file() {
        let dir = tmp("new");
        let mut store = Store::load_from(Some(&dir));
        let from = store.get("sepia").unwrap().theme.look.clone();
        let slug = new_theme(&mut store, &dir, "Old photo", "me", "Started from Sepia.", from.clone(), None).unwrap();
        assert_eq!(slug, "old-photo");
        let text = std::fs::read_to_string(dir.join("old-photo.toml")).unwrap();
        let (t, w) = Theme::from_toml(&text).unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!((t.name.as_str(), t.author.as_str(), t.description.as_str()), ("Old photo", "me", "Started from Sepia."));
        assert_eq!(t.look, from);
        // the same name again gets its own file
        assert_eq!(new_theme(&mut store, &dir, "Old photo", "me", "", Look::default(), None).unwrap(), "old-photo-2");
        assert!(new_theme(&mut store, &dir, "   ", "me", "", Look::default(), None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
