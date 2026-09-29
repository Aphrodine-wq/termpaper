//! The Scenes page: shelves (favourites, recents, categories) on the left,
//! the chosen shelf's scenes on the right, and type-to-filter search.

use super::MenuCtx;
use crate::config::CycleScope;
use crate::prefs::CycleOrder;
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
    /// a handful of scenes picked at random when the menu opens
    Surprise,
    Category(Category),
    /// a Classic sub-group ("nature", "water", …), nested under Classic
    Group(&'static str),
}

impl Shelf {
    pub fn label(self) -> &'static str {
        match self {
            Shelf::Favorites => "★ Favorites",
            Shelf::Recent => "◷ Recent",
            Shelf::Surprise => "✦ Surprise",
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

/// How many scenes the Surprise shelf offers.
pub const SURPRISE: usize = 8;

/// Every shelf, in order: Favorites, Recent, Surprise, then each category
/// that has scenes — Classic followed by its sub-groups. Empty categories
/// are left out until scenes land in them.
pub fn shelves() -> Vec<Shelf> {
    let mut v = vec![Shelf::Favorites, Shelf::Recent, Shelf::Surprise];
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
/// stored order and skip names that no longer exist. The Surprise shelf's
/// scenes live in the [`Browser`] (see [`Browser::scenes_on`]): here it is
/// empty.
pub fn shelf_scenes(shelf: Shelf, ctx: &MenuCtx) -> Vec<Entry> {
    let named = |names: &[String]| names.iter().filter_map(|n| scene::lookup(n)).collect();
    match shelf {
        Shelf::Favorites => named(&ctx.favorites),
        Shelf::Recent => named(&ctx.recents),
        Shelf::Surprise => Vec::new(),
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
            let rank = if name == *first || title == *first {
                0
            } else if name.starts_with(first.as_str()) || title.starts_with(first.as_str()) {
                1
            } else if name.contains(first.as_str()) || title.contains(first.as_str()) {
                2
            } else {
                3
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
    /// the Surprise shelf: scene names picked when the menu opened
    pub surprise: Vec<&'static str>,
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
            surprise: Vec::new(),
        }
    }

    /// Roll a fresh Surprise shelf: [`SURPRISE`] distinct scenes, at random.
    pub fn reroll_surprise(&mut self) {
        let n = scene::entries().count();
        self.reroll_surprise_with(|k| rand::random_range(0..k.max(1)), n);
    }

    /// `reroll_surprise` with the picks injected (tests): `pick(k)` returns
    /// an index below `k`.
    pub fn reroll_surprise_with(&mut self, mut pick: impl FnMut(usize) -> usize, total: usize) {
        let mut pool: Vec<&'static str> = scene::all_names();
        pool.truncate(total);
        self.surprise.clear();
        while self.surprise.len() < SURPRISE && !pool.is_empty() {
            let i = pick(pool.len()).min(pool.len() - 1);
            self.surprise.push(pool.swap_remove(i));
        }
    }

    /// Scenes on a shelf, the Surprise shelf included.
    pub fn scenes_on(&self, shelf: Shelf, ctx: &MenuCtx) -> Vec<Entry> {
        match shelf {
            Shelf::Surprise => self.surprise.iter().filter_map(|n| scene::lookup(n)).collect(),
            _ => shelf_scenes(shelf, ctx),
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
            self.scenes_on(self.current_shelf(), ctx)
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

    /// `r`: a different scene from the same list, at random.
    pub fn random_row(&mut self, ctx: &MenuCtx) {
        let n = self.list(ctx).len();
        if n < 2 {
            return;
        }
        let cur = self.row_in(n);
        let step = rand::random_range(1..n);
        self.row = (cur + step) % n;
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
/// `scope`: the next in list order, or with `Shuffle` any other one, `pick`
/// choosing. An empty scope (no favourites yet) cycles through everything;
/// a scope holding only the current scene stays on it, and so does the
/// Variants scope (see [`cycle_variant`]).
pub fn cycle_next(
    names: &[&'static str],
    current: usize,
    scope: CycleScope,
    favorites: &[String],
    order: CycleOrder,
    pick: u64,
) -> usize {
    let n = names.len();
    if n == 0 {
        return 0;
    }
    let current = current % n;
    if scope == CycleScope::Variants {
        return current;
    }
    let category = |name: &str| scene::lookup(name).map(|e| e.category());
    if scope == CycleScope::Surprise {
        // a category first, so every kind of scene gets its turn, then a
        // scene in it (never the one showing)
        let cats: Vec<scene::Category> = scene::Category::ALL
            .into_iter()
            .filter(|c| (0..n).any(|i| i != current && category(names[i]) == Some(*c)))
            .collect();
        if cats.is_empty() {
            return current;
        }
        let cat = cats[(pick % cats.len() as u64) as usize];
        let in_cat: Vec<usize> = (1..n).map(|step| (current + step) % n).filter(|&i| category(names[i]) == Some(cat)).collect();
        return in_cat[((pick / cats.len() as u64) % in_cat.len() as u64) as usize];
    }
    let here = category(names[current]);
    let in_scope = |name: &str| match scope {
        CycleScope::All | CycleScope::Variants | CycleScope::Surprise => true,
        CycleScope::Category => category(name) == here,
        CycleScope::Favorites => favorites.iter().any(|f| f == name),
        CycleScope::Studio => scene::lookup(name).is_some_and(|e| e.needs_gpu()),
        CycleScope::Classic => scene::lookup(name).is_some_and(|e| !e.needs_gpu()),
    };
    let scoped = names.iter().any(|n| in_scope(n));
    let others: Vec<usize> = (1..n).map(|step| (current + step) % n).filter(|&i| !scoped || in_scope(names[i])).collect();
    match (others.len(), order) {
        (0, _) => current,
        (_, CycleOrder::InOrder) => others[0],
        (k, CycleOrder::Shuffle) => others[(pick % k as u64) as usize],
    }
}

/// The variant the Variants cycle moves to after `current` (None is the
/// scene's default, its first): the next, or with `Shuffle` any other.
/// None when the scene has fewer than two.
pub fn cycle_variant(variants: &[&'static str], current: Option<&str>, order: CycleOrder, pick: u64) -> Option<&'static str> {
    let n = variants.len();
    if n < 2 {
        return None;
    }
    let at = current.and_then(|c| variants.iter().position(|v| *v == c)).unwrap_or(0);
    let step = match order {
        CycleOrder::InOrder => 1,
        CycleOrder::Shuffle => 1 + (pick % (n as u64 - 1)) as usize,
    };
    Some(variants[(at + step) % n])
}
