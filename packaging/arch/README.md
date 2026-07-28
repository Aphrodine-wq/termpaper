# Arch Linux package

## From a git tag (published release)

```sh
cd packaging/arch
makepkg -si
```

Requires the `v0.1.0` tag on the remote configured in `PKGBUILD`.

## Local dev build (no tag)

From the repo root:

```sh
./packaging/arch/build-local.sh
```

This tarball-builds the current tree and runs `makepkg -si` against a generated `PKGBUILD.local`.

After install:

```sh
termpaper rain
termpaper --list
```
