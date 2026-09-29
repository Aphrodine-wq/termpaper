//! The Look: everything about how a frame is coloured and finished that is
//! not the scene itself. Colour grade, three-way tone wheels, a palette, and
//! the effect stack with strengths. A Theme is a named Look; a link group
//! mirrors its Look across panes.
//!
//! Most of the grade is baked into a 3D lookup table ([`Lut3D`]) that the CPU
//! and the GPU apply identically, so a new colour control is one change here
//! and costs nothing per pixel. Hue, saturation and contrast keep their own
//! bit-exact pass (`color_grade.rs`), applied just before the table.
//!
//! No terminal or GPU dependencies: the website's theme builder mirrors this
//! file (see `examples/theme_vectors.rs` for the shared test vectors).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An sRGB colour, written `#rrggbb` in theme and config files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// `#rgb`, `#rrggbb`, with or without the `#`.
    pub fn parse(s: &str) -> Option<Rgb> {
        let h = s.trim().trim_start_matches('#');
        let v = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
        match h.len() {
            6 => Some(Rgb(v(0, 2)?, v(2, 2)?, v(4, 2)?)),
            3 => {
                let d = |i| v(i, 1).map(|x| x * 17);
                Some(Rgb(d(0)?, d(1)?, d(2)?))
            }
            _ => None,
        }
    }

    pub fn tuple(self) -> (u8, u8, u8) {
        (self.0, self.1, self.2)
    }

    fn unit(self) -> [f64; 3] {
        [self.0 as f64 / 255.0, self.1 as f64 / 255.0, self.2 as f64 / 255.0]
    }
}

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgb::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("not a colour: {s:?} (want #rrggbb)")))
    }
}

// Files only carry what differs from neutral: a theme or config lists the
// handful of controls it moves, not all thirty.
fn is_zero(v: &f32) -> bool {
    v.abs() < 1e-6
}
fn is_one(v: &f32) -> bool {
    (v - 1.0).abs() < 1e-6
}
fn tone_off(t: &Tone) -> bool {
    t.amount < 1e-6
}
fn grade_neutral(g: &Grade) -> bool {
    *g == Grade::default() || {
        let d = Grade::default();
        is_zero(&g.hue)
            && is_one(&g.saturation)
            && is_one(&g.contrast)
            && is_zero(&g.exposure)
            && is_zero(&g.vibrance)
            && is_zero(&g.temperature)
            && is_zero(&g.tint)
            && (g.gamma - d.gamma).abs() < 1e-6
            && is_zero(&g.fade)
            && tone_off(&g.shadows)
            && tone_off(&g.midtones)
            && tone_off(&g.highlights)
            && is_zero(&g.balance)
    }
}
fn palette_unused(p: &Palette) -> bool {
    p.mode == PaletteMode::Off && p.colors.is_empty()
}
fn effects_empty(e: &Effects) -> bool {
    e.stack.is_empty() && e.amounts.is_empty()
}
fn is_false(v: &bool) -> bool {
    !*v
}

/// A colour push for one tonal range: toward `hue` (degrees) by `amount`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tone {
    pub hue: f32,
    /// 0 (off) ..= 1
    pub amount: f32,
}

impl Default for Tone {
    fn default() -> Self {
        Tone { hue: 30.0, amount: 0.0 }
    }
}

/// The colour grade. Neutral values leave the picture untouched.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grade {
    /// hue rotation, degrees (0 = off)
    #[serde(skip_serializing_if = "is_zero")]
    pub hue: f32,
    /// 0 ..= 2.5, 1 = unchanged
    #[serde(skip_serializing_if = "is_one")]
    pub saturation: f32,
    /// 0.5 ..= 2.5, 1 = unchanged
    #[serde(skip_serializing_if = "is_one")]
    pub contrast: f32,
    /// stops, -2 ..= 2
    #[serde(skip_serializing_if = "is_zero")]
    pub exposure: f32,
    /// -1 ..= 1: boosts muted colours more than vivid ones
    #[serde(skip_serializing_if = "is_zero")]
    pub vibrance: f32,
    /// -1 (cool) ..= 1 (warm)
    #[serde(skip_serializing_if = "is_zero")]
    pub temperature: f32,
    /// -1 (green) ..= 1 (magenta)
    #[serde(skip_serializing_if = "is_zero")]
    pub tint: f32,
    /// 0.5 ..= 2, 1 = unchanged; above 1 lifts the midtones
    #[serde(skip_serializing_if = "is_one")]
    pub gamma: f32,
    /// 0 ..= 0.5: lifts the blacks toward grey, a matte film look
    #[serde(skip_serializing_if = "is_zero")]
    pub fade: f32,
    #[serde(skip_serializing_if = "tone_off")]
    pub shadows: Tone,
    #[serde(skip_serializing_if = "tone_off")]
    pub midtones: Tone,
    #[serde(skip_serializing_if = "tone_off")]
    pub highlights: Tone,
    /// -1 ..= 1: where shadows end and highlights begin
    #[serde(skip_serializing_if = "is_zero")]
    pub balance: f32,
}

