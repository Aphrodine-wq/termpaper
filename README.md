# termpaper

**Wallpaper Engine for the terminal.** Forty-nine hand-animated truecolor worlds —
rain on glass, neon skylines, deep ocean, demoscene plasma — rendered live at up to
**120 fps** (240 max) in any pane that can paint 24-bit color. Stack **22 filters**,
remix **per-scene themes**, grade color on a 100-step wheel, remap every keybind,
span monitors as one seamless wall, or build your own scenes.

Your terminal never had to be boring. Tune everything.

```sh
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash && termpaper rain
```

One line to install. One word to start: `termpaper fire`.

```sh
termpaper rain --fps 120     # high-refresh rain on glass
termpaper life --speed 4     # crank the simulation
termpaper --list             # the catalog
termpaper                    # press ? — full command center
```

## Install

### Requirements

| What | Why |
|------|-----|
| **Truecolor terminal** | The full palette — `kitty`, `ghostty`, `alacritty`, `foot`, `wezterm`, … Check: `echo $COLORTERM` → `truecolor` or `24bit`. |
| **Rust toolchain** | Only if you're building from source ([rustup.rs](https://rustup.rs)). |

No truecolor? termpaper still runs — it auto-falls back to xterm-256.

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
termpaper life --fps 120    # high-refresh
termpaper                 # press ? for settings
```

**Screensaver / fullscreen:** any pane, tmux split, or dedicated window — go
big. Spanning monitors? See [The swarm](#the-swarm--instances-sync--multi-monitor-walls).

**Solo mode:** `termpaper --no-link fire` · **Wallpaper cluster:** `termpaper --group wallpaper rain`

## Make it yours

termpaper is built to be dialed in — live, from the keyboard, with every change
saved to `~/.config/termpaper/config.toml`.

| Layer | What you control | How |
|-------|------------------|-----|
| **Motion** | FPS (10→240, **120** preset), speed (0.25×→4×), pause | `?` Settings, `[`/`]` fps, `,`/`.` speed, `--fps 120` |
| **Look** | 3 pixel modes, 3 detail levels, 2–4 themes per scene | Settings rows + `--pixels` `--detail` `--theme` |
| **Color** | Global hue, saturation, contrast + 100-step wheel | `c` in-scene, Settings rows 3–5 |
| **Post** | 22 stackable filters + quick-preview on `f` | Settings filter list, `--filter` (repeatable) |
| **Feel** | Temporal smooth, dim, fade, clock overlay | Settings + config |
| **Layout** | Wall crop, terminal padding, link group, solo mode | `--wall` `--pad` `--group` `--no-link` |
| **Input** | 13 remappable actions | `[keys]` in config.toml |
| **Memory** | Per-scene theme, full config persistence | `[themes]` table, menu auto-save |

**FPS presets** (Settings ◂/▸ or `[` / `]`): `10 · 24 · 30 · 60 · 90 · **120** · 144 · 165 · 240`

**Speed presets** (Settings or `,` / `.`): `0.25 · 0.5 · 0.75 · 1 · 1.25 · 1.5 · 2 · 3 · 4`

**Example — a fully loaded profile:**

```sh
termpaper life \
  --fps 120 --speed 4 --detail low --pixels braille \
  --filter noir --filter sharpen --filter gamma --filter posterize \
  --filter grain --filter vignette \
  --group wallpaper
```

```toml
# ~/.config/termpaper/config.toml
scene = "life"
fps = 120
speed = 4.0
detail = "low"
pixels = "braille"
smooth = 0.5
fade = 0.1
clock = true
filters = ["noir", "sharpen", "gamma", "posterize", "grain", "vignette"]
group = "wallpaper"

[themes]
life = "ember"
nexus = "amber"

