//! Shared particle physics: verlet integration with gravity, drag, wind,
//! soft-bounds bounce, and a spring-damper steering helper. Scenes own the
//! particle state; these helpers keep motion real instead of lerped.

/// A verlet point mass. `px/py` = previous position (verlet memory).
#[derive(Clone, Copy, Debug, Default)]
pub struct Body {
    pub x: f32,
    pub y: f32,
    pub px: f32,
    pub py: f32,
}

impl Body {
    pub fn new(x: f32, y: f32) -> Self {
        Body { x, y, px: x, py: y }
    }

    pub fn velocity(&self) -> (f32, f32) {
        (self.x - self.px, self.y - self.py)
    }

    pub fn set_velocity(&mut self, vx: f32, vy: f32, dt: f32) {
        self.px = self.x - vx * dt;
        self.py = self.y - vy * dt;
    }
}

/// Forces applied during integration.
#[derive(Clone, Copy, Debug, Default)]
pub struct Forces {
    pub gravity: f32,
    /// quadratic air drag coefficient (0..~2)
    pub drag: f32,
    pub wind_x: f32,
    pub wind_y: f32,
    /// extra per-step acceleration (e.g. lift, gust impulses)
    pub ax: f32,
    pub ay: f32,
}

/// Verlet integrate one body one step. `dt` in seconds.
pub fn integrate(b: &mut Body, f: &Forces, dt: f32) {
    let (vx, vy) = b.velocity();
    // drag opposes motion, quadratically for bigger kicks at speed
    let speed = (vx * vx + vy * vy).sqrt();
    let drag_k = (1.0 - f.drag * (0.4 + speed * 0.02) * dt).max(0.0);
    let nx = b.x + vx * drag_k + (f.ax + f.wind_x) * dt * dt;
    let ny = b.y + vy * drag_k + (f.gravity + f.ay + f.wind_y) * dt * dt;
    b.px = b.x;
    b.py = b.y;
    b.x = nx;
    b.y = ny;
}

/// Soft-bounds bounce: reflect at edges with damping, no clamp popping.
/// `rest` = restitution 0..1 (0.6 is a gentle thud).
#[allow(dead_code)] // utility for scenes (rain splash) — kept public
pub fn bounce_bounds(b: &mut Body, min_x: f32, min_y: f32, max_x: f32, max_y: f32, rest: f32) {
    let (vx, vy) = b.velocity();
    if b.x < min_x {
        b.x = min_x;
        b.px = b.x + vx * rest;
    } else if b.x > max_x {
        b.x = max_x;
        b.px = b.x + vx * rest;
    }
    if b.y < min_y {
        b.y = min_y;
        b.py = b.y + vy * rest;
    } else if b.y > max_y {
        b.y = max_y;
        b.py = b.y + vy * rest;
    }
}

/// Spring-damper: acceleration pulling `cur` toward `target`, critically
/// damped-ish. Use for smooth steering (koi turns).
pub fn spring_damper(cur: f32, vel: f32, target: f32, stiffness: f32, damping: f32) -> f32 {
    (target - cur) * stiffness - vel * damping
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravity_falls_and_stays_stable() {
        let mut b = Body::new(0.0, 0.0);
        let f = Forces {
            gravity: 30.0,
            drag: 0.0,
            ..Default::default()
        };
        for _ in 0..600 {
            integrate(&mut b, &f, 1.0 / 60.0);
        }
        assert!(b.y > 0.0, "gravity should pull down");
        assert!(b.y.is_finite() && b.y < 1e6, "stable");
    }

    #[test]
    fn drag_reaches_terminal_velocity() {
        let mut b = Body::new(0.0, 0.0);
        let f = Forces {
            gravity: 30.0,
            drag: 1.5,
            ..Default::default()
        };
        let mut speeds = Vec::new();
        for _ in 0..600 {
            integrate(&mut b, &f, 1.0 / 60.0);
            speeds.push(b.velocity().1 / (1.0 / 60.0));
        }
        let late = &speeds[500..];
        let mean = late.iter().sum::<f32>() / late.len() as f32;
        let var = late.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / late.len() as f32;
        assert!(var.sqrt() < mean * 0.3, "should settle near terminal velocity");
    }

    #[test]
    fn bounce_keeps_body_in_bounds_and_loses_energy() {
        let mut b = Body::new(5.0, 5.0);
        b.set_velocity(60.0, 0.0, 1.0 / 60.0);
        let f = Forces::default();
        let mut peak_speed = 0.0f32;
        for _ in 0..120 {
            integrate(&mut b, &f, 1.0 / 60.0);
            bounce_bounds(&mut b, 0.0, 0.0, 10.0, 10.0, 0.6);
            let s = b.velocity().0.abs();
            peak_speed = peak_speed.max(s);
            assert!(b.x >= 0.0 && b.x <= 10.0, "escaped: {}", b.x);
        }
        let final_speed = b.velocity().0.abs();
        assert!(final_speed < 1.0, "restitution should drain energy");
    }

    #[test]
    fn spring_converges_without_runaway() {
        let (mut cur, mut vel) = (0.0f32, 0.0f32);
        for _ in 0..600 {
            let a = spring_damper(cur, vel, 10.0, 20.0, 8.0);
            vel += a * (1.0 / 60.0);
            cur += vel * (1.0 / 60.0);
        }
        assert!((cur - 10.0).abs() < 0.1, "spring should converge, at {cur}");
        assert!(vel.abs() < 0.1, "and come to rest");
    }
}
