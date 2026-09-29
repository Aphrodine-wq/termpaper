//! termpaper — Wallpaper Engine for the terminal. Live truecolor worlds.

use termpaper::{color_wheel, config, filter, link, menu, render, scene, transition, wall};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use menu::{Effect, Menu, MenuCtx};
use rand::RngExt;
use render::Pixels;
use scene::{Detail, SceneOptions};
use ratatui::{layout::Rect, style::Style, text::Text, widgets::Paragraph};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "termpaper",
    version,
    about = "Wallpaper Engine for the terminal — 120fps, 22 filters, sync clusters, seamless walls"
)]
struct Args {
    /// Scene to run (see --list)
    scene: Option<String>,

    /// List available scenes and exit
    #[arg(long)]
    list: bool,

    /// Rotate through all scenes every N seconds
    #[arg(long)]
    cycle: Option<f64>,

    /// Target frames per second
    #[arg(long)]
    fps: Option<u32>,

    /// Throttle to this fps while the terminal is unfocused (needs a
    /// terminal that reports focus; off unless set)
    #[arg(long)]
    idle_fps: Option<u32>,

    /// Animation speed multiplier
    #[arg(long)]
    speed: Option<f32>,

    /// Scene color theme (see --list; e.g. nexus: cyan/amber/violet/mono)
    #[arg(long)]
    theme: Option<String>,

    /// Post-processing filter (repeatable): scanlines, vignette, grain,
    /// warm, cool, hue, crt
    #[arg(long)]
    filter: Vec<String>,

    /// Detail level: low, medium or high (particle/layer counts)
    #[arg(long)]
    detail: Option<String>,

    /// Pixel mode: half, quad or braille
    #[arg(long)]
    pixels: Option<String>,

    /// Bump text scale: 1, 2 or 3
    #[arg(long)]
    text_scale: Option<u32>,

    /// Force 256-color output even on truecolor terminals
    #[arg(long)]
    no_truecolor: bool,

    /// Legacy alias for --renderer gpu. Requires --features gpu and Vulkan;
    /// falls back to CPU if unavailable (shown in the settings menu).
    #[arg(long)]
    gpu: bool,

    /// Rendering backend: auto = GPU post-processing when compiled and
    /// available, scenes run on the CPU; gpu = same, but reports if the GPU
    /// is missing; cpu = everything on the CPU; shader = draw every scene
    /// from its experimental WGSL world instead of the Rust scene
    #[arg(long, value_enum)]
    renderer: Option<termpaper::engine::Renderer>,

    /// Screensaver mode: any key exits
    #[arg(long)]
    screensaver: bool,

    /// Enable instance linking (default on)
    #[arg(long, overrides_with = "no_link")]
    link: bool,

    /// Disable instance linking
    #[arg(long)]
    no_link: bool,

    /// List live termpaper instances and exit
    #[arg(long)]
    instances: bool,

    /// Publish a scene switch to all running instances and exit
    #[arg(long)]
    switch: Option<String>,

    /// Link group for this instance (instances in the same group sync)
    #[arg(long)]
    group: Option<String>,

    /// With --switch, publish to every link group
    #[arg(long)]
    all_groups: bool,

    /// Never join a video wall: render the local canvas and hide this
    /// window's geometry from peers (linking still syncs scenes)
    #[arg(long)]
    no_wall: bool,

    /// Manual video-wall tiling: COLSxROWS:INDEX (e.g. 2x1:0)
    #[arg(long)]
    wall: Option<String>,

    /// Terminal padding in px (all sides) so wall crops line up across
    /// window borders despite the margin
    #[arg(long)]
    pad: Option<i32>,
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
    let args = Args::parse();

    // termpaper is a color-art program: color output is the entire point.
    // crossterm honors NO_COLOR by stripping all colors, which would render
    // every scene as blank gray blocks — explicitly re-enable colors.
    crossterm::style::Colored::set_ansi_color_disabled(false);

    if args.instances {
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
        return Ok(());
    }

