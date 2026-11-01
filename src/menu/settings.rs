//! The settings pages as data. Each row is a [`Setting`]; every value is
//! read ([`value`]) and changed ([`step`]) through one match on its
//! [`SettingId`], so rows can be reordered or moved between pages freely.
//!
//! Stepping rules: ordered scales (dim, speed, fps, quality…) clamp at their
//! ends; unordered choices (variant, pixels, renderer…) wrap; toggles flip on
//! either arrow. Sliders are numeric rows described by a [`Num`]: one spec
//! drives their stepping, their text and the bar the view draws.

use super::{fps_step, speed_step, Effect, MenuCtx, Page};
use crate::studio as color_wheel;
use crate::config::CycleScope;
use crate::engine::Renderer;
use crate::link::{self, GROUP_PRESETS};
use crate::look::{Effects, Look, PaletteMode, Rgb};
use crate::scene::{self, Detail};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingId {
    // Look
    Theme,
    Variant,
    ColorGrade,
    Exposure,
    Contrast,
    Saturation,
    Vibrance,
    Temperature,
    Tint,
    Hue,
    Matte,
    PaletteMode,
    Palette,
    PaletteStrength,
    Filters,
    Glow,
    Vignette,
    Grain,
    Letterbox,
    Dim,
    TextScale,
    ResetLook,
    // Tune: the scene's own sliders
    Param0,
    Param1,
    Param2,
    Param3,
    TuneRandom,
    TuneReset,
    // Playback
    Speed,
    Cycle,
    CycleScope,
    Order,
    TimeOfDay,
    OnLaunch,
    Transition,
    Fade,
    ResetPlayback,
    // Display
    Fullscreen,
    Colors,
    Pixels,
    Quality,
    Smooth,
    CellShape,
    Fps,
    AdaptFps,
    Unfocused,
    Battery,
    Bandwidth,
    Renderer,
    Gpu,
    StudioBudget,
    StudioFps,
    DepthStrength,
    CameraMotion,
    PointerParallax,
    Clock,
    ClockSize,
    ClockFormat,
    ClockCorner,
    ClockColor,
    ClockLevel,
    Caption,
    CaptionSecs,
    BatteryMeter,
    Hud,
    Mouse,
    MenuAccent,
    MenuSide,
    MenuFrost,
    PauseBehind,
    Night,
    NightFrom,
    NightTo,
    NightLevel,
    TerminalCheck,
    ResetDisplay,
    // Wall
    Link,
    Group,
    NewGroup,
    SyncLook,
    Layout,
    WallView,
    FocusReaction,
    Grid,
    GridPos,
    Padding,
    Placement,
    Bezel,
    PauseWall,
    WallUp,
    WallDown,
    Align,
    Instances,
    // Keys: one row per action in `config::ACTIONS`, then a reset
    Key(u8),
    ResetKeys,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// ←/→ step through values; Enter steps forward
    Choice,
    /// on/off: ←, → and Enter all flip it
    Toggle,
    /// a number on a bar (see [`num`]): ←/→ step it
    Slider,
    /// Enter or → opens an overlay or sub-page
    Open,
    /// Enter does something once (reset…)
    Action,
    /// read-only
    Info,
}

#[derive(Clone, Copy)]
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

use Kind::{Action, Choice, Info, Open, Slider, Toggle};
use SettingId as S;

#[rustfmt::skip]
pub const LOOK: &[Setting] = &[
    row(S::Theme, "Theme", Choice, "A whole look at once: grade, palette and effects. All of them, with previews, on the Themes page."),
    row(S::Variant, "Variant", Choice, "This scene's time of day or weather; remembered per scene (t in Scenes)."),
    row(S::ColorGrade, "Colour studio…", Open, "The colour wheel, tone wheels and palette in one place (also c)."),
    row(S::Exposure, "Exposure", Slider, "Brighter or darker, in photographic stops."),
    row(S::Contrast, "Contrast", Slider, "Punchier or flatter."),
    row(S::Saturation, "Saturation", Slider, "Stronger or softer colour; 0% is black and white."),
    row(S::Vibrance, "Vibrance", Slider, "Lifts muted colours more than vivid ones: richer without neon."),
    row(S::Temperature, "Temperature", Slider, "Cooler blue light or warmer amber light."),
    row(S::Tint, "Tint", Slider, "Toward green or toward magenta."),
    row(S::Hue, "Hue shift", Slider, "Turns every colour round the colour wheel."),
    row(S::Matte, "Matte", Slider, "Lifts the blacks toward grey: a faded-film look."),
    row(S::PaletteMode, "Palette", Choice, "Recolour with a palette: map it by brightness, tint with it, or snap to it."),
    row(S::Palette, "Palette colours…", Open, "Choose a palette or build your own (in the colour studio)."),
    row(S::PaletteStrength, "Palette strength", Slider, "How far toward the palette's colours."),
    row(S::Filters, "Effects…", Open, "Stack effects with their strengths, or start from a preset."),
    row(S::Glow, "Glow", Slider, "Bright areas bleed light (bloom)."),
    row(S::Vignette, "Vignette", Slider, "Darkens toward the edges."),
    row(S::Grain, "Film grain", Slider, "Fine animated grain."),
    row(S::Letterbox, "Letterbox", Slider, "Cinema bars top and bottom."),
    row(S::Dim, "Brightness", Slider, "Overall brightness; lower is calmer behind your windows."),
    row(S::TextScale, "Text size", Choice, "Caption size for text scenes such as bump; auto fits the pane."),
    row(S::ResetLook, "Reset look", Action, "Back to neutral: no theme, grade, palette or effects."),
];

#[rustfmt::skip]
pub const TUNE: &[Setting] = &[
    row(S::Param0, "Slider 1", Slider, "This scene's own control: the label is the scene's. The middle is the scene as its author made it."),
    row(S::Param1, "Slider 2", Slider, "This scene's own control: the label is the scene's. The middle is the scene as its author made it."),
    row(S::Param2, "Slider 3", Slider, "This scene's own control: the label is the scene's. The middle is the scene as its author made it."),
    row(S::Param3, "Slider 4", Slider, "This scene's own control: the label is the scene's. The middle is the scene as its author made it."),
    row(S::TuneRandom, "Randomize", Action, "Roll every slider of this scene at random. u undoes it."),
    row(S::TuneReset, "Reset", Action, "Every slider of this scene back to the middle."),
];

/// The Keys page: an `Open` row per action (Enter, then press the key), and
/// a reset. Built from `config::ACTIONS`, so a new action is a new row.
pub const KEYS: [Setting; crate::config::ACTIONS.len() + 1] = {
    let mut rows = [row(S::ResetKeys, "Reset keys", Action, "Every action back to its default key."); crate::config::ACTIONS.len() + 1];
    let mut i = 0;
    while i < crate::config::ACTIONS.len() {
        rows[i] = row(S::Key(i as u8), "", Open, "Enter, then press the key you want for this. A key another action holds swaps over.");
        i += 1;
    }
    rows
};

