//! Scene rendering into the post pipeline's resident pixel buffer.
use crate::scene::{self, SceneOptions};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    dims: [u32; 4],
    ids: [u32; 4],
    time: [f32; 4],
    primary: [f32; 4],
    secondary: [f32; 4],
    background: [f32; 4],
}

pub struct World {
    layout: wgpu::BindGroupLayout,
    draw: std::collections::HashMap<u32, wgpu::ComputePipeline>,
    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    init: wgpu::ComputePipeline,
    step: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    state: [wgpu::Buffer; 2],
    dims: (u32, u32),
    tick: u64,
    front: usize,
    initialized: bool,
}

fn color(v: [u8; 3]) -> [f32; 4] {
    [
        v[0] as f32 / 255.0,
        v[1] as f32 / 255.0,
        v[2] as f32 / 255.0,
        1.0,
    ]
}

fn palette(name: &str, theme: &str) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let (mut a, mut b, mut bg): ([u8; 3], [u8; 3], [u8; 3]) = match name {
        "fire" | "lava" | "incense" | "lanterns" => ([255, 145, 50], [255, 219, 145], [12, 7, 12]),
        "ocean" | "ripple" | "koi" | "aquarium" | "abyss" => {
            ([45, 151, 157], [167, 218, 216], [4, 19, 29])
        }
        "canopy" | "meadow" | "fireflies" => ([95, 146, 76], [220, 220, 141], [9, 20, 22]),
        "scroll" => ([49, 56, 49], [130, 132, 115], [227, 223, 204]),
        "airspace" | "clouds" => ([117, 178, 207], [247, 226, 199], [48, 84, 132]),
        "clockwork" | "pendulum" | "pipes" => ([182, 137, 77], [232, 216, 172], [12, 17, 22]),
        "city" | "traffic" | "drive" | "den" => ([239, 164, 89], [96, 160, 187], [8, 12, 22]),
        "aurora" => ([64, 202, 144], [159, 110, 217], [6, 13, 29]),
        "sand" => ([185, 140, 89], [231, 202, 153], [24, 22, 30]),
        "candy" | "plasma" | "mosaic" => ([225, 99, 159], [85, 174, 219], [19, 13, 32]),
        _ => ([98, 170, 204], [226, 190, 134], [6, 10, 22]),
    };
    match theme {
        "mono" | "noir" | "silver" | "steel" | "chrome" | "void" | "basalt" => {
            a = [169, 182, 193];
            b = [239, 240, 226];
            bg = [8, 11, 17];
        }
        "ice" | "frost" | "arctic" | "cold" | "winter" | "pond-blue" => {
            a = [115, 182, 225];
            b = [208, 237, 243];
            bg = [9, 22, 39];
        }
        "ember" | "inferno" | "hot" | "hotmetal" | "crimson" | "fire" => {
            a = [229, 83, 47];
            b = [255, 195, 106];
            bg = [25, 8, 15];
        }
        "warm" | "golden" | "gold" | "amber" | "mono-gold" | "evening" | "sunset" | "sepia" => {
            a = [224, 160, 83];
            b = [244, 206, 153];
            bg = [29, 17, 31];
        }
        "emerald" | "jade" | "green" | "deep-green" | "pcb" | "lime" | "acid" | "toxic" => {
            a = [80, 186, 107];
            b = [213, 222, 113];
            bg = [8, 24, 18];
        }
        "violet" | "royal" | "indigo" | "prism" => {
            a = [132, 116, 198];
            b = [199, 167, 225];
            bg = [15, 14, 32];
        }
        "pastel" | "blossom" => {
            a = [223, 173, 178];
            b = [179, 209, 202];
            bg = [31, 28, 40];
        }
        "night" | "midnight" | "night-outside" | "trench" | "dark" => {
            bg = [3, 8, 19];
            a = [67, 109, 149];
            b = [181, 196, 202];
        }
        _ => {}
    }
    if name == "scroll" {
        (a, b, bg) = match theme {
            "night" => ([168, 174, 184], [224, 228, 234], [7, 7, 9]),
            "indigo" => ([146, 168, 208], [216, 228, 246], [12, 18, 38]),
            _ => ([40, 36, 34], [22, 20, 20], [214, 202, 176]),
        };
    }
    (color(a), color(b), color(bg))
}

