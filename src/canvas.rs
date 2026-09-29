//! Pixel canvas: a grid of truecolor cells, optionally carrying a glyph.
//! Rendered to the terminal with the half-block technique (see `render.rs`).

/// A single canvas cell: an RGB color plus an optional character.
///
/// When `ch` is `Some`, the cell is drawn as that glyph with `color` as the
/// foreground instead of as a half-block pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub color: (u8, u8, u8),
    pub ch: Option<char>,
}

impl Cell {
    pub const BLACK: Cell = Cell {
        color: (0, 0, 0),
        ch: None,
    };
}

#[derive(Clone)]
pub struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
    /// cells written since the last reset (coverage auditing)
    touched: Vec<bool>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Canvas {
            width,
            height,
            cells: vec![Cell::BLACK; width * height],
            touched: vec![false; width * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Resize the canvas, clearing all contents.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.cells = vec![Cell::BLACK; width * height];
        self.touched = vec![false; width * height];
    }

    /// Was this cell ever written since the last resize?
    #[cfg(test)]
    pub fn touched(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
            && self.touched[y as usize * self.width + x as usize]
    }

    /// Fill the whole canvas with a solid color, dropping any glyphs.
    pub fn clear(&mut self, color: (u8, u8, u8)) {
        self.cells.fill(Cell { color, ch: None });
        self.touched.fill(true);
    }

    /// Fill a horizontal span within one row.
    pub fn fill_span(&mut self, x: i32, y: i32, len: i32, color: (u8, u8, u8)) {
        for dx in 0..len {
            self.set(x + dx, y, color);
        }
    }

    /// Fill one row with a solid color.
    pub fn fill_row(&mut self, y: usize, color: (u8, u8, u8)) {
        if y < self.height {
            for x in 0..self.width {
                self.cells[y * self.width + x] = Cell { color, ch: None };
                self.touched[y * self.width + x] = true;
            }
        }
    }

    /// Set a pixel. Out-of-bounds writes are ignored.
    pub fn set(&mut self, x: i32, y: i32, color: (u8, u8, u8)) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = y as usize * self.width + x as usize;
            self.cells[i] = Cell { color, ch: None };
            self.touched[i] = true;
        }
    }

    /// Alpha-blend a pixel toward `color` ("over"). Out-of-bounds writes are
    /// ignored; the glyph is dropped. The shared painterly primitive behind
    /// `anim::draw` — every brush, wash and soft edge ends up here.
    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, color: (u8, u8, u8), a: f32) {
        if a <= 0.0 {
            return;
        }
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = y as usize * self.width + x as usize;
            let c = &mut self.cells[i];
            c.color = if a >= 1.0 { color } else { lerp(c.color, color, a) };
            c.ch = None;
            self.touched[i] = true;
        }
    }

    /// Additive light scaled by `a` (saturating). Out-of-bounds ignored.
    #[inline]
    pub fn add_scaled(&mut self, x: i32, y: i32, color: (u8, u8, u8), a: f32) {
        if a <= 0.0 {
            return;
        }
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = y as usize * self.width + x as usize;
            let c = &mut self.cells[i];
            let a = a.min(4.0);
            c.color = (
                (c.color.0 as f32 + color.0 as f32 * a).min(255.0) as u8,
                (c.color.1 as f32 + color.1 as f32 * a).min(255.0) as u8,
                (c.color.2 as f32 + color.2 as f32 * a).min(255.0) as u8,
            );
            self.touched[i] = true;
        }
    }

    /// Multiply a pixel's color by `f` (shadows, darkening). Out-of-bounds ignored.
    #[inline]
    pub fn mul(&mut self, x: i32, y: i32, f: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = y as usize * self.width + x as usize;
            let c = &mut self.cells[i];
            c.color = scale(c.color, f.max(0.0));
            self.touched[i] = true;
        }
    }

    /// Set a glyph cell. Out-of-bounds writes are ignored.
    pub fn set_char(&mut self, x: i32, y: i32, ch: char, color: (u8, u8, u8)) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = y as usize * self.width + x as usize;
            self.cells[i] = Cell {
                color,
                ch: Some(ch),
            };
            self.touched[i] = true;
        }
    }

    /// Set a pixel using float coordinates (truncated).
    pub fn set_f(&mut self, x: f32, y: f32, color: (u8, u8, u8)) {
        self.set(x as i32, y as i32, color);
    }

    /// Read a cell. Out-of-bounds reads return black.
    pub fn get(&self, x: i32, y: i32) -> Cell {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.cells[y as usize * self.width + x as usize]
        } else {
            Cell::BLACK
        }
    }

    /// Raw cell slice, row-major. The GPU backend packs straight from this
    /// rather than going through `get()` — at 70k pixels a frame the per-pixel
    /// bounds check is the whole cost of the upload.
    pub fn cells_raw(&self) -> &[Cell] {
        &self.cells
    }

    /// Mutable raw cell slice, for writing a GPU readback back in place.
    pub fn cells_raw_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// Shallow clone used by the temporal smoothing step.
    pub fn clone_for_smooth(&self) -> Canvas {
        Canvas {
            width: self.width,
            height: self.height,
            cells: self.cells.clone(),
            touched: self.touched.clone(),
        }
    }

    /// Snapshot into an existing canvas, reusing its buffers.
    ///
    /// Same result as `clone_for_smooth` but with no allocation, so the
    /// per-frame smoothing snapshot stops churning the allocator.
    pub fn snapshot_into(&self, dst: &mut Canvas) {
        if dst.width != self.width || dst.height != self.height {
            dst.width = self.width;
            dst.height = self.height;
            dst.cells.resize(self.cells.len(), Cell::BLACK);
            dst.touched.resize(self.touched.len(), false);
        }
        dst.cells.copy_from_slice(&self.cells);
        dst.touched.copy_from_slice(&self.touched);
    }

    /// Additive radial glow, clipped to the canvas once.
    ///
    /// This is the hot primitive of the whole engine: nearly every scene draws
    /// a particle as one glow plus a core pixel, so glow throughput *is* the
    /// particle ceiling. `add()` per pixel costs a manual coordinate check plus
    /// two separate `Vec` bounds checks (it indexes `touched` and `cells`
    /// independently), and at r=3 that bookkeeping was ~87% of the call.
    ///
    /// Clipping the disc to the canvas up front makes every write provably
    /// in-bounds, so the inner loop walks row sub-slices by iterator and pays
    /// no per-pixel checks. Output is bit-identical to the old `add()` loop —
    /// the falloff keeps its original division rather than a hoisted
    /// reciprocal, which would differ in the last bit.
    pub fn add_glow(&mut self, x: i32, y: i32, r: i32, color: (u8, u8, u8), intensity: f32) {
        if r <= 0 || self.width == 0 || self.height == 0 {
            return;
        }
        let (w, h) = (self.width as i32, self.height as i32);
        let y0 = (y - r).max(0);
        let y1 = (y + r).min(h - 1);
        let x0 = (x - r).max(0);
        let x1 = (x + r).min(w - 1);
        if y0 > y1 || x0 > x1 {
            return;
        }
        let r2 = r * r;
        let denom = (r2 + 1) as f32;
        let (cr, cg, cb) = (color.0 as f32, color.1 as f32, color.2 as f32);
        let (lo, hi) = (x0 as usize, x1 as usize);

        // The falloff depends only on d2, so the *whole scaled colour* does
        // too: precompute it once per call and the inner loop keeps no float
        // math at all. Entries use the same arithmetic as the per-pixel form,
        // so this stays bit-identical. r2+1 entries is fewer than the pixels in
        // the disc for every radius, so it is a win even at r=1.
        const MAX_LUT_R2: usize = 256; // r <= 16
        let mut lut = [(0u8, 0u8, 0u8); MAX_LUT_R2 + 1];
        let lut_ok = (r2 as usize) <= MAX_LUT_R2;
        if lut_ok {
            for (d2, e) in lut.iter_mut().enumerate().take(r2 as usize + 1) {
                let f = 1.0 - d2 as f32 / denom;
                let k = intensity * f * f;
                *e = (
                    (cr * k).clamp(0.0, 255.0) as u8,
                    (cg * k).clamp(0.0, 255.0) as u8,
                    (cb * k).clamp(0.0, 255.0) as u8,
                );
            }
        }

        for py in y0..=y1 {
            let dy = py - y;
            let dy2 = dy * dy;
            if dy2 > r2 {
                continue;
            }
            let row = py as usize * self.width;
            let cells = &mut self.cells[row + lo..=row + hi];
            let touched = &mut self.touched[row + lo..=row + hi];
            for (i, (cell, seen)) in cells.iter_mut().zip(touched.iter_mut()).enumerate() {
                let dx = (lo + i) as i32 - x;
                let d2 = dx * dx + dy2;
                if d2 > r2 {
                    continue;
                }
                let add = if lut_ok {
                    lut[d2 as usize]
                } else {
                    // radius beyond the table: same math, computed inline
                    let f = 1.0 - d2 as f32 / denom;
                    let k = intensity * f * f;
                    (
                        (cr * k).clamp(0.0, 255.0) as u8,
                        (cg * k).clamp(0.0, 255.0) as u8,
                        (cb * k).clamp(0.0, 255.0) as u8,
                    )
                };
                let c = &mut cell.color;
                c.0 = c.0.saturating_add(add.0);
                c.1 = c.1.saturating_add(add.1);
                c.2 = c.2.saturating_add(add.2);
                *seen = true;
            }
        }
    }

    /// Apply a per-pixel color transform across the whole canvas.
    ///
    /// Semantics match `set()` — the glyph is cleared and the cell marked
    /// touched — but the bounds check is gone and the `touched` bookkeeping is
    /// paid once for the buffer instead of once per pixel. Whole-canvas passes
    /// (filters, color grading) are memory-bandwidth bound, so avoiding the
    /// per-pixel `get`/`set` round trip is most of their cost.
    pub fn map_colors(&mut self, mut f: impl FnMut((u8, u8, u8)) -> (u8, u8, u8)) {
        for c in self.cells.iter_mut() {
            c.color = f(c.color);
            c.ch = None;
        }
        self.touched.fill(true);
    }

    /// Temporal smoothing: blend this canvas toward `prev` by `a`
    /// (cur = prev*(1-a) + cur*a). Glyph cells are never blended.
    pub fn smooth_blend(&mut self, prev: &Canvas, a: f32) {
        if a >= 0.999 || self.width != prev.width || self.height != prev.height {
            return;
        }
        let a = a.clamp(0.0, 1.0);
        for i in 0..self.cells.len() {
            if self.cells[i].ch.is_some() {
                continue; // text stays crisp
            }
            let (c, p) = (self.cells[i].color, prev.cells[i].color);
            self.cells[i].color = (
                (p.0 as f32 * (1.0 - a) + c.0 as f32 * a) as u8,
                (p.1 as f32 * (1.0 - a) + c.1 as f32 * a) as u8,
                (p.2 as f32 * (1.0 - a) + c.2 as f32 * a) as u8,
            );
        }
    }

    /// Additively blend a color onto a pixel (used for glows, splashes).
    pub fn add(&mut self, x: i32, y: i32, color: (u8, u8, u8)) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.touched[y as usize * self.width + x as usize] = true;
            let c = &mut self.cells[y as usize * self.width + x as usize];
            c.color = (
                c.color.0.saturating_add(color.0),
                c.color.1.saturating_add(color.1),
                c.color.2.saturating_add(color.2),
            );
        }
    }
}

