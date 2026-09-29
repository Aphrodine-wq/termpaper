# Packaging

| Platform | File | Published by |
|---|---|---|
| Arch Linux | `arch/PKGBUILD` | AUR (see `arch/README.md`) |
| macOS / Linux (Homebrew) | `homebrew/termpaper.rb` | a tap repo, `Aphrodine-wq/homebrew-tap` |
| Windows (Scoop) | `scoop/termpaper.json` | a bucket repo, or `scoop install <raw url>` |

The release workflow (`.github/workflows/release.yml`) builds every archive
these point at when a `v*` tag is pushed:

- `termpaper-x86_64-unknown-linux-gnu.tar.gz`, `termpaper-aarch64-unknown-linux-gnu.tar.gz`
- `termpaper-x86_64-pc-windows-msvc.zip`, `termpaper-aarch64-pc-windows-msvc.zip`
- `termpaper-aarch64-apple-darwin.tar.gz`, `termpaper-x86_64-apple-darwin.tar.gz`,
  and `termpaper-universal-apple-darwin.tar.gz` (both in one binary)

After a release, fill in the `FILL-IN-AT-RELEASE` hashes:

```sh
gh release download v0.2.0 -p 'termpaper-*' -D /tmp/tp && (cd /tmp/tp && sha256sum *)
```

The one-line installers need no manifest: `install.sh` (Linux, macOS) and
`install.ps1` (Windows) download the latest release directly.
