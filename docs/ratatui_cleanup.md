# Ratatui Cleanup Review

Date: 2026-10-28
Scope: `figby-rs/src/tui/` (84,995 LOC including tests)
Ratatui version: 0.30.1 / lockfile 0.30.2

## Verdict

Figby hand-rolls roughly **27 UI surfaces**. Only **one** delegates to a
ratatui built-in widget (`Toolbox` wraps `List`). No community widget from
the Ratatui third-party showcase is used.

- **12** `impl Widget` / `impl StatefulWidget` components
- **15** render-only panels / dialogs / overlays
- Built-ins actually used: `Block`, `Paragraph`, `List`, `Clear`
- Relevant built-ins unused: **`Tabs`, `Scrollbar`, `Gauge` / `LineGauge`, `Table`**
- Official third-party showcase: 13 widgets; **8 relevant; 0 used**

Goal is not zero custom code. Custom code is justified where no maintained,
reasonably scoped widget exists. The main avoidable cost is duplicated
file-browser and form code.

## Built-in replacement candidates

| Surface | Location | Current | Should use |
|---|---|---|---|
| Layer panel | `layers.rs:1029` | custom hierarchical list, groups, collapse, hand-rolled scroll | **`tui-tree-widget`** |
| Undo history | `undo_panel.rs` | `Paragraph` + manual scroll offset | `List` + `ListState` + `Scrollbar` |
| Light list | `light_panel.rs` | `Paragraph` lines | `List` + `ListState` |
| Recent files / font lists / dir listings | `file_ops.rs`, `export.rs`, `dialogs/*` | hand-built line lists + manual scroll | `List` / `Table` or `tui-widget-list` |
| Side panel tabs | `side_panel.rs:207-266` | manual tab row + separator | built-in `Tabs` |
| Document tab strip | `mod.rs:216-269` | manual `Paragraph` spans | built-in `Tabs` (keep hit-rect calculation beside it) |
| Keybinding overlay scroll | `overlays.rs:67-71` | manual max-scroll math | `Paragraph::scroll` already; add `Scrollbar` |
| Playback progress | `player.rs:337`, `player.rs:1008` | hand-drawn block characters | `Gauge` / `LineGauge` |
| Settings form | `status.rs:86-140` | custom field navigation | `ratatui-form` / `rat-text`, or keep (small) |

## Community replacement candidates

From the official third-party showcase:

| Surface | Candidate | Assessment |
|---|---|---|
| Layer panel | `tui-tree-widget` 0.24.1 | **Strong.** 2.0M downloads, active, ratatui 0.30. Exact fit for groups/collapse. |
| Menu bar | `tui-menu` 0.3.1 | Good crate, 43k downloads, ratatui 0.30, nested menus. **No mouse support.** Current menu is mouse-aware. Keep custom or fork and add mouse. |
| Scrollable help/side content | `tui-scrollview` 0.6.8 | Good, 513k downloads, active. Could replace manual scroll regions. |
| File/recent/undo lists | `tui-widget-list` 0.15.3 | Good for variable-height rows; otherwise built-in `List` is enough. |
| Throbber | `throbber-widgets-tui` 0.11.1 | 1.05M downloads, active. Overkill: only a spinner string is needed in the status bar. Low value. |
| Text editing | `ratatui-textarea` 0.9.3 | Active, official showcase. Only fits if text-tool blocks become textarea-edited; current canvas editing is different. |
| Image preview | `ratatui-image` 12.0.0-rc | Active. Could preview source images in import dialogs; the editor is intentionally ASCII, so not a core replacement. |
| Checkboxes | `tui-checkbox` 0.5.3 | Low value; current +/- property controls are not plain checkboxes. |

Useful but **not on the official showcase**:

- `tui-popup` 0.7.7 — official `tui-widgets`, 1.1M downloads. Good for a
  shared modal shell; all dialogs currently repeat `Clear` + `Block`.
