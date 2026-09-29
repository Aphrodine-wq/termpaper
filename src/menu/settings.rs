//! The settings pages as data. Each row is a [`Setting`]; every value is
//! read ([`value`]) and changed ([`step`]) through one match on its
//! [`SettingId`], so rows can be reordered or moved between pages freely.
//!
//! Stepping rules: ordered scales (dim, speed, fps, quality…) clamp at their
//! ends; unordered choices (theme, pixels, renderer…) wrap; toggles flip on
//! either arrow.

use super::{fps_step, speed_step, Effect, MenuCtx, Page};
use crate::color_wheel;
use crate::config::CycleScope;
use crate::engine::Renderer;
use crate::link::{self, GROUP_PRESETS};
use crate::scene::{self, Detail};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingId {
    Theme,
    ColorGrade,
    Filters,
    Dim,
    TextScale,
    Speed,
    Cycle,
    CycleScope,
    Fade,
    Quality,
    Pixels,
    Fps,
    Smooth,
    Clock,
    Renderer,
    Link,
    Group,
    Wall,
    Align,
    Instances,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// ←/→ step through values; Enter steps forward
    Choice,
    /// on/off: ←, → and Enter all flip it
    Toggle,
    /// Enter or → opens an overlay or sub-page
    Open,
    /// read-only
    Info,
}

pub struct Setting {
    pub id: SettingId,
    pub label: &'static str,
    /// one line shown under the page while the row has focus
    pub help: &'static str,
    pub kind: Kind,
}

const fn row(id: SettingId, label: &'static str, kind: Kind, help: &'static str) -> Setting {
    Setting {
        id,
        label,
        help,
        kind,
    }
}

use Kind::{Choice, Info, Open, Toggle};
use SettingId as S;

#[rustfmt::skip]
pub const LOOK: &[Setting] = &[
    row(S::Theme, "Theme", Choice, "Palette for the scene on screen; remembered per scene."),
    row(S::ColorGrade, "Color grade…", Open, "Hue, saturation and contrast on the colour wheel (also c)."),
    row(S::Filters, "Filters…", Open, "Post-processing: stack filters in order, or start from a preset."),
    row(S::Dim, "Dim", Choice, "Overall brightness; lower is calmer behind your windows."),
    row(S::TextScale, "Text size", Choice, "Caption size for text scenes such as bump; auto fits the pane."),
];

#[rustfmt::skip]
pub const PLAYBACK: &[Setting] = &[
    row(S::Speed, "Speed", Choice, "Animation speed. Also , and . outside the menu."),
    row(S::Cycle, "Cycle", Choice, "Move on to another scene every few minutes; off stays put."),
    row(S::CycleScope, "Cycle through", Choice, "Where Cycle picks from: every scene, this category, or favourites."),
    row(S::Fade, "Fade", Choice, "Cross-fade length when the scene changes."),
];

#[rustfmt::skip]
pub const DISPLAY: &[Setting] = &[
    row(S::Quality, "Quality", Choice, "Particle and layer density; lower it on laptops. Also d."),
    row(S::Pixels, "Pixels", Choice, "Cell packing: half blocks, quadrants (sharper) or braille (finest)."),
    row(S::Fps, "FPS", Choice, "Frame-rate cap. Also [ and ] outside the menu."),
    row(S::Smooth, "Smooth", Choice, "Temporal smoothing: blends frames to soften flicker."),
    row(S::Clock, "Clock", Toggle, "A small HH:MM in the top-right corner."),
    row(S::Renderer, "Renderer", Choice, "auto uses the GPU when present; cpu never does. Applies now."),
];

#[rustfmt::skip]
pub const WALL: &[Setting] = &[
    row(S::Link, "Link", Toggle, "Sync scene and settings with the other termpaper panes in your group."),
    row(S::Group, "Group", Choice, "Panes only sync within a group. Needs Link on."),
    row(S::Wall, "Wall mode", Toggle, "auto spans linked panes into one picture; off keeps this pane whole."),
    row(S::Align, "Align monitors…", Open, "Line the picture up across monitors with gaps or bezels."),
    row(S::Instances, "Instances", Info, "Live termpaper panes in your group, listed below."),
];

