//! Contact sheets and timings for Studio shader scenes.
//!
//! ```text
//! cargo run --release --example shader_review -- [SCENE|all] [options]
//!   --out DIR        where PNGs go (default target/shader_review)
//!   --dir DIR        scene sources (default src/gpu/scenes) — read from disk,
//!                    so edits show up without rebuilding
//!   --times a,b,c    scene times in seconds (default 5,60,600)
//!   --spp N          samples per pixel for the hi-res sheet (default 16)
//!   --desk           render across this machine's monitors (Hyprland), to
//!                    scale in millimetres: `<name>_desk.png`, one row per theme
//!   --bench          GPU time per frame at 544x132, 1 spp, medium detail,
//!                    cycling through the themes (needs timestamps)
//! ```
//!
//! Per scene it writes `<name>.png` (rows = themes, columns = times, plus a
//! framing row: portrait 9:16 and ultrawide 32:9) and `<name>_term.png`, the
//! same scene at the resolution a 1080p terminal actually shows in half-block
//! mode (213x112 px), enlarged 3x with hard pixel edges — what you will see.
use std::path::{Path, PathBuf};
use termpaper::gpu::{self, Gpu};
use termpaper::scene::{shader, Detail};

struct Opts {
    which: String,
    out: PathBuf,
    dir: PathBuf,
    times: Vec<f64>,
    spp: u32,
    bench: bool,
    desk: bool,
}

fn parse() -> Opts {
    let mut o = Opts {
        which: "all".into(),
        out: PathBuf::from("target/shader_review"),
        dir: PathBuf::from("src/gpu/scenes"),
        times: vec![5.0, 60.0, 600.0],
        spp: 16,
        bench: false,
        desk: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => o.out = args.next().expect("--out DIR").into(),
            "--dir" => o.dir = args.next().expect("--dir DIR").into(),
            "--times" => {
                o.times = args
                    .next()
                    .expect("--times a,b")
                    .split(',')
                    .map(|t| t.trim().parse().expect("time in seconds"))
                    .collect()
            }
            "--spp" => o.spp = args.next().expect("--spp N").parse().expect("number"),
            "--bench" => o.bench = true,
            "--desk" => o.desk = true,
            s if !s.starts_with("--") => o.which = s.to_string(),
            s => panic!("unknown option {s}"),
        }
    }
    o
}

struct Sheet {
    w: usize,
    h: usize,
    px: Vec<[u8; 3]>,
}

impl Sheet {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![[24, 24, 28]; w * h] }
    }
    fn blit(&mut self, x0: usize, y0: usize, img: &gpu::ShaderPixels, scale: usize) {
        for y in 0..img.height * scale {
            for x in 0..img.width * scale {
                let v = img.data[(y / scale) * img.width + x / scale];
                let (dx, dy) = (x0 + x, y0 + y);
                if dx < self.w && dy < self.h {
                    self.px[dy * self.w + dx] = [(v & 0xff) as u8, ((v >> 8) & 0xff) as u8, ((v >> 16) & 0xff) as u8];
                }
            }
        }
    }
    fn save(&self, path: &Path) {
        let ppm = path.with_extension("ppm");
        let mut bytes = format!("P6\n{} {}\n255\n", self.w, self.h).into_bytes();
        for p in &self.px {
            bytes.extend_from_slice(p);
        }
        std::fs::write(&ppm, bytes).expect("write ppm");
        let ok = std::process::Command::new("magick")
            .arg(&ppm)
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            let _ = std::fs::remove_file(&ppm);
            println!("  wrote {}", path.display());
        } else {
            println!("  wrote {} (install ImageMagick for PNG)", ppm.display());
        }
    }
}

fn render(
    g: &mut Gpu,
    name: &str,
    composed: &shader::Composed,
    size: (usize, usize),
    theme: u32,
    t: f64,
    spp: u32,
) -> Result<gpu::ShaderPixels, String> {
    render_at(g, name, composed, size, theme, t, spp, Detail::High)
}

