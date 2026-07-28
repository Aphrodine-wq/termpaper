//! Frame-pacing jitter bench: naive sleep vs coarse-sleep + spin-wait.
use std::time::{Duration, Instant};

fn stddev(errors: &[f64]) -> f64 {
    let mean = errors.iter().sum::<f64>() / errors.len() as f64;
    (errors.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / errors.len() as f64).sqrt()
}

fn main() {
    let frame = Duration::from_secs_f64(1.0 / 60.0);
    let work = Duration::from_millis(4);

    // naive: sleep the whole remaining budget
    let mut errs = Vec::new();
    let mut deadline = Instant::now() + frame;
    for _ in 0..300 {
        std::thread::sleep(work);
        let now = Instant::now();
        if now < deadline {
            std::thread::sleep(deadline - now);
        }
        errs.push(Instant::now().duration_since(deadline).as_secs_f64() * 1000.0);
        deadline += frame;
    }
    println!(
        "naive sleep:      jitter stddev {:.3} ms over 300 frames",
        stddev(&errs)
    );

    // paced: coarse sleep to 1ms before deadline, then spin
    let mut errs = Vec::new();
    let mut deadline = Instant::now() + frame;
    for _ in 0..300 {
        std::thread::sleep(work);
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline - now;
            let coarse = remaining.saturating_sub(Duration::from_millis(1));
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
    println!(
        "coarse+spin pace: jitter stddev {:.3} ms over 300 frames",
        stddev(&errs)
    );
}
