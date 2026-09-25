#!/usr/bin/env python3
"""Local stdio MCP server for driving and recording Figby's TUI over tmux."""
import base64
import io
import json
import os
from pathlib import Path
import re
import shutil
import unicodedata
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "figby-rs/target/debug/figby"
RECORDINGS = ROOT / "assets/e2e-art/recordings"
COLS, ROWS = 140, 50
SERVER_NAME = f"figby-mcp-{os.getpid()}"
server = None

TOOLS = [
    {"name": "launch", "description": "Launch Figby TUI in a controlled PTY. Optionally open workspace-relative figmap.",
     "inputSchema": {"type": "object", "properties": {
        "cols": {"type": "integer", "minimum": 80, "maximum": 240},
        "rows": {"type": "integer", "minimum": 24, "maximum": 100},
        "figmap": {"type": "string", "description": "Workspace-relative .figmap path"}},
        "additionalProperties": False}},

    {"name": "resize", "description": "Resize Figby PTY; screen capture uses same dimensions.",
     "inputSchema": {"type": "object", "properties": {
         "cols": {"type": "integer", "minimum": 80, "maximum": 240},
         "rows": {"type": "integer", "minimum": 24, "maximum": 100}},
         "required": ["cols", "rows"], "additionalProperties": False}},
    {"name": "keypress", "description": "Send named key to Figby: Enter, Escape, Tab, Space, arrows, F1..F12, tmux keys such as C-o, or one literal printable key.",
     "inputSchema": {"type": "object", "properties": {"key": {"type": "string"}}, "required": ["key"], "additionalProperties": False}},
    {"name": "type_text", "description": "Type literal text into Figby. Does not interpret key names.",
     "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"], "additionalProperties": False}},
    {"name": "mouse", "description": "Send an xterm SGR mouse event at terminal column,row (zero-based).",
     "inputSchema": {"type": "object", "properties": {
         "x": {"type": "integer", "minimum": 0}, "y": {"type": "integer", "minimum": 0},
         "action": {"type": "string", "enum": ["down", "up", "move", "wheel_up", "wheel_down"]},
         "button": {"type": "string", "enum": ["left", "middle", "right"]}},
         "required": ["x", "y", "action"], "additionalProperties": False}},
    {"name": "snapshot", "description": "Capture current Figby terminal screen as text; also appends frame when recording.",
     "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False}},
    {"name": "wait_for_text", "description": "Wait up to timeout for text to appear in current screen.",
     "inputSchema": {"type": "object", "properties": {
         "text": {"type": "string"}, "timeout_seconds": {"type": "number", "minimum": 0.1, "maximum": 30}},
         "required": ["text"], "additionalProperties": False}},
    {"name": "play_timeline", "description": "Show timeline and start its in-place animation playback.",
     "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False}},
    {"name": "record_start", "description": "Start asciicast recording from live PTY captures. Refuses overwrite.",
     "inputSchema": {"type": "object", "properties": {
         "name": {"type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"}},
         "required": ["name"], "additionalProperties": False}},
    {"name": "record_stop", "description": "Stop current recording and write .cast; optionally build matching .gif when agg installed.",
     "inputSchema": {"type": "object", "properties": {"make_gif": {"type": "boolean"}}, "additionalProperties": False}},
    {"name": "stop", "description": "Stop Figby PTY and active recording; restore terminal/session resources.",
     "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False}},
]


def tmux(*args, check=True, timeout=10):
    if server is None:
        raise RuntimeError("Figby is not launched")
    return subprocess.run(["tmux", "-L", SERVER_NAME, *map(str, args)], cwd=ROOT,
                          capture_output=True, text=True, check=check, timeout=timeout)


def require_session():
    if server is None:
        raise RuntimeError("Figby is not launched; call launch first")


def snapshot_image():
    require_session()
    try:
        from PIL import Image, ImageDraw, ImageFont
    except ImportError as exc:
        raise RuntimeError("PNG snapshots require Pillow (python3 -m pip install pillow)") from exc
    with tempfile.NamedTemporaryFile(prefix="figby-mcp-", suffix=".png", delete=False) as temp:
        image_path = Path(temp.name)
    try:
        tmux("capture-pane", "-t", SERVER_NAME, "-p")
        result = tmux("capture-pane", "-t", SERVER_NAME, "-e", "-C")
        converter = shutil.which("ansilove")
        if converter:
            subprocess.run([converter, "-o", str(image_path), "-"], input=result.stdout,
                           text=True, cwd=ROOT, check=True, timeout=10)
        else:
            font = ImageFont.load_default(size=14)
            ansi = re.compile(r"\x1b\[([0-9;]*)m")
            palette = [(0, 0, 0), (205, 49, 49), (13, 188, 121), (229, 229, 16),
                       (36, 114, 200), (188, 63, 188), (17, 168, 205), (229, 229, 229),
                       (102, 102, 102), (241, 76, 76), (35, 209, 139), (245, 245, 67),
                       (59, 142, 234), (214, 112, 214), (41, 184, 219), (255, 255, 255)]
            fg, bg = (220, 220, 232), (13, 13, 26)
            cell_w, cell_h = 10, 20
            image = Image.new("RGB", (COLS * cell_w, ROWS * cell_h), bg)
            draw = ImageDraw.Draw(image)
            for y, source_line in enumerate(result.stdout.splitlines()[:ROWS]):
                x = 0
                line = ansi.sub(lambda match: "\x00" + match.group(1) + "\x00", source_line)
                parts = line.split("\x00")
                for index, segment in enumerate(parts):
                    if index % 2 == 1:
                        codes = [int(code) for code in segment.split(";") if code]
                        i = 0
                        while i < len(codes):
                            code = codes[i]
                            if code == 0:
                                fg, bg = (220, 220, 232), (13, 13, 26)
                            elif 30 <= code <= 37:
                                fg = palette[code - 30]
                            elif 90 <= code <= 97:
                                fg = palette[code - 90 + 8]
                            elif 40 <= code <= 47:
                                bg = palette[code - 40]
                            elif 100 <= code <= 107:
                                bg = palette[code - 100 + 8]
                            elif code in (38, 48) and i + 4 < len(codes) and codes[i + 1] == 2:
                                color = tuple(codes[i + 2:i + 5])
                                if code == 38:
                                    fg = color
                                else:
                                    bg = color
                                i += 4
                            elif code == 7:
                                fg, bg = bg, fg
                            i += 1
                        continue
                    for char in segment:
                        if char == "\x00":
                            continue
                        width = 2 if unicodedata.east_asian_width(char) in ("W", "F") else 1
                        if width == 0:
                            continue
                        left = x * cell_w
                        draw.rectangle((left, y * cell_h, left + width * cell_w, (y + 1) * cell_h), fill=bg)
                        draw.text((left, y * cell_h), char, fill=fg, font=font)
                        x += width
                        if x >= COLS:
                            break
            image.save(image_path, format="PNG")
        data = image_path.read_bytes()
        if not data.startswith(b"\x89PNG\r\n\x1a\n"):
            raise RuntimeError("snapshot renderer did not produce a PNG")
        return base64.b64encode(data).decode("ascii")
    finally:
        image_path.unlink(missing_ok=True)


def capture_screen():
    require_session()
    colored = tmux("capture-pane", "-p", "-e", "-t", SERVER_NAME).stdout.rstrip("\n")
    plain = tmux("capture-pane", "-p", "-t", SERVER_NAME).stdout.rstrip("\n")
    if server.get("recording"):
        record = server["recording"]
        now = time.monotonic() - record["started"]
        record["events"].append([round(now, 3), "o", "\x1b[2J\x1b[H" + colored + "\n"])
    return plain


def launch(args):
    global server, COLS, ROWS
    if server is not None:
        stop({})
    if not BIN.is_file():
        raise RuntimeError("Figby binary missing; build with cargo build --manifest-path figby-rs/Cargo.toml")
    COLS = max(80, min(240, int(args.get("cols", 140))))
    ROWS = max(24, min(100, int(args.get("rows", 50))))
    tmux_args = ["tmux", "-L", SERVER_NAME, "new-session", "-d", "-s", SERVER_NAME,
                 "-x", str(COLS), "-y", str(ROWS), "-c", str(ROOT),
                 str(BIN), "--tui"]
    subprocess.run(tmux_args, cwd=ROOT, check=True, capture_output=True, timeout=10)
    server = {"cols": COLS, "rows": ROWS, "recording": None, "started": time.monotonic()}
    tmux("set-option", "-t", SERVER_NAME, "window-size", "manual")
    time.sleep(1.5)
    tmux("send-keys", "-t", SERVER_NAME, "Escape")
    time.sleep(0.2)
    if args.get("figmap"):
        open_figmap(args["figmap"])
    return {"cols": COLS, "rows": ROWS, "screen": capture_screen()}


def open_figmap(relative_path):
    path = (ROOT / relative_path).resolve()
    if not path.is_relative_to(ROOT) or path.suffix.lower() != ".figmap" or not path.is_file():
        raise ValueError("figmap must be an existing workspace-relative .figmap")
    if "Font Editor" not in tmux("capture-pane", "-p", "-t", SERVER_NAME).stdout:
        raise RuntimeError("Figmap open currently requires Font Editor mode")
    # Startup lands in Font Editor Overview, whose Ctrl+O opens figmaps.
    tmux("send-keys", "-t", SERVER_NAME, "Escape")
    time.sleep(0.15)
    tmux("send-keys", "-t", SERVER_NAME, "C-o")
    time.sleep(0.25)
    tmux("send-keys", "-t", SERVER_NAME, "-l", path.relative_to(ROOT).as_posix())
    time.sleep(0.15)
    tmux("send-keys", "-t", SERVER_NAME, "Enter")
    time.sleep(0.5)


def do_tool(name, args):
    global server, COLS, ROWS
    if name == "launch":
        return launch(args)
    if name == "stop":
        was_running = server is not None
        recording = stop_recording(False) if server and server.get("recording") else None
        subprocess.run(["tmux", "-L", SERVER_NAME, "kill-session", "-t", SERVER_NAME],
                       cwd=ROOT, capture_output=True, timeout=10)
        server = None
        return {"stopped": was_running, "recording": recording}
    require_session()
    if name == "resize":
        cols = max(80, min(240, int(args["cols"])))
        rows = max(24, min(100, int(args["rows"])))
        tmux("resize-window", "-t", SERVER_NAME, "-x", cols, "-y", rows)
        COLS, ROWS = cols, rows
        server["cols"], server["rows"] = cols, rows
        time.sleep(0.2)
        return {"cols": cols, "rows": rows, "screen": capture_screen()}
    if name == "keypress":
        key = args["key"]
        if key == "Space":
            tmux("send-keys", "-t", SERVER_NAME, "Space")
        elif len(key) == 1 and key.isprintable():
            tmux("send-keys", "-t", SERVER_NAME, "-l", key)
        elif re.fullmatch(r"C-[a-z]", key) or key in {
            "Enter", "Escape", "Tab", "Backspace", "Delete", "Up", "Down",
            "Left", "Right", "Home", "End", "PageUp", "PageDown",
            *(f"F{i}" for i in range(1, 13))
        }:
            tmux("send-keys", "-t", SERVER_NAME, key)
        else:
            raise ValueError("invalid key name or printable key")
        time.sleep(0.08)
        return {"sent": key}
    if name == "type_text":
        text = args["text"]
        if len(text) > 4096 or "\x00" in text:
            raise ValueError("text too long or contains NUL")
        tmux("send-keys", "-t", SERVER_NAME, "-l", text)
        return {"typed_characters": len(text)}
    if name == "mouse":
        x, y = int(args["x"]), int(args["y"])
        if x >= COLS or y >= ROWS:
            raise ValueError(f"coordinates exceed PTY bounds {COLS}x{ROWS}")
        action = args["action"]
        button = {"left": 0, "middle": 1, "right": 2}.get(args.get("button", "left"), 0)
        if action == "move":
            code, final = 32, "M"
        elif action.startswith("wheel"):
            code, final = 64 + (1 if action == "wheel_down" else 0), "M"
        elif action == "up":
            code, final = 3, "m"
        else:
            code, final = button, "M"
        sgr = f"\x1b[<{code};{x + 1};{y + 1}{final}"
        tmux("send-keys", "-t", SERVER_NAME, "-l", sgr)
        time.sleep(0.05)
        return {"sent": action, "x": x, "y": y}
    if name == "snapshot":
        screen = capture_screen()
        image = snapshot_image()
        return {"cols": COLS, "rows": ROWS, "screen": screen,
                "png_base64": image}
    if name == "wait_for_text":
        needle = args["text"]
        timeout = max(0.1, min(30.0, float(args.get("timeout_seconds", 5))))
        deadline = time.monotonic() + timeout
        while True:
            screen = capture_screen()
            if needle in screen:
                return {"found": True, "screen": screen}
            if time.monotonic() >= deadline:
                return {"found": False, "screen": screen}
            time.sleep(0.1)
    if name == "play_timeline":
        tmux("send-keys", "-t", SERVER_NAME, "-l", "T")
        time.sleep(0.2)
        tmux("send-keys", "-t", SERVER_NAME, "Space")
        time.sleep(0.1)
        return {"started": True, "screen": capture_screen()}
    if name == "record_start":
        if server.get("recording"):
            raise RuntimeError("recording already active")
        name = args["name"]
        cast_path = RECORDINGS / f"{name}.cast"
        gif_path = cast_path.with_suffix(".gif")
        if cast_path.exists() or gif_path.exists():
            raise FileExistsError(f"refusing to overwrite {name}.cast or {name}.gif; choose another name")
        RECORDINGS.mkdir(parents=True, exist_ok=True)
        server["recording"] = {"name": name, "started": time.monotonic(), "events": []}
        capture_screen()
        return {"recording": name, "cols": COLS, "rows": ROWS}
    if name == "record_stop":
        return stop_recording(bool(args.get("make_gif", True)))
    raise ValueError(f"unknown tool: {name}")


def stop_recording(make_gif):
    record = server.get("recording") if server else None
    if record is None:
        raise RuntimeError("no recording is active")
    capture_screen()
    path = RECORDINGS / f"{record['name']}.cast"
    gif_path = path.with_suffix(".gif")
    if path.exists() or gif_path.exists():
        server["recording"] = record
        raise FileExistsError(f"refusing to overwrite {path.name} or {gif_path.name}")
    server["recording"] = None
    header = {"version": 3, "term": {"cols": COLS, "rows": ROWS},
              "timestamp": int(time.time()), "duration": round(time.monotonic() - record["started"], 3),
              "command": f"Figby TUI ({server['cols']}x{server['rows']})",
              "env": {"TERM": "xterm-256color"}}
    with path.open("w", encoding="utf-8") as cast:
        cast.write(json.dumps(header) + "\n")
        for timestamp, event_type, data in record["events"]:
            cast.write(json.dumps([timestamp, event_type, data], ensure_ascii=True) + "\n")
    gif_path = path.with_suffix(".gif")
    if make_gif and shutil.which("agg"):
        subprocess.run(["agg", "--quiet", "--fps-cap", "24", "--idle-time-limit", "1",
                        str(path), str(gif_path)], cwd=ROOT, check=True)
    return {"cast": str(path.relative_to(ROOT)), "gif": str(gif_path.relative_to(ROOT)) if gif_path.exists() else None,
            "events": len(record["events"]), "duration_seconds": header["duration"],
            "dimensions": f"{COLS}x{ROWS}"}


def response(message_id, result=None, error=None):
    message = {"jsonrpc": "2.0", "id": message_id}
    if error is not None:
        message["error"] = error
    else:
        message["result"] = result
    sys.stdout.write(json.dumps(message, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def handle(message):
    method = message.get("method")
    params = message.get("params", {})
    message_id = message.get("id")
    if method == "notifications/initialized":
        return
    if method == "notifications/cancelled":
        return
    if method == "initialize":
        response(message_id, {"protocolVersion": params.get("protocolVersion", "2024-11-05"),
                              "capabilities": {"tools": {"listChanged": False}},
                              "serverInfo": {"name": "figby-tui-control", "version": "0.1.0"}})
        return
    if method == "ping":
        response(message_id, {})
        return
    if method == "tools/list":
        response(message_id, {"tools": TOOLS})
        return
    if method == "tools/call":
        try:
            name = params.get("name", "")
            result = do_tool(name, params.get("arguments") or {})
            content = [{"type": "text", "text": json.dumps(result, ensure_ascii=False)}]
            if name == "snapshot" and "png_base64" in result:
                content.append({"type": "image", "data": result["png_base64"], "mimeType": "image/png"})
                result.pop("png_base64")
                content[0]["text"] = json.dumps(result, ensure_ascii=False)
            response(message_id, {"content": content})
        except Exception as exc:
            response(message_id, {"content": [{"type": "text", "text": str(exc)}], "isError": True})
        return
    if message_id is not None:
        response(message_id, error={"code": -32601, "message": f"Method not found: {method}"})


def main():
    def cleanup(_signum=None, _frame=None):
        if server is not None:
            do_tool("stop", {})
        raise SystemExit(0)

    signal.signal(signal.SIGTERM, cleanup)
    signal.signal(signal.SIGINT, cleanup)
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            break
        if not line.strip():
            continue
        try:
            handle(json.loads(line))
        except Exception as exc:
            print(f"figby-mcp: {exc}", file=sys.stderr, flush=True)
    if server is not None:
        do_tool("stop", {})


if __name__ == "__main__":
    main()
