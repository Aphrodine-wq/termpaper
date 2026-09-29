//! Playback and display preferences beyond the classic flat config keys.
//! Each group is one struct, kept as its own table in config.toml
//! (`[playback]`, `[display]`), changed from the menu with one effect, and
//! written back with only the values that differ from the defaults.
//!
//! ```toml
//! [playback]
//! order = "in-order"
//! transition = "iris"
//!
//! [display]
//! clock_style = "large"
//! night = true
//! night_level = 0.4
//! ```
//!
//! Every choice reads leniently: a value this build does not know (a
//! newer config) falls back to the default instead of failing the file.

use serde::{Deserialize, Serialize};

/// An enum of named choices: config spelling, menu label, `ALL` in menu
/// order (the first is the default), lenient serde through the spelling.
macro_rules! choice {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident => $s:literal, $label:literal;)+ }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vm])* $v,)+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),+];

            /// As config.toml spells it.
            pub fn name(self) -> &'static str {
                match self {
                    $($name::$v => $s,)+
                }
            }

            /// As the menu shows it.
            pub fn label(self) -> &'static str {
                match self {
                    $($name::$v => $label,)+
                }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s {
                    $($s => Some($name::$v),)+
                    _ => None,
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::ALL[0]
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.name())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                // numbers are accepted too: `unfocused = 30`
                let v = toml::Value::deserialize(d)?;
                let s = match &v {
                    toml::Value::String(s) => s.clone(),
                    toml::Value::Integer(i) => i.to_string(),
                    _ => String::new(),
                };
                Ok($name::parse(&s).unwrap_or_default())
            }
        }
    };
}

choice!(
    /// The order the auto-cycle takes scenes in.
    CycleOrder {
        Shuffle => "shuffle", "shuffle";
        InOrder => "in-order", "in order";
    }
);

choice!(
    /// How one scene gives way to the next.
    TransitionStyle {
        Fade => "fade", "fade";
        Dissolve => "dissolve", "dissolve";
        Wipe => "wipe", "wipe";
        Iris => "iris", "iris";
        Blinds => "blinds", "blinds";
    }
);

choice!(
    /// The scene a fresh start opens on.
    OnLaunch {
        Last => "last", "last scene";
        Favorite => "favorite", "random favourite";
        Random => "random", "random scene";
    }
);

choice!(
    /// Colour output: what the terminal is told.
    Colors {
        Auto => "auto", "auto";
        Truecolor => "truecolor", "24-bit";
        Ansi256 => "256", "256 colours";
    }
);

choice!(
    /// What happens while the terminal is not focused.
    Unfocused {
        Keep => "keep", "keep going";
        Fps30 => "30", "30 fps";
        Fps15 => "15", "15 fps";
        Pause => "pause", "pause";
    }
);

choice!(
    /// What happens on battery power.
    Battery {
        Save => "save", "save power";
        Keep => "keep", "full speed";
    }
);

choice!(
    /// How much the terminal is sent per frame: small colour changes can be
    /// skipped so busy scenes cost less to draw.
    Bandwidth {
        Balanced => "balanced", "balanced";
        Full => "full", "full";
        Light => "light", "light";
    }
);

choice!(
    /// Which GPU draws Studio scenes (takes effect on the next start of the
    /// renderer).
    GpuChoice {
        Auto => "auto", "auto";
        Integrated => "integrated", "integrated";
        Discrete => "discrete", "discrete";
    }
);

choice!(
    /// The clock's size.
    ClockStyle {
        Small => "small", "small";
        Large => "large", "large";
    }
);

choice!(
    /// What the clock says.
    ClockFormat {
        H24 => "24h", "24-hour";
        H12 => "12h", "12-hour";
        Seconds => "seconds", "with seconds";
        Date => "date", "with date";
    }
);

