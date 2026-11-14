# FX Scripting Plan (text scripts and a node-graph editor)

Status: **proposal**, written 2026-11-13. Phase 4 (read-only node graph) is
implemented, done first because the node-graph mode was asked for first;
phases 0-3 and 5-6 are not started. See the phase 4 note in section 5.
Scope: two new ways to drive the tachyonfx effects in Figby's TUI:

1. **Script mode**: a text script that composes and runs effects against the
   canvas or a region, with timing, sequencing and parallel composition.
2. **Visual script mode**: a node-graph editor in the TUI that compiles to the
   same effect graph as the text mode.

Versions checked: `tachyonfx 0.25.1` (Cargo.lock), `ratatui 0.30.1`.

---

## 1. What tachyonfx 0.25 offers

tachyonfx runs *after* widgets render. It mutates cells already in the
ratatui `Buffer` (colours, symbols), so it layers over any widget without
changing the widget.

### 1.1 Core types

| Type | What it is |
|---|---|
| `Effect` | A shader plus its area and cell filter. `process(dt, buf, area)`, `done()`. |
| `EffectTimer` | Duration plus `Interpolation`; can reverse and reset. A `(ms, Interp)` tuple works too. |
| `EffectManager<K>` | Runs many effects per frame. `add_effect`, `add_unique_effect(key, fx)` (replaces the running effect with that key), `cancel_unique_effect`, `is_running`, `process_effects`. |
| `Interpolation` | 34 easings: Linear, Quad/Cubic/Quart/Quint/Sine/Expo/Circ/Back/Elastic/Bounce In/Out/InOut, SmoothStep, Spring, Reverse. |
| `Motion` | Direction for sweeps and slides (LeftToRight, RightToLeft, UpToDown, DownToUp). |
| `CellFilter` | Picks the cells an effect touches (see 1.4). |
| `pattern::*` | Spatial shapes for how progress spreads over an area (see 1.5). |
| `Shader` trait | Write custom effects. |
| `dsl::EffectDsl` | A runtime compiler from text to `Effect`, and `to_dsl()` back to text (see 1.6). |
| `wave`, `widget` | Wave signal helpers and widgets for visualising effects. |

### 1.2 Effect constructors (`tachyonfx::fx`)

Colour:
- `fade_from(bg, fg, t)` / `fade_to(bg, fg, t)`: fade from or to the given colours.
- `fade_from_fg(c, t)` / `fade_to_fg(c, t)`: the same, foreground only.
- `paint`, `paint_fg`, `paint_bg`: set colours over time.
- `darken`, `darken_fg`, `lighten`, `lighten_fg`: change lightness.
- `saturate`, `saturate_fg`: change saturation.
- `hsl_shift`, `hsl_shift_fg`: shift hue, saturation and lightness (tween in HSL).

Text and reveal:
- `dissolve(t)`: text breaks up into random cells. `coalesce(t)` is the reverse.
- `dissolve_to(style, t)` / `coalesce_from(style, t)`: the same, with a target style.
- `evolve`, `evolve_from`, `evolve_into`: cycle glyphs through symbol sets, optionally revealing the content underneath.

Motion and geometry:
- `sweep_in` / `sweep_out(motion, gradient_len, randomness, colour, t)`: a wipe with a gradient edge.
- `slide_in` / `slide_out`: cells slide in or out from a direction.
- `expand`, `stretch`: block-character growth or shrink.
- `explode`: content disperses from the centre.
- `translate(fx, offset, t)`: move an effect's area. `translate_buf` moves an auxiliary buffer.

Timing and composition:
- `sequence(&[..])`: run one after another.
- `parallel(&[..])`: run together; done when all are done.
- `delay(d, fx)`: start later. `sleep(d)`: a plain pause.
- `repeat(fx, mode)`: repeat by count or duration. `repeating(fx)`: forever.
- `ping_pong(fx)`: forward then backward.
- `with_duration`, `prolong_start`, `prolong_end`: clamp or pad the timing.
- `freeze_at(alpha, ..)`: hold at one progress value.
- `remap_alpha`: squeeze progress into a sub-range.
- `never_complete`, `timed_never_complete`, `run_once`: lifecycle control.
- `consume_tick`, `dispatch_event`: one-tick and event-on-start helpers.

