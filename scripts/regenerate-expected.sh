#!/bin/sh
# regenerate-expected.sh — Regenerate expected test outputs from C FIGlet 2.2.5
#
# Usage: ./scripts/regenerate-expected.sh
#   Builds C figlet from c-figlet/, runs every fixture-backed scenario from
#   the authoritative scenario table (figby-rs/tests/cparity/scenarios.tsv),
#   and overwrites tests/res*.txt with byte-exact output from C figlet.
#
# The scenario table is the single source of truth for args + input
# (GPT review F-27): tests with status `special` have bespoke harness logic
# in run_tests.rs and are skipped here; `known-divergence` rows still
# regenerate a C reference (useful for tracking the gap).
#
# Environment:
#   CC — C compiler (default: gcc)
#   FIGLET_BINARY — path to C figlet (default: builds from c-figlet/)
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FONTS_DIR="$REPO_ROOT/fonts"
TESTS_DIR="$REPO_ROOT/tests"
C_FIGLET_DIR="$REPO_ROOT/c-figlet"
TABLE="$REPO_ROOT/figby-rs/tests/cparity/scenarios.tsv"

# === Build C FIGlet ===
FIGLET_BINARY="${FIGLET_BINARY:-"$C_FIGLET_DIR/figlet"}"
if [ ! -x "$FIGLET_BINARY" ]; then
    echo "Building C figlet from $C_FIGLET_DIR ..."
    CC="${CC:-gcc}"
    # Pass DEFAULTFONTDIR so the binary can resolve fonts without FIGLET_FONTDIR
    make -C "$C_FIGLET_DIR" \
        CC="$CC" \
        XCFLAGS="-DTLF_FONTS -DDEFAULTFONTDIR='\"$FONTS_DIR\"' -DDEFAULTFONTFILE='\"standard\"'" \
        figlet 2>&1 | grep -v "^make:" | grep -v redefined || true
    if [ ! -x "$FIGLET_BINARY" ]; then
        echo "ERROR: Failed to build C figlet" >&2
        exit 1
    fi
fi

# === Helper: run C figlet ===
run_figlet() {
    FIGLET_FONTDIR="$FONTS_DIR" "$FIGLET_BINARY" "$@"
}

if [ ! -f "$TABLE" ]; then
    echo "ERROR: scenario table missing: $TABLE" >&2
    exit 1
fi

echo "=== Generating expected outputs from $TABLE ==="

generated=0
skipped=0
while IFS=$'\t' read -r num args input status note; do
    # Skip comments and blank lines
    case "$num" in "" | \#*) continue ;; esac

    # Bespoke-harness scenarios have no res file.
    if [ "$status" = "special" ]; then
        skipped=$((skipped + 1))
        continue
    fi

    res="$TESTS_DIR/res$(printf '%03d' "$num").txt"

    case "$input" in
        file:*)
            fname="${input#file:}"
            cat "$TESTS_DIR/$fname" | run_figlet $args > "$res"
            ;;
        lit:*)
            literal="${input#lit:}"
            printf '%b' "$literal" | run_figlet $args > "$res"
            ;;
        *)
            echo "  WARN: test $num has unknown input spec '$input' — skipping"
            skipped=$((skipped + 1))
            continue
            ;;
    esac
    echo "  Test $num: $note"
    generated=$((generated + 1))
done < "$TABLE"

echo "=== Done: generated $generated expected output(s), skipped $skipped special scenario(s) ==="