# Third-Party Notices

ZapTerm is free software licensed under the GNU GPL v3 or later (see
[LICENSE](LICENSE)). It is built on and distributes third-party open-source
components that remain under their own licenses; this file collects the
required attributions.

Almost all bundled dependencies are under **permissive** licenses (MIT,
Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode, CC0, BSL-1.0,
CDLA-Permissive-2.0). A few are under the **MPL-2.0** — a file-level weak
copyleft that is fully compatible with the GPL-3.0-or-later ZapTerm itself uses,
and that imposes no obligations on ZapTerm beyond keeping those files' own notices:
`serialport` (serial console support), Servo's `cssparser` / `selectors` /
`dtoa-short` (pulled in by Tauri's HTML handling) and `option-ext` (via `dirs`).
No GPL/LGPL/AGPL third-party code is bundled. The allow-list is enforced in CI
by `cargo deny` (see [deny.toml](deny.toml)).

## Rust crates

The application links a large set of Rust crates (see `Cargo.lock` for the
exact set and versions), including notably:

- **tauri**, tauri-plugin-* — MIT OR Apache-2.0
- **russh**, russh-keys, russh-sftp — Apache-2.0
- **rusqlite** (bundled SQLite) — MIT; SQLite itself is public domain
- **tokio**, futures — MIT
- **portable-pty**, tokio-serial — MIT
- **vte**, **regex**, **serde**, **serde_json** — MIT OR Apache-2.0
- **keyring**, **aes-gcm**, **sha2**, **zeroize** — MIT OR Apache-2.0
- **window-vibrancy**, **notify**, **chrono** — MIT

Full, generated license texts for every crate can be produced with
[`cargo about`](https://github.com/EmbarkStudios/cargo-about):

```sh
cargo install cargo-about
cargo about generate about.hbs > THIRD-PARTY-RUST.html
```

## Frontend (JavaScript) components

Bundled into the web UI (see `frontend/pnpm-lock.yaml` for exact versions):

- **@xterm/xterm** and addons — MIT
- **svelte**, **vite**, **@sveltejs/vite-plugin-svelte** — MIT
- **tailwindcss** — MIT
- **@tauri-apps/api** and plugins — MIT OR Apache-2.0

## Fonts

- **Geist** — SIL Open Font License 1.1
- **JetBrains Mono** — SIL Open Font License 1.1
- **Cascadia Code** — SIL Open Font License 1.1

The SIL OFL permits embedding the fonts in software (of any license); it
only restricts selling the fonts on their own.

---

> Note: this is a summary. For a public release of the binaries, generate and
> ship the complete per-component license texts (e.g. via `cargo about` for
> Rust and `license-checker`/`pnpm licenses` for the frontend).
