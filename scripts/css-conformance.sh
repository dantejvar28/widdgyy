#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Compare widdgyy layout boxes with Firefox headless.
#
# Usage:
#   ./scripts/css-conformance.sh                 # all fixtures
#   ./scripts/css-conformance.sh text-basic ...  # selected fixtures
#
# Each fixture lives in tests/conformance/<name>/ and contains:
#   config.py      widdgyy scene
#   styles.css     widdgyy styles
#   index.html     equivalent browser document
#   fixture.json   optional {"width", "height", "tolerance"}
#
# Firefox is optional: fixtures are skipped cleanly when it is not installed
# or Marionette cannot start.

set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIREFOX_SCRIPT="$ROOT/scripts/firefox_layout.py"
COMPARE_SCRIPT="$ROOT/scripts/compare_layout.py"
WIDDGYY="${WIDDGYY:-$ROOT/target/debug/widdgyy}"

built=0
if [[ -z "${WIDDGYY:-}" ]]; then
    echo "building widdgyy..."
    cargo build --manifest-path "$ROOT/Cargo.toml" --quiet || exit 1
    WIDDGYY="$ROOT/target/debug/widdgyy"
    built=1
fi
if [[ ! -x "$WIDDGYY" ]]; then
    WIDDGYY="$ROOT/target/release/widdgyy"
fi
if [[ ! -x "$WIDDGYY" && $built -eq 0 ]]; then
    echo "building widdgyy..."
    cargo build --manifest-path "$ROOT/Cargo.toml" --quiet || exit 1
    WIDDGYY="$ROOT/target/debug/widdgyy"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fixtures=()
if [[ $# -gt 0 ]]; then
    for name in "$@"; do
        fixtures+=("$ROOT/tests/conformance/$name")
    done
else
    while IFS= read -r dir; do
        fixtures+=("$dir")
    done < <(find "$ROOT/tests/conformance" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort)
fi

if [[ ${#fixtures[@]} -eq 0 ]]; then
    echo "no conformance fixtures found"
    exit 0
fi

failed=0
skipped=0
passed=0

for dir in "${fixtures[@]}"; do
    name="$(basename "$dir")"
    if [[ ! -f "$dir/config.py" || ! -f "$dir/index.html" ]]; then
        echo "FAIL $name: missing config.py or index.html"
        failed=$((failed + 1))
        continue
    fi

    settings="$dir/fixture.json"
    width=800
    height=600
    tolerance=2.0
    if [[ -f "$settings" ]]; then
        eval "$(python3 - "$settings" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
print(f"width={int(data.get('width', 800))}")
print(f"height={int(data.get('height', 600))}")
print(f"tolerance={float(data.get('tolerance', 2.0))}")
PY
)"
    fi

    ff_json="$TMP/$name-firefox.json"
    python3 "$FIREFOX_SCRIPT" \
        --html "$dir/index.html" \
        --out "$ff_json" \
        --width "$width" \
        --height "$height" \
        >/dev/null 2>"$TMP/$name-firefox.log"
    status=$?
    if [[ $status -eq 3 ]]; then
        echo "skip $name: Firefox/Marionette unavailable"
        skipped=$((skipped + 1))
        continue
    fi
    if [[ $status -ne 0 ]]; then
        echo "FAIL $name: Firefox helper failed"
        sed 's/^/     /' "$TMP/$name-firefox.log" | head -5
        failed=$((failed + 1))
        continue
    fi

    read -r vw vh < <(python3 - "$ff_json" <<'PY'
import json, sys
viewport = json.load(open(sys.argv[1]))["viewport"]
print(int(viewport["width"]), int(viewport["height"]))
PY
)

    wg_json="$TMP/$name-widdgyy.json"
    "$WIDDGYY" --dump-layout --config "$dir/config.py" \
        --viewport "${vw}x${vh}" >"$wg_json" 2>"$TMP/$name-widdgyy.log"
    if [[ $? -ne 0 ]]; then
        echo "FAIL $name: widdgyy --dump-layout failed"
        sed 's/^/     /' "$TMP/$name-widdgyy.log" | head -5
        failed=$((failed + 1))
        continue
    fi

    if python3 "$COMPARE_SCRIPT" \
        --widdgyy "$wg_json" \
        --firefox "$ff_json" \
        --tolerance "$tolerance" \
        --label "$name"; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
    fi
done

echo
echo "conformance: $passed passed, $failed failed, $skipped skipped"
[[ $failed -eq 0 ]]