impl Default for Grade {
    fn default() -> Self {
        Grade {
            hue: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            exposure: 0.0,
            vibrance: 0.0,
            temperature: 0.0,
            tint: 0.0,
            gamma: 1.0,
            fade: 0.0,
            shadows: Tone { hue: 215.0, amount: 0.0 },
            midtones: Tone { hue: 30.0, amount: 0.0 },
            highlights: Tone { hue: 40.0, amount: 0.0 },
            balance: 0.0,
        }
    }
}

/// How the palette recolours the picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaletteMode {
    #[default]
    Off,
    /// brightness mapped onto a gradient through the colours
    Map,
    /// the gradient's hue, the picture's own brightness
    Tint,
    /// every pixel becomes its nearest palette colour (retro)
    Snap,
}

impl PaletteMode {
    pub const ALL: [PaletteMode; 4] = [PaletteMode::Off, PaletteMode::Map, PaletteMode::Tint, PaletteMode::Snap];

    pub fn name(self) -> &'static str {
        match self {
            PaletteMode::Off => "off",
            PaletteMode::Map => "map",
            PaletteMode::Tint => "tint",
            PaletteMode::Snap => "snap",
        }
    }
}

/// A palette of 2 to 8 colours, darkest first by convention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub mode: PaletteMode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<Rgb>,
    /// 0 ..= 1: how far toward the palette (map and tint)
    #[serde(skip_serializing_if = "is_one")]
    pub strength: f32,
    /// snap: ordered dithering between the two nearest colours
    #[serde(skip_serializing_if = "is_false")]
    pub dither: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            mode: PaletteMode::Off,
            colors: Vec::new(),
            strength: 1.0,
            dither: false,
        }
    }
}

pub const MAX_COLORS: usize = 8;

/// The effect stack: names applied in order, each with a strength (1 is
/// the classic look, 0 is off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Effects {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stack: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub amounts: BTreeMap<String, f32>,
}

impl Effects {
    pub fn amount(&self, name: &str) -> f32 {
        self.amounts.get(name).copied().unwrap_or(1.0)
    }

    pub fn set_amount(&mut self, name: &str, v: f32) {
        let v = round2(v.clamp(0.0, 2.0));
        if (v - 1.0).abs() < 1e-4 {
            self.amounts.remove(name);
        } else {
            self.amounts.insert(name.to_string(), v);
        }
    }
}

/// Everything a Theme holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    #[serde(skip_serializing_if = "grade_neutral")]
    pub grade: Grade,
    #[serde(skip_serializing_if = "palette_unused")]
    pub palette: Palette,
    #[serde(skip_serializing_if = "effects_empty")]
    pub effects: Effects,
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

fn finite_or(v: f32, d: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        d
    }
}

impl Look {
    /// A neutral look with just this effect stack (tools and tests).
    pub fn with_effects(stack: &[String]) -> Look {
        let mut l = Look::default();
        l.effects.stack = stack.to_vec();
        l
    }

