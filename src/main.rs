//! termpaper — Wallpaper Engine for the terminal. Live truecolor worlds.

use termpaper::{color_wheel, config, filter, link, menu, render, scene, transition, wall};

use clap::{Parser, Subcommand};
use config::CycleScope;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use menu::{Effect, Menu, MenuCtx};
use rand::RngExt;
use render::Pixels;
use scene::{Detail, SceneOptions};
use ratatui::{layout::Rect, style::Style, text::Text, widgets::Paragraph};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "termpaper",
    version,
    about = "Wallpaper Engine for the terminal — 120fps, 22 filters, sync clusters, seamless walls",
    after_help = "Press ? in a running scene for the menu; `termpaper list` shows the catalog."
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Scene to run (see `termpaper list`)
    #[arg(help_heading = "Scene", display_order = 0)]
    scene: Option<String>,

    /// Scene color theme (e.g. nexus: cyan/amber/violet/mono)
    #[arg(long, help_heading = "Scene")]
    theme: Option<String>,

    /// Move on to another scene every N seconds
    #[arg(long, value_name = "SECS", help_heading = "Scene")]
    cycle: Option<f64>,

    /// Animation speed multiplier
    #[arg(long, help_heading = "Scene")]
    speed: Option<f32>,

    /// Screensaver mode: any key exits
    #[arg(long, help_heading = "Scene")]
    screensaver: bool,

    /// Post-processing filter, repeatable (e.g. --filter crt --filter bloom)
    #[arg(long, help_heading = "Look")]
    filter: Vec<String>,

    /// Pixel mode: half, quad or braille
    #[arg(long, help_heading = "Look")]
    pixels: Option<String>,

    /// Bump text scale: 1, 2 or 3
    #[arg(long, help_heading = "Look")]
    text_scale: Option<u32>,

    /// Force 256-color output even on truecolor terminals
    #[arg(long, help_heading = "Look")]
    no_truecolor: bool,

    /// Target frames per second
    #[arg(long, help_heading = "Performance")]
    fps: Option<u32>,

    /// Throttle to this fps while the terminal is unfocused (needs a
    /// terminal that reports focus; off unless set)
    #[arg(long, help_heading = "Performance")]
    idle_fps: Option<u32>,

    /// Quality: low, medium or high (particle/layer counts)
    #[arg(long, help_heading = "Performance")]
    detail: Option<String>,

    /// Rendering backend: auto = GPU post-processing when compiled and
    /// available, scenes run on the CPU; gpu = same, but reports if the GPU
    /// is missing; cpu = everything on the CPU; shader = draw every scene
    /// from its experimental WGSL world instead of the Rust scene
    #[arg(long, value_enum, help_heading = "Performance")]
    renderer: Option<termpaper::engine::Renderer>,

    /// Legacy alias for --renderer gpu
    #[arg(long, hide = true)]
    gpu: bool,

    /// Enable instance linking (default on)
    #[arg(long, overrides_with = "no_link", help_heading = "Linking & wall")]
    link: bool,

    /// Disable instance linking
    #[arg(long, help_heading = "Linking & wall")]
    no_link: bool,

    /// Link group for this instance (instances in the same group sync)
    #[arg(long, help_heading = "Linking & wall")]
    group: Option<String>,

    /// Never join a video wall: render the local canvas and hide this
    /// window's geometry from peers (linking still syncs scenes)
    #[arg(long, help_heading = "Linking & wall")]
    no_wall: bool,

    /// Manual video-wall tiling: COLSxROWS:INDEX (e.g. 2x1:0)
    #[arg(long, help_heading = "Linking & wall")]
    wall: Option<String>,

    /// Terminal padding in px (all sides) so wall crops line up across
    /// window borders despite the margin
    #[arg(long, help_heading = "Linking & wall")]
    pad: Option<i32>,

    // Pre-subcommand spellings, kept working but out of --help.
    /// Old spelling of `termpaper list`
    #[arg(long, hide = true)]
    list: bool,

    /// Old spelling of `termpaper instances`
    #[arg(long, hide = true)]
    instances: bool,

    /// Old spelling of `termpaper switch SCENE`
    #[arg(long, hide = true, value_name = "SCENE")]
    switch: Option<String>,

    /// Old spelling of `termpaper switch SCENE --all`
    #[arg(long, hide = true)]
    all_groups: bool,
}

#[derive(Subcommand)]
enum Command {
    /// List scenes by category and exit
    List {
        /// Only this category: coast, wilds, weather, city, cozy, space or classic
        #[arg(long)]
        category: Option<String>,
    },
    /// List live termpaper instances in every group and exit
    Instances,
    /// Switch running instances to a scene and exit
    Switch {
        /// Scene to switch to
        scene: String,
        /// Link group to switch (default: "default")
        #[arg(long)]
        group: Option<String>,
        /// Switch every link group
        #[arg(long, conflicts_with = "group")]
        all: bool,
    },
}

fn print_instances() {
    let list = link::list_all_instances();
    if list.is_empty() {
        println!("no live termpaper instances");
    }
    for i in &list {
        println!(
            "pid {:<8} group {:<10} scene {:<12} up {}s",
            i.pid,
            i.group,
            i.scene,
            link::uptime_secs(i.started_at)
        );
    }
}

fn switch_remote(scene: &str, group: Option<&str>, all: bool) -> std::io::Result<()> {
    if !scene::exists(scene) {
        eprintln!("termpaper: unknown scene '{scene}'. See `termpaper list`.");
        std::process::exit(2);
    }
    let seed: u64 = rand::rng().random();
    let t0 = link::epoch_now_ms();
    if all {
        link::publish_remote_all_groups(scene, seed, t0)?;
        println!("published switch to '{scene}' (all groups)");
    } else {
        let group = group
            .map(link::sanitize_group)
            .unwrap_or_else(|| "default".into());
        link::publish_remote(scene, seed, t0, &group)?;
        println!("published switch to '{scene}' (group {group})");
    }
    Ok(())
}

fn print_list(category: Option<&str>) {
    let only = category.map(|s| {
        scene::Category::parse(&s.to_lowercase()).unwrap_or_else(|| {
            let slugs: Vec<_> = scene::Category::ALL.iter().map(|c| c.slug()).collect();
            eprintln!("termpaper: unknown category '{s}'. Categories: {}", slugs.join(", "));
            std::process::exit(2);
        })
    });
    // one write: piping into `head` shouldn't panic on SIGPIPE
    let mut out = String::new();
    for cat in scene::Category::ALL {
        if only.is_some_and(|c| c != cat) {
            continue;
        }
        let group: Vec<_> = scene::entries().filter(|e| e.category() == cat).collect();
        if group.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{} ({})\n", cat.label(), group.len()));
        for e in group {
            out.push_str(&format!("  {:<14} {}\n", e.name(), e.desc()));
        }
    }
    use std::io::Write;
    let _ = std::io::stdout().write_all(out.as_bytes());
}

fn detect_truecolor() -> bool {
    std::env::var("COLORTERM")
        .map(|v| v.contains("truecolor") || v.contains("24bit"))
        .unwrap_or(false)
}

/// Fallback quality profile when neither CLI nor config sets one.
/// macOS (MacBook thermal/battery headroom) defaults to half-res pixels
/// and 0.5x particle counts; explicit --detail/--pixels/config always win.
fn platform_default_detail() -> Detail {
    if cfg!(target_os = "macos") {
        Detail::Low
    } else {
        Detail::default()
    }
}

fn platform_default_pixels() -> Pixels {
    if cfg!(target_os = "macos") {
        Pixels::Half
    } else {
        Pixels::default()
    }
}

