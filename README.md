# termpaper

**Wallpaper Engine for the terminal.** Real places, rendered live on your GPU —
Big Sur at sunset, a Shinjuku alley in the rain, the aurora over Tromsø, a
ramen stall under paper lanterns, a black hole bending its own light — in
the terminal you already use, on **Linux, macOS and Windows**. **50 Studio
scenes** are ray-marched, physically lit WGSL shaders with 3–4 time-of-day
and weather variants each; the **51 Classic** hand-animated CPU scenes are
still here. Give any of them a **theme** — a colour grade, a palette and a
few effects in one small file — pick from 34, make your own, or install one
someone shared. Run one terminal per monitor and they become **one
continuous picture across every screen**.

```sh
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash && termpaper tokyo
```

```powershell
irm https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.ps1 | iex; termpaper tokyo
```

```sh
termpaper bigsur                     # a Studio scene
termpaper tokyo --variant snow       # every scene has variants
termpaper koi --theme tokyo-night    # and every theme works on every scene
termpaper list                       # the catalog, by category
termpaper theme browse               # themes people shared
termpaper                            # press ? — browse, preview, tune
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

### Where it runs

| | Terminals | Studio scenes on | Notes |
|---|---|---|---|
| **Linux** | kitty, Ghostty, WezTerm, Alacritty, foot, GNOME Terminal, Konsole, … | Vulkan | The desk-spanning wall is automatic on Hyprland. |
| **macOS** | iTerm2, Ghostty, kitty, WezTerm, Alacritty, Terminal.app | Metal | Terminal.app shows 256 colours; termpaper notices and adapts. Starts at half-block pixels and low quality to stay cool on a laptop; change either in the menu. |
| **Windows** | Windows Terminal, WezTerm, Alacritty, the classic console | Vulkan, else DX12 | Use a font with block and braille glyphs for quadrants, sextants and braille (Cascadia Mono, the Windows Terminal default, has them all); half blocks work with any font. |

No GPU? Studio scenes fall back to a related Classic scene, and linked
terminals without a GPU still stay in sync with the rest. No 24-bit colour?
termpaper falls back to 256 colours by itself. Not sure what your terminal
can do? `?` → Display → **Terminal check…** shows you and sets things to
match; the first launch walks you through it.

### Quick install

**macOS and Linux** — builds with `cargo` when you have Rust, and otherwise
fetches the prebuilt binary (`--binary` asks for it either way):

```sh
curl -fsSL https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.sh | bash
```

**Windows** (PowerShell, no administrator rights needed) — puts
`termpaper.exe` in `%LOCALAPPDATA%\Programs\termpaper` and adds it to your
PATH:

```powershell
irm https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.ps1 | iex
```

Prebuilt binaries are attached to every `v*` release: Linux x86_64 and
aarch64, macOS arm64, x86_64 and universal, Windows x86_64 and ARM64.

### Other ways

**Cargo** (any OS, Rust from [rustup.rs](https://rustup.rs)):

```sh
cargo install --git https://github.com/Aphrodine-wq/termpaper.git --locked
```

**From a clone:**

```sh
git clone https://github.com/Aphrodine-wq/termpaper.git && cd termpaper && ./install.sh
# or: cargo install --path . --locked
```

**Arch Linux** (system package): `cd packaging/arch && makepkg -si` (see
[packaging/arch/README.md](packaging/arch/README.md)).

**Smaller builds:** `--no-default-features` leaves out the GPU (Classic
scenes only) and the theme gallery; add `--features gpu` or `--features net`
back as you like.

### After install

If your shell says *command not found* after a cargo install:

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # add to ~/.bashrc or ~/.zshrc
```

On Windows, open a new terminal so it sees the new PATH. Then:

```sh
termpaper list            # the catalog, by category
termpaper rain            # a scene
termpaper                 # press ? for the menu
```

