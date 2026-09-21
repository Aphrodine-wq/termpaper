//! Anti-aliased, alpha-blended drawing primitives.
//!
//! Every primitive is generic over [`Surface`] so the same code paints the
//! live canvas or an offscreen [`Plate`]. Coverage is computed analytically
//! (distance to segment, span overlap) so slow motion stays smooth at the
//! coarse resolutions a terminal offers: positions are `f32` everywhere and
//! nothing snaps to whole cells.
use super::Rgb;
use crate::canvas::{lerp, Canvas};

/// Something pixels can be painted onto.
pub trait Surface {
    fn size(&self) -> (usize, usize);
    /// "over" blend: dst = lerp(dst, color, a). Out of bounds is ignored.
    fn blend_px(&mut self, x: i32, y: i32, color: Rgb, a: f32);
    /// Additive light scaled by `a`. Out of bounds is ignored.
    fn add_px(&mut self, x: i32, y: i32, color: Rgb, a: f32);
    /// Multiply the existing color by `f` (darken). Out of bounds is ignored.
    fn mul_px(&mut self, x: i32, y: i32, f: f32);
}

impl Surface for Canvas {
    fn size(&self) -> (usize, usize) {
        (self.width(), self.height())
    }
    #[inline]
    fn blend_px(&mut self, x: i32, y: i32, color: Rgb, a: f32) {
        self.blend(x, y, color, a);
    }
    #[inline]
    fn add_px(&mut self, x: i32, y: i32, color: Rgb, a: f32) {
        self.add_scaled(x, y, color, a);
    }
    #[inline]
    fn mul_px(&mut self, x: i32, y: i32, f: f32) {
        self.mul(x, y, f);
    }
}

/// Offscreen RGBA layer. Paint it once (on init, on resize, or when a slow
/// parameter crosses a threshold) and composite it every frame with an
/// offset and a per-pixel tint closure — that is how static ridges, trunks,
/// ground and foreground crops cost a lerp per pixel instead of a noise field.
#[derive(Clone)]
pub struct Plate {
    w: usize,
    h: usize,
    rgb: Vec<Rgb>,
    alpha: Vec<f32>,
}

impl Plate {
    pub fn new(w: usize, h: usize) -> Self {
        Plate {
            w,
            h,
            rgb: vec![(0, 0, 0); w * h],
            alpha: vec![0.0; w * h],
        }
    }

    pub fn width(&self) -> usize {
        self.w
    }

    pub fn height(&self) -> usize {
        self.h
    }

    pub fn clear(&mut self) {
        self.rgb.fill((0, 0, 0));
        self.alpha.fill(0.0);
    }

    /// Alpha at a pixel (0 outside).
    pub fn alpha_at(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            0.0
        } else {
            self.alpha[y as usize * self.w + x as usize]
        }
    }

    /// Color at a pixel (black outside).
    pub fn color_at(&self, x: i32, y: i32) -> Rgb {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            (0, 0, 0)
        } else {
            self.rgb[y as usize * self.w + x as usize]
        }
    }

    /// First and one-past-last row holding any paint, so composites skip
    /// empty bands. `(0, 0)` when the plate is empty.
    pub fn rows_covered(&self) -> (usize, usize) {
        let mut first = None;
        let mut last = 0;
        for y in 0..self.h {
            let row = &self.alpha[y * self.w..(y + 1) * self.w];
            if row.iter().any(|&a| a > 0.0) {
                first.get_or_insert(y);
                last = y + 1;
            }
        }
        match first {
            Some(f) => (f, last),
            None => (0, 0),
        }
    }

    /// Composite onto `dst` at `(dx, dy)`; `tint(x, y, color)` receives plate
    /// coordinates and returns the lit color (light, fog, flash).
    pub fn composite<F: Fn(i32, i32, Rgb) -> Rgb>(&self, dst: &mut Canvas, dx: i32, dy: i32, tint: F) {
        let (y0, y1) = self.rows_covered();
        self.composite_rows(dst, dx, dy, y0, y1, tint);
    }

    /// Composite without a tint: alpha lerp only.
    pub fn composite_plain(&self, dst: &mut Canvas, dx: i32, dy: i32) {
        let (y0, y1) = self.rows_covered();
        self.composite_rows(dst, dx, dy, y0, y1, |_, _, c| c);
    }

    /// Composite only plate rows `y0..y1`.
    pub fn composite_rows<F: Fn(i32, i32, Rgb) -> Rgb>(
        &self,
        dst: &mut Canvas,
        dx: i32,
        dy: i32,
        y0: usize,
        y1: usize,
        tint: F,
    ) {
        let (dw, dh) = (dst.width() as i32, dst.height() as i32);
        let y1 = y1.min(self.h);
        for y in y0..y1 {
            let ty = dy + y as i32;
            if ty < 0 || ty >= dh {
                continue;
            }
            let row = y * self.w;
            // clip the x range once per row instead of per pixel
            let x0 = (-dx).max(0) as usize;
            let x1 = ((dw - dx).max(0) as usize).min(self.w);
            for x in x0..x1 {
                let a = self.alpha[row + x];
                if a <= 0.001 {
                    continue;
                }
                let c = tint(x as i32, y as i32, self.rgb[row + x]);
                dst.blend(dx + x as i32, ty, c, a);
            }
        }
    }
}