#[rustfmt::skip]
pub const PLAYBACK: &[Setting] = &[
    row(S::Speed, "Speed", Choice, "Animation speed. Also , and . outside the menu."),
    row(S::Cycle, "Cycle", Choice, "Move on to another scene every so often; off stays put."),
    row(S::CycleScope, "Cycle through", Choice, "Where Cycle picks from: every scene, this category, favourites, the playlist (p in Scenes), Studio or Classic scenes, this scene's variants, or a surprise from a different category each time."),
    row(S::Order, "Order", Choice, "Take them in list order, or shuffled."),
    row(S::TimeOfDay, "Follow the clock", Toggle, "Switch to the scene's dawn, day, dusk or night variant as the day goes on, where it has them."),
    row(S::OnLaunch, "On launch", Choice, "What termpaper opens on: the scene you left, a random favourite, or any scene."),
    row(S::Transition, "Transition", Choice, "How one scene gives way to the next; random picks a different one each time."),
    row(S::Fade, "Length", Slider, "How long a scene change takes."),
    row(S::ResetPlayback, "Reset this page", Action, "Speed, cycling and transitions back to how termpaper starts. u undoes it."),
];

#[rustfmt::skip]
pub const DISPLAY: &[Setting] = &[
    row(S::Fullscreen, "Fullscreen", Toggle, "Fill the screen with the terminal window while termpaper runs; F toggles it. Put back when you quit."),
    row(S::Colors, "Colours", Choice, "24-bit colour, or 256 colours for terminals that cannot show more. Auto asks the terminal."),
    row(S::Pixels, "Pixels", Choice, "How each cell splits into pixels: half blocks, quadrants, braille, sextants, text shading, or solid blocks."),
    row(S::Quality, "Quality", Choice, "Particle and layer density; lower it on laptops. Also d."),
    row(S::Smooth, "Smooth", Slider, "Blends each frame into the last to soften flicker."),
    row(S::CellShape, "Cell shape", Choice, "A character cell's height over its width, for round circles and seamless walls. Auto asks the terminal."),
    row(S::Fps, "FPS", Choice, "Frame-rate cap. Also [ and ] outside the menu."),
    row(S::AdaptFps, "Adapt FPS", Toggle, "Lower the frame rate while the terminal cannot keep up, and raise it again when it can."),
    row(S::Unfocused, "When unfocused", Choice, "What to do while another window has the focus."),
    row(S::Battery, "On battery", Choice, "Save power on battery: at most 30 fps, and the integrated GPU when GPU is auto."),
    row(S::Bandwidth, "Output", Choice, "Balanced skips colour changes too small to see in Studio scenes; light skips more everywhere; full sends everything."),
    row(S::Renderer, "Renderer", Choice, "auto uses the GPU when present; cpu never does. Applies now."),
    row(S::Gpu, "GPU", Choice, "Which graphics chip draws Studio scenes. Auto takes the fast one, or the integrated one on battery."),
    row(S::StudioBudget, "Studio budget", Slider, "GPU time a Studio scene may take per frame before its detail steps down."),
    row(S::StudioFps, "Studio FPS", Choice, "Frame-rate cap while a Studio scene shows."),
    row(S::DepthStrength, "Depth strength", Slider, "How far depth scenes move their camera in response to motion and the desk."),
    row(S::CameraMotion, "Camera motion", Choice, "Still, a slow ambient drift, or a more cinematic camera path in depth scenes."),
    row(S::PointerParallax, "Pointer parallax", Choice, "On Hyprland, gently shift the depth-scene view as the pointer moves across the desk."),
    row(S::Clock, "Clock", Toggle, "A clock over the scene."),
    row(S::ClockSize, "Clock style", Choice, "Small text; large digits that frost the picture under them; large solid digits; or large outlines."),
    row(S::ClockFormat, "Clock shows", Choice, "24-hour, 12-hour, with seconds, or with the date."),
    row(S::ClockCorner, "Clock position", Choice, "Which corner the clock sits in, or the centre."),
    row(S::ClockColor, "Clock colour", Choice, "Quiet grey, white, warm, cool, green or amber."),
    row(S::ClockLevel, "Clock brightness", Slider, "How strongly the clock shows over the picture."),
    row(S::Caption, "Scene name", Toggle, "Show the scene's name for a few seconds when it changes."),
    row(S::CaptionSecs, "Name stays", Slider, "How long the scene's name stays up after a switch."),
    row(S::BatteryMeter, "Battery", Toggle, "The battery's charge in the corner opposite the clock (laptops)."),
    row(S::Hud, "Performance", Toggle, "Frame rate, frame time and output size in the top-left corner."),
    row(S::Mouse, "Mouse", Toggle, "Click, drag and scroll in the menu. Off leaves the mouse to the terminal (text selection)."),
    row(S::MenuAccent, "Menu colour", Choice, "The accent the menu and the colour studio are drawn in."),
    row(S::MenuSide, "Menu side", Choice, "Which side of the screen the drawer opens on."),
    row(S::MenuFrost, "Menu glass", Toggle, "The drawer tints the live picture under it; off blanks it for a plainer look."),
    row(S::PauseBehind, "Pause behind apps", Toggle, "Hold the picture while another window is fullscreen (Hyprland): a wallpaper need not run under a game."),
    row(S::Night, "Night dimming", Toggle, "Dim the picture at night, on a schedule."),
    row(S::NightFrom, "From", Choice, "When night dimming starts."),
    row(S::NightTo, "Until", Choice, "When night dimming ends."),
    row(S::NightLevel, "Night brightness", Slider, "How bright the picture is at night."),
    row(S::TerminalCheck, "Terminal check…", Open, "See what this terminal can show, and set Colours and Pixels to match."),
    row(S::ResetDisplay, "Reset this page", Action, "Everything on this page back to how termpaper starts in this terminal. u undoes it."),
];

#[rustfmt::skip]
pub const WALL: &[Setting] = &[
    row(S::Link, "Link", Toggle, "Sync scene and settings with the other termpaper panes in your group."),
    row(S::Group, "Group", Choice, "Panes only sync within a group. Needs Link on."),
    row(S::NewGroup, "New group…", Open, "Name a group of your own; panes you start with --group NAME join it."),
    row(S::SyncLook, "Sync look", Toggle, "Take the group's theme, grade and effects. Off keeps this pane's own look."),
    row(S::Layout, "Layout", Choice, "auto spans linked panes into one picture from where their windows are (Hyprland); grid is one you set; off keeps this pane whole."),
    row(S::WallView, "Wall view", Choice, "One continuous picture, or a different view into the same 3D world on each monitor."),
    row(S::FocusReaction, "Focus reaction", Choice, "On Hyprland, depth scenes respond to a change of focused monitor."),
    row(S::Grid, "Grid", Choice, "Columns × rows of the grid layout: set the same on every pane."),
    row(S::GridPos, "This pane", Choice, "Which cell of the grid this pane shows."),
    row(S::Padding, "Padding", Slider, "The terminal's inner padding, so the picture lines up across window borders."),
    row(S::Placement, "Placement", Choice, "Where the terminal leaves its spare pixels when the window is not a whole number of cells."),
    row(S::Bezel, "Bezels", Slider, "Monitor frame width: the picture carries on behind the bezels (Hyprland)."),
    row(S::PauseWall, "Pause the wall", Action, "Freeze every pane in the group, or carry on. Also space."),
    row(S::WallUp, "Start wall", Action, "Open a termpaper on every monitor, in this group (Hyprland)."),
    row(S::WallDown, "Stop wall", Action, "Close the terminals Start wall opened."),
    row(S::Align, "Align monitors…", Open, "Line the picture up across monitors with gaps or bezels."),
    row(S::Instances, "Instances", Info, "Live termpaper panes in your group, listed below (★ leads)."),
];

