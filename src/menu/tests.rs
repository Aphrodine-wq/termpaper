use super::browser::{self, Column, Shelf};
use super::settings::{self, Kind, SettingId};
use super::*;
use crate::scene::{self, Category};
use ratatui::{backend::TestBackend, Terminal};

fn ctx() -> MenuCtx {
    MenuCtx {
        renderer_status: "CPU · test".into(),
        wall_status: "wall: local".into(),
        scene_name: "rain",
        pixels: Pixels::Half,
        detail: Detail::Medium,
        theme: Some("night".into()),
        speed: 1.0,
        fps: 60,
        smooth: 0.3,
        dim: 0.8,
        fade: 0.25,
        clock: true,
        look: {
            let mut l = crate::look::Look::default();
            l.effects.stack = vec!["scanlines".into()];
            l
        },
        link_enabled: true,
        link_group: "default".into(),
        wall_enabled: true,
        truecolor: true,
        key_display: vec![("quit".into(), "q".into()), ("next".into(), "right".into())],
        instances: vec!["pid 1234     rain         up 5s (you)".into()],
        themes: std::sync::Arc::new(themes::rows(&crate::theme::Store::load_from(None))),
        ..Default::default()
    }
}

/// `ctx()` with one theme of your own ("Mine", made on bigsur) in the list.
fn with_yours(c: &MenuCtx) -> MenuCtx {
    let mut rows = (*c.themes).clone();
    let mut mine = rows[1].clone();
    mine.slug = "mine".into();
    mine.name = "Mine".into();
    mine.shelf = "Yours";
    mine.yours = true;
    mine.scene = Some(("bigsur".into(), None));
    rows.insert(1, mine);
    MenuCtx { themes: std::sync::Arc::new(rows), ..c.clone() }
}

fn theme_at(c: &MenuCtx, row: usize) -> &themes::ThemeRow {
    &c.themes[row]
}

fn opened(c: &MenuCtx) -> Menu {
    let mut m = Menu::new();
    m.open(c);
    m
}

/// Feed keys, collecting every effect.
fn keys(m: &mut Menu, c: &MenuCtx, inputs: &[Input]) -> Vec<Effect> {
    inputs.iter().flat_map(|i| m.handle(*i, c)).collect()
}

fn type_text(m: &mut Menu, c: &MenuCtx, s: &str) -> Vec<Effect> {
    s.chars()
        .flat_map(|ch| m.handle(Input::Char(ch), c))
        .collect()
}

fn highlighted(m: &Menu, c: &MenuCtx) -> &'static str {
    m.browser.highlighted(c).map(|e| e.name()).unwrap_or("")
}

fn later() -> Instant {
    Instant::now() + PREVIEW_DELAY + Duration::from_millis(50)
}

// ── browser navigation ──────────────────────────────────────────────────

#[test]
fn opens_on_the_current_scene() {
    let c = ctx();
    let m = opened(&c);
    assert_eq!(m.browser.current_shelf(), Shelf::Group("nature"));
    assert_eq!(highlighted(&m, &c), "rain");
    assert_eq!(m.browser.column, Column::Scenes);
}

#[test]
fn category_and_scene_navigation() {
    let c = ctx();
    let mut m = opened(&c);
    let nature = browser::shelf_scenes(Shelf::Group("nature"), &c);
    let start = m.browser.row;
    keys(&mut m, &c, &[Input::Down]);
    assert_eq!(highlighted(&m, &c), nature[start + 1].name());
    keys(&mut m, &c, &[Input::Char('k')]);
    assert_eq!(highlighted(&m, &c), "rain", "k moves back up");

    // ← to the shelves, then down a shelf: the scene list follows
    keys(&mut m, &c, &[Input::Left]);
    assert_eq!(m.browser.column, Column::Shelves);
    let shelves = browser::shelves();
    let before = m.browser.shelf;
    keys(&mut m, &c, &[Input::Char('j')]);
    assert_eq!(m.browser.shelf, before + 1);
    let shelf = shelves[before + 1];
    assert_eq!(m.browser.current_shelf(), shelf);
    assert_eq!(m.browser.row, 0, "a new shelf starts at its first scene");

    // → (or Enter) back into the scenes of that shelf
    keys(&mut m, &c, &[Input::Right]);
    assert_eq!(m.browser.column, Column::Scenes);
    assert_eq!(
        highlighted(&m, &c),
        browser::shelf_scenes(shelf, &c)[0].name()
    );

    // Home/End clamp to the ends of the list
    keys(&mut m, &c, &[Input::End]);
    let list = m.browser.list(&c);
    assert_eq!(m.browser.row, list.len() - 1);
    keys(&mut m, &c, &[Input::Down]);
    assert_eq!(m.browser.row, list.len() - 1, "no wrap past the end");
    keys(&mut m, &c, &[Input::Home, Input::Up]);
    assert_eq!(m.browser.row, 0);
}

#[test]
fn shelves_cover_every_scene() {
    let c = ctx();
    let shelves = browser::shelves();
    assert_eq!(shelves[0], Shelf::Favorites);
    assert_eq!(shelves[1], Shelf::Recent);
    // every category with scenes is a shelf, and every scene is on its
    // category shelf; Classic's sub-groups partition the Classic scenes
    let mut in_groups = 0;
    for s in &shelves {
        if let Shelf::Group(_) = s {
            in_groups += browser::shelf_scenes(*s, &c).len();
        }
    }
    let classic = scene::entries()
        .filter(|e| e.category() == Category::Classic)
        .count();
    assert_eq!(in_groups, classic);
    let on_categories: usize = shelves
        .iter()
        .filter(|s| matches!(s, Shelf::Category(_)))
        .map(|s| browser::shelf_scenes(*s, &c).len())
        .sum();
    assert_eq!(on_categories, scene::entries().count());
    // bigsur is a Studio coast scene
    let coast = browser::shelf_scenes(Shelf::Category(Category::Coast), &c);
    assert!(coast.iter().any(|e| e.name() == "bigsur"));
}

// ── search ──────────────────────────────────────────────────────────────

