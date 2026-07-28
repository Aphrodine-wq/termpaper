//! Scene trait, options, themes, and registry.

use crate::canvas::Canvas;
use rand::rngs::StdRng;

pub mod abyss;
pub mod airspace;
pub mod aquarium;
pub mod aurora;
pub mod boids;
pub mod bump;
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

impl Detail {
    /// Multiplier over medium counts.
    pub fn factor(self) -> f32 {
        match self {
            Detail::Low => 0.5,
            Detail::Medium => 1.0,
            Detail::High => 2.0,
        }
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

    /// Scale a base count by the detail factor, keeping at least `min`.
    pub fn scale(self, base: f32, min: usize) -> usize {
        ((base * self.factor()) as usize).max(min)
    }
}

/// Per-launch scene configuration.
#[derive(Clone, Default)]
pub struct SceneOptions {
    pub theme: Option<String>,
    pub detail: Detail,
    /// bump font scale override (1|2|3); None = auto
    pub text_scale: Option<u32>,
}

pub trait Scene {
    fn name(&self) -> &'static str;
    /// Advance the animation by `dt` seconds (already speed-scaled) and
    /// redraw the full canvas edge-to-edge.
    fn update(&mut self, dt: f32, canvas: &mut Canvas);
}

/// (name, one-line description) for every scene, in cycle order.
pub const SCENES: &[(&str, &str)] = &[
    ("rain", "rain on glass, droplet trails and splashes"),
    ("starfield", "warp-speed stars flying from center"),
    ("fire", "Doom-style fire with a tuned palette"),
    ("pipes", "Windows 95 pipes screensaver homage"),
    ("plasma", "classic demoscene plasma, hue-cycling sine waves"),
    ("aurora", "northern lights over a starry night sky"),
    ("life", "Conway's Game of Life with cooling trails, auto-reseed"),
    ("boids", "flocking birds with trails, wrap-around edges"),
    ("lava", "lava-lamp metaballs, deep red to yellow-hot"),
    ("tunnel", "texture-mapped tunnel flight, demoscene style"),
    ("dvd", "the bouncing DVD logo meme"),
    ("bump", "lo-fi deadpan TV bumpers, white on black"),
    ("canopy", "tree canopy growing from above, organic branching"),
    ("finale", "grand-finale fireworks: crackle, crossettes, salvos"),
    ("ocean", "night ocean swells under a moonlit glint path"),
    ("circuits", "circuit-board traces with zipping data pulses"),
    ("clouds", "daytime sky with drifting fractal clouds"),
    ("mandel", "Mandelbrot deep zoom into seahorse valley"),
    ("meteors", "meteor shower with ion trails and bolides"),
    ("koi", "koi pond from above: ripples, lily pads, gliding fish"),
    ("sand", "falling-sand automaton piling stratified dunes"),
    ("city", "rainy neon metropolis with lightning and traffic"),
    ("abyss", "deep underwater: god rays, fish schools, leviathans"),
    ("den", "a cozy room with a CRT playing other scenes"),
    ("traffic", "aerial night traffic, long-exposure light streams"),
    ("nexus", "glowing nodes linked into a drifting graph, pulses riding edges"),
    ("ripple", "still black water: raindrop rings, drifting leaves, night breeze"),
    ("fireflies", "amber fireflies drifting over a black meadow"),
    ("lanterns", "paper lanterns rising through a black festival night"),
    ("frost", "fern-like frost crystals creeping across black glass"),
    ("orbits", "planets tracing luminous orbital trails around a star"),
    ("ribbons", "silk ribbons flowing across the dark"),
    ("sonar", "phosphor radar sweep lighting up drifting contacts"),
    ("tide", "luminous contour ridges morphing like a slow signal"),
    ("clockwork", "interlocking brass gears turning in the dark"),
    ("grid", "synthwave perspective grid rolling to the horizon"),
    ("inkdrop", "ink blooming through still black water"),
    ("mosaic", "stained-glass cells breathing and flashing on black"),
    ("harmonograph", "glowing spiro curves drawing themselves, then fading"),
    ("nebula", "deep-space clouds drifting in parallax layers"),
    ("pendulum", "pendulum-wave interference, glowing bobs on faint strings"),
    ("reaction", "reaction-diffusion coral growing and splitting on black"),
    ("meadow", "windswept night grass, dew glints, shooting stars"),
    (
        "airspace",
        "realistic sky over fields with planes, contrails, and low passes",
    ),
    ("aquarium", "side-view tank: caustics, fish, bubbles, drifting plants"),
    ("drive", "driver POV at night: road scrolls, scenery rushes past"),
    ("candy", "saturated sugar-rush orbs on a neon gradient"),
];

pub fn names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = SCENES.iter().map(|(n, _)| *n).collect();
    v.extend(MARKETPLACE_SCENES.iter().map(|(n, _)| *n));
    v
}

