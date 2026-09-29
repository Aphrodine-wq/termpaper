//! Studio shader scenes on the GPU.
//!
//! A scene pass is one compute dispatch over this pane's window that writes
//! packed sRGB pixels into the post chain's input buffer; filters, grading,
//! smoothing and cell packing then run exactly as for a CPU canvas. Scene
//! shaders compile on a background thread inside error scopes, so a broken or
//! slow-compiling scene never stalls the frame loop and never poisons the
//! device for the other scenes.
use std::collections::HashMap;
use std::sync::mpsc;

use super::Gpu;
use crate::scene::shader::{self, Composed, ShaderSpec};
use crate::scene::Detail;

/// Uniform block of `scene_entry.wgsl`. Every member is 16 bytes, so the
/// std140 and std430 layouts coincide.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniforms {
    pub map: [f32; 4],
    pub view: [f32; 4],
    pub time: [f32; 4],
    pub size: [u32; 4],
    pub ids: [u32; 4],
    pub qual: [f32; 4],
    pub params: [[f32; 4]; 2],
}
const _: () = assert!(std::mem::size_of::<FrameUniforms>() == 128);

/// How a pane's pixels map into composition space (y up, the frame's short
/// side spanning [-0.5, 0.5]). `origin` is the point at the top-left corner of
/// the window's first pixel; `step` is p per pixel along x and (downward) y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderView {
    pub origin: [f64; 2],
    pub step: [f64; 2],
    pub half: [f64; 2],
}

impl ShaderView {
    /// The view for a canvas of `full` pixels whose pixels are `aspect` times
    /// taller than wide, of which this pane renders the window whose top-left
    /// pixel is `win`. A single terminal is `win = (0, 0)` of its own canvas.
    pub fn for_canvas(full: (usize, usize), win: (usize, usize), aspect: f64) -> Self {
        Self::for_window(full, (win.0 as i64, win.1 as i64), aspect)
    }

    /// `for_canvas` for a window that may start outside the canvas (a crop
    /// widened by an apron): the scene is defined everywhere, so pixels left
    /// of or above the wall are simply further out in composition space.
    pub fn for_window(full: (usize, usize), win: (i64, i64), aspect: f64) -> Self {
        let w = full.0.max(1) as f64;
        let h = full.1.max(1) as f64 * aspect;
        let short = w.min(h);
        let step = [1.0 / short, aspect / short];
        let half = [w / (2.0 * short), h / (2.0 * short)];
        let origin = [-half[0] + win.0 as f64 * step[0], half[1] - win.1 as f64 * step[1]];
        Self { origin, step, half }
    }
}

/// Scene time from the shared clock. Continuous (ms resolution): a Studio
/// scene has no tick grid, so any frame rate animates evenly, and panes still
/// draw identical pixels because the frame clock renders every pane for the
/// same slot times. Wrapped hourly (f32 loses sub-frame precision after a few
/// hours), with the first seconds of each hour blended from the previous
/// cycle so nothing jumps. `tick` is the 60 Hz Classic tick of the moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderTime {
    pub t: f32,
    pub t_prev: f32,
    pub blend: f32,
    pub tick: u64,
}

pub const WRAP_SECS: f64 = 3600.0;
pub const WRAP_BLEND_SECS: f64 = 8.0;

pub fn shader_time(elapsed_ms: u64, speed: f32) -> ShaderTime {
    let tick = elapsed_ms.saturating_mul(60) / 1000;
    let t = elapsed_ms as f64 / 1000.0 * speed.max(0.0) as f64;
    let cycle = (t / WRAP_SECS).floor();
    let tw = t - cycle * WRAP_SECS;
    let blend = if cycle >= 1.0 && tw < WRAP_BLEND_SECS {
        let x = tw / WRAP_BLEND_SECS;
        (x * x * (3.0 - 2.0 * x)) as f32
    } else {
        1.0
    };
    ShaderTime {
        t: tw as f32,
        t_prev: (tw + WRAP_SECS) as f32,
        blend,
        tick,
    }
}

