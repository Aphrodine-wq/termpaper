//! The terminal check and the first-run welcome: two small overlays that
//! help a new setup look right.
//!
//! The check draws a colour gradient and three rows of block glyphs and
//! asks what shows. From the answers it sets Colours (24-bit or 256) and,
//! when the glyphs of the pixel mode in use do not show, a mode whose glyphs
//! do. The welcome runs once, on a fresh install: the keys that matter, the
//! check, then an invitation to pick a theme.

use crate::menu::{Effect, Input, Page};
use crate::prefs::{Colors, DisplayPrefs};
use crate::render::Pixels;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

/// What the check needs to know about the host.
#[derive(Clone, Debug)]
pub struct CheckCtx {
    /// the emulator's name, as far as the environment tells
    pub terminal: &'static str,
    /// the terminal says it shows 24-bit colour
    pub term_truecolor: bool,
    pub display: DisplayPrefs,
    pub pixels: Pixels,
}

/// The check's questions, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Question {
    Colour,
    Quadrants,
    Sextants,
    Braille,
}

impl Question {
    pub const ALL: [Question; 4] = [Question::Colour, Question::Quadrants, Question::Sextants, Question::Braille];

    pub fn title(self) -> &'static str {
        match self {
            Question::Colour => "Colour",
            Question::Quadrants => "Quadrants",
            Question::Sextants => "Sextants",
            Question::Braille => "Braille",
        }
    }

    fn ask(self) -> &'static str {
        match self {
            Question::Colour => "Does the bar fade smoothly, without steps or bands?",
            _ => "Do these show as small blocks and dots, not boxes, question marks or gaps?",
        }
    }

    /// The glyphs a pixel-mode question shows.
    pub fn sample(self) -> &'static str {
        match self {
            Question::Colour => "",
            Question::Quadrants => "▘ ▝ ▖ ▗ ▚ ▞ ▙ ▛ ▜ ▟ ▀ ▄",
            Question::Sextants => "🬀 🬁 🬂 🬃 🬄 🬅 🬆 🬇 🬈 🬉 🬋 🬏",
            Question::Braille => "⠁ ⠃ ⠇ ⡇ ⣇ ⣧ ⣷ ⣿ ⢸ ⠿ ⠶ ⣤",
        }
    }
}

/// The terminal check.
#[derive(Clone, Debug, Default)]
pub struct Check {
    pub open: bool,
    at: usize,
    answers: [Option<bool>; 4],
}

impl Check {
    pub fn open(&mut self) {
        *self = Check { open: true, ..Default::default() };
    }

    /// The question on screen.
    pub fn question(&self) -> Question {
        Question::ALL[self.at.min(Question::ALL.len() - 1)]
    }

    pub fn answers(&self) -> [Option<bool>; 4] {
        self.answers
    }

