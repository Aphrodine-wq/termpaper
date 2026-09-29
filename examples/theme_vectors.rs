//! Golden vectors for ports of the theme format and the look (the
//! website's TypeScript): every built-in theme, a few edge cases, a fixed
//! set of colours through the look's per-pixel stages (grade, transform,
//! snap: `studio::graded`), and each theme's share code.
//!
//! ```text
//! cargo run --example theme_vectors > theme_vectors.json
//! ```
use termpaper::look::{Look, PaletteMode, Rgb, Tone};
use termpaper::theme::{Store, Theme};

fn inputs() -> Vec<(u8, u8, u8)> {
    let mut v = Vec::new();
    for r in [0u8, 128, 255] {
        for g in [0u8, 128, 255] {
            for b in [0u8, 128, 255] {
                v.push((r, g, b));
            }
        }
    }
    for k in [16u8, 48, 90, 160, 210, 240] {
        v.push((k, k, k));
    }
    v.extend(termpaper::studio::REFS);
    v.extend([(200, 60, 40), (40, 200, 90), (60, 90, 220), (250, 200, 30), (140, 40, 160), (20, 30, 40)]);
    v
}

fn edge_cases() -> Vec<(String, Look)> {
    let mut v = Vec::new();
    let mut l = Look::default();
    l.grade.hue = 200.0;
    l.grade.saturation = 1.8;
    l.grade.contrast = 1.6;
    v.push(("hue, saturation and contrast".to_string(), l));
    let mut l = Look::default();
    l.grade.saturation = 0.4;
    l.grade.contrast = 0.7;
    v.push(("no hue, low saturation".to_string(), l));
    let mut l = Look::default();
    l.grade.exposure = 1.3;
    l.grade.temperature = -0.8;
    l.grade.tint = 0.6;
    l.grade.gamma = 0.7;
    v.push(("exposure, white balance, gamma".to_string(), l));
    let mut l = Look::default();
    l.grade.vibrance = -0.7;
    l.grade.fade = 0.3;
    l.grade.shadows = Tone { hue: 250.0, amount: 1.0 };
    l.grade.midtones = Tone { hue: 90.0, amount: 0.6 };
    l.grade.highlights = Tone { hue: 20.0, amount: 0.9 };
    l.grade.balance = -0.5;
    v.push(("vibrance, fade, tone wheels".to_string(), l));
    let pal = vec![Rgb(10, 10, 40), Rgb(200, 30, 90), Rgb(250, 240, 200)];
    for mode in [PaletteMode::Map, PaletteMode::Tint, PaletteMode::Snap] {
        let mut l = Look::default();
        l.palette.mode = mode;
        l.palette.colors = pal.clone();
        l.palette.strength = 0.7;
        v.push((format!("palette {}", mode.name()), l));
    }
    v
}

fn main() {
    let colors = inputs();
    let store = Store::load_from(None);
    let mut looks = Vec::new();
    for e in &store.entries {
        looks.push((e.slug.clone(), e.theme.look.clone()));
    }
    looks.extend(edge_cases());
    let vectors: Vec<serde_json::Value> = looks
        .iter()
        .map(|(name, look)| {
            let out: Vec<[u8; 3]> = colors
                .iter()
                .map(|c| {
                    let o = termpaper::studio::graded(*c, look);
                    [o.0, o.1, o.2]
                })
                .collect();
            serde_json::json!({ "name": name, "look": look, "out": out })
        })
        .collect();
    let codes: Vec<serde_json::Value> = store
        .entries
        .iter()
        .map(|e| serde_json::json!({ "slug": e.slug, "code": e.theme.to_code(), "theme": e.theme }))
        .collect();
    let bad = ["tp1:", "tp1:!!!", "tp2:abc", "hello"];
    let doc = serde_json::json!({
        "format": termpaper::theme::FORMAT,
        "inputs": colors.iter().map(|c| [c.0, c.1, c.2]).collect::<Vec<_>>(),
        "looks": vectors,
        "codes": codes,
        "bad_codes": bad,
        "roundtrip": Theme::from_code(&store.entries[1].theme.to_code()).map(|(t, _)| t.name).ok(),
    });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap());
}