fn main() -> std::io::Result<()> {
    let mut args = Args::parse();

    // termpaper is a color-art program: color output is the entire point.
    // crossterm honors NO_COLOR by stripping all colors, which would render
    // every scene as blank gray blocks — explicitly re-enable colors.
    crossterm::style::Colored::set_ansi_color_disabled(false);

    // the old flags map onto the subcommands (same precedence as before)
    let command = args.command.take().or_else(|| {
        if args.instances {
            Some(Command::Instances)
        } else if let Some(scene) = args.switch.clone() {
            Some(Command::Switch { scene, group: None, all: args.all_groups })
        } else if args.list {
            Some(Command::List { category: None })
        } else {
            None
        }
    });
    match command {
        Some(Command::Instances) => {
            print_instances();
            return Ok(());
        }
        Some(Command::Switch { scene, group, all }) => {
            // `termpaper --group G switch X` works as well as `switch X --group G`
            let group = group.or_else(|| args.group.clone());
            return switch_remote(&scene, group.as_deref(), all);
        }
        Some(Command::List { category }) => {
            print_list(category.as_deref());
            return Ok(());
        }
        None => {}
    }

    let cfg = config::load();
    let names = scene::all_names();

    let detail = args
        .detail
        .as_deref()
        .or(cfg.detail.as_deref())
        .and_then(Detail::parse)
        .unwrap_or_else(platform_default_detail);
    let scene_name = args
        .scene
        .clone()
        .or_else(|| cfg.scene.clone())
        .unwrap_or_else(|| config::DEFAULT_SCENE.to_string());
    if !scene::exists(&scene_name) {
        eprintln!(
            "termpaper: unknown scene '{scene_name}'. Available: {}",
            names.join(", ")
        );
        std::process::exit(2);
    }
    let pixels = args
        .pixels
        .as_deref()
        .or(cfg.pixels.as_deref())
        .and_then(Pixels::parse)
        .unwrap_or_else(platform_default_pixels);
    let keymap = config::KeyMap::new(&cfg);
    let theme = args
        .theme
        .clone()
        .or_else(|| cfg.themes.get(&scene_name).cloned())
        .or_else(|| cfg.theme.clone());
    let text_scale = args.text_scale.or(cfg.text_scale);
    let filters = if args.filter.is_empty() {
        cfg.filters.clone()
    } else {
        args.filter.clone()
    };
    let fps = args.fps.or(cfg.fps).unwrap_or(config::DEFAULT_FPS).clamp(1, 240);
    let idle_fps = args.idle_fps.or(cfg.idle_fps).map(|f| f.clamp(1, 240));
    let speed = args.speed.or(cfg.speed).unwrap_or(1.0);
    let cycle = args.cycle.or(cfg.cycle).filter(|c| *c > 0.0);
    let hue_shift = cfg.hue_shift.unwrap_or(0.0);
    let saturation = cfg.saturation.unwrap_or(1.0);
    let contrast = cfg.contrast.unwrap_or(1.0);
    let link_enabled = if args.no_link {
        false
    } else if args.link {
        true
    } else {
        cfg.link.unwrap_or(true)
    };
    let link_group = args
        .group
        .clone()
        .or(cfg.group.clone())
        .map(|g| link::sanitize_group(&g))
        .unwrap_or_else(|| "default".into());
    let renderer = args.renderer.unwrap_or_else(|| {
        if args.gpu { termpaper::engine::Renderer::Gpu }
        else { cfg.renderer.unwrap_or_else(|| match cfg.gpu {
            Some(true) => termpaper::engine::Renderer::Gpu,
            Some(false) => termpaper::engine::Renderer::Cpu,
            None => termpaper::engine::Renderer::Auto,
        }) }
    });
    let cfg_budget = cfg
        .gpu_budget_ms
        .unwrap_or(termpaper::engine::DEFAULT_GPU_BUDGET_MS)
        .clamp(0.5, 50.0);
    let cfg_shader_fps = cfg.shader_fps.unwrap_or(60).clamp(10, 240);
    let settings = Settings {
        link_enabled,
        link_group,
        wall_spec: if args.no_wall { None } else { args.wall.clone() },
        wall_enabled: !args.no_wall && cfg.wall.unwrap_or(true),
        cli_no_link: args.no_link,
        cli_group: args.group.as_deref().map(link::sanitize_group),
        cli_no_wall: args.no_wall,
        smooth: cfg.smooth.unwrap_or(0.3),
        dim: cfg.dim.unwrap_or(1.0),
        fade: cfg.fade.unwrap_or(0.25),
        clock: cfg.clock.unwrap_or(true),
        pad: args.pad.or(cfg.pad).unwrap_or(0),
        cycle_scope: cfg.cycle_scope.as_deref().map(CycleScope::parse).unwrap_or_default(),
        cfg,
        keymap,
        text_scale,
        theme,
        detail,
        pixels,
        filters,
        fps,
        idle_fps,
        speed,
        cycle,
        hue_shift,
        saturation,
        contrast,
        screensaver: args.screensaver,
        truecolor: detect_truecolor() && !args.no_truecolor,
        renderer,
        gpu_budget_ms: cfg_budget,
        shader_fps: cfg_shader_fps,
    };

    let mut terminal = ratatui::init();
    terminal.hide_cursor()?;
    // focus reporting lets unfocused instances skip the pacing spin (and
    // honor --idle-fps); terminals without support just never send events
    let _ = crossterm::execute!(std::io::stdout(), event::EnableFocusChange);
    let result = run(&mut terminal, &scene_name, settings);
    let _ = crossterm::execute!(std::io::stdout(), event::DisableFocusChange);
    ratatui::restore();
    result
}

struct Settings {
    link_enabled: bool,
    link_group: String,
    wall_spec: Option<String>,
    /// automatic video wall from linked peers' geometry (false = --no-wall
    /// or `wall = false`: local canvas, geometry hidden from peers)
    wall_enabled: bool,
    /// launch-time isolation flags; the `0` reset must not undo them
    cli_no_link: bool,
    cli_group: Option<String>,
    cli_no_wall: bool,
    smooth: f32,
    dim: f32,
    fade: f32,
    clock: bool,
    pad: i32,
    cfg: config::Config,
    theme: Option<String>,
    text_scale: Option<u32>,
    keymap: config::KeyMap,
    detail: Detail,
    pixels: Pixels,
    filters: Vec<String>,
    fps: u32,
    /// fps cap applied while unfocused (None = no throttle)
    idle_fps: Option<u32>,
    speed: f32,
    cycle: Option<f64>,
    /// which scenes `cycle` rotates through
    cycle_scope: CycleScope,
    hue_shift: f32,
    saturation: f32,
    contrast: f32,
    screensaver: bool,
    truecolor: bool,
    renderer: termpaper::engine::Renderer,
    /// GPU ms per frame a Studio scene may use (quality governor budget)
    gpu_budget_ms: f32,
    /// fps cap for Studio scenes: they animate on the shared 60 Hz tick
    shader_fps: u32,
}

/// Snapshot the current runtime settings for broadcast.
/// How long before the frame deadline to stop sleeping and busy-wait.
/// Sized from `examples/pace_bench.rs` — see the frame loop for the tradeoff.
const SPIN_TAIL: Duration = Duration::from_micros(100);

/// Largest single simulation step. Bounds physics after a stall.
const MAX_STEP: f32 = 1.0 / 30.0;
/// Most catch-up steps per frame, so repaying a hitch never starves render.
const MAX_CATCHUP: usize = 4;
/// Debt past this is written off: returning from suspend should not simulate
/// minutes of scene time to catch up.
const MAX_DEBT: f32 = 1.5;

/// Spend owed wall time as bounded simulation steps, draining `debt`.
///
/// Returns the per-step dt values (already speed-scaled) and how many are live.
/// Always at least one step so the frame still redraws when nothing is owed.
fn plan_steps(debt: &mut f32, speed: f32) -> ([f32; MAX_CATCHUP], usize) {
    let mut step_dt = [0.0f32; MAX_CATCHUP];
    let spend = debt.min(MAX_CATCHUP as f32 * MAX_STEP);
    if spend <= 1e-6 {
        // paused, or nothing owed yet — still redraw the current state
        return (step_dt, 1);
    }
    // split the owed time into EQUAL steps rather than MAX_STEP chunks plus
    // a small remainder: verlet velocity is displacement-per-previous-step,
    // so a 1/30 step followed by a 1/300 remainder mis-scales it 10x for a
    // step — a visible speed pulse in every verlet scene after a hitch
    let n = ((spend / MAX_STEP).ceil() as usize).clamp(1, MAX_CATCHUP);
    let s = spend / n as f32;
    for slot in step_dt.iter_mut().take(n) {
        *slot = s * speed;
    }
    *debt -= spend;
    (step_dt, n)
}