    /// y or Enter says yes, n no, ← goes back a question, Esc closes and
    /// changes nothing. The last answer closes it and returns the changes.
    pub fn handle(&mut self, input: Input, ctx: &CheckCtx) -> Vec<Effect> {
        match input {
            Input::Char('y') | Input::Char('Y') | Input::Enter => self.answer(true, ctx),
            Input::Char('n') | Input::Char('N') => self.answer(false, ctx),
            Input::Left | Input::Backspace | Input::Up => {
                self.at = self.at.saturating_sub(1);
                Vec::new()
            }
            Input::Esc | Input::Char('q') => {
                self.open = false;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn answer(&mut self, yes: bool, ctx: &CheckCtx) -> Vec<Effect> {
        self.answers[self.at] = Some(yes);
        self.at += 1;
        if self.at < Question::ALL.len() {
            return Vec::new();
        }
        self.open = false;
        self.results(ctx)
    }

    /// Whether a pixel mode's glyphs showed (unasked counts as yes).
    pub fn works(&self, p: Pixels) -> bool {
        let [_, quad, sext, braille] = self.answers;
        match p {
            Pixels::Half | Pixels::Ascii | Pixels::Blocks => true,
            Pixels::Quad => quad != Some(false),
            Pixels::Sextant => sext != Some(false),
            Pixels::Braille => braille != Some(false),
        }
    }

    /// The settings the answers call for.
    pub fn results(&self, ctx: &CheckCtx) -> Vec<Effect> {
        let mut fx = Vec::new();
        let colors = match self.answers[0] {
            Some(true) if ctx.term_truecolor => Colors::Auto,
            Some(true) => Colors::Truecolor,
            Some(false) => Colors::Ansi256,
            None => ctx.display.colors,
        };
        if colors != ctx.display.colors {
            fx.push(Effect::SetDisplay(DisplayPrefs { colors, ..ctx.display.clone() }));
        }
        if !self.works(ctx.pixels) {
            let p = [Pixels::Quad, Pixels::Half].into_iter().find(|p| self.works(*p)).unwrap_or(Pixels::Half);
            fx.push(Effect::SetPixels(p));
        }
        fx
    }

    /// One line saying what changed, for the host's notice.
    pub fn summary(&self, ctx: &CheckCtx) -> String {
        let colour = match self.answers[0] {
            Some(true) => "24-bit colour",
            Some(false) => "256 colours",
            None => "colours as they were",
        };
        let pixels = if self.works(ctx.pixels) {
            format!("{} pixels work", ctx.pixels.name())
        } else {
            let p = [Pixels::Quad, Pixels::Half].into_iter().find(|p| self.works(*p)).unwrap_or(Pixels::Half);
            format!("pixels now {}", p.name())
        };
        let extra = if self.answers[2] == Some(true) && ctx.pixels != Pixels::Sextant {
            " · sextants work too (Display → Pixels)"
        } else {
            ""
        };
        format!("Checked: {colour} · {pixels}{extra}")
    }
}

const PANEL_BG: Color = Color::Rgb(16, 18, 26);
const TEXT: Color = Color::Rgb(215, 220, 232);
const MUTED: Color = Color::Rgb(128, 136, 156);
const ACCENT: Color = Color::Rgb(122, 162, 247);
const GOOD: Color = Color::Rgb(158, 206, 106);
const BAD: Color = Color::Rgb(247, 118, 142);

/// A centred panel of at most `w`×`h` cells, cleared and framed.
fn panel(f: &mut Frame, area: Rect, w: u16, h: u16, title: &str) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let r = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, r);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(Span::styled(format!(" {title} "), Style::new().fg(TEXT).add_modifier(Modifier::BOLD)))
        .style(Style::new().bg(PANEL_BG));
    let inner = block.inner(r);
    f.render_widget(block, r);
    Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), inner.height)
}

fn key(k: &str) -> Span<'static> {
    Span::styled(k.to_string(), Style::new().fg(TEXT).add_modifier(Modifier::BOLD))
}

fn muted(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::new().fg(MUTED))
}

/// A smooth gradient through deep blue, violet and amber, one colour a cell
/// (24-bit, whatever Colours says: the point is to see what the terminal
/// does with it).
pub fn gradient(f: &mut Frame, r: Rect) {
    let stops = [(12.0, 24.0, 64.0), (122.0, 72.0, 190.0), (250.0, 168.0, 72.0)];
    let w = r.width.max(2) as f32 - 1.0;
    for x in 0..r.width {
        let t = x as f32 / w * 2.0;
        let (a, b, k) = if t < 1.0 { (stops[0], stops[1], t) } else { (stops[1], stops[2], t - 1.0) };
        let c = Color::Rgb(
            (a.0 + (b.0 - a.0) * k) as u8,
            (a.1 + (b.1 - a.1) * k) as u8,
            (a.2 + (b.2 - a.2) * k) as u8,
        );
        for y in r.y..r.y + r.height {
            if let Some(cell) = f.buffer_mut().cell_mut((r.x + x, y)) {
                cell.set_symbol(" ").set_bg(c);
            }
        }
    }
}

