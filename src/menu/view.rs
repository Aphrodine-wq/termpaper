//! Drawing the menu. The drawer takes the left ~46% of the screen (all of it
//! on narrow terminals) over a darkened copy of the scene, so the live
//! picture stays visible beside it — and faintly through it.

use super::browser::{self, Column, Shelf};
use super::settings::{self, Kind};
use super::{Menu, MenuCtx, Page};
use crate::scene::{self, Entry};
use crate::{brand, config};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap,
    },
    Frame,
};
use std::time::Instant;

/// One frame's colours: a soft palette on truecolor terminals, the nearest
/// named colours elsewhere.
struct Pal {
    text: Color,
    muted: Color,
    faint: Color,
    accent: Color,
    /// focused-row background
    hi_bg: Color,
    /// solid background for the help overlay
    panel: Color,
    fav: Color,
    live: Color,
    warn: Color,
}

fn pal(truecolor: bool) -> Pal {
    if truecolor {
        Pal {
            text: Color::Rgb(214, 218, 228),
            muted: Color::Rgb(140, 147, 164),
            faint: Color::Rgb(88, 94, 110),
            accent: Color::Rgb(122, 196, 236),
            hi_bg: Color::Rgb(40, 50, 72),
            panel: Color::Rgb(12, 13, 18),
            fav: Color::Rgb(240, 196, 92),
            live: Color::Rgb(126, 212, 146),
            warn: Color::Rgb(236, 160, 110),
        }
    } else {
        Pal {
            text: Color::White,
            muted: Color::Gray,
            faint: Color::DarkGray,
            accent: Color::Cyan,
            hi_bg: Color::Indexed(237),
            panel: Color::Indexed(233),
            fav: Color::Yellow,
            live: Color::Green,
            warn: Color::LightRed,
        }
    }
}

fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

fn width(s: &str) -> usize {
    s.chars().count()
}

/// `s` cut to `w` columns, with an ellipsis when it had to be cut.
fn fit(s: &str, w: usize) -> String {
    if width(s) <= w {
        return s.to_string();
    }
    if w == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(w - 1).collect();
    out.push('…');
    out
}

/// `s` cut or padded to exactly `w` columns.
fn pad(s: &str, w: usize) -> String {
    let f = fit(s, w);
    let n = width(&f);
    format!("{f}{}", " ".repeat(w - n))
}

/// Selection plus an offset that keeps it roughly centred in `height` rows.
fn list_state(selected: usize, len: usize, height: u16) -> ListState {
    let h = height as usize;
    let offset = if len <= h {
        0
    } else {
        selected.saturating_sub(h / 2).min(len - h)
    };
    ListState::default()
        .with_selected(Some(selected))
        .with_offset(offset)
}

/// The drawer: the left ~46% of the screen, at least 44 columns, and the
/// full width below 70 columns.
pub fn drawer_rect(area: Rect) -> Rect {
    let w = if area.width < 70 {
        area.width
    } else {
        ((area.width as u32 * 46 / 100) as u16).max(44)
    };
    Rect {
        width: w.min(area.width),
        ..area
    }
}