fn settings_msg(settings: &Settings, opts: &SceneOptions, quick: &Option<String>) -> link::SettingsMsg {
    link::SettingsMsg {
        pixels: settings.pixels.name().to_string(),
        detail: opts.detail.name().to_string(),
        filters: settings.filters.clone(),
        theme: opts.theme.clone(),
        text_scale: settings.text_scale,
        speed: settings.speed,
        fps: settings.fps,
        smooth: settings.smooth,
        dim: settings.dim,
        fade: settings.fade,
        clock: settings.clock,
        quick: quick.clone(),
        hue_shift: settings.hue_shift,
        saturation: settings.saturation,
        contrast: settings.contrast,
    }
}

/// Merge current runtime settings into the stored config and save it. Only
/// values that differ from the defaults are written, floats rounded to two
/// decimals (see `config::store`). Callers go through `SaveTimer` so rapid
/// changes coalesce into one write.
fn persist(settings: &mut Settings, scene_name: &str, theme: Option<&str>, detail: Detail) {
    // --no-link / --group / --no-wall isolate one launch: while the live
    // value is still the one the flag forced, keep what the config had
    let link = if settings.cli_no_link && !settings.link_enabled {
        settings.cfg.link.unwrap_or(config::DEFAULT_LINK)
    } else {
        settings.link_enabled
    };
    let group = if settings.cli_group.as_deref() == Some(settings.link_group.as_str()) {
        settings.cfg.group.clone().unwrap_or_else(|| config::DEFAULT_GROUP.into())
    } else {
        settings.link_group.clone()
    };
    let wall = if settings.cli_no_wall && !settings.wall_enabled {
        settings.cfg.wall.unwrap_or(true)
    } else {
        settings.wall_enabled
    };
    let live = config::Live {
        scene: scene_name,
        theme,
        pixels: settings.pixels.name(),
        default_pixels: platform_default_pixels().name(),
        detail: detail.name(),
        default_detail: platform_default_detail().name(),
        filters: &settings.filters,
        text_scale: settings.text_scale,
        fps: settings.fps,
        speed: settings.speed,
        smooth: settings.smooth,
        dim: settings.dim,
        fade: settings.fade,
        clock: settings.clock,
        cycle: settings.cycle,
        cycle_scope: settings.cycle_scope,
        hue_shift: settings.hue_shift,
        saturation: settings.saturation,
        contrast: settings.contrast,
        renderer: settings.renderer,
        link,
        group: &group,
        wall,
    };
    config::store(&mut settings.cfg, &live);
    if let Err(e) = config::save(&settings.cfg) {
        eprintln!("termpaper: could not save config: {e}");
    }
}

/// What a browser preview replaced: restored when the preview is dropped,
/// forgotten when it is kept.
struct PreviewOrigin {
    idx: usize,
    theme: Option<String>,
    /// the group anchor the original scene was running on
    sync: Option<(u64, u64)>,
    /// scene being previewed now
    showing: usize,
}

/// The scene the config should remember. A preview is only on loan, so the
/// scene it replaced (with that scene's theme) is what counts.
fn remembered_scene<'a>(
    preview: &'a Option<PreviewOrigin>,
    names: &[&'static str],
    idx: usize,
    opts: &'a SceneOptions,
) -> (&'static str, Option<&'a str>) {
    match preview {
        Some(p) => (names[p.idx], p.theme.as_deref()),
        None => (names[idx], opts.theme.as_deref()),
    }
}

/// Snapshot what the menu shows and adjusts. The status lines and instance
/// list only matter for drawing; key handling passes empties.
fn menu_ctx(
    settings: &Settings,
    opts: &SceneOptions,
    scene_name: &'static str,
    renderer_status: String,
    wall_status: String,
    instances: Vec<String>,
) -> MenuCtx {
    MenuCtx {
        gpu: menu::gpu_state(&renderer_status, cfg!(feature = "gpu"), settings.renderer),
        renderer_status,
        wall_status,
        scene_name,
        pixels: settings.pixels,
        detail: opts.detail,
        theme: opts.theme.clone(),
        text_scale: settings.text_scale,
        speed: settings.speed,
        fps: settings.fps,
        smooth: settings.smooth,
        dim: settings.dim,
        fade: settings.fade,
        clock: settings.clock,
        cycle: settings.cycle,
        cycle_scope: settings.cycle_scope,
        hue_shift: settings.hue_shift,
        saturation: settings.saturation,
        contrast: settings.contrast,
        renderer: settings.renderer,
        link_enabled: settings.link_enabled,
        link_group: settings.link_group.clone(),
        wall_enabled: settings.wall_enabled,
        truecolor: settings.truecolor,
        filters: settings.filters.clone(),
        favorites: settings.cfg.favorites.clone(),
        recents: settings.cfg.recents.clone(),
        scene_themes: settings.cfg.themes.clone(),
        key_display: config::ACTIONS
            .iter()
            .map(|&a| {
                let k = settings
                    .cfg
                    .keys
                    .get(a)
                    .cloned()
                    .unwrap_or_else(|| config::default_key(a).to_string());
                (a.to_string(), k)
            })
            .collect(),
        instances,
    }
}

fn reset_link_guard(
    guard: &mut Option<link::Guard>,
    enabled: bool,
    group: &str,
    scene: &str,
    pad: (i32, i32),
) {
    *guard = None;
    if enabled {
        *guard = link::Guard::new(scene, group).map(|mut g| {
            g.set_pad(pad);
            g
        });
    }
}

/// Link settings the `0` reset restores. `--no-link` and `--group` are
/// launch-time isolation decisions (a solo art piece on one monitor), so a
/// reset never re-enables linking or rejoins the default group on such an
/// instance.
fn link_defaults(cli_no_link: bool, cli_group: Option<&str>) -> (bool, String) {
    (
        config::DEFAULT_LINK && !cli_no_link,
        cli_group.unwrap_or(config::DEFAULT_GROUP).to_string(),
    )
}

