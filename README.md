# termpaper

**Wallpaper Engine for the terminal.** Real places, rendered live on your GPU —
Big Sur at sunset, a Shinjuku alley in the rain, the aurora over Tromsø, a
cabin fireplace while it snows outside — in any terminal that paints 24-bit
colour. **41 Studio scenes** are ray-marched, physically lit WGSL shaders
with 3–4 time-of-day and weather themes each; the **51 Classic** hand-animated
CPU scenes are still here. Run one terminal per monitor and they become
**one continuous picture across every screen**, portrait and landscape
lined up in millimetres, frames presented in lockstep.

```sh
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash && termpaper tokyo
```

```sh
termpaper bigsur                 # a Studio scene (needs a Vulkan GPU)
termpaper tokyo --theme snow     # every scene has themes
termpaper list                   # the catalog, by category
termpaper wall up                # one terminal per monitor, one picture
termpaper                        # press ? — browse, preview, tune
```

## Studio scenes

Each Studio scene is one stateless WGSL shader: every pixel is a pure function
of where it is and what time it is. That is what makes them cheap to sync — a
terminal on the left monitor and one on the right render their own halves of
the same frame from the same clock, with nothing to replay and nothing to
drift. They are supersampled (up to 24 samples per pixel, adapted to a GPU
time budget), tonemapped with AgX, and dithered once, statically, so they
cost the terminal as little bandwidth as possible.

<!-- studio-catalog:begin -->
**Coast & Water** (8)

| scene | | themes |
|---|---|---|
| `bigsur` | Big Sur Coast — headlands of the Big Sur coast dropping into Pacific swells as marine fog rolls in | sunset · fog · noon · moonlight |
| `harbor` | Fishing Harbor at Dawn — lobster boats riding at their moorings in a misty cove of fish shacks and a wooden pier | dawn · dusk · night |
| `icebergs` | Jökulsárlón — blue icebergs drift in a glacier lagoon below Vatnajökull, ice glinting on black sand | day · twilight · aurora |
| `jellyfish` | Moon Jelly Gallery — translucent moon jellies pulse and drift through the blue of an aquarium gallery tank | blue · sunset · deep |
| `kelp` | Monterey Kelp Forest — looking up through swaying giant kelp as sun shafts pour down and a school of fish mills | sunlit · deep · twilight |
| `lagoon` | Bora Bora Lagoon — turquoise shallows off an overwater-bungalow deck, Mount Otemanu rising across the lagoon | noon · golden · night |
| `lighthouse` | Lighthouse in the Storm — a Maine lighthouse on a granite headland, its beam sweeping through rain, spray and storm | storm · fog · dusk |
| `waterfall` | Skógafoss — a 60 m curtain of water pours off a mossy basalt cliff into mist, a rainbow in the spray | summer · winter · midnight |

**Mountains & Wild** (11)

| scene | | themes |
|---|---|---|
| `autumn` | Maple Lake — red and gold maples around a still lake, mist on the water, leaves drifting down | morning · golden · rain |
| `bamboo` | Arashiyama Bamboo — the path through Kyoto's bamboo grove, tall culms swaying and meeting overhead | day · rain · lantern |
| `dolomites` | Dolomites Alpenglow — the Tre Cime towers above green alpine meadows and a lone hut as cloud drifts past | alpenglow · midday · starry |
| `dunes` | Sahara Dunes — knife-edged Saharan dunes, sand streaming off the crests, a camel caravan crossing | golden · noon · moonlit |
| `fjord` | Norwegian Fjord — sheer walls dropping into a still green fjord as a ferry draws its wake past the falls | summer · winter · overcast |
| `kilauea` | Kilauea Lava — a lava channel crossing black pahoehoe to the sea, the steam plume glowing above the surf | night · dusk · eruption |
| `mesa` | Monument Valley — the Mittens and Merrick Butte rising from red sand, a dirt road leading in | sunset · noon · night |
| `redwoods` | Redwood Fog — shafts of morning sun slanting through fog between old-growth redwood trunks | morning · overcast · dusk |
| `sakura` | Cherry Blossom Canal — cherry trees arching over a Meguro canal, petals drifting onto the water below | day · night · rain |
| `tuscany` | Tuscan Hills — rolling Val d'Orcia hills, a cypress-lined road up to a farmhouse, fog in the valleys | dawn · summer · autumn |
| `yosemite` | Yosemite Valley — El Capitan, Half Dome and Bridalveil Fall from Tunnel View as cloud drifts up the valley | morning · sunset · winter |

