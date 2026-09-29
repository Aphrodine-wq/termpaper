//! What is drawn over the scene: the clock (small text, or large digits
//! that lighten the picture under them), the scene's name after a switch,
//! and the performance readout. Each draws straight into the frame's
//! buffer after the scene, so it costs nothing when off.

use crate::platform::LocalTime;
use crate::prefs::{ClockFormat, ClockStyle, Corner};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// Monday first, as `LocalTime::weekday` counts.
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The clock's text: `14:05`, `2:05 pm`, `14:05:09`, or `Tue 29 Sep  14:05`.
pub fn clock_text(t: &LocalTime, format: ClockFormat) -> String {
    match format {
        ClockFormat::H24 => format!("{:02}:{:02}", t.hour, t.minute),
        ClockFormat::H12 => {
            let h = match t.hour % 12 {
                0 => 12,
                h => h,
            };
            format!("{h}:{:02} {}", t.minute, if t.hour < 12 { "am" } else { "pm" })
        }
        ClockFormat::Seconds => format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second),
        ClockFormat::Date => format!(
            "{} {} {}  {:02}:{:02}",
            WEEKDAYS[t.weekday as usize % 7],
            t.day,
            MONTHS[(t.month as usize).saturating_sub(1) % 12],
            t.hour,
            t.minute
        ),
    }
}

/// Where a `w`×`h` box goes in `area`, one cell in from the edges.
pub fn place(area: Rect, w: u16, h: u16, corner: Corner) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let right = area.x + area.width.saturating_sub(w);
    let bottom = area.y + area.height.saturating_sub(h);
    let (x, y) = match corner {
        Corner::TopLeft => (area.x + 1.min(area.width - w), area.y),
        Corner::TopRight => (right.saturating_sub(1).max(area.x), area.y),
        Corner::BottomLeft => (area.x + 1.min(area.width - w), bottom),
        Corner::BottomRight => (right.saturating_sub(1).max(area.x), bottom),
        Corner::Center => (area.x + (area.width - w) / 2, area.y + (area.height - h) / 2),
    };
    Rect::new(x, y, w, h)
}

fn grey(level: f32) -> Color {
    let g = (level.clamp(0.0, 1.0) * 255.0) as u8;
    Color::Rgb(g, g, g.saturating_add(10))
}

/// Small text in a corner, dim grey scaled by `brightness`.
pub fn draw_text(buf: &mut Buffer, area: Rect, text: &str, corner: Corner, brightness: f32) {
    let w = text.chars().count() as u16;
    if w == 0 || area.width == 0 || area.height == 0 || brightness * 0.36 < 0.03 {
        return;
    }
    let r = place(area, w, 1, corner);
    buf.set_stringn(r.x, r.y, text, r.width as usize, Style::new().fg(grey(0.36 * brightness)));
}