/// Everything that defines one shader frame.
#[derive(Clone, Copy, Debug)]
pub struct FrameDesc {
    pub view: ShaderView,
    pub window: (usize, usize),
    pub time: ShaderTime,
    pub speed: f32,
    pub seed: u64,
    pub theme: u32,
    pub detail: Detail,
    pub spp: u32,
    pub mirror: bool,
    pub exposure: f32,
}

pub fn uniforms(d: &FrameDesc) -> FrameUniforms {
    let v = d.view;
    // anti-aliasing width: geometric mean of the pixel's two sides
    let px = (v.step[0] * v.step[1]).sqrt();
    FrameUniforms {
        map: [v.origin[0] as f32, v.origin[1] as f32, v.step[0] as f32, v.step[1] as f32],
        view: [v.half[0] as f32, v.half[1] as f32, px as f32, (v.half[0] / v.half[1].max(1e-9)) as f32],
        time: [d.time.t, d.time.t_prev, d.time.blend, d.speed],
        size: [d.window.0 as u32, d.window.1 as u32, d.spp.max(1), d.mirror as u32],
        ids: [d.seed as u32, (d.seed >> 32) as u32, d.theme, shader::detail_index(d.detail)],
        qual: [shader::march_scale(d.detail), d.exposure, 0.0, 0.0],
        params: [[0.0; 4]; 2],
    }
}

/// Compile state of one scene.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Compiling,
    Ready,
    Failed(String),
}

enum Slot {
    Queued,
    Ready(wgpu::ComputePipeline),
    Failed(String),
}

struct Job {
    name: &'static str,
    composed: Composed,
}

pub(crate) struct ShaderScenes {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    uniform: wgpu::Buffer,
    /// Bind group for the current output buffer, and the buffer generation it
    /// was built for (the post chain reallocates its buffers on resize).
    bind: Option<(u64, wgpu::BindGroup)>,
    tx: mpsc::Sender<Job>,
    rx: mpsc::Receiver<(&'static str, Result<wgpu::ComputePipeline, String>)>,
    slots: HashMap<&'static str, Slot>,
    /// Scene sources come from this directory instead of the embedded copies
    /// (`TERMPAPER_SHADER_DIR`), re-read whenever a file changes.
    dev_dir: Option<std::path::PathBuf>,
    dev_stamp: HashMap<&'static str, std::time::SystemTime>,
    dev_checked: std::time::Instant,
}

fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("studio-scene"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<FrameUniforms>() as u64),
                },
                count: None,
            },
        ],
    })
}

/// Rewrite naga's `wgsl:LINE:COL` positions in terms of the scene files.
fn relocate(msg: &str, composed: &Composed) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(i) = rest.find("wgsl:") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 5..];
        let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
        match digits.parse::<usize>() {
            Ok(line) => {
                let (file, l) = composed.locate(line);
                out.push_str(&format!("{file}:{l}"));
                rest = &tail[digits.len()..];
            }
            Err(_) => {
                out.push_str("wgsl:");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Compile one composed scene inside error scopes. Must run on the thread
/// that pushed the scopes (they are thread-local).
fn compile(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    name: &str,
    composed: &Composed,
) -> Result<wgpu::ComputePipeline, String> {
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    // Scene loops are literally bounded (the test suite lints for it), so the
    // injected loop counters would only cost time in 100-step march loops.
    // Bounds checks stay on.
    let checks = wgpu::ShaderRuntimeChecks {
        force_loop_bounding: false,
        ..wgpu::ShaderRuntimeChecks::checked()
    };
    let module = unsafe {
        device.create_shader_module_trusted(
            wgpu::ShaderModuleDescriptor {
                label: Some(name),
                source: wgpu::ShaderSource::Wgsl(composed.source.as_str().into()),
            },
            checks,
        )
    };
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(name),
        layout: Some(layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions {
            constants: &[],
            zero_initialize_workgroup_memory: false,
        },
        cache: None,
    });
    let internal = pollster::block_on(internal.pop());
    let validation = pollster::block_on(validation.pop());
    match validation.or(internal) {
        Some(e) => Err(relocate(&e.to_string(), composed)),
        None => Ok(pipeline),
    }
}

/// Where compile errors go: stderr would scribble over the TUI.
fn log_error(name: &str, msg: &str) {
    let dir = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")))
        .map(|d| d.join("termpaper"));
    if let Some(dir) = dir {
        let _ = std::fs::create_dir_all(&dir);
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("shader-errors.log"))
        {
            let _ = writeln!(f, "--- {name}\n{msg}\n");
        }
    }
}

