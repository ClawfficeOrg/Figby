#!/usr/bin/env python3
"""Generate linked Figby TUI demo recordings: pixel art, then animated polish."""
import json
import os
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
ART = ROOT / "assets/e2e-art"
REC = ART / "recordings"
BIN = ROOT / "figby-rs/target/debug/figby"
WIDTH, HEIGHT = 64, 24


def cell(ch=" ", fg=None, bg=None):
    return {"ch": ch, "fg": fg, "bg": bg, "height": None}


def stamp(grid, x, y, lines, fg, transparent=" "):
    for dy, row in enumerate(lines):
        for dx, ch in enumerate(row):
            if ch != transparent and 0 <= y + dy < HEIGHT and 0 <= x + dx < WIDTH:
                grid[y + dy][x + dx] = cell(ch, fg)


def figlet_rows(text):
    run = subprocess.run([str(BIN), "-f", "fonts/big", text], cwd=ROOT,
                         capture_output=True, text=True, check=True)
    return run.stdout.rstrip("\n").splitlines()


def base_canvas():
    grid = [[cell(" ", bg="#070B18") for _ in range(WIDTH)] for _ in range(HEIGHT)]
    for y in range(HEIGHT):
        for x in range(WIDTH):
            if (x * 31 + y * 17 + x * y) % 53 == 0:
                grid[y][x] = cell(".", "#3B5278", "#070B18")

    title = figlet_rows("FIGBY")
    title_x = (WIDTH - max(map(len, title))) // 2
    stamp(grid, title_x, 2, title, "#F8C66B")

    stamp(grid, 7, 13, ["  /\\  ", " /##\\ ", "  ||  ", " /||\\ "], "#76D39B")
    stamp(grid, 50, 13, ["  /\\  ", " /##\\ ", "  ||  ", " /||\\ "], "#76D39B")
    stamp(grid, 25, 15, ["..####..", ".######.", "########", "##.##.##"], "#67C9E8")
    stamp(grid, 0, 21, ["=" * WIDTH], "#36516B")
    stamp(grid, 0, 22, ["~" * WIDTH], "#1D344A")
    stamp(grid, 0, 23, ["-" * WIDTH], "#13283C")
    return grid


def layer(name, grid):
    return {
        "buffer": {"cells": grid, "width": WIDTH, "height": HEIGHT},
        "name": name, "visible": True, "locked": False, "opacity": 255,
        "blend_mode": "Normal", "mask": None, "group": None, "link": None,
        "accepts_lighting": True, "casts_shadow": True,
    }


def write_scenes():
    base = base_canvas()
    static = {
        "version": 1, "kind": "Image", "width": WIDTH, "height": HEIGHT,
        "layers": [layer("FIGBY garden", base)], "groups": [], "links": [],
        "active_layer": 0, "timeline": None, "palette": [], "lights": [],
    }
    (ART / "figby-garden.figmap").write_text(json.dumps(static, indent=1) + "\n")

    frames = []
    offsets = [(-2, 0), (-1, -1), (0, 0), (1, 1), (2, 0), (0, -1)]
    for idx, (dx, dy) in enumerate(offsets):
        grid = [[dict(c) for c in row] for row in base]
        # Ember and firefly trails shift around FIGBY like a small particle pass.
        for p in range(9):
            x = 7 + (p * 7 + idx * 5) % 50
            y = 10 + (p * 3 + idx * 2) % 10
            if grid[y][x]["ch"] == " ":
                grid[y][x] = cell(["*", ".", "+"][((p + idx) % 3)],
                                  ["#FFB35C", "#75E8FF", "#F8DB82"][(p + idx) % 3])
        thumb = [[grid[y][x]["ch"] for x in range(0, WIDTH, 8)]
                 for y in range(0, HEIGHT, 6)]
        frames.append({
            "thumbnail": thumb, "has_keyframe": True, "label": f"K{idx + 1}",
            "delay": 70,
            "document_state": [{"cells": grid, "width": WIDTH, "height": HEIGHT}],
            "layer_keyframes": [{"position_offset": [dx, dy], "opacity": 255,
                                 "blend_mode": "Normal"}],
        })

    animation = {
        "version": 1, "kind": "Animation", "width": WIDTH, "height": HEIGHT,
        "layers": [layer("FIGBY / light stage", base)], "groups": [], "links": [],
        "active_layer": 0,
        "timeline": {
            "fps": 8, "loop_enabled": True, "frames": frames,
            "light_keyframes": [
                {"time": 0.0, "light_index": 1,
                 "properties": {"position": [5.0, 5.0, 5.0], "intensity": 0.7,
                                "color": "#FFB45E", "direction": None, "attenuation": None},
                 "easing": "EaseIn"},
                {"time": 0.5, "light_index": 1,
                 "properties": {"position": [32.0, 4.0, 5.0], "intensity": 1.4,
                                "color": "#72DDF4", "direction": None, "attenuation": None},
                 "easing": "EaseIn"},
                {"time": 1.0, "light_index": 1,
                 "properties": {"position": [58.0, 6.0, 5.0], "intensity": 0.75,
                                "color": "#FFC96B", "direction": None, "attenuation": None},
                 "easing": "EaseIn"},
            ],
        },
        "palette": [],
        "lights": [
            {"Ambient": {"intensity": 0.45, "color": "#8AA6FF", "target": "Both"}},
            {"Point": {"position": [5.0, 5.0, 5.0], "intensity": 1.0,
                        "color": "#FFB45E",
                        "attenuation": {"constant": 1.0, "linear": 0.08, "quadratic": 0.025},
                        "target": "Both"}},
        ],
    }
    (ART / "figby-keyframes.figmap").write_text(json.dumps(animation, indent=1) + "\n")