**Weather & Sky** (7)

| scene | | themes |
|---|---|---|
| `cloudsea` | Above the Cloud Sea — sunrise from a rocky summit over a sea of cloud, far peaks standing out like islands | sunrise · sunset · moon |
| `eclipse` | Total Solar Eclipse — totality over open country: the corona, the diamond ring, a sunset on every horizon | totality · desert · mountain |
| `goldengate` | Fog over the Golden Gate — the Golden Gate's towers rising out of a rolling fog bank, the city faint beyond | morning · sunset · night |
| `milkyway` | Milky Way over Joshua Tree — the galactic core rising over Joshua trees and granite boulders in the Mojave | summer · winter · moonrise |
| `snowfall` | Snowfall in the Pines — an old lamp by a footpath in a snowy pine wood, its warm cone full of falling snow | night · bluehour · blizzard |
| `supercell` | Great Plains Supercell — a rotating supercell towers over golden wheat, rain shaft and wall cloud, a lone farm | afternoon · dusk · night |
| `windowrain` | Rain on the Window — a rain-streaked window at night, the wet city street beyond melted into bokeh | city · dusk · neon |

**Cities & Streets** (10)

| scene | | themes |
|---|---|---|
| `freeway` | LA Freeway Timelapse — long-exposure light trails on a curving LA freeway, downtown towers glowing in the haze | dusk · night · rain |
| `havana` | Havana Malecón — waves bursting over Havana's seawall at sunset, 1950s cars under faded colonial arcades | sunset · day · storm |
| `hongkong` | Victoria Harbour — Hong Kong Island's towers across Victoria Harbour, lights streaking on the water | night · bluehour · fog |
| `manhattan` | Manhattan Rooftops — wooden water towers over Chelsea rooftops as dusk settles on the Midtown skyline | dusk · night · snow |
| `mongkok` | Mong Kok Neon — neon signs stacked over a wet Mong Kok street, red taxis and buses passing below | rain · night · fog |
| `paris` | Paris Café Street — a Haussmann street running to the Eiffel Tower, a café glowing under its awning | rain · autumn · night |
| `shibuya` | Shibuya Scramble — the Shibuya scramble from above: crowds surge across every stripe when the lights change | rain · night · day |
| `tokyo` | Shinjuku Alley in the Rain — a narrow Shinjuku yokocho at night: stacked signs, red lanterns, wet asphalt mirroring it | rain · clear · snow |
| `trainwindow` | Train Window — golden-hour countryside streaming past a train window, poles flicking by, the sun low | golden · night · snow |
| `venice` | Venice Canal — a narrow Venetian rio between weathered palazzi, a gondola drifting under a stone bridge | morning · sunset · night |

**Cozy & Interiors** (7)

| scene | | themes |
|---|---|---|
| `cafe` | Coffee Shop Window — a steaming cup at a cafe window, the rainy street outside melting into bokeh | rain · snow · morning |
| `fireplace` | Cabin Fireplace — a crackling fire in a fieldstone hearth, snow falling past the cabin window | snow · rain · autumn |
| `library` | Candlelit Library — an old reading room: towering shelves, a green banker's lamp, a candle, dust in the air | candle · dawn · storm |
| `lofi` | Late Night Desk — a desk lamp and a laptop's glow, a cat asleep by a rainy window over the city | rain · snow · dawn |
| `onsen` | Mountain Onsen — an outdoor hot spring steaming among snowy rocks, stone lanterns glowing by the pines | snow · night · autumn |
| `porch` | Summer Storm Porch — a screened Southern porch at dusk: rain off the eaves, a rocking chair, fireflies after | storm · fireflies · night |
| `ramen` | Ramen Counter — a bowl steaming on a ramen counter under paper lanterns, rain beyond the noren | rain · night · snow |

