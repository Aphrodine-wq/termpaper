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
rain_ snow_ bokeh_ light_ fire_ l2d_ entry_`. The name `F` is taken too: the
entry point declares `var<uniform> F: Frame`, so a scene's own constant
called `F` (a focal length, say) fails to compile — call it `FOCAL`. The modules live in
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
- Reserved words bite: `target`, `patch`, `pass`, `filter`, `sample`, `texture`, `meta`.
- `%` on floats is not GLSL `mod`: use `fmod_pos`. No implicit int/float
  conversion. An array you index dynamically must be a `var`.
- `smoothstep(a, b, x)` is undefined for `a >= b` (use `sstep`).

### Lessons from the first 41 scenes

- **Shadow rays:** the march stops up to ~0.4 px from the surface, so a fixed
  normal offset self-shadows at distance. Start at
  `p + n * (base + t * ctx.px * 1.5)`.
- **Cheap bounds count as hits:** `rm_march` treats any distance below the
  pixel-footprint threshold as a hit, so pad your early-out bounds (return a
  distance a little larger than the true gap) or you get invisible walls.
- **Rain costs bandwidth:** `rain_streaks` adds uniform brightness; gate it by
  local light (streaks only show where lamps light them) or it repaints most
  of the terminal every frame.
- **Sky colour:** the sky model has no ozone, so low-sun horizons drift olive;
  tint or blend toward a palette horizon where it matters.
- **Walls:** a portrait monitor beside the landscape row sees far past the
  frame (|p.x| up to ~2.8, |p.y| up to ~1). The entry point continues the view
  as a cylinder there (x only), which suits perspective scenes; flat 2.5D
  scenes that want true desk scale can undo it past `ctx.half.x`. Rooms must
  not end in black: continue walls, add a second window or lamp.

### Iterate

```sh
cargo test --test shader_scenes                                    # naga-validates every scene, lints the contract
cargo run --release --example shader_review -- <name>              # contact sheets → target/shader_review/
cargo run --release --example shader_review -- <name> --bench      # GPU time vs budget
cargo run --release --example shader_review -- <name> --desk       # across your real monitors (Hyprland), to scale
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

People stack effects and grade the picture at runtime (the menu, the colour
studio, themes). Scenes should look good with none of it; effects are
seasoning.

The order, on the CPU and the GPU alike: effects → grade (hue, saturation,
contrast) → the look's 3D lookup table (exposure, white balance, gamma,
vibrance, tone wheels, matte, palette map or tint) → palette snap → the `f`
preview effect → dim and the scene transition → smoothing → cell packing.
The CPU is the reference: every pass mirrored in `src/gpu/shaders/post.wgsl`
has a parity test in `tests/gpu_parity.rs`, run with `--ignored` on a GPU.

To add an effect:

1. Implement it in `src/filter.rs` as `fn my_effect(canvas: &mut Canvas, …)`
   taking what its strength means (see `params`: strength 1 is the effect's
   usual, 0–2 the range), and dispatch it in `apply_with`.
2. Add the name to `FILTER_CYCLE`, a line to `filter_help` in
   `src/menu/settings.rs`, and it to `reads_neighbours` if it samples
   neighbouring pixels (walls then render an apron for it).
3. Mirror it in `post.wgsl` and extend the parity test.
4. Add a unit test.

## Writing a theme

A theme is data, not code: a TOML file with a colour grade, an optional
palette and an effect stack. The easiest way to make one is in termpaper —
dial in a look (`c` for the colour studio, `?` → Look → Effects…), then
`n` on the Themes page — or in the browser at the site's theme studio.
Then it is a file in `~/.config/termpaper/themes/` to refine by hand:

```toml
format = 1                    # the file format; termpaper refuses newer ones
name = "Late Shift"           # up to 40 characters
author = "you"                # up to 32
description = "What it does, in a line."   # up to 160
tags = ["warm", "film"]       # up to 8, lower case

[grade]                       # every key optional; the neutral value is shown
hue = 0.0                     # 0–360, turns every colour round the wheel
saturation = 1.0              # 0–2.5
contrast = 1.0                # 0.5–2.5
exposure = 0.0                # -2–2, in stops
vibrance = 0.0                # -1–1
temperature = 0.0             # -1 cool … 1 warm
tint = 0.0                    # -1 green … 1 magenta
gamma = 1.0                   # 0.5–2, above 1 lifts the midtones
fade = 0.0                    # 0–0.5, lifts the blacks (matte)
shadows = { hue = 215.0, amount = 0.0 }      # tone wheels: amount 0–1
midtones = { hue = 30.0, amount = 0.0 }
highlights = { hue = 40.0, amount = 0.0 }
balance = 0.0                 # -1–1, where shadows end and highlights begin

[palette]
mode = "map"                  # off | map (by brightness) | tint | snap
colors = ["#1a1b26", "#7aa2f7", "#e0af68"]   # 2–8, darkest first for map
strength = 1.0                # 0–1
dither = false                # snap only

[effects]
stack = ["halation", "grain"] # applied in this order
grain = 0.5                   # a strength per effect, 0–2; 1 when left out

[scene]                       # optional: the scene it was made for
name = "tokyo"
variant = "rain"

[display]                     # optional
dim = 0.8                     # a brightness to go with it
```

`termpaper theme check FILE` loads it the way termpaper will and says what
it clamped or dropped. `termpaper theme export NAME --code` gives the share
code, and `termpaper theme publish NAME` puts it in the gallery.

The built-in themes are the same files in `src/themes/`, compiled in by
`build.rs` (their `tags` start with their shelf: cinematic, terminal, retro
or mood). A new one there needs nothing else; the site's copy of the list
comes from `termpaper theme list --json`.

## Pull requests

- Keep scenes focused — one file, one visual idea
- Include a short README highlight if the scene is substantial
- Run `cargo test --all-targets` (and with `--no-default-features`) before
  opening a PR; with a GPU, the `--ignored` GPU tests too
- MIT license — your scene code is contributed under the same license

## Ideas welcome

Scenes that are distinct from existing ones — new motion, perspective, or mood —
are the most valuable contributions. Check `termpaper list` first to avoid
duplicating something already in the catalog.