choice!(
    /// Where an overlay sits.
    Corner {
        TopRight => "top-right", "top right";
        TopLeft => "top-left", "top left";
        BottomRight => "bottom-right", "bottom right";
        BottomLeft => "bottom-left", "bottom left";
        Center => "center", "centre";
    }
);

choice!(
    /// How linked panes share the picture.
    WallMode {
        Auto => "auto", "auto";
        Grid => "grid", "grid";
        Off => "off", "off";
    }
);

/// Manual wall grids the Grid row steps through, (columns, rows).
pub const GRIDS: &[(u8, u8)] = &[(2, 1), (1, 2), (3, 1), (2, 2), (4, 1), (3, 2), (4, 2), (3, 3)];

/// The Wall page's layout settings, as the menu shows and changes them.
/// They live in the config's flat keys (`wall`, `wall_grid`, `pad`,
/// `placement`, `sync_look`) and the desk's bezel.
#[derive(Clone, Debug, PartialEq)]
pub struct WallPrefs {
    pub mode: WallMode,
    /// the manual grid: columns, rows, and this pane's cell (row by row)
    pub grid: (u8, u8, u8),
    /// the terminal's inner padding, px
    pub pad: f32,
    pub placement: crate::wall::Placement,
    /// monitor bezels, mm
    pub bezel_mm: f32,
    /// take the group's look (theme, grade, effects)
    pub sync_look: bool,
}

impl Default for WallPrefs {
    fn default() -> Self {
        WallPrefs {
            mode: WallMode::Auto,
            grid: (2, 1, 0),
            pad: 0.0,
            placement: crate::wall::Placement::TopLeft,
            bezel_mm: 0.0,
            sync_look: true,
        }
    }
}

impl WallPrefs {
    /// The grid as `--wall` spells it: `COLSxROWS:INDEX`.
    pub fn grid_spec(&self) -> String {
        let (c, r, i) = self.grid;
        format!("{c}x{r}:{i}")
    }

    /// Read `COLSxROWS:INDEX`.
    pub fn parse_grid(spec: &str) -> Option<(u8, u8, u8)> {
        let (g, i) = spec.split_once(':')?;
        let (c, r) = g.split_once('x')?;
        let (c, r, i) = (c.trim().parse::<u8>().ok()?, r.trim().parse::<u8>().ok()?, i.trim().parse::<u8>().ok()?);
        (c > 0 && r > 0 && (i as u16) < c as u16 * r as u16).then_some((c, r, i))
    }
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn is_true(v: &bool) -> bool {
    *v
}

/// How scenes follow one another.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackPrefs {
    #[serde(skip_serializing_if = "is_default")]
    pub order: CycleOrder,
    #[serde(skip_serializing_if = "is_default")]
    pub transition: TransitionStyle,
    /// switch the scene's variant with the time of day (morning, day,
    /// evening, night) where it has such variants
    #[serde(skip_serializing_if = "is_default")]
    pub time_of_day: bool,
    #[serde(skip_serializing_if = "is_default")]
    pub on_launch: OnLaunch,
}

