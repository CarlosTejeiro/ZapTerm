# ZapTerm website

The static site published at <https://carlostejeiro.github.io/ZapTerm/> by
`.github/workflows/pages.yml` on every push to `main` that touches `site/`.

Plain HTML, CSS and a little JavaScript — no build step. Preview locally with:

```sh
cd site && python3 -m http.server 8000
```

- `img/` — screenshots of the real app (WebP), taken against simulated lab
  devices; host names and addresses are fictional.
- `fonts/` — Geist and JetBrains Mono (SIL Open Font License, licences included).
- Download buttons are resolved from the latest GitHub release by `main.js`;
  without JavaScript or network they link to the releases page.
