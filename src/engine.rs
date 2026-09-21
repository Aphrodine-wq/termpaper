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
    #[default]
    Auto,
    Gpu,
    Cpu,
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
    let mut backend = gpu
        .as_ref()
        .map(|g| format!("GPU · {}", g.adapter_name()))
        .unwrap_or_else(|| {
            if renderer == Renderer::Cpu {
                "CPU · selected"
            } else {
                "CPU · GPU unavailable"
            }
            .into()
        });
    #[cfg(not(feature = "gpu"))]
    let backend = if renderer == Renderer::Cpu {
        "CPU · selected"
    } else {
        "CPU · GPU feature not compiled"
    }
    .to_string();
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
        if request.generation != generation {
            generation = request.generation;
            player = Some(Playback::new(&k.name, &k.opts, k.seed));
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
            backend = "CPU · GPU error (fallback)".into();
        }
        #[cfg(feature = "gpu")]
        let gpu_scene = gpu.is_some();
        #[cfg(not(feature = "gpu"))]
        let gpu_scene = false;
        if gpu_scene && k.name != "bump" {
            if (raw.width(), raw.height()) != k.size {
                raw.resize(k.size.0, k.size.1);
            }
        } else {
            player.as_mut().unwrap().advance_cancellable(
                &k.name,
                &k.opts,
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
        if let Some(g) = &mut gpu {
            g.resize(k.size.0, k.size.1, k.grid.0 * k.grid.1);
            if k.name == "bump" {
                g.canvas_frame();
            } else {
                g.scene_frame(&k.name, &k.opts, k.seed, t, request.speed);
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
        if gpu_scene && cells.is_none() {
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
            backend: backend.clone(),
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
