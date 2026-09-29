//! Pixel-count render modes: half-block (1x2 px/cell, color-true),
//! quadrant (2x2 px/cell) and braille (2x4 px/cell, highest density).

use crate::canvas::{rgb_to_256, Canvas};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

/// How many canvas pixels each terminal cell shows, and how.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Pixels {
    /// 1x2 px per cell via '▀' — the classic, most color-true.
    Half,
    /// 2x2 px per cell via quadrant glyphs (▘▝▖▗ and combos).
    #[default]
    Quad,
    /// 2x4 px per cell via braille dot patterns — highest density.
    Braille,
}

impl Pixels {
    /// Canvas pixels per terminal cell, (wide, tall).
    pub fn cell_size(self) -> (usize, usize) {
        match self {
            Pixels::Half => (1, 2),
            Pixels::Quad => (2, 2),
            Pixels::Braille => (2, 4),
        }
    }

    /// Height of one canvas pixel divided by its width, on screen. A terminal
    /// cell is close to 1:2, so half-block and braille pixels are roughly
    /// square while quadrant pixels are twice as tall as they are wide.
    /// Scenes use this to keep circles round and to detect portrait canvases.
    pub fn aspect(self) -> f32 {
        let (pw, ph) = self.cell_size();
        // cell is 1 wide : 2 tall; pixel = (1/pw) wide, (2/ph) tall
        (2.0 / ph as f32) / (1.0 / pw as f32)
    }

    #[allow(dead_code)] // used by the settings menu
    pub fn name(self) -> &'static str {
        match self {
            Pixels::Half => "half",
            Pixels::Quad => "quad",
            Pixels::Braille => "braille",
        }
    }

    #[allow(dead_code)] // used by the settings menu
    pub fn next(self) -> Self {
        match self {
            Pixels::Half => Pixels::Quad,
            Pixels::Quad => Pixels::Braille,
            Pixels::Braille => Pixels::Half,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "half" => Some(Pixels::Half),
            "quad" => Some(Pixels::Quad),
            "braille" => Some(Pixels::Braille),
            _ => None,
        }
    }
}

fn to_color(rgb: (u8, u8, u8), truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(rgb.0, rgb.1, rgb.2)
    } else {
        Color::Indexed(rgb_to_256(rgb.0, rgb.1, rgb.2))
    }
}

fn lum(c: (u8, u8, u8)) -> u32 {
    (c.0 as u32) * 3 + (c.1 as u32) * 6 + c.2 as u32
}

/// Split a block of pixels into (fg mask bits, fg color, bg color) by a
/// luminance threshold. Bits are set for pixels *above* the average.
///
/// Called once per terminal cell per frame, so it is kept to two passes with
/// nothing recomputed: luminances are cached from the first pass rather than
/// re-derived, and the whole-block channel means (the fallback when every pixel
/// lands on one side of the average) are accumulated up front instead of
/// re-scanning the block once per channel.
fn split(pixels: &[(u8, u8, u8)]) -> (u8, (u8, u8, u8), (u8, u8, u8)) {
    // Braille is the widest block at 2x4
    const MAX: usize = 8;
    let n = pixels.len().min(MAX);
    let mut lums = [0u32; MAX];
    let mut lsum = 0u32;
    let mut tot = [0u32; 3];
    for (i, c) in pixels.iter().take(n).enumerate() {
        let l = lum(*c);
        lums[i] = l;
        lsum += l;
        tot[0] += c.0 as u32;
        tot[1] += c.1 as u32;
        tot[2] += c.2 as u32;
    }
    let avg = lsum / n as u32;
    let mut mask = 0u8;
    let (mut fg, mut bg) = ([0u32; 3], [0u32; 3]);
    let (mut wf, mut wb) = (0u32, 0u32);
    for (i, c) in pixels.iter().take(n).enumerate() {
        let l = lums[i];
        if l > avg {
            mask |= 1 << i;
            // weight by distance above the average: bright pixels dominate
            let wgt = l - avg + 1;
            fg[0] += c.0 as u32 * wgt;
            fg[1] += c.1 as u32 * wgt;
            fg[2] += c.2 as u32 * wgt;
            wf += wgt;
        } else {
            let wgt = avg - l + 1;
            bg[0] += c.0 as u32 * wgt;
            bg[1] += c.1 as u32 * wgt;
            bg[2] += c.2 as u32 * wgt;
            wb += wgt;
        }
    }
    let nn = n as u32;
    let avg3 = |a: [u32; 3], w: u32| match w {
        0 => (
            (tot[0] / nn) as u8,
            (tot[1] / nn) as u8,
            (tot[2] / nn) as u8,
        ),
        _ => ((a[0] / w) as u8, (a[1] / w) as u8, (a[2] / w) as u8),
    };
    (mask, avg3(fg, wf), avg3(bg, wb))
}

