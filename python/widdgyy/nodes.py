# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Small, dependency-free DSL for widdgyy's scene description.

The config file builds ordinary Python objects and the engine converts them to
the same JSON document it has always consumed. Styling stays in styles.css:
every node exposes ``cls`` and the engine applies those classes through the
normal CSS cascade.
"""

from __future__ import annotations

from typing import Any, Iterable, Mapping, Sequence

ANCHORS = {
    "top-left",
    "top-center",
    "top-right",
    "center-left",
    "center",
    "center-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
}


def _classes(value: str | Iterable[str] | None) -> str | None:
    if value is None:
        return None
    if isinstance(value, str):
        return " ".join(value.split())
    if isinstance(value, Iterable):
        return " ".join(str(item) for item in value)
    raise TypeError("cls must be a string or an iterable of strings")


def _offset(value: Sequence[float] | None) -> tuple[float, float] | None:
    if value is None:
        return None
    if len(value) != 2:
        raise ValueError("offset must be a pair: offset=(x, y)")
    return (float(value[0]), float(value[1]))


class Sticker:
    """Base class for every node in the scene."""

    sticker_type = ""

    def __init__(
        self,
        *,
        cls: str | Iterable[str] | None = None,
        id: str | None = None,
        anchor: str | None = None,
        offset: Sequence[float] | None = None,
        offset_x: float | None = None,
        offset_y: float | None = None,
        z: int | None = None,
        z_index: int | None = None,
        style: Mapping[str, Any] | None = None,
        **kwargs: Any,
    ) -> None:
        if anchor is not None and anchor not in ANCHORS:
            raise ValueError(
                f"invalid anchor {anchor!r}; expected one of {sorted(ANCHORS)}"
            )
        self.data: dict[str, Any] = {"type": self.sticker_type}
        classes = _classes(cls)
        if classes:
            self.data["class"] = classes
        if id is not None:
            self.data["id"] = id
        if anchor is not None:
            self.data["anchor"] = anchor
        point = _offset(offset)
        if point is None and (offset_x is not None or offset_y is not None):
            point = (float(offset_x or 0.0), float(offset_y or 0.0))
        if point is not None:
            self.data["offset_x"] = point[0]
            self.data["offset_y"] = point[1]
        depth = z if z is not None else z_index
        if depth is not None:
            self.data["z_index"] = depth
        if style is not None:
            self.data["style"] = dict(style)
        for key, value in kwargs.items():
            if value is not None:
                self.data[key] = value

    def to_dict(self) -> dict[str, Any]:
        """Return the JSON-compatible document for this sticker."""
        return dict(self.data)

    def __repr__(self) -> str:  # pragma: no cover - debugging helper
        return f"{type(self).__name__}({self.data!r})"


class Text(Sticker):
    sticker_type = "text"

    def __init__(self, text: str = "", **kwargs: Any) -> None:
        super().__init__(**kwargs)
        self.data["text"] = str(text)


class Clock(Sticker):
    sticker_type = "clock"

    def __init__(self, format: str = "%H:%M:%S", **kwargs: Any) -> None:
        super().__init__(**kwargs)
        self.data["format"] = format


class ApiText(Sticker):
    sticker_type = "api_text"

    def __init__(
        self,
        url: str,
        template: str = "",
        fields: Mapping[str, str] | None = None,
        **kwargs: Any,
    ) -> None:
        super().__init__(**kwargs)
        self.data["url"] = url
        if template:
            self.data["template"] = template
        if fields is not None:
            self.data["fields"] = dict(fields)


class _Media(Sticker):
    sticker_type = "media"

    def __init__(self, path: str, **kwargs: Any) -> None:
        super().__init__(**kwargs)
        self.data["path"] = str(path)


class Video(_Media):
    """Video file. The engine also detects the kind by extension."""


class Image(_Media):
    """Static image file."""


class Gif(_Media):
    """Animated GIF file."""


class _Container(Sticker):
    def __init__(
        self,
        *children: Sticker,
        children_list: Sequence[Sticker] | None = None,
        **kwargs: Any,
    ) -> None:
        # `children` is reserved for the positional form; list= also works.
        super().__init__(**kwargs)
        items: list[Sticker] = list(children)
        if children_list is not None:
            items.extend(children_list)
        if items:
            for item in items:
                if not isinstance(item, Sticker):
                    raise TypeError(
                        f"children must be Sticker instances, got {type(item).__name__}"
                    )
            self.data["children"] = [item.to_dict() for item in items]


class HBox(_Container):
    sticker_type = "hbox"


class VBox(_Container):
    sticker_type = "vbox"


class Grid(_Container):
    sticker_type = "grid"

    def __init__(self, *children: Sticker, columns: int = 2, **kwargs: Any) -> None:
        super().__init__(*children, columns=columns, **kwargs)
