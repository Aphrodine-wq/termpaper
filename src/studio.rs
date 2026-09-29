//! The colour studio: every grading control of the Look in one panel.
//!
//! Three tabs, each a column of controls beside a wheel:
//!
//! - **Colour**: a colour-balance wheel (the puck is temperature across,
//!   tint up and down) ringed by the hue shift, with exposure, contrast,
//!   saturation, vibrance, temperature, tint, hue and matte.
//! - **Tones**: three small wheels, shadows, midtones and highlights, each
//!   pushing its range toward a hue, and the balance between them.
//! - **Palette**: named palettes, the palette mode and strength, and an
//!   editor for each colour (the wheel picks hue and saturation).
//!
//! Along the bottom, reference colours before and after the look. The
//! wheels are drawn in half blocks (two round pixels per cell, corrected
//! for the terminal's cell shape) and anti-aliased. Keys and mouse reach
//! every control; changes leave as [`Effect::SetLook`], so they sync, save
//! and undo like the menu's.

use crate::canvas::{hsv, rgb_to_256, Canvas};
use crate::look::{Look, PaletteMode, Rgb, MAX_COLORS, PALETTES};
use crate::menu::{Effect, Input, Mouse};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};
use std::cell::RefCell;

pub fn format_hue(h: f32) -> String {
    if h < 0.5 {
        "off".into()
    } else {
        format!("{:.0}°", h)
    }
}

pub fn format_sat(s: f32) -> String {
    format!("{:.0}%", s * 100.0)
}

pub fn format_contrast(c: f32) -> String {
    format!("{:.2}×", c)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Colour,
    Tones,
    Palette,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Colour, Tab::Tones, Tab::Palette];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Colour => "Colour",
            Tab::Tones => "Tones",
            Tab::Palette => "Palette",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// One control on a tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ctl {
    Exposure,
    Contrast,
    Saturation,
    Vibrance,
    Temperature,
    Tint,
    Hue,
    Matte,
    ShadowHue,
    ShadowAmount,
    MidHue,
    MidAmount,
    HighHue,
    HighAmount,
    Balance,
    Preset,
    Mode,
    Strength,
    Dither,
    Swatch,
    SwatchHue,
    SwatchSat,
    SwatchVal,
    AddColour,
    RemoveColour,
}

use Ctl as C;

fn controls(tab: Tab) -> &'static [Ctl] {
    match tab {
        Tab::Colour => &[C::Exposure, C::Contrast, C::Saturation, C::Vibrance, C::Temperature, C::Tint, C::Hue, C::Matte],
        Tab::Tones => &[C::ShadowHue, C::ShadowAmount, C::MidHue, C::MidAmount, C::HighHue, C::HighAmount, C::Balance],
        Tab::Palette => &[
            C::Preset,
            C::Mode,
            C::Strength,
            C::Dither,
            C::Swatch,
            C::SwatchHue,
            C::SwatchSat,
            C::SwatchVal,
            C::AddColour,
            C::RemoveColour,
        ],
    }
}

fn label(c: Ctl) -> &'static str {
    match c {
        C::Exposure => "Exposure",
        C::Contrast => "Contrast",
        C::Saturation => "Saturation",
        C::Vibrance => "Vibrance",
        C::Temperature => "Temperature",
        C::Tint => "Tint",
        C::Hue => "Hue shift",
        C::Matte => "Matte",
        C::ShadowHue => "Shadows hue",
        C::ShadowAmount => "Shadows",
        C::MidHue => "Midtones hue",
        C::MidAmount => "Midtones",
        C::HighHue => "Highlights hue",
        C::HighAmount => "Highlights",
        C::Balance => "Balance",
        C::Preset => "Palette",
        C::Mode => "Mode",
        C::Strength => "Strength",
        C::Dither => "Dither",
        C::Swatch => "Colour",
        C::SwatchHue => "Hue",
        C::SwatchSat => "Saturation",
        C::SwatchVal => "Brightness",
        C::AddColour => "Add colour",
        C::RemoveColour => "Remove colour",
    }
}

fn help(c: Ctl) -> &'static str {
    match c {
        C::Exposure => "Brighter or darker, in photographic stops.",
        C::Contrast => "Punchier or flatter.",
        C::Saturation => "Stronger or softer colour; 0% is black and white.",
        C::Vibrance => "Lifts muted colours more than vivid ones.",
        C::Temperature => "Cooler or warmer light: the wheel's puck, left and right.",
        C::Tint => "Toward green or magenta: the wheel's puck, down and up.",
        C::Hue => "Turns every colour round the wheel; the outer ring shows where they land.",
        C::Matte => "Lifts the blacks toward grey: a faded-film look.",
        C::ShadowHue | C::MidHue | C::HighHue => "The colour this range leans toward (or drag its wheel).",
        C::ShadowAmount => "How strongly the shadows take their colour.",
        C::MidAmount => "How strongly the midtones take their colour.",
        C::HighAmount => "How strongly the highlights take their colour.",
        C::Balance => "Where the shadows end and the highlights begin.",
        C::Preset => "A named palette: terminal colour schemes and moods.",
        C::Mode => "map: by brightness · tint: its hues, your brightness · snap: only its colours.",
        C::Strength => "How far toward the palette (map and tint).",
        C::Dither => "Snap: mix the two nearest colours in a fine pattern.",
        C::Swatch => "Pick a colour to edit (←→), or click one.",
        C::SwatchHue | C::SwatchSat => "Edit the colour, or drag it on the wheel.",
        C::SwatchVal => "The colour's brightness.",
        C::AddColour => "Add a colour after this one (up to 8).",
        C::RemoveColour => "Remove this colour (a palette keeps at least 2).",
    }
}

/// A slider's range: `neutral` is where the bar fills from.
#[derive(Clone, Copy)]
struct Spec {
    min: f32,
    max: f32,
    step: f32,
    neutral: f32,
    places: u32,
    wraps: bool,
}

const fn s(min: f32, max: f32, step: f32, neutral: f32, places: u32) -> Spec {
    Spec {
        min,
        max,
        step,
        neutral,
        places,
        wraps: false,
    }
}

const HUE: Spec = Spec {
    min: 0.0,
    max: 360.0,
    step: 5.0,
    neutral: 0.0,
    places: 0,
    wraps: true,
};

