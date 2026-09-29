//! The `?` menu: a drawer down the left of the screen with the live scene
//! still playing beside it. A pure state machine — keys in, [`Effect`]s out —
//! so all of it is tested without a terminal; `main.rs` applies the effects.
//!
//! Pages: Scenes (the browser, see [`browser`]) and four settings pages
//! declared as data in [`settings`]. Drawing lives in [`view`].

pub mod browser;
pub mod settings;
#[cfg(test)]
mod tests;
pub mod themes;
pub mod view;

use crate::config::CycleScope;
use crate::engine::Renderer;
use crate::render::Pixels;
use crate::scene::Detail;
use browser::{Browser, Column};
use ratatui::layout::Rect;
use settings::{Kind, SettingId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long the browser highlight must rest on a scene before it previews.
pub const PREVIEW_DELAY: Duration = Duration::from_millis(300);

/// How long a footer notice ("coming soon") stays up.
pub const FLASH_FOR: Duration = Duration::from_secs(3);

/// FPS presets — menu ◂/▸ and `[`/`]` keys step through these (120 = high-refresh sweet spot).
pub const FPS_PRESETS: &[u32] = &[10, 24, 30, 60, 90, 120, 144, 165, 240];

/// Speed presets for the Playback page and `,`/`.` keys.
pub const SPEED_PRESETS: &[f32] = &[0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

fn step_preset_u32(cur: u32, presets: &[u32], up: bool) -> u32 {
    if let Some(i) = presets.iter().position(|&v| v == cur) {
        return if up {
            presets[(i + 1).min(presets.len() - 1)]
        } else if i > 0 {
            presets[i - 1]
        } else {
            presets[0]
        };
    }
    if up {
        presets
            .iter()
            .find(|&&v| v > cur)
            .copied()
            .unwrap_or(*presets.last().unwrap())
    } else {
        presets
            .iter()
            .rfind(|&&v| v < cur)
            .copied()
            .unwrap_or(presets[0])
    }
}

fn step_preset_f32(cur: f32, presets: &[f32], up: bool) -> f32 {
    let near = |v: f32| presets.iter().position(|&p| (p - v).abs() < 0.001);
    if let Some(i) = near(cur) {
        return if up {
            presets[(i + 1).min(presets.len() - 1)]
        } else if i > 0 {
            presets[i - 1]
        } else {
            presets[0]
        };
    }
    if up {
        presets
            .iter()
            .find(|&&v| v > cur)
            .copied()
            .unwrap_or(*presets.last().unwrap())
    } else {
        presets
            .iter()
            .rfind(|&&v| v < cur)
            .copied()
            .unwrap_or(presets[0])
    }
}

/// Step FPS through [`FPS_PRESETS`].
pub fn fps_step(cur: u32, up: bool) -> u32 {
    step_preset_u32(cur, FPS_PRESETS, up)
}

/// Step speed through [`SPEED_PRESETS`].
pub fn speed_step(cur: f32, up: bool) -> f32 {
    step_preset_f32(cur, SPEED_PRESETS, up)
}

/// The menu's pages, in tab order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Scenes,
    Themes,
    Look,
    Playback,
    Display,
    Wall,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::Scenes,
        Page::Themes,
        Page::Look,
        Page::Playback,
        Page::Display,
        Page::Wall,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Scenes => "Scenes",
            Page::Themes => "Themes",
            Page::Look => "Look",
            Page::Playback => "Playback",
            Page::Display => "Display",
            Page::Wall => "Wall",
        }
    }

    /// For narrow drawers.
    pub fn short_title(self) -> &'static str {
        match self {
            Page::Scenes => "Scenes",
            Page::Themes => "Themes",
            Page::Look => "Look",
            Page::Playback => "Play",
            Page::Display => "Show",
            Page::Wall => "Wall",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    fn offset(self, d: isize) -> Page {
        let n = Self::ALL.len() as isize;
        Self::ALL[(self.index() as isize + d).rem_euclid(n) as usize]
    }
}

/// What the mouse did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mouse {
    /// left button pressed
    Down,
    /// moved with the left button held
    Drag,
    ScrollUp,
    ScrollDown,
}

/// What a point on screen is, recorded by the view while it draws: the
/// mouse goes through exactly the same effects as the keys.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// anywhere outside the drawer (the live scene)
    Outside,
    /// the drawer's own empty space
    Drawer,
    Tab(Page),
    /// a settings row's label
    Row(usize),
    /// a settings row's value (toggle, choice, open, action)
    Value(usize),
    /// a slider's bar: its row, first column and width
    Slider { row: usize, x0: u16, width: u16 },
    Shelf(usize),
    Scene(usize),
    /// a theme in the Themes list (its index in the visible list)
    Theme(usize),
    /// an Effects sub-page row, and its on/off box
    FilterRow(usize),
    FilterBox(usize),
    /// an effect's strength bar
    FilterSlider { row: usize, x0: u16, width: u16 },
}

#[derive(Clone, Copy, Debug)]
pub struct HitBox {
    pub rect: Rect,
    pub hit: Hit,
}

/// Two clicks on the same thing within this are a double click.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// A key or mouse action, as the menu sees it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Char(char),
    Mouse { kind: Mouse, x: u16, y: u16 },
}