**Screensaver / fullscreen:** any pane, tmux split, or dedicated window — go
big. Spanning monitors? See [The swarm](#the-swarm--instances-sync--multi-monitor-walls).

**Solo mode:** `termpaper --no-link fire` · **Wallpaper cluster:** `termpaper --group wallpaper rain`

## Make it yours

termpaper is built to be dialed in — live, from the keyboard or the mouse,
with every change saved to your config (see [Config](#config)).

| Layer | What you control | How |
|-------|------------------|-----|
| **Look** | Themes: a grade, a palette and effects in one, 34 built in, yours, and the gallery's | `?` → Themes, `--theme`, `termpaper theme …` |
| **Colour** | Exposure, contrast, saturation, vibrance, white balance, hue, matte; shadow, midtone and highlight wheels; palettes | `c` — the colour studio |
| **Effects** | 27, stacked in order, each with a strength; 14 presets | `?` → Look → Effects…, `--filter`, `f` previews one |
| **Motion** | Speed, cycling (order, scope, time of day), transitions, pause | `?` → Playback, `,`/`.`, `space` |
| **Picture** | Colours, 6 pixel modes, quality, smoothing, cell shape | `?` → Display, `--pixels`, `--detail` |
| **Pace & power** | FPS (adapts to the terminal), unfocused and battery behaviour, output bandwidth, GPU choice | `?` → Display, `[`/`]` |
| **On screen** | Clock (small or large, format, corner), scene name, performance readout, night dimming | `?` → Display |
| **Walls** | Link group, sync look, layout (auto or a grid on any OS), padding, bezels | `?` → Wall, `--group`, `--wall`, `--pad` |
| **Input** | 13 remappable actions; mouse in the menu | `[keys]` in the config |

`u` undoes the last change, from anywhere in the menu. **Example — a fully
loaded start:**

```sh
termpaper jupiter --theme synthwave --fps 120 --pixels sextant --group wallpaper
```

termpaper ignores `NO_COLOR` — this is a colour-art program. Force the
256-colour look with `--no-truecolor` or `?` → Display → Colours.

## Themes

A **theme** is a whole look in one small file: a colour grade, an optional
palette, and a stack of effects with their strengths — and, if you like, the
scene it was made for. Themes work on every scene. (What each scene has on
its own — Tokyo in snow, Big Sur at noon — are its **variants**: `t` in the
scene browser, `--variant`.)

**34 built in**, on shelves: Cinematic (Teal & Orange, Bleach Bypass, Noir,
Golden Hour, Kodachrome, Super 8, …), Terminal palettes (Tokyo Night,
Catppuccin Mocha, Gruvbox, Nord, Dracula, Rosé Pine, …), Retro (Amber CRT,
Green Phosphor, Game Boy, Synthwave, Vaporwave) and Mood (Arctic, Ember,
Cyberpunk, Sakura, …).

**In the menu** (`?`, `Tab` to Themes): rest on a theme and it previews on
the live scene; `Enter` wears it, `Esc` puts yours back. `e` opens it in the
colour studio, `n` saves your look as a new theme, `U` saves edits into one
of yours, `c` copies its **share code**, `i` imports a code, a file or a
gallery link, `s` switches to the scene it was made for, and `/` searches.

**From a shell:**

```sh
termpaper theme list                    # every theme, with swatches
termpaper theme show gruvbox            # what it does, its file, its code
termpaper theme apply nord              # wear it; running panes in the group follow
termpaper theme new "Late Shift"        # save the look you have as a theme of yours
termpaper theme export late-shift --code
termpaper theme import tp1:…            # a share code, a .toml file, or - for stdin
termpaper theme check my-theme.toml     # before you share it
```

Yours live in `~/.config/termpaper/themes/` (`%APPDATA%\termpaper\themes`
on Windows) as TOML you can edit by hand:

```toml
format = 1
name = "Late Shift"
author = "you"
tags = ["warm", "film"]

[grade]
exposure = -0.2
contrast = 1.15
shadows = { hue = 200.0, amount = 0.4 }
highlights = { hue = 35.0, amount = 0.5 }

[palette]
mode = "tint"               # off | map | tint | snap
colors = ["#1a1b26", "#7aa2f7", "#e0af68"]
strength = 0.6

[effects]
stack = ["halation", "grain", "vignette"]
grain = 0.5                 # strengths: 1 is the effect's usual

[scene]                     # optional: made for
name = "tokyo"
variant = "rain"
```

A **share code** (`tp1:…`) is the same theme compressed into one line: paste
it anywhere a theme name goes.

### The gallery

[termpaper's website](https://termpaper-site.vercel.app/themes/) has a
gallery of themes people shared, each previewed on real scenes, and a
**theme studio** for making one in the browser — with every control
termpaper has, and your terminal's colour scheme (kitty, Ghostty,
Alacritty, iTerm2, Windows Terminal) as a palette.

```sh
termpaper theme browse                  # newest first; --popular, --tag warm, or words to search
termpaper theme install 8aV7gsQH        # an id, a link to its page, or a share code
termpaper theme publish late-shift      # share one of yours: anyone can install it
termpaper theme unpublish 8aV7gsQH      # take it down again
```

Or press `g` on the Themes page: the gallery's themes arrive on a
**Gallery** shelf, preview like any other, and `Enter` installs one.

termpaper only talks to the gallery when you ask it to — these commands and
`g` — with a 5-second limit, and sends nothing but the request. Publishing
keeps an edit token in termpaper's state folder so `unpublish` works from
the same machine. The site counts installs, likes and reports once per
address, by a salted hash rather than the address itself; three reports
hide a theme until someone looks. Point termpaper at another gallery with
`gallery_url` in the config or `$TERMPAPER_GALLERY`.

## The menu — press `?`

A drawer slides in on the left; the **live** scene keeps playing on the right.
Six pages — `Tab` / `Shift-Tab` switch, or click a tab:

- **Scenes** — the browser. Categories on the left (★ Favorites, ◷ Recent,
  Coast & Water, Mountains & Wild, Weather, City, Cozy, Space, … and Classic
  with its sub-groups), their scenes on the right, and a detail panel with
  the description, variants, tags and a GPU badge. Rest on a scene for a
  moment and it **previews** behind the drawer on this pane only; `Enter`
  keeps it (and switches your linked group), `Esc` puts the original back.
  `/` filters, `f` stars, `t` steps through its variants, `r` picks one at
  random.
- **Themes** — see [Themes](#themes).
- **Look** — theme and variant, the colour studio, exposure, contrast,
  saturation, vibrance, temperature, tint, hue, matte, palette and its
  strength, Effects… (grouped, with presets and a strength per effect),
  glow, vignette, grain, letterbox, brightness, text size.
- **Playback** — speed; cycle (30 s to an hour) through all scenes, a
  category, favourites, Studio or Classic scenes, or this scene's variants,
  in order or shuffled; **follow the clock** (dawn, day, dusk and night
  variants as the day goes on); what to open on; the **transition** (fade,
  dissolve, wipe, iris, blinds) and its length.
- **Display** — Picture: colours, pixels, quality, smoothing, cell shape.
  Speed & power: FPS and *adapt FPS* (steps down while the terminal cannot
  keep up), when unfocused (keep, 30, 15, pause), on battery, output
  bandwidth, renderer, GPU (integrated or discrete), the Studio time budget
  and FPS. On screen: clock (small or large, 12/24-hour, seconds or date,
  any corner), scene name on change, performance readout, mouse. Night:
  dim on a schedule. Terminal check….
- **Wall** — link and group (or a new one), sync look, layout (auto on
  Hyprland, or a grid you set on any OS), this pane's cell, padding,
  placement, bezels, pause the wall, start and stop the wall, align
  monitors, and the live instances (★ leads).

`↑`/`↓` move, `←`/`→` change a value, `Enter` toggles or opens, `Esc` backs
out, `u` undoes; the mouse clicks, drags sliders and scrolls. The footer
only shows keys that do something right now, and `?` inside the menu lists
the rest. Changes are saved a moment after you stop — only the ones that
differ from the defaults. The art never stops.

### The colour studio — press `c`

Every grading control in one panel, on anti-aliased wheels drawn in half
blocks (round, whatever your font's cell shape):

- **Colour** — a white-balance wheel (temperature across, tint up and
  down) ringed by the hue shift, with exposure, contrast, saturation,
  vibrance, temperature, tint, hue and matte beside it.
- **Tones** — three wheels, shadows, midtones and highlights, each pushing
  its range toward a colour, and the balance between them.
- **Palette** — 23 named palettes or your own 2–8 colours, mapped by
  brightness, tinted toward, or snapped to (with optional dithering).

Along the bottom, reference colours before and after. Keys and mouse reach
everything; `0` resets what you are on. Changes sync to linked panes, save,
and undo like any other.

## Quick start

```sh
termpaper rain --fps 120          # high-refresh
termpaper list                    # the catalog
termpaper fire --variant frost
termpaper lofi --theme faded-film
termpaper --cycle 300             # a new scene every five minutes
termpaper --no-link candy         # solo — off the swarm
```

Press **`?`** for the menu · **`c`** colour studio · **`[`/`]`** fps · **`,`/`.`** speed · **`q`** quit.

## Usage

```
Usage: termpaper [OPTIONS] [SCENE] [COMMAND]

Commands:
  list       List scenes by category and exit   (--category coast|wilds|…|classic, --json)
  instances  List live termpaper instances in every group and exit
  switch     Switch running instances to a scene and exit   (--group G | --all)
  desk       Show the physical desk (monitors in millimetres) and the wall plan
  calibrate  Line the monitors up with a millimetre test pattern
  wall       Start or stop one wall terminal per monitor (Hyprland)
  theme      Themes: list, apply, make and share them, and install them from the gallery

Scene:
  [SCENE]                  Scene to run (see `termpaper list`)
      --theme <THEME>      A theme (see `termpaper theme list`), or one of this scene's variants
      --variant <VARIANT>  The scene's variant: its time of day, weather or colours
      --cycle <SECS>       Move on to another scene every N seconds
      --speed <SPEED>      Animation speed multiplier
      --screensaver        Screensaver mode: any key exits

Look:
      --filter <FILTER>          An effect, repeatable (e.g. --filter crt --filter bloom)
      --pixels <PIXELS>          half, quad, sextant, braille, ascii or blocks
      --text-scale <TEXT_SCALE>  Bump text scale: 1, 2 or 3
      --no-truecolor             256 colours even on a truecolor terminal

Performance:
      --fps <FPS>            Target frames per second
      --idle-fps <IDLE_FPS>  Slow down while the terminal is unfocused
      --detail <DETAIL>      Quality: low, medium or high
      --renderer <RENDERER>  auto, gpu, cpu or shader

Linking & wall:
      --link / --no-link   Instance linking (default on)
      --group <GROUP>      Link group for this instance
      --no-wall            Never join a video wall
      --wall <WALL>        Manual video-wall tiling: COLSxROWS:INDEX (e.g. 2x1:0)
      --pad <PAD>          Terminal padding: px (7), points (3.5pt), or x,y
```

`termpaper <command> --help` has the details. The pre-subcommand spellings
(`--list`, `--instances`, `--switch`, `--all-groups`, `--gpu`) still work.

## The swarm — instances, sync & multi-monitor walls

Every running termpaper on your machine registers itself — a local swarm with
no network, no daemon. Link them into sync clusters, cut one loose for solo
viewing, or tile them into a **video wall** so one animation flows across
monitors like a single panoramic canvas.

### Every terminal, accounted for

Each instance checks into a file-based registry:

| Where | |
|-------|--|
| Linux | `$XDG_RUNTIME_DIR/termpaper/` (named groups under `groups/<name>/`) |
| macOS | `/tmp/termpaper-$UID/` |
| Windows | `%TEMP%\termpaper\` |

Every instance writes `inst-<pid>.json` with its **pid**, **scene**, **link
group**, terminal size, **window geometry** (Hyprland via `hyprctl` when
available), and **padding** (for wall alignment). Each pane touches its
entry every two seconds; one untouched for ten is gone, so a crashed pane
never haunts the swarm.

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
- Live appearance settings — the look (theme, grade, palette, effects; Wall →
  Sync look off keeps a pane's own), fps, smooth, dim, fade, the transition,
  the `f` effect preview
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
The anchor persists as long as any member of the cluster lives. The
lowest-pid instance leads (★ on the Wall page): it answers restart
requests, retimes the cluster after a system suspend, and follows the clock
for everyone.

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

**Without Hyprland** — on macOS, Windows or any other desktop — set a grid:
Wall → Layout → grid, the same columns × rows on every pane and each pane's
own cell, or from the command line:

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
appearance settings (the look, fps, smooth, dim, fade, the `f` effect
preview) apply live. Remote changes are session-only and never touch
your config file. A new instance adopts the anchor on launch and replays to
the shared frame out of sight; if that would take more than a few seconds,
the group restarts the scene together instead.

Linking works the same on macOS and Windows. Only the wall's *auto* layout
(from where the windows are) needs Hyprland — the grid layout works
everywhere.

## Pixel modes

Terminal cells are tall — termpaper packs extra resolution into each one
(`?` → Display → Pixels, or `--pixels`):

- **quad** (default) — 2×2 px/cell via quadrant glyphs (`▘▝▖▗`…). Sharp
  detail; luminance-weighted colour split per cell.
- **half** — 1×2 px/cell via `▀`. Every pixel keeps its exact colour, and it
  works with any font.
- **sextant** — 2×3 px/cell via the sextant glyphs of Unicode 13 (Cascadia,
  Iosevka, recent JetBrains Mono, kitty and WezTerm's built-in boxes).
- **braille** — 2×4 px/cell. Maximum density, with some colour compromise.
- **ascii** — one character per cell from a density ramp (` .:-=+*#%@`):
  works in any font and any terminal.
- **blocks** — one pixel per cell, background colour only: the least to
  send, for slow links.

Glyph scenes (**bump**) stay typographic in every mode. The terminal check
(`?` → Display → Terminal check…) shows which of these your font draws.

## Effects

Cinematic post on every frame — **27 effects**, stacked in order, each with a
strength (1 is its usual, 0–2):

- **Colour:** `warm`, `cool`, `sepia`, `noir`, `duotone`, `thermal`,
  `invert`, `hue`, `spectrum`, `gamma`, `posterize`
- **Light:** `bloom`, `halation`, `vignette`
- **Texture:** `grain`, `scanlines`, `crt`, `chroma`, `dither`
- **Lens & frame:** `letterbox`, `tiltshift`, `sharpen`, `edges`
- **Geometry:** `pixelate`, `warp`, `mirror`, `kaleido`

`?` → Look → Effects… has them grouped with presets — Clean, Film, CRT,
Dream, Cinema, VHS, Arcade, Neon, Soft, Miniature, Sketch, Print, 8-bit,
Kaleidoscope — and `f` outside the menu previews one at a time. A theme
carries its own stack.

**Temporal smoothing** (`?` → Display → Smooth) blends each frame toward the
last — silk at any fps. Glyph cells never blur.

```sh
termpaper fire --filter crt
termpaper city --filter scanlines --filter vignette
```

## The GPU engine

Studio scenes and the post-processing chain run on the GPU through `wgpu`:
Vulkan on Linux, Metal on macOS, Vulkan or else DX12 on Windows
(`WGPU_BACKEND=vulkan|metal|dx12` overrides). Every frame is one GPU
submission: the scene pass writes this pane's window of the picture — plus a
small apron when an effect reads neighbours, so effects stay seamless across
a wall — then the effects, the look, the transition, smoothing and
terminal-cell packing run as compute passes and only the finished cells are
read back, pipelined so the CPU never waits. Every pass has a CPU twin, and
tests hold the two to the same output.

- **Quality governor.** The scene pass is timed on the GPU and the sample
  count per pixel moves within a budget (Display → Studio budget, 3 ms by
  default) so a wallpaper never makes your compositor stutter. Only
  anti-aliasing changes — never geometry — so panes of a wall that settle on
  different counts still meet cleanly.
- **Bandwidth.** The terminal write is the real bottleneck, so Studio scenes
  cap at their own FPS (Display → Studio FPS), frames that would be
  identical are skipped, and cells whose colours barely moved are not
  re-sent (Display → Output: full, balanced or light). *Adapt FPS* measures
  how long the terminal takes to swallow a frame and steps down while it
  cannot keep up. Output is wrapped in synchronized updates, so no terminal
  shows half a frame.
- **Power.** On battery termpaper holds 30 fps and, with GPU on auto, uses
  the integrated chip; unfocused it can slow down or pause.
- **Robust.** Scene shaders compile on a background thread while the old
  scene fades out, inside error scopes: a broken shader shows its Classic
  fallback and never takes the GPU down for the others.

```sh
termpaper bigsur --renderer auto     # GPU when available (default)
termpaper bigsur --renderer cpu      # force the CPU: Studio scenes show their fallback
cargo test --release --test gpu_shader_scenes -- --ignored   # render every scene on your GPU
```

`?` → `?` shows the active path, e.g. `GPU shader · AMD Radeon RX 6700 XT · 6 spp · 1.8 ms`,
and Display → Performance puts frame rate, frame time and output size on screen.

**GPU worlds (experimental)** — `--renderer shader` draws Classic scenes from
the older all-in-one `world.wgsl` interpretations instead of their Rust code.

## Controls (defaults — all remappable in `[keys]`)

| Key     | Action                                  |
| ------- | --------------------------------------- |
| `q`     | quit (`Esc` / `Ctrl-C` always work too) |
| `?`     | the menu (`?` inside it for help) |
| `←`/`→` | previous / next scene                   |
| `c`     | the colour studio                       |
| `f`     | preview effects one at a time           |
| `d`     | cycle quality                           |
| `[`/`]` | fps down / up (through presets incl. **120**) |
| `,`/`.` | speed down / up                         |
| `space` | pause (the whole wall, when linked)     |
| `0`     | reset all settings to defaults (not while the menu is open) |

## Config

`~/.config/termpaper/config.toml` (`%APPDATA%\termpaper\config.toml` on
Windows; `$XDG_CONFIG_HOME` wins everywhere) — read at startup, **written
back** by the menu (atomically, a moment after the last change). Only values
that differ from the defaults are written. Everything is optional; CLI flags
override.

```toml
scene = "jupiter"
fps = 120                 # 1–240
speed = 1.0               # animation multiplier
pixels = "sextant"        # half|quad|sextant|braille|ascii|blocks
detail = "medium"         # low|medium|high
smooth = 0.3              # temporal smoothing, 0 = off
dim = 1.0                 # brightness 0.2–1.0
fade = 0.6                # scene transition seconds
clock = true
text_scale = 2            # 1|2|3 — bump scene only
cycle = 300               # a new scene every N seconds
cycle_scope = "favorites" # all|category|favorites|studio|classic|variants
favorites = ["bigsur", "koi"]
link = true               # false = solo art
group = "wallpaper"       # sync cluster
wall = true               # false = never crop into a video wall
wall_grid = "2x1:0"       # a manual wall: COLSxROWS:INDEX
pad = 0                   # terminal padding: px, "3.5pt", or [x, y]
placement = "top-left"    # top-left|center
sync_look = true          # take the group's look
gallery_url = "https://termpaper-site.vercel.app"   # the theme gallery
look_theme = "tokyo-night"  # the theme the look came from, if any

[look]                    # the look: what themes, the menu and the colour studio set
[look.grade]
contrast = 1.1
shadows = { hue = 220.0, amount = 0.3 }
[look.effects]
stack = ["bloom", "grain"]
grain = 0.5

[playback]
order = "shuffle"         # in-order|shuffle
transition = "dissolve"   # fade|dissolve|wipe|iris|blinds
time_of_day = true        # follow the clock through a scene's variants
on_launch = "favorite"    # last|favorite|random

[display]
colors = "auto"           # auto|truecolor|256
adapt_fps = true
unfocused = "30"          # keep|30|15|pause
battery = "save"          # save|keep
bandwidth = "balanced"    # full|balanced|light
gpu = "auto"              # auto|integrated|discrete
clock_style = "large"     # small|large
clock_format = "24h"      # 24h|12h|seconds|date
clock_corner = "top-right"
caption = true
hud = false
mouse = true
night = true
night_from = 22
night_to = 7
night_level = 0.5
studio_budget_ms = 3.0
studio_fps = 60

[themes]                  # each scene's variant
tokyo = "rain"
bigsur = "fog"

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

termpaper also writes `filters`, `hue_shift`, `saturation` and `contrast` as
plain mirrors of the look, so an older termpaper sharing the file keeps the
basics. Themes of your own sit beside it in `themes/`; the monitor layout for
walls in `desk.toml`. Unknown keys warn but never fail, and files from older
versions (`idle_fps`, `gpu_budget_ms`, `shader_fps`, …) still load.

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
