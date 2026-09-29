//! The menu, rasterised to PNG for review without a terminal.
//!
//! ```text
//! cargo run --example ui_snapshot -- [COLSxROWS] [OUT_DIR]
//! ```
//!
//! Draws every menu state (each page, the Effects sub-page, search, help)
//! over a live Classic scene, the way the frame loop composes them, then
//! paints the cell buffer: text with a real monospace font (`TERMPAPER_FONT`,
//! else JetBrains Mono / DejaVu / Liberation from the usual places), block
//! elements, box drawing and braille geometrically, the way terminals draw
//! them. Writes `OUT_DIR/<state>.png` (default `target/ui_snapshot`).
use rand::{rngs::StdRng, SeedableRng};
use ratatui::{backend::TestBackend, buffer::Buffer, style::Color, style::Modifier, Terminal};
use std::collections::HashMap;
use std::path::PathBuf;
use termpaper::canvas::Canvas;
use termpaper::menu::{self, Input, Menu, MenuCtx, Page};
use termpaper::render::{self, Pixels};
use termpaper::scene::{self, Detail, SceneOptions};

const CW: usize = 10;
const CH: usize = 20;

fn xterm(i: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match i {
        0..=15 => BASE[i as usize],
        16..=231 => {
            let v = i - 16;
            let s = |c: u8| if c == 0 { 0 } else { 55 + c * 40 };
            (s(v / 36), s((v / 6) % 6), s(v % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            (g, g, g)
        }
    }
}

fn rgb(c: Color, default: (u8, u8, u8)) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => xterm(i),
        Color::Reset => default,
        Color::Black => xterm(0),
        Color::Red => xterm(1),
        Color::Green => xterm(2),
        Color::Yellow => xterm(3),
        Color::Blue => xterm(4),
        Color::Magenta => xterm(5),
        Color::Cyan => xterm(6),
        Color::Gray => xterm(7),
        Color::DarkGray => xterm(8),
        Color::LightRed => xterm(9),
        Color::LightGreen => xterm(10),
        Color::LightYellow => xterm(11),
        Color::LightBlue => xterm(12),
        Color::LightMagenta => xterm(13),
        Color::LightCyan => xterm(14),
        Color::White => xterm(15),
    }
}

struct Painter {
    w: usize,
    h: usize,
    px: Vec<(u8, u8, u8)>,
    regular: fontdue::Font,
    bold: Option<fontdue::Font>,
    /// symbol fonts for glyphs the monospace font lacks, as terminals fall
    /// back to them
    fallback: Vec<fontdue::Font>,
    cache: HashMap<(char, bool), (fontdue::Metrics, Vec<u8>)>,
}

