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
- `faq.html` — every question and answer, grouped by topic, with search
  (`faq.js`). Add a question as a `<details id="…">` in its topic: the id is
  its shareable link (`faq.html#id`) and the search picks it up on its own.
  The home page shows a few of them and a search box that opens this page.
- `automation.html` — Python scripts and notebooks, with the code of two
  example scripts (`version_audit.py`, `push_template.py` with `ntp.j2` and
  `devices.csv`) exactly as they were run for the screenshots (`img/auto-*`).
- `privacy.html` — the privacy policy (linked from every footer). It describes what
  the released app and this site actually do; update it before releasing any change in
  how data is handled.
- `downloads/<version>/` — not in the repository: the deployment copies the Windows
  installers of the versions listed in `.github/store-installers.txt` from their
  releases, for the Microsoft Store, which needs a package URL that doesn't
  redirect. Keep every version submitted to the Store in that list.
