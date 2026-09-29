//! Pixel-level diagnostic: half mode is a lossless pixel view of the canvas,
//! so running the same content through it isolates real shader divergence from
//! the block-split tie flips that quad/braille mix in.
use rand::{rngs::StdRng, SeedableRng};
use ratatui::{buffer::Buffer, layout::Rect};
use termpaper::canvas::Canvas;
use termpaper::gpu::{FrameCells, Gpu, Plan};
use termpaper::render::Pixels;
use termpaper::scene::{self, Detail, SceneOptions};
use termpaper::{filter, render};

fn main() {
    let (cols, rows) = (544usize, 66usize); // half mode -> 544x132 px
    let (w, h) = (cols, rows * 2);
    let opts = SceneOptions { theme: None, detail: Detail::Low, text_scale: None, pixels: Default::default() };
    let mut s = scene::create("koi", &opts, StdRng::seed_from_u64(7)).unwrap();
    let mut canvas = Canvas::new(w, h);
    for _ in 0..40 { s.update(1.0 / 60.0, &mut canvas); }

    let mut gpu = Gpu::new(w, h, cols * rows).unwrap();
    let area = Rect::new(0, 0, cols as u16, rows as u16);

    for name in ["hue", "spectrum", "sharpen", "bloom", "pixelate", "edges"] {
        let filters = vec![name.to_string()];
        let plan = Plan {
            look: &termpaper::look::Look::with_effects(&filters), lut: None, quick_filter: None, t: 1.5, dim: 1.0, smooth: 0.0,
            pixels: Pixels::Half, cols, rows, crop: (0, 0), virt: None, hysteresis: 0,
        };
        let mut c = canvas.clone_for_smooth();
        filter::apply_all(&filters, &mut c, plan.t);
        let mut buf = Buffer::empty(area);
        render::draw(&c, area, &mut buf, true, Pixels::Half);

        gpu.drain();
        let words = gpu.run_blocking(&canvas, &plan).unwrap();
        let cells = FrameCells { words: &words, cols, rows };

        let mut hist = [0usize; 8];
        let mut worst = 0i32;
        let mut sample = None;
        for row in 0..rows {
            for col in 0..cols {
                let cell = &buf[(col as u16, row as u16)];
                let rgb = |c: ratatui::style::Color| match c {
                    ratatui::style::Color::Rgb(r, g, b) => (r, g, b), _ => (0, 0, 0) };
                let (cf, cb) = (rgb(cell.fg), rgb(cell.bg));
                let (_, gf, gb) = cells.get(row * cols + col);
                let mut d = 0i32;
                for (a, b) in [(cf.0,gf.0),(cf.1,gf.1),(cf.2,gf.2),(cb.0,gb.0),(cb.1,gb.1),(cb.2,gb.2)] {
                    d = d.max((a as i32 - b as i32).abs());
                }
                hist[(d as usize).min(7)] += 1;
                if d > worst { worst = d; sample = Some((col, row, cf, gf, cb, gb)); }
            }
        }
        println!("{name:<10} worst Δ {worst:>3}  hist(Δ=0..7+) {hist:?}");
        if worst > 1 { println!("           sample {:?}", sample.unwrap()); }
    }
}