#[test]
fn search_matches_name_title_desc_and_tags() {
    let names = |q: &str| {
        browser::search(q)
            .iter()
            .map(|e| e.name())
            .collect::<Vec<_>>()
    };
    assert_eq!(names("koi").first(), Some(&"koi"), "name hit first");
    assert!(
        names("GLASS").contains(&"rain"),
        "description, case-insensitive"
    );
    assert!(names("demoscene").contains(&"plasma"), "tags / group");
    assert!(names("big sur").contains(&"bigsur"), "title words");
    assert!(names("cliffs").contains(&"bigsur"), "studio tags");
    assert!(names("zzzz-no-such-thing").is_empty());
    // every word must match
    let both = names("night ocean");
    assert!(both.contains(&"ocean"));
    assert!(!both.contains(&"rain"));
    // name/title prefix hits sort before description hits
    let fire = names("fire");
    assert_eq!(fire[0], "fire");
    assert!(fire.contains(&"campfire"));
    assert_eq!(browser::search("   ").len(), scene::entries().count());
}

#[test]
fn search_filters_then_esc_clears_and_keeps_place() {
    let c = ctx();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Char('/')]);
    assert!(m.typing());
    // typing `f`, `t`, `j`, `?` is text, not shortcuts
    assert!(type_text(&mut m, &c, "koi").is_empty());
    assert!(m.browser.filtering());
    assert_eq!(highlighted(&m, &c), "koi");
    assert!(!m.help);
    // Backspace edits
    keys(&mut m, &c, &[Input::Backspace]);
    assert_eq!(m.browser.query, "ko");
    type_text(&mut m, &c, "i");
    // Esc clears the filter; koi stays highlighted, on its own shelf
    keys(&mut m, &c, &[Input::Esc]);
    assert!(m.open, "first Esc only clears the search");
    assert!(!m.browser.filtering() && !m.typing());
    assert_eq!(m.browser.current_shelf(), Shelf::Group("water"));
    assert_eq!(highlighted(&m, &c), "koi");
    // a second Esc closes
    keys(&mut m, &c, &[Input::Esc]);
    assert!(!m.open);
}

#[test]
fn search_enter_switches_to_the_hit() {
    let c = ctx();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Char('/')]);
    type_text(&mut m, &c, "tunnel");
    let fx = keys(&mut m, &c, &[Input::Enter]);
    assert_eq!(fx, vec![Effect::SwitchScene("tunnel")]);
    assert!(!m.open);
}

// ── favourites and recents ──────────────────────────────────────────────

#[test]
fn favourite_toggling_and_shelf() {
    let mut c = ctx();
    let mut m = opened(&c);
    let fx = keys(&mut m, &c, &[Input::Char('f')]);
    assert_eq!(fx, vec![Effect::ToggleFavorite("rain")]);
    // the host applies it; the Favorites shelf then lists it
    assert!(browser::toggle_favorite(&mut c.favorites, "rain"));
    assert!(browser::toggle_favorite(&mut c.favorites, "koi"));
    let favs = browser::shelf_scenes(Shelf::Favorites, &c);
    assert_eq!(
        favs.iter().map(|e| e.name()).collect::<Vec<_>>(),
        vec!["rain", "koi"]
    );
    // toggling again removes; names that vanished from the catalog are skipped
    assert!(!browser::toggle_favorite(&mut c.favorites, "rain"));
    c.favorites.push("deleted-scene".into());
    let favs = browser::shelf_scenes(Shelf::Favorites, &c);
    assert_eq!(
        favs.iter().map(|e| e.name()).collect::<Vec<_>>(),
        vec!["koi"]
    );
    // `f` is not a shortcut on the shelf column
    keys(&mut m, &c, &[Input::Left]);
    assert!(keys(&mut m, &c, &[Input::Char('f')]).is_empty());
}

#[test]
fn favourites_persist_round_trip() {
    let mut cfg = crate::config::Config::default();
    browser::toggle_favorite(&mut cfg.favorites, "bigsur");
    browser::toggle_favorite(&mut cfg.favorites, "koi");
    browser::push_recent(&mut cfg.recents, "fire");
    let text = toml::to_string_pretty(&cfg).unwrap();
    let back: crate::config::Config = toml::from_str(&text).unwrap();
    assert_eq!(back.favorites, vec!["bigsur", "koi"]);
    assert_eq!(back.recents, vec!["fire"]);
    // and the browser reads them back
    let c = MenuCtx {
        favorites: back.favorites,
        recents: back.recents,
        ..ctx()
    };
    assert_eq!(browser::shelf_scenes(Shelf::Favorites, &c).len(), 2);
    assert_eq!(browser::shelf_scenes(Shelf::Recent, &c)[0].name(), "fire");
}

#[test]
fn recents_are_newest_first_unique_and_capped() {
    let mut r = Vec::new();
    for n in ["a", "b", "c", "b"] {
        browser::push_recent(&mut r, n);
    }
    assert_eq!(r, vec!["b", "c", "a"]);
    for i in 0..20 {
        browser::push_recent(&mut r, &format!("s{i}"));
    }
    assert_eq!(r.len(), browser::RECENTS);
    assert_eq!(r[0], "s19");
}

#[test]
fn unfavouriting_under_the_cursor_clamps() {
    let mut c = ctx();
    c.favorites = vec!["koi".into(), "fire".into()];
    let mut m = opened(&c);
    m.browser.shelf = 0; // Favorites
    m.browser.row = 1;
    assert_eq!(highlighted(&m, &c), "fire");
    c.favorites.pop();
    assert_eq!(
        highlighted(&m, &c),
        "koi",
        "row clamps to the shrunken list"
    );
}

#[test]
fn t_cycles_the_highlighted_scenes_theme() {
    let mut c = ctx();
    let mut m = opened(&c);
    // on screen: steps from the live theme
    let fx = keys(&mut m, &c, &[Input::Char('t')]);
    assert_eq!(
        fx,
        vec![Effect::SetSceneTheme {
            scene: "rain",
            theme: "storm".into()
        }]
    );
    // off screen: steps from its remembered theme
    keys(&mut m, &c, &[Input::Down]);
    let other = highlighted(&m, &c);
    let themes = scene::themes(other);
    c.scene_themes.insert(other.into(), themes[1].into());
    let fx = keys(&mut m, &c, &[Input::Char('t')]);
    assert_eq!(
        fx,
        vec![Effect::SetSceneTheme {
            scene: other,
            theme: themes[2 % themes.len()].into()
        }]
    );
}