/// Quadrant glyph for a 2x2 mask. Bit order: TL, TR, BL, BR.
pub fn quad_glyph(mask: u8) -> char {
    match mask & 0xF {
        0b0000 => ' ',
        0b0001 => '▘',
        0b0010 => '▝',
        0b0011 => '▀',
        0b0100 => '▖',
        0b0101 => '▌',
        0b0110 => '▞',
        0b0111 => '▛',
        0b1000 => '▗',
        0b1001 => '▚',
        0b1010 => '▐',
        0b1011 => '▜',
        0b1100 => '▄',
        0b1101 => '▙',
        0b1110 => '▟',
        _ => '█',
    }
}

/// Braille character for a 2x4 mask. Bit order: left column top→bottom,
/// then right column top→bottom (braille dots 1,2,3,7,4,5,6,8).
pub fn braille_glyph(mask: u8) -> char {
    const REMAP: [u8; 8] = [0, 1, 2, 6, 3, 4, 5, 7];
    let mut m = 0u8;
    for (i, &dot) in REMAP.iter().enumerate() {
        if mask & (1 << i) != 0 {
            m |= 1 << dot;
        }
    }
    char::from_u32(0x2800 + m as u32).unwrap_or('⠀')
}

/// Paint the canvas into a ratatui buffer in the given pixel mode.
/// Canvas dims must be (area.width*pw, area.height*ph).
pub fn draw(canvas: &Canvas, area: Rect, buf: &mut Buffer, truecolor: bool, mode: Pixels) {
    draw_crop(canvas, 0, 0, area, buf, truecolor, mode);
}

/// Paint a crop of a larger (virtual) canvas. Canvas pixel (crop_x, crop_y)
/// maps to terminal cell (0,0).
pub fn draw_crop(
    canvas: &Canvas,
    crop_x: i32,
    crop_y: i32,
    area: Rect,
    buf: &mut Buffer,
    truecolor: bool,
    mode: Pixels,
) {
    let (pw, ph) = mode.cell_size();
    for row in 0..area.height {
        for col in 0..area.width {
            let cell = &mut buf[(area.x + col, area.y + row)];
            // glyph cells (bump) keep their character in every mode
            let tl = canvas.get(
                crop_x + (col as usize * pw) as i32,
                crop_y + (row as usize * ph) as i32,
            );
            if let Some(ch) = tl.ch {
                let bg = canvas.get(
                    crop_x + (col as usize * pw) as i32,
                    crop_y + (row as usize * ph + ph - 1) as i32,
                );
                cell.set_char(ch);
                cell.set_fg(to_color(tl.color, truecolor));
                cell.set_bg(to_color(bg.color, truecolor));
                continue;
            }
            match mode {
                Pixels::Half => {
                    let bottom = canvas.get(crop_x + col as i32, crop_y + (row * 2 + 1) as i32);
                    cell.set_char('▀');
                    cell.set_fg(to_color(tl.color, truecolor));
                    cell.set_bg(to_color(bottom.color, truecolor));
                }
                Pixels::Quad => {
                    let px = [
                        tl.color,
                        canvas.get(crop_x + col as i32 * 2 + 1, crop_y + row as i32 * 2).color,
                        canvas.get(crop_x + col as i32 * 2, crop_y + row as i32 * 2 + 1).color,
                        canvas.get(crop_x + col as i32 * 2 + 1, crop_y + row as i32 * 2 + 1).color,
                    ];
                    let (mask, fg, bg) = split(&px);
                    cell.set_char(quad_glyph(mask));
                    cell.set_fg(to_color(fg, truecolor));
                    cell.set_bg(to_color(bg, truecolor));
                }
                Pixels::Braille => {
                    let mut px = [(0u8, 0u8, 0u8); 8];
                    for dy in 0..4 {
                        for dx in 0..2 {
                            px[dx * 4 + dy] = canvas
                                .get(
                                    crop_x + col as i32 * 2 + dx as i32,
                                    crop_y + row as i32 * 4 + dy as i32,
                                )
                                .color;
                        }
                    }
                    let (mask, fg, bg) = split(&px);
                    cell.set_char(braille_glyph(mask));
                    cell.set_fg(to_color(fg, truecolor));
                    cell.set_bg(to_color(bg, truecolor));
                }
            }
        }
    }
}

