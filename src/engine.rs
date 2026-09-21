//! A single rendering worker with a bounded, latest-request mailbox.
//! Terminal input and menus never wait for scene initialization or GPU work.
use crate::{
    canvas::Canvas, color_grade, filter, render::Pixels, scene::SceneOptions, sync::Playback,
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
    /// GPU post-processing and every scene drawn from its WGSL shader arm
    /// (the experimental GPU worlds) instead of the Rust scene.
    Shader,
}

/// Below this pixel count the GPU round trip costs more than the arithmetic
/// it saves; the worker stays on the CPU path. Roughly an 80x25 terminal in
/// half mode.
pub const MIN_GPU_PIXELS: usize = 4_000;

/// Decide how one request is rendered.
///
/// `post`: the GPU runs filters, grading, smoothing and cell packing.
/// `world`: the GPU also draws the scene from its WGSL arm instead of running
/// the Rust scene. Scenes are Rust artwork first — the WGSL worlds are opt-in
/// via `scene::GPU_WORLD_SCENES` or `--renderer shader`. `bump` always keeps
/// its CPU text layout.
pub fn plan_backend(renderer: Renderer, has_gpu: bool, size: (usize, usize), name: &str) -> (bool, bool) {
    let post = has_gpu && renderer != Renderer::Cpu && size.0 * size.1 >= MIN_GPU_PIXELS;
    let world = post
        && name != "bump"
        && (renderer == Renderer::Shader || crate::scene::gpu_world(name));
    (post, world)
}

/// Human-readable backend line for the settings menu.
fn describe_backend(
    renderer: Renderer,
    adapter: Option<&str>,
    errored: bool,
    post: bool,
    world: bool,
) -> String {
    match adapter {
        Some(a) if world => format!("GPU world · {a}"),
        Some(a) if post => format!("GPU post · {a} · CPU scene"),
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
            player = Some(Playback::new(&k.name, &opts, k.seed));
            previous = Canvas::new(1, 1);
            elapsed_ms = request.elapsed_ms;
            #[cfg(feature = "gpu")]
            if let Some(g) = &mut gpu {
                g.invalidate();
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
        let (gpu_post, gpu_world) = plan_backend(renderer, adapter.is_some(), k.size, &k.name);
        let backend = describe_backend(renderer, adapter.as_deref(), errored, gpu_post, gpu_world);
        if gpu_world {
            if (raw.width(), raw.height()) != k.size {
                raw.resize(k.size.0, k.size.1);
            }
        } else {
            player.as_mut().unwrap().advance_cancellable(
                &k.name,
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
        #[allow(unused_mut)]
        let mut cells = None;
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
            backend,
            render_ms: start.elapsed().as_secs_f32() * 1000.0,
        };
        let mut mailbox = shared.0.lock().unwrap();
        if mailbox.generation == generation && !mailbox.stopped {
            mailbox.frame = Some(frame);
        }
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
        }
    }

    #[test]
    fn scenes_default_to_the_cpu_even_with_a_gpu() {
        // the Rust scenes are the artwork: a GPU only post-processes them
        for name in crate::scene::names() {
            let (post, world) = plan_backend(Renderer::Auto, true, (200, 100), name);
            assert!(post, "{name}: GPU post-processing expected");
            assert_eq!(world, crate::scene::gpu_world(name), "{name}");
        }
        // shader mode opts every scene into its WGSL arm, except bump's text
        assert_eq!(plan_backend(Renderer::Shader, true, (200, 100), "scroll"), (true, true));
        assert_eq!(plan_backend(Renderer::Shader, true, (200, 100), "bump"), (true, false));
        // no device, tiny canvas, or an explicit cpu choice: everything on the CPU
        assert_eq!(plan_backend(Renderer::Auto, false, (200, 100), "scroll"), (false, false));
        assert_eq!(plan_backend(Renderer::Auto, true, (40, 12), "scroll"), (false, false));
        assert_eq!(plan_backend(Renderer::Cpu, true, (200, 100), "scroll"), (false, false));
    }

    #[test]
    fn backend_line_names_the_path() {
        assert_eq!(describe_backend(Renderer::Cpu, None, false, false, false), "CPU · selected");
        assert_eq!(
            describe_backend(Renderer::Gpu, None, false, false, false),
            "CPU · GPU requested but unavailable"
        );
        assert_eq!(describe_backend(Renderer::Auto, None, true, false, false), "CPU · GPU error (fallback)");
        assert_eq!(describe_backend(Renderer::Auto, Some("Navi"), false, true, false), "GPU post · Navi · CPU scene");
        assert_eq!(describe_backend(Renderer::Shader, Some("Navi"), false, true, true), "GPU world · Navi");
        assert_eq!(describe_backend(Renderer::Auto, Some("Navi"), false, false, false), "CPU · small canvas");
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
