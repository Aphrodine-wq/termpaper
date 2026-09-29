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

    /// Terminal padding so wall crops line up across window borders despite
    /// the margin: px (`7`), points (`3.5pt`, e.g. kitty's
    /// window_padding_width), or `x,y`
    #[arg(long, help_heading = "Linking & wall")]
    pad: Option<String>,

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
    /// Show the physical desk (monitors in millimetres) and the wall plan
    Desk,
    /// Line the monitors up: every wall pane shows a millimetre test
    /// pattern while this terminal nudges monitor offsets, bezels and scale
    Calibrate,
    /// Start or stop one wall terminal per monitor
    Wall {
        #[command(subcommand)]
        action: WallCmd,
    },
}

#[derive(Subcommand)]
enum WallCmd {
    /// Open a kitty on every monitor (fonts matched to pixel pitch, zero
    /// padding) running termpaper in one group; skips monitors that have one
    Up {
        /// Print the commands instead of running them
        #[arg(long)]
        dry_run: bool,
        /// Font size in points on the reference (first landscape) monitor
        #[arg(long, default_value_t = 11.0)]
        font: f32,
        /// Only these outputs (repeatable), e.g. --monitor DP-1
        #[arg(long)]
        monitor: Vec<String>,
        /// Extra arguments for each termpaper (after --)
        #[arg(last = true)]
        extra: Vec<String>,
    },
    /// Close every wall terminal
    Down,
}

/// `termpaper desk`: the desk, and the group's current plan if any.
fn print_desk(group: &str) {
    let Some(mons) = termpaper::hypr::monitors() else {
        eprintln!("termpaper: not running under Hyprland");
        std::process::exit(1);
    };
    let cfg = termpaper::desk::load_desk();
    let desk = termpaper::desk::Desk::from_hypr(&mons, &cfg);
    print!("{}", termpaper::desk::describe(&desk));
    println!(
        "align {} · bezel {} mm · frame {} · portrait {} · config {}",
        cfg.align.name(),
        cfg.bezel_mm,
        cfg.frame,
        cfg.portrait,
        termpaper::desk::desk_path().map(|p| p.display().to_string()).unwrap_or_default()
    );
    let plan = link::group_dir(group).and_then(|d| termpaper::wallplan::WallPlan::load(&d));
    match plan {
        Some(p) => {
            println!(
                "\nwall plan (group {group}, rev {}, leader {}): frame {:.0}x{:.0} mm, classic canvas {}x{} cells",
                p.rev, p.leader, p.frame.w, p.frame.h, p.classic_cells.0, p.classic_cells.1
            );
            for pp in &p.panes {
                println!(
                    "  pid {:<8} {:<10} content x {:>7.1} y {:>6.1} w {:>6.1} h {:>6.1} mm{}",
                    pp.pid,
                    pp.monitor,
                    pp.content.x,
                    pp.content.y,
                    pp.content.w,
                    pp.content.h,
                    if pp.portrait { "  (portrait)" } else { "" }
                );
            }
        }
        None => println!("\nno wall plan in group {group} (start panes with `termpaper wall up`)"),
    }
}

/// The group's panes as the wall planner sees them.
fn panes_of(group: &str) -> Vec<termpaper::wallplan::PaneGeom> {
    link::list_instances_in_group(group)
        .into_iter()
        .filter(|i| i.geo.is_some())
        .map(|i| termpaper::wallplan::PaneGeom {
            pid: i.pid,
            ancestors: termpaper::hypr::ancestors(i.pid),
            cols: i.cols.min(u16::MAX as usize) as u16,
            rows: i.rows.min(u16::MAX as usize) as u16,
            cell: i.cell.map(|(w, h)| (w as f64, h as f64)),
            pad: (i.pad.0 as f64, i.pad.1 as f64),
            centered: i.placement == wall::Placement::Center,
        })
        .collect()
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

fn switch_remote(scene: &str, group: Option<&str>, all: bool, args: &Args) -> std::io::Result<()> {
    if !scene::exists(scene) {
        eprintln!("termpaper: unknown scene '{scene}'. See `termpaper list`.");
        std::process::exit(2);
    }
    let seed: u64 = rand::rng().random();
    // a short fade-out lead, so running panes swap together
    let t0 = link::epoch_now_ms() + (config::DEFAULT_FADE * 1000.0) as u64 + SWITCH_MARGIN_MS;
    // sim settings for a group that has no anchor yet: this config's
    let cfg = config::load();
    let defaults = link::Anchor {
        stamp: link::Stamp::default(),
        scene: scene.to_string(),
        theme: None,
        seed,
        t0_ms: t0,
        paused_at_ms: None,
        speed: args.speed.or(cfg.speed).unwrap_or(config::DEFAULT_SPEED),
        detail: args.detail.as_deref().or(cfg.detail.as_deref()).and_then(Detail::parse)
            .unwrap_or_else(platform_default_detail).name().to_string(),
        pixels: args.pixels.as_deref().or(cfg.pixels.as_deref()).and_then(Pixels::parse)
            .unwrap_or_else(platform_default_pixels).name().to_string(),
        text_scale: args.text_scale.or(cfg.text_scale),
        proto: link::PROTO,
    };
    if all {
        link::publish_remote_all_groups(scene, seed, t0, &defaults)?;
        println!("published switch to '{scene}' (all groups)");
    } else {
        let group = group
            .map(link::sanitize_group)
            .unwrap_or_else(|| "default".into());
        link::publish_remote(scene, seed, t0, &group, &defaults)?;
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
            return switch_remote(&scene, group.as_deref(), all, &args);
        }
        Some(Command::List { category }) => {
            print_list(category.as_deref());
            return Ok(());
        }
        Some(Command::Desk) => {
            let cfg = config::load();
            let group = args.group.clone().or(cfg.group.clone()).map(|g| link::sanitize_group(&g))
                .unwrap_or_else(|| "default".into());
            print_desk(&group);
            return Ok(());
        }
        Some(Command::Calibrate) => {
            let cfg = config::load();
            let group = args.group.clone().or(cfg.group.clone()).map(|g| link::sanitize_group(&g))
                .unwrap_or_else(|| "default".into());
            return termpaper::calibrate::run_standalone(&group);
        }
        Some(Command::Wall { action: WallCmd::Down }) => {
            let n = termpaper::launch::wall_down()?;
            println!("closed {n} wall terminal(s)");
            return Ok(());
        }
        Some(Command::Wall { action: WallCmd::Up { dry_run, font, monitor, extra } }) => {
            let Some(mons) = termpaper::hypr::monitors() else {
                eprintln!("termpaper: `wall up` needs Hyprland");
                std::process::exit(1);
            };
            let desk = termpaper::desk::Desk::from_hypr(&mons, &termpaper::desk::load_desk());
            let group = args.group.clone().map(|g| link::sanitize_group(&g)).unwrap_or_else(|| "wallpaper".into());
            let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "termpaper".into());
            let plans = termpaper::launch::plan_wall_up(&desk, &exe, font, None, &group, &monitor, &extra);
            let launched = termpaper::launch::run_wall_up(&plans, dry_run)?;
            for p in &plans {
                let state = if launched.contains(p) { if dry_run { "would start" } else { "started" } } else { "already running" };
                println!("{:<10} {:>5.2} pt  {state}", p.monitor, p.font_pt);
                if dry_run {
                    println!("  hyprctl dispatch '{}'", p.lua_dispatch());
                }
            }
            return Ok(());
        }
        None => {}
    }

    let cfg = config::load();
    let names = scene::all_names();
    // which terminal this is: 24-bit colour and a frame rate it can take
    let caps = termpaper::term_caps::detect();

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
    // grade, palette and effects; `--filter` replaces the effect stack
    let mut look = config::look_of(&cfg);
    if !args.filter.is_empty() {
        look.effects.stack = args.filter.clone();
    }
    let fps = args.fps.or(cfg.fps).unwrap_or(caps.default_fps).clamp(1, 240);
    let idle_fps = args.idle_fps.or(cfg.idle_fps).map(|f| f.clamp(1, 240));
    let speed = args.speed.or(cfg.speed).unwrap_or(1.0);
    let cycle = args.cycle.or(cfg.cycle).filter(|c| *c > 0.0);
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
        pad: args
            .pad
            .as_deref()
            .and_then(config::PadSpec::parse)
            .or_else(|| cfg.pad.clone())
            // layout px: Hyprland geometry is logical, and so are points
            .map(|p| p.to_px(1.0))
            .unwrap_or((0.0, 0.0)),
        placement: cfg.placement.unwrap_or_default(),
        hysteresis: cfg.hysteresis.unwrap_or(DEFAULT_HYSTERESIS),
        cycle_scope: cfg.cycle_scope.as_deref().map(CycleScope::parse).unwrap_or_default(),
        cfg,
        keymap,
        text_scale,
        theme,
        detail,
        pixels,
        look: termpaper::look::Baked::new(look),
        fps,
        idle_fps,
        speed,
        cycle,
        screensaver: args.screensaver,
        truecolor: caps.truecolor && !args.no_truecolor,
        default_fps: caps.default_fps,
        renderer,
        gpu_budget_ms: cfg_budget,
        shader_fps: cfg_shader_fps,
    };

    // 1 ms timed waits on Windows (frame pacing); a no-op elsewhere
    let _timer = termpaper::platform::TimerResolution::raise();
    let mut terminal = init_terminal()?;
    terminal.hide_cursor()?;
    // focus reporting lets unfocused instances skip the pacing spin (and
    // honor --idle-fps); terminals without support just never send events.
    // The mouse drives the menu: click, drag sliders, scroll.
    let _ = crossterm::execute!(terminal.backend_mut(), event::EnableFocusChange, event::EnableMouseCapture);
    let result = run(&mut terminal, &scene_name, settings);
    let _ = crossterm::execute!(terminal.backend_mut(), event::DisableFocusChange);
    restore_terminal();
    result
}