fn apply_defaults(
    settings: &mut Settings,
    opts: &mut SceneOptions,
    scene_name: &str,
    transition: &mut transition::Transition,
    guard: &mut Option<link::Guard>,
    quick_filter: &mut Option<String>,
) {
    config::reset_stored_defaults(&mut settings.cfg, scene_name);

    settings.pixels = platform_default_pixels();
    settings.detail = platform_default_detail();
    settings.theme = None;
    settings.filters.clear();
    settings.text_scale = None;
    settings.fps = config::DEFAULT_FPS;
    settings.speed = config::DEFAULT_SPEED;
    settings.smooth = config::DEFAULT_SMOOTH;
    settings.dim = config::DEFAULT_DIM;
    settings.fade = config::DEFAULT_FADE;
    settings.clock = config::DEFAULT_CLOCK;
    settings.cycle = None;
    settings.cycle_scope = CycleScope::All;
    settings.hue_shift = 0.0;
    settings.saturation = 1.0;
    settings.contrast = 1.0;
    let (link_enabled, link_group) = link_defaults(settings.cli_no_link, settings.cli_group.as_deref());
    settings.link_enabled = link_enabled;
    settings.link_group = link_group;
    settings.wall_enabled = !settings.cli_no_wall;

    opts.detail = settings.detail;
    opts.theme = None;
    opts.text_scale = None;
    *quick_filter = None;

    transition.set_fade_secs(settings.fade);
    reset_link_guard(
        guard,
        settings.link_enabled,
        &settings.link_group,
        scene_name,
        (settings.pad, settings.pad),
    );
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    start_scene: &str,
    mut settings: Settings,
) -> std::io::Result<()> {
    let names = scene::all_names();
    let mut guard = if settings.link_enabled {
        link::Guard::new(start_scene, &settings.link_group).map(|mut g| {
            g.set_pad((settings.pad, settings.pad));
            g
        })
    } else {
        None
    };
    // Guard reaps dead sessions; adopt a living group's scene immediately.
    let mut control_stamp = link::Stamp::default();
    // artwork sync: (seed, t0_ms) for the next scene creation, if any
    let mut sync_params: Option<(u64, u64)> = None;
    let mut sync_theme: Option<Option<String>> = None;
    // sync params of the scene currently on screen — lets receivers skip
    // identical re-publishes (heartbeat/duplicates) without a visible
    // restart, and lets the leader re-publish the exact same sim state
    let mut cur_sync: Option<(u64, u64)> = None;
    let mut menu = Menu::new();
    // menu effects queue here and apply at the top of the next frame
    let mut pending_fx: Vec<Effect> = Vec::new();
    // set while the browser previews a scene on this pane only
    let mut preview: Option<PreviewOrigin> = None;
    let mut save = config::SaveTimer::default();
    let mut color_open = false;
    let mut color_param = color_wheel::Param::Hue;
    let mut opts = SceneOptions {
        theme: settings.theme.clone(),
        detail: settings.detail,
        text_scale: settings.text_scale,
        pixels: settings.pixels,
    };
    let mut transition = transition::Transition::new();
    transition.set_fade_secs(settings.fade);
    let mut paused = false;
    // terminals that report focus keep this current; ones that don't never
    // send the events, so it stays true and nothing changes for them
    let mut focused = true;
    let mut quick_filter: Option<String> = None;
    let mut idx = names
        .iter()
        .position(|n| *n == start_scene)
        .unwrap_or(0);
    if let Some(g) = &mut guard {
        if let Some(ctrl) = g.latest_scene() {
            sync_params = Some((ctrl.seed, ctrl.t0_ms));
            cur_sync = sync_params;
            sync_theme = Some(ctrl.theme);
            if let Some(i) = names.iter().position(|n| *n == ctrl.scene) {
                transition.request(i);
            }
        } else {
            let seed = rand::rng().random();
            let t0 = link::epoch_now_ms();
            cur_sync = Some((seed, t0));
            g.publish(names[idx], opts.theme.as_deref(), seed, t0);
        }
    }
    let mut worker = termpaper::engine::Worker::new(settings.renderer);
    let mut rendered: Option<termpaper::engine::Frame> = None;
    // backend status while no frame is coming (a Studio shader compiling)
    let mut worker_status: Option<String> = None;
    let mut local_seed: u64 = rand::rng().random();
    let mut local_elapsed = 0.0f64;
    let mut wall_layout: Option<wall::WallLayout> = None;
    let mut wall_refresh = Instant::now() - Duration::from_secs(10);
    // hyprctl inside the frame loop stalls the frame it lands on
    let mut geo_watcher = if (settings.link_enabled && settings.wall_enabled) || settings.wall_spec.is_some() {
        Some(wall::GeoWatcher::spawn())
    } else {
        None
    };

    // one registry snapshot shared by the heartbeat, wall layout, and menu.
    // Scanning the registry means readdir + a /proc stat per entry + a file
    // read per peer — doing that every frame with the menu open was the
    // hottest path in the whole loop with many instances up.
    struct PeerCache {
        list: Vec<link::InstanceInfo>,
        menu_lines: Vec<String>,
        group: String,
        fetched: Instant,
    }
    let mut peers = PeerCache {
        list: Vec::new(),
        menu_lines: Vec::new(),
        group: String::new(),
        fetched: Instant::now() - Duration::from_secs(10),
    };

    let launch = Instant::now();
    let mut last_switch = Instant::now();
    let mut last_frame = Instant::now();
    // un-simulated wall time carried forward (see `plan_steps`)
    let mut sim_debt = 0.0f32;
    let mut last_heartbeat = Instant::now();
    // cached "HH:MM" for the clock overlay (refreshed at most every 10s)
    let mut clock_text = String::new();
    let mut clock_stamp = Instant::now() - Duration::from_secs(60);

    loop {
        let now = Instant::now();
        let wall_dt = (now - last_frame).as_secs_f32();
        // transition fades are cosmetic: clamp and never carry a remainder
        let raw_dt = wall_dt.min(MAX_STEP);
        last_frame = now;
        if !paused { local_elapsed += wall_dt as f64; }

        // Scene time is owed against the wall clock. Linked instances agree on
        // a scene only because each simulates `now - t0` worth of time, so any
        // time the per-step clamp drops has to be carried forward rather than
        // discarded — otherwise every frame that overruns MAX_STEP leaves this
        // pane permanently behind its peers, and the 15s heartbeat won't repair
        // it (peers already holding this (seed, t0) skip the rebuild).
        let (_step_dt, _n_steps) = if paused {
            // frozen: accrue nothing and spend nothing, but still redraw. Note
            // debt left over from a hitch must not be drained here either, or
            // the scene would keep creeping forward while paused.
            ([0.0f32; MAX_CATCHUP], 1)
        } else {
            sim_debt = (sim_debt + wall_dt).min(MAX_DEBT);
            plan_steps(&mut sim_debt, settings.speed)
        };
        let mut fps_target = match settings.idle_fps {
            Some(idle) if !focused => settings.fps.min(idle),
            _ => settings.fps,
        };
        // Studio scenes change once per 60 Hz tick: more frames would only
        // re-send identical cells
        if scene::lookup(names[idx]).is_some_and(|e| e.needs_gpu()) {
            fps_target = fps_target.min(settings.shader_fps);
        }
        let frame_dur = Duration::from_secs_f64(1.0 / fps_target as f64);

        // menu: fire the preview timer, then apply what the menu asked for
        if menu.open {
            pending_fx.extend(menu.tick(now));
        }
        let mut effects: VecDeque<Effect> = std::mem::take(&mut pending_fx).into();
        while let Some(fx) = effects.pop_front() {
            match fx {
                Effect::SwitchScene(name) => {
                    let Some(i) = names.iter().position(|n| *n == name) else {
                        continue;
                    };
                    menu::browser::push_recent(&mut settings.cfg.recents, name);
                    save.mark(now);
                    let kept = preview.take().is_some() && i == idx && transition.pending().is_none();
                    if kept {
                        // the preview becomes the real scene as it is: its
                        // local clock turns into the group's anchor, so
                        // nothing restarts here or on the peers
                        let t0 = link::epoch_now_ms().saturating_sub((local_elapsed * 1000.0) as u64);
                        cur_sync = Some((local_seed, t0));
                        if let Some(g) = &mut guard {
                            g.set_synced(true);
                            g.publish(name, opts.theme.as_deref(), local_seed, t0);
                        }
                    } else {
                        let seed: u64 = rand::rng().random();
                        let t0 = link::epoch_now_ms();
                        sync_params = Some((seed, t0));
                        let theme = settings.cfg.themes.get(name).cloned()
                            .or_else(|| settings.theme.clone());
                        sync_theme = Some(theme.clone());
                        transition.request(i);
                        if let Some(g) = &mut guard {
                            g.publish(name, theme.as_deref(), seed, t0);
                        }
                    }
                    last_switch = now;
                }
                Effect::Preview(name) => {
                    let Some(i) = names.iter().position(|n| *n == name) else {
                        continue;
                    };
                    let p = preview.get_or_insert_with(|| PreviewOrigin {
                        idx,
                        theme: opts.theme.clone(),
                        sync: cur_sync,
                        showing: idx,
                    });
                    p.showing = i;
                    // local only: no shared anchor and nothing published;
                    // the scene's remembered theme applies at the swap
                    sync_params = None;
                    sync_theme = None;
                    transition.request(i);
                    last_switch = now;
                }
                Effect::EndPreview => {
                    if let Some(p) = preview.take() {
                        // restore only if the preview is still what is on
                        // screen — a peer's switch meanwhile wins
                        if idx == p.showing || transition.pending() == Some(p.showing) {
                            sync_params = p.sync;
                            sync_theme = Some(p.theme);
                            transition.request(p.idx);
                            last_switch = now;
                        }
                    }
                }
                Effect::SetSceneTheme { scene, theme } => {
                    settings.cfg.themes.insert(scene.to_string(), theme.clone());
                    save.mark(now);
                    if let Some(p) = &mut preview {
                        if names[p.idx] == scene {
                            p.theme = Some(theme.clone());
                        }
                    }
                    if scene == names[idx] {
                        if preview.is_some() {
                            // a previewed scene re-themes locally
                            opts.theme = Some(theme);
                        } else {
                            effects.push_front(Effect::SetTheme(Some(theme)));
                        }
                    }
                }
                Effect::ToggleFavorite(name) => {
                    menu::browser::toggle_favorite(&mut settings.cfg.favorites, name);
                    save.mark(now);
                }
                Effect::SetPixels(p) => {
                    settings.pixels = p;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetDetail(d) => {
                    opts.detail = d;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetTheme(t) => {
                    if let Some(g) = &mut guard {
                        let seed: u64 = rand::rng().random();
                        let t0 = link::epoch_now_ms();
                        sync_params = Some((seed, t0));
                        sync_theme = Some(t.clone());
                        transition.request(idx);
                        g.publish(names[idx], t.as_deref(), seed, t0);
                    }
                    opts.theme = t;
                    save.mark(now);
                }
                Effect::SetTextScale(ts) => {
                    settings.text_scale = ts;
                    opts.text_scale = ts;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetSpeed(sp) => {
                    settings.speed = sp;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetFps(fps) => {
                    settings.fps = fps;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetSmooth(sm) => {
                    settings.smooth = sm;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetDim(d) => {
                    settings.dim = d;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetFade(fd) => {
                    settings.fade = fd;
                    transition.set_fade_secs(fd);
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetClock(c) => {
                    settings.clock = c;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetCycle(c) => {
                    // local-only: rotations don't propagate
                    settings.cycle = c;
                    last_switch = now;
                    save.mark(now);
                }
                Effect::SetCycleScope(s) => {
                    settings.cycle_scope = s;
                    save.mark(now);
                }
                Effect::SetRenderer(r) => {
                    // a fresh worker picks the backend; the old one winds
                    // down on its own when dropped
                    settings.renderer = r;
                    worker = termpaper::engine::Worker::new(r);
                    rendered = None;
                    worker_status = None;
                    save.mark(now);
                }
                Effect::SetLink(on) => {
                    settings.link_enabled = on;
                    reset_link_guard(
                        &mut guard,
                        on,
                        &settings.link_group,
                        names[idx],
                        (settings.pad, settings.pad),
                    );
                    if !on && settings.wall_spec.is_none() {
                        // the wall refresh stops with linking: drop the crop now
                        wall_layout = None;
                    }
                    save.mark(now);
                }
                Effect::SetLinkGroup(g) => {
                    settings.link_group = g;
                    reset_link_guard(
                        &mut guard,
                        settings.link_enabled,
                        &settings.link_group,
                        names[idx],
                        (settings.pad, settings.pad),
                    );
                    save.mark(now);
                }
                Effect::SetWall(on) => {
                    settings.wall_enabled = on;
                    if on && geo_watcher.is_none() {
                        geo_watcher = Some(wall::GeoWatcher::spawn());
                    }
                    if !on && settings.wall_spec.is_none() {
                        wall_layout = None;
                    }
                    // re-lay the wall (and republish geometry) next frame
                    wall_refresh = Instant::now() - Duration::from_secs(10);
                    save.mark(now);
                }
                Effect::ToggleFilter(f) => {
                    if let Some(pos) = settings.filters.iter().position(|x| *x == f) {
                        settings.filters.remove(pos);
                    } else {
                        settings.filters.push(f);
                    }
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::SetFilters(v) => {
                    settings.filters = v;
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::OpenColorGrade => {
                    color_open = true;
                    color_param = color_wheel::Param::Hue;
                }
                // placeholder until the alignment tool lands
                Effect::OpenCalibration => menu.flash("Align monitors: coming soon"),
            }
        }
        // config writes coalesce while the menu or wheel is up; otherwise
        // (including right after either closes) they go out immediately
        let write = if menu.open || color_open { save.due(now) } else { save.take() };
        if write {
            let (scene_name, theme) = remembered_scene(&preview, &names, idx, &opts);
            persist(&mut settings, scene_name, theme, opts.detail);
        }

        // scene cycling
        if let Some(secs) = settings.cycle {
            if now.duration_since(last_switch).as_secs_f64() >= secs {
                sync_params = None; // local cycle: no shared seed
                cur_sync = None;
                if let Some(g) = &mut guard {
                    g.set_synced(false);
                }
                transition.request(menu::browser::cycle_next(
                    &names,
                    idx,
                    settings.cycle_scope,
                    &settings.cfg.favorites,
                ));
                last_switch = now;
            }
        }

        // refresh the shared registry snapshot at most every 2s (1s while
        // the menu is open so its instance list feels live)
        if settings.link_enabled {
            let max_age = if menu.open { Duration::from_secs(1) } else { Duration::from_secs(2) };
            if peers.fetched.elapsed() > max_age || peers.group != settings.link_group {
                peers.fetched = Instant::now();
                peers.group = settings.link_group.clone();
                peers.list = link::list_instances_in_group(&settings.link_group);
                peers.menu_lines = peers
                    .list
                    .iter()
                    .map(|i| {
                        format!(
                            "pid {:<8} {:<12} up {}s{}",
                            i.pid,
                            i.scene,
                            link::uptime_secs(i.started_at),
                            if i.pid == std::process::id() { " (you)" } else { "" }
                        )
                    })
                    .collect();
            }
        }

        // sync heartbeat: the lowest-pid live instance re-publishes the
        // current scene with its ORIGINAL (seed, t0) every 15s — late
        // joiners adopt it and drifted peers re-align, while in-sync
        // receivers skip the identical control without a visible restart
        if let Some(g) = &mut guard {
            if now.duration_since(last_heartbeat).as_secs() >= 15 {
                last_heartbeat = now;
                if let Some((seed, t0)) = cur_sync {
                    // a peer that cycled locally holds no shared anchor, so it
                    // must not win the election and then publish nothing
                    let leader = peers
                        .list
                        .iter()
                        .filter(|i| i.synced)
                        .map(|i| i.pid)
                        .min()
                        .map(|m| m == std::process::id())
                        .unwrap_or(false);
                    if leader {
                        g.publish(names[idx], opts.theme.as_deref(), seed, t0);
                    }
                }
            }
        }

        // instance linking: control channel poll — a stat per frame; the
        // file is only read+parsed when a publish replaced it
        if let Some(g) = &mut guard {
            if let Some(ctrl) = g.poll_control(control_stamp) {
                control_stamp = link::Stamp {
                    epoch: ctrl.epoch,
                    seq: ctrl.seq,
                    from_pid: ctrl.from_pid,
                };
                match ctrl.kind {
                    link::ControlKind::Scene => {
                        if let Some(i) = names.iter().position(|n| *n == ctrl.scene) {
                            // identical re-publish (heartbeat/duplicate) —
                            // already running this exact sim, skip silently
                            let identical = i == idx && cur_sync == Some((ctrl.seed, ctrl.t0_ms));
                            if !identical {
                                sync_theme = Some(ctrl.theme);
                                sync_params = Some((ctrl.seed, ctrl.t0_ms));
                                transition.request(i);
                                last_switch = Instant::now();
                            }
                        }
                    }
                    link::ControlKind::Settings => {
                        // session-only apply: never persisted to config
                        if let Some(m) = ctrl.settings {
                            if let Some(p) = render::Pixels::parse(&m.pixels) {
                                settings.pixels = p;
                            }
                            settings.filters = m.filters;
                            settings.speed = m.speed;
                            settings.fps = m.fps;
                            settings.smooth = m.smooth;
                            settings.dim = m.dim;
                            settings.fade = m.fade;
                            settings.clock = m.clock;
                            transition.set_fade_secs(m.fade);
                            quick_filter = m.quick;
                            settings.hue_shift = m.hue_shift;
                            settings.saturation = m.saturation;
                            settings.contrast = m.contrast;
                            let mut recreate = false;
                            if let Some(d) = scene::Detail::parse(&m.detail) {
                                if d != opts.detail {
                                    opts.detail = d;
                                    recreate = true;
                                }
                            }
                            if m.theme != opts.theme {
                                opts.theme = m.theme;
                                recreate = true;
                            }
                            if m.text_scale != settings.text_scale {
                                settings.text_scale = m.text_scale;
                                opts.text_scale = m.text_scale;
                                recreate = true;
                            }
                            if recreate && cur_sync.is_none() {
                                transition.request(idx);
                                sync_params = None;
                            }
                        }
                    }
                }
            }
        }

        // transition: fade out → swap → fade in
        let (fade, swap) = transition.tick(raw_dt);
        if let Some(i) = swap {
            idx = i % names.len();
            opts.theme = sync_theme.take().unwrap_or_else(|| {
                settings.cfg.themes.get(names[idx]).cloned()
                    .or_else(|| settings.theme.clone())
            });
            // artwork sync: linked switches carry (seed, t0) so every
            // instance builds the identical simulation
            let sp = sync_params.take();
            cur_sync = sp;
            local_seed = rand::rng().random();
            local_elapsed = 0.0;
            sim_debt = 0.0;
            if let Some(g) = &mut guard {
                g.set_scene(names[idx]);
                g.set_synced(cur_sync.is_some());
            }
            last_switch = now;
        }

        // video wall: refresh layout every 2s from registry geometry
        if (settings.link_enabled || settings.wall_spec.is_some())
            && wall_refresh.elapsed() > Duration::from_secs(2)
        {
            wall_refresh = Instant::now();
            let (cols, rows) = (terminal.size()?.width as usize, terminal.size()?.height as usize);
            // a --no-wall pane publishes no geometry, so peers never fold it
            // into their wall either
            let geo = geo_watcher
                .as_ref()
                .filter(|_| settings.wall_enabled)
                .and_then(|w| w.latest());
            if let Some(g) = &mut guard {
                g.set_geometry(cols, rows, geo.map(|g| (g.x, g.y, g.w, g.h)));
            }
            wall_layout = if let Some(spec) = &settings.wall_spec {
                wall::manual_layout(spec, cols, rows)
            } else if settings.link_enabled && settings.wall_enabled {
                let mut parts: Vec<wall::Participant> = peers
                    .list
                    .iter()
                    .filter_map(|i| {
                        i.geo.map(|(x, y, w, h)| wall::Participant {
                            pid: i.pid,
                            geo: wall::Geo { x, y, w, h },
                            cols: i.cols.max(1),
                            rows: i.rows.max(1),
                            pad: i.pad,
                        })
                    })
                    .collect();
                // include ourselves even if hyprctl is unavailable to others
                if let Some(g) = geo {
                    if !parts.iter().any(|p| p.pid == std::process::id()) {
                        parts.push(wall::Participant {
                            pid: std::process::id(),
                            geo: g,
                            cols,
                            rows,
                            pad: (settings.pad, settings.pad),
                        });
                    }
                }
                wall::compute_layout(parts, std::process::id())
            } else {
                None
            };
            if let Some(l) = wall_layout {
                if l.too_big {
                    wall_layout = None; // guard: virtual area too big, stay local
                }
            }
        }

        // clock overlay: one cheap `date` call per 10s, handles TZ/DST
        if settings.clock && clock_stamp.elapsed() > Duration::from_secs(10) {
            clock_stamp = Instant::now();
            clock_text = std::process::Command::new("date")
                .arg("+%H:%M")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
        }

        terminal.draw(|f| {
            let area = f.area();
            let (pw, ph) = settings.pixels.cell_size();
            let size = match wall_layout {
                Some(l) => (l.virtual_w * pw, l.virtual_h * ph),
                None => (area.width as usize * pw, area.height as usize * ph),
            };
            let crop = wall_layout.map(|l| (l.crop_x * pw, l.crop_y * ph)).unwrap_or((0, 0));
            let (seed, elapsed_ms) = cur_sync.map(|(s, t0)| (s, link::epoch_now_ms().saturating_sub(t0)))
                .unwrap_or((local_seed, (local_elapsed * 1000.0) as u64));
            let request = termpaper::engine::Request {
                generation: 0,
                key: termpaper::engine::SceneKey {
                    name: names[idx].into(), seed, opts: opts.clone(), size,
                    grid: (area.width as usize, area.height as usize), crop, pixels: settings.pixels,
                },
                elapsed_ms, speed: settings.speed, paused, filters: settings.filters.clone(),
                quick: quick_filter.clone(), hue: settings.hue_shift, saturation: settings.saturation,
                contrast: settings.contrast, dim: fade * settings.dim, smooth: settings.smooth,
                budget_ms: settings.gpu_budget_ms,
                prefetch: transition.pending().map(|i| names[i % names.len()].to_string()),
            };
            if let Some(frame) = worker.submit(request) { rendered = Some(frame); worker_status = None; }
            if let Some(st) = worker.take_status() { worker_status = Some(st); }
            if rendered.as_ref().is_some_and(|frame| frame.generation != worker.generation()) {
                rendered = None;
            }
            if let Some(frame) = &rendered {
                #[allow(unused_mut)]
                let mut drawn = false;
                #[cfg(feature = "gpu")]
                if let Some(words) = &frame.cells {
                    let cells = termpaper::gpu::FrameCells {
                        words, cols: area.width as usize, rows: area.height as usize,
                    };
                    termpaper::gpu::blit(&cells, &frame.canvas, crop, settings.pixels, area,
                        f.buffer_mut(), settings.truecolor);
                    drawn = true;
                }
                if !drawn {
                    render::draw_crop(&frame.canvas, crop.0 as i32, crop.1 as i32, area,
                        f.buffer_mut(), settings.truecolor, settings.pixels);
                }
            }
            // bottom-left hint, fading out over its last second
            let hint_age = launch.elapsed().as_secs_f32();
            if hint_age < 4.0 && !menu.open {
                let a = 1.0 - (hint_age - 3.0).clamp(0.0, 1.0);
                let g = (90.0 * a) as u8;
                if g > 8 {
                    let hint = format!("? menu · c color grade · ←/→ scene · q quit · {}", names[idx]);
                    let rect = Rect {
                        x: area.x,
                        y: area.y + area.height.saturating_sub(1),
                        width: area.width.min(hint.len() as u16 + 2),
                        height: 1,
                    };
                    f.render_widget(
                        Paragraph::new(Text::raw(hint))
                            .style(Style::new().fg(ratatui::style::Color::Rgb(g, g, g + 10))),
                        rect,
                    );
                }
            }

            // top-right clock, dim gray scaled by the dim setting
            if settings.clock && !clock_text.is_empty() {
                let g = (90.0 * settings.dim) as u8;
                let b = (100.0 * settings.dim) as u8;
                if g > 8 {
                    let rect = Rect {
                        x: area.x + area.width.saturating_sub(6),
                        y: area.y,
                        width: area.width.min(6),
                        height: 1,
                    };
                    f.render_widget(
                        Paragraph::new(Text::raw(clock_text.clone()))
                            .alignment(ratatui::layout::Alignment::Right)
                            .style(Style::new().fg(ratatui::style::Color::Rgb(g, g, b))),
                        rect,
                    );
                }
            }

            // the menu floats over the live scene
            if menu.open {
                let status = worker_status.clone().or_else(|| rendered.as_ref().map(|f| format!("{} · worker {:.1} ms", f.backend, f.render_ms)))
                    .unwrap_or_else(|| "Renderer initializing…".into());
                let wall_status = match wall_layout {
                    Some(l) => format!("wall: {}x{} cells @ ({},{})", l.virtual_w, l.virtual_h, l.crop_x, l.crop_y),
                    None => "wall: local".into(),
                };
                let instances = if settings.link_enabled {
                    peers.menu_lines.clone()
                } else {
                    vec!["linking disabled (solo art)".into()]
                };
                let ctx = menu_ctx(&settings, &opts, names[idx], status, wall_status, instances);
                menu::view::render(f, area, &menu, &ctx);
            }
            if color_open {
                color_wheel::render(
                    f,
                    area,
                    settings.hue_shift,
                    settings.saturation,
                    settings.contrast,
                    color_param,
                );
            }
        })?;

        // steady pacing: coarse sleep to ~1ms before the deadline, then
        // Sleep (inside the event poll) to just before the deadline, then spin
        // the last SPIN_TAIL for steady timing. See examples/pace_bench.rs: at
        // 240fps a 1ms tail costs ~22% of a core per instance purely spinning,
        // and several instances pacing on one machine then starve each other
        // into dropped frames. 100us holds jitter at ~0.025ms — 0.6% of a
        // 240fps frame — for ~1% CPU.
        let deadline = last_frame + frame_dur;
        let mut events_handled = 0;
        loop {
            let now2 = Instant::now();
            // Rendering may already have exhausted the frame budget. Always
            // poll input anyway; otherwise expensive scenes become inescapable.
            if events_handled >= 64 {
                break;
            }
            let remaining = deadline.saturating_duration_since(now2);
            let coarse = remaining.saturating_sub(SPIN_TAIL);
            let has_event = if coarse.is_zero() {
                event::poll(Duration::ZERO)?
            } else {
                event::poll(coarse)?
            };
            if has_event {
                events_handled += 1;
                let ev = event::read()?;
                match ev {
                    Event::FocusGained => {
                        focused = true;
                        continue;
                    }
                    Event::FocusLost => {
                        focused = false;
                        continue;
                    }
                    _ => {}
                }
                if let Event::Key(key) = ev {
                if key.kind == KeyEventKind::Press {
                    let km = &settings.keymap;

                    if color_open {
                        if key.code == KeyCode::Esc || km.matches("color", key.code) {
                            color_open = false;
                            save.mark(Instant::now());
                            break;
                        }
                        if key.code == KeyCode::Tab {
                            color_param = match color_param {
                                color_wheel::Param::Hue => color_wheel::Param::Saturation,
                                color_wheel::Param::Saturation => color_wheel::Param::Contrast,
                                color_wheel::Param::Contrast => color_wheel::Param::Hue,
                            };
                            break;
                        }
                        if key.code == KeyCode::Char('0') {
                            settings.hue_shift = 0.0;
                            settings.saturation = 1.0;
                            settings.contrast = 1.0;
                            if let Some(g) = &mut guard {
                                g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                            }
                            break;
                        }
                        let fine = match key.code {
                            KeyCode::Left => Some(-1),
                            KeyCode::Right => Some(1),
                            KeyCode::Up => Some(10),
                            KeyCode::Down => Some(-10),
                            _ => None,
                        };
                        if let Some(steps) = fine {
                            match color_param {
                                color_wheel::Param::Hue => {
                                    settings.hue_shift = color_wheel::step_hue_steps(
                                        settings.hue_shift,
                                        steps,
                                    );
                                }
                                color_wheel::Param::Saturation => {
                                    settings.saturation = color_wheel::step_sat(
                                        settings.saturation,
                                        steps as f32 * color_wheel::SAT_STEP,
                                    );
                                }
                                color_wheel::Param::Contrast => {
                                    settings.contrast = color_wheel::step_contrast(
                                        settings.contrast,
                                        steps as f32 * color_wheel::CONTRAST_STEP,
                                    );
                                }
                            }
                            if let Some(g) = &mut guard {
                                g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                            }
                            break;
                        }
                        continue;
                    }

                    // Ctrl-C quits from anywhere, menu or not
                    let ctrl_c = key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL);

                    if menu.open && !ctrl_c {
                        // modal: every key goes to the menu — including `0`,
                        // so the reset below can't fire from inside it
                        let remapped_close = !menu.typing()
                            && key.code != KeyCode::Char('?')
                            && km.matches("menu", key.code);
                        if remapped_close {
                            pending_fx.extend(menu.close());
                        } else if let Some(input) = menu::Input::from_key(key) {
                            let ctx = menu_ctx(&settings, &opts, names[idx], String::new(), String::new(), Vec::new());
                            pending_fx.extend(menu.handle(input, &ctx));
                        }
                        // apply before the next key reads a stale snapshot
                        if !pending_fx.is_empty() || !menu.open {
                            break;
                        }
                        continue;
                    }

                    if km.matches("reset", key.code) {
                        apply_defaults(
                            &mut settings,
                            &mut opts,
                            names[idx],
                            &mut transition,
                            &mut guard,
                            &mut quick_filter,
                        );
                        save.take();
                        persist(&mut settings, names[idx], opts.theme.as_deref(), opts.detail);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(
                                &settings,
                                &opts,
                                &quick_filter,
                            ));
                        }
                        continue;
                    }

                    let quit = settings.screensaver
                        || km.matches("quit", key.code)
                        || matches!(key.code, KeyCode::Esc)
                        || ctrl_c;
                    if quit {
                        if save.take() {
                            let (scene_name, theme) = remembered_scene(&preview, &names, idx, &opts);
                            persist(&mut settings, scene_name, theme, opts.detail);
                        }
                        return Ok(());
                    }
                    if km.matches("menu", key.code) {
                        let ctx = menu_ctx(&settings, &opts, names[idx], String::new(), String::new(), Vec::new());
                        menu.open(&ctx);
                        continue;
                    }
                    if km.matches("color", key.code) {
                        color_open = !color_open;
                        if color_open {
                            color_param = color_wheel::Param::Hue;
                            if settings.hue_shift < 0.5
                                && (settings.saturation - 1.0).abs() < 0.02
                                && (settings.contrast - 1.0).abs() < 0.02
                            {
                                settings.hue_shift = 45.0;
                            }
                        } else {
                            save.mark(Instant::now());
                        }
                        continue;
                    }
                    let mut switch_to = |i: usize, _: &mut usize| {
                        let i = i % names.len();
                        // publisher picks the seed + start time; both sides
                        // will build the scene from them
                        let seed: u64 = rand::rng().random();
                        let t0 = link::epoch_now_ms();
                        sync_params = Some((seed, t0));
                        let theme = settings.cfg.themes.get(names[i]).cloned()
                            .or_else(|| settings.theme.clone());
                        sync_theme = Some(theme.clone());
                        transition.request(i);
                        if let Some(g) = &mut guard {
                            g.publish(names[i], theme.as_deref(), seed, t0);
                        }
                        last_switch = Instant::now();
                    };
                    if km.matches("next", key.code) || key.code == KeyCode::Tab {
                        let i = (idx + 1) % names.len();
                        switch_to(i, &mut idx);
                    } else if km.matches("prev", key.code) {
                        let i = (idx + names.len() - 1) % names.len();
                        switch_to(i, &mut idx);
                    } else if km.matches("pause", key.code) {
                        paused = !paused;
                    } else if km.matches("filter_next", key.code) {
                        if quick_filter.as_deref() == Some("smooth-off") {
                            settings.smooth = 0.3;
                            quick_filter = None;
                            continue;
                        }
                        quick_filter = match quick_filter.as_deref() {
                            None => Some(filter::FILTER_CYCLE[0].to_string()),
                            Some(cur) => filter::FILTER_CYCLE
                                .iter()
                                .position(|&f| f == cur)
                                .map(|i| {
                                    if i + 1 < filter::FILTER_CYCLE.len() {
                                        Some(filter::FILTER_CYCLE[i + 1].to_string())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(None),
                        };
                        // publish AFTER mutating so peers get the new state
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                    } else if km.matches("detail_next", key.code) {
                        opts.detail = opts.detail.next();
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                    } else if km.matches("fps_up", key.code) {
                        settings.fps = menu::fps_step(settings.fps, true);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                        save.mark(Instant::now());
                    } else if km.matches("fps_down", key.code) {
                        settings.fps = menu::fps_step(settings.fps, false);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                        save.mark(Instant::now());
                    } else if km.matches("speed_up", key.code) {
                        settings.speed = menu::speed_step(settings.speed, true);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                        save.mark(Instant::now());
                    } else if km.matches("speed_down", key.code) {
                        settings.speed = menu::speed_step(settings.speed, false);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                        save.mark(Instant::now());
                    }
                }
                }
                // event handled: keep polling until the deadline so every
                // queued key drains this frame — a held key used to feed
                // through at one press per frame, backing up the terminal
                // buffer and replaying long after release
                continue;
            }
            // spin the tail for steady frame timing — but only while
            // focused: an unfocused wallpaper trades ~1ms of pacing jitter
            // for not burning a spinning core per instance
            if focused {
                while Instant::now() < deadline {
                    std::hint::spin_loop();
                }
            }
            break;
        }
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;

    /// A normal frame simulates exactly the wall time that passed.
    #[test]
    fn steady_frame_spends_all_its_time() {
        let mut debt = 1.0 / 120.0;
        let (dts, n) = plan_steps(&mut debt, 1.0);
        assert_eq!(n, 1);
        assert!((dts[0] - 1.0 / 120.0).abs() < 1e-6);
        assert!(debt < 1e-6, "steady frames must leave nothing owed");
    }

    /// The core sync property: over many frames, simulated time tracks wall
    /// time even when individual frames overrun MAX_STEP. Before the catch-up
    /// the clamp silently discarded the overrun and the pane fell behind.
    #[test]
    fn hitches_are_repaid_so_sim_time_tracks_wall_time() {
        let mut debt = 0.0f32;
        let mut simulated = 0.0f32;
        let mut wall = 0.0f32;
        // 200 good frames at 120fps with a 150ms stall every 20th
        for i in 0..200 {
            let frame = if i % 20 == 19 { 0.150 } else { 1.0 / 120.0 };
            wall += frame;
            debt = (debt + frame).min(MAX_DEBT);
            let (dts, n) = plan_steps(&mut debt, 1.0);
            simulated += dts.iter().take(n).sum::<f32>();
        }
        // whatever is still owed is bounded by one frame's worth of steps
        let drift = (wall - simulated - debt).abs();
        assert!(drift < 1e-3, "sim time drifted from wall time by {drift}s");
        assert!(
            debt < MAX_CATCHUP as f32 * MAX_STEP,
            "debt should stay bounded, got {debt}"
        );
    }

    /// No single step may exceed MAX_STEP, or a stall would blow up physics.
    #[test]
    fn no_single_step_exceeds_the_clamp() {
        let mut debt = 5.0; // absurd stall
        let (dts, n) = plan_steps(&mut debt, 1.0);
        for s in dts.iter().take(n) {
            assert!(*s <= MAX_STEP + 1e-6, "step {s} exceeds MAX_STEP");
        }
        assert_eq!(n, MAX_CATCHUP, "a big stall should use the full budget");
    }

    /// Debt is capped, so resuming from suspend does not simulate minutes.
    #[test]
    fn debt_is_capped_for_suspend() {
        let mut debt = 0.0f32;
        debt = (debt + 3600.0).min(MAX_DEBT);
        assert_eq!(debt, MAX_DEBT);
        let mut total = 0.0;
        // draining is bounded: a handful of frames, not an hour of simulation
        for _ in 0..20 {
            let (dts, n) = plan_steps(&mut debt, 1.0);
            total += dts.iter().take(n).sum::<f32>();
        }
        assert!(total <= MAX_DEBT + 1e-3, "drained {total}s, cap is {MAX_DEBT}");
        assert!(debt < 1e-6, "cap should fully drain within 20 frames");
    }

    /// Nothing owed still yields one redraw step (the paused path in the frame
    /// loop bypasses `plan_steps` entirely so leftover debt is not drained).
    #[test]
    fn idle_still_yields_a_redraw_step() {
        let mut debt = 0.0f32;
        let (dts, n) = plan_steps(&mut debt, 1.0);
        assert_eq!(n, 1);
        assert_eq!(dts[0], 0.0);
    }

    /// Pausing must not spend debt carried in from a hitch.
    #[test]
    fn pausing_preserves_outstanding_debt() {
        let mut debt = 0.5f32;
        let before = debt;
        // the loop's paused branch: no accrual, no plan_steps call
        let (dts, n) = ([0.0f32; MAX_CATCHUP], 1);
        assert_eq!(n, 1);
        assert_eq!(dts[0], 0.0);
        assert_eq!(debt, before, "paused frames must leave debt untouched");
        // and it is still there to repay on resume
        let (dts, n) = plan_steps(&mut debt, 1.0);
        assert!(dts.iter().take(n).sum::<f32>() > 0.0);
    }

    /// Catch-up steps are equal-sized: a hitch must never emit a big step
    /// followed by a tiny remainder, or verlet velocity (displacement per
    /// previous step) mis-scales for one step and the scene visibly pulses.
    #[test]
    fn catchup_steps_are_equal_sized() {
        let mut debt = MAX_STEP + MAX_STEP / 10.0; // just past one step
        let (dts, n) = plan_steps(&mut debt, 1.0);
        assert_eq!(n, 2);
        assert!(
            (dts[0] - dts[1]).abs() < 1e-6,
            "steps should be equal, got {} and {}",
            dts[0],
            dts[1]
        );
        assert!((dts[0] + dts[1] - (MAX_STEP + MAX_STEP / 10.0)).abs() < 1e-6);
    }

    /// `--speed` scales simulated time without changing the debt accounting.
    #[test]
    fn speed_scales_steps_only() {
        let mut debt = 1.0 / 60.0;
        let (dts, n) = plan_steps(&mut debt, 4.0);
        assert_eq!(n, 1);
        assert!((dts[0] - 4.0 / 60.0).abs() < 1e-6);
        assert!(debt < 1e-6);
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(argv: &[&str]) -> Args {
        Args::try_parse_from(std::iter::once("termpaper").chain(argv.iter().copied()))
            .unwrap_or_else(|e| panic!("{argv:?}: {e}"))
    }

    #[test]
    fn clap_definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn positional_scene_still_works() {
        let a = parse(&["koi", "--fps", "60"]);
        assert_eq!(a.scene.as_deref(), Some("koi"));
        assert!(a.command.is_none());
        assert_eq!(a.fps, Some(60));
    }

    #[test]
    fn subcommands_parse() {
        match parse(&["list", "--category", "coast"]).command {
            Some(Command::List { category }) => assert_eq!(category.as_deref(), Some("coast")),
            _ => panic!("expected list"),
        }
        assert!(matches!(parse(&["instances"]).command, Some(Command::Instances)));
        match parse(&["switch", "fire", "--group", "art"]).command {
            Some(Command::Switch { scene, group, all }) => {
                assert_eq!((scene.as_str(), group.as_deref(), all), ("fire", Some("art"), false));
            }
            _ => panic!("expected switch"),
        }
        assert!(matches!(
            parse(&["switch", "fire", "--all"]).command,
            Some(Command::Switch { all: true, .. })
        ));
        assert!(Args::try_parse_from(["termpaper", "switch", "fire", "--all", "--group", "x"]).is_err());
    }

    #[test]
    fn old_flags_are_hidden_aliases() {
        let a = parse(&["--list"]);
        assert!(a.list && a.command.is_none());
        let a = parse(&["--switch", "fire", "--all-groups"]);
        assert_eq!(a.switch.as_deref(), Some("fire"));
        assert!(a.all_groups);
        assert!(parse(&["--instances"]).instances);
        assert!(parse(&["--gpu"]).gpu);
        let help = Args::command().render_long_help().to_string();
        for hidden in ["--list", "--instances", "--switch", "--all-groups", "--gpu"] {
            assert!(!help.contains(hidden), "{hidden} should be hidden from --help");
        }
        for heading in ["Scene:", "Look:", "Performance:", "Linking & wall:"] {
            assert!(help.contains(heading), "missing heading {heading}\n{help}");
        }
    }
}
