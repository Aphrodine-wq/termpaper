# termpaper

**Wallpaper Engine for the terminal.** termpaper renders fullscreen animated
art scenes in truecolor — eye-candy for a terminal pane, a tmux split, or a
screensaver. 47 scenes, 3 pixel modes, composable filters, per-scene themes,
and a built-in settings menu with fastfetch-style branding.

```sh
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash && termpaper rain
```

One command: install, then rain on glass. Already installed? Just `termpaper fire`.

```sh
termpaper rain            # run one scene
termpaper --cycle 30      # rotate through all scenes every 30s
termpaper --list          # see what's available
termpaper                 # then press ? for the menu
```

## Install

### Requirements

| What | Why |
|------|-----|
| **Truecolor terminal** | Full 24-bit art (`kitty`, `ghostty`, `alacritty`, `foot`, `wezterm`, …). Check: `echo $COLORTERM` → `truecolor` or `24bit`. |
| **Rust toolchain** | Only needed to build from source ([rustup.rs](https://rustup.rs)). |

256-color fallback works automatically if your terminal is not truecolor.

### Quick install (recommended)

From a clone of this repo:

```sh
git clone https://github.com/Aphrodine-wq/termpaper.git
cd termpaper
./install.sh
```

The script runs `cargo install`, symlinks `~/.local/bin/termpaper` if needed, and prints PATH hints.

One-liner (install + first scene):

```sh
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash && termpaper rain
```

Install only:

```sh
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash
```

Override the clone URL: `TERMPAPER_REPO=https://github.com/you/fork.git ./install.sh`

### Other install methods

**Cargo, from a local clone** (developers):

```sh
cargo install --path . --locked
```

**Cargo, from Git** (no clone needed):

```sh
cargo install --git https://github.com/Aphrodine-wq/termpaper.git --locked
```

**Arch Linux** (system package):

```sh
cd packaging/arch && makepkg -si
```

See [packaging/arch/README.md](packaging/arch/README.md) for local dev builds without a git tag.

**Prebuilt binary** (no Rust — after a [GitHub Release](https://github.com/Aphrodine-wq/termpaper/releases) exists):

```sh
./install.sh --binary
# or
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash -s -- --binary
```

Linux x86_64 and aarch64 builds are attached to each `v*` release tag.

### After install

`termpaper` lands in `~/.cargo/bin/termpaper`. If your shell says *command not found*:

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # add to ~/.bashrc or ~/.zshrc
```

Then try:

```sh
termpaper --list          # scenes + themes
termpaper rain            # default scene
termpaper life --fps 30   # pick a scene and cap fps
termpaper                 # press ? for settings
```

**Screensaver / fullscreen:** run in any terminal pane, tmux split, or dedicated window. For a Hyprland background layer, see [Video wall](#video-wall) and the example `termpaper-wallpaper.sh` pattern in the wiki/docs of your setup.

**Solo art** (no sync with other windows): `termpaper --no-link fire`

**Wallpaper group** (monitors sync together): `termpaper --group wallpaper rain`

Truecolor terminals (`COLORTERM=truecolor/24bit`) get full 24-bit color;
everything else falls back to the xterm-256 palette automatically (or force
it with `--no-truecolor`). termpaper is a color-art program — it overrides
`NO_COLOR` detection, since without color there is no picture.

## The menu — press `?`

A centered modal over the **live** scene (it keeps animating behind the box).
Every tab opens with a compact fastfetch-style header: ASCII logo, version,
scene, display mode, and link group. The **About** tab expands that into a
full product page.

- **Scenes** — scrollable browser with descriptions, `↑/↓` to browse,
  `Enter` to switch. Shows `n of 47`.
- **Settings** — Pixels, Detail, Theme, Hue, Saturation, Contrast, Text scale,
  Speed, FPS, Smooth, Dim, Fade, Clock, Cycle, and a filter toggle list. All
  apply live *and* persist to `config.toml` automatically.
- **Keybinds** — read-only table of current bindings.
- **About** — full product page with ASCII logo and live stats.

`Esc` or `?` closes.

## Quick start

```sh
termpaper rain              # run a scene
termpaper --list            # browse names + themes
termpaper fire --theme frost
termpaper --cycle 30        # rotate all scenes every 30s
termpaper --no-link candy   # solo instance, no sync
```

Press **`?`** for the settings menu · **`q`** to quit · **`0`** to reset defaults.

## Usage

```
termpaper [SCENE] [OPTIONS]

Arguments:
  [SCENE]               Scene to run (see --list)

Options:
      --list              List scenes and exit
      --cycle <SECS>      Rotate through all scenes every N seconds
      --fps <N>           Target frames per second [default: 60]
      --speed <MULT>      Animation speed multiplier [default: 1.0]
      --theme <NAME>      Scene color theme (per scene; e.g. fire: classic|frost|inferno)
      --detail <LEVEL>    low|medium|high — particle/layer/emitter counts
      --pixels <MODE>     half|quad|braille
      --text-scale <N>    1|2|3 — bump font scale
      --filter <NAME>     Repeatable: scanlines, vignette, grain, warm, cool, hue,
                          crt, bloom, duotone, pixelate, chroma
      --no-truecolor      Force 256-color output
      --screensaver       Any key exits
      --pad <PX>          Terminal padding in px (all sides) so wall crops
                          line up across window borders
  -h, --help              Print help
```

## Multiple terminals

Instances in the same **link group** sync automatically (file-based, no
sockets): each running termpaper registers in
`$XDG_RUNTIME_DIR/termpaper/` (group `default`) or
`$XDG_RUNTIME_DIR/termpaper/groups/<name>/`. Manual scene switches —
arrows, menu browser — publish to every other instance in that group,
which fades to the same scene. `--cycle` rotations deliberately don't
propagate.

```sh
termpaper --instances              # list all groups (pid, group, scene, uptime)
termpaper --switch fire            # default group only
termpaper --switch fire --group wallpaper
termpaper --switch fire --all-groups
termpaper --no-link                # solo art — no sync at all
termpaper --group desk             # separate sync cluster from wallpaper
```

The menu **Settings** tab has **Link** (on/off) and **Group** rows.
Presets cycle through `default`, `wallpaper`, `desk`, and `art`. The
**Instances** section shows who's in your group. Menu settings edits
(pixels, detail, filters, theme, text scale, speed, fps, smooth, dim,
fade) and the `f` quick filter also propagate live to linked instances in
the same group — remote applies are session-only and never touch your
config file. New instances adopt the group's current scene and settings
on launch (fast-forwarding to the same animation frame), and the
lowest-pid instance in the group re-publishes the current scene every 15s
as a sync anchor — identical re-publishes are skipped silently, so no
visible restarts.

Linking works on macOS too (iTerm2, Terminal.app, …): without
`$XDG_RUNTIME_DIR` the registry lives in `/tmp/termpaper-$UID`. Only the
video wall's *auto* window-geometry mode is Linux/Hyprland-only — manual
`--wall COLSxROWS:INDEX` tiling works everywhere.

## Video wall

Linked terminals with known window positions act as viewports onto one
shared virtual canvas — a meteor flies from one window into the next.
Auto mode reads window geometry from `hyprctl` (Hyprland only); manual
mode tiles without a compositor:

```sh
termpaper --wall 2x1:0   # left half of a 2x1 wall
termpaper --wall 2x1:1   # right half
termpaper --no-wall      # stay local
```

**Margin aware**: if your terminal pads its grid (kitty/alacritty
`padding`), tell termpaper so crops line up across window borders —
`pad = 7` in `config.toml` or `--pad 7`. Each instance publishes its
padding in the registry and the layout insets every window's content
rect, so the shared canvas runs edge-to-edge of the *text*, not the
frame. (Any remaining gap between windows' content belongs to the virtual
canvas too — the fullscreen wallpaper instance renders it.)

Seed/timestamp sync makes both halves the same continuous picture. If the
combined wall exceeds ~3x the standard canvas, instances stay local.

**Artwork sync**: a published switch carries an rng seed and start
timestamp — receivers build the scene from the same seed and fast-forward
the simulation, so two linked terminals show the *same* animation frames,
like one wallpaper spanning windows. Control messages are totally ordered
(millisecond epoch + per-publisher sequence, ties by pid), so rapid scene
flipping never drops a switch.

## Pixel modes

Terminal cells are taller than wide, so termpaper packs multiple canvas
pixels per cell:

- **quad** (default) — 2×2 px/cell via quadrant glyphs (`▘▝▖▗`…). Twice the
  horizontal resolution; colors use a luminance-weighted fg/bg split per cell.
- **half** — 1×2 px/cell via `▀`. The most color-true: each pixel keeps its
  exact RGB. Pick it if quad looks crushed on your terminal.
- **braille** — 2×4 px/cell via braille dot patterns. Highest density;
  reads almost like a bitmap, at the cost of per-cell color fidelity.

Glyph scenes (bump) keep their characters in every mode.

**macOS default profile**: on macOS, an unconfigured launch defaults to
`pixels = "half"` + `detail = "low"` (half-resolution, color-true pixels
and 0.5× particle counts) to stay cool on MacBook thermals/battery. The
scenes are unchanged — set `pixels`/`detail` explicitly (CLI, config, or
the menu) to override.

## Filters

Post-processing on the finished frame, composable and ordered:
`scanlines`, `vignette`, `grain` (animated), `warm`, `cool`, `hue`
(120° rotation), `crt` (scanlines + vignette + chromatic fringe),
`bloom` (bright-pass glow), `duotone` (luminance → black-to-accent
gradient), `pixelate` (3×3 mosaic), `chroma` (chromatic fringe alone),
`spectrum` (animated full-cycle hue rotation), `edges` (neon edge-glow),
`thermal` (false-color heat ramp), `warp` (animated sine wobble).

**Temporal smoothing** is on by default (`smooth = 0.3`): each frame blends
exponentially toward the last, taking the edge off stepping at any fps.
Glyph cells (bump) are never smoothed — text stays crisp. Set
`smooth = 0` in config to disable, or adjust live in Settings.

```sh
termpaper fire --filter crt
termpaper city --filter scanlines --filter vignette
```

## Controls (defaults — all remappable)

| Key     | Action                                  |
| ------- | --------------------------------------- |
| `q`     | quit (`Esc` / `Ctrl-C` always work too) |
| `?`     | settings menu                           |
| `←`/`→` | previous / next scene                   |
| `c`     | 100-step color wheel (hue/sat/contrast) |
| `f`     | cycle quick filter                      |
| `d`     | cycle detail level                      |
| `space` | pause (freeze animation)                |
| `0`     | reset all settings to defaults          |

## Config

`~/.config/termpaper/config.toml` — read at startup, **written back** by the
menu (atomically, temp + rename). Everything optional; CLI flags override.

```toml
scene = "rain"
pixels = "quad"           # half|quad|braille
detail = "medium"         # low|medium|high
filters = ["scanlines", "vignette"]
fps = 60
speed = 1.0
smooth = 0.3            # temporal smoothing strength, 0 disables
dim = 1.0               # global brightness 0.2-1.0 (dim art behind terminals)
fade = 0.25             # scene transition seconds, 0.1-1.0
hue_shift = 0           # global hue rotation 0-360°, 0 = off
saturation = 1.0        # 0 = grayscale, 2+ = vivid
contrast = 1.0          # 0.5-2.5, pivot at mid-gray
pad = 0                 # terminal padding px per side (wall alignment)
clock = true            # HH:MM overlay, top-right corner
text_scale = 2            # 1|2|3 -> bump font scale
# cycle = 30              # rotate scenes every N seconds

[themes]                  # per-scene remembered theme
nexus = "amber"
koi = "ink"

[keys]                    # single chars or: space esc tab enter left right up down
quit = "q"
menu = "?"
next = "right"
prev = "left"
filter_next = "f"
detail_next = "d"
pause = "space"
color = "c"
```

Unknown keys/actions warn but never fail.

## Scene Marketplace

Browse, install, and **publish community scenes on GitHub** — without merging into
the main repo yourself.

```sh
termpaper marketplace list
termpaper marketplace install Aphrodine-wq/example-pulse
termpaper pulse
termpaper marketplace publish          # checklist + PR link
```

- **Catalog:** [`marketplace/index.json`](marketplace/index.json) on GitHub
- **Template:** [`marketplace/template/`](marketplace/template/)
- **Docs:** [`marketplace/README.md`](marketplace/README.md)

Install clones a scene repo, saves it under `~/.local/share/termpaper/marketplace/`,
and rebuilds termpaper so the scene is compiled in. Press **`?` → Marketplace**
in any running scene for a quick summary.

Prebuilt `--binary` installs can browse the catalog; running community scenes
requires Rust and a source tree (`./install.sh` or `TERMPAPER_SRC`).

## Adding your own scenes

**Yes** — termpaper is built for this. Animations are **scenes**: Rust modules
that implement the `Scene` trait, paint into a truecolor canvas each frame, and
register in `src/scene/mod.rs`.

**Two paths:**

1. **Scene Marketplace (share on GitHub)** — copy [`marketplace/template/`](marketplace/template/),
   push to your repo, PR your entry into `marketplace/index.json`. Others install
   with `termpaper marketplace install your-user/your-scene`.

2. **Core catalog (PR to termpaper)** — fork, add `src/scene/your_scene.rs`,
   register in `mod.rs`, open a PR. Best for scenes you want shipped with every
   install.

There is no hot-load plugin folder — scenes are compiled in for speed and
type safety.

```sh
# marketplace path:
termpaper marketplace publish
termpaper marketplace install your-user/your-scene

# core catalog path:
cargo install --path . --locked
termpaper your_scene
```

Full recipe, trait API, themes, filters, and design bar:
[CONTRIBUTING.md](CONTRIBUTING.md) and [marketplace/README.md](marketplace/README.md).

## Scenes (47)

Ambient: **rain** · **starfield** · **fire** · **pipes** ·
**plasma** · **aurora** · **life** · **boids** ·
**lava** · **tunnel** · **dvd** · **bump** · **canopy** ·
**finale** · **ocean** · **circuits** · **clouds** · **mandel** ·
**meteors** · **koi** · **sand** · **city** · **abyss** ·
**den** · **traffic** · **nexus** · **ripple** · **fireflies** ·
**lanterns** · **frost** · **orbits** · **ribbons** · **sonar** ·
**tide** · **clockwork** · **grid** · **inkdrop** · **mosaic** ·
**harmonograph** · **nebula** · **pendulum** · **reaction** · **meadow** ·
**airspace** · **aquarium** · **drive** · **candy**
— run `termpaper --list` for one-line descriptions.

A dim clock (`HH:MM`) sits in the top-right corner of every scene —
toggle it in Settings → Clock.

Every scene has 2–4 named themes (`termpaper koi --theme ink`,
`termpaper fire --theme frost`, `termpaper city --theme noir`…); pick them
from the menu or set them per-scene in `[themes]`.

Highlights: **airspace** paints a realistic sky over rolling fields with
layered aircraft, contrails, and nav lights — eight themes from clear day
to storm and busy air traffic. **drive** is first-person night driving:
dashboard silhouette, scrolling lane markings, oncoming headlights, and
side scenery streaking past (distinct from aerial **traffic**). **aquarium**
is a side-view tank with caustic gravel, wandering fish, rising bubbles,
and feed/filter/shadow events. **candy** pushes saturation hard — glossy orbs
on a neon gradient. Press **c** for the 100-step color wheel (white center
hole, hue ring + saturation/contrast rows). Same controls in Settings. **orbits** runs true Kepler ellipses — eccentric anomaly solved
per frame — with comet-ribbon trails and eased syzygy transits. **frost**
grows fern-like dendrites that taper to glowing tips, then melts and
reseeds. **finale** fires 11 shell types (peony, willow, strobe,
double-ring, heart, waterfall…) to a backlit crowd filming on phones.
**koi** are anatomical — tapered bodies, flapping fins, undulating tails,
kohaku/sanke/showa/ogon breeds, shadows and wakes. **traffic** paints
long-exposure headlight/taillight streams over a readable overpass with
brake-wave congestion and a lane closure. **den** is a cozy room whose CRT
plays real other scenes, with a Fuji-style alpenglow mountain in the
window and a remote LED that blinks on channel changes.

## Design notes

The polish bar is set by the fireworks scene, and every scene is built to
the same recipe:

- **Depth** — scenes compose 2–3 planes: a restrained background, midground
  action, and a foreground layer (crowd silhouettes, out-of-focus petals or
  flakes drifting past the camera, near-camera glows).
- **Eased motion** — nothing moves linearly. Gravity, drag, smoothstep
  envelopes, sine breathing: tunnels drift rather than tick, the DVD logo
  squashes on impact, boids swirl and scatter.
- **Events** — instead of uniform loops, scenes run on an anticipation →
  payoff → decay rhythm: formation passes in airspace, bloom pulses in plasma,
  murmuration swirls in boids, power surges in circuits, sun-breaks in
  clouds, sandstorms, lightning on every horizon.
- **Lighting interplay** — bright events touch their surroundings: bursts
  rim the crowd, the TV flickers onto the den's floor, glow bleeds off pipe
  heads, jellyfish light the abyss.
- **Palette discipline** — backgrounds stay under ~25% luminance so the
  accents pop; gradients are hand-tuned, muddy mid-tones avoided.

## How it works

Each scene is a `Scene` trait implementation painting into a truecolor pixel
`Canvas` with delta-time animation. Element counts scale with terminal area
(`density_for`: 1× at 160×100, clamped 0.5–4×) stacked with the Detail
multiplier, so scenes fill any pane from 40×12 to 400×200. Falling things
move through a shared verlet physics module (gravity, drag, wind, soft
bounce, spring-damper steering). The canvas is filtered, then blitted
through ratatui in the active pixel mode; RGB is quantized to xterm-256 on
non-truecolor terminals. Resizes are picked up every frame. Every scene renders edge-to-edge: a
coverage test asserts <2% unpainted cells at any size — no borders, no
margins, no unused pixels.

## License

MIT — see [LICENSE](LICENSE). Contributions welcome: [CONTRIBUTING.md](CONTRIBUTING.md).
