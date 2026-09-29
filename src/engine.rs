//! A single rendering worker with a bounded, latest-request mailbox.
//! Terminal input and menus never wait for scene initialization or GPU work.
use crate::{
    canvas::Canvas,
    color_grade, filter,
    render::Pixels,
    scene::{shader::ShaderSpec, SceneOptions},
    sync::Playback,
};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Renderer {
    /// GPU post-processing when compiled and available; scenes run on the CPU.
    #[default]
    Auto,
    /// Like `Auto`, but reports when the GPU was requested and is missing.
    Gpu,
    /// Everything on the CPU.
    Cpu,
    /// GPU post-processing and every Classic scene drawn from its WGSL arm
    /// (the experimental GPU worlds) instead of the Rust scene.
    Shader,
}

/// How one request is rendered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    /// Scene, filters and packing on the CPU.
    Cpu,
    /// Rust scene on the CPU; filters, grading and cell packing on the GPU.
    GpuPost,
    /// Classic scene drawn from its WGSL world arm, post on the GPU.
    GpuWorld,
    /// A Studio shader scene: everything on the GPU.
    GpuShader(&'static ShaderSpec),
    /// A Studio scene with no GPU to draw it: its Classic fallback on the CPU.
    ShaderFallback(&'static str),
}

impl Backend {
    pub fn gpu_post(self) -> bool {
        matches!(self, Backend::GpuPost | Backend::GpuWorld | Backend::GpuShader(_))
    }
}

/// Default GPU time budget for a Studio scene pass, per frame.
pub const DEFAULT_GPU_BUDGET_MS: f32 = 3.0;

/// Below this pixel count the GPU round trip costs more than the arithmetic
/// it saves; the worker stays on the CPU path. Roughly an 80x25 terminal in
/// half mode.
pub const MIN_GPU_PIXELS: usize = 4_000;

/// Decide how one request is rendered.
///
/// Studio scenes go to the GPU whenever there is one (they have no CPU
/// renderer, so the small-canvas cutoff does not apply) and to their Classic
/// fallback otherwise. Classic scenes are Rust artwork first: the GPU
/// post-processes them, and draws them from their WGSL world arm only when
/// opted in via `scene::GPU_WORLD_SCENES` or `--renderer shader`. `bump`
/// always keeps its CPU text layout.
pub fn plan_backend(renderer: Renderer, has_gpu: bool, size: (usize, usize), name: &str) -> Backend {
    if let Some(spec) = crate::scene::shader::find(name) {
        return if has_gpu && renderer != Renderer::Cpu {
            Backend::GpuShader(spec)
        } else {
            Backend::ShaderFallback(spec.fallback)
        };
    }
    let post = has_gpu && renderer != Renderer::Cpu && size.0 * size.1 >= MIN_GPU_PIXELS;
    let world = post
        && name != "bump"
        && (renderer == Renderer::Shader || crate::scene::gpu_world(name));
    match (post, world) {
        (true, true) => Backend::GpuWorld,
        (true, false) => Backend::GpuPost,
        _ => Backend::Cpu,
    }
}

/// Human-readable backend line for the settings menu.
fn describe_backend(
    renderer: Renderer,
    adapter: Option<&str>,
    errored: bool,
    backend: Backend,
) -> String {
    match adapter {
        _ if matches!(backend, Backend::ShaderFallback(_)) => {
            let Backend::ShaderFallback(fb) = backend else { unreachable!() };
            let why = if errored {
                "GPU error"
            } else if renderer == Renderer::Cpu {
                "CPU selected"
            } else if cfg!(feature = "gpu") {
                "no GPU"
            } else {
                "GPU feature not compiled"
            };
            format!("CPU · {why} · showing Classic '{fb}'")
        }
        Some(a) if matches!(backend, Backend::GpuShader(_)) => format!("GPU shader · {a}"),
        Some(a) if backend == Backend::GpuWorld => format!("GPU world · {a}"),
        Some(a) if backend == Backend::GpuPost => format!("GPU post · {a} · CPU scene"),
        Some(_) => "CPU · small canvas".into(),
        None if errored => "CPU · GPU error (fallback)".into(),
        None => match renderer {
            Renderer::Cpu => "CPU · selected".into(),
            Renderer::Gpu | Renderer::Shader => "CPU · GPU requested but unavailable".into(),
            Renderer::Auto => {
                if cfg!(feature = "gpu") {
                    "CPU · GPU unavailable".into()
                } else {
                    "CPU · GPU feature not compiled".into()
                }
            }
        },
    }
}

/// Everything that decides a simulation's state. Changing any of it
/// rebuilds the simulation: a new worker generation, replayed up to the
/// clock out of sight.
#[derive(Clone, PartialEq)]
pub struct SimKey {
    pub name: String,
    pub seed: u64,
    /// with `pixels` set to the mode the canvas is shown in: Classic scenes
    /// compose for its pixel aspect
    pub opts: SceneOptions,
    /// canvas size in pixels; (0, 0) for Studio scenes
    pub size: (usize, usize),
    pub speed: f32,
}

impl std::fmt::Debug for SimKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimKey")
            .field("name", &self.name)
            .field("seed", &self.seed)
            .field("theme", &self.opts.theme)
            .field("detail", &self.opts.detail)
            .field("text_scale", &self.opts.text_scale)
            .field("pixels", &self.opts.pixels)
            .field("size", &self.size)
            .field("speed", &self.speed)
            .finish()
    }
}

