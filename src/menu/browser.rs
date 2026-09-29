//! The Scenes page: shelves (favourites, recents, categories) on the left,
//! the chosen shelf's scenes on the right, and type-to-filter search.

use super::MenuCtx;
use crate::config::CycleScope;
use crate::scene::{self, Category, Entry};

/// How many recently switched-to scenes are remembered.
pub const RECENTS: usize = 8;

/// Classic sub-groups, in shelf order (see `scene::Entry::group`). Groups
/// missing here still show, after these.
pub const CLASSIC_GROUPS: &[&str] = &[
    "nature",
    "water",
    "space",
    "demoscene",
    "generative",
    "urban",
    "machines",
    "cozy",
];

/// One entry in the left column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shelf {
    Favorites,
    Recent,
    Category(Category),
    /// a Classic sub-group ("nature", "water", …), nested under Classic
    Group(&'static str),
}

impl Shelf {
    pub fn label(self) -> &'static str {
        match self {
            Shelf::Favorites => "★ Favorites",
            Shelf::Recent => "◷ Recent",
            Shelf::Category(c) => c.label(),
            Shelf::Group(g) => g,
        }
    }

    pub fn nested(self) -> bool {
        matches!(self, Shelf::Group(_))
    }
}

fn group_rank(g: &str) -> usize {
    CLASSIC_GROUPS
        .iter()
        .position(|x| *x == g)
        .unwrap_or(CLASSIC_GROUPS.len())
}

fn classic() -> impl Iterator<Item = Entry> {
    scene::entries().filter(|e| e.category() == Category::Classic)
}

/// Every shelf, in order: Favorites, Recent, then each category that has
/// scenes — Classic followed by its sub-groups. Empty categories are left
/// out until scenes land in them.
pub fn shelves() -> Vec<Shelf> {
    let mut v = vec![Shelf::Favorites, Shelf::Recent];
    for c in Category::ALL {
        if !scene::entries().any(|e| e.category() == c) {
            continue;
        }
        v.push(Shelf::Category(c));
        if c == Category::Classic {
            let mut groups: Vec<&'static str> = Vec::new();
            for e in classic() {
                if !groups.contains(&e.group()) {
                    groups.push(e.group());
                }
            }
            groups.sort_by_key(|g| group_rank(g));
            v.extend(groups.into_iter().map(Shelf::Group));
        }
    }
    v
}

/// Scenes on a shelf, in display order. Favourites and recents keep their
/// stored order and skip names that no longer exist.
pub fn shelf_scenes(shelf: Shelf, ctx: &MenuCtx) -> Vec<Entry> {
    let named = |names: &[String]| names.iter().filter_map(|n| scene::lookup(n)).collect();
    match shelf {
        Shelf::Favorites => named(&ctx.favorites),
        Shelf::Recent => named(&ctx.recents),
        Shelf::Category(Category::Classic) => {
            let mut v: Vec<Entry> = classic().collect();
            v.sort_by_key(|e| group_rank(e.group()));
            v
        }
        Shelf::Category(c) => scene::entries().filter(|e| e.category() == c).collect(),
        Shelf::Group(g) => classic().filter(|e| e.group() == g).collect(),
    }
}

/// Scenes matching every word of `query` (case-insensitive) anywhere in
/// their name, title, description, tags or category. Hits on the name or
/// title sort first; otherwise catalog order.
pub fn search(query: &str) -> Vec<Entry> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let Some(first) = words.first() else {
        return scene::entries().collect();
    };
    let mut hits: Vec<(u8, Entry)> = scene::entries()
        .filter_map(|e| {
            let name = e.name().to_lowercase();
            let title = e.title().to_lowercase();
            let hay = format!(
                "{name} {title} {} {} {} {}",
                e.desc().to_lowercase(),
                e.tags().join(" ").to_lowercase(),
                e.category().label().to_lowercase(),
                e.group().to_lowercase(),
            );
            if !words.iter().all(|w| hay.contains(w.as_str())) {
                return None;
            }
            let rank = if name.starts_with(first.as_str()) || title.starts_with(first.as_str()) {
                0
            } else if name.contains(first.as_str()) || title.contains(first.as_str()) {
                1
            } else {
                2
            };
            Some((rank, e))
        })
        .collect();
    hits.sort_by_key(|(rank, _)| *rank);
    hits.into_iter().map(|(_, e)| e).collect()
}