#[allow(clippy::too_many_arguments)]
fn render_at(
    g: &mut Gpu,
    name: &str,
    composed: &shader::Composed,
    size: (usize, usize),
    theme: u32,
    t: f64,
    spp: u32,
    detail: Detail,
) -> Result<gpu::ShaderPixels, String> {
    let ms = (t * 1000.0) as u64;
    let u = gpu::uniforms(&gpu::FrameDesc {
        view: gpu::ShaderView::for_canvas(size, (0, 0), 1.0),
        window: size,
        time: gpu::shader_time(ms, 1.0),
        speed: 1.0,
        seed: 0x5eed_1234_abcd,
        theme,
        detail,
        spp,
        mirror: false,
        kaleido: false,
        exposure: 0.0,
    });
    g.render_shader_pixels(name, composed, &u)
}

/// Render a scene across the desk: every monitor a fullscreen pane, placed by
/// the physical wall plan's own mapping, composited to scale (px per mm).
fn render_desk(g: &mut Gpu, name: &str, composed: &shader::Composed, themes: usize, t: f64, out: &Path) {
    use termpaper::desk::{comp_view, Desk, DeskConfig};
    let mons = termpaper::hypr::monitors().unwrap_or_default();
    if mons.is_empty() {
        println!("  --desk needs Hyprland");
        return;
    }
    let desk = Desk::from_hypr(&mons, &termpaper::desk::load_desk());
    let cfg = DeskConfig::default();
    let panes: Vec<_> = desk.monitors.iter().map(|d| (d.rect, d.mon.portrait())).collect();
    let Some(frame) = desk.frame(&cfg, &panes) else { return };
    let ppm = 0.8; // output px per mm
    let bb = desk.monitors.iter().map(|d| d.rect).reduce(|a, b| a.union(&b)).unwrap();
    let (w, h) = ((bb.w * ppm).ceil() as usize, (bb.h * ppm).ceil() as usize);
    let gap = 8;
    let mut sheet = Sheet::new(w, (h + gap) * themes);
    for ti in 0..themes {
        for d in &desk.monitors {
            let px = ((d.rect.w * ppm).round().max(1.0) as usize, (d.rect.h * ppm).round().max(1.0) as usize);
            let v = comp_view(frame, d.rect, px);
            let u = gpu::uniforms(&gpu::FrameDesc {
                view: gpu::ShaderView { origin: v.origin, step: v.step, half: v.half },
                window: px,
                time: gpu::shader_time((t * 1000.0) as u64, 1.0),
                speed: 1.0,
                seed: 0x5eed_1234_abcd,
                theme: ti as u32,
                detail: Detail::High,
                spp: 6,
                mirror: false,
                kaleido: false,
                exposure: 0.0,
            });
            match g.render_shader_pixels(name, composed, &u) {
                Ok(p) => {
                    let x0 = ((d.rect.x - bb.x) * ppm) as usize;
                    let y0 = ((d.rect.y - bb.y) * ppm) as usize + ti * (h + gap);
                    sheet.blit(x0, y0, &p, 1);
                }
                Err(e) => {
                    println!("  {e}");
                    return;
                }
            }
        }
    }
    sheet.save(&out.join(format!("{name}_desk.png")));
}

fn scene_names(o: &Opts) -> Vec<String> {
    if o.which != "all" {
        return vec![o.which.clone()];
    }
    let mut v: Vec<String> = std::fs::read_dir(&o.dir)
        .expect("scene dir")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "wgsl").then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    v.sort();
    v
}