/// Scale an RGB color by a 0.0..=1.0-ish factor.
pub fn scale(color: (u8, u8, u8), f: f32) -> (u8, u8, u8) {
    (
        (color.0 as f32 * f).clamp(0.0, 255.0) as u8,
        (color.1 as f32 * f).clamp(0.0, 255.0) as u8,
        (color.2 as f32 * f).clamp(0.0, 255.0) as u8,
    )
}

/// Linear interpolation between two RGB colors, `t` in 0.0..=1.0.
pub fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
    )
}

/// Smoothstep easing 0→1.
pub fn ease_smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ease-out cubic: fast start, gentle settle.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Filled disc with a coverage-antialiased edge. `r` may be fractional.
///
/// Cells fully inside the radius are written opaquely with `set`; the rim gets
/// an additive contribution weighted by how much of the cell the disc covers,
/// which is how the scenes already composite light onto dark backgrounds. The
/// result is a round edge instead of the stair-stepped one a `d2 <= r2` test
/// produces at the 1-4px radii the scenes actually use.
pub fn disc(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, color: (u8, u8, u8)) {
    if r <= 0.0 {
        return;
    }
    let x0 = (cx - r - 1.0).floor() as i32;
    let x1 = (cx + r + 1.0).ceil() as i32;
    let y0 = (cy - r - 1.0).floor() as i32;
    let y1 = (cy + r + 1.0).ceil() as i32;
    for y in y0..=y1 {
        let dy = y as f32 - cy;
        for x in x0..=x1 {
            let dx = x as f32 - cx;
            // signed distance from the edge, in cells: >= 0.5 fully inside
            let cov = (r + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            if cov >= 0.999 {
                canvas.set(x, y, color);
            } else if cov > 0.0 {
                canvas.add(x, y, scale(color, cov));
            }
        }
    }
}

/// Bounded radial glow: additive, quadratic falloff, no sqrt in the loop.
/// `r` is the radius in pixels (kept small — this is the perf-safe glow).
///
/// Thin wrapper over [`Canvas::add_glow`], which does the clipping; kept as a
/// free function because every scene calls it this way.
pub fn glow(canvas: &mut Canvas, x: i32, y: i32, r: i32, color: (u8, u8, u8), intensity: f32) {
    canvas.add_glow(x, y, r, color, intensity);
}

/// Additive subcell point: a bilinear 2x2 splat at a fractional position.
///
/// Same coordinate convention as [`disc`]: an integer position lands the
/// whole splat on that cell; fractional positions spread it across the
/// 2x2 neighborhood by coverage. This is what makes slow motion glide —
/// an object moving 0.2 cells/frame shifts weight smoothly between cells
/// instead of popping a whole cell every fifth frame.
pub fn dot(canvas: &mut Canvas, x: f32, y: f32, color: (u8, u8, u8), alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let (xf, yf) = (x.floor(), y.floor());
    let (fx, fy) = (x - xf, y - yf);
    let (xi, yi) = (xf as i32, yf as i32);
    let weights = [
        (xi, yi, (1.0 - fx) * (1.0 - fy)),
        (xi + 1, yi, fx * (1.0 - fy)),
        (xi, yi + 1, (1.0 - fx) * fy),
        (xi + 1, yi + 1, fx * fy),
    ];
    for (px, py, w) in weights {
        if w > 1.0 / 512.0 {
            canvas.add(px, py, scale(color, alpha * w));
        }
    }
}

/// [`glow`] with a fractional center: the falloff is computed from the true
/// float distance, so a drifting glow slides instead of snapping cell to
/// cell. Same geometry as [`Canvas::add_glow`] at integer centers.
pub fn glow_f(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, color: (u8, u8, u8), intensity: f32) {
    if r <= 0.0 || canvas.width() == 0 || canvas.height() == 0 {
        return;
    }
    let (w, h) = (canvas.width() as i32, canvas.height() as i32);
    let x0 = ((cx - r).floor() as i32).max(0);
    let x1 = ((cx + r).ceil() as i32).min(w - 1);
    let y0 = ((cy - r).floor() as i32).max(0);
    let y1 = ((cy + r).ceil() as i32).min(h - 1);
    let r2 = r * r;
    let denom = r2 + 1.0;
    for py in y0..=y1 {
        let dy = py as f32 - cy;
        let dy2 = dy * dy;
        if dy2 > r2 {
            continue;
        }
        for px in x0..=x1 {
            let dx = px as f32 - cx;
            let d2 = dx * dx + dy2;
            if d2 > r2 {
                continue;
            }
            let f = 1.0 - d2 / denom;
            canvas.add(px, py, scale(color, intensity * f * f));
        }
    }
}

/// Anti-aliased line segment: subcell samples every ~0.7 cells, splatted
/// through [`dot`]. Additive, so bright trails over dark backgrounds.
pub fn line_f(
    canvas: &mut Canvas,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: (u8, u8, u8),
    alpha: f32,
) {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt();
    let n = (len / 0.7).ceil().max(1.0) as usize;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        dot(canvas, x0 + dx * t, y0 + dy * t, color, alpha);
    }
}