/// Draw the check over `area`.
pub fn render(f: &mut Frame, area: Rect, check: &Check, ctx: &CheckCtx) {
    let inner = panel(f, area, 68, 15, "Terminal check");
    if inner.height < 6 {
        return;
    }
    let q = check.question();
    let colour_word = if ctx.term_truecolor { "says it shows 24-bit colour" } else { "does not say it shows 24-bit colour" };
    let mut lines = vec![
        Line::from(vec![muted("This is "), Span::styled(ctx.terminal.to_string(), Style::new().fg(TEXT)), muted(&format!(", and it {colour_word}."))]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{} of {}  ", check.at.min(3) + 1, Question::ALL.len()), Style::new().fg(MUTED)),
            Span::styled(q.title().to_string(), Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
    ];
    let sample_at = inner.y + lines.len() as u16;
    if q == Question::Colour {
        lines.push(Line::from(""));
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(Span::styled(q.sample().to_string(), Style::new().fg(TEXT))));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(q.ask().to_string(), Style::new().fg(TEXT))));
    lines.push(Line::from(""));
    // what has been answered so far
    let mut so_far = Vec::new();
    for (i, qq) in Question::ALL.iter().enumerate() {
        let (mark, colour) = match check.answers[i] {
            Some(true) => ("✓", GOOD),
            Some(false) => ("✗", BAD),
            None => ("·", MUTED),
        };
        so_far.push(Span::styled(format!("{mark} "), Style::new().fg(colour)));
        so_far.push(muted(&format!("{}   ", qq.title())));
    }
    lines.push(Line::from(so_far));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        key("y"),
        muted(" yes   "),
        key("n"),
        muted(" no   "),
        key("←"),
        muted(" back   "),
        key("Esc"),
        muted(" close, change nothing"),
    ]));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    if q == Question::Colour {
        gradient(f, Rect::new(inner.x, sample_at, inner.width, 2));
    }
}

/// The welcome's pages.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WelcomePage {
    #[default]
    Keys,
    Check,
    Theme,
}

/// The first-run welcome.
#[derive(Clone, Debug, Default)]
pub struct Welcome {
    pub open: bool,
    pub page: WelcomePage,
    pub check: Check,
}

impl Welcome {
    pub fn open(&mut self) {
        *self = Welcome { open: true, ..Default::default() };
    }

    fn finish(&mut self, mut fx: Vec<Effect>) -> Vec<Effect> {
        self.open = false;
        fx.push(Effect::Onboarded);
        fx
    }

    /// Enter moves on, Esc skips the rest; the check page is the check.
    pub fn handle(&mut self, input: Input, ctx: &CheckCtx) -> Vec<Effect> {
        match self.page {
            WelcomePage::Keys => match input {
                Input::Enter | Input::Right | Input::Char(' ') => {
                    self.page = WelcomePage::Check;
                    self.check.open();
                    Vec::new()
                }
                Input::Esc | Input::Char('q') => self.finish(Vec::new()),
                _ => Vec::new(),
            },
            WelcomePage::Check => {
                let fx = self.check.handle(input, ctx);
                if !self.check.open {
                    self.page = WelcomePage::Theme;
                }
                fx
            }
            WelcomePage::Theme => match input {
                Input::Enter | Input::Right => self.finish(vec![Effect::OpenMenuAt(Page::Themes)]),
                Input::Esc | Input::Char('q') => self.finish(Vec::new()),
                _ => Vec::new(),
            },
        }
    }
}

