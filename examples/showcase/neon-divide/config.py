# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Neon Divide: a glossy cyberpunk market dashboard over a split-tone wallpaper."""

from pathlib import Path

from widdgyy import Clock, Grid, HBox, Image, Text, VBox

SCENE_DIR = Path(__file__).resolve().parent

stickers = [
    Image(
        str(SCENE_DIR / "background.jpg"),
        cls="wallpaper",
        width="100%",
        height="100%",
        z=-1,
    ),
    Grid(
        HBox(
            HBox(
                Text("WIDDGYY", cls="brand-mark"),
                Text("// NEON DIVIDE", cls="brand-sub"),
                cls="brand",
            ),
            Text("NIGHT CITY  ·  SECTOR 07", cls="top-center"),
            HBox(
                Text("●", cls="status-dot"),
                Text("UPLINK 98%", cls="status"),
                cls="status-box",
            ),
            cls="topbar",
        ),
        VBox(
            Text("LOCAL TIME / SECTOR 07", cls="eyebrow"),
            Clock("%H:%M", cls="clock"),
            HBox(
                Text("WEDNESDAY", cls="date"),
                Text("12 NOV 2026", cls="date dim"),
                Text("SEC", cls="date dim"),
                Clock("%S", cls="seconds"),
                cls="date-row",
            ),
            Text("The city splits into signal and noise.", cls="quote"),
            cls="clock-block",
        ),
        HBox(
            VBox(
                Text("CPU", cls="stat-label"),
                Text("27%", cls="stat-value"),
                cls="stat",
            ),
            VBox(
                Text("MEMORY", cls="stat-label"),
                Text("512 MB", cls="stat-value small"),
                cls="stat",
            ),
            VBox(
                Text("GPU", cls="stat-label"),
                Text("31%", cls="stat-value"),
                cls="stat",
            ),
            VBox(
                Text("NET", cls="stat-label"),
                Text("1.2 GB/S", cls="stat-value small cyan"),
                cls="stat",
            ),
            cls="stats",
        ),
        VBox(
            HBox(
                Text("MARKET FEED", cls="panel-title"),
                HBox(
                    Text("●", cls="market-dot"),
                    Text("LIVE", cls="live"),
                    cls="live-box",
                ),
                cls="panel-head",
            ),
            HBox(
                Text("S&P 500", cls="market-name"),
                Text("5,987.21", cls="market-price"),
                Text("+0.84%", cls="change up"),
                cls="market-row",
            ),
            HBox(
                Text("NASDAQ", cls="market-name"),
                Text("19,234.55", cls="market-price"),
                Text("+1.21%", cls="change up"),
                cls="market-row",
            ),
            HBox(
                Text("DOW JONES", cls="market-name"),
                Text("43,871.09", cls="market-price"),
                Text("+0.36%", cls="change up"),
                cls="market-row",
            ),
            HBox(
                Text("BTC / USD", cls="market-name"),
                Text("97,420.00", cls="market-price"),
                Text("-0.32%", cls="change down"),
                cls="market-row",
            ),
            HBox(
                Text("ETH / USD", cls="market-name"),
                Text("3,412.88", cls="market-price"),
                Text("+0.54%", cls="change up"),
                cls="market-row",
            ),
            HBox(
                Text("EUR / USD", cls="market-name"),
                Text("1.0871", cls="market-price"),
                Text("+0.08%", cls="change up"),
                cls="market-row",
            ),
            HBox(
                Text("VIX", cls="market-name"),
                Text("14.62", cls="market-price"),
                Text("-2.41%", cls="change down"),
                cls="market-row",
            ),
            HBox(
                VBox(
                    Text("18°C", cls="temperature"),
                    Text("ACID RAIN  ·  82%", cls="weather-label"),
                    cls="weather-copy",
                ),
                Text("▲", cls="weather-arrow"),
                cls="market-weather",
            ),
            Text("SIMULATED FEED  /  15 MIN DELAY", cls="disclaimer"),
            cls="market-panel",
        ),
        columns=3,
        cls="page",
    ),
]
