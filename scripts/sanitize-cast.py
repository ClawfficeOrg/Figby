#!/usr/bin/env python3
"""Sanitize a .cast file: replace East Asian Ambiguous box-drawing chars
with ASCII equivalents so agg renders single-width correctly."""
import json
import sys
from pathlib import Path

# Box-drawing / geometric shapes -> ASCII (all width-1)
REPLACEMENTS = {
    "┌": "+", "─": "-", "┐": "+", "│": "|",
    "└": "+", "┘": "+", "├": "+", "┤": "+",
    "┬": "+", "┴": "+", "┼": "+",
    "╔": "+", "═": "=", "╗": "+", "╝": "+",
    "╚": "+", "║": "=", "╠": "+", "╣": "+",
    "╦": "+", "╩": "+", "╬": "+",
    "━": "-", "┃": "|", "┏": "+", "┓": "+",
    "┗": "+", "┛": "+",
    "╭": "+", "╮": "+", "╰": "+", "╯": "+",
}

def is_pua(ch: str) -> bool:
    """True for Unicode Private Use Area codepoints (nerd-font icons)."""
    cp = ord(ch)
    return (0xE000 <= cp <= 0xF8FF or
            0xF0000 <= cp <= 0xFFFFD or
            0x100000 <= cp <= 0x10FFFD)

def sanitize_text(text: str) -> str:
    for src, dst in REPLACEMENTS.items():
        text = text.replace(src, dst)
    # Replace PUA (nerd-font) glyphs with a single-width placeholder.
    # They have no Unicode width; agg renders them as zero-width,
    # which shifts every subsequent column left and breaks the layout.
    text = "".join("*" if is_pua(ch) else ch for ch in text)
    return text

def main():
    src = Path(sys.argv[1])
    dst = Path(sys.argv[2])

    with src.open() as f:
        header = json.loads(f.readline())
        events = []
        for line in f:
            line = line.strip()
            if not line:
                continue
            ev = json.loads(line)
            # ev = [timestamp, "o", data]
            if len(ev) == 3 and ev[1] == "o":
                ev[2] = sanitize_text(ev[2])
            events.append(ev)

    with dst.open("w") as f:
        f.write(json.dumps(header) + "\n")
        for ev in events:
            f.write(json.dumps(ev, ensure_ascii=True) + "\n")

    print(f"Sanitized {len(events)} events: {src} -> {dst}")

if __name__ == "__main__":
    main()
