//! Global hue/saturation/contrast picker UI. Used from settings and the `c` key.
//! The hue ring shows 100 discrete steps around a white center hole.

use crate::canvas::hsv;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
    Frame,
};

pub const HUE_STEPS: usize = 100;
pub const HUE_STEP: f32 = 360.0 / HUE_STEPS as f32;
pub const SAT_STEP: f32 = 0.15;
pub const CONTRAST_STEP: f32 = 0.15;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Param {
    Hue,
    Saturation,
    Contrast,
}

pub fn step_hue(cur: f32, delta: f32) -> f32 {
    step_hue_steps(cur, (delta / HUE_STEP).round() as i32)
}

pub fn step_hue_steps(cur: f32, steps: i32) -> f32 {
    if steps == 0 {
        return cur;
    }
    if cur < 0.5 {
        if steps > 0 {
            return (steps as f32 * HUE_STEP).min(360.0).max(HUE_STEP);
        }
        return 0.0;
    }
    let idx = ((cur / HUE_STEP).round() as i32).rem_euclid(HUE_STEPS as i32);
    let next = (idx + steps).rem_euclid(HUE_STEPS as i32);
    if next == 0 && steps < 0 {
        0.0
    } else {
        (next as f32 * HUE_STEP).max(HUE_STEP)
    }
}

pub fn step_sat(cur: f32, delta: f32) -> f32 {
    (cur + delta).clamp(0.0, 2.5)
}

pub fn step_contrast(cur: f32, delta: f32) -> f32 {
    (cur + delta).clamp(0.5, 2.5)
}

pub fn format_hue(h: f32) -> String {
    if h < 0.5 {
        "off".into()
    } else {
        format!("{:.0}°", h)
    }
}

pub fn format_sat(s: f32) -> String {
    format!("{:.0}%", s * 100.0)
}

pub fn format_contrast(c: f32) -> String {
    format!("{:.2}x", c)
}

fn paint(buf: &mut Buffer, x: u16, y: u16, ch: char, fg: Color, bg: Color) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char(ch);
        cell.set_fg(fg);
        cell.set_bg(bg);
    }
}

fn draw_wheel(buf: &mut Buffer, area: Rect, hue_deg: f32, sat: f32, active: Param) {
    if area.width < 8 || area.height < 6 {
        return;
    }
    let cx = area.x as f32 + area.width as f32 * 0.5 - 0.5;
    let cy = area.y as f32 + area.height as f32 * 0.5 - 0.5;
    let outer = (area.width.min(area.height) as f32 * 0.48).max(4.0);
    let inner = outer * 0.44;
    let ring_mid = (inner + outer) * 0.5;
    let sat_mul = sat.clamp(0.0, 1.0);
    let sel_idx = if hue_deg >= 0.5 {
        ((hue_deg / HUE_STEP).round() as i32).rem_euclid(HUE_STEPS as i32)
    } else {
        -1
    };

    for py in 0..area.height {
        for px in 0..area.width {
            let x = px as f32 + area.x as f32 - cx;
            let y = py as f32 + area.y as f32 - cy;
            let dist = (x * x + y * y).sqrt();
            let cell_x = area.x + px;
            let cell_y = area.y + py;

            if dist < inner {
                paint(buf, cell_x, cell_y, ' ', Color::Rgb(248, 248, 252), Color::Rgb(248, 248, 252));
            } else if dist <= outer {
                let ang = (-y).atan2(x);
                let mut deg = ang.to_degrees();
                if deg < 0.0 {
                    deg += 360.0;
                }
                let idx = ((deg / 360.0) * HUE_STEPS as f32).round() as i32 % HUE_STEPS as i32;
                let step_h = idx as f32 / HUE_STEPS as f32;
                let (r, g, b) = hsv(step_h, 0.90 * sat_mul + 0.08, 0.94);
                let on_ring = (dist - ring_mid).abs() <= 0.75;
                let ch = if active == Param::Hue && idx == sel_idx && on_ring {
                    '◉'
                } else {
                    '·'
                };
                paint(
                    buf,
                    cell_x,
                    cell_y,
                    ch,
                    Color::Rgb(r, g, b),
                    Color::Rgb(0, 0, 0),
                );
            }
        }
    }
}

pub fn render(
    f: &mut Frame,
    area: Rect,
    hue: f32,
    saturation: f32,
    contrast: f32,
    active: Param,
) {
    let w = area.width.min(54).max(40);
    let h = area.height.min(30).max(22);
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    Clear.render(rect, f.buffer_mut());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::DarkGray))
        .style(Style::new().bg(Color::Rgb(0, 0, 0)))
        .title(Span::styled(
            " color grade ",
            Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    block.render(rect, f.buffer_mut());

    let wheel_h = inner.height.saturating_sub(5).max(8);
    let wheel = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: wheel_h,
    };
    draw_wheel(f.buffer_mut(), wheel, hue, saturation, active);

    fn row(label: &str, value: &str, on: bool) -> Line<'static> {
        let style = if on {
            Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::Gray)
        };
        Line::from(vec![
            Span::styled(format!("{label:<11}"), style),
            Span::styled(format!("◂ {value} ▸"), Style::new().fg(Color::Gray)),
        ])
    }

    let text_y = inner.y + wheel_h;
    let text_h = inner.height.saturating_sub(wheel_h);
    let text_rect = Rect {
        x: inner.x,
        y: text_y,
        width: inner.width,
        height: text_h,
    };
    let lines = vec![
        row("Hue", &format_hue(hue), active == Param::Hue),
        row("Saturation", &format_sat(saturation), active == Param::Saturation),
        row("Contrast", &format_contrast(contrast), active == Param::Contrast),
        Line::from(Span::styled(
            "Tab param · ◂/▸ · ↑/↓×10 · 0 reset grade · c/Esc close",
            Style::new().fg(Color::DarkGray),
        )),
    ];
    Paragraph::new(lines)
        .style(Style::new().fg(Color::Gray))
        .render(text_rect, f.buffer_mut());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_steps_wrap_at_100() {
        assert!((step_hue_steps(350.0, 5) - 7.2).abs() < 0.01);
        assert_eq!(step_hue_steps(3.6, -1), 0.0);
        assert_eq!(step_hue_steps(0.0, 5), 18.0);
    }
}