[keys]
quit = "q"
menu = "?"
fps_up = "]"
fps_down = "["
speed_up = "."
speed_down = ","
```

Every linked instance in your group can receive live setting changes — pixels,
filters, fps, color grade — without touching their config files.

termpaper ignores `NO_COLOR` — this is a color-art program. Force retro 256-color
with `--no-truecolor` if you want the fallback look.

## The menu — press `?`

A glass panel over the **live** scene — animation keeps running behind it.
Every tab opens with a fastfetch-style header: ASCII logo, version, scene,
display mode, link status. The **About** tab goes full cinema poster.

- **Scenes** — the catalog at your fingertips. `↑/↓` browse, `Enter` switch.
  `n of 49`.
- **Instances** — who's linked in your cluster right now (pid, scene, uptime).
  `(you)` marks this window. Know your swarm before you flip the whole wall.
- **Settings** — the full mixing desk: Pixels, Detail, Theme, Hue, Saturation,
  Contrast, **Link**, **Group**, Text scale, **Speed**, **FPS** (presets through
  **120**), Smooth, Dim, Fade, Clock, Cycle, and all 22 filters. ◂/▸ adjusts;
  everything persists to config.
- **Keybinds** — your control map.
- **About** — the full product page. Logo, stats, vibes.

`Esc` or `?` closes. The art never stops.

## Quick start

```sh
termpaper rain --fps 120    # high-refresh default vibe
termpaper --list            # the catalog
termpaper fire --theme frost
termpaper --cycle 30        # let it rotate
termpaper --no-link candy   # solo — off the swarm
```

Press **`?`** for the command center · **`[`/`]`** fps · **`,`/`.`** speed · **`q`** quit · **`0`** reset.

## Usage

```
termpaper [SCENE] [OPTIONS]

Arguments:
  [SCENE]               Scene to run (see --list)

Options:
      --list              List scenes and exit
      --cycle <SECS>      Rotate through all scenes every N seconds
      --fps <N>           Target FPS — 10–240. Classic scenes present at the
                          highest divisor of their 60 Hz tick (60/30/20/15...),
                          Studio scenes at up to shader_fps (60)
      --idle-fps <N>      FPS cap while the terminal is unfocused (off unless set;
                          needs a terminal that reports focus)
      --speed <MULT>      Animation speed multiplier [default: 1.0]
      --theme <NAME>      Scene color theme (per scene; e.g. fire: classic|frost|inferno)
      --detail <LEVEL>    low|medium|high — particle/layer/emitter counts
      --pixels <MODE>     half|quad|braille
      --text-scale <N>    1|2|3 — bump font scale
      --filter <NAME>     Repeatable: scanlines, vignette, grain, warm, cool, hue,
                          crt, bloom, duotone, pixelate, chroma
      --no-truecolor      Force 256-color output
      --screensaver       Any key exits
      --no-link             Unlink this terminal — solo art, no sync
      --group <NAME>        Link group (instances in the same group sync)
      --instances           List every live termpaper (all groups) and exit
      --switch <SCENE>      Publish a scene switch to linked instances and exit
      --all-groups          With --switch, publish to every link group
      --wall <COLSxROWS:IDX> Manual video-wall tile (e.g. 2x1:0)
      --no-wall             Never join a video wall (local canvas, geometry hidden from peers)
      --pad <PAD>         Terminal padding so wall crops line up across window
                          borders: px (7), points (3.5pt), or x,y
  -h, --help              Print help