/// Darken the scene under the drawer instead of blanking it: every cell
/// becomes a very dim version of its own colour, so the drawer reads as
/// tinted glass over the live picture.
fn frost(buf: &mut Buffer, rect: Rect, truecolor: bool) {
    fn rgb(c: Color) -> (u16, u16, u16) {
        match c {
            Color::Rgb(r, g, b) => (r as u16, g as u16, b as u16),
            _ => (0, 0, 0),
        }
    }
    let rect = rect.intersection(buf.area);
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            let bg = if truecolor {
                let (a, b) = (rgb(cell.fg), rgb(cell.bg));
                // mean of the cell's two colours at 18%, over a blue-black
                let mix = |u: u16, v: u16, base: u16| (base + (u + v) * 9 / 100) as u8;
                Color::Rgb(mix(a.0, b.0, 8), mix(a.1, b.1, 9), mix(a.2, b.2, 14))
            } else {
                Color::Indexed(234)
            };
            cell.reset();
            cell.set_bg(bg);
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx) {
    let p = pal(ctx.truecolor);
    let rect = drawer_rect(area);
    if rect.width < 12 || rect.height < 5 {
        return;
    }
    frost(f.buffer_mut(), rect, ctx.truecolor);

    let shown = scene::lookup(ctx.scene_name).map_or(ctx.scene_name, |e| e.title());
    let right = match m.previewing() {
        Some(_) => format!(" preview · {shown} "),
        None => format!(" {shown} "),
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(p.faint))
        .title(Line::from(Span::styled(" termpaper ", bold(p.accent))))
        .title(
            Line::from(Span::styled(
                fit(&right, (rect.width as usize).saturating_sub(16)),
                fg(p.muted),
            ))
            .right_aligned(),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    // a second hint line when the drawer is tall and one line is not enough
    let hints = key_hints(m, ctx);
    let keys_h = if inner.height >= 16 && !hints_fit(&hints, inner.width as usize, 1) {
        2
    } else {
        1
    };
    let [tabs, body, keys] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(keys_h),
    ])
    .areas(inner);
    draw_tabs(f, tabs, m, &p);
    // a blank row under the tabs when there is room to breathe
    let body = if body.height >= 12 {
        Rect {
            y: body.y + 1,
            height: body.height - 1,
            ..body
        }
    } else {
        body
    };
    match m.page {
        Page::Scenes => draw_browser(f, body, m, ctx, &p),
        _ if m.filters_open => draw_filters(f, body, m, ctx, &p),
        _ => draw_settings(f, body, m, ctx, &p),
    }
    draw_keys(f, keys, hints, &p);
    if m.help {
        draw_help(f, area, m, ctx, &p);
    }
}

fn draw_tabs(f: &mut Frame, area: Rect, m: &Menu, p: &Pal) {
    let divider = if area.width >= 41 { " · " } else { " " };
    let tabs = Tabs::new(Page::ALL.iter().map(|pg| pg.title()))
        .select(m.page.index())
        .style(fg(p.muted))
        .highlight_style(bold(p.accent).add_modifier(Modifier::UNDERLINED))
        .divider(Span::styled(divider, fg(p.faint)))
        .padding("", "");
    f.render_widget(tabs, area);
}

// ── Scenes ──────────────────────────────────────────────────────────────