/// The rows of a settings page (Scenes has none: it is the browser).
pub fn page(p: Page) -> &'static [Setting] {
    match p {
        Page::Scenes | Page::Themes => &[],
        Page::Look => LOOK,
        Page::Tune => TUNE,
        Page::Playback => PLAYBACK,
        Page::Display => DISPLAY,
        Page::Wall => WALL,
        Page::Keys => &KEYS,
    }
}

/// A row's label as shown: Tune rows carry the scene's own slider names,
/// Keys rows the action they bind.
pub fn label(s: &Setting, ctx: &MenuCtx) -> String {
    match s.id {
        S::Param0 | S::Param1 | S::Param2 | S::Param3 => {
            let i = param_index(s.id);
            ctx.param_labels.get(i).cloned().unwrap_or_else(|| "—".to_string())
        }
        S::Key(i) => crate::config::ACTIONS
            .get(i as usize)
            .map(|a| {
                let l = crate::config::action_label(a);
                let mut c = l.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => a.to_string(),
                }
            })
            .unwrap_or_default(),
        _ => s.label.to_string(),
    }
}

/// A row's help line as shown.
pub fn help(s: &Setting, ctx: &MenuCtx) -> &'static str {
    if matches!(s.id, S::Param0 | S::Param1 | S::Param2 | S::Param3 | S::TuneRandom | S::TuneReset) && ctx.param_labels.is_empty() {
        return "This scene has no sliders of its own. Studio scenes that declare params get them here.";
    }
    s.help
}

fn param_index(id: SettingId) -> usize {
    match id {
        S::Param0 => 0,
        S::Param1 => 1,
        S::Param2 => 2,
        _ => 3,
    }
}

/// Every slider of the scene set at random (the ones it has).
fn random_params(ctx: &MenuCtx) -> [f32; 4] {
    let mut p = [crate::scene::shader::PARAM_NEUTRAL; 4];
    for (i, slot) in p.iter_mut().enumerate() {
        if i < ctx.param_labels.len() {
            *slot = round_dec(rand::random_range(0.0f32..=1.0), 2);
        }
    }
    p
}

/// Section headings drawn above rows: (index of the first row, heading).
pub fn sections(p: Page) -> &'static [(usize, &'static str)] {
    match p {
        Page::Look => &[(0, "Presets"), (2, "Colour"), (11, "Palette"), (14, "Effects"), (19, "Light & text")],
        Page::Playback => &[(0, "Scenes"), (6, "Transitions")],
        Page::Display => &[(0, "Picture"), (6, "Speed & power"), (15, "On screen"), (25, "Menu"), (30, "Night"), (34, "Terminal")],
        Page::Wall => &[(0, "Linking"), (4, "Layout"), (10, "The wall")],
        Page::Tune => &[(0, "This scene")],
        Page::Keys => &[(0, "Actions")],
        _ => &[],
    }
}

/// Auto-cycle intervals (seconds) the Cycle row steps through, after off.
pub const CYCLE_STEPS: &[f64] = &[30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0];

/// Studio frame-rate caps the Studio FPS row steps through.
pub const STUDIO_FPS_STEPS: &[u32] = &[30, 45, 60, 90, 120];

/// Cell shapes (height over width) after auto.
pub const CELL_SHAPES: &[f32] = &[1.6, 1.8, 2.0, 2.2, 2.4];

const TEXT_SCALES: [Option<u32>; 4] = [None, Some(1), Some(2), Some(3)];
const QUALITIES: [Detail; 3] = [Detail::Low, Detail::Medium, Detail::High];
const RENDERERS: [Renderer; 4] = [
    Renderer::Auto,
    Renderer::Gpu,
    Renderer::Cpu,
    Renderer::Shader,
];

/// The palette a Palette row starts from when the look has none yet: the
/// Tokyo Night colours, darkest first.
pub const DEFAULT_PALETTE: [Rgb; 5] = [
    Rgb(0x1a, 0x1b, 0x26),
    Rgb(0x41, 0x48, 0x68),
    Rgb(0x7a, 0xa2, 0xf7),
    Rgb(0xbb, 0x9a, 0xf7),
    Rgb(0xc0, 0xca, 0xf5),
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
        Some(s) if s >= 3600.0 && (s % 3600.0).abs() < 1e-6 => format!("{:.0} h", s / 3600.0),
        Some(s) if s >= 60.0 && (s % 60.0).abs() < 1e-6 => format!("{:.0} min", s / 60.0),
        Some(s) => format!("{s:.0} s"),
    }
}

/// The next of `all` from `cur`, wrapping.
fn step_in<T: Copy + PartialEq>(all: &[T], cur: T, dir: i32) -> T {
    let i = all.iter().position(|v| *v == cur).unwrap_or(0);
    all[wrap(i, all.len(), dir)]
}

/// The Display prefs with one change.
fn with_display(ctx: &MenuCtx, f: impl FnOnce(&mut crate::prefs::DisplayPrefs)) -> Option<Effect> {
    let mut d = ctx.display.clone();
    f(&mut d);
    (d != ctx.display).then_some(Effect::SetDisplay(d))
}

/// The Wall prefs with one change.
fn with_wall(ctx: &MenuCtx, f: impl FnOnce(&mut crate::prefs::WallPrefs)) -> Option<Effect> {
    let mut w = ctx.wall.clone();
    f(&mut w);
    (w != ctx.wall).then_some(Effect::SetWallPrefs(w))
}

/// The Playback prefs with one change.
fn with_playback(ctx: &MenuCtx, f: impl FnOnce(&mut crate::prefs::PlaybackPrefs)) -> Option<Effect> {
    let mut p = ctx.playback.clone();
    f(&mut p);
    (p != ctx.playback).then_some(Effect::SetPlayback(p))
}

fn hour_label(h: u8) -> String {
    format!("{h:02}:00")
}