**Space** (7)

| scene | | themes |
|---|---|---|
| `blackhole` | Black Hole — a black hole's accretion disk bent around its shadow, one side Doppler-bright | amber · blue · edge-on |
| `earthrise` | Earthrise — the Earth hanging over the lunar horizon, craters stretching long shadows below | apollo · crescent · eclipse |
| `iss` | Earth from the ISS — the curved limb of the Earth from orbit: city lights, clouds, airglow, an orbital sunrise | night · sunrise · day |
| `jupiter` | Jupiter — Jupiter's banded storms and the Great Red Spot, Io's shadow crossing the clouds | voyager · juno · aurora |
| `moonrise` | Moonrise over the Ocean — a huge orange moon lifting off the sea, its glittering path running in to a dark shore | harvest · full · crescent |
| `saturn` | Cassini at Saturn — Saturn and its rings from Cassini: ring shadows on the clouds, a moon on its orbit | sunlit · backlit · equinox |
| `tromso` | Aurora over Tromsø — green aurora curtains rippling over a snowy Norwegian fjord and its village lights | green · vivid · faint |

<!-- studio-catalog:end -->

No GPU? Studio scenes fall back to a related Classic scene, and linked
terminals without a GPU still stay in sync with each other.

## Install

### Requirements

| What | Why |
|------|-----|
| **Truecolor terminal** | The full palette — `kitty`, `ghostty`, `alacritty`, `foot`, `wezterm`, … Check: `echo $COLORTERM` → `truecolor` or `24bit`. |
| **A Vulkan GPU** | For the Studio scenes (any recent AMD, Intel or NVIDIA driver; `vulkan-icd-loader`). Without one, Studio scenes show a Classic fallback. |
| **Hyprland** *(optional)* | For the physical multi-monitor wall, `termpaper desk`, `wall up` and `calibrate`. Everything else runs anywhere. |
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
termpaper list            # the catalog, by category
termpaper rain            # default scene
termpaper life --fps 120    # high-refresh
termpaper                 # press ? for the menu
```

**Screensaver / fullscreen:** any pane, tmux split, or dedicated window — go
big. Spanning monitors? See [The swarm](#the-swarm--instances-sync--multi-monitor-walls).

**Solo mode:** `termpaper --no-link fire` · **Wallpaper cluster:** `termpaper --group wallpaper rain`

## Make it yours

termpaper is built to be dialed in — live, from the keyboard, with every change
saved to `~/.config/termpaper/config.toml`.

| Layer | What you control | How |
|-------|------------------|-----|
| **Motion** | FPS (10→240, **120** preset), speed (0.25×→4×), pause | `?` → Playback / Display, `[`/`]` fps, `,`/`.` speed, `--fps 120` |
| **Look** | 3 pixel modes, 3 quality levels, 2–4 themes per scene | `?` → Look / Display + `--pixels` `--detail` `--theme` |
| **Color** | Global hue, saturation, contrast on a 100-step wheel | `c` in-scene, `?` → Look → Color grade… |
| **Post** | 22 stackable filters, 4 presets, quick-preview on `f` | `?` → Look → Filters…, `--filter` (repeatable) |
| **Feel** | Temporal smooth, dim, fade, clock overlay | `?` menu + config |
| **Layout** | Wall crop, terminal padding, link group, solo mode | `--wall` `--pad` `--group` `--no-link` |
| **Input** | 13 remappable actions | `[keys]` in config.toml |
| **Memory** | Per-scene theme, favourites, recents | `[themes]`, `favorites`, menu auto-save |

**FPS presets** (`?` → Display ◂/▸ or `[` / `]`): `10 · 24 · 30 · 60 · 90 · **120** · 144 · 165 · 240`

**Speed presets** (`?` → Playback or `,` / `.`): `0.25 · 0.5 · 0.75 · 1 · 1.25 · 1.5 · 2 · 3 · 4`

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

A drawer slides in on the left; the **live** scene keeps playing on the right
(and dimly through the glass). Five pages — `Tab` / `Shift-Tab` switch:

- **Scenes** — the browser. Categories on the left (★ Favorites, ◷ Recent,
  Coast & Water, Mountains & Wild, … and Classic with its sub-groups), their
  scenes on the right, and a detail panel with description, themes, tags and a
  GPU badge. Rest on a scene for a moment and it **previews** behind the drawer
  on this pane only; `Enter` keeps it (and switches your linked group), `Esc`
  puts the original back. `/` filters by name, description and tags, `f` stars
  a scene, `t` cycles its theme.
- **Look** — Theme, Color grade… (the colour wheel), Filters… (grouped Colour /
  Texture / Geometry, Clean · Film · CRT · Dream presets, the active stack in
  order), Dim, Text size.
- **Playback** — Speed, Cycle (off / 1 / 5 / 15 / 30 min), Cycle through
  (all scenes / this category / favourites), Fade.
- **Display** — Quality, Pixels, FPS, Smooth, Clock, Renderer.
- **Wall** — Link, Group, Wall mode, Align monitors…, and the live instances in
  your group (`(you)` marks this window).

`↑`/`↓` move, `←`/`→` change a value (or switch column in the browser), `Enter`
toggles or opens, `Esc` backs out and closes. `?` inside the menu lists every
key plus version and renderer info. The footer only shows keys that do
something right now. Changes are saved a moment after you stop pressing keys —
and only the ones that differ from the defaults. The art never stops.

## Quick start

```sh
termpaper rain --fps 120    # high-refresh default vibe
termpaper list              # the catalog
termpaper fire --theme frost
termpaper --cycle 30        # let it rotate
termpaper --no-link candy   # solo — off the swarm
```

Press **`?`** for the command center · **`[`/`]`** fps · **`,`/`.`** speed · **`q`** quit · **`0`** reset.

## Usage

```
Usage: termpaper [OPTIONS] [SCENE] [COMMAND]