// ── live preview ────────────────────────────────────────────────────────

#[test]
fn preview_then_esc_restores_the_original() {
    let c = ctx();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Down]);
    let target = highlighted(&m, &c);
    assert!(m.tick(Instant::now()).is_empty(), "not before the delay");
    assert_eq!(m.tick(later()), vec![Effect::Preview(target)]);
    assert!(m.tick(later()).is_empty(), "fires once");
    assert_eq!(m.previewing(), Some(target));
    let fx = keys(&mut m, &c, &[Input::Esc]);
    assert_eq!(fx, vec![Effect::EndPreview]);
    assert!(!m.open);
    assert_eq!(m.previewing(), None);
}

#[test]
fn preview_then_enter_keeps_it() {
    let c = ctx();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Down]);
    let target = highlighted(&m, &c);
    m.tick(later());
    // the host shows the preview; ctx now says it is on screen
    let shown = MenuCtx {
        scene_name: target,
        ..ctx()
    };
    let fx = keys(&mut m, &shown, &[Input::Enter]);
    assert_eq!(
        fx,
        vec![Effect::SwitchScene(target)],
        "kept, never restored"
    );
    assert!(!m.open);
}

#[test]
fn returning_to_the_origin_or_leaving_the_page_ends_the_preview() {
    let c = ctx();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Down]);
    m.tick(later());
    keys(&mut m, &c, &[Input::Up]);
    assert_eq!(m.tick(later()), vec![Effect::EndPreview]);

    keys(&mut m, &c, &[Input::Down]);
    m.tick(later());
    assert!(m.previewing().is_some());
    let fx = keys(&mut m, &c, &[Input::Tab]);
    assert_eq!(fx, vec![Effect::EndPreview]);
    assert_eq!(m.page, Page::Themes);
    assert!(m.open);
    // no scene previews from another page
    assert!(m.tick(later()).is_empty());
}

#[test]
fn browsing_shelves_does_not_preview() {
    let c = ctx();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Left, Input::Down, Input::Down]);
    assert!(m.tick(later()).is_empty());
}

#[test]
fn enter_on_the_running_scene_just_closes() {
    let c = ctx();
    let mut m = opened(&c);
    assert!(keys(&mut m, &c, &[Input::Enter]).is_empty());
    assert!(!m.open);
}

// ── themes page ─────────────────────────────────────────────────────────

fn on_themes(c: &MenuCtx) -> Menu {
    let mut m = opened(c);
    m.goto(Page::Themes);
    m
}

#[test]
fn resting_on_a_theme_previews_it_and_enter_applies() {
    let c = ctx();
    let mut m = on_themes(&c);
    // nothing until the highlight has rested
    keys(&mut m, &c, &[Input::Down]);
    assert!(m.tick_with(Instant::now(), &c).is_empty());
    assert_eq!(m.tick_with(later(), &c), vec![Effect::PreviewLook(theme_at(&c, 1).look.clone())]);
    assert!(m.look_previewing());
    // once is enough
    assert!(m.tick_with(later(), &c).is_empty());
    assert_eq!(keys(&mut m, &c, &[Input::Enter]), vec![Effect::ApplyTheme(theme_at(&c, 1).slug.clone())]);
    assert!(!m.look_previewing(), "the preview became the real thing");
    assert!(m.open, "the list stays open for another go");
}

#[test]
fn leaving_the_page_or_closing_ends_the_preview() {
    let c = ctx();
    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Down]);
    m.tick_with(later(), &c);
    assert_eq!(keys(&mut m, &c, &[Input::Tab]), vec![Effect::EndLookPreview]);
    assert_eq!(m.page, Page::Look);

    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Down]);
    m.tick_with(later(), &c);
    assert_eq!(keys(&mut m, &c, &[Input::Esc]), vec![Effect::EndLookPreview]);
    assert!(!m.open);
    // no preview, nothing to end
    let mut m = on_themes(&c);
    assert!(keys(&mut m, &c, &[Input::Esc]).is_empty());
}

#[test]
fn the_list_opens_on_the_theme_in_use_which_shows_your_look() {
    let active = ctx().themes[3].slug.clone();
    let c = MenuCtx { active_theme: Some(active.clone()), theme_modified: true, ..ctx() };
    let mut m = on_themes(&c);
    assert_eq!(m.highlighted_theme(&c).unwrap().slug, active);
    keys(&mut m, &c, &[Input::Down]);
    assert!(matches!(m.tick_with(later(), &c)[..], [Effect::PreviewLook(_)]));
    // back on the theme in use: your look (edits and all), not its file
    keys(&mut m, &c, &[Input::Up]);
    assert_eq!(m.tick_with(later(), &c), vec![Effect::EndLookPreview]);
}

#[test]
fn theme_search_filters_and_enter_applies() {
    let c = ctx();
    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Char('/')]);
    assert!(m.typing());
    type_text(&mut m, &c, "gruv");
    assert_eq!(m.highlighted_theme(&c).unwrap().slug, "gruvbox");
    // keys that mean something elsewhere are text while searching
    assert!(type_text(&mut m, &c, "x").is_empty());
    keys(&mut m, &c, &[Input::Backspace]);
    assert_eq!(keys(&mut m, &c, &[Input::Enter]), vec![Effect::ApplyTheme("gruvbox".into())]);
    assert!(!m.typing());
    // the whole list again, still on it
    assert!(m.theme_query.is_empty());
    assert_eq!(m.highlighted_theme(&c).unwrap().slug, "gruvbox");
    // a paste lands in the search
    keys(&mut m, &c, &[Input::Char('/')]);
    m.paste("tokyo\n");
    assert_eq!(m.highlighted_theme(&c).unwrap().slug, "tokyo-night");
}

