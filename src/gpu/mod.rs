//! GPU post-processing backend.
//!
//! The CPU pipeline stays the reference implementation and the default. This
//! module takes the part of a frame that is pure per-pixel arithmetic —
//! filters, colour grading, dimming, temporal smoothing, and the pixels →
//! terminal-cells packing step — and runs it as compute shaders instead.
//!
//! Why only that part: the benches in `examples/render_bench.rs` put a
//! 272x33-cell braille frame at 7.4ms, of which 6.8ms is that arithmetic. The
//! scene update stays on the CPU because 30-odd of the 47 scenes are stateful
//! particle systems, not fragment functions.
//!
//! Two things make this a win rather than a wash at these tiny resolutions:
//!
//! 1. **The readback is per-cell, not per-pixel.** `pack_cells` runs the
//!    luminance split and glyph selection on the GPU, so a braille frame reads
//!    back 12 bytes per terminal cell instead of 32 bytes of pixels.
//! 2. **The readback is pipelined.** Frame N is submitted while frame N-1's
//!    staging buffer is being read, so the CPU never fences on the GPU. The
//!    cost is one frame of display latency — 4ms at 240fps, on a wallpaper.
//!
//! Below `MIN_GPU_PIXELS` the round trip costs more than the arithmetic saves
//! and `should_use` says so; the caller stays on the CPU path.
//!
//! # What this actually buys, measured
//!
//! `examples/gpu_bench.rs` on Navi 22 times the moved stage in isolation:
//! pipelined, it is 1.9x faster at 18k px, 4.4x at 36k and 7.1x at 72k.
//!
//! End to end in the real binary the number is much smaller. A 250x45 braille
//! terminal at 240fps with bloom+vignette burns 5.48 CPU-seconds over a 7s
//! window on the CPU path and 4.18 on the GPU path — about 24% less, while
//! delivering slightly more frames.
//!
//! The gap between 7x and 24% is the point: once post-processing moves off the
//! CPU, the bottleneck becomes the terminal write. That run emits ~183MB of SGR
//! sequences in 8 seconds, and no amount of compute throughput touches it.
//! Anyone optimising this further should go after the escape-sequence
//! emission, not the pixel math — that is where the remaining frame time is.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::canvas::{Canvas, Cell};
use crate::render::Pixels;
mod world;

/// Below this pixel count the GPU path is not worth its setup cost.
///
/// The original guess here was ~24k px, on the theory that a round trip would
/// swamp the arithmetic at half-mode sizes. `examples/gpu_bench.rs` on Navi 22
/// says otherwise: pipelined, the GPU path beats the CPU 2.1x at 18k px, 4.3x
/// at 36k and 6.5x at 72k. The submit is cheap because the readback never
/// fences, so the only real floor is the per-dispatch overhead of a frame with
/// almost nothing in it.
///
/// 4k px is roughly an 80x25 terminal in half mode — below that the whole
/// frame is under 50us on the CPU and the GPU cannot save time that is not
/// being spent.
pub const MIN_GPU_PIXELS: usize = 4_000;

/// Uniform slots reserved per frame. Every scheduled pass consumes one, and
/// the worst realistic stack (a full filter list, grade, dim, smooth, pack) is
/// well inside this.
const MAX_PASSES: usize = 96;

/// Staging buffers in flight. Three is one more than the two the one-frame
/// latency strictly needs, so a late-returning map never stalls the encoder.
const SLOTS: usize = 3;

/// Cell readback stride, in u32: codepoint, packed fg, packed bg.
const CELL_WORDS: usize = 3;

/// `pack_cells` writes this codepoint when the cell carries a glyph the CPU
/// owns; fg/bg are still valid and only the character is substituted.
const GLYPH_CELL: u32 = 0xffff_ffff;

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    /// w, h, cell_w, cell_h
    dims: [u32; 4],
    /// cols, rows, crop_x, crop_y
    grid: [u32; 4],
    /// t, p0, p1, p2
    fp: [f32; 4],
    /// p3..p6 — reserved for filters that grow more knobs
    fp2: [f32; 4],
    /// pixel mode, then three filter-specific flags
    flags: [u32; 4],
}