/// Axis-aligned rectangle at a fractional origin: opaque interior,
/// coverage-antialiased edges.
///
/// Region convention (unlike the point convention of [`dot`]/[`disc`]):
/// the rect spans `[x, x+w) x [y, y+h)` in the same space where cell `i`
/// covers `[i, i+1)` — so integer origin and size fill exactly the cells
/// `x as i32 ..` like an integer fill would, and fractional origins slide
/// the whole shape smoothly.
pub fn rect_f(canvas: &mut Canvas, x: f32, y: f32, w: f32, h: f32, color: (u8, u8, u8)) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let px0 = x.floor() as i32;
    let px1 = (x + w - 1e-4).floor() as i32;
    let py0 = y.floor() as i32;
    let py1 = (y + h - 1e-4).floor() as i32;
    for py in py0..=py1 {
        let cov_y = ((y + h).min(py as f32 + 1.0) - y.max(py as f32)).clamp(0.0, 1.0);
        for px in px0..=px1 {
            let cov_x = ((x + w).min(px as f32 + 1.0) - x.max(px as f32)).clamp(0.0, 1.0);
            let cov = cov_x * cov_y;
            if cov >= 0.999 {
                canvas.set(px, py, color);
            } else if cov > 0.0 {
                canvas.add(px, py, scale(color, cov));
            }
        }
    }
}

