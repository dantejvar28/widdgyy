# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Minimal Python configuration. Copy it to ~/.config/widdgyy/config.py."""

from widdgyy import ApiText, Clock, HBox, Video

WEATHER = (
    "https://api.open-meteo.com/v1/forecast"
    "?latitude=-34.6&longitude=-58.4&current=temperature_2m,relative_humidity_2m"
)

stickers = [
    Video("assets/wallpaper.mp4", cls="background", z=-1),
    HBox(
        Clock("%H:%M:%S", cls="clock"),
        anchor="center",
        offset=(0, -220),
    ),
    ApiText(
        WEATHER,
        template="{temp}°C  {hum}%",
        fields={"temp": "current.temperature_2m", "hum": "current.relative_humidity_2m"},
        cls="weather",
        anchor="bottom-right",
        offset=(-40, -40),
    ),
]