/// A key as the Keys page shows it.
pub fn pretty_key(k: &str) -> String {
    match k.to_lowercase().as_str() {
        "right" => "→".into(),
        "left" => "←".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "space" => "Space".into(),
        "esc" | "escape" => "Esc".into(),
        "tab" => "Tab".into(),
        "enter" | "return" => "Enter".into(),
        _ => k.to_string(),
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

/// Round to `places` decimals, so repeated steps store 0.9, not 0.90000004.
fn round_dec(v: f32, places: u32) -> f32 {
    let k = 10f32.powi(places as i32);
    (v * k).round() / k
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

// ── numbers ─────────────────────────────────────────────────────────────

/// A slider's range. `neutral` is the value that changes nothing: the bar
/// fills from it, so a bipolar control (temperature) grows either way from
/// the middle. `places` is how values are rounded after a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Num {
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub neutral: f32,
    pub places: u32,
    /// the value wraps round (hue) instead of clamping
    pub wraps: bool,
}

const fn n(min: f32, max: f32, step: f32, neutral: f32, places: u32) -> Num {
    Num {
        min,
        max,
        step,
        neutral,
        places,
        wraps: false,
    }
}

/// The range of a numeric row.
pub fn num(id: SettingId) -> Option<Num> {
    Some(match id {
        S::Exposure => n(-2.0, 2.0, 0.1, 0.0, 1),
        S::Contrast => n(0.5, 2.5, 0.05, 1.0, 2),
        S::Saturation => n(0.0, 2.5, 0.05, 1.0, 2),
        S::Vibrance | S::Temperature | S::Tint => n(-1.0, 1.0, 0.05, 0.0, 2),
        S::Hue => Num {
            wraps: true,
            ..n(0.0, 360.0, 5.0, 0.0, 0)
        },
        S::Matte => n(0.0, 0.5, 0.02, 0.0, 2),
        S::PaletteStrength => n(0.0, 1.0, 0.05, 0.0, 2),
        S::Glow | S::Vignette | S::Grain | S::Letterbox => n(0.0, 2.0, 0.1, 0.0, 1),
        S::Dim => n(0.2, 1.0, 0.1, 1.0, 1),
        S::Smooth => n(0.0, 0.9, 0.1, 0.0, 1),
        S::Fade => n(0.1, 1.0, 0.05, 0.1, 2),
        S::StudioBudget => n(1.0, 12.0, 0.5, crate::prefs::STUDIO_BUDGET_MS, 1),
        S::DepthStrength => n(0.0, 1.0, 0.05, crate::prefs::DEPTH_STRENGTH, 2),
        S::NightLevel => n(0.1, 1.0, 0.05, 1.0, 2),
        S::ClockLevel => n(0.2, 1.0, 0.1, crate::prefs::CLOCK_LEVEL, 1),
        S::CaptionSecs => n(1.0, 15.0, 1.0, crate::prefs::CAPTION_SECS, 0),
        S::Padding => n(0.0, 24.0, 1.0, 0.0, 0),
        S::Bezel => n(0.0, 30.0, 0.5, 0.0, 1),
        S::Param0 | S::Param1 | S::Param2 | S::Param3 => n(0.0, 1.0, 0.05, crate::scene::shader::PARAM_NEUTRAL, 2),
        _ => return None,
    })
}

/// The effect a quick slider (Glow, Vignette…) sets the strength of.
fn quick_effect(id: SettingId) -> Option<&'static str> {
    match id {
        S::Glow => Some("bloom"),
        S::Vignette => Some("vignette"),
        S::Grain => Some("grain"),
        S::Letterbox => Some("letterbox"),
        _ => None,
    }
}

/// A numeric row's current value.
pub fn num_value(id: SettingId, ctx: &MenuCtx) -> Option<f32> {
    let g = &ctx.look.grade;
    Some(match id {
        S::Exposure => g.exposure,
        S::Contrast => g.contrast,
        S::Saturation => g.saturation,
        S::Vibrance => g.vibrance,
        S::Temperature => g.temperature,
        S::Tint => g.tint,
        S::Hue => g.hue,
        S::Matte => g.fade,
        S::PaletteStrength => ctx.look.palette.strength,
        S::Dim => ctx.dim,
        S::Smooth => ctx.smooth,
        S::Fade => ctx.fade,
        S::StudioBudget => ctx.display.studio_budget_ms,
        S::DepthStrength => ctx.display.depth_strength,
        S::NightLevel => ctx.display.night_level,
        S::ClockLevel => ctx.display.clock_level,
        S::CaptionSecs => ctx.display.caption_secs,
        S::Padding => ctx.wall.pad,
        S::Bezel => ctx.wall.bezel_mm,
        S::Param0 | S::Param1 | S::Param2 | S::Param3 => ctx.params[param_index(id)],
        _ => {
            let fx = quick_effect(id)?;
            let e = &ctx.look.effects;
            if e.stack.iter().any(|s| s == fx) {
                e.amount(fx)
            } else {
                0.0
            }
        }
    })
}

/// A look with one quick effect at strength `v` (0 takes it off the stack).
fn with_effect(look: &Look, fx: &str, v: f32) -> Look {
    let mut l = look.clone();
    let on = l.effects.stack.iter().position(|s| s == fx);
    if v <= 1e-4 {
        if let Some(i) = on {
            l.effects.stack.remove(i);
        }
        l.effects.amounts.remove(fx);
    } else {
        if on.is_none() {
            l.effects.stack.push(fx.to_string());
        }
        l.effects.set_amount(fx, v);
    }
    l
}

/// The effect that sets a numeric row to `v`.
fn set_num(id: SettingId, ctx: &MenuCtx, v: f32) -> Option<Effect> {
    let mut look = ctx.look.clone();
    let g = &mut look.grade;
    match id {
        S::Exposure => g.exposure = v,
        S::Contrast => g.contrast = v,
        S::Saturation => g.saturation = v,
        S::Vibrance => g.vibrance = v,
        S::Temperature => g.temperature = v,
        S::Tint => g.tint = v,
        S::Hue => g.hue = v,
        S::Matte => g.fade = v,
        S::PaletteStrength => look.palette.strength = v,
        S::Dim => return Some(Effect::SetDim(v)),
        S::Smooth => return Some(Effect::SetSmooth(v)),
        S::Fade => return Some(Effect::SetFade(v)),
        S::StudioBudget => return with_display(ctx, |d| d.studio_budget_ms = v),
        S::DepthStrength => return with_display(ctx, |d| d.depth_strength = v),
        S::NightLevel => return with_display(ctx, |d| d.night_level = v),
        S::ClockLevel => return with_display(ctx, |d| d.clock_level = v),
        S::CaptionSecs => return with_display(ctx, |d| d.caption_secs = v),
        S::Padding => return with_wall(ctx, |w| w.pad = v),
        S::Bezel => return with_wall(ctx, |w| w.bezel_mm = v),
        S::Param0 | S::Param1 | S::Param2 | S::Param3 => {
            let mut p = ctx.params;
            p[param_index(id)] = v;
            return Some(Effect::SetParams(p));
        }
        _ => return quick_effect(id).map(|fx| Effect::SetLook(with_effect(&ctx.look, fx, v))),
    }
    Some(Effect::SetLook(look))
}

/// Where a slider's bar is: (value, neutral) as fractions of its range.
pub fn fraction(id: SettingId, ctx: &MenuCtx) -> Option<(f32, f32)> {
    let spec = num(id)?;
    let v = num_value(id, ctx)?;
    let span = (spec.max - spec.min).max(1e-6);
    let f = |x: f32| ((x - spec.min) / span).clamp(0.0, 1.0);
    Some((f(v), f(spec.neutral)))
}

/// Set a slider from a point on its bar (the mouse): `frac` 0 is the left
/// end, 1 the right; the value lands on the slider's step grid.
pub fn set_fraction(id: SettingId, ctx: &MenuCtx, frac: f32) -> Option<Effect> {
    let spec = num(id)?;
    if !enabled(id, ctx) {
        return None;
    }
    let raw = spec.min + frac.clamp(0.0, 1.0) * (spec.max - spec.min);
    let snapped = (raw / spec.step).round() * spec.step;
    let mut v = round_dec(snapped.clamp(spec.min, spec.max), spec.places);
    if spec.wraps {
        v = v.rem_euclid(spec.max);
    }
    changed(num_value(id, ctx)?, v).and_then(|v| set_num(id, ctx, v))
}

/// The value to set after stepping a slider once in `dir`.
fn step_num(id: SettingId, ctx: &MenuCtx, dir: i32) -> Option<Effect> {
    let spec = num(id)?;
    let cur = num_value(id, ctx)?;
    let raw = cur + spec.step * dir.signum() as f32;
    let next = if spec.wraps {
        round_dec(raw, spec.places).rem_euclid(spec.max)
    } else {
        round_dec(raw.clamp(spec.min, spec.max), spec.places)
    };
    changed(cur, next).and_then(|v| set_num(id, ctx, v))
}

fn signed(v: f32) -> String {
    if v.abs() < 5e-3 {
        "0".into()
    } else {
        format!("{:+.0}", v * 100.0)
    }
}

fn strength(v: f32) -> String {
    if v <= 1e-4 {
        "off".into()
    } else {
        format!("{:.0}%", v * 100.0)
    }
}

/// Whether the row does anything right now (Group needs Link on, the
/// palette strength a palette to be strong in).
pub fn enabled(id: SettingId, ctx: &MenuCtx) -> bool {
    match id {
        S::Group | S::NewGroup | S::SyncLook | S::PauseWall => ctx.link_enabled,
        S::Grid | S::GridPos => ctx.wall.mode == crate::prefs::WallMode::Grid,
        S::Bezel => ctx.hypr && ctx.wall.mode == crate::prefs::WallMode::Auto,
        S::WallUp | S::WallDown => ctx.hypr,
        S::PaletteStrength => matches!(ctx.look.palette.mode, PaletteMode::Map | PaletteMode::Tint),
        S::ClockSize | S::ClockFormat | S::ClockCorner | S::ClockColor | S::ClockLevel => ctx.clock,
        S::CaptionSecs => ctx.display.caption,
        S::PauseBehind | S::PointerParallax | S::FocusReaction => ctx.hypr,
        S::NightFrom | S::NightTo | S::NightLevel => ctx.display.night,
        S::Order => ctx.cycle.is_some(),
        S::Gpu | S::StudioBudget | S::StudioFps => cfg!(feature = "gpu"),
        S::Fullscreen => ctx.fullscreen_available,
        S::Param0 | S::Param1 | S::Param2 | S::Param3 => param_index(id) < ctx.param_labels.len(),
        S::TuneRandom | S::TuneReset => !ctx.param_labels.is_empty(),
        _ => true,
    }
}

/// The row's value as shown.
pub fn value(id: SettingId, ctx: &MenuCtx) -> String {
    let on_off = |b: bool| if b { "on" } else { "off" }.to_string();
    let g = &ctx.look.grade;
    match id {
        S::Theme => match ctx.active_theme.as_deref().and_then(|s| ctx.themes.iter().find(|t| t.slug == s)) {
            Some(t) if ctx.theme_modified => format!("{} · edited", t.name),
            Some(t) => t.name.clone(),
            None => "none".into(),
        },
        S::Variant => ctx
            .theme
            .clone()
            .or_else(|| scene::themes(ctx.scene_name).first().map(|t| t.to_string()))
            .unwrap_or_else(|| "—".into()),
        S::ColorGrade => {
            if ctx.look.basic_grade_is_neutral() && ctx.look.lut_is_identity() {
                "neutral".into()
            } else if ctx.look.basic_grade_is_neutral() {
                "graded".into()
            } else {
                format!(
                    "{} · {} · {}",
                    color_wheel::format_hue(g.hue),
                    color_wheel::format_sat(g.saturation),
                    color_wheel::format_contrast(g.contrast)
                )
            }
        }
        S::Exposure if g.exposure.abs() < 0.05 => "0 EV".into(),
        S::Exposure => format!("{:+.1} EV", g.exposure),
        S::Contrast => format!("{:.2}×", g.contrast),
        S::Saturation => format!("{:.0}%", g.saturation * 100.0),
        S::Vibrance => signed(g.vibrance),
        S::Temperature => signed(g.temperature),
        S::Tint => signed(g.tint),
        S::Hue => color_wheel::format_hue(g.hue),
        S::Matte => strength(g.fade),
        S::PaletteMode => ctx.look.palette.mode.name().into(),
        S::Palette => match ctx.look.palette.colors.len() {
            0 => "none".into(),
            n => format!("{n} colours"),
        },
        S::PaletteStrength => format!("{:.0}%", ctx.look.palette.strength * 100.0),
        S::Filters => match ctx.look.effects.stack.len() {
            0 => "none".into(),
            n => format!("{n} on"),
        },
        S::Glow | S::Vignette | S::Grain | S::Letterbox => strength(num_value(id, ctx).unwrap_or(0.0)),
        S::Dim => format!("{:.0}%", ctx.dim * 100.0),
        S::TextScale => ctx.text_scale.map_or("auto".into(), |t| format!("{t}×")),
        S::ResetLook if ctx.look.is_neutral() => "neutral".into(),
        S::ResetLook | S::ResetPlayback | S::ResetDisplay | S::TuneRandom | S::ResetKeys => String::new(),
        S::Param0 | S::Param1 | S::Param2 | S::Param3 if param_index(id) >= ctx.param_labels.len() => "—".into(),
        S::Param0 | S::Param1 | S::Param2 | S::Param3 => format!("{:.0}%", ctx.params[param_index(id)] * 100.0),
        S::TuneReset if ctx.params.iter().all(|v| (v - crate::scene::shader::PARAM_NEUTRAL).abs() < 1e-3) => "as designed".into(),
        S::TuneReset => String::new(),
        S::Key(i) => ctx.key_display.get(i as usize).map(|(_, k)| pretty_key(k)).unwrap_or_default(),
        S::Speed => format!("{}×", short(ctx.speed)),
        S::Cycle => cycle_label(ctx.cycle),
        S::CycleScope => match ctx.cycle_scope {
            CycleScope::All => "all scenes".into(),
            CycleScope::Category => "this category".into(),
            CycleScope::Favorites => "favourites".into(),
            CycleScope::Studio => "Studio scenes".into(),
            CycleScope::Classic => "Classic scenes".into(),
            CycleScope::Variants => "this scene's variants".into(),
            CycleScope::Surprise => "surprise me".into(),
            CycleScope::Playlist => "the playlist".into(),
        },
        S::Order => ctx.playback.order.label().into(),
        S::TimeOfDay => on_off(ctx.playback.time_of_day),
        S::OnLaunch => ctx.playback.on_launch.label().into(),
        S::Transition => ctx.playback.transition.label().into(),
        S::Fade => format!("{} s", short(ctx.fade)),
        S::Fullscreen if !ctx.fullscreen_available => "not available here".into(),
        S::Fullscreen => ctx.fullscreen_status.clone(),
        S::Colors if ctx.display.colors == crate::prefs::Colors::Auto => {
            format!("auto ({})", if ctx.term_truecolor { "24-bit" } else { "256" })
        }
        S::Colors => ctx.display.colors.label().into(),
        S::Quality => ctx.detail.name().into(),
        S::Pixels => ctx.pixels.name().into(),
        S::CellShape => ctx.display.cell_aspect.map_or("auto".into(), |a| format!("{a:.1} : 1")),
        S::Fps => ctx.fps.to_string(),
        S::AdaptFps => on_off(ctx.display.adapt_fps),
        S::Unfocused => ctx.display.unfocused.label().into(),
        S::Battery => ctx.display.battery.label().into(),
        S::Bandwidth => ctx.display.bandwidth.label().into(),
        S::Smooth if ctx.smooth < 0.05 => "off".into(),
        S::Smooth => format!("{:.1}", ctx.smooth),
        S::Clock => on_off(ctx.clock),
        S::ClockSize => ctx.display.clock_style.label().into(),
        S::ClockFormat => ctx.display.clock_format.label().into(),
        S::ClockCorner => ctx.display.clock_corner.label().into(),
        S::ClockColor => ctx.display.clock_color.label().into(),
        S::ClockLevel => format!("{:.0}%", ctx.display.clock_level * 100.0),
        S::Caption => on_off(ctx.display.caption),
        S::CaptionSecs => format!("{} s", short(ctx.display.caption_secs)),
        S::BatteryMeter => on_off(ctx.display.battery_meter),
        S::Hud => on_off(ctx.display.hud),
        S::Mouse => on_off(ctx.display.mouse),
        S::MenuAccent => ctx.display.menu_accent.label().into(),
        S::MenuSide => ctx.display.menu_side.label().into(),
        S::MenuFrost => on_off(ctx.display.menu_frost),
        S::PauseBehind if !ctx.hypr => "needs Hyprland".into(),
        S::PauseBehind => on_off(ctx.display.pause_behind_fullscreen),
        S::Night => on_off(ctx.display.night),
        S::NightFrom => hour_label(ctx.display.night_from),
        S::NightTo => hour_label(ctx.display.night_to),
        S::NightLevel => format!("{:.0}%", ctx.display.night_level * 100.0),
        S::TerminalCheck => String::new(),
        S::Renderer => renderer_name(ctx.renderer).into(),
        S::Gpu => ctx.display.gpu.label().into(),
        S::StudioBudget => format!("{} ms", short(ctx.display.studio_budget_ms)),
        S::StudioFps => ctx.display.studio_fps.to_string(),
        S::DepthStrength => format!("{:.0}%", ctx.display.depth_strength * 100.0),
        S::CameraMotion => ctx.display.camera_motion.label().into(),
        S::PointerParallax => ctx.display.pointer_parallax.label().into(),
        S::Link if ctx.link_enabled => "on".into(),
        S::Link => "off (solo)".into(),
        S::Group if ctx.link_enabled => ctx.link_group.clone(),
        S::Group => "—".into(),
        S::NewGroup => String::new(),
        S::SyncLook => on_off(ctx.wall.sync_look),
        S::Layout => ctx.wall.mode.label().into(),
        S::WallView => ctx.wall.view.label().into(),
        S::FocusReaction => ctx.wall.focus.label().into(),
        S::Grid => format!("{} × {}", ctx.wall.grid.0, ctx.wall.grid.1),
        S::GridPos => {
            let (c, _, i) = ctx.wall.grid;
            format!("column {}, row {}", i % c.max(1) + 1, i / c.max(1) + 1)
        }
        S::Padding => format!("{} px", short(ctx.wall.pad)),
        S::Placement => match ctx.wall.placement {
            crate::wall::Placement::TopLeft => "top-left".into(),
            crate::wall::Placement::Center => "centre".into(),
        },
        S::Bezel => format!("{} mm", short(ctx.wall.bezel_mm)),
        S::PauseWall => if ctx.paused { "paused" } else { "playing" }.into(),
        S::WallUp | S::WallDown if !ctx.hypr => "needs Hyprland".into(),
        S::WallUp | S::WallDown => String::new(),
        S::Align => String::new(),
        S::Instances if !ctx.link_enabled => "solo".into(),
        S::Instances => format!("{} live", ctx.instances.len()),
    }
}

/// Step a row's value back (`dir < 0`) or forward. None when nothing
/// changes: a clamped end, a disabled row, or a row without a value.
pub fn step(id: SettingId, ctx: &MenuCtx, dir: i32) -> Option<Effect> {
    let up = dir > 0;
    if !enabled(id, ctx) {
        return None;
    }
    // Dim, Smooth and Fade are sliders with their own effects (set_num)
    if num(id).is_some() {
        return step_num(id, ctx, dir);
    }
    match id {
        S::Theme => {
            let n = ctx.themes.len();
            if n == 0 {
                return None;
            }
            let cur = ctx.active_theme.as_deref().and_then(|s| ctx.themes.iter().position(|t| t.slug == s));
            let i = match cur {
                Some(i) => wrap(i, n, dir),
                None if up => 0,
                None => n - 1,
            };
            Some(Effect::ApplyTheme(ctx.themes[i].slug.clone()))
        }
        S::Variant => {
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
        S::PaletteMode => {
            let all = PaletteMode::ALL;
            let i = all.iter().position(|m| *m == ctx.look.palette.mode).unwrap_or(0);
            let mut look = ctx.look.clone();
            look.palette.mode = all[wrap(i, all.len(), dir)];
            if look.palette.colors.len() < 2 {
                look.palette.colors = DEFAULT_PALETTE.to_vec();
            }
            Some(Effect::SetLook(look))
        }
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
        S::Order => with_playback(ctx, |p| p.order = step_in(crate::prefs::CycleOrder::ALL, p.order, dir)),
        S::TimeOfDay => with_playback(ctx, |p| p.time_of_day = !p.time_of_day),
        S::OnLaunch => with_playback(ctx, |p| p.on_launch = step_in(crate::prefs::OnLaunch::ALL, p.on_launch, dir)),
        S::Transition => with_playback(ctx, |p| p.transition = step_in(crate::prefs::TransitionStyle::ALL, p.transition, dir)),
        S::Fullscreen => with_display(ctx, |d| d.fullscreen = !d.fullscreen),
        S::Colors => with_display(ctx, |d| d.colors = step_in(crate::prefs::Colors::ALL, d.colors, dir)),
        S::CellShape => with_display(ctx, |d| {
            // auto, then the shapes
            let all: Vec<Option<f32>> = std::iter::once(None).chain(CELL_SHAPES.iter().map(|a| Some(*a))).collect();
            let i = all.iter().position(|a| a.map(|v| (v * 10.0).round()) == d.cell_aspect.map(|v| (v * 10.0).round())).unwrap_or(0);
            d.cell_aspect = all[wrap(i, all.len(), dir)];
        }),
        S::AdaptFps => with_display(ctx, |d| d.adapt_fps = !d.adapt_fps),
        S::Unfocused => with_display(ctx, |d| d.unfocused = step_in(crate::prefs::Unfocused::ALL, d.unfocused, dir)),
        S::Battery => with_display(ctx, |d| d.battery = step_in(crate::prefs::Battery::ALL, d.battery, dir)),
        S::Bandwidth => with_display(ctx, |d| d.bandwidth = step_in(crate::prefs::Bandwidth::ALL, d.bandwidth, dir)),
        S::Gpu => with_display(ctx, |d| d.gpu = step_in(crate::prefs::GpuChoice::ALL, d.gpu, dir)),
        S::StudioFps => with_display(ctx, |d| {
            let i = STUDIO_FPS_STEPS.iter().position(|f| *f >= d.studio_fps).unwrap_or(STUDIO_FPS_STEPS.len() - 1);
            d.studio_fps = STUDIO_FPS_STEPS[clamp_step(i, STUDIO_FPS_STEPS.len(), dir)];
        }),
        S::CameraMotion => with_display(ctx, |d| d.camera_motion = step_in(crate::prefs::CameraMotion::ALL, d.camera_motion, dir)),
        S::PointerParallax => with_display(ctx, |d| d.pointer_parallax = step_in(crate::prefs::Response::ALL, d.pointer_parallax, dir)),
        S::ClockSize => with_display(ctx, |d| d.clock_style = step_in(crate::prefs::ClockStyle::ALL, d.clock_style, dir)),
        S::ClockFormat => with_display(ctx, |d| d.clock_format = step_in(crate::prefs::ClockFormat::ALL, d.clock_format, dir)),
        S::ClockCorner => with_display(ctx, |d| d.clock_corner = step_in(crate::prefs::Corner::ALL, d.clock_corner, dir)),
        S::Caption => with_display(ctx, |d| d.caption = !d.caption),
        S::ClockColor => with_display(ctx, |d| d.clock_color = step_in(crate::prefs::ClockColor::ALL, d.clock_color, dir)),
        S::BatteryMeter => with_display(ctx, |d| d.battery_meter = !d.battery_meter),
        S::MenuAccent => with_display(ctx, |d| d.menu_accent = step_in(crate::prefs::Accent::ALL, d.menu_accent, dir)),
        S::MenuSide => with_display(ctx, |d| d.menu_side = step_in(crate::prefs::Side::ALL, d.menu_side, dir)),
        S::MenuFrost => with_display(ctx, |d| d.menu_frost = !d.menu_frost),
        S::PauseBehind => with_display(ctx, |d| d.pause_behind_fullscreen = !d.pause_behind_fullscreen),
        S::Hud => with_display(ctx, |d| d.hud = !d.hud),
        S::Mouse => with_display(ctx, |d| d.mouse = !d.mouse),
        S::Night => with_display(ctx, |d| d.night = !d.night),
        S::NightFrom => with_display(ctx, |d| d.night_from = (d.night_from as i32 + dir.signum()).rem_euclid(24) as u8),
        S::NightTo => with_display(ctx, |d| d.night_to = (d.night_to as i32 + dir.signum()).rem_euclid(24) as u8),
        S::Quality => {
            let i = QUALITIES.iter().position(|q| *q == ctx.detail).unwrap_or(1);
            let next = QUALITIES[clamp_step(i, QUALITIES.len(), dir)];
            (next != ctx.detail).then_some(Effect::SetDetail(next))
        }
        S::Pixels => Some(Effect::SetPixels(if up { ctx.pixels.next() } else { ctx.pixels.prev() })),
        S::Fps => {
            let next = fps_step(ctx.fps, up);
            (next != ctx.fps).then_some(Effect::SetFps(next))
        }
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
        S::Group => Some(Effect::SetLinkGroup(next_group(&ctx.link_group, dir))),
        S::SyncLook => with_wall(ctx, |w| w.sync_look = !w.sync_look),
        S::Layout => with_wall(ctx, |w| w.mode = step_in(crate::prefs::WallMode::ALL, w.mode, dir)),
        S::WallView => with_wall(ctx, |w| w.view = step_in(crate::prefs::WallView::ALL, w.view, dir)),
        S::FocusReaction => with_wall(ctx, |w| w.focus = step_in(crate::prefs::Response::ALL, w.focus, dir)),
        S::Grid => with_wall(ctx, |w| {
            let (c, r, _) = w.grid;
            let (nc, nr) = step_in(crate::prefs::GRIDS, (c, r), dir);
            w.grid = (nc, nr, w.grid.2.min(nc * nr - 1));
        }),
        S::GridPos => with_wall(ctx, |w| {
            let (c, r, i) = w.grid;
            w.grid.2 = (i as i32 + dir.signum()).rem_euclid(c as i32 * r as i32) as u8;
        }),
        S::Placement => with_wall(ctx, |w| {
            w.placement = match w.placement {
                crate::wall::Placement::TopLeft => crate::wall::Placement::Center,
                crate::wall::Placement::Center => crate::wall::Placement::TopLeft,
            }
        }),
        _ => None,
    }
}

/// What Enter does on an Action row.
pub fn action(id: SettingId, ctx: &MenuCtx) -> Option<Effect> {
    match id {
        S::ResetLook => (!ctx.look.is_neutral()).then(|| Effect::SetLook(Look::default())),
        S::TuneRandom => Some(Effect::SetParams(random_params(ctx))),
        S::TuneReset => Some(Effect::SetParams([crate::scene::shader::PARAM_NEUTRAL; 4])),
        S::ResetKeys => Some(Effect::ResetKeys),
        S::ResetPlayback => Some(Effect::ResetPage(Page::Playback)),
        S::ResetDisplay => Some(Effect::ResetPage(Page::Display)),
        S::PauseWall => Some(Effect::PauseWall),
        S::WallUp => Some(Effect::WallUp),
        S::WallDown => Some(Effect::WallDown),
        _ => None,
    }
}

// ── Effects sub-page ────────────────────────────────────────────────────

/// Effects by what they do, in sub-page order. Covers `filter::FILTER_CYCLE`
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
        "Light & texture",
        &[
            "bloom",
            "halation",
            "vignette",
            "grain",
            "scanlines",
            "crt",
            "chroma",
            "dither",
        ],
    ),
    (
        "Shape",
        &[
            "letterbox",
            "tiltshift",
            "pixelate",
            "warp",
            "mirror",
            "kaleido",
            "edges",
            "sharpen",
        ],
    ),
];