Escape hatches (Rust only, **not available in the DSL**): `effect_fn`,
`effect_fn_buf`, `offscreen_buffer`, `translate_buf`, `dynamic_area`,
`dispatch_event`.

Deprecated: `resize_area`, `term256_colors`.

**There is no `glitch` effect in 0.25.** A glitch has to be built: register a
custom DSL effect that combines `evolve` with random `hsl_shift` bursts under
a `DissolvePattern`, or write a `Shader`.

### 1.3 Methods on `Effect`

- `.with_area(rect)`: limit the effect to a region.
- `.with_filter(CellFilter)`: limit it to matching cells.
- `.with_pattern(pattern)`: shape how progress spreads across the area.
- Reversal and other timer controls through `EffectTimer`.

### 1.4 Cell filters

`All`, `Area(rect)`, `RefArea` (a shared rect that can change later),
`FgColor(c)`, `BgColor(c)`, `Inner(margin)`, `Outer(margin)` (a border ring),
`Text`, `NonEmpty`, `AllOf`/`AnyOf`/`NoneOf`/`Not` (boolean logic),
`Layout(layout, idx)`, `PositionFn`, `EvalCell` (custom predicates), and
`Static` (cache a filter until the area changes).

### 1.5 Patterns

`RadialPattern`, `DiamondPattern`, `SpiralPattern`, `DiagonalPattern`,
`CheckerboardPattern`, `SweepPattern`, `CoalescePattern`, `DissolvePattern`,
`WavePattern`, plus the combinators `BlendPattern`, `CombinedPattern`
(`PatternOp`) and `InvertedPattern`.

### 1.6 The built-in Effect DSL (the key finding)

tachyonfx ships a runtime effect language behind the `dsl` cargo feature.
**`dsl` is a default feature, and Figby already compiles it.** `anpa 0.10.0`,
the DSL's parser dependency, is already in `Cargo.lock`, so using it adds no
new dependencies.

- The syntax is a subset of Rust expressions: "valid DSL code is valid Rust
  code with the appropriate imports". It allows `let` bindings, method
  chains, unqualified enum variants, and `(500, CircOut)` timer tuples.
  Comments are allowed.
- There is no `if`, `match` or loops. A script is a single expression.
- Compile with `EffectDsl::new().compiler().bind("name", value).compile(src)`.
  Values injected with `.bind` can be colours, `Motion`, `Rect` and so on.
- Errors are `DslParseError`, with start and end line and column,
  `context()` (the source with the error highlighted), and `error_text()`.
  That is enough to show inline diagnostics in a TUI editor.
- `EffectDsl::register("name", |args| ...)` adds custom effects such as
  `glitch`.
- `effect.to_dsl()` serialises an effect back to text (comments are lost).
- The browser editor **TachyonFX FTL** uses the same DSL, so scripts can be
  prototyped there and pasted into Figby.

Example:

```rust
let t = (600, QuadOut);
fx::sequence(&[
    fx::fade_from(Color::Black, Color::Black, t),
    fx::parallel(&[
        fx::sweep_in(LeftToRight, 12, 0, Color::Cyan, (500, SineOut)),
        fx::hsl_shift_fg([40.0, 0.0, 0.0], (800, Linear)).with_filter(Text),
    ]),
]).with_area(canvas)
```

---

## 2. What Figby uses now

All tachyonfx use is native-only. `tachyonfx` sits under
`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` with the
`std-duration` feature, and `pub mod tui` is gated by
`#[cfg(not(target_arch = "wasm32"))]` in `src/lib.rs`.

