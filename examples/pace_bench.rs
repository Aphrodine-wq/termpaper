//! Frame-pacing bench: jitter *and* CPU burn per strategy.
//!
//! Jitter alone is misleading — a spin-wait can win on jitter while burning a
//! core, and several termpaper instances pacing on the same machine starve each
//! other. Measures both, at the frame rates that matter.
use std::time::{Duration, Instant};

fn stddev(errors: &[f64]) -> f64 {
    let mean = errors.iter().sum::<f64>() / errors.len() as f64;
    (errors.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / errors.len() as f64).sqrt()
}

/// CPU time this thread has actually consumed, via clock_gettime on Linux.
fn cpu_secs() -> f64 {
    #[cfg(target_os = "linux")]
    {
        // CLOCK_THREAD_CPUTIME_ID = 3
        let mut ts = [0i64; 2];
        // SAFETY: writing two i64s that clock_gettime fills in
        let rc = unsafe {
            extern "C" {
                fn clock_gettime(clk: i32, tp: *mut i64) -> i32;
            }
            clock_gettime(3, ts.as_mut_ptr())
        };
        if rc == 0 {
            return ts[0] as f64 + ts[1] as f64 / 1e9;
        }
    }
    0.0
}

/// `spin_us`: how long before the deadline to stop sleeping and busy-wait.
/// 0 means never spin (pure sleep).
fn run(label: &str, fps: u32, work: Duration, spin_us: u64, frames: usize) {
    let frame = Duration::from_secs_f64(1.0 / fps as f64);
    let spin = Duration::from_micros(spin_us);
    let mut errs = Vec::with_capacity(frames);
    let mut deadline = Instant::now() + frame;
    let cpu0 = cpu_secs();
    let wall0 = Instant::now();
    for _ in 0..frames {
        std::thread::sleep(work); // stand-in for render work
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline - now;
            let coarse = remaining.saturating_sub(spin);
            if coarse.is_zero() {
                while Instant::now() < deadline {
                    std::hint::spin_loop();
                }
                break;
            }
            std::thread::sleep(coarse);
        }
        errs.push(Instant::now().duration_since(deadline).as_secs_f64() * 1000.0);
        deadline += frame;
    }
    let cpu = cpu_secs() - cpu0;
    let wall = wall0.elapsed().as_secs_f64();
    // the work itself is unavoidable; everything above it is pacing overhead
    let work_cpu = work.as_secs_f64() * frames as f64;
    println!(
        "  {label:<26} jitter {:.3}ms   cpu {:.0}% of wall   pacing burn {:+.0}ms",
        stddev(&errs),
        cpu / wall * 100.0,
        (cpu - work_cpu) * 1000.0,
    );
}

fn main() {
    // `work` is deliberately small: after the filter/grade optimisations a real
    // frame is well under 1ms, so almost the whole budget is pacing.
    for (fps, work_us) in [(240u32, 700u64), (120, 700), (60, 700)] {
        println!("\n=== {fps}fps, {work_us}us of work per frame ===");
        let work = Duration::from_micros(work_us);
        run("pure sleep", fps, work, 0, 400);
        run("spin last 100us", fps, work, 100, 400);
        run("spin last 250us", fps, work, 250, 400);
        run("spin last 1ms (current)", fps, work, 1000, 400);
    }
}