/// How frames reach the terminal, and what is drawn over them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayPrefs {
    #[serde(skip_serializing_if = "is_default")]
    pub colors: Colors,
    /// lower the frame rate while the terminal cannot keep up, and raise it
    /// again when it can
    #[serde(skip_serializing_if = "is_true")]
    pub adapt_fps: bool,
    #[serde(skip_serializing_if = "is_default")]
    pub unfocused: Unfocused,
    #[serde(skip_serializing_if = "is_default")]
    pub battery: Battery,
    #[serde(skip_serializing_if = "is_default")]
    pub bandwidth: Bandwidth,
    #[serde(skip_serializing_if = "is_default")]
    pub gpu: GpuChoice,
    #[serde(skip_serializing_if = "is_default")]
    pub clock_style: ClockStyle,
    #[serde(skip_serializing_if = "is_default")]
    pub clock_format: ClockFormat,
    #[serde(skip_serializing_if = "is_default")]
    pub clock_corner: Corner,
    /// the scene's name for a moment after it changes
    #[serde(skip_serializing_if = "is_default")]
    pub caption: bool,
    /// frame rate, frame time and output size in a corner
    #[serde(skip_serializing_if = "is_default")]
    pub hud: bool,
    /// clicks and the wheel work the menu
    #[serde(skip_serializing_if = "is_true")]
    pub mouse: bool,
    /// cell height over width, when the terminal does not say (None: ask
    /// it, else assume 2)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_aspect: Option<f32>,
    /// dim between `night_from` and `night_to` (hours, local time)
    #[serde(skip_serializing_if = "is_default")]
    pub night: bool,
    #[serde(skip_serializing_if = "is_night_from")]
    pub night_from: u8,
    #[serde(skip_serializing_if = "is_night_to")]
    pub night_to: u8,
    /// brightness at night, 0.1 ..= 1
    #[serde(skip_serializing_if = "is_night_level")]
    pub night_level: f32,
    /// GPU milliseconds a Studio scene may spend per frame before its
    /// quality steps down
    #[serde(skip_serializing_if = "is_studio_budget")]
    pub studio_budget_ms: f32,
    /// frame-rate cap while a Studio scene shows
    #[serde(skip_serializing_if = "is_studio_fps")]
    pub studio_fps: u32,
}

pub const STUDIO_BUDGET_MS: f32 = 3.0;
pub const STUDIO_FPS: u32 = 60;

fn is_studio_budget(v: &f32) -> bool {
    (*v - STUDIO_BUDGET_MS).abs() < 1e-6
}

fn is_studio_fps(v: &u32) -> bool {
    *v == STUDIO_FPS
}

pub const NIGHT_FROM: u8 = 22;
pub const NIGHT_TO: u8 = 7;
pub const NIGHT_LEVEL: f32 = 0.5;

fn is_night_from(v: &u8) -> bool {
    *v == NIGHT_FROM
}

fn is_night_to(v: &u8) -> bool {
    *v == NIGHT_TO
}

fn is_night_level(v: &f32) -> bool {
    (*v - NIGHT_LEVEL).abs() < 1e-6
}

impl Default for DisplayPrefs {
    fn default() -> Self {
        DisplayPrefs {
            colors: Colors::Auto,
            adapt_fps: true,
            unfocused: Unfocused::Keep,
            battery: Battery::Save,
            bandwidth: Bandwidth::Balanced,
            gpu: GpuChoice::Auto,
            clock_style: ClockStyle::Small,
            clock_format: ClockFormat::H24,
            clock_corner: Corner::TopRight,
            caption: false,
            hud: false,
            mouse: true,
            cell_aspect: None,
            night: false,
            night_from: NIGHT_FROM,
            night_to: NIGHT_TO,
            night_level: NIGHT_LEVEL,
            studio_budget_ms: STUDIO_BUDGET_MS,
            studio_fps: STUDIO_FPS,
        }
    }
}

impl DisplayPrefs {
    /// Clamp what a hand-edited file may have out of range.
    pub fn sanitize(&mut self) {
        self.night_from %= 24;
        self.night_to %= 24;
        if !self.night_level.is_finite() {
            self.night_level = NIGHT_LEVEL;
        }
        self.night_level = self.night_level.clamp(0.1, 1.0);
        self.cell_aspect = self.cell_aspect.filter(|a| a.is_finite() && (0.8..=4.0).contains(a));
        if !self.studio_budget_ms.is_finite() {
            self.studio_budget_ms = STUDIO_BUDGET_MS;
        }
        self.studio_budget_ms = self.studio_budget_ms.clamp(0.5, 16.0);
        self.studio_fps = self.studio_fps.clamp(10, 240);
    }