/// Where a pass writes, which decides whether the ping-pong flips after it.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    /// Writes the bound `dst` buffer — flip src/dst after.
    Pong,
    /// Writes a scratch plane directly — the ping-pong is untouched.
    Aux,
    /// Writes the cell buffer, and dispatches over the cell grid.
    Cells,
}

struct Pass {
    pipeline: &'static str,
    params: Params,
    target: Target,
}

struct Slot {
    buf: wgpu::Buffer,
    ready: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    serial: usize,
    /// Cells the in-flight copy will contain, or None when the slot is idle.
    pending: Option<usize>,
}

/// A frame's worth of post-processing, described independently of the GPU so
/// the caller can build it without holding the device.
pub struct Plan<'a> {
    pub filters: &'a [String],
    pub quick_filter: Option<&'a str>,
    pub t: f32,
    pub hue_shift: f32,
    pub saturation: f32,
    pub contrast: f32,
    pub dim: f32,
    /// The engine's `settings.smooth`; the shader wants `1.0 - smooth`.
    pub smooth: f32,
    pub pixels: Pixels,
    pub cols: usize,
    pub rows: usize,
    pub crop: (usize, usize),
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: std::collections::HashMap<&'static str, wgpu::ComputePipeline>,
    layout: wgpu::BindGroupLayout,

    /// Ping-pong pair, each `w*h` u32.
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    /// Three `w*h` planes in one allocation: smoothing history, then the two
    /// bloom blur legs. Sharing a binding keeps the layout inside the
    /// downlevel limit of four storage buffers per stage.
    scratch: wgpu::Buffer,
    buf_cells: wgpu::Buffer,
    params_buf: wgpu::Buffer,
    /// [a→b, b→a]. Empty until the first `resize`, which is called from
    /// `new` — a bind group cannot be built before the buffers it points at.
    bind: Vec<wgpu::BindGroup>,

    slots: Vec<Slot>,
    frame: usize,
    params_stride: u32,

    dims: (usize, usize),
    cell_capacity: usize,
    /// True once the scratch history plane holds a frame at these dimensions.
    prev_valid: bool,

    /// Upload scratch, kept across frames so the packing does not allocate.
    upload: Vec<u32>,
    /// The most recent frame that finished its readback. Retained rather than
    /// consumed: if a poll finds nothing new, redisplaying this beats stalling
    /// the frame loop or falling back to a full CPU render, and at 120-240fps a
    /// single repeated frame of a wallpaper is not visible.
    readback: Vec<u32>,
    /// Cells `readback` describes, so a terminal resize invalidates it.
    readback_cells: usize,
    readback_serial: Option<usize>,
    failed: Arc<AtomicBool>,

    adapter_name: String,
    world: Option<world::World>,
    world_frame: Option<(String, crate::scene::SceneOptions, u64, f32, f32)>,
}

/// The finished cells of a frame, ready to be written into a ratatui buffer.
pub struct FrameCells<'a> {
    pub words: &'a [u32],
    pub cols: usize,
    pub rows: usize,
}

impl<'a> FrameCells<'a> {
    /// Codepoint, fg and bg for one cell. Codepoint is `GLYPH_CELL` when the
    /// character belongs to the canvas rather than the renderer.
    pub fn get(&self, i: usize) -> (u32, (u8, u8, u8), (u8, u8, u8)) {
        let w = &self.words[i * CELL_WORDS..i * CELL_WORDS + CELL_WORDS];
        let unpack = |v: u32| {
            (
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
            )
        };
        (w[0], unpack(w[1]), unpack(w[2]))
    }

    pub fn is_glyph(cp: u32) -> bool {
        cp == GLYPH_CELL
    }
}