fn spec(c: Ctl) -> Option<Spec> {
    Some(match c {
        C::Exposure => s(-2.0, 2.0, 0.1, 0.0, 1),
        C::Contrast => s(0.5, 2.5, 0.05, 1.0, 2),
        C::Saturation => s(0.0, 2.5, 0.05, 1.0, 2),
        C::Vibrance | C::Temperature | C::Tint | C::Balance => s(-1.0, 1.0, 0.05, 0.0, 2),
        C::Hue | C::ShadowHue | C::MidHue | C::HighHue | C::SwatchHue => HUE,
        C::Matte => s(0.0, 0.5, 0.02, 0.0, 2),
        C::ShadowAmount | C::MidAmount | C::HighAmount | C::Strength | C::SwatchSat | C::SwatchVal => {
            s(0.0, 1.0, 0.05, 0.0, 2)
        }
        _ => return None,
    })
}

fn round_dec(v: f32, places: u32) -> f32 {
    let k = 10f32.powi(places as i32);
    (v * k).round() / k
}

/// RGB → (hue degrees, saturation, value).
fn to_hsv(c: Rgb) -> (f32, f32, f32) {
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d < 1e-6 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let sat = if max < 1e-6 { 0.0 } else { d / max };
    (h, sat, max)
}

fn from_hsv(h: f32, sat: f32, v: f32) -> Rgb {
    let (r, g, b) = hsv(h / 360.0, sat.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    Rgb(r, g, b)
}

fn get(c: Ctl, l: &Look, sw: usize) -> f32 {
    let g = &l.grade;
    let swatch = || l.palette.colors.get(sw).copied().map(to_hsv).unwrap_or((0.0, 0.0, 0.0));
    match c {
        C::Exposure => g.exposure,
        C::Contrast => g.contrast,
        C::Saturation => g.saturation,
        C::Vibrance => g.vibrance,
        C::Temperature => g.temperature,
        C::Tint => g.tint,
        C::Hue => g.hue,
        C::Matte => g.fade,
        C::ShadowHue => g.shadows.hue,
        C::ShadowAmount => g.shadows.amount,
        C::MidHue => g.midtones.hue,
        C::MidAmount => g.midtones.amount,
        C::HighHue => g.highlights.hue,
        C::HighAmount => g.highlights.amount,
        C::Balance => g.balance,
        C::Strength => l.palette.strength,
        C::SwatchHue => swatch().0,
        C::SwatchSat => swatch().1,
        C::SwatchVal => swatch().2,
        _ => 0.0,
    }
}

fn set(c: Ctl, l: &mut Look, sw: usize, v: f32) {
    let g = &mut l.grade;
    match c {
        C::Exposure => g.exposure = v,
        C::Contrast => g.contrast = v,
        C::Saturation => g.saturation = v,
        C::Vibrance => g.vibrance = v,
        C::Temperature => g.temperature = v,
        C::Tint => g.tint = v,
        C::Hue => g.hue = v,
        C::Matte => g.fade = v,
        C::ShadowHue => g.shadows.hue = v,
        C::ShadowAmount => g.shadows.amount = v,
        C::MidHue => g.midtones.hue = v,
        C::MidAmount => g.midtones.amount = v,
        C::HighHue => g.highlights.hue = v,
        C::HighAmount => g.highlights.amount = v,
        C::Balance => g.balance = v,
        C::Strength => l.palette.strength = v,
        C::SwatchHue | C::SwatchSat | C::SwatchVal => {
            if let Some(col) = l.palette.colors.get_mut(sw) {
                let (mut h, mut sat, mut val) = to_hsv(*col);
                match c {
                    C::SwatchHue => h = v,
                    C::SwatchSat => sat = v,
                    _ => val = v,
                }
                *col = from_hsv(h, sat, val);
            }
        }
        _ => {}
    }
}

fn signed(v: f32) -> String {
    if v.abs() < 5e-3 {
        "0".into()
    } else {
        format!("{:+.0}", v * 100.0)
    }
}

fn text(c: Ctl, l: &Look, sw: usize) -> String {
    let v = get(c, l, sw);
    match c {
        C::Exposure if v.abs() < 0.05 => "0 EV".into(),
        C::Exposure => format!("{v:+.1} EV"),
        C::Contrast => format_contrast(v),
        C::Saturation => format_sat(v),
        C::Vibrance | C::Temperature | C::Tint | C::Balance => signed(v),
        C::Hue => format_hue(v),
        C::ShadowHue | C::MidHue | C::HighHue | C::SwatchHue => format!("{v:.0}°"),
        C::Matte | C::ShadowAmount | C::MidAmount | C::HighAmount | C::Strength | C::SwatchSat | C::SwatchVal => {
            format!("{:.0}%", v * 100.0)
        }
        C::Preset => crate::look::palette_name(&l.palette.colors).unwrap_or("custom").into(),
        C::Mode => l.palette.mode.name().into(),
        C::Dither => if l.palette.dither { "on" } else { "off" }.into(),
        C::Swatch => match l.palette.colors.get(sw) {
            Some(c) => format!("{} of {} · {}", sw + 1, l.palette.colors.len(), c.hex()),
            None => "none".into(),
        },
        C::AddColour | C::RemoveColour => String::new(),
    }
}

/// Whether a control does anything with this look.
fn enabled(c: Ctl, l: &Look) -> bool {
    let has = l.palette.colors.len();
    match c {
        C::Strength => matches!(l.palette.mode, PaletteMode::Map | PaletteMode::Tint),
        C::Dither => l.palette.mode == PaletteMode::Snap,
        C::Swatch | C::SwatchHue | C::SwatchSat | C::SwatchVal => has > 0,
        C::AddColour => has < MAX_COLORS,
        C::RemoveColour => has > 2,
        _ => true,
    }
}

/// What a point in the panel is, recorded as it draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Outside,
    Panel,
    Tab(Tab),
    Ctl(usize),
    Slider { ctl: usize, x0: u16, width: u16 },
    /// a wheel: which (0 = the big one, 1..=3 = shadows, midtones,
    /// highlights), its centre and radii in cells
    Wheel { which: u8, cx: f32, cy: f32, rx: f32, ry: f32 },
    Swatch(usize),
}

pub struct Studio {
    pub open: bool,
    pub tab: Tab,
    focus: [usize; 3],
    /// the palette colour being edited
    pub swatch: usize,
    pub hits: RefCell<Vec<(Rect, Hit)>>,
    /// a drag keeps working the control it started on
    drag: Option<Hit>,
}

impl Default for Studio {
    fn default() -> Self {
        Self::new()
    }
}

