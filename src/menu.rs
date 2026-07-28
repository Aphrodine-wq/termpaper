//! The "?" settings menu: a modal, centered box over the live scene.
//! Small state machine — logic here is tested without a terminal.

use crate::color_wheel;
use crate::filter::FILTER_CYCLE;
use crate::link::{self, GROUP_PRESETS};
use crate::scene::{self, Detail};
use crate::render::Pixels;

pub const SECTIONS: &[&str] = &[
    "Scenes",
    "Marketplace",
    "Instances",
    "Settings",
    "Keybinds",
    "About",
];

/// Fixed settings rows; filter toggles are appended after these.
pub const SETTINGS_ROWS: &[&str] = &[
    "Pixels",
    "Detail",
    "Theme",
    "Hue",
    "Saturation",
    "Contrast",
    "Link",
    "Group",
    "Text scale",
    "Speed",
    "FPS",
    "Smooth",
    "Dim",
    "Fade",
    "Clock",
    "Cycle",
];

fn next_group(cur: &str, up: bool) -> String {
    let cur = link::sanitize_group(cur);
    let pos = GROUP_PRESETS.iter().position(|&g| g == cur.as_str());
    match (pos, up) {
        (None, true) => GROUP_PRESETS[0].to_string(),
        (None, false) => cur,
        (Some(i), true) => GROUP_PRESETS[(i + 1) % GROUP_PRESETS.len()].to_string(),
        (Some(0), false) => cur,
        (Some(i), false) => GROUP_PRESETS[i - 1].to_string(),
    }
}

/// Auto-rotate intervals the Cycle row steps through (seconds).
const CYCLE_STEPS: &[f64] = &[15.0, 30.0, 60.0, 120.0];