/// The rows of a settings page (Scenes has none: it is the browser).
pub fn page(p: Page) -> &'static [Setting] {
    match p {
        Page::Scenes => &[],
        Page::Look => LOOK,
        Page::Playback => PLAYBACK,
        Page::Display => DISPLAY,
        Page::Wall => WALL,
    }
}

/// Auto-cycle intervals (seconds) the Cycle row steps through, after off.
pub const CYCLE_STEPS: &[f64] = &[60.0, 300.0, 900.0, 1800.0];

const TEXT_SCALES: [Option<u32>; 4] = [None, Some(1), Some(2), Some(3)];
const QUALITIES: [Detail; 3] = [Detail::Low, Detail::Medium, Detail::High];
const RENDERERS: [Renderer; 4] = [
    Renderer::Auto,
    Renderer::Gpu,
    Renderer::Cpu,
    Renderer::Shader,
];

pub fn renderer_name(r: Renderer) -> &'static str {
    match r {
        Renderer::Auto => "auto",
        Renderer::Gpu => "gpu",
        Renderer::Cpu => "cpu",
        Renderer::Shader => "shader (exp.)",
    }
}

fn cycle_label(c: Option<f64>) -> String {
    match c {
        None => "off".into(),
        Some(s) if s >= 60.0 && (s % 60.0).abs() < 1e-6 => format!("{:.0} min", s / 60.0),
        Some(s) => format!("{s:.0} s"),
    }
}