| Where | What |
|---|---|
| `src/tui/fx.rs` | Two wrappers. `WelcomeFx` is `fade_from_fg(DarkGray, 400ms QuadOut)` over the welcome dialog. `AppFadeIn` is `fade_from(Black, Black, 600ms QuadOut)` over the full frame. |
| `src/tui/app_state.rs` (~1365) | `welcome.fx: Option<WelcomeFx>`, `welcome.fade_in: Option<AppFadeIn>`, `frame.fx_last_tick`. |
| `src/tui/mod.rs` (`render`, ~77-119) | Computes `delta_time` from `fx_last_tick`, processes both effects after the widgets draw, and drops them when `done()`. |
| `src/tui/event_loop.rs` (~83-95) | In `RenderMode::Dirty`, forces a redraw while either effect is `Some`. Any new effect source must join this condition. |
| `src/tui/dispatch/*` | Many actions set `welcome.fx = None` to cancel the welcome fade. |
| `src/tui/side_panel.rs` | `TabId::Effects` exists but only renders a placeholder. This is the natural home for the new UI. |

Figby uses no `EffectManager`, no DSL, no filters and no patterns. Unrelated
to tachyonfx, Figby also has its own particle system (`particles.rs`),
lighting (`lighting.rs`) and a frame timeline (`timeline.rs`, figseq).

---

## 3. Script language: options and recommendation

| Option | New deps | Native size | wasm32 | Fits the job | Cost to us |
|---|---|---|---|---|---|
| **A. tachyonfx DSL as is** | none (already linked) | ~0 | DSL is std-only; irrelevant while the TUI is native-only | Covers every effect, combinator, filter and pattern; FTL-compatible; round-trips with `to_dsl` | Small: bindings, registry, UI |
| B. Rhai, driving a Figby API | `rhai` (~400 KB as wasm per its book; similar native) | +0.5-1 MB | Supported (`wasm32-unknown-unknown`) | Gives loops, conditions and functions, but every effect needs a hand-written binding | Medium-high: we would re-expose ~50 constructors |
| C. Lua via `mlua` | `mlua` + vendored C Lua | +~0.5 MB plus a C toolchain | Poor (C on `wasm32-unknown-unknown`) | Same as B | High, and adds a C build dependency |
| D. Our own TS-like or Python-like DSL (chumsky or winnow) | parser crate | small | fine | Whatever we design | High: grammar, errors and docs, all duplicating A |

**Recommendation: A, the tachyonfx DSL, for effect expressions, plus a thin
Figby layer on top.** On the "TypeScript-like or Python-like" question: the
DSL is Rust-like, and that is the right call for this project. It is what
tachyonfx's own docs, the FTL editor and its examples use, so anything
written in it can be copied from upstream unchanged. It also costs nothing
in binary size or dependencies. A Python-like or TS-like surface would mean
building our own parser, error reporting and docs, and keeping them in step
with every tachyonfx release.

The DSL has two gaps, and Figby fills them itself:

1. **Targets.** Before compiling, Figby binds named `Rect`s with `.bind`:
   `canvas`, `screen`, `selection`, `layer_1`..`layer_n` (layer bounds),
   `toolbox`, `side_panel`, `status`. Scripts then write
   `.with_area(selection)`.
2. **Triggers and slots.** A script file holds one or more named effects in
   a small TOML wrapper, which Figby already parses:

   ```toml
   [[effect]]
   name    = "intro"
   trigger = "on_open"        # on_open | on_save | on_key:"F7" | manual | loop
   slot    = "intro"          # EffectManager unique key: re-trigger replaces
   source  = '''
   fx::sequence(&[ fx::coalesce((500, QuadOut)), fx::sleep(300), fx::glitch(200) ])
       .with_area(canvas)
   '''
   ```

   The file extension is `.figfx.toml`.

Figby-registered custom effects: `glitch(ms)`, `pulse(color, ms)`,
`scanline(motion, ms)`, and possibly `lighting_flash` and
`particles_burst` to hook into Figby's own systems.

Revisit B (Rhai) only if users need control flow, such as "for each layer,
stagger a sweep". Even then, Rhai should *generate DSL strings* or call a
handful of builder functions. It should not replace the DSL.

---

