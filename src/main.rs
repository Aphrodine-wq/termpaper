//! termpaper — Wallpaper Engine for the terminal.

use termpaper::{canvas, color_grade, color_wheel, config, filter, link, marketplace, menu, render, scene, transition, wall};

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use menu::{Effect, Menu, MenuCtx};
use rand::{rngs::StdRng, RngExt, SeedableRng};
use render::Pixels;
use scene::{Detail, SceneOptions};
use ratatui::{layout::Rect, style::Style, text::Text, widgets::Paragraph};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "termpaper",
    version,
    about = "Wallpaper Engine for the terminal: animated truecolor scenes"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

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

    /// Disable video-wall mode
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

#[derive(Subcommand)]
enum Command {
    /// Browse, install, and publish community scenes from GitHub
    Marketplace {
        #[command(subcommand)]
        action: MarketplaceAction,
    },
}

#[derive(Subcommand)]
enum MarketplaceAction {
    /// List scenes in the marketplace catalog
    List {
        #[arg(long)]
        json: bool,
    },
    /// Search the catalog
    Search {
        query: String,
    },
    /// Show details for a catalog id or scene name
    Info {
        id: String,
    },
    /// Clone a scene from GitHub and rebuild termpaper
    Install {
        id: String,
        #[arg(long)]
        no_rebuild: bool,
    },
    /// Remove an installed community scene
    Remove {
        id: String,
    },
    /// List locally installed community scenes
    Installed,
    /// Re-download installed scenes from GitHub
    Update,
    /// Rebuild termpaper with installed community scenes
    Rebuild,
    /// How to publish your own scene on GitHub
    Publish,
}

fn run_marketplace(action: MarketplaceAction) -> std::io::Result<()> {
    let code = match action {
        MarketplaceAction::List { json } => marketplace::cmd_list(json),
        MarketplaceAction::Search { query } => marketplace::cmd_search(&query),
        MarketplaceAction::Info { id } => marketplace::cmd_info(&id),
        MarketplaceAction::Install { id, no_rebuild } => marketplace::cmd_install(&id, no_rebuild),
        MarketplaceAction::Remove { id } => marketplace::remove(&id),
        MarketplaceAction::Installed => marketplace::cmd_installed(),
        MarketplaceAction::Update => marketplace::cmd_update(),
        MarketplaceAction::Rebuild => marketplace::rebuild(),
        MarketplaceAction::Publish => marketplace::cmd_publish(),
    };
    match code {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("termpaper: {e}");
            std::process::exit(1);
        }
    }
}

fn detect_truecolor() -> bool {
    std::env::var("COLORTERM")
        .map(|v| v.contains("truecolor") || v.contains("24bit"))
        .unwrap_or(false)
}

