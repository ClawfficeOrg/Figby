#!/bin/sh
# install-fonts.sh — install Figby's fonts into a directory figby searches.
#
# Usage: scripts/install-fonts.sh [DEST]
#   DEST defaults to $FIGBY_FONT_DIR, else /usr/local/share/figlet
#   (one of figby's built-in font dirs, so no flags are needed afterwards).
#   For a non-system DEST, point figby at it with `export FIGLET_FONTDIR=DEST`
#   or `figby -d DEST`.
#
# Installs the figby-fonts submodule (initialised if missing) plus the fonts
# bundled in fonts/. Bundled fonts win on name clashes.
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="${1:-${FIGBY_FONT_DIR:-/usr/local/share/figlet}}"

if [ ! -d "$ROOT/figby-fonts/fonts" ]; then
    echo "Initialising figby-fonts submodule ..."
    git -C "$ROOT" submodule update --init figby-fonts
fi

SUDO=""
mkdir -p "$DEST" 2>/dev/null || SUDO="sudo"
[ -n "$SUDO" ] && $SUDO mkdir -p "$DEST"
[ -w "$DEST" ] || SUDO="sudo"

count=0
for src in "$ROOT/figby-fonts/fonts" "$ROOT/fonts"; do
    for f in "$src"/*.flf "$src"/*.tlf "$src"/*.flc; do
        [ -e "$f" ] || continue
        $SUDO cp "$f" "$DEST/"
        count=$((count + 1))
    done
done
echo "Installed $count font files into $DEST"
case "$DEST" in
    /usr/local/share/figlet|/usr/share/figlet) ;;
    *) echo "Use them with: export FIGLET_FONTDIR=\"$DEST\"   (or figby -d \"$DEST\")" ;;
esac