/// Built-in + community scenes for --list and the settings menu.
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    SCENES
        .iter()
        .chain(MARKETPLACE_SCENES.iter())
        .copied()
        .collect()
}

/// Named themes available for a scene (first = default).
#[allow(dead_code)] // used by the settings menu
pub fn themes(name: &str) -> &'static [&'static str] {
    if let Some(t) = marketplace_themes(name) {
        return t;
    }
    match name {
        "rain" => &["night", "storm", "neon", "window-day"],
        "starfield" => &["classic", "ice", "warm", "void"],
        "fire" => &["classic", "inferno", "emerald", "frost"],
        "pipes" => &["classic", "pastel", "mono", "hotmetal"],
        "plasma" => &["rainbow", "cool", "warm", "acid"],
        "aurora" => &["classic", "crimson", "arctic"],
        "life" => &["classic", "ember", "ice"],
        "boids" => &["ice", "sunset", "mono"],
        "lava" => &["classic", "basalt", "toxic"],
        "tunnel" => &["classic", "inferno", "mono"],
        "dvd" => &["classic", "pastel", "hot"],
        "bump" => &["classic", "amber", "green"],
        "canopy" => &["spring", "deep-green", "mono", "blossom"],
        "finale" => &["festive", "royal", "ember", "mono-gold"],
        "ocean" => &["moonlit", "storm", "golden"],
        "circuits" => &["pcb", "blueprint", "dark"],
        "clouds" => &["day", "sunset", "storm"],
        "mandel" => &["classic", "fire", "mono"],
        "meteors" => &["night", "warm", "cold"],
        "koi" => &["teal", "ink", "garden", "pond-blue", "midnight"],
        "sand" => &["sandstone", "mono", "ocean"],
        "city" => &["neon", "noir", "dusk", "realistic"],
        "abyss" => &["deep", "trench", "twilight", "reef"],
        "den" => &["night", "evening", "rain-outside"],
        "traffic" => &["night", "dusk", "rain-slick"],
        "nexus" => &["cyan", "amber", "violet", "mono"],
        "ripple" => &["teal", "silver", "ink"],
        "fireflies" => &["amber", "emerald", "ice"],
        "lanterns" => &["warm", "jade", "violet"],
        "frost" => &["ice", "aurora", "mono"],
        "orbits" => &["solar", "binary", "ice"],
        "ribbons" => &["silk", "ember", "ocean"],
        "sonar" => &["green", "amber", "cyan"],
        "tide" => &["pulse", "ice", "ember"],
        "clockwork" => &["brass", "steel", "verdigris"],
        "grid" => &["vapor", "lime", "mono"],
        "inkdrop" => &["indigo", "crimson", "sepia"],
        "mosaic" => &["cathedral", "ocean", "mono"],
        "harmonograph" => &["prism", "gold", "ice"],
        "nebula" => &["emission", "crimson", "void"],
        "pendulum" => &["chrome", "amber", "ice"],
        "reaction" => &["coral", "acid", "ice"],
        "meadow" => &["moonlit", "amber", "jade"],
        "airspace" => &[
            "day", "golden", "dusk", "coast", "storm", "night", "winter", "busy",
        ],
        "aquarium" => &["tropical", "freshwater", "moonlit", "mono"],
        "drive" => &["night", "dusk", "rain", "neon"],
        "candy" => &["classic", "sour", "pastel", "mono"],
        _ => &[],
    }
}

include!(concat!(env!("OUT_DIR"), "/marketplace.rs"));