    /// Clamp every value into range and drop what cannot apply: a theme
    /// file or a peer's message is never trusted to be sane.
    pub fn sanitize(&mut self) {
        let d = Grade::default();
        let g = &mut self.grade;
        g.hue = finite_or(g.hue, 0.0).rem_euclid(360.0);
        g.saturation = finite_or(g.saturation, d.saturation).clamp(0.0, 2.5);
        g.contrast = finite_or(g.contrast, d.contrast).clamp(0.5, 2.5);
        g.exposure = finite_or(g.exposure, 0.0).clamp(-2.0, 2.0);
        g.vibrance = finite_or(g.vibrance, 0.0).clamp(-1.0, 1.0);
        g.temperature = finite_or(g.temperature, 0.0).clamp(-1.0, 1.0);
        g.tint = finite_or(g.tint, 0.0).clamp(-1.0, 1.0);
        g.gamma = finite_or(g.gamma, 1.0).clamp(0.5, 2.0);
        g.fade = finite_or(g.fade, 0.0).clamp(0.0, 0.5);
        g.balance = finite_or(g.balance, 0.0).clamp(-1.0, 1.0);
        for t in [&mut g.shadows, &mut g.midtones, &mut g.highlights] {
            t.hue = finite_or(t.hue, 0.0).rem_euclid(360.0);
            t.amount = finite_or(t.amount, 0.0).clamp(0.0, 1.0);
        }
        let p = &mut self.palette;
        p.colors.truncate(MAX_COLORS);
        p.strength = finite_or(p.strength, 1.0).clamp(0.0, 1.0);
        if p.colors.len() < 2 && p.mode != PaletteMode::Off {
            p.mode = PaletteMode::Off;
        }
        self.effects.stack.retain(|n| !n.trim().is_empty());
        self.effects.stack.truncate(16);
        let amounts = std::mem::take(&mut self.effects.amounts);
        for (k, v) in amounts {
            if self.effects.stack.contains(&k) || crate::filter::FILTER_CYCLE.contains(&k.as_str()) {
                self.effects.set_amount(&k, finite_or(v, 1.0));
            }
        }
    }

    /// True when the lookup table would leave every colour as it is.
    pub fn lut_is_identity(&self) -> bool {
        let g = &self.grade;
        let d = Grade::default();
        g.exposure.abs() < 1e-3
            && g.vibrance.abs() < 1e-3
            && g.temperature.abs() < 1e-3
            && g.tint.abs() < 1e-3
            && (g.gamma - d.gamma).abs() < 1e-3
            && g.fade < 1e-3
            && g.shadows.amount < 1e-3
            && g.midtones.amount < 1e-3
            && g.highlights.amount < 1e-3
            && !matches!(self.palette.mode, PaletteMode::Map | PaletteMode::Tint)
    }

    /// Whether the palette snap pass runs.
    pub fn snaps(&self) -> bool {
        self.palette.mode == PaletteMode::Snap && self.palette.colors.len() >= 2
    }

    /// Hue, saturation and contrast all neutral (their own pass is skipped).
    pub fn basic_grade_is_neutral(&self) -> bool {
        self.grade.hue < 0.5 && (self.grade.saturation - 1.0).abs() < 0.02 && (self.grade.contrast - 1.0).abs() < 0.02
    }

    /// Nothing at all to do after the scene: no grade, no effects.
    pub fn is_neutral(&self) -> bool {
        self.basic_grade_is_neutral() && self.lut_is_identity() && !self.snaps() && self.effects.stack.is_empty()
    }

    /// Everything that changes pixels, hashed: two looks with the same
    /// fingerprint draw the same frame.
    pub fn fingerprint(&self) -> u64 {
        let g = &self.grade;
        let mut h = self.lut_key();
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x100000001b3);
        };
        for v in [g.hue, g.saturation, g.contrast] {
            mix(v.to_bits() as u64);
        }
        mix(self.palette.dither as u64);
        for name in &self.effects.stack {
            for b in name.bytes() {
                mix(b as u64);
            }
            mix(0xff);
            mix(self.effects.amount(name).to_bits() as u64);
        }
        h
    }

    /// Every number rounded to two decimals, so repeated ±0.05 steps store
    /// `0.15`, not `0.15000001`.
    pub fn rounded(&self) -> Look {
        let mut l = self.clone();
        let g = &mut l.grade;
        for v in [
            &mut g.hue,
            &mut g.saturation,
            &mut g.contrast,
            &mut g.exposure,
            &mut g.vibrance,
            &mut g.temperature,
            &mut g.tint,
            &mut g.gamma,
            &mut g.fade,
            &mut g.balance,
            &mut g.shadows.hue,
            &mut g.shadows.amount,
            &mut g.midtones.hue,
            &mut g.midtones.amount,
            &mut g.highlights.hue,
            &mut g.highlights.amount,
            &mut l.palette.strength,
        ] {
            *v = round2(*v);
        }
        for v in l.effects.amounts.values_mut() {
            *v = round2(*v);
        }
        l
    }

    /// Whether anything is set that the four legacy config keys (`filters`,
    /// `hue_shift`, `saturation`, `contrast`) cannot say on their own.
    pub fn has_extras(&self) -> bool {
        let mut basic = Look::default();
        basic.grade.hue = self.grade.hue;
        basic.grade.saturation = self.grade.saturation;
        basic.grade.contrast = self.grade.contrast;
        basic.effects.stack = self.effects.stack.clone();
        basic != *self
    }

    /// A stable 64-bit fingerprint of what the lookup table depends on, so
    /// the GPU re-uploads it only when it changed.
    pub fn lut_key(&self) -> u64 {
        let g = &self.grade;
        let mut h = 0xcbf29ce484222325u64;
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x100000001b3);
        };
        for v in [
            g.exposure,
            g.vibrance,
            g.temperature,
            g.tint,
            g.gamma,
            g.fade,
            g.balance,
            g.shadows.hue,
            g.shadows.amount,
            g.midtones.hue,
            g.midtones.amount,
            g.highlights.hue,
            g.highlights.amount,
            self.palette.strength,
        ] {
            mix(v.to_bits() as u64);
        }
        mix(self.palette.mode as u64);
        for c in &self.palette.colors {
            mix(((c.0 as u64) << 16) | ((c.1 as u64) << 8) | c.2 as u64);
        }
        h
    }
}