    if let Some(scene) = &args.switch {
        if !scene::exists(scene) {
            eprintln!("termpaper: unknown scene '{scene}'. See --list.");
            std::process::exit(2);
        }
        let seed: u64 = rand::rng().random();
        let t0 = link::epoch_now_ms();
        if args.all_groups {
            link::publish_remote_all_groups(scene, seed, t0)?;
            println!("published switch to '{scene}' (all groups)");
        } else {
            let group = args
                .group
                .as_deref()
                .map(link::sanitize_group)
                .unwrap_or_else(|| "default".into());
            link::publish_remote(scene, seed, t0, &group)?;
            println!("published switch to '{scene}' (group {group})");
        }
        return Ok(());
    }

    if args.list {
        // one write: piping into `head` shouldn't panic on SIGPIPE
        let mut out = String::new();
        for cat in scene::Category::ALL {
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
        return Ok(());
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
        .unwrap_or_else(|| "rain".to_string());
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

/// Merge current runtime settings into the stored config and save it.
fn persist(settings: &mut Settings, scene_name: &str, opts: &SceneOptions) {
    let cfg = &mut settings.cfg;
    cfg.scene = Some(scene_name.to_string());
    cfg.pixels = Some(settings.pixels.name().to_string());
    cfg.detail = Some(opts.detail.name().to_string());
    cfg.filters = settings.filters.clone();
    cfg.text_scale = settings.text_scale;
    cfg.smooth = Some(settings.smooth);
    cfg.dim = Some(settings.dim);
    cfg.fade = Some(settings.fade);
    cfg.clock = Some(settings.clock);
    cfg.cycle = settings.cycle;
    cfg.fps = Some(settings.fps);
    cfg.renderer = Some(settings.renderer);
    cfg.speed = Some(settings.speed);
    cfg.hue_shift = if settings.hue_shift < 0.5 {
        None
    } else {
        Some(settings.hue_shift)
    };
    cfg.saturation = if (settings.saturation - 1.0).abs() < 0.02 {
        None
    } else {
        Some(settings.saturation)
    };
    cfg.contrast = if (settings.contrast - 1.0).abs() < 0.02 {
        None
    } else {
        Some(settings.contrast)
    };
    cfg.link = if settings.link_enabled { None } else { Some(false) };
    cfg.wall = if settings.wall_enabled { None } else { Some(false) };
    cfg.group = if settings.link_group == "default" {
        None
    } else {
        Some(settings.link_group.clone())
    };
    if let Some(t) = &opts.theme {
        cfg.themes.insert(scene_name.to_string(), t.clone());
    }
    if let Err(e) = config::save(cfg) {
        eprintln!("termpaper: could not save config: {e}");
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
    let geo_watcher = if (settings.link_enabled && settings.wall_enabled) || settings.wall_spec.is_some() {
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

        // scene cycling
        if let Some(secs) = settings.cycle {
            if now.duration_since(last_switch).as_secs_f64() >= secs {
                sync_params = None; // local cycle: no shared seed
                cur_sync = None;
                if let Some(g) = &mut guard {
                    g.set_synced(false);
                }
                transition.request((idx + 1) % names.len());
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
                let ctx = MenuCtx {
                    renderer_status: worker_status.clone().or_else(|| rendered.as_ref().map(|f| format!("{} · worker {:.1} ms", f.backend, f.render_ms)))
                        .unwrap_or_else(|| "Renderer initializing…".into()),
                    wall_status: match wall_layout {
                        Some(l) => format!("wall: {}x{} cells @ ({},{})", l.virtual_w, l.virtual_h, l.crop_x, l.crop_y),
                        None => "wall: local".into(),
                    },
                    scene_name: names[idx],
                    scene_idx: idx,
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
                    hue_shift: settings.hue_shift,
                    saturation: settings.saturation,
                    contrast: settings.contrast,
                    link_enabled: settings.link_enabled,
                    link_group: settings.link_group.clone(),
                    truecolor: settings.truecolor,
                    filters: settings.filters.clone(),
                    instances: if settings.link_enabled {
                        peers.menu_lines.clone()
                    } else {
                        vec!["linking disabled (solo art)".into()]
                    },
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
                };
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
                            persist(&mut settings, names[idx], &opts);
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

                    if km.matches("reset", key.code) {
                        apply_defaults(
                            &mut settings,
                            &mut opts,
                            names[idx],
                            &mut transition,
                            &mut guard,
                            &mut quick_filter,
                        );
                        persist(&mut settings, names[idx], &opts);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(
                                &settings,
                                &opts,
                                &quick_filter,
                            ));
                        }
                        continue;
                    }

                    if menu.open {
                        // modal: Esc or the menu key closes, rest routes in
                        if key.code == KeyCode::Esc || km.matches("menu", key.code) {
                            menu.close();
                            continue;
                        }
                        let input = match key.code {
                            KeyCode::Up => Some(menu::Input::Up),
                            KeyCode::Down => Some(menu::Input::Down),
                            KeyCode::Left => Some(menu::Input::Left),
                            KeyCode::Right => Some(menu::Input::Right),
                            KeyCode::Enter => Some(menu::Input::Enter),
                            _ => None,
                        };
                        if let Some(input) = input {
                            let ctx = MenuCtx {
                                renderer_status: String::new(),
                                wall_status: String::new(),
                                scene_name: names[idx],
                                scene_idx: idx,
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
                                hue_shift: settings.hue_shift,
                                saturation: settings.saturation,
                                contrast: settings.contrast,
                                link_enabled: settings.link_enabled,
                                link_group: settings.link_group.clone(),
                                truecolor: settings.truecolor,
                                filters: settings.filters.clone(),
                                instances: Vec::new(),
                                key_display: Vec::new(),
                            };
                            let effects = menu.handle(input, &ctx);
                            for fx in effects {
                                match fx {
                                    Effect::SwitchScene(i) => {
                                        let i = i % names.len();
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
                                    }
                                    Effect::SetPixels(p) => {
                                        settings.pixels = p;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetDetail(d) => {
                                        opts.detail = d;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
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
                                    }
                                    Effect::SetTextScale(ts) => {
                                        settings.text_scale = ts;
                                        opts.text_scale = ts;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetSpeed(sp) => {
                                        settings.speed = sp;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetFps(fps) => {
                                        settings.fps = fps;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetSmooth(sm) => {
                                        settings.smooth = sm;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetDim(d) => {
                                        settings.dim = d;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetFade(fd) => {
                                        settings.fade = fd;
                                        transition.set_fade_secs(fd);
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetClock(c) => {
                                        settings.clock = c;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetCycle(c) => {
                                        // local-only: rotations don't propagate
                                        settings.cycle = c;
                                        last_switch = Instant::now();
                                    }
                                    Effect::SetHueShift(h) => {
                                        settings.hue_shift = h;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(
                                                &settings,
                                                &opts,
                                                &quick_filter,
                                            ));
                                        }
                                    }
                                    Effect::SetSaturation(s) => {
                                        settings.saturation = s;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(
                                                &settings,
                                                &opts,
                                                &quick_filter,
                                            ));
                                        }
                                    }
                                    Effect::SetContrast(c) => {
                                        settings.contrast = c;
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(
                                                &settings,
                                                &opts,
                                                &quick_filter,
                                            ));
                                        }
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
                                        persist(&mut settings, names[idx], &opts);
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
                                        persist(&mut settings, names[idx], &opts);
                                    }
                                    Effect::ToggleFilter(f) => {
                                        if let Some(pos) =
                                            settings.filters.iter().position(|x| *x == f)
                                        {
                                            settings.filters.remove(pos);
                                        } else {
                                            settings.filters.push(f);
                                        }
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::Persist => persist(&mut settings, names[idx], &opts),
                                }
                            }
                        }
                        continue;
                    }

                    let quit = settings.screensaver
                        || km.matches("quit", key.code)
                        || matches!(key.code, KeyCode::Esc)
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL));
                    if quit {
                        return Ok(());
                    }
                    if km.matches("menu", key.code) {
                        menu.toggle(idx);
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
                            persist(&mut settings, names[idx], &opts);
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
                        persist(&mut settings, names[idx], &opts);
                    } else if km.matches("fps_down", key.code) {
                        settings.fps = menu::fps_step(settings.fps, false);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                        persist(&mut settings, names[idx], &opts);
                    } else if km.matches("speed_up", key.code) {
                        settings.speed = menu::speed_step(settings.speed, true);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                        persist(&mut settings, names[idx], &opts);
                    } else if km.matches("speed_down", key.code) {
                        settings.speed = menu::speed_step(settings.speed, false);
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                        persist(&mut settings, names[idx], &opts);
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