/// 3×5 digits for the large clock, a row per string, `#` lit.
fn glyph(c: char) -> Option<[&'static str; 5]> {
    Some(match c {
        '0' => ["###", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["###", "..#", "###", "#..", "###"],
        '3' => ["###", "..#", "###", "..#", "###"],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "###", "..#", "###"],
        '6' => ["###", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", "..#", "..#", "..#"],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "###"],
        ':' => [".", "#", ".", "#", "."],
        ' ' => [".", ".", ".", ".", "."],
        _ => return None,
    })
}

/// Lighten a colour toward white by `k` (0..1).
fn lighten(c: Color, k: f32) -> Color {
    match c {
        Color::Rgb(r, g, b) => {
            let up = |v: u8| (v as f32 + (255.0 - v as f32) * k) as u8;
            Color::Rgb(up(r), up(g), up(b))
        }
        other => other,
    }
}

/// The large clock's size in cells for `text` (digits two cells wide per
/// font pixel, one row per pixel, a gap between glyphs).
pub fn large_size(text: &str) -> (u16, u16) {
    let w: usize = text.chars().filter_map(glyph).map(|g| g[0].len() * 2 + 2).sum();
    (w.saturating_sub(2) as u16, 5)
}

/// Large digits that lighten the scene under them: frosted glass, legible
/// on any picture. Only digits, colons and spaces are drawn (12-hour and
/// date suffixes go small, under the digits).
pub fn draw_large(buf: &mut Buffer, area: Rect, text: &str, corner: Corner, brightness: f32) {
    let digits: String = text.chars().take_while(|c| glyph(*c).is_some()).collect();
    let digits = digits.trim_end();
    let rest = text[digits.len()..].trim();
    let (w, h) = large_size(digits);
    if w == 0 || area.width < w || area.height < h + 1 {
        return draw_text(buf, area, text, corner, brightness);
    }
    let r = place(area, w, h + u16::from(!rest.is_empty()), corner);
    let k = 0.42 * brightness.clamp(0.0, 1.0);
    let mut x = r.x;
    for c in digits.chars() {
        let Some(g) = glyph(c) else {
            continue;
        };
        for (row, line) in g.iter().enumerate() {
            for (col, px) in line.chars().enumerate() {
                if px != '#' {
                    continue;
                }
                for dx in 0..2 {
                    let (cx, cy) = (x + col as u16 * 2 + dx, r.y + row as u16);
                    if let Some(cell) = buf.cell_mut((cx, cy)) {
                        cell.fg = lighten(cell.fg, k);
                        cell.bg = lighten(cell.bg, k);
                    }
                }
            }
        }
        x += g[0].len() as u16 * 2 + 2;
    }
    if !rest.is_empty() {
        let tr = Rect::new(r.x, r.y + h, r.width, 1);
        let x = tr.x + tr.width.saturating_sub(rest.chars().count() as u16);
        buf.set_stringn(x, tr.y, rest, tr.width as usize, Style::new().fg(grey(0.5 * brightness)));
    }
}

/// The clock in its style.
pub fn draw_clock(buf: &mut Buffer, area: Rect, text: &str, style: ClockStyle, corner: Corner, brightness: f32) {
    match style {
        ClockStyle::Small => draw_text(buf, area, text, corner, brightness),
        ClockStyle::Large => draw_large(buf, area, text, corner, brightness),
    }
}

/// How visible the scene caption is `age` seconds after a switch: in over
/// half a second, held, out by four.
pub fn caption_alpha(age: f32) -> f32 {
    if !(0.0..4.0).contains(&age) {
        0.0
    } else if age < 0.5 {
        age / 0.5
    } else if age < 3.0 {
        1.0
    } else {
        4.0 - age
    }
}

/// The scene's name and variant, bottom left, fading with `alpha`.
pub fn draw_caption(buf: &mut Buffer, area: Rect, title: &str, variant: Option<&str>, alpha: f32) {
    if alpha <= 0.02 || area.height < 3 || area.width < 8 {
        return;
    }
    let y = area.y + area.height - 2;
    let x = area.x + 2;
    let room = area.width.saturating_sub(4) as usize;
    buf.set_stringn(x, y, title, room, Style::new().fg(grey(0.85 * alpha)).add_modifier(ratatui::style::Modifier::BOLD));
    if let Some(v) = variant {
        let used = title.chars().count() as u16 + 3;
        if (used as usize) < room {
            buf.set_stringn(x + used, y, v, room - used as usize, Style::new().fg(grey(0.5 * alpha)));
        }
    }
}

/// Frame-rate and cost readout, top left: a line per entry.
pub fn draw_hud(buf: &mut Buffer, area: Rect, lines: &[String]) {
    for (i, l) in lines.iter().enumerate().take(area.height as usize) {
        let w = l.chars().count() as u16 + 2;
        let r = Rect::new(area.x, area.y + i as u16, w.min(area.width), 1);
        for x in r.x..r.x + r.width {
            if let Some(cell) = buf.cell_mut((x, r.y)) {
                cell.set_symbol(" ").set_bg(Color::Rgb(12, 13, 18)).set_fg(Color::Rgb(170, 176, 190));
            }
        }
        buf.set_stringn(r.x + 1, r.y, l, r.width.saturating_sub(1) as usize, Style::new().fg(Color::Rgb(170, 176, 190)).bg(Color::Rgb(12, 13, 18)));
    }
}

/// Bytes per second as people read them.
pub fn rate(bytes_per_sec: f64) -> String {
    match bytes_per_sec {
        b if b >= 1e6 => format!("{:.1} MB/s", b / 1e6),
        b if b >= 1e3 => format!("{:.0} kB/s", b / 1e3),
        b => format!("{b:.0} B/s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32, minute: u32, second: u32) -> LocalTime {
        LocalTime { year: 2026, month: 9, day: 29, weekday: 1, hour, minute, second }
    }

    #[test]
    fn clock_formats() {
        assert_eq!(clock_text(&at(14, 5, 9), ClockFormat::H24), "14:05");
        assert_eq!(clock_text(&at(14, 5, 9), ClockFormat::H12), "2:05 pm");
        assert_eq!(clock_text(&at(0, 30, 0), ClockFormat::H12), "12:30 am");
        assert_eq!(clock_text(&at(12, 0, 0), ClockFormat::H12), "12:00 pm");
        assert_eq!(clock_text(&at(14, 5, 9), ClockFormat::Seconds), "14:05:09");
        assert_eq!(clock_text(&at(14, 5, 9), ClockFormat::Date), "Tue 29 Sep  14:05");
    }

    #[test]
    fn places_stay_inside() {
        let a = Rect::new(0, 0, 80, 24);
        for c in Corner::ALL {
            let r = place(a, 10, 5, *c);
            assert!(r.right() <= 80 && r.bottom() <= 24, "{c:?} {r:?}");
        }
        assert_eq!(place(a, 10, 1, Corner::TopRight), Rect::new(69, 0, 10, 1));
        assert_eq!(place(a, 10, 5, Corner::Center), Rect::new(35, 9, 10, 5));
        // bigger than the area: clipped, not out of bounds
        let r = place(Rect::new(0, 0, 4, 2), 10, 5, Corner::BottomRight);
        assert!(r.width <= 4 && r.height <= 2);
    }

    #[test]
    fn large_digits_lighten_and_fall_back_when_cramped() {
        let area = Rect::new(0, 0, 60, 12);
        let mut buf = Buffer::empty(area);
        for y in 0..12 {
            for x in 0..60 {
                buf[(x, y)].set_symbol("▀").set_fg(Color::Rgb(20, 40, 60)).set_bg(Color::Rgb(20, 40, 60));
            }
        }
        draw_large(&mut buf, area, "12:30 pm", Corner::TopLeft, 1.0);
        let lit = (0..12).flat_map(|y| (0..60).map(move |x| (x, y))).filter(|&(x, y)| buf[(x, y)].fg != Color::Rgb(20, 40, 60)).count();
        assert!(lit > 40, "{lit}");
        // the suffix goes small under the digits
        let text: String = (0..60).map(|x| buf[(x, 5)].symbol().to_string()).collect();
        assert!(text.contains("pm"), "{text}");
        assert_eq!(large_size("12:30"), (34, 5));
        // too small for digits: the plain text instead
        let tiny = Rect::new(0, 0, 10, 3);
        let mut b = Buffer::empty(tiny);
        draw_large(&mut b, tiny, "12:30", Corner::TopLeft, 1.0);
        let row: String = (0..10).map(|x| b[(x, 0)].symbol().to_string()).collect();
        assert!(row.contains("12:30"), "{row}");
    }

    #[test]
    fn caption_fades_in_holds_and_out() {
        assert_eq!(caption_alpha(-1.0), 0.0);
        assert!((caption_alpha(0.25) - 0.5).abs() < 1e-6);
        assert_eq!(caption_alpha(2.0), 1.0);
        assert!((caption_alpha(3.5) - 0.5).abs() < 1e-6);
        assert_eq!(caption_alpha(4.5), 0.0);
        assert_eq!(rate(2_500_000.0), "2.5 MB/s");
        assert_eq!(rate(1234.0), "1 kB/s");
    }
}
