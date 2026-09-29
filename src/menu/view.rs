//! Drawing the menu. The drawer takes the left ~46% of the screen (all of it
//! on narrow terminals) over a darkened copy of the scene, so the live
//! picture stays visible beside it — and faintly through it.
//!
//! While it draws, the view records what is where ([`super::Hit`]) so the
//! mouse can reach every control the keys do.

use super::browser::{self, Column, Shelf};
use super::settings::{self, Kind};
use super::{Hit, HitBox, Menu, MenuCtx, Page};

use crate::scene::{self, Entry};
use crate::{brand, config};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
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
    /// text drawn on the accent (selected tab)
    on_accent: Color,
    /// focused-row background
    hi_bg: Color,
    /// solid background for the help overlay
    panel: Color,
    /// a slider's empty track
    track: Color,
    fav: Color,
    live: Color,
    warn: Color,
}

fn pal(truecolor: bool) -> Pal {
    if truecolor {
        Pal {
            text: Color::Rgb(220, 224, 234),
            muted: Color::Rgb(146, 153, 170),
            faint: Color::Rgb(88, 94, 110),
            accent: Color::Rgb(122, 196, 236),
            on_accent: Color::Rgb(10, 14, 22),
            hi_bg: Color::Rgb(36, 46, 68),
            panel: Color::Rgb(12, 13, 18),
            track: Color::Rgb(56, 62, 78),
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
            on_accent: Color::Black,
            hi_bg: Color::Indexed(237),
            panel: Color::Indexed(233),
            track: Color::Indexed(239),
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

/// `s` cut or padded on the left to exactly `w` columns (right-aligned).
fn pad_left(s: &str, w: usize) -> String {
    let f = fit(s, w);
    let n = width(&f);
    format!("{}{f}", " ".repeat(w - n))
}

/// First visible line of a list of `len` lines in `height` rows, keeping
/// `selected` roughly centred.
fn scroll_offset(selected: usize, len: usize, height: usize) -> usize {
    if len <= height {
        0
    } else {
        selected.saturating_sub(height / 2).min(len - height)
    }
}

fn hit(m: &Menu, rect: Rect, what: Hit) {
    if rect.width > 0 && rect.height > 0 {
        m.hits.borrow_mut().push(HitBox { rect, hit: what });
    }
}

/// Draw one line in a one-row rect.
fn line_at(f: &mut Frame, x: u16, y: u16, w: u16, line: Line<'static>) {
    f.render_widget(Paragraph::new(line), Rect::new(x, y, w, 1));
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
    m.hits.borrow_mut().clear();
    let p = pal(ctx.truecolor);
    let rect = drawer_rect(area);
    if rect.width < 12 || rect.height < 5 {
        return;
    }
    // the live scene beside the drawer: a click there closes the menu; the
    // drawer's own empty space does nothing
    hit(m, area, Hit::Outside);
    hit(m, rect, Hit::Drawer);
    frost(f.buffer_mut(), rect, ctx.truecolor);

    let shown = scene::lookup(ctx.scene_name).map_or(ctx.scene_name, |e| e.title());
    let variant = ctx.theme.clone().or_else(|| scene::themes(ctx.scene_name).first().map(|t| t.to_string()));
    let right = match (m.previewing(), variant) {
        (Some(_), _) => format!(" preview · {shown} "),
        (None, Some(v)) => format!(" {shown} · {v} "),
        (None, None) => format!(" {shown} "),
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
        Page::Themes => draw_themes(f, body, m, ctx, &p),
        _ if m.filters_open => draw_filters(f, body, m, ctx, &p),
        _ => draw_settings(f, body, m, ctx, &p),
    }
    match &m.prompt {
        // a question takes the footer: what is being asked, what is typed
        Some(pr) => {
            let q = format!("{}: ", pr.question());
            let room = (keys.width as usize).saturating_sub(width(&q) + 2);
            let shown: String = {
                let t: Vec<char> = pr.text.chars().collect();
                t[t.len().saturating_sub(room)..].iter().collect()
            };
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(q, bold(p.accent)),
                        Span::styled(shown, fg(p.text)),
                        Span::styled("▏", fg(p.accent)),
                    ]),
                    Line::from(vec![
                        Span::styled("Enter", bold(p.text)),
                        Span::styled(" ok  ", fg(p.faint)),
                        Span::styled("Esc", bold(p.text)),
                        Span::styled(" cancel", fg(p.faint)),
                    ]),
                ]),
                Rect { y: keys.y.saturating_sub(1), height: keys.height + 1, ..keys },
            );
        }
        None => {
            // pages without a help line give a notice the footer for a moment
            let flash = matches!(m.page, Page::Scenes | Page::Themes)
                .then(|| m.flash_text(Instant::now()))
                .flatten();
            match flash {
                Some(msg) => f.render_widget(
                    Paragraph::new(Line::from(Span::styled(msg.to_string(), bold(p.accent)))).wrap(Wrap { trim: true }),
                    keys,
                ),
                None => draw_keys(f, keys, hints, &p),
            }
        }
    }
    if m.help {
        draw_help(f, area, m, ctx, &p);
    }
}