#[test]
fn new_theme_names_the_look_you_have() {
    let c = ctx();
    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Down]);
    m.tick_with(later(), &c);
    // n puts your look back on screen while you name it
    assert_eq!(keys(&mut m, &c, &[Input::Char('n')]), vec![Effect::EndLookPreview]);
    assert!(m.typing());
    type_text(&mut m, &c, "Late night");
    keys(&mut m, &c, &[Input::Backspace]);
    assert_eq!(keys(&mut m, &c, &[Input::Enter]), vec![Effect::SaveTheme("Late nigh".into())]);
    // an empty name or Esc saves nothing
    keys(&mut m, &c, &[Input::Char('n')]);
    type_text(&mut m, &c, "   ");
    assert!(keys(&mut m, &c, &[Input::Enter]).is_empty());
    keys(&mut m, &c, &[Input::Char('n')]);
    type_text(&mut m, &c, "abc");
    assert!(keys(&mut m, &c, &[Input::Esc]).is_empty());
    assert!(m.open && m.prompt.is_none());
}

#[test]
fn import_takes_a_pasted_code() {
    let c = ctx();
    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Char('i')]);
    m.paste("tp1:abc\r\n");
    assert_eq!(keys(&mut m, &c, &[Input::Enter]), vec![Effect::ImportTheme("tp1:abc".into())]);
    // share codes are long: the prompt takes all of one
    keys(&mut m, &c, &[Input::Char('i')]);
    let code = format!("tp1:{}", "A".repeat(900));
    m.paste(&code);
    assert_eq!(keys(&mut m, &c, &[Input::Enter]), vec![Effect::ImportTheme(code)]);
}

#[test]
fn your_themes_rename_and_delete_but_built_ins_do_not() {
    let c = with_yours(&ctx());
    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Down]);
    assert_eq!(m.highlighted_theme(&c).unwrap().slug, "mine");
    // rename starts from the old name
    keys(&mut m, &c, &[Input::Char('r')]);
    assert_eq!(m.prompt.as_ref().unwrap().text, "Mine");
    type_text(&mut m, &c, " too");
    assert_eq!(
        keys(&mut m, &c, &[Input::Enter]),
        vec![Effect::RenameTheme { slug: "mine".into(), name: "Mine too".into() }]
    );
    // delete asks first
    keys(&mut m, &c, &[Input::Char('x')]);
    type_text(&mut m, &c, "n");
    assert!(keys(&mut m, &c, &[Input::Enter]).is_empty());
    keys(&mut m, &c, &[Input::Char('x')]);
    type_text(&mut m, &c, "y");
    assert_eq!(keys(&mut m, &c, &[Input::Enter]), vec![Effect::DeleteTheme("mine".into())]);
    // built-ins: a notice, no prompt
    keys(&mut m, &c, &[Input::Home]);
    for k in ['r', 'x', 'U'] {
        assert!(keys(&mut m, &c, &[Input::Char(k)]).is_empty());
        assert!(m.prompt.is_none(), "{k}");
        assert!(m.flash_text(Instant::now()).is_some(), "{k} explains itself");
    }
}

#[test]
fn update_saves_edits_into_your_theme_in_use_only() {
    let base = with_yours(&ctx());
    let mut m = on_themes(&base);
    keys(&mut m, &base, &[Input::Down]);
    // not in use
    assert!(keys(&mut m, &base, &[Input::Char('U')]).is_empty());
    // in use, unchanged
    let c = MenuCtx { active_theme: Some("mine".into()), ..base.clone() };
    assert!(keys(&mut m, &c, &[Input::Char('U')]).is_empty());
    assert_eq!(m.flash_text(Instant::now()), Some("No changes to save"));
    // in use and edited
    let c = MenuCtx { theme_modified: true, ..c };
    assert_eq!(keys(&mut m, &c, &[Input::Char('U')]), vec![Effect::UpdateTheme("mine".into())]);
}

#[test]
fn scene_edit_and_share_keys() {
    let c = with_yours(&ctx());
    let mut m = on_themes(&c);
    // Clean has no scene: s does nothing
    assert!(keys(&mut m, &c, &[Input::Char('s')]).is_empty());
    keys(&mut m, &c, &[Input::Down]);
    assert_eq!(keys(&mut m, &c, &[Input::Char('s')]), vec![Effect::SceneFromTheme("mine".into())]);
    assert_eq!(keys(&mut m, &c, &[Input::Char('c')]), vec![Effect::ShareTheme("mine".into())]);
    // e applies it (when it is not already in use) and opens the studio
    assert_eq!(
        keys(&mut m, &c, &[Input::Char('e')]),
        vec![Effect::ApplyTheme("mine".into()), Effect::OpenColorGrade]
    );
    let c = MenuCtx { active_theme: Some("mine".into()), ..c };
    assert_eq!(keys(&mut m, &c, &[Input::Char('e')]), vec![Effect::OpenColorGrade]);
    assert_eq!(keys(&mut m, &c, &[Input::Char('u')]), vec![Effect::Undo]);
}

#[test]
fn the_highlight_follows_a_theme_the_list_gained() {
    let c = ctx();
    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Down, Input::Down]);
    let was = m.highlighted_theme(&c).unwrap().slug.clone();
    // one of yours is saved: it lands near the top, above the highlight
    let grown = with_yours(&c);
    m.focus_theme("mine", &grown.themes);
    assert_eq!(m.highlighted_theme(&grown).unwrap().slug, "mine");
    m.focus_theme(&was, &grown.themes);
    assert_eq!(m.highlighted_theme(&grown).unwrap().slug, was);
    // a search that would hide it is dropped
    keys(&mut m, &grown, &[Input::Char('/')]);
    type_text(&mut m, &grown, "nord");
    m.focus_theme("mine", &grown.themes);
    assert!(m.theme_query.is_empty() && !m.typing());
    assert_eq!(m.highlighted_theme(&grown).unwrap().slug, "mine");
}