/// The terminal every frame is drawn through. Output is block-buffered: a
/// frame is hundreds of KB of SGR sequences, and the line-buffered `Stdout`
/// that `ratatui::init` uses would split it into hundreds of writes the
/// terminal can render half-way through (tearing).
type Term = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::BufWriter<std::io::Stdout>>>;

/// Frame output buffer. Big enough for a full braille frame of a large
/// terminal, so a frame normally leaves in one write.
const OUT_BUFFER: usize = 1 << 20;

/// `ratatui::init` with a buffered writer: raw mode, alternate screen, and a
/// panic hook that restores the terminal before the message prints.
fn init_terminal() -> std::io::Result<Term> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        hook(info);
    }));
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    let out = std::io::BufWriter::with_capacity(OUT_BUFFER, std::io::stdout());
    ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(out))
}

fn restore_terminal() {
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    );
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
    /// terminal padding in layout px (x, y)
    pad: (f32, f32),
    /// where the terminal puts its leftover strip
    placement: wall::Placement,
    /// Studio scenes' cell hysteresis threshold (levels; 0 = off)
    hysteresis: u8,
    cfg: config::Config,
    theme: Option<String>,
    text_scale: Option<u32>,
    keymap: config::KeyMap,
    detail: Detail,
    pixels: Pixels,
    /// grade, palette and effect stack, with its lookup table
    look: termpaper::look::Baked,
    fps: u32,
    /// fps cap applied while unfocused (None = no throttle)
    idle_fps: Option<u32>,
    speed: f32,
    cycle: Option<f64>,
    /// which scenes `cycle` rotates through
    cycle_scope: CycleScope,
    screensaver: bool,
    truecolor: bool,
    /// the terminal's frame-rate default (a config value equal to it is not
    /// written back)
    default_fps: u32,
    renderer: termpaper::engine::Renderer,
    /// GPU ms per frame a Studio scene may use (quality governor budget)
    gpu_budget_ms: f32,
    /// fps cap for Studio scenes: they animate on the shared 60 Hz tick
    shader_fps: u32,
}

/// The terminal's cell size in layout px from its reported pixel size
/// (TIOCGWINSZ), divided by the monitor scale when the compositor tells us.
fn measure_cell_px(geo: Option<&wall::GeoWatcher>) -> Option<(f32, f32)> {
    let ws = crossterm::terminal::window_size().ok()?;
    wall::measure_cell(
        ws.columns as usize,
        ws.rows as usize,
        ws.width,
        ws.height,
        geo.and_then(|w| w.latest_scale()),
        geo.and_then(|w| w.latest()),
    )
}

/// Cell hysteresis for Studio scenes unless the config says otherwise:
/// colour changes of up to this many levels are not re-sent.
const DEFAULT_HYSTERESIS: u8 = 3;

/// How long before the frame deadline to stop sleeping and busy-wait.
/// Sized from `examples/pace_bench.rs` — see the frame loop for the tradeoff.
const SPIN_TAIL: Duration = Duration::from_micros(100);

/// Frame period for a scene: Classic scenes snap to a divisor of their
/// 60 Hz tick; Studio scenes use continuous time, so any rate works (capped
/// by `shader_fps`). Either way the deadlines sit on the anchor's slot grid.
fn frame_period_ms(settings: &Settings, focused: bool, scene_name: &str) -> f64 {
    let mut fps = match settings.idle_fps {
        Some(idle) if !focused => settings.fps.min(idle),
        _ => settings.fps,
    };
    if scene::lookup(scene_name).is_some_and(|e| e.needs_gpu()) {
        fps = fps.min(settings.shader_fps);
    } else {
        fps = termpaper::sync::classic_fps(fps);
    }
    1000.0 / fps.max(1) as f64
}

/// The appearance settings peers mirror. Sim settings (speed, detail,
/// pixels, text scale, theme) travel in the anchor instead: any change to
/// them is picked up by `sync_sim` and published as a new anchor.
fn settings_msg(settings: &Settings, _opts: &SceneOptions, quick: &Option<String>) -> link::SettingsMsg {
    let look = settings.look.get();
    link::SettingsMsg {
        // the basics older binaries understand, mirrored from the look
        filters: look.effects.stack.clone(),
        fps: settings.fps,
        smooth: settings.smooth,
        dim: settings.dim,
        fade: settings.fade,
        clock: settings.clock,
        quick: quick.clone(),
        hue_shift: look.grade.hue,
        saturation: look.grade.saturation,
        contrast: look.grade.contrast,
        look: Some(look.clone()),
    }
}

/// Session-only apply of a peer's appearance settings.
fn apply_appearance(
    m: link::SettingsMsg,
    settings: &mut Settings,
    transition: &mut transition::Transition,
    quick_filter: &mut Option<String>,
) {
    let look = match m.look {
        Some(l) => l,
        // an older binary only speaks the basics: keep the rest of ours
        None => {
            let mut l = settings.look.get().clone();
            l.effects.stack = m.filters;
            l.grade.hue = m.hue_shift;
            l.grade.saturation = m.saturation;
            l.grade.contrast = m.contrast;
            l
        }
    };
    settings.look.set(look);
    settings.fps = m.fps.clamp(1, 240);
    settings.smooth = m.smooth;
    settings.dim = m.dim;
    settings.fade = m.fade;
    settings.clock = m.clock;
    transition.set_fade_secs(m.fade);
    *quick_filter = m.quick;
}

/// Grace before a published switch takes effect, on top of the fade: time
/// for every peer's per-frame poll to see the anchor before its fade-out
/// has to start, so all panes fade and swap together.
const SWITCH_MARGIN_MS: u64 = 120;

