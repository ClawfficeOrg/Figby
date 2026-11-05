# Templates

- `figby-header.ftmp` — a working `.ftmp` template (TOML frontmatter).
  Render it with `figby -d fonts -T assets/templates/figby-header.ftmp`
  from the repo root.
- `*.ftmp.draft` — design sketches for a planned YAML element format
  (`ftmp-elements`). The current parser reads TOML only, so `figby -T`
  rejects them; they are renamed so nobody mistakes them for working
  examples.
- `figby-30w.ftm`, `figby-dos-rebel.txt` — rendered sample output
  (ANSI / plain text), not templates.
