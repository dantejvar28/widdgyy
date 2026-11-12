# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Neon Terminal: a type-only cyberpunk dashboard with a nested CSS grid."""

from widdgyy import Clock, Grid, HBox, Text, VBox

stickers = [
    Grid(
        Text("NIGHT CITY", cls="watermark"),
        HBox(
            Text("WIDDGYY://NIGHT_CITY", cls="brand"),
            HBox(
                Text("●", cls="status-dot"),
                Text("SYS.ONLINE", cls="status"),
                cls="status-box",
            ),
            cls="topbar",
        ),
        VBox(
            Text("LOCAL NODE  /  07", cls="eyebrow"),
            Clock("%H:%M:%S", cls="clock"),
            HBox(
                Text("WED 12.11.2026", cls="date"),
                Text("34.6037S / 58.3816W", cls="date dim"),
                cls="date-row",
            ),
            Text("All systems nominal. Awaiting next frame.", cls="note"),
            cls="clock-block",
        ),
        Grid(
            VBox(Text("CPU", cls="stat-label"), Text("12%", cls="stat-value"), cls="stat"),
            VBox(Text("MEM", cls="stat-label"), Text("38%", cls="stat-value"), cls="stat"),
            VBox(Text("GPU", cls="stat-label"), Text("27%", cls="stat-value"), cls="stat"),
            VBox(Text("FPS", cls="stat-label"), Text("60", cls="stat-value"), cls="stat"),
            VBox(Text("UPTIME", cls="stat-label"), Text("04:12:33", cls="stat-value small"), cls="stat wide"),
            VBox(Text("SIGNAL", cls="stat-label"), Text("LOCKED", cls="stat-value small cyan"), cls="stat wide"),
            columns=4,
            cls="stats",
        ),
        HBox(
            Text("VELLO + WGPU", cls="ticker"),
            Text("1080P / 60HZ / NV12", cls="ticker dim"),
            Text("ALPHA 0.2", cls="ticker accent"),
            cls="footer",
        ),
        columns=3,
        cls="page",
    ),
]