Commands:
  list       List scenes by category and exit   (--category coast|wilds|…|classic)
  instances  List live termpaper instances in every group and exit
  switch     Switch running instances to a scene and exit   (--group G | --all)

Scene:
  [SCENE]              Scene to run (see `termpaper list`)
      --theme <THEME>  Scene color theme (e.g. nexus: cyan/amber/violet/mono)
      --cycle <SECS>   Move on to another scene every N seconds
      --speed <SPEED>  Animation speed multiplier
      --screensaver    Screensaver mode: any key exits

Look:
      --filter <FILTER>          Post-processing filter, repeatable
      --pixels <PIXELS>          Pixel mode: half, quad or braille
      --text-scale <TEXT_SCALE>  Bump text scale: 1, 2 or 3
      --no-truecolor             Force 256-color output even on truecolor terminals

Performance:
      --fps <FPS>            Target frames per second (Classic scenes snap to a divisor
                             of their 60 Hz tick; Studio scenes cap at shader_fps)
      --idle-fps <IDLE_FPS>  Throttle while the terminal is unfocused
      --detail <DETAIL>      Quality: low, medium or high (particle/layer counts)
      --renderer <RENDERER>  auto, gpu, cpu or shader

Linking & wall:
      --link           Enable instance linking (default on)
      --no-link        Disable instance linking
      --group <GROUP>  Link group for this instance (instances in the same group sync)
      --no-wall        Never join a video wall
      --wall <WALL>    Manual video-wall tiling: COLSxROWS:INDEX (e.g. 2x1:0)
      --pad <PAD>      Terminal padding so wall crops line up across windows:
                       px (7), points (3.5pt, like kitty's window_padding_width), or x,y
```

The pre-subcommand spellings (`--list`, `--instances`, `--switch`,
`--all-groups`, `--gpu`) still work; they are just no longer listed.

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
termpaper instances
# pid 12345    group wallpaper   scene rain       up 842s
# pid 12389    group wallpaper   scene rain       up 841s
# pid 12401    group desk        scene candy      up 12s
```

Or **`?` → Wall** — live peers in your cluster, `(you)` on this window.

Spin up a second monitor? It adopts the group's anchor (`anchor.json`: scene,
seed, start time, pause state, speed, detail, pixels, text scale, theme) and
replays toward the shared frame out of sight. If that replay would take more
than a few seconds, the group restarts the scene together after a fade.