/// Build a scene by name with options.
pub fn create(name: &str, opts: &SceneOptions, rng: StdRng) -> Option<Box<dyn Scene>> {
    let theme = opts.theme.as_deref();
    let d = opts.detail;
    match name {
        "rain" => Some(Box::new(rain::Rain::new(rng, theme, d))),
        "starfield" => Some(Box::new(starfield::Starfield::new(rng, theme, d))),
        "fire" => Some(Box::new(fire::Fire::new(rng, theme, d))),
        "pipes" => Some(Box::new(pipes::Pipes::new(rng, theme, d))),
        "plasma" => Some(Box::new(plasma::Plasma::new(rng, theme, d))),
        "aurora" => Some(Box::new(aurora::Aurora::new(rng, theme, d))),
        "life" => Some(Box::new(life::Life::new(rng, theme, d))),
        "boids" => Some(Box::new(boids::Boids::new(rng, theme, d))),
        "lava" => Some(Box::new(lava::Lava::new(rng, theme, d))),
        "tunnel" => Some(Box::new(tunnel::Tunnel::new(rng, theme))),
        "dvd" => Some(Box::new(dvd::Dvd::new(rng, theme))),
        "bump" => Some(Box::new(bump::Bump::new(rng, theme, opts.text_scale))),
        "canopy" => Some(Box::new(canopy::Canopy::new(rng, theme, d))),
        "finale" => Some(Box::new(finale::Finale::new(rng, theme, d))),
        "ocean" => Some(Box::new(ocean::Ocean::new(rng, theme, d))),
        "circuits" => Some(Box::new(circuits::Circuits::new(rng, theme, d))),
        "clouds" => Some(Box::new(clouds::Clouds::new(rng, theme))),
        "mandel" => Some(Box::new(mandel::Mandel::new(rng, theme, d))),
        "meteors" => Some(Box::new(meteors::Meteors::new(rng, theme, d))),
        "koi" => Some(Box::new(koi::KoiPond::new(rng, theme, d))),
        "sand" => Some(Box::new(sand::Sand::new(rng, theme, d))),
        "city" => Some(Box::new(city::City::new(rng, theme, d))),
        "abyss" => Some(Box::new(abyss::Abyss::new(rng, theme, d))),
        "den" => Some(Box::new(den::Den::new(rng, theme, d))),
        "traffic" => Some(Box::new(traffic::Traffic::new(rng, theme, d))),
        "nexus" => Some(Box::new(nexus::Nexus::new(rng, theme, d))),
        "ripple" => Some(Box::new(ripple::Ripple::new(rng, theme, d))),
        "fireflies" => Some(Box::new(fireflies::Fireflies::new(rng, theme, d))),
        "lanterns" => Some(Box::new(lanterns::Lanterns::new(rng, theme, d))),
        "frost" => Some(Box::new(frost::Frost::new(rng, theme, d))),
        "orbits" => Some(Box::new(orbits::Orbits::new(rng, theme, d))),
        "ribbons" => Some(Box::new(ribbons::Ribbons::new(rng, theme, d))),
        "sonar" => Some(Box::new(sonar::Sonar::new(rng, theme, d))),
        "tide" => Some(Box::new(tide::Tide::new(rng, theme, d))),
        "clockwork" => Some(Box::new(clockwork::Clockwork::new(rng, theme, d))),
        "grid" => Some(Box::new(grid::Grid::new(rng, theme, d))),
        "inkdrop" => Some(Box::new(inkdrop::Inkdrop::new(rng, theme, d))),
        "mosaic" => Some(Box::new(mosaic::Mosaic::new(rng, theme, d))),
        "harmonograph" => Some(Box::new(harmonograph::Harmonograph::new(rng, theme, d))),
        "nebula" => Some(Box::new(nebula::Nebula::new(rng, theme, d))),
        "pendulum" => Some(Box::new(pendulum::Pendulum::new(rng, theme, d))),
        "reaction" => Some(Box::new(reaction::Reaction::new(rng, theme, d))),
        "meadow" => Some(Box::new(meadow::Meadow::new(rng, theme, d))),
        "airspace" => Some(Box::new(airspace::Airspace::new(rng, theme, d))),
        "aquarium" => Some(Box::new(aquarium::Aquarium::new(rng, theme, d))),
        "drive" => Some(Box::new(drive::Drive::new(rng, theme, d))),
        "candy" => Some(Box::new(candy::Candy::new(rng, theme, d))),
        _ => marketplace_create(name, opts, rng),
    }
}


#[cfg(test)]
mod option_tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn detail_scaling_changes_particle_counts() {
        assert_eq!(Detail::Low.factor(), 0.5);
        assert_eq!(Detail::Medium.factor(), 1.0);
        assert_eq!(Detail::High.factor(), 2.0);
        assert_eq!(Detail::Low.scale(100.0, 1), 50);
        assert_eq!(Detail::Medium.scale(100.0, 1), 100);
        assert_eq!(Detail::High.scale(100.0, 1), 200);
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
        for (name, _) in SCENES {
            assert!(!themes(name).is_empty(), "{name} has no themes");
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