impl SimKey {
    /// The key for a scene. A Studio scene is a stateless function of time
    /// and position, so its canvas size and pixel mode are view, not sim:
    /// they are cleared here and a wall or pixel change never rebuilds it.
    /// (Its Classic fallback on a GPU-less machine rebuilds inside
    /// `Playback`, which watches its own size.)
    pub fn new(name: &str, seed: u64, mut opts: SceneOptions, size: (usize, usize), speed: f32) -> Self {
        let size = if crate::scene::shader::find(name).is_some() {
            opts.pixels = Pixels::default();
            (0, 0)
        } else {
            size
        };
        SimKey { name: name.to_string(), seed, opts, size, speed }
    }
}

/// How the simulation is shown. Changing it keeps the simulation running
/// and only resets per-view history (temporal smoothing, cell hysteresis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewKey {
    /// the full (wall) canvas in pixels this pane shows a window of
    pub canvas: (usize, usize),
    /// terminal cells
    pub grid: (usize, usize),
    /// canvas pixel at the pane's top-left cell
    pub crop: (usize, usize),
    pub pixels: Pixels,
    /// terminal cell height / width; 2.0 when the terminal does not say
    pub cell_aspect: f32,
    /// bumped by the caller for view changes the fields do not show
    pub rev: u64,
}

impl ViewKey {
    /// On-screen height / width of one canvas pixel.
    pub fn pixel_aspect(&self) -> f32 {
        let (pw, ph) = self.pixels.cell_size();
        self.cell_aspect * pw as f32 / ph as f32
    }
}

/// Cell aspect assumed when the terminal does not report its pixel size.
pub const DEFAULT_CELL_ASPECT: f32 = 2.0;

#[derive(Clone)]
pub struct Request {
    pub generation: u64,
    pub sim: SimKey,
    pub view: ViewKey,
    /// scene clock to render, ms since the anchor's t0
    pub elapsed_ms: u64,
    /// hold the previous clock (legacy; anchored panes freeze `elapsed_ms`)
    pub paused: bool,
    pub filters: Vec<String>,
    pub quick: Option<String>,
    pub hue: f32,
    pub saturation: f32,
    pub contrast: f32,
    pub dim: f32,
    pub smooth: f32,
    /// GPU milliseconds a Studio scene pass may take per frame.
    pub budget_ms: f32,
    /// The scene a pending switch will show: its shader starts compiling now.
    pub prefetch: Option<String>,
    /// Studio scenes: cell hysteresis threshold in levels (0 = off). Cells
    /// whose colours move by no more re-send last frame's values.
    pub hysteresis: u8,
}

/// Studio scenes render this many extra pixels around the pane's window when
/// a filter reads neighbours (blur, edges, warp...), so those filters see the
/// real scene at the pane's edges instead of clamping there — seamless
/// across the panes of a wall.
pub const APRON: usize = 8;