## 4. Node editor: options and recommendation

| Crate | Version / date | Licence | ratatui | Interactive | Notes |
|---|---|---|---|---|---|
| **`rataflow`** | 0.1.0, 2026-08-18 | MIT | `^0.30` | Yes | Mouse drag, connect and reconnect; keyboard spatial and Tab navigation with rebindable actions; pan and zoom; multi-select; connection validation (Strict/Loose plus custom validators); generic `NodeContent`/`EdgeContent`; minimap; serde snapshots for undo, redo, save and restore; optional sugiyama auto-layout; crossterm, termion, termwiz and ratzilla backends. ~198 stars, 15 commits. Its dev-deps already include tachyonfx 0.25. Young: 0.1, single author. |
| `tui-nodes` | 0.10.0, 2026-01-07 | MIT (per search; docs.rs page did not show it) | `^0.30` | No: renders a node graph | Hosted on sr.ht; 1 of 15 items documented; `Graph`/`Node`/`Connection`/`NodeLayout` and a `NodeGraph` stateful widget. We would write all interaction ourselves. |
| Hand-rolled | n/a | n/a | n/a | Whatever we build | Boxes on a `Canvas` or `Buffer`, wires drawn with box-drawing characters, plus our own hit-testing. Large UX cost. |

Nothing else turned up. The ratatui third-party showcase lists only
`tui-nodes` for graphs (and `tui-tree-widget` for trees). The user's memory
of "a node editor in the ratatui ecosystem" is most likely `tui-nodes`, which
is a viewer, or `rataflow`, which is newer and a real editor.

**Recommendation: `rataflow`, wrapped behind our own graph model.**

- Figby owns the canonical `FxGraph` intermediate representation (IR). It is
  serde, in a new native-only module such as `src/tui/fx_script/graph.rs`.
  The `rataflow` graph is only a view of it, so a rataflow API break or
  abandonment costs us the view, not the data.
- Pin the exact `rataflow` version and put it behind a cargo feature
  (`fx-graph`, on by default for native) so it can be dropped if needed.
- Fallback: if rataflow proves too young, render with `tui-nodes` and add
  our own keyboard-first interaction, such as "select node, `a` to add a
  child, `c` to connect".

### 4.1 One effect graph for both modes

```
 text (.figfx.toml) ──► EffectDsl::compile ──► Effect ──► EffectManager
                               ▲
 node graph (FxGraph IR) ──► emit DSL text ──┘
```

- **Graph to text.** Each node emits a DSL fragment: `Sequence` gives
  `fx::sequence(&[..children])`, `Fade` gives `fx::fade_to_fg(c, (ms, interp))`,
  and so on. Because the graph always compiles *through the DSL*, both modes
  share one compiler, one set of errors and one runtime. The emitted text is
  also what users see in a "view as script" pane.
- **Text to graph** is harder. The DSL's AST is not known to be public (not
  verified), and `to_dsl()` goes from an `Effect` to text, not to a tree.
  Options: (a) a small winnow parser for the subset our nodes emit (calls,
  arrays, tuples, method chains); (b) graph-only files store the IR as JSON
  alongside the generated DSL. Start with (b) and add (a) later if users want
  to open hand-written scripts as graphs.

Node kinds (v1): **Effect** nodes, one per `fx::` constructor, with
parameters edited in the Props panel; **Combinator** nodes (Sequence,
Parallel, Repeat, PingPong, Delay, Prolong, Freeze); **Modifier** nodes
(Area/Target, Filter, Pattern); **Value** nodes (Colour, Timer/Interpolation,
Motion, Rect target). Edges always flow from child to parent, which gives a
tree. Validation rejects cycles and type mismatches, and rataflow's custom
validator can enforce both.

---

## 5. Phased plan (smallest useful first)

**Phase 0: plumbing (no UI).** Add `src/tui/fx_script/` (native-only,
because it sits under `tui`). Replace the two ad-hoc `Option<..Fx>` fields
with one `EffectManager<FxSlot>`, or add the manager alongside them. Add
`manager.is_running()` to the `needs_redraw` condition in `event_loop.rs`.
Port `WelcomeFx` and `AppFadeIn` to DSL strings as the first test cases.

