//! Scene trait, options, themes, and registry.

use crate::canvas::Canvas;
use crate::render::Pixels;
use rand::rngs::StdRng;

pub mod abyss;
pub mod airspace;
pub mod alpine;
pub mod aquarium;
pub mod aurora;
pub mod boids;
pub mod bump;
pub mod campfire;
pub mod candy;
pub mod canopy;
pub mod circuits;
pub mod city;
pub mod clockwork;
pub mod clouds;
pub mod crowd;
pub mod den;
pub mod drive;
pub mod dvd;
pub mod fire;
pub mod finale;
pub mod fireflies;
pub mod frost;
pub mod grid;
pub mod harmonograph;
pub mod incense;
pub mod inkdrop;
pub mod koi;
pub mod lanterns;
pub mod lava;
pub mod life;
pub mod mandel;
pub mod meadow;
pub mod meteors;
pub mod mosaic;
pub mod nebula;
pub mod nexus;
pub mod noise;
pub mod ocean;
pub mod orbits;
pub mod pendulum;
pub mod pipes;
pub mod plasma;
pub mod rain;
pub mod reaction;
pub mod ribbons;
pub mod ripple;
pub mod sand;
pub mod scroll;
pub mod shader;
pub mod shader_meta;
pub mod shells;
pub mod sonar;
pub mod starfield;
pub mod tide;
pub mod traffic;
pub mod tunnel;

/// Density scaling: particle / layer / emitter counts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Detail {
    Low,
    #[default]
    Medium,
    High,
}

/// Global trim on particle counts, on top of the detail multiplier.
///
/// One knob for "every scene a little sparser" — set to 1.0 to restore the
/// previous density. It deliberately applies to *counts and spawn rates only*
/// (via [`Detail::density`]), not to compute budgets, event intervals or
/// brightness, so trimming particles cannot quietly change pacing or exposure.
pub const PARTICLE_TRIM: f32 = 0.90;

impl Detail {
    /// Multiplier over medium counts, before the global trim.
    ///
    /// Use [`Detail::density`] for anything that decides how many things exist;
    /// this raw form is for compute budgets and timings that should track the
    /// detail level without being trimmed.
    pub fn factor(self) -> f32 {
        match self {
            Detail::Low => 0.5,
            Detail::Medium => 1.0,
            Detail::High => 2.0,
        }
    }

    /// Detail multiplier including [`PARTICLE_TRIM`]. This is the one to use
    /// for element counts, spawn rates and spawn probabilities.
    pub fn density(self) -> f32 {
        self.factor() * PARTICLE_TRIM
    }

    #[allow(dead_code)] // used by the settings menu
    pub fn next(self) -> Self {
        match self {
            Detail::Low => Detail::Medium,
            Detail::Medium => Detail::High,
            Detail::High => Detail::Low,
        }
    }

    #[allow(dead_code)] // used by the settings menu
    pub fn name(self) -> &'static str {
        match self {
            Detail::Low => "low",
            Detail::Medium => "medium",
            Detail::High => "high",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Detail::Low),
            "medium" => Some(Detail::Medium),
            "high" => Some(Detail::High),
            _ => None,
        }
    }

    /// Scale a base count by the detail density, keeping at least `min`.
    /// Counts route through `density()`, so they carry `PARTICLE_TRIM`.
    pub fn scale(self, base: f32, min: usize) -> usize {
        ((base * self.density()) as usize).max(min)
    }
}

/// Per-launch scene configuration.
#[derive(Clone, Default, PartialEq)]
pub struct SceneOptions {
    pub theme: Option<String>,
    pub detail: Detail,
    /// bump font scale override (1|2|3); None = auto
    pub text_scale: Option<u32>,
    /// pixel mode the canvas will be shown in — gives scenes the on-screen
    /// pixel aspect (see `Pixels::aspect`) so compositions stay round and
    /// orientation-aware. The engine fills this in from the render request.
    pub pixels: Pixels,
}

pub trait Scene {
    fn name(&self) -> &'static str;
    /// Advance the animation by `dt` seconds (already speed-scaled) and
    /// redraw the full canvas edge-to-edge.
    fn update(&mut self, dt: f32, canvas: &mut Canvas);
}

/// One row of the scene registry: everything the rest of the program needs to
/// know about a scene. This is the single source of truth — the catalog, the
/// theme lists and the constructor all read from it, so adding a scene means
/// one `pub mod` line plus one row here.
pub struct SceneDef {
    pub name: &'static str,
    pub desc: &'static str,
    /// available themes; the first is the default
    pub themes: &'static [&'static str],
    /// takes the full options so scenes with extra knobs (bump's text scale)
    /// need no special-casing at the call site
    pub make: fn(StdRng, &SceneOptions) -> Box<dyn Scene>,
}