/// Framerate-independent exponential approach toward a target:
/// `approach(cur, target, rate, dt)` moves the same fraction of the
/// remaining distance per unit *time* regardless of step size, unlike the
/// hand-rolled `cur += (target - cur) * rate * dt`, which overshoots on
/// catch-up steps and lags at high fps.
pub fn approach(cur: f32, target: f32, rate: f32, dt: f32) -> f32 {
    cur + (target - cur) * (1.0 - (-rate * dt).exp())
}

/// Cubic ease-in-out: gentle at both ends.
#[allow(dead_code)] // utility kept for scene polish
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Ease-out with a slight overshoot — for pop-in accents.
#[allow(dead_code)] // utility kept for scene polish
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
}

/// Dim the whole canvas by a factor (scene fades/transitions).
pub fn dim(canvas: &mut Canvas, f: f32) {
    if f >= 0.999 {
        return;
    }
    let (w, h) = (canvas.width(), canvas.height());
    for y in 0..h {
        for x in 0..w {
            let c = canvas.get(x as i32, y as i32).color;
            canvas.set(x as i32, y as i32, scale(c, f));
        }
    }
}

/// [`dim`], shaped by a transition mask: each pixel is scaled by `f` times
/// how much of it the mask shows. The canvas is the whole wall.
pub fn dim_masked(canvas: &mut Canvas, f: f32, mask: Option<&crate::transition::Mask>) {
    let Some(m) = mask else {
        return dim(canvas, f);
    };
    let (w, h) = (canvas.width(), canvas.height());
    for y in 0..h {
        for x in 0..w {
            let c = canvas.get(x as i32, y as i32).color;
            canvas.set(x as i32, y as i32, scale(c, f * m.factor(x as i32, y as i32)));
        }
    }
}