/// Named effect stacks with strengths, applied in this order.
pub const PRESETS: &[(&str, &[(&str, f32)])] = &[
    ("Clean", &[]),
    ("Film", &[("warm", 1.0), ("grain", 1.0), ("vignette", 1.0)]),
    ("CRT", &[("bloom", 1.0), ("crt", 1.0)]),
    ("Dream", &[("bloom", 1.0), ("chroma", 1.0), ("vignette", 1.0)]),
    ("Cinema", &[("halation", 0.8), ("grain", 0.5), ("vignette", 0.6), ("letterbox", 1.0)]),
    ("VHS", &[("chroma", 1.4), ("warp", 0.5), ("scanlines", 0.8), ("grain", 0.8)]),
    ("Arcade", &[("pixelate", 0.5), ("bloom", 1.2), ("scanlines", 0.6)]),
    ("Neon", &[("bloom", 1.6), ("halation", 0.5), ("chroma", 0.6)]),
    ("Soft", &[("bloom", 0.8), ("vignette", 0.6)]),
    ("Miniature", &[("tiltshift", 1.0), ("vignette", 0.4)]),
    ("Sketch", &[("edges", 1.4)]),
    ("Print", &[("posterize", 1.0), ("grain", 0.6), ("vignette", 0.5)]),
    ("8-bit", &[("pixelate", 0.5), ("dither", 1.2)]),
    ("Kaleidoscope", &[("kaleido", 1.0), ("bloom", 0.7)]),
];

