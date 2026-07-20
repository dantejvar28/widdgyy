#!/usr/bin/env bash
set -euo pipefail

export PYTHONPATH="/usr/share/widdgyy${PYTHONPATH:+:${PYTHONPATH}}"
exec python /usr/share/widdgyy/main.py "$@"