**Phase 1: run a named effect over a region.** Add a command-palette or
keymap action, `Run effect…`. It prompts for a single DSL expression, binds
the target rects (`canvas`, `selection`, `layer_n`, `screen`), compiles,
and runs it in a `manual` slot. Show compile errors in the status bar with
line and column. Add a few built-in presets (`fade-in`, `sweep`, `dissolve`,
`coalesce`, `glitch`) and the custom `glitch` registration.

> Status 2026-11-14: **done**, with the minimum of phase 0. `fx_script/runtime.rs`
> holds one `EffectManager<FxSlot>` (slot `Manual`), ticked last in
> `TuiApp::render` and added to the `Dirty` redraw condition (plus one settle
> frame after an effect ends). `WelcomeFx`/`AppFadeIn` still run in their own
> fields; porting them is left for phase 0 proper. F7 / View > Run Effect…
> opens `fx_script/prompt.rs`; `dispatch/fx_run.rs` binds the targets from
> the last frame's layout (selection and `layer_N` mapped through canvas
> scroll and zoom; `layer_N` is the bounding box of the layer's non-blank
> cells), compiles and runs. Errors show as `fx line:column: message` in the
> status bar. `glitch(ms)` is registered in `figby_dsl()` and is also an
> `FxEffect::Glitch` leaf, so the `fade-in` and `glitch` presets live in
> `presets.rs` with the others and show in F9. The glitch pattern depends
> only on cell position and elapsed effect time (deterministic for a fixed
> `dt`). Not done: the "rasterize an effect to cell frames" addition below.

**Phase 2: script files, sequencing and triggers.** Load `.figfx.toml`
(multiple named effects, triggers, slots). Open it from the Effects
side-panel tab, which lists effects with play and stop buttons. Add
`on_open`, `on_save`, `on_key` and `loop` triggers. Support hot reload when
the file changes. Use `sequence`/`parallel`/`delay` directly from the DSL.

**Phase 3: in-TUI script editor.** Add a text area (reuse the existing TUI
text components) with live preview: recompile on pause and replay over the
target. Show inline error highlighting from `DslParseError::context()`.

**Phase 4: node graph (read-only).** Build the `FxGraph` IR and graph-to-DSL
emitter, then render the IR with rataflow in a full-screen "FX Graph"
overlay. Add a "view as script" pane.

> Status 2026-11-13: **done** (first, ahead of phases 0-3). `src/tui/fx_script/`
> holds the IR (`graph.rs`), DSL compile with target bindings (`compile.rs`),
> preset graphs (`presets.rs`) and the F9 overlay (`graph_view.rs`).
> rataflow `=0.1.0` (MIT, ratatui `^0.30`, one ratatui in the tree) sits
> behind the default `fx-graph` feature. Effect nodes carry their
> parameters inline; separate Value nodes wait for phase 5. The overlay only
> shows presets: loading or saving graph files is phase 5, and running an
> effect from the graph is phase 1.