/// A look ready to render: sanitised, with its lookup table built once
/// (tables are rebuilt only when an input to them changed).
#[derive(Clone, Debug, Default)]
pub struct Baked {
    pub look: std::sync::Arc<Look>,
    pub lut: Option<std::sync::Arc<Lut3D>>,
    key: Option<u64>,
}

impl Baked {
    pub fn new(look: Look) -> Self {
        let mut b = Baked::default();
        b.set(look);
        b
    }

    /// Replace the look, rebuilding the table only if its inputs changed.
    pub fn set(&mut self, mut look: Look) {
        look.sanitize();
        let key = look.lut_key();
        if self.key != Some(key) {
            self.lut = look.lut().map(std::sync::Arc::new);
            self.key = Some(key);
        }
        self.look = std::sync::Arc::new(look);
    }

    pub fn get(&self) -> &Look {
        &self.look
    }
}

// ── the lookup table ────────────────────────────────────────────────────

/// Grid points per axis. 33 is the usual size for grading LUTs: fine enough
/// that tetrahedral interpolation stays within one 8-bit step of the exact
/// transform for every control here.
pub const LUT_N: usize = 33;

/// A 3D colour table: `LUT_N³` entries, red fastest, each packed as three
/// 10-bit channels (`r | g << 10 | b << 20`).
#[derive(Clone, Debug, PartialEq)]
pub struct Lut3D {
    pub data: Vec<u32>,
}

fn pack10(c: [f64; 3]) -> u32 {
    let q = |v: f64| ((v.clamp(0.0, 1.0) * 1023.0).round() as u32).min(1023);
    q(c[0]) | (q(c[1]) << 10) | (q(c[2]) << 20)
}

fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f64) -> f64 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn luma(c: [f64; 3]) -> f64 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Fully saturated colour for a hue in degrees, as sRGB 0..1.
fn hue_rgb(deg: f64) -> [f64; 3] {
    let h = (deg / 60.0).rem_euclid(6.0);
    let x = 1.0 - ((h % 2.0) - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}

/// The palette's gradient at `t` in 0..1, colours evenly spaced.
fn gradient(colors: &[Rgb], t: f64) -> [f64; 3] {
    let n = colors.len();
    if n == 1 {
        return colors[0].unit();
    }
    let x = t.clamp(0.0, 1.0) * (n - 1) as f64;
    let i = (x.floor() as usize).min(n - 2);
    let f = x - i as f64;
    let (a, b) = (colors[i].unit(), colors[i + 1].unit());
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]
}