/// Speeds and scales without trailing zeros: 1×, 0.25×, 1.5×.
fn short(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn wrap(i: usize, n: usize, dir: i32) -> usize {
    (i as i64 + dir.signum() as i64).rem_euclid(n as i64) as usize
}

fn clamp_step(i: usize, n: usize, dir: i32) -> usize {
    (i as i64 + dir.signum() as i64).clamp(0, n as i64 - 1) as usize
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

/// Some(new) only when it differs — a step into a clamped end is a no-op.
fn changed(old: f32, new: f32) -> Option<f32> {
    ((old - new).abs() > 1e-4).then_some(new)
}

/// Previous/next link group preset, wrapping; a custom group name steps
/// onto the presets.
pub fn next_group(cur: &str, dir: i32) -> String {
    let cur = link::sanitize_group(cur);
    let n = GROUP_PRESETS.len();
    let i = match GROUP_PRESETS.iter().position(|&g| g == cur.as_str()) {
        Some(i) => wrap(i, n, dir),
        None if dir > 0 => 0,
        None => n - 1,
    };
    GROUP_PRESETS[i].to_string()
}

/// The next auto-cycle interval: off → 1 → 5 → 15 → 30 min, clamping at
/// both ends. A custom `--cycle` value steps to the neighbouring preset.
pub fn cycle_step(cur: Option<f64>, up: bool) -> Option<f64> {
    match (cur, up) {
        (None, true) => Some(CYCLE_STEPS[0]),
        (None, false) => None,
        (Some(c), true) => Some(
            CYCLE_STEPS
                .iter()
                .copied()
                .find(|&s| s > c + 1e-6)
                .unwrap_or(c),
        ),
        (Some(c), false) => CYCLE_STEPS.iter().rev().copied().find(|&s| s < c - 1e-6),
    }
}

/// Whether the row does anything right now (Group needs Link on).
pub fn enabled(id: SettingId, ctx: &MenuCtx) -> bool {
    match id {
        S::Group => ctx.link_enabled,
        _ => true,
    }
}

/// The row's value as shown.
pub fn value(id: SettingId, ctx: &MenuCtx) -> String {
    let on_off = |b: bool| if b { "on" } else { "off" }.to_string();
    match id {
        S::Theme => ctx
            .theme
            .clone()
            .or_else(|| scene::themes(ctx.scene_name).first().map(|t| t.to_string()))
            .unwrap_or_else(|| "—".into()),
        S::ColorGrade => {
            if ctx.hue_shift < 0.5
                && (ctx.saturation - 1.0).abs() < 0.02
                && (ctx.contrast - 1.0).abs() < 0.02
            {
                "neutral".into()
            } else {
                format!(
                    "{} · {} · {}",
                    color_wheel::format_hue(ctx.hue_shift),
                    color_wheel::format_sat(ctx.saturation),
                    color_wheel::format_contrast(ctx.contrast)
                )
            }
        }
        S::Filters => match ctx.filters.len() {
            0 => "none".into(),
            n => format!("{n} on"),
        },
        S::Dim => format!("{:.0}%", ctx.dim * 100.0),
        S::TextScale => ctx.text_scale.map_or("auto".into(), |t| format!("{t}×")),
        S::Speed => format!("{}×", short(ctx.speed)),
        S::Cycle => cycle_label(ctx.cycle),
        S::CycleScope => match ctx.cycle_scope {
            CycleScope::All => "all scenes".into(),
            CycleScope::Category => "this category".into(),
            CycleScope::Favorites => "favourites".into(),
        },
        S::Fade => format!("{} s", short(ctx.fade)),
        S::Quality => ctx.detail.name().into(),
        S::Pixels => ctx.pixels.name().into(),
        S::Fps => ctx.fps.to_string(),
        S::Smooth if ctx.smooth < 0.05 => "off".into(),
        S::Smooth => format!("{:.1}", ctx.smooth),
        S::Clock => on_off(ctx.clock),
        S::Renderer => renderer_name(ctx.renderer).into(),
        S::Link if ctx.link_enabled => "on".into(),
        S::Link => "off (solo)".into(),
        S::Group if ctx.link_enabled => ctx.link_group.clone(),
        S::Group => "—".into(),
        S::Wall => if ctx.wall_enabled { "auto" } else { "off" }.into(),
        S::Align => String::new(),
        S::Instances if !ctx.link_enabled => "solo".into(),
        S::Instances => format!("{} live", ctx.instances.len()),
    }
}

/// Step a row's value back (`dir < 0`) or forward. None when nothing
/// changes: a clamped end, a disabled row, or a row without a value.
pub fn step(id: SettingId, ctx: &MenuCtx, dir: i32) -> Option<Effect> {
    let up = dir > 0;
    let d = dir.signum() as f32;
    match id {
        S::Theme => {
            let themes = scene::themes(ctx.scene_name);
            if themes.is_empty() {
                return None;
            }
            let cur = ctx
                .theme
                .as_deref()
                .and_then(|t| themes.iter().position(|x| *x == t))
                .unwrap_or(0);
            Some(Effect::SetTheme(Some(
                themes[wrap(cur, themes.len(), dir)].to_string(),
            )))
        }
        S::Dim => changed(ctx.dim, round1((ctx.dim + 0.1 * d).clamp(0.2, 1.0))).map(Effect::SetDim),
        S::TextScale => {
            let i = TEXT_SCALES
                .iter()
                .position(|t| *t == ctx.text_scale)
                .unwrap_or(0);
            Some(Effect::SetTextScale(
                TEXT_SCALES[wrap(i, TEXT_SCALES.len(), dir)],
            ))
        }
        S::Speed => changed(ctx.speed, speed_step(ctx.speed, up)).map(Effect::SetSpeed),
        S::Cycle => {
            let next = cycle_step(ctx.cycle, up);
            (next != ctx.cycle).then_some(Effect::SetCycle(next))
        }
        S::CycleScope => {
            let all = CycleScope::ALL;
            let i = all.iter().position(|s| *s == ctx.cycle_scope).unwrap_or(0);
            Some(Effect::SetCycleScope(all[wrap(i, all.len(), dir)]))
        }
        S::Fade => changed(
            ctx.fade,
            crate::config::round2((ctx.fade + 0.05 * d).clamp(0.1, 1.0)),
        )
        .map(Effect::SetFade),
        S::Quality => {
            let i = QUALITIES.iter().position(|q| *q == ctx.detail).unwrap_or(1);
            let next = QUALITIES[clamp_step(i, QUALITIES.len(), dir)];
            (next != ctx.detail).then_some(Effect::SetDetail(next))
        }
        S::Pixels => Some(Effect::SetPixels(if up {
            ctx.pixels.next()
        } else {
            ctx.pixels.next().next()
        })),
        S::Fps => {
            let next = fps_step(ctx.fps, up);
            (next != ctx.fps).then_some(Effect::SetFps(next))
        }
        S::Smooth => changed(ctx.smooth, round1((ctx.smooth + 0.1 * d).clamp(0.0, 0.9)))
            .map(Effect::SetSmooth),
        S::Clock => Some(Effect::SetClock(!ctx.clock)),
        S::Renderer => {
            let i = RENDERERS
                .iter()
                .position(|r| *r == ctx.renderer)
                .unwrap_or(0);
            Some(Effect::SetRenderer(
                RENDERERS[wrap(i, RENDERERS.len(), dir)],
            ))
        }
        S::Link => Some(Effect::SetLink(!ctx.link_enabled)),
        S::Group => ctx
            .link_enabled
            .then(|| Effect::SetLinkGroup(next_group(&ctx.link_group, dir))),
        S::Wall => Some(Effect::SetWall(!ctx.wall_enabled)),
        S::ColorGrade | S::Filters | S::Align | S::Instances => None,
    }
}

// ── Filters sub-page ────────────────────────────────────────────────────

/// Filters by what they do, in sub-page order. Covers `filter::FILTER_CYCLE`
/// exactly (tested).
pub const FILTER_GROUPS: &[(&str, &[&str])] = &[
    (
        "Colour",
        &[
            "warm",
            "cool",
            "sepia",
            "noir",
            "duotone",
            "thermal",
            "invert",
            "hue",
            "spectrum",
            "gamma",
            "posterize",
        ],
    ),
    (
        "Texture",
        &["scanlines", "crt", "grain", "vignette", "bloom", "chroma"],
    ),
    (
        "Geometry",
        &["pixelate", "warp", "mirror", "edges", "sharpen"],
    ),
];

/// Named filter stacks, applied in this order.
pub const PRESETS: &[(&str, &[&str])] = &[
    ("Clean", &[]),
    ("Film", &["warm", "grain", "vignette"]),
    ("CRT", &["bloom", "crt"]),
    ("Dream", &["bloom", "chroma", "vignette"]),
];

/// Every filter in sub-page order.
pub fn filter_list() -> impl Iterator<Item = &'static str> {
    FILTER_GROUPS.iter().flat_map(|(_, f)| f.iter().copied())
}

/// Selectable rows on the Filters sub-page: the preset row, then filters.
pub fn filter_rows() -> usize {
    1 + filter_list().count()
}

/// The filter on sub-page row `row` (row 0 is the preset).
pub fn filter_at(row: usize) -> Option<&'static str> {
    row.checked_sub(1).and_then(|i| filter_list().nth(i))
}