impl Surface for Plate {
    fn size(&self) -> (usize, usize) {
        (self.w, self.h)
    }
    #[inline]
    fn blend_px(&mut self, x: i32, y: i32, color: Rgb, a: f32) {
        if a <= 0.0 || x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = y as usize * self.w + x as usize;
        let a = a.min(1.0);
        let old_a = self.alpha[i];
        let new_a = a + old_a * (1.0 - a);
        if new_a <= 0.0 {
            return;
        }
        // straight-alpha "over": weight the new color by a, the old by what
        // remains of its own alpha
        let t = a / new_a;
        self.rgb[i] = lerp(self.rgb[i], color, t);
        self.alpha[i] = new_a;
    }
    #[inline]
    fn add_px(&mut self, x: i32, y: i32, color: Rgb, a: f32) {
        if a <= 0.0 || x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = y as usize * self.w + x as usize;
        let c = self.rgb[i];
        self.rgb[i] = (
            (c.0 as f32 + color.0 as f32 * a).min(255.0) as u8,
            (c.1 as f32 + color.1 as f32 * a).min(255.0) as u8,
            (c.2 as f32 + color.2 as f32 * a).min(255.0) as u8,
        );
        self.alpha[i] = (self.alpha[i] + a * (1.0 - self.alpha[i])).min(1.0);
    }
    #[inline]
    fn mul_px(&mut self, x: i32, y: i32, f: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = y as usize * self.w + x as usize;
        self.rgb[i] = crate::canvas::scale(self.rgb[i], f.max(0.0));
    }
}

/// Alpha-blend one pixel.
#[inline]
pub fn blend<S: Surface>(s: &mut S, x: i32, y: i32, color: Rgb, a: f32) {
    s.blend_px(x, y, color, a);
}

/// Bilinear 2x2 sub-cell alpha blend: the "over" twin of `canvas::dot`.
/// Weight is conserved, so a slowly moving point never flickers.
pub fn blend_f<S: Surface>(s: &mut S, x: f32, y: f32, color: Rgb, a: f32) {
    if a <= 0.0 {
        return;
    }
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (ix, iy) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - ix, fy - iy);
    let (ix, iy) = (ix as i32, iy as i32);
    s.blend_px(ix, iy, color, a * (1.0 - tx) * (1.0 - ty));
    s.blend_px(ix + 1, iy, color, a * tx * (1.0 - ty));
    s.blend_px(ix, iy + 1, color, a * (1.0 - tx) * ty);
    s.blend_px(ix + 1, iy + 1, color, a * tx * ty);
}

