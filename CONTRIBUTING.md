# Contributing to termpaper

termpaper is open-source terminal cinema. The best way to add your voice is a
**scene**. There are two kinds:

- **Studio scenes** (new work goes here): one stateless WGSL file under
  `src/gpu/scenes/`, rendered on the GPU. Real places, physically lit.
- **Classic scenes**: Rust modules under `src/scene/` drawing into a CPU
  canvas. The original catalog; still supported.

## Quick start

```sh
git clone https://github.com/Aphrodine-wq/termpaper.git
cd termpaper
./install.sh              # or: cargo install --path . --locked
cargo test --lib           # run the test suite
```

## Adding a Studio scene

A Studio scene is a single file, `src/gpu/scenes/<name>.wgsl`. `build.rs`
finds it; no Rust changes, no registry edits. It is a **pure function**:

```wgsl
fn scene(p: vec2f, ctx: Ctx) -> vec3f   // linear HDR radiance, >= 0, never NaN
```

Every pane of a multi-monitor wall calls it for its own crop at the shared
time, so a pure function is what makes walls line up exactly.

### Header

```wgsl
//! name: redwoods                   # = the file name; lowercase, digits, '-'
//! title: Redwood Fog               # <= 32 chars, shown in the browser
//! category: wilds                  # coast | wilds | weather | city | cozy | space
//! tags: forest, fog, god-rays
//! desc: shafts of morning sun cutting through fog between towering redwoods   # <= 90 chars
//! themes: morning, overcast, dusk  # 2-5; first is the default; each visibly different
//! uses: camera, raymarch, sky, fog # library modules (dependencies resolve themselves)
//! cost: heavy                      # light | medium | heavy (see Performance)
//! tonemap: agx                     # optional: agx (default) | punchy | aces | neutral
//! exposure: 0.0                    # optional, stops
//! fallback: canopy                 # Classic scene shown when there is no GPU
//! credits: original                # or the MIT/CC0 sources you drew on
```

### Coordinates and composition

- `p` is **y-up and centred on the whole wall frame**. The frame's *short*
  side spans [-0.5, 0.5]. `ctx.half` is the frame's half extent (16:9 →
  `half = (0.889, 0.5)`).
- Keep the hero of the picture inside `|p| <= 0.45`. The scene must stay
  defined far beyond that: walls reach `|p.x| >= 1.8`, portrait monitors
  `|p.y| ~ 0.9`. Extend, repeat or fade — never a hard edge or black band.
- Landscape horizons sit around `p.y` in [-0.2, 0.15]. For 3D use
  `cam_look_at(ro, at, roll, fov_short_deg)`: the FOV applies to the short
  side, so a portrait monitor sees more sky and foreground, like turning a
  real camera.
- Use `ctx.px` (one pixel in p units) for anti-aliasing widths and LOD, never
  hard-coded pixel sizes. The output is small (a 1080p terminal is ~213x112
  pixels), so soft edges and big shapes read best.

### Determinism (required)

- No state, storage buffers, atomics or workgroup memory. The lint in
  `tests/shader_scenes.rs` enforces it.
- Time comes only from `ctx.t` (seconds). Randomness only from `hash_*`:
  integer hashes, identical on every GPU. Never `fract(sin(...))`.
- Events (lightning, meteors, a passing car) are closed-form: use
  `hash_event(ctx.t, period, salt)` → (random, phase in slot, slot index).
- Per-launch variety only through `ctx.seed` / `hash_seeded2`.

### Terminal bandwidth

Every terminal cell that changes costs escape codes, and the terminal write is
the real bottleneck. So:

- Prefer a static camera (motion lives *in* the scene). If the camera must
  move, keep it glacial.
- Aim for under ~40% of the frame animating at any moment; leave calm areas.
- No temporal noise, film grain or animated dither (the entry point adds
  static dither). Flicker at most ~1 Hz and small in area.
- Big full-frame flashes (lightning) only as rare events.

### Performance

Measure with `cargo run --release --example shader_review -- <name> --bench`
(544x132 px, 1 sample per pixel, medium detail, about three 1080p panes):

