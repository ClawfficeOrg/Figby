#!/usr/bin/env bash
# cparity-diff.sh — differential test: Figby vs C FIGlet 2.2.5, side by side.
#
# Usage: scripts/cparity-diff.sh [path/to/figby]
#   (default: figby-rs/target/debug/figby — build it first)
#
# Builds the reference C FIGlet from c-figlet/ in a temporary directory (the
# repo is left untouched), then runs both binaries over the same matrix and
# fails on the first byte of difference. Unlike the res*.txt fixtures this needs
# no regeneration and covers every font in fonts/, so a divergence shows up
# even when nobody thought to write a scenario for it.
#
# Matrix: every font x layout/justify/direction flags x tests/input.txt, plus
# non-ASCII input, control files (by name), option order, and atoi-style args.
#
# Environment: CC (default gcc), FIGLET_BINARY (skip the build, use this C figlet).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIGBY="${1:-$ROOT/figby-rs/target/debug/figby}"
FIGBY="$(cd "$(dirname "$FIGBY")" && pwd)/$(basename "$FIGBY")"
[ -x "$FIGBY" ] || { echo "figby binary not found: $FIGBY (cargo build first)" >&2; exit 2; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ -n "${FIGLET_BINARY:-}" ]; then
    CFIG="$FIGLET_BINARY"
else
    cp "$ROOT"/c-figlet/*.c "$ROOT"/c-figlet/*.h "$ROOT"/c-figlet/Makefile "$WORK/"
    make -s -C "$WORK" CC="${CC:-gcc}" DEFAULTFONTDIR="$ROOT/fonts" figlet >/dev/null 2>&1 \
        || { echo "failed to build C figlet" >&2; exit 2; }
    CFIG="$WORK/figlet"
fi

export FIGLET_FONTDIR="$ROOT/fonts"
cd "$ROOT"

total=0
fails=0
# compare <stdin-file> <args...>
compare() {
    local input="$1"; shift
    total=$((total + 1))
    if ! cmp -s <("$CFIG" "$@" < "$input" 2>&1) <("$FIGBY" "$@" < "$input" 2>&1); then
        fails=$((fails + 1))
        if [ "$fails" -le 20 ]; then
            echo "DIFF: figby $* < ${input#"$ROOT"/}"
        fi
    fi
}
lit() { printf '%b' "$1" > "$WORK/in"; echo "$WORK/in"; }

# 1. Every font x flags, standard input text.
fonts=(fonts/*.flf tests/*.tlf tests/*.flf)
flagsets=("" "-k" "-W" "-S" "-o" "-s" "-c" "-r" "-R" "-L" "-p" "-w 40" "-w 200"
          "-m 0" "-m 1" "-m 15" "-m 63" "-kpc" "-WrR" "-oc")
for f in "${fonts[@]}"; do
    for flags in "${flagsets[@]}"; do
        # shellcheck disable=SC2086
        compare tests/input.txt -f "$f" $flags
    done
done

# 2. Long text, paragraph mode, narrow widths.
for flags in "" "-p" "-p -w 30" "-c" "-w 5"; do
    # shellcheck disable=SC2086
    compare tests/longtext.txt $flags
done

# 3. Non-ASCII input. Raw bytes go through the default ISO 2022 decoder
# (Latin-1 GR, SO/SI, single shifts); `-C utf8` gets well-formed UTF-8 only,
# because C's UTF-8 reader swallows the byte after a broken sequence and
# Figby deliberately does not (see README "Intentional differences").
for text in 'h\xe9llo \xc3\xa9\n' '\xe4\xf6\xfc\xdf\n' '\x1b*I\x8eA A\n' '\x0eAB\x0fC\n' 'a\x01b\x02c\n'; do
    in="$(lit "$text")"
    compare "$in" -f standard
    compare "$in" -f standard -D
done
for text in 'h\xc3\xa9llo w\xc3\xb6rld \xc3\x9f\n' 'a\n\xc3\xa9b\n\n\xc3\xa4\xc3\xb6\n' '\xe6\x97\xa5\xe6\x9c\xac\n'; do
    in="$(lit "$text")"
    compare "$in" -f standard -C utf8
    compare "$in" -f standard -p -C utf8
    compare "$in" -f banner -c -C utf8
done
compare "$(lit 'ABCDE')" -f banner -C uskata
compare "$(lit '\261\262\263\264\265')" -f banner -C jis0201
compare "$(lit 'h\xc3\xa9\n')" -f standard -C upper -C utf8
compare "$(lit 'h\xc3\xa9\n')" -f standard -C upper -N -C utf8

# 4. Option order and C atoi argument handling.
in="$(lit 'Hello World\n')"
for flags in "-c -l" "-l -c" "-s -k" "-k -s" "-o -W" "-W -o" "-E -D" "-k -s -k" \
             "-w 0" "-w -5" "-w abc" "-m abc" "-m -2" "-f slant.flf" "-f big -f slant"; do
    # shellcheck disable=SC2086
    compare "$in" $flags
done

echo "cparity-diff: $((total - fails))/$total identical"
[ "$fails" -eq 0 ]
