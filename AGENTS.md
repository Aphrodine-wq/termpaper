# Notes for coding agents

Read CONTRIBUTING.md first. It is the scene contract: coordinates,
determinism, terminal bandwidth, performance budgets and WGSL gotchas. The
rules below are the maintainer's standing instructions, and they are not
written down anywhere else.

## Never

- **Run `cargo fmt`.** The repo is deliberately not rustfmt-clean. The
  flat nesting in the `main.rs` event loop is intentional. Match the
  surrounding style by hand.
- **Test a live instance in the default link group.** The maintainer's own
  instances use it, so a test there retargets their screens. For example:

  ```sh
  scratch=$(mktemp -d)
  XDG_CONFIG_HOME=$scratch/config XDG_STATE_HOME=$scratch/state \
  XDG_RUNTIME_DIR=$scratch/run \
    tmux new -d -s tp-test 'termpaper --group clitest bigsur'
  # ...look, then:
  tmux kill-session -t tp-test
  ```

- **Touch the portrait-monitor scroll instance.** It runs from
  `~/.local/bin/scroll-monitor.sh` with
  `XDG_CONFIG_HOME=~/.config/termpaper-scroll`. Don't kill it, restart it or
  send it controls.
- **Commit, push, merge, tag or publish without the maintainer's OK.** That
  covers the site and the gallery too.
- **Port Shadertoy code.** Most of it is CC BY-NC-SA and this project is MIT.
  Reimplementing a technique is fine; copying code is not. Fill in `credits:`
  honestly.

## Design rules that are easy to miss

- **The terminal write is the bottleneck, not the GPU.** Keep the camera
  still and put the motion inside the scene. Keep under about 40% of cells
  changing at default Tune values. Anything that moves the camera (drift,
  pointer parallax) defaults to off.
- **Walls must line up exactly.** Every pane calls `scene()` at the shared
  time. Any live input (pointer, focus, audio) is sampled by the group leader
  and published with timestamps. Each pane eases it in closed form from the
  shared clock, so panes stay bit-identical. A pane must never smooth input
  on its own frame cadence.
- **Budgets:** light 0.8 ms, medium 1.6 ms, heavy 2.5 ms. Measure with
  `cargo run --release --example shader_review -- <name> --bench`. Voxel
  Valley is currently over budget; don't add more scenes that are.
- **Hyprland:** never call `hyprctl` or query the socket from the render
  loop. Watchers run on their own threads, and only on the group leader.
- **The CPU is the reference for every GPU post pass**
  (`tests/gpu_parity.rs`).
- **Check for duplicates first.** Before adding a scene, `ls
  src/gpu/scenes/` and `src/scene/`; there are already 113.

## Checks before handing back

```sh
cargo test --lib
cargo test --test shader_scenes          # validates and lints every Studio scene
cargo test --all-targets --no-default-features
```