#[test]
fn notices_show_in_the_footer_on_the_themes_page() {
    let c = ctx();
    let mut m = on_themes(&c);
    keys(&mut m, &c, &[Input::Char('x')]);
    let s = screen(100, 30, &m, &c);
    assert!(s.contains("Built-in themes cannot be deleted"), "{s}");
}

#[test]
fn theme_keys_in_the_footer_match_the_row() {
    let c = with_yours(&ctx());
    let mut m = on_themes(&c);
    let hints = |m: &Menu, c: &MenuCtx| view::key_hints(m, c).into_iter().map(|(k, _)| k).collect::<Vec<_>>();
    let built_in = hints(&m, &c);
    assert!(!built_in.contains(&"r") && !built_in.contains(&"U") && !built_in.contains(&"s"));
    keys(&mut m, &c, &[Input::Down]);
    let yours = hints(&m, &c);
    assert!(yours.contains(&"r") && yours.contains(&"x") && yours.contains(&"s"));
    assert!(!yours.contains(&"U"), "nothing to save");
    let edited = MenuCtx { active_theme: Some("mine".into()), theme_modified: true, ..c.clone() };
    assert!(hints(&m, &edited).contains(&"U"));
    assert!(view::key_hints(&m, &edited).contains(&("Enter", "revert")));
}

// ── settings pages ──────────────────────────────────────────────────────

/// A context with every value mid-range, so both directions move.
fn mid() -> MenuCtx {
    MenuCtx {
        scene_name: "nexus",
        theme: Some("amber".into()),
        pixels: Pixels::Quad,
        detail: Detail::Medium,
        text_scale: Some(2),
        speed: 1.5,
        fps: 90,
        smooth: 0.4,
        dim: 0.6,
        fade: 0.5,
        cycle: Some(300.0),
        cycle_scope: CycleScope::Category,
        renderer: Renderer::Gpu,
        link_group: "wallpaper".into(),
        active_theme: Some("nord".into()),
        // every look slider mid-range, so it can move both ways
        look: {
            let mut l = crate::look::Look::default();
            l.grade.exposure = 0.5;
            l.grade.contrast = 1.2;
            l.grade.saturation = 1.2;
            l.grade.vibrance = 0.2;
            l.grade.temperature = -0.2;
            l.grade.tint = 0.1;
            l.grade.hue = 90.0;
            l.grade.fade = 0.1;
            l.palette.mode = crate::look::PaletteMode::Map;
            l.palette.colors = settings::DEFAULT_PALETTE.to_vec();
            l.palette.strength = 0.5;
            l.effects.stack = vec!["bloom".into(), "vignette".into(), "grain".into(), "letterbox".into()];
            for f in ["bloom", "vignette", "grain", "letterbox"] {
                l.effects.set_amount(f, 0.8);
            }
            l
        },
        ..ctx()
    }
}

/// Focus a settings row by id (keys only: ↓ until it is there).
fn focus(m: &mut Menu, c: &MenuCtx, id: SettingId) {
    let rows = settings::page(m.page);
    let target = rows.iter().position(|s| s.id == id).expect("row on this page");
    keys(m, c, &[Input::Home]);
    for _ in 0..target {
        keys(m, c, &[Input::Down]);
    }
    assert_eq!(rows[m.row()].id, id);
}

#[test]
fn every_setting_steps_both_ways() {
    let c = mid();
    for page in Page::ALL {
        for s in settings::page(page) {
            let back = settings::step(s.id, &c, -1);
            let fwd = settings::step(s.id, &c, 1);
            match s.kind {
                Kind::Choice | Kind::Slider => {
                    assert!(
                        back.is_some() && fwd.is_some(),
                        "{:?} must step both ways",
                        s.id
                    );
                    assert_ne!(back, fwd, "{:?}: ← and → should differ", s.id);
                }
                Kind::Toggle => {
                    assert!(back.is_some(), "{:?}", s.id);
                    assert_eq!(back, fwd, "{:?}: a toggle flips either way", s.id);
                }
                Kind::Open | Kind::Info | Kind::Action => {
                    assert!(back.is_none() && fwd.is_none(), "{:?} has no value", s.id);
                }
            }
            // every row with a value shows something readable, and every
            // slider knows where its bar is
            if !matches!(s.kind, Kind::Open | Kind::Action) {
                assert!(!settings::value(s.id, &c).is_empty(), "{:?}", s.id);
            }
            if s.kind == Kind::Slider {
                let (f, n) = settings::fraction(s.id, &c).expect("slider fraction");
                assert!((0.0..=1.0).contains(&f) && (0.0..=1.0).contains(&n), "{:?}", s.id);
            }
        }
    }
}

