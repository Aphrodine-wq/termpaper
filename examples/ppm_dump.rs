//! Dump a scene frame as a binary PPM for visual review.
//! Usage: ppm_dump <scene> <secs> <WxH> <theme|-> [half|quad|braille] <out.ppm>
//!
//! The pixel mode tells the scene which on-screen pixel aspect to compose
//! for (quad pixels are twice as tall as wide). To view the dump the way a
//! terminal would show it, scale the PNG's height by that aspect, e.g. for
//! quad: `magick out.ppm -filter point -resize 200%x400% out.png`.
use rand::{rngs::StdRng, SeedableRng};
use std::io::Write;
use termpaper::canvas::Canvas;
use termpaper::render::Pixels;
use termpaper::scene::{self, Detail, SceneOptions};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage: ppm_dump <scene> <secs> <WxH> <theme|-> [half|quad|braille] <out.ppm>");
        std::process::exit(2);
    }
    let secs: f32 = a[2].parse().unwrap();
    let (w, h) = a[3]
        .split_once('x')
        .map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap()))
        .unwrap();
    let theme = if a[4] == "-" { None } else { Some(a[4].clone()) };
    let (pixels, out) = if a.len() >= 7 {
        (Pixels::parse(&a[5]).expect("pixel mode: half|quad|braille"), &a[6])
    } else {
        (Pixels::Quad, &a[5])
    };
    let opts = SceneOptions {
        theme,
        detail: Detail::Medium,
        text_scale: None,
        pixels,
    };
    let mut s = scene::create(&a[1], &opts, StdRng::seed_from_u64(42)).expect("scene");
    let mut c = Canvas::new(w, h);
    for _ in 0..(secs * 30.0) as usize {
        s.update(1.0 / 30.0, &mut c);
    }
    let mut f = std::fs::File::create(out).unwrap();
    write!(f, "P6\n{w} {h}\n255\n").unwrap();
    let mut buf = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = c.get(x as i32, y as i32).color;
            buf.extend_from_slice(&[r, g, b]);
        }
    }
    f.write_all(&buf).unwrap();
    eprintln!(
        "{} {}x{} {} — view with: magick {} -filter point -resize 200%x{}% out.png",
        a[1],
        w,
        h,
        pixels.name(),
        out,
        (200.0 * pixels.aspect()) as u32
    );
}
