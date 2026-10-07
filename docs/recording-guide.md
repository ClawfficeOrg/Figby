# Driving and Recording Figby (guide for agents)

How to control the Figby TUI from a script or an MCP client and record it as `.cast` + `.gif`.
Worked example: `assets/e2e-art/timelapse/driver/ch4_opus_lighting_v6.py`.

## 1. Architecture

```
agent / driver script ──► tool call ──► scripts/figby-mcp.py (do_tool) ──► tmux session
                                                                              └─ asciinema rec --command "figby --tui"
```

- `scripts/figby-mcp.py` is a stdio MCP server (`figby-tui-control`). It runs Figby inside a **tmux** window
  (fixed size) and injects keys/mouse with `tmux send-keys`.
- When `record` is passed to `launch`, **asciinema is the outer process** and Figby is its `--command`.
  The cast is the real PTY stream (not tmux snapshots, which `agg` misrenders).
- Two ways to reach the same `do_tool()`:
  1. **MCP attached** to the session: call the tools directly (`launch`, `keypress`, ...).
  2. **Daemon + driver scripts** (used for all timelapses): `driver/daemon.py` loads `scripts/figby-mcp.py`
     and serves `do_tool()` on a unix socket (`/tmp/figby-driver.sock`). Python scripts call it via
     `driver/fig.py: call(name, args)`. Use this when the MCP isn't attached or you want long scripted runs.

Prereqs: `tmux`, `asciinema` (3.x), `agg` on PATH; a fresh `cargo build --manifest-path figby-rs/Cargo.toml`
(the server runs `figby-rs/target/debug/figby --tui`). **Restart the MCP server / daemon after editing
`scripts/figby-mcp.py`** — it keeps old code until the process exits.

## 2. Tool reference

| Tool | Args | Notes |
|---|---|---|
| `launch` | `cols` 80–240, `rows` 24–100 (default 140×50), `figmap` (repo-relative), `record` (cast basename) | `record` writes `assets/e2e-art/recordings/<name>.cast`. Refuses if `.cast` **or** `.gif` already exists — pick a new name (`-v7`), never delete someone's recording. |
| `keypress` | `key` | `Enter Escape Tab Space Backspace Delete Up Down Left Right Home End PageUp PageDown F1..F12`, `C-a`..`C-z`, or one printable char. **No Alt combos.** |
| `type_text` | `text` (≤4096) | Literal text. Use for Alt (`"\x1b" + "f"` = Alt+F), Backspace (`"\x7f"`), Shift+arrows (`"\x1b[1;2B"` = Shift+Down, `...A` up). |
| `mouse` | `x`, `y` (0-based col,row), `action` = `down up move wheel_up wheel_down`, `button` | `move` **with** `button` = drag (SGR 32+b); without = hover. `up` must repeat the real button. A click = move, down, up. |
| `snapshot` | – | Returns `screen` (text) + `png_base64`. Use `screen` for assertions, PNG for looking. |
| `wait_for_text` | `text`, `timeout_seconds` ≤30 | Returns `{found, screen}`. |
| `play_timeline` | – | Sends `T` then Space (show timeline, play). |
| `record_stop` | `make_gif` (bool) | Quits Figby so asciinema finalises the cast. Pass `make_gif:false` if you post-process (see §5). |
| `stop` | – | Kill PTY + any active recording. Call before `launch` to start clean. |
| `resize` | `cols`, `rows` | Don't resize mid-recording. |
| `record_start` | – | **Does not work** (cannot inject asciinema into a live TUI). Always `launch` with `record`. |

## 3. Driver-script pattern