/// Resample part of a larger canvas into `out` (a pane-sized canvas of
/// `cols x rows` cells in `mode`). Pane cell (c, r) shows the source cells
/// from `origin + (c, r) * step` over one `step`; both canvases have the same
/// pixels per cell. Steps above ~1 average the covered pixels (box filter),
/// so a coarser pane does not alias or skip columns; below that it samples
/// the nearest pixel, keeping glyph characters.
pub fn resample_view(src: &Canvas, origin: [f64; 2], step: [f64; 2], cols: usize, rows: usize, mode: Pixels, out: &mut Canvas) {
    let (pw, ph) = mode.cell_size();
    let (w, h) = (cols * pw, rows * ph);
    if (out.width(), out.height()) != (w, h) {
        out.resize(w, h);
    }
    let ox = origin[0] * pw as f64;
    let oy = origin[1] * ph as f64;
    let (sx, sy) = (step[0].max(1e-6), step[1].max(1e-6));
    for y in 0..h {
        let y0 = oy + y as f64 * sy;
        for x in 0..w {
            let x0 = ox + x as f64 * sx;
            if sx <= 1.25 && sy <= 1.25 {
                let c = src.get((x0 + sx * 0.5).floor() as i32, (y0 + sy * 0.5).floor() as i32);
                match c.ch {
                    Some(ch) => out.set_char(x as i32, y as i32, ch, c.color),
                    None => out.set(x as i32, y as i32, c.color),
                }
                continue;
            }
            // pixels whose centres fall inside the footprint (at least one)
            let (ax, bx) = ((x0 - 0.5).ceil() as i32, ((x0 + sx - 0.5).ceil() as i32).max((x0 - 0.5).ceil() as i32 + 1));
            let (ay, by) = ((y0 - 0.5).ceil() as i32, ((y0 + sy - 0.5).ceil() as i32).max((y0 - 0.5).ceil() as i32 + 1));
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for yy in ay..by {
                for xx in ax..bx {
                    let c = src.get(xx, yy).color;
                    r += c.0 as u32;
                    g += c.1 as u32;
                    b += c.2 as u32;
                    n += 1;
                }
            }
            let n = n.max(1);
            out.set(x as i32, y as i32, ((r / n) as u8, (g / n) as u8, (b / n) as u8));
        }
    }
}

/// Whether a view is a whole-pixel crop, so the plain crop path applies.
pub fn view_is_crop(origin: [f64; 2], step: [f64; 2], mode: Pixels) -> Option<(i32, i32)> {
    let (pw, ph) = mode.cell_size();
    let (x, y) = (origin[0] * pw as f64, origin[1] * ph as f64);
    let whole = |v: f64| (v - v.round()).abs() < 0.05;
    ((step[0] - 1.0).abs() < 0.01 && (step[1] - 1.0).abs() < 0.01 && whole(x) && whole(y))
        .then(|| (x.round() as i32, y.round() as i32))
}

/// Paint a pane's view of a larger canvas (see [`resample_view`]), taking
/// the plain crop path when the view is a whole-pixel crop.
#[allow(clippy::too_many_arguments)]
pub fn draw_view(
    canvas: &Canvas,
    origin: [f64; 2],
    step: [f64; 2],
    area: Rect,
    buf: &mut Buffer,
    truecolor: bool,
    mode: Pixels,
    scratch: &mut Canvas,
) {
    if let Some((x, y)) = view_is_crop(origin, step, mode) {
        draw_crop(canvas, x, y, area, buf, truecolor, mode);
        return;
    }
    resample_view(canvas, origin, step, area.width as usize, area.height as usize, mode, scratch);
    draw_crop(scratch, 0, 0, area, buf, truecolor, mode);
}

#[cfg(test)]
mod view_tests {
    use super::*;

    #[test]
    fn unit_views_are_crops_and_match_draw_crop() {
        let mut c = Canvas::new(40, 40);
        for y in 0..40 {
            for x in 0..40 {
                c.set(x, y, ((x * 6) as u8, (y * 6) as u8, 90));
            }
        }
        assert_eq!(view_is_crop([3.0, 2.0], [1.0, 1.0], Pixels::Half), Some((3, 4)));
        assert_eq!(view_is_crop([3.3, 2.0], [1.0, 1.0], Pixels::Half), None);
        let area = Rect::new(0, 0, 10, 8);
        let mut a = Buffer::empty(area);
        let mut b = Buffer::empty(area);
        let mut scratch = Canvas::new(1, 1);
        draw_crop(&c, 3, 4, area, &mut a, true, Pixels::Half);
        // force the resampling path with a hair-off origin: still identical
        resample_view(&c, [3.0 + 1e-9, 2.0], [1.0, 1.0], 10, 8, Pixels::Half, &mut scratch);
        draw_crop(&scratch, 0, 0, area, &mut b, true, Pixels::Half);
        assert_eq!(a, b);
    }

