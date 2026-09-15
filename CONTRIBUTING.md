# Contributing to termpaper

termpaper is open-source terminal cinema. The best way to add your voice is a
**scene** — a self-contained animation module — registered in the catalog.

## Quick start

```sh
git clone https://github.com/Aphrodine-wq/termpaper.git
cd termpaper
./install.sh              # or: cargo install --path . --locked
cargo test --lib           # run the test suite
```

## Adding a scene

### 1. Create `src/scene/your_scene.rs`

Every scene implements the `Scene` trait:

```rust
use super::{Detail, Scene};
use crate::canvas::Canvas;
use rand::rngs::StdRng;

pub struct YourScene {
    detail: Detail,
    t: f32,
    w: usize,
    h: usize,
}

impl YourScene {
    pub fn new(_rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let _ = theme; // or match on theme names
        YourScene { detail, t: 0.0, w: 0, h: 0 }
    }
}

impl Scene for YourScene {
    fn name(&self) -> &'static str {
        "your_scene"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        self.t += dt;
        canvas.clear((0, 0, 0));
        // draw every frame — full redraw, edge to edge
    }
}
```

### 2. Register in `src/scene/mod.rs`

1. `pub mod your_scene;`
2. Add to `SCENES`: `("your_scene", "one-line description"),`
3. Add themes: `"your_scene" => &["classic", "mono"],`
4. Add to `create()`: `"your_scene" => Some(Box::new(your_scene::YourScene::new(rng, theme, d))),`

### 3. Verify

```sh
cargo test --lib
cargo run --example frame_dump -- your_scene 8 120x40 classic
termpaper --list | grep your_scene
```

The `edge_coverage_tests` and `scaling_tests` in `mod.rs` run against **every**
registered scene automatically.

## Design notes

Follow the polish bar used across the catalog:

- **2–3 depth planes** — background, midground, foreground
- **Hand-tuned palettes** — per-theme color sets, not random RGB
- **Events** — ~18–30s anticipation → payoff → decay loops
- **Detail scaling** — use `detail.scale(count, min)` so low/medium/high modes matter
- **Edge-to-edge** — fill the canvas; no borders or unused margins

Reuse helpers from `canvas.rs` (`lerp`, `scale`, `glow`, `disc`, `ease_smooth`,
`density_for`), `scene/noise.rs` (`fbm`), and `physics.rs` (`spring_damper`).

Use `disc()` for round shapes rather than a `dx*dx + dy*dy <= r*r` test — it
antialiases the rim, which matters at the 1–4 cell radii scenes actually use.
Note that `density_for(w, h)` **already contains a `w * h` term**, so
`(w * h / K) as f32 * density_for(w, h)` is quadratic in area: on a normal
terminal it lands under the count floor, and `detail.scale`'s low/high split
then collapses onto that floor and does nothing. Pick one area term, not two.

## Post-processing

Users can stack filters and global color grading at runtime (Settings menu or
`c` overlay). Scenes should look good unfiltered; filters are optional seasoning.

Available filters live in `src/filter.rs`. To add one:

1. Implement `pub fn my_filter(canvas: &mut Canvas) { ... }`
2. Add the name to `FILTER_CYCLE`
3. Wire it in `apply()` match arm
4. Add a unit test

## Color grading

Global hue / saturation / contrast is applied in `src/color_grade.rs` after
scene render and before dim/smooth. Scenes do not need to know about it.

## Pull requests

- Keep scenes focused — one file, one visual idea
- Include a short README highlight if the scene is substantial
- Run `cargo test --lib` before opening a PR
- MIT license — your scene code is contributed under the same license

## Ideas welcome

Scenes that are distinct from existing ones — new motion, perspective, or mood —
are the most valuable contributions. Check `termpaper --list` first to avoid
duplicating something already in the catalog.