- `rat-text` 3.1.0 / `rat-widget` 3.2.1 — text inputs, number inputs,
  checkboxes, tabs, table, file dialog, status bar. Active and broad, but a
  larger dependency.
- `tui-input` 0.15.5 — 2.3M downloads, active. Fits the many simple
  single-line inputs.
- `ratatui-explorer` 0.3.0 — 179k downloads, active, ratatui 0.30. File
  browser candidate.
- `tui-file-explorer` 4.0.0 — 13k downloads, active, multi-pane browser.
- `ratatui-form` 0.1.1 / `ratiform` 0.7.1 — form builders for the repeated
  dialogs.

Avoid: `ratatui-statusbar` (last release 2024, ratatui 0.26).

## Justified custom components — keep

No good drop-in exists for these:

1. Canvas cell editor — built-in `Canvas` is for drawing shapes, not editing
   a colored cell grid.
2. Font glyph grid + char editor.
3. Animation timeline, keyframe/tween editors, transport bar, thumbnails.
4. Palette swatch picker — `tui-rgb-picker` and `tui_color_picker` are
   tiny/new; not worth switching.
5. Palette editor / lighting swatches.
6. Lighting editor.
7. Welcome screen — mascot, animated title, custom menu hit rects.
8. Brush preview.
9. Animation player overlay.
10. Image editor state/preview.
11. Status bar — no maintained crate; custom `Line` composition is correct.
12. Props panel — app-specific controls.

## Biggest real problem: duplication, not lack of crates

Three or more copies of a hand-built file browser exist:

- `file_ops.rs:1126+` — 6 modes
- `export.rs:551+`
- `dialogs/gif_import.rs:469+`
- `dialogs/rascii_import.rs:469+`
- `dialogs/system_font.rs:230+`

Same for modal forms:

- `dialogs/new_image.rs:242`
- `dialogs/system_font.rs:230`
- `dialogs/font_import_options.rs:150`
- export / file-ops forms

Highest-value cleanup: extract **one** internal file-browser component and
**one** modal/form shell. Fork `ratatui-explorer` only if ZIP browsing,
`.flf/.tlf` filtering, and recent files are required — those are the reasons
the existing crates will not drop in cleanly.

## Prerequisite: crossterm version split

`Cargo.lock` currently contains both:

- `crossterm 0.28.1` — direct Figby dependency
- `crossterm 0.29.0` — via `ratatui-crossterm`

```text
crossterm v0.28.1
└── figby v6.2.1

crossterm v0.29.0
└── ratatui-crossterm v0.1.2
    └── ratatui v0.30.2
```

Most modern community crates (`tui-input`, `tui-file-explorer`,
`ratatui-which-key`, `tui-checkbox`) require crossterm 0.29. Upgrade Figby's
direct dependency to 0.29 before adopting any of them, otherwise their event
types will not match the app's.

## Recommended order

### P0 — low risk, clear wins

1. Upgrade `crossterm` 0.28 → 0.29.
2. Replace layer panel list with `tui-tree-widget`.
3. Replace undo/light/recent lists with `List` / `ListState` + `Scrollbar`.
4. Replace playback progress bar with `Gauge` / `LineGauge`.
5. Replace both tab strips with built-in `Tabs`.
6. Add `Scrollbar` to keybindings and scrollable panels.

### P1 — dedupe

7. Extract shared file browser; evaluate forking `ratatui-explorer`.
8. Extract shared modal/form shell; adopt `tui-popup` + `tui-input` or
   `rat-text`.
9. Convert simple dialogs to that form layer.

### P2 — keep custom

10. Canvas, font editor, timeline, palette, lighting, welcome, player,
    brush, status bar.
11. Menu: keep current mouse-capable implementation, or fork `tui-menu`
    only if menu requirements grow.

Net result: roughly **6–8 hand-rolled surfaces eliminated**, **~5 more
deduped into shared internals**, and **~10 genuinely justify staying custom**.