/// What the pane simulates now, and what a scheduled switch will swap in.
struct SimState {
    /// the anchor on screen: the group's, or a local one (unlinked/--cycle)
    cur: link::Anchor,
    /// `cur` is the group's anchor
    synced: bool,
    /// anchor to swap to at its `t0_ms`, and whether it is the group's
    next: Option<(link::Anchor, bool)>,
}

impl SimState {
    /// The anchor the pane is running or about to run: what incoming
    /// anchors and local edits are compared against.
    fn target(&self) -> &link::Anchor {
        self.next.as_ref().map(|n| &n.0).unwrap_or(&self.cur)
    }

    fn target_synced(&self) -> bool {
        self.next.as_ref().map(|n| n.1).unwrap_or(self.synced)
    }

    /// Replace the timing (t0/pause) of the target anchor, keeping its
    /// simulation: a retime never rebuilds.
    fn retime(&mut self, a: &link::Anchor, transition: &mut transition::Transition, names: &[&str]) {
        match &mut self.next {
            Some((n, _)) => {
                n.t0_ms = a.t0_ms;
                n.paused_at_ms = a.paused_at_ms;
                n.stamp = a.stamp;
                if let Some(i) = names.iter().position(|s| *s == n.scene) {
                    transition.schedule(i, n.t0_ms);
                }
            }
            None => {
                self.cur.t0_ms = a.t0_ms;
                self.cur.paused_at_ms = a.paused_at_ms;
                self.cur.stamp = a.stamp;
            }
        }
    }
}

/// An anchor for `scene` carrying the pane's current sim settings.
fn anchor_from(settings: &Settings, opts: &SceneOptions, scene: &str, seed: u64, t0_ms: u64) -> link::Anchor {
    link::Anchor {
        stamp: link::Stamp::default(),
        scene: scene.to_string(),
        theme: opts.theme.clone(),
        seed,
        t0_ms,
        paused_at_ms: None,
        speed: settings.speed,
        detail: opts.detail.name().to_string(),
        pixels: settings.pixels.name().to_string(),
        text_scale: opts.text_scale,
        proto: link::PROTO,
    }
}

/// Mirror an anchor's sim settings into the local settings (session only),
/// so the pane's desired state matches what it runs.
fn adopt_settings(a: &link::Anchor, settings: &mut Settings, opts: &mut SceneOptions) {
    opts.theme = a.theme.clone();
    if let Some(d) = Detail::parse(&a.detail) {
        opts.detail = d;
    }
    opts.text_scale = a.text_scale;
    settings.text_scale = a.text_scale;
    settings.speed = a.speed;
    if let Some(p) = Pixels::parse(&a.pixels) {
        settings.pixels = p;
        opts.pixels = p;
    }
}

/// Start a switch to `a`: it takes effect after the fade (plus a grace
/// period when linked, so peers fade out with us), published to the group
/// when `publish` and there is one.
fn begin_switch(
    st: &mut SimState,
    guard: &mut Option<link::Guard>,
    transition: &mut transition::Transition,
    names: &[&str],
    a: link::Anchor,
    publish: bool,
) {
    begin_switch_paused(st, guard, transition, names, a, publish, false);
}

/// `begin_switch`, optionally keeping a paused anchor paused (frozen on the
/// new start): a re-anchor must not unpause a wall the user paused.
fn begin_switch_paused(
    st: &mut SimState,
    guard: &mut Option<link::Guard>,
    transition: &mut transition::Transition,
    names: &[&str],
    mut a: link::Anchor,
    publish: bool,
    keep_pause: bool,
) {
    let linked = publish && guard.is_some();
    a.t0_ms = link::epoch_now_ms() + transition.fade_ms() + if linked { SWITCH_MARGIN_MS } else { 0 };
    a.paused_at_ms = (keep_pause && a.paused_at_ms.is_some()).then_some(a.t0_ms);
    if let (true, Some(g)) = (linked, guard.as_mut()) {
        g.publish_anchor(&mut a);
    }
    if let Some(i) = names.iter().position(|n| *n == a.scene) {
        transition.schedule(i, a.t0_ms);
        st.next = Some((a, linked));
    }
}

/// Apply an anchor another instance published: a retime moves the clock,
/// anything else schedules a switch at the anchor's t0 (immediately, when
/// that already passed).
fn receive_anchor(
    st: &mut SimState,
    a: link::Anchor,
    transition: &mut transition::Transition,
    names: &[&str],
    settings: &mut Settings,
    opts: &mut SceneOptions,
) {
    if !names.contains(&a.scene.as_str()) {
        return; // a scene this binary lacks: keep what we have
    }
    match link::classify(st.target(), &a) {
        link::AnchorChange::Same => {}
        link::AnchorChange::Retime => st.retime(&a, transition, names),
        link::AnchorChange::Switch => {
            adopt_settings(&a, settings, opts);
            if let Some(i) = names.iter().position(|n| *n == a.scene) {
                transition.schedule(i, a.t0_ms);
            }
            st.next = Some((a, true));
            return;
        }
    }
    // a retime of the group's anchor makes a locally cycling pane rejoin
    match &mut st.next {
        Some((_, s)) => *s = true,
        None => st.synced = true,
    }
}

/// Local edits to sim settings (menu, keys, reset) show up as a mismatch
/// between the settings and the target anchor: publish that as a switch.
fn sync_sim(
    st: &mut SimState,
    guard: &mut Option<link::Guard>,
    transition: &mut transition::Transition,
    names: &[&str],
    settings: &Settings,
    opts: &SceneOptions,
) {
    let t = st.target();
    let desired = anchor_from(settings, opts, &t.scene, t.seed, t.t0_ms);
    if !desired.same_sim(t) {
        begin_switch(st, guard, transition, names, desired, true);
    }
}

/// A user-initiated switch to scene `i` with its remembered theme,
/// published to the group.
fn switch_scene(
    i: usize,
    st: &mut SimState,
    guard: &mut Option<link::Guard>,
    transition: &mut transition::Transition,
    names: &[&str],
    settings: &Settings,
    opts: &mut SceneOptions,
) {
    opts.theme = settings.cfg.themes.get(names[i]).cloned().or_else(|| settings.theme.clone());
    let a = anchor_from(settings, opts, names[i], rand::rng().random(), 0);
    begin_switch(st, guard, transition, names, a, true);
}

/// The lowest live pid running the group's anchor leads: it answers
/// re-anchor requests and retimes the group after a suspend.
fn is_leader(peers: &[link::InstanceInfo], synced: bool) -> bool {
    let me = std::process::id();
    let lowest = peers
        .iter()
        .filter(|i| i.synced)
        .map(|i| i.pid)
        .chain(synced.then_some(me))
        .min();
    lowest == Some(me)
}

/// Join the (new) group: adopt its anchor, or publish ours when it has none.
fn join_group(
    st: &mut SimState,
    guard: &mut Option<link::Guard>,
    transition: &mut transition::Transition,
    names: &[&str],
    settings: &mut Settings,
    opts: &mut SceneOptions,
) {
    let Some(g) = guard.as_mut() else {
        st.synced = false;
        return;
    };
    match g.latest_anchor(st.target()) {
        Some(a) => receive_anchor(st, a, transition, names, settings, opts),
        None => {
            if st.next.is_none() {
                g.publish_anchor(&mut st.cur);
                st.synced = true;
            } else if let Some((n, s)) = &mut st.next {
                g.publish_anchor(n);
                *s = true;
            }
        }
    }
}

/// Realtime jumps beyond this relative to the monotonic clock are a system
/// suspend: the anchor is retimed so the scene resumes where it stopped.
const SUSPEND_JUMP_MS: i64 = 5_000;