/// Area-based density multiplier: 1.0 at 160x100, clamped 0.5..4.0.
/// Stacks with the Detail multiplier so scenes fill any terminal size.
pub fn density_for(w: usize, h: usize) -> f32 {
    ((w * h) as f32 / 16000.0).clamp(0.5, 4.0)
}

/// HSV → RGB. `h` in 0.0..1.0 (wraps), `s`/`v` in 0.0..=1.0.
pub fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - h.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (
        (r * 255.0).clamp(0.0, 255.0) as u8,
        (g * 255.0).clamp(0.0, 255.0) as u8,
        (b * 255.0).clamp(0.0, 255.0) as u8,
    )
}

/// Map a truecolor RGB value to the closest xterm-256 palette index.
///
/// Exact grays use the 24-step grayscale ramp (232..=255, with the cube's
/// black/white corners 16/231 as endpoints); everything else uses the
/// 6x6x6 color cube starting at index 16.
pub fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        if r < 8 {
            16 // black: cube corner beats ramp entry 232
        } else if r > 248 {
            231 // white: cube corner beats ramp end 255
        } else {
            // ramp entries are 8 + 10*n for n in 0..24
            (232 + ((r as u16 - 8) * 24 + 123) / 247) as u8
        }
    } else {
        let q = |v: u8| ((v as u16 * 5 + 127) / 255) as u8;
        16 + 36 * q(r) + 6 * q(g) + q(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_black_white_red() {
        assert_eq!(rgb_to_256(0, 0, 0), 16);
        assert_eq!(rgb_to_256(255, 255, 255), 231);
        assert_eq!(rgb_to_256(255, 0, 0), 196);
    }

    #[test]
    fn quantize_cube_corners_and_gray() {
        assert_eq!(rgb_to_256(0, 255, 0), 46); // pure green
        assert_eq!(rgb_to_256(0, 0, 255), 21); // pure blue
        let gray = rgb_to_256(128, 128, 128);
        assert!((232..=255).contains(&gray));
    }

    #[test]
    fn canvas_set_get_clear() {
        let mut c = Canvas::new(4, 3);
        c.set(1, 2, (10, 20, 30));
        assert_eq!(c.get(1, 2).color, (10, 20, 30));
        assert_eq!(c.get(1, 2).ch, None);

        c.set_char(0, 0, 'x', (1, 2, 3));
        assert_eq!(c.get(0, 0).ch, Some('x'));

        // out of bounds is ignored / reads black
        c.set(-1, 99, (9, 9, 9));
        assert_eq!(c.get(-1, 99), Cell::BLACK);

        c.clear((5, 5, 5));
        assert_eq!(c.get(1, 2), Cell { color: (5, 5, 5), ch: None });
        assert_eq!(c.get(0, 0).ch, None);
    }

    #[test]
    fn canvas_resize_clears() {
        let mut c = Canvas::new(2, 2);
        c.set(0, 0, (7, 7, 7));
        c.resize(3, 1);
        assert_eq!(c.width(), 3);
        assert_eq!(c.height(), 1);
        assert_eq!(c.get(0, 0), Cell::BLACK);
    }
}

#[cfg(test)]
mod polish_tests {
    use super::*;

    #[test]
    fn easing_endpoints_and_shape() {
        assert_eq!(ease_smooth(0.0), 0.0);
        assert_eq!(ease_smooth(1.0), 1.0);
        assert!(ease_smooth(0.5) == 0.5);
        assert!(ease_out(0.2) > 0.2, "ease-out runs ahead early");
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_smooth(-1.0), 0.0);
        assert_eq!(ease_smooth(2.0), 1.0);
    }

    #[test]
    fn glow_is_bounded_and_brightest_at_center() {
        let mut c = Canvas::new(20, 20);
        c.clear((0, 0, 0));
        glow(&mut c, 10, 10, 3, (100, 100, 100), 1.0);
        let center = c.get(10, 10).color.0;
        let edge = c.get(13, 10).color.0;
        let outside = c.get(15, 10).color.0;
        assert!(center > edge, "center {center} > edge {edge}");
        assert!(edge > 0);
        assert_eq!(outside, 0, "glow must be bounded by radius");
    }

    #[test]
    fn dim_scales_luminance() {
        let mut c = Canvas::new(2, 2);
        c.clear((100, 50, 200));
        dim(&mut c, 0.5);
        assert_eq!(c.get(0, 0).color, (50, 25, 100));
        dim(&mut c, 1.0);
        assert_eq!(c.get(0, 0).color, (50, 25, 100));
    }
}