def tmux(sock, *args, check=True):
    return subprocess.run(["tmux", "-L", sock, *args], cwd=ROOT, check=check,
                          capture_output=True, text=True, timeout=30)


def record(name, path, animate=False):
    sock = f"figbydemo{os.getpid()}"
    session = "demo"
    cast_path = REC / f"{name}.cast"
    gif_path = REC / f"{name}.gif"
    if cast_path.exists() or gif_path.exists():
        raise FileExistsError(f"refusing to overwrite {cast_path.name} or {gif_path.name}")
    command = (
        "asciinema rec --capture-input --window-size 140x50 "
        f"--idle-time-limit 1 --command '{BIN} --tui' '{cast_path.relative_to(ROOT)}'"
    )
    tmux(sock, "new-session", "-d", "-s", session, "-x", "140", "-y", "50",
         "-c", str(ROOT), command)

    def key(*keys, delay=0.3):
        for key in keys:
            if key == "Space":
                tmux(sock, "send-keys", "-t", session, "Space")
            elif len(key) == 1:
                tmux(sock, "send-keys", "-t", session, "-l", key)
            else:
                tmux(sock, "send-keys", "-t", session, key)
        time.sleep(delay)

    def type_text(text, delay=0.035):
        for char in text:
            tmux(sock, "send-keys", "-t", session, "-l", char)
            time.sleep(delay)

    def capture(delay=0.5):
        time.sleep(delay)
        return tmux(sock, "capture-pane", "-t", session, "-p", "-e").stdout

    try:
        capture(3.0)
        key("Escape")
        # FontEditor Overview Ctrl+O opens the figmap-capable project dialog.
        key("C-o")
        type_text(str(path.relative_to(ROOT)))
        key("Enter", delay=1.0)
        state = capture(1.0)
        if "64x24" not in state or "FIGBY" not in state:
            raise RuntimeError(f"{path.name} did not open in TUI canvas")

        if animate:
            # Show the two-light rig, then play loaded six-frame timeline.
            key("G")
            lighting = capture(0.8)
            if "Lighting Editor" not in lighting:
                raise RuntimeError("G did not enter Lighting Editor")
            key("Escape")
            capture(0.3)
            # Animation menu's Play / Pause item is the reliable scoped path.
            key("M-a")
            capture(0.4)
            key("Escape")
            key("Space")
            playback_states = []
            for _ in range(10):
                playback_states.append(capture(0.35))
            visible_playback = re.sub(
                r"\x1b\[[0-?]*[ -/]*[@-~]", "", "\n".join(playback_states)
            )
            progress = set(re.findall(r"([1-6])/6", visible_playback))
            if len(progress) < 2:
                raise RuntimeError(f"timeline did not visibly advance: {sorted(progress)}")
            key("Escape")
            capture(0.6)
        else:
            # Reveal the real layer drawer and timeline strip around the
            # established FIGlet + pixel-art composition.
            key("?")
            capture(0.7)
            key("T")
            capture(0.8)
            key("Escape")
            capture(0.6)
        key("C-c", delay=0.2)
        key("C-d", delay=0.2)
        time.sleep(0.5)
    finally:
        tmux(sock, "kill-session", "-t", session, check=False)

    if not cast_path.is_file() or cast_path.stat().st_size < 1000:
        raise RuntimeError(f"asciinema did not record {cast_path.name}")
    subprocess.run(["agg", "--quiet", "--fps-cap", "24", "--idle-time-limit", "1",
                    str(cast_path), str(gif_path)], cwd=ROOT, check=True)
    meta = json.loads(cast_path.read_text().splitlines()[0])
    if meta.get("term", {}).get("cols") != 140 or meta.get("term", {}).get("rows") != 50:
        raise RuntimeError(f"unexpected recording dimensions in {cast_path.name}: {meta}")
    print(f"{cast_path.relative_to(ROOT)} and {gif_path.relative_to(ROOT)} recorded at 140x50")


def main():
    if not BIN.exists():
        raise SystemExit("Build first: cargo build --manifest-path figby-rs/Cargo.toml")
    REC.mkdir(parents=True, exist_ok=True)
    write_scenes()
    record("figby-sketch-final", ART / "figby-garden.figmap")
    record("figby-keyframe-light-show-final", ART / "figby-keyframes.figmap", animate=True)


if __name__ == "__main__":
    main()
