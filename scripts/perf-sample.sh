#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Sample the CPU, RSS and (optional) total GPU utilization of a running
# widdgyy process.
#
# Usage:
#   ./scripts/perf-sample.sh [SECONDS] [PID]
#
# GPU utilization is the whole GPU, including the desktop; use it for
# relative before/after comparisons on the same machine.

set -u

SECONDS_TO_SAMPLE="${1:-10}"
PID="${2:-$(pgrep -x widdgyy | head -1)}"

if [[ -z "$PID" || ! -d "/proc/$PID" ]]; then
    echo "widdgyy is not running; start it first or pass a PID" >&2
    exit 1
fi

cpu_values=()
rss_values=()
gpu_values=()

for _ in $(seq 1 "$SECONDS_TO_SAMPLE"); do
    cpu=$(ps -p "$PID" -o %cpu= | tr -d ' ')
    rss=$(ps -p "$PID" -o rss= | tr -d ' ')
    [[ -n "$cpu" ]] && cpu_values+=("$cpu")
    [[ -n "$rss" ]] && rss_values+=("$rss")
    if command -v nvidia-smi >/dev/null 2>&1; then
        gpu=$(nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits 2>/dev/null | head -1 | tr -d ' ')
        [[ -n "$gpu" ]] && gpu_values+=("$gpu")
    fi
    sleep 1
done

summarize() {
    local label=$1
    shift
    if [[ $# -eq 0 ]]; then
        echo "$label: n/a"
        return
    fi
    printf "%s: " "$label"
    printf "%s\n" "$@" | awk '
        { sum += $1; if (NR == 1 || $1 < min) min = $1; if ($1 > max) max = $1 }
        END { printf "avg=%.1f min=%.1f max=%.1f (%d samples)\n", sum/NR, min, max, NR }
    '
}

echo "widdgyy pid=$PID duration=${SECONDS_TO_SAMPLE}s"
summarize "cpu %" "${cpu_values[@]}"
summarize "rss MB" "$(printf "%s\n" "${rss_values[@]}" | awk '{print $1/1024}')"
summarize "gpu % (whole GPU)" "${gpu_values[@]}"
