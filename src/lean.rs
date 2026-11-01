//! A ratatui backend that says the same frame in fewer bytes.
//!
//! The terminal write is the bottleneck, and ratatui's crossterm backend
//! spends bytes it does not need to: an absolute `ESC[row;colH` for every
//! skipped run of cells, and both colours again whenever either changes.
//! This wrapper overrides only `draw`: a skip along the same row becomes a
//! relative `ESC[nC`, and only the colour that changed is re-sent. Every
//! other call goes straight to the crossterm backend.

use std::io::{self, Write};

use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{Attribute, SetAttribute, SetBackgroundColor, SetForegroundColor};
use ratatui::backend::{Backend, ClearType, CrosstermBackend, IntoCrossterm, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier};
use unicode_width::UnicodeWidthStr;

pub struct LeanBackend<W: Write> {
    inner: CrosstermBackend<W>,
}

impl<W: Write> LeanBackend<W> {
    pub fn new(writer: W) -> Self {
        LeanBackend { inner: CrosstermBackend::new(writer) }
    }
}

impl<W: Write> Write for LeanBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut self.inner)
    }
}

/// `ESC[38;2;r;g;bm` / `ESC[48;2;r;g;bm`, or crossterm's spelling for a
/// colour that is not RGB.
fn set_fg(w: &mut impl Write, c: Color) -> io::Result<()> {
    match c {
        Color::Rgb(r, g, b) => write!(w, "\x1b[38;2;{r};{g};{b}m"),
        c => queue!(w, SetForegroundColor(c.into_crossterm())),
    }
}

fn set_bg(w: &mut impl Write, c: Color) -> io::Result<()> {
    match c {
        Color::Rgb(r, g, b) => write!(w, "\x1b[48;2;{r};{g};{b}m"),
        c => queue!(w, SetBackgroundColor(c.into_crossterm())),
    }
}

/// Reset every attribute, then set the ones `m` asks for. Resetting also
/// drops the colours, which the caller re-sends.
fn set_modifier(w: &mut impl Write, m: Modifier) -> io::Result<()> {
    queue!(w, SetAttribute(Attribute::Reset))?;
    for (bit, attr) in [
        (Modifier::BOLD, Attribute::Bold),
        (Modifier::DIM, Attribute::Dim),
        (Modifier::ITALIC, Attribute::Italic),
        (Modifier::UNDERLINED, Attribute::Underlined),
        (Modifier::SLOW_BLINK, Attribute::SlowBlink),
        (Modifier::RAPID_BLINK, Attribute::RapidBlink),
        (Modifier::REVERSED, Attribute::Reverse),
        (Modifier::HIDDEN, Attribute::Hidden),
        (Modifier::CROSSED_OUT, Attribute::CrossedOut),
    ] {
        if m.contains(bit) {
            queue!(w, SetAttribute(attr))?;
        }
    }
    Ok(())
}