    #[test]
    fn coarse_views_average_instead_of_skipping() {
        // alternating black/white columns, viewed at 2 source px per pane px
        let mut c = Canvas::new(20, 4);
        for y in 0..4 {
            for x in 0..20 {
                let v = if x % 2 == 0 { 0 } else { 200 };
                c.set(x, y, (v, v, v));
            }
        }
        let mut out = Canvas::new(1, 1);
        resample_view(&c, [0.0, 0.0], [2.0, 1.0], 10, 2, Pixels::Half, &mut out);
        for x in 0..10 {
            assert_eq!(out.get(x, 0).color, (100, 100, 100), "column {x} should be the average");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pixel_aspect_matches_cell_geometry() {
        use super::Pixels;
        // a 1:2 terminal cell: half and braille pixels are square, quad
        // pixels are twice as tall as wide
        assert_eq!(Pixels::Half.aspect(), 1.0);
        assert_eq!(Pixels::Braille.aspect(), 1.0);
        assert_eq!(Pixels::Quad.aspect(), 2.0);
    }

    use super::*;

    #[test]
    fn quad_mapping_corners_and_combos() {
        assert_eq!(quad_glyph(0b0000), ' ');
        assert_eq!(quad_glyph(0b0001), '▘'); // top-left
        assert_eq!(quad_glyph(0b0010), '▝'); // top-right
        assert_eq!(quad_glyph(0b0100), '▖'); // bottom-left
        assert_eq!(quad_glyph(0b1000), '▗'); // bottom-right
        assert_eq!(quad_glyph(0b0011), '▀'); // top half
        assert_eq!(quad_glyph(0b1100), '▄'); // bottom half
        assert_eq!(quad_glyph(0b0101), '▌'); // left half
        assert_eq!(quad_glyph(0b1010), '▐'); // right half
        assert_eq!(quad_glyph(0b1001), '▚'); // TL+BR diagonal
        assert_eq!(quad_glyph(0b0110), '▞'); // TR+BL diagonal
        assert_eq!(quad_glyph(0b1111), '█'); // full
    }

    #[test]
    fn braille_mapping() {
        assert_eq!(braille_glyph(0), '⠀'); // blank
        assert_eq!(braille_glyph(0b0001), '⠁'); // dot 1 (top-left)
        assert_eq!(braille_glyph(0b10000), '⠈'); // dot 4 (top-right)
        assert_eq!(braille_glyph(0b1111), '⡇'); // left column (dots 1,2,3,7)
        assert_eq!(braille_glyph(0xFF), '⣿'); // full
        // bottom-left pixel (bit 3) maps to braille dot 7 (0x40)
        assert_eq!(braille_glyph(0b1000), '⡀');
    }

    #[test]
    fn split_assigns_bright_pixels_to_fg() {
        let px = [(255, 255, 255), (250, 250, 250), (0, 0, 0), (5, 5, 5)];
        let (mask, fg, bg) = split(&px);
        assert_eq!(mask, 0b0011); // first two pixels bright
        assert!(fg.0 > 200);
        assert!(bg.0 < 20);
        // count-weighted: one very bright pixel dominates the fg mean
        let px2 = [(255, 0, 0), (140, 100, 100), (0, 0, 255), (10, 10, 10)];
        let (_, fg2, _) = split(&px2);
        assert!(fg2.0 > fg2.2, "brightest pixel should pull fg: {fg2:?}");
        // uniform block: mask 0, both colors = average
        let (mask2, fg3, bg3) = split(&[(100, 100, 100); 4]);
        assert_eq!(mask2, 0);
        assert_eq!(fg3, bg3);
    }

    #[test]
    fn quad_cell_renders_two_by_two() {
        let mut c = Canvas::new(2, 2);
        c.set(0, 0, (255, 255, 255));
        c.set(1, 0, (255, 255, 255));
        c.set(0, 1, (0, 0, 0));
        c.set(1, 1, (0, 0, 0));
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        draw(&c, area, &mut buf, true, Pixels::Quad);
        let cell = &buf[(0u16, 0u16)];
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(255, 255, 255));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn braille_cell_renders_two_by_four() {
        let mut c = Canvas::new(2, 4);
        // left column bright, right column dark → dots 1,2,3,7 set
        for y in 0..4 {
            c.set(0, y, (255, 255, 255));
            c.set(1, y, (0, 0, 0));
        }
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        draw(&c, area, &mut buf, true, Pixels::Braille);
        let cell = &buf[(0u16, 0u16)];
        assert_eq!(cell.symbol(), "⡇");
    }
}
