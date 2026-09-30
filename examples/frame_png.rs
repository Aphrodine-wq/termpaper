//! A Classic scene as a PNG contact sheet, for review without a terminal.
//!
//! ```text
//! cargo run --release --example frame_png -- <scene> [OUT_DIR] [WxH=160x90] [secs=2,10,30,60,90,120] [fps=60]
//! ```
//!
//! One row per variant, one column per moment; each canvas pixel is drawn
//! as a 4x4 block so the sheet is readable. Frames are the scene's own
//! pixels (no post-processing), stepped at `fps` from a fixed seed, so two
//! runs of one build are identical and a before/after pair shows exactly
//! what a change did. Writes `OUT_DIR/<scene>.png` (default
//! `target/frame_png`).
use rand::{rngs::StdRng, SeedableRng};
use std::path::PathBuf;
use termpaper::canvas::Canvas;
use termpaper::scene::{self, Detail, SceneOptions};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = args.first().map(String::as_str).unwrap_or("koi");
    let out = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("target/frame_png"));
    let (w, h) = args
        .get(2)
        .and_then(|s| s.split_once('x'))
        .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))
        .unwrap_or((160, 90));
    let secs: Vec<f32> = args
        .get(3)
        .map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![2.0, 10.0, 30.0, 60.0, 90.0, 120.0]);
    let fps: f32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let Some(def) = scene::find(name) else {
        eprintln!("{name}: not a Classic scene");
        std::process::exit(2);
    };
    let scale = 4usize;
    let gap = 6usize;
    let cols = secs.len();
    let rows = def.themes.len();
    let (sw, sh) = (gap + cols * (w * scale + gap), gap + rows * (h * scale + gap));
    let mut px = vec![[22u8, 22, 26]; sw * sh];
    for (ri, theme) in def.themes.iter().enumerate() {
        let opts = SceneOptions { theme: Some(theme.to_string()), detail: Detail::Medium, text_scale: None, pixels: Default::default() };
        let mut s = scene::create(name, &opts, StdRng::seed_from_u64(4242)).unwrap();
        let mut c = Canvas::new(w, h);
        let mut t = 0.0f32;
        let dt = 1.0 / fps;
        for (ci, &at) in secs.iter().enumerate() {
            while t < at {
                s.update(dt, &mut c);
                t += dt;
            }
            let (x0, y0) = (gap + ci * (w * scale + gap), gap + ri * (h * scale + gap));
            for y in 0..h {
                for x in 0..w {
                    let (r, g, b) = c.get(x as i32, y as i32).color;
                    for dy in 0..scale {
                        for dx in 0..scale {
                            px[(y0 + y * scale + dy) * sw + x0 + x * scale + dx] = [r, g, b];
                        }
                    }
                }
            }
        }
    }
    std::fs::create_dir_all(&out).expect("out dir");
    let path = out.join(format!("{name}.png"));
    let file = std::fs::File::create(&path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), sw as u32, sh as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    let flat: Vec<u8> = px.iter().flat_map(|p| p.iter().copied()).collect();
    writer.write_image_data(&flat).expect("png data");
    println!("wrote {} ({} variants x {} moments, {w}x{h} px at {fps} fps)", path.display(), rows, cols);
}