fn draw_browser(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let b = &m.browser;
    // detail panel under the lists, shrinking with the terminal
    let detail_h = match area.height {
        h if h >= 18 => 6,
        h if h >= 11 => 4,
        h if h >= 7 => 2,
        _ => 0,
    };
    let search_h = u16::from(b.searching || b.filtering());
    let [search, lists, detail] = Layout::vertical([
        Constraint::Length(search_h),
        Constraint::Min(1),
        Constraint::Length(detail_h),
    ])
    .areas(area);
    if search_h > 0 {
        draw_search(f, search, m, ctx, p);
    }
    m.page_len.set(lists.height as usize);
    if b.filtering() {
        // hits span every category, so they get the full width
        draw_scene_list(f, lists, m, ctx, p, true);
    } else {
        let left_w = (lists.width * 2 / 5).clamp(12, 24);
        let [left, _, right] = Layout::horizontal([
            Constraint::Length(left_w),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(lists);
        draw_shelves(f, left, m, ctx, p);
        draw_scene_list(f, right, m, ctx, p, b.column == Column::Scenes);
    }
    draw_detail(f, detail, m, ctx, p);
}

fn draw_search(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let b = &m.browser;
    let mut spans = vec![Span::styled("/ ", bold(p.accent))];
    if b.query.is_empty() {
        spans.push(Span::styled("type to filter…", fg(p.faint)));
    } else {
        spans.push(Span::styled(b.query.clone(), fg(p.text)));
    }
    if b.searching {
        spans.push(Span::styled("▏", fg(p.accent)));
    }
    if b.filtering() {
        let n = b.list(ctx).len();
        let s = if n == 1 { "" } else { "es" };
        spans.push(Span::styled(format!("  {n} match{s}"), fg(p.faint)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_shelves(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let shelves = browser::shelves();
    let w = area.width as usize;
    let items: Vec<ListItem> = shelves
        .iter()
        .map(|&s| {
            let count = format!("{:>3}", browser::shelf_scenes(s, ctx).len());
            let label = if s.nested() {
                format!("  {}", s.label())
            } else {
                s.label().to_string()
            };
            let style = match s {
                Shelf::Favorites => fg(p.fav),
                Shelf::Group(_) => fg(p.muted),
                _ => fg(p.text),
            };
            ListItem::new(Line::from(vec![
                Span::styled(pad(&label, w.saturating_sub(count.len())), style),
                Span::styled(count, fg(p.faint)),
            ]))
        })
        .collect();
    let sel = m.browser.shelf.min(shelves.len() - 1);
    let hl = if m.browser.column == Column::Shelves {
        Style::new().bg(p.hi_bg).add_modifier(Modifier::BOLD)
    } else {
        bold(p.accent)
    };
    let mut state = list_state(sel, shelves.len(), area.height);
    f.render_stateful_widget(List::new(items).highlight_style(hl), area, &mut state);
}

fn draw_scene_list(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal, focused: bool) {
    let b = &m.browser;
    let list = b.list(ctx);
    if list.is_empty() {
        let hint = if b.filtering() {
            "no matches — Esc clears"
        } else {
            match b.current_shelf() {
                Shelf::Favorites => "press f on a scene to star it",
                Shelf::Recent => "scenes you switch to land here",
                _ => "nothing here yet",
            }
        };
        f.render_widget(
            Paragraph::new(hint)
                .style(fg(p.faint))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    let w = area.width as usize;
    let items: Vec<ListItem> = list
        .iter()
        .map(|e| {
            let name = e.name();
            let live = name == ctx.scene_name;
            let fav = ctx.favorites.iter().any(|f| f == name);
            let theme = ctx
                .scene_themes
                .get(name)
                .map(String::as_str)
                .or_else(|| e.themes().first().copied())
                .unwrap_or("");
            let theme_w = if w >= 24 {
                width(theme).min((w - 4) / 3)
            } else {
                0
            };
            let title_w = w.saturating_sub(3 + if theme_w > 0 { theme_w + 1 } else { 0 });
            let mut spans = vec![
                Span::styled(if live { "●" } else { " " }, fg(p.live)),
                Span::styled(if fav { "★" } else { " " }, fg(p.fav)),
                Span::raw(" "),
                Span::styled(pad(e.title(), title_w), fg(p.text)),
            ];
            if theme_w > 0 {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(fit(theme, theme_w), fg(p.faint)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let sel = b.row_in(list.len());
    let hl = if focused {
        Style::new().bg(p.hi_bg).add_modifier(Modifier::BOLD)
    } else {
        Style::new().add_modifier(Modifier::BOLD)
    };
    let mut state = list_state(sel, list.len(), area.height);
    f.render_stateful_widget(List::new(items).highlight_style(hl), area, &mut state);
}

fn draw_detail(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    if area.height == 0 {
        return;
    }
    let area = if area.height >= 4 {
        let rule = Block::new().borders(Borders::TOP).border_style(fg(p.faint));
        let inner = rule.inner(area);
        f.render_widget(rule, area);
        inner
    } else {
        area
    };
    let Some(e) = m.browser.highlighted(ctx) else {
        return;
    };
    let mut lines = Vec::new();
    let mut head = vec![Span::styled(e.title().to_string(), bold(p.text))];
    if e.title() != e.name() {
        head.push(Span::styled(format!(" · {}", e.name()), fg(p.faint)));
    }
    if e.needs_gpu() {
        head.push(Span::raw("  "));
        head.push(Span::styled(
            " GPU ",
            Style::new().fg(Color::Black).bg(p.accent),
        ));
    }
    lines.push(Line::from(head));
    if let Entry::Shader(spec) = e {
        match ctx.gpu {
            Some(false) => lines.push(Line::from(Span::styled(
                format!("needs GPU → shows {} here", spec.fallback),
                fg(p.warn),
            ))),
            None => lines.push(Line::from(Span::styled(
                format!("needs a GPU; otherwise shows {}", spec.fallback),
                fg(p.faint),
            ))),
            Some(true) => {}
        }
    }
    lines.push(Line::from(Span::styled(e.desc().to_string(), fg(p.muted))));
    let cur = if e.name() == ctx.scene_name {
        ctx.theme.clone()
    } else {
        ctx.scene_themes.get(e.name()).cloned()
    }
    .or_else(|| e.themes().first().map(|t| t.to_string()));
    let mut themes = vec![Span::styled("themes ", fg(p.faint))];
    for (i, t) in e.themes().iter().enumerate() {
        if i > 0 {
            themes.push(Span::styled(" · ", fg(p.faint)));
        }
        let style = if Some(*t) == cur.as_deref() {
            bold(p.accent)
        } else {
            fg(p.muted)
        };
        themes.push(Span::styled(t.to_string(), style));
    }
    lines.push(Line::from(themes));
    if !e.tags().is_empty() {
        lines.push(Line::from(Span::styled(
            format!("tags {}", e.tags().join(", ")),
            fg(p.faint),
        )));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

// ── Settings pages ──────────────────────────────────────────────────────

/// The focused row's help (or a pending notice) under the rows.
fn draw_help_line(f: &mut Frame, area: Rect, m: &Menu, text: &str, p: &Pal) {
    let line = match m.flash_text(Instant::now()) {
        Some(msg) => Line::from(Span::styled(msg.to_string(), bold(p.accent))),
        None => Line::from(Span::styled(text.to_string(), fg(p.muted))),
    };
    let rule = Block::new().borders(Borders::TOP).border_style(fg(p.faint));
    let area = if area.height >= 2 {
        let inner = rule.inner(area);
        f.render_widget(rule, area);
        inner
    } else {
        area
    };
    f.render_widget(Paragraph::new(line).wrap(Wrap { trim: true }), area);
}

fn help_height(body: u16) -> u16 {
    match body {
        h if h >= 10 => 3,
        h if h >= 5 => 2,
        _ => 1,
    }
}

fn draw_settings(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let rows = settings::page(m.page);
    if rows.is_empty() {
        return;
    }
    let sel = m.row();
    // rows, then their help right beneath (rule + two lines), then extras
    let rows_h = (rows.len() as u16)
        .min(area.height.saturating_sub(1))
        .max(1);
    let help_h = (area.height - rows_h).min(3);
    let list_area = Rect {
        height: rows_h,
        ..area
    };
    let help_area = Rect {
        y: area.y + rows_h,
        height: help_h,
        ..area
    };
    let rest = Rect {
        y: help_area.bottom(),
        height: area.bottom() - help_area.bottom(),
        ..area
    };
    let w = list_area.width as usize;
    let lw = (w / 2).clamp(8, 17);
    let vw = w.saturating_sub(lw + 4);
    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let focused = i == sel;
            let on = settings::enabled(s.id, ctx);
            let v = fit(&settings::value(s.id, ctx), vw);
            let (label_style, value_style) = match (on, focused) {
                (false, _) => (fg(p.faint), fg(p.faint)),
                (true, true) => (bold(p.text), fg(p.accent)),
                (true, false) => (fg(p.text), fg(p.muted)),
            };
            let mut spans = vec![Span::styled(pad(s.label, lw), label_style)];
            match s.kind {
                Kind::Choice | Kind::Toggle if focused && on => {
                    spans.push(Span::styled("◂ ", fg(p.accent)));
                    spans.push(Span::styled(v, value_style));
                    spans.push(Span::styled(" ▸", fg(p.accent)));
                }
                Kind::Open => {
                    spans.push(Span::raw("  "));
                    if !v.is_empty() {
                        spans.push(Span::styled(v, value_style));
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(
                        "›",
                        fg(if focused { p.accent } else { p.faint }),
                    ));
                }
                _ => {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(v, value_style));
                }
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut state = list_state(sel, rows.len(), list_area.height);
    f.render_stateful_widget(
        List::new(items).highlight_style(Style::new().bg(p.hi_bg)),
        list_area,
        &mut state,
    );
    draw_help_line(f, help_area, m, rows[sel].help, p);
    // the Wall page lists its peers underneath
    if m.page == Page::Wall && rest.height >= 3 {
        let peers = Rect {
            y: rest.y + 1,
            height: rest.height - 1,
            ..rest
        };
        let mut lines = vec![Line::from(Span::styled("Instances", bold(p.muted)))];
        if ctx.instances.is_empty() {
            lines.push(Line::from(Span::styled("no other live panes", fg(p.faint))));
        }
        for i in &ctx.instances {
            lines.push(Line::from(Span::styled(fit(i, w), fg(p.faint))));
        }
        f.render_widget(Paragraph::new(lines), peers);
    }
}

fn draw_filters(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let [head, list_area, help_area] = Layout::vertical([
        Constraint::Length(2.min(area.height)),
        Constraint::Min(1),
        Constraint::Length(help_height(area.height)),
    ])
    .areas(area);
    let w = head.width as usize;
    let stack = if ctx.filters.is_empty() {
        "none".to_string()
    } else {
        ctx.filters.join(" → ")
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("‹ Filters", bold(p.accent)),
                Span::styled("  applied in stack order", fg(p.faint)),
            ]),
            Line::from(vec![
                Span::styled("stack ", fg(p.faint)),
                Span::styled(fit(&stack, w.saturating_sub(6)), fg(p.text)),
            ]),
        ]),
        head,
    );

    // one list with non-selectable group headings; map rows to lines
    let lw = 12usize.min(w);
    let on = |name: &str| ctx.filters.iter().position(|f| f == name);
    let preset = settings::preset_of(&ctx.filters).map_or("custom", |i| settings::PRESETS[i].0);
    let sel = m.filter_row().min(settings::filter_rows() - 1);
    let mut items = vec![ListItem::new(Line::from(vec![
        Span::styled(pad("Preset", lw), bold(p.text)),
        Span::styled(if sel == 0 { "◂ " } else { "  " }, fg(p.accent)),
        Span::styled(preset, fg(p.accent)),
        Span::styled(if sel == 0 { " ▸" } else { "" }, fg(p.accent)),
    ]))];
    let mut line_of_row = vec![0usize];
    for (group, names) in settings::FILTER_GROUPS {
        items.push(ListItem::new(Line::from(Span::styled(
            *group,
            bold(p.faint),
        ))));
        for name in names.iter() {
            line_of_row.push(items.len());
            let (mark, style) = match on(name) {
                Some(_) => ("[x] ", fg(p.live)),
                None => ("[ ] ", fg(p.muted)),
            };
            items.push(ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(mark, style),
                Span::styled(name.to_string(), style),
            ])));
        }
    }
    let total = items.len();
    let mut state = list_state(line_of_row[sel], total, list_area.height);
    f.render_stateful_widget(
        List::new(items).highlight_style(Style::new().bg(p.hi_bg).add_modifier(Modifier::BOLD)),
        list_area,
        &mut state,
    );

    let help = match settings::filter_at(sel) {
        None => "Presets replace the stack: Clean, Film, CRT, Dream.".to_string(),
        Some(name) => match on(name) {
            Some(i) => format!(
                "{} On, #{} in the stack.",
                settings::filter_help(name),
                i + 1
            ),
            None => settings::filter_help(name).to_string(),
        },
    };
    draw_help_line(f, help_area, m, &help, p);
}

// ── Footer and help ─────────────────────────────────────────────────────

/// The keys that do something right now, as (key, action) — the footer
/// never advertises a key that would do nothing.
pub fn key_hints(m: &Menu, ctx: &MenuCtx) -> Vec<(&'static str, &'static str)> {
    if m.help {
        return vec![("↑↓", "scroll"), ("Esc", "close")];
    }
    let close = if m.previewing().is_some() {
        "undo preview"
    } else {
        "close"
    };
    match m.page {
        Page::Scenes => {
            let hit = m.browser.highlighted(ctx);
            let mut v = vec![("↑↓", "move")];
            if m.browser.searching {
                v.insert(0, ("type", "filter"));
                if hit.is_some() {
                    v.push(("Enter", "switch"));
                }
                v.push(("Esc", "clear"));
                return v;
            }
            if m.browser.column == Column::Shelves {
                v.extend([("→", "scenes"), ("/", "search")]);
            } else {
                if let Some(e) = hit {
                    v.extend([("Enter", "switch"), ("f", "star")]);
                    if e.themes().len() > 1 {
                        v.push(("t", "theme"));
                    }
                }
                v.extend([("/", "search"), ("←", "categories")]);
            }
            v.extend([("Tab", "page"), ("?", "help"), ("Esc", close)]);
            v
        }
        _ if m.filters_open => {
            let what = if m.filter_row() == 0 {
                "preset"
            } else {
                "toggle"
            };
            vec![
                ("↑↓", "move"),
                ("←→", what),
                ("Esc", "back"),
                ("Tab", "page"),
            ]
        }
        _ => {
            let set = &settings::page(m.page)[m.row()];
            let mut v = vec![("↑↓", "move")];
            match set.kind {
                Kind::Choice if settings::enabled(set.id, ctx) => v.push(("←→", "change")),
                Kind::Toggle => v.push(("←→", "toggle")),
                Kind::Open => v.push(("Enter", "open")),
                _ => {}
            }
            v.extend([("Tab", "page"), ("?", "help"), ("Esc", "close")]);
            v
        }
    }
}

/// Which hints go first when space runs out: plain movement is guessable,
/// how to act, get help and get out is not.
fn hint_rank(key: &str) -> u8 {
    match key {
        "Esc" | "?" | "Enter" | "type" => 0,
        "↑↓" | "Tab" | "←" | "→" => 2,
        _ => 1,
    }
}

/// Lay hints out left to right over `lines` rows of `w` columns; None if
/// they do not all fit.
fn flow_hints(
    hints: &[(&'static str, &'static str)],
    w: usize,
    lines: usize,
) -> Option<Vec<Vec<usize>>> {
    let mut out: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = 0;
    for (i, (k, a)) in hints.iter().enumerate() {
        let need = width(k) + 1 + width(a);
        let sep = if used == 0 { 0 } else { 2 };
        if used + sep + need > w {
            if out.len() == lines || need > w {
                return None;
            }
            out.push(Vec::new());
            used = 0;
        }
        used += if used == 0 { need } else { 2 + need };
        out.last_mut().unwrap().push(i);
    }
    Some(out)
}

fn hints_fit(hints: &[(&'static str, &'static str)], w: usize, lines: usize) -> bool {
    flow_hints(hints, w, lines).is_some()
}

fn draw_keys(f: &mut Frame, area: Rect, mut hints: Vec<(&'static str, &'static str)>, p: &Pal) {
    let (w, lines) = (area.width as usize, area.height.max(1) as usize);
    // drop the least important (latest first) until the rest fit
    let layout = loop {
        if let Some(l) = flow_hints(&hints, w, lines) {
            break l;
        }
        // max_by_key keeps the last of equals: the rightmost worst hint
        let Some(worst) = (0..hints.len()).max_by_key(|&i| hint_rank(hints[i].0)) else {
            break Vec::new();
        };
        hints.remove(worst);
    };
    let text: Vec<Line> = layout
        .iter()
        .map(|row| {
            let mut spans = Vec::new();
            for (n, &i) in row.iter().enumerate() {
                if n > 0 {
                    spans.push(Span::raw("  "));
                }
                let (k, a) = hints[i];
                spans.push(Span::styled(k, bold(p.text)));
                spans.push(Span::styled(format!(" {a}"), fg(p.faint)));
            }
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(text), area);
}

/// Keys inside the menu, for the help overlay.
const MENU_KEYS: &[(&str, &str)] = &[
    ("Tab  Shift-Tab", "next / previous page"),
    ("↑ ↓  j k", "move"),
    ("← →  h l", "change a value · switch column"),
    ("PgUp PgDn Home End", "jump"),
    ("Enter", "switch scene · toggle · open"),
    ("/", "search scenes"),
    ("f", "star the highlighted scene"),
    ("t", "next theme for it"),
    ("?", "this help"),
    ("Esc", "back · clear search · close"),
];

fn pretty_key(k: &str) -> String {
    match k.to_lowercase().as_str() {
        "right" => "→".into(),
        "left" => "←".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "space" => "Space".into(),
        "esc" | "escape" => "Esc".into(),
        "tab" => "Tab".into(),
        "enter" | "return" => "Enter".into(),
        _ => k.to_string(),
    }
}

fn action_help(action: &str) -> &'static str {
    match action {
        "quit" => "quit",
        "menu" => "open this menu",
        "next" => "next scene",
        "prev" => "previous scene",
        "filter_next" => "try filters one by one",
        "detail_next" => "cycle quality",
        "pause" => "pause",
        "color" => "color grade",
        "reset" => "reset every setting",
        "fps_up" => "raise the FPS cap",
        "fps_down" => "lower the FPS cap",
        "speed_up" => "faster",
        "speed_down" => "slower",
        _ => "",
    }
}

fn help_lines(ctx: &MenuCtx, p: &Pal, w: u16) -> Vec<Line<'static>> {
    let head = |s: &str| Line::from(Span::styled(s.to_string(), bold(p.accent)));
    // key column narrows with the overlay
    let kw = (w as usize * 2 / 5).clamp(8, 19);
    let row = |k: &str, a: &str| {
        Line::from(vec![
            Span::styled(format!("  {} ", pad(k, kw)), fg(p.text)),
            Span::styled(a.to_string(), fg(p.muted)),
        ])
    };
    let mut v = vec![head("In the menu")];
    v.extend(MENU_KEYS.iter().map(|(k, a)| row(k, a)));
    v.push(Line::from(""));
    v.push(head("On the wallpaper"));
    for (action, key) in &ctx.key_display {
        v.push(row(&pretty_key(key), action_help(action)));
    }
    v.push(Line::from(""));
    v.push(head("About"));
    let total = scene::entries().count();
    let studio = scene::entries().filter(|e| e.needs_gpu()).count();
    v.push(row(
        &format!("termpaper {}", brand::VERSION),
        "Wallpaper Engine for the terminal",
    ));
    v.push(row(
        "scenes",
        &format!("{total}: {studio} Studio (GPU), {} Classic", total - studio),
    ));
    v.push(row("renderer", &ctx.renderer_status));
    v.push(row("wall", ctx.wall_status.trim_start_matches("wall: ")));
    let path = config::config_path().map_or("—".into(), |p| p.display().to_string());
    v.push(row("config", &path));
    v.push(row("source", "github.com/Aphrodine-wq/termpaper"));
    v
}

fn draw_help(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let w = area.width.saturating_sub(4).clamp(area.width.min(24), 72);
    let h = area.height.saturating_sub(2).clamp(area.height.min(6), 40);
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(p.faint))
        .style(Style::new().bg(p.panel))
        .title(Line::from(Span::styled(" help ", bold(p.accent))))
        .title_bottom(Line::from(Span::styled(" Esc close ", fg(p.faint))).right_aligned());
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let lines = help_lines(ctx, p, inner.width);
    let max = (lines.len() as u16).saturating_sub(inner.height);
    m.help_max.set(max);
    f.render_widget(
        Paragraph::new(lines).scroll((m.help_scroll.min(max), 0)),
        inner,
    );
}
