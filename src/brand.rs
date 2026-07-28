//! fastfetch-style branding for the settings menu.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Compact logo for the menu header strip (~34 cols).
pub const LOGO_COMPACT: &[&str] = &[
    "┏━━━ termpaper ━━━━━━━━━━━━━━━━━━━┓",
    "┃  wallpaper engine for the tty   ┃",
    "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
];

/// Full logo for the About tab.
pub const LOGO_FULL: &[&str] = &[
    "  ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
    "  ┃                                       ┃",
    "  ┃   ████████╗███████╗██████╗ ███╗   ███╗",
    "  ┃   ╚══██╔══╝██╔════╝██╔══██╗████╗ ████║",
    "  ┃      ██║   █████╗  ██████╔╝██╔████╔██║",
    "  ┃      ██║   ██╔══╝  ██╔══██╗██║╚██╔╝██║",
    "  ┃      ██║   ███████╗██║  ██║██║ ╚═╝ ██║",
    "  ┃      ╚═╝   ╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝",
    "  ┃   ██████╗  █████╗ ██████╗ ███████╗██████╗ ",
    "  ┃   ██╔══██╗██╔══██╗██╔══██╗██╔════╝██╔══██╗",
    "  ┃   ██████╔╝███████║██████╔╝█████╗  ██████╔╝",
    "  ┃   ██╔═══╝ ██╔══██║██╔═══╝ ██╔══╝  ██╔══██╗",
    "  ┃   ██║     ██║  ██║██║     ███████╗██║  ██║",
    "  ┃   ╚═╝     ╚═╝  ╚═╝╚═╝     ╚══════╝╚═╝  ╚═╝",
    "  ┃                                       ┃",
    "  ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
];

pub struct MenuStats<'a> {
    pub scene: &'a str,
    pub theme: Option<&'a str>,
    pub pixels: &'a str,
    pub fps: u32,
    pub speed: f32,
    pub link_enabled: bool,
    pub link_group: &'a str,
    pub truecolor: bool,
    pub scene_count: usize,
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::new().fg(Color::DarkGray))
}

fn label(key: &str, val: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<10}"), Style::new().fg(Color::Cyan)),
        Span::styled(val.into(), Style::new().fg(Color::Gray)),
    ])
}

fn logo_line(s: &str, accent: bool) -> Line<'static> {
    if accent {
        Line::from(Span::styled(
            s.to_string(),
            Style::new()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
    } else if s.contains('█') || s.contains('╔') {
        Line::from(Span::styled(
            s.to_string(),
            Style::new().fg(Color::Rgb(100, 180, 220)),
        ))
    } else {
        Line::from(Span::styled(s.to_string(), Style::new().fg(Color::DarkGray)))
    }
}

fn term_name() -> String {
    std::env::var("TERM").unwrap_or_else(|_| "unknown".into())
}

/// Header strip shown on every menu tab (logo + stat rows).
pub fn compact_lines(stats: &MenuStats) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for (i, line) in LOGO_COMPACT.iter().enumerate() {
        out.push(logo_line(line, i == 0));
    }
    out.push(Line::from(""));
    let theme = stats.theme.unwrap_or("default");
    let color_mode = if stats.truecolor {
        "truecolor"
    } else {
        "256-color"
    };
    out.push(label(
        "version",
        format!("{VERSION} · {} scenes", stats.scene_count),
    ));
    out.push(label(
        "runtime",
        format!(
            "{} · {} fps · {:.2}x",
            stats.scene, stats.fps, stats.speed
        ),
    ));
    out.push(label(
        "display",
        format!(
            "{} · {} · {}",
            stats.pixels,
            color_mode,
            term_name()
        ),
    ));
    let link = if stats.link_enabled {
        format!("on ({})", stats.link_group)
    } else {
        "off (solo)".into()
    };
    out.push(label("link", link));
    out.push(label("theme", theme));
    out.push(Line::from(""));
    out
}

/// Full product page for the About tab.
pub fn about_lines(stats: &MenuStats) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for (i, line) in LOGO_FULL.iter().enumerate() {
        out.push(logo_line(line, i == 0 || i == 2));
    }
    out.push(Line::from(""));
    out.push(Line::from(Span::styled(
        "Wallpaper Engine for the terminal",
        Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    out.push(Line::from(dim(
        "Animated truecolor scenes · link groups · video wall",
    )));
    out.push(Line::from(""));
    out.push(label("version", VERSION));
    out.push(label("scenes", format!("{}", stats.scene_count)));
    out.push(label("scene", stats.scene));
    out.push(label(
        "theme",
        stats.theme.unwrap_or("default"),
    ));
    out.push(label(
        "display",
        format!(
            "{} · {} · {}",
            stats.pixels,
            if stats.truecolor {
                "truecolor"
            } else {
                "256-color"
            },
            term_name()
        ),
    ));
    out.push(label(
        "runtime",
        format!("{} fps · {:.2}x speed", stats.fps, stats.speed),
    ));
    out.push(label(
        "link",
        if stats.link_enabled {
            format!("on · group {}", stats.link_group)
        } else {
            "off · solo art".into()
        },
    ));
    out.push(Line::from(""));
    out.push(Line::from(dim("MIT license · open source")));
    out.push(Line::from(dim("add scenes — see CONTRIBUTING.md")));
    out.push(Line::from(""));
    out.push(Line::from(dim("press 0 to reset all settings to defaults")));
    out
}