/// Additive 2x2 sub-cell splat (a soft point of light).
pub fn add_f<S: Surface>(s: &mut S, x: f32, y: f32, color: Rgb, a: f32) {
    if a <= 0.0 {
        return;
    }
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (ix, iy) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - ix, fy - iy);
    let (ix, iy) = (ix as i32, iy as i32);
    s.add_px(ix, iy, color, a * (1.0 - tx) * (1.0 - ty));
    s.add_px(ix + 1, iy, color, a * tx * (1.0 - ty));
    s.add_px(ix, iy + 1, color, a * (1.0 - tx) * ty);
    s.add_px(ix + 1, iy + 1, color, a * tx * ty);
}

/// Distance from `p` to segment `a`-`b`, plus the projection parameter 0..1.
#[inline]
fn seg_dist(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> (f32, f32) {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 1e-6 {
        0.0
    } else {
        (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (x0 + dx * t, y0 + dy * t);
    (((px - cx).powi(2) + (py - cy).powi(2)).sqrt(), t)
}

/// Anti-aliased thick segment with round caps. `width` in pixels.
pub fn stroke_f<S: Surface>(s: &mut S, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Rgb, a: f32) {
    stroke_taper(s, x0, y0, x1, y1, width, width, color, a);
}

/// Tapered stroke: `w0` wide at the start, `w1` at the end. Reeds, feathers,
/// brush marks, limbs.
#[allow(clippy::too_many_arguments)]
pub fn stroke_taper<S: Surface>(
    s: &mut S,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    w0: f32,
    w1: f32,
    color: Rgb,
    a: f32,
) {
    capsule_shaded(s, x0, y0, w0 * 0.5, x1, y1, w1 * 0.5, a, |_, _| color);
}

/// Limb segment: radius `r0` at one end, `r1` at the other.
#[allow(clippy::too_many_arguments)]
pub fn capsule<S: Surface>(s: &mut S, x0: f32, y0: f32, r0: f32, x1: f32, y1: f32, r1: f32, color: Rgb, a: f32) {
    capsule_shaded(s, x0, y0, r0, x1, y1, r1, a, |_, _| color);
}

/// Capsule whose color comes from `shade(u, v)`: `u` runs 0..1 along the
/// limb, `v` runs -1..1 across it (negative = left of travel). Use it for
/// three-tone cel shading and rim light on limbs.
#[allow(clippy::too_many_arguments)]
pub fn capsule_shaded<S: Surface, F: Fn(f32, f32) -> Rgb>(
    s: &mut S,
    x0: f32,
    y0: f32,
    r0: f32,
    x1: f32,
    y1: f32,
    r1: f32,
    a: f32,
    shade: F,
) {
    if a <= 0.0 {
        return;
    }
    let (sw, sh) = s.size();
    let rmax = r0.max(r1) + 1.0;
    let bx0 = (x0.min(x1) - rmax).floor().max(0.0) as i32;
    let by0 = (y0.min(y1) - rmax).floor().max(0.0) as i32;
    let bx1 = ((x0.max(x1) + rmax).ceil() as i32).min(sw as i32 - 1);
    let by1 = ((y0.max(y1) + rmax).ceil() as i32).min(sh as i32 - 1);
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt().max(1e-4);
    let (nx, ny) = (-dy / len, dx / len);
    for py in by0..=by1 {
        for px in bx0..=bx1 {
            let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
            let (d, t) = seg_dist(cx, cy, x0, y0, x1, y1);
            let r = r0 + (r1 - r0) * t;
            let cov = (r + 0.5 - d).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let side = ((cx - x0) * nx + (cy - y0) * ny) / r.max(0.1);
            let c = shade(t, side.clamp(-1.0, 1.0));
            s.blend_px(px, py, c, a * cov);
        }
    }
}

/// Filled polygon (convex or concave, even-odd), anti-aliased by four
/// sub-scanlines per row with exact horizontal span coverage.
pub fn polygon_fill<S: Surface>(s: &mut S, pts: &[(f32, f32)], color: Rgb, a: f32) {
    polygon_fill_shaded(s, pts, a, |_, _| color);
}

/// Polygon with per-pixel color from `shade(u, v)`, normalized 0..1 inside
/// the polygon's bounding box (mountain gradients, tent faces, ice slopes).
pub fn polygon_fill_shaded<S: Surface, F: Fn(f32, f32) -> Rgb>(s: &mut S, pts: &[(f32, f32)], a: f32, shade: F) {
    if pts.len() < 3 || a <= 0.0 {
        return;
    }
    let (sw, sh) = s.size();
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in pts {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }
    if !(minx.is_finite() && maxx.is_finite() && miny.is_finite() && maxy.is_finite()) {
        return;
    }
    let bw = (maxx - minx).max(1e-3);
    let bh = (maxy - miny).max(1e-3);
    let x_lo = (minx.floor() as i32).max(0);
    let x_hi = (maxx.ceil() as i32).min(sw as i32 - 1);
    let y_lo = (miny.floor() as i32).max(0);
    let y_hi = (maxy.ceil() as i32).min(sh as i32 - 1);
    if x_hi < x_lo || y_hi < y_lo {
        return;
    }
    let width = (x_hi - x_lo + 1) as usize;
    let mut cov = vec![0.0f32; width];
    let mut xs: Vec<f32> = Vec::with_capacity(pts.len());
    const SUB: usize = 4;
    for py in y_lo..=y_hi {
        cov.fill(0.0);
        let mut any = false;
        for sub in 0..SUB {
            let sy = py as f32 + (sub as f32 + 0.5) / SUB as f32;
            xs.clear();
            for i in 0..pts.len() {
                let (x0, y0) = pts[i];
                let (x1, y1) = pts[(i + 1) % pts.len()];
                if (y0 <= sy) != (y1 <= sy) {
                    let t = (sy - y0) / (y1 - y0);
                    xs.push(x0 + (x1 - x0) * t);
                }
            }
            if xs.len() < 2 {
                continue;
            }
            xs.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let (xa, xb) = (pair[0], pair[1]);
                let ia = xa.floor().max(x_lo as f32) as i32;
                let ib = xb.ceil().min(x_hi as f32 + 1.0) as i32;
                for px in ia..ib {
                    let l = xa.max(px as f32);
                    let r = xb.min(px as f32 + 1.0);
                    if r > l {
                        cov[(px - x_lo) as usize] += (r - l) / SUB as f32;
                        any = true;
                    }
                }
            }
        }
        if !any {
            continue;
        }
        let v = (py as f32 + 0.5 - miny) / bh;
        for (i, &c) in cov.iter().enumerate() {
            if c <= 0.002 {
                continue;
            }
            let px = x_lo + i as i32;
            let u = (px as f32 + 0.5 - minx) / bw;
            s.blend_px(px, py, shade(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)), a * c.min(1.0));
        }
    }
}

/// Anti-aliased filled ellipse, optionally rotated (`rot` in radians).
#[allow(clippy::too_many_arguments)]
pub fn ellipse_f<S: Surface>(s: &mut S, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, color: Rgb, a: f32) {
    ellipse_shaded(s, cx, cy, rx, ry, rot, a, |_, _| color);
}

/// Ellipse with color from `shade(u, v)` in the ellipse's own frame, both
/// -1..1 (u along the rx axis). Bodies, bellies, heads with a lit side.
#[allow(clippy::too_many_arguments)]
pub fn ellipse_shaded<S: Surface, F: Fn(f32, f32) -> Rgb>(
    s: &mut S,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rot: f32,
    a: f32,
    shade: F,
) {
    if a <= 0.0 || rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let (sw, sh) = s.size();
    let ext = rx.max(ry) + 1.0;
    let bx0 = ((cx - ext).floor() as i32).max(0);
    let by0 = ((cy - ext).floor() as i32).max(0);
    let bx1 = ((cx + ext).ceil() as i32).min(sw as i32 - 1);
    let by1 = ((cy + ext).ceil() as i32).min(sh as i32 - 1);
    let (cr, sr) = (rot.cos(), rot.sin());
    let r_eff = rx.min(ry);
    for py in by0..=by1 {
        for px in bx0..=bx1 {
            let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
            let u = (dx * cr + dy * sr) / rx;
            let v = (-dx * sr + dy * cr) / ry;
            let d = (u * u + v * v).sqrt();
            // approximate pixel distance to the rim
            let cov = ((1.0 - d) * r_eff + 0.5).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            s.blend_px(px, py, shade(u.clamp(-1.0, 1.0), v.clamp(-1.0, 1.0)), a * cov);
        }
    }
}

/// Point on a quadratic bézier.
pub fn bez2_at(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    (
        u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
        u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1,
    )
}

/// Point on a cubic bézier.
pub fn bez3_at(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    (
        u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0,
        u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1,
    )
}

fn approx_len(pts: &[(f32, f32)]) -> f32 {
    pts.windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum()
}

/// Quadratic bézier stroked as tapered segments (`w0` → `w1`).
#[allow(clippy::too_many_arguments)]
pub fn bezier2<S: Surface>(s: &mut S, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), w0: f32, w1: f32, color: Rgb, a: f32) {
    let n = ((approx_len(&[p0, p1, p2]) / 2.0).ceil() as usize).clamp(2, 64);
    let mut prev = p0;
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let p = bez2_at(p0, p1, p2, t);
        let (wa, wb) = (w0 + (w1 - w0) * (i - 1) as f32 / n as f32, w0 + (w1 - w0) * t);
        stroke_taper(s, prev.0, prev.1, p.0, p.1, wa, wb, color, a);
        prev = p;
    }
}