impl Gpu {
    /// Bring up a compute-only device, or return `None` if there is no usable
    /// adapter. Every failure here is non-fatal: the caller keeps the CPU path.
    pub fn new(width: usize, height: usize, max_cells: usize) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: Default::default(),
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .ok()?;
        let adapter_name = adapter.get_info().name;

        // downlevel defaults, but with the storage-buffer binding count the
        // layout actually needs. Requesting more than the adapter offers is a
        // panic in wgpu, not an Err, so check before asking.
        let mut limits = wgpu::Limits::downlevel_defaults();
        limits.max_storage_buffers_per_shader_stage = 4;
        let have = adapter.limits();
        if have.max_storage_buffers_per_shader_stage < limits.max_storage_buffers_per_shader_stage
            || have.max_storage_buffer_binding_size < limits.max_storage_buffer_binding_size
            || have.max_compute_workgroup_size_x < 8
            || have.max_compute_workgroup_size_y < 8
        {
            return None;
        }

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("termpaper-post"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            ..Default::default()
        }))
        .ok()?;

        // Let the worker switch to CPU instead of repeatedly submitting to a
        // failed device or leaving the terminal permanently blank.
        let failed = Arc::new(AtomicBool::new(false));
        let error_flag = failed.clone();
        device.on_uncaptured_error(Arc::new(move |e| {
            error_flag.store(true, Ordering::Release);
            eprintln!("termpaper: gpu error: {e}");
        }));
        let lost_flag = failed.clone();
        device.set_device_lost_callback(move |_reason, _message| {
            lost_flag.store(true, Ordering::Release);
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/post.wgsl").into()),
        });

        let storage = |read_only: bool| wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post-layout"),
            entries: &[
                entry(0, storage(true)),
                entry(1, storage(false)),
                entry(
                    2,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        // one slot per pass, selected by dynamic offset — every
                        // pass is encoded into a single command buffer, so they
                        // cannot share a single mutable uniform
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Params>() as u64
                        ),
                    },
                ),
                entry(3, storage(false)),
                entry(4, storage(false)),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        const ENTRIES: &[&str] = &[
            "copy",
            "scanlines",
            "vignette",
            "grain",
            "warm",
            "cool",
            "hue",
            "duotone",
            "thermal",
            "invert",
            "sepia",
            "posterize",
            "gamma",
            "noir",
            "dim",
            "chroma",
            "pixelate",
            "edges",
            "warp",
            "sharpen",
            "mirror",
            "bloom_bright",
            "bloom_h",
            "bloom_v_add",
            "grade",
            "temporal_smooth",
            "pack_cells",
        ];
        let mut pipelines = std::collections::HashMap::new();
        for name in ENTRIES {
            let p = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            });
            pipelines.insert(*name, p);
        }

        let align = device.limits().min_uniform_buffer_offset_alignment;
        let params_stride = align.max(std::mem::size_of::<Params>() as u32);

        let mut gpu = Gpu {
            buf_a: Self::pixel_buffer(&device, 1, "a"),
            buf_b: Self::pixel_buffer(&device, 1, "b"),
            scratch: Self::pixel_buffer(&device, 3, "scratch"),
            buf_cells: Self::cell_buffer(&device, 1),
            params_buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("params"),
                size: params_stride as u64 * MAX_PASSES as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            bind: Vec::new(),
            slots: Vec::new(),
            frame: 0,
            params_stride,
            dims: (0, 0),
            cell_capacity: 0,
            prev_valid: false,
            upload: Vec::new(),
            readback: Vec::new(),
            readback_cells: 0,
            readback_serial: None,
            failed,
            device,
            queue,
            pipelines,
            layout,
            adapter_name,
            world: None,
            world_frame: None,
        };
        gpu.resize(width, height, max_cells);
        Some(gpu)
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    /// Invalidate old scene frames without waiting for outstanding commands.
    pub fn invalidate(&mut self) {
        let (w, h) = self.dims;
        self.dims = (0, 0);
        self.resize(w, h, self.cell_capacity);
    }

    pub fn scene_frame(&mut self, name: &str, opts: &crate::scene::SceneOptions, seed: u64, seconds: f32, speed: f32) {
        self.world_frame = Some((name.into(), opts.clone(), seed, seconds, speed));
    }

    /// Lightweight text scenes keep their authored CPU layout; the GPU still
    /// handles their filters, grading, smoothing and terminal packing.
    pub fn canvas_frame(&mut self) {
        self.world_frame = None;
    }

    fn pixel_buffer(device: &wgpu::Device, px: usize, label: &str) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (px.max(1) * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn cell_buffer(device: &wgpu::Device, cells: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cells"),
            size: (cells.max(1) * CELL_WORDS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Reallocate for a new canvas size. Drops any in-flight readback, because
    /// its cell count no longer describes the terminal.
    pub fn resize(&mut self, width: usize, height: usize, max_cells: usize) {
        if self.dims == (width, height) && self.cell_capacity >= max_cells {
            return;
        }
        let px = width * height;
        self.buf_a = Self::pixel_buffer(&self.device, px, "a");
        self.buf_b = Self::pixel_buffer(&self.device, px, "b");
        self.scratch = Self::pixel_buffer(&self.device, px * 3, "scratch");
        self.buf_cells = Self::cell_buffer(&self.device, max_cells);

        let mk = |src: &wgpu::Buffer, dst: &wgpu::Buffer, this: &Gpu| {
            this.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post-bind"),
                layout: &this.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: src.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: dst.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &this.params_buf,
                            offset: 0,
                            size: wgpu::BufferSize::new(std::mem::size_of::<Params>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: this.scratch.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: this.buf_cells.as_entire_binding(),
                    },
                ],
            })
        };
        self.bind = vec![
            mk(&self.buf_a, &self.buf_b, self),
            mk(&self.buf_b, &self.buf_a, self),
        ];

        let bytes = (max_cells.max(1) * CELL_WORDS * 4) as u64;
        self.slots = (0..SLOTS)
            .map(|i| Slot {
                buf: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("staging{i}")),
                    size: bytes,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                ready: Arc::new(AtomicBool::new(false)),
                failed: Arc::new(AtomicBool::new(false)),
                serial: 0,
                pending: None,
            })
            .collect();

        self.dims = (width, height);
        self.cell_capacity = max_cells;
        self.prev_valid = false;
        self.frame = 0;
        self.upload.resize(px, 0);
        self.readback.clear();
        self.readback_cells = 0;
        self.readback_serial = None;
        if let Some(world) = &mut self.world {
            world.reset(&self.device, (width, height));
        }
    }

    /// Whether the GPU path is expected to beat the CPU at this size.
    pub fn should_use(width: usize, height: usize) -> bool {
        width * height >= MIN_GPU_PIXELS
    }

    /// Build the ordered pass list for a plan. Split out from `submit` so the
    /// parity test can assert on scheduling without a device.
    fn schedule(&self, plan: &Plan) -> Vec<Pass> {
        let (w, h) = self.dims;
        let base = Params {
            dims: [
                w as u32,
                h as u32,
                plan.pixels.cell_size().0 as u32,
                plan.pixels.cell_size().1 as u32,
            ],
            grid: [
                plan.cols as u32,
                plan.rows as u32,
                plan.crop.0 as u32,
                plan.crop.1 as u32,
            ],
            fp: [plan.t, 0.0, 0.0, 0.0],
            fp2: [0.0; 4],
            flags: [
                match plan.pixels {
                    Pixels::Half => 0,
                    Pixels::Quad => 1,
                    Pixels::Braille => 2,
                },
                0,
                0,
                0,
            ],
        };

        let mut passes = Vec::new();
        let mut push = |pipeline: &'static str, params: Params, target: Target| {
            passes.push(Pass {
                pipeline,
                params,
                target,
            })
        };

        let filter_pass = |name: &str, push: &mut dyn FnMut(&'static str, Params, Target)| {
            let mut p = base;
            match name {
                "scanlines" => push("scanlines", p, Target::Pong),
                "vignette" => push("vignette", p, Target::Pong),
                "grain" => push("grain", p, Target::Pong),
                "warm" => push("warm", p, Target::Pong),
                "cool" => push("cool", p, Target::Pong),
                "hue" => {
                    p.fp[1] = 120.0 / 360.0;
                    push("hue", p, Target::Pong);
                }
                "spectrum" => {
                    // matches `filter::spectrum`: 30 deg/s, wrapped
                    p.fp[1] = (plan.t * 30.0).rem_euclid(360.0) / 360.0;
                    push("hue", p, Target::Pong);
                }
                "crt" => {
                    push("chroma", p, Target::Pong);
                    push("scanlines", p, Target::Pong);
                    push("vignette", p, Target::Pong);
                }
                "bloom" => {
                    push("bloom_bright", p, Target::Aux);
                    push("bloom_h", p, Target::Aux);
                    push("bloom_v_add", p, Target::Pong);
                }
                "duotone" => push("duotone", p, Target::Pong),
                "pixelate" => push("pixelate", p, Target::Pong),
                "chroma" => push("chroma", p, Target::Pong),
                "edges" => push("edges", p, Target::Pong),
                "thermal" => push("thermal", p, Target::Pong),
                "warp" => push("warp", p, Target::Pong),
                "invert" => push("invert", p, Target::Pong),
                "sepia" => push("sepia", p, Target::Pong),
                "posterize" => push("posterize", p, Target::Pong),
                "gamma" => push("gamma", p, Target::Pong),
                "sharpen" => push("sharpen", p, Target::Pong),
                "mirror" => push("mirror", p, Target::Pong),
                "noir" => push("noir", p, Target::Pong),
                // unknown names are ignored on the CPU too
                _ => {}
            }
        };

        for name in plan.filters {
            filter_pass(name, &mut push);
        }

        // colour grade — same early-out as `color_grade::apply`
        let hue_on = plan.hue_shift >= 0.5;
        let sat_on = (plan.saturation - 1.0).abs() > 0.02;
        let con_on = (plan.contrast - 1.0).abs() > 0.02;
        if hue_on || sat_on || con_on {
            let mut p = base;
            p.fp[1] = plan.hue_shift / 360.0;
            p.fp[2] = plan.saturation;
            p.fp[3] = plan.contrast;
            p.flags[1] = hue_on as u32;
            p.flags[2] = sat_on as u32;
            p.flags[3] = con_on as u32;
            push("grade", p, Target::Pong);
        }

        if let Some(q) = plan.quick_filter {
            filter_pass(q, &mut push);
        }

        if plan.dim < 0.999 {
            let mut p = base;
            p.fp[1] = plan.dim;
            push("dim", p, Target::Pong);
        }

        if plan.smooth > 0.001 {
            let mut p = base;
            p.fp[1] = 1.0 - plan.smooth;
            push("temporal_smooth", p, Target::Pong);
        }

        push("pack_cells", base, Target::Cells);
        passes
    }

    /// Encode and submit one frame. Returns false if the plan needs more
    /// uniform slots than are reserved, in which case nothing is submitted and
    /// the caller should fall back to the CPU for this frame.
    pub fn submit(&mut self, canvas: &Canvas, plan: &Plan) -> bool {
        if self.failed() { return false; }
        let (w, h) = self.dims;
        debug_assert_eq!((canvas.width(), canvas.height()), (w, h));
        let cells = plan.cols * plan.rows;
        if cells == 0 || cells > self.cell_capacity {
            return false;
        }
        let _ = self.device.poll(wgpu::PollType::Poll);
        let slot_idx = self.frame % SLOTS;
        if self.slots[slot_idx].pending.is_some() {
            // A full ring is backpressure, never a reason to wait for the GPU.
            return false;
        }

        let passes = self.schedule(plan);
        if passes.len() > MAX_PASSES {
            return false;
        }

        // Pack the canvas. The glyph flag rides in the top byte so the shaders
        // know which cells hold text without needing the characters.
        if let Some((name, opts, seed, seconds, speed)) = &self.world_frame {
            let world = self.world.get_or_insert_with(|| world::World::new(&self.device, self.dims));
            world.render(&self.device, &self.queue, &self.buf_a, self.dims,
                name, opts, *seed, *seconds, *speed);
            if self.failed() { return false; }
        } else {
            for (dst, cell) in self.upload.iter_mut().zip(canvas.cells_raw()) {
                let (r, g, b) = cell.color;
                let fl = u32::from(cell.ch.is_some());
                *dst = r as u32 | (g as u32) << 8 | (b as u32) << 16 | fl << 24;
            }
            self.queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&self.upload));
        }

        // Smoothing reads history; on the first frame at a new size there is
        // none, so seed it with the current frame — same as the CPU path,
        // which starts from a black `prev_canvas` and converges. Seeding with
        // the current frame instead avoids a one-frame fade-in on resize.
        if plan.smooth > 0.001 && !self.prev_valid {
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("seed") });
            enc.copy_buffer_to_buffer(&self.buf_a, 0, &self.scratch, 0, (w * h * 4) as u64);
            self.queue.submit([enc.finish()]);
            self.prev_valid = true;
        }

        // One uniform write for every pass, then dynamic offsets select them.
        let stride = self.params_stride as usize;
        let mut raw = vec![0u8; stride * passes.len()];
        for (i, pass) in passes.iter().enumerate() {
            let bytes = bytemuck::bytes_of(&pass.params);
            raw[i * stride..i * stride + bytes.len()].copy_from_slice(bytes);
        }
        self.queue.write_buffer(&self.params_buf, 0, &raw);

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("post") });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("post"),
                timestamp_writes: None,
            });
            let mut src_is_a = true;
            for (i, pass) in passes.iter().enumerate() {
                let Some(pipeline) = self.pipelines.get(pass.pipeline) else {
                    continue;
                };
                cp.set_pipeline(pipeline);
                let bg = if src_is_a { 0 } else { 1 };
                cp.set_bind_group(0, &self.bind[bg], &[(i * stride) as u32]);
                let (gx, gy) = match pass.target {
                    Target::Cells => (plan.cols, plan.rows),
                    _ => (w, h),
                };
                cp.dispatch_workgroups(gx.div_ceil(8) as u32, gy.div_ceil(8) as u32, 1);
                if pass.target == Target::Pong {
                    src_is_a = !src_is_a;
                }
            }
        }
        enc.copy_buffer_to_buffer(
            &self.buf_cells,
            0,
            &self.slots[slot_idx].buf,
            0,
            (cells * CELL_WORDS * 4) as u64,
        );
        self.queue.submit([enc.finish()]);

        let ready = self.slots[slot_idx].ready.clone();
        ready.store(false, Ordering::Release);
        let cb_ready = ready.clone();
        let failed = self.slots[slot_idx].failed.clone();
        failed.store(false, Ordering::Release);
        self.slots[slot_idx]
            .buf
            .slice(..(cells * CELL_WORDS * 4) as u64)
            .map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    cb_ready.store(true, Ordering::Release);
                } else {
                    failed.store(true, Ordering::Release);
                }
            });
        self.slots[slot_idx].pending = Some(cells);
        self.slots[slot_idx].serial = self.frame;
        self.frame += 1;
        true
    }

    fn reclaim(&mut self, i: usize) {
        if self.slots[i].pending.take().is_some() {
            self.slots[i].buf.unmap();
            self.slots[i].ready.store(false, Ordering::Release);
            self.slots[i].failed.store(false, Ordering::Release);
        }
    }

    /// Non-blocking: drive map callbacks, then hand back the oldest completed
    /// frame if one is ready. Call once per frame after `submit`.
    ///
    /// Returns `None` on the first frame after a resize — there is nothing in
    /// flight yet — and the caller draws that one on the CPU.
    pub fn poll_cells(&mut self, cols: usize, rows: usize) -> Option<FrameCells<'_>> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let mut ready: Vec<_> = (0..self.slots.len())
            .filter(|&i| self.slots[i].pending.is_some()
                && (self.slots[i].ready.load(Ordering::Acquire)
                    || self.slots[i].failed.load(Ordering::Acquire)))
            .collect();
        ready.sort_by_key(|&i| self.slots[i].serial);
        for i in ready {
            let n = self.slots[i].pending.unwrap();
            if self.slots[i].failed.load(Ordering::Acquire) {
                self.failed.store(true, Ordering::Release);
            }
            if n == cols * rows && !self.slots[i].failed.load(Ordering::Acquire)
                && self.readback_serial.is_none_or(|serial| self.slots[i].serial > serial) {
                if let Ok(view) = self.slots[i].buf.slice(..(n * CELL_WORDS * 4) as u64).get_mapped_range() {
                    self.readback.clear();
                    self.readback.extend_from_slice(bytemuck::cast_slice(&view[..]));
                    self.readback_cells = n;
                    self.readback_serial = Some(self.slots[i].serial);
                }
            }
            self.reclaim(i);
        }
        if self.readback_cells != cols * rows || self.readback.is_empty() {
            return None;
        }
        Some(FrameCells {
            words: &self.readback,
            cols,
            rows,
        })
    }

    /// Block until every in-flight frame has landed. Used by the benches and
    /// by shutdown; the frame loop never calls this.
    pub fn drain(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        for i in 0..self.slots.len() {
            self.reclaim(i);
        }
        self.frame = 0;
        self.readback_serial = None;
    }

    /// Run one plan start to finish and return the finished cells, blocking on
    /// the GPU. This is the un-pipelined path: correctness tests and the bench
    /// use it, the frame loop does not.
    pub fn run_blocking(&mut self, canvas: &Canvas, plan: &Plan) -> Option<Vec<u32>> {
        if !self.submit(canvas, plan) {
            return None;
        }
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let i = (self.frame - 1) % SLOTS;
        let n = self.slots[i].pending?;
        let out = {
            let view = self.slots[i]
                .buf
                .slice(..(n * CELL_WORDS * 4) as u64)
                .get_mapped_range();
            bytemuck::cast_slice::<u8, u32>(&view.ok()?[..]).to_vec()
        };
        self.reclaim(i);
        Some(out)
    }
}