    /// Take a peer's settings for what the whole wall shows alike (the
    /// clock, the scene name, night dimming), keeping this pane's own
    /// (colours, power, mouse, GPU…).
    pub fn adopt_shared(&mut self, peer: &DisplayPrefs) {
        self.clock_style = peer.clock_style;
        self.clock_format = peer.clock_format;
        self.clock_corner = peer.clock_corner;
        self.caption = peer.caption;
        self.night = peer.night;
        self.night_from = peer.night_from;
        self.night_to = peer.night_to;
        self.night_level = peer.night_level;
    }

    /// Whether night dimming applies at `hour` (0-23, local): the window
    /// may run past midnight (22 → 7) or not (1 → 5).
    pub fn is_night(&self, hour: u32) -> bool {
        if !self.night || self.night_from == self.night_to {
            return false;
        }
        let (from, to, h) = (self.night_from as u32, self.night_to as u32, hour % 24);
        if from < to {
            (from..to).contains(&h)
        } else {
            h >= from || h < to
        }
    }

    /// The brightness factor night dimming puts on the picture at `hour`.
    pub fn night_factor(&self, hour: u32) -> f32 {
        if self.is_night(hour) {
            self.night_level
        } else {
            1.0
        }
    }

    /// The frame rate while unfocused: None keeps going, Some(0) pauses.
    pub fn unfocused_fps(&self) -> Option<u32> {
        match self.unfocused {
            Unfocused::Keep => None,
            Unfocused::Fps30 => Some(30),
            Unfocused::Fps15 => Some(15),
            Unfocused::Pause => Some(0),
        }
    }

    /// Cell colour changes of up to this many levels are not re-sent, for
    /// Studio and Classic scenes.
    pub fn hysteresis(&self) -> (u8, u8) {
        match self.bandwidth {
            Bandwidth::Full => (0, 0),
            Bandwidth::Balanced => (3, 0),
            Bandwidth::Light => (8, 6),
        }
    }
}

/// The hour a variant named after a time of day starts at (dawn,
/// morning, day, afternoon, golden, sunset, dusk, bluehour, night,
/// midnight and their kin). None for weather, season and colour variants.
pub fn variant_hour(name: &str) -> Option<u32> {
    Some(match name {
        "dawn" | "sunrise" => 6,
        "morning" => 8,
        "day" | "noon" | "midday" | "window-day" => 11,
        "afternoon" => 14,
        "golden" => 17,
        "sunset" | "alpenglow" => 18,
        "dusk" | "evening" | "twilight" => 19,
        "bluehour" => 20,
        "night" | "moonlit" | "moonlight" | "moon" | "starry" => 21,
        "midnight" => 23,
        _ => return None,
    })
}

/// When a scene's first (default) variant is not named for a time, it is
/// its daytime look, from this hour.
const DEFAULT_VARIANT_HOUR: u32 = 9;