impl Look {
    /// The exact transform the table samples, for one sRGB colour (0..1).
    /// Order: exposure and white balance in linear light, then gamma,
    /// vibrance, the tone wheels, the fade, and the palette.
    pub fn transform(&self, c: [f64; 3]) -> [f64; 3] {
        let g = &self.grade;
        // exposure and white balance, in linear light
        let mut lin = [srgb_to_linear(c[0]), srgb_to_linear(c[1]), srgb_to_linear(c[2])];
        let ev = 2f64.powf(g.exposure as f64);
        let t = g.temperature as f64;
        let m = g.tint as f64;
        let gains = [1.0 + 0.22 * t + 0.06 * m, 1.0 - 0.14 * m, 1.0 - 0.22 * t + 0.06 * m];
        // keep brightness: normalise the gains by their own luminance
        let norm = luma(gains).max(1e-6);
        for k in 0..3 {
            lin[k] = (lin[k] * ev * gains[k] / norm).max(0.0);
        }
        let mut s = [
            linear_to_srgb(lin[0].min(1.0)),
            linear_to_srgb(lin[1].min(1.0)),
            linear_to_srgb(lin[2].min(1.0)),
        ];
        // gamma on the midtones
        let gm = g.gamma as f64;
        if (gm - 1.0).abs() > 1e-6 {
            for v in &mut s {
                *v = v.max(0.0).powf(1.0 / gm);
            }
        }
        // vibrance: saturate muted colours more than vivid ones
        let vib = g.vibrance as f64;
        if vib.abs() > 1e-6 {
            let l = luma(s);
            let hi = s[0].max(s[1]).max(s[2]);
            let lo = s[0].min(s[1]).min(s[2]);
            let sat = if hi > 1e-6 { (hi - lo) / hi } else { 0.0 };
            let k = 1.0 + vib * (1.0 - sat);
            for v in &mut s {
                *v = l + (*v - l) * k;
            }
        }
        // three-way tone wheels: a chroma push (no brightness change) per range
        let pivot = (0.5 + 0.35 * g.balance as f64).clamp(0.1, 0.9);
        let l = luma(s).clamp(0.0, 1.0);
        let lo_w = ((pivot - l) / pivot).clamp(0.0, 1.0).powi(2);
        let hi_w = ((l - pivot) / (1.0 - pivot)).clamp(0.0, 1.0).powi(2);
        let mid_w = (1.0 - lo_w - hi_w).clamp(0.0, 1.0) * (4.0 * l * (1.0 - l)).clamp(0.0, 1.0);
        for (tone, w) in [(g.shadows, lo_w), (g.midtones, mid_w), (g.highlights, hi_w)] {
            if tone.amount > 1e-6 && w > 0.0 {
                let h = hue_rgb(tone.hue as f64);
                let hl = luma(h);
                let a = tone.amount as f64 * w * 0.35;
                for k in 0..3 {
                    s[k] += (h[k] - hl) * a;
                }
            }
        }
        // fade: lift the blacks
        let f = g.fade as f64;
        if f > 1e-6 {
            for v in &mut s {
                *v = f + (1.0 - f) * *v;
            }
        }
        // palette map / tint
        let p = &self.palette;
        if p.colors.len() >= 2 && matches!(p.mode, PaletteMode::Map | PaletteMode::Tint) {
            let l = luma(s).clamp(0.0, 1.0);
            let mut target = gradient(&p.colors, l);
            if p.mode == PaletteMode::Tint {
                // the gradient's colour at the picture's own brightness
                let tl = luma(target);
                let k = if tl > 1e-4 { l / tl } else { 0.0 };
                for v in &mut target {
                    *v = (*v * k).min(1.0);
                }
                // a gradient too dark to lift: fall back toward grey
                if tl <= 1e-4 {
                    target = [l, l, l];
                }
            }
            let st = p.strength as f64;
            for k in 0..3 {
                s[k] += (target[k] - s[k]) * st;
            }
        }
        [s[0].clamp(0.0, 1.0), s[1].clamp(0.0, 1.0), s[2].clamp(0.0, 1.0)]
    }

    /// The lookup table for this look, or None when it would change nothing.
    pub fn lut(&self) -> Option<Lut3D> {
        if self.lut_is_identity() {
            return None;
        }
        let n = LUT_N;
        let mut data = Vec::with_capacity(n * n * n);
        let step = 1.0 / (n - 1) as f64;
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    data.push(pack10(self.transform([r as f64 * step, g as f64 * step, b as f64 * step])));
                }
            }
        }
        Some(Lut3D { data })
    }
}