/// Which column of the browser has focus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Column {
    Shelves,
    Scenes,
}

pub struct Browser {
    pub column: Column,
    /// index into [`shelves`]
    pub shelf: usize,
    /// index into [`Browser::list`]
    pub row: usize,
    pub query: String,
    /// the search box has the keyboard
    pub searching: bool,
}

impl Default for Browser {
    fn default() -> Self {
        Self::new()
    }
}

impl Browser {
    pub fn new() -> Self {
        Browser {
            column: Column::Scenes,
            shelf: 0,
            row: 0,
            query: String::new(),
            searching: false,
        }
    }

    pub fn current_shelf(&self) -> Shelf {
        let all = shelves();
        all[self.shelf.min(all.len() - 1)]
    }

    /// True when the right column shows search hits instead of a shelf.
    pub fn filtering(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// The right-hand list: search hits while a query is typed, else the
    /// current shelf.
    pub fn list(&self, ctx: &MenuCtx) -> Vec<Entry> {
        if self.filtering() {
            search(&self.query)
        } else {
            shelf_scenes(self.current_shelf(), ctx)
        }
    }

    /// Row within `len` items — favourites can shrink under the cursor.
    pub fn row_in(&self, len: usize) -> usize {
        self.row.min(len.saturating_sub(1))
    }

    pub fn highlighted(&self, ctx: &MenuCtx) -> Option<Entry> {
        let list = self.list(ctx);
        list.get(self.row_in(list.len())).copied()
    }

    pub fn move_row(&mut self, delta: isize, ctx: &MenuCtx) {
        let n = self.list(ctx).len();
        let cur = self.row_in(n) as isize;
        self.row = (cur.saturating_add(delta)).clamp(0, n.saturating_sub(1) as isize) as usize;
    }

    pub fn move_shelf(&mut self, delta: isize) {
        let n = shelves().len();
        let cur = self.shelf.min(n - 1) as isize;
        self.shelf = (cur.saturating_add(delta)).clamp(0, n as isize - 1) as usize;
        self.row = 0;
    }

    /// Point at `name`: its category (a Classic scene: its sub-group), with
    /// the scene highlighted.
    pub fn reveal(&mut self, name: &str, ctx: &MenuCtx) {
        let Some(e) = scene::lookup(name) else {
            return;
        };
        let target = match e.category() {
            Category::Classic => Shelf::Group(e.group()),
            c => Shelf::Category(c),
        };
        if let Some(i) = shelves().iter().position(|s| *s == target) {
            self.shelf = i;
            self.row = shelf_scenes(target, ctx)
                .iter()
                .position(|x| x.name() == name)
                .unwrap_or(0);
        }
    }

    pub fn clear_search(&mut self) {
        self.query.clear();
        self.searching = false;
    }
}

/// Record a switch: newest first, no duplicates, at most [`RECENTS`].
pub fn push_recent(recents: &mut Vec<String>, name: &str) {
    recents.retain(|n| n != name);
    recents.insert(0, name.to_string());
    recents.truncate(RECENTS);
}

/// Star or unstar a scene; true if it is now a favourite.
pub fn toggle_favorite(favorites: &mut Vec<String>, name: &str) -> bool {
    if let Some(i) = favorites.iter().position(|n| n == name) {
        favorites.remove(i);
        false
    } else {
        favorites.push(name.to_string());
        true
    }
}

/// The scene the auto-cycle moves to after `names[current]`, within
/// `scope`. An empty scope (no favourites yet) cycles through everything;
/// a scope holding only the current scene stays on it.
pub fn cycle_next(
    names: &[&'static str],
    current: usize,
    scope: CycleScope,
    favorites: &[String],
) -> usize {
    let n = names.len();
    if n == 0 {
        return 0;
    }
    let current = current % n;
    let category = |name: &str| scene::lookup(name).map(|e| e.category());
    let here = category(names[current]);
    let in_scope = |name: &str| match scope {
        CycleScope::All => true,
        CycleScope::Category => category(name) == here,
        CycleScope::Favorites => favorites.iter().any(|f| f == name),
    };
    let scoped = names.iter().any(|n| in_scope(n));
    (1..=n)
        .map(|step| (current + step) % n)
        .find(|&i| !scoped || in_scope(names[i]))
        .unwrap_or(current)
}