/// A 1x1 canvas through the colour stages, for the reference swatches.
pub fn graded(c: (u8, u8, u8), look: &Look) -> (u8, u8, u8) {
    let mut cv = Canvas::new(1, 1);
    cv.set(0, 0, c);
    let g = &look.grade;
    crate::color_grade::apply(&mut cv, g.hue, g.saturation, g.contrast);
    let c = cv.get(0, 0).color;
    let t = look.transform([c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0]);
    let out = ((t[0] * 255.0).round() as u8, (t[1] * 255.0).round() as u8, (t[2] * 255.0).round() as u8);
    if look.snaps() {
        crate::look::snap_pixel(out, &look.palette.colors, false, 0, 0)
    } else {
        out
    }
}

/// Reference colours shown before and after the look: skin, sky, foliage,
/// sunset, neon, white, grey, shadow, red, yellow.
pub const REFS: [(u8, u8, u8); 10] = [
    (232, 186, 160),
    (112, 162, 222),
    (72, 132, 62),
    (242, 142, 62),
    (212, 64, 162),
    (248, 248, 242),
    (128, 128, 128),
    (40, 42, 50),
    (200, 44, 44),
    (250, 220, 92),
];

/// The hue the wheel shows at angle `theta` (degrees, counter-clockwise from
/// the right): warm to the right, magenta up, cool to the left, green down,
/// the way temperature and tint move a picture.
fn hue_at(theta: f32) -> f32 {
    (30.0 - theta).rem_euclid(360.0)
}

/// The inverse: the angle a hue sits at on the wheel.
fn angle_of(hue: f32) -> f32 {
    (30.0 - hue).rem_euclid(360.0)
}

impl Studio {
    pub fn new() -> Self {
        Studio {
            open: false,
            tab: Tab::Colour,
            focus: [0; 3],
            swatch: 0,
            hits: RefCell::new(Vec::new()),
            drag: None,
        }
    }

    pub fn open_on(&mut self, tab: Tab) {
        self.open = true;
        self.tab = tab;
        self.drag = None;
    }

    fn ctl(&self) -> Ctl {
        let c = controls(self.tab);
        c[self.focus[self.tab.index()].min(c.len() - 1)]
    }

    fn set_look(l: Look) -> Vec<Effect> {
        vec![Effect::SetLook(l)]
    }

    /// Step the focused control; `mul` for big steps.
    fn step(&mut self, look: &Look, dir: i32, mul: f32) -> Vec<Effect> {
        let c = self.ctl();
        if !enabled(c, look) {
            return Vec::new();
        }
        let mut l = look.clone();
        let sw = self.swatch.min(l.palette.colors.len().saturating_sub(1));
        match c {
            C::Preset => {
                let n = PALETTES.len();
                let cur = PALETTES.iter().position(|(_, p)| *p == l.palette.colors.as_slice());
                let i = match cur {
                    Some(i) => (i as i64 + dir.signum() as i64).rem_euclid(n as i64) as usize,
                    None if dir > 0 => 0,
                    None => n - 1,
                };
                l.palette.colors = PALETTES[i].1.to_vec();
                if l.palette.mode == PaletteMode::Off {
                    l.palette.mode = PaletteMode::Map;
                }
                self.swatch = 0;
                Self::set_look(l)
            }
            C::Mode => {
                let all = PaletteMode::ALL;
                let i = all.iter().position(|m| *m == l.palette.mode).unwrap_or(0);
                l.palette.mode = all[(i as i64 + dir.signum() as i64).rem_euclid(all.len() as i64) as usize];
                if l.palette.colors.len() < 2 {
                    l.palette.colors = PALETTES[0].1.to_vec();
                }
                Self::set_look(l)
            }
            C::Dither => {
                l.palette.dither = !l.palette.dither;
                Self::set_look(l)
            }
            C::Swatch => {
                let n = l.palette.colors.len().max(1);
                self.swatch = (sw as i64 + dir.signum() as i64).rem_euclid(n as i64) as usize;
                Vec::new()
            }
            C::AddColour | C::RemoveColour => Vec::new(),
            _ => {
                let Some(sp) = spec(c) else {
                    return Vec::new();
                };
                let cur = get(c, &l, sw);
                let raw = cur + sp.step * mul * dir.signum() as f32;
                let v = if sp.wraps {
                    round_dec(raw, sp.places).rem_euclid(sp.max)
                } else {
                    round_dec(raw.clamp(sp.min, sp.max), sp.places)
                };
                if (v - cur).abs() < 1e-4 {
                    return Vec::new();
                }
                set(c, &mut l, sw, v);
                Self::set_look(l)
            }
        }
    }

    fn activate(&mut self, look: &Look) -> Vec<Effect> {
        let c = self.ctl();
        if !enabled(c, look) {
            return Vec::new();
        }
        let mut l = look.clone();
        let n = l.palette.colors.len();
        match c {
            C::AddColour => {
                let at = (self.swatch + 1).min(n);
                // halfway to the next colour, or a lighter copy at the end
                let base = l.palette.colors.get(self.swatch).copied().unwrap_or(Rgb(128, 128, 128));
                let next = l.palette.colors.get(at).copied();
                let new = match next {
                    Some(b) => Rgb(
                        ((base.0 as u16 + b.0 as u16) / 2) as u8,
                        ((base.1 as u16 + b.1 as u16) / 2) as u8,
                        ((base.2 as u16 + b.2 as u16) / 2) as u8,
                    ),
                    None => {
                        let (h, s, v) = to_hsv(base);
                        from_hsv(h, s * 0.8, (v + 0.25).min(1.0))
                    }
                };
                l.palette.colors.insert(at, new);
                self.swatch = at;
                Self::set_look(l)
            }
            C::RemoveColour => {
                l.palette.colors.remove(self.swatch.min(n - 1));
                self.swatch = self.swatch.min(l.palette.colors.len() - 1);
                Self::set_look(l)
            }
            _ => self.step(look, 1, 1.0),
        }
    }

    /// Reset the focused control to neutral.
    fn reset_one(&self, look: &Look) -> Vec<Effect> {
        let c = self.ctl();
        let Some(sp) = spec(c) else {
            return Vec::new();
        };
        if matches!(c, C::SwatchHue | C::SwatchSat | C::SwatchVal | C::Strength) {
            return Vec::new();
        }
        let mut l = look.clone();
        let neutral = match c {
            C::ShadowHue | C::MidHue | C::HighHue => get(c, &Look::default(), 0),
            _ => sp.neutral,
        };
        set(c, &mut l, 0, neutral);
        if l == *look {
            Vec::new()
        } else {
            Self::set_look(l)
        }
    }