impl Lut3D {
    /// The table that maps every colour to itself (tests, and the website).
    pub fn identity() -> Lut3D {
        let n = LUT_N;
        let step = 1.0 / (n - 1) as f64;
        let mut data = Vec::with_capacity(n * n * n);
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    data.push(pack10([r as f64 * step, g as f64 * step, b as f64 * step]));
                }
            }
        }
        Lut3D { data }
    }

    fn node(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        let v = self.data[(b * LUT_N + g) * LUT_N + r];
        [(v & 1023) as f32, ((v >> 10) & 1023) as f32, ((v >> 20) & 1023) as f32]
    }

    /// One colour through the table, tetrahedral interpolation. The GPU's
    /// `look_lut` pass does exactly this arithmetic in f32.
    pub fn apply(&self, c: (u8, u8, u8)) -> (u8, u8, u8) {
        let scale = (LUT_N - 1) as f32 / 255.0;
        let axis = |v: u8| {
            let f = v as f32 * scale;
            let i = (f.floor() as usize).min(LUT_N - 2);
            (i, f - i as f32)
        };
        let (ri, fr) = axis(c.0);
        let (gi, fg) = axis(c.1);
        let (bi, fb) = axis(c.2);
        let c000 = self.node(ri, gi, bi);
        let c111 = self.node(ri + 1, gi + 1, bi + 1);
        // the tetrahedron containing the point, by the order of its fractions
        let (w, ca, cb) = if fr > fg {
            if fg > fb {
                ([1.0 - fr, fr - fg, fg - fb, fb], self.node(ri + 1, gi, bi), self.node(ri + 1, gi + 1, bi))
            } else if fr > fb {
                ([1.0 - fr, fr - fb, fb - fg, fg], self.node(ri + 1, gi, bi), self.node(ri + 1, gi, bi + 1))
            } else {
                ([1.0 - fb, fb - fr, fr - fg, fg], self.node(ri, gi, bi + 1), self.node(ri + 1, gi, bi + 1))
            }
        } else if fb > fg {
            ([1.0 - fb, fb - fg, fg - fr, fr], self.node(ri, gi, bi + 1), self.node(ri, gi + 1, bi + 1))
        } else if fb > fr {
            ([1.0 - fg, fg - fb, fb - fr, fr], self.node(ri, gi + 1, bi), self.node(ri, gi + 1, bi + 1))
        } else {
            ([1.0 - fg, fg - fr, fr - fb, fb], self.node(ri, gi + 1, bi), self.node(ri + 1, gi + 1, bi))
        };
        let ch = |k: usize| {
            let v = w[0] * c000[k] + w[1] * ca[k] + w[2] * cb[k] + w[3] * c111[k];
            ((v * (255.0 / 1023.0) + 0.5).floor()).clamp(0.0, 255.0) as u8
        };
        (ch(0), ch(1), ch(2))
    }
}

/// Apply a look's table to a canvas (glyph cells lose their glyph, as with
/// every colour pass).
pub fn apply_lut(canvas: &mut crate::canvas::Canvas, lut: &Lut3D) {
    canvas.map_colors(|c| lut.apply(c));
}

// ── palette snap ────────────────────────────────────────────────────────

/// 4x4 Bayer matrix, 0..16.
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Squared distance with a little weight on green, where the eye is keenest.
fn dist2(a: (u8, u8, u8), b: Rgb) -> i32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    2 * dr * dr + 4 * dg * dg + 3 * db * db
}

/// The palette colour for one pixel at global position (gx, gy): the
/// nearest colour, or with `dither` the two nearest mixed by a Bayer
/// threshold. Global coordinates keep the pattern seamless across a wall.
pub fn snap_pixel(c: (u8, u8, u8), colors: &[Rgb], dither: bool, gx: i32, gy: i32) -> (u8, u8, u8) {
    let mut best = (i32::MAX, 0usize);
    let mut second = (i32::MAX, 0usize);
    for (i, &p) in colors.iter().enumerate() {
        let d = dist2(c, p);
        if d < best.0 {
            second = best;
            best = (d, i);
        } else if d < second.0 {
            second = (d, i);
        }
    }
    if !dither || second.0 == i32::MAX {
        return colors[best.1].tuple();
    }
    // how far toward the second colour this pixel sits, 0..0.5
    let (d1, d2) = ((best.0 as f32).sqrt(), (second.0 as f32).sqrt());
    let t = if d1 + d2 > 0.0 { d1 / (d1 + d2) } else { 0.0 };
    let threshold = (BAYER4[gy.rem_euclid(4) as usize][gx.rem_euclid(4) as usize] as f32 + 0.5) / 16.0;
    if t > threshold {
        colors[second.1].tuple()
    } else {
        colors[best.1].tuple()
    }
}

