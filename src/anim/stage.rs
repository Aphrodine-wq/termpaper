//! Composition helpers: the stage (canvas size, pixel aspect, orientation),
//! a slowly drifting camera and parallax layers built on [`Plate`]s.
use super::draw::Plate;
use crate::canvas::Canvas;
use crate::render::Pixels;

/// What a scene needs to know about the canvas it is composing for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub w: usize,
    pub h: usize,
    pub wf: f32,
    pub hf: f32,
    /// multiply a y-extent by this to make it visually equal to an x-extent
    /// (0.5 for quad pixels, 1.0 for half/braille)
    pub sy: f32,
    /// on-screen width / height; `< 1` is taller than wide
    pub aspect: f32,
    pub portrait: bool,
}

impl Stage {
    pub fn new(w: usize, h: usize, pixels: Pixels) -> Self {
        let (wf, hf) = (w as f32, h as f32);
        let sy = 1.0 / pixels.aspect();
        let screen_h = hf / sy.max(1e-3);
        let aspect = if screen_h > 0.0 { wf / screen_h } else { 1.0 };
        Stage {
            w,
            h,
            wf,
            hf,
            sy,
            aspect,
            portrait: aspect < 0.9,
        }
    }

    pub fn of(canvas: &Canvas, pixels: Pixels) -> Self {
        Stage::new(canvas.width(), canvas.height(), pixels)
    }

    /// Canvas height expressed in x-pixel (screen) units.
    pub fn screen_h(&self) -> f32 {
        self.hf / self.sy.max(1e-3)
    }

    /// A hero size that reads the same in both orientations: `k` times the
    /// shorter on-screen edge, in x-pixel units.
    pub fn hero(&self, k: f32) -> f32 {
        self.wf.min(self.screen_h()) * k
    }

    /// Horizon row: `land_frac` of the height in landscape, `port_frac` in portrait.
    pub fn horizon(&self, land_frac: f32, port_frac: f32) -> f32 {
        self.hf * if self.portrait { port_frac } else { land_frac }
    }

    /// Pick a value by orientation.
    pub fn pick<T>(&self, landscape: T, portrait: T) -> T {
        if self.portrait {
            portrait
        } else {
            landscape
        }
    }

    /// y-radius in canvas px for a circle of screen radius `r`.
    pub fn ry(&self, r: f32) -> f32 {
        r * self.sy
    }

    /// Whether the canvas is too small to carry rigs; scenes still paint
    /// sky and ground but skip characters.
    pub fn tiny(&self) -> bool {
        self.w < 48 || self.h < 20
    }
}

/// A camera that drifts on a lissajous figure — no RNG, so it stays in sync
/// across linked instances.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub amp: (f32, f32),
    /// periods in seconds
    pub period: (f32, f32),
    pub phase: f32,
}

impl Camera {
    pub fn new(amp: (f32, f32), period: (f32, f32), phase: f32) -> Self {
        Camera {
            x: 0.0,
            y: 0.0,
            amp,
            period,
            phase,
        }
    }

    pub fn step(&mut self, t: f32) {
        let tau = std::f32::consts::TAU;
        self.x = self.amp.0 * (t * tau / self.period.0.max(0.01) + self.phase).sin();
        self.y = self.amp.1 * (t * tau / self.period.1.max(0.01) + self.phase * 0.7).cos();
    }
}

/// A plate that scrolls with the camera by its depth. Built `pad` px wider
/// than the canvas on each side so drift never exposes an edge.
pub struct ParallaxLayer {
    /// 0 = infinitely far (static), 1 = nearest (full camera motion)
    pub depth: f32,
    pub plate: Plate,
    pub pad: i32,
}

impl ParallaxLayer {
    pub fn new(depth: f32, plate: Plate, pad: i32) -> Self {
        ParallaxLayer { depth, plate, pad }
    }

    /// Canvas offset for this layer under the camera.
    pub fn offset(&self, cam: &Camera, k: f32) -> (i32, i32) {
        let ox = -self.pad - (cam.x * self.depth * k).round() as i32;
        let oy = -(cam.y * self.depth * k * 0.5).round() as i32;
        (ox, oy)
    }

    pub fn composite<F: Fn(i32, i32, super::Rgb) -> super::Rgb>(&self, dst: &mut Canvas, cam: &Camera, k: f32, tint: F) {
        let (ox, oy) = self.offset(cam, k);
        self.plate.composite(dst, ox, oy, tint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_portrait_detection_uses_pixel_aspect() {
        let p = Stage::new(270, 548, Pixels::Half);
        assert!(p.portrait);
        assert_eq!(p.sy, 1.0);
        let l = Stage::new(200, 100, Pixels::Quad);
        assert!(!l.portrait);
        assert_eq!(l.sy, 0.5);
        assert!((l.aspect - 1.0).abs() < 1e-5);
        let q = Stage::new(100, 200, Pixels::Quad);
        assert!(q.portrait);
        assert!((q.aspect - 0.25).abs() < 1e-5);
        let wide = Stage::new(384, 102, Pixels::Quad);
        assert!(!wide.portrait);
        assert!((wide.hero(0.5) - 102.0).abs() < 1e-3);
        assert_eq!(wide.horizon(0.6, 0.5), 102.0 * 0.6);
        assert_eq!(p.horizon(0.6, 0.5), 548.0 * 0.5);
        assert!(Stage::new(40, 12, Pixels::Quad).tiny());
    }

    #[test]
    fn camera_and_parallax_offsets() {
        let mut cam = Camera::new((4.0, 2.0), (10.0, 14.0), 0.0);
        cam.step(2.5);
        assert!((cam.x - 4.0).abs() < 1e-4);
        let far = ParallaxLayer::new(0.0, Plate::new(10, 4), 6);
        let near = ParallaxLayer::new(1.0, Plate::new(10, 4), 6);
        assert_eq!(far.offset(&cam, 1.0), (-6, 0));
        assert_eq!(near.offset(&cam, 1.0).0, -10);
    }
}