impl ShaderScenes {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = bind_group_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("studio-scene"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("studio-uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (tx, jobs) = mpsc::channel::<Job>();
        let (done, rx) = mpsc::channel();
        let dev = device.clone();
        let pl = pipeline_layout.clone();
        // wgpu documents create_shader_module as stack-hungry; big scenes nest
        std::thread::Builder::new()
            .name("termpaper-shaderc".into())
            .stack_size(16 << 20)
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let r = compile(&dev, &pl, job.name, &job.composed);
                    if let Err(e) = &r {
                        log_error(job.name, e);
                    }
                    if done.send((job.name, r)).is_err() {
                        break;
                    }
                }
            })
            .expect("shader compile thread");
        let dev_dir = std::env::var_os("TERMPAPER_SHADER_DIR").map(std::path::PathBuf::from);
        Self {
            layout,
            pipeline_layout,
            uniform,
            bind: None,
            tx,
            rx,
            slots: HashMap::new(),
            dev_dir,
            dev_stamp: HashMap::new(),
            dev_checked: std::time::Instant::now(),
        }
    }

    fn source(&mut self, spec: &'static ShaderSpec) -> Composed {
        if let Some(dir) = &self.dev_dir {
            match shader::compose_from_dir(spec.name, dir) {
                Ok((c, _)) => return c,
                Err(e) => log_error(spec.name, &e),
            }
        }
        shader::compose(spec)
    }

    fn drain(&mut self) {
        while let Ok((name, r)) = self.rx.try_recv() {
            let slot = match r {
                Ok(p) => Slot::Ready(p),
                Err(e) => Slot::Failed(e),
            };
            self.slots.insert(name, slot);
        }
    }

    /// Hot reload: with TERMPAPER_SHADER_DIR set, recompile a scene whose
    /// file changed. Checked at most twice a second.
    fn hot_reload(&mut self, spec: &'static ShaderSpec) {
        let Some(dir) = &self.dev_dir else { return };
        if self.dev_checked.elapsed() < std::time::Duration::from_millis(500) {
            return;
        }
        self.dev_checked = std::time::Instant::now();
        let newest = std::fs::read_dir(dir.join("lib"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok()?.metadata().ok()?.modified().ok())
            .chain(std::fs::metadata(dir.join(format!("{}.wgsl", spec.name))).and_then(|m| m.modified()))
            .max();
        if let Some(m) = newest {
            let prev = self.dev_stamp.insert(spec.name, m);
            if prev.is_some_and(|p| p != m) && !matches!(self.slots.get(spec.name), Some(Slot::Queued)) {
                let composed = self.source(spec);
                if self.tx.send(Job { name: spec.name, composed }).is_ok() {
                    // keep drawing the old pipeline until the new one lands
                    if !matches!(self.slots.get(spec.name), Some(Slot::Ready(_))) {
                        self.slots.insert(spec.name, Slot::Queued);
                    }
                }
            }
        }
    }

    /// Queue a compile if this scene has never been requested.
    pub(crate) fn request(&mut self, spec: &'static ShaderSpec) {
        self.drain();
        if !self.slots.contains_key(spec.name) {
            let composed = self.source(spec);
            if self.tx.send(Job { name: spec.name, composed }).is_ok() {
                self.slots.insert(spec.name, Slot::Queued);
            }
        }
    }

    pub(crate) fn status(&mut self, spec: &'static ShaderSpec) -> Status {
        self.request(spec);
        self.hot_reload(spec);
        self.drain();
        match self.slots.get(spec.name) {
            Some(Slot::Ready(_)) => Status::Ready,
            Some(Slot::Failed(e)) => Status::Failed(e.clone()),
            _ => Status::Compiling,
        }
    }

    pub(crate) fn pipeline(&self, name: &str) -> Option<&wgpu::ComputePipeline> {
        match self.slots.get(name) {
            Some(Slot::Ready(p)) => Some(p),
            _ => None,
        }
    }

    pub(crate) fn bind_group(&mut self, device: &wgpu::Device, out: &wgpu::Buffer, generation: u64) -> &wgpu::BindGroup {
        if self.bind.as_ref().is_none_or(|(g, _)| *g != generation) {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("studio-scene"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: out.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.uniform.as_entire_binding() },
                ],
            });
            self.bind = Some((generation, bg));
        }
        &self.bind.as_ref().unwrap().1
    }

    /// The bind group built by the last `bind_group` call.
    pub(crate) fn current_bind(&self) -> &wgpu::BindGroup {
        &self.bind.as_ref().expect("bind_group called first").1
    }

    pub(crate) fn write_uniforms(&self, queue: &wgpu::Queue, u: &FrameUniforms) {
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(u));
    }
}

