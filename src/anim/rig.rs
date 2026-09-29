//! Rigs and motion: forward-kinematic bone chains, two-bone IK, eased
//! keyframe tracks, spring "follow" values for overlapping action, and
//! squash-and-stretch.
//!
//! Rigs are authored in *screen* units (a round rig stays round); the
//! skeleton squashes y by `sy` when it writes canvas coordinates.
use crate::canvas::{ease_in_out, ease_out, ease_out_back, ease_smooth};
use crate::physics::spring_damper;

#[derive(Clone, Copy, Debug)]
pub struct Bone {
    /// index of the parent bone; `None` hangs off the root
    pub parent: Option<u8>,
    /// length in screen units before `scale`
    pub len: f32,
    /// rest angle in radians relative to the parent's direction
    pub rest: f32,
}

/// A solved bone: start, end (canvas px) and absolute screen-space angle.
#[derive(Clone, Copy, Debug, Default)]
pub struct Joint {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub angle: f32,
}

#[derive(Clone, Debug)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    /// root position in canvas px
    pub root: (f32, f32),
    /// absolute angle of the root direction (radians, screen space)
    pub root_angle: f32,
    pub scale: f32,
    pub flip_x: bool,
    /// pixel aspect squash (`Stage::sy`)
    pub sy: f32,
}

/// Per-bone angle offsets from rest.
pub type Pose = Vec<f32>;

impl Skeleton {
    pub fn new(bones: Vec<Bone>, root: (f32, f32), scale: f32, sy: f32) -> Self {
        Skeleton {
            bones,
            root,
            root_angle: 0.0,
            scale,
            flip_x: false,
            sy,
        }
    }

    /// Forward kinematics: `pose[i]` is added to bone i's rest angle. Missing
    /// pose entries count as zero. Output is in canvas pixels.
    pub fn solve(&self, pose: &[f32], out: &mut Vec<Joint>) {
        out.clear();
        out.resize(self.bones.len(), Joint::default());
        // screen-space endpoints relative to the root, before squash
        let mut ends: Vec<(f32, f32, f32)> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            let (px, py, pa) = match b.parent {
                Some(p) => ends[p as usize],
                None => (0.0, 0.0, self.root_angle),
            };
            let a = pa + b.rest + pose.get(i).copied().unwrap_or(0.0);
            let len = b.len * self.scale;
            let ex = px + a.cos() * len;
            let ey = py + a.sin() * len;
            ends.push((ex, ey, a));
            let fx = if self.flip_x { -1.0 } else { 1.0 };
            out[i] = Joint {
                x0: self.root.0 + px * fx,
                y0: self.root.1 + py * self.sy,
                x1: self.root.0 + ex * fx,
                y1: self.root.1 + ey * self.sy,
                angle: a,
            };
        }
    }

    /// End of bone `i` from a solved pose.
    pub fn tip(joints: &[Joint], i: usize) -> (f32, f32) {
        joints.get(i).map(|j| (j.x1, j.y1)).unwrap_or((0.0, 0.0))
    }
}

/// Analytic two-bone IK in screen units. Returns the elbow position and the
/// absolute angles of both bones. `bend` (+1 / -1) picks which side the elbow
/// folds toward. Unreachable targets stretch straight toward the target.
pub fn ik2(root: (f32, f32), target: (f32, f32), l1: f32, l2: f32, bend: f32) -> ((f32, f32), f32, f32) {
    let (dx, dy) = (target.0 - root.0, target.1 - root.1);
    let d = (dx * dx + dy * dy).sqrt().max(1e-4);
    let base = dy.atan2(dx);
    let reach = (l1 + l2).max(1e-3);
    let d = d.min(reach).max((l1 - l2).abs() + 1e-3);
    // law of cosines
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a0 = base - bend.signum() * cos_a.acos();
    let elbow = (root.0 + a0.cos() * l1, root.1 + a0.sin() * l1);
    let a1 = (target.1 - elbow.1).atan2(target.0 - elbow.0);
    (elbow, a0, a1)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    In,
    Out,
    InOut,
    Smooth,
    OutBack,
    Elastic,
    /// jump to the value at the key (no interpolation)
    Hold,
}

