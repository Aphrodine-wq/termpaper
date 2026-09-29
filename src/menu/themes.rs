//! The Themes page: every theme (built in and yours) with swatches that show
//! what it does, a live preview while you browse, and the tools to make,
//! update, share and import your own.

use crate::look::{Look, PaletteMode};
use crate::theme::{Entry, Source, Store, CATEGORIES};

/// One theme as the menu shows it; the host builds these from the store.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeRow {
    pub slug: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub tags: Vec<String>,
    /// the heading it is listed under
    pub shelf: &'static str,
    /// a file in your themes folder (can be updated, renamed, deleted)
    pub yours: bool,
    /// colours that show what the theme does: its palette, or reference
    /// colours run through its grade
    pub swatches: Vec<(u8, u8, u8)>,
    /// the scene (and variant) it was made for
    pub scene: Option<(String, Option<String>)>,
    pub look: Look,
}

/// What a text prompt is asking for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// a name for a new theme made from the current look
    NewTheme,
    /// a new name for one of your themes
    Rename(String),
    /// "y" to delete one of your themes
    Delete(String),
    /// a tp1: code or a file path
    Import,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub kind: PromptKind,
    pub text: String,
}

impl Prompt {
    pub fn new(kind: PromptKind, text: impl Into<String>) -> Self {
        Prompt { kind, text: text.into() }
    }

    /// The question, as the footer asks it.
    pub fn question(&self) -> String {
        match &self.kind {
            PromptKind::NewTheme => "Save your look as".into(),
            PromptKind::Rename(_) => "New name".into(),
            PromptKind::Delete(_) => "Delete it? type y".into(),
            PromptKind::Import => "Paste a tp1: code or a file path".into(),
        }
    }

    /// Longest answer accepted (share codes are long).
    pub fn max_len(&self) -> usize {
        match self.kind {
            PromptKind::Import => 4096,
            _ => crate::theme::MAX_NAME,
        }
    }
}

/// The themes matching every word of `query` (name, tags, description,
/// shelf), in the host's order.
pub fn visible<'a>(rows: &'a [ThemeRow], query: &str) -> Vec<&'a ThemeRow> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    rows.iter()
        .filter(|r| {
            if words.is_empty() {
                return true;
            }
            let hay = format!(
                "{} {} {} {} {}",
                r.name.to_lowercase(),
                r.tags.join(" "),
                r.description.to_lowercase(),
                r.shelf.to_lowercase(),
                r.author.to_lowercase()
            );
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

/// Up to six colours that show what a look does: its palette when it has
/// one, else reference colours (sky, foliage, amber, magenta, skin, shadow)
/// run through it.
pub fn swatches(look: &Look) -> Vec<(u8, u8, u8)> {
    let c = &look.palette.colors;
    if c.len() >= 2 && look.palette.mode != PaletteMode::Off {
        let k = c.len().min(6);
        return (0..k).map(|i| c[i * (c.len() - 1) / (k - 1)].tuple()).collect();
    }
    [1, 2, 3, 4, 0, 7].iter().map(|&i| crate::studio::graded(crate::studio::REFS[i], look)).collect()
}

fn row(e: &Entry, shelf: &'static str) -> ThemeRow {
    let t = &e.theme;
    ThemeRow {
        slug: e.slug.clone(),
        name: t.name.clone(),
        author: t.author.clone(),
        description: t.description.clone(),
        tags: t.tags.clone(),
        shelf,
        yours: e.source == Source::User,
        swatches: swatches(&t.look),
        scene: t.scene.as_ref().map(|s| (s.name.clone(), s.variant.clone())),
        look: t.look.clone(),
    }
}

/// Every theme in the store as the Themes page lists it: Clean to start
/// from, then yours, then the built-ins by category.
pub fn rows(store: &Store) -> Vec<ThemeRow> {
    let builtin = |e: &&Entry| e.source == Source::Builtin;
    let mut rows = Vec::new();
    rows.extend(store.entries.iter().filter(builtin).filter(|e| e.slug == "clean").map(|e| row(e, "Start")));
    rows.extend(store.entries.iter().filter(|e| e.source == Source::User).map(|e| row(e, "Yours")));
    for (tag, heading) in CATEGORIES {
        rows.extend(
            store
                .entries
                .iter()
                .filter(builtin)
                .filter(|e| e.slug != "clean" && e.theme.category() == Some(*tag))
                .map(|e| row(e, heading)),
        );
    }
    // anything uncategorised still shows up
    rows.extend(
        store
            .entries
            .iter()
            .filter(builtin)
            .filter(|e| e.slug != "clean" && e.theme.category().is_none())
            .map(|e| row(e, "More")),
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_is_listed_once_clean_first() {
        let store = Store::load_from(None);
        let r = rows(&store);
        assert_eq!(r.len(), store.entries.len());
        assert_eq!(r[0].slug, "clean");
        assert_eq!(r[0].shelf, "Start");
        let mut slugs: Vec<&str> = r.iter().map(|t| t.slug.as_str()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), r.len(), "no theme listed twice");
        assert!(r.iter().all(|t| t.shelf != "More"), "every built-in has a category");
        // shelves are contiguous
        let mut seen: Vec<&str> = Vec::new();
        for t in &r {
            if seen.last() != Some(&t.shelf) {
                assert!(!seen.contains(&t.shelf), "{} split", t.shelf);
                seen.push(t.shelf);
            }
        }
    }

    #[test]
    fn swatches_show_the_palette_or_the_grade() {
        let store = Store::load_from(None);
        let r = rows(&store);
        let clean = &r[0];
        assert_eq!(clean.swatches.len(), 6);
        // Clean leaves the reference colours alone
        assert_eq!(clean.swatches[0], crate::studio::REFS[1]);
        let gb = r.iter().find(|t| t.slug == "game-boy").unwrap();
        let pal: Vec<_> = gb.look.palette.colors.iter().map(|c| c.tuple()).collect();
        assert!(gb.swatches.iter().all(|s| pal.contains(s)), "palette themes show their colours");
        assert_eq!(gb.swatches.first(), pal.first());
        assert_eq!(gb.swatches.last(), pal.last());
    }

    #[test]
    fn search_matches_every_word() {
        let r = rows(&Store::load_from(None));
        let hit = visible(&r, "tokyo");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].slug, "tokyo-night");
        assert!(visible(&r, "retro green").iter().any(|t| t.slug == "green-phosphor"));
        assert!(visible(&r, "zzz-nothing").is_empty());
        assert_eq!(visible(&r, "  ").len(), r.len());
    }
}
