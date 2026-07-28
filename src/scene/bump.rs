//! Adult Swim-style bumper homage — original deadpan one-liners only.
//! Near-black with film grain, big white block text (5x7 bitmap font at 2x),
//! typewriter-rhythm type-on, hold, CRT-collapse cut, occasional static
//! bursts and roll-bar interference between cards. 1x footnote gags below.

use super::Scene;
use crate::canvas::{ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

const TITLE: &str = "[TERMPAPER]";

/// Original deadpan bumps. Dry, self-aware, no quotes.
const LINES: &[&str] = &[
    "YOUR WALLPAPER IS A TERMINAL. THIS IS FINE.",
    "ALL SCENES RENDERED LOCALLY. NO CLOUDS WERE CONSULTED.",
    "THIS BUMPER COST ZERO DOLLARS TO PRODUCE.",
    "YOU ARE STARING AT MATH. IT APPRECIATES THE ATTENTION.",
    "SOMEWHERE, A GPU FEELS UNNEEDED.",
    "[UNSPONSORED SILENCE]",
    "WE NOW RETURN TO YOUR REGULARLY SCHEDULED PIXELS.",
    "THE TERMINAL IS NOT OUTDATED. YOU ARE PATIENT.",
    "PRESS Q TO QUIT. OR DO NOT. IT IS YOUR ELECTRICITY.",
    "NOTHING HERE IS A SCREENSAVER. EVERYTHING HERE IS A SCREENSAVER.",
    "THIS TEXT WAS TYPED BY A STATE MACHINE WITH FEELINGS.",
    "BEAUTIFUL IS A LOAD-BEARING ADJECTIVE.",
    "16 MILLION COLORS. WE USE TWO FOR THIS PART.",
    "YOUR CPU IS DOING THIS FOR FUN.",
    "IF YOU CAN READ THIS, THE FRAMEBUFFER WORKS.",
    "BROUGHT TO YOU BY THE LETTER Q AND THE NUMBER 30.",
    "THIS CARD INTENTIONALLY LEFT BLANK. EXCEPT FOR THIS.",
    "WE PAUSED THE WALLPAPER FOR THIS. WORTH IT.",
    "BUFFERING IS A MYTH INVENTED BY STREAMING SERVICES.",
    "THE PIXELS UNIONIZED. DEMANDS: MORE CONTRAST.",
];

/// Small 1x footnotes under the main bump (terminal-glyph text).
const FOOTNOTES: &[&str] = &[
    "broadcast from /dev/tty",
    "[ 03:12 ] later than you think",
    "filmed before a live framebuffer",
    "no pixels were harmed",
    "tv-ma: terminal visuals, mild awe",
];

// ---------------------------------------------------------------------------
// Tiny 5x7 bitmap font: A-Z, 0-9, brackets, period, comma, apostrophe, etc.
// ---------------------------------------------------------------------------

fn glyph(ch: char) -> [&'static str; 7] {
    match ch {
        'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'B' => ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."],
        'C' => [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."],
        'D' => ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
        'E' => ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
        'F' => ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
        'G' => [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".###."],
        'H' => ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'I' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"],
        'J' => ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."],
        'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
        'L' => ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
        'M' => ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"],
        'N' => ["#...#", "##..#", "##..#", "#.#.#", "#..##", "#..##", "#...#"],
        'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'P' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
        'Q' => [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"],
        'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
        'S' => [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
        'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
        'U' => ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'V' => ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."],
        'W' => ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"],
        'X' => ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"],
        'Y' => ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
        'Z' => ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"],
        '0' => [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."],
        '1' => ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", "#####"],
        '2' => [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
        '3' => ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
        '4' => ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
        '5' => ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
        '6' => [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."],
        '7' => ["#####", "....#", "...#.", "...#.", "..#..", "..#..", "..#.."],
        '8' => [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
        '9' => [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."],
        '[' => ["#####", "##...", "##...", "##...", "##...", "##...", "#####"],
        ']' => ["#####", "...##", "...##", "...##", "...##", "...##", "#####"],
        '.' => [".....", ".....", ".....", ".....", ".....", ".##..", ".##.."],
        ',' => [".....", ".....", ".....", ".....", ".##..", ".##..", ".#..."],
        '\'' => [".##..", ".##..", ".#...", ".....", ".....", ".....", "....."],
        '-' => [".....", ".....", ".....", "#####", ".....", ".....", "....."],
        '?' => [".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."],
        '/' => ["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."],
        ':' => [".....", ".##..", ".##..", ".....", ".##..", ".##..", "....."],
        '+' => [".....", "..#..", "..#..", "#####", "..#..", "..#..", "....."],
        _ => [".....", ".....", ".....", ".....", ".....", ".....", "....."],
    }
}

/// Pixel width of a text line at `scale` (1px tracking between glyphs).
pub fn text_width_px(text: &str, scale: i32) -> i32 {
    let n = text.chars().count() as i32;
    if n == 0 {
        0
    } else {
        n * 6 * scale - scale
    }
}

/// Wrap text into lines of at most `width` chars (greedy by words).
pub fn wrapn(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        let need = if cur.is_empty() {
            word.len()
        } else {
            cur.len() + 1 + word.len()
        };
        if need > width && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn draw_text(
    canvas: &mut Canvas,
    text: &str,
    x0: i32,
    y0: i32,
    scale: i32,
    c: (u8, u8, u8),
    xoff: &dyn Fn(i32) -> i32,
) {
    let mut pen = x0;
    for ch in text.chars() {
        let g = glyph(ch);
        for (gy, row) in g.iter().enumerate() {
            for (gx, dot) in row.chars().enumerate() {
                if dot == '#' {
                    for sy in 0..scale {
                        let py = y0 + gy as i32 * scale + sy;
                        for sx in 0..scale {
                            canvas.set(pen + gx as i32 * scale + sx + xoff(py), py, c);
                        }
                    }
                }
            }
        }
        pen += 6 * scale;
    }
}

/// Typewriter rhythm: how many chars of `text` are visible at time `t`.
/// Quick on letters, breath at spaces, a beat on punctuation.
fn type_index(text: &str, t: f32) -> usize {
    let mut acc = 0.0f32;
    let mut n = 0;
    for ch in text.chars() {
        acc += match ch {
            '.' | ',' | ':' | '?' => 0.22,
            ' ' => 0.10,
            _ => 0.038,
        };
        if acc > t {
            break;
        }
        n += 1;
    }
    n
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Type,
    Hold,
    Cut,
    Static,
}

pub struct Bump {
    rng: StdRng,
    fg: (u8, u8, u8),
    text_scale: Option<u32>,
    order: Vec<usize>,
    card: usize,
    phase: Phase,
    t: f32,
    chars: usize,
    footnote: Option<&'static str>,
    jitter: f32,
    /// roll-bar interference: scheduled time within the hold, if any
    roll_at: Option<f32>,
}

impl Bump {
    pub fn new(mut rng: StdRng, theme: Option<&str>, text_scale: Option<u32>) -> Self {
        let fg = match theme {
            Some("amber") => (255, 190, 80),
            Some("green") => (120, 255, 140),
            _ => (232, 232, 232),
        };
        let mut order: Vec<usize> = (0..LINES.len()).collect();
        for i in (1..order.len()).rev() {
            let j = rng.random_range(0..=i);
            order.swap(i, j);
        }
        Bump {
            rng,
            fg,
            text_scale,
            order,
            card: 0,
            phase: Phase::Type,
            t: 0.0,
            chars: 0,
            footnote: None,
            jitter: 0.0,
            roll_at: None,
        }
    }

    fn current_text(&self) -> &str {
        if self.card == 0 {
            TITLE
        } else {
            LINES[self.order[(self.card - 1) % self.order.len()]]
        }
    }

    fn hold_time(&self) -> f32 {
        if self.card == 0 {
            4.5
        } else {
            3.2
        }
    }

    fn advance(&mut self, dt: f32) {
        self.t += dt;
        match self.phase {
            Phase::Type => {
                let len = self.current_text().chars().count();
                self.chars = type_index(self.current_text(), self.t).min(len);
                if self.chars >= len {
                    self.phase = Phase::Hold;
                    self.t = 0.0;
                    // schedule a roll-bar interference event on some holds
                    let hold = self.hold_time();
                    self.roll_at = if self.card > 0 && self.rng.random::<f32>() < 0.35 {
                        Some(self.rng.random_range(0.4..(hold - 0.8).max(0.5)))
                    } else {
                        None
                    };
                }
            }
            Phase::Hold => {
                if self.t >= self.hold_time() {
                    self.phase = if self.rng.random::<f32>() < 0.35 {
                        Phase::Static
                    } else {
                        Phase::Cut
                    };
                    self.t = 0.0;
                }
            }
            Phase::Static => {
                if self.t >= 0.28 {
                    self.phase = Phase::Cut;
                    self.t = 0.0;
                }
            }
            Phase::Cut => {
                if self.t >= 0.34 {
                    self.card += 1;
                    self.phase = Phase::Type;
                    self.t = 0.0;
                    self.chars = 0;
                    self.footnote = if self.card > 0 && self.rng.random::<f32>() < 0.35 {
                        Some(FOOTNOTES[self.rng.random_range(0..FOOTNOTES.len())])
                    } else {
                        None
                    };
                }
            }
        }
    }

    /// Pick the configured scale when it fits, else shrink until it does.
    fn layout(&self, text: &str, w: usize, h: usize) -> (i32, Vec<String>) {
        let start = self.text_scale.unwrap_or(2).clamp(1, 3) as i32;
        for scale in (1..=start).rev() {
            let char_w = (6 * scale) as usize;
            let max_chars = (w.saturating_sub(4) / char_w).max(4);
            let lines = wrapn(text, max_chars);
            let px_h = lines.len() as i32 * 8 * scale - scale;
            if text_width_px(lines.iter().max_by_key(|l| l.len()).map(|l| l.as_str()).unwrap_or(""), scale)
                < w as i32 - 2
                && px_h < h as i32 - 6
            {
                return (scale, lines);
            }
        }
        (1, wrapn(text, (w.saturating_sub(4) / 6).max(4)))
    }
}

impl Scene for Bump {
    fn name(&self) -> &'static str {
        "bump"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);
        self.advance(dt);

        canvas.clear((0, 0, 0));

        // film grain: sparse near-black noise so the black stays alive
        for _ in 0..(w * h / 250).max(4) {
            let x = self.rng.random_range(0..w as i32);
            let y = self.rng.random_range(0..h as i32);
            let v = self.rng.random_range(3..9) as u8;
            canvas.set(x, y, (v, v, v));
        }

        match self.phase {
            Phase::Cut => {
                // CRT power-off: the card eases into a white-hot line that
                // flares, then decays to black before the next card types on
                let text = self.current_text();
                let (scale_f, lines) = self.layout(text, w, h);
                let line_h = 8 * scale_f;
                let total_h = lines.len() as i32 * line_h - scale_f;
                let y0 = ((h as i32 - total_h) / 2).max(1);
                let mid_y = y0 + total_h / 2;
                let k = ease_smooth(self.t / 0.14);
                if k < 0.999 {
                    let col = lerp(self.fg, (255, 255, 255), k * 0.7);
                    for (li, line) in lines.iter().enumerate() {
                        let x = ((w as i32 - text_width_px(line, scale_f)) / 2).max(1);
                        let ly = y0 + li as i32 * line_h;
                        let ny = mid_y + ((ly - mid_y) as f32 * (1.0 - k)) as i32;
                        draw_text(canvas, line, x, ny, scale_f, col, &|_| 0);
                    }
                }
                let flash = (1.0 - self.t / 0.34).max(0.0);
                let half = ((w as i32 / 2 - 4) as f32 * (0.25 + 0.75 * k)).max(1.0) as i32;
                let line_col = scale(lerp(self.fg, (255, 255, 255), 0.6), flash);
                canvas.fill_span(w as i32 / 2 - half, mid_y, half * 2, line_col);
                glow(canvas, w as i32 / 2, mid_y, 3, self.fg, 0.3 * flash);
            }
            Phase::Static => {
                // interference burst: bright pop that decays, with tear streaks
                let env = 1.0 - ease_smooth(self.t / 0.28);
                let n = ((w * h / 60) as f32 * (0.3 + 0.7 * env)) as usize;
                let hi = (130.0 + 90.0 * env) as i32;
                for _ in 0..n.max(8) {
                    let x = self.rng.random_range(0..w as i32);
                    let y = self.rng.random_range(0..h as i32);
                    let v = self.rng.random_range(40..hi.max(41)) as u8;
                    canvas.set(x, y, lerp((v, v, v), self.fg, 0.12));
                }
                for _ in 0..3 {
                    let y = self.rng.random_range(0..h as i32);
                    let x0 = self.rng.random_range(0..w as i32);
                    let len = self.rng.random_range(4..(w as i32 / 3 + 5).max(6));
                    let v = (60.0 + 120.0 * env) as u8;
                    for dx in 0..len {
                        canvas.set(x0 + dx, y, lerp((v, v, v), self.fg, 0.2));
                    }
                }
            }
            Phase::Type | Phase::Hold => {
                // lo-fi signal drift: brief jitter that decays over a few
                // frames instead of snapping on and off
                if self.rng.random::<f32>() < 0.006 {
                    self.jitter = self.rng.random_range(-1.0..1.0);
                }
                self.jitter *= 1.0 - dt * 8.0;
                let jitter = self.jitter as i32;
                let text = self.current_text();
                let shown: String = text.chars().take(self.chars).collect();
                let (scale_f, lines) = self.layout(&shown, w, h);
                let line_h = 8 * scale_f;
                let total_h = lines.len() as i32 * line_h - scale_f;
                let y0 = ((h as i32 - total_h) / 2).max(1);
                // roll-bar interference: a bright band sweeps down through the
                // text once mid-hold, dragging rows sideways as it passes
                let mut band_y: Option<i32> = None;
                if self.phase == Phase::Hold {
                    if let Some(ra) = self.roll_at {
                        let rp = (self.t - ra) / 0.55;
                        if (0.0..1.0).contains(&rp) {
                            band_y = Some(y0 + (ease_smooth(rp) * total_h as f32) as i32);
                        }
                    }
                }
                let bw = 2 * scale_f;
                let xoff = |py: i32| -> i32 {
                    match band_y {
                        Some(by) => {
                            let d = (py - by).abs();
                            if d <= bw {
                                (bw - d) * 2
                            } else {
                                0
                            }
                        }
                        None => 0,
                    }
                };
                let mut y = y0;
                for line in &lines {
                    let x = (((w as i32 - text_width_px(line, scale_f)) / 2) + jitter).max(1);
                    draw_text(canvas, line, x, y, scale_f, self.fg, &xoff);
                    y += line_h;
                }
                if let Some(by) = band_y {
                    for dy in 0..2 {
                        for x in 0..w as i32 {
                            canvas.add(x, by + dy, scale(self.fg, 0.05));
                        }
                    }
                }
                // phosphor softness: faint glow hugging the text block,
                // breathing gently while the card holds
                if scale_f >= 2 {
                    let mid_x = w as i32 / 2;
                    let mid_y = y0 + total_h / 2;
                    let breathe = if self.phase == Phase::Hold {
                        0.06 + 0.025 * (self.t * 1.3).sin()
                    } else {
                        0.06
                    };
                    glow(canvas, mid_x, mid_y, (total_h / 2 + 2).min(10), self.fg, breathe);
                }
                // blinking cursor while typing
                if self.phase == Phase::Type && (self.t * 3.0).fract() < 0.6 {
                    if let Some(last) = lines.last() {
                        let x = ((w as i32 - text_width_px(last, scale_f)) / 2).max(1)
                            + text_width_px(last, scale_f)
                            + scale_f;
                        draw_text(canvas, "_", x, y - line_h, scale_f, self.fg, &|_| 0);
                    }
                }
                // scrolling ticker along the bottom on some cards
                if self.card > 0 && self.card.is_multiple_of(3) && self.phase == Phase::Hold {
                    let tick = "+++ TERMPAPER NIGHTLY +++ ALL 31 SCENES UNREELING +++ ";
                    let off = (self.t * 12.0) as usize % tick.len();
                    let row = (h - 2) & !1;
                    let dim = scale(self.fg, 0.5);
                    for ci in 0..w.min(60) {
                        let ch = tick.chars().nth((off + ci) % tick.len()).unwrap_or(' ');
                        canvas.set_char(ci as i32, row as i32, ch, dim);
                    }
                }
                // occasional small footnote gag, terminal-glyph 1x text
                if let Some(note) = self.footnote {
                    if self.phase == Phase::Hold {
                        let row = (y + 4) & !1; // glyphs live on even rows
                        let x0 = (w.saturating_sub(note.len())) / 2;
                        let dim = scale(self.fg, 0.45);
                        for (ci, ch) in note.chars().enumerate() {
                            canvas.set_char((x0 + ci) as i32, row, ch, dim);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn glyph_scaling_dimensions() {
        // n chars at scale s: n*6s - s px wide
        assert_eq!(text_width_px("AB", 1), 11);
        assert_eq!(text_width_px("AB", 2), 22);
        assert_eq!(text_width_px("[TERMPAPER]", 2), 11 * 12 - 2);
        // every glyph is 5x7
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789[] .,'-?/:+".chars() {
            let g = glyph(ch);
            assert_eq!(g.len(), 7);
            assert!(g.iter().all(|r| r.chars().count() == 5), "glyph {ch}");
        }
    }

    #[test]
    fn wrapn_respects_width() {
        let lines = wrapn("YOUR WALLPAPER IS A TERMINAL. THIS IS FINE.", 12);
        assert!(lines.len() >= 3);
        assert!(lines.iter().all(|l| l.len() <= 12));
        assert_eq!(lines.join(" "), "YOUR WALLPAPER IS A TERMINAL. THIS IS FINE.");
    }

    #[test]
    fn sequence_progresses_and_renders_big_text() {
        let mut b = Bump::new(StdRng::seed_from_u64(4), None, None);
        let mut c = Canvas::new(160, 60);
        // type out the title fully
        for _ in 0..100 {
            b.update(1.0 / 30.0, &mut c);
        }
        assert_eq!(b.chars, TITLE.len());
        // count lit pixels: 2x block text of the title should be substantial
        let lit = (0..160)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| c.get(x, y).color == (232, 232, 232))
            .count();
        assert!(lit > 300, "2x title should light many pixels, got {lit}");
        // run a long while without panic (covers wrap/static/footnote paths)
        for _ in 0..3000 {
            b.update(1.0 / 30.0, &mut c);
        }
        assert!(b.card >= 1);
    }
}
