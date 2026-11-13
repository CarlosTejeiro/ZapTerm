# Arch Linux packaging

`PKGBUILD` builds ZapTerm **from source** and installs it as the `zapterm` package
(binary at `/usr/bin/zapterm`, plus an icon and a `.desktop` launcher).

## Why not the AUR?

The ZapTerm repository and its releases are **private**. A public AUR package
must download its sources from a publicly reachable URL, which isn't possible
here, and a token can't be embedded in a published PKGBUILD. So this PKGBUILD
is for **your own machines** (or collaborators with repo access), not the AUR.

## Requirements

- An SSH key with read access to `github.com/CarlosTejeiro/zapterm`
  (`git clone git@github.com:CarlosTejeiro/zapterm.git` must work for you).
- Build deps are declared in the PKGBUILD (`rust`, `cargo`, `nodejs`, `pnpm`,
  `webkit2gtk-4.1`, `gtk3`, `libayatana-appindicator`, …); `makepkg` installs
  the missing ones with `-s`.

## Install / upgrade

```sh
cd packaging/arch
makepkg -si          # build + install (pulls the v<pkgver> tag)
```

To move to a newer release, bump `pkgver` in the PKGBUILD to the new tag's
version (e.g. `0.10.0`) and run `makepkg -si` again.

## Notes

- The build pins the source to the `v<pkgver>` git tag for reproducibility.
- It tries `cargo tauri build --no-bundle`; if the Tauri CLI can't be
  installed it falls back to building the frontend with pnpm and the Rust
  binary with `cargo build --release --bin app`.
- A `zapterm-bin` PKGBUILD that repacks the released `.deb` (no recompile) would
  be cheaper, but needs the `.deb` available locally or on a host you control;
  add it here later if that becomes the preferred path.