/// Cubic bézier stroked as tapered segments (`w0` → `w1`).
#[allow(clippy::too_many_arguments)]
pub fn bezier3<S: Surface>(
    s: &mut S,
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    w0: f32,
    w1: f32,
    color: Rgb,
    a: f32,
) {
    let n = ((approx_len(&[p0, p1, p2, p3]) / 2.0).ceil() as usize).clamp(2, 64);
    let mut prev = p0;
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let p = bez3_at(p0, p1, p2, p3, t);
        let (wa, wb) = (w0 + (w1 - w0) * (i - 1) as f32 / n as f32, w0 + (w1 - w0) * t);
        stroke_taper(s, prev.0, prev.1, p.0, p.1, wa, wb, color, a);
        prev = p;
    }
}

/// Piecewise-linear color ramp over `stops` sorted by position.
pub fn ramp(stops: &[(f32, Rgb)], t: f32) -> Rgb {
    match stops {
        [] => (0, 0, 0),
        [only] => only.1,
        _ => {
            if t <= stops[0].0 {
                return stops[0].1;
            }
            for w in stops.windows(2) {
                let (t0, c0) = w[0];
                let (t1, c1) = w[1];
                if t <= t1 {
                    let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
                    return lerp(c0, c1, f.clamp(0.0, 1.0));
                }
            }
            stops[stops.len() - 1].1
        }
    }
}

