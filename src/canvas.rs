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

    /// Shallow clone used by the temporal smoothing step.
    pub fn clone_for_smooth(&self) -> Canvas {
        Canvas {
            width: self.width,
            height: self.height,
            cells: self.cells.clone(),
            touched: self.touched.clone(),
        }
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
#[allow(dead_code)] // utility kept for scene polish
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Bounded radial glow: additive, quadratic falloff, no sqrt in the loop.
/// `r` is the radius in pixels (kept small — this is the perf-safe glow).
pub fn glow(canvas: &mut Canvas, x: i32, y: i32, r: i32, color: (u8, u8, u8), intensity: f32) {
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