impl World {
    pub fn reset(&mut self, device: &wgpu::Device, _dims: (usize, usize)) {
        let dims = (192, 192);
        if self.dims != dims {
            self.state = std::array::from_fn(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("world-state"),
                    size: (dims.0 as u64 * dims.1 as u64).max(256) * 16,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            });
            self.dims = dims;
        }
        self.tick = 0;
        self.front = 0;
        self.initialized = false;
    }

    pub fn new(device: &wgpu::Device, _dims: (usize, usize)) -> Self {
        // A canonical simulation lattice makes physics independent of pixel
        // mode, monitor resolution, and portrait orientation. Drawing samples
        // state through normalized coordinates (U.dims.zw), never output indices.
        let (sw, sh) = (192u32, 192u32);
        let entries: Vec<_> = (0..4)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                count: None,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 3 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding == 0,
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world"),
            entries: &entries,
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene-worlds"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/world.wgsl").into()),
        });
        let pipeline = |entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let state = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("world-state"),
                size: (sw as u64 * sh as u64).max(256) * 16,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("world-params"),
            contents: &[0u8; std::mem::size_of::<Uniforms>()],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            layout,
            draw: Default::default(),
            init: pipeline("initialize"),
            step: pipeline("simulate"),
            shader,
            pipeline_layout: pl,
            params,
            state,
            dims: (sw, sh),
            tick: 0,
            front: 0,
            initialized: false,
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &wgpu::Buffer,
        size: (usize, usize),
        name: &str,
        opts: &SceneOptions,
        seed: u64,
        seconds: f32,
        speed: f32,
    ) {
        let id = scene::SCENES.iter().position(|s| s.name == name).unwrap() as u32;
        self.draw.entry(id).or_insert_with(|| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&self.pipeline_layout),
                module: &self.shader,
                entry_point: Some("draw"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("SCENE", id as f64)],
                    ..Default::default()
                },
                cache: None,
            })
        });
        let theme = opts.theme.as_deref().unwrap_or(scene::themes(name)[0]);
        let (a, b, bg) = palette(name, theme);
        let mut p = Uniforms {
            dims: [size.0 as u32, size.1 as u32, self.dims.0, self.dims.1],
            ids: [
                id,
                scene::themes(name)
                    .iter()
                    .position(|&s| s == theme)
                    .unwrap_or(0) as u32,
                (seed ^ (seed >> 32)) as u32,
                self.tick as u32,
            ],
            time: [
                seconds * speed,
                1.0 / 60.0,
                opts.detail.factor(),
                size.0 as f32 / size.1.max(1) as f32,
            ],
            primary: a,
            secondary: b,
            background: bg,
        };
        if !self.initialized {
            self.dispatch(device, queue, output, &p, &self.init, self.dims);
            self.front ^= 1;
            self.initialized = true;
        }
        if matches!(name, "fire" | "life" | "sand" | "reaction" | "boids") {
            let target = (seconds as f64 * speed as f64 * 60.0) as u64;
            for _ in 0..target.saturating_sub(self.tick).min(4) {
                p.ids[3] = self.tick as u32;
                self.dispatch(device, queue, output, &p, &self.step, self.dims);
                self.front ^= 1;
                self.tick += 1;
            }
        }
        self.dispatch(
            device,
            queue,
            output,
            &p,
            &self.draw[&id],
            (size.0 as u32, size.1 as u32),
        );
    }

    fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &wgpu::Buffer,
        params: &Uniforms,
        pipeline: &wgpu::ComputePipeline,
        dims: (u32, u32),
    ) {
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(params));
        let buffers = [
            &self.state[self.front],
            &self.state[self.front ^ 1],
            output,
            &self.params,
        ];
        let entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(i, b)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world"),
            layout: &self.layout,
            entries: &entries,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("world"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("world"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(dims.0.div_ceil(8), dims.1.div_ceil(8), 1);
        }
        queue.submit([encoder.finish()]);
    }
}
