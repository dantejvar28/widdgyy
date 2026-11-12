# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Bonsai Editorial: a warm, print-inspired scene using an image wallpaper."""

from widdgyy import Clock, Grid, HBox, Image, Text, VBox

stickers = [
    Grid(
        Image("../../../assets/tree.png", cls="tree"),
        HBox(cls="veil"),
        HBox(
            Text("WIDDGYY STUDIES", cls="brand"),
            Text("NO. 03  /  KYOTO  35.0116° N", cls="meta-line"),
            cls="masthead",
        ),
        HBox(
            Text("BONSAI", cls="display"),
            VBox(
                Text("盆", cls="kanji"),
                Text("栽", cls="kanji"),
                cls="kanji-stack",
            ),
            cls="title-row",
        ),
        VBox(
            Text("LOCAL TIME", cls="eyebrow"),
            Clock("%H:%M", cls="clock"),
            Text("WEDNESDAY  ·  12 NOVEMBER", cls="date"),
            cls="clock-block",
        ),
        VBox(
            Text("18°", cls="temperature"),
            Text("LIGHT RAIN", cls="eyebrow"),
            Text("HUMIDITY 82%  /  STILL AIR", cls="date"),
            cls="weather-block",
        ),
        VBox(
            Text("CULTIVATE", cls="index-word"),
            Text("SIMPLIFY", cls="index-word"),
            Text("OBSERVE", cls="index-word"),
            HBox(Text("静", cls="seal"), cls="seal-row"),
            cls="index",
        ),
        HBox(
            Text("THE TREE IS THE LAYOUT.", cls="quote"),
            Text("GRID AREAS · INTRINSIC TRACKS · MARGIN COLLAPSE", cls="colophon"),
            cls="footer",
        ),
        columns=3,
        cls="page",
    ),
]
