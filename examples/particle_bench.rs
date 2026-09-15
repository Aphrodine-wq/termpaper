//! Particle-path bench: what does one particle actually cost, and where does
//! the time go? Most scenes draw a particle as a `glow()` plus a core `set`,
//! so glow throughput sets the particle ceiling.
use std::time::Instant;
use termpaper::canvas::{glow, Canvas};

fn bench(label: &str, iters: usize, mut f: impl FnMut()) -> f64 {
    f();
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / iters as f64;
    println!("  {label:<40} {ns:>9.1} ns");
    ns
}

fn main() {
    let (w, h) = (544usize, 132usize); // braille-mode canvas for a wide pane
    let mut c = Canvas::new(w, h);

    println!("=== primitives (per call) ===");
    let mut i = 0u32;
    bench("canvas.set (in bounds)", 2_000_000, || {
        i = i.wrapping_add(7);
        c.set((i % w as u32) as i32, (i % h as u32) as i32, (200, 180, 90));
    });
    bench("canvas.add (in bounds)", 2_000_000, || {
        i = i.wrapping_add(7);
        c.add((i % w as u32) as i32, (i % h as u32) as i32, (20, 18, 9));
    });
    bench("canvas.add (out of bounds)", 2_000_000, || {
        c.add(-50, -50, (20, 18, 9));
    });

    println!("\n=== glow() per call, by radius (interior, no clipping) ===");
    let mut per_r = Vec::new();
    for r in 1..=5i32 {
        let ns = bench(&format!("glow r={r}  ({} px in disc)", {
            let mut n = 0;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        n += 1;
                    }
                }
            }
            n
        }), 300_000, || {
            glow(&mut c, 270, 66, r, (255, 200, 120), 0.5);
        });
        per_r.push((r, ns));
    }

    println!("\n=== glow() at the canvas edge (every write bounds-rejected) ===");
    for r in [2i32, 4] {
        bench(&format!("glow r={r} fully off-canvas"), 300_000, || {
            glow(&mut c, -20, -20, r, (255, 200, 120), 0.5);
        });
    }

    println!("\n=== particle budget at 240fps (4.17ms/frame) ===");
    for (r, ns) in &per_r {
        // a particle is typically one glow plus a core set
        let each = ns + 12.0;
        println!(
            "  r={r}: {:.0} ns/particle → {:.0} particles/frame at 240fps",
            each,
            4_170_000.0 / each
        );
    }
}