/// Snap a canvas to its palette. `origin` is the canvas's (0, 0) in global
/// wall pixels.
pub fn apply_snap(canvas: &mut crate::canvas::Canvas, palette: &Palette, origin: (i32, i32)) {
    if palette.colors.len() < 2 {
        return;
    }
    let w = canvas.width();
    let cells = canvas.cells_raw_mut();
    for (i, cell) in cells.iter_mut().enumerate() {
        let (x, y) = ((i % w) as i32 + origin.0, (i / w) as i32 + origin.1);
        cell.color = snap_pixel(cell.color, &palette.colors, palette.dither, x, y);
        cell.ch = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: i32) -> bool {
        (a.0 as i32 - b.0 as i32).abs() <= tol
            && (a.1 as i32 - b.1 as i32).abs() <= tol
            && (a.2 as i32 - b.2 as i32).abs() <= tol
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(Rgb::parse("#1a1b26"), Some(Rgb(0x1a, 0x1b, 0x26)));
        assert_eq!(Rgb::parse("fff"), Some(Rgb(255, 255, 255)));
        assert_eq!(Rgb::parse("#12345"), None);
        assert_eq!(Rgb::parse("#gggggg"), None);
        assert_eq!(Rgb(1, 2, 255).hex(), "#0102ff");
    }

    #[test]
    fn a_neutral_look_builds_no_table() {
        let l = Look::default();
        assert!(l.lut_is_identity() && l.is_neutral());
        assert!(l.lut().is_none());
        // hue/sat/contrast have their own pass: still no table
        let mut l2 = Look::default();
        l2.grade.saturation = 1.5;
        assert!(l2.lut().is_none() && !l2.is_neutral());
    }

    #[test]
    fn the_identity_table_is_within_one_step_everywhere() {
        let lut = Lut3D::identity();
        for r in (0..=255).step_by(5) {
            for g in (0..=255).step_by(7) {
                for b in (0..=255).step_by(11) {
                    let c = (r as u8, g as u8, b as u8);
                    assert!(close(lut.apply(c), c, 1), "{c:?} -> {:?}", lut.apply(c));
                }
            }
        }
        assert_eq!(lut.apply((0, 0, 0)), (0, 0, 0));
        assert_eq!(lut.apply((255, 255, 255)), (255, 255, 255));
    }

    #[test]
    fn the_table_tracks_the_exact_transform() {
        // strong settings on every control at once
        let mut l = Look::default();
        l.grade.exposure = 0.7;
        l.grade.temperature = 0.6;
        l.grade.tint = -0.3;
        l.grade.vibrance = 0.5;
        l.grade.gamma = 1.3;
        l.grade.fade = 0.1;
        l.grade.shadows = Tone { hue: 210.0, amount: 0.6 };
        l.grade.highlights = Tone { hue: 35.0, amount: 0.5 };
        let lut = l.lut().unwrap();
        let mut worst = 0;
        for r in (0..=255).step_by(9) {
            for g in (0..=255).step_by(13) {
                for b in (0..=255).step_by(17) {
                    let c = (r as u8, g as u8, b as u8);
                    let e = l.transform([r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]);
                    let exact = ((e[0] * 255.0).round() as i32, (e[1] * 255.0).round() as i32, (e[2] * 255.0).round() as i32);
                    let got = lut.apply(c);
                    worst = worst
                        .max((got.0 as i32 - exact.0).abs())
                        .max((got.1 as i32 - exact.1).abs())
                        .max((got.2 as i32 - exact.2).abs());
                }
            }
        }
        assert!(worst <= 3, "table strays {worst} steps from the transform");
    }

    #[test]
    fn controls_move_colour_the_right_way() {
        let lum = |c: (u8, u8, u8)| c.0 as i32 * 2 + c.1 as i32 * 5 + c.2 as i32;
        let grey = (120, 120, 120);
        let with = |f: &dyn Fn(&mut Look)| {
            let mut l = Look::default();
            f(&mut l);
            l.lut().unwrap().apply(grey)
        };
        assert!(lum(with(&|l| l.grade.exposure = 1.0)) > lum(grey) + 100);
        assert!(lum(with(&|l| l.grade.exposure = -1.0)) < lum(grey) - 100);
        let warm = with(&|l| l.grade.temperature = 1.0);
        assert!(warm.0 > warm.2 + 20, "warm is redder: {warm:?}");
        let cool = with(&|l| l.grade.temperature = -1.0);
        assert!(cool.2 > cool.0 + 20, "cool is bluer: {cool:?}");
        let magenta = with(&|l| l.grade.tint = 1.0);
        assert!(magenta.1 < magenta.0 && magenta.1 < magenta.2, "{magenta:?}");
        let faded = {
            let mut l = Look::default();
            l.grade.fade = 0.2;
            l.lut().unwrap().apply((0, 0, 0))
        };
        assert!(faded.0 > 40, "fade lifts black: {faded:?}");
        // a shadow tint colours dark greys but leaves white alone
        let mut l = Look::default();
        l.grade.shadows = Tone { hue: 220.0, amount: 1.0 };
        let lut = l.lut().unwrap();
        let dark = lut.apply((40, 40, 40));
        assert!(dark.2 > dark.0 + 3, "blue shadows: {dark:?}");
        assert!(close(lut.apply((255, 255, 255)), (255, 255, 255), 1));
    }

    #[test]
    fn palette_map_runs_black_to_first_and_white_to_last() {
        let mut l = Look::default();
        l.palette = Palette {
            mode: PaletteMode::Map,
            colors: vec![Rgb(0x1a, 0x1b, 0x26), Rgb(0x7a, 0xa2, 0xf7), Rgb(0xc0, 0xca, 0xf5)],
            strength: 1.0,
            dither: false,
        };
        let lut = l.lut().unwrap();
        assert!(close(lut.apply((0, 0, 0)), (0x1a, 0x1b, 0x26), 1));
        assert!(close(lut.apply((255, 255, 255)), (0xc0, 0xca, 0xf5), 1));
        // half strength sits between the picture and the palette
        l.palette.strength = 0.5;
        let half = l.lut().unwrap().apply((255, 255, 255));
        assert!(half.0 > 0xc0 && half.0 < 255, "{half:?}");
        // tint keeps brightness: white stays near white
        l.palette.mode = PaletteMode::Tint;
        l.palette.strength = 1.0;
        let t = l.lut().unwrap().apply((128, 128, 128));
        let lum = |c: (u8, u8, u8)| 0.2126 * c.0 as f64 + 0.7152 * c.1 as f64 + 0.0722 * c.2 as f64;
        assert!((lum(t) - 128.0).abs() < 14.0, "tint keeps luminance: {t:?}");
    }

    #[test]
    fn snap_picks_the_nearest_colour_and_dithers_between_two() {
        let pal = [Rgb(0, 0, 0), Rgb(255, 255, 255)];
        assert_eq!(snap_pixel((30, 30, 30), &pal, false, 0, 0), (0, 0, 0));
        assert_eq!(snap_pixel((220, 220, 220), &pal, false, 0, 0), (255, 255, 255));
        // mid grey dithers: both colours appear over a 4x4 tile
        let mut seen = std::collections::HashSet::new();
        for y in 0..4 {
            for x in 0..4 {
                seen.insert(snap_pixel((128, 128, 128), &pal, true, x, y));
            }
        }
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn sanitize_clamps_and_disables_what_cannot_apply() {
        let mut l = Look::default();
        l.grade.exposure = 99.0;
        l.grade.hue = -30.0;
        l.grade.gamma = f32::NAN;
        l.palette.mode = PaletteMode::Map;
        l.palette.colors = vec![Rgb(1, 2, 3)];
        l.effects.stack = vec!["bloom".into(), " ".into()];
        l.effects.amounts.insert("bloom".into(), 7.0);
        l.effects.amounts.insert("nonsense".into(), 0.3);
        l.sanitize();
        assert_eq!(l.grade.exposure, 2.0);
        assert_eq!(l.grade.hue, 330.0);
        assert_eq!(l.grade.gamma, 1.0);
        assert_eq!(l.palette.mode, PaletteMode::Off, "one colour is not a palette");
        assert_eq!(l.effects.stack, vec!["bloom"]);
        assert_eq!(l.effects.amount("bloom"), 2.0);
        assert!(!l.effects.amounts.contains_key("nonsense"));
    }

    #[test]
    fn toml_round_trip_and_defaults() {
        let mut l = Look::default();
        l.grade.temperature = -0.15;
        l.grade.shadows = Tone { hue: 230.0, amount: 0.2 };
        l.palette.mode = PaletteMode::Map;
        l.palette.colors = vec![Rgb(0x1a, 0x1b, 0x26), Rgb(0xc0, 0xca, 0xf5)];
        l.effects.stack = vec!["bloom".into(), "vignette".into()];
        l.effects.set_amount("bloom", 0.6);
        let text = toml::to_string(&l).unwrap();
        assert!(text.contains("\"#1a1b26\""), "{text}");
        let back: Look = toml::from_str(&text).unwrap();
        assert_eq!(back, l);
        // a sparse file fills in every default
        let sparse: Look = toml::from_str("[grade]\nvibrance = 0.3\n").unwrap();
        assert_eq!(sparse.grade.vibrance, 0.3);
        assert_eq!(sparse.grade.gamma, 1.0);
        assert_eq!(sparse.palette.mode, PaletteMode::Off);
    }

    #[test]
    fn lut_key_changes_with_the_table_inputs_only() {
        let a = Look::default();
        let mut b = Look::default();
        b.grade.saturation = 2.0; // own pass: not in the table
        assert_eq!(a.lut_key(), b.lut_key());
        b.grade.exposure = 0.5;
        assert_ne!(a.lut_key(), b.lut_key());
    }
}
