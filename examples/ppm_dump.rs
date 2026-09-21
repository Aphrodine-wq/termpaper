//! Dump a scene frame as a binary PPM for visual review.
//! Usage: ppm_dump <scene> <secs> <WxH> <theme|-> <out.ppm>
use rand::{rngs::StdRng, SeedableRng};
use std::io::Write;
use termpaper::canvas::Canvas;
use termpaper::scene::{self, Detail, SceneOptions};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let secs: f32 = a[2].parse().unwrap();
    let (w, h) = a[3].split_once('x').map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())).unwrap();
    let theme = if a[4] == "-" { None } else { Some(a[4].clone()) };
    let opts = SceneOptions { theme, detail: Detail::Medium, text_scale: None };
    let mut s = scene::create(&a[1], &opts, StdRng::seed_from_u64(42)).expect("scene");
    let mut c = Canvas::new(w, h);
    for _ in 0..(secs * 30.0) as usize {
        s.update(1.0 / 30.0, &mut c);
    }
    let mut f = std::fs::File::create(&a[5]).unwrap();
    write!(f, "P6\n{w} {h}\n255\n").unwrap();
    let mut buf = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = c.get(x as i32, y as i32).color;
            buf.extend_from_slice(&[r, g, b]);
        }
    }
    f.write_all(&buf).unwrap();
}