    /// Reset everything on this tab.
    fn reset_tab(&self, look: &Look) -> Vec<Effect> {
        let mut l = look.clone();
        let d = Look::default();
        match self.tab {
            Tab::Colour => {
                let keep = (l.grade.shadows, l.grade.midtones, l.grade.highlights, l.grade.balance);
                l.grade = d.grade;
                (l.grade.shadows, l.grade.midtones, l.grade.highlights, l.grade.balance) = keep;
            }
            Tab::Tones => {
                l.grade.shadows = d.grade.shadows;
                l.grade.midtones = d.grade.midtones;
                l.grade.highlights = d.grade.highlights;
                l.grade.balance = 0.0;
            }
            Tab::Palette => l.palette.mode = PaletteMode::Off,
        }
        if l == *look {
            Vec::new()
        } else {
            Self::set_look(l)
        }
    }

    pub fn handle(&mut self, input: Input, look: &Look) -> Vec<Effect> {
        if !self.open {
            return Vec::new();
        }
        let ti = self.tab.index();
        let n = controls(self.tab).len();
        match input {
            Input::Mouse { kind, x, y } => return self.mouse(kind, x, y, look),
            Input::Esc | Input::Char('c') | Input::Char('q') => {
                self.open = false;
                self.drag = None;
            }
            Input::Tab => self.tab = Tab::ALL[(ti + 1) % 3],
            Input::BackTab => self.tab = Tab::ALL[(ti + 2) % 3],
            Input::Up | Input::Char('k') => self.focus[ti] = (self.focus[ti] + n - 1) % n,
            Input::Down | Input::Char('j') => self.focus[ti] = (self.focus[ti] + 1) % n,
            Input::Home => self.focus[ti] = 0,
            Input::End => self.focus[ti] = n - 1,
            Input::Left | Input::Char('h') => return self.step(look, -1, 1.0),
            Input::Right | Input::Char('l') => return self.step(look, 1, 1.0),
            Input::PageDown => return self.step(look, -1, 5.0),
            Input::PageUp => return self.step(look, 1, 5.0),
            Input::Enter | Input::Char(' ') => return self.activate(look),
            Input::Char('0') => return self.reset_one(look),
            Input::Char('R') => return self.reset_tab(look),
            Input::Char('u') => return vec![Effect::Undo],
            Input::Char('1') => self.tab = Tab::Colour,
            Input::Char('2') => self.tab = Tab::Tones,
            Input::Char('3') => self.tab = Tab::Palette,
            _ => {}
        }
        Vec::new()
    }

    fn hit_at(&self, x: u16, y: u16) -> Option<Hit> {
        self.hits
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
            .map(|(_, h)| *h)
    }