/// Post filters that read neighbouring pixels.
#[cfg(feature = "gpu")]
fn needs_apron(filters: &[String], quick: Option<&str>) -> bool {
    const NEIGHBOURS: &[&str] = &["bloom", "crt", "chroma", "pixelate", "edges", "warp", "sharpen"];
    filters.iter().map(String::as_str).chain(quick).any(|f| NEIGHBOURS.contains(&f))
}

pub struct Frame {
    pub generation: u64,
    pub canvas: Canvas,
    pub cells: Option<Vec<u32>>,
    /// terminal grid `cells` were packed for
    pub grid: (usize, usize),
    /// the scene clock this frame shows (the request it was rendered for,
    /// which for pipelined GPU readback is an earlier one than the request
    /// that returned it)
    pub elapsed_ms: u64,
    pub backend: String,
    pub render_ms: f32,
}

#[derive(Default)]
struct Mailbox {
    request: Option<Request>,
    frame: Option<Frame>,
    generation: u64,
    stopped: bool,
    /// Latest backend status when there is no new frame (shader compiling).
    status: Option<String>,
    /// While a rebuilt simulation replays: estimated ms until caught up.
    catchup_ms: Option<u32>,
}

pub struct Worker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    sim: Option<SimKey>,
    generation: u64,
}

impl Worker {
    pub fn new(renderer: Renderer) -> Self {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("termpaper-render".into())
            .spawn(move || run(thread_shared, renderer))
            .expect("render worker");
        Self {
            shared,
            sim: None,
            generation: 0,
        }
    }

    /// Hand the worker the latest request and take the latest finished
    /// frame of the current generation. Only a sim change starts a new
    /// generation; view changes (crop, grid, pixel aspect) and the clock
    /// never do.
    pub fn submit(&mut self, mut request: Request) -> Option<Frame> {
        if self.sim.as_ref() != Some(&request.sim) {
            self.generation = self.generation.wrapping_add(1);
            self.sim = Some(request.sim.clone());
        }
        request.generation = self.generation;
        let (lock, wake) = &*self.shared;
        let mut mailbox = lock.lock().unwrap();
        if mailbox.generation != self.generation {
            mailbox.catchup_ms = None;
        }
        mailbox.generation = self.generation;
        mailbox.request = Some(request);
        let frame = mailbox
            .frame
            .take()
            .filter(|f| f.generation == self.generation);
        wake.notify_one();
        frame
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// While the current generation replays its simulation up to the clock:
    /// the estimated ms until it catches up (None once it shows frames).
    pub fn catchup_ms(&self) -> Option<u32> {
        let mailbox = self.shared.0.lock().unwrap();
        mailbox.catchup_ms.filter(|_| mailbox.generation == self.generation)
    }

    /// Status line published while no frame is available (e.g. a Studio
    /// shader compiling), cleared once read.
    pub fn take_status(&mut self) -> Option<String> {
        self.shared.0.lock().unwrap().status.take()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        let mut mailbox = lock.lock().unwrap();
        mailbox.stopped = true;
        mailbox.request = None;
        wake.notify_one();
        // Detached worker owns its resources. Quitting never joins GPU work.
    }
}

/// Mix every value that decides a Studio frame's pixels into one hash, so a
/// request that would redraw the identical frame can reuse the last cells.
#[cfg(feature = "gpu")]
fn frame_hash(u: &crate::gpu::FrameUniforms, r: &Request) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(bytemuck::bytes_of(u));
    for f in &r.filters {
        eat(f.as_bytes());
        eat(&[0]);
    }
    if let Some(q) = &r.quick {
        eat(q.as_bytes());
    }
    for v in [r.hue, r.saturation, r.contrast, r.dim, r.smooth] {
        eat(&v.to_le_bytes());
    }
    let v = &r.view;
    for n in [v.grid.0, v.grid.1, v.crop.0, v.crop.1, v.canvas.0, v.canvas.1] {
        eat(&(n as u64).to_le_bytes());
    }
    eat(&[v.pixels as u8, r.hysteresis]);
    h
}

