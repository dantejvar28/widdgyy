#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare widdgyy `--dump-layout` output against Firefox boxes."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--widdgyy", required=True)
    parser.add_argument("--firefox", required=True)
    parser.add_argument("--tolerance", type=float, default=2.0)
    parser.add_argument("--label", default="fixture")
    args = parser.parse_args()

    widdgyy = json.loads(Path(args.widdgyy).read_text())
    firefox = json.loads(Path(args.firefox).read_text())
    nodes = {
        node["id"]: node
        for node in widdgyy.get("nodes", [])
        if node.get("id")
    }
    boxes = firefox.get("boxes", {})

    failed = False
    for key, expected in boxes.items():
        node = nodes.get(key)
        if node is None:
            print(f"FAIL {args.label}: missing widdgyy node id={key!r}")
            failed = True
            continue
        for field in ("x", "y", "width", "height"):
            actual = float(node[field])
            wanted = float(expected[field])
            delta = abs(actual - wanted)
            if delta > args.tolerance:
                print(
                    f"FAIL {args.label}: {key}.{field} "
                    f"widdgyy={actual:.2f} firefox={wanted:.2f} delta={delta:.2f}"
                )
                failed = True
    for key, expected_lines in firefox.get("lines", {}).items():
        node = nodes.get(key)
        if node is None:
            print(f"FAIL {args.label}: missing widdgyy node id={key!r} for line boxes")
            failed = True
            continue
        actual_lines = node.get("lines", [])
        if len(actual_lines) != len(expected_lines):
            print(
                f"FAIL {args.label}: {key} has {len(actual_lines)} line(s), "
                f"Firefox has {len(expected_lines)}"
            )
            failed = True
            continue
        for index, (actual, wanted) in enumerate(zip(actual_lines, expected_lines)):
            for field in ("x", "width"):
                delta = abs(float(actual[field]) - float(wanted[field]))
                if delta > args.tolerance:
                    print(
                        f"FAIL {args.label}: {key} line {index}.{field} "
                        f"widdgyy={actual[field]:.2f} firefox={wanted[field]:.2f} "
                        f"delta={delta:.2f}"
                    )
                    failed = True

    if not failed:
        print(f"ok   {args.label}: {len(boxes)} box(es) within {args.tolerance}px")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