```bash
# terminal 1 (repo root) — keep running
python3 assets/e2e-art/timelapse/driver/daemon.py
# terminal 2
cd assets/e2e-art/timelapse/driver   # scripts import `from ui import *`
python3 ch4_opus_lighting_v6.py --dry       # drive only: no recording, no saves — ALWAYS do this first
python3 ch4_opus_lighting_v6.py             # record -> recordings/<name>-raw.cast
python3 ch4_opus_lighting_v6.py --render    # speed up, trim, agg -> timelapse/<name>.cast + .gif
```
(Scripts expect the repo root as cwd for relative figmap paths — the v6 docstring runs them from root with the full path; `ui.py`/`lib.py` are found via the script's own dir.)

Skeleton:

```python
from ui import *            # key text alt click drag screen snap wait tool pick side_tab set_char ...
call("stop")
r = call("launch", {"cols":140, "rows":50, "figmap":"path/in.figmap", "record":"my-take-v1-raw"})
assert "error" not in r, r
time.sleep(2.0)
for step in STEPS:
    step(); print(step.__name__, snap(), flush=True)   # snap() = png path, handy for review
print(call("record_stop", {"make_gif": False}))
```

Helper modules in `assets/e2e-art/timelapse/driver/`:
- `fig.py` socket client · `daemon.py` server · `lib.py`/`ui.py` primitives (`click`, `drag`, `alt`, `bs`, `new_image`, `new_layer`, `tool`, `pick`, `side_tab`, `set_char`, `set_size`, `stroke`, `dab`, `emitter_field`)
- `h.sh` shell helpers (`k KEY`, `t TEXT`, `alt X`, `s` snapshot) for quick manual poking
- `speedup.py in out [speed] [max_gap] [realtime_marker]` · `ch2.py` (`save_as`, keyframes, lighting) · `ch4.py` (`calibrate`, title painting)

## 4. Driving rules that bite

- **Assert after every edit.** Read `screen()` and check the value. `=`/`-` in fields can hit zoom hotkeys.
- **Calibrate canvas origin**: `ch4.calibrate()` finds the `╔` row/col and sets `ui.CX0/CY0`. Re-run after layout changes (e.g. entering lighting mode). At 140×50, 1x zoom, canvas inner origin ≈ col 34 row 16 (layout-dependent; calibrate).
- **Fixed UI coords** (140×50): toolbox col 5, rows from `TOOLS` in `ui.py`; palette FG swatch (2,23), groups at `GROUP_ROW`; side-panel tabs at row 5 (`layers` 113, `props` 121, `libs` 129).
- **Keyboard beats mouse** for dialogs. Menus: `alt("f")` then Down×N + Enter. Save As Figmap = Alt+F, Down×4, Enter, wait `Save Figmap As`, backspace the field, type path, Enter (Enter again on `Overwrite?`).
- **Alt keys** only via `type_text("\x1b"+c)`. Shift+arrows via escape sequences above.
- **Space/Enter paint** on canvas (Space fixed in 6.0.47); mouse drag painting also works. Braille tool ignores the mouse.
- **Text tool blocks** vanish on tool change unless rasterized (`Ctrl+R`, i.e. `key("C-r")`).
- **Fill** matches by char equality and ignores selections.
- **Keyframe editor** selection is unbounded: press Up×30 before Down×N. Tween needs keyframes on both start and end frames.
- **Side panel on Layers swallows `+ - D` and Shift+arrows** — switch to the Props tab before lighting work.
- **Palette hex mode (`h`)** can deadlock input; avoid.
- Sleep 0.05–0.3 s between actions; cheap UI waits hide race bugs, so prefer `wait("text")` after dialogs.

## 5. Lighting-mode cheatsheet (used by the title recordings)

Enter `G` (Props tab first). Select light with Up/Down; `P` adds point light, `D` directional; `+`/`-` intensity
(±0.1); `C` cycles selected light colour `white → warm → amber → red → magenta → blue → cyan → green`;
arrow keys move a light 1 cell, Shift+arrow moves vertically via escape sequences; `L` in the layer panel excludes
a layer from lighting (keep Shadow out). Light colour is a **multiplicative tint** — it cannot add a hue the base
colour lacks (cyan light on cyan faces ≈ invisible).
Helpers: `v5.select(i)`, `v5.nudge`, `v5.tick`, `v5.travel({light:(x,y,step)}, dt)`, `v1.intensity(delta)`.

## 6. Render pipeline (cast → gif)

`--render` does: `speedup.py raw out speed 0.7` (event deltas ÷ speed, gaps capped 0.7 s; optional marker keeps
real-time for e.g. `Playing`) → trim before the quit prompt (`Unsaved Changes` / `\x1b[?1049l`) → drop leading
welcome screen frames → append a hold → `agg --fps-cap 24 --idle-time-limit <hold+.5>`.
Use 2–3× for lighting takes, ~13× for long build-alongs. Outputs go under `assets/e2e-art/timelapse/`.

## 7. Checklist for a new take

1. `cargo build` fresh binary; start `daemon.py` (or attach MCP).
2. Copy the nearest driver (`ch4_opus_lighting_v6.py`) to a new version; change `LIT`, `TIMELAPSE`, `RAW`, and the `record` name. Names must be unused (`.cast` and `.gif`). Never overwrite approved recordings.
3. `--dry` and fix assertions. Look at PNGs from `snap()`.
4. Record, then `--render`. **Actually view the GIF/frames** before reporting it done (v6 was reported unviewed).
5. Check key rows of the final screen in text (e.g. gap rows stay open).
6. Don't commit until the user says so; new `.cast/.gif/.figmap/.py` are untracked by default.

## 8. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `refusing to overwrite` | Name collision; use a new `record` name. |
| `asciinema did not produce *.cast` | Figby never exited: end with `record_stop` (it sends quit + `n`), not `stop`. |
| Dialog filled with `verwrite --cols ...` | Someone typed asciinema into Figby. Only `launch(record=...)`. |
| GIF split down the middle / shifted columns | A tmux-snapshot cast was used. Use the asciinema PTY cast. |
| Keys land in wrong panel | Wrong side tab or open dialog; check `screen()` first. |
| MCP behaves like old code | Restart the server/daemon. |