/// A preset's effect stack.
pub fn preset_effects(i: usize) -> Effects {
    let mut e = Effects::default();
    for (name, a) in PRESETS[i].1 {
        e.stack.push(name.to_string());
        e.set_amount(name, *a);
    }
    e
}

/// Every effect in sub-page order.
pub fn filter_list() -> impl Iterator<Item = &'static str> {
    FILTER_GROUPS.iter().flat_map(|(_, f)| f.iter().copied())
}

/// Selectable rows on the Effects sub-page: the preset row, then effects.
pub fn filter_rows() -> usize {
    1 + filter_list().count()
}

/// The effect on sub-page row `row` (row 0 is the preset).
pub fn filter_at(row: usize) -> Option<&'static str> {
    row.checked_sub(1).and_then(|i| filter_list().nth(i))
}

/// The preset matching the stack and its strengths exactly, if any.
pub fn preset_of(effects: &Effects) -> Option<usize> {
    PRESETS.iter().enumerate().position(|(i, _)| {
        let p = preset_effects(i);
        p.stack == effects.stack && p.stack.iter().all(|n| (p.amount(n) - effects.amount(n)).abs() < 0.01)
    })
}

/// A look with its effect stack replaced.
fn with_effects(look: &Look, e: Effects) -> Effect {
    let mut l = look.clone();
    l.effects = e;
    Effect::SetLook(l)
}

