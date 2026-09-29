//! Studio shader-scene headers: parsing and validation.
//!
//! Every scene in `src/gpu/scenes/*.wgsl` opens with a block of `//!` lines:
//!
//! ```text
//! //! name: bigsur
//! //! title: Big Sur Coast
//! //! category: coast
//! //! tags: ocean, cliffs, fog, sunset
//! //! desc: Pacific swells breaking under the Big Sur cliffs as marine fog rolls in
//! //! themes: sunset, fog, noon, moonlight
//! //! uses: camera, sky, water, fog
//! //! cost: heavy
//! //! fallback: ocean
//! //! credits: original
//! ```
//!
//! This file is std-only on purpose: `build.rs` includes it with `#[path]` to
//! generate the registry, and the runtime (hot reload) and the test suite use
//! the same parser, so a header that builds is a header that loads.

/// Library modules scenes can pull in with `uses:`, each with the modules it
/// needs. Order here is the order they are concatenated in, so a module only
/// ever calls modules listed above it.
pub const MODULES: &[(&str, &[&str])] = &[
    ("core", &[]),
    ("hash", &[]),
    ("color", &[]),
    ("tonemap", &[]),
    ("dither", &[]),
    ("noise", &["hash"]),
    ("camera", &[]),
    ("sdf", &[]),
    ("raymarch", &["sdf"]),
    ("sky", &["noise"]),
    ("stars", &["noise"]),
    ("clouds", &["noise"]),
    ("fog", &["noise"]),
    ("water", &["noise"]),
    ("wet", &["noise"]),
    ("rain", &["noise"]),
    ("snow", &["noise"]),
    ("bokeh", &["hash"]),
    ("light", &["hash"]),
    ("fire", &["noise"]),
    ("l2d", &["noise"]),
];

/// Always composed, whatever a scene lists.
pub const ALWAYS: &[&str] = &["core", "hash", "color", "tonemap", "dither"];

/// Scene categories, in browser order.
pub const CATEGORIES: &[&str] = &["coast", "wilds", "weather", "city", "cozy", "space"];

pub const COSTS: &[&str] = &["light", "medium", "heavy"];

/// `tonemap:` values. The index is the `TM_MODE` constant the shader sees.
pub const TONEMAPS: &[&str] = &["agx", "punchy", "aces", "neutral"];

pub const MAX_DESC: usize = 90;
pub const MAX_TITLE: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct Meta {
    pub name: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub desc: String,
    pub themes: Vec<String>,
    /// Resolved: every module the scene needs (always-on modules included),
    /// dependencies added, in `MODULES` order.
    pub uses: Vec<&'static str>,
    pub cost: String,
    pub tonemap: String,
    pub exposure: f32,
    pub ss_manual: bool,
    pub fallback: String,
    pub credits: String,
}

impl Meta {
    pub fn category_rank(&self) -> usize {
        CATEGORIES
            .iter()
            .position(|c| *c == self.category)
            .unwrap_or(CATEGORIES.len())
    }
    pub fn tonemap_mode(&self) -> u32 {
        TONEMAPS.iter().position(|t| *t == self.tonemap).unwrap_or(0) as u32
    }
}

fn list(v: &str) -> Vec<String> {
    v.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Expand `uses` with always-on modules and dependencies, in canonical order.
pub fn resolve_uses(uses: &[String]) -> Result<Vec<&'static str>, String> {
    let mut want: Vec<&'static str> = Vec::new();
    fn add(name: &str, want: &mut Vec<&'static str>, depth: usize) -> Result<(), String> {
        if depth > 16 {
            return Err(format!("module dependency cycle at '{name}'"));
        }
        let Some((m, deps)) = MODULES.iter().find(|(m, _)| *m == name) else {
            return Err(format!(
                "unknown module '{name}' in uses (known: {})",
                MODULES.iter().map(|m| m.0).collect::<Vec<_>>().join(", ")
            ));
        };
        for d in deps.iter() {
            add(d, want, depth + 1)?;
        }
        if !want.contains(m) {
            want.push(m);
        }
        Ok(())
    }
    for m in ALWAYS {
        add(m, &mut want, 0)?;
    }
    for u in uses {
        add(u, &mut want, 0)?;
    }
    let order = |m: &&str| MODULES.iter().position(|(n, _)| n == m).unwrap_or(usize::MAX);
    want.sort_by_key(order);
    Ok(want)
}