### Sync clusters

Same link group = same clock:

- Scene switches (`←` / `→`, menu, `termpaper switch`) — one command, every
  window, fading out and in at the same moment
- Sim settings — speed, detail, pixels, text scale, theme — travel in the
  anchor, so panes started with different flags converge
- Live appearance settings — filters, fps, smooth, dim, fade, color grade, `f`
  quick-filter preview
- Pause (`space`) freezes the whole wall; resuming continues where it stopped
- **Frame lock** — identical animation state, presented on the same frame
  slots, like one wallpaper torn across panes

`--cycle` auto-rotation stays local. Your desk doesn't have to follow your wall.

**Presets** (`?` → Wall → Group): `default`, `wallpaper`, `desk`, `art`. Split
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
| `termpaper --no-link` | Launch | no — this launch only (`link = false` in config makes it stick) |
| **`?` → Wall → Link → off** | Runtime | yes |
| `termpaper --group desk` | Launch | no — different cluster for this launch, not unlinked but isolated |

When unlinked, the Wall page's instance list reads `linking disabled (solo
art)`. This window dances alone — no publishes in, no orders out.

Rejoin the swarm: **Link → on** on the menu's Wall page, or relaunch with
`termpaper --group wallpaper`.

**Command the fleet** from anywhere:

```sh
termpaper switch fire                   # default group only
termpaper switch fire --group wallpaper
termpaper switch fire --all
```

### One canvas. Every monitor.

On Hyprland, linked terminals become windows onto one picture that spans
the desk. termpaper reads every monitor's size in millimetres, rotation and
scale from the compositor, lays them out left to right as they sit on the
desk (centred on one line by default), and one terminal — the group's leader —
publishes a **wall plan**: where each terminal's grid of cells sits, in
millimetres. Every pane adopts it verbatim.

- **Studio scenes** map each pane straight into the scene's coordinates, so a
  horizon crosses from a landscape monitor onto a rotated portrait one at the
  same physical height, whatever the fonts or pixel pitch. The portrait
  screen simply sees more sky above and more foreground below.
- **Classic scenes** share one canvas at a common cell pitch; a pane whose
  cells are a different size resamples it with a box filter.

```sh
termpaper wall up              # a kitty per monitor, fonts matched to pixel pitch
termpaper wall up --dry-run    # show the commands first
termpaper desk                 # monitors in mm, and the current plan
termpaper calibrate            # line the screens up (see below)
termpaper wall down
```

`wall up` scales each terminal's font by its monitor's pixel pitch so cells
come out the same physical size everywhere, forces zero padding, and classes
the windows `termpaper-wallpaper-<OUTPUT>` — point the hyprwinwrap plugin at
that pattern and they become a live desktop background.

**Calibrate.** Monitors on arms rarely sit exactly where the compositor
thinks. `termpaper calibrate` (or `?` → Wall → Align monitors…) switches every
pane to a millimetre test pattern — a 10 mm grid, level lines across the desk,
diagonals and a circle crossing each seam, a 100 mm ruler per screen. Nudge
the selected monitor with the arrows (Shift = 10 mm, Alt = 0.2 mm) until the
lines run straight, `[`/`]` for bezel width, `-`/`=` if a real ruler disagrees
with the bar, `Enter` to save. It goes to `~/.config/termpaper/desk.toml`,
shared by every termpaper on the machine:

```toml
align = "center"        # center|top|bottom|hypr
bezel_mm = 0.0          # >0 hides the art behind the bezels, like a window
frame = "row"           # the picture's frame: the landscape row, "all", or "monitor:NAME"
portrait = "extend"     # Classic scenes on portrait screens: extend|band|separate