/// Tabs as pills: the current page on the accent, the rest quiet. Narrow
/// drawers get short names.
fn draw_tabs(f: &mut Frame, area: Rect, m: &Menu, p: &Pal) {
    let full: usize = Page::ALL.iter().map(|pg| width(pg.title()) + 2).sum::<usize>() + Page::ALL.len() - 1;
    let short = full > area.width as usize;
    let mut x = area.x;
    for (i, pg) in Page::ALL.iter().enumerate() {
        let name = if short { pg.short_title() } else { pg.title() };
        let label = format!(" {name} ");
        let w = width(&label) as u16;
        if x + w > area.x + area.width {
            break;
        }
        let style = if *pg == m.page {
            Style::new().fg(p.on_accent).bg(p.accent).add_modifier(Modifier::BOLD)
        } else {
            fg(p.muted)
        };
        line_at(f, x, area.y, w, Line::from(Span::styled(label, style)));
        hit(m, Rect::new(x, area.y, w, 1), Hit::Tab(*pg));
        x += w;
        if i + 1 < Page::ALL.len() {
            x += 1;
        }
    }
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
    let sel = m.browser.shelf.min(shelves.len() - 1);
    let focused = m.browser.column == Column::Shelves;
    let off = scroll_offset(sel, shelves.len(), area.height as usize);
    for (row, (i, &s)) in shelves.iter().enumerate().skip(off).take(area.height as usize).enumerate() {
        let y = area.y + row as u16;
        let count = format!("{:>3}", browser::shelf_scenes(s, ctx).len());
        let label = if s.nested() {
            format!("  {}", s.label())
        } else {
            s.label().to_string()
        };
        let mut style = match s {
            Shelf::Favorites => fg(p.fav),
            Shelf::Group(_) => fg(p.muted),
            _ => fg(p.text),
        };
        if i == sel {
            style = if focused {
                style.bg(p.hi_bg).add_modifier(Modifier::BOLD)
            } else {
                bold(p.accent)
            };
        }
        let count_style = if i == sel && focused { fg(p.faint).bg(p.hi_bg) } else { fg(p.faint) };
        line_at(
            f,
            area.x,
            y,
            area.width,
            Line::from(vec![
                Span::styled(pad(&label, w.saturating_sub(count.len())), style),
                Span::styled(count, count_style),
            ]),
        );
        hit(m, Rect::new(area.x, y, area.width, 1), Hit::Shelf(i));
    }
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
    let sel = b.row_in(list.len());
    let off = scroll_offset(sel, list.len(), area.height as usize);
    for (row, (i, e)) in list.iter().enumerate().skip(off).take(area.height as usize).enumerate() {
        let y = area.y + row as u16;
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
        let row_style = if i == sel && focused {
            Style::new().bg(p.hi_bg).add_modifier(Modifier::BOLD)
        } else if i == sel {
            Style::new().add_modifier(Modifier::BOLD)
        } else {
            Style::new()
        };
        let mut spans = vec![
            Span::styled(if live { "●" } else { " " }, fg(p.live).patch(row_style)),
            Span::styled(if fav { "★" } else { " " }, fg(p.fav).patch(row_style)),
            Span::styled(" ", row_style),
            Span::styled(pad(e.title(), title_w), fg(p.text).patch(row_style)),
        ];
        if theme_w > 0 {
            spans.push(Span::styled(" ", row_style));
            spans.push(Span::styled(pad(&fit(theme, theme_w), theme_w), fg(p.faint).patch(row_style)));
        }
        line_at(f, area.x, y, area.width, Line::from(spans));
        hit(m, Rect::new(area.x, y, area.width, 1), Hit::Scene(i));
    }
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
    head.push(Span::raw("  "));
    if e.needs_gpu() {
        head.push(Span::styled(" Studio ", Style::new().fg(p.on_accent).bg(p.accent)));
    } else {
        head.push(Span::styled(" Classic ", Style::new().fg(p.on_accent).bg(p.muted)));
    }
    lines.push(Line::from(head));
    if let Entry::Shader(spec) = e {
        match ctx.gpu {
            Some(false) => lines.push(Line::from(Span::styled(
                format!("needs a GPU → shows {} here", spec.fallback),
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
    // variants as chips: the one in use on the accent
    let mut chips = vec![Span::styled("variants ", fg(p.faint))];
    for t in e.themes() {
        if Some(*t) == cur.as_deref() {
            chips.push(Span::styled(format!(" {t} "), Style::new().fg(p.on_accent).bg(p.accent)));
        } else {
            chips.push(Span::styled(format!(" {t} "), fg(p.muted)));
        }
    }
    lines.push(Line::from(chips));
    if !e.tags().is_empty() {
        lines.push(Line::from(Span::styled(
            format!("tags {}", e.tags().join(", ")),
            fg(p.faint),
        )));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

// ── Themes ──────────────────────────────────────────────────────────────

fn draw_themes(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let list = super::themes::visible(&ctx.themes, &m.theme_query);
    let detail_h = match area.height {
        h if h >= 18 => 6,
        h if h >= 11 => 4,
        _ => 0,
    };
    let search_h = u16::from(m.theme_searching || !m.theme_query.is_empty());
    let [search, rows, detail] = Layout::vertical([
        Constraint::Length(search_h),
        Constraint::Min(1),
        Constraint::Length(detail_h),
    ])
    .areas(area);
    if search_h > 0 {
        let mut spans = vec![Span::styled("/ ", bold(p.accent))];
        if m.theme_query.is_empty() {
            spans.push(Span::styled("type to filter…", fg(p.faint)));
        } else {
            spans.push(Span::styled(m.theme_query.clone(), fg(p.text)));
        }
        if m.theme_searching {
            spans.push(Span::styled("▏", fg(p.accent)));
        }
        spans.push(Span::styled(format!("  {} themes", list.len()), fg(p.faint)));
        f.render_widget(Paragraph::new(Line::from(spans)), search);
    }
    if list.is_empty() {
        f.render_widget(Paragraph::new("no themes match — Esc clears").style(fg(p.faint)), rows);
        return;
    }
    // lines: a heading wherever the shelf changes, then the themes
    enum L {
        Head(&'static str),
        Row(usize),
    }
    let mut lines = Vec::new();
    let mut shelf = "";
    for (i, t) in list.iter().enumerate() {
        if t.shelf != shelf {
            shelf = t.shelf;
            lines.push(L::Head(t.shelf));
        }
        lines.push(L::Row(i));
    }
    let sel = m.theme_row.min(list.len() - 1);
    let sel_line = lines.iter().position(|l| matches!(l, L::Row(r) if *r == sel)).unwrap_or(0);
    m.page_len.set(rows.height as usize);
    let off = scroll_offset(sel_line, lines.len(), rows.height as usize);
    let w = rows.width as usize;
    let last = off + rows.height as usize - 1;
    for (k, line) in lines.iter().enumerate().skip(off).take(rows.height as usize) {
        let y = rows.y + (k - off) as u16;
        let rect = Rect::new(rows.x, y, rows.width, 1);
        match line {
            // a heading with nothing under it on screen waits for the scroll
            L::Head(_) if k == last && k + 1 < lines.len() => {}
            L::Head(name) => {
                let head = name.to_uppercase();
                let rule_w = w.saturating_sub(width(&head) + 2);
                line_at(
                    f,
                    rect.x,
                    y,
                    rect.width,
                    Line::from(vec![
                        Span::styled(format!(" {head} "), bold(p.faint)),
                        Span::styled("─".repeat(rule_w), fg(p.track)),
                    ]),
                );
            }
            L::Row(i) => {
                let t = list[*i];
                let focused = *i == sel;
                let active = ctx.active_theme.as_deref() == Some(t.slug.as_str());
                let bg = focused.then_some(p.hi_bg);
                if let Some(b) = bg {
                    f.buffer_mut().set_style(rect, Style::new().bg(b));
                }
                let st = |s: Style| match bg {
                    Some(b) => s.bg(b),
                    None => s,
                };
                // swatches on the right: two cells a colour
                let sw_n = t.swatches.len().min(6);
                let sw_w = sw_n * 2;
                let name_w = w.saturating_sub(4 + sw_w + 2);
                let mut name = t.name.clone();
                if active && ctx.theme_modified {
                    name.push_str(" · edited");
                }
                let mut spans = vec![
                    Span::styled(if focused { "▌" } else { " " }, st(fg(p.accent))),
                    Span::styled(if active { "●" } else { " " }, st(fg(p.live))),
                    Span::styled(" ", st(Style::new())),
                    Span::styled(
                        pad(&name, name_w),
                        st(if focused { bold(p.text) } else if t.yours { fg(p.fav) } else { fg(p.text) }),
                    ),
                    Span::styled(" ", st(Style::new())),
                ];
                for c in t.swatches.iter().take(sw_n) {
                    let col = if ctx.truecolor {
                        Color::Rgb(c.0, c.1, c.2)
                    } else {
                        Color::Indexed(crate::canvas::rgb_to_256(c.0, c.1, c.2))
                    };
                    spans.push(Span::styled("██", st(fg(col))));
                }
                line_at(f, rect.x, y, rect.width, Line::from(spans));
                hit(m, rect, Hit::Theme(*i));
            }
        }
    }
    // the highlighted theme in full
    if detail_h > 0 {
        let rule = Block::new().borders(Borders::TOP).border_style(fg(p.faint));
        let inner = rule.inner(detail);
        f.render_widget(rule, detail);
        let t = list[sel];
        let mut head = vec![Span::styled(t.name.clone(), bold(p.text))];
        if !t.author.is_empty() {
            head.push(Span::styled(format!(" · by {}", t.author), fg(p.faint)));
        }
        if ctx.active_theme.as_deref() == Some(t.slug.as_str()) {
            head.push(Span::raw("  "));
            let tag = if ctx.theme_modified { " in use · edited " } else { " in use " };
            head.push(Span::styled(tag, Style::new().fg(p.on_accent).bg(p.live)));
        } else if m.look_previewing() {
            head.push(Span::raw("  "));
            head.push(Span::styled(" preview ", Style::new().fg(p.on_accent).bg(p.accent)));
        }
        if t.yours {
            head.push(Span::raw("  "));
            head.push(Span::styled(" yours ", Style::new().fg(p.on_accent).bg(p.fav)));
        }
        let mut lines = vec![Line::from(head), Line::from(Span::styled(t.description.clone(), fg(p.muted)))];
        if let Some((scene_name, variant)) = &t.scene {
            let title = scene::lookup(scene_name).map_or(scene_name.as_str(), |e| e.title());
            let v = variant.as_deref().map(|v| format!(" · {v}")).unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled("made for ", fg(p.faint)),
                Span::styled(format!("{title}{v}"), fg(p.text)),
                Span::styled("  s switches to it", fg(p.faint)),
            ]));
        }
        if !t.tags.is_empty() {
            lines.push(Line::from(Span::styled(format!("tags {}", t.tags.join(", ")), fg(p.faint))));
        }
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    }
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

/// A slider bar `w` wide: the fill runs from the neutral point to the
/// value, so a bipolar control grows either way from its middle.
fn slider_spans(frac: f32, neutral: f32, w: usize, focused: bool, on: bool, bg: Option<Color>, p: &Pal) -> Vec<Span<'static>> {
    if w == 0 {
        return Vec::new();
    }
    let at = |v: f32| ((v.clamp(0.0, 1.0) * (w - 1) as f32).round() as usize).min(w - 1);
    let (k, n) = (at(frac), at(neutral));
    let (lo, hi) = (k.min(n), k.max(n));
    let fill = if !on {
        p.faint
    } else if focused {
        p.accent
    } else {
        p.muted
    };
    let knob = if focused && on { p.text } else { fill };
    let with_bg = |s: Style| match bg {
        Some(b) => s.bg(b),
        None => s,
    };
    let mut spans = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::new();
    for i in 0..w {
        let (ch, style) = if i == k {
            ('●', with_bg(fg(knob)))
        } else if i >= lo && i <= hi {
            ('━', with_bg(fg(fill)))
        } else {
            ('─', with_bg(fg(p.track)))
        };
        if style != run_style && !run.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        run_style = style;
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    spans
}

/// One page line: a section heading or a settings row.
enum PageLine {
    Blank,
    Section(&'static str),
    Row(usize),
}

fn draw_settings(f: &mut Frame, area: Rect, m: &Menu, ctx: &MenuCtx, p: &Pal) {
    let rows = settings::page(m.page);
    if rows.is_empty() {
        return;
    }
    let sel = m.row();
    // headings interleaved with rows
    let sections = settings::sections(m.page);
    let mut lines = Vec::new();
    for i in 0..rows.len() {
        if let Some((_, name)) = sections.iter().find(|(at, _)| *at == i) {
            if !lines.is_empty() {
                lines.push(PageLine::Blank);
            }
            lines.push(PageLine::Section(name));
        }
        lines.push(PageLine::Row(i));
    }
    // rows, then their help right beneath (rule + two lines), then extras
    let help_h = help_height(area.height).min(area.height.saturating_sub(1));
    let list_h = (lines.len() as u16).min(area.height.saturating_sub(help_h)).max(1);
    let list_area = Rect { height: list_h, ..area };
    let help_area = Rect {
        y: area.y + list_h,
        height: help_h,
        ..area
    };
    let rest = Rect {
        y: help_area.bottom(),
        height: area.bottom().saturating_sub(help_area.bottom()),
        ..area
    };
    m.page_len.set(list_h as usize);
    let w = list_area.width as usize;
    let lw = (w * 2 / 5).clamp(8, 18);
    // value column: after the label and a two-column gutter
    let vx = list_area.x + (lw + 3).min(w) as u16;
    let vw = (list_area.x + list_area.width).saturating_sub(vx) as usize;
    let sel_line = lines.iter().position(|l| matches!(l, PageLine::Row(r) if *r == sel)).unwrap_or(0);
    let off = scroll_offset(sel_line, lines.len(), list_h as usize);
    for (row_on_screen, line) in lines.iter().enumerate().skip(off).take(list_h as usize).map(|(i, l)| (i - off, l)) {
        let y = list_area.y + row_on_screen as u16;
        match line {
            PageLine::Blank => {}
            PageLine::Section(name) => {
                let head = name.to_uppercase();
                let rule_w = w.saturating_sub(width(&head) + 2);
                line_at(
                    f,
                    list_area.x,
                    y,
                    list_area.width,
                    Line::from(vec![
                        Span::styled(format!(" {head} "), bold(p.faint)),
                        Span::styled("─".repeat(rule_w), fg(p.track)),
                    ]),
                );
            }
            PageLine::Row(i) => {
                let s = &rows[*i];
                let focused = *i == sel;
                let on = settings::enabled(s.id, ctx);
                let bg = focused.then_some(p.hi_bg);
                let with_bg = |st: Style| match bg {
                    Some(b) => st.bg(b),
                    None => st,
                };
                let row_rect = Rect::new(list_area.x, y, list_area.width, 1);
                if let Some(b) = bg {
                    f.buffer_mut().set_style(row_rect, Style::new().bg(b));
                }
                let label_style = match (on, focused) {
                    (false, _) => fg(p.faint),
                    (true, true) => bold(p.text),
                    (true, false) if s.kind == Kind::Action => fg(p.accent),
                    (true, false) => fg(p.text),
                };
                let bar = if focused { "▌" } else { " " };
                let mut spans = vec![
                    Span::styled(bar, with_bg(fg(p.accent))),
                    Span::styled(pad(s.label, lw), with_bg(label_style)),
                    Span::styled("  ", with_bg(Style::new())),
                ];
                let v = settings::value(s.id, ctx);
                let value_style = match (on, focused) {
                    (false, _) => fg(p.faint),
                    (true, true) => fg(p.accent),
                    (true, false) => fg(p.muted),
                };
                match s.kind {
                    Kind::Slider => {
                        let tw = 8.min(vw);
                        let bw = vw.saturating_sub(tw + 1);
                        if bw >= 4 {
                            let (frac, neutral) = settings::fraction(s.id, ctx).unwrap_or((0.0, 0.0));
                            spans.extend(slider_spans(frac, neutral, bw, focused, on, bg, p));
                            spans.push(Span::styled(" ", with_bg(Style::new())));
                            hit(m, Rect::new(vx, y, bw as u16, 1), Hit::Slider { row: *i, x0: vx, width: bw as u16 });
                        }
                        spans.push(Span::styled(pad_left(&v, tw), with_bg(value_style)));
                    }
                    Kind::Toggle => {
                        let is_on = v.starts_with("on") || v == "auto";
                        let (mark, style) = if is_on { ("● ", fg(p.live)) } else { ("○ ", fg(p.faint)) };
                        spans.push(Span::styled(mark, with_bg(style)));
                        spans.push(Span::styled(fit(&v, vw.saturating_sub(2)), with_bg(value_style)));
                        hit(m, Rect::new(vx, y, vw as u16, 1), Hit::Value(*i));
                    }
                    Kind::Choice if focused && on => {
                        spans.push(Span::styled("◂ ", with_bg(fg(p.accent))));
                        spans.push(Span::styled(fit(&v, vw.saturating_sub(4)), with_bg(value_style)));
                        spans.push(Span::styled(" ▸", with_bg(fg(p.accent))));
                        hit(m, Rect::new(vx, y, vw as u16, 1), Hit::Value(*i));
                    }
                    Kind::Open | Kind::Action => {
                        if !v.is_empty() {
                            spans.push(Span::styled(fit(&v, vw.saturating_sub(2)), with_bg(value_style)));
                            spans.push(Span::styled(" ", with_bg(Style::new())));
                        }
                        let mark = if s.kind == Kind::Open { "›" } else { "↵" };
                        spans.push(Span::styled(mark, with_bg(fg(if focused { p.accent } else { p.faint }))));
                        hit(m, Rect::new(vx, y, vw as u16, 1), Hit::Value(*i));
                    }
                    _ => {
                        spans.push(Span::styled(fit(&v, vw), with_bg(value_style)));
                        if s.kind == Kind::Choice {
                            hit(m, Rect::new(vx, y, vw as u16, 1), Hit::Value(*i));
                        }
                    }
                }
                line_at(f, list_area.x, y, list_area.width, Line::from(spans));
                // the label focuses the row; the value acts
                hit(m, Rect::new(list_area.x, y, (lw + 3).min(w) as u16, 1), Hit::Row(*i));
            }
        }
    }
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
    let effects = &ctx.look.effects;
    let strength = |n: &str| format!("{:.0}%", effects.amount(n) * 100.0);
    let stack = if effects.stack.is_empty() {
        "none".to_string()
    } else {
        effects
            .stack
            .iter()
            .map(|n| if crate::filter::has_amount(n) { format!("{n} {}", strength(n)) } else { n.clone() })
            .collect::<Vec<_>>()
            .join(" → ")
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("‹ Effects", bold(p.accent)),
                Span::styled("  applied top to bottom of the stack", fg(p.faint)),
            ]),
            Line::from(vec![
                Span::styled("stack ", fg(p.faint)),
                Span::styled(fit(&stack, w.saturating_sub(6)), fg(p.text)),
            ]),
        ]),
        head,
    );

    // one list with non-selectable group headings; rows map onto lines
    let lw = 13usize.min(w);
    let on = |name: &str| effects.stack.iter().position(|f| f == name);
    let preset = settings::preset_of(effects).map_or("custom", |i| settings::PRESETS[i].0);
    let sel = m.filter_row().min(settings::filter_rows() - 1);
    enum L {
        Preset,
        Group(&'static str),
        Effect(usize, &'static str),
    }
    let mut lines = vec![L::Preset];
    let mut row = 1;
    for (group, names) in settings::FILTER_GROUPS {
        lines.push(L::Group(group));
        for name in names.iter() {
            lines.push(L::Effect(row, name));
            row += 1;
        }
    }
    let sel_line = lines
        .iter()
        .position(|l| match l {
            L::Preset => sel == 0,
            L::Effect(r, _) => *r == sel,
            L::Group(_) => false,
        })
        .unwrap_or(0);
    let off = scroll_offset(sel_line, lines.len(), list_area.height as usize);
    for (k, line) in lines.iter().enumerate().skip(off).take(list_area.height as usize) {
        let y = list_area.y + (k - off) as u16;
        let rect = Rect::new(list_area.x, y, list_area.width, 1);
        match line {
            L::Preset => {
                let focused = sel == 0;
                let bg = focused.then_some(p.hi_bg);
                if let Some(b) = bg {
                    f.buffer_mut().set_style(rect, Style::new().bg(b));
                }
                let st = |s: Style| match bg {
                    Some(b) => s.bg(b),
                    None => s,
                };
                line_at(
                    f,
                    rect.x,
                    y,
                    rect.width,
                    Line::from(vec![
                        Span::styled(if focused { "▌" } else { " " }, st(fg(p.accent))),
                        Span::styled(pad("Preset", lw), st(bold(p.text))),
                        Span::styled(if focused { "◂ " } else { "  " }, st(fg(p.accent))),
                        Span::styled(preset, st(fg(p.accent))),
                        Span::styled(if focused { " ▸" } else { "" }, st(fg(p.accent))),
                    ]),
                );
                hit(m, rect, Hit::FilterRow(0));
            }
            L::Group(g) => {
                let head = g.to_uppercase();
                let rule_w = (rect.width as usize).saturating_sub(width(&head) + 2);
                line_at(
                    f,
                    rect.x,
                    y,
                    rect.width,
                    Line::from(vec![
                        Span::styled(format!(" {head} "), bold(p.faint)),
                        Span::styled("─".repeat(rule_w), fg(p.track)),
                    ]),
                );
            }
            L::Effect(r, name) => {
                let focused = *r == sel;
                let bg = focused.then_some(p.hi_bg);
                if let Some(b) = bg {
                    f.buffer_mut().set_style(rect, Style::new().bg(b));
                }
                let st = |s: Style| match bg {
                    Some(b) => s.bg(b),
                    None => s,
                };
                let is_on = on(name);
                let (mark, mark_style) = match is_on {
                    Some(_) => ("● ", fg(p.live)),
                    None => ("○ ", fg(p.faint)),
                };
                let name_style = if is_on.is_some() { fg(p.text) } else { fg(p.muted) };
                let mut spans = vec![
                    Span::styled(if focused { "▌ " } else { "  " }, st(fg(p.accent))),
                    Span::styled(mark, st(mark_style)),
                    Span::styled(pad(name, lw), st(name_style)),
                ];
                hit(m, rect, Hit::FilterRow(*r));
                hit(m, Rect::new(rect.x + 2, y, 2, 1), Hit::FilterBox(*r));
                let used = 2 + 2 + lw;
                let avail = (rect.width as usize).saturating_sub(used + 1);
                // a strength bar for effects that are on, and on the focused
                // row (where it is the thing ←→ and the mouse move)
                let show_bar = is_on.is_some() || focused;
                if crate::filter::has_amount(name) && avail >= 10 && show_bar {
                    let tw = 5;
                    let bw = avail.saturating_sub(tw + 4);
                    let a = if is_on.is_some() { effects.amount(name) } else { 0.0 };
                    let x0 = rect.x + (used + 1) as u16;
                    spans.push(Span::styled(" ", st(Style::new())));
                    spans.extend(slider_spans(a / 2.0, 0.0, bw, focused, is_on.is_some(), bg, p));
                    let label = if is_on.is_some() { strength(name) } else { String::new() };
                    spans.push(Span::styled(format!(" {}", pad_left(&label, tw)), st(fg(p.muted))));
                    hit(m, Rect::new(x0, y, bw as u16, 1), Hit::FilterSlider { row: *r, x0, width: bw as u16 });
                }
                if let Some(i) = is_on {
                    spans.push(Span::styled(format!(" #{}", i + 1), st(fg(p.faint))));
                }
                line_at(f, rect.x, y, rect.width, Line::from(spans));
            }
        }
    }

    let help = match settings::filter_at(sel) {
        None => "Presets replace the stack and its strengths.".to_string(),
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
    if m.prompt.is_some() {
        return vec![("Enter", "ok"), ("Esc", "cancel")];
    }
    match m.page {
        Page::Themes => {
            if m.theme_searching {
                return vec![("type", "filter"), ("↑↓", "move"), ("Enter", "apply"), ("Esc", "clear")];
            }
            let t = m.highlighted_theme(ctx);
            let active = t.is_some_and(|t| ctx.active_theme.as_deref() == Some(t.slug.as_str()));
            let edited = active && ctx.theme_modified;
            let shared = t.is_some_and(|t| super::themes::is_gallery(&t.slug));
            let enter = if shared { "install" } else if edited { "revert" } else { "apply" };
            let mut v = vec![("↑↓", "move"), ("Enter", enter)];
            if t.is_some_and(|t| t.scene.is_some()) {
                v.push(("s", "its scene"));
            }
            if edited && t.is_some_and(|t| t.yours) {
                v.push(("U", "save changes"));
            }
            v.extend([("e", "edit"), ("n", "save as new"), ("c", "share"), ("i", "import"), ("g", "gallery")]);
            if t.is_some_and(|t| t.yours) {
                v.extend([("r", "rename"), ("x", "delete")]);
            }
            v.extend([("/", "search"), ("u", "undo"), ("Tab", "page"), ("?", "help"), ("Esc", "close")]);
            v
        }
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
                        v.push(("t", "variant"));
                    }
                }
                v.extend([("r", "random"), ("/", "search"), ("←", "categories")]);
            }
            v.extend([("Tab", "page"), ("?", "help"), ("Esc", close)]);
            v
        }
        _ if m.filters_open => {
            if m.filter_row() == 0 {
                vec![("↑↓", "move"), ("←→", "preset"), ("u", "undo"), ("Esc", "back"), ("Tab", "page")]
            } else {
                vec![("↑↓", "move"), ("Enter", "on/off"), ("←→", "strength"), ("u", "undo"), ("Esc", "back"), ("Tab", "page")]
            }
        }
        _ => {
            let set = &settings::page(m.page)[m.row()];
            let mut v = vec![("↑↓", "move")];
            let on = settings::enabled(set.id, ctx);
            match set.kind {
                Kind::Choice if on => v.push(("←→", "change")),
                Kind::Slider if on => v.push(("←→", "adjust")),
                Kind::Toggle => v.push(("←→", "toggle")),
                Kind::Open => v.push(("Enter", "open")),
                Kind::Action => v.push(("Enter", "apply")),
                _ => {}
            }
            v.extend([("u", "undo"), ("Tab", "page"), ("?", "help"), ("Esc", "close")]);
            v
        }
    }
}

/// Which hints go first when space runs out: plain movement is guessable,
/// how to act, get help and get out is not.
fn hint_rank(key: &str) -> u8 {
    match key {
        "Esc" | "?" | "Enter" | "type" => 0,
        "↑↓" | "Tab" | "←" | "→" | "u" => 2,
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
    ("u", "undo the last change"),
    ("/", "search scenes"),
    ("f", "star the highlighted scene"),
    ("t", "next variant for it"),
    ("mouse", "click anything · drag sliders · wheel scrolls"),
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
        "filter_next" => "try effects one by one",
        "detail_next" => "cycle quality",
        "pause" => "pause",
        "color" => "colour studio",
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