**Phase 5: node graph editing.** Add, delete and connect nodes, and edit
parameters in the Props panel. Validate connections. Add undo and redo
(rataflow snapshots, or Figby's `undo.rs`). Save and load the IR plus the
DSL.

**Phase 6 (optional).** Add text-to-graph import (a winnow subset parser).
Bake effects into the timeline or figseq frames, rendering the effect at a
fixed `dt` per frame so it can be exported as GIF. Consider Rhai for control
flow if users ask.

---

## 6. Risks

- **wasm.** Today there is no impact: `tachyonfx` and `tui` are native-only.
  If a web build ever wants effects, tachyonfx has a `wasm` feature
  (web-time), and rataflow has a `ratzilla` feature. Both match Figby's wasm
  stack (`ratzilla 0.3`). Keep `fx_script` free of `std::fs` in its core
  (compile from `&str`) so it could move later.
- **API churn.** tachyonfx 0.x breaks between minor versions, and so does
  the DSL surface. rataflow is at 0.1. Mitigations: pin versions; own the IR;
  add golden tests on the emitted DSL text.
- **Performance.** Compile once per load or trigger, never per frame. Watch
  `PositionFn`/`EvalCell` filters and full-screen patterns on large
  terminals; wrap reused filters in `CellFilter::Static`. In `Dirty` render
  mode, an effect that never completes (`repeating`, `never_complete`) forces
  continuous redraws. Show an indicator for this and stop such effects when
  the window loses focus.
- **Interaction with editing.** Effects mutate the *rendered* buffer, not
  the document. Users may think the art changed. Make preview versus baked
  output explicit. Effects over the canvas while the user is drawing may be
  confusing, so consider pausing manual slots on input.
- **UX of a terminal node editor.** Wiring with a mouse in a terminal is
  clumsy, and many terminals and multiplexers lose drag events.
  Keyboard-first connection must work. The tree shape (combinator plus
  children) suits an outline or tree editor better than free wiring. A
  `tui-tree-widget`-style "structured editor" may beat a free-form graph.
- **Headless testing.** Use ratatui `TestBackend` (Figby already has 8 test
  files using it) and step effects with a fixed `dt`, then assert on cells.
  Randomised effects (`dissolve`, `coalesce`, `sweep` randomness, `glitch`)
  need a fixed seed. Whether tachyonfx exposes seeding for these was **not
  verified**. Test the DSL compile and error paths, graph-to-DSL emission
  (golden strings) and IR serde round-trips as plain unit tests, with no
  terminal.
- **Custom effects and `to_dsl`.** Effects registered with
  `EffectDsl::register` (such as `glitch`) may not serialise back through
  `to_dsl()`. Not verified. This matters only if we rely on `to_dsl` for the
  text view; the IR-to-DSL emitter avoids it.

---

## 7. Open questions for the user

1. Should scripted effects be preview-only (UI polish, transitions), or also
   bakeable into the timeline and GIF/figseq export? Bakeable changes the
   priority of Phase 6.
2. Is the Rust-like tachyonfx DSL acceptable as the user-facing language, or
   is a Python-like or TS-like surface a hard requirement? A hard requirement
   means a custom parser that emits DSL: Phase 1 cost grows by a few days.
3. Is a free-form node graph what you want, or would a structured tree
   editor (combinators as folders, effects as leaves) be just as good? The
   tree is far easier to use from the keyboard in a terminal.
4. Is a young dependency (`rataflow` 0.1, single maintainer) acceptable
   behind a feature flag, or do you prefer `tui-nodes` plus our own
   interaction?
5. Which targets matter beyond canvas, selection, layers and the whole
   screen? For example: per-glyph or per-text-run filters, or "cells of
   colour X" (`FgColor`).
6. Should script files live per document (next to the `.figby` file), per
   user (config directory), or both?
7. Should `glitch` match an existing look (a reference GIF or terminal
   app)? tachyonfx 0.25 has no glitch built in.
8. Do effects ever need to run in the wasm/web build?

## Rasterize an effect to cell frames (phase 1 addition)

Goal: run a tachyonfx effect for a fixed number of ticks and store each step as
a frame of terminal cells, to export or edit.

How:
- Copy the target region of the canvas into a ratatui `Buffer`.
- Step the effect with a fixed `dt` (for example 1/30 s), never wall-clock time,
  so the same script always produces the same frames.
- After each step, copy the buffer's symbols and colours into a frame.
- Output: a list of frames, which feeds `figseq` playback, the GIF exporter
  (`output.rs::export_cells_to_gif`) or the animation timeline as keyframes.

Open items:
- Determinism: confirm tachyonfx lets us fix the random seed for dissolve and
  similar effects (needed for repeatable output and headless tests).
- Size: cap frames per capture and capture a region, not the full screen.
- Custom effects (e.g. `glitch`) must be pure functions of `dt` and cell state
  to stay deterministic.
