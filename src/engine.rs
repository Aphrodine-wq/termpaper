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

#[derive(Clone, PartialEq)]
pub struct SceneKey {
    pub name: String,
    pub seed: u64,
    pub opts: SceneOptions,
    pub size: (usize, usize),
    pub grid: (usize, usize),
    pub crop: (usize, usize),
    pub pixels: Pixels,
}

#[derive(Clone)]
pub struct Request {
    pub generation: u64,
    pub key: SceneKey,
    pub elapsed_ms: u64,
    pub speed: f32,
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
}

pub struct Frame {
    pub generation: u64,
    pub canvas: Canvas,
    pub cells: Option<Vec<u32>>,
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
}

pub struct Worker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    key: Option<SceneKey>,
    generation: u64,
    speed: f32,
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
            key: None,
            generation: 0,
            speed: 1.0,
        }
    }

    pub fn submit(&mut self, mut request: Request) -> Option<Frame> {
        let changed = self.key.as_ref() != Some(&request.key) || self.speed != request.speed;
        if changed {
            self.generation = self.generation.wrapping_add(1);
            self.key = Some(request.key.clone());
            self.speed = request.speed;
        }
        request.generation = self.generation;
        let (lock, wake) = &*self.shared;
        let mut mailbox = lock.lock().unwrap();
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
/// request that would redraw the identical tick can reuse the last cells.
#[cfg(feature = "gpu")]
fn frame_hash(u: &crate::gpu::FrameUniforms, r: &Request, k: &SceneKey) -> u64 {
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
    for v in [k.grid.0, k.grid.1, k.crop.0, k.crop.1] {
        eat(&(v as u64).to_le_bytes());
    }
    eat(&[k.pixels as u8]);
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
        let k = &request.key;
        // scenes see the pixel mode they will be shown in (aspect/orientation)
        let opts = SceneOptions {
            pixels: k.pixels,
            ..k.opts.clone()
        };
        if request.generation != generation {
            generation = request.generation;
            player = None;
            previous = Canvas::new(1, 1);
            elapsed_ms = request.elapsed_ms;
            #[cfg(feature = "gpu")]
            {
                last_hash = None;
                if let Some(g) = &mut gpu {
                    g.invalidate();
                }
                if let Some(spec) = crate::scene::shader::find(&k.name) {
                    governor.reset(crate::scene::shader::max_spp(spec.cost, opts.detail));
                }
            }
        }
        if request.paused {
            request.elapsed_ms = elapsed_ms;
        } else {
            elapsed_ms = request.elapsed_ms;
        }
        #[cfg(feature = "gpu")]
        if gpu.as_ref().is_some_and(|g| g.failed()) {
            gpu = None;
            adapter = None;
            errored = true;
        }
        let mut backend = plan_backend(renderer, adapter.is_some(), k.size, &k.name);
        if let Backend::GpuShader(spec) = backend {
            if broken.contains(&spec.name) {
                backend = Backend::ShaderFallback(spec.fallback);
            }
        }
        #[allow(unused_mut)]
        let mut label = describe_backend(renderer, adapter.as_deref(), errored, backend);
        #[allow(unused_mut)]
        let mut cells = None;

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
            let (pw, ph) = k.pixels.cell_size();
            // the pane's window: every pixel its cells show, starting at the
            // crop — rendered even where it overhangs the wall canvas, since
            // a Studio scene is defined everywhere
            let window = ((k.grid.0 * pw).max(1), (k.grid.1 * ph).max(1));
            g.resize(window.0, window.1, k.grid.0 * k.grid.1);
            governor.set_budget(request.budget_ms);
            let mirror = request.filters.iter().any(|f| f == "mirror");
            let theme = opts
                .theme
                .as_deref()
                .and_then(|t| spec.themes.iter().position(|s| *s == t))
                .unwrap_or(0) as u32;
            let spp = governor.spp();
            let u = crate::gpu::uniforms(&crate::gpu::FrameDesc {
                view: crate::gpu::ShaderView::for_canvas(k.size, k.crop, k.pixels.aspect() as f64),
                window,
                time: crate::gpu::shader_time(request.elapsed_ms, request.speed),
                speed: request.speed,
                seed: k.seed,
                theme,
                detail: opts.detail,
                spp,
                mirror,
                exposure: 0.0,
            });
            let hash = frame_hash(&u, &request, k);
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
                    pixels: k.pixels,
                    cols: k.grid.0,
                    rows: k.grid.1,
                    crop: (0, 0),
                };
                g.poll_cells(k.grid.0, k.grid.1);
                if g.submit(&raw, &plan) {
                    last_hash = Some(hash);
                } else {
                    governor.on_backpressure();
                }
            }
            if let Some(done) = g.poll_cells(k.grid.0, k.grid.1) {
                cells = Some(done.words.to_vec());
            }
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
            _ => &k.name,
        };
        if matches!(backend, Backend::GpuShader(_)) {
            // GPU vanished between planning and drawing: next request replans
            continue;
        }
        let gpu_world = backend == Backend::GpuWorld;
        let gpu_post = backend.gpu_post();
        if gpu_world {
            if (raw.width(), raw.height()) != k.size {
                raw.resize(k.size.0, k.size.1);
            }
        } else {
            let player = player.get_or_insert_with(|| Playback::new(cpu_name, &opts, k.seed));
            player.advance_cancellable(
                cpu_name,
                &opts,
                k.size,
                request.elapsed_ms,
                request.speed,
                request.paused,
                &mut raw,
                || {
                    let mailbox = shared.0.lock().unwrap();
                    mailbox.stopped || mailbox.generation != generation
                },
            );
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
            g.resize(k.size.0, k.size.1, k.grid.0 * k.grid.1);
            if gpu_world {
                g.scene_frame(&k.name, &opts, k.seed, t, request.speed);
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
                pixels: k.pixels,
                cols: k.grid.0,
                rows: k.grid.1,
                crop: k.crop,
            };
            // Reclaim completed slots before trying to submit into the ring.
            g.poll_cells(k.grid.0, k.grid.1);
            g.submit(&raw, &plan);
            if let Some(done) = g.poll_cells(k.grid.0, k.grid.1) {
                cells = Some(done.words.to_vec());
            }
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
            backend: label,
            render_ms: start.elapsed().as_secs_f32() * 1000.0,
        };
        let mut mailbox = shared.0.lock().unwrap();
        if mailbox.generation == generation && !mailbox.stopped {
            mailbox.frame = Some(frame);
        }
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
        Request {
            generation: 0,
            key: SceneKey {
                name: name.into(),
                seed: 42,
                opts: SceneOptions::default(),
                size: (24, 24),
                grid: (24, 12),
                crop: (0, 0),
                pixels: Pixels::Half,
            },
            elapsed_ms: 100,
            speed: 1.0,
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
        let mut worker = Worker {
            shared: Arc::new((Mutex::new(Mailbox::default()), Condvar::new())),
            key: None,
            generation: 0,
            speed: 1.0,
        };
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
            backend: "test".into(),
            render_ms: 0.0,
        });
        assert!(worker.submit(request("scroll")).is_none());
        assert_eq!(worker.generation(), 2);
        let mut faster = request("scroll");
        faster.speed = 2.0;
        worker.submit(faster);
        assert_eq!(worker.generation(), 3);
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