/// Draw the welcome over `area`. `keys` are (key, what it does) as the
/// user's keymap has them.
pub fn render_welcome(f: &mut Frame, area: Rect, w: &Welcome, ctx: &CheckCtx, keys: &[(String, &str)]) {
    match w.page {
        WelcomePage::Check => render(f, area, &w.check, ctx),
        WelcomePage::Keys => {
            let inner = panel(f, area, 66, 9 + keys.len() as u16, "Welcome to termpaper");
            let mut lines = vec![Line::from(muted("A moving wallpaper for your terminal. The keys that matter:")), Line::from("")];
            for (k, what) in keys {
                lines.push(Line::from(vec![Span::styled(format!("  {k:<10}"), Style::new().fg(TEXT).add_modifier(Modifier::BOLD)), muted(what)]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(muted("Next, a quick check of what this terminal can show.")));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![key("Enter"), muted(" next    "), key("Esc"), muted(" skip")]));
            f.render_widget(Paragraph::new(lines), inner);
        }
        WelcomePage::Theme => {
            let inner = panel(f, area, 66, 9, "Pick a look");
            let lines = vec![
                Line::from(muted("Themes recolour every scene: film grades, terminal palettes,")),
                Line::from(muted("retro screens. Browse them with a live preview.")),
                Line::from(""),
                Line::from(vec![key("Enter"), muted(" browse themes    "), key("Esc"), muted(" keep it as it is")]),
                Line::from(""),
                Line::from(muted("Any time later: ? opens the menu, Tab moves between pages.")),
            ];
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn ctx() -> CheckCtx {
        CheckCtx { terminal: "kitty", term_truecolor: true, display: DisplayPrefs::default(), pixels: Pixels::Quad }
    }

    fn answer(c: &mut Check, ctx: &CheckCtx, yes: [bool; 4]) -> Vec<Effect> {
        yes.iter().flat_map(|y| c.handle(Input::Char(if *y { 'y' } else { 'n' }), ctx)).collect()
    }

    #[test]
    fn all_yes_changes_nothing() {
        let mut c = Check::default();
        c.open();
        assert!(answer(&mut c, &ctx(), [true; 4]).is_empty());
        assert!(!c.open);
        assert_eq!(c.summary(&ctx()), "Checked: 24-bit colour · quad pixels work · sextants work too (Display → Pixels)");
    }

    #[test]
    fn banding_and_missing_glyphs_pick_what_works() {
        let mut c = Check::default();
        c.open();
        // banded colour, no quadrants, sextants fine, no braille
        let fx = answer(&mut c, &ctx(), [false, false, true, false]);
        assert_eq!(
            fx,
            vec![
                Effect::SetDisplay(DisplayPrefs { colors: Colors::Ansi256, ..Default::default() }),
                Effect::SetPixels(Pixels::Half)
            ]
        );
        // braille in use and missing: quadrants work, so quadrants
        let mut c = Check::default();
        c.open();
        let braille = CheckCtx { pixels: Pixels::Braille, ..ctx() };
        assert_eq!(answer(&mut c, &braille, [true, true, false, false]), vec![Effect::SetPixels(Pixels::Quad)]);
        // a smooth bar where the terminal did not say 24-bit: say it
        let mut c = Check::default();
        c.open();
        let quiet = CheckCtx { term_truecolor: false, ..ctx() };
        assert_eq!(
            answer(&mut c, &quiet, [true; 4]),
            vec![Effect::SetDisplay(DisplayPrefs { colors: Colors::Truecolor, ..Default::default() })]
        );
    }

    #[test]
    fn back_and_escape() {
        let mut c = Check::default();
        c.open();
        c.handle(Input::Char('n'), &ctx());
        assert_eq!(c.question(), Question::Quadrants);
        c.handle(Input::Left, &ctx());
        assert_eq!(c.question(), Question::Colour);
        assert!(c.handle(Input::Esc, &ctx()).is_empty());
        assert!(!c.open);
    }

    #[test]
    fn the_welcome_walks_through_and_marks_itself_done() {
        let mut w = Welcome::default();
        w.open();
        assert!(w.handle(Input::Enter, &ctx()).is_empty());
        assert_eq!(w.page, WelcomePage::Check);
        for _ in 0..4 {
            w.handle(Input::Char('y'), &ctx());
        }
        assert_eq!(w.page, WelcomePage::Theme);
        assert_eq!(w.handle(Input::Enter, &ctx()), vec![Effect::OpenMenuAt(Page::Themes), Effect::Onboarded]);
        assert!(!w.open);
        // skipped at once
        let mut w = Welcome::default();
        w.open();
        assert_eq!(w.handle(Input::Esc, &ctx()), vec![Effect::Onboarded]);
    }

    #[test]
    fn panels_draw_at_any_size() {
        for (wd, ht) in [(120, 40), (80, 24), (40, 12), (10, 4), (1, 1)] {
            let mut t = Terminal::new(TestBackend::new(wd, ht)).unwrap();
            let mut c = Check::default();
            c.open();
            t.draw(|f| render(f, f.area(), &c, &ctx())).unwrap();
            let mut w = Welcome::default();
            w.open();
            let keys = vec![("?".to_string(), "the menu")];
            t.draw(|f| render_welcome(f, f.area(), &w, &ctx(), &keys)).unwrap();
        }
    }
}