/// A rebuilt simulation estimated to need longer than this to replay up to
/// the anchor's clock asks for a fresh anchor instead.
const REANCHOR_ETA_MS: u32 = 3_000;

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
        look: settings.look.get(),
        text_scale: settings.text_scale,
        fps: settings.fps,
        default_fps: settings.default_fps,
        speed: settings.speed,
        smooth: settings.smooth,
        dim: settings.dim,
        fade: settings.fade,
        clock: settings.clock,
        cycle: settings.cycle,
        cycle_scope: settings.cycle_scope,
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

/// How many changes `u` can step back through.
const UNDO_DEPTH: usize = 64;

/// Repeats of one kind of change closer together than this undo as one.
const UNDO_BURST: Duration = Duration::from_millis(900);

/// The settings a menu change can touch, for undo.
struct Snapshot {
    look: termpaper::look::Look,
    dim: f32,
    smooth: f32,
    fade: f32,
    fps: u32,
    speed: f32,
    pixels: Pixels,
    detail: Detail,
    theme: Option<String>,
    text_scale: Option<u32>,
    clock: bool,
    cycle: Option<f64>,
    cycle_scope: CycleScope,
    renderer: termpaper::engine::Renderer,
}

impl Snapshot {
    fn take(s: &Settings, opts: &SceneOptions) -> Self {
        Snapshot {
            look: s.look.get().clone(),
            dim: s.dim,
            smooth: s.smooth,
            fade: s.fade,
            fps: s.fps,
            speed: s.speed,
            pixels: s.pixels,
            detail: opts.detail,
            theme: opts.theme.clone(),
            text_scale: s.text_scale,
            clock: s.clock,
            cycle: s.cycle,
            cycle_scope: s.cycle_scope,
            renderer: s.renderer,
        }
    }

    /// Put everything back. Sim settings (variant, detail, pixels, speed,
    /// text size) then reach the group through `sync_sim` like any edit.
    fn restore(self, s: &mut Settings, opts: &mut SceneOptions, transition: &mut transition::Transition) {
        s.look.set(self.look);
        s.dim = self.dim;
        s.smooth = self.smooth;
        s.fade = self.fade;
        transition.set_fade_secs(self.fade);
        s.fps = self.fps;
        s.speed = self.speed;
        s.pixels = self.pixels;
        opts.detail = self.detail;
        opts.theme = self.theme;
        s.text_scale = self.text_scale;
        opts.text_scale = self.text_scale;
        s.clock = self.clock;
        s.cycle = self.cycle;
        s.cycle_scope = self.cycle_scope;
        s.renderer = self.renderer;
    }
}