fn run(shared: Arc<(Mutex<Mailbox>, Condvar)>, renderer: Renderer) {
    #[cfg(feature = "gpu")]
    let mut gpu = if renderer != Renderer::Cpu {
        crate::gpu::Gpu::new(1, 1, 1)
    } else {
        None
    };
    #[cfg(feature = "gpu")]
    let mut adapter: Option<String> = gpu.as_ref().map(|g| g.adapter_name().to_string());
    #[cfg(not(feature = "gpu"))]
    let adapter: Option<String> = None;
    #[allow(unused_mut)]
    let mut errored = false;
    let mut generation = 0;
    let mut player: Option<Playback> = None;
    let mut raw = Canvas::new(1, 1);
    let mut previous = Canvas::new(1, 1);
    let mut last = Instant::now();
    let mut elapsed_ms = 0;
    // the view the per-view history (smoothing, hysteresis) belongs to
    let mut view: Option<ViewKey> = None;
    #[cfg(feature = "gpu")]
    let mut governor = crate::governor::Governor::new(DEFAULT_GPU_BUDGET_MS, 4);
    // Studio scenes whose shader failed to compile this session: shown via
    // their Classic fallback instead of retrying every frame
    #[allow(unused_mut)]
    let mut broken: Vec<&'static str> = Vec::new();
    #[cfg(feature = "gpu")]
    let mut last_hash: Option<u64> = None;
    loop {
        let mut request = {
            let (lock, wake) = &*shared;
            let mut mailbox = lock.lock().unwrap();
            while mailbox.request.is_none() && !mailbox.stopped {
                mailbox = wake.wait(mailbox).unwrap();
            }
            if mailbox.stopped {
                return;
            }
            mailbox.request.take().unwrap()
        };
        let start = Instant::now();
        let sim = &request.sim;
        let v = request.view;
        // scenes see the pixel mode they will be shown in (aspect/orientation)
        let opts = SceneOptions {
            pixels: v.pixels,
            ..sim.opts.clone()
        };
        if request.generation != generation {
            generation = request.generation;
            player = None;
            previous = Canvas::new(1, 1);
            elapsed_ms = request.elapsed_ms;
            view = None;
            #[cfg(feature = "gpu")]
            {
                last_hash = None;
                if let Some(g) = &mut gpu {
                    g.invalidate();
                }
                if let Some(spec) = crate::scene::shader::find(&sim.name) {
                    governor.reset(crate::scene::shader::max_spp(spec.cost, opts.detail));
                }
            }
        }
        if view != Some(v) {
            // same simulation, new window onto it: history from the old
            // window would ghost into the new one
            #[cfg(feature = "gpu")]
            if let Some(g) = &mut gpu {
                g.reset_history();
            }
            view = Some(v);
        }
        if request.paused {
            request.elapsed_ms = elapsed_ms;
        } else {
            elapsed_ms = request.elapsed_ms;
        }
        let sim = &request.sim;
        #[cfg(feature = "gpu")]
        if gpu.as_ref().is_some_and(|g| g.failed()) {
            gpu = None;
            adapter = None;
            errored = true;
        }
        let mut backend = plan_backend(renderer, adapter.is_some(), v.canvas, &sim.name);
        if let Backend::GpuShader(spec) = backend {
            if broken.contains(&spec.name) {
                backend = Backend::ShaderFallback(spec.fallback);
            }
        }
        #[allow(unused_mut)]
        let mut label = describe_backend(renderer, adapter.as_deref(), errored, backend);
        #[allow(unused_mut)]
        let mut cells = None;
        // the clock of the frame `cells` hold
        #[allow(unused_mut)]
        let mut shown_ms = request.elapsed_ms;

        #[cfg(feature = "gpu")]
        if let Some(name) = &request.prefetch {
            if let (Some(g), Some(spec)) = (gpu.as_mut(), crate::scene::shader::find(name)) {
                g.prefetch(spec);
            }
        }

        // Studio scene: the whole frame is GPU work.
        #[cfg(feature = "gpu")]
        if let (Backend::GpuShader(spec), Some(g)) = (backend, gpu.as_mut()) {
            use crate::gpu::ShaderStatus;
            match g.shader_status(spec) {
                ShaderStatus::Compiling => {
                    publish_status(&shared, generation, format!("{label} · compiling {}…", spec.name));
                    continue;
                }
                ShaderStatus::Failed(_) => {
                    broken.push(spec.name);
                    continue;
                }
                ShaderStatus::Ready => {}
            }
            let (pw, ph) = v.pixels.cell_size();
            // the pane's window: every pixel its cells show, starting at the
            // crop — rendered even where it overhangs the wall canvas, since
            // a Studio scene is defined everywhere — plus an apron all round
            // when a filter reads neighbours
            let apron = if needs_apron(&request.filters, request.quick.as_deref()) { APRON } else { 0 };
            let window = ((v.grid.0 * pw).max(1) + 2 * apron, (v.grid.1 * ph).max(1) + 2 * apron);
            let origin = (v.crop.0 as i64 - apron as i64, v.crop.1 as i64 - apron as i64);
            g.resize(window.0, window.1, v.grid.0 * v.grid.1);
            governor.set_budget(request.budget_ms);
            let mirror = request.filters.iter().any(|f| f == "mirror");
            let theme = opts
                .theme
                .as_deref()
                .and_then(|t| spec.themes.iter().position(|s| *s == t))
                .unwrap_or(0) as u32;
            let spp = governor.spp();
            let u = crate::gpu::uniforms(&crate::gpu::FrameDesc {
                view: crate::gpu::ShaderView::for_window(v.canvas, origin, v.pixel_aspect() as f64),
                window,
                time: crate::gpu::shader_time(request.elapsed_ms, sim.speed),
                speed: sim.speed,
                seed: sim.seed,
                theme,
                detail: opts.detail,
                spp,
                mirror,
                exposure: 0.0,
            });
            let hash = frame_hash(&u, &request);
            let post_filters: Vec<String> = request.filters.iter().filter(|f| *f != "mirror").cloned().collect();
            let smooth = request
                .smooth
                .clamp(0.0, 0.999)
                .powf(last.elapsed().as_secs_f32() * 60.0);
            last = Instant::now();
            if last_hash != Some(hash) {
                g.shader_frame(spec, u);
                let plan = crate::gpu::Plan {
                    filters: &post_filters,
                    quick_filter: request.quick.as_deref(),
                    t: request.elapsed_ms as f32 / 1000.0,
                    hue_shift: request.hue,
                    saturation: request.saturation,
                    contrast: request.contrast,
                    dim: request.dim,
                    smooth,
                    pixels: v.pixels,
                    cols: v.grid.0,
                    rows: v.grid.1,
                    // cells start past the apron
                    crop: (apron, apron),
                    // position-dependent filters work in wall coordinates
                    virt: Some([
                        origin.0 as i32,
                        origin.1 as i32,
                        v.canvas.0 as i32,
                        v.canvas.1 as i32,
                    ]),
                    hysteresis: request.hysteresis,
                };
                g.poll_cells(v.grid.0, v.grid.1);
                if g.submit_tagged(&raw, &plan, request.elapsed_ms) {
                    last_hash = Some(hash);
                } else {
                    governor.on_backpressure();
                }
            }
            if let Some(done) = g.poll_cells(v.grid.0, v.grid.1) {
                cells = Some(done.words.to_vec());
            }
            shown_ms = g.readback_tag();
            if let Some(ms) = g.take_scene_ms() {
                governor.observe(ms, spp);
                label = format!("{label} · {spp} spp · {ms:.1} ms");
            } else {
                label = format!("{label} · {spp} spp");
            }
            if cells.is_none() {
                continue;
            }
            let frame = Frame {
                generation,
                canvas: Canvas::new(1, 1),
                cells,
                grid: v.grid,
                elapsed_ms: shown_ms,
                backend: label,
                render_ms: start.elapsed().as_secs_f32() * 1000.0,
            };
            let mut mailbox = shared.0.lock().unwrap();
            if mailbox.generation == generation && !mailbox.stopped {
                mailbox.frame = Some(frame);
            }
            continue;
        }

        // Classic scene (or a Studio scene's fallback) on the CPU.
        let cpu_name: &str = match backend {
            Backend::ShaderFallback(fb) => fb,
            _ => &sim.name,
        };
        if matches!(backend, Backend::GpuShader(_)) {
            // GPU vanished between planning and drawing: next request replans
            continue;
        }
        let size = v.canvas;
        let gpu_world = backend == Backend::GpuWorld;
        let gpu_post = backend.gpu_post();
        if gpu_world {
            if (raw.width(), raw.height()) != size {
                raw.resize(size.0, size.1);
            }
        } else {
            let player = player.get_or_insert_with(|| Playback::new(cpu_name, &opts, sim.seed));
            let progress = player.advance_cancellable(
                cpu_name,
                &opts,
                size,
                request.elapsed_ms,
                sim.speed,
                request.paused,
                &mut raw,
                || {
                    let mailbox = shared.0.lock().unwrap();
                    mailbox.stopped || mailbox.generation != generation
                },
            );
            // a rebuilt simulation replays out of sight: no frame (the pane
            // stays black or mid-fade), only the estimate of how long
            publish_catchup(&shared, generation, (!progress.ready).then_some(progress.eta).flatten());
            if !progress.ready {
                continue;
            }
        }
        // Discard superseded work before encoding or applying filters.
        if shared.0.lock().unwrap().generation != generation {
            continue;
        }
        let t = request.elapsed_ms as f32 / 1000.0;
        let smooth = request
            .smooth
            .clamp(0.0, 0.999)
            .powf(last.elapsed().as_secs_f32() * 60.0);
        last = Instant::now();
        #[cfg(feature = "gpu")]
        if let Some(g) = gpu.as_mut().filter(|_| gpu_post) {
            g.resize(size.0, size.1, v.grid.0 * v.grid.1);
            if gpu_world {
                g.scene_frame(&sim.name, &opts, sim.seed, t, sim.speed);
            } else {
                g.canvas_frame();
            }
            let plan = crate::gpu::Plan {
                filters: &request.filters,
                quick_filter: request.quick.as_deref(),
                t,
                hue_shift: request.hue,
                saturation: request.saturation,
                contrast: request.contrast,
                dim: request.dim,
                smooth,
                pixels: v.pixels,
                cols: v.grid.0,
                rows: v.grid.1,
                crop: v.crop,
                // a Classic canvas is the whole wall: unchanged filters, and
                // no hysteresis (the CPU path it mirrors has none)
                virt: None,
                hysteresis: 0,
            };
            // Reclaim completed slots before trying to submit into the ring.
            g.poll_cells(v.grid.0, v.grid.1);
            g.submit_tagged(&raw, &plan, request.elapsed_ms);
            if let Some(done) = g.poll_cells(v.grid.0, v.grid.1) {
                cells = Some(done.words.to_vec());
            }
            shown_ms = g.readback_tag();
        }
        // Initial GPU frames are in flight. Do not publish a blank CPU canvas
        // over the last completed image while waiting for asynchronous readback.
        if gpu_post && cells.is_none() {
            continue;
        }
        let mut canvas = raw.clone_for_smooth();
        if cells.is_none() {
            filter::apply_all(&request.filters, &mut canvas, t);
            color_grade::apply(
                &mut canvas,
                request.hue,
                request.saturation,
                request.contrast,
            );
            if let Some(q) = &request.quick {
                filter::apply(q, &mut canvas, t);
            }
            crate::canvas::dim(&mut canvas, request.dim);
            if request.smooth > 0.001 {
                canvas.smooth_blend(&previous, 1.0 - smooth);
                canvas.snapshot_into(&mut previous);
            }
        }
        let frame = Frame {
            generation,
            canvas,
            cells,
            grid: v.grid,
            elapsed_ms: shown_ms,
            backend: label,
            render_ms: start.elapsed().as_secs_f32() * 1000.0,
        };
        let mut mailbox = shared.0.lock().unwrap();
        if mailbox.generation == generation && !mailbox.stopped {
            mailbox.frame = Some(frame);
        }
    }
}