fn main() {
    let o = parse();
    std::fs::create_dir_all(&o.out).expect("out dir");
    let mut g = Gpu::new(1, 1, 1).expect("a Vulkan GPU");
    println!("adapter: {}", g.adapter_name());
    let mut failures = 0;
    for name in scene_names(&o) {
        let (composed, meta) = match shader::compose_from_dir(&name, &o.dir) {
            Ok(c) => c,
            Err(e) => {
                println!("{name}: {e}");
                failures += 1;
                continue;
            }
        };
        println!("{name} — {} [{}] themes: {}", meta.title, meta.cost, meta.themes.join(", "));
        if o.bench {
            let mut samples = Vec::new();
            let mut failed = false;
            for i in 0..24 {
                let theme = (i % meta.themes.len()) as u32;
                match render_at(&mut g, &name, &composed, (544, 132), theme, 30.0 + i as f64 * 0.25, 1, Detail::Medium) {
                    Ok(p) => samples.extend(p.gpu_ms),
                    Err(e) => {
                        println!("  {e}");
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                failures += 1;
                continue;
            }
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if let Some(med) = samples.get(samples.len() / 2) {
                // at 544x132 (about three 1080p half-block panes) and 1 spp;
                // a pane then fits several samples in the 3 ms governor budget
                let budget = match meta.cost.as_str() {
                    "light" => 0.8,
                    "medium" => 1.6,
                    _ => 2.5,
                };
                let verdict = if *med <= budget { "ok" } else { "OVER BUDGET" };
                println!("  bench 544x132 @1spp: {med:.3} ms (budget {budget} ms) {verdict}");
            } else {
                println!("  bench: adapter has no timestamp queries");
            }
            continue;
        }
        let themes = meta.themes.len();
        if o.desk {
            let t = o.times.get(1).copied().unwrap_or(o.times[0]);
            render_desk(&mut g, &name, &composed, themes, t, &o.out);
            continue;
        }
        // hi-res sheet
        let (tw, th, gap) = (480usize, 270usize, 6usize);
        let cols = o.times.len();
        let frame_row_h = 270;
        let sheet_w = gap + cols * (tw + gap);
        let sheet_h = gap + themes * (th + gap) + frame_row_h + gap;
        let mut sheet = Sheet::new(sheet_w.max(152 + 480 + 3 * gap), sheet_h);
        let mut ok = true;
        'outer: for ti in 0..themes {
            for (ci, &t) in o.times.iter().enumerate() {
                match render(&mut g, &name, &composed, (tw, th), ti as u32, t, o.spp) {
                    Ok(p) => sheet.blit(gap + ci * (tw + gap), gap + ti * (th + gap), &p, 1),
                    Err(e) => {
                        println!("  {e}");
                        ok = false;
                        break 'outer;
                    }
                }
            }
        }
        if !ok {
            failures += 1;
            continue;
        }
        // framing: portrait 9:16 and ultrawide 32:9 at the second time
        let t = o.times.get(1).copied().unwrap_or(o.times[0]);
        let y = gap + themes * (th + gap);
        if let Ok(p) = render(&mut g, &name, &composed, (152, 270), 0, t, o.spp) {
            sheet.blit(gap, y, &p, 1);
        }
        if let Ok(p) = render(&mut g, &name, &composed, (sheet_w.saturating_sub(152 + 3 * gap).min(960), 270), 0, t, o.spp) {
            sheet.blit(152 + 2 * gap, y, &p, 1);
        }
        sheet.save(&o.out.join(format!("{name}.png")));

        // terminal-resolution sheet: 213x112 px (half blocks on 1080p), 3x
        let (tw, th, sc) = (213usize, 112usize, 3usize);
        let mut term = Sheet::new(gap + cols * (tw * sc + gap), gap + themes * (th * sc + gap));
        for ti in 0..themes {
            for (ci, &t) in o.times.iter().enumerate() {
                if let Ok(p) = render(&mut g, &name, &composed, (tw, th), ti as u32, t, 8) {
                    term.blit(gap + ci * (tw * sc + gap), gap + ti * (th * sc + gap), &p, sc);
                }
            }
        }
        term.save(&o.out.join(format!("{name}_term.png")));
    }
    if failures > 0 {
        std::process::exit(1);
    }
}