```

## The swarm — instances, sync & multi-monitor walls

Every running termpaper on your machine registers itself — a local swarm with
no network, no daemon. Link them into sync clusters, cut one loose for solo
viewing, or tile them into a **video wall** so one animation flows across
monitors like a single panoramic canvas.

### Every terminal, accounted for

Each instance checks into a file-based registry:

| Location | Group |
|----------|-------|
| `$XDG_RUNTIME_DIR/termpaper/` | `default` |
| `$XDG_RUNTIME_DIR/termpaper/groups/<name>/` | named groups |

Every instance writes `inst-<pid>.json` with its **pid**, **scene**, **link
group**, terminal size, **window geometry** (Hyprland via `hyprctl` when
available), and **padding** (for wall alignment). Stale entries are reaped
Stale entries vanish when a process exits. No ghosts in the swarm.

**Roll call:**

```sh
termpaper --instances
# pid 12345    group wallpaper   scene rain       up 842s
# pid 12389    group wallpaper   scene rain       up 841s
# pid 12401    group desk        scene candy      up 12s
```

Or **`?` → Instances** — live peers in your cluster, `(you)` on this window.

Spin up a second monitor? It adopts the group's anchor (`anchor.json`: scene,
seed, start time, pause state, speed, detail, pixels, text scale, theme) and
replays toward the shared frame out of sight. If that replay would take more
than a few seconds, the group restarts the scene together after a fade.

### Sync clusters

Same link group = same clock:

- Scene switches (`←` / `→`, menu, `termpaper --switch`) — one command, every
  window, fading out and in at the same moment
- Sim settings — speed, detail, pixels, text scale, theme — travel in the
  anchor, so panes started with different flags converge
- Live appearance settings — filters, fps, smooth, dim, fade, color grade, `f`
  quick-filter preview
- Pause (`space`) freezes the whole wall; resuming continues where it stopped
- **Frame lock** — identical animation state, presented on the same frame
  slots, like one wallpaper torn across panes

`--cycle` auto-rotation stays local. Your desk doesn't have to follow your wall.

**Presets** (Settings → Group): `default`, `wallpaper`, `desk`, `art`. Split
clusters so your work terminal and your wallpaper rig live separate lives:

```sh
termpaper --group wallpaper rain    # monitor 1
termpaper --group wallpaper rain    # monitor 2 — stays in sync
termpaper --group desk candy        # independent cluster
```

Remote tweaks are **session-only** — nobody's config file gets overwritten.
The anchor persists as long as any member of the cluster lives, so there is no
heartbeat. The lowest-pid instance leads: it answers restart requests and
retimes the cluster after a system suspend.

### Go solo

Any instance can **leave the swarm** without killing the art:

| Method | When | Persists? |
|--------|------|-----------|
| `termpaper --no-link` | Launch | yes (`link = false` in config) |
| **`?` → Settings → Link → off** | Runtime | yes |
| `termpaper --group desk` | Launch | yes — different cluster, not unlinked but isolated |

When unlinked, **Instances** reads `linking disabled (solo art)`. This window
dances alone — no publishes in, no orders out.

Rejoin the swarm: **Link → on** in Settings, or relaunch with
`termpaper --group wallpaper`.

**Command the fleet** from anywhere:

```sh
termpaper --switch fire                 # default group only
termpaper --switch fire --group wallpaper
termpaper --switch fire --all-groups
```

### One canvas. Every monitor.

When linked instances know where they sit on screen, they become **windows
into one giant virtual canvas** — meteors streak from the left bezel into
the right, rain falls through the gap, one uninterrupted picture.

**Hyprland:** geometry comes from `hyprctl`. Same group, different monitors — done:

```sh
termpaper --group wallpaper rain   # left monitor
termpaper --group wallpaper rain   # right monitor
```

**Manual grid** (any compositor, any layout):

```sh
termpaper --wall 2x1:0 --group wallpaper rain   # left half of a 2×1 wall
termpaper --wall 2x1:1 --group wallpaper rain   # right half
termpaper --no-wall                             # local canvas only, never folded into a wall
```

**Padding:** kitty/alacritty window margins? Set `pad = 7` (px), `pad = "3.5pt"`
(kitty's `window_padding_width` is in points) or `pad = [x, y]`, plus
`placement = "center"` if your terminal centres its grid (default `top-left`),
so crops line up at the text edge, not the window chrome. Every instance
publishes its inset and its measured cell size; the wall math handles the rest.

Shared seed + geometry = **one continuous frame** across the grid. If the
virtual canvas gets too huge (~3× standard area), instances gracefully fall
back to local mode.

### Fleet commands

```sh
termpaper --instances              # list all groups (pid, group, scene, uptime)
termpaper --no-link                # unlink — solo art
termpaper --group wallpaper        # join / isolate a sync cluster
termpaper --switch fire --group wallpaper
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
timestamp. Linked scenes use fixed 60Hz simulation steps independent of
display FPS, with bounded catch-up work per frame and no 30-second replay
cutoff. Wall-size changes rebuild from the seed at the final shared size,
so monitor discovery order does not permanently change the artwork. Filters
share the scene clock, and trail smoothing adjusts to each display's frame
duration. Matching scene options and virtual canvas sizes are required for
matching simulation frames; terminal presentation is not hardware frame-locked.
Speed/detail changes reconstruct the shared simulation and may briefly replay.
Control messages are totally ordered
(millisecond epoch + per-publisher sequence, ties by pid), so rapid scene
flipping never drops a switch.

## Pixel modes

Terminal cells are tall — termpaper packs extra resolution into each one:

- **quad** (default) — 2×2 px/cell via quadrant glyphs (`▘▝▖▗`…). Sharp horizontal
  detail; luminance-weighted fg/bg split per cell.
- **half** — 1×2 px/cell via `▀`. Purist mode: every pixel keeps its exact RGB.
- **braille** — 2×4 px/cell. Maximum density — almost bitmap, with a little
  color compromise per cell.

Glyph scenes (**bump**) stay typographic in every mode.

**macOS default profile**: on macOS, an unconfigured launch defaults to
`pixels = "half"` + `detail = "low"` (half-resolution, color-true pixels
and 0.5× particle counts) to stay cool on MacBook thermals/battery. The
scenes are unchanged — set `pixels`/`detail` explicitly (CLI, config, or
the menu) to override.