[monitors."DP-1"]
offset_mm = [0.0, -12.0]
size_mm = [336.0, 597.0]   # only if the EDID size is missing or wrong
```

**Without Hyprland** the older cell-count wall still works: same group on
each monitor, or a manual grid:

```sh
termpaper --wall 2x1:0 --group wallpaper rain   # left half of a 2×1 wall
termpaper --wall 2x1:1 --group wallpaper rain   # right half
termpaper --no-wall                             # never join a wall
```

**Padding:** if your terminal pads its grid, say so — `pad = 7` (px),
`pad = "3.5pt"` (kitty's `window_padding_width` is in points) or `pad = [x, y]`,
plus `placement = "center"` if the terminal centres its grid — so the wall
lines up at the text edge, not the window chrome.

### Fleet commands

```sh
termpaper instances                # list all groups (pid, group, scene, uptime)
termpaper --no-link                # unlink — solo art
termpaper --group wallpaper        # join / isolate a sync cluster
termpaper switch fire --group wallpaper
```

The menu's **Wall** page has **Link** (on/off) and **Group** rows.
Presets cycle through `default`, `wallpaper`, `desk`, and `art`, and the
page lists who's in your group. Settings that change the simulation
(scene, theme, detail, pixels, speed, text size) travel in the group's
**anchor** and switch every pane together after a synchronised fade;
appearance settings (filters, fps, smooth, dim, fade, colour grade, the `f`
quick filter) apply live. Remote changes are session-only and never touch
your config file. A new instance adopts the anchor on launch and replays to
the shared frame out of sight; if that would take more than a few seconds,
the group restarts the scene together instead.

Linking works on macOS too (iTerm2, Terminal.app, …): without
`$XDG_RUNTIME_DIR` the registry lives in `/tmp/termpaper-$UID`. Only the
video wall's *auto* window-geometry mode is Linux/Hyprland-only — manual
`--wall COLSxROWS:INDEX` tiling works everywhere.

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
Toggle any from **`?` → Look → Filters…** — grouped Colour / Texture /
Geometry, with Clean · Film · CRT · Dream presets — or preview one live with
**`f`**.

**Temporal smoothing** (`smooth = 0.3` by default) blends each frame toward
the last — silk at any fps. Glyph cells never blur. Kill it with `smooth = 0`
or dial it in live from `?` → Display.

```sh
termpaper fire --filter crt
termpaper city --filter scanlines --filter vignette
```

## The GPU engine

Studio scenes and the post-processing chain run on Vulkan through `wgpu`
(on by default; `cargo build --no-default-features` builds a CPU-only binary
with the Classic scenes). Every frame is one GPU submission: the scene pass
writes this pane's window of the picture — plus a small apron when a filter
reads neighbours, so filters stay seamless across a wall — then filters,
colour grade, smoothing and terminal-cell packing run as compute passes and
only the finished cells are read back, pipelined so the CPU never waits.

- **Quality governor.** Each frame the scene pass is timed on the GPU and the
  sample count per pixel moves within a budget (`gpu_budget_ms`, default 3 ms)
  so a wallpaper never makes your compositor stutter. Only anti-aliasing
  changes — never geometry — so panes of a wall that settle on different
  counts still meet cleanly. `Quality` (low / medium / high) sets the ceiling
  and the ray-march step count.
- **Bandwidth.** The terminal write is the real bottleneck, so Studio scenes
  cap at `shader_fps` (60), skip frames that would be identical, and pack
  cells with hysteresis: a cell whose colours moved by only a few levels keeps
  its previous value and ratatui never re-sends it (about 45% fewer bytes).
  Output is wrapped in DEC 2026 synchronized updates, so no terminal ever
  shows half a frame.
- **Robust.** Scene shaders compile on a background thread while the old
  scene fades out, inside error scopes: a broken shader shows its Classic
  fallback and never takes the GPU down for the others.

```sh
termpaper bigsur --renderer auto     # GPU when available (default)
termpaper bigsur --renderer cpu      # force the CPU: Studio scenes show their fallback
cargo test --release --test gpu_shader_scenes -- --ignored   # render every scene on your GPU
```

`?` shows the active path, e.g. `GPU shader · AMD Radeon RX 6700 XT · 6 spp · 1.8 ms`.

**GPU worlds (experimental)** — `--renderer shader` draws Classic scenes from
the older all-in-one `world.wgsl` interpretations instead of their Rust code.

## Controls (defaults — all remappable in `[keys]`)

| Key     | Action                                  |
| ------- | --------------------------------------- |
| `q`     | quit (`Esc` / `Ctrl-C` always work too) |
| `?`     | menu: scene browser + settings (`?` inside it for help) |
| `←`/`→` | previous / next scene                   |
| `c`     | 100-step color wheel (hue/sat/contrast) |
| `f`     | cycle quick filter preview              |
| `d`     | cycle detail level                      |
| `[`/`]` | fps down / up (through presets incl. **120**) |
| `,`/`.` | speed down / up                         |
| `space` | pause (freeze animation)                |
| `0`     | reset all settings to defaults (not while the menu is open) |

## Config

`~/.config/termpaper/config.toml` — read at startup, **written back** by the
menu (atomically, temp + rename, a moment after the last change). Only values
that differ from the defaults are written. Everything optional; CLI flags
override.

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
# cycle = 300           # auto-rotate scenes every N seconds (local only)
# cycle_scope = "favorites"   # all|category|favorites
favorites = ["bigsur", "koi"] # ★ in the menu browser
gpu_budget_ms = 3.0     # GPU time per frame a Studio scene may use
shader_fps = 60         # fps cap while a Studio scene is showing

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

A **Studio scene** is one `.wgsl` file in `src/gpu/scenes/` — a header and a
`fn scene(p: vec2f, ctx: Ctx) -> vec3f`. `build.rs` finds it; there is no
registry to edit. The shared library (`src/gpu/scenes/lib/`) brings a
physical sky, volumetric clouds, water with Fresnel and caustics, fog, rain,
snow, fire, bokeh, SDFs and ray-marching. Iterate without rebuilding:

```sh
cargo run --release --example shader_review -- yourscene         # contact sheets
cargo run --release --example shader_review -- yourscene --bench # GPU time vs budget
TERMPAPER_SHADER_DIR=src/gpu/scenes cargo run --release -- yourscene   # live, hot reload
```

Classic scenes are Rust modules implementing the `Scene` trait. The full
recipe for both, the author contract and the design bar:
[CONTRIBUTING.md](CONTRIBUTING.md).

## Classic scenes (51)

The original catalog — one word each, full descriptions in `termpaper list`:

**rain** · **starfield** · **fire** · **pipes** · **plasma** · **aurora** · **life** · **boids** ·
**lava** · **tunnel** · **dvd** · **bump** · **canopy** ·
**finale** · **ocean** · **circuits** · **clouds** · **mandel** ·
**meteors** · **koi** · **sand** · **city** · **abyss** ·
**den** · **traffic** · **nexus** · **ripple** · **fireflies** ·
**lanterns** · **incense** · **frost** · **orbits** · **ribbons** · **sonar** ·
**tide** · **clockwork** · **grid** · **inkdrop** · **mosaic** ·
**harmonograph** · **nebula** · **pendulum** · **reaction** · **meadow** ·
**airspace** · **aquarium** · **drive** · **candy** · **scroll** · **alpine** · **campfire**
— run `termpaper list` for one-line descriptions.

A dim clock (`HH:MM`) sits in the top-right corner of every scene —
toggle it in `?` → Display → Clock.

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
hole, hue ring + saturation/contrast rows), also at `?` → Look → Color grade…. **orbits** runs true Kepler ellipses — eccentric anomaly solved
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
shaders instead, see **The GPU engine**. Resizes every frame. Coverage test:
<2% unpainted cells at any size — no borders, no dead pixels.

## License

MIT — take it, remix it, ship scenes. [CONTRIBUTING.md](CONTRIBUTING.md) for the craft.