#[cfg(test)]
mod smooth_tests {
    use super::*;

    #[test]
    fn constant_input_converges() {
        let mut prev = Canvas::new(4, 4);
        prev.clear((0, 0, 0));
        let mut cur = Canvas::new(4, 4);
        cur.clear((100, 100, 100));
        for _ in 0..60 {
            let snapshot = cur.clone_for_smooth();
            cur.clear((100, 100, 100));
            cur.smooth_blend(&snapshot, 0.3);
        }
        assert_eq!(cur.get(0, 0).color, (100, 100, 100));
    }

    #[test]
    fn step_input_follows_curve() {
        // first frame after a step: prev*0.7 + new*0.3
        let mut prev = Canvas::new(2, 2);
        prev.clear((0, 0, 0));
        let mut cur = Canvas::new(2, 2);
        cur.clear((100, 100, 100));
        cur.smooth_blend(&prev, 0.3);
        assert_eq!(cur.get(0, 0).color, (30, 30, 30));
        // second frame: 30*0.7 + 100*0.3 = 51
        let snapshot = cur.clone_for_smooth();
        cur.clear((100, 100, 100));
        cur.smooth_blend(&snapshot, 0.3);
        assert_eq!(cur.get(0, 0).color, (51, 51, 51));
    }

    #[test]
    fn disc_is_opaque_inside_and_soft_at_the_rim() {
        let mut c = Canvas::new(21, 21);
        c.clear((0, 0, 0));
        disc(&mut c, 10.0, 10.0, 4.0, (200, 200, 200));
        // center is fully covered
        assert_eq!(c.get(10, 10).color, (200, 200, 200));
        // a cell straddling the edge is partial: lit, but not full value
        let rim = c.get(14, 10).color;
        assert!(rim.0 > 0 && rim.0 < 200, "rim should be partial, got {rim:?}");
        // well outside stays black
        assert_eq!(c.get(17, 10).color, (0, 0, 0));
        // round, not square: the corner of the bounding box is untouched
        assert_eq!(c.get(14, 14).color, (0, 0, 0));
    }

    #[test]
    fn disc_degenerate_radii_are_safe() {
        let mut c = Canvas::new(5, 5);
        c.clear((0, 0, 0));
        disc(&mut c, 2.0, 2.0, 0.0, (255, 255, 255));
        disc(&mut c, 2.0, 2.0, -1.0, (255, 255, 255));
        assert_eq!(c.get(2, 2).color, (0, 0, 0));
        // fully off-canvas is clipped, not a panic
        disc(&mut c, -50.0, -50.0, 3.0, (255, 255, 255));
        disc(&mut c, 99.0, 99.0, 3.0, (255, 255, 255));
    }

