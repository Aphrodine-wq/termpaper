# Scene Marketplace

Community scenes for [termpaper](https://github.com/Aphrodine-wq/termpaper). Browse the
catalog, install scenes from GitHub, and publish your own.

## Browse & install

```sh
termpaper marketplace list
termpaper marketplace info Aphrodine-wq/example-pulse
termpaper marketplace install Aphrodine-wq/example-pulse
termpaper pulse
```

Install clones the scene repo (or a subdirectory of it), saves it under
`~/.local/share/termpaper/marketplace/`, and rebuilds termpaper so the scene
is compiled in. You need Rust + a termpaper source tree (from `./install.sh`
or `TERMPAPER_SRC`).

```sh
termpaper marketplace installed
termpaper marketplace update
termpaper marketplace rebuild
```

Press **`?`** in any scene → **Marketplace** tab for a quick summary.

## Publish your scene

1. Copy [`template/`](template/) into a new public GitHub repo.
2. Edit `scene.rs` and `scene.toml` (see [CONTRIBUTING.md](../CONTRIBUTING.md)).
3. Open a PR adding an entry to [`index.json`](index.json).

```sh
termpaper marketplace publish   # prints the full checklist + PR link
```

### Index entry fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | yes | Unique id, e.g. `your-user/my-scene` |
| `scene_name` | yes | CLI name (`termpaper my_scene`) |
| `description` | yes | One line for `--list` |
| `author` | yes | GitHub username |
| `repo` | yes | Clone URL |
| `subdir` | no | Path inside repo if not at root |
| `struct_name` | no | Rust struct (default: capitalized `scene_name`) |
| `themes` | no | Theme names for the settings menu |
| `tags` | no | Search keywords |

## How it works

Marketplace scenes are **Rust source**, same as built-in scenes. On install,
`build.rs` reads `~/.config/termpaper/marketplace.toml` and generates a
compile-time module for each installed package. Prebuilt `--binary` installs
can browse the catalog but need a source rebuild to run community scenes.

## Example

[`examples/pulse/`](examples/pulse/) — demo scene listed in the index.