/// Report (or clear, with None) how long the current generation's replay
/// still needs.
fn publish_catchup(shared: &Arc<(Mutex<Mailbox>, Condvar)>, generation: u64, eta: Option<std::time::Duration>) {
    let mut mailbox = shared.0.lock().unwrap();
    if mailbox.generation == generation {
        mailbox.catchup_ms = eta.map(|d| d.as_millis().min(u32::MAX as u128) as u32);
    }
}

/// While a shader compiles there is no frame to show; keep whatever is on
/// screen and only update the status line the menu displays.
#[cfg(feature = "gpu")]
fn publish_status(shared: &Arc<(Mutex<Mailbox>, Condvar)>, generation: u64, status: String) {
    let mut mailbox = shared.0.lock().unwrap();
    if mailbox.generation == generation {
        mailbox.status = Some(status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(name: &str) -> Request {
        let opts = SceneOptions { pixels: Pixels::Half, ..SceneOptions::default() };
        Request {
            generation: 0,
            sim: SimKey::new(name, 42, opts, (24, 24), 1.0),
            view: ViewKey {
                canvas: (24, 24),
                grid: (24, 12),
                crop: (0, 0),
                pixels: Pixels::Half,
                cell_aspect: DEFAULT_CELL_ASPECT,
                rev: 0,
            },
            elapsed_ms: 100,
            paused: false,
            filters: vec![],
            quick: None,
            hue: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            dim: 1.0,
            smooth: 0.0,
            budget_ms: DEFAULT_GPU_BUDGET_MS,
            prefetch: None,
            hysteresis: 0,
        }
    }

    #[test]
    fn classic_scenes_default_to_the_cpu_even_with_a_gpu() {
        // the Rust scenes are the artwork: a GPU only post-processes them
        for name in crate::scene::names() {
            let b = plan_backend(Renderer::Auto, true, (200, 100), name);
            let want = if crate::scene::gpu_world(name) { Backend::GpuWorld } else { Backend::GpuPost };
            assert_eq!(b, want, "{name}");
        }
        // shader mode opts every Classic scene into its WGSL arm, except bump's text
        assert_eq!(plan_backend(Renderer::Shader, true, (200, 100), "scroll"), Backend::GpuWorld);
        assert_eq!(plan_backend(Renderer::Shader, true, (200, 100), "bump"), Backend::GpuPost);
        // no device, tiny canvas, or an explicit cpu choice: everything on the CPU
        assert_eq!(plan_backend(Renderer::Auto, false, (200, 100), "scroll"), Backend::Cpu);
        assert_eq!(plan_backend(Renderer::Auto, true, (40, 12), "scroll"), Backend::Cpu);
        assert_eq!(plan_backend(Renderer::Cpu, true, (200, 100), "scroll"), Backend::Cpu);
    }

    #[test]
    fn studio_scenes_need_a_gpu_and_fall_back_without_one() {
        for spec in crate::scene::shader::SHADER_SCENES {
            // any size, any renderer except cpu: the GPU draws it
            for r in [Renderer::Auto, Renderer::Gpu, Renderer::Shader] {
                assert_eq!(plan_backend(r, true, (40, 12), spec.name), Backend::GpuShader(spec));
            }
            assert_eq!(plan_backend(Renderer::Auto, false, (200, 100), spec.name), Backend::ShaderFallback(spec.fallback));
            assert_eq!(plan_backend(Renderer::Cpu, true, (200, 100), spec.name), Backend::ShaderFallback(spec.fallback));
        }
    }

    #[test]
    fn backend_line_names_the_path() {
        assert_eq!(describe_backend(Renderer::Cpu, None, false, Backend::Cpu), "CPU · selected");
        assert_eq!(
            describe_backend(Renderer::Gpu, None, false, Backend::Cpu),
            "CPU · GPU requested but unavailable"
        );
        assert_eq!(describe_backend(Renderer::Auto, None, true, Backend::Cpu), "CPU · GPU error (fallback)");
        assert_eq!(describe_backend(Renderer::Auto, Some("Navi"), false, Backend::GpuPost), "GPU post · Navi · CPU scene");
        assert_eq!(describe_backend(Renderer::Shader, Some("Navi"), false, Backend::GpuWorld), "GPU world · Navi");
        assert_eq!(describe_backend(Renderer::Auto, Some("Navi"), false, Backend::Cpu), "CPU · small canvas");
        assert_eq!(
            describe_backend(Renderer::Cpu, Some("Navi"), false, Backend::ShaderFallback("ocean")),
            "CPU · CPU selected · showing Classic 'ocean'"
        );
    }

    #[test]
    fn mailbox_is_bounded_and_discards_superseded_frames() {
        // No worker consumes the queue: submissions must remain nonblocking
        // even when rendering is indefinitely busy.
        let mut worker = idle_worker();
        for ms in 0..1000 {
            let mut r = request("fire");
            r.elapsed_ms = ms;
            assert!(worker.submit(r).is_none());
        }
        assert_eq!(worker.generation(), 1);
        assert_eq!(
            worker
                .shared
                .0
                .lock()
                .unwrap()
                .request
                .as_ref()
                .unwrap()
                .elapsed_ms,
            999
        );
        worker.shared.0.lock().unwrap().frame = Some(Frame {
            generation: 1,
            canvas: Canvas::new(24, 24),
            cells: None,
            grid: (24, 12),
            elapsed_ms: 999,
            backend: "test".into(),
            render_ms: 0.0,
        });
        assert!(worker.submit(request("scroll")).is_none());
        assert_eq!(worker.generation(), 2);
        let mut faster = request("scroll");
        faster.sim.speed = 2.0;
        worker.submit(faster);
        assert_eq!(worker.generation(), 3);
    }

    fn idle_worker() -> Worker {
        Worker {
            shared: Arc::new((Mutex::new(Mailbox::default()), Condvar::new())),
            sim: None,
            generation: 0,
        }
    }

    /// Moving the window (wall crop, terminal resize, pixel aspect) or the
    /// clock keeps the simulation; changing what is simulated rebuilds it.
    #[test]
    fn view_changes_keep_the_generation_and_sim_changes_bump_it() {
        let mut worker = idle_worker();
        worker.submit(request("fire"));
        let gen = worker.generation();
        let views: [fn(&mut ViewKey); 5] = [
            |v| v.crop = (8, 4),
            |v| v.grid = (30, 10),
            |v| v.cell_aspect = 2.25,
            |v| v.rev += 1,
            |v| v.crop = (0, 0),
        ];
        for (i, change) in views.iter().enumerate() {
            let mut r = request("fire");
            r.elapsed_ms = 5_000 + i as u64;
            change(&mut r.view);
            worker.submit(r);
            assert_eq!(worker.generation(), gen, "view change {i} rebuilt the simulation");
        }
        // a frame of the current generation survives a view change
        worker.shared.0.lock().unwrap().frame = Some(Frame {
            generation: gen,
            canvas: Canvas::new(24, 24),
            cells: None,
            grid: (24, 12),
            elapsed_ms: 5_000,
            backend: "test".into(),
            render_ms: 0.0,
        });
        let mut moved = request("fire");
        moved.view.crop = (2, 2);
        assert!(worker.submit(moved).is_some(), "an old-view frame stays showable");
        let sims: [fn(&mut SimKey); 5] = [
            |s| s.seed = 43,
            |s| s.speed = 1.5,
            |s| s.size = (48, 24),
            |s| s.opts.detail = crate::scene::Detail::High,
            |s| s.name = "rain".into(),
        ];
        for (i, change) in sims.iter().enumerate() {
            let before = worker.generation();
            let mut r = request("fire");
            change(&mut r.sim);
            worker.submit(r);
            assert_eq!(worker.generation(), before + 1, "sim change {i} kept the old simulation");
            worker.submit(request("fire"));
        }
    }

    /// A Studio scene has no state: canvas size and pixel mode are view.
    #[test]
    fn studio_sim_keys_ignore_canvas_and_pixels() {
        let Some(spec) = crate::scene::shader::SHADER_SCENES.first() else { return };
        let half = SceneOptions { pixels: Pixels::Half, ..SceneOptions::default() };
        let quad = SceneOptions { pixels: Pixels::Quad, ..SceneOptions::default() };
        assert_eq!(
            SimKey::new(spec.name, 1, half.clone(), (100, 50), 1.0),
            SimKey::new(spec.name, 1, quad.clone(), (400, 90), 1.0)
        );
        assert_ne!(SimKey::new("fire", 1, half, (100, 50), 1.0), SimKey::new("fire", 1, quad, (100, 50), 1.0));
    }

    #[test]
    fn cpu_worker_returns_frames_and_survives_rapid_scene_changes() {
        let mut worker = Worker::new(Renderer::Cpu);
        for name in crate::scene::names() {
            worker.submit(request(name));
        }
        let start = Instant::now();
        loop {
            if let Some(frame) = worker.submit(request("fire")) {
                assert_eq!(frame.generation, worker.generation());
                assert_eq!((frame.canvas.width(), frame.canvas.height()), (24, 24));
                assert_eq!(frame.backend, "CPU · selected");
                break;
            }
            assert!(
                start.elapsed() < std::time::Duration::from_secs(5),
                "worker stalled"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
