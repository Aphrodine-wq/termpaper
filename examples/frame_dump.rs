//! Frame dump for scene self-review (no display needed).
//! Usage: frame_dump <scene> [secs=6] [WxH=100x40] [theme]
//! Prints an ASCII luminance-ramp frame plus a histogram report.
use rand::{rngs::StdRng, SeedableRng};
use termpaper::canvas::Canvas;
use termpaper::scene::{self, Detail, SceneOptions};

fn lum(c: (u8, u8, u8)) -> u32 {
    (c.0 as u32 * 2 + c.1 as u32 * 3 + c.2 as u32) / 6
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).map(|s| s.as_str()).unwrap_or("koi");
    let secs: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(6.0);
    let (w, h) = args
        .get(3)
        .and_then(|s| s.split_once('x'))
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((100usize, 40usize));
    let theme = args.get(4).map(|s| s.as_str());

    let opts = SceneOptions {
        theme: theme.map(|t| t.to_string()),
        detail: Detail::Medium,
        text_scale: None,
    };
    let mut s = scene::create(name, &opts, StdRng::seed_from_u64(42)).expect("scene exists");
    let mut canvas = Canvas::new(w, h);
    let steps = (secs * 30.0) as usize;
    for _ in 0..steps {
        s.update(1.0 / 30.0, &mut canvas);
    }

    // ASCII frame: 2 vertical pixels per char row via luminance ramp
    const RAMP: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    println!("--- {name} (theme: {}) {w}x{h} after {secs}s ---", theme.unwrap_or("default"));
    for y in (0..h).step_by(2) {
        let mut line = String::with_capacity(w);
        for x in 0..w {
            let top = lum(canvas.get(x as i32, y as i32).color);
            let bot = if y + 1 < h {
                lum(canvas.get(x as i32, y as i32 + 1).color)
            } else {
                0
            };
            let l = ((top + bot) / 2).min(255);
            line.push(RAMP[(l as usize * (RAMP.len() - 1)) / 255]);
        }
        println!("{line}");
    }

    // histogram report
    let total = (w * h) as f32;
    let (mut black, mut near_black, mut bright, mut saturated) = (0usize, 0usize, 0usize, 0usize);
    let mut hist = [0usize; 8]; // luminance octiles
    let mut row_fill = vec![0usize; h];
    for y in 0..h {
        for x in 0..w {
            let c = canvas.get(x as i32, y as i32).color;
            let l = lum(c);
            if l == 0 {
                black += 1;
            } else if l < 16 {
                near_black += 1;
            }
            if l > 180 {
                bright += 1;
            }
            let (mx, mn) = (c.0.max(c.1).max(c.2), c.0.min(c.1).min(c.2));
            if mx > 60 && mx - mn > 40 {
                saturated += 1;
            }
            hist[(l as usize * 8) / 256] += 1;
            if l > 16 {
                row_fill[y] += 1;
            }
        }
    }
    let pct = |n: usize| format!("{:.1}%", n as f32 / total * 100.0);
    println!("--- histogram ---");
    println!("pure black: {}  near-black(<16): {}  bright(>180): {}  saturated accents: {}",
        pct(black), pct(near_black), pct(bright), pct(saturated));
    print!("lum octiles:");
    for (i, n) in hist.iter().enumerate() {
        print!("  {i}:{:>4.1}%", *n as f32 / total * 100.0);
    }
    println!();
    let filled_rows = row_fill.iter().filter(|&&n| n > 0).count();
    let filled_cols = (0..w)
        .filter(|&x| (0..h).any(|y| lum(canvas.get(x as i32, y as i32).color) > 16))
        .count();
    println!(
        "coverage: rows with content {}/{} ({:.0}%), cols {}/{} ({:.0}%)",
        filled_rows,
        h,
        filled_rows as f32 / h as f32 * 100.0,
        filled_cols,
        w,
        filled_cols as f32 / w as f32 * 100.0
    );
}