impl Input {
    /// Map a terminal mouse event. Right and middle buttons are ignored.
    pub fn from_mouse(ev: crossterm::event::MouseEvent) -> Option<Input> {
        use crossterm::event::{MouseButton, MouseEventKind as K};
        let kind = match ev.kind {
            K::Down(MouseButton::Left) => Mouse::Down,
            K::Drag(MouseButton::Left) => Mouse::Drag,
            K::ScrollUp => Mouse::ScrollUp,
            K::ScrollDown => Mouse::ScrollDown,
            _ => return None,
        };
        Some(Input::Mouse { kind, x: ev.column, y: ev.row })
    }

    /// Map a terminal key press. `None` for keys the menu ignores, including
    /// Ctrl/Alt chords — those stay with the host, so Ctrl-C still quits.
    pub fn from_key(key: crossterm::event::KeyEvent) -> Option<Input> {
        use crossterm::event::{KeyCode as K, KeyModifiers as M};
        if key.modifiers.intersects(M::CONTROL | M::ALT) {
            return None;
        }
        Some(match key.code {
            K::Up => Input::Up,
            K::Down => Input::Down,
            K::Left => Input::Left,
            K::Right => Input::Right,
            K::PageUp => Input::PageUp,
            K::PageDown => Input::PageDown,
            K::Home => Input::Home,
            K::End => Input::End,
            K::Enter => Input::Enter,
            K::Esc => Input::Esc,
            K::Tab if key.modifiers.contains(M::SHIFT) => Input::BackTab,
            K::Tab => Input::Tab,
            K::BackTab => Input::BackTab,
            K::Backspace => Input::Backspace,
            K::Char(c) => Input::Char(c),
            _ => return None,
        })
    }
}