#[test]
fn ordered_values_clamp_and_choices_wrap() {
    let step = |id, c: &MenuCtx, d| settings::step(id, c, d);
    // clamped ends are no-ops
    let top = MenuCtx {
        dim: 1.0,
        fps: 240,
        speed: 4.0,
        detail: Detail::High,
        cycle: Some(1800.0),
        ..mid()
    };
    assert_eq!(step(SettingId::Dim, &top, 1), None);
    assert_eq!(step(SettingId::Dim, &top, -1), Some(Effect::SetDim(0.9)));
    assert_eq!(step(SettingId::Fps, &top, 1), None);
    assert_eq!(step(SettingId::Speed, &top, 1), None);
    assert_eq!(step(SettingId::Quality, &top, 1), None);
    assert_eq!(step(SettingId::Cycle, &top, 1), None);
    let bottom = MenuCtx {
        dim: 0.2,
        fps: 10,
        smooth: 0.0,
        fade: 0.1,
        detail: Detail::Low,
        cycle: None,
        ..mid()
    };
    assert_eq!(step(SettingId::Dim, &bottom, -1), None);
    assert_eq!(step(SettingId::Fps, &bottom, -1), None);
    assert_eq!(step(SettingId::Smooth, &bottom, -1), None);
    assert_eq!(step(SettingId::Fade, &bottom, -1), None);
    assert_eq!(step(SettingId::Quality, &bottom, -1), None);
    assert_eq!(step(SettingId::Cycle, &bottom, -1), None);
    assert_eq!(
        step(SettingId::Cycle, &bottom, 1),
        Some(Effect::SetCycle(Some(60.0)))
    );
    // no float drift from repeated steps
    assert_eq!(
        step(SettingId::Fade, &bottom, 1),
        Some(Effect::SetFade(0.15))
    );
    // a custom --cycle value steps to its neighbouring presets
    let custom = MenuCtx {
        cycle: Some(45.0),
        ..mid()
    };
    assert_eq!(
        step(SettingId::Cycle, &custom, 1),
        Some(Effect::SetCycle(Some(60.0)))
    );
    assert_eq!(
        step(SettingId::Cycle, &custom, -1),
        Some(Effect::SetCycle(None))
    );

    // unordered choices wrap both ways
    let first = MenuCtx {
        pixels: Pixels::Half,
        theme: Some("cyan".into()),
        renderer: Renderer::Auto,
        link_group: "default".into(),
        text_scale: None,
        cycle_scope: CycleScope::All,
        ..mid()
    };
    assert_eq!(
        step(SettingId::Pixels, &first, -1),
        Some(Effect::SetPixels(Pixels::Braille))
    );
    assert_eq!(
        step(SettingId::Variant, &first, -1),
        Some(Effect::SetTheme(Some("mono".into())))
    );
    assert_eq!(
        step(SettingId::Renderer, &first, -1),
        Some(Effect::SetRenderer(Renderer::Shader))
    );
    assert_eq!(
        step(SettingId::Group, &first, -1),
        Some(Effect::SetLinkGroup("art".into()))
    );
    assert_eq!(
        step(SettingId::TextScale, &first, -1),
        Some(Effect::SetTextScale(Some(3)))
    );
    assert_eq!(
        step(SettingId::CycleScope, &first, -1),
        Some(Effect::SetCycleScope(CycleScope::Favorites))
    );
    // Group does nothing while Link is off
    let solo = MenuCtx {
        link_enabled: false,
        ..mid()
    };
    assert_eq!(step(SettingId::Group, &solo, 1), None);
    assert!(!settings::enabled(SettingId::Group, &solo));
}

#[test]
fn settings_navigation_and_actions() {
    let c = mid();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Tab, Input::Tab]);
    assert_eq!(m.page, Page::Look);
    // ↑ from the first row wraps within the page instead of leaving it
    keys(&mut m, &c, &[Input::Up]);
    assert_eq!(m.row(), settings::LOOK.len() - 1);
    keys(&mut m, &c, &[Input::Home]);
    assert_eq!(settings::LOOK[m.row()].id, SettingId::Theme);
    // the Theme row steps through the Themes page's list
    let nord = c.themes.iter().position(|t| t.slug == "nord").unwrap();
    assert_eq!(
        keys(&mut m, &c, &[Input::Right]),
        vec![Effect::ApplyTheme(c.themes[nord + 1].slug.clone())]
    );
    assert_eq!(settings::value(SettingId::Theme, &c), "Nord");
    assert_eq!(
        settings::value(SettingId::Theme, &MenuCtx { theme_modified: true, ..mid() }),
        "Nord · edited"
    );
    assert_eq!(settings::value(SettingId::Theme, &MenuCtx { active_theme: None, ..mid() }), "none");
    keys(&mut m, &c, &[Input::Down]);
    assert_eq!(settings::LOOK[m.row()].id, SettingId::Variant);
    assert_eq!(
        keys(&mut m, &c, &[Input::Right]),
        vec![Effect::SetTheme(Some("violet".into()))]
    );
    // Colour studio… opens the wheel
    keys(&mut m, &c, &[Input::Down]);
    assert_eq!(
        keys(&mut m, &c, &[Input::Enter]),
        vec![Effect::OpenColorGrade]
    );
    // a slider steps and u asks the host to undo
    focus(&mut m, &c, SettingId::Exposure);
    let mut brighter = c.look.clone();
    brighter.grade.exposure = 0.6;
    assert_eq!(keys(&mut m, &c, &[Input::Right]), vec![Effect::SetLook(brighter)]);
    assert_eq!(keys(&mut m, &c, &[Input::Char('u')]), vec![Effect::Undo]);
    // Reset look only answers Enter
    focus(&mut m, &c, SettingId::ResetLook);
    assert!(keys(&mut m, &c, &[Input::Right]).is_empty());
    assert_eq!(
        keys(&mut m, &c, &[Input::Enter]),
        vec![Effect::SetLook(crate::look::Look::default())]
    );
    // Effects… opens the sub-page; Esc comes back without closing
    focus(&mut m, &c, SettingId::Filters);
    keys(&mut m, &c, &[Input::Enter]);
    assert!(m.filters_open);
    keys(&mut m, &c, &[Input::Esc]);
    assert!(!m.filters_open && m.open);
    // Shift-Tab wraps round to the last page; Wall's Align emits its placeholder
    keys(&mut m, &c, &[Input::BackTab, Input::BackTab, Input::BackTab]);
    assert_eq!(m.page, Page::Wall);
    keys(
        &mut m,
        &c,
        &[Input::Home, Input::Down, Input::Down, Input::Down],
    );
    assert_eq!(settings::WALL[m.row()].id, SettingId::Align);
    assert_eq!(
        keys(&mut m, &c, &[Input::Enter]),
        vec![Effect::OpenCalibration]
    );
    // the page's focused row is remembered across tab switches
    keys(&mut m, &c, &[Input::Tab, Input::BackTab]);
    assert_eq!(settings::WALL[m.row()].id, SettingId::Align);
}