/// What a browser preview replaced: restored when the preview is dropped,
/// forgotten when it is kept.
struct PreviewOrigin {
    idx: usize,
    theme: Option<String>,
    /// the anchor the original scene was running on, and whether it was
    /// the group's
    anchor: link::Anchor,
    synced: bool,
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
        look: settings.look.get().clone(),
        renderer: settings.renderer,
        link_enabled: settings.link_enabled,
        link_group: settings.link_group.clone(),
        wall_enabled: settings.wall_enabled,
        truecolor: settings.truecolor,
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
    pad: (f32, f32),
) {
    *guard = None;
    if enabled {
        *guard = link::Guard::new(scene, group).map(|mut g| {
            // placement and cell size follow with the next wall refresh
            g.set_layout(pad, wall::Placement::default(), None);
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
    settings.look.set(termpaper::look::Look::default());
    settings.text_scale = None;
    settings.fps = settings.default_fps;
    settings.speed = config::DEFAULT_SPEED;
    settings.smooth = config::DEFAULT_SMOOTH;
    settings.dim = config::DEFAULT_DIM;
    settings.fade = config::DEFAULT_FADE;
    settings.clock = config::DEFAULT_CLOCK;
    settings.cycle = None;
    settings.cycle_scope = CycleScope::All;
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
        settings.pad,
    );
}

fn run(
    terminal: &mut Term,
    start_scene: &str,
    mut settings: Settings,
) -> std::io::Result<()> {
    let names = scene::all_names();
    // the terminal's true cell size (layout px), when it reports one
    let mut cell_px: Option<(f32, f32)> = None;
    let mut cell_grid = (0usize, 0usize);
    let mut guard = if settings.link_enabled {
        link::Guard::new(start_scene, &settings.link_group).map(|mut g| {
            g.set_layout(settings.pad, settings.placement, cell_px);
            g
        })
    } else {
        None
    };
    // legacy control.json: only older binaries still speak through it
    let mut control_stamp = link::Stamp::default();
    let mut menu = Menu::new();
    // menu effects queue here and apply at the top of the next frame
    let mut pending_fx: Vec<Effect> = Vec::new();
    // set while the browser previews a scene on this pane only
    let mut preview: Option<PreviewOrigin> = None;
    let mut save = config::SaveTimer::default();
    // settings as they were before each change the menu made (`u` undoes)
    let mut undo: Vec<Snapshot> = Vec::new();
    let mut undo_last: Option<(std::mem::Discriminant<Effect>, Instant)> = None;
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
    // terminals that report focus keep this current; ones that don't never
    // send the events, so it stays true and nothing changes for them
    let mut focused = true;
    let mut quick_filter: Option<String> = None;
    let mut idx = names
        .iter()
        .position(|n| *n == start_scene)
        .unwrap_or(0);
    // The simulation anchor: a local one from the launch settings, replaced
    // by the group's when there is one (Guard::new reaped dead sessions, so
    // anchor.json belongs to a living group). Otherwise ours becomes it.
    let mut st = SimState {
        cur: anchor_from(&settings, &opts, names[idx], rand::rng().random(), link::epoch_now_ms()),
        synced: false,
        next: None,
    };
    if let Some(g) = &mut guard {
        match g.latest_anchor(&st.cur) {
            Some(a) if names.contains(&a.scene.as_str()) => {
                adopt_settings(&a, &mut settings, &mut opts);
                idx = names.iter().position(|n| *n == a.scene).unwrap_or(idx);
                st.cur = a;
                st.synced = true;
            }
            // the group runs a scene this build lacks: show ours locally and
            // leave the group's anchor alone (as `receive_anchor` does)
            Some(_) => {}
            None => {
                g.publish_anchor(&mut st.cur);
                st.synced = true;
            }
        }
        g.set_scene(names[idx]);
    }
    let mut worker = termpaper::engine::Worker::new(settings.renderer);
    let mut rendered: Option<termpaper::engine::Frame> = None;
    // backend status while no frame is coming (a Studio shader compiling)
    let mut worker_status: Option<String> = None;
    // the shared epoch clock and the anchor's slot grid frames are paced on;
    // also detects a suspend (realtime jumping ahead of the monotonic clock)
    let mut clock = termpaper::sync::FrameClock::new();
    // how many slots ahead of display each frame is requested
    let mut lead = termpaper::sync::Lead::new();
    // origin of the slot grid the lead's outstanding requests refer to
    let mut grid_origin = st.cur.t0_ms;
    // the anchor this pane asked the leader to replace (catch-up too long)
    let mut reanchor_wanted: Option<link::Stamp> = None;
    // bumped whenever the wall layout moves this pane's window
    let mut view_rev = 0u64;
    let mut wall_layout: Option<wall::WallLayout> = None;
    let mut wall_refresh = Instant::now() - Duration::from_secs(10);
    // hyprctl inside the frame loop stalls the frame it lands on
    let hypr_ok = termpaper::hypr::present() && termpaper::hypr::monitors().is_some();
    let mut geo_watcher = if hypr_ok && ((settings.link_enabled && settings.wall_enabled) || settings.wall_spec.is_some()) {
        Some(wall::GeoWatcher::spawn())
    } else {
        None
    };
    // The physical wall plan (Hyprland): one leader computes it for the
    // whole group from the desk model and every pane adopts it — portrait
    // and landscape monitors line up in millimetres. The cell-count layout
    // stays as the fallback everywhere else.
    let mut planner: Option<termpaper::wallplan::Planner> = None;
    let mut plan_watcher: Option<termpaper::wallplan::PlanWatcher> = None;
    let mut calib_watcher: Option<termpaper::calibrate::Watcher> = None;
    let mut wall_plan: Option<termpaper::wallplan::WallPlan> = None;
    let mut plan_group = String::new();
    // resampling buffer for Classic panes whose cells differ from the canvas
    let mut classic_scratch = termpaper::canvas::Canvas::new(1, 1);
    // monitors for the calibration pattern, refreshed with the wall
    let mut calib_monitors: Vec<termpaper::hypr::HyprMonitor> = Vec::new();
    // this pane drives a calibration opened from the menu
    let mut calib_ctl: Option<termpaper::calibrate::Controller> = None;

    // one registry snapshot shared by the leader duties, wall layout, and menu.
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
    // cached "HH:MM" for the clock overlay (refreshed at most every 10s)
    let mut clock_text = String::new();
    let mut clock_stamp = Instant::now() - Duration::from_secs(60);

    loop {
        let now = Instant::now();

        // System suspend: the realtime clock ran on while the monotonic one
        // stood still. Move t0 forward by the gap so the scene resumes where
        // it stopped instead of replaying the whole sleep. The leader
        // publishes the retime; the others apply the same shift locally
        // right away and then adopt the leader's (a retime: no rebuild).
        {
            let jump = clock.resync(now, termpaper::sync::epoch_now_ms_f64()) as i64;
            // (a clock stepped backwards is the mirror case: without the
            // shift a Classic scene would hold still until it caught up)
            if jump.abs() > SUSPEND_JUMP_MS && !st.target().paused() {
                let mut a = st.target().clone();
                a.t0_ms = a.t0_ms.saturating_add_signed(jump);
                if let (true, Some(g)) = (st.target_synced() && is_leader(&peers.list, true), guard.as_mut()) {
                    g.publish_anchor(&mut a);
                }
                st.retime(&a, &mut transition, &names);
            }
        }

        // menu: fire the preview timer, then apply what the menu asked for.
        // Sim settings (theme, detail, pixels, speed, text size) only need
        // setting: `sync_sim` publishes the difference as a synced switch.
        if menu.open {
            pending_fx.extend(menu.tick(now));
        }
        let mut effects: VecDeque<Effect> = std::mem::take(&mut pending_fx).into();
        while let Some(fx) = effects.pop_front() {
            // one undo step per change, or per burst of the same change (a
            // held arrow key sweeping a slider undoes in one go)
            if fx.undoable() {
                let kind = std::mem::discriminant(&fx);
                let fresh = undo_last.is_none_or(|(k, at)| k != kind || now.saturating_duration_since(at) > UNDO_BURST);
                if fresh {
                    undo.push(Snapshot::take(&settings, &opts));
                    if undo.len() > UNDO_DEPTH {
                        undo.remove(0);
                    }
                }
                undo_last = Some((kind, now));
            }
            match fx {
                Effect::SwitchScene(name) => {
                    let Some(i) = names.iter().position(|n| *n == name) else {
                        continue;
                    };
                    menu::browser::push_recent(&mut settings.cfg.recents, name);
                    save.mark(now);
                    let kept = preview.take().is_some() && st.next.is_none() && st.cur.scene == name;
                    if kept {
                        // the preview becomes the real scene as it is: its
                        // local anchor turns into the group's, so nothing
                        // restarts here or on the peers
                        if let Some(g) = &mut guard {
                            g.publish_anchor(&mut st.cur);
                            st.synced = true;
                        }
                    } else {
                        switch_scene(i, &mut st, &mut guard, &mut transition, &names, &settings, &mut opts);
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
                        anchor: st.target().clone(),
                        synced: st.target_synced(),
                        showing: idx,
                    });
                    p.showing = i;
                    // local only: nothing published; the previewed scene
                    // shows with its remembered theme
                    let mut o = opts.clone();
                    o.theme = settings.cfg.themes.get(names[i]).cloned().or_else(|| settings.theme.clone());
                    let a = anchor_from(&settings, &o, names[i], rand::rng().random(), 0);
                    opts.theme = o.theme;
                    begin_switch(&mut st, &mut guard, &mut transition, &names, a, false);
                    last_switch = now;
                }
                Effect::EndPreview => {
                    if let Some(p) = preview.take() {
                        // restore only if the preview is still what runs —
                        // a peer's switch meanwhile wins
                        if st.target().scene == names[p.showing] && !st.target_synced() {
                            if p.synced && guard.is_some() {
                                // back to whatever the group runs now
                                join_group(&mut st, &mut guard, &mut transition, &names, &mut settings, &mut opts);
                            } else {
                                opts.theme = p.anchor.theme.clone();
                                begin_switch(&mut st, &mut guard, &mut transition, &names, p.anchor, false);
                            }
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
                            p.anchor.theme = Some(theme.clone());
                        }
                    }
                    if scene == st.target().scene {
                        if preview.is_some() {
                            // a previewed scene re-themes locally
                            let mut a = st.target().clone();
                            a.theme = Some(theme);
                            opts.theme = a.theme.clone();
                            begin_switch(&mut st, &mut guard, &mut transition, &names, a, false);
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
                    save.mark(now);
                }
                Effect::SetDetail(d) => {
                    opts.detail = d;
                    save.mark(now);
                }
                Effect::SetTheme(t) => {
                    opts.theme = t;
                    save.mark(now);
                }
                Effect::SetTextScale(ts) => {
                    settings.text_scale = ts;
                    opts.text_scale = ts;
                    save.mark(now);
                }
                Effect::SetSpeed(sp) => {
                    settings.speed = sp;
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
                    reset_link_guard(&mut guard, on, &settings.link_group, names[idx], settings.pad);
                    join_group(&mut st, &mut guard, &mut transition, &names, &mut settings, &mut opts);
                    if !on && settings.wall_spec.is_none() {
                        // the wall refresh stops with linking: drop the crop now
                        wall_layout = None;
                    }
                    save.mark(now);
                }
                Effect::SetLinkGroup(g) => {
                    settings.link_group = g;
                    reset_link_guard(&mut guard, settings.link_enabled, &settings.link_group, names[idx], settings.pad);
                    join_group(&mut st, &mut guard, &mut transition, &names, &mut settings, &mut opts);
                    save.mark(now);
                }
                Effect::SetWall(on) => {
                    settings.wall_enabled = on;
                    if on && hypr_ok && geo_watcher.is_none() {
                        geo_watcher = Some(wall::GeoWatcher::spawn());
                    }
                    if !on && settings.wall_spec.is_none() {
                        wall_layout = None;
                    }
                    // re-lay the wall (and republish geometry) next frame
                    wall_refresh = Instant::now() - Duration::from_secs(10);
                    save.mark(now);
                }
                Effect::SetLook(l) => {
                    settings.look.set(l);
                    if let Some(g) = &mut guard {
                        g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                    }
                    save.mark(now);
                }
                Effect::OpenColorGrade | Effect::OpenPalette => {
                    color_open = true;
                    color_param = color_wheel::Param::Hue;
                }
                Effect::Undo => {
                    match undo.pop() {
                        Some(snap) => {
                            let renderer_before = settings.renderer;
                            snap.restore(&mut settings, &mut opts, &mut transition);
                            if settings.renderer != renderer_before {
                                worker = termpaper::engine::Worker::new(settings.renderer);
                                rendered = None;
                                worker_status = None;
                            }
                            if let Some(g) = &mut guard {
                                g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                            }
                            save.mark(now);
                            menu.flash("Undone");
                        }
                        None => menu.flash("Nothing to undo"),
                    }
                    undo_last = None;
                }
                // placeholder until the alignment tool lands
                Effect::OpenCalibration => {
                    // check first: starting a controller publishes the
                    // pattern to the whole group
                    if !settings.link_enabled {
                        menu.flash("Align monitors: turn Link on first");
                    } else {
                        let ctl = link::group_dir(&settings.link_group).and_then(|d| {
                            let _ = std::fs::create_dir_all(&d);
                            termpaper::calibrate::Controller::start(d)
                        });
                        match ctl {
                            Some(c) => {
                                calib_ctl = Some(c);
                                menu.close();
                            }
                            None => menu.flash("Align monitors needs Hyprland"),
                        }
                    }
                }
            }
        }
        // config writes coalesce while the menu or wheel is up; otherwise
        // (including right after either closes) they go out immediately
        let write = if menu.open || color_open { save.due(now) } else { save.take() };
        if write {
            let (scene_name, theme) = remembered_scene(&preview, &names, idx, &opts);
            persist(&mut settings, scene_name, theme, opts.detail);
        }

        // scene cycling (local: rotations don't propagate)
        if let Some(secs) = settings.cycle {
            if preview.is_none() && now.duration_since(last_switch).as_secs_f64() >= secs {
                let i = menu::browser::cycle_next(&names, idx, settings.cycle_scope, &settings.cfg.favorites);
                let mut o = opts.clone();
                o.theme = settings.cfg.themes.get(names[i]).cloned().or_else(|| settings.theme.clone());
                let a = anchor_from(&settings, &o, names[i], rand::rng().random(), 0);
                opts.theme = o.theme;
                begin_switch(&mut st, &mut guard, &mut transition, &names, a, false);
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

        // instance linking: a stat per channel per frame; a file is only
        // read and parsed when a publish replaced it
        if let Some(g) = &mut guard {
            g.heartbeat();
            if let Some(a) = g.poll_anchor() {
                receive_anchor(&mut st, a, &mut transition, &names, &mut settings, &mut opts);
                last_switch = Instant::now();
            }
            if let Some(m) = g.poll_settings() {
                apply_appearance(m, &mut settings, &mut transition, &mut quick_filter);
            }
            // an older binary in the group speaks through control.json
            if let Some(ctrl) = g.poll_control(control_stamp) {
                control_stamp = link::Stamp {
                    epoch: ctrl.epoch,
                    seq: ctrl.seq,
                    from_pid: ctrl.from_pid,
                };
                match ctrl.kind {
                    link::ControlKind::Scene => {
                        let a = link::Anchor::from_legacy(&ctrl, st.target());
                        receive_anchor(&mut st, a, &mut transition, &names, &mut settings, &mut opts);
                        last_switch = Instant::now();
                    }
                    link::ControlKind::Settings => {
                        if let Some(m) = ctrl.settings {
                            apply_appearance(m, &mut settings, &mut transition, &mut quick_filter);
                        }
                    }
                }
            }
        }
        // local edits to sim settings become a new anchor
        // a preview is local: its settings must not go out as a switch
        if preview.is_none() {
            sync_sim(&mut st, &mut guard, &mut transition, &names, &settings, &opts);
        }

        // Frame slots: this frame is presented at `slot` of the anchor's
        // grid (t0 + n·period, the same on every pane) and requests the
        // frame for `slot + lead`, so what the terminal shows at a slot was
        // rendered for that slot.
        let mut period = frame_period_ms(&settings, focused, &st.cur.scene);
        let mut slot = clock.slot_at(now, st.cur.t0_ms, period);
        let mut target = slot + lead.lead() as i64;

        // transition: fade out → swap → fade in, on the shared epoch clock,
        // evaluated for the moment the requested frame will be on screen
        let shown_at = termpaper::sync::FrameClock::slot_epoch(st.cur.t0_ms, period, target) as u64;
        let (fade, swap) = transition.tick_at(shown_at);
        if swap.is_some() {
            if let Some((a, synced)) = st.next.take() {
                st.cur = a;
                st.synced = synced;
            }
            idx = names.iter().position(|n| *n == st.cur.scene).unwrap_or(idx);
            reanchor_wanted = None;
            if let Some(g) = &mut guard {
                g.set_scene(names[idx]);
                g.set_synced(st.synced);
                g.set_reanchor(None);
            }
            last_switch = now;
            period = frame_period_ms(&settings, focused, &st.cur.scene);
            slot = clock.slot_at(now, st.cur.t0_ms, period);
            target = slot + lead.lead() as i64;
        }
        if st.cur.t0_ms != grid_origin {
            // new grid: outstanding requests no longer map onto it
            grid_origin = st.cur.t0_ms;
            lead.reset();
        }

        // the terminal's true cell size, re-measured whenever the grid
        // changes (and with every wall refresh: the monitor scale may have)
        let term = terminal.size()?;
        if (term.width as usize, term.height as usize) != cell_grid {
            cell_grid = (term.width as usize, term.height as usize);
            cell_px = measure_cell_px(geo_watcher.as_ref());
            wall_refresh = Instant::now() - Duration::from_secs(10);
        }

        // video wall: refresh layout every 2s from registry geometry
        if (settings.link_enabled || settings.wall_spec.is_some())
            && wall_refresh.elapsed() > Duration::from_secs(2)
        {
            wall_refresh = Instant::now();
            let (cols, rows) = cell_grid;
            cell_px = measure_cell_px(geo_watcher.as_ref());
            // a --no-wall pane publishes no geometry, so peers never fold it
            // into their wall either
            let geo = geo_watcher
                .as_ref()
                .filter(|_| settings.wall_enabled)
                .and_then(|w| w.latest());
            if let Some(g) = &mut guard {
                g.set_geometry(cols, rows, geo.map(|g| (g.x, g.y, g.w, g.h)));
                g.set_layout(settings.pad, settings.placement, cell_px);
            }
            let old_layout = wall_layout;
            wall_layout = if let Some(spec) = &settings.wall_spec {
                wall::manual_layout(spec, cols, rows)
            } else if settings.link_enabled && settings.wall_enabled {
                let me = std::process::id();
                let mut parts: Vec<wall::Participant> = peers
                    .list
                    .iter()
                    .filter(|i| i.pid != me)
                    .filter_map(|i| {
                        i.geo.map(|(x, y, w, h)| wall::Participant {
                            pid: i.pid,
                            geo: wall::Geo { x, y, w, h },
                            cols: i.cols.max(1),
                            rows: i.rows.max(1),
                            pad: i.pad,
                            cell: i.cell,
                            placement: i.placement,
                        })
                    })
                    .collect();
                // ourselves from fresh local facts (our registry entry can be
                // two seconds stale), even if hyprctl is unavailable to others
                if let Some(g) = geo {
                    parts.push(wall::Participant {
                        pid: me,
                        geo: g,
                        cols,
                        rows,
                        pad: settings.pad,
                        cell: cell_px,
                        placement: settings.placement,
                    });
                }
                wall::compute_layout(parts, me)
            } else {
                None
            };
            if let Some(l) = wall_layout {
                if l.too_big {
                    wall_layout = None; // guard: virtual area too big, stay local
                }
            }
            if wall_layout != old_layout {
                view_rev += 1;
            }
            // physical plan: attach to the group's plan and calibration files
            let physical = hypr_ok && settings.link_enabled && settings.wall_enabled && settings.wall_spec.is_none();
            if physical {
                if plan_watcher.is_none() || plan_group != settings.link_group {
                    plan_group = settings.link_group.clone();
                    wall_plan = None;
                    planner = None;
                    if let Some(dir) = link::group_dir(&plan_group) {
                        let group = plan_group.clone();
                        planner = Some(termpaper::wallplan::Planner::spawn(dir.clone(), std::process::id(), move || panes_of(&group)));
                        plan_watcher = Some(termpaper::wallplan::PlanWatcher::new(dir.clone()));
                        calib_watcher = Some(termpaper::calibrate::Watcher::new(dir));
                    }
                    view_rev += 1;
                }
                if let Some(p) = &planner {
                    p.set_active(is_leader(&peers.list, st.synced));
                }
                calib_monitors = termpaper::hypr::monitors().unwrap_or_default();
            } else if plan_watcher.is_some() {
                planner = None;
                plan_watcher = None;
                calib_watcher = None;
                wall_plan = None;
                view_rev += 1;
            }
        }

        // clock overlay: local time (TZ and DST from the OS), refreshed at
        // most once a second
        if settings.clock && clock_stamp.elapsed() > Duration::from_secs(1) {
            clock_stamp = Instant::now();
            let t = termpaper::platform::local_time();
            clock_text = format!("{:02}:{:02}", t.hour, t.minute);
        }

        // a newly published plan, and any calibration in progress
        if let Some(w) = &mut plan_watcher {
            if let Some(p) = w.poll() {
                wall_plan = Some(p);
                view_rev += 1;
            }
        }
        let calib_state = calib_watcher.as_mut().and_then(|w| w.poll().cloned());

        // DEC 2026 synchronized output: the terminal holds the old frame
        // until the end marker, so a frame never shows half-written. The
        // begin marker rides in the same buffered write as the frame (draw
        // flushes); terminals without support ignore both.
        crossterm::queue!(terminal.backend_mut(), crossterm::terminal::BeginSynchronizedUpdate)?;
        // what is on screen comes from the running anchor; `settings` may
        // already hold the next one's values while a switch is pending
        let shown_pixels = Pixels::parse(&st.cur.pixels).unwrap_or(settings.pixels);
        let shown_opts = SceneOptions {
            theme: st.cur.theme.clone(),
            detail: Detail::parse(&st.cur.detail).unwrap_or(opts.detail),
            text_scale: st.cur.text_scale,
            pixels: shown_pixels,
        };
        // the scene clock of the target slot (frozen while the wall is paused)
        let elapsed_ms = match st.cur.paused_at_ms {
            Some(at) => at.saturating_sub(st.cur.t0_ms),
            None => termpaper::sync::slot_elapsed_ms(target, period),
        };
        terminal.draw(|f| {
            let area = f.area();
            let (pw, ph) = shown_pixels.cell_size();
            let me = std::process::id();
            let my_plan = wall_plan.as_ref().and_then(|p| p.pane(me).map(|pp| (p.classic_cells, pp.clone())));
            let local = (area.width as usize * pw, area.height as usize * ph);
            // Classic canvas and this pane's window onto it: the physical
            // plan's shared canvas, else the cell-count wall, else local
            let (size, crop, classic_map) = if let Some((cells, pp)) = &my_plan {
                match pp.classic {
                    Some((o, stp)) => {
                        let crop = render::view_is_crop(o, stp, shown_pixels)
                            .map(|(x, y)| (x.max(0) as usize, y.max(0) as usize))
                            .unwrap_or((0, 0));
                        ((cells.0 * pw, cells.1 * ph), crop, Some((o, stp)))
                    }
                    None => (local, (0, 0), None),
                }
            } else if let Some(l) = wall_layout {
                ((l.virtual_w * pw, l.virtual_h * ph), (l.crop_x * pw, l.crop_y * ph), None)
            } else {
                (local, (0, 0), None)
            };
            let request = termpaper::engine::Request {
                generation: 0,
                // what is simulated: a change rebuilds (and replays out of sight)
                sim: termpaper::engine::SimKey::new(&st.cur.scene, st.cur.seed, shown_opts.clone(), size, st.cur.speed),
                // how it is shown: a change keeps the simulation running
                view: termpaper::engine::ViewKey {
                    canvas: size,
                    grid: (area.width as usize, area.height as usize),
                    crop,
                    pixels: shown_pixels,
                    // the real cell shape: this pane's own when local, one
                    // shared by every pane in a wall (else the 1:2 default)
                    cell_aspect: match wall_layout {
                        Some(l) => l.cell_aspect.unwrap_or(termpaper::engine::DEFAULT_CELL_ASPECT),
                        None => cell_px.map(|(w, h)| h / w).unwrap_or(termpaper::engine::DEFAULT_CELL_ASPECT),
                    },
                    rev: view_rev,
                    comp: my_plan.as_ref().map(|(_, pp)| pp.comp),
                    classic_map,
                },
                // a paused anchor freezes elapsed itself: the worker keeps
                // replaying up to it (a pane joining a paused wall catches up)
                elapsed_ms, paused: false, look: settings.look.clone(),
                quick: quick_filter.clone(), dim: fade * settings.dim, smooth: settings.smooth,
                budget_ms: settings.gpu_budget_ms,
                prefetch: transition.pending().map(|i| names[i % names.len()].to_string()),
                // Studio scenes only: Classic GPU output stays identical to
                // the CPU path, which has no hysteresis
                hysteresis: if scene::lookup(&st.cur.scene).is_some_and(|e| e.needs_gpu()) {
                    settings.hysteresis
                } else {
                    0
                },
            };
            if let Some(frame) = worker.submit(request) {
                lead.on_frame(frame.elapsed_ms, slot);
                rendered = Some(frame);
                worker_status = None;
            }
            lead.on_submit(elapsed_ms, slot);
            if let Some(status) = worker.take_status() { worker_status = Some(status); }
            if rendered.as_ref().is_some_and(|frame| frame.generation != worker.generation()) {
                rendered = None;
            }
            // calibration: every pane shows the millimetre pattern instead
            let geo_now = geo_watcher.as_ref().and_then(|w| w.latest());
            let calib_drawn = match (&calib_state, geo_now) {
                (Some(cs), Some(g)) => {
                    let geom = termpaper::wallplan::PaneGeom {
                        pid: me,
                        ancestors: Vec::new(),
                        cols: area.width,
                        rows: area.height,
                        cell: cell_px.map(|(w, h)| (w as f64, h as f64)),
                        pad: (settings.pad.0 as f64, settings.pad.1 as f64),
                        centered: settings.placement == wall::Placement::Center,
                    };
                    let win = [g.x as f64, g.y as f64, g.w as f64, g.h as f64];
                    match termpaper::calibrate::pane_pattern(cs, &calib_monitors, win, &geom, Pixels::Half) {
                        Some((canvas, label)) => {
                            render::draw_crop(&canvas, 0, 0, area, f.buffer_mut(), settings.truecolor, Pixels::Half);
                            let w = (label.chars().count() as u16).min(area.width.saturating_sub(2));
                            f.render_widget(
                                Paragraph::new(Text::raw(label)).style(
                                    Style::new()
                                        .fg(ratatui::style::Color::Rgb(245, 245, 245))
                                        .bg(ratatui::style::Color::Rgb(24, 26, 34)),
                                ),
                                Rect { x: area.x + 1, y: area.y + 2, width: w, height: 1 },
                            );
                            true
                        }
                        None => false,
                    }
                }
                _ => false,
            };
            if let Some(c) = calib_ctl.as_ref().filter(|_| calib_drawn || calib_state.is_some()) {
                // the controller's instructions along the bottom of this pane
                let lines = termpaper::calibrate::help_lines(&c.state);
                let h = (lines.len() as u16).min(area.height);
                let rect = Rect { x: area.x, y: area.y + area.height.saturating_sub(h), width: area.width, height: h };
                f.render_widget(
                    Paragraph::new(Text::raw(lines.join("\n"))).style(
                        Style::new().fg(ratatui::style::Color::Rgb(235, 235, 235)).bg(ratatui::style::Color::Rgb(16, 18, 26)),
                    ),
                    rect,
                );
            }
            if let Some(frame) = rendered.as_ref().filter(|_| !calib_drawn) {
                #[allow(unused_mut)]
                let mut drawn = false;
                #[cfg(feature = "gpu")]
                if let Some(words) = &frame.cells {
                    // the frame's own grid: after a resize the last frame
                    // (same simulation, old view) stays up until the next
                    let cells = termpaper::gpu::FrameCells {
                        words, cols: frame.grid.0, rows: frame.grid.1,
                    };
                    termpaper::gpu::blit(&cells, &frame.canvas, crop, shown_pixels, area,
                        f.buffer_mut(), settings.truecolor);
                    drawn = true;
                }
                if !drawn {
                    match classic_map.filter(|(o, s)| render::view_is_crop(*o, *s, shown_pixels).is_none()) {
                        // a pane whose cells differ from the shared canvas
                        Some((o, stp)) => render::draw_view(&frame.canvas, o, stp, area, f.buffer_mut(),
                            settings.truecolor, shown_pixels, &mut classic_scratch),
                        None => render::draw_crop(&frame.canvas, crop.0 as i32, crop.1 as i32, area,
                            f.buffer_mut(), settings.truecolor, shown_pixels),
                    }
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
                let status = worker_status.clone().or_else(|| rendered.as_ref().map(|f| format!("{} · worker {:.1} ms · lead {}", f.backend, f.render_ms, lead.lead())))
                    .unwrap_or_else(|| "Renderer initializing…".into());
                let wall_status = match (my_plan.as_ref(), wall_layout) {
                    (Some((_, pp)), _) => format!(
                        "wall: physical plan r{} · {} panes · this one on {}",
                        wall_plan.as_ref().map(|p| p.rev).unwrap_or(0),
                        wall_plan.as_ref().map(|p| p.panes.len()).unwrap_or(0),
                        pp.monitor
                    ),
                    (None, Some(l)) => format!("wall: {}x{} cells @ ({},{})", l.virtual_w, l.virtual_h, l.crop_x, l.crop_y),
                    _ => "wall: local".into(),
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
                let g = &settings.look.get().grade;
                color_wheel::render(f, area, g.hue, g.saturation, g.contrast, color_param);
            }
        })?;
        crossterm::execute!(terminal.backend_mut(), crossterm::terminal::EndSynchronizedUpdate)?;

        // A rebuilt simulation that would need more than a few seconds of
        // replay to reach the anchor's clock (an hours-old anchor, a wall
        // resize) asks for a fresh anchor instead: the leader restarts the
        // scene for the whole group, together, after a fade.
        if st.next.is_none() && worker.catchup_ms().is_some_and(|eta| eta > REANCHOR_ETA_MS) {
            match guard.as_mut().filter(|_| st.synced) {
                Some(g) => {
                    g.set_reanchor(Some(st.cur.stamp));
                    reanchor_wanted = Some(st.cur.stamp);
                }
                None => {
                    let mut a = st.cur.clone();
                    a.seed = rand::rng().random();
                    begin_switch_paused(&mut st, &mut guard, &mut transition, &names, a, false, true);
                }
            }
        }
        // the leader answers re-anchor requests for the anchor it runs
        if st.synced && st.next.is_none() && guard.is_some() && is_leader(&peers.list, true) {
            let stamp = st.cur.stamp;
            let wanted = reanchor_wanted == Some(stamp) || peers.list.iter().any(|i| i.reanchor == Some(stamp));
            if wanted {
                let mut a = st.cur.clone();
                a.seed = rand::rng().random();
                begin_switch_paused(&mut st, &mut guard, &mut transition, &names, a, true, true);
            }
        }

        // steady pacing: coarse sleep to ~1ms before the deadline, then
        // Sleep (inside the event poll) to just before the deadline, then spin
        // the last SPIN_TAIL for steady timing. See examples/pace_bench.rs: at
        // 240fps a 1ms tail costs ~22% of a core per instance purely spinning,
        // and several instances pacing on one machine then starve each other
        // into dropped frames. 100us holds jitter at ~0.025ms — 0.6% of a
        // 240fps frame — for ~1% CPU.
        //
        // The deadline is the next slot of the anchor's grid, not "last frame
        // + period": every pane with this anchor and rate presents at the same
        // moments, and a late frame does not push every later one back.
        let deadline = clock.slot_instant(grid_origin, period, slot + 1);
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
                    Event::Mouse(mouse) => {
                        let Some(input) = menu::Input::from_mouse(mouse) else {
                            continue;
                        };
                        if menu.open {
                            let ctx = menu_ctx(&settings, &opts, names[idx], String::new(), String::new(), Vec::new());
                            pending_fx.extend(menu.handle(input, &ctx));
                            if !pending_fx.is_empty() || !menu.open {
                                break;
                            }
                        } else if !color_open
                            && matches!(input, menu::Input::Mouse { kind: menu::Mouse::Down, .. })
                            && !settings.screensaver
                        {
                            // a click opens the menu: the way in for mouse users
                            let ctx = menu_ctx(&settings, &opts, names[idx], String::new(), String::new(), Vec::new());
                            menu.open(&ctx);
                            break;
                        }
                        continue;
                    }
                    _ => {}
                }
                if let Event::Key(key) = ev {
                if key.kind == KeyEventKind::Press {
                    let km = &settings.keymap;

                    // a calibration opened from the menu takes every key
                    if let Some(c) = &mut calib_ctl {
                        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                        let alt = key.modifiers.contains(KeyModifiers::ALT);
                        match c.key(key.code, shift, alt) {
                            termpaper::calibrate::Outcome::Continue => {}
                            _ => calib_ctl = None,
                        }
                        break;
                    }

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
                            let mut look = settings.look.get().clone();
                            look.grade.hue = 0.0;
                            look.grade.saturation = 1.0;
                            look.grade.contrast = 1.0;
                            settings.look.set(look);
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
                            let mut look = settings.look.get().clone();
                            let g = &mut look.grade;
                            match color_param {
                                color_wheel::Param::Hue => {
                                    g.hue = color_wheel::step_hue_steps(g.hue, steps);
                                }
                                color_wheel::Param::Saturation => {
                                    g.saturation = color_wheel::step_sat(g.saturation, steps as f32 * color_wheel::SAT_STEP);
                                }
                                color_wheel::Param::Contrast => {
                                    g.contrast = color_wheel::step_contrast(g.contrast, steps as f32 * color_wheel::CONTRAST_STEP);
                                }
                            }
                            settings.look.set(look);
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
                        // the reset may have moved us to the default group
                        join_group(&mut st, &mut guard, &mut transition, &names, &mut settings, &mut opts);
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
                            if settings.look.get().basic_grade_is_neutral() {
                                let mut look = settings.look.get().clone();
                                look.grade.hue = 45.0;
                                settings.look.set(look);
                            }
                        } else {
                            save.mark(Instant::now());
                        }
                        continue;
                    }
                    // relative to the scene a pending switch lands on, so
                    // rapid presses keep stepping
                    let base = names.iter().position(|n| *n == st.target().scene).unwrap_or(idx);
                    let mut switch_to = |i: usize| {
                        // the publisher picks the seed and start time; every
                        // pane builds the scene from them
                        switch_scene(i % names.len(), &mut st, &mut guard, &mut transition,
                            &names, &settings, &mut opts);
                        last_switch = Instant::now();
                    };
                    if km.matches("next", key.code) || key.code == KeyCode::Tab {
                        switch_to(base + 1);
                    } else if km.matches("prev", key.code) {
                        switch_to(base + names.len() - 1);
                    } else if km.matches("pause", key.code) {
                        // wall-wide: a retime anchor freezes (or resumes) the
                        // shared clock; the simulation itself is untouched
                        let now_ms = link::epoch_now_ms();
                        let mut a = st.target().clone();
                        if a.paused() { a.resume(now_ms) } else { a.pause(now_ms) }
                        if let (true, Some(g)) = (st.target_synced(), guard.as_mut()) {
                            g.publish_anchor(&mut a);
                        }
                        st.retime(&a, &mut transition, &names);
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