impl<W: Write> Backend for LeanBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let w = &mut self.inner;
        // None = not known yet: the first colour of the frame is always sent
        let mut fg: Option<Color> = None;
        let mut bg: Option<Color> = None;
        let mut modifier = Modifier::empty();
        // where the cursor is now, when we know it exactly
        let mut cursor: Option<(u16, u16)> = None;
        for (x, y, cell) in content {
            match cursor {
                Some((cx, cy)) if cy == y && cx == x => {}
                Some((cx, cy)) if cy == y && cx < x => {
                    let n = x - cx;
                    if n == 1 { w.write_all(b"\x1b[C")?; } else { write!(w, "\x1b[{n}C")?; }
                }
                _ => queue!(w, MoveTo(x, y))?,
            }
            if cell.modifier != modifier {
                set_modifier(w, cell.modifier)?;
                modifier = cell.modifier;
                fg = None;
                bg = None;
            }
            match (fg != Some(cell.fg), bg != Some(cell.bg)) {
                (true, true) => match (cell.fg, cell.bg) {
                    (Color::Rgb(r, g, b), Color::Rgb(r2, g2, b2)) => {
                        write!(w, "\x1b[38;2;{r};{g};{b};48;2;{r2};{g2};{b2}m")?
                    }
                    (f, b) => {
                        set_fg(w, f)?;
                        set_bg(w, b)?;
                    }
                },
                (true, false) => set_fg(w, cell.fg)?,
                (false, true) => set_bg(w, cell.bg)?,
                (false, false) => {}
            }
            fg = Some(cell.fg);
            bg = Some(cell.bg);
            let sym = cell.symbol();
            w.write_all(sym.as_bytes())?;
            // a width the terminal might disagree on (wide or zero-width
            // glyphs) or the pending wrap at the right edge: move absolutely
            // next time
            cursor = match sym.width() {
                1 => Some((x + 1, y)),
                _ => None,
            };
        }
        queue!(
            w,
            SetForegroundColor(crossterm::style::Color::Reset),
            SetBackgroundColor(crossterm::style::Color::Reset),
            SetAttribute(Attribute::Reset),
        )
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }
    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }
    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }
    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }
    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }
    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }
    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }
    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }
    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A writer the test can still read after handing it to a backend.
    #[derive(Clone, Default)]
    struct Shared(Rc<RefCell<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Shared {
        fn bytes(&self) -> Vec<u8> {
            self.0.borrow().clone()
        }
    }

    /// Replay the bytes a backend wrote into a grid of (symbol, fg, bg).
    /// Only what both backends emit is understood: CUP, CUF and SGR.
    fn replay(bytes: &[u8], w: usize, h: usize) -> Vec<(String, [i32; 3], [i32; 3])> {
        let mut grid = vec![(" ".to_string(), [-1; 3], [-1; 3]); w * h];
        let s = std::str::from_utf8(bytes).unwrap();
        let (mut x, mut y) = (0usize, 0usize);
        let (mut fg, mut bg) = ([-1; 3], [-1; 3]);
        let mut it = s.chars().peekable();
        while let Some(c) = it.next() {
            if c != '\x1b' {
                grid[y * w + x] = (c.to_string(), fg, bg);
                x += 1;
                continue;
            }
            assert_eq!(it.next(), Some('['));
            let mut args = String::new();
            let fin = loop {
                let c = it.next().unwrap();
                if c.is_ascii_alphabetic() { break c; }
                args.push(c);
            };
            let v: Vec<i32> = args.split(';').filter(|a| !a.is_empty()).map(|a| a.parse().unwrap()).collect();
            match fin {
                'H' => { y = v[0] as usize - 1; x = v[1] as usize - 1; }
                'C' => x += v.first().copied().unwrap_or(1) as usize,
                'm' => {
                    let mut i = 0;
                    while i < v.len() {
                        match v[i] {
                            38 => { fg = [v[i + 2], v[i + 3], v[i + 4]]; i += 5; }
                            48 => { bg = [v[i + 2], v[i + 3], v[i + 4]]; i += 5; }
                            _ => i += 1,
                        }
                    }
                }
                _ => {}
            }
        }
        grid
    }

    fn frame(seed: u32, w: u16, h: u16) -> Buffer {
        let mut b = Buffer::empty(Rect::new(0, 0, w, h));
        let mut s = seed;
        for y in 0..h {
            for x in 0..w {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let v = (s >> 16) as u8;
                let cell = &mut b[(x, y)];
                cell.set_symbol(["▀", "▖", " ", "█"][(v % 4) as usize]);
                cell.set_fg(Color::Rgb(v, v / 2, 7));
                cell.set_bg(Color::Rgb(v / 3, (v % 3) * 40, v));
            }
        }
        b
    }

    /// The lean backend's bytes paint exactly what ratatui's do, across a
    /// sparse diff (gaps on one row, new rows, runs).
    #[test]
    fn paints_the_same_cells_in_fewer_bytes() {
        let (w, h) = (40u16, 6u16);
        let a = frame(1, w, h);
        let mut b = a.clone();
        let mut s = 7u32;
        for y in 0..h {
            for x in 0..w {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                if (s >> 16) % 3 == 0 {
                    b[(x, y)].set_fg(Color::Rgb((s >> 8) as u8, 1, 2));
                }
                if (s >> 16) % 5 == 0 {
                    b[(x, y)].set_bg(Color::Rgb(9, (s >> 9) as u8, 3));
                }
            }
        }
        let first: Vec<(u16, u16, &Cell)> = a.content.iter().enumerate()
            .map(|(i, c)| ((i as u16) % w, (i as u16) / w, c)).collect();
        let diff = a.diff(&b);

        let draw = |b: &mut dyn FnMut(Vec<(u16, u16, &Cell)>)| {
            b(first.clone());
            b(diff.clone());
        };
        let stock_out = Shared::default();
        let mut stock = CrosstermBackend::new(stock_out.clone());
        draw(&mut |c| stock.draw(c.into_iter()).unwrap());
        let lean_out = Shared::default();
        let mut lean = LeanBackend::new(lean_out.clone());
        lean.draw(first.clone().into_iter()).unwrap();
        let before = lean_out.bytes().len();
        lean.draw(diff.clone().into_iter()).unwrap();

        let want = replay(&stock_out.bytes(), w as usize, h as usize);
        let got = replay(&lean_out.bytes(), w as usize, h as usize);
        assert_eq!(got, want);
        let diff_only = Shared::default();
        CrosstermBackend::new(diff_only.clone()).draw(diff.clone().into_iter()).unwrap();
        let stock_diff = diff_only.bytes().len();
        let lean_diff = lean_out.bytes().len() - before;
        assert!(lean_diff < stock_diff, "lean {lean_diff} B vs stock {stock_diff} B");
    }

    /// A wide glyph leaves the cursor somewhere we do not trust: the next
    /// cell moves absolutely.
    #[test]
    fn moves_absolutely_after_a_wide_glyph() {
        let mut b = Buffer::empty(Rect::new(0, 0, 6, 1));
        b[(0, 0)].set_symbol("界");
        b[(3, 0)].set_symbol("x");
        let cells = [(0u16, 0u16, &b[(0, 0)]), (3, 0, &b[(3, 0)])];
        let out = Shared::default();
        LeanBackend::new(out.clone()).draw(cells.into_iter()).unwrap();
        let out = String::from_utf8(out.bytes()).unwrap();
        assert!(out.contains("\x1b[1;4H"), "{out:?}");
    }
}
