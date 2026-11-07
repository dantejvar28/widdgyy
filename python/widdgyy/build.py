# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Execute a config.py and print its scene document as JSON."""

from __future__ import annotations

import json
import runpy
import sys
import traceback

from .nodes import Sticker


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: python3 -m widdgyy.build CONFIG.py", file=sys.stderr)
        return 2
    try:
        namespace = runpy.run_path(sys.argv[1], run_name="__main__")
    except Exception:
        traceback.print_exc()
        return 1

    stickers = namespace.get("stickers")
    if stickers is None:
        print(
            "config.py must define a top-level `stickers` list",
            file=sys.stderr,
        )
        return 1
    if not isinstance(stickers, (list, tuple)):
        print("`stickers` must be a list", file=sys.stderr)
        return 1

    document = []
    for index, sticker in enumerate(stickers):
        if not isinstance(sticker, Sticker):
            print(
                f"stickers[{index}] is {type(sticker).__name__}, expected a Sticker",
                file=sys.stderr,
            )
            return 1
        document.append(sticker.to_dict())

    json.dump({"stickers": document}, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
