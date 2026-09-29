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
pub mod view;

use crate::config::CycleScope;
use crate::engine::Renderer;
use crate::render::Pixels;
use crate::scene::Detail;
use browser::{Browser, Column};
use settings::{Kind, SettingId};
use std::cell::Cell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long the browser highlight must rest on a scene before it previews.
pub const PREVIEW_DELAY: Duration = Duration::from_millis(300);

/// How long a footer notice ("coming soon") stays up.
pub const FLASH_FOR: Duration = Duration::from_secs(3);

/// FPS presets — menu ◂/▸ and `[`/`]` keys step through these (120 = high-refresh sweet spot).
pub const FPS_PRESETS: &[u32] = &[10, 24, 30, 60, 90, 120, 144, 165, 240];

/// Speed presets for the Playback page and `,`/`.` keys.
pub const SPEED_PRESETS: &[f32] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

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
    Look,
    Playback,
    Display,
    Wall,
}

impl Page {
    pub const ALL: [Page; 5] = [
        Page::Scenes,
        Page::Look,
        Page::Playback,
        Page::Display,
        Page::Wall,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Scenes => "Scenes",
            Page::Look => "Look",
            Page::Playback => "Playback",
            Page::Display => "Display",
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

/// A key, as the menu sees it.
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
}

impl Input {
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
    SetRenderer(Renderer),
    SetLink(bool),
    SetLinkGroup(String),
    /// automatic video wall across linked panes
    SetWall(bool),
    ToggleFilter(String),
    /// replace the whole filter stack (a preset)
    SetFilters(Vec<String>),
    /// open the colour wheel over the menu
    OpenColorGrade,
    /// monitor alignment tool — not wired yet; the host flashes a notice
    OpenCalibration,
}

/// Snapshot of the host's current settings, for display and adjustment.
#[derive(Default)]
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
    pub hue_shift: f32,
    pub saturation: f32,
    pub contrast: f32,
    pub renderer: Renderer,
    /// whether Studio scenes can render here; None = not known yet
    pub gpu: Option<bool>,
    pub link_enabled: bool,
    pub link_group: String,
    pub wall_enabled: bool,
    pub truecolor: bool,
    pub filters: Vec<String>,
    pub favorites: Vec<String>,
    /// newest first
    pub recents: Vec<String>,
    /// remembered per-scene themes (`[themes]`)
    pub scene_themes: HashMap<String, String>,
    /// (action, key) for every rebindable action
    pub key_display: Vec<(String, String)>,
    pub instances: Vec<String>,
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
    /// Look → Filters… sub-page is showing
    pub filters_open: bool,
    filter_row: usize,
    /// focused row on each settings page, by `Page::index`
    rows: [usize; 5],
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
            filters_open: false,
            filter_row: 0,
            rows: [0; 5],
            browser: Browser::new(),
            origin: "",
            previewing: None,
            hover: None,
            flash: None,
            page_len: Cell::new(10),
            help_max: Cell::new(u16::MAX),
        }
    }

    /// Open over the scene on screen, with the browser pointing at it. The
    /// last page used is kept.
    pub fn open(&mut self, ctx: &MenuCtx) {
        self.open = true;
        self.help = false;
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
        self.filters_open = false;
        self.browser.clear_search();
        self.end_preview()
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

    /// True while keystrokes are text (the scene search box), so the host
    /// must not treat them as shortcuts.
    pub fn typing(&self) -> bool {
        self.open && !self.help && self.page == Page::Scenes && self.browser.searching
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
            _ if self.filters_open => self.handle_filters(input, ctx),
            _ => self.handle_settings(input, ctx),
        }
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

    /// Restart the preview timer on whatever the browser now highlights.
    fn touch(&mut self, ctx: &MenuCtx) {
        self.hover = self
            .browser
            .highlighted(ctx)
            .map(|e| (e.name(), Instant::now()));
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
                    Kind::Info => Vec::new(),
                    Kind::Choice | Kind::Toggle => step(1),
                }
            }
            Input::Char('?') => self.help = true,
            Input::Esc => return self.close(),
            _ => {}
        }
        Vec::new()
    }

    fn activate(&mut self, id: SettingId) -> Vec<Effect> {
        match id {
            SettingId::ColorGrade => vec![Effect::OpenColorGrade],
            SettingId::Filters => {
                self.filters_open = true;
                self.filter_row = 0;
                Vec::new()
            }
            SettingId::Align => vec![Effect::OpenCalibration],
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
            Input::Right | Input::Char('l') | Input::Enter => {
                return settings::filter_step(r, ctx, 1).into_iter().collect()
            }
            Input::Esc | Input::Backspace => self.filters_open = false,
            Input::Char('?') => self.help = true,
            _ => {}
        }
        Vec::new()
    }
}