/// Step the preset row, or change an effect's strength: → turns an effect on
/// (at its classic strength) or strengthens it, ← weakens it and takes it
/// off at zero. On/off effects (mirror) just flip.
pub fn filter_step(row: usize, ctx: &MenuCtx, dir: i32) -> Option<Effect> {
    let e = &ctx.look.effects;
    if row == 0 {
        let n = PRESETS.len();
        let next = match preset_of(e) {
            Some(i) => wrap(i, n, dir),
            None if dir > 0 => 0,
            None => n - 1,
        };
        return Some(with_effects(&ctx.look, preset_effects(next)));
    }
    let name = filter_at(row)?;
    if !crate::filter::has_amount(name) {
        return filter_toggle(row, ctx);
    }
    let on = e.stack.iter().any(|s| s == name);
    let cur = if on { e.amount(name) } else { 0.0 };
    let next = if !on && dir > 0 {
        1.0
    } else {
        round_dec((cur + 0.1 * dir.signum() as f32).clamp(0.0, 2.0), 1)
    };
    changed(cur, next).map(|v| Effect::SetLook(with_effect(&ctx.look, name, v)))
}

/// Set an effect's strength from a point on its bar (the mouse); the far
/// left takes it off.
pub fn filter_set_strength(row: usize, ctx: &MenuCtx, frac: f32) -> Option<Effect> {
    let name = filter_at(row)?;
    if !crate::filter::has_amount(name) {
        return None;
    }
    let v = round_dec((frac.clamp(0.0, 1.0) * 2.0 * 10.0).round() / 10.0, 1);
    let e = &ctx.look.effects;
    let cur = if e.stack.iter().any(|s| s == name) { e.amount(name) } else { 0.0 };
    changed(cur, v).map(|v| Effect::SetLook(with_effect(&ctx.look, name, v)))
}