## Filters

Cinematic post on every frame — **22 filters**, stack them, order matters:
`scanlines`, `vignette`, `grain`, `warm`, `cool`, `hue`, `crt`, `bloom`,
`duotone`, `pixelate`, `chroma`, `spectrum`, `edges`, `thermal`, `warp`,
`invert`, `sepia`, `posterize`, `gamma`, `sharpen`, `mirror`, `noir`.
Toggle any from **`?` → Settings**; preview one live with **`f`**.

**Temporal smoothing** (`smooth = 0.3` by default) blends each frame toward
the last — silk at any fps. Glyph cells never blur. Kill it with `smooth = 0`
or dial it in live from Settings.

```sh
termpaper fire --filter crt
termpaper city --filter scanlines --filter vignette
```

## GPU post-processing (optional)

Build with `gpu` to run filters, colour grading, smoothing and terminal-cell
packing on Vulkan. Scenes always run on the CPU — they are the artwork — and
the worker uploads each frame for the GPU to finish. `auto` uses the GPU when
compiled and available; `gpu` does the same but reports when the GPU is
missing; `cpu` keeps everything on the CPU. `--gpu` remains an alias for
`--renderer gpu`.

```sh
cargo install termpaper --features gpu
termpaper scroll --renderer auto --fps 120
termpaper plasma --renderer gpu --pixels braille --filter bloom
termpaper scroll --renderer cpu
```

Open `?` to see the active path ("GPU post · adapter · CPU scene") or the CPU
fallback reason. Builds without the feature still work on the CPU. Device and
validation errors trigger fallback.

Scene work runs in a separate worker with a single latest-request mailbox.
Switches invalidate old frames; full GPU readback queues skip submissions
instead of waiting on the input thread.

**GPU worlds (experimental).** `src/gpu/shaders/world.wgsl` carries a shader
re-interpretation of the catalog. A scene listed in `scene::GPU_WORLD_SCENES`
is drawn from its WGSL arm instead of its Rust code; the list is empty by
default. `--renderer shader` forces the shader arm for every scene (Bump keeps
its CPU text layout). Fire, Life, Boids, Sand and Reaction there use fixed-step
simulation on a resolution-independent 192×192 lattice. The worlds are new
interpretations, not pixel-identical ports; use the same backend, seed and
settings across linked monitors.

The default target is 120 FPS, with 144 available in Settings. This is a cap,
not a guarantee: terminal escape-sequence throughput can still limit display
rate. The worker timing in the menu is CPU submission time, not GPU timing or
measured display FPS. For hardware measurements and scene image captures
(`--world` reviews the shader arms):

```sh
cargo run --release --features gpu --example world_review -- all /tmp
cargo run --release --features gpu --example world_review -- all /tmp --world
cargo test --release --features gpu --test gpu_world -- --ignored
```

**Fidelity.** `gpu_bench` checks the shaders against the CPU functions in two
stages: every filter must land within one 8-bit step per channel (15 of 21 are
bit-exact; the rest differ by one because the CPU truncates between stages and
RADV contracts multiply-adds), and every glyph that differs in quad/braille must
be a pixel that sat within 20 luminance units of its block's split threshold —
a coin flip that was always going to land either way. `grain` is deliberately
different: the CPU pulls from a seeded RNG in raster order, the shader uses a
positional hash. Same range, same per-frame determinism, different noise.

## Controls (defaults — all remappable in `[keys]`)

| Key     | Action                                  |
| ------- | --------------------------------------- |
| `q`     | quit (`Esc` / `Ctrl-C` always work too) |
| `?`     | settings menu — full mixing desk        |
| `←`/`→` | previous / next scene                   |
| `c`     | 100-step color wheel (hue/sat/contrast) |
| `f`     | cycle quick filter preview              |
| `d`     | cycle detail level                      |
| `[`/`]` | fps down / up (through presets incl. **120**) |
| `,`/`.` | speed down / up                         |
| `space` | pause (freeze animation)                |
| `0`     | reset all settings to defaults          |

## Config

`~/.config/termpaper/config.toml` — read at startup, **written back** by the
menu (atomically, temp + rename). Everything optional; CLI flags override.