/// Write a finished GPU frame into a ratatui buffer.
///
/// `canvas` is only consulted for glyph characters: `pack_cells` already
/// resolved every colour and every block/braille codepoint, and marked the
/// cells whose character the CPU owns.
pub fn blit(
    cells: &FrameCells,
    canvas: &Canvas,
    crop: (usize, usize),
    pixels: Pixels,
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    truecolor: bool,
) {
    use crate::canvas::rgb_to_256;
    let (pw, ph) = pixels.cell_size();
    let to_color = |c: (u8, u8, u8)| {
        if truecolor {
            ratatui::style::Color::Rgb(c.0, c.1, c.2)
        } else {
            ratatui::style::Color::Indexed(rgb_to_256(c.0, c.1, c.2))
        }
    };
    let raw: &[Cell] = canvas.cells_raw();
    let cw = canvas.width();
    for row in 0..area.height.min(cells.rows as u16) {
        for col in 0..area.width.min(cells.cols as u16) {
            let (cp, fg, bg) = cells.get(row as usize * cells.cols + col as usize);
            let out = &mut buf[(area.x + col, area.y + row)];
            let ch = if FrameCells::is_glyph(cp) {
                // the GPU flagged it; the character itself lives on the CPU
                let x = crop.0 + col as usize * pw;
                let y = crop.1 + row as usize * ph;
                raw.get(y * cw + x).and_then(|c| c.ch).unwrap_or(' ')
            } else {
                char::from_u32(cp).unwrap_or(' ')
            };
            out.set_char(ch);
            out.set_fg(to_color(fg));
            out.set_bg(to_color(bg));
        }
    }
}