fn entropy_rng() -> StdRng {
    StdRng::from_rng(&mut rand::rng())
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

    if let Some(Command::Marketplace { action }) = args.command {
        return run_marketplace(action);
    }

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
        if scene::create(scene, &Default::default(), entropy_rng()).is_none() {
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
        let out: String = scene::catalog()
            .iter()
            .map(|(name, desc)| format!("{name:<12} {desc}\n"))
            .collect();
        use std::io::Write;
        let _ = std::io::stdout().write_all(out.as_bytes());
        return Ok(());
    }

    let cfg = config::load();
    let names = scene::names();

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
    let probe = SceneOptions {
        theme: None,
        detail,
        text_scale: None,
    };
    if scene::create(&scene_name, &probe, entropy_rng()).is_none() {
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
    let fps = args.fps.or(cfg.fps).unwrap_or(60).clamp(1, 240);
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
    let settings = Settings {
        link_enabled,
        link_group,
        wall_spec: if args.no_wall { None } else { args.wall.clone() },
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
        speed,
        cycle,
        hue_shift,
        saturation,
        contrast,
        screensaver: args.screensaver,
        truecolor: detect_truecolor() && !args.no_truecolor,
    };

    let mut terminal = ratatui::init();
    terminal.hide_cursor()?;
    let result = run(&mut terminal, &scene_name, settings);
    ratatui::restore();
    result
}

struct Settings {
    link_enabled: bool,
    link_group: String,
    wall_spec: Option<String>,
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
    speed: f32,
    cycle: Option<f64>,
    hue_shift: f32,
    saturation: f32,
    contrast: f32,
    screensaver: bool,
    truecolor: bool,
}

/// Snapshot the current runtime settings for broadcast.
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

fn apply_defaults(
    settings: &mut Settings,
    opts: &mut SceneOptions,
    scene_name: &str,
    transition: &mut transition::Transition,
    guard: &mut Option<link::Guard>,
    quick_filter: &mut Option<String>,
    current: &mut Box<dyn scene::Scene>,
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
    settings.link_enabled = config::DEFAULT_LINK;
    settings.link_group = config::DEFAULT_GROUP.into();

    opts.detail = settings.detail;
    opts.theme = None;
    opts.text_scale = None;
    *quick_filter = None;

    transition.set_fade_secs(settings.fade);
    *current = scene::create(scene_name, opts, entropy_rng()).expect("registry");
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
    let names = scene::names();
    let mut guard = if settings.link_enabled {
        link::Guard::new(start_scene, &settings.link_group).map(|mut g| {
            g.set_pad((settings.pad, settings.pad));
            g
        })
    } else {
        None
    };
    let mut control_stamp = link::Stamp::default();
    // artwork sync: (seed, t0_ms) for the next scene creation, if any
    let mut sync_params: Option<(u64, u64)> = None;
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
    };
    let mut transition = transition::Transition::new();
    transition.set_fade_secs(settings.fade);
    let mut paused = false;
    let mut quick_filter: Option<String> = None;
    let mut idx = names
        .iter()
        .position(|n| *n == start_scene)
        .unwrap_or(0);
    let mut current = scene::create(names[idx], &opts, entropy_rng()).expect("validated");
    let mut canvas = canvas::Canvas::new(1, 1);
    let mut prev_canvas = canvas::Canvas::new(1, 1);
    let mut wall_layout: Option<wall::WallLayout> = None;
    let mut wall_refresh = Instant::now() - Duration::from_secs(10);

    let launch = Instant::now();
    let mut last_switch = Instant::now();
    let mut last_frame = Instant::now();
    let mut last_heartbeat = Instant::now();
    // cached "HH:MM" for the clock overlay (refreshed at most every 10s)
    let mut clock_text = String::new();
    let mut clock_stamp = Instant::now() - Duration::from_secs(60);

    loop {
        let now = Instant::now();
        let raw_dt = (now - last_frame).as_secs_f32().min(1.0 / 30.0);
        let dt = if paused {
            0.0 // frozen
        } else {
            (now - last_frame).as_secs_f32().min(1.0 / 30.0) * settings.speed
        };
        last_frame = now;
        let frame_dur = Duration::from_secs_f64(1.0 / settings.fps as f64);

        // scene cycling
        if let Some(secs) = settings.cycle {
            if now.duration_since(last_switch).as_secs_f64() >= secs {
                sync_params = None; // local cycle: no shared seed
                cur_sync = None;
                transition.request((idx + 1) % names.len());
                last_switch = now;
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
                    let leader = link::list_instances_in_group(&settings.link_group)
                        .iter()
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

        // instance linking: control channel poll (cheap stat per frame)
        if let Some(g) = &guard {
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
                                if let Some(t) = &ctrl.theme {
                                    opts.theme = Some(t.clone());
                                }
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
                            if recreate {
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
            opts.theme = settings
                .cfg
                .themes
                .get(names[idx])
                .cloned()
                .or_else(|| settings.theme.clone());
            // artwork sync: linked switches carry (seed, t0) so every
            // instance builds the identical simulation
            let sp = sync_params.take();
            cur_sync = sp;
            let rng = match sp {
                Some((seed, _)) => StdRng::seed_from_u64(seed),
                None => entropy_rng(),
            };
            current = scene::create(names[idx], &opts, rng).expect("registry");
            if let Some((_, t0)) = sp {
                // fast-forward to the publisher's sim time — time-boxed by
                // WALL CLOCK (a slightly younger scene beats a frozen one)
                let elapsed_ms = link::epoch_now_ms().saturating_sub(t0);
                let steps = ((elapsed_ms as f32 / (1000.0 / 60.0)) as usize).min(3600);
                let mut scratch = canvas::Canvas::new(canvas.width().max(1), canvas.height().max(1));
                let ff_start = Instant::now();
                for _ in 0..steps {
                    current.update(1.0 / 60.0, &mut scratch);
                    if ff_start.elapsed() > Duration::from_millis(200) {
                        break; // silently settle slightly younger
                    }
                }
            }
            if let Some(g) = &mut guard {
                g.set_scene(names[idx]);
            }
            last_switch = now;
        }

        // video wall: refresh layout every 2s from registry geometry
        if (settings.link_enabled || settings.wall_spec.is_some())
            && wall_refresh.elapsed() > Duration::from_secs(2)
        {
            wall_refresh = Instant::now();
            let (cols, rows) = (terminal.size()?.width as usize, terminal.size()?.height as usize);
            let geo = wall::own_geo();
            if let Some(g) = &mut guard {
                g.set_geometry(cols, rows, geo.map(|g| (g.x, g.y, g.w, g.h)));
            }
            wall_layout = if let Some(spec) = &settings.wall_spec {
                wall::manual_layout(spec, cols, rows)
            } else if settings.link_enabled {
                let mut parts: Vec<wall::Participant> = link::list_instances_in_group(&settings.link_group)
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
            let (w, h2) = match wall_layout {
                Some(l) => (l.virtual_w * pw, l.virtual_h * ph),
                None => (area.width as usize * pw, area.height as usize * ph),
            };
            if canvas.width() != w || canvas.height() != h2 {
                canvas.resize(w, h2);
            }
            current.update(dt, &mut canvas);
            filter::apply_all(&settings.filters, &mut canvas, launch.elapsed().as_secs_f32());
            color_grade::apply(
                &mut canvas,
                settings.hue_shift,
                settings.saturation,
                settings.contrast,
            );
            if let Some(qf) = &quick_filter {
                filter::apply(qf, &mut canvas, launch.elapsed().as_secs_f32());
            }
            canvas::dim(&mut canvas, fade * settings.dim);
            // temporal smoothing (glyph cells excluded inside smooth_blend)
            if settings.smooth > 0.001 {
                canvas.smooth_blend(&prev_canvas, 1.0 - settings.smooth);
                prev_canvas = canvas.clone_for_smooth();
            }
            match wall_layout {
                Some(l) => render::draw_crop(
                    &canvas,
                    l.crop_x as i32 * pw as i32,
                    l.crop_y as i32 * ph as i32,
                    area,
                    f.buffer_mut(),
                    settings.truecolor,
                    settings.pixels,
                ),
                None => render::draw(&canvas, area, f.buffer_mut(), settings.truecolor, settings.pixels),
            }

            // bottom-left hint, fading out over its last second
            let hint_age = launch.elapsed().as_secs_f32();
            if hint_age < 4.0 && !menu.open {
                let a = 1.0 - (hint_age - 3.0).clamp(0.0, 1.0);
                let g = (90.0 * a) as u8;
                if g > 8 {
                    let hint = format!("? menu · c color grade · ←/→ scene · q quit · {}", current.name());
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
                        link::list_instances_in_group(&settings.link_group)
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
                            .collect()
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
        // spin-wait the tail; input is processed whenever it arrives
        let deadline = last_frame + frame_dur;
        loop {
            let now2 = Instant::now();
            if now2 >= deadline {
                break;
            }
            let remaining = deadline - now2;
            let coarse = remaining.saturating_sub(Duration::from_millis(1));
            let has_event = if coarse.is_zero() {
                event::poll(Duration::ZERO)?
            } else {
                event::poll(coarse)?
            };
            if has_event {
                if let Event::Key(key) = event::read()? {
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
                            &mut current,
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
                                        transition.request(i);
                                        if let Some(g) = &mut guard {
                                            g.publish(names[i], None, seed, t0);
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
                                        current = scene::create(names[idx], &opts, entropy_rng())
                                            .expect("registry");
                                        if let Some(g) = &mut guard {
                                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                                        }
                                    }
                                    Effect::SetTheme(t) => {
                                        if let Some(g) = &mut guard {
                                            let seed: u64 = rand::rng().random();
                                            let t0 = link::epoch_now_ms();
                                            sync_params = Some((seed, t0));
                                            g.publish(names[idx], t.as_deref(), seed, t0);
                                        }
                                        opts.theme = t;
                                        current = scene::create(names[idx], &opts, entropy_rng())
                                            .expect("registry");
                                    }
                                    Effect::SetTextScale(ts) => {
                                        settings.text_scale = ts;
                                        opts.text_scale = ts;
                                        current = scene::create(names[idx], &opts, entropy_rng())
                                            .expect("registry");
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
                        transition.request(i);
                        if let Some(g) = &mut guard {
                            g.publish(names[i], None, seed, t0);
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
                        current = scene::create(names[idx], &opts, entropy_rng()).expect("registry");
                        if let Some(g) = &mut guard {
                            g.publish_settings(&settings_msg(&settings, &opts, &quick_filter));
                        }
                    }
                }
            }
            }
            // spin the tail for steady frame timing
            while Instant::now() < deadline {
                std::hint::spin_loop();
            }
            break;
        }
    }
}