/// The variant of `variants` for `hour`: the one whose time started last
/// (wrapping round midnight). A scene's untimed default variant counts as
/// its day. None when no variant is named for a time of day.
pub fn variant_for_hour<'a>(variants: &[&'a str], hour: u32) -> Option<&'a str> {
    let mut timed: Vec<(&str, u32)> = variants.iter().filter_map(|v| Some((*v, variant_hour(v)?))).collect();
    if timed.is_empty() {
        return None;
    }
    if let Some(first) = variants.first().filter(|v| variant_hour(v).is_none()) {
        timed.push((first, DEFAULT_VARIANT_HOUR));
    }
    let h = hour % 24;
    timed
        .iter()
        .filter(|(_, at)| *at <= h)
        .max_by_key(|(_, at)| *at)
        .or_else(|| timed.iter().max_by_key(|(_, at)| *at))
        .map(|(v, _)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_write_nothing_and_read_back() {
        let p = PlaybackPrefs::default();
        assert_eq!(toml::to_string(&p).unwrap().trim(), "");
        let d = DisplayPrefs::default();
        assert_eq!(toml::to_string(&d).unwrap().trim(), "");
        let back: DisplayPrefs = toml::from_str("").unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn choices_round_trip_and_unknowns_fall_back() {
        let p = PlaybackPrefs { order: CycleOrder::InOrder, transition: TransitionStyle::Iris, time_of_day: true, on_launch: OnLaunch::Random };
        let text = toml::to_string(&p).unwrap();
        assert!(text.contains("order = \"in-order\"") && text.contains("transition = \"iris\""), "{text}");
        assert_eq!(toml::from_str::<PlaybackPrefs>(&text).unwrap(), p);
        let d: DisplayPrefs = toml::from_str("unfocused = 15\ncolors = \"256\"\nclock_style = \"huge\"\nmouse = false").unwrap();
        assert_eq!(d.unfocused, Unfocused::Fps15);
        assert_eq!(d.colors, Colors::Ansi256);
        assert_eq!(d.clock_style, ClockStyle::Small, "an unknown value is the default");
        assert!(!d.mouse && d.adapt_fps);
        let text = toml::to_string(&d).unwrap();
        assert!(text.contains("mouse = false") && !text.contains("adapt_fps"), "{text}");
    }

    #[test]
    fn wall_grids_read_and_write() {
        let w = WallPrefs { grid: (3, 2, 4), ..Default::default() };
        assert_eq!(w.grid_spec(), "3x2:4");
        assert_eq!(WallPrefs::parse_grid("3x2:4"), Some((3, 2, 4)));
        assert_eq!(WallPrefs::parse_grid("2x1:2"), None, "past the last cell");
        assert_eq!(WallPrefs::parse_grid("0x1:0"), None);
        assert_eq!(WallPrefs::parse_grid("junk"), None);
        assert!(GRIDS.iter().all(|&(c, r)| WallPrefs::parse_grid(&format!("{c}x{r}:0")).is_some()));
    }

    #[test]
    fn night_windows_cross_midnight() {
        let mut d = DisplayPrefs { night: true, ..Default::default() };
        assert!(d.is_night(23) && d.is_night(0) && d.is_night(6));
        assert!(!d.is_night(7) && !d.is_night(12) && !d.is_night(21));
        assert_eq!(d.night_factor(2), NIGHT_LEVEL);
        assert_eq!(d.night_factor(12), 1.0);
        d.night_from = 1;
        d.night_to = 5;
        assert!(d.is_night(1) && d.is_night(4) && !d.is_night(5) && !d.is_night(0));
        d.night = false;
        assert!(!d.is_night(2));
        let mut wild = DisplayPrefs { night_level: 9.0, night_from: 30, cell_aspect: Some(40.0), ..Default::default() };
        wild.sanitize();
        assert_eq!((wild.night_level, wild.night_from, wild.cell_aspect), (1.0, 6, None));
    }

    #[test]
    fn timed_variants_follow_the_clock() {
        // hongkong: a night scene with no day look stays at night by day
        let v = ["night", "bluehour", "fog"];
        assert_eq!(variant_for_hour(&v, 23), Some("night"));
        assert_eq!(variant_for_hour(&v, 20), Some("bluehour"));
        assert_eq!(variant_for_hour(&v, 10), Some("night"));
        let day = ["dawn", "day", "dusk", "night"];
        assert_eq!(variant_for_hour(&day, 7), Some("dawn"));
        assert_eq!(variant_for_hour(&day, 13), Some("day"));
        assert_eq!(variant_for_hour(&day, 19), Some("dusk"));
        assert_eq!(variant_for_hour(&day, 2), Some("night"));
        // koi: its untimed default is the day, midnight the night
        let koi = ["garden", "ink", "midnight", "pond-blue", "teal"];
        assert_eq!(variant_for_hour(&koi, 12), Some("garden"));
        assert_eq!(variant_for_hour(&koi, 23), Some("midnight"));
        assert_eq!(variant_for_hour(&koi, 4), Some("midnight"));
        assert_eq!(variant_for_hour(&["cyan", "amber"], 12), None);
    }
}