| cost   | budget  |
|--------|---------|
| light  | 0.8 ms  |
| medium | 1.6 ms  |
| heavy  | 2.5 ms  |

At runtime a governor raises samples per pixel inside a GPU-time budget, so a
cheaper scene just gets smoother edges. Other rules:
- Every loop needs a literal bound (`for (var i = 0; i < 64; i++) { if (i >= n) { break; } }`).
- fbm at most 8 octaves. Compute expensive per-frame constants (sun colour,
  sky ambient) once per call and pass them down, not in every helper.
- Add cheap distance bounds to `map()` (above the tallest peak, far from any
  object) before evaluating detailed noise. That is usually the biggest win.

### Themes and detail

Branch on `ctx.theme` in a `look(theme)` function at the top of the file.
Themes are time-of-day or weather variants of the same place. Use `ctx.detail`
(0/1/2) and `ctx.march` for structural quality. Never change structure by
sample count: panes of one wall may run different counts.

### The library

Reserved prefixes (scenes must not define these names): `hash_ noise_ col_
tm_ dither_ cam_ sdf_ sdf2_ op_ rm_ sky_ star_ cloud_ vol_ fog_ water_ wet_
rain_ snow_ bokeh_ light_ fire_ l2d_ entry_`. The modules live in
`src/gpu/scenes/lib/`; read them, they are short. Highlights:

- `sky_atmosphere(rd, sun, ctx)`, `sky_sun_dir(elev, azim)`, `sky_sun_light`,
  `sky_ambient`, `sky_sun_disk`, `sky_moon`, `sky_night`
- `rm_march`, `rm_march_k`, `rm_normal`, `rm_shadow`, `rm_ao`, `rm_shafts`
  (the scene defines `fn map(p: vec3f, ctx: Ctx) -> vec2f`)
- `cloud_march` (volumetric slab), `cloud_sheet` (cheap distant layer)
- `fog_height`, `fog_exp`, `fog_sun`, `fog_layer2d`
- `water_height`, `water_normal`, `water_fresnel`, `water_color`,
  `water_rain_ripples`, `water_caustics`
- `rain_streaks`, `rain_glass`, `rain_splashes`, `snow_flakes`
- `bokeh_disc`, `bokeh_field`, `light_neon`, `light_flicker`, `light_ggx`,
  `light_window_grid`, `fire_flame`, `fire_embers`, `fire_light`
- `l2d_ridge`, `l2d_skyline`, `l2d_treeline`, `star_field`, `star_milky_way`
- `noise_*` (value, gradient, worley, fbm, ridged, warp, curl, terrain),
  `col_hex`, `col_kelvin`, `col_mix_oklab`, `sstep` (a smoothstep that
  accepts `a > b`), `fmod_pos` (GLSL `mod`)

### WGSL gotchas

- Mixing bitwise and arithmetic operators needs parentheses: `(a * b) ^ c`.
- Reserved words bite: `target`, `patch`, `filter`, `sample`, `texture`, `meta`.
- `%` on floats is not GLSL `mod`: use `fmod_pos`. No implicit int/float
  conversion. An array you index dynamically must be a `var`.
- `smoothstep(a, b, x)` is undefined for `a >= b` (use `sstep`).

### Iterate

```sh
cargo test --test shader_scenes                                    # naga-validates every scene, lints the contract
cargo run --release --example shader_review -- <name>              # contact sheets → target/shader_review/
cargo run --release --example shader_review -- <name> --bench      # GPU time vs budget
TERMPAPER_SHADER_DIR=src/gpu/scenes cargo run --release -- <name>  # live, hot-reloads on save
```

`shader_review` reads scenes from disk, so you can edit and re-run without
rebuilding. `<name>_term.png` shows the scene at the resolution a 1080p
terminal really displays; judge your work there.

### Licensing

Original code, or MIT/CC0 sources credited in `credits:`. **Do not port
Shadertoy code under CC BY-NC-SA** (most famous shaders are), because the
project is MIT. Techniques are fine to reimplement; code is not.


## Adding a Classic scene

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