/// The preset matching the stack exactly, if any.
pub fn preset_of(filters: &[String]) -> Option<usize> {
    PRESETS
        .iter()
        .position(|(_, p)| p.len() == filters.len() && p.iter().zip(filters).all(|(a, b)| a == b))
}

/// Step the preset row, or toggle a filter row.
pub fn filter_step(row: usize, ctx: &MenuCtx, dir: i32) -> Option<Effect> {
    if row == 0 {
        let n = PRESETS.len();
        let next = match preset_of(&ctx.filters) {
            Some(i) => wrap(i, n, dir),
            None if dir > 0 => 0,
            None => n - 1,
        };
        return Some(Effect::SetFilters(
            PRESETS[next].1.iter().map(|s| s.to_string()).collect(),
        ));
    }
    filter_at(row).map(|f| Effect::ToggleFilter(f.to_string()))
}

pub fn filter_help(name: &str) -> &'static str {
    match name {
        "warm" => "Warm amber cast.",
        "cool" => "Cool blue cast.",
        "sepia" => "Old-photo brown tone.",
        "noir" => "Black and white with deep contrast.",
        "duotone" => "Brightness mapped onto a black-to-accent ramp.",
        "thermal" => "False-colour heat map.",
        "invert" => "Photographic negative.",
        "hue" => "Turns every colour 120° round the wheel.",
        "spectrum" => "Colours drift round the wheel every ~12 s.",
        "gamma" => "Deeper midtones and shadows.",
        "posterize" => "Flattens colour into a few bands.",
        "scanlines" => "Darkens every other pixel row.",
        "crt" => "Scanlines, vignette and colour fringing.",
        "grain" => "Animated film grain.",
        "vignette" => "Darkens toward the edges.",
        "bloom" => "Bright areas bleed light.",
        "chroma" => "Colour fringing toward the edges.",
        "pixelate" => "Chunky 3×3 mosaic.",
        "warp" => "Rows ripple side to side.",
        "mirror" => "Flips the picture left to right.",
        "edges" => "Only outlines glow, on black.",
        "sharpen" => "Crisper detail.",
        _ => "",
    }
}