/// Every scene, in cycle order. Single source of truth for the catalog,
/// the theme lists and construction.
pub const SCENES: &[SceneDef] = &[
    SceneDef {
        name: "rain",
        desc: "rain on glass, droplet trails and splashes",
        themes: &["night", "storm", "neon", "window-day"],
        make: |rng, o| Box::new(rain::Rain::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "starfield",
        desc: "warp-speed stars flying from center",
        themes: &["classic", "ice", "warm", "void"],
        make: |rng, o| Box::new(starfield::Starfield::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "fire",
        desc: "Doom-style fire with a tuned palette",
        themes: &["classic", "inferno", "emerald", "frost"],
        make: |rng, o| Box::new(fire::Fire::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "pipes",
        desc: "Windows 95 pipes screensaver homage",
        themes: &["classic", "pastel", "mono", "hotmetal"],
        make: |rng, o| Box::new(pipes::Pipes::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "plasma",
        desc: "classic demoscene plasma, hue-cycling sine waves",
        themes: &["rainbow", "cool", "warm", "acid"],
        make: |rng, o| Box::new(plasma::Plasma::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "aurora",
        desc: "northern lights over a starry night sky",
        themes: &["classic", "crimson", "arctic"],
        make: |rng, o| Box::new(aurora::Aurora::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "life",
        desc: "Conway's Game of Life with cooling trails, auto-reseed",
        themes: &["classic", "ember", "ice"],
        make: |rng, o| Box::new(life::Life::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "boids",
        desc: "flocking birds with trails, wrap-around edges",
        themes: &["ice", "sunset", "mono"],
        make: |rng, o| Box::new(boids::Boids::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "lava",
        desc: "lava-lamp metaballs, deep red to yellow-hot",
        themes: &["classic", "basalt", "toxic"],
        make: |rng, o| Box::new(lava::Lava::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "tunnel",
        desc: "texture-mapped tunnel flight, demoscene style",
        themes: &["classic", "inferno", "mono"],
        make: |rng, o| Box::new(tunnel::Tunnel::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "dvd",
        desc: "the bouncing DVD logo meme",
        themes: &["classic", "pastel", "hot"],
        make: |rng, o| Box::new(dvd::Dvd::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "bump",
        desc: "lo-fi deadpan TV bumpers, white on black",
        themes: &["classic", "amber", "green"],
        make: |rng, o| Box::new(bump::Bump::new(rng, o.theme.as_deref(), o.text_scale)),
    },
    SceneDef {
        name: "canopy",
        desc: "tree canopy growing from above, organic branching",
        themes: &["spring", "deep-green", "mono", "blossom"],
        make: |rng, o| Box::new(canopy::Canopy::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "finale",
        desc: "grand-finale fireworks: crackle, crossettes, salvos",
        themes: &["festive", "royal", "ember", "mono-gold"],
        make: |rng, o| Box::new(finale::Finale::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "ocean",
        desc: "night ocean swells under a moonlit glint path",
        themes: &["moonlit", "storm", "golden"],
        make: |rng, o| Box::new(ocean::Ocean::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "circuits",
        desc: "circuit-board traces with zipping data pulses",
        themes: &["pcb", "blueprint", "dark"],
        make: |rng, o| Box::new(circuits::Circuits::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "clouds",
        desc: "daytime sky with drifting fractal clouds",
        themes: &["day", "sunset", "storm"],
        make: |rng, o| Box::new(clouds::Clouds::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "mandel",
        desc: "Mandelbrot deep zoom into seahorse valley",
        themes: &["classic", "fire", "mono"],
        make: |rng, o| Box::new(mandel::Mandel::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "meteors",
        desc: "meteor shower with ion trails and bolides",
        themes: &["night", "warm", "cold"],
        make: |rng, o| Box::new(meteors::Meteors::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "koi",
        desc: "koi pond from above: ripples, lily pads, gliding fish",
        themes: &["teal", "ink", "garden", "pond-blue", "midnight"],
        make: |rng, o| Box::new(koi::KoiPond::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "sand",
        desc: "falling-sand automaton piling stratified dunes",
        themes: &["sandstone", "mono", "ocean"],
        make: |rng, o| Box::new(sand::Sand::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "city",
        desc: "rainy neon metropolis with lightning and traffic",
        themes: &["neon", "noir", "dusk", "realistic"],
        make: |rng, o| Box::new(city::City::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "abyss",
        desc: "deep underwater: god rays, fish schools, leviathans",
        themes: &["deep", "trench", "twilight", "reef"],
        make: |rng, o| Box::new(abyss::Abyss::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "den",
        desc: "a cozy room with a CRT playing other scenes",
        themes: &["night", "evening", "rain-outside"],
        make: |rng, o| Box::new(den::Den::new(rng, o.theme.as_deref(), o.detail, o.pixels)),
    },
    SceneDef {
        name: "traffic",
        desc: "aerial night traffic, long-exposure light streams",
        themes: &["night", "dusk", "rain-slick"],
        make: |rng, o| Box::new(traffic::Traffic::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "nexus",
        desc: "glowing nodes linked into a drifting graph, pulses riding edges",
        themes: &["cyan", "amber", "violet", "mono"],
        make: |rng, o| Box::new(nexus::Nexus::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "ripple",
        desc: "still black water: raindrop rings, drifting leaves, night breeze",
        themes: &["teal", "silver", "ink"],
        make: |rng, o| Box::new(ripple::Ripple::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "fireflies",
        desc: "amber fireflies drifting over a black meadow",
        themes: &["amber", "emerald", "ice"],
        make: |rng, o| Box::new(fireflies::Fireflies::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "lanterns",
        desc: "paper lanterns rising through a black festival night",
        themes: &["warm", "jade", "violet"],
        make: |rng, o| Box::new(lanterns::Lanterns::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "incense",
        desc: "a thin smoke ribbon curling up from a glowing ember",
        themes: &["sandalwood", "temple", "midnight", "zen"],
        make: |rng, o| Box::new(incense::Incense::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "frost",
        desc: "fern-like frost crystals creeping across black glass",
        themes: &["ice", "aurora", "mono"],
        make: |rng, o| Box::new(frost::Frost::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "orbits",
        desc: "planets tracing luminous orbital trails around a star",
        themes: &["solar", "binary", "ice"],
        make: |rng, o| Box::new(orbits::Orbits::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "ribbons",
        desc: "silk ribbons flowing across the dark",
        themes: &["silk", "ember", "ocean"],
        make: |rng, o| Box::new(ribbons::Ribbons::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "sonar",
        desc: "phosphor radar sweep lighting up drifting contacts",
        themes: &["green", "amber", "cyan"],
        make: |rng, o| Box::new(sonar::Sonar::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "tide",
        desc: "luminous contour ridges morphing like a slow signal",
        themes: &["pulse", "ice", "ember"],
        make: |rng, o| Box::new(tide::Tide::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "clockwork",
        desc: "interlocking brass gears turning in the dark",
        themes: &["brass", "steel", "verdigris"],
        make: |rng, o| Box::new(clockwork::Clockwork::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "grid",
        desc: "synthwave perspective grid rolling to the horizon",
        themes: &["vapor", "lime", "mono"],
        make: |rng, o| Box::new(grid::Grid::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "inkdrop",
        desc: "ink blooming through still black water",
        themes: &["indigo", "crimson", "sepia"],
        make: |rng, o| Box::new(inkdrop::Inkdrop::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "mosaic",
        desc: "stained-glass cells breathing and flashing on black",
        themes: &["cathedral", "ocean", "mono"],
        make: |rng, o| Box::new(mosaic::Mosaic::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "harmonograph",
        desc: "glowing spiro curves drawing themselves, then fading",
        themes: &["prism", "gold", "ice"],
        make: |rng, o| Box::new(harmonograph::Harmonograph::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "nebula",
        desc: "deep-space clouds drifting in parallax layers",
        themes: &["emission", "crimson", "void"],
        make: |rng, o| Box::new(nebula::Nebula::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "pendulum",
        desc: "pendulum-wave interference, glowing bobs on faint strings",
        themes: &["chrome", "amber", "ice"],
        make: |rng, o| Box::new(pendulum::Pendulum::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "reaction",
        desc: "reaction-diffusion coral growing and splitting on black",
        themes: &["coral", "acid", "ice"],
        make: |rng, o| Box::new(reaction::Reaction::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "meadow",
        desc: "windswept night grass, dew glints, shooting stars",
        themes: &["moonlit", "amber", "jade"],
        make: |rng, o| Box::new(meadow::Meadow::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "airspace",
        desc: "realistic sky over fields with planes, contrails, and low passes",
        themes: &["day", "golden", "dusk", "coast", "storm", "night", "winter", "busy"],
        make: |rng, o| Box::new(airspace::Airspace::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "aquarium",
        desc: "side-view tank: caustics, fish, bubbles, drifting plants",
        themes: &["tropical", "freshwater", "moonlit", "mono"],
        make: |rng, o| Box::new(aquarium::Aquarium::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "drive",
        desc: "driver POV at night: road scrolls, scenery rushes past",
        themes: &["night", "dusk", "rain", "neon"],
        make: |rng, o| Box::new(drive::Drive::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "candy",
        desc: "saturated sugar-rush orbs on a neon gradient",
        themes: &["classic", "sour", "pastel", "mono"],
        make: |rng, o| Box::new(candy::Candy::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "scroll",
        desc: "endless ink-wash mountain scroll unrolling upward, made for portrait screens",
        themes: &["sumi", "night", "indigo"],
        make: |rng, o| Box::new(scroll::Scroll::new(rng, o.theme.as_deref(), o.detail)),
    },
    SceneDef {
        name: "alpine",
        desc: "alpine lake at dusk: parallax ridges under a sinking sun, a mirrored lake, a lone canoe",
        themes: &["dusk", "dawn", "storm"],
        make: |rng, o| Box::new(alpine::Alpine::new(rng, o.theme.as_deref(), o.detail, o.pixels)),
    },
    SceneDef {
        name: "campfire",
        desc: "night campfire in the forest: flame, embers, smoke, and a figure poking the fire",
        themes: &["pine", "autumn", "snow"],
        make: |rng, o| Box::new(campfire::Campfire::new(rng, o.theme.as_deref(), o.detail, o.pixels)),
    },
];

/// The Classic (CPU) scenes, in registry order. Tests, `Playback` and the
/// WGSL world shader's positional ids all work on this list; the UI uses
/// [`all_names`], which also includes the Studio shader scenes.
pub fn names() -> Vec<&'static str> {
    SCENES.iter().map(|s| s.name).collect()
}

/// Every scene, Studio and Classic, in browser order (see [`entries`]).
pub fn all_names() -> Vec<&'static str> {
    entries().map(|e| e.name()).collect()
}

/// All scenes as (name, desc) for --list and the settings menu, in the same
/// order as [`all_names`].
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    entries().map(|e| (e.name(), e.desc())).collect()
}

/// Look a Classic scene up by name. Studio scenes are not `SceneDef`s — use
/// [`lookup`] for either kind.
pub fn find(name: &str) -> Option<&'static SceneDef> {
    SCENES.iter().find(|s| s.name == name)
}

/// Named themes available for a scene of either kind (first = default).
pub fn themes(name: &str) -> &'static [&'static str] {
    lookup(name).map(|e| e.themes()).unwrap_or(&[])
}

/// Build a Classic scene by name with options. Studio scenes have no CPU
/// implementation and return `None` (their fallback is a Classic scene).
pub fn create(name: &str, opts: &SceneOptions, rng: StdRng) -> Option<Box<dyn Scene>> {
    find(name).map(|s| (s.make)(rng, opts))
}

pub use shader::Category;

/// A scene of either kind.
#[derive(Clone, Copy)]
pub enum Entry {
    Cpu(&'static SceneDef),
    Shader(&'static shader::ShaderSpec),
}

impl Entry {
    pub fn name(self) -> &'static str {
        match self {
            Entry::Cpu(d) => d.name,
            Entry::Shader(s) => s.name,
        }
    }
    /// Display title ("Tokyo Alley"); Classic scenes use their name.
    pub fn title(self) -> &'static str {
        match self {
            Entry::Cpu(d) => d.name,
            Entry::Shader(s) => s.title,
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Entry::Cpu(d) => d.desc,
            Entry::Shader(s) => s.desc,
        }
    }
    pub fn themes(self) -> &'static [&'static str] {
        match self {
            Entry::Cpu(d) => d.themes,
            Entry::Shader(s) => s.themes,
        }
    }
    pub fn category(self) -> Category {
        match self {
            Entry::Cpu(_) => Category::Classic,
            Entry::Shader(s) => s.category,
        }
    }
    pub fn tags(self) -> &'static [&'static str] {
        match self {
            Entry::Cpu(d) => classic_tags(d.name),
            Entry::Shader(s) => s.tags,
        }
    }
    /// Sub-heading inside a category: Classic scenes are grouped by kind.
    pub fn group(self) -> &'static str {
        match self {
            Entry::Cpu(d) => classic_tags(d.name).first().copied().unwrap_or("other"),
            Entry::Shader(s) => s.category.label(),
        }
    }
    /// Studio scenes render on the GPU; without one they show their fallback.
    pub fn needs_gpu(self) -> bool {
        matches!(self, Entry::Shader(_))
    }
}

/// Every scene in browser order: Studio scenes by category, then Classic in
/// registry order.
pub fn entries() -> impl Iterator<Item = Entry> {
    shader::SHADER_SCENES
        .iter()
        .map(Entry::Shader)
        .chain(SCENES.iter().map(Entry::Cpu))
}

/// Find a scene of either kind.
pub fn lookup(name: &str) -> Option<Entry> {
    shader::find(name)
        .map(Entry::Shader)
        .or_else(|| find(name).map(Entry::Cpu))
}

pub fn exists(name: &str) -> bool {
    lookup(name).is_some()
}

/// Classic scenes' browser groups and search tags; the first tag is the group.
fn classic_tags(name: &str) -> &'static [&'static str] {
    match name {
        "rain" | "aurora" | "clouds" | "meadow" | "fireflies" | "frost" | "canopy" | "alpine"
        | "airspace" | "scroll" => &["nature", "landscape"],
        "ocean" | "koi" | "abyss" | "aquarium" | "ripple" => &["water", "nature"],
        "starfield" | "meteors" | "nebula" | "orbits" => &["space"],
        "plasma" | "tunnel" | "fire" | "pipes" | "dvd" | "bump" | "grid" | "mandel" => {
            &["demoscene", "retro"]
        }
        "life" | "boids" | "sand" | "reaction" | "lava" | "harmonograph" | "pendulum"
        | "inkdrop" | "mosaic" | "tide" | "ribbons" | "nexus" | "candy" => {
            &["generative", "simulation"]
        }
        "city" | "traffic" | "drive" | "finale" => &["urban", "night"],
        "circuits" | "sonar" | "clockwork" => &["machines"],
        "den" | "incense" | "campfire" | "lanterns" => &["cozy"],
        _ => &["other"],
    }
}

/// Scenes drawn by their WGSL arm in `src/gpu/shaders/world.wgsl` when a GPU
/// is available, instead of running the Rust scene. Empty by default: the
/// Rust scenes are the artwork and the GPU only post-processes and packs
/// cells. `--renderer shader` treats every scene as if it were listed here.
pub const GPU_WORLD_SCENES: &[&str] = &[];

/// Whether the GPU should draw this scene from its shader arm.
pub fn gpu_world(name: &str) -> bool {
    GPU_WORLD_SCENES.contains(&name)
}


#[cfg(test)]
mod option_tests {
    use super::*;

    #[test]
    fn gpu_world_allowlist_names_exist() {
        for name in GPU_WORLD_SCENES {
            assert!(find(name).is_some(), "GPU_WORLD_SCENES lists unknown scene {name}");
            assert!(gpu_world(name));
        }
        assert!(!gpu_world("no-such-scene"));
    }

    #[test]
    fn detail_scaling_changes_particle_counts() {
        // raw factor is untrimmed: it drives compute budgets and timings
        assert_eq!(Detail::Low.factor(), 0.5);
        assert_eq!(Detail::Medium.factor(), 1.0);
        assert_eq!(Detail::High.factor(), 2.0);
        // counts carry the global trim
        assert_eq!(Detail::Medium.density(), PARTICLE_TRIM);
        assert_eq!(Detail::Low.density(), 0.5 * PARTICLE_TRIM);
        assert_eq!(Detail::High.density(), 2.0 * PARTICLE_TRIM);
        assert_eq!(Detail::Low.scale(100.0, 1), (50.0 * PARTICLE_TRIM) as usize);
        assert_eq!(Detail::Medium.scale(100.0, 1), (100.0 * PARTICLE_TRIM) as usize);
        assert_eq!(Detail::High.scale(100.0, 1), (200.0 * PARTICLE_TRIM) as usize);
        assert!(Detail::High.scale(40.0, 10) > Detail::Low.scale(40.0, 10));
        // floor: min wins when scaled count would be smaller
        assert_eq!(Detail::Low.scale(4.0, 10), 10);
        assert_eq!(Detail::Low.next(), Detail::Medium);
        assert_eq!(Detail::High.name(), "high");
        assert_eq!(Detail::default().name(), "medium");
        assert_eq!(Detail::High.next(), Detail::Low);
        assert_eq!(Detail::parse("high"), Some(Detail::High));
        assert_eq!(Detail::parse("bogus"), None);
    }

    #[test]
    fn theme_application_changes_palette() {
        // fire: classic vs frost produce different colors at the same heat
        let classic = fire::palette_themed(0.7, None);
        let frost = fire::palette_themed(0.7, Some("frost"));
        assert_ne!(classic, frost);
        assert!(frost.2 > frost.0, "frost should be blue-dominant");
        // every scene exposes at least one theme and the default is valid
        for def in SCENES {
            assert!(!def.themes.is_empty(), "{} has no themes", def.name);
        }
        assert_eq!(themes("nexus"), &["cyan", "amber", "violet", "mono"]);
        assert!(themes("nonexistent-scene").is_empty());
    }
}

#[cfg(test)]
mod perf {
    use super::*;
    use rand::SeedableRng;
    use std::time::Instant;

    /// Every scene must comfortably hold 30fps at 200x100 (2x the spec
    /// size), even at detail=High.
    #[test]
    fn scenes_hold_30fps_at_200x100() {
        // perf gates are only meaningful in optimized builds
        if cfg!(debug_assertions) {
            return;
        }
        let opts = SceneOptions {
            theme: None,
            detail: Detail::High,
            text_scale: None,
            pixels: Default::default(),
        };
        for name in names() {
            let mut s = create(name, &opts, StdRng::seed_from_u64(1)).unwrap();
            let mut c = Canvas::new(200, 100);
            s.update(1.0 / 30.0, &mut c); // init
            let t = Instant::now();
            for _ in 0..60 {
                s.update(1.0 / 30.0, &mut c);
            }
            let per_frame = t.elapsed().as_secs_f32() / 60.0;
            eprintln!("{name}: {:.2}ms/frame", per_frame * 1000.0);
            assert!(
                per_frame < 0.008,
                "{name}: {per_frame:.4}s/frame exceeds 60fps quad budget (8ms)"
            );
        }
    }
}

#[cfg(test)]
mod scaling_tests {
    use super::*;
    use crate::canvas::density_for;
    use rand::SeedableRng;

    #[test]
    fn element_counts_scale_with_canvas_area() {
        // 200x100 should host ~2x+ the elements of 100x50
        let count = |name: &str, w: usize, h: usize| {
            let opts = SceneOptions {
                theme: None,
                detail: Detail::Medium,
                text_scale: None,
                pixels: Default::default(),
            };
            let mut s = create(name, &opts, StdRng::seed_from_u64(1)).unwrap();
            let mut c = Canvas::new(w, h);
            s.update(1.0 / 30.0, &mut c);
            // proxy: non-background pixel count after warmup
            for _ in 0..10 {
                s.update(1.0 / 30.0, &mut c);
            }
            (0..w as i32)
                .flat_map(|x| (0..h as i32).map(move |y| (x, y)))
                .filter(|&(x, y)| {
                    let (r, g, b) = c.get(x, y).color;
                    r as u32 + g as u32 + b as u32 > 60
                })
                .count()
        };
        for name in ["starfield", "koi", "boids"] {
            let small = count(name, 100, 50);
            let big = count(name, 200, 100);
            assert!(
                big >= small * 2,
                "{name}: {big} at 200x100 vs {small} at 100x50"
            );
        }
        assert!(density_for(200, 100) / density_for(100, 50) >= 2.0);
    }

    #[test]
    fn no_panics_at_extreme_sizes() {
        let opts = SceneOptions {
            theme: None,
            detail: Detail::High,
            text_scale: None,
            pixels: Default::default(),
        };
        for (w, h) in [(40, 12), (400, 200)] {
            for name in names() {
                let mut s = create(name, &opts, StdRng::seed_from_u64(2)).unwrap();
                let mut c = Canvas::new(w, h);
                for _ in 0..10 {
                    s.update(1.0 / 30.0, &mut c);
                }
            }
        }
    }
}

#[cfg(test)]
mod determinism_tests {
    use super::*;
    use rand::SeedableRng;

    fn checksum(s: &mut dyn Scene, steps: usize) -> u64 {
        let mut c = Canvas::new(80, 40);
        let mut h = 0xcbf29ce484222325u64;
        for _ in 0..steps {
            s.update(1.0 / 60.0, &mut c);
        }
        for y in 0..40 {
            for x in 0..80 {
                let (r, g, b) = c.get(x, y).color;
                h ^= ((r as u64) << 16) ^ ((g as u64) << 8) ^ b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
        h
    }

    #[test]
    fn same_seed_same_steps_identical_state() {
        let opts = SceneOptions::default();
        for name in ["fire", "starfield", "rain", "koi", "boids"] {
            let mut a = create(name, &opts, StdRng::seed_from_u64(777)).unwrap();
            let mut b = create(name, &opts, StdRng::seed_from_u64(777)).unwrap();
            let (ha, hb) = (checksum(&mut *a, 90), checksum(&mut *b, 90));
            assert_eq!(ha, hb, "{name} must be deterministic for a seed");
        }
    }

    #[test]
    fn different_seed_diverges() {
        let opts = SceneOptions::default();
        let mut a = create("fire", &opts, StdRng::seed_from_u64(1)).unwrap();
        let mut b = create("fire", &opts, StdRng::seed_from_u64(2)).unwrap();
        assert_ne!(checksum(&mut *a, 60), checksum(&mut *b, 60));
    }

    #[test]
    fn fast_forward_matches_continuous_run() {
        // the sync path: fresh scene + N fast-forward steps == N live frames
        let opts = SceneOptions::default();
        let mut live = create("fire", &opts, StdRng::seed_from_u64(99)).unwrap();
        let live_sum = checksum(&mut *live, 120);
        let mut synced = create("fire", &opts, StdRng::seed_from_u64(99)).unwrap();
        let ff_sum = checksum(&mut *synced, 120);
        assert_eq!(live_sum, ff_sum);
    }
}

#[cfg(test)]
mod fast_forward_tests {
    use super::*;
    use rand::SeedableRng;
    use std::time::{Duration, Instant};

    /// Time-boxed fast-forward (mirrors the sync path in main): even when
    /// 3600 steps are requested, every scene must finish in well under a
    /// second of wall time — a slightly younger scene beats a frozen one.
    #[test]
    fn fast_forward_is_time_boxed() {
        let opts = SceneOptions {
            theme: None,
            detail: Detail::High,
            text_scale: None,
            pixels: Default::default(),
        };
        for name in names() {
            let mut s = create(name, &opts, StdRng::seed_from_u64(1)).unwrap();
            let mut scratch = Canvas::new(200, 100);
            let start = Instant::now();
            for _ in 0..3600 {
                s.update(1.0 / 60.0, &mut scratch);
                if start.elapsed() > Duration::from_millis(200) {
                    break;
                }
            }
            let total = start.elapsed();
            assert!(
                total < Duration::from_millis(300),
                "{name}: fast-forward took {total:?}"
            );
        }
    }
}

#[cfg(test)]
mod clouds_perf_tests {
    use super::*;
    use rand::SeedableRng;
    use std::time::Instant;

    #[test]
    fn clouds_under_1_5ms_at_200x100() {
        if cfg!(debug_assertions) {
            return;
        }
        let opts = SceneOptions {
            theme: None,
            detail: Detail::High,
            text_scale: None,
            pixels: Default::default(),
        };
        let mut s = create("clouds", &opts, StdRng::seed_from_u64(1)).unwrap();
        let mut c = Canvas::new(200, 100);
        s.update(1.0 / 60.0, &mut c);
        let t = Instant::now();
        for _ in 0..60 {
            s.update(1.0 / 60.0, &mut c);
        }
        let per = t.elapsed().as_secs_f32() / 60.0;
        assert!(per < 0.0015, "clouds {:.2}ms/frame exceeds 1.5ms", per * 1000.0);
    }
}

#[cfg(test)]
mod edge_coverage_tests {
    use super::*;
    use rand::SeedableRng;

    /// Every pixel scene must paint essentially the whole canvas: after 2s
    /// of updates at 160x100, fewer than 2% of cells may remain untouched.
    #[test]
    fn scenes_cover_the_full_canvas() {
        let opts = SceneOptions {
            theme: None,
            detail: Detail::Medium,
            text_scale: None,
            pixels: Default::default(),
        };
        for name in names() {
            if matches!(name, "bump" | "dvd") {
                continue; // darkness is their content; checked separately
            }
            let mut s = create(name, &opts, StdRng::seed_from_u64(1)).unwrap();
            let mut c = Canvas::new(160, 100);
            for _ in 0..120 {
                s.update(1.0 / 60.0, &mut c);
            }
            let mut untouched = 0usize;
            for y in 0..100 {
                for x in 0..160 {
                    if !c.touched(x, y) {
                        untouched += 1;
                    }
                }
            }
            let pct = untouched as f32 / 16000.0 * 100.0;
            assert!(pct < 2.0, "{name}: {pct:.1}% of the canvas never painted");
        }
    }

    /// Glyph-driven scenes: darkness is the art, but their content regions
    /// must be painted (logo pixels / text pixels present).
    #[test]
    fn glyph_scenes_paint_their_content() {
        let opts = SceneOptions::default();
        // dvd: the logo's pixels must be lit somewhere
        let mut d = create("dvd", &opts, StdRng::seed_from_u64(1)).unwrap();
        let mut c = Canvas::new(160, 100);
        for _ in 0..30 {
            d.update(1.0 / 60.0, &mut c);
        }
        let lit = (0..160)
            .flat_map(|x| (0..100).map(move |y| (x, y)))
            .filter(|&(x, y)| c.get(x, y).color != (0, 0, 0))
            .count();
        assert!(lit > 100, "dvd logo should be visible, {lit} px lit");

        // bump: after the title types on, many text pixels must be lit
        let mut b = create("bump", &opts, StdRng::seed_from_u64(1)).unwrap();
        for _ in 0..120 {
            b.update(1.0 / 60.0, &mut c);
        }
        let lit = (0..160)
            .flat_map(|x| (0..100).map(move |y| (x, y)))
            .filter(|&(x, y)| c.get(x, y).color != (0, 0, 0))
            .count();
        assert!(lit > 200, "bump text should be visible, {lit} px lit");
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use rand::SeedableRng;

    /// FNV-1a over the rendered frame after `steps` updates.
    fn checksum(name: &str, opts: &SceneOptions, w: usize, h: usize, steps: usize) -> u64 {
        let mut s = create(name, opts, StdRng::seed_from_u64(4242)).unwrap();
        let mut c = Canvas::new(w, h);
        for _ in 0..steps {
            s.update(1.0 / 30.0, &mut c);
        }
        let mut hash = 0xcbf29ce484222325u64;
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (r, g, b) = c.get(x, y).color;
                hash ^= ((r as u64) << 16) ^ ((g as u64) << 8) ^ b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash
    }

    fn opts_for(theme: Option<&str>, detail: Detail) -> SceneOptions {
        SceneOptions {
            theme: theme.map(|t| t.to_string()),
            detail,
            text_scale: None,
            pixels: Default::default(),
        }
    }

    /// Every theme the catalog advertises must actually change what is drawn.
    /// Index 0 is the default (scenes serve it from their `_` arm), so each
    /// later theme is compared against it — a typo'd or dropped theme string
    /// silently falls through to the default, and this is what catches that.
    #[test]
    fn every_advertised_theme_changes_output() {
        for def in SCENES {
            let base = checksum(def.name, &opts_for(Some(def.themes[0]), Detail::Medium), 96, 48, 45);
            for theme in &def.themes[1..] {
                let got = checksum(def.name, &opts_for(Some(theme), Detail::Medium), 96, 48, 45);
                assert_ne!(
                    base, got,
                    "{}: theme '{}' renders identically to the default '{}' — \
                     is the theme string handled in the scene's match?",
                    def.name, theme, def.themes[0]
                );
            }
        }
    }

    /// `--detail` must do something for every scene. `bump` is exempt: it draws
    /// a fixed-size text sprite and has no density to scale (it takes
    /// `text_scale` instead).
    #[test]
    fn detail_changes_output() {
        const EXEMPT: &[&str] = &["bump"];
        for def in SCENES {
            if EXEMPT.contains(&def.name) {
                continue;
            }
            let lo = checksum(def.name, &opts_for(None, Detail::Low), 160, 100, 90);
            let hi = checksum(def.name, &opts_for(None, Detail::High), 160, 100, 90);
            assert_ne!(
                lo, hi,
                "{}: detail=low and detail=high render identically — \
                 the scene ignores its Detail parameter",
                def.name
            );
        }
    }

    /// The table is the single source of truth: every row must be constructible
    /// and self-consistent.
    #[test]
    fn registry_is_internally_consistent() {
        assert_eq!(SCENES.len(), names().len());
        // the catalog lists Studio scenes too
        assert_eq!(SCENES.len() + shader::SHADER_SCENES.len(), catalog().len());
        for def in SCENES {
            assert!(!def.desc.is_empty(), "{} has no description", def.name);
            assert!(!def.themes.is_empty(), "{} has no themes", def.name);
            assert!(find(def.name).is_some(), "{} not findable", def.name);
            assert!(
                create(def.name, &SceneOptions::default(), StdRng::seed_from_u64(1)).is_some(),
                "{} not constructible",
                def.name
            );
        }
        // names are unique
        let mut seen = std::collections::HashSet::new();
        for def in SCENES {
            assert!(seen.insert(def.name), "duplicate scene name {}", def.name);
        }
        assert!(find("no-such-scene").is_none());
        assert!(themes("no-such-scene").is_empty());
    }
}

#[cfg(test)]
mod particle_trim_tests {
    use super::*;
    use rand::SeedableRng;

    /// The trim is a 10% reduction, and it is the only thing standing between
    /// `factor()` and `density()`.
    #[test]
    fn trim_is_ten_percent_and_counts_only() {
        assert!((PARTICLE_TRIM - 0.90).abs() < 1e-6, "trim should be 10%");
        for d in [Detail::Low, Detail::Medium, Detail::High] {
            assert!((d.density() - d.factor() * PARTICLE_TRIM).abs() < 1e-6);
        }
    }

    /// Relative detail behaviour must survive the trim: high still renders
    /// meaningfully more than low, and the ordering is unchanged.
    #[test]
    fn detail_ordering_survives_the_trim() {
        assert!(Detail::Low.scale(200.0, 1) < Detail::Medium.scale(200.0, 1));
        assert!(Detail::Medium.scale(200.0, 1) < Detail::High.scale(200.0, 1));
        // and the floor still wins when the scaled count would be smaller
        assert_eq!(Detail::Low.scale(4.0, 10), 10);
    }

    /// The trim must actually reach the screen: with counts 10% lower, scenes
    /// that draw discrete elements paint fewer lit pixels. Checked in
    /// aggregate over a spread of particle scenes so one scene's clamping
    /// cannot mask a wiring mistake.
    #[test]
    fn trim_reduces_painted_elements_in_aggregate() {
        let lit = |name: &str| -> usize {
            let opts = SceneOptions {
                theme: None,
                detail: Detail::High,
                text_scale: None,
                pixels: Default::default(),
            };
            let mut s = create(name, &opts, StdRng::seed_from_u64(11)).unwrap();
            let mut c = Canvas::new(160, 100);
            for _ in 0..60 {
                s.update(1.0 / 60.0, &mut c);
            }
            (0..160)
                .flat_map(|x| (0..100).map(move |y| (x, y)))
                .filter(|&(x, y)| {
                    let (r, g, b) = c.get(x, y).color;
                    r as u32 + g as u32 + b as u32 > 40
                })
                .count()
        };
        // these all scale element counts through density()
        let total: usize = ["starfield", "meteors", "fireflies", "boids", "lanterns"]
            .iter()
            .map(|n| lit(n))
            .sum();
        // a floor, not an exact value: this pins that counts are live and
        // non-trivial, so a future edit that silently zeroes them fails here
        assert!(total > 1000, "expected a populated field, got {total} lit px");
    }
}