/// Step the cycle interval up/down through off → 15 → 30 → 60 → 120s,
/// clamping at both ends.
fn cycle_step(cur: Option<f64>, up: bool) -> Option<f64> {
    let pos = cur.and_then(|c| CYCLE_STEPS.iter().position(|&s| (s - c).abs() < 0.001));
    match (pos, up) {
        (None, true) => Some(CYCLE_STEPS[0]),
        (None, false) => None,
        (Some(i), true) => Some(CYCLE_STEPS[(i + 1).min(CYCLE_STEPS.len() - 1)]),
        (Some(0), false) => None,
        (Some(i), false) => Some(CYCLE_STEPS[i - 1]),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    Enter,
}

/// Side effects the host must apply (and persist where noted).
#[derive(Clone, PartialEq, Debug)]
pub enum Effect {
    SwitchScene(usize),
    SetPixels(Pixels),
    SetDetail(Detail),
    SetTheme(Option<String>),
    SetTextScale(Option<u32>),
    SetSpeed(f32),
    SetFps(u32),
    SetSmooth(f32),
    SetDim(f32),
    SetFade(f32),
    SetClock(bool),
    /// auto-rotate interval in seconds; None = off (local-only, not linked)
    SetCycle(Option<f64>),
    SetHueShift(f32),
    SetSaturation(f32),
    SetContrast(f32),
    SetLink(bool),
    SetLinkGroup(String),
    ToggleFilter(String),
    Persist,
}

pub struct Menu {
    pub open: bool,
    pub section: usize,
    pub scene_cursor: usize,
    pub settings_cursor: usize,
}

impl Menu {
    pub fn new() -> Self {
        Menu {
            open: false,
            section: 0,
            scene_cursor: 0,
            settings_cursor: 0,
        }
    }

    pub fn toggle(&mut self, current_scene: usize) {
        self.open = !self.open;
        if self.open {
            self.scene_cursor = current_scene;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    fn settings_row_count() -> usize {
        SETTINGS_ROWS.len() + FILTER_CYCLE.len()
    }

    /// Current settings context used to render/adjust rows.
    pub fn handle(&mut self, input: Input, ctx: &MenuCtx) -> Vec<Effect> {
        let mut fx = Vec::new();
        match self.section {
            0 => self.handle_scenes(input, &mut fx),
            3 => self.handle_settings(input, ctx, &mut fx),
            _ => {
                match input {
                    Input::Left | Input::Up => self.nav_section(false),
                    Input::Right | Input::Down => self.nav_section(true),
                    _ => {}
                }
            }
        }
        fx
    }

    fn handle_scenes(&mut self, input: Input, fx: &mut Vec<Effect>) {
        let n = scene::catalog().len();
        match input {
            Input::Up => self.scene_cursor = (self.scene_cursor + n - 1) % n,
            Input::Down => self.scene_cursor = (self.scene_cursor + 1) % n,
            Input::Left => self.section = (self.section + SECTIONS.len() - 1) % SECTIONS.len(),
            Input::Right => self.section = (self.section + 1) % SECTIONS.len(),
            Input::Enter => {
                fx.push(Effect::SwitchScene(self.scene_cursor));
                fx.push(Effect::Persist);
                self.close();
            }
        }
    }

    fn handle_settings(&mut self, input: Input, ctx: &MenuCtx, fx: &mut Vec<Effect>) {
        let rows = Self::settings_row_count();
        match input {
            Input::Up => {
                if self.settings_cursor == 0 {
                    self.nav_section(false);
                } else {
                    self.settings_cursor -= 1;
                }
            }
            Input::Down => {
                if self.settings_cursor == rows - 1 {
                    self.nav_section(true);
                } else {
                    self.settings_cursor += 1;
                }
            }
            Input::Left | Input::Right | Input::Enter => {
                let right = input != Input::Left;
                let row = self.settings_cursor;
                match row {
                    0 => fx.push(Effect::SetPixels(if right {
                        ctx.pixels.next()
                    } else {
                        ctx.pixels.next().next()
                    })),
                    1 => fx.push(Effect::SetDetail(ctx.detail.next())),
                    2 => {
                        let themes = scene::themes(ctx.scene_name);
                        let next = match &ctx.theme {
                            None => themes.first().map(|s| s.to_string()),
                            Some(cur) => themes
                                .iter()
                                .position(|t| t == cur)
                                .map(|i| themes[(i + 1) % themes.len()].to_string())
                                .or_else(|| themes.first().map(|s| s.to_string())),
                        };
                        fx.push(Effect::SetTheme(next));
                    }
                    3 => {
                        let d = if right {
                            color_wheel::HUE_STEP
                        } else {
                            -color_wheel::HUE_STEP
                        };
                        fx.push(Effect::SetHueShift(color_wheel::step_hue(ctx.hue_shift, d)));
                    }
                    4 => {
                        let d = if right {
                            color_wheel::SAT_STEP
                        } else {
                            -color_wheel::SAT_STEP
                        };
                        fx.push(Effect::SetSaturation(color_wheel::step_sat(
                            ctx.saturation,
                            d,
                        )));
                    }
                    5 => {
                        let d = if right {
                            color_wheel::CONTRAST_STEP
                        } else {
                            -color_wheel::CONTRAST_STEP
                        };
                        fx.push(Effect::SetContrast(color_wheel::step_contrast(
                            ctx.contrast,
                            d,
                        )));
                    }
                    6 => fx.push(Effect::SetLink(!ctx.link_enabled)),
                    7 => {
                        if ctx.link_enabled {
                            fx.push(Effect::SetLinkGroup(next_group(&ctx.link_group, right)));
                        }
                    }
                    8 => {
                        let next = match ctx.text_scale {
                            None => Some(1),
                            Some(1) => Some(2),
                            Some(2) => Some(3),
                            _ => None,
                        };
                        fx.push(Effect::SetTextScale(next));
                    }
                    9 => {
                        let d = if right { 0.25 } else { -0.25 };
                        fx.push(Effect::SetSpeed((ctx.speed + d).clamp(0.25, 4.0)));
                    }
                    10 => {
                        let d = if right { 5 } else { -5 };
                        fx.push(Effect::SetFps((ctx.fps as i32 + d).clamp(10, 120) as u32));
                    }
                    11 => {
                        let d = if right { 0.1 } else { -0.1 };
                        fx.push(Effect::SetSmooth((ctx.smooth + d).clamp(0.0, 0.9)));
                    }
                    12 => {
                        let d = if right { 0.1 } else { -0.1 };
                        fx.push(Effect::SetDim((ctx.dim + d).clamp(0.2, 1.0)));
                    }
                    13 => {
                        let d = if right { 0.05 } else { -0.05 };
                        fx.push(Effect::SetFade((ctx.fade + d).clamp(0.1, 1.0)));
                    }
                    14 => fx.push(Effect::SetClock(!ctx.clock)),
                    15 => fx.push(Effect::SetCycle(cycle_step(ctx.cycle, right))),
                    _ => {
                        let fi = row - SETTINGS_ROWS.len();
                        fx.push(Effect::ToggleFilter(FILTER_CYCLE[fi].to_string()));
                    }
                }
                fx.push(Effect::Persist);
            }
        }
    }

    /// Section navigation from the non-list sections.
    pub fn nav_section(&mut self, right: bool) {
        self.section = if right {
            (self.section + 1) % SECTIONS.len()
        } else {
            (self.section + SECTIONS.len() - 1) % SECTIONS.len()
        };
    }
}

/// Snapshot of the host's current settings, for display + adjust.
pub struct MenuCtx {
    pub scene_name: &'static str,
    pub scene_idx: usize,
    pub pixels: Pixels,
    pub detail: Detail,
    pub theme: Option<String>,
    pub text_scale: Option<u32>,
    pub speed: f32,
    pub fps: u32,
    pub smooth: f32,
    pub dim: f32,
    pub fade: f32,
    pub clock: bool,
    pub cycle: Option<f64>,
    pub hue_shift: f32,
    pub saturation: f32,
    pub contrast: f32,
    pub link_enabled: bool,
    pub link_group: String,
    pub truecolor: bool,
    pub filters: Vec<String>,
    pub key_display: Vec<(String, String)>,
    pub instances: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> MenuCtx {
        MenuCtx {
            scene_name: "nexus",
            scene_idx: 1,
            pixels: Pixels::Half,
            detail: Detail::Medium,
            theme: Some("cyan".into()),
            text_scale: None,
            speed: 1.0,
            fps: 30,
            smooth: 0.3,
            dim: 1.0,
            fade: 0.25,
            clock: true,
            cycle: None,
            hue_shift: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            link_enabled: true,
            link_group: "default".into(),
            truecolor: true,
            filters: vec!["scanlines".into()],
            key_display: vec![("quit".into(), "q".into())],
            instances: vec![],
        }
    }

    #[test]
    fn settings_adjust_and_toggle() {
        let mut m = Menu::new();
        m.toggle(0);
        m.section = 3;
        m.settings_cursor = SETTINGS_ROWS.len();
        let fx = m.handle(Input::Enter, &ctx());
        assert!(fx.contains(&Effect::ToggleFilter("scanlines".into())));
        m.settings_cursor = 10;
        let mut c = ctx();
        c.fps = 10;
        let fx = m.handle(Input::Left, &c);
        assert!(fx.contains(&Effect::SetFps(10)));
    }

    #[test]
    fn settings_edges_leave_section() {
        let mut m = Menu::new();
        m.toggle(0);
        m.section = 3;
        m.settings_cursor = 0;
        let _ = m.handle(Input::Up, &ctx());
        assert_eq!(m.section, 2, "up on first row goes to previous section");
        m.section = 3;
        m.settings_cursor = Menu::settings_row_count() - 1;
        let _ = m.handle(Input::Down, &ctx());
        assert_eq!(m.section, 4, "down on last row goes to next section");
    }
}

pub mod view {
    use super::{Menu, MenuCtx, SECTIONS, SETTINGS_ROWS};
    use crate::{brand, color_wheel, filter::FILTER_CYCLE, scene};
    use ratatui::{
        layout::{Alignment, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, Paragraph, Wrap},
        Frame,
    };

    fn dim(s: impl Into<String>) -> Span<'static> {
        Span::styled(s.into(), Style::new().fg(Color::DarkGray))
    }
    fn hot(s: impl Into<String>) -> Span<'static> {
        Span::styled(
            s.into(),
            Style::new().fg(Color::Black).bg(Color::Gray).add_modifier(Modifier::BOLD),
        )
    }

    pub fn render(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx) {
        let w = (area.width * 3 / 5).clamp(30, area.width);
        let h = (area.height * 75 / 100).clamp(14, area.height);
        let rect = Rect {
            x: area.x + (area.width - w) / 2,
            y: area.y + (area.height - h) / 2,
            width: w,
            height: h,
        };
        f.render_widget(Clear, rect);
        let title = format!(" termpaper · v{} ", brand::VERSION);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(Color::DarkGray))
            .style(Style::new().bg(Color::Rgb(0, 0, 0)))
            .title(Span::styled(
                title,
                Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let stats = brand::MenuStats {
            scene: ctx.scene_name,
            theme: ctx.theme.as_deref(),
            pixels: ctx.pixels.name(),
            fps: ctx.fps,
            speed: ctx.speed,
            link_enabled: ctx.link_enabled,
            link_group: &ctx.link_group,
            truecolor: ctx.truecolor,
            scene_count: scene::catalog().len(),
        };

        let mut tabs: Vec<Span> = Vec::new();
        for (i, s) in SECTIONS.iter().enumerate() {
            if i == m.section {
                tabs.push(hot(format!(" {s} ")));
            } else {
                tabs.push(dim(format!(" {s} ")));
            }
            tabs.push(dim(" "));
        }
        let mut lines = brand::compact_lines(&stats);
        lines.push(Line::from(tabs));
        lines.push(Line::from(""));

        match m.section {
            0 => {
                let visible = (inner.height as usize).saturating_sub(6).max(4);
                let catalog = scene::catalog();
                let total = catalog.len();
                let start = m
                    .scene_cursor
                    .saturating_sub(visible / 2)
                    .min(total.saturating_sub(visible));
                for (i, (name, desc)) in catalog.iter().enumerate().skip(start).take(visible) {
                    let cur = if i == ctx.scene_idx { "●" } else { " " };
                    let row = format!("{cur} {name:<12} {desc}");
                    if i == m.scene_cursor {
                        lines.push(Line::from(hot(row)));
                    } else {
                        lines.push(Line::from(Span::styled(
                            row,
                            Style::new().fg(if i == ctx.scene_idx { Color::Green } else { Color::Gray }),
                        )));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(dim(format!(
                    "{} of {} · ↑/↓ browse · Enter switch",
                    m.scene_cursor + 1,
                    catalog.len()
                ))));
            }
            1 => {
                for line in crate::marketplace::menu_lines() {
                    lines.push(Line::from(Span::styled(
                        line,
                        Style::new().fg(Color::Gray),
                    )));
                }
            }
            2 => {
                if ctx.instances.is_empty() {
                    lines.push(Line::from(dim("no other live instances")));
                } else {
                    for line in &ctx.instances {
                        lines.push(Line::from(Span::styled(line.clone(), Style::new().fg(Color::Gray))));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(dim("termpaper --no-link          solo art (no sync)")));
                lines.push(Line::from(dim("termpaper --group NAME     separate sync group")));
                lines.push(Line::from(dim("termpaper --switch X --group wallpaper")));
            }
            3 => {
                let theme_disp = ctx.theme.clone().unwrap_or_else(|| "default".into());
                let ts_disp = ctx
                    .text_scale
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "auto".into());
                let values = [
                    ctx.pixels.name().to_string(),
                    ctx.detail.name().to_string(),
                    theme_disp,
                    color_wheel::format_hue(ctx.hue_shift),
                    color_wheel::format_sat(ctx.saturation),
                    color_wheel::format_contrast(ctx.contrast),
                    if ctx.link_enabled {
                        "on".into()
                    } else {
                        "off (solo)".into()
                    },
                    if ctx.link_enabled {
                        ctx.link_group.clone()
                    } else {
                        "—".into()
                    },
                    ts_disp,
                    format!("{:.2}x", ctx.speed),
                    format!("{}", ctx.fps),
                    format!("{:.1}", ctx.smooth),
                    format!("{:.1}", ctx.dim),
                    format!("{:.2}s", ctx.fade),
                    if ctx.clock { "on".into() } else { "off".into() },
                    ctx.cycle
                        .map(|c| format!("{:.0}s", c))
                        .unwrap_or_else(|| "off".into()),
                ];
                for (i, row) in SETTINGS_ROWS.iter().enumerate() {
                    let line = format!("{row:<12} ◂ {} ▸", values[i]);
                    if i == m.settings_cursor {
                        lines.push(Line::from(hot(line)));
                    } else {
                        lines.push(Line::from(Span::styled(line, Style::new().fg(Color::Gray))));
                    }
                }
                lines.push(Line::from(dim("─ filters ─")));
                for (fi, fname) in FILTER_CYCLE.iter().enumerate() {
                    let on = ctx.filters.iter().any(|f| f == fname);
                    let line = format!("{} [{}]", fname, if on { "x" } else { " " });
                    let row = SETTINGS_ROWS.len() + fi;
                    if row == m.settings_cursor {
                        lines.push(Line::from(hot(line)));
                    } else {
                        lines.push(Line::from(Span::styled(
                            line,
                            Style::new().fg(if on { Color::Green } else { Color::DarkGray }),
                        )));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(dim("↑/↓ rows · ◂/▸ adjust · 0 reset all · c color grade · Esc close · saved automatically")));
                if (3..=5).contains(&m.settings_cursor) {
                    lines.push(Line::from(""));
                    lines.push(Line::from(dim("press c for the 100-step color wheel")));
                }
                if m.settings_cursor == 7 && !ctx.link_enabled {
                    lines.push(Line::from(dim("enable Link to sync with a group")));
                }
            }
            4 => {
                for (action, key) in &ctx.key_display {
                    lines.push(Line::from(Span::styled(
                        format!("{action:<14} {key}"),
                        Style::new().fg(Color::Gray),
                    )));
                }
            }
            5 => {
                lines.extend(brand::about_lines(&stats));
            }
            _ => {}
        }

        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left)
            .style(Style::new().fg(Color::Gray));
        f.render_widget(para, inner);
    }
}