/// Result of a blocking render (tooling and tests).
#[derive(Debug)]
pub struct Pixels {
    pub width: usize,
    pub height: usize,
    /// Packed `r | g<<8 | b<<16`.
    pub data: Vec<u32>,
    /// GPU time of the scene pass, when the adapter supports timestamps.
    pub gpu_ms: Option<f32>,
}

impl Gpu {
    fn shader_scenes(&mut self) -> &mut ShaderScenes {
        if self.shader.is_none() {
            self.shader = Some(ShaderScenes::new(&self.device));
        }
        self.shader.as_mut().unwrap()
    }

    /// Compile state of a Studio scene; queues the compile on first ask.
    pub fn shader_status(&mut self, spec: &'static ShaderSpec) -> Status {
        self.shader_scenes().status(spec)
    }

    /// Start compiling a scene that is about to be shown (during the fade).
    pub fn prefetch(&mut self, spec: &'static ShaderSpec) {
        self.shader_scenes().request(spec);
    }

    /// Draw the next submitted frame from this Studio scene instead of the
    /// uploaded canvas. The window size must match the last `resize`.
    pub fn shader_frame(&mut self, spec: &'static ShaderSpec, u: FrameUniforms) {
        self.world_frame = None;
        self.shader_frame = Some((spec, u));
    }

    /// GPU milliseconds of the most recently read-back scene pass.
    pub fn take_scene_ms(&mut self) -> Option<f32> {
        self.scene_ms.take()
    }