#[test]
fn filters_sub_page_toggles_and_presets() {
    let c = mid();
    let mut m = opened(&c);
    m.goto(Page::Look);
    focus(&mut m, &c, SettingId::Filters);
    keys(&mut m, &c, &[Input::Enter]);
    assert!(m.filters_open);
    let with_effects = |base: &MenuCtx, e: crate::look::Effects| {
        let mut l = base.look.clone();
        l.effects = e;
        Effect::SetLook(l)
    };
    // row 0: presets; a custom stack → goes to Clean
    assert_eq!(
        keys(&mut m, &c, &[Input::Right]),
        vec![with_effects(&c, settings::preset_effects(0))]
    );
    // Film (as the preset stores it) → CRT
    let film = MenuCtx {
        look: {
            let mut l = crate::look::Look::default();
            l.effects = settings::preset_effects(1);
            l
        },
        ..mid()
    };
    assert_eq!(settings::preset_of(&film.look.effects), Some(1));
    assert_eq!(
        keys(&mut m, &film, &[Input::Right]),
        vec![with_effects(&film, settings::preset_effects(2))]
    );
    // row 1 is the first Colour effect (warm, off here): → turns it on at
    // full strength, ← from off does nothing, Enter toggles
    keys(&mut m, &c, &[Input::Down]);
    assert_eq!(settings::filter_at(m.filter_row()), Some("warm"));
    let mut warm = c.look.effects.clone();
    warm.stack.push("warm".into());
    assert_eq!(keys(&mut m, &c, &[Input::Right]), vec![with_effects(&c, warm.clone())]);
    assert!(keys(&mut m, &c, &[Input::Left]).is_empty());
    assert_eq!(keys(&mut m, &c, &[Input::Enter]), vec![with_effects(&c, warm)]);
    // an effect that is on: ← weakens it, → strengthens it
    let bloom_row = (1..settings::filter_rows()).find(|r| settings::filter_at(*r) == Some("bloom")).unwrap();
    while m.filter_row() != bloom_row {
        keys(&mut m, &c, &[Input::Down]);
    }
    let mut weaker = c.look.effects.clone();
    weaker.set_amount("bloom", 0.7);
    assert_eq!(keys(&mut m, &c, &[Input::Left]), vec![with_effects(&c, weaker)]);
}

#[test]
fn filter_groups_cover_the_filter_cycle_exactly() {
    let mut grouped: Vec<&str> = settings::filter_list().collect();
    let mut cycle: Vec<&str> = crate::filter::FILTER_CYCLE.to_vec();
    grouped.sort_unstable();
    cycle.sort_unstable();
    assert_eq!(grouped, cycle);
    for f in crate::filter::FILTER_CYCLE {
        assert!(!settings::filter_help(f).is_empty(), "{f} has no help");
    }
    for (name, p) in settings::PRESETS {
        assert!(p.iter().all(|(f, _)| crate::filter::FILTER_CYCLE.contains(f)), "{name}");
        assert!(p.iter().all(|(_, a)| (0.0..=2.0).contains(a)), "{name}");
    }
    // every preset is recognised as itself
    for i in 0..settings::PRESETS.len() {
        assert_eq!(settings::preset_of(&settings::preset_effects(i)), Some(i));
    }
}

#[test]
fn zero_inside_the_menu_does_nothing() {
    let c = mid();
    for page in Page::ALL {
        let mut m = opened(&c);
        m.goto(page);
        let row = m.row();
        assert!(keys(&mut m, &c, &[Input::Char('0')]).is_empty(), "{page:?}");
        assert!(m.open && m.page == page && m.row() == row);
    }
    // and the key reaches the menu (the host routes menu keys before reset)
    let zero = crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('0'));
    assert_eq!(Input::from_key(zero), Some(Input::Char('0')));
}

#[test]
fn key_mapping() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let k = |code, mods| Input::from_key(KeyEvent::new(code, mods));
    assert_eq!(
        k(KeyCode::Char('c'), KeyModifiers::CONTROL),
        None,
        "Ctrl-C stays with the host"
    );
    assert_eq!(
        k(KeyCode::BackTab, KeyModifiers::SHIFT),
        Some(Input::BackTab)
    );
    assert_eq!(k(KeyCode::Tab, KeyModifiers::SHIFT), Some(Input::BackTab));
    assert_eq!(k(KeyCode::Tab, KeyModifiers::NONE), Some(Input::Tab));
    assert_eq!(k(KeyCode::F(5), KeyModifiers::NONE), None);
}

#[test]
fn help_overlay_toggles_and_swallows_keys() {
    let c = mid();
    let mut m = opened(&c);
    keys(&mut m, &c, &[Input::Char('?')]);
    assert!(m.help);
    assert!(keys(&mut m, &c, &[Input::Tab, Input::Char('f'), Input::Left]).is_empty());
    assert_eq!(m.page, Page::Scenes, "Tab is swallowed while help is up");
    assert!(m.help);
    keys(&mut m, &c, &[Input::Esc]);
    assert!(!m.help && m.open);
}

#[test]
fn footer_only_lists_working_keys() {
    let has =
        |m: &Menu, c: &MenuCtx, k: &str| view::key_hints(m, c).iter().any(|(key, _)| *key == k);
    let c = mid();
    let mut m = opened(&c);
    assert!(has(&m, &c, "f") && has(&m, &c, "Enter"));
    keys(&mut m, &c, &[Input::Left]);
    assert!(!has(&m, &c, "f"), "f does nothing on the shelves");
    // empty Favorites: nothing to switch to or star
    keys(&mut m, &c, &[Input::Home, Input::Right]);
    assert!(!has(&m, &c, "Enter") && !has(&m, &c, "f"));
    // the old footer's fps/speed keys never appear
    for page in Page::ALL {
        m.goto(page);
        assert!(!has(&m, &c, "[ ]") && !has(&m, &c, ", ."));
    }
    // an Info row has nothing to change; a disabled Group has no ←→
    m.goto(Page::Wall);
    keys(&mut m, &c, &[Input::End]);
    assert!(!has(&m, &c, "←→") && !has(&m, &c, "Enter"));
    let solo = MenuCtx {
        link_enabled: false,
        ..mid()
    };
    keys(&mut m, &solo, &[Input::Home, Input::Down]);
    assert_eq!(settings::WALL[m.row()].id, SettingId::Group);
    assert!(!has(&m, &solo, "←→"));
}