impl Painter {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: (u8, u8, u8)) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                self.px[yy * self.w + xx] = c;
            }
        }
    }

    fn blend(&mut self, x: usize, y: usize, c: (u8, u8, u8), a: f32) {
        if x >= self.w || y >= self.h {
            return;
        }
        let p = &mut self.px[y * self.w + x];
        let m = |bg: u8, fg: u8| (bg as f32 + (fg as f32 - bg as f32) * a).round() as u8;
        *p = (m(p.0, c.0), m(p.1, c.1), m(p.2, c.2));
    }

    /// Glyphs terminals draw themselves: blocks, box lines, braille. True
    /// when drawn.
    fn geometric(&mut self, ch: char, x: usize, y: usize, c: (u8, u8, u8)) -> bool {
        let (hw, hh) = (CW / 2, CH / 2);
        let quad = |tl: bool, tr: bool, bl: bool, br: bool| [tl, tr, bl, br];
        let q = match ch {
            '▀' => Some(quad(true, true, false, false)),
            '▄' => Some(quad(false, false, true, true)),
            '█' => Some(quad(true, true, true, true)),
            '▌' => Some(quad(true, false, true, false)),
            '▐' => Some(quad(false, true, false, true)),
            '▘' => Some(quad(true, false, false, false)),
            '▝' => Some(quad(false, true, false, false)),
            '▖' => Some(quad(false, false, true, false)),
            '▗' => Some(quad(false, false, false, true)),
            '▚' => Some(quad(true, false, false, true)),
            '▞' => Some(quad(false, true, true, false)),
            '▛' => Some(quad(true, true, true, false)),
            '▜' => Some(quad(true, true, false, true)),
            '▙' => Some(quad(true, false, true, true)),
            '▟' => Some(quad(false, true, true, true)),
            _ => None,
        };
        if let Some([tl, tr, bl, br]) = q {
            for (on, dx, dy) in [(tl, 0, 0), (tr, hw, 0), (bl, 0, hh), (br, hw, hh)] {
                if on {
                    self.rect(x + dx, y + dy, hw, hh, c);
                }
            }
            return true;
        }
        let mid_y = y + CH / 2;
        let mid_x = x + CW / 2;
        match ch {
            '─' => self.rect(x, mid_y, CW, 1, c),
            '━' => self.rect(x, mid_y - 1, CW, 3, c),
            '│' => self.rect(mid_x, y, 1, CH, c),
            '╭' => {
                self.rect(mid_x, mid_y, CW - CW / 2, 1, c);
                self.rect(mid_x, mid_y, 1, CH - CH / 2, c);
            }
            '╮' => {
                self.rect(x, mid_y, CW / 2 + 1, 1, c);
                self.rect(mid_x, mid_y, 1, CH - CH / 2, c);
            }
            '╰' => {
                self.rect(mid_x, mid_y, CW - CW / 2, 1, c);
                self.rect(mid_x, y, 1, CH / 2 + 1, c);
            }
            '╯' => {
                self.rect(x, mid_y, CW / 2 + 1, 1, c);
                self.rect(mid_x, y, 1, CH / 2 + 1, c);
            }
            '\u{2800}'..='\u{28ff}' => {
                let bits = ch as u32 - 0x2800;
                // dots 1,2,3,7 left column, 4,5,6,8 right
                let at = [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (0, 3), (1, 3)];
                for (i, (cx, cy)) in at.iter().enumerate() {
                    if bits & (1 << i) != 0 {
                        self.rect(x + 2 + cx * (CW / 2), y + 2 + cy * (CH / 4), 2, 2, c);
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn glyph(&mut self, ch: char, bold: bool, x: usize, y: usize, c: (u8, u8, u8)) {
        if ch == ' ' || self.geometric(ch, x, y, c) {
            return;
        }
        let key = (ch, bold);
        if !self.cache.contains_key(&key) {
            let primary = if bold { self.bold.as_ref().unwrap_or(&self.regular) } else { &self.regular };
            let font = std::iter::once(primary)
                .chain(self.fallback.iter())
                .find(|f| f.lookup_glyph_index(ch) != 0)
                .unwrap_or(primary);
            let r = font.rasterize(ch, CH as f32 * 0.8);
            self.cache.insert(key, r);
        }
        let (m, bitmap) = self.cache[&key].clone();
        let baseline = y as i32 + (CH as f32 * 0.78) as i32;
        let gx = x as i32 + m.xmin;
        let gy = baseline - m.height as i32 - m.ymin;
        for row in 0..m.height {
            for col in 0..m.width {
                let a = bitmap[row * m.width + col] as f32 / 255.0;
                if a > 0.0 {
                    let (px, py) = (gx + col as i32, gy + row as i32);
                    if px >= 0 && py >= 0 {
                        self.blend(px as usize, py as usize, c, a);
                    }
                }
            }
        }
    }

    fn paint(&mut self, buf: &Buffer) {
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                let (px, py) = (x as usize * CW, y as usize * CH);
                let bg = rgb(cell.bg, (0, 0, 0));
                let fgc = rgb(cell.fg, (220, 220, 220));
                self.rect(px, py, CW, CH, bg);
                let bold = cell.modifier.contains(Modifier::BOLD);
                if let Some(ch) = cell.symbol().chars().next() {
                    self.glyph(ch, bold, px, py, fgc);
                }
                if cell.modifier.contains(Modifier::UNDERLINED) {
                    self.rect(px, py + CH - 3, CW, 1, fgc);
                }
            }
        }
    }

    fn save(&self, path: &PathBuf) {
        let file = std::fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        let data: Vec<u8> = self.px.iter().flat_map(|p| [p.0, p.1, p.2]).collect();
        w.write_image_data(&data).unwrap();
    }
}

fn load_font(bold: bool) -> Option<fontdue::Font> {
    let mut paths: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("TERMPAPER_FONT") {
        paths.push(p);
    }
    let style = if bold { "Bold" } else { "Regular" };
    for dir in ["/usr/share/fonts/TTF", "/usr/share/fonts/truetype/dejavu", "/usr/share/fonts/liberation"] {
        paths.push(format!("{dir}/JetBrainsMonoNerdFont-{style}.ttf"));
        paths.push(format!("{dir}/JetBrainsMono-{style}.ttf"));
        paths.push(format!("{dir}/DejaVuSansMono{}.ttf", if bold { "-Bold" } else { "" }));
        paths.push(format!("{dir}/LiberationMono-{style}.ttf"));
    }
    paths.iter().find_map(|p| {
        let bytes = std::fs::read(p).ok()?;
        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
    })
}

/// A plausible host snapshot for the menu.
fn ctx() -> MenuCtx {
    let mut look = termpaper::look::Look::default();
    look.grade.exposure = 0.3;
    look.grade.temperature = -0.25;
    look.grade.vibrance = 0.35;
    look.effects.stack = vec!["bloom".into(), "vignette".into()];
    look.effects.set_amount("vignette", 0.6);
    MenuCtx {
        renderer_status: "GPU shader · AMD Radeon RX 6700 XT · Vulkan".into(),
        wall_status: "wall: local".into(),
        scene_name: "koi",
        pixels: Pixels::Half,
        detail: Detail::Medium,
        theme: Some("teal".into()),
        speed: 1.0,
        fps: 120,
        smooth: 0.3,
        dim: 0.9,
        fade: 0.25,
        clock: true,
        look,
        link_enabled: true,
        link_group: "default".into(),
        wall_enabled: true,
        truecolor: true,
        gpu: Some(true),
        favorites: vec!["tokyo".into(), "bigsur".into(), "koi".into()],
        recents: vec!["koi".into(), "fire".into()],
        key_display: termpaper::config::ACTIONS
            .iter()
            .map(|a| (a.to_string(), termpaper::config::default_key(a).to_string()))
            .collect(),
        instances: vec!["pid 4242     koi          up 312s (you)".into()],
        ..Default::default()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (cols, rows) = args
        .get(1)
        .and_then(|s| s.split_once('x'))
        .and_then(|(a, b)| Some((a.parse::<u16>().ok()?, b.parse::<u16>().ok()?)))
        .unwrap_or((120, 36));
    let out = PathBuf::from(args.get(2).map(String::as_str).unwrap_or("target/ui_snapshot"));
    std::fs::create_dir_all(&out).unwrap();
    let regular = load_font(false).expect("no monospace font found; set TERMPAPER_FONT");
    let bold = load_font(true);
    let fallback: Vec<fontdue::Font> = [
        "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansSymbols-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]
    .iter()
    .filter_map(|p| fontdue::Font::from_bytes(std::fs::read(p).ok()?, fontdue::FontSettings::default()).ok())
    .collect();

    // the scene behind the menu, as the frame loop would draw it
    let opts = SceneOptions { theme: Some("teal".into()), detail: Detail::Medium, text_scale: None, pixels: Pixels::Half };
    let mut s = scene::create("koi", &opts, StdRng::seed_from_u64(3)).unwrap();
    let mut canvas = Canvas::new(cols as usize, rows as usize * 2);
    for _ in 0..240 {
        s.update(1.0 / 60.0, &mut canvas);
    }

    let c = ctx();
    let mut states: Vec<(String, Menu)> = Vec::new();
    for page in Page::ALL {
        let mut m = Menu::new();
        m.open(&c);
        m.goto(page);
        if page == Page::Look {
            // focus a slider so its bar shows as active
            for _ in 0..2 {
                m.handle(Input::Down, &c);
            }
        }
        states.push((format!("page_{}", page.title().to_lowercase()), m));
    }
    let mut m = Menu::new();
    m.open(&c);
    m.goto(Page::Look);
    let fx = menu::settings::LOOK.iter().position(|s| s.id == menu::settings::SettingId::Filters).unwrap();
    for _ in 0..fx {
        m.handle(Input::Down, &c);
    }
    m.handle(Input::Enter, &c);
    for _ in 0..13 {
        m.handle(Input::Down, &c);
    }
    states.push(("effects".into(), m));
    let mut m = Menu::new();
    m.open(&c);
    m.handle(Input::Char('/'), &c);
    for ch in "rain".chars() {
        m.handle(Input::Char(ch), &c);
    }
    states.push(("search".into(), m));
    let mut m = Menu::new();
    m.open(&c);
    m.handle(Input::Char('?'), &c);
    states.push(("help".into(), m));

    for (name, m) in &states {
        let mut term = Terminal::new(TestBackend::new(cols, rows)).unwrap();
        term.draw(|f| {
            render::draw(&canvas, f.area(), f.buffer_mut(), true, Pixels::Half);
            menu::view::render(f, f.area(), m, &c);
        })
        .unwrap();
        let mut p = Painter {
            w: cols as usize * CW,
            h: rows as usize * CH,
            px: vec![(0, 0, 0); cols as usize * CW * rows as usize * CH],
            regular: regular.clone(),
            bold: bold.clone(),
            fallback: fallback.clone(),
            cache: HashMap::new(),
        };
        p.paint(term.backend().buffer());
        let path = out.join(format!("{name}.png"));
        p.save(&path);
        println!("{}", path.display());
    }
}
