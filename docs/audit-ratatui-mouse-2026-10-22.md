# Mouse and hit-testing audit — 2026-10-22

Scope: `figby-rs/src/tui/**`. Worktree `Figby-tui-mouse`, branch
`fix/tui-mouse-hit-layer`, based on `master` at `6413cf5`.

Method: read every `MouseEventKind` arm, every stored hit rect, and every
`Widget`/`StatefulWidget` impl. Cross-checked against
`clawffice-design/skills/ratatui-modern-ui/SKILL.md` section 6 (component
index) and the ratatui built-in list.

Rule this audit measures against: ratatui built-ins are render-only and never
hit-test. Anything that hit-tests must either be a crate that owns its hit test,
or hand-rolled and **z-ordered**: one place decides which layer the pointer is
in, top layer first, and an overlay consumes the event.

## 1. Inventory

### 1.1 Custom widgets (legitimate — none of these have a crate equivalent)

| widget | file:line |
| --- | --- |
| `BrushState` cursor | `brush.rs:275` |
| `CanvasWidget` | `canvas.rs:317` |
| `GlyphGridWidget` | `font_editor.rs:145` |
| `FontEditor` | `font_editor.rs:1776` |
| `MenuBar` (stateful) | `menu.rs:358` |
| `Palette` swatches | `palette.rs:575` |
| `AnimationPlayer` | `player.rs:288` |
| `CanvasSettings` | `status.rs:86` |
| `AnimationTimeline` (stateful) | `timeline.rs:1100`, `:1114` |
| `Toolbox` | `toolbox.rs:196` |
| `UndoPanel` | `undo_panel.rs:105` |
| `WelcomeScreen` | `welcome.rs:465` |

These are canvas/grid/timeline domain widgets. The finding is not "replace
them"; it is that each owns a rect with no shared ordering.

### 1.2 Stored hit rects

| rect | declared | populated | reset every frame |
| --- | --- | --- | --- |
| `ui.tab_rects` | `app_state.rs:1357` | `mod.rs:267` | no — stale on the welcome early return |
| `animation.transport_rects` | `app_state.rs:774` | `mod.rs:361` | no — stale when the timeline is hidden or too short |
| `props_panel.rects` | `props_panel.rs:58` | via `side_panel.render`, `mod.rs:326` | only inside `if let Some(right_panel)` |
| `dialogs.quit_confirm_buttons` | `app_state.rs:1309` | `overlays.rs:248` | never; consulted only while the flag is set |
| `menu_bar.header_rects` / `item_rects` | `menu.rs:71-73` | `mod.rs:209` | yes (`menu.rs:363`, `:320`) |
| `welcome.recent_rects` / `font_rects` / `image_rects` | `welcome.rs:66-68` | `welcome.rs:253-258` | yes — reassigned |
| `layers.button_rects` | `layers.rs:774` | `layers.rs:1100` | yes (`layers.rs:1045`) |
| `file_ops.entry_rects` | `file_ops.rs:181` | `file_ops.rs:1112` | yes (`:1082`) |
| `font_editor.grid_state.cell_rects` | `font_editor.rs:100` | `font_editor.rs:472` | yes (`:462`) |

## 2. Defects

### D1 — chrome answers clicks behind a dialog

`dispatch.rs:432` handles the quit-confirm dialog first and returns, but the
menu bar (`:464`) and the document tab strip (`:479`) run **before** the guards
for settings (`:505`), file ops (`:509`), export (`:526`) and rascii import
(`:530`). With any of those dialogs open, clicking a tab switches the document
behind it, and clicking the menu bar opens a menu behind it. The quit-confirm
dialog is the only overlay that actually owns the pointer.

### D2 — a hidden panel keeps its hit rects

`mod.rs:308` clears `props_panel.rects` inside `if let Some(rp) = fl.right_panel`.
Zen mode (`layout.rs:77`, `right_panel: None`) and "panel closed" never enter
that branch, so the last rects survive. `dispatch.rs:643` gates on
`side_panel.open`, not on whether the panel was drawn this frame. Result: ghost
clicks on a panel that is not on screen.

### D3 — `transport_rects` survives a hidden timeline

`mod.rs:361` assigns only when `inner.height >= 5`; `mod.rs:335` only when
`fl.timeline` is `Some`. When the timeline is hidden or the pane is squeezed,
yesterday's rects stay. `dispatch.rs:662` re-checks `timeline_visible` and the
rect area, so the damage is bounded — but the guard is the only thing stopping
a click on an old button position.

### D4 — `tab_rects` survive the welcome screen

`mod.rs:209-211` renders the menu bar and overlays for the welcome screen and
returns before the tab strip is drawn, so `tab_rects` keeps the pre-welcome
values. `dispatch.rs:490` runs before the welcome check at `:490`... in this
revision, before `:503`. The welcome cancel path pops a stale tab.

### D5 — the palette editor leaks hover and wheel

`dispatch.rs:816-828` swallows `Down/Drag/Up` for the full-screen palette
editor and returns, but `Moved` (`:852`), wheel, and the timeline
(`:662`) run earlier or fall through. Hover effects and timeline seeks reach
widgets under an opaque overlay.

### D6 — scroll state is owned by `dispatch.rs`

Five panes clamp their own scroll offsets in the event handler instead of the
pane that draws the content: welcome (`welcome.rs:107-115`), file ops
(`file_ops.rs:1043-1056`), the font grid (`font_editor.rs:450-485`), the
timeline (`dispatch.rs:737-776`) and the keybindings overlay
(`dispatch.rs:1350-1366`, drawn at `overlays.rs:70`). `dispatch.rs` therefore
has to know each pane's content height.

## 3. Plan

| # | change | files |
| --- | --- | --- |
| 1 | one `PointerLayer` enum, top-down match before any chrome | `dispatch.rs` |
| 2 | clear `tab_rects`, `transport_rects`, `props_panel.rects` at the top of `render` | `mod.rs`, `mod.rs:335-371` |
| 3 | timeline: clear when it is not drawn | `mod.rs` |
| 4 | welcome: tab rects are already cleared by (2); click cannot reach them | `mod.rs` |
| 5 | palette editor: consume every event kind, move above chrome | `dispatch.rs` |
| 6 | move scroll clamping into each pane; `dispatch.rs` only calls `scroll_by` | welcome, file_ops, font_editor, timeline, overlays |

Not planned: replacing the 12 custom widgets with crates. No crate covers a
canvas, a glyph grid, a timeline, a layer stack or a swatch palette; ratcn's
closest components (List, Tabs, Progress) are restyle-only and the index marks
them "no gap". `tui-widget-list` would replace the file-ops list and the
welcome recent list, tracked separately because it changes rendering, not
hit-testing.

## 4. Verification

- `cargo test --manifest-path figby-rs/Cargo.toml`
- `cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets -- -D warnings`
- New tests seed the hit rects the render pass would store (same pattern as
  `test_doc_tab_click_switches_document`), then assert a dialog swallows the
  click, and that a render with the panels hidden leaves no rects behind.