    /// Render a scene straight to pixels, blocking, outside the frame
    /// pipeline. `composed` overrides the embedded source (the review tool
    /// reads scenes from disk so authors iterate without rebuilding).
    pub fn render_shader_pixels(
        &mut self,
        name: &str,
        composed: &Composed,
        u: &FrameUniforms,
    ) -> Result<Pixels, String> {
        let (w, h) = (u.size[0] as usize, u.size[1] as usize);
        let device = self.device.clone();
        let layout = self.shader_scenes().pipeline_layout.clone();
        let pipeline = compile(&device, &layout, name, composed)?;
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("studio-review-out"),
            size: (w * h * 4).max(4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("studio-review-read"),
            size: (w * h * 4).max(4) as u64 + 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.shader_scenes();
        self.shader.as_ref().unwrap().write_uniforms(&self.queue, u);
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("studio-review"),
            layout: &self.shader.as_ref().unwrap().layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: out.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.shader.as_ref().unwrap().uniform.as_entire_binding(),
                },
            ],
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("studio-review") });
        {
            let tw = self.timestamps.as_ref().map(|(qs, _)| wgpu::ComputePassTimestampWrites {
                query_set: qs,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            });
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("studio-review"),
                timestamp_writes: tw,
            });
            cp.set_pipeline(&pipeline);
            cp.set_bind_group(0, &bg, &[]);
            cp.dispatch_workgroups((w as u32).div_ceil(8), (h as u32).div_ceil(8), 1);
        }
        enc.copy_buffer_to_buffer(&out, 0, &read, 0, (w * h * 4).max(4) as u64);
        if let Some((qs, resolve)) = &self.timestamps {
            enc.resolve_query_set(qs, 0..2, resolve, 0);
            enc.copy_buffer_to_buffer(resolve, 0, &read, (w * h * 4).max(4) as u64, 16);
        }
        self.queue.submit([enc.finish()]);
        let slice = read.slice(..);
        let (stx, srx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = stx.send(r.is_ok());
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        if !srx.recv().unwrap_or(false) {
            return Err("readback failed".into());
        }
        let view = slice.get_mapped_range().map_err(|e| format!("{e:?}"))?;
        let words: &[u32] = bytemuck::cast_slice(&view[..]);
        let data = words[..w * h].to_vec();
        let gpu_ms = self.timestamps.as_ref().map(|_| {
            let t0 = words[w * h] as u64 | (words[w * h + 1] as u64) << 32;
            let t1 = words[w * h + 2] as u64 | (words[w * h + 3] as u64) << 32;
            (t1.saturating_sub(t0) as f64 * self.queue.get_timestamp_period() as f64 / 1e6) as f32
        });
        drop(view);
        read.unmap();
        Ok(Pixels { width: w, height: h, data, gpu_ms })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_covers_the_short_side_with_unit_height() {
        // 200x100 square pixels: short side is height → y spans [-0.5, 0.5]
        let v = ShaderView::for_canvas((200, 100), (0, 0), 1.0);
        assert!((v.half[1] - 0.5).abs() < 1e-12);
        assert!((v.half[0] - 1.0).abs() < 1e-12);
        assert!((v.origin[0] + 1.0).abs() < 1e-12 && (v.origin[1] - 0.5).abs() < 1e-12);
        // portrait: width is the short side
        let p = ShaderView::for_canvas((100, 200), (0, 0), 1.0);
        assert!((p.half[0] - 0.5).abs() < 1e-12 && (p.half[1] - 1.0).abs() < 1e-12);
        // quad pixels are twice as tall: 200x100 px of 1x2 cells is 200x200
        let q = ShaderView::for_canvas((200, 100), (0, 0), 2.0);
        assert!((q.half[0] - 0.5).abs() < 1e-12 && (q.half[1] - 0.5).abs() < 1e-12);
        assert!((q.step[1] - 2.0 * q.step[0]).abs() < 1e-12);
    }

    #[test]
    fn an_apron_extends_the_same_mapping() {
        let crop = ShaderView::for_canvas((300, 120), (4, 2), 2.0);
        let apron = ShaderView::for_window((300, 120), (4 - 8, 2 - 8), 2.0);
        assert!((apron.origin[0] - (crop.origin[0] - 8.0 * crop.step[0])).abs() < 1e-12);
        assert!((apron.origin[1] - (crop.origin[1] + 8.0 * crop.step[1])).abs() < 1e-12);
        assert_eq!((apron.step, apron.half), (crop.step, crop.half));
    }

    #[test]
    fn crops_tile_the_same_mapping() {
        let full = ShaderView::for_canvas((300, 120), (0, 0), 1.0);
        let crop = ShaderView::for_canvas((300, 120), (150, 60), 1.0);
        // the crop's first pixel is the full view's pixel (150, 60)
        assert!((crop.origin[0] - (full.origin[0] + 150.0 * full.step[0])).abs() < 1e-12);
        assert!((crop.origin[1] - (full.origin[1] - 60.0 * full.step[1])).abs() < 1e-12);
    }

    #[test]
    fn time_is_continuous_and_wraps_with_a_blend() {
        let a = shader_time(1000, 1.0);
        let b = shader_time(1008, 1.0);
        assert!((b.t - a.t - 0.008).abs() < 1e-5, "no 60 Hz quantisation: any fps animates evenly");
        assert_eq!((a.tick, b.tick), (60, 60));
        assert_eq!(a.blend, 1.0);
        let wrap = shader_time(3_600_000 + 2000, 1.0);
        assert!(wrap.t < 3.0 && wrap.blend < 1.0 && wrap.blend > 0.0);
        assert!((wrap.t_prev - (wrap.t + 3600.0)).abs() < 1e-3);
        let settled = shader_time(3_600_000 + 9000, 1.0);
        assert_eq!(settled.blend, 1.0);
        // speed scales scene time
        assert!((shader_time(10_000, 2.0).t - 20.0).abs() < 1e-4);
    }
}