/// Vertical gradient over rows `y0..y1`, full width, `stops` over 0..1.
pub fn gradient_v(canvas: &mut Canvas, y0: i32, y1: i32, stops: &[(f32, Rgb)]) {
    let h = canvas.height() as i32;
    let (ya, yb) = (y0.max(0), y1.min(h));
    if yb <= ya {
        return;
    }
    let span = (y1 - y0 - 1).max(1) as f32;
    for y in ya..yb {
        let t = (y - y0) as f32 / span;
        canvas.fill_row(y as usize, ramp(stops, t));
    }
}

/// Additive elliptical light pool with quadratic falloff. `ry` should be
/// `rx * stage.sy` for a pool that looks round on screen.
pub fn radial_light<S: Surface>(s: &mut S, cx: f32, cy: f32, rx: f32, ry: f32, color: Rgb, intensity: f32) {
    if intensity <= 0.0 || rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let (sw, sh) = s.size();
    let bx0 = ((cx - rx).floor() as i32).max(0);
    let by0 = ((cy - ry).floor() as i32).max(0);
    let bx1 = ((cx + rx).ceil() as i32).min(sw as i32 - 1);
    let by1 = ((cy + ry).ceil() as i32).min(sh as i32 - 1);
    for py in by0..=by1 {
        let dy = (py as f32 + 0.5 - cy) / ry;
        let dy2 = dy * dy;
        if dy2 >= 1.0 {
            continue;
        }
        for px in bx0..=bx1 {
            let dx = (px as f32 + 0.5 - cx) / rx;
            let d2 = dx * dx + dy2;
            if d2 >= 1.0 {
                continue;
            }
            let f = 1.0 - d2;
            s.add_px(px, py, color, intensity * f * f);
        }
    }
}