/// Enter on an effect row: on at its classic strength, or off.
pub fn filter_toggle(row: usize, ctx: &MenuCtx) -> Option<Effect> {
    let name = filter_at(row)?;
    let on = ctx.look.effects.stack.iter().any(|s| s == name);
    Some(Effect::SetLook(with_effect(&ctx.look, name, if on { 0.0 } else { 1.0 })))
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
        "hue" => "Turns every colour round the wheel (120° at full strength).",
        "spectrum" => "Colours drift round the wheel, once every ~12 s.",
        "gamma" => "Deeper midtones and shadows.",
        "posterize" => "Flattens colour into a few bands.",
        "bloom" => "Bright areas bleed light.",
        "halation" => "A warm film glow around highlights.",
        "vignette" => "Darkens toward the edges.",
        "grain" => "Animated film grain.",
        "scanlines" => "Darkens every other pixel row.",
        "crt" => "Scanlines, vignette and colour fringing.",
        "chroma" => "Colour fringing toward the edges.",
        "dither" => "Few colours, ordered dithering: 8-bit graphics.",
        "letterbox" => "Cinema bars top and bottom.",
        "tiltshift" => "Sharp band across the middle, blur above and below: a miniature.",
        "pixelate" => "Chunky mosaic.",
        "warp" => "Rows ripple side to side.",
        "mirror" => "Flips the picture left to right.",
        "kaleido" => "Four-fold mirror symmetry.",
        "edges" => "Only outlines glow, on black.",
        "sharpen" => "Crisper detail.",
        _ => "",
    }
}