/// Apply an easing to `t` in 0..1.
pub fn ease(e: Ease, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match e {
        Ease::Linear => t,
        Ease::In => t * t * t,
        Ease::Out => ease_out(t),
        Ease::InOut => ease_in_out(t),
        Ease::Smooth => ease_smooth(t),
        Ease::OutBack => ease_out_back(t),
        Ease::Elastic => {
            if t <= 0.0 || t >= 1.0 {
                t
            } else {
                let p = 0.4;
                (2.0f32).powf(-10.0 * t) * ((t - p / 4.0) * std::f32::consts::TAU / p).sin() + 1.0
            }
        }
        Ease::Hold => {
            if t >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// A keyframe: value `v` reached at time `t`, approached with `ease`.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub t: f32,
    pub v: f32,
    pub ease: Ease,
}

impl Key {
    pub const fn new(t: f32, v: f32, ease: Ease) -> Self {
        Key { t, v, ease }
    }
}

/// An eased keyframe track. Holds the first/last value outside its range.
#[derive(Clone, Debug, Default)]
pub struct Track {
    pub keys: Vec<Key>,
}

impl Track {
    pub fn new(keys: Vec<Key>) -> Self {
        Track { keys }
    }

    pub fn at(&self, t: f32) -> f32 {
        let Some(first) = self.keys.first() else {
            return 0.0;
        };
        if t <= first.t {
            return first.v;
        }
        for w in self.keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t <= b.t {
                let span = (b.t - a.t).max(1e-6);
                let f = ease(b.ease, (t - a.t) / span);
                return a.v + (b.v - a.v) * f;
            }
        }
        self.keys[self.keys.len() - 1].v
    }

    pub fn duration(&self) -> f32 {
        self.keys.last().map(|k| k.t).unwrap_or(0.0)
    }
}

/// A clip: one track per channel (bone angles plus extras), fixed duration.
#[derive(Clone, Debug, Default)]
pub struct Clip {
    pub dur: f32,
    pub tracks: Vec<Track>,
}

impl Clip {
    pub fn new(dur: f32, tracks: Vec<Track>) -> Self {
        Clip { dur, tracks }
    }

    /// Sample every track at `t` into `out` (resized to the track count).
    pub fn sample(&self, t: f32, out: &mut Vec<f32>) {
        out.clear();
        out.extend(self.tracks.iter().map(|tr| tr.at(t)));
    }

    /// Sample with time wrapped into the clip (looping walk cycles).
    pub fn sample_looped(&self, t: f32, out: &mut Vec<f32>) {
        let tt = if self.dur > 0.0 { t.rem_euclid(self.dur) } else { 0.0 };
        self.sample(tt, out);
    }
}

/// Blend two poses channel-wise.
pub fn blend_pose(a: &[f32], b: &[f32], t: f32, out: &mut Vec<f32>) {
    out.clear();
    let n = a.len().max(b.len());
    for i in 0..n {
        let va = a.get(i).copied().unwrap_or(0.0);
        let vb = b.get(i).copied().unwrap_or(0.0);
        out.push(va + (vb - va) * t.clamp(0.0, 1.0));
    }
}

/// Overlapping action: a value that chases its target through a spring, so
/// child parts (tails, ears, hoods, the thumb's last joint, a gun's recoil)
/// lag, overshoot and settle instead of stopping with their parent.
#[derive(Clone, Copy, Debug)]
pub struct Follow {
    pub val: f32,
    pub vel: f32,
    /// stiffness
    pub k: f32,
    /// damping
    pub d: f32,
}

impl Follow {
    pub fn new(val: f32, k: f32, d: f32) -> Self {
        Follow { val, vel: 0.0, k, d }
    }

    /// Advance toward `target`; returns the new value. Sub-stepped so stiff
    /// springs stay stable at the 1/30 s the fast-forward path uses.
    pub fn step(&mut self, target: f32, dt: f32) -> f32 {
        let n = ((dt * self.k.sqrt() * 0.5).ceil() as usize).clamp(1, 8);
        let h = dt / n as f32;
        for _ in 0..n {
            let acc = spring_damper(self.val, self.vel, target, self.k, self.d);
            self.vel += acc * h;
            self.val += self.vel * h;
        }
        self.val
    }

    /// Instant velocity impulse (a kick, a recoil).
    pub fn kick(&mut self, impulse: f32) {
        self.vel += impulse;
    }

    /// Snap to a value with no velocity.
    pub fn set(&mut self, v: f32) {
        self.val = v;
        self.vel = 0.0;
    }
}

/// Volume-preserving squash and stretch: `s > 0` stretches along the axis,
/// `s < 0` squashes. Returns `(along, across)` scale factors.
pub fn squash(s: f32) -> (f32, f32) {
    let along = (1.0 + s).max(0.2);
    (along, 1.0 / along)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn fk_tip_positions() {
        // two bones of length 10, the second bent 90° down (screen y-down)
        let sk = Skeleton::new(
            vec![
                Bone { parent: None, len: 10.0, rest: 0.0 },
                Bone { parent: Some(0), len: 10.0, rest: FRAC_PI_2 },
            ],
            (100.0, 50.0),
            1.0,
            0.5,
        );
        let mut j = Vec::new();
        sk.solve(&[0.0, 0.0], &mut j);
        assert!((j[0].x1 - 110.0).abs() < 1e-4 && (j[0].y1 - 50.0).abs() < 1e-4);
        // the second bone drops 10 screen units = 5 canvas px under sy = 0.5
        let tip = Skeleton::tip(&j, 1);
        assert!((tip.0 - 110.0).abs() < 1e-4 && (tip.1 - 55.0).abs() < 1e-4, "{tip:?}");
        // a pose offset rotates the chain
        sk.solve(&[FRAC_PI_2, 0.0], &mut j);
        assert!((j[0].x1 - 100.0).abs() < 1e-4 && (j[0].y1 - 55.0).abs() < 1e-4);
        // flipped rigs mirror in x only
        let mut fl = sk.clone();
        fl.flip_x = true;
        fl.solve(&[0.0, 0.0], &mut j);
        assert!((j[0].x1 - 90.0).abs() < 1e-4);
    }

    #[test]
    fn ik_reaches_the_target() {
        let (elbow, a0, a1) = ik2((0.0, 0.0), (12.0, 5.0), 8.0, 8.0, 1.0);
        let hand = (elbow.0 + a1.cos() * 8.0, elbow.1 + a1.sin() * 8.0);
        assert!((hand.0 - 12.0).abs() < 1e-3 && (hand.1 - 5.0).abs() < 1e-3, "{hand:?}");
        assert!((elbow.0 - a0.cos() * 8.0).abs() < 1e-3);
        // the other bend side mirrors the elbow across the root→target line
        let (elbow2, _, _) = ik2((0.0, 0.0), (12.0, 5.0), 8.0, 8.0, -1.0);
        assert!((elbow.1 - elbow2.1).abs() > 1.0);
        // unreachable: straight line toward the target
        let (_, b0, b1) = ik2((0.0, 0.0), (100.0, 0.0), 8.0, 8.0, 1.0);
        assert!(b0.abs() < 1e-3 && b1.abs() < 1e-3);
    }

    #[test]
    fn track_holds_ends_and_eases() {
        let tr = Track::new(vec![
            Key::new(0.0, 0.0, Ease::Linear),
            Key::new(1.0, 10.0, Ease::Linear),
            Key::new(2.0, 0.0, Ease::Out),
        ]);
        assert_eq!(tr.at(-1.0), 0.0);
        assert_eq!(tr.at(0.5), 5.0);
        assert_eq!(tr.at(1.0), 10.0);
        assert!(tr.at(1.2) < 6.0, "ease-out drops fast at first");
        assert_eq!(tr.at(5.0), 0.0);
        assert_eq!(ease(Ease::Hold, 0.99), 0.0);
        assert!(ease(Ease::OutBack, 0.6) > 1.0, "overshoot");
        assert!((ease(Ease::Elastic, 1.0) - 1.0).abs() < 1e-6);
        let clip = Clip::new(2.0, vec![tr.clone(), tr]);
        let mut p = Vec::new();
        clip.sample_looped(2.5, &mut p);
        assert_eq!(p, vec![5.0, 5.0]);
    }

    #[test]
    fn follow_converges_and_overshoots() {
        let mut f = Follow::new(0.0, 60.0, 6.0);
        let mut peak = 0.0f32;
        for _ in 0..300 {
            let v = f.step(1.0, 1.0 / 60.0);
            peak = peak.max(v);
        }
        assert!((f.val - 1.0).abs() < 1e-2, "{}", f.val);
        assert!(peak > 1.02, "underdamped spring should overshoot: {peak}");
        // stiff spring at the fast-forward step size stays finite
        let mut s = Follow::new(0.0, 900.0, 20.0);
        for _ in 0..100 {
            s.step(1.0, 1.0 / 30.0);
        }
        assert!(s.val.is_finite() && (s.val - 1.0).abs() < 0.05, "{}", s.val);
    }

    #[test]
    fn squash_preserves_volume() {
        let (a, b) = squash(0.25);
        assert!((a * b - 1.0).abs() < 1e-6);
        let (c, d) = squash(-0.25);
        assert!(c < 1.0 && d > 1.0);
    }
}