#[test]
fn cycle_next_respects_scope() {
    let names = scene::all_names();
    let at = |n: &str| names.iter().position(|x| *x == n).unwrap();
    let favs = vec!["koi".to_string(), "fire".to_string()];
    let i = browser::cycle_next(&names, at("rain"), CycleScope::All, &favs);
    assert_eq!(i, (at("rain") + 1) % names.len());
    // favourites: only starred scenes, wrapping round
    let a = browser::cycle_next(&names, at("rain"), CycleScope::Favorites, &favs);
    let b = browser::cycle_next(&names, a, CycleScope::Favorites, &favs);
    let mut got = vec![names[a], names[b]];
    got.sort_unstable();
    assert_eq!(got, vec!["fire", "koi"]);
    // no favourites yet: every scene
    let i = browser::cycle_next(&names, at("rain"), CycleScope::Favorites, &[]);
    assert_eq!(i, (at("rain") + 1) % names.len());
    // category: stays inside the current scene's category
    let i = browser::cycle_next(&names, at("rain"), CycleScope::Category, &[]);
    assert_eq!(
        scene::lookup(names[i]).unwrap().category(),
        Category::Classic
    );
}

#[test]
fn gpu_state_from_status() {
    assert_eq!(
        gpu_state("GPU shader · RTX", true, Renderer::Auto),
        Some(true)
    );
    assert_eq!(
        gpu_state("CPU · GPU unavailable", true, Renderer::Auto),
        Some(false)
    );
    assert_eq!(gpu_state("anything", false, Renderer::Auto), Some(false));
    assert_eq!(gpu_state("GPU post", true, Renderer::Cpu), Some(false));
    assert_eq!(
        gpu_state("Renderer initializing…", true, Renderer::Auto),
        None
    );
}

// ── layout ──────────────────────────────────────────────────────────────

fn screen(w: u16, h: u16, m: &Menu, c: &MenuCtx) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| view::render(f, f.area(), m, c)).unwrap();
    let buf = term.backend().buffer();
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

/// Every state worth drawing: each page, search, the filters sub-page, help.
fn states(c: &MenuCtx) -> Vec<(&'static str, Menu)> {
    let mut v = Vec::new();
    for page in Page::ALL {
        let mut m = opened(c);
        m.goto(page);
        v.push((page.title(), m));
    }
    let mut m = opened(c);
    keys(&mut m, c, &[Input::Char('/')]);
    type_text(&mut m, c, "ocean");
    v.push(("search", m));
    let mut m = opened(c);
    m.goto(Page::Look);
    focus(&mut m, c, SettingId::Filters);
    keys(&mut m, c, &[Input::Enter]);
    v.push(("filters", m));
    let mut m = opened(c);
    m.goto(Page::Themes);
    keys(&mut m, c, &[Input::Char('/')]);
    type_text(&mut m, c, "nord");
    v.push(("theme search", m));
    let mut m = opened(c);
    m.goto(Page::Themes);
    keys(&mut m, c, &[Input::Char('n')]);
    type_text(&mut m, c, "Late night");
    v.push(("theme prompt", m));
    let mut m = opened(c);
    keys(&mut m, c, &[Input::Char('?')]);
    v.push(("help", m));
    let mut m = opened(c);
    keys(&mut m, c, &[Input::Left, Input::Home, Input::Right]);
    v.push(("empty favourites", m));
    v
}

#[test]
fn layout_degrades_sanely() {
    let c = ctx();
    for (w, h) in [(40, 12), (80, 24), (200, 60), (12, 5), (1, 1)] {
        for (name, m) in states(&c) {
            let s = screen(w, h, &m, &c);
            if w < 40 {
                continue; // tiny terminals: only "does not panic"
            }
            // the help overlay may cover the tabs
            assert!(
                name == "help" || s.contains("Scenes"),
                "{name} at {w}x{h}: no tabs\n{s}"
            );
            let key_text = match name {
                "Scenes" => "rain", // Classic scenes are titled by name
                "Themes" => "Clean",
                "Look" => "Variant",
                "Playback" => "Speed",
                "Display" => "Quality",
                "Wall" => "Link",
                "search" => "/ ocean",
                "theme search" => "Nord",
                "theme prompt" => "Late night",
                "filters" => "Preset",
                "help" => "help",
                _ => "Favorites",
            };
            assert!(
                s.contains(key_text),
                "{name} at {w}x{h}: missing {key_text:?}\n{s}"
            );
        }
    }
}

#[test]
fn drawer_leaves_the_scene_visible() {
    use ratatui::layout::Rect;
    let r = view::drawer_rect(Rect::new(0, 0, 200, 60));
    assert_eq!(r.width, 92, "46% of the width");
    assert_eq!(
        view::drawer_rect(Rect::new(0, 0, 80, 24)).width,
        44,
        "never under 44"
    );
    assert_eq!(
        view::drawer_rect(Rect::new(0, 0, 60, 24)).width,
        60,
        "full width when narrow"
    );
    // at 120 columns the right half of the screen is untouched by the menu
    let c = ctx();
    let m = opened(&c);
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    term.draw(|f| {
        for y in 0..36 {
            for x in 0..120 {
                f.buffer_mut()[(x, y)].set_symbol("▀");
            }
        }
        view::render(f, f.area(), &m, &c);
    })
    .unwrap();
    let buf = term.backend().buffer();
    assert_eq!(buf[(119, 18)].symbol(), "▀");
    assert_eq!(buf[(56, 18)].symbol(), "▀");
    assert_ne!(buf[(20, 18)].symbol(), "▀", "drawer covers the left");
}

/// `TERMPAPER_SNAPSHOT=100x30 cargo test snapshot -- --nocapture` prints
/// every menu state as text at that size.
#[test]
fn snapshot() {
    let mut c = ctx();
    c.favorites = vec!["bigsur".into(), "koi".into()];
    c.recents = vec!["fire".into(), "rain".into()];
    let size = std::env::var("TERMPAPER_SNAPSHOT").ok();
    let (w, h) = size
        .as_deref()
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((100, 30));
    for (name, m) in &states(&c) {
        let s = screen(w, h, m, &c);
        assert!(s.contains("termpaper"), "{name}");
        if size.is_some() {
            println!("── {name} ──\n{s}");
        }
    }
}