/// Multiplicative soft elliptical shadow: darkens by up to `strength` at the
/// centre, fading to nothing at the rim.
pub fn soft_shadow<S: Surface>(s: &mut S, cx: f32, cy: f32, rx: f32, ry: f32, strength: f32) {
    if strength <= 0.0 || rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let (sw, sh) = s.size();
    let bx0 = ((cx - rx).floor() as i32).max(0);
    let by0 = ((cy - ry).floor() as i32).max(0);
    let bx1 = ((cx + rx).ceil() as i32).min(sw as i32 - 1);
    let by1 = ((cy + ry).ceil() as i32).min(sh as i32 - 1);
    for py in by0..=by1 {
        let dy = (py as f32 + 0.5 - cy) / ry;
        let dy2 = dy * dy;
        if dy2 >= 1.0 {
            continue;
        }
        for px in bx0..=bx1 {
            let dx = (px as f32 + 0.5 - cx) / rx;
            let d2 = dx * dx + dy2;
            if d2 >= 1.0 {
                continue;
            }
            let f = 1.0 - d2;
            s.mul_px(px, py, 1.0 - strength * f * f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(p: &Plate) -> f32 {
        p.alpha.iter().sum()
    }

    #[test]
    fn polygon_coverage_matches_area() {
        // a 10x6 axis-aligned rectangle covers 60 px exactly, and a
        // half-pixel-offset one covers the same area split across edges
        let mut p = Plate::new(32, 32);
        polygon_fill(&mut p, &[(2.0, 2.0), (12.0, 2.0), (12.0, 8.0), (2.0, 8.0)], (255, 255, 255), 1.0);
        assert!((coverage(&p) - 60.0).abs() < 0.05, "{}", coverage(&p));
        let mut q = Plate::new(32, 32);
        polygon_fill(&mut q, &[(2.5, 2.5), (12.5, 2.5), (12.5, 8.5), (2.5, 8.5)], (255, 255, 255), 1.0);
        assert!((coverage(&q) - 60.0).abs() < 0.6, "{}", coverage(&q));
        // a triangle: half the box
        let mut t = Plate::new(32, 32);
        polygon_fill(&mut t, &[(0.0, 0.0), (20.0, 0.0), (0.0, 20.0)], (255, 255, 255), 1.0);
        assert!((coverage(&t) - 200.0).abs() < 3.0, "{}", coverage(&t));
    }

    #[test]
    fn stroke_is_symmetric_and_sized() {
        let mut p = Plate::new(40, 40);
        stroke_f(&mut p, 5.0, 20.0, 35.0, 20.0, 4.0, (200, 100, 50), 1.0);
        // rows equidistant from the centre line get equal coverage
        for x in 8..32 {
            let above = p.alpha_at(x, 17);
            let below = p.alpha_at(x, 22);
            assert!((above - below).abs() < 1e-3, "x={x} {above} {below}");
            assert!(p.alpha_at(x, 19) > 0.99 && p.alpha_at(x, 20) > 0.99);
            assert_eq!(p.alpha_at(x, 14), 0.0);
        }
        // total coverage ≈ length * width + round caps
        let area = coverage(&p);
        let expect = 30.0 * 4.0 + std::f32::consts::PI * 4.0;
        assert!((area - expect).abs() < expect * 0.1, "{area} vs {expect}");
    }

    #[test]
    fn blend_f_conserves_weight() {
        let mut p = Plate::new(8, 8);
        blend_f(&mut p, 3.3, 4.7, (255, 255, 255), 1.0);
        assert!((coverage(&p) - 1.0).abs() < 1e-4);
        let mut q = Plate::new(8, 8);
        blend_f(&mut q, 3.5, 4.5, (255, 255, 255), 0.5);
        assert!((q.alpha_at(3, 4) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn plate_composite_round_trips() {
        let mut p = Plate::new(6, 4);
        blend(&mut p, 2, 1, (10, 20, 30), 1.0);
        blend(&mut p, 3, 1, (100, 100, 100), 0.5);
        let mut c = Canvas::new(6, 4);
        c.clear((0, 0, 0));
        p.composite_plain(&mut c, 0, 0);
        assert_eq!(c.get(2, 1).color, (10, 20, 30));
        assert_eq!(c.get(3, 1).color, (50, 50, 50));
        assert_eq!(c.get(0, 0).color, (0, 0, 0));
        assert_eq!(p.rows_covered(), (1, 2));
        // offset composite clips instead of wrapping
        let mut d = Canvas::new(6, 4);
        p.composite(&mut d, -2, 0, |_, _, c| c);
        assert_eq!(d.get(0, 1).color, (10, 20, 30));
        assert_eq!(d.get(4, 1).color, (0, 0, 0));
    }

    #[test]
    fn ellipse_and_lights_stay_inside_their_bounds() {
        let mut p = Plate::new(40, 40);
        ellipse_f(&mut p, 20.0, 20.0, 10.0, 5.0, 0.0, (255, 255, 255), 1.0);
        let area = coverage(&p);
        let expect = std::f32::consts::PI * 10.0 * 5.0;
        assert!((area - expect).abs() < expect * 0.08, "{area} vs {expect}");
        assert_eq!(p.alpha_at(20, 13), 0.0);
        assert!(p.alpha_at(20, 20) > 0.99);
        let mut c = Canvas::new(40, 40);
        c.clear((100, 100, 100));
        radial_light(&mut c, 20.0, 20.0, 8.0, 8.0, (255, 0, 0), 1.0);
        assert!(c.get(20, 20).color.0 > 200);
        assert_eq!(c.get(20, 5).color, (100, 100, 100));
        soft_shadow(&mut c, 20.0, 20.0, 8.0, 8.0, 0.5);
        assert!(c.get(20, 20).color.1 < 60);
        assert_eq!(c.get(20, 5).color, (100, 100, 100));
    }

    #[test]
    fn ramp_and_gradient() {
        let stops = [(0.0, (0, 0, 0)), (0.5, (100, 100, 100)), (1.0, (200, 0, 0))];
        assert_eq!(ramp(&stops, -1.0), (0, 0, 0));
        assert_eq!(ramp(&stops, 0.25), (50, 50, 50));
        assert_eq!(ramp(&stops, 2.0), (200, 0, 0));
        let mut c = Canvas::new(4, 10);
        gradient_v(&mut c, 0, 10, &stops);
        assert_eq!(c.get(0, 0).color, (0, 0, 0));
        assert_eq!(c.get(3, 9).color, (200, 0, 0));
        assert!(c.get(1, 4).color.0 > 60);
    }

    #[test]
    fn bezier_touches_its_endpoints() {
        let mut p = Plate::new(40, 40);
        bezier2(&mut p, (2.0, 30.0), (20.0, -10.0), (38.0, 30.0), 2.0, 2.0, (255, 255, 255), 1.0);
        assert!(p.alpha_at(2, 29) > 0.3);
        assert!(p.alpha_at(37, 29) > 0.3);
        assert!(p.alpha_at(20, 10) > 0.3, "apex");
        assert_eq!(p.alpha_at(20, 30), 0.0);
    }
}
