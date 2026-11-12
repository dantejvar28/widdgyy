# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Night Train: a cinematic video wallpaper with a sparse editorial overlay."""

from pathlib import Path

from widdgyy import Clock, Grid, HBox, Text, VBox, Video

SCENE_DIR = Path(__file__).resolve().parent

stickers = [
    Video(
        str(SCENE_DIR / "night-train.mp4"),
        cls="backdrop",
        width="100%",
        height="100%",
        loop=True,
        z=-1,
    ),
    Grid(
        VBox(
            Text("N", cls="rail-letter"),
            Text("I", cls="rail-letter"),
            Text("G", cls="rail-letter"),
            Text("H", cls="rail-letter"),
            Text("T", cls="rail-letter"),
            cls="rail",
        ),
        HBox(
            Text("WIDDGYY / CINEMA 04", cls="brand"),
            Text("NIGHT TRAIN  ·  35 MM  ·  LOOP", cls="meta-line"),
            cls="topbar",
        ),
        VBox(
            Text("LOCAL TIME", cls="eyebrow"),
            Clock("%H:%M", cls="clock"),
            HBox(
                Text("WEDNESDAY", cls="date"),
                Text("12 NOV 2026", cls="date dim"),
                cls="date-row",
            ),
            cls="clock-block",
        ),
        VBox(
            Text("35.0116° N  /  135.7681° E", cls="coordinate"),
            Text("LAST TRAIN HOME", cls="statement"),
            Text("18°  ·  LIGHT RAIN  ·  STILL AIR", cls="meta-line"),
            Text("Somewhere between the last train and sunrise.", cls="note"),
            cls="meta",
        ),
        HBox(
            Text("終電", cls="kanji"),
            Text("ONE FRAME AT A TIME", cls="colophon"),
            cls="footer",
        ),
        columns=3,
        cls="page",
    ),
]