    fn mouse(&mut self, kind: Mouse, x: u16, y: u16, look: &Look) -> Vec<Effect> {
        let ti = self.tab.index();
        let n = controls(self.tab).len();
        match kind {
            Mouse::ScrollUp => {
                self.focus[ti] = (self.focus[ti] + n - 1) % n;
                return Vec::new();
            }
            Mouse::ScrollDown => {
                self.focus[ti] = (self.focus[ti] + 1) % n;
                return Vec::new();
            }
            _ => {}
        }
        // a drag stays with the control it started on
        let hit = match (kind, self.drag) {
            (Mouse::Drag, Some(h)) => Some(h),
            _ => self.hit_at(x, y),
        };
        let Some(hit) = hit else {
            return Vec::new();
        };
        if kind == Mouse::Down {
            self.drag = matches!(hit, Hit::Slider { .. } | Hit::Wheel { .. }).then_some(hit);
        }
        match (kind, hit) {
            (Mouse::Down, Hit::Outside) => {
                self.open = false;
                Vec::new()
            }
            (Mouse::Down, Hit::Tab(t)) => {
                self.tab = t;
                Vec::new()
            }
            (Mouse::Down, Hit::Ctl(i)) => {
                self.focus[ti] = i;
                let c = self.ctl();
                if matches!(c, C::AddColour | C::RemoveColour | C::Dither) {
                    return self.activate(look);
                }
                Vec::new()
            }
            (_, Hit::Slider { ctl, x0, width }) => {
                self.focus[ti] = ctl;
                let c = self.ctl();
                let (Some(sp), true) = (spec(c), enabled(c, look)) else {
                    return Vec::new();
                };
                let frac = x.saturating_sub(x0) as f32 / width.saturating_sub(1).max(1) as f32;
                let raw = sp.min + frac.clamp(0.0, 1.0) * (sp.max - sp.min);
                let mut v = round_dec(((raw / sp.step).round() * sp.step).clamp(sp.min, sp.max), sp.places);
                if sp.wraps {
                    v = v.rem_euclid(sp.max);
                }
                let mut l = look.clone();
                let sw = self.swatch.min(l.palette.colors.len().saturating_sub(1));
                set(c, &mut l, sw, v);
                if l == *look {
                    Vec::new()
                } else {
                    Self::set_look(l)
                }
            }
            (_, Hit::Wheel { which, cx, cy, rx, ry }) => {
                // the point as (u, v) on the unit disc, y up
                let (mut u, mut v) = ((x as f32 + 0.5 - cx) / rx, -(y as f32 + 0.5 - cy) / ry);
                let d = (u * u + v * v).sqrt();
                if d > 1.0 {
                    u /= d;
                    v /= d;
                }
                let r = (u * u + v * v).sqrt().min(1.0);
                let hue = hue_at(v.atan2(u).to_degrees());
                let mut l = look.clone();
                let q = |f: f32| round_dec(f, 2);
                match (which, self.tab) {
                    (0, Tab::Palette) => {
                        let sw = self.swatch.min(l.palette.colors.len().saturating_sub(1));
                        if let Some(col) = l.palette.colors.get_mut(sw) {
                            let (_, _, val) = to_hsv(*col);
                            *col = from_hsv(hue.round(), q(r), val.max(0.2));
                        }
                    }
                    (0, _) => {
                        l.grade.temperature = q(u);
                        l.grade.tint = q(v);
                    }
                    (w, _) => {
                        let t = match w {
                            1 => &mut l.grade.shadows,
                            2 => &mut l.grade.midtones,
                            _ => &mut l.grade.highlights,
                        };
                        t.hue = hue.round();
                        t.amount = q(r);
                    }
                }
                if l == *look {
                    Vec::new()
                } else {
                    Self::set_look(l)
                }
            }
            (Mouse::Down, Hit::Swatch(i)) => {
                self.swatch = i;
                if let Some(p) = controls(Tab::Palette).iter().position(|c| *c == C::Swatch) {
                    self.focus[Tab::Palette.index()] = p;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

// ── drawing ─────────────────────────────────────────────────────────────

struct Pal {
    text: Color,
    muted: Color,
    faint: Color,
    accent: Color,
    on_accent: Color,
    hi_bg: Color,
    track: Color,
    live: Color,
    /// the panel's own background, which the wheels blend into
    panel: (u8, u8, u8),
}

fn pal(truecolor: bool) -> Pal {
    if truecolor {
        Pal {
            text: Color::Rgb(220, 224, 234),
            muted: Color::Rgb(146, 153, 170),
            faint: Color::Rgb(88, 94, 110),
            accent: Color::Rgb(122, 196, 236),
            on_accent: Color::Rgb(10, 14, 22),
            hi_bg: Color::Rgb(36, 46, 68),
            track: Color::Rgb(56, 62, 78),
            live: Color::Rgb(126, 212, 146),
            panel: (13, 15, 22),
        }
    } else {
        Pal {
            text: Color::White,
            muted: Color::Gray,
            faint: Color::DarkGray,
            accent: Color::Cyan,
            on_accent: Color::Black,
            hi_bg: Color::Indexed(237),
            track: Color::Indexed(239),
            live: Color::Green,
            panel: (18, 18, 18),
        }
    }
}

fn color(c: (u8, u8, u8), truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(c.0, c.1, c.2)
    } else {
        Color::Indexed(rgb_to_256(c.0, c.1, c.2))
    }
}

fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

fn width(s: &str) -> usize {
    s.chars().count()
}

fn pad(s: &str, w: usize) -> String {
    let n = width(s);
    if n >= w {
        s.chars().take(w).collect()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

fn pad_left(s: &str, w: usize) -> String {
    let n = width(s);
    if n >= w {
        s.chars().take(w).collect()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

/// What a wheel shows at (u, v) on its unit disc, or None outside it (and
/// its ring). `ring` adds the hue-shift ring around the big wheel.
struct WheelPaint {
    /// brightness of the disc (the palette editor previews a colour's value)
    value: f32,
    /// the hue ring's shift, degrees (None: no ring)
    ring: Option<f32>,
    /// pucks, as (u, v) on the disc
    pucks: Vec<(f32, f32)>,
}

impl WheelPaint {
    fn sample(&self, u: f32, v: f32) -> Option<((u8, u8, u8), f32)> {
        let d = (u * u + v * v).sqrt();
        let theta = v.atan2(u).to_degrees();
        if d <= 1.0 {
            let (r, g, b) = hsv(hue_at(theta) / 360.0, d.min(1.0), self.value);
            return Some(((r, g, b), 1.0));
        }
        if let Some(shift) = self.ring {
            if (1.12..=1.3).contains(&d) {
                let (r, g, b) = hsv((hue_at(theta) + shift) / 360.0, 0.9, 0.95);
                return Some(((r, g, b), 1.0));
            }
        }
        None
    }

    /// The puck rings drawn over the disc at (u, v): Some(colour) on a ring.
    fn puck(&self, u: f32, v: f32, r_units: f32) -> Option<(u8, u8, u8)> {
        for &(pu, pv) in &self.pucks {
            let d = ((u - pu).powi(2) + (v - pv).powi(2)).sqrt();
            if d < r_units * 0.45 {
                return Some((20, 22, 30));
            }
            if d < r_units {
                return Some((250, 250, 250));
            }
        }
        None
    }
}

/// A round wheel in half blocks inside `rect`, corrected for the cell
/// shape. Returns (centre x, centre y, x radius, y radius) in cells, for the
/// mouse.
fn draw_wheel(f: &mut Frame, rect: Rect, wp: &WheelPaint, aspect: f32, p: &Pal, truecolor: bool) -> (f32, f32, f32, f32) {
    if rect.width < 4 || rect.height < 2 {
        return (0.0, 0.0, 1.0, 1.0);
    }
    let outer = if wp.ring.is_some() { 1.32 } else { 1.04 };
    // a half-block pixel is one cell wide and aspect/2 cells tall (in widths)
    let px_h = aspect / 2.0;
    let (wpx, hpx) = (rect.width as f32, rect.height as f32 * 2.0);
    // the disc's radius in pixel columns, fitting both ways
    let rx = ((wpx / 2.0 - 0.5) / outer).min((hpx / 2.0 - 0.5) * px_h / outer).max(1.0);
    let ry = rx / px_h;
    let (cx, cy) = (wpx / 2.0, hpx / 2.0);
    let panel = p.panel;
    let puck_r = (2.2 / rx).max(0.07);
    for row in 0..rect.height {
        for col in 0..rect.width {
            let mut pix = [panel; 2];
            for (half, out) in pix.iter_mut().enumerate() {
                // 3x3 supersampling: soft edges at terminal resolution
                let mut acc = [0f32; 3];
                for sy in 0..3 {
                    for sx in 0..3 {
                        let x = col as f32 + (sx as f32 + 0.5) / 3.0;
                        let y = row as f32 * 2.0 + half as f32 + (sy as f32 + 0.5) / 3.0;
                        let (u, v) = ((x - cx) / rx, -(y - cy) / ry);
                        let c = match wp.puck(u, v, puck_r) {
                            Some(pc) => pc,
                            None => wp.sample(u, v).map(|(c, _)| c).unwrap_or(panel),
                        };
                        acc[0] += c.0 as f32;
                        acc[1] += c.1 as f32;
                        acc[2] += c.2 as f32;
                    }
                }
                *out = ((acc[0] / 9.0).round() as u8, (acc[1] / 9.0).round() as u8, (acc[2] / 9.0).round() as u8);
            }
            if let Some(cell) = f.buffer_mut().cell_mut((rect.x + col, rect.y + row)) {
                cell.set_char('▀');
                cell.set_fg(color(pix[0], truecolor));
                cell.set_bg(color(pix[1], truecolor));
            }
        }
    }
    (rect.x as f32 + cx, rect.y as f32 + cy / 2.0, rx, ry / 2.0)
}

fn slider_spans(frac: f32, neutral: f32, w: usize, focused: bool, on: bool, bg: Option<Color>, p: &Pal) -> Vec<Span<'static>> {
    if w == 0 {
        return Vec::new();
    }
    let at = |v: f32| ((v.clamp(0.0, 1.0) * (w - 1) as f32).round() as usize).min(w - 1);
    let (k, n) = (at(frac), at(neutral));
    let (lo, hi) = (k.min(n), k.max(n));
    let fill = if !on {
        p.faint
    } else if focused {
        p.accent
    } else {
        p.muted
    };
    let knob = if focused && on { p.text } else { fill };
    let st = |s: Style| match bg {
        Some(b) => s.bg(b),
        None => s,
    };
    (0..w)
        .map(|i| {
            if i == k {
                Span::styled("●", st(fg(knob)))
            } else if i >= lo && i <= hi {
                Span::styled("━", st(fg(fill)))
            } else {
                Span::styled("─", st(fg(p.track)))
            }
        })
        .collect()
}

/// The studio's panel: the left of the screen, like the menu's drawer.
pub fn panel_rect(area: Rect) -> Rect {
    let w = if area.width < 76 {
        area.width
    } else {
        ((area.width as u32 * 54 / 100) as u16).max(72).min(area.width)
    };
    Rect { width: w, ..area }
}

/// Rows a round wheel `w` cells wide needs (half blocks, this cell shape).
fn wheel_rows(w: u16, aspect: f32, ring: bool) -> u16 {
    let k = if ring { 1.0 } else { 1.04 / 1.32 };
    ((w as f32 * k / aspect.max(0.5)).ceil() as u16).max(3)
}

/// Lay the key hints out on one line, dropping the least important until
/// they fit.
fn keys_line(w: usize, p: &Pal) -> Line<'static> {
    // in order of importance
    let all: [(&str, &str); 7] = [
        ("Esc", "close"),
        ("←→", "adjust"),
        ("↑↓", "control"),
        ("Tab", "page"),
        ("0", "reset"),
        ("u", "undo"),
        ("R", "reset tab"),
    ];
    let cost = |k: &str, a: &str| width(k) + 1 + width(a) + 2;
    let mut keep = all.len();
    while keep > 1 && all[..keep].iter().map(|(k, a)| cost(k, a)).sum::<usize>() > w + 2 {
        keep -= 1;
    }
    // show them in reading order: movement first, close last
    let order = [2usize, 1, 4, 6, 5, 3, 0];
    let mut spans = Vec::new();
    for &i in order.iter().filter(|&&i| i < keep) {
        let (k, a) = all[i];
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(k, bold(p.text)));
        spans.push(Span::styled(format!(" {a}"), fg(p.faint)));
    }
    Line::from(spans)
}

pub fn render(f: &mut Frame, area: Rect, st: &Studio, look: &Look, truecolor: bool, cell_aspect: f32) {
    st.hits.borrow_mut().clear();
    let p = pal(truecolor);
    let full = panel_rect(area);
    if full.width < 30 || full.height < 12 {
        return;
    }
    let ctls = controls(st.tab);
    let focus = st.focus[st.tab.index()].min(ctls.len() - 1);
    let sw = st.swatch.min(look.palette.colors.len().saturating_sub(1));
    let has_strip = st.tab == Tab::Palette && !look.palette.colors.is_empty();
    let ctl_rows = ctls.len() as u16 + if has_strip { 2 } else { 0 };

    // how tall the content wants to be, then a panel that fits it
    let inner_w = full.width.saturating_sub(2);
    let side = inner_w >= 64 && st.tab != Tab::Tones;
    let ring = st.tab == Tab::Colour;
    let (wheel_w, wheel_h) = if st.tab == Tab::Tones {
        let w = (inner_w / 3).saturating_sub(2).min(22);
        (w, wheel_rows(w, cell_aspect, false))
    } else if side {
        let w = (inner_w * 40 / 100).clamp(16, 34);
        (w, wheel_rows(w, cell_aspect, ring))
    } else {
        let w = inner_w.min(28);
        (w, wheel_rows(w, cell_aspect, ring))
    };
    // wheel plus its caption, beside or above the controls
    let body_want = if side {
        (wheel_h + 1).max(ctl_rows)
    } else {
        wheel_h + 1 + 1 + ctl_rows
    };
    let tail = 3 + 1 + 1; // before/after strip, help, keys
    let body_h = body_want.min(full.height.saturating_sub(2 + tail));
    let rect = Rect { height: (body_h + tail + 2).min(full.height), ..full };
    st.hits.borrow_mut().push((area, Hit::Outside));
    st.hits.borrow_mut().push((rect, Hit::Panel));
    let panel_bg = color(p.panel, truecolor);
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if let Some(c) = f.buffer_mut().cell_mut((x, y)) {
                c.reset();
                c.set_bg(panel_bg);
            }
        }
    }
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(p.faint))
        .style(Style::new().bg(panel_bg))
        .title(Line::from(Span::styled(" colour studio ", bold(p.accent))));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    // tabs, right-aligned in the top border
    let mut tx = rect.x + rect.width - 2;
    for t in Tab::ALL.iter().rev() {
        let label = format!(" {} ", t.title());
        let w = width(&label) as u16;
        tx = tx.saturating_sub(w + 1);
        let style = if *t == st.tab {
            Style::new().fg(p.on_accent).bg(p.accent).add_modifier(Modifier::BOLD)
        } else {
            fg(p.muted).bg(panel_bg)
        };
        f.render_widget(Paragraph::new(Line::from(Span::styled(label, style))), Rect::new(tx, rect.y, w, 1));
        st.hits.borrow_mut().push((Rect::new(tx, rect.y, w, 1), Hit::Tab(*t)));
    }

    let body = Rect { height: body_h, ..inner };
    let (wheel_area, ctl_area) = if side {
        let gap = 3;
        (
            Rect { width: wheel_w, ..body },
            Rect { x: body.x + wheel_w + gap, width: body.width.saturating_sub(wheel_w + gap), ..body },
        )
    } else {
        let wh = (wheel_h + 1).min(body_h);
        (
            Rect { height: wh, ..body },
            Rect { y: body.y + wh + 1, height: body_h.saturating_sub(wh + 1), ..body },
        )
    };

    // the wheels, each with its caption right under it
    let caption = |f: &mut Frame, text: &str, x: u16, y: u16, w: u16| {
        if width(text) <= w as usize && y < inner.y + inner.height {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(text.to_string(), fg(p.faint)))).centered(),
                Rect::new(x, y, w, 1),
            );
        }
    };
    match st.tab {
        Tab::Colour | Tab::Palette => {
            let (value, ring, pucks) = match st.tab {
                Tab::Colour => (
                    1.0,
                    Some(look.grade.hue),
                    vec![(look.grade.temperature.clamp(-1.0, 1.0), look.grade.tint.clamp(-1.0, 1.0))],
                ),
                _ => {
                    let (h, s, v) = look.palette.colors.get(sw).copied().map(to_hsv).unwrap_or((0.0, 0.0, 1.0));
                    let a = angle_of(h).to_radians();
                    (v.max(0.25), None, if look.palette.colors.is_empty() { vec![] } else { vec![(s * a.cos(), s * a.sin())] })
                }
            };
            // centre the wheel in its column
            let ww = wheel_area.width.min(wheel_w);
            let wx = wheel_area.x + (wheel_area.width - ww) / 2;
            let wh = wheel_h.min(wheel_area.height.saturating_sub(1));
            let wa = Rect::new(wx, wheel_area.y, ww, wh);
            let (cx, cy, rx, ry) = draw_wheel(f, wa, &WheelPaint { value, ring, pucks }, cell_aspect, &p, truecolor);
            st.hits.borrow_mut().push((wa, Hit::Wheel { which: 0, cx, cy, rx, ry }));
            let text = match st.tab {
                Tab::Colour => "puck: warm → · magenta ↑",
                _ => "hue round · saturation out",
            };
            caption(f, text, wheel_area.x, wa.y + wa.height, wheel_area.width);
        }
        Tab::Tones => {
            let tones = [
                ("Shadows", look.grade.shadows),
                ("Midtones", look.grade.midtones),
                ("Highlights", look.grade.highlights),
            ];
            let col_w = wheel_area.width / 3;
            for (i, (name, tone)) in tones.iter().enumerate() {
                let x = wheel_area.x + col_w * i as u16 + col_w.saturating_sub(wheel_w) / 2;
                let wh = wheel_h.min(wheel_area.height.saturating_sub(1));
                let wr = Rect::new(x, wheel_area.y, wheel_w, wh);
                let a = angle_of(tone.hue).to_radians();
                let wp = WheelPaint { value: 1.0, ring: None, pucks: vec![(tone.amount * a.cos(), tone.amount * a.sin())] };
                let (cx, cy, rx, ry) = draw_wheel(f, wr, &wp, cell_aspect, &p, truecolor);
                st.hits.borrow_mut().push((wr, Hit::Wheel { which: i as u8 + 1, cx, cy, rx, ry }));
                caption(f, name, wheel_area.x + col_w * i as u16, wr.y + wr.height, col_w);
            }
        }
    }

    // the controls
    let w = ctl_area.width as usize;
    let lw = (w * 2 / 5).clamp(9, 15);
    let mut y = ctl_area.y;
    let bottom = ctl_area.y + ctl_area.height;
    for (i, c) in ctls.iter().enumerate() {
        if y >= bottom {
            break;
        }
        // the palette's colours as a strip above the colour editor
        if *c == C::Swatch && has_strip {
            let n = look.palette.colors.len() as u16;
            let cell_w = ((w as u16).saturating_sub(1) / n.max(1)).clamp(2, 5);
            let mut x = ctl_area.x + 1;
            for (k, col) in look.palette.colors.iter().enumerate() {
                let r = Rect::new(x, y, cell_w, 1);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "█".repeat(cell_w as usize - 1),
                        fg(color(col.tuple(), truecolor)).bg(panel_bg),
                    ))),
                    r,
                );
                if k == sw && y + 1 < bottom {
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled("▔".repeat(cell_w as usize - 1), fg(p.text).bg(panel_bg)))),
                        Rect::new(x, y + 1, cell_w, 1),
                    );
                }
                st.hits.borrow_mut().push((r, Hit::Swatch(k)));
                x += cell_w;
            }
            y += 2;
            if y >= bottom {
                break;
            }
        }
        let focused = i == focus;
        let on = enabled(*c, look);
        let bg = focused.then_some(p.hi_bg);
        let row = Rect::new(ctl_area.x, y, ctl_area.width, 1);
        if let Some(b) = bg {
            f.buffer_mut().set_style(row, Style::new().bg(b));
        }
        let stb = |s: Style| match bg {
            Some(b) => s.bg(b),
            None => s.bg(panel_bg),
        };
        let label_style = match (on, focused) {
            (false, _) => fg(p.faint),
            (true, true) => bold(p.text),
            (true, false) if matches!(c, C::AddColour | C::RemoveColour) => fg(p.accent),
            (true, false) => fg(p.text),
        };
        let mut spans = vec![
            Span::styled(if focused { "▌" } else { " " }, stb(fg(p.accent))),
            Span::styled(pad(label(*c), lw), stb(label_style)),
            Span::styled(" ", stb(Style::new())),
        ];
        let vx = ctl_area.x + (lw + 2) as u16;
        let vw = w.saturating_sub(lw + 2);
        let t = text(*c, look, sw);
        let value_style = match (on, focused) {
            (false, _) => fg(p.faint),
            (true, true) => fg(p.accent),
            (true, false) => fg(p.muted),
        };
        match spec(*c) {
            Some(sp) => {
                let tw = 8.min(vw);
                let bw = vw.saturating_sub(tw + 1);
                if bw >= 4 {
                    let v = get(*c, look, sw);
                    let span = (sp.max - sp.min).max(1e-6);
                    let frac = (v - sp.min) / span;
                    let neutral = (sp.neutral - sp.min) / span;
                    spans.extend(slider_spans(frac, neutral, bw, focused, on, Some(bg.unwrap_or(panel_bg)), &p));
                    spans.push(Span::styled(" ", stb(Style::new())));
                    st.hits.borrow_mut().push((Rect::new(vx, y, bw as u16, 1), Hit::Slider { ctl: i, x0: vx, width: bw as u16 }));
                }
                spans.push(Span::styled(pad_left(&t, tw), stb(value_style)));
            }
            None => match c {
                C::Dither => {
                    let (m, s) = if look.palette.dither { ("● ", fg(p.live)) } else { ("○ ", fg(p.faint)) };
                    spans.push(Span::styled(m, stb(s)));
                    spans.push(Span::styled(t, stb(value_style)));
                }
                C::AddColour | C::RemoveColour => {
                    spans.push(Span::styled(if focused { "↵" } else { "" }, stb(fg(p.accent))));
                }
                _ if focused && on => {
                    spans.push(Span::styled("◂ ", stb(fg(p.accent))));
                    spans.push(Span::styled(t, stb(value_style)));
                    spans.push(Span::styled(" ▸", stb(fg(p.accent))));
                }
                _ => spans.push(Span::styled(t, stb(value_style))),
            },
        }
        f.render_widget(Paragraph::new(Line::from(spans)), row);
        st.hits.borrow_mut().push((Rect::new(ctl_area.x, y, (lw + 2) as u16, 1), Hit::Ctl(i)));
        y += 1;
    }

    // before / after reference colours
    let sy = inner.y + body_h;
    let strip_w = inner.width.saturating_sub(8) as usize;
    let per = (strip_w / REFS.len()).clamp(1, 6);
    let row = |label: &str, cols: Vec<(u8, u8, u8)>| {
        let mut spans = vec![Span::styled(pad(label, 8), fg(p.faint))];
        for c in cols {
            spans.push(Span::styled("█".repeat(per), fg(color(c, truecolor))));
        }
        Line::from(spans)
    };
    let before: Vec<_> = REFS.to_vec();
    let after: Vec<_> = REFS.iter().map(|c| graded(*c, look)).collect();
    f.render_widget(
        Paragraph::new(vec![Line::from(""), row("before", before), row("after", after)]),
        Rect::new(inner.x, sy, inner.width, 3),
    );

    // help and keys
    let hy = sy + 3;
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(help(ctls[focus]), fg(p.muted)))),
        Rect::new(inner.x, hy, inner.width, 1),
    );
    f.render_widget(Paragraph::new(keys_line(inner.width as usize, &p)), Rect::new(inner.x, hy + 1, inner.width, 1));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look() -> Look {
        Look::default()
    }

    #[test]
    fn keys_step_the_focused_control_and_tabs_switch() {
        let mut s = Studio::new();
        s.open_on(Tab::Colour);
        let l = look();
        // exposure first: → brightens by one step
        let fx = s.handle(Input::Right, &l);
        let Effect::SetLook(n) = &fx[0] else { panic!("{fx:?}") };
        assert!((n.grade.exposure - 0.1).abs() < 1e-6);
        // ↓ to contrast, 0 does nothing at neutral
        s.handle(Input::Down, &l);
        assert!(s.handle(Input::Char('0'), &l).is_empty());
        // Tab goes to Tones, Esc closes
        s.handle(Input::Tab, &l);
        assert_eq!(s.tab, Tab::Tones);
        s.handle(Input::Esc, &l);
        assert!(!s.open);
    }

    #[test]
    fn hue_wraps_and_ranges_clamp() {
        let mut s = Studio::new();
        s.open_on(Tab::Colour);
        let l = look();
        let hue = controls(Tab::Colour).iter().position(|c| *c == C::Hue).unwrap();
        s.focus[0] = hue;
        let Effect::SetLook(n) = &s.handle(Input::Left, &l)[0] else { panic!() };
        assert_eq!(n.grade.hue, 355.0, "hue wraps round");
        let mut top = look();
        top.grade.exposure = 2.0;
        s.focus[0] = 0;
        assert!(s.handle(Input::Right, &top).is_empty(), "exposure clamps at +2");
    }

    #[test]
    fn palette_presets_modes_and_editing() {
        let mut s = Studio::new();
        s.open_on(Tab::Palette);
        let l = look();
        // the preset row loads a palette and turns mapping on
        let Effect::SetLook(n) = &s.handle(Input::Right, &l)[0] else { panic!() };
        assert_eq!(n.palette.colors, PALETTES[0].1.to_vec());
        assert_eq!(n.palette.mode, PaletteMode::Map);
        // add then remove a colour
        let add = controls(Tab::Palette).iter().position(|c| *c == C::AddColour).unwrap();
        s.focus[2] = add;
        let Effect::SetLook(more) = &s.handle(Input::Enter, n)[0] else { panic!() };
        assert_eq!(more.palette.colors.len(), n.palette.colors.len() + 1);
        s.focus[2] = add + 1;
        let Effect::SetLook(fewer) = &s.handle(Input::Enter, more)[0] else { panic!() };
        assert_eq!(fewer.palette.colors.len(), n.palette.colors.len());
        // editing the selected colour's brightness
        let val = controls(Tab::Palette).iter().position(|c| *c == C::SwatchVal).unwrap();
        s.focus[2] = val;
        s.swatch = 0;
        let Effect::SetLook(brighter) = &s.handle(Input::PageUp, n)[0] else { panic!() };
        assert!(to_hsv(brighter.palette.colors[0]).2 > to_hsv(n.palette.colors[0]).2);
    }

    #[test]
    fn hsv_round_trip() {
        for c in [Rgb(255, 0, 0), Rgb(18, 200, 90), Rgb(40, 40, 40), Rgb(0x7a, 0xa2, 0xf7)] {
            let (h, s, v) = to_hsv(c);
            let back = from_hsv(h, s, v);
            let d = |a: u8, b: u8| (a as i32 - b as i32).abs();
            assert!(d(back.0, c.0) <= 1 && d(back.1, c.1) <= 1 && d(back.2, c.2) <= 1, "{c:?} -> {back:?}");
        }
    }

    #[test]
    fn wheel_orientation_matches_temperature_and_tint() {
        // right is warm (orange), left cool (blue), up magenta, down green
        assert_eq!(hue_at(0.0), 30.0);
        assert_eq!(hue_at(180.0), 210.0);
        assert_eq!(hue_at(90.0), 300.0);
        assert_eq!(hue_at(-90.0), 120.0);
        for h in [0.0f32, 45.0, 200.0, 330.0] {
            assert!((hue_at(angle_of(h)) - h).abs() < 1e-3);
        }
    }

    #[test]
    fn graded_references_follow_the_look() {
        let l = look();
        assert_eq!(graded((100, 150, 200), &l), (100, 150, 200));
        let mut warm = look();
        warm.grade.temperature = 1.0;
        let g = graded((128, 128, 128), &warm);
        assert!(g.0 > g.2, "{g:?}");
    }

    #[test]
    fn renders_at_small_and_large_sizes_without_panicking() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut s = Studio::new();
        let mut l = look();
        l.palette.colors = PALETTES[0].1.to_vec();
        l.palette.mode = PaletteMode::Map;
        for tab in Tab::ALL {
            s.open_on(tab);
            for (w, h) in [(40, 14), (80, 24), (200, 60), (10, 4)] {
                let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                term.draw(|f| render(f, f.area(), &s, &l, true, 2.1)).unwrap();
            }
        }
    }
}