```toml
scene = "life"
fps = 120                 # 10–240; Classic scenes snap to 60/30/20/15..., Studio cap at shader_fps
speed = 4.0               # 0.25–4.0 animation multiplier
pixels = "braille"        # half|quad|braille
detail = "low"            # low|medium|high — particle counts
filters = ["noir", "sharpen", "gamma", "posterize", "grain", "vignette"]
smooth = 0.5            # temporal smoothing, 0 = off
dim = 1.0               # global brightness 0.2–1.0
fade = 0.25             # scene transition seconds
hue_shift = 0           # global hue 0–360°
saturation = 1.0        # 0 = grayscale, 2+ = vivid
contrast = 1.0          # 0.5–2.5
pad = 0                 # terminal padding: px, "3.5pt", or [x, y] (wall alignment)
placement = "top-left"  # top-left|center — where the terminal puts leftover space
hysteresis = 3          # Studio scenes: don't resend cells that moved <= 3 levels
link = true             # false = solo art
group = "wallpaper"     # sync cluster
wall = true             # false = never crop into a video wall
clock = true
text_scale = 2          # 1|2|3 — bump scene only
# cycle = 30            # auto-rotate scenes (local only)

[themes]
life = "ember"
nexus = "amber"

[keys]
quit = "q"
menu = "?"
fps_up = "]"
fps_down = "["
speed_up = "."
speed_down = ","
filter_next = "f"
color = "c"
reset = "0"
```

Unknown keys/actions warn but never fail.

## Build your own

termpaper is a canvas for terminal artists. A **scene** is a Rust module
implementing the `Scene` trait — full-frame truecolor, every tick.

**The lane in:** PR to termpaper itself — your scene ships with every install.

Scenes compile in — no plugins, no runtime overhead. Pure speed, pure type safety.

```sh
cargo install --path . --locked
termpaper your_scene
```

Full recipe, trait API, themes, filters, and design bar:
[CONTRIBUTING.md](CONTRIBUTING.md).

## Scenes (49)

The catalog — one word each, full descriptions in `termpaper --list`:

**rain** · **starfield** · **fire** · **pipes** · **plasma** · **aurora** · **life** · **boids** ·
**lava** · **tunnel** · **dvd** · **bump** · **canopy** ·
**finale** · **ocean** · **circuits** · **clouds** · **mandel** ·
**meteors** · **koi** · **sand** · **city** · **abyss** ·
**den** · **traffic** · **nexus** · **ripple** · **fireflies** ·
**lanterns** · **incense** · **frost** · **orbits** · **ribbons** · **sonar** ·
**tide** · **clockwork** · **grid** · **inkdrop** · **mosaic** ·
**harmonograph** · **nebula** · **pendulum** · **reaction** · **meadow** ·
**airspace** · **aquarium** · **drive** · **candy** · **scroll**
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
and feed/filter/shadow events. **candy** pushes saturation hard — glossy
specular orbs on a neon gradient, hues drawn from two tight anchor families
rather than at random, across three graded depth planes, with a sugar-rush
wavefront that flares each orb as it sweeps past. Press **c** for the 100-step color wheel (white center
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

## The craft

Every scene meets the same bar — set by **finale** and held across the catalog:

- **Depth** — background, midground, foreground. Crowd silhouettes, drifting
  petals, near-camera glow. Layers you feel, not flat loops.
- **Eased motion** — gravity, drag, smoothstep, sine breath. Tunnels drift;
  the DVD logo squashes on impact; boids murmurate.
- **Events** — anticipation → payoff → decay. Formation passes, plasma blooms,
  circuit surges, cloud sun-breaks, sandstorms, horizon lightning.
- **Lighting** — bright moments touch the world around them. Bursts rim the
  crowd; the den's TV washes the floor; pipe heads glow; jellyfish light the abyss.
- **Palette discipline** — backgrounds stay dark (~25% luminance) so accents
  hit hard. Hand-tuned gradients. No muddy mid-tones.

## Under the hood

Every scene implements `Scene` — a truecolor pixel canvas, delta-time animation,
edge-to-edge coverage. Density scales with terminal area (`density_for`: 1× at
160×100, clamped 0.5–4×) plus Detail multiplier, so art fills anything from a
phone pane to an ultrawide. Physics (gravity, drag, springs) lives in a shared
module. Filters hit the frame, ratatui blits in your pixel mode, non-truecolor
terminals get xterm-256 quantization — or the whole post chain runs in compute
shaders instead, see **GPU post-processing**. Resizes every frame. Coverage test:
<2% unpainted cells at any size — no borders, no dead pixels.

## License

MIT — take it, remix it, ship scenes. [CONTRIBUTING.md](CONTRIBUTING.md) for the craft.