/// Side effects the host applies. The host also persists whatever changed
/// (debounced), so there is no separate "save" effect.
#[derive(Clone, PartialEq, Debug)]
pub enum Effect {
    /// Switch to a scene and publish it to the linked group. A scene that is
    /// already on screen as a preview is kept exactly as it is.
    SwitchScene(&'static str),
    /// Show a scene on this pane only, without publishing (browser hover).
    Preview(&'static str),
    /// Drop the preview: back to the scene and theme shown before it.
    EndPreview,
    /// Remember a theme for a scene (`t` in the browser); applied live when
    /// that scene is on screen.
    SetSceneTheme {
        scene: &'static str,
        theme: String,
    },
    ToggleFavorite(&'static str),
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
    SetCycleScope(CycleScope),
    /// order, transition style, time of day, what opens on launch
    SetPlayback(crate::prefs::PlaybackPrefs),
    /// colours, power, overlays, night dimming and the rest of Display
    SetDisplay(crate::prefs::DisplayPrefs),
    /// the terminal check: what this terminal can show
    OpenTerminalCheck,
    /// open the menu on a page (the welcome's last step)
    OpenMenuAt(Page),
    /// the first-run welcome is done (or skipped): never again
    Onboarded,
    SetRenderer(Renderer),
    SetLink(bool),
    SetLinkGroup(String),
    /// wall layout, padding, placement, bezels, sync look
    SetWallPrefs(crate::prefs::WallPrefs),
    /// pause (or resume) every pane in the group
    PauseWall,
    /// open a termpaper on every monitor (Hyprland)
    WallUp,
    /// close the wall terminals
    WallDown,
    /// replace the Look: grade, palette, effect stack and strengths
    SetLook(crate::look::Look),
    /// open the colour studio over the menu
    OpenColorGrade,
    /// open the colour studio on its palette
    OpenPalette,
    /// put back what the last change replaced (`u`)
    Undo,
    /// monitor alignment tool — not wired yet; the host flashes a notice
    OpenCalibration,
    /// apply a theme's look (by slug) and remember it as the active theme
    ApplyTheme(String),
    /// show a look on this pane only while the Themes page browses
    PreviewLook(crate::look::Look),
    /// drop the look preview: back to the look before it
    EndLookPreview,
    /// save the current look as a new theme with this name
    SaveTheme(String),
    /// save the current look into one of your themes
    UpdateTheme(String),
    RenameTheme { slug: String, name: String },
    DeleteTheme(String),
    /// copy a theme's share code (OSC 52) and show it
    ShareTheme(String),
    /// a tp1: code or a theme file's path
    ImportTheme(String),
    /// switch to the scene (and variant) a theme was made for
    SceneFromTheme(String),
}

impl Effect {
    /// Changes `u` can take back: settings, not navigation or actions.
    pub fn undoable(&self) -> bool {
        matches!(
            self,
            Effect::SetPixels(_)
                | Effect::SetDetail(_)
                | Effect::SetTheme(_)
                | Effect::SetTextScale(_)
                | Effect::SetSpeed(_)
                | Effect::SetFps(_)
                | Effect::SetSmooth(_)
                | Effect::SetDim(_)
                | Effect::SetFade(_)
                | Effect::SetClock(_)
                | Effect::SetCycle(_)
                | Effect::SetCycleScope(_)
                | Effect::SetRenderer(_)
                | Effect::SetLook(_)
                | Effect::ApplyTheme(_)
                | Effect::SetPlayback(_)
                | Effect::SetDisplay(_)
                | Effect::SetWallPrefs(_)
        )
    }
}

/// Snapshot of the host's current settings, for display and adjustment.
#[derive(Clone, Default)]
pub struct MenuCtx {
    pub renderer_status: String,
    /// "wall: WxH cells @ (x,y)" when this pane is a crop of a video wall,
    /// else "wall: local"
    pub wall_status: String,
    /// scene on screen (a preview while one is showing)
    pub scene_name: &'static str,
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
    pub cycle_scope: CycleScope,
    /// grade, palette and effect stack
    pub look: crate::look::Look,
    pub renderer: Renderer,
    /// whether Studio scenes can render here; None = not known yet
    pub gpu: Option<bool>,
    pub link_enabled: bool,
    pub link_group: String,
    pub wall_enabled: bool,
    pub truecolor: bool,
    pub favorites: Vec<String>,
    /// newest first
    pub recents: Vec<String>,
    /// remembered per-scene themes (`[themes]`)
    pub scene_themes: HashMap<String, String>,
    /// (action, key) for every rebindable action
    pub key_display: Vec<(String, String)>,
    pub instances: Vec<String>,
    /// every theme, in the order the Themes page lists them
    pub themes: std::sync::Arc<Vec<themes::ThemeRow>>,
    /// the theme the look came from (slug), and whether it has changed since
    pub active_theme: Option<String>,
    pub theme_modified: bool,
    pub playback: crate::prefs::PlaybackPrefs,
    pub display: crate::prefs::DisplayPrefs,
    pub wall: crate::prefs::WallPrefs,
    /// running under Hyprland (the physical wall, bezels, start/stop wall)
    pub hypr: bool,
    /// the group's picture is paused
    pub paused: bool,
    /// the terminal says it shows 24-bit colour (what Colours: auto means)
    pub term_truecolor: bool,
}

/// Whether Studio (GPU) scenes can render, from what the host knows: the
/// build, the chosen renderer, and the worker's backend status line.
pub fn gpu_state(status: &str, compiled: bool, renderer: Renderer) -> Option<bool> {
    if !compiled || renderer == Renderer::Cpu {
        return Some(false);
    }
    if status.starts_with("GPU") {
        return Some(true);
    }
    if ["unavailable", "no GPU", "GPU error", "not compiled"]
        .iter()
        .any(|s| status.contains(s))
    {
        return Some(false);
    }
    None
}

pub struct Menu {
    pub open: bool,
    pub page: Page,
    /// the `?` help overlay
    pub help: bool,
    help_scroll: u16,
    /// Themes page: highlighted row (in the visible list), search
    pub theme_row: usize,
    pub theme_query: String,
    pub theme_searching: bool,
    /// highlighted theme and when the highlight landed on it (preview timer)
    theme_hover: Option<(usize, Instant)>,
    /// a look preview is showing behind the menu
    look_previewing: bool,
    /// a question in the footer waiting for typed text
    pub prompt: Option<themes::Prompt>,
    /// Look → Filters… sub-page is showing
    pub filters_open: bool,
    filter_row: usize,
    /// focused row on each settings page, by `Page::index`
    rows: [usize; 6],
    pub browser: Browser,
    /// scene on screen when the menu opened: what Esc returns to
    origin: &'static str,
    /// scene previewed behind the drawer
    previewing: Option<&'static str>,
    /// highlighted scene and when the highlight landed on it
    hover: Option<(&'static str, Instant)>,
    flash: Option<(String, Instant)>,
    /// visible list height from the last draw, for PgUp/PgDn
    page_len: Cell<usize>,
    /// furthest the help text can scroll, from the last draw
    help_max: Cell<u16>,
    /// what is where on screen, from the last draw (topmost last)
    pub hits: RefCell<Vec<HitBox>>,
    /// the last click, for double clicks
    last_click: Option<(Hit, Instant)>,
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    pub fn new() -> Self {
        Menu {
            open: false,
            page: Page::Scenes,
            help: false,
            help_scroll: 0,
            theme_row: 0,
            theme_query: String::new(),
            theme_searching: false,
            theme_hover: None,
            look_previewing: false,
            prompt: None,
            filters_open: false,
            filter_row: 0,
            rows: [0; 6],
            browser: Browser::new(),
            origin: "",
            previewing: None,
            hover: None,
            flash: None,
            page_len: Cell::new(10),
            help_max: Cell::new(u16::MAX),
            hits: RefCell::new(Vec::new()),
            last_click: None,
        }
    }

    /// Open over the scene on screen, with the browser pointing at it. The
    /// last page used is kept.
    pub fn open(&mut self, ctx: &MenuCtx) {
        self.open = true;
        self.help = false;
        self.prompt = None;
        self.theme_searching = false;
        self.theme_query.clear();
        self.look_previewing = false;
        self.theme_hover = None;
        // the Themes list opens on the active theme
        if let Some(active) = &ctx.active_theme {
            if let Some(i) = ctx.themes.iter().position(|t| &t.slug == active) {
                self.theme_row = i;
            }
        }
        self.filters_open = false;
        self.origin = ctx.scene_name;
        self.previewing = None;
        self.hover = None;
        self.browser.clear_search();
        self.browser.column = Column::Scenes;
        self.browser.reveal(ctx.scene_name, ctx);
    }

    /// Close, undoing a preview that was never kept.
    pub fn close(&mut self) -> Vec<Effect> {
        self.open = false;
        self.help = false;
        self.prompt = None;
        self.filters_open = false;
        self.browser.clear_search();
        let mut fx = self.end_preview();
        fx.extend(self.end_look_preview());
        fx
    }

    fn end_look_preview(&mut self) -> Vec<Effect> {
        self.theme_hover = None;
        if std::mem::take(&mut self.look_previewing) {
            vec![Effect::EndLookPreview]
        } else {
            Vec::new()
        }
    }

    /// Whether a look preview is showing (the host keeps the real look).
    pub fn look_previewing(&self) -> bool {
        self.look_previewing
    }

    fn end_preview(&mut self) -> Vec<Effect> {
        self.hover = None;
        match self.previewing.take() {
            Some(_) => vec![Effect::EndPreview],
            None => Vec::new(),
        }
    }

    /// Scene previewed behind the drawer, if any.
    pub fn previewing(&self) -> Option<&'static str> {
        self.previewing
    }

    /// True while keystrokes are text (a search box, a prompt), so the host
    /// must not treat them as shortcuts.
    pub fn typing(&self) -> bool {
        self.open
            && !self.help
            && (self.prompt.is_some()
                || (self.page == Page::Scenes && self.browser.searching)
                || (self.page == Page::Themes && self.theme_searching))
    }

    /// Paste text into whatever is being typed (a share code, a name).
    pub fn paste(&mut self, text: &str) {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if let Some(p) = &mut self.prompt {
            let room = p.max_len().saturating_sub(p.text.chars().count());
            p.text.extend(clean.chars().take(room));
        } else if self.page == Page::Themes && self.theme_searching {
            self.theme_query.push_str(&clean);
            self.theme_row = 0;
        } else if self.page == Page::Scenes && self.browser.searching {
            self.browser.query.push_str(&clean);
            self.browser.row = 0;
        }
    }

    /// Show a short notice in the footer.
    pub fn flash(&mut self, msg: impl Into<String>) {
        self.flash = Some((msg.into(), Instant::now()));
    }

    fn flash_text(&self, now: Instant) -> Option<&str> {
        self.flash
            .as_ref()
            .filter(|(_, at)| now.saturating_duration_since(*at) < FLASH_FOR)
            .map(|(s, _)| s.as_str())
    }

    /// Focused row on the current settings page.
    pub fn row(&self) -> usize {
        let n = settings::page(self.page).len();
        self.rows[self.page.index()].min(n.saturating_sub(1))
    }

    /// Focused row on the Filters sub-page (0 = preset).
    pub fn filter_row(&self) -> usize {
        self.filter_row
    }

    pub fn handle(&mut self, input: Input, ctx: &MenuCtx) -> Vec<Effect> {
        if !self.open {
            return Vec::new();
        }
        if let Input::Mouse { kind, x, y } = input {
            return self.handle_mouse(kind, x, y, ctx);
        }
        if self.prompt.is_some() {
            return self.handle_prompt(input);
        }
        if self.help {
            self.handle_help(input);
            return Vec::new();
        }
        match input {
            Input::Tab => return self.goto(self.page.offset(1)),
            Input::BackTab => return self.goto(self.page.offset(-1)),
            _ => {}
        }
        match self.page {
            Page::Scenes => self.handle_browser(input, ctx),
            Page::Themes => self.handle_themes(input, ctx),
            _ if self.filters_open => self.handle_filters(input, ctx),
            _ => self.handle_settings(input, ctx),
        }
    }

    /// Text into the prompt; Enter answers it, Esc drops it.
    fn handle_prompt(&mut self, input: Input) -> Vec<Effect> {
        let Some(p) = &mut self.prompt else {
            return Vec::new();
        };
        match input {
            Input::Char(c) => {
                if p.text.chars().count() < p.max_len() {
                    p.text.push(c);
                }
            }
            Input::Backspace => {
                p.text.pop();
            }
            Input::Esc => self.prompt = None,
            Input::Enter => {
                let p = self.prompt.take().unwrap();
                let text = p.text.trim().to_string();
                return match p.kind {
                    themes::PromptKind::NewTheme if !text.is_empty() => vec![Effect::SaveTheme(text)],
                    themes::PromptKind::Rename(slug) if !text.is_empty() => vec![Effect::RenameTheme { slug, name: text }],
                    themes::PromptKind::Delete(slug) if text.eq_ignore_ascii_case("y") || text.eq_ignore_ascii_case("yes") => {
                        vec![Effect::DeleteTheme(slug)]
                    }
                    themes::PromptKind::Import if !text.is_empty() => vec![Effect::ImportTheme(text)],
                    themes::PromptKind::NewGroup if !crate::link::sanitize_group(&text).is_empty() => {
                        vec![Effect::SetLinkGroup(crate::link::sanitize_group(&text))]
                    }
                    _ => Vec::new(),
                };
            }
            _ => {}
        }
        Vec::new()
    }

    /// Put the highlight on a theme after the list changed under it (one
    /// saved, renamed or imported), dropping a search that would hide it.
    pub fn focus_theme(&mut self, slug: &str, rows: &[themes::ThemeRow]) {
        if let Some(i) = themes::visible(rows, &self.theme_query).iter().position(|t| t.slug == slug) {
            self.theme_row = i;
        } else if let Some(i) = rows.iter().position(|t| t.slug == slug) {
            self.theme_query.clear();
            self.theme_searching = false;
            self.theme_row = i;
        }
        self.theme_hover = None;
    }

    /// The highlighted theme, if any.
    pub fn highlighted_theme<'a>(&self, ctx: &'a MenuCtx) -> Option<&'a themes::ThemeRow> {
        let list = themes::visible(&ctx.themes, &self.theme_query);
        list.get(self.theme_row.min(list.len().saturating_sub(1))).copied()
    }

    /// Restart the preview timer on the highlighted theme.
    fn touch_theme(&mut self) {
        self.theme_hover = Some((self.theme_row, Instant::now()));
    }

    fn handle_themes(&mut self, input: Input, ctx: &MenuCtx) -> Vec<Effect> {
        let n = themes::visible(&ctx.themes, &self.theme_query).len();
        let page = self.page_len.get().max(1);
        let mv = |row: &mut usize, d: isize| {
            *row = (*row as isize + d).clamp(0, n.saturating_sub(1) as isize) as usize;
        };
        if self.theme_searching {
            match input {
                Input::Char(c) => {
                    self.theme_query.push(c);
                    self.theme_row = 0;
                }
                Input::Backspace => {
                    if self.theme_query.pop().is_none() {
                        self.theme_searching = false;
                    }
                    self.theme_row = 0;
                }
                Input::Esc => {
                    self.theme_searching = false;
                    self.theme_query.clear();
                }
                Input::Enter => {
                    // back to the whole list, on the theme picked
                    self.theme_searching = false;
                    let hit = self.highlighted_theme(ctx).map(|t| t.slug.clone());
                    self.theme_query.clear();
                    if let Some(slug) = hit {
                        self.theme_row = ctx.themes.iter().position(|t| t.slug == slug).unwrap_or(0);
                        self.look_previewing = false;
                        self.theme_hover = None;
                        return vec![Effect::ApplyTheme(slug)];
                    }
                }
                Input::Up => mv(&mut self.theme_row, -1),
                Input::Down => mv(&mut self.theme_row, 1),
                _ => {}
            }
            self.touch_theme();
            return Vec::new();
        }
        let hit = self.highlighted_theme(ctx).cloned();
        match input {
            Input::Up | Input::Char('k') => mv(&mut self.theme_row, -1),
            Input::Down | Input::Char('j') => mv(&mut self.theme_row, 1),
            Input::PageUp => mv(&mut self.theme_row, -(page as isize)),
            Input::PageDown => mv(&mut self.theme_row, page as isize),
            Input::Home => self.theme_row = 0,
            Input::End => self.theme_row = n.saturating_sub(1),
            Input::Enter => {
                if let Some(t) = hit {
                    // the preview becomes the real thing
                    self.look_previewing = false;
                    self.theme_hover = None;
                    return vec![Effect::ApplyTheme(t.slug)];
                }
                return Vec::new();
            }
            Input::Char('/') => {
                self.theme_searching = true;
                self.theme_query.clear();
                return Vec::new();
            }
            Input::Char('s') => {
                if let Some(t) = hit.filter(|t| t.scene.is_some()) {
                    return vec![Effect::SceneFromTheme(t.slug)];
                }
                return Vec::new();
            }
            Input::Char('e') => {
                if let Some(t) = hit {
                    self.look_previewing = false;
                    self.theme_hover = None;
                    let mut fx = Vec::new();
                    if ctx.active_theme.as_deref() != Some(t.slug.as_str()) {
                        fx.push(Effect::ApplyTheme(t.slug));
                    }
                    fx.push(Effect::OpenColorGrade);
                    return fx;
                }
                return Vec::new();
            }
            Input::Char('n') => {
                // saves the look you have: back on screen while you name it
                let fx = self.end_look_preview();
                self.prompt = Some(themes::Prompt::new(themes::PromptKind::NewTheme, ""));
                return fx;
            }
            Input::Char('U') => {
                let active = |t: &themes::ThemeRow| ctx.active_theme.as_deref() == Some(t.slug.as_str());
                match hit {
                    Some(t) if t.yours && active(&t) && ctx.theme_modified => {
                        let mut fx = self.end_look_preview();
                        fx.push(Effect::UpdateTheme(t.slug));
                        return fx;
                    }
                    Some(t) if t.yours && active(&t) => self.flash("No changes to save"),
                    Some(t) if t.yours => self.flash("U saves your edits into the theme in use: apply this one first"),
                    _ => self.flash("Built-in themes stay as they are: n saves your look as a new theme"),
                }
                return Vec::new();
            }
            Input::Char('r') => {
                match hit.filter(|t| t.yours) {
                    Some(t) => self.prompt = Some(themes::Prompt::new(themes::PromptKind::Rename(t.slug), t.name)),
                    None => self.flash("Built-in themes keep their names: n saves your look as a new theme"),
                }
                return Vec::new();
            }
            Input::Char('x') => {
                match hit.filter(|t| t.yours) {
                    Some(t) => self.prompt = Some(themes::Prompt::new(themes::PromptKind::Delete(t.slug), "")),
                    None => self.flash("Built-in themes cannot be deleted"),
                }
                return Vec::new();
            }
            Input::Char('c') => {
                if let Some(t) = hit {
                    return vec![Effect::ShareTheme(t.slug)];
                }
                return Vec::new();
            }
            Input::Char('i') => {
                self.prompt = Some(themes::Prompt::new(themes::PromptKind::Import, ""));
                return Vec::new();
            }
            Input::Char('u') => return vec![Effect::Undo],
            Input::Char('?') => {
                self.help = true;
                return Vec::new();
            }
            Input::Esc => return self.close(),
            _ => return Vec::new(),
        }
        self.touch_theme();
        Vec::new()
    }

    /// Switch pages. Leaving the browser drops the search and any preview:
    /// previews only live while browsing.
    pub fn goto(&mut self, page: Page) -> Vec<Effect> {
        self.filters_open = false;
        let mut fx = Vec::new();
        if self.page == Page::Scenes && page != Page::Scenes {
            self.browser.clear_search();
            fx = self.end_preview();
        }
        if self.page == Page::Themes && page != Page::Themes {
            self.theme_searching = false;
            fx.extend(self.end_look_preview());
        }
        self.page = page;
        fx
    }

    /// Advance timers: fires the browser preview once the highlight has
    /// rested for [`PREVIEW_DELAY`]. Call every frame while open.
    pub fn tick(&mut self, now: Instant) -> Vec<Effect> {
        if !self.open || self.help || self.page != Page::Scenes {
            return Vec::new();
        }
        let Some((name, at)) = self.hover else {
            return Vec::new();
        };
        if now.saturating_duration_since(at) < PREVIEW_DELAY {
            return Vec::new();
        }
        self.hover = None;
        if name == self.origin {
            // back on the scene the menu opened over
            return self.end_preview();
        }
        if self.previewing == Some(name) {
            return Vec::new();
        }
        self.previewing = Some(name);
        vec![Effect::Preview(name)]
    }

    /// `tick`, with what the Themes page's preview needs: once the highlight
    /// rests on a theme, its look shows on this pane (not published).
    pub fn tick_with(&mut self, now: Instant, ctx: &MenuCtx) -> Vec<Effect> {
        if !(self.open && !self.help && self.prompt.is_none() && self.page == Page::Themes) {
            return self.tick(now);
        }
        let Some((row, at)) = self.theme_hover else {
            return Vec::new();
        };
        if now.saturating_duration_since(at) < PREVIEW_DELAY {
            return Vec::new();
        }
        self.theme_hover = None;
        let list = themes::visible(&ctx.themes, &self.theme_query);
        let Some(t) = list.get(row.min(list.len().saturating_sub(1))) else {
            return Vec::new();
        };
        // resting on the theme in use shows your look as it is, edits and all
        if ctx.active_theme.as_deref() == Some(t.slug.as_str()) {
            return self.end_look_preview();
        }
        self.look_previewing = true;
        vec![Effect::PreviewLook(t.look.clone())]
    }

    /// Restart the preview timer on whatever the browser now highlights.
    fn touch(&mut self, ctx: &MenuCtx) {
        self.hover = self
            .browser
            .highlighted(ctx)
            .map(|e| (e.name(), Instant::now()));
    }

    /// The topmost thing drawn at (x, y) in the last frame.
    pub fn hit_at(&self, x: u16, y: u16) -> Option<Hit> {
        self.hits
            .borrow()
            .iter()
            .rev()
            .find(|h| x >= h.rect.x && x < h.rect.x + h.rect.width && y >= h.rect.y && y < h.rect.y + h.rect.height)
            .map(|h| h.hit)
    }

    /// Clicks, drags and the wheel, through the same paths as the keys.
    fn handle_mouse(&mut self, kind: Mouse, x: u16, y: u16, ctx: &MenuCtx) -> Vec<Effect> {
        let scroll = match kind {
            Mouse::ScrollUp => Some(Input::Up),
            Mouse::ScrollDown => Some(Input::Down),
            _ => None,
        };
        if let Some(key) = scroll {
            // the wheel moves the selection of whatever is showing, without
            // wrapping round the ends of a settings page
            if self.help {
                self.handle_help(key);
                return Vec::new();
            }
            if self.page != Page::Scenes && !self.filters_open {
                let n = settings::page(self.page).len();
                let r = self.row();
                let at_end = (key == Input::Up && r == 0) || (key == Input::Down && r + 1 >= n);
                if at_end {
                    return Vec::new();
                }
            }
            return self.handle(key, ctx);
        }
        let Some(hit) = self.hit_at(x, y) else {
            return Vec::new();
        };
        let now = Instant::now();
        let double = kind == Mouse::Down
            && self.last_click.is_some_and(|(h, at)| h == hit && now.saturating_duration_since(at) < DOUBLE_CLICK);
        if kind == Mouse::Down {
            self.last_click = Some((hit, now));
        }
        if self.help {
            // a click anywhere closes the help overlay
            if kind == Mouse::Down {
                self.help = false;
            }
            return Vec::new();
        }
        let pi = self.page.index();
        match (kind, hit) {
            (Mouse::Down, Hit::Outside) => self.close(),
            (Mouse::Down, Hit::Tab(p)) => self.goto(p),
            (Mouse::Down, Hit::Row(r)) => {
                self.rows[pi] = r;
                Vec::new()
            }
            (Mouse::Down, Hit::Value(r)) => {
                self.rows[pi] = r;
                self.handle(Input::Enter, ctx)
            }
            (_, Hit::Slider { row, x0, width }) => {
                self.rows[pi] = row;
                let frac = x.saturating_sub(x0) as f32 / width.saturating_sub(1).max(1) as f32;
                let id = settings::page(self.page).get(row).map(|s| s.id);
                id.and_then(|id| settings::set_fraction(id, ctx, frac)).into_iter().collect()
            }
            (Mouse::Down, Hit::Shelf(i)) => {
                self.browser.column = Column::Shelves;
                self.browser.shelf = i;
                self.browser.row = 0;
                self.hover = None;
                Vec::new()
            }
            (Mouse::Down, Hit::Scene(i)) => {
                self.browser.column = Column::Scenes;
                self.browser.row = i;
                if double {
                    return self.switch(ctx);
                }
                self.touch(ctx);
                Vec::new()
            }
            (Mouse::Down, Hit::Theme(i)) => {
                self.theme_row = i;
                if double {
                    return self.handle_themes(Input::Enter, ctx);
                }
                self.touch_theme();
                Vec::new()
            }
            (Mouse::Down, Hit::FilterRow(r)) => {
                self.filter_row = r;
                if double {
                    return self.handle(Input::Enter, ctx);
                }
                Vec::new()
            }
            (Mouse::Down, Hit::FilterBox(r)) => {
                self.filter_row = r;
                self.handle(Input::Enter, ctx)
            }
            (_, Hit::FilterSlider { row, x0, width }) => {
                self.filter_row = row;
                let frac = x.saturating_sub(x0) as f32 / width.saturating_sub(1).max(1) as f32;
                settings::filter_set_strength(row, ctx, frac).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    fn handle_help(&mut self, input: Input) {
        let page = self.page_len.get().max(1) as u16;
        let max = self.help_max.get();
        match input {
            Input::Esc | Input::Enter | Input::Char('?') => {
                self.help = false;
                self.help_scroll = 0;
            }
            Input::Up | Input::Char('k') => self.help_scroll = self.help_scroll.saturating_sub(1),
            Input::Down | Input::Char('j') => self.help_scroll = (self.help_scroll + 1).min(max),
            Input::PageUp => self.help_scroll = self.help_scroll.saturating_sub(page),
            Input::PageDown => self.help_scroll = self.help_scroll.saturating_add(page).min(max),
            Input::Home => self.help_scroll = 0,
            Input::End => self.help_scroll = max,
            _ => {}
        }
    }

    fn handle_browser(&mut self, input: Input, ctx: &MenuCtx) -> Vec<Effect> {
        let page = self.page_len.get().max(1) as isize;
        if self.browser.searching {
            match input {
                Input::Char(c) => {
                    self.browser.query.push(c);
                    self.browser.row = 0;
                    self.browser.column = Column::Scenes;
                    self.touch(ctx);
                }
                Input::Backspace => {
                    if self.browser.query.pop().is_none() {
                        self.browser.searching = false;
                    }
                    self.browser.row = 0;
                    self.touch(ctx);
                }
                Input::Esc => {
                    // clear the filter but keep your place: the hit stays
                    // highlighted, now on its own shelf
                    let hit = self.browser.highlighted(ctx);
                    self.browser.clear_search();
                    if let Some(e) = hit {
                        self.browser.reveal(e.name(), ctx);
                    }
                }
                Input::Enter => return self.switch(ctx),
                Input::Up => self.nav(-1, ctx),
                Input::Down => self.nav(1, ctx),
                Input::PageUp => self.nav(-page, ctx),
                Input::PageDown => self.nav(page, ctx),
                Input::Home => self.nav(isize::MIN / 2, ctx),
                Input::End => self.nav(isize::MAX / 2, ctx),
                _ => {}
            }
            return Vec::new();
        }
        let on_scenes = self.browser.column == Column::Scenes;
        match input {
            Input::Up | Input::Char('k') => self.nav(-1, ctx),
            Input::Down | Input::Char('j') => self.nav(1, ctx),
            Input::PageUp => self.nav(-page, ctx),
            Input::PageDown => self.nav(page, ctx),
            Input::Home => self.nav(isize::MIN / 2, ctx),
            Input::End => self.nav(isize::MAX / 2, ctx),
            Input::Left | Input::Char('h') => {
                self.browser.column = Column::Shelves;
                self.hover = None;
            }
            Input::Right | Input::Char('l') => {
                self.browser.column = Column::Scenes;
                self.touch(ctx);
            }
            Input::Enter if on_scenes => return self.switch(ctx),
            Input::Enter => {
                self.browser.column = Column::Scenes;
                self.touch(ctx);
            }
            Input::Char('/') => {
                self.browser.searching = true;
                self.browser.column = Column::Scenes;
            }
            Input::Char('f') if on_scenes => {
                if let Some(e) = self.browser.highlighted(ctx) {
                    return vec![Effect::ToggleFavorite(e.name())];
                }
            }
            Input::Char('t') if on_scenes => return self.cycle_theme(ctx),
            Input::Char('?') => self.help = true,
            Input::Esc => return self.close(),
            _ => {}
        }
        Vec::new()
    }

    fn nav(&mut self, delta: isize, ctx: &MenuCtx) {
        match self.browser.column {
            Column::Shelves => self.browser.move_shelf(delta),
            Column::Scenes => {
                self.browser.move_row(delta, ctx);
                self.touch(ctx);
            }
        }
    }

    /// Enter on a scene: switch to it (keeping it as-is if it is the
    /// preview) and close. Enter on the scene the menu opened over just
    /// closes, undoing any preview still showing.
    fn switch(&mut self, ctx: &MenuCtx) -> Vec<Effect> {
        let Some(e) = self.browser.highlighted(ctx) else {
            return Vec::new();
        };
        if e.name() == self.origin {
            return self.close();
        }
        // the preview (if any) is being kept or replaced: nothing to undo
        self.previewing = None;
        let mut fx = self.close();
        fx.push(Effect::SwitchScene(e.name()));
        fx
    }

    /// `t`: step the highlighted scene's theme forward.
    fn cycle_theme(&mut self, ctx: &MenuCtx) -> Vec<Effect> {
        let Some(e) = self.browser.highlighted(ctx) else {
            return Vec::new();
        };
        let themes = e.themes();
        if themes.len() < 2 {
            return Vec::new();
        }
        let cur = if e.name() == ctx.scene_name {
            ctx.theme.clone()
        } else {
            ctx.scene_themes.get(e.name()).cloned()
        };
        let i = cur
            .and_then(|t| themes.iter().position(|x| *x == t))
            .unwrap_or(0);
        vec![Effect::SetSceneTheme {
            scene: e.name(),
            theme: themes[(i + 1) % themes.len()].to_string(),
        }]
    }

    fn handle_settings(&mut self, input: Input, ctx: &MenuCtx) -> Vec<Effect> {
        let rows = settings::page(self.page);
        if rows.is_empty() {
            return Vec::new();
        }
        let n = rows.len();
        let pi = self.page.index();
        let r = self.row();
        let set = &rows[r];
        let step = |dir: i32| settings::step(set.id, ctx, dir).into_iter().collect();
        match input {
            Input::Up | Input::Char('k') => self.rows[pi] = (r + n - 1) % n,
            Input::Down | Input::Char('j') => self.rows[pi] = (r + 1) % n,
            Input::Home | Input::PageUp => self.rows[pi] = 0,
            Input::End | Input::PageDown => self.rows[pi] = n - 1,
            Input::Left | Input::Char('h') => return step(-1),
            Input::Right | Input::Char('l') | Input::Enter => {
                return match set.kind {
                    Kind::Open => self.activate(set.id),
                    Kind::Action if input == Input::Enter => {
                        settings::action(set.id, ctx).into_iter().collect()
                    }
                    Kind::Action | Kind::Info => Vec::new(),
                    Kind::Choice | Kind::Toggle | Kind::Slider => step(1),
                }
            }
            Input::Char('u') => return vec![Effect::Undo],
            Input::Char('?') => self.help = true,
            Input::Esc => return self.close(),
            _ => {}
        }
        Vec::new()
    }

    fn activate(&mut self, id: SettingId) -> Vec<Effect> {
        match id {
            SettingId::ColorGrade => vec![Effect::OpenColorGrade],
            SettingId::Palette => vec![Effect::OpenPalette],
            SettingId::Filters => {
                self.filters_open = true;
                self.filter_row = 0;
                Vec::new()
            }
            SettingId::Align => vec![Effect::OpenCalibration],
            SettingId::TerminalCheck => vec![Effect::OpenTerminalCheck],
            SettingId::NewGroup => {
                self.prompt = Some(themes::Prompt::new(themes::PromptKind::NewGroup, ""));
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_filters(&mut self, input: Input, ctx: &MenuCtx) -> Vec<Effect> {
        let n = settings::filter_rows();
        let r = self.filter_row.min(n - 1);
        match input {
            Input::Up | Input::Char('k') => self.filter_row = (r + n - 1) % n,
            Input::Down | Input::Char('j') => self.filter_row = (r + 1) % n,
            Input::Home | Input::PageUp => self.filter_row = 0,
            Input::End | Input::PageDown => self.filter_row = n - 1,
            Input::Left | Input::Char('h') => {
                return settings::filter_step(r, ctx, -1).into_iter().collect()
            }
            Input::Right | Input::Char('l') => {
                return settings::filter_step(r, ctx, 1).into_iter().collect()
            }
            // Enter flips an effect on or off; on the preset row it steps
            Input::Enter | Input::Char(' ') if r == 0 => {
                return settings::filter_step(r, ctx, 1).into_iter().collect()
            }
            Input::Enter | Input::Char(' ') => {
                return settings::filter_toggle(r, ctx).into_iter().collect()
            }
            Input::Char('u') => return vec![Effect::Undo],
            Input::Esc | Input::Backspace => self.filters_open = false,
            Input::Char('?') => self.help = true,
            _ => {}
        }
        Vec::new()
    }
}