/// Parse a scene header. `stem` is the file name without `.wgsl`; the header's
/// `name` must match it. Errors carry the 1-based line they refer to.
pub fn parse(text: &str, stem: &str) -> Result<Meta, (usize, String)> {
    let mut fields: Vec<(usize, String, String)> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("//!") else {
            if line.trim().is_empty() && fields.is_empty() {
                continue;
            }
            break;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        let Some((k, v)) = rest.split_once(':') else {
            return Err((i + 1, format!("header line is not 'key: value': {rest}")));
        };
        let k = k.trim().to_ascii_lowercase();
        if fields.iter().any(|(_, f, _)| *f == k) {
            return Err((i + 1, format!("duplicate header key '{k}'")));
        }
        fields.push((i + 1, k, v.trim().to_string()));
    }
    let get = |k: &str| fields.iter().find(|(_, f, _)| f == k).map(|(l, _, v)| (*l, v.clone()));
    let need = |k: &str| get(k).ok_or((1usize, format!("missing required header '{k}'")));

    const KNOWN: &[&str] = &[
        "name", "title", "category", "tags", "desc", "themes", "uses", "cost", "tonemap",
        "exposure", "ss", "fallback", "credits",
    ];
    if let Some((l, k, _)) = fields.iter().find(|(_, k, _)| !KNOWN.contains(&k.as_str())) {
        return Err((*l, format!("unknown header key '{k}'")));
    }

    let (l, name) = need("name")?;
    if name != stem {
        return Err((l, format!("name '{name}' must match the file name '{stem}'")));
    }
    if name.is_empty()
        || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err((l, format!("name '{name}' must be lowercase ascii, digits or '-'")));
    }
    let (l, title) = need("title")?;
    if title.chars().count() > MAX_TITLE {
        return Err((l, format!("title is longer than {MAX_TITLE} characters")));
    }
    let (l, category) = need("category")?;
    if !CATEGORIES.contains(&category.as_str()) {
        return Err((l, format!("category '{category}' is not one of {}", CATEGORIES.join("|"))));
    }
    let (l, desc) = need("desc")?;
    if desc.chars().count() > MAX_DESC {
        return Err((l, format!("desc is longer than {MAX_DESC} characters")));
    }
    let (l, themes) = need("themes")?;
    let themes = list(&themes);
    if themes.len() < 2 || themes.len() > 5 {
        return Err((l, "themes must list 2 to 5 names (first is the default)".into()));
    }
    for (i, t) in themes.iter().enumerate() {
        if themes[..i].contains(t) {
            return Err((l, format!("theme '{t}' is listed twice")));
        }
    }
    let (_, fallback) = need("fallback")?;
    let tags = get("tags").map(|(_, v)| list(&v)).unwrap_or_default();
    let uses_raw = get("uses").map(|(_, v)| list(&v)).unwrap_or_default();
    let uses = resolve_uses(&uses_raw).map_err(|e| (get("uses").map(|u| u.0).unwrap_or(1), e))?;
    let cost = get("cost").map(|(_, v)| v).unwrap_or_else(|| "medium".into());
    if !COSTS.contains(&cost.as_str()) {
        return Err((get("cost").unwrap().0, format!("cost must be one of {}", COSTS.join("|"))));
    }
    let tonemap = get("tonemap").map(|(_, v)| v).unwrap_or_else(|| "agx".into());
    if !TONEMAPS.contains(&tonemap.as_str()) {
        return Err((
            get("tonemap").unwrap().0,
            format!("tonemap must be one of {}", TONEMAPS.join("|")),
        ));
    }
    let exposure = match get("exposure") {
        Some((l, v)) => v
            .parse::<f32>()
            .ok()
            .filter(|e| e.is_finite() && e.abs() <= 8.0)
            .ok_or((l, "exposure must be a number of stops in [-8, 8]".to_string()))?,
        None => 0.0,
    };
    let ss_manual = match get("ss") {
        Some((_, v)) if v == "manual" => true,
        Some((_, v)) if v == "auto" => false,
        Some((l, _)) => return Err((l, "ss must be auto or manual".into())),
        None => false,
    };
    let credits = get("credits").map(|(_, v)| v).unwrap_or_else(|| "original".into());
    Ok(Meta {
        name,
        title,
        category,
        tags,
        desc,
        themes,
        uses,
        cost,
        tonemap,
        exposure,
        ss_manual,
        fallback,
        credits,
    })
}

/// Checks across the whole set: unique names.
pub fn validate_set(metas: &[Meta]) -> Result<(), String> {
    for (i, m) in metas.iter().enumerate() {
        if metas[..i].iter().any(|o| o.name == m.name) {
            return Err(format!("two shader scenes are named '{}'", m.name));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "//! name: demo\n//! title: Demo\n//! category: coast\n//! desc: a demo\n//! themes: a, b\n//! uses: raymarch\n//! fallback: ocean\n\nfn scene() {}\n";

    #[test]
    fn parses_and_resolves_dependencies() {
        let m = parse(GOOD, "demo").unwrap();
        assert_eq!(m.themes, vec!["a", "b"]);
        assert_eq!(m.cost, "medium");
        // raymarch pulls in sdf; always-on modules come first
        assert_eq!(m.uses, vec!["core", "hash", "color", "tonemap", "dither", "sdf", "raymarch"]);
    }

    #[test]
    fn rejects_bad_headers() {
        assert!(parse(GOOD, "other").is_err());
        assert!(parse(&GOOD.replace("coast", "beach"), "demo").is_err());
        assert!(parse(&GOOD.replace("a, b", "a"), "demo").is_err());
        assert!(parse(&GOOD.replace("raymarch", "nope"), "demo").is_err());
        assert!(parse(&GOOD.replace("//! fallback: ocean\n", ""), "demo").is_err());
        assert!(parse(&format!("//! bogus: 1\n{GOOD}"), "demo").is_err());
    }
}
