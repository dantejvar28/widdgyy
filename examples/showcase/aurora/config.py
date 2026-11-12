# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Aurora Grid: a no-video scene built almost entirely out of CSS grid areas."""

from widdgyy import Clock, Grid, HBox, Text, VBox

stickers = [
    Grid(
        HBox(
            Text("WIDDGYY", cls="brand"),
            Text("NIGHT DESK / 02", cls="kicker"),
            cls="topbar",
        ),
        HBox(
            Text("AURORA", cls="wordmark"),
            cls="wordmark-wrap",
        ),
        VBox(
            Text("LOCAL TIME", cls="label"),
            Clock("%H:%M", cls="time"),
            Text("BUENOS AIRES  ·  AR", cls="caption"),
            cls="clock-card",
        ),
        VBox(
            Text("22°", cls="temperature"),
            Text("CLEAR NIGHT", cls="label"),
            Text("HUMIDITY 68%  /  WIND 11 KMH", cls="caption"),
            cls="weather-card",
        ),
        VBox(
            Text("The desktop is a canvas.", cls="quote"),
            Text("BLOCK · FLEX · GRID · INTRINSIC SIZING", cls="quote-caption"),
            cls="quote-card",
        ),
        HBox(
            Text("01", cls="footer-index"),
            Text("GPU RENDERED  /  VELLO + WGPU", cls="footer-copy"),
            Text("ALPHA", cls="footer-tag"),
            cls="footer",
        ),
        columns=4,
        cls="page",
    ),
]