    #[test]
    fn dot_integer_position_hits_one_cell() {
        let mut c = Canvas::new(8, 8);
        c.clear((0, 0, 0));
        dot(&mut c, 3.0, 3.0, (200, 100, 50), 1.0);
        assert_eq!(c.get(3, 3).color, (200, 100, 50));
        assert_eq!(c.get(4, 3).color, (0, 0, 0));
        assert_eq!(c.get(3, 4).color, (0, 0, 0));
    }

    #[test]
    fn dot_half_position_splits_weight() {
        let mut c = Canvas::new(8, 8);
        c.clear((0, 0, 0));
        dot(&mut c, 3.5, 3.0, (200, 200, 200), 1.0);
        let (a, b) = (c.get(3, 3).color.0, c.get(4, 3).color.0);
        assert_eq!(a, b, "even split expected, got {a} vs {b}");
        assert!(a > 0 && a < 200);
        // total light is conserved (within u8 rounding)
        assert!((a as i32 + b as i32 - 200).abs() <= 2);
    }

    #[test]
    fn dot_off_canvas_and_zero_alpha_are_safe() {
        let mut c = Canvas::new(4, 4);
        c.clear((0, 0, 0));
        dot(&mut c, -10.0, -10.0, (255, 255, 255), 1.0);
        dot(&mut c, 100.0, 2.0, (255, 255, 255), 1.0);
        dot(&mut c, 2.0, 2.0, (255, 255, 255), 0.0);
        assert_eq!(c.get(2, 2).color, (0, 0, 0));
    }

    #[test]
    fn glow_f_matches_add_glow_at_integer_center() {
        let mut a = Canvas::new(16, 16);
        a.clear((0, 0, 0));
        a.add_glow(8, 8, 3, (200, 150, 100), 0.9);
        let mut b = Canvas::new(16, 16);
        b.clear((0, 0, 0));
        glow_f(&mut b, 8.0, 8.0, 3.0, (200, 150, 100), 0.9);
        for y in 0..16 {
            for x in 0..16 {
                let (ca, cb) = (a.get(x, y).color, b.get(x, y).color);
                let d = (ca.0 as i32 - cb.0 as i32).abs();
                assert!(d <= 1, "mismatch at ({x},{y}): {ca:?} vs {cb:?}");
            }
        }
    }

    #[test]
    fn glow_f_is_bounded_and_slides() {
        let mut c = Canvas::new(16, 16);
        c.clear((0, 0, 0));
        glow_f(&mut c, 8.5, 8.0, 2.0, (200, 200, 200), 1.0);
        // brightest at the two cells nearest the center, both equally lit
        assert_eq!(c.get(8, 8).color, c.get(9, 8).color);
        assert!(c.get(8, 8).color.0 > 0);
        // outside the radius stays black
        assert_eq!(c.get(12, 8).color, (0, 0, 0));
        // off-canvas center clips, no panic
        glow_f(&mut c, -5.0, -5.0, 3.0, (255, 255, 255), 1.0);
    }

    #[test]
    fn line_f_covers_both_endpoints() {
        let mut c = Canvas::new(16, 16);
        c.clear((0, 0, 0));
        line_f(&mut c, 2.0, 2.0, 12.0, 9.0, (200, 200, 200), 1.0);
        assert!(c.get(2, 2).color.0 > 0, "start endpoint unlit");
        assert!(c.get(12, 9).color.0 > 0, "end endpoint unlit");
        // no gaps: every column between the endpoints has light somewhere
        for x in 2..=12 {
            let lit = (0..16).any(|y| c.get(x, y).color.0 > 0);
            assert!(lit, "gap at column {x}");
        }
    }

    #[test]
    fn rect_f_integer_matches_integer_fill() {
        let mut c = Canvas::new(10, 10);
        c.clear((0, 0, 0));
        rect_f(&mut c, 2.0, 3.0, 3.0, 2.0, (200, 100, 50));
        for y in 0..10i32 {
            for x in 0..10i32 {
                let inside = (2..5).contains(&x) && (3..5).contains(&y);
                let expect = if inside { (200, 100, 50) } else { (0, 0, 0) };
                assert_eq!(c.get(x, y).color, expect, "cell ({x},{y})");
            }
        }
    }

