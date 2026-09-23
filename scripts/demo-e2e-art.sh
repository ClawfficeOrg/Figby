#!/bin/bash
# Narrated manual-style demo of the e2e-art scenes for asciinema.
# Run under `asciinema rec`: typing is slow, deliberate, human-paced —
# the point is a recording that looks hand-driven, not a test script.
# Usage: ./scripts/demo-e2e-art.sh
set -u

type_out() { # $1=text — slow human typing into the shell
    local text="$1" i=0
    while [ "$i" -lt "${#text}" ]; do
        printf '%s' "${text:$i:1}"
        sleep 0.05
        i=$((i+1))
    done
    sleep 0.4
    printf '\n'
}

run() { # $1=label $2...=command — show the command, then run it
    echo "\$ $*"
    sleep 0.6
    "$@"
}

pause() { # $1=message — beat for the viewer to look at the art
    echo ""
    echo "-- $1 --"
    sleep "$2"
}

BIN=./figby-rs/target/debug/figby
PV="python3 scripts/preview-figmap.py"
ART=assets/e2e-art

clear
echo "Figby e2e-art — tribute sprites, 8-bit backdrop, animated banner"
sleep 1.5

echo ""
echo "First, the headless contract — scenes load, layers check out:"
sleep 0.8
type_out "cargo test --manifest-path figby-rs/Cargo.toml --test e2e_art"
cargo test --manifest-path figby-rs/Cargo.toml --test e2e_art 2>&1 | tail -6
pause "4/4 green — now the art itself" 1.5

clear
echo "Scene 1/5 — plumber tribute (night gradient, hills, sparkles)"
sleep 1
run $PV $ART/sprite-plumber-tribute.figmap --hold 3
pause "cap, face, overalls, boots — drawn freehand-style on the Hero layer" 1

clear
echo "Scene 2/5 — quest tribute (day gradient, sword arm raised)"
sleep 1
run $PV $ART/sprite-quest-tribute.figmap --hold 3
pause "hood, tunic, sword — an homage, not a copy" 1

clear
echo "Scene 3/5 — invader tribute (symmetric bug, antennae + claws)"
sleep 1
run $PV $ART/sprite-invader-tribute.figmap --hold 3
pause "the arcade silhouette, re-drawn from scratch" 1

clear
echo "Scene 4/5 — castle backdrop: Sky / Castle / Hero layers"
sleep 1
run $PV $ART/layers-castle-tribute.figmap --hold 4
pause "keep + twin towers + brick ground, hero visiting from scene 1" 1.5

clear
echo "Scene 5/5 — FIGBY banner finale: real fonts/big glyphs, gold + halo"
sleep 1
run $PV $ART/banner-figby-finale.figmap --frames 2
pause "6 twinkle/ember frames + sweeping point light — then the real player" 1.5

clear
echo "And the real TUI player, same file, looped:"
sleep 1
type_out "$BIN --play $ART/banner-figby-finale.figmap --loop"
timeout 8 "$BIN" --play "$ART/banner-figby-finale.figmap" --loop || true
echo ""
echo "done — assets/e2e-art, scripts/gen-e2e-art.py, scripts/e2e-art-tmux.sh"