    #[test]
    fn rect_f_fractional_softens_edges() {
        let mut c = Canvas::new(10, 10);
        c.clear((0, 0, 0));
        rect_f(&mut c, 2.5, 3.0, 3.0, 2.0, (200, 200, 200));
        // interior opaque, straddled edge cells at half strength
        assert_eq!(c.get(3, 3).color, (200, 200, 200));
        let (l, r) = (c.get(2, 3).color.0, c.get(5, 3).color.0);
        assert!(l > 0 && l < 200, "left edge should be partial, got {l}");
        assert!(r > 0 && r < 200, "right edge should be partial, got {r}");
        // degenerate sizes are safe
        rect_f(&mut c, 1.0, 1.0, 0.0, 5.0, (255, 255, 255));
        rect_f(&mut c, 1.0, 1.0, 5.0, -1.0, (255, 255, 255));
    }

    #[test]
    fn approach_is_framerate_independent() {
        // one big step lands exactly where two half steps do
        let one = approach(0.0, 10.0, 3.0, 0.2);
        let half = approach(approach(0.0, 10.0, 3.0, 0.1), 10.0, 3.0, 0.1);
        assert!((one - half).abs() < 1e-4, "one={one} half={half}");
        // converges toward the target, never past it
        assert!(one > 0.0 && one < 10.0);
        assert!(approach(5.0, 5.0, 10.0, 1.0) == 5.0);
    }

    #[test]
    fn glyph_cells_never_blend() {
        let mut prev = Canvas::new(2, 2);
        prev.clear((0, 0, 0));
        let mut cur = Canvas::new(2, 2);
        cur.clear((100, 100, 100));
        cur.set_char(0, 0, 'x', (200, 200, 200));
        cur.smooth_blend(&prev, 0.3);
        assert_eq!(cur.get(0, 0).color, (200, 200, 200));
        assert_eq!(cur.get(1, 0).color, (30, 30, 30));
    }
}

#[cfg(test)]
mod glow_equivalence_tests {
    use super::*;

    /// The original implementation, kept verbatim as the reference.
    fn glow_reference(
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        r: i32,
        color: (u8, u8, u8),
        intensity: f32,
    ) {
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                let d2 = dx * dx + dy * dy;
                if d2 <= r2 {
                    let f = 1.0 - d2 as f32 / (r2 + 1) as f32;
                    canvas.add(x + dx, y + dy, scale(color, intensity * f * f));
                }
            }
        }
    }

    /// Bit-identical output, including at every edge and corner, off-canvas,
    /// and where additive saturation clips.
    #[test]
    fn clipped_glow_matches_reference_everywhere() {
        let (w, h) = (37usize, 23usize);
        for r in [1i32, 2, 3, 4, 5, 9, 16, 17, 23] {
            for &(x, y) in &[
                (18, 11),   // interior
                (0, 0),     // corner
                (36, 22),   // opposite corner
                (0, 11),    // left edge
                (36, 11),   // right edge
                (18, 0),    // top edge
                (18, 22),   // bottom edge
                (-4, -4),   // straddling off-canvas
                (40, 25),   // fully off-canvas
                (-99, 11),  // far off-canvas
            ] {
                for &intensity in &[0.0f32, 0.15, 0.5, 1.0, 2.5] {
                    let mut a = Canvas::new(w, h);
                    let mut b = Canvas::new(w, h);
                    // pre-fill so saturating_add clipping is exercised
                    for i in 0..w * h {
                        let v = (i % 251) as u8;
                        a.cells[i].color = (v, 255 - v, v / 2);
                        b.cells[i].color = (v, 255 - v, v / 2);
                    }
                    glow_reference(&mut a, x, y, r, (240, 130, 60), intensity);
                    b.add_glow(x, y, r, (240, 130, 60), intensity);
                    assert_eq!(
                        a.cells, b.cells,
                        "colors differ at r={r} pos=({x},{y}) i={intensity}"
                    );
                    assert_eq!(
                        a.touched, b.touched,
                        "touched differs at r={r} pos=({x},{y}) i={intensity}"
                    );
                }
            }
        }
    }

    /// Degenerate radii must be no-ops, not panics.
    #[test]
    fn nonpositive_radius_is_a_noop() {
        let mut c = Canvas::new(5, 5);
        c.add_glow(2, 2, 0, (255, 255, 255), 1.0);
        c.add_glow(2, 2, -3, (255, 255, 255), 1.0);
        assert!(c.cells.iter().all(|x| x.color == (0, 0, 0)));
        assert!(c.touched.iter().all(|t| !t));
    }

    /// Glyph cells keep their character: glow is additive light, not a rewrite.
    #[test]
    fn glow_preserves_glyphs() {
        let mut c = Canvas::new(9, 9);
        c.set_char(4, 4, 'A', (10, 10, 10));
        c.add_glow(4, 4, 3, (200, 200, 200), 1.0);
        assert_eq!(c.get(4, 4).ch, Some('A'));
    }
}
